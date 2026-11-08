//! docs/features/flow-scope.md (RM-0083) "Acceptance criteria", the `spec`
//! binary: Flow back to ADR-0055's three link types, and `--stage S` that
//! cuts the shown nodes after the fold. Each run has its own scratch `HOME`
//! and reads a scratch copy; the fixtures are only read:
//!
//! - FS-2 (a): `fixtures/flow-gaps` plus a document holding `depends_on`
//!   alone and no `parent:` (WING-ANNEX, written into the copy only): no
//!   node, no edge, `spec graph` still lists its link, the reads' types the
//!   three, no `inner` entry of `depends_on`, `LND-HAND -> PLT-MARK` gone and
//!   PLT-MARK a node by `[[flow.definition]]`. M1, M2.
//! - FS-2 (b): `fixtures/roadmap-a`: no node, `spec roadmap` its
//!   `expected.txt`, `depends-cycle` still found for RM-0020 and RM-0021.
//! - FS-3: `--stage gather` on `fixtures/trace-almanac` crosses its groups.
//!   M3, M4.
//! - FS-4: the rows, `left_out` and `notes` stand under every stage. M5.
//! - FS-5: `-`, an unused stage, a ROOT naming nothing. M6, M7.
//! - FS-9: the fold first: each kept object is the read's own, at every
//!   depth. M14, M15.
//! - F2 and "CLI": the header, `truncated` and the omitted counts are what
//!   is kept, past the cut; S is compared byte for byte (no trim, no case
//!   fold, no NFC).
//!
//! FS-6 is MCP `mcp_flow.rs` and http `flow.rs`; FS-1 core `flow_read.rs`;
//! FS-7 eval `scale_cli.rs`; FS-8 `flow_linear.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::graph::spec30;
use common::{Run, Scratch, fixture, read_text, spec, write};
use serde_json::{Value, json};

const GAPS: &str = "flow-gaps";
const ALMANAC: &str = "trace-almanac";

/// The ten rows' codes, in the spec's table order.
const CODES: [&str; 10] = [
    "no-start",
    "two-starts",
    "unreachable",
    "no-way-to-end",
    "trap-cycle",
    "unjoined-parallel",
    "cycle-no-repeat",
    "definition-unused",
    "definition-once",
    "supply-unfed",
];

/// ADR-0055's three, the only types a flow reads (F1).
const FLOW_TYPES: [&str; 3] = ["follows", "supplies", "uses"];

/// The keys F3 holds byte-equal to the read without `--stage`.
const STANDING: [&str; 6] = ["gaps", "left_out", "notes", "root", "reason", "depth"];

/// FS-2 (a)'s document: `depends_on` alone, no `parent:`.
const ANNEX: &str = "---\nid: WING-ANNEX\nclass: canon\ntitle: The annex\nowner: owner\n\
                     reviewed: 2026-10-01\nlinks:\n  depends_on: [WING-LEND]\n---\n\n\
                     # The annex\n\nBooks wait here until the lending wing takes them.\n";

/// A scratch `HOME`, a working directory outside every project, and the runs.
struct Flow {
    scratch: Scratch,
    home: PathBuf,
    cwd: PathBuf,
}

impl Flow {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self { scratch, home, cwd }
    }

    /// `spec <args> --root <root>`.
    fn other(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--root", root.to_str().expect("UTF-8 root")]);
        spec30(&self.home, &self.cwd, &all)
    }

    /// `spec flow <args> --root <root>`.
    fn run(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = vec!["flow"];
        all.extend_from_slice(args);
        self.other(root, &all)
    }

    /// `spec flow <args> --root <root>`, which must answer exit 0, nothing
    /// on stderr.
    fn text(&self, root: &Path, args: &[&str]) -> Run {
        let run = self.run(root, args);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        run
    }

    /// `spec flow <args> --json --root <root>`, which must answer exit 0.
    fn json(&self, root: &Path, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.run(root, &all);
        run.code(0);
        run.json()
    }
}

fn first_line(run: &Run) -> &str {
    run.stdout
        .lines()
        .next()
        .unwrap_or_else(|| panic!("no output: {}", run.show()))
}

/// The `gap ...` lines of a text.
fn gap_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("gap "))
        .collect()
}

/// Every line but the header and the `gap` lines: the node and edge lines,
/// and a truncation tail.
fn body_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .skip(1)
        .filter(|line| !line.starts_with("gap "))
        .collect()
}

/// The node IDs of a document, in its order.
fn ids(document: &Value) -> Vec<&str> {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an id"))
        .collect()
}

/// The node `id` of a document.
fn node<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == id)
        .unwrap_or_else(|| panic!("no node {id}: {document}"))
}

fn edges(document: &Value) -> &Vec<Value> {
    document["edges"].as_array().expect("edges")
}

/// The edge types of a document and of its nodes' `inner` entries.
fn types_read(document: &Value) -> (BTreeSet<String>, BTreeSet<String>) {
    let edge_types = edges(document)
        .iter()
        .map(|edge| edge["type"].as_str().expect("a type").to_owned())
        .collect();
    let inner_types = document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .flat_map(|node| node["inner"].as_array().into_iter().flatten())
        .map(|inner| inner["type"].as_str().expect("a type").to_owned())
        .collect();
    (edge_types, inner_types)
}

/// The rows' codes, in order.
fn codes(document: &Value) -> Vec<&str> {
    document["gaps"]
        .as_array()
        .expect("gaps")
        .iter()
        .map(|gap| gap["code"].as_str().expect("a code"))
        .collect()
}

/// D2, F2 to F4: `staged` is `whole` cut to the nodes `kept` names, in the
/// whole read's order, and the edges between them: each kept node and edge
/// the whole read's object, its text lines the whole read's lines; the
/// standing keys equal; the header and the counts what is kept.
fn assert_cut(whole: &Value, staged: &Value, kept: &[&str], context: &str) {
    for key in STANDING {
        assert_eq!(staged[key], whole[key], "{context}: `{key}` stands (F3)");
    }
    let want_nodes: Vec<&Value> = whole["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|node| kept.contains(&node["id"].as_str().expect("an id")))
        .collect();
    let got_nodes: Vec<&Value> = staged["nodes"].as_array().expect("nodes").iter().collect();
    assert_eq!(
        got_nodes, want_nodes,
        "{context}: each kept node is the read's own object, in its order (F4)"
    );
    assert_eq!(ids(staged), kept, "{context}: the kept nodes");
    let want_edges: Vec<&Value> = edges(whole)
        .iter()
        .filter(|edge| {
            kept.contains(&edge["from"].as_str().expect("from"))
                && kept.contains(&edge["to"].as_str().expect("to"))
        })
        .collect();
    let got_edges: Vec<&Value> = edges(staged).iter().collect();
    assert_eq!(
        got_edges, want_edges,
        "{context}: the edges whose two ends are kept, each the read's own"
    );
    assert_eq!(staged["truncated"], json!(false), "{context}");
    assert_eq!(staged["omitted_nodes"], json!(0), "{context}");
    assert_eq!(staged["omitted_edges"], json!(0), "{context}");
}

/// `whole` and `staged` as text: the same `gap` lines byte for byte, the
/// header naming what is kept, every other staged line one of the whole
/// read's, in its order.
fn assert_cut_text(whole: &Run, staged: &Run, nodes: usize, edges: usize, context: &str) {
    assert_eq!(
        gap_lines(&staged.stdout),
        gap_lines(&whole.stdout),
        "{context}: the ten rows, byte for byte"
    );
    let header = first_line(whole);
    let (scope, rest) = header.split_once(" | nodes ").expect("a header");
    let depth = rest.split_once(", depth ").expect("a depth").1;
    assert_eq!(
        first_line(staged),
        format!("{scope} | nodes {nodes}, edges {edges}, depth {depth}"),
        "{context}: the header counts what is kept"
    );
    let all = body_lines(&whole.stdout);
    let mut from = 0;
    for line in body_lines(&staged.stdout) {
        let at = all[from..]
            .iter()
            .position(|whole_line| *whole_line == line)
            .unwrap_or_else(|| {
                panic!(
                    "{context}: `{line}` is no line of the read without --stage, in order:\n{}",
                    whole.stdout
                )
            });
        from += at + 1;
    }
}

// ------------------------------------------------------------------- FS-2

/// FS-2 (a): a document holding `depends_on` alone and no `parent:` is no
/// flow: no node, no edge names it, while `spec graph` still lists its
/// link; every read's edges are the three types, no `inner` entry names
/// `depends_on`, `LND-HAND -> PLT-MARK` is gone and PLT-MARK stays a node
/// by `[[flow.definition]]`. M1: `depends_on` restored to the table. M2:
/// the scope admitting a document by any `LINK_TYPES` entry.
#[test]
fn fs2_depends_on_alone_is_no_flow() {
    let flow = Flow::new("flow-scope-annex");
    let root = flow.scratch.copy(GAPS, "gaps");
    write(&root, "docs/spec/wing-annex.md", ANNEX);

    let whole = flow.json(&root, &[]);
    assert!(
        !ids(&whole).contains(&"WING-ANNEX"),
        "WING-ANNEX is no node: {whole}"
    );
    for edge in edges(&whole) {
        assert!(
            edge["from"] != "WING-ANNEX" && edge["to"] != "WING-ANNEX",
            "no edge names WING-ANNEX: {edge}"
        );
        assert!(
            !(edge["from"] == "LND-HAND" && edge["to"] == "PLT-MARK"),
            "LND-HAND -> PLT-MARK | depends_on is gone: {edge}"
        );
    }
    assert_eq!(
        node(&whole, "PLT-MARK")["kind"],
        "bookplate",
        "PLT-MARK stays, selected by [[flow.definition]]: {whole}"
    );
    let text = flow.text(&root, &[]);
    assert!(
        !text.stdout.contains("WING-ANNEX") && !text.stdout.contains(" | depends_on | "),
        "{}",
        text.show()
    );

    // Every read, whole and folded: the three types, and no `inner` entry
    // of `depends_on`.
    let reads: [&[&str]; 7] = [
        &[],
        &["--depth", "0"],
        &["--depth", "1"],
        &["--depth", "2"],
        &["WING-LIBRARY", "--depth", "1"],
        &["WING-LEND", "--depth", "0"],
        &["WING-LEND"],
    ];
    for args in reads {
        let document = flow.json(&root, args);
        let (edge_types, inner_types) = types_read(&document);
        for found in edge_types.iter().chain(&inner_types) {
            assert!(
                FLOW_TYPES.contains(&found.as_str()),
                "{args:?}: `{found}` read: {document}"
            );
        }
        assert!(
            !inner_types.contains("depends_on"),
            "{args:?}: no `inner` names depends_on: {document}"
        );
    }
    let (edge_types, _) = types_read(&whole);
    assert_eq!(
        edge_types,
        FLOW_TYPES
            .iter()
            .map(|t| (*t).to_owned())
            .collect::<BTreeSet<String>>(),
        "the read's edges hold exactly the three: {whole}"
    );

    // `spec graph` reads `depends_on` as before (`LINK_TYPES`).
    let graph = flow.other(&root, &["graph", "WING-ANNEX"]);
    graph.code(0);
    assert!(
        graph.stdout.lines().any(|line| line
            .starts_with("WING-ANNEX --depends_on--> WING-LEND | docs/spec/wing-annex.md:")),
        "{}",
        graph.show()
    );
}

/// FS-2 (b): a plan's items hold `depends_on` alone, so the unrooted flow
/// has no node; `spec roadmap` is its `expected.txt` byte for byte, and
/// `spec check` still finds the `depends-cycle` of RM-0020 and RM-0021.
/// M1, M2.
#[test]
fn fs2_a_plan_is_no_flow_and_reads_as_before() {
    let flow = Flow::new("flow-scope-plan");
    let root = flow.scratch.copy("roadmap-a", "plan");
    let document = flow.json(&root, &[]);
    assert_eq!(document["nodes"], json!([]), "{document}");
    assert_eq!(document["edges"], json!([]), "{document}");
    assert_eq!(codes(&document), CODES, "{document}");
    let run = flow.text(&root, &[]);
    assert_eq!(
        first_line(&run),
        "flow all | nodes 0, edges 0, depth -",
        "{}",
        run.show()
    );

    let roadmap = spec(&flow.home, &root, &["roadmap"]);
    roadmap.code(0);
    let want = read_text(&fixture("roadmap-a"), "expected.txt");
    assert!(
        roadmap.stdout == want,
        "spec roadmap differs\n--- got\n{}--- want\n{want}",
        roadmap.stdout
    );

    let check = spec(&flow.home, &root, &["--json", "check"]);
    assert!(
        check.code == 0 || check.code == 1,
        "a report: {}",
        check.show()
    );
    let report = check.json();
    let cycles: Vec<String> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == "depends-cycle")
        .map(Value::to_string)
        .collect();
    assert!(!cycles.is_empty(), "no depends-cycle: {report}");
    for id in ["RM-0020", "RM-0021"] {
        assert!(
            cycles.iter().any(|finding| finding.contains(id)),
            "depends-cycle names {id}: {cycles:?}"
        );
    }
}

// ------------------------------------------------------------------- FS-3

/// FS-3: `--stage gather` keeps STEP-CALL under PART-GATHER and STEP-SORT
/// under PART-PRINT, their one `follows` edge, and no group; the header
/// counts the two and the one. M3: groups kept although they have no stage
/// of their own. M4: `group` computed over the kept nodes (null).
#[test]
fn fs3_a_stage_crosses_its_groups() {
    let flow = Flow::new("flow-scope-gather");
    let root = fixture(ALMANAC);
    let document = flow.json(&root, &["--stage", "gather"]);
    assert_eq!(ids(&document), ["STEP-CALL", "STEP-SORT"], "{document}");
    let call = node(&document, "STEP-CALL");
    assert_eq!(call["group"], "PART-GATHER", "{call}");
    assert_eq!(call["distance"], json!(0), "{call}");
    assert_eq!(call["start"], json!(true), "{call}");
    let sort = node(&document, "STEP-SORT");
    assert_eq!(sort["group"], "PART-PRINT", "{sort}");
    assert_eq!(sort["distance"], json!(1), "{sort}");
    assert_eq!(
        document["edges"],
        json!([{"from": "STEP-CALL", "to": "STEP-SORT", "type": "follows", "kind": null,
            "when": null, "count": 1, "path": "docs/spec/step-sort.md", "line": 10}]),
        "{document}"
    );
    for group in ["BOOK-ALMANAC", "PART-GATHER", "PART-PRINT"] {
        assert!(!ids(&document).contains(&group), "no group {group}");
    }
    let run = flow.text(&root, &["--stage", "gather"]);
    assert_eq!(
        first_line(&run),
        "flow all | nodes 2, edges 1, depth -",
        "{}",
        run.show()
    );
    assert_eq!(
        body_lines(&run.stdout),
        [
            "0 STEP-CALL | errand | Call for entries | docs/spec/step-call.md:1 | start | \
             group PART-GATHER | stage gather",
            "1 STEP-SORT | errand | Sort the entries | docs/spec/step-sort.md:1 | \
             group PART-PRINT | stage gather",
            "STEP-CALL -> STEP-SORT | follows | x1 | docs/spec/step-sort.md:10",
        ],
        "{}",
        run.show()
    );
    let whole = flow.json(&root, &[]);
    assert_cut(&whole, &document, &["STEP-CALL", "STEP-SORT"], "gather");
}

// ------------------------------------------------------------------- FS-4

/// FS-4: for `gather`, `print`, `-` and `bind`, the text's `gap` lines and
/// the JSON's `gaps`, `left_out` and `notes` (and `root`, `reason`,
/// `depth`) are the read's without `--stage`, at every depth asked. M5:
/// rows computed over the narrowed scope: `bind`'s empty scope has no start.
/// (A `follows` edge crossing a scope is an entry or a way out, so on this
/// fixture `gather`'s, `print`'s and `-`'s narrowed rows equal the whole
/// read's; `bind` is what turns M5 red here.)
#[test]
fn fs4_the_rows_stand_under_every_stage() {
    let flow = Flow::new("flow-scope-rows");
    let root = fixture(ALMANAC);
    let scopes: [&[&str]; 3] = [&[], &["BOOK-ALMANAC"], &["--depth", "1"]];
    for scope in scopes {
        let whole_text = flow.text(&root, scope);
        let whole = flow.json(&root, scope);
        assert_eq!(codes(&whole), CODES);
        for stage in ["gather", "print", "-", "bind"] {
            let mut args = scope.to_vec();
            args.extend_from_slice(&["--stage", stage]);
            let context = format!("{args:?}");
            let text = flow.text(&root, &args);
            assert_eq!(
                gap_lines(&text.stdout),
                gap_lines(&whole_text.stdout),
                "{context}: the gap lines, byte for byte"
            );
            let document = flow.json(&root, &args);
            for key in STANDING {
                assert_eq!(document[key], whole[key], "{context}: `{key}`");
            }
        }
    }
}

// ------------------------------------------------------------------- FS-5

/// FS-5: `-` keeps the one flow step with no stage, STEP-READ, and no edge;
/// a stage nothing carries is 0 nodes, 0 edges, the ten rows, exit 0; a
/// ROOT naming nothing is exit 1 with its `reason` whatever the stage. M6:
/// `-` read as a literal stage (0 nodes). M7: `-` keeping non-steps
/// (BOOK-ALMANAC and the PARTs listed).
#[test]
fn fs5_none_and_an_unused_stage() {
    let flow = Flow::new("flow-scope-none");
    let root = fixture(ALMANAC);
    let whole = flow.json(&root, &[]);
    let whole_text = flow.text(&root, &[]);

    let unstaged = flow.json(&root, &["--stage", "-"]);
    assert_eq!(ids(&unstaged), ["STEP-READ"], "{unstaged}");
    assert_eq!(unstaged["edges"], json!([]), "{unstaged}");
    assert_cut(&whole, &unstaged, &["STEP-READ"], "-");
    let text = flow.text(&root, &["--stage", "-"]);
    assert_cut_text(&whole_text, &text, 1, 0, "-");

    let unused = flow.json(&root, &["--stage", "bind"]);
    assert_eq!(unused["nodes"], json!([]), "{unused}");
    assert_eq!(unused["edges"], json!([]), "{unused}");
    assert_eq!(codes(&unused), CODES, "the ten rows");
    assert_cut(&whole, &unused, &[], "bind");
    let text = flow.text(&root, &["--stage", "bind"]);
    assert_eq!(
        first_line(&text),
        "flow all | nodes 0, edges 0, depth -",
        "{}",
        text.show()
    );
    assert_eq!(text.stdout.lines().count(), 11, "{}", text.show());

    // A ROOT naming nothing: exit 1 whatever the stage, the same bytes.
    for json in [false, true] {
        let mut plain = vec!["NOPE-404"];
        if json {
            plain.push("--json");
        }
        let mut staged = vec!["NOPE-404", "--stage", "gather"];
        if json {
            staged.push("--json");
        }
        let before = flow.run(&root, &plain);
        let after = flow.run(&root, &staged);
        after.code(1);
        assert_eq!(
            (after.code, &after.stdout, &after.stderr),
            (before.code, &before.stdout, &before.stderr),
            "json {json}: as without --stage"
        );
        if json {
            let document = after.json();
            assert!(
                document["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("NOPE-404")),
                "{document}"
            );
        }
    }
}

// ------------------------------------------------------------------- FS-9

/// FS-9: the fold comes first and the walk is the read's: under `print`,
/// and under `gather --trace`, each kept node's and edge's object is the
/// one of the same read without `--stage` (STEP-SET keeps `distance` 3,
/// STEP-BIND 4 and `end`); at `--depth 1` with no ROOT only groups are shown,
/// so `gather` keeps nothing, while under PART-PRINT it keeps STEP-SORT.
/// M14: the walk recomputed over the stage's documents (STEP-SET reads 0).
/// M15: the cut made before the fold (`--depth 1 --stage gather` lists
/// STEP-CALL and STEP-SORT).
#[test]
fn fs9_the_fold_comes_first() {
    let flow = Flow::new("flow-scope-fold");
    let root = fixture(ALMANAC);

    let whole = flow.json(&root, &[]);
    let print = flow.json(&root, &["--stage", "print"]);
    assert_cut(&whole, &print, &["STEP-SET", "STEP-BIND"], "print");
    assert_eq!(node(&print, "STEP-SET")["distance"], json!(3), "{print}");
    assert_eq!(node(&print, "STEP-BIND")["distance"], json!(4), "{print}");
    assert_eq!(node(&print, "STEP-BIND")["end"], json!(true), "{print}");
    assert_eq!(node(&print, "STEP-SET")["group"], "PART-GATHER", "{print}");
    assert_eq!(node(&print, "STEP-BIND")["group"], "PART-PRINT", "{print}");
    assert_cut_text(
        &flow.text(&root, &[]),
        &flow.text(&root, &["--stage", "print"]),
        2,
        1,
        "print",
    );

    let traced = flow.json(&root, &["--trace"]);
    let gather = flow.json(&root, &["--stage", "gather", "--trace"]);
    assert_cut(
        &traced,
        &gather,
        &["STEP-CALL", "STEP-SORT"],
        "gather --trace",
    );
    assert_eq!(node(&gather, "STEP-CALL")["rollup"], "no card", "{gather}");
    assert_cut_text(
        &flow.text(&root, &["--trace"]),
        &flow.text(&root, &["--stage", "gather", "--trace"]),
        2,
        1,
        "gather --trace",
    );

    let top = flow.json(&root, &["--depth", "1", "--stage", "gather"]);
    assert_eq!(top["nodes"], json!([]), "only groups are shown: {top}");
    assert_eq!(top["edges"], json!([]), "{top}");
    let shown = flow.json(&root, &["--depth", "1"]);
    assert_cut(&shown, &top, &[], "--depth 1 gather");

    let args = ["PART-PRINT", "--depth", "1", "--stage", "gather"];
    let part = flow.json(&root, &args);
    let part_whole = flow.json(&root, &["PART-PRINT", "--depth", "1"]);
    assert_cut(
        &part_whole,
        &part,
        &["STEP-SORT"],
        "PART-PRINT --depth 1 gather",
    );
}

// ------------------------------------------------------------- F2, "CLI"

/// The title of a step past the cut: long, so 700 lines pass 40 000
/// characters.
const LONG: &str = "A long way round the block, and back again along the river";

/// A project of `steps` steps, each following the one before, the first
/// three `stage: few`.
fn past_the_cut(flow: &Flow, steps: usize) -> PathBuf {
    let root = flow.scratch.dir("pace");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"pace-walk\"\n\n[ids]\nPACE = { kind = \"pace\", width = 4 }\n\n\
         [[flow.step]]\nkinds = [\"pace\"]\n\n[[flow.start]]\n\
         paths = [\"docs/spec/pace-0001.md\"]\n",
    );
    for step in 1..=steps {
        let stage = if step <= 3 { "stage: few\n" } else { "" };
        let links = if step == 1 {
            String::new()
        } else {
            format!("links:\n  follows: [PACE-{:04}]\n", step - 1)
        };
        write(
            &root,
            &format!("docs/spec/pace-{step:04}.md"),
            format!(
                "---\nid: PACE-{step:04}\nclass: canon\ntitle: {LONG} {step}\nowner: owner\n\
                 reviewed: 2026-10-01\n{stage}{links}---\n\n# {LONG} {step}\n\nA step.\n"
            ),
        );
    }
    root
}

/// F2: past the cut the read without `--stage` is truncated, the stage's
/// three nodes are not: its header, `truncated` and omitted counts are what
/// is kept, each kept object the read's own.
#[test]
fn f2_the_cut_counts_what_is_kept() {
    let flow = Flow::new("flow-scope-cut");
    let root = past_the_cut(&flow, 700);
    let whole = flow.json(&root, &[]);
    assert_eq!(whole["truncated"], json!(true), "the read is cut");
    let few = flow.json(&root, &["--stage", "few"]);
    assert_eq!(few["truncated"], json!(false), "{few}");
    assert_eq!(few["omitted_nodes"], json!(0), "{few}");
    assert_eq!(few["omitted_edges"], json!(0), "{few}");
    assert_eq!(ids(&few), ["PACE-0001", "PACE-0002", "PACE-0003"], "{few}");
    assert_eq!(edges(&few).len(), 2, "{few}");
    for (at, kept) in few["nodes"].as_array().expect("nodes").iter().enumerate() {
        assert_eq!(*kept, whole["nodes"][at], "the read's own object");
    }
    for key in STANDING {
        assert_eq!(few[key], whole[key], "`{key}` stands");
    }
    let text = flow.text(&root, &["--stage", "few"]);
    assert_eq!(
        first_line(&text),
        "flow all | nodes 3, edges 2, depth -",
        "{}",
        text.show()
    );
    assert!(
        !text.stdout.contains("[truncated: "),
        "no tail: {}",
        text.show()
    );
}

/// "CLI": S is any text compared byte for byte: no trim, no case fold, no
/// NFC; a stage in another script is kept by its own bytes alone.
#[test]
fn the_stage_is_compared_byte_for_byte() {
    let flow = Flow::new("flow-scope-bytes");
    let root = fixture(ALMANAC);
    for stage in ["Gather", "GATHER", " gather", "gather ", "gathe"] {
        let document = flow.json(&root, &["--stage", stage]);
        assert_eq!(document["nodes"], json!([]), "`{stage}`: {document}");
    }

    // STEP-READ's stage written decomposed (NFD), in a copy.
    let copy = flow.scratch.copy(ALMANAC, "nfd");
    let nfd = "re\u{0301}vise";
    let nfc = "r\u{00e9}vise";
    let text = read_text(&copy, "docs/spec/step-read.md");
    write(
        &copy,
        "docs/spec/step-read.md",
        text.replacen(
            "reviewed: 2026-10-01\n",
            &format!("reviewed: 2026-10-01\nstage: {nfd}\n"),
            1,
        ),
    );
    let kept = flow.json(&copy, &["--stage", nfd]);
    assert_eq!(ids(&kept), ["STEP-READ"], "{kept}");
    assert_eq!(node(&kept, "STEP-READ")["stage"], nfd, "{kept}");
    let composed = flow.json(&copy, &["--stage", nfc]);
    assert_eq!(composed["nodes"], json!([]), "no NFC: {composed}");
    let none = flow.json(&copy, &["--stage", "-"]);
    assert_eq!(
        none["nodes"],
        json!([]),
        "STEP-READ has a stage now: {none}"
    );

    // A stage in Cyrillic, on STEP-BIND.
    let cyrillic = "\u{043f}\u{0435}\u{0447}\u{0430}\u{0442}\u{044c}";
    let text = read_text(&copy, "docs/spec/step-bind.md");
    write(
        &copy,
        "docs/spec/step-bind.md",
        text.replacen("stage: print\n", &format!("stage: {cyrillic}\n"), 1),
    );
    let bind = flow.json(&copy, &["--stage", cyrillic]);
    assert_eq!(ids(&bind), ["STEP-BIND"], "{bind}");
    assert_eq!(node(&bind, "STEP-BIND")["distance"], json!(4), "{bind}");
    let print = flow.json(&copy, &["--stage", "print"]);
    assert_eq!(ids(&print), ["STEP-SET"], "{print}");
}
