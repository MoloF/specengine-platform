//! docs/features/proposal-apply.md, "Idempotence" and "Reject" as
//! iteration 4 rules them (review of iteration 3, minors):
//!
//! - a commit completes a proposal only when it carries the proposal's
//!   text: step 5's comparison on the commit's blob of the path (the target
//!   located under the `[ids]` of that commit's `specengine.toml`), so the
//!   new text and an apply's rebased merge complete, other text never does
//!   (no completion note, no prompt, an SQL-`approved` row not completed);
//! - `reject` refuses on any commit on the branch carrying the proposal's
//!   `Proposal:` trailer, before the prompt and after it, naming the newest
//!   with why it does not complete the proposal (`changes a, b` / `has N
//!   parents` / `does not carry the proposal's text`), counting older ones,
//!   and saying what completes it;
//! - steps 5 and 10 and `reject` share that hint: "a commit on `t1` with
//!   the trailer `Proposal: PR-0001`, one parent, changing only `<path>`
//!   to the proposal's text completes it (`spec approve PR-0001`)".
//!
//! Through the library with a consent callback; the current directory the
//! main worktree, the proposal raised in the linked worktree on `t1`
//! (`common::proposal`).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::proposal::{DECIDER, NOW, Pair, refused};
use common::{replace, write};
use specengine_cli::{Exit, Preview};

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

/// `git commit -q -m <subject> -m "Proposal: <id>"` in the linked worktree
/// of what is staged: the new tip of `t1`.
fn commit_with_trailer(pair: &Pair, subject: &str, id: &str, extra: &[&str]) -> String {
    let trailer = format!("Proposal: {id}");
    let mut args = vec!["commit", "-q", "-m", subject, "-m", &trailer];
    args.extend_from_slice(extra);
    pair.git.git(&pair.linked, &args);
    pair.rev(&pair.linked, "t1")
}

/// The shared hint of steps 5, 10 and `reject`.
fn completes_when(id: &str) -> String {
    format!(
        "a commit on `t1` with the trailer `Proposal: {id}`, one parent, changing only `{PATH}` \
         to the proposal's text completes it (`spec approve {id}`)"
    )
}

/// `reject`'s refusal for a trailer commit that does not complete the
/// proposal.
fn names_it(id: &str, commit: &str, older: &str, why: &str) -> String {
    format!(
        "`{id}` has the commit {commit} on `t1` with the trailer `Proposal: {id}`{older}, which \
         does not complete it ({why}): a proposal with a commit in history is never rejected; {}",
        completes_when(id)
    )
}

/// `reject` refused before any prompt with `want`, nothing written, no
/// event.
fn reject_refused_before_the_prompt(pair: &Pair, id: &str, want: &str) {
    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, id, "Not wanted.", true);
    let reason = refused(&outcome, "reject with a trailer commit on t1");
    assert_eq!(reason, want);
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");
}

/// The reviewer's t1: a pre-commit hook stages `generated.txt` into the
/// apply commit. Step 10's reason names the commit, why it is not the
/// apply commit and the shared hint, exactly; `approved` stays. Approved
/// again, step 5 names the same commit with why and the same hint. `reject`
/// of the `approved` proposal is refused before the prompt naming the
/// commit "(changes <path>, generated.txt)" with the hint, nothing
/// written, no event (review of iteration 3: it was rejected while history
/// held the owner-attributed edit). M: reject guard only for completing
/// commits.
#[test]
fn a_two_path_commit_is_named_with_the_shared_hint_and_refuses_reject() {
    let pair = Pair::new("pa-content-two", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    hook(
        &pair,
        "#!/bin/sh\necho generated > generated.txt\ngit add generated.txt\n",
    );
    let reason = refused(&pair.approve(&pair.main, &id), "two paths at step 10");
    remove_hook(&pair);
    let two = pair.rev(&pair.linked, "t1");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 10): the commit {two} on `t1` is not the apply commit (it \
             changes {PATH}, generated.txt); `{id}` stays approved; {}",
            completes_when(&id)
        )
    );
    assert_eq!(pair.proposal(&id).status.as_str(), "approved");

    let reason = refused(&pair.approve(&pair.main, &id), "two paths at step 5");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 5): `{TARGET}`: the proposal's text is already in place in \
             `{PATH}`; the commit {two} on `t1` carries `Proposal: {id}` but changes {PATH}, \
             generated.txt; {}; nothing to write or commit",
            completes_when(&id)
        )
    );

    reject_refused_before_the_prompt(
        &pair,
        &id,
        &names_it(&id, &two, "", &format!("changes {PATH}, generated.txt")),
    );
    assert_eq!(pair.proposal(&id).status.as_str(), "approved");
}

/// An `open` proposal whose trailer commit is a merge (`side` merged with
/// `--no-ff`, the edit added to the merge): `reject` is refused naming it
/// "(has 2 parents)". A newer trailer commit that changes another file
/// too: the refusal names that newest one "(changes …)" and counts the
/// merge: " (1 older one(s) too)". M: reject guard only for completing
/// commits.
#[test]
fn reject_names_a_merge_commit_and_counts_the_older_ones() {
    let pair = Pair::new("pa-content-merge", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    pair.git
        .git(&pair.linked, &["checkout", "-q", "-b", "side"]);
    write(&pair.linked, "side.txt", "side\n");
    pair.git.git(&pair.linked, &["add", "side.txt"]);
    pair.git
        .git(&pair.linked, &["commit", "-q", "-m", "a side commit"]);
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
    pair.git.git(
        &pair.linked,
        &["merge", "-q", "--no-ff", "--no-commit", "side"],
    );
    replace(&pair.linked, PATH, FROM, TO);
    pair.git.git(&pair.linked, &["add", PATH]);
    let merge = commit_with_trailer(&pair, "spec: apply by merge", &id, &[]);
    assert_eq!(
        pair.git_text(&pair.linked, &["log", "-1", "--format=%P"])
            .split(' ')
            .count(),
        2
    );
    reject_refused_before_the_prompt(&pair, &id, &names_it(&id, &merge, "", "has 2 parents"));

    write(&pair.linked, "other.txt", "other\n");
    replace(
        &pair.linked,
        PATH,
        "is not a reference",
        "is never a reference",
    );
    pair.git.git(&pair.linked, &["add", "other.txt", PATH]);
    let newest = commit_with_trailer(&pair, "spec: more", &id, &[]);
    reject_refused_before_the_prompt(
        &pair,
        &id,
        &names_it(
            &id,
            &newest,
            " (1 older one(s) too)",
            &format!("changes {PATH}, other.txt"),
        ),
    );
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
}

/// t2i: a `Proposal: PR-0001` commit that changes only the path, one
/// parent, but to other text on the proposal's line. `review` shows no
/// completion note (its preview `conflicts`); `approve` of the `open`
/// proposal asks nothing and is refused at step 5 (the edits overlap),
/// still `open`; `reject` is refused naming it "(does not carry the
/// proposal's text)". Set `approved` by SQL, `approve` still does not
/// complete it: refused at step 5, `approved` with no commit (review of
/// iteration 3: an approved one completed silently). M: the content check
/// removed.
#[test]
fn a_commit_with_other_text_never_completes() {
    let pair = Pair::new("pa-content-other", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    replace(&pair.linked, PATH, FROM, "the sprint DIFFERENT;");
    let other = commit_with_trailer(&pair, "spec: apply PR-0001", &id, &["--", PATH]);

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
    assert_eq!(review.document.preview, Some(Preview::Conflicts));

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "other text, open");
    assert!(questions.is_empty(), "no completion prompt: {questions:?}");
    assert!(reason.contains("(step 5)"), "{reason}");
    assert!(reason.contains("the edits overlap"), "{reason}");
    assert_eq!(pair.proposal(&id).status.as_str(), "open");

    reject_refused_before_the_prompt(
        &pair,
        &id,
        &names_it(&id, &other, "", "does not carry the proposal's text"),
    );

    pair.sql(&format!(
        "update proposals set status = 'approved', decided_by = '{DECIDER}', \
         decided_at = '{NOW}', updated_at = '{NOW}' where id = '{id}'"
    ));
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "other text, approved by SQL");
    assert!(questions.is_empty(), "{questions:?}");
    assert!(reason.contains("(step 5)"), "{reason}");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "approved", "not completed");
    assert!(stored.applied_commit.is_none(), "{stored:?}");
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&ev("apply_failed", Some(5)))
    );
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
}

/// A `Proposal: PR-0001` commit that changes only the path to other text,
/// then a plain commit (no trailer) that puts the proposal's text in
/// place: `approve` of the `open` proposal asks nothing and step 5 says
/// the text is already in place, naming the trailer commit "but does not
/// carry the proposal's text" with the shared hint; still `open`, nothing
/// written; `reject` is refused naming it the same way. M: the content
/// check removed.
#[test]
fn a_trailer_commit_with_other_text_is_named_at_step_5_when_the_text_came_otherwise() {
    let pair = Pair::new("pa-content-later", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    replace(&pair.linked, PATH, FROM, "the sprint ends now;");
    let other = commit_with_trailer(&pair, "spec: apply PR-0001", &id, &["--", PATH]);
    replace(&pair.linked, PATH, "the sprint ends now;", TO);
    pair.commit_all(&pair.linked, "the sprint ends at once");

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "text in place by a plain commit");
    assert!(questions.is_empty(), "no completion prompt: {questions:?}");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 5): `{TARGET}`: the proposal's text is already in place in \
             `{PATH}`; the commit {other} on `t1` carries `Proposal: {id}` but does not carry \
             the proposal's text; {}; nothing to write or commit",
            completes_when(&id)
        )
    );
    let mut after = pair.state();
    assert_eq!(after.proposals, before.proposals, "still open");
    after.proposals.clone_from(&before.proposals);
    assert_eq!(after, before, "nothing written");

    reject_refused_before_the_prompt(
        &pair,
        &id,
        &names_it(&id, &other, "", "does not carry the proposal's text"),
    );
}

/// A two-path trailer commit lands on `t1` while the owner is asked to
/// reject the `open` proposal (the consent callback commits it, then
/// answers yes): refused after the prompt naming it "(changes …)", asked
/// once, nothing written by the rejection, no event, still `open`. M:
/// reject guard only for completing commits.
#[test]
fn a_two_path_trailer_commit_landing_during_the_reject_prompt_refuses_it() {
    let pair = Pair::new("pa-content-during", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let events = pair.events().len();
    let mut asked = Vec::new();
    let mut consent = |question: &str| {
        asked.push(question.to_owned());
        replace(&pair.linked, PATH, FROM, TO);
        write(&pair.linked, "other.txt", "x\n");
        pair.git.git(&pair.linked, &["add", "other.txt", PATH]);
        commit_with_trailer(&pair, "spec: apply PR-0001", &id, &[]);
        true
    };
    let outcome = specengine_cli::reject(
        &pair.env(&pair.main),
        &specengine_cli::Globals::default(),
        &specengine_cli::RejectRequest {
            id: id.clone(),
            reason: "Dropped.".to_owned(),
            now: common::proposal::LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut consent,
    );
    assert_eq!(asked.len(), 1, "{asked:?}");
    let landed = pair.rev(&pair.linked, "t1");
    let reason = refused(&outcome, "landed during the prompt");
    assert_eq!(
        reason,
        names_it(&id, &landed, "", &format!("changes {PATH}, other.txt"))
    );
    assert_eq!(pair.events().len(), events, "no event");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "open");
    assert!(stored.decision_note.is_none());
}

/// trebase: a manual edit of another line of the section on `t1`, then
/// `approve` rebases the proposal (a merge) and a pre-commit hook stages
/// another file: step 10 leaves it `approved`. Split into the path's commit
/// (the trailer kept) and the other file's: `review` notes the split
/// commit, `approve` completes it with no prompt — the commit's text is
/// the rebased merge, not the proposal's new text, and still carries it.
/// M: the content check as literal `new_text` (accepted deviation 1).
#[test]
fn a_rebased_applys_split_commit_completes() {
    let pair = Pair::new("pa-content-rebase", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    replace(
        &pair.linked,
        PATH,
        "is not a reference",
        "is never a reference",
    );
    pair.commit_all(&pair.linked, "manual edit of the section");
    hook(
        &pair,
        "#!/bin/sh\necho generated > generated.txt\ngit add generated.txt\n",
    );
    let reason = refused(&pair.approve(&pair.main, &id), "two paths at step 10");
    assert!(reason.contains("(step 10)"), "{reason}");
    remove_hook(&pair);
    let held = pair.proposal(&id);
    assert_eq!(held.status.as_str(), "approved");

    pair.git.git(&pair.linked, &["reset", "-q", "HEAD~"]);
    pair.git.git(
        &pair.linked,
        &["commit", "-q", "-C", "ORIG_HEAD", "--", PATH],
    );
    let split = pair.rev(&pair.linked, "t1");
    pair.git.git(&pair.linked, &["add", "generated.txt"]);
    pair.git
        .git(&pair.linked, &["commit", "-q", "-m", "the generated file"]);
    let committed = String::from_utf8(
        pair.git
            .git(&pair.linked, &["show", &format!("{split}:{PATH}")]),
    )
    .unwrap();
    assert!(
        committed.contains(TO) && committed.contains("is never a reference"),
        "the split commit holds the rebased merge: {committed}"
    );
    assert_ne!(
        pair.node(&pair.linked, TARGET).text,
        held.new_text,
        "the span is not the proposal's literal new text"
    );

    let review = pair.review_ok(&pair.main, &id);
    let note = format!(
        "its commit {split} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert!(
        review.document.notes.contains(&note),
        "{:?}",
        review.document.notes
    );
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty());
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(split.as_str()));
    assert_eq!(stored.decided_by, held.decided_by);
}

/// The commit's own `specengine.toml` decides the parse (accepted deviation
/// 5): removed on `t1` before the trailer commit (which changes only the
/// path to the proposal's text) and from the worktree, the commit's tree
/// has none and the one read now (the recorded root's) cannot be read, so
/// it does not complete the proposal; broken there (not TOML), the reason names
/// `<commit>:specengine.toml`. Either way `review` shows no completion
/// note and `reject` is refused naming the commit and why. M: the content
/// check removed (the commit completes).
#[test]
fn a_trailer_commit_whose_config_cannot_be_read_does_not_complete() {
    for broken in [false, true] {
        let label = if broken {
            "pa-content-badcfg"
        } else {
            "pa-content-nocfg"
        };
        let pair = Pair::new(label, "spec-a");
        let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
        if broken {
            write(&pair.linked, "specengine.toml", "[ids\nnot toml\n");
            pair.commit_all(&pair.linked, "break the config");
        } else {
            pair.git.git(&pair.linked, &["rm", "-q", "specengine.toml"]);
            pair.git
                .git(&pair.linked, &["commit", "-q", "-m", "drop the config"]);
        }
        replace(&pair.linked, PATH, FROM, TO);
        let commit = commit_with_trailer(&pair, "spec: apply PR-0001", &id, &["--", PATH]);

        let review = pair.review_ok(&pair.main, &id);
        assert!(
            !review
                .document
                .notes
                .iter()
                .any(|line| line.contains("completes it")),
            "{label}: {:?}",
            review.document.notes
        );

        let before = pair.state();
        let events = pair.events().len();
        let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Not wanted.", true);
        let reason = refused(&outcome, label);
        let head = format!(
            "`{id}` has the commit {commit} on `t1` with the trailer `Proposal: {id}`, which does \
             not complete it ("
        );
        assert!(reason.starts_with(&head), "{label}: {reason}");
        // Not in the commit's tree: the config read now (the recorded
        // root's), which is gone from the worktree too (iteration 5).
        let why = if broken {
            format!("{commit}:specengine.toml")
        } else {
            format!(
                "its tree has no `specengine.toml`, and the one read now: cannot read \
                 {}/specengine.toml: ",
                pair.linked.display()
            )
        };
        assert!(reason.contains(&why), "{label}: {reason}");
        assert!(reason.ends_with(&completes_when(&id)), "{label}: {reason}");
        assert!(questions.is_empty(), "{label}: {questions:?}");
        assert_eq!(pair.state(), before, "{label}: nothing written");
        assert_eq!(pair.events().len(), events, "{label}: no event");
    }
}
