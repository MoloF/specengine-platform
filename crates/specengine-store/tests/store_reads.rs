//! docs/features/store-reads.md (RM-0079), AC-12's store half that needs
//! no private SQL ("The lookups serve the read"), over every configured
//! fixture (`fixtures/*/specengine.toml`):
//! - `SpecGraph::from_rows` over every stored row and the whole input is
//!   the whole graph: every edge, every node's links and ancestors.
//! - For every node m, one snapshot's `held(m)` and `ending(m)` are exactly
//!   the whole graph's edges m holds or whose written end holds m, each
//!   whole (all its ends); `from_rows` over them and their files holds, at
//!   each (path, `ord`), the whole graph's edge in `link_type`, `origin`,
//!   `holder`, `source`, `target`, `names_source`, `written`, `line`,
//!   `offset`, `reason()`, `edge` and `state()`.
//! - `holders(key)` is the files whose `given_keys` hold the key, by path;
//!   `parent(path)` is the stored `parent:` row.
//!
//! M: ENDING not completed by `EDGE`; edges in fetch order instead of
//! (path, `ord`); `EDGE` without `f.path = ?2` (another file's rows mixed
//! in). The plans and the 10k/100k costs are `src/edges.rs`' unit tests.
//! The fixtures are only read: each runs on a scratch copy.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{Corpus, Scratch};
use specengine_core::check::{
    CheckInput, Edge, Endpoint, GraphRows, NodeAt, SpecGraph, given_keys,
};
use specengine_store::{SqliteIndex, StoredEdge, check_input};

/// The configured fixtures, sorted.
fn configured_fixtures() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(common::repository_root().join("fixtures"))
        .expect("fixtures/")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("specengine.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

/// One fixture indexed on a scratch copy, with its whole input (bytes
/// attached).
struct Indexed {
    corpus: Corpus,
    index: SqliteIndex,
    input: CheckInput,
}

fn indexed(name: &str, scratch: &Scratch) -> Indexed {
    let corpus = Corpus::copy_of(name, scratch, name);
    let mut index = corpus.open(&scratch.db(name));
    corpus.update(&mut index);
    let input = check_input(&corpus.tree(), &corpus.scheme);
    Indexed {
        corpus,
        index,
        input,
    }
}

/// A node as (path, position).
fn node_key(graph: &SpecGraph<'_>, at: NodeAt) -> (String, usize) {
    (graph.paths()[at.file].to_owned(), at.ord)
}

fn endpoint_view(graph: &SpecGraph<'_>, end: &Endpoint) -> String {
    match end {
        Endpoint::Nodes(ends) => format!(
            "nodes {:?}",
            ends.iter()
                .map(|&at| node_key(graph, at))
                .collect::<Vec<_>>()
        ),
        Endpoint::Dangling(reason) => format!("dangling {reason}"),
        Endpoint::Skipped => "skipped".to_owned(),
        Endpoint::Unchecked => "unchecked".to_owned(),
    }
}

/// Every field AC-12 names, file indexes as paths.
fn edge_view(graph: &SpecGraph<'_>, edge: &Edge) -> String {
    format!(
        "{} {:?} file {} holder {:?} source {} target {} names_source {} written {:?} line {} \
         offset {} reason {:?} edge {:?} state {:?}",
        edge.link_type,
        edge.origin,
        graph.paths()[edge.file],
        node_key(graph, edge.holder),
        endpoint_view(graph, &edge.source),
        endpoint_view(graph, &edge.target),
        edge.names_source,
        edge.written,
        edge.line,
        edge.offset,
        edge.reason(),
        edge.edge,
        edge.state()
    )
}

/// The graph's edges by (citing path, `ord`): `ord` is the edge's position
/// among its file's edges (core's order).
fn by_key<'g>(graph: &'g SpecGraph<'_>) -> BTreeMap<(String, usize), &'g Edge> {
    let mut out = BTreeMap::new();
    let mut next: BTreeMap<usize, usize> = BTreeMap::new();
    for edge in graph.edges() {
        let ord = next.entry(edge.file).or_insert(0);
        out.insert((graph.paths()[edge.file].to_owned(), *ord), edge);
        *ord += 1;
    }
    out
}

/// Every node of the graph.
fn nodes(graph: &SpecGraph<'_>) -> Vec<NodeAt> {
    (0..graph.paths().len())
        .flat_map(|file| (0..graph.nodes(file).len()).map(move |ord| NodeAt { file, ord }))
        .collect()
}

/// A node's links as edge keys and nodes, for graphs over different inputs.
fn links_view(graph: &SpecGraph<'_>, at: NodeAt) -> String {
    let mut next: BTreeMap<usize, usize> = BTreeMap::new();
    let keys: Vec<(String, usize)> = graph
        .edges()
        .iter()
        .map(|edge| {
            let ord = next.entry(edge.file).or_insert(0);
            let key = (graph.paths()[edge.file].to_owned(), *ord);
            *ord += 1;
            key
        })
        .collect();
    let links = graph.links(at, |_| true);
    let side = |pairs: &[(usize, NodeAt)]| -> Vec<((String, usize), (String, usize))> {
        pairs
            .iter()
            .map(|&(edge, node)| (keys[edge].clone(), node_key(graph, node)))
            .collect()
    };
    format!(
        "out {:?} in {:?} left_out {:?}",
        side(&links.outgoing),
        side(&links.incoming),
        links
            .left_out
            .iter()
            .map(|&edge| keys[edge].clone())
            .collect::<Vec<_>>()
    )
}

/// AC-12: `from_rows` over every stored row and the whole input answers as
/// `SpecGraph::new`: the same edges in the same order, and every node's
/// links and ancestors.
#[test]
fn ac12_from_rows_over_every_stored_row_is_the_whole_graph() {
    let scratch = Scratch::new("store-reads-rows");
    let fixtures = configured_fixtures();
    // 20 since docs/features/workspaces.md added `fixtures/studio-lamp`, 21
    // since docs/features/flow-scenario.md added `fixtures/flow-aside`.
    assert_eq!(fixtures.len(), 21, "premise: the configured fixtures");
    let mut edges = 0;
    for name in &fixtures {
        let fixture = indexed(name, &scratch);
        let (scheme, paths) = (&fixture.corpus.scheme, &fixture.corpus.paths);
        let whole = SpecGraph::new(&fixture.input, scheme, paths);
        let stored = fixture.index.stored_edges().expect("stored edges");
        let rows = GraphRows {
            edges: stored.edges.clone(),
            parents: stored.parents.clone(),
        };
        let built = SpecGraph::from_rows(&fixture.input, scheme, paths, rows);
        let want: Vec<String> = whole.edges().iter().map(|e| edge_view(&whole, e)).collect();
        let got: Vec<String> = built.edges().iter().map(|e| edge_view(&built, e)).collect();
        assert_eq!(got.len(), want.len(), "{name}: the edge count");
        for (at, (got, want)) in got.iter().zip(&want).enumerate() {
            assert_eq!(got, want, "{name}: edge {at}");
        }
        for at in nodes(&whole) {
            assert_eq!(
                links_view(&built, at),
                links_view(&whole, at),
                "{name}: links of {:?}",
                node_key(&whole, at)
            );
            assert_eq!(
                built.ancestors(at),
                whole.ancestors(at),
                "{name}: ancestors of {:?}",
                node_key(&whole, at)
            );
        }
        edges += want.len();
    }
    assert!(edges > 100, "premise: the fixtures' edges ({edges})");
}

/// AC-12: for every node m of every fixture, `held(m)` and `ending(m)` are
/// the whole graph's edges m holds and whose written end holds m, each
/// whole and by (path, `ord`); `from_rows` over them, on the files of
/// their ends, holds the whole graph's edge at each (path, `ord`).
#[test]
fn ac12_each_nodes_rows_build_its_subset_graph() {
    let scratch = Scratch::new("store-reads-node");
    let mut checked = 0;
    for name in configured_fixtures() {
        let fixture = indexed(&name, &scratch);
        checked += node_rows_build_subset_graphs(&fixture, &name);
    }
    assert!(checked > 200, "premise: subset edges checked ({checked})");
}

/// [`ac12_each_nodes_rows_build_its_subset_graph`] over one indexed copy;
/// the subset edges it checked.
fn node_rows_build_subset_graphs(fixture: &Indexed, name: &str) -> usize {
    let mut checked = 0;
    {
        let (scheme, paths) = (&fixture.corpus.scheme, &fixture.corpus.paths);
        let whole = SpecGraph::new(&fixture.input, scheme, paths);
        let keyed = by_key(&whole);
        let stored = fixture.index.stored_edges().expect("stored edges");
        let stored_by_key: BTreeMap<(String, usize), &StoredEdge> = stored
            .edges
            .iter()
            .map(|edge| ((edge.path.clone(), edge.ord), edge))
            .collect();
        let snapshot = fixture.index.snapshot().expect("snapshot");
        for m in nodes(&whole) {
            let (path, ord) = node_key(&whole, m);
            let held = snapshot.held(&path, ord).expect("held");
            let ending = snapshot.ending(&path, ord).expect("ending");
            let context = format!("{name} ({path}, {ord})");
            for (what, rows) in [("held", &held), ("ending", &ending)] {
                let order: Vec<(String, usize)> =
                    rows.iter().map(|row| (row.path.clone(), row.ord)).collect();
                let mut sorted = order.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(order, sorted, "{context}: {what} by (path, ord), each once");
                for row in rows.iter() {
                    assert_eq!(
                        Some(&row),
                        stored_by_key.get(&(row.path.clone(), row.ord)),
                        "{context}: {what} gives the edge whole"
                    );
                }
            }
            let want_held: BTreeSet<(String, usize)> = keyed
                .iter()
                .filter(|(_, edge)| edge.holder == m)
                .map(|(key, _)| key.clone())
                .collect();
            let want_ending: BTreeSet<(String, usize)> = keyed
                .iter()
                .filter(|(_, edge)| {
                    matches!(edge.written_end(), Endpoint::Nodes(ends) if ends.contains(&m))
                })
                .map(|(key, _)| key.clone())
                .collect();
            let got_held: BTreeSet<(String, usize)> =
                held.iter().map(|row| (row.path.clone(), row.ord)).collect();
            let got_ending: BTreeSet<(String, usize)> = ending
                .iter()
                .map(|row| (row.path.clone(), row.ord))
                .collect();
            assert_eq!(got_held, want_held, "{context}: HELD");
            assert_eq!(got_ending, want_ending, "{context}: ENDING");

            let mut rows: BTreeMap<(String, usize), StoredEdge> = BTreeMap::new();
            for row in held.into_iter().chain(ending) {
                rows.insert((row.path.clone(), row.ord), row);
            }
            let mut files: BTreeSet<String> = BTreeSet::from([path.clone()]);
            for row in rows.values() {
                files.insert(row.path.clone());
                files.extend(row.ends.iter().map(|(end, _)| end.clone()));
            }
            let subset = CheckInput {
                files: fixture
                    .input
                    .files
                    .iter()
                    .filter(|file| files.contains(&file.path))
                    .cloned()
                    .collect(),
                problems: Vec::new(),
            };
            let rows: Vec<StoredEdge> = rows.into_values().collect();
            let built = SpecGraph::from_rows(
                &subset,
                scheme,
                paths,
                GraphRows {
                    edges: rows.clone(),
                    parents: Vec::new(),
                },
            );
            assert_eq!(
                built.edges().len(),
                rows.len(),
                "{context}: one edge per row"
            );
            for (row, edge) in rows.iter().zip(built.edges()) {
                let key = (row.path.clone(), row.ord);
                let want = keyed
                    .get(&key)
                    .unwrap_or_else(|| panic!("{context}: {key:?} is a whole edge"));
                assert_eq!(
                    edge_view(&built, edge),
                    edge_view(&whole, want),
                    "{context}: the subset graph's edge {key:?}"
                );
                checked += 1;
            }
        }
    }
    checked
}

/// Twins of two `spec-a` documents, by bytes: the record `A-101` (a
/// mention's end) and the target's `parent:` document, each ID then held
/// by two files.
fn with_twins(fixture: &Indexed) -> Vec<String> {
    let records = &fixture.corpus.paths.records;
    let spec = &fixture.corpus.paths.spec;
    let mut twins = Vec::new();
    for (from, to) in [
        (
            format!("{records}/A/A-101.md"),
            format!("{records}/A/A-101-twin.md"),
        ),
        (
            format!("{spec}/movement/README.md"),
            format!("{spec}/movement/README-twin.md"),
        ),
    ] {
        let bytes = fixture.corpus.bytes(&from);
        fixture.corpus.write(&to, bytes);
        twins.push(to);
    }
    twins
}

/// AC-12 and SR-0 M "ENDING not completed by `EDGE`": in a `spec-a` copy
/// where a mention's end and a `parent:` each have two holders (no fixture
/// has one), every resolved edge and parent row naming them holds both
/// ends; `ending(m)` gives such an edge whole (all its ends, not only m)
/// and the per-node subset graphs hold the whole graph's edges.
#[test]
fn ac12_ids_with_two_holders_end_whole() {
    let scratch = Scratch::new("store-reads-twins");
    let mut fixture = indexed("spec-a", &scratch);
    with_twins(&fixture);
    fixture.corpus.update(&mut fixture.index);
    fixture.input = check_input(&fixture.corpus.tree(), &fixture.corpus.scheme);
    let stored = fixture.index.stored_edges().expect("stored edges");
    assert!(
        stored.edges.iter().any(|edge| edge.ends.len() > 1),
        "premise: an edge with two ends"
    );
    assert!(
        stored.parents.iter().any(|parent| parent.ends.len() > 1),
        "premise: a parent row with two ends"
    );
    let checked = node_rows_build_subset_graphs(&fixture, "spec-a with twins");
    assert!(checked > 20, "premise: subset edges checked ({checked})");
}

/// AC-12: `holders(key)` is, by path, the files whose `given_keys` hold the
/// key, for every key any file gives and a key none gives; `parent(path)`
/// is the stored `parent:` row of every file, `None` without one.
#[test]
fn ac12_holders_and_parent_are_the_stored_rows() {
    let scratch = Scratch::new("store-reads-keys");
    let mut keys_checked = 0;
    for name in configured_fixtures() {
        let fixture = indexed(&name, &scratch);
        let mut holders: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in &fixture.input.files {
            for key in given_keys(file) {
                holders
                    .entry(key.text())
                    .or_default()
                    .push(file.path.clone());
            }
        }
        let stored = fixture.index.stored_edges().expect("stored edges");
        let snapshot = fixture.index.snapshot().expect("snapshot");
        for (key, mut want) in holders {
            want.sort();
            want.dedup();
            assert_eq!(
                snapshot.holders(&key).expect("holders"),
                want,
                "{name}: holders({key})"
            );
            keys_checked += 1;
        }
        assert!(
            snapshot
                .holders("name:NO-SUCH-KEY-404")
                .expect("holders")
                .is_empty(),
            "{name}: a key no file gives"
        );
        for file in &fixture.input.files {
            let want = stored
                .parents
                .iter()
                .find(|parent| parent.path == file.path);
            assert_eq!(
                snapshot.parent(&file.path).expect("parent").as_ref(),
                want,
                "{name}: parent({})",
                file.path
            );
        }
    }
    assert!(keys_checked > 300, "premise: keys checked ({keys_checked})");
}
