//! docs/features/spec-lock.md TE-6 (`--editorial`), TE-8 (trailers, hooks,
//! a failing hook), RD-10, RD-11 and RD-15: the lock commit and what else
//! an accept writes, over scratch repositories of `fixtures/code-a`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;
mod lock_common;

use std::fs;

use common::git::Sandbox;
use common::{Scratch, copy_dir, data_dir, fixture};
use lock_common::{CLOCK, DECIDER, LockRepo, at, blocks, node_blocks, value};
use specengine_cli::{
    Exit, Globals, LockAcceptRequest, LockOutcome, LockPrompt, LockTrailer, Outcome, lock_accept,
    render_text,
};
use specengine_store::{ProposalFilter, ProposalQueue as _, SqliteQueue};

const COUNTER_MD: &str = "docs/spec/counter.md";
const COUNTER_RS: &str = "crates/tally/src/counter.rs";

/// PIECE-COUNTER's text changed at its revision (no raise), committed.
fn change_piece_text(repo: &LockRepo) {
    repo.replace(
        COUNTER_MD,
        "A count that grows one step at a time and never passes its cap.",
        "A count that grows by one step at a time and never passes its cap.",
    );
}

/// `git interpret-trailers --parse` over `commit`'s message.
fn trailers(repo: &LockRepo, commit: &str) -> String {
    let message = repo.message(commit);
    String::from_utf8(repo.git.git_stdin(
        &repo.root,
        &["interpret-trailers", "--parse"],
        message.as_bytes(),
    ))
    .expect("UTF-8 trailers")
}

// ------------------------------------------------------------------ TE-6

/// TE-6 at R: PIECE-COUNTER's text changed at rev 3: `--editorial` asks
/// once, sets the new `norm_hash` on its four entries and nothing else
/// (code hashes, `file_blob`, `commit`, `verified_*` kept), commits with
/// `Lock-mode: editorial`; a plain accept after it changes nothing.
#[test]
fn editorial_refreshes_the_spec_hash_at_r() {
    let repo = LockRepo::code_a("lock-ed-at-r");
    repo.accept_ok("PIECE-COUNTER");
    let old = repo.lock_text();
    let old_hash = repo.norm_hash("PIECE-COUNTER");
    change_piece_text(&repo);
    repo.commit_all("an editorial change");
    let new_hash = repo.norm_hash("PIECE-COUNTER");
    assert_ne!(old_hash, new_hash);
    // A plain accept refuses; the editorial one is the way.
    repo.accept("PIECE-COUNTER", true).refused("plain accept");
    let head = repo.head();
    let attempt = repo.editorial("PIECE-COUNTER", true);
    let outcome = attempt.committed("editorial");
    assert_eq!(
        attempt.question(),
        format!(
            "lock PIECE-COUNTER rev 3 (editorial: spec_hash of 4 entries){}",
            at(&head, "main", &repo.root)
        )
    );
    assert_eq!(
        attempt.rows(),
        "refreshed | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally struct_item\n\
refreshed | implements | crates/tally/src/lib.rs | module crates/tally\n\
refreshed | verifies | crates/tally/tests/bounds.rs | module crates/tally::test:bounds\n\
refreshed | configures | data/limits.ron | ron root\n"
    );
    assert_eq!(repo.lock_text(), old.replace(&old_hash, &new_hash));
    let commit = outcome.commit.clone().expect("a commit");
    assert_eq!(
        trailers(&repo, &commit),
        format!("Lock: PIECE-COUNTER\nLock-mode: editorial\nDecided-by: {DECIDER}\n")
    );
    assert_eq!(repo.changed(&commit), vec!["spec.lock"]);
    repo.accept("PIECE-COUNTER", true)
        .unchanged("plain accept after it");
}

/// RD-10 (M: `ast_hash` refreshed): the text and the bound code both
/// changed and committed: `--editorial` changes only `spec_hash`; every
/// `ast_hash`, `file_blob`, `commit`, `verified_at` stays the old one.
#[test]
fn editorial_keeps_code_hashes() {
    let repo = LockRepo::code_a("lock-ed-code");
    repo.accept_ok("PIECE-COUNTER");
    let old = repo.lock_text();
    let old_hash = repo.norm_hash("PIECE-COUNTER");
    change_piece_text(&repo);
    repo.replace(
        COUNTER_RS,
        "    count: u32,\n}",
        "    count: u32,\n    spare: u32,\n}",
    );
    repo.replace("data/limits.ron", "step: 2,", "step: 4,");
    repo.commit_all("text and code changed");
    let attempt = repo.attempt_at(&repo.root, "PIECE-COUNTER", true, lock_common::LATER, true);
    attempt.committed("editorial");
    let new_hash = repo.norm_hash("PIECE-COUNTER");
    let lock = repo.lock_text();
    assert_eq!(
        lock,
        old.replace(&old_hash, &new_hash),
        "only spec_hash changed"
    );
    for block in blocks(&lock) {
        assert_eq!(value(&block, "verified_at"), Some(lock_common::DAY));
    }
}

/// RD-10 (M: after a raise): LAW-BOUND raised from rev 2 to 3: no entry
/// at 3, `--editorial` is exit 1, no question, nothing changed.
#[test]
fn editorial_after_a_revision_raise_is_refused() {
    let repo = LockRepo::code_a("lock-ed-raise");
    repo.accept_ok("LAW-BOUND");
    repo.replace(COUNTER_MD, "{#LAW-BOUND rev=2}", "{#LAW-BOUND rev=3}");
    repo.commit_all("LAW-BOUND raised");
    let before = repo.state();
    let attempt = repo.editorial("LAW-BOUND", true);
    let refusal = attempt.refused("after a raise");
    assert!(attempt.prompts.is_empty(), "{:?}", attempt.prompts);
    assert!(refusal.contains("rev 3"), "{refusal}");
    assert_eq!(repo.state(), before);
}

/// TE-6: every entry at R matches: exit 0 `nothing to change`, no
/// question, the rows `unchanged`.
#[test]
fn editorial_with_everything_matching_changes_nothing() {
    let repo = LockRepo::code_a("lock-ed-match");
    repo.accept_ok("PIECE-COUNTER");
    let before = repo.state();
    let attempt = repo.editorial("PIECE-COUNTER", true);
    attempt.unchanged("all matching");
    assert!(
        attempt
            .rows()
            .lines()
            .all(|row| row.starts_with("unchanged | ")),
        "{}",
        attempt.rows()
    );
    assert_eq!(attempt.rows().lines().count(), 4);
    assert_eq!(repo.state(), before);
    let run = repo.spec_pty(
        &repo.root,
        &["lock", "accept", "PIECE-COUNTER", "--editorial"],
        None,
    );
    run.code(0);
    assert!(!run.asked, "{}", run.show());
    assert!(
        run.stdout.ends_with("nothing to change\n"),
        "{}",
        run.show()
    );
}

/// TE-6: `--editorial` walks no code: a modified and an untracked code
/// file do not refuse it, and stay as they were; a dirty node file does.
#[test]
fn editorial_ignores_dirty_code() {
    let repo = LockRepo::code_a("lock-ed-dirty");
    repo.accept_ok("PIECE-COUNTER");
    change_piece_text(&repo);
    repo.commit_all("an editorial change");
    repo.replace(
        COUNTER_RS,
        "    count: u32,\n}",
        "    count: u32,\n    spare: u32,\n}",
    );
    repo.write(
        "crates/tally/src/fresh.rs",
        "// @implements PIECE-COUNTER\npub fn fresh() {}\n",
    );
    let status = repo.porcelain();
    let attempt = repo.editorial("PIECE-COUNTER", true);
    let outcome = attempt.committed("dirty code");
    assert_eq!(repo.porcelain(), status, "the dirty code as it was");
    assert_eq!(
        repo.changed(outcome.commit.as_deref().expect("a commit")),
        vec!["spec.lock"]
    );
    // The node's own file dirty refuses.
    repo.replace(COUNTER_MD, "never passes its cap", "never passes the cap");
    let attempt = repo.editorial("PIECE-COUNTER", true);
    let refusal = attempt.refused("dirty node file");
    assert!(
        refusal.contains("`docs/spec/counter.md` modified"),
        "{refusal}"
    );
}

// ------------------------------------------------------------------ TE-8

/// TE-8: the lock commit's trailers, parsed by `git interpret-trailers
/// --parse`: `Lock`, `Lock-mode`, `Decided-by`, then a caller's own
/// (slice 4c's `Proposal:`), in that order; the subject `spec: lock <ID>`.
#[test]
fn lock_commit_trailers_parse() {
    let repo = LockRepo::code_a("lock-trailers");
    let outcome = repo.accept_ok("cap-check/PASS-01");
    let commit = outcome.commit.expect("a commit");
    assert_eq!(
        trailers(&repo, &commit),
        format!("Lock: cap-check/PASS-01\nLock-mode: accept\nDecided-by: {DECIDER}\n")
    );
    assert_eq!(
        repo.git_text(&["log", "-1", "--format=%s", &commit]),
        "spec: lock cap-check/PASS-01"
    );
    // A caller's trailer after the three.
    let mut consent = |_: &LockPrompt<'_>| true;
    let outcome = lock_accept(
        &repo.env(&repo.root),
        &Globals::default(),
        &LockAcceptRequest {
            reference: "AIM-01".to_owned(),
            editorial: false,
            now: CLOCK.to_owned(),
            git: repo.git_env(&repo.root),
        },
        &mut consent,
        &[LockTrailer {
            key: "Proposal".to_owned(),
            value: "PR-7".to_owned(),
        }],
    )
    .expect("accept through the alias");
    let commit = outcome.commit.expect("a commit");
    assert_eq!(
        trailers(&repo, &commit),
        format!("Lock: GOAL-01\nLock-mode: accept\nDecided-by: {DECIDER}\nProposal: PR-7\n")
    );
}

/// RD-11 (M: `--no-verify`), TE-8: the lock commit runs the repository's
/// hooks: `pre-commit`, `commit-msg` and `post-commit` each leave a line
/// in a file outside the repository.
#[test]
fn lock_commit_runs_hooks() {
    let repo = LockRepo::code_a("lock-hooks");
    let log = repo.scratch.join("hooks.log");
    for hook in ["pre-commit", "commit-msg", "post-commit"] {
        repo.hook(hook, &format!("echo {hook} >> '{}'", log.display()));
    }
    repo.accept_ok("PIECE-COUNTER");
    let ran = fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(ran, "pre-commit\ncommit-msg\npost-commit\n");
}

/// TE-8: a failing hook restores the bytes: a first lock's file and its
/// index entry are gone, an existing lock's bytes are the old ones; exit
/// 1, HEAD, status and index as they were. `pre-commit` and `commit-msg`
/// alike.
#[test]
fn a_failing_hook_restores_the_bytes() {
    for hook in ["pre-commit", "commit-msg"] {
        // A first lock.
        let repo = LockRepo::code_a("lock-hook-fail-first");
        repo.hook(hook, "exit 1");
        let before = repo.state();
        let attempt = repo.accept("PIECE-COUNTER", true);
        let refusal = attempt.refused(hook);
        assert!(refusal.contains("restored"), "{hook}: {refusal}");
        assert_eq!(attempt.prompts.len(), 1);
        assert!(repo.lock().is_none(), "{hook}: a new lock left behind");
        assert_eq!(
            repo.git_text(&["ls-files", "--", "spec.lock"]),
            "",
            "{hook}"
        );
        assert_eq!(repo.state(), before, "{hook}: a first lock");

        // An existing lock.
        let repo = LockRepo::code_a("lock-hook-fail-again");
        repo.accept_ok("PIECE-COUNTER");
        repo.hook(hook, "exit 1");
        let before = repo.state();
        let attempt = repo.accept("LAW-BOUND", true);
        attempt.refused(hook);
        assert_eq!(repo.state(), before, "{hook}: an existing lock");
    }
}

/// RD-11, TE-8 (code review iteration 1, major): a `pre-commit` hook that
/// stages another file (`git add NOTES.txt`, as `cargo fmt && git add -u`
/// would) sweeps it into the lock commit under `commit --only`. The commit
/// is then not "the lock alone": exit 1 naming the commit, never `exit 0`
/// `committed` (the commit is not rewritten; HEAD has moved).
#[test]
fn a_hook_that_stages_another_file_fails_the_accept() {
    let repo = LockRepo::code_a("lock-hook-stages");
    repo.write("NOTES.txt", "committed\n");
    repo.commit_all("notes");
    repo.write("NOTES.txt", "changed, not staged\n");
    repo.hook("pre-commit", "git add NOTES.txt");
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let outcome = attempt
        .outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("exit {:?}: {error}", error.exit));
    let after = repo.head();
    assert_ne!(after, head, "the scenario: git made a commit");
    assert!(
        repo.changed(&after).contains(&"NOTES.txt".to_owned()),
        "the scenario: the hook swept NOTES.txt in: {:?}",
        repo.changed(&after)
    );
    assert_eq!(
        outcome.exit(),
        Exit::NotFound,
        "a lock commit that is not the lock alone is reported as success: {outcome:?}"
    );
    let refusal = outcome.refusal.clone().unwrap_or_default();
    assert!(
        refusal.contains(&after),
        "the refusal names the commit {after}: {refusal}"
    );
    assert_undo_advice(&refusal, &after, &head);
    assert_eq!(outcome.commit.as_deref(), Some(after.as_str()));
}

/// Iteration 3 (M: advise `<old>`): a made commit's refusal undoes it with
/// `git reset --soft <commit>^`, never with the old HEAD's OID (which,
/// once another commit lands, would drop that one too).
fn assert_undo_advice(refusal: &str, commit: &str, old: &str) {
    assert!(
        refusal.contains(&format!("git reset --soft {commit}^")),
        "the undo names {commit}^: {refusal}"
    );
    assert!(
        !refusal.contains(&format!("git reset --soft {old}")),
        "the undo names the old HEAD: {refusal}"
    );
}

/// RD-11 (M: a staged path committed): other staged paths (a new file, a
/// spec file no input of the node) stay staged and out of the lock
/// commit; one parent, the old HEAD; only `spec.lock` changed.
#[test]
fn lock_commit_takes_only_the_lock() {
    let repo = LockRepo::code_a("lock-only");
    repo.write("NOTES.txt", "staged, not the lock's\n");
    repo.replace(
        "docs/records/PREM-001.md",
        "nothing guards it.",
        "nothing guards it at all.",
    );
    repo.git(&["add", "NOTES.txt", "docs/records/PREM-001.md"]);
    repo.write("UNSTAGED.txt", "untracked\n");
    let status = repo.porcelain();
    let staged = repo.git_text(&["diff", "--cached", "--name-only"]);
    let head = repo.head();
    let outcome = repo.accept_ok("PIECE-COUNTER");
    let commit = outcome.commit.expect("a commit");
    assert_eq!(repo.parents(&commit), vec![head]);
    assert_eq!(repo.changed(&commit), vec!["spec.lock"]);
    assert_eq!(repo.porcelain(), status, "the other paths as they were");
    assert_eq!(repo.git_text(&["diff", "--cached", "--name-only"]), staged);
    assert_eq!(
        repo.show_blob(&commit, "spec.lock"),
        repo.lock().expect("the lock"),
        "the committed lock is the worktree's"
    );
}

/// RD-15 (M: a queue event): an accept writes the data directory's index,
/// `spec.lock` and one commit: no proposal, no event; no other file under
/// the root; only `refs/heads/main` moves.
#[test]
fn lock_accept_writes_no_queue_row() {
    let repo = LockRepo::code_a("lock-queue");
    let before = repo.state();
    repo.accept_ok("PIECE-COUNTER");
    let after = repo.state();
    let mut changed: Vec<&String> = after
        .files
        .iter()
        .filter(|(path, bytes)| before.files.get(*path) != Some(bytes))
        .map(|(path, _)| path)
        .collect();
    changed.extend(
        before
            .files
            .keys()
            .filter(|path| !after.files.contains_key(*path)),
    );
    assert_eq!(changed, vec!["spec.lock"]);
    let refs_before: Vec<&str> = before.refs.lines().collect();
    let refs_after: Vec<&str> = after.refs.lines().collect();
    assert_eq!(refs_before.len(), refs_after.len());
    for (one, two) in refs_before.iter().zip(&refs_after) {
        if one.starts_with("refs/heads/main ") {
            assert_ne!(one, two);
        } else {
            assert_eq!(one, two);
        }
    }
    let db = data_dir(&repo.home).join("code-a.db");
    if db.exists() {
        let queue = SqliteQueue::open(&db, "code-a").expect("the queue opens");
        assert!(
            queue
                .list(&ProposalFilter::default())
                .expect("list")
                .is_empty()
        );
        assert!(queue.events().expect("events").is_empty());
    }
    let lock = repo.lock_text();
    assert_eq!(node_blocks(&lock, "PIECE-COUNTER").len(), 4);
}

// ------------------------------------------- step 10's check (iteration 2)

/// The stdout text the binary prints for `outcome`.
fn stdout_of(outcome: &LockOutcome) -> String {
    render_text(&Outcome::Lock(Box::new(outcome.clone())))
}

/// Step 10's check, "made, a check fails" (code review iteration 1, major):
/// a `commit-msg` hook rewrites the message, so the commit git makes has
/// no `Lock:` trailer naming the node. Exit 1 naming the commit and the
/// missing trailer; the commit stays (HEAD moved, nothing rewritten),
/// `outcome.commit` names it, stdout prints no `committed` line.
#[test]
fn a_commit_without_the_lock_trailer_fails_the_accept() {
    let repo = LockRepo::code_a("lock-hook-message");
    repo.hook("commit-msg", "printf 'rewritten by a hook\\n' > \"$1\"");
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let outcome = attempt
        .outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("exit {:?}: {error}", error.exit));
    let after = repo.head();
    assert_ne!(after, head, "the scenario: git made the commit");
    assert_eq!(repo.changed(&after), vec!["spec.lock"], "the lock alone");
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    let refusal = outcome.refusal.clone().unwrap_or_default();
    assert!(refusal.contains(&after), "names the commit: {refusal}");
    assert!(
        refusal.contains("Lock: PIECE-COUNTER"),
        "names the trailer: {refusal}"
    );
    assert_undo_advice(&refusal, &after, &head);
    assert_eq!(outcome.commit.as_deref(), Some(after.as_str()));
    assert!(
        !stdout_of(outcome).contains("committed"),
        "{}",
        stdout_of(outcome)
    );
}

/// Step 10's check, "made, HEAD did not move": a `post-commit` hook winds
/// HEAD back (`git reset --soft HEAD~1`), so git reports success and HEAD
/// is the old one. Exit 1, the bytes restored: a first lock leaves no
/// file and no index entry; an existing lock its old bytes, its index
/// entry as it was.
#[test]
fn a_head_that_did_not_move_restores_the_bytes() {
    // A first lock.
    let repo = LockRepo::code_a("lock-hook-rewind-first");
    repo.hook("post-commit", "git reset -q --soft HEAD~1");
    let before = repo.state();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let refusal = attempt.refused("HEAD did not move, first lock");
    assert!(refusal.contains("did not move"), "{refusal}");
    assert!(repo.lock().is_none(), "the new lock is left");
    assert_eq!(repo.state(), before, "a first lock");

    // An existing lock: its bytes, HEAD.
    let repo = LockRepo::code_a("lock-hook-rewind-again");
    repo.accept_ok("PIECE-COUNTER");
    repo.hook("post-commit", "git reset -q --soft HEAD~1");
    let lock = repo.lock();
    let head = repo.head();
    let attempt = repo.accept("LAW-BOUND", true);
    let refusal = attempt.refused("HEAD did not move, existing lock");
    assert!(refusal.contains("did not move"), "{refusal}");
    assert_eq!(repo.lock(), lock, "the old bytes");
    assert_eq!(repo.head(), head);
}

/// Step 10's check, "made, HEAD did not move", an existing lock: the
/// restore also leaves the lock's index entry as it was. `commit --only`
/// updated it to the new bytes before the hook wound HEAD back; restoring
/// the file alone leaves the unconfirmed-by-HEAD lock staged (`MM
/// spec.lock`), for the owner's next commit to take.
#[test]
fn a_head_that_did_not_move_leaves_the_index_as_it_was() {
    let repo = LockRepo::code_a("lock-hook-rewind-index");
    repo.accept_ok("PIECE-COUNTER");
    repo.hook("post-commit", "git reset -q --soft HEAD~1");
    let before = repo.state();
    let attempt = repo.accept("LAW-BOUND", true);
    attempt.refused("HEAD did not move, existing lock");
    assert_eq!(
        String::from_utf8_lossy(&repo.porcelain()),
        String::from_utf8_lossy(&before.status),
        "status after the restore"
    );
    assert_eq!(repo.state(), before);
}

/// Iteration 3, step 10's fourth check (M: drop the blob check): a
/// `pre-commit` hook appends to `spec.lock` and stages it, so the commit
/// holds `spec.lock` alone, with its trailer, but not the bytes the owner
/// confirmed. Exit 1 naming the commit and "not the bytes the owner
/// confirmed"; `outcome.commit` kept, history not rewritten (the branch is
/// that commit), the undo `git reset --soft <commit>^`.
#[test]
fn a_hook_that_rewrites_the_staged_lock_fails_the_accept() {
    let repo = LockRepo::code_a("lock-hook-rewrites-lock");
    repo.hook(
        "pre-commit",
        "printf '# appended by a hook\\n' >> spec.lock && git add spec.lock",
    );
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let outcome = attempt
        .outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("exit {:?}: {error}", error.exit));
    let after = repo.head();
    assert_ne!(after, head, "the scenario: git made the commit");
    assert_eq!(repo.changed(&after), vec!["spec.lock"], "the lock alone");
    assert!(
        String::from_utf8_lossy(&repo.show_blob(&after, "spec.lock"))
            .ends_with("# appended by a hook\n"),
        "the scenario: the hook's bytes are committed"
    );
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    let refusal = outcome.refusal.clone().unwrap_or_default();
    assert!(refusal.contains(&after), "names the commit: {refusal}");
    assert!(
        refusal.contains("not the bytes the owner confirmed"),
        "{refusal}"
    );
    assert_undo_advice(&refusal, &after, &head);
    assert_eq!(outcome.commit.as_deref(), Some(after.as_str()));
    assert_eq!(
        repo.git_text(&["rev-parse", "refs/heads/main"]),
        after,
        "not rewritten"
    );
}

/// Iteration 3 (M: restore unconditionally): a hook that edits
/// `spec.lock` and then fails leaves the edited file as the hook left it
/// (it is no longer the bytes this run wrote), exit 1; a first lock and
/// an existing one alike.
#[test]
fn a_failing_hook_that_edits_the_lock_leaves_it_as_edited() {
    for existing in [false, true] {
        let repo = LockRepo::code_a("lock-hook-edits-lock");
        if existing {
            repo.accept_ok("PREM-001");
        }
        repo.hook(
            "pre-commit",
            "printf '# edited by a hook\\n' >> spec.lock\nexit 1",
        );
        let head = repo.head();
        let attempt = repo.accept("PIECE-COUNTER", true);
        let refusal = attempt.refused("an edited lock, a failed commit");
        assert!(refusal.contains("left as it is"), "{refusal}");
        assert_eq!(repo.head(), head);
        let lock = repo.lock_text();
        assert!(
            lock.ends_with("# edited by a hook\n"),
            "existing {existing}: {lock}"
        );
        assert!(
            lock.contains("node = \"PIECE-COUNTER\""),
            "existing {existing}: {lock}"
        );
    }
}

/// Iteration 3 (M: the commit found through HEAD, not the branch): a
/// `post-commit` hook checks out another branch at the old commit. The
/// lock commit is `main`'s tip, verified there: exit 0 naming it, though
/// HEAD is elsewhere.
#[test]
fn the_lock_commit_is_found_on_its_branch() {
    let repo = LockRepo::code_a("lock-hook-switches");
    repo.hook("post-commit", "git switch -q -c elsewhere HEAD~1");
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let outcome = attempt.committed("a checkout after the commit");
    let main = repo.git_text(&["rev-parse", "refs/heads/main"]);
    assert_eq!(outcome.commit.as_deref(), Some(main.as_str()));
    assert_eq!(repo.parents(&main), vec![head.clone()]);
    assert_eq!(repo.changed(&main), vec!["spec.lock"]);
    assert_eq!(
        repo.git_text(&["symbolic-ref", "--short", "HEAD"]),
        "elsewhere"
    );
    assert_eq!(repo.head(), head, "HEAD is the old commit, on `elsewhere`");
}

/// Iteration 3 (M: restore when HEAD moved but the branch did not): a
/// `post-commit` hook detaches HEAD at the new commit and sets `main`
/// back. The commit is on no branch: exit 1 "unverified", and the lock's
/// bytes are left as written (the working tree is at that commit).
#[test]
fn a_commit_off_its_branch_is_unverified() {
    let repo = LockRepo::code_a("lock-hook-detaches");
    repo.hook(
        "post-commit",
        "git checkout -q --detach && git update-ref refs/heads/main HEAD~1",
    );
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let refusal = attempt.refused("the branch did not move, HEAD did");
    assert!(refusal.contains("unverified"), "{refusal}");
    assert!(refusal.contains("left as written"), "{refusal}");
    assert_eq!(repo.git_text(&["rev-parse", "refs/heads/main"]), head);
    let lock = repo.lock_text();
    assert!(
        lock.contains("node = \"PIECE-COUNTER\""),
        "left as written: {lock}"
    );
}

/// Step 10's parent check (final review's staging; M: drop the parent
/// check): a `post-commit` hook commits again on top, reusing the lock
/// commit's message (`-C HEAD`: the `Lock:` trailer kept; the tree, so
/// `spec.lock`'s bytes and the changed paths, the same). Only the parents
/// are wrong: exit 1 naming the branch's tip, the undo `git reset --soft
/// <tip>^`.
#[test]
fn a_second_commit_on_top_is_not_the_lock_commit() {
    let repo = LockRepo::code_a("lock-hook-commits-again");
    repo.hook(
        "post-commit",
        "[ -n \"$LOCK_TEST_AGAIN\" ] || LOCK_TEST_AGAIN=1 git commit -q --allow-empty --no-verify -C HEAD",
    );
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let outcome = attempt
        .outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("exit {:?}: {error}", error.exit));
    let tip = repo.git_text(&["rev-parse", "refs/heads/main"]);
    let lock_commit = repo.git_text(&["rev-parse", "refs/heads/main~1"]);
    assert_eq!(
        repo.parents(&lock_commit),
        vec![head.clone()],
        "the scenario"
    );
    assert_eq!(
        repo.parents(&tip),
        vec![lock_commit.clone()],
        "the scenario"
    );
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    let refusal = outcome.refusal.clone().unwrap_or_default();
    assert!(refusal.contains(&tip), "names the tip: {refusal}");
    assert!(refusal.contains("parents"), "{refusal}");
    assert_undo_advice(&refusal, &tip, &head);
    assert_eq!(outcome.commit.as_deref(), Some(tip.as_str()));
}

/// Step 10's check, "failed, HEAD moved to a commit that fails": a
/// `pre-commit` hook moves HEAD to a commit of its own
/// (`git commit-tree` + `git update-ref`), then fails. That commit is not
/// this run's: exit 1 naming it, the lock's bytes restored (a first lock:
/// no file, no index entry).
#[test]
fn a_failed_commit_never_takes_another_commit_as_its_own() {
    let repo = LockRepo::code_a("lock-hook-elsewhere");
    repo.hook(
        "pre-commit",
        "c=$(git commit-tree -p HEAD -m elsewhere \"HEAD^{tree}\") && git update-ref HEAD \"$c\"\nexit 1",
    );
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let refusal = attempt.refused("a failed commit, HEAD elsewhere");
    let after = repo.head();
    assert_ne!(after, head, "the scenario: the hook moved HEAD");
    assert_eq!(repo.message(&after).trim_end(), "elsewhere");
    assert!(refusal.contains(&after), "names that commit: {refusal}");
    assert!(repo.lock().is_none(), "the new lock is left");
    assert_eq!(repo.git_text(&["ls-files", "--", "spec.lock"]), "");
}

// ------------------------------------------------------------- places

/// The lock without its `commit` lines.
fn without_commits(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("commit = "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// "Each lock travels on its worktree's branch" (WA-10): a project in a
/// subdirectory of its worktree locks at its own root (`<root>/spec.lock`,
/// files root-relative), the commit changes that path alone, the question
/// names the worktree; run from below the root too.
#[test]
fn a_project_in_a_subdirectory_locks_at_its_root() {
    let scratch = Scratch::new("lock-subdir");
    let git = Sandbox::new(scratch.path());
    let top = scratch.dir("top");
    copy_dir(&fixture("code-a"), &top.join("project"));
    fs::remove_file(top.join("project/expected.json")).expect("expected.json");
    git.init(&top);
    git.add_all(&top);
    git.commit(&top, "the fixture in a subdirectory");
    let home = scratch.home("h");
    let repo = LockRepo {
        root: top.join("project"),
        scratch,
        git,
        home,
    };
    let head = repo.head();
    let cwd = repo.root.join("crates");
    let attempt = repo.attempt_at(&cwd, "PIECE-COUNTER", false, CLOCK, true);
    let outcome = attempt.committed("a project in a subdirectory");
    assert_eq!(
        attempt.question(),
        format!(
            "lock PIECE-COUNTER rev 3 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped){}",
            at(&head, "main", &top)
        )
    );
    let commit = outcome.commit.clone().expect("a commit");
    assert_eq!(repo.changed(&commit), vec!["project/spec.lock"]);
    assert!(!top.join("spec.lock").exists());
    let plain = LockRepo::code_a("lock-subdir-plain");
    plain.accept_ok("PIECE-COUNTER");
    assert_eq!(
        without_commits(&repo.lock_text()),
        without_commits(&plain.lock_text())
    );
    assert!(repo.porcelain().is_empty());
}

/// WA-10: an accept in a linked worktree commits on that worktree's
/// branch; the main worktree's branch and files are untouched.
#[test]
fn accept_in_a_linked_worktree_commits_on_its_branch() {
    let repo = LockRepo::code_a("lock-linked");
    let linked = repo.scratch.join("t1");
    repo.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "t1",
        linked.to_str().expect("a UTF-8 path"),
    ]);
    let linked = fs::canonicalize(linked).expect("the linked worktree");
    let main_before = repo.state();
    let head = repo.head();
    let attempt = repo.attempt_at(&linked, "PIECE-COUNTER", false, CLOCK, true);
    let outcome = attempt.committed("linked");
    assert_eq!(
        attempt.question(),
        format!(
            "lock PIECE-COUNTER rev 3 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped){}",
            at(&head, "t1", &linked)
        )
    );
    let commit = outcome.commit.clone().expect("a commit");
    assert_eq!(repo.git_text(&["rev-parse", "refs/heads/t1"]), commit);
    assert_eq!(repo.head(), head, "main did not move");
    let main_after = repo.state();
    assert_eq!(
        main_after.files, main_before.files,
        "main's files untouched"
    );
    assert_eq!(main_after.status, main_before.status);
    let twin = LockRepo::code_a("lock-linked-twin");
    twin.accept_ok("PIECE-COUNTER");
    assert_eq!(
        fs::read(linked.join("spec.lock")).expect("the linked lock"),
        twin.lock().expect("the twin's lock"),
        "the same commit gives the same lock"
    );
}
