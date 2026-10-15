//! docs/features/rev-rule.md TE-9: manual, over a real project, never
//! written. `SPECENGINE_PILOT_DIR` names the project's root (its
//! `specengine.toml`, inside a git worktree); the test clones the worktree
//! into a scratch directory (`git clone --no-hardlinks`) and runs only
//! there. First, as the project is: `spec check --staged`, `--changed` and
//! `spec bump` give no `rev-not-raised` while `HEAD` holds no `spec.lock`
//! (the counts and wall times printed). With `SPECENGINE_PILOT_ACCEPT=1`
//! it then accepts, in the clone only, every ID a marker token names (one
//! lock commit each, as `verify_pilot.rs`), changes the text of every
//! locked node by one sentence, stages it and prints the rule's in-scope
//! count (its findings), `spec bump --staged`'s rows and their wall times.
//! The clone's lock and the project's `HEAD`, status and index are checked
//! unchanged. No project name or path is printed or written anywhere.
//!
//! Run in the foreground, release, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --release
//! --test rev_rule_pilot --run-ignored ignored-only --no-capture`.

mod common;
mod lock_common;
mod rev_rule_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Scratch;
use common::git::Sandbox;
use lock_common::{CLOCK, LockRepo, blocks, value};
use rev_rule_common::{Bump, Check, bump_at, check_at, spec_at, stage_all};

/// The sentence every locked node's text gains.
const SENTENCE: &str = "A sentence the rule must see.";

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

/// `spec check <flag>`, timed: its rule findings printed by count.
fn timed_check(repo: &LockRepo, flag: &str) -> Check {
    let started = Instant::now();
    let check = check_at(repo, &repo.root, &[flag], &[]);
    eprintln!(
        "TE-9 check {flag}: exit {}, verdict {}, {} rev-not-raised, {} causes, wall {:?} (text and JSON)",
        check.code(),
        check.verdict(),
        check.rev().len(),
        check.causes().len(),
        started.elapsed()
    );
    check
}

/// `spec bump <flags>`, timed: its rows by need.
fn timed_bump(repo: &LockRepo, flags: &[&str]) -> Bump {
    let started = Instant::now();
    let bump = bump_at(repo, &repo.root, flags, &[]);
    let mut needs: BTreeMap<String, usize> = BTreeMap::new();
    if bump.code() == 0 {
        for (_, need) in bump.needs() {
            *needs.entry(need).or_default() += 1;
        }
    }
    eprintln!(
        "TE-9 bump {flags:?}: exit {}, rows {needs:?}, wall {:?} (text and JSON)",
        bump.code(),
        started.elapsed()
    );
    bump
}

/// `id`'s text in the clone given [`SENTENCE`] as a paragraph of its own:
/// after a section's heading line, or after a document's front-matter.
/// `false` when `spec show` does not find it.
fn change_text(repo: &LockRepo, id: &str) -> bool {
    let run = spec_at(repo, &repo.root, &["--json", "show", id], &[]);
    if run.code != 0 {
        return false;
    }
    let shown = run.json();
    let node = &shown["nodes"][0];
    let (Some(path), Some(line)) = (node["path"].as_str(), node["line"].as_u64()) else {
        return false;
    };
    let text = repo.read(path);
    let mut lines: Vec<&str> = text.split('\n').collect();
    let after = if line == 1 && lines.first() == Some(&"---") {
        match lines.iter().skip(1).position(|line| *line == "---") {
            Some(at) => at + 2,
            None => return false,
        }
    } else {
        usize::try_from(line).expect("a line")
    };
    lines.insert(after, SENTENCE);
    lines.insert(after, "");
    repo.write(path, lines.join("\n"));
    true
}

#[test]
#[ignore = "manual (TE-9): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_rev_rule_counts() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("rev-rule-pilot");
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

    // As the project is: no lock at `HEAD`, no finding.
    let locked_at_head = repo
        .git
        .git_output(&repo.root, &["cat-file", "-e", "HEAD:./spec.lock"], &[])
        .status
        .success();
    eprintln!("TE-9 HEAD holds a spec.lock: {locked_at_head}");
    for flag in ["--staged", "--changed"] {
        let check = timed_check(&repo, flag);
        if !locked_at_head {
            assert!(check.rev().is_empty(), "silent without a lock ({flag})");
        }
    }
    let bump = timed_bump(&repo, &[]);
    if !locked_at_head && bump.code() == 0 {
        assert!(
            bump.needs().iter().all(|(_, need)| need == "suggested"),
            "nothing required without a lock"
        );
    }

    if std::env::var_os("SPECENGINE_PILOT_ACCEPT").is_some() {
        let ids = marker_ids(&repo.git, &clone);
        let started = Instant::now();
        let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
        for id in &ids {
            let attempt = repo.attempt_at(&repo.root, id, false, CLOCK, true);
            let kind = match &attempt.outcome {
                Ok(outcome) if outcome.commit.is_some() => "committed",
                Ok(outcome) if outcome.unchanged => "nothing to change",
                Ok(_) => "exit 1",
                Err(_) => "exit 2",
            };
            *outcomes.entry(kind.to_owned()).or_default() += 1;
        }
        eprintln!(
            "TE-9 accepts in the clone: {} IDs, {:?}, {outcomes:?}",
            ids.len(),
            started.elapsed()
        );
        let lock_before = repo.lock();
        let nodes: BTreeSet<String> = repo
            .lock()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map(|text| {
                blocks(&text)
                    .iter()
                    .filter_map(|block| value(block, "node").map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let edited = nodes.iter().filter(|node| change_text(&repo, node)).count();
        eprintln!(
            "TE-9 locked nodes: {}, their text changed: {edited}",
            nodes.len()
        );
        stage_all(&repo);
        let check = timed_check(&repo, "--staged");
        eprintln!("TE-9 in scope (findings): {}", check.rev().len());
        assert!(
            check.rev().len() <= edited,
            "a finding per changed node at most"
        );
        if edited > 0 {
            assert!(
                !check.rev().is_empty(),
                "the changed locked nodes are judged"
            );
        }
        timed_bump(&repo, &["--staged"]);
        assert_eq!(
            repo.lock(),
            lock_before,
            "check and bump wrote the clone's lock"
        );
    }

    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}
