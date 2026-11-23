//! The blob store (task spec `work-references`, "The blob store",
//! "Collection"; canon `work-references.md`): a reference's file
//! bytes in `<data dir>/blobs/<slug>/`, each named by its BLAKE3, never in a
//! repository, a dump or an off-host copy.
//!
//! - **Layout**: the store 0700, created on its first write; `<b3>` 0400;
//!   `.incoming/<16 hex>.part` 0600, an upload in flight; `.swept`, empty,
//!   its mtime the last sweep's.
//! - **A write**, in order: [`Store::begin`] judges the caps on the declared
//!   size, walks the store (its files' bytes, and a due sweep's
//!   candidates, `.incoming` aside) and creates the `.part` exclusively;
//!   [`Upload::write`] passes each piece through the hasher, the sniff and
//!   one 64 KiB buffer, the floor read every 16 MiB; [`Upload::finish`]
//!   syncs the file and reads its facts; [`Store::commit`], the one step a
//!   daemon runs in the project's turn, renames it into place (or, for
//!   bytes stored already, removes it and touches the file), syncs the
//!   directory, judges the file count when the bytes are new, and sweeps
//!   when due. A refusal, a cut body, a dropped
//!   connection or a panic removes the `.part` ([`PartGuard`]).
//! - **Seams** ([`BlobSeams`]): the free space and the clock are the
//!   caller's to inject (RF4, RF13).
//!
//! No pixel, sample or frame is decoded: the type and facts are core's
//! [`specengine_core::blob`] over the first bytes and headers.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use specengine_core::blob::{
    BLOB_BUFFER, BLOB_FILES_MAX, BLOB_GRACE, BlobCaps, BlobRefusal, BlobRefusalKind, FLOOR_EVERY,
    Facts, Intake, Judged, Media, PART_GRACE, Received, SNIFF_BYTES, SWEEP_EVERY, SWEEP_MAX, facts,
    files_refusal, is_b3, sniff,
};
use specengine_core::reference::ReferencesConfig;
use specengine_store::SqliteQueue;

/// The directory of an upload in flight.
const INCOMING: &str = ".incoming";

/// The empty file whose mtime is the last sweep's.
const SWEPT: &str = ".swept";

/// The bytes still free on the file system holding a path.
pub type FreeSpace = Arc<dyn Fn(&Path) -> io::Result<u64> + Send + Sync>;

/// Now.
pub type Clock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// The free space and the clock a store reads, injectable.
#[derive(Clone)]
pub struct BlobSeams {
    pub free_space: FreeSpace,
    pub now: Clock,
}

impl Default for BlobSeams {
    fn default() -> Self {
        Self {
            free_space: Arc::new(crate::group::available_bytes),
            now: Arc::new(SystemTime::now),
        }
    }
}

impl std::fmt::Debug for BlobSeams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BlobSeams")
    }
}

/// The bytes a project's uploads in flight have declared, shared by them
/// (a daemon keeps one per project): the quota counts them.
#[derive(Debug, Clone, Default)]
pub struct BlobFlight(Arc<AtomicU64>);

impl BlobFlight {
    /// The bytes declared now.
    pub fn bytes(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    /// `bytes` declared in flight at once, and what the project's other
    /// uploads had declared before: two uploads that begin together each
    /// count the other's bytes, so neither passes the quota on the other's.
    fn take(&self, bytes: u64) -> (FlightGuard, u64) {
        let before = self.0.fetch_add(bytes, Ordering::SeqCst);
        (
            FlightGuard {
                flight: Arc::clone(&self.0),
                bytes,
            },
            before,
        )
    }
}

/// An upload's declared bytes in flight, given back when dropped.
#[derive(Debug)]
struct FlightGuard {
    flight: Arc<AtomicU64>,
    bytes: u64,
}

impl Drop for FlightGuard {
    fn drop(&mut self) {
        self.flight.fetch_sub(self.bytes, Ordering::SeqCst);
    }
}

/// Why a store's write did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobFailure {
    /// A cap, a type, `allow`, the body: exit 1, a 413, 415 or 400.
    Refused(BlobRefusal),
    /// The store cannot be created or written: exit 2, a 503.
    Cannot(String),
}

/// The store's `.part` of an upload: removed when dropped unless renamed.
#[derive(Debug)]
struct PartGuard {
    path: Option<PathBuf>,
}

impl PartGuard {
    fn path(&self) -> &Path {
        self.path.as_deref().unwrap_or_else(|| Path::new(""))
    }

    /// The file is renamed or removed by the caller: nothing left to drop.
    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

/// A due sweep's candidates, found outside the turn.
#[derive(Debug, Default)]
struct Sweep {
    /// Stored files no live reference named, over [`BLOB_GRACE`] old.
    blobs: Vec<String>,
    /// `.part` files over [`PART_GRACE`] old.
    parts: Vec<PathBuf>,
}

/// A project's store.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
    caps: BlobCaps,
    seams: BlobSeams,
}

/// The store's directory of `slug`.
pub fn store_dir(data_dir: &Path, slug: &str) -> PathBuf {
    data_dir.join("blobs").join(slug)
}

/// An upload in flight, outside the turn.
pub struct Upload {
    store: Store,
    part: PartGuard,
    writer: BufWriter<File>,
    intake: Intake,
    declared: u64,
    next_floor: u64,
    sweep: Option<Sweep>,
    flight: FlightGuard,
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload")
            .field("part", &self.part)
            .field("declared", &self.declared)
            .field("received", &self.intake.size())
            .finish_non_exhaustive()
    }
}

/// An upload whose bytes are all in, synced, typed and their facts read:
/// what the turn renames.
#[derive(Debug)]
pub struct Spooled {
    part: PartGuard,
    received: Received,
    facts: Facts,
    sweep: Option<Sweep>,
    _flight: FlightGuard,
}

impl Spooled {
    /// The bytes' BLAKE3.
    pub fn b3(&self) -> &str {
        &self.received.b3
    }
}

/// `new`: the store named no such file before; `existing`: equal bytes were
/// stored already.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredAs {
    New,
    Existing,
}

impl StoredAs {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Existing => "existing",
        }
    }
}

/// A stored file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub b3: String,
    pub judged: Judged,
    pub stored: StoredAs,
}

/// A file named by a reference, opened to be served.
#[derive(Debug)]
pub struct BlobFile {
    pub file: File,
    pub size: u64,
}

impl Store {
    pub fn new(dir: PathBuf, caps: BlobCaps, seams: BlobSeams) -> Self {
        Self { dir, caps, seams }
    }

    /// The path of the file `b3`.
    pub fn path_of(&self, b3: &str) -> PathBuf {
        self.dir.join(b3)
    }

    /// The store holds the file `b3` now.
    pub fn holds(&self, b3: &str) -> bool {
        is_b3(b3) && fs::symlink_metadata(self.path_of(b3)).is_ok_and(|meta| meta.is_file())
    }

    /// The line of a store that cannot be created or written.
    pub fn unwritable_line(&self) -> String {
        format!(
            "the blob store {} is not writable: on a hub, chown 10001:10001 \
             /data/specengine/blobs",
            self.dir.display()
        )
    }

    fn cannot(&self) -> BlobFailure {
        BlobFailure::Cannot(self.unwritable_line())
    }

    /// The store and its `.incoming`, 0700, created when absent.
    fn ensure(&self) -> Result<(), BlobFailure> {
        let incoming = self.dir.join(INCOMING);
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&incoming)
            .map_err(|_| self.cannot())
    }

    fn now(&self) -> SystemTime {
        (self.seams.now)()
    }

    /// `modified` is older than `grace` now.
    fn older(&self, modified: SystemTime, grace: Duration) -> bool {
        self.now()
            .duration_since(modified)
            .is_ok_and(|age| age > grace)
    }

    /// A sweep is due: `.swept` absent or over [`SWEEP_EVERY`] old.
    fn sweep_due(&self) -> bool {
        match fs::metadata(self.dir.join(SWEPT)).and_then(|meta| meta.modified()) {
            Ok(modified) => self.older(modified, SWEEP_EVERY),
            Err(_) => true,
        }
    }

    /// An upload of `declared` bytes of the file `name` begun (task spec
    /// `work-references`, "The blob store" steps 1 and 2): the file cap on
    /// the declared size before anything is created; the declared bytes
    /// taken in flight, before the walk, so every other upload is counted at
    /// least once (still in flight at the take, or renamed before the walk);
    /// the store walked (its files' bytes; a due sweep's candidates, the
    /// live rows read once through `queue`); the quota judged on the walk,
    /// the other uploads' bytes and these, every refusal giving them back;
    /// the floor; then the `.part`, created exclusively. The file count is
    /// judged in the turn ([`Self::commit`]).
    pub fn begin(
        &self,
        name: &str,
        declared: u64,
        allow: Option<ReferencesConfig>,
        flight: &BlobFlight,
        queue: Option<&SqliteQueue>,
    ) -> Result<Upload, BlobFailure> {
        if let Some(refusal) = self.caps.file_refusal(name, declared) {
            return Err(BlobFailure::Refused(refusal));
        }
        self.ensure()?;
        let (flight, before) = flight.take(declared);
        let walk = self.walk(queue)?;
        if let Some(refusal) = self.caps.quota_refusal_beside(walk.bytes, before, declared) {
            return Err(BlobFailure::Refused(refusal));
        }
        let available = (self.seams.free_space)(&self.dir).map_err(|_| self.cannot())?;
        if let Some(refusal) = self.caps.floor_refusal(available, declared) {
            return Err(BlobFailure::Refused(refusal));
        }
        let (path, file) = self.part()?;
        let part = PartGuard { path: Some(path) };
        Ok(Upload {
            store: self.clone(),
            part,
            writer: BufWriter::with_capacity(BLOB_BUFFER, file),
            intake: Intake::new(name, self.caps.file_max, allow),
            declared,
            next_floor: FLOOR_EVERY,
            sweep: walk.sweep,
            flight,
        })
    }

    /// A new `.part`, `O_CREAT|O_EXCL`, 0600, under a random 16-hex name.
    fn part(&self) -> Result<(PathBuf, File), BlobFailure> {
        for _ in 0..8 {
            let path = self
                .dir
                .join(INCOMING)
                .join(format!("{}.part", random_hex()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => return Ok((path, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(self.cannot()),
            }
        }
        Err(self.cannot())
    }

    /// The store's files counted and their bytes summed, `.incoming` and
    /// `.swept` aside; when a sweep is due, its candidates.
    fn walk(&self, queue: Option<&SqliteQueue>) -> Result<Walk, BlobFailure> {
        let due = self.sweep_due();
        let mut walk = Walk::default();
        let mut old: BTreeSet<String> = BTreeSet::new();
        let entries = fs::read_dir(&self.dir).map_err(|_| self.cannot())?;
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !is_b3(name) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            walk.bytes = walk.bytes.saturating_add(meta.len());
            if due && meta.modified().is_ok_and(|at| self.older(at, BLOB_GRACE)) {
                old.insert(name.to_owned());
            }
        }
        if !due {
            return Ok(walk);
        }
        let mut sweep = Sweep::default();
        if !old.is_empty() {
            if let Some(queue) = queue {
                let named = queue.each_live_blob(|hash| {
                    old.remove(hash);
                });
                // A queue that cannot be read now sweeps nothing.
                if named.is_err() {
                    old.clear();
                }
            }
            sweep.blobs = old.into_iter().take(SWEEP_MAX).collect();
        }
        if let Ok(entries) = fs::read_dir(self.dir.join(INCOMING)) {
            let mut parts: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .metadata()
                        .and_then(|meta| meta.modified())
                        .is_ok_and(|at| self.older(at, PART_GRACE))
                })
                .map(|entry| entry.path())
                .collect();
            parts.sort();
            parts.truncate(SWEEP_MAX.saturating_sub(sweep.blobs.len()));
            sweep.parts = parts;
        }
        walk.sweep = Some(sweep);
        Ok(walk)
    }

    /// `staged` put in place (task spec `work-references`, "The blob store"
    /// step 4), the one step a daemon runs in the project's turn: when the
    /// store holds no such file, the `.part` renamed to its BLAKE3 and made
    /// 0400 (refused when the store, counted now, holds
    /// [`specengine_core::blob::BLOB_FILES_MAX`] files already); else the
    /// `.part` removed and the file's mtime set to
    /// now. The directory synced; then the sweep, when due, each candidate
    /// read again (its mtime, and `queue`'s indexed lookup) before it goes.
    pub fn commit(
        &self,
        staged: Spooled,
        queue: Option<&SqliteQueue>,
    ) -> Result<Committed, BlobFailure> {
        let Spooled {
            mut part,
            received,
            facts,
            sweep,
            _flight,
        } = staged;
        let target = self.path_of(&received.b3);
        let stored = if fs::symlink_metadata(&target).is_ok_and(|meta| meta.is_file()) {
            let _ = fs::remove_file(part.path());
            part.disarm();
            File::open(&target)
                .and_then(|file| file.set_modified(self.now()))
                .map_err(|_| self.cannot())?;
            StoredAs::Existing
        } else {
            // Counted in the turn: no two uploads' renames pass it together.
            if let Some(refusal) = files_refusal(self.files()?) {
                return Err(BlobFailure::Refused(refusal));
            }
            fs::rename(part.path(), &target).map_err(|_| self.cannot())?;
            part.disarm();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o400))
                .map_err(|_| self.cannot())?;
            StoredAs::New
        };
        File::open(&self.dir)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| self.cannot())?;
        if let Some(sweep) = sweep {
            self.sweep(sweep, queue);
        }
        Ok(Committed {
            b3: received.b3,
            judged: Judged {
                media: received.media,
                facts,
                size: received.size,
            },
            stored,
        })
    }

    /// The sweep's turn: unless another upload swept meanwhile, each
    /// candidate whose mtime is still old and which no live reference names
    /// now is unlinked, each stale `.part` too; `.swept` touched. Nothing it
    /// fails to remove fails the upload.
    fn sweep(&self, sweep: Sweep, queue: Option<&SqliteQueue>) {
        if !self.sweep_due() {
            return;
        }
        for hash in &sweep.blobs {
            let path = self.path_of(hash);
            let old = fs::symlink_metadata(&path)
                .and_then(|meta| meta.modified())
                .is_ok_and(|at| self.older(at, BLOB_GRACE));
            if !old {
                continue;
            }
            let named = match queue {
                Some(queue) => queue.blob_named(hash).unwrap_or(true),
                None => false,
            };
            if !named {
                let _ = fs::remove_file(&path);
            }
        }
        for path in &sweep.parts {
            let old = fs::symlink_metadata(path)
                .and_then(|meta| meta.modified())
                .is_ok_and(|at| self.older(at, PART_GRACE));
            if old {
                let _ = fs::remove_file(path);
            }
        }
        let swept = self.dir.join(SWEPT);
        let touched = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&swept)
            .and_then(|file| file.set_modified(self.now()));
        let _ = touched;
    }

    /// The store's files now, counted up to
    /// [`specengine_core::blob::BLOB_FILES_MAX`] (names alone, no file
    /// opened).
    fn files(&self) -> Result<u64, BlobFailure> {
        let entries = fs::read_dir(&self.dir).map_err(|_| self.cannot())?;
        let mut files = 0;
        for entry in entries.flatten() {
            if files >= BLOB_FILES_MAX {
                break;
            }
            let named = entry.file_name().to_str().is_some_and(is_b3);
            if named && entry.file_type().is_ok_and(|kind| kind.is_file()) {
                files += 1;
            }
        }
        Ok(files)
    }

    /// Each of `hashes` unlinked (a delete's collection, in its turn);
    /// a file already gone is no failure.
    pub fn unlink(&self, hashes: &[String]) {
        for hash in hashes {
            if is_b3(hash) {
                let _ = fs::remove_file(self.path_of(hash));
            }
        }
    }

    /// The stored file `b3` opened, its size read; `None` when the store
    /// holds no such file.
    pub fn open(&self, b3: &str) -> Option<BlobFile> {
        if !is_b3(b3) {
            return None;
        }
        let file = File::open(self.path_of(b3)).ok()?;
        let meta = file.metadata().ok()?;
        meta.is_file().then(|| BlobFile {
            size: meta.len(),
            file,
        })
    }

    /// The stored file `b3` judged as an upload was: its sniffed type, its
    /// facts and its size; `None` when the store holds no such file or its
    /// bytes are no type stored here.
    pub fn judge(&self, b3: &str) -> Option<Judged> {
        let BlobFile { mut file, size } = self.open(b3)?;
        let mut head = Vec::with_capacity(SNIFF_BYTES);
        (&mut file)
            .take(SNIFF_BYTES as u64)
            .read_to_end(&mut head)
            .ok()?;
        let media = sniff(&head).ok()?;
        Some(Judged {
            media,
            facts: facts(media, &mut file, size),
            size,
        })
    }

    /// The sniffed type of the stored file `b3` (an agent copy served).
    pub fn media_of(&self, b3: &str) -> Option<Media> {
        let BlobFile { file, .. } = self.open(b3)?;
        let mut head = Vec::with_capacity(SNIFF_BYTES);
        file.take(SNIFF_BYTES as u64).read_to_end(&mut head).ok()?;
        sniff(&head).ok()
    }
}

/// What a store's walk found.
#[derive(Debug, Default)]
struct Walk {
    bytes: u64,
    sweep: Option<Sweep>,
}

impl Upload {
    /// The bytes received so far.
    pub fn received(&self) -> u64 {
        self.intake.size()
    }

    /// `chunk` passed on (step 1): hashed and sniffed first, so a refused
    /// type stops the body before it is kept; then written; past each
    /// [`FLOOR_EVERY`] bytes the floor read again with what is still to
    /// come.
    pub fn write(&mut self, chunk: &[u8]) -> Result<(), BlobFailure> {
        self.intake.take(chunk).map_err(BlobFailure::Refused)?;
        let received = self.intake.size();
        if received > self.declared {
            return Err(BlobFailure::Refused(BlobRefusal {
                kind: BlobRefusalKind::Body,
                line: format!(
                    "refused: the body is longer than the {} bytes it declared",
                    self.declared
                ),
            }));
        }
        self.writer
            .write_all(chunk)
            .map_err(|_| self.store.cannot())?;
        if received >= self.next_floor {
            self.next_floor = self.next_floor.saturating_add(FLOOR_EVERY);
            let available =
                (self.store.seams.free_space)(&self.store.dir).map_err(|_| self.store.cannot())?;
            if let Some(refusal) = self
                .store
                .caps
                .floor_refusal(available, self.declared - received)
            {
                return Err(BlobFailure::Refused(refusal));
            }
        }
        Ok(())
    }

    /// The bytes are all in (step 3): the declared size checked, the type
    /// of a short file sniffed now, the `.part` synced, its facts read.
    pub fn finish(self) -> Result<Spooled, BlobFailure> {
        let Self {
            store,
            part,
            writer,
            intake,
            declared,
            sweep,
            flight,
            ..
        } = self;
        if intake.size() < declared {
            return Err(BlobFailure::Refused(BlobRefusal {
                kind: BlobRefusalKind::Body,
                line: format!(
                    "refused: the body ended at {} of the {declared} bytes it declared",
                    intake.size()
                ),
            }));
        }
        let received = intake.finish().map_err(BlobFailure::Refused)?;
        let file = writer.into_inner().map_err(|_| store.cannot())?;
        file.sync_all().map_err(|_| store.cannot())?;
        drop(file);
        let reader = File::open(part.path()).map_err(|_| store.cannot())?;
        let facts = facts(received.media, reader, received.size);
        Ok(Spooled {
            part,
            received,
            facts,
            sweep,
            _flight: flight,
        })
    }
}

/// 16 lower-case hex digits from the system's random source; else from the
/// clock, the process and a counter (`O_EXCL` keeps a name unique either
/// way).
fn random_hex() -> String {
    let mut bytes = [0u8; 8];
    let read = File::open("/dev/urandom").and_then(|mut source| source.read_exact(&mut bytes));
    if read.is_err() {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos() as u64);
        let mixed = nanos
            ^ u64::from(std::process::id()).rotate_left(32)
            ^ COUNTER.fetch_add(1, Ordering::SeqCst).rotate_left(16);
        bytes = mixed.to_le_bytes();
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
