//! docs/features/corpus-map.md (RM-0112, ADR-0080) "Acceptance criteria",
//! the CLI half: `spec map [--archive] [--json]` over scratch copies of
//! `fixtures/spec-a` ("the copy", only read), each under its own `HOME`,
//! with invented files written here.
//!
//! - CM1: the copy's counts (12 documents, 7 sections, 35 edges, 3
//!   parents, 0 ghosts, 1 orphan), the nodes in (path, ord) order,
//!   RULE-STAM-REGEN `of` MEC-STAMINA's index, each node's usage core
//!   `usage_of`'s and the `usage-orphans` table; the edges equal the union,
//!   over every live node X, of `spec graph X --depth 1 --type T…` for every
//!   type of `types` and `mentions`, resolved ends between admitted nodes,
//!   merged per (src, dst, type) with their places counted; `--archive`
//!   gives 13 documents, DEC-0007 `archived` with its `supersedes` edge, the
//!   usage still the live files'. M: an edge per place; self-edges kept;
//!   Tier 3 nodes without `--archive`.
//! - CM2: `depends_on: [MEC-GONE]` in `sprint.md` and a wiki mention in
//!   `stamina.md` give one ghost, `sources` both, `places` 2; the `project:`
//!   link gives none. M: skipped links as ghosts; a ghost per place.
//! - CM3: the text holds "Data"'s lines; past `OUTPUT_CAP_CHARS` the text is
//!   cut at a line with its tail, the JSON whole; the copy at two roots and
//!   two `HOME`s, its files written in opposite orders, gives byte-identical
//!   text and JSON; `notes` sorted, each once; exit 0 with zero nodes, 2 on
//!   usage, no project, a config or `HOME` error, never 1. M: the JSON cut
//!   with the text; `notes` in input order.
//!
//! Every expected value is worked out from the spec and the files here,
//! never read back from a run of the read under test.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Run, Scratch, read, read_text, replace, spec, spec_with, write};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_core::usage::usage_of;
use specengine_store::{WorkingTree, check_input};

/// The copy's live nodes in (path, ord) order, worked out from its files:
/// each document, then its ID sections by position.
const LIVE: [&str; 19] = [
    "docs/features/stamina-tuning.md",
    // corpus-map iteration 3: a feature-scoped ID in a feature document is
    // named by its canonical reference.
    "stamina-tuning/AC-07",
    "A-101",
    "A-102",
    "DEC-0023",
    "Q-031",
    "Q-032",
    "R-12",
    "TERM-exhausted",
    "DOM-GAME",
    "RULE-CORE-LOOP",
    "DOM-MOVEMENT",
    "RULE-MOVE-SPEEDS",
    "MEC-SPRINT",
    "RULE-SPRINT-COST",
    "EDGE-SPRINT-EMPTY",
    "MEC-STAMINA",
    "RULE-STAM-REGEN",
    "EDGE-STAM-ZERO",
];

/// The sections of [`LIVE`] with their document.
const SECTIONS: [(&str, &str); 7] = [
    ("stamina-tuning/AC-07", "docs/features/stamina-tuning.md"),
    ("RULE-CORE-LOOP", "DOM-GAME"),
    ("RULE-MOVE-SPEEDS", "DOM-MOVEMENT"),
    ("RULE-SPRINT-COST", "MEC-SPRINT"),
    ("EDGE-SPRINT-EMPTY", "MEC-SPRINT"),
    ("RULE-STAM-REGEN", "MEC-STAMINA"),
    ("EDGE-STAM-ZERO", "MEC-STAMINA"),
];

/// `(used_by, mentioned_by, orphan)` of one node.
type Counts = (u64, u64, Option<bool>);

/// `docs/features/usage-orphans.md`'s table on `fixtures/spec-a`, the
/// counts CM1 names.
const USAGE: [(&str, Counts); 13] = [
    ("MEC-STAMINA", (2, 6, Some(false))),
    ("RULE-STAM-REGEN", (2, 3, None)),
    ("MEC-SPRINT", (1, 1, Some(false))),
    ("DOM-MOVEMENT", (0, 1, Some(false))),
    ("DOM-GAME", (0, 0, Some(false))),
    ("A-101", (2, 3, Some(false))),
    ("A-102", (0, 0, Some(true))),
    ("DEC-0023", (1, 1, Some(false))),
    ("Q-031", (1, 3, Some(false))),
    ("Q-032", (0, 0, Some(false))),
    ("R-12", (1, 6, Some(false))),
    ("TERM-exhausted", (1, 1, Some(false))),
    ("docs/features/stamina-tuning.md", (0, 1, Some(false))),
];

/// The key sets of "Data", in the order written there.
const MAP_DOCUMENT: [&str; 7] = [
    "archive", "notes", "counts", "nodes", "edges", "parents", "ghosts",
];
const MAP_COUNTS: [&str; 6] = [
    "documents",
    "sections",
    "edges",
    "parents",
    "ghosts",
    "orphans",
];
const MAP_NODE: [&str; 12] = [
    "name",
    "id",
    "kind",
    "title",
    "path",
    "line",
    "of",
    "status",
    "archived",
    "used_by",
    "mentioned_by",
    "orphan",
];
const MAP_EDGE: [&str; 5] = ["src", "dst", "type", "weak", "places"];
const MAP_PARENT: [&str; 2] = ["child", "parent"];
const MAP_GHOST: [&str; 4] = ["written", "reason", "sources", "places"];

/// The weak link type: `weak` is set for it alone.
const WEAK: &str = "mentions";

/// The text's cut, as the library says it.
const CAP: usize = specengine_cli::OUTPUT_CAP_CHARS;

/// A JSON object's keys, in the order written.
struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> Visitor<'de> for Seen {
            type Value = Keys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

/// The top-level keys of the JSON object `text`, in the order written.
fn keys_in_order(text: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(text)
        .unwrap_or_else(|error| panic!("{error}: {text}"))
        .0
}

/// One scratch copy and its `HOME`.
struct Copy {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
}

impl Copy {
    /// A copy of `fixtures/spec-a` with `files` written in.
    fn new(label: &str, files: &[(&str, &str)]) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy("spec-a", "copy");
        for (path, text) in files {
            write(&root, path, text);
        }
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            root,
            home,
        }
    }

    fn run(&self, args: &[&str]) -> Run {
        spec(&self.home, &self.root, args)
    }

    /// `spec args… --json`, exit 0: the document.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.run(&all);
        run.code(0);
        run.json()
    }

    /// `spec map [--archive] --json`, exit 0.
    fn map(&self, archive: bool) -> Value {
        if archive {
            self.json(&["map", "--archive"])
        } else {
            self.json(&["map"])
        }
    }
}

/// The served names, in order.
fn names(map: &Value) -> Vec<String> {
    map["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// The index of `name` among the served nodes (the first holder).
fn index_of(map: &Value, name: &str) -> usize {
    names(map)
        .iter()
        .position(|served| served == name)
        .unwrap_or_else(|| panic!("{name} not served: {map}"))
}

/// The served node named `name` (the first holder).
fn node<'a>(map: &'a Value, name: &str) -> &'a Value {
    &map["nodes"][index_of(map, name)]
}

/// An index of the served document.
fn at(value: &Value) -> usize {
    usize::try_from(
        value
            .as_u64()
            .unwrap_or_else(|| panic!("an index: {value}")),
    )
    .expect("usize")
}

/// The served edges as `(src name, dst name, type) -> places`, each
/// `weak` checked against its type; the order checked as (src, dst, type).
fn served_edges(map: &Value) -> BTreeMap<(String, String, String), u64> {
    let names = names(map);
    let mut out = BTreeMap::new();
    let mut previous: Option<(usize, usize, String)> = None;
    for edge in map["edges"].as_array().expect("edges") {
        let src = at(&edge["src"]);
        let dst = at(&edge["dst"]);
        let kind = edge["type"].as_str().expect("a type").to_owned();
        assert_eq!(
            edge["weak"],
            json!(kind == WEAK),
            "`weak` is set for `mentions` alone: {edge}"
        );
        let key = (src, dst, kind.clone());
        if let Some(previous) = &previous {
            assert!(
                previous < &key,
                "edges sorted by (src, dst, type), once each: {edge}"
            );
        }
        previous = Some(key);
        let places = edge["places"].as_u64().expect("places");
        assert!(places >= 1, "{edge}");
        let triple = (names[src].clone(), names[dst].clone(), kind);
        assert!(out.insert(triple, places).is_none(), "{edge}");
    }
    out
}

/// CM1's oracle: for every name X of `names` (each a REF `spec graph`
/// takes), `spec graph X --depth 1` following every type the default walk
/// lists and `mentions`; each resolved edge between two of `names`, `src`
/// not `dst`, counted once per written place (path, line, written form),
/// merged per (src, dst, type). The graph names a node by its ID, else its
/// path; each name is taken to the map's canonical one through the node's
/// `path:line` as the graph's own node lists give it.
fn graph_oracle(
    copy: &Copy,
    names: &[String],
    archive: bool,
) -> BTreeMap<(String, String, String), u64> {
    let admitted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let map = copy.map(archive);
    let canonical: BTreeMap<(String, u64), String> = map["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                (
                    node["path"].as_str().expect("a path").to_owned(),
                    node["line"].as_u64().expect("a line"),
                ),
                node["name"].as_str().expect("a name").to_owned(),
            )
        })
        .collect();
    let default = copy.json(&["graph", "MEC-STAMINA"]);
    let mut types: Vec<String> = default["types"]
        .as_array()
        .expect("types")
        .iter()
        .map(|followed| followed["type"].as_str().expect("a type").to_owned())
        .collect();
    assert!(types.len() > 5, "{default}");
    types.push(WEAK.to_owned());
    let mut places: BTreeSet<(String, String, String, String, u64, String)> = BTreeSet::new();
    let mut graph_names: BTreeMap<String, (String, u64)> = BTreeMap::new();
    for name in names {
        let mut args: Vec<&str> = vec!["graph", name, "--depth", "1"];
        for kind in &types {
            args.push("--type");
            args.push(kind);
        }
        if archive {
            args.push("--archive");
        }
        let graph = copy.json(&args);
        for node in graph["nodes"].as_array().expect("nodes") {
            let path = node["path"].as_str().expect("a path").to_owned();
            let own = node["id"].as_str().unwrap_or(&path).to_owned();
            graph_names.insert(own, (path, node["line"].as_u64().expect("a line")));
        }
        for edge in graph["edges"].as_array().expect("edges") {
            if edge["state"] != "resolved" {
                continue;
            }
            places.insert((
                edge["src"].as_str().expect("src").to_owned(),
                edge["dst"].as_str().expect("dst").to_owned(),
                edge["type"].as_str().expect("a type").to_owned(),
                edge["path"].as_str().expect("a path").to_owned(),
                edge["line"].as_u64().expect("a line"),
                edge["written"].as_str().expect("written").to_owned(),
            ));
        }
    }
    let to_canonical = |name: &str| -> Option<String> {
        graph_names
            .get(name)
            .and_then(|place| canonical.get(place))
            .cloned()
    };
    let mut out = BTreeMap::new();
    for (src, dst, kind, path, line, written) in places {
        let (Some(src), Some(dst)) = (to_canonical(&src), to_canonical(&dst)) else {
            continue;
        };
        if src == dst || !admitted.contains(src.as_str()) || !admitted.contains(dst.as_str()) {
            continue;
        }
        let _ = (path, line, written);
        *out.entry((src, dst, kind)).or_insert(0) += 1;
    }
    out
}

/// Core `usage_of` over the copy's whole graph, by `(path, line)`.
fn core_usage(root: &Path) -> BTreeMap<(String, u64), Counts> {
    let text = read_text(root, "specengine.toml");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let tree = WorkingTree::new(root, &config.paths).expect("the working tree");
    let input = check_input(&tree, &config.scheme);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let usage = usage_of(&graph);
    let mut out = BTreeMap::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            let node = usage
                .get(&at)
                .unwrap_or_else(|| panic!("{}: no usage", graph.name(at)));
            out.insert(
                (
                    graph.paths()[file].to_owned(),
                    u64::try_from(graph.line(at)).expect("u64"),
                ),
                (
                    u64::from(node.used_by),
                    u64::from(node.mentioned_by),
                    node.orphan,
                ),
            );
        }
    }
    out
}

/// A served node's `(used_by, mentioned_by, orphan)`.
fn served_counts(node: &Value) -> Counts {
    (
        node["used_by"].as_u64().expect("used_by"),
        node["mentioned_by"].as_u64().expect("mentioned_by"),
        node["orphan"].as_bool(),
    )
}

/// Every served node's usage is core's at its place.
fn assert_usage_is_cores(copy: &Copy, map: &Value) {
    let core = core_usage(&copy.root);
    for served in map["nodes"].as_array().expect("nodes") {
        let place = (
            served["path"].as_str().expect("a path").to_owned(),
            served["line"].as_u64().expect("a line"),
        );
        let want = core
            .get(&place)
            .unwrap_or_else(|| panic!("{place:?}: not in core's graph"));
        assert_eq!(served_counts(served), *want, "{served}");
    }
}

/// Every section's `of` is its document's index, every document's `null`.
fn assert_of(map: &Value) {
    let nodes = map["nodes"].as_array().expect("nodes");
    for (index, served) in nodes.iter().enumerate() {
        let name = served["name"].as_str().expect("a name");
        match SECTIONS.iter().find(|(section, _)| *section == name) {
            Some((_, document)) => {
                let of = at(&served["of"]);
                assert_eq!(nodes[of]["name"], *document, "{served}");
                assert!(of < index, "{served}");
                assert!(nodes[of]["of"].is_null(), "{served}");
                assert!(
                    served["orphan"].is_null(),
                    "a section has no flag: {served}"
                );
            }
            None => assert!(served["of"].is_null(), "a document: {served}"),
        }
    }
}

// ------------------------------------------------------------------ CM1

/// CM1: the copy's counts, order, `of`, usage and edges, the keys of
/// "Data" in its order. M: an edge per place; Tier 3 nodes without
/// `--archive`.
#[test]
fn cm1_the_map_read_on_the_copy() {
    let copy = Copy::new("cm1", &[]);
    let run = copy.run(&["map", "--json"]);
    run.code(0);
    let body = run.stdout.strip_suffix('\n').expect("one line end");
    assert_eq!(keys_in_order(body), MAP_DOCUMENT, "MapDocument");
    let map = run.json();
    assert_eq!(map["archive"], false);
    assert_eq!(map["notes"], json!([]), "{map}");
    assert_eq!(
        map["counts"],
        json!({"documents": 12, "sections": 7, "edges": 35, "parents": 3, "ghosts": 0, "orphans": 1}),
        "CM1's counts"
    );
    let counts = serde_json::to_string(&map["counts"]).expect("text");
    assert_eq!(
        keys_in_order(&raw_part(body, "counts")),
        MAP_COUNTS,
        "MapCounts: {counts}"
    );
    assert_eq!(names(&map), LIVE, "nodes in (path, ord) order");
    for served in map["nodes"].as_array().expect("nodes") {
        let keys: BTreeSet<&str> = served
            .as_object()
            .expect("a node")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, MAP_NODE.into_iter().collect(), "{served}");
        assert_eq!(served["archived"], false, "{served}");
    }
    assert_eq!(at(&node(&map, "RULE-STAM-REGEN")["of"]), 16);
    assert_eq!(index_of(&map, "MEC-STAMINA"), 16);
    assert_of(&map);
    // "Data"'s three nodes, whole.
    assert_eq!(
        map["nodes"][0],
        json!({"name": "docs/features/stamina-tuning.md", "id": null, "kind": null, "title": "Stamina tuning", "path": "docs/features/stamina-tuning.md", "line": 1, "of": null, "status": "draft", "archived": false, "used_by": 0, "mentioned_by": 1, "orphan": false})
    );
    assert_eq!(
        *node(&map, "A-102"),
        json!({"name": "A-102", "id": "A-102", "kind": "requirement", "title": "The lantern burns one unit of oil per minute", "path": "docs/records/A/A-102.md", "line": 1, "of": null, "status": "open", "archived": false, "used_by": 0, "mentioned_by": 0, "orphan": true})
    );
    assert_eq!(
        *node(&map, "RULE-STAM-REGEN"),
        json!({"name": "RULE-STAM-REGEN", "id": "RULE-STAM-REGEN", "kind": "rule", "title": "Regeneration", "path": "docs/spec/movement/stamina.md", "line": 21, "of": 16, "status": null, "archived": false, "used_by": 2, "mentioned_by": 3, "orphan": null})
    );
    // The usage-orphans table, then core's for every node.
    for (name, counts) in USAGE {
        assert_eq!(served_counts(node(&map, name)), counts, "{name}");
    }
    assert_usage_is_cores(&copy, &map);
    let orphans = map["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|served| served["orphan"] == true)
        .count();
    assert_eq!(orphans, 1, "`orphans` counts the flags");
    // The edges: keys, the graph oracle, 25 of them mentions.
    for (part, want) in [("edges", &MAP_EDGE[..]), ("parents", &MAP_PARENT[..])] {
        for item in map[part].as_array().expect(part) {
            let keys: BTreeSet<&str> = item
                .as_object()
                .expect("an object")
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(keys, want.iter().copied().collect(), "{part}: {item}");
        }
    }
    let served = served_edges(&map);
    let live: Vec<String> = LIVE.iter().map(|name| (*name).to_owned()).collect();
    let oracle = graph_oracle(&copy, &live, false);
    assert_eq!(
        oracle.len(),
        35,
        "the oracle's own count, worked out by hand"
    );
    assert_eq!(
        served, oracle,
        "the edges are the graph's, merged per triple"
    );
    assert_eq!(
        served.keys().filter(|(_, _, kind)| kind == WEAK).count(),
        25,
        "25 of the 35 are mentions"
    );
    assert_eq!(
        served.get(&(
            "docs/features/stamina-tuning.md".to_owned(),
            "R-12".to_owned(),
            WEAK.to_owned()
        )),
        Some(&2),
        "two written places, one edge"
    );
    assert!(
        !served
            .keys()
            .any(|(src, dst, _)| src == "DEC-0007" || dst == "DEC-0007"),
        "DEC-0023's `supersedes` of the Tier 3 DEC-0007 is out"
    );
    assert_eq!(
        map["edges"].as_array().expect("edges").iter().find(|edge| {
            edge["src"] == 13 && edge["dst"] == 16 && edge["type"] == "depends_on"
        }),
        Some(&json!({"src": 13, "dst": 16, "type": "depends_on", "weak": false, "places": 1})),
        "Data's edge"
    );
    assert_eq!(
        map["parents"],
        json!([{"child": 11, "parent": 9}, {"child": 13, "parent": 11}, {"child": 16, "parent": 11}])
    );
    assert_eq!(map["ghosts"], json!([]));
}

/// The raw text of the value of `key` in the JSON object `body` (an object
/// value), its keys' order kept.
fn raw_part(body: &str, key: &str) -> String {
    let marker = format!("\"{key}\":");
    let start = body
        .find(&marker)
        .unwrap_or_else(|| panic!("{key}: {body}"))
        + marker.len();
    let rest = body[start..].trim_start();
    let mut depth = 0;
    for (index, letter) in rest.char_indices() {
        match letter {
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return rest[..=index].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("{key}: no closed value in {body}");
}

/// CM1, `--archive`: 13 documents, DEC-0007 `archived` at its place with
/// its `supersedes` edge from DEC-0023 (two written places) and its mention,
/// every node's usage still the live files' (DEC-0023 `used_by` 1).
#[test]
fn cm1_archive_admits_the_tier3_decision_and_keeps_the_live_counts() {
    let copy = Copy::new("cm1-archive", &[]);
    let map = copy.map(true);
    assert_eq!(map["archive"], true);
    assert_eq!(
        map["counts"],
        json!({"documents": 13, "sections": 7, "edges": 37, "parents": 3, "ghosts": 0, "orphans": 1}),
        "CM1: 13 documents; the edges the oracle's below"
    );
    let mut want: Vec<String> = LIVE.iter().map(|name| (*name).to_owned()).collect();
    want.insert(4, "DEC-0007".to_owned());
    assert_eq!(names(&map), want, "DEC-0007 at its (path, ord) place");
    let decision = node(&map, "DEC-0007");
    assert_eq!(decision["archived"], true, "{decision}");
    assert_eq!(decision["path"], "docs/records/DEC/DEC-0007.md");
    assert_eq!(decision["status"], "superseded-by DEC-0023");
    for served in map["nodes"].as_array().expect("nodes") {
        if served["name"] != "DEC-0007" {
            assert_eq!(served["archived"], false, "{served}");
        }
    }
    assert_of(&map);
    for (name, counts) in USAGE {
        assert_eq!(
            served_counts(node(&map, name)),
            counts,
            "{name}: the live files' counts"
        );
    }
    assert_usage_is_cores(&copy, &map);
    let served = served_edges(&map);
    assert_eq!(
        served,
        graph_oracle(&copy, &want, true),
        "the edges, archive"
    );
    assert_eq!(
        served.get(&(
            "DEC-0023".to_owned(),
            "DEC-0007".to_owned(),
            "supersedes".to_owned()
        )),
        Some(&2),
        "its `supersedes` edge: `supersedes:` and `status: superseded-by`"
    );
    // The live answer's indexes shift by one past DEC-0007.
    assert_eq!(
        map["parents"],
        json!([{"child": 12, "parent": 10}, {"child": 14, "parent": 12}, {"child": 17, "parent": 12}])
    );
}

/// CM1: a node citing itself, and a section citing itself, give no edge;
/// its other links stay. M: self-edges kept.
#[test]
fn cm1_a_node_citing_itself_draws_no_edge() {
    let copy = Copy::new(
        "cm1-self",
        &[(
            "docs/records/A/A-103.md",
            "---\nid: A-103\nclass: canon\nstatus: open\nlinks:\n  derived_from: [A-103, R-12]\n---\n\n# The gate closes at dusk\n\nA-103 holds until the gate is rebuilt; see A-101.\n",
        )],
    );
    replace(
        &copy.root,
        "docs/spec/movement/stamina.md",
        "- Base rate 10 units/s",
        "- Base rate 10 units/s as RULE-STAM-REGEN says",
    );
    let map = copy.map(false);
    let served = served_edges(&map);
    assert!(
        !served.keys().any(|(src, dst, _)| src == dst),
        "no self-edge: {served:?}"
    );
    for kept in [
        ("A-103", "A-101", WEAK),
        ("A-103", "R-12", "derived_from"),
        ("RULE-STAM-REGEN", "R-12", WEAK),
    ] {
        assert!(
            served.contains_key(&(kept.0.to_owned(), kept.1.to_owned(), kept.2.to_owned())),
            "{kept:?}: {served:?}"
        );
    }
    assert_eq!(served, graph_oracle(&copy, &names(&map), false));
    assert_eq!(map["counts"]["edges"], json!(served.len()));
}

// ------------------------------------------------------------------ CM2

/// The copy with CM2's two dangling ends written in.
fn ghost_copy(label: &str) -> Copy {
    let copy = Copy::new(label, &[]);
    replace(
        &copy.root,
        "docs/spec/movement/sprint.md",
        "depends_on: [MEC-STAMINA]",
        "depends_on: [MEC-STAMINA, MEC-GONE]",
    );
    replace(
        &copy.root,
        "docs/spec/movement/stamina.md",
        "\n## Regeneration {#RULE-STAM-REGEN}",
        "\nOnce paired with [[MEC-GONE]].\n\n## Regeneration {#RULE-STAM-REGEN}",
    );
    copy
}

/// CM2: one ghost for MEC-GONE, from MEC-SPRINT and MEC-STAMINA, two
/// places, its reason the first place's (`show --links`' dangling reason);
/// the `project:` link `shared:DEC-0023@1` gives none. M: skipped links as
/// ghosts; a ghost per place.
#[test]
fn cm2_one_ghost_per_written_end() {
    let copy = ghost_copy("cm2");
    let shown = copy.json(&["show", "MEC-SPRINT", "--links"]);
    let reason = shown["nodes"][0]["links"]["outgoing"]
        .as_array()
        .expect("outgoing")
        .iter()
        .find(|link| link["written"] == "MEC-GONE")
        .unwrap_or_else(|| panic!("MEC-GONE out of MEC-SPRINT: {shown}"))
        .clone();
    assert_eq!(reason["state"], "dangling", "{reason}");
    let skipped = shown["nodes"][0]["links"]["outgoing"]
        .as_array()
        .expect("outgoing")
        .iter()
        .any(|link| link["state"] == "skipped");
    assert!(
        skipped,
        "the premise: the `project:` link is there: {shown}"
    );
    let map = copy.map(false);
    let sprint = index_of(&map, "MEC-SPRINT");
    let stamina = index_of(&map, "MEC-STAMINA");
    assert_eq!(
        map["ghosts"],
        json!([{"written": "MEC-GONE", "reason": reason["reason"], "sources": [sprint, stamina], "places": 2}]),
        "{map}"
    );
    assert!(map["ghosts"][0]["reason"].is_string(), "{map}");
    let keys: BTreeSet<&str> = map["ghosts"][0]
        .as_object()
        .expect("a ghost")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, MAP_GHOST.into_iter().collect(), "MapGhost");
    assert_eq!(map["counts"]["ghosts"], 1);
    assert_eq!(map["counts"]["edges"], 35, "a ghost is no edge");
    let run = copy.run(&["map"]);
    run.code(0);
    assert!(
        run.stdout
            .lines()
            .any(|line| line == "ghost MEC-GONE | from MEC-SPRINT, MEC-STAMINA | places 2"),
        "Data's ghost line:\n{}",
        run.stdout
    );
    assert!(
        run.stdout
            .lines()
            .next()
            .is_some_and(|line| line.ends_with(", ghosts 1, orphans 1")),
        "{}",
        run.stdout
    );
}

// ------------------------------------------------------------------ CM3

/// The line of each served node, edge, parent and ghost as "Data" writes
/// them, built from the JSON.
fn lines_of(map: &Value) -> Vec<String> {
    let names = names(map);
    let mut out = Vec::new();
    for served in map["nodes"].as_array().expect("nodes") {
        let text = |key: &str| served[key].as_str().unwrap_or("-").to_owned();
        let mut line = format!(
            "{} | {} | used by {}, mentioned by {}",
            text("name"),
            text("kind"),
            served["used_by"],
            served["mentioned_by"]
        );
        if served["orphan"] == true {
            line.push_str(" | orphan");
        }
        if !served["of"].is_null() {
            line.push_str(&format!(" | of {}", names[at(&served["of"])]));
        }
        line.push_str(&format!(
            " | {} | {}:{}",
            text("title"),
            text("path"),
            served["line"]
        ));
        if served["archived"] == true {
            line.push_str(" | archived");
        }
        out.push(line);
    }
    for edge in map["edges"].as_array().expect("edges") {
        let mut line = format!(
            "{} --{}--> {}",
            names[at(&edge["src"])],
            edge["type"].as_str().expect("a type"),
            names[at(&edge["dst"])]
        );
        if edge["places"].as_u64().expect("places") > 1 {
            line.push_str(&format!(" | places {}", edge["places"]));
        }
        out.push(line);
    }
    for parent in map["parents"].as_array().expect("parents") {
        out.push(format!(
            "{} in {}",
            names[at(&parent["child"])],
            names[at(&parent["parent"])]
        ));
    }
    for ghost in map["ghosts"].as_array().expect("ghosts") {
        let sources: Vec<&str> = ghost["sources"]
            .as_array()
            .expect("sources")
            .iter()
            .map(|source| names[at(source)].as_str())
            .collect();
        out.push(format!(
            "ghost {} | from {} | places {}",
            ghost["written"].as_str().expect("written"),
            sources.join(", "),
            ghost["places"]
        ));
    }
    out
}

/// CM3: the text holds "Data"'s header and lines, every item a line in the
/// JSON's order: nodes, edges, parents, ghosts.
#[test]
fn cm3_the_text_holds_datas_lines() {
    let copy = Copy::new("cm3-text", &[]);
    let run = copy.run(&["map"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines[0],
        "map | nodes 19 (12 documents, 7 sections), edges 35, parents 3, ghosts 0, orphans 1",
        "{}",
        run.stdout
    );
    for line in [
        "docs/features/stamina-tuning.md | - | used by 0, mentioned by 1 | Stamina tuning | docs/features/stamina-tuning.md:1",
        "A-102 | requirement | used by 0, mentioned by 0 | orphan | The lantern burns one unit of oil per minute | docs/records/A/A-102.md:1",
        "RULE-STAM-REGEN | rule | used by 2, mentioned by 3 | of MEC-STAMINA | Regeneration | docs/spec/movement/stamina.md:21",
        "MEC-SPRINT --depends_on--> MEC-STAMINA",
        "DOM-MOVEMENT in DOM-GAME",
        "docs/features/stamina-tuning.md --mentions--> R-12 | places 2",
    ] {
        assert!(lines.contains(&line), "{line:?} not in:\n{}", run.stdout);
    }
    let map = copy.map(false);
    assert_eq!(
        lines[1..],
        lines_of(&map),
        "one line per item, in the JSON's order"
    );
    assert!(run.stdout.ends_with('\n'), "{:?}", run.stdout);

    let archived = copy.run(&["map", "--archive"]);
    archived.code(0);
    let lines: Vec<&str> = archived.stdout.lines().collect();
    assert_eq!(lines[1..], lines_of(&copy.map(true)), "--archive");
    let decision = lines
        .iter()
        .find(|line| line.starts_with("DEC-0007 | decision | used by "))
        .unwrap_or_else(|| panic!("{}", archived.stdout));
    assert!(
        decision.ends_with(
            " | Stamina regenerates while walking | docs/records/DEC/DEC-0007.md:1 | archived"
        ),
        "{decision}"
    );
}

/// CM3: past `OUTPUT_CAP_CHARS` the text keeps its header and whole lines
/// while they fit, then `[truncated: <k> of <n> lines not shown; the
/// daemon's map is whole]`; the JSON holds every node. M: the JSON cut with
/// the text.
#[test]
fn cm3_the_text_is_cut_at_a_line_and_the_json_stays_whole() {
    let copy = Copy::new("cm3-cap", &[]);
    let many = 400;
    for n in 0..many {
        write(
            &copy.root,
            &format!("docs/spec/many/m-{n:03}.md"),
            format!(
                "---\nid: MEC-MANY-{n:03}\nclass: canon\nparent: DOM-MOVEMENT\n---\n\n# Lamp wick {n:03} {}\n\nSee MEC-STAMINA.\n",
                "of the long stair below the lamp room".repeat(2)
            ),
        );
    }
    let map = copy.map(false);
    let all = lines_of(&map);
    let nodes = 19 + many;
    assert_eq!(
        map["nodes"].as_array().expect("nodes").len(),
        nodes,
        "M: the JSON whole"
    );
    assert_eq!(map["counts"]["documents"], 12 + many);
    assert_eq!(map["counts"]["parents"], 3 + many);
    assert_eq!(
        map["edges"].as_array().expect("edges").len(),
        35 + many,
        "each new document's mention of MEC-STAMINA"
    );
    assert_eq!(map["parents"].as_array().expect("parents").len(), 3 + many);
    let run = copy.run(&["map"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let tail = *lines.last().expect("a line");
    let shown = &lines[1..lines.len() - 1];
    assert!(!shown.is_empty(), "at least one line kept");
    assert!(shown.len() < all.len(), "the text is cut");
    assert_eq!(
        tail,
        format!(
            "[truncated: {} of {} lines not shown; the daemon's map is whole]",
            all.len() - shown.len(),
            all.len()
        )
    );
    assert!(lines[0].starts_with("map | nodes 419 ("), "{}", lines[0]);
    assert_eq!(shown, &all[..shown.len()], "whole lines, in order");
    let body: usize = lines[..lines.len() - 1]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(body <= CAP, "{body}");
    assert!(
        body + all[shown.len()].chars().count() + 1 > CAP,
        "the next line would have fit: {body}"
    );
}

/// The files of `from`, sorted, as (relative path, bytes).
fn files_under(from: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let entry = entry.expect("an entry");
            let path = entry.path();
            if entry.file_type().expect("a type").is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).expect("under the root");
                out.push(relative.to_str().expect("UTF-8").to_owned());
            }
        }
    }
    let mut paths = Vec::new();
    walk(from, from, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = read(from, &path);
            (path, bytes)
        })
        .collect()
}

/// The notes' scene: two IDs held twice each (the one later in the
/// alphabet at the earlier paths) and a `parent:` cycle; MEC-SPRINT names
/// MEC-ZED, which lands on its first holder.
const NOTES_SCENE: [(&str, &str); 6] = [
    (
        "docs/spec/a-twins/one.md",
        "---\nid: MEC-ZED\nclass: canon\n---\n\n# Zed one\n",
    ),
    (
        "docs/spec/a-twins/two.md",
        "---\nid: MEC-ZED\nclass: canon\n---\n\n# Zed two\n",
    ),
    (
        "docs/spec/z-twins/one.md",
        "---\nid: MEC-ABLE\nclass: canon\n---\n\n# Able one\n",
    ),
    (
        "docs/spec/z-twins/two.md",
        "---\nid: MEC-ABLE\nclass: canon\n---\n\n# Able two\n",
    ),
    (
        "docs/spec/loop/knot.md",
        "---\nid: MEC-KNOT\nclass: canon\nparent: MEC-LOOP\n---\n\n# Knot\n",
    ),
    (
        "docs/spec/loop/loop.md",
        "---\nid: MEC-LOOP\nclass: canon\nparent: MEC-KNOT\n---\n\n# Loop\n",
    ),
];

/// CM3, "Rules and edge cases": both holders of a shared ID are drawn and
/// `notes` says so, an edge to it lands on the first by (path, ord); the
/// cycle is noted; `notes` sorted, each once. M: `notes` in input order.
#[test]
fn cm3_notes_are_sorted_and_once_and_twins_are_both_drawn() {
    let copy = Copy::new("cm3-notes", &NOTES_SCENE);
    replace(
        &copy.root,
        "docs/spec/movement/sprint.md",
        "depends_on: [MEC-STAMINA]",
        "depends_on: [MEC-STAMINA, MEC-ZED]",
    );
    let map = copy.map(false);
    let served = names(&map);
    for twin in ["MEC-ZED", "MEC-ABLE"] {
        assert_eq!(
            served.iter().filter(|name| *name == twin).count(),
            2,
            "both holders drawn: {served:?}"
        );
    }
    let notes: Vec<String> = map["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect();
    let mut sorted = notes.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(notes, sorted, "sorted, each once");
    for word in ["MEC-ZED", "MEC-ABLE", "MEC-LOOP"] {
        assert!(
            notes.iter().any(|note| note.contains(word)),
            "a note names {word}: {notes:?}"
        );
    }
    assert!(notes.len() >= 3, "{notes:?}");
    let first = served
        .iter()
        .position(|name| name == "MEC-ZED")
        .expect("MEC-ZED");
    assert_eq!(map["nodes"][first]["path"], "docs/spec/a-twins/one.md");
    let sprint = index_of(&map, "MEC-SPRINT");
    let lands: Vec<usize> = map["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .filter(|edge| edge["src"] == json!(sprint) && edge["type"] == "depends_on")
        .map(|edge| at(&edge["dst"]))
        .collect();
    assert_eq!(
        lands,
        [first, index_of(&map, "MEC-STAMINA")],
        "the edge to MEC-ZED lands on its first holder"
    );
}

/// CM3: the copy (CM2's ghost and the notes' scene in it) at two roots and
/// two `HOME`s, its files written in opposite orders, gives byte-identical
/// text and JSON, with and without `--archive`.
#[test]
fn cm3_one_corpus_one_answer() {
    let scene = ghost_copy("cm3-source");
    let mut files = files_under(&scene.root);
    files.extend(
        NOTES_SCENE
            .iter()
            .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec())),
    );
    files.sort();
    let scratch = Scratch::new("cm3-twice");
    let first = scratch.dir("one/deeper/copy");
    for (path, bytes) in &files {
        write(&first, path, bytes);
    }
    let second = scratch.dir("two");
    for (path, bytes) in files.iter().rev() {
        write(&second, path, bytes);
    }
    let homes = [scratch.home("first"), scratch.home("second")];
    for args in [
        &["map"][..],
        &["map", "--json"],
        &["map", "--archive"],
        &["map", "--archive", "--json"],
    ] {
        let one = spec(&homes[0], &first, args);
        let two = spec(&homes[1], &second, args);
        one.code(0);
        assert_eq!(
            (two.code, &two.stdout, &two.stderr),
            (one.code, &one.stdout, &one.stderr),
            "{args:?}"
        );
        assert!(one.stdout.contains("MEC-GONE"), "{args:?}: {}", one.stdout);
    }
}

/// CM3 ("Data", the exits): zero nodes is an answer (exit 0); a stray
/// argument or flag, no project, a config that does not read and no `HOME`
/// exit 2; never 1.
#[test]
fn cm3_exits() {
    let scratch = Scratch::new("cm3-exits");
    let home = scratch.home("h");
    let empty = scratch.dir("empty");
    write(
        &empty,
        "specengine.toml",
        "[project]\nslug = \"empty-keep\"\n",
    );
    let run = spec(&home, &empty, &["map"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "map | nodes 0 (0 documents, 0 sections), edges 0, parents 0, ghosts 0, orphans 0\n",
        "{}",
        run.show()
    );
    let run = spec(&home, &empty, &["map", "--json"]);
    run.code(0);
    assert_eq!(
        run.json(),
        json!({"archive": false, "notes": [], "counts": {"documents": 0, "sections": 0, "edges": 0, "parents": 0, "ghosts": 0, "orphans": 0}, "nodes": [], "edges": [], "parents": [], "ghosts": []})
    );

    let copy = Copy::new("cm3-usage", &[]);
    for args in [
        &["map", "MEC-STAMINA"][..],
        &["map", "--depth", "1"],
        &["map", "--kind", "mechanic"],
    ] {
        let run = copy.run(args);
        run.code(2);
        assert!(run.stdout.is_empty(), "{}", run.show());
    }
    let nowhere = scratch.dir("nowhere");
    spec(&home, &nowhere, &["map"]).code(2);
    let broken = scratch.dir("broken");
    write(&broken, "specengine.toml", "[[[ broken\n");
    spec(&home, &broken, &["map", "--json"]).code(2);
    let homeless = spec_with(
        &copy.root,
        &["map"],
        &[("PATH", OsStr::new("/usr/bin:/bin"))],
    );
    homeless.code(2);
}

/// CM3 at a forced small cap (`MapOutcome::text_within`): the header, at
/// least one line, whole lines while they fit, then the tail naming the
/// rest; the outcome itself (the JSON's) keeps every node. M: the JSON cut
/// with the text.
#[test]
fn cm3_a_forced_small_cap() {
    let copy = Copy::new("cm3-small-cap", &[]);
    let env = specengine_cli::Env {
        cwd: copy.root.clone(),
        home: Some(copy.home.clone().into_os_string()),
        xdg_data_home: None,
    };
    let outcome = specengine_cli::map(
        &env,
        &specengine_cli::Globals::default(),
        &specengine_cli::MapRequest { archive: false },
    )
    .unwrap_or_else(|error| panic!("map: {error}"));
    assert_eq!(outcome.rows.len(), 19, "the outcome is whole");
    let all = lines_of(&copy.map(false));
    let header =
        "map | nodes 19 (12 documents, 7 sections), edges 35, parents 3, ghosts 0, orphans 1";
    for cap in [1, 200, 1_000] {
        let text = outcome.text_within(cap);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], header, "{cap}");
        let shown = &lines[1..lines.len() - 1];
        assert!(!shown.is_empty(), "{cap}: at least one line");
        assert_eq!(shown, &all[..shown.len()], "{cap}");
        assert_eq!(
            *lines.last().expect("the tail"),
            format!(
                "[truncated: {} of {} lines not shown; the daemon's map is whole]",
                all.len() - shown.len(),
                all.len()
            ),
            "{cap}"
        );
        if shown.len() > 1 {
            let body: usize = lines[..lines.len() - 1]
                .iter()
                .map(|line| line.chars().count() + 1)
                .sum();
            assert!(body <= cap, "{cap}: {body}");
            assert!(
                body + all[shown.len()].chars().count() + 1 > cap,
                "{cap}: the next line would have fit"
            );
        }
    }
    let whole = outcome.text_within(CAP);
    assert_eq!(whole, copy.run(&["map"]).stdout, "the CLI cuts at the cap");
}

/// The holders' scene: DEC-0030 held by a Tier 3 decision sorted first and
/// by a live one; a document under each of DEC-0030 and DEC-0007 (held by
/// a Tier 3 file alone); MEC-SPRINT depending on both.
const HOLDERS_SCENE: [(&str, &str); 4] = [
    (
        "docs/records/DEC/DEC-0030-old.md",
        "---\nid: DEC-0030\nclass: decision\nstatus: superseded-by DEC-0023\n---\n\n# The old gate rule\n",
    ),
    (
        "docs/records/DEC/DEC-0030.md",
        "---\nid: DEC-0030\nclass: decision\nstatus: accepted\n---\n\n# The gate rule\n",
    ),
    (
        "docs/spec/gate.md",
        "---\nid: MEC-GATE\nclass: canon\nparent: DEC-0030\n---\n\n# Gate\n",
    ),
    (
        "docs/spec/lamp.md",
        "---\nid: MEC-LAMP\nclass: canon\nparent: DEC-0007\n---\n\n# Lamp\n",
    ),
];

/// CM1, the ruling of iteration 2: an edge end or a `parent:` with several
/// holders lands on the first admitted holder by (path, ord): without
/// `--archive` the live DEC-0030, with it the Tier 3 one sorted first; with
/// no holder admitted (DEC-0007 without `--archive`) nothing is drawn. M:
/// back to the first holder.
#[test]
fn cm1_an_end_lands_on_the_first_admitted_holder() {
    let copy = Copy::new("cm1-holders", &HOLDERS_SCENE);
    replace(
        &copy.root,
        "docs/spec/movement/sprint.md",
        "depends_on: [MEC-STAMINA]",
        "depends_on: [MEC-STAMINA, DEC-0030, DEC-0007]",
    );
    for (archive, holder) in [
        (false, "docs/records/DEC/DEC-0030.md"),
        (true, "docs/records/DEC/DEC-0030-old.md"),
    ] {
        let map = copy.map(archive);
        let nodes = map["nodes"].as_array().expect("nodes");
        let at_path = |path: &str| {
            nodes
                .iter()
                .position(|node| node["path"] == path)
                .unwrap_or_else(|| panic!("{path}: {map}"))
        };
        let landed = at_path(holder);
        let sprint = index_of(&map, "MEC-SPRINT");
        let depends: Vec<usize> = map["edges"]
            .as_array()
            .expect("edges")
            .iter()
            .filter(|edge| edge["src"] == json!(sprint) && edge["type"] == "depends_on")
            .map(|edge| at(&edge["dst"]))
            .collect();
        let gate = index_of(&map, "MEC-GATE");
        let lamp = index_of(&map, "MEC-LAMP");
        let parent_of = |child: usize| {
            map["parents"]
                .as_array()
                .expect("parents")
                .iter()
                .find(|parent| parent["child"] == json!(child))
                .map(|parent| at(&parent["parent"]))
        };
        assert_eq!(parent_of(gate), Some(landed), "archive {archive}: {map}");
        if archive {
            let old = at_path("docs/records/DEC/DEC-0007.md");
            let mut want = vec![landed, old, index_of(&map, "MEC-STAMINA")];
            want.sort_unstable();
            assert_eq!(depends, want, "archive: {map}");
            assert_eq!(parent_of(lamp), Some(old), "archive: {map}");
        } else {
            let mut want = vec![landed, index_of(&map, "MEC-STAMINA")];
            want.sort_unstable();
            assert_eq!(depends, want, "no holder of DEC-0007 admitted: {map}");
            assert_eq!(parent_of(lamp), None, "no holder admitted: {map}");
            assert!(
                nodes
                    .iter()
                    .all(|node| node["path"] != "docs/records/DEC/DEC-0030-old.md"),
                "{map}"
            );
        }
    }
}

/// Iteration 2's stderr: the notes first, then the warnings, each group
/// sorted, each line escaped; the JSON's `notes` every one of them, sorted,
/// each once.
#[test]
fn cm3_stderr_holds_the_notes_then_the_warnings() {
    let copy = Copy::new("cm3-stderr", &NOTES_SCENE);
    let run = copy.run(&["map"]);
    run.code(0);
    let lines = run.stderr_lines();
    let notes: Vec<&str> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("note: "))
        .collect();
    let warnings: Vec<&str> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("warning: "))
        .collect();
    assert_eq!(notes.len() + warnings.len(), lines.len(), "{}", run.show());
    assert!(
        lines[..notes.len()]
            .iter()
            .all(|line| line.starts_with("note: ")),
        "notes first: {}",
        run.show()
    );
    for group in [&notes, &warnings] {
        let mut sorted = group.to_vec();
        sorted.sort_unstable();
        assert_eq!(*group, sorted, "{}", run.show());
    }
    for word in ["`MEC-ABLE` has 2 holders", "`MEC-ZED` has 2 holders"] {
        assert!(
            warnings.iter().any(|warning| warning.starts_with(word)),
            "{word}: {}",
            run.show()
        );
    }
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("`parent:` forms a cycle through ")),
        "{}",
        run.show()
    );
    let mut all: Vec<String> = notes
        .iter()
        .chain(&warnings)
        .map(|text| (*text).to_owned())
        .collect();
    all.sort();
    all.dedup();
    assert_eq!(copy.map(false)["notes"], json!(all), "the JSON's notes");
}

/// The names' scene (corpus-map iteration 3): two features holding the
/// feature-scoped AC-01 once each; `lamp-care` holding AC-02 twice and the
/// project-scoped MEC-LANTERN, which `docs/spec/lantern.md` holds too; a
/// project-scoped RULE-WICK held once, in a feature; MEC-ZED held twice
/// outside the features.
const NAMES_SCENE: [(&str, &str); 6] = [
    (
        "docs/features/lamp-care.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Lamp care\n\n## Wick trimmed {#AC-01}\n\nTrim daily.\n\n## Glass clean {#AC-02}\n\nWipe at dusk.\n\n## Glass dry {#AC-02}\n\nDry after rain.\n\n## The lantern {#MEC-LANTERN}\n\nBurns oil; see MEC-SPRINT.\n\n## Wick length {#RULE-WICK}\n\nTwo fingers.\n",
    ),
    (
        "docs/features/oil-store.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Oil store\n\n## Barrels sealed {#AC-01}\n\nSeal them.\n",
    ),
    (
        "docs/spec/lantern.md",
        "---\nid: MEC-LANTERN\nclass: canon\n---\n\n# Lantern\n",
    ),
    (
        "docs/spec/a-twins/one.md",
        "---\nid: MEC-ZED\nclass: canon\n---\n\n# Zed one\n",
    ),
    (
        "docs/spec/a-twins/two.md",
        "---\nid: MEC-ZED\nclass: canon\n---\n\n# Zed two\n",
    ),
    (
        "docs/records/A/A-104.md",
        "---\nid: A-104\nclass: canon\n---\n\n# Oil lasts a night\n\nSee lamp-care/AC-02 and oil-store/AC-01.\n",
    ),
];

/// The names' scene mapped: the JSON, and the stderr warnings of the text
/// run, the `warning: ` dropped.
fn names_scene(label: &str) -> (Copy, Value, Vec<String>) {
    let copy = Copy::new(label, &NAMES_SCENE);
    let map = copy.map(false);
    let run = copy.run(&["map"]);
    run.code(0);
    let warnings = run
        .stderr_lines()
        .iter()
        .filter_map(|line| line.strip_prefix("warning: "))
        .map(str::to_owned)
        .collect();
    (copy, map, warnings)
}

/// The served nodes as `(name, path)`.
fn named(map: &Value) -> Vec<(String, String)> {
    map["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                node["name"].as_str().expect("a name").to_owned(),
                node["path"].as_str().expect("a path").to_owned(),
            )
        })
        .collect()
}

/// The warnings naming `name` as a shared name.
fn shared<'a>(warnings: &'a [String], name: &str) -> Vec<&'a String> {
    let opening = format!("`{name}` has ");
    warnings
        .iter()
        .filter(|warning| warning.starts_with(&opening))
        .collect()
}

/// Iteration 3: a feature-scoped ID held once in each of two features is two
/// `slug/ID` names and no warning; the edges name them so. M: the warning
/// keyed on the bare ID.
#[test]
fn cm1_a_feature_scoped_id_in_two_features_is_two_names_and_no_warning() {
    let (copy, map, warnings) = names_scene("cm1-names-features");
    let nodes = named(&map);
    for want in [
        ("lamp-care/AC-01", "docs/features/lamp-care.md"),
        ("oil-store/AC-01", "docs/features/oil-store.md"),
        ("stamina-tuning/AC-07", "docs/features/stamina-tuning.md"),
    ] {
        assert!(
            nodes.contains(&(want.0.to_owned(), want.1.to_owned())),
            "{want:?}: {nodes:?}"
        );
    }
    assert!(
        !nodes
            .iter()
            .any(|(name, _)| name == "AC-01" || name == "AC-07"),
        "{nodes:?}"
    );
    for name in ["AC-01", "lamp-care/AC-01", "oil-store/AC-01"] {
        assert_eq!(
            shared(&warnings, name),
            Vec::<&String>::new(),
            "{warnings:?}"
        );
    }
    let served = served_edges(&map);
    for target in ["lamp-care/AC-02", "oil-store/AC-01"] {
        assert!(
            served.contains_key(&("A-104".to_owned(), target.to_owned(), WEAK.to_owned())),
            "{target}: {served:?}"
        );
    }
    let text = copy.run(&["map"]);
    assert!(
        text.stdout
            .lines()
            .any(|line| line == "MEC-SPRINT --mentions--> stamina-tuning/AC-07"),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout
            .lines()
            .any(|line| line.starts_with("oil-store/AC-01 | criterion | used by ")),
        "{}",
        text.stdout
    );
}

/// Iteration 3: an ID held twice inside one feature is one shared name,
/// `slug/ID`, and warns under it, naming both places. M: no warning for a
/// `slug/ID` name.
#[test]
fn cm1_a_duplicate_inside_one_feature_warns_under_its_slug() {
    let (copy, map, warnings) = names_scene("cm1-names-twice");
    let twice: Vec<(String, String)> = named(&map)
        .into_iter()
        .filter(|(name, _)| name == "lamp-care/AC-02")
        .collect();
    assert_eq!(twice.len(), 2, "{twice:?}");
    let lines: Vec<u64> = map["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|node| node["name"] == "lamp-care/AC-02")
        .map(|node| node["line"].as_u64().expect("a line"))
        .collect();
    assert_eq!(
        shared(&warnings, "lamp-care/AC-02"),
        [&format!(
            "`lamp-care/AC-02` has 2 holders, each mapped, its links on the first: \
             docs/features/lamp-care.md:{}, docs/features/lamp-care.md:{}",
            lines[0], lines[1]
        )],
        "{warnings:?}"
    );
    assert!(
        map["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| note
                .as_str()
                .is_some_and(|note| note.starts_with("`lamp-care/AC-02` has 2 holders"))),
        "{map}"
    );
    assert_eq!(shared(&warnings, "AC-02"), Vec::<&String>::new());
    drop(copy);
}

/// Iteration 3: a project-scoped ID held in a feature and elsewhere is
/// `slug/ID` in the feature and bare outside, two names and no warning; held
/// once in a feature, it stays bare. M: `slug/` for feature-scoped IDs
/// alone.
#[test]
fn cm1_a_project_scoped_id_repeated_in_a_feature_is_named_by_its_slug() {
    let (copy, map, warnings) = names_scene("cm1-names-project");
    let nodes = named(&map);
    for want in [
        ("lamp-care/MEC-LANTERN", "docs/features/lamp-care.md"),
        ("MEC-LANTERN", "docs/spec/lantern.md"),
        ("RULE-WICK", "docs/features/lamp-care.md"),
    ] {
        assert!(
            nodes.contains(&(want.0.to_owned(), want.1.to_owned())),
            "{want:?}: {nodes:?}"
        );
    }
    for name in ["MEC-LANTERN", "lamp-care/MEC-LANTERN"] {
        assert_eq!(
            shared(&warnings, name),
            Vec::<&String>::new(),
            "{warnings:?}"
        );
    }
    let served = served_edges(&map);
    assert!(
        served.contains_key(&(
            "lamp-care/MEC-LANTERN".to_owned(),
            "MEC-SPRINT".to_owned(),
            WEAK.to_owned()
        )),
        "{served:?}"
    );
    drop(copy);
}

/// Iteration 3: an ID held twice outside the features keeps its bare name on
/// both and still warns. M: no warning for a bare name.
#[test]
fn cm1_a_bare_duplicate_outside_features_still_warns() {
    let (copy, map, warnings) = names_scene("cm1-names-bare");
    let twice: Vec<(String, String)> = named(&map)
        .into_iter()
        .filter(|(name, _)| name == "MEC-ZED")
        .collect();
    assert_eq!(
        twice,
        [
            ("MEC-ZED".to_owned(), "docs/spec/a-twins/one.md".to_owned()),
            ("MEC-ZED".to_owned(), "docs/spec/a-twins/two.md".to_owned()),
        ]
    );
    assert_eq!(
        shared(&warnings, "MEC-ZED"),
        [
            &"`MEC-ZED` has 2 holders, each mapped, its links on the first: \
           docs/spec/a-twins/one.md:1, docs/spec/a-twins/two.md:1"
                .to_owned()
        ],
        "{warnings:?}"
    );
    drop(copy);
}
