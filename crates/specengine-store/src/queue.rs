//! The proposal queue (task spec `proposal-apply`, "Data"): the operational
//! tables `proposals` and `events` in the project's database, beside the
//! index. They are made by the queue's own schema steps on `PRAGMA
//! user_version` (0 → 1 → 2 → 3 → 4 → 5; a higher version is a newer
//! build's: refused),
//! in one `Immediate` transaction, and no list of `schema` names them, so an
//! index rebuild or an `INDEX_FORMAT` change never drops them (the index
//! leaves `user_version` alone).
//!
//! - **Kinds** (`docs/canon/agent-intake.md` "Stored"): `update` applies; a
//!   `question` and a `discrepancy` decide: their fields live in the eleven
//!   columns step 2 adds ([`Intake`]), the update's five text columns NULL.
//!   They are stored by [`ProposalQueue::create_intake`], whose dedup reads
//!   the queue inside the inserting transaction. A `create` (task spec
//!   `proposal-kinds`) applies as an update does: a new file (its base
//!   `NULL`) or new `{#ID}` sections in a node's span; its targets and new
//!   IDs live in `target_ids` (no new column, no schema step), and
//!   [`ProposalQueue::create`] stores it only when no live (`open`,
//!   `approved`) create of the project, any repository, holds one of its
//!   new IDs, checked inside the inserting transaction
//!   ([`QueueError::Reserved`]; [`ProposalQueue::reserved`] reads them).
//! - **Records** (`docs/canon/decision-record.md` "Queue and documents"): a deciding kind is
//!   approved only with its decision record ([`DecisionRecord`], the five
//!   columns step 3 adds), by [`ProposalQueue::approve_record_from`], which
//!   issues the record's ID under the write lock ([`RecordSeries`]: one
//!   more than the highest of the corpus and of every ID the queue issued;
//!   a proposal keeps its own across a reopen); [`ProposalQueue::approve`],
//!   [`ProposalQueue::approve_from`] refuse it, [`ProposalQueue::applied`]
//!   and [`ProposalQueue::applied_with`] refuse one without its record. The
//!   record is replaced by a later approval and kept by a reopen.
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
//! - **Backup** (`docs/canon/queue-backup.md` "Store", `queue/state.rs`;
//!   format 2: `docs/canon/tasks.md` "Backup"): every row of the four
//!   tables read as stored in one snapshot ([`SqliteQueue::stored_rows`]),
//!   and inserted as given into an empty queue ([`SqliteQueue::restore`]).
//! - **Tasks** (`docs/canon/tasks.md` "Store", `queue/tasks.rs`): step 4
//!   adds `tasks`, `runs` and a proposal's `task_id`; a proposal binds to a
//!   task at creation ([`ProposalQueue::create_with_task`],
//!   [`ProposalQueue::create_intake_with_task`]), checked inside the
//!   inserting transaction; an applied task-bound proposal refreshes the
//!   task's snapshot in its recording transaction
//!   ([`SqliteQueue::applied_refreshing`]).
//! - **Stages** (`docs/canon/decision-staging.md` "Queue",
//!   `queue/stage.rs`): step 5 adds `staged`, `staged_at`; an `open`
//!   proposal's one replaceable staged choice ([`Stage`]), set and cleared
//!   by [`ProposalQueue::stage_from`] and [`ProposalQueue::unstage_from`]
//!   as a compare-and-set on [`Seen`], which holds the stage read: every
//!   compare-and-set op compares it too. Every op leaving `open` clears
//!   both columns; `.approved` and `.rejected` carry `staged_at` when the
//!   decision confirms the stage ([`Decision::staged_at`]).
//!
//! No `rusqlite` type is public (canon `architecture.md#distribution`).

mod stage;
mod state;
mod tasks;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use specengine_core::check::Finding;
use specengine_core::intake::{
    CREATE_KIND, DISCREPANCY_KIND, Evidence, GapType, IntakeOption, IntakeSeverity, QUESTION_KIND,
    UPDATE_KIND, normalized_summary,
};
use specengine_core::is_clean_relative;
use specengine_core::proposal::{
    Author, author_field_problem, is_utc_timestamp, patch_hash_input, proposal_id, proposal_number,
};
pub use specengine_core::record::Choice;
use specengine_core::record::record_id;
use specengine_model::Severity;

use crate::error::{Db, StoreError};
use crate::worktree::is_oid;
use crate::{b3_hash, schema};

pub use stage::{EVENT_STAGED, EVENT_UNSTAGED, Stage, StagedChoice};
pub use state::{
    EVENT_COLUMNS, PROPOSAL_COLUMNS, QueueCounts, RUN_COLUMNS, Restore, StoredEvent,
    StoredProposal, StoredQueue, StoredRun, StoredTask, TASK_COLUMNS, proposal_columns,
};
pub use tasks::{
    EVENT_TASK_APPROVED, EVENT_TASK_CANCELLED, EVENT_TASK_CHANGES_REQUESTED, EVENT_TASK_CLAIMED,
    EVENT_TASK_COMPLETED, EVENT_TASK_CREATED, EVENT_TASK_PLANNED, EVENT_TASK_REFRESHED,
    EVENT_TASK_RUN_REPORTED, NewTask, RefreshEntry, Run, SnapshotEntry, StoredNote, Task,
    TaskChange, TaskList, TaskRefresh, TaskSeen, TaskSnapshot, UnreadableTask, binding_problem,
    claimed_elsewhere, task_event,
};

/// The `user_version` the queue's steps bring a DB to.
pub const QUEUE_SCHEMA_VERSION: i64 = 5;

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

/// Step 1 → 2: the intake columns, appended in this order
/// (`docs/canon/agent-intake.md` "Stored").
const STEP_2: &str = "
ALTER TABLE proposals ADD COLUMN target_ids TEXT;
ALTER TABLE proposals ADD COLUMN severity TEXT;
ALTER TABLE proposals ADD COLUMN gap_type TEXT;
ALTER TABLE proposals ADD COLUMN summary TEXT;
ALTER TABLE proposals ADD COLUMN working_answer TEXT;
ALTER TABLE proposals ADD COLUMN price_of_other TEXT;
ALTER TABLE proposals ADD COLUMN evidence TEXT;
ALTER TABLE proposals ADD COLUMN options TEXT;
ALTER TABLE proposals ADD COLUMN recommendation TEXT;
ALTER TABLE proposals ADD COLUMN distinct_from TEXT;
ALTER TABLE proposals ADD COLUMN linked TEXT;
";

/// Step 2 → 3: a decision record's columns, appended in this order
/// (`docs/canon/decision-record.md` "Queue and documents").
const STEP_3: &str = "
ALTER TABLE proposals ADD COLUMN record_id TEXT;
ALTER TABLE proposals ADD COLUMN record_path TEXT;
ALTER TABLE proposals ADD COLUMN record_title TEXT;
ALTER TABLE proposals ADD COLUMN record_text TEXT;
ALTER TABLE proposals ADD COLUMN choice TEXT;
";

/// Step 3 → 4: the tasks and their runs (`docs/canon/tasks.md` "Store"),
/// and a proposal's task.
const STEP_4: &str = "
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  project TEXT, git_common_dir TEXT, status TEXT,
  title TEXT, goal TEXT, targets TEXT, plan TEXT, criteria TEXT, affected_nodes TEXT,
  owner_notes TEXT, snapshot TEXT, claim TEXT, author TEXT,
  created_at TEXT, updated_at TEXT, revision TEXT
) STRICT;
CREATE TABLE runs (
  task_id TEXT, run INTEGER, role TEXT, worktree TEXT, branch TEXT, author TEXT,
  started_at TEXT, ended_at TEXT, outcome TEXT, summary TEXT, changed_files TEXT,
  PRIMARY KEY (task_id, run)
) STRICT;
ALTER TABLE proposals ADD COLUMN task_id TEXT;
";

/// Step 4 → 5: a proposal's staged choice and its time
/// (`docs/canon/decision-staging.md` "Queue"), appended in this order.
const STEP_5: &str = "
ALTER TABLE proposals ADD COLUMN staged TEXT;
ALTER TABLE proposals ADD COLUMN staged_at TEXT;
";

/// The record columns, in table order.
const RECORD_COLUMNS: [&str; 5] = [
    "record_id",
    "record_path",
    "record_title",
    "record_text",
    "choice",
];

/// Every column of `proposals`, in table order.
const COLUMNS: &str = "id, project, kind, status, target_id, target_path, git_common_dir, \
     worktree, root_rel, branch, base_commit, base_hash, base_text, new_text, patch_hash, \
     rationale, author, diagnostics, decided_by, decided_at, decision_note, applied_commit, \
     created_at, updated_at, target_ids, severity, gap_type, summary, working_answer, \
     price_of_other, evidence, options, recommendation, distinct_from, linked, record_id, \
     record_path, record_title, record_text, choice, task_id, staged, staged_at";

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

/// What a proposal does: `update` and `create` apply (a file written and
/// committed); the intake kinds decide (an approval writes a decision
/// record), or are settled by a rejection whose reason is the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalKind {
    /// Replace one node's span.
    Update,
    /// A question to the owner, with the agent's working answer.
    Question,
    /// A discrepancy with evidence and priced options.
    Discrepancy,
    /// Add a node: a new spec file, or new `{#ID}` sections in a node's
    /// span.
    Create,
}

impl ProposalKind {
    pub const ALL: [Self; 4] = [
        Self::Update,
        Self::Question,
        Self::Discrepancy,
        Self::Create,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Update => UPDATE_KIND,
            Self::Question => QUESTION_KIND,
            Self::Discrepancy => DISCREPANCY_KIND,
            Self::Create => CREATE_KIND,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }

    /// An `update` replaces a node's span; a `create` writes a new file or
    /// a span with new sections: both are applied as one commit.
    pub const fn applies(self) -> bool {
        matches!(self, Self::Update | Self::Create)
    }

    /// A question or a discrepancy: its approval writes a decision record.
    pub const fn decides(self) -> bool {
        matches!(self, Self::Question | Self::Discrepancy)
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
    /// [`ProposalKind::Update`] or [`ProposalKind::Create`].
    pub kind: ProposalKind,
    /// `ID`, or `slug/ID` for a feature-scoped one; a document without an
    /// `id:` by its path. A create's new file: its `id:`, else its path.
    pub target_id: String,
    /// Root-relative.
    pub target_path: String,
    pub place: Place,
    /// `b3:` of the span's bytes at creation; `None` only for a create's
    /// new file (with `base_text`).
    pub base_hash: Option<String>,
    /// The span at creation (the merge base of a later apply); `None` only
    /// for a create's new file.
    pub base_text: Option<String>,
    /// Its replacement, as spliced; a new file's bytes.
    pub new_text: String,
    /// [`patch_hash`] of the three above (an absent base as `""`).
    pub patch_hash: String,
    pub rationale: String,
    pub author: Author,
    /// The findings the edit introduces.
    pub diagnostics: Vec<ProposalFinding>,
    /// A create's new IDs, canonical (`slug/ID` for a feature-scoped one),
    /// in text order: a new file's `id:` first when it declares one; empty
    /// for an update.
    pub new_ids: Vec<String>,
}

/// The fields of a `question` or a `discrepancy`
/// (`docs/canon/agent-intake.md` "Stored").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intake {
    /// The canonical IDs, in the order given; `target_id` is the first and
    /// `target_path` its holder.
    pub target_ids: Vec<String>,
    pub severity: IntakeSeverity,
    /// A discrepancy's.
    pub gap_type: Option<GapType>,
    /// A question's text, a discrepancy's summary.
    pub summary: String,
    /// A question's; a discrepancy's when given.
    pub working_answer: Option<String>,
    /// A question's.
    pub price_of_other: Option<String>,
    /// A discrepancy's; `[]` for a question.
    pub evidence: Vec<Evidence>,
    /// A discrepancy's; `[]` for a question.
    pub options: Vec<IntakeOption>,
    /// A discrepancy's: an index into `options`.
    pub recommendation: Option<u64>,
    /// The hits the author declared this item distinct from.
    pub distinct_from: Vec<String>,
}

/// What an intake stores ([`ProposalQueue::create_intake`]); the queue adds
/// the ID, project, status and times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewIntake {
    /// [`ProposalKind::Question`] or [`ProposalKind::Discrepancy`].
    pub kind: ProposalKind,
    /// Root-relative: the first target's holder.
    pub target_path: String,
    pub place: Place,
    pub author: Author,
    pub intake: Intake,
}

/// A stored item the dedup found: of the same kind and project, sharing a
/// target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueMatch {
    pub id: String,
    pub status: ProposalStatus,
    /// The rejection's reason (`decision_note`) of a rejected one.
    pub reason: Option<String>,
    /// Its decision record's ID, root-relative path and title, as stored.
    pub record_id: Option<String>,
    pub record_path: Option<String>,
    pub record_title: Option<String>,
}

/// A decision record of a question or a discrepancy, set at apply step 7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The record's ID, issued by the queue.
    pub id: String,
    /// Root-relative, clean.
    pub path: String,
    pub title: String,
    /// The rendered bytes, as written and committed.
    pub text: String,
    pub choice: Choice,
}

/// The series a record's ID is issued from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSeries {
    /// The `[decision_records] prefix`.
    pub prefix: String,
    /// Its `[ids]` width.
    pub width: u32,
    /// The highest number of the prefix (or its aliases) in the corpus.
    pub corpus_max: u64,
}

/// Apply step 7 of a question or a discrepancy: its record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordApproval {
    pub series: RecordSeries,
    /// The ID previewed before the owner's consent; issued only if it is
    /// still the next.
    pub preview: String,
    /// Root-relative, clean.
    pub path: String,
    pub title: String,
    pub text: String,
    pub choice: Choice,
}

/// What [`ProposalQueue::create_intake`] found and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeResult {
    /// Same kind and project (any repository, any state), sharing a target,
    /// the normalised summary equal: by ID number.
    pub hits: Vec<QueueMatch>,
    /// Same kind and project, sharing a target, other text: by ID number.
    pub related: Vec<QueueMatch>,
    /// The stored item; `None` when a hit was not named in
    /// `distinct_from` (nothing stored, no ID taken).
    pub created: Option<Proposal>,
    /// The stored linked update.
    pub linked: Option<Proposal>,
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
    /// `update` only: the intake kinds store these five NULL and read
    /// them as `""`.
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
    /// A `question`'s or a `discrepancy`'s fields; `None` for an `update`.
    pub intake: Option<Intake>,
    /// The other proposal of a discrepancy and its proposed patch (an
    /// `update`), both ways.
    pub linked: Option<String>,
    /// A question's or a discrepancy's decision record, from its step 7
    /// on; `None` for an update.
    pub record: Option<DecisionRecord>,
    /// A create's new IDs, canonical, in text order (its `target_ids` but
    /// the first, and the first too for a new file declaring an `id:`);
    /// empty for any other kind.
    pub new_ids: Vec<String>,
    /// The task it was raised for (`T-NNNN`); never changes.
    pub task_id: Option<String>,
    /// The owner's staged choice, on an `open` proposal only
    /// (`docs/canon/decision-staging.md`).
    pub staged: Option<StagedChoice>,
}

impl Proposal {
    /// A create of a new file: no base (`base_hash` stored `NULL`, read as
    /// `""`).
    pub fn new_file(&self) -> bool {
        self.kind == ProposalKind::Create && self.base_hash.is_empty()
    }

    /// Every target, canonical: an intake's, as given; a create's
    /// `target_id` then its other new IDs; an update's `[target_id]`.
    pub fn target_ids(&self) -> Vec<String> {
        if let Some(intake) = &self.intake {
            return intake.target_ids.clone();
        }
        create_targets(&self.target_id, self.new_file(), &self.new_ids)
    }

    /// Its state as read now: the key of a compare-and-set change.
    pub fn seen(&self) -> Seen {
        Seen {
            status: self.status,
            updated_at: self.updated_at.clone(),
            staged: self.staged.clone(),
        }
    }
}

/// The state of a proposal as one run read it, or wrote it at apply step 7:
/// the key of [`ProposalQueue::approve_from`] and
/// [`ProposalQueue::reopen_from`]. Every change sets `updated_at`, and an
/// `approved` → `approved` change only to a later time, so one key names
/// one hold. The stage read is part of it: a stage replaced within the
/// second `updated_at` resolves is a change too
/// (`docs/canon/decision-staging.md` "Terminal").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub status: ProposalStatus,
    pub updated_at: String,
    pub staged: Option<StagedChoice>,
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

/// One event of a live tail ([`SqliteQueue::events_after`]): its `seq`,
/// `type` and `payload` as stored (the payload's JSON text untouched).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailEvent {
    pub seq: i64,
    pub event_type: String,
    pub payload: String,
}

/// What [`SqliteQueue::events_after`] read in its one read transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventsAfter {
    /// The project's events after the `seq` given, by `seq`, from at most
    /// the limit's rows; a row whose `type` or `payload` is NULL is
    /// skipped.
    pub events: Vec<TailEvent>,
    /// Where the next poll starts (its `after`): the last row read when the
    /// limit was reached, else the table's highest `seq` (any project's) or
    /// the `after` given, whichever is higher; `None` when nothing was
    /// given and the table has no row (or no queue table exists yet).
    pub last_seq: Option<i64>,
    /// The limit's rows were read (skipped ones counted): the next poll
    /// may find more at once.
    pub full: bool,
}

/// An owner's decision: who, the optional note (an approval's `--note`,
/// a rejection's `--reason`) and the stage it confirms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// `Name <email>`.
    pub decided_by: String,
    pub note: Option<String>,
    /// The `staged_at` of the staged choice this decision confirms (its
    /// flags taken from the stage); `None` when it confirms none. Logged
    /// with `.approved` and `.rejected`; a compare-and-set op refuses one
    /// the proposal does not hold ([`QueueError::Invalid`]); `approve`,
    /// `reject` and `applied_with` compare nothing and log it as given.
    pub staged_at: Option<String>,
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
    /// Apply step 7 of a deciding kind: the record ID previewed is no
    /// longer the next (another run took it meanwhile); `next` is. Nothing
    /// written.
    Issued { next: String },
    /// A create's new ID `id` is held by the live create `by` (`open` or
    /// `approved`, any repository of the project). Nothing written.
    Reserved { id: String, by: String },
    /// A task change refused (`docs/canon/tasks.md` "Transitions",
    /// "Task-bound proposals"): no such task, a state or run that does not
    /// allow it, a lost compare-and-set, a proposal the task does not take.
    /// `reason` is one line; nothing written.
    TaskRefused { id: String, reason: String },
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
            Self::Issued { next } => write!(
                f,
                "the record ID previewed was issued meanwhile; the next is `{next}`"
            ),
            Self::Reserved { id, by } => {
                write!(f, "`{id}` is reserved by `{by}`, a live create")
            }
            Self::TaskRefused { reason, .. } => f.write_str(reason),
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

/// `b3:` of `target_id` LF `base_hash` LF `new_text` (07 §1.2); a create's
/// new file: `base_hash` `""`.
pub fn patch_hash(target_id: &str, base_hash: &str, new_text: &str) -> String {
    b3_hash(&patch_hash_input(target_id, base_hash, new_text))
}

/// A new ID a live create holds ([`ProposalQueue::reserved`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    /// Canonical (`slug/ID` for a feature-scoped one).
    pub id: String,
    /// The create holding it.
    pub by: String,
    /// `open` or `approved`.
    pub status: ProposalStatus,
}

/// A create's `target_ids`: `new_ids` alone for a new file whose first new
/// ID is its `target_id` (its `id:`), else `target_id` then `new_ids`.
fn create_targets(target_id: &str, new_file: bool, new_ids: &[String]) -> Vec<String> {
    if new_file && new_ids.first().map(String::as_str) == Some(target_id) {
        return new_ids.to_vec();
    }
    let mut targets = Vec::with_capacity(new_ids.len() + 1);
    targets.push(target_id.to_owned());
    targets.extend(new_ids.iter().cloned());
    targets
}

/// The proposal queue of one project. Every change is one `Immediate`
/// transaction with its event; every read one snapshot.
pub trait ProposalQueue {
    /// Stores `proposal` as `open` under the next ID (highest + 1, taken in
    /// this transaction), with `created_at` = `updated_at` = `now`, and
    /// logs `proposal.created`. A kind that never applies is
    /// [`QueueError::Invalid`] ([`Self::create_intake`] stores it); so is
    /// an update without its base or with new IDs, a create's base given in
    /// part, or its new IDs repeated. A create whose new ID a live create of
    /// the project holds (any repository, read inside this transaction):
    /// [`QueueError::Reserved`], nothing written.
    fn create(&mut self, proposal: &NewProposal, now: &str) -> Result<Proposal, QueueError>;
    /// [`Self::create`] bound to the task `task` (`T-NNNN`), when given:
    /// read inside the inserting transaction, it must be the project's,
    /// of the proposal's repository, neither `done` nor `cancelled`, and,
    /// once claimed, claimed in the proposal's worktree; else
    /// [`QueueError::TaskRefused`], nothing written
    /// (`docs/canon/tasks.md` "Task-bound proposals").
    fn create_with_task(
        &mut self,
        proposal: &NewProposal,
        task: Option<&str>,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// The new IDs the project's live creates (`open`, `approved`, any
    /// repository) hold, by proposal ID number then in text order; read
    /// only. A row whose targets do not read holds none.
    fn reserved(&self) -> Result<Vec<Reservation>, QueueError>;
    /// The intake's step 7, one `Immediate` transaction: the queue's hits
    /// and related items read under the write lock ([`IntakeResult`]);
    /// when every hit, `corpus_hits` (names) and the queue's (IDs), is
    /// named byte for byte in its `distinct_from` (none found included),
    /// `intake` stored `open` under the next ID, then `patch` (an
    /// `update`) under the one after, `linked` both ways, a
    /// `proposal.created` each, committed; else nothing written. A kind
    /// that applies is [`QueueError::Invalid`].
    fn create_intake(
        &mut self,
        intake: &NewIntake,
        corpus_hits: &[String],
        patch: Option<&NewProposal>,
        now: &str,
    ) -> Result<IntakeResult, QueueError>;
    /// [`Self::create_intake`] bound to the task `task`, checked as
    /// [`Self::create_with_task`] checks it before the dedup; the linked
    /// update takes the same `task_id` in the same transaction.
    fn create_intake_with_task(
        &mut self,
        intake: &NewIntake,
        corpus_hits: &[String],
        patch: Option<&NewProposal>,
        task: Option<&str>,
        now: &str,
    ) -> Result<IntakeResult, QueueError>;
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
    /// The stored state not `seen` (its stage too): [`QueueError::Changed`],
    /// nothing written; `applied`, `rejected`: [`QueueError::Status`]; a
    /// kind that never applies: [`QueueError::Invalid`]. Leaving `open`
    /// clears the stage (this op, `approve`, `approve_record_from`,
    /// `applied`, `applied_with`, `reject`, `reject_from`), and
    /// `.approved`, `.rejected` add `staged_at` when the decision confirms
    /// it ([`Decision::staged_at`]).
    fn approve_from(
        &mut self,
        id: &str,
        seen: &Seen,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// The next record ID of `series`, read only: one more than the
    /// greater of `series.corpus_max` and the highest number of the
    /// project's stored `record_id`s of its prefix, zero-padded to its
    /// width.
    fn next_record(&self, series: &RecordSeries) -> Result<String, QueueError>;
    /// Apply step 7 of a question or a discrepancy: [`Self::approve_from`]'s
    /// compare-and-set on `seen`, and its record set (replacing an earlier
    /// one). The record's ID is the one the proposal holds from an earlier
    /// step 7, else [`Self::next_record`] read again under the write lock;
    /// not `approval.preview` → [`QueueError::Issued`], nothing written.
    /// An `update`, a path that is not clean, a choice not of the kind or
    /// out of range: [`QueueError::Invalid`]. Logs `proposal.approved`
    /// (with `record`) from `open`.
    fn approve_record_from(
        &mut self,
        id: &str,
        seen: &Seen,
        approval: &RecordApproval,
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
    /// kept. `applied`, `rejected`: [`QueueError::Status`]; a kind that
    /// never applies: [`QueueError::Invalid`].
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
    /// Stages `stage` on an `open` proposal, replacing any earlier one, as a
    /// compare-and-set on `seen`
    /// (`docs/canon/decision-staging.md` "Queue"): `staged`, `staged_at` =
    /// `updated_at` = `now`, logging
    /// `proposal.staged` (`staged`, `staged_at`). The state no longer
    /// `seen`: [`QueueError::Changed`]; not `open`: [`QueueError::Status`];
    /// a stage the kind cannot hold ([`Stage::problem`]):
    /// [`QueueError::Invalid`]; nothing written.
    fn stage_from(
        &mut self,
        id: &str,
        seen: &Seen,
        stage: &Stage,
        now: &str,
    ) -> Result<Proposal, QueueError>;
    /// Clears an `open` proposal's stage as [`Self::stage_from`] sets one
    /// (`updated_at` = `now`), logging `proposal.unstaged`: `Ok(true)`;
    /// nothing staged: `Ok(false)`, nothing written, no event.
    fn unstage_from(&mut self, id: &str, seen: &Seen, now: &str) -> Result<bool, QueueError>;
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
    /// number, `tasks` by ID number, `runs` by task and number, then
    /// `events` by `seq` (a table a DB does not hold yet left out). Equal
    /// dumps: the same queue.
    pub fn dump(&self) -> Result<String, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let mut out = String::new();
        for (table, order) in [
            ("proposals", ID_ORDER),
            ("tasks", ID_ORDER),
            ("runs", tasks::RUN_ORDER),
            ("events", "ORDER BY seq"),
        ] {
            if !schema::has_table(&tx, table)? {
                continue;
            }
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

    /// A live tail's poll (task spec `daemon-read`, "Data"): in one short
    /// read transaction, ended before returning, the bound project's events
    /// with a `seq` above `after`, by `seq`, from at most `limit` rows (none
    /// read when `after` is `None`), and where the next poll starts
    /// ([`EventsAfter::last_seq`]). A DB whose queue tables do not exist
    /// yet reads as no event; nothing is written, no schema step runs (open
    /// it with [`Self::open_existing`]). The `user_version` is read in the
    /// same transaction, so a handle kept from poll to poll still refuses a
    /// newer build's queue ([`QueueError::SchemaTooNew`]).
    pub fn events_after(
        &self,
        after: Option<i64>,
        limit: usize,
    ) -> Result<EventsAfter, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let version = user_version(&tx)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        if !schema::has_table(&tx, "events")? {
            return Ok(EventsAfter {
                events: Vec::new(),
                last_seq: after,
                full: false,
            });
        }
        let highest: Option<i64> = tx
            .query_row("SELECT max(seq) FROM main.events", [], |row| row.get(0))
            .db()?;
        let Some(after) = after else {
            tx.commit().db()?;
            return Ok(EventsAfter {
                events: Vec::new(),
                last_seq: highest,
                full: false,
            });
        };
        let mut events = Vec::new();
        let mut read = 0;
        let mut last_read = after;
        {
            let mut statement = tx
                .prepare(
                    "SELECT seq, type, payload FROM main.events \
                     WHERE project = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
                )
                .db()?;
            let rows = statement
                .query_map(
                    rusqlite::params![
                        self.project,
                        after,
                        i64::try_from(limit).unwrap_or(i64::MAX)
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .db()?;
            for row in rows {
                let (seq, event_type, payload) = row.db()?;
                read += 1;
                last_read = seq;
                if let (Some(event_type), Some(payload)) = (event_type, payload) {
                    events.push(TailEvent {
                        seq,
                        event_type,
                        payload,
                    });
                }
            }
        }
        tx.commit().db()?;
        let full = read >= limit;
        let last_seq = if full {
            last_read
        } else {
            highest.map_or(after, |highest| highest.max(after))
        };
        Ok(EventsAfter {
            events,
            last_seq: Some(last_seq),
            full,
        })
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
        approved_without_record(&current)?;
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
            confirms(&current, decision)?;
        }
        tx.execute(
            "UPDATE main.proposals SET status = ?1, decided_by = ?2, decided_at = ?3, \
             decision_note = ?4, updated_at = ?3, staged = NULL, staged_at = NULL \
             WHERE id = ?5 AND project = ?6",
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
            log(
                &tx,
                &project,
                EVENT_APPROVED,
                &with_staged(json!({ "id": id }), decision),
                now,
            )?;
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
        if let Some(seen) = seen {
            if current.seen() != *seen {
                return Err(changed(current));
            }
            confirms(&current, decision)?;
        }
        tx.execute(
            "UPDATE main.proposals SET status = ?1, decided_by = ?2, decided_at = ?3, \
             decision_note = ?4, updated_at = ?3, staged = NULL, staged_at = NULL \
             WHERE id = ?5 AND project = ?6",
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
            &with_staged(json!({ "id": id, "reason": decision.note }), decision),
            now,
        )?;
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// [`ProposalQueue::applied`] (`decision` `None`: from `approved` only)
    /// and [`ProposalQueue::applied_with`] (also from `open`, deciding), as
    /// [`Self::applied_refreshing`] when `refresh` is given: the stored
    /// proposal and the snapshot nodes refreshed.
    fn applied_if(
        &mut self,
        id: &str,
        commit: &str,
        decision: Option<&Decision>,
        refresh: Option<&TaskRefresh>,
        now: &str,
    ) -> Result<(Proposal, Vec<String>), QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        if current.kind.decides() && current.record.is_none() {
            return Err(QueueError::Invalid(format!(
                "`{id}` is a {} without its decision record: it is applied only after the \
                 record's step 7",
                current.kind.as_str()
            )));
        }
        let record = current.record.as_ref().map(|record| record.id.clone());
        match (current.status, decision) {
            (ProposalStatus::Approved, _) => {}
            (ProposalStatus::Open, Some(decision)) => {
                tx.execute(
                    "UPDATE main.proposals SET decided_by = ?1, decided_at = ?2, \
                     decision_note = ?3 WHERE id = ?4 AND project = ?5",
                    rusqlite::params![decision.decided_by, now, decision.note, id, project],
                )
                .db()?;
                // Deliberately no compare-and-set (module documentation,
                // "Runs"): the stage it confirms, if any, as the caller says.
                log(
                    &tx,
                    &project,
                    EVENT_APPROVED,
                    &with_staged(
                        with_record(json!({ "id": id }), record.as_deref()),
                        decision,
                    ),
                    now,
                )?;
            }
            _ => return Err(status_error(current)),
        }
        tx.execute(
            "UPDATE main.proposals SET status = ?1, applied_commit = ?2, updated_at = ?3, \
             staged = NULL, staged_at = NULL WHERE id = ?4 AND project = ?5",
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
            &with_record(json!({ "id": id, "commit": commit }), record.as_deref()),
            now,
        )?;
        let refreshed = match refresh {
            Some(refresh) if current.task_id.as_deref() == Some(refresh.task_id.as_str()) => {
                tasks::refresh_snapshot(&tx, &project, id, refresh, now)?
            }
            _ => Vec::new(),
        };
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok((stored, refreshed))
    }

    /// A task-bound `update` or section-form `create` applied in its task's
    /// compared place (`docs/canon/tasks.md` "Task-bound proposals"):
    /// [`ProposalQueue::applied`] (`decision` `None`) or
    /// [`ProposalQueue::applied_with`], and in the same transaction each
    /// snapshot node of `refresh` still frozen at its pre-apply hash takes
    /// the applied text and hash, one `task.refreshed` each (`task`,
    /// `proposal`, `node`). A task that does not read, or a proposal not
    /// bound to `refresh.task_id`, refreshes nothing: recording the commit
    /// never fails for the task's sake. The stored proposal and the nodes
    /// refreshed, in snapshot order.
    pub fn applied_refreshing(
        &mut self,
        id: &str,
        commit: &str,
        decision: Option<&Decision>,
        refresh: &TaskRefresh,
        now: &str,
    ) -> Result<(Proposal, Vec<String>), QueueError> {
        self.applied_if(id, commit, decision, Some(refresh), now)
    }

    /// [`ProposalQueue::approve_record_from`].
    fn approve_record_if(
        &mut self,
        id: &str,
        seen: &Seen,
        approval: &RecordApproval,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        if !current.kind.decides() {
            let article = if current.kind == ProposalKind::Update {
                "an"
            } else {
                "a"
            };
            return Err(QueueError::Invalid(format!(
                "`{id}` is {article} {}: its approval writes no decision record",
                current.kind.as_str()
            )));
        }
        match current.status {
            ProposalStatus::Open | ProposalStatus::Approved => {}
            _ => return Err(status_error(current)),
        }
        if current.seen() != *seen {
            return Err(changed(current));
        }
        if current.status == ProposalStatus::Approved && now <= current.updated_at.as_str() {
            return Err(QueueError::Invalid(format!(
                "`{id}` was approved at {}, not before now ({now}): approve it again in a moment",
                current.updated_at
            )));
        }
        if !is_clean_relative(&approval.path) {
            return Err(QueueError::Invalid(format!(
                "the record path {:?} is not a clean root-relative path",
                approval.path
            )));
        }
        if let Some(problem) = choice_problem(&approval.choice, current.kind, &current) {
            return Err(QueueError::Invalid(format!("`{id}`: {problem}")));
        }
        let next = match &current.record {
            Some(record) => record.id.clone(),
            None => record_id(
                &approval.series.prefix,
                approval.series.width,
                next_number(&tx, &project, &approval.series)?,
            ),
        };
        if next != approval.preview {
            return Err(QueueError::Issued { next });
        }
        confirms(&current, decision)?;
        let choice = to_json(&approval.choice)?;
        tx.execute(
            "UPDATE main.proposals SET status = ?1, decided_by = ?2, decided_at = ?3, \
             decision_note = ?4, updated_at = ?3, record_id = ?5, record_path = ?6, \
             record_title = ?7, record_text = ?8, choice = ?9, staged = NULL, staged_at = NULL \
             WHERE id = ?10 AND project = ?11",
            rusqlite::params![
                ProposalStatus::Approved.as_str(),
                decision.decided_by,
                now,
                decision.note,
                approval.preview,
                approval.path,
                approval.title,
                approval.text,
                choice,
                id,
                project
            ],
        )
        .db()?;
        if current.status == ProposalStatus::Open {
            log(
                &tx,
                &project,
                EVENT_APPROVED,
                &with_staged(json!({ "id": id, "record": approval.preview }), decision),
                now,
            )?;
        }
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
/// the tables, 1 → 2 adds the intake columns, 2 → 3 the record columns,
/// 3 → 4 the tasks, their runs and a proposal's task, 4 → 5 a proposal's
/// stage (0 → 5 runs all five; a schema-4 DB opens as 5, every stage
/// `NULL`);
/// [`QUEUE_SCHEMA_VERSION`] is left alone; a higher version is refused.
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
    if version == 1 {
        tx.execute_batch(STEP_2).db()?;
        version = 2;
    }
    if version == 2 {
        tx.execute_batch(STEP_3).db()?;
        version = 3;
    }
    if version == 3 {
        tx.execute_batch(STEP_4).db()?;
        version = 4;
    }
    if version == 4 {
        tx.execute_batch(STEP_5).db()?;
        version = 5;
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
        let mut text = Vec::with_capacity(PROPOSAL_COLUMNS.len());
        for column in 0..PROPOSAL_COLUMNS.len() {
            text.push(row.get::<_, Option<String>>(column)?);
        }
        Ok(Self { text })
    }

    /// The stored state, when it reads: `list_readable`'s filter before
    /// decoding.
    fn status(&self) -> Option<ProposalStatus> {
        self.text[3].as_deref().and_then(ProposalStatus::parse)
    }

    fn decode(self) -> Result<Proposal, UnreadableRow> {
        let mut values = self.text.into_iter();
        let mut take = || values.next().flatten();
        let id = take().unwrap_or_default();
        let required = |value: Option<String>, column: &str| {
            value.ok_or_else(|| corrupt(&id, column, "it is NULL"))
        };
        let project = required(take(), "project")?;
        let kind_text = required(take(), "kind")?;
        let kind = ProposalKind::parse(&kind_text)
            .ok_or_else(|| corrupt(&id, "kind", format!("`{kind_text}` is no kind")))?;
        let status_text = required(take(), "status")?;
        let status = ProposalStatus::parse(&status_text)
            .ok_or_else(|| corrupt(&id, "status", format!("`{status_text}` is no state")))?;
        let target_id = required(take(), "target_id")?;
        let target_path = required(take(), "target_path")?;
        let place = Place {
            git_common_dir: required(take(), "git_common_dir")?,
            worktree: required(take(), "worktree")?,
            root_rel: required(take(), "root_rel")?,
            branch: required(take(), "branch")?,
            base_commit: required(take(), "base_commit")?,
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
        // An update's texts; the intake kinds store them NULL, a create's
        // new file its base.
        let base_columns = [take(), take()];
        let mut update_text = |column: &str| {
            let value = take();
            if kind.applies() {
                required(value, column)
            } else {
                Ok(String::new())
            }
        };
        let new_text = update_text("new_text")?;
        let patch_hash = update_text("patch_hash")?;
        let rationale = update_text("rationale")?;
        let [base_hash, base_text] = match (kind, base_columns) {
            (ProposalKind::Create, [None, None]) => [String::new(), String::new()],
            (ProposalKind::Create, [Some(hash), _]) if hash.is_empty() => {
                return Err(corrupt(&id, "base_hash", "it is empty on a create"));
            }
            (ProposalKind::Create, [Some(hash), Some(text)]) => [hash, text],
            (ProposalKind::Create, [hash, _]) => {
                let column = if hash.is_none() {
                    "base_hash"
                } else {
                    "base_text"
                };
                return Err(corrupt(
                    &id,
                    column,
                    "it is NULL while the base's other column is set",
                ));
            }
            (_, [hash, text]) if kind.applies() => {
                [required(hash, "base_hash")?, required(text, "base_text")?]
            }
            _ => [String::new(), String::new()],
        };
        let author_text = required(take(), "author")?;
        let author: Author =
            serde_json::from_str(&author_text).map_err(|error| corrupt(&id, "author", error))?;
        for field in [&author.role, &author.model, &author.run] {
            if let Some(problem) = field.as_deref().and_then(author_field_problem) {
                return Err(corrupt(&id, "author", problem));
            }
        }
        let diagnostics_text = required(take(), "diagnostics")?;
        let diagnostics: Vec<ProposalFinding> = serde_json::from_str(&diagnostics_text)
            .map_err(|error| corrupt(&id, "diagnostics", error))?;
        let decided_by = take();
        let decided_at = take();
        let decision_note = take();
        let applied_commit = take();
        let created_at = required(take(), "created_at")?;
        let updated_at = required(take(), "updated_at")?;
        let columns = IntakeColumns {
            target_ids: take(),
            severity: take(),
            gap_type: take(),
            summary: take(),
            working_answer: take(),
            price_of_other: take(),
            evidence: take(),
            options: take(),
            recommendation: take(),
            distinct_from: take(),
        };
        let linked = take();
        let record_columns: [Option<String>; 5] = std::array::from_fn(|_| take());
        let task_id = take();
        let staged_columns = [take(), take()];
        if let Some(task) = &task_id
            && specengine_core::task::task_number(task).is_none()
        {
            return Err(corrupt(&id, "task_id", format!("{task:?} is no task ID")));
        }
        let new_file = kind == ProposalKind::Create && base_hash.is_empty();
        let staged = stage::decode(&id, kind, status, new_file, staged_columns)?;
        let new_ids = if kind == ProposalKind::Create {
            columns.decode_create(&id, &target_id, base_hash.is_empty(), linked.as_deref())?
        } else {
            Vec::new()
        };
        let intake = if kind.applies() {
            None
        } else {
            Some(columns.decode(&id, kind, &target_id)?)
        };
        let record = decode_record(&id, kind, status, intake.as_ref(), record_columns)?;
        if let Some(linked) = &linked
            && proposal_number(linked).is_none()
        {
            return Err(corrupt(
                &id,
                "linked",
                format!("{linked:?} is no proposal ID"),
            ));
        }
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
            intake,
            linked,
            record,
            new_ids,
            task_id,
            staged,
        })
    }
}

/// The record columns of a row ([`RECORD_COLUMNS`]): none on an update;
/// all or none on a question or a discrepancy, all on an approved or
/// applied one; `record_id` an ID, `record_path` clean, `choice` of the
/// row's kind and in range. Else the row is corrupt, named.
fn decode_record(
    id: &str,
    kind: ProposalKind,
    status: ProposalStatus,
    intake: Option<&Intake>,
    columns: [Option<String>; 5],
) -> Result<Option<DecisionRecord>, UnreadableRow> {
    if !kind.decides() {
        if let Some(at) = columns.iter().position(Option::is_some) {
            let holder = if kind == ProposalKind::Update {
                "an update"
            } else {
                "a create"
            };
            return Err(corrupt(
                id,
                RECORD_COLUMNS[at],
                format!("{holder} holds no record"),
            ));
        }
        return Ok(None);
    }
    if columns.iter().all(Option::is_none) {
        if matches!(status, ProposalStatus::Approved | ProposalStatus::Applied) {
            return Err(corrupt(
                id,
                RECORD_COLUMNS[0],
                format!("it is NULL on an {status} {}", kind.as_str()),
            ));
        }
        return Ok(None);
    }
    if let Some(at) = columns.iter().position(Option::is_none) {
        return Err(corrupt(
            id,
            RECORD_COLUMNS[at],
            "it is NULL while the record's other columns are set",
        ));
    }
    let [
        Some(record),
        Some(path),
        Some(title),
        Some(text),
        Some(choice),
    ] = columns
    else {
        return Err(corrupt(id, RECORD_COLUMNS[0], "it is NULL"));
    };
    if !is_record_id(&record) {
        return Err(corrupt(
            id,
            "record_id",
            format!("{record:?} is no record ID"),
        ));
    }
    if !is_clean_relative(&path) {
        return Err(corrupt(
            id,
            "record_path",
            format!("{path:?} is no clean root-relative path"),
        ));
    }
    let shape = || {
        corrupt(
            id,
            "choice",
            "not `{\"option\":N}`, `{\"working_answer\":true}` or `{\"answer\":\"…\"}`",
        )
    };
    let value: Value = serde_json::from_str(&choice).map_err(|_| shape())?;
    let choice = Choice::from_json(&value).ok_or_else(shape)?;
    if let Some(problem) = intake.and_then(|intake| choice_fits(&choice, kind, intake)) {
        return Err(corrupt(id, "choice", problem));
    }
    Ok(Some(DecisionRecord {
        id: record,
        path,
        title,
        text,
        choice,
    }))
}

/// `text` has a canonical ID's shape: an optional `slug/` (a lower-case
/// letter, then lower-case letters, digits and `-`), an ASCII prefix (a
/// capital letter, then capitals and digits), `-`, a body of ASCII letters,
/// digits and `-`, not ending in `-`.
fn is_canonical_id(text: &str) -> bool {
    let id = match text.split_once('/') {
        Some((slug, id)) => {
            let slug_ok = slug.starts_with(|c: char| c.is_ascii_lowercase())
                && slug
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
            if !slug_ok {
                return false;
            }
            id
        }
        None => text,
    };
    id.split_once('-').is_some_and(|(prefix, body)| {
        prefix.starts_with(|c: char| c.is_ascii_uppercase())
            && prefix
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            && !body.is_empty()
            && !body.ends_with('-')
            && body
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

/// `text` has a record ID's shape: an ASCII prefix (a capital letter, then
/// capitals and digits), `-`, ASCII digits.
fn is_record_id(text: &str) -> bool {
    text.split_once('-').is_some_and(|(prefix, digits)| {
        prefix.starts_with(|c: char| c.is_ascii_uppercase())
            && prefix
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            && !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// Why `choice` is not one `kind` takes: an option of a discrepancy, in
/// range of its options; the working answer or another answer of a
/// question. `None`: it is.
fn choice_fits(choice: &Choice, kind: ProposalKind, intake: &Intake) -> Option<String> {
    match (kind, choice) {
        (ProposalKind::Discrepancy, Choice::Option(index)) => {
            let fits = usize::try_from(*index).is_ok_and(|index| index < intake.options.len());
            (!fits).then(|| {
                format!(
                    "option {index} is out of range of its {} options",
                    intake.options.len()
                )
            })
        }
        (ProposalKind::Question, Choice::WorkingAnswer | Choice::Answer(_)) => None,
        _ => Some(format!(
            "{} is no choice of a {}",
            choice.described(),
            kind.as_str()
        )),
    }
}

/// [`choice_fits`] of a stored proposal's intake.
fn choice_problem(choice: &Choice, kind: ProposalKind, proposal: &Proposal) -> Option<String> {
    let intake = proposal.intake.as_ref()?;
    choice_fits(choice, kind, intake)
}

/// `payload` with `record` added when there is one.
fn with_record(mut payload: Value, record: Option<&str>) -> Value {
    if let (Some(record), Value::Object(object)) = (record, &mut payload) {
        object.insert("record".to_owned(), Value::String(record.to_owned()));
    }
    payload
}

/// `payload` with `staged_at` added, last, when `decision` confirms a stage
/// (the payload's other keys sort before it, so a JSON map of either order
/// writes it last).
fn with_staged(mut payload: Value, decision: &Decision) -> Value {
    if let (Some(at), Value::Object(object)) = (&decision.staged_at, &mut payload) {
        object.insert("staged_at".to_owned(), Value::String(at.clone()));
    }
    payload
}

/// A decision confirming a stage confirms the one `current` holds (its
/// `staged_at`), else [`QueueError::Invalid`], nothing written.
fn confirms(current: &Proposal, decision: &Decision) -> Result<(), QueueError> {
    let Some(at) = &decision.staged_at else {
        return Ok(());
    };
    if current
        .staged
        .as_ref()
        .is_some_and(|staged| staged.at == *at)
    {
        return Ok(());
    }
    Err(QueueError::Invalid(format!(
        "`{}` holds no choice staged at {at}: the decision confirms none",
        current.id
    )))
}

/// The highest number `<prefix>-<digits>` of the project's stored
/// `record_id`s and `series.corpus_max`, plus one.
fn next_number(conn: &Connection, project: &str, series: &RecordSeries) -> Result<u64, QueueError> {
    let mut statement = conn
        .prepare(
            "SELECT record_id FROM main.proposals WHERE project = ?1 AND record_id IS NOT NULL",
        )
        .db()?;
    let ids: Vec<Option<String>> = statement
        .query_map([project], |row| row.get(0))
        .db()?
        .collect::<rusqlite::Result<_>>()
        .db()?;
    let mut highest = series.corpus_max;
    for id in ids.into_iter().flatten() {
        let number = id
            .strip_prefix(series.prefix.as_str())
            .and_then(|rest| rest.strip_prefix('-'))
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse::<u64>().ok());
        if let Some(number) = number {
            highest = highest.max(number);
        }
    }
    highest.checked_add(1).ok_or_else(|| {
        QueueError::Invalid(format!(
            "`{}` numbers reach {highest}: no record ID follows",
            series.prefix
        ))
    })
}

/// The intake columns of a row, before decoding.
struct IntakeColumns {
    target_ids: Option<String>,
    severity: Option<String>,
    gap_type: Option<String>,
    summary: Option<String>,
    working_answer: Option<String>,
    price_of_other: Option<String>,
    evidence: Option<String>,
    options: Option<String>,
    recommendation: Option<String>,
    distinct_from: Option<String>,
}

impl IntakeColumns {
    /// The new IDs of a `create` row (`new_file`: its base `NULL`):
    /// `target_ids` a JSON list of canonical IDs headed by `target_id`
    /// (a new file without an `id:`: its path), each once, every other
    /// intake column and `linked` `NULL`; else the row is corrupt, named.
    fn decode_create(
        &self,
        id: &str,
        target_id: &str,
        new_file: bool,
        linked: Option<&str>,
    ) -> Result<Vec<String>, UnreadableRow> {
        let others = [
            ("severity", &self.severity),
            ("gap_type", &self.gap_type),
            ("summary", &self.summary),
            ("working_answer", &self.working_answer),
            ("price_of_other", &self.price_of_other),
            ("evidence", &self.evidence),
            ("options", &self.options),
            ("recommendation", &self.recommendation),
            ("distinct_from", &self.distinct_from),
        ];
        if let Some((column, _)) = others.iter().find(|(_, value)| value.is_some()) {
            return Err(corrupt(id, column, "a create holds no intake field"));
        }
        if linked.is_some() {
            return Err(corrupt(id, "linked", "a create is linked to nothing"));
        }
        let text = self
            .target_ids
            .as_deref()
            .ok_or_else(|| corrupt(id, "target_ids", "it is NULL on a create"))?;
        let targets: Vec<String> =
            serde_json::from_str(text).map_err(|error| corrupt(id, "target_ids", error))?;
        if targets.first().map(String::as_str) != Some(target_id) {
            return Err(corrupt(
                id,
                "target_ids",
                "its first ID is not the row's `target_id`",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (at, target) in targets.iter().enumerate() {
            // Its target, a document without an `id:`, by its path.
            let path = at == 0 && is_path(target);
            if !(path || is_canonical_id(target)) || !seen.insert(target.as_str()) {
                return Err(corrupt(
                    id,
                    "target_ids",
                    format!("{target:?} is no canonical ID, or repeated"),
                ));
            }
        }
        Ok(new_ids_of(&targets, new_file))
    }

    /// The fields of a `question` or `discrepancy` row: a column its kind
    /// requires NULL, JSON not of its shape, an enum or `recommendation`
    /// out of range make the row corrupt, named.
    fn decode(
        self,
        id: &str,
        kind: ProposalKind,
        target_id: &str,
    ) -> Result<Intake, UnreadableRow> {
        let question = kind == ProposalKind::Question;
        let required = |value: Option<String>, column: &str| {
            value.ok_or_else(|| corrupt(id, column, "it is NULL"))
        };
        fn json<T: serde::de::DeserializeOwned>(
            id: &str,
            column: &str,
            text: &str,
        ) -> Result<T, UnreadableRow> {
            serde_json::from_str(text).map_err(|error| corrupt(id, column, error))
        }
        let target_ids: Vec<String> =
            json(id, "target_ids", &required(self.target_ids, "target_ids")?)?;
        if target_ids.first().map(String::as_str) != Some(target_id) {
            return Err(corrupt(
                id,
                "target_ids",
                "its first ID is not the row's `target_id`",
            ));
        }
        let severity_text = required(self.severity, "severity")?;
        let severity = IntakeSeverity::parse(&severity_text)
            .ok_or_else(|| corrupt(id, "severity", format!("`{severity_text}` is no severity")))?;
        let gap_type = if question {
            None
        } else {
            let text = required(self.gap_type, "gap_type")?;
            Some(
                GapType::parse(&text)
                    .ok_or_else(|| corrupt(id, "gap_type", format!("`{text}` is no gap type")))?,
            )
        };
        let summary = required(self.summary, "summary")?;
        let working_answer = if question {
            Some(required(self.working_answer, "working_answer")?)
        } else {
            self.working_answer
        };
        let price_of_other = if question {
            Some(required(self.price_of_other, "price_of_other")?)
        } else {
            None
        };
        let (evidence, options, recommendation) = if question {
            (Vec::new(), Vec::new(), None)
        } else {
            let evidence: Vec<Evidence> =
                json(id, "evidence", &required(self.evidence, "evidence")?)?;
            let options: Vec<IntakeOption> =
                json(id, "options", &required(self.options, "options")?)?;
            let text = required(self.recommendation, "recommendation")?;
            let recommendation = text
                .parse::<u64>()
                .ok()
                .filter(|number| number.to_string() == text)
                .filter(|&number| usize::try_from(number).is_ok_and(|index| index < options.len()))
                .ok_or_else(|| {
                    corrupt(
                        id,
                        "recommendation",
                        format!("`{text}` is no index into its {} options", options.len()),
                    )
                })?;
            (evidence, options, Some(recommendation))
        };
        let distinct_from: Vec<String> = json(
            id,
            "distinct_from",
            &required(self.distinct_from, "distinct_from")?,
        )?;
        Ok(Intake {
            target_ids,
            severity,
            gap_type,
            summary,
            working_answer,
            price_of_other,
            evidence,
            options,
            recommendation,
            distinct_from,
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

/// A deciding kind refuses an approval without its decision record
/// ([`ProposalQueue::approve_record_from`] approves it), nothing written.
fn approved_without_record(proposal: &Proposal) -> Result<(), QueueError> {
    if proposal.kind.applies() {
        return Ok(());
    }
    Err(QueueError::Invalid(format!(
        "`{}` is a {}: it is approved only with its decision record (or rejected with the \
         answer)",
        proposal.id,
        proposal.kind.as_str()
    )))
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
    log_text(tx, project, event_type, &payload.to_string(), at)
}

/// [`log`] of a payload already written as JSON text.
fn log_text(
    tx: &Transaction<'_>,
    project: &str,
    event_type: &str,
    payload: &str,
    at: &str,
) -> Result<(), QueueError> {
    tx.execute(
        "INSERT INTO main.events (project, type, payload, at) VALUES (?1, ?2, ?3, ?4)",
        [project, event_type, payload, at],
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

/// Inserts `proposal` (an `update` or a `create`) as `open` under the
/// next ID, `linked` to the given proposal, bound to `task`; a create's
/// `target_ids` set ([`create_targets`]), an update's `NULL`; its ID.
fn insert_update(
    tx: &Transaction<'_>,
    project: &str,
    proposal: &NewProposal,
    linked: Option<&str>,
    task: Option<&str>,
    now: &str,
) -> Result<String, QueueError> {
    let author = to_json(&proposal.author)?;
    let diagnostics = to_json(&proposal.diagnostics)?;
    let target_ids = (proposal.kind == ProposalKind::Create)
        .then(|| {
            to_json(&create_targets(
                &proposal.target_id,
                proposal.base_hash.is_none(),
                &proposal.new_ids,
            ))
        })
        .transpose()?;
    let id = next_id(tx)?;
    let place = &proposal.place;
    tx.execute(
        &format!(
            "INSERT INTO main.proposals ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
             ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, NULL, NULL, NULL, NULL, ?19, ?19, \
             ?21, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, ?20, NULL, NULL, NULL, \
             NULL, NULL, ?22, NULL, NULL)"
        ),
        rusqlite::params![
            id,
            project,
            proposal.kind.as_str(),
            ProposalStatus::Open.as_str(),
            proposal.target_id,
            proposal.target_path,
            place.git_common_dir,
            place.worktree,
            place.root_rel,
            place.branch,
            place.base_commit,
            proposal.base_hash,
            proposal.base_text,
            proposal.new_text,
            proposal.patch_hash,
            proposal.rationale,
            author,
            diagnostics,
            now,
            linked,
            target_ids,
            task,
        ],
    )
    .db()?;
    Ok(id)
}

/// Why `proposal` is no proposal [`ProposalQueue::create`] stores: a kind
/// that never applies, an update without its base or with new IDs, a
/// create's base given in part, a new file whose `id:` is not its first
/// new ID, a new ID empty or repeated.
fn new_proposal_problem(proposal: &NewProposal) -> Option<String> {
    let kind = proposal.kind;
    if !kind.applies() {
        return Some(format!(
            "`create` stores a proposal that applies, not a {}: `create_intake` stores it",
            kind.as_str()
        ));
    }
    let based = proposal.base_hash.is_some();
    if based != proposal.base_text.is_some() {
        return Some("a proposal's base is its hash and its text, both or neither".to_owned());
    }
    if kind == ProposalKind::Update {
        if !based {
            return Some("an update is written against its base".to_owned());
        }
        if !proposal.new_ids.is_empty() {
            return Some("an update adds no ID".to_owned());
        }
        return None;
    }
    if proposal.base_hash.as_deref() == Some("") {
        return Some("a create's base hash is empty".to_owned());
    }
    let mut seen = std::collections::BTreeSet::new();
    for id in &proposal.new_ids {
        if id.is_empty() || !seen.insert(id.as_str()) {
            return Some(format!("a create's new ID {id:?} is empty or repeated"));
        }
    }
    if !based
        && !is_path(&proposal.target_id)
        && proposal.new_ids.first() != Some(&proposal.target_id)
    {
        return Some(format!(
            "a new file's target `{}` is neither its path nor its first new ID",
            proposal.target_id
        ));
    }
    None
}

/// A stored target names a document by its root-relative path (it ends in
/// `.md`), not an ID.
fn is_path(target: &str) -> bool {
    target.ends_with(specengine_core::DOCUMENT_EXTENSION)
}

/// A create row's new IDs from its stored `target_ids` (headed by its
/// `target_id`): all but the first, and the first too for a new file (no
/// base) whose target is an ID.
fn new_ids_of(target_ids: &[String], new_file: bool) -> Vec<String> {
    match target_ids.split_first() {
        Some((first, _)) if new_file && !is_path(first) => target_ids.to_vec(),
        Some((_, rest)) => rest.to_vec(),
        None => Vec::new(),
    }
}

/// The live creates' new IDs ([`ProposalQueue::reserved`]), read in `conn`
/// (a write transaction's, for [`ProposalQueue::create`]).
fn live_reservations(conn: &Connection, project: &str) -> Result<Vec<Reservation>, QueueError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT id, status, base_hash, target_ids FROM main.proposals \
             WHERE project = ?1 AND kind = ?2 AND status IN (?3, ?4) {ID_ORDER}"
        ))
        .db()?;
    let rows: Vec<[Option<String>; 4]> = statement
        .query_map(
            [
                project,
                CREATE_KIND,
                ProposalStatus::Open.as_str(),
                ProposalStatus::Approved.as_str(),
            ],
            |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?]),
        )
        .db()?
        .collect::<rusqlite::Result<_>>()
        .db()?;
    let mut held = Vec::new();
    for [id, status, base_hash, target_ids] in rows {
        let (Some(id), Some(status)) = (id, status.as_deref().and_then(ProposalStatus::parse))
        else {
            continue;
        };
        let Some(targets) = target_ids
            .as_deref()
            .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        else {
            continue;
        };
        for new_id in new_ids_of(&targets, base_hash.is_none()) {
            held.push(Reservation {
                id: new_id,
                by: id.clone(),
                status,
            });
        }
    }
    Ok(held)
}

/// Inserts a question or a discrepancy as `open` under the next ID: the
/// update's five text columns NULL, `diagnostics` `[]`, JSON lists as
/// given (an evidence item's absent keys `null`); its ID.
fn insert_intake(
    tx: &Transaction<'_>,
    project: &str,
    new: &NewIntake,
    task: Option<&str>,
    now: &str,
) -> Result<String, QueueError> {
    let intake = &new.intake;
    let discrepancy = new.kind == ProposalKind::Discrepancy;
    let author = to_json(&new.author)?;
    let diagnostics = to_json(&Vec::<ProposalFinding>::new())?;
    let target_ids = to_json(&intake.target_ids)?;
    let evidence = discrepancy.then(|| to_json(&intake.evidence)).transpose()?;
    let options = discrepancy.then(|| to_json(&intake.options)).transpose()?;
    let recommendation = intake.recommendation.map(|index| index.to_string());
    let distinct_from = to_json(&intake.distinct_from)?;
    let id = next_id(tx)?;
    let place = &new.place;
    tx.execute(
        &format!(
            "INSERT INTO main.proposals ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
             ?10, ?11, NULL, NULL, NULL, NULL, NULL, ?12, ?13, NULL, NULL, NULL, NULL, ?14, ?14, \
             ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, NULL, NULL, NULL, NULL, NULL, NULL, \
             ?25, NULL, NULL)"
        ),
        rusqlite::params![
            id,
            project,
            new.kind.as_str(),
            ProposalStatus::Open.as_str(),
            intake.target_ids[0],
            new.target_path,
            place.git_common_dir,
            place.worktree,
            place.root_rel,
            place.branch,
            place.base_commit,
            author,
            diagnostics,
            now,
            target_ids,
            intake.severity.as_str(),
            intake.gap_type.map(GapType::as_str),
            intake.summary,
            intake.working_answer,
            intake.price_of_other,
            evidence,
            options,
            recommendation,
            distinct_from,
            task,
        ],
    )
    .db()?;
    Ok(id)
}

/// The project's rows of the intake's kind (any repository, any state)
/// that share a target with it, by ID number: hits when the normalised
/// summary is equal, else related. A row whose targets or state do not
/// read shares nothing.
fn queue_matches(
    tx: &Transaction<'_>,
    project: &str,
    intake: &NewIntake,
) -> Result<(Vec<QueueMatch>, Vec<QueueMatch>), QueueError> {
    let wanted = normalized_summary(&intake.intake.summary);
    let mut statement = tx
        .prepare(&format!(
            "SELECT id, status, target_ids, summary, decision_note, record_id, record_path, \
             record_title FROM main.proposals WHERE project = ?1 AND kind = ?2 {ID_ORDER}"
        ))
        .db()?;
    let rows: Vec<MatchRow> = statement
        .query_map([project, intake.kind.as_str()], |row| {
            Ok(MatchRow {
                id: row.get(0)?,
                status: row.get(1)?,
                target_ids: row.get(2)?,
                summary: row.get(3)?,
                decision_note: row.get(4)?,
                record_id: row.get(5)?,
                record_path: row.get(6)?,
                record_title: row.get(7)?,
            })
        })
        .db()?
        .collect::<rusqlite::Result<_>>()
        .db()?;
    let (mut hits, mut related) = (Vec::new(), Vec::new());
    for row in rows {
        let MatchRow {
            id,
            status,
            target_ids,
            summary,
            decision_note,
            record_id,
            record_path,
            record_title,
        } = row;
        let (Some(id), Some(status)) = (id, status.as_deref().and_then(ProposalStatus::parse))
        else {
            continue;
        };
        let targets: Vec<String> = target_ids
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        if !targets
            .iter()
            .any(|target| intake.intake.target_ids.contains(target))
        {
            continue;
        }
        let same = summary
            .as_deref()
            .is_some_and(|summary| normalized_summary(summary) == wanted);
        if same {
            let reason = (status == ProposalStatus::Rejected)
                .then_some(decision_note)
                .flatten();
            hits.push(QueueMatch {
                id,
                status,
                reason,
                record_id,
                record_path,
                record_title,
            });
        } else {
            related.push(QueueMatch {
                id,
                status,
                reason: None,
                record_id,
                record_path,
                record_title,
            });
        }
    }
    Ok((hits, related))
}

/// A row [`queue_matches`] reads, not decoded.
struct MatchRow {
    id: Option<String>,
    status: Option<String>,
    target_ids: Option<String>,
    summary: Option<String>,
    decision_note: Option<String>,
    record_id: Option<String>,
    record_path: Option<String>,
    record_title: Option<String>,
}

impl ProposalQueue for SqliteQueue {
    fn create(&mut self, proposal: &NewProposal, now: &str) -> Result<Proposal, QueueError> {
        self.create_with_task(proposal, None, now)
    }

    fn create_with_task(
        &mut self,
        proposal: &NewProposal,
        task: Option<&str>,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        if let Some(problem) = new_proposal_problem(proposal) {
            return Err(QueueError::Invalid(problem));
        }
        let project = self.project.clone();
        let tx = self.write()?;
        if let Some(task) = task {
            tasks::check_binding(&tx, &project, task, &proposal.place)?;
        }
        // The live creates read under the write lock this one inserts
        // under: two creates of one new ID never both store.
        if !proposal.new_ids.is_empty() {
            let held = live_reservations(&tx, &project)?;
            for new_id in &proposal.new_ids {
                if let Some(holder) = held.iter().find(|held| held.id == *new_id) {
                    return Err(QueueError::Reserved {
                        id: new_id.clone(),
                        by: holder.by.clone(),
                    });
                }
            }
        }
        let id = insert_update(&tx, &project, proposal, None, task, now)?;
        log(&tx, &project, EVENT_CREATED, &json!({ "id": id }), now)?;
        let stored = existing(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    fn create_intake(
        &mut self,
        intake: &NewIntake,
        corpus_hits: &[String],
        patch: Option<&NewProposal>,
        now: &str,
    ) -> Result<IntakeResult, QueueError> {
        self.create_intake_with_task(intake, corpus_hits, patch, None, now)
    }

    fn create_intake_with_task(
        &mut self,
        intake: &NewIntake,
        corpus_hits: &[String],
        patch: Option<&NewProposal>,
        task: Option<&str>,
        now: &str,
    ) -> Result<IntakeResult, QueueError> {
        check_time(now)?;
        if intake.kind.applies() {
            return Err(QueueError::Invalid(format!(
                "an intake stores a question or a discrepancy, not an {}",
                intake.kind.as_str()
            )));
        }
        if intake.intake.target_ids.is_empty() {
            return Err(QueueError::Invalid("an intake names no target".to_owned()));
        }
        if patch.is_some_and(|patch| {
            patch.kind != ProposalKind::Update || new_proposal_problem(patch).is_some()
        }) {
            return Err(QueueError::Invalid(
                "a linked proposal is an update".to_owned(),
            ));
        }
        let project = self.project.clone();
        let tx = self.write()?;
        if let Some(task) = task {
            tasks::check_binding(&tx, &project, task, &intake.place)?;
        }
        // The dedup reads under the write lock it inserts under: a parallel
        // intake of the same item waits, then finds this one.
        let (hits, related) = queue_matches(&tx, &project, intake)?;
        let named = |name: &str| {
            intake
                .intake
                .distinct_from
                .iter()
                .any(|entry| entry == name)
        };
        let all_named =
            corpus_hits.iter().all(|name| named(name)) && hits.iter().all(|hit| named(&hit.id));
        if !all_named {
            drop(tx);
            return Ok(IntakeResult {
                hits,
                related,
                created: None,
                linked: None,
            });
        }
        let id = insert_intake(&tx, &project, intake, task, now)?;
        log(&tx, &project, EVENT_CREATED, &json!({ "id": id }), now)?;
        let mut linked = None;
        if let Some(patch) = patch {
            // The linked update takes the item's task, in this transaction.
            let update = insert_update(&tx, &project, patch, Some(&id), task, now)?;
            tx.execute(
                "UPDATE main.proposals SET linked = ?1 WHERE id = ?2 AND project = ?3",
                [update.as_str(), id.as_str(), project.as_str()],
            )
            .db()?;
            log(&tx, &project, EVENT_CREATED, &json!({ "id": update }), now)?;
            linked = Some(existing(&tx, &project, &update)?);
        }
        let created = existing(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok(IntakeResult {
            hits,
            related,
            created: Some(created),
            linked,
        })
    }

    fn reserved(&self) -> Result<Vec<Reservation>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let held = live_reservations(&tx, &self.project)?;
        tx.commit().db()?;
        Ok(held)
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

    fn next_record(&self, series: &RecordSeries) -> Result<String, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let number = next_number(&tx, &self.project, series)?;
        tx.commit().db()?;
        Ok(record_id(&series.prefix, series.width, number))
    }

    fn approve_record_from(
        &mut self,
        id: &str,
        seen: &Seen,
        approval: &RecordApproval,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.approve_record_if(id, seen, approval, decision, now)
    }

    fn applied(&mut self, id: &str, commit: &str, now: &str) -> Result<Proposal, QueueError> {
        self.applied_if(id, commit, None, None, now)
            .map(|(applied, _)| applied)
    }

    fn applied_with(
        &mut self,
        id: &str,
        commit: &str,
        decision: &Decision,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.applied_if(id, commit, Some(decision), None, now)
            .map(|(applied, _)| applied)
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

    fn stage_from(
        &mut self,
        id: &str,
        seen: &Seen,
        stage: &Stage,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        self.stage_if(id, seen, Some(stage), now)
            .map(|(stored, _)| stored)
    }

    fn unstage_from(&mut self, id: &str, seen: &Seen, now: &str) -> Result<bool, QueueError> {
        self.stage_if(id, seen, None, now)
            .map(|(_, written)| written)
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
