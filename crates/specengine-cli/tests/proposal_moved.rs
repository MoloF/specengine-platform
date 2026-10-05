//! docs/features/proposal-apply.md, "Reject" and "Idempotence" as
//! iteration 5 rules them (review of iteration 4: MAJOR — an orphan, a
//! proposal whose recorded repository path is gone, counted a moved
//! project too, and `reject` skipped the lookup and recorded `rejected`
//! with its completing commit on the branch; known limit — a gone branch or
//! base commit gave no way out; nits — `reject`'s identity read in the
//! recorded worktree whatever repository it belongs to now, `review` of a
//! completing commit with its worktree gone):
//!
//! - an orphan's trailer commits are read on the branch of its name in the
//!   current repository; one there refuses the rejection, with "; it was
//!   recorded in the repository <old>, now gone or moved: move the
//!   repository back to <old> (`git worktree repair`), then `spec approve
//!   PR-0001`" (iteration 6), and following it completes the proposal;
//!   none there, or neither the branch nor the base commit in it, and the
//!   rejection goes through; the branch alone missing refuses, naming the
//!   way out "then `spec reject PR-0001`"; the base commit alone missing
//!   (pruned after a rebase): the branch's whole history is read
//!   (iteration 6, review of iteration 5);
//! - `reject`'s `decided_by` is the committer of the proposal's history (the
//!   current repository when the recorded worktree belongs to another or is
//!   gone), else the current repository's;
//! - the branch gone: recreated at a commit (`git branch t1 <commit>`, the
//!   way out named, ending "then `spec approve PR-0001` or `spec reject
//!   PR-0001`"), `reject` records the rejection;
//! - `review` of an `open` proposal whose commit is on `t1`, the worktree
//!   removed: one note, no step-2 note, preview `unavailable`.
//!
//! Through the library with a consent callback; the proposal raised in the
//! linked worktree on `t1` (`common::proposal`). Where the committer
//! identity is the subject, the caller's git environment has no identity
//! variables, so each repository's own `user.name` / `user.email` decides.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::proposal::{DECIDER, LATER, Pair, refused};
use common::{replace, write};
use specengine_cli::{
    CliError, Exit, Globals, Message, Preview, ProposalOutcome, RejectRequest, reject,
};
use specengine_store::GitEnv;

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";

fn ev(kind: &str, step: Option<u64>) -> (String, Option<u64>) {
    (format!("proposal.{kind}"), step)
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

/// `git worktree remove <linked>` from the main worktree (the branch kept).
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

/// The whole project moved: `main` and `t1` into `<scratch>/moved/`, the
/// worktree link repaired from the moved main worktree. The moved main
/// worktree, canonical.
fn move_project(pair: &Pair) -> PathBuf {
    let moved = pair.scratch.join("moved");
    fs::create_dir_all(&moved).unwrap();
    fs::rename(&pair.main, moved.join("main")).unwrap();
    fs::rename(&pair.linked, moved.join("t1")).unwrap();
    let main = fs::canonicalize(moved.join("main")).unwrap();
    let linked = fs::canonicalize(moved.join("t1")).unwrap();
    pair.git.git(
        &main,
        &["worktree", "repair", linked.to_str().expect("UTF-8")],
    );
    assert_eq!(
        pair.git_text(&linked, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "t1",
        "the moved worktree works"
    );
    main
}

/// The sandbox's variables in `cwd` without any identity: each
/// repository's config decides.
fn anonymous(pair: &Pair, cwd: &Path) -> GitEnv {
    pair.git_env(cwd)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "")
}

/// `user.name`, `user.email` and `user.useConfigOnly` in `dir`'s repository.
fn identity(pair: &Pair, dir: &Path, name: &str, email: &str) {
    pair.git.git(dir, &["config", "user.name", name]);
    pair.git.git(dir, &["config", "user.email", email]);
    pair.git.git(dir, &["config", "user.useConfigOnly", "true"]);
}

/// Library `reject` of `id` in `cwd` with the caller's git environment
/// `git`, consent yes: the outcome and the questions.
fn reject_as(
    pair: &Pair,
    cwd: &Path,
    id: &str,
    git: GitEnv,
) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
    let mut questions = Vec::new();
    let outcome = reject(
        &pair.env(cwd),
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: "Not wanted.".to_owned(),
            now: LATER.to_owned(),
            git,
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    );
    (outcome, questions)
}

fn has_note(outcome: &ProposalOutcome, note: &str) -> bool {
    outcome
        .messages
        .iter()
        .any(|message| matches!(message, Message::Note(text) if text == note))
}

/// The rejection went through: exit 0, one prompt, `rejected` by
/// `decided_by` with the reason, one `rejected` event.
fn assert_rejected(
    pair: &Pair,
    id: &str,
    outcome: &Result<ProposalOutcome, CliError>,
    decided_by: &str,
) {
    let outcome = outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("reject {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let stored = pair.proposal(id);
    assert_eq!(stored.status.as_str(), "rejected");
    assert_eq!(stored.decided_by.as_deref(), Some(decided_by));
    assert_eq!(stored.decided_at.as_deref(), Some(LATER));
    assert_eq!(stored.decision_note.as_deref(), Some("Not wanted."));
    assert_eq!(pair.events_of(id).last(), Some(&ev("rejected", None)));
}

/// tm (review of iteration 4, MAJOR): the proposal's own commit on `t1`,
/// then the whole project moved (`git worktree repair`): the recorded
/// common dir is gone, so `reject` takes it as an orphan with the note
/// "`PR-0001` belongs to the repository <old>, which no longer exists",
/// reads `t1` in the current (moved) repository, finds the commit and is
/// refused before any prompt: "`PR-0001` has its commit <sha> on `t1`: a
/// proposal whose commit is in history is never rejected; `spec approve
/// PR-0001` completes it; it was recorded in the repository <old>, now
/// gone or moved: move the repository back to <old> (`git worktree
/// repair`), then `spec approve PR-0001`" (iteration 6; accepted
/// deviation: <old> is the recorded common dir, ending `/.git`). Still
/// `open`, no event. M: the orphan lookup skipped always (recorded
/// `rejected`); the way-out suffix dropped.
#[test]
fn a_moved_projects_commit_refuses_the_orphans_rejection_with_the_way_out() {
    let pair = Pair::new("pa-moved-commit", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let old = pair.proposal(&id).place.git_common_dir;
    let main = move_project(&pair);
    assert!(!Path::new(&old).exists(), "the recorded common dir is gone");

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&main, &id, "Moved.", true);
    let reason = refused(&outcome, "reject after a move, its commit on t1");
    assert_eq!(
        reason,
        format!(
            "`{id}` has its commit {commit} on `t1`: a proposal whose commit is in history is \
             never rejected; `spec approve {id}` completes it; it was recorded in the repository \
             {old}, now gone or moved: move the repository back to {old} (`git worktree \
             repair`), then `spec approve {id}`"
        )
    );
    let outcome = outcome.unwrap();
    assert!(
        has_note(
            &outcome,
            &format!("`{id}` belongs to the repository {old}, which no longer exists")
        ),
        "{:?}",
        outcome.messages
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "open");
    assert!(stored.decided_by.is_none());
    assert_eq!(pair.events().len(), events, "no event");
}

/// tm2: no commit on `t1`, the project moved: the orphan's lookup reads
/// `t1` in the moved repository, finds no trailer commit, and the
/// rejection goes through (one prompt, `rejected`, one event, the orphan
/// note). M: a lookup that finds nothing taken as a failure.
#[test]
fn a_moved_projects_proposal_without_a_commit_is_rejected() {
    let pair = Pair::new("pa-moved-none", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let old = pair.proposal(&id).place.git_common_dir;
    let main = move_project(&pair);
    let (outcome, questions) = reject_as(&pair, &main, &id, pair.git_env(&main));
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert_rejected(&pair, &id, &outcome, DECIDER);
    assert!(
        has_note(
            outcome.as_ref().unwrap(),
            &format!("`{id}` belongs to the repository {old}, which no longer exists")
        ),
        "{outcome:?}"
    );
}

/// tm3: the proposal's commit on `t1`, then its repository deleted; the
/// current repository an unrelated copy of the project (a fresh `git
/// init`, no `t1`, committer "Bob" in its config). The orphan's lookup is
/// skipped (the branch is not in the current repository) and the
/// rejection goes through, decided by the current repository's committer
/// (`history()` has no repository to read). M: the lookup refusing on a
/// missing branch; `decided_by` from the gone place.
#[test]
fn a_gone_repositorys_proposal_is_rejected_from_an_unrelated_copy_by_its_committer() {
    let pair = Pair::new("pa-moved-gone", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    commit_by_hand(&pair, &id);
    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.commit_all(&other, "a fresh copy of the project");
    let other = fs::canonicalize(other).unwrap();
    identity(&pair, &other, "Bob", "bob@example.org");
    fs::remove_dir_all(&pair.linked).unwrap();
    fs::remove_dir_all(&pair.main).unwrap();

    let (outcome, questions) = reject_as(&pair, &other, &id, anonymous(&pair, &other));
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert_rejected(&pair, &id, &outcome, "Bob <bob@example.org>");
}

/// tw (review of iteration 4, nit): the recorded worktree removed and its
/// path reused by another repository (committer "Eve", a branch `t1`
/// holding a `Proposal: PR-0001` commit); the recorded repository's `t1`
/// has none. `review` notes no completion; `reject` goes through, decided
/// by the recorded repository's committer ("Ann Owner", the current main
/// worktree's config), not Eve. M: `decided_by` read in the recorded
/// worktree whatever repository it belongs to.
#[test]
fn reject_is_decided_by_the_proposals_repository_not_the_one_now_at_its_worktree() {
    let pair = Pair::new("pa-moved-reused", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    remove_worktree(&pair);
    let foreign = pair.scratch.copy("spec-a", "t1");
    assert_eq!(fs::canonicalize(&foreign).unwrap(), pair.linked);
    pair.git.init(&foreign);
    identity(&pair, &foreign, "Eve", "eve@example.org");
    pair.commit_all(&foreign, "another repository");
    pair.git.git(&foreign, &["branch", "-m", "t1"]);
    replace(&foreign, PATH, FROM, TO);
    pair.git.git(
        &foreign,
        &[
            "commit",
            "-q",
            "-am",
            "spec: apply",
            "-m",
            &format!("Proposal: {id}"),
        ],
    );
    identity(&pair, &pair.main, "Ann Owner", "ann@example.org");

    let review = pair.review_ok(&pair.main, &id);
    assert!(
        !review
            .document
            .notes
            .iter()
            .any(|line| line.contains("completes it")),
        "{:?}",
        review.document.notes
    );

    let (outcome, questions) = reject_as(&pair, &pair.main, &id, anonymous(&pair, &pair.main));
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert_rejected(&pair, &id, &outcome, "Ann Owner <ann@example.org>");
}

/// tg (review of iteration 4, known limit): an abandoned task — the
/// proposal open, no commit, the worktree removed and `t1` deleted.
/// `reject` is refused naming the way out ("recreate it at its last commit
/// (`git branch t1 <commit>`), then `spec approve PR-0001` or `spec reject
/// PR-0001`", iteration 6: the same hint in all three commands); followed
/// (`t1` recreated at the proposal's base commit), `review` has no "cannot
/// tell" note and `reject` records `rejected` with one prompt and one
/// event. M: the way-out text dropped.
#[test]
fn a_gone_branch_recreated_as_the_refusal_says_lets_reject_through() {
    let pair = Pair::new("pa-moved-branch", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    remove_worktree(&pair);
    pair.git.git(&pair.main, &["branch", "-q", "-D", "t1"]);

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Abandoned.", true);
    let reason = refused(&outcome, "reject, its branch gone");
    let way_out = format!(
        "recreate it at its last commit (`git branch t1 <commit>`), then `spec approve {id}` or \
         `spec reject {id}`"
    );
    assert_eq!(
        reason,
        format!(
            "cannot tell whether `{id}` has its commit in history: the branch `t1` no longer \
             exists; {way_out}"
        )
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.events().len(), events, "no event");

    let base = pair.proposal(&id).place.base_commit;
    pair.git.git(&pair.main, &["branch", "t1", &base]);
    let review = pair.review_ok(&pair.main, &id);
    assert!(
        !review
            .document
            .notes
            .iter()
            .any(|line| line.contains("cannot tell")),
        "{:?}",
        review.document.notes
    );
    let (outcome, questions) = reject_as(&pair, &pair.main, &id, pair.git_env(&pair.main));
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert_rejected(&pair, &id, &outcome, DECIDER);
}

/// An `open` proposal's commit made by hand on `t1`, the worktree removed
/// (the branch kept): `review` from the main worktree gives exactly one
/// note — "its commit <sha> is on `t1`: `spec approve PR-0001` completes
/// it; no new apply is needed" — no "not applicable now (step 2)" note
/// about the gone worktree, preview `unavailable`; the text output alike.
/// M: `review` running steps 2–6 when a completing commit exists.
#[test]
fn review_of_a_completing_commit_with_its_worktree_gone_gives_one_note() {
    let pair = Pair::new("pa-moved-review", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    remove_worktree(&pair);

    let review = pair.review_ok(&pair.main, &id);
    let note = format!(
        "its commit {commit} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert_eq!(review.document.notes, std::slice::from_ref(&note));
    assert_eq!(review.document.preview, Some(Preview::Unavailable));
    let text = common::proposal::printed(&review).0;
    assert!(text.contains(&note), "{text}");
    assert!(!text.contains("step 2"), "{text}");
    assert_eq!(
        pair.proposal(&id).status.as_str(),
        "open",
        "review writes nothing"
    );
}

/// The orphan suffix of a refusal naming a commit (iteration 6): the way
/// out to `spec approve` in the recorded repository.
fn moved_back(id: &str, old: &str) -> String {
    format!(
        "; it was recorded in the repository {old}, now gone or moved: move the repository back \
         to {old} (`git worktree repair`), then `spec approve {id}`"
    )
}

/// `reject`'s refusal naming the proposal's own commit on `t1`.
fn has_its_commit(id: &str, commit: &str) -> String {
    format!(
        "`{id}` has its commit {commit} on `t1`: a proposal whose commit is in history is never \
         rejected; `spec approve {id}` completes it"
    )
}

/// `main` moved on, `t1` rebased onto it in `linked`, every reflog expired
/// and the unreachable commits pruned.
fn rebase_and_prune(pair: &Pair, main: &Path, linked: &Path) {
    write(main, "m.txt", "main moves\n");
    pair.git.git(main, &["add", "m.txt"]);
    pair.git.git(main, &["commit", "-q", "-m", "main moves"]);
    pair.git.git(linked, &["rebase", "-q", "main"]);
    pair.git
        .git(main, &["reflog", "expire", "--expire=now", "--all"]);
    pair.git.git(main, &["gc", "-q", "--prune=now"]);
}

/// Whether `dir`'s repository holds the object `oid`.
fn has_object(pair: &Pair, dir: &Path, oid: &str) -> bool {
    pair.git
        .git_output(dir, &["cat-file", "-e", oid], &[])
        .status
        .success()
}

/// n2 r (iteration 6, review of iteration 5, FIX: the orphan lookup was
/// skipped when the branch or the base commit was missing, so the
/// rejection went through with the proposal's own commit on a renamed
/// branch): the proposal's commit on `t1`, the project moved, `t1`
/// renamed `t1-done`. `reject` (an orphan; the base commit there, the
/// branch not) is refused before any prompt: "cannot tell whether
/// `PR-0001` has its commit in this repository: the branch `t1` no longer
/// exists; recreate it at its last commit (`git branch t1 <commit>`), then
/// `spec reject PR-0001`" (accepted deviation: the way out, not the
/// commit). Followed (`git branch t1 t1-done`): refused naming that commit
/// with the orphan suffix. Still `open`, no event. M: the orphan lookup
/// skipped on any missing branch or base commit (recorded `rejected`).
#[test]
fn a_moved_projects_renamed_branch_refuses_the_orphans_rejection_with_the_way_out() {
    let pair = Pair::new("pa-moved-renamed", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let old = pair.proposal(&id).place.git_common_dir;
    let main = move_project(&pair);
    pair.git.git(&main, &["branch", "-m", "t1", "t1-done"]);

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&main, &id, "Moved.", true);
    assert_eq!(
        refused(&outcome, "reject after a move, t1 renamed"),
        format!(
            "cannot tell whether `{id}` has its commit in this repository: the branch `t1` no \
             longer exists; recreate it at its last commit (`git branch t1 <commit>`), then `spec \
             reject {id}`"
        )
    );
    assert!(
        has_note(
            outcome.as_ref().unwrap(),
            &format!("`{id}` belongs to the repository {old}, which no longer exists")
        ),
        "{outcome:?}"
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");

    pair.git.git(&main, &["branch", "t1", "t1-done"]);
    let (outcome, questions) = pair.reject_answer(&main, &id, "Moved.", true);
    assert_eq!(
        refused(&outcome, "reject after the branch recreated"),
        format!("{}{}", has_its_commit(&id, &commit), moved_back(&id, &old))
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "open");
    assert!(stored.decided_by.is_none());
    assert_eq!(pair.events().len(), events, "no event");
}

/// n2 b (iteration 6, review of iteration 5, FIX): work on `t1` before
/// the proposal (its base commit `t1`'s own), the proposal's commit by
/// hand, the project moved, `main` moved on and `t1` rebased onto it, the
/// reflogs expired and the old commits pruned: the base commit is not in
/// the repository (asserted), the branch is. `reject` (an orphan) reads
/// the branch's whole history and is refused before any prompt naming the
/// rebased commit, with the orphan suffix. Still `open`, no event. M: the
/// orphan lookup skipped on any missing branch or base commit (recorded
/// `rejected`); the whole-branch fallback removed.
#[test]
fn a_moved_projects_rebased_commit_with_its_base_pruned_refuses_the_orphans_rejection() {
    let pair = Pair::new("pa-moved-pruned", "spec-a");
    write(&pair.linked, "w.txt", "work\n");
    pair.git.git(&pair.linked, &["add", "w.txt"]);
    pair.git
        .git(&pair.linked, &["commit", "-q", "-m", "t1 work"]);
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.proposal(&id).place.base_commit;
    assert_eq!(base, pair.rev(&pair.linked, "t1"), "the base is t1's own");
    let commit = commit_by_hand(&pair, &id);
    let old = pair.proposal(&id).place.git_common_dir;
    let main = move_project(&pair);
    let linked = main.parent().expect("moved/").join("t1");
    rebase_and_prune(&pair, &main, &linked);
    let rebased = pair.rev(&linked, "t1");
    assert_ne!(rebased, commit, "rebased");
    assert!(!has_object(&pair, &main, &base), "the base commit pruned");
    assert!(!has_object(&pair, &main, &commit), "the old commit pruned");

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&main, &id, "Moved.", true);
    assert_eq!(
        refused(&outcome, "reject after a move, the base pruned"),
        format!("{}{}", has_its_commit(&id, &rebased), moved_back(&id, &old))
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "open");
    assert!(stored.decided_by.is_none());
    assert_eq!(pair.events().len(), events, "no event");
}

/// tm followed (iteration 6, review of iteration 5, nit: the orphan suffix
/// only implied the way out): the proposal's commit on `t1`, the project
/// moved; `reject` refused ending "…: move the repository back to <old>
/// (`git worktree repair`), then `spec approve PR-0001`". Followed — both
/// worktrees moved back where they were recorded, `git worktree repair` —
/// `approve` asks to complete it by that commit ("complete PR-0001 by its
/// commit <sha> on t1 in <worktree>? [y/N]") and records `applied` with
/// it, no new commit. M: the suffix's way out dropped.
#[test]
fn the_orphans_way_out_followed_completes_the_proposal_by_its_commit() {
    let pair = Pair::new("pa-moved-back", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let commit = commit_by_hand(&pair, &id);
    let place = pair.proposal(&id).place;
    let old = place.git_common_dir.clone();
    let main = move_project(&pair);
    let (outcome, questions) = pair.reject_answer(&main, &id, "Moved.", true);
    let reason = refused(&outcome, "reject after a move, its commit on t1");
    assert!(reason.ends_with(&moved_back(&id, &old)), "{reason}");
    assert!(questions.is_empty(), "{questions:?}");

    // Followed: the repository back at <old>, the links repaired.
    assert_eq!(Path::new(&old), pair.main.join(".git"));
    fs::rename(&main, &pair.main).unwrap();
    fs::rename(main.parent().expect("moved/").join("t1"), &pair.linked).unwrap();
    pair.git.git(
        &pair.main,
        &["worktree", "repair", pair.linked.to_str().expect("UTF-8")],
    );
    assert!(Path::new(&old).is_dir(), "the recorded common dir is back");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "complete {id} by its commit {commit} on t1 in {}? [y/N]",
            place.worktree
        )]
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    assert_eq!(pair.events_of(&id).last(), Some(&ev("applied", None)));
}
