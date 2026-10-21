//! docs/features/tested-evidence.md (8b-1) TE-7 (Q-H, 8a's TE-9): manual,
//! over a real project, run by the owner; the project itself is never
//! written. `SPECENGINE_PILOT_DIR` names the project's root (its
//! `specengine.toml`, inside a git worktree); the test clones the worktree
//! into a scratch directory (`git clone --no-hardlinks`), gives the clone a
//! `[verify] tests` naming the fake runner of the 8a-1 tests (never the
//! project's own command; nothing builds) and commits it there, then runs
//! `spec verify --tests` in the clone through the library, the owner's
//! answer `y`: the blocks land in the clone only. It prints counts and
//! timings only (the run and its write, then plain `spec verify` over the
//! clone and its `tested` entries by state), never a project name or path,
//! and checks that the project's HEAD, status and index are unchanged, that
//! the clone holds at most one new commit beyond the setup (the evidence
//! commit, `spec.lock` alone), and that a project without a bound test
//! writes nothing (`empty`).
//!
//! Run in the foreground, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --test
//! tested_pilot --run-ignored ignored-only --no-capture`.

#![cfg(unix)]

mod common;
mod lock_common;
mod verify_tests_common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Scratch;
use common::git::Sandbox;
use lock_common::LockRepo;
use serde_json::Value;
use verify_tests_common::{Kiln, RUNNER, toml_list};

/// HEAD, status (without the optional index refresh) and index.
fn state(git: &Sandbox, top: &Path) -> (Option<String>, Vec<u8>, String) {
    (
        git.head(top),
        git.git(
            top,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
        ),
        git.git_text(top, &["ls-files", "-s"]),
    )
}

/// `config` with `tests = <template>` in its `[verify]` table (made when
/// absent), any `tests` or `timeout` it had dropped.
fn with_tests(config: &str, template: &str) -> String {
    let mut out = String::new();
    let mut in_verify = false;
    let mut placed = false;
    for line in config.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_verify = trimmed == "[verify]";
        }
        if in_verify && (trimmed.starts_with("tests") || trimmed.starts_with("timeout")) {
            continue;
        }
        out.push_str(line);
        if in_verify && trimmed == "[verify]" {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("tests = {template}\n"));
            placed = true;
        }
    }
    if !placed {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("\n[verify]\ntests = {template}\n"));
    }
    out
}

/// How many entries of `list` have each value of `key`.
fn by(list: &Value, key: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for entry in list.as_array().into_iter().flatten() {
        let value = match &entry[key] {
            Value::String(text) => text.clone(),
            Value::Null => "null".to_owned(),
            other => other.to_string(),
        };
        *out.entry(value).or_default() += 1;
    }
    out
}

#[test]
#[ignore = "manual (TE-7): set SPECENGINE_PILOT_DIR to a project root; the project is only read"]
fn pilot_tested_evidence() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("tested-pilot");
    let git = Sandbox::new(scratch.path());
    let pilot_git = git.clone();
    let top = PathBuf::from(pilot_git.git_text(&root, &["rev-parse", "--show-toplevel"]));
    let inner = root
        .strip_prefix(&top)
        .expect("the root lies in its worktree")
        .to_path_buf();
    let before = state(&pilot_git, &top);

    let clone = scratch.join("clone");
    git.git(
        scratch.path(),
        &[
            "clone",
            "-q",
            "--no-hardlinks",
            top.to_str().expect("a UTF-8 path"),
            clone.to_str().expect("a UTF-8 path"),
        ],
    );
    git.quiet(&clone);
    let repo = LockRepo {
        root: clone.join(&inner),
        home: scratch.home("h"),
        git,
        scratch,
    };
    let config = repo.read("specengine.toml");
    let slug = config
        .parse::<toml::Table>()
        .ok()
        .and_then(|table| {
            table
                .get("project")
                .and_then(|project| project.get("slug"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        })
        .expect("the project's slug");
    let template = toml_list(&[RUNNER, "{test}"]);
    repo.write("specengine.toml", with_tests(&config, &template));
    let kiln = Kiln::bare(repo, Box::leak(slug.into_boxed_str()));
    let _reaper = kiln.reaper();
    kiln.write_runner(RUNNER, 0o755);
    kiln.repo
        .commit_all("a scratch verify table and a fake runner");
    let setup = kiln.repo.head();
    let lock_before = kiln.repo.lock();

    let started = Instant::now();
    let run = kiln.run();
    let elapsed = started.elapsed();
    let head = kiln.repo.head();
    let root_text = kiln.root().display().to_string();
    if run.outcome.is_some() {
        let doc = run.json();
        let rows = by(&doc["lock"]["rows"], "change");
        eprintln!(
            "TE-7 verify --tests: exit {}, verdict {}, counts {}, lock rows {rows:?}, committed {}, refusal {}, wall {elapsed:?}",
            run.code,
            doc["verdict"],
            doc["counts"],
            doc["lock"]["commit"].is_string(),
            doc["lock"]["refusal"],
        );
        assert!(!run.text().contains(&root_text), "an absolute path printed");
        if doc["verdict"] == "empty" {
            assert_eq!(head, setup, "no bound test: nothing written");
            assert_eq!(
                kiln.repo.lock(),
                lock_before,
                "no bound test: the lock as it was"
            );
        }
    } else {
        eprintln!(
            "TE-7 verify --tests: exit {}: {:?}",
            run.code,
            run.stderr_lines()
        );
    }
    if head != setup {
        assert_eq!(
            kiln.repo.parents(&head),
            std::slice::from_ref(&setup),
            "one evidence commit"
        );
        let top_relative = inner.join("spec.lock");
        assert_eq!(
            kiln.repo
                .git_text(&["diff-tree", "--no-commit-id", "--name-only", "-r", &head]),
            top_relative.display().to_string(),
            "the evidence commit changes spec.lock alone"
        );
    }

    let started = Instant::now();
    let plain = kiln.repo.spec_piped(kiln.root(), &["verify", "--json"]);
    let plain_elapsed = started.elapsed();
    let document = plain.json();
    eprintln!(
        "TE-7 spec verify: exit {}, verdict {}, rows {}, tested {} by state {:?}, wall {plain_elapsed:?}",
        plain.code,
        document["verdict"],
        document["rows"].as_array().map_or(0, Vec::len),
        document["tested"].as_array().map_or(0, Vec::len),
        by(&document["tested"], "state"),
    );
    assert!(
        !plain.stdout.contains(&root_text),
        "an absolute path printed"
    );
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}
