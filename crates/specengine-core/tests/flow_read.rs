//! docs/features/flow-read.md (RM-0043, ADR-0047, ADR-0055), core: the pure
//! half of `spec flow` on the real parser and the real spec graph, over
//! `fixtures/flow-gaps`, `fixtures/flow-relay` and `fixtures/flow-hire`, each
//! read through its own `specengine.toml` alone (ADR-0008):
//!
//! - "Data": `FLOW_LINK_TYPES`, the `[flow]` tables, a bad table's first
//!   error by line, the rows of `expected.json`;
//! - AC-01: no order of the input changes a node, an edge or a row;
//! - AC-05: a depth changes no row;
//! - a seeded brute-force reading of the graph rows, whole and under a ROOT,
//!   from the spec's definitions alone: transitive closures, no Tarjan;
//! - the walks are no recursion: a ring and a `parent:` chain of thousands;
//! - docs/features/flow-linear.md AC-10: a `parent:` chain of 20 000, and
//!   a read linear in the chain's depth, unbounded and at `--depth 0`;
//! - docs/features/trace-views.md "Folded start and end": at every depth a
//!   shown node's `start` and `end` over what it holds, its `stage` its own,
//!   read from the unfolded read alone.
//!
//! The reads, exits and surfaces are CLI `tests/flow_read.rs`, MCP
//! `tests/mcp_flow.rs` and http `tests/flow.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::hint::black_box;
use std::time::{Duration, Instant};

use common::check::{Config, fixture_input};
use common::{corpus_scheme, fixture, render_diagnostics, render_link, walked_md_files};
use serde_json::Value;
use specengine_core::check::{CheckInput, NodeAt, SpecGraph};
use specengine_core::{
    FlowAsk, FlowConfig, FlowNode, FlowRead, FlowTable, GapCause, GapCode, GapState, ProjectConfig,
    flow_read,
};
use specengine_model::{Direction, FLOW_LINK_TYPES, FOLLOWS, IdScheme, LINK_TYPES};

const GAPS: &str = "flow-gaps";
const RELAY: &str = "flow-relay";
const HIRE: &str = "flow-hire";
const ALMANAC: &str = "trace-almanac";

/// How a test names the config in a cause.
const LABEL: &str = "the-config.toml";

// ------------------------------------------------------------------ helpers

fn config_text(corpus: &str) -> String {
    fs::read_to_string(fixture(corpus).join("specengine.toml")).expect("specengine.toml")
}

fn expected() -> Value {
    serde_json::from_str(
        &fs::read_to_string(fixture(GAPS).join("expected.json")).expect("expected.json"),
    )
    .expect("JSON")
}

/// The one node named `name`.
fn find(graph: &SpecGraph<'_>, name: &str) -> NodeAt {
    let mut found = Vec::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            if graph.name(at) == name {
                found.push(at);
            }
        }
    }
    assert_eq!(found.len(), 1, "{name}: {found:?}");
    found[0]
}

/// The flow of a parsed corpus under `text`'s `[flow]`.
fn read_input(
    text: &str,
    config: &Config,
    input: &CheckInput,
    root: Option<&str>,
    depth: Option<usize>,
) -> FlowRead {
    let graph = SpecGraph::new(input, &config.scheme, &config.paths);
    let flow = FlowConfig::from_toml(text, &config.scheme);
    let roots: Option<Vec<NodeAt>> = root.map(|name| vec![find(&graph, name)]);
    flow_read(
        &graph,
        &FlowAsk {
            roots: roots.as_deref(),
            depth,
            config: &flow,
            config_label: LABEL,
            scope_name: root.unwrap_or("the-whole"),
        },
    )
}

/// The flow of a fixture.
fn read(corpus: &str, root: Option<&str>, depth: Option<usize>) -> FlowRead {
    let (config, input) = fixture_input(&fixture(corpus));
    read_input(&config_text(corpus), &config, &input, root, depth)
}

/// `(code, state, count, items)` of every row, in order.
type Row = (String, String, Option<usize>, Vec<Vec<String>>);

fn rows(read: &FlowRead) -> Vec<Row> {
    read.gaps
        .iter()
        .map(|gap| {
            (
                gap.code.as_str().to_owned(),
                gap.state.as_str().to_owned(),
                gap.count,
                gap.items.clone(),
            )
        })
        .collect()
}

/// The row of `code`.
fn row(read: &FlowRead, code: GapCode) -> &specengine_core::FlowGap {
    read.gaps
        .iter()
        .find(|gap| gap.code == code)
        .unwrap_or_else(|| panic!("no row {code:?}"))
}

/// Nodes and edges as text, for a comparison across inputs.
fn shape(read: &FlowRead) -> (Vec<String>, Vec<String>) {
    (
        read.nodes.iter().map(|node| format!("{node:?}")).collect(),
        read.edges.iter().map(|edge| format!("{edge:?}")).collect(),
    )
}

fn table(text: &str, scheme: &IdScheme) -> FlowConfig {
    FlowConfig::from_toml(text, scheme)
}

// ------------------------------------------------- "Folded start and end"

/// A shown node's name, as `group` names it.
fn name_of(node: &FlowNode) -> &str {
    node.id.as_deref().unwrap_or(&node.path)
}

/// docs/features/trace-views.md "Folded start and end", brute force: the
/// unfolded read gives each document its marks, its stage and its place in the
/// tree; at every depth a shown node's `start` and `end` are true exactly when
/// it or a document folded into it carries one, and its `stage` is its own.
/// Gap rows are no business of a fold. M: `steps.starts.contains(&file)` kept
/// in `shown`.
#[test]
fn a_fold_marks_a_start_and_an_end_by_what_it_holds() {
    for (corpus, root) in [
        (GAPS, Some("WING-LIBRARY")),
        (GAPS, Some("WING-BROKEN")),
        (GAPS, None),
        (RELAY, None),
        (HIRE, None),
        (ALMANAC, None),
        (ALMANAC, Some("BOOK-ALMANAC")),
        (ALMANAC, Some("PART-PRINT")),
    ] {
        let whole = read(corpus, root, None);
        let above: BTreeMap<&str, &str> = whole
            .nodes
            .iter()
            .filter_map(|node| Some((name_of(node), node.group.as_deref()?)))
            .collect();
        let level = |name: &str| {
            let (mut at, mut levels) = (name, 0);
            while let Some(parent) = above.get(at) {
                at = parent;
                levels += 1;
            }
            levels
        };
        let shown_as = |name: &'_ str, depth: usize| -> String {
            let mut at = name.to_owned();
            while level(&at) > depth {
                at = above[at.as_str()].to_owned();
            }
            at
        };
        let deepest = whole
            .nodes
            .iter()
            .map(|node| level(name_of(node)))
            .max()
            .unwrap_or(0);
        for depth in 0..=deepest + 1 {
            let context = format!("{corpus} {root:?} at depth {depth}");
            let folded = read(corpus, root, Some(depth));
            assert_eq!(rows(&folded), rows(&whole), "{context}");
            let mut seen = 0;
            for node in &folded.nodes {
                let held: Vec<&FlowNode> = whole
                    .nodes
                    .iter()
                    .filter(|document| shown_as(name_of(document), depth) == name_of(node))
                    .collect();
                assert!(!held.is_empty(), "{context}: {node:?}");
                seen += held.len();
                assert_eq!(
                    node.start,
                    held.iter().any(|document| document.start),
                    "{context}: {node:?}"
                );
                assert_eq!(
                    node.end,
                    held.iter().any(|document| document.end),
                    "{context}: {node:?}"
                );
                let own = held
                    .iter()
                    .find(|document| name_of(document) == name_of(node))
                    .expect("a shown node holds itself");
                assert_eq!(node.stage, own.stage, "{context}: {node:?}");
            }
            assert_eq!(seen, whole.nodes.len(), "{context}: nothing lost");
        }
    }
}

// ------------------------------------------------------------------- "Data"

/// "Data": the table, verbatim, each direction reading the link; every type
/// a member of `LINK_TYPES`, so the read invents none.
#[test]
fn the_flow_link_types_are_the_specs() {
    assert_eq!(
        FLOW_LINK_TYPES,
        [
            (FOLLOWS, Direction::In),
            ("uses", Direction::Out),
            ("supplies", Direction::Out),
            ("depends_on", Direction::Out),
        ]
    );
    for (link_type, _) in FLOW_LINK_TYPES {
        assert!(LINK_TYPES.contains(&link_type), "{link_type}");
    }
}

/// "Fixture": flow-gaps parses as `expected.json` says: IDs, kinds, titles,
/// sections, `parent:`, every link written, no diagnostic.
#[test]
fn the_lending_corpus_reads_as_expected() {
    let corpus = fixture(GAPS);
    let scheme = corpus_scheme(&corpus);
    let expected = expected();
    let files = expected["files"].as_object().expect("files");
    let mut seen = BTreeSet::new();
    for (path, bytes) in walked_md_files(&corpus) {
        let want = files
            .get(&path)
            .unwrap_or_else(|| panic!("{path} is not in expected.json"));
        let parsed = specengine_core::parse(&path, &bytes, &scheme);
        assert_eq!(render_diagnostics(&parsed), Vec::<String>::new(), "{path}");
        let node = parsed.document().expect("a document");
        assert_eq!(node.id.as_deref(), want["id"].as_str(), "{path}");
        assert_eq!(node.kind.as_deref(), want["kind"].as_str(), "{path}");
        assert_eq!(node.title.as_deref(), want["title"].as_str(), "{path}");
        assert_eq!(
            node.parent.as_ref().map(|parent| parent.id.as_str()),
            want["parent"].as_str(),
            "{path}"
        );
        assert_eq!(
            path,
            format!(
                "docs/spec/{}.md",
                want["id"].as_str().expect("an ID").to_lowercase()
            ),
            "one document per node at its lowercase ID"
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
        assert_eq!(sections, want_sections, "{path}");
        let links: Vec<String> = parsed
            .links
            .iter()
            .map(|link| {
                let mut line = render_link(link);
                if let specengine_model::LinkTarget::Reference(reference) = &link.dst
                    && let Some(edge) = &reference.edge
                {
                    if let Some(kind) = edge.kind {
                        line.push_str(&format!(" | kind {}", kind.as_str()));
                    }
                    if let Some(when) = &edge.when {
                        line.push_str(&format!(" | when {when}"));
                    }
                }
                line
            })
            .collect();
        let want_links: Vec<String> = want["links"]
            .as_array()
            .expect("links")
            .iter()
            .map(|link| link.as_str().expect("a string").to_owned())
            .collect();
        assert_eq!(links, want_links, "{path}");
        seen.insert(path);
    }
    assert_eq!(seen.len(), 29, "the fixture's documents");
    assert_eq!(
        seen,
        files.keys().cloned().collect::<BTreeSet<_>>(),
        "expected.json names every document"
    );
}

/// "Data": every scope's rows are `expected.json`'s, the state always, the
/// count and items of a measured row.
#[test]
fn every_scope_gives_the_rows_of_expected_json() {
    let expected = expected();
    for (scope, root) in [
        ("all", None),
        ("WING-LIBRARY", Some("WING-LIBRARY")),
        ("WING-LEND", Some("WING-LEND")),
        ("WING-BROKEN", Some("WING-BROKEN")),
        ("WING-HEADLESS", Some("WING-HEADLESS")),
    ] {
        let got = read(GAPS, root, None);
        let want = expected["gaps"][scope].as_array().expect("rows");
        assert_eq!(got.gaps.len(), 10, "{scope}");
        for (gap, want) in got.gaps.iter().zip(want) {
            assert_eq!(gap.code.as_str(), want["code"], "{scope}");
            assert_eq!(gap.state.as_str(), want["state"], "{scope}: {gap:?}");
            if gap.state == GapState::Measured {
                assert_eq!(
                    gap.count,
                    want["count"].as_u64().map(|n| n as usize),
                    "{scope}: {gap:?}"
                );
                let items: Vec<Vec<String>> = want["items"]
                    .as_array()
                    .expect("items")
                    .iter()
                    .map(|item| {
                        item.as_array()
                            .expect("an item")
                            .iter()
                            .map(|name| name.as_str().expect("a name").to_owned())
                            .collect()
                    })
                    .collect();
                assert_eq!(gap.items, items, "{scope}: {gap:?}");
                // docs/features/flow-authoring.md D11: a code above 0, for
                // the CLI to word; `None` at 0.
                let cause = (gap.count != Some(0)).then_some(GapCause::Found(gap.code));
                assert_eq!(gap.cause, cause, "{scope}: {gap:?}");
            }
        }
    }
}

/// The rows name the scope and the config the way the caller does: the core
/// knows no file name and no word for "all".
#[test]
fn the_caller_names_the_scope_and_the_config() {
    let got = read(RELAY, None, None);
    assert_eq!(row(&got, GapCode::NoStart).state, GapState::NotConfigured);
    let (config, input) = fixture_input(&fixture(RELAY));
    let text = format!(
        "{}\n[[flow.start]]\npaths = [\"docs/spec/nothing.md\"]\n",
        config_text(RELAY)
    );
    let got = read_input(&text, &config, &input, None, None);
    assert_eq!(
        row(&got, GapCode::NoStart).items,
        [["the-whole".to_owned()]],
        "the name the caller gave"
    );
}

/// "Data": the ten rows in table order, each corpus, each depth; a depth
/// changes none.
#[test]
fn a_depth_changes_no_row() {
    for corpus in [GAPS, RELAY, HIRE] {
        let whole = rows(&read(corpus, None, None));
        assert_eq!(
            whole.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
            [
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
            ],
            "{corpus}"
        );
        for depth in [0, 1, 2, 3, 100] {
            assert_eq!(
                rows(&read(corpus, None, Some(depth))),
                whole,
                "{corpus} depth {depth}"
            );
        }
    }
}

/// AC-01: no order of the input changes a node, an edge or a row.
#[test]
fn no_order_of_the_input_changes_the_read() {
    for (corpus, root) in [
        (GAPS, None),
        (GAPS, Some("WING-BROKEN")),
        (GAPS, Some("WING-LIBRARY")),
        (RELAY, None),
        (HIRE, None),
    ] {
        for depth in [None, Some(1)] {
            let (config, input) = fixture_input(&fixture(corpus));
            let text = config_text(corpus);
            let want = read_input(&text, &config, &input, root, depth);
            let count = input.files.len();
            let mut orders: Vec<Vec<usize>> = vec![(0..count).rev().collect()];
            for step in [3, 7, 11] {
                orders.push((0..count).map(|at| (at + step) % count).collect());
            }
            for order in orders {
                let (_, mut files) = fixture_input(&fixture(corpus));
                let mut taken: Vec<_> = files.files.drain(..).map(Some).collect();
                let shuffled = CheckInput {
                    files: order
                        .iter()
                        .map(|at| taken[*at].take().expect("once"))
                        .collect(),
                    problems: Vec::new(),
                };
                let got = read_input(&text, &config, &shuffled, root, depth);
                assert_eq!(rows(&got), rows(&want), "{corpus} {root:?} {depth:?}");
                assert_eq!(shape(&got), shape(&want), "{corpus} {root:?} {depth:?}");
            }
        }
    }
}

// ------------------------------------------------------------------- [flow]

/// "Data": no table, an empty one and an empty list are no rule, no error:
/// the caller's `not_configured`.
#[test]
fn no_table_is_no_rule() {
    let scheme = corpus_scheme(&fixture(GAPS));
    for text in [
        "",
        "[flow]\n",
        "[flow]\nend = []\n",
        "[flow]\nstep = []\nstart = []\nend = []\ndefinition = []\n",
    ] {
        let flow = table(text, &scheme);
        assert_eq!(flow, FlowConfig::default(), "{text:?}");
    }
}

/// "Data": each table lists its rules in the order written, each rule's
/// selectors as given.
#[test]
fn each_table_reads_its_rules_in_order() {
    let flow = FlowConfig::from_toml(&config_text(GAPS), &corpus_scheme(&fixture(GAPS)));
    for got in [&flow.step, &flow.start, &flow.end, &flow.definition] {
        assert_eq!(got.error, None);
        assert_eq!(got.rules.len(), 1);
    }
    assert_eq!(flow.step.rules[0].kinds, ["lending"]);
    assert_eq!(
        flow.start.rules[0].paths,
        [
            "docs/spec/lnd-ask.md",
            "docs/spec/lnd-a.md",
            "docs/spec/lnd-b.md"
        ]
    );
    assert_eq!(flow.definition.rules[0].kinds, ["bookplate"]);

    let scheme = corpus_scheme(&fixture(GAPS));
    let flow = table(
        "[[flow.end]]\nkinds = [\"lending\"]\n\n[[flow.end]]\nclasses = [\"canon\"]\n\
         paths = [\"docs/**\"]\nwhen = { phase = [\"last\", \"late\"] }\n",
        &scheme,
    );
    assert_eq!(flow.end.error, None);
    assert_eq!(flow.end.rules.len(), 2);
    assert_eq!(flow.end.rules[0].line, 1);
    assert_eq!(flow.end.rules[1].line, 4);
    assert_eq!(
        flow.end.rules[1].when,
        [(
            "phase".to_owned(),
            vec!["last".to_owned(), "late".to_owned()]
        )]
    );
    assert_eq!(flow.start, FlowTable::default());
}

/// "Data": a bad table's first error by line; the other tables read on.
#[test]
fn a_bad_table_keeps_its_first_error_and_leaves_the_others() {
    let scheme = corpus_scheme(&fixture(GAPS));
    for (text, line, fragment) in [
        (
            "[[flow.end]]\n",
            1,
            "without a selector: give `kinds`, `classes`, `paths` or `when`",
        ),
        (
            "[[flow.end]]\nkinds = [\"lending\"]\nflavour = 1\n",
            3,
            "unknown field `flavour`",
        ),
        (
            "[[flow.end]]\nkinds = [\"nothing\"]\n",
            2,
            "`nothing` is the kind of no `[ids]` prefix",
        ),
        (
            "[[flow.end]]\nclasses = [\"record\"]\n",
            2,
            "`record` is none of canon | decision | spec | generated",
        ),
        ("[[flow.end]]\npaths = [\"docs/../x.md\"]\n", 2, "`paths`"),
        ("[[flow.end]]\nwhen = {}\n", 2, "`when` is empty"),
        ("[[flow.end]]\nkinds = 3\n", 2, "`kinds`"),
        // The first by line, a bad kind above an unknown key.
        (
            "[[flow.end]]\nkinds = [\"nothing\"]\nflavour = 1\n",
            2,
            "`nothing`",
        ),
        (
            "[[flow.end]]\nkinds = [\"lending\"]\n\n[[flow.end]]\n\n[[flow.end]]\nkinds = 3\n",
            4,
            "without a selector",
        ),
    ] {
        let text = format!("{text}\n[[flow.start]]\nkinds = [\"lending\"]\n");
        let flow = table(&text, &scheme);
        let error = flow.end.error.as_ref().unwrap_or_else(|| panic!("{text}"));
        assert_eq!(error.line, Some(line), "{text}: {error:?}");
        assert!(error.message.contains(fragment), "{text}: {error:?}");
        assert!(flow.end.rules.is_empty(), "{text}");
        assert_eq!(flow.start.error, None, "{text}");
        assert_eq!(flow.start.rules.len(), 1, "{text}");
        assert_eq!(flow.step, FlowTable::default(), "{text}");
    }
}

/// `[flow]` itself cannot be trusted: every table carries the error, and no
/// row is called measured on a rule nobody could read.
#[test]
fn a_bad_flow_fails_every_table() {
    let scheme = corpus_scheme(&fixture(GAPS));
    for (text, line) in [
        ("[flow]\nflavour = 1\n", 2),
        ("flow = 3\n", 1),
        ("[flow]\nstart = 3\n", 2),
    ] {
        let flow = table(text, &scheme);
        assert!(flow.start.error.is_some(), "{text}: {flow:?}");
        assert_eq!(
            flow.start.error.as_ref().and_then(|error| error.line),
            Some(line),
            "{text}: {flow:?}"
        );
        if text.contains("flavour") || text.starts_with("flow") {
            assert!(flow.end.error.is_some(), "{text}");
            assert!(flow.step.error.is_some(), "{text}");
            assert!(flow.definition.error.is_some(), "{text}");
        }
    }
}

/// `ProjectConfig` only name-checks `[flow]`: a bad one fails no read, a
/// top-level typo still does.
#[test]
fn the_project_config_only_name_checks_flow() {
    let base = "[project]\nslug = \"lending\"\n";
    for flow in [
        "[[flow.end]]\n",
        "[flow]\nflavour = 1\n",
        "[[flow.definition]]\nkinds = [\"nothing\"]\n",
    ] {
        ProjectConfig::from_toml(&format!("{base}{flow}"))
            .unwrap_or_else(|error| panic!("{flow}: {error:?}"));
    }
    ProjectConfig::from_toml(&format!("flow = 3\n{base}"))
        .unwrap_or_else(|error| panic!("a scalar `flow`: {error:?}"));
    let error = ProjectConfig::from_toml(&format!("{base}[flows]\nend = 1\n"));
    assert!(error.is_err(), "a neighbouring name stays an error");
}

// ----------------------------------------------------- the brute-force view

/// A small seeded generator: a test's corpora are the same on every run.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % bound
    }
}

/// One random `follows` item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    from: usize,
    to: usize,
    kind: Option<&'static str>,
}

/// A random flow: `steps` steps, the first `grouped` of them under
/// `GROUP-01`; `items` runs `from` then `to`, `to` declaring.
#[derive(Debug)]
struct Plan {
    steps: usize,
    grouped: usize,
    starts: BTreeSet<usize>,
    ends: BTreeSet<usize>,
    items: Vec<Item>,
}

fn step_name(step: usize) -> String {
    format!("STEP-{:02}", step + 1)
}

fn step_path(step: usize) -> String {
    format!("docs/spec/{}.md", step_name(step).to_lowercase())
}

impl Plan {
    fn random(seed: u64) -> Self {
        let mut lcg = Lcg(seed.wrapping_mul(2_862_933_555_777_941_757).wrapping_add(3));
        let steps = 2 + lcg.below(11);
        let grouped = 1 + lcg.below(steps);
        let mut starts = BTreeSet::from([lcg.below(steps)]);
        if lcg.below(3) == 0 {
            starts.insert(lcg.below(steps));
        }
        let mut ends = BTreeSet::from([lcg.below(steps)]);
        if lcg.below(2) == 0 {
            ends.insert(lcg.below(steps));
        }
        let mut items = Vec::new();
        for to in 0..steps {
            for _ in 0..lcg.below(3) {
                let from = lcg.below(steps);
                let kind = match lcg.below(6) {
                    0 => Some("alternative"),
                    1 | 2 => Some("repeat"),
                    3 => Some("parallel"),
                    _ => None,
                };
                items.push(Item { from, to, kind });
            }
        }
        Self {
            steps,
            grouped,
            starts,
            ends,
            items,
        }
    }

    /// The corpus and its config text.
    fn corpus(&self) -> (String, Vec<(String, String)>) {
        let list = |set: &BTreeSet<usize>| {
            set.iter()
                .map(|step| format!("\"{}\"", step_path(*step)))
                .collect::<Vec<String>>()
                .join(", ")
        };
        let text = format!(
            "[project]\nslug = \"stages\"\n\n[ids]\nSTEP = {{ kind = \"stage\", width = 2 }}\n\
             GROUP = {{ kind = \"group\", width = 2 }}\n\n[[flow.step]]\nkinds = [\"stage\"]\n\n\
             [[flow.start]]\npaths = [{}]\n\n[[flow.end]]\npaths = [{}]\n",
            list(&self.starts),
            list(&self.ends)
        );
        let mut files = vec![(
            "docs/spec/group-01.md".to_owned(),
            "---\nid: GROUP-01\nclass: canon\ntitle: A group\nowner: owner\n\
             reviewed: 2026-10-01\n---\n\n# A group\n\nA group.\n"
                .to_owned(),
        )];
        for step in 0..self.steps {
            let parent = if step < self.grouped {
                "parent: GROUP-01\n"
            } else {
                ""
            };
            let items: String = self
                .items
                .iter()
                .filter(|item| item.to == step)
                .map(|item| match item.kind {
                    Some(kind) => {
                        format!("    - {{to: {}, kind: {kind}}}\n", step_name(item.from))
                    }
                    None => format!("    - {}\n", step_name(item.from)),
                })
                .collect();
            let links = if items.is_empty() {
                String::new()
            } else {
                format!("links:\n  follows:\n{items}")
            };
            files.push((
                step_path(step),
                format!(
                    "---\nid: {name}\nclass: canon\ntitle: A step\n{parent}owner: owner\n\
                     reviewed: 2026-10-01\n{links}---\n\n# A step\n\nA step.\n",
                    name = step_name(step)
                ),
            ));
        }
        (text, files)
    }
}

/// The positive transitive closure of `edges` over `members`.
fn closure(
    members: &BTreeSet<usize>,
    edges: &BTreeSet<(usize, usize)>,
) -> BTreeSet<(usize, usize)> {
    let mut reach: BTreeSet<(usize, usize)> = edges
        .iter()
        .filter(|(from, to)| members.contains(from) && members.contains(to))
        .copied()
        .collect();
    for &via in members {
        for &from in members {
            for &to in members {
                if reach.contains(&(from, via)) && reach.contains(&(via, to)) {
                    reach.insert((from, to));
                }
            }
        }
    }
    reach
}

/// The cycles of a closure: a member with a path to itself, grouped by
/// mutual reach.
fn cycles_of(members: &BTreeSet<usize>, reach: &BTreeSet<(usize, usize)>) -> Vec<BTreeSet<usize>> {
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    for &member in members {
        if seen.contains(&member) || !reach.contains(&(member, member)) {
            continue;
        }
        let component: BTreeSet<usize> = members
            .iter()
            .copied()
            .filter(|other| {
                *other == member
                    || (reach.contains(&(member, *other)) && reach.contains(&(*other, member)))
            })
            .collect();
        seen.extend(component.iter().copied());
        found.push(component);
    }
    found
}

fn one_each(steps: impl IntoIterator<Item = usize>) -> Vec<Vec<String>> {
    let mut items: Vec<Vec<String>> = steps.into_iter().map(|s| vec![step_name(s)]).collect();
    items.sort();
    items
}

fn named(cycles: &[BTreeSet<usize>]) -> Vec<Vec<String>> {
    let mut items: Vec<Vec<String>> = cycles
        .iter()
        .map(|cycle| cycle.iter().map(|step| step_name(*step)).collect())
        .collect();
    items.sort();
    items
}

/// The graph rows of a [`Plan`] under `scope` (a subset of the steps), from
/// the spec's sentences, with no Tarjan.
struct Reading {
    no_start: bool,
    two_starts: Vec<Vec<String>>,
    unreachable: Vec<Vec<String>>,
    no_way_to_end: Vec<Vec<String>>,
    trap_cycle: Vec<Vec<String>>,
    cycle_no_repeat: Vec<Vec<String>>,
    unjoined: Option<Vec<Vec<String>>>,
}

fn reading(plan: &Plan, scope: &BTreeSet<usize>) -> Reading {
    let every: BTreeSet<(usize, usize)> =
        plan.items.iter().map(|item| (item.from, item.to)).collect();
    let forward = closure(scope, &every);
    let kept: BTreeSet<(usize, usize)> = plan
        .items
        .iter()
        .filter(|item| item.kind != Some("repeat"))
        .map(|item| (item.from, item.to))
        .collect();
    let without_repeat = closure(scope, &kept);

    let entering: BTreeSet<usize> = every
        .iter()
        .filter(|(from, to)| !scope.contains(from) && scope.contains(to))
        .map(|(_, to)| *to)
        .collect();
    let leaving: BTreeSet<usize> = every
        .iter()
        .filter(|(from, to)| scope.contains(from) && !scope.contains(to))
        .map(|(from, _)| *from)
        .collect();
    let starts: BTreeSet<usize> = plan.starts.intersection(scope).copied().collect();
    let ends: BTreeSet<usize> = plan.ends.intersection(scope).copied().collect();

    let begun: BTreeSet<usize> = starts.union(&entering).copied().collect();
    let done: BTreeSet<usize> = ends.union(&leaving).copied().collect();
    let reached = |step: usize| {
        begun.contains(&step) || begun.iter().any(|from| forward.contains(&(*from, step)))
    };
    let finishes =
        |step: usize| done.contains(&step) || done.iter().any(|to| forward.contains(&(step, *to)));

    let cycles = cycles_of(scope, &forward);
    let traps: Vec<BTreeSet<usize>> = cycles
        .iter()
        .filter(|cycle| !cycle.iter().copied().any(finishes))
        .cloned()
        .collect();
    let trapped: BTreeSet<usize> = traps.iter().flatten().copied().collect();

    // Only a split of two branches: "sharing no later step" does not say
    // how three that pairwise differ read, and this view takes no side.
    let mut unjoined = Some(Vec::new());
    let splits: BTreeSet<usize> = plan
        .items
        .iter()
        .filter(|item| item.kind == Some("parallel") && scope.contains(&item.from))
        .map(|item| item.from)
        .collect();
    for split in splits {
        let branches: BTreeSet<usize> = plan
            .items
            .iter()
            .filter(|item| item.kind == Some("parallel") && item.from == split)
            .map(|item| item.to)
            .collect();
        if branches.len() > 2 {
            unjoined = None;
            break;
        }
        let walk = |branch: usize| {
            // The walk stops at the split, a branch that is the split too.
            if branch == split {
                return BTreeSet::new();
            }
            let mut seen = BTreeSet::from([branch]);
            let mut queue = VecDeque::from([branch]);
            while let Some(step) = queue.pop_front() {
                for (from, to) in &every {
                    if *from == step && *to != split && seen.insert(*to) {
                        queue.push_back(*to);
                    }
                }
            }
            seen
        };
        let mut walks = branches.iter().map(|branch| walk(*branch));
        let first = walks.next().expect("a branch");
        let met = branches.len() > 1
            && walks
                .fold(first, |common, other| {
                    common.intersection(&other).copied().collect()
                })
                .iter()
                .next()
                .is_some();
        if !met && let Some(list) = &mut unjoined {
            list.push(vec![step_name(split)]);
        }
    }
    if let Some(list) = &mut unjoined {
        list.sort();
    }

    Reading {
        no_start: starts.is_empty() && entering.is_empty(),
        two_starts: if starts.len() > 1 {
            vec![starts.iter().map(|step| step_name(*step)).collect()]
        } else {
            Vec::new()
        },
        unreachable: one_each(scope.iter().copied().filter(|step| !reached(*step))),
        no_way_to_end: one_each(
            scope
                .iter()
                .copied()
                .filter(|step| !finishes(*step) && !trapped.contains(step)),
        ),
        trap_cycle: named(&traps),
        cycle_no_repeat: named(&cycles_of(scope, &without_repeat)),
        unjoined,
    }
}

/// Asserts `got` is `want` for one plan and scope.
fn assert_reading(got: &FlowRead, want: &Reading, context: &str) {
    let items = |code: GapCode| row(got, code).items.clone();
    assert_eq!(
        row(got, GapCode::NoStart).count,
        Some(usize::from(want.no_start)),
        "no-start {context}"
    );
    assert_eq!(items(GapCode::TwoStarts), want.two_starts, "{context}");
    if want.no_start {
        assert_eq!(
            row(got, GapCode::Unreachable).state,
            GapState::Cannot,
            "{context}"
        );
    } else {
        assert_eq!(
            items(GapCode::Unreachable),
            want.unreachable,
            "unreachable {context}"
        );
    }
    assert_eq!(
        items(GapCode::NoWayToEnd),
        want.no_way_to_end,
        "no-way-to-end {context}"
    );
    assert_eq!(
        items(GapCode::TrapCycle),
        want.trap_cycle,
        "traps {context}"
    );
    assert_eq!(
        items(GapCode::CycleNoRepeat),
        want.cycle_no_repeat,
        "cycles {context}"
    );
    if let Some(unjoined) = &want.unjoined {
        assert_eq!(
            &items(GapCode::UnjoinedParallel),
            unjoined,
            "unjoined {context}"
        );
    }
}

/// Every graph row, on 400 seeded flows, whole and under a ROOT crossing
/// the rest of the flow: the implementation against the spec's sentences
/// written as closures. Every plan's distance from the marked starts too.
#[test]
fn a_random_flow_reads_as_its_closures_do() {
    let mut planted: BTreeMap<&str, usize> = BTreeMap::new();
    for seed in 0..400u64 {
        let plan = Plan::random(seed);
        let (text, files) = plan.corpus();
        let config = Config::from_toml(&text);
        let borrowed: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, body)| (path.as_str(), body.as_str()))
            .collect();
        let input = config.input(&borrowed);

        let whole: BTreeSet<usize> = (0..plan.steps).collect();
        let got = read_input(&text, &config, &input, None, None);
        let want = reading(&plan, &whole);
        assert_reading(&got, &want, &format!("seed {seed} whole: {plan:?}"));

        // The distance: the shortest `follows` path from a marked start.
        let mut distance: BTreeMap<usize, usize> =
            plan.starts.iter().map(|start| (*start, 0)).collect();
        let mut queue: VecDeque<usize> = plan.starts.iter().copied().collect();
        while let Some(step) = queue.pop_front() {
            for item in plan.items.iter().filter(|item| item.from == step) {
                if !distance.contains_key(&item.to) {
                    distance.insert(item.to, distance[&step] + 1);
                    queue.push_back(item.to);
                }
            }
        }
        for step in 0..plan.steps {
            let node = got
                .nodes
                .iter()
                .find(|node| node.id.as_deref() == Some(step_name(step).as_str()))
                .unwrap_or_else(|| panic!("seed {seed}: no node {}", step_name(step)));
            assert_eq!(
                node.distance,
                distance.get(&step).copied(),
                "seed {seed} {}: {plan:?}",
                step_name(step)
            );
        }

        let grouped: BTreeSet<usize> = (0..plan.grouped).collect();
        let got = read_input(&text, &config, &input, Some("GROUP-01"), None);
        let scoped = reading(&plan, &grouped);
        assert_reading(&got, &scoped, &format!("seed {seed} scoped: {plan:?}"));

        for (name, found) in [
            ("no-start", usize::from(scoped.no_start)),
            ("unreachable", scoped.unreachable.len()),
            ("no-way-to-end", want.no_way_to_end.len()),
            ("trap-cycle", want.trap_cycle.len()),
            ("cycle-no-repeat", want.cycle_no_repeat.len()),
            (
                "unjoined-parallel",
                want.unjoined.as_ref().map_or(0, Vec::len),
            ),
        ] {
            *planted.entry(name).or_default() += found;
        }
    }
    // The seeds reach every rule: a generator that never planted one proves
    // nothing about it.
    for (name, found) in &planted {
        assert!(*found >= 20, "{name}: only {found} planted: {planted:?}");
    }
    assert_eq!(planted.len(), 6, "{planted:?}");
}

// ------------------------------------------------------------ deep corpora

/// A ring of 4 000 steps and, beside it, a `parent:` chain of 20 000
/// documents, IDs at width 5: no stack depth grows with them
/// (docs/features/flow-linear.md AC-10 "The deep test").
#[test]
fn a_long_ring_and_a_deep_tree_are_walked_without_recursion() {
    let (ring, chain) = (4000, 20_000);
    let text = "[project]\nslug = \"deep\"\n\n[ids]\nSTEP = { kind = \"stage\", width = 4 }\n\
                DEEP = { kind = \"layer\", width = 5 }\n"
        .to_owned();
    let config = Config::from_toml(&text);
    let mut files = Vec::new();
    for step in 1..=ring {
        let to = if step == 1 { ring } else { step - 1 };
        files.push((
            format!("docs/spec/step-{step:04}.md"),
            format!(
                "---\nid: STEP-{step:04}\nclass: canon\ntitle: A step\nowner: owner\n\
                 reviewed: 2026-10-01\nlinks:\n  follows: [STEP-{to:04}]\n---\n\n# A step\n"
            ),
        ));
    }
    for step in 1..=chain {
        let parent = if step == 1 {
            String::new()
        } else {
            format!("parent: DEEP-{:05}\n", step - 1)
        };
        files.push((
            format!("docs/spec/deep-{step:05}.md"),
            format!(
                "---\nid: DEEP-{step:05}\nclass: canon\ntitle: A layer\n{parent}owner: owner\n\
                 reviewed: 2026-10-01\nlinks:\n  uses: [STEP-0001]\n---\n\n# A layer\n"
            ),
        ));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    let input = config.input(&borrowed);
    let got = read_input(&text, &config, &input, None, None);
    assert_eq!(got.nodes.len(), ring + chain);
    let cycle = &row(&got, GapCode::CycleNoRepeat).items;
    assert_eq!(cycle.len(), 1, "one cycle");
    assert_eq!(cycle[0].len(), ring, "of every member");

    let got = read_input(&text, &config, &input, Some("DEEP-00001"), Some(0));
    assert_eq!(got.nodes.len(), 1, "the chain folds into its root");
    assert_eq!(got.nodes[0].holds, chain - 1);
}

/// A `parent:` chain of `layers` documents, each using one step, so every
/// layer is in the whole read before its ancestor walk: the corpus of the
/// ratio (docs/features/flow-linear.md AC-10).
fn chain_of(layers: usize) -> (String, Config, CheckInput) {
    let text = "[project]\nslug = \"chain\"\n\n[ids]\nSTEP = { kind = \"stage\", width = 5 }\n\
                DEEP = { kind = \"layer\", width = 5 }\n"
        .to_owned();
    let config = Config::from_toml(&text);
    let mut files = vec![(
        "docs/spec/step-00001.md".to_owned(),
        "---\nid: STEP-00001\nclass: canon\ntitle: A step\nowner: owner\n\
         reviewed: 2026-10-01\n---\n\n# A step\n"
            .to_owned(),
    )];
    for layer in 1..=layers {
        let parent = if layer == 1 {
            String::new()
        } else {
            format!("parent: DEEP-{:05}\n", layer - 1)
        };
        files.push((
            format!("docs/spec/deep-{layer:05}.md"),
            format!(
                "---\nid: DEEP-{layer:05}\nclass: canon\ntitle: A layer\n{parent}owner: owner\n\
                 reviewed: 2026-10-01\nlinks:\n  uses: [STEP-00001]\n---\n\n# A layer\n"
            ),
        ));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    let input = config.input(&borrowed);
    (text, config, input)
}

/// The time of one whole read of `graph` at `depth`, `flow_read` alone.
fn read_time(graph: &SpecGraph<'_>, flow: &FlowConfig, depth: Option<usize>) -> Duration {
    let started = Instant::now();
    let read = flow_read(
        graph,
        &FlowAsk {
            roots: None,
            depth,
            config: flow,
            config_label: LABEL,
            scope_name: "the-whole",
        },
    );
    let elapsed = started.elapsed();
    black_box(read);
    elapsed
}

/// AC-10 "The ratio": a whole read of a `parent:` chain of 8 000 costs at
/// most 8x one of 2 000 (linear: 4x; quadratic: 16x), timed on `flow_read`
/// alone, each graph built before, unbounded and at `--depth 0`; each
/// measure the best of three, the sizes alternating, the 2 000 time floored
/// at 50 ms. M: a per-member `ancestors` in `scope_of`, `parented` or
/// `tree_of`; the fold climbing per call (at depth 0).
#[test]
fn a_chain_four_times_as_deep_reads_in_at_most_eight_times_the_time() {
    let (small, large) = (2_000, 8_000);
    let corpora = [chain_of(small), chain_of(large)];
    let graphs: Vec<(SpecGraph<'_>, FlowConfig)> = corpora
        .iter()
        .map(|(text, config, input)| {
            (
                SpecGraph::new(input, &config.scheme, &config.paths),
                FlowConfig::from_toml(text, &config.scheme),
            )
        })
        .collect();
    // The reads are the chain: every layer and the step, one edge.
    for ((graph, flow), layers) in graphs.iter().zip([small, large]) {
        let read = flow_read(
            graph,
            &FlowAsk {
                roots: None,
                depth: Some(0),
                config: flow,
                config_label: LABEL,
                scope_name: "the-whole",
            },
        );
        assert_eq!(
            read.nodes.len(),
            2,
            "{layers}: the chain folds into its top"
        );
        let top = read
            .nodes
            .iter()
            .find(|node| name_of(node) == "DEEP-00001")
            .expect("the top");
        assert_eq!(top.holds, layers - 1);
    }
    for depth in [None, Some(0)] {
        let mut best = [Duration::MAX; 2];
        for _ in 0..3 {
            for (at, (graph, flow)) in graphs.iter().enumerate() {
                best[at] = best[at].min(read_time(graph, flow, depth));
            }
        }
        let floor = best[0].max(Duration::from_millis(50));
        assert!(
            best[1] <= floor * 8,
            "depth {depth:?}: {large} layers took {:?}, {small} took {:?} (floored {floor:?}): \
             more than 8x",
            best[1],
            best[0]
        );
    }
}
