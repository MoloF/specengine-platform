//! docs/features/flow-read.md (RM-0043, ADR-0047, ADR-0055), the `spec`
//! binary over `fixtures/flow-relay` (an invented game), `fixtures/flow-hire`
//! (a hired bicycle), `fixtures/flow-gaps` (a public library's lending, each
//! gap planted once) and `fixtures/spec-c` (no flow link), each in its own
//! scratch `HOME`; the fixtures are only read:
//!
//! - AC-01: the walk's order, the gap rows of a project with no `[flow]`, the
//!   same bytes from copies written in opposite file orders. M1 turns
//!   `ac01_the_relay_walks_from_its_start` red; the copies stay alike under
//!   it, `SpecGraph` having sorted the files by path before any walk.
//! - AC-02: the four types, `from` and `to` the reading direction. M2.
//! - AC-03: the planted gaps, the rows of `expected.json`.
//! - AC-04: loops are no gaps, on both projects. M3, M4.
//! - AC-05: lifted edges; a count conserved at every depth. M5.
//! - AC-06: the same binary on four projects (the scans are `genre.rs`,
//!   core `check_genre.rs`, http `no_domain.rs`, MCP `mcp_genre.rs`). M6.
//! - AC-07: the cap (`mcp_flow.rs` and http `flow.rs` the other surfaces).
//!   M7.
//! - AC-08: `[flow]` and its rows, the commands it never touches. M8.
//! - "Exits", "Scope", "Scope edges", the item cap, the rules and edge data.
//! - The readings the spec leaves to its implementer, settled for review: a
//!   self-edge while shown and folded, an unmeasured row's `-` and `null`, a
//!   bad `[[flow.step]]`, a derived start, a folded node's `distance`, a
//!   depth of documents, the rows reading every live document, an unresolved
//!   `follows`. M9, M10.
//! - docs/features/flow-canvas.md AC-14, `FlowNode.gaps`: the distinct
//!   documents the rows name among what a node holds, on the same corpora.
//!   Its M8 (counting per row) turns `ac14_` red.
//! - docs/features/trace-views.md AC-02 reverses `start` and `end` staying a
//!   folded node's own: the two pins here flipped, the rest of the fold is
//!   `trace_views.rs`.
//! - docs/features/flow-authoring.md D11: a row above 0 carries its line, the
//!   rows printed here with it; `flow_authoring.rs` holds the table.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use common::graph::{keys, spec30};
use common::{
    Run, Scratch, data_dir, fixture, index, md_files, read, read_text, replace, snapshot,
    spec_with, write,
};
use serde_json::{Value, json};

const RELAY: &str = "flow-relay";
const HIRE: &str = "flow-hire";
const GAPS: &str = "flow-gaps";

/// The ten rows, in the spec's table order.
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

/// The rows a project with no `[flow]` cannot measure: `Needs` of the table.
const NEEDING_A_TABLE: [(&str, &str); 7] = [
    ("no-start", "start"),
    ("two-starts", "start"),
    ("unreachable", "start"),
    ("no-way-to-end", "end"),
    ("trap-cycle", "end"),
    ("definition-unused", "definition"),
    ("definition-once", "definition"),
];

/// The rows needing no table.
const NEEDING_NONE: [&str; 3] = ["unjoined-parallel", "cycle-no-repeat", "supply-unfed"];

/// "Data": the cause of a row needing an absent `[[flow.start]]`, verbatim
/// (docs/features/flow-authoring.md "`cause`": the prefix stays, its second
/// half says how).
const NO_START_CAUSE: &str = "no [[flow.start]] rule: add one naming the first step by paths";

/// docs/features/flow-authoring.md "`cause`": the line of a row above 0, by
/// the row. The whole table: `flow_authoring.rs`.
const TWO_STARTS: &str = "more than one start: narrow [[flow.start]] to one";
const UNREACHABLE: &str =
    "no path from a start reaches it: add a follows link on it, naming a step before it";
const NO_WAY_TO_END: &str =
    "no path from it reaches an end: have a later step follow it, or match it in [[flow.end]]";
const TRAP_CYCLE: &str = "a loop with no way out: have a step outside it follow a member, or match a member in \
     [[flow.end]]";
const UNJOINED_PARALLEL: &str = "a parallel fork with one branch, or branches that never meet: \
                                 add a branch, or a later step following them all";
const CYCLE_NO_REPEAT: &str =
    "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}";
const DEFINITION_UNUSED: &str =
    "no document uses it: add a uses link to it, or drop it from [[flow.definition]]";
const DEFINITION_ONCE: &str = "only one document uses it: add a uses link from a second document, or fold it into that \
     document";
const SUPPLY_UNFED: &str =
    "these only supply each other: add a supplies link into them from a node outside";
const NO_START: &str = "no step here is a start: match the flow's first step in [[flow.start]], \
                        or, reading a part, add a follows link into it from outside";

/// A `[flow]` over flow-relay by `paths` alone: its kinds stay its own. Not
/// `kit-oil.md`: no document `uses` it, a true `definition-unused`.
const RELAY_FLOW: &str = "\n[[flow.start]]\npaths = [\"docs/spec/leg-start.md\"]\n\n\
                          [[flow.end]]\npaths = [\"docs/spec/leg-end.md\"]\n\n\
                          [[flow.definition]]\npaths = [\"docs/spec/kit-torch.md\"]\n";

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

    /// `spec flow <args> --root <root>`.
    fn run(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = vec!["flow"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--root", root.to_str().expect("UTF-8 root")]);
        spec30(&self.home, &self.cwd, &all)
    }

    /// `spec flow <args> --root <root>`, which must answer exit 0, nothing
    /// on stderr.
    fn text(&self, root: &Path, args: &[&str]) -> Run {
        let run = self.run(root, args);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        run
    }

    /// `spec flow <args> --root <root> --json`, which must answer exit 0.
    fn json(&self, root: &Path, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.run(root, &all);
        run.code(0);
        run.json()
    }

    /// A copy of `fixtures/<name>` with `extra` appended to its config.
    fn copy(&self, name: &str, dir: &str, extra: &str) -> PathBuf {
        let root = self.scratch.copy(name, dir);
        let config = read_text(&root, "specengine.toml");
        write(&root, "specengine.toml", format!("{config}{extra}"));
        root
    }

    /// `spec <args> --root <root>`.
    fn other(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--root", root.to_str().expect("UTF-8 root")]);
        spec30(&self.home, &self.cwd, &all)
    }
}

/// `(code, stdout, stderr)`, for a byte comparison.
fn bytes_of(run: &Run) -> (i32, String, String) {
    (run.code, run.stdout.clone(), run.stderr.clone())
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

/// The field of a line up to its first ` | `.
fn head(line: &str) -> &str {
    line.split(" | ").next().expect("a field")
}

/// The node lines: `<distance or -> <name> | ...`.
fn node_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| {
            let first = head(line);
            let Some((distance, name)) = first.split_once(' ') else {
                return false;
            };
            !name.contains(' ') && (distance == "-" || distance.parse::<usize>().is_ok())
        })
        .collect()
}

/// The edge lines: `<from> -> <to> | ...`.
fn edge_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| head(line).contains(" -> "))
        .collect()
}

/// `(distance, name)` of each node line.
fn walk(stdout: &str) -> Vec<(String, String)> {
    node_lines(stdout)
        .into_iter()
        .map(|line| {
            let (distance, name) = head(line).split_once(' ').expect("two fields");
            (distance.to_owned(), name.to_owned())
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(one, two)| ((*one).to_owned(), (*two).to_owned()))
        .collect()
}

/// `fixtures/flow-gaps/expected.json`.
fn expected() -> Value {
    serde_json::from_str(&read_text(&fixture(GAPS), "expected.json")).expect("expected.json")
}

/// The row of `code` in a document.
fn row<'a>(document: &'a Value, code: &str) -> &'a Value {
    document["gaps"]
        .as_array()
        .expect("gaps")
        .iter()
        .find(|gap| gap["code"] == code)
        .unwrap_or_else(|| panic!("no row {code}: {document}"))
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

/// `document`'s rows are `expected.json`'s rows of `scope`, in order: the
/// state always, the count and the items of a measured row, which has a
/// cause above 0 and none at 0 (docs/features/flow-authoring.md D11); any
/// other row has one.
fn assert_rows(document: &Value, scope: &str) {
    let want = expected()["gaps"][scope].clone();
    let want = want.as_array().expect("the scope's rows");
    assert_eq!(codes(document), CODES, "{scope}: the rows' order");
    for (gap, want) in document["gaps"].as_array().expect("gaps").iter().zip(want) {
        assert_eq!(gap["code"], want["code"], "{scope}");
        assert_eq!(gap["state"], want["state"], "{scope}: {gap}");
        if want["state"] == "measured" {
            assert_eq!(gap["count"], want["count"], "{scope}: {gap}");
            assert_eq!(gap["items"], want["items"], "{scope}: {gap}");
            assert_eq!(
                gap["cause"].is_string(),
                want["count"] != json!(0),
                "{scope}: {gap}"
            );
            assert_eq!(gap["cause"].is_null(), want["count"] == json!(0));
        } else {
            assert!(gap["cause"].is_string(), "{scope}: {gap}");
        }
    }
}

/// A document's edges by `(from, to, type)`, each edge its own entry.
fn edge<'a>(document: &'a Value, from: &str, to: &str, kind: &str) -> &'a Value {
    let found: Vec<&Value> = document["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .filter(|edge| edge["from"] == from && edge["to"] == to && edge["type"] == kind)
        .collect();
    assert_eq!(found.len(), 1, "{from} -> {to} {kind}: {document}");
    found[0]
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

/// Per type: the edges' `count` and the nodes' `inner` together.
fn counted(document: &Value) -> BTreeMap<String, u64> {
    let mut totals = BTreeMap::new();
    for edge in document["edges"].as_array().expect("edges") {
        let kind = edge["type"].as_str().expect("a type").to_owned();
        *totals.entry(kind).or_insert(0) += edge["count"].as_u64().expect("a count");
    }
    for node in document["nodes"].as_array().expect("nodes") {
        for inner in node["inner"].as_array().into_iter().flatten() {
            let kind = inner["type"].as_str().expect("a type").to_owned();
            *totals.entry(kind).or_insert(0) += inner["count"].as_u64().expect("a count");
        }
    }
    totals
}

/// A node's `inner` by type.
fn inner(node: &Value) -> BTreeMap<String, u64> {
    node["inner"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            (
                item["type"].as_str().expect("a type").to_owned(),
                item["count"].as_u64().expect("a count"),
            )
        })
        .collect()
}

fn typed(items: &[(&str, u64)]) -> BTreeMap<String, u64> {
    items
        .iter()
        .map(|(kind, count)| ((*kind).to_owned(), *count))
        .collect()
}

/// A copy of `name` whose documents are written and indexed one at a time,
/// forward or reversed, so its database holds rows in that order.
fn written_in_order(
    scratch: &Scratch,
    name: &str,
    dir: &str,
    home: &Path,
    reverse: bool,
) -> PathBuf {
    let source = scratch.dir(&format!("source-{dir}"));
    common::copy_dir(&fixture(name), &source);
    let root = scratch.dir(dir);
    write(&root, "specengine.toml", read(&source, "specengine.toml"));
    let mut files: Vec<String> = md_files(&source.join("docs"))
        .into_iter()
        .map(|file| format!("docs/{file}"))
        .collect();
    if reverse {
        files.reverse();
    }
    index(home, &root);
    for file in files {
        write(&root, &file, read(&source, &file));
        index(home, &root);
    }
    root
}

/// A document of a generated corpus: `extra` is its `links:` block, written
/// whole, and may be empty.
fn document(id: &str, title: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntitle: {title}\nowner: owner\nreviewed: 2026-10-01\n\
         {extra}---\n\n# {title}\n\nA step.\n"
    )
}

/// A corpus of `steps` documents `PACE-0001`... with a config of `[ids]` and
/// `flow`; `links` gives a step's `links:` block.
fn generated(
    scratch: &Scratch,
    dir: &str,
    flow: &str,
    steps: usize,
    title: &str,
    links: impl Fn(usize) -> String,
) -> PathBuf {
    let root = scratch.dir(dir);
    write(
        &root,
        "specengine.toml",
        format!(
            "[project]\nslug = \"pace-{dir}\"\n\n[ids]\nPACE = {{ kind = \"pace\", width = 4 }}\n\
             {flow}"
        ),
    );
    for step in 1..=steps {
        let id = format!("PACE-{step:04}");
        write(
            &root,
            &format!("docs/spec/{}.md", id.to_lowercase()),
            document(&id, &format!("{title} {step}"), &links(step)),
        );
    }
    root
}

// ------------------------------------------------------------------- AC-01

/// AC-01: exit 0, the first line, the walk, the rows of a project with no
/// `[flow]`, on the fixture itself, which stays untouched.
#[test]
fn ac01_the_relay_walks_from_its_start() {
    let flow = Flow::new("flow-walk");
    let root = fixture(RELAY);
    let before = snapshot(&root);
    let run = flow.text(&root, &[]);
    assert_eq!(first_line(&run), "flow all | nodes 8, edges 10, depth -");
    assert_eq!(
        walk(&run.stdout),
        pairs(&[
            ("0", "LEG-START"),
            ("1", "LEG-DRAW"),
            ("2", "LEG-RUN"),
            ("3", "LEG-FALL"),
            ("3", "LEG-SCORE"),
            ("4", "LEG-END"),
            ("-", "KIT-OIL"),
            ("-", "KIT-TORCH"),
        ]),
        "{}",
        run.show()
    );
    assert_eq!(edge_lines(&run.stdout).len(), 10, "{}", run.show());
    assert!(
        node_lines(&run.stdout).contains(&"- KIT-OIL | kit | The oil | docs/spec/kit-oil.md:1"),
        "{}",
        run.show()
    );
    // Every line has its place: the header, the rows, the nodes, the edges.
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines.len(), 1 + 10 + 8 + 10, "{}", run.show());
    assert!(lines[1..11].iter().all(|line| line.starts_with("gap ")));

    let document = flow.json(&root, &[]);
    assert_eq!(document["root"], Value::Null, "{document}");
    assert_eq!(document["depth"], Value::Null, "{document}");
    assert_eq!(document["reason"], Value::Null, "{document}");
    assert_eq!(document["truncated"], json!(false), "{document}");
    assert_eq!(
        document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["distance"].clone())
            .collect::<Vec<Value>>(),
        [
            json!(0),
            json!(1),
            json!(2),
            json!(3),
            json!(3),
            json!(4),
            Value::Null,
            Value::Null
        ],
        "{document}"
    );
    assert_eq!(snapshot(&root), before, "the fixture is only read");
}

/// AC-01: seven rows `not_configured`, three `measured` at 0. M8 turns the
/// first half red.
#[test]
fn ac01_no_flow_table_seven_rows_not_configured_three_measured() {
    let flow = Flow::new("flow-rows");
    let root = fixture(RELAY);
    let run = flow.text(&root, &[]);
    let lines = gap_lines(&run.stdout);
    assert_eq!(lines.len(), 10, "{}", run.show());
    for (line, code) in lines.iter().zip(CODES) {
        assert!(
            line.starts_with(&format!("gap {code} | ")),
            "{code}: {line}"
        );
    }
    for line in &lines {
        let code = line["gap ".len()..].split(" | ").next().expect("a code");
        if NEEDING_NONE.contains(&code) {
            assert_eq!(*line, format!("gap {code} | measured | 0"));
        } else {
            assert!(
                line.starts_with(&format!("gap {code} | not_configured | ")),
                "{line}"
            );
            assert!(
                !line.split(" | ").any(|field| field == "0"),
                "a row needing an absent table is never 0: {line}"
            );
        }
    }

    let document = flow.json(&root, &[]);
    assert_eq!(codes(&document), CODES, "{document}");
    for (code, table) in NEEDING_A_TABLE {
        let gap = row(&document, code);
        assert_eq!(gap["state"], "not_configured", "{gap}");
        assert_ne!(gap["count"], json!(0), "never 0: {gap}");
        let cause = gap["cause"].as_str().expect("a cause");
        assert!(
            cause.starts_with(&format!("no [[flow.{table}]] rule: ")),
            "{code}: {cause}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(&format!("gap {code} | "))
                    && line.ends_with(&format!(" | {cause}"))),
            "the text carries the cause: {}",
            run.show()
        );
        if table == "start" {
            assert_eq!(cause, NO_START_CAUSE, "{code}");
        }
    }
    for code in NEEDING_NONE {
        let gap = row(&document, code);
        assert_eq!(gap["state"], "measured", "{gap}");
        assert_eq!(gap["count"], json!(0), "{gap}");
        assert_eq!(gap["items"], json!([]), "{gap}");
        assert_eq!(gap["cause"], Value::Null, "{gap}");
    }
}

/// AC-01: the same bytes from copies written in opposite file orders, at
/// different roots under their own `HOME`, and a repeat run the same bytes.
#[test]
fn ac01_opposite_file_orders_give_the_same_bytes() {
    for (name, scope) in [
        (RELAY, None),
        (HIRE, None),
        (GAPS, Some("WING-BROKEN")),
        (GAPS, Some("WING-LIBRARY")),
    ] {
        let scratch = Scratch::new("flow-order");
        let (home_a, home_b) = (scratch.home("a"), scratch.home("b"));
        let forward = written_in_order(&scratch, name, "forward", &home_a, false);
        let reverse = written_in_order(&scratch, name, "other/reverse", &home_b, true);
        let mut reads: Vec<Vec<&str>> = vec![
            vec!["flow"],
            vec!["--json", "flow"],
            vec!["flow", "--depth", "0"],
            vec!["flow", "--depth", "1"],
            vec!["--json", "flow", "--depth", "1"],
        ];
        if let Some(root) = scope {
            reads.push(vec!["flow", root]);
            reads.push(vec!["--json", "flow", root]);
            reads.push(vec!["flow", root, "--depth", "1"]);
        }
        for args in reads {
            let one = spec30(&home_a, &forward, &args);
            let other = spec30(&home_b, &reverse, &args);
            one.code(0);
            assert_eq!(bytes_of(&one), bytes_of(&other), "{name}: {args:?}");
            assert_eq!(
                bytes_of(&one),
                bytes_of(&spec30(&home_a, &forward, &args)),
                "{name}: {args:?} repeated"
            );
            assert!(
                !one.stdout.contains(forward.to_str().unwrap()),
                "no absolute root: {}",
                one.show()
            );
        }
    }
}

// ------------------------------------------------------------------- AC-02

/// AC-02: all four types in flow-gaps; `from` and `to` the reading
/// direction, read against the document each edge is written in. M2.
#[test]
fn ac02_every_type_reads_in_its_own_direction() {
    let flow = Flow::new("flow-types");
    let document = flow.json(&fixture(GAPS), &[]);
    let want = expected();
    let edges = document["edges"].as_array().expect("edges");
    let types: BTreeSet<&str> = edges
        .iter()
        .map(|edge| edge["type"].as_str().expect("a type"))
        .collect();
    assert_eq!(
        types,
        BTreeSet::from(["depends_on", "follows", "supplies", "uses"]),
        "{document}"
    );
    for edge in edges {
        let written_in = want["files"][edge["path"].as_str().expect("a path")]["id"].clone();
        let declared_by = if edge["type"] == "follows" {
            &edge["to"]
        } else {
            &edge["from"]
        };
        assert_eq!(*declared_by, written_in, "{edge}");
        assert_eq!(edge["count"], json!(1), "{edge}");
    }
    for (kind, count) in want["edges"].as_object().expect("the counts") {
        assert_eq!(
            edges.iter().filter(|edge| edge["type"] == *kind).count() as u64,
            count.as_u64().expect("a count"),
            "{kind}: {document}"
        );
    }
    assert_eq!(
        edge(&document, "LND-HAND", "PLT-MARK", "depends_on")["path"],
        "docs/spec/lnd-hand.md"
    );
    assert_eq!(
        edge(&document, "PLT-FEED-A", "PLT-FEED-B", "supplies")["path"],
        "docs/spec/plt-feed-a.md"
    );
    assert_eq!(
        edge(&document, "LND-STAMP", "PLT-MARK", "uses")["path"],
        "docs/spec/lnd-stamp.md"
    );
    assert_eq!(
        edge(&document, "LND-ASK", "LND-CHECK", "follows")["path"],
        "docs/spec/lnd-check.md"
    );
}

/// AC-02 and "Output": the relay's edges as printed, labels included, in
/// the reading direction. M2 turns `LEG-START -> LEG-DRAW` red.
#[test]
fn ac02_the_relay_prints_its_edges_forward() {
    let flow = Flow::new("flow-forward");
    let run = flow.text(&fixture(RELAY), &[]);
    let printed: BTreeSet<&str> = edge_lines(&run.stdout).into_iter().collect();
    let want = BTreeSet::from([
        "LEG-START -> LEG-DRAW | follows | x1 | docs/spec/leg-draw.md:9",
        "LEG-SCORE -> LEG-DRAW | follows | x1 | kind repeat | when rounds remain | \
         docs/spec/leg-draw.md:10",
        "LEG-SCORE -> LEG-END | follows | x1 | kind alternative | when no round remains | \
         docs/spec/leg-end.md:9",
        "LEG-RUN -> LEG-FALL | follows | x1 | kind alternative | when the torch goes out | \
         docs/spec/leg-fall.md:9",
        "LEG-DRAW -> LEG-RUN | follows | x1 | docs/spec/leg-run.md:9",
        "LEG-FALL -> LEG-RUN | follows | x1 | kind repeat | when the torch is relit | \
         docs/spec/leg-run.md:10",
        "LEG-RUN -> LEG-SCORE | follows | x1 | when the runner arrives | \
         docs/spec/leg-score.md:9",
        "LEG-FALL -> KIT-TORCH | uses | x1 | docs/spec/leg-fall.md:10",
        "LEG-RUN -> KIT-TORCH | uses | x1 | docs/spec/leg-run.md:11",
        "KIT-OIL -> KIT-TORCH | supplies | x1 | docs/spec/kit-oil.md:8",
    ]);
    assert_eq!(printed, want, "{}", run.show());
    let document = flow.json(&fixture(RELAY), &[]);
    assert_eq!(
        *edge(&document, "LEG-SCORE", "LEG-DRAW", "follows"),
        json!({"from": "LEG-SCORE", "to": "LEG-DRAW", "type": "follows", "kind": "repeat",
            "when": "rounds remain", "count": 1, "path": "docs/spec/leg-draw.md", "line": 10})
    );
    assert_eq!(
        *edge(&document, "LEG-START", "LEG-DRAW", "follows"),
        json!({"from": "LEG-START", "to": "LEG-DRAW", "type": "follows", "kind": null,
            "when": null, "count": 1, "path": "docs/spec/leg-draw.md", "line": 9})
    );
}

/// "Output": every key of every object is present, absent `null`.
#[test]
fn every_key_is_present() {
    let flow = Flow::new("flow-keys");
    let root = fixture(GAPS);
    for document in [
        flow.json(&root, &[]),
        flow.json(&root, &["WING-LIBRARY", "--depth", "1"]),
        flow.json(&fixture(RELAY), &[]),
        flow.json(&fixture("spec-c"), &[]),
    ] {
        assert_eq!(
            keys(&document),
            BTreeSet::from([
                "root",
                "reason",
                "depth",
                "notes",
                "left_out",
                "truncated",
                "omitted_nodes",
                "omitted_edges",
                "gaps",
                "nodes",
                "edges",
            ]),
            "{document}"
        );
        for gap in document["gaps"].as_array().expect("gaps") {
            assert_eq!(
                keys(gap),
                BTreeSet::from(["code", "state", "count", "items", "cause"]),
                "{gap}"
            );
            assert!(
                ["measured", "not_configured", "cannot"].contains(&gap["state"].as_str().unwrap()),
                "{gap}"
            );
        }
        for node in document["nodes"].as_array().expect("nodes") {
            assert_eq!(
                keys(node),
                BTreeSet::from([
                    "id", "kind", "title", "path", "line", "distance", "group", "stage", "start",
                    "end", "holds", "inner", "gaps", "rollup", "counts",
                ]),
                "{node}"
            );
            for inner in node["inner"].as_array().into_iter().flatten() {
                assert_eq!(keys(inner), BTreeSet::from(["type", "count"]), "{inner}");
            }
        }
        for edge in document["edges"].as_array().expect("edges") {
            assert_eq!(
                keys(edge),
                BTreeSet::from([
                    "from", "to", "type", "kind", "when", "count", "path", "line"
                ]),
                "{edge}"
            );
        }
    }
}

// ------------------------------------------------------------------- AC-03

/// AC-03: WING-BROKEN's rows, as printed: the whole of each line.
#[test]
fn ac03_the_broken_wing_holds_every_gap() {
    let flow = Flow::new("flow-broken");
    let root = fixture(GAPS);
    let run = flow.text(&root, &["WING-BROKEN"]);
    assert_eq!(
        gap_lines(&run.stdout),
        [
            "gap no-start | measured | 0",
            format!("gap two-starts | measured | 1 | LND-A + LND-B | {TWO_STARTS}").as_str(),
            &format!("gap unreachable | measured | 1 | LND-STRAY | {UNREACHABLE}"),
            &format!("gap no-way-to-end | measured | 2 | LND-DEAD, LND-STRAY | {NO_WAY_TO_END}"),
            &format!("gap trap-cycle | measured | 1 | LND-TRAP-A + LND-TRAP-B | {TRAP_CYCLE}"),
            &format!("gap unjoined-parallel | measured | 1 | LND-B | {UNJOINED_PARALLEL}"),
            &format!(
                "gap cycle-no-repeat | measured | 1 | LND-SPIN-A + LND-SPIN-B | {CYCLE_NO_REPEAT}"
            ),
            &format!(
                "gap definition-unused | measured | 3 | PLT-FEED-A, PLT-FEED-B, PLT-UNUSED | \
                 {DEFINITION_UNUSED}"
            ),
            &format!("gap definition-once | measured | 1 | PLT-ONCE | {DEFINITION_ONCE}"),
            &format!("gap supply-unfed | measured | 1 | PLT-FEED-A + PLT-FEED-B | {SUPPLY_UNFED}"),
        ],
        "{}",
        run.show()
    );
    assert_eq!(
        first_line(&run),
        "flow WING-BROKEN | nodes 17, edges 17, depth -",
        "{}",
        run.show()
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(document["root"], "WING-BROKEN", "{document}");
    assert_rows(&document, "WING-BROKEN");
}

/// AC-03: WING-HEADLESS: `no-start` names the scope, `unreachable` is
/// `cannot`, and the steps with no way out are still counted.
#[test]
fn ac03_the_headless_wing_has_no_start() {
    let flow = Flow::new("flow-headless");
    let root = fixture(GAPS);
    let run = flow.text(&root, &["WING-HEADLESS"]);
    let lines = gap_lines(&run.stdout);
    assert_eq!(
        lines[0],
        format!("gap no-start | measured | 1 | WING-HEADLESS | {NO_START}")
    );
    assert!(
        lines[2].starts_with("gap unreachable | cannot | "),
        "{lines:?}"
    );
    assert_eq!(
        lines[3],
        format!("gap no-way-to-end | measured | 2 | LND-X, LND-Y | {NO_WAY_TO_END}")
    );
    let document = flow.json(&root, &["WING-HEADLESS"]);
    assert_rows(&document, "WING-HEADLESS");
    assert_eq!(
        first_line(&run),
        "flow WING-HEADLESS | nodes 3, edges 1, depth -"
    );
}

/// "Scope": with no ROOT, and from the top of the tree, the one reading of
/// the whole corpus; three starts, and only the marked ones.
#[test]
fn the_whole_corpus_reads_alike_with_and_without_a_root() {
    let flow = Flow::new("flow-whole");
    let root = fixture(GAPS);
    let all = flow.json(&root, &[]);
    let top = flow.json(&root, &["WING-LIBRARY"]);
    assert_rows(&all, "all");
    assert_rows(&top, "WING-LIBRARY");
    assert_eq!(all["gaps"], top["gaps"]);
    assert_eq!(all["nodes"], top["nodes"]);
    assert_eq!(all["edges"], top["edges"]);
    assert_eq!(all["nodes"].as_array().expect("nodes").len(), 29, "{all}");
    assert_eq!(
        counted(&all),
        typed(&[
            ("follows", 25),
            ("uses", 3),
            ("supplies", 2),
            ("depends_on", 1),
        ])
    );
    for id in ["LND-ASK", "LND-A", "LND-B"] {
        assert_eq!(node(&all, id)["distance"], json!(0), "{id}");
        assert_eq!(node(&all, id)["start"], json!(true), "{id}");
    }
    // Not a marked start: nothing enters it, but a project marked its own.
    assert_eq!(node(&all, "LND-Y")["distance"], Value::Null);
    assert_eq!(node(&all, "LND-Y")["start"], json!(false));
}

// ------------------------------------------------------------------- AC-04

/// AC-04: a whole flow with both loops lists nothing; its resting end
/// carries its `repeat` edge. M3, M4.
#[test]
fn ac04_the_lending_wing_lists_nothing() {
    let flow = Flow::new("flow-loops");
    let root = fixture(GAPS);
    let run = flow.text(&root, &["WING-LEND"]);
    assert_eq!(
        gap_lines(&run.stdout),
        CODES
            .iter()
            .map(|code| format!("gap {code} | measured | 0"))
            .collect::<Vec<String>>(),
        "{}",
        run.show()
    );
    assert_eq!(
        walk(&run.stdout),
        pairs(&[
            ("0", "LND-ASK"),
            ("1", "LND-CHECK"),
            ("2", "LND-NOTE"),
            ("2", "LND-STAMP"),
            ("3", "LND-HAND"),
            ("4", "LND-OPEN"),
            ("-", "PLT-MARK"),
            ("-", "WING-LEND"),
        ]),
        "{}",
        run.show()
    );
    let open = node_lines(&run.stdout)
        .into_iter()
        .find(|line| head(line) == "4 LND-OPEN")
        .expect("LND-OPEN");
    assert!(
        open.starts_with("4 LND-OPEN | lending | The loan stays open | docs/spec/lnd-open.md:1 | "),
        "{open}"
    );
    assert!(open.split(" | ").any(|field| field == "end"), "{open}");
    assert!(
        edge_lines(&run.stdout).contains(
            &"LND-OPEN -> LND-OPEN | follows | x1 | kind repeat | docs/spec/lnd-open.md:11"
        ),
        "{}",
        run.show()
    );
    let document = flow.json(&root, &["WING-LEND"]);
    assert_rows(&document, "WING-LEND");
    assert_eq!(
        node(&document, "LND-OPEN")["end"],
        json!(true),
        "{document}"
    );
    assert_eq!(
        node(&document, "LND-ASK")["start"],
        json!(true),
        "{document}"
    );
    assert_eq!(
        node(&document, "LND-ASK")["group"],
        "WING-LEND",
        "{document}"
    );
    assert_eq!(
        edge(&document, "LND-OPEN", "LND-OPEN", "follows")["kind"],
        "repeat",
        "{document}"
    );
}

/// AC-04: flow-relay with a scratch `[flow]` by `paths`: its two loops list
/// nothing either, its project's kinds still its own. M3.
#[test]
fn ac04_the_relay_with_a_flow_table_lists_nothing() {
    let flow = Flow::new("flow-relay-table");
    let root = flow.copy(RELAY, "relay", RELAY_FLOW);
    let run = flow.text(&root, &[]);
    assert_eq!(
        gap_lines(&run.stdout),
        CODES
            .iter()
            .map(|code| format!("gap {code} | measured | 0"))
            .collect::<Vec<String>>(),
        "{}",
        run.show()
    );
    assert_eq!(first_line(&run), "flow all | nodes 8, edges 10, depth -");
    assert!(
        node_lines(&run.stdout)
            .contains(&"0 LEG-START | leg | The start | docs/spec/leg-start.md:1 | start"),
        "{}",
        run.show()
    );
    assert!(
        node_lines(&run.stdout)
            .contains(&"4 LEG-END | leg | The end | docs/spec/leg-end.md:1 | end"),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------- AC-05

/// AC-05: at depth 1 the two edges leaving WING-LEND are one, `x2`, the
/// first written holding its place; the rest folds into `inner`. M5.
#[test]
fn ac05_the_wings_show_one_lifted_edge() {
    let flow = Flow::new("flow-lifted");
    let root = fixture(GAPS);
    let run = flow.text(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        first_line(&run),
        "flow WING-LIBRARY | nodes 4, edges 1, depth 1",
        "{}",
        run.show()
    );
    assert_eq!(
        edge_lines(&run.stdout),
        ["WING-LEND -> WING-BROKEN | follows | x2 | kind alternative | docs/spec/lnd-a.md:10"],
        "{}",
        run.show()
    );
    let document = flow.json(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(document["depth"], json!(1), "{document}");
    assert_eq!(
        *edge(&document, "WING-LEND", "WING-BROKEN", "follows"),
        json!({"from": "WING-LEND", "to": "WING-BROKEN", "type": "follows",
            "kind": "alternative", "when": null, "count": 2,
            "path": "docs/spec/lnd-a.md", "line": 10})
    );
    let lend = node(&document, "WING-LEND");
    assert_eq!(lend["holds"], json!(7), "{lend}");
    assert_eq!(
        inner(lend),
        typed(&[("follows", 8), ("uses", 2), ("depends_on", 1)]),
        "{lend}"
    );
    let broken = node(&document, "WING-BROKEN");
    assert_eq!(broken["holds"], json!(16), "{broken}");
    assert_eq!(
        inner(broken),
        typed(&[("follows", 14), ("uses", 1), ("supplies", 2)]),
        "{broken}"
    );
    let headless = node(&document, "WING-HEADLESS");
    assert_eq!(headless["holds"], json!(2), "{headless}");
    assert_eq!(inner(headless), typed(&[("follows", 1)]), "{headless}");
}

/// AC-05: the depths 0, 1, 2 and unbounded conserve each type's count and
/// keep every gap row byte for byte; `--depth` never changes a gap row.
#[test]
fn ac05_a_depth_conserves_counts_and_keeps_the_rows() {
    let flow = Flow::new("flow-depths");
    let root = fixture(GAPS);
    let want = typed(&[
        ("follows", 25),
        ("uses", 3),
        ("supplies", 2),
        ("depends_on", 1),
    ]);
    let whole = flow.text(&root, &["WING-LIBRARY"]);
    let rows = gap_lines(&whole.stdout);
    let mut shown = Vec::new();
    for depth in ["0", "1", "2"] {
        let args = ["WING-LIBRARY", "--depth", depth];
        let run = flow.text(&root, &args);
        assert_eq!(gap_lines(&run.stdout), rows, "depth {depth}");
        let document = flow.json(&root, &args);
        assert_eq!(counted(&document), want, "depth {depth}: {document}");
        shown.push(document["nodes"].as_array().expect("nodes").len());
    }
    let document = flow.json(&root, &["WING-LIBRARY"]);
    assert_eq!(counted(&document), want, "{document}");
    shown.push(document["nodes"].as_array().expect("nodes").len());
    assert_eq!(shown, [1, 4, 29, 29], "the nodes at each depth");

    let top = flow.json(&root, &["WING-LIBRARY", "--depth", "0"]);
    assert_eq!(node(&top, "WING-LIBRARY")["holds"], json!(28), "{top}");
    assert_eq!(top["edges"], json!([]), "{top}");
    assert_eq!(inner(node(&top, "WING-LIBRARY")), want, "{top}");
}

/// "Depth": `when` stays at count 1, leaves at a merge; two kinds stay two
/// edges; the first written gives `path` and `line`.
#[test]
fn a_merged_edge_keeps_when_only_alone() {
    let flow = Flow::new("flow-when");

    // Both labelled: merged, `when` gone, the first written's place.
    let root = flow.scratch.copy(GAPS, "both");
    replace(
        &root,
        "docs/spec/lnd-a.md",
        "{to: LND-OPEN, kind: alternative}",
        "{to: LND-OPEN, kind: alternative, when: \"the desk is closed\"}",
    );
    replace(
        &root,
        "docs/spec/lnd-b.md",
        "{to: LND-OPEN, kind: alternative}",
        "{to: LND-OPEN, kind: alternative, when: \"the desk is shut\"}",
    );
    let run = flow.text(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        edge_lines(&run.stdout),
        ["WING-LEND -> WING-BROKEN | follows | x2 | kind alternative | docs/spec/lnd-a.md:10"],
        "{}",
        run.show()
    );
    let run = flow.text(&root, &[]);
    assert!(
        edge_lines(&run.stdout).contains(
            &"LND-OPEN -> LND-A | follows | x1 | kind alternative | when the desk is closed | \
              docs/spec/lnd-a.md:10"
        ),
        "{}",
        run.show()
    );

    // One edge alone: lifted, it keeps its `when`.
    let root = flow.scratch.copy(GAPS, "one");
    replace(
        &root,
        "docs/spec/lnd-a.md",
        "{to: LND-OPEN, kind: alternative}",
        "{to: LND-OPEN, kind: alternative, when: \"the desk is closed\"}",
    );
    replace(
        &root,
        "docs/spec/lnd-b.md",
        "    - {to: LND-OPEN, kind: alternative}\n",
        "    - LND-STRAY\n",
    );
    let run = flow.text(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        edge_lines(&run.stdout),
        [
            "WING-LEND -> WING-BROKEN | follows | x1 | kind alternative | when the desk is \
             closed | docs/spec/lnd-a.md:10"
        ],
        "{}",
        run.show()
    );

    // Two kinds: two edges, each `x1`, each its own.
    let root = flow.scratch.copy(GAPS, "kinds");
    replace(
        &root,
        "docs/spec/lnd-b.md",
        "    - {to: LND-OPEN, kind: alternative}\n",
        "    - LND-OPEN\n",
    );
    let run = flow.text(&root, &["WING-LIBRARY", "--depth", "1"]);
    let printed: BTreeSet<&str> = edge_lines(&run.stdout).into_iter().collect();
    assert_eq!(
        printed,
        BTreeSet::from([
            "WING-LEND -> WING-BROKEN | follows | x1 | kind alternative | docs/spec/lnd-a.md:10",
            "WING-LEND -> WING-BROKEN | follows | x1 | docs/spec/lnd-b.md:10",
        ]),
        "{}",
        run.show()
    );
}

/// "Depth": a node folding others prints `holds`, `inner`; the bounded
/// header says so; a ROOT's own depth counts from it.
#[test]
fn a_depth_counts_from_the_root() {
    let flow = Flow::new("flow-root-depth");
    let root = fixture(GAPS);
    let run = flow.text(&root, &["WING-BROKEN", "--depth", "0"]);
    assert_eq!(
        first_line(&run),
        "flow WING-BROKEN | nodes 1, edges 0, depth 0",
        "{}",
        run.show()
    );
    let line = node_lines(&run.stdout)[0];
    assert!(line.contains(" | holds 16, inner 17"), "{line}");
    let document = flow.json(&root, &["WING-BROKEN", "--depth", "0"]);
    assert_eq!(
        inner(node(&document, "WING-BROKEN")),
        typed(&[("follows", 14), ("uses", 1), ("supplies", 2)]),
        "only the scope's edges: {document}"
    );
    // No parent in the relay: every document is a root.
    let relay = fixture(RELAY);
    assert_eq!(
        first_line(&flow.text(&relay, &["--depth", "0"])),
        "flow all | nodes 8, edges 10, depth 0"
    );
}

// ------------------------------------------------------------------- AC-06

/// AC-06: the same binary on four projects that share no word. M6 is the
/// core scan.
#[test]
fn ac06_the_same_binary_answers_on_four_projects() {
    let flow = Flow::new("flow-genre");
    for (name, first) in [
        (RELAY, "flow all | nodes 8, edges 10, depth -"),
        (HIRE, "flow all | nodes 8, edges 9, depth -"),
        (GAPS, "flow all | nodes 29, edges 31, depth -"),
        ("spec-c", "flow all | nodes 0, edges 0, depth -"),
    ] {
        let root = fixture(name);
        let before = snapshot(&root);
        let run = flow.text(&root, &[]);
        assert_eq!(first_line(&run), first, "{}", run.show());
        assert_eq!(gap_lines(&run.stdout).len(), 10, "{name}: {}", run.show());
        assert_eq!(snapshot(&root), before, "{name}: only read");
    }
    // spec-c's rows stand with no node: nothing to read is no failure.
    let run = flow.text(&fixture("spec-c"), &[]);
    assert_eq!(run.stdout.lines().count(), 11, "{}", run.show());
    // flow-hire's one parallel branch is a row of its own, not a failure.
    let run = flow.text(&fixture(HIRE), &[]);
    assert!(
        gap_lines(&run.stdout).contains(
            &format!("gap unjoined-parallel | measured | 1 | JRN-UNLOCK | {UNJOINED_PARALLEL}")
                .as_str()
        ),
        "{}",
        run.show()
    );
    // The two game-design and tooling corpora hold `depends_on`.
    for name in ["spec-a", "spec-b"] {
        let root = fixture(name);
        let run = flow.run(&root, &[]);
        run.code(0);
        assert!(
            first_line(&run).starts_with("flow all | nodes "),
            "{name}: {}",
            run.show()
        );
    }
}

// ------------------------------------------------------------------- AC-07

/// A flow-gaps-free corpus past the cut: `steps` long lines, each step
/// following the last, nodes alone past 40 000 characters; the cut lands
/// inside a multi-byte title on no character boundary it could break.
fn past_the_cap(flow: &Flow, dir: &str, steps: usize) -> PathBuf {
    let title = format!("A long way round the {} block", "\u{0442}".repeat(30));
    generated(&flow.scratch, dir, "", steps, &title, |step| {
        if step == 1 {
            String::new()
        } else {
            format!("links:\n  follows: [PACE-{:04}]\n", step - 1)
        }
    })
}

/// `[truncated: ...]` as "Output" writes it.
fn tail(k: usize, n: usize, j: usize, m: usize) -> String {
    format!(
        "[truncated: {k} of {n} nodes and {j} of {m} edges not shown; give a ROOT or lower \
         --depth]"
    )
}

/// The text's and the JSON's cut agree: asserts the text within the cap and
/// cut at a line, every gap row there, the tail last, the JSON's items the
/// printed ones. Returns the printed node and edge counts.
fn assert_cut(flow: &Flow, root: &Path, nodes: usize, edges: usize) -> (usize, usize) {
    let run = flow.text(root, &[]);
    let document = flow.json(root, &[]);
    assert_eq!(
        first_line(&run),
        format!("flow all | nodes {nodes}, edges {edges}, depth -"),
        "the header counts the whole"
    );
    let lines: Vec<&str> = run.stdout.lines().collect();
    let last = *lines.last().expect("lines");
    let before: String = lines[..lines.len() - 1]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
    assert!(
        before.chars().count() <= specengine_cli::OUTPUT_CAP_CHARS,
        "{} characters before the tail",
        before.chars().count()
    );
    assert!(run.stdout.ends_with("]\n"), "the tail ends the text");
    assert_eq!(gap_lines(&run.stdout).len(), 10, "every gap row printed");
    assert_eq!(codes(&document), CODES);

    let printed_nodes = node_lines(&run.stdout);
    let printed_edges = edge_lines(&run.stdout);
    // Every line is whole: nothing but the rows, the nodes and the edges.
    assert_eq!(
        lines.len(),
        1 + 10 + printed_nodes.len() + printed_edges.len() + 1,
        "{last}"
    );
    assert_eq!(
        last,
        tail(
            nodes - printed_nodes.len(),
            nodes,
            edges - printed_edges.len(),
            edges
        )
    );
    assert_eq!(document["truncated"], json!(true));
    assert_eq!(
        document["omitted_nodes"],
        json!(nodes - printed_nodes.len()),
        "the JSON's cut is the text's"
    );
    assert_eq!(
        document["omitted_edges"],
        json!(edges - printed_edges.len()),
        "the JSON's cut is the text's"
    );
    assert_eq!(
        document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["id"].as_str().expect("an ID").to_owned())
            .collect::<Vec<String>>(),
        printed_nodes
            .iter()
            .map(|line| head(line).split(' ').nth(1).expect("a name").to_owned())
            .collect::<Vec<String>>(),
        "JSON holds the printed nodes only"
    );
    assert_eq!(
        document["edges"]
            .as_array()
            .expect("edges")
            .iter()
            .map(|edge| format!(
                "{} -> {}",
                edge["from"].as_str().expect("from"),
                edge["to"].as_str().expect("to")
            ))
            .collect::<Vec<String>>(),
        printed_edges
            .iter()
            .map(|line| head(line).to_owned())
            .collect::<Vec<String>>(),
        "JSON holds the printed edges only"
    );
    (printed_nodes.len(), printed_edges.len())
}

/// AC-07: nodes alone past the cut: no edge printed, none in JSON, the
/// gap rows above them all, on characters not bytes. M7.
#[test]
fn ac07_a_corpus_past_the_cap_is_cut_at_a_line() {
    let flow = Flow::new("flow-cap");
    let root = past_the_cap(&flow, "long", 600);
    let (nodes, edges) = assert_cut(&flow, &root, 600, 599);
    assert!(nodes > 100 && nodes < 600, "{nodes} nodes printed");
    assert_eq!(edges, 0, "nodes fill the cut");
}

/// AC-07: nodes fit, edges pass the cut: JSON keeps the printed edges and
/// no other. M7.
#[test]
fn ac07_edges_past_the_cut_leave_the_json() {
    let flow = Flow::new("flow-cap-edges");
    let when = "w".repeat(250);
    let root = generated(&flow.scratch, "edges", "", 200, "A step", |step| {
        if step == 1 {
            String::new()
        } else {
            format!(
                "links:\n  follows:\n    - {{to: PACE-{:04}, when: \"{when}\"}}\n",
                step - 1
            )
        }
    });
    let (nodes, edges) = assert_cut(&flow, &root, 200, 199);
    assert_eq!(nodes, 200, "every node printed");
    assert!(edges > 0 && edges < 199, "{edges} edges printed");
}

/// "Output": a row prints at most 50 items, then `, ... and <k> more`; `count`
/// is the whole; the text's row stays one line.
#[test]
fn a_row_prints_at_most_fifty_items() {
    let flow = Flow::new("flow-items");
    let table = "\n[[flow.step]]\nkinds = [\"pace\"]\n\n\
                 [[flow.start]]\npaths = [\"docs/spec/pace-0001.md\"]\n\n\
                 [[flow.end]]\npaths = [\"docs/spec/pace-0001.md\"]\n";
    let root = generated(&flow.scratch, "items", table, 60, "A step", |_| {
        String::new()
    });
    let run = flow.text(&root, &[]);
    let names: Vec<String> = (2..=51).map(|step| format!("PACE-{step:04}")).collect();
    let want = format!(
        "gap unreachable | measured | 59 | {}, \u{2026} and 9 more | {UNREACHABLE}",
        names.join(", ")
    );
    assert!(
        gap_lines(&run.stdout).contains(&want.as_str()),
        "{}",
        run.show()
    );
    let document = flow.json(&root, &[]);
    assert_eq!(row(&document, "unreachable")["count"], json!(59));
    assert_eq!(
        row(&document, "unreachable")["items"]
            .as_array()
            .expect("items")
            .len(),
        50,
        "JSON holds the printed items only"
    );

    // At exactly 50, no tail.
    let root = generated(&flow.scratch, "fifty", table, 51, "A step", |_| {
        String::new()
    });
    let run = flow.text(&root, &[]);
    let line = gap_lines(&run.stdout)
        .into_iter()
        .find(|line| line.starts_with("gap unreachable "))
        .expect("the row")
        .to_owned();
    assert!(
        line.ends_with(&format!(", PACE-0051 | {UNREACHABLE}")),
        "{line}"
    );
    assert!(!line.contains('\u{2026}'), "{line}");
}

// ------------------------------------------------------------------- AC-08

/// AC-08: no `[[flow.end]]`: its two rows `not_configured` with the cause;
/// the other rows still measure. M8.
#[test]
fn ac08_an_absent_table_leaves_its_rows_not_configured() {
    let flow = Flow::new("flow-absent");
    let root = flow.scratch.copy(GAPS, "gaps");
    replace(
        &root,
        "specengine.toml",
        "[[flow.end]]\npaths = [\"docs/spec/lnd-open.md\", \"docs/spec/lnd-end.md\", \
         \"docs/spec/lnd-end-two.md\"]\n",
        "",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    for code in ["no-way-to-end", "trap-cycle"] {
        let gap = row(&document, code);
        assert_eq!(gap["state"], "not_configured", "{gap}");
        assert_ne!(gap["count"], json!(0), "never 0: {gap}");
        assert!(
            gap["cause"]
                .as_str()
                .expect("a cause")
                .starts_with("no [[flow.end]] rule: "),
            "{gap}"
        );
    }
    assert_eq!(row(&document, "two-starts")["state"], "measured");
    assert_eq!(row(&document, "two-starts")["count"], json!(1));
    assert_eq!(row(&document, "unreachable")["count"], json!(1));
    assert_eq!(row(&document, "cycle-no-repeat")["count"], json!(1));
    assert_eq!(row(&document, "definition-unused")["count"], json!(3));
}

/// AC-08: a bad table: the rows needing it `cannot`, naming `<config>:<line>`,
/// exit 0; `spec check` and `spec tree` answer as they did without it.
#[test]
fn ac08_a_bad_rule_leaves_its_rows_cannot() {
    let flow = Flow::new("flow-bad");
    let pristine = flow.copy(GAPS, "pristine", "");
    let (check_before, tree_before) = (
        flow.other(&pristine, &["check"]),
        flow.other(&pristine, &["tree"]),
    );
    check_before.code(0);
    tree_before.code(0);
    for (dir, table, codes_cannot) in [
        (
            "no-selector",
            "\n[[flow.definition]]   # HERE\n",
            &["definition-unused", "definition-once"][..],
        ),
        (
            "unknown-key",
            "\n[[flow.definition]]\nkinds = [\"bookplate\"]\nflavour = 1   # HERE\n",
            &["definition-unused", "definition-once"][..],
        ),
        (
            "bad-kind",
            "\n[[flow.end]]\nkinds = [\"nothing\"]   # HERE\n",
            &["no-way-to-end", "trap-cycle"][..],
        ),
    ] {
        let root = flow.copy(GAPS, dir, table);
        let config = read_text(&root, "specengine.toml");
        let at = config
            .lines()
            .position(|line| line.ends_with("# HERE"))
            .expect("the mark")
            + 1;
        let run = flow.text(&root, &["WING-BROKEN"]);
        let document = flow.json(&root, &["WING-BROKEN"]);
        for code in codes_cannot {
            let gap = row(&document, code);
            assert_eq!(gap["state"], "cannot", "{dir}: {gap}");
            let cause = gap["cause"].as_str().expect("a cause");
            assert!(
                cause.starts_with(&format!("specengine.toml:{at}: ")),
                "{dir}: {cause}"
            );
            assert!(
                gap_lines(&run.stdout)
                    .iter()
                    .any(|line| line.starts_with(&format!("gap {code} | cannot"))
                        && line.ends_with(cause)),
                "{dir}: the text carries it: {}",
                run.show()
            );
        }
        for gap in document["gaps"].as_array().expect("gaps") {
            if !codes_cannot.contains(&gap["code"].as_str().expect("a code")) {
                assert_eq!(gap["state"], "measured", "{dir}: {gap}");
            }
        }
        // `[flow]` is `spec flow`'s alone: nothing else reads it.
        let check = flow.other(&root, &["check"]);
        assert_eq!(check.code, 0, "{dir}: {}", check.show());
        assert_eq!(bytes_of(&check), bytes_of(&check_before), "{dir}: check");
        let tree = flow.other(&root, &["tree"]);
        assert_eq!(bytes_of(&tree), bytes_of(&tree_before), "{dir}: tree");
    }
}

/// "Data": the first error by line wins, and a rule's table alone is judged.
#[test]
fn the_first_error_by_line_is_the_cause() {
    let flow = Flow::new("flow-first");
    let root = flow.copy(
        GAPS,
        "first",
        "\n[[flow.definition]]\nkinds = [\"nothing\"]   # HERE\nflavour = 1\n",
    );
    let at = read_text(&root, "specengine.toml")
        .lines()
        .position(|line| line.ends_with("# HERE"))
        .expect("the mark")
        + 1;
    let document = flow.json(&root, &[]);
    let cause = row(&document, "definition-once")["cause"].clone();
    let cause = cause.as_str().expect("a cause");
    assert!(
        cause.starts_with(&format!("specengine.toml:{at}: ")),
        "{cause}"
    );
    assert!(cause.contains("nothing"), "{cause}");
    assert_eq!(cause, row(&document, "definition-unused")["cause"]);
}

/// AC-08: a `when` on a project's own key selects its document, and draws
/// the warning the spec names, which blocks nothing. `[[check.rules]]`'
/// grammar: a list ORs.
#[test]
fn ac08_a_when_on_a_scratch_key_selects_its_document() {
    let flow = Flow::new("flow-when-key");
    let root = flow.copy(
        RELAY,
        "relay",
        "\n[[flow.start]]\npaths = [\"docs/spec/leg-start.md\"]\n\n\
         [[flow.end]]\nwhen = { phase = \"last\" }\n",
    );
    replace(
        &root,
        "docs/spec/leg-end.md",
        "title: The end\n",
        "title: The end\nphase: last\n",
    );
    let document = flow.json(&root, &[]);
    assert_eq!(node(&document, "LEG-END")["end"], json!(true), "{document}");
    assert_eq!(
        node(&document, "LEG-SCORE")["end"],
        json!(false),
        "{document}"
    );
    assert_eq!(
        row(&document, "no-way-to-end")["count"],
        json!(0),
        "{document}"
    );

    let check = flow.other(&root, &["--json", "check"]);
    check.code(0);
    let report = check.json();
    let findings = report["findings"].as_array().expect("findings");
    assert_eq!(
        findings
            .iter()
            .map(|finding| (finding["code"].clone(), finding["severity"].clone()))
            .collect::<Vec<_>>(),
        [(json!("unknown-key"), json!("warning"))],
        "the warning stands, and blocks nothing: {report}"
    );

    // ANDed with another selector: a `kinds` the document lacks selects none.
    let root = flow.copy(
        RELAY,
        "and",
        "\n[[flow.start]]\npaths = [\"docs/spec/leg-start.md\"]\n\n\
         [[flow.end]]\nkinds = [\"kit\"]\npaths = [\"docs/spec/leg-end.md\"]\n",
    );
    let document = flow.json(&root, &[]);
    assert_eq!(
        node(&document, "LEG-END")["end"],
        json!(false),
        "{document}"
    );

    // Rules OR: a second selects what the first left.
    let root = flow.copy(
        RELAY,
        "or",
        "\n[[flow.start]]\npaths = [\"docs/spec/leg-start.md\"]\n\n\
         [[flow.end]]\nkinds = [\"kit\"]\n\n[[flow.end]]\npaths = [\"docs/spec/leg-end.md\"]\n",
    );
    let document = flow.json(&root, &[]);
    assert_eq!(node(&document, "LEG-END")["end"], json!(true), "{document}");
    assert_eq!(node(&document, "KIT-OIL")["end"], json!(true), "{document}");
}

// ------------------------------------------------------------------- Exits

/// "Exits": 0 whatever a gap says, 1 only a ROOT naming nothing, 2 usage,
/// a bad `--depth`, a look-alike, config outside `[flow]`, `HOME`, a store
/// error; a refusal prints no document.
#[test]
fn exits_of_the_flow_read() {
    let flow = Flow::new("flow-exit");
    let root = fixture(GAPS);

    let run = flow.run(&root, &["LND-NOPE"]);
    run.code(1);
    assert!(run.stderr.starts_with("spec: "), "{}", run.show());
    assert!(run.stderr.contains("LND-NOPE"), "{}", run.show());
    let run = flow.run(&root, &["LND-NOPE", "--json"]);
    run.code(1);
    let document = run.json();
    assert!(
        document["reason"]
            .as_str()
            .expect("a reason")
            .contains("LND-NOPE"),
        "{document}"
    );
    assert_eq!(document["nodes"], json!([]), "{document}");
    assert_eq!(document["edges"], json!([]), "{document}");

    let look_alike = "LND-\u{0410}SK";
    for args in [
        &[look_alike][..],
        &[look_alike, "--json"],
        &["other:LND-ASK"],
        &["--depth", "-1"],
        &["--depth", "x"],
        &["--depth", "-1", "--json"],
        &["--depth"],
        &["LND-ASK", "LND-CHECK"],
        &["--archive"],
        &["--type", "follows"],
        &["--nope"],
    ] {
        let run = flow.run(&root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: {}", run.show());
        assert!(!run.stderr.is_empty(), "{args:?}");
    }
    let run = flow.run(&root, &[look_alike]);
    assert!(run.stderr.contains("`LND-ASK`"), "{}", run.show());

    // Config outside `[flow]`.
    let bad = flow.copy(GAPS, "bad-config", "\n[bogus]\nkey = 1\n");
    let run = flow.run(&bad, &[]);
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(run.stderr.contains("specengine.toml"), "{}", run.show());

    // No project, no `HOME`.
    let elsewhere = flow.scratch.dir("no-project");
    spec30(&flow.home, &elsewhere, &["flow"]).code(2);
    let run = spec_with(
        &flow.cwd,
        &["flow", "--root", root.to_str().expect("UTF-8 root")],
        &[],
    );
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());

    // A store error: the database file holds no database.
    let stored = flow.scratch.home("stored");
    let copy = flow.scratch.copy(GAPS, "stored");
    fs::create_dir_all(data_dir(&stored)).expect("data directory");
    write(
        &data_dir(&stored),
        "flow-gaps.db",
        "not a database at all, not by a long way",
    );
    let run = spec30(
        &stored,
        &flow.cwd,
        &["flow", "--root", copy.to_str().expect("UTF-8 root")],
    );
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());

    // A corpus holding nothing is an answer, not a refusal.
    let empty = flow.scratch.dir("empty");
    write(&empty, "specengine.toml", "[project]\nslug = \"empty\"\n");
    let run = flow.run(&empty, &[]);
    run.code(0);
    assert_eq!(first_line(&run), "flow all | nodes 0, edges 0, depth -");
    assert_eq!(gap_lines(&run.stdout).len(), 10, "{}", run.show());
}

// ------------------------------------------------------------- Rules

/// "Rules and edge cases": a gap row changes no exit, finding, status or
/// gate: the broken corpus is clean, and every other command answers alike
/// with `[flow]` taken away.
#[test]
fn a_gap_row_changes_no_exit_finding_or_status() {
    let flow = Flow::new("flow-no-gate");
    let root = flow.scratch.copy(GAPS, "with");
    let check = flow.other(&root, &["check"]);
    check.code(0);
    assert!(
        check
            .stdout
            .lines()
            .any(|line| line.starts_with("spec check ") && line.ends_with(" \u{2014} clean")),
        "{}",
        check.show()
    );
    assert!(
        check
            .stdout
            .contains(" 29 documents, 0 errors, 0 warnings,"),
        "{}",
        check.show()
    );

    let bare = flow.scratch.copy(GAPS, "bare");
    let config = read_text(&bare, "specengine.toml");
    let (kept, _) = config.split_once("[[flow.step]]").expect("the tables");
    write(&bare, "specengine.toml", kept);
    for args in [
        &["check"][..],
        &["--json", "check"],
        &["check", "--debt"],
        &["tree"],
        &["--json", "tree"],
        &["graph", "LND-CHECK"],
        &["health"],
        &["--json", "health"],
    ] {
        let with = flow.other(&root, args);
        let without = flow.other(&bare, args);
        assert_eq!(bytes_of(&with), bytes_of(&without), "{args:?}");
    }
}

/// "Steps": `[[flow.step]]` is optional: without it a document no `follows`
/// touches is no step, so no row lists it.
#[test]
fn a_step_is_a_rule_or_a_follows_edge() {
    let flow = Flow::new("flow-steps");
    let root = flow.scratch.copy(GAPS, "gaps");
    replace(
        &root,
        "specengine.toml",
        "[[flow.step]]\nkinds = [\"lending\"]\n\n",
        "",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "unreachable")["count"],
        json!(0),
        "{document}"
    );
    assert_eq!(
        row(&document, "no-way-to-end")["items"],
        json!([["LND-DEAD"]]),
        "{document}"
    );
    assert_eq!(node(&document, "LND-STRAY")["distance"], Value::Null);

    // A start or an end is a step with no `follows` near it.
    let lone = flow.copy(
        RELAY,
        "relay",
        "\n[[flow.start]]\npaths = [\"docs/spec/kit-oil.md\", \"docs/spec/leg-start.md\"]\n\n\
         [[flow.end]]\npaths = [\"docs/spec/leg-end.md\"]\n",
    );
    let document = flow.json(&lone, &[]);
    assert_eq!(
        node(&document, "KIT-OIL")["distance"],
        json!(0),
        "{document}"
    );
    assert_eq!(
        row(&document, "two-starts")["count"],
        json!(1),
        "{document}"
    );
    assert_eq!(
        row(&document, "no-way-to-end")["items"],
        json!([["KIT-OIL"]]),
        "{document}"
    );
}

/// "A cycle": a self-edge is one. A step following itself plainly, no end
/// near, is a trap and a cycle with no `repeat`; with `repeat` a trap alone.
#[test]
fn a_self_edge_is_a_cycle() {
    let flow = Flow::new("flow-self");
    let root = flow.scratch.copy(GAPS, "plain");
    replace(
        &root,
        "docs/spec/lnd-dead.md",
        "    - LND-A\n",
        "    - LND-A\n    - LND-DEAD\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "cycle-no-repeat")["items"],
        json!([["LND-DEAD"], ["LND-SPIN-A", "LND-SPIN-B"]]),
        "{document}"
    );
    assert_eq!(
        row(&document, "trap-cycle")["items"],
        json!([["LND-DEAD"], ["LND-TRAP-A", "LND-TRAP-B"]]),
        "{document}"
    );
    assert_eq!(
        row(&document, "no-way-to-end")["items"],
        json!([["LND-STRAY"]]),
        "in a trap, so not a dead end: {document}"
    );

    let root = flow.scratch.copy(GAPS, "repeat");
    replace(
        &root,
        "docs/spec/lnd-dead.md",
        "    - LND-A\n",
        "    - LND-A\n    - {to: LND-DEAD, kind: repeat}\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "cycle-no-repeat")["items"],
        json!([["LND-SPIN-A", "LND-SPIN-B"]]),
        "{document}"
    );
    assert_eq!(
        row(&document, "trap-cycle")["items"],
        json!([["LND-DEAD"], ["LND-TRAP-A", "LND-TRAP-B"]]),
        "{document}"
    );
}

/// `unjoined-parallel`: the walk stops at the split, so branches that meet
/// only by coming back there share no later step.
#[test]
fn branches_meeting_only_at_the_split_are_unjoined() {
    let flow = Flow::new("flow-split");
    let root = flow.scratch.copy(GAPS, "gaps");
    // Under the lending wing the two branches meet at LND-HAND: take that
    // away, LND-NOTE and LND-STAMP each coming back to LND-CHECK alone.
    replace(
        &root,
        "docs/spec/lnd-hand.md",
        "    - LND-STAMP\n    - LND-NOTE\n",
        "    - LND-STAMP\n",
    );
    replace(
        &root,
        "docs/spec/lnd-check.md",
        "    - {to: LND-HAND, kind: repeat}\n",
        "    - {to: LND-HAND, kind: repeat}\n    - {to: LND-NOTE, kind: repeat}\n",
    );
    let document = flow.json(&root, &["WING-LEND"]);
    assert_eq!(
        row(&document, "unjoined-parallel")["items"],
        json!([["LND-CHECK"]]),
        "{document}"
    );
}

/// "Scope edges": under a ROOT a `follows` edge crossing the scope is an
/// entry or a way out: a sub-flow is not called broken.
#[test]
fn a_crossing_edge_is_an_entry_or_a_way_out() {
    let flow = Flow::new("flow-crossing");

    // No start in the closed wing, but LND-OPEN enters it twice.
    let root = flow.scratch.copy(GAPS, "entry");
    replace(
        &root,
        "specengine.toml",
        "paths = [\"docs/spec/lnd-ask.md\", \"docs/spec/lnd-a.md\", \"docs/spec/lnd-b.md\"]",
        "paths = [\"docs/spec/lnd-ask.md\"]",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(row(&document, "no-start")["count"], json!(0), "{document}");
    assert_eq!(
        row(&document, "two-starts")["count"],
        json!(0),
        "{document}"
    );
    assert_eq!(
        row(&document, "unreachable")["items"],
        json!([["LND-STRAY"]]),
        "entered steps are reached: {document}"
    );

    // No end in the lending wing, but LND-OPEN leaves it.
    let root = flow.scratch.copy(GAPS, "exit");
    replace(
        &root,
        "specengine.toml",
        "paths = [\"docs/spec/lnd-open.md\", \"docs/spec/lnd-end.md\", \
         \"docs/spec/lnd-end-two.md\"]",
        "paths = [\"docs/spec/lnd-end.md\", \"docs/spec/lnd-end-two.md\"]",
    );
    let document = flow.json(&root, &["WING-LEND"]);
    assert_eq!(
        row(&document, "no-way-to-end")["count"],
        json!(0),
        "{document}"
    );
    assert_eq!(
        row(&document, "trap-cycle")["count"],
        json!(0),
        "{document}"
    );

    // The edge crosses only under a ROOT: read whole, LND-OPEN goes on.
    let document = flow.json(&root, &[]);
    assert_eq!(
        node(&document, "LND-OPEN")["end"],
        json!(false),
        "{document}"
    );
}

/// "Scope edges": `definition-*` and `supply-unfed` read every live
/// document: a use outside the ROOT counts, and a use in no live one does
/// not.
#[test]
fn the_domain_free_rows_read_every_live_document() {
    let flow = Flow::new("flow-live");

    // A use outside the closed wing.
    let root = flow.scratch.copy(GAPS, "outside");
    replace(
        &root,
        "docs/spec/lnd-x.md",
        "    - LND-Y\n",
        "    - LND-Y\n  uses: [PLT-UNUSED]\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "definition-unused")["items"],
        json!([["PLT-FEED-A"], ["PLT-FEED-B"]]),
        "{document}"
    );
    assert_eq!(
        row(&document, "definition-once")["items"],
        json!([["PLT-ONCE"], ["PLT-UNUSED"]]),
        "{document}"
    );

    // A generated document's and a Tier 3 document's use count for nothing.
    let root = flow.scratch.copy(GAPS, "not-live");
    write(
        &root,
        "docs/spec/lnd-gen.md",
        "---\nid: LND-GEN\nclass: generated\nlinks:\n  uses: [PLT-UNUSED]\n---\n\n\
         # Generated\n",
    );
    write(
        &root,
        "docs/spec/lnd-old.md",
        "---\nid: LND-OLD\nclass: spec\nstatus: shipped\nscope: [lending]\n\
         links:\n  uses: [PLT-UNUSED]\n---\n\n# Old\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "definition-unused")["count"],
        json!(3),
        "{document}"
    );

    // A node outside the supply cycle feeds it: the row goes.
    let root = flow.scratch.copy(GAPS, "fed");
    replace(
        &root,
        "docs/spec/plt-unused.md",
        "reviewed: 2026-10-01\n",
        "reviewed: 2026-10-01\nlinks:\n  supplies: [PLT-FEED-A]\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "supply-unfed")["count"],
        json!(0),
        "{document}"
    );

    // So does a feed from outside the ROOT: the links are every live
    // document's, what the row lists the scope's.
    let root = flow.scratch.copy(GAPS, "fed-outside");
    replace(
        &root,
        "docs/spec/lnd-y.md",
        "reviewed: 2026-10-01\n",
        "reviewed: 2026-10-01\nlinks:\n  supplies: [PLT-FEED-A]\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "supply-unfed")["count"],
        json!(0),
        "{document}"
    );

    // A feed from no live document is none.
    let root = flow.scratch.copy(GAPS, "fed-dead");
    write(
        &root,
        "docs/spec/lnd-gen.md",
        "---\nid: LND-GEN\nclass: generated\nlinks:\n  supplies: [PLT-FEED-A]\n---\n\n\
         # Generated\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "supply-unfed")["count"],
        json!(1),
        "{document}"
    );

    // What the rows list stays the scope's: the same cycle and plates, out
    // of the lending wing's sight.
    let document = flow.json(&fixture(GAPS), &["WING-LEND"]);
    for code in ["definition-unused", "definition-once", "supply-unfed"] {
        assert_eq!(
            row(&document, code)["count"],
            json!(0),
            "{code}: {document}"
        );
    }
}

/// "Scope": no ROOT: a document holding, receiving or selected, with its
/// ancestors, and no other. Documents only: a link to `ID#SECTION` lands on
/// its document, a section ROOT on its document.
#[test]
fn the_scope_is_documents_holding_or_receiving_with_their_ancestors() {
    let flow = Flow::new("flow-scope");
    let root = flow.scratch.copy(RELAY, "relay");
    write(
        &root,
        "docs/spec/kit-lone.md",
        document("KIT-LONE", "A lone kit", ""),
    );
    write(
        &root,
        "docs/spec/kit-case.md",
        document("KIT-CASE", "The case", ""),
    );
    replace(
        &root,
        "docs/spec/kit-oil.md",
        "title: The oil\n",
        "title: The oil\nparent: KIT-CASE\n",
    );
    let document = flow.json(&root, &[]);
    let ids: BTreeSet<&str> = document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an ID"))
        .collect();
    assert!(ids.contains("KIT-CASE"), "an ancestor: {document}");
    assert!(!ids.contains("KIT-LONE"), "no link, no rule: {document}");
    assert_eq!(ids.len(), 9, "{document}");
    assert_eq!(
        node(&document, "KIT-OIL")["group"],
        "KIT-CASE",
        "{document}"
    );

    let root = flow.scratch.copy(GAPS, "section");
    replace(
        &root,
        "docs/spec/lnd-check.md",
        "    - LND-ASK\n",
        "    - LND-ASK#LND-ASK-LINE\n",
    );
    let document = flow.json(&root, &["WING-LEND"]);
    assert_eq!(
        edge(&document, "LND-ASK", "LND-CHECK", "follows")["line"],
        json!(10),
        "{document}"
    );
    assert!(
        document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .all(|node| node["id"] != "LND-ASK-LINE"),
        "{document}"
    );
    let document = flow.json(&fixture(GAPS), &["LND-ASK-LINE"]);
    assert_eq!(
        document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["id"].as_str().expect("an ID"))
            .collect::<Vec<&str>>(),
        ["LND-ASK"],
        "{document}"
    );
}

/// "Scope": Tier 3 and generated stay out, counted in `left_out` and the
/// header; a link into one is no edge.
#[test]
fn tier_3_and_generated_stay_out() {
    let flow = Flow::new("flow-left-out");
    let root = flow.scratch.copy(RELAY, "relay");
    write(
        &root,
        "docs/spec/kit-gen.md",
        "---\nid: KIT-GEN\nclass: generated\nlinks:\n  uses: [KIT-TORCH]\n---\n\n\
         # Generated\n",
    );
    write(
        &root,
        "docs/spec/kit-old.md",
        "---\nid: KIT-OLD\nclass: spec\nstatus: shipped\nscope: [kit]\n\
         links:\n  supplies: [KIT-TORCH]\n---\n\n# Old\n",
    );
    let run = flow.text(&root, &[]);
    let header = first_line(&run);
    assert!(
        header.starts_with("flow all | nodes 8, edges 10, depth -; left out: "),
        "{}",
        run.show()
    );
    assert!(
        header.contains("1 generated") && header.contains("1 archived"),
        "{header}"
    );
    let document = flow.json(&root, &[]);
    assert_eq!(document["left_out"]["generated"], json!(1), "{document}");
    assert_eq!(document["left_out"]["tier3"], json!(1), "{document}");
}

/// A read sees the working tree with no `spec index` first, and a link
/// removed leaves it, as `spec tree`.
#[test]
fn a_read_follows_the_working_tree() {
    let flow = Flow::new("flow-fresh");
    let root = flow.scratch.copy(RELAY, "relay");
    assert_eq!(
        first_line(&flow.text(&root, &[])),
        "flow all | nodes 8, edges 10, depth -"
    );
    replace(
        &root,
        "docs/spec/leg-end.md",
        "kind: alternative",
        "kind: repeat",
    );
    replace(
        &root,
        "docs/spec/kit-oil.md",
        "links:\n  supplies: [KIT-TORCH]\n",
        "",
    );
    let run = flow.text(&root, &[]);
    assert_eq!(first_line(&run), "flow all | nodes 7, edges 9, depth -");
    assert!(
        edge_lines(&run.stdout).contains(
            &"LEG-SCORE -> LEG-END | follows | x1 | kind repeat | when no round remains | \
              docs/spec/leg-end.md:9"
        ),
        "{}",
        run.show()
    );
}

// -------------------------------------------------------- settled readings

/// A document's edge to itself stays an edge while the document is shown,
/// and joins `inner` once a fold takes it.
#[test]
fn a_self_edge_is_an_edge_while_shown_and_inner_when_folded() {
    let flow = Flow::new("flow-self-fold");
    let root = fixture(GAPS);
    for args in [&["WING-LEND"][..], &["WING-LEND", "--depth", "1"]] {
        let shown = flow.json(&root, args);
        assert_eq!(
            edge(&shown, "LND-OPEN", "LND-OPEN", "follows")["kind"],
            "repeat",
            "{args:?}: {shown}"
        );
        assert_eq!(
            inner(node(&shown, "LND-OPEN")),
            typed(&[]),
            "{args:?}: not both: {shown}"
        );
        assert_eq!(node(&shown, "LND-OPEN")["holds"], json!(0), "{args:?}");
    }
    let folded = flow.json(&root, &["WING-LEND", "--depth", "0"]);
    assert_eq!(folded["edges"], json!([]), "{folded}");
    assert_eq!(
        inner(node(&folded, "WING-LEND")),
        typed(&[("follows", 8), ("uses", 2), ("depends_on", 1)]),
        "its self-edge among the eight: {folded}"
    );
}

/// An unmeasured row prints `-` for its count and JSON `null`, with no
/// items: `not_configured` and `cannot` alike, each with its cause.
#[test]
fn an_unmeasured_row_prints_a_dash_and_null() {
    let flow = Flow::new("flow-dash");
    let relay = fixture(RELAY);
    let run = flow.text(&relay, &[]);
    assert!(
        gap_lines(&run.stdout)
            .contains(&format!("gap no-start | not_configured | - | {NO_START_CAUSE}").as_str()),
        "{}",
        run.show()
    );
    let document = flow.json(&relay, &[]);
    for (code, _) in NEEDING_A_TABLE {
        assert_eq!(row(&document, code)["count"], Value::Null, "{code}");
        assert_eq!(row(&document, code)["items"], json!([]), "{code}");
    }

    let gaps = fixture(GAPS);
    let run = flow.text(&gaps, &["WING-HEADLESS"]);
    assert!(
        gap_lines(&run.stdout)[2].starts_with("gap unreachable | cannot | - | "),
        "{}",
        run.show()
    );
    let document = flow.json(&gaps, &["WING-HEADLESS"]);
    let gap = row(&document, "unreachable");
    assert_eq!(gap["count"], Value::Null, "{gap}");
    assert_eq!(gap["items"], json!([]), "{gap}");
    assert!(gap["cause"].is_string(), "{gap}");
}

/// A bad `[[flow.step]]`: the rows that walk steps, `unreachable` and
/// `no-way-to-end`, are `cannot` with its line; the rest measure on.
#[test]
fn a_bad_step_table_leaves_the_walked_rows_cannot() {
    let flow = Flow::new("flow-bad-step");
    let root = flow.copy(
        GAPS,
        "bad-step",
        "\n[[flow.step]]\nkinds = [\"lending\"]\nflavour = 1   # HERE\n",
    );
    let at = read_text(&root, "specengine.toml")
        .lines()
        .position(|line| line.ends_with("# HERE"))
        .expect("the mark")
        + 1;
    let document = flow.json(&root, &["WING-BROKEN"]);
    for code in ["unreachable", "no-way-to-end"] {
        let gap = row(&document, code);
        assert_eq!(gap["state"], "cannot", "{gap}");
        assert!(
            gap["cause"]
                .as_str()
                .expect("a cause")
                .starts_with(&format!("specengine.toml:{at}: ")),
            "{gap}"
        );
    }
    for gap in document["gaps"].as_array().expect("gaps") {
        if !["unreachable", "no-way-to-end"].contains(&gap["code"].as_str().expect("a code")) {
            assert_eq!(gap["state"], "measured", "{gap}");
        }
    }
}

/// With no marked start a start is a step no `follows` edge enters; a step's
/// edge to itself enters nothing.
#[test]
fn a_derived_start_ignores_a_self_edge() {
    let flow = Flow::new("flow-derived");
    let root = flow.scratch.copy(RELAY, "relay");
    replace(
        &root,
        "docs/spec/leg-start.md",
        "reviewed: 2026-10-01\n",
        "reviewed: 2026-10-01\nlinks:\n  follows:\n    - {to: LEG-START, kind: repeat}\n",
    );
    let document = flow.json(&root, &[]);
    assert_eq!(
        node(&document, "LEG-START")["distance"],
        json!(0),
        "{document}"
    );
    assert_eq!(
        node(&document, "LEG-START")["start"],
        json!(true),
        "{document}"
    );
    assert_eq!(
        node(&document, "LEG-END")["distance"],
        json!(4),
        "{document}"
    );
}

/// A folded node's `distance` is the least of what it holds, itself
/// included, so a screen orders the top level without reading the whole
/// flow; `null` only where nothing it holds is reached. `start` and `end`
/// stay its own. M10: a folded node takes only its own `distance`.
#[test]
fn a_folded_node_takes_the_least_distance_it_holds() {
    let flow = Flow::new("flow-folded-distance");
    let root = flow.scratch.copy(RELAY, "relay");
    replace(
        &root,
        "docs/spec/leg-run.md",
        "title: The run\n",
        "title: The run\nparent: LEG-SCORE\n",
    );
    replace(
        &root,
        "docs/spec/leg-fall.md",
        "title: The fall\n",
        "title: The fall\nparent: LEG-DRAW\n",
    );
    let run = flow.text(&root, &["--depth", "0"]);
    assert_eq!(
        walk(&run.stdout),
        pairs(&[
            ("0", "LEG-START"),
            ("1", "LEG-DRAW"),
            ("2", "LEG-SCORE"),
            ("4", "LEG-END"),
            ("-", "KIT-OIL"),
            ("-", "KIT-TORCH"),
        ]),
        "the top level ordered by what it holds: {}",
        run.show()
    );
    let document = flow.json(&root, &["--depth", "0"]);
    let score = node(&document, "LEG-SCORE");
    assert_eq!(score["holds"], json!(1), "{score}");
    assert_eq!(
        score["distance"],
        json!(2),
        "LEG-RUN's 2, below its own 3: {score}"
    );
    let draw = node(&document, "LEG-DRAW");
    assert_eq!(draw["holds"], json!(1), "{draw}");
    assert_eq!(
        draw["distance"],
        json!(1),
        "its own 1, below LEG-FALL's 3: {draw}"
    );

    let gaps = fixture(GAPS);
    let document = flow.json(&gaps, &["WING-LEND", "--depth", "0"]);
    let lend = node(&document, "WING-LEND");
    assert_eq!(lend["distance"], json!(0), "it holds LND-ASK at 0: {lend}");
    // docs/features/trace-views.md "Folded start and end" reverses this
    // slice's rule: both now fold beside `distance`.
    assert_eq!(lend["start"], json!(true), "it holds LND-ASK: {lend}");
    assert_eq!(lend["end"], json!(true), "it holds LND-OPEN: {lend}");

    let document = flow.json(&gaps, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        node(&document, "WING-HEADLESS")["distance"],
        Value::Null,
        "LND-X and LND-Y are reached by nothing: {document}"
    );
    assert_eq!(
        node(&document, "WING-LEND")["distance"],
        json!(0),
        "{document}"
    );
}

/// A `follows` that does not resolve, dangling or another project's, gives
/// no edge and drops no document: it stays a node and a step, so a typo
/// shows in `unreachable` and `no-way-to-end` rather than leaving the read.
/// M9: an unresolved `follows` drops its document.
#[test]
fn an_unresolved_follows_stays_a_node_and_a_step() {
    let flow = Flow::new("flow-unresolved");
    let root = flow.copy(RELAY, "relay", RELAY_FLOW);
    write(
        &root,
        "docs/spec/leg-lost.md",
        document(
            "LEG-LOST",
            "A lost leg",
            "links:\n  follows: [LEG-NOWHERE]\n",
        ),
    );
    write(
        &root,
        "docs/spec/leg-away.md",
        document(
            "LEG-AWAY",
            "A leg abroad",
            "links:\n  follows:\n    - other:LEG-THERE\n",
        ),
    );
    let run = flow.text(&root, &[]);
    assert_eq!(
        first_line(&run),
        "flow all | nodes 10, edges 10, depth -",
        "both stay nodes, neither gives an edge: {}",
        run.show()
    );
    assert!(
        !edge_lines(&run.stdout)
            .iter()
            .any(|line| line.contains("NOWHERE") || line.contains("THERE")),
        "{}",
        run.show()
    );
    let document = flow.json(&root, &[]);
    for id in ["LEG-LOST", "LEG-AWAY"] {
        let unreached = node(&document, id);
        assert_eq!(unreached["distance"], Value::Null, "{unreached}");
        assert_eq!(unreached["start"], json!(false), "a start is marked here");
    }
    let both = json!([["LEG-AWAY"], ["LEG-LOST"]]);
    assert_eq!(row(&document, "unreachable")["items"], both, "{document}");
    assert_eq!(row(&document, "no-way-to-end")["items"], both, "{document}");

    // Under a ROOT, with no `[[flow.step]]`: holding the `follows` is what
    // makes it a step, a labelled item as a plain one.
    let root = flow.scratch.copy(GAPS, "gaps");
    replace(
        &root,
        "specengine.toml",
        "[[flow.step]]\nkinds = [\"lending\"]\n\n",
        "",
    );
    replace(
        &root,
        "docs/spec/lnd-stray.md",
        "reviewed: 2026-10-01\n",
        "reviewed: 2026-10-01\nlinks:\n  follows:\n    - {to: LND-NOWHERE, kind: repeat}\n",
    );
    let document = flow.json(&root, &["WING-BROKEN"]);
    assert_eq!(
        row(&document, "unreachable")["items"],
        json!([["LND-STRAY"]]),
        "{document}"
    );
    assert_eq!(
        row(&document, "no-way-to-end")["items"],
        json!([["LND-DEAD"], ["LND-STRAY"]]),
        "{document}"
    );

    // A left-out document stays out: an unresolved `follows` brings back no
    // Tier 3 or generated file.
    let root = flow.copy(RELAY, "left-out", RELAY_FLOW);
    write(
        &root,
        "docs/spec/kit-old.md",
        "---\nid: KIT-OLD\nclass: spec\nstatus: shipped\nscope: [kit]\n\
         links:\n  follows: [KIT-NOWHERE]\n---\n\n# Old\n",
    );
    write(
        &root,
        "docs/spec/kit-gen.md",
        "---\nid: KIT-GEN\nclass: generated\nlinks:\n  follows: [KIT-NOWHERE]\n---\n\n\
         # Generated\n",
    );
    let document = flow.json(&root, &[]);
    assert_eq!(
        document["nodes"].as_array().expect("nodes").len(),
        8,
        "{document}"
    );
    assert_eq!(
        row(&document, "unreachable")["count"],
        json!(0),
        "{document}"
    );
}

/// With no marked start, a step whose only `follows` does not resolve has
/// nothing entering it: a derived start, at 0. M9.
#[test]
fn an_unresolved_follows_alone_is_a_derived_start() {
    let flow = Flow::new("flow-unresolved-start");
    let root = flow.scratch.copy(RELAY, "relay");
    write(
        &root,
        "docs/spec/leg-lost.md",
        document(
            "LEG-LOST",
            "A lost leg",
            "links:\n  follows: [LEG-NOWHERE]\n",
        ),
    );
    write(
        &root,
        "docs/spec/leg-away.md",
        document(
            "LEG-AWAY",
            "A leg abroad",
            "links:\n  follows:\n    - other:LEG-THERE\n",
        ),
    );
    let document = flow.json(&root, &[]);
    for id in ["LEG-LOST", "LEG-AWAY", "LEG-START"] {
        let start = node(&document, id);
        assert_eq!(start["distance"], json!(0), "{id}: {start}");
        assert_eq!(start["start"], json!(true), "{id}: {start}");
    }
    assert_eq!(
        node(&document, "LEG-END")["distance"],
        json!(4),
        "{document}"
    );
}

/// One `parallel` branch lists its split, however well the branch goes on:
/// a parallel of one is the row's first case.
#[test]
fn a_one_branch_split_stays_listed() {
    let flow = Flow::new("flow-one-branch");
    let root = flow.scratch.copy(GAPS, "gaps");
    replace(
        &root,
        "docs/spec/lnd-note.md",
        "    - {to: LND-CHECK, kind: parallel}\n",
        "    - LND-CHECK\n",
    );
    let run = flow.text(&root, &["WING-LEND"]);
    assert!(
        gap_lines(&run.stdout).contains(
            &format!("gap unjoined-parallel | measured | 1 | LND-CHECK | {UNJOINED_PARALLEL}")
                .as_str()
        ),
        "{}",
        run.show()
    );
}

/// A depth counts documents only: a section is no level, even the one a
/// document's `parent:` names (`spec tree` counts it: one deeper).
#[test]
fn a_depth_counts_documents_only() {
    let flow = Flow::new("flow-depth-documents");
    let root = flow.scratch.copy(GAPS, "gaps");
    write(
        &root,
        "docs/spec/lnd-sub.md",
        "---\nid: LND-SUB\nclass: canon\ntitle: A side desk\nparent: LND-ASK-LINE\n\
         owner: owner\nreviewed: 2026-10-01\n---\n\n# A side desk\n\nA side desk.\n",
    );
    let tree = flow.other(&root, &["--json", "tree", "WING-LEND"]);
    tree.code(0);
    let tree = tree.json();
    let depth_of = |id: &str| {
        tree["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("no {id}: {tree}"))["depth"]
            .clone()
    };
    assert_eq!(depth_of("LND-SUB"), json!(3), "the tree counts the section");

    let two = flow.json(&root, &["WING-LEND", "--depth", "2"]);
    assert_eq!(node(&two, "LND-SUB")["group"], "LND-ASK", "{two}");
    assert_eq!(node(&two, "LND-ASK")["holds"], json!(0), "{two}");
    let one = flow.json(&root, &["WING-LEND", "--depth", "1"]);
    assert!(
        one["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .all(|node| node["id"] != "LND-SUB"),
        "{one}"
    );
    assert_eq!(node(&one, "LND-ASK")["holds"], json!(1), "{one}");
}

// --------------------------------------------------------------- Edge data

/// Edge data: a broken front-matter, a dangling target, a dangling parent:
/// exit 0, the links that resolve, and no panic.
#[test]
fn broken_documents_cost_only_their_links() {
    let flow = Flow::new("flow-broken-doc");
    let root = flow.scratch.copy(RELAY, "relay");
    write(
        &root,
        "docs/spec/kit-bad.md",
        "---\nid: [unclosed\n---\n\n# Bad\n",
    );
    write(
        &root,
        "docs/spec/kit-lost.md",
        "---\nid: KIT-LOST\nclass: canon\ntitle: Lost\nparent: KIT-NOWHERE\nowner: owner\n\
         reviewed: 2026-10-01\nlinks:\n  uses: [KIT-TORCH, KIT-GONE]\n---\n\n# Lost\n",
    );
    let run = flow.text(&root, &[]);
    assert_eq!(
        first_line(&run),
        "flow all | nodes 9, edges 11, depth -",
        "{}",
        run.show()
    );
    assert!(
        edge_lines(&run.stdout)
            .contains(&"KIT-LOST -> KIT-TORCH | uses | x1 | docs/spec/kit-lost.md:9"),
        "{}",
        run.show()
    );
}

/// Edge data: a `parent:` cycle ends the walk (the deadline kills a hang),
/// exit 0, each document once.
#[test]
fn a_parent_cycle_ends() {
    let flow = Flow::new("flow-parent-cycle");
    let root = flow.scratch.copy(RELAY, "relay");
    replace(
        &root,
        "docs/spec/kit-oil.md",
        "title: The oil\n",
        "title: The oil\nparent: KIT-TORCH\n",
    );
    replace(
        &root,
        "docs/spec/kit-torch.md",
        "title: The torch\n",
        "title: The torch\nparent: KIT-OIL\n",
    );
    for args in [
        &[][..],
        &["KIT-OIL"],
        &["--depth", "0"],
        &["KIT-OIL", "--depth", "1"],
    ] {
        let document = flow.json(&root, args);
        let ids: Vec<&str> = document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["id"].as_str().expect("an ID"))
            .collect();
        let unique: BTreeSet<&&str> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "{args:?}: each once: {document}");
    }
}

/// Edge data: two documents holding one ID: the read answers, and a ROOT
/// naming it takes both holders ("Scope": "ROOT's holders").
#[test]
fn two_holders_of_one_id_both_stand() {
    let flow = Flow::new("flow-dup");
    let root = flow.scratch.copy(GAPS, "gaps");
    write(
        &root,
        "docs/spec/other.md",
        "---\nid: WING-HEADLESS\nclass: canon\ntitle: Another back room\nowner: owner\n\
         reviewed: 2026-10-01\n---\n\n# Another back room\n",
    );
    let document = flow.json(&root, &["WING-HEADLESS"]);
    let paths: BTreeSet<&str> = document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["path"].as_str().expect("a path"))
        .collect();
    assert!(paths.contains("docs/spec/other.md"), "{document}");
    assert!(paths.contains("docs/spec/wing-headless.md"), "{document}");
}

// ------------------------------------------------- flow-canvas.md, AC-14

/// `gaps` of each shown node, by name.
fn gaps_by_node(document: &Value) -> BTreeMap<&str, u64> {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            let id = node["id"].as_str().expect("an ID");
            (id, node["gaps"].as_u64().expect("a count"))
        })
        .collect()
}

/// The sum of `gaps` over a read's nodes.
fn gaps_sum(document: &Value) -> u64 {
    gaps_by_node(document).values().sum()
}

/// `(name, gaps)` pairs as a map, for comparing with [`gaps_by_node`].
fn gaps_of<const N: usize>(pairs: [(&str, u64); N]) -> BTreeMap<&str, u64> {
    BTreeMap::from(pairs)
}

/// How many times the rows name each document: `no-start`'s scope is no
/// document, so that row is left out.
fn naming_rows(document: &Value) -> BTreeMap<String, usize> {
    let mut named = BTreeMap::new();
    for gap in document["gaps"].as_array().expect("gaps") {
        if gap["code"] == "no-start" {
            continue;
        }
        for item in gap["items"].as_array().expect("items") {
            for member in item.as_array().expect("members") {
                let member = member.as_str().expect("a name").to_owned();
                *named.entry(member).or_default() += 1;
            }
        }
    }
    named
}

/// AC-14: `gaps` 1, 12, 2 on the three wings, 0 on the library, and a
/// document two rows name counted once. M8.
#[test]
fn ac14_each_wing_counts_its_documents_once() {
    let flow = Flow::new("flow-count");
    let root = fixture(GAPS);
    let args = ["WING-LIBRARY", "--depth", "1"];
    let document = flow.json(&root, &args);
    assert_eq!(
        gaps_by_node(&document),
        gaps_of([
            ("WING-BROKEN", 12),
            ("WING-HEADLESS", 2),
            ("WING-LEND", 1),
            ("WING-LIBRARY", 0),
        ]),
        "{document}"
    );

    // The rows name 15 documents 21 times, six of them twice, so counting
    // per row cannot give the 15.
    let named = naming_rows(&document);
    assert_eq!(named.len(), 15, "{named:?}");
    assert_eq!(named.values().sum::<usize>(), 21, "{named:?}");
    let twice: Vec<&str> = named
        .iter()
        .filter(|(_, rows)| **rows == 2)
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        twice,
        [
            "LND-B",
            "LND-STRAY",
            "LND-X",
            "LND-Y",
            "PLT-FEED-A",
            "PLT-FEED-B"
        ]
    );

    // A repeat is the same document, byte for byte.
    let mut with_json = args.to_vec();
    with_json.push("--json");
    assert_eq!(
        bytes_of(&flow.run(&root, &with_json)),
        bytes_of(&flow.run(&root, &with_json))
    );
}

/// AC-14: unfolded, a node is 1 where the rows name it, however many, and 0
/// where none does; every node, not only the planted.
#[test]
fn ac14_unfolded_a_named_document_is_one() {
    let flow = Flow::new("flow-one");
    let root = fixture(GAPS);
    let document = flow.json(&root, &["WING-LIBRARY"]);
    let named = naming_rows(&document);
    let served = gaps_by_node(&document);
    assert_eq!(served.len(), 29, "{document}");
    for (id, gaps) in served {
        assert_eq!(gaps, u64::from(named.contains_key(id)), "{id}: {document}");
    }
}

/// AC-14: the sum over a read's nodes is 15 at depth 0, 1, 2 and unbounded,
/// with a ROOT and without: rows before any fold, so no depth moves it.
#[test]
fn ac14_no_depth_moves_the_sum() {
    let flow = Flow::new("flow-sum");
    let root = fixture(GAPS);
    for args in [
        &["WING-LIBRARY", "--depth", "0"][..],
        &["WING-LIBRARY", "--depth", "1"],
        &["WING-LIBRARY", "--depth", "2"],
        &["WING-LIBRARY"],
        &["--depth", "0"],
        &["--depth", "1"],
        &["--depth", "2"],
        &[],
    ] {
        let document = flow.json(&root, args);
        assert_eq!(gaps_sum(&document), 15, "{args:?}: {document}");
    }
    let top = flow.json(&root, &["WING-LIBRARY", "--depth", "0"]);
    assert_eq!(gaps_by_node(&top), gaps_of([("WING-LIBRARY", 15)]), "{top}");
}

/// AC-14: each read counts by its own rows: WING-LEND read alone has no
/// gap, though the library's read counts `LND-ASK` a second start; a read
/// of WING-BROKEN keeps its 12.
#[test]
fn ac14_each_read_counts_by_its_own_rows() {
    let flow = Flow::new("flow-own");
    let root = fixture(GAPS);
    let lend = flow.json(&root, &["WING-LEND", "--depth", "0"]);
    assert_eq!(gaps_by_node(&lend), gaps_of([("WING-LEND", 0)]), "{lend}");
    let broken = flow.json(&root, &["WING-BROKEN", "--depth", "0"]);
    assert_eq!(
        gaps_by_node(&broken),
        gaps_of([("WING-BROKEN", 12)]),
        "{broken}"
    );
}

/// AC-14: `no-start`'s scope is no document: WING-HEADLESS names itself
/// there, and stays 0 at unbounded depth; folded it holds its two steps,
/// not three.
#[test]
fn ac14_the_no_start_scope_counts_as_no_document() {
    let flow = Flow::new("flow-scope-row");
    let root = fixture(GAPS);
    let document = flow.json(&root, &["WING-HEADLESS"]);
    assert_eq!(
        row(&document, "no-start")["items"],
        json!([["WING-HEADLESS"]]),
        "{document}"
    );
    assert_eq!(
        gaps_by_node(&document),
        gaps_of([("LND-X", 1), ("LND-Y", 1), ("WING-HEADLESS", 0)]),
        "{document}"
    );
    let folded = flow.json(&root, &["WING-HEADLESS", "--depth", "0"]);
    assert_eq!(
        gaps_by_node(&folded),
        gaps_of([("WING-HEADLESS", 2)]),
        "{folded}"
    );
}

/// AC-14: a text node line ends ` | gaps <g>` above 0 and holds no such
/// field at 0: the depth-1 read whole, then every line of every depth
/// against its document.
#[test]
fn ac14_a_text_line_ends_gaps_above_zero() {
    let flow = Flow::new("flow-text");
    let root = fixture(GAPS);
    let run = flow.text(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        node_lines(&run.stdout),
        [
            "0 WING-BROKEN | wing | The closed wing | docs/spec/wing-broken.md:1 | start | end \
             | group WING-LIBRARY | holds 16, inner 17 | gaps 12",
            "0 WING-LEND | wing | The lending wing | docs/spec/wing-lend.md:1 | start | end \
             | group WING-LIBRARY | holds 7, inner 11 | gaps 1",
            "- WING-HEADLESS | wing | The back room | docs/spec/wing-headless.md:1 | group \
             WING-LIBRARY | holds 2, inner 1 | gaps 2",
            "- WING-LIBRARY | wing | The public library | docs/spec/wing-library.md:1",
        ],
        "{}",
        run.show()
    );
    let headless = flow.text(&root, &["WING-HEADLESS"]);
    assert_eq!(
        node_lines(&headless.stdout),
        [
            "0 LND-Y | lending | A key with no lock | docs/spec/lnd-y.md:1 | start | group \
             WING-HEADLESS | gaps 1",
            "1 LND-X | lending | A lock with one key | docs/spec/lnd-x.md:1 | group \
             WING-HEADLESS | gaps 1",
            "- WING-HEADLESS | wing | The back room | docs/spec/wing-headless.md:1",
        ],
        "{}",
        headless.show()
    );

    for args in [
        &["WING-LIBRARY", "--depth", "0"][..],
        &["WING-LIBRARY", "--depth", "1"],
        &["WING-LIBRARY"],
        &["WING-LEND"],
    ] {
        let run = flow.text(&root, args);
        let document = flow.json(&root, args);
        let nodes = document["nodes"].as_array().expect("nodes");
        let lines = node_lines(&run.stdout);
        assert_eq!(lines.len(), nodes.len(), "{args:?}: {}", run.show());
        for (line, node) in lines.iter().zip(nodes) {
            let gaps = node["gaps"].as_u64().expect("a count");
            assert!(head(line).ends_with(node["id"].as_str().expect("an ID")));
            let tail = line.rsplit(" | ").next().expect("a field");
            if gaps > 0 {
                assert_eq!(tail, format!("gaps {gaps}"), "{args:?}: {line}");
            } else {
                assert!(!line.contains(" | gaps "), "{args:?}: {line}");
            }
        }
    }
}

/// AC-14 and "Data": `gaps` reads the measured rows, before any cap: 59
/// documents in a row printing 50.
#[test]
fn ac14_the_count_reads_past_a_rows_item_cap() {
    let flow = Flow::new("flow-count-cap");
    let table = "\n[[flow.step]]\nkinds = [\"pace\"]\n\n\
                 [[flow.start]]\npaths = [\"docs/spec/pace-0001.md\"]\n\n\
                 [[flow.end]]\npaths = [\"docs/spec/pace-0001.md\"]\n";
    let root = generated(&flow.scratch, "items", table, 60, "A step", |step| {
        if step == 1 {
            String::new()
        } else {
            "parent: PACE-0001\n".to_owned()
        }
    });
    let folded = flow.json(&root, &["--depth", "0"]);
    assert_eq!(row(&folded, "unreachable")["count"], json!(59));
    assert_eq!(
        row(&folded, "unreachable")["items"]
            .as_array()
            .expect("items")
            .len(),
        50,
        "the row prints 50"
    );
    assert_eq!(
        gaps_by_node(&folded),
        gaps_of([("PACE-0001", 59)]),
        "{folded}"
    );
    assert_eq!(gaps_sum(&flow.json(&root, &[])), 59);
}

/// AC-14: a row's member outside the read is held by no node: a supply
/// cycle crossing the scope names both, the read counts the one it holds.
#[test]
fn ac14_a_member_outside_the_read_is_held_by_no_node() {
    let flow = Flow::new("flow-outside");
    let root = flow.scratch.copy(GAPS, "gaps");
    replace(
        &root,
        "docs/spec/plt-feed-b.md",
        "parent: WING-BROKEN\n",
        "parent: WING-LEND\n",
    );
    let document = flow.json(&root, &["WING-BROKEN", "--depth", "0"]);
    assert_eq!(
        row(&document, "supply-unfed")["items"],
        json!([["PLT-FEED-A", "PLT-FEED-B"]]),
        "{document}"
    );
    assert_eq!(
        gaps_by_node(&document),
        gaps_of([("WING-BROKEN", 11)]),
        "{document}"
    );
}

/// AC-14: two holders of one ID are two documents, so the count goes by
/// file and not by name.
#[test]
fn ac14_two_holders_of_one_id_are_two_documents() {
    let flow = Flow::new("flow-count-dup");
    let root = flow.scratch.copy(GAPS, "gaps");
    write(
        &root,
        "docs/spec/lnd-stray-too.md",
        "---\nid: LND-STRAY\nclass: canon\ntitle: A second stray\nparent: WING-BROKEN\n\
         owner: owner\nreviewed: 2026-10-01\n---\n\n# A second stray\n\nA step.\n",
    );
    let document = flow.json(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        gaps_by_node(&document)["WING-BROKEN"],
        13,
        "12 and the second holder: {document}"
    );
}

/// AC-14: a row not measured names nothing: a bad `[[flow.definition]]`
/// takes the two definition rows out of the count; the two documents
/// `supply-unfed` also names stay.
#[test]
fn ac14_an_unmeasured_row_names_no_document() {
    let flow = Flow::new("flow-count-bad");
    let root = flow.copy(GAPS, "gaps", "\n[[flow.definition]]\n");
    let document = flow.json(&root, &["WING-LIBRARY", "--depth", "1"]);
    assert_eq!(row(&document, "definition-unused")["state"], "cannot");
    assert_eq!(row(&document, "definition-once")["state"], "cannot");
    assert_eq!(
        gaps_by_node(&document),
        gaps_of([
            ("WING-BROKEN", 10),
            ("WING-HEADLESS", 2),
            ("WING-LEND", 1),
            ("WING-LIBRARY", 0),
        ]),
        "{document}"
    );
}

/// AC-14 on another project (ADR-0008): flow-relay, its own kinds, with one
/// definition nothing uses: a game's document counts as a library's does.
#[test]
fn ac14_the_count_holds_on_another_project() {
    let flow = Flow::new("flow-count-relay");
    let extra = format!("{RELAY_FLOW}\n[[flow.definition]]\npaths = [\"docs/spec/kit-oil.md\"]\n");
    let root = flow.copy(RELAY, "relay", &extra);
    let document = flow.json(&root, &[]);
    assert_eq!(row(&document, "definition-unused")["count"], json!(1));
    let named = naming_rows(&document);
    assert_eq!(gaps_sum(&document), named.len() as u64, "{document}");
    assert_eq!(node(&document, "KIT-OIL")["gaps"], json!(1), "{document}");

    // No `[flow]` at all: three rows still measure; the rest name nothing.
    let plain = flow.json(&fixture(RELAY), &[]);
    assert_eq!(
        gaps_sum(&plain),
        naming_rows(&plain).len() as u64,
        "{plain}"
    );
}
