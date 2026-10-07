//! docs/features/proposal-apply.md, "Idempotence" and "Reject" as
//! iteration 4 rules them (review of iteration 3, MAJOR: once the recorded
//! worktree was removed, a proposal's own commit on its branch was never
//! found again): the trailer lookup (the completion, `review`'s note,
//! `reject`'s guard) reads the recorded worktree when it is a directory of
//! the recorded repository, else the current repository, whose branches are
//! the same refs; a lookup git cannot make is named (`review`: a note;
//! `approve`: a note, and appended to the reason of an exit 2 at steps
//! 2–6; `reject`: refused exit 1, nothing written, no event).
//!
//! - an `approved` proposal left by step 10 with its own commit on `t1`
//!   (a post-commit hook committed on top), the linked worktree then
//!   removed (the branch kept): `review` notes the commit, `approve`
//!   completes it with no prompt (a warning: the index of a gone worktree
//!   is not updated), nothing written in the main worktree; `reject` is
//!   refused naming the commit;
//! - an `open` proposal with its commit made by hand, the worktree
//!   removed: the completion asks and records the current repository's
//!   committer identity; none there: exit 2 at step 7 naming both places;
//! - the branch merged into `main` and deleted with its worktree, or with
//!   the base commit missing too: the lookup's failure is named in all
//!   three commands with the way out; the base commit alone missing: the
//!   branch's whole history is read, nothing is "cannot tell" (iteration
//!   6); a lookup git cannot make is shown before the consent question;
//! - the content check on both fixtures (two schemes, English and Russian
//!   prose) and with the project root below the worktree's top (the
//!   commit's `<root_rel>/specengine.toml`).
//!
//! Through the library with a consent callback; the current directory the
//! main worktree, the proposal raised in the linked worktree on `t1`
//! (`common::proposal`).

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::proposal::{
    CASES, DECIDER, LATER, NOW, Pair, State, cannot, edit, printed, refused, with_vars,
};
use common::{copy_dir, replace, write};
use serde_json::Value;
use specengine_cli::{
    ApproveRequest, Env, Exit, Globals, Message, Preview, ProposalOutcome, ProposeRequest,
    ProposedText, ShowRequest, approve, propose, show,
};
use specengine_store::GitEnv;

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";

fn ev(kind: &str, step: Option<u64>) -> (String, Option<u64>) {
    (format!("proposal.{kind}"), step)
}

fn hook(pair: &Pair, name: &str, body: &str) {
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let path = hooks.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn remove_hook(pair: &Pair, name: &str) {
    fs::remove_file(pair.main.join(".git/hooks").join(name)).unwrap();
}

/// The proposal's edit committed by hand on `t1` with its `Proposal:`
/// trailer: the commit.
fn commit_by_hand(pair: &Pair, id: &str) -> String {
    replace(&pair.linked, PATH, FROM, TO);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            "The sprint ends at once.",
            "-m",
            &format!("Proposal: {id}"),
            "--",
            PATH,
        ],
    );
    pair.rev(&pair.linked, "t1")
}

/// `git worktree remove <linked>` from the main worktree (the branch
/// kept).
fn remove_worktree(pair: &Pair) {
    pair.git.git(
        &pair.main,
        &[
            "worktree",
            "remove",
            pair.linked.to_str().expect("a UTF-8 scratch path"),
        ],
    );
    assert!(!pair.linked.exists());
}

/// The reviewer's t12: approved, a post-commit hook commits `gen.txt` on
/// top of the apply commit, so step 10 leaves the proposal `approved`
/// with its own commit one below the tip of `t1`. The hook removed. The
/// apply commit.
fn approved_with_its_commit_below_the_tip(pair: &Pair, id: &str) -> String {
    hook(
        pair,
        "post-commit",
        "#!/bin/sh\n[ -f gen.txt ] && exit 0\necho g > gen.txt\ngit add gen.txt\n\
         git commit -q -m generated\n",
    );
    let reason = refused(&pair.approve(&pair.main, id), "a commit on top (step 10)");
    assert!(reason.contains("(step 10)"), "{reason}");
    remove_hook(pair, "post-commit");
    assert_eq!(pair.proposal(id).status.as_str(), "approved");
    let commit = pair.rev(&pair.linked, "t1~1");
    assert_eq!(
        pair.git_text(
            &pair.linked,
            &[
                "log",
                "-1",
                "--format=%(trailers:key=Proposal,valueonly)",
                &commit
            ]
        ),
        id
    );
    assert_eq!(
        pair.git_text(&pair.linked, &["log", "-1", "--format=%s"]),
        "generated"
    );
    commit
}

/// What a completion must not touch: the main worktree, every ref.
fn main_side(state: &State) -> impl PartialEq + std::fmt::Debug {
    (
        state.main_files.clone(),
        state.refs.clone(),
        state.main_head.clone(),
        state.main_status.clone(),
        state.main_staged.clone(),
    )
}

fn notes_of(outcome: &ProposalOutcome) -> Vec<String> {
    outcome
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::Note(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn warnings_of(outcome: &ProposalOutcome) -> Vec<String> {
    outcome
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::Warning(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The reason a completing commit gives for refusing a rejection.
fn has_its_commit(id: &str, commit: &str) -> String {
    format!(
        "`{id}` has its commit {commit} on `t1`: a proposal whose commit is in history is never \
         rejected; `spec approve {id}` completes it"
    )
}

/// t12 (review of iteration 3, MAJOR): `approved` with its own commit on
/// `t1`, the linked worktree removed. `review` from the main worktree
/// notes "its commit <sha> is on `t1`: `spec approve PR-0001` completes
/// it" (text and JSON); `approve` completes it with no prompt — `applied`
/// with that commit, step 7's decision kept, events `apply_failed` (10)
/// then `applied`, a note "completed by its commit", a warning that the
/// index was not updated (its worktree is gone) — and writes nothing in
/// the main worktree or the refs. M: `history()` always the recorded
/// worktree.
#[test]
fn an_approved_proposal_completes_by_its_commit_after_its_worktree_is_removed() {
    let pair = Pair::new("pa-gone-approved", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = approved_with_its_commit_below_the_tip(&pair, &id);
    let held = pair.proposal(&id);
    remove_worktree(&pair);

    let review = pair.review_ok(&pair.main, &id);
    let note = format!(
        "its commit {commit} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert_eq!(
        review
            .document
            .notes
            .iter()
            .filter(|line| **line == note)
            .count(),
        1,
        "{:?}",
        review.document.notes
    );
    assert!(
        !review
            .document
            .notes
            .iter()
            .any(|line| line.contains("cannot tell")),
        "{:?}",
        review.document.notes
    );
    let (text, json) = printed(&review);
    assert!(text.contains(&note), "{text}");
    let json: Value = serde_json::from_str(&json).expect("JSON");
    assert!(
        json["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|line| line == &Value::String(note.clone())),
        "{json}"
    );

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "an approved completion asks nothing");
    assert!(
        notes_of(&outcome).contains(&format!(
            "`{id}` completed by its commit {commit} on `t1`; no new commit"
        )),
        "{:?}",
        outcome.messages
    );
    let warnings = warnings_of(&outcome);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("the index was not updated after the completion: ")
            && warnings[0].ends_with("; the next command updates it"),
        "{warnings:?}"
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(stored.decided_by, held.decided_by, "step 7's decision");
    assert_eq!(stored.decided_at, held.decided_at);
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10)),
            ev("applied", None)
        ]
    );
    assert_eq!(
        main_side(&pair.state()),
        main_side(&before),
        "nothing but the queue and the data directory written"
    );
}

/// t12r: the same `approved` proposal, its worktree removed: `reject` is
/// refused before any prompt naming the commit ("never rejected; `spec
/// approve` completes it"), nothing written, no event; `approve` then
/// completes it. M: `history()` always the recorded worktree (the
/// rejection refused "cannot tell"); before iteration 4 it went through.
#[test]
fn reject_refuses_an_approved_proposal_whose_commit_is_on_the_branch_of_a_removed_worktree() {
    let pair = Pair::new("pa-gone-reject", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = approved_with_its_commit_below_the_tip(&pair, &id);
    remove_worktree(&pair);

    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Too late.", true);
    let reason = refused(&outcome, "its commit on t1, the worktree removed");
    assert_eq!(reason, has_its_commit(&id, &commit));
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");

    pair.approve_ok(&pair.main, &id);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
}

/// An `open` proposal whose commit was made by hand on `t1`, the linked
/// worktree removed. No identity anywhere: `approve` exits 2 at step 7
/// before the prompt, the message naming the current worktree and the
/// recorded one ("no git identity in <main> (the worktree <t1> is not
/// there)"), one `apply_failed` (7), still `open`. With an identity in the
/// main worktree only (`git config --worktree`): asked `complete PR-0001
/// by its commit <sha> on t1 in <main>? [y/N]` (the root the commit was
/// looked up in, iteration 5); declined, nothing changes;
/// consented, `applied` with that commit decided by that identity at now,
/// events `approved` then `applied`. M: `history()` always the recorded
/// worktree.
#[test]
fn an_open_proposal_completes_with_the_current_repositorys_identity_after_its_worktree_is_removed()
{
    let pair = Pair::new("pa-gone-open", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let worktree = pair.proposal(&id).place.worktree;
    remove_worktree(&pair);

    pair.git
        .git(&pair.main, &["config", "user.useConfigOnly", "true"]);
    let anonymous = pair
        .git_env(&pair.main)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, anonymous.clone());
    let message = cannot(&outcome, "no identity anywhere");
    assert!(message.contains("(step 7)"), "{message}");
    assert!(
        message.contains(&format!(
            "no git identity in {} (the worktree {worktree} is not there); set user.name and \
             user.email",
            pair.main.display()
        )),
        "{message}"
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "still open, nothing written");
    assert_eq!(
        pair.events_of(&id),
        [ev("created", None), ev("apply_failed", Some(7))]
    );

    pair.git
        .git(&pair.main, &["config", "extensions.worktreeConfig", "true"]);
    pair.git.git(
        &pair.main,
        &["config", "--worktree", "user.name", "Main Owner"],
    );
    pair.git.git(
        &pair.main,
        &["config", "--worktree", "user.email", "main@example.org"],
    );
    // The commit was looked up in the current project's root (the
    // worktree is gone): the prompt names it, not the gone worktree.
    let question = format!(
        "complete {id} by its commit {commit} on t1 in {}? [y/N]",
        pair.main.display()
    );
    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, false, anonymous.clone());
    let reason = refused(&outcome, "declined completion");
    assert!(
        reason.contains(&format!(
            "`{id}` not completed: the answer was not `y`; nothing changed"
        )),
        "{reason}"
    );
    assert_eq!(questions, std::slice::from_ref(&question));
    assert_eq!(pair.state(), before, "nothing changed");
    assert_eq!(pair.events().len(), events, "nothing logged");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, anonymous);
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions, [question]);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(
        stored.decided_by.as_deref(),
        Some("Main Owner <main@example.org>"),
        "the current repository's identity"
    );
    assert_eq!(stored.decided_at.as_deref(), Some(LATER));
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("apply_failed", Some(7)),
            ev("approved", None),
            ev("applied", None)
        ]
    );
    assert_eq!(
        main_side(&pair.state()),
        main_side(&before),
        "no new commit, the main worktree untouched"
    );
}

/// tgone: an `open` proposal's commit made by hand on `t1`, `t1` merged
/// into `main` (fast-forward), its worktree removed and the branch
/// deleted. `review` notes "cannot tell whether its commit is on `t1`: the
/// branch `t1` no longer exists" (no completion note); `approve` exits 2
/// at step 2 (the worktree gone) with that text appended to the reason
/// ("…; cannot tell whether `PR-0001` has its commit on `t1`: the branch
/// `t1` no longer exists"), no prompt, one `apply_failed` (2) carrying
/// it, still `open`; `reject` is refused exit 1 "cannot tell whether
/// `PR-0001` has its commit in history: the branch `t1` no longer exists"
/// before any prompt, nothing written, no event (review of iteration 3: it
/// recorded `rejected` with the commit in `main`'s history). Each text
/// ends with the way out (review of iteration 4), in `reject` too
/// (iteration 6, accepted deviation: the hint is the same in all three
/// commands): "; recreate it at its last commit (`git branch t1
/// <commit>`), then `spec approve PR-0001` or `spec reject PR-0001`". M: a
/// failed lookup treated as "no commit" in reject.
#[test]
fn a_failed_lookup_is_noted_by_review_appended_by_approve_and_refuses_reject() {
    let pair = Pair::new("pa-gone-branch", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let worktree = pair.proposal(&id).place.worktree;
    pair.git
        .git(&pair.main, &["merge", "-q", "--ff-only", "t1"]);
    remove_worktree(&pair);
    pair.git.git(&pair.main, &["branch", "-q", "-d", "t1"]);
    assert_eq!(pair.rev(&pair.main, "main"), commit, "merged into main");

    // The way out named (iteration 5): the branch's last commit is not
    // known, so `<commit>` stays literal; then approve (a commit found
    // completes it) or reject (iteration 6).
    let gone = format!(
        "the branch `t1` no longer exists; recreate it at its last commit (`git branch t1 \
         <commit>`), then `spec approve {id}` or `spec reject {id}`"
    );
    let review = pair.review_ok(&pair.main, &id);
    let note = format!("cannot tell whether its commit is on `t1`: {gone}");
    assert_eq!(
        review
            .document
            .notes
            .iter()
            .filter(|line| **line == note)
            .count(),
        1,
        "{:?}",
        review.document.notes
    );
    assert!(
        !review
            .document
            .notes
            .iter()
            .any(|line| line.contains("completes it")),
        "{:?}",
        review.document.notes
    );
    assert!(printed(&review).0.contains(&note));

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let message = cannot(&outcome, "approve, its worktree and branch gone");
    let unknown = format!("cannot tell whether `{id}` has its commit on `t1`: {gone}");
    assert_eq!(
        message,
        format!(
            "spec: `{id}` not applied (step 2): the proposal's worktree {worktree} no longer \
             exists; {unknown}"
        )
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "still open, nothing written");
    assert_eq!(
        pair.events_of(&id),
        [ev("created", None), ev("apply_failed", Some(2))]
    );
    let logged = pair.events().pop().expect("an event");
    assert!(
        logged.payload["reason"]
            .as_str()
            .is_some_and(|reason| reason.ends_with(&unknown)),
        "{:?}",
        logged.payload
    );

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Merged anyway.", true);
    let reason = refused(&outcome, "reject, its branch gone");
    assert_eq!(
        reason,
        format!("cannot tell whether `{id}` has its commit in history: {gone}")
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
}

/// The proposal's base commit missing from the repository (set by SQL to
/// an object ID git does not have), the branch there (iteration 6, review
/// of iteration 5: a base commit pruned after a rebase turned the lookup
/// into "cannot tell", which disabled the re-merge protection and left
/// `reject` a dead end): the branch's whole history is read instead, so
/// nothing is "cannot tell". tgb: `review` has no "cannot tell" note,
/// preview `applies`; `approve` declined asks exactly the apply question,
/// no note line before it, nothing changed; `reject` goes through (one
/// prompt, `rejected`, one event). tgb3: a second proposal with a missing
/// base commit is applied with no "cannot tell" note, its commit on `t1`
/// carrying its trailer; approved again, it is refused as already applied
/// with that commit. M: the whole-branch fallback removed (a missing base
/// commit is a lookup error).
#[test]
fn a_missing_base_commit_reads_the_whole_branch_and_lets_reject_through() {
    let pair = Pair::new("pa-gone-base", "spec-a");
    let first = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let second = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    let fake = "0123456789abcdef0123456789abcdef01234567";
    pair.sql(&format!("update proposals set base_commit = '{fake}'"));
    for id in [&first, &second] {
        assert_eq!(pair.proposal(id).place.base_commit, fake);
    }
    let unknown = |notes: &[String]| notes.iter().any(|line| line.contains("cannot tell"));
    let worktree = pair.proposal(&first).place.worktree;

    let review = pair.review_ok(&pair.main, &first);
    assert!(
        !unknown(&review.document.notes),
        "{:?}",
        review.document.notes
    );
    assert_eq!(review.document.preview, Some(Preview::Applies));

    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) =
        pair.approve_answer(&pair.main, &first, false, pair.git_env(&pair.main));
    assert_eq!(
        questions,
        [format!(
            "apply {first} to {PATH} on t1 in {worktree} (applies)? [y/N]"
        )],
        "the question alone, no note line"
    );
    let reason = refused(&outcome, "approve declined, its base commit missing");
    assert_eq!(
        reason,
        format!("`{first}` not applied: the answer was not `y`; nothing changed")
    );
    assert!(!unknown(&notes_of(outcome.as_ref().unwrap())));
    assert_eq!(pair.state(), before, "nothing written");

    let (outcome, questions) = pair.reject_answer(&pair.main, &first, "Unsure.", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("reject {first}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    let stored = pair.proposal(&first);
    assert_eq!(stored.status.as_str(), "rejected");
    assert_eq!(stored.decision_note.as_deref(), Some("Unsure."));
    assert_eq!(pair.events().len(), events + 1, "one event");
    assert_eq!(pair.events_of(&first).last(), Some(&ev("rejected", None)));

    let outcome = pair.approve_ok(&pair.main, &second);
    assert!(!unknown(&notes_of(&outcome)), "{:?}", outcome.messages);
    let tip = pair.rev(&pair.linked, "t1");
    let stored = pair.proposal(&second);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(tip.as_str()));
    assert!(
        pair.git_text(&pair.linked, &["log", "-1", "--format=%B"])
            .contains(&format!("Proposal: {second}"))
    );
    let again = pair.approve(&pair.main, &second);
    assert_eq!(
        refused(&again, "approve of an applied proposal"),
        format!("`{second}` is already applied: commit {tip}")
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), tip, "no second commit");
}

/// tgb2 (iteration 6): the base commit missing (SQL) and the branch
/// deleted with its worktree, a proposal of the current repository. The
/// lookup names both and the way out — "the branch `t1` no longer exists,
/// and the proposal's base commit <sha> is not in the repository; recreate
/// the branch at its last commit (`git branch t1 <commit>`), then `spec
/// approve PR-0001` or `spec reject PR-0001`" — in `review`'s note,
/// appended to `approve`'s exit 2 at step 2 (no prompt), and in
/// `reject`'s refusal before any prompt (accepted deviation: refused, not
/// skipped as for an orphan), nothing written, no event. Followed (`git
/// branch t1 main`; the base still missing, the branch's whole history
/// read): `reject` records `rejected`. M: the both-missing text dropped;
/// a missing branch skipped as "no commit".
#[test]
fn a_missing_base_and_branch_are_named_together_and_the_way_out_lets_reject_through() {
    let pair = Pair::new("pa-gone-both", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let fake = "0123456789abcdef0123456789abcdef01234567";
    pair.sql(&format!(
        "update proposals set base_commit = '{fake}' where id = '{id}'"
    ));
    let worktree = pair.proposal(&id).place.worktree;
    remove_worktree(&pair);
    pair.git.git(&pair.main, &["branch", "-q", "-D", "t1"]);
    let both = format!(
        "the branch `t1` no longer exists, and the proposal's base commit {fake} is not in the \
         repository; recreate the branch at its last commit (`git branch t1 <commit>`), then \
         `spec approve {id}` or `spec reject {id}`"
    );

    let review = pair.review_ok(&pair.main, &id);
    let note = format!("cannot tell whether its commit is on `t1`: {both}");
    assert_eq!(
        review
            .document
            .notes
            .iter()
            .filter(|line| **line == note)
            .count(),
        1,
        "{:?}",
        review.document.notes
    );

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    assert_eq!(
        cannot(&outcome, "approve, the base commit and the branch missing"),
        format!(
            "spec: `{id}` not applied (step 2): the proposal's worktree {worktree} no longer \
             exists; cannot tell whether `{id}` has its commit on `t1`: {both}"
        )
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "still open, nothing written");

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Abandoned.", true);
    assert_eq!(
        refused(&outcome, "reject, the base commit and the branch missing"),
        format!("cannot tell whether `{id}` has its commit in history: {both}")
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");

    pair.git.git(&pair.main, &["branch", "t1", "main"]);
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Abandoned.", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("reject {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert_eq!(pair.proposal(&id).status.as_str(), "rejected");
    assert_eq!(pair.events().len(), events + 1, "one event");
    assert_eq!(pair.events_of(&id).last(), Some(&ev("rejected", None)));
}

/// tn (iteration 6, review of iteration 5, nit): a lookup git cannot make
/// — a `git` first on the caller's `PATH` that fails `git log -z` (the
/// trailer read) and runs the real one otherwise — is shown before the
/// consent question: `approve` asks one two-line question, "note: cannot
/// tell whether `PR-0001` has its commit on `t1`: <git's error>" then
/// "apply PR-0001 to <path> on t1 in <worktree> (applies)? [y/N]".
/// Declined: refused, nothing written. Accepted: applied, the note among
/// the outcome's messages too (accepted deviation: printed again after the
/// answer). M: the note shown only after the answer.
#[test]
fn a_failed_lookups_note_is_shown_before_the_consent_question() {
    let pair = Pair::new("pa-note-first", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let shim = pair.scratch.dir("shim");
    let real = pair.git.git_program().to_path_buf();
    fs::write(
        shim.join("git"),
        format!(
            "#!/bin/sh\nlog=0\nz=0\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    log) \
             log=1 ;;\n    -z) z=1 ;;\n  esac\ndone\nif [ $log = 1 ] && [ $z = 1 ]; then\n  \
             echo 'fatal: simulated log failure' >&2\n  exit 128\nfi\nexec '{real}' \"$@\"\n",
            real = real.display()
        ),
    )
    .unwrap();
    fs::set_permissions(shim.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = pair.git.var("PATH").expect("the sandbox PATH").to_owned();
    let mut wrapped = shim.into_os_string();
    wrapped.push(":");
    wrapped.push(&path);
    let git = with_vars(&pair.git_env(&pair.main), &[("PATH", OsStr::new(&wrapped))]);
    let worktree = pair.proposal(&id).place.worktree;
    let question = format!("apply {id} to {PATH} on t1 in {worktree} (applies)? [y/N]");
    let note = format!("cannot tell whether `{id}` has its commit on `t1`: ");
    let noted = |questions: &[String]| {
        assert_eq!(questions.len(), 1, "{questions:?}");
        let lines: Vec<&str> = questions[0].lines().collect();
        assert_eq!(lines.len(), 2, "the note, then the question: {questions:?}");
        assert!(
            lines[0].starts_with(&format!("note: {note}"))
                && lines[0].contains("simulated log failure"),
            "{questions:?}"
        );
        assert_eq!(lines[1], question);
    };

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, false, git.clone());
    noted(&questions);
    assert_eq!(
        refused(&outcome, "approve declined"),
        format!("`{id}` not applied: the answer was not `y`; nothing changed")
    );
    assert_eq!(pair.state(), before, "nothing written");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, git);
    noted(&questions);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        notes_of(&outcome)
            .iter()
            .any(|line| line.starts_with(&note) && line.contains("simulated log failure")),
        "{:?}",
        outcome.messages
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(
        stored.applied_commit.as_deref(),
        Some(pair.rev(&pair.linked, "t1").as_str())
    );
}

/// tn, the completion's question (iteration 6: step 5 finds the
/// proposal's own commit, and the completion asks through the consent
/// wrapper): an `open` proposal's commit made by hand on `t1`; a `git`
/// first on the caller's `PATH` fails only the run's first `git log -z`
/// (the lookup before the steps; a marker file, removed before each run),
/// so step 5's own lookup finds the commit. `approve` asks one two-line
/// question, "note: cannot tell whether `PR-0001` has its commit on `t1`:
/// <git's error>" then "complete PR-0001 by its commit <sha> on t1 in
/// <worktree>? [y/N]". Declined: refused, nothing written; accepted:
/// `applied` with that commit, no new commit. M: the note shown only after
/// the answer.
#[test]
fn a_failed_lookups_note_is_shown_before_the_completions_question() {
    let pair = Pair::new("pa-note-complete", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let shim = pair.scratch.dir("shim-once");
    let marker = pair.scratch.join("log-failed-once");
    let real = pair.git.git_program().to_path_buf();
    fs::write(
        shim.join("git"),
        format!(
            "#!/bin/sh\nlog=0\nz=0\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    log) \
             log=1 ;;\n    -z) z=1 ;;\n  esac\ndone\nif [ $log = 1 ] && [ $z = 1 ] && [ ! -e \
             '{marker}' ]; then\n  : > '{marker}'\n  echo 'fatal: simulated log failure' >&2\n  \
             exit 128\nfi\nexec '{real}' \"$@\"\n",
            marker = marker.display(),
            real = real.display()
        ),
    )
    .unwrap();
    fs::set_permissions(shim.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = pair.git.var("PATH").expect("the sandbox PATH").to_owned();
    let mut wrapped = shim.into_os_string();
    wrapped.push(":");
    wrapped.push(&path);
    let git = with_vars(&pair.git_env(&pair.main), &[("PATH", OsStr::new(&wrapped))]);
    let worktree = pair.proposal(&id).place.worktree;
    let question = format!("complete {id} by its commit {commit} on t1 in {worktree}? [y/N]");
    let note = format!("note: cannot tell whether `{id}` has its commit on `t1`: ");
    let noted = |questions: &[String]| {
        assert_eq!(questions.len(), 1, "{questions:?}");
        let lines: Vec<&str> = questions[0].lines().collect();
        assert_eq!(lines.len(), 2, "the note, then the question: {questions:?}");
        assert!(
            lines[0].starts_with(&note) && lines[0].contains("simulated log failure"),
            "{questions:?}"
        );
        assert_eq!(lines[1], question);
    };

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, false, git.clone());
    assert!(marker.exists(), "the first lookup failed");
    noted(&questions);
    assert_eq!(
        refused(&outcome, "completion declined"),
        format!("`{id}` not completed: the answer was not `y`; nothing changed")
    );
    assert_eq!(pair.state(), before, "nothing written");

    fs::remove_file(&marker).unwrap();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, git);
    noted(&questions);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
}

/// Both fixtures (spec-a, a game; spec-b, a command-line tool with Russian
/// prose and other `[ids]`): an `open` proposal's text committed by hand on
/// `t1` with its trailer, the worktree removed; the content check parses
/// the commit's blob under that commit's `specengine.toml`, so `approve`
/// asks to complete it and records `applied` with that commit. M: the
/// content check reading the wrong scheme or path (no completion); before
/// iteration 4 no completion once the worktree was gone.
#[test]
fn a_commit_by_hand_completes_after_the_worktree_is_removed_in_both_fixtures() {
    for case in &CASES {
        let pair = Pair::of(&format!("pa-gone-{}", case.fixture), case);
        let (_, span) = pair.span(&pair.linked, case.target);
        let new = edit(&span, case.first.0, case.first.1);
        let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
        replace(&pair.linked, case.path, &span, &new);
        pair.git.git(
            &pair.linked,
            &[
                "commit",
                "-q",
                "-m",
                "By hand.",
                "-m",
                &format!("Proposal: {id}"),
                "--",
                case.path,
            ],
        );
        let commit = pair.rev(&pair.linked, "t1");
        let worktree = pair.proposal(&id).place.worktree;
        remove_worktree(&pair);

        let (outcome, questions) =
            pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        let outcome = outcome.unwrap_or_else(|error| panic!("{}: {error}", case.fixture));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "{}: {outcome:?}",
            case.fixture
        );
        assert_eq!(
            questions,
            [format!(
                "complete {id} by its commit {commit} on t1 in {}? [y/N]",
                pair.main.display()
            )],
            "{}",
            case.fixture
        );
        assert!(
            !questions[0].contains(&worktree),
            "{}: the gone worktree is not named: {questions:?}",
            case.fixture
        );
        let stored = pair.proposal(&id);
        assert_eq!(stored.status.as_str(), "applied", "{}", case.fixture);
        assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
        assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    }
}

/// A project root below the worktree's top (`root_rel` `proj`, spec-b):
/// proposed in `t1/proj`, its text committed by hand with the trailer
/// (only `proj/<path>`), the worktree removed. From `main/proj`: `review`
/// notes the commit, `reject` is refused naming it, `approve` completes it
/// — the content check reads `proj/specengine.toml` and `proj/<path>` in
/// the commit's tree. M: the commit's config read at the top
/// (`specengine.toml`, not `<root_rel>/specengine.toml`).
#[test]
fn a_root_below_the_top_completes_by_its_commit() {
    let scratch = common::Scratch::new("pa-gone-subdir");
    let git = common::git::Sandbox::new(scratch.path());
    let main = scratch.dir("main");
    copy_dir(&common::fixture("spec-b"), &main.join("proj"));
    write(&main, "README.txt", "outside the root\n");
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "a project below the top");
    let linked = scratch.join("t1");
    git.git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "t1",
            linked.to_str().unwrap(),
        ],
    );
    let linked = fs::canonicalize(linked).unwrap();
    let main = fs::canonicalize(main).unwrap();
    let home = scratch.home("h");
    let env = |cwd: &std::path::Path| Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.clone().into_os_string()),
        xdg_data_home: None,
    };
    let genv = |cwd: &std::path::Path| GitEnv::new(cwd, git.vars());
    let globals = Globals::default();
    let case = &CASES[1];
    assert_eq!(case.fixture, "spec-b");
    let node = show(
        &env(&linked.join("proj")),
        &globals,
        &ShowRequest {
            reference: case.target.to_owned(),
            links: false,
            archive: false,
        },
    )
    .unwrap()
    .nodes
    .remove(0);
    let new_text = edit(&node.text, case.first.0, case.first.1);
    let outcome = propose(
        &env(&linked.join("proj")),
        &globals,
        &ProposeRequest {
            target: case.target.to_owned(),
            base: node.span_hash.clone(),
            text: ProposedText::Given(new_text.clone().into_bytes()),
            rationale: "Below the top.".to_owned(),
            author_role: None,
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: genv(&linked.join("proj")),
        },
    )
    .unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let id = outcome.document.id.clone().expect("an ID");
    let top_path = format!("proj/{}", case.path);
    replace(&linked, &top_path, &node.text, &new_text);
    git.git(
        &linked,
        &[
            "commit",
            "-q",
            "-m",
            "By hand, below the top.",
            "-m",
            &format!("Proposal: {id}"),
            "--",
            &top_path,
        ],
    );
    let commit = git.git_text(&linked, &["rev-parse", "t1"]);
    git.git(&main, &["worktree", "remove", linked.to_str().unwrap()]);

    let review = specengine_cli::review(
        &env(&main.join("proj")),
        &globals,
        &specengine_cli::ReviewRequest {
            id: id.clone(),
            git: genv(&main.join("proj")),
        },
    )
    .unwrap();
    let note = format!(
        "its commit {commit} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert!(
        review.document.notes.contains(&note),
        "{:?}",
        review.document.notes
    );

    let mut asked = Vec::new();
    let outcome = specengine_cli::reject(
        &env(&main.join("proj")),
        &globals,
        &specengine_cli::RejectRequest {
            id: id.clone(),
            reason: Some("Below the top.".to_owned()),
            now: LATER.to_owned(),
            git: genv(&main.join("proj")),
        },
        &mut |question: &str| {
            asked.push(question.to_owned());
            true
        },
    );
    let reason = refused(&outcome, "reject below the top");
    assert!(
        reason.contains(&format!("has its commit {commit} on `t1`")),
        "{reason}"
    );
    assert!(asked.is_empty(), "{asked:?}");

    let mut questions = Vec::new();
    let outcome = approve(
        &env(&main.join("proj")),
        &globals,
        &ApproveRequest {
            id: id.clone(),
            note: None,
            now: LATER.to_owned(),
            git: genv(&main.join("proj")),
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "complete {id} by its commit {commit} on t1 in {}? [y/N]",
            main.join("proj").display()
        )]
    );
    assert!(
        outcome.messages.iter().any(|message| matches!(message,
            Message::Note(text) if text.contains(&format!("completed by its commit {commit}")))),
        "{:?}",
        outcome.messages
    );
}
