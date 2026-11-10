//! docs/features/doc-modal.md (RM-0095, with RM-0101), the CLI half of the
//! Rust lane: the `spec` binary over scratch copies of `fixtures/spec-a` (a
//! game), `fixtures/spec-b` (a command line, Cyrillic) and
//! `fixtures/flow-aside` (a pottery workshop), and over `fixtures/flow-gaps`
//! (a library's lending), each in its own scratch `HOME`; the fixtures are
//! only read.
//!
//! - DM-1: every link of every node a fixture lists (`spec tree --json`,
//!   `flow-aside` under `ATL-POTTERY`, and every node of its parse) carries
//!   `title` and `end_kind` right after `name`, in every link, plain ones
//!   too; a resolved link's are the first node of `spec show <name>
//!   --json`, the other end's, outgoing and incoming alike; any other link's
//!   are both `null`. `POT-BISQUE`'s links are the spec's; spec-b's dangling
//!   mention at `docs/spec/cli.md:25` has both `null`.
//! - DM-3: a gap row's `titles` has `items`' shape, each title `spec show
//!   <member> --json`'s; the `two-starts` rows of `flow-aside` and
//!   `flow-gaps` as the spec gives them.
//! - DM-1 and DM-3 "the text is byte-equal": [`GOLDEN`], every read above
//!   recorded from the binary before the slice (3d3180e, by
//!   `record_the_answers_before_doc_modal`, `#[ignore]`, by hand): its text
//!   and stderr byte-equal now, its JSON byte-equal once the three keys are
//!   taken out ([`common::keys::without_doc_modal_keys`]).
//!
//! The twins are MCP `mcp_read.rs` (DM-2 `get_node`), `mcp_flow.rs` (DM-3
//! `get_flow`) and http `flow.rs` (both routes); the key sets
//! `daemon_keys.rs`; the cut at 50 CLI `flow_read.rs`; the subset's parity
//! and counters (DM-4) the CLI's `store_reads_tests.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use common::keys::without_doc_modal_keys;
use common::{Run, SPEC, Scratch, program_with, read_text};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_store::{WorkingTree, b3_hash, check_input};

/// The golden of the reads before the slice, beside this file.
const GOLDEN: &str = "doc_modal_before.json";

/// The commit the golden was recorded from: the slice's spec, before any
/// `src/` change of it.
const RECORDED_WITH: &str = "spec at 3d3180e (doc-modal's spec commit, before any src change of \
                             the slice)";

/// A link's keys, in DM-1's order; `kind` and `when` follow when
/// labelled.
const LINK_KEYS: [&str; 11] = [
    "type", "origin", "at", "name", "title", "end_kind", "written", "path", "line", "state",
    "reason",
];

/// The labelled edge's two, after [`LINK_KEYS`] when present.
const LABEL_KEYS: [&str; 2] = ["kind", "when"];

/// The fixtures of DM-1, each with the ROOT its tree is listed under.
const LINKED: [(&str, Option<&str>); 3] = [
    ("spec-a", None),
    ("spec-b", None),
    ("flow-aside", Some("ATL-POTTERY")),
];

/// The flow reads of DM-3, by fixture.
const FLOWS: [(&str, &[&str]); 6] = [
    ("flow-aside", &["ATL-POTTERY", "--depth", "1", "--in-flow"]),
    ("flow-aside", &["ATL-POTTERY", "--depth", "1"]),
    ("flow-aside", &[]),
    ("flow-gaps", &[]),
    ("flow-gaps", &["WING-BROKEN"]),
    ("flow-gaps", &["WING-LIBRARY", "--depth", "1"]),
];

const CAUSE_TWO_STARTS: &str = "more than one start: narrow [[flow.start]] to one";

// ------------------------------------------------------------------ helpers

/// A scratch copy of a fixture, its `HOME`, a working directory outside it.
struct Copy {
    _scratch: Scratch,
    home: PathBuf,
    cwd: PathBuf,
    root: PathBuf,
}

impl Copy {
    fn new(fixture: &str) -> Self {
        let scratch = Scratch::new("doc-modal");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let root = scratch.copy(fixture, "copy");
        Self {
            _scratch: scratch,
            home,
            cwd,
            root,
        }
    }

    /// `program <args> --root <copy>`.
    fn run_with(&self, program: &OsStr, args: &[&str]) -> Run {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--root", self.root.to_str().expect("UTF-8 root")]);
        program_with(program, &self.cwd, &all, &[("HOME", self.home.as_os_str())])
    }

    /// The binary under test.
    fn run(&self, args: &[&str]) -> Run {
        self.run_with(OsStr::new(SPEC), args)
    }

    /// `args --json`: exit 0, one document.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.run(&all);
        run.code(0);
        run.json()
    }

    /// The scratch's paths, longest first, as placeholders.
    fn placeholders(&self) -> Vec<(String, &'static str)> {
        let text = |path: &Path| path.to_str().expect("UTF-8 path").to_owned();
        let mut out = vec![
            (text(&self.root), "<root>"),
            (text(&self.home), "<home>"),
            (text(&self.cwd), "<cwd>"),
        ];
        out.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
        out
    }
}

/// A node's name: its ID, else its path.
fn name(node: &Value) -> String {
    node["id"]
        .as_str()
        .or_else(|| node["path"].as_str())
        .unwrap_or_else(|| panic!("a node with neither ID nor path: {node}"))
        .to_owned()
}

/// The names `spec tree [ROOT] --json` lists, in its order, by `program`.
fn listed(copy: &Copy, program: &OsStr, root: Option<&str>) -> Vec<String> {
    let mut args = vec!["tree"];
    args.extend(root);
    args.push("--json");
    let run = copy.run_with(program, &args);
    run.code(0);
    let document = run.json();
    let nodes = document["nodes"].as_array().expect("nodes");
    assert!(
        !nodes.is_empty(),
        "premise: the tree lists nodes: {document}"
    );
    nodes.iter().map(name).collect()
}

/// Every node name of the copy's parse, outside Tier 3, in file order.
fn parsed(root: &Path) -> Vec<String> {
    let text = read_text(root, "specengine.toml");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let tree = WorkingTree::new(root, &config.paths).expect("the working tree");
    let input = check_input(&tree, &config.scheme);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let mut out = Vec::new();
    for file in 0..graph.paths().len() {
        if graph.is_tier3(file) {
            continue;
        }
        for ord in 0..graph.nodes(file).len() {
            out.push(graph.name(NodeAt { file, ord }));
        }
    }
    out
}

/// DM-1's names of a fixture: the tree's, then the parse's not listed,
/// each once; the second value is the tree's alone.
fn names_of(copy: &Copy, program: &OsStr, root: Option<&str>) -> (Vec<String>, BTreeSet<String>) {
    let tree = listed(copy, program, root);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for name in tree.iter().cloned().chain(parsed(&copy.root)) {
        if seen.insert(name.clone()) {
            out.push(name);
        }
    }
    (out, tree.into_iter().collect())
}

/// Every link of a `show --links` document, outgoing then incoming, node by
/// node.
fn links_of(document: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    for node in document["nodes"].as_array().expect("nodes") {
        let links = &node["links"];
        for way in ["outgoing", "incoming"] {
            out.extend(
                links[way]
                    .as_array()
                    .unwrap_or_else(|| panic!("no `{way}`: {node}")),
            );
        }
    }
    out
}

/// `link` written back as the CLI writes it if its keys stand in DM-1's
/// order: [`LINK_KEYS`], then [`LABEL_KEYS`] present. Found in the stdout
/// byte for byte, it proves the order.
fn in_data_order(link: &Value) -> String {
    let object = link.as_object().expect("a link object");
    let mut parts = Vec::new();
    for key in LINK_KEYS
        .iter()
        .chain(&LABEL_KEYS)
        .filter(|key| object.contains_key(**key))
    {
        parts.push(format!(
            "{}:{}",
            serde_json::to_string(key).expect("a key"),
            serde_json::to_string(&object[*key]).expect("a value")
        ));
    }
    format!("{{{}}}", parts.join(","))
}

/// The first node of `spec show <name> --json`: `(title, kind)`.
fn first_node(
    copy: &Copy,
    cache: &mut BTreeMap<String, (Value, Value)>,
    name: &str,
) -> (Value, Value) {
    if let Some(found) = cache.get(name) {
        return found.clone();
    }
    let document = copy.json(&["show", name]);
    let node = &document["nodes"][0];
    assert!(node.is_object(), "spec show {name}: no node: {document}");
    let found = (node["title"].clone(), node["kind"].clone());
    cache.insert(name.to_owned(), found.clone());
    found
}

/// The row of `code` in a flow document.
fn gap<'a>(document: &'a Value, code: &str) -> &'a Value {
    document["gaps"]
        .as_array()
        .expect("gaps")
        .iter()
        .find(|gap| gap["code"] == code)
        .unwrap_or_else(|| panic!("no row {code}: {document}"))
}

/// The lengths of a list of lists, `None` for anything else.
fn shape(value: &Value) -> Option<Vec<usize>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_array().map(Vec::len))
        .collect()
}

// ------------------------------------------------------------------ DM-1

/// DM-1 over one fixture: every link of every listed node, its keys in
/// DM-1's order and its end's title and kind. Returns `(resolved, other)`
/// links checked.
fn every_link_carries_its_end(fixture: &str, root: Option<&str>) -> (usize, usize) {
    let copy = Copy::new(fixture);
    let (names, tree) = names_of(&copy, OsStr::new(SPEC), root);
    let mut cache = BTreeMap::new();
    let (mut resolved, mut other, mut shown) = (0usize, 0usize, 0usize);
    let mut failed: Vec<String> = Vec::new();
    for name in &names {
        let run = copy.run(&["show", name, "--links", "--json"]);
        if run.code != 0 {
            assert!(
                !tree.contains(name),
                "{fixture}: a listed node shows: {}",
                run.show()
            );
            continue;
        }
        shown += 1;
        let document = run.json();
        for link in links_of(&document) {
            let object = link.as_object().expect("a link object");
            let keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();
            let required: BTreeSet<&str> = LINK_KEYS.into_iter().collect();
            let allowed: BTreeSet<&str> = LINK_KEYS.iter().chain(&LABEL_KEYS).copied().collect();
            if !required.is_subset(&keys) || !keys.is_subset(&allowed) {
                failed.push(format!("{name}: keys {keys:?}: {link}"));
                continue;
            }
            let written = in_data_order(link);
            if !run.stdout.contains(&written) {
                failed.push(format!("{name}: not in Data's order: {written}"));
            }
            if link["state"] == "resolved" {
                resolved += 1;
                let end = link["name"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name}: a resolved link names its end: {link}"));
                let (title, kind) = first_node(&copy, &mut cache, end);
                if link["title"] != title || link["end_kind"] != kind {
                    failed.push(format!(
                        "{name}: {end} is ({title}, {kind}), the link says ({}, {}): {link}",
                        link["title"], link["end_kind"]
                    ));
                }
            } else {
                other += 1;
                if !link["title"].is_null() || !link["end_kind"].is_null() {
                    failed.push(format!("{name}: an unresolved link titled: {link}"));
                }
            }
        }
    }
    assert!(
        shown >= tree.len(),
        "{fixture}: premise: every listed node shown"
    );
    assert!(resolved > 0, "{fixture}: premise: resolved links checked");
    assert!(
        failed.is_empty(),
        "{fixture}: {} of {} links:\n{}",
        failed.len(),
        resolved + other,
        failed.join("\n")
    );
    (resolved, other)
}

/// DM-1 on spec-a. M: `title` from the shown node (`here`); `end_kind` from
/// the edge label; an incoming link titled by its target.
#[test]
fn dm1_every_link_of_spec_a_carries_its_end() {
    every_link_carries_its_end("spec-a", None);
}

/// DM-1 on spec-b, whose dangling mention is the unresolved case. M:
/// `skip_serializing_if` on `title` (the dangling link loses its key).
#[test]
fn dm1_every_link_of_spec_b_carries_its_end() {
    let (_, other) = every_link_carries_its_end("spec-b", None);
    assert!(other > 0, "premise: an unresolved link checked");
}

/// DM-1 on flow-aside under `ATL-POTTERY`: 182 nodes, records and ID-less
/// notes among them.
#[test]
fn dm1_every_link_of_flow_aside_carries_its_end() {
    every_link_carries_its_end("flow-aside", Some("ATL-POTTERY"));
}

/// DM-1, the spec's example: `POT-BISQUE`'s five links, its first outgoing and first
/// incoming byte for byte as the spec prints them; the text block is the one
/// before the slice, no title in it; a second read is the same bytes.
#[test]
fn dm1_pot_bisque_links_are_datas() {
    let copy = Copy::new("flow-aside");
    let run = copy.run(&["show", "POT-BISQUE", "--links", "--json"]);
    run.code(0);
    assert!(
        run.stdout.contains(
            "{\"type\":\"follows\",\"origin\":\"frontmatter\",\"at\":null,\"name\":\"POT-DRY\",\
             \"title\":\"Drying on the racks\",\"end_kind\":\"potstep\",\"written\":\"POT-DRY\",\
             \"path\":\"docs/workshop/run/bisque.md\",\"line\":10,\"state\":\"resolved\",\
             \"reason\":null}"
        ),
        "{}",
        run.show()
    );
    assert!(
        run.stdout.contains(
            "{\"type\":\"depends_on\",\"origin\":\"frontmatter\",\"at\":null,\
             \"name\":\"PUZZLE-0003\",\"title\":\"A puzzle of a cone that bends late, the \
             first\",\"end_kind\":\"puzzle\",\"written\":\"POT-BISQUE\",\
             \"path\":\"docs/puzzles/PUZZLE-0003.md\",\"line\":9,\"state\":\"resolved\",\
             \"reason\":null}"
        ),
        "{}",
        run.show()
    );
    let document = run.json();
    let links = &document["nodes"][0]["links"];
    let ends = |way: &str| -> Vec<(String, String, Value, Value)> {
        links[way]
            .as_array()
            .expect("links")
            .iter()
            .map(|link| {
                (
                    link["type"].as_str().expect("a type").to_owned(),
                    link["name"].as_str().expect("a name").to_owned(),
                    link["title"].clone(),
                    link["end_kind"].clone(),
                )
            })
            .collect()
    };
    let end = |kind: &str, name: &str, title: &str, end_kind: &str| {
        (
            kind.to_owned(),
            name.to_owned(),
            json!(title),
            json!(end_kind),
        )
    };
    assert_eq!(
        ends("outgoing"),
        [
            end("follows", "POT-DRY", "Drying on the racks", "potstep"),
            end("uses", "WBAY-KILN", "Kiln bay", "workbay"),
            end("uses", "CONE-BISQUE", "Bisque cone", "conerule"),
        ]
    );
    assert_eq!(
        ends("incoming"),
        [
            end(
                "depends_on",
                "PUZZLE-0003",
                "A puzzle of a cone that bends late, the first",
                "puzzle"
            ),
            end("follows", "POT-GLAZE", "Dipping in glaze", "potstep"),
        ]
    );
    let again = copy.run(&["show", "POT-BISQUE", "--links", "--json"]);
    assert_eq!(again.stdout, run.stdout, "a repeat is the same bytes");

    let text = copy.run(&["show", "POT-BISQUE", "--links"]);
    text.code(0);
    let block: Vec<&str> = text
        .stdout
        .lines()
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .collect();
    assert_eq!(
        block,
        [
            "  out follows POT-DRY | docs/workshop/run/bisque.md:10",
            "  out uses WBAY-KILN | docs/workshop/run/bisque.md:11",
            "  out uses CONE-BISQUE | docs/workshop/run/bisque.md:11",
            "  in depends_on PUZZLE-0003 | docs/puzzles/PUZZLE-0003.md:9",
            "  in follows POT-GLAZE | docs/workshop/run/glaze.md:11",
            "  links 3 out, 2 in",
        ],
        "{}",
        text.show()
    );
    // The header names the node's own kind, `potstep`; no end's title.
    for title in ["Drying on the racks", "Kiln bay", "Dipping in glaze"] {
        assert!(
            !text.stdout.contains(title),
            "the text names no end: {title}"
        );
    }
}

/// DM-1: spec-b's dangling mention at `docs/spec/cli.md:25` (`R\u{0415}Q-003`, a
/// mixed-script ID) has `title` and `end_kind` both `null`, present.
#[test]
fn dm1_the_dangling_mention_has_both_null() {
    let copy = Copy::new("spec-b");
    let run = copy.run(&["show", "MOD-CLI", "--links", "--json"]);
    run.code(0);
    let document = run.json();
    let dangling: Vec<&Value> = links_of(&document)
        .into_iter()
        .filter(|link| link["path"] == "docs/spec/cli.md" && link["line"] == 25)
        .filter(|link| link["state"] != "resolved")
        .collect();
    assert_eq!(dangling.len(), 1, "{document}");
    let link = dangling[0];
    assert_eq!(link["written"], json!("R\u{0415}Q-003"), "{link}");
    assert_eq!(link["state"], json!("dangling"), "{link}");
    let object = link.as_object().expect("an object");
    for key in ["title", "end_kind"] {
        assert_eq!(
            object.get(key),
            Some(&Value::Null),
            "`{key}` present, null: {link}"
        );
    }
    assert!(run.stdout.contains(&in_data_order(link)), "{}", run.show());
}

// ------------------------------------------------------------------ DM-3

/// DM-3, the spec's example: on flow-aside, `spec flow ATL-POTTERY --depth 1 --in-flow
/// --json`'s `two-starts` row, the members folded into `KRUN-FIRING` and
/// `WBAY-GALLERY` titled by their own titles; the text row is the one
/// before. M: titles from the folded box ("Firing run").
#[test]
fn dm3_the_aside_two_starts_row_is_datas() {
    let copy = Copy::new("flow-aside");
    let args = ["flow", "ATL-POTTERY", "--depth", "1", "--in-flow"];
    let document = copy.json(&args);
    assert_eq!(
        *gap(&document, "two-starts"),
        json!({"code": "two-starts", "state": "measured", "count": 1,
            "items": [["POT-WEDGE", "docs/workshop/gallery/open-day.md"]],
            "titles": [["Wedging", "Open day"]],
            "cause": CAUSE_TWO_STARTS})
    );
    let groups: BTreeMap<String, Value> = document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| (name(node), node["title"].clone()))
        .collect();
    assert_eq!(groups.get("KRUN-FIRING"), Some(&json!("Firing run")));
    assert!(
        !groups.contains_key("POT-WEDGE"),
        "premise: folded at depth 1"
    );
    let text = copy.run(&args);
    text.code(0);
    assert!(
        text.stdout.lines().any(|line| line
            == format!(
                "gap two-starts | measured | 1 | POT-WEDGE + \
                 docs/workshop/gallery/open-day.md | {CAUSE_TWO_STARTS}"
            )),
        "{}",
        text.show()
    );
}

/// DM-3 on flow-gaps, whole and under two ROOTs: every row's `titles` has
/// `items`' shape, each title `spec show <member> --json`'s; the
/// `two-starts` row of the whole read is the spec's three titles; the text
/// prints no title on a gap line.
#[test]
fn dm3_each_gap_title_is_its_members_own() {
    let copy = Copy::new("flow-gaps");
    let mut cache = BTreeMap::new();
    let mut titled = 0usize;
    for args in [
        &["flow"][..],
        &["flow", "WING-BROKEN"][..],
        &["flow", "WING-LIBRARY", "--depth", "1"][..],
    ] {
        let document = copy.json(args);
        for row in document["gaps"].as_array().expect("gaps") {
            assert_eq!(
                shape(&row["titles"]),
                shape(&row["items"]),
                "{args:?}: {row}"
            );
            let items = row["items"].as_array().expect("items");
            for (item, titles) in items.iter().zip(row["titles"].as_array().expect("titles")) {
                for (member, title) in item
                    .as_array()
                    .expect("an item")
                    .iter()
                    .zip(titles.as_array().expect("a title list"))
                {
                    let member = member.as_str().expect("a name");
                    let (own, _) = first_node(&copy, &mut cache, member);
                    assert_eq!(*title, own, "{args:?}: {member}: {row}");
                    titled += usize::from(title.is_string());
                }
            }
        }
        let text = copy.run(args);
        text.code(0);
        for line in text.stdout.lines().filter(|line| line.starts_with("gap ")) {
            for (title, _) in cache.values() {
                if let Some(title) = title.as_str() {
                    assert!(!line.contains(title), "{args:?}: {line}");
                }
            }
        }
    }
    assert!(titled > 0, "premise: titled members");
    let whole = copy.json(&["flow"]);
    assert_eq!(
        gap(&whole, "two-starts")["titles"],
        json!([[
            "The first closed desk",
            "Ask at the desk",
            "The second closed desk"
        ]])
    );
    assert_eq!(
        gap(&whole, "two-starts")["items"],
        json!([["LND-A", "LND-ASK", "LND-B"]])
    );
}

/// DM-3, a headless scope (the orchestrator's reading of 2026-11-10): the
/// `no-start` row names the scope, titled by its first ROOT holder's title,
/// `null` at Top (named `all`). On flow-gaps, `flow PLT-UNUSED` gives
/// `[["The old plate"]]`; on a copy whose `[[flow.start]]` matches nothing,
/// the read at Top `[[null]]`.
#[test]
fn dm3_a_headless_scope_is_titled_by_its_root() {
    let copy = Copy::new("flow-gaps");
    let part = copy.json(&["flow", "PLT-UNUSED"]);
    let row = gap(&part, "no-start");
    assert_eq!(row["items"], json!([["PLT-UNUSED"]]), "{row}");
    assert_eq!(row["titles"], json!([["The old plate"]]), "{row}");

    let config = read_text(&copy.root, "specengine.toml");
    let starts = "[[flow.start]]\npaths = [\"docs/spec/lnd-ask.md\", \"docs/spec/lnd-a.md\", \
                  \"docs/spec/lnd-b.md\"]\n";
    assert!(config.contains(starts), "premise: the fixture's starts");
    fs::write(
        copy.root.join("specengine.toml"),
        config.replace(
            starts,
            "[[flow.start]]\npaths = [\"docs/spec/no-such-step.md\"]\n",
        ),
    )
    .expect("the config");
    let top = copy.json(&["flow"]);
    let row = gap(&top, "no-start");
    assert_eq!(row["count"], json!(1), "{row}");
    assert_eq!(row["items"], json!([["all"]]), "{row}");
    assert_eq!(row["titles"], json!([[null]]), "{row}");
}

// ------------------------------------------------------------------ the golden

/// One read, as the golden holds it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Row {
    fixture: String,
    args: Vec<String>,
    /// Its exit, then the JSON read's.
    code: String,
    /// b3 of the text stdout, of the JSON stdout less the slice's keys, and
    /// of the text stderr, each with scratch paths as placeholders.
    text: String,
    json: String,
    stderr: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Golden {
    recorded_with: String,
    procedure: String,
    rows: Vec<Row>,
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(GOLDEN)
}

fn golden() -> Golden {
    let path = golden_path();
    serde_json::from_slice(
        &fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .expect("the golden is JSON")
}

/// `text` with the scratch's paths as placeholders.
fn normalised(text: &str, places: &[(String, &'static str)]) -> String {
    let mut out = text.to_owned();
    for (from, to) in places {
        out = out.replace(from.as_str(), to);
    }
    out
}

/// A JSON stdout with doc-modal's keys out, every other byte kept; a stdout
/// holding no JSON document as it is.
fn json_before(stdout: &str) -> String {
    if stdout.starts_with('{') {
        without_doc_modal_keys(stdout)
    } else {
        stdout.to_owned()
    }
}

/// One read by `program` on `copy`: its row.
fn play(copy: &Copy, program: &OsStr, fixture: &str, args: &[String]) -> Row {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let plain = copy.run_with(program, &args);
    let mut asked = args.clone();
    asked.push("--json");
    let json = copy.run_with(program, &asked);
    let places = copy.placeholders();
    let hash = |text: &str| b3_hash(normalised(text, &places).as_bytes());
    Row {
        fixture: fixture.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: format!("{} {}", plain.code, json.code),
        text: hash(&plain.stdout),
        json: hash(&json_before(&json.stdout)),
        stderr: hash(&plain.stderr),
    }
}

/// The golden's reads, by fixture: `show <name> --links` for each DM-1 name
/// (the tree's by `program`, then the parse's), then the flow reads of
/// [`FLOWS`]; each on one copy of its fixture with one `HOME`.
fn play_all(program: &OsStr) -> Vec<Row> {
    let mut rows = Vec::new();
    for (fixture, root) in LINKED {
        let copy = Copy::new(fixture);
        let (names, _) = names_of(&copy, program, root);
        for name in names {
            let args = vec!["show".to_owned(), name, "--links".to_owned()];
            rows.push(play(&copy, program, fixture, &args));
        }
        for (_, flags) in FLOWS.iter().filter(|(name, _)| *name == fixture) {
            let mut args = vec!["flow".to_owned()];
            args.extend(flags.iter().map(|flag| (*flag).to_owned()));
            rows.push(play(&copy, program, fixture, &args));
        }
    }
    let copy = Copy::new("flow-gaps");
    for (_, flags) in FLOWS.iter().filter(|(name, _)| *name == "flow-gaps") {
        let mut args = vec!["flow".to_owned()];
        args.extend(flags.iter().map(|flag| (*flag).to_owned()));
        rows.push(play(&copy, program, "flow-gaps", &args));
    }
    rows
}

/// DM-1 and DM-3, "the text is byte-equal": every read of the golden, played
/// now: its exits, text and stderr byte-equal to the binary before the
/// slice, its JSON byte-equal with doc-modal's three keys out (a node's own
/// `title` kept). M: a ` | <title>` on a text link line; a title on a gap
/// line.
#[test]
fn dm1_dm3_the_reads_are_the_bytes_before_but_the_keys() {
    let golden = golden();
    assert_eq!(golden.recorded_with, RECORDED_WITH);
    let now = play_all(OsStr::new(SPEC));
    let want: Vec<(String, Vec<String>)> = golden
        .rows
        .iter()
        .map(|row| (row.fixture.clone(), row.args.clone()))
        .collect();
    let got: Vec<(String, Vec<String>)> = now
        .iter()
        .map(|row| (row.fixture.clone(), row.args.clone()))
        .collect();
    assert_eq!(got, want, "the golden holds every read, in order");
    let differ: Vec<String> = golden
        .rows
        .iter()
        .zip(&now)
        .filter(|(before, after)| before != after)
        .map(|(before, after)| format!("{before:?}\n  now {after:?}"))
        .collect();
    assert!(
        differ.is_empty(),
        "{} of {} reads differ from the build before:\n{}",
        differ.len(),
        now.len(),
        differ.join("\n")
    );
}

/// Writes [`GOLDEN`] from the binary `SPECENGINE_BEFORE_SPEC` names
/// (3d3180e's `spec`, built before any `src/` change of doc-modal). Run
/// once, by hand: `SPECENGINE_BEFORE_SPEC=<binary> cargo nextest run -p
/// specengine-cli --test doc_modal record_the_answers_before
/// --run-ignored ignored-only`. Every read is played twice and must answer
/// the same both times. It refuses when the golden exists.
#[test]
#[ignore = "manual: records doc-modal's golden from 3d3180e's binary, once"]
fn record_the_answers_before_doc_modal() {
    let program = std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC");
    let path = golden_path();
    assert!(
        !path.exists(),
        "{GOLDEN} exists: it is recorded once, never again"
    );
    let rows = play_all(&program);
    let again = play_all(&program);
    assert_eq!(rows, again, "the binary before answers alike twice");
    for row in &rows {
        assert!(
            row.code == "0 0" || row.args[0] == "show",
            "a flow read answers: {row:?}"
        );
    }
    let golden = Golden {
        recorded_with: RECORDED_WITH.to_owned(),
        procedure: "per fixture: one fresh copy, one fresh HOME, an empty working directory; \
                    `show <name> --links` for every name `spec tree --json` lists (flow-aside \
                    under ATL-POTTERY) then every other node of the parse, then the flow reads; \
                    each read plain and with --json, `--root <copy>` last; b3 of the text \
                    stdout, of the JSON stdout without doc-modal's keys, of the stderr; scratch \
                    paths as <root>, <home>, <cwd>"
            .to_owned(),
        rows,
    };
    // One row a line.
    let mut text = format!(
        "{{\"recorded_with\":{},\"procedure\":{},\"rows\":[\n",
        serde_json::to_string(&golden.recorded_with).expect("JSON"),
        serde_json::to_string(&golden.procedure).expect("JSON")
    );
    let rows: Vec<String> = golden
        .rows
        .iter()
        .map(|row| serde_json::to_string(row).expect("JSON"))
        .collect();
    text.push_str(&rows.join(",\n"));
    text.push_str("\n]}\n");
    fs::write(&path, text).expect("write the golden");
}

// ------------------------------------------------------------------ the scan

/// The scan takes `title` and `end_kind` only beside `end_kind` (a link),
/// `titles` only beside `items` and `code` (a gap row), every other byte
/// kept: a node's own `title` stays.
#[test]
fn the_doc_modal_scan_takes_only_the_slices_keys() {
    assert_eq!(
        without_doc_modal_keys(
            "{\"title\":\"T\",\"links\":{\"outgoing\":[{\"name\":\"A\",\"title\":\"B\",\
             \"end_kind\":null,\"written\":\"x\"}]}}\n"
        ),
        "{\"title\":\"T\",\"links\":{\"outgoing\":[{\"name\":\"A\",\"written\":\"x\"}]}}\n"
    );
    assert_eq!(
        without_doc_modal_keys(
            "{\"gaps\":[{\"code\":\"x\",\"items\":[[\"A\"]],\"titles\":[[null]],\
             \"cause\":null}],\"nodes\":[{\"title\":\"N\",\"titles\":1}]}"
        ),
        "{\"gaps\":[{\"code\":\"x\",\"items\":[[\"A\"]],\"cause\":null}],\
         \"nodes\":[{\"title\":\"N\",\"titles\":1}]}"
    );
    assert_eq!(
        without_doc_modal_keys("{\"end_kind\":\"k\",\"title\":\"t\"}"),
        "{}"
    );
    assert_eq!(without_doc_modal_keys("[1,{},[]]"), "[1,{},[]]");
}
