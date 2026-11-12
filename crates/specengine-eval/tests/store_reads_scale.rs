//! docs/features/store-reads.md (RM-0079), eval's half, on the tree of
//! `specengine-eval scale --corpus-only --nodes 1000 --seed 1`:
//! - SR-0 "Differential": the subset read's outcome (`specengine_cli::show`
//!   and `specengine_cli::bundle`, as MCP and the daemon call them) equals
//!   the walk's (D5, the developer's test-only entry on the production
//!   walk's code path) on the same index state, text, JSON and messages,
//!   for the size's 20 W targets, its 4 hubs and 200 seeded nodes; at 10k
//!   under `#[ignore]`.
//! - SR-3: each stored file's decoded parse, through one snapshot's
//!   `decode_all`, equals `parse(path, bytes, scheme)`, with its BLAKE3
//!   (the fixtures' half is the store's `tests/projection.rs`).
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, with `HOME` and `XDG_DATA_HOME`
//! scratch directories of its own. No test asserts a time. Nothing here
//! names a pilot.

#[path = "../../specengine-store/tests/common/edge_tree.rs"]
#[allow(dead_code)]
mod edge_tree;
mod import_support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use edge_tree::Rng;
use import_support::{Scratch, eval, stderr};
use serde_json::Value;
use specengine_cli::{
    BundleOutcome, BundleRequest, CliError, Env, GitEnv, Globals, Outcome, ShowOutcome,
    ShowRequest, render_json, render_text,
};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{IndexWriter as _, SqliteIndex, WorkingTree, b3_hash, check_input};

/// The project of the generated config.
const PROJECT: &str = "synthetic";

/// The seeded nodes of SR-0's differential, besides the targets and hubs.
const SEEDED: usize = 200;

/// The generated tree of one size and its `size.json`.
struct Generated {
    scratch: Scratch,
    tree: PathBuf,
    size: Value,
}

/// `scale --corpus-only --nodes <nodes> --seed 1` into a fresh scratch.
fn generated(nodes: usize) -> Generated {
    let scratch = Scratch::new("store-reads-scale");
    let home = scratch.join("home");
    let xdg = scratch.join("xdg");
    for dir in [&home, &xdg] {
        fs::create_dir_all(dir).expect("scratch directory");
    }
    let out = scratch.join("out");
    let output = eval()
        .env("HOME", &home)
        .env("XDG_DATA_HOME", &xdg)
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
        "`scale --corpus-only --nodes {nodes} --seed 1` exits 0; stderr:\n{}",
        stderr(&output)
    );
    let size_dir = out.join(format!("scale/synthetic/n{nodes}"));
    let tree = fs::canonicalize(size_dir.join("tree")).expect("the tree");
    let size: Value =
        serde_json::from_slice(&fs::read(size_dir.join("size.json")).expect("size.json"))
            .expect("size.json is JSON");
    Generated {
        scratch,
        tree,
        size,
    }
}

/// The tree's `[ids]` and `[paths]`.
fn config(tree: &Path) -> (IdScheme, Paths) {
    let text = fs::read_to_string(tree.join("specengine.toml")).expect("specengine.toml");
    let scheme = IdScheme::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));
    let paths = Paths::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));
    (scheme, paths)
}

/// The strings of `size[key]`.
fn strings(size: &Value, key: &str) -> Vec<String> {
    size[key]
        .as_array()
        .unwrap_or_else(|| panic!("size.json `{key}` is a list"))
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .or_else(|| item["id"].as_str().map(str::to_owned))
                .unwrap_or_else(|| panic!("size.json `{key}` item {item} names an ID"))
        })
        .collect()
}

/// [`SEEDED`] distinct node IDs of the tree, drawn by the store's
/// xorshift64* seeded 1 from every ID in path order.
fn seeded_ids(tree: &Path) -> Vec<String> {
    let (scheme, paths) = config(tree);
    let walking = WorkingTree::new(tree, &paths).expect("working tree");
    let input = check_input(&walking, &scheme);
    let mut every: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for file in &input.files {
        for node in file.parsed.iter().flat_map(|parsed| &parsed.nodes) {
            if let Some(id) = &node.id
                && seen.insert(id.clone())
            {
                every.push(id.clone());
            }
        }
    }
    assert!(every.len() > SEEDED * 2, "premise: {} IDs", every.len());
    let mut rng = Rng::new(1);
    let mut picked: BTreeSet<usize> = BTreeSet::new();
    while picked.len() < SEEDED {
        picked.insert(rng.below(every.len()));
    }
    picked.into_iter().map(|at| every[at].clone()).collect()
}

/// What every call takes, as `scale` gives it: `Globals {root: tree}`, an
/// `Env` at the tree with a scratch home, git's environment with that home.
struct Session {
    env: Env,
    globals: Globals,
    git: GitEnv,
}

impl Session {
    fn new(tree: &Path, home: &Path) -> Self {
        Self {
            env: Env {
                cwd: tree.to_path_buf(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            globals: Globals {
                root: Some(tree.to_path_buf()),
                config: None,
            },
            git: GitEnv::new(tree.to_path_buf(), [("HOME", home.as_os_str().to_owned())]),
        }
    }

    fn show_request(&self, reference: &str, links: bool, archive: bool) -> ShowRequest {
        ShowRequest {
            reference: reference.to_owned(),
            links,
            archive,
            bindings: false,
            history: false,
            git: self.git.clone(),
        }
    }

    fn bundle_request(&self, reference: &str, budget: Option<i64>) -> BundleRequest {
        BundleRequest {
            references: vec![reference.to_owned()],
            budget,
            git: self.git.clone(),
        }
    }
}

/// `show` as the walk (D5) answers it: `specengine_cli::show_walk`, the
/// `#[doc(hidden)]` test-only entry on the production walk's code path.
fn walk_show(session: &Session, request: &ShowRequest) -> Result<ShowOutcome, CliError> {
    specengine_cli::show_walk(&session.env, &session.globals, request)
}

/// `bundle` as the walk (D5) answers it: `specengine_cli::bundle_walk`.
fn walk_bundle(session: &Session, request: &BundleRequest) -> Result<BundleOutcome, CliError> {
    specengine_cli::bundle_walk(&session.env, &session.globals, request)
}

/// An outcome as the CLI prints it: the text, the JSON and every message,
/// or the error's exit and line.
fn shown(result: Result<Outcome, CliError>) -> String {
    match result {
        Ok(outcome) => {
            let messages = match &outcome {
                Outcome::Show(show) => &show.messages,
                Outcome::Bundle(bundle) => &bundle.messages,
                other => panic!("not a show or a bundle: {other:?}"),
            };
            let lines: Vec<String> = messages.iter().map(|message| message.line()).collect();
            format!(
                "{}--- json\n{}\n--- messages\n{}",
                render_text(&outcome),
                render_json(&outcome),
                lines.join("\n")
            )
        }
        Err(error) => format!("error {:?}: {}", error.exit, error.message),
    }
}

/// The first line where two answers part.
fn first_difference(left: &str, right: &str) -> String {
    for (number, (a, b)) in left.lines().zip(right.lines()).enumerate() {
        if a != b {
            return format!("line {}:\n  subset {a:?}\n  walk   {b:?}", number + 1);
        }
    }
    format!(
        "{} lines from the subset read, {} from the walk",
        left.lines().count(),
        right.lines().count()
    )
}

/// SR-0 "Differential" on the tree of one size: for each target, hub and
/// seeded node, `show`, `show --links`, `show --links --archive`, `bundle`
/// at the default budget and at 10 000, the subset read's answer equals the
/// walk's, both on the same index state (the tree is never edited).
fn differential(nodes: usize) {
    let generated = generated(nodes);
    let home = generated.scratch.join("read-home");
    fs::create_dir_all(&home).expect("read home");
    let session = Session::new(&generated.tree, &home);
    let targets = strings(&generated.size, "targets");
    let hubs = strings(&generated.size, "hubs");
    assert_eq!(targets.len(), 20, "premise: the size's W targets");
    assert_eq!(hubs.len(), 4, "premise: the size's hubs");
    let mut references: Vec<String> = Vec::new();
    for reference in targets
        .into_iter()
        .chain(hubs)
        .chain(seeded_ids(&generated.tree))
    {
        if !references.contains(&reference) {
            references.push(reference);
        }
    }
    let mut failed: Vec<String> = Vec::new();
    let mut compared = 0usize;
    for reference in &references {
        for (links, archive) in [(false, false), (true, false), (true, true)] {
            let request = session.show_request(reference, links, archive);
            let subset =
                specengine_cli::show(&session.env, &session.globals, &request).map(Outcome::Show);
            let walk = walk_show(&session, &request).map(Outcome::Show);
            let (subset, walk) = (shown(subset), shown(walk));
            compared += 1;
            if subset != walk {
                failed.push(format!(
                    "show {reference} links={links} archive={archive}: {}",
                    first_difference(&subset, &walk)
                ));
            }
        }
        for budget in [None, Some(10_000)] {
            let request = session.bundle_request(reference, budget);
            let subset = specengine_cli::bundle(&session.env, &session.globals, &request)
                .map(Outcome::Bundle);
            let walk = walk_bundle(&session, &request).map(Outcome::Bundle);
            let (subset, walk) = (shown(subset), shown(walk));
            compared += 1;
            if subset != walk {
                failed.push(format!(
                    "bundle {reference} budget={budget:?}: {}",
                    first_difference(&subset, &walk)
                ));
            }
        }
    }
    assert!(
        references.len() > SEEDED,
        "premise: {} references",
        references.len()
    );
    assert!(
        failed.is_empty(),
        "n{nodes}: {} of {compared} answers differ between the subset read and the walk:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// SR-0 "Differential" at 1k: the subset read is the walk's answer for the
/// 20 W targets, the 4 hubs and 200 seeded nodes. M: ENDING not completed
/// by `EDGE`; end files left out of the subset; C(REF) cut to one round;
/// a section on a chain not going to its enclosing node; candidate
/// `answers` edges left out; edges in fetch order.
#[test]
fn sr0_the_subset_read_answers_as_the_walk_on_the_1k_tree() {
    differential(1_000);
}

/// SR-0 "Differential" at 10k.
#[test]
#[ignore = "10k: the differential takes minutes in a debug build"]
fn sr0_the_subset_read_answers_as_the_walk_on_the_10k_tree() {
    differential(10_000);
}

/// SR-3 on the 1k tree: one snapshot's `decode_all` holds every walked file
/// in path order, each parse equal to `parse(path, bytes, scheme)` and each
/// BLAKE3 its bytes'. M: `rebuild_parsed` dropping `summary`.
#[test]
fn sr3_the_1k_tree_reads_back_as_parsed() {
    let generated = generated(1_000);
    let tree = &generated.tree;
    let (scheme, paths) = config(tree);
    let db = generated.scratch.join("db");
    fs::create_dir_all(&db).expect("db directory");
    let mut index = SqliteIndex::open(db.join("index.db"), PROJECT, tree).expect("open");
    let walking = WorkingTree::new(tree, &paths).expect("working tree");
    let report = index.update(&walking, &scheme).expect("update");
    let snapshot = index.snapshot().expect("snapshot");
    let all = snapshot.decode_all().expect("decode_all");
    assert_eq!(
        all.input.files.len(),
        report.walked,
        "decode_all holds every walked file"
    );
    let mut previous: Option<&str> = None;
    let mut differ: Vec<String> = Vec::new();
    for file in &all.input.files {
        assert!(
            previous.is_none_or(|before| before < file.path.as_str()),
            "path order: {previous:?} then {}",
            file.path
        );
        previous = Some(&file.path);
        let bytes = fs::read(tree.join(&file.path)).expect("a tree file");
        let parsed = specengine_core::parse(&file.path, &bytes, &scheme);
        if file.parsed.as_ref() != Some(&parsed) {
            differ.push(format!(
                "{}: the decoded parse differs from parse()",
                file.path
            ));
        }
        let hash = b3_hash(&bytes);
        let want = hash.strip_prefix("b3:").map(str::to_owned);
        if all.blake3.get(&file.path) != Some(&want) {
            differ.push(format!(
                "{}: the row's BLAKE3 {:?} is not the bytes' {want:?}",
                file.path,
                all.blake3.get(&file.path)
            ));
        }
    }
    assert!(
        differ.is_empty(),
        "{} of {} files:\n{}",
        differ.len(),
        all.input.files.len(),
        differ.join("\n")
    );
}
