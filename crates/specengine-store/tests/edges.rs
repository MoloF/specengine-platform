//! docs/features/edge-table.md "Acceptance criteria", the store's half:
//! core's resolved edges kept in the index, incrementally, equal to a
//! rebuild.
//!
//! - AC-1 `ac1_seeded_edits_*`: 40 seeds (four tests of ten, run side by
//!   side) of 25 steps over a generated tree of 12 to 20 files
//!   (`common::edge_tree`); after each step, through `update` or
//!   `update_paths` of the changed paths (a config edit walks), some
//!   through `Reversed`, `dump_worktree` equals a fresh DB's. The tally of
//!   each test shows every listed case; a failure names its seed and step.
//!   M: dependents found by new names only; the fixpoint cut to one
//!   round; `paths_fp` ignored; a `features` edit leaving its edges.
//! - AC-2 `ac2_*` and after every AC-1 step: `stored_edges` equals
//!   `SpecGraph::new` over `indexed_input` and over `check_input` of the
//!   bytes on every stored column, grouped by file, parents included
//!   (`common/edge_rows.rs`). M: `state` from the target of a
//!   `names_source` edge; `n` in reverse; `label` dropped; `ParentRef`
//!   without `scope`; `bare` from `written` again.
//! - AC-3 `ac3_*`: `INDEX_FORMAT` 16, the history's last line, the four
//!   tables' lines in the format corpora's dump (`format.rs` pins the hash
//!   and the rewritten stamp). M: the stamp left at 15; a table missing
//!   from the dump's `TABLES`.
//! - ET-2 `et2_*`: incremental work on a designed tree of 40 files, and
//!   the report's JSON keys. M: a full pass on every update;
//!   dependents found by new names only.
//! - `example_*`: the rows, the dump line and the two edits of the example
//!   docs/features/edge-table.md "Data" shows.
//! - ET-4 `et4_*`: `probe_paths` answers as path-by-path probing does (the
//!   listing count: `src/source.rs`).
//!
//! Every test runs on scratch copies under `std::env::temp_dir()`; the
//! fixtures are only read. Non-Latin text is written as escapes only.

#![cfg(unix)]

mod common;
#[path = "common/edge_rows.rs"]
mod edge_rows;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use common::edge_tree::{CASES, CONFIG_PATH, Change, Doc, Rng, Tree, scheduled_case};
use common::{Corpus, Reversed, Scratch, assert_equals_fresh, dump_diff};
use edge_rows::{EdgeRow, ParentRow, assert_stored_edges, stored_rows};
use specengine_store::{INDEX_FORMAT, IndexWriter, Probed, Source, SqliteIndex, UpdateReport};

/// The steps of one AC-1 seed.
const STEPS: usize = 25;

// ---------------------------------------------------------------- AC-1, AC-2

/// Makes `path` writable again when it is there (a mode-000 step).
fn unlock(root: &Path, path: &str) {
    let absolute = root.join(path);
    if fs::symlink_metadata(&absolute).is_ok() {
        common::chmod(root, path, 0o644);
    }
}

fn write_doc(root: &Path, path: &str, doc: &Doc) {
    unlock(root, path);
    common::write(root, path, doc.bytes());
    if doc.locked {
        common::chmod(root, path, 0o000);
    }
}

fn write_tree(root: &Path, tree: &Tree) {
    common::write(root, CONFIG_PATH, tree.config.text());
    for (path, doc) in &tree.files {
        write_doc(root, path, doc);
    }
}

/// Puts one step's change on disk: a rename keeps the bytes.
fn apply_change(root: &Path, tree: &Tree, change: &Change) {
    if let Some((from, to)) = &change.renamed {
        let target = root.join(to);
        fs::create_dir_all(target.parent().expect("parent")).expect("target directory");
        fs::rename(root.join(from), &target).expect("rename");
    }
    for path in &change.removed {
        unlock(root, path);
        common::remove(root, path);
    }
    for path in &change.written {
        write_doc(root, path, &tree.files[path]);
    }
    if change.config {
        common::write(root, CONFIG_PATH, tree.config.text());
    }
}

/// The worktree dump of a new DB with one `update` of the corpus as it is
/// now; the DB is deleted after.
fn fresh_dump(corpus: &Corpus, scratch: &Scratch) -> String {
    let db = scratch.db("fresh");
    let dump = {
        let mut index = corpus.open(&db);
        corpus.update(&mut index);
        index.dump_worktree().expect("dump of a fresh index")
    };
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(format!("{}{suffix}", db.display()));
    }
    dump
}

/// AC-1 and AC-2 after one step.
fn check_step(index: &SqliteIndex, corpus: &Corpus, scratch: &Scratch, context: &str) {
    let incremental = index.dump_worktree().expect("dump");
    let fresh = fresh_dump(corpus, scratch);
    assert!(
        incremental == fresh,
        "{context}: the incremental index differs from a fresh one\n{}",
        dump_diff(&incremental, &fresh)
    );
    assert_stored_edges(index, &corpus.root, &corpus.scheme, &corpus.paths, context);
}

/// How the steps of a run reached the index.
#[derive(Default)]
struct Tally {
    cases: BTreeMap<&'static str, usize>,
    updates: usize,
    update_paths: usize,
    reversed: usize,
}

fn run_seed(seed: u64, tally: &mut Tally) {
    let scratch = Scratch::new(&format!("edges-seed-{seed}"));
    let mut rng = Rng::new(seed);
    let mut tree = Tree::generate(&mut rng);
    let files = tree.files.len();
    assert!((12..=20).contains(&files), "seed {seed}: {files} files");
    let root = scratch.join("wt");
    fs::create_dir_all(&root).expect("worktree");
    write_tree(&root, &tree);
    let mut corpus = Corpus::load(&root);
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    check_step(
        &index,
        &corpus,
        &scratch,
        &format!("seed {seed}, the first index"),
    );
    for step in 1..=STEPS {
        let case = scheduled_case(seed, step, &mut rng);
        let change = tree.step(&mut rng, case);
        let files = tree.files.len();
        assert!(
            (12..=20).contains(&files),
            "seed {seed} step {step}: {files} files"
        );
        apply_change(&corpus.root, &tree, &change);
        if change.config {
            corpus.reload();
        }
        for performed in &change.cases {
            *tally.cases.entry(*performed).or_default() += 1;
        }
        let through_paths = (step + step / 2) % 2 == 1;
        let reversed = rng.chance(30);
        let context = format!(
            "seed {seed} step {step} {:?} through {}{}",
            change.cases,
            if through_paths {
                "update_paths"
            } else {
                "update"
            },
            if reversed { ", reversed" } else { "" }
        );
        let walking = corpus.tree();
        let backwards = Reversed(&walking);
        let source: &dyn Source = if reversed { &backwards } else { &walking };
        let named = change.paths();
        let named: Vec<&str> = named.iter().map(String::as_str).collect();
        let report = if through_paths {
            index.update_paths(source, &corpus.scheme, &named)
        } else {
            index.update(source, &corpus.scheme)
        }
        .unwrap_or_else(|error| panic!("{context}: {error}"));
        if through_paths {
            tally.update_paths += 1;
        } else {
            tally.updates += 1;
        }
        if reversed {
            tally.reversed += 1;
        }
        if through_paths && change.config {
            assert_eq!(
                report.walked,
                corpus.listing().paths.len(),
                "{context}: a config edit walks: {report:?}"
            );
        }
        check_step(&index, &corpus, &scratch, &context);
    }
}

fn run_seeds(seeds: std::ops::Range<u64>) {
    let mut tally = Tally::default();
    for seed in seeds.clone() {
        run_seed(seed, &mut tally);
    }
    eprintln!(
        "seeds {seeds:?}: {} update, {} update_paths, {} reversed; cases {:?}",
        tally.updates, tally.update_paths, tally.reversed, tally.cases
    );
    let missing: Vec<&str> = CASES
        .iter()
        .copied()
        .filter(|case| !tally.cases.contains_key(case))
        .collect();
    assert!(
        missing.is_empty(),
        "seeds {seeds:?}: cases never performed: {missing:?}"
    );
    assert!(tally.updates > 0 && tally.update_paths > 0 && tally.reversed > 0);
}

#[test]
fn ac1_seeded_edits_equal_a_rebuild_seeds_0_to_9() {
    run_seeds(0..10);
}

#[test]
fn ac1_seeded_edits_equal_a_rebuild_seeds_10_to_19() {
    run_seeds(10..20);
}

#[test]
fn ac1_seeded_edits_equal_a_rebuild_seeds_20_to_29() {
    run_seeds(20..30);
}

#[test]
fn ac1_seeded_edits_equal_a_rebuild_seeds_30_to_39() {
    run_seeds(30..40);
}

/// Every fixture with a root `specengine.toml`, by name.
fn configured_fixtures() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(common::repository_root().join("fixtures"))
        .expect("fixtures/ readable")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.join("specengine.toml").is_file())
        .map(|path| {
            path.file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn ac2_every_configured_fixture_stores_cores_edges() {
    let names = configured_fixtures();
    assert!(names.len() >= 18, "configured fixtures: {names:?}");
    let scratch = Scratch::new("edges-fixtures");
    let mut edges: Vec<EdgeRow> = Vec::new();
    let mut parents: Vec<ParentRow> = Vec::new();
    for name in &names {
        let corpus = Corpus::copy_of(name, &scratch, name);
        let mut index = corpus.open(&scratch.db(name));
        corpus.update(&mut index);
        assert_stored_edges(&index, &corpus.root, &corpus.scheme, &corpus.paths, name);
        let (stored, stored_parents) = stored_rows(&index);
        edges.extend(stored);
        parents.extend(stored_parents);
    }
    // The comparison is not vacuous: the fixtures store each kind of row.
    let count = |keep: &dyn Fn(&EdgeRow) -> bool| edges.iter().filter(|row| keep(row)).count();
    assert!(
        count(&|row| row.state == "resolved") > 0,
        "no resolved edge"
    );
    assert!(
        count(&|row| row.state == "dangling") > 0,
        "no dangling edge"
    );
    assert!(count(&|row| row.label.is_some()) > 0, "no labelled edge");
    assert!(count(&|row| row.origin == "inline") > 0, "no inline edge");
    assert!(
        count(&|row| row.holder > 0) > 0,
        "no edge held by a section"
    );
    assert!(
        parents.iter().any(|row| row.state == "resolved"),
        "no resolved parent"
    );
}

// ------------------------------------------------------------------- AC-3

/// The corpora of `format.rs`'s dump.
const DUMPED: [&str; 6] = [
    "spec-a",
    "spec-b",
    "roadmap-a",
    "flow-relay",
    "trace-almanac",
    "parent-ref",
];

/// The four tables of "Data", in the dump's naming.
const EDGE_TABLES: [&str; 4] = ["edges", "edge_parents", "edge_deps", "edge_names"];

/// The edge tables made `INDEX_FORMAT` 16; a later format keeps them
/// (docs/features/read-trust.md made it 17 with no edge change), and the
/// history keeps edge-table's `16` line verbatim, before the current one.
#[test]
fn ac3_the_edge_tables_made_format_16_and_the_history_keeps_it() {
    const {
        assert!(
            INDEX_FORMAT >= 16,
            "edge-table AC-3: the edge tables are format 16 or later"
        );
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/format_history.txt");
    let history = fs::read_to_string(&path).expect("tests/format_history.txt");
    let lines: Vec<&str> = history
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    let sixteen: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("16 "))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        sixteen.len(),
        1,
        "format_history.txt holds one `16 <hash>` line:\n{history}"
    );
    assert_eq!(
        lines[sixteen[0]], "16 dc7833af98bdb2c4f5e4a83f1b040235b045e2c2367f3eeba3d9140bacc43c72",
        "edge-table's line, verbatim"
    );
    let last = lines.last().expect("a history line");
    assert!(
        last.starts_with(&format!("{INDEX_FORMAT} ")),
        "format_history.txt ends with the current format: {last}"
    );
}

#[test]
fn ac3_the_format_corpora_dump_the_four_edge_tables() {
    let scratch = Scratch::new("edges-format");
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for name in DUMPED {
        let corpus = Corpus::copy_of(name, &scratch, name);
        for line in corpus.fresh_dump(&scratch).lines() {
            if let Some((table, _)) = line.split_once('\t') {
                *seen.entry(table.to_owned()).or_default() += 1;
            }
        }
    }
    for table in EDGE_TABLES {
        assert!(
            seen.get(table).is_some_and(|&lines| lines > 0),
            "the dump of {DUMPED:?} holds no `{table}\\t` line: {seen:?}"
        );
    }
}

// ---------------------------------------------------------------- Example

const EXAMPLE_CONFIG: &str = "[paths]\nroots = [\"docs\"]\nfeatures = \"docs/features\"\n\n\
[ids]\nR = { kind = \"requirement\", width = 2 }\n\
AC = { kind = \"criterion\", width = 2, scope = \"feature\" }\n";
const EXAMPLE_A: &str = "---\nid: R-01\nlinks:\n  depends_on: [R-02, R-09]\n---\n# A\n\n\
Needs R-02 and [the intro](b.md#intro).\n";
const EXAMPLE_B: &str = "---\nid: R-02\nparent: stamina/AC-01\naliases: [OLD-2]\n---\n# B\n\n\
## Intro\n";
const EXAMPLE_STAMINA: &str = "# Stamina\n\n### Delay {#AC-01}\n";

fn example_corpus(scratch: &Scratch) -> Corpus {
    let root = scratch.join("wt");
    common::write(&root, CONFIG_PATH, EXAMPLE_CONFIG);
    common::write(&root, "docs/a.md", EXAMPLE_A);
    common::write(&root, "docs/b.md", EXAMPLE_B);
    common::write(&root, "docs/features/stamina.md", EXAMPLE_STAMINA);
    Corpus::load(&root)
}

fn edge(ord: usize, link_type: &str, origin: &str, at: usize, end: Option<&str>) -> EdgeRow {
    EdgeRow {
        path: "docs/a.md".to_owned(),
        ord,
        holder: 0,
        link_type: link_type.to_owned(),
        origin: origin.to_owned(),
        names_source: false,
        state: if end.is_some() {
            "resolved"
        } else {
            "dangling"
        }
        .to_owned(),
        reason: end
            .is_none()
            .then(|| "resolves to no ID and no alias".to_owned()),
        label: None,
        at,
        ends: end.map(|path| (path.to_owned(), 0)).into_iter().collect(),
    }
}

/// `table`'s dump lines of the worktree: path to keys (the last column).
fn keys_of(dump: &str, table: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in dump.lines() {
        let Some(json) = line.strip_prefix(&format!("{table}\t")) else {
            continue;
        };
        let row: Vec<serde_json::Value> =
            serde_json::from_str(json).unwrap_or_else(|error| panic!("{line}: {error}"));
        let path = row[2].as_str().expect("the owner's path").to_owned();
        let key = row
            .last()
            .and_then(serde_json::Value::as_str)
            .expect("the key")
            .to_owned();
        out.entry(path).or_default().insert(key);
    }
    out
}

fn keyed(entries: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
    entries
        .iter()
        .map(|(path, keys)| {
            (
                (*path).to_owned(),
                keys.iter().map(|key| (*key).to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn example_the_tree_stores_the_documented_rows() {
    let scratch = Scratch::new("edges-example");
    let corpus = example_corpus(&scratch);
    let mut index = corpus.open(&scratch.db("index"));
    let report = corpus.update(&mut index);
    assert!(
        report.resolved_all,
        "a new worktree: a full pass: {report:?}"
    );
    assert_eq!(
        report.resolved, 3,
        "the full pass resolves every file: {report:?}"
    );

    let (edges, parents) = stored_rows(&index);
    assert_eq!(
        edges,
        vec![
            edge(0, "depends_on", "frontmatter", 35, Some("docs/b.md")),
            edge(1, "depends_on", "frontmatter", 41, None),
            edge(2, "mentions", "inline", 62, Some("docs/b.md")),
            edge(3, "mentions", "inline", 83, Some("docs/b.md")),
        ],
        "the example of docs/features/edge-table.md \"Data\""
    );
    assert_eq!(
        parents,
        vec![ParentRow {
            path: "docs/b.md".to_owned(),
            state: "resolved".to_owned(),
            reason: None,
            ends: vec![("docs/features/stamina.md".to_owned(), 1)],
        }]
    );

    let root = index.root().to_str().expect("UTF-8 root").to_owned();
    let dump = index
        .dump_worktree()
        .expect("dump")
        .replace(&root, "<root>");
    let line = "edges\t[\"demo\",\"<root>\",\"docs/a.md\",0,0,0,\"depends_on\",\"frontmatter\",\
                0,\"resolved\",null,null,35,\"docs/b.md\",0]";
    assert!(
        dump.lines().any(|candidate| candidate == line),
        "the dump holds {line}\n{}",
        dump.lines()
            .filter(|candidate| candidate.starts_with("edge"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        keys_of(&dump, "edge_deps"),
        keyed(&[
            ("docs/a.md", &["name:R-02", "name:R-09", "path:docs/b.md"]),
            ("docs/b.md", &["path:docs/features/stamina.md"]),
        ])
    );
    assert_eq!(
        keys_of(&dump, "edge_names"),
        keyed(&[
            ("docs/a.md", &["name:R-01", "path:docs/a.md"]),
            ("docs/b.md", &["name:OLD-2", "name:R-02", "path:docs/b.md"]),
            (
                "docs/features/stamina.md",
                &["name:AC-01", "path:docs/features/stamina.md"]
            ),
        ])
    );
    assert_stored_edges(
        &index,
        &corpus.root,
        &corpus.scheme,
        &corpus.paths,
        "the example",
    );
}

#[test]
fn example_two_edits_resolve_only_their_dependents() {
    let scratch = Scratch::new("edges-example-edits");
    let corpus = example_corpus(&scratch);
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);

    // Editing the prose of a.md: A is that file alone.
    corpus.write("docs/a.md", format!("{EXAMPLE_A}\nMore prose.\n"));
    let report = corpus.update(&mut index);
    assert_eq!(report.resolved, 1, "a.md's prose: {report:?}");
    assert!(!report.resolved_all, "{report:?}");
    assert_equals_fresh(&index, &corpus, &scratch, "a.md's prose");

    // Deleting stamina.md: N is its names, A is b.md.
    corpus.remove("docs/features/stamina.md");
    let report = corpus.update(&mut index);
    assert_eq!(
        report.resolved, 1,
        "the deleted feature document: {report:?}"
    );
    let (_, parents) = stored_rows(&index);
    assert_eq!(
        parents,
        vec![ParentRow {
            path: "docs/b.md".to_owned(),
            state: "dangling".to_owned(),
            reason: Some("resolves to no feature document `docs/features/stamina.md`".to_owned()),
            ends: Vec::new(),
        }]
    );
    assert_equals_fresh(&index, &corpus, &scratch, "the deleted feature document");
    assert_stored_edges(
        &index,
        &corpus.root,
        &corpus.scheme,
        &corpus.paths,
        "deleted",
    );
}

// ------------------------------------------- one-file lookups (review)

/// The config of the one-file scenario: `AC` feature-scoped.
const TWIN_CONFIG: &str = "[paths]\nroots = [\"docs\"]\nfeatures = \"docs/features\"\n\n\
[ids]\nR = { kind = \"requirement\", width = 2 }\n\
AC = { kind = \"criterion\", width = 2, scope = \"feature\" }\n";

/// A one-file place (`slug/ID`, and a feature-scoped bare ID in its own
/// feature document) looks the written ID up as an `aliases:` entry only
/// when no file has the ID itself as one: the Latin twin in another file
/// moves the resolution. Steps: the look-alike resolves; a new file with
/// the twin; the twin deleted. After each, `dump_worktree` equals a fresh
/// DB's and the stored edges equal core's (AC-1, AC-2). The look-alike is a
/// Cyrillic `A` (U+0410), read as Latin `A`.
#[test]
fn a_latin_twin_alias_in_another_file_moves_a_one_file_resolution() {
    let look_alike = common::edge_tree::look_alike_ac();
    for (name, citing, line) in [
        (
            "slug/ID",
            "docs/a.md",
            format!("Per stamina/{look_alike} there."),
        ),
        (
            "own feature",
            "docs/features/stamina.md",
            format!("Own {look_alike} here."),
        ),
    ] {
        let scratch = Scratch::new("edges-twin");
        let root = scratch.join("wt");
        common::write(&root, CONFIG_PATH, TWIN_CONFIG);
        let mut stamina =
            format!("---\naliases: [\"{look_alike}\"]\n---\n# Stamina\n\n### Delay {{#AC-01}}\n");
        let mut a = "---\nid: R-01\n---\n# A\n".to_owned();
        if citing == "docs/a.md" {
            a.push_str(&format!("\n{line}\n"));
        } else {
            stamina.push_str(&format!("\n{line}\n"));
        }
        common::write(&root, "docs/features/stamina.md", &stamina);
        common::write(&root, "docs/a.md", &a);
        let corpus = Corpus::load(&root);
        let mut index = corpus.open(&scratch.db("index"));
        corpus.update(&mut index);
        let (edges, _) = stored_rows(&index);
        let cited: Vec<&EdgeRow> = edges.iter().filter(|row| row.path == citing).collect();
        assert_eq!(cited.len(), 1, "{name}: {edges:#?}");
        assert_eq!(
            cited[0].state, "resolved",
            "{name}: through the written text: {cited:#?}"
        );
        check_step(
            &index,
            &corpus,
            &scratch,
            &format!("{name}: the look-alike"),
        );

        corpus.write("docs/x.md", "---\naliases: [AC-52]\n---\n# X\n");
        corpus.update(&mut index);
        check_step(
            &index,
            &corpus,
            &scratch,
            &format!("{name}: the twin added"),
        );

        corpus.remove("docs/x.md");
        corpus.update(&mut index);
        check_step(
            &index,
            &corpus,
            &scratch,
            &format!("{name}: the twin deleted"),
        );

        // The same through `update_paths` of the twin's file.
        corpus.write("docs/x.md", "---\naliases: [AC-52]\n---\n# X\n");
        index
            .update_paths(&corpus.tree(), &corpus.scheme, &["docs/x.md"])
            .expect("update_paths");
        check_step(
            &index,
            &corpus,
            &scratch,
            &format!("{name}: the twin named"),
        );
    }
}

// ------------------------------------------------------------------- ET-2

const ET2_CONFIG: &str = "[paths]\nroots = [\"docs\"]\nfeatures = \"docs/features\"\n\
link_base = \"docs\"\n\n[ids]\n\
R = { kind = \"requirement\", width = 2, aliases_from = [\"RQ\"] }\n\
AC = { kind = \"criterion\", width = 2, scope = \"feature\" }\n";
const F: &str = "docs/f.md";
const F_TEXT: &str = "---\nid: R-01\naliases: [R-91]\n---\n# F\n\nSome prose.\n\n## Intro\n\n\
More prose.\n";
/// Cites F by its ID (front-matter) and by its alias (inline).
const G1_TEXT: &str =
    "---\nid: R-02\nlinks:\n  depends_on: [R-01]\n---\n# G1\n\nAlso R-91 counts.\n";
/// Cites F by a path link with an anchor.
const G2_TEXT: &str = "---\nid: R-03\n---\n# G2\n\nSee [the intro](f.md#intro).\n";
const ET2_FILES: usize = 40;

/// The designed tree: F, G1, G2 and 37 files citing each other, never F.
fn et2_corpus(scratch: &Scratch) -> Corpus {
    let root = scratch.join("wt");
    common::write(&root, CONFIG_PATH, ET2_CONFIG);
    common::write(&root, F, F_TEXT);
    common::write(&root, "docs/g1.md", G1_TEXT);
    common::write(&root, "docs/g2.md", G2_TEXT);
    for k in 4..=ET2_FILES {
        common::write(
            &root,
            &format!("docs/o{k:02}.md"),
            format!(
                "---\nid: R-{k:02}\n---\n# O{k}\n\nAfter R-{:02}, see [next](o{:02}.md).\n",
                k - 1,
                k + 1
            ),
        );
    }
    Corpus::load(&root)
}

/// The keys of `UpdateReport`'s JSON before this slice (ET-2).
const REPORT_KEYS: [&str; 9] = [
    "missing_roots",
    "parsed",
    "removed",
    "reparsed_all",
    "skipped_names",
    "unchanged",
    "unreadable",
    "unreadable_dirs",
    "walked",
];

fn assert_report_keys(report: &UpdateReport, step: &str) {
    let value = serde_json::to_value(report).expect("the report encodes");
    let keys: BTreeSet<&str> = value
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        REPORT_KEYS.into_iter().collect::<BTreeSet<_>>(),
        "{step}: the report's JSON keys"
    );
}

#[test]
fn et2_an_update_resolves_only_what_the_change_can_move() {
    let scratch = Scratch::new("edges-et2");
    let mut corpus = et2_corpus(&scratch);
    assert_eq!(corpus.listing().paths.len(), ET2_FILES);
    let mut index = corpus.open(&scratch.db("index"));
    let mut reports = Vec::new();
    let mut step = |index: &mut SqliteIndex, corpus: &Corpus, name: &str, report: UpdateReport| {
        assert_equals_fresh(index, corpus, &scratch, name);
        assert_stored_edges(index, &corpus.root, &corpus.scheme, &corpus.paths, name);
        reports.push((name.to_owned(), report.clone()));
        report
    };

    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "the first index", report);
    assert!(report.resolved_all, "the first index: {report:?}");
    assert_eq!(report.resolved, ET2_FILES, "the first index: {report:?}");

    // No change: no edge row read or written.
    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "no change", report);
    assert_eq!(report.resolved, 0, "no change: {report:?}");
    assert!(!report.resolved_all, "no change: {report:?}");
    let report = index
        .update_paths(&corpus.tree(), &corpus.scheme, &["docs/o10.md"])
        .expect("update_paths");
    let report = step(&mut index, &corpus, "no change, named", report);
    assert_eq!(report.resolved, 0, "no change, named: {report:?}");
    assert!(!report.resolved_all, "no change, named: {report:?}");

    // F's prose: F, G1 (its ID and its alias) and G2 (its path).
    corpus.replace(F, "Some prose.", "Some other prose.");
    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "F's prose", report);
    assert_eq!(report.parsed, 1, "F's prose: {report:?}");
    assert_eq!(report.resolved, 3, "F's prose: {report:?}");
    assert!(!report.resolved_all, "F's prose: {report:?}");
    corpus.replace(F, "Some other prose.", "Some prose.");
    let report = index
        .update_paths(&corpus.tree(), &corpus.scheme, &[F])
        .expect("update_paths");
    let report = step(&mut index, &corpus, "F's prose, named", report);
    assert_eq!(report.resolved, 3, "F's prose, named: {report:?}");

    // Deleting F: its old names reach G1 and G2.
    corpus.remove(F);
    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "F deleted", report);
    assert_eq!(report.removed, 1, "F deleted: {report:?}");
    assert_eq!(report.resolved, 2, "F deleted: {report:?}");

    // F again: its new names reach the files whose misses read them.
    corpus.write(F, F_TEXT);
    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "F restored", report);
    assert_eq!(report.resolved, 3, "F restored: {report:?}");

    // `link_base` alone: every edge, nothing re-parsed.
    corpus.replace(
        CONFIG_PATH,
        "link_base = \"docs\"\n",
        "link_base = \"docs/sub\"\n",
    );
    corpus.reload();
    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "a link_base edit", report);
    assert!(report.resolved_all, "a link_base edit: {report:?}");
    assert_eq!(report.parsed, 0, "a link_base edit: {report:?}");
    assert!(!report.reparsed_all, "a link_base edit: {report:?}");
    assert_eq!(report.resolved, ET2_FILES, "a link_base edit: {report:?}");

    // An `[ids]` edit: every file re-parsed, every edge.
    let text = corpus.read_text(CONFIG_PATH);
    corpus.write(
        CONFIG_PATH,
        format!("{text}Q = {{ kind = \"question\", width = 2 }}\n"),
    );
    corpus.reload();
    let report = corpus.update(&mut index);
    let report = step(&mut index, &corpus, "an [ids] edit", report);
    assert!(report.reparsed_all, "an [ids] edit: {report:?}");
    assert!(report.resolved_all, "an [ids] edit: {report:?}");

    // A `[paths]` edit, then `update_paths` of one file: a walk.
    corpus.replace(
        CONFIG_PATH,
        "link_base = \"docs/sub\"\n",
        "link_base = \"docs\"\n",
    );
    corpus.reload();
    let report = index
        .update_paths(&corpus.tree(), &corpus.scheme, &["docs/o10.md"])
        .expect("update_paths");
    let report = step(&mut index, &corpus, "a [paths] edit, named", report);
    assert_eq!(
        report.walked, ET2_FILES,
        "a [paths] edit, named: {report:?}"
    );
    assert!(report.resolved_all, "a [paths] edit, named: {report:?}");
    assert_eq!(report.parsed, 0, "a [paths] edit, named: {report:?}");

    // A rebuild recomputes every edge.
    let report = index
        .rebuild(&corpus.tree(), &corpus.scheme)
        .expect("rebuild");
    let report = step(&mut index, &corpus, "a rebuild", report);
    assert!(report.resolved_all, "a rebuild: {report:?}");
    assert_eq!(report.resolved, ET2_FILES, "a rebuild: {report:?}");

    for (name, report) in &reports {
        assert_report_keys(report, name);
    }
}

// ------------------------------------------------------------------- ET-4

#[test]
fn et4_probe_paths_answers_as_probing_path_by_path() {
    let scratch = Scratch::new("edges-probe");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write("docs/spec/deep/inner/x.md", "# X\n");
    let tree = corpus.tree();
    let mut names: Vec<String> = corpus.listing().paths;
    assert!(names.len() > 5, "{names:?}");
    names.extend(
        [
            "docs",
            "docs/spec",
            "docs/spec/deep",
            "docs/spec/deep/inner",
            "docs/spec/",
            "./docs/spec/game.md",
            "",
            "docs/spec/nothing.md",
            "docs/nothing/nothing.md",
            "specengine.toml",
            "/absolute.md",
            "docs/spec/game.md/inside.md",
        ]
        .map(str::to_owned),
    );
    let named: Vec<&str> = names.iter().map(String::as_str).collect();
    let expected: Vec<Probed> = named
        .iter()
        .map(|path| {
            if tree.is_dir(path) {
                Probed::Dir
            } else if tree.probe(path) {
                Probed::Listed
            } else {
                Probed::Unlisted
            }
        })
        .collect();
    assert_eq!(tree.probe_paths(&named), expected);
    assert!(expected.contains(&Probed::Dir), "{expected:?}");
    assert!(expected.contains(&Probed::Listed), "{expected:?}");
    assert!(expected.contains(&Probed::Unlisted), "{expected:?}");
    // The default, path by path, answers the same through a wrapper.
    let backwards = Reversed(&tree);
    let listed: Vec<&str> = named
        .iter()
        .copied()
        .filter(|path| !tree.is_dir(path))
        .collect();
    let through: Vec<Probed> = backwards.probe_paths(&listed);
    let direct: Vec<Probed> = tree.probe_paths(&listed);
    assert_eq!(through, direct, "the trait's default equals the override");
}
