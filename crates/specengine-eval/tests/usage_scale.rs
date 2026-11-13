//! docs/features/usage-orphans.md (RM-0111) "Acceptance criteria", UO12
//! [measure]: the uses at scale, on `catalog_scale.rs`'s generated tree (the
//! tree of `specengine-eval scale --corpus-only --nodes N --seed 1`, its
//! areas named the spec, committed once into git), with no history on top
//! (the uses read no git).
//!
//! - `uo12_usage_holds_at_1000_nodes` (default run): core `usage_of` over the
//!   tree's graph is what the library's catalog (`--archive`, every row) and
//!   Health's row 8 serve: each row's `used_by`, `mentioned_by` and
//!   `orphan` by place, `live` the live documents, the rows exactly the
//!   orphans, by kind then path; some node is used (the premise).
//! - `uo12_measure_1k`, `uo12_measure_10k`, `uo12_measure_100k` (`#[ignore]`,
//!   release): the same check, then, warm, the median of five runs of the
//!   release `spec catalog --kind node --json` (the whole read), of core's
//!   `usage_of` over the built graph (the pass the slice adds) and of the
//!   release `spec health --json`. Bars at 10 000 nodes: the catalog's whole
//!   read at most 2 s, the usage pass at most 100 ms, Health at most 1.5 s;
//!   1 000 and 100 000 are reported.
//!
//! Run the measure: `cargo build --release -p specengine-cli --bin spec`,
//! then `cargo nextest run -p specengine-eval --release --test usage_scale
//! --run-ignored only --no-capture`.
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, with a `HOME` of its own.
//! Nothing here names a pilot; every name is invented.

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use git::Sandbox;
use import_support::{Scratch, eval, stderr};
use specengine_cli::{
    CatalogRequest, Env, GitEnv, Globals, HealthRequest, View, catalog_with_view, health,
};
use specengine_core::ProjectConfig;
use specengine_core::check::SpecGraph;
use specengine_core::usage::{NodeUsage, usage_of};
use specengine_store::{WorkingTree, check_input};

/// The kind of every part node of the generated tree.
const KIND: &str = "node";
/// Timed runs; the median is kept.
const RUNS: usize = 5;
/// UO12's bars at 10 000 nodes.
const CATALOG_BAR: Duration = Duration::from_secs(2);
const USAGE_BAR: Duration = Duration::from_millis(100);
const HEALTH_BAR: Duration = Duration::from_millis(1_500);

/// Core's answer by `(path, line)`: each node's usage, its kind, whether it
/// is a document.
type CoreAnswer = BTreeMap<(String, usize), (NodeUsage, Option<String>, bool)>;

/// The generated tree, committed, and its `HOME`.
struct Tree {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    git: Sandbox,
}

impl Tree {
    fn new(label: &str, nodes: usize) -> Self {
        let scratch = Scratch::new(&format!("usage-scale-{label}"));
        let home = scratch.join("home");
        fs::create_dir_all(&home).expect("a home");
        let out = scratch.join("out");
        let output = eval()
            .env("HOME", &home)
            .env_remove("XDG_DATA_HOME")
            .arg("scale")
            .arg("--out")
            .arg(&out)
            .args([
                "--corpus-only",
                "--nodes",
                &nodes.to_string(),
                "--seed",
                "1",
            ])
            .output()
            .expect("specengine-eval runs");
        assert!(
            output.status.success(),
            "`scale --corpus-only --nodes {nodes} --seed 1`: {}",
            stderr(&output)
        );
        let root =
            fs::canonicalize(out.join(format!("scale/synthetic/n{nodes}/tree"))).expect("the tree");
        // As catalog_scale.rs: the areas named the spec, so the catalog's
        // directionless tree lists the part nodes.
        let config = root.join("specengine.toml");
        let text = fs::read_to_string(&config).expect("the config");
        let anchor = "tier1_name = \"README.md\"\n";
        assert_eq!(text.matches(anchor).count(), 1, "{text}");
        fs::write(
            &config,
            text.replacen(anchor, &format!("{anchor}spec = \"areas\"\n"), 1),
        )
        .expect("the config");
        let git = Sandbox::new(&scratch.0);
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the whole tree");
        Self {
            _scratch: scratch,
            root,
            home,
            git,
        }
    }

    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// Core's answer over the tree's whole graph, keyed by `(path, line)`,
    /// with each document's kind; and the time of the pass alone (the
    /// median of [`RUNS`] after one warm-up).
    fn core(&self) -> (CoreAnswer, Duration) {
        let text = fs::read_to_string(self.root.join("specengine.toml")).expect("the config");
        let config = ProjectConfig::from_toml(&text).expect("the config");
        let tree = WorkingTree::new(&self.root, &config.paths).expect("the working tree");
        let input = check_input(&tree, &config.scheme);
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let (pass, _) = timed(|| {
            let _ = usage_of(&graph);
        });
        let usage = usage_of(&graph);
        let mut out = BTreeMap::new();
        for (at, node) in usage {
            let kind = graph.node(at).and_then(|node| node.kind.clone());
            out.insert(
                (graph.paths()[at.file].to_owned(), graph.line(at)),
                (node, kind, at.ord == 0),
            );
        }
        (out, pass)
    }

    /// The library's catalog and Health against core's answer.
    fn check(&self) -> (usize, usize, Duration) {
        let (core, pass) = self.core();
        let outcome = catalog_with_view(
            &self.env(),
            &Globals::default(),
            &CatalogRequest {
                kinds: vec![KIND.to_owned()],
                archive: true,
            },
            View::Browser,
            None,
        )
        .unwrap_or_else(|error| panic!("catalog: {error}"));
        assert!(outcome.rows.len() > 10, "{} rows", outcome.rows.len());
        let mut used = 0;
        for row in &outcome.rows {
            let (node, _, _) = core
                .get(&(row.path.clone(), row.line))
                .unwrap_or_else(|| panic!("{}:{}: not in the graph", row.path, row.line));
            assert_eq!(
                (row.used_by, row.mentioned_by, row.orphan),
                (node.used_by, node.mentioned_by, node.orphan),
                "{:?} at {}:{}",
                row.id,
                row.path,
                row.line
            );
            if row.used_by > 0 {
                used += 1;
            }
        }
        assert!(used > 0, "premise: some part node is used");

        let report = health(
            &self.env(),
            &Globals::default(),
            &HealthRequest {
                git: GitEnv::new(self.root.clone(), self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("health: {error}"));
        let orphans = &report.orphans;
        let live = core
            .values()
            .filter(|(node, _, document)| *document && node.orphan.is_some())
            .count();
        assert_eq!(orphans.live, live, "every live document judged");
        let mut want: Vec<(Option<String>, String)> = core
            .iter()
            .filter(|(_, (node, _, document))| *document && node.orphan == Some(true))
            .map(|((path, _), (_, kind, _))| (kind.clone(), path.clone()))
            .collect();
        want.sort_by(|a, b| (a.0.is_none(), &a.0, &a.1).cmp(&(b.0.is_none(), &b.0, &b.1)));
        let got: Vec<(Option<String>, String)> = orphans
            .rows
            .iter()
            .map(|row| (row.kind.clone(), row.path.clone()))
            .collect();
        assert_eq!(got, want, "the orphans by kind, then path");
        (outcome.rows.len(), orphans.rows.len(), pass)
    }
}

/// UO12, the default check: 1 000 nodes.
#[test]
fn uo12_usage_holds_at_1000_nodes() {
    let tree = Tree::new("check-1k", 1_000);
    let (rows, orphans, _) = tree.check();
    assert!(
        rows > 0 && orphans <= rows,
        "{rows} rows, {orphans} orphans"
    );
}

/// The release `spec` beside this test binary (`target.noindex/release`).
fn release_spec() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    exe.parent()
        .and_then(Path::parent)
        .expect("the profile directory")
        .join("spec")
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

/// [`RUNS`] timed runs of `run` after one warm-up: the median and all.
fn timed(mut run: impl FnMut()) -> (Duration, Vec<Duration>) {
    run();
    let times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect();
    (median(times.clone()), times)
}

fn millis(times: &[Duration]) -> Vec<u128> {
    times.iter().map(Duration::as_millis).collect()
}

/// What one size measured.
struct Measured {
    catalog: Duration,
    pass: Duration,
    health: Duration,
}

/// The measure at `nodes`: the check, then the timings, printed.
fn measure(nodes: usize) -> Measured {
    let started = Instant::now();
    let tree = Tree::new(&format!("measure-{nodes}"), nodes);
    let seeding = started.elapsed();
    let spec = release_spec();
    assert!(
        spec.exists(),
        "{}: build the release `spec` first",
        spec.display()
    );
    let (rows, orphans, pass) = tree.check();
    let vars: Vec<(String, std::ffi::OsString)> = tree
        .git
        .vars()
        .into_iter()
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .filter(|(name, _)| name != "HOME")
        .chain([("HOME".to_owned(), tree.home.clone().into_os_string())])
        .collect();
    let run = |args: &[&str]| {
        let output = Command::new(&spec)
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)))
            .current_dir(&tree.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .output()
            .expect("spec runs");
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let (catalog, catalog_runs) = timed(|| run(&["catalog", "--kind", KIND, "--json"]));
    let (health, health_runs) = timed(|| run(&["health", "--json"]));
    println!(
        "UO12 at {nodes} nodes: {rows} catalog rows (--archive), {orphans} orphans; seeding {:.1} \
         s; spec catalog --kind node --json (release, warm) median {} ms, runs {:?} ms; usage_of \
         pass median {} ms; spec health --json (release, warm) median {} ms, runs {:?} ms",
        seeding.as_secs_f64(),
        catalog.as_millis(),
        millis(&catalog_runs),
        pass.as_millis(),
        health.as_millis(),
        millis(&health_runs),
    );
    Measured {
        catalog,
        pass,
        health,
    }
}

#[test]
#[ignore = "UO12 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test usage_scale --run-ignored only \
            --no-capture"]
fn uo12_measure_1k() {
    measure(1_000);
}

#[test]
#[ignore = "UO12 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test usage_scale --run-ignored only \
            --no-capture"]
fn uo12_measure_10k() {
    let measured = measure(10_000);
    assert!(
        measured.catalog <= CATALOG_BAR,
        "UO12's catalog bar at 10k: {} ms > 2 s",
        measured.catalog.as_millis()
    );
    assert!(
        measured.pass <= USAGE_BAR,
        "UO12's usage pass bar at 10k: {} ms > 100 ms",
        measured.pass.as_millis()
    );
    assert!(
        measured.health <= HEALTH_BAR,
        "UO12's Health bar at 10k: {} ms > 1.5 s",
        measured.health.as_millis()
    );
}

#[test]
#[ignore = "UO12 [measure], release, reported only: cargo build --release -p specengine-cli \
            --bin spec, then cargo nextest run -p specengine-eval --release --test usage_scale \
            --run-ignored only --no-capture"]
fn uo12_measure_100k() {
    measure(100_000);
}
