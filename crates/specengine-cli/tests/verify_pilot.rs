//! docs/features/spec-verify.md TE-10: manual, over a real project, never
//! written. `SPECENGINE_PILOT_DIR` names the project's root (its
//! `specengine.toml`, inside a git worktree); the test clones the worktree
//! into a scratch directory (`git clone --no-hardlinks`). With
//! `SPECENGINE_PILOT_ACCEPT=1` it first accepts, in the clone only, every
//! ID a marker token names (`spec lock accept` through the library, the
//! owner's consent given, one lock commit each); else every row is
//! `unlocked`. Then `spec verify --json`, plain and `--changed`, runs in the
//! clone: it prints the rows' counts by state and category, the counters
//! and the wall time (AC-10's baseline), and checks the clone's HEAD,
//! status and lock and the project's HEAD, status and index unchanged. No
//! project name or path is printed or written anywhere.
//!
//! Run in the foreground, release, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --release
//! --test verify_pilot --run-ignored ignored-only --no-capture`.

mod common;
mod lock_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Scratch;
use common::git::Sandbox;
use lock_common::{CLOCK, LockRepo};

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

#[test]
#[ignore = "manual (TE-10): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_verify_counts() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("verify-pilot");
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

    if std::env::var_os("SPECENGINE_PILOT_ACCEPT").is_some() {
        let ids = marker_ids(&repo.git, &clone);
        let started = Instant::now();
        let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
        for id in &ids {
            let attempt = repo.attempt_at(&repo.root, id, false, CLOCK, true);
            let kind = match &attempt.outcome {
                Ok(outcome) if outcome.commit.is_some() => "committed".to_owned(),
                Ok(outcome) if outcome.unchanged => "nothing to change".to_owned(),
                Ok(_) => "exit 1".to_owned(),
                Err(_) => "exit 2".to_owned(),
            };
            *outcomes.entry(kind).or_default() += 1;
        }
        eprintln!(
            "TE-10 accepts in the clone: {} IDs, {:?}, {outcomes:?}",
            ids.len(),
            started.elapsed()
        );
    }

    let clone_before = state(&repo.git, &clone);
    let lock_before = repo.lock();
    for extra in [&[][..], &["--changed"][..]] {
        let mut args = vec!["verify", "--json"];
        args.extend_from_slice(extra);
        let started = Instant::now();
        let run = repo.spec_piped(&repo.root, &args);
        let elapsed = started.elapsed();
        assert!(
            [0, 1, 2].contains(&run.code),
            "{extra:?}: exit {}",
            run.code
        );
        let json = run.json();
        let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
        let rows = json["rows"].as_array().expect("rows");
        for row in rows {
            let state = row["state"].as_str().expect("a state");
            let category = row["category"].as_str().unwrap_or("-");
            let pre = if row["pre_existing"] == true {
                " pre-existing"
            } else {
                ""
            };
            *by_state
                .entry(format!("{state} {category}{pre}"))
                .or_default() += 1;
        }
        eprintln!(
            "TE-10 verify {extra:?}: exit {}, {} rows, verdict {}, counts {}, cascade {}, {} causes, wall {elapsed:?}",
            run.code,
            rows.len(),
            json["verdict"],
            json["counts"],
            json["cascade"],
            json["cannot"].as_array().map_or(0, Vec::len),
        );
        for (state, count) in &by_state {
            eprintln!("  {count:6}  {state}");
        }
        let text = run.stdout.clone();
        let root_text = repo.root.display().to_string();
        assert!(!text.contains(&root_text), "an absolute path printed");
    }
    assert_eq!(
        state(&repo.git, &clone),
        clone_before,
        "verify changed the clone"
    );
    assert_eq!(repo.lock(), lock_before, "verify wrote the clone's lock");
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}
