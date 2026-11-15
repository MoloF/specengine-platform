//! The feed's side of the queue (`docs/features/auto-feed.md`): what
//! `spec feed` and `GET …/feed` read, in one deferred read transaction
//! ([`FeedSnapshot`]), never a write.
//!
//! The rows come as stored, only the columns the feed shows: a task's
//! `id`, repository, `status`, `title` and `targets`; a run's outcome,
//! summary and end; a proposal's `kind`, `status`, `target_id`, `task_id`,
//! repository and `created_at`; a card's `title`. States are the stored
//! text, never decoded into an enum, so a value this build does not know is
//! shown as it is. The events are the project's (the slug's, as the
//! daemon's tail streams them), by `seq`; a payload that does not parse as
//! JSON is kept with no payload ([`FeedEvent::payload`] `None`), never
//! skipped: the caller lists it and says so. A row whose `type` is NULL is
//! passed over, as the tail passes it over.
//!
//! A handle of [`SqliteQueue::open_existing`] works: a table a queue's
//! schema does not hold yet reads as none, and nothing is created or
//! stepped; a newer build's queue is [`QueueError::SchemaTooNew`].

use rusqlite::{Row, Transaction, TransactionBehavior};
use serde_json::Value;

use super::{ID_ORDER, QUEUE_SCHEMA_VERSION, QueueError, SqliteQueue, index_only, user_version};
use crate::error::Db;
use crate::schema;

/// The queue step that made `tasks`, `runs` and `proposals.task_id`.
const TASKS_SCHEMA: i64 = 4;
/// The queue step that made `cards`.
const CARDS_SCHEMA: i64 = 9;

/// One event as the feed reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedEvent {
    pub seq: i64,
    pub event_type: String,
    /// `None`: the stored payload is NULL or no JSON.
    pub payload: Option<Value>,
    /// As stored.
    pub at: Option<String>,
}

/// Which events a read takes, by `seq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedWindow {
    /// `seq` above it.
    After(i64),
    /// `at` at or after it (`YYYY-MM-DDTHH:MM:SSZ`, compared as text).
    From(String),
    /// The newest kept ones, at most this many.
    Newest(usize),
}

/// A task row as the feed shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedTask {
    pub id: String,
    /// As stored.
    pub git_common_dir: Option<String>,
    /// The stored state's text.
    pub status: Option<String>,
    pub title: Option<String>,
    /// The stored JSON array's strings; none when it does not read.
    pub targets: Vec<String>,
}

/// A run row as the feed shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRun {
    pub task_id: String,
    pub run: i64,
    pub outcome: Option<String>,
    pub summary: Option<String>,
    pub ended_at: Option<String>,
}

/// A proposal row as the feed shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedProposal {
    pub id: String,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub target_id: Option<String>,
    pub task_id: Option<String>,
    pub git_common_dir: Option<String>,
    pub created_at: Option<String>,
}

/// A card row as the feed shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedCard {
    pub id: String,
    pub title: Option<String>,
}

/// One deferred read transaction over the queue for the feed: every read
/// through it sees one snapshot. It ends when dropped, nothing written.
pub struct FeedSnapshot<'q> {
    tx: Transaction<'q>,
    project: &'q str,
    version: i64,
}

impl SqliteQueue {
    /// The feed's snapshot (see the module documentation).
    pub fn feed_snapshot(&self) -> Result<FeedSnapshot<'_>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let version = user_version(&tx)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        Ok(FeedSnapshot {
            tx,
            project: &self.project,
            version,
        })
    }
}

impl FeedSnapshot<'_> {
    /// The project's tasks by number, every repository's.
    pub fn tasks(&self) -> Result<Vec<FeedTask>, QueueError> {
        if self.version < TASKS_SCHEMA || !schema::has_table(&self.tx, "tasks")? {
            return Ok(Vec::new());
        }
        let mut statement = self
            .tx
            .prepare(&format!(
                "SELECT id, git_common_dir, status, title, targets FROM main.tasks \
                 WHERE project = ?1 {ID_ORDER}"
            ))
            .db()?;
        let rows = statement
            .query_map([self.project], |row| {
                Ok(FeedTask {
                    id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    git_common_dir: row.get(1)?,
                    status: row.get(2)?,
                    title: row.get(3)?,
                    targets: strings(row.get::<_, Option<String>>(4)?.as_deref()),
                })
            })
            .db()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .db()?;
        Ok(rows)
    }

    /// The runs of the project's tasks, by task number, then run number.
    pub fn runs(&self) -> Result<Vec<FeedRun>, QueueError> {
        if self.version < TASKS_SCHEMA
            || !schema::has_table(&self.tx, "tasks")?
            || !schema::has_table(&self.tx, "runs")?
        {
            return Ok(Vec::new());
        }
        let mut statement = self
            .tx
            .prepare(
                "SELECT r.task_id, r.run, r.outcome, r.summary, r.ended_at FROM main.runs r \
                 JOIN main.tasks t ON t.id = r.task_id WHERE t.project = ?1 \
                 ORDER BY length(r.task_id), r.task_id, r.run",
            )
            .db()?;
        let rows = statement
            .query_map([self.project], |row| {
                Ok(FeedRun {
                    task_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    run: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                    outcome: row.get(2)?,
                    summary: row.get(3)?,
                    ended_at: row.get(4)?,
                })
            })
            .db()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .db()?;
        Ok(rows)
    }

    /// The project's proposals by ID number, every repository's; none in an
    /// index-only DB.
    pub fn proposals(&self) -> Result<Vec<FeedProposal>, QueueError> {
        if index_only(&self.tx)? || !schema::has_table(&self.tx, "proposals")? {
            return Ok(Vec::new());
        }
        // Below step 4 no proposal is bound to a task.
        let task = if self.version < TASKS_SCHEMA {
            "NULL"
        } else {
            "task_id"
        };
        let mut statement = self
            .tx
            .prepare(&format!(
                "SELECT id, kind, status, target_id, {task}, git_common_dir, created_at \
                 FROM main.proposals WHERE project = ?1 {ID_ORDER}"
            ))
            .db()?;
        let rows = statement
            .query_map([self.project], |row| {
                Ok(FeedProposal {
                    id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    kind: row.get(1)?,
                    status: row.get(2)?,
                    target_id: row.get(3)?,
                    task_id: row.get(4)?,
                    git_common_dir: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .db()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .db()?;
        Ok(rows)
    }

    /// The project's cards by number.
    pub fn cards(&self) -> Result<Vec<FeedCard>, QueueError> {
        if self.version < CARDS_SCHEMA || !schema::has_table(&self.tx, "cards")? {
            return Ok(Vec::new());
        }
        let mut statement = self
            .tx
            .prepare(&format!(
                "SELECT id, title FROM main.cards WHERE project = ?1 {ID_ORDER}"
            ))
            .db()?;
        let rows = statement
            .query_map([self.project], |row| {
                Ok(FeedCard {
                    id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    title: row.get(1)?,
                })
            })
            .db()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .db()?;
        Ok(rows)
    }

    /// Every event of the project whose type starts with `prefix`
    /// (`auto.`), by `seq`, whatever the window.
    pub fn events_typed(&self, prefix: &str) -> Result<Vec<FeedEvent>, QueueError> {
        if !schema::has_table(&self.tx, "events")? {
            return Ok(Vec::new());
        }
        let mut statement = self
            .tx
            .prepare(
                "SELECT seq, type, payload, at FROM main.events \
                 WHERE project = ?1 AND substr(type, 1, length(?2)) = ?2 ORDER BY seq",
            )
            .db()?;
        let mut rows = statement.query([self.project, prefix]).db()?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().db()? {
            if let Some(event) = feed_event(row).db()? {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// The window's events of one of `types` (none asked: every type), by
    /// `seq`, uncut; [`FeedWindow::Newest`] is the newest that many of the
    /// project's events, whatever their type, then those of `types`.
    pub fn events_in(
        &self,
        window: &FeedWindow,
        types: &[&str],
    ) -> Result<Vec<FeedEvent>, QueueError> {
        let mut events = Vec::new();
        let floor = match window {
            FeedWindow::Newest(count) => self.newest_floor(*count)?,
            _ => None,
        };
        self.walk(window, floor, |event| {
            if types.is_empty() || types.contains(&event.event_type.as_str()) {
                events.push(event);
            }
            true
        })?;
        Ok(events)
    }

    /// The window's events `keep` takes, by `seq`, at most `limit`, and
    /// whether one more was there (`more`). [`FeedWindow::Newest`]: the
    /// newest `min(limit, n)` kept, scanned newest first; `more` false.
    pub fn scan(
        &self,
        window: &FeedWindow,
        limit: usize,
        mut keep: impl FnMut(&FeedEvent) -> bool,
    ) -> Result<(Vec<FeedEvent>, bool), QueueError> {
        let mut events = Vec::new();
        let mut more = false;
        match window {
            FeedWindow::Newest(count) => {
                let limit = limit.min(*count);
                if limit > 0 && schema::has_table(&self.tx, "events")? {
                    let mut statement = self
                        .tx
                        .prepare(
                            "SELECT seq, type, payload, at FROM main.events \
                             WHERE project = ?1 ORDER BY seq DESC",
                        )
                        .db()?;
                    let mut rows = statement.query([self.project]).db()?;
                    while let Some(row) = rows.next().db()? {
                        let Some(event) = feed_event(row).db()? else {
                            continue;
                        };
                        if keep(&event) {
                            events.push(event);
                            if events.len() >= limit {
                                break;
                            }
                        }
                    }
                    events.reverse();
                }
            }
            _ => {
                self.walk(window, None, |event| {
                    if !keep(&event) {
                        return true;
                    }
                    if events.len() >= limit {
                        more = true;
                        return false;
                    }
                    events.push(event);
                    true
                })?;
            }
        }
        Ok((events, more))
    }

    /// The `seq` of the `count`-th newest event of the project: the
    /// [`FeedWindow::Newest`] window starts there; `None`, every event.
    fn newest_floor(&self, count: usize) -> Result<Option<i64>, QueueError> {
        if !schema::has_table(&self.tx, "events")? {
            return Ok(None);
        }
        let Some(offset) = count.checked_sub(1) else {
            return Ok(Some(i64::MAX));
        };
        let mut statement = self
            .tx
            .prepare(
                "SELECT seq FROM main.events WHERE project = ?1 AND type IS NOT NULL \
                 ORDER BY seq DESC LIMIT 1 OFFSET ?2",
            )
            .db()?;
        let mut rows = statement
            .query(rusqlite::params![
                self.project,
                i64::try_from(offset).unwrap_or(i64::MAX)
            ])
            .db()?;
        Ok(match rows.next().db()? {
            Some(row) => Some(row.get::<_, i64>(0).db()?),
            None => None,
        })
    }

    /// The window's events oldest first, each handed to `visit` until it
    /// answers `false`; `floor`, a lowest `seq` for [`FeedWindow::Newest`].
    fn walk(
        &self,
        window: &FeedWindow,
        floor: Option<i64>,
        mut visit: impl FnMut(FeedEvent) -> bool,
    ) -> Result<(), QueueError> {
        if !schema::has_table(&self.tx, "events")? {
            return Ok(());
        }
        let (condition, bound): (&str, rusqlite::types::Value) = match window {
            FeedWindow::After(seq) => ("seq > ?2", (*seq).into()),
            FeedWindow::From(at) => ("at >= ?2", at.clone().into()),
            FeedWindow::Newest(_) => ("seq >= ?2", floor.unwrap_or(i64::MIN).into()),
        };
        let mut statement = self
            .tx
            .prepare(&format!(
                "SELECT seq, type, payload, at FROM main.events \
                 WHERE project = ?1 AND {condition} ORDER BY seq"
            ))
            .db()?;
        let mut rows = statement
            .query(rusqlite::params![self.project, bound])
            .db()?;
        while let Some(row) = rows.next().db()? {
            let Some(event) = feed_event(row).db()? else {
                continue;
            };
            if !visit(event) {
                break;
            }
        }
        Ok(())
    }
}

/// An event row (`seq`, `type`, `payload`, `at`); `None` when its `type` is
/// NULL.
fn feed_event(row: &Row<'_>) -> rusqlite::Result<Option<FeedEvent>> {
    let Some(event_type) = row.get::<_, Option<String>>(1)? else {
        return Ok(None);
    };
    let payload = row
        .get::<_, Option<String>>(2)?
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    Ok(Some(FeedEvent {
        seq: row.get(0)?,
        event_type,
        payload,
        at: row.get(3)?,
    }))
}

/// A stored JSON array's strings; anything else reads as none.
fn strings(text: Option<&str>) -> Vec<String> {
    text.and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .unwrap_or_default()
}
