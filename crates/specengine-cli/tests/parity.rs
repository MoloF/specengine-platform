//! This repository's own documents through the `spec` binary, configured by
//! the committed root `specengine.toml` (docs/features/spec-cli-switch.md;
//! the config and the std walk are the store's `parity_config/mod.rs`,
//! shared through a `#[path]` module, not copied).
//!
//! AC-04 (on a scratch copy of the walked documents and the root config;
//! the export never runs in this repository): with `docs/index.md` deleted,
//! X recreates it byte-identical to the working tree's; run again it
//! reports `unchanged docs/index.md: <n> bytes` and keeps the mtime;
//! `--stdout` prints the same bytes; one byte edited → `spec check` exit 1,
//! `index-drift` naming X. AC-02: G from the top (`spec check` in this
//! repository, no `--root`): exit 0, `— clean`, the stdout the library's;
//! `--json` has 0 errors, debt, expired and stale; `--debt` lists no
//! finding (mutation: the five-digit mention `ADR-00011` put back into
//! `docs/canon/spec-check.md` on a scratch copy → exactly that one
//! `mention-dangling`, exit 0). AC-14: `spec check --root <this repository>` from a
//! scratch directory, no `--config`: exit 0, the library's stdout. AC-07:
//! the summary's W is `specengine_core::check::worst_w` of the same walk.
//! Every run gets a scratch `HOME` that stays empty; `git status` (no
//! optional locks, untracked files listed; the owner's `.claude/` aside) is
//! the same before and after: the repository is only read.

#![cfg(unix)]

mod common;

#[allow(dead_code)]
#[path = "../../specengine-store/tests/parity_config/mod.rs"]
mod parity_config;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::check::{library, library_with, text};
use common::{Scratch, repository_root, snapshot, spec, write};
use parity_config::{DANGLING, EXPORT, GATE, INDEX, add_dangling_mention, root_toml, std_walk};
use specengine_core::ProjectConfig;
use specengine_core::check::{Verdict, worst_w};
use specengine_store::{WorkingTree, check_input};

/// `git status` of `root` without `.claude/` (the owner's, edited while
/// Claude Code runs; never written by `spec`).
fn git_status(root: &Path) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).claude",
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git status");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The documents the root config walks under `root`, and W of that walk
/// by the library.
fn walked_documents(root: &Path) -> (Vec<String>, u64) {
    let project = ProjectConfig::from_toml(&root_toml()).expect("the root config");
    let tree = WorkingTree::new(root, &project.paths).expect("the working tree");
    let input = check_input(&tree, &project.scheme);
    assert!(input.problems.is_empty(), "{:?}", input.problems);
    let w = worst_w(&input, &project.paths);
    (input.files.into_iter().map(|file| file.path).collect(), w)
}

/// The `worst W <n> B` value of a summary line.
fn summary_w(stdout: &str) -> u64 {
    let summary = stdout.lines().last().expect("a summary line");
    let (_, rest) = summary.split_once(", worst W ").expect("W in the summary");
    rest.split_once(" B \u{2014} ")
        .expect("` B — ` after W")
        .0
        .parse()
        .expect("W is a number")
}

#[test]
fn export_index_recreates_this_repository_s_index() {
    let repository = repository_root();
    let (documents, _) = walked_documents(&repository);
    assert!(documents.iter().any(|path| path == INDEX), "{INDEX} walked");
    let working = fs::read(repository.join(INDEX)).expect("the working-tree index");

    let scratch = Scratch::new("parity");
    let home = scratch.home("h");
    let copy = scratch.dir("copy");
    for path in &documents {
        if path != INDEX {
            write(&copy, path, fs::read(repository.join(path)).unwrap());
        }
    }
    write(&copy, "specengine.toml", root_toml());
    assert!(!copy.join(INDEX).exists());

    let run = spec(&home, &copy, &["export", "index"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("wrote {INDEX}: {} bytes\n", working.len())
    );
    assert_eq!(run.stderr, "");
    let recreated = fs::read(copy.join(INDEX)).unwrap();
    assert!(
        recreated == working,
        "the recreated index differs from the working tree's ({} vs {} bytes)",
        recreated.len(),
        working.len()
    );

    // Again: unchanged, the mtime kept.
    let mtime = fs::metadata(copy.join(INDEX)).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let run = spec(&home, &copy, &["export", "index"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("unchanged {INDEX}: {} bytes\n", working.len())
    );
    assert_eq!(
        fs::metadata(copy.join(INDEX)).unwrap().modified().unwrap(),
        mtime,
        "the unchanged index was rewritten"
    );
    // `--stdout`: the same bytes, the header naming X and G.
    let run = spec(&home, &copy, &["export", "index", "--stdout"]);
    run.code(0);
    assert!(run.stdout.as_bytes() == working, "--stdout is the file");
    let header = run.stdout.split("\n## ").next().unwrap();
    for command in [EXPORT, GATE] {
        assert!(
            header.contains(&format!("`{command}`")),
            "{command}: {header}"
        );
    }

    let report = library(&copy);
    assert_eq!(report.verdict, Verdict::Clean, "{:?}", report.findings);
    let run = spec(&home, &copy, &["check"]);
    run.code(0);
    assert_eq!(run.stdout, text(&report, false));
    assert!(run.stdout.ends_with(" \u{2014} clean\n"), "{}", run.stdout);

    // Mutation (scratch): one byte of the index edited → exit 1,
    // `index-drift` naming X.
    let mut edited = working.clone();
    let at = edited
        .windows(b"# Documentation index".len())
        .position(|window| window == b"# Documentation index")
        .expect("the index heading");
    edited[at + 2] = b'd';
    write(&copy, INDEX, &edited);
    let run = spec(&home, &copy, &["check"]);
    run.code(1);
    let drift: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| line.starts_with(&format!("error  {INDEX}:")))
        .collect();
    assert_eq!(drift.len(), 1, "{}", run.stdout);
    assert!(
        drift[0].contains(": index-drift: ") && drift[0].contains(&format!("`{EXPORT}`")),
        "{}",
        drift[0]
    );
    assert!(
        run.stdout.ends_with(" \u{2014} blocked\n"),
        "{}",
        run.stdout
    );
    assert!(snapshot(&home).is_empty(), "something under HOME");
}

#[test]
fn g_from_the_top_is_clean_with_no_finding() {
    let repository = repository_root();
    let scratch = Scratch::new("gate");
    let home = scratch.home("h");
    let before = git_status(&repository);
    let (documents, w) = walked_documents(&repository);

    let run = spec(&home, &repository, &["check"]);
    run.code(0);
    let report = library(&repository);
    assert_eq!(report.verdict, Verdict::Clean);
    assert_eq!(run.stdout, text(&report, false));
    assert!(
        run.stdout.starts_with("spec check [enforce]: ")
            && run.stdout.ends_with(" \u{2014} clean\n"),
        "{}",
        run.stdout
    );
    assert_eq!(run.stderr, "");
    assert_eq!(summary_w(&run.stdout), w, "AC-07: W is the library's");
    assert_eq!(report.counts.worst_w_bytes, w);

    let json = spec(&home, &repository, &["--json", "check"]);
    json.code(0);
    let document = json.json();
    let counts = &document["counts"];
    for key in ["errors", "debt", "expired", "stale"] {
        assert_eq!(counts[key], 0, "{key}: {document}");
    }
    assert_eq!(counts["documents"], documents.len());
    assert_eq!(counts["worst_w_bytes"], w, "{document}");
    assert_eq!(document["mode"], "enforce");
    assert_eq!(document["verdict"], "clean");

    let debt = spec(&home, &repository, &["check", "--debt"]);
    debt.code(0);
    assert_eq!(debt.stdout, text(&report, true));
    let findings: Vec<&str> = debt
        .stdout
        .lines()
        .filter(|line| !line.starts_with("spec check ["))
        .collect();
    assert!(findings.is_empty(), "the pin: no finding\n{}", debt.stdout);

    // Mutation (a scratch copy of the walked documents and the root
    // config): the five-digit mention back → `--debt` lists exactly that
    // one warning, at its line; the gate stays open.
    let copy = scratch.dir("dangling");
    for path in &documents {
        write(&copy, path, fs::read(repository.join(path)).unwrap());
    }
    write(&copy, "specengine.toml", root_toml());
    let line = add_dangling_mention(&copy);
    let (code, path, subject) = DANGLING;
    let run = spec(&home, &copy, &["check", "--debt"]);
    run.code(0);
    assert_eq!(run.stdout, text(&library(&copy), true));
    let findings: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| !line.starts_with("spec check ["))
        .collect();
    assert_eq!(findings.len(), 1, "{}", run.stdout);
    assert!(
        findings[0].starts_with(&format!("warning  {path}:{line}: {code}: "))
            && findings[0].contains(subject),
        "the mutation (code, path, subject, line {line}): {}",
        findings[0]
    );
    assert!(run.stdout.ends_with(" \u{2014} clean\n"), "{}", run.stdout);

    assert_eq!(git_status(&repository), before, "the repository changed");
    assert!(snapshot(&home).is_empty(), "something under HOME");
    assert_eq!(std_walk(&repository).len(), documents.len());
}

#[test]
fn dogfood_check_of_this_repository_is_clean_and_changes_nothing() {
    let repository = repository_root();
    let scratch = Scratch::new("dogfood");
    let home = scratch.home("h");
    let before = git_status(&repository);
    let (_, w) = walked_documents(&repository);

    let root_arg = repository.to_str().unwrap();
    let run = spec(&home, scratch.path(), &["check", "--root", root_arg]);
    run.code(0);
    let report = library_with(&repository, &repository.join("specengine.toml"), None);
    assert_eq!(report.verdict, Verdict::Clean);
    assert_eq!(run.stdout, text(&report, false));
    assert!(run.stdout.ends_with(" \u{2014} clean\n"), "{}", run.stdout);
    assert_eq!(run.stderr, "");
    assert_eq!(summary_w(&run.stdout), w, "AC-07: W is the library's");

    assert_eq!(git_status(&repository), before, "the repository changed");
    assert!(snapshot(&home).is_empty(), "something under HOME");
}
