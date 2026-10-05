//! `spec approve PR [--note T]` and `spec reject PR --reason T` (task spec
//! `proposal-apply`, "Apply", "Idempotence", "Reject"): the one write door.
//!
//! The apply, in the recorded worktree only:
//!
//! 1. consent: `main` refuses a stdin that is no terminal (no event); after
//!    the read-only steps the `consent` callback is asked `apply PR-0001 to
//!    <path> on <branch> in <worktree> (applies|rebases)? [y/N]`;
//! 2. to 6. place, file, resolve, text, structure: [`crate::preflight`];
//! 7. `approved`, `decided_by` the worktree's committer identity (none:
//!    exit 2, asked before the prompt), as a compare-and-set on the state
//!    this run read (`open`, or `approved` for a run that started
//!    `approved`): another run's change since refuses (exit 1), leaving
//!    the state as it is;
//! 8. write: the place re-checked (`HEAD` on the recorded branch at the
//!    commit step 2 found, no operation in progress), the file read again,
//!    equal to step 3's bytes, replaced atomically, its mode kept;
//! 9. commit: `git commit --only` of the one path with the message and its
//!    four trailers, hooks run; a failure restores the bytes and reopens,
//!    unless git made this run's commit anyway (the branch's or `HEAD`'s
//!    new commit carries the `Proposal:` trailer): then step 10 judges it;
//! 10. verify: the new commit's one parent is the old `HEAD`, it changes
//!     only the path, its `Proposal:` trailer is the ID → `applied` (even
//!     when the hold was reopened meanwhile: the commit is this run's), the
//!     index updated (failure: a warning); else it stays `approved`, exit 1
//!     naming the commit (the branch did not move: the commit `HEAD` names
//!     now).
//!
//! Every refusal at steps 2–10 logs one `proposal.apply_failed`. Only a
//! run that wrote `approved` at its own step 7 reopens, and only while the
//! proposal is still in that state; before it the run holds nothing and
//! changes no state (another run may hold it). The proposal's own commit
//! on its branch (`Proposal:` trailer in `base_commit..<branch>`, the
//! branch's whole history when the base commit is pruned; one parent,
//! changing only the path to step 5's text on its parent's blob or its
//! own) completes it without a new commit, looked up first (in the current
//! repository when the recorded worktree is not there; a lookup git cannot
//! make is a note, naming the way out, shown before the consent prompt
//! too) and again at step 5:
//! `approved` → `applied`; `open` → `applied` after the consent `complete
//! PR-0001 by its commit <sha> on <branch> in <worktree>? [y/N]` (the
//! current project root when the lookup read the current repository),
//! logging `proposal.approved` and `proposal.applied`. Git runs `-C
//! <worktree>` (that lookup: `-C` the current project root), stdin null,
//! never with the caller's local `GIT_*` variables; nothing outside the
//! recorded worktree and the data directory is written.

use std::path::Path;

use specengine_core::proposal::{CommitFacts, PROPOSAL_TRAILER, commit_message};
use specengine_store::{
    ApplyFailure, Decision, GitEnv, IndexWriter as _, Proposal, ProposalQueue as _, ProposalStatus,
    QueueError, Seen, Source as _, WorkingTree, replace_file, same_repository,
};

use crate::location::open_index;
use crate::preflight::{
    History, Missing, Prepared, StepFailure, completes_when, completing, history, place_unchanged,
    prepare, recorded_project, recorded_root, trailer_lookup, trailer_lookup_here,
};
use crate::proposals::{
    Find, Preview, ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, checked_now,
    escaped_error, find, no_proposal, open_context, queue_cannot, queue_refusal, top_path,
    with_diff, written_id,
};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// `spec approve` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveRequest {
    /// `PR` as given.
    pub id: String,
    /// `--note T`: kept as the decision's note.
    pub note: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// `spec reject` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectRequest {
    /// `PR` as given.
    pub id: String,
    /// `--reason T`: non-empty.
    pub reason: String,
    pub now: String,
    pub git: GitEnv,
}

/// The owner's answer to one question (`main`: a `[y/N]` prompt on
/// stderr, the answer read from the terminal; only `y` or `yes`
/// consents).
pub type Consent<'a> = &'a mut dyn FnMut(&str) -> bool;

/// `spec approve`: applies an open proposal (steps 2–10), or completes one
/// whose own commit is on its branch.
pub fn approve(
    env: &Env,
    globals: &Globals,
    request: &ApproveRequest,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    run_approve(env, globals, request, consent).map_err(escaped_error)
}

fn run_approve(
    env: &Env,
    globals: &Globals,
    request: &ApproveRequest,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    const COMMAND: QueueCommand = QueueCommand::Approve;
    let now = checked_now(&request.now)?;
    let id = written_id(&request.id)?;
    let mut context = open_context(env, globals, &request.git)?;
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
    // The state this run read: the key of its every queue change.
    let read = proposal.seen();
    let mut messages = Vec::new();
    let document = |proposal: &Proposal, scratch: &Path| with_diff(proposal, &request.git, scratch);
    match proposal.status {
        ProposalStatus::Applied => {
            let reason = format!(
                "`{id}` is already applied: commit {}",
                proposal.applied_commit.as_deref().unwrap_or("unknown")
            );
            return Ok(ProposalOutcome::refused(
                COMMAND,
                document(&proposal, &context.data_dir),
                &reason,
                messages,
            ));
        }
        ProposalStatus::Rejected => {
            let reason = format!("`{id}` is rejected: a rejected proposal is never applied");
            return Ok(ProposalOutcome::refused(
                COMMAND,
                document(&proposal, &context.data_dir),
                &reason,
                messages,
            ));
        }
        ProposalStatus::Open | ProposalStatus::Approved => {}
    }
    // The proposal's own commit already on its branch: completed, no new
    // commit. Its branch read in the current repository when the recorded
    // worktree is not there; a lookup git cannot make is named (a note,
    // shown before the consent prompt too, or in the reason of an exit 2 at
    // steps 2–6, which has no notes).
    let mut unknown = None;
    match trailer_lookup(&request.git, &context, &proposal) {
        Ok(found) => {
            if let Some(own) = completing(&found) {
                let commit = own.commit.clone();
                return complete(
                    env,
                    request,
                    &mut context,
                    &proposal,
                    &commit,
                    consent,
                    messages,
                );
            }
        }
        Err(error) => {
            let note = one_line(&format!(
                "cannot tell whether `{id}` has its commit on `{}`: {error}",
                proposal.place.branch
            ));
            messages.push(Message::Note(note.clone()));
            unknown = Some(note);
        }
    }

    // Until step 7 writes `approved`, this run holds nothing: a refusal
    // only logs, whatever state another run left.
    let prepared = match prepare(env, &request.git, &context, &proposal, &mut messages) {
        Ok(prepared) => prepared,
        Err(mut failure) => {
            if let Some(commit) = &failure.completing {
                let mut asked = |question: &str| consent(&noted(unknown.as_deref(), question));
                return complete(
                    env,
                    request,
                    &mut context,
                    &proposal,
                    commit,
                    &mut asked,
                    messages,
                );
            }
            if failure.exit == Exit::CannotRun
                && let Some(unknown) = &unknown
            {
                failure.reason.push_str(&format!("; {unknown}"));
            }
            let preview = if failure.conflict.is_some() {
                Preview::Conflicts
            } else {
                Preview::Unavailable
            };
            return failed(
                &mut context,
                request,
                &proposal,
                None,
                failure,
                preview,
                messages,
            );
        }
    };
    let decided_by = match prepared.git.committer_ident() {
        Ok(ident) => ident,
        Err(error) => {
            let failure = StepFailure::cannot(
                7,
                format!(
                    "no git identity in {}: {error}; set user.name and user.email",
                    proposal.place.worktree
                ),
            );
            return failed(
                &mut context,
                request,
                &proposal,
                None,
                failure,
                prepared.preview,
                messages,
            );
        }
    };
    let question = format!(
        "apply {id} to {} on {} in {} ({})? [y/N]",
        prepared.top_path,
        proposal.place.branch,
        proposal.place.worktree,
        prepared.preview.as_str()
    );
    if !consent(&noted(unknown.as_deref(), &escape_controls(&question))) {
        let mut document = with_diff(&proposal, &request.git, &context.data_dir);
        document.preview = Some(prepared.preview);
        return Ok(ProposalOutcome::refused(
            COMMAND,
            document,
            &format!("`{id}` not applied: the answer was not `y`; nothing changed"),
            messages,
        ));
    }

    // Step 7: a compare-and-set on the state read.
    let decision = Decision {
        decided_by: decided_by.clone(),
        note: request.note.clone(),
    };
    let held = match context.queue.approve_from(&id, &read, &decision, now) {
        Ok(approved) => approved.seen(),
        Err(
            error @ (QueueError::Changed { .. }
            | QueueError::Status { .. }
            | QueueError::Invalid(_)),
        ) => {
            let failure = StepFailure::refused(7, format!("{error}; nothing written"));
            return failed(
                &mut context,
                request,
                &proposal,
                None,
                failure,
                prepared.preview,
                messages,
            );
        }
        Err(error) => return Err(queue_cannot(error)),
    };

    // Step 8: the place and the file as checked, then the write.
    let path = proposal.target_path.as_str();
    let file = prepared.tree.root().join(path);
    let unchanged =
        place_unchanged(&prepared, &proposal.place).and_then(|()| match prepared.tree.read(path) {
            Ok(bytes) if bytes == prepared.bytes => Ok(()),
            Ok(_) => Err(StepFailure::refused(
                8,
                format!("`{path}` changed while being applied; nothing written"),
            )),
            Err(error) => Err(StepFailure::refused(
                8,
                format!("cannot read `{path}` again: {error}; nothing written"),
            )),
        });
    if let Err(failure) = unchanged {
        return failed(
            &mut context,
            request,
            &proposal,
            Some(&held),
            failure,
            prepared.preview,
            messages,
        );
    }
    if let Err(error) = replace_file(&file, &prepared.patched) {
        let failure = StepFailure::refused(
            8,
            format!("cannot write `{path}`: {error}; nothing written"),
        );
        return failed(
            &mut context,
            request,
            &proposal,
            Some(&held),
            failure,
            prepared.preview,
            messages,
        );
    }

    // Step 9.
    let message = commit_message(&CommitFacts {
        id: &id,
        rationale: &proposal.rationale,
        decided_by: &decided_by,
        author: &proposal.author,
        base_commit: &proposal.place.base_commit,
    });
    if let Err(error) = prepared
        .git
        .commit_only(&context.data_dir, &message, &prepared.top_path)
    {
        // Git can fail after making the commit (an index write): this
        // run's commit, on the branch or where `HEAD` went, is judged by
        // step 10, never overwritten by a restore. Anything else moving
        // the branch is not this run's commit.
        let branch = &proposal.place.branch;
        match made_anyway(&prepared, &proposal) {
            Made::Commit { tip, on_branch } => {
                let moved = if on_branch {
                    format!("`{branch}` moved to {tip}")
                } else {
                    format!("HEAD moved to {tip}")
                };
                messages.push(Message::Warning(format!(
                    "the commit reported a failure, yet {moved}: {error}"
                )));
            }
            Made::Nothing { foreign } => {
                let restored = match replace_file(&file, &prepared.bytes) {
                    Ok(()) => "its bytes restored".to_owned(),
                    Err(restore) => {
                        messages.push(Message::Warning(format!(
                            "cannot restore `{path}` in {}: {restore}; it holds the \
                             proposal's bytes, uncommitted",
                            proposal.place.worktree
                        )));
                        "its bytes NOT restored".to_owned()
                    }
                };
                let foreign = foreign.map_or_else(String::new, |tip| {
                    format!(
                        "; `{branch}` moved to {tip}, which has no `{PROPOSAL_TRAILER}: {id}` \
                         trailer (not this apply's commit)"
                    )
                });
                let failure = StepFailure::refused(
                    9,
                    format!("the commit of `{path}` failed, {restored}: {error}{foreign}"),
                );
                return failed(
                    &mut context,
                    request,
                    &proposal,
                    Some(&held),
                    failure,
                    prepared.preview,
                    messages,
                );
            }
        }
    }

    // Step 10.
    // This run's own commit verified: `applied`, even when its hold was
    // reopened meanwhile.
    match verify(&prepared, &proposal) {
        Ok(commit) => {
            let applied = match context.queue.applied_with(&id, &commit, &decision, now) {
                Ok(applied) => applied,
                Err(error) => match recorded_already(&context, &error, &commit, &mut messages) {
                    Some(stored) => stored,
                    None => {
                        let reason = queue_refusal(error)?;
                        let document = with_diff(&proposal, &request.git, &context.data_dir);
                        return Ok(ProposalOutcome::refused(
                            COMMAND, document, &reason, messages,
                        ));
                    }
                },
            };
            let Prepared {
                mut index,
                tree,
                recorded,
                ..
            } = prepared;
            if let Err(error) = index
                .index
                .update_paths(&tree, &recorded.config.scheme, &[path])
            {
                messages.push(Message::Warning(format!(
                    "the index was not updated after the commit: {error}; the next command \
                     updates it"
                )));
            }
            let document = with_diff(&applied, &request.git, &context.data_dir);
            Ok(ProposalOutcome::done(COMMAND, document, messages))
        }
        Err(reason) => {
            let failure = StepFailure::refused(10, reason);
            let preview = prepared.preview;
            failed(
                &mut context,
                request,
                &proposal,
                Some(&held),
                failure,
                preview,
                messages,
            )
        }
    }
}

/// Step 10: the branch's new commit, when its one parent is the old `HEAD`,
/// it changes only the path and its `Proposal:` trailer is the ID; else
/// why not, naming the commit.
fn verify(prepared: &Prepared, proposal: &Proposal) -> Result<String, String> {
    let git = &prepared.git;
    let branch = &proposal.place.branch;
    let commit = match git.commit_of(&format!("refs/heads/{branch}")) {
        Ok(Some(commit)) => commit,
        Ok(None) => return Err(format!("`{branch}` names no commit after the commit")),
        Err(error) => return Err(format!("cannot read `{branch}` after the commit: {error}")),
    };
    if commit == prepared.head {
        let head = match git.head() {
            Ok(Some(head)) if head != prepared.head => head,
            Ok(_) => {
                return Err(format!(
                    "`{branch}` did not move from {commit} and HEAD names no new commit; `{}` \
                     stays approved",
                    proposal.id
                ));
            }
            Err(error) => {
                return Err(format!(
                    "`{branch}` did not move from {commit}, and HEAD cannot be read: {error}; \
                     `{}` stays approved",
                    proposal.id
                ));
            }
        };
        let on = match git.branch() {
            Ok(Some(other)) => format!("on `{other}`"),
            Ok(None) => "detached".to_owned(),
            Err(_) => "on an unknown branch".to_owned(),
        };
        return Err(format!(
            "`{branch}` did not move from {commit}: the commit went to {head} (HEAD {on}); `{}` \
             stays approved: once that commit is on `{branch}`, `spec approve` completes it",
            proposal.id
        ));
    }
    let stays = |why: String| {
        format!(
            "the commit {commit} on `{branch}` is not the apply commit ({why}); `{}` stays \
             approved; {}",
            proposal.id,
            completes_when(proposal)
        )
    };
    match git.parents(&commit) {
        Ok(parents) if parents == [prepared.head.as_str()] => {}
        Ok(parents) => return Err(stays(format!("its parents are {}", parents.join(" ")))),
        Err(error) => return Err(stays(error.to_string())),
    }
    match git.changed_paths(&prepared.head, &commit) {
        Ok(changed) if changed == [prepared.top_path.as_str()] => {}
        Ok(changed) => return Err(stays(format!("it changes {}", changed.join(", ")))),
        Err(error) => return Err(stays(error.to_string())),
    }
    match git.trailer_values(&commit, PROPOSAL_TRAILER) {
        Ok(values) if values.contains(&proposal.id) => Ok(commit),
        Ok(_) => Err(stays(format!(
            "no `{PROPOSAL_TRAILER}: {}` trailer",
            proposal.id
        ))),
        Err(error) => Err(stays(error.to_string())),
    }
}

/// A refusal at step 2–10: one `proposal.apply_failed` logged. `held`,
/// the state this run wrote at step 7 ([`Seen`]), is reopened (unless at
/// step 10) only while it is still that state; before step 7 (`None`) the
/// run holds nothing and the state stays as it is, whoever left it. Exit
/// 2 as an error, exit 1 as the refused document. A proposal applied or
/// rejected meanwhile logs nothing; its document is the stored one.
fn failed(
    context: &mut QueueContext,
    request: &ApproveRequest,
    proposal: &Proposal,
    held: Option<&Seen>,
    failure: StepFailure,
    preview: Preview,
    mut messages: Vec<Message>,
) -> Result<ProposalOutcome, CliError> {
    let attempt = ApplyFailure {
        step: failure.step,
        reason: failure.reason.clone(),
    };
    let logged = match held {
        Some(held) => context
            .queue
            .reopen_from(&proposal.id, held, &attempt, &request.now),
        None => context
            .queue
            .log_failure(&proposal.id, &attempt, &request.now),
    };
    let logged = match logged {
        Ok(logged) => logged,
        Err(error @ QueueError::Status { .. }) => {
            messages.push(Message::Note(format!("no failure logged: {error}")));
            match context.queue.get(&proposal.id) {
                Ok(Some(stored)) => stored,
                Ok(None) => proposal.clone(),
                Err(error) => return Err(queue_cannot(error)),
            }
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    if failure.exit == Exit::CannotRun {
        return Err(CliError::spec(format!(
            "`{}` not applied (step {}): {}",
            proposal.id, failure.step, failure.reason
        )));
    }
    let mut document = with_diff(&logged, &request.git, &context.data_dir);
    if matches!(
        logged.status,
        ProposalStatus::Open | ProposalStatus::Approved
    ) {
        document.preview = Some(preview);
        document.conflict = failure.conflict;
    }
    let reason = format!(
        "`{}` not applied (step {}): {}",
        proposal.id, failure.step, failure.reason
    );
    Ok(ProposalOutcome::refused(
        QueueCommand::Approve,
        document,
        &reason,
        messages,
    ))
}

/// What a failed `git commit` left: this run's commit (its `Proposal:`
/// trailer) as the branch's or `HEAD`'s new commit, or nothing of it.
enum Made {
    Commit {
        tip: String,
        /// The branch's tip; else where `HEAD` went.
        on_branch: bool,
    },
    Nothing {
        /// The branch's new tip that is not this run's commit.
        foreign: Option<String>,
    },
}

/// After a failed `git commit` at step 9: whether git made this run's
/// commit anyway, by the new tip of the branch, else of `HEAD`, carrying
/// the proposal's `Proposal:` trailer.
fn made_anyway(prepared: &Prepared, proposal: &Proposal) -> Made {
    let git = &prepared.git;
    let own = |tip: &str| {
        git.trailer_values(tip, PROPOSAL_TRAILER)
            .is_ok_and(|values| values.contains(&proposal.id))
    };
    let branch_tip = match git.commit_of(&format!("refs/heads/{}", proposal.place.branch)) {
        Ok(Some(tip)) if tip != prepared.head => Some(tip),
        _ => None,
    };
    if let Some(tip) = branch_tip.as_deref().filter(|tip| own(tip)) {
        return Made::Commit {
            tip: tip.to_owned(),
            on_branch: true,
        };
    }
    if let Ok(Some(tip)) = git.head()
        && tip != prepared.head
        && branch_tip.as_deref() != Some(tip.as_str())
        && own(&tip)
    {
        return Made::Commit {
            tip,
            on_branch: false,
        };
    }
    Made::Nothing {
        foreign: branch_tip,
    }
}

/// The consent question with the lookup's note before it (`unknown`: git
/// could not tell whether the proposal's commit is on its branch), escaped:
/// the owner reads it before answering.
fn noted(unknown: Option<&str>, question: &str) -> String {
    match unknown {
        Some(note) => format!(
            "{}\n{question}",
            escape_controls(&Message::Note(note.to_owned()).line())
        ),
        None => question.to_owned(),
    }
}

/// The proposal's own commit (`commit`, [`completing`]) on its branch
/// completes it, no new commit: `approved` → `applied`, its decision kept;
/// `open` → `applied` after the owner's consent (`complete PR-0001 by its
/// commit <sha> on <branch> in <worktree>? [y/N]`), decided by the
/// worktree's committer identity, the current repository's when the
/// worktree is not there (none: exit 2 before the prompt). Nothing but the
/// queue and the data directory's index is written.
fn complete(
    env: &Env,
    request: &ApproveRequest,
    context: &mut QueueContext,
    proposal: &Proposal,
    commit: &str,
    consent: Consent<'_>,
    mut messages: Vec<Message>,
) -> Result<ProposalOutcome, CliError> {
    const COMMAND: QueueCommand = QueueCommand::Approve;
    let id = &proposal.id;
    let recorded = if proposal.status == ProposalStatus::Open {
        // The worktree's committer; the current repository's (the same
        // repository) when the worktree is not there.
        // `read`: where the commit was looked up, named by the prompt.
        let worktree = &proposal.place.worktree;
        let (decided_by, place, read) = match history(&request.git, context, proposal) {
            Ok(History::Recorded(git)) => (
                git.committer_ident().ok(),
                worktree.clone(),
                worktree.clone(),
            ),
            Ok(History::Current(git)) => (
                git.committer_ident().ok(),
                format!(
                    "{} (the worktree {worktree} is not there)",
                    git.dir().display()
                ),
                git.dir().display().to_string(),
            ),
            Err(_) => (None, worktree.clone(), worktree.clone()),
        };
        let Some(decided_by) = decided_by else {
            let failure = StepFailure::cannot(
                7,
                format!("no git identity in {place}; set user.name and user.email"),
            );
            return failed(
                context,
                request,
                proposal,
                None,
                failure,
                Preview::Unavailable,
                messages,
            );
        };
        let question = format!(
            "complete {id} by its commit {commit} on {} in {read}? [y/N]",
            proposal.place.branch
        );
        if !consent(&escape_controls(&question)) {
            let document = with_diff(proposal, &request.git, &context.data_dir);
            return Ok(ProposalOutcome::refused(
                COMMAND,
                document,
                &format!("`{id}` not completed: the answer was not `y`; nothing changed"),
                messages,
            ));
        }
        let decision = Decision {
            decided_by,
            note: request.note.clone(),
        };
        context
            .queue
            .applied_with(id, commit, &decision, &request.now)
    } else {
        context.queue.applied(id, commit, &request.now)
    };
    let applied = match recorded {
        Ok(applied) => applied,
        Err(error) => match recorded_already(context, &error, commit, &mut messages) {
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
    messages.push(Message::Note(format!(
        "`{id}` completed by its commit {commit} on `{}`; no new commit",
        proposal.place.branch
    )));
    reindex(env, context, &applied, &mut messages);
    let document = with_diff(&applied, &request.git, &context.data_dir);
    Ok(ProposalOutcome::done(COMMAND, document, messages))
}

/// `applied` with `commit` refused because another run (a completion)
/// recorded the same commit meanwhile: the stored proposal, with a note.
fn recorded_already(
    context: &QueueContext,
    error: &QueueError,
    commit: &str,
    messages: &mut Vec<Message>,
) -> Option<Proposal> {
    let QueueError::Status {
        id,
        status: ProposalStatus::Applied,
        applied_commit: Some(stored),
    } = error
    else {
        return None;
    };
    if stored != commit {
        return None;
    }
    let stored = context.queue.get(id).ok().flatten()?;
    messages.push(Message::Note(format!(
        "`{id}` was recorded applied with {commit} by another run meanwhile"
    )));
    Some(stored)
}

/// The recorded root's index brought up to date for the applied path
/// after a completion; a failure is a warning.
fn reindex(env: &Env, context: &QueueContext, proposal: &Proposal, messages: &mut Vec<Message>) {
    let updated = recorded_project(&recorded_root(proposal), &context.slug).and_then(|recorded| {
        let mut open = open_index(env, &recorded).map_err(|error| error.message)?;
        let tree = WorkingTree::new(&recorded.root, &recorded.config.paths)
            .map_err(|error| error.to_string())?;
        open.index
            .update_paths(&tree, &recorded.config.scheme, &[&proposal.target_path])
            .map(|_| ())
            .map_err(|error| error.to_string())
    });
    if let Err(error) = updated {
        messages.push(Message::Warning(format!(
            "the index was not updated after the completion: {error}; the next command \
             updates it"
        )));
    }
}

/// `spec reject`: an open or approved proposal rejected with its reason,
/// the owner's explicit action (the prompt names an approved one), as a
/// compare-and-set on the state read (another run's change since: exit 1);
/// also an orphan (its recorded common dir gone: its branch read in the
/// current repository). Refused (checked before the prompt and again after
/// it, nothing written, no event) when a commit on its branch carries its
/// `Proposal:` trailer, naming it (and why it does not complete it, if
/// so), or when git cannot tell (naming the way out); an orphan's lookup
/// is skipped only when neither the branch nor the base commit is in the
/// current repository. Decided by the committer of its [`History`], else
/// of the current repository.
pub fn reject(
    env: &Env,
    globals: &Globals,
    request: &RejectRequest,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    run_reject(env, globals, request, consent).map_err(escaped_error)
}

fn run_reject(
    env: &Env,
    globals: &Globals,
    request: &RejectRequest,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    const COMMAND: QueueCommand = QueueCommand::Reject;
    let now = checked_now(&request.now)?;
    if request.reason.trim().is_empty() {
        return Err(CliError::spec(
            "--reason is empty: say why the proposal is rejected",
        ));
    }
    let id = written_id(&request.id)?;
    let mut context = open_context(env, globals, &request.git)?;
    let Some(id) = id else {
        return Ok(ProposalOutcome::refused(
            COMMAND,
            ProposalDocument::default(),
            &no_proposal(&request.id),
            Vec::new(),
        ));
    };
    let proposal = match find(&context, &id, Find::OrGoneRepository)? {
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
    // `find` gives another repository's proposal only when it is gone.
    let orphan = !same_repository(&proposal.place.git_common_dir, &context.common_dir);
    if orphan {
        messages.push(Message::Note(format!(
            "`{id}` belongs to the repository {}, which no longer exists",
            proposal.place.git_common_dir
        )));
    }
    let document = with_diff(&proposal, &request.git, &context.data_dir);
    let rejectable = match proposal.status {
        ProposalStatus::Open | ProposalStatus::Approved => true,
        ProposalStatus::Applied | ProposalStatus::Rejected => false,
    };
    if !rejectable {
        let mut reason = format!(
            "`{id}` is {}: only an open or approved proposal is rejected",
            proposal.status
        );
        if let Some(commit) = &proposal.applied_commit {
            reason.push_str(&format!(" (commit {commit})"));
        }
        return Ok(ProposalOutcome::refused(
            COMMAND, document, &reason, messages,
        ));
    }
    // A proposal with a commit in history is never recorded as rejected;
    // nor one whose history git cannot read.
    if let Some(reason) = committed(&request.git, &context, &proposal, orphan) {
        return Ok(ProposalOutcome::refused(
            COMMAND, document, &reason, messages,
        ));
    }
    // The committer of the proposal's history ([`History`]), else of the
    // current repository.
    let decided_by = history(&request.git, &context, &proposal)
        .ok()
        .and_then(|history| history.git().committer_ident().ok())
        .or_else(|| context.git.committer_ident().ok())
        .ok_or_else(|| {
            CliError::spec(format!(
                "no git identity in {} or in {}: set user.name and user.email",
                proposal.place.worktree,
                context.project.root.display()
            ))
        })?;
    let approved = if proposal.status == ProposalStatus::Approved {
        "approved "
    } else {
        ""
    };
    let question = format!(
        "reject {approved}{id} ({} in {} on {} in {})? [y/N]",
        proposal.target_id,
        top_path(&proposal),
        proposal.place.branch,
        proposal.place.worktree
    );
    if !consent(&escape_controls(&question)) {
        return Ok(ProposalOutcome::refused(
            COMMAND,
            document,
            &format!("`{id}` not rejected: the answer was not `y`; nothing changed"),
            messages,
        ));
    }
    // Its commit may have landed while the owner was asked.
    if let Some(reason) = committed(&request.git, &context, &proposal, orphan) {
        return Ok(ProposalOutcome::refused(
            COMMAND, document, &reason, messages,
        ));
    }
    let decision = Decision {
        decided_by,
        note: Some(request.reason.clone()),
    };
    // A compare-and-set on the state read: another run's change since
    // refuses (exit 1).
    let rejected = context
        .queue
        .reject_from(&id, &proposal.seen(), &decision, now);
    match rejected {
        Ok(rejected) => {
            let document = with_diff(&rejected, &request.git, &context.data_dir);
            Ok(ProposalOutcome::done(COMMAND, document, messages))
        }
        Err(error) => {
            let reason = queue_refusal(error)?;
            Ok(ProposalOutcome::refused(
                COMMAND, document, &reason, messages,
            ))
        }
    }
}

/// Why `spec reject` refuses the proposal: a commit on its branch carries
/// its `Proposal:` trailer (the one that completes it, else the newest,
/// with why it does not), or git cannot tell. Read in its [`History`]
/// ([`trailer_lookup`]); an `orphan`'s (its recorded repository gone or
/// moved) in the current repository ([`trailer_lookup_here`]): none there
/// when neither the branch nor the base commit is in it (another
/// repository); the branch alone missing refuses, naming the way out.
fn committed(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    orphan: bool,
) -> Option<String> {
    let id = &proposal.id;
    let branch = &proposal.place.branch;
    let found = if orphan {
        match trailer_lookup_here(context, proposal) {
            Ok(found) => found,
            Err(error) if error.missing == Some(Missing::Both) => return None,
            Err(error) => {
                return Some(format!(
                    "cannot tell whether `{id}` has its commit in this repository: {error}"
                ));
            }
        }
    } else {
        match trailer_lookup(git_env, context, proposal) {
            Ok(found) => found,
            Err(error) => {
                return Some(format!(
                    "cannot tell whether `{id}` has its commit in history: {error}"
                ));
            }
        }
    };
    let mut reason = if let Some(own) = completing(&found) {
        format!(
            "`{id}` has its commit {} on `{branch}`: a proposal whose commit is in history is \
             never rejected; `spec approve {id}` completes it",
            own.commit
        )
    } else {
        let newest = found.first()?;
        let why = newest.not_completing.as_deref().unwrap_or_default();
        let more = match found.len() {
            1 => String::new(),
            count => format!(" ({} older one(s) too)", count - 1),
        };
        format!(
            "`{id}` has the commit {} on `{branch}` with the trailer `{PROPOSAL_TRAILER}: \
             {id}`{more}, which does not complete it ({why}): a proposal with a commit in \
             history is never rejected; {}",
            newest.commit,
            completes_when(proposal)
        )
    };
    if orphan {
        reason.push_str(&format!(
            "; it was recorded in the repository {old}, now gone or moved: move the repository \
             back to {old} (`git worktree repair`), then `spec approve {id}`",
            old = proposal.place.git_common_dir
        ));
    }
    Some(reason)
}
