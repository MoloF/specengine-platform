//! docs/features/proposal-apply.md, "Idempotence" as iteration 5 rules it
//! (review of iteration 4, MAJOR: `git merge-file` is not idempotent, so a
//! rebased apply's own commit never completed and the next approve merged
//! the proposal a second time; known limit: a commit tree without
//! `specengine.toml` never completed):
//!
//! - a trailer commit completes when it has one parent, changes only the
//!   path, and its span is step 5's text on its FIRST PARENT's blob (what
//!   the apply wrote on top of it); step 5's text on its own blob stays a
//!   second way to accept;
//! - step 5 refuses a text to write when a trailer commit on the branch
//!   carries the proposal by that parent check but does not complete it
//!   (two paths), naming it with why and the shared hint: never merged
//!   again;
//! - `review` with a completing commit skips steps 2–6: one note "its
//!   commit <sha> is on `t1`: `spec approve PR-0001` completes it; no new
//!   apply is needed", preview `unavailable`;
//! - a commit whose tree has no `<root_rel>/specengine.toml` is parsed under
//!   the config read now (the recorded root's; the current project's when
//!   the worktree is gone).
//!
//! Iteration 6 (n4): the base commit pruned after a rebase, the branch's
//! whole history is read, so the rebased apply's commit still completes.
//!
//! The rebase (trials tr, tr3): the proposal inserts a paragraph after the
//! first; the owner then edits the first paragraph on `t1`; the apply
//! rebases, and merging the proposal again into that result is not the
//! result (asserted: the premise of these tests). Through the library with
//! a consent callback; the current directory the main worktree, the
//! proposal raised in the linked worktree on `t1` (`common::proposal`).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::proposal::{DECIDER, Pair, refused};
use common::{read_text, replace, write};
use specengine_cli::{Exit, Message, Preview};

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";
/// The proposal: a paragraph inserted after the first one.
const PARAGRAPH: (&str, &str) = ("Q-031.\n", "Q-031.\n\nPlaytest note: stop at once.\n");
/// The owner's edit of the first paragraph, after the proposal was raised.
const OWNER: (&str, &str) = ("the sprint ends;", "the sprint ends now;");
const NOTE_LINE: &str = "Playtest note: stop at once.";

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

/// The shared hint of steps 5, 10 and `reject`.
fn completes_when(id: &str) -> String {
    format!(
        "a commit on `t1` with the trailer `Proposal: {id}`, one parent, changing only `{PATH}` \
         to the proposal's text completes it (`spec approve {id}`)"
    )
}

/// `review`'s one note for a completing commit.
fn completes_note(id: &str, commit: &str) -> String {
    format!(
        "its commit {commit} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    )
}

/// How many times the inserted paragraph is in the linked worktree's file.
fn paragraphs(pair: &Pair) -> usize {
    read_text(&pair.linked, PATH).matches(NOTE_LINE).count()
}

/// The proposal raised on `t1`, then the owner's edit of the first
/// paragraph committed there: the apply rebases. The proposal's ID.
fn rebasing(pair: &Pair) -> String {
    let id = pair.propose_edit(&pair.linked, TARGET, PARAGRAPH.0, PARAGRAPH.1);
    replace(&pair.linked, PATH, OWNER.0, OWNER.1);
    pair.git
        .git(&pair.linked, &["commit", "-q", "-am", "owner edit"]);
    id
}

/// The premise: `git merge-file` of the proposal (base → new) into the
/// span as the rebased apply left it is not that span (the paragraph would
/// be inserted again).
fn assert_not_idempotent(pair: &Pair, id: &str) {
    let proposal = pair.proposal(id);
    let span = pair.node(&pair.linked, TARGET).text;
    let dir = pair.scratch.join("merge");
    fs::create_dir_all(&dir).unwrap();
    write(&dir, "current", &span);
    write(&dir, "base", &proposal.base_text);
    write(&dir, "new", &proposal.new_text);
    let merged = pair
        .git
        .git(&dir, &["merge-file", "-p", "current", "base", "new"]);
    let merged = String::from_utf8(merged).expect("UTF-8 merge");
    assert!(span.contains(NOTE_LINE), "the apply inserted it: {span}");
    assert_ne!(merged, span, "the merge run again changes the rebased span");
}

/// tr (review of iteration 4, MAJOR): the rebased apply's commit on `t1`,
/// then the row set back to `approved` with no commit (an interrupted
/// apply, AC-16 by SQL). `review`: exactly one note, that the commit
/// completes it, preview `unavailable` (steps 2–6 not run). `approve`:
/// completes it with no prompt — `applied` with that commit, the decision
/// of step 7 kept, the note "completed by its commit", no second commit,
/// the paragraph once. M: the parent-blob check removed (only the commit's
/// own blob: the merge on it is not the span, so no completion and a
/// second merge).
#[test]
fn a_non_idempotent_rebased_applys_commit_completes_without_a_second_commit() {
    let pair = Pair::new("pa-parent-tr", "spec-a");
    let id = rebasing(&pair);
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        questions.len() == 1 && questions[0].ends_with(" (rebases)? [y/N]"),
        "the apply rebases: {questions:?}"
    );
    let commit = pair.rev(&pair.linked, "t1");
    let applied = pair.proposal(&id);
    assert_eq!(applied.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(paragraphs(&pair), 1);
    assert_not_idempotent(&pair, &id);

    pair.sql(&format!(
        "update proposals set status = 'approved', applied_commit = NULL where id = '{id}'"
    ));
    let held = pair.proposal(&id);
    assert_eq!(held.status.as_str(), "approved");

    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.notes,
        [completes_note(&id, &commit)],
        "one note, no step note"
    );
    assert_eq!(review.document.preview, Some(Preview::Unavailable));

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "an approved completion asks nothing");
    assert!(
        outcome.messages.iter().any(|message| matches!(message,
            Message::Note(text)
                if *text == format!("`{id}` completed by its commit {commit} on `t1`; no new commit"))),
        "{:?}",
        outcome.messages
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no second commit");
    assert_eq!(paragraphs(&pair), 1, "the paragraph once");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(stored.decided_by, held.decided_by, "step 7's decision");
    assert_eq!(stored.decided_at, held.decided_at);
    assert_eq!(pair.events_of(&id).last(), Some(&ev("applied", None)));
}

/// n4 (iteration 6, review of iteration 5, FIX: a base commit pruned after
/// a rebase made the lookup "cannot tell", which step 5 ignores when it has
/// a text to write, so the next approve merged the proposal a second
/// time): work on `t1` before the proposal (its base commit `t1`'s own),
/// the rebased apply committed, the row set back to `approved` (AC-16 by
/// SQL), then `main` moved on, `t1` rebased onto it, every reflog expired
/// and the old commits pruned (the base commit not in the repository,
/// asserted; merging again still not idempotent). The branch's whole
/// history is read: `review` gives exactly one note, that the rebased
/// commit completes it, preview `unavailable`; `reject` is refused before
/// any prompt naming it; `approve` completes it with no prompt — no new
/// commit, the paragraph once, `applied` with that commit, step 7's
/// decision kept. M: the whole-branch fallback removed (a "(rebases)?"
/// prompt and a second paragraph).
#[test]
fn a_rebased_applys_commit_completes_after_its_base_commit_is_pruned() {
    let pair = Pair::new("pa-parent-pruned", "spec-a");
    write(&pair.linked, "w.txt", "work\n");
    pair.git.git(&pair.linked, &["add", "w.txt"]);
    pair.git
        .git(&pair.linked, &["commit", "-q", "-m", "t1 work"]);
    let work = pair.rev(&pair.linked, "t1");
    let id = rebasing(&pair);
    assert_eq!(pair.proposal(&id).place.base_commit, work, "t1's own base");
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        questions.len() == 1 && questions[0].ends_with(" (rebases)? [y/N]"),
        "the apply rebases: {questions:?}"
    );
    pair.sql(&format!(
        "update proposals set status = 'approved', applied_commit = NULL where id = '{id}'"
    ));
    let held = pair.proposal(&id);
    assert_eq!(held.status.as_str(), "approved");

    write(&pair.main, "m.txt", "main moves\n");
    pair.git.git(&pair.main, &["add", "m.txt"]);
    pair.git
        .git(&pair.main, &["commit", "-q", "-m", "main moves"]);
    pair.git.git(&pair.linked, &["rebase", "-q", "main"]);
    pair.git
        .git(&pair.main, &["reflog", "expire", "--expire=now", "--all"]);
    pair.git.git(&pair.main, &["gc", "-q", "--prune=now"]);
    let commit = pair.rev(&pair.linked, "t1");
    let pruned = pair
        .git
        .git_output(&pair.main, &["cat-file", "-e", &work], &[]);
    assert!(!pruned.status.success(), "the base commit is pruned");
    assert_eq!(paragraphs(&pair), 1);
    assert_not_idempotent(&pair, &id);

    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.notes,
        [completes_note(&id, &commit)],
        "one note, no \"cannot tell\""
    );
    assert_eq!(review.document.preview, Some(Preview::Unavailable));

    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "No.", true);
    assert_eq!(
        refused(&outcome, "reject, the base pruned"),
        format!(
            "`{id}` has its commit {commit} on `t1`: a proposal whose commit is in history is \
             never rejected; `spec approve {id}` completes it"
        )
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.events().len(), events, "no event");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        questions.is_empty(),
        "an approved completion asks nothing: {questions:?}"
    );
    assert!(
        outcome.messages.iter().any(|message| matches!(message,
            Message::Note(text)
                if *text == format!("`{id}` completed by its commit {commit} on `t1`; no new commit"))),
        "{:?}",
        outcome.messages
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no second commit");
    assert_eq!(paragraphs(&pair), 1, "the paragraph once");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(stored.decided_by, held.decided_by, "step 7's decision");
    assert_eq!(stored.decided_at, held.decided_at);
    assert_eq!(pair.events_of(&id).last(), Some(&ev("applied", None)));
}

/// tr3: the same rebase, a pre-commit hook staging `generated.txt` into the
/// apply commit (two paths): step 10 leaves it `approved`. `review`: one
/// note "not applicable now (step 5): the commit <sha> on `t1` carries
/// `Proposal: PR-0001` and applied the proposal on top of its parent, but
/// changes <path>, generated.txt: a new apply would apply it again; <hint>;
/// no new apply". `approve`: refused exit 1 at step 5 with that text, no
/// prompt, no re-merge (the tip and the paragraph count as they were), the
/// row untouched, one `apply_failed` (5). `reject`: refused before the
/// prompt naming the commit. Split into the path's commit (the trailer
/// kept) and the other file's: `review` notes the split commit, `approve`
/// completes it with no prompt and no new commit. M: the `carries` refusal
/// at step 5 removed (a second merge is committed).
#[test]
fn a_carrying_two_path_commit_refuses_step_5_without_a_re_merge_and_its_split_completes() {
    let pair = Pair::new("pa-parent-tr3", "spec-a");
    let id = rebasing(&pair);
    hook(
        &pair,
        "#!/bin/sh\necho generated > generated.txt\ngit add generated.txt\n",
    );
    let reason = refused(&pair.approve(&pair.main, &id), "two paths at step 10");
    assert!(reason.contains("(step 10)"), "{reason}");
    remove_hook(&pair);
    let two = pair.rev(&pair.linked, "t1");
    let held = pair.proposal(&id);
    assert_eq!(held.status.as_str(), "approved");
    assert_eq!(paragraphs(&pair), 1);
    assert_not_idempotent(&pair, &id);

    let carries = format!(
        "the commit {two} on `t1` carries `Proposal: {id}` and applied the proposal on top of its \
         parent, but changes {PATH}, generated.txt: a new apply would apply it again; {}; no new \
         apply",
        completes_when(&id)
    );
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.notes,
        [format!("not applicable now (step 5): {carries}")]
    );
    assert_eq!(review.document.preview, Some(Preview::Unavailable));

    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "approve over a carrying two-path commit");
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert!(reason.contains("(step 5)"), "{reason}");
    assert!(reason.contains(&carries), "{reason}");
    assert_eq!(pair.rev(&pair.linked, "t1"), two, "nothing committed");
    assert_eq!(paragraphs(&pair), 1, "not merged again");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.proposal(&id), held, "approved, untouched");
    assert_eq!(pair.events().len(), events + 1);
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&ev("apply_failed", Some(5)))
    );

    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Not wanted.", true);
    let reason = refused(&outcome, "reject over a carrying two-path commit");
    assert_eq!(
        reason,
        format!(
            "`{id}` has the commit {two} on `t1` with the trailer `Proposal: {id}`, which does \
             not complete it (changes {PATH}, generated.txt): a proposal with a commit in history \
             is never rejected; {}",
            completes_when(&id)
        )
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");

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

    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(review.document.notes, [completes_note(&id, &split)]);
    assert_eq!(review.document.preview, Some(Preview::Unavailable));
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "an approved completion asks nothing");
    assert_eq!(pair.rev(&pair.linked, "t1"), tip, "no new commit");
    assert_eq!(paragraphs(&pair), 1);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(split.as_str()));
    assert_eq!(stored.decided_by, held.decided_by, "step 7's decision");
}

/// tu (review of iteration 4, known limit): `specengine.toml` not committed
/// on `t1` (untracked there and excluded, still on disk), the proposal's
/// text committed by hand with its trailer: the commit's tree (and its
/// parent's) has no `specengine.toml`, so it is parsed under the config
/// read now. Worktree there: `review` notes the commit, `reject` is refused
/// naming it, `approve` asks `complete … in <t1>?` and records `applied`.
/// Worktree removed (the branch kept): from the main worktree the current
/// project's config is read; the prompt names the main worktree and the
/// completion is recorded. M: the config fallback removed (the commit
/// "cannot be read": no completion, reject refused for ever).
#[test]
fn a_commit_tree_without_its_config_completes_under_the_config_read_now() {
    for removed in [false, true] {
        let label = if removed {
            "pa-parent-tu-gone"
        } else {
            "pa-parent-tu"
        };
        let pair = Pair::new(label, "spec-a");
        pair.git
            .git(&pair.linked, &["rm", "-q", "--cached", "specengine.toml"]);
        let exclude = pair.main.join(".git/info/exclude");
        fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        fs::write(&exclude, "specengine.toml\n").unwrap();
        pair.git
            .git(&pair.linked, &["commit", "-q", "-m", "untrack the config"]);
        assert!(pair.linked.join("specengine.toml").is_file());
        assert_eq!(
            pair.git_text(&pair.linked, &["status", "--porcelain"]),
            "",
            "{label}: the config is ignored"
        );

        let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
        replace(&pair.linked, PATH, FROM, TO);
        pair.git.git(
            &pair.linked,
            &[
                "commit",
                "-q",
                "-m",
                "spec: apply",
                "-m",
                &format!("Proposal: {id}"),
                "--",
                PATH,
            ],
        );
        let commit = pair.rev(&pair.linked, "t1");
        let probe = pair.git.git_output(
            &pair.linked,
            &["cat-file", "-e", &format!("{commit}:specengine.toml")],
            &[],
        );
        assert!(!probe.status.success(), "{label}: not in the commit's tree");
        let read_in = if removed {
            pair.git.git(
                &pair.main,
                &[
                    "worktree",
                    "remove",
                    "--force",
                    pair.linked.to_str().unwrap(),
                ],
            );
            assert!(!pair.linked.exists());
            pair.main.clone()
        } else {
            pair.linked.clone()
        };

        let review = pair.review_ok(&pair.main, &id);
        assert_eq!(
            review.document.notes,
            [completes_note(&id, &commit)],
            "{label}"
        );
        assert_eq!(
            review.document.preview,
            Some(Preview::Unavailable),
            "{label}"
        );

        let events = pair.events().len();
        let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Not wanted.", true);
        let reason = refused(&outcome, label);
        assert_eq!(
            reason,
            format!(
                "`{id}` has its commit {commit} on `t1`: a proposal whose commit is in history is \
                 never rejected; `spec approve {id}` completes it"
            ),
            "{label}"
        );
        assert!(questions.is_empty(), "{label}: {questions:?}");
        assert_eq!(pair.events().len(), events, "{label}: no event");

        let (outcome, questions) =
            pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        let outcome = outcome.unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{label}: {outcome:?}");
        assert_eq!(
            questions,
            [format!(
                "complete {id} by its commit {commit} on t1 in {}? [y/N]",
                read_in.display()
            )],
            "{label}"
        );
        let stored = pair.proposal(&id);
        assert_eq!(stored.status.as_str(), "applied", "{label}");
        assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
        assert_eq!(stored.decided_by.as_deref(), Some(DECIDER), "{label}");
    }
}
