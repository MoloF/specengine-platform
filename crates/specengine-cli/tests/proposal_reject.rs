//! docs/features/proposal-apply.md, `spec reject` as iteration 3 rules it
//! (the owner's session decision): an `open` or `approved` proposal is
//! rejected with its reason as a compare-and-set on the state read — the
//! prompt names an approved one `reject approved PR-0001 (<target> in
//! <path> on <branch> in <worktree>)? [y/N]`; one whose own commit is on
//! its branch (checked before the prompt and again after it) is refused
//! naming it, nothing written, no event: `spec approve` completes it.
//!
//! Through the library with a consent callback; the current directory the
//! main worktree, the proposal raised in the linked worktree on `t1`
//! (`common::proposal`).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::proposal::{DECIDER, LATER, Pair, refused};
use common::replace;
use specengine_cli::{CliError, Exit, Globals, ProposalOutcome, RejectRequest, reject};
use specengine_store::{ApplyFailure, Decision, ProposalQueue as _};

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";

fn ev(kind: &str, step: Option<u64>) -> (String, Option<u64>) {
    (format!("proposal.{kind}"), step)
}

/// Library `reject` of `id` in the main worktree at [`LATER`] with
/// `consent`.
fn reject_with(
    pair: &Pair,
    id: &str,
    reason: &str,
    consent: &mut dyn FnMut(&str) -> bool,
) -> Result<ProposalOutcome, CliError> {
    reject(
        &pair.env(&pair.main),
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: reason.to_owned(),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        consent,
    )
}

/// The proposal left `approved` by an apply whose pre-commit hook moved
/// `HEAD` to `side` (step 10): its commit on `side`, not on `t1`; `HEAD`
/// back on `t1`, the hook removed. The commit on `side`.
fn stuck_approved(pair: &Pair, id: &str) -> String {
    pair.git.git(&pair.linked, &["branch", "-f", "side", "t1"]);
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    fs::write(&hook, "#!/bin/sh\ngit symbolic-ref HEAD refs/heads/side\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let reason = refused(&pair.approve(&pair.main, id), "HEAD moved by the hook");
    assert!(reason.contains("(step 10)"), "{reason}");
    fs::remove_file(&hook).unwrap();
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
    assert_eq!(pair.proposal(id).status.as_str(), "approved");
    pair.rev(&pair.linked, "side")
}

/// The reason a commit on the branch gives for refusing a rejection.
fn has_its_commit(id: &str, commit: &str) -> String {
    format!(
        "`{id}` has its commit {commit} on `t1`: a proposal whose commit is in history is never \
         rejected; `spec approve {id}` completes it"
    )
}

/// An `approved` proposal whose commit is not on `t1` (it went to `side`):
/// the prompt names it approved with its place; consented, `rejected` with
/// the reason as the note, decided by the worktree's committer at now, one
/// `proposal.rejected` carrying the reason; approve then refuses it. M:
/// `approved` refused by `reject`.
#[test]
fn an_approved_proposal_without_its_commit_is_rejected_with_one_event() {
    let pair = Pair::new("pa-reject-approved", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    stuck_approved(&pair, &id);
    let worktree = pair.proposal(&id).place.worktree;
    let events = pair.events().len();

    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Not this way.", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "reject approved {id} ({TARGET} in {PATH} on t1 in {worktree})? [y/N]"
        )]
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "rejected");
    assert_eq!(stored.decision_note.as_deref(), Some("Not this way."));
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(stored.decided_at.as_deref(), Some(LATER));
    assert_eq!(stored.updated_at, LATER);
    assert!(stored.applied_commit.is_none());
    let added: Vec<_> = pair.events().into_iter().skip(events).collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(added[0].event_type, "proposal.rejected");
    assert_eq!(added[0].payload["reason"], "Not this way.");
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10)),
            ev("rejected", None)
        ]
    );
    let reason = refused(&pair.approve(&pair.main, &id), "approve of a rejected one");
    assert!(reason.contains("is rejected"), "{reason}");
}

/// A proposal whose own commit is on `t1` — `approved` with its commit
/// cherry-picked from `side`, and `open` with its commit made by hand —
/// is refused before any prompt naming the commit ("never rejected;
/// `spec approve` completes it"), nothing written, no event; `approve`
/// then completes each. M: reject allowed with a completing commit.
#[test]
fn a_proposal_with_its_commit_on_the_branch_is_never_rejected() {
    let pair = Pair::new("pa-reject-committed", "spec-a");
    let approved = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let side = stuck_approved(&pair, &approved);
    pair.git.git(&pair.linked, &["cherry-pick", &side]);
    let picked = pair.rev(&pair.linked, "t1");

    let open = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    replace(
        &pair.linked,
        PATH,
        "is not a reference",
        "is never a reference",
    );
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            "Never a reference.",
            "-m",
            &format!("Proposal: {open}"),
            "--",
            PATH,
        ],
    );
    let by_hand = pair.rev(&pair.linked, "t1");

    for (id, commit) in [(&approved, &picked), (&open, &by_hand)] {
        let before = pair.state();
        let events = pair.events().len();
        let (outcome, questions) = pair.reject_answer(&pair.main, id, "Too late.", true);
        let reason = refused(&outcome, id);
        assert_eq!(reason, has_its_commit(id, commit), "{id}");
        assert!(questions.is_empty(), "{id}: no prompt");
        assert_eq!(pair.state(), before, "{id}: nothing written");
        assert_eq!(pair.events().len(), events, "{id}: no event");
    }

    let outcome = pair.approve_ok(&pair.main, &approved);
    assert!(outcome.refusal.is_none());
    assert_eq!(
        pair.proposal(&approved).applied_commit.as_deref(),
        Some(picked.as_str())
    );
    pair.approve_ok(&pair.main, &open);
    assert_eq!(
        pair.proposal(&open).applied_commit.as_deref(),
        Some(by_hand.as_str())
    );
}

/// The proposal's commit lands on `t1` while the owner is asked (the
/// consent callback cherry-picks it, then answers yes): refused after the
/// prompt naming the commit, nothing written, no event, still `approved`.
/// M: reject allowed with a completing commit (the second check dropped).
#[test]
fn a_commit_landing_during_the_reject_prompt_refuses_the_rejection() {
    let pair = Pair::new("pa-reject-landing", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let side = stuck_approved(&pair, &id);
    let before = pair.proposal(&id);
    let events = pair.events().len();
    let mut asked = 0;
    let mut consent = |_: &str| {
        asked += 1;
        pair.git.git(&pair.linked, &["cherry-pick", &side]);
        true
    };
    let outcome = reject_with(&pair, &id, "Dropped.", &mut consent);
    assert_eq!(asked, 1);
    let picked = pair.rev(&pair.linked, "t1");
    let reason = refused(&outcome, "landed during the prompt");
    assert_eq!(reason, has_its_commit(&id, &picked));
    assert_eq!(pair.proposal(&id), before, "approved, untouched");
    assert_eq!(pair.events().len(), events, "no event");
}

/// `reject` is a compare-and-set on the state it read: an `open`
/// proposal approved and reopened by another run while the owner is
/// asked (the same status, a new `updated_at`), and an `approved` one
/// re-dated meanwhile, are refused exit 1 naming the stored state and time,
/// nothing written by the rejection (no `proposal.rejected`). M:
/// `reject_from` without its compare-and-set.
#[test]
fn reject_refuses_a_state_changed_during_the_prompt() {
    let pair = Pair::new("pa-reject-cas", "spec-a");
    let open = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let reopened_at = "2026-10-06T07:00:00Z";
    let mut consent = |_: &str| {
        let mut queue = pair.queue();
        let decision = Decision {
            decided_by: "Another Run <other@example.org>".to_owned(),
            note: None,
        };
        queue
            .approve(&open, &decision, "2026-10-06T06:00:00Z")
            .unwrap();
        let failure = ApplyFailure {
            step: 9,
            reason: "another run's hook".to_owned(),
        };
        queue.reopen(&open, &failure, reopened_at).unwrap();
        true
    };
    let outcome = reject_with(&pair, &open, "Dropped.", &mut consent);
    let reason = refused(&outcome, "open, approved and reopened meanwhile");
    assert!(
        reason.contains(&open) && reason.contains(reopened_at),
        "{reason}"
    );
    let stored = pair.proposal(&open);
    assert_eq!(stored.status.as_str(), "open");
    assert_eq!(stored.updated_at, reopened_at);
    assert!(stored.decision_note.is_none(), "{stored:?}");
    assert_eq!(
        pair.events_of(&open),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(9))
        ]
    );

    let approved = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    stuck_approved(&pair, &approved);
    let redated = "2026-10-06T07:30:00Z";
    let events = pair.events().len();
    let mut consent = |_: &str| {
        pair.sql(&format!(
            "update proposals set updated_at = '{redated}' where id = '{approved}'"
        ));
        true
    };
    let outcome = reject_with(&pair, &approved, "Dropped.", &mut consent);
    let reason = refused(&outcome, "approved, re-dated meanwhile");
    assert!(
        reason.contains(&approved) && reason.contains(redated),
        "{reason}"
    );
    let stored = pair.proposal(&approved);
    assert_eq!(stored.status.as_str(), "approved");
    assert_eq!(stored.updated_at, redated);
    assert_eq!(pair.events().len(), events, "no event");
}
