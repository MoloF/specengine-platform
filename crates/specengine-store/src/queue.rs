//! The proposal queue (task spec `proposal-apply`, "Data"): the operational
//! tables `proposals` and `events` in the project's database, beside the
//! index. They are made by the queue's own schema steps on `PRAGMA
//! user_version` (0 → 1; a higher version is a newer build's: refused), in
//! one `Immediate` transaction, and no list of `schema` names them, so an
//! index rebuild or an `INDEX_FORMAT` change never drops them (the index
//! leaves `user_version` alone).
//!
//! - **IDs**: `PR-NNNN`, the highest number in the table plus one, taken in
//!   the inserting transaction; rows are never deleted, so an ID is never
//!   reused.
//! - **States**: `open → approved → applied`, `open → rejected`, and on
//!   the owner's rejection as a compare-and-set on the state read `approved
//!   → rejected` ([`ProposalQueue::reject_from`]; the caller refuses one
//!   whose commit is on its branch);
//!   `approved` only inside an apply or after a crash in it. Every state
//!   change is one `Immediate` transaction holding the change and its
//!   event: `proposal.created`, `.approved`, `.applied` (`commit`),
//!   `.rejected` (`reason`); `.apply_failed` (`step`, `reason`) records an
//!   attempt refused at apply steps 2–10, whatever it does to the state
//!   ([`ProposalQueue::reopen`]). An `open` proposal whose own commit is on
//!   its branch goes to `applied` in one transaction logging `.approved`
//!   then `.applied` ([`ProposalQueue::applied_with`]). Every payload is
//!   JSON with `id`.
//! - **Runs**: an apply changes the state only by compare-and-set on the
//!   state it read ([`Seen`]): step 7 ([`ProposalQueue::approve_from`])
//!   never takes over a proposal another run holds; a refusal reopens only
//!   what the run itself wrote at step 7 ([`ProposalQueue::reopen_from`]),
//!   and before that only logs ([`ProposalQueue::log_failure`]). Step 10's
//!   recording and a completion ([`ProposalQueue::applied_with`],
//!   [`ProposalQueue::applied`]) are deliberately not compare-and-set: the
//!   proposal's commit is in history, a fact whichever run holds the state
//!   or reopened it; only `applied` and `rejected` refuse them
//!   ([`QueueError::Status`]).
//! - **Stored values** are checked when read: a `base_commit` that is no
//!   object ID, a `branch` git would not take as a branch name (or one
//!   starting with `-`), an author field outside its grammar is a corrupt
//!   row, never handed to git: [`ProposalQueue::get`] and
//!   [`ProposalQueue::list`] fail naming it, [`ProposalQueue::list_readable`]
//!   skips it and names it ([`UnreadableRow`]).
//! - **Repository**: a proposal records its git common dir; the queue
//!   stores what it is given and filters by it ([`ProposalFilter`]); the
//!   comparison of places is [`crate::same_repository`].
//! - **Time**: every time stamp is given by the caller (an injected clock),
//!   `YYYY-MM-DDTHH:MM:SSZ`, stored as given.
//!
//! No `rusqlite` type is public (canon `architecture.md#distribution`).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use specengine_core::check::Finding;
use specengine_core::proposal::{
    Author, author_field_problem, is_utc_timestamp, patch_hash_input, proposal_id, proposal_number,
};
use specengine_model::Severity;

use crate::error::{Db, StoreError};
use crate::worktree::is_oid;
use crate::{b3_hash, schema};

/// The `user_version` the queue's steps bring a DB to.
pub const QUEUE_SCHEMA_VERSION: i64 = 1;

/// The apply step whose failure leaves an `approved` proposal `approved`:
/// the verification of a commit that exists ([`ProposalQueue::reopen`]).
pub const APPLY_VERIFY_STEP: u8 = 10;

/// Step 0 → 1: the two tables (the task spec's DDL, `STRICT`).
const STEP_1: &str = "
CREATE TABLE proposals (
  id TEXT PRIMARY KEY,
  project TEXT, kind TEXT, status TEXT,
  target_id TEXT, target_path TEXT,
  git_common_dir TEXT, worktree TEXT, root_rel TEXT,
  branch TEXT, base_commit TEXT,
  base_hash TEXT, base_text TEXT, new_text TEXT,
  patch_hash TEXT,
  rationale TEXT, author TEXT,
  diagnostics TEXT,
  decided_by TEXT, decided_at TEXT, decision_note TEXT, applied_commit TEXT,
  created_at TEXT, updated_at TEXT
) STRICT;
CREATE TABLE events (
  seq INTEGER PRIMARY KEY, project TEXT, type TEXT, payload TEXT, at TEXT
) STRICT;
";

/// Every column of `proposals`, in table order.
const COLUMNS: &str = "id, project, kind, status, target_id, target_path, git_common_dir, \
     worktree, root_rel, branch, base_commit, base_hash, base_text, new_text, patch_hash, \
     rationale, author, diagnostics, decided_by, decided_at, decision_note, applied_commit, \
     created_at, updated_at";

/// The order of proposal IDs: by number (`PR-9999` before `PR-10000`).
const ID_ORDER: &str = "ORDER BY length(id), id";

/// `proposal.created`.
pub const EVENT_CREATED: &str = "proposal.created";
/// `proposal.approved`.
pub const EVENT_APPROVED: &str = "proposal.approved";
/// `proposal.applied`, with `commit`.
pub const EVENT_APPLIED: &str = "proposal.applied";
/// `proposal.rejected`, with `reason`.
pub const EVENT_REJECTED: &str = "proposal.rejected";
/// `proposal.apply_failed`, with `step` and `reason`.
pub const EVENT_APPLY_FAILED: &str = "proposal.apply_failed";

/// What a proposal does; slice 1 has one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalKind {
    /// Replace one node's span.
    Update,
}

impl ProposalKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Update => "update",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        (text == "update").then_some(Self::Update)
    }
}

/// Where a proposal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalStatus {
    Open,
    /// Inside an apply, or after a crash in one.
    Approved,
    Applied,
    Rejected,
}

impl ProposalStatus {
    pub const ALL: [Self; 4] = [Self::Open, Self::Approved, Self::Applied, Self::Rejected];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Approved => "approved",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == text)
    }
}

impl fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a proposal was raised and applies (ADR-0032): all canonical, all
/// recorded at creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Place {
    /// The repository: its git common dir, canonical.
    pub git_common_dir: String,
    /// The worktree's top, canonical.
    pub worktree: String,
    /// The project root inside the worktree, `/`-separated; `""` at its top.
    pub root_rel: String,
    /// The branch `HEAD` named (`symbolic-ref`), without `refs/heads/`.
    pub branch: String,
    /// `HEAD`'s commit at creation, hex.
    pub base_commit: String,
}

/// One introduced finding, as stored in `diagnostics` (05 §7 item 2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProposalFinding {
    pub code: String,
    pub severity: Severity,
    /// Root-relative.
    pub path: String,
    pub line: usize,
    pub subject: String,
    pub message: String,
}

impl From<&Finding> for ProposalFinding {
    fn from(finding: &Finding) -> Self {
        Self {
            code: finding.code.clone(),
            severity: finding.severity,
            path: finding.path.clone(),
            line: finding.line,
            subject: finding.subject.clone(),
            message: finding.message.clone(),
        }
    }
}

/// What `spec propose` stores; the queue adds the ID, project, status and
/// times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProposal {
    pub kind: ProposalKind,
    /// `ID`, or `slug/ID` for a feature-scoped one.
    pub target_id: String,
    /// Root-relative.
    pub target_path: String,
    pub place: Place,
    /// `b3:` of the span's bytes at creation.
    pub base_hash: String,
    /// The span at creation (the merge base of a later apply).
    pub base_text: String,
    /// Its replacement, as spliced.
    pub new_text: String,
    /// [`patch_hash`] of the three above.
    pub patch_hash: String,
    pub rationale: String,
    pub author: Author,
    /// The findings the edit introduces.
    pub diagnostics: Vec<ProposalFinding>,
}

/// A stored proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: String,
    pub project: String,
    pub kind: ProposalKind,
    pub status: ProposalStatus,
    pub target_id: String,
    pub target_path: String,
    pub place: Place,
    pub base_hash: String,
    pub base_text: String,
    pub new_text: String,
    pub patch_hash: String,
    pub rationale: String,
    pub author: Author,
    pub diagnostics: Vec<ProposalFinding>,
    /// The decider's git identity (`Name <email>`), once approved or
    /// rejected.
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    /// `--note` of an approval, `--reason` of a rejection.
    pub decision_note: Option<String>,
    pub applied_commit: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Proposal {
    /// Its state as read now: the key of a compare-and-set change.
    pub fn seen(&self) -> Seen {
        Seen {
            status: self.status,
            updated_at: self.updated_at.clone(),
        }
    }
}

/// The state of a proposal as one run read it, or wrote it at apply step 7:
/// the key of [`ProposalQueue::approve_from`] and
/// [`ProposalQueue::reopen_from`]. Every change sets `updated_at`, and an
/// `approved` → `approved` change only to a later time, so one key names
/// one hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub status: ProposalStatus,
    pub updated_at: String,
}

/// A stored row that does not decode: its ID (as stored), the column and
/// why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableRow {
    pub id: String,
    pub column: String,
    pub reason: String,
}

impl fmt::Display for UnreadableRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "proposal {}: the stored `{}` cannot be read: {}",
            self.id, self.column, self.reason
        )
    }
}

impl From<UnreadableRow> for QueueError {
    fn from(row: UnreadableRow) -> Self {
        Self::Store(StoreError::Sqlite(row.to_string()))
    }
}

/// What [`ProposalQueue::list_readable`] gives: the proposals the filter
/// admits, by ID number, and the rows skipped because they do not decode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProposalList {
    pub proposals: Vec<Proposal>,
    pub unreadable: Vec<UnreadableRow>,
}

/// One event of the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Rising.
    pub seq: i64,
    pub project: String,
    /// [`EVENT_CREATED`], …
    pub event_type: String,
    /// JSON, with `id`.
    pub payload: Value,
    pub at: String,
}

/// An owner's decision: who and the optional note (an approval's `--note`,
/// a rejection's `--reason`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// `Name <email>`.
    pub decided_by: String,
    pub note: Option<String>,
}

/// An apply attempt refused at `step` (2–10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyFailure {
    pub step: u8,
    /// One line.
    pub reason: String,
}

/// Which proposals [`ProposalQueue::list`] gives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProposalFilter {
    /// Only this repository's (the canonical common dir, as stored);
    /// `None`: every repository's.
    pub git_common_dir: Option<String>,
    /// Only these states; empty: all.
    pub statuses: Vec<ProposalStatus>,
}

/// An error of the queue; no `rusqlite` type crosses it.
#[derive(Debug)]
pub enum QueueError {
    /// The database (or opening it) failed.
    Store(StoreError),
    /// The queue's tables were made by a newer build (`user_version`
    /// above [`QUEUE_SCHEMA_VERSION`]): exit 2, nothing changed.
    SchemaTooNew { found: i64 },
    /// No proposal of the handle's project has this ID.
    Unknown { id: String },
    /// The change is not allowed from the proposal's state; an applied
    /// one names its commit.
    Status {
        id: String,
        status: ProposalStatus,
        applied_commit: Option<String>,
    },
    /// A value given to the queue is not one it stores: a time stamp not
    /// `YYYY-MM-DDTHH:MM:SSZ` (or, for an `approved` proposal approved
    /// again, not later than its last change), an apply step outside 2–10.
    Invalid(String),
    /// A compare-and-set found the proposal no longer in the state the run
    /// read: another run approved, reopened, applied or rejected it.
    /// Nothing written.
    Changed {
        id: String,
        status: ProposalStatus,
        updated_at: String,
    },
}

impl From<StoreError> for QueueError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(f),
            Self::SchemaTooNew { found } => write!(
                f,
                "the proposal queue's tables are of a newer SpecEngine (schema version {found}, \
                 this build knows {QUEUE_SCHEMA_VERSION}): upgrade SpecEngine (nothing changed)"
            ),
            Self::Unknown { id } => write!(f, "no proposal `{id}` in this project's queue"),
            Self::Status {
                id,
                status,
                applied_commit: Some(commit),
            } => write!(f, "`{id}` is {status}: commit {commit}"),
            Self::Status { id, status, .. } => write!(f, "`{id}` is {status}"),
            Self::Invalid(message) => f.write_str(message),
            Self::Changed {
                id,
                status,
                updated_at,
            } => write!(
                f,
                "`{id}` changed since this run read it: it is {status} since {updated_at} \
                 (another `spec approve` or `spec reject` holds or decided it)"
            ),
        }
    }
}

impl std::error::Error for QueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

/// `b3:` of `target_id` LF `base_hash` LF `new_text` (07 §1.2).
pub fn patch_hash(target_id: &str, base_hash: &str, new_text: &str) -> String {
    b3_hash(&patch_hash_input(target_id, base_hash, new_text))
}

/// The proposal queue of one project. Every change is one `Immediate`
/// transaction with its event; every read one snapshot.
pub trait ProposalQueue {
    /// Stores `proposal` as `open` under the next ID (highest + 1, taken in
    /// this transaction), with `created_at` = `updated_at` = `now`, and
    /// logs `proposal.created`.
    fn create(&mut self, proposal: &NewProposal, now: &str) -> Result<Proposal, QueueError>;
    /// The project's proposal `id`; `Ok(None)` when there is none.
    fn get(&self, id: &str) -> Result<Option<Proposal>, QueueError>;
    /// The project's proposals `filter` admits, by ID number.
    fn list(&self, filter: &ProposalFilter) -> Result<Vec<Proposal>, QueueError>;
    /// [`Self::list`] that skips a row which does not decode and names it
    /// in [`ProposalList::unreadable`] (a row whose stored state reads and
    /// `filter` excludes is skipped silently).
    fn list_readable(&self, filter: &ProposalFilter) -> Result<ProposalList, QueueError>;
    /// Apply step 7: `open` → `approved` with the decision (`decided_at` =
    /// `now`), logging `proposal.approved`; `approved` stays, the decision
    /// replaced, no event (no state change). Else [`QueueError::Status`].
    fn approve(&mut self, id: &str, decision: &Decision, now: &str)
    -> Result<Proposal, QueueError>;
    /// Apply step 7 as a compare-and-set on `seen`, the state this run
    /// read: `open` → `approved` (logging `proposal.approved`), or
    /// `approved` → `approved` with the decision replaced (no event) when
    /// `now` is later than `seen.updated_at` (else [`QueueError::Invalid`]).
    /// The stored state not `seen`: [`QueueError::Changed`], nothing
    /// written; `applied`, `rejected`: [`QueueError::Status`].
    fn approve_from(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// Step 10 passed: `approved` → `applied` with `applied_commit`,
    /// logging `proposal.applied` (`commit`). Else [`QueueError::Status`].
    fn applied(&mut self, id: &str, commit: &str, now: &str) -> Result<Proposal, QueueError>;
    /// [`Self::applied`] that also takes an `open` proposal, for a run
    /// whose owner consented and whose own commit is on the branch (a
    /// completion of an `open` proposal, or step 10 after its hold was
    /// reopened meanwhile): `open` → `applied` with `decision` (`decided_at`
    /// = `now`) and `applied_commit`, logging `proposal.approved` then
    /// `proposal.applied` (`commit`); `approved` → `applied`, its decision
    /// kept. `applied`, `rejected`: [`QueueError::Status`].
    fn applied_with(
        &mut self,
        id: &str,
        commit: &str,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// An attempt refused at `failure.step`: logs one
    /// `proposal.apply_failed` (`step`, `reason`); an `approved` proposal
    /// goes back to `open`, its decision cleared, unless the step is
    /// [`APPLY_VERIFY_STEP`] (a commit exists: it stays `approved`); an
    /// `open` one stays. `applied`, `rejected`: [`QueueError::Status`],
    /// nothing logged.
    fn reopen(
        &mut self,
        id: &str,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// [`Self::reopen`] for a run that holds `held` (the state it read, or
    /// wrote at step 7): one `proposal.apply_failed` logged; the state
    /// changes only while it is still `held`, so a proposal another run
    /// holds (or reopened) is left as it is. `applied`, `rejected`:
    /// [`QueueError::Status`], nothing logged.
    fn reopen_from(
        &mut self,
        id: &str,
        held: &Seen,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// A refusal at `failure.step` of a run that holds nothing (refused
    /// before its own step 7 wrote `approved`): one `proposal.apply_failed`
    /// logged, the state left as it is (`open`, or `approved` that a stopped
    /// or another live run holds). `applied`, `rejected`:
    /// [`QueueError::Status`], nothing logged.
    fn log_failure(
        &mut self,
        id: &str,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// `open` → `rejected` with the decision (its note the reason),
    /// logging `proposal.rejected` (`reason`). Else
    /// [`QueueError::Status`].
    fn reject(&mut self, id: &str, decision: &Decision, now: &str) -> Result<Proposal, QueueError>;
    /// The owner's rejection of an `open` or `approved` proposal (the
    /// caller checks that no commit of it is on its branch): → `rejected`
    /// with the decision, logging `proposal.rejected` (`reason`), as a
    /// compare-and-set on `seen`, the state the caller read (else
    /// [`QueueError::Changed`], nothing written); `applied`, `rejected`:
    /// [`QueueError::Status`].
    fn reject_from(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// The rejection of an orphan (a proposal whose repository no longer
    /// exists; the caller decides): [`Self::reject_from`].
    fn reject_orphan(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// The project's events, by `seq`.
    fn events(&self) -> Result<Vec<Event>, QueueError>;
}

/// The queue in the project's SQLite database (the index's file).
pub struct SqliteQueue {
    conn: Connection,
    project: String,
}

impl SqliteQueue {
    /// Opens (creating when absent) the database `db` and brings the
    /// queue's tables to [`QUEUE_SCHEMA_VERSION`]. The directory must
    /// exist ([`StoreError::DbDirMissing`]); a new file gets the index's
    /// creation PRAGMAs, every connection its settings. A `user_version`
    /// above this build's is [`QueueError::SchemaTooNew`].
    pub fn open(db: impl AsRef<Path>, project: &str) -> Result<Self, QueueError> {
        let db = db.as_ref();
        let parent = match db.parent() {
            Some(parent) if parent.as_os_str().is_empty() => Path::new("."),
            Some(parent) => parent,
            None => {
                return Err(StoreError::io(
                    db,
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "the database path names no file",
                    ),
                )
                .into());
            }
        };
        if !fs::metadata(parent).is_ok_and(|meta| meta.is_dir()) {
            return Err(StoreError::DbDirMissing {
                dir: parent.to_path_buf(),
            }
            .into());
        }
        let mut conn = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .db()?;
        schema::configure(&conn)?;
        // The queue is the only copy of what the owner decided: every
        // commit synced, not only the WAL checkpoints (the index's
        // `NORMAL` loses at most a cache).
        conn.pragma_update(None, "synchronous", "FULL").db()?;
        migrate(&mut conn)?;
        Ok(Self {
            conn,
            project: project.to_owned(),
        })
    }

    /// The project the handle is bound to.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// The canonical dump of the queue's tables, every project's: one line
    /// per row, `<table>\t<JSON array of every column>`, `proposals` by ID
    /// number, then `events` by `seq`. Equal dumps: the same queue.
    pub fn dump(&self) -> Result<String, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let mut out = String::new();
        for (table, order) in [("proposals", ID_ORDER), ("events", "ORDER BY seq")] {
            let mut statement = tx
                .prepare(&format!("SELECT * FROM main.{table} {order}"))
                .db()?;
            let width = statement.column_count();
            let mut rows = statement.query([]).db()?;
            while let Some(row) = rows.next().db()? {
                let mut values = Vec::with_capacity(width);
                for column in 0..width {
                    values.push(match row.get_ref(column).db()? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(number) => Value::from(number),
                        ValueRef::Real(number) => Value::from(number),
                        ValueRef::Text(text) => {
                            Value::String(String::from_utf8_lossy(text).into_owned())
                        }
                        ValueRef::Blob(bytes) => Value::String(format!("blob:{}", b3_hash(bytes))),
                    });
                }
                out.push_str(table);
                out.push('\t');
                out.push_str(&Value::Array(values).to_string());
                out.push('\n');
            }
        }
        tx.commit().db()?;
        Ok(out)
    }

    fn write(&mut self) -> Result<Transaction<'_>, QueueError> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .db()?)
    }

    /// The project's rows `filter`'s repository admits, by ID number, in
    /// one snapshot, not decoded.
    fn raw_rows(&self, filter: &ProposalFilter) -> Result<Vec<RawRow>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let mut statement = tx
            .prepare(&format!(
                "SELECT {COLUMNS} FROM main.proposals \
                 WHERE project = ?1 AND (?2 IS NULL OR git_common_dir = ?2) {ID_ORDER}"
            ))
            .db()?;
        let raws: Vec<RawRow> = statement
            .query_map(
                rusqlite::params![self.project, filter.git_common_dir],
                RawRow::read,
            )
            .db()?
            .collect::<rusqlite::Result<_>>()
            .db()?;
        drop(statement);
        tx.commit().db()?;
        Ok(raws)
    }

    /// [`ProposalQueue::approve`] (`seen` `None`) and
    /// [`ProposalQueue::approve_from`].
    fn approve_if(
        &mut self,
        id: &str,
        seen: Option<&Seen>,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        match current.status {
            ProposalStatus::Open | ProposalStatus::Approved => {}
            _ => return Err(status_error(current)),
        }
        if let Some(seen) = seen {
            if current.seen() != *seen {
                return Err(changed(current));
            }
            if current.status == ProposalStatus::Approved && now <= current.updated_at.as_str() {
                return Err(QueueError::Invalid(format!(
                    "`{id}` was approved at {}, not before now ({now}): approve it again in a \
                     moment",
                    current.updated_at
                )));
            }
        }
        tx.execute(
            "UPDATE main.proposals SET status = ?1, decided_by = ?2, decided_at = ?3, \
             decision_note = ?4, updated_at = ?3 WHERE id = ?5 AND project = ?6",
            rusqlite::params![
                ProposalStatus::Approved.as_str(),
                decision.decided_by,
                now,
                decision.note,
                id,
                project
            ],
        )
        .db()?;
        if current.status == ProposalStatus::Open {
            log(&tx, &project, EVENT_APPROVED, &json!({ "id": id }), now)?;
        }
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// [`ProposalQueue::reject`] (`seen` `None`: from `open` only) and
    /// [`ProposalQueue::reject_from`] (from `open` or `approved`, the state
    /// read).
    fn reject_if(
        &mut self,
        id: &str,
        seen: Option<&Seen>,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        match (current.status, seen) {
            (ProposalStatus::Open, _) | (ProposalStatus::Approved, Some(_)) => {}
            _ => return Err(status_error(current)),
        }
        if seen.is_some_and(|seen| current.seen() != *seen) {
            return Err(changed(current));
        }
        tx.execute(
            "UPDATE main.proposals SET status = ?1, decided_by = ?2, decided_at = ?3, \
             decision_note = ?4, updated_at = ?3 WHERE id = ?5 AND project = ?6",
            rusqlite::params![
                ProposalStatus::Rejected.as_str(),
                decision.decided_by,
                now,
                decision.note,
                id,
                project
            ],
        )
        .db()?;
        log(
            &tx,
            &project,
            EVENT_REJECTED,
            &json!({ "id": id, "reason": decision.note }),
            now,
        )?;
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// [`ProposalQueue::applied`] (`decision` `None`: from `approved` only)
    /// and [`ProposalQueue::applied_with`] (also from `open`, deciding).
    fn applied_if(
        &mut self,
        id: &str,
        commit: &str,
        decision: Option<&Decision>,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        match (current.status, decision) {
            (ProposalStatus::Approved, _) => {}
            (ProposalStatus::Open, Some(decision)) => {
                tx.execute(
                    "UPDATE main.proposals SET decided_by = ?1, decided_at = ?2, \
                     decision_note = ?3 WHERE id = ?4 AND project = ?5",
                    rusqlite::params![decision.decided_by, now, decision.note, id, project],
                )
                .db()?;
                log(&tx, &project, EVENT_APPROVED, &json!({ "id": id }), now)?;
            }
            _ => return Err(status_error(current)),
        }
        tx.execute(
            "UPDATE main.proposals SET status = ?1, applied_commit = ?2, updated_at = ?3 \
             WHERE id = ?4 AND project = ?5",
            [
                ProposalStatus::Applied.as_str(),
                commit,
                now,
                id,
                project.as_str(),
            ],
        )
        .db()?;
        log(
            &tx,
            &project,
            EVENT_APPLIED,
            &json!({ "id": id, "commit": commit }),
            now,
        )?;
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// [`ProposalQueue::reopen`] ([`Hold::Any`]),
    /// [`ProposalQueue::reopen_from`] ([`Hold::Held`]) and
    /// [`ProposalQueue::log_failure`] ([`Hold::Nothing`]).
    fn reopen_if(
        &mut self,
        id: &str,
        hold: Hold<'_>,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        if !(2..=APPLY_VERIFY_STEP).contains(&failure.step) {
            return Err(QueueError::Invalid(format!(
                "apply step {} is not one of 2-{APPLY_VERIFY_STEP}",
                failure.step
            )));
        }
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        let holds = match hold {
            Hold::Any => true,
            Hold::Held(held) => current.seen() == *held,
            Hold::Nothing => false,
        };
        match current.status {
            ProposalStatus::Open => {}
            ProposalStatus::Approved if failure.step == APPLY_VERIFY_STEP || !holds => {}
            ProposalStatus::Approved => {
                tx.execute(
                    "UPDATE main.proposals SET status = ?1, decided_by = NULL, \
                     decided_at = NULL, decision_note = NULL, updated_at = ?2 \
                     WHERE id = ?3 AND project = ?4",
                    [ProposalStatus::Open.as_str(), now, id, project.as_str()],
                )
                .db()?;
            }
            ProposalStatus::Applied | ProposalStatus::Rejected => {
                return Err(status_error(current));
            }
        }
        log(
            &tx,
            &project,
            EVENT_APPLY_FAILED,
            &json!({ "id": id, "step": failure.step, "reason": failure.reason }),
            now,
        )?;
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok(stored)
    }
}

/// Which `approved` proposal a refused run may reopen.
#[derive(Clone, Copy)]
enum Hold<'a> {
    /// Any ([`ProposalQueue::reopen`]).
    Any,
    /// Only one still in the state the run wrote.
    Held(&'a Seen),
    /// None: the run wrote nothing.
    Nothing,
}

/// The queue's schema steps, in one `Immediate` transaction: 0 → 1 makes
/// the tables; [`QUEUE_SCHEMA_VERSION`] is left alone; a higher version is
/// refused.
fn migrate(conn: &mut Connection) -> Result<(), QueueError> {
    let version = user_version(conn)?;
    if version > QUEUE_SCHEMA_VERSION {
        return Err(QueueError::SchemaTooNew { found: version });
    }
    if version == QUEUE_SCHEMA_VERSION {
        return Ok(());
    }
    schema::prepare_new(conn)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .db()?;
    // Read again under the write lock: another process may have stepped.
    let mut version = user_version(&tx)?;
    if version > QUEUE_SCHEMA_VERSION {
        return Err(QueueError::SchemaTooNew { found: version });
    }
    if version == 0 {
        tx.execute_batch(STEP_1).db()?;
        version = 1;
    }
    tx.pragma_update(None, "user_version", version).db()?;
    tx.commit().db()?;
    Ok(())
}

fn user_version(conn: &Connection) -> Result<i64, QueueError> {
    Ok(conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .db()?)
}

fn check_time(now: &str) -> Result<(), QueueError> {
    if is_utc_timestamp(now) {
        Ok(())
    } else {
        Err(QueueError::Invalid(format!(
            "`{now}` is no UTC time stamp `YYYY-MM-DDTHH:MM:SSZ`"
        )))
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String, QueueError> {
    serde_json::to_string(value)
        .map_err(|error| QueueError::Store(StoreError::Sqlite(format!("encoding JSON: {error}"))))
}

/// A stored value that does not decode.
fn corrupt(id: &str, column: &str, message: impl fmt::Display) -> UnreadableRow {
    UnreadableRow {
        id: id.to_owned(),
        column: column.to_owned(),
        reason: message.to_string(),
    }
}

/// `name` can be the branch a proposal applies on: a name `git
/// check-ref-format --branch` takes (no control character, space, `~ ^ :
/// ? * [ \`, `..`, `@{`, `//`; no component starting with `.` or ending
/// with `.lock`; not starting or ending with `/`, not ending with `.`; not
/// `@` or `HEAD`), not starting with `-`.
fn is_branch_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['-', '/'])
        && !name.ends_with(['/', '.'])
        && name != "@"
        && name != "HEAD"
        && !name.contains("..")
        && !name.contains("@{")
        && !name.contains("//")
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
        && name
            .split('/')
            .all(|part| !part.starts_with('.') && !part.ends_with(".lock"))
}

/// The row's columns, in [`COLUMNS`] order, before decoding.
struct RawRow {
    text: Vec<Option<String>>,
}

impl RawRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        let mut text = Vec::with_capacity(24);
        for column in 0..24 {
            text.push(row.get::<_, Option<String>>(column)?);
        }
        Ok(Self { text })
    }

    /// The stored state, when it reads: `list_readable`'s filter before
    /// decoding.
    fn status(&self) -> Option<ProposalStatus> {
        self.text[3].as_deref().and_then(ProposalStatus::parse)
    }

    fn decode(mut self) -> Result<Proposal, UnreadableRow> {
        let mut take = |index: usize| self.text[index].take();
        let id = take(0).unwrap_or_default();
        let required = |value: Option<String>, column: &str| {
            value.ok_or_else(|| corrupt(&id, column, "it is NULL"))
        };
        let project = required(take(1), "project")?;
        let kind_text = required(take(2), "kind")?;
        let kind = ProposalKind::parse(&kind_text)
            .ok_or_else(|| corrupt(&id, "kind", format!("`{kind_text}` is no kind")))?;
        let status_text = required(take(3), "status")?;
        let status = ProposalStatus::parse(&status_text)
            .ok_or_else(|| corrupt(&id, "status", format!("`{status_text}` is no state")))?;
        let target_id = required(take(4), "target_id")?;
        let target_path = required(take(5), "target_path")?;
        let place = Place {
            git_common_dir: required(take(6), "git_common_dir")?,
            worktree: required(take(7), "worktree")?,
            root_rel: required(take(8), "root_rel")?,
            branch: required(take(9), "branch")?,
            base_commit: required(take(10), "base_commit")?,
        };
        if !is_branch_name(&place.branch) {
            return Err(corrupt(
                &id,
                "branch",
                format!("{:?} is no branch name", place.branch),
            ));
        }
        if !is_oid(place.base_commit.as_bytes()) {
            return Err(corrupt(
                &id,
                "base_commit",
                format!("{:?} is no object ID", place.base_commit),
            ));
        }
        let base_hash = required(take(11), "base_hash")?;
        let base_text = required(take(12), "base_text")?;
        let new_text = required(take(13), "new_text")?;
        let patch_hash = required(take(14), "patch_hash")?;
        let rationale = required(take(15), "rationale")?;
        let author_text = required(take(16), "author")?;
        let author: Author =
            serde_json::from_str(&author_text).map_err(|error| corrupt(&id, "author", error))?;
        for field in [&author.role, &author.model, &author.run] {
            if let Some(problem) = field.as_deref().and_then(author_field_problem) {
                return Err(corrupt(&id, "author", problem));
            }
        }
        let diagnostics_text = required(take(17), "diagnostics")?;
        let diagnostics: Vec<ProposalFinding> = serde_json::from_str(&diagnostics_text)
            .map_err(|error| corrupt(&id, "diagnostics", error))?;
        let decided_by = take(18);
        let decided_at = take(19);
        let decision_note = take(20);
        let applied_commit = take(21);
        let created_at = required(take(22), "created_at")?;
        let updated_at = required(take(23), "updated_at")?;
        Ok(Proposal {
            id,
            project,
            kind,
            status,
            target_id,
            target_path,
            place,
            base_hash,
            base_text,
            new_text,
            patch_hash,
            rationale,
            author,
            diagnostics,
            decided_by,
            decided_at,
            decision_note,
            applied_commit,
            created_at,
            updated_at,
        })
    }
}

fn select_one(conn: &Connection, project: &str, id: &str) -> Result<Option<Proposal>, QueueError> {
    let raw = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM main.proposals WHERE id = ?1 AND project = ?2"),
            [id, project],
            RawRow::read,
        )
        .optional()
        .db()?;
    Ok(raw.map(RawRow::decode).transpose()?)
}

/// The proposal `id` inside a write transaction, or [`QueueError::Unknown`].
fn existing(tx: &Transaction<'_>, project: &str, id: &str) -> Result<Proposal, QueueError> {
    select_one(tx, project, id)?.ok_or_else(|| QueueError::Unknown { id: id.to_owned() })
}

fn status_error(proposal: Proposal) -> QueueError {
    QueueError::Status {
        id: proposal.id,
        status: proposal.status,
        applied_commit: proposal.applied_commit,
    }
}

fn changed(proposal: Proposal) -> QueueError {
    QueueError::Changed {
        id: proposal.id,
        status: proposal.status,
        updated_at: proposal.updated_at,
    }
}

fn log(
    tx: &Transaction<'_>,
    project: &str,
    event_type: &str,
    payload: &Value,
    at: &str,
) -> Result<(), QueueError> {
    tx.execute(
        "INSERT INTO main.events (project, type, payload, at) VALUES (?1, ?2, ?3, ?4)",
        [project, event_type, &payload.to_string(), at],
    )
    .db()?;
    Ok(())
}

/// The next ID: the highest number of any stored ID, plus one.
fn next_id(tx: &Transaction<'_>) -> Result<String, QueueError> {
    let mut statement = tx.prepare("SELECT id FROM main.proposals").db()?;
    let mut rows = statement.query([]).db()?;
    let mut highest = 0;
    while let Some(row) = rows.next().db()? {
        let id: Option<String> = row.get(0).db()?;
        if let Some(number) = id.as_deref().and_then(proposal_number) {
            highest = highest.max(number);
        }
    }
    let next = highest.checked_add(1).ok_or_else(|| {
        QueueError::Store(StoreError::Sqlite(format!(
            "the queue holds `{}`: no proposal ID follows it",
            proposal_id(highest)
        )))
    })?;
    Ok(proposal_id(next))
}

impl ProposalQueue for SqliteQueue {
    fn create(&mut self, proposal: &NewProposal, now: &str) -> Result<Proposal, QueueError> {
        check_time(now)?;
        let author = to_json(&proposal.author)?;
        let diagnostics = to_json(&proposal.diagnostics)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let id = next_id(&tx)?;
        let place = &proposal.place;
        tx.execute(
            &format!(
                "INSERT INTO main.proposals ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
                 ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, NULL, NULL, NULL, NULL, ?19, \
                 ?19)"
            ),
            [
                id.as_str(),
                &project,
                proposal.kind.as_str(),
                ProposalStatus::Open.as_str(),
                &proposal.target_id,
                &proposal.target_path,
                &place.git_common_dir,
                &place.worktree,
                &place.root_rel,
                &place.branch,
                &place.base_commit,
                &proposal.base_hash,
                &proposal.base_text,
                &proposal.new_text,
                &proposal.patch_hash,
                &proposal.rationale,
                &author,
                &diagnostics,
                now,
            ],
        )
        .db()?;
        log(&tx, &project, EVENT_CREATED, &json!({ "id": id }), now)?;
        let stored = existing(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    fn get(&self, id: &str) -> Result<Option<Proposal>, QueueError> {
        select_one(&self.conn, &self.project, id)
    }

    fn list(&self, filter: &ProposalFilter) -> Result<Vec<Proposal>, QueueError> {
        let mut proposals = Vec::new();
        for raw in self.raw_rows(filter)? {
            let proposal = raw.decode()?;
            if filter.statuses.is_empty() || filter.statuses.contains(&proposal.status) {
                proposals.push(proposal);
            }
        }
        Ok(proposals)
    }

    fn list_readable(&self, filter: &ProposalFilter) -> Result<ProposalList, QueueError> {
        let admits = |status: ProposalStatus| {
            filter.statuses.is_empty() || filter.statuses.contains(&status)
        };
        let mut list = ProposalList::default();
        for raw in self.raw_rows(filter)? {
            if raw.status().is_some_and(|status| !admits(status)) {
                continue;
            }
            match raw.decode() {
                Ok(proposal) => list.proposals.push(proposal),
                Err(row) => list.unreadable.push(row),
            }
        }
        Ok(list)
    }

    fn approve(
        &mut self,
        id: &str,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.approve_if(id, None, decision, now)
    }

    fn approve_from(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.approve_if(id, Some(seen), decision, now)
    }

    fn applied(&mut self, id: &str, commit: &str, now: &str) -> Result<Proposal, QueueError> {
        self.applied_if(id, commit, None, now)
    }

    fn applied_with(
        &mut self,
        id: &str,
        commit: &str,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.applied_if(id, commit, Some(decision), now)
    }

    fn reopen(
        &mut self,
        id: &str,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.reopen_if(id, Hold::Any, failure, now)
    }

    fn reopen_from(
        &mut self,
        id: &str,
        held: &Seen,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.reopen_if(id, Hold::Held(held), failure, now)
    }

    fn log_failure(
        &mut self,
        id: &str,
        failure: &ApplyFailure,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.reopen_if(id, Hold::Nothing, failure, now)
    }

    fn reject(&mut self, id: &str, decision: &Decision, now: &str) -> Result<Proposal, QueueError> {
        self.reject_if(id, None, decision, now)
    }

    fn reject_from(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.reject_if(id, Some(seen), decision, now)
    }

    fn reject_orphan(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.reject_from(id, seen, decision, now)
    }

    fn events(&self) -> Result<Vec<Event>, QueueError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT seq, project, type, payload, at FROM main.events \
                 WHERE project = ?1 ORDER BY seq",
            )
            .db()?;
        let rows: Vec<EventRow> = statement
            .query_map([&self.project], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .db()?
            .collect::<rusqlite::Result<_>>()
            .db()?;
        rows.into_iter()
            .map(|(seq, project, event_type, payload, at)| {
                let broken = |column: &str| {
                    QueueError::Store(StoreError::Sqlite(format!(
                        "event {seq}: the stored `{column}` cannot be read"
                    )))
                };
                let payload = payload.ok_or_else(|| broken("payload"))?;
                Ok(Event {
                    seq,
                    project: project.ok_or_else(|| broken("project"))?,
                    event_type: event_type.ok_or_else(|| broken("type"))?,
                    payload: serde_json::from_str(&payload).map_err(|_| broken("payload"))?,
                    at: at.ok_or_else(|| broken("at"))?,
                })
            })
            .collect()
    }
}

/// An `events` row before decoding: `seq`, `project`, `type`, `payload`,
/// `at`.
type EventRow = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// The project and the database file, for messages.
impl fmt::Debug for SqliteQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteQueue")
            .field("project", &self.project)
            .field("db", &self.conn.path().map(PathBuf::from))
            .finish()
    }
}
