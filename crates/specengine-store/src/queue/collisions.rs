//! The collision ledger (task spec `collision-pass` CP3, CP5, CP6): one row
//! per pair of nodes an agent judged, keyed by the project and both names,
//! each name's `norm_hash` beside it. Step 13 → 14 makes the table. It is a
//! local cache, never a record: no dump, no restore and no import reads it,
//! a mark logs no event and changes no proposal, and nothing but the pass
//! reads it.
//!
//! - [`SqliteQueue::collision_marks`]: the project's rows by `(a, b)`, in
//!   one snapshot; a DB without the table (a schema 13 or lower queue
//!   opened with [`SqliteQueue::open_existing`]) reads none. A row that does
//!   not read is ignored and named in a note.
//! - [`SqliteQueue::mark_collisions`]: one `Immediate` transaction inserting
//!   or replacing one row per pair, `a` before `b` byte-wise, each norm
//!   following its name, the author as JSON and the call's clock.
//! - [`SqliteQueue::proposal_exists`]: whether the project's queue holds a
//!   proposal of that ID, in any state.
//! - [`clear_ledger`]: a restore empties its project's ledger in its
//!   transaction.

use rusqlite::types::ValueRef;
use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior};
use specengine_core::collisions::{CollisionVerdict, proposal_problem};
use specengine_core::drift::is_drift_hash;
use specengine_core::proposal::Author;

use super::{
    QUEUE_SCHEMA_VERSION, QueueError, SqliteQueue, check_time, index_only, to_json, user_version,
};
use crate::error::Db;
use crate::schema;

/// The ledger's table, step 14's.
const TABLE: &str = "collision_marks";

/// One verdict of the project's ledger, as it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionMark {
    /// Before `b` byte-wise.
    pub a: String,
    pub a_norm: String,
    pub b: String,
    pub b_norm: String,
    pub verdict: CollisionVerdict,
    /// The `PR-` ID a `reported` pair names.
    pub proposal: Option<String>,
    /// Core's `Author` as JSON, as stored.
    pub author: String,
    pub at: String,
}

/// The project's ledger: its rows that read, by `(a, b)`, and a note for
/// each row that does not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollisionMarks {
    pub marks: Vec<CollisionMark>,
    /// `collision mark <a> <b>: unreadable (<column>: <reason>); ignored`,
    /// by `(a, b)`.
    pub unreadable: Vec<String>,
}

/// One pair to mark, its sides in either order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCollisionMark {
    pub a: String,
    pub a_norm: String,
    pub b: String,
    pub b_norm: String,
    pub verdict: CollisionVerdict,
    pub proposal: Option<String>,
}

/// What one mark wrote: the rows inserted or replaced, and of them those a
/// row of the same pair stood in before.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkResult {
    pub marked: usize,
    pub replaced: usize,
}

/// The columns a read selects, in this order.
const COLUMNS: &str = "a, a_norm, b, b_norm, verdict, proposal, author, at";

impl SqliteQueue {
    /// The bound project's ledger, by `(a, b)`, in one read transaction;
    /// none in a DB without the table. A queue of a newer build is
    /// [`QueueError::SchemaTooNew`].
    pub fn collision_marks(&self) -> Result<CollisionMarks, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let version = user_version(&tx)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        let mut ledger = CollisionMarks::default();
        if schema::has_table(&tx, TABLE)? {
            let mut statement = tx
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM main.{TABLE} WHERE project = ?1 ORDER BY a, b"
                ))
                .db()?;
            let mut rows = statement.query([self.project.as_str()]).db()?;
            while let Some(row) = rows.next().db()? {
                match read_mark(row).db()? {
                    Ok(mark) => ledger.marks.push(mark),
                    Err(note) => ledger.unreadable.push(note),
                }
            }
        }
        tx.commit().db()?;
        Ok(ledger)
    }

    /// Marks `marks` for the bound project at `now` by `author`, in one
    /// `Immediate` transaction: each pair's row inserted, or replacing the
    /// row of the same two names. Only into a queue at this build's schema:
    /// a newer one is [`QueueError::SchemaTooNew`], an older one (a handle
    /// of [`SqliteQueue::open_existing`]) [`QueueError::Invalid`]. No event
    /// is logged and no other
    /// row changes. A pair of one node, a repeated pair, a bad norm, a
    /// `proposal` its verdict does not take or a bad `now`:
    /// [`QueueError::Invalid`], nothing written.
    pub fn mark_collisions(
        &mut self,
        marks: &[NewCollisionMark],
        author: &Author,
        now: &str,
    ) -> Result<MarkResult, QueueError> {
        check_time(now)?;
        let mut rows: Vec<(&str, &str, &str, &str, &NewCollisionMark)> =
            Vec::with_capacity(marks.len());
        for (index, mark) in marks.iter().enumerate() {
            if let Some(problem) = mark_problem(mark) {
                return Err(QueueError::Invalid(format!("pair {index}: {problem}")));
            }
            let (a, a_norm, b, b_norm) = ordered(mark);
            if rows.iter().any(|row| row.0 == a && row.2 == b) {
                return Err(QueueError::Invalid(format!(
                    "pair {index}: the pair is given twice"
                )));
            }
            rows.push((a, a_norm, b, b_norm, mark));
        }
        let author = to_json(author)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let found = user_version(&tx)?;
        if found > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found });
        }
        if found < QUEUE_SCHEMA_VERSION {
            return Err(QueueError::Invalid(format!(
                "the queue stands at schema {found}, below this build's \
                 {QUEUE_SCHEMA_VERSION}: open it with its schema steps"
            )));
        }
        let mut result = MarkResult::default();
        {
            let mut exists = tx
                .prepare(&format!(
                    "SELECT 1 FROM main.{TABLE} WHERE project = ?1 AND a = ?2 AND b = ?3"
                ))
                .db()?;
            let mut insert = tx
                .prepare(&format!(
                    "INSERT OR REPLACE INTO main.{TABLE} (project, {COLUMNS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
                ))
                .db()?;
            for &(a, a_norm, b, b_norm, mark) in &rows {
                let stood = exists
                    .query_row((project.as_str(), a, b), |_| Ok(()))
                    .optional()
                    .db()?
                    .is_some();
                insert
                    .execute((
                        project.as_str(),
                        a,
                        a_norm,
                        b,
                        b_norm,
                        mark.verdict.as_str(),
                        mark.proposal.as_deref(),
                        author.as_str(),
                        now,
                    ))
                    .db()?;
                result.marked += 1;
                result.replaced += usize::from(stood);
            }
        }
        tx.commit().db()?;
        Ok(result)
    }

    /// Whether the bound project's queue holds the proposal `id`, in any
    /// state; none in a DB no queue step has run on.
    pub fn proposal_exists(&self, id: &str) -> Result<bool, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let version = user_version(&tx)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        let found = if index_only(&tx)? || !schema::has_table(&tx, "proposals")? {
            false
        } else {
            tx.query_row(
                "SELECT 1 FROM main.proposals WHERE project = ?1 AND id = ?2",
                (self.project.as_str(), id),
                |_| Ok(()),
            )
            .optional()
            .db()?
            .is_some()
        };
        tx.commit().db()?;
        Ok(found)
    }
}

/// Empties `project`'s ledger inside the caller's transaction: a
/// restore's (no dump holds it); another project's rows stay, and a DB
/// without the table has none.
pub(super) fn clear_ledger(tx: &Transaction<'_>, project: &str) -> Result<(), QueueError> {
    if schema::has_table(tx, TABLE)? {
        tx.execute(
            &format!("DELETE FROM main.{TABLE} WHERE project = ?1"),
            [project],
        )
        .db()?;
    }
    Ok(())
}

/// `(a, a_norm, b, b_norm)` with `a` before `b` byte-wise.
fn ordered(mark: &NewCollisionMark) -> (&str, &str, &str, &str) {
    if mark.a > mark.b {
        (&mark.b, &mark.b_norm, &mark.a, &mark.a_norm)
    } else {
        (&mark.a, &mark.a_norm, &mark.b, &mark.b_norm)
    }
}

/// Why a pair cannot be stored as given.
fn mark_problem(mark: &NewCollisionMark) -> Option<String> {
    if mark.a == mark.b {
        return Some("a pair of one node".to_owned());
    }
    for (name, norm) in [("a_norm", &mark.a_norm), ("b_norm", &mark.b_norm)] {
        if !is_drift_hash(norm) {
            return Some(format!("{name}: not `b3:` and 64 lower-case hex digits"));
        }
    }
    proposal_problem(mark.verdict, mark.proposal.as_deref())
        .map(|problem| format!("proposal: {problem}"))
}

/// One row as a mark, or the note naming why it does not read.
fn read_mark(row: &Row<'_>) -> rusqlite::Result<Result<CollisionMark, String>> {
    let mut texts: Vec<Option<String>> = Vec::with_capacity(8);
    let mut bad: Option<(&str, String)> = None;
    for (index, column) in [
        "a", "a_norm", "b", "b_norm", "verdict", "proposal", "author", "at",
    ]
    .into_iter()
    .enumerate()
    {
        let text = match row.get_ref(index)? {
            ValueRef::Null => None,
            ValueRef::Text(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) => Some(text.to_owned()),
                Err(_) => {
                    bad.get_or_insert((column, "not UTF-8".to_owned()));
                    Some(String::from_utf8_lossy(bytes).into_owned())
                }
            },
            _ => {
                bad.get_or_insert((column, "not text".to_owned()));
                None
            }
        };
        texts.push(text);
    }
    let mut texts = texts.into_iter();
    let mut next = || texts.next().flatten();
    let (a, a_norm, b, b_norm) = (next(), next(), next(), next());
    let (verdict, proposal, author, at) = (next(), next(), next(), next());
    let unreadable = |column: &str, reason: &str| {
        format!(
            "collision mark {} {}: unreadable ({column}: {reason}); ignored",
            a.as_deref().unwrap_or("NULL"),
            b.as_deref().unwrap_or("NULL")
        )
    };
    if let Some((column, reason)) = &bad {
        return Ok(Err(unreadable(column, reason)));
    }
    let (Some(a_name), Some(b_name)) = (a.as_deref(), b.as_deref()) else {
        return Ok(Err(unreadable("a", "NULL")));
    };
    if a_name >= b_name {
        return Ok(Err(unreadable("a", "not before b")));
    }
    for (column, norm) in [("a_norm", &a_norm), ("b_norm", &b_norm)] {
        if !norm.as_deref().is_some_and(is_drift_hash) {
            return Ok(Err(unreadable(
                column,
                "not `b3:` and 64 lower-case hex digits",
            )));
        }
    }
    let Some(parsed) = verdict.as_deref().and_then(CollisionVerdict::parse) else {
        return Ok(Err(unreadable(
            "verdict",
            &format!("unknown verdict `{}`", verdict.as_deref().unwrap_or("NULL")),
        )));
    };
    if parsed == CollisionVerdict::Reported && proposal.is_none() {
        return Ok(Err(unreadable("proposal", "a reported pair names none")));
    }
    Ok(Ok(CollisionMark {
        a: a_name.to_owned(),
        a_norm: a_norm.unwrap_or_default(),
        b: b_name.to_owned(),
        b_norm: b_norm.unwrap_or_default(),
        verdict: parsed,
        proposal,
        author: author.unwrap_or_default(),
        at: at.unwrap_or_default(),
    }))
}
