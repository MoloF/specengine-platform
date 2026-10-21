//! docs/features/impact.md TE-7, the owner's pilots by hand (`#[ignore]`,
//! release, read-only): with `SPECENGINE_PILOT_DIR` naming a pilot's root,
//! `SPECENGINE_PILOT_REFS` its REFs and `SPECENGINE_PILOT_SYMBOLS` its
//! queries (each comma-separated, either may be empty), a scratch `HOME`:
//! each `spec impact REF` and `spec impact --symbol QUERY` answers (exit 0,
//! or 1 for a query no symbol matches), warm (the median of five) under
//! 1 s; only the scratch data directory is written: every file under the
//! pilot and its `git status` as before. Each run is killed past the
//! common watchdog (`common::RUN_TIMEOUT`). No pilot's name or path is in
//! this repository: the variables carry them.
//!
//! `SPECENGINE_PILOT_DIR=… SPECENGINE_PILOT_REFS=… SPECENGINE_PILOT_SYMBOLS=…
//! cargo nextest run -p specengine-cli --release --test impact_pilot
//! --run-ignored ignored-only --no-capture`

#![cfg(unix)]

mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Run, Scratch};

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// The comma-separated values of `name`, trimmed, none empty.
fn listed(name: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `git status --porcelain=v1 -z --untracked-files=all` without the
/// optional index refresh, at `root` (empty outside git).
fn status(root: &Path) -> Vec<u8> {
    Command::new("git")
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map(|output| output.stdout)
        .unwrap_or_default()
}

/// TE-7: the pilot's latency per form, the pilot untouched.
#[test]
#[ignore = "manual (TE-7): the owner's pilots, read-only"]
fn impact_pilot() {
    let root =
        PathBuf::from(std::env::var_os("SPECENGINE_PILOT_DIR").expect("SPECENGINE_PILOT_DIR"));
    let refs = listed("SPECENGINE_PILOT_REFS");
    let symbols = listed("SPECENGINE_PILOT_SYMBOLS");
    assert!(
        !refs.is_empty() || !symbols.is_empty(),
        "SPECENGINE_PILOT_REFS or SPECENGINE_PILOT_SYMBOLS"
    );
    let scratch = Scratch::new("im-pilot");
    let home = scratch.home("h");
    let path = std::env::var_os("PATH").unwrap_or_default();
    let vars: Vec<(&str, OsString)> = vec![("HOME", home.into_os_string()), ("PATH", path)];
    let env: Vec<(&str, &std::ffi::OsStr)> = vars
        .iter()
        .map(|(name, value)| (*name, value.as_os_str()))
        .collect();
    let run = |args: &[&str]| -> Run { common::spec_with(&root, args, &env) };
    let files_before = common::snapshot(&root);
    let status_before = status(&root);
    let mut forms: Vec<(String, Vec<String>)> = refs
        .iter()
        .map(|reference| {
            (
                reference.clone(),
                vec!["impact".to_owned(), reference.clone()],
            )
        })
        .collect();
    forms.extend(symbols.iter().map(|query| {
        (
            format!("--symbol {query}"),
            vec!["impact".to_owned(), "--symbol".to_owned(), query.clone()],
        )
    }));
    for (label, args) in &forms {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let first = run(&args);
        assert!(
            [0, 1].contains(&first.code),
            "{label}: an answer\n{}",
            first.show()
        );
        let took = median(|| {
            let started = Instant::now();
            let again = run(&args);
            assert_eq!(again.code, first.code, "{label}: {}", again.show());
            started.elapsed()
        });
        let lines = first.stdout.lines().next().unwrap_or_default().to_owned();
        eprintln!(
            "TE-7 pilot: impact {label} warm {} ms; exit {}; {lines}",
            took.as_millis(),
            first.code
        );
        assert!(took < Duration::from_secs(1), "{label}: {took:?}");
    }
    assert!(
        common::snapshot(&root) == files_before,
        "the pilot's files unchanged"
    );
    assert_eq!(
        status(&root),
        status_before,
        "the pilot's git status unchanged"
    );
}
