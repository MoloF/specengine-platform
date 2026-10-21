//! docs/features/tested-reads.md (8b-2) TE-7: manual, over a real
//! project, run by the owner; the project itself is never written.
//! `SPECENGINE_PILOT_DIR` names the project's root (its `specengine.toml`,
//! inside a git worktree); the test clones the worktree into a scratch
//! directory (`git clone --no-hardlinks`, the committed state) and reads
//! the clone with a scratch `HOME`. It prints counts and timings only,
//! never a project name, path or ID, and checks that the project's HEAD,
//! status and index are unchanged.
//!
//! With blocks in the lock (after the owner's passing `spec verify
//! --tests`): for each node holding a present block (at most 20), every
//! entry `show <node> --bindings`, `impact <node>` and the package of a
//! scratch task on those nodes show equals plain `spec verify --json`'s for
//! that node, key for key; a node without sections lists `verify --node`'s;
//! the package lists `verify --task T --json`'s; every bundle line is the
//! line of a plain-verify entry. Without a block: every read's `tested` is
//! `[]` (`null` without `[code]`).
//!
//! Run in the foreground, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --test
//! tested_reads_pilot --run-ignored ignored-only --no-capture`.

#![cfg(unix)]

mod bindings_common;
mod code_index_common;
mod common;
mod lock_common;
mod tested_common;
mod verify_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use bindings_common::{list, read_with, run_with, text, vars_of};
use common::Scratch;
use common::git::Sandbox;
use lock_common::LockRepo;
use serde_json::Value;

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

/// `spec verify <args> --json`'s document from the clone's root.
fn verify(repo: &LockRepo, args: &[&str]) -> Value {
    let mut all = vec!["verify"];
    all.extend_from_slice(args);
    all.push("--json");
    let run = run_with(&repo.root, &all, &vars_of(repo));
    common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("verify {args:?}: {problem} (exit {})", run.code))
}

#[test]
#[ignore = "manual (TE-7): set SPECENGINE_PILOT_DIR to a project root; the project is only read"]
fn pilot_tested_reads() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("tested-reads-pilot");
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
    let vars = vars_of(&repo);
    let root_text = repo.root.display().to_string();

    let started = Instant::now();
    let plain = verify(&repo, &[]);
    let plain_elapsed = started.elapsed();
    let entries: BTreeMap<String, Value> = plain["tested"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["state"] != "node_gone")
        .map(|entry| (text(&entry["node"]).to_owned(), entry.clone()))
        .collect();
    let lines: BTreeSet<String> = entries.values().map(tested_common::entry_line).collect();
    eprintln!(
        "TE-7 plain verify: tested {} (present {}), wall {plain_elapsed:?}",
        plain["tested"].as_array().map_or(0, Vec::len),
        entries.len()
    );
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut walls: BTreeMap<&str, std::time::Duration> = BTreeMap::new();
    let mut timed = |name: &'static str, args: &[&str]| {
        let started = Instant::now();
        let read = read_with(&repo.root, args, &vars);
        *walls.entry(name).or_default() += started.elapsed();
        *counts.entry(name).or_default() += 1;
        for printed in [&read.text.stdout, &read.json_run.stdout] {
            assert!(
                !printed.contains(&root_text),
                "{name}: an absolute path printed"
            );
        }
        read
    };

    let nodes: Vec<String> = if entries.is_empty() {
        // No block: a sample of bound nodes, every read `[]`.
        let mut bound: Vec<String> = list(&plain["rows"])
            .iter()
            .map(|row| text(&row["node"]).to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        bound.truncate(5);
        bound
    } else {
        entries.keys().take(20).cloned().collect()
    };
    let code = plain["tested"].is_array();
    for node in &nodes {
        let shown = timed("show --bindings", &["show", node, "--bindings"]);
        for shown_node in list(&shown.json()["nodes"]) {
            let tested = &shown_node["bindings"]["tested"];
            if entries.is_empty() {
                assert!(
                    tested.as_array().is_some_and(Vec::is_empty) || (!code && tested.is_null()),
                    "show: no block, `tested` []"
                );
                continue;
            }
            for entry in list(tested) {
                assert_eq!(
                    Some(&entry),
                    entries.get(text(&entry["node"])),
                    "show: an entry is plain verify's"
                );
            }
            if list(&shown_node["sections"]).is_empty() {
                assert_eq!(
                    tested,
                    &verify(&repo, &["--node", node])["tested"],
                    "show: a leaf lists verify --node's"
                );
            }
        }
        let reached = timed("impact", &["impact", node]);
        let document = reached.json();
        if entries.is_empty() {
            assert!(
                document["tested"].as_array().is_some_and(Vec::is_empty)
                    || (!code && document["tested"].is_null()),
                "impact: no block, `tested` []"
            );
        } else {
            for entry in list(&document["tested"]) {
                assert_eq!(
                    Some(&entry),
                    entries.get(text(&entry["node"])),
                    "impact: an entry is plain verify's"
                );
            }
        }
        let bundle = timed("bundle", &["bundle", node]);
        for line in tested_common::read_tested_lines(&bundle.text.stdout) {
            assert!(lines.contains(&line), "bundle: a line is plain verify's");
        }
    }
    if !nodes.is_empty() {
        let picked: Vec<&str> = nodes.iter().take(3).map(String::as_str).collect();
        let task = verify_common::new_task(&repo, &picked);
        let package = timed("task show", &["task", "show", &task]).json();
        assert_eq!(
            package["tested"],
            verify(&repo, &["--task", &task])["tested"],
            "the package lists verify --task's"
        );
    }
    eprintln!(
        "TE-7 reads: nodes {}, runs {counts:?}, wall {walls:?}",
        nodes.len()
    );
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}
