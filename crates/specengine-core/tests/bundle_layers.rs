//! docs/features/spec-cli-bundle.md, core: `bundle_layers` over a
//! `SpecGraph` of files parsed by the real parser (in memory) — the layer
//! table by link type, direction and `[ids]` scope with kinds no rule
//! knows, targets merged and de-duplicated, dangling and `project:` links
//! not followed, cycles, "open" by links (an `answers` on a section within
//! the question, a Tier 3 `answers`), a dangling working answer — and the
//! narrow `[budgets] bundle_node` reader.

mod common;

use common::check::Config;
use specengine_core::check::{
    BundleLayer, BundleLayers, NodeAt, SpecGraph, bundle_layers, bundle_node_from_toml,
};

/// Kinds no fixture and no rule uses; `CHK` is feature-scoped.
const TOML: &str = "\
[ids]
TOP  = { kind = \"k-one\",   shape = \"name\" }
ITEM = { kind = \"k-two\",   shape = \"name\" }
PART = { kind = \"k-three\", shape = \"name\" }
ASK  = { kind = \"k-four\",  width = 2 }
GUESS = { kind = \"k-five\", width = 2 }
RULING = { kind = \"k-six\", width = 3 }
CHK  = { kind = \"k-seven\", width = 2, scope = \"feature\" }
NOTE = { kind = \"k-eight\", width = 2 }
";

fn doc(id: &str, extra: &str, body: &str) -> String {
    format!("---\nid: {id}\nclass: canon\n{extra}---\n\n# Title of {id}\n\n{body}")
}

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

/// `(layer, name, via)` of every candidate, in order.
fn view(graph: &SpecGraph<'_>, layers: &BundleLayers) -> Vec<(BundleLayer, String, Vec<String>)> {
    layers
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.layer,
                graph.name(candidate.node),
                candidate
                    .via
                    .iter()
                    .map(|(link_type, direction)| format!("{link_type} {}", direction.as_str()))
                    .collect(),
            )
        })
        .collect()
}

fn corpus() -> Vec<(&'static str, String)> {
    vec![
        ("docs/spec/top.md", doc("TOP-ROOT", "", "The root.\n")),
        (
            "docs/spec/item.md",
            doc(
                "ITEM-MAIN",
                "parent: TOP-ROOT\nlinks:\n  depends_on: [ITEM-DEP, ITEM-NOPE, other:ITEM-X]\n  \
                 uses_term: [NOTE-01]\n  derived_from: [GUESS-01]\n",
                "Main item.\n\n## Inner {#PART-INNER}\n\nInner part.\n",
            ),
        ),
        (
            "docs/spec/dep.md",
            doc(
                "ITEM-DEP",
                "parent: TOP-ROOT\nlinks:\n  depends_on: [ITEM-MAIN]\n  constrains: [PART-INNER]\n",
                "Depends back.\n",
            ),
        ),
        (
            "docs/records/ask.md",
            doc(
                "ASK-01",
                "status: answered\nworking_answer: GUESS-01\nrefs: [ITEM-MAIN]\n",
                "Open by links whatever the status says.\n",
            ),
        ),
        (
            "docs/records/ask2.md",
            doc(
                "ASK-02",
                "status: open\nworking_answer: GUESS-99\n",
                "Asks about PART-INNER.\n\n## Detail {#ASK-03}\n\nA section.\n",
            ),
        ),
        (
            "docs/records/guess.md",
            doc("GUESS-01", "status: open\n", "A guess.\n"),
        ),
        (
            "docs/records/ruling.md",
            "---\nid: RULING-001\nclass: decision\nstatus: accepted\ndate: 2026-09-01\n\
             canon: docs/spec/item.md#inner\nscope: [x]\n---\n\n# Ruling\n\nRuled.\n"
                .to_owned(),
        ),
        (
            "docs/records/old.md",
            "---\nid: RULING-002\nclass: decision\nstatus: superseded-by RULING-001\ndate: 2026-08-01\n\
             canon: docs/spec/item.md\nscope: [x]\nlinks:\n  answers: [ASK-01]\n---\n\n# Old\n\nOld.\n"
                .to_owned(),
        ),
        ("docs/records/note.md", doc("NOTE-01", "", "A term.\n")),
        (
            "docs/features/f.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# F\n\n## Criteria\n\n\
             ### Holds {#CHK-01}\n\nChecks ITEM-MAIN.\n\n### Plain {#PART-PLAIN}\n\nAlso ITEM-MAIN.\n"
                .to_owned(),
        ),
    ]
}

/// The table: layer 2 by any type with a working answer, open by links
/// (a Tier 3 `answers` closes nothing, `status:` is not read); ancestors
/// nearest first; a feature-scoped section a criterion, another section
/// not; the decision by `canon` onto a section within the target, the
/// superseded one not; `depends_on` both ways and `constrains` onto the
/// section give one neighbour, `via` its qualifying pairs only (the
/// incoming `depends_on` is impact, A7); dangling and `project:` links followed
/// nowhere; every kind unknown to the rules.
#[test]
fn the_layers_follow_link_types_directions_and_scopes_only() {
    let config = Config::from_toml(TOML);
    let files = corpus();
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let input = config.input(&borrowed);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let main = find(&graph, "ITEM-MAIN");
    let layers = bundle_layers(&graph, &[main]);
    assert_eq!(layers.targets, [main]);
    assert!(layers.merged.is_empty());
    let got = view(&graph, &layers);
    let want: Vec<(BundleLayer, String, Vec<String>)> = vec![
        (
            BundleLayer::OpenQuestions,
            "ASK-01".into(),
            vec!["mentions in".into()],
        ),
        (
            BundleLayer::OpenQuestions,
            "ASK-02".into(),
            vec!["mentions in".into()],
        ),
        (BundleLayer::Ancestors, "TOP-ROOT".into(), vec![]),
        (
            BundleLayer::Criteria,
            "CHK-01".into(),
            vec!["mentions in".into()],
        ),
        (
            BundleLayer::Decisions,
            "RULING-001".into(),
            vec!["canon in".into()],
        ),
        (
            BundleLayer::Neighbours,
            "GUESS-01".into(),
            vec!["derived_from out".into()],
        ),
        (
            BundleLayer::Neighbours,
            "ITEM-DEP".into(),
            vec!["constrains in".into(), "depends_on out".into()],
        ),
        (
            BundleLayer::Terms,
            "NOTE-01".into(),
            vec!["uses_term out".into()],
        ),
    ];
    assert_eq!(got, want, "{got:#?}");
    // ASK-02's working answer dangles: it is still held, whatever its state.
    let ask2 = layers
        .candidates
        .iter()
        .find(|candidate| graph.name(candidate.node) == "ASK-02")
        .unwrap();
    let edge = &graph.edges()[ask2.working_answer.expect("a working answer edge")];
    assert_eq!(edge.written, "GUESS-99");
}

/// A live `answers` onto a section within the question closes it; the
/// question then stands nowhere (`answers out` is in no layer).
#[test]
fn an_answer_on_a_section_within_the_question_closes_it() {
    let config = Config::from_toml(TOML);
    let mut files = corpus();
    files.push((
        "docs/records/ruling3.md",
        "---\nid: RULING-003\nclass: decision\nstatus: accepted\ndate: 2026-09-02\nscope: [x]\n\
         links:\n  answers: [ASK-03]\n---\n\n# Answers the detail\n\nYes.\n"
            .to_owned(),
    ));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let input = config.input(&borrowed);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let layers = bundle_layers(&graph, &[find(&graph, "ITEM-MAIN")]);
    let names: Vec<String> = view(&graph, &layers)
        .into_iter()
        .map(|(_, n, _)| n)
        .collect();
    assert!(!names.contains(&"ASK-02".to_owned()), "{names:?}");
    assert!(names.contains(&"ASK-01".to_owned()), "{names:?}");
}

/// Targets: sorted by (path, position), each once; a section within a
/// named document merged into it; nodes within a target are no
/// candidates; a parent cycle and a link cycle end, each node once.
#[test]
fn targets_merge_and_cycles_end() {
    let config = Config::from_toml(TOML);
    let files = [
        (
            "docs/spec/a.md",
            doc(
                "TOP-A",
                "parent: TOP-B\nlinks:\n  depends_on: [TOP-B]\n",
                "A.\n\n## In {#PART-A}\n\nx\n",
            ),
        ),
        (
            "docs/spec/b.md",
            doc(
                "TOP-B",
                "parent: TOP-A\nlinks:\n  depends_on: [TOP-A, PART-A]\n",
                "B.\n",
            ),
        ),
    ];
    let input = config.input(
        &files
            .iter()
            .map(|(p, t)| (*p, t.as_str()))
            .collect::<Vec<_>>(),
    );
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let a = find(&graph, "TOP-A");
    let part = find(&graph, "PART-A");
    let b = find(&graph, "TOP-B");
    let layers = bundle_layers(&graph, &[part, a, a]);
    assert_eq!(layers.targets, [a]);
    assert_eq!(layers.merged, [(part, a)]);
    let got = view(&graph, &layers);
    let names: Vec<&str> = got.iter().map(|(_, n, _)| n.as_str()).collect();
    assert_eq!(
        names.iter().filter(|&&n| n == "TOP-B").count(),
        1,
        "{got:?}"
    );
    assert!(
        !names.contains(&"PART-A") && !names.contains(&"TOP-A"),
        "{got:?}"
    );

    let both = bundle_layers(&graph, &[b, a]);
    assert_eq!(both.targets, [a, b]);
    assert!(both.candidates.is_empty(), "{:?}", view(&graph, &both));
}

/// `[budgets] bundle_node` alone: absent, other keys only, or `budgets`
/// no table → `None`; a value 1..=u32::MAX at its line; anything else an
/// error at its line; broken `[classes]`, `[check]` and other `[budgets]`
/// keys are not judged; a file that is no TOML is an error.
#[test]
fn bundle_node_is_read_alone() {
    assert_eq!(bundle_node_from_toml("").unwrap(), None);
    assert_eq!(
        bundle_node_from_toml("[budgets]\ntier1_bytes = 1\n").unwrap(),
        None
    );
    assert_eq!(bundle_node_from_toml("budgets = 3\n").unwrap(), None);
    let node = bundle_node_from_toml("[ids]\n\n[budgets]\nbundle_node = 300\n")
        .unwrap()
        .expect("a value");
    assert_eq!((node.tokens, node.line), (300, 4));
    let node = bundle_node_from_toml("[budgets]\nbundle_node = 4294967295\n")
        .unwrap()
        .unwrap();
    assert_eq!(node.tokens, u32::MAX);
    let broken = "[classes]\ncanon = 5\n[check]\nmode = \"sometimes\"\n[budgets]\n\
                  tier1_bytes = \"x\"\nmystery = 1\nbundle_node = 7\n";
    let node = bundle_node_from_toml(broken).unwrap().unwrap();
    assert_eq!((node.tokens, node.line), (7, 8));
    for value in ["0", "-1", "4294967296", "\"300\"", "1.5", "[1]", "true"] {
        let text = format!("[project]\nslug = \"s\"\n[budgets]\nbundle_node = {value}\n");
        let error = bundle_node_from_toml(&text).expect_err(value);
        assert_eq!(error.line, Some(4), "{value}: {error:?}");
        assert!(error.message.contains("bundle_node"), "{value}: {error:?}");
    }
    let error = bundle_node_from_toml("[budgets\nbundle_node = 3\n").expect_err("no TOML");
    assert!(error.line.is_some(), "{error:?}");
}
