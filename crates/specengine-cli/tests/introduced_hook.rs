//! AC-17 of docs/features/spec-cli-introduced.md, on the harness of 2b's
//! AC-10 (`pre_commit_hook.rs`): this repository's own
//! `.githooks/pre-commit`, copied with its mode into scratch repositories
//! (every git process in the scratch sandbox), `core.hooksPath` at it, and
//! a logging `cargo` shim first on `PATH` that execs the built `spec` with
//! the arguments after `run -q -p specengine-cli --`. The project carries a
//! backlog at `HEAD` (an erroneous document committed with
//! `--no-verify`) under `enforce-introduced`: a clean edit commits; an
//! introduced error, a `git mv` of the erroneous document and a new
//! baseline entry (also under `enforce`) are refused; `--no-verify` commits
//! an error, after which a clean `--amend` commits (R1 pinned: the error
//! is pre-existing against the amended commit's `HEAD`). This repository
//! stays `enforce` and its hook runs the same one command.

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use common::git::Sandbox;
use common::{SPEC, Scratch, read_text, repository_root, snapshot, write};

const PREFIX: &str = "run -q -p specengine-cli --";
const CALL: &str = "run -q -p specengine-cli -- check --staged --root .";

fn config(mode: &str) -> String {
    format!(
        "[paths]\nroots = [\"docs\"]\n\n[ids]\nR = {{ kind = \"requirement\", width = 2 }}\n\n[classes]\nspec  = {{ required = [\"class\", \"status\", \"scope\"], closed = true }}\n\n[check]\nmode = \"{mode}\"\n"
    )
}

fn clean(title: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# {title}\n\nText.\n")
}

/// No front-matter: `class-missing`, an error.
const BAD: &str = "# A document without a class\n";

const DEBT: &str = "[[debt]]\ncode    = \"class-missing\"\npath    = \"docs/backlog.md\"\nreason  = \"the backlog\"\nexpires = \"2999-12-31\"\n";

const NEW_ENTRY: &str = "\n[[debt]]\ncode    = \"class-missing\"\npath    = \"docs/later.md\"\nreason  = \"planned\"\nexpires = \"2999-12-31\"\n";

#[derive(Debug, PartialEq, Eq)]
enum Commit {
    Recorded,
    Refused,
}

struct HookRepo {
    _scratch: Scratch,
    git: Sandbox,
    top: PathBuf,
    log: PathBuf,
    shim: PathBuf,
}

impl HookRepo {
    /// The project in `mode` with a backlog (`docs/backlog.md`, an error),
    /// `baseline` as `.spec-debt.toml` when given, the real hook, one
    /// commit made with `--no-verify`, `core.hooksPath` set.
    fn new(name: &str, mode: &str, baseline: Option<&str>) -> Self {
        let scratch = Scratch::new(name);
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        write(&top, "specengine.toml", config(mode));
        write(&top, "docs/a.md", clean("A"));
        write(&top, "docs/backlog.md", BAD);
        if let Some(baseline) = baseline {
            write(&top, ".spec-debt.toml", baseline);
        }
        let hook = top.join(".githooks/pre-commit");
        fs::create_dir_all(hook.parent().unwrap()).unwrap();
        fs::copy(repository_root().join(".githooks/pre-commit"), &hook).unwrap();
        git.init(&top);
        git.git(&top, &["config", "core.hooksPath", ".githooks"]);
        git.add_all(&top);
        git.commit(&top, "the backlog");

        let bin = scratch.dir("bin");
        let shim = bin.join("cargo");
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\n\
                 printf '%s\\n' \"$*\" >> \"$SHIM_LOG\"\n\
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

    /// `git commit -q -m <message> <args>`, the shim first on `PATH`: the
    /// outcome (judged by `HEAD`), the shim's calls, the whole run.
    fn commit(&self, message: &str, args: &[&str]) -> (Commit, Vec<String>, String) {
        fs::write(&self.log, "").unwrap();
        let before = self.git.head(&self.top);
        let mut full = vec!["commit", "-q", "-m", message];
        full.extend_from_slice(args);
        let path = std::env::join_paths(std::iter::once(self.shim.clone()).chain(
            std::env::split_paths(self.git.var("PATH").expect("the sandbox PATH")),
        ))
        .unwrap();
        let extra: Vec<(&str, &OsStr)> = vec![
            ("PATH", path.as_os_str()),
            ("SHIM_LOG", self.log.as_os_str()),
        ];
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

    fn reset(&self) {
        self.git(&["reset", "-q", "--hard", "HEAD"]);
        self.git(&["clean", "-q", "-f", "-d"]);
    }
}

impl Drop for HookRepo {
    fn drop(&mut self) {
        let home = snapshot(self.git.home());
        if !std::thread::panicking() {
            assert!(home.is_empty(), "under HOME: {:?}", home.keys());
        }
    }
}

/// A clean edit over the backlog commits; an introduced error and a
/// `git mv` of the erroneous document are refused.
#[test]
fn a_backlog_lets_a_clean_edit_through_and_refuses_what_a_commit_adds() {
    let repo = HookRepo::new("intro-hook-edit", "enforce-introduced", None);

    write(&repo.top, "docs/a.md", clean("A, edited"));
    repo.git(&["add", "docs/a.md"]);
    let (outcome, calls, shown) = repo.commit("a clean edit", &[]);
    assert_eq!(outcome, Commit::Recorded, "{shown}");
    assert_eq!(calls, [CALL], "{shown}");

    write(&repo.top, "docs/bad.md", BAD);
    repo.git(&["add", "docs/bad.md"]);
    let (outcome, calls, shown) = repo.commit("an introduced error", &[]);
    assert_eq!(outcome, Commit::Refused, "{shown}");
    assert_eq!(calls, [CALL], "{shown}");
    assert!(
        shown.contains("error  docs/bad.md:1: class-missing: "),
        "{shown}"
    );
    assert!(
        !shown.contains("error  docs/backlog.md"),
        "the backlog does not block: {shown}"
    );
    repo.reset();

    repo.git(&["mv", "docs/backlog.md", "docs/moved.md"]);
    let (outcome, calls, shown) = repo.commit("a move", &[]);
    assert_eq!(outcome, Commit::Refused, "{shown}");
    assert_eq!(calls, [CALL], "{shown}");
    assert!(
        shown.contains("error  docs/moved.md:1: class-missing: "),
        "{shown}"
    );
}

/// A new baseline entry is refused under `enforce-introduced` and under
/// `enforce` (the backlog in debt at `HEAD`).
#[test]
fn a_new_baseline_entry_is_refused_in_both_modes() {
    for mode in ["enforce-introduced", "enforce"] {
        let repo = HookRepo::new("intro-hook-debt", mode, Some(DEBT));
        write(&repo.top, ".spec-debt.toml", format!("{DEBT}{NEW_ENTRY}"));
        repo.git(&["add", ".spec-debt.toml"]);
        let (outcome, calls, shown) = repo.commit("a new entry", &[]);
        assert_eq!(outcome, Commit::Refused, "{mode}: {shown}");
        assert_eq!(calls, [CALL], "{shown}");
        assert!(
            shown.contains("new  docs/later.md: debt-new: "),
            "{mode}: {shown}"
        );
        // `--no-verify` records it, no call.
        let (outcome, calls, shown) = repo.commit("a new entry, skipped", &["--no-verify"]);
        assert_eq!(outcome, Commit::Recorded, "{mode}: {shown}");
        assert!(calls.is_empty(), "{calls:?}");
    }
}

/// R1 pinned: an error committed with `--no-verify` is pre-existing from
/// then on; a clean `--amend` of that very commit runs the hook and is
/// recorded.
#[test]
fn an_error_committed_with_no_verify_then_amended_stays_pre_existing() {
    let repo = HookRepo::new("intro-hook-r1", "enforce-introduced", None);
    write(&repo.top, "docs/bad.md", BAD);
    repo.git(&["add", "docs/bad.md"]);
    let (outcome, calls, shown) = repo.commit("an error, unchecked", &["--no-verify"]);
    assert_eq!(outcome, Commit::Recorded, "{shown}");
    assert!(calls.is_empty(), "{calls:?}");

    write(&repo.top, "docs/a.md", clean("A, amended"));
    repo.git(&["add", "docs/a.md"]);
    let (outcome, calls, shown) = repo.commit("amended", &["--amend"]);
    assert_eq!(outcome, Commit::Recorded, "R1: {shown}");
    assert_eq!(calls, [CALL], "the hook ran: {shown}");
    let tree = String::from_utf8(
        repo.git
            .git(&repo.top, &["ls-tree", "-r", "--name-only", "HEAD"]),
    )
    .unwrap();
    assert!(tree.lines().any(|path| path == "docs/bad.md"), "{tree}");
}

/// This repository stays `enforce`, and its hook runs exactly the one
/// command the shim saw above.
#[test]
fn this_repository_stays_enforce_with_the_same_hook_command() {
    let root = repository_root();
    let text = read_text(&root, "specengine.toml");
    let check = specengine_core::check::CheckConfig::from_toml(&text).expect("this config");
    assert_eq!(check.mode, specengine_core::check::Mode::Enforce);
    let hook = read_text(&root, ".githooks/pre-commit");
    let runs: Vec<&str> = hook
        .lines()
        .filter(|line| line.contains("cargo run") && !line.trim_start().starts_with('#'))
        .collect();
    assert_eq!(
        runs.iter()
            .filter(|line| line.contains(&format!("cargo {CALL}")))
            .count(),
        1,
        "{runs:?}"
    );
}
