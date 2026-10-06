//! The queue as stored, for its backup and restore
//! (`docs/canon/queue-backup.md` "Store"): every row of `proposals` and
//! `events`, every project's, read raw in one snapshot and never decoded
//! (a row `get` refuses is kept as it is, never skipped as `list_readable`
//! skips it), and inserted as given into an empty queue in one `Immediate`
//! transaction that logs no event of its own: the next ID and `seq` follow
//! the highest restored.
//! The dump's format is the CLI's; the store reads and inserts rows only.

use std::fs;
use std::io;
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, Row, Transaction, TransactionBehavior};

use super::{
    ID_ORDER, QUEUE_SCHEMA_VERSION, QueueError, SqliteQueue, corrupt, migrate, user_version,
};
use crate::error::{Db, StoreError};
use crate::schema;

/// Every column of `proposals`, in table order (canon `proposal-queue.md`,
/// "Store"): queue schema 1's 24, then the eleven step 2 appends
/// (`docs/canon/agent-intake.md` "Stored"), then the five of a decision
/// record step 3 appends (task spec `decision-apply`, "Data").
pub const PROPOSAL_COLUMNS: [&str; 40] = [
    "id",
    "project",
    "kind",
    "status",
    "target_id",
    "target_path",
    "git_common_dir",
    "worktree",
    "root_rel",
    "branch",
    "base_commit",
    "base_hash",
    "base_text",
    "new_text",
    "patch_hash",
    "rationale",
    "author",
    "diagnostics",
    "decided_by",
    "decided_at",
    "decision_note",
    "applied_commit",
    "created_at",
    "updated_at",
    "target_ids",
    "severity",
    "gap_type",
    "summary",
    "working_answer",
    "price_of_other",
    "evidence",
    "options",
    "recommendation",
    "distinct_from",
    "linked",
    "record_id",
    "record_path",
    "record_title",
    "record_text",
    "choice",
];

/// The columns of queue schema 1's `proposals`: the first of
/// [`PROPOSAL_COLUMNS`].
const SCHEMA_1_COLUMNS: usize = 24;

/// The columns of queue schema 2's `proposals`: the first of
/// [`PROPOSAL_COLUMNS`].
const SCHEMA_2_COLUMNS: usize = 35;

/// The `proposals` columns of queue schema `schema`, in table order: 1, 2
/// and 3 (this build's) are known, a dump of any restores; `None` for any
/// other.
pub fn proposal_columns(schema: i64) -> Option<&'static [&'static str]> {
    match schema {
        1 => Some(&PROPOSAL_COLUMNS[..SCHEMA_1_COLUMNS]),
        2 => Some(&PROPOSAL_COLUMNS[..SCHEMA_2_COLUMNS]),
        QUEUE_SCHEMA_VERSION => Some(&PROPOSAL_COLUMNS),
        _ => None,
    }
}

/// Every column of `events`, in table order: `seq` (`INTEGER`), then the
/// `TEXT` ones.
pub const EVENT_COLUMNS: [&str; 5] = ["seq", "project", "type", "payload", "at"];

/// A `proposals` row as stored, never decoded: every column in
/// [`PROPOSAL_COLUMNS`] order, `TEXT` or `NULL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProposal {
    pub columns: [Option<String>; PROPOSAL_COLUMNS.len()],
}

impl StoredProposal {
    /// The stored `id`.
    pub fn id(&self) -> Option<&str> {
        self.columns[0].as_deref()
    }

    /// The stored `project`.
    pub fn project(&self) -> Option<&str> {
        self.columns[1].as_deref()
    }
}

/// An `events` row as stored: `seq`, and the `TEXT` columns after it in
/// [`EVENT_COLUMNS`] order (`project`, `type`, `payload`, `at`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub seq: i64,
    pub columns: [Option<String>; EVENT_COLUMNS.len() - 1],
}

impl StoredEvent {
    /// The stored `project`.
    pub fn project(&self) -> Option<&str> {
        self.columns[0].as_deref()
    }
}

/// Both queue tables as stored. Read ([`SqliteQueue::stored_rows`]):
/// `proposals` by ID number, `events` by `seq`; given to
/// [`SqliteQueue::restore`]: in any order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredQueue {
    pub proposals: Vec<StoredProposal>,
    pub events: Vec<StoredEvent>,
}

impl StoredQueue {
    /// Its row counts.
    pub fn counts(&self) -> QueueCounts {
        QueueCounts {
            proposals: self.proposals.len() as u64,
            events: self.events.len() as u64,
        }
    }
}

/// The rows of the two queue tables, every project's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueCounts {
    pub proposals: u64,
    pub events: u64,
}

impl QueueCounts {
    /// Neither table holds a row.
    pub fn is_empty(&self) -> bool {
        self.proposals == 0 && self.events == 0
    }
}

/// What [`SqliteQueue::restore`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restore {
    /// Every row inserted, committed.
    Restored,
    /// A table held a row (any project's) under the write lock: nothing
    /// inserted.
    Occupied(QueueCounts),
}

impl SqliteQueue {
    /// Opens the database `db` only when it exists, creating nothing and
    /// running no schema step: `None` when there is no such file (an empty
    /// queue). A `user_version` above this build's is
    /// [`QueueError::SchemaTooNew`]; a DB whose queue tables do not exist
    /// yet reads as an empty queue ([`Self::stored_rows`],
    /// [`Self::counts`]).
    pub fn open_existing(db: impl AsRef<Path>, project: &str) -> Result<Option<Self>, QueueError> {
        let db = db.as_ref();
        match fs::metadata(db) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StoreError::io(db, error).into()),
        }
        let conn = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .db()?;
        schema::configure(&conn)?;
        conn.pragma_update(None, "synchronous", "FULL").db()?;
        let version = user_version(&conn)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        Ok(Some(Self {
            conn,
            project: project.to_owned(),
        }))
    }

    /// Every row of both tables, every project's, as stored, in one read
    /// transaction: `proposals` by ID number, `events` by `seq`. A `TEXT`
    /// column that holds no UTF-8 text fails, naming row and column. A DB
    /// still at queue schema 1 or 2 (no step runs here) reads its 24 or 35
    /// columns, the later ones `None`.
    pub fn stored_rows(&self) -> Result<StoredQueue, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let mut state = StoredQueue::default();
        if has_queue_tables(&tx)? {
            let version = user_version(&tx)?;
            let columns = match proposal_columns(version) {
                Some(columns) => columns,
                None if version > QUEUE_SCHEMA_VERSION => {
                    return Err(QueueError::SchemaTooNew { found: version });
                }
                None => {
                    return Err(QueueError::Invalid(format!(
                        "the queue's tables stand at user_version {version}, which no SpecEngine \
                         writes"
                    )));
                }
            };
            let mut statement = tx
                .prepare(&format!(
                    "SELECT {} FROM main.proposals {ID_ORDER}",
                    columns.join(", ")
                ))
                .db()?;
            let mut rows = statement.query([]).db()?;
            while let Some(row) = rows.next().db()? {
                state.proposals.push(stored_proposal(row, columns.len())?);
            }
            drop(rows);
            drop(statement);
            let mut statement = tx
                .prepare(&format!(
                    "SELECT {} FROM main.events ORDER BY seq",
                    EVENT_COLUMNS.join(", ")
                ))
                .db()?;
            let mut rows = statement.query([]).db()?;
            while let Some(row) = rows.next().db()? {
                state.events.push(stored_event(row)?);
            }
        }
        tx.commit().db()?;
        Ok(state)
    }

    /// The row counts of both tables, every project's, in one snapshot.
    pub fn counts(&self) -> Result<QueueCounts, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let counts = counts_in(&tx)?;
        tx.commit().db()?;
        Ok(counts)
    }

    /// Restores `state` into an empty queue: in one `Immediate`
    /// transaction, both tables found empty (any project's rows) under the
    /// write lock, every row inserted as given, committed; no event of its
    /// own. Else [`Restore::Occupied`], nothing inserted. The queue's
    /// schema steps run first (a handle of [`Self::open_existing`] too). A
    /// row of another project than the handle's is
    /// [`QueueError::Invalid`], nothing written.
    pub fn restore(&mut self, state: &StoredQueue) -> Result<Restore, QueueError> {
        let project = self.project.clone();
        if let Some(row) = state
            .proposals
            .iter()
            .find(|row| row.project() != Some(project.as_str()))
        {
            return Err(QueueError::Invalid(format!(
                "proposal {}: not a row of the project `{project}`",
                row.id().unwrap_or("NULL")
            )));
        }
        if let Some(row) = state
            .events
            .iter()
            .find(|row| row.project() != Some(project.as_str()))
        {
            return Err(QueueError::Invalid(format!(
                "event {}: not a row of the project `{project}`",
                row.seq
            )));
        }
        migrate(&mut self.conn)?;
        let tx = self.write()?;
        let counts = counts_in(&tx)?;
        if !counts.is_empty() {
            return Ok(Restore::Occupied(counts));
        }
        {
            let placeholders: Vec<String> = (1..=PROPOSAL_COLUMNS.len())
                .map(|number| format!("?{number}"))
                .collect();
            let mut insert = tx
                .prepare(&format!(
                    "INSERT INTO main.proposals ({}) VALUES ({})",
                    PROPOSAL_COLUMNS.join(", "),
                    placeholders.join(", ")
                ))
                .db()?;
            for row in &state.proposals {
                insert
                    .execute(rusqlite::params_from_iter(row.columns.iter()))
                    .db()?;
            }
            let mut insert = tx
                .prepare(&format!(
                    "INSERT INTO main.events ({}) VALUES (?1, ?2, ?3, ?4, ?5)",
                    EVENT_COLUMNS.join(", ")
                ))
                .db()?;
            for row in &state.events {
                let [project, event_type, payload, at] = &row.columns;
                insert
                    .execute(rusqlite::params![row.seq, project, event_type, payload, at])
                    .db()?;
            }
        }
        tx.commit().db()?;
        Ok(Restore::Restored)
    }
}

/// Both queue tables exist (a DB the queue's steps have not reached has
/// none).
fn has_queue_tables(conn: &Connection) -> Result<bool, QueueError> {
    Ok(schema::has_table(conn, "proposals")? && schema::has_table(conn, "events")?)
}

fn counts_in(conn: &Connection) -> Result<QueueCounts, QueueError> {
    if !has_queue_tables(conn)? {
        return Ok(QueueCounts::default());
    }
    let count = |table: &str| -> Result<u64, QueueError> {
        let rows: i64 = conn
            .query_row(&format!("SELECT count(*) FROM main.{table}"), [], |row| {
                row.get(0)
            })
            .db()?;
        Ok(u64::try_from(rows).unwrap_or(0))
    };
    Ok(QueueCounts {
        proposals: count("proposals")?,
        events: count("events")?,
    })
}

/// A `proposals` row as stored, its first `width` columns read (the rest
/// `None`); a value that is no UTF-8 text fails, naming the row (its `id`,
/// as far as it reads) and the column.
fn stored_proposal(row: &Row<'_>, width: usize) -> Result<StoredProposal, QueueError> {
    let name = match row.get_ref(0).db()? {
        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        _ => "NULL".to_owned(),
    };
    let mut columns: [Option<String>; PROPOSAL_COLUMNS.len()] = std::array::from_fn(|_| None);
    for (index, column) in PROPOSAL_COLUMNS.iter().enumerate().take(width) {
        columns[index] = stored_text(row, index)?.map_err(|why| corrupt(&name, column, why))?;
    }
    Ok(StoredProposal { columns })
}

/// An `events` row as stored, as [`stored_proposal`].
fn stored_event(row: &Row<'_>) -> Result<StoredEvent, QueueError> {
    let seq: i64 = row.get(0).db()?;
    let mut columns: [Option<String>; EVENT_COLUMNS.len() - 1] = Default::default();
    for (index, column) in EVENT_COLUMNS.iter().enumerate().skip(1) {
        columns[index - 1] = stored_text(row, index)?.map_err(|why| {
            QueueError::Store(StoreError::Sqlite(format!(
                "event {seq}: the stored `{column}` cannot be read: {why}"
            )))
        })?;
    }
    Ok(StoredEvent { seq, columns })
}

/// A `TEXT` column's value: `Ok(Err(why))` when it is no UTF-8 text (a
/// `STRICT` table holds none, but a file written elsewhere may).
fn stored_text(
    row: &Row<'_>,
    index: usize,
) -> Result<Result<Option<String>, &'static str>, QueueError> {
    Ok(match row.get_ref(index).db()? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => Ok(Some(text.to_owned())),
            Err(_) => Err("it is not UTF-8 text"),
        },
        ValueRef::Integer(_) | ValueRef::Real(_) => Err("it is a number, not text"),
        ValueRef::Blob(_) => Err("it is a BLOB, not text"),
    })
}
