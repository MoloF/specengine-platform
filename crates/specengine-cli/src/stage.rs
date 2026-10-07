//! The owner's staged choice (canon `decision-staging`; task spec
//! `decision-staging`): one replaceable choice per `open` proposal, the
//! flags `spec approve|reject` would take, kept only in the queue.
//!
//! - **Staging** ([`stage`], [`unstage`]; no `spec` command: the daemon's
//!   decision route calls them). A stage is checked as the terminal checks
//!   its flags, before anything is stored: the ID and the repository (an
//!   orphan takes only a reject), the state `open`, the `updated_at` the
//!   caller read (else refused with the current document), the decision
//!   flags ([`crate::decide`]'s first checks; a decision flag on an
//!   `update` or a `create` refused), the caps (`note`, `reason` 4 096
//!   bytes; `answer` 2 048, `canon` 512) and a character the terminal
//!   shows escaped, named. An `update`'s or a section-form `create`'s
//!   stage records the target's span hash as step 5 reads it (steps 2–6
//!   read-only, as `spec review`: nothing written in any worktree). The
//!   answer is the review document, or why not ([`StageCause`]).
//! - **Confirming** ([`Staging`]): `spec approve PR` without a flag takes a
//!   staged approve, `spec reject PR` without `--reason` a staged reject;
//!   the stage is shown before the question (`staged <at>: spec approve
//!   PR --option 1 --note "…"`, the chosen option's line), the question is
//!   marked `staged`, and step 7, a completion and a reject compare it with
//!   the row byte for byte: replaced or removed → exit 1, nothing changed.
//!   Typed flags win whole: the stage is left unused, named in a note.

use serde::{Deserialize, Serialize};
use specengine_core::record::refused_char;
use specengine_store::{
    GitEnv, Proposal, ProposalQueue as _, ProposalStatus, QueueError, Stage, StagedChoice,
    same_repository,
};

use crate::apply::{ApproveFlags, ApproveRequest, flag_on_apply};
use crate::decide::first_checks;
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, checked_now, elsewhere,
    escaped_error, no_proposal, open_context, option_line, queue_cannot, repository_gone,
    with_diff, written_id,
};
use crate::review::{document_with, preview_run, previewed};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls};

/// The most bytes of an approval's note and a rejection's reason, typed or
/// staged (canon `decision-staging`, "The stage").
pub(crate) const NOTE_MAX_BYTES: usize = 4096;

/// `flag`'s `text` over [`NOTE_MAX_BYTES`]: the refusal (exit 1).
pub(crate) fn over_cap(flag: &str, text: &str) -> Option<String> {
    (text.len() > NOTE_MAX_BYTES).then(|| {
        format!(
            "{flag}: {} bytes; at most {NOTE_MAX_BYTES}; nothing changed",
            text.len()
        )
    })
}

/// A stage as the daemon's decision route receives it: the stage's fields
/// without `span_hash`, and the `updated_at` of the document the caller
/// read. `{"decision":"approve","option":1,"answer":null,"canon":null,
/// "note":"…","updated_at":"…"}` (an approve's flags absent = `null`) or
/// `{"decision":"reject","reason":"…","updated_at":"…"}`; any other key, or
/// one missing, is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase", deny_unknown_fields)]
pub enum StageBody {
    /// `spec approve PR` with these flags.
    Approve {
        #[serde(default)]
        option: Option<u64>,
        #[serde(default)]
        answer: Option<String>,
        #[serde(default)]
        canon: Option<String>,
        #[serde(default)]
        note: Option<String>,
        updated_at: String,
    },
    /// `spec reject PR --reason T`.
    Reject { reason: String, updated_at: String },
}

impl StageBody {
    /// The body `bytes` decoded (JSON of one of the two shapes), or exit 2
    /// naming the defect.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        serde_json::from_slice(bytes).map_err(|error| {
            CliError::spec(format!(
                "the stage is not `{{\"decision\":\"approve\",\"option\",\"answer\",\"canon\",\
                 \"note\",\"updated_at\"}}` or `{{\"decision\":\"reject\",\"reason\",\
                 \"updated_at\"}}` as JSON: {error}; nothing changed"
            ))
        })
    }

    /// The `updated_at` the caller read.
    pub fn updated_at(&self) -> &str {
        match self {
            Self::Approve { updated_at, .. } | Self::Reject { updated_at, .. } => updated_at,
        }
    }

    /// `true` for a reject.
    pub fn rejects(&self) -> bool {
        matches!(self, Self::Reject { .. })
    }
}

/// [`stage`]'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageRequest {
    /// `PR` as given.
    pub id: String,
    pub body: StageBody,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`, the stage's `staged_at`.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// [`unstage`]'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstageRequest {
    /// `PR` as given.
    pub id: String,
    pub now: String,
    pub git: GitEnv,
}

/// Why [`stage`] or [`unstage`] stored nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageCause {
    /// The request's own defect, as the terminal's exit 2 usage line: a
    /// flag the kind does not take, a flag missing or malformed, a blank
    /// reason, a look-alike ID.
    Usage,
    /// No such proposal in the project's queue.
    Unknown,
    /// The proposal refuses it (the refused document's last note): not
    /// `open`, another repository, an orphan's approve, `updated_at`
    /// changed, a lost compare-and-set, a cap, a character.
    Refused,
}

/// What [`stage`] or [`unstage`] answered: the review document (refused:
/// its last note the reason), and why nothing was stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageOutcome {
    pub proposal: ProposalOutcome,
    /// `None`: stored (or, unstaging, nothing was staged).
    pub cause: Option<StageCause>,
}

impl StageOutcome {
    /// 0 stored; 2 a usage defect; 1 unknown or refused.
    pub fn exit(&self) -> Exit {
        match self.cause {
            None => Exit::Answered,
            Some(StageCause::Usage) => Exit::CannotRun,
            Some(StageCause::Unknown | StageCause::Refused) => Exit::NotFound,
        }
    }

    fn done(document: ProposalDocument, messages: Vec<Message>) -> Self {
        Self {
            proposal: ProposalOutcome::done(QueueCommand::Review, document, messages),
            cause: None,
        }
    }

    fn refused(
        cause: StageCause,
        document: ProposalDocument,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            proposal: ProposalOutcome::refused(QueueCommand::Review, document, reason, messages),
            cause: Some(cause),
        }
    }

    /// A usage defect: `error`'s line, its `spec: ` left to the stderr
    /// line.
    fn usage(error: &CliError) -> Self {
        let reason = error
            .message
            .strip_prefix("spec: ")
            .unwrap_or(&error.message);
        Self::refused(
            StageCause::Usage,
            ProposalDocument::default(),
            reason,
            Vec::new(),
        )
    }
}

impl Serialize for StageOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.proposal.serialize(serializer)
    }
}

/// Stages the owner's choice on an `open` proposal of the current
/// repository (see the module documentation), replacing any earlier one:
/// one `proposal.staged`; nothing else is written but the data directory.
/// Exit 2 ([`CliError`]) only when it cannot run here.
pub fn stage(
    env: &Env,
    globals: &Globals,
    request: &StageRequest,
) -> Result<StageOutcome, CliError> {
    run_stage(env, globals, request).map_err(escaped_error)
}

/// Clears an `open` proposal's staged choice: one `proposal.unstaged`;
/// nothing staged: the document, no event. Exit 2 ([`CliError`]) only when
/// it cannot run here.
pub fn unstage(
    env: &Env,
    globals: &Globals,
    request: &UnstageRequest,
) -> Result<StageOutcome, CliError> {
    run_unstage(env, globals, request).map_err(escaped_error)
}

fn run_stage(
    env: &Env,
    globals: &Globals,
    request: &StageRequest,
) -> Result<StageOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let written = match written_id(&request.id) {
        Ok(written) => written,
        Err(error) => return Ok(StageOutcome::usage(&error)),
    };
    let mut context = open_context(env, globals, &request.git)?;
    let body = &request.body;
    let proposal = match open_proposal(&context, written, request, body.rejects())? {
        Ok(proposal) => proposal,
        Err(outcome) => return Ok(outcome),
    };
    let id = proposal.id.clone();
    let current = || with_diff(&proposal, &request.git, &context.data_dir);
    if body.updated_at() != proposal.updated_at {
        let reason = format!(
            "`{id}` changed since it was read: updated at {}, the choice was made on the one of \
             {}; read it again; nothing staged",
            proposal.updated_at,
            body.updated_at()
        );
        return Ok(StageOutcome::refused(
            StageCause::Refused,
            current(),
            &reason,
            Vec::new(),
        ));
    }
    let checked = checked(&context, &proposal, body);
    let mut stage = match checked {
        Ok(stage) => stage,
        Err((cause, reason)) => {
            return Ok(StageOutcome::refused(cause, current(), &reason, Vec::new()));
        }
    };
    // Steps 2–6 read-only, as `spec review`: the preview, and the span hash
    // an `update`'s or a section-form `create`'s stage records.
    let mut messages = Vec::new();
    let run = preview_run(env, &request.git, &context, &proposal, &mut messages);
    if let Stage::Approve { span_hash, .. } = &mut stage
        && proposal.kind.applies()
        && !proposal.new_file()
    {
        span_hash.clone_from(&run.span);
    }
    match context.queue.stage_from(&id, &proposal.seen(), &stage, now) {
        Ok(stored) => {
            let document = document_with(&request.git, &context, &stored, run);
            Ok(StageOutcome::done(document, messages))
        }
        Err(error) => refused_by_queue(&context, request, &proposal, error, messages),
    }
}

fn run_unstage(
    env: &Env,
    globals: &Globals,
    request: &UnstageRequest,
) -> Result<StageOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let written = match written_id(&request.id) {
        Ok(written) => written,
        Err(error) => return Ok(StageOutcome::usage(&error)),
    };
    let mut context = open_context(env, globals, &request.git)?;
    let proposal = match open_proposal(&context, written, request, true)? {
        Ok(proposal) => proposal,
        Err(outcome) => return Ok(outcome),
    };
    let id = proposal.id.clone();
    match context.queue.unstage_from(&id, &proposal.seen(), now) {
        Ok(_) => {
            let stored = match context.queue.get(&id) {
                Ok(Some(stored)) => stored,
                Ok(None) => proposal,
                Err(error) => return Err(queue_cannot(error)),
            };
            let mut messages = Vec::new();
            let document = previewed(env, &request.git, &context, &stored, &mut messages);
            Ok(StageOutcome::done(document, messages))
        }
        Err(error) => refused_by_queue(&context, request, &proposal, error, Vec::new()),
    }
}

/// What a stage request names: its ID as written and its git environment.
trait Named {
    fn id(&self) -> &str;
    fn git(&self) -> &GitEnv;
}

impl Named for StageRequest {
    fn id(&self) -> &str {
        &self.id
    }

    fn git(&self) -> &GitEnv {
        &self.git
    }
}

impl Named for UnstageRequest {
    fn id(&self) -> &str {
        &self.id
    }

    fn git(&self) -> &GitEnv {
        &self.git
    }
}

/// The `open` proposal `written` names, of the current repository (one of
/// a repository that no longer exists only when `orphan` takes it: a
/// reject, an unstage); else the outcome refusing the request.
fn open_proposal(
    context: &QueueContext,
    written: Option<String>,
    request: &impl Named,
    orphan: bool,
) -> Result<Result<Proposal, StageOutcome>, CliError> {
    let unknown = |reason: &str| {
        Ok(Err(StageOutcome::refused(
            StageCause::Unknown,
            ProposalDocument::default(),
            reason,
            Vec::new(),
        )))
    };
    let Some(id) = written else {
        return unknown(&no_proposal(request.id()));
    };
    let proposal = match context.queue.get(&id) {
        Ok(Some(proposal)) => proposal,
        Ok(None) => return unknown(&format!("no proposal `{id}` in this project's queue")),
        Err(error) => return Err(queue_cannot(error)),
    };
    let refused = |reason: &str| {
        let document = with_diff(&proposal, request.git(), &context.data_dir);
        Ok(Err(StageOutcome::refused(
            StageCause::Refused,
            document,
            reason,
            Vec::new(),
        )))
    };
    if !same_repository(&proposal.place.git_common_dir, &context.common_dir) {
        let gone = repository_gone(&proposal);
        if !(gone && orphan) {
            return refused(&elsewhere(context, &proposal, gone));
        }
    }
    if proposal.status != ProposalStatus::Open {
        let mut reason = format!(
            "`{id}` is {}: only an open proposal takes a staged choice",
            proposal.status
        );
        if let Some(commit) = &proposal.applied_commit {
            reason.push_str(&format!(" (commit {commit})"));
        }
        reason.push_str("; nothing changed");
        return refused(&reason);
    }
    Ok(Ok(proposal))
}

/// A refusal of the queue's: a lost compare-and-set or a state that no
/// longer takes it (the current document), an ID gone; else exit 2.
fn refused_by_queue(
    context: &QueueContext,
    request: &impl Named,
    proposal: &Proposal,
    error: QueueError,
    messages: Vec<Message>,
) -> Result<StageOutcome, CliError> {
    let cause = match &error {
        QueueError::Changed { .. } | QueueError::Status { .. } | QueueError::Invalid(_) => {
            StageCause::Refused
        }
        QueueError::Unknown { .. } => StageCause::Unknown,
        _ => return Err(queue_cannot(error)),
    };
    let stored = context
        .queue
        .get(&proposal.id)
        .ok()
        .flatten()
        .unwrap_or_else(|| proposal.clone());
    let document = with_diff(&stored, request.git(), &context.data_dir);
    Ok(StageOutcome::refused(
        cause,
        document,
        &format!("{error}; nothing staged"),
        messages,
    ))
}

/// The stage `body` names for `proposal`, checked as the terminal checks
/// its flags (see the module documentation), its `span_hash` not yet
/// read; else why not.
fn checked(
    context: &QueueContext,
    proposal: &Proposal,
    body: &StageBody,
) -> Result<Stage, (StageCause, String)> {
    let usage = |error: CliError| {
        let reason = error
            .message
            .strip_prefix("spec: ")
            .unwrap_or(&error.message)
            .to_owned();
        (StageCause::Usage, reason)
    };
    let refused = |reason: String| (StageCause::Refused, reason);
    match body {
        StageBody::Reject { reason, .. } => {
            if reason.trim().is_empty() {
                return Err(usage(CliError::spec(
                    "--reason is empty: say why the proposal is rejected",
                )));
            }
            if let Some(over) = over_cap("--reason", reason) {
                return Err(refused(over));
            }
            if let Some(found) = escaped_in("--reason", reason) {
                return Err(refused(found));
            }
            Ok(Stage::Reject {
                reason: reason.clone(),
            })
        }
        StageBody::Approve {
            option,
            answer,
            canon,
            note,
            ..
        } => {
            let flags = ApproveFlags {
                option: *option,
                answer: answer.clone(),
                canon: canon.clone(),
            };
            if proposal.kind.applies() {
                if let Some(reason) = flag_on_apply(proposal, &flags) {
                    return Err((StageCause::Usage, reason));
                }
            } else {
                let Some(intake) = proposal.intake.as_ref() else {
                    return Err((
                        StageCause::Refused,
                        format!(
                            "`{}` holds no fields of a {}",
                            proposal.id,
                            proposal.kind.as_str()
                        ),
                    ));
                };
                let scheme = &context.project.config.scheme;
                match first_checks(proposal, intake, &flags, scheme) {
                    Ok(Ok(_)) => {}
                    Ok(Err(reason)) => return Err(refused(reason)),
                    Err(error) => return Err(usage(error)),
                }
            }
            if let Some(over) = note.as_deref().and_then(|note| over_cap("--note", note)) {
                return Err(refused(over));
            }
            let texts = [("--answer", answer), ("--canon", canon), ("--note", note)];
            for (flag, text) in texts {
                if let Some(found) = text.as_deref().and_then(|text| escaped_in(flag, text)) {
                    return Err(refused(found));
                }
            }
            Ok(Stage::Approve {
                option: *option,
                answer: answer.clone(),
                canon: canon.clone(),
                note: note.clone(),
                span_hash: None,
            })
        }
    }
}

/// `flag`'s `text` holding a character the terminal shows escaped
/// ([`crate::escape_controls`]): the refusal, naming it.
fn escaped_in(flag: &str, text: &str) -> Option<String> {
    refused_char(text).map(|c| {
        format!(
            "{flag}: holds U+{:04X}: a staged choice never carries it; nothing changed",
            u32::from(c)
        )
    })
}

/// What `spec approve|reject` does with the proposal's staged choice (see
/// the module documentation).
#[derive(Debug, Clone, Default)]
pub(crate) struct Staging {
    /// The stage this run confirms: its flags are the run's.
    shown: Option<StagedChoice>,
    /// A stage left unused: the note naming it.
    unused: Option<String>,
    /// The staged command and the chosen option's line.
    lines: Vec<String>,
}

impl Staging {
    /// `spec approve`'s request and flags: as typed, any stage left unused
    /// (named) when a flag is typed (`--note` too) or a reject is staged;
    /// else a staged approve's flags and note.
    pub(crate) fn for_approve(
        proposal: &Proposal,
        request: &ApproveRequest,
        flags: &ApproveFlags,
    ) -> (ApproveRequest, ApproveFlags, Self) {
        let typed = request.note.is_some() || flags.any();
        let Some(staged) = &proposal.staged else {
            return (request.clone(), flags.clone(), Self::default());
        };
        match &staged.stage {
            Stage::Approve {
                option,
                answer,
                canon,
                note,
                ..
            } if !typed => {
                let taken = ApproveRequest {
                    note: note.clone(),
                    ..request.clone()
                };
                let flags = ApproveFlags {
                    option: *option,
                    answer: answer.clone(),
                    canon: canon.clone(),
                };
                let staging = Self::confirming(proposal, staged);
                (taken, flags, staging)
            }
            _ => (request.clone(), flags.clone(), Self::unused(staged)),
        }
    }

    /// `spec reject`'s reason: as typed (any stage left unused, named),
    /// else a staged reject's; none typed and none staged → exit 2 naming
    /// `--reason`; an approve staged → exit 2 naming `spec approve PR`.
    pub(crate) fn for_reject(
        proposal: &Proposal,
        reason: Option<&str>,
    ) -> Result<(String, Self), CliError> {
        let id = &proposal.id;
        match (reason, &proposal.staged) {
            (Some(reason), None) => Ok((reason.to_owned(), Self::default())),
            (Some(reason), Some(staged)) => Ok((reason.to_owned(), Self::unused(staged))),
            (None, Some(staged)) => match &staged.stage {
                Stage::Reject { reason } => {
                    Ok((reason.clone(), Self::confirming(proposal, staged)))
                }
                Stage::Approve { .. } => Err(CliError::spec(format!(
                    "`{id}` has an approve staged at {}: `spec approve {id}` confirms it; `spec \
                     reject {id} --reason T` rejects it; nothing changed",
                    staged.at
                ))),
            },
            (None, None) => Err(CliError::spec(format!(
                "`spec reject {id}` needs `--reason T`: no reject is staged on it; say why the \
                 proposal is rejected; nothing changed"
            ))),
        }
    }

    fn unused(staged: &StagedChoice) -> Self {
        Self {
            unused: Some(format!(
                "the choice staged {} is not used: the typed flags decide",
                staged.at
            )),
            ..Self::default()
        }
    }

    /// The stage `staged` confirmed by this run: shown as its command and
    /// the chosen option's line.
    fn confirming(proposal: &Proposal, staged: &StagedChoice) -> Self {
        let id = &proposal.id;
        let mut command = match &staged.stage {
            Stage::Approve { .. } => format!("spec approve {id}"),
            Stage::Reject { reason } => format!("spec reject {id} --reason {}", quoted(reason)),
        };
        let mut lines = Vec::new();
        if let Stage::Approve {
            option,
            answer,
            canon,
            note,
            ..
        } = &staged.stage
        {
            if let Some(option) = option {
                command.push_str(&format!(" --option {option}"));
            }
            for (flag, value) in [("--answer", answer), ("--canon", canon), ("--note", note)] {
                if let Some(value) = value {
                    command.push_str(&format!(" {flag} {}", quoted(value)));
                }
            }
            let chosen = option.and_then(|option| {
                let intake = proposal.intake.as_ref()?;
                let index = usize::try_from(option).ok()?;
                let line = option_line(index, intake.options.get(index)?, intake.recommendation);
                Some(format!("  {line}"))
            });
            lines.extend(chosen);
        }
        lines.insert(0, format!("staged {}: {command}", staged.at));
        Self {
            shown: Some(staged.clone()),
            unused: None,
            lines,
        }
    }

    /// The stage this run confirms.
    pub(crate) fn shown(&self) -> Option<&StagedChoice> {
        self.shown.as_ref()
    }

    /// The outcome's notes: the stage left unused.
    pub(crate) fn notes(&self) -> Vec<Message> {
        self.unused.iter().cloned().map(Message::Note).collect()
    }

    /// The lines above a question, each ending in LF, escaped: the unused
    /// stage's note; the staged command and the chosen option's line;
    /// `stale`, the staleness note. Each is one terminal line: a line feed,
    /// a carriage return or a tab inside it is written `\n`, `\r`, `\t`,
    /// every other control character escaped ([`crate::escape_controls`]).
    pub(crate) fn preface(&self, stale: Option<&str>) -> String {
        let note = |text: &str| Message::Note(text.to_owned()).line();
        let mut lines = Vec::new();
        lines.extend(self.unused.as_deref().map(note));
        lines.extend(self.lines.iter().cloned());
        lines.extend(stale.map(note));
        let mut out = String::new();
        for line in lines {
            let single = line
                .replace('\r', "\\r")
                .replace('\n', "\\n")
                .replace('\t', "\\t");
            out.push_str(&escape_controls(&single));
            out.push('\n');
        }
        out
    }

    /// `, staged` closing a question's parenthesis when it confirms a stage.
    pub(crate) fn mark(&self) -> &'static str {
        if self.shown.is_some() { ", staged" } else { "" }
    }

    /// The `staged_at` of the stage the decision confirms.
    pub(crate) fn staged_at(&self) -> Option<String> {
        self.shown.as_ref().map(|staged| staged.at.clone())
    }

    /// The stage shown, replaced or removed on a proposal still `open`
    /// (read again now): the refusal (exit 1, nothing changed); `None`
    /// when none was shown, the row is as shown, or another state took it
    /// (the queue's own refusal names that).
    pub(crate) fn replaced(&self, context: &QueueContext, id: &str) -> Option<String> {
        let shown = self.shown.as_ref()?;
        let row = context.queue.get(id).ok().flatten()?;
        (row.status == ProposalStatus::Open && row.staged.as_ref() != Some(shown)).then(|| {
            format!(
                "{id} changed since the question: its staged choice was replaced or removed; \
                 nothing changed"
            )
        })
    }
}

/// A completion's compare, which its recording does not make
/// (`applied_with`): the stage of `read`, the proposal as this run read it
/// (the one shown, one left unused, or none), against the row read again
/// now, still `open`, as step 7's compare-and-set compares it: changed →
/// the refusal (exit 1, nothing changed); `None` when it is as read or
/// another state took it.
pub(crate) fn changed_since(context: &QueueContext, read: &Proposal) -> Option<String> {
    let id = &read.id;
    let row = context.queue.get(id).ok().flatten()?;
    if row.status != ProposalStatus::Open || row.staged == read.staged {
        return None;
    }
    let what = if read.staged.is_none() {
        "a choice was staged on it meanwhile"
    } else {
        "its staged choice was replaced or removed"
    };
    Some(format!(
        "{id} changed since the question: {what}; nothing changed"
    ))
}

/// `text` double-quoted, `\` and `"` backslashed, and a line feed, a
/// carriage return and a tab written `\n`, `\r`, `\t`, so the staged
/// command stays one line on the terminal whatever its values hold (every
/// other control character is escaped with the line,
/// [`crate::escape_controls`]): no value can print lines of its own between
/// `staged <at>:` and the question.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
