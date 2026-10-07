//! `spec review PR` (task spec `proposal-apply`, "Data"): the review
//! document of one proposal of the current repository. An open or approved
//! one gets its `preview`, apply steps 2–6 run read-only (nothing written
//! in any worktree; the recorded root's index and git's scratch files live
//! in the data directory): `applies`, `rebases`, `conflicts` with the
//! merge's text, or `unavailable` with the step and its reason in `notes`.
//! An open or approved one whose own commit is already on its branch says
//! so in `notes` (`spec approve` completes it; no new apply is needed),
//! its preview `unavailable`, steps 2–6 not run; its branch read in the
//! current repository when the recorded worktree is not there; a lookup git
//! cannot make is a note too. A question or a discrepancy (canon
//! `agent-intake`, "Review document") never applies: no diff, no preview,
//! no step run. A create's new file (task spec `proposal-kinds`) is
//! previewed by its own steps 2–6 ([`crate::create`]): `applies` or
//! `unavailable`; its diff is from an empty base.
//! `--brief` ([`review_brief`], MCP `get_proposal`): the same document,
//! brief. A staged approve whose `span_hash` is not the target's now gets
//! the staleness note (canon `decision-staging`, "Staleness").

use specengine_store::{GitEnv, Proposal, ProposalStatus, Stage};

use crate::create::prepare_file;
use crate::preflight::{completing, prepare_spanned, trailer_lookup};
use crate::proposals::{
    Find, Preview, ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, briefed,
    escaped_error, find, no_proposal, open_context, with_diff, written_id,
};
use crate::{CliError, Env, Globals, Message, one_line};

/// `spec review` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRequest {
    /// `PR` as given.
    pub id: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// `spec review`: the proposal's document, its preview computed now.
pub fn review(
    env: &Env,
    globals: &Globals,
    request: &ReviewRequest,
) -> Result<ProposalOutcome, CliError> {
    run_review(env, globals, request).map_err(escaped_error)
}

/// `spec review PR --brief` (MCP `get_proposal`): [`review`], its answer
/// brief: the texts, `diff` and `conflict` dropped, at most 20 introduced
/// findings, the text cut at the output cap. Reads only, as `review`.
pub fn review_brief(
    env: &Env,
    globals: &Globals,
    request: &ReviewRequest,
) -> Result<ProposalOutcome, CliError> {
    review(env, globals, request).map(briefed)
}

fn run_review(
    env: &Env,
    globals: &Globals,
    request: &ReviewRequest,
) -> Result<ProposalOutcome, CliError> {
    const COMMAND: QueueCommand = QueueCommand::Review;
    let id = written_id(&request.id)?;
    let context = open_context(env, globals, &request.git)?;
    let Some(id) = id else {
        return Ok(ProposalOutcome::refused(
            COMMAND,
            ProposalDocument::default(),
            &no_proposal(&request.id),
            Vec::new(),
        ));
    };
    let proposal = match find(&context, &id, Find::SameRepository)? {
        Ok(proposal) => proposal,
        Err(reason) => {
            return Ok(ProposalOutcome::refused(
                COMMAND,
                ProposalDocument::default(),
                &reason,
                Vec::new(),
            ));
        }
    };
    let mut messages = Vec::new();
    let document = previewed(env, &request.git, &context, &proposal, &mut messages);
    Ok(ProposalOutcome::done(COMMAND, document, messages))
}

/// The proposal's document with its diff and, open or approved, its
/// preview; a staged approve's staleness note.
pub(crate) fn previewed(
    env: &Env,
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> ProposalDocument {
    let run = preview_run(env, git_env, context, proposal, messages);
    document_with(git_env, context, proposal, run)
}

/// What steps 2–6, read-only, gave for a proposal: its preview, a
/// conflict's text, the notes, and the target's span hash as step 5 read
/// it (an `update`'s or a section-form `create`'s, once resolved).
#[derive(Debug, Default)]
pub(crate) struct PreviewRun {
    preview: Option<Preview>,
    conflict: Option<String>,
    notes: Vec<String>,
    /// The target's span hash now; `None` unread.
    pub span: Option<String>,
}

/// Steps 2–6 for an open or approved `update` or `create`, read-only
/// ([`previewed`]); nothing run for any other.
pub(crate) fn preview_run(
    env: &Env,
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> PreviewRun {
    let mut run = PreviewRun::default();
    if !proposal.kind.applies()
        || !matches!(
            proposal.status,
            ProposalStatus::Open | ProposalStatus::Approved
        )
    {
        return run;
    }
    // Its branch read in the current repository when the recorded worktree
    // is not there; a lookup git cannot make is named. Its own commit
    // there: `approve` completes it, steps 2–6 are not run (no new apply).
    let own_commit = match trailer_lookup(git_env, context, proposal) {
        Ok(found) => completing(&found).map(|own| own.commit.clone()),
        Err(error) => {
            run.notes.push(one_line(&format!(
                "cannot tell whether its commit is on `{}`: {error}",
                proposal.place.branch
            )));
            None
        }
    };
    if let Some(commit) = own_commit {
        run.preview = Some(Preview::Unavailable);
        run.notes.push(format!(
            "its commit {commit} is on `{}`: `spec approve {}` completes it; no new apply is \
             needed",
            proposal.place.branch, proposal.id
        ));
        return run;
    }
    let prepared = if proposal.new_file() {
        prepare_file(env, git_env, context, proposal, messages).map(|_| Preview::Applies)
    } else {
        prepare_spanned(env, git_env, context, proposal, messages, &mut run.span)
            .map(|prepared| prepared.preview)
    };
    match prepared {
        Ok(preview) => run.preview = Some(preview),
        Err(failure) => match failure.conflict {
            Some(conflict) => {
                run.preview = Some(Preview::Conflicts);
                run.conflict = Some(conflict);
            }
            None => {
                run.preview = Some(Preview::Unavailable);
                run.notes.push(one_line(&format!(
                    "not applicable now (step {}): {}",
                    failure.step, failure.reason
                )));
            }
        },
    }
    run
}

/// `proposal`'s document with its diff and `run`'s preview, conflict and
/// notes, then the staleness note of its staged approve
/// ([`stale_note`]) when the change applies now (a conflict or an
/// unavailable preview says what it does instead).
pub(crate) fn document_with(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    run: PreviewRun,
) -> ProposalDocument {
    let mut document = with_diff(proposal, git_env, &context.data_dir);
    let applies = matches!(run.preview, Some(Preview::Applies | Preview::Rebases));
    document.preview = run.preview;
    document.conflict = run.conflict;
    document.notes.extend(run.notes);
    if applies && let Some(note) = stale_note(proposal, run.span.as_deref()) {
        document.notes.push(note);
    }
    document
}

/// Canon `decision-staging`, "Staleness": a staged approve whose
/// `span_hash` is not the target's span hash `now` (step 5's reading):
/// `staged against <h1>; the target is now <h2>: the change applies as it
/// rebases`. Never a refusal; `None` when either hash is unknown or they
/// are equal.
pub(crate) fn stale_note(proposal: &Proposal, now: Option<&str>) -> Option<String> {
    let staged = proposal.staged.as_ref()?;
    let Stage::Approve {
        span_hash: Some(then),
        ..
    } = &staged.stage
    else {
        return None;
    };
    let now = now?;
    (then != now).then(|| {
        format!("staged against {then}; the target is now {now}: the change applies as it rebases")
    })
}
