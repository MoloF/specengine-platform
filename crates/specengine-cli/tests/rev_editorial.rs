//! docs/features/rev-discipline.md RD-2 (`spec approve --editorial`: the
//! vouching `spec.lock` entries refreshed in the apply commit), RD-8's
//! `same_revision_hint_names_a_raise_and_editorial` and TE-1 (apply through
//! a `pre-commit` hook running `spec check --staged --root .`), through the
//! library over scratch repositories of two fixtures of different nature
//! (ADR-0008):
//!
//! - `fixtures/code-a` (a Rust library) with the golden lock
//!   `fixtures/spec-lock/code-a.lock` (format 2) committed on top
//!   (`verify_common::golden`): `PIECE-COUNTER`, a document at `rev: 3`,
//!   holds four entries vouching for its text;
//! - `fixtures/verify-ecs` (an ECS-style game crate) with its four nodes
//!   accepted (`verify_common::ecs`), which verifies `clean`.
//!
//! Proposals are raised and approved in the root's worktree on `main`, the
//! owner's question recorded. The expected locks, trailers and questions
//! are built from the spec's "Editorial refresh", never from the output.

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_common;
mod verify_common;

use std::fs;

use common::proposal::edit;
use lock_common::{DECIDER, LockRepo, node_blocks};
use rev_common::{At, after_base, approve_at, proposed, trailers};
use specengine_cli::{Globals, ShowRequest, show};
use verify_common::{ECS, STAMINA_MD, ecs, golden, verify};

const COUNTER_MD: &str = "docs/spec/counter.md";
const PIECE: &str = "PIECE-COUNTER";
/// A change of `PIECE-COUNTER`'s own text (its first paragraph).
const MEANING: (&str, &str) = ("grows one step at a time", "grows a single step at a time");
/// The repository top of every scenario: the root.
const LOCK: &str = "spec.lock";

/// The library's view from the root.
fn at(repo: &LockRepo) -> At<'_> {
    At::new(&repo.root, &repo.home, repo.git_env(&repo.root))
}

/// `(span_hash, text, norm_hash)` of `reference` as `spec show` gives them.
fn shown(repo: &LockRepo, reference: &str) -> (String, String, String) {
    let outcome = show(
        &repo.env(&repo.root),
        &Globals::default(),
        &ShowRequest {
            reference: reference.to_owned(),
            links: false,
            archive: false,
            bindings: false,
            git: repo.git_env(&repo.root),
        },
    )
    .unwrap_or_else(|error| panic!("show {reference}: {error}"));
    let node = outcome.nodes.into_iter().next().expect("one node");
    (
        node.span_hash,
        node.text,
        node.norm_hash.expect("a norm hash"),
    )
}

/// Proposes `reference`'s span with `(from, to)` applied: its ID.
fn propose_edit(repo: &LockRepo, reference: &str, (from, to): (&str, &str)) -> String {
    let (hash, text, _) = shown(repo, reference);
    proposed(&at(repo), reference, &hash, &edit(&text, from, to))
}

/// The apply question's tail after the preview, `id` on `main` at the root.
fn question(repo: &LockRepo, id: &str, path: &str, tail: &str) -> String {
    format!(
        "apply {id} to {path} on main in {} ({tail})? [y/N]",
        repo.root.display()
    )
}

/// `text` with `node`'s `n`-th block's `spec_hash` (0-based, in file
/// order) replaced by `fake`.
fn with_hash_of_block(text: &str, node: &str, n: usize, fake: &str) -> String {
    let head = format!("[[binding]]\nnode = \"{node}\"\n");
    let at = text
        .match_indices(&head)
        .nth(n)
        .unwrap_or_else(|| panic!("no block {n} of {node}"))
        .0;
    let line = at + text[at..].find("spec_hash = \"").expect("spec_hash");
    let end = line + text[line..].find('\n').expect("its line end");
    format!("{}spec_hash = \"{fake}\"{}", &text[..line], &text[end..])
}

/// A `b3:` digest no text has.
fn fake_hash() -> String {
    format!("b3:{}", "0".repeat(64))
}

/// The paths `commit` changes against its parent, sorted.
fn changed_paths(repo: &LockRepo, commit: &str) -> Vec<String> {
    let mut paths = repo.changed(commit);
    paths.sort();
    paths
}

/// The golden repository with one `PIECE-COUNTER` block (its second)
/// made non-vouching (another `spec_hash` at rev 3) and committed.
fn golden_with_a_stale_entry(label: &str) -> (LockRepo, String) {
    let repo = golden(label);
    let tampered = with_hash_of_block(&repo.lock_text(), PIECE, 1, &fake_hash());
    repo.write(LOCK, &tampered);
    repo.commit_all("an earlier unreviewed change left an entry");
    (repo, tampered)
}

// ------------------------------------------------------------------ RD-2

/// RD-2: `spec approve --editorial` on a locked node makes one commit, its
/// one parent the old `HEAD`, changing exactly the text and `spec.lock`;
/// the trailers after `Base-commit:` are `Revision: ID 3 (editorial)`,
/// `Lock: ID`, `Lock-mode: editorial`; the committed lock is the
/// worktree's, each refreshed `spec_hash` the committed text's
/// `norm_hash`; the revision kept; `applied` with that commit.
/// M: a second commit for the lock.
#[test]
fn editorial_apply_is_one_commit_of_text_and_lock() {
    let repo = golden("rd2-one-commit");
    let old_head = repo.head();
    let (_, _, old_norm) = shown(&repo, PIECE);
    let id = propose_edit(&repo, PIECE, MEANING);
    let asked = approve_at(&at(&repo), &id, true, true);
    let outcome = asked.applied("an editorial approve");
    let head = repo.head();
    assert_eq!(
        repo.parents(&head),
        std::slice::from_ref(&old_head),
        "one commit"
    );
    assert_eq!(changed_paths(&repo, &head), [COUNTER_MD, LOCK]);
    assert_eq!(
        after_base(&repo.message(&head)),
        [
            "Revision: PIECE-COUNTER 3 (editorial)",
            "Lock: PIECE-COUNTER",
            "Lock-mode: editorial"
        ]
    );
    let all = trailers(&repo.message(&head));
    assert_eq!(all[0], format!("Proposal: {id}"));
    assert_eq!(all[1], format!("Decided-by: {DECIDER}"));
    assert_eq!(repo.show_blob(&head, LOCK), repo.lock().expect("the lock"));
    let (_, text, new_norm) = shown(&repo, PIECE);
    assert_ne!(new_norm, old_norm);
    assert!(text.contains("rev: 3\n"), "kept: {text}");
    let lock = repo.lock_text();
    let blocks = node_blocks(&lock, PIECE);
    assert_eq!(blocks.len(), 4);
    for block in &blocks {
        assert!(
            block.contains(&format!("spec_hash = \"{new_norm}\"")),
            "{block}"
        );
    }
    assert_eq!(
        outcome.document.applied_commit.as_deref(),
        Some(head.as_str())
    );
    assert_eq!(repo.porcelain(), b"", "the worktree clean");
}

/// RD-2: the refresh sets `spec_hash` on the vouching entries and nothing
/// else: the code fields (format 2's `levels`, digests, fingerprints,
/// `body_bytes`), `file_blob`, `commit`, `verified_*`, every other node's
/// entries and the header byte for byte; the lock is the old one with
/// exactly that value replaced. M: a code field touched (recomputed).
#[test]
fn editorial_refresh_keeps_code_hashes_and_commit() {
    let repo = golden("rd2-code-kept");
    let before = repo.lock_text();
    let (_, _, old_norm) = shown(&repo, PIECE);
    let id = propose_edit(&repo, PIECE, MEANING);
    approve_at(&at(&repo), &id, true, true).applied("an editorial approve");
    let (_, _, new_norm) = shown(&repo, PIECE);
    assert_eq!(
        before.matches(&old_norm).count(),
        4,
        "only PIECE-COUNTER's four entries hold its old hash"
    );
    assert_eq!(repo.lock_text(), before.replace(&old_norm, &new_norm));
}

/// RD-2 (Q10): an entry at the node's revision whose `spec_hash` is not
/// its old `norm_hash` (an earlier unreviewed change) vouches for nothing:
/// left byte for byte, the other three refreshed; the question counts
/// three. M: an unfiltered refresh (every entry at R).
#[test]
fn only_vouching_entries_are_refreshed() {
    let (repo, tampered) = golden_with_a_stale_entry("rd2-vouching");
    let stale = node_blocks(&tampered, PIECE)[1].clone();
    let (_, _, old_norm) = shown(&repo, PIECE);
    let id = propose_edit(&repo, PIECE, MEANING);
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("an editorial approve");
    assert_eq!(
        asked.question(),
        question(
            &repo,
            &id,
            COUNTER_MD,
            "applies, editorial: PIECE-COUNTER stays 3; spec.lock: 3 entries"
        )
    );
    let (_, _, new_norm) = shown(&repo, PIECE);
    let lock = repo.lock_text();
    let blocks = node_blocks(&lock, PIECE);
    assert_eq!(blocks[1], stale, "the non-vouching entry byte for byte");
    assert_eq!(lock, tampered.replace(&old_norm, &new_norm));
}

/// RD-2: the refresh reads the lock without the build's code recipe: a
/// format-2 lock written under another `code_recipe` is refreshed, that
/// line kept (docs/features/lock-levels.md "Readers", Q29: "refreshed:
/// code fields, `code_recipe` kept"); another `norm_recipe` or a `format`
/// past 2 exits 2 naming the lock's and this build's, nothing written. M:
/// accept's `read_lock` (any other recipe refused).
#[test]
fn editorial_refresh_ignores_code_recipe() {
    let repo = golden("rd2-code-recipe");
    let other = "code_recipe = \"specengine-hash/v3 tree-sitter-rust/0.20.0 abi/14\"";
    let current = "code_recipe = \"specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15\"";
    let text = repo.lock_text().replacen(current, other, 1);
    repo.write(LOCK, &text);
    repo.commit_all("a lock of another code recipe");
    let (_, _, old_norm) = shown(&repo, PIECE);
    let id = propose_edit(&repo, PIECE, MEANING);
    approve_at(&at(&repo), &id, true, true).applied("another code recipe");
    let (_, _, new_norm) = shown(&repo, PIECE);
    assert_eq!(repo.lock_text(), text.replace(&old_norm, &new_norm));

    for (label, from, to, named) in [
        (
            "norm",
            "norm_recipe = \"specengine-norm/v1\"",
            "norm_recipe = \"specengine-norm/v0\"",
            ["specengine-norm/v0", "specengine-norm/v1"],
        ),
        ("format", "format = 2", "format = 9", ["9", "format"]),
    ] {
        let repo = golden(&format!("rd2-recipe-{label}"));
        let text = repo.lock_text().replacen(from, to, 1);
        repo.write(LOCK, &text);
        repo.commit_all("a lock of another recipe");
        let id = propose_edit(&repo, PIECE, MEANING);
        let before = repo.state();
        let asked = approve_at(&at(&repo), &id, true, true);
        let message = asked.cannot(label);
        for part in named {
            assert!(message.contains(part), "{label}: {part:?} in {message}");
        }
        assert!(asked.questions.is_empty(), "{label}: before the question");
        assert_eq!(repo.state(), before, "{label}: nothing written");
    }
}

/// RD-2: under `--editorial` the lock must be tracked and equal to `HEAD`:
/// modified, staged, deleted or untracked refuses at step 3, exit 1 naming
/// its state, nothing written, before the question; a meaning approve does
/// not read the lock. M: the lock read from the worktree unchecked.
#[test]
fn dirty_lock_refuses_editorial_apply() {
    for (state, dirty) in [
        ("modified", "modified"),
        ("staged", "staged"),
        ("deleted", "deleted"),
        ("untracked", "untracked"),
    ] {
        let repo = golden(&format!("rd2-dirty-{state}"));
        let lock = repo.lock_text();
        match state {
            "modified" => repo.write(LOCK, format!("{lock}\n")),
            "staged" => {
                repo.write(LOCK, format!("{lock}\n"));
                repo.git(&["add", LOCK]);
            }
            "deleted" => fs::remove_file(repo.root.join(LOCK)).unwrap(),
            _ => {
                repo.git(&["rm", "-q", "--cached", LOCK]);
                repo.git(&["commit", "-q", "-m", "the lock untracked"]);
            }
        }
        let id = propose_edit(&repo, PIECE, MEANING);
        let before = repo.state();
        let asked = approve_at(&at(&repo), &id, true, true);
        let reason = asked.refused(state);
        assert!(reason.contains("(step 3)"), "{state}: {reason}");
        assert!(
            reason.contains("spec.lock") && reason.contains(dirty),
            "{state}: {reason}"
        );
        assert!(asked.questions.is_empty(), "{state}: before the question");
        assert_eq!(repo.state(), before, "{state}: nothing written");
    }
    // A meaning approve leaves the lock alone, dirty or not.
    let repo = golden("rd2-dirty-meaning");
    let dirty = format!("{}\n", repo.lock_text());
    repo.write(LOCK, &dirty);
    let id = propose_edit(&repo, PIECE, MEANING);
    approve_at(&at(&repo), &id, false, true).applied("a meaning approve");
    assert_eq!(repo.lock_text(), dirty);
    assert_eq!(changed_paths(&repo, &repo.head()), [COUNTER_MD]);
}

/// RD-2: no `spec.lock`: `--editorial` writes the text alone (one path,
/// no `Lock:` trailer, no lock made); a lock holding no entry of the node:
/// the text alone too, the lock untouched, the question naming no lock.
/// M: a lock created, or `Lock-mode` written with no entry refreshed.
#[test]
fn editorial_without_a_lock_writes_text_only() {
    let repo = LockRepo::code_a("rd2-no-lock");
    let id = propose_edit(&repo, PIECE, MEANING);
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("no lock");
    assert_eq!(
        asked.question(),
        question(
            &repo,
            &id,
            COUNTER_MD,
            "applies, editorial: PIECE-COUNTER stays 3"
        )
    );
    let head = repo.head();
    assert_eq!(changed_paths(&repo, &head), [COUNTER_MD]);
    assert_eq!(
        after_base(&repo.message(&head)),
        ["Revision: PIECE-COUNTER 3 (editorial)"]
    );
    assert!(repo.lock().is_none(), "no lock made");

    let repo = golden("rd2-no-entry");
    let lock = repo.lock_text();
    let id = propose_edit(
        &repo,
        "LAW-STEP",
        ("Each step adds one.", "Each step adds 1."),
    );
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("no entry of the node");
    assert_eq!(
        asked.question(),
        question(
            &repo,
            &id,
            COUNTER_MD,
            "applies, editorial: LAW-STEP stays 1"
        )
    );
    let head = repo.head();
    assert_eq!(changed_paths(&repo, &head), [COUNTER_MD]);
    assert_eq!(
        after_base(&repo.message(&head)),
        ["Revision: LAW-STEP 1 (editorial)"]
    );
    assert_eq!(repo.lock_text(), lock);
}

/// RD-2: the editorial question names what stays and the lock write
/// (`; spec.lock: N entries`), only when entries change; a meaning
/// approve's names the raise and no lock. M: the lock part dropped.
#[test]
fn editorial_prompt_names_the_lock_write() {
    let repo = golden("rd2-prompt");
    let id = propose_edit(&repo, PIECE, MEANING);
    let declined = approve_at(&at(&repo), &id, true, false);
    let reason = declined.refused("declined");
    assert!(reason.contains("not `y`"), "{reason}");
    assert_eq!(
        declined.question(),
        question(
            &repo,
            &id,
            COUNTER_MD,
            "applies, editorial: PIECE-COUNTER stays 3; spec.lock: 4 entries"
        )
    );
    let meaning = approve_at(&at(&repo), &id, false, false);
    assert_eq!(
        meaning.question(),
        question(
            &repo,
            &id,
            COUNTER_MD,
            "applies; raises PIECE-COUNTER 3 -> 4"
        )
    );
}

/// RD-2: step 10 checks each refreshed `spec_hash` against the committed
/// text blob's `norm_hash`: a `pre-commit` hook that changes the node's
/// text in the commit leaves a lock that vouches for another text: exit 1
/// naming the commit, the proposal stays `approved`.
/// M: that check dropped (a lock vouching for a text it never saw).
#[test]
fn step_10_checks_the_lock_hash_against_the_committed_blob() {
    let repo = golden("rd2-step-10-text");
    let id = propose_edit(&repo, PIECE, MEANING);
    repo.hook(
        "pre-commit",
        &format!(
            "perl -pi -e 's/never passes its cap/never passes the cap/' {COUNTER_MD} && git add \
             {COUNTER_MD}"
        ),
    );
    let old_head = repo.head();
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("the committed text is not the one vouched for");
    let head = repo.head();
    assert_ne!(head, old_head, "the commit exists");
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(reason.contains(&head), "names the commit: {reason}");
    assert!(
        String::from_utf8(repo.show_blob(&head, COUNTER_MD))
            .unwrap()
            .contains("never passes the cap"),
        "the hook's text committed"
    );
    let stored = rev_common_status(&repo, &id);
    assert_eq!(stored, "approved", "never reopened after a commit");
}

/// RD-2: a `pre-commit` hook rewriting `spec.lock` in the commit fails
/// step 10 (the committed lock blob is not the bytes confirmed): exit 1
/// naming the commit, `approved`. M: the lock blob unchecked.
#[test]
fn hook_rewriting_the_lock_fails_step_10() {
    let repo = golden("rd2-step-10-lock");
    let id = propose_edit(&repo, PIECE, MEANING);
    repo.hook(
        "pre-commit",
        "printf '# a hook was here\\n' >> spec.lock && git add spec.lock",
    );
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("a hook rewrote the lock");
    let head = repo.head();
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(reason.contains(&head), "names the commit: {reason}");
    assert!(
        String::from_utf8(repo.show_blob(&head, LOCK))
            .unwrap()
            .ends_with("# a hook was here\n")
    );
    assert_eq!(rev_common_status(&repo, &id), "approved");
}

/// RD-2: a commit deleting `spec.lock` (a hook dropping it from the
/// index) fails step 10, "it has no spec.lock": exit 1, `approved`.
/// M: a deleted lock passing as "changed".
#[test]
fn editorial_commit_deleting_the_lock_fails_step_10() {
    let repo = golden("rd2-step-10-deleted");
    let id = propose_edit(&repo, PIECE, MEANING);
    repo.hook("pre-commit", "git rm --cached -q spec.lock");
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("the commit deletes the lock");
    let head = repo.head();
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(reason.contains(&head), "names the commit: {reason}");
    assert!(reason.contains("it has no spec.lock"), "{reason}");
    assert_eq!(rev_common_status(&repo, &id), "approved");
}

/// RD-2: a failing `pre-commit` hook: both files restored to their bytes
/// (`their bytes restored`), reopened, exit 1 at step 9, the repository as
/// before; a lock the hook changed before failing is left as it is, named
/// as not restored, the text restored. M: only the text restored.
#[test]
fn failed_editorial_commit_restores_text_and_lock() {
    let repo = golden("rd2-restore");
    let id = propose_edit(&repo, PIECE, MEANING);
    repo.hook("pre-commit", "echo refused by the hook >&2; exit 1");
    let before = repo.state();
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("a failing hook");
    assert!(reason.contains("(step 9)"), "{reason}");
    assert!(reason.contains("their bytes restored"), "{reason}");
    assert_eq!(repo.state(), before, "both restored, nothing committed");
    assert_eq!(rev_common_status(&repo, &id), "open");

    let repo = golden("rd2-restore-left");
    let id = propose_edit(&repo, PIECE, MEANING);
    let text_before = repo.read(COUNTER_MD);
    repo.hook("pre-commit", "printf 'x' >> spec.lock; exit 1");
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("a hook changing the lock, then failing");
    assert!(reason.contains("(step 9)"), "{reason}");
    assert!(!reason.contains("their bytes restored"), "{reason}");
    assert!(
        reason.contains("spec.lock") && reason.contains("not restored"),
        "{reason}"
    );
    assert_eq!(repo.read(COUNTER_MD), text_before, "the text restored");
    assert!(
        repo.lock_text().ends_with('x'),
        "the lock left as the hook made it"
    );
}

/// The proposal's status in the repository's queue.
fn rev_common_status(repo: &LockRepo, id: &str) -> String {
    use specengine_store::{ProposalQueue as _, SqliteQueue};
    let db = common::data_dir(&repo.home).join("code-a.db");
    let queue = SqliteQueue::open(&db, "code-a").expect("the queue");
    queue
        .get(id)
        .expect("readable")
        .expect("the proposal")
        .status
        .as_str()
        .to_owned()
}

// ------------------------------------------------------------------ RD-8

/// RD-8: after Q7 no proposal raises `rev`: verify's `spec_ahead
/// same_revision` hint and accept's refusal of text changed at a revision
/// both say "raise its `rev` by hand, commit, then accept; or
/// `--editorial`" (the hint naming `spec bump` after "by hand" since
/// docs/features/rev-rule.md RD-4), never "a proposal raising".
/// M: the old step kept.
#[test]
fn same_revision_hint_names_a_raise_and_editorial() {
    let step = "raise its `rev` by hand, commit, then accept; or `--editorial`";
    let repo = golden("rd8-hint");
    repo.replace(COUNTER_MD, MEANING.0, MEANING.1);
    repo.commit_all("a meaning change at rev 3, no raise");
    let run = verify(&repo, &[]);
    let hint = run
        .lines()
        .into_iter()
        .find(|line| line.starts_with("hint: spec_ahead same_revision: "))
        .unwrap_or_else(|| panic!("no same_revision hint: {}", run.text.show()))
        .to_owned();
    assert!(hint.contains("raise its `rev` by hand"), "{hint}");
    assert!(
        hint.contains("commit, then accept; or `--editorial`"),
        "{hint}"
    );
    assert!(!hint.contains("a proposal raising"), "{hint}");
    let attempt = repo.accept(PIECE, true);
    let refusal = attempt.refused("text changed at rev 3");
    assert!(refusal.contains(step), "{refusal}");
    assert!(!refusal.contains("through a proposal"), "{refusal}");
}

// ------------------------------------------------------------------ TE-1

/// Installs a `pre-commit` hook running `spec check --staged --root .`
/// (the built binary), then appending `ran` to `log` when it passed.
fn check_hook(repo: &LockRepo) -> std::path::PathBuf {
    let log = repo.scratch.join("hook.log");
    repo.hook(
        "pre-commit",
        &format!(
            "\"{}\" check --staged --root . >/dev/null || exit 1\necho ran >> '{}'",
            common::SPEC,
            log.display()
        ),
    );
    log
}

/// The number of passing hook runs.
fn hook_runs(log: &std::path::Path) -> usize {
    fs::read_to_string(log).map_or(0, |text| text.lines().count())
}

/// TE-1 (code-a, golden lock): through the hook, a meaning approve of
/// `PIECE-COUNTER` (a document at `rev: 3`) raises it to 4 in place, its
/// nested `LAW-BOUND` (unchanged, `rev=2`) kept, the lock untouched; a
/// second proposal raised against rev 3 rebases and raises 4 -> 5; an
/// editorial approve of a third keeps 5. Each commit passed the hook.
#[test]
fn te1_meaning_then_rebase_through_the_hook() {
    let repo = golden("te1-meaning");
    // A second paragraph in PIECE-COUNTER's text, apart from the first.
    repo.replace(
        COUNTER_MD,
        "never passes its cap.\n",
        "never passes its cap.\n\nIt starts at zero.\n",
    );
    repo.commit_all("a second paragraph");
    // The new text is not what the golden lock vouches for: accept it.
    repo.editorial(PIECE, true)
        .committed("the paragraph is editorial");
    let log = check_hook(&repo);
    let lock = repo.lock_text();
    let first = propose_edit(&repo, PIECE, MEANING);
    let second = propose_edit(&repo, PIECE, ("starts at zero.", "starts at nought."));
    let before = repo.read(COUNTER_MD);
    let asked = approve_at(&at(&repo), &first, false, true);
    asked.applied("the meaning approve");
    assert_eq!(
        asked.question(),
        question(
            &repo,
            &first,
            COUNTER_MD,
            "applies; raises PIECE-COUNTER 3 -> 4"
        )
    );
    assert_eq!(
        repo.read(COUNTER_MD),
        edit(&edit(&before, MEANING.0, MEANING.1), "rev: 3\n", "rev: 4\n")
    );
    assert!(
        repo.read(COUNTER_MD)
            .contains("## Bound {#LAW-BOUND rev=2}")
    );
    assert_eq!(repo.lock_text(), lock, "a meaning approve writes no lock");
    assert_eq!(
        after_base(&repo.message(&repo.head())),
        ["Revision: PIECE-COUNTER 3 -> 4"]
    );
    let asked = approve_at(&at(&repo), &second, false, true);
    asked.applied("the rebased one");
    assert_eq!(
        asked.question(),
        question(
            &repo,
            &second,
            COUNTER_MD,
            "rebases; raises PIECE-COUNTER 4 -> 5"
        )
    );
    assert!(repo.read(COUNTER_MD).contains("\nrev: 5\n"));
    assert!(repo.read(COUNTER_MD).contains("starts at nought."));
    assert_eq!(hook_runs(&log), 2, "both commits passed the hook");
    assert_eq!(repo.porcelain(), b"");
}

/// TE-1 (code-a): through the hook, an editorial approve keeps rev 3,
/// refreshes the three vouching entries and leaves the stale one byte for
/// byte, in one commit that passed the hook.
#[test]
fn te1_editorial_through_the_hook_refreshes_vouching_entries() {
    let (repo, tampered) = golden_with_a_stale_entry("te1-editorial");
    let log = check_hook(&repo);
    let (_, _, old_norm) = shown(&repo, PIECE);
    let id = propose_edit(&repo, PIECE, MEANING);
    approve_at(&at(&repo), &id, true, true).applied("editorial");
    let (_, text, new_norm) = shown(&repo, PIECE);
    assert!(text.contains("\nrev: 3\n"), "{text}");
    assert_eq!(repo.lock_text(), tampered.replace(&old_norm, &new_norm));
    assert_eq!(hook_runs(&log), 1);
    assert_eq!(changed_paths(&repo, &repo.head()), [COUNTER_MD, LOCK]);
}

/// TE-1 (verify-ecs, a game crate verifying `clean`): an editorial approve
/// of `PERK-STAMINA`'s text (a document at `rev: 2`) through the hook
/// keeps its revision and refreshes its entries: `spec verify` stays
/// exit 0, `clean`, every row of the node `ok`; a CRLF file's meaning
/// approve of `PERK-REGEN` (`rev=1`) raises it to 2 keeping CRLF, the
/// nested `PERK-DRAIN` unchanged and unwritten.
#[test]
fn te1_ecs_editorial_keeps_verify_ok_and_crlf_raise() {
    let repo = ecs("te1-ecs");
    assert_eq!(verify(&repo, &[]).verdict(), "clean");
    let log = check_hook(&repo);
    let id = propose_edit(
        &repo,
        "PERK-STAMINA",
        ("spends stamina to sprint", "spends its stamina to sprint"),
    );
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("editorial on verify-ecs");
    assert!(
        asked
            .question()
            .ends_with("(applies, editorial: PERK-STAMINA stays 2; spec.lock: 2 entries)? [y/N]"),
        "{}",
        asked.question()
    );
    // `spec verify --json` as the binary (the queue now holds proposals,
    // so not `verify_common::verify`, which asserts it empty).
    let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
    assert_eq!(run.code, 0, "{}", run.show());
    let document = run.json();
    assert_eq!(document["verdict"], "clean", "{}", run.show());
    let rows: Vec<&serde_json::Value> = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["node"] == "PERK-STAMINA")
        .collect();
    assert!(!rows.is_empty(), "{}", run.show());
    for row in rows {
        assert_eq!(row["state"], "ok", "{row}");
    }

    // CRLF: the file's line ends kept, the section raised 1 -> 2.
    let crlf = repo.read(STAMINA_MD).replace('\n', "\r\n");
    repo.write(STAMINA_MD, &crlf);
    repo.commit_all("CRLF");
    let id = propose_edit(
        &repo,
        "PERK-REGEN",
        ("a fixed amount per second", "a fixed amount each second"),
    );
    let asked = approve_at(&at(&repo), &id, false, true);
    asked.applied("a CRLF raise");
    assert_eq!(
        repo.read(STAMINA_MD),
        edit(
            &edit(
                &crlf,
                "a fixed amount per second",
                "a fixed amount each second"
            ),
            "{#PERK-REGEN rev=1}",
            "{#PERK-REGEN rev=2}"
        )
    );
    assert!(repo.read(STAMINA_MD).contains("## Drain {#PERK-DRAIN}\r\n"));
    assert_eq!(
        hook_runs(&log),
        2,
        "both apply commits passed the hook: {ECS}"
    );
}

/// TE-1 (code-a): a document of a closed class whose contract admits no
/// `rev` (`GOAL-01`, made a `decision` here): through the hook, a meaning
/// approve refuses at step 6 before the question, nothing written; the
/// same proposal `--editorial` passes, keeping its vouching entries
/// refreshed.
#[test]
fn te1_closed_class_refused_then_editorial_passes() {
    let repo = golden("te1-closed");
    repo.replace(
        "docs/records/GOAL-01.md",
        "class: canon\n",
        "class: decision\n",
    );
    repo.append(
        "specengine.toml",
        "\n[classes]\ndecision = { required = [\"class\", \"id\"], optional = [\"owner\", \
         \"reviewed\"], closed = true }\n",
    );
    repo.commit_all("GOAL-01 a closed decision");
    let log = check_hook(&repo);
    let id = propose_edit(
        &repo,
        "GOAL-01",
        (
            "may step as often as it likes",
            "may step as often as it wants",
        ),
    );
    let before = repo.state();
    let asked = approve_at(&at(&repo), &id, false, true);
    let reason = asked.refused("a closed class");
    assert!(reason.contains("(step 6)"), "{reason}");
    assert!(
        reason.ends_with(
            "`GOAL-01`: class `decision` is closed and does not admit `rev`: admit it in \
             [classes], or approve with --editorial"
        ),
        "{reason}"
    );
    assert!(asked.questions.is_empty());
    assert_eq!(repo.state(), before);
    let (_, _, old_norm) = shown(&repo, "GOAL-01");
    let lock = repo.lock_text();
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("--editorial into a closed class");
    let (_, text, new_norm) = shown(&repo, "GOAL-01");
    assert!(!text.contains("rev:"), "{text}");
    assert_eq!(repo.lock_text(), lock.replace(&old_norm, &new_norm));
    assert_eq!(
        after_base(&repo.message(&repo.head())),
        [
            "Revision: GOAL-01 1 (editorial)",
            "Lock: GOAL-01",
            "Lock-mode: editorial"
        ]
    );
    assert_eq!(hook_runs(&log), 1);
}

/// RD-2 ("Editorial refresh", "Write and commit"): step 8 re-reads the
/// lock; changed since step 3 (here while the owner is asked), exit 1
/// "spec.lock changed while being applied; nothing written": the text and
/// the lock as they were, the proposal still `open`. M: the lock not read
/// again at step 8 (the other run's bytes overwritten).
#[test]
fn lock_changed_during_the_question_refuses_at_step_8() {
    let repo = golden("rd2-step-8");
    let id = propose_edit(&repo, PIECE, MEANING);
    let text_before = repo.read(COUNTER_MD);
    let changed = format!("{}# another run\n", repo.lock_text());
    let lock_path = repo.root.join(LOCK);
    let mut asked = Vec::new();
    let outcome = specengine_cli::approve_editorial(
        &repo.env(&repo.root),
        &Globals::default(),
        &specengine_cli::ApproveRequest {
            id: id.clone(),
            note: None,
            now: rev_common::LATER.to_owned(),
            git: repo.git_env(&repo.root),
        },
        &specengine_cli::ApproveFlags::default(),
        &mut |question: &str| {
            asked.push(question.to_owned());
            fs::write(&lock_path, &changed).expect("another run's lock");
            true
        },
    );
    let reason = common::proposal::refused(&outcome, "the lock changed meanwhile");
    assert_eq!(asked.len(), 1);
    assert!(reason.contains("(step 8)"), "{reason}");
    assert!(
        reason.contains("spec.lock changed while being applied; nothing written"),
        "{reason}"
    );
    assert_eq!(repo.read(COUNTER_MD), text_before, "the text not written");
    assert_eq!(repo.lock_text(), changed, "the other run's lock kept");
    assert_eq!(rev_common_status(&repo, &id), "open");
}

/// RD-2 ("Write and commit": accept's rules): a failing commit whose
/// restore of the text cannot be made (its directory made read-only by the
/// hook) is a warning naming the worktree and `git checkout --
/// ':(literal)<path>'` (the path a quoted literal pathspec), and the
/// refusal says the text "is not restored (see the warning)", never
/// "restored"; the lock is restored. M: "their bytes restored" whatever
/// the restore did.
#[test]
fn a_failed_restore_is_a_warning_naming_its_git_command() {
    let repo = golden("rd2-restore-fails");
    let id = propose_edit(&repo, PIECE, MEANING);
    let lock_before = repo.lock_text();
    repo.hook("pre-commit", "chmod a-w docs/spec; exit 1");
    let asked = approve_at(&at(&repo), &id, true, true);
    let spec_dir = repo.root.join("docs/spec");
    let mut writable = fs::metadata(&spec_dir).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut writable, 0o755);
    fs::set_permissions(&spec_dir, writable).unwrap();
    let reason = asked.refused("the text cannot be restored");
    assert!(reason.contains("(step 9)"), "{reason}");
    assert!(!reason.contains("their bytes restored"), "{reason}");
    assert!(
        reason.contains(&format!("`{COUNTER_MD}` is not restored (see the warning)")),
        "{reason}"
    );
    let outcome = asked.outcome.as_ref().unwrap();
    let head = format!(
        "warning: cannot restore `{COUNTER_MD}` in {}: ",
        repo.root.display()
    );
    let tail = format!("; `git checkout -- ':(literal){COUNTER_MD}'` restores it");
    let warned = outcome
        .messages
        .iter()
        .map(|message| message.line())
        .filter(|line| line.starts_with(&head) && line.ends_with(&tail))
        .count();
    assert_eq!(warned, 1, "{:?}", outcome.messages);
    assert_eq!(repo.lock_text(), lock_before, "the lock restored");
}

/// Makes a directory writable again when dropped (a test that made it
/// read-only cleans up even when it fails).
struct Writable(std::path::PathBuf);

impl Drop for Writable {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, std::os::unix::fs::PermissionsExt::from_mode(0o755));
    }
}

/// RD-2 ("Editorial refresh", step 8): the lock write fails (the root made
/// read-only while the owner is asked: the atomic replace has no room for
/// its sibling; the text's directory stays writable): exit 1 at step 8,
/// the text put back while it holds this run's bytes and named
/// `` `docs/spec/counter.md` restored `` (one file: never "their bytes"),
/// nothing committed, the repository as before, `open`.
/// M: the text left as written after the failed lock write.
#[test]
fn failed_lock_write_restores_the_text_at_step_8() {
    let repo = golden("rd2-step-8-lock-write");
    let id = propose_edit(&repo, PIECE, MEANING);
    let before = repo.state();
    let root = repo.root.clone();
    let guard = Writable(root.clone());
    let mut asked = Vec::new();
    let outcome = specengine_cli::approve_editorial(
        &repo.env(&repo.root),
        &Globals::default(),
        &specengine_cli::ApproveRequest {
            id: id.clone(),
            note: None,
            now: rev_common::LATER.to_owned(),
            git: repo.git_env(&repo.root),
        },
        &specengine_cli::ApproveFlags::default(),
        &mut |question: &str| {
            asked.push(question.to_owned());
            fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o555))
                .expect("a read-only root");
            true
        },
    );
    drop(guard);
    let reason = common::proposal::refused(&outcome, "the lock cannot be written");
    assert_eq!(asked.len(), 1);
    assert!(reason.contains("(step 8)"), "{reason}");
    assert!(reason.contains(&format!("cannot write {LOCK}")), "{reason}");
    assert!(
        reason.ends_with(&format!("; `{COUNTER_MD}` restored; nothing committed")),
        "{reason}"
    );
    assert!(!reason.contains("their bytes restored"), "{reason}");
    assert_eq!(repo.state(), before, "the text restored, nothing committed");
    assert_eq!(rev_common_status(&repo, &id), "open");
}

/// RD-2 step 10 (Q16): the editorial commit made, then undone by a
/// `post-commit` hook (`git reset --soft HEAD~1`), so neither the branch
/// nor `HEAD` moved: nothing is restored, the proposal stays `approved`,
/// and the refusal (step 10) ends naming both files as staying "as the
/// commit left them" (here: written and staged, the soft reset keeping the
/// commit's index), with the way back `git reset -q -- '<text>' '<lock>'`,
/// then `git checkout -- '<text>' '<lock>'`, each path a quoted
/// `:(literal)` pathspec; those two commands give the repository as it
/// was. M: the clause dropped.
#[test]
fn unmoved_branch_and_head_leave_text_and_lock_staged() {
    let repo = golden("rd2-step-10-unmoved");
    let id = propose_edit(&repo, PIECE, MEANING);
    let before = repo.state();
    let old_head = repo.head();
    let lock_before = repo.lock_text();
    repo.hook("post-commit", "git reset -q --soft HEAD~1");
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("a commit undone by a hook");
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(reason.contains("did not move"), "{reason}");
    let (text, lock) = (
        format!("':(literal){COUNTER_MD}'"),
        format!("':(literal){LOCK}'"),
    );
    let clause = format!(
        "`{id}` stays approved; `{COUNTER_MD}` and `{LOCK}` stay as the commit left them in {}: \
         `git reset -q -- {text} {lock}`, then `git checkout -- {text} {lock}` puts them back",
        repo.root.display()
    );
    assert!(
        reason.ends_with(&clause),
        "{reason}\nwant the end: {clause}"
    );
    assert_eq!(repo.head(), old_head, "HEAD back where it was");
    assert_eq!(rev_common_status(&repo, &id), "approved");
    // Nothing restored: the text and the refreshed lock as written, staged.
    assert!(
        repo.read(COUNTER_MD).contains(MEANING.1),
        "the text as written"
    );
    let (_, _, new_norm) = shown(&repo, PIECE);
    assert_ne!(repo.lock_text(), lock_before, "the lock as written");
    assert!(
        repo.lock_text()
            .contains(&format!("spec_hash = \"{new_norm}\"")),
        "the refreshed lock"
    );
    assert_eq!(
        repo.git_text(&["diff", "--cached", "--name-only"]),
        format!("{COUNTER_MD}\n{LOCK}")
    );
    // The way back the refusal names (each word as the shell passes it).
    let (text, lock) = (
        format!(":(literal){COUNTER_MD}"),
        format!(":(literal){LOCK}"),
    );
    repo.git(&["reset", "-q", "--", &text, &lock]);
    repo.git(&["checkout", "--", &text, &lock]);
    assert_eq!(repo.state(), before, "the repository as it was");
}

/// The command of `reason` that starts with `` `<start> `` up to its
/// closing backtick, as printed.
fn printed_command(reason: &str, start: &str) -> String {
    let from = reason
        .find(&format!("`{start}"))
        .unwrap_or_else(|| panic!("no `{start}` in: {reason}"))
        + 1;
    let to = from + reason[from..].find('`').expect("its closing backtick");
    reason[from..to].to_owned()
}

/// RD-2 step 10 (Q16) with a spec path a shell and git would both
/// misread unquoted: `docs/spec/count er's [1].md` (a space, a `'` and a
/// glob class). An unrelated tracked file the bare path matches as a git
/// glob, `docs/spec/count er's 1.md`, holds an unstaged edit. The two
/// commands the refusal prints, run through `sh -c` exactly as printed in
/// the worktree, each exit 0 and give back exactly the text and the lock
/// at `HEAD`: the repository as it was before the approve, the unrelated
/// file's edit kept. M: `pathspec_word` returning the bare path (the
/// shell splits it at the space and the `'`), or quoting it without
/// `:(literal)` (git's glob overwrites the unrelated edit).
#[test]
fn restore_commands_quote_literal_pathspecs() {
    let repo = golden("rd2-q16-pathspec");
    let spec = "docs/spec/count er's [1].md";
    let decoy = "docs/spec/count er's 1.md";
    repo.git(&["mv", COUNTER_MD, spec]);
    repo.write(decoy, "# Notes\n\nNothing specified here.\n");
    repo.commit_all("the counter under an awkward name, a neighbour");
    let id = propose_edit(&repo, PIECE, MEANING);
    let edited = "# Notes\n\nNothing specified here, yet; an edit in progress.\n";
    repo.write(decoy, edited);
    let before = repo.state();
    let old_head = repo.head();
    let lock_before = repo.lock_text();
    let text_before = repo.read(spec);
    repo.hook("post-commit", "git reset -q --soft HEAD~1");
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("a commit undone by a hook");
    assert!(reason.contains("(step 10)"), "{reason}");
    assert_eq!(repo.head(), old_head, "HEAD back where it was");
    assert_eq!(rev_common_status(&repo, &id), "approved");
    assert_ne!(
        repo.read(spec),
        text_before,
        "the text as the commit left it"
    );
    assert_ne!(
        repo.lock_text(),
        lock_before,
        "the lock as the commit left it"
    );
    assert_eq!(repo.read(decoy), edited, "the neighbour untouched so far");

    // The way back, pasted into a shell as printed.
    for start in ["git reset -q -- ", "git checkout -- "] {
        let command = printed_command(&reason, start);
        let output = repo
            .git
            .command("sh", &repo.root, &[])
            .arg("-c")
            .arg(&command)
            .output()
            .expect("sh runs");
        assert!(
            output.status.success(),
            "sh -c {command:?}: {}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(repo.read(decoy), edited, "the neighbour's edit kept");
    assert_eq!(repo.read(spec), text_before, "the text back at HEAD");
    assert_eq!(repo.lock_text(), lock_before, "the lock back at HEAD");
    assert_eq!(repo.git_text(&["diff", "--cached", "--name-only"]), "");
    assert_eq!(repo.state(), before, "exactly the two files given back");

    // The words, as the spec's clause has them.
    let word = |path: &str| format!("':(literal){}'", path.replace('\'', r"'\''"));
    let (text, lock) = (word(spec), word(LOCK));
    assert_eq!(text, r"':(literal)docs/spec/count er'\''s [1].md'");
    let clause = format!(
        "`{id}` stays approved; `{spec}` and `{LOCK}` stay as the commit left them in {}: \
         `git reset -q -- {text} {lock}`, then `git checkout -- {text} {lock}` puts them back",
        repo.root.display()
    );
    assert!(
        reason.ends_with(&clause),
        "{reason}\nwant the end: {clause}"
    );
}

/// RD-6 with a lock ("Review document, stage, queue"): `lock_entries`
/// counts the entries an editorial approve refreshes, the vouching ones
/// only (3 of `PIECE-COUNTER`'s 4 when one is stale); an unreadable lock
/// gives `null` and a note naming `lock_entries`.
/// M: every entry at R counted.
#[test]
fn revisions_count_the_vouching_lock_entries() {
    let (repo, _) = golden_with_a_stale_entry("rd6-lock-entries");
    let id = propose_edit(&repo, PIECE, MEANING);
    let (text, value) = rev_common::review_at(&at(&repo), &id);
    assert_eq!(
        value["revisions"],
        serde_json::json!([rev_common::revision_entry(PIECE, 3, 4, Some(3))])
    );
    assert!(
        text.contains("\nrevisions: PIECE-COUNTER 3 -> 4 (3 lock entries)\n"),
        "{text}"
    );
    repo.write(LOCK, "not a lock\n");
    let (_, value) = rev_common::review_at(&at(&repo), &id);
    assert_eq!(
        value["revisions"],
        serde_json::json!([rev_common::revision_entry(PIECE, 3, 4, None)])
    );
    let notes = value["notes"].as_array().expect("notes");
    assert!(
        notes.iter().any(|note| note
            .as_str()
            .is_some_and(|note| note.contains("lock_entries"))),
        "{notes:?}"
    );
}

/// RD-2 ("Write and commit", step 10: "exactly those two paths changed"):
/// a hook adding a third file to the editorial commit fails step 10, exit
/// 1 naming the commit, `approved`. M: the paths unchecked.
#[test]
fn a_third_path_in_the_editorial_commit_fails_step_10() {
    let repo = golden("rd2-step-10-third");
    let id = propose_edit(&repo, PIECE, MEANING);
    repo.hook("pre-commit", "echo note > NOTES.txt && git add NOTES.txt");
    let asked = approve_at(&at(&repo), &id, true, true);
    let reason = asked.refused("a third path");
    let head = repo.head();
    assert!(reason.contains("(step 10)"), "{reason}");
    assert!(reason.contains(&head), "names the commit: {reason}");
    assert!(reason.contains("NOTES.txt"), "{reason}");
    assert_eq!(rev_common_status(&repo, &id), "approved");
}
