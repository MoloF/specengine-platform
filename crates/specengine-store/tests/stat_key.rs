//! docs/features/read-trust.md (RM-0054) "Acceptance criteria" AC-8, the
//! store's half: the freshness step keyed by stat ("Data", "The key and the
//! racy rule", R1-R7).
//! - (a) A warm `update` of a 50-file tree last changed 2.1 s before its
//!   first update reads no file (`hashed` 0, the counting source's reads 0,
//!   `unchanged` 50); `update_paths` still reads what it is named and
//!   `rebuild` every file (R2, R7).
//! - (b) A same-size edit with the mtime restored to the nanosecond is read:
//!   the ctime moved.
//! - (c) A source whose `stat` gives one fixed key, its mtime an hour back
//!   and its ctime the wall clock of the source's making (or a time in the
//!   future), is read on every update while its bytes change at one size;
//!   with the ctime an hour back too, the second update reads nothing.
//! - (d) A touched file read again stores its new key on an unchanged hash
//!   (`parsed` 0, `hashed` 1), through `update_paths` and through `update`;
//!   the next update reads nothing.
//! - (e) Two copies of one tree, made 2 s apart at two roots, dump equal
//!   (root masked) while their `file_stats` differ; neither table is a line
//!   of the dump.
//! - (f) A warm no-op opens no write transaction: under a second
//!   connection's `BEGIN IMMEDIATE` it returns `Ok` within 1 s with
//!   `committed` false, and without it the second connection's
//!   `PRAGMA data_version` does not move; again with a mode-000 file.
//! - (g) `walk_notes` hold the listing's three facts and survive an
//!   `update_paths`; `trusted_notes` gives them, and `None` for an old
//!   stamp, another `[ids]` or `[paths]` fingerprint, or no worktree row.
//!   An `update_paths` that lists a missing root's first file, or unlists a
//!   root's last, walks, so the notes never go stale (review iteration 2).
//!
//! (e), (f) and (g) look at the database through a raw connection: what a
//! no-op leaves unwritten, a table the dump leaves out and a stale stamp
//! cannot be seen through the store's API (see `api.rs`).
//!
//! Every test works under its own scratch directory in
//! `std::env::temp_dir()`, removed on drop. A wait for the racy window
//! sleeps 2.1 s, no more.

#![cfg(unix)]

mod common;

use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::{Corpus, Scratch, blake3_hex};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{
    IndexWriter, Listing, Probed, Source, SpecIndex, SqliteIndex, StatKey, UpdateReport, WalkNotes,
};

/// The racy window's wait (`RACY_NS` is 2 s).
const PAST_RACY: Duration = Duration::from_millis(2100);
/// One hour, in nanoseconds.
const HOUR_NS: i64 = 3_600_000_000_000;
/// The scratch tree's config: one prefix, one written root.
const CONFIG: &str = "[ids]\nR = { kind = \"requirement\", width = 2 }\n\n\
                      [paths]\nroots = [\"docs\"]\n";

/// The file of record `number`, its body `body`.
fn record(number: usize, body: &str) -> String {
    format!(
        "---\nid: R-{number:02}\nkind: requirement\nstatus: draft\n---\n\n\
         # Item {number:02}\n\n{body}\n"
    )
}

fn record_path(number: usize) -> String {
    format!("docs/r-{number:02}.md")
}

/// A tree at `<scratch>/<dir>` with [`CONFIG`] (or `config`) and records
/// 1 to `count`.
fn tree(scratch: &Scratch, dir: &str, config: &str, count: usize) -> Corpus {
    let root = scratch.join(dir);
    common::write(&root, "specengine.toml", config);
    for number in 1..=count {
        common::write(
            &root,
            &record_path(number),
            record(number, &format!("Body text {number:02} ka zo.")),
        );
    }
    Corpus::load(&root)
}

/// Nanoseconds since the Unix epoch, now.
fn now_ns() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("after the epoch")
            .as_nanos(),
    )
    .expect("before 2262")
}

/// A source over `inner` that records every `read` and forwards the rest,
/// `stat` included.
struct Counting<'a> {
    inner: &'a dyn Source,
    reads: RefCell<Vec<String>>,
    stats: Cell<usize>,
}

impl<'a> Counting<'a> {
    fn new(inner: &'a dyn Source) -> Self {
        Self {
            inner,
            reads: RefCell::new(Vec::new()),
            stats: Cell::new(0),
        }
    }

    /// The paths read since the last take, in call order.
    fn take_reads(&self) -> Vec<String> {
        std::mem::take(&mut *self.reads.borrow_mut())
    }
}

impl Source for Counting<'_> {
    fn root(&self) -> &Path {
        self.inner.root()
    }

    fn paths(&self) -> &Paths {
        self.inner.paths()
    }

    fn list(&self) -> io::Result<Listing> {
        self.inner.list()
    }

    fn probe(&self, path: &str) -> bool {
        self.inner.probe(path)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.reads.borrow_mut().push(path.to_owned());
        self.inner.read(path)
    }

    fn is_dir(&self, path: &str) -> bool {
        self.inner.is_dir(path)
    }

    fn probe_paths(&self, paths: &[&str]) -> Vec<Probed> {
        self.inner.probe_paths(paths)
    }

    fn stat(&self, path: &str) -> Option<StatKey> {
        self.stats.set(self.stats.get() + 1);
        self.inner.stat(path)
    }
}

/// A source over `inner` whose `stat` gives `key` for every path, and that
/// counts its reads.
struct FixedKey<'a> {
    inner: &'a dyn Source,
    key: StatKey,
    reads: Cell<usize>,
}

impl Source for FixedKey<'_> {
    fn root(&self) -> &Path {
        self.inner.root()
    }

    fn paths(&self) -> &Paths {
        self.inner.paths()
    }

    fn list(&self) -> io::Result<Listing> {
        self.inner.list()
    }

    fn probe(&self, path: &str) -> bool {
        self.inner.probe(path)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.reads.set(self.reads.get() + 1);
        self.inner.read(path)
    }

    fn is_dir(&self, path: &str) -> bool {
        self.inner.is_dir(path)
    }

    fn probe_paths(&self, paths: &[&str]) -> Vec<Probed> {
        self.inner.probe_paths(paths)
    }

    fn stat(&self, _path: &str) -> Option<StatKey> {
        Some(self.key)
    }
}

fn update(index: &mut SqliteIndex, source: &dyn Source, scheme: &IdScheme) -> UpdateReport {
    index
        .update(source, scheme)
        .unwrap_or_else(|error| panic!("update: {error}"))
}

fn update_paths(
    index: &mut SqliteIndex,
    source: &dyn Source,
    scheme: &IdScheme,
    paths: &[&str],
) -> UpdateReport {
    index
        .update_paths(source, scheme, paths)
        .unwrap_or_else(|error| panic!("update_paths {paths:?}: {error}"))
}

/// The stored BLAKE3 of `path`.
fn stored_hash(index: &SqliteIndex, path: &str) -> Option<String> {
    index
        .file(path)
        .expect("file")
        .unwrap_or_else(|| panic!("{path} is stored"))
        .blake3
}

/// A raw connection to the index's database file, `busy_timeout` 5 s.
fn raw(db: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(db).expect("raw connection");
    conn.busy_timeout(Duration::from_secs(5))
        .expect("busy timeout");
    conn
}

/// Every `file_stats` row of the database, by path:
/// `(path, size, mtime_ns, ctime_ns, ino, dev, taken_ns)`.
fn file_stats(db: &Path) -> Vec<(String, i64, i64, i64, i64, i64, i64)> {
    let conn = raw(db);
    let mut statement = conn
        .prepare(
            "SELECT f.path, s.size, s.mtime_ns, s.ctime_ns, s.ino, s.dev, s.taken_ns \
             FROM file_stats s JOIN files f ON f.file_id = s.file_id ORDER BY f.path",
        )
        .expect("the file_stats table");
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })
        .expect("query")
        .map(|row| row.expect("row"))
        .collect()
}

/// Every `walk_notes` row: `(missing_roots, unreadable_dirs, skipped_names)`.
fn walk_notes(db: &Path) -> Vec<(String, String, i64)> {
    let conn = raw(db);
    let mut statement = conn
        .prepare("SELECT missing_roots, unreadable_dirs, skipped_names FROM walk_notes")
        .expect("the walk_notes table");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("query")
        .map(|row| row.expect("row"))
        .collect()
}

fn data_version(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("PRAGMA data_version", [], |row| row.get(0))
        .expect("data_version")
}

/// The worktree dump with the root replaced by `<root>`.
fn masked_dump(index: &SqliteIndex, root: &Path) -> String {
    index
        .dump_worktree()
        .expect("dump")
        .replace(root.to_str().expect("UTF-8 root"), "<root>")
}

/// A mode-000 entry is unreadable here (not running as root).
fn mode_000_binds(path: &Path) -> bool {
    if path.is_dir() {
        fs::read_dir(path).is_err()
    } else {
        fs::read(path).is_err()
    }
}

// ---------------------------------------------------------------- AC-8 (a)

#[test]
fn ac8a_a_warm_update_of_fifty_files_reads_none() {
    let scratch = Scratch::new("stat-key-a");
    let corpus = tree(&scratch, "wt", CONFIG, 50);
    std::thread::sleep(PAST_RACY);
    let mut index = corpus.open(&scratch.db("index"));
    let walking = corpus.tree();
    let counting = Counting::new(&walking);

    let cold = update(&mut index, &counting, &corpus.scheme);
    assert_eq!(cold.walked, 50, "{cold:?}");
    assert_eq!(cold.parsed, 50, "{cold:?}");
    assert_eq!(
        cold.hashed, 50,
        "the first update reads every file: {cold:?}"
    );
    assert_eq!(counting.take_reads().len(), 50, "the first update's reads");
    assert!(
        counting.stats.get() >= 50,
        "premise: the store asks the source for each file's key ({} calls)",
        counting.stats.get()
    );
    assert_eq!(
        file_stats(&scratch.db("index")).len(),
        50,
        "every read file keeps its key"
    );

    let warm = update(&mut index, &counting, &corpus.scheme);
    assert_eq!(warm.hashed, 0, "a warm update reads no file: {warm:?}");
    assert_eq!(
        counting.take_reads(),
        Vec::<String>::new(),
        "a warm update calls `read` on no file"
    );
    assert_eq!(warm.unchanged, 50, "{warm:?}");
    assert_eq!(warm.parsed, 0, "{warm:?}");
    assert!(!warm.committed, "a warm update commits nothing: {warm:?}");

    // R2: `update_paths` reads every path it is named, whatever its key.
    let named = record_path(7);
    let by_path = update_paths(&mut index, &counting, &corpus.scheme, &[&named]);
    assert_eq!(by_path.hashed, 1, "{by_path:?}");
    assert_eq!(counting.take_reads(), std::slice::from_ref(&named));
    assert_eq!(by_path.parsed, 0, "{by_path:?}");

    // R2, R7: a rebuild reads every file.
    let rebuilt = index.rebuild(&counting, &corpus.scheme).expect("rebuild");
    assert_eq!(
        rebuilt.hashed, 50,
        "a rebuild reads every file: {rebuilt:?}"
    );
    assert_eq!(counting.take_reads().len(), 50, "the rebuild's reads");
    assert_eq!(
        file_stats(&scratch.db("index")).len(),
        50,
        "a rebuild stores every key"
    );
    let after = update(&mut index, &counting, &corpus.scheme);
    assert_eq!(after.hashed, 0, "warm again after the rebuild: {after:?}");
    assert!(counting.take_reads().is_empty());
}

// ---------------------------------------------------------------- AC-8 (b)

#[test]
fn ac8b_a_same_size_edit_with_its_mtime_restored_is_read() {
    let scratch = Scratch::new("stat-key-b");
    let corpus = tree(&scratch, "wt", CONFIG, 3);
    std::thread::sleep(PAST_RACY);
    let mut index = corpus.open(&scratch.db("index"));
    let walking = corpus.tree();
    let counting = Counting::new(&walking);
    update(&mut index, &counting, &corpus.scheme);
    counting.take_reads();

    let path = record_path(2);
    let absolute = corpus.root.join(&path);
    let before = fs::metadata(&absolute).expect("metadata");
    let old_mtime = before.modified().expect("mtime");
    let text = corpus.read_text(&path);
    let edited = text.replace("Body text 02 ka zo.", "Body text 02 vo xi.");
    assert_ne!(edited, text, "premise: the edit changes the bytes");
    assert_eq!(edited.len(), text.len(), "premise: a same-size edit");
    fs::write(&absolute, &edited).expect("edit in place");
    let file = fs::File::options()
        .write(true)
        .open(&absolute)
        .expect("open");
    file.set_times(fs::FileTimes::new().set_modified(old_mtime))
        .expect("set_times");
    drop(file);
    let after = fs::metadata(&absolute).expect("metadata");
    assert_eq!(
        (after.len(), after.mtime(), after.mtime_nsec(), after.ino()),
        (
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ino()
        ),
        "premise: size, mtime to the nanosecond and inode are restored"
    );
    assert_ne!(
        (after.ctime(), after.ctime_nsec()),
        (before.ctime(), before.ctime_nsec()),
        "premise: the ctime moved"
    );

    let report = update(&mut index, &counting, &corpus.scheme);
    assert_eq!(
        counting.take_reads(),
        std::slice::from_ref(&path),
        "the next update reads the edited file: {report:?}"
    );
    assert_eq!(report.parsed, 1, "{report:?}");
    assert_eq!(
        stored_hash(&index, &path),
        Some(blake3_hex(edited.as_bytes())),
        "the new rows are stored"
    );
}

// ---------------------------------------------------------------- AC-8 (c)

/// Three same-size versions of one file through a source with one fixed
/// key: `(reads, stored hash equals the bytes)` of each update after the
/// first.
fn fixed_key_updates(name: &str, ctime_ns: impl Fn(i64) -> i64) -> Vec<(usize, bool, u128)> {
    let scratch = Scratch::new(name);
    let corpus = tree(&scratch, "wt", CONFIG, 1);
    let path = record_path(1);
    let mut index = corpus.open(&scratch.db("index"));
    let walking = corpus.tree();
    let made = now_ns();
    let size = corpus.bytes(&path).len() as u64;
    let source = FixedKey {
        inner: &walking,
        key: StatKey {
            size,
            mtime_ns: made - HOUR_NS,
            ctime_ns: ctime_ns(made),
            ino: 7,
            dev: 9,
        },
        reads: Cell::new(0),
    };
    let started = Instant::now();
    update(&mut index, &source, &corpus.scheme);
    assert_eq!(
        source.reads.replace(0),
        1,
        "the first update reads the file"
    );
    let mut out = Vec::new();
    for word in ["vo xi", "qa je", "ju ky"] {
        let text = record(1, &format!("Body text 01 {word}."));
        assert_eq!(text.len() as u64, size, "premise: one size");
        corpus.write(&path, &text);
        let report = update(&mut index, &source, &corpus.scheme);
        let reads = source.reads.replace(0);
        assert_eq!(
            report.hashed, reads,
            "`hashed` counts the reads: {report:?}"
        );
        out.push((
            reads,
            stored_hash(&index, &path) == Some(blake3_hex(text.as_bytes())),
            started.elapsed().as_millis(),
        ));
    }
    out
}

#[test]
fn ac8c_a_key_within_the_racy_window_is_read_on_every_update() {
    // The ctime is the wall clock of the key's making: every update runs
    // within 2 s of it, so the stored key stays racy.
    let racy = fixed_key_updates("stat-key-c-racy", |made| made);
    assert!(
        racy.iter().all(|(_, _, ms)| *ms < 1500),
        "premise: the updates ran within 1.5 s of the key: {racy:?}"
    );
    assert!(
        racy.iter().all(|(reads, fresh, _)| *reads == 1 && *fresh),
        "a racy key, its mtime an hour back, is read on every update and its \
         new bytes stored: (reads, fresh, ms) {racy:?}"
    );

    // A file dated in the future (the wall clock set back): racy forever.
    let future = fixed_key_updates("stat-key-c-future", |made| made + HOUR_NS);
    assert!(
        future.iter().all(|(reads, fresh, _)| *reads == 1 && *fresh),
        "a key dated in the future is read on every update: {future:?}"
    );

    // The control: the ctime an hour back too, the key is trusted.
    let old = fixed_key_updates("stat-key-c-old", |made| made - HOUR_NS);
    assert_eq!(
        old[0].0, 0,
        "a key taken an hour after its change is trusted: the second update \
         reads nothing: {old:?}"
    );
}

// ---------------------------------------------------------------- AC-8 (d)

#[test]
fn ac8d_a_touched_file_read_again_stores_its_new_key() {
    let scratch = Scratch::new("stat-key-d");
    let corpus = tree(&scratch, "wt", CONFIG, 4);
    std::thread::sleep(PAST_RACY);
    let mut index = corpus.open(&scratch.db("index"));
    let walking = corpus.tree();
    let counting = Counting::new(&walking);
    update(&mut index, &counting, &corpus.scheme);
    counting.take_reads();

    let by_update = record_path(1);
    let by_path = record_path(3);
    let now = SystemTime::now();
    for path in [&by_update, &by_path] {
        let file = fs::File::options()
            .write(true)
            .open(corpus.root.join(path))
            .expect("open");
        file.set_times(fs::FileTimes::new().set_modified(now))
            .expect("touch");
    }
    std::thread::sleep(PAST_RACY);

    // `update_paths` first: it reads its path, finds the hash unchanged
    // and stores the new key.
    let report = update_paths(&mut index, &counting, &corpus.scheme, &[&by_path]);
    assert_eq!(
        (report.parsed, report.hashed),
        (0, 1),
        "update_paths of a touched file: {report:?}"
    );
    assert_eq!(counting.take_reads(), std::slice::from_ref(&by_path));

    // Then `update`: the other touched file alone is read, and the one
    // `update_paths` read is trusted by its new key.
    let report = update(&mut index, &counting, &corpus.scheme);
    assert_eq!(
        counting.take_reads(),
        std::slice::from_ref(&by_update),
        "update reads the touched file `update_paths` did not: {report:?}"
    );
    assert_eq!((report.parsed, report.hashed), (0, 1), "{report:?}");

    let report = update(&mut index, &counting, &corpus.scheme);
    assert_eq!(
        counting.take_reads(),
        Vec::<String>::new(),
        "the next update reads nothing: {report:?}"
    );
    assert_eq!(report.hashed, 0, "{report:?}");
}

// ---------------------------------------------------------------- AC-8 (e)

#[test]
fn ac8e_two_copies_of_one_tree_dump_equal_while_their_keys_differ() {
    let scratch = Scratch::new("stat-key-e");
    let first = tree(&scratch, "one", CONFIG, 6);
    std::thread::sleep(PAST_RACY);
    let second = tree(&scratch, "two", CONFIG, 6);
    let mut one = first.open(&scratch.db("one"));
    let mut two = second.open(&scratch.db("two"));
    first.update(&mut one);
    second.update(&mut two);

    let left = masked_dump(&one, &first.root);
    let right = masked_dump(&two, &second.root);
    assert!(
        left == right,
        "two copies of one tree dump equal, root masked\n{}",
        common::dump_diff(&left, &right)
    );
    for dump in [&left, &right] {
        assert!(
            !dump
                .lines()
                .any(|line| line.starts_with("file_stats\t") || line.starts_with("walk_notes\t")),
            "neither `file_stats` nor `walk_notes` is in the dump"
        );
    }
    let left_stats = file_stats(&scratch.db("one"));
    let right_stats = file_stats(&scratch.db("two"));
    assert_eq!(left_stats.len(), 6, "{left_stats:?}");
    assert_eq!(right_stats.len(), 6, "{right_stats:?}");
    for (left, right) in left_stats.iter().zip(&right_stats) {
        assert_eq!(left.0, right.0, "the same paths");
        assert_eq!(left.1, right.1, "{}: the same size", left.0);
        assert_ne!(
            (left.4, left.3),
            (right.4, right.3),
            "{}: the copies' inodes and ctimes differ",
            left.0
        );
    }
}

// ---------------------------------------------------------------- AC-8 (f)

/// (f1) and (f2) on `corpus`, warm and past the racy window.
fn assert_no_op_takes_no_lock(corpus: &Corpus, db: &Path, named: &str, label: &str) {
    let mut index = corpus.open(db);
    let walking = corpus.tree();
    update(&mut index, &walking, &corpus.scheme);
    let warm = update(&mut index, &walking, &corpus.scheme);
    assert!(!warm.committed, "{label}: premise, a warm update: {warm:?}");

    // (f1) A second connection holds the write lock.
    let holder = raw(db);
    holder
        .execute_batch("BEGIN IMMEDIATE")
        .expect("the second connection takes the write lock");
    let started = Instant::now();
    let report = index.update(&walking, &corpus.scheme);
    let took = started.elapsed();
    let report = report.unwrap_or_else(|error| {
        panic!("{label}: update under another writer's lock fails: {error}")
    });
    assert!(
        took < Duration::from_secs(1),
        "{label}: update waited {took:?} for the write lock"
    );
    assert!(!report.committed, "{label}: {report:?}");
    let started = Instant::now();
    let report = index.update_paths(&walking, &corpus.scheme, &[named]);
    let took = started.elapsed();
    let report = report.unwrap_or_else(|error| {
        panic!("{label}: update_paths under another writer's lock fails: {error}")
    });
    assert!(
        took < Duration::from_secs(1),
        "{label}: update_paths waited {took:?} for the write lock"
    );
    assert!(!report.committed, "{label}: {report:?}");
    holder.execute_batch("ROLLBACK").expect("release");

    // (f2) Without the lock, nothing is committed.
    let watcher = raw(db);
    let before = data_version(&watcher);
    let report = update(&mut index, &walking, &corpus.scheme);
    assert!(!report.committed, "{label}: {report:?}");
    let report = update_paths(&mut index, &walking, &corpus.scheme, &[named]);
    assert!(!report.committed, "{label}: {report:?}");
    assert_eq!(
        data_version(&watcher),
        before,
        "{label}: a no-op update commits nothing"
    );
}

#[test]
fn ac8f_a_warm_no_op_takes_no_write_lock() {
    let scratch = Scratch::new("stat-key-f");
    let plain = tree(&scratch, "plain", CONFIG, 5);
    let locked = tree(&scratch, "locked", CONFIG, 5);
    locked.write("docs/locked.md", record(9, "Body text 09 ka zo."));
    common::chmod(&locked.root, "docs/locked.md", 0o000);
    std::thread::sleep(PAST_RACY);

    assert_no_op_takes_no_lock(&plain, &scratch.db("plain"), &record_path(2), "plain");
    if mode_000_binds(&locked.root.join("docs/locked.md")) {
        assert_no_op_takes_no_lock(
            &locked,
            &scratch.db("locked"),
            &record_path(2),
            "a mode-000 file",
        );
        let index = locked.open(&scratch.db("locked"));
        let stored = index.file("docs/locked.md").expect("file");
        assert!(
            stored
                .as_ref()
                .is_some_and(|file| file.blake3.is_none() && file.read_error.is_some()),
            "premise: the mode-000 file is stored unreadable: {stored:?}"
        );
    } else {
        eprintln!("mode-000 files are readable here (root?): that run is skipped");
    }
    common::chmod(&locked.root, "docs/locked.md", 0o644);
}

// ---------------------------------------------------------------- AC-8 (g)

#[test]
fn ac8g_walk_notes_hold_the_listings_facts_and_trusted_notes_checks_them() {
    let scratch = Scratch::new("stat-key-g");
    let config = "[ids]\nR = { kind = \"requirement\", width = 2 }\n\n\
                  [paths]\nroots = [\"docs\", \"notes\"]\n";
    let corpus = tree(&scratch, "wt", config, 3);
    corpus.write("docs/private/hidden.md", record(8, "Body text 08 ka zo."));
    common::chmod(&corpus.root, "docs/private", 0o000);
    let private_binds = mode_000_binds(&corpus.root.join("docs/private"));
    if !private_binds {
        eprintln!("mode-000 directories are listable here (root?): that fact is skipped");
    }
    let bad_name = corpus
        .root
        .join("docs")
        .join(OsStr::from_bytes(b"bad-\xff.md"));
    let skipped = match fs::write(&bad_name, record(7, "Body text 07 ka zo.")) {
        Ok(()) => 1,
        Err(error) => {
            eprintln!("a non-UTF-8 name cannot be created here ({error}): that fact is skipped");
            0
        }
    };
    let expected = WalkNotes {
        missing_roots: vec!["notes".to_owned()],
        skipped_names: skipped,
        unreadable_dirs: if private_binds {
            vec!["docs/private".to_owned()]
        } else {
            Vec::new()
        },
    };

    let db = scratch.db("index");
    let mut index = corpus.open(&db);
    let report = corpus.update(&mut index);
    assert_eq!(WalkNotes::from(&report), expected, "premise: {report:?}");
    let stored = walk_notes(&db);
    let unreadable_json = if private_binds {
        "[\"docs/private\"]"
    } else {
        "[]"
    };
    assert_eq!(
        stored,
        [(
            "[\"notes\"]".to_owned(),
            unreadable_json.to_owned(),
            skipped as i64
        )],
        "walk_notes after update"
    );
    assert_eq!(
        index
            .trusted_notes(&corpus.scheme, &corpus.paths)
            .expect("trusted_notes"),
        Some(expected.clone()),
        "trusted_notes gives the stored facts"
    );

    // An `update_paths` that probed, over an edited file, leaves them.
    let path = record_path(2);
    corpus.write(&path, record(2, "Body text 02 vo xi je."));
    let report = update_paths(&mut index, &corpus.tree(), &corpus.scheme, &[&path]);
    assert_eq!(report.parsed, 1, "{report:?}");
    assert_eq!(walk_notes(&db), stored, "update_paths leaves the notes");
    assert_eq!(
        index
            .trusted_notes(&corpus.scheme, &corpus.paths)
            .expect("trusted_notes"),
        Some(expected.clone())
    );

    // Another `[ids]` fingerprint.
    let other_scheme = IdScheme::from_toml(
        "[ids]\nR = { kind = \"requirement\", width = 2 }\nQ = { kind = \"question\", width = 3 }\n",
    )
    .expect("scheme");
    assert_eq!(
        index
            .trusted_notes(&other_scheme, &corpus.paths)
            .expect("trusted_notes"),
        None,
        "another [ids] fingerprint"
    );
    // Another `[paths]` fingerprint.
    let other_paths = Paths::from_toml("[paths]\nroots = [\"docs\"]\n").expect("paths");
    assert_ne!(other_paths, corpus.paths, "premise");
    assert_eq!(
        index
            .trusted_notes(&corpus.scheme, &other_paths)
            .expect("trusted_notes"),
        None,
        "another [paths] fingerprint"
    );
    // No worktree row: a handle on another root of the same database.
    let other_root: PathBuf = scratch.join("other");
    common::write(&other_root, "specengine.toml", config);
    let other = common::open(&db, &fs::canonicalize(&other_root).expect("root"));
    assert_eq!(
        other
            .trusted_notes(&corpus.scheme, &corpus.paths)
            .expect("trusted_notes"),
        None,
        "no worktree row"
    );
    // An old stamp.
    let changed = raw(&db)
        .execute(
            "UPDATE index_meta SET value = ?1 WHERE key = 'format'",
            ["16"],
        )
        .expect("rewrite the stamp");
    assert_eq!(changed, 1, "one stamp row");
    assert_eq!(
        index
            .trusted_notes(&corpus.scheme, &corpus.paths)
            .expect("trusted_notes"),
        None,
        "an old stamp"
    );

    common::chmod(&corpus.root, "docs/private", 0o755);
}

/// `walk_notes` cannot go stale through `update_paths`: an update that
/// lists the first file under a missing root, or unlists a root's last
/// stored file, walks, so the notes record the root's new state; a file
/// added beside a stored one does not walk.
#[test]
fn ac8g_update_paths_walks_when_a_root_gains_its_first_file_or_loses_its_last() {
    let scratch = Scratch::new("stat-key-roots");
    let config = "[ids]\nR = { kind = \"requirement\", width = 2 }\n\n\
                  [paths]\nroots = [\"docs\", \"notes\"]\n";
    let corpus = tree(&scratch, "wt", config, 3);
    let missing = WalkNotes {
        missing_roots: vec!["notes".to_owned()],
        ..WalkNotes::default()
    };
    let db = scratch.db("index");
    let mut index = corpus.open(&db);
    corpus.update(&mut index);
    let notes = |index: &SqliteIndex| {
        index
            .trusted_notes(&corpus.scheme, &corpus.paths)
            .expect("trusted_notes")
    };
    assert_eq!(
        notes(&index),
        Some(missing.clone()),
        "premise: `notes` is missing"
    );

    let first = "notes/r-07.md";
    corpus.write(first, record(7, "Body text 07 ka zo."));
    let report = update_paths(&mut index, &corpus.tree(), &corpus.scheme, &[first]);
    assert_eq!(report.walked, 4, "the root's first file walks: {report:?}");
    assert_eq!(
        notes(&index),
        Some(WalkNotes::default()),
        "`notes` is no longer missing"
    );

    let second = "notes/r-08.md";
    corpus.write(second, record(8, "Body text 08 ka zo."));
    let report = update_paths(&mut index, &corpus.tree(), &corpus.scheme, &[second]);
    assert_eq!(
        report.walked, 1,
        "a file beside a stored one does not walk: {report:?}"
    );

    corpus.remove(first);
    corpus.remove(second);
    fs::remove_dir(corpus.root.join("notes")).expect("the emptied root goes");
    let report = update_paths(&mut index, &corpus.tree(), &corpus.scheme, &[first, second]);
    assert_eq!(report.removed, 2, "{report:?}");
    assert_eq!(
        report.walked, 3,
        "the root's last files gone walks: {report:?}"
    );
    assert_eq!(notes(&index), Some(missing), "`notes` is missing again");
}
