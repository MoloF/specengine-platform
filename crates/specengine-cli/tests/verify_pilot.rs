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
//!
//! docs/features/verify-node.md TE-5 (`pilot_verify_node_latency`), alike:
//! in the clone, the code index warmed by one `spec index`, warm `spec
//! verify --node <ID> --json` per sampled marker ID (every
//! [`NODE_SAMPLE_EVERY`]th of the sorted marker IDs, at most
//! [`NODE_SAMPLES`]) against 1 s, plain `spec verify --json` alongside;
//! the IDs are numbered, never printed. It decides 6c.
//!
//! docs/features/show-bindings.md TE-6 (`pilot_show_bindings_latency`),
//! alike: warm `spec show <ID> --bindings --json` per sampled marker ID
//! against 1 s, `spec show <ID> --json` alongside; every answer exit 0 or
//! 1, its rows counted by `sync` (a degraded read's note counted); the
//! clone and the project unchanged. Figures only.

mod common;
mod lock_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// A scratch clone of the project `SPECENGINE_PILOT_DIR` names: the
/// repository (its root the project root's place in the clone), the
/// clone's top, the project's git and top, and the project's state.
fn pilot_clone(label: &str) -> (LockRepo, PathBuf, Sandbox, PathBuf, ProjectState) {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new(label);
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
    (repo, clone, pilot_git, top, before)
}

/// HEAD, status and index of a worktree ([`state`]).
type ProjectState = (Option<String>, Vec<u8>, String);

#[test]
#[ignore = "manual (TE-10): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_verify_counts() {
    let (repo, clone, pilot_git, top, before) = pilot_clone("verify-pilot");

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

/// TE-5's sample: every this many-th sorted marker ID.
const NODE_SAMPLE_EVERY: usize = 7;

/// TE-5's sample: at most this many IDs.
const NODE_SAMPLES: usize = 12;

/// MCP's per-call bound (`mcp-read.md` "Latency"), which 6b-2b's
/// `check_binding` must keep: TE-5 records each call against it.
const NODE_BOUND: Duration = Duration::from_secs(1);

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// docs/features/verify-node.md TE-5: warm `spec verify --node <ID>
/// --json` per sampled marker ID in a scratch clone (every exit an
/// answer: 0, 1 or 2 with a document, or 2 with stdout empty for an ID
/// naming nothing), the median of five against [`NODE_BOUND`], plain
/// `spec verify --json` alongside; the clone's and the project's HEAD,
/// status and index and the clone's lock unchanged. Prints figures only:
/// no ID, project name or path.
#[test]
#[ignore = "manual (verify-node TE-5): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_verify_node_latency() {
    let (repo, clone, pilot_git, top, before) = pilot_clone("verify-node-pilot");
    let ids: Vec<String> = marker_ids(&repo.git, &clone)
        .into_iter()
        .step_by(NODE_SAMPLE_EVERY)
        .take(NODE_SAMPLES)
        .collect();
    assert!(!ids.is_empty(), "the project holds marker IDs");
    let clone_before = state(&repo.git, &clone);
    let lock_before = repo.lock();
    let index = repo.spec_piped(&repo.root, &["index"]);
    assert_eq!(index.code, 0, "spec index warms the code index");

    let root_text = repo.root.display().to_string();
    let plain = median(|| {
        let started = Instant::now();
        let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
        let took = started.elapsed();
        assert!([0, 1, 2].contains(&run.code), "plain: exit {}", run.code);
        took
    });
    eprintln!(
        "TE-5 plain verify, warm, median of 5: {} ms",
        plain.as_millis()
    );
    let mut slowest = Duration::ZERO;
    let mut over = 0;
    for (number, id) in ids.iter().enumerate() {
        let mut rows = 0;
        let mut code = 0;
        let took = median(|| {
            let started = Instant::now();
            let run = repo.spec_piped(&repo.root, &["verify", "--node", id, "--json"]);
            let took = started.elapsed();
            assert!(
                [0, 1, 2].contains(&run.code),
                "sample {number}: exit {}",
                run.code
            );
            assert!(
                !run.stdout.contains(&root_text) && !run.stderr.contains(&root_text),
                "sample {number}: an absolute path printed"
            );
            code = run.code;
            rows = if run.stdout.is_empty() {
                0
            } else {
                run.json()["rows"].as_array().map_or(0, Vec::len)
            };
            took
        });
        slowest = slowest.max(took);
        if took > NODE_BOUND {
            over += 1;
        }
        eprintln!(
            "TE-5 sample {number}: exit {code}, {rows} rows, warm median of 5 {} ms{}",
            took.as_millis(),
            if took > NODE_BOUND { " (over 1 s)" } else { "" }
        );
    }
    eprintln!(
        "TE-5 verify --node: {} samples, slowest {} ms, {over} over 1 s; plain {} ms",
        ids.len(),
        slowest.as_millis(),
        plain.as_millis()
    );
    assert_eq!(
        state(&repo.git, &clone),
        clone_before,
        "verify --node changed the clone"
    );
    assert_eq!(
        repo.lock(),
        lock_before,
        "verify --node wrote the clone's lock"
    );
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}

/// docs/features/show-bindings.md TE-6: warm `spec show <ID> --bindings
/// --json` per sampled marker ID in a scratch clone, the median of five
/// against [`NODE_BOUND`], the same read without `--bindings` alongside;
/// every read answers (exit 0 or 1, never 2: no code cause fails a read);
/// the clone's and the project's HEAD, status and index and the clone's
/// lock unchanged. Prints figures only: no ID, project name or path.
#[test]
#[ignore = "manual (show-bindings TE-6): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_show_bindings_latency() {
    let (repo, clone, pilot_git, top, before) = pilot_clone("show-bindings-pilot");
    let ids: Vec<String> = marker_ids(&repo.git, &clone)
        .into_iter()
        .step_by(NODE_SAMPLE_EVERY)
        .take(NODE_SAMPLES)
        .collect();
    assert!(!ids.is_empty(), "the project holds marker IDs");
    let clone_before = state(&repo.git, &clone);
    let lock_before = repo.lock();
    let index = repo.spec_piped(&repo.root, &["index"]);
    assert_eq!(index.code, 0, "spec index warms the indexes");

    let root_text = repo.root.display().to_string();
    let mut slowest = Duration::ZERO;
    let mut over = 0;
    let mut by_sync: BTreeMap<String, usize> = BTreeMap::new();
    let mut notes = 0;
    for (number, id) in ids.iter().enumerate() {
        let mut rows = 0;
        let mut code = 0;
        let took = median(|| {
            let started = Instant::now();
            let run = repo.spec_piped(&repo.root, &["show", id, "--bindings", "--json"]);
            let took = started.elapsed();
            assert!(
                [0, 1].contains(&run.code),
                "sample {number}: exit {}, never a code cause",
                run.code
            );
            assert!(
                !run.stdout.contains(&root_text) && !run.stderr.contains(&root_text),
                "sample {number}: an absolute path printed"
            );
            code = run.code;
            rows = 0;
            if run.code == 0 {
                let json = run.json();
                for node in json["nodes"].as_array().expect("nodes") {
                    for row in node["bindings"]["rows"].as_array().into_iter().flatten() {
                        rows += 1;
                        *by_sync
                            .entry(row["sync"].as_str().unwrap_or("-").to_owned())
                            .or_default() += 1;
                    }
                }
            }
            took
        });
        if run_has_note(&repo, id) {
            notes += 1;
        }
        let plain = median(|| {
            let started = Instant::now();
            let run = repo.spec_piped(&repo.root, &["show", id, "--json"]);
            let took = started.elapsed();
            assert!(
                [0, 1].contains(&run.code),
                "sample {number}: exit {}",
                run.code
            );
            took
        });
        slowest = slowest.max(took);
        if took > NODE_BOUND {
            over += 1;
        }
        eprintln!(
            "TE-6 sample {number}: exit {code}, {rows} rows, warm median of 5 {} ms (show alone {} ms){}",
            took.as_millis(),
            plain.as_millis(),
            if took > NODE_BOUND { " (over 1 s)" } else { "" }
        );
    }
    eprintln!(
        "TE-6 show --bindings: {} samples, slowest {} ms, {over} over 1 s, {notes} with a bindings note; rows by sync (five runs each) {by_sync:?}",
        ids.len(),
        slowest.as_millis()
    );
    assert_eq!(
        state(&repo.git, &clone),
        clone_before,
        "show --bindings changed the clone"
    );
    assert_eq!(
        repo.lock(),
        lock_before,
        "show --bindings wrote the clone's lock"
    );
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}

/// Whether `spec show <id> --bindings` notes a code cause
/// (`note: bindings: `).
fn run_has_note(repo: &LockRepo, id: &str) -> bool {
    repo.spec_piped(&repo.root, &["show", id, "--bindings"])
        .stderr
        .lines()
        .any(|line| line.starts_with("note: bindings: "))
}
