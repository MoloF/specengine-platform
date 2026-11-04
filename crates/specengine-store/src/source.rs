//! Where the bytes come from: listing and reading kept apart from indexing
//! and checking. [`WorkingTree`] reads the working tree by the `[paths]`
//! rules; [`GitIndex`] the git index, for `spec check --staged` (07 §2).
//!
//! **Walk** (`crates/specengine-store/README.md`, "Walk"): regular files
//! whose name ends exactly in `.md` under the roots, minus `exclude`; symlinks and
//! names starting with `.` are skipped below a root (the census rule, plus
//! dot-files); no `.gitignore`; names that are not UTF-8 are skipped and
//! counted; a root that names no directory and no `.md` file (missing, a
//! symlink on the way, another kind of file) is reported, and a directory
//! on the way to a root that cannot be listed is an unreadable directory,
//! never a missing root (a default root's absence is ignored; an unread
//! one's is not known). Paths are
//! root-relative with `/`, built from the names as the OS lists them (a
//! root's own components included), and byte-sorted.
//!
//! [`Source::probe_paths`] probes a batch of named paths: [`WorkingTree`]
//! reads each directory's listing at most once per call
//! (`docs/features/edge-table.md` "Data"). [`Source::stat`] gives a file's
//! [`StatKey`], which an update compares before it reads (the `read-trust`
//! spec, "The key and the racy rule").
//!
//! [`GitIndex`] walks the git index by the same rules
//! (docs/canon/spec-check-git.md, "The staged check"): the stage-0 regular
//! entries (`100644`, `100755`) as the files, symlinks (`120000`) and
//! gitlinks (`160000`) skipped, a directory existing when an entry lies
//! under it; a root is a directory only when a regular entry lies under it.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, DirEntry, FileType};
use std::io;
use std::path::{Path, PathBuf};

use specengine_core::check::Cause;
use specengine_core::{DOCUMENT_EXTENSION, Paths, WalkScope, is_clean_relative, is_under};

use crate::error::StoreError;
use crate::git::{Blob, Entry, EntryKind, GitEnv, GitFailure, Staged};

/// What a walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// Root-relative `/` paths, byte-sorted, unique.
    pub paths: Vec<String>,
    /// Configured roots that name no directory and no `.md` file, as written.
    pub missing_roots: Vec<String>,
    /// Directory and `.md` names skipped because they are not UTF-8.
    pub skipped_names: usize,
    /// Directories below a root, or on the way to one (`""`: the root
    /// itself), that could not be listed, root-relative; their files are
    /// not in `paths`.
    pub unreadable_dirs: Vec<String>,
}

/// A file's stat key (the `read-trust` spec, "The key and the racy
/// rule"): size, mtime and ctime in nanoseconds from the Unix epoch
/// (negative before it), inode and device, from one `lstat`. Inode and
/// device are compared for equality only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatKey {
    pub size: u64,
    pub mtime_ns: i64,
    pub ctime_ns: i64,
    pub ino: u64,
    pub dev: u64,
}

/// What [`Source::probe_paths`] made of one named path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Probed {
    /// Not a directory, and listed by the walk rules now ([`Source::probe`]).
    Listed,
    /// Neither a directory nor listed.
    Unlisted,
    /// A directory the walk could reach files through ([`Source::is_dir`]).
    Dir,
}

/// A worktree's spec files: listing, probing and reading. Implementations
/// must agree with themselves: a path is [`Source::probe`]d `true` exactly
/// when [`Source::list`] would list it at that moment.
pub trait Source {
    /// The worktree root this source reads; compared with the index handle's
    /// (canonical) root.
    fn root(&self) -> &Path;
    /// The `[paths]` it walks by; the index resolves links by the same
    /// (`features`, `link_base`) and fingerprints them.
    fn paths(&self) -> &Paths;
    /// Every indexable file. An error only when the worktree root itself
    /// cannot be listed.
    fn list(&self) -> io::Result<Listing>;
    /// Whether `path` (root-relative, `/`) is listed by the walk rules now.
    fn probe(&self, path: &str) -> bool;
    /// The bytes of a listed file.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
    /// The stat key of a listed file, taken before it is read; `None` when
    /// the source has none (the default) or cannot tell, and the file is
    /// then always read.
    fn stat(&self, path: &str) -> Option<StatKey> {
        let _ = path;
        None
    }
    /// Whether `path` (root-relative, `/`) is now a directory the walk could
    /// reach files through; `update_paths` then walks everything. The default
    /// answers "not a directory", for a source without directories.
    fn is_dir(&self, path: &str) -> bool {
        let _ = path;
        false
    }
    /// Each named path, in order: [`Probed::Dir`] by [`Source::is_dir`],
    /// else [`Probed::Listed`] by [`Source::probe`], else
    /// [`Probed::Unlisted`]. The default goes path by path; a source may
    /// share work across the batch, with the same answers.
    fn probe_paths(&self, paths: &[&str]) -> Vec<Probed> {
        paths
            .iter()
            .map(|path| {
                if self.is_dir(path) {
                    Probed::Dir
                } else if self.probe(path) {
                    Probed::Listed
                } else {
                    Probed::Unlisted
                }
            })
            .collect()
    }
}

/// The working tree under `root`, walked by `[paths]`.
#[derive(Debug, Clone)]
pub struct WorkingTree {
    root: PathBuf,
    /// The `[paths]` it was made with.
    paths: Paths,
    /// The roots and the exclude globs, compiled once: core's
    /// [`WalkScope`], the matcher `spec check` judges link targets by.
    scope: WalkScope,
}

impl WorkingTree {
    /// `root` is canonicalised (it must exist); `paths` gives the roots and
    /// the exclude globs.
    pub fn new(root: impl AsRef<Path>, paths: &Paths) -> Result<Self, StoreError> {
        let requested = root.as_ref();
        let root = fs::canonicalize(requested).map_err(|error| StoreError::io(requested, error))?;
        Ok(Self {
            root,
            paths: paths.clone(),
            scope: paths.walk_scope(),
        })
    }

    fn excluded(&self, path: &str) -> bool {
        self.scope.is_excluded(path)
    }

    /// The kind of a root-relative path (a configured root, or a directory
    /// for [`Source::is_dir`]), looked up by exact name component by
    /// component: `None` when a component is missing, a symlink, or a
    /// non-directory before the end, or a directory on the way cannot be
    /// read.
    fn root_kind(&self, root: &str, listings: &mut Listings) -> Option<FileType> {
        self.resolve(root, listings).ok().flatten()
    }

    /// [`WorkingTree::root_kind`], telling "not there" (`Ok(None)`) from
    /// "cannot tell": `Err` names the directory on the way (root-relative,
    /// `""` for the root itself) that could not be listed, or whose entry's
    /// type could not be read.
    fn resolve(&self, root: &str, listings: &mut Listings) -> Result<Option<FileType>, String> {
        let mut dir = self.root.clone();
        let mut read = Vec::new();
        let mut components = root.split('/').peekable();
        while let Some(component) = components.next() {
            let Some(kind) = listings
                .entry_kind(&dir, OsStr::new(component))
                .map_err(|_| read.join("/"))?
            else {
                return Ok(None);
            };
            if kind.is_symlink() {
                return Ok(None);
            }
            if components.peek().is_some() {
                if !kind.is_dir() {
                    return Ok(None);
                }
                dir.push(component);
                read.push(component);
            } else {
                return Ok(Some(kind));
            }
        }
        Ok(None)
    }

    fn walk(
        &self,
        absolute: &Path,
        relative: &str,
        found: &mut BTreeSet<String>,
        listing: &mut Listing,
    ) {
        note_listing(absolute);
        let Ok(entries) = fs::read_dir(absolute) else {
            listing.unreadable_dirs.push(relative.to_owned());
            return;
        };
        let mut unreadable = false;
        for entry in entries {
            let Ok(entry) = entry else {
                unreadable = true;
                continue;
            };
            let name = entry.file_name();
            let bytes = name.as_encoded_bytes();
            if bytes.starts_with(b".") {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                unreadable = true;
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let Some(text) = name.to_str() else {
                if kind.is_dir()
                    || (kind.is_file() && bytes.ends_with(DOCUMENT_EXTENSION.as_bytes()))
                {
                    listing.skipped_names += 1;
                }
                continue;
            };
            let child = format!("{relative}/{text}");
            if kind.is_dir() {
                self.walk(&absolute.join(&name), &child, found, listing);
            } else if kind.is_file() && text.ends_with(DOCUMENT_EXTENSION) && !self.excluded(&child)
            {
                found.insert(child);
            }
        }
        if unreadable {
            listing.unreadable_dirs.push(relative.to_owned());
        }
    }
}

impl Source for WorkingTree {
    fn root(&self) -> &Path {
        &self.root
    }

    fn paths(&self) -> &Paths {
        &self.paths
    }

    fn list(&self) -> io::Result<Listing> {
        note_listing(&self.root);
        fs::read_dir(&self.root)?;
        let mut listing = Listing::default();
        let mut found = BTreeSet::new();
        let listings = &mut Listings::direct();
        for root in self.scope.roots() {
            match self.resolve(root, listings) {
                Ok(Some(kind)) if kind.is_dir() => {
                    self.walk(&self.root.join(root), root, &mut found, &mut listing);
                }
                Ok(Some(kind)) if kind.is_file() && root.ends_with(DOCUMENT_EXTENSION) => {
                    if !self.excluded(root) {
                        found.insert(root.clone());
                    }
                }
                Ok(_) => listing.missing_roots.push(root.clone()),
                // Could not tell whether the root is there: never "missing"
                // (ignored for a default root), always unverified.
                Err(dir) => listing.unreadable_dirs.push(dir),
            }
        }
        listing.paths = found.into_iter().collect();
        listing.unreadable_dirs.sort();
        listing.unreadable_dirs.dedup();
        Ok(listing)
    }

    fn probe(&self, path: &str) -> bool {
        self.probe_in(path, &mut Listings::direct())
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.read_file(path)
    }

    /// `lstat` of the path: `None` for an unclean path, an error, a symlink
    /// or anything but a regular file.
    fn stat(&self, path: &str) -> Option<StatKey> {
        if !is_clean_relative(path) {
            return None;
        }
        stat_key(&self.root.join(path))
    }

    fn is_dir(&self, path: &str) -> bool {
        self.is_dir_in(path, &mut Listings::direct())
    }

    /// One listing memo for the whole batch: each directory is read at most
    /// once per call.
    fn probe_paths(&self, paths: &[&str]) -> Vec<Probed> {
        let listings = &mut Listings::memo();
        paths
            .iter()
            .map(|path| {
                if self.is_dir_in(path, listings) {
                    Probed::Dir
                } else if self.probe_in(path, listings) {
                    Probed::Listed
                } else {
                    Probed::Unlisted
                }
            })
            .collect()
    }
}

impl WorkingTree {
    /// [`Source::probe`] through `listings`.
    fn probe_in(&self, path: &str, listings: &mut Listings) -> bool {
        // A clean `.md` path some root admits (it lies in, or is, the root;
        // the components below the root follow the walk's dot-name rule, the
        // root's own components do not), matching no exclude glob.
        if !self.scope.in_walk_scope(path) {
            return false;
        }
        let mut dir = self.root.clone();
        let components: Vec<&str> = path.split('/').collect();
        for (index, component) in components.iter().enumerate() {
            let Ok(Some(kind)) = listings.entry_kind(&dir, OsStr::new(component)) else {
                return false;
            };
            if kind.is_symlink() {
                return false;
            }
            if index + 1 < components.len() {
                if !kind.is_dir() {
                    return false;
                }
                dir.push(component);
            } else {
                return kind.is_file();
            }
        }
        false
    }

    /// [`Source::read`].
    fn read_file(&self, path: &str) -> io::Result<Vec<u8>> {
        if !is_clean_relative(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a root-relative path",
            ));
        }
        // No component below the root may be a symlink, as for `probe`:
        // every directory on the way, and the file itself.
        let mut absolute = self.root.clone();
        let mut components = path.split('/').peekable();
        while let Some(component) = components.next() {
            absolute.push(component);
            let kind = fs::symlink_metadata(&absolute)?.file_type();
            let last = components.peek().is_none();
            if kind.is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "a symlink on the path",
                ));
            }
            if last && !kind.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not a regular file",
                ));
            }
            if !last && !kind.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not a directory on the path",
                ));
            }
        }
        fs::read(&absolute)
    }

    /// [`Source::is_dir`] through `listings`.
    fn is_dir_in(&self, path: &str, listings: &mut Listings) -> bool {
        if !is_clean_relative(path) {
            return false;
        }
        // A root, a directory under one (no dot-name below the root, as the
        // walk skips them), or a directory containing one.
        related_to_a_root(&self.scope, path)
            && self
                .root_kind(path, listings)
                .is_some_and(|kind| kind.is_dir())
    }
}

/// How [`WorkingTree`] reads a directory's listing for one lookup: afresh
/// every time, or once per directory for one [`Source::probe_paths`] call.
struct Listings {
    /// Directory → its entries by name (`None`: it could not be listed);
    /// `None`: no memo.
    memo: Option<BTreeMap<PathBuf, Option<Listed>>>,
}

/// One directory's listing.
struct Listed {
    /// The first entry of each name.
    entries: BTreeMap<OsString, DirEntry>,
    /// Some entry could not be read.
    unread: bool,
}

impl Listings {
    fn direct() -> Self {
        Self { memo: None }
    }

    fn memo() -> Self {
        Self {
            memo: Some(BTreeMap::new()),
        }
    }

    /// [`entry_kind`], the listing read once per directory with a memo.
    fn entry_kind(&mut self, dir: &Path, name: &OsStr) -> io::Result<Option<FileType>> {
        let Some(memo) = &mut self.memo else {
            return entry_kind(dir, name);
        };
        let listed = memo
            .entry(dir.to_path_buf())
            .or_insert_with(|| list_dir(dir));
        let Some(listed) = listed else {
            return Err(io::Error::other("the directory cannot be listed"));
        };
        match listed.entries.get(name) {
            Some(entry) => entry.file_type().map(Some),
            None if listed.unread => Err(io::Error::other("an entry could not be read")),
            None => Ok(None),
        }
    }
}

/// The entries of `dir` by name; `None` when it cannot be listed.
fn list_dir(dir: &Path) -> Option<Listed> {
    note_listing(dir);
    let mut listed = Listed {
        entries: BTreeMap::new(),
        unread: false,
    };
    for entry in fs::read_dir(dir).ok()? {
        match entry {
            Ok(entry) => {
                listed.entries.entry(entry.file_name()).or_insert(entry);
            }
            Err(_) => listed.unread = true,
        }
    }
    Some(listed)
}

/// `path` is a root, lies under one, or contains one with no dot-name
/// below it: a directory [`Source::is_dir`] may answer for.
fn related_to_a_root(scope: &WalkScope, path: &str) -> bool {
    scope.roots().iter().any(|root| {
        path == root.as_str()
            || is_under(root, path)
            || (is_under(path, root)
                && path
                    .split('/')
                    .skip(root.split('/').count())
                    .all(|component| !component.starts_with('.')))
    })
}

/// The files the git index stages under the root, walked by `[paths]` with
/// [`WorkingTree`]'s rules: stage-0 regular entries (`100644`, `100755`)
/// under a root; below a root the dot-name test before the UTF-8 test,
/// symlinks and gitlinks skipped, a non-UTF-8 directory counted once per
/// root, a non-UTF-8 `.md` name once, `exclude` on files only; a root that
/// is no regular `.md` entry and holds no regular entry is missing. The
/// bytes are read when it is built (one `cat-file --batch` session) and
/// kept: [`Source::read`] never touches the working tree.
///
/// For checks only: an [`crate::IndexWriter`] would store staged bytes as
/// the working tree's rows.
#[derive(Debug, Clone)]
pub struct GitIndex {
    /// Canonical.
    root: PathBuf,
    /// The `[paths]` it was walked by.
    paths: Paths,
    scope: WalkScope,
    listing: Listing,
    /// Every listed path and its blob's OID.
    listed: BTreeMap<String, String>,
    /// The listed OIDs' objects.
    objects: BTreeMap<String, Blob>,
    /// Every stage-0 regular entry's path, byte-sorted: the directories.
    regular: Vec<Vec<u8>>,
}

impl GitIndex {
    /// The index of the repository `root` lies in (git runs in `root`,
    /// with `git`'s environment), walked by `paths`, every listed blob
    /// read. `Err`: the causes of a check that cannot start (each at `.`,
    /// or one per unmerged path under the root). A blob missing from the
    /// object database is no error here: reading its path fails.
    pub fn open(root: impl AsRef<Path>, paths: &Paths, git: &GitEnv) -> Result<Self, Vec<Cause>> {
        let staged = Staged::read(root.as_ref(), git)?;
        Self::from_staged(staged, paths).map_err(|failure| vec![failure.cause()])
    }

    /// Walks `staged` by `paths`, reads every listed blob through its
    /// session, each OID once, in path order, and ends the session (a
    /// session left by a failed read is killed when `staged` drops).
    pub(crate) fn from_staged(mut staged: Staged, paths: &Paths) -> Result<Self, GitFailure> {
        let walk = IndexWalk::new(&staged.entries, paths);
        walk.read(&mut staged)?;
        staged.finish()?;
        Ok(Self::from_walk(staged, walk))
    }

    /// The index of `walk`, its blobs read through `staged` (every object
    /// `staged` read is kept: [`GitIndex::object`] gives the base's too).
    pub(crate) fn from_walk(staged: Staged, walk: IndexWalk) -> Self {
        let regular = staged
            .entries
            .into_iter()
            .filter(|entry| entry.kind == EntryKind::Regular)
            .map(|entry| entry.path)
            .collect();
        Self {
            root: staged.root,
            paths: walk.paths,
            scope: walk.scope,
            listing: walk.listing,
            listed: walk.listed,
            objects: staged.objects,
            regular,
        }
    }

    /// The OID of the listed path `path`.
    pub(crate) fn oid_of(&self, path: &str) -> Option<&str> {
        self.listed.get(path).map(String::as_str)
    }

    /// An object read through the session, by OID.
    pub(crate) fn object(&self, oid: &str) -> Option<&Blob> {
        self.objects.get(oid)
    }

    /// An object read through the session, by OID, moved out: a later
    /// [`Source::read`] of a path holding it fails as a missing blob, so
    /// only the base takes, after the checked run.
    pub(crate) fn take_object(&mut self, oid: &str) -> Option<Blob> {
        self.objects.remove(oid)
    }
}

/// The index's listing by `[paths]`, before any blob is read.
#[derive(Debug, Clone)]
pub(crate) struct IndexWalk {
    pub(crate) paths: Paths,
    pub(crate) scope: WalkScope,
    pub(crate) listing: Listing,
    /// Every listed path and its blob's OID.
    pub(crate) listed: BTreeMap<String, String>,
}

impl IndexWalk {
    /// `entries` (byte-sorted) walked by `paths`.
    pub(crate) fn new(entries: &[Entry], paths: &Paths) -> Self {
        let scope = paths.walk_scope();
        let (listing, listed) = walk_index(entries, &scope);
        Self {
            paths: paths.clone(),
            scope,
            listing,
            listed,
        }
    }

    /// Reads every listed blob through `staged`'s session, each OID once,
    /// in path order; the session stays open.
    pub(crate) fn read(&self, staged: &mut Staged) -> Result<(), GitFailure> {
        for oid in self.listed.values() {
            staged.blob(oid)?;
        }
        Ok(())
    }
}

impl Source for GitIndex {
    fn root(&self) -> &Path {
        &self.root
    }

    fn paths(&self) -> &Paths {
        &self.paths
    }

    fn list(&self) -> io::Result<Listing> {
        Ok(self.listing.clone())
    }

    fn probe(&self, path: &str) -> bool {
        self.listed.contains_key(path)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let Some(oid) = self.listed.get(path) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "not a staged document",
            ));
        };
        self.objects.get(oid).unwrap_or(&Blob::Missing).bytes()
    }

    fn is_dir(&self, path: &str) -> bool {
        is_clean_relative(path)
            && related_to_a_root(&self.scope, path)
            && holds_an_entry(&self.regular, path.as_bytes())
    }
}

/// Some path of the byte-sorted `paths` lies strictly under `dir`.
fn holds_an_entry(paths: &[Vec<u8>], dir: &[u8]) -> bool {
    let mut prefix = dir.to_vec();
    prefix.push(b'/');
    let at = paths.partition_point(|path| path.as_slice() < prefix.as_slice());
    paths.get(at).is_some_and(|path| path.starts_with(&prefix))
}

/// What the walk makes of one entry below a root.
enum Walked {
    /// A document to list.
    Listed,
    /// Below a non-UTF-8 directory: counted once, by its path below the
    /// root (this many bytes of the rest).
    UnderSkippedDir(usize),
    /// A non-UTF-8 `.md` name: counted.
    SkippedName,
    /// Anything else: a dot-name on the way, a symlink, a gitlink, another
    /// name.
    Ignored,
}

/// The walk's verdict on `rest`, an entry's path below a root, WorkingTree's
/// order per component: the dot-name test, then the kind (only the last
/// component can be a symlink or a gitlink), then the UTF-8 test.
fn walk_entry(rest: &[u8], kind: EntryKind) -> Walked {
    let components: Vec<&[u8]> = rest.split(|&byte| byte == b'/').collect();
    let mut offset = 0;
    for (index, component) in components.iter().enumerate() {
        let last = index + 1 == components.len();
        if component.starts_with(b".") {
            return Walked::Ignored;
        }
        if last && kind != EntryKind::Regular {
            return Walked::Ignored;
        }
        if std::str::from_utf8(component).is_err() {
            if !last {
                return Walked::UnderSkippedDir(offset + component.len());
            }
            return if component.ends_with(DOCUMENT_EXTENSION.as_bytes()) {
                Walked::SkippedName
            } else {
                Walked::Ignored
            };
        }
        offset += component.len() + 1;
    }
    if rest.ends_with(DOCUMENT_EXTENSION.as_bytes()) {
        Walked::Listed
    } else {
        Walked::Ignored
    }
}

/// The listing of `entries` (byte-sorted) by `scope`, and each listed
/// path's OID: the index's, and `HEAD`'s tree for the base.
pub(crate) fn walk_index(
    entries: &[Entry],
    scope: &WalkScope,
) -> (Listing, BTreeMap<String, String>) {
    let mut listing = Listing::default();
    let mut listed = BTreeMap::new();
    for root in scope.roots() {
        let exact = entries
            .binary_search_by(|entry| entry.path.as_slice().cmp(root.as_bytes()))
            .ok()
            .map(|at| &entries[at])
            .filter(|entry| entry.kind == EntryKind::Regular);
        if let Some(entry) = exact {
            if !root.ends_with(DOCUMENT_EXTENSION) {
                listing.missing_roots.push(root.clone());
            } else if !scope.is_excluded(root) {
                listed.insert(root.clone(), entry.oid.clone());
            }
            continue;
        }
        let prefix = format!("{root}/");
        let start = entries.partition_point(|entry| entry.path.as_slice() < prefix.as_bytes());
        let under: Vec<&Entry> = entries[start..]
            .iter()
            .take_while(|entry| entry.path.starts_with(prefix.as_bytes()))
            .collect();
        if !under.iter().any(|entry| entry.kind == EntryKind::Regular) {
            listing.missing_roots.push(root.clone());
            continue;
        }
        let mut skipped_dirs = BTreeSet::new();
        for entry in under {
            let rest = &entry.path[prefix.len()..];
            match walk_entry(rest, entry.kind) {
                Walked::Listed => {
                    // Every component passed the UTF-8 test.
                    if let Ok(path) = std::str::from_utf8(&entry.path)
                        && !scope.is_excluded(path)
                    {
                        listed.insert(path.to_owned(), entry.oid.clone());
                    }
                }
                Walked::UnderSkippedDir(len) => {
                    skipped_dirs.insert(&rest[..len]);
                }
                Walked::SkippedName => listing.skipped_names += 1,
                Walked::Ignored => {}
            }
        }
        listing.skipped_names += skipped_dirs.len();
    }
    listing.paths = listed.keys().cloned().collect();
    (listing, listed)
}

/// The stat key of the regular file at `path`, by `lstat`; `None` for an
/// error, a symlink, any other kind of file, or a time whose nanoseconds
/// overflow an `i64`.
#[cfg(unix)]
fn stat_key(path: &Path) -> Option<StatKey> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let ns = |seconds: i64, nanos: i64| {
        seconds
            .checked_mul(1_000_000_000)
            .and_then(|whole| whole.checked_add(nanos))
    };
    Some(StatKey {
        size: meta.size(),
        mtime_ns: ns(meta.mtime(), meta.mtime_nsec())?,
        ctime_ns: ns(meta.ctime(), meta.ctime_nsec())?,
        ino: meta.ino(),
        dev: meta.dev(),
    })
}

/// No stat key off Unix: every file is read.
#[cfg(not(unix))]
fn stat_key(_: &Path) -> Option<StatKey> {
    None
}

/// The type of the entry of `dir` named exactly `name` (no normalisation,
/// no symlink followed), as its listing shows it; `Ok(None)` when the
/// listing has no such name. `Err` when it cannot tell: the listing fails,
/// the entry's type cannot be read, or an entry could not be read and the
/// name was not among the others.
fn entry_kind(dir: &Path, name: &OsStr) -> io::Result<Option<FileType>> {
    note_listing(dir);
    let mut unread = None;
    for entry in fs::read_dir(dir)? {
        match entry {
            Ok(entry) if entry.file_name() == name => return entry.file_type().map(Some),
            Ok(_) => {}
            Err(error) => unread = Some(error),
        }
    }
    match unread {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

/// Counts a listing of `dir` for the batched-probe test
/// (`docs/features/edge-table.md` "Acceptance criteria" ET-4): per
/// thread, every directory a [`WorkingTree`] reads.
#[cfg(test)]
fn note_listing(dir: &Path) {
    LISTINGS.with_borrow_mut(|counts| *counts.entry(dir.to_path_buf()).or_default() += 1);
}

#[cfg(not(test))]
fn note_listing(_: &Path) {}

#[cfg(test)]
thread_local! {
    /// Directory → how often this thread listed it.
    static LISTINGS: std::cell::RefCell<BTreeMap<PathBuf, usize>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
}

/// The listings this thread made since the last call, by directory
/// (absolute), and a fresh count.
#[cfg(test)]
#[allow(dead_code)] // The ET-4 test reads it.
pub(crate) fn take_listings() -> BTreeMap<PathBuf, usize> {
    LISTINGS.with_borrow_mut(std::mem::take)
}

#[cfg(test)]
mod tests {
    //! docs/features/edge-table.md "Acceptance criteria" ET-4: batched
    //! probes. The listing counter (`take_listings`) shows that an
    //! `update_paths` naming 2 000 files in one directory of 2 000, plus 50
    //! in a nested one, lists each directory at most twice, and the dump
    //! equals a walk's. M: `WorkingTree` without the `probe_paths` override
    //! (path-by-path `entry_kind`: 2 000 listings of the large directory).

    use std::fs;
    use std::path::PathBuf;

    use specengine_core::{IdSchemeToml, Paths};
    use specengine_model::IdScheme;

    use super::{Probed, Source, WorkingTree, take_listings};
    use crate::{IndexWriter, SqliteIndex};

    const CONFIG: &str =
        "[paths]\nroots = [\"docs\"]\n\n[ids]\nZQ = { kind = \"probe\", width = 2 }\n";
    const LARGE: usize = 2_000;
    const NESTED: usize = 50;

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-probes-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("wt/docs/large/nested")).expect("scratch");
            fs::create_dir_all(path.join("data")).expect("scratch");
            Self(fs::canonicalize(path).expect("canonical"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The 2 050 named files, root-relative.
    fn named() -> Vec<String> {
        (0..LARGE)
            .map(|i| format!("docs/large/f{i:04}.md"))
            .chain((0..NESTED).map(|i| format!("docs/large/nested/n{i:02}.md")))
            .collect()
    }

    #[test]
    fn update_paths_lists_each_directory_at_most_twice() {
        let scratch = Scratch::new("batch");
        let root = scratch.0.join("wt");
        fs::write(root.join("specengine.toml"), CONFIG).expect("config");
        let paths_named = named();
        for (i, path) in paths_named.iter().enumerate() {
            fs::write(
                root.join(path),
                format!("# F{i}\n\nSee ZQ-{:02}.\n", i % 90),
            )
            .expect("file");
        }
        let scheme = IdScheme::from_toml(CONFIG).expect("scheme");
        let paths = Paths::from_toml(CONFIG).expect("paths");
        let tree = WorkingTree::new(&root, &paths).expect("working tree");
        let mut index =
            SqliteIndex::open(scratch.0.join("data/index.db"), "demo", &root).expect("open");
        index.update(&tree, &scheme).expect("the first index");

        // Every named file changes, so the update reads each of them.
        for path in &paths_named {
            let mut bytes = fs::read(root.join(path)).expect("read");
            bytes.extend_from_slice(b"\nMore.\n");
            fs::write(root.join(path), bytes).expect("write");
        }
        let names: Vec<&str> = paths_named.iter().map(String::as_str).collect();
        let _ = take_listings();
        let report = index
            .update_paths(&tree, &scheme, &names)
            .expect("update_paths");
        let listings = take_listings();
        assert_eq!(report.walked, LARGE + NESTED, "{report:?}");
        assert_eq!(report.parsed, LARGE + NESTED, "{report:?}");
        let large = root.join("docs/large");
        eprintln!(
            "listings in one update_paths: {:?}",
            listings
                .iter()
                .map(|(dir, count)| (dir.strip_prefix(&root).unwrap_or(dir), count))
                .collect::<Vec<_>>()
        );
        assert!(
            listings.contains_key(&large),
            "the counter sees the large directory: {listings:?}"
        );
        for (dir, count) in &listings {
            assert!(
                *count <= 2,
                "{} listed {count} times in one update_paths",
                dir.display()
            );
        }

        // The batch answers as path by path.
        let probed = tree.probe_paths(&names);
        assert!(
            probed.iter().all(|probe| *probe == Probed::Listed),
            "{probed:?}"
        );

        // The dump equals a walk's.
        let mut walked =
            SqliteIndex::open(scratch.0.join("data/walk.db"), "demo", &root).expect("open");
        walked.update(&tree, &scheme).expect("a walk");
        assert!(
            index.dump_worktree().expect("dump") == walked.dump_worktree().expect("dump"),
            "update_paths of 2 050 files differs from a walk"
        );
    }
}
