//! The git reads of `spec verify` (`docs/canon/spec-verify.md` "Base",
//! "Cascade"): the check's base reader
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
//! - in the session, the blobs of the paths asked for, each OID once per
//!   `read_files` call.
//!
//! `spec verify --staged` ([`VerifyGit::staged`]) opens the check's index
//! reader instead ([`Staged::read`]: `ls-files -s -z`, the intent-to-add
//! detector, an unmerged path refused), the index git names
//! (`GIT_INDEX_FILE` kept), and lists its stage-0 entries under the root
//! as a [`CommitListing`] labelled [`INDEX_LABEL`]; then the same session
//! reads `HEAD` and the blobs.
//!
//! `spec lock rebase` (task spec `lock-rebase`) reads objects by
//! OID alone ([`VerifyGit::read_blobs`]): in the session, each OID once,
//! raw bytes, never a path, a tree, the index or the working tree.
//!
//! Every failure is a [`Cause`] at `.` with a fixed message, never git's
//! text or an absolute path.

use std::collections::BTreeMap;
use std::path::Path;

use specengine_core::WalkScope;
use specengine_core::check::Cause;

use crate::git::{Blob, Entry, EntryKind, GitEnv, IndexFailure, Staged};
use crate::source::walk_index;

/// The label of the git index's listing ([`VerifyGit::staged`]), where a
/// commit's listing has its OID.
pub const INDEX_LABEL: &str = "the index";

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

/// A commit's tree under the root, listed (`ls-tree -r -z`), byte-sorted;
/// or the git index's stage-0 entries under the root
/// ([`VerifyGit::staged`]).
#[derive(Debug, Clone)]
pub struct CommitListing {
    /// The commit's full OID, or [`INDEX_LABEL`].
    label: String,
    entries: Vec<Entry>,
}

/// Why [`VerifyGit::staged`] lists no index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexRefusal {
    /// The root unreadable, the environment refused, no repository, `git`
    /// not runnable or failing: one cause at `.`.
    Git(Cause),
    /// The root-relative paths with unmerged stages, byte-sorted (lossy
    /// UTF-8).
    Unmerged(Vec<String>),
}

/// One file of a commit read: its bytes, or why they cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitFile {
    Bytes(Vec<u8>),
    /// [`MISSING_COMMIT_BLOB`], [`COMMIT_NOT_A_BLOB`], or the entry is no
    /// regular file.
    Unreadable(&'static str),
}

/// An object read by its OID alone ([`VerifyGit::read_blobs`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectBlob {
    /// A blob: its raw bytes, no filter applied.
    Bytes(Vec<u8>),
    /// No object of that OID in the database (gc, a partial or shallow
    /// clone: no fetch is tried), or a text that is no OID.
    Missing,
    /// An object of another type (a tree, a commit, a tag).
    Foreign,
}

impl CommitListing {
    /// The commit's full OID; the index's listing: [`INDEX_LABEL`].
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The listing is the git index's ([`VerifyGit::staged`]).
    pub fn is_index(&self) -> bool {
        self.label == INDEX_LABEL
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

/// A full object name: 40 (SHA-1) or 64 (SHA-256) lowercase hex.
fn is_object_name(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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

    /// Git for the root over the git index git names (`GIT_INDEX_FILE`
    /// kept: a hook's `commit -a`, `--only` index too), listed as `spec
    /// check --staged` lists it ([`Staged::read`]: `ls-files -s -z`,
    /// intent-to-add entries dropped), and its stage-0 entries under the
    /// root as a listing labelled [`INDEX_LABEL`]; the session then reads
    /// `HEAD` ([`VerifyGit::list_commit`]) and any blob. Nothing is
    /// written; no `hash-object` runs unless asked.
    pub fn staged(root: &Path, env: &GitEnv) -> Result<(Self, CommitListing), IndexRefusal> {
        let mut staged = Staged::read_index(root, env).map_err(|failure| match failure {
            IndexFailure::Cause(cause) => IndexRefusal::Git(cause),
            IndexFailure::Unmerged(paths) => IndexRefusal::Unmerged(
                paths
                    .iter()
                    .map(|path| String::from_utf8_lossy(path).into_owned())
                    .collect(),
            ),
        })?;
        let listing = CommitListing {
            label: INDEX_LABEL.to_owned(),
            entries: std::mem::take(&mut staged.entries),
        };
        Ok((Self { staged }, listing))
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
        Ok(Some(CommitListing {
            label: commit,
            entries,
        }))
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

    /// The objects `oids` name, by OID alone, each asked of the session
    /// once however often it is given (`GIT_NO_LAZY_FETCH`: a missing one
    /// is never fetched) and moved out of it (no second copy). A text that
    /// is not 40 or 64 lowercase hex names no object: [`ObjectBlob::Missing`],
    /// never asked (the session would read it as a revision). The cause:
    /// git cannot be run or its reply cannot be read.
    pub fn read_blobs<'o>(
        &mut self,
        oids: impl IntoIterator<Item = &'o str>,
    ) -> Result<BTreeMap<String, ObjectBlob>, Cause> {
        let mut read = BTreeMap::new();
        for oid in oids {
            if read.contains_key(oid) {
                continue;
            }
            if !is_object_name(oid) {
                read.insert(oid.to_owned(), ObjectBlob::Missing);
                continue;
            }
            self.staged.blob(oid).map_err(|failure| failure.cause())?;
            let object = match self.staged.objects.remove(oid) {
                Some(Blob::Bytes(bytes)) => ObjectBlob::Bytes(bytes),
                Some(Blob::Foreign) => ObjectBlob::Foreign,
                Some(Blob::Missing) | None => ObjectBlob::Missing,
            };
            read.insert(oid.to_owned(), object);
        }
        Ok(read)
    }

    /// Ends the session: it must end cleanly.
    pub fn finish(mut self) -> Result<(), Cause> {
        self.staged.finish().map_err(|failure| failure.cause())
    }
}
