//! A question round (canon `round`; task spec `round`): the
//! owner's open questions and discrepancies on one sheet, answered in one
//! sitting, each answer still confirmed by its own `[y/N]`.
//!
//! - **`spec round new [--limit N]`** ([`round_new`]): read-only. The
//!   current repository's `open` items whose kind decides (no change
//!   proposal), in round order (core [`specengine_core::round`]: severity,
//!   the waiting tasks, `created_at`, the ID's number; a task waits on an
//!   item when its package's `open_proposals` would list it, it being
//!   neither `done` nor `cancelled`). Held items, open change proposals,
//!   another repository's items, a gone repository's, corrupt rows and the
//!   items past `--limit` are notes, never items. One database and tree
//!   state give the same bytes: no clock, no relative age, no read time.
//!   The sheet is Markdown; every text of the queue, the index or a task
//!   is escaped and never starts a line (a multi-line text indented two
//!   spaces, a one-line slot after a frame word with LF and CR written
//!   `\n`, `\r`), so no agent text can plant an answer line.
//! - **`spec round answer FILE`** ([`round_answer`]): `main` has refused a
//!   stdin that is no terminal before this runs. The file (a regular file,
//!   its size read before any byte), the grammar, the project line (else
//!   exit 2, nothing staged), then each answer checked against the queue
//!   now and refused alone, named: its line and why. What passes is staged
//!   in round order as the daemon's decision route stages
//!   ([`crate::stage::stage_with`], one `proposal.staged` each), reported,
//!   then walked: each staged item's own `spec approve|reject` with no
//!   typed flag, asked through the [`RoundTerminal`]; `y` decides it, any
//!   other answer keeps the stage, the end of input keeps the rest. A stage
//!   replaced since this run staged it is refused before its question, and
//!   one replaced while it is asked is refused by step 7's compare.
//! - **`spec round answer --staged`**: the walk alone, over the current
//!   repository's open deciding items holding a stage, in round order; a
//!   staged change proposal, and another or a gone repository's staged
//!   item, is named in a note, never asked (a gone repository's item kept
//!   staged names `spec reject <ID>` alone).
//! - [`round_stage`]: the answers of a text checked and staged, no walk
//!   (the daemon's later route).
//!
//! The core is domain-free: the round names no node kind and no
//! ID prefix of a project; texts and titles travel verbatim.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::path::PathBuf;

use serde::Serialize;
use specengine_core::check::CheckInput;
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_core::round::{
    AnswerLine, Answers, OrderKey, PROJECT_LINE, ROUND_FILE_MAX_BYTES, ROUND_LIMIT_DEFAULT,
    ROUND_LIMIT_MAX, Refusal, Token, answers_text, order_key, parse_answers, reading_minutes,
};
use specengine_store::{
    GitEnv, Proposal, ProposalFilter, ProposalQueue as _, ProposalStatus, Stage, StagedChoice,
    Task, same_repository,
};

use crate::apply::{ApproveRequest, RejectRequest, approve, reject};
use crate::corpus::indexed;
use crate::decide::record_ready;
use crate::package::{lists_proposal, position, wanted_nodes};
use crate::project::ProjectRoot;
use crate::proposals::{
    ProposalOutcome, QueueContext, checked_now, elsewhere, escaped_error, no_proposal,
    open_context, queue_cannot, render_text as proposal_text, repository_gone,
};
use crate::propose::{is_path_target, resolved_node};
use crate::stage::{StageBody, StageRequest, stage_with, staged_command};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The most IDs a note names; the rest are counted.
const IDS_NAMED_MAX: usize = 10;

/// The most line numbers a refusal names; the rest are counted.
const LINES_NAMED_MAX: usize = 10;

/// The most waiting tasks an item's `Waiting:` line names.
const WAITING_NAMED_MAX: usize = 10;

/// `spec round new` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundNewRequest {
    /// `--limit N`: 1 to [`ROUND_LIMIT_MAX`]; `None`: [`ROUND_LIMIT_DEFAULT`].
    pub limit: Option<i64>,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// An item's target, resolved now in the current root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundTarget {
    /// `null` for a path target.
    pub id: Option<String>,
    /// Its holder; a gone ID's `null` unless it is the item's first target.
    pub path: Option<String>,
    /// `null` when gone or untitled.
    pub title: Option<String>,
    /// Found now.
    #[serde(skip)]
    pub found: bool,
}

/// A task waiting on an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundTask {
    pub id: String,
    pub status: String,
    pub title: Option<String>,
}

/// One item of a sheet: 19 keys, in this order, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundItem {
    pub id: String,
    pub kind: String,
    pub severity: Option<IntakeSeverity>,
    pub gap_type: Option<GapType>,
    pub summary: String,
    pub working_answer: Option<String>,
    pub price_of_other: Option<String>,
    pub options: Vec<IntakeOption>,
    pub recommendation: Option<u64>,
    pub evidence: Vec<Evidence>,
    pub targets: Vec<RoundTarget>,
    /// Every waiting task, by ID.
    pub tasks: Vec<RoundTask>,
    pub worktree: String,
    pub branch: String,
    pub author: Author,
    pub staged: Option<Stage>,
    pub staged_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// What `spec round new` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundSheet {
    pub project: String,
    /// In round order, at most the limit.
    pub items: Vec<RoundItem>,
    /// The items past the limit.
    pub more: usize,
    /// Minutes of reading of the items listed.
    pub minutes: usize,
    /// Printed as `note:` lines at the end of the sheet.
    pub notes: Vec<String>,
    /// The index's warnings (stderr).
    #[serde(skip)]
    pub messages: Vec<Message>,
    /// Each item's lines on the sheet, rendered once by [`round_new`] (the
    /// reading time counts them).
    #[serde(skip)]
    texts: Vec<String>,
}

/// `spec round new`: the sheet (see the module documentation). Nothing is
/// written but the data directory.
pub fn round_new(
    env: &Env,
    globals: &Globals,
    request: &RoundNewRequest,
) -> Result<RoundSheet, CliError> {
    run_new(env, globals, request).map_err(escaped_error)
}

fn run_new(
    env: &Env,
    globals: &Globals,
    request: &RoundNewRequest,
) -> Result<RoundSheet, CliError> {
    let limit = match request.limit {
        None => ROUND_LIMIT_DEFAULT,
        Some(limit) => usize::try_from(limit)
            .ok()
            .filter(|limit| (1..=ROUND_LIMIT_MAX).contains(limit))
            .ok_or_else(|| {
                CliError::spec(format!(
                    "--limit {limit}: the limit is 1 to {ROUND_LIMIT_MAX}"
                ))
            })?,
    };
    let context = open_context(env, globals, &request.git)?;
    let mut notes = Vec::new();
    let waiting = Waiting::load(&context, &mut notes)?;
    let listed = context
        .queue
        .list_readable(&ProposalFilter {
            git_common_dir: None,
            statuses: vec![ProposalStatus::Open, ProposalStatus::Approved],
        })
        .map_err(queue_cannot)?;
    let mut items = Vec::new();
    let (mut held, mut changes, mut others, mut orphans) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for proposal in listed.proposals {
        let ours = same_repository(&proposal.place.git_common_dir, &context.common_dir);
        let open = proposal.status == ProposalStatus::Open;
        if !proposal.kind.decides() {
            if ours && open {
                changes.push(proposal.id);
            }
        } else if !ours {
            if repository_gone(&proposal) {
                orphans.push(proposal.id);
            } else {
                others.push(proposal.id);
            }
        } else if open {
            items.push(proposal);
        } else {
            held.push(proposal.id);
        }
    }
    let mut ordered: Vec<(OrderKey, Proposal, Vec<&Task>)> = items
        .into_iter()
        .map(|proposal| {
            let tasks = waiting.of(&proposal);
            (round_key(&proposal, tasks.len()), proposal, tasks)
        })
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let total = ordered.len();
    let more = total.saturating_sub(limit);

    let mut sheet_notes = Vec::new();
    if more > 0 {
        sheet_notes.push(format!(
            "{more} more item(s) not on this sheet: spec round new --limit {}",
            total.min(ROUND_LIMIT_MAX)
        ));
    }
    if !held.is_empty() {
        sheet_notes.push(format!(
            "{} held item(s) not on this sheet (approved: an apply in progress or stopped): {}; \
             spec approve <ID> completes each",
            held.len(),
            named(&held)
        ));
    }
    if !changes.is_empty() {
        sheet_notes.push(format!(
            "{} open change proposal(s) not on this sheet: {}; spec inbox lists them",
            changes.len(),
            named(&changes)
        ));
    }
    if !others.is_empty() {
        sheet_notes.push(format!(
            "{} item(s) of another repository of the project `{}` not on this sheet: {}; run \
             `spec round new` there",
            others.len(),
            context.slug,
            named(&others)
        ));
    }
    if !orphans.is_empty() {
        sheet_notes.push(format!(
            "{} item(s) of a repository that no longer exists not on this sheet: {}; spec reject \
             <ID> --reason ... takes one out",
            orphans.len(),
            named(&orphans)
        ));
    }
    sheet_notes.extend(
        listed
            .unreadable
            .iter()
            .map(|row| one_line(&format!("{row}; not listed"))),
    );
    sheet_notes.extend(notes);

    // The index refreshed, as by every read: the targets' titles now.
    let mut messages = Vec::new();
    let input = indexed(env, &context.project, &mut messages, false)?;
    let items: Vec<RoundItem> = ordered
        .iter()
        .take(limit)
        .map(|(_, proposal, tasks)| item(&context.project, &input, proposal, tasks))
        .collect();
    let texts: Vec<String> = items.iter().map(item_text).collect();
    let chars: usize = texts.iter().map(|text| text.chars().count()).sum();
    Ok(RoundSheet {
        project: context.slug.clone(),
        items,
        more,
        minutes: reading_minutes(chars),
        notes: sheet_notes,
        messages,
        texts,
    })
}

/// The current repository's tasks neither `done` nor `cancelled`, by ID,
/// each with the nodes its package lists proposals against
/// ([`wanted_nodes`]), read once.
struct Waiting {
    tasks: Vec<(Task, BTreeSet<String>)>,
}

impl Waiting {
    /// Read once; a row that does not decode is a note.
    fn load(context: &QueueContext, notes: &mut Vec<String>) -> Result<Self, CliError> {
        let listed = context.queue.list_tasks(None).map_err(queue_cannot)?;
        notes.extend(
            listed
                .unreadable
                .iter()
                .map(|row| one_line(&format!("{row}; not counted among the waiting tasks"))),
        );
        let tasks = listed
            .tasks
            .into_iter()
            .filter(|task| {
                !task.status.is_closed()
                    && same_repository(&task.git_common_dir, &context.common_dir)
            })
            .map(|task| {
                let wanted = wanted_nodes(&task).into_iter().map(str::to_owned).collect();
                (task, wanted)
            })
            .collect();
        Ok(Self { tasks })
    }

    /// The tasks whose package would list `proposal` (canon `task-package`,
    /// "Package", read the other way).
    fn of(&self, proposal: &Proposal) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|(task, wanted)| lists_proposal(task, wanted, proposal))
            .map(|(task, _)| task)
            .collect()
    }
}

/// `proposal`'s round order key with `waiting` tasks.
fn round_key(proposal: &Proposal, waiting: usize) -> OrderKey {
    order_key(
        proposal.intake.as_ref().map(|intake| intake.severity),
        waiting,
        &proposal.created_at,
        &proposal.id,
    )
}

/// `ids`, at most [`IDS_NAMED_MAX`], the rest counted.
fn named(ids: &[String]) -> String {
    let mut text = ids
        .iter()
        .take(IDS_NAMED_MAX)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if ids.len() > IDS_NAMED_MAX {
        text.push_str(&format!(" and {} more", ids.len() - IDS_NAMED_MAX));
    }
    text
}

/// One sheet item of `proposal`, its targets resolved in `project` over
/// `input`, `tasks` waiting on it.
fn item(
    project: &ProjectRoot,
    input: &CheckInput,
    proposal: &Proposal,
    tasks: &[&Task],
) -> RoundItem {
    let intake = proposal.intake.as_ref();
    let targets = proposal
        .target_ids()
        .iter()
        .map(|stored| target(project, input, proposal, stored))
        .collect();
    let staged = proposal.staged.as_ref();
    RoundItem {
        id: proposal.id.clone(),
        kind: proposal.kind.as_str().to_owned(),
        severity: intake.map(|intake| intake.severity),
        gap_type: intake.and_then(|intake| intake.gap_type),
        summary: intake
            .map(|intake| intake.summary.clone())
            .unwrap_or_default(),
        working_answer: intake.and_then(|intake| intake.working_answer.clone()),
        price_of_other: intake.and_then(|intake| intake.price_of_other.clone()),
        options: intake
            .map(|intake| intake.options.clone())
            .unwrap_or_default(),
        recommendation: intake.and_then(|intake| intake.recommendation),
        evidence: intake
            .map(|intake| intake.evidence.clone())
            .unwrap_or_default(),
        targets,
        tasks: tasks
            .iter()
            .map(|task| RoundTask {
                id: task.id.clone(),
                status: task.status.as_str().to_owned(),
                title: task.title.clone(),
            })
            .collect(),
        worktree: proposal.place.worktree.clone(),
        branch: proposal.place.branch.clone(),
        author: proposal.author.clone(),
        staged: staged.map(|staged| staged.stage.clone()),
        staged_at: staged.map(|staged| staged.at.clone()),
        created_at: proposal.created_at.clone(),
        updated_at: proposal.updated_at.clone(),
    }
}

/// The stored target `stored` of `proposal`, resolved now.
fn target(
    project: &ProjectRoot,
    input: &CheckInput,
    proposal: &Proposal,
    stored: &str,
) -> RoundTarget {
    let path_target = is_path_target(stored);
    let id = (!path_target).then(|| stored.to_owned());
    match resolved_node(project, input, stored) {
        Ok(Ok((canonical, path))) => {
            let title = input
                .files
                .iter()
                .find(|file| file.path == path)
                .and_then(|file| file.parsed.as_ref())
                .and_then(|parsed| {
                    let ord = position(parsed, &canonical)?;
                    parsed.nodes.get(ord)?.title.clone()
                });
            RoundTarget {
                id,
                path: Some(path),
                title,
                found: true,
            }
        }
        _ => {
            let path = if path_target {
                Some(stored.to_owned())
            } else if stored == proposal.target_id {
                Some(proposal.target_path.clone())
            } else {
                None
            };
            RoundTarget {
                id,
                path,
                title: None,
                found: false,
            }
        }
    }
}

/// One line of text of the queue, the index or a task, for the sheet:
/// control characters and bidirectional marks escaped, U+2028 and U+2029
/// written `\u{2028}`, `\u{2029}`.
fn shown(text: &str) -> String {
    escape_controls(text)
        .replace('\u{2028}', "\\u{2028}")
        .replace('\u{2029}', "\\u{2029}")
}

/// A one-line slot: LF and CR written `\n`, `\r`, then [`shown`].
fn slot(text: &str) -> String {
    shown(&text.replace('\n', "\\n").replace('\r', "\\r"))
}

/// A multi-line text, one line per LF, each indented two spaces.
fn indented(out: &mut String, text: &str) {
    for line in text.split('\n') {
        out.push_str("  ");
        out.push_str(&shown(line));
        out.push('\n');
    }
}

/// An item's lines on the sheet, its heading through its answer slot.
fn item_text(item: &RoundItem) -> String {
    let mut out = format!("## {} - {}", slot(&item.id), slot(&item.kind));
    if let Some(severity) = item.severity {
        out.push_str(&format!(", {}", severity.as_str()));
    }
    out.push_str("\n\n");
    indented(&mut out, &item.summary);
    if let Some(answer) = &item.working_answer {
        out.push_str("\nWorking answer:\n");
        indented(&mut out, answer);
    }
    if let Some(price) = &item.price_of_other {
        out.push_str("\nAnother answer costs:\n");
        indented(&mut out, price);
    }
    if !item.options.is_empty() {
        out.push_str("\nOptions:\n");
        for (index, option) in item.options.iter().enumerate() {
            let recommended =
                item.recommendation.and_then(|at| usize::try_from(at).ok()) == Some(index);
            out.push_str(&format!(
                "  - [{index}] {} | {} | {}{}\n",
                slot(&option.label),
                slot(&option.effect),
                slot(&option.price),
                if recommended { " (recommended)" } else { "" }
            ));
        }
    }
    out.push('\n');
    let mut tail = Vec::new();
    if !item.targets.is_empty() {
        let concerns: Vec<String> = item.targets.iter().map(concern).collect();
        tail.push(format!("Concerns: {}", concerns.join("; ")));
    }
    if !item.tasks.is_empty() {
        let mut named: Vec<String> = item
            .tasks
            .iter()
            .take(WAITING_NAMED_MAX)
            .map(|task| match &task.title {
                Some(title) => format!("{} ({})", slot(title), slot(&task.id)),
                None => slot(&task.id),
            })
            .collect();
        if item.tasks.len() > WAITING_NAMED_MAX {
            named.push(format!("{} more", item.tasks.len() - WAITING_NAMED_MAX));
        }
        tail.push(format!("Waiting: {}", named.join("; ")));
    }
    if let (Some(stage), Some(at)) = (&item.staged, &item.staged_at) {
        tail.push(format!(
            "Staged {}: {}",
            slot(at),
            shown(&staged_command(&item.id, stage))
        ));
    }
    if !tail.is_empty() {
        for line in tail {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}:\n", slot(&item.id)));
    out
}

/// A target on the `Concerns:` line: `<title> (<ID or path>)`; gone:
/// `<ID or path> (not found now)`.
fn concern(target: &RoundTarget) -> String {
    let name = slot(
        target
            .id
            .as_deref()
            .or(target.path.as_deref())
            .unwrap_or_default(),
    );
    match (&target.title, target.found) {
        (_, false) => format!("{name} (not found now)"),
        (Some(title), true) => format!("{} ({name})", slot(title)),
        (None, true) => name,
    }
}

/// The sheet's Markdown (see the module documentation).
pub(crate) fn sheet_text(sheet: &RoundSheet) -> String {
    let slug = slot(&sheet.project);
    let mut out = format!("# Open questions: {slug}\n\n{PROJECT_LINE} {slug}\n\n");
    match sheet.items.first() {
        None => out.push_str("No open questions or discrepancies.\n"),
        Some(first) => {
            let count = sheet.items.len();
            out.push_str(&format!(
                "{count} {}, about {} min of reading. Answer each on a line of its own, at the \
                 start of the line: `{}: ok`. ok takes the working answer, a number takes that \
                 option, `reject: <reason>` rejects, ? or nothing leaves it open, other text is \
                 your own answer to a question. Keep the project line. Then, in a terminal: spec \
                 round answer <file>; each answer is asked again there, one at a time.\n",
                if count == 1 { "item" } else { "items" },
                sheet.minutes,
                slot(&first.id)
            ));
            for text in &sheet.texts {
                out.push('\n');
                out.push_str(text);
            }
        }
    }
    if !sheet.notes.is_empty() {
        out.push('\n');
        for note in &sheet.notes {
            out.push_str(&format!("note: {}\n", slot(note)));
        }
    }
    out
}

/// The owner's terminal during `spec round answer` (`main`: stdout, and a
/// `[y/N]` prompt on stderr read from the terminal).
pub trait RoundTerminal {
    /// The report, once, before the first question (text: stdout).
    fn report(&mut self, text: &str);
    /// One item's question: `Some(true)` only for `y` or `yes`; `None` at
    /// the end of input.
    fn ask(&mut self, question: &str) -> Option<bool>;
    /// One line of the walk (stderr): a decided item, one kept or refused.
    fn tell(&mut self, line: &str);
}

/// `spec round answer` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundAnswerRequest {
    /// `FILE`, relative to the current directory; `None`: `--staged`.
    pub file: Option<PathBuf>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`, the stages' `staged_at`
    /// and the decisions' time.
    pub now: String,
    pub git: GitEnv,
}

/// [`round_stage`] options: the answers as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundStageRequest {
    pub text: String,
    pub now: String,
    pub git: GitEnv,
}

/// What became of one answer line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AnswerOutcome {
    Staged,
    Open,
    Refused,
}

/// What became of one walked item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WalkOutcome {
    Applied,
    Rejected,
    Kept,
    Refused,
}

/// One answer line, in file order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundAnswer {
    /// As written; a line that is no answer line: the ID it reads as.
    pub id: String,
    pub line: usize,
    pub answer: String,
    pub outcome: AnswerOutcome,
    /// The stage stored for it; `null` unless staged.
    pub stage: Option<Stage>,
    /// Why it was refused.
    pub reason: Option<String>,
    /// A staged one's report words after `staged: `; empty otherwise (the
    /// report words the others by their outcome and reason).
    #[serde(skip)]
    pub said: String,
}

/// One walked item, in round order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundStep {
    pub id: String,
    pub outcome: WalkOutcome,
    /// The `spec: apply` commit of an applied one.
    pub commit: Option<String>,
    /// Its decision record's ID.
    pub record: Option<String>,
    /// Why it was kept or refused.
    pub reason: Option<String>,
}

/// What `spec round answer` (or [`round_stage`]) did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundRun {
    pub project: String,
    pub answers: Vec<RoundAnswer>,
    pub walk: Vec<RoundStep>,
    pub notes: Vec<String>,
    /// The notes as `note:` lines (stderr).
    #[serde(skip)]
    pub messages: Vec<Message>,
    /// The report still to print with the outcome's text; `None` once
    /// [`RoundTerminal::report`] has shown it, or with no file.
    #[serde(skip)]
    pub report: Option<String>,
    /// The walk ran: its summary closes the text.
    #[serde(skip)]
    pub walked: bool,
}

impl RoundRun {
    /// 0 when nothing was refused or kept; else 1.
    pub fn exit(&self) -> Exit {
        let refused = self
            .answers
            .iter()
            .any(|answer| answer.outcome == AnswerOutcome::Refused);
        let unsettled = self
            .walk
            .iter()
            .any(|step| matches!(step.outcome, WalkOutcome::Kept | WalkOutcome::Refused));
        if refused || unsettled {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

/// `spec round answer FILE | --staged` (see the module documentation):
/// `main` has checked that stdin is a terminal. Exit 2 ([`CliError`]) only
/// for the file, the project line or what keeps any command from running;
/// every answer or item refused alone is named in the run.
pub fn round_answer(
    env: &Env,
    globals: &Globals,
    request: &RoundAnswerRequest,
    terminal: &mut dyn RoundTerminal,
) -> Result<RoundRun, CliError> {
    run_answer(env, globals, request, terminal).map_err(escaped_error)
}

/// The answers of `request.text` checked against the queue now and staged,
/// with no walk (canon `round`, "Run", steps 2–4).
pub fn round_stage(
    env: &Env,
    globals: &Globals,
    request: &RoundStageRequest,
) -> Result<RoundRun, CliError> {
    run_stage(env, globals, request).map_err(escaped_error)
}

fn run_stage(
    env: &Env,
    globals: &Globals,
    request: &RoundStageRequest,
) -> Result<RoundRun, CliError> {
    let now = checked_now(&request.now)?;
    let mut context = open_context(env, globals, &request.git)?;
    let mut notes = Vec::new();
    let waiting = Waiting::load(&context, &mut notes)?;
    let (mut run, _) = staged_answers(
        env,
        &mut context,
        &waiting,
        &request.text,
        None,
        now,
        &request.git,
    )?;
    run.notes.extend(notes);
    run.messages = run.notes.iter().cloned().map(Message::Note).collect();
    run.report = Some(report_text(&run));
    Ok(run)
}

fn run_answer(
    env: &Env,
    globals: &Globals,
    request: &RoundAnswerRequest,
    terminal: &mut dyn RoundTerminal,
) -> Result<RoundRun, CliError> {
    let now = checked_now(&request.now)?;
    let text = match &request.file {
        Some(file) => Some((read_answers(env, file)?, file.display().to_string())),
        None => None,
    };
    let mut context = open_context(env, globals, &request.git)?;
    let mut notes = Vec::new();
    let waiting = Waiting::load(&context, &mut notes)?;
    let (mut run, walked) = match &text {
        Some((text, label)) => {
            let (run, walked) = staged_answers(
                env,
                &mut context,
                &waiting,
                text,
                Some(label),
                now,
                &request.git,
            )?;
            terminal.report(&report_text(&run));
            (run, walked)
        }
        None => staged_now(&context, &waiting)?,
    };
    run.notes.extend(notes);
    run.walk = walk(env, globals, &context, &walked, now, &request.git, terminal);
    run.walked = true;
    run.messages = run.notes.iter().cloned().map(Message::Note).collect();
    Ok(run)
}

/// The returned file's text (canon `round`, "Answers"): a regular file of
/// at most [`ROUND_FILE_MAX_BYTES`], its size read before any byte, UTF-8;
/// else exit 2 naming the file (and the line).
fn read_answers(env: &Env, file: &Path) -> Result<String, CliError> {
    let label = file.display();
    let path = env.cwd.join(file);
    let cannot =
        |error: std::io::Error| CliError::spec(format!("{label}: cannot read it: {error}"));
    let not_regular = || {
        CliError::spec(format!(
            "{label}: not a regular file: give the filled sheet or a reply as a file"
        ))
    };
    let over = |bytes: u64| {
        CliError::spec(format!(
            "{label}: {bytes} bytes; at most {ROUND_FILE_MAX_BYTES}: return only the answer lines"
        ))
    };
    let cap = u64::try_from(ROUND_FILE_MAX_BYTES).unwrap_or(u64::MAX);
    let meta = fs::metadata(&path).map_err(cannot)?;
    if !meta.is_file() {
        return Err(not_regular());
    }
    if meta.len() > cap {
        return Err(over(meta.len()));
    }
    // Opened without waiting (a FIFO put in its place since never holds the
    // open), then judged again by its handle before any byte is read.
    let opened = open_nonblocking(&path).map_err(cannot)?;
    let meta = opened.metadata().map_err(cannot)?;
    if !meta.is_file() {
        return Err(not_regular());
    }
    if meta.len() > cap {
        return Err(over(meta.len()));
    }
    let mut bytes = Vec::new();
    opened
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(cannot)?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > cap {
        return Err(over(read));
    }
    answers_text(&bytes)
        .map(str::to_owned)
        .map_err(|line| CliError::spec(format!("{label}:{line}: not UTF-8")))
}

/// `O_NONBLOCK` of `open(2)`, the kernels' ABI value (this crate takes no
/// `libc`); 0 on a system not listed here, where the metadata read before
/// the open stays the only guard against a FIFO.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
const O_NONBLOCK: i32 = 0x0004;
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "s390x",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "loongarch64"
    )
))]
const O_NONBLOCK: i32 = 0o4000;
#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
        all(
            any(target_os = "linux", target_os = "android"),
            any(
                target_arch = "x86",
                target_arch = "x86_64",
                target_arch = "arm",
                target_arch = "aarch64",
                target_arch = "riscv64",
                target_arch = "s390x",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "loongarch64"
            )
        )
    ))
))]
const O_NONBLOCK: i32 = 0;

/// `path` opened for reading without waiting for a writer.
#[cfg(unix)]
fn open_nonblocking(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK)
        .open(path)
}

/// `path` opened for reading.
#[cfg(not(unix))]
fn open_nonblocking(path: &Path) -> std::io::Result<fs::File> {
    fs::File::open(path)
}

/// An item the walk asks: its stage as this run staged or found it, and
/// the time of a stage it replaced.
struct Walked {
    id: String,
    expected: StagedChoice,
    replaced: Option<String>,
    /// Of a repository that no longer exists: `--staged` never asks it.
    orphan: bool,
}

/// Steps 2–4 over `text` (`label`: its file): the grammar, the project
/// line (exit 2), each answer checked against the queue now, the passing
/// ones staged in round order; the run without its walk, and what the walk
/// asks.
fn staged_answers(
    env: &Env,
    context: &mut QueueContext,
    waiting: &Waiting,
    text: &str,
    label: Option<&str>,
    now: &str,
    git: &GitEnv,
) -> Result<(RoundRun, Vec<Walked>), CliError> {
    let parsed = parse_answers(text);
    project_checked(&parsed, &context.slug, label)?;
    let mut ready = Ready::default();
    let mut planned = Vec::new();
    let mut refusals = Vec::with_capacity(parsed.answers.len());
    for (index, line) in parsed.answers.iter().enumerate() {
        refusals.push(match plan(context, git, line, &mut ready) {
            Ok(None) => None,
            Ok(Some((proposal, body))) => {
                let key = round_key(&proposal, waiting.of(&proposal).len());
                planned.push((key, index, proposal, body));
                None
            }
            Err(reason) => Some(reason),
        });
    }
    // The parsed lines' texts moved, never copied: a long file holds each
    // once.
    let mut answers: Vec<RoundAnswer> = parsed
        .answers
        .into_iter()
        .zip(refusals)
        .map(|(line, refusal)| {
            let mut answer = RoundAnswer {
                id: line.id,
                line: line.line,
                answer: line.answer,
                outcome: AnswerOutcome::Open,
                stage: None,
                reason: None,
                said: String::new(),
            };
            if let Some(reason) = refusal {
                refuse(&mut answer, &reason);
            }
            answer
        })
        .collect();
    planned.sort_by(|a, b| a.0.cmp(&b.0));
    let mut walked = Vec::new();
    for (_, index, proposal, body) in planned {
        let request = StageRequest {
            id: proposal.id.clone(),
            body,
            now: now.to_owned(),
            git: git.clone(),
        };
        let outcome = match stage_with(env, context, &request) {
            Ok(outcome) => outcome,
            Err(error) => {
                refuse(&mut answers[index], unprefixed(&error.message));
                continue;
            }
        };
        let document = &outcome.proposal.document;
        let (Some(stage), Some(at), None) = (&document.staged, &document.staged_at, outcome.cause)
        else {
            let reason = outcome
                .proposal
                .refusal
                .as_deref()
                .unwrap_or("the stage was not stored");
            refuse(&mut answers[index], reason);
            continue;
        };
        let mut said = staged_what(&proposal, stage);
        if let Some(previous) = &proposal.staged {
            said.push_str(&format!("; replaces the choice staged {}", previous.at));
        }
        let answer = &mut answers[index];
        answer.outcome = AnswerOutcome::Staged;
        answer.stage = Some(stage.clone());
        answer.said = said;
        walked.push(Walked {
            id: proposal.id.clone(),
            expected: StagedChoice {
                stage: stage.clone(),
                at: at.clone(),
            },
            replaced: proposal.staged.as_ref().map(|previous| previous.at.clone()),
            orphan: !same_repository(&proposal.place.git_common_dir, &context.common_dir),
        });
    }
    let run = RoundRun {
        project: context.slug.clone(),
        answers,
        walk: Vec::new(),
        notes: Vec::new(),
        messages: Vec::new(),
        report: None,
        walked: false,
    };
    Ok((run, walked))
}

/// `--staged`: the current repository's open deciding items holding a
/// stage, in round order; a staged change proposal, and another or a gone
/// repository's staged item, noted, never asked.
fn staged_now(
    context: &QueueContext,
    waiting: &Waiting,
) -> Result<(RoundRun, Vec<Walked>), CliError> {
    let listed = context
        .queue
        .list_readable(&ProposalFilter {
            git_common_dir: None,
            statuses: vec![ProposalStatus::Open],
        })
        .map_err(queue_cannot)?;
    let mut items = Vec::new();
    let (mut changes, mut others, mut orphans) = (Vec::new(), Vec::new(), Vec::new());
    for proposal in listed.proposals {
        if proposal.staged.is_none() {
            continue;
        }
        let ours = same_repository(&proposal.place.git_common_dir, &context.common_dir);
        if !proposal.kind.decides() {
            if ours {
                changes.push(proposal.id);
            }
        } else if ours {
            items.push((round_key(&proposal, waiting.of(&proposal).len()), proposal));
        } else if repository_gone(&proposal) {
            orphans.push(proposal.id);
        } else {
            others.push(proposal.id);
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let mut notes = Vec::new();
    if !changes.is_empty() {
        notes.push(format!(
            "{} staged change proposal(s) not asked: {}; spec approve <ID> confirms each",
            changes.len(),
            named(&changes)
        ));
    }
    if !others.is_empty() {
        notes.push(format!(
            "{} staged item(s) of another repository of the project `{}` not asked: {}; run `spec \
             round answer --staged` there",
            others.len(),
            context.slug,
            named(&others)
        ));
    }
    if !orphans.is_empty() {
        notes.push(format!(
            "{} staged item(s) of a repository that no longer exists not asked: {}; spec reject \
             <ID> confirms each",
            orphans.len(),
            named(&orphans)
        ));
    }
    notes.extend(
        listed
            .unreadable
            .iter()
            .map(|row| one_line(&format!("{row}; not asked"))),
    );
    let walked = items
        .into_iter()
        .filter_map(|(_, proposal)| {
            Some(Walked {
                expected: proposal.staged?,
                id: proposal.id,
                replaced: None,
                orphan: false,
            })
        })
        .collect();
    let run = RoundRun {
        project: context.slug.clone(),
        answers: Vec::new(),
        walk: Vec::new(),
        notes,
        messages: Vec::new(),
        report: None,
        walked: false,
    };
    Ok((run, walked))
}

/// The project line: exactly one, naming `slug`; else exit 2, nothing
/// staged.
fn project_checked(parsed: &Answers, slug: &str, label: Option<&str>) -> Result<(), CliError> {
    let at = |line: Option<usize>| match (label, line) {
        (Some(label), Some(line)) => format!("{label}:{line}: "),
        (Some(label), None) => format!("{label}: "),
        (None, Some(line)) => format!("line {line}: "),
        (None, None) => String::new(),
    };
    match parsed.projects.as_slice() {
        [] => Err(CliError::spec(format!(
            "{}the answers name no project: keep the line `{PROJECT_LINE} {slug}`; nothing \
             changed",
            at(None)
        ))),
        [one] if one.slug == slug => Ok(()),
        [one] => Err(CliError::spec(format!(
            "{}the answers are for `{}`, this is `{slug}`; nothing changed",
            at(Some(one.line)),
            one.slug
        ))),
        many => {
            let lines: Vec<usize> = many.iter().map(|project| project.line).collect();
            Err(CliError::spec(format!(
                "{}the answers name a project on lines {}: keep one line `{PROJECT_LINE} \
                 {slug}`; nothing changed",
                at(None),
                listed(&lines)
            )))
        }
    }
}

/// `4 and 7`, `4, 7 and 9`; past [`LINES_NAMED_MAX`] lines, the first of
/// them and the rest counted (`2, 3, …, 11 and 3990 more`), so a refusal
/// stays one short line however many lines it names.
fn listed(lines: &[usize]) -> String {
    if lines.len() > LINES_NAMED_MAX {
        let named: Vec<String> = lines
            .iter()
            .take(LINES_NAMED_MAX)
            .map(usize::to_string)
            .collect();
        return format!(
            "{} and {} more",
            named.join(", "),
            lines.len() - LINES_NAMED_MAX
        );
    }
    let texts: Vec<String> = lines.iter().map(usize::to_string).collect();
    match texts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// The answer refused: `reason`, its trailing `; nothing changed` dropped.
fn refuse(answer: &mut RoundAnswer, reason: &str) {
    let reason = reason.strip_suffix("; nothing changed").unwrap_or(reason);
    let reason = reason.strip_suffix("; nothing staged").unwrap_or(reason);
    answer.outcome = AnswerOutcome::Refused;
    answer.stage = None;
    answer.said = String::new();
    answer.reason = Some(reason.to_owned());
}

/// A refusal's line without its `spec: `.
fn unprefixed(message: &str) -> &str {
    message.strip_prefix("spec: ").unwrap_or(message)
}

/// The report's words for a stored stage of `proposal`.
fn staged_what(proposal: &Proposal, stage: &Stage) -> String {
    match stage {
        Stage::Reject { .. } => "reject".to_owned(),
        Stage::Approve {
            option: Some(option),
            ..
        } => {
            let label = proposal
                .intake
                .as_ref()
                .zip(usize::try_from(*option).ok())
                .and_then(|(intake, at)| intake.options.get(at))
                .map(|chosen| format!(": {}", slot(&chosen.label)))
                .unwrap_or_default();
            format!("approve, option {option}{label}")
        }
        Stage::Approve {
            answer: Some(_), ..
        } => "approve, your answer".to_owned(),
        Stage::Approve { .. } => "approve, the working answer".to_owned(),
    }
}

/// Apply steps 2 and 3, read-only; a recorded place that passed is not
/// read again (a refusal names its item, so it is made for each).
#[derive(Default)]
struct Ready {
    places: BTreeSet<(String, String, String)>,
}

impl Ready {
    /// Why an approve of `proposal` would stop at step 2 or 3 now.
    fn check(&mut self, git: &GitEnv, slug: &str, proposal: &Proposal) -> Result<(), String> {
        let place = &proposal.place;
        let key = (
            place.worktree.clone(),
            place.branch.clone(),
            place.root_rel.clone(),
        );
        if self.places.contains(&key) {
            return Ok(());
        }
        match record_ready(git, slug, proposal) {
            Ok(()) => {
                self.places.insert(key);
                Ok(())
            }
            Err(failure) => {
                let reason = failure
                    .reason
                    .strip_suffix("; nothing changed")
                    .unwrap_or(&failure.reason);
                Err(format!(
                    "not applied (step {}): {reason}; `reject: <reason>` still settles it",
                    failure.step
                ))
            }
        }
    }
}

/// What `line` asks of the queue: nothing (left open), or the proposal and
/// the stage to store; else why it is refused (canon `round`, "Answers").
fn plan(
    context: &QueueContext,
    git: &GitEnv,
    line: &AnswerLine,
    ready: &mut Ready,
) -> Result<Option<(Proposal, StageBody)>, String> {
    let token = match &line.token {
        Ok(Token::Open) => return Ok(None),
        Ok(token) => token,
        Err(refusal) => return Err(refused_line(line, refusal)),
    };
    let id = &line.id;
    let proposal = match context.queue.get(id) {
        Ok(Some(proposal)) => proposal,
        Ok(None) => return Err(format!("no proposal `{id}` in this project's queue")),
        Err(error) => return Err(error.to_string()),
    };
    let rejects = matches!(token, Token::Reject(_));
    if !same_repository(&proposal.place.git_common_dir, &context.common_dir) {
        let gone = repository_gone(&proposal);
        if !(gone && rejects) {
            return Err(elsewhere(context, &proposal, gone));
        }
    }
    match proposal.status {
        ProposalStatus::Open => {}
        ProposalStatus::Applied => {
            return Err(format!(
                "`{id}` is applied: commit {}",
                proposal.applied_commit.as_deref().unwrap_or("unknown")
            ));
        }
        ProposalStatus::Rejected => return Err(format!("`{id}` is rejected")),
        ProposalStatus::Approved => {
            return Err(format!(
                "`{id}` is held (approved: an apply in progress or stopped): `spec approve {id}` \
                 completes it"
            ));
        }
    }
    if !proposal.kind.decides() {
        return Err(format!(
            "`{id}` is a change proposal ({}): `spec approve {id}` decides it",
            proposal.kind.as_str()
        ));
    }
    let Some(intake) = proposal.intake.as_ref() else {
        return Err(format!(
            "`{id}` holds no fields of a {}",
            proposal.kind.as_str()
        ));
    };
    // An item with options is answered by one of them; one with none by
    // its working answer or the owner's text.
    let count = intake.options.len();
    let range = format!("0-{}", count.saturating_sub(1));
    let updated_at = proposal.updated_at.clone();
    let approve = |option: Option<u64>, answer: Option<String>| StageBody::Approve {
        option,
        answer,
        canon: None,
        note: None,
        updated_at: updated_at.clone(),
    };
    let body = match (token, count) {
        (Token::Reject(reason), _) => StageBody::Reject {
            reason: reason.clone(),
            updated_at: updated_at.clone(),
        },
        (Token::Ok, 0) => approve(None, None),
        (Token::Text(text), 0) => approve(None, Some(text.clone())),
        (Token::Option(_), 0) => {
            return Err(format!(
                "`{id}` has no options: `ok` takes its working answer, other text is your answer"
            ));
        }
        (Token::Option(option), _) => {
            if usize::try_from(*option).map_or(true, |option| option >= count) {
                return Err(format!("option {option}: `{id}` has options {range}"));
            }
            approve(Some(*option), None)
        }
        (Token::Ok | Token::Text(_) | Token::Open, _) => {
            return Err(format!(
                "`{id}` takes one of its options by number, {range}, or `reject: <reason>`"
            ));
        }
    };
    if !rejects {
        if !intake
            .target_ids
            .iter()
            .any(|target| !is_path_target(target))
        {
            return Err(format!(
                "`{id}` names no ID, only a path: its record needs --canon: `spec approve {id} \
                 --canon REF`"
            ));
        }
        ready.check(git, &context.slug, &proposal)?;
    }
    Ok(Some((proposal, body)))
}

/// `text` with the characters that show nothing written `\u{..}`:
/// U+FEFF, the zero-width space and joiners, U+2060 (control characters
/// and bidirectional marks are escaped with the line).
fn visible(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\u{feff}' | '\u{200b}'..='\u{200d}' | '\u{2060}') {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// Why `line`, refused by the grammar, is refused.
fn refused_line(line: &AnswerLine, refusal: &Refusal) -> String {
    match refusal {
        Refusal::NotAnAnswerLine {
            written,
            latin,
            look_alike,
        } => {
            let written = visible(written);
            if *look_alike {
                format!(
                    "`{written}` uses look-alike letters or digits; proposal IDs are Latin only: \
                     write `{latin}:` at the start of the line"
                )
            } else {
                format!("`{written}` is no answer line: write `{latin}:` at the start of the line")
            }
        }
        Refusal::NoProposal => no_proposal(&line.id),
        Refusal::RejectWithoutReason => {
            "`reject` needs a reason: write `reject: <reason>`".to_owned()
        }
        Refusal::RejectWithoutColon => {
            "`reject` takes its reason after a colon right behind it: write `reject: <reason>`"
                .to_owned()
        }
        Refusal::OverCap { reason, bytes, max } => {
            let what = if *reason { "the reason" } else { "the answer" };
            format!("{what}: {bytes} bytes; at most {max}")
        }
        Refusal::Escaped(c) => format!("holds U+{:04X}: an answer never carries it", u32::from(*c)),
        Refusal::Duplicate(lines) => format!(
            "answered on lines {}: give one answer per item",
            listed(lines)
        ),
    }
}

/// Step 5: each item's own `spec approve|reject` with no typed flag, in
/// order; a refusal is named and the walk goes on; the end of input keeps
/// the rest.
fn walk(
    env: &Env,
    globals: &Globals,
    context: &QueueContext,
    items: &[Walked],
    now: &str,
    git: &GitEnv,
    terminal: &mut dyn RoundTerminal,
) -> Vec<RoundStep> {
    let mut steps = Vec::with_capacity(items.len());
    let mut ended = false;
    for item in items {
        let id = &item.id;
        let kept = kept_line(id, &item.expected.stage, item.orphan);
        if ended {
            terminal.tell(&kept);
            steps.push(step(
                id,
                WalkOutcome::Kept,
                Some(format!("end of input: {kept}")),
            ));
            continue;
        }
        let mut asked = None;
        let mut moved = None;
        let mut end = false;
        let outcome = {
            let mut consent = |question: &str| -> bool {
                if let Some(reason) = moved_since(context, id, &item.expected) {
                    moved = Some(reason);
                    return false;
                }
                let mut shown = String::new();
                if let Some(at) = &item.replaced {
                    shown.push_str(&format!(
                        "note: this answer replaces the choice staged {at}\n"
                    ));
                }
                shown.push_str(question);
                match terminal.ask(&shown) {
                    Some(yes) => {
                        asked = Some(yes);
                        yes
                    }
                    None => {
                        end = true;
                        false
                    }
                }
            };
            match &item.expected.stage {
                Stage::Approve { .. } => approve(
                    env,
                    globals,
                    &ApproveRequest {
                        id: id.clone(),
                        note: None,
                        now: now.to_owned(),
                        git: git.clone(),
                    },
                    &mut consent,
                ),
                Stage::Reject { .. } => reject(
                    env,
                    globals,
                    &RejectRequest {
                        id: id.clone(),
                        reason: None,
                        now: now.to_owned(),
                        git: git.clone(),
                    },
                    &mut consent,
                ),
            }
        };
        // A stage replaced before its question: refused, the command's own
        // notes (written for a typed run) left out.
        if let Some(reason) = moved {
            terminal.tell(&escape_controls(&format!("{id}: refused: {reason}")));
            steps.push(step(id, WalkOutcome::Refused, Some(reason)));
            continue;
        }
        let done = match outcome {
            Ok(outcome) => {
                for message in &outcome.messages {
                    terminal.tell(&escape_controls(&message.line()));
                }
                outcome
            }
            Err(error) => {
                let reason = unprefixed(&error.message).to_owned();
                terminal.tell(&escape_controls(&format!("{id}: refused: {reason}")));
                steps.push(step(id, WalkOutcome::Refused, Some(reason)));
                continue;
            }
        };
        steps.push(decided(id, &done, asked, end, &kept, terminal));
        ended = end;
    }
    steps
}

/// One walked item's step from its command's outcome.
fn decided(
    id: &str,
    outcome: &ProposalOutcome,
    asked: Option<bool>,
    end: bool,
    kept: &str,
    terminal: &mut dyn RoundTerminal,
) -> RoundStep {
    match &outcome.refusal {
        None => {
            for line in proposal_text(outcome).lines() {
                terminal.tell(line);
            }
            let document = &outcome.document;
            let rejected = document.status.as_deref() == Some(ProposalStatus::Rejected.as_str());
            RoundStep {
                id: id.to_owned(),
                outcome: if rejected {
                    WalkOutcome::Rejected
                } else {
                    WalkOutcome::Applied
                },
                commit: document.applied_commit.clone(),
                record: document.record_id.clone(),
                reason: None,
            }
        }
        Some(_) if end => {
            terminal.tell(kept);
            step(id, WalkOutcome::Kept, Some(format!("end of input: {kept}")))
        }
        Some(_) if asked == Some(false) => {
            terminal.tell(kept);
            step(id, WalkOutcome::Kept, Some(kept.to_owned()))
        }
        Some(reason) => {
            terminal.tell(&escape_controls(&format!("{id}: refused: {reason}")));
            step(id, WalkOutcome::Refused, Some(reason.clone()))
        }
    }
}

fn step(id: &str, outcome: WalkOutcome, reason: Option<String>) -> RoundStep {
    RoundStep {
        id: id.to_owned(),
        outcome,
        commit: None,
        record: None,
        reason,
    }
}

/// `kept staged: spec approve <ID> or spec round answer --staged confirms
/// it` (a reject: `spec reject <ID>`); an `orphan`'s, which `--staged`
/// never asks: `kept staged: spec reject <ID> confirms it`.
fn kept_line(id: &str, stage: &Stage, orphan: bool) -> String {
    let command = match stage {
        Stage::Approve { .. } => "spec approve",
        Stage::Reject { .. } => "spec reject",
    };
    if orphan {
        return format!("kept staged: {command} {id} confirms it");
    }
    format!("kept staged: {command} {id} or spec round answer --staged confirms it")
}

/// The item `id` still `open` holding another stage than `expected` (the
/// one this run staged or found): the refusal, before its question.
fn moved_since(context: &QueueContext, id: &str, expected: &StagedChoice) -> Option<String> {
    let row = context.queue.get(id).ok().flatten()?;
    (row.status == ProposalStatus::Open && row.staged.as_ref() != Some(expected)).then(|| {
        format!(
            "`{id}` changed since this round staged it: its staged choice was replaced or \
             removed; nothing changed"
        )
    })
}

/// The report: one line per answer line, in file order, then the count.
fn report_text(run: &RoundRun) -> String {
    // Sized once: a long file's report is never copied as it grows.
    let size: usize = run
        .answers
        .iter()
        .map(|answer| {
            answer.id.len() + answer.said.len() + answer.reason.as_ref().map_or(0, String::len) + 48
        })
        .sum();
    let mut out = String::with_capacity(size + 64);
    let (mut staged, mut open, mut refused) = (0, 0, 0);
    for answer in &run.answers {
        let said = match answer.outcome {
            AnswerOutcome::Staged => {
                staged += 1;
                format!("staged: {}", answer.said)
            }
            AnswerOutcome::Open => {
                open += 1;
                "left open".to_owned()
            }
            AnswerOutcome::Refused => {
                refused += 1;
                format!(
                    "refused: {}; nothing staged",
                    answer.reason.as_deref().unwrap_or_default()
                )
            }
        };
        out.push_str(&escape_controls(&one_line(&format!(
            "{} line {}: {said}",
            answer.id, answer.line
        ))));
        out.push('\n');
    }
    out.push_str(&format!(
        "answers: {staged} staged, {open} left open, {refused} refused\n"
    ));
    out
}

/// The run's text: the report when it is still to print, then the walk's
/// summary when the walk ran.
pub(crate) fn render_text(run: &RoundRun) -> String {
    let mut out = run.report.clone().unwrap_or_default();
    if run.walked {
        let count = |outcome: WalkOutcome| {
            run.walk
                .iter()
                .filter(|step| step.outcome == outcome)
                .count()
        };
        out.push_str(&format!(
            "round: {} applied, {} rejected, {} kept staged, {} refused\n",
            count(WalkOutcome::Applied),
            count(WalkOutcome::Rejected),
            count(WalkOutcome::Kept),
            count(WalkOutcome::Refused)
        ));
    }
    out
}
