//! Where the bytes come from: listing and reading kept apart from indexing,
//! so `spec check --staged` (07 §2) can feed staged blobs through the same
//! writer. [`WorkingTree`] reads the working tree by the `[paths]` rules.
//!
//! **Walk** (`crates/specengine-store/README.md`, "Walk"): regular files
//! whose name ends exactly in `.md` under the roots, minus `exclude`; symlinks and
//! names starting with `.` are skipped below a root (the census rule, plus
//! dot-files); no `.gitignore`; names that are not UTF-8 are skipped and
//! counted; a root that names no directory and no `.md` file (missing, a
//! symlink on the way, another kind of file) is reported. Paths are
//! root-relative with `/`, built from the names as the OS lists them (a
//! root's own components included), and byte-sorted.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, FileType};
use std::io;
use std::path::{Path, PathBuf};

use specengine_core::{DOCUMENT_EXTENSION, Paths, WalkScope, is_clean_relative, is_under};

use crate::error::StoreError;

/// What a walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// Root-relative `/` paths, byte-sorted, unique.
    pub paths: Vec<String>,
    /// Configured roots that name no directory and no `.md` file, as written.
    pub missing_roots: Vec<String>,
    /// Directory and `.md` names skipped because they are not UTF-8.
    pub skipped_names: usize,
    /// Directories below a root that could not be listed, root-relative;
    /// their files are not in `paths`.
    pub unreadable_dirs: Vec<String>,
}

/// A worktree's spec files: listing, probing and reading. Implementations
/// must agree with themselves: a path is [`Source::probe`]d `true` exactly
/// when [`Source::list`] would list it at that moment.
pub trait Source {
    /// The worktree root this source reads; compared with the index handle's
    /// (canonical) root.
    fn root(&self) -> &Path;
    /// Every indexable file. An error only when the worktree root itself
    /// cannot be listed.
    fn list(&self) -> io::Result<Listing>;
    /// Whether `path` (root-relative, `/`) is listed by the walk rules now.
    fn probe(&self, path: &str) -> bool;
    /// The bytes of a listed file.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
    /// Whether `path` (root-relative, `/`) is now a directory the walk could
    /// reach files through; `update_paths` then walks everything. The default
    /// answers "not a directory", for a source without directories.
    fn is_dir(&self, path: &str) -> bool {
        let _ = path;
        false
    }
}

/// The working tree under `root`, walked by `[paths]`.
#[derive(Debug, Clone)]
pub struct WorkingTree {
    root: PathBuf,
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
            scope: paths.walk_scope(),
        })
    }

    fn excluded(&self, path: &str) -> bool {
        self.scope.is_excluded(path)
    }

    /// The kind of a root-relative path (a configured root, or a directory
    /// for [`Source::is_dir`]), looked up by exact name component by
    /// component: `None` when a component is missing, a symlink, or a
    /// non-directory before the end.
    fn root_kind(&self, root: &str) -> Option<FileType> {
        let mut dir = self.root.clone();
        let mut components = root.split('/').peekable();
        while let Some(component) = components.next() {
            let kind = entry_kind(&dir, OsStr::new(component))?;
            if kind.is_symlink() {
                return None;
            }
            if components.peek().is_some() {
                if !kind.is_dir() {
                    return None;
                }
                dir.push(component);
            } else {
                return Some(kind);
            }
        }
        None
    }

    fn walk(
        &self,
        absolute: &Path,
        relative: &str,
        found: &mut BTreeSet<String>,
        listing: &mut Listing,
    ) {
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

    fn list(&self) -> io::Result<Listing> {
        fs::read_dir(&self.root)?;
        let mut listing = Listing::default();
        let mut found = BTreeSet::new();
        for root in self.scope.roots() {
            match self.root_kind(root) {
                Some(kind) if kind.is_dir() => {
                    self.walk(&self.root.join(root), root, &mut found, &mut listing);
                }
                Some(kind) if kind.is_file() && root.ends_with(DOCUMENT_EXTENSION) => {
                    if !self.excluded(root) {
                        found.insert(root.clone());
                    }
                }
                _ => listing.missing_roots.push(root.clone()),
            }
        }
        listing.paths = found.into_iter().collect();
        listing.unreadable_dirs.sort();
        listing.unreadable_dirs.dedup();
        Ok(listing)
    }

    fn probe(&self, path: &str) -> bool {
        // A clean `.md` path some root admits (it lies in, or is, the root;
        // the components below the root follow the walk's dot-name rule, the
        // root's own components do not), matching no exclude glob.
        if !self.scope.in_walk_scope(path) {
            return false;
        }
        let mut dir = self.root.clone();
        let components: Vec<&str> = path.split('/').collect();
        for (index, component) in components.iter().enumerate() {
            let Some(kind) = entry_kind(&dir, OsStr::new(component)) else {
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

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
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

    fn is_dir(&self, path: &str) -> bool {
        if !is_clean_relative(path) {
            return false;
        }
        // A root, a directory under one (no dot-name below the root, as the
        // walk skips them), or a directory containing one.
        let related = self.scope.roots().iter().any(|root| {
            path == root.as_str()
                || is_under(root, path)
                || (is_under(path, root)
                    && path
                        .split('/')
                        .skip(root.split('/').count())
                        .all(|component| !component.starts_with('.')))
        });
        related && self.root_kind(path).is_some_and(|kind| kind.is_dir())
    }
}

/// The type of the entry of `dir` named exactly `name` (no normalisation,
/// no symlink followed), as its listing shows it; `None` when the listing
/// fails or has no such name.
fn entry_kind(dir: &Path, name: &OsStr) -> Option<FileType> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| entry.file_name() == name)
        .and_then(|entry| entry.file_type().ok())
}
