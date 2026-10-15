//! docs/features/rev-rule.md TE-6 (this repository: no `spec.lock`, so an
//! ADR edit staged gives no finding; its hook untouched), TE-7
//! (`fixtures/spec-c`, a Python web service's spec: no lock, no finding; a
//! lock vouching for one of its nodes, that node judged by its ID alone)
//! and TE-8 (the rule's findings and `spec bump`'s rows byte-identical from
//! another root, another cwd and a repository built in reverse creation
//! order: "Rules and edge cases").
//!
//! This repository is only read: its `HEAD` through `git archive` into a
//! scratch file, its hook file. The source scans of TE-7 are the genre
//! scans (core `genre.rs`, CLI `genre.rs`, store `genre.rs`), which walk
//! every source of the crates, the rule's included.

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_rule_common;
mod verify_common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::git::Sandbox;
use common::{Scratch, repository_root, snapshot};
use lock_common::{LockRepo, block, fake_oid, lock_of};
use rev_rule_common::{
    BOUND, BOUND_EDIT, CAP_MD, COUNTER_MD, GOAL, GOAL_EDIT, GOAL_MD, LOCK, PASS, PASS_EDIT,
    STEP_EDIT, bump_at, changed, check_at, message, refresh, stage_all, staged,
};
use serde_json::Value;
use verify_common::{clone_dir, golden, golden_file};

// ------------------------------------------------------------------ TE-6

/// `git args` in this repository, read-only (no optional locks).
fn repository_git(args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).expect("UTF-8")
}

/// TE-6: this repository tracks no `spec.lock` and has none; its
/// `pre-commit` hook still runs only `spec check --staged --root .`. Its
/// `HEAD` (through `git archive` into a scratch file) committed in a
/// scratch repository, an ADR's text edited and staged: `spec check
/// --staged` finds no `rev-not-raised`, exit 0, the same bytes as
/// `--changed`.
#[test]
fn te6_this_repository_has_no_lock_and_an_adr_edit_no_finding() {
    let top = repository_root();
    assert!(!top.join(LOCK).exists(), "this repository has no spec.lock");
    assert_eq!(repository_git(&["ls-files", "--", LOCK]), "");
    let hook = fs::read_to_string(top.join(".githooks/pre-commit")).expect("the hook");
    assert!(hook.contains("-- check --staged --root ."), "{hook}");
    assert!(!hook.contains("-- verify"), "{hook}");
    assert!(!hook.contains("-- bump"), "{hook}");

    let scratch = Scratch::new("te6-here");
    let git = Sandbox::new(scratch.path());
    let tar = scratch.join("head.tar");
    let output = format!("--output={}", tar.display());
    repository_git(&["archive", "--format=tar", &output, "HEAD"]);
    let root = scratch.dir("here");
    let untar = Command::new("/usr/bin/tar")
        .arg("-xf")
        .arg(&tar)
        .arg("-C")
        .arg(&root)
        .status()
        .expect("tar runs");
    assert!(untar.success());
    fs::remove_file(tar).expect("remove the archive");
    git.init(&root);
    git.add_all(&root);
    git.commit(&root, "this repository at HEAD");
    let home = scratch.home("h");
    let repo = LockRepo {
        scratch,
        git,
        root,
        home,
    };

    let adr = "docs/decisions/ADR-0043.md";
    let text = repo.read(adr);
    let body = text
        .find("\n---\n")
        .map(|at| at + "\n---\n".len())
        .expect("front-matter");
    let at = body + text[body..].find(" the ").expect("a word to change");
    repo.write(
        adr,
        format!("{} The {}", &text[..at], &text[at + " the ".len()..]),
    );
    assert_ne!(repo.read(adr), text);
    stage_all(&repo);
    let check = staged(&repo);
    assert!(check.subjects().is_empty(), "{}", check.show());
    assert_eq!(check.code(), 0, "{}", check.show());
    let disk = changed(&repo);
    assert_eq!(
        (&check.text.stdout, &check.json_run.stdout),
        (&disk.text.stdout, &disk.json_run.stdout)
    );
}

// ------------------------------------------------------------------ TE-7

/// TE-7: `fixtures/spec-c` (dock-hire, Python, prefixes and kinds of its
/// own, no `[code]`): `FEE-DAY-CAP` edited without a raise, no lock: no
/// finding. A lock vouching for `FEE-DAY-CAP` at its `HEAD` text committed
/// (code fields as a Python project's would be): the same edit gives that
/// one finding, the others unchanged; nothing in the rule reads a stack.
#[test]
fn te7_spec_c_no_lock_no_finding_and_a_lock_judges_by_id() {
    let fees = "docs/spec/fees.md";
    let cap_edit = ("never exceed 12.00.", "never exceed 15.00.");

    let repo = LockRepo::of("te7-spec-c", "spec-c", "repo", &[]);
    repo.replace(fees, cap_edit.0, cap_edit.1);
    stage_all(&repo);
    let without = staged(&repo);
    assert!(without.subjects().is_empty(), "{}", without.show());

    let locked = LockRepo::of("te7-spec-c-locked", "spec-c", "repo", &[]);
    let hash = locked.norm_hash("FEE-DAY-CAP");
    let entry = block(
        "FEE-DAY-CAP",
        1,
        &hash,
        "implements",
        "src/dock_hire/fees.py",
        "module",
        "dock_hire.fees",
        None,
        &fake_oid('a'),
        &fake_oid('b'),
        "2026-10-08",
    );
    locked.write(LOCK, lock_of(&[entry]));
    locked.commit_all("a lock");
    locked.replace(fees, cap_edit.0, cap_edit.1);
    stage_all(&locked);
    let with = staged(&locked);
    assert_eq!(with.subjects(), ["FEE-DAY-CAP"], "{}", with.show());
    let finding = with.finding("FEE-DAY-CAP");
    assert_eq!(finding["path"], fees);
    assert_eq!(finding["line"], 22);
    assert_eq!(
        finding["message"].as_str(),
        Some(message("FEE-DAY-CAP", false, 1, 2).as_str())
    );
    assert_eq!(with.others(), without.others(), "the other findings");
}

// ------------------------------------------------------------------ TE-8

/// The scenario every TE-8 repository stages: `LAW-BOUND`,
/// `cap-check/PASS-01` and `GOAL-01` (one of its two entries refreshed)
/// changed without a raise, `LAW-STEP` (unlocked) too.
fn scenario(repo: &LockRepo) {
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    repo.replace(CAP_MD, PASS_EDIT.0, PASS_EDIT.1);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    refresh(repo, GOAL, Some(1));
    stage_all(repo);
}

/// Every output of the rule from `cwd` (`root`: the `--root` arguments):
/// `check --staged`, `check --changed`, `bump`, `bump --staged`, each its
/// exit, text and JSON.
fn printed(repo: &LockRepo, cwd: &Path, root: &[&str]) -> Vec<(String, i32, String, String)> {
    let mut out = Vec::new();
    for flag in ["--staged", "--changed"] {
        let mut flags = vec![flag];
        flags.extend_from_slice(root);
        let check = check_at(repo, cwd, &flags, &[]);
        out.push((
            format!("check {flag}"),
            check.code(),
            check.text.stdout.clone(),
            check.json_run.stdout.clone(),
        ));
    }
    for flag in [None, Some("--staged")] {
        let mut flags: Vec<&str> = flag.into_iter().collect();
        flags.extend_from_slice(root);
        let bump = bump_at(repo, cwd, &flags, &[]);
        out.push((
            format!("bump {flag:?}"),
            bump.code(),
            bump.text.stdout.clone(),
            bump.json_run.stdout.clone(),
        ));
    }
    out
}

/// TE-8: the same commit and staged change checked from another root (a
/// byte copy), from a subdirectory (`--root ../..`) and in a repository
/// built in reverse creation order: the rule's findings and `spec bump`'s
/// rows, text and JSON, byte-identical. M: walk or creation order leaking
/// into the findings or rows.
#[test]
fn te8_findings_and_rows_are_byte_identical_across_roots_and_order() {
    let repo = golden("te8-same");
    let head = repo.head();
    scenario(&repo);
    let reference = printed(&repo, &repo.root, &[]);
    let check: Value = serde_json::from_str(&reference[0].3).expect("the JSON");
    let mut subjects: Vec<&str> = check["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == rev_rule_common::CODE)
        .map(|finding| finding["subject"].as_str().expect("a subject"))
        .collect();
    subjects.sort_unstable();
    assert_eq!(subjects, [GOAL, BOUND, PASS], "{reference:#?}");

    let copy = clone_dir(&repo, "copy");
    let other = LockRepo {
        scratch: Scratch::new("te8-same-other"),
        git: Sandbox::new(repo.scratch.path()),
        root: copy,
        home: repo.scratch.home("copy-home"),
    };
    assert_eq!(printed(&other, &other.root, &[]), reference, "another root");

    let sub = repo.root.join("crates/tally");
    assert_eq!(
        printed(&repo, &sub, &["--root", "../.."]),
        reference,
        "another cwd"
    );

    let reversed = LockRepo::uncommitted("te8-reversed");
    let mut files: Vec<(String, Option<Vec<u8>>)> = snapshot(&reversed.root).into_iter().collect();
    for (path, _) in &files {
        let full = reversed.root.join(path);
        if full.is_file() {
            fs::remove_file(&full).expect("remove");
        }
    }
    files.reverse();
    for (path, bytes) in files {
        if let Some(bytes) = bytes {
            reversed.write(&path, bytes);
        }
    }
    reversed.git.init(&reversed.root);
    reversed.git.add_all(&reversed.root);
    reversed.git.commit(&reversed.root, "the fixture");
    reversed.write(LOCK, fs::read(golden_file()).expect("the golden lock"));
    reversed.commit_all("the lock");
    assert_eq!(reversed.head(), head, "the same commits");
    scenario(&reversed);
    assert_eq!(
        printed(&reversed, &reversed.root, &[]),
        reference,
        "reverse creation order"
    );
}
