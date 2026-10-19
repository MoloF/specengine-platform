//! docs/features/rev-rule.md RD-2 (the rule's wiring into `spec check
//! --staged` and `--changed`: which runs read which lock, through which git
//! index, in which mode, with which causes), RD-3 (4b-1's apply commits
//! under a real hook with a lock), TE-2 (`--changed` = `--staged` on equal
//! trees; plain unchanged) and TE-3 (invalid, foreign and unmerged locks;
//! two refreshes meeting in `spec lock regen`), over scratch repositories
//! of `fixtures/code-a` with its golden lock committed and of
//! `fixtures/verify-ecs` (an ECS-style game crate) with its four nodes
//! accepted. The texts are the spec's (`docs/features/rev-rule.md`
//! "Data", "Rules and edge cases").
//!
//! Left to slice 5b, the later of it and 4b-2: `format_1_head_lock_is_read`
//! and TE-3's rebase commit under `check --staged` (no format 2 yet).

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_common;
mod rev_rule_common;
mod verify_common;

use std::fs;

use common::proposal::edit;
use rev_common::{At, approve_at, proposed};
use rev_rule_common::{
    BOUND, BOUND_EDIT, BOUND_LINE, CAP_MD, COUNTER_MD, Check, GOAL, GOAL_EDIT, GOAL_MD, LOCK,
    PASS_EDIT, PIECE, PIECE_EDIT, REGEN_EDIT, STAMINA_MD, STEP_EDIT, changed, check_at, check_mode,
    commit_hooked, error_line, hook_check, hook_runs, lock_hash, node_of, plain, refresh, shown,
    stage_all, staged, with_top,
};
use serde_json::Value;
use verify_common::{ecs, golden, golden_format_1};

/// A lock no TOML reader takes.
const GARBAGE: &str = "this is not a lock\n";

/// The cause of an unmerged path under `--staged`, unchanged by 4b-2:
/// `docs/canon/spec-check-git.md` "The staged check".
const UNMERGED: &str = "spec.lock: unmerged in the git index: the conflict is not resolved";

/// Some cause names `HEAD`'s lock.
fn names_head_lock(check: &Check) -> bool {
    check
        .causes()
        .iter()
        .any(|cause| cause.contains("HEAD:spec.lock"))
}

/// Some cause names the checked lock and none `HEAD`'s.
fn names_checked_lock(check: &Check) -> bool {
    let causes = check.causes();
    causes.iter().any(|cause| cause.starts_with("spec.lock"))
        && !causes.iter().any(|cause| cause.contains("HEAD:spec.lock"))
}

/// Cannot check: exit 2, the verdict, no rule finding.
fn assert_cannot(check: &Check, context: &str) {
    assert_eq!(check.code(), 2, "{context}:\n{}", check.show());
    assert_eq!(check.verdict(), "cannot-check", "{context}");
    assert!(check.subjects().is_empty(), "{context}:\n{}", check.show());
}

// ------------------------------------------------------------------ RD-2

/// RD-2: a plain `spec check` never reads the lock: a locked node changed
/// on disk gives no finding (`--changed` on the same disk does), and its
/// bytes are the same with the lock unparseable or gone.
/// M: the rule run plain.
#[test]
fn plain_check_has_no_rev_finding() {
    let repo = golden("rr2-plain");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let first = plain(&repo);
    assert!(first.subjects().is_empty(), "{}", first.show());
    assert_eq!(first.code(), 0, "{}", first.show());
    let control = changed(&repo);
    assert_eq!(control.subjects(), [BOUND], "{}", control.show());

    let printed = |check: &Check| {
        (
            check.code(),
            check.text.stdout.clone(),
            check.json_run.stdout.clone(),
        )
    };
    repo.write(LOCK, GARBAGE);
    assert_eq!(
        printed(&plain(&repo)),
        printed(&first),
        "an unparseable lock"
    );
    fs::remove_file(repo.root.join(LOCK)).expect("remove the lock");
    assert_eq!(printed(&plain(&repo)), printed(&first), "no lock");
}

/// RD-2: under `--staged` the checked lock is the blob of the index git
/// names: `GIT_INDEX_FILE` an index holding the text change and the
/// refreshed lock passes; the default index (the text change, `HEAD`'s
/// lock) and the disk (`HEAD`'s lock) give the finding.
/// M: the lock read from `.git/index` (or the disk).
#[test]
fn rule_reads_the_lock_through_git_index_file() {
    let repo = golden("rr2-index-file");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.git(&["add", "--", COUNTER_MD]);
    let alternate = repo.scratch.join("alternate-index");
    fs::copy(repo.root.join(".git/index"), &alternate).expect("copy the index");
    refresh(&repo, BOUND, None);
    repo.git.git_env(
        &repo.root,
        &["add", "--", LOCK],
        &[("GIT_INDEX_FILE", alternate.as_os_str())],
    );
    // The disk's lock back to the default index's, `HEAD`'s.
    repo.git(&["checkout", "--", LOCK]);
    assert_eq!(
        repo.git_text(&["diff", "--name-only", "HEAD", "--", LOCK]),
        ""
    );

    let through = check_at(
        &repo,
        &repo.root,
        &["--staged"],
        &[("GIT_INDEX_FILE", alternate.as_os_str())],
    );
    assert!(through.subjects().is_empty(), "{}", through.show());
    assert_eq!(through.code(), 0, "{}", through.show());

    let default = staged(&repo);
    assert_eq!(default.subjects(), [BOUND], "{}", default.show());
    assert_eq!(default.code(), 1);
    let disk = changed(&repo);
    assert_eq!(disk.subjects(), [BOUND], "{}", disk.show());
}

/// RD-2: under `[check] mode = "observe"` (committed, so `HEAD`'s mode is
/// no stricter) the finding is in the JSON, introduced, and blocks
/// nothing: exit 0, `observed`. M: `observe` blocks.
#[test]
fn observe_never_blocks_a_rev_finding() {
    let repo = golden("rr2-observe");
    check_mode(&repo, "observe");
    repo.commit_all("observe");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    for check in [staged(&repo), changed(&repo)] {
        assert_eq!(check.code(), 0, "{}", check.show());
        assert_eq!(check.verdict(), "observed", "{}", check.show());
        assert_eq!(check.json["mode"], "observe");
        assert_eq!(check.subjects(), [BOUND], "{}", check.show());
        assert_eq!(check.finding(BOUND)["introduced"], true);
    }
}

/// RD-2: a lock of another `code_recipe` (a grammar bump not yet rebased)
/// still has its `node`, `rev`, `spec_hash` read: the finding, no cause;
/// refreshed, clean. M: `code_recipe` compared.
#[test]
fn other_code_recipe_lock_still_checks() {
    let repo = golden("rr2-code-recipe");
    let other = with_top(
        &repo.lock_text(),
        "code_recipe",
        "\"specengine-hash/v9 tree-sitter-rust/9.9.9 abi/99\"",
    );
    repo.write(LOCK, other);
    repo.commit_all("a lock of another code recipe");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.code(), 1, "{}", check.show());
    assert!(check.causes().is_empty(), "{}", check.show());
    assert_eq!(check.subjects(), [BOUND]);

    refresh(&repo, BOUND, None);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.code(), 0, "{}", check.show());
    assert!(check.subjects().is_empty());
}

/// RD-2 / TE-3: a node changed (a candidate) with a lock that cannot be
/// read as one: cannot check, exit 2, the cause naming `spec.lock` (the
/// staged lock) or `HEAD:spec.lock` (`HEAD`'s, unparseable or of a newer
/// `format`). M: an invalid lock with a candidate passes.
#[test]
fn invalid_lock_is_cannot_check() {
    let repo = golden("rr2-invalid-staged");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.write(LOCK, GARBAGE);
    stage_all(&repo);
    let check = staged(&repo);
    assert_cannot(&check, "the staged lock unparseable");
    assert!(names_checked_lock(&check), "{:?}", check.causes());
    let check = changed(&repo);
    assert_cannot(&check, "the disk's lock unparseable");
    assert!(names_checked_lock(&check), "{:?}", check.causes());

    let repo = golden("rr2-invalid-head");
    repo.write(LOCK, GARBAGE);
    repo.commit_all("an unparseable lock");
    repo.write(
        LOCK,
        fs::read(verify_common::golden_file()).expect("the golden"),
    );
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_cannot(&check, "HEAD's lock unparseable");
    assert!(names_head_lock(&check), "{:?}", check.causes());

    let repo = golden("rr2-newer-format");
    let newer = with_top(&repo.lock_text(), "format", "99");
    repo.write(LOCK, newer);
    repo.commit_all("a lock of a newer format");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_cannot(&check, "HEAD's lock of format 99");
    assert!(names_head_lock(&check), "{:?}", check.causes());
}

/// RD-2: a lock of another `norm_recipe` (its `spec_hash` values are not
/// this build's): with a candidate, cannot check naming `HEAD:spec.lock`,
/// or `spec.lock` when only the staged one is; with none, unread.
/// M: `norm_recipe` ignored.
#[test]
fn other_norm_recipe_lock_is_cannot_check() {
    let recipe = "\"specengine-norm/v2\"";
    let repo = golden("rr2-norm-head");
    let other = with_top(&repo.lock_text(), "norm_recipe", recipe);
    repo.write(LOCK, other);
    repo.commit_all("a lock of another norm recipe");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_cannot(&check, "HEAD's norm_recipe");
    assert!(names_head_lock(&check), "{:?}", check.causes());

    let repo = golden("rr2-norm-staged");
    let other = with_top(&repo.lock_text(), "norm_recipe", recipe);
    repo.write(LOCK, other);
    stage_all(&repo);
    let lock_only = staged(&repo);
    assert_eq!(lock_only.code(), 0, "no candidate: {}", lock_only.show());
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_cannot(&check, "the staged norm_recipe");
    assert!(names_checked_lock(&check), "{:?}", check.causes());
}

/// RD-2 / TE-3: with no node changed against `HEAD` neither lock is
/// parsed: an unparseable staged lock alone, a heading rename and a new
/// node beside it, then an unparseable `HEAD` lock: clean, exit 0. An
/// unlocked node's change is a candidate: then the locks are read, and
/// `HEAD`'s unparseable one is a cause. M: a lock parsed with no
/// candidate.
#[test]
fn lock_only_commit_never_parses_the_lock() {
    let repo = golden("rr2-lock-only");
    repo.write(LOCK, GARBAGE);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.code(), 0, "{}", check.show());
    assert!(check.causes().is_empty(), "{}", check.show());

    repo.replace(
        COUNTER_MD,
        "## Bound {#LAW-BOUND rev=2}",
        "## Upper bound {#LAW-BOUND rev=2}",
    );
    repo.append(
        "docs/spec/limits.md",
        "\n## Ceiling {#LAW-CEILING}\n\nA law no commit has held.\n",
    );
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.code(), 0, "{}", check.show());

    repo.commit_all("an unparseable lock, a rename, a new node");
    let check = staged(&repo);
    assert_eq!(check.code(), 0, "{}", check.show());
    let check = changed(&repo);
    assert_eq!(check.code(), 0, "{}", check.show());

    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_cannot(&check, "a candidate: the locks are read");
    assert!(names_head_lock(&check), "{:?}", check.causes());
}

// ------------------------------------------------------------------ RD-3

/// RD-3: 4b-1's applies through a `pre-commit` hook running `spec check
/// --staged --root .`, the golden lock committed: a hand edit of
/// `LAW-BOUND` is refused by that hook (the rule is live), then a meaning
/// approve of it (2 -> 3), an editorial approve of `GOAL-01` (its two
/// entries refreshed in the commit) and a meaning approve of the document
/// `PIECE-COUNTER` (3 -> 4) each pass it. 4c-1's
/// `lock_commit_passes_spec_check_staged` (`drift_approve.rs`) holds with
/// the rule inside. M: 4b-1's raise removed.
#[test]
fn apply_commits_pass_the_revision_rule() {
    let repo = golden("rr3-apply");
    let log = hook_check(&repo);
    let head = repo.head();
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let refused = commit_hooked(&repo, "a hand edit, no raise");
    assert!(!refused.status.success(), "{}", shown(&refused));
    assert!(
        shown(&refused).contains(&error_line(COUNTER_MD, BOUND_LINE, BOUND, false, 2, 3)),
        "{}",
        shown(&refused)
    );
    assert_eq!(repo.head(), head);
    repo.git(&["reset", "-q", "--hard"]);

    let at = At::new(&repo.root, &repo.home, repo.git_env(&repo.root));
    let propose = |reference: &str, (from, to): (&str, &str)| {
        let (span, text, _) = node_of(&repo, reference);
        proposed(&at, reference, &span, &edit(&text, from, to))
    };
    let bound = propose(BOUND, BOUND_EDIT);
    let goal = propose(GOAL, GOAL_EDIT);
    approve_at(&at, &bound, false, true).applied("a meaning approve of LAW-BOUND");
    assert!(
        repo.read(COUNTER_MD)
            .contains("## Bound {#LAW-BOUND rev=3}")
    );
    let old_goal = lock_hash(&repo.lock_text(), GOAL);
    approve_at(&at, &goal, true, true).applied("an editorial approve of GOAL-01");
    let (_, _, new_goal) = node_of(&repo, GOAL);
    assert_eq!(lock_hash(&repo.lock_text(), GOAL), new_goal);
    assert!(!repo.lock_text().contains(&old_goal));
    let piece = propose(PIECE, PIECE_EDIT);
    approve_at(&at, &piece, false, true).applied("a meaning approve of PIECE-COUNTER");
    assert!(repo.read(COUNTER_MD).contains("\nrev: 4\n"));
    assert_eq!(hook_runs(&log), 3, "each apply commit passed the hook");
    assert_eq!(repo.porcelain(), b"");
    assert_eq!(repo.read(GOAL_MD).matches(GOAL_EDIT.1).count(), 1);
}

// ------------------------------------------------------------------ TE-2

/// TE-2: on a fully staged tree `--changed` and `--staged` print the same
/// bytes, text and JSON, findings of the rule included; on both fixtures.
/// code-a holds the format-1 golden: its `GOAL-01` (two entries, one
/// refreshed) and `cap-check/PASS-01` entries give the three findings
/// (docs/features/lock-levels.md Q10: the rule reads format 1).
#[test]
fn changed_equals_staged_on_equal_trees() {
    let repo = golden_format_1("te2-code-a");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    repo.replace(CAP_MD, PASS_EDIT.0, PASS_EDIT.1);
    refresh(&repo, GOAL, Some(0));

    let game = ecs("te2-ecs");
    game.replace(STAMINA_MD, REGEN_EDIT.0, REGEN_EDIT.1);

    for (name, repo, findings) in [("code-a", &repo, 3), ("verify-ecs", &game, 1)] {
        stage_all(repo);
        let from_index = staged(repo);
        let from_disk = changed(repo);
        assert_eq!(
            from_index.subjects().len(),
            findings,
            "{name}:\n{}",
            from_index.show()
        );
        assert_eq!(
            (
                from_index.code(),
                &from_index.text.stdout,
                &from_index.json_run.stdout
            ),
            (
                from_disk.code(),
                &from_disk.text.stdout,
                &from_disk.json_run.stdout
            ),
            "{name}"
        );
    }
}

// ------------------------------------------------------------------ TE-3

/// TE-3: two branches each refresh `LAW-BOUND`'s entries editorially
/// (`spec lock accept --editorial`) after changing another paragraph of
/// its text; their merge conflicts only in `spec.lock`, which `spec check
/// --staged` refuses as unmerged (exit 2); `spec lock regen` keeps one
/// refresh per entry, so the merged text matches neither: the merge commit
/// is refused by the `--staged` hook, naming `LAW-BOUND`; committed with
/// `--no-verify`, `spec verify` reads its locked rows `spec_ahead
/// same_revision` ("Rules and edge cases").
#[test]
fn two_refreshes_through_regen_meet_the_rule() {
    let repo = golden("te3-regen");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.\n",
        "The count never exceeds the cap.\n\nIt holds after every step.\n\nIt holds after a reset too.\n",
    );
    repo.commit_all("two more paragraphs");
    repo.editorial(BOUND, true)
        .committed("the paragraphs are editorial");
    repo.git(&["branch", "side"]);

    repo.replace(COUNTER_MD, "after a reset too.", "after any reset too.");
    repo.commit_all("main's wording");
    repo.editorial(BOUND, true).committed("main's refresh");
    let main_hash = repo.norm_hash(BOUND);

    repo.git(&["checkout", "-q", "side"]);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.commit_all("side's wording");
    repo.editorial(BOUND, true).committed("side's refresh");
    let side_hash = repo.norm_hash(BOUND);
    repo.git(&["checkout", "-q", "main"]);

    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "a conflict: {}", shown(&merge));
    assert_ne!(repo.git_text(&["ls-files", "-u", "--", LOCK]), "");
    assert_eq!(
        repo.git_text(&["ls-files", "-u", "--", COUNTER_MD]),
        "",
        "the text merged"
    );
    let unmerged = staged(&repo);
    assert_cannot(&unmerged, "an unmerged spec.lock");
    assert!(
        unmerged.causes().contains(&UNMERGED.to_owned()),
        "{:?}",
        unmerged.causes()
    );

    repo.regen(true).written("regen");
    let kept = lock_hash(&repo.lock_text(), BOUND);
    assert!(kept == main_hash || kept == side_hash, "{kept}");
    repo.git(&["add", "--", LOCK]);
    let merged_hash = repo.norm_hash(BOUND);
    assert_ne!(merged_hash, main_hash);
    assert_ne!(merged_hash, side_hash);

    let log = hook_check(&repo);
    let head = repo.head();
    let refused = repo
        .git
        .git_output(&repo.root, &["commit", "-q", "--no-edit"], &[]);
    assert!(!refused.status.success(), "{}", shown(&refused));
    assert!(
        shown(&refused).contains(&error_line(COUNTER_MD, BOUND_LINE, BOUND, false, 2, 3)),
        "{}",
        shown(&refused)
    );
    assert_eq!(repo.head(), head);
    assert_eq!(hook_runs(&log), 0);

    repo.git(&["commit", "-q", "--no-edit", "--no-verify"]);
    assert_eq!(repo.parents(&repo.head()).len(), 2, "the merge commit");
    let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
    let document = run.json();
    let states: Vec<(Value, Value)> = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["node"] == BOUND && !row["lock"].is_null())
        .map(|row| (row["state"].clone(), row["category"].clone()))
        .collect();
    assert!(!states.is_empty(), "{}", run.show());
    for state in &states {
        assert_eq!(
            state,
            &(Value::from("spec_ahead"), Value::from("same_revision")),
            "{}",
            run.show()
        );
    }
}

/// TE-3: `spec check --staged` over an index where `spec.lock` alone is
/// unmerged (two branches' accepts disagreeing on one entry's
/// `verified_at`), a locked node changed beside it: cannot check, the
/// cause the unmerged path's. With `--changed` (no git index read) the
/// rule reads the disk's lock, its conflict markers unparseable: cannot
/// check naming `spec.lock`.
#[test]
fn unmerged_lock_is_refused_under_staged() {
    let repo = golden("te3-unmerged");
    let first = "verified_at = \"2026-10-08\"";
    repo.git(&["branch", "side"]);
    let text = repo.lock_text();
    repo.write(
        LOCK,
        text.replacen(first, "verified_at = \"2026-10-09\"", 1),
    );
    repo.commit_all("main's verified_at");
    repo.git(&["checkout", "-q", "side"]);
    repo.write(
        LOCK,
        text.replacen(first, "verified_at = \"2026-10-10\"", 1),
    );
    repo.commit_all("side's verified_at");
    repo.git(&["checkout", "-q", "main"]);
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "{}", shown(&merge));
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.git(&["add", "--", COUNTER_MD]);
    let check = staged(&repo);
    assert_cannot(&check, "unmerged");
    assert_eq!(check.causes(), [UNMERGED], "{}", check.show());
    let disk = changed(&repo);
    assert_cannot(&disk, "the disk's conflicted lock");
    assert!(names_checked_lock(&disk), "{:?}", disk.causes());
}
