//! docs/features/proposal-apply.md, "Apply" 7–10 as iteration 3 rules them
//! (review of iteration 2, MAJOR and nit): a run reopens a proposal only
//! when it set `approved` itself at step 7, and only while the row is still
//! that state; a refusal before its own hold logs one `apply_failed` and
//! leaves the state as another run (or the owner) left it; step 9 counts a
//! failed `git commit` as made only when the new tip carries this
//! proposal's `Proposal:` trailer; step 10 records this run's verified
//! commit as `applied` even when its hold was reopened meanwhile.
//!
//! "Library approve": the current directory the main worktree, the
//! proposal raised in the linked worktree on `t1` (`common::proposal`).

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::proposal::{DECIDER, LATER, NOW, Pair, refused, sqlite3, with_vars};
use common::{read, read_text, write};
use specengine_cli::{ApproveRequest, CliError, Exit, Globals, Message, ProposalOutcome, approve};
use specengine_store::GitEnv;

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

/// Library `approve` of `id` in `cwd` at [`LATER`] with `note` and
/// consent yes: the outcome and the questions.
fn approve_noted(
    pair: &Pair,
    id: &str,
    note: Option<&str>,
    git: GitEnv,
) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = approve(
        &pair.env(&pair.main),
        &Globals::default(),
        &ApproveRequest {
            id: id.to_owned(),
            note: note.map(str::to_owned),
            now: LATER.to_owned(),
            git,
        },
        &mut consent,
    );
    (outcome, questions)
}

/// `id` set `approved` by SQL (another run's hold, or one a stopped run
/// left), decided at [`NOW`].
fn approved_by_hand(pair: &Pair, id: &str) {
    pair.sql(&format!(
        "update proposals set status = 'approved', decided_by = '{DECIDER}', \
         decided_at = '{NOW}', decision_note = 'by hand', updated_at = '{NOW}' where id = '{id}'"
    ));
}

/// The step of a refusal: exit 1's reason or exit 2's message.
fn reason_of(outcome: &Result<ProposalOutcome, CliError>) -> String {
    match outcome {
        Ok(outcome) => {
            assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
            outcome.refusal.clone().expect("a refusal")
        }
        Err(error) => error.message.clone(),
    }
}

/// An `approved` proposal this run did not approve (set by SQL), refused
/// before step 7 at step 2 (`HEAD` on another branch, exit 2), step 3 (the
/// target dirty), step 4 (the target moved to another file) and step 5 (a
/// conflicting edit on `t1`): each logs one `apply_failed` with its step
/// and leaves it `approved`, its decision and `updated_at` as they were
/// (review of iteration 2: every such refusal reopened it). Undone, the
/// same proposals apply. M: a refusal before the run's own hold reopens.
#[test]
fn refusals_before_the_runs_own_hold_leave_approved_as_it_is() {
    let pair = Pair::new("pa-hold-rule", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let moved = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", "immediately", "at once");
    approved_by_hand(&pair, &id);
    approved_by_hand(&pair, &moved);
    let held = pair.proposal(&id);
    let held_moved = pair.proposal(&moved);
    let base = pair.rev(&pair.linked, "t1");
    let mut steps = Vec::new();
    let mut attempt = |label: &str, which: &str, step: u64| {
        let (outcome, questions) =
            pair.approve_answer(&pair.main, which, true, pair.git_env(&pair.main));
        let reason = reason_of(&outcome);
        assert!(
            reason.contains(&format!("(step {step})")),
            "{label}: {reason}"
        );
        assert!(questions.is_empty(), "{label}: refused before the prompt");
        if which == id {
            assert_eq!(pair.proposal(&id), held, "{label}: approved as it was");
            steps.push(ev("apply_failed", Some(step)));
        } else {
            assert_eq!(
                pair.proposal(&moved),
                held_moved,
                "{label}: approved as it was"
            );
        }
    };

    pair.git
        .git(&pair.linked, &["checkout", "-q", "-b", "other"]);
    attempt("HEAD on another branch", &id, 2);
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);

    let mut dirty = read(&pair.linked, PATH);
    dirty.extend_from_slice(b"\nA line not committed.\n");
    write(&pair.linked, PATH, &dirty);
    attempt("a dirty target", &id, 3);
    pair.git.git(&pair.linked, &["checkout", "--", PATH]);

    let stamina = "docs/spec/movement/stamina.md";
    let text = read_text(&pair.linked, stamina);
    let cut = text.find("## Depletion").unwrap();
    write(&pair.linked, stamina, &text[..cut]);
    write(
        &pair.linked,
        "docs/spec/movement/zero.md",
        format!("---\nclass: canon\n---\n\n# Zero\n\n{}", &text[cut..]),
    );
    pair.commit_all(&pair.linked, "moved");
    attempt("the target moved", &moved, 4);
    pair.git
        .git(&pair.linked, &["reset", "-q", "--hard", &base]);

    common::replace(&pair.linked, PATH, FROM, "the sprint halts;");
    pair.commit_all(&pair.linked, "a conflicting edit");
    attempt("a conflict", &id, 5);
    pair.git
        .git(&pair.linked, &["reset", "-q", "--hard", &base]);

    let mut want = vec![ev("created", None)];
    want.extend(steps);
    assert_eq!(pair.events_of(&id), want);
    assert_eq!(
        pair.events_of(&moved),
        [ev("created", None), ev("apply_failed", Some(4))]
    );

    pair.approve_ok(&pair.main, &id);
    assert_eq!(pair.proposal(&id).status.as_str(), "applied");
}

/// Writes the release file on drop: a panicking test never leaves the
/// waiting hook behind.
struct Release(PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = fs::write(&self.0, b"go");
    }
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Run A approves; its pre-commit hook waits. Run B approves the same ID
/// meanwhile: B reads `approved` (A's hold), finds the target written by
/// A (dirty) and is refused at step 3, exit 1, no prompt, and leaves A's
/// hold `approved` (review of iteration 2, h1: B reopened it and A's step
/// 10 then failed "PR-0001 is open" with its commit made). Released, A
/// commits and records `applied`. Events: created, approved,
/// apply_failed (3), applied. M: a refusal before the run's own hold
/// reopens.
#[test]
fn another_runs_refusal_during_a_slow_hook_never_reopens_the_hold() {
    let pair = Pair::new("pa-hold-h1", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    let started = pair.scratch.join("hook-started");
    let go = pair.scratch.join("hook-go");
    hook(
        &pair,
        &format!(
            "#!/bin/sh\ntouch '{}'\ni=0\nwhile [ ! -e '{}' ]; do\n  i=$((i+1))\n  \
             [ $i -gt 600 ] && exit 1\n  sleep 0.1\ndone\nexit 0\n",
            started.display(),
            go.display()
        ),
    );
    std::thread::scope(|scope| {
        let release = Release(go.clone());
        let a =
            scope.spawn(|| pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main)));
        wait_for(&started);
        assert_eq!(pair.proposal(&id).status.as_str(), "approved", "A holds it");
        let (b, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        let reason = refused(&b, "B during A's hook");
        assert!(
            reason.contains("(step 3)") && reason.contains("uncommitted changes"),
            "{reason}"
        );
        assert!(questions.is_empty(), "{questions:?}");
        let during = pair.proposal(&id);
        assert_eq!(during.status.as_str(), "approved", "A's hold kept");
        assert_eq!(during.decided_by.as_deref(), Some(DECIDER));
        drop(release);
        let (a, questions) = a.join().expect("run A");
        let a = a.unwrap_or_else(|error| panic!("A: {error}"));
        assert_eq!(a.exit(), Exit::Answered, "A: {a:?}");
        assert_eq!(questions.len(), 1);
    });
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(pair.rev(&pair.linked, "t1^"), base, "A's one commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(3)),
            ev("applied", None)
        ]
    );
}

/// A pre-commit hook that moves `t1` by plumbing to a commit of its own
/// (no `Proposal:` trailer) and exits 1: step 9 judges the commit not
/// made — the target's bytes restored (the worktree clean against the new
/// tip), this run's own hold reopened (`open`), one `apply_failed` at step
/// 9 whose reason names the foreign tip "which has no `Proposal: PR-0001`
/// trailer (not this apply's commit)" (review of iteration 2, nit: the
/// bytes stayed uncommitted, the proposal `approved`). Without the hook the
/// proposal applies on top of that tip. M: "made" judged by the branch's
/// movement only.
#[test]
fn a_foreign_tip_after_a_failed_commit_is_restored_and_reopened() {
    let pair = Pair::new("pa-hold-plumbing", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let original = read(&pair.linked, PATH);
    let linked = pair.linked.display().to_string();
    hook(
        &pair,
        &format!(
            "#!/bin/sh\nunset GIT_INDEX_FILE\nc=$(git -C '{linked}' commit-tree -p HEAD -m \
             \"the hook's own commit\" 'HEAD^{{tree}}')\ngit -C '{linked}' update-ref \
             refs/heads/t1 \"$c\"\nexit 1\n"
        ),
    );
    let reason = refused(&pair.approve(&pair.main, &id), "a foreign tip");
    let tip = pair.rev(&pair.linked, "t1");
    assert_eq!(
        pair.git_text(&pair.linked, &["log", "-1", "--format=%s", "t1"]),
        "the hook's own commit"
    );
    assert!(reason.contains("(step 9)"), "{reason}");
    assert!(
        reason.contains(&format!(
            "the commit of `{PATH}` failed, its bytes restored"
        )),
        "{reason}"
    );
    assert!(
        reason.contains(&format!(
            "`t1` moved to {tip}, which has no `Proposal: {id}` trailer (not this apply's commit)"
        )),
        "{reason}"
    );
    assert_eq!(read(&pair.linked, PATH), original, "restored");
    assert_eq!(pair.state().linked_status, "", "clean against the new tip");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "open", "its own hold reopened");
    assert!(stored.decided_by.is_none());
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(9))
        ]
    );

    remove_hook(&pair);
    pair.approve_ok(&pair.main, &id);
    assert_eq!(
        pair.rev(&pair.linked, "t1^"),
        tip,
        "on top of the hook's commit"
    );
    assert_eq!(pair.proposal(&id).status.as_str(), "applied");
}

/// A pre-commit hook that moves `HEAD` to `side` and a `git` (a `PATH`
/// wrapper) that fails after making the commit: the commit carries this
/// proposal's trailer on `HEAD`'s new branch, so step 9 warns "the commit
/// reported a failure, yet HEAD moved to <sha>" and restores nothing (the
/// worktree clean on `side`), and step 10 names that commit and `HEAD`'s
/// branch: `approved` stays (review of iteration 2, nit: the restore
/// overwrote committed bytes). M: the trailer test dropped (a restore over
/// the commit).
#[test]
fn a_failed_commit_that_went_to_heads_new_branch_is_judged_not_restored() {
    let pair = Pair::new("pa-hold-head-wrapper", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    pair.git.git(&pair.linked, &["branch", "side"]);
    hook(&pair, "#!/bin/sh\ngit symbolic-ref HEAD refs/heads/side\n");
    let wrapper = pair.scratch.dir("wrapper");
    let real = pair.git.git_program().to_path_buf();
    fs::write(
        wrapper.join("git"),
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = commit ]; then\n    '{real}' \
             \"$@\" || exit $?\n    echo 'wrapper: failed after the commit' >&2\n    exit 1\n  \
             fi\ndone\nexec '{real}' \"$@\"\n",
            real = real.display()
        ),
    )
    .unwrap();
    fs::set_permissions(wrapper.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = pair.git.var("PATH").expect("the sandbox PATH").to_owned();
    let mut wrapped = wrapper.into_os_string();
    wrapped.push(":");
    wrapped.push(&path);
    let git = with_vars(&pair.git_env(&pair.main), &[("PATH", OsStr::new(&wrapped))]);

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, git);
    assert_eq!(questions.len(), 1);
    let reason = refused(&outcome, "HEAD moved, git failed");
    let side = pair.rev(&pair.linked, "side");
    assert_ne!(side, base, "the commit went to side");
    assert_eq!(pair.rev(&pair.linked, "t1"), base, "t1 did not move");
    let messages = &outcome.as_ref().unwrap().messages;
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::Warning(text)
            if text.contains(&format!("the commit reported a failure, yet HEAD moved to {side}"))
                && text.contains("wrapper: failed after the commit"))),
        "{messages:?}"
    );
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(
        reason.contains(&side) && reason.contains("HEAD on `side`"),
        "{reason}"
    );
    assert!(reason.contains("stays approved"), "{reason}");
    assert_eq!(
        read(&pair.linked, PATH),
        pair.git
            .git(&pair.main, &["show", &format!("{side}:{PATH}")]),
        "not restored over the commit"
    );
    assert_eq!(pair.state().linked_status, "", "clean on side");
    assert_eq!(pair.proposal(&id).status.as_str(), "approved");
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("apply_failed", Some(10))
        ]
    );
}

/// The row set back to `open` (by `sqlite3` from the pre-commit hook)
/// while this run commits: step 10 verifies its own commit and records
/// `applied` with this run's decision (decided by the committer at now,
/// the `--note` kept) in one transaction from `open`, events `approved`
/// and `applied` after the first `approved` (review of iteration 2: step
/// 10 failed "PR-0001 is open" with the commit made). M: step 10 records
/// only from `approved`.
#[test]
fn step_10_records_its_own_commit_after_the_hold_was_reopened() {
    let pair = Pair::new("pa-hold-h10", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let base = pair.rev(&pair.linked, "t1");
    hook(
        &pair,
        &format!(
            "#!/bin/sh\n'{}' '{}' \"update proposals set status = 'open', decided_by = NULL, \
             decided_at = NULL, decision_note = NULL, updated_at = '2026-10-05T00:00:00Z' \
             where id = '{id}'\"\n",
            sqlite3().to_string_lossy(),
            pair.db().display()
        ),
    );
    let (outcome, questions) = approve_noted(&pair, &id, Some("my note"), pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(pair.rev(&pair.linked, "t1^"), base);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(stored.decided_at.as_deref(), Some(LATER));
    assert_eq!(stored.decision_note.as_deref(), Some("my note"));
    assert_eq!(
        pair.events_of(&id),
        [
            ev("created", None),
            ev("approved", None),
            ev("approved", None),
            ev("applied", None)
        ]
    );
}
