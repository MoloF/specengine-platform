//! docs/features/flow-scenario.md (RM-0094), the CLI half of the Rust lane:
//! the `spec` binary over `fixtures/flow-aside` (an invented pottery
//! workshop: 181 documents, 26 of them in the flow) and six earlier fixtures
//! of other natures, each in its own scratch `HOME`; the fixtures are only
//! read, every write goes to a scratch copy.
//!
//! - FS-1: the fixture is clean (181 documents, 0 errors, 0 warnings) and
//!   parses as its `expected.json` says. The genre scans are core
//!   `check_genre.rs`, http `no_domain.rs` and MCP `mcp_genre.rs`; the
//!   anonymity scan is eval `anonymity.rs`.
//! - FS-2: "in the flow" is what the read with no ROOT holds (F), on seven
//!   fixtures, at depth 0, 1 and unbounded under every document holding a
//!   child; the fixture's numbers. The Tier 3 ROOT's fold is core
//!   `tests/flow_read.rs`.
//! - FS-3: the rule is links, `[flow]` and `parent:` alone: a record's kind
//!   and an ID-less document are in, a `depends_on` is not.
//! - FS-4: the cut, `not_in_flow`, the kept objects byte-equal to the uncut
//!   read's, `--trace` and `--stage` beside it, the text header.
//! - FS-5: a read without the flag prints as before; the goldens are
//!   `flow_linear.rs`, `trace_views.rs` and `store_reads.rs`.
//! - FS-6: the 40x variant, built here in a scratch directory.
//! - FS-7: `types` is the model's `FLOW_LINK_TYPES` on every read, checked by
//!   every JSON read of this file.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use common::keys::{FLOW_KEYS, FLOW_SCENARIO_KEYS, without_keys};
use common::{Run, Scratch, copy_dir, fixture, read_text, snapshot, spec, write};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_model::{FLOW_LINK_TYPES, LinkOrigin, LinkTarget};
use specengine_store::{WorkingTree, check_input};

const ASIDE: &str = "flow-aside";

/// FS-2's fixtures: the pottery and six of other natures (a game, a
/// bicycle hire, a library, an almanac, a web service, a plan).
const RULE_FIXTURES: [&str; 7] = [
    ASIDE,
    "flow-gaps",
    "flow-relay",
    "flow-hire",
    "trace-almanac",
    "spec-c",
    "roadmap-a",
];

/// The fixture's documents, its in-flow documents, and its records.
const DOCUMENTS: usize = 181;
const IN_FLOW: usize = 26;
const RECORDS: usize = 142;

/// The record directories, each record named by its ID.
const RECORD_DIRS: [(&str, &str, usize); 3] = [
    ("docs/edicts", "EDICT", 32),
    ("docs/hunches", "HUNCH", 60),
    ("docs/puzzles", "PUZZLE", 50),
];

/// The parts no flow link and no `parent:` of the flow reaches.
const ASIDE_PARTS: [&str; 4] = ["WBAY-OFFICE", "WBAY-WATER", "WBAY-FUEL", "WBAY-CANTEEN"];

/// The ID-less start and end.
const OPEN_DAY: &str = "docs/workshop/gallery/open-day.md";

// ------------------------------------------------------------------ helpers

/// A scratch `HOME`, a working directory outside every project, the runs.
struct Reads {
    scratch: Scratch,
    home: PathBuf,
    cwd: PathBuf,
}

impl Reads {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self { scratch, home, cwd }
    }

    /// `spec <args> --root <root>`.
    fn run(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--root", root.to_str().expect("UTF-8 root")]);
        spec(&self.home, &self.cwd, &all)
    }

    /// `spec flow <args> --root <root>`: exit 0, nothing on stderr.
    fn text(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = vec!["flow"];
        all.extend_from_slice(args);
        let run = self.run(root, &all);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        run
    }

    /// `spec flow <args> --json --root <root>`: exit 0, one document whose
    /// `types` is the model's (FS-7, on every read of this file).
    fn flow(&self, root: &Path, args: &[&str]) -> Value {
        let mut all = vec!["flow"];
        all.extend_from_slice(args);
        all.push("--json");
        let run = self.run(root, &all);
        run.code(0);
        let document = run.json();
        assert_eq!(
            document["types"],
            json!(model_types()),
            "FS-7: `types` of {args:?}: {document}"
        );
        document
    }
}

/// `FLOW_LINK_TYPES`' names, in order.
fn model_types() -> Vec<&'static str> {
    FLOW_LINK_TYPES.iter().map(|(name, _)| *name).collect()
}

/// A node's name: its ID, else its path.
fn name(node: &Value) -> String {
    node["id"]
        .as_str()
        .or_else(|| node["path"].as_str())
        .unwrap_or_else(|| panic!("a node with neither ID nor path: {node}"))
        .to_owned()
}

fn nodes(document: &Value) -> &[Value] {
    document["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("no nodes: {document}"))
}

fn names(document: &Value) -> Vec<String> {
    nodes(document).iter().map(name).collect()
}

/// The node named `wanted`.
fn node<'a>(document: &'a Value, wanted: &str) -> &'a Value {
    nodes(document)
        .iter()
        .find(|node| name(node) == wanted)
        .unwrap_or_else(|| panic!("no node {wanted}: {document}"))
}

/// The names of the nodes whose `in_flow` is `true`; every node carries a
/// boolean.
fn in_flow_names(document: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for node in nodes(document) {
        let flag = node["in_flow"]
            .as_bool()
            .unwrap_or_else(|| panic!("`in_flow` is no boolean: {node}"));
        if flag {
            out.insert(name(node));
        }
    }
    out
}

/// F: the paths of the read with no ROOT, each node of which is in the
/// flow.
fn flow_paths(reads: &Reads, root: &Path) -> BTreeSet<String> {
    let document = reads.flow(root, &[]);
    let mut out = BTreeSet::new();
    for node in nodes(&document) {
        assert_eq!(
            node["in_flow"],
            json!(true),
            "{}: a read with no ROOT holds only the flow: {node}",
            root.display()
        );
        out.insert(node["path"].as_str().expect("a path").to_owned());
    }
    assert_eq!(
        document["not_in_flow"],
        Value::Null,
        "no flag, no count: {document}"
    );
    out
}

/// The names (ID, else path) of the live documents holding a child in
/// another file, read from the whole graph of `root`.
fn holders(root: &Path) -> Vec<String> {
    let text = read_text(root, "specengine.toml");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let tree = WorkingTree::new(root, &config.paths).expect("the working tree");
    let input = check_input(&tree, &config.scheme);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let mut out = Vec::new();
    for document in graph.documents() {
        let held = graph
            .children(document)
            .iter()
            .any(|child: &NodeAt| child.file != document.file);
        if held && !graph.is_tier3(document.file) {
            out.push(graph.name(document));
        }
    }
    out
}

/// `(name, group)` of each node of a read.
fn groups(document: &Value) -> BTreeMap<String, Option<String>> {
    nodes(document)
        .iter()
        .map(|node| (name(node), node["group"].as_str().map(str::to_owned)))
        .collect()
}

/// The names the unbounded read of a ROOT holds in the flow: a node whose
/// path is in F, and each node it is placed under by `group`.
fn in_flow_by_group(whole: &Value, flow: &BTreeSet<String>) -> BTreeSet<String> {
    let group_of = groups(whole);
    let mut out = BTreeSet::new();
    for node in nodes(whole) {
        if !flow.contains(node["path"].as_str().expect("a path")) {
            continue;
        }
        let mut at = Some(name(node));
        let mut steps = 0;
        while let Some(current) = at {
            steps += 1;
            assert!(steps <= group_of.len() + 1, "a group cycle: {whole}");
            at = group_of.get(&current).cloned().flatten();
            out.insert(current);
        }
    }
    out
}

/// A scratch copy of the fixture at `<scratch>/<dir>`.
fn aside_copy(reads: &Reads, dir: &str) -> PathBuf {
    reads.scratch.copy(ASIDE, dir)
}

/// The record files of the fixture: `(path, ID)`.
fn record_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (dir, prefix, count) in RECORD_DIRS {
        for number in 1..=count {
            let id = format!("{prefix}-{number:04}");
            out.push((format!("{dir}/{id}.md"), id));
        }
    }
    out
}

// ------------------------------------------------------------------- FS-1

/// FS-1: `spec check` on the fixture itself: 181 documents, no finding,
/// clean, in text and JSON; the fixture is only read.
#[test]
fn fs1_the_fixture_is_clean() {
    let reads = Reads::new("fs-check");
    let root = fixture(ASIDE);
    let before = snapshot(&root);
    let run = reads.run(&root, &["check"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    let summary = run
        .stdout
        .lines()
        .last()
        .unwrap_or_else(|| panic!("no summary: {}", run.show()));
    assert!(
        summary.contains(&format!(" {DOCUMENTS} documents, 0 errors, 0 warnings,")),
        "{}",
        run.show()
    );
    assert!(summary.ends_with(" \u{2014} clean"), "{summary}");
    let run = reads.run(&root, &["--json", "check"]);
    run.code(0);
    let report = run.json();
    assert_eq!(report["verdict"], "clean", "{report}");
    assert_eq!(report["findings"], json!([]), "{report}");
    let expected: Value =
        serde_json::from_str(&read_text(&root, "expected.json")).expect("expected.json");
    assert_eq!(expected["check"]["verdict"], report["verdict"]);
    assert_eq!(expected["check"]["errors"], json!(0));
    assert_eq!(expected["check"]["warnings"], json!(0));
    assert_eq!(snapshot(&root), before, "the fixture is only read");
}

/// The front-matter links of a parsed file, as `expected.json` writes them:
/// `<src> <type> frontmatter <to>[ | kind <k>][ | when <w>]`.
fn written_links(parsed: &specengine_model::ParsedFile) -> Vec<String> {
    parsed
        .links
        .iter()
        .map(|link| {
            assert_eq!(link.origin, LinkOrigin::Frontmatter, "{link:?}");
            let LinkTarget::Reference(reference) = &link.dst else {
                panic!("a path link: {link:?}");
            };
            let mut out = format!(
                "{} {} frontmatter {}",
                link.src.as_deref().unwrap_or("-"),
                link.link_type,
                reference.id
            );
            if let Some(label) = &reference.edge {
                if let Some(kind) = label.kind {
                    out.push_str(&format!(" | kind {}", kind.as_str()));
                }
                if let Some(when) = &label.when {
                    out.push_str(&format!(" | when {when}"));
                }
            }
            out
        })
        .collect()
}

/// FS-1 "as Data says": every walked file parses as `expected.json` names it
/// (ID, kind, title, `parent:`, `stage:`, its ID sections, its links), the
/// two lists name every file, the counts are the spec's, and the prefixes
/// and kinds are the config's. No title repeats an ID, and no two titles are
/// alike.
#[test]
fn fs1_the_fixture_parses_as_expected() {
    let root = fixture(ASIDE);
    let text = read_text(&root, "specengine.toml");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let expected: Value =
        serde_json::from_str(&read_text(&root, "expected.json")).expect("expected.json");
    let files = expected["files"].as_object().expect("files");
    assert_eq!(expected["documents"], json!(DOCUMENTS));
    assert_eq!(files.len(), DOCUMENTS);
    assert_eq!(expected["in_flow"], json!(IN_FLOW));
    assert_eq!(
        files
            .values()
            .filter(|file| file["in_flow"] == json!(true))
            .count(),
        IN_FLOW
    );
    let tree = WorkingTree::new(&root, &config.paths).expect("the working tree");
    let input = check_input(&tree, &config.scheme);
    let mut seen = BTreeSet::new();
    let mut titles = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for file in &input.files {
        let want = files
            .get(&file.path)
            .unwrap_or_else(|| panic!("{} is not in expected.json", file.path));
        let parsed = file
            .parsed
            .as_ref()
            .unwrap_or_else(|| panic!("{} parses", file.path));
        let document = parsed.document().expect("a document");
        assert_eq!(document.id.as_deref(), want["id"].as_str(), "{}", file.path);
        assert_eq!(
            document.kind.as_deref(),
            want["kind"].as_str(),
            "{}",
            file.path
        );
        assert_eq!(
            document.title.as_deref(),
            want["title"].as_str(),
            "{}",
            file.path
        );
        assert_eq!(
            document.parent.as_ref().map(|parent| parent.id.as_str()),
            want["parent"].as_str(),
            "{}",
            file.path
        );
        assert_eq!(
            document
                .fields
                .as_ref()
                .and_then(|fields| fields.stage.as_deref()),
            want["stage"].as_str(),
            "{}",
            file.path
        );
        let sections: Vec<String> = parsed
            .sections()
            .iter()
            .filter_map(|section| section.id.clone())
            .collect();
        let want_sections: Vec<String> = want["sections"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|section| section.as_str().expect("a string").to_owned())
            .collect();
        assert_eq!(sections, want_sections, "{}", file.path);
        let want_links: Vec<String> = want["links"]
            .as_array()
            .expect("links")
            .iter()
            .map(|link| link.as_str().expect("a string").to_owned())
            .collect();
        assert_eq!(written_links(parsed), want_links, "{}", file.path);
        titles.insert(document.title.clone().expect("a title"));
        ids.extend(document.id.clone());
        seen.insert(file.path.clone());
    }
    assert_eq!(
        seen,
        files.keys().cloned().collect::<BTreeSet<_>>(),
        "expected.json names every walked document"
    );
    assert_eq!(titles.len(), DOCUMENTS, "every title is a document's own");
    assert!(titles.is_disjoint(&ids), "no title repeats an ID");
    let mut prefixes: Vec<String> = config
        .scheme
        .prefixes()
        .iter()
        .map(|spec| spec.prefix.clone())
        .collect();
    prefixes.sort();
    assert_eq!(json!(prefixes), expected["prefixes"]);
    let mut kinds: Vec<String> = config
        .scheme
        .prefixes()
        .iter()
        .map(|spec| spec.kind.clone())
        .collect();
    kinds.sort();
    assert_eq!(json!(kinds), expected["kinds"]);
    // "155 are not (141 records, 8 notes, 4 parts and 2 desk papers)".
    let out: Vec<&String> = files
        .iter()
        .filter(|(_, file)| file["in_flow"] == json!(false))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(out.len(), DOCUMENTS - IN_FLOW);
    let records = out
        .iter()
        .filter(|path| RECORD_DIRS.iter().any(|(dir, ..)| path.starts_with(dir)))
        .count();
    assert_eq!(records, RECORDS - 1);
    assert_eq!(
        out.iter()
            .filter(|path| path.starts_with("docs/notes/"))
            .count(),
        8
    );
}

// ------------------------------------------------------------------- FS-2

/// FS-2 "The rule", on seven fixtures: every node of the read with no ROOT
/// is in the flow; under every live document holding a child, at depth 0, 1
/// and unbounded, a node is in the flow exactly when its path, or a path the
/// unbounded read places under it by `group`, is in F. M: membership taken
/// from the rooted scope; `parent:` holders left out; `[flow]` selection left
/// out.
#[test]
fn fs2_in_the_flow_is_what_the_read_with_no_root_holds() {
    let reads = Reads::new("fs-rule");
    let mut compared = 0usize;
    let mut both = 0usize;
    for fixture_name in RULE_FIXTURES {
        let root = fixture(fixture_name);
        let flow = flow_paths(&reads, &root);
        for holder in holders(&root) {
            let whole = reads.flow(&root, &[&holder]);
            let want = in_flow_by_group(&whole, &flow);
            for depth in [Some("0"), Some("1"), None] {
                let mut args = vec![holder.as_str()];
                if let Some(depth) = depth {
                    args.extend(["--depth", depth]);
                }
                let read = reads.flow(&root, &args);
                for node in nodes(&read) {
                    let wanted = want.contains(&name(node));
                    assert_eq!(
                        node["in_flow"],
                        json!(wanted),
                        "{fixture_name} {args:?}: {}: F holds it or something it \
                         holds: {wanted}",
                        name(node)
                    );
                    compared += 1;
                }
                let flags = in_flow_names(&read);
                if !flags.is_empty() && flags.len() < nodes(&read).len() {
                    both += 1;
                }
            }
        }
    }
    assert!(compared > 500, "premise: {compared} nodes compared");
    assert!(
        both > 3,
        "premise: reads holding both kinds of node: {both}"
    );
}

/// FS-2 on the fixture: F has 26 paths, `expected.json`'s; `ATL-POTTERY`
/// at depth 1 gives 24 nodes, 12 in the flow: itself, `KRUN-FIRING` and the
/// parts but four.
#[test]
fn fs2_the_fixture_holds_twenty_six_in_the_flow() {
    let reads = Reads::new("fs-numbers");
    let root = fixture(ASIDE);
    let flow = flow_paths(&reads, &root);
    let expected: Value =
        serde_json::from_str(&read_text(&root, "expected.json")).expect("expected.json");
    let marked: BTreeSet<String> = expected["files"]
        .as_object()
        .expect("files")
        .iter()
        .filter(|(_, file)| file["in_flow"] == json!(true))
        .map(|(path, _)| path.clone())
        .collect();
    assert_eq!(flow.len(), IN_FLOW, "{flow:?}");
    assert_eq!(flow, marked, "F is expected.json's");
    let read = reads.flow(&root, &["ATL-POTTERY", "--depth", "1"]);
    assert_eq!(nodes(&read).len(), 24, "{read}");
    let mut want: BTreeSet<String> = names(&read)
        .into_iter()
        .filter(|name| name.starts_with("WBAY-"))
        .filter(|name| !ASIDE_PARTS.contains(&name.as_str()))
        .collect();
    assert_eq!(want.len(), 10, "premise: ten parts in the flow");
    want.extend(["ATL-POTTERY".to_owned(), "KRUN-FIRING".to_owned()]);
    assert_eq!(in_flow_names(&read), want, "{read}");
}

// ------------------------------------------------------------------- FS-3

/// FS-3 "Domain-free": `HUNCH-0007`, of a record's kind, is in the flow by a
/// `uses`, and so is the ID-less `open-day.md` by `[flow]`; `PUZZLE-0003`,
/// whose only link is a `depends_on` on a step, is not, nor is any other
/// record or any note, in F and in the unbounded read under the top. M:
/// `depends_on` counted as a flow link; a node with no ID taken as out.
#[test]
fn fs3_a_records_kind_and_an_id_less_document_are_in_by_links_alone() {
    let reads = Reads::new("fs-domain");
    let root = fixture(ASIDE);
    let flow = flow_paths(&reads, &root);
    assert!(flow.contains("docs/hunches/HUNCH-0007.md"), "{flow:?}");
    assert!(flow.contains(OPEN_DAY), "{flow:?}");
    for (path, id) in record_files() {
        assert_eq!(
            flow.contains(&path),
            id == "HUNCH-0007",
            "{id}: only HUNCH-0007 of the records is in F"
        );
    }
    for number in 1..=8 {
        let path = format!("docs/notes/note-{number}.md");
        assert!(!flow.contains(&path), "{path} is out of F");
    }
    let whole = reads.flow(&root, &["ATL-POTTERY"]);
    assert_eq!(
        nodes(&whole).len(),
        DOCUMENTS,
        "premise: the whole tree: {whole}"
    );
    let flags = in_flow_names(&whole);
    assert!(flags.contains("HUNCH-0007"));
    assert!(flags.contains(OPEN_DAY));
    assert!(!flags.contains("PUZZLE-0003"));
    assert_eq!(flags.len(), IN_FLOW, "{flags:?}");
    for (_, id) in record_files() {
        assert_eq!(flags.contains(&id), id == "HUNCH-0007", "{id}");
    }
    for node in nodes(&whole) {
        if node["id"].is_null() && name(node) != OPEN_DAY {
            assert_eq!(node["in_flow"], json!(false), "an ID-less note: {node}");
        }
    }
    let open_day = node(&whole, OPEN_DAY);
    assert_eq!(open_day["start"], json!(true), "{open_day}");
}

// ------------------------------------------------------------------- FS-4

/// FS-4: a kept node or edge equals the uncut read's; the read's other keys
/// too.
fn assert_kept_as_uncut(context: &str, cut: &Value, uncut: &Value) {
    for key in [
        "gaps", "left_out", "notes", "root", "reason", "depth", "types",
    ] {
        assert_eq!(
            cut[key], uncut[key],
            "{context}: `{key}` is the uncut read's"
        );
    }
    for kept in nodes(cut) {
        assert_eq!(
            kept,
            node(uncut, &name(kept)),
            "{context}: a kept node is the uncut read's object"
        );
    }
    let uncut_edges = uncut["edges"].as_array().expect("edges");
    for edge in cut["edges"].as_array().expect("edges") {
        assert!(
            uncut_edges.contains(edge),
            "{context}: a kept edge is the uncut read's: {edge}"
        );
    }
    assert_eq!(cut["truncated"], json!(false), "{context}");
    assert_eq!(cut["omitted_nodes"], json!(0), "{context}");
    assert_eq!(cut["omitted_edges"], json!(0), "{context}");
}

/// FS-4 "The cut", at depth 1: ATL-POTTERY 12 nodes and 14 documents left
/// out (four parts, two desk papers under one, eight notes); WBAY-KILN its
/// four in served order and 141; WBAY-OFFICE its own node, out of the flow,
/// and 2; KRUN-FIRING 11 and 0. Without the flag `not_in_flow` is `null`.
/// Each kept node and edge is the uncut read's object. M: the cut before the
/// fold (holds change); counting nodes, not documents (14 becomes 12);
/// dropping a ROOT's node (WBAY-OFFICE 0 nodes); `not_in_flow` 0 without the
/// flag.
#[test]
fn fs4_the_cut_keeps_the_flow_and_counts_documents() {
    let reads = Reads::new("fs-cut");
    let root = fixture(ASIDE);
    for (scope, kept, left) in [
        ("ATL-POTTERY", 12, 14),
        ("WBAY-KILN", 4, 141),
        ("WBAY-OFFICE", 1, 2),
        ("KRUN-FIRING", 11, 0),
    ] {
        let uncut = reads.flow(&root, &[scope, "--depth", "1"]);
        assert_eq!(uncut["not_in_flow"], Value::Null, "{scope}: {uncut}");
        let cut = reads.flow(&root, &[scope, "--depth", "1", "--in-flow"]);
        assert_eq!(nodes(&cut).len(), kept, "{scope}: {cut}");
        assert_eq!(cut["not_in_flow"], json!(left), "{scope}: {cut}");
        assert_kept_as_uncut(scope, &cut, &uncut);
        assert!(
            names(&cut).contains(&scope.to_owned()),
            "{scope}: the ROOT stays"
        );
        for node in nodes(&cut) {
            if name(node) != scope {
                assert_eq!(node["in_flow"], json!(true), "{scope}: {node}");
            }
        }
        // Each dropped node counts itself and what it holds.
        let dropped: usize = nodes(&uncut)
            .iter()
            .filter(|node| node["in_flow"] == json!(false) && name(node) != scope)
            .map(|node| 1 + node["holds"].as_u64().expect("holds") as usize)
            .sum();
        assert_eq!(dropped, left, "{scope}: 1 + holds of each dropped node");
    }
    let kiln = reads.flow(&root, &["WBAY-KILN", "--depth", "1", "--in-flow"]);
    assert_eq!(
        names(&kiln),
        ["HUNCH-0007", "WBAY-KILN", "CONE-BISQUE", "CONE-GLOST"],
        "{kiln}"
    );
    assert_eq!(kiln["edges"], json!([]), "{kiln}");
    assert_eq!(
        node(
            &reads.flow(&root, &["ATL-POTTERY", "--depth", "1", "--in-flow"]),
            "WBAY-KILN"
        )["holds"],
        json!(144),
        "a group's holds counts what the cut drops"
    );
    let office = reads.flow(&root, &["WBAY-OFFICE", "--depth", "1", "--in-flow"]);
    assert_eq!(names(&office), ["WBAY-OFFICE"], "{office}");
    assert_eq!(node(&office, "WBAY-OFFICE")["in_flow"], json!(false));
}

/// FS-4 rules: with no ROOT the cut drops nothing (`not_in_flow` 0, the
/// nodes and edges the uncut read's), at every depth; a ROOT at depth 0
/// keeps its one node.
#[test]
fn fs4_with_no_root_the_cut_drops_nothing() {
    let reads = Reads::new("fs-no-root");
    for fixture_name in [ASIDE, "flow-gaps", "trace-almanac"] {
        let root = fixture(fixture_name);
        for depth in [None, Some("0"), Some("1")] {
            let mut args: Vec<&str> = Vec::new();
            if let Some(depth) = depth {
                args.extend(["--depth", depth]);
            }
            let uncut = reads.flow(&root, &args);
            args.push("--in-flow");
            let cut = reads.flow(&root, &args);
            assert_eq!(cut["not_in_flow"], json!(0), "{fixture_name} {args:?}");
            assert_eq!(cut["nodes"], uncut["nodes"], "{fixture_name} {args:?}");
            assert_eq!(cut["edges"], uncut["edges"], "{fixture_name} {args:?}");
        }
    }
    let root = fixture(ASIDE);
    let office = reads.flow(&root, &["WBAY-OFFICE", "--depth", "0", "--in-flow"]);
    assert_eq!(names(&office), ["WBAY-OFFICE"], "{office}");
    assert_eq!(
        office["not_in_flow"],
        json!(0),
        "the fold holds them: {office}"
    );
}

/// FS-4 "The text header ends `; not in the flow: 141`", after the rest of
/// the header; without the flag the header is as before.
#[test]
fn fs4_the_text_header_counts_what_the_cut_left_out() {
    let reads = Reads::new("fs-header");
    let root = fixture(ASIDE);
    let cut = reads.text(&root, &["WBAY-KILN", "--depth", "1", "--in-flow"]);
    assert_eq!(
        cut.stdout.lines().next(),
        Some("flow WBAY-KILN | nodes 4, edges 0, depth 1; not in the flow: 141"),
        "{}",
        cut.show()
    );
    let uncut = reads.text(&root, &["WBAY-KILN", "--depth", "1"]);
    assert_eq!(
        uncut.stdout.lines().next(),
        Some("flow WBAY-KILN | nodes 145, edges 0, depth 1"),
        "{}",
        uncut.show()
    );
    assert!(
        !uncut.stdout.contains("not in the flow"),
        "{}",
        uncut.show()
    );
    // Every other line of the cut read is a line of the uncut read.
    let lines: BTreeSet<&str> = uncut.stdout.lines().collect();
    for line in cut.stdout.lines().skip(1) {
        assert!(lines.contains(line), "{line:?} is the uncut read's");
    }
}

/// FS-4 "`KRUN-FIRING --stage glazing --in-flow` gives 2 nodes and 0": the
/// stage cuts first, the ROOT it dropped stays dropped, and the two are the
/// stage read's objects.
#[test]
fn fs4_the_stage_cuts_first() {
    let reads = Reads::new("fs-stage");
    let root = fixture(ASIDE);
    let staged = reads.flow(&root, &["KRUN-FIRING", "--stage", "glazing"]);
    let both = reads.flow(&root, &["KRUN-FIRING", "--stage", "glazing", "--in-flow"]);
    assert_eq!(names(&both), ["POT-GLAZE", "POT-FIRE"], "{both}");
    assert_eq!(both["not_in_flow"], json!(0), "{both}");
    assert_eq!(staged["not_in_flow"], Value::Null, "{staged}");
    assert_kept_as_uncut("glazing", &both, &staged);
    assert_eq!(both["edges"], staged["edges"], "{both}");
}

/// FS-4 "With `--trace`": after a card on `PUZZLE-0010` in a scratch copy,
/// every node the cut keeps, `WBAY-KILN` with the card in its counts, is
/// the uncut read's, at depth 1 under WBAY-KILN and under the top. M:
/// `counts` computed after the cut.
#[test]
fn fs4_a_card_on_a_dropped_record_still_counts() {
    let reads = Reads::new("fs-trace");
    let root = aside_copy(&reads, "copy");
    let config = read_text(&root, "specengine.toml");
    write(
        &root,
        "specengine.toml",
        format!(
            "{config}\n[[card.flow]]\nname = \"firing\"\ncolumns = [\"open\", \"doing\", \
             \"review\", \"closed\"]\naccept = {{ from = \"review\", to = \"closed\" }}\n"
        ),
    );
    reads
        .run(
            &root,
            &[
                "card",
                "new",
                "--title",
                "Check the shelf",
                "--target",
                "PUZZLE-0010",
            ],
        )
        .code(0);
    for scope in ["WBAY-KILN", "ATL-POTTERY"] {
        let uncut = reads.flow(&root, &[scope, "--depth", "1", "--trace"]);
        let cut = reads.flow(&root, &[scope, "--depth", "1", "--trace", "--in-flow"]);
        assert_kept_as_uncut(scope, &cut, &uncut);
        let kiln = node(&cut, "WBAY-KILN");
        assert_eq!(
            kiln["counts"]["in_work"],
            json!(1),
            "{scope}: the card on a dropped record counts on its holder: {kiln}"
        );
    }
}

// ------------------------------------------------------------------- FS-5

/// FS-5 (the goldens are elsewhere): a read without the flag prints no
/// `in flow` field on any line, and its JSON with the four keys removed is
/// a document of the keys before.
#[test]
fn fs5_a_read_without_the_flag_prints_as_before() {
    let reads = Reads::new("fs-default");
    let root = fixture(ASIDE);
    let run = reads.text(&root, &["ATL-POTTERY", "--depth", "1"]);
    assert!(!run.stdout.contains("in flow"), "{}", run.show());
    assert!(!run.stdout.contains("not in the flow"), "{}", run.show());
    let mut all = vec!["flow", "ATL-POTTERY", "--depth", "1", "--json"];
    all.extend(["--root", root.to_str().expect("UTF-8")]);
    let json = spec(&reads.home, &reads.cwd, &all);
    json.code(0);
    let before: Value =
        serde_json::from_str(&without_keys(&json.stdout, &FLOW_KEYS)).expect("JSON");
    assert_eq!(
        before.as_object().expect("an object").len(),
        11,
        "the read's keys before: {before}"
    );
    for node in nodes(&before) {
        assert_eq!(node.as_object().expect("an object").len(), 15, "{node}");
    }
}

/// The scan of `common::keys` takes a member with its one comma, at any
/// depth, never a key's name inside a value.
#[test]
fn the_key_scan_keeps_every_other_byte() {
    let keys = FLOW_SCENARIO_KEYS;
    assert_eq!(
        without_keys(
            "{\"a\":1,\"in_flow\":true,\"b\":[{\"types\":[\"x\"],\"c\":\"in_flow\"}]}\n",
            &keys
        ),
        "{\"a\":1,\"b\":[{\"c\":\"in_flow\"}]}\n"
    );
    assert_eq!(
        without_keys("{\"ancestors\":[{\"id\":\"A\\\"\"}],\"x\":null}", &keys),
        "{\"x\":null}"
    );
    assert_eq!(without_keys("{\"types\":[]}", &keys), "{}");
    assert_eq!(without_keys("[1,{},[]]", &keys), "[1,{},[]]");
    assert_eq!(
        without_keys("{\"x\":\"a,b\",\"not_in_flow\":null,\"y\":true}", &keys),
        "{\"x\":\"a,b\",\"y\":true}"
    );
}

// ------------------------------------------------------------------- FS-6

/// The 40x variant at `<scratch>/forty`: each record 40 times under its
/// prefix (the copies numbered after the originals), everything else as the
/// fixture has it.
fn forty_times(reads: &Reads) -> PathBuf {
    let root = reads.scratch.join("forty");
    copy_dir(&fixture(ASIDE), &root);
    let records = record_files();
    for (dir, prefix, count) in RECORD_DIRS {
        for (path, id) in records.iter().filter(|(path, _)| path.starts_with(dir)) {
            let text = read_text(&root, path);
            let title = text
                .lines()
                .find_map(|line| line.strip_prefix("title: "))
                .expect("a title")
                .to_owned();
            let number: usize = id[prefix.len() + 1..].parse().expect("a number");
            for round in 1..40 {
                let copy_id = format!("{prefix}-{:04}", number + round * count);
                let copy_title = format!("{title}, copy {round}");
                let copied = text
                    .replacen(&format!("id: {id}\n"), &format!("id: {copy_id}\n"), 1)
                    .replace(&title, &copy_title);
                write(&root, &format!("{dir}/{copy_id}.md"), copied);
            }
        }
    }
    let count = RECORD_DIRS
        .iter()
        .map(|(dir, ..)| {
            fs::read_dir(root.join(dir))
                .expect("a record directory")
                .count()
        })
        .sum::<usize>();
    assert_eq!(count, RECORDS * 40, "premise: aside40's records");
    fs::canonicalize(&root).expect("the variant")
}

/// FS-6 "At 40x": WBAY-KILN at depth 1 with the flag is its four nodes,
/// 5 679 left out, not truncated; without it the capped read is truncated
/// and prints none of the three in the flow (the records sort first). M: the
/// flag ignored.
#[test]
fn fs6_the_cut_reads_the_flow_out_of_forty_times_the_records() {
    let reads = Reads::new("fs-forty");
    let root = forty_times(&reads);
    let cut = reads.flow(&root, &["WBAY-KILN", "--depth", "1", "--in-flow"]);
    assert_eq!(
        names(&cut),
        ["HUNCH-0007", "WBAY-KILN", "CONE-BISQUE", "CONE-GLOST"],
        "{cut}"
    );
    assert_eq!(cut["not_in_flow"], json!(RECORDS * 40 - 1), "{cut}");
    assert_eq!(cut["truncated"], json!(false), "{cut}");
    let uncut = reads.flow(&root, &["WBAY-KILN", "--depth", "1"]);
    assert_eq!(uncut["truncated"], json!(true), "the capped read");
    let printed = names(&uncut);
    assert!(!printed.is_empty(), "premise: the capped read prints some");
    for id in ["CONE-BISQUE", "CONE-GLOST", "HUNCH-0007"] {
        assert!(!printed.contains(&id.to_owned()), "{id} is past the cap");
    }
}

// ------------------------------------------------------------------- FS-7

/// FS-7: `types` is `FLOW_LINK_TYPES`' names in order on every read: the
/// edgeless WBAY-KILN, a stage nothing carries, the read with no ROOT and a
/// ROOT naming nothing (exit 1). M: `types` from the read's edges (`[]`
/// here).
#[test]
fn fs7_every_read_serves_the_models_types() {
    let reads = Reads::new("fs-types");
    let root = fixture(ASIDE);
    assert_eq!(model_types(), ["follows", "uses", "supplies"]);
    let kiln = reads.flow(&root, &["WBAY-KILN", "--depth", "1"]);
    assert_eq!(kiln["edges"], json!([]), "premise: edgeless: {kiln}");
    reads.flow(&root, &["WBAY-KILN", "--depth", "1", "--in-flow"]);
    let none = reads.flow(&root, &["KRUN-FIRING", "--stage", "no-such-stage"]);
    assert_eq!(none["nodes"], json!([]), "{none}");
    reads.flow(&root, &[]);
    reads.flow(&fixture("spec-c"), &[]);
    let missing = reads.run(&root, &["flow", "WBAY-NOWHERE", "--json"]);
    missing.code(1);
    assert_eq!(
        missing.json()["types"],
        json!(model_types()),
        "{}",
        missing.show()
    );
}
