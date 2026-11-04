//! docs/features/read-trust.md (RM-0054) "Acceptance criteria" AC-14, one
//! file's increment, on the tree of `specengine-eval scale --corpus-only
//! --nodes 100000 --seed 1`, indexed and warm:
//! - `plain_us`: `update_paths` of one edited leaf part file (its body
//!   changed, its IDs kept), edges included, best of three; the bar is
//!   200 ms (08 s3 AC-10, "per-file increment <= 200 ms");
//! - D14, with no bar: the same tree plus one file citing every document
//!   by path, as a generated index does, editing (i) the same leaf file
//!   (`fan_out_leaf_us`) and (ii) that file (`fan_out_index_us`), each the
//!   best of three.
//!
//! It prints `{plain_us, fan_out_leaf_us, fan_out_index_us}` on one stdout
//! line, then asserts the bar. A leaf is a part file none of whose IDs any
//! other file names, so its only dependent, once the citing file is
//! written, is that file. Each timed call is checked to have parsed the
//! one file and resolved its edges incrementally (`resolved` at least 1,
//! `resolved_all` false).
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, with `HOME` and `XDG_DATA_HOME`
//! scratch directories of its own. Nothing here names a pilot.

mod import_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use import_support::{Scratch, eval, stderr};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{IndexWriter as _, SqliteIndex, UpdateReport, WorkingTree};

/// The project of the generated config.
const PROJECT: &str = "synthetic";
/// The size of the tree (`--nodes`).
const NODES: usize = 100_000;
/// Timed runs of each case; the best is kept.
const RUNS: usize = 3;
/// AC-14's bar for `plain_us`.
const PLAIN_BAR: Duration = Duration::from_millis(200);
/// The file citing every document by path, root-relative.
const CITING: &str = "areas/index.md";

/// Every `.md` file of the tree, root-relative, sorted.
fn markdown_files(tree: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("readable directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let relative = path.strip_prefix(root).expect("under the tree");
                out.push(relative.to_str().expect("UTF-8 path").to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, tree, &mut out);
    out.sort();
    out
}

/// Every `P-` and six digits in `text`, as written, delimited on both
/// sides by a byte that is no ASCII letter, digit, `-` or `_`.
fn id_tokens(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_';
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find("P-") {
        let start = at + found;
        let end = start + 8;
        let digits = bytes.get(start + 2..end);
        if digits.is_some_and(|digits| digits.iter().all(u8::is_ascii_digit))
            && (start == 0 || !word(bytes[start - 1]))
            && bytes.get(end).is_none_or(|&byte| !word(byte))
        {
            out.push(&text[start..end]);
        }
        at = start + 2;
    }
    out
}

/// The IDs a part file defines: its `id:` and each `{#P-......}`.
fn defined_ids(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(id) = line.strip_prefix("id: ") {
            out.push(id.trim());
        }
        if let Some(start) = line.find("{#P-") {
            let rest = &line[start + 2..];
            if let Some(end) = rest.find('}') {
                out.push(&rest[..end]);
            }
        }
    }
    out
}

/// The first part file in path order none of whose IDs any other file
/// names: each occurs once in the tree, as its own definition.
fn first_leaf(tree: &Path, files: &[String]) -> String {
    let mut texts: Vec<(String, String)> = Vec::with_capacity(files.len());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for path in files {
        let text = fs::read_to_string(tree.join(path)).expect("a UTF-8 file");
        for token in id_tokens(&text) {
            *counts.entry(token.to_owned()).or_default() += 1;
        }
        texts.push((path.clone(), text));
    }
    texts
        .iter()
        .filter(|(path, _)| path.starts_with("areas/") && !path.ends_with("/README.md"))
        .find(|(_, text)| {
            let ids = defined_ids(text);
            !ids.is_empty() && ids.iter().all(|id| counts.get(*id) == Some(&1))
        })
        .map(|(path, _)| path.clone())
        .expect("the tree holds a leaf part file")
}

/// The file citing every other document of the tree by a relative path.
fn citing_text(files: &[String]) -> String {
    let mut text = String::from("---\nclass: generated\n---\n\n# Ka zo\n\n");
    for path in files {
        let link = match path.strip_prefix("areas/") {
            Some(under) => under.to_owned(),
            None => format!("../{path}"),
        };
        text.push_str(&format!("- [ka]({link})\n"));
    }
    text
}

/// Appends a prose line to the file, different for each `round`: the body
/// changes, the IDs stay.
fn edit_body(tree: &Path, path: &str, round: usize) {
    let absolute = tree.join(path);
    let mut text = fs::read_to_string(&absolute).expect("the edited file");
    text.push_str("\nKa jo ve");
    for _ in 0..=round {
        text.push_str(" zo");
    }
    text.push_str(".\n");
    fs::write(&absolute, text).expect("write the edited file");
}

/// `update_paths` of `path`, edited before each of [`RUNS`] calls; the
/// best time, and the reports.
fn best_of(
    index: &mut SqliteIndex,
    tree: &Path,
    paths: &Paths,
    scheme: &IdScheme,
    path: &str,
    label: &str,
) -> (Duration, Vec<UpdateReport>) {
    let mut best = Duration::MAX;
    let mut reports = Vec::with_capacity(RUNS);
    for round in 0..RUNS {
        edit_body(tree, path, round);
        let walking = WorkingTree::new(tree, paths).expect("working tree");
        let started = Instant::now();
        let report = index
            .update_paths(&walking, scheme, &[path])
            .expect("update_paths");
        let took = started.elapsed();
        assert_eq!(report.parsed, 1, "{label}, round {round}: {report:?}");
        assert!(
            report.resolved >= 1 && !report.resolved_all,
            "{label}, round {round}: the edges are resolved incrementally: {report:?}, \
             resolved {}, resolved_all {}",
            report.resolved,
            report.resolved_all
        );
        eprintln!(
            "{label} round {round}: {} us, resolved {}",
            took.as_micros(),
            report.resolved
        );
        best = best.min(took);
        reports.push(report);
    }
    (best, reports)
}

/// AC-14, manual and in release:
/// `cargo nextest run -p specengine-eval --release --test read_trust_scale
/// --run-ignored ignored-only --no-capture`.
#[test]
#[ignore = "manual, release: cargo nextest run -p specengine-eval --release --test \
            read_trust_scale --run-ignored ignored-only --no-capture"]
fn ac14_one_files_increment_on_the_100k_tree() {
    let scratch = Scratch::new("read-trust-scale");
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
        .args([
            "--corpus-only",
            "--nodes",
            &NODES.to_string(),
            "--seed",
            "1",
        ])
        .output()
        .expect("specengine-eval runs");
    assert!(
        output.status.success(),
        "`scale --corpus-only --nodes {NODES} --seed 1` exits 0; stderr:\n{}",
        stderr(&output)
    );
    let tree: PathBuf =
        fs::canonicalize(out.join(format!("scale/synthetic/n{NODES}/tree"))).expect("the tree");
    let text = fs::read_to_string(tree.join("specengine.toml")).expect("specengine.toml");
    let scheme = IdScheme::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));
    let paths = Paths::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));

    let files = markdown_files(&tree);
    assert!(
        !files.iter().any(|path| path == CITING),
        "premise: the tree has no {CITING}"
    );
    let leaf = first_leaf(&tree, &files);
    eprintln!("{} files; the leaf is {leaf}", files.len());

    let mut index = SqliteIndex::open(db.join("index.db"), PROJECT, &tree).expect("open");
    let walking = WorkingTree::new(&tree, &paths).expect("working tree");
    let cold = index.update(&walking, &scheme).expect("the cold update");
    assert!(cold.resolved_all, "the cold update: {cold:?}");
    assert_eq!(cold.walked, files.len(), "the cold update: {cold:?}");
    let walking = WorkingTree::new(&tree, &paths).expect("working tree");
    let warm = index.update(&walking, &scheme).expect("the warm update");
    assert_eq!(warm.parsed, 0, "the warm update: {warm:?}");

    let (plain, _) = best_of(&mut index, &tree, &paths, &scheme, &leaf, "plain");

    fs::write(tree.join(CITING), citing_text(&files)).expect("write the citing file");
    let walking = WorkingTree::new(&tree, &paths).expect("working tree");
    let added = index
        .update_paths(&walking, &scheme, &[CITING])
        .expect("update_paths of the citing file");
    assert_eq!(added.parsed, 1, "the citing file is added: {added:?}");

    let (fan_out_leaf, leaf_reports) =
        best_of(&mut index, &tree, &paths, &scheme, &leaf, "fan-out leaf");
    assert!(
        leaf_reports.iter().all(|report| report.resolved >= 2),
        "premise: the citing file depends on the leaf by path, so both are resolved: {:?}",
        leaf_reports
            .iter()
            .map(|report| report.resolved)
            .collect::<Vec<_>>()
    );
    let (fan_out_index, _) = best_of(&mut index, &tree, &paths, &scheme, CITING, "fan-out index");

    println!(
        "{}",
        serde_json::json!({
            "plain_us": plain.as_micros() as u64,
            "fan_out_leaf_us": fan_out_leaf.as_micros() as u64,
            "fan_out_index_us": fan_out_index.as_micros() as u64,
        })
    );
    assert!(
        plain <= PLAIN_BAR,
        "AC-14: one file's increment takes {} us, over the bar of {} us",
        plain.as_micros(),
        PLAIN_BAR.as_micros()
    );
}
