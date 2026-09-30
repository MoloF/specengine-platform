//! AC-10 and AC-14 of docs/features/spec-cli-check.md on this repository's
//! own documents, with the parity config of the store's `check_parity.rs`
//! (shared through a `#[path]` module, not copied; it keeps `xtask`'s
//! registry: this repository registers nothing until 2b).
//!
//! AC-10: on a scratch copy of the walked documents, `docs/index.md`
//! deleted, `spec export index` recreates it byte-identical to the working
//! tree's, and `spec check` is then `clean`, exit 0. AC-14 (dogfood):
//! `spec check --root <this repository> --config <the parity config in a
//! temp dir>` is `clean`, exit 0, stdout the library's; `git status
//! --porcelain --untracked-files=all` is the same before and after (git
//! only observes). The repository is only read.

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
use parity_config::{INDEX, parity_toml};
use specengine_core::ProjectConfig;
use specengine_core::check::Verdict;
use specengine_store::{WorkingTree, check_input};

fn git_status(root: &Path) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git status");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The documents the parity config walks in this repository.
fn walked_documents(repository: &Path) -> Vec<String> {
    let toml = parity_toml(repository, true);
    let project = ProjectConfig::from_toml(&toml).expect("the parity config");
    let tree = WorkingTree::new(repository, &project.paths).expect("the working tree");
    let input = check_input(&tree, &project.scheme);
    assert!(input.problems.is_empty(), "{:?}", input.problems);
    input.files.into_iter().map(|file| file.path).collect()
}

#[test]
fn export_index_recreates_this_repository_s_index() {
    let repository = repository_root();
    let documents = walked_documents(&repository);
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
    write(&copy, "specengine.toml", parity_toml(&copy, true));
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

    let report = library(&copy);
    assert_eq!(report.verdict, Verdict::Clean, "{:?}", report.findings);
    let run = spec(&home, &copy, &["check"]);
    run.code(0);
    assert_eq!(run.stdout, text(&report, false));
    assert!(run.stdout.ends_with(" — clean\n"), "{}", run.stdout);
    assert!(snapshot(&home).is_empty(), "something under HOME");
}

#[test]
fn dogfood_check_of_this_repository_is_clean_and_changes_nothing() {
    let repository = repository_root();
    let scratch = Scratch::new("dogfood");
    let home = scratch.home("h");
    let config = scratch.join("cfg/parity.toml");
    write(
        scratch.path(),
        "cfg/parity.toml",
        parity_toml(&repository, true),
    );
    let before = git_status(&repository);

    let root_arg = repository.to_str().unwrap();
    let config_arg = config.to_str().unwrap();
    let run = spec(
        &home,
        scratch.path(),
        &["check", "--root", root_arg, "--config", config_arg],
    );
    run.code(0);
    let report = library_with(&repository, &config, None);
    assert_eq!(report.verdict, Verdict::Clean);
    assert_eq!(run.stdout, text(&report, false));
    assert!(run.stdout.ends_with(" — clean\n"), "{}", run.stdout);
    assert_eq!(run.stderr, "");

    assert_eq!(git_status(&repository), before, "the repository changed");
    assert!(snapshot(&home).is_empty(), "something under HOME");
}
