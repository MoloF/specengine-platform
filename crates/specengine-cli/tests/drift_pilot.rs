//! docs/features/drift-queue.md TE-12: manual, over a real project, never
//! written. `SPECENGINE_PILOT_DIR` names the project's root (its
//! `specengine.toml`, inside a git worktree); the test clones the worktree
//! into a scratch directory (`git clone --no-hardlinks`), locks it there
//! by its own accept (every ID a marker token names, `spec lock accept`
//! through the library, consent given, one lock commit each; the clone
//! only), then raises `spec propose drift` (a sweep) in the clone with a
//! scratch data directory: it prints the items by state, the counts of
//! created and hit items, the rows and nodes judged and the wall time
//! (against AC-10's baseline); a second raise stores nothing (every item a
//! hit). It checks the clone's HEAD, status and lock and the project's
//! HEAD, status and index unchanged. No project name or path is printed
//! or written anywhere.
//!
//! Run in the foreground, release, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --release
//! --test drift_pilot --run-ignored ignored-only --no-capture`.

mod common;
mod lock_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Scratch;
use common::git::Sandbox;
use lock_common::{CLOCK, LockRepo};
use specengine_cli::{DriftRequest, Exit, Globals, Outcome, propose_drift, render_json};

/// HEAD, status (read without the optional index refresh, which would
/// write) and index of a worktree.
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

/// The IDs marker tokens name in the tracked `.rs` and `.ron` files.
fn marker_ids(git: &Sandbox, top: &Path) -> BTreeSet<String> {
    let output = git.git_output(
        top,
        &[
            "grep",
            "-h",
            "-o",
            "-E",
            "@(implements|verifies|configures|assumes)[[:space:]]+[^[:space:]]+",
            "--",
            "*.rs",
            "*.ron",
        ],
        &[],
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|token| token.split('@').next().unwrap_or(token).to_owned())
        .filter(|id| !id.is_empty() && id.is_ascii())
        .collect()
}

/// One sweep in the clone at `now`: the drift document.
fn sweep(repo: &LockRepo, now: &str) -> serde_json::Value {
    let outcome = propose_drift(
        &repo.env(&repo.root),
        &Globals::default(),
        &DriftRequest {
            references: Vec::new(),
            author_role: Some("tester".to_owned()),
            author_model: None,
            run: None,
            now: now.to_owned(),
            git: repo.git_env(&repo.root),
        },
        None,
    )
    .unwrap_or_else(|error| panic!("propose drift: exit {:?}: {}", error.exit, error.message));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "refused: {:?}",
        outcome.refusal
    );
    serde_json::from_str(&render_json(&Outcome::Drift(Box::new(outcome)))).expect("JSON")
}

#[test]
#[ignore = "manual (TE-12): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_drift_raise() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("drift-pilot");
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

    // Locked by the test's own accepts, in the clone only.
    let ids = marker_ids(&repo.git, &clone);
    let started = Instant::now();
    let mut accepted: BTreeMap<String, usize> = BTreeMap::new();
    for id in &ids {
        let attempt = repo.attempt_at(&repo.root, id, false, CLOCK, true);
        let kind = match &attempt.outcome {
            Ok(outcome) if outcome.commit.is_some() => "committed",
            Ok(outcome) if outcome.unchanged => "nothing to change",
            Ok(_) => "exit 1",
            Err(_) => "exit 2",
        };
        *accepted.entry(kind.to_owned()).or_default() += 1;
    }
    eprintln!(
        "TE-12 accepts in the clone: {} IDs, {:?}, {accepted:?}",
        ids.len(),
        started.elapsed()
    );

    let clone_before = state(&repo.git, &clone);
    let lock_before = repo.lock();
    let started = Instant::now();
    let first = sweep(&repo, "2026-10-14T09:00:00Z");
    let elapsed = started.elapsed();
    let created = first["created"].as_array().expect("created");
    let mut by_summary: BTreeMap<String, usize> = BTreeMap::new();
    for item in created {
        for label in item["summary"].as_str().expect("a summary").split(", ") {
            *by_summary.entry(label.to_owned()).or_default() += 1;
        }
    }
    eprintln!(
        "TE-12 raise: {} created, {} hits, {} rows of {} nodes judged, {} notes, wall {elapsed:?}",
        created.len(),
        first["hits"].as_array().map_or(0, Vec::len),
        first["judged"]["rows"],
        first["judged"]["nodes"],
        first["notes"].as_array().map_or(0, Vec::len),
    );
    for (label, count) in &by_summary {
        eprintln!("  {count:6}  {label}");
    }
    let second = sweep(&repo, "2026-10-14T09:05:00Z");
    assert_eq!(
        second["created"].as_array().map_or(0, Vec::len),
        0,
        "a second raise stores nothing"
    );
    assert_eq!(
        second["hits"].as_array().map_or(0, Vec::len),
        created.len(),
        "every item a hit"
    );
    let printed = first.to_string();
    let root_text = repo.root.display().to_string();
    assert!(!printed.contains(&root_text), "an absolute path printed");
    assert_eq!(
        state(&repo.git, &clone),
        clone_before,
        "the raise changed the clone"
    );
    assert_eq!(repo.lock(), lock_before, "the raise wrote the clone's lock");
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}
