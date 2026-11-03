//! The sparse set (task spec `hub-git`, "The sparse set"): the paths a hub's
//! sparse checkout must hold, from the repository's top, read from the
//! root's `specengine.toml` at `HEAD`, and those `HEAD` holds that the
//! worktree lacks. `spec sparse-set` prints them; a sync's look stops on a
//! path missing (`crate::sync`).
//!
//! The set, in config order: `specengine.toml`, `spec.lock` and
//! `.spec-debt.toml`; each `[paths] roots` entry; `[decision_records] dir`
//! and `template`; the `index = true` entry's `writes`. Each is
//! root-relative in the config and printed from the top. A directory
//! `HEAD` holds ends in `/`; a path inside a listed directory, and a repeat,
//! are dropped. Read with `GIT_NO_LAZY_FETCH=1` (every [`WorktreeGit`]
//! child): nothing is fetched. Nothing is written.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path};

use specengine_core::check::Report;
use specengine_core::lock::LOCK_FILE;

use crate::check::{BASELINE_FILE, CONFIG_FILE, NamedBytes, load_config};
use crate::worktree::{GitError, WorktreeGit, one_line, success};

/// What [`sparse_set`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SparseSet {
    /// From the top, in config order (see the module documentation).
    pub paths: Vec<String>,
    /// The paths of [`SparseSet::paths`] `HEAD` holds that the worktree
    /// lacks, in the same order.
    pub missing: Vec<String>,
}

/// Why no sparse set could be read.
#[derive(Debug)]
pub enum SparseError {
    /// No git repository holds the root.
    NoRepository,
    /// `HEAD` has no commit yet.
    NoCommit,
    /// `HEAD`'s config cannot be used: its causes, the config named
    /// `label` (`HEAD:<path from the top>`).
    Config { label: String, report: Box<Report> },
    /// Git failed, or the root does not lie in the worktree git names.
    Git(GitError),
}

impl From<GitError> for SparseError {
    fn from(error: GitError) -> Self {
        Self::Git(error)
    }
}

/// The sparse set of the canonical project root `root`, read through `git`
/// (a handle in `root`); see the module documentation.
pub fn sparse_set(root: &Path, git: &WorktreeGit) -> Result<SparseSet, SparseError> {
    let Some(top) = git.top_if_repository()? else {
        return Err(SparseError::NoRepository);
    };
    let prefix = root_prefix(root, &top)?;
    let Some(head) = git.head()? else {
        return Err(SparseError::NoCommit);
    };
    let label = format!("HEAD:{prefix}{CONFIG_FILE}");
    let config_path = format!("{prefix}{CONFIG_FILE}");
    let bytes = if git.has_path(&head, &config_path)? {
        git.local_blob_at(&head, &config_path)
            .map_err(|error| io::Error::other(error.to_string()))
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "HEAD's tree has no such file",
        ))
    };
    let named = NamedBytes {
        name: label.clone(),
        bytes,
    };
    let (project, check) = load_config(&named).map_err(|report| SparseError::Config {
        label: label.clone(),
        report,
    })?;
    let mut wanted: Vec<String> = vec![
        CONFIG_FILE.to_owned(),
        LOCK_FILE.to_owned(),
        BASELINE_FILE.to_owned(),
    ];
    wanted.extend(project.paths.roots.iter().cloned());
    if let Some(records) = &project.decision_records {
        wanted.push(records.dir.clone());
        wanted.push(records.template.clone());
    }
    if let Some(generator) = check.index_generator() {
        wanted.extend(generator.writes.iter().cloned());
    }
    let from_top: Vec<String> = wanted
        .iter()
        .map(|path| {
            let path = path.trim_end_matches('/');
            format!("{prefix}{path}")
        })
        .collect();
    let kinds = kinds_at(git, &head, &from_top)?;
    let listed: Vec<(String, Kind)> = from_top
        .into_iter()
        .zip(kinds)
        .map(|(path, kind)| match kind {
            Kind::Tree => (format!("{path}/"), kind),
            _ => (path, kind),
        })
        .collect();
    let mut set = SparseSet::default();
    for (path, kind) in &listed {
        let inside = listed.iter().any(|(other, other_kind)| {
            *other_kind == Kind::Tree && other != path && path.starts_with(other.as_str())
        });
        if inside || set.paths.contains(path) {
            continue;
        }
        set.paths.push(path.clone());
        if *kind != Kind::Absent && lacks(&top, path, *kind) {
            set.missing.push(path.clone());
        }
    }
    Ok(set)
}

/// What `HEAD` holds at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Tree,
    Blob,
    /// Nothing, or what is no tree and no blob (a submodule's commit).
    Absent,
}

/// `root` from `top`, `/`-separated with a trailing `/`; empty at the top.
fn root_prefix(root: &Path, top: &Path) -> Result<String, SparseError> {
    let unreadable = || {
        SparseError::Git(GitError::Unreadable {
            command: "rev-parse",
        })
    };
    let relative = root.strip_prefix(top).map_err(|_| unreadable())?;
    let mut prefix = String::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(unreadable());
        };
        prefix.push_str(name.to_str().ok_or_else(unreadable)?);
        prefix.push('/');
    }
    Ok(prefix)
}

/// What `head` holds at each top-relative path, in order, from one `cat-file
/// --batch-check=%(objecttype)` reading `<head>:<path>` per line. A path
/// holding a line break is never asked: absent.
fn kinds_at(git: &WorktreeGit, head: &str, paths: &[String]) -> Result<Vec<Kind>, GitError> {
    const COMMAND: &str = "cat-file";
    let asked: Vec<&String> = paths
        .iter()
        .filter(|path| !path.contains(['\n', '\r']) && !path.is_empty())
        .collect();
    let mut input = Vec::new();
    for path in &asked {
        input.extend_from_slice(format!("{head}:{path}\n").as_bytes());
    }
    let answers: Vec<Kind> = if asked.is_empty() {
        Vec::new()
    } else {
        let args = [
            OsStr::new("cat-file"),
            OsStr::new("--batch-check=%(objecttype)"),
        ];
        let output = success(COMMAND, git.output_with_input(&args, true, &input)?)?;
        let text = String::from_utf8_lossy(one_line(&output).unwrap_or_default()).into_owned();
        let kinds: Vec<Kind> = text
            .split('\n')
            .map(|line| match line {
                "tree" => Kind::Tree,
                "blob" => Kind::Blob,
                _ => Kind::Absent,
            })
            .collect();
        if kinds.len() != asked.len() {
            return Err(GitError::Unreadable { command: COMMAND });
        }
        kinds
    };
    let mut answers = answers.into_iter();
    Ok(paths
        .iter()
        .map(|path| {
            if path.contains(['\n', '\r']) || path.is_empty() {
                Kind::Absent
            } else {
                answers.next().unwrap_or(Kind::Absent)
            }
        })
        .collect())
}

/// The worktree under `top` lacks `path` (from the top, a directory ending
/// in `/`), which `HEAD` holds as `kind`: nothing there, or no directory
/// where `HEAD` holds a tree.
fn lacks(top: &Path, path: &str, kind: Kind) -> bool {
    let on_disk = top.join(path.trim_end_matches('/'));
    match fs::symlink_metadata(&on_disk) {
        Ok(meta) => kind == Kind::Tree && !meta.is_dir(),
        Err(_) => true,
    }
}
