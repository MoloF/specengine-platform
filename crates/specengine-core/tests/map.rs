//! docs/features/corpus-map.md (RM-0112, ADR-0080) "Acceptance criteria",
//! the core half of CM1 and CM2: `corpus_map` over `fixtures/spec-a`'s graph
//! (parsed in memory; the fixture only read).
//!
//! - CM1: 19 nodes (12 documents, 7 sections) in (path, position) order,
//!   each section `of` its document's index, each node's usage
//!   `usage_of`'s; 35 edges, one per (source, target, type), 25 weak, sorted,
//!   none a self-edge, none to the Tier 3 DEC-0007; 3 parents; no ghost.
//!   With the archive: 13 documents and DEC-0023's `supersedes` of DEC-0007
//!   with its two places. The files given in the opposite order give the
//!   same map, node for node. M: an edge per place; self-edges kept; Tier 3
//!   nodes without the archive.
//! - CM2: two dangling ends written `MEC-GONE` (a `depends_on` and a wiki
//!   mention) are one ghost, its sources both holders, `places` 2; the
//!   `project:` link is none. M: skipped links as ghosts; a ghost per place.

mod common;

use std::collections::BTreeMap;

use common::check::{Config, fixture_input};
use common::fixture;
use specengine_core::check::{CheckInput, SpecGraph};
use specengine_core::map::{CorpusMap, corpus_map};
use specengine_core::usage::usage_of;

/// `fixtures/spec-a`'s live nodes in (path, position) order.
const LIVE: [&str; 19] = [
    "docs/features/stamina-tuning.md",
    "AC-07",
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

/// The map's node names, in order.
fn names(graph: &SpecGraph<'_>, map: &CorpusMap) -> Vec<String> {
    map.nodes.iter().map(|node| graph.name(node.at)).collect()
}

/// The edges as `(src, dst, type) -> places`, by name.
fn edges(graph: &SpecGraph<'_>, map: &CorpusMap) -> BTreeMap<(String, String, String), usize> {
    let names = names(graph, map);
    map.edges
        .iter()
        .map(|edge| {
            (
                (
                    names[edge.src].clone(),
                    names[edge.dst].clone(),
                    edge.link_type.clone(),
                ),
                edge.places,
            )
        })
        .collect()
}

/// The checks every map holds: (path, position) order, `of`, usage,
/// sorted edges between distinct nodes with `weak` for `mentions`.
fn assert_well_formed(graph: &SpecGraph<'_>, map: &CorpusMap) {
    let usage = usage_of(graph);
    let keys: Vec<(&str, usize)> = map
        .nodes
        .iter()
        .map(|node| (graph.paths()[node.at.file], node.at.ord))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(keys, sorted, "nodes in (path, position) order, once each");
    for (index, node) in map.nodes.iter().enumerate() {
        assert_eq!(
            Some(&node.usage),
            usage.get(&node.at),
            "{}",
            graph.name(node.at)
        );
        match node.of {
            None => assert_eq!(node.at.ord, 0, "a document: {}", graph.name(node.at)),
            Some(of) => {
                assert!(of < index);
                assert_eq!(map.nodes[of].at.file, node.at.file);
                assert_eq!(map.nodes[of].at.ord, 0);
                assert_eq!(node.usage.orphan, None, "a section has no flag");
            }
        }
    }
    let mut previous = None;
    for edge in &map.edges {
        assert_ne!(edge.src, edge.dst, "M: self-edges kept");
        assert!(edge.places >= 1);
        assert_eq!(edge.weak, edge.link_type == "mentions", "{edge:?}");
        let key = (edge.src, edge.dst, edge.link_type.clone());
        if let Some(previous) = &previous {
            assert!(previous < &key, "sorted, one per triple: {edge:?}");
        }
        previous = Some(key);
    }
}

/// The spec-a graph of `input`, then `check` on its map.
fn with_map(
    config: &Config,
    input: &CheckInput,
    archive: bool,
    check: impl FnOnce(&SpecGraph<'_>, &CorpusMap),
) {
    let graph = SpecGraph::new(input, &config.scheme, &config.paths);
    let map = corpus_map(&graph, archive);
    assert_well_formed(&graph, &map);
    check(&graph, &map);
}

/// CM1, core: the copy's nodes, edges and parents; the archive's.
#[test]
fn cm1_corpus_map_on_spec_a() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    with_map(&config, &input, false, |graph, map| {
        assert_eq!(names(graph, map), LIVE);
        let documents = map.nodes.iter().filter(|node| node.of.is_none()).count();
        assert_eq!((documents, map.nodes.len() - documents), (12, 7));
        assert_eq!(map.nodes[17].of, Some(16), "RULE-STAM-REGEN of MEC-STAMINA");
        assert_eq!(
            map.nodes[15].of,
            Some(13),
            "EDGE-SPRINT-EMPTY of MEC-SPRINT"
        );
        let edges = edges(graph, map);
        assert_eq!(map.edges.len(), 35, "M: an edge per place: {edges:?}");
        assert_eq!(map.edges.iter().filter(|edge| edge.weak).count(), 25);
        assert_eq!(
            edges.get(&(
                "docs/features/stamina-tuning.md".to_owned(),
                "R-12".to_owned(),
                "mentions".to_owned()
            )),
            Some(&2)
        );
        assert_eq!(
            edges.get(&(
                "MEC-SPRINT".to_owned(),
                "MEC-STAMINA".to_owned(),
                "depends_on".to_owned()
            )),
            Some(&1)
        );
        let parents: Vec<(usize, usize)> = map
            .parents
            .iter()
            .map(|parent| (parent.child, parent.parent))
            .collect();
        assert_eq!(parents, [(11, 9), (13, 11), (16, 11)]);
        assert!(map.ghosts.is_empty(), "{:?}", map.ghosts);
        assert_eq!(
            map.nodes
                .iter()
                .filter(|node| node.usage.orphan == Some(true))
                .count(),
            1
        );
    });
    with_map(&config, &input, true, |graph, map| {
        let mut want: Vec<&str> = LIVE.to_vec();
        want.insert(4, "DEC-0007");
        assert_eq!(
            names(graph, map),
            want,
            "M: Tier 3 nodes without the archive"
        );
        assert_eq!(map.edges.len(), 37);
        let edges = edges(graph, map);
        assert_eq!(
            edges.get(&(
                "DEC-0023".to_owned(),
                "DEC-0007".to_owned(),
                "supersedes".to_owned()
            )),
            Some(&2)
        );
        assert_eq!(map.nodes[4].usage.orphan, None, "not live: no flag");
    });
}

/// CM1, core: the files in the opposite order give the same map, node for
/// node; a self-citation draws no edge.
#[test]
fn cm1_one_graph_one_map_whatever_the_file_order() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let mut reversed = input.clone();
    reversed.files.reverse();
    let mut seen = Vec::new();
    for input in [&input, &reversed] {
        with_map(&config, input, true, |graph, map| {
            let names = names(graph, map);
            let places: Vec<(String, usize)> = map
                .nodes
                .iter()
                .map(|node| (graph.paths()[node.at.file].to_owned(), graph.line(node.at)))
                .collect();
            let edges: Vec<(usize, usize, String, usize, bool)> = map
                .edges
                .iter()
                .map(|edge| {
                    (
                        edge.src,
                        edge.dst,
                        edge.link_type.clone(),
                        edge.places,
                        edge.weak,
                    )
                })
                .collect();
            let parents: Vec<(usize, usize)> = map
                .parents
                .iter()
                .map(|parent| (parent.child, parent.parent))
                .collect();
            let of: Vec<Option<usize>> = map.nodes.iter().map(|node| node.of).collect();
            seen.push((names, places, edges, parents, of));
        });
    }
    assert_eq!(seen[0], seen[1]);

    let text = std::fs::read_to_string(fixture("spec-a").join("specengine.toml")).expect("config");
    let config = Config::from_toml(&text);
    let files = [
        (
            "docs/records/A/A-103.md",
            "---\nid: A-103\nclass: canon\nlinks:\n  derived_from: [A-103, A-101]\n---\n\n# Gate\n\nA-103 holds; see [[A-103#gate]].\n",
        ),
        (
            "docs/records/A/A-101.md",
            "---\nid: A-101\nclass: canon\n---\n\n# Walking\n\nSee A-103.\n",
        ),
    ];
    let input = config.input(&files);
    with_map(&config, &input, false, |graph, map| {
        let edges = edges(graph, map);
        assert_eq!(
            edges.keys().cloned().collect::<Vec<_>>(),
            [
                (
                    "A-101".to_owned(),
                    "A-103".to_owned(),
                    "mentions".to_owned()
                ),
                (
                    "A-103".to_owned(),
                    "A-101".to_owned(),
                    "derived_from".to_owned()
                ),
            ],
            "M: self-edges kept"
        );
    });
}

/// CM2, core: one ghost per written text, its sources both holders, its
/// places 2; the `project:` link none.
#[test]
fn cm2_one_ghost_per_written_end() {
    let corpus = fixture("spec-a");
    let (config, mut input) = fixture_input(&corpus);
    let edit = |path: &str, from: &str, to: &str| {
        let text = std::fs::read_to_string(corpus.join(path)).expect("a fixture file");
        assert_eq!(text.matches(from).count(), 1, "{path}: {from}");
        (path.to_owned(), text.replacen(from, to, 1))
    };
    let edits = [
        edit(
            "docs/spec/movement/sprint.md",
            "depends_on: [MEC-STAMINA]",
            "depends_on: [MEC-STAMINA, MEC-GONE]",
        ),
        edit(
            "docs/spec/movement/stamina.md",
            "\n## Regeneration {#RULE-STAM-REGEN}",
            "\nOnce paired with [[MEC-GONE]].\n\n## Regeneration {#RULE-STAM-REGEN}",
        ),
    ];
    for (path, text) in &edits {
        let file = input
            .files
            .iter_mut()
            .find(|file| &file.path == path)
            .unwrap_or_else(|| panic!("{path}"));
        *file = config
            .input(&[(path.as_str(), text.as_str())])
            .files
            .remove(0);
    }
    with_map(&config, &input, false, |graph, map| {
        let names = names(graph, map);
        let ghosts: Vec<(String, Vec<&str>, usize)> = map
            .ghosts
            .iter()
            .map(|ghost| {
                (
                    ghost.written.clone(),
                    ghost.sources.iter().map(|at| names[*at].as_str()).collect(),
                    ghost.places,
                )
            })
            .collect();
        assert_eq!(
            ghosts,
            [("MEC-GONE".to_owned(), vec!["MEC-SPRINT", "MEC-STAMINA"], 2)],
            "M: skipped links as ghosts; a ghost per place"
        );
        assert!(!map.ghosts[0].reason.is_empty());
        assert_eq!(map.edges.len(), 35, "a ghost is no edge");
    });
}
