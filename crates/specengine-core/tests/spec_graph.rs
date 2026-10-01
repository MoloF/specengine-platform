//! docs/features/spec-cli-graph.md, "Core additions": `SpecGraph` over a
//! `CheckInput` parsed by the real parser (in memory, the check's input
//! type): containment (section nesting, `parent:` resolved from its file,
//! several holders, dangling, `project:`, cycles broken at their first
//! member), links resolved from their citing file (anchors landing on the
//! innermost ID section, unchecked and foreign targets), the live
//! standing, a breadth-first walk with a depth bound and a visited set,
//! and the model's link-type tables (graph and impact directions).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use common::check::{Config, fixture_input};
use common::fixture;
use specengine_core::check::{
    CheckInput, Endpoint, LinkState, NodeAt, Parent, SpecGraph, Standing,
};
use specengine_model::{
    Direction, IMPACT_LINK_TYPES, LINK_TYPES, graph_direction, impact_direction, is_weak_link,
};

const TOML: &str = "\
[ids]
DOM  = { kind = \"domain\",      shape = \"name\" }
MEC  = { kind = \"mechanic\",    shape = \"name\" }
RULE = { kind = \"rule\",        shape = \"name\" }
R    = { kind = \"requirement\", width = 2 }
DEC  = { kind = \"decision\",    width = 4 }
AC   = { kind = \"criterion\",   width = 2, scope = \"feature\" }
";

fn config() -> Config {
    Config::from_toml(TOML)
}

/// A canon document `id` with `extra` front-matter lines and `body`.
fn doc(id: &str, extra: &str, body: &str) -> String {
    format!("---\nid: {id}\nclass: canon\n{extra}---\n\n# Title\n\n{body}")
}

/// The node named `name` (ID, else path); panics when absent or repeated.
fn find(graph: &SpecGraph<'_>, name: &str) -> NodeAt {
    let found = all(graph, name);
    assert_eq!(found.len(), 1, "{name}: {found:?}");
    found[0]
}

fn all(graph: &SpecGraph<'_>, name: &str) -> Vec<NodeAt> {
    let mut found = Vec::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            if graph.name(at) == name {
                found.push(at);
            }
        }
    }
    found
}

fn names(graph: &SpecGraph<'_>, nodes: &[NodeAt]) -> Vec<String> {
    nodes.iter().map(|&at| graph.name(at)).collect()
}

/// `(name, distance)` of a walk.
fn reached(graph: &SpecGraph<'_>, nodes: &[(NodeAt, usize)]) -> Vec<(String, usize)> {
    nodes
        .iter()
        .map(|&(at, distance)| (graph.name(at), distance))
        .collect()
}

/// `src --type--> dst @path:line` of an edge, both ends by name.
fn render(graph: &SpecGraph<'_>, index: usize) -> String {
    let edge = &graph.edges()[index];
    let end = |endpoint: &Endpoint| match endpoint {
        Endpoint::Nodes(nodes) => names(graph, nodes).join("+"),
        Endpoint::Dangling(_) => format!("!{}", edge.written),
        Endpoint::Skipped => format!("~{}", edge.written),
        Endpoint::Unchecked => format!("?{}", edge.written),
    };
    format!(
        "{} --{}--> {} @{}:{}",
        end(&edge.source),
        edge.link_type,
        end(&edge.target),
        graph.paths()[edge.file],
        edge.line
    )
}

fn input(files: &[(&str, &str)]) -> (Config, CheckInput) {
    let config = config();
    let input = config.input(files);
    (config, input)
}

#[test]
fn sections_hang_under_the_nearest_id_section_else_the_document() {
    let (config, input) = input(&[
        (
            "docs/spec/a.md",
            &doc(
                "MEC-A",
                "",
                "## One {#RULE-ONE}\n\n### Inner {#RULE-INNER}\n\nx\n\n## Plain\n\n### Loose {#RULE-LOOSE}\n\ny\n",
            ),
        ),
        (
            "docs/features/f.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# F\n\n## Criteria\n\n### First {#AC-01}\n\nz\n",
        ),
    ]);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let a = find(&graph, "MEC-A");
    let one = find(&graph, "RULE-ONE");
    let inner = find(&graph, "RULE-INNER");
    let loose = find(&graph, "RULE-LOOSE");
    assert_eq!(graph.listed_under(inner), Some(one));
    assert_eq!(graph.listed_under(one), Some(a));
    assert_eq!(
        graph.listed_under(loose),
        Some(a),
        "an ID-less heading holds nothing"
    );
    assert_eq!(graph.listed_under(a), None);
    assert_eq!(graph.parent(a), Some(Parent::None));
    assert_eq!(
        graph.parent(inner),
        Some(Parent::Node {
            node: one,
            others: Vec::new()
        })
    );
    assert_eq!(names(&graph, graph.children(a)), ["RULE-ONE", "RULE-LOOSE"]);
    assert_eq!(names(&graph, graph.children(one)), ["RULE-INNER"]);
    assert_eq!(
        names(&graph, &graph.ancestors(inner)),
        ["RULE-ONE", "MEC-A"]
    );
    // An ID-less feature document holds its criteria.
    let feature = find(&graph, "docs/features/f.md");
    let criterion = find(&graph, "AC-01");
    assert_eq!(graph.listed_under(criterion), Some(feature));
    assert_eq!(graph.document(criterion.file), Some(feature));
    assert_eq!(
        names(&graph, &graph.within(a)),
        ["MEC-A", "RULE-ONE", "RULE-INNER", "RULE-LOOSE"]
    );
    assert_eq!(
        names(&graph, &graph.within(one)),
        ["RULE-ONE", "RULE-INNER"]
    );
}

#[test]
fn parents_resolve_dangle_skip_and_take_the_first_holder() {
    let (config, input) = input(&[
        ("docs/spec/b/x.md", &doc("DOM-X", "", "b\n")),
        (
            "docs/spec/a/x.md",
            &doc("DOM-X", "aliases: [DOM-EX]\n", "a\n"),
        ),
        ("docs/spec/c.md", &doc("MEC-C", "parent: DOM-X\n", "")),
        ("docs/spec/d.md", &doc("MEC-D", "parent: DOM-EX\n", "")),
        ("docs/spec/e.md", &doc("MEC-E", "parent: DOM-NOWHERE\n", "")),
        ("docs/spec/f.md", &doc("MEC-F", "parent: other:DOM-X\n", "")),
    ]);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let holders = all(&graph, "DOM-X");
    assert_eq!(holders.len(), 2);
    let first = holders[0];
    assert_eq!(graph.paths()[first.file], "docs/spec/a/x.md");
    let c = find(&graph, "MEC-C");
    assert_eq!(
        graph.parent(c),
        Some(Parent::Node {
            node: first,
            others: vec![holders[1]]
        })
    );
    assert_eq!(graph.listed_under(c), Some(first));
    // Through an `aliases:` entry of the first holder.
    let d = find(&graph, "MEC-D");
    assert_eq!(graph.listed_under(d), Some(first));
    assert_eq!(names(&graph, graph.children(first)), ["MEC-C", "MEC-D"]);
    assert!(graph.children(holders[1]).is_empty());
    let e = find(&graph, "MEC-E");
    match graph.parent(e) {
        Some(Parent::Dangling { written, reason }) => {
            assert_eq!(written, "DOM-NOWHERE");
            assert!(!reason.is_empty());
        }
        other => panic!("MEC-E: {other:?}"),
    }
    assert_eq!(graph.listed_under(e), None);
    let f = find(&graph, "MEC-F");
    assert!(
        matches!(graph.parent(f), Some(Parent::Skipped { .. })),
        "{:?}",
        graph.parent(f)
    );
    assert_eq!(graph.listed_under(f), None);
    assert!(graph.cycles().is_empty());
}

#[test]
fn a_parent_cycle_is_broken_at_its_first_member_whatever_the_input_order() {
    let files = [
        ("docs/spec/b.md", doc("MEC-B", "parent: MEC-A\n", "")),
        ("docs/spec/a.md", doc("MEC-A", "parent: MEC-B\n", "")),
        ("docs/spec/c.md", doc("MEC-C", "parent: MEC-C\n", "")),
        (
            "docs/spec/d.md",
            doc("MEC-D", "parent: MEC-B\n", "## S {#RULE-S}\n"),
        ),
        ("docs/spec/e.md", doc("MEC-E", "parent: MEC-F\n", "")),
        ("docs/spec/f.md", doc("MEC-F", "parent: MEC-G\n", "")),
        ("docs/spec/g.md", doc("MEC-G", "parent: MEC-E\n", "")),
    ];
    let mut views = Vec::new();
    for reverse in [false, true] {
        let mut ordered: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        if reverse {
            ordered.reverse();
        }
        let (config, input) = input(&ordered);
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let cycles: Vec<Vec<String>> = graph
            .cycles()
            .iter()
            .map(|members| names(&graph, members))
            .collect();
        assert_eq!(
            cycles,
            [
                vec!["MEC-A", "MEC-B"],
                vec!["MEC-C"],
                vec!["MEC-E", "MEC-F", "MEC-G"]
            ]
        );
        let a = find(&graph, "MEC-A");
        let b = find(&graph, "MEC-B");
        let c = find(&graph, "MEC-C");
        assert_eq!(
            graph.parent(a),
            Some(Parent::Cycle {
                members: vec![a, b]
            })
        );
        assert_eq!(graph.parent(c), Some(Parent::Cycle { members: vec![c] }));
        assert_eq!(graph.listed_under(a), None);
        assert_eq!(graph.listed_under(c), None);
        assert_eq!(graph.listed_under(b), Some(a));
        let s = find(&graph, "RULE-S");
        assert_eq!(
            names(&graph, &graph.ancestors(s)),
            ["MEC-D", "MEC-B", "MEC-A"]
        );
        assert!(graph.ancestors(a).is_empty());
        // Every node is reachable once from the roots: a tree, no loop.
        let mut seen = Vec::new();
        let mut stack: Vec<NodeAt> = graph
            .documents()
            .filter(|&at| graph.listed_under(at).is_none())
            .collect();
        while let Some(at) = stack.pop() {
            assert!(!seen.contains(&at), "{} twice", graph.name(at));
            seen.push(at);
            stack.extend(graph.children(at));
        }
        let total: usize = (0..graph.paths().len()).map(|f| graph.nodes(f).len()).sum();
        assert_eq!(seen.len(), total);
        let mut view: Vec<(String, Option<String>)> = seen
            .iter()
            .map(|&at| {
                (
                    graph.name(at),
                    graph.listed_under(at).map(|p| graph.name(p)),
                )
            })
            .collect();
        view.sort();
        views.push(view);
    }
    assert_eq!(views[0], views[1], "input order changes the tree");
}

/// A three-node `depends_on` cycle plus a self-link: the walk visits each
/// node once at its first distance, lists every followed edge, stops at
/// the depth bound; the impact table walks the same links backwards.
#[test]
fn the_walk_keeps_a_visited_set_and_a_depth_bound() {
    let (config, input) = input(&[
        (
            "docs/spec/a.md",
            &doc("MEC-A", "links:\n  depends_on: [MEC-B, MEC-A]\n", ""),
        ),
        (
            "docs/spec/b.md",
            &doc("MEC-B", "links:\n  depends_on: [MEC-C]\n", ""),
        ),
        (
            "docs/spec/c.md",
            &doc(
                "MEC-C",
                "links:\n  depends_on: [MEC-A]\n  derived_from: [R-01]\n",
                "Mentions R-02.\n",
            ),
        ),
        ("docs/records/r1.md", &doc("R-01", "", "")),
        ("docs/records/r2.md", &doc("R-02", "", "")),
    ]);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let a = find(&graph, "MEC-A");
    let walk = graph.walk(&[a], graph_direction, None, |_| true);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [
            ("MEC-A".to_owned(), 0),
            ("MEC-B".to_owned(), 1),
            ("MEC-C".to_owned(), 2),
            ("R-01".to_owned(), 3)
        ]
    );
    let edges: Vec<String> = walk.edges.iter().map(|&e| render(&graph, e)).collect();
    assert_eq!(
        edges,
        [
            "MEC-A --depends_on--> MEC-B @docs/spec/a.md:5",
            "MEC-A --depends_on--> MEC-A @docs/spec/a.md:5",
            "MEC-B --depends_on--> MEC-C @docs/spec/b.md:5",
            "MEC-C --depends_on--> MEC-A @docs/spec/c.md:5",
            "MEC-C --derived_from--> R-01 @docs/spec/c.md:6",
        ]
    );
    assert!(walk.left_out.is_empty());
    // Depth bounds: nodes at the bound are not expanded.
    let walk = graph.walk(&[a], graph_direction, Some(1), |_| true);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [("MEC-A".to_owned(), 0), ("MEC-B".to_owned(), 1)]
    );
    assert_eq!(walk.edges.len(), 2, "only MEC-A's edges");
    let walk = graph.walk(&[a], graph_direction, Some(0), |_| true);
    assert_eq!(reached(&graph, &walk.nodes), [("MEC-A".to_owned(), 0)]);
    assert!(walk.edges.is_empty());
    // `mentions` only when asked.
    let c = find(&graph, "MEC-C");
    let mentions = |t: &str| (t == "mentions").then_some(Direction::Out);
    let walk = graph.walk(&[c], mentions, None, |_| true);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [("MEC-C".to_owned(), 0), ("R-02".to_owned(), 1)]
    );
    // Impact: `depends_on` and `derived_from` backwards.
    let r1 = find(&graph, "R-01");
    let walk = graph.walk(&[r1], impact_direction, None, |_| true);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [
            ("R-01".to_owned(), 0),
            ("MEC-C".to_owned(), 1),
            ("MEC-B".to_owned(), 2),
            ("MEC-A".to_owned(), 3)
        ]
    );
    // Edges written in a refused file are counted, never followed.
    let b_file = find(&graph, "MEC-B").file;
    let walk = graph.walk(&[a], graph_direction, None, |file| file != b_file);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [("MEC-A".to_owned(), 0), ("MEC-B".to_owned(), 1)]
    );
    assert_eq!(walk.left_out.len(), 1);
    // Several starts, one of them repeated: each once at 0.
    let walk = graph.walk(&[a, c, a], graph_direction, Some(0), |_| true);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [("MEC-A".to_owned(), 0), ("MEC-C".to_owned(), 0)]
    );
}

#[test]
fn links_land_on_the_innermost_id_section_of_an_anchor() {
    let (config, input) = input(&[
        (
            "docs/spec/t.md",
            &doc(
                "MEC-T",
                "",
                "## Regen {#RULE-REGEN}\n\n### Delay\n\n<a name=\"pause\"></a>\n\nx\n\n## Notes\n\ny\n",
            ),
        ),
        (
            "docs/spec/u.md",
            &doc(
                "MEC-U",
                "",
                "See [d](t.md#delay), [p](t.md#pause), [n](t.md#notes), [r](t.md#RULE-REGEN), [x](t.md#nowhere), [t](t.md), [l](../../LICENSE), [g](gone.md) and other:R-01.\n",
            ),
        ),
        (
            "docs/records/d.md",
            "---\nid: DEC-0001\nclass: decision\nstatus: accepted\ncanon: docs/spec/t.md#delay\n---\n\n# D\n",
        ),
    ]);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let u = find(&graph, "MEC-U");
    let rendered: Vec<(String, LinkState, Option<String>)> = graph
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.file == u.file)
        .map(|(index, edge)| {
            (
                render(&graph, index),
                edge.state(),
                edge.reason().map(str::to_owned),
            )
        })
        .collect();
    let want = [
        (
            "MEC-U --mentions--> RULE-REGEN @docs/spec/u.md:8",
            LinkState::Resolved,
        ),
        (
            "MEC-U --mentions--> RULE-REGEN @docs/spec/u.md:8",
            LinkState::Resolved,
        ),
        (
            "MEC-U --mentions--> MEC-T @docs/spec/u.md:8",
            LinkState::Resolved,
        ),
        (
            "MEC-U --mentions--> RULE-REGEN @docs/spec/u.md:8",
            LinkState::Resolved,
        ),
        (
            "MEC-U --mentions--> MEC-T @docs/spec/u.md:8",
            LinkState::Resolved,
        ),
        (
            "MEC-U --mentions--> MEC-T @docs/spec/u.md:8",
            LinkState::Resolved,
        ),
        (
            "MEC-U --mentions--> ?../../LICENSE @docs/spec/u.md:8",
            LinkState::Unchecked,
        ),
        (
            "MEC-U --mentions--> !gone.md @docs/spec/u.md:8",
            LinkState::Dangling,
        ),
        (
            "MEC-U --mentions--> ~other:R-01 @docs/spec/u.md:8",
            LinkState::Skipped,
        ),
    ];
    assert_eq!(
        rendered
            .iter()
            .map(|(line, state, _)| (line.as_str(), *state))
            .collect::<Vec<_>>(),
        want,
        "{rendered:#?}"
    );
    // The anchor naming nothing lands on the document; the reason names it.
    let nowhere = &rendered[4];
    assert!(
        nowhere.2.as_deref().is_some_and(|r| r.contains("nowhere")),
        "{nowhere:?}"
    );
    assert!(rendered[0].2.is_none(), "{:?}", rendered[0]);
    assert!(rendered[7].2.is_some(), "a dangling link says why");
    // A path `canon:` with a slug anchor lands on the section holding it.
    let regen = find(&graph, "RULE-REGEN");
    let canon: Vec<String> = graph
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.link_type == "canon")
        .map(|(index, _)| render(&graph, index))
        .collect();
    assert_eq!(
        canon,
        ["DEC-0001 --canon--> RULE-REGEN @docs/records/d.md:5"]
    );
    // Incoming at the document: every link into it or a nested section.
    let t = find(&graph, "MEC-T");
    let links = graph.links(t, |_| true);
    assert!(links.outgoing.is_empty());
    assert_eq!(links.incoming.len(), 7, "{links:?}");
    assert_eq!(
        links
            .incoming
            .iter()
            .filter(|(_, landed)| *landed == regen)
            .count(),
        4
    );
    let refused = graph.links(t, |file| file != u.file);
    assert_eq!(refused.incoming.len(), 1);
    assert_eq!(refused.left_out.len(), 6);
    // Outgoing of a section: only what its span holds.
    let links = graph.links(regen, |_| true);
    assert!(links.outgoing.is_empty());
    assert_eq!(links.incoming.len(), 4);
}

#[test]
fn standing_and_superseded_by_follow_the_live_rule() {
    let (config, input) = input(&[
        ("docs/spec/live.md", &doc("MEC-L", "", "")),
        (
            "docs/records/gen.md",
            "---\nclass: generated\ngenerator: g\nsource: s\n---\n\n# G\n\nSee MEC-L.\n",
        ),
        (
            "docs/records/old.md",
            "---\nid: DEC-0001\nclass: decision\nstatus: superseded-by DEC-0002\n---\n\n# Old\n",
        ),
        (
            "docs/records/new.md",
            "---\nid: DEC-0002\nclass: decision\nstatus: accepted\n---\n\n# New\n",
        ),
        (
            "docs/features/done.md",
            "---\nclass: spec\nstatus: shipped\nscope: [x]\n---\n\n# Done\n",
        ),
        (
            "docs/spec/broken.md",
            "---\nid: [unclosed\n---\n\n# Broken\n",
        ),
    ]);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let standing = |path: &str| graph.standing(graph.file_of(path).unwrap());
    assert_eq!(standing("docs/spec/live.md"), Standing::Live);
    assert_eq!(standing("docs/records/gen.md"), Standing::Generated);
    assert_eq!(standing("docs/records/old.md"), Standing::Tier3);
    assert_eq!(standing("docs/records/new.md"), Standing::Live);
    assert_eq!(standing("docs/features/done.md"), Standing::Tier3);
    assert_eq!(standing("docs/spec/broken.md"), Standing::Live);
    assert!(graph.is_tier3(graph.file_of("docs/records/old.md").unwrap()));
    // `status: superseded-by X`: X supersedes the document, written in it.
    let old = find(&graph, "DEC-0001");
    let new = find(&graph, "DEC-0002");
    let superseding: Vec<&specengine_core::check::Edge> = graph
        .edges()
        .iter()
        .filter(|edge| edge.link_type == "supersedes")
        .collect();
    assert_eq!(superseding.len(), 1);
    let edge = superseding[0];
    assert!(edge.names_source);
    assert_eq!(edge.source, Endpoint::Nodes(vec![new]));
    assert_eq!(edge.target, Endpoint::Nodes(vec![old]));
    assert_eq!(graph.paths()[edge.file], "docs/records/old.md");
    // Listed incoming on the superseded one, not outgoing.
    let links = graph.links(old, |_| true);
    assert!(links.outgoing.is_empty(), "{links:?}");
    assert_eq!(links.incoming.len(), 1);
    // On the superseding one: outgoing, though written in the other file,
    // leaving the node itself; its file refused, counted, not listed.
    let index = graph
        .edges()
        .iter()
        .position(|edge| edge.link_type == "supersedes")
        .unwrap();
    let links = graph.links(new, |_| true);
    assert_eq!(links.outgoing, [(index, new)], "{links:?}");
    assert!(links.incoming.is_empty(), "{links:?}");
    assert!(links.left_out.is_empty(), "{links:?}");
    let old_file = graph.file_of("docs/records/old.md").unwrap();
    let links = graph.links(new, |file| file != old_file);
    assert!(links.outgoing.is_empty(), "{links:?}");
    assert!(links.incoming.is_empty(), "{links:?}");
    assert_eq!(links.left_out, [index], "{links:?}");
    // The superseded one's own view does not depend on admitting itself.
    let links = graph.links(old, |file| file == old_file);
    assert_eq!(links.incoming.len(), 1, "{links:?}");
}

/// `status: superseded-by X` naming no node: the edge dangles at its
/// source, written at D's `status:` line, still listed incoming on D (the
/// check's `ref-dangling` place).
#[test]
fn a_dangling_superseded_by_is_incoming_on_its_document() {
    let (config, input) = input(&[(
        "docs/records/note.md",
        "---\nstatus: superseded-by DEC-0404\n---\n\n# Note\n",
    )]);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let note = graph
        .document(graph.file_of("docs/records/note.md").unwrap())
        .unwrap();
    let links = graph.links(note, |_| true);
    assert!(links.outgoing.is_empty(), "{links:?}");
    assert_eq!(links.incoming.len(), 1, "{links:?}");
    let (index, landed) = links.incoming[0];
    assert_eq!(landed, note);
    let edge = &graph.edges()[index];
    assert_eq!(edge.link_type, "supersedes");
    assert!(edge.names_source);
    assert!(
        matches!(edge.source, Endpoint::Dangling(_)),
        "{:?}",
        edge.source
    );
    assert_eq!(edge.state(), LinkState::Dangling);
    assert_eq!(edge.line, 2);
    assert_eq!(edge.written, "DEC-0404");
}

/// The fixtures through the graph: spec-a's and spec-b's containment
/// matches the spec's example shape; the default and impact walks give
/// AC-13's distances.
#[test]
fn both_fixtures_walk_as_the_spec_says() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let r12 = find(&graph, "R-12");
    let walk = graph.walk(&[r12], impact_direction, None, |file| {
        graph.standing(file) == Standing::Live
    });
    assert_eq!(
        reached(&graph, &walk.nodes),
        [
            ("R-12".to_owned(), 0),
            ("MEC-STAMINA".to_owned(), 1),
            ("MEC-SPRINT".to_owned(), 2),
            ("RULE-STAM-REGEN".to_owned(), 3)
        ]
    );
    let sprint = find(&graph, "MEC-SPRINT");
    assert_eq!(
        names(&graph, &graph.ancestors(find(&graph, "EDGE-SPRINT-EMPTY"))),
        ["RULE-SPRINT-COST", "MEC-SPRINT", "DOM-MOVEMENT", "DOM-GAME"]
    );
    assert_eq!(
        names(&graph, graph.children(find(&graph, "DOM-MOVEMENT"))),
        ["RULE-MOVE-SPEEDS", "MEC-SPRINT", "MEC-STAMINA"]
    );
    assert_eq!(graph.line(sprint), 1);
    assert_eq!(graph.line(find(&graph, "EDGE-SPRINT-EMPTY")), 22);

    let (config, input) = fixture_input(&fixture("spec-b"));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let req2 = find(&graph, "REQ-002");
    let walk = graph.walk(&[req2], impact_direction, None, |_| true);
    assert_eq!(
        reached(&graph, &walk.nodes),
        [("REQ-002".to_owned(), 0), ("MOD-CLI".to_owned(), 1)]
    );
    let edge = &graph.edges()[walk.edges[0]];
    assert_eq!(edge.written, "\u{0422}\u{0420}\u{0411}-002");
    assert_eq!(
        names(&graph, graph.children(find(&graph, "MOD-CLI"))),
        ["CMD-SYNC", "CMD-STATUS"]
    );
}

/// The model's one table for every project (Q1, Q2).
#[test]
fn the_link_type_tables_are_the_spec_s() {
    assert_eq!(
        IMPACT_LINK_TYPES,
        [
            ("depends_on", Direction::In),
            ("derived_from", Direction::In),
            ("verifies", Direction::In),
            ("uses_term", Direction::In),
            ("constrains", Direction::Out),
        ]
    );
    for link_type in LINK_TYPES {
        assert_eq!(
            graph_direction(link_type),
            Some(Direction::Out),
            "{link_type}"
        );
        assert!(!is_weak_link(link_type), "{link_type}");
        let impact = match link_type {
            "depends_on" | "derived_from" | "verifies" | "uses_term" => Some(Direction::In),
            "constrains" => Some(Direction::Out),
            _ => None,
        };
        assert_eq!(impact_direction(link_type), impact, "{link_type}");
    }
    assert_eq!(graph_direction("mentions"), None);
    assert!(is_weak_link("mentions"));
    assert_eq!(impact_direction("mentions"), None);
    // An unknown declared type is strong, followed outgoing, not by impact.
    assert_eq!(graph_direction("inspired_by"), Some(Direction::Out));
    assert!(!is_weak_link("inspired_by"));
    assert_eq!(impact_direction("inspired_by"), None);
    assert_eq!(Direction::Out.as_str(), "out");
    assert_eq!(Direction::In.as_str(), "in");
}
