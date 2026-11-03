//! The committed tree of an apply (task spec `apply-outputs`, "The
//! committed tree"): `HEAD`'s tree under the root at the apply's checked
//! commit, its `specengine.toml` included, with each path the apply writes
//! laid over it at the bytes it writes. That is the tree `git commit
//! --only` records and a staged hook judges; never the disk walk, never the
//! git index.
//!
//! Built as `spec check --changed` builds its base: one
//! [`Staged::head_only`] session (its environment and guard,
//! `GIT_NO_LAZY_FETCH=1` on every child, so a blob a partial clone lacks is
//! read as missing, never fetched), `ls-tree -r -z <commit>` in the root,
//! one `cat-file --batch` session for `HEAD`'s config, the outputs of its
//! `index = true` entry and every listed document, then walked by `HEAD`'s
//! `[paths]` into a [`GitIndex`] ([`GitIndex::from_walk`]). Nothing is
//! written.

use std::collections::BTreeMap;
use std::path::Path;

use specengine_core::ProjectConfig;
use specengine_core::check::CheckConfig;

use crate::check::CONFIG_FILE;
use crate::git::{Blob, Entry, EntryKind, GitEnv, GitFailure, Staged};
use crate::source::{GitIndex, IndexWalk};

/// Why a document `HEAD` lists cannot be read: its blob is not in the
/// object database (no fetch is tried).
const MISSING_HEAD_BLOB: &str = "HEAD's blob is missing from the git object database";

/// Why a document `HEAD` lists as a regular file cannot be read: its OID
/// names another object type.
const HEAD_NOT_A_BLOB: &str = "HEAD's object is not a blob";

/// What [`committed_tree`] read.
#[derive(Debug)]
pub enum CommittedTree {
    /// `HEAD`'s config cannot be used (absent, unreadable, invalid), or git
    /// failed before it was read: why, one line. `HEAD`'s index entry is
    /// unknown.
    Unknown(String),
    /// `HEAD`'s config holds no `[[generators]]` entry with `index = true`:
    /// nothing is rendered.
    NoIndex,
    /// `HEAD`'s config and its index entry, the tree to render from.
    Indexed(Box<Committed>),
}

/// `HEAD`'s config, its index entry's outputs as `HEAD` holds them, and the
/// committed tree with the documents whose blobs it could not read.
#[derive(Debug)]
pub struct Committed {
    /// `HEAD`'s top level, `[project]`, `[ids]` and `[paths]`.
    pub project: ProjectConfig,
    /// `HEAD`'s `[[generators]]` and the rest; its index entry is there.
    pub check: CheckConfig,
    /// The committed tree as a [`crate::Source`]; a document in
    /// [`Committed::unread`] reads as an error.
    pub tree: GitIndex,
    /// Each listed document whose blob is not in the object database, or
    /// is no blob, by path: `` `<path>` cannot be read (…) ``. Not empty:
    /// the tree is partial, nothing is rendered from it.
    pub unread: BTreeMap<String, String>,
    /// `HEAD`'s file at each root-relative path the index entry writes.
    outputs: BTreeMap<String, HeadFile>,
}

impl Committed {
    /// `HEAD`'s file at the root-relative output `path`: absent when the
    /// entry does not write it.
    pub fn at_head(&self, path: &str) -> &HeadFile {
        self.outputs.get(path).unwrap_or(&HeadFile::Absent)
    }
}

/// `HEAD`'s file at an output's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadFile {
    /// `HEAD`'s tree has nothing there.
    Absent,
    /// A regular file's blob.
    Bytes(Vec<u8>),
    /// Something that cannot be compared: why, one line (`` `<path>` … ``).
    Unreadable(String),
}

/// The committed tree of the root `root` at the commit `head` (40 or 64
/// hex), `overlay` (root-relative paths and the bytes the apply writes
/// there) laid over it; git's children run with `git`'s environment (an
/// apply passes its worktree's, the local `GIT_*` variables dropped). See
/// the module documentation.
pub fn committed_tree(
    root: &Path,
    git: &GitEnv,
    head: &str,
    overlay: &[(&str, &[u8])],
) -> CommittedTree {
    match read(root, git, head, overlay) {
        Ok(tree) => tree,
        Err(failure) => CommittedTree::Unknown(failure.cause().message),
    }
}

fn read(
    root: &Path,
    git: &GitEnv,
    head: &str,
    overlay: &[(&str, &[u8])],
) -> Result<CommittedTree, GitFailure> {
    let mut session = match Staged::head_only(root, git) {
        Ok(session) => session,
        Err(cause) => return Ok(CommittedTree::Unknown(cause.message)),
    };
    let mut entries = session.head_entries(head)?;
    let text = match config_text(&mut session, &entries)? {
        Ok(text) => text,
        Err(why) => {
            session.finish()?;
            return Ok(CommittedTree::Unknown(why));
        }
    };
    let (project, check) = match parse_config(&text) {
        Ok(parsed) => parsed,
        Err(why) => {
            session.finish()?;
            return Ok(CommittedTree::Unknown(why));
        }
    };
    let Some(generator) = check.index_generator() else {
        session.finish()?;
        return Ok(CommittedTree::NoIndex);
    };
    let mut outputs = BTreeMap::new();
    for path in &generator.writes {
        let file = head_file(&mut session, &entries, path)?;
        outputs.insert(path.clone(), file);
    }
    // The apply's paths at the bytes it writes, each under a key no object
    // ID can be: read from the session's objects, never asked of git.
    for (at, (path, bytes)) in overlay.iter().enumerate() {
        let key = format!("overlay-{at}");
        session
            .objects
            .insert(key.clone(), Blob::Bytes(bytes.to_vec()));
        let entry = Entry {
            path: path.as_bytes().to_vec(),
            kind: EntryKind::Regular,
            oid: key,
        };
        match entries.binary_search_by(|known| known.path.as_slice().cmp(path.as_bytes())) {
            Ok(found) => entries[found] = entry,
            Err(slot) => entries.insert(slot, entry),
        }
    }
    let walk = IndexWalk::new(&entries, &project.paths);
    walk.read(&mut session)?;
    session.finish()?;
    let unread = walk
        .listed
        .iter()
        .filter_map(|(path, oid)| {
            let why = match session.objects.get(oid) {
                Some(Blob::Bytes(_)) => return None,
                Some(Blob::Foreign) => HEAD_NOT_A_BLOB,
                Some(Blob::Missing) | None => MISSING_HEAD_BLOB,
            };
            Some((path.clone(), format!("`{path}` cannot be read ({why})")))
        })
        .collect();
    // The directories the walk reaches files through: `HEAD`'s entries and
    // the overlay's.
    session.entries = entries;
    let tree = GitIndex::from_walk(session, walk);
    Ok(CommittedTree::Indexed(Box::new(Committed {
        project,
        check,
        tree,
        unread,
        outputs,
    })))
}

/// `HEAD`'s `specengine.toml` under the root as text; `Err`: why not, one
/// line.
fn config_text(
    session: &mut Staged,
    entries: &[Entry],
) -> Result<Result<String, String>, GitFailure> {
    let Some(entry) = entry_at(entries, CONFIG_FILE) else {
        return Ok(Err(format!("HEAD has no {CONFIG_FILE}")));
    };
    let why = match entry.kind {
        EntryKind::Regular => match session.blob(&entry.oid)? {
            Blob::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) => return Ok(Ok(text.to_owned())),
                Err(_) => "not UTF-8",
            },
            Blob::Missing => "its blob is missing from the git object database",
            Blob::Foreign => "its object is not a blob",
        },
        EntryKind::Symlink => "a symbolic link",
        EntryKind::Gitlink => "a submodule",
        EntryKind::Other => "not a regular file",
    };
    Ok(Err(format!("HEAD's {CONFIG_FILE} cannot be read ({why})")))
}

/// `HEAD`'s config read as the project's and the check's tables; `Err`:
/// `HEAD's specengine.toml is not a valid config (line N): <message>`.
fn parse_config(text: &str) -> Result<(ProjectConfig, CheckConfig), String> {
    let invalid = |line: Option<usize>, message: String| {
        let at = line
            .map(|line| format!(" (line {line})"))
            .unwrap_or_default();
        format!("HEAD's {CONFIG_FILE} is not a valid config{at}: {message}")
    };
    let project =
        ProjectConfig::from_toml(text).map_err(|error| invalid(error.line, error.message))?;
    let check = CheckConfig::from_toml(text).map_err(|error| invalid(error.line, error.message))?;
    Ok((project, check))
}

/// `HEAD`'s file at the root-relative `path`, its blob read through the
/// session.
fn head_file(session: &mut Staged, entries: &[Entry], path: &str) -> Result<HeadFile, GitFailure> {
    let Some(entry) = entry_at(entries, path) else {
        return Ok(HeadFile::Absent);
    };
    if entry.kind != EntryKind::Regular {
        return Ok(HeadFile::Unreadable(format!(
            "`{path}` is not a regular file in HEAD's tree"
        )));
    }
    Ok(match session.blob(&entry.oid)? {
        Blob::Bytes(bytes) => HeadFile::Bytes(bytes.clone()),
        Blob::Missing => {
            HeadFile::Unreadable(format!("`{path}` cannot be read ({MISSING_HEAD_BLOB})"))
        }
        Blob::Foreign => {
            HeadFile::Unreadable(format!("`{path}` cannot be read ({HEAD_NOT_A_BLOB})"))
        }
    })
}

/// The entry at exactly `path` of the byte-sorted `entries`.
fn entry_at<'e>(entries: &'e [Entry], path: &str) -> Option<&'e Entry> {
    entries
        .binary_search_by(|entry| entry.path.as_slice().cmp(path.as_bytes()))
        .ok()
        .map(|at| &entries[at])
}
