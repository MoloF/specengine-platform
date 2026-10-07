//! `spec approve PR [--note T]` and `spec reject PR [--reason T]` (task spec
//! `proposal-apply`, "Apply", "Idempotence", "Reject"): the one write door.
//! A staged choice (canon `decision-staging`, "Terminal"; [`Staging`]) is
//! taken by `spec approve PR` without flags (an approve) and `spec reject
//! PR` without `--reason` (a reject), shown before the question, which is
//! marked `staged`, and compared with the row byte for byte at step 7, a
//! completion and a reject; typed flags win whole, the stage named unused.
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
//!
//! A task-bound `update` or section-form `create` recorded at step 10 or by
//! a completion, in its task's compared place, refreshes the task's
//! snapshot in the same transaction (canon `tasks`, "Task-bound
//! proposals"): its target's entry and each snapshot node of its file
//! enclosing or inside it whose pre-apply hash is its snapshot hash take
//! the applied text and hash ([`task_refresh`]).
//!
//! A question or a discrepancy (canon `decision-record`, "Flags", "Steps")
//! is approved into a decision record, one new file and one commit where it
//! was raised ([`crate::decide`]); `--option N`, `--answer T`, `--canon REF`
//! are its flags only (on an update or a create: exit 2). A create's new
//! file (task spec `proposal-kinds`) is one new file and one commit too
//! ([`crate::create`]); its new sections apply as an update. Reject settles
//! a question or a discrepancy with the answer as
//! its reason, refused as an update's when a commit of it is in history
//! (its record's: none completes one with no record issued); holding no
//! record, a history git cannot read refuses nothing (a note, before the
//! prompt too). `decided_by` the current repository's committer unless it
//! holds a record issued at an earlier step 7 (then as an update's).

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use specengine_core::proposal::{CommitFacts, PROPOSAL_TRAILER, commit_message};
use specengine_model::IdScheme;
use specengine_store::{
    ApplyFailure, Decision, GitEnv, IndexWriter as _, Proposal, ProposalKind, ProposalQueue as _,
    ProposalStatus, QueueError, RefreshEntry, Seen, Source as _, TaskRefresh, WorkingTree,
    WorktreeGit, replace_file, same_repository, span_hash,
};

use crate::location::open_index;
use crate::package::position;
use crate::preflight::{
    History, LookupError, Missing, Prepared, StepFailure, TrailerCommit, completes_when,
    completing, history, place_unchanged, prepare_spanned, recorded_project, recorded_root,
    trailer_lookup, trailer_lookup_here,
};
use crate::proposals::{
    Find, Preview, ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, checked_now,
    escaped_error, find, no_proposal, open_context, queue_cannot, queue_refusal, top_path,
    with_diff, written_id,
};
use crate::review::stale_note;
use crate::stage::{self, Staging};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};
use crate::{create, decide};

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

/// `spec approve`'s flags of a question or a discrepancy (canon
/// `decision-record`, "Flags"); all `None` for an update or a create.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApproveFlags {
    /// `--option N`: a discrepancy's chosen option, from 0.
    pub option: Option<u64>,
    /// `--answer T`: a question's answer other than its working answer.
    pub answer: Option<String>,
    /// `--canon REF`: the section the record governs (`ID`, `ID#SECTION`,
    /// `path#anchor`).
    pub canon: Option<String>,
}

impl ApproveFlags {
    /// Any flag given.
    pub fn any(&self) -> bool {
        self.option.is_some() || self.answer.is_some() || self.canon.is_some()
    }
}

/// `spec reject` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectRequest {
    /// `PR` as given.
    pub id: String,
    /// `--reason T`: non-empty, at most 4 096 bytes; left out only when a
    /// reject is staged, whose reason it takes (canon `decision-staging`,
    /// "Terminal").
    pub reason: Option<String>,
    pub now: String,
    pub git: GitEnv,
}

/// The owner's answer to one question (`main`: a `[y/N]` prompt on
/// stderr, the answer read from the terminal; only `y` or `yes`
/// consents).
pub type Consent<'a> = &'a mut dyn FnMut(&str) -> bool;

/// `spec approve`: applies an open proposal (steps 2–10), or completes one
/// whose own commit is on its branch; no flag ([`approve_with`]).
pub fn approve(
    env: &Env,
    globals: &Globals,
    request: &ApproveRequest,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    approve_with(env, globals, request, &ApproveFlags::default(), consent)
}

/// `spec approve PR [--note T] [--option N | --answer T] [--canon REF]`:
/// [`approve`], a question or a discrepancy decided into its record by
/// `flags`.
pub fn approve_with(
    env: &Env,
    globals: &Globals,
    request: &ApproveRequest,
    flags: &ApproveFlags,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    run_approve(env, globals, request, flags, consent).map_err(escaped_error)
}

fn run_approve(
    env: &Env,
    globals: &Globals,
    request: &ApproveRequest,
    flags: &ApproveFlags,
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
            let mut reason = format!(
                "`{id}` is already applied: commit {}",
                proposal.applied_commit.as_deref().unwrap_or("unknown")
            );
            if let Some(record) = &proposal.record {
                reason.push_str(&format!(", its record `{}`", record.id));
            }
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
    // `--note` (a reject's `--reason` too) at most 4 096 bytes
    // (canon `decision-staging`, "The stage": as a stage's).
    if let Some(reason) = request
        .note
        .as_deref()
        .and_then(|note| stage::over_cap("--note", note))
    {
        return Ok(ProposalOutcome::refused(
            COMMAND,
            document(&proposal, &context.data_dir),
            &reason,
            messages,
        ));
    }
    // The staged choice: taken when no flag is typed, else left unused,
    // named (canon `decision-staging`, "Terminal").
    let (request, flags, staging) = Staging::for_approve(&proposal, request, flags);
    messages.extend(staging.notes());
    let (request, flags) = (&request, &flags);
    if proposal.kind.decides() {
        return decide::approve_record(
            env,
            request,
            flags,
            &mut context,
            &proposal,
            &staging,
            consent,
        );
    }
    if let Some(reason) = flag_on_apply(&proposal, flags) {
        return Err(CliError::spec(reason));
    }
    if proposal.new_file() {
        return create::approve_file(env, request, &mut context, &proposal, &staging, consent);
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
                    &staging,
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
    let mut span = None;
    let prepared = match prepare_spanned(
        env,
        &request.git,
        &context,
        &proposal,
        &mut messages,
        &mut span,
    ) {
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
                    &staging,
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
                Some(preview),
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
                Some(prepared.preview),
                messages,
            );
        }
    };
    // A staged approve's staleness: a note at the prompt, never a refusal.
    let stale = staging
        .shown()
        .and_then(|_| stale_note(&proposal, span.as_deref()));
    if let Some(note) = &stale {
        messages.push(Message::Note(note.clone()));
    }
    let question = format!(
        "{}apply {id} to {} on {} in {} ({}{})? [y/N]",
        staging.preface(stale.as_deref()),
        prepared.top_path,
        proposal.place.branch,
        proposal.place.worktree,
        prepared.preview.as_str(),
        staging.mark()
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

    // Step 7: a compare-and-set on the state read, the stage shown too.
    let decision = Decision {
        decided_by: decided_by.clone(),
        note: request.note.clone(),
        staged_at: staging.staged_at(),
    };
    let held = match context.queue.approve_from(&id, &read, &decision, now) {
        Ok(approved) => approved.seen(),
        Err(
            error @ (QueueError::Changed { .. }
            | QueueError::Status { .. }
            | QueueError::Invalid(_)),
        ) => {
            let replaced = match error {
                QueueError::Changed { .. } => staging.replaced(&context, &id),
                _ => None,
            };
            let reason = replaced.unwrap_or_else(|| format!("{error}; nothing written"));
            let failure = StepFailure::refused(7, reason);
            return failed(
                &mut context,
                request,
                &proposal,
                None,
                failure,
                Some(prepared.preview),
                messages,
            );
        }
        Err(error) => return Err(queue_cannot(error)),
    };

    // Step 8: the place and the file as checked, then the write.
    let path = proposal.target_path.as_str();
    let file = prepared.tree.root().join(path);
    let unchanged = place_unchanged(&prepared.git, &prepared.head, &proposal.place).and_then(
        |()| match prepared.tree.read(path) {
            Ok(bytes) if bytes == prepared.bytes => Ok(()),
            Ok(_) => Err(StepFailure::refused(
                8,
                format!("`{path}` changed while being applied; nothing written"),
            )),
            Err(error) => Err(StepFailure::refused(
                8,
                format!("cannot read `{path}` again: {error}; nothing written"),
            )),
        },
    );
    if let Err(failure) = unchanged {
        return failed(
            &mut context,
            request,
            &proposal,
            Some(&held),
            failure,
            Some(prepared.preview),
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
            Some(prepared.preview),
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
        match made_anyway(&prepared.git, &prepared.head, &proposal) {
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
                    Some(prepared.preview),
                    messages,
                );
            }
        }
    }

    // Step 10.
    // This run's own commit verified: `applied`, even when its hold was
    // reopened meanwhile.
    let changes_only_the_path =
        |commit: &str| match prepared.git.changed_paths(&prepared.head, commit) {
            Ok(changed) if changed == [prepared.top_path.as_str()] => Ok(()),
            Ok(changed) => Err(format!("it changes {}", changed.join(", "))),
            Err(error) => Err(error.to_string()),
        };
    match verify(
        &prepared.git,
        &prepared.head,
        &proposal,
        &changes_only_the_path,
    ) {
        Ok(commit) => {
            let refresh = task_refresh(
                &context,
                &proposal,
                &prepared.recorded.config.scheme,
                &prepared.bytes,
                &prepared.patched,
            );
            let recorded = record_applied(
                &mut context,
                &id,
                &commit,
                Some(&decision),
                refresh.as_ref(),
                now,
            )
            .map(|(applied, refreshed)| {
                refreshed_note(&proposal, &refreshed, &mut messages);
                applied
            });
            let applied = match recorded {
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
                Some(preview),
                messages,
            )
        }
    }
}

/// Why `flags` do not go with an `update` or a `create` (a decision flag
/// decides a question or a discrepancy): exit 2's reason; `None` when none
/// is given.
pub(crate) fn flag_on_apply(proposal: &Proposal, flags: &ApproveFlags) -> Option<String> {
    if !flags.any() {
        return None;
    }
    let id = &proposal.id;
    let flag = match (flags.option, &flags.answer) {
        (Some(_), _) => "--option",
        (None, Some(_)) => "--answer",
        (None, None) => "--canon",
    };
    let article = if proposal.kind == ProposalKind::Update {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "`{flag}` decides a question or a discrepancy; `{id}` is {article} {}: `spec approve {id}` \
         applies it as proposed; nothing changed",
        proposal.kind.as_str()
    ))
}

/// Step 10: the branch's new commit, when its one parent is the old `HEAD`
/// (`head`), `content` passes it (it changes only the path; a record: it
/// adds only the record's path with its text) and its `Proposal:` trailer
/// is the ID; else why not, naming the commit.
pub(crate) fn verify(
    git: &WorktreeGit,
    head: &str,
    proposal: &Proposal,
    content: &dyn Fn(&str) -> Result<(), String>,
) -> Result<String, String> {
    let old_head = head;
    let branch = &proposal.place.branch;
    let commit = match git.commit_of(&format!("refs/heads/{branch}")) {
        Ok(Some(commit)) => commit,
        Ok(None) => return Err(format!("`{branch}` names no commit after the commit")),
        Err(error) => return Err(format!("cannot read `{branch}` after the commit: {error}")),
    };
    if commit == old_head {
        let head = match git.head() {
            Ok(Some(head)) if head != old_head => head,
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
        Ok(parents) if parents == [old_head] => {}
        Ok(parents) => return Err(stays(format!("its parents are {}", parents.join(" ")))),
        Err(error) => return Err(stays(error.to_string())),
    }
    content(&commit).map_err(&stays)?;
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
/// 2 as an error, exit 1 as the refused document (an update's with its
/// `preview`). A proposal applied or rejected meanwhile logs nothing; its
/// document is the stored one.
pub(crate) fn failed(
    context: &mut QueueContext,
    request: &ApproveRequest,
    proposal: &Proposal,
    held: Option<&Seen>,
    failure: StepFailure,
    preview: Option<Preview>,
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
    ) && preview.is_some()
    {
        document.preview = preview;
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
pub(crate) enum Made {
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
/// the proposal's `Proposal:` trailer (`head`: the branch's commit step 2
/// found).
pub(crate) fn made_anyway(git: &WorktreeGit, head: &str, proposal: &Proposal) -> Made {
    let own = |tip: &str| {
        git.trailer_values(tip, PROPOSAL_TRAILER)
            .is_ok_and(|values| values.contains(&proposal.id))
    };
    let branch_tip = match git.commit_of(&format!("refs/heads/{}", proposal.place.branch)) {
        Ok(Some(tip)) if tip != head => Some(tip),
        _ => None,
    };
    if let Some(tip) = branch_tip.as_deref().filter(|tip| own(tip)) {
        return Made::Commit {
            tip: tip.to_owned(),
            on_branch: true,
        };
    }
    if let Ok(Some(tip)) = git.head()
        && tip != head
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
pub(crate) fn noted(unknown: Option<&str>, question: &str) -> String {
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
/// queue and the data directory's index is written. A staged choice it
/// confirms (`staging`, canon `decision-staging`, "Terminal") is
/// shown before the question, which ends ` (staged)`; it is compared with
/// the row again before the recording (`applied_with` compares nothing):
/// changed → exit 1, nothing changed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complete(
    env: &Env,
    request: &ApproveRequest,
    context: &mut QueueContext,
    proposal: &Proposal,
    commit: &str,
    staging: &Staging,
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
                (!proposal.kind.decides()).then_some(Preview::Unavailable),
                messages,
            );
        };
        let marked = if staging.shown().is_some() {
            " (staged)"
        } else {
            ""
        };
        let question = format!(
            "{}complete {id} by its commit {commit} on {} in {read}{marked}? [y/N]",
            staging.preface(None),
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
        // The stage read (shown, unused or none), compared again as step 7
        // compares it: the recording compares nothing.
        if let Some(reason) = stage::changed_since(context, proposal) {
            let document = with_diff(proposal, &request.git, &context.data_dir);
            return Ok(ProposalOutcome::refused(
                COMMAND, document, &reason, messages,
            ));
        }
        let decision = Decision {
            decided_by,
            note: request.note.clone(),
            staged_at: staging.staged_at(),
        };
        let refresh = completion_refresh(&request.git, context, proposal, commit);
        record_applied(
            context,
            id,
            commit,
            Some(&decision),
            refresh.as_ref(),
            &request.now,
        )
        .map(|(applied, refreshed)| {
            refreshed_note(proposal, &refreshed, &mut messages);
            applied
        })
    } else {
        let refresh = completion_refresh(&request.git, context, proposal, commit);
        record_applied(context, id, commit, None, refresh.as_ref(), &request.now).map(
            |(applied, refreshed)| {
                refreshed_note(proposal, &refreshed, &mut messages);
                applied
            },
        )
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

/// Step 10's and a completion's recording: `applied_with` (`decision`
/// given) or `applied`, refreshing the task's snapshot in the same
/// transaction when `refresh` is given; the nodes refreshed.
fn record_applied(
    context: &mut QueueContext,
    id: &str,
    commit: &str,
    decision: Option<&Decision>,
    refresh: Option<&TaskRefresh>,
    now: &str,
) -> Result<(Proposal, Vec<String>), QueueError> {
    match (refresh, decision) {
        (Some(refresh), decision) => context
            .queue
            .applied_refreshing(id, commit, decision, refresh, now),
        (None, Some(decision)) => context
            .queue
            .applied_with(id, commit, decision, now)
            .map(|applied| (applied, Vec::new())),
        (None, None) => context
            .queue
            .applied(id, commit, now)
            .map(|applied| (applied, Vec::new())),
    }
}

/// `<task>: its snapshot of <nodes> refreshed by <PR>` when any was.
fn refreshed_note(proposal: &Proposal, refreshed: &[String], messages: &mut Vec<Message>) {
    if let (Some(task), false) = (&proposal.task_id, refreshed.is_empty()) {
        messages.push(Message::Note(format!(
            "{task}: its snapshot of {} refreshed by `{}`",
            refreshed.join(", "),
            proposal.id
        )));
    }
}

/// What a task-bound `update` or section-form `create` applied as
/// `before` → `after` (its file's bytes, parsed under `scheme`) refreshes
/// in its task's snapshot: only in the task's compared place (the claim's
/// worktree and branch, else the snapshot's, its root there), the target's
/// entry and each snapshot node of the file enclosing or inside the
/// target whose pre-apply hash is its snapshot hash, with its applied text
/// and hash (a node the apply left byte for byte takes nothing). `None`
/// when nothing is refreshed (no task, no snapshot, another place, a parse
/// that fails).
pub(crate) fn task_refresh(
    context: &QueueContext,
    proposal: &Proposal,
    scheme: &IdScheme,
    before: &[u8],
    after: &[u8],
) -> Option<TaskRefresh> {
    if proposal.new_file() || !proposal.kind.applies() {
        return None;
    }
    let task = context
        .queue
        .get_task(proposal.task_id.as_deref()?)
        .ok()??;
    // A done or cancelled task's snapshot stays as it was.
    if task.status.is_closed() {
        return None;
    }
    let snapshot = task.snapshot.as_ref()?;
    let (worktree, branch) = match &task.claim {
        Some(claim) => (claim.worktree.as_str(), claim.branch.as_str()),
        None => (
            snapshot.place.worktree.as_str(),
            snapshot.place.branch.as_str(),
        ),
    };
    let place = &proposal.place;
    if place.worktree != worktree
        || place.branch != branch
        || place.root_rel != snapshot.place.root_rel
    {
        return None;
    }
    let path = proposal.target_path.as_str();
    let parse = |bytes: &[u8]| {
        panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, bytes, scheme)
        }))
        .ok()
    };
    let (parsed_before, parsed_after) = (parse(before)?, parse(after)?);
    let target = parsed_before.nodes[position(&parsed_before, &proposal.target_id)?].span;
    let mut entries = Vec::new();
    for node in snapshot.nodes.iter().filter(|node| node.path == path) {
        let Some(at) = position(&parsed_before, &node.id) else {
            continue;
        };
        let was = &parsed_before.nodes[at];
        let span = was.span;
        let encloses = span.start <= target.start && target.end <= span.end;
        let inside = target.start <= span.start && span.end <= target.end;
        if !(encloses || inside) || span_hash(before, was) != node.span_hash {
            continue;
        }
        let Some(now) = position(&parsed_after, &node.id).map(|at| &parsed_after.nodes[at]) else {
            continue;
        };
        let Ok(text) = String::from_utf8(specengine_core::patch::span_bytes(after, now).to_vec())
        else {
            continue;
        };
        // A node whose bytes the apply left as they were takes nothing.
        let hash = span_hash(after, now);
        if hash == node.span_hash {
            continue;
        }
        entries.push(RefreshEntry {
            id: node.id.clone(),
            was: node.span_hash.clone(),
            span_hash: hash,
            text,
        });
    }
    (!entries.is_empty()).then(|| TaskRefresh {
        task_id: task.id.clone(),
        entries,
    })
}

/// [`task_refresh`] of a completion: `before` the file at the completing
/// commit's first parent, `after` at the commit, read where its history is
/// ([`history`]), parsed under the recorded root's scheme; `None` when any
/// of it cannot be read.
fn completion_refresh(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    commit: &str,
) -> Option<TaskRefresh> {
    proposal.task_id.as_ref()?;
    let git = match history(git_env, context, proposal).ok()? {
        History::Recorded(git) => git,
        History::Current(git) => git.clone(),
    };
    let path = top_path(proposal);
    let parent = git.parents(commit).ok()?.into_iter().next()?;
    let before = git.blob_at(&parent, &path).ok()?;
    let after = git.blob_at(commit, &path).ok()?;
    let recorded = recorded_project(&recorded_root(proposal), &context.slug).ok()?;
    task_refresh(context, proposal, &recorded.config.scheme, &before, &after)
}

/// `applied` with `commit` refused because another run (a completion)
/// recorded the same commit meanwhile: the stored proposal, with a note.
pub(crate) fn recorded_already(
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

/// The recorded root's index brought up to date for the applied path (a
/// decision record's path) after a completion; a failure is a warning.
fn reindex(env: &Env, context: &QueueContext, proposal: &Proposal, messages: &mut Vec<Message>) {
    let path = proposal
        .record
        .as_ref()
        .map_or(proposal.target_path.as_str(), |record| record.path.as_str());
    let updated = recorded_project(&recorded_root(proposal), &context.slug).and_then(|recorded| {
        let mut open = open_index(env, &recorded).map_err(|error| error.message)?;
        let tree = WorkingTree::new(&recorded.root, &recorded.config.paths)
            .map_err(|error| error.to_string())?;
        open.index
            .update_paths(&tree, &recorded.config.scheme, &[path])
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
/// current repository. A question or a discrepancy holding no record is
/// refused only by such a commit found: its history unread, a note. Decided
/// by the committer of its [`History`], else of the current repository.
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
    if request
        .reason
        .as_deref()
        .is_some_and(|reason| reason.trim().is_empty())
    {
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
    if let Some(reason) = request
        .reason
        .as_deref()
        .and_then(|reason| stage::over_cap("--reason", reason))
    {
        return Ok(ProposalOutcome::refused(
            COMMAND, document, &reason, messages,
        ));
    }
    // The reason typed, else a staged reject's (canon `decision-staging`,
    // "Terminal").
    let (reason, staging) = Staging::for_reject(&proposal, request.reason.as_deref())?;
    messages.extend(staging.notes());
    // A proposal with a commit in history is never recorded as rejected;
    // nor one whose history git cannot read, when it may have a commit: an
    // update, or a question or a discrepancy holding a record issued at an
    // earlier step 7. One holding none is refused only by a commit with its
    // trailer found (a queue restored from before step 7 while its
    // record's commit is there: neither approve nor reject takes it); its
    // history unread (its branch deleted after a merge), it is rejected,
    // noted.
    let applies = proposal.kind.applies() || proposal.record.is_some();
    let mut unknown = None;
    let refusal = if applies {
        committed(&request.git, &context, &proposal, orphan)
    } else if proposal.kind.decides() {
        match trailer_found(&request.git, &context, &proposal, orphan) {
            Ok(found) => carried(&proposal, orphan, &found),
            Err(error) => {
                let why = match error.missing {
                    Some(_) => format!("the branch `{}` no longer exists", proposal.place.branch),
                    None => error.reason,
                };
                let note = one_line(&format!(
                    "`{id}` holds no record, and its history cannot be read ({why}): rejected \
                     without looking for a commit of it"
                ));
                messages.push(Message::Note(note.clone()));
                unknown = Some(note);
                None
            }
        }
    } else {
        None
    };
    if let Some(reason) = refusal {
        return Ok(ProposalOutcome::refused(
            COMMAND, document, &reason, messages,
        ));
    }
    // The committer of the proposal's history ([`History`]), else of the
    // current repository.
    let decided_by = applies
        .then(|| history(&request.git, &context, &proposal).ok())
        .flatten()
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
        "{}reject {approved}{id} ({} in {} on {} in {}{})? [y/N]",
        staging.preface(None),
        proposal.target_id,
        top_path(&proposal),
        proposal.place.branch,
        proposal.place.worktree,
        staging.mark()
    );
    if !consent(&noted(unknown.as_deref(), &escape_controls(&question))) {
        return Ok(ProposalOutcome::refused(
            COMMAND,
            document,
            &format!("`{id}` not rejected: the answer was not `y`; nothing changed"),
            messages,
        ));
    }
    // Its commit may have landed while the owner was asked.
    if applies && let Some(reason) = committed(&request.git, &context, &proposal, orphan) {
        return Ok(ProposalOutcome::refused(
            COMMAND, document, &reason, messages,
        ));
    }
    let decision = Decision {
        decided_by,
        note: Some(reason),
        staged_at: staging.staged_at(),
    };
    // A compare-and-set on the state read (its stage too): another run's
    // change since refuses (exit 1).
    let rejected = context
        .queue
        .reject_from(&id, &proposal.seen(), &decision, now);
    match rejected {
        Ok(rejected) => {
            let document = with_diff(&rejected, &request.git, &context.data_dir);
            Ok(ProposalOutcome::done(COMMAND, document, messages))
        }
        Err(error) => {
            let replaced = match error {
                QueueError::Changed { .. } => staging.replaced(&context, &id),
                _ => None,
            };
            let reason = match replaced {
                Some(reason) => reason,
                None => queue_refusal(error)?,
            };
            Ok(ProposalOutcome::refused(
                COMMAND, document, &reason, messages,
            ))
        }
    }
}

/// Why `spec reject` refuses the proposal: a commit on its branch carries
/// its `Proposal:` trailer ([`carried`]), or git cannot tell
/// ([`trailer_found`]).
fn committed(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    orphan: bool,
) -> Option<String> {
    let id = &proposal.id;
    match trailer_found(git_env, context, proposal, orphan) {
        Ok(found) => carried(proposal, orphan, &found),
        Err(error) if orphan => Some(format!(
            "cannot tell whether `{id}` has its commit in this repository: {error}"
        )),
        Err(error) => Some(format!(
            "cannot tell whether `{id}` has its commit in history: {error}"
        )),
    }
}

/// The commits on the proposal's branch carrying its `Proposal:` trailer.
/// Read in its [`History`] ([`trailer_lookup`]); an `orphan`'s (its
/// recorded repository gone or moved) in the current repository
/// ([`trailer_lookup_here`]): none there when neither the branch nor the
/// base commit is in it (another repository); the branch alone missing is
/// an error, naming the way out.
fn trailer_found(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    orphan: bool,
) -> Result<Vec<TrailerCommit>, LookupError> {
    if orphan {
        match trailer_lookup_here(context, proposal) {
            Err(error) if error.missing == Some(Missing::Both) => Ok(Vec::new()),
            found => found,
        }
    } else {
        trailer_lookup(git_env, context, proposal)
    }
}

/// Why `spec reject` refuses the proposal whose trailer commits are
/// `found`: the one that completes it, else the newest, with why it does
/// not; `None` when there is none.
fn carried(proposal: &Proposal, orphan: bool, found: &[TrailerCommit]) -> Option<String> {
    let id = &proposal.id;
    let branch = &proposal.place.branch;
    let mut reason = if let Some(own) = completing(found) {
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
