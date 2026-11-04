//! docs/features/edge-table.md "Acceptance criteria" AC-2, eval's half
//! (RM-0078 "Done"): on the tree of `specengine-eval scale --corpus-only
//! --nodes 1000 --seed 1`, the stored edges equal `SpecGraph::new` over
//! `indexed_input` and over the bytes, before and after an `update_paths`
//! of 20 seeded part-file edits; after them the worktree dumps as a fresh
//! DB's (the store's hard property). The comparison is the store's
//! own (its `tests/common/edge_rows.rs`, through `#[path]`), the edits'
//! generator its xorshift64* (`tests/common/edge_tree.rs`).
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, `HOME` and `XDG_DATA_HOME`
//! scratch directories of its own. No test asserts a time. Nothing here
//! names a pilot.

#[path = "../../specengine-store/tests/common/edge_rows.rs"]
#[allow(dead_code)]
mod edge_rows;
#[path = "../../specengine-store/tests/common/edge_tree.rs"]
#[allow(dead_code)]
mod edge_tree;
mod import_support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use edge_tree::Rng;
use import_support::{Scratch, eval, stderr};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{IndexWriter, SqliteIndex, WorkingTree};

/// The project of the generated config.
const PROJECT: &str = "synthetic";
/// The seeded edits of "Acceptance criteria" AC-2.
const EDITS: usize = 20;

/// Every part file under `areas/`, root-relative, sorted.
fn part_files(tree: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("readable directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("P-") && name.ends_with(".md"))
            {
                let relative = path.strip_prefix(root).expect("under the tree");
                out.push(relative.to_str().expect("UTF-8 path").to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, &tree.join("areas"), &mut out);
    out.sort();
    out
}

/// A part node's ID: `P-` and six digits.
fn part_id(number: usize) -> String {
    format!("P-{number:06}")
}

/// Replaces the first `from` in the file by `to`; `false` when absent.
fn replace_first(tree: &Path, path: &str, from: &str, to: &str) -> bool {
    let absolute = tree.join(path);
    let text = fs::read_to_string(&absolute).expect("a part file");
    if !text.contains(from) {
        return false;
    }
    fs::write(&absolute, text.replacen(from, to, 1)).expect("write");
    true
}

fn append(tree: &Path, path: &str, text: &str) {
    let absolute = tree.join(path);
    let mut bytes = fs::read(&absolute).expect("a part file");
    bytes.extend_from_slice(text.as_bytes());
    fs::write(&absolute, bytes).expect("write");
}

/// Drops the first ` {#P-......}` of a section heading.
fn drop_section_id(tree: &Path, path: &str) -> bool {
    let absolute = tree.join(path);
    let text = fs::read_to_string(&absolute).expect("a part file");
    let Some(start) = text.find(" {#P-") else {
        return false;
    };
    let Some(length) = text[start..].find('}') else {
        return false;
    };
    let mut edited = text[..start].to_owned();
    edited.push_str(&text[start + length + 1..]);
    fs::write(&absolute, edited).expect("write");
    true
}

/// One seeded edit of a part file, or a new one; the paths it touched.
fn edit(tree: &Path, rng: &mut Rng, parts: &mut Vec<String>, k: usize) -> Vec<String> {
    let at = rng.below(parts.len());
    let path = parts[at].clone();
    let cited = part_id(1 + rng.below(1000));
    match k % 8 {
        0 => append(tree, &path, "\nMore jo kava words.\n"),
        1 => append(
            tree,
            &path,
            &format!("\nSee {cited} and {} here.\n", part_id(900_000 + k)),
        ),
        2 => {
            if !drop_section_id(tree, &path) {
                append(tree, &path, "\nNo section here.\n");
            }
        }
        3 => {
            if !replace_first(tree, &path, "{#P-0", "{#P-9") {
                append(tree, &path, "\nNo section to rename.\n");
            }
        }
        4 => {
            fs::remove_file(tree.join(&path)).expect("delete a part file");
            parts.remove(at);
        }
        5 => {
            let added = format!("areas/a00/g0000/{}.md", part_id(990_000 + k));
            fs::write(
                tree.join(&added),
                format!(
                    "---\nclass: spec\nid: {}\nstatus: draft\nscope: [a00]\n\
                     links:\n  depends_on: [{cited}]\n---\n\n# Ka zo\n\n\
                     Ka zo P-000001 ve.\n\n## Ji vo {{#{}}}\n\nJi vo {cited} ye.\n",
                    part_id(990_000 + k),
                    part_id(991_000 + k)
                ),
            )
            .expect("add a part file");
            parts.push(added.clone());
            return vec![added];
        }
        6 => {
            let other = parts[rng.below(parts.len())].clone();
            append(
                tree,
                &path,
                &format!("\nSee [ka](/{other}#{cited}) and [zo](/{other}).\n"),
            );
        }
        _ => {
            if !replace_first(tree, &path, "\nid: P-0", "\nid: P-8") {
                append(tree, &path, "\nNo ID to move.\n");
            }
        }
    }
    vec![path]
}

/// The first lines where two dumps differ.
fn first_difference(left: &str, right: &str) -> String {
    let only_left: Vec<&str> = left
        .lines()
        .filter(|line| !right.lines().any(|other| other == *line))
        .take(4)
        .collect();
    let only_right: Vec<&str> = right
        .lines()
        .filter(|line| !left.lines().any(|other| other == *line))
        .take(4)
        .collect();
    format!("only incremental: {only_left:#?}\nonly fresh: {only_right:#?}")
}

#[test]
fn ac2_the_scale_tree_stores_cores_edges_before_and_after_twenty_edits() {
    let scratch = Scratch::new("scale-edges");
    let home = scratch.join("home");
    let xdg = scratch.join("xdg");
    let db = scratch.join("db");
    for dir in [&home, &xdg, &db] {
        fs::create_dir_all(dir).expect("scratch directory");
    }
    let out = scratch.join("out");
    let output = eval()
        .env("HOME", &home)
        .env("XDG_DATA_HOME", &xdg)
        .arg("scale")
        .arg("--out")
        .arg(&out)
        .args(["--corpus-only", "--nodes", "1000", "--seed", "1"])
        .output()
        .expect("specengine-eval runs");
    assert!(
        output.status.success(),
        "`scale --corpus-only --nodes 1000 --seed 1` exits 0; stderr:\n{}",
        stderr(&output)
    );
    let tree = fs::canonicalize(out.join("scale/synthetic/n1000/tree")).expect("the tree");
    let text = fs::read_to_string(tree.join("specengine.toml")).expect("specengine.toml");
    let scheme = IdScheme::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));
    let paths = Paths::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));

    let mut index = SqliteIndex::open(db.join("index.db"), PROJECT, &tree).expect("open");
    let walking = WorkingTree::new(&tree, &paths).expect("working tree");
    let report = index.update(&walking, &scheme).expect("update");
    assert!(report.resolved_all, "{report:?}");
    edge_rows::assert_stored_edges(&index, &tree, &scheme, &paths, "the generated tree");
    let (edges, _) = edge_rows::stored_rows(&index);
    assert!(
        edges.iter().any(|edge| edge.state == "resolved") && edges.iter().any(|e| e.holder > 0),
        "the tree's edges are stored: {} rows",
        edges.len()
    );

    let mut rng = Rng::new(1);
    let mut parts = part_files(&tree);
    assert!(parts.len() > EDITS, "{} part files", parts.len());
    let mut changed: BTreeSet<String> = BTreeSet::new();
    for k in 0..EDITS {
        changed.extend(edit(&tree, &mut rng, &mut parts, k));
    }
    let named: Vec<&str> = changed.iter().map(String::as_str).collect();
    let walking = WorkingTree::new(&tree, &paths).expect("working tree");
    let report = index
        .update_paths(&walking, &scheme, &named)
        .expect("update_paths");
    eprintln!(
        "{} stored edges; 20 edits over {} paths: {report:?}, resolved {}",
        edges.len(),
        named.len(),
        report.resolved
    );
    assert!(!report.resolved_all, "named part files: {report:?}");
    assert!(report.resolved > 0, "named part files: {report:?}");
    edge_rows::assert_stored_edges(&index, &tree, &scheme, &paths, "after 20 part-file edits");

    let mut fresh = SqliteIndex::open(db.join("fresh.db"), PROJECT, &tree).expect("open");
    fresh.update(&walking, &scheme).expect("update");
    let incremental = index.dump_worktree().expect("dump");
    let rebuilt = fresh.dump_worktree().expect("dump");
    assert!(
        incremental == rebuilt,
        "after 20 part-file edits the incremental index differs from a fresh one\n{}",
        first_difference(&incremental, &rebuilt)
    );
}
