//! The task gate's side of the queue (`docs/canon/gate.md` "Verdict" step
//! 6, "Event"; ADR-0037): the tasks of one repository as the gate reads
//! them, and its one record, `gate.observed`. Both are bounded by the
//! gate's deadline: SQLite's busy handler waits only what remains of it
//! ([`StoreError::Busy`] past it).
//!
//! - [`open_for_gate`] opens an existing database read-only: no file is
//!   created, no schema step runs, no checkpoint on close; a newer build's
//!   queue is [`QueueError::SchemaTooNew`].
//! - [`GateQueue::tasks`]: the project's rows of the repository (compared
//!   as directories, [`same_repository`]) by number, only `id`, `status`
//!   and `claim` decoded ([`GateTask`]); a row that does not decode is
//!   named ([`UnreadableTask`]), never skipped silently: the caller decides
//!   whether it matters.
//! - [`record_gate_observed`]: one `gate.observed` event `{path, zone,
//!   worktree, branch}` in one `Immediate` transaction, only into a queue
//!   at this build's schema; no database or another schema records
//!   nothing (`Ok(false)`).

use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use serde::Serialize;
use specengine_core::gate::GateTask;
use specengine_core::task::task_number;
use specengine_model::{TaskClaim, TaskStatus};

use super::{
    ID_ORDER, QUEUE_SCHEMA_VERSION, QueueError, UnreadableTask, check_time, log_text, to_json,
    user_version,
};
use crate::error::{Db, StoreError};
use crate::{same_repository, schema};

/// `gate.observed`: an uncovered write `observe` let through.
pub const EVENT_GATE_OBSERVED: &str = "gate.observed";

/// The payload of `gate.observed`, its keys in this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateObserved {
    /// Root-relative, `/`-joined.
    pub path: String,
    /// `None`: the config file itself.
    pub zone: Option<String>,
    /// The file's worktree top, canonical.
    pub worktree: String,
    /// `None`: a detached `HEAD`.
    pub branch: Option<String>,
}

/// A project's queue opened for one gate verdict: read-only.
pub struct GateQueue {
    conn: Connection,
    project: String,
}

/// Opens the database `db` for the gate's read: `None` when there is no
/// such file. Read-only, nothing created, no schema step, no checkpoint
/// when closed; busy waits end at `deadline`.
pub fn open_for_gate(
    db: &Path,
    project: &str,
    deadline: Instant,
) -> Result<Option<GateQueue>, QueueError> {
    match fs::metadata(db) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StoreError::io(db, error).into()),
    }
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .db()?;
    settle(&conn, deadline)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .db()?;
    Ok(Some(GateQueue {
        conn,
        project: project.to_owned(),
    }))
}

impl GateQueue {
    /// The project's tasks of the repository `git_common_dir` by number, in
    /// one read transaction: each row's `id`, `status` and `claim`; a row
    /// of the repository whose `id`, `status` or `claim` does not decode
    /// (or an `in_progress` one without a claim, or one with no recorded
    /// repository) goes to the second list. A queue without `tasks` (a
    /// schema before 4) holds none.
    pub fn tasks(
        &self,
        git_common_dir: &Path,
    ) -> Result<(Vec<GateTask>, Vec<UnreadableTask>), QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let version = user_version(&tx)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        let mut tasks = Vec::new();
        let mut unreadable = Vec::new();
        if !schema::has_table(&tx, "tasks")? {
            tx.commit().db()?;
            return Ok((tasks, unreadable));
        }
        let rows = {
            let mut statement = tx
                .prepare(&format!(
                    "SELECT id, git_common_dir, status, claim FROM main.tasks \
                     WHERE project = ?1 {ID_ORDER}"
                ))
                .db()?;
            statement
                .query_map([&self.project], |row| {
                    Ok([
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ])
                })
                .db()?
                .collect::<rusqlite::Result<Vec<_>>>()
                .db()?
        };
        tx.commit().db()?;
        for [id, repository, status, claim] in rows {
            let id = id.unwrap_or_default();
            let Some(repository) = repository else {
                unreadable.push(corrupt(&id, "git_common_dir", "it is NULL"));
                continue;
            };
            if !same_repository(&repository, git_common_dir) {
                continue;
            }
            match decode(&id, status, claim) {
                Ok(task) => tasks.push(task),
                Err(row) => unreadable.push(row),
            }
        }
        Ok((tasks, unreadable))
    }
}

/// Records one `gate.observed` of `project` at `now`: `Ok(true)` stored;
/// `Ok(false)` nothing stored because there is no database `db`, or its
/// queue is not at this build's schema. Busy waits end at `deadline`.
pub fn record_gate_observed(
    db: &Path,
    project: &str,
    observed: &GateObserved,
    now: &str,
    deadline: Instant,
) -> Result<bool, QueueError> {
    check_time(now)?;
    match fs::metadata(db) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(StoreError::io(db, error).into()),
    }
    let mut conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .db()?;
    settle(&conn, deadline)?;
    conn.pragma_update(None, "synchronous", "FULL").db()?;
    let payload = to_json(observed)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .db()?;
    if user_version(&tx)? != QUEUE_SCHEMA_VERSION || !schema::has_table(&tx, "events")? {
        return Ok(false);
    }
    log_text(&tx, project, EVENT_GATE_OBSERVED, &payload, now)?;
    tx.commit().db()?;
    Ok(true)
}

/// The connection's settings for the gate: busy waits bounded by what
/// remains until `deadline` (none left: [`StoreError::Busy`]), schemas
/// untrusted.
fn settle(conn: &Connection, deadline: Instant) -> Result<(), QueueError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(StoreError::Busy.into());
    }
    conn.busy_timeout(remaining).db()?;
    conn.pragma_update(None, "trusted_schema", "OFF").db()?;
    Ok(())
}

/// The gate's columns of one row of the repository.
fn decode(
    id: &str,
    status: Option<String>,
    claim: Option<String>,
) -> Result<GateTask, UnreadableTask> {
    if task_number(id).is_none() {
        return Err(corrupt(id, "id", format!("{id:?} is no task ID")));
    }
    let status_text = status.ok_or_else(|| corrupt(id, "status", "it is NULL"))?;
    let status = TaskStatus::parse(&status_text)
        .ok_or_else(|| corrupt(id, "status", format!("`{status_text}` is no state")))?;
    let claim = match claim {
        Some(text) => Some(
            serde_json::from_str::<TaskClaim>(&text)
                .map_err(|error| corrupt(id, "claim", error))?,
        ),
        None => None,
    };
    if status == TaskStatus::InProgress && claim.is_none() {
        return Err(corrupt(
            id,
            "claim",
            "it is NULL, yet the task is in_progress",
        ));
    }
    Ok(GateTask {
        id: id.to_owned(),
        status,
        claim,
    })
}

fn corrupt(id: &str, column: &str, reason: impl std::fmt::Display) -> UnreadableTask {
    UnreadableTask {
        id: id.to_owned(),
        column: column.to_owned(),
        reason: reason.to_string(),
    }
}
