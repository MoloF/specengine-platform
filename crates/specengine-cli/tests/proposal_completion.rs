//! docs/features/proposal-apply.md, "Idempotence" as iteration 3 extends it
//! (review of iteration 2, MAJOR: a proposal once `open` again could never
//! be completed by its own commit), through the library with a consent
//! callback:
//!
//! - an `open` proposal whose own commit (its `Proposal:` trailer, one
//!   parent, only its path) is on its branch: `review` notes the commit
//!   once; `approve` asks `complete PR-0001 by its commit <sha> on <branch>
//!   in <worktree>? [y/N]`, declined changes nothing, yes records `applied`
//!   with the worktree's identity (events `approved`, `applied`), no new
//!   commit; no identity in the worktree: exit 2 at step 7 before the
//!   prompt, logged, still `open`;
//! - an `approved` proposal whose trailer commit changes two paths: step 5
//!   names the commit and why, the proposal stays `approved`; split, the
//!   next approve completes it without a prompt;
//! - an `approved` proposal refused at step 2 before its commit is
//!   cherry-picked stays `approved`; after the cherry-pick `review` names
//!   the commit and `approve` completes it;
//! - two completions of the same commit: the one that records second ends
//!   done with a note, one `applied` logged.
//!
//! "Library approve": the current directory the main worktree, the
//! proposal raised in the linked worktree on `t1` (`common::proposal`).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::proposal::{DECIDER, LATER, Pair, cannot, printed, refused};
use common::{read, replace};
use specengine_cli::{ApproveRequest, Exit, Globals, Message, Preview, approve};

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";

fn ev(kind: &str, step: Option<u64>) -> (String, Option<u64>) {
    (format!("proposal.{kind}"), step)
}

fn hook(pair: &Pair, body: &str) {
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let path = hooks.join("pre-commit");
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn remove_hook(pair: &Pair) {
    fs::remove_file(pair.main.join(".git/hooks/pre-commit")).unwrap();
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

/// The completion note of `review` (its document's notes) and whether a
/// step-5 note repeats it.
fn review_notes(pair: &Pair, id: &str) -> (Vec<String>, String) {
    let review = pair.review_ok(&pair.main, id);
    let text = printed(&review).0;
    (review.document.notes.clone(), text)
}

/// An `open` proposal with its own commit on `t1` (made by hand): `review`
/// says once that `spec approve` completes it (no second "not applicable
/// now (step 5)" note); with no git identity `approve` exits 2 at step 7
/// before any prompt (one `apply_failed`, still `open`); declined, exit 1
/// "not completed", nothing changed or logged; consented, `applied` with
/// that commit, decided by the worktree's committer at now, events
/// `approved` then `applied`, the note "completed by its commit", no new
/// commit; a further approve is refused naming the commit. M: the
/// completion only for `approved` (step 5 "already in place" for ever).
#[test]
fn an_open_proposal_with_its_commit_on_the_branch_completes_with_consent() {
    let pair = Pair::new("pa-complete-open", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let worktree = pair.proposal(&id).place.worktree;

    let (notes, text) = review_notes(&pair, &id);
    let note = format!(
        "its commit {commit} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert_eq!(
        notes.iter().filter(|line| **line == note).count(),
        1,
        "{notes:?}"
    );
    assert!(
        !notes.iter().any(|line| line.contains("(step 5)")),
        "no duplicate step-5 note: {notes:?}"
    );
    assert!(text.contains(&note), "{text}");

    // No identity in the worktree: exit 2 before the prompt, logged.
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
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, anonymous);
    let message = cannot(&outcome, "no identity");
    assert!(
        message.contains("(step 7)") && message.contains("no git identity"),
        "{message}"
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "still open, nothing written");
    assert_eq!(
        pair.events_of(&id),
        [ev("created", None), ev("apply_failed", Some(7))]
    );
    pair.git
        .git(&pair.main, &["config", "--unset", "user.useConfigOnly"]);

    let question = format!("complete {id} by its commit {commit} on t1 in {worktree}? [y/N]");
    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) =
        pair.approve_answer(&pair.main, &id, false, pair.git_env(&pair.main));
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

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions, [question]);
    assert!(
        outcome.messages.iter().any(|message| matches!(message,
            Message::Note(text)
                if text == &format!("`{id}` completed by its commit {commit} on `t1`; no new commit"))),
        "{:?}",
        outcome.messages
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
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
    let reason = refused(&pair.approve(&pair.main, &id), "applied");
    assert!(reason.contains(&commit), "{reason}");
}

/// A pre-commit hook that stages another file: step 10 refuses the
/// two-path commit, `approved` stays. Approved again before the split:
/// step 5 says the text is in place and names that commit with why it
/// does not complete ("changes …, generated.txt"), no prompt, still
/// `approved` with its decision (review of iteration 2: it was reopened
/// and never completable). Split into the path's commit (the trailer kept)
/// and the other file's, the next approve completes: no prompt, `applied`
/// with the split commit, the decision of step 7 kept, no new commit. M: a
/// refusal before the run's own hold reopens.
#[test]
fn a_two_path_commit_is_named_at_step_5_and_its_split_completes() {
    let pair = Pair::new("pa-complete-split", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    hook(
        &pair,
        "#!/bin/sh\necho generated > generated.txt\ngit add generated.txt\n",
    );
    let reason = refused(&pair.approve(&pair.main, &id), "two paths at step 10");
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(
        reason.contains(&format!("it changes {PATH}, generated.txt")),
        "{reason}"
    );
    let two = pair.rev(&pair.linked, "t1");
    let held = pair.proposal(&id);
    assert_eq!(held.status.as_str(), "approved");
    remove_hook(&pair);

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "before the split");
    assert!(questions.is_empty(), "{questions:?}");
    assert!(reason.contains("(step 5)"), "{reason}");
    assert!(reason.contains("already in place"), "{reason}");
    assert!(
        reason.contains(&format!(
            "the commit {two} on `t1` carries `Proposal: {id}` but changes {PATH}, generated.txt"
        )),
        "{reason}"
    );
    assert_eq!(pair.proposal(&id), held, "approved, untouched");
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10)),
            ev("apply_failed", Some(5))
        ]
    );

    pair.git.git(&pair.linked, &["reset", "-q", "HEAD~"]);
    pair.git.git(
        &pair.linked,
        &["commit", "-q", "-C", "ORIG_HEAD", "--", PATH],
    );
    let split = pair.rev(&pair.linked, "t1");
    pair.git.git(&pair.linked, &["add", "generated.txt"]);
    pair.git
        .git(&pair.linked, &["commit", "-q", "-m", "the generated file"]);
    let tip = pair.rev(&pair.linked, "t1");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "an approved completion asks nothing");
    assert_eq!(pair.rev(&pair.linked, "t1"), tip, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(split.as_str()));
    assert_eq!(stored.decided_by, held.decided_by, "step 7's decision");
    assert_eq!(stored.decided_at, held.decided_at);
    assert_eq!(pair.events_of(&id).last(), Some(&ev("applied", None)));
}

/// A pre-commit hook that moves `HEAD` to `side`: step 10 leaves the
/// proposal `approved`. Approved again while `HEAD` is still on `side`
/// (before the cherry-pick): exit 2 at step 2, still `approved` (review of
/// iteration 2: reopened, after which the cherry-pick could never complete
/// it). Cherry-picked onto `t1`: `review` names the commit once and shows
/// no step-5 note, its preview `unavailable`; `approve` completes it with
/// no prompt. M: a refusal before the run's own hold reopens.
#[test]
fn a_refusal_before_the_cherry_pick_keeps_approved_and_the_pick_completes() {
    let pair = Pair::new("pa-complete-pick", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    pair.git.git(&pair.linked, &["branch", "side"]);
    hook(&pair, "#!/bin/sh\ngit symbolic-ref HEAD refs/heads/side\n");
    let reason = refused(&pair.approve(&pair.main, &id), "HEAD moved by the hook");
    assert!(reason.contains("(step 10)"), "{reason}");
    remove_hook(&pair);
    let held = pair.proposal(&id);
    assert_eq!(held.status.as_str(), "approved");
    let side = pair.rev(&pair.linked, "side");
    assert_eq!(pair.rev(&pair.linked, "t1"), base);

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let message = cannot(&outcome, "HEAD on side");
    assert!(message.contains("(step 2)"), "{message}");
    assert!(questions.is_empty());
    assert_eq!(pair.proposal(&id), held, "approved, untouched");
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10)),
            ev("apply_failed", Some(2))
        ]
    );

    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
    pair.git.git(&pair.linked, &["cherry-pick", &side]);
    let picked = pair.rev(&pair.linked, "t1");
    let review = pair.review_ok(&pair.main, &id);
    let note = format!(
        "its commit {picked} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert_eq!(review.document.notes, [note], "one note, no step-5 note");
    assert_eq!(review.document.preview, Some(Preview::Unavailable));

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty());
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(picked.as_str()));
    assert_eq!(stored.decided_by, held.decided_by);
    assert_eq!(pair.rev(&pair.linked, "t1"), picked, "no new commit");
    assert_eq!(
        read(&pair.linked, PATH),
        pair.git
            .git(&pair.main, &["show", &format!("{picked}:{PATH}")])
    );
}

/// Two runs complete the same `open` proposal by the same commit: the
/// outer run asks; while it asks, a second run (the consent callback)
/// completes it. The outer run, consented, finds it recorded `applied`
/// with that same commit and ends done (exit 0) with the note "was
/// recorded applied with <sha> by another run meanwhile"; one `approved`
/// and one `applied` logged, no new commit. M: the outer run refused
/// ("already applied").
#[test]
fn a_completion_recorded_meanwhile_by_another_run_ends_done_with_a_note() {
    let pair = Pair::new("pa-complete-twice", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let mut inner = None;
    let mut consent = |_: &str| {
        let (outcome, questions) =
            pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        inner = Some((outcome, questions));
        true
    };
    let outcome = approve(
        &pair.env(&pair.main),
        &Globals::default(),
        &ApproveRequest {
            id: id.clone(),
            note: None,
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut consent,
    );
    let (inner, questions) = inner.expect("the inner run ran");
    let inner = inner.unwrap_or_else(|error| panic!("inner: {error}"));
    assert_eq!(inner.exit(), Exit::Answered, "inner: {inner:?}");
    assert_eq!(questions.len(), 1, "the inner run asked too");
    let outcome = outcome.unwrap_or_else(|error| panic!("outer: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "outer: {outcome:?}");
    assert!(
        outcome.messages.iter().any(|message| matches!(message,
            Message::Note(text)
                if text == &format!("`{id}` was recorded applied with {commit} by another run meanwhile"))),
        "{:?}",
        outcome.messages
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("applied", None)
        ]
    );
}
