//! The git reads of `spec verify` (`docs/features/spec-verify.md` "Base",
//! "Cascade and spec side"): the check's base reader
//! ([`Staged::head_only`]: its environment, its guard, its one `cat-file
//! --batch` session) extended to code, manifests and `spec.lock`, for any
//! commit. Read-only: git runs in the root under the caller's [`GitEnv`]
//! (never [`crate::WorktreeGit`]), with `GIT_OPTIONAL_LOCKS=0`, and only
//! the commands of `docs/canon/architecture-code.md#sync-in-reads`:
//!
//! - `rev-parse --is-inside-work-tree` when opened;
//! - in the session, requests by name: a path's unmerged stages
//!   (`:1:`, `:2:`, `:3:./<path>`, a stage only an unmerged path has);
//! - `hash-object --no-filters --stdin-paths`: files on disk by their
//!   blob OID, raw bytes, no clean filter, never `-w`;
//! - `rev-parse --verify -q <rev>^{commit}`, then `ls-tree -r -z <oid>`:
//!   a commit's tree under the root, root-relative;
//! - in the session, the blobs of the paths asked for, each OID once.
//!
//! Every failure is a [`Cause`] at `.` with a fixed message, never git's
//! text or an absolute path.

use std::collections::BTreeMap;
use std::path::Path;

use specengine_core::WalkScope;
use specengine_core::check::Cause;

use crate::git::{Blob, Entry, EntryKind, GitEnv, Staged};
use crate::source::walk_index;

/// The cause of a blob `ls-tree` lists that the object database lacks.
pub const MISSING_COMMIT_BLOB: &str = "the blob is missing from the git object database";

/// The cause of a regular entry whose OID names another object type.
pub const COMMIT_NOT_A_BLOB: &str = "the object is not a blob";

/// The git reads of one `spec verify` call, in the root.
#[derive(Debug)]
pub struct VerifyGit {
    staged: Staged,
}

/// What a commit's tree holds at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitEntryKind {
    /// `100644`, `100755`.
    Regular,
    /// `120000`.
    Symlink,
    /// `160000`.
    Submodule,
    /// Any other mode.
    Other,
}

/// One entry of a commit's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEntry {
    pub kind: CommitEntryKind,
    /// Lower-case hex, 40 or 64 characters.
    pub oid: String,
}

/// A commit's tree under the root, listed (`ls-tree -r -z`), byte-sorted.
#[derive(Debug, Clone)]
pub struct CommitListing {
    commit: String,
    entries: Vec<Entry>,
}

/// One file of a commit read: its bytes, or why they cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitFile {
    Bytes(Vec<u8>),
    /// [`MISSING_COMMIT_BLOB`], [`COMMIT_NOT_A_BLOB`], or the entry is no
    /// regular file.
    Unreadable(&'static str),
}

impl CommitListing {
    /// The commit's full OID.
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// The entry at exactly the root-relative `path`.
    pub fn entry(&self, path: &str) -> Option<CommitEntry> {
        self.entries
            .binary_search_by(|entry| entry.path.as_slice().cmp(path.as_bytes()))
            .ok()
            .map(|at| commit_entry(&self.entries[at]))
    }

    /// Every entry whose path is UTF-8, in byte order (a path that is not
    /// is skipped, as the walks skip such names).
    pub fn entries(&self) -> impl Iterator<Item = (&str, CommitEntry)> + '_ {
        self.entries.iter().filter_map(|entry| {
            std::str::from_utf8(&entry.path)
                .ok()
                .map(|path| (path, commit_entry(entry)))
        })
    }

    /// The documents the spec walk lists at this commit under `scope`
    /// (the `[paths]` of the checked tree), as `spec check --changed`'s
    /// base lists them.
    pub fn documents(&self, scope: &WalkScope) -> Vec<String> {
        let (_, listed) = walk_index(&self.entries, scope);
        listed.into_keys().collect()
    }
}

fn commit_entry(entry: &Entry) -> CommitEntry {
    CommitEntry {
        kind: match entry.kind {
            EntryKind::Regular => CommitEntryKind::Regular,
            EntryKind::Symlink => CommitEntryKind::Symlink,
            EntryKind::Gitlink => CommitEntryKind::Submodule,
            EntryKind::Other => CommitEntryKind::Other,
        },
        oid: entry.oid.clone(),
    }
}

impl VerifyGit {
    /// Git for the root (canonicalised): the caller's environment resolved
    /// and guarded as `spec check --changed`'s, the root inside a git
    /// working tree. The cause: the root unreadable, the environment
    /// refused, no repository, `git` not runnable or failing.
    pub fn open(root: &Path, env: &GitEnv) -> Result<Self, Cause> {
        Ok(Self {
            staged: Staged::head_only(root, env)?,
        })
    }

    /// Whether the root-relative `path` is unmerged in the git index: a
    /// stage 1, 2 or 3 entry, asked by name in the session.
    pub fn is_unmerged(&mut self, path: &str) -> Result<bool, Cause> {
        for stage in 1..=3 {
            let name = format!(":{stage}:./{path}");
            if self
                .staged
                .named(&name)
                .map_err(|failure| failure.cause())?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The blob OIDs of the root-relative files `paths` as they are on
    /// disk, raw bytes (no clean filter), in the order given; nothing is
    /// written. One `hash-object` for all.
    pub fn hash_files(&self, paths: &[&str]) -> Result<Vec<String>, Cause> {
        self.staged
            .hash_files(paths)
            .map_err(|failure| failure.cause())
    }

    /// The tree under the root of the commit `rev` names (`HEAD`; any
    /// commit-ish), listed; `None` when it names none (an unborn `HEAD`).
    pub fn list_commit(&mut self, rev: &str) -> Result<Option<CommitListing>, Cause> {
        let Some(commit) = self
            .staged
            .commit_named(rev)
            .map_err(|failure| failure.cause())?
        else {
            return Ok(None);
        };
        let entries = self
            .staged
            .head_entries(&commit)
            .map_err(|failure| failure.cause())?;
        Ok(Some(CommitListing { commit, entries }))
    }

    /// The files at `paths` (root-relative) of `listing`, each blob read
    /// once through the session and moved out of it at its last use (no
    /// second copy); a path the listing lacks is left out, an entry that is
    /// no regular file or whose object cannot be read is
    /// [`CommitFile::Unreadable`].
    pub fn read_files<'p>(
        &mut self,
        listing: &CommitListing,
        paths: impl IntoIterator<Item = &'p str>,
    ) -> Result<BTreeMap<String, CommitFile>, Cause> {
        let mut files = BTreeMap::new();
        let mut regular: Vec<(&'p str, &str)> = Vec::new();
        for path in paths {
            let Some(at) = listing
                .entries
                .binary_search_by(|entry| entry.path.as_slice().cmp(path.as_bytes()))
                .ok()
            else {
                continue;
            };
            let entry = &listing.entries[at];
            if entry.kind == EntryKind::Regular {
                regular.push((path, entry.oid.as_str()));
            } else {
                files.insert(
                    path.to_owned(),
                    CommitFile::Unreadable("not a regular file"),
                );
            }
        }
        // Every object read first; then each moved out at its OID's last
        // use, copied before.
        let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, oid) in &regular {
            self.staged.blob(oid).map_err(|failure| failure.cause())?;
            *uses.entry(oid).or_default() += 1;
        }
        for (path, oid) in regular {
            let left = uses.get_mut(oid).map_or(0, |left| {
                *left -= 1;
                *left
            });
            let blob = if left == 0 {
                self.staged.objects.remove(oid)
            } else {
                self.staged.objects.get(oid).cloned()
            };
            let file = match blob {
                Some(Blob::Bytes(bytes)) => CommitFile::Bytes(bytes),
                Some(Blob::Foreign) => CommitFile::Unreadable(COMMIT_NOT_A_BLOB),
                Some(Blob::Missing) | None => CommitFile::Unreadable(MISSING_COMMIT_BLOB),
            };
            files.insert(path.to_owned(), file);
        }
        Ok(files)
    }

    /// Ends the session: it must end cleanly.
    pub fn finish(mut self) -> Result<(), Cause> {
        self.staged.finish().map_err(|failure| failure.cause())
    }
}
