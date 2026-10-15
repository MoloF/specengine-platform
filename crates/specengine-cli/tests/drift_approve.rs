//! docs/features/drift-queue.md, the approve of a drift item and its
//! review (`spec approve PR [--editorial]`, `spec reject PR`, `spec
//! review PR`, the inbox, a stage): TE-2, TE-3's approve half, TE-5,
//! TE-8 and the named RD-6..RD-11, RD-20 tests, over scratch repositories
//! of `fixtures/code-a` with the golden lock
//! `fixtures/spec-lock/code-a.lock` committed and an edit committed.
//!
//! The approve's lock is compared with a twin repository (the same
//! commits) where `spec lock accept` ran through the library at the same
//! clock: the bytes `spec.lock` must hold ("Rules and edge cases"). The
//! expected texts are the spec's ("Accept's seams", "Approve").
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::fs;
use std::path::Path;

use drift_common::{
    COUNTER, COUNTER_MD, DECIDED, DECIDED_DAY, RAISED, approve_at, approve_with_flags, approve_yes,
    cap_changed, drifted, proposal, raise_at, raised, reject_at, review_of, sql, trailers,
};
use lock_common::{Attempt, DECIDER, LockRepo, at, node_blocks, value};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, Exit, Globals, INBOX_RATIONALE_CHARS, InboxRequest, LockAcceptRequest,
    LockPrompt, Outcome, StageBody, StageCause, StageRequest, inbox, lock_accept, render_text,
    stage,
};
use specengine_store::{ProposalQueue as _, ProposalStatus};
use verify_common::golden;

const SLUG: &str = "code-a";

/// The twin of `repo`: the same history (fixture, golden lock, `edit`
/// committed), no queue.
fn twin(label: &str, edit: impl FnOnce(&LockRepo)) -> LockRepo {
    drifted(label, edit)
}

/// The twin's library accept of `node` (`editorial`) at [`DECIDED`],
/// consent yes.
fn twin_accept(twin: &LockRepo, node: &str, editorial: bool) -> Attempt {
    twin.attempt_at(&twin.root, node, editorial, DECIDED, true)
}

/// The approve's question for an accept's: its rows, then its question
/// with ` for <id>` before ` at <commit>` ("Accept's seams").
/// The twin's tail (` at <commit> on <branch> in <worktree>? [y/N]`) is
/// `repo`'s: `head` on `main` in its root.
fn with_proposal(attempt: &Attempt, id: &str, head: &str, root: &Path) -> String {
    let question = attempt.question();
    let (lead, _) = question.split_once(" at ").expect("` at ` in the question");
    format!(
        "{}{lead} for {id}{}",
        attempt.rows(),
        at(head, "main", root)
    )
}

/// The events of the queue, `(type, payload)`.
fn events(repo: &LockRepo) -> Vec<(String, Value)> {
    let queue = drift_common::queue_of(repo, SLUG).expect("a queue");
    queue
        .events()
        .expect("events")
        .into_iter()
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// A node's lock blocks, each its `key = value` pairs.
type Blocks = Vec<Vec<(String, String)>>;

/// A lock block's `key = value` lines, quotes trimmed.
fn fields(block: &str) -> Vec<(String, String)> {
    block
        .lines()
        .filter_map(|line| line.split_once(" = "))
        .map(|(key, value)| (key.to_owned(), value.trim_matches('"').to_owned()))
        .collect()
}

/// The ID of the one item `raised` created for `node`.
fn item_of(raised: &drift_common::Raised, node: &str) -> String {
    raised
        .created()
        .into_iter()
        .find(|(_, created)| created == node)
        .unwrap_or_else(|| panic!("no item for {node}: {}", raised.json_text()))
        .0
}

// ------------------------------------------------------------- TE-2, RD-6

/// TE-2 / RD-6: raise -> review -> approve -> `spec verify` `ok`: the
/// approve asks the accept's question with ` for PR-0001`, commits
/// `spec.lock` alone (one parent, the subject `spec: lock LAW-BOUND`, the
/// trailers `Lock`, `Lock-mode: accept`, `Decided-by`, then `Proposal:
/// PR-0001` last, as `git interpret-trailers --parse` reads them); the item
/// `applied` with that commit; `spec.lock` holds the bytes the twin's
/// `spec lock accept LAW-BOUND` writes; `CAP`'s row now `ok`. M: a second
/// accept; `Proposal:` first; another file committed.
#[test]
fn drift_approve_writes_the_bytes_lock_accept_writes() {
    let repo = drifted("da-bytes", cap_changed);
    let twin = twin("da-bytes-twin", cap_changed);
    assert_eq!(repo.head(), twin.head(), "the same commits");
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    assert_eq!(id, "PR-0001");
    let head = repo.head();
    let accepted = twin_accept(&twin, "LAW-BOUND", false);
    accepted.committed("the twin's accept");

    let approved = approve_yes(&repo, &id);
    let outcome = approved.done("approve");
    assert_eq!(
        approved.question(),
        with_proposal(&accepted, &id, &head, &repo.root)
    );
    assert!(
        approved
            .question()
            .ends_with(&format!(" for PR-0001{}", at(&head, "main", &repo.root))),
        "{}",
        approved.question()
    );
    let commit = repo.head();
    assert_ne!(commit, head, "a lock commit");
    assert_eq!(repo.parents(&commit), [head.as_str()]);
    assert_eq!(repo.changed(&commit), ["spec.lock"], "the lock alone");
    let message = repo.message(&commit);
    assert_eq!(
        message.lines().next(),
        Some("spec: lock LAW-BOUND"),
        "{message}"
    );
    assert_eq!(
        trailers(&message),
        [
            "Lock: LAW-BOUND".to_owned(),
            "Lock-mode: accept".to_owned(),
            format!("Decided-by: {DECIDER}"),
            "Proposal: PR-0001".to_owned(),
        ]
    );
    let parsed = repo.git.git_stdin(
        &repo.root,
        &["interpret-trailers", "--parse"],
        message.as_bytes(),
    );
    assert_eq!(
        String::from_utf8(parsed).expect("UTF-8"),
        format!("Lock: LAW-BOUND\nLock-mode: accept\nDecided-by: {DECIDER}\nProposal: PR-0001\n")
    );
    assert_eq!(
        repo.lock(),
        twin.lock(),
        "the bytes spec lock accept writes"
    );
    let stored = proposal(&repo, SLUG, &id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(outcome.document.status.as_deref(), Some("applied"));
    assert!(repo.porcelain().is_empty(), "the tree is clean");
    // `spec verify`: CAP's row reads `ok` now.
    let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
    let rows = run.json()["rows"].clone();
    let cap = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["node"] == "LAW-BOUND" && row["symbol"] == "crates/tally::counter::CAP")
        .expect("CAP's row")
        .clone();
    assert_eq!(cap["state"], "ok", "{cap}");
    // The stored evidence is never rewritten.
    assert_eq!(
        stored.drift,
        proposal(&repo, SLUG, &id).drift,
        "the item's drift"
    );
}

/// RD-1: a drift item never runs the update apply: its approve writes no
/// spec file and commits no apply trailers (`Base-commit:`, a rationale),
/// only the lock commit (`spec: lock LAW-BOUND`, `Lock:` first); the item
/// records that commit. M: `Drift` in the `Applies` family.
#[test]
fn drift_item_never_runs_the_update_apply() {
    let repo = drifted("da-not-update", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let spec_before = fs::read(repo.root.join(COUNTER_MD)).unwrap();
    approve_yes(&repo, &id).done("approve");
    let commit = repo.head();
    let message = repo.message(&commit);
    assert!(message.starts_with("spec: lock LAW-BOUND\n"), "{message}");
    assert!(!message.contains("Base-commit:"), "{message}");
    assert_eq!(trailers(&message)[0], "Lock: LAW-BOUND");
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(fs::read(repo.root.join(COUNTER_MD)).unwrap(), spec_before);
    let stored = proposal(&repo, SLUG, &id);
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(stored.target_path, "spec.lock");
}

/// RD-6: the lock commit runs the repository's hooks (`pre-commit`,
/// `commit-msg` seeing `Proposal: PR-0001`, `post-commit`). M:
/// `--no-verify`.
#[test]
fn drift_approve_runs_hooks() {
    let repo = drifted("da-hooks", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let log = repo.scratch.join("hooks.log");
    for hook in ["pre-commit", "post-commit"] {
        repo.hook(hook, &format!("echo {hook} >> '{}'", log.display()));
    }
    repo.hook(
        "commit-msg",
        &format!("grep '^Proposal: ' \"$1\" >> '{}'", log.display()),
    );
    approve_yes(&repo, &id).done("approve");
    assert_eq!(
        fs::read_to_string(&log).unwrap_or_default(),
        "pre-commit\nProposal: PR-0001\npost-commit\n"
    );
}

/// RD-6: unrelated staged and modified files stay out of the lock commit
/// and as they were (staged, modified). M: the node's or another file
/// committed.
#[test]
fn drift_approve_commits_only_the_lock() {
    let repo = drifted("da-only", |repo| {
        cap_changed(repo);
        repo.write("notes/todo.txt", "a tracked note\n");
    });
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.write("NOTES.txt", "a staged note\n");
    repo.git(&["add", "NOTES.txt"]);
    repo.append("notes/todo.txt", "an edit not staged\n");
    let head = repo.head();
    approve_yes(&repo, &id).done("approve");
    let commit = repo.head();
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(repo.parents(&commit), [head]);
    let status = repo.git_text(&["status", "--porcelain=v1"]);
    assert_eq!(status, "A  NOTES.txt\n M notes/todo.txt", "{status}");
}

/// RD-6, the fifth check, TE-5
/// (`docs/features/drift-queue.md` "Accept's seams"): a `commit-msg` hook
/// dropping `Proposal:` leaves a commit
/// that fails step 10: exit 1 naming the trailer, the item `approved`
/// (not `applied`), the commit kept. M: no fifth check.
#[test]
fn dropped_proposal_trailer_keeps_the_item_approved() {
    let repo = drifted("da-dropped", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.hook(
        "commit-msg",
        "grep -v '^Proposal: ' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"",
    );
    let head = repo.head();
    let approved = approve_yes(&repo, &id);
    let reason = approved.refused("a dropped trailer");
    assert!(reason.contains("Proposal: PR-0001"), "{reason}");
    let commit = repo.head();
    assert_ne!(commit, head, "git made the commit");
    assert!(!repo.message(&commit).contains("Proposal:"));
    let stored = proposal(&repo, SLUG, &id);
    assert_eq!(stored.status, ProposalStatus::Approved, "{reason}");
    assert_eq!(stored.applied_commit, None);
}

// ------------------------------------------------------------------ RD-7

/// A golden repository with a linked worktree on `t1` holding `edit`
/// committed there: `(repo, linked)`; the main worktree stays on `main`.
fn linked(label: &str, edit: impl FnOnce(&LockRepo, &Path)) -> (LockRepo, std::path::PathBuf) {
    let repo = golden(label);
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
    edit(&repo, &linked);
    repo.git.add_all(&linked);
    repo.git.commit(&linked, "the edit on t1");
    (repo, linked)
}

/// RD-7: raised in the linked worktree on `t1`, approved from the main
/// worktree: the lock commit lands on `t1` in the linked worktree, the
/// question names it; `main`, its files and status untouched. M: the
/// accept run in the current directory's worktree.
#[test]
fn drift_approves_only_where_raised() {
    let (repo, linked) = linked("da-where", |_, linked| {
        common::replace(linked, COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    });
    let raised = raise_at(&repo, &linked, &[], RAISED);
    let id = item_of(&raised, "LAW-BOUND");
    let main_of = |repo: &LockRepo| {
        let state = repo.state();
        (
            state.head,
            repo.git_text(&["rev-parse", "refs/heads/main"]),
            state.status,
            state.index,
            state.files,
        )
    };
    let main_before = main_of(&repo);
    let t1_head = repo.git.head(&linked).expect("t1's HEAD");
    let approved = approve_at(&repo, &repo.root, &id, false, true);
    approved.done("approve from main");
    assert!(
        approved
            .question()
            .ends_with(&format!(" for {id}{}", at(&t1_head, "t1", &linked))),
        "{}",
        approved.question()
    );
    let commit = repo.git_text(&["rev-parse", "refs/heads/t1"]);
    assert_ne!(commit, t1_head);
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(main_of(&repo), main_before, "main untouched");
    assert_eq!(
        proposal(&repo, SLUG, &id).applied_commit.as_deref(),
        Some(commit.as_str())
    );
}

/// RD-7: the accept runs at the recorded root, never the current
/// directory's: approved from the main worktree (on `main`, its own lock),
/// the item raised on `t1` is locked in the linked worktree's
/// `spec.lock`, the main worktree's `spec.lock` byte for byte as before.
/// M: `Globals` of the current directory (the accept then on `main`, its
/// branch check refusing).
#[test]
fn drift_approve_locks_the_recorded_root() {
    let (repo, linked) = linked("da-root", |_, linked| {
        common::replace(linked, COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    });
    let raised = raise_at(&repo, &linked, &[], RAISED);
    let id = item_of(&raised, "LAW-BOUND");
    let main_lock = repo.lock();
    let linked_lock = fs::read(linked.join("spec.lock")).expect("t1's lock");
    approve_at(&repo, &repo.root, &id, false, true).done("approve from main");
    assert_eq!(repo.lock(), main_lock, "main's lock untouched");
    assert_ne!(
        fs::read(linked.join("spec.lock")).expect("t1's lock"),
        linked_lock,
        "t1's lock written"
    );
    assert_eq!(repo.git.head(&repo.root), Some(repo.head()));
}

/// RD-7 ("Accept's seams"): an accept naming a proposal and its recorded
/// branch refuses another `HEAD`: exit 1 `` HEAD left `t9` after PR-0012's
/// place was checked; nothing written ``, no question, nothing written.
/// M: `branch` unchecked.
#[test]
fn drift_approve_locks_the_recorded_branch() {
    let repo = drifted("da-branch", cap_changed);
    let before = repo.state();
    let mut asked = 0;
    let mut consent = |_: &LockPrompt<'_>| {
        asked += 1;
        true
    };
    let outcome = lock_accept(
        &repo.env(&repo.root),
        &Globals::default(),
        &LockAcceptRequest {
            reference: "LAW-BOUND".to_owned(),
            editorial: false,
            now: DECIDED.to_owned(),
            git: repo.git_env(&repo.root),
            proposal: Some("PR-0012".to_owned()),
            branch: Some("t9".to_owned()),
        },
        &mut consent,
        &[],
    )
    .expect("an answer");
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    assert_eq!(
        outcome.refusal.as_deref(),
        Some("HEAD left `t9` after PR-0012's place was checked; nothing written")
    );
    assert_eq!(asked, 0, "no question");
    assert_eq!(repo.state(), before, "nothing written");
}

/// RD-7: `spec approve` of a drift item refuses a stdin that is no
/// terminal before anything is read or logged: exit 2, nothing changed,
/// even for an ID the queue does not hold. M: a read before the terminal
/// check.
#[test]
fn drift_approve_needs_a_terminal_first() {
    let repo = drifted("da-terminal", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let before = (repo.state(), drift_common::dump(&repo, SLUG));
    for target in [id.as_str(), "PR-0099"] {
        let run = repo.spec_piped(&repo.root, &["approve", target]);
        run.code(2);
        assert!(
            run.stderr
                .contains("asks the owner for consent on a terminal"),
            "{target}: {}",
            run.show()
        );
        assert_eq!(run.stdout, "");
        assert_eq!(
            (repo.state(), drift_common::dump(&repo, SLUG)),
            before,
            "{target}"
        );
    }
}

/// RD-7: a bound file of the node modified (not committed) refuses the
/// approve as the accept refuses it: exit 1 naming the file, one
/// `proposal.apply_failed` at step 3, the item `open`, nothing written.
/// M: the dirty input ignored.
#[test]
fn dirty_bound_file_refuses_drift_approve() {
    let repo = drifted("da-dirty", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.append(COUNTER, "// not committed\n");
    let before = repo.state();
    let events_before = events(&repo).len();
    let approved = approve_yes(&repo, &id);
    let reason = approved.refused("a dirty bound file");
    assert!(
        reason.contains("`crates/tally/src/counter.rs` modified"),
        "{reason}"
    );
    assert!(approved.questions.is_empty(), "no question");
    assert_eq!(repo.state(), before);
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
    let logged = events(&repo);
    assert_eq!(logged.len(), events_before + 1, "{logged:?}");
    let (kind, payload) = logged.last().unwrap();
    assert_eq!(kind, "proposal.apply_failed");
    assert_eq!(payload["step"], json!(3), "{payload}");
}

/// TE-3 / "Approve" (ask: no identity: exit 2, logged as step 7): no
/// committer identity in the recorded worktree: exit 2 before the
/// question, one `apply_failed` at step 7, the item `open`, nothing
/// written.
#[test]
fn no_git_identity_exits_2_logged_at_step_7() {
    let repo = drifted("da-ident", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.git(&["config", "user.useConfigOnly", "true"]);
    let git = repo
        .git_env(&repo.root)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let before = repo.state();
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::approve(
        &repo.env(&repo.root),
        &Globals::default(),
        &specengine_cli::ApproveRequest {
            id: id.clone(),
            note: None,
            now: DECIDED.to_owned(),
            git,
        },
        &mut consent,
    );
    let error = outcome.expect_err("no identity: exit 2");
    assert_eq!(error.exit, Exit::CannotRun, "{}", error.message);
    assert!(error.message.contains("(step 7)"), "{}", error.message);
    assert!(questions.is_empty(), "asked: {questions:?}");
    assert_eq!(repo.state(), before);
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
    let (kind, payload) = events(&repo).last().cloned().unwrap();
    assert_eq!(kind, "proposal.apply_failed");
    assert_eq!(payload["step"], json!(7), "{payload}");
}

// ------------------------------------------------------------------ RD-8

/// LAW-BOUND's text edited at its locked revision (`spec_ahead
/// same_revision`), code unchanged.
fn same_revision(repo: &LockRepo) {
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
}

/// RD-8: `spec approve PR --editorial` runs the accept's `--editorial`:
/// one commit of `spec.lock`, `Lock-mode: editorial`, the bytes the
/// twin's `spec lock accept --editorial LAW-BOUND` writes; a plain approve
/// refused first, its parenthetical naming `spec approve PR-0001
/// --editorial` (never `spec lock accept`). M: `editorial_refusal` as
/// `!= Applies` (exit 2).
#[test]
fn drift_editorial_runs_accept_editorial() {
    let repo = drifted("da-editorial", same_revision);
    let twin = twin("da-editorial-twin", same_revision);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let plain = approve_yes(&repo, &id);
    let reason = plain.refused("a plain approve at R");
    assert!(
        reason.contains("spec approve PR-0001 --editorial"),
        "{reason}"
    );
    assert!(!reason.contains("spec lock accept"), "{reason}");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
    twin_accept(&twin, "LAW-BOUND", true).committed("the twin's editorial");
    let approved = approve_at(&repo, &repo.root, &id, true, true);
    approved.done("approve --editorial");
    let commit = repo.head();
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(
        trailers(&repo.message(&commit)),
        [
            "Lock: LAW-BOUND".to_owned(),
            "Lock-mode: editorial".to_owned(),
            format!("Decided-by: {DECIDER}"),
            "Proposal: PR-0001".to_owned(),
        ]
    );
    assert_eq!(repo.lock(), twin.lock(), "accept's editorial bytes");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Applied);
}

/// The LAW-BOUND blocks of the lock before and after an approve
/// `--editorial` of the item `same_revision` (and `prepare`) raises, and
/// the node's new `norm_hash`.
fn editorial_blocks(label: &str, prepare: impl FnOnce(&LockRepo)) -> (Blocks, Blocks, String) {
    let repo = drifted(label, |repo| {
        prepare(repo);
        same_revision(repo);
    });
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let old = repo.lock_text();
    approve_at(&repo, &repo.root, &id, true, true).done("approve --editorial");
    let new = repo.lock_text();
    let norm = repo.norm_hash("LAW-BOUND");
    let before: Vec<_> = node_blocks(&old, "LAW-BOUND")
        .iter()
        .map(|block| fields(block))
        .collect();
    let after: Vec<_> = node_blocks(&new, "LAW-BOUND")
        .iter()
        .map(|block| fields(block))
        .collect();
    assert_eq!(before.len(), after.len());
    assert!(after.len() >= 4, "{after:?}");
    (before, after, norm)
}

/// RD-8: an editorial approve keeps every LAW-BOUND entry's code fields
/// (`ast_hash`, `file_blob`, the key, `rev`), its spec hash the new text's.
/// M: code hashes refreshed.
#[test]
fn drift_editorial_keeps_code_hashes() {
    let (before, after, norm) = editorial_blocks("da-editorial-code", |_| {});
    for (old_fields, new_fields) in before.iter().zip(&after) {
        assert_eq!(value(new_fields, "spec_hash"), Some(norm.as_str()));
        for key in ["ast_hash", "file_blob", "symbol", "file", "relation", "rev"] {
            assert_eq!(value(new_fields, key), value(old_fields, key), "{key}");
        }
    }
}

/// RD-8: `--editorial` refreshes every entry at R, as the accept's does,
/// one that never vouched for the old text too (its spec hash another, as
/// a regen dropping a branch's refresh leaves it): all four LAW-BOUND
/// entries carry the new spec hash. M: 4b-1's vouching filter.
#[test]
fn drift_editorial_refreshes_every_entry_at_r() {
    let (before, after, norm) = editorial_blocks("da-editorial-every", |repo| {
        // The configures entry of `CAP` vouches for no text of the node.
        let lock = repo.lock_text();
        let block = node_blocks(&lock, "LAW-BOUND")
            .into_iter()
            .find(|block| block.contains("symbol = \"crates/tally::counter::CAP\""))
            .expect("CAP's entry");
        let old_hash = value(&fields(&block), "spec_hash")
            .expect("a spec hash")
            .to_owned();
        let stray = format!("b3:{}", "f".repeat(64));
        let rewritten = block.replacen(&old_hash, &stray, 1);
        repo.write("spec.lock", lock.replacen(&block, &rewritten, 1));
    });
    let strays: Vec<_> = before
        .iter()
        .filter(|fields| {
            value(fields, "spec_hash") == Some(format!("b3:{}", "f".repeat(64)).as_str())
        })
        .collect();
    assert_eq!(strays.len(), 1, "the scenario: one entry not vouching");
    for new_fields in &after {
        assert_eq!(
            value(new_fields, "spec_hash"),
            Some(norm.as_str()),
            "{new_fields:?}"
        );
    }
}

/// RD-8: `--editorial` on `spec_changed` (PIECE-COUNTER's `rev` raised:
/// no entry at R) is refused naming `spec approve PR-0001`; the item stays
/// `open`, nothing written.
#[test]
fn editorial_without_entry_at_r_refuses() {
    let repo = drifted("da-editorial-r", |repo| {
        repo.replace(COUNTER_MD, "rev: 3\n", "rev: 4\n");
        repo.replace(
            COUNTER_MD,
            "A count that grows one step",
            "A count that grows by one step",
        );
    });
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "PIECE-COUNTER");
    let before = repo.state();
    let approved = approve_at(&repo, &repo.root, &id, true, true);
    let reason = approved.refused("editorial without an entry at R");
    assert!(reason.contains(&format!("spec approve {id}")), "{reason}");
    assert!(
        !reason.contains("spec lock accept"),
        "the item's command, not the hand accept: {reason}"
    );
    assert_eq!(repo.state(), before);
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
}

/// RD-8: a decision flag on a drift item exits 2 naming the accept
/// (`spec approve PR-0001 runs the lock accept for LAW-BOUND`), nothing
/// written.
#[test]
fn a_decision_flag_on_a_drift_item_exits_2() {
    let repo = drifted("da-flags", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let before = (repo.state(), drift_common::dump(&repo, SLUG));
    for flags in [
        ApproveFlags {
            option: Some(0),
            ..ApproveFlags::default()
        },
        ApproveFlags {
            answer: Some("Yes.".to_owned()),
            ..ApproveFlags::default()
        },
        ApproveFlags {
            canon: Some("LAW-BOUND".to_owned()),
            ..ApproveFlags::default()
        },
    ] {
        let approved = approve_with_flags(&repo, &repo.root, &id, false, &flags, true);
        let message = approved.cannot(&format!("{flags:?}"));
        assert!(
            message.contains(&format!(
                "spec approve {id} runs the lock accept for LAW-BOUND"
            )),
            "{message}"
        );
        assert_eq!(
            (repo.state(), drift_common::dump(&repo, SLUG)),
            before,
            "{flags:?}"
        );
    }
}

// ------------------------------------------------------------------ RD-9

/// Makes the lock commit an approve of `id` makes (the library accept
/// naming the item and its branch) without recording it: a run killed
/// after its commit. The commit.
fn commit_as_killed(repo: &LockRepo, node: &str, id: &str) -> String {
    let mut consent = |_: &LockPrompt<'_>| true;
    let outcome = lock_accept(
        &repo.env(&repo.root),
        &Globals::default(),
        &LockAcceptRequest {
            reference: node.to_owned(),
            editorial: false,
            now: DECIDED.to_owned(),
            git: repo.git_env(&repo.root),
            proposal: Some(id.to_owned()),
            branch: Some("main".to_owned()),
        },
        &mut consent,
        &[],
    )
    .expect("the accept");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.commit.expect("a commit")
}

/// RD-9 / TE-5: a run killed after its commit: the item `open`, its lock
/// commit (`Proposal:`, `Lock:`) on `main`: `spec approve` completes it by
/// the trailer, no new commit, `applied` with that commit; a run killed
/// before the commit (`approved`, nothing written) runs the accept and
/// commits. M: the killed run committed again; the commit judged by
/// `trailer_commit` (the lock read as Markdown).
#[test]
fn killed_drift_approve_completes_by_trailer() {
    let repo = drifted("da-killed", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let commit = commit_as_killed(&repo, "LAW-BOUND", &id);
    let lock = repo.lock();
    let approved = approve_yes(&repo, &id);
    approved.done("completion");
    assert_eq!(repo.head(), commit, "no new commit");
    assert_eq!(repo.lock(), lock);
    let stored = proposal(&repo, SLUG, &id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));

    // Killed before the commit: `approved`, nothing written.
    let repo = drifted("da-killed-before", cap_changed);
    let raised = drift_common::raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    {
        let mut queue = drift_common::queue_of(&repo, SLUG).expect("a queue");
        queue
            .approve(
                &id,
                &specengine_store::Decision {
                    decided_by: DECIDER.to_owned(),
                    note: None,
                    staged_at: None,
                },
                RAISED,
            )
            .expect("approved");
    }
    let head = repo.head();
    approve_yes(&repo, &id).done("after a run killed before its commit");
    let commit = repo.head();
    assert_ne!(commit, head, "the accept committed");
    assert_eq!(
        proposal(&repo, SLUG, &id).applied_commit.as_deref(),
        Some(commit.as_str())
    );
}

/// RD-9: a plain reject is refused while the item's lock commit is on its
/// branch; the item stays `open`. M: the lock commit ignored.
#[test]
fn reject_refuses_a_drift_item_with_its_lock_commit() {
    let repo = drifted("da-reject", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let commit = commit_as_killed(&repo, "LAW-BOUND", &id);
    let rejected = reject_at(&repo, &repo.root, &id);
    let reason = rejected.refused("a reject with the lock commit");
    assert!(
        reason.contains(&commit[..7]) || reason.contains("approve"),
        "{reason}"
    );
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
    // Without a lock commit, a reject settles it.
    let other = drifted("da-reject-plain", cap_changed);
    let raised = drift_common::raised(&other, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    reject_at(&other, &other.root, &id).done("a plain reject");
    assert_eq!(proposal(&other, SLUG, &id).status, ProposalStatus::Rejected);
}

/// RD-9 / TE-5: a failing `pre-commit` hook: the bytes restored, the item
/// reopened (`open`), exit 1, one `apply_failed` at step 9; a hook that
/// edits the lock then fails: the lock left as edited, the item stays
/// `approved`. M: reopened after a commit; `left` reopened.
#[test]
fn drift_approve_reopens_only_restored_bytes() {
    let repo = drifted("da-restored", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.hook("pre-commit", "exit 1");
    let before = repo.state();
    let approved = approve_yes(&repo, &id);
    approved.refused("a failing hook");
    assert_eq!(repo.state(), before, "restored");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
    let (kind, payload) = events(&repo).last().cloned().unwrap();
    assert_eq!(kind, "proposal.apply_failed");
    assert_eq!(payload["step"], json!(9), "{payload}");

    let repo = drifted("da-left", cap_changed);
    let raised = drift_common::raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.hook(
        "pre-commit",
        "printf '# edited by a hook\\n' >> spec.lock\nexit 1",
    );
    let approved = approve_yes(&repo, &id);
    let reason = approved.refused("a hook editing the lock");
    assert!(reason.contains("left as it is"), "{reason}");
    assert!(repo.lock_text().ends_with("# edited by a hook\n"));
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Approved);
}

/// RD-9 / TE-5 (Q-B): a commit made whose branch and `HEAD` a
/// `post-commit` hook winds back: the accept restores the bytes, the item
/// is reopened (`open`), not kept `approved` (4b-1's Q16). M: Q16 taken.
#[test]
fn unmoved_lock_commit_reopens_the_drift_item() {
    let repo = drifted("da-unmoved", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.hook("post-commit", "git reset -q --soft HEAD~1");
    let lock = repo.lock();
    let head = repo.head();
    let approved = approve_yes(&repo, &id);
    let reason = approved.refused("HEAD wound back");
    assert!(reason.contains("did not move"), "{reason}");
    assert_eq!(repo.head(), head);
    assert_eq!(repo.lock(), lock, "the bytes restored");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
}

// ------------------------------------------------------------ RD-10, RD-11

/// RD-10: the golden `PIECE-COUNTER` item (`spec_ahead marker_behind`
/// only, every entry locked as it is): nothing to accept: exit 1 `PR-0001
/// not applied: nothing to accept now for PIECE-COUNTER (<why>); spec
/// reject PR-0001 --reason … settles it`, no question, still `open`, one
/// `apply_failed` logged, nothing written. M: `applied` with nothing
/// accepted.
#[test]
fn drift_with_nothing_to_accept_is_not_applied() {
    let repo = golden("da-nothing");
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "PIECE-COUNTER");
    let before = repo.state();
    let approved = approve_yes(&repo, &id);
    let reason = approved.refused("nothing to accept");
    let (head, tail) = reason
        .split_once(" (")
        .unwrap_or_else(|| panic!("no `(why)`: {reason}"));
    assert_eq!(
        head,
        "PR-0001 not applied: nothing to accept now for PIECE-COUNTER"
    );
    assert!(
        tail.ends_with("); spec reject PR-0001 --reason \u{2026} settles it"),
        "{reason}"
    );
    assert!(approved.questions.is_empty());
    assert_eq!(repo.state(), before);
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
    assert_eq!(events(&repo).last().unwrap().0, "proposal.apply_failed");
}

/// RD-10, TE-2: the review of a drift item: 48 keys, `drift` `{hash,
/// rows, more}` as stored, `preview` `null` and the note `spec approve
/// PR-0001 shows the rows before asking`; `diff`, `conflict`,
/// `base_text`, `new_text`, `revisions` `null`; the text `drift: <n>
/// row(s)` with the rows indented; the inbox line `PR-0001 | drift | open
/// | LAW-BOUND | main | <created_at> | <summary>`, the summary's first line
/// cut at the inbox's [`INBOX_RATIONALE_CHARS`] (one less and `…`): since
/// docs/features/symbol-discriminator.md LAW-BOUND's summary holds the two
/// `impl Tally` rows and runs past it. No walk of the code: a review with
/// the code unreadable answers alike. M: a preview computed.
#[test]
fn drift_review_has_no_preview() {
    let repo = drifted("da-review", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let stored = proposal(&repo, SLUG, &id);
    let item = stored.drift.clone().expect("drift");
    let (text, json_text, document) = review_of(&repo, &id);
    assert_eq!(document.as_object().unwrap().len(), 48);
    assert_eq!(document["kind"], json!("drift"));
    assert_eq!(document["preview"], Value::Null);
    for key in ["diff", "conflict", "base_text", "new_text", "revisions"] {
        assert_eq!(document[key], Value::Null, "{key}");
    }
    assert_eq!(
        document["drift"],
        json!({"hash": item.hash, "rows": item.evidence.rows, "more": item.evidence.more})
    );
    assert!(
        json_text.contains(&format!(
            "\"drift\":{{\"hash\":\"{}\",\"rows\":[",
            item.hash
        )),
        "{json_text}"
    );
    let notes: Vec<&str> = document["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        notes.contains(&"spec approve PR-0001 shows the rows before asking"),
        "{notes:?}"
    );
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("drift: "))
        .unwrap_or_else(|| panic!("no drift line: {text}"));
    assert_eq!(
        lines[at],
        format!("drift: {} row(s)", item.evidence.rows.len())
    );
    for (line, row) in lines[at + 1..].iter().zip(&item.evidence.rows) {
        assert!(line.starts_with("  "), "{line}");
        assert!(line.contains(&format!(" | {} | ", row.node)), "{line}");
    }
    assert_eq!(
        lines[at - 1].split_once(':').map(|(key, _)| key),
        Some("revisions"),
        "drift after revisions: {text}"
    );
    // The inbox.
    let listed = inbox(
        &repo.env(&repo.root),
        &Globals::default(),
        &InboxRequest {
            all: false,
            git: repo.git_env(&repo.root),
        },
    )
    .expect("inbox");
    let printed = render_text(&Outcome::Inbox(listed));
    let first = item.summary.lines().next().unwrap_or_default();
    let shown = if first.chars().count() <= INBOX_RATIONALE_CHARS {
        first.to_owned()
    } else {
        let mut cut: String = first.chars().take(INBOX_RATIONALE_CHARS - 1).collect();
        cut.push('\u{2026}');
        cut
    };
    assert!(
        printed.lines().any(|line| line
            == format!("PR-0001 | drift | open | LAW-BOUND | main | {RAISED} | {shown}")),
        "{printed}"
    );
    // No walk: the bound code unreadable, the review alike.
    use std::os::unix::fs::PermissionsExt as _;
    let path = repo.root.join(COUNTER);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let again = review_of(&repo, &id);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(again.1, json_text, "no code read by the review");
}

/// RD-11: a stage of a drift item (approve, `--editorial`, reject) is
/// refused by the CLI before any preview: `StageCause::Refused`,
/// `PR-0001 is a drift item, decided on a terminal only; nothing staged`;
/// nothing stored, no event. M: the CLI check removed (the store refusing
/// alone).
#[test]
fn drift_item_cannot_be_staged() {
    let repo = drifted("da-stage", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let updated_at = proposal(&repo, SLUG, &id).updated_at;
    let before = drift_common::dump(&repo, SLUG);
    for body in [
        StageBody::Approve {
            option: None,
            answer: None,
            canon: None,
            editorial: None,
            note: None,
            updated_at: updated_at.clone(),
        },
        StageBody::Approve {
            option: None,
            answer: None,
            canon: None,
            editorial: Some(true),
            note: Some("typo".to_owned()),
            updated_at: updated_at.clone(),
        },
        StageBody::Reject {
            reason: "not now".to_owned(),
            updated_at: updated_at.clone(),
        },
    ] {
        let outcome = stage(
            &repo.env(&repo.root),
            &Globals::default(),
            &StageRequest {
                id: id.clone(),
                body: body.clone(),
                now: DECIDED.to_owned(),
                git: repo.git_env(&repo.root),
            },
        )
        .expect("an answer");
        assert_eq!(outcome.cause, Some(StageCause::Refused), "{body:?}");
        assert_eq!(
            outcome.proposal.refusal.as_deref(),
            Some("PR-0001 is a drift item, decided on a terminal only; nothing staged"),
            "{body:?}"
        );
        assert_eq!(drift_common::dump(&repo, SLUG), before, "{body:?}");
    }
}

// --------------------------------------------------------------- RD-4

/// RD-4: the approve never reads the stored evidence: the item's `drift`
/// and `drift_hash` rewritten in the database (still of their shape), the
/// approve locks what the accept judges now, the bytes the twin writes;
/// the stored columns as rewritten. M: approve reading the stored column.
#[test]
fn approve_never_writes_stored_values() {
    let repo = drifted("da-stored", cap_changed);
    let twin = twin("da-stored-twin", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let other_hash = format!("b3:{}", "a".repeat(64));
    let stored = proposal(&repo, SLUG, &id);
    let evidence = serde_json::to_string(&stored.drift.as_ref().unwrap().evidence).unwrap();
    // Every stored row now says another state (still a finding's): the
    // approve judges anew, whatever the column claims.
    let rewritten = evidence
        .replace("code_changed", "raised")
        .replace("\"code_ahead\"", "\"spec_ahead\"");
    assert_ne!(rewritten, evidence);
    sql(
        &repo,
        SLUG,
        &format!(
            "UPDATE proposals SET drift = '{}', drift_hash = '{other_hash}' WHERE id = '{id}';",
            rewritten.replace('\'', "''")
        ),
    );
    twin_accept(&twin, "LAW-BOUND", false).committed("the twin");
    approve_yes(&repo, &id).done("approve");
    assert_eq!(repo.lock(), twin.lock());
    let after = proposal(&repo, SLUG, &id);
    let item = after.drift.expect("drift");
    assert_eq!(item.hash, other_hash, "the stored hash as rewritten");
    assert_eq!(serde_json::to_string(&item.evidence).unwrap(), rewritten);
}

// --------------------------------------------------------------- RD-5

/// RD-5 (Q14): an applied item never settles: its lock commit reverted,
/// the same evidence raises a new item with the note `PR-0002: LAW-BOUND
/// drifts as when PR-0001 was applied by <commit>; that lock is not in
/// effect at the commit judged`. M: `applied` settling.
#[test]
fn reverted_lock_commit_raises_again() {
    let repo = drifted("da-reverted", cap_changed);
    let first = raised(&repo, &[]);
    let id = item_of(&first, "LAW-BOUND");
    approve_yes(&repo, &id).done("approve");
    let applied = proposal(&repo, SLUG, &id)
        .applied_commit
        .expect("applied by a commit");
    repo.git(&["revert", "--no-edit", &applied]);
    let again = raised(&repo, &[]);
    let created = again.created();
    let (new_id, _) = created
        .iter()
        .find(|(_, node)| node == "LAW-BOUND")
        .unwrap_or_else(|| panic!("LAW-BOUND raised again: {}", again.json_text()));
    assert_ne!(new_id, &id);
    assert_eq!(
        proposal(&repo, SLUG, new_id).drift.unwrap().hash,
        proposal(&repo, SLUG, &id).drift.unwrap().hash,
        "the same evidence"
    );
    let note = format!(
        "{new_id}: LAW-BOUND drifts as when {id} was applied by {applied}; that lock is not in \
         effect at the commit judged"
    );
    assert!(again.notes().contains(&note), "{:?}", again.notes());
    assert!(
        again.stderr().contains(&format!("note: {note}")),
        "{:?}",
        again.stderr()
    );
}

// -------------------------------------------------------------- TE-8

/// TE-8: the lock commit under the project's own hooks: a `pre-commit`
/// running `spec check --staged` passes and the item is applied.
#[test]
fn lock_commit_passes_spec_check_staged() {
    let repo = drifted("da-check-hook", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let hook_home = repo.scratch.home("hook");
    repo.hook(
        "pre-commit",
        &format!(
            "HOME='{}' '{}' check --staged",
            hook_home.display(),
            common::SPEC
        ),
    );
    approve_yes(&repo, &id).done("approve under spec check --staged");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Applied);
}

/// TE-8: `spec verify --changed` as the `pre-commit` hook, on the
/// second fixture (verify-ecs: every binding sound but a dangling marker
/// naming no node, another node's pre-existing Fix row): under
/// `enforce-introduced` the lock commit passes (the item applied); under
/// `enforce` the Fix row fails it: the bytes restored, the item reopened.
///
/// docs/features/rev-rule.md TE-5 merged in: the hook running `spec
/// verify --staged` beside `--changed`, through a drift item's approve
/// and through a plain `spec lock accept`. With `TUNE-01`'s document
/// deleted on disk (uncommitted: its markers dangle there, a Fix row
/// `HEAD` lacks) under `enforce-introduced`, the `--staged` hook judges
/// only `HEAD` and the staged lock: it passes; the `--changed` hook reads
/// the disk: it refuses, restored, reopened. Under `enforce` the
/// pre-existing Fix row refuses both. M: `--staged` reading the disk.
#[test]
fn lock_commit_under_spec_verify_hooks() {
    // (the hook's flag, the mode, TUNE-01's document gone on disk, passes)
    let cases = [
        ("--changed", "enforce-introduced", false, true),
        ("--changed", "enforce", false, false),
        ("--staged", "enforce-introduced", false, true),
        ("--staged", "enforce", false, false),
        ("--staged", "enforce-introduced", true, true),
        ("--changed", "enforce-introduced", true, false),
    ];
    for (flag, mode, gone, passes) in cases {
        let throughs: &[&str] = if flag == "--changed" && !gone {
            &["approve"]
        } else {
            &["approve", "accept"]
        };
        for through in throughs {
            let context = format!("{flag} {mode} gone={gone} {through}");
            let label = format!(
                "da-verify-{}-{mode}-{}-{through}",
                flag.trim_start_matches('-'),
                u8::from(gone)
            );
            let repo = drift_common::ecs_drifted(&label, |repo| {
                repo.replace(
                    drift_common::STAMINA,
                    "(stamina.current + rate * dt).min(stamina.max)",
                    "(stamina.current + rate * dt * 2.0).min(stamina.max)",
                );
                repo.write(
                    "crates/game/src/stray.rs",
                    "// @implements PERK-NOPE\npub fn stray() {}\n",
                );
                repo.append("crates/game/src/lib.rs", "pub mod stray;\n");
                verify_common::set_mode(&repo.root, mode);
            });
            let slug = "verify-ecs";
            let id = (*through == "approve").then(|| item_of(&raised(&repo, &[]), "PERK-REGEN"));
            let hook_home = repo.scratch.home("hook");
            repo.hook(
                "pre-commit",
                &format!(
                    "HOME='{}' '{}' verify {flag}",
                    hook_home.display(),
                    common::SPEC
                ),
            );
            if gone {
                fs::remove_file(repo.root.join("docs/records/TUNE-01.md"))
                    .expect("remove TUNE-01's document");
            }
            let before = repo.state();
            match &id {
                Some(id) => {
                    let approved = approve_yes(&repo, id);
                    if passes {
                        approved.done(&context);
                        assert_eq!(
                            proposal(&repo, slug, id).status,
                            ProposalStatus::Applied,
                            "{context}"
                        );
                    } else {
                        approved.refused(&context);
                        assert_eq!(
                            proposal(&repo, slug, id).status,
                            ProposalStatus::Open,
                            "{context}"
                        );
                    }
                }
                None => {
                    let attempt = repo.accept("PERK-REGEN", true);
                    if passes {
                        attempt.committed(&context);
                    } else {
                        attempt.refused(&context);
                    }
                }
            }
            if passes {
                assert_eq!(repo.changed(&repo.head()), ["spec.lock"], "{context}");
            } else {
                assert_eq!(repo.state(), before, "{context}: restored");
            }
        }
    }
}

/// TE-8: `--editorial` after a text commit made with `--no-verify`:
/// `spec verify` reads `LAW-BOUND`'s rows `ok` (but the fixture's own
/// `bad_marker` and `cannot_verify` rows).
#[test]
fn editorial_after_a_no_verify_text_commit_reads_ok() {
    let repo = golden("da-no-verify");
    repo.hook("pre-commit", "exit 1");
    same_revision(&repo);
    repo.git.add_all(&repo.root);
    repo.git(&["commit", "-q", "--no-verify", "-m", "the text"]);
    fs::remove_file(repo.root.join(".git/hooks/pre-commit")).unwrap();
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    approve_at(&repo, &repo.root, &id, true, true).done("editorial");
    let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
    let document = run.json();
    let states: Vec<&str> = document["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["node"] == "LAW-BOUND" && !row["lock"].is_null())
        .map(|row| row["state"].as_str().unwrap())
        .collect();
    assert!(!states.is_empty());
    assert!(states.iter().all(|state| *state == "ok"), "{states:?}");
    let _ = DECIDED_DAY;
}

/// RD-6 ("Accept's seams": `written` is `#[serde(skip)]`): `spec --json
/// lock accept` on a terminal still prints the document's eight keys in
/// order, `reference`, `node`, `rev`, `mode`, `rows`, `commit`,
/// `unchanged`, `refusal`; neither `written` nor `nothing`. M: `written`
/// serialized.
#[test]
fn lock_accept_json_keeps_its_keys() {
    let repo = LockRepo::code_a("da-json-keys");
    let run = repo.spec_pty(
        &repo.root,
        &["--json", "lock", "accept", "PIECE-COUNTER"],
        Some("y"),
    );
    run.code(0);
    assert!(run.asked, "{}", run.show());
    let document: Value = serde_json::from_str(run.stdout.trim_end())
        .unwrap_or_else(|error| panic!("{error}: {}", run.show()));
    let mut keys: Vec<&str> = document
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "commit",
            "mode",
            "node",
            "reference",
            "refusal",
            "rev",
            "rows",
            "unchanged"
        ],
        "{document}"
    );
    let printed = run.stdout.trim_end();
    let order = [
        "reference",
        "node",
        "rev",
        "mode",
        "rows",
        "commit",
        "unchanged",
        "refusal",
    ];
    let at: Vec<usize> = order
        .iter()
        .map(|key| printed.find(&format!("\"{key}\":")).expect(key))
        .collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{printed}");
    assert_eq!(document["commit"], json!(repo.head()));
}

// ------------------------------------------------------------- RD-20

/// RD-20 (Q-D): the restore commands an accept prints name the lock as a
/// quoted literal pathspec: a project at `proj[1]/` of its worktree, a
/// first lock, a `pre-commit` hook that edits the lock then fails: the
/// refusal names `` `git rm --cached -- ':(literal)proj[1]/spec.lock'` ``.
/// M: a raw pathspec.
#[test]
fn lock_restore_commands_quote_the_path() {
    let scratch = common::Scratch::new("da-quote");
    let git = common::git::Sandbox::new(scratch.path());
    let top = scratch.dir("top");
    common::copy_dir(&common::fixture("code-a"), &top.join("proj[1]"));
    fs::remove_file(top.join("proj[1]/expected.json")).unwrap();
    git.init(&top);
    git.add_all(&top);
    git.commit(&top, "the fixture in a subdirectory");
    let home = scratch.home("h");
    let repo = LockRepo {
        root: top.join("proj[1]"),
        scratch,
        git,
        home,
    };
    let hooks = top.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf '# edited by a hook\\n' >> 'proj[1]/spec.lock'\nexit 1\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let refusal = attempt.refused("an edited first lock");
    assert!(
        refusal.contains("`git rm --cached -- ':(literal)proj[1]/spec.lock'`"),
        "{refusal}"
    );
    assert!(!refusal.contains("-- proj[1]/spec.lock"), "{refusal}");
}

// ------------------------------------------------- rows as stored (RD-12)

/// The item's stored `drift` column rewritten as a later build could have
/// stored it: its `code_ahead` row (`CAP`) twice, first with its keys reversed (a
/// non-alphabetical order, neither verify's nor the view's), then in
/// verify's order; each with 5b's `levels` and 5c's `anchor` (a nested
/// object) after `compared`. Returns the column and the two rows' texts.
fn rows_of_a_later_build(repo: &LockRepo, id: &str) -> (String, [String; 2]) {
    let column = sql(
        repo,
        SLUG,
        &format!("SELECT drift FROM proposals WHERE id = '{id}';"),
    );
    // `CAP`'s row as the raise stored it, cut from the column's text.
    let head = "{\"node\":\"LAW-BOUND\",\"relation\":\"configures\",\
                \"file\":\"crates/tally/src/counter.rs\",\"kind\":\"item\",\
                \"symbol\":\"crates/tally::counter::CAP\",";
    let tail = "\"compared\":\"ast\"}";
    let from = column
        .find(head)
        .unwrap_or_else(|| panic!("CAP's row: {column}"));
    let to = from + column[from..].find(tail).expect("its end") + tail.len();
    let row = &column[from..to];
    assert!(
        row.contains("\"markers\":[{\"line\":8,\"rev\":2}],\"node_rev\":2,"),
        "integers as written: {row}"
    );
    let commit_at = row.find("\"commit\":\"").expect("its lock's commit") + 10;
    let commit = &row[commit_at..commit_at + 40];
    let later = "\"levels\":[\"sig\",\"body\"],\"anchor\":{\"to\":{\"line\":12,\"rev\":3},\
                 \"from\":null,\"n\":-1}";
    let in_order = format!("{},{later}}}", row.strip_suffix('}').expect("an object"));
    // The same row, its keys reversed (nested ones too).
    let reversed = format!(
        "{{\"compared\":\"ast\",\"lock\":{{\"verified_at\":\"2026-10-08\",\
         \"commit\":\"{commit}\",\"rev\":2}},\"node_rev\":2,\"markers\":[{{\"rev\":2,\
         \"line\":8}}],\"pre_existing\":true,\"class\":\"check\",\"detail\":null,\
         \"category\":\"code_changed\",\"state\":\"code_ahead\",\"item_kind\":\"const_item\",\
         \"symbol\":\"crates/tally::counter::CAP\",\"kind\":\"item\",\
         \"file\":\"crates/tally/src/counter.rs\",\"relation\":\"configures\",\
         \"node\":\"LAW-BOUND\",{later}}}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&reversed).unwrap(),
        serde_json::from_str::<Value>(&in_order).unwrap(),
        "the same row"
    );
    let rows = [reversed, in_order];
    let rewritten = format!("{{\"rows\":[{}],\"more\":0}}", rows.join(","));
    sql(
        repo,
        SLUG,
        &format!(
            "UPDATE proposals SET drift = '{}' WHERE id = '{id}';",
            rewritten.replace('\'', "''")
        ),
    );
    (rewritten, rows)
}

/// RD-12, "Review, inbox, round, stage" (`drift` `{hash, rows, more}` as
/// stored; Q-A): `spec review --json` gives `drift.rows` byte for byte as
/// the column stores them: each row's keys in their stored order (one
/// reversed, one in verify's), a later key and a nested object after
/// `compared` kept, the integers (`line`, `rev`, `node_rev`, a negative
/// one) as written; the text prints the rows read (`drift: 2 row(s)`, 4a's
/// form); a row with a known key `null` reads too. M: `StoredJson`
/// sorting an object's keys; `StoredJson` dropping a key; the review's
/// rows taken from the view (`DriftRow`).
#[test]
fn review_gives_the_rows_as_stored() {
    let repo = drifted("da-as-stored", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let hash = proposal(&repo, SLUG, &id).drift.expect("drift").hash;
    let (_, rows) = rows_of_a_later_build(&repo, &id);
    let (text, json_text, document) = review_of(&repo, &id);
    let want = format!(
        "\"drift\":{{\"hash\":\"{hash}\",\"rows\":[{}],\"more\":0}}",
        rows.join(",")
    );
    assert!(
        json_text.contains(&want),
        "drift.rows as stored:\n want {want}\n got  {json_text}"
    );
    assert_eq!(
        document["drift"]["rows"][0]["anchor"]["to"]["line"],
        json!(12)
    );
    assert_eq!(document["drift"]["rows"][1]["markers"][0]["rev"], json!(2));
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| *line == "drift: 2 row(s)")
        .unwrap_or_else(|| panic!("two rows read: {text}"));
    assert_eq!(lines[at + 1], lines[at + 2], "the same row read twice");
    assert!(lines[at + 1].starts_with("  "), "{text}");
    // A known key `null` is valid: the row reads.
    let nulled = rows[1]
        .replace("\"class\":\"check\"", "\"class\":null")
        .replace("\"compared\":\"ast\"", "\"compared\":null");
    assert_ne!(nulled, rows[1]);
    sql(
        &repo,
        SLUG,
        &format!(
            "UPDATE proposals SET drift = '{{\"rows\":[{nulled}],\"more\":0}}' WHERE id = '{id}';"
        ),
    );
    let (_, json_text, _) = review_of(&repo, &id);
    assert!(
        json_text.contains(&format!("\"rows\":[{nulled}],\"more\":0}}")),
        "{json_text}"
    );
}

// ------------------------------------------- RD-6's fifth check, kept apart

/// A `commit-msg` hook that drops every trailer line starting `keys`.
fn dropping_hook(repo: &LockRepo, keys: &[&str]) {
    let pattern = keys
        .iter()
        .map(|key| format!("^{key}: "))
        .collect::<Vec<_>>()
        .join("\\|");
    repo.hook(
        "commit-msg",
        &format!("grep -v '{pattern}' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\""),
    );
}

/// A `git` first on a scratch `PATH` that runs the real one and, when it
/// was the lock's `git -C <dir> commit --only …` and succeeded, reports a
/// failure anyway (exit 1, a line on stderr): "git commit reported failure
/// but the branch moved". Every other command is the real git's. The
/// `PATH` value.
fn failing_commit_path(repo: &LockRepo) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = repo.scratch.dir("failing-commit");
    let real = repo
        .git
        .git_program()
        .to_str()
        .expect("a UTF-8 git")
        .to_owned();
    let shim = dir.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\n'{real}' \"$@\"\ncode=$?\n\
             if [ $code -eq 0 ] && [ \"$1\" = -C ] && [ \"$3\" = commit ] && \
             [ \"$4\" = --only ]; then\n\
             echo 'error: the commit was reported failed by a test shim' >&2\nexit 1\nfi\n\
             exit $code\n"
        ),
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("executable");
    repo.git.path_with(&dir)
}

/// `spec approve ID` from the root at [`DECIDED`], `y`, its git run with
/// `path` as `PATH`.
fn approve_on_path(repo: &LockRepo, id: &str, path: std::ffi::OsString) -> drift_common::Asked {
    let mut vars = repo.git.vars();
    vars.retain(|(name, _)| name != "PATH");
    vars.push(("PATH".into(), path));
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::approve(
        &repo.env(&repo.root),
        &Globals::default(),
        &specengine_cli::ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: DECIDED.to_owned(),
            git: specengine_cli::GitEnv::new(&repo.root, vars),
        },
        &mut consent,
    );
    drift_common::Asked { outcome, questions }
}

/// [`left_by`]'s answer.
type Left = (ProposalStatus, Option<String>, String, Value, bool, String);

/// What the run left: the item's status, its `applied_commit`, the last
/// event's type and step, whether `spec.lock` in the worktree is the
/// commit's (nothing put back), and `git status --porcelain`.
fn left_by(repo: &LockRepo, id: &str, commit: &str) -> Left {
    let stored = proposal(repo, SLUG, id);
    let (kind, payload) = events(repo).last().cloned().expect("an event");
    let kept = repo.lock() == Some(repo.show_blob(commit, "spec.lock"));
    (
        stored.status,
        stored.applied_commit,
        kind,
        payload["step"].clone(),
        kept,
        repo.git_text(&["status", "--porcelain=v1", "--untracked-files=all"]),
    )
}

/// RD-6 / TE-5, `docs/features/drift-queue.md` "Accept's seams" (the
/// re-review's minor): the commit succeeded, a `commit-msg` hook dropped
/// `Proposal:` alone, so the four "is it ours" checks hold: the refusal is
/// the untrailed one (``the lock commit <C> on `main` is this run's
/// spec.lock, yet it has no `Proposal: PR-0001` trailer: it is unverified
/// and stays as git made it: `git reset --soft <C>^` undoes it, its
/// changes kept staged``), nothing put back, the worktree clean, the item
/// `approved` (an `apply_failed` at step 10). M: `Check::Untrailed`
/// mapped to `NotAlone`.
#[test]
fn untrailed_lock_commit_is_kept_unverified() {
    let repo = drifted("da-untrailed", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    dropping_hook(&repo, &["Proposal"]);
    let head = repo.head();
    let reason = approve_yes(&repo, &id).refused("Proposal: dropped");
    let commit = repo.head();
    assert_ne!(commit, head, "git made the commit");
    assert_eq!(repo.parents(&commit), [head]);
    assert_eq!(
        reason,
        format!(
            "`PR-0001` not applied (step 10): the lock commit {commit} on `main` is this run's \
             spec.lock, yet it has no `Proposal: PR-0001` trailer: it is unverified and stays as \
             git made it: `git reset --soft {commit}^` undoes it, its changes kept staged"
        )
    );
    assert_eq!(
        left_by(&repo, &id, &commit),
        (
            ProposalStatus::Approved,
            None,
            "proposal.apply_failed".to_owned(),
            json!(10),
            true,
            String::new()
        )
    );
}

/// RD-6 / TE-5 (the re-review's minor, its two faults at once): `git
/// commit` reports a failure though it moved `main` (a `PATH` shim), and
/// a hook dropped `Proposal:` alone: the four checks hold, so the commit
/// is this run's: the refusal is the commit's failure, then the untrailed
/// text; no restore (the worktree's `spec.lock` the commit's, the tree
/// clean), the item `approved` (`apply_failed` at step 10), never
/// reopened. M: `Check::Untrailed` mapped to `NotAlone` (the old bytes put
/// back, the item reopened at step 9).
#[test]
fn failed_commit_that_moved_main_keeps_an_untrailed_lock_commit() {
    let repo = drifted("da-untrailed-failed", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    dropping_hook(&repo, &["Proposal"]);
    let head = repo.head();
    let path = failing_commit_path(&repo);
    let reason = approve_on_path(&repo, &id, path).refused("a failed commit that moved main");
    let commit = repo.head();
    assert_ne!(commit, head, "git made the commit");
    assert!(!repo.message(&commit).contains("Proposal:"));
    assert!(
        reason.starts_with("`PR-0001` not applied (step 10): the commit of spec.lock failed (")
            && reason.contains("the commit was reported failed by a test shim"),
        "{reason}"
    );
    assert!(
        reason.ends_with(&format!(
            "); the lock commit {commit} on `main` is this run's spec.lock, yet it has no \
             `Proposal: PR-0001` trailer: it is unverified and stays as git made it: `git \
             reset --soft {commit}^` undoes it, its changes kept staged"
        )),
        "{reason}"
    );
    assert_eq!(
        left_by(&repo, &id, &commit),
        (
            ProposalStatus::Approved,
            None,
            "proposal.apply_failed".to_owned(),
            json!(10),
            true,
            String::new()
        ),
        "{reason}"
    );
}

/// RD-6: a failing four-check is still "not this run's", with the missing
/// caller trailer among its reasons, after the four's: a hook dropping
/// both `Lock:` and `Proposal:` on the succeeded commit (``is not this
/// run's spec.lock alone (it has no `Lock: LAW-BOUND` trailer; it has no
/// `Proposal: PR-0001` trailer)``, kept, the item `approved`), and on a
/// commit reported failed that moved `main` (``which is not this lock
/// commit (…; …)``, the bytes put back as for another run's commit). M:
/// the caller's trailers left out of a `NotAlone`'s reasons.
#[test]
fn four_check_failure_names_the_missing_trailer_too() {
    let reasons = "(it has no `Lock: LAW-BOUND` trailer; it has no `Proposal: PR-0001` trailer)";
    let repo = drifted("da-not-alone", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    dropping_hook(&repo, &["Lock", "Proposal"]);
    let reason = approve_yes(&repo, &id).refused("Lock: and Proposal: dropped");
    let commit = repo.head();
    assert!(
        reason.starts_with(&format!(
            "`PR-0001` not applied (step 10): the lock commit {commit} on `main` is not this \
             run's spec.lock alone {reasons}; it stays as git made it: "
        )),
        "{reason}"
    );
    let left = left_by(&repo, &id, &commit);
    assert_eq!(
        (left.0, left.3, left.4),
        (ProposalStatus::Approved, json!(10), true)
    );

    let repo = drifted("da-not-alone-failed", cap_changed);
    let raised = drift_common::raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    dropping_hook(&repo, &["Lock", "Proposal"]);
    let path = failing_commit_path(&repo);
    let reason = approve_on_path(&repo, &id, path).refused("both dropped, commit failed");
    let commit = repo.head();
    assert!(
        reason.contains(&format!(
            "`main` moved to {commit}, which is not this lock commit {reasons}: "
        )),
        "{reason}"
    );
}

// ------------------------------------- RD-9's completion, as accept commits

/// The lock commit a killed `spec approve` (`editorial`: `--editorial`)
/// of `id` left: the library accept naming the item and `main`, consent
/// yes, not recorded. The commit.
fn killed_lock_commit(repo: &LockRepo, node: &str, id: &str, editorial: bool) -> String {
    let mut consent = |_: &LockPrompt<'_>| true;
    let outcome = lock_accept(
        &repo.env(&repo.root),
        &Globals::default(),
        &LockAcceptRequest {
            reference: node.to_owned(),
            editorial,
            now: DECIDED.to_owned(),
            git: repo.git_env(&repo.root),
            proposal: Some(id.to_owned()),
            branch: Some("main".to_owned()),
        },
        &mut consent,
        &[],
    )
    .expect("the accept");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.commit.expect("a commit")
}

/// `git commit` (`amend`: `--amend`) with `message` from a scratch file.
fn commit_message(repo: &LockRepo, message: &str, amend: bool) -> String {
    let file = repo.scratch.join("message.txt");
    fs::write(&file, message).expect("the message");
    let file = file.to_str().expect("a UTF-8 path").to_owned();
    let mut args = vec!["commit", "-q", "--cleanup=verbatim", "-F", file.as_str()];
    if amend {
        args.push("--amend");
    }
    repo.git(&args);
    repo.head()
}

/// The item `id` on a branch whose tip `commit` carries `Proposal: <id>`
/// but is no accept's lock commit (`why`): a reject is not refused by it
/// (docs/features/drift-follow-up.md Q11): asked, declined here, it notes
/// the commit and `why`, the item still `open` (`drift_follow_up.rs`
/// rejects such items); then the approve is never completed by it: it runs
/// its own accept, which `then` judges (`Some(true)`: a new lock commit
/// on top applies the item; `Some(false)`: nothing to accept, refused, no
/// commit, `open`).
fn not_completed_by(repo: &LockRepo, id: &str, commit: &str, why: &str, new_commit: bool) {
    let mut questions = Vec::new();
    let declined = specengine_cli::reject(
        &repo.env(&repo.root),
        &Globals::default(),
        &specengine_cli::RejectRequest {
            id: id.to_owned(),
            reason: Some("Not drift.".to_owned()),
            now: DECIDED.to_owned(),
            git: repo.git_env(&repo.root),
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            false
        },
    )
    .unwrap_or_else(|error| panic!("{why}: {error}"));
    assert_eq!(questions.len(), 1, "{why}: asked once");
    assert_eq!(
        declined.refusal.as_deref(),
        Some(format!("`{id}` not rejected: the answer was not `y`; nothing changed").as_str()),
        "{why}"
    );
    let note = format!(
        "the commit {commit} on `main` carries `Proposal: {id}` but does not complete it \
         ({why}); rejected all the same"
    );
    assert!(
        declined.document.notes.contains(&note),
        "{why}: {:?}",
        declined.document.notes
    );
    assert_eq!(
        proposal(repo, SLUG, id).status,
        ProposalStatus::Open,
        "{why}"
    );
    let approved = approve_yes(repo, id);
    let stored = proposal(repo, SLUG, id);
    assert_ne!(
        stored.applied_commit.as_deref(),
        Some(commit),
        "{why}: completed by a commit that is no accept's"
    );
    if new_commit {
        approved.done(why);
        let head = repo.head();
        assert_eq!(
            repo.parents(&head),
            [commit.to_owned()],
            "{why}: its own commit"
        );
        assert_eq!(stored.status, ProposalStatus::Applied, "{why}");
        assert_eq!(
            stored.applied_commit.as_deref(),
            Some(head.as_str()),
            "{why}"
        );
    } else {
        let reason = approved.refused(why);
        assert!(
            reason.starts_with(&format!(
                "{id} not applied: nothing to accept now for LAW-BOUND"
            )),
            "{why}: {reason}"
        );
        assert_eq!(repo.head(), commit, "{why}: no new commit");
        assert_eq!(stored.status, ProposalStatus::Open, "{why}");
    }
}

/// RD-9, `docs/features/drift-queue.md` "Approve" (iteration 2: the drift
/// arm mirrors accept's step 10): a `Proposal:` commit on the item's
/// branch completes it only as accept commits: a commit deleting
/// `spec.lock` that carries every trailer (`Lock`, `Lock-mode: accept`,
/// `Decided-by`, `Proposal`) does not (`has no spec.lock`: the approve
/// makes its own lock commit on top); this run's lock commit without
/// `Lock-mode`, or with `Lock-mode: regen`, does not (``has no `Lock-mode:
/// accept` or `editorial` trailer``: the approve finds nothing to accept,
/// the item stays `open`); a reject notes each, no longer refused by it
/// (drift-follow-up Q11). M: the blob check removed; the `Lock-mode` check
/// removed or taking any value.
#[test]
fn only_an_accepts_lock_commit_completes_a_drift_item() {
    // A commit deleting `spec.lock`, every trailer on it.
    let repo = drifted("da-complete-deleted", cap_changed);
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    repo.git(&["rm", "-q", "--", "spec.lock"]);
    let commit = commit_message(
        &repo,
        &format!(
            "spec: lock LAW-BOUND\n\nLock: LAW-BOUND\nLock-mode: accept\nDecided-by: \
             {DECIDER}\nProposal: {id}\n"
        ),
        false,
    );
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(repo.lock(), None);
    not_completed_by(&repo, &id, &commit, "has no spec.lock", true);

    // This run's lock commit, its `Lock-mode` dropped or rewritten.
    for mode in [None, Some("regen")] {
        let repo = drifted(
            &format!("da-complete-{}", mode.unwrap_or("none")),
            cap_changed,
        );
        let raised = drift_common::raised(&repo, &[]);
        let id = item_of(&raised, "LAW-BOUND");
        let made = killed_lock_commit(&repo, "LAW-BOUND", &id, false);
        let message = repo.message(&made);
        assert!(message.contains("\nLock-mode: accept\n"), "{message}");
        let rewritten = match mode {
            None => message.replace("Lock-mode: accept\n", ""),
            Some(mode) => message.replace("Lock-mode: accept\n", &format!("Lock-mode: {mode}\n")),
        };
        let commit = commit_message(&repo, &rewritten, true);
        assert_eq!(repo.changed(&commit), ["spec.lock"], "{mode:?}");
        assert!(repo.message(&commit).contains(&format!("Proposal: {id}")));
        not_completed_by(
            &repo,
            &id,
            &commit,
            "has no `Lock-mode: accept` or `editorial` trailer",
            false,
        );
    }
}

/// RD-9: a genuine editorial lock commit of a killed `spec approve
/// --editorial` (`Lock-mode: editorial`) completes the item by its
/// trailer, as an accept's does (`killed_drift_approve_completes_by_trailer`):
/// no new commit, `applied` with it. M: the `Lock-mode` check taking
/// `accept` alone.
#[test]
fn killed_editorial_drift_approve_completes_by_trailer() {
    let repo = drifted("da-killed-editorial", |repo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
    });
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "LAW-BOUND");
    let commit = killed_lock_commit(&repo, "LAW-BOUND", &id, true);
    assert!(
        repo.message(&commit).contains("\nLock-mode: editorial\n"),
        "{}",
        repo.message(&commit)
    );
    let lock = repo.lock();
    approve_at(&repo, &repo.root, &id, true, true).done("completion of an editorial");
    assert_eq!(repo.head(), commit, "no new commit");
    assert_eq!(repo.lock(), lock);
    let stored = proposal(&repo, SLUG, &id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
}
