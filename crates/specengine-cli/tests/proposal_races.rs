//! docs/features/proposal-apply.md, the apply against what can happen
//! while the owner is asked and while git commits (review of iteration 1,
//! fixed in iteration 2), through the library with a consent callback
//! that acts before it answers:
//!
//! - the place re-checked at step 8: a branch switched, a commit made or a
//!   cherry-pick started during the prompt → refused at step 8, nothing
//!   written or committed, `open`;
//! - step 7 a compare-and-set on the state this run read: an approve of the
//!   same ID run inside the callback wins; the outer run is refused at
//!   step 7 and never reopens what the inner run left;
//! - step 10 names the commit `HEAD` points to when the branch did not
//!   move (a pre-commit hook that moves `HEAD`); a cherry-pick of that
//!   commit then completes the proposal;
//! - a `git` that fails after making the commit (a `PATH` wrapper): the
//!   moved branch is judged by step 10, never restored over.
//!
//! "Library approve": consent yes, the current directory the main
//! worktree, the proposal raised in the linked worktree on `t1`
//! (`common::proposal`).

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use common::proposal::{DECIDER, LATER, Pair, refused, with_vars};
use common::{read, write};
use specengine_cli::{ApproveRequest, CliError, Exit, Globals, Message, ProposalOutcome, approve};
use specengine_store::GitEnv;

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";

/// Library `approve` of `id` in `cwd` at [`LATER`] with `consent`.
fn approve_with(
    pair: &Pair,
    cwd: &Path,
    id: &str,
    git: GitEnv,
    consent: &mut dyn FnMut(&str) -> bool,
) -> Result<ProposalOutcome, CliError> {
    approve(
        &pair.env(cwd),
        &Globals::default(),
        &ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: LATER.to_owned(),
            git,
        },
        consent,
    )
}

/// The `(type, step)` events of `id` as `&str` pairs, for comparing.
fn events(pair: &Pair, id: &str) -> Vec<(String, Option<u64>)> {
    pair.events_of(id)
}

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

/// The place re-checked right before the write (review of iteration 1,
/// MAJOR: `git checkout -b t2` during the prompt put the apply commit on
/// `t2`): a branch switch, a commit on `t1` (another file) and a
/// cherry-pick state, each made by the consent callback, are refused at
/// step 8 naming what changed; the target's bytes, `t1` and the new branch
/// unchanged; `open`, no decision; one `apply_failed` at step 8. Undone,
/// the same proposal applies. M: the step-8 place re-check removed (the
/// commit lands on `t2`, or on the new `HEAD`, and step 10 refuses).
#[test]
fn the_place_changed_during_the_prompt_is_refused_at_step_8() {
    type Act = fn(&Pair) -> String;
    let cases: [(&str, Act, Act, &str); 3] = [
        (
            "branch switch",
            |pair| {
                pair.git.git(&pair.linked, &["checkout", "-q", "-b", "t2"]);
                String::new()
            },
            |pair| {
                pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
                String::new()
            },
            "moved to `t2` from `t1`",
        ),
        (
            "commit",
            |pair| {
                write(&pair.linked, "notes.txt", "made while asked\n");
                pair.commit_all(&pair.linked, "made while asked");
                pair.rev(&pair.linked, "HEAD")
            },
            |_| String::new(),
            "`t1` moved from ",
        ),
        (
            "cherry-pick state",
            |pair| {
                let path = pair.git_text(
                    &pair.linked,
                    &["rev-parse", "--git-path", "CHERRY_PICK_HEAD"],
                );
                let path = pair.linked.join(path);
                fs::write(&path, format!("{}\n", pair.rev(&pair.linked, "HEAD"))).unwrap();
                String::new()
            },
            |pair| {
                let path = pair.git_text(
                    &pair.linked,
                    &["rev-parse", "--git-path", "CHERRY_PICK_HEAD"],
                );
                fs::remove_file(pair.linked.join(path)).unwrap();
                String::new()
            },
            "a cherry-pick started in ",
        ),
    ];
    for (label, act, undo, named) in cases {
        let pair = Pair::new("pa-step8", "spec-a");
        let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
        let base = pair.rev(&pair.linked, "t1");
        let bytes = read(&pair.linked, PATH);
        let mut made = String::new();
        let mut asked = 0;
        let outcome = approve_with(
            &pair,
            &pair.main,
            &id,
            pair.git_env(&pair.main),
            &mut |_question| {
                asked += 1;
                made = act(&pair);
                true
            },
        );
        assert_eq!(asked, 1, "{label}");
        let reason = refused(&outcome, label);
        assert!(reason.contains("(step 8)"), "{label}: {reason}");
        assert!(reason.contains(named), "{label}: {reason}");
        assert!(reason.contains("nothing written"), "{label}: {reason}");
        assert_eq!(read(&pair.linked, PATH), bytes, "{label}: nothing written");
        let t1 = pair.rev(&pair.linked, "t1");
        if made.is_empty() {
            assert_eq!(t1, base, "{label}: no commit on t1");
        } else {
            assert_eq!(t1, made, "{label}: only the callback's commit on t1");
            assert!(
                reason.contains(&made),
                "{label}: names the new HEAD: {reason}"
            );
        }
        if label == "branch switch" {
            assert_eq!(pair.rev(&pair.linked, "t2"), base, "nothing on t2");
        }
        let stored = pair.proposal(&id);
        assert_eq!(stored.status.as_str(), "open", "{label}");
        assert!(stored.decided_by.is_none(), "{label}");
        assert_eq!(
            events(&pair, &id),
            [
                ev("created", None),
                ev("approved", None),
                ev("apply_failed", Some(8))
            ],
            "{label}"
        );

        undo(&pair);
        let outcome = pair.approve_ok(&pair.main, &id);
        let head = pair.rev(&pair.linked, "t1");
        assert_eq!(
            pair.rev(&pair.linked, "t1^"),
            pair.rev(&pair.linked, if made.is_empty() { &base } else { &made }),
            "{label}: applied on the branch as it is now"
        );
        assert_eq!(
            outcome.document.applied_commit.as_deref(),
            Some(head.as_str())
        );
    }
}

/// Step 7 as a compare-and-set (review of iteration 1, minor: two
/// approvals both passed step 7, and the refused one reopened what the
/// other held). An approve of the same ID run inside the consent callback:
///
/// - applies → the outer run is refused at step 7 naming the commit,
///   logs nothing ("no failure logged" note), one commit, `applied`;
/// - is refused at its own step 8 (its consent changed the file) and
///   reopens its own hold → the outer run (file restored) is refused at
///   step 7 ("changed since this run read it"), no commit, `open`;
/// - stops at step 10 (a hook moved `HEAD`) and leaves it `approved` →
///   the outer run is refused at step 7 and does not reopen it.
///
/// M: the compare in `approve_from` dropped (the second case commits);
/// `reopen_from` ignoring the hold (the third case reopens).
#[test]
fn a_nested_approve_of_the_same_id_wins_and_the_outer_run_is_refused_at_step_7() {
    // 1. The inner run applies.
    let pair = Pair::new("pa-nested-applies", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    let mut inner = None;
    let outcome = approve_with(
        &pair,
        &pair.main,
        &id,
        pair.git_env(&pair.main),
        &mut |_| {
            inner = Some(pair.approve(&pair.main, &id));
            true
        },
    );
    let inner = inner.expect("asked").expect("the inner run");
    assert_eq!(inner.exit(), Exit::Answered, "{inner:?}");
    let head = pair.rev(&pair.linked, "t1");
    assert_ne!(head, base);
    let reason = refused(&outcome, "outer after an applying inner run");
    assert!(reason.contains("(step 7)"), "{reason}");
    assert!(reason.contains(&head), "names the inner commit: {reason}");
    let outcome = outcome.unwrap();
    assert!(
        outcome.messages.iter().any(
            |message| matches!(message, Message::Note(text) if text.contains("no failure logged"))
        ),
        "{outcome:?}"
    );
    assert_eq!(outcome.document.status.as_deref(), Some("applied"));
    assert_eq!(
        pair.git_text(&pair.main, &["rev-list", "--count", &format!("{base}..t1")]),
        "1",
        "one commit"
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(
        events(&pair, &id),
        [
            ev("created", None),
            ev("approved", None),
            ev("applied", None)
        ],
        "the outer run logs nothing"
    );

    // 2. The inner run is refused at its step 8 and reopens its own hold.
    let pair = Pair::new("pa-nested-reopens", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    let bytes = read(&pair.linked, PATH);
    let mut inner = None;
    let outcome = approve_with(
        &pair,
        &pair.main,
        &id,
        pair.git_env(&pair.main),
        &mut |_| {
            inner = Some(approve_with(
                &pair,
                &pair.main,
                &id,
                pair.git_env(&pair.main),
                &mut |_| {
                    let mut changed = bytes.clone();
                    changed.extend_from_slice(b"\nChanged while asked.\n");
                    write(&pair.linked, PATH, changed);
                    true
                },
            ));
            write(&pair.linked, PATH, &bytes);
            true
        },
    );
    let inner = inner.expect("asked");
    let inner_reason = refused(&inner, "the inner run");
    assert!(inner_reason.contains("(step 8)"), "{inner_reason}");
    let reason = refused(&outcome, "outer after a reopening inner run");
    assert!(reason.contains("(step 7)"), "{reason}");
    assert!(
        reason.contains("changed since this run read it"),
        "{reason}"
    );
    assert!(reason.contains("nothing written"), "{reason}");
    assert_eq!(pair.rev(&pair.linked, "t1"), base, "no commit");
    assert_eq!(read(&pair.linked, PATH), bytes);
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
    assert_eq!(
        events(&pair, &id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(8)),
            ev("apply_failed", Some(7))
        ]
    );

    // 3. The inner run stops at step 10: `approved` stays.
    let pair = Pair::new("pa-nested-stuck", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    pair.git.git(&pair.linked, &["branch", "side"]);
    hook(&pair, "#!/bin/sh\ngit symbolic-ref HEAD refs/heads/side\n");
    let mut inner = None;
    let outcome = approve_with(
        &pair,
        &pair.main,
        &id,
        pair.git_env(&pair.main),
        &mut |_| {
            inner = Some(pair.approve(&pair.main, &id));
            true
        },
    );
    let inner_reason = refused(&inner.expect("asked"), "the inner run");
    assert!(inner_reason.contains("(step 10)"), "{inner_reason}");
    let reason = refused(&outcome, "outer after a stuck inner run");
    assert!(reason.contains("(step 7)"), "{reason}");
    assert!(
        reason.contains("changed since this run read it"),
        "{reason}"
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), base, "no commit on t1");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "approved", "not reopened");
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(
        events(&pair, &id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10)),
            ev("apply_failed", Some(7))
        ]
    );
}

/// A pre-commit hook that moves `HEAD` to `side`: the commit lands on
/// `side`, `t1` does not move; step 10 names that commit and `HEAD`'s
/// branch, the proposal stays `approved` (review of iteration 1: verify
/// read `t1` and reported its old commit). Cherry-picked onto `t1`, the
/// next approve completes it: `applied` with the cherry-pick, no prompt, no
/// new commit. M: verify reports the branch only (no commit named).
#[test]
fn a_hook_that_moves_head_is_named_at_step_10_and_a_cherry_pick_completes() {
    let pair = Pair::new("pa-head-hook", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    pair.git.git(&pair.linked, &["branch", "side"]);
    hook(&pair, "#!/bin/sh\ngit symbolic-ref HEAD refs/heads/side\n");
    let outcome = pair.approve(&pair.main, &id);
    let reason = refused(&outcome, "HEAD moved by the hook");
    let side = pair.rev(&pair.linked, "side");
    assert_ne!(side, base, "the commit went to side");
    assert_eq!(pair.rev(&pair.linked, "t1"), base, "t1 did not move");
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(reason.contains(&side), "names the commit: {reason}");
    assert!(
        reason.contains("HEAD on `side`"),
        "names HEAD's branch: {reason}"
    );
    assert!(reason.contains("stays approved"), "{reason}");
    assert_eq!(pair.proposal(&id).status.as_str(), "approved");
    assert_eq!(
        events(&pair, &id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10))
        ]
    );

    remove_hook(&pair);
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
    pair.git.git(&pair.linked, &["cherry-pick", "side"]);
    let picked = pair.rev(&pair.linked, "t1");
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        questions.is_empty(),
        "a completion asks nothing: {questions:?}"
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), picked, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(picked.as_str()));
    assert_eq!(events(&pair, &id).last().unwrap(), &ev("applied", None));
}

/// A `git` (a `PATH` wrapper) that makes the commit and then exits 128:
/// the branch moved, so step 10 judges the commit — `applied`, a warning
/// naming the moved branch and git's text, the file not restored (it holds
/// the committed bytes, the worktree clean), no `apply_failed` (review of
/// iteration 1, nit: the restore overwrote committed content).
#[test]
fn a_git_that_fails_after_committing_is_verified_not_restored() {
    let pair = Pair::new("pa-git-wrapper", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    let wrapper = pair.scratch.dir("wrapper");
    let marker = pair.scratch.join("wrapper-failed");
    let real = pair.git.git_program().to_path_buf();
    fs::write(
        wrapper.join("git"),
        format!(
            "#!/bin/sh\n'{}' \"$@\"\ncode=$?\nif [ $code -eq 0 ]; then\n  for arg in \"$@\"; do\n    \
             if [ \"$arg\" = commit ]; then\n      echo 'wrapper: index write failed after the \
             commit' >&2\n      touch '{}'\n      exit 128\n    fi\n  done\nfi\nexit $code\n",
            real.display(),
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(wrapper.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = pair
        .git
        .vars()
        .into_iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value)
        .expect("the sandbox PATH");
    let mut wrapped = wrapper.into_os_string();
    wrapped.push(":");
    wrapped.push(&path);
    let git = with_vars(&pair.git_env(&pair.main), &[("PATH", OsStr::new(&wrapped))]);

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, git);
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert!(marker.exists(), "the wrapper failed the commit");
    assert_eq!(questions.len(), 1);
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(pair.rev(&pair.linked, "t1^"), base, "one commit");
    let warned = outcome.messages.iter().any(|message| {
        matches!(message, Message::Warning(text)
            if text.contains(&format!("`t1` moved to {head}"))
                && text.contains("wrapper: index write failed"))
    });
    assert!(warned, "{:?}", outcome.messages);
    let committed = pair
        .git
        .git(&pair.main, &["show", &format!("{head}:{PATH}")]);
    assert_eq!(read(&pair.linked, PATH), committed, "not restored");
    assert_eq!(pair.state().linked_status, "", "the worktree is clean");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(
        events(&pair, &id),
        [
            ev("created", None),
            ev("approved", None),
            ev("applied", None)
        ]
    );
}
