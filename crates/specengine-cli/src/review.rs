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
//! no step run.
//! `--brief` ([`review_brief`], MCP `get_proposal`): the same document,
//! brief.

use specengine_store::{GitEnv, Proposal, ProposalStatus};

use crate::preflight::{completing, prepare, trailer_lookup};
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
/// preview.
pub(crate) fn previewed(
    env: &Env,
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> ProposalDocument {
    let mut document = with_diff(proposal, git_env, &context.data_dir);
    if !proposal.kind.applies()
        || !matches!(
            proposal.status,
            ProposalStatus::Open | ProposalStatus::Approved
        )
    {
        return document;
    }
    // Its branch read in the current repository when the recorded worktree
    // is not there; a lookup git cannot make is named. Its own commit
    // there: `approve` completes it, steps 2–6 are not run (no new apply).
    let own_commit = match trailer_lookup(git_env, context, proposal) {
        Ok(found) => completing(&found).map(|own| own.commit.clone()),
        Err(error) => {
            document.notes.push(one_line(&format!(
                "cannot tell whether its commit is on `{}`: {error}",
                proposal.place.branch
            )));
            None
        }
    };
    if let Some(commit) = own_commit {
        document.preview = Some(Preview::Unavailable);
        document.notes.push(format!(
            "its commit {commit} is on `{}`: `spec approve {}` completes it; no new apply is \
             needed",
            proposal.place.branch, proposal.id
        ));
        return document;
    }
    match prepare(env, git_env, context, proposal, messages) {
        Ok(prepared) => document.preview = Some(prepared.preview),
        Err(failure) => match failure.conflict {
            Some(conflict) => {
                document.preview = Some(Preview::Conflicts);
                document.conflict = Some(conflict);
            }
            None => {
                document.preview = Some(Preview::Unavailable);
                document.notes.push(one_line(&format!(
                    "not applicable now (step {}): {}",
                    failure.step, failure.reason
                )));
            }
        },
    }
    document
}
