//! docs/features/rev-discipline.md RD-4 and TE-2 (completion and reject's
//! lookup, "Editorial refresh": "Completion and reject's lookup take the
//! path alone, or with `spec.lock` and `Lock-mode: editorial`. Completion
//! accepts step 5's text with or without the raise (Q12)"), through the
//! library over scratch repositories of `fixtures/code-a` with its golden
//! lock (an editorial apply's two-path commit) and `fixtures/spec-a` (a
//! meaning apply's raised commit, a hand commit without the raise).
//!
//! An apply interrupted after its commit is made as AC-16 of
//! docs/features/proposal-apply.md makes it: the row set back by SQL
//! (`approved` with no commit, or `open`); one interrupted before its
//! commit by a failing `pre-commit` hook.

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_common;
mod verify_common;

use std::process::{Command, Stdio};

use common::proposal::{Pair, edit, sqlite3};
use common::{read_text, replace};
use lock_common::LockRepo;
use rev_common::{At, approve_at, proposed, reject_at};
use specengine_cli::{Globals, Message, ShowRequest, show};
use specengine_store::{ProposalQueue as _, SqliteQueue};
use verify_common::golden;

const COUNTER_MD: &str = "docs/spec/counter.md";
const PIECE: &str = "PIECE-COUNTER";
const MEANING: (&str, &str) = ("grows one step at a time", "grows a single step at a time");
const SPRINT: &str = "docs/spec/movement/sprint.md";
const EMPTY: &str = "EDGE-SPRINT-EMPTY";
const EMPTY_LINE: (&str, &str) = ("the sprint ends;", "the sprint ends at once;");

fn at(repo: &LockRepo) -> At<'_> {
    At::new(&repo.root, &repo.home, repo.git_env(&repo.root))
}

fn main_at(pair: &Pair) -> At<'_> {
    At::new(&pair.main, &pair.home, pair.git_env(&pair.main))
}

fn linked_at(pair: &Pair) -> At<'_> {
    At::new(&pair.linked, &pair.home, pair.git_env(&pair.linked))
}

/// The code-a queue's database.
fn db(repo: &LockRepo) -> std::path::PathBuf {
    common::data_dir(&repo.home).join("code-a.db")
}

/// `sqlite3 <db> <sql>`; it must succeed.
fn sql(db: &std::path::Path, statement: &str) {
    let output = Command::new(sqlite3())
        .arg(db)
        .arg(statement)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statement}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `id` set back to `approved` with no commit: an apply interrupted after
/// its commit, before its record (proposal-apply AC-16).
fn interrupted(db: &std::path::Path, id: &str) {
    sql(
        db,
        &format!(
            "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = '{id}';"
        ),
    );
}

/// `id` set back to `open`: its commit made by hand or by a run that never
/// recorded its decision.
fn reopened(db: &std::path::Path, id: &str) {
    sql(
        db,
        &format!(
            "UPDATE proposals SET status = 'open', applied_commit = NULL, decided_by = NULL, \
             decided_at = NULL, decision_note = NULL WHERE id = '{id}';"
        ),
    );
}

fn status(db: &std::path::Path, slug: &str, id: &str) -> (String, Option<String>) {
    let queue = SqliteQueue::open(db, slug).expect("the queue");
    let proposal = queue.get(id).expect("readable").expect("the proposal");
    (proposal.status.as_str().to_owned(), proposal.applied_commit)
}

/// Proposes `PIECE-COUNTER`'s span with [`MEANING`]: its ID.
fn propose_piece(repo: &LockRepo) -> String {
    let outcome = show(
        &repo.env(&repo.root),
        &Globals::default(),
        &ShowRequest {
            reference: PIECE.to_owned(),
            links: false,
            archive: false,
            bindings: false,
            git: repo.git_env(&repo.root),
        },
    )
    .expect("show");
    let node = &outcome.nodes[0];
    proposed(
        &at(repo),
        PIECE,
        &node.span_hash,
        &edit(&node.text, MEANING.0, MEANING.1),
    )
}

/// The completion note of `id` by `commit` on `branch`.
fn completed_note(id: &str, commit: &str, branch: &str) -> String {
    format!("`{id}` completed by its commit {commit} on `{branch}`; no new commit")
}

fn noted(messages: &[Message], note: &str) -> bool {
    messages
        .iter()
        .any(|message| matches!(message, Message::Note(text) if text == note))
}

// ------------------------------------------------------------- RD-4, TE-2

/// RD-4, TE-2: an editorial apply's commit (the text and `spec.lock`,
/// `Lock-mode: editorial`) completes its proposal when the apply was
/// interrupted after the commit: `approved` -> `applied` with no question
/// and no new commit; `open` -> `applied` after the `complete` question.
/// Interrupted before the commit (a failing hook): restored, reopened,
/// and the next approve applies it in one commit.
/// M: the completion's lookup taking the path alone.
#[test]
fn interrupted_editorial_apply_completes() {
    let repo = golden("rd4-editorial");
    let id = propose_piece(&repo);
    approve_at(&at(&repo), &id, true, true).applied("the editorial apply");
    let commit = repo.head();
    assert_eq!(repo.changed(&commit).len(), 2, "text and lock");

    interrupted(&db(&repo), &id);
    let asked = approve_at(&at(&repo), &id, false, true);
    let outcome = asked.applied("completion of approved");
    assert!(
        asked.questions.is_empty(),
        "no question: {:?}",
        asked.questions
    );
    assert!(
        noted(&outcome.messages, &completed_note(&id, &commit, "main")),
        "{:?}",
        outcome.messages
    );
    assert_eq!(repo.head(), commit, "no new commit");
    assert_eq!(
        status(&db(&repo), "code-a", &id),
        ("applied".to_owned(), Some(commit.clone()))
    );

    reopened(&db(&repo), &id);
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("completion of open");
    assert_eq!(
        asked.question(),
        format!(
            "complete {id} by its commit {commit} on main in {}? [y/N]",
            repo.root.display()
        )
    );
    assert_eq!(repo.head(), commit, "no new commit");
    assert_eq!(status(&db(&repo), "code-a", &id).0, "applied");

    // Interrupted before the commit: the hook refuses it.
    let repo = golden("rd4-editorial-before");
    let id = propose_piece(&repo);
    repo.hook("pre-commit", "exit 1");
    let before = repo.state();
    approve_at(&at(&repo), &id, true, true).refused("the hook refuses the commit");
    assert_eq!(repo.state(), before);
    assert_eq!(status(&db(&repo), "code-a", &id).0, "open");
    std::fs::remove_file(repo.root.join(".git/hooks/pre-commit")).unwrap();
    approve_at(&at(&repo), &id, true, true).applied("applied after all");
    let head = repo.head();
    assert_eq!(repo.parents(&head), [before.head.clone().unwrap()]);
    let mut changed = repo.changed(&head);
    changed.sort();
    assert_eq!(changed, [COUNTER_MD, "spec.lock"]);
}

/// RD-4, TE-2: a meaning apply's commit carrying the raise completes its
/// proposal after an interruption (no question, no new commit), and reject
/// is refused naming it. M: completion comparing the text as spliced only.
#[test]
fn interrupted_meaning_apply_completes_with_the_raise() {
    let pair = Pair::new("rd4-meaning", "spec-a");
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    let id = proposed(
        &linked_at(&pair),
        EMPTY,
        &hash,
        &edit(&text, EMPTY_LINE.0, EMPTY_LINE.1),
    );
    approve_at(&main_at(&pair), &id, false, true).applied("the meaning apply");
    let commit = pair.rev(&pair.linked, "t1");
    assert!(read_text(&pair.linked, SPRINT).contains(".playtest rev=2}"));

    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = '{id}';"
    ));
    let rejected = reject_at(&main_at(&pair), &id);
    let reason = rejected.refused("its commit is in history");
    assert!(
        reason.contains(&format!("`{id}` has its commit {commit} on `t1`")),
        "{reason}"
    );
    assert!(rejected.questions.is_empty(), "refused before the question");
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let outcome = asked.applied("completion");
    assert!(asked.questions.is_empty());
    assert!(
        noted(&outcome.messages, &completed_note(&id, &commit, "t1")),
        "{:?}",
        outcome.messages
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));

    // A rebased meaning apply (its merge not idempotent on its own blob,
    // docs/canon/proposal-completion.md "Completion"): only step 5's text
    // on its parent with the raise completes it.
    let pair = Pair::new("rd4-meaning-rebased", "spec-a");
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    let id = proposed(
        &linked_at(&pair),
        EMPTY,
        &hash,
        &edit(
            &text,
            "Q-031.\n",
            "Q-031.\n\nPlaytest note: stop at once.\n",
        ),
    );
    replace(
        &pair.linked,
        SPRINT,
        "the sprint ends;",
        "the sprint ends now;",
    );
    pair.git
        .git(&pair.linked, &["commit", "-q", "-am", "owner edit"]);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("the rebased meaning apply");
    assert!(
        asked
            .question()
            .ends_with("(rebases; raises EDGE-SPRINT-EMPTY 1 -> 2)? [y/N]")
    );
    let commit = pair.rev(&pair.linked, "t1");
    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = '{id}';"
    ));
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let outcome = asked.applied("completion of the rebased apply");
    assert!(asked.questions.is_empty());
    assert!(
        noted(&outcome.messages, &completed_note(&id, &commit, "t1")),
        "{:?}",
        outcome.messages
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    assert_eq!(
        read_text(&pair.linked, SPRINT)
            .matches("Playtest note")
            .count(),
        1,
        "never merged again"
    );
}

/// RD-4: reject's lookup finds an editorial apply's two-path commit (the
/// text and `spec.lock`, `Lock-mode: editorial`) as the proposal's own:
/// reject refused, exit 1, naming it and `spec approve`, nothing changed.
/// M: reject's lookup taking the path alone.
#[test]
fn reject_refuses_with_a_two_path_apply_commit() {
    let repo = golden("rd4-reject");
    let id = propose_piece(&repo);
    approve_at(&at(&repo), &id, true, true).applied("the editorial apply");
    let commit = repo.head();
    for (label, set_back) in [
        ("approved", interrupted as fn(&std::path::Path, &str)),
        ("open", reopened as fn(&std::path::Path, &str)),
    ] {
        set_back(&db(&repo), &id);
        let before = repo.state();
        let rejected = reject_at(&at(&repo), &id);
        let reason = rejected.refused(label);
        assert!(
            reason.contains(&format!(
                "`{id}` has its commit {commit} on `main`: a proposal whose commit is in history \
                 is never rejected; `spec approve {id}` completes it"
            )),
            "{label}: {reason}"
        );
        assert!(rejected.questions.is_empty(), "{label}");
        assert_eq!(repo.state(), before, "{label}");
        assert_eq!(status(&db(&repo), "code-a", &id).0, label);
    }
}

/// RD-4 (Q12): an apply commit made before 4b-1, without the raise (the
/// proposal's text as spliced, by hand here, with its `Proposal:`
/// trailer), completes the proposal: `open` after the `complete` question,
/// `approved` with none; no new commit, the file not raised afterwards.
/// M: the raise required.
#[test]
fn unraised_apply_commit_completes() {
    for state in ["open", "approved"] {
        let pair = Pair::new(&format!("rd4-unraised-{state}"), "spec-a");
        let (hash, text) = pair.span(&pair.linked, EMPTY);
        let id = proposed(
            &linked_at(&pair),
            EMPTY,
            &hash,
            &edit(&text, EMPTY_LINE.0, EMPTY_LINE.1),
        );
        replace(&pair.linked, SPRINT, EMPTY_LINE.0, EMPTY_LINE.1);
        pair.git.git(
            &pair.linked,
            &[
                "commit",
                "-q",
                "-m",
                &format!("spec: apply {id}"),
                "-m",
                &format!("Proposal: {id}"),
                "--",
                SPRINT,
            ],
        );
        let commit = pair.rev(&pair.linked, "t1");
        let file = read_text(&pair.linked, SPRINT);
        assert!(!file.contains(" rev=2}"), "no raise in the hand commit");
        if state == "approved" {
            pair.sql(&format!(
                "UPDATE proposals SET status = 'approved', decided_by = 'Ann Owner \
                 <ann@example.org>', decided_at = '2026-10-06T07:00:00Z' WHERE id = '{id}';"
            ));
        }
        let asked = approve_at(&main_at(&pair), &id, false, true);
        let outcome = asked.applied(state);
        if state == "open" {
            assert_eq!(
                asked.question(),
                format!(
                    "complete {id} by its commit {commit} on t1 in {}? [y/N]",
                    pair.linked.display()
                )
            );
        } else {
            assert!(asked.questions.is_empty(), "{:?}", asked.questions);
        }
        assert!(
            noted(&outcome.messages, &completed_note(&id, &commit, "t1")),
            "{state}: {:?}",
            outcome.messages
        );
        assert_eq!(
            pair.rev(&pair.linked, "t1"),
            commit,
            "{state}: no new commit"
        );
        assert_eq!(read_text(&pair.linked, SPRINT), file, "{state}: not raised");
        let stored = pair.proposal(&id);
        assert_eq!(stored.status.as_str(), "applied", "{state}");
        assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    }

    // A rebased apply made before 4b-1: the merge written without the
    // raise, where merging again on the commit's own blob is not that blob
    // (docs/canon/proposal-completion.md "Completion"): only step 5's text
    // on its parent, as spliced, completes it.
    let pair = Pair::new("rd4-unraised-rebased", "spec-a");
    let paragraph = ("Q-031.\n", "Q-031.\n\nPlaytest note: stop at once.\n");
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    let id = proposed(
        &linked_at(&pair),
        EMPTY,
        &hash,
        &edit(&text, paragraph.0, paragraph.1),
    );
    replace(
        &pair.linked,
        SPRINT,
        "the sprint ends;",
        "the sprint ends now;",
    );
    pair.git
        .git(&pair.linked, &["commit", "-q", "-am", "owner edit"]);
    let proposal = pair.proposal(&id);
    let (_, current) = pair.span(&pair.linked, EMPTY);
    let dir = pair.scratch.dir("merge");
    for (name, side) in [
        ("current", &current),
        ("base", &proposal.base_text),
        ("new", &proposal.new_text),
    ] {
        std::fs::write(dir.join(name), format!("{side}\n")).unwrap();
    }
    let merged = String::from_utf8(
        pair.git
            .git(&dir, &["merge-file", "-p", "current", "base", "new"]),
    )
    .unwrap();
    let merged = merged.strip_suffix('\n').unwrap_or(&merged).to_owned();
    std::fs::write(dir.join("current"), format!("{merged}\n")).unwrap();
    let again = String::from_utf8(
        pair.git
            .git(&dir, &["merge-file", "-p", "current", "base", "new"]),
    )
    .unwrap();
    assert_ne!(
        again.strip_suffix('\n').unwrap_or(&again),
        merged,
        "the premise: the merge is not idempotent"
    );
    replace(&pair.linked, SPRINT, &current, &merged);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            &format!("spec: apply {id}"),
            "-m",
            &format!("Proposal: {id}"),
            "--",
            SPRINT,
        ],
    );
    let commit = pair.rev(&pair.linked, "t1");
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let outcome = asked.applied("an unraised rebased commit");
    assert!(
        noted(&outcome.messages, &completed_note(&id, &commit, "t1")),
        "{:?}",
        outcome.messages
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    assert_eq!(
        read_text(&pair.linked, SPRINT)
            .matches("Playtest note")
            .count(),
        1,
        "never merged again"
    );
}
