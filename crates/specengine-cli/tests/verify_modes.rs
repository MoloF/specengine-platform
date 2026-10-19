//! docs/features/spec-verify.md TE-4, TE-8 (the `[code]` half), RD-1's
//! CLI half, RD-5 and RD-6: the verdict and exit of `spec verify` under
//! the three `[verify]` modes, plain and with `--changed`, over five
//! states of `fixtures/verify-ecs` (clean, Check only, a Fix introduced, a
//! Fix pre-existing, `cannot_verify`); the base (`HEAD`, unborn, a merge
//! in progress, an unmerged `spec.lock`); the stricter mode of `HEAD`; the
//! run causes; and Q7's hooks: two `conflict` nodes, `spec lock accept
//! --editorial` of one under a pre-commit hook running `spec verify
//! --changed` (`enforce-introduced`) commits, the plain hook under
//! `enforce` refuses it.

mod common;
mod lock_common;
mod verify_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use lock_common::LockRepo;
use serde_json::{Value, json};
use verify_common::{
    ECS_LIB, MODES, STAMINA, STAMINA_MD, clone_dir, ecs, ecs_unlocked, set_mode, verify, verify_at,
};

/// The note of a plain `enforce-introduced` run ("Verdict and exit").
const NO_BASE: &str =
    "mode `enforce-introduced` has no base without --changed: judged as `enforce`";

/// The text of cause 2 (`docs/canon/code-bindings.md` "The `[code]` table").
const NO_CODE: &str =
    "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The five states of TE-4's matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Clean,
    CheckOnly,
    FixIntroduced,
    FixPreExisting,
    Cannot,
}

const CASES: [Case; 5] = [
    Case::Clean,
    Case::CheckOnly,
    Case::FixIntroduced,
    Case::FixPreExisting,
    Case::Cannot,
];

/// `from` replaced by `to` in `root/relative`, exactly once.
fn replace_in(root: &Path, relative: &str, from: &str, to: &str) {
    let path = root.join(relative);
    let text = fs::read_to_string(&path).expect("read");
    assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?}");
    fs::write(path, text.replacen(from, to, 1)).expect("write");
}

/// A copy of `base` with `mode` committed in its config, then `case`
/// applied (committed only for the pre-existing Fix).
fn cell(base: &LockRepo, mode: &str, case: Case, dir: &str) -> PathBuf {
    let root = clone_dir(base, dir);
    set_mode(&root, mode);
    base.git.add_all(&root);
    base.git.commit(&root, &format!("mode {mode}"));
    match case {
        Case::Clean => {}
        // A body edit: `code_ahead` (Check).
        Case::CheckOnly => replace_in(
            &root,
            STAMINA,
            "(stamina.current + rate * dt)",
            "(stamina.current + 2.0 * rate * dt)",
        ),
        // A locked item's marker removed: `broken_binding marker_gone`.
        Case::FixIntroduced | Case::FixPreExisting => {
            replace_in(&root, STAMINA, "// @implements PERK-DRAIN\n", "");
            if case == Case::FixPreExisting {
                base.git.add_all(&root);
                base.git.commit(&root, "the marker dropped");
            }
        }
        Case::Cannot => replace_in(
            &root,
            STAMINA,
            "(stamina.current + rate * dt)",
            "(stamina.current + rate * )",
        ),
    }
    root
}

/// What TE-4's matrix expects: the verdict and exit.
fn expected(mode: &str, changed: bool, case: Case) -> (&'static str, i32) {
    match case {
        Case::Clean => ("clean", 0),
        Case::CheckOnly => ("observed", 0),
        Case::Cannot => ("cannot-verify", 2),
        Case::FixIntroduced | Case::FixPreExisting => match mode {
            "observe" => ("observed", 0),
            "enforce" => ("blocked", 1),
            _ if changed && case == Case::FixPreExisting => ("observed", 0),
            _ => ("blocked", 1),
        },
    }
}

/// TE-4: {three modes} x {plain, `--changed`} x {clean, Check only, Fix
/// introduced, Fix pre-existing, `cannot_verify`}: the verdict, the exit,
/// the mode judged, `base`, the pre-existing marks and the plain
/// `enforce-introduced` note. M: exit 1 under `observe`; a Check
/// blocking; a Pre-existing Fix blocking; the base ignored.
#[test]
fn modes_by_base_by_findings() {
    let base = ecs("verify-matrix");
    let mut n = 0;
    for mode in MODES {
        for case in CASES {
            n += 1;
            let root = cell(&base, mode, case, &format!("cell-{n}"));
            for changed in [false, true] {
                let extra: &[&str] = if changed { &["--changed"] } else { &[] };
                let context = format!("{mode} {case:?} {extra:?}");
                let run = verify_at(&base, &root, extra);
                let (verdict, code) = expected(mode, changed, case);
                assert_eq!(
                    (run.verdict(), run.code()),
                    (verdict, code),
                    "{context}:\n{}",
                    run.text.show()
                );
                let judged = if !changed && mode == "enforce-introduced" {
                    "enforce"
                } else {
                    mode
                };
                assert_eq!(run.json["mode"], judged, "{context}");
                assert_eq!(
                    run.json["base"],
                    if changed { json!("HEAD") } else { Value::Null },
                    "{context}"
                );
                let noted = run.json["notes"]
                    .as_array()
                    .expect("notes")
                    .contains(&json!(NO_BASE));
                assert_eq!(noted, judged != mode, "{context}");
                assert_eq!(
                    run.text.stderr.contains(NO_BASE),
                    judged != mode,
                    "{context}: {}",
                    run.text.stderr
                );
                let pre: Vec<&Value> = run.rows_where(|row| row["pre_existing"] == true);
                let expected_pre = usize::from(changed && case == Case::FixPreExisting);
                assert_eq!(pre.len(), expected_pre, "{context}");
                assert_eq!(
                    run.json["counts"]["pre_existing"], expected_pre,
                    "{context}"
                );
                assert!(
                    run.summary()
                        .starts_with(&format!("verify: {verdict} [{judged}]: ")),
                    "{context}: {}",
                    run.summary()
                );
                assert_eq!(
                    run.summary().contains("pre-existing)"),
                    changed,
                    "{context}: {}",
                    run.summary()
                );
            }
        }
    }
    assert_eq!(n, 15);
}

/// RD-5: a Fix already at `HEAD` is Pre-existing with `--changed`: marked
/// in JSON, prefixed in the text, counted in the summary, left out of
/// `delta`; plain, the same row is introduced. M: the base ignored.
#[test]
fn drift_at_head_is_pre_existing() {
    let base = ecs("verify-pre-existing");
    let root = cell(&base, "observe", Case::FixPreExisting, "pre");
    let symbol = "crates/game::stamina::drain_system";
    let changed = verify_at(&base, &root, &["--changed"]);
    let row = changed.row("PERK-DRAIN", "implements", STAMINA, Some(symbol));
    assert_eq!(
        (&row["state"], &row["category"], &row["pre_existing"]),
        (
            &json!("broken_binding"),
            &json!("marker_gone"),
            &json!(true)
        )
    );
    changed.has_line("pre-existing fix | broken_binding marker_gone | PERK-DRAIN | implements | crates/game/src/stamina.rs | item crates/game::stamina::drain_system function_item");
    assert_eq!(
        changed.summary(),
        "verify: observed [observe]: 1 fix, 0 check (1 pre-existing), 0 cannot, 5 ok; 3 by blob, 2 by ast; 0 skipped"
    );
    assert_eq!(changed.json["delta"]["bindings"]["lost"], json!([]));
    assert_eq!(changed.json["delta"]["affected_nodes"], json!([]));

    let plain = verify_at(&base, &root, &[]);
    let row = plain.row("PERK-DRAIN", "implements", STAMINA, Some(symbol));
    assert_eq!(row["pre_existing"], false);
    plain.has_line("fix | broken_binding marker_gone | PERK-DRAIN | implements | crates/game/src/stamina.rs | item crates/game::stamina::drain_system function_item");
    assert_eq!(plain.json["delta"]["affected_nodes"], json!(["PERK-DRAIN"]));
    assert_eq!(
        plain.json["delta"]["bindings"]["lost"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    // Another category at HEAD is no Pre-existing: the file deleted now.
    fs::remove_file(root.join(STAMINA)).expect("remove");
    let gone = verify_at(&base, &root, &["--changed"]);
    let row = gone.row("PERK-DRAIN", "implements", STAMINA, Some(symbol));
    assert_eq!(
        (&row["category"], &row["pre_existing"]),
        (&json!("file_gone"), &json!(false))
    );
}

/// RD-5: under `enforce-introduced` with `--changed` a Pre-existing Fix
/// passes (`observed`, exit 0) while an introduced one blocks (exit 1).
/// M: a Pre-existing Fix blocking.
#[test]
fn pre_existing_fix_passes_enforce_introduced() {
    let base = ecs("verify-pre-passes");
    let root = cell(&base, "enforce-introduced", Case::FixPreExisting, "pre");
    let run = verify_at(&base, &root, &["--changed"]);
    assert_eq!(
        (run.verdict(), run.code()),
        ("observed", 0),
        "{}",
        run.text.show()
    );
    // A second Fix, introduced, beside it: blocked.
    replace_in(
        &root,
        ECS_LIB,
        "    // @implements PERK-STAMINA@2\n    pub fn build",
        "    pub fn build",
    );
    let both = verify_at(&base, &root, &["--changed"]);
    assert_eq!(
        (both.verdict(), both.code()),
        ("blocked", 1),
        "{}",
        both.text.show()
    );
    assert_eq!(both.json["counts"]["pre_existing"], 1);
    assert_eq!(both.json["counts"]["fix"], 2);
}

/// RD-5: with `--changed` the stricter of the current and `HEAD`'s modes
/// judges, with a note; `HEAD`'s unreadable mode is a note, the current
/// one judging. M: `HEAD`'s mode ignored.
#[test]
fn head_stricter_verify_mode_wins() {
    let base = ecs("verify-head-mode");
    let root = cell(&base, "enforce", Case::FixIntroduced, "strict");
    // The worktree loosens the mode, uncommitted.
    let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
    fs::write(
        root.join("specengine.toml"),
        config.replace("mode = \"enforce\"", "mode = \"observe\""),
    )
    .expect("config");
    let changed = verify_at(&base, &root, &["--changed"]);
    assert_eq!(
        (changed.verdict(), changed.code()),
        ("blocked", 1),
        "{}",
        changed.text.show()
    );
    assert_eq!(changed.json["mode"], "enforce");
    let notes = changed.json["notes"].as_array().expect("notes").clone();
    assert_eq!(
        notes,
        vec![json!(
            "HEAD's specengine.toml sets mode `enforce`, stricter than `observe`: verify runs in `enforce`"
        )]
    );
    assert!(changed.text.stderr.contains("stricter than `observe`"));
    // Plain: the current mode alone.
    let plain = verify_at(&base, &root, &[]);
    assert_eq!((plain.verdict(), plain.code()), ("observed", 0));
    // A stricter current mode wins over HEAD's looser one, no note.
    fs::write(root.join("specengine.toml"), &config).expect("config");
    base.git.add_all(&root);
    base.git.commit(&root, "back");
    let looser = config.replace("mode = \"enforce\"", "mode = \"observe\"");
    fs::write(root.join("specengine.toml"), &looser).expect("config");
    base.git.add_all(&root);
    base.git.commit(&root, "observe at HEAD");
    fs::write(root.join("specengine.toml"), &config).expect("config");
    // The Fix, committed with "back", is pre-existing: `enforce` blocks it.
    let current = verify_at(&base, &root, &["--changed"]);
    assert_eq!(
        (current.verdict(), current.code()),
        ("blocked", 1),
        "{}",
        current.text.show()
    );
    assert_eq!(current.json["notes"], json!([]));
    // HEAD's config not a valid one: a note, the current mode judging.
    fs::write(
        root.join("specengine.toml"),
        format!("{looser}\n[verify]\nmode = \"strict\"\n"),
    )
    .expect("config");
    base.git.add_all(&root);
    base.git.commit(&root, "a broken mode at HEAD");
    fs::write(root.join("specengine.toml"), &looser).expect("config");
    let unknown = verify_at(&base, &root, &["--changed"]);
    assert_eq!(unknown.json["mode"], "observe");
    let notes = unknown.json["notes"].as_array().expect("notes");
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0]
            .as_str()
            .unwrap_or_default()
            .contains("HEAD's mode is unknown"),
        "{notes:?}"
    );
}

/// RD-6: `observe` never exits 1: Fix rows introduced, pre-existing and
/// Check rows together, plain and with `--changed`. M: exit 1 under
/// `observe`.
#[test]
fn observe_never_exits_1() {
    let base = ecs("verify-observe");
    let root = cell(&base, "observe", Case::FixPreExisting, "all");
    replace_in(
        &root,
        ECS_LIB,
        "    // @implements PERK-STAMINA@2\n    pub fn build",
        "    pub fn build",
    );
    replace_in(
        &root,
        STAMINA,
        "(stamina.current + rate * dt)",
        "(stamina.current + 2.0 * rate * dt)",
    );
    replace_in(
        &root,
        STAMINA,
        "// @configures TUNE-01@1\n",
        "// @configures TUNE-01@2\n",
    );
    for extra in [&[][..], &["--changed"][..]] {
        let run = verify_at(&base, &root, extra);
        assert_eq!(
            (run.verdict(), run.code()),
            ("observed", 0),
            "{extra:?}:\n{}",
            run.text.show()
        );
        assert_eq!(run.json["counts"]["fix"], 3, "{extra:?}");
        // The regen body edit; lib.rs's module row, compared by its blob.
        assert_eq!(run.json["counts"]["check"], 2, "{extra:?}");
    }
}

/// RD-6: Check rows (`code_ahead`, `unlocked`, `spec_ahead`) never block,
/// even under `enforce`. M: a Check blocking.
#[test]
fn check_findings_never_block() {
    let base = ecs("verify-check");
    let root = cell(&base, "enforce", Case::CheckOnly, "check");
    // `unlocked`: a new marker; `spec_ahead`: TUNE-01's text at its revision.
    replace_in(
        &root,
        STAMINA,
        "/// Seconds a runner rests",
        "// @implements PERK-REGEN\n/// Seconds a runner rests",
    );
    replace_in(
        &root,
        "docs/records/TUNE-01.md",
        "read from the movement file.",
        "read from the movement data file.",
    );
    for extra in [&[][..], &["--changed"][..]] {
        let run = verify_at(&base, &root, extra);
        assert_eq!(
            (run.verdict(), run.code()),
            ("observed", 0),
            "{extra:?}:\n{}",
            run.text.show()
        );
        let mut states: Vec<&str> = run
            .rows_where(|row| !row["class"].is_null())
            .iter()
            .map(|row| row["state"].as_str().unwrap_or_default())
            .collect();
        states.sort_unstable();
        assert_eq!(
            states,
            ["code_ahead", "spec_ahead", "spec_ahead", "unlocked"],
            "{extra:?}"
        );
        assert_eq!(run.json["counts"]["fix"], 0);
        assert_eq!(run.json["counts"]["check"], 4);
    }
}

/// RD-6, TE-8: a project without `[code]` exits 2 in every mode with cause
/// 2's exact text, plain and `--changed`; `fixtures/spec-c` (a Python
/// project) the same. M: no `[code]` exit 0.
#[test]
fn verify_without_code_table_is_exit_2() {
    let repo = ecs("verify-no-code");
    let config = repo.read("specengine.toml");
    let without = config.replace("[code]\nroots = [\"crates\", \"assets\"]\n", "");
    assert_ne!(without, config);
    for mode in MODES {
        repo.write("specengine.toml", &without);
        set_mode(&repo.root, mode);
        for extra in [&[][..], &["--changed"][..]] {
            let run = verify(&repo, extra);
            assert_eq!(run.code(), 2, "{mode} {extra:?}:\n{}", run.text.show());
            assert_eq!(run.json["cannot"], json!([NO_CODE]), "{mode} {extra:?}");
            run.has_line(&format!("cannot: {NO_CODE}"));
            assert!(run.rows().is_empty());
        }
    }
    let spec_c = LockRepo::of("verify-spec-c", "spec-c", "repo", &[]);
    let run = verify(&spec_c, &[]);
    assert_eq!(run.code(), 2, "{}", run.text.show());
    assert_eq!(run.json["cannot"], json!([NO_CODE]));
    assert_eq!(
        run.text.stdout,
        format!(
            "cannot: {NO_CODE}\nverify: cannot-verify [observe]: 0 fix, 0 check, 0 cannot, 0 ok; 0 by blob, 0 by ast; 0 skipped\n"
        )
    );
}

/// RD-1, flipped by docs/features/verify-tests.md (8a-1) RD-1: no
/// `[verify]` table runs `observe`; plain `spec verify` reads `tests` and
/// `timeout` (a valid pair changes nothing) and reads the table closed: a
/// `tests` that is no argv list is cause 1 at its line with the
/// argv-list text, exit 2, the report printed. M: default `enforce`; a
/// string `tests` accepted.
#[test]
fn verify_reads_its_table_closed() {
    let repo = ecs("verify-table");
    let run = verify(&repo, &[]);
    assert_eq!(
        (run.json["mode"].as_str(), run.code()),
        (Some("observe"), 0)
    );
    let config = repo.read("specengine.toml");
    repo.write(
        "specengine.toml",
        format!("{config}\n[verify]\ntests = [\"scripts/run-test\", \"{{test}}\"]\ntimeout = 30\n"),
    );
    let valid = verify(&repo, &[]);
    assert_eq!(
        (valid.json["mode"].as_str(), valid.code()),
        (Some("observe"), 0),
        "{}",
        valid.text.show()
    );
    assert_eq!(valid.json["cannot"], json!([]));
    let line = config.lines().count() + 3;
    repo.write(
        "specengine.toml",
        format!("{config}\n[verify]\ntests = \"x {{test}}\"\n"),
    );
    let refused = verify(&repo, &[]);
    assert_eq!(refused.code(), 2, "{}", refused.text.show());
    let cannot = refused.json["cannot"].as_array().expect("cannot");
    assert_eq!(cannot.len(), 1, "{cannot:?}");
    let cause = cannot[0].as_str().unwrap_or_default();
    assert!(
        cause.starts_with(&format!(
            "specengine.toml:{line}: [verify] tests must be an argv list, e.g. [\"scripts/run-test\", \"{{test}}\"]"
        )),
        "{cause}"
    );
    assert!(!cause.contains("is not built"), "{cause}");
    assert_eq!(refused.json["mode"], Value::Null);
    refused.has_line(&format!("cannot: {cause}"));
    assert_eq!(
        refused.summary(),
        "verify: cannot-verify: 0 fix, 0 check, 0 cannot, 0 ok; 0 by blob, 0 by ast; 0 skipped"
    );
}

/// RD-1: every other command only name-checks `[verify]`: `spec show`,
/// `spec check` and `spec refs` answer with a valid table and with one
/// verify itself refuses. M: `verify` not in `RawFile`.
#[test]
fn verify_table_is_name_checked() {
    let repo = ecs_unlocked("verify-name-checked");
    let config = repo.read("specengine.toml");
    for table in ["[verify]\nmode = \"enforce\"\n", "[verify]\ntests = 1\n"] {
        repo.write("specengine.toml", format!("{config}\n{table}"));
        for args in [
            &["show", "PERK-REGEN"][..],
            &["refs", "PERK-REGEN"][..],
            &["tree"][..],
        ] {
            let run = repo.spec_piped(&repo.root, args);
            run.code(0);
        }
    }
}

/// "Verdict and exit": several run causes print as `cannot:` lines in the
/// normative order, a config error (1) before the lock's (6); no row is
/// judged; exit 2.
#[test]
fn run_causes_print_in_order() {
    let repo = ecs("verify-causes");
    repo.write("spec.lock", "format = 2\n");
    let config = repo.read("specengine.toml");
    repo.write(
        "specengine.toml",
        format!("{config}\n[verify]\nmode = \"strict\"\n"),
    );
    let run = verify(&repo, &["--changed"]);
    assert_eq!(run.code(), 2, "{}", run.text.show());
    let cannot: Vec<&str> = run.json["cannot"]
        .as_array()
        .expect("cannot")
        .iter()
        .map(|cause| cause.as_str().unwrap_or_default())
        .collect();
    assert_eq!(cannot.len(), 2, "{cannot:?}");
    assert!(cannot[0].starts_with("specengine.toml:"), "{cannot:?}");
    assert!(cannot[1].starts_with("spec.lock"), "{cannot:?}");
    let lines: Vec<&str> = run
        .lines()
        .into_iter()
        .filter(|line| line.starts_with("cannot: "))
        .collect();
    assert_eq!(
        lines,
        cannot
            .iter()
            .map(|cause| format!("cannot: {cause}"))
            .collect::<Vec<_>>()
    );
    assert!(run.rows().is_empty());
}

/// TE-4: an unborn `HEAD`: plain judges the worktree; `--changed` has base
/// `unborn`, every row introduced (an uncommitted lock's Fix blocks under
/// `enforce-introduced`).
#[test]
fn unborn_head_is_all_introduced() {
    let accepted = ecs("verify-unborn-lock");
    let repo = LockRepo::uncommitted("verify-unborn");
    fs::remove_dir_all(&repo.root).expect("clear");
    common::copy_dir(&common::fixture(verify_common::ECS), &repo.root);
    repo.git.init(&repo.root);
    let plain = verify(&repo, &[]);
    assert_eq!(
        (plain.verdict(), plain.code()),
        ("observed", 0),
        "{}",
        plain.text.show()
    );
    assert_eq!(plain.json["counts"]["check"], 6);
    let changed = verify(&repo, &["--changed"]);
    assert_eq!(changed.json["base"], "unborn");
    assert_eq!((changed.verdict(), changed.code()), ("observed", 0));
    assert!(
        changed.summary().contains("(0 pre-existing)"),
        "{}",
        changed.summary()
    );

    repo.write("spec.lock", accepted.lock().expect("a lock"));
    set_mode(&repo.root, "enforce-introduced");
    replace_in(&repo.root, STAMINA, "// @implements PERK-DRAIN\n", "");
    let fix = verify(&repo, &["--changed"]);
    assert_eq!(fix.json["base"], "unborn");
    assert_eq!(
        (fix.verdict(), fix.code()),
        ("blocked", 1),
        "{}",
        fix.text.show()
    );
    assert_eq!(fix.json["counts"]["ok"], 5);
    assert_eq!(fix.json["counts"]["pre_existing"], 0);
}

/// TE-4: a merge in progress (a conflicted file outside every binding):
/// `--changed` still judges against `HEAD`, a pre-existing Fix stays
/// Pre-existing.
#[test]
fn merge_in_progress_still_judges_against_head() {
    let base = ecs("verify-merge");
    let root = cell(&base, "enforce-introduced", Case::FixPreExisting, "merge");
    let git = |args: &[&str]| base.git.git(&root, args);
    fs::write(root.join("notes.txt"), "base\n").expect("notes");
    base.git.add_all(&root);
    base.git.commit(&root, "notes");
    git(&["checkout", "-q", "-b", "side"]);
    fs::write(root.join("notes.txt"), "side\n").expect("notes");
    base.git.add_all(&root);
    base.git.commit(&root, "side notes");
    git(&["checkout", "-q", "main"]);
    fs::write(root.join("notes.txt"), "main\n").expect("notes");
    base.git.add_all(&root);
    base.git.commit(&root, "main notes");
    let merge = base
        .git
        .git_output(&root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: a conflicted merge");
    assert!(root.join(".git/MERGE_HEAD").exists());
    let run = verify_at(&base, &root, &["--changed"]);
    assert_eq!(run.json["base"], "HEAD");
    assert_eq!(
        (run.verdict(), run.code()),
        ("observed", 0),
        "{}",
        run.text.show()
    );
    assert_eq!(run.json["counts"]["pre_existing"], 1);
}

/// A copy of `base` in a merge whose `spec.lock` conflicts (both branches
/// changed one entry's `verified_at`).
fn unmerged_copy(base: &LockRepo, dir: &str) -> PathBuf {
    let root = clone_dir(base, dir);
    let git = |args: &[&str]| base.git.git(&root, args);
    git(&["checkout", "-q", "-b", "side"]);
    let entry = "verified_at = \"2026-10-08\"\n\n[[binding]]\nnode = \"PERK-REGEN\"";
    replace_in(
        &root,
        "spec.lock",
        entry,
        &entry.replace("2026-10-08", "2026-10-09"),
    );
    base.git.add_all(&root);
    base.git.commit(&root, "side lock");
    git(&["checkout", "-q", "main"]);
    replace_in(
        &root,
        "spec.lock",
        entry,
        &entry.replace("2026-10-08", "2026-10-07"),
    );
    base.git.add_all(&root);
    base.git.commit(&root, "main lock");
    let merge = base
        .git
        .git_output(&root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: spec.lock conflicts");
    root
}

/// TE-4, Q5: an unmerged `spec.lock` (two branches' accepts): cause 5
/// `spec.lock is unmerged: run spec lock regen` in every mode, plain too,
/// before the lock's own cause.
#[test]
fn unmerged_lock_is_a_run_cause() {
    let base = ecs("verify-unmerged");
    let root = unmerged_copy(&base, "unmerged");
    let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
    for mode in MODES {
        fs::write(root.join("specengine.toml"), &config).expect("config");
        set_mode(&root, mode);
        for extra in [&[][..], &["--changed"][..]] {
            let run = verify_at(&base, &root, extra);
            assert_eq!(run.code(), 2, "{mode} {extra:?}:\n{}", run.text.show());
            let cannot = run.json["cannot"].as_array().expect("cannot");
            assert_eq!(
                cannot.first(),
                Some(&json!("spec.lock is unmerged: run spec lock regen")),
                "{mode} {extra:?}: {cannot:?}"
            );
            assert!(run.rows().is_empty());
        }
    }
}

/// TE-4, Q7: two `conflict` nodes at `HEAD`. A project pre-commit hook
/// running plain `spec verify` under `enforce` refuses the lock commit of
/// `spec lock accept --editorial` of one (the other's Fix blocks; nothing
/// changes); the hook running `spec verify --changed` under
/// `enforce-introduced` lets it commit, exit 0.
#[test]
fn hooked_verify_changed_lets_one_conflict_be_accepted() {
    let repo = ecs("verify-hook");
    repo.replace(
        STAMINA_MD,
        "while the runner rests.",
        "while the runner rests still.",
    );
    repo.replace(
        STAMINA_MD,
        "at zero the runner walks.",
        "at zero the runner must walk.",
    );
    repo.replace(
        STAMINA,
        "(stamina.current + rate * dt)",
        "(stamina.current + rate * dt * 1.0)",
    );
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt)",
        "(stamina.current - cost * dt * 1.0)",
    );
    let config = repo.read("specengine.toml");
    repo.write(
        "specengine.toml",
        format!("{config}\n[verify]\nmode = \"enforce\"\n"),
    );
    repo.commit_all("two conflicts");
    let both = verify(&repo, &["--changed"]);
    assert_eq!(
        both.rows_where(|row| row["state"] == "conflict" && row["pre_existing"] == true)
            .len(),
        2,
        "{}",
        both.text.show()
    );

    let spec = common::SPEC;
    repo.hook("pre-commit", &format!("exec '{spec}' verify"));
    let before = repo.state();
    let attempt = repo.editorial("PERK-REGEN", true);
    let refusal = attempt.refused("plain hook under enforce");
    assert!(refusal.contains("restored"), "{refusal}");
    assert_eq!(repo.state(), before, "the refused accept changed nothing");

    repo.write(
        "specengine.toml",
        format!("{config}\n[verify]\nmode = \"enforce-introduced\"\n"),
    );
    repo.commit_all("enforce-introduced");
    repo.hook("pre-commit", &format!("exec '{spec}' verify --changed"));
    let head = repo.head();
    let attempt = repo.editorial("PERK-REGEN", true);
    let outcome = attempt.committed("hook running verify --changed");
    assert_eq!(outcome.exit().code(), 0);
    assert_ne!(repo.head(), head);
    assert_eq!(repo.changed(&repo.head()), ["spec.lock"]);
}

/// TE-8 (ADR-0008): the sources of `spec verify` (CLI `verify.rs`,
/// `verify_rows.rs`, core `verify.rs`, store `verify_git.rs`) hold no
/// fixture prefix (`PREFIX-`, every fixture's, the repository's own `ADR`
/// citations aside), no kind of the fixtures verify runs over (code-a,
/// verify-ecs) or of the game-design one (spec-a), no `cargo`, `nextest`,
/// `bevy` and no path literal; a module's own `#[cfg(test)] mod` aside.
/// M: a "mechanic" literal.
#[test]
fn verify_sources_hold_no_project_words() {
    let root = common::repository_root();
    let mut prefixes = std::collections::BTreeSet::new();
    let mut kinds = std::collections::BTreeSet::new();
    for entry in fs::read_dir(root.join("fixtures")).expect("fixtures") {
        let config = entry.expect("entry").path().join("specengine.toml");
        let Ok(text) = fs::read_to_string(&config) else {
            continue;
        };
        let table: toml::Table = text.parse().expect("a fixture config");
        let Some(ids) = table.get("ids").and_then(toml::Value::as_table) else {
            continue;
        };
        let name = config
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        for (prefix, spec) in ids {
            prefixes.insert(prefix.clone());
            for alias in spec
                .get("aliases_from")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
            {
                prefixes.insert(alias.as_str().unwrap_or_default().to_owned());
            }
            if ["code-a", "verify-ecs", "spec-a"].contains(&name.as_str())
                && let Some(kind) = spec.get("kind").and_then(toml::Value::as_str)
            {
                kinds.insert(kind.to_owned());
            }
        }
    }
    prefixes.remove("ADR");
    assert!(prefixes.contains("PERK") && prefixes.contains("GOAL") && kinds.contains("mechanic"));
    // Absolute path prefixes, built at run time (the anonymity test bans
    // them as literals).
    let slash = "/";
    let paths: Vec<String> = ["Users", "home", "Volumes", "var/folders"]
        .iter()
        .map(|part| format!("{slash}{part}{slash}"))
        .collect();
    let words = ["cargo", "nextest", "bevy", "tally", "crates/", "fixtures/"];
    let boundary = |line: &str, at: usize, len: usize| {
        let before = line[..at].chars().next_back();
        let after = line[at + len..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
    };
    let mut offences = Vec::new();
    for file in [
        "crates/specengine-cli/src/verify.rs",
        "crates/specengine-cli/src/verify_rows.rs",
        "crates/specengine-core/src/verify.rs",
        "crates/specengine-store/src/verify_git.rs",
    ] {
        let whole = fs::read_to_string(root.join(file)).expect("a verify source");
        // A module's own `#[cfg(test)] mod` (over the fixtures, by design)
        // ends what is scanned.
        let text = whole
            .find("\n#[cfg(test)]\nmod ")
            .map_or(whole.as_str(), |at| &whole[..at]);
        assert!(text.len() > 1000, "{file}: the code is scanned");
        for (n, line) in text.lines().enumerate() {
            for prefix in &prefixes {
                let needle = format!("{prefix}-");
                for (at, _) in line.match_indices(&needle) {
                    let before = line[..at].chars().next_back();
                    if !before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        offences.push(format!("{file}:{}: prefix {prefix}", n + 1));
                    }
                }
            }
            for kind in &kinds {
                for (at, _) in line.match_indices(kind.as_str()) {
                    if boundary(line, at, kind.len()) {
                        offences.push(format!("{file}:{}: kind {kind}", n + 1));
                    }
                }
            }
            for word in words
                .iter()
                .copied()
                .chain(paths.iter().map(String::as_str))
            {
                if line.contains(word) {
                    offences.push(format!("{file}:{}: {word}", n + 1));
                }
            }
        }
    }
    assert!(offences.is_empty(), "{offences:#?}");
}

/// The walk gap's text for an unreadable `stamina.rs`.
const WALK_GAP: &str = "the code walk is incomplete: `crates/game/src/stamina.rs` cannot be read";

/// "Verdict and exit", iteration 2: the walk runs whenever `[code]` reads,
/// so a walk gap (cause 3) prints its `cannot:` line first beside an
/// unmerged lock (5) and an invalid one (6), in that order, plain and
/// `--changed`. M: the walk skipped when another cause is present.
#[test]
fn walk_gap_prints_before_the_locks_causes() {
    let base = ecs("verify-cause-order");
    let invalid = clone_dir(&base, "invalid");
    fs::write(invalid.join("spec.lock"), "format = 9\n").expect("lock");
    let merged = unmerged_copy(&base, "merged");
    for (root, expected) in [(&invalid, vec!["3", "6"]), (&merged, vec!["3", "5", "6"])] {
        let stamina = root.join(STAMINA);
        fs::set_permissions(&stamina, fs::Permissions::from_mode(0o000)).expect("chmod");
        for extra in [&[][..], &["--changed"][..]] {
            let run = verify_at(&base, root, extra);
            fs::set_permissions(&stamina, fs::Permissions::from_mode(0o644)).expect("chmod");
            let cannot: Vec<String> = run.json["cannot"]
                .as_array()
                .expect("cannot")
                .iter()
                .map(|cause| cause.as_str().unwrap_or_default().to_owned())
                .collect();
            let kinds: Vec<&str> = cannot
                .iter()
                .map(|cause| {
                    if cause.starts_with(WALK_GAP) {
                        "3"
                    } else if cause == "spec.lock is unmerged: run spec lock regen" {
                        "5"
                    } else if cause.starts_with("spec.lock:") {
                        "6"
                    } else {
                        "?"
                    }
                })
                .collect();
            assert_eq!(kinds, expected, "{extra:?}: {cannot:?}");
            let lines: Vec<&str> = run
                .lines()
                .into_iter()
                .filter(|line| line.starts_with("cannot: "))
                .collect();
            assert_eq!(
                lines,
                cannot
                    .iter()
                    .map(|cause| format!("cannot: {cause}"))
                    .collect::<Vec<_>>()
            );
            assert_eq!(run.code(), 2);
            assert!(run.rows().is_empty());
            fs::set_permissions(&stamina, fs::Permissions::from_mode(0o000)).expect("chmod");
        }
        fs::set_permissions(&stamina, fs::Permissions::from_mode(0o644)).expect("chmod");
    }
}

/// Iteration 2: the mode shown, text and JSON, is the one the verdict
/// used, a run cause or not: plain `enforce-introduced` shows `[enforce]`
/// with its note beside an invalid lock; with `--changed` (no base read)
/// `[enforce-introduced]`, no note. M: the configured mode shown.
#[test]
fn run_cause_shows_the_mode_the_verdict_used() {
    let repo = ecs("verify-cause-mode");
    set_mode(&repo.root, "enforce-introduced");
    repo.write("spec.lock", "format = 9\n");
    let plain = verify(&repo, &[]);
    assert_eq!(plain.code(), 2);
    assert_eq!(plain.json["mode"], "enforce");
    assert_eq!(plain.json["notes"], json!([NO_BASE]));
    assert!(plain.text.stderr.contains(NO_BASE), "{}", plain.text.stderr);
    assert_eq!(
        plain.summary(),
        "verify: cannot-verify [enforce]: 0 fix, 0 check, 0 cannot, 0 ok; 0 by blob, 0 by ast; 0 skipped"
    );
    let changed = verify(&repo, &["--changed"]);
    assert_eq!(changed.json["mode"], "enforce-introduced");
    assert_eq!(changed.json["notes"], json!([]));
    assert_eq!(
        changed.summary(),
        "verify: cannot-verify [enforce-introduced]: 0 fix, 0 check (0 pre-existing), 0 cannot, 0 ok; 0 by blob, 0 by ast; 0 skipped"
    );
}
