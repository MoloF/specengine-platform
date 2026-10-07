//! What the proposal queue's commands share (task spec `proposal-apply`):
//! `spec propose`, `spec review`, `spec approve` and `spec reject` answer
//! with one review document ([`ProposalDocument`], the later MCP
//! `get_proposal`); `spec inbox` lists ([`crate::inbox`]).
//!
//! - **Place** ([`open_context`]): the project found as every command
//!   finds it, but `--config` must be the root's own `specengine.toml`; an
//!   `[ids]` that takes the engine's `PR` prefix stops every queue command;
//!   the current repository is the git common dir of the project root, git
//!   run with every local `GIT_*` variable of the caller dropped; the queue
//!   lives in the project's database (`<slug>.db`, shared by every
//!   worktree of the repository and every repository of the slug).
//! - **Proposal ID** ([`written_id`]): `PR-` and 4 or more digits, as the
//!   queue writes it; look-alike letters or digits stop the command naming
//!   the Latin form; anything else is "no proposal" (exit 1).
//! - **Repository** ([`find`]): a proposal of another repository of the
//!   same slug stops the command, naming that repository; `spec reject`
//!   alone takes one whose recorded repository no longer exists (moved or
//!   deleted), `open` or `approved` (nothing can be applied any more), so
//!   an orphan can leave the inbox.
//! - **Document**: every key present, absent = `null` (a list `[]`); times
//!   as stored; `diff` the span's base → new hunks of `git diff
//!   --no-index`, headers `--- base <path>` and `+++ proposed <path>`;
//!   `notes` the reasons a preview is unavailable and a refusal's reason.
//!   The intake kinds (canon `agent-intake`, "Review document") fill the
//!   eleven keys after `updated_at` instead of an update's texts, `diff`,
//!   `preview`; their decision record (canon `decision-record`, "Queue and
//!   documents") the five after `linked`: `record_id`, `record_path`, `record_title`,
//!   `record_text`, `choice` (an object); then `task_id`, the task it was
//!   raised for (canon `tasks`, "Task-bound proposals"). A create (task spec
//!   `proposal-kinds`) is an update's document, `target_ids` as stored; a
//!   new file's base `null`, its diff from an empty base.
//! - **Brief** (`--brief`, MCP `get_proposal`, `propose_change`): the texts
//!   (`record_text` too), `diff` and `conflict` dropped, at most [`SHOW_TAIL_NAMES`] findings
//!   (the rest counted in a note), the text cut at [`OUTPUT_CAP_CHARS`].
//!
//! - **Terminal**: the text output and stderr of the queue's commands
//!   carry agent-written text (rationale, texts, paths): every C0 and C1
//!   control character but LF and TAB is printed escaped
//!   ([`crate::escape_controls`]); JSON escapes them itself.
//!
//! Exit codes: 0 done; 1 refused by the proposal or its target (the
//! document's `notes` and stderr say why); 2 cannot run here.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::{
    Author, ProposalIdError, is_utc_timestamp, look_alike_message, parse_proposal_id, prefix_clash,
};
use specengine_store::{
    Choice, GitEnv, Proposal, ProposalFinding, ProposalQueue as _, QueueError, SqliteQueue,
    WorktreeGit, same_repository,
};

use crate::cap::{OUTPUT_CAP_CHARS, SHOW_TAIL_NAMES};
use crate::location::prepared_data_dir;
use crate::project::{CONFIG_FILE, ProjectRoot, discover};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// Which command answered with a review document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueCommand {
    Propose,
    Review,
    Approve,
    Reject,
}

/// What applying an open or approved proposal now would do: apply steps
/// 2–6, read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Preview {
    /// The span is as proposed against: the new text replaces it.
    Applies,
    /// The span changed since; the three-way merge is clean.
    Rebases,
    /// The span changed since and the edits overlap (`conflict`).
    Conflicts,
    /// A step refused; `notes` says which and why.
    Unavailable,
}

impl Preview {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applies => "applies",
            Self::Rebases => "rebases",
            Self::Conflicts => "conflicts",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The review document: every key, in this order, absent = `null`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProposalDocument {
    pub id: Option<String>,
    pub project: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub target_id: Option<String>,
    /// Root-relative.
    pub target_path: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub base_hash: Option<String>,
    pub base_text: Option<String>,
    pub new_text: Option<String>,
    pub patch_hash: Option<String>,
    pub rationale: Option<String>,
    /// `{type, role, model, run}`.
    pub author: Option<Author>,
    /// The findings the edit introduced at creation.
    pub diagnostics: Option<Vec<ProposalFinding>>,
    /// Base → new hunks, with the two header lines; `""` when equal.
    pub diff: Option<String>,
    /// Open and approved proposals only.
    pub preview: Option<Preview>,
    /// `git merge-file`'s text when the preview conflicts.
    pub conflict: Option<String>,
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    pub decision_note: Option<String>,
    pub applied_commit: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// The canonical IDs: an update's `[target_id]`; a create's
    /// `target_id` then its other new IDs.
    pub target_ids: Vec<String>,
    pub severity: Option<IntakeSeverity>,
    pub gap_type: Option<GapType>,
    /// A question's text, a discrepancy's summary.
    pub summary: Option<String>,
    pub working_answer: Option<String>,
    pub price_of_other: Option<String>,
    pub evidence: Vec<Evidence>,
    pub options: Vec<IntakeOption>,
    /// An index into `options`.
    pub recommendation: Option<u64>,
    pub distinct_from: Vec<String>,
    /// The other proposal of a discrepancy and its proposed patch.
    pub linked: Option<String>,
    /// A question's or a discrepancy's decision record, from its step 7.
    pub record_id: Option<String>,
    /// Root-relative.
    pub record_path: Option<String>,
    pub record_title: Option<String>,
    /// The record's bytes; `null` in a brief answer.
    pub record_text: Option<String>,
    /// `{"option":N}`, `{"working_answer":true}` or `{"answer":"…"}`.
    pub choice: Option<Choice>,
    /// The task it was raised for; `null` unbound.
    pub task_id: Option<String>,
    /// Why the preview is unavailable, what a reader should know, and a
    /// refusal's reason last; each one line.
    pub notes: Vec<String>,
}

impl ProposalDocument {
    /// The stored proposal's keys; `diff`, `preview`, `conflict` and
    /// `notes` left for the caller. An update's texts, hashes and rationale;
    /// an intake kind's fields instead (those `null`).
    pub fn of(proposal: &Proposal) -> Self {
        let applies = proposal.kind.applies();
        let text = |value: &str| applies.then(|| value.to_owned());
        // A new file has no base.
        let based = applies && !proposal.new_file();
        let base = |value: &str| based.then(|| value.to_owned());
        let intake = proposal.intake.as_ref();
        Self {
            id: Some(proposal.id.clone()),
            project: Some(proposal.project.clone()),
            kind: Some(proposal.kind.as_str().to_owned()),
            status: Some(proposal.status.as_str().to_owned()),
            target_id: Some(proposal.target_id.clone()),
            target_path: Some(proposal.target_path.clone()),
            worktree: Some(proposal.place.worktree.clone()),
            branch: Some(proposal.place.branch.clone()),
            base_commit: Some(proposal.place.base_commit.clone()),
            base_hash: base(&proposal.base_hash),
            base_text: base(&proposal.base_text),
            new_text: text(&proposal.new_text),
            patch_hash: text(&proposal.patch_hash),
            rationale: text(&proposal.rationale),
            author: Some(proposal.author.clone()),
            diagnostics: Some(proposal.diagnostics.clone()),
            diff: None,
            preview: None,
            conflict: None,
            decided_by: proposal.decided_by.clone(),
            decided_at: proposal.decided_at.clone(),
            decision_note: proposal.decision_note.clone(),
            applied_commit: proposal.applied_commit.clone(),
            created_at: Some(proposal.created_at.clone()),
            updated_at: Some(proposal.updated_at.clone()),
            target_ids: proposal.target_ids(),
            severity: intake.map(|intake| intake.severity),
            gap_type: intake.and_then(|intake| intake.gap_type),
            summary: intake.map(|intake| intake.summary.clone()),
            working_answer: intake.and_then(|intake| intake.working_answer.clone()),
            price_of_other: intake.and_then(|intake| intake.price_of_other.clone()),
            evidence: intake
                .map(|intake| intake.evidence.clone())
                .unwrap_or_default(),
            options: intake
                .map(|intake| intake.options.clone())
                .unwrap_or_default(),
            recommendation: intake.and_then(|intake| intake.recommendation),
            distinct_from: intake
                .map(|intake| intake.distinct_from.clone())
                .unwrap_or_default(),
            linked: proposal.linked.clone(),
            record_id: proposal.record.as_ref().map(|record| record.id.clone()),
            record_path: proposal.record.as_ref().map(|record| record.path.clone()),
            record_title: proposal.record.as_ref().map(|record| record.title.clone()),
            record_text: proposal.record.as_ref().map(|record| record.text.clone()),
            choice: proposal.record.as_ref().map(|record| record.choice.clone()),
            task_id: proposal.task_id.clone(),
            notes: Vec::new(),
        }
    }
}

/// What `spec propose`, `spec review`, `spec approve` or `spec reject`
/// answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalOutcome {
    pub command: QueueCommand,
    pub document: ProposalDocument,
    /// Why the command was refused (exit 1); also the document's last note.
    pub refusal: Option<String>,
    pub messages: Vec<Message>,
    /// `--brief` ([`briefed`]): how many introduced findings past
    /// [`SHOW_TAIL_NAMES`] were dropped; `None` without it.
    pub brief: Option<usize>,
}

impl ProposalOutcome {
    /// Exit 0, or 1 when refused.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    /// A refused command: `reason` (one line) is the refusal and the
    /// document's last note.
    pub(crate) fn refused(
        command: QueueCommand,
        mut document: ProposalDocument,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        let reason = one_line(reason);
        document.notes.push(reason.clone());
        Self {
            command,
            document,
            refusal: Some(reason),
            messages,
            brief: None,
        }
    }

    /// A command that did what it was asked.
    pub(crate) fn done(
        command: QueueCommand,
        document: ProposalDocument,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            command,
            document,
            refusal: None,
            messages,
            brief: None,
        }
    }
}

/// `--brief`: `base_text`, `new_text`, `diff` and `conflict` dropped; at
/// most [`SHOW_TAIL_NAMES`] introduced findings, the rest counted in a note
/// (`<k> more introduced finding(s): spec review PR`, before a refusal's
/// reason, also on stderr); the text cut at [`OUTPUT_CAP_CHARS`]
/// ([`render_text`]).
pub(crate) fn briefed(mut outcome: ProposalOutcome) -> ProposalOutcome {
    let document = &mut outcome.document;
    document.base_text = None;
    document.new_text = None;
    document.diff = None;
    document.conflict = None;
    document.record_text = None;
    let mut omitted = 0;
    if let Some(findings) = &mut document.diagnostics
        && findings.len() > SHOW_TAIL_NAMES
    {
        omitted = findings.len() - SHOW_TAIL_NAMES;
        findings.truncate(SHOW_TAIL_NAMES);
        let note = more_findings_note(omitted, document.id.as_deref().unwrap_or("PR"));
        let at = document.notes.len() - usize::from(outcome.refusal.is_some());
        document.notes.insert(at, note.clone());
        outcome.messages.push(Message::Note(note));
    }
    outcome.brief = Some(omitted);
    outcome
}

/// The note on the introduced findings a brief answer leaves out.
pub(crate) fn more_findings_note(omitted: usize, id: &str) -> String {
    format!("{omitted} more introduced finding(s): spec review {id}")
}

impl Serialize for ProposalOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.document.serialize(serializer)
    }
}

/// Where the queue's commands run: the project, its database and the
/// current repository.
pub(crate) struct QueueContext {
    pub project: ProjectRoot,
    pub slug: String,
    /// The data directory: the database's, and the scratch files' of git.
    pub data_dir: PathBuf,
    pub queue: SqliteQueue,
    /// Git at the project root, the caller's local variables dropped.
    pub git: WorktreeGit,
    /// The current repository's canonical git common dir.
    pub common_dir: PathBuf,
}

/// Finds the project and opens its queue (see the module documentation):
/// exit 2 for a `--config` other than the root's `specengine.toml`, an
/// `[ids]` taking `PR`, a root in no git worktree, and what the read
/// commands refuse (no project, no slug, `HOME`, the database).
pub(crate) fn open_context(
    env: &Env,
    globals: &Globals,
    git_env: &GitEnv,
) -> Result<QueueContext, CliError> {
    let project = discover(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    if let Some(clash) = prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    let git = WorktreeGit::new(&project.root, git_env)
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    let common_dir = git.common_dir().map_err(|error| {
        CliError::spec(format!(
            "the project root {} lies in no git worktree ({error}): proposals are bound to a \
             git worktree and applied as commits",
            project.root.display()
        ))
    })?;
    let data_dir = prepared_data_dir(env, &project)?;
    let queue =
        SqliteQueue::open(data_dir.join(format!("{slug}.db")), &slug).map_err(queue_cannot)?;
    Ok(QueueContext {
        project,
        slug,
        data_dir,
        queue,
        git,
        common_dir,
    })
}

/// `--config`, when given, must name the project root's own
/// `specengine.toml`: the queue is the root slug's (exit 2 otherwise).
pub(crate) fn require_root_config(
    env: &Env,
    globals: &Globals,
    project: &ProjectRoot,
) -> Result<(), CliError> {
    let Some(config) = &globals.config else {
        return Ok(());
    };
    let given = fs::canonicalize(env.cwd.join(config)).ok();
    let own = fs::canonicalize(project.root.join(CONFIG_FILE)).ok();
    if given.is_none() || given != own {
        return Err(CliError::spec(format!(
            "--config {}: the proposal queue reads only the project root's {CONFIG_FILE} \
             (the config proposals are checked and applied under); drop --config",
            config.display()
        )));
    }
    Ok(())
}

/// A queue command's error with its control characters escaped: it may
/// quote stored, agent-written text.
pub(crate) fn escaped_error(error: CliError) -> CliError {
    CliError {
        exit: error.exit,
        message: escape_controls(&error.message),
    }
}

/// A queue error that is no refusal: exit 2.
pub(crate) fn queue_cannot(error: QueueError) -> CliError {
    CliError::spec(error)
}

/// A queue error: a refusal (no such proposal, a state that does not allow
/// the change) as `Ok(reason)`, anything else exit 2.
pub(crate) fn queue_refusal(error: QueueError) -> Result<String, CliError> {
    match error {
        QueueError::Unknown { .. } | QueueError::Status { .. } | QueueError::Changed { .. } => {
            Ok(error.to_string())
        }
        other => Err(queue_cannot(other)),
    }
}

/// The proposal ID as written: `Some` when it is one, `None` when it is
/// none in any script (exit 1 once the queue is open: every queue command
/// first refuses a clashing `[ids]`); look-alikes exit 2 naming the fix.
pub(crate) fn written_id(written: &str) -> Result<Option<String>, CliError> {
    match parse_proposal_id(written) {
        Ok(id) => Ok(Some(id)),
        Err(ProposalIdError::LookAlike { fix }) => {
            Err(CliError::spec(look_alike_message(written.trim(), &fix)))
        }
        Err(ProposalIdError::NotAnId) => Ok(None),
    }
}

/// Exit 1: `written` names no proposal.
pub(crate) fn no_proposal(written: &str) -> String {
    format!(
        "no proposal `{}`: a proposal ID is `PR-` and 4 or more digits, as `spec inbox` lists it",
        written.trim()
    )
}

/// Which proposals [`find`] gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Find {
    /// The current repository's only.
    SameRepository,
    /// Also one whose recorded common dir no longer exists (`spec
    /// reject`: the orphan rule).
    OrGoneRepository,
}

/// The proposal's recorded common dir no longer exists (the orphan rule).
pub(crate) fn repository_gone(proposal: &Proposal) -> bool {
    fs::symlink_metadata(&proposal.place.git_common_dir).is_err()
}

/// The proposal `id` of the current repository: `Ok(Err(reason))` when
/// the queue has none (exit 1); another repository's (the same slug) exit
/// 2, naming it, unless `which` takes a gone repository's and the recorded
/// common dir no longer exists.
pub(crate) fn find(
    context: &QueueContext,
    id: &str,
    which: Find,
) -> Result<Result<Proposal, String>, CliError> {
    let proposal = match context.queue.get(id) {
        Ok(Some(proposal)) => proposal,
        Ok(None) => {
            return Ok(Err(format!("no proposal `{id}` in this project's queue")));
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    if !same_repository(&proposal.place.git_common_dir, &context.common_dir) {
        let gone = repository_gone(&proposal);
        if gone && which == Find::OrGoneRepository {
            return Ok(Ok(proposal));
        }
        if gone {
            return Err(CliError::spec(format!(
                "`{id}` belongs to the repository {} (worktree {}), which no longer exists: \
                 `spec reject {id} --reason …` takes it out of the inbox unless its commit is in \
                 history",
                proposal.place.git_common_dir, proposal.place.worktree
            )));
        }
        return Err(CliError::spec(format!(
            "`{id}` belongs to another repository of the project `{}`: {} (worktree {}); this \
             one is {}: run the command there",
            context.slug,
            proposal.place.git_common_dir,
            proposal.place.worktree,
            context.common_dir.display()
        )));
    }
    Ok(Ok(proposal))
}

/// The injected clock's time stamp must be `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn checked_now(now: &str) -> Result<&str, CliError> {
    if is_utc_timestamp(now) {
        Ok(now)
    } else {
        Err(CliError::spec(format!(
            "the clock gave `{now}`, not a UTC time stamp YYYY-MM-DDTHH:MM:SSZ"
        )))
    }
}

/// The target's path from the worktree's top: `root_rel/target_path`.
pub(crate) fn top_path(proposal: &Proposal) -> String {
    top_of(proposal, &proposal.target_path)
}

/// A root-relative `path` of the proposal's recorded root from the
/// worktree's top: `root_rel/path`.
pub(crate) fn top_of(proposal: &Proposal, path: &str) -> String {
    if proposal.place.root_rel.is_empty() {
        path.to_owned()
    } else {
        format!("{}/{path}", proposal.place.root_rel)
    }
}

/// The document's `diff`: the span's base → new hunks (`git diff
/// --no-index`, scratch files in `scratch`), headed `--- base <path>` and
/// `+++ proposed <path>`; `""` when the texts are equal. A git failure is a
/// note and no diff.
pub(crate) fn fill_diff(
    document: &mut ProposalDocument,
    proposal: &Proposal,
    git_env: &GitEnv,
    scratch: &Path,
) {
    // The intake kinds have no texts: no diff.
    if !proposal.kind.applies() {
        return;
    }
    let hunks = WorktreeGit::new(scratch, git_env).and_then(|git| {
        git.diff_hunks(
            scratch,
            proposal.base_text.as_bytes(),
            proposal.new_text.as_bytes(),
        )
    });
    match hunks {
        Ok(hunks) if hunks.is_empty() => document.diff = Some(String::new()),
        Ok(hunks) => {
            document.diff = Some(format!(
                "--- base {path}\n+++ proposed {path}\n{}",
                String::from_utf8_lossy(&hunks),
                path = proposal.target_path
            ));
        }
        Err(error) => document.notes.push(one_line(&format!("no diff: {error}"))),
    }
}

/// [`ProposalDocument::of`] with its `diff` ([`fill_diff`]).
pub(crate) fn with_diff(proposal: &Proposal, git_env: &GitEnv, scratch: &Path) -> ProposalDocument {
    let mut document = ProposalDocument::of(proposal);
    fill_diff(&mut document, proposal, git_env, scratch);
    document
}

/// One introduced finding as a line: `<severity>  <path>:<line>: <code>:
/// <message>` (the check's report line).
pub(crate) fn finding_line(finding: &ProposalFinding) -> String {
    let severity = match finding.severity {
        specengine_model::Severity::Error => "error",
        specengine_model::Severity::Warning => "warning",
    };
    one_line(&format!(
        "{severity}  {}:{}: {}: {}",
        finding.path, finding.line, finding.code, finding.message
    ))
}

/// The outcome as stdout text: `spec review`'s `key: value` lines; `spec
/// propose`'s ID, `introduced: <n>` and the finding lines; `spec
/// approve`'s and `spec reject`'s one line. A refused command prints only
/// the conflict, when there is one. Control characters escaped; a brief
/// answer cut at [`OUTPUT_CAP_CHARS`] ([`cut_brief`]).
pub(crate) fn render_text(outcome: &ProposalOutcome) -> String {
    let text = escape_controls(&raw_text(outcome));
    match outcome.brief {
        Some(_) => cut_brief(text, outcome.document.id.as_deref().unwrap_or("PR")),
        None => text,
    }
}

/// A brief text over [`OUTPUT_CAP_CHARS`] characters: the lines that fit
/// (a first line longer than the cap cut at it), then `[truncated: <k> of
/// <n> lines not shown: spec review PR]`.
pub(crate) fn cut_brief(text: String, id: &str) -> String {
    if text.chars().count() <= OUTPUT_CAP_CHARS {
        return text;
    }
    let total = text.lines().count();
    let mut kept = String::new();
    let mut chars = 0;
    let mut shown = 0;
    for line in text.split_inclusive('\n') {
        let length = line.chars().count();
        if chars + length > OUTPUT_CAP_CHARS {
            if shown == 0 {
                kept.extend(line.chars().take(OUTPUT_CAP_CHARS - 1));
                kept.push('\n');
                shown = 1;
            }
            break;
        }
        kept.push_str(line);
        chars += length;
        shown += 1;
    }
    kept.push_str(&format!(
        "[truncated: {} of {total} lines not shown: spec review {id}]\n",
        total - shown
    ));
    kept
}

fn raw_text(outcome: &ProposalOutcome) -> String {
    let document = &outcome.document;
    let mut out = String::new();
    if outcome.refusal.is_some() {
        if let Some(conflict) = &document.conflict {
            out.push_str(conflict);
            if !conflict.is_empty() && !conflict.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    let id = document.id.as_deref().unwrap_or_default();
    match outcome.command {
        QueueCommand::Review => return review_text(document),
        QueueCommand::Propose => {
            let findings = document.diagnostics.as_deref().unwrap_or_default();
            let introduced = findings.len() + outcome.brief.unwrap_or(0);
            out.push_str(&format!("{id}\nintroduced: {introduced}\n"));
            for finding in findings {
                out.push_str(&finding_line(finding));
                out.push('\n');
            }
        }
        QueueCommand::Approve => {
            out.push_str(&format!(
                "applied {id} as {} on {}",
                document.applied_commit.as_deref().unwrap_or_default(),
                document.branch.as_deref().unwrap_or_default()
            ));
            if let (Some(record), Some(path)) = (&document.record_id, &document.record_path) {
                out.push_str(&format!(": {record} {path}"));
            }
            out.push('\n');
        }
        QueueCommand::Reject => out.push_str(&format!("rejected {id}\n")),
    }
    out
}

/// `spec review`'s text: every key of the document in its order, `key:
/// value`; texts as indented blocks (`key:` then each line indented by
/// two spaces); lists as `key: <count>` and an indented line each; an
/// absent value `key: -`.
fn review_text(document: &ProposalDocument) -> String {
    let mut out = String::new();
    let line = |out: &mut String, key: &str, value: Option<&str>| {
        out.push_str(&format!(
            "{key}: {}\n",
            value.map_or_else(|| "-".to_owned(), one_line)
        ));
    };
    let block = |out: &mut String, key: &str, value: Option<&str>| match value {
        None => out.push_str(&format!("{key}: -\n")),
        Some(text) => {
            out.push_str(&format!("{key}:\n"));
            for text_line in text.split_terminator('\n') {
                if text_line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("  {text_line}\n"));
                }
            }
        }
    };
    let list = |out: &mut String, key: &str, lines: Option<Vec<String>>| match lines {
        None => out.push_str(&format!("{key}: -\n")),
        Some(lines) => {
            out.push_str(&format!("{key}: {}\n", lines.len()));
            for text_line in lines {
                out.push_str(&format!("  {text_line}\n"));
            }
        }
    };
    line(&mut out, "id", document.id.as_deref());
    line(&mut out, "project", document.project.as_deref());
    line(&mut out, "kind", document.kind.as_deref());
    line(&mut out, "status", document.status.as_deref());
    line(&mut out, "target_id", document.target_id.as_deref());
    line(&mut out, "target_path", document.target_path.as_deref());
    line(&mut out, "worktree", document.worktree.as_deref());
    line(&mut out, "branch", document.branch.as_deref());
    line(&mut out, "base_commit", document.base_commit.as_deref());
    line(&mut out, "base_hash", document.base_hash.as_deref());
    block(&mut out, "base_text", document.base_text.as_deref());
    block(&mut out, "new_text", document.new_text.as_deref());
    line(&mut out, "patch_hash", document.patch_hash.as_deref());
    block(&mut out, "rationale", document.rationale.as_deref());
    let author = document.author.as_ref().map(Author::provenance);
    line(&mut out, "author", author.as_deref());
    list(
        &mut out,
        "diagnostics",
        document
            .diagnostics
            .as_ref()
            .map(|findings| findings.iter().map(finding_line).collect()),
    );
    block(&mut out, "diff", document.diff.as_deref());
    line(&mut out, "preview", document.preview.map(Preview::as_str));
    block(&mut out, "conflict", document.conflict.as_deref());
    line(&mut out, "decided_by", document.decided_by.as_deref());
    line(&mut out, "decided_at", document.decided_at.as_deref());
    block(&mut out, "decision_note", document.decision_note.as_deref());
    line(
        &mut out,
        "applied_commit",
        document.applied_commit.as_deref(),
    );
    line(&mut out, "created_at", document.created_at.as_deref());
    line(&mut out, "updated_at", document.updated_at.as_deref());
    list(&mut out, "target_ids", Some(document.target_ids.clone()));
    line(
        &mut out,
        "severity",
        document.severity.map(IntakeSeverity::as_str),
    );
    line(&mut out, "gap_type", document.gap_type.map(GapType::as_str));
    line(&mut out, "summary", document.summary.as_deref());
    line(
        &mut out,
        "working_answer",
        document.working_answer.as_deref(),
    );
    line(
        &mut out,
        "price_of_other",
        document.price_of_other.as_deref(),
    );
    list(
        &mut out,
        "evidence",
        Some(document.evidence.iter().map(evidence_line).collect()),
    );
    list(
        &mut out,
        "options",
        Some(
            document
                .options
                .iter()
                .enumerate()
                .map(|(index, option)| option_line(index, option, document.recommendation))
                .collect(),
        ),
    );
    let recommendation = document.recommendation.map(|index| index.to_string());
    line(&mut out, "recommendation", recommendation.as_deref());
    list(
        &mut out,
        "distinct_from",
        Some(document.distinct_from.clone()),
    );
    line(&mut out, "linked", document.linked.as_deref());
    line(&mut out, "record_id", document.record_id.as_deref());
    line(&mut out, "record_path", document.record_path.as_deref());
    line(&mut out, "record_title", document.record_title.as_deref());
    block(&mut out, "record_text", document.record_text.as_deref());
    let choice = document
        .choice
        .as_ref()
        .and_then(|choice| serde_json::to_string(choice).ok());
    line(&mut out, "choice", choice.as_deref());
    line(&mut out, "task_id", document.task_id.as_deref());
    list(&mut out, "notes", Some(document.notes.clone()));
    out
}

/// One evidence item: `<file>[:<lines>][ <qpath>] | <observed> |
/// <documented>`, on one line.
pub(crate) fn evidence_line(item: &Evidence) -> String {
    let mut place = item.file.clone();
    if let Some(lines) = &item.lines {
        place.push(':');
        place.push_str(lines);
    }
    if let Some(qpath) = &item.qpath {
        place.push(' ');
        place.push_str(qpath);
    }
    one_line(&format!(
        "{place} | {} | {}",
        item.observed, item.documented
    ))
}

/// One option: `[<i>] <label> | <effect> | <price>`, ` (recommended)` on
/// the recommended one, on one line.
pub(crate) fn option_line(index: usize, option: &IntakeOption, recommended: Option<u64>) -> String {
    let mark = if recommended.and_then(|at| usize::try_from(at).ok()) == Some(index) {
        " (recommended)"
    } else {
        ""
    };
    one_line(&format!(
        "[{index}] {} | {} | {}{mark}",
        option.label, option.effect, option.price
    ))
}
