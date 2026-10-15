//! `spec approve PR [--note T] [--editorial]` of a `drift` item (task spec
//! `drift-queue`, "Approve"; canon `proposal-drift`): the item's
//! node locked by the same accept `spec lock accept` runs
//! ([`crate::lock_accept`]), apply's steps around it. Nothing stored in the
//! item is read here: its rows and hash were for the owner's review.
//!
//! 1. `main` refuses a stdin that is no terminal, before anything is read;
//! 2. apply's place ([`place_step`]): the recorded worktree, `HEAD` on the
//!    recorded branch, no operation in progress (exit 2, logged); `HEAD`
//!    past the item's `base_commit` is a note;
//! 3. to 6. the accept of the item's `target_id` at the recorded root (not
//!    the current directory's), naming the item and the recorded branch
//!    (another branch: exit 1): its steps 2 to 7; text changed at the
//!    revision exits 1 naming `spec approve PR --editorial`; nothing to
//!    accept exits 1 `PR not applied: nothing to accept now for <ID>
//!    (<why>); spec reject PR --reason … settles it`, the item still open;
//!    each refusal logged at step 3, a missing committer identity at step
//!    7 (exit 2);
//! 7. the accept's rows and question (` for PR` in it) asked through the
//!    owner's consent; after `y`, `approved` as a compare-and-set on the
//!    state read (lost: `PR changed meanwhile; nothing written`);
//! 8. to 10. the accept's write and commit; what it left written
//!    ([`LockWritten`]), never its text: a verified commit records
//!    `applied` (`proposal.applied`); bytes put back reopen it (logged at
//!    step 9; also when git made a commit a hook reset, branch and `HEAD`
//!    unmoved: the accept restored it); a commit unverified or bytes left
//!    keep it `approved` (logged at step 10), exit 1; nothing written
//!    after the consent reopens it (step 8).
//!
//! A lock commit of the item already on its branch (its `Proposal:` and
//! `Lock:` trailers) completes it with no new commit, looked up first by
//! the approve ([`crate::preflight`]'s drift arm).

use specengine_store::{Decision, Proposal, ProposalQueue as _, QueueError, Seen};

use crate::apply::{ApproveRequest, Consent, failed, noted, recorded_already};
use crate::lock::{LockAcceptRequest, LockOutcome, LockPrompt, LockWritten, lock_accept};
use crate::preflight::{StepFailure, place_step, recorded_root};
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, queue_cannot, queue_refusal,
    with_diff,
};
use crate::{CliError, Env, Globals, Message};

/// The apply step a refusal of the accept before the question is logged
/// at: its steps 2 to 7 stand for apply's 3 to 6.
const ACCEPT_STEP: u8 = 3;
/// Apply's step 7: `approved` (a missing committer identity is logged at
/// it too, as apply's).
const APPROVE_STEP: u8 = 7;
/// The steps after the consent: the write, the commit (a restore), the
/// verification (a commit made, or bytes left).
const WRITE_STEP: u8 = 8;
const RESTORE_STEP: u8 = 9;
const VERIFY_STEP: u8 = 10;

/// The approve of the drift item `proposal` (see the module
/// documentation); `unknown`: the completion lookup's note when git could
/// not tell (shown before the question too).
#[allow(clippy::too_many_arguments)]
pub(crate) fn approve_drift(
    env: &Env,
    request: &ApproveRequest,
    context: &mut QueueContext,
    proposal: &Proposal,
    editorial: bool,
    unknown: Option<&str>,
    consent: Consent<'_>,
    mut messages: Vec<Message>,
) -> Result<ProposalOutcome, CliError> {
    const COMMAND: QueueCommand = QueueCommand::Approve;
    let id = proposal.id.as_str();
    let read = proposal.seen();

    // Step 2: apply's place.
    let placed = match place_step(&request.git, proposal) {
        Ok(placed) => placed,
        Err(mut failure) => {
            if let Some(unknown) = unknown {
                failure.reason.push_str(&format!("; {unknown}"));
            }
            return failed(context, request, proposal, None, failure, None, messages);
        }
    };
    if placed.head != proposal.place.base_commit {
        messages.push(Message::Note(format!("HEAD moved since {id} was raised")));
    }

    // Steps 3 to 10: the accept of the item's node at the recorded root,
    // on the recorded branch; step 7 inside the consent, after `y`.
    let globals = Globals {
        root: Some(recorded_root(proposal)),
        config: None,
    };
    let accept = LockAcceptRequest {
        reference: proposal.target_id.clone(),
        editorial,
        now: request.now.clone(),
        git: request.git.clone(),
        proposal: Some(proposal.id.clone()),
        branch: Some(proposal.place.branch.clone()),
    };
    let mut asked = false;
    let mut held: Option<(Seen, Decision)> = None;
    let mut lost: Option<StepFailure> = None;
    let accepted = {
        let mut ask = |prompt: &LockPrompt<'_>| -> bool {
            asked = true;
            // The rows before the question; both escaped by the accept.
            let question = format!("{}{}", prompt.rows, prompt.question);
            if !consent(&noted(unknown, &question)) {
                return false;
            }
            let decided_by = match placed.git.committer_ident() {
                Ok(ident) => ident,
                Err(error) => {
                    lost = Some(StepFailure::cannot(
                        APPROVE_STEP,
                        format!(
                            "no git identity in {}: {error}; set user.name and user.email",
                            proposal.place.worktree
                        ),
                    ));
                    return false;
                }
            };
            let decision = Decision {
                decided_by,
                note: request.note.clone(),
                staged_at: None,
            };
            match context
                .queue
                .approve_from(id, &read, &decision, &request.now)
            {
                Ok(approved) => {
                    held = Some((approved.seen(), decision));
                    true
                }
                Err(QueueError::Changed { .. } | QueueError::Status { .. }) => {
                    lost = Some(StepFailure::refused(
                        APPROVE_STEP,
                        format!("{id} changed meanwhile; nothing written"),
                    ));
                    false
                }
                Err(error @ QueueError::Invalid(_)) => {
                    lost = Some(StepFailure::refused(
                        APPROVE_STEP,
                        format!("{error}; nothing written"),
                    ));
                    false
                }
                Err(error) => {
                    lost = Some(StepFailure::cannot(
                        APPROVE_STEP,
                        format!("{error}; nothing written"),
                    ));
                    false
                }
            }
        };
        lock_accept(env, &globals, &accept, &mut ask, &[])
    };

    let outcome = match accepted {
        Ok(outcome) => outcome,
        Err(error) => {
            let reason = unprefixed(&error);
            return match &held {
                // After `y`: what the run holds reopened.
                Some((seen, _)) => {
                    let failure = StepFailure::cannot(WRITE_STEP, reason);
                    failed(
                        context,
                        request,
                        proposal,
                        Some(seen),
                        failure,
                        None,
                        messages,
                    )
                }
                None => {
                    // Before the question: the identity's step when it is
                    // the identity that is missing.
                    let step = if !asked && placed.git.committer_ident().is_err() {
                        APPROVE_STEP
                    } else {
                        ACCEPT_STEP
                    };
                    let failure = StepFailure::cannot(step, reason);
                    failed(context, request, proposal, None, failure, None, messages)
                }
            };
        }
    };
    let LockOutcome {
        messages: accept_messages,
        refusal,
        written,
        nothing,
        commit,
        ..
    } = outcome;
    messages.extend(accept_messages);

    let Some((seen, decision)) = held else {
        if let Some(failure) = lost {
            return failed(context, request, proposal, None, failure, None, messages);
        }
        if asked {
            let document = with_diff(proposal, &request.git, &context.data_dir);
            return Ok(ProposalOutcome::refused(
                COMMAND,
                document,
                &format!("`{id}` not applied: the answer was not `y`; nothing changed"),
                messages,
            ));
        }
        // Refused before the question: nothing to accept (Q6: the item
        // stays open; a reject settles it), or another refusal of the
        // accept's steps 2 to 7.
        if let Some(why) = nothing {
            let reason = format!(
                "{id} not applied: nothing to accept now for {} ({why}); spec reject {id} \
                 --reason … settles it",
                proposal.target_id
            );
            return logged_refusal(context, request, proposal, &reason, messages);
        }
        let reason = refusal.unwrap_or_else(|| "the lock accept wrote nothing".to_owned());
        let failure = StepFailure::refused(ACCEPT_STEP, reason);
        return failed(context, request, proposal, None, failure, None, messages);
    };

    match (written, commit) {
        (LockWritten::Committed, Some(commit)) => {
            let recorded = context
                .queue
                .applied_with(id, &commit, &decision, &request.now);
            let applied = match recorded {
                Ok(applied) => applied,
                Err(error) => match recorded_already(context, &error, &commit, &mut messages) {
                    Some(stored) => stored,
                    None => {
                        let reason = queue_refusal(error)?;
                        let document = with_diff(proposal, &request.git, &context.data_dir);
                        return Ok(ProposalOutcome::refused(
                            COMMAND, document, &reason, messages,
                        ));
                    }
                },
            };
            let document = with_diff(&applied, &request.git, &context.data_dir);
            Ok(ProposalOutcome::done(COMMAND, document, messages))
        }
        (written, _) => {
            let reason = refusal.unwrap_or_else(|| "the lock commit was not verified".to_owned());
            let step = match written {
                LockWritten::Restored => RESTORE_STEP,
                LockWritten::Unverified | LockWritten::Left => VERIFY_STEP,
                LockWritten::None | LockWritten::Committed => WRITE_STEP,
            };
            let failure = StepFailure::refused(step, reason);
            failed(
                context,
                request,
                proposal,
                Some(&seen),
                failure,
                None,
                messages,
            )
        }
    }
}

/// A refusal before step 7 in its own words: one `proposal.apply_failed`
/// logged at [`ACCEPT_STEP`], the state left as it is, exit 1 with the
/// stored document.
fn logged_refusal(
    context: &mut QueueContext,
    request: &ApproveRequest,
    proposal: &Proposal,
    reason: &str,
    mut messages: Vec<Message>,
) -> Result<ProposalOutcome, CliError> {
    let attempt = specengine_store::ApplyFailure {
        step: ACCEPT_STEP,
        reason: crate::one_line(reason),
    };
    let stored = match context
        .queue
        .log_failure(&proposal.id, &attempt, &request.now)
    {
        Ok(stored) => stored,
        Err(error @ QueueError::Status { .. }) => {
            messages.push(Message::Note(format!("no failure logged: {error}")));
            proposal.clone()
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    let document: ProposalDocument = with_diff(&stored, &request.git, &context.data_dir);
    Ok(ProposalOutcome::refused(
        QueueCommand::Approve,
        document,
        reason,
        messages,
    ))
}

/// A CLI error's message without its `spec: ` prefix.
fn unprefixed(error: &CliError) -> String {
    error
        .message
        .strip_prefix("spec: ")
        .unwrap_or(&error.message)
        .to_owned()
}
