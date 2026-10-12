//! docs/features/spec-lock.md TE-11: manual, over a real project, never
//! written. `SPECENGINE_PILOT_DIR` names the project's root (its
//! `specengine.toml`, inside a git worktree); the test clones the worktree
//! into a scratch directory (`git clone --no-hardlinks`), and there, for
//! every ID a marker token names, runs `spec lock accept` through the
//! library with the consent declined: nothing is ever written, not even in
//! the clone. It prints the counts of the rows by change and state and the
//! refusals by kind, and checks the project's own HEAD, status and index
//! unchanged. No project name or path is written anywhere.
//!
//! Run in the foreground, release, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --release
//! --test lock_pilot --run-ignored ignored-only --no-capture`.

mod common;
mod lock_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use common::Scratch;
use common::git::Sandbox;
use lock_common::{CLOCK, LockRepo};

/// The project's state that must not change: HEAD, status (read without
/// the optional index refresh, which would write), index.
fn pilot_state(git: &Sandbox, top: &Path) -> (Option<String>, Vec<u8>, String) {
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

/// The IDs marker tokens name in the clone's tracked `.rs` and `.ron`
/// files (`@implements|verifies|configures|assumes ID[@rev]`).
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

#[test]
#[ignore = "manual (TE-11): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_rows_by_state() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("lock-pilot");
    let git = Sandbox::new(scratch.path());
    // The scratch sandbox reads the project too (its ceiling is the
    // scratch, which the project is not under); nothing is made beside it.
    let pilot_git = git.clone();
    let top = PathBuf::from(pilot_git.git_text(&root, &["rev-parse", "--show-toplevel"]));
    let inner = root
        .strip_prefix(&top)
        .expect("the root lies in its worktree")
        .to_path_buf();
    let before = pilot_state(&pilot_git, &top);

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
    let ids = marker_ids(&repo.git, &clone);
    let mut rows: BTreeMap<String, usize> = BTreeMap::new();
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    let started = std::time::Instant::now();
    for id in &ids {
        let attempt = repo.attempt_at(&repo.root, id, false, CLOCK, false);
        let kind = match &attempt.outcome {
            Ok(outcome) if outcome.unchanged => "exit 0 nothing to change".to_owned(),
            Ok(outcome) => match outcome.refusal.as_deref() {
                Some("not confirmed; nothing written") => "asked (declined)".to_owned(),
                Some(reason) => format!(
                    "exit 1 {}",
                    reason
                        .split(':')
                        .next()
                        .unwrap_or(reason)
                        .chars()
                        .take(40)
                        .collect::<String>()
                ),
                None => "exit 0".to_owned(),
            },
            Err(error) => format!(
                "exit 2 {}",
                error.message.chars().take(50).collect::<String>()
            ),
        };
        *outcomes.entry(kind).or_default() += 1;
        for line in attempt.rows().lines() {
            let fields: Vec<&str> = line.split(" | ").collect();
            let change = fields.first().copied().unwrap_or_default();
            let state = if fields.len() > 4 { fields[4] } else { "" };
            let state = state.split_whitespace().next().unwrap_or_default();
            *rows
                .entry(format!("{change} {state}").trim().to_owned())
                .or_default() += 1;
        }
    }
    eprintln!(
        "TE-11: {} IDs named by markers, {:?}",
        ids.len(),
        started.elapsed()
    );
    for (kind, count) in &outcomes {
        eprintln!("  outcome  {count:6}  {kind}");
    }
    for (kind, count) in &rows {
        eprintln!("  row      {count:6}  {kind}");
    }
    assert!(repo.lock().is_none(), "the clone's lock was written");
    assert_eq!(pilot_state(&pilot_git, &top), before, "the project changed");
}
