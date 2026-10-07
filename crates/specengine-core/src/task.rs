//! The engine's own words of a task
//! (`docs/canon/tasks.md` "Commands", "Transitions";
//! `docs/canon/task-package.md` "Caps"): the `T-NNNN`
//! identifier and its look-alikes, the `[ids]` clash with it, the pure
//! state machine [`transition`], and the caps every caller applies. Pure:
//! nothing is read or written here, and no project's prefix, kind, role or
//! stack word appears (ADR-0008, ADR-0027): `T` is the engine's.

use specengine_model::script::normalize_char;
use specengine_model::{IdScheme, RunOutcome, TaskStatus};

/// The prefix of every task ID; a project's `[ids]` may not take it.
pub const TASK_PREFIX: &str = "T";

/// The fewest digits of a task ID: `T-0001`; more past `T-9999`.
pub const TASK_DIGITS: usize = 4;

/// The most bytes of a task's `title`.
pub const TITLE_MAX: usize = 256;
/// The most bytes of a task's `goal`.
pub const GOAL_MAX: usize = 4096;
/// The most bytes of a plan (`plan_md`).
pub const PLAN_MAX: usize = 16_384;
/// The most criteria of a plan.
pub const CRITERIA_MAX: usize = 32;
/// The most bytes of one criterion.
pub const CRITERION_MAX: usize = 1024;
/// The most `--nodes` of a new task.
pub const NODES_MAX: usize = 64;
/// The most `affected_nodes` of a plan.
pub const AFFECTED_MAX: usize = 64;
/// The most nodes a snapshot freezes.
pub const SNAPSHOT_MAX: usize = 128;
/// The most bytes of an owner's note (`changes --note`).
pub const NOTE_MAX: usize = 4096;
/// The most bytes of a run's `summary`.
pub const RUN_SUMMARY_MAX: usize = 4096;
/// The most `changed_files` of a run.
pub const CHANGED_FILES_MAX: usize = 256;
/// The most bytes of one changed file.
pub const CHANGED_FILE_MAX: usize = 512;
/// The most bytes of one node's `snapshot_diff` entry, cut at a line end.
pub const DIFF_MAX: usize = 8192;
/// The most bytes of every `snapshot_diff` entry together; later entries
/// are left out (`diff` `null`, `cut` `true`).
pub const DIFFS_TOTAL_MAX: usize = 262_144;
/// The most bytes of a criterion reference's text in a package, cut at a
/// line end (a note names it): `spec show` reads it whole.
pub const CRITERION_TEXT_MAX: usize = 8192;
/// The characters a package's JSON and its `note:` lines may hold
/// together: past it, the `snapshot_diff` texts, then the criteria
/// references' texts, are left out from the last (a note each kind). With
/// the brief's 40 000 characters it is MCP's result size
/// (`docs/canon/task-package.md` "Caps").
pub const PACKAGE_BUDGET: usize = 460_000;
/// The budget of a task's bundle without `[budgets] bundle_task`, in
/// estimated tokens (05 §6: a task bundle, 10k).
pub const DEFAULT_TASK_BUDGET: u32 = 10_000;

/// The task ID of `number`: `T-` and the number, zero-padded to
/// [`TASK_DIGITS`].
pub fn task_id(number: u64) -> String {
    format!("{TASK_PREFIX}-{number:0width$}", width = TASK_DIGITS)
}

/// The number of a task ID written exactly as [`task_id`] writes it
/// (`T-0001`, `T-10000`; never `T-00001`, `T-1`, `t-0001`).
pub fn task_number(id: &str) -> Option<u64> {
    let digits = id.strip_prefix(TASK_PREFIX)?.strip_prefix('-')?;
    if digits.len() < TASK_DIGITS || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u64 = digits.parse().ok()?;
    (task_id(number) == id).then_some(number)
}

/// Why a written task ID was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskIdError {
    /// Look-alike letters or digits whose Latin form is a task ID: `fix`
    /// (exit 2, naming it).
    LookAlike { fix: String },
    /// Not a task ID in any script.
    NotAnId,
}

/// The task ID `written` names (surrounding whitespace ignored); a
/// look-alike of one is [`TaskIdError::LookAlike`] with the Latin form
/// (ADR-0009: never taken silently).
pub fn parse_task_id(written: &str) -> Result<String, TaskIdError> {
    let written = written.trim();
    if task_number(written).is_some() {
        return Ok(written.to_owned());
    }
    let fix: String = written.chars().map(normalize_char).collect();
    if fix != written && task_number(&fix).is_some() {
        return Err(TaskIdError::LookAlike { fix });
    }
    Err(TaskIdError::NotAnId)
}

/// One line naming the Latin form of a look-alike task ID.
pub fn task_look_alike_message(written: &str, fix: &str) -> String {
    format!("`{written}` uses look-alike letters or digits; task IDs are Latin only: write `{fix}`")
}

/// Why the project's `[ids]` clashes with the engine's [`TASK_PREFIX`]: a
/// configured prefix `T`, or an `aliases_from` entry that reads `T` once
/// its look-alikes are normalised. `None`: no clash.
pub fn task_prefix_clash(scheme: &IdScheme) -> Option<String> {
    for spec in scheme.prefixes() {
        if spec.prefix == TASK_PREFIX {
            return Some(format!(
                "the project's `[ids]` configures the prefix `{TASK_PREFIX}`, which is \
                 SpecEngine's task prefix (`{TASK_PREFIX}-0001`): rename it before using tasks"
            ));
        }
        for alias in &spec.aliases_from {
            let normalized: String = alias.chars().map(normalize_char).collect();
            if normalized == TASK_PREFIX {
                return Some(format!(
                    "the project's `[ids]` lists `{alias}` in `aliases_from` of `{}`, which reads \
                     as SpecEngine's task prefix `{TASK_PREFIX}`: rename it before using tasks",
                    spec.prefix
                ));
            }
        }
    }
    None
}

/// What a command does to a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskAction {
    New,
    Plan,
    Approve,
    Changes,
    Claim,
    Report,
    Complete,
    Cancel,
}

impl TaskAction {
    pub const ALL: [Self; 8] = [
        Self::New,
        Self::Plan,
        Self::Approve,
        Self::Changes,
        Self::Claim,
        Self::Report,
        Self::Complete,
        Self::Cancel,
    ];

    /// The command's name: `spec task <name>`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Plan => "plan",
            Self::Approve => "approve",
            Self::Changes => "changes",
            Self::Claim => "claim",
            Self::Report => "report",
            Self::Complete => "complete",
            Self::Cancel => "cancel",
        }
    }

    /// The states it starts from (none for `new`).
    pub const fn from(self) -> &'static [TaskStatus] {
        use TaskStatus::{ChangesRequested, Draft, InProgress, Ready, Review};
        match self {
            Self::New => &[],
            Self::Plan => &[Draft, ChangesRequested],
            Self::Approve => &[Draft, Review, ChangesRequested, Ready],
            Self::Changes => &[Review],
            Self::Claim => &[Ready],
            Self::Report | Self::Complete => &[InProgress],
            Self::Cancel => &[Draft, Review, ChangesRequested, Ready, InProgress],
        }
    }

    /// The state it leaves the task in.
    pub const fn to(self) -> TaskStatus {
        match self {
            Self::New => TaskStatus::Draft,
            Self::Plan => TaskStatus::Review,
            Self::Approve => TaskStatus::Ready,
            Self::Changes => TaskStatus::ChangesRequested,
            Self::Claim | Self::Report => TaskStatus::InProgress,
            Self::Complete => TaskStatus::Done,
            Self::Cancel => TaskStatus::Cancelled,
        }
    }
}

/// The pure state machine (`docs/canon/tasks.md` "Transitions"): the state
/// `action` leaves a task of state `status` in (`None`: no task yet, only
/// `new`), or the states it needs. `report` (its run open) keeps
/// `in_progress`, `complete` (its run closed) ends it: the runs are the
/// caller's check.
pub fn transition(
    status: Option<TaskStatus>,
    action: TaskAction,
) -> Result<TaskStatus, &'static [TaskStatus]> {
    match (status, action) {
        (None, TaskAction::New) => Ok(TaskStatus::Draft),
        (Some(status), action) if action.from().contains(&status) => Ok(action.to()),
        _ => Err(action.from()),
    }
}

/// ``T-0001 is <status>: `<action>` needs <states>; nothing changed``.
pub fn needs_message(
    id: &str,
    status: TaskStatus,
    action: TaskAction,
    needs: &[TaskStatus],
) -> String {
    format!(
        "{id} is {status}: `{}` needs {}; nothing changed",
        action.as_str(),
        states_text(needs)
    )
}

/// `a`, `a or b`, `a, b or c`.
pub fn states_text(states: &[TaskStatus]) -> String {
    match states {
        [] => "no task".to_owned(),
        [one] => one.as_str().to_owned(),
        [head @ .., last] => format!(
            "{} or {}",
            head.iter()
                .map(|status| status.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            last.as_str()
        ),
    }
}

/// `field: <n> bytes; at most <max>` when `value` is longer than `max`
/// UTF-8 bytes.
pub fn over_bytes(field: &str, value: &str, max: usize) -> Option<String> {
    (value.len() > max).then(|| format!("{field}: {} bytes; at most {max}", value.len()))
}

/// `field: <n> items; at most <max>` when `items` is longer than `max`.
pub fn over_items(field: &str, items: usize, max: usize) -> Option<String> {
    (items > max).then(|| format!("{field}: {items} items; at most {max}"))
}

/// `field: a control character` when `value` holds one (a role, a
/// worktree, a changed file are names, never free text).
pub fn control_problem(field: &str, value: &str) -> Option<String> {
    value
        .chars()
        .any(char::is_control)
        .then(|| format!("{field}: holds a control character"))
}

/// The run outcome `written` names, else why not.
pub fn run_outcome(written: &str) -> Result<RunOutcome, String> {
    RunOutcome::parse(written).ok_or_else(|| {
        let mut names = String::new();
        for (at, outcome) in RunOutcome::ALL.iter().enumerate() {
            if at > 0 {
                names.push_str(if at + 1 == RunOutcome::ALL.len() {
                    " or "
                } else {
                    ", "
                });
            }
            names.push_str(&format!("`{}`", outcome.as_str()));
        }
        format!("outcome: `{written}` is not {names}")
    })
}
