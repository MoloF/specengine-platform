//! AC-10 of docs/features/spec-cli-switch.md: this repository's own
//! `.githooks/pre-commit`, copied with its mode into scratch repositories
//! (every git process in the scratch sandbox: `GIT_CONFIG_GLOBAL=/dev/null`,
//! `GIT_CONFIG_NOSYSTEM=1`, a scratch `HOME`), `core.hooksPath` at it, and a
//! logging `cargo` shim first on `PATH` that execs the built `spec` with the
//! arguments after `run -q -p specengine-cli --` (so no cargo build runs).
//! The project is crafted: `docs/` under a small `specengine.toml` with a
//! canon cap. The cases, with the mutation each one catches in brackets:
//! (a) the shim gets exactly `run -q -p specengine-cli -- check --staged
//! --root .` [no `--root .`]; (b) a staged bad `.md` is refused with `HEAD`
//! kept and the message naming the export command, a clean one committed;
//! (c) nothing relevant staged → no call [unconditional]; (d) only
//! `specengine.toml` staged with a cap below a document, or only a bad
//! `.spec-debt.toml` → refused [`.md` only]; (e) `git mv a.md a.txt`
//! checks [renames on]; (f) 5 000 staged `.md`, one bad → refused [`grep -q`
//! on a `pipefail` pipeline]; (g) the shim exiting 127 or 2 → refused
//! [`|| true`]; (h) `commit -a` with the error only on disk → refused; (i)
//! `--no-verify` commits, no call; (j) the hook is `100755` in `HEAD`, the
//! index and on disk, and a copy without the executable bit is ignored by
//! git, so (b)'s refusal is lost [`chmod -x`]; (k) `a.md` made a symlink (a
//! type change) checks [`--diff-filter=ACMD`]. AC-11: `docs.yml` runs only
//! `cargo run --locked -q -p specengine-cli -- check --root .`.
//!
//! This repository is only read (its hook and workflow files, and
//! `git ls-files` / `git ls-tree` of the hook).

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;

use common::git::Sandbox;
use common::{SPEC, Scratch, repository_root, snapshot, write};

/// The argument prefix of the registered commands, then G `--staged`.
const PREFIX: &str = "run -q -p specengine-cli --";
const CALL: &str = "run -q -p specengine-cli -- check --staged --root .";
const EXPORT: &str = "cargo run -q -p specengine-cli -- export index";

const CONFIG: &str = "\
[paths]
roots = [\"docs\"]

[ids]
R = { kind = \"requirement\", width = 2 }

[budgets]
canon_bytes = 12288

[classes]
canon = { required = [\"class\", \"tier\", \"scope\"], closed = true }
spec  = { required = [\"class\", \"status\", \"scope\"], closed = true }

[check]
mode = \"enforce\"
";

fn clean(title: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# {title}\n\nText.\n")
}

/// No front-matter: `class-missing`, an error.
const BAD: &str = "# A document without a class\n";

const GUIDE: &str =
    "---\nclass: canon\ntier: 2\nscope: [x]\n---\n\n# Guide\n\nText of the guide.\n";

/// The hook's outcome, judged by `HEAD`.
#[derive(Debug, PartialEq, Eq)]
enum Commit {
    Recorded,
    Refused,
}

struct HookRepo {
    /// Kept for its lifetime: removed on drop, after the HOME check.
    _scratch: Scratch,
    git: Sandbox,
    top: PathBuf,
    log: PathBuf,
    shim: PathBuf,
}

impl HookRepo {
    /// A clean project, the real hook at `.githooks/pre-commit` (its mode
    /// copied; `mode` overrides it), one commit, `core.hooksPath` set.
    fn new(name: &str, mode: Option<u32>) -> Self {
        let scratch = Scratch::new(name);
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        write(&top, "specengine.toml", CONFIG);
        write(&top, "docs/guide.md", GUIDE);
        write(&top, "docs/a.md", clean("A"));
        write(&top, "docs/k.md", clean("K"));
        write(&top, "notes.txt", "not a document\n");
        let hook = top.join(".githooks/pre-commit");
        fs::create_dir_all(hook.parent().unwrap()).unwrap();
        fs::copy(repository_root().join(".githooks/pre-commit"), &hook).unwrap();
        if let Some(mode) = mode {
            fs::set_permissions(&hook, fs::Permissions::from_mode(mode)).unwrap();
        }
        git.init(&top);
        git.git(&top, &["config", "core.hooksPath", ".githooks"]);
        git.add_all(&top);
        git.commit(&top, "base");

        let bin = scratch.dir("bin");
        let shim = bin.join("cargo");
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\n\
                 printf '%s\\n' \"$*\" >> \"$SHIM_LOG\"\n\
                 if [ -n \"$SHIM_EXIT\" ]; then exit \"$SHIM_EXIT\"; fi\n\
                 if [ \"$1 $2 $3 $4 $5\" = \"{PREFIX}\" ]; then\n\
                 \x20 shift 5\n\
                 \x20 exec '{SPEC}' \"$@\"\n\
                 fi\n\
                 echo \"shim: unexpected cargo $*\" >&2\n\
                 exit 97\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let log = scratch.join("cargo.log");
        fs::write(&log, "").unwrap();
        Self {
            _scratch: scratch,
            git,
            top,
            log,
            shim: bin,
        }
    }

    fn git(&self, args: &[&str]) {
        self.git.git(&self.top, args);
    }

    /// `git commit -q -m <message> <args>`, the shim first on `PATH`, and
    /// `SHIM_EXIT` when given: the outcome, the calls the shim logged, and
    /// the whole run for messages.
    fn commit(
        &self,
        message: &str,
        args: &[&str],
        exit: Option<&str>,
    ) -> (Commit, Vec<String>, String) {
        fs::write(&self.log, "").unwrap();
        let before = self.git.head(&self.top);
        let mut full = vec!["commit", "-q", "-m", message];
        full.extend_from_slice(args);
        let path = std::env::join_paths(std::iter::once(self.shim.clone()).chain(
            std::env::split_paths(self.git.var("PATH").expect("the sandbox PATH")),
        ))
        .unwrap();
        let mut extra: Vec<(&str, &OsStr)> = vec![
            ("PATH", path.as_os_str()),
            ("SHIM_LOG", self.log.as_os_str()),
        ];
        if let Some(exit) = exit {
            extra.push(("SHIM_EXIT", OsStr::new(exit)));
        }
        let output = self.git.git_output(&self.top, &full, &extra);
        let after = self.git.head(&self.top);
        let shown = format!(
            "git {full:?}: {}\n--- stdout\n{}--- stderr\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let outcome = if output.status.success() {
            assert_ne!(before, after, "{shown}");
            Commit::Recorded
        } else {
            assert_eq!(before, after, "{shown}");
            Commit::Refused
        };
        let calls = fs::read_to_string(&self.log)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        (outcome, calls, shown)
    }

    /// Back to `HEAD`: index and working tree, untracked files removed.
    fn reset(&self) {
        self.git(&["reset", "-q", "--hard", "HEAD"]);
        self.git(&["clean", "-q", "-f", "-d"]);
    }
}

impl Drop for HookRepo {
    fn drop(&mut self) {
        // Nothing of the hook's run lands under the scratch HOME.
        let home = snapshot(self.git.home());
        if !std::thread::panicking() {
            assert!(home.is_empty(), "under HOME: {:?}", home.keys());
        }
    }
}

#[test]
fn a_and_b_the_call_a_bad_document_refused_a_clean_one_committed() {
    let repo = HookRepo::new("hook-ab", None);

    // (b) refused: a staged bad `.md`; (a) the one call, exactly.
    write(&repo.top, "docs/bad.md", BAD);
    repo.git(&["add", "docs/bad.md"]);
    let (outcome, calls, shown) = repo.commit("bad", &[], None);
    assert_eq!(outcome, Commit::Refused, "{shown}");
    assert_eq!(calls, [CALL], "(a) {shown}");
    assert!(shown.contains(": class-missing: "), "the report: {shown}");
    assert!(shown.contains(" \u{2014} blocked"), "{shown}");
    assert!(shown.contains(EXPORT), "the message names X: {shown}");
    repo.reset();

    // (b) recorded: a clean one.
    write(&repo.top, "docs/b.md", clean("B"));
    repo.git(&["add", "docs/b.md"]);
    let (outcome, calls, shown) = repo.commit("clean", &[], None);
    assert_eq!(outcome, Commit::Recorded, "{shown}");
    assert_eq!(calls, [CALL], "(a) {shown}");
}

#[test]
fn c_d_nothing_relevant_is_not_checked_the_config_and_baseline_are() {
    let repo = HookRepo::new("hook-cd", None);

    // (c) only a non-document: no call, recorded.
    write(&repo.top, "notes.txt", "changed\n");
    write(&repo.top, "docs/data.txt", "not a document\n");
    repo.git(&["add", "notes.txt", "docs/data.txt"]);
    let (outcome, calls, shown) = repo.commit("notes", &[], None);
    assert_eq!(outcome, Commit::Recorded, "{shown}");
    assert!(calls.is_empty(), "(c) {calls:?}");

    // (d) only `specengine.toml`, its canon cap below `docs/guide.md`.
    let capped = CONFIG.replace("canon_bytes = 12288", "canon_bytes = 10");
    write(&repo.top, "specengine.toml", &capped);
    repo.git(&["add", "specengine.toml"]);
    let (outcome, calls, shown) = repo.commit("cap", &[], None);
    assert_eq!(outcome, Commit::Refused, "(d) config: {shown}");
    assert_eq!(calls, [CALL], "{shown}");
    assert!(
        shown.contains("docs/guide.md:") && shown.contains(": budget: "),
        "{shown}"
    );
    repo.reset();

    // (d) only a bad `.spec-debt.toml`: cannot-check, refused.
    write(&repo.top, ".spec-debt.toml", "[[debt]]\ncode = 1\n");
    repo.git(&["add", ".spec-debt.toml"]);
    let (outcome, calls, shown) = repo.commit("baseline", &[], None);
    assert_eq!(outcome, Commit::Refused, "(d) baseline: {shown}");
    assert_eq!(calls, [CALL], "{shown}");
    assert!(shown.contains(" \u{2014} cannot-check"), "{shown}");
    repo.reset();

    // A good `.spec-debt.toml` alone is checked and refused: its entry is
    // new debt against `HEAD`'s baseline (none), which blocks under
    // `enforce` too (docs/features/spec-cli-introduced.md, 2a.2 Q7).
    write(
        &repo.top,
        ".spec-debt.toml",
        "[[debt]]\ncode    = \"budget\"\npath    = \"docs/guide.md\"\nreason  = \"r\"\nexpires = \"2999-12-31\"\n",
    );
    repo.git(&["add", ".spec-debt.toml"]);
    let (outcome, calls, shown) = repo.commit("good baseline", &[], None);
    assert_eq!(outcome, Commit::Refused, "{shown}");
    assert_eq!(calls, [CALL], "{shown}");
    assert!(
        shown.contains("new  docs/guide.md: debt-new: "),
        "the new entry is named: {shown}"
    );
    assert!(shown.contains(" \u{2014} blocked"), "{shown}");
}

#[test]
fn e_k_a_document_renamed_away_or_made_a_symlink_is_checked() {
    let repo = HookRepo::new("hook-ek", None);

    // (e) `git mv docs/a.md docs/a.txt`: the `.md` side of the rename counts.
    repo.git(&["mv", "docs/a.md", "docs/a.txt"]);
    let (outcome, calls, shown) = repo.commit("rename", &[], None);
    assert_eq!(calls, [CALL], "(e) {shown}");
    assert_eq!(outcome, Commit::Recorded, "{shown}");

    // (k) `docs/k.md` made a symlink: a type change, checked.
    fs::remove_file(repo.top.join("docs/k.md")).unwrap();
    std::os::unix::fs::symlink("../notes.txt", repo.top.join("docs/k.md")).unwrap();
    repo.git(&["add", "docs/k.md"]);
    let status = String::from_utf8(repo.git.git(
        &repo.top,
        &["diff", "--cached", "--name-status", "--no-renames"],
    ))
    .unwrap();
    assert_eq!(status, "T\tdocs/k.md\n", "a type change");
    let (outcome, calls, shown) = repo.commit("symlink", &[], None);
    assert_eq!(calls, [CALL], "(k) {shown}");
    assert_eq!(outcome, Commit::Recorded, "{shown}");
}

#[test]
fn f_five_thousand_staged_documents_one_bad_are_refused() {
    let repo = HookRepo::new("hook-f", None);
    let bulk = repo.top.join("docs/bulk");
    fs::create_dir_all(&bulk).unwrap();
    let text = clean("Bulk");
    for n in 0..5_000 {
        fs::write(bulk.join(format!("f{n:04}.md")), &text).unwrap();
    }
    fs::write(bulk.join("f4999.md"), BAD).unwrap();
    repo.git(&["add", "docs/bulk"]);
    let (outcome, calls, shown) = repo.commit("bulk", &[], None);
    assert_eq!(
        outcome,
        Commit::Refused,
        "(f) {}",
        &shown[..shown.len().min(4000)]
    );
    assert_eq!(calls, [CALL]);
    assert!(shown.contains("docs/bulk/f4999.md:1: class-missing: "));
}

#[test]
fn g_h_i_a_failing_cargo_commit_a_and_no_verify() {
    let repo = HookRepo::new("hook-ghi", None);

    // (g) the shim exits 127 (no toolchain) or 2: refused, naming X.
    write(&repo.top, "docs/b.md", clean("B"));
    repo.git(&["add", "docs/b.md"]);
    for exit in ["127", "2"] {
        let (outcome, calls, shown) = repo.commit("cargo fails", &[], Some(exit));
        assert_eq!(outcome, Commit::Refused, "(g) exit {exit}: {shown}");
        assert_eq!(calls, [CALL], "{shown}");
        assert!(shown.contains(EXPORT), "{shown}");
    }
    repo.reset();

    // (h) `commit -a`, the error only on disk: refused.
    write(&repo.top, "docs/a.md", BAD);
    let (outcome, calls, shown) = repo.commit("commit -a", &["-a"], None);
    assert_eq!(outcome, Commit::Refused, "(h) {shown}");
    assert_eq!(calls, [CALL], "{shown}");
    assert!(shown.contains("docs/a.md:1: class-missing: "), "{shown}");
    repo.reset();

    // (i) `--no-verify`: recorded, no call.
    write(&repo.top, "docs/bad.md", BAD);
    repo.git(&["add", "docs/bad.md"]);
    let (outcome, calls, shown) = repo.commit("skip", &["--no-verify"], None);
    assert_eq!(outcome, Commit::Recorded, "(i) {shown}");
    assert!(calls.is_empty(), "(i) {calls:?}");
}

/// `git args` in this repository, read-only (no optional locks).
fn repository_git(args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn j_the_hook_is_executable_and_without_the_bit_it_is_lost() {
    let hook = repository_root().join(".githooks/pre-commit");
    let mode = fs::metadata(&hook).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755, "on disk: {mode:o}");
    let index = repository_git(&["ls-files", "-s", "--", ".githooks/pre-commit"]);
    assert!(index.starts_with("100755 "), "the index: {index}");
    let head = repository_git(&["ls-tree", "HEAD", "--", ".githooks/pre-commit"]);
    assert!(head.starts_with("100755 "), "HEAD: {head}");

    // The copy made without the bit: git ignores the hook, (b)'s staged bad
    // document is recorded.
    let repo = HookRepo::new("hook-j", Some(0o644));
    write(&repo.top, "docs/bad.md", BAD);
    repo.git(&["add", "docs/bad.md"]);
    let (outcome, calls, shown) = repo.commit("bad, no hook", &[], None);
    assert_eq!(outcome, Commit::Recorded, "{shown}");
    assert!(calls.is_empty(), "{calls:?}");
}

/// AC-11: the workflow's one command is the check over the checkout.
#[test]
fn the_workflow_runs_only_the_check() {
    let workflow =
        fs::read_to_string(repository_root().join(".github/workflows/docs.yml")).expect("docs.yml");
    let runs: Vec<&str> = workflow
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("- run: ")
                .or_else(|| line.trim().strip_prefix("run: "))
        })
        .collect();
    assert_eq!(
        runs,
        ["cargo run --locked -q -p specengine-cli -- check --root ."],
        "{workflow}"
    );
    for retired in ["xtask", "sync-saving"] {
        assert!(!workflow.contains(retired), "{retired} in docs.yml");
        let hook = fs::read_to_string(repository_root().join(".githooks/pre-commit")).unwrap();
        assert!(!hook.contains(retired), "{retired} in the hook");
    }
}
