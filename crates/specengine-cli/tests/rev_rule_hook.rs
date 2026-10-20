//! docs/features/rev-rule.md TE-1 (08 AC-13): a `pre-commit` hook running
//! the built `spec check --staged --root .`, commits made by `git commit`
//! in scratch repositories of two projects of different nature (ADR-0008):
//! `fixtures/code-a` (a Rust library) with its golden lock
//! `fixtures/spec-lock/code-a.lock` committed, and `fixtures/verify-ecs`
//! (an ECS-style game crate: systems, a plugin, RON tuning data) with its
//! four nodes accepted. A locked node edited without a raise is refused,
//! the refusal naming it and both ways out; raised by hand, a
//! whitespace-only edit and an unlocked node pass; `observe` passes with
//! the finding in `--json`, `enforce-introduced` and `enforce` refuse.
//! The texts are `docs/features/rev-rule.md` "Data". 4b-2's carry-over
//! (rev-rule.md TE-3, Q18; docs/features/lock-rebase.md TE-7): the
//! commit `spec lock rebase` makes passes the same hook under `enforce`.

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_rule_common;
mod verify_common;

use lock_common::{LockRepo, golden_format_1_text};
use rev_rule_common::{
    BOUND, BOUND_EDIT, BOUND_LINE, COUNTER_MD, GOAL_EDIT, GOAL_MD, PIECE_EDIT, PIECE_LINE,
    REGEN_EDIT, STAMINA_EDIT, STAMINA_MD, STEP_EDIT, check_mode, commit_hooked, error_line,
    hook_check, hook_runs, shown, stage_all, staged,
};
use verify_common::{ecs, golden};

/// Both ways out the refusal names ("The rule `rev-not-raised`").
const WAYS_OUT: [&str; 2] = [
    "raise it (",
    "or have the owner approve it through the queue with --editorial",
];

/// TE-1 (code-a): `LAW-BOUND` (a section at `rev=2`, four vouching
/// entries) and `PIECE-COUNTER` (a document at `rev: 3`) edited without a
/// raise: the commit is refused, `HEAD` kept, the hook's output naming
/// each at its line with both ways out. M: the rule off.
#[test]
fn te1_unraised_locked_edit_is_refused_naming_both_ways_out() {
    let repo = golden("te1-refused");
    let log = hook_check(&repo);
    let head = repo.head();
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    stage_all(&repo);
    let output = commit_hooked(&repo, "an unraised edit");
    let printed = shown(&output);
    assert!(!output.status.success(), "{printed}");
    assert_eq!(repo.head(), head, "nothing committed");
    assert_eq!(hook_runs(&log), 0);
    for line in [
        error_line(COUNTER_MD, BOUND_LINE, BOUND, false, 2, 3),
        error_line(COUNTER_MD, PIECE_LINE, "PIECE-COUNTER", true, 3, 4),
    ] {
        assert!(printed.contains(&line), "{line:?} not in:\n{printed}");
    }
    for way in WAYS_OUT {
        assert!(printed.contains(way), "{way:?} not in:\n{printed}");
    }
}

/// TE-1 (code-a): each commit passes the hook: `LAW-BOUND` raised by hand
/// with its edit (`rev=3`); `GOAL-01` (revision 1 unwritten) given `rev: 2`
/// with its edit; a whitespace-only edit of `PIECE-COUNTER` (trailing
/// blanks, a doubled blank line: its meaning hash kept); the unlocked
/// `LAW-STEP` edited.
#[test]
fn te1_raise_whitespace_and_unlocked_pass() {
    let repo = golden("te1-passes");
    let log = hook_check(&repo);
    let mut commits = 0;
    let mut commit = |message: &str| {
        stage_all(&repo);
        let output = commit_hooked(&repo, message);
        assert!(output.status.success(), "{message}: {}", shown(&output));
        commits += 1;
        assert_eq!(hook_runs(&log), commits, "{message}");
    };

    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, "{#LAW-BOUND rev=2}", "{#LAW-BOUND rev=3}");
    commit("LAW-BOUND raised by hand");

    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    repo.replace(GOAL_MD, "id: GOAL-01\n", "id: GOAL-01\nrev: 2\n");
    commit("GOAL-01 raised by hand");

    repo.replace(
        COUNTER_MD,
        "never passes its cap.\n",
        "never passes its cap.   \n\n",
    );
    commit("whitespace only");

    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    commit("an unlocked node");
    assert_eq!(repo.porcelain(), b"");
}

/// TE-1 (code-a): the same unraised edit of `LAW-BOUND` under each
/// `[check] mode` (committed first, so `HEAD`'s mode is no stricter): the
/// finding in `--json` in every mode; `observe` commits through the hook,
/// `enforce-introduced` and `enforce` refuse.
#[test]
fn te1_observe_passes_enforce_introduced_refuses() {
    for (mode, passes, verdict) in [
        ("observe", true, "observed"),
        ("enforce-introduced", false, "blocked"),
        ("enforce", false, "blocked"),
    ] {
        let repo = golden(&format!("te1-mode-{mode}"));
        check_mode(&repo, mode);
        repo.commit_all("the mode");
        let log = hook_check(&repo);
        repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
        stage_all(&repo);
        let check = staged(&repo);
        assert_eq!(check.subjects(), [BOUND], "{mode}:\n{}", check.show());
        assert_eq!(check.verdict(), verdict, "{mode}:\n{}", check.show());
        assert_eq!(check.finding(BOUND)["introduced"], true, "{mode}");
        let output = commit_hooked(&repo, "an unraised edit");
        assert_eq!(
            output.status.success(),
            passes,
            "{mode}: {}",
            shown(&output)
        );
        assert_eq!(hook_runs(&log), usize::from(passes), "{mode}");
    }
}

/// TE-1 (verify-ecs, the second genre): `PERK-REGEN` (a section at
/// `rev=1`) and `PERK-STAMINA` (a document at `rev: 2`) edited without a
/// raise: refused, each named at its line; raised by hand (`rev=2`,
/// `rev: 3`), the commit passes; the unlocked `PERK-WALK` edited passes.
#[test]
fn te1_game_crate_unraised_refused_raised_passes() {
    let repo = ecs("te1-ecs");
    let log = hook_check(&repo);
    let head = repo.head();
    repo.replace(STAMINA_MD, REGEN_EDIT.0, REGEN_EDIT.1);
    repo.replace(STAMINA_MD, STAMINA_EDIT.0, STAMINA_EDIT.1);
    stage_all(&repo);
    let output = commit_hooked(&repo, "an unraised edit");
    let printed = shown(&output);
    assert!(!output.status.success(), "{printed}");
    assert_eq!(repo.head(), head);
    for line in [
        error_line(STAMINA_MD, 3, "PERK-STAMINA", true, 2, 3),
        error_line(STAMINA_MD, 13, "PERK-REGEN", false, 1, 2),
    ] {
        assert!(printed.contains(&line), "{line:?} not in:\n{printed}");
    }

    repo.replace(STAMINA_MD, "{#PERK-REGEN rev=1}", "{#PERK-REGEN rev=2}");
    repo.replace(STAMINA_MD, "\nrev: 2\n", "\nrev: 3\n");
    stage_all(&repo);
    let output = commit_hooked(&repo, "raised by hand");
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(hook_runs(&log), 1);

    repo.replace(
        "docs/spec/movement.md",
        "Walking costs nothing.",
        "Walking costs no stamina.",
    );
    stage_all(&repo);
    let output = commit_hooked(&repo, "an unlocked node");
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(hook_runs(&log), 2);
}

/// docs/features/lock-rebase.md TE-7 (4b-2's carry-over: rev-rule.md
/// TE-3, Q18; a scenario, no mutation: Q38): code-a's format-1 golden
/// committed under `[check] mode = "enforce"`, a `pre-commit` hook running
/// the built `spec check --staged --root .`; `spec lock rebase` on a
/// terminal, `y`: the hook runs on the rebase commit and passes (a
/// lock-only commit changes no `norm_hash`), the commit made, the lock
/// alone, exit 0.
#[test]
fn te7_rebase_commit_passes_a_check_staged_hook_under_enforce() {
    let repo = LockRepo::code_a("te7-rebase");
    repo.write("spec.lock", golden_format_1_text());
    check_mode(&repo, "enforce");
    repo.commit_all("the format-1 lock, enforce");
    let log = hook_check(&repo);
    let head = repo.head();
    let run = repo.spec_pty(&repo.root, &["lock", "rebase"], Some("y"));
    run.code(0);
    assert!(run.asked, "{}", run.show());
    let commit = repo.head();
    assert_ne!(commit, head, "the commit made");
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(repo.parents(&commit), [head]);
    assert!(
        run.stdout.ends_with(&format!("committed {commit}\n")),
        "{}",
        run.show()
    );
    assert_eq!(hook_runs(&log), 1, "the hook ran and passed");
}
