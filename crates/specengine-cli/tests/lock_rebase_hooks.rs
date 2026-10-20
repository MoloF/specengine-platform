//! docs/features/lock-rebase.md TE-5 (Q41; "The base", Q14): a project's
//! `pre-commit` hook running the built `spec verify --staged`, or
//! `--changed`, judges the rebase commit (`git commit --only spec.lock`,
//! hooks run) on `fixtures/verify-ecs` (an ECS-style game crate: systems,
//! a plugin, RON tuning data; `drift_approve.rs`
//! `lock_commit_under_spec_verify_hooks`' corpus): its four nodes
//! accepted, then a marker naming no node added (`PERK-NOPE`: a
//! `bad_marker dangling` Fix row, Pre-existing at `HEAD`), `PERK-REGEN`'s
//! system edited (`code_ahead`, a Check row) and the accepted lock
//! committed under another grammar's `code_recipe` (`Variant::OtherCode`,
//! a simulated grammar bump), with `[verify] mode`. Under
//! `enforce-introduced` both hooks pass the rebase commit (`HEAD`'s old
//! lock judged with the staged or written one: every row Pre-existing),
//! exit 0, the lock accept's bytes;
//! under `enforce` the Fix row refuses it: exit 1, the bytes restored,
//! nothing changed.

#![cfg(unix)]

mod common;
mod lock_common;
mod verify_common;

use std::path::PathBuf;

use lock_common::{LockRepo, Variant};
use verify_common::{ECS_LIB, STAMINA, ecs, set_mode};

/// A marker naming a node no document holds (`PERK-NOPE`): a Fix row.
const STRAY: (&str, &str) = (
    "crates/game/src/stray.rs",
    "// @implements PERK-NOPE\npub fn stray() {}\n",
);

/// The edit that makes `PERK-REGEN`'s system `code_ahead` (a Check row).
const REGEN_EDIT: (&str, &str) = (
    "(stamina.current + rate * dt).min(stamina.max)",
    "(stamina.current + rate * dt * 2.0).min(stamina.max)",
);

/// verify-ecs, its four nodes accepted (the accepted lock returned), the
/// edit, the lock under another grammar's `code_recipe` and `[verify]
/// mode = "<mode>"` committed (no hook runs).
fn ecs_bumped(label: &str, mode: &str) -> (LockRepo, String) {
    let repo = ecs(label);
    let accepted = repo.lock_text();
    repo.replace(STAMINA, REGEN_EDIT.0, REGEN_EDIT.1);
    repo.write(STRAY.0, STRAY.1);
    repo.append(ECS_LIB, "pub mod stray;\n");
    repo.write("spec.lock", Variant::OtherCode.text(&accepted));
    set_mode(&repo.root, mode);
    repo.commit_all("the edit, another grammar, the mode");
    (repo, accepted)
}

/// A `pre-commit` hook running the built `spec verify <flag>` (a `HOME` of
/// its own), then appending `ran` to the log it returns when it passed.
fn verify_hook(repo: &LockRepo, flag: &str) -> PathBuf {
    let log = repo.scratch.join("hook.log");
    let home = repo.scratch.home("hook");
    repo.hook(
        "pre-commit",
        &format!(
            "HOME='{}' '{}' verify {flag} || exit 1\necho ran >> '{}'",
            home.display(),
            common::SPEC,
            log.display()
        ),
    );
    log
}

/// The passing runs of the hook.
fn hook_runs(log: &std::path::Path) -> usize {
    std::fs::read_to_string(log).map_or(0, |text| text.lines().count())
}

/// TE-5: under `enforce-introduced` a `--staged` hook and a `--changed`
/// hook each pass the rebase commit: exit 0, one commit of the lock alone,
/// accept's bytes; the Fix row is there (verify after it, plain: the
/// `PERK-NOPE` row of class `fix`).
#[test]
fn project_verify_hooks_pass_the_rebase_commit_under_enforce_introduced() {
    for flag in ["--staged", "--changed"] {
        let context = format!("{flag} enforce-introduced");
        let (repo, accepted) = ecs_bumped(
            &format!("lrh-pass-{}", flag.trim_start_matches('-')),
            "enforce-introduced",
        );
        let log = verify_hook(&repo, flag);
        let head = repo.head();
        let attempt = repo.rebase(true);
        let commit = attempt
            .committed(&context)
            .commit
            .clone()
            .expect("a commit");
        assert_eq!(repo.head(), commit, "{context}");
        assert_eq!(repo.parents(&commit), [head], "{context}");
        assert_eq!(repo.changed(&commit), ["spec.lock"], "{context}");
        assert_eq!(repo.lock_text(), accepted, "{context}: accept's bytes");
        assert_eq!(hook_runs(&log), 1, "{context}: the hook ran and passed");
        let run = verify_common::verify(&repo, &[]);
        let fix = run.rows_where(|row| row["node"] == "PERK-NOPE" && row["class"] == "fix");
        assert_eq!(fix.len(), 1, "{context}: the Fix row\n{}", run.text.show());
    }
}

/// TE-5: under `enforce` the Fix row refuses the rebase commit, whichever
/// hook: exit 1 after the one question, the commit not made, the bytes
/// restored, `HEAD`, refs, index, status and files as before.
#[test]
fn an_enforce_hook_refuses_the_rebase_commit_and_restores() {
    for flag in ["--staged", "--changed"] {
        let context = format!("{flag} enforce");
        let (repo, _) = ecs_bumped(
            &format!("lrh-refuse-{}", flag.trim_start_matches('-')),
            "enforce",
        );
        let log = verify_hook(&repo, flag);
        let before = repo.state();
        let lock = repo.lock_text();
        let attempt = repo.rebase(true);
        let refusal = attempt.refused(&context);
        assert_eq!(attempt.prompts.len(), 1, "{context}");
        assert!(refusal.contains("restored"), "{context}: {refusal}");
        assert_eq!(repo.state(), before, "{context}: restored");
        assert_eq!(repo.lock_text(), lock, "{context}");
        assert_eq!(hook_runs(&log), 0, "{context}: the hook refused");
        let outcome = attempt.outcome.as_ref().expect("an outcome");
        assert!(outcome.commit.is_none(), "{context}: {outcome:?}");
    }
}
