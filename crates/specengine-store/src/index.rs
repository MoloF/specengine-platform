//! The SQLite handle: where the DB may live, how it is opened, and the two
//! traits over it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use specengine_model::IdScheme;

use crate::error::{Db, StoreError};
use crate::source::Source;
use crate::write::Mode;
use crate::{
    IdHit, IndexWriter, IndexedFile, SearchQuery, SearchResults, SpecIndex, UpdateReport, dump,
    read, schema, write,
};

/// A connection bound to one worktree `(project, canonical root)` of an
/// index DB. One handle per thread; any number of handles may share a DB.
pub struct SqliteIndex {
    pub(crate) conn: Connection,
    pub(crate) project: String,
    pub(crate) root: PathBuf,
    /// `root` as stored in `worktrees.root`.
    pub(crate) root_text: String,
}

/// The connection's settings as SQLite reports them (05 §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbSettings {
    /// `wal`.
    pub journal_mode: String,
    /// 2: incremental.
    pub auto_vacuum: i64,
    /// Milliseconds: 5000.
    pub busy_timeout: i64,
    /// 1.
    pub foreign_keys: i64,
    /// 1: normal.
    pub synchronous: i64,
    /// Bytes: 67108864.
    pub journal_size_limit: i64,
    /// 0.
    pub trusted_schema: i64,
    /// 0.
    pub recursive_triggers: i64,
    /// `ENABLE_FTS5` is compiled in.
    pub fts5: bool,
    /// The SQLite library version.
    pub sqlite_version: String,
}

impl SqliteIndex {
    /// Opens (creating when absent) the index DB at `db` and binds the handle
    /// to the worktree `(project, root)`; `root` must exist and is
    /// canonicalised.
    ///
    /// Before anything is created: [`StoreError::DbInsideWorktree`] when the
    /// DB's directory (`..` and symlinks resolved) or an existing DB file's
    /// canonical path lies inside the worktree (ADR-0003);
    /// [`StoreError::DbDirMissing`] when the directory does not exist. A new
    /// DB gets `auto_vacuum=INCREMENTAL` and WAL before its first table;
    /// every connection gets `busy_timeout=5000`, `foreign_keys=ON`,
    /// `synchronous=NORMAL`, `journal_size_limit=67108864`,
    /// `trusted_schema=OFF`, `recursive_triggers=OFF`.
    pub fn open(
        db: impl AsRef<Path>,
        project: &str,
        root: impl AsRef<Path>,
    ) -> Result<Self, StoreError> {
        let requested = root.as_ref();
        let root = fs::canonicalize(requested).map_err(|error| StoreError::io(requested, error))?;
        if !root.is_dir() {
            return Err(StoreError::io(
                &root,
                io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "the worktree root is not a directory",
                ),
            ));
        }
        let Some(root_text) = root.to_str().map(str::to_owned) else {
            return Err(StoreError::io(
                &root,
                io::Error::new(io::ErrorKind::InvalidData, "the worktree root is not UTF-8"),
            ));
        };
        let target = locate(db.as_ref(), &root)?;
        let mut conn = Connection::open_with_flags(
            &target,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .db()?;
        schema::configure(&conn)?;
        schema::ensure(&mut conn)?;
        Ok(Self {
            conn,
            project: project.to_owned(),
            root,
            root_text,
        })
    }

    /// The canonical worktree root the handle is bound to.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The project the handle is bound to.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// The connection's PRAGMAs and the SQLite build, read back.
    pub fn settings(&self) -> Result<DbSettings, StoreError> {
        let text = |name: &str| {
            self.conn
                .pragma_query_value(None, name, |row| row.get::<_, String>(0))
                .db()
        };
        let number = |name: &str| {
            self.conn
                .pragma_query_value(None, name, |row| row.get::<_, i64>(0))
                .db()
        };
        Ok(DbSettings {
            journal_mode: text("journal_mode")?,
            auto_vacuum: number("auto_vacuum")?,
            busy_timeout: number("busy_timeout")?,
            foreign_keys: number("foreign_keys")?,
            synchronous: number("synchronous")?,
            journal_size_limit: number("journal_size_limit")?,
            trusted_schema: number("trusted_schema")?,
            recursive_triggers: number("recursive_triggers")?,
            fts5: self
                .conn
                .query_row(
                    "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .db()?
                == 1,
            sqlite_version: self
                .conn
                .query_row("SELECT sqlite_version()", [], |row| row.get(0))
                .db()?,
        })
    }

    /// FTS5 `integrity-check` with rank 1: the full-text index agrees with
    /// the `nodes` rows it is built from. An `Err` names the disagreement.
    pub fn check_fts(&mut self) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .db()?;
        tx.execute(
            "INSERT INTO nodes_fts (nodes_fts, rank) VALUES ('integrity-check', 1)",
            [],
        )
        .db()?;
        tx.commit().db()
    }

    /// The canonical dump of the whole DB: every index table in column order,
    /// surrogate keys replaced by `(project, root)`, `path`, `(path, ord)`,
    /// rows sorted, plus the FTS5 vocabulary (`fts5vocab` `row`: term,
    /// documents, occurrences). One line per row, `<table>\t<JSON array>`.
    /// "Equals a rebuild" means equal dumps.
    pub fn dump(&self) -> Result<String, StoreError> {
        dump::whole(self)
    }

    /// The canonical dump of this handle's worktree only: its `worktrees`
    /// row and the rows it owns, `index_meta`, and the FTS5 entries of its
    /// nodes (`fts5vocab` `instance`: term, path, ord, column, offset), which
    /// other worktrees cannot change.
    pub fn dump_worktree(&self) -> Result<String, StoreError> {
        dump::worktree(self)
    }

    /// The source walks this handle's worktree.
    pub(crate) fn check_root(&self, source: &dyn Source) -> Result<(), StoreError> {
        let found = fs::canonicalize(source.root()).unwrap_or_else(|_| source.root().to_path_buf());
        if found == self.root {
            Ok(())
        } else {
            Err(StoreError::RootMismatch {
                expected: self.root.clone(),
                found,
            })
        }
    }
}

impl SpecIndex for SqliteIndex {
    fn files(&self) -> Result<Vec<String>, StoreError> {
        read::files(self)
    }

    fn file(&self, path: &str) -> Result<Option<IndexedFile>, StoreError> {
        read::file(self, path)
    }

    fn lookup_id(&self, id: &str) -> Result<Vec<IdHit>, StoreError> {
        read::lookup_id(self, id)
    }

    fn search(&self, query: &SearchQuery) -> Result<SearchResults, StoreError> {
        read::search(self, query)
    }
}

impl IndexWriter for SqliteIndex {
    fn update(
        &mut self,
        source: &dyn Source,
        scheme: &IdScheme,
    ) -> Result<UpdateReport, StoreError> {
        write::run(self, source, scheme, Mode::Walk)
    }

    fn update_paths(
        &mut self,
        source: &dyn Source,
        scheme: &IdScheme,
        paths: &[&str],
    ) -> Result<UpdateReport, StoreError> {
        write::run(self, source, scheme, Mode::Paths(paths))
    }

    fn rebuild(
        &mut self,
        source: &dyn Source,
        scheme: &IdScheme,
    ) -> Result<UpdateReport, StoreError> {
        write::run(self, source, scheme, Mode::Rebuild)
    }
}

/// The path to open for `db`, or why not: the location guard of ADR-0003.
/// Nothing is created here.
fn locate(db: &Path, worktree: &Path) -> Result<PathBuf, StoreError> {
    let absolute = if db.is_absolute() {
        db.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| StoreError::io(db, error))?
            .join(db)
    };
    let (Some(name), Some(parent)) = (absolute.file_name(), absolute.parent()) else {
        return Err(StoreError::io(
            db,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the index database path names no file",
            ),
        ));
    };
    let dir = match fs::canonicalize(parent) {
        Ok(dir) if dir.is_dir() => dir,
        Ok(_) => {
            return Err(StoreError::DbDirMissing {
                dir: parent.to_path_buf(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(StoreError::DbDirMissing {
                dir: parent.to_path_buf(),
            });
        }
        Err(error) => return Err(StoreError::io(parent, error)),
    };
    if dir.starts_with(worktree) {
        return Err(StoreError::DbInsideWorktree {
            db: db.to_path_buf(),
            worktree: worktree.to_path_buf(),
        });
    }
    let target = dir.join(name);
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            // An existing file, or a symlink to one: judged (and opened) by
            // where it resolves; a dangling symlink is refused.
            let resolved =
                fs::canonicalize(&target).map_err(|error| StoreError::io(&target, error))?;
            if resolved.starts_with(worktree) {
                return Err(StoreError::DbInsideWorktree {
                    db: db.to_path_buf(),
                    worktree: worktree.to_path_buf(),
                });
            }
            Ok(resolved)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(target),
        Err(error) => Err(StoreError::io(&target, error)),
    }
}
