//! docs/features/health-code.md TE-7, the route's half: manual, over a
//! real project, never written. `SPECENGINE_PILOT_DIR` names the project's
//! root (holding its `specengine.toml`, inside a git worktree); a daemon
//! with a scratch `HOME` serves it, its alarm bounding the run, and stops
//! with the test. `GET health` first (both indexes built in the project's
//! turn) and repeated: the same bytes, the two wall times printed (against
//! the 2 s of AC-10), rows 4-5's states and counts; the project's HEAD and
//! status unchanged. No project name, path, ID or cause text is printed.
//!
//! docs/features/health-drift.md TE-7: row 6's state, causes counted,
//! `counts`, rows and evidence counted, in the same body (the route's
//! repeat against AC-10's 2 s already holds row 6's cost).
//!
//! Run in the foreground, release, one pilot at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-http
//! --release --test health_code_pilot --run-ignored ignored-only
//! --no-capture`.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{Scratch, Server};

/// The repeat's budget (AC-10's 2 s).
const REPEAT_BUDGET: Duration = Duration::from_secs(2);

/// TE-7: the route's wall time on a pilot, first and repeated.
#[test]
#[ignore = "manual (TE-7): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_health_route_timing() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("health-route-pilot");
    let git = scratch.git();
    let status = |git: &common::Git| {
        (
            git.run(&root, &["rev-parse", "HEAD"]),
            git.run(
                &root,
                &[
                    "--no-optional-locks",
                    "status",
                    "--porcelain=v1",
                    "-z",
                    "--untracked-files=all",
                ],
            ),
        )
    };
    let before = status(&git);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let projects = server.get("/api/projects").status(200).json();
    let slug = projects[0]["slug"]
        .as_str()
        .expect("the served project's slug")
        .to_owned();
    let path = format!("/api/projects/{slug}/health");

    let started = Instant::now();
    let first = server.get(&path);
    let first_time = started.elapsed();
    assert_eq!(first.status, 200, "status {}", first.status);
    let started = Instant::now();
    let again = server.get(&path);
    let again_time = started.elapsed();
    assert_eq!(again.status, 200, "status {}", again.status);
    assert_eq!(again.text(), first.text(), "a repeat gives the same bytes");
    let document = first.json();
    for row in ["assumes", "unbound"] {
        println!(
            "{row}: {} | counts {}",
            document[row]["state"].as_str().unwrap_or("?"),
            document[row]["counts"]
        );
    }
    println!("code: {}", document["code"]);
    let drift = &document["drift"];
    println!(
        "drift: {} | causes {} | counts {} | rows {} | tested {}",
        drift["state"].as_str().unwrap_or("?"),
        drift["causes"].as_array().map_or(0, Vec::len),
        drift["counts"],
        drift["rows"].as_array().map_or(0, Vec::len),
        drift["tested"]
            .as_array()
            .map_or_else(|| "null".to_owned(), |entries| entries.len().to_string())
    );
    println!(
        "route wall time: first {} ms, repeat {} ms (AC-10: 2000 ms)",
        first_time.as_millis(),
        again_time.as_millis()
    );
    drop(server);
    assert!(
        again_time <= REPEAT_BUDGET,
        "a repeat took {} ms",
        again_time.as_millis()
    );
    assert_eq!(status(&git), before, "the project is unchanged");
}
