//! docs/features/corpus-map.md (RM-0112, ADR-0080) "Acceptance criteria",
//! CM12's read cost [measure]: the map at scale, on `catalog_scale.rs`'s
//! generated tree (the tree of `specengine-eval scale --corpus-only --nodes
//! N --seed 1`, its areas named the spec), committed once into git.
//!
//! - `cm12_the_map_holds_at_1000_nodes` (default run): the library's map is
//!   core's over the tree's graph: every node of the live files once, in
//!   (path, position) order, each with core `usage_of`'s counts; every edge
//!   between two served nodes, never a self-edge, one per (source, target,
//!   type), sorted; a parent per document whose parent resolves; the counts
//!   line up; some node is used and some edge served (the premise).
//! - `cm12_measure_1k`, `cm12_measure_10k`, `cm12_measure_100k` (`#[ignore]`,
//!   release): the same check, then, warm, the median of five runs of the
//!   release `spec map --json`. The bar: at most 1 s at 10 000 nodes; 1 000
//!   and 100 000 are reported.
//!
//! Run the measure: `cargo build --release -p specengine-cli --bin spec`,
//! then `cargo nextest run -p specengine-eval --release --test map_scale
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
use specengine_cli::{Env, Globals, MapOutcome, MapRequest, map};
use specengine_core::ProjectConfig;
use specengine_core::check::{SpecGraph, Standing};
use specengine_core::usage::usage_of;
use specengine_store::{WorkingTree, check_input};

/// Timed runs; the median is kept.
const RUNS: usize = 5;
/// CM12's read bar at 10 000 nodes.
const MAP_BAR: Duration = Duration::from_secs(1);

/// The generated tree, committed, and its `HOME`.
struct Tree {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    git: Sandbox,
}

impl Tree {
    fn new(label: &str, nodes: usize) -> Self {
        let scratch = Scratch::new(&format!("map-scale-{label}"));
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
        // As catalog_scale.rs: the areas named the spec.
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

    /// The library's map against core's graph of the tree: the rows, edges
    /// and parents checked; `(nodes, edges)`.
    fn check(&self) -> (usize, usize) {
        let text = fs::read_to_string(self.root.join("specengine.toml")).expect("the config");
        let config = ProjectConfig::from_toml(&text).expect("the config");
        let tree = WorkingTree::new(&self.root, &config.paths).expect("the working tree");
        let input = check_input(&tree, &config.scheme);
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let usage = usage_of(&graph);
        let mut core: BTreeMap<(String, usize), (u32, u32, Option<bool>)> = BTreeMap::new();
        for (at, node) in &usage {
            if graph.standing(at.file) == Standing::Live {
                core.insert(
                    (graph.paths()[at.file].to_owned(), graph.line(*at)),
                    (node.used_by, node.mentioned_by, node.orphan),
                );
            }
        }
        let outcome: MapOutcome = map(&self.env(), &Globals::default(), &MapRequest::default())
            .unwrap_or_else(|error| panic!("map: {error}"));
        assert_eq!(outcome.rows.len(), core.len(), "every live node once");
        let places: Vec<(String, usize)> = outcome
            .rows
            .iter()
            .map(|row| (row.path.clone(), row.line))
            .collect();
        let mut sorted = places.clone();
        sorted.sort();
        assert_eq!(places, sorted, "(path, position) order");
        for row in &outcome.rows {
            assert_eq!(
                Some(&(row.used_by, row.mentioned_by, row.orphan)),
                core.get(&(row.path.clone(), row.line)),
                "{}",
                row.name
            );
            if let Some(of) = row.of {
                assert_eq!(outcome.rows[of].path, row.path, "{}", row.name);
                assert!(outcome.rows[of].of.is_none(), "{}", row.name);
            }
        }
        let mut previous = None;
        for edge in &outcome.edges {
            assert!(edge.src < outcome.rows.len() && edge.dst < outcome.rows.len());
            assert_ne!(edge.src, edge.dst, "no self-edge");
            assert!(edge.places >= 1);
            let key = (edge.src, edge.dst, edge.link_type.clone());
            if let Some(previous) = &previous {
                assert!(previous < &key, "sorted, one per triple");
            }
            previous = Some(key);
        }
        for parent in &outcome.parents {
            assert!(
                outcome.rows[parent.child].of.is_none(),
                "a document's parent"
            );
        }
        assert!(
            outcome.rows.iter().any(|row| row.used_by > 0),
            "premise: some node is used"
        );
        assert!(!outcome.edges.is_empty(), "premise: some edge is served");
        (outcome.rows.len(), outcome.edges.len())
    }
}

/// CM12, the default check: 1 000 nodes.
#[test]
fn cm12_the_map_holds_at_1000_nodes() {
    let tree = Tree::new("check-1k", 1_000);
    let (nodes, edges) = tree.check();
    // The generator's 1 000 count every node it writes; the map serves the
    // live files' (813 on seed 1), each checked against core above.
    assert!(nodes > 500 && edges > 500, "{nodes} nodes, {edges} edges");
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

/// The measure at `nodes`: the check, then the median of [`RUNS`] warm runs
/// of the release `spec map --json`, printed.
fn measure(nodes: usize) -> Duration {
    let started = Instant::now();
    let tree = Tree::new(&format!("measure-{nodes}"), nodes);
    let seeding = started.elapsed();
    let spec = release_spec();
    assert!(
        spec.exists(),
        "{}: build the release `spec` first",
        spec.display()
    );
    let (served, edges) = tree.check();
    let vars: Vec<(String, std::ffi::OsString)> = tree
        .git
        .vars()
        .into_iter()
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .filter(|(name, _)| name != "HOME")
        .chain([("HOME".to_owned(), tree.home.clone().into_os_string())])
        .collect();
    let run = || {
        let output = Command::new(&spec)
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)))
            .current_dir(&tree.root)
            .args(["map", "--json"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .output()
            .expect("spec runs");
        assert!(
            output.status.success(),
            "spec map: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run();
    let times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect();
    let read = median(times.clone());
    println!(
        "CM12 at {nodes} nodes: {served} map nodes, {edges} edges; seeding {:.1} s; spec map \
         --json (release, warm) median {} ms, runs {:?} ms",
        seeding.as_secs_f64(),
        read.as_millis(),
        times.iter().map(Duration::as_millis).collect::<Vec<_>>()
    );
    read
}

#[test]
#[ignore = "CM12 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test map_scale --run-ignored only \
            --no-capture"]
fn cm12_measure_1k() {
    measure(1_000);
}

#[test]
#[ignore = "CM12 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test map_scale --run-ignored only \
            --no-capture"]
fn cm12_measure_10k() {
    let read = measure(10_000);
    assert!(
        read <= MAP_BAR,
        "CM12's read bar at 10k: {} ms > 1 s",
        read.as_millis()
    );
}

#[test]
#[ignore = "CM12 [measure], release, reported only: cargo build --release -p specengine-cli \
            --bin spec, then cargo nextest run -p specengine-eval --release --test map_scale \
            --run-ignored only --no-capture"]
fn cm12_measure_100k() {
    measure(100_000);
}
