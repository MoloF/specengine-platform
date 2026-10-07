//! Tasks and their runs in the project's queue
//! (`docs/canon/tasks.md` "Store", "Transitions", "Task-bound proposals"):
//! a task is a record only
//! the owner moves to `ready`; an agent claims it, reports its run and
//! completes it. Every change is one `Immediate` transaction holding the
//! change and its event, a compare-and-set on the task's `revision` the
//! caller read ([`TaskSeen`]), which every change (a snapshot refresh too)
//! raises by one: two changes within one second never overwrite each
//! other. IDs are the highest number plus one, rows never deleted. Nothing here decides what a task holds: the CLI resolves
//! targets and freezes texts; the store keeps them, checks the transition
//! (core [`transition`]) and the run, and refuses the rest
//! ([`QueueError::TaskRefused`]).
//!
//! Columns hold the package's keys as JSON
//! (`docs/canon/task-package.md` "Package"): `targets` and `affected_nodes` canonical IDs or paths,
//! `criteria` `{ref, text}` (a reference's text `null`), `owner_notes`
//! `{at, note, by}`, `snapshot` `{at, by, place, nodes: [{id, path,
//! span_hash, text}]}`, `claim` `{at, role, worktree, branch}`, `author` and
//! a run's `author` as a proposal's. A value that does not decode, or an
//! unknown state, is a corrupt row, named ([`UnreadableTask`]).

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specengine_core::proposal::{Author, author_field_problem};
use specengine_core::task::{TaskAction, needs_message, task_id, task_number, transition};
use specengine_model::{RunOutcome, SnapshotPlace, TaskClaim, TaskCriterion, TaskStatus};

use super::{ID_ORDER, Place, QueueError, SqliteQueue, check_time, log, to_json};
use crate::error::{Db, StoreError};
use crate::git::same_dir;
use crate::same_repository;

/// `task.created`.
pub const EVENT_TASK_CREATED: &str = "task.created";
/// `task.planned`.
pub const EVENT_TASK_PLANNED: &str = "task.planned";
/// `task.approved`: the snapshot (re)frozen.
pub const EVENT_TASK_APPROVED: &str = "task.approved";
/// `task.changes_requested`.
pub const EVENT_TASK_CHANGES_REQUESTED: &str = "task.changes_requested";
/// `task.claimed`, with `run`.
pub const EVENT_TASK_CLAIMED: &str = "task.claimed";
/// `task.run_reported`, with `run`.
pub const EVENT_TASK_RUN_REPORTED: &str = "task.run_reported";
/// `task.completed`.
pub const EVENT_TASK_COMPLETED: &str = "task.completed";
/// `task.cancelled`.
pub const EVENT_TASK_CANCELLED: &str = "task.cancelled";
/// `task.refreshed`, with `proposal` and `node`: one snapshot node took a
/// bound proposal's applied text.
pub const EVENT_TASK_REFRESHED: &str = "task.refreshed";

/// The order of runs: by task number, then run number.
pub(crate) const RUN_ORDER: &str = "ORDER BY length(task_id), task_id, run";

/// The event of `action`.
pub const fn task_event(action: TaskAction) -> &'static str {
    match action {
        TaskAction::New => EVENT_TASK_CREATED,
        TaskAction::Plan => EVENT_TASK_PLANNED,
        TaskAction::Approve => EVENT_TASK_APPROVED,
        TaskAction::Changes => EVENT_TASK_CHANGES_REQUESTED,
        TaskAction::Claim => EVENT_TASK_CLAIMED,
        TaskAction::Report => EVENT_TASK_RUN_REPORTED,
        TaskAction::Complete => EVENT_TASK_COMPLETED,
        TaskAction::Cancel => EVENT_TASK_CANCELLED,
    }
}

const TASK_SELECT: &str = "id, project, git_common_dir, status, title, goal, targets, plan, \
     criteria, affected_nodes, owner_notes, snapshot, claim, author, created_at, updated_at, \
     revision";

const RUN_SELECT: &str = "task_id, run, role, worktree, branch, author, started_at, ended_at, \
     outcome, summary, changed_files";

/// What `spec task new` stores; the queue adds the ID, project, state and
/// times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    /// The repository it was made in: its canonical git common dir.
    pub git_common_dir: String,
    pub title: Option<String>,
    pub goal: Option<String>,
    /// Canonical IDs, or an id-less document's path, each once.
    pub targets: Vec<String>,
    pub author: Author,
}

/// An owner's note (`spec task changes --note`), with who wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredNote {
    pub at: String,
    pub note: String,
    /// The owner's git identity, `Name <email>`.
    pub by: String,
}

/// A snapshot node as frozen: its text from disk and its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub id: String,
    /// Root-relative, in the snapshot's place.
    pub path: String,
    pub span_hash: String,
    pub text: String,
}

/// The snapshot `approve` freezes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub at: String,
    /// The approving owner's git identity.
    pub by: String,
    pub place: SnapshotPlace,
    pub nodes: Vec<SnapshotEntry>,
}

/// One run of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub task_id: String,
    /// From 1.
    pub run: u64,
    pub role: String,
    pub worktree: String,
    pub branch: String,
    pub author: Author,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub outcome: Option<RunOutcome>,
    pub summary: Option<String>,
    pub changed_files: Vec<String>,
}

/// A stored task, its runs by number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub project: String,
    /// The repository it was made in.
    pub git_common_dir: String,
    pub status: TaskStatus,
    pub title: Option<String>,
    pub goal: Option<String>,
    pub targets: Vec<String>,
    pub plan: Option<String>,
    pub criteria: Vec<TaskCriterion>,
    pub affected_nodes: Vec<String>,
    /// Oldest first.
    pub owner_notes: Vec<StoredNote>,
    pub snapshot: Option<TaskSnapshot>,
    pub claim: Option<TaskClaim>,
    pub author: Author,
    pub created_at: String,
    pub updated_at: String,
    /// 1 when made, one more on every change.
    pub revision: u64,
    pub runs: Vec<Run>,
}

impl Task {
    /// Its state as read now: the key of a compare-and-set change.
    pub fn seen(&self) -> TaskSeen {
        TaskSeen {
            status: self.status,
            updated_at: self.updated_at.clone(),
            revision: self.revision,
        }
    }

    /// Its last run while it has not ended.
    pub fn open_run(&self) -> Option<&Run> {
        self.runs.last().filter(|run| run.ended_at.is_none())
    }
}

/// The state of a task as one run read it: the key of
/// [`SqliteQueue::change_task`], compared by `revision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSeen {
    pub status: TaskStatus,
    pub updated_at: String,
    pub revision: u64,
}

/// A stored task that does not decode: its ID (as stored), the column and
/// why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableTask {
    pub id: String,
    pub column: String,
    pub reason: String,
}

impl fmt::Display for UnreadableTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "task {}: the stored `{}` cannot be read: {}",
            self.id, self.column, self.reason
        )
    }
}

impl From<UnreadableTask> for QueueError {
    fn from(row: UnreadableTask) -> Self {
        Self::Store(StoreError::Sqlite(row.to_string()))
    }
}

/// What [`SqliteQueue::list_tasks`] gives: the tasks by number, and the
/// rows skipped because they do not decode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskList {
    pub tasks: Vec<Task>,
    pub unreadable: Vec<UnreadableTask>,
}

/// One change of [`SqliteQueue::change_task`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskChange {
    /// `draft`, `changes_requested` → `review`, the plan replaced.
    Plan {
        plan: String,
        criteria: Vec<TaskCriterion>,
        affected_nodes: Vec<String>,
    },
    /// → `ready`, the snapshot (re)frozen.
    Approve { snapshot: TaskSnapshot },
    /// `review` → `changes_requested`, the note appended.
    Changes { note: StoredNote },
    /// `ready` → `in_progress`, the claim set and a run opened.
    Claim { claim: TaskClaim, author: Author },
    /// The open run closed; the state kept.
    Report {
        outcome: RunOutcome,
        summary: String,
        changed_files: Vec<String>,
    },
    /// `in_progress`, its run closed → `done`.
    Complete,
    /// Any state before `done` → `cancelled`.
    Cancel,
}

impl TaskChange {
    pub const fn action(&self) -> TaskAction {
        match self {
            Self::Plan { .. } => TaskAction::Plan,
            Self::Approve { .. } => TaskAction::Approve,
            Self::Changes { .. } => TaskAction::Changes,
            Self::Claim { .. } => TaskAction::Claim,
            Self::Report { .. } => TaskAction::Report,
            Self::Complete => TaskAction::Complete,
            Self::Cancel => TaskAction::Cancel,
        }
    }
}

/// The snapshot nodes a bound proposal's apply refreshes
/// ([`SqliteQueue::applied_refreshing`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRefresh {
    pub task_id: String,
    pub entries: Vec<RefreshEntry>,
}

/// One snapshot node: refreshed only while still frozen at `was`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshEntry {
    pub id: String,
    /// Its snapshot hash, equal to its pre-apply hash.
    pub was: String,
    /// The applied text and its hash.
    pub span_hash: String,
    pub text: String,
}

/// A refusal of the task `id`, exit 1.
fn refused(id: &str, reason: String) -> QueueError {
    QueueError::TaskRefused {
        id: id.to_owned(),
        reason,
    }
}

/// `no task \`T\` in this project's queue`.
fn no_task(id: &str) -> QueueError {
    refused(id, format!("no task `{id}` in this project's queue"))
}

impl SqliteQueue {
    /// Stores `task` as `draft` under the next ID (highest + 1, taken in
    /// this transaction), `created_at` = `updated_at` = `now`, its plan,
    /// snapshot and claim `NULL`, its lists `[]`; logs `task.created`.
    pub fn create_task(&mut self, task: &NewTask, now: &str) -> Result<Task, QueueError> {
        check_time(now)?;
        if task.targets.is_empty() {
            return Err(QueueError::Invalid(
                "a task names at least one node".to_owned(),
            ));
        }
        let project = self.project.clone();
        let tx = self.write()?;
        let id = next_task_id(&tx)?;
        tx.execute(
            &format!(
                "INSERT INTO main.tasks ({TASK_SELECT}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, \
                 '[]', '[]', '[]', NULL, NULL, ?8, ?9, ?9, '1')"
            ),
            rusqlite::params![
                id,
                project,
                task.git_common_dir,
                TaskStatus::Draft.as_str(),
                task.title,
                task.goal,
                to_json(&task.targets)?,
                to_json(&task.author)?,
                now,
            ],
        )
        .db()?;
        log(&tx, &project, EVENT_TASK_CREATED, &json!({ "id": id }), now)?;
        let stored = existing_task(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// The project's task `id` with its runs, in one snapshot; `Ok(None)`
    /// when there is none; a row that does not decode fails, named.
    pub fn get_task(&self, id: &str) -> Result<Option<Task>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let task = select_task(&tx, &self.project, id)?;
        tx.commit().db()?;
        Ok(task)
    }

    /// The project's tasks (of the repository `git_common_dir` when given)
    /// by number, in one snapshot; a row that does not decode is skipped
    /// into [`TaskList::unreadable`].
    pub fn list_tasks(&self, git_common_dir: Option<&str>) -> Result<TaskList, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let rows = {
            let mut statement = tx
                .prepare(&format!(
                    "SELECT {TASK_SELECT} FROM main.tasks WHERE project = ?1 \
                     AND (?2 IS NULL OR git_common_dir = ?2) {ID_ORDER}"
                ))
                .db()?;
            statement
                .query_map(rusqlite::params![self.project, git_common_dir], raw_task)
                .db()?
                .collect::<rusqlite::Result<Vec<_>>>()
                .db()?
        };
        let mut list = TaskList::default();
        for raw in rows {
            let id = raw[0].clone().unwrap_or_default();
            let runs = raw_runs(&tx, &id)?;
            match decode_task(raw, runs) {
                Ok(task) => list.tasks.push(task),
                Err(row) => list.unreadable.push(row),
            }
        }
        tx.commit().db()?;
        Ok(list)
    }

    /// One change of the task `id` as a compare-and-set on `seen` (another
    /// run's change since: [`QueueError::TaskRefused`], nothing written):
    /// the transition (core [`transition`]), `report` with its run open,
    /// `complete` with it closed, else refused naming why; then the change,
    /// its event (`task.claimed` and `task.run_reported` with `run`) and
    /// `updated_at` = `now`, in one transaction. The stored task and the
    /// run it opened or closed.
    pub fn change_task(
        &mut self,
        id: &str,
        seen: &TaskSeen,
        change: &TaskChange,
        now: &str,
    ) -> Result<(Task, Option<u64>), QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = select_task(&tx, &project, id)?.ok_or_else(|| no_task(id))?;
        if current.revision != seen.revision {
            return Err(refused(
                id,
                format!(
                    "`{id}` changed since this run read it: it is {} since {}; nothing changed",
                    current.status, current.updated_at
                ),
            ));
        }
        let action = change.action();
        let to = transition(Some(current.status), action)
            .map_err(|needs| refused(id, needs_message(id, current.status, action, needs)))?;
        let mut run = None;
        match change {
            TaskChange::Plan {
                plan,
                criteria,
                affected_nodes,
            } => {
                tx.execute(
                    "UPDATE main.tasks SET status = ?1, plan = ?2, criteria = ?3, \
                     affected_nodes = ?4, updated_at = ?5 WHERE id = ?6 AND project = ?7",
                    rusqlite::params![
                        to.as_str(),
                        plan,
                        to_json(criteria)?,
                        to_json(affected_nodes)?,
                        now,
                        id,
                        project
                    ],
                )
                .db()?;
            }
            TaskChange::Approve { snapshot } => {
                tx.execute(
                    "UPDATE main.tasks SET status = ?1, snapshot = ?2, updated_at = ?3 \
                     WHERE id = ?4 AND project = ?5",
                    rusqlite::params![to.as_str(), to_json(snapshot)?, now, id, project],
                )
                .db()?;
            }
            TaskChange::Changes { note } => {
                let mut notes = current.owner_notes.clone();
                notes.push(note.clone());
                tx.execute(
                    "UPDATE main.tasks SET status = ?1, owner_notes = ?2, updated_at = ?3 \
                     WHERE id = ?4 AND project = ?5",
                    rusqlite::params![to.as_str(), to_json(&notes)?, now, id, project],
                )
                .db()?;
            }
            TaskChange::Claim { claim, author } => {
                let number = current.runs.last().map_or(1, |last| last.run + 1);
                tx.execute(
                    "UPDATE main.tasks SET status = ?1, claim = ?2, updated_at = ?3 \
                     WHERE id = ?4 AND project = ?5",
                    rusqlite::params![to.as_str(), to_json(claim)?, now, id, project],
                )
                .db()?;
                tx.execute(
                    &format!(
                        "INSERT INTO main.runs ({RUN_SELECT}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, \
                         NULL, NULL, NULL, NULL)"
                    ),
                    rusqlite::params![
                        id,
                        i64::try_from(number).unwrap_or(i64::MAX),
                        claim.role,
                        claim.worktree,
                        claim.branch,
                        to_json(author)?,
                        now,
                    ],
                )
                .db()?;
                run = Some(number);
            }
            TaskChange::Report {
                outcome,
                summary,
                changed_files,
            } => {
                let Some(open) = current.open_run() else {
                    return Err(refused(
                        id,
                        format!(
                            "{id} is {}, no run open: `report` needs its open run (`claim` \
                             opens one); nothing changed",
                            current.status
                        ),
                    ));
                };
                tx.execute(
                    "UPDATE main.runs SET ended_at = ?1, outcome = ?2, summary = ?3, \
                     changed_files = ?4 WHERE task_id = ?5 AND run = ?6",
                    rusqlite::params![
                        now,
                        outcome.as_str(),
                        summary,
                        to_json(changed_files)?,
                        id,
                        i64::try_from(open.run).unwrap_or(i64::MAX)
                    ],
                )
                .db()?;
                tx.execute(
                    "UPDATE main.tasks SET updated_at = ?1 WHERE id = ?2 AND project = ?3",
                    rusqlite::params![now, id, project],
                )
                .db()?;
                run = Some(open.run);
            }
            TaskChange::Complete => {
                if let Some(open) = current.open_run() {
                    return Err(refused(
                        id,
                        format!(
                            "{id} is {}, its run {} open: `complete` needs it closed \
                             (`report`); nothing changed",
                            current.status, open.run
                        ),
                    ));
                }
                set_status(&tx, &project, id, to, now)?;
            }
            TaskChange::Cancel => {
                // A run still open ends with the task, its outcome unknown.
                if let Some(open) = current.open_run() {
                    tx.execute(
                        "UPDATE main.runs SET ended_at = ?1 WHERE task_id = ?2 AND run = ?3",
                        rusqlite::params![now, id, i64::try_from(open.run).unwrap_or(i64::MAX)],
                    )
                    .db()?;
                }
                set_status(&tx, &project, id, to, now)?;
            }
        }
        bump(&tx, &project, id, current.revision)?;
        let payload = match run {
            Some(run) => json!({ "id": id, "run": run }),
            None => json!({ "id": id }),
        };
        log(&tx, &project, task_event(action), &payload, now)?;
        let stored = existing_task(&tx, &project, id)?;
        tx.commit().db()?;
        Ok((stored, run))
    }
}

/// The task's `revision` after `was`: one more.
fn bump(tx: &Transaction<'_>, project: &str, id: &str, was: u64) -> Result<(), QueueError> {
    tx.execute(
        "UPDATE main.tasks SET revision = ?1 WHERE id = ?2 AND project = ?3",
        rusqlite::params![was.saturating_add(1).to_string(), id, project],
    )
    .db()?;
    Ok(())
}

fn set_status(
    tx: &Transaction<'_>,
    project: &str,
    id: &str,
    to: TaskStatus,
    now: &str,
) -> Result<(), QueueError> {
    tx.execute(
        "UPDATE main.tasks SET status = ?1, updated_at = ?2 WHERE id = ?3 AND project = ?4",
        rusqlite::params![to.as_str(), now, id, project],
    )
    .db()?;
    Ok(())
}

/// The next task ID: the highest number of any stored task, plus one.
fn next_task_id(tx: &Transaction<'_>) -> Result<String, QueueError> {
    let mut statement = tx.prepare("SELECT id FROM main.tasks").db()?;
    let mut rows = statement.query([]).db()?;
    let mut highest = 0;
    while let Some(row) = rows.next().db()? {
        let id: Option<String> = row.get(0).db()?;
        if let Some(number) = id.as_deref().and_then(task_number) {
            highest = highest.max(number);
        }
    }
    let next = highest.checked_add(1).ok_or_else(|| {
        QueueError::Store(StoreError::Sqlite(format!(
            "the queue holds `{}`: no task ID follows it",
            task_id(highest)
        )))
    })?;
    Ok(task_id(next))
}

/// A task row's columns in [`TASK_SELECT`] order, not decoded.
type RawTask = Vec<Option<String>>;

fn raw_task(row: &Row<'_>) -> rusqlite::Result<RawTask> {
    (0..17)
        .map(|column| row.get::<_, Option<String>>(column))
        .collect()
}

/// A run row as read: `run` aside, its columns in [`RUN_SELECT`] order.
struct RawRun {
    run: Option<i64>,
    text: Vec<Option<String>>,
}

fn raw_runs(conn: &Connection, task: &str) -> Result<Vec<RawRun>, QueueError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT {RUN_SELECT} FROM main.runs WHERE task_id = ?1 ORDER BY run"
        ))
        .db()?;
    let rows = statement
        .query_map([task], |row| {
            let mut text = Vec::with_capacity(11);
            for column in 0..11 {
                text.push(if column == 1 {
                    None
                } else {
                    row.get::<_, Option<String>>(column)?
                });
            }
            Ok(RawRun {
                run: row.get::<_, Option<i64>>(1)?,
                text,
            })
        })
        .db()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .db()?;
    Ok(rows)
}

fn select_task(conn: &Connection, project: &str, id: &str) -> Result<Option<Task>, QueueError> {
    let raw = conn
        .query_row(
            &format!("SELECT {TASK_SELECT} FROM main.tasks WHERE id = ?1 AND project = ?2"),
            [id, project],
            raw_task,
        )
        .optional()
        .db()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let runs = raw_runs(conn, id)?;
    Ok(Some(decode_task(raw, runs)?))
}

fn existing_task(tx: &Transaction<'_>, project: &str, id: &str) -> Result<Task, QueueError> {
    select_task(tx, project, id)?.ok_or_else(|| no_task(id))
}

fn unreadable(id: &str, column: &str, reason: impl fmt::Display) -> UnreadableTask {
    UnreadableTask {
        id: id.to_owned(),
        column: column.to_owned(),
        reason: reason.to_string(),
    }
}

/// The JSON of `column`, decoded; `NULL` is `default` when given, else
/// corrupt.
fn json_column<T: serde::de::DeserializeOwned>(
    id: &str,
    column: &str,
    value: Option<String>,
    default: Option<T>,
) -> Result<T, UnreadableTask> {
    match (value, default) {
        (Some(text), _) => {
            serde_json::from_str(&text).map_err(|error| unreadable(id, column, error))
        }
        (None, Some(default)) => Ok(default),
        (None, None) => Err(unreadable(id, column, "it is NULL")),
    }
}

/// The author JSON of `column`, its fields checked as a proposal's.
fn author_column(id: &str, column: &str, value: Option<String>) -> Result<Author, UnreadableTask> {
    let author: Author = json_column(id, column, value, None)?;
    for field in [&author.role, &author.model, &author.run] {
        if let Some(problem) = field.as_deref().and_then(author_field_problem) {
            return Err(unreadable(id, column, problem));
        }
    }
    Ok(author)
}

fn decode_task(raw: RawTask, runs: Vec<RawRun>) -> Result<Task, UnreadableTask> {
    let mut values = raw.into_iter();
    let mut take = || values.next().flatten();
    let id = take().unwrap_or_default();
    if task_number(&id).is_none() {
        return Err(unreadable(&id, "id", format!("{id:?} is no task ID")));
    }
    let required = |value: Option<String>, column: &str| {
        value.ok_or_else(|| unreadable(&id, column, "it is NULL"))
    };
    let project = required(take(), "project")?;
    let git_common_dir = required(take(), "git_common_dir")?;
    let status_text = required(take(), "status")?;
    let status = TaskStatus::parse(&status_text)
        .ok_or_else(|| unreadable(&id, "status", format!("`{status_text}` is no state")))?;
    let title = take();
    let goal = take();
    let targets: Vec<String> = json_column(&id, "targets", take(), None)?;
    let plan = take();
    let criteria: Vec<TaskCriterion> = json_column(&id, "criteria", take(), Some(Vec::new()))?;
    let affected_nodes: Vec<String> = json_column(&id, "affected_nodes", take(), Some(Vec::new()))?;
    let owner_notes: Vec<StoredNote> = json_column(&id, "owner_notes", take(), Some(Vec::new()))?;
    let snapshot: Option<TaskSnapshot> = json_column(&id, "snapshot", take(), Some(None))?;
    let claim: Option<TaskClaim> = json_column(&id, "claim", take(), Some(None))?;
    let author = author_column(&id, "author", take())?;
    let created_at = required(take(), "created_at")?;
    let updated_at = required(take(), "updated_at")?;
    let revision_text = required(take(), "revision")?;
    let revision = revision_text
        .parse::<u64>()
        .ok()
        .filter(|revision| *revision >= 1 && revision.to_string() == revision_text)
        .ok_or_else(|| {
            unreadable(
                &id,
                "revision",
                format!("{revision_text:?} is no revision (1, 2, …)"),
            )
        })?;
    let mut decoded = Vec::with_capacity(runs.len());
    for run in runs {
        decoded.push(decode_run(&id, run)?);
    }
    Ok(Task {
        id,
        project,
        git_common_dir,
        status,
        title,
        goal,
        targets,
        plan,
        criteria,
        affected_nodes,
        owner_notes,
        snapshot,
        claim,
        author,
        created_at,
        updated_at,
        revision,
        runs: decoded,
    })
}

fn decode_run(id: &str, raw: RawRun) -> Result<Run, UnreadableTask> {
    let run = raw
        .run
        .and_then(|run| u64::try_from(run).ok())
        .filter(|&run| run >= 1)
        .ok_or_else(|| unreadable(id, "runs.run", "it is no run number from 1"))?;
    let column = |name: &str| format!("runs[{run}].{name}");
    let mut values = raw.text.into_iter();
    let mut take = || values.next().flatten();
    let task_id = take().unwrap_or_default();
    let _run_column = take();
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| unreadable(id, &column(name), "it is NULL"))
    };
    let role = required(take(), "role")?;
    let worktree = required(take(), "worktree")?;
    let branch = required(take(), "branch")?;
    let author = author_column(id, &column("author"), take())?;
    let started_at = required(take(), "started_at")?;
    let ended_at = take();
    let outcome = match take() {
        Some(text) => Some(RunOutcome::parse(&text).ok_or_else(|| {
            unreadable(id, &column("outcome"), format!("`{text}` is no outcome"))
        })?),
        None => None,
    };
    let summary = take();
    let changed_files: Vec<String> =
        json_column(id, &column("changed_files"), take(), Some(Vec::new()))?;
    Ok(Run {
        task_id,
        run,
        role,
        worktree,
        branch,
        author,
        started_at,
        ended_at,
        outcome,
        summary,
        changed_files,
    })
}

/// A proposal raised at `place` may be bound to the task `id`: the
/// project's, of that repository, neither `done` nor `cancelled`, and once
/// claimed, claimed in that worktree
/// (`docs/canon/tasks.md` "Task-bound proposals"). Read inside the
/// inserting transaction; else [`QueueError::TaskRefused`] naming why.
pub(crate) fn check_binding(
    tx: &Transaction<'_>,
    project: &str,
    id: &str,
    place: &Place,
) -> Result<(), QueueError> {
    let task = select_task(tx, project, id)?.ok_or_else(|| no_task(id))?;
    if let Some(reason) = binding_problem(&task, place) {
        return Err(refused(id, reason));
    }
    Ok(())
}

/// Why a proposal raised at `place` is not bound to `task`; `None`: it is.
/// Repositories and worktrees compare as directories
/// ([`crate::same_repository`]), not as strings.
pub fn binding_problem(task: &Task, place: &Place) -> Option<String> {
    let id = &task.id;
    if !same_repository(&task.git_common_dir, Path::new(&place.git_common_dir)) {
        return Some(format!(
            "`{id}` is a task of another repository ({}); a proposal is bound to a task of its \
             own repository ({})",
            task.git_common_dir, place.git_common_dir
        ));
    }
    if task.status.is_closed() {
        return Some(format!(
            "`{id}` is {}: a proposal is bound only to a task that is neither done nor cancelled",
            task.status
        ));
    }
    claimed_elsewhere(task, Path::new(&place.worktree)).map(|claim| {
        format!(
            "`{id}` is claimed in the worktree {}: a proposal bound to it is raised there, not in \
             {}",
            claim.worktree, place.worktree
        )
    })
}

/// The claim of `task` when it names another worktree than `worktree`
/// (compared as directories): the place check of a bound proposal, a
/// report and a completion.
pub fn claimed_elsewhere<'t>(task: &'t Task, worktree: &Path) -> Option<&'t TaskClaim> {
    task.claim
        .as_ref()
        .filter(|claim| !same_dir(Path::new(&claim.worktree), worktree))
}

/// The refresh of [`SqliteQueue::applied_refreshing`] inside its
/// transaction: each snapshot node of `refresh` still at its `was` hash
/// takes its applied text and hash; one `task.refreshed` each, the task's
/// `updated_at` = `now` and its `revision` raised when any. A task that
/// does not read, or is `done` or `cancelled`, refreshes nothing.
pub(crate) fn refresh_snapshot(
    tx: &Transaction<'_>,
    project: &str,
    proposal: &str,
    refresh: &TaskRefresh,
    now: &str,
) -> Result<Vec<String>, QueueError> {
    let task = &refresh.task_id;
    type Stored = (Option<String>, Option<String>, Option<String>);
    let stored: Option<Stored> = tx
        .query_row(
            "SELECT snapshot, status, revision FROM main.tasks WHERE id = ?1 AND project = ?2",
            [task, project],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .db()?;
    let Some((Some(text), Some(status), Some(revision))) = stored else {
        return Ok(Vec::new());
    };
    let open = TaskStatus::parse(&status).is_some_and(|status| !status.is_closed());
    let Some(revision) = revision.parse::<u64>().ok().filter(|_| open) else {
        return Ok(Vec::new());
    };
    let Ok(mut snapshot) = serde_json::from_str::<TaskSnapshot>(&text) else {
        return Ok(Vec::new());
    };
    let mut refreshed = Vec::new();
    for node in &mut snapshot.nodes {
        let Some(entry) = refresh
            .entries
            .iter()
            .find(|entry| entry.id == node.id && entry.was == node.span_hash)
        else {
            continue;
        };
        node.span_hash.clone_from(&entry.span_hash);
        node.text.clone_from(&entry.text);
        refreshed.push(node.id.clone());
    }
    if refreshed.is_empty() {
        return Ok(refreshed);
    }
    tx.execute(
        "UPDATE main.tasks SET snapshot = ?1, updated_at = ?2 WHERE id = ?3 AND project = ?4",
        rusqlite::params![to_json(&snapshot)?, now, task, project],
    )
    .db()?;
    bump(tx, project, task, revision)?;
    for node in &refreshed {
        log(
            tx,
            project,
            EVENT_TASK_REFRESHED,
            &json!({ "id": task, "proposal": proposal, "node": node }),
            now,
        )?;
    }
    Ok(refreshed)
}
