//! What can go wrong with the index. A broken spec file is never an error
//! here (ADR-0012): it is a row with its diagnostics or its `read_error`.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// An error of the index handle; no `rusqlite` type crosses this boundary.
#[derive(Debug)]
pub enum StoreError {
    /// The DB file (or, for a new one, its directory) resolves inside the
    /// worktree it would index (ADR-0003); nothing was created.
    DbInsideWorktree { db: PathBuf, worktree: PathBuf },
    /// The DB's directory does not exist; nothing was created.
    DbDirMissing { dir: PathBuf },
    /// The handle's worktree has no rows of the current index format: never
    /// indexed, or the tables were recreated for another format since.
    NotIndexed,
    /// The source walks another directory than the handle's worktree.
    RootMismatch { expected: PathBuf, found: PathBuf },
    /// Another connection held the database longer than `busy_timeout`.
    Busy,
    /// A file-system call on `path` failed (worktree root, DB location,
    /// listing the worktree).
    Io { path: PathBuf, source: io::Error },
    /// SQLite reported an error, or a stored value did not decode.
    Sqlite(String),
}

impl StoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DbInsideWorktree { db, worktree } => write!(
                f,
                "{}: the index database lies inside the worktree {} it indexes; \
                 put it outside the repository (nothing was created)",
                db.display(),
                worktree.display()
            ),
            Self::DbDirMissing { dir } => write!(
                f,
                "{}: the directory of the index database does not exist (nothing was created)",
                dir.display()
            ),
            Self::NotIndexed => f.write_str(
                "the worktree is not indexed in the current index format; run an update",
            ),
            Self::RootMismatch { expected, found } => write!(
                f,
                "the source walks {} but the index handle is bound to {}",
                found.display(),
                expected.display()
            ),
            Self::Busy => f.write_str(
                "the index database stayed locked by another connection past the busy timeout",
            ),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Sqlite(message) => write!(f, "index database: {message}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// `rusqlite` results into [`StoreError`], crate-private so no `rusqlite`
/// type reaches the public surface (not even through a `From` impl).
pub(crate) trait Db<T> {
    fn db(self) -> Result<T, StoreError>;
}

impl<T> Db<T> for rusqlite::Result<T> {
    fn db(self) -> Result<T, StoreError> {
        self.map_err(|error| match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                StoreError::Busy
            }
            _ => StoreError::Sqlite(error.to_string()),
        })
    }
}
