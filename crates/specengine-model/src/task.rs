//! A task and its package
//! (`docs/canon/task-package.md` "Package", "Versioning";
//! `docs/canon/tasks.md` "Transitions"; ADR-0027): the ten task states and
//! the versioned, stack-neutral document an agent is given.
//!
//! Types only: the store keeps a task's rows, the CLI assembles the
//! package. Every key is always written, absent = `null`, lists `[]`; a new
//! key keeps [`TASK_PACKAGE_SCHEMA_VERSION`], a removed, renamed or
//! re-meant one raises it. No key names a stack, a tool chain or a role:
//! the project's own words travel as data.

use serde::{Deserialize, Serialize};

/// The package's `schema_version`.
pub const TASK_PACKAGE_SCHEMA_VERSION: u32 = 1;

/// Where a task stands (05 §3.3). `analysis`, `in_review` and `accepted`
/// are known, never entered by this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Draft,
    Analysis,
    Review,
    ChangesRequested,
    Ready,
    InProgress,
    InReview,
    Done,
    Accepted,
    Cancelled,
}

impl TaskStatus {
    pub const ALL: [Self; 10] = [
        Self::Draft,
        Self::Analysis,
        Self::Review,
        Self::ChangesRequested,
        Self::Ready,
        Self::InProgress,
        Self::InReview,
        Self::Done,
        Self::Accepted,
        Self::Cancelled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Analysis => "analysis",
            Self::Review => "review",
            Self::ChangesRequested => "changes_requested",
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::InReview => "in_review",
            Self::Done => "done",
            Self::Accepted => "accepted",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == text)
    }

    /// `done` and `cancelled`: no proposal binds to it any more.
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a run ended, as its agent reports it; it never moves the task's
/// state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunOutcome {
    Completed,
    Partial,
    Failed,
    Abandoned,
}

impl RunOutcome {
    pub const ALL: [Self; 4] = [
        Self::Completed,
        Self::Partial,
        Self::Failed,
        Self::Abandoned,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|outcome| outcome.as_str() == text)
    }
}

/// Who wrote a task: the proposals' author, `{type, role, model, run}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskAuthor {
    #[serde(rename = "type")]
    pub author_type: TaskAuthorType,
    pub role: Option<String>,
    pub model: Option<String>,
    pub run: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskAuthorType {
    Human,
    Agent,
}

/// A target as resolved in the compared place: a node gone keeps its
/// stored ID (`null` for one stored by its path) and path (else the
/// snapshot's), `kind` and `title` `null`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageTarget {
    pub id: Option<String>,
    pub path: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
}

/// A criterion: a reference (`ref`) or free text. Stored with the text of
/// a reference `null`; in the package the reference's text in the
/// compared place (`null` when it is gone).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskCriterion {
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub text: Option<String>,
}

/// What an open proposal on the task assumes until the owner decides: a
/// question's working answer, a discrepancy's recommended option.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageAssumption {
    pub proposal: String,
    pub text: String,
}

/// An `open` or `approved` proposal on the task's nodes, or bound to it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageProposal {
    pub id: String,
    /// Free: the queue's kinds.
    pub kind: String,
    pub status: String,
    pub target_ids: Vec<String>,
    pub task_id: Option<String>,
    /// A question's or a discrepancy's text, else the rationale's first
    /// line.
    pub summary: Option<String>,
}

/// One `spec task changes --note`, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OwnerNote {
    pub at: String,
    pub note: String,
}

/// A code binding of a node: Phase 3; the list stays `[]` until then.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackageBinding {}

/// Where the snapshot was frozen: the approving worktree, the root in it,
/// its branch and `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotPlace {
    pub worktree: String,
    pub root_rel: String,
    pub branch: String,
    pub commit: String,
}

/// The approved spec: each node with its span hash as frozen.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecSnapshot {
    pub at: String,
    pub place: SnapshotPlace,
    pub nodes: Vec<SnapshotNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotNode {
    /// Canonical: an ID, `slug/ID`, or an id-less document's path.
    pub id: String,
    /// Root-relative, where it was frozen.
    pub path: String,
    pub span_hash: String,
}

/// A snapshot node changed (or gone) in the compared place: the diff from
/// the frozen text, cut at a line end (`cut`); `diff` `null` and `cut`
/// `true` past the package's total.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub id: String,
    pub path: String,
    /// The snapshot's.
    pub span_hash: String,
    pub diff: Option<String>,
    pub cut: bool,
}

/// The claim: who works on the task, where.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskClaim {
    pub at: String,
    /// The project's own role name, verbatim.
    pub role: String,
    /// Canonical.
    pub worktree: String,
    pub branch: String,
}

/// One run, by number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskRun {
    pub run: u64,
    pub role: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub outcome: Option<RunOutcome>,
    pub summary: Option<String>,
    pub changed_files: Vec<String>,
}

/// The bundle by reference: the targets, the budget, `spec bundle`'s hash
/// now (`null` when it cannot be assembled).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageBundle {
    pub node_ids: Vec<String>,
    pub budget: u32,
    pub bundle_hash: Option<String>,
}

/// The task package: `spec task show --json`, MCP `get_task`'s
/// `structuredContent`. One database and tree state give byte-identical
/// JSON: no read time inside.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskPackage {
    pub schema_version: u32,
    pub id: String,
    pub project: String,
    pub status: TaskStatus,
    pub title: Option<String>,
    pub goal: Option<String>,
    /// `[project] profile`, verbatim; never branched on.
    pub profile: Option<String>,
    /// Per read, never stored: a snapshot node changed or gone in the
    /// compared place; `null` with a note when it cannot be told.
    pub stale: Option<bool>,
    pub targets: Vec<PackageTarget>,
    pub criteria: Vec<TaskCriterion>,
    pub affected_nodes: Vec<String>,
    pub plan: Option<String>,
    pub assumptions: Vec<PackageAssumption>,
    pub open_proposals: Vec<PackageProposal>,
    pub owner_notes: Vec<OwnerNote>,
    pub bindings: Vec<PackageBinding>,
    pub spec_snapshot: Option<SpecSnapshot>,
    /// `[]` when not stale, `null` when staleness cannot be told.
    pub snapshot_diff: Option<Vec<SnapshotDiff>>,
    pub claim: Option<TaskClaim>,
    pub runs: Vec<TaskRun>,
    pub bundle: PackageBundle,
    pub author: TaskAuthor,
    pub created_at: String,
    pub updated_at: String,
    pub notes: Vec<String>,
}
