//! Where the index database lives (owner's answer Q1 of the task spec):
//! `~/Library/Application Support/specengine/<slug>.db` on macOS; elsewhere
//! `$XDG_DATA_HOME/specengine/` when that is absolute, else
//! `$HOME/.local/share/specengine/`. One database per project; each
//! worktree's rows are keyed by `(project, root)` by the store.
//!
//! `HOME` unset, empty or relative → exit 2. A data directory inside the
//! canonical project root (its nearest existing ancestor canonicalised: a
//! project at `$HOME` is refused, a known limitation) → exit 2 with nothing
//! created; else the directory is created and the store opens the database,
//! its own guard (the database outside the worktree) applying again.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use specengine_store::SqliteIndex;

use crate::project::ProjectRoot;
use crate::{CliError, Env, store_error};

/// The directory holding every project's index database.
pub fn data_dir(env: &Env) -> Result<PathBuf, CliError> {
    let home = match env.home.as_deref() {
        None => {
            return Err(CliError::spec(
                "HOME is not set: the index database lives under the home directory",
            ));
        }
        Some(home) if home.is_empty() => {
            return Err(CliError::spec(
                "HOME is empty: the index database lives under the home directory",
            ));
        }
        Some(home) => PathBuf::from(home),
    };
    if !home.is_absolute() {
        return Err(CliError::spec(format!(
            "HOME is not an absolute path ({}): the index database lives under it",
            home.display()
        )));
    }
    Ok(host_data_dir(&home, env.xdg_data_home.as_deref()))
}

#[cfg(target_os = "macos")]
fn host_data_dir(home: &Path, _xdg_data_home: Option<&OsStr>) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join("specengine")
}

#[cfg(not(target_os = "macos"))]
fn host_data_dir(home: &Path, xdg_data_home: Option<&OsStr>) -> PathBuf {
    match xdg_data_home.map(Path::new) {
        Some(xdg) if xdg.is_absolute() => xdg.join("specengine"),
        _ => home.join(".local").join("share").join("specengine"),
    }
}

/// The database of the project `slug`: `<data_dir>/<slug>.db`.
pub fn db_path(env: &Env, slug: &str) -> Result<PathBuf, CliError> {
    Ok(data_dir(env)?.join(format!("{slug}.db")))
}

/// An open index handle on the project's worktree.
pub struct OpenIndex {
    pub index: SqliteIndex,
    /// The database file, as built from the environment.
    pub db: PathBuf,
    pub slug: String,
}

/// Opens (creating the data directory and the database when absent) the
/// index of `project`: the slug is required, the data directory must lie
/// outside the root.
pub fn open_index(env: &Env, project: &ProjectRoot) -> Result<OpenIndex, CliError> {
    let slug = project.slug()?.to_owned();
    let dir = prepared_data_dir(env, project)?;
    let db = dir.join(format!("{slug}.db"));
    let index = SqliteIndex::open(&db, &slug, &project.root).map_err(store_error)?;
    Ok(OpenIndex { index, db, slug })
}

/// The data directory, created when absent, refused when it lies inside
/// the project root (nothing created then): where the database and the
/// proposal queue's scratch files go.
pub(crate) fn prepared_data_dir(env: &Env, project: &ProjectRoot) -> Result<PathBuf, CliError> {
    let dir = checked_data_dir(env, project)?;
    fs::create_dir_all(&dir).map_err(|error| {
        CliError::spec(format!(
            "cannot create the data directory {}: {error}",
            dir.display()
        ))
    })?;
    Ok(dir)
}

/// The data directory, refused as [`prepared_data_dir`] refuses it, but
/// never created: `spec export state` and `spec import-state` write nothing
/// before their own writes.
pub(crate) fn checked_data_dir(env: &Env, project: &ProjectRoot) -> Result<PathBuf, CliError> {
    let dir = data_dir(env)?;
    let resolved = resolve_nonexistent(&dir).map_err(|error| {
        CliError::spec(format!(
            "cannot resolve the data directory {}: {error}",
            dir.display()
        ))
    })?;
    if resolved.starts_with(&project.root) {
        return Err(CliError::spec(format!(
            "the data directory {} lies inside the project root; the index must live \
             outside the repository (a project at the home directory is not supported); \
             nothing was created",
            dir.display()
        )));
    }
    Ok(dir)
}

/// `path` with its nearest existing ancestor canonicalised and the missing
/// rest appended as written. Creates nothing.
pub(crate) fn resolve_nonexistent(path: &Path) -> io::Result<PathBuf> {
    let mut missing: Vec<&OsStr> = Vec::new();
    let mut existing = path;
    loop {
        match fs::canonicalize(existing) {
            Ok(canonical) => {
                let mut resolved = canonical;
                for name in missing.iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let (Some(Component::Normal(name)), Some(parent)) =
                    (existing.components().next_back(), existing.parent())
                else {
                    return Err(error);
                };
                missing.push(name);
                existing = parent;
            }
            Err(error) => return Err(error),
        }
    }
}
