//! docs/features/health-code.md TE-7, the CLI half: manual, over a real
//! project, never written. `SPECENGINE_PILOT_DIR` names the project's root
//! (inside a git worktree); `SPECENGINE_PILOT_CONFIG`, when set, names a
//! config kept outside the project (with its `[[health.bound]]` rules),
//! passed as `--config`. A scratch `HOME` holds the data directory: the
//! first call builds both indexes in the call (the code index's
//! `full_ms`), a repeat refreshes and re-derives nothing (`noop`). It
//! prints rows 4-5's states, causes counted, `counts`, the stored code
//! files and the two wall times (against the 2 s of AC-10 and the code
//! index's `noop_ms` ≤ 200, the latter `specengine-eval code-index`'s own
//! figure, run apart), and checks the project's HEAD, status and index
//! unchanged. No project name, path, ID or cause text is printed or
//! written anywhere.
//!
//! docs/features/health-drift.md TE-7: row 6 too: its state, causes
//! counted, `counts`, rows and evidence counted, and its share of a warm
//! call (the same call with no `git` on `PATH`, row 6 `cannot`, rows 3-5
//! byte-equal), against AC-10's 2 s for the whole.
//!
//! Run in the foreground, release, one pilot at a time:
//! `SPECENGINE_PILOT_DIR=<root> [SPECENGINE_PILOT_CONFIG=<file>] cargo
//! nextest run -p specengine-cli --release --test health_code_pilot
//! --run-ignored ignored-only --no-capture`.

#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::git::Sandbox;
use common::{Run, Scratch, spec, spec_with};
use serde_json::Value;

/// The repeat's budget (AC-10's 2 s).
const REPEAT_BUDGET: Duration = Duration::from_secs(2);

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

/// `spec --root <root> [--config <config>] health --json` with `HOME`
/// `home`, and its wall time.
fn timed_health(home: &Path, root: &Path, config: Option<&Path>) -> (Run, Duration) {
    let root_text = root.to_str().expect("a UTF-8 root").to_owned();
    let config_text = config.map(|config| config.to_str().expect("a UTF-8 config").to_owned());
    let mut args: Vec<&str> = vec!["--root", root_text.as_str()];
    if let Some(config) = &config_text {
        args.extend(["--config", config.as_str()]);
    }
    args.extend(["health", "--json"]);
    let started = Instant::now();
    let run = spec(home, root, &args);
    (run, started.elapsed())
}

/// `spec --root <root> [--config <config>] health --json` with `HOME`
/// `home` and `PATH` an empty directory (row 6 `cannot`: no `git`), and
/// its wall time.
fn timed_health_without_git(
    home: &Path,
    root: &Path,
    config: Option<&Path>,
    empty: &Path,
) -> (Run, Duration) {
    let root_text = root.to_str().expect("a UTF-8 root").to_owned();
    let config_text = config.map(|config| config.to_str().expect("a UTF-8 config").to_owned());
    let mut args: Vec<&str> = vec!["--root", root_text.as_str()];
    if let Some(config) = &config_text {
        args.extend(["--config", config.as_str()]);
    }
    args.extend(["health", "--json"]);
    let started = Instant::now();
    let run = spec_with(
        root,
        &args,
        &[("HOME", home.as_os_str()), ("PATH", empty.as_os_str())],
    );
    (run, started.elapsed())
}

/// Row 6's state, its causes counted, its counts, rows and evidence
/// counted: no name, no path.
fn drift_shape(row: &Value) -> String {
    format!(
        "{} | causes {} | counts {} | rows {} | tested {} | notes {}",
        row["state"].as_str().unwrap_or("?"),
        row["causes"].as_array().map_or(0, Vec::len),
        row["counts"],
        row["rows"].as_array().map_or(0, Vec::len),
        row["tested"]
            .as_array()
            .map_or_else(|| "null".to_owned(), |entries| entries.len().to_string()),
        row["notes"].as_array().map_or(0, Vec::len)
    )
}

/// A row's state, its causes counted and its counts: no name, no path.
fn shape(row: &Value) -> String {
    format!(
        "{} | causes {} | counts {} | rows {}",
        row["state"].as_str().unwrap_or("?"),
        row["causes"].as_array().map_or(0, Vec::len),
        row["counts"],
        row["rows"].as_array().map_or(0, Vec::len)
    )
}

/// TE-7: rows 4-5 on a pilot, their figures and the call's wall time,
/// first and repeated; read-only proved.
#[test]
#[ignore = "manual (TE-7): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_health_code_rows() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let config = std::env::var_os("SPECENGINE_PILOT_CONFIG").map(PathBuf::from);
    if let Some(config) = &config {
        assert!(
            !config
                .canonicalize()
                .expect("the config exists")
                .starts_with(&root),
            "the pilot's config lives outside the project"
        );
    }
    let scratch = Scratch::new("health-code-pilot");
    let git = Sandbox::new(scratch.path());
    let top = PathBuf::from(git.git_text(&root, &["rev-parse", "--show-toplevel"]));
    let before = state(&git, &top);

    let home = scratch.home("h");
    let (first, first_time) = timed_health(&home, &root, config.as_deref());
    assert_eq!(first.code, 0, "exit {}", first.code);
    let (again, again_time) = timed_health(&home, &root, config.as_deref());
    assert_eq!(again.code, 0, "exit {}", again.code);
    assert_eq!(again.stdout, first.stdout, "a repeat gives the same bytes");
    let document = first.json();
    println!("documents: {}", document["documents"]);
    println!("code: {}", document["code"]);
    println!("assumes: {}", shape(&document["assumes"]));
    println!("unbound: {}", shape(&document["unbound"]));
    println!("drift: {}", drift_shape(&document["drift"]));
    let empty = scratch.dir("no-git-bin");
    let (without, without_time) = timed_health_without_git(&home, &root, config.as_deref(), &empty);
    assert_eq!(without.code, 0, "exit {}", without.code);
    let rows_3_5 = without.json();
    assert_eq!(
        rows_3_5["drift"]["state"], "cannot",
        "no git: row 6 `cannot`"
    );
    for key in [
        "documents",
        "amendments",
        "notes",
        "code",
        "assumes",
        "unbound",
    ] {
        assert_eq!(
            rows_3_5[key], document[key],
            "rows 3-5's `{key}` without row 6"
        );
    }
    println!(
        "row 6's share of a warm call: {} ms (repeat {} ms, rows 3-5 alone {} ms)",
        again_time.saturating_sub(without_time).as_millis(),
        again_time.as_millis(),
        without_time.as_millis()
    );
    println!(
        "stderr lines: first {}, repeat {}",
        first.stderr.lines().count(),
        again.stderr.lines().count()
    );
    println!(
        "wall time: first {} ms, repeat {} ms (AC-10: 2000 ms)",
        first_time.as_millis(),
        again_time.as_millis()
    );
    assert!(
        again_time <= REPEAT_BUDGET,
        "a repeat took {} ms",
        again_time.as_millis()
    );
    assert_eq!(state(&git, &top), before, "the project is unchanged");
}
