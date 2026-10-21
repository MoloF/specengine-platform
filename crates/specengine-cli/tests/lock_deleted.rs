//! docs/features/spec-verify.md TE-11 (RD-11 item 4, slice 3's carried
//! review item): a lock commit that deletes `spec.lock` fails step 10's
//! check, exit 1 naming the commit, the failed check and the undo; never
//! "made but cannot be verified".
//!
//! docs/features/tested-evidence.md (8b-1) RD-6 ("Else accept's checks"):
//! the evidence commit of `spec verify --tests` is checked alike.

mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use lock_common::LockRepo;
use specengine_cli::Exit;

/// TE-11: accept over an existing lock; a pre-commit hook drops
/// `spec.lock` from the commit, which therefore deletes it: exit 1 naming
/// the commit, the check it fails (`it has no spec.lock`) and `git reset
/// --soft <commit>^`; the commit stays as git made it. M: absent read as
/// unreadable ("made but cannot be verified").
#[test]
fn lock_commit_deleting_the_lock_fails_a_check() {
    let repo = LockRepo::code_a("lock-deleted");
    repo.accept_ok("LAW-BOUND");
    assert!(repo.lock().is_some(), "an existing lock");
    repo.replace(
        "docs/spec/counter.md",
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    repo.commit_all("an editorial change");
    repo.hook("pre-commit", "git rm --cached -q spec.lock");
    let head = repo.head();
    let attempt = repo.editorial("LAW-BOUND", true);
    assert_eq!(attempt.prompts.len(), 1, "one question");
    let outcome = attempt
        .outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("exit {:?}: {error}", error.exit));
    let after = repo.head();
    assert_ne!(after, head, "the scenario: git made a commit");
    assert_eq!(repo.parents(&after), vec![head.clone()]);
    assert_eq!(
        repo.changed(&after),
        ["spec.lock"],
        "the commit deletes the lock"
    );
    assert_eq!(
        repo.git_text(&["ls-tree", "--name-only", &after, "--", "spec.lock"]),
        "",
        "no spec.lock in the commit"
    );
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    assert_eq!(outcome.exit().code(), 1);
    let refusal = outcome.refusal.clone().unwrap_or_default();
    assert!(refusal.contains(&after), "names the commit: {refusal}");
    assert!(
        refusal.contains("is not this run's spec.lock alone"),
        "a failed check: {refusal}"
    );
    assert!(
        refusal.contains("it has no spec.lock"),
        "the check: {refusal}"
    );
    assert!(
        refusal.contains(&format!("git reset --soft {after}^")),
        "the undo: {refusal}"
    );
    assert!(!refusal.contains("cannot be verified"), "{refusal}");
}

/// RD-6 (the evidence commit: accept's four checks; M: absent read as
/// unreadable, or a `committed` line): after a pass, KILN-RAMP's code
/// edited and committed, a pre-commit hook drops `spec.lock` from the
/// evidence commit, which therefore deletes it: exit 1 with every verdict
/// kept, `lock.refusal` naming the commit, `it has no spec.lock` and `git
/// reset --soft <commit>^`, `lock.commit` and the event's `lock_commit`
/// null; the commit stays as git made it.
#[test]
fn evidence_commit_deleting_the_lock_fails_a_check() {
    let kiln = verify_tests_common::Kiln::new("lock-deleted-tests");
    tested_kiln::pass(&kiln);
    kiln.repo.replace(
        verify_tests_common::RAMP,
        "(now + rate).min(peak)",
        "now.saturating_add(rate).min(peak)",
    );
    kiln.repo.commit_all("the ramp saturates");
    kiln.repo.hook("pre-commit", "git rm --cached -q spec.lock");
    let head = kiln.repo.head();
    let run = kiln.run();
    run.code(1);
    assert_eq!(
        run.node("KILN-RAMP")["verdict"],
        "verified",
        "the verdict kept"
    );
    let after = kiln.repo.head();
    assert_ne!(after, head, "the scenario: git made a commit");
    assert_eq!(kiln.repo.parents(&after), vec![head]);
    assert_eq!(kiln.repo.changed(&after), ["spec.lock"]);
    let document = run.json();
    let refusal = document["lock"]["refusal"]
        .as_str()
        .unwrap_or_else(|| panic!("a refusal: {}", run.show()))
        .to_owned();
    assert!(refusal.contains(&after), "names the commit: {refusal}");
    assert!(refusal.contains("it has no spec.lock"), "{refusal}");
    assert!(
        refusal.contains(&format!("git reset --soft {after}^")),
        "the undo: {refusal}"
    );
    assert_eq!(document["lock"]["commit"], serde_json::Value::Null);
    assert!(
        !run.lines()
            .iter()
            .any(|line| line.starts_with("committed ")),
        "{}",
        run.show()
    );
    let events = kiln.events();
    let last: serde_json::Value =
        serde_json::from_str(events.last().expect("the event")).expect("JSON");
    assert_eq!(last["lock_commit"], serde_json::Value::Null);
}
