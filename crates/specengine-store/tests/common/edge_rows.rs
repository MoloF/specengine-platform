//! The comparison of docs/features/edge-table.md "Acceptance criteria"
//! AC-2: the stored edge rows (`SqliteIndex::stored_edges`) against core's
//! resolution, `SpecGraph::new` over the index-fed input and over a fresh
//! parse of the bytes, on every stored column, grouped by file. Shared by
//! the store's `tests/edges.rs` and eval's `tests/scale_edges.rs` (through
//! `#[path]`).
//!
//! Parents ("Data", the rows of `edge_parents`): a document whose
//! `SpecGraph::parent` is not `Cycle` equals its stored row, `Parent::None`
//! is no row; a cycle's breaker has a `resolved` row whose first end is a
//! member of its cycle (the table holds `parent:` before cycle breaking).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use specengine_core::Paths;
use specengine_core::check::{CheckInput, Endpoint, LinkState, Parent, SpecGraph};
use specengine_model::{EdgeLabel, IdScheme, LinkOrigin};
use specengine_store::{SpecIndex, SqliteIndex, WorkingTree, check_input};

/// The text of a stored state or origin, whichever type carries it.
pub trait AsText {
    fn as_text(&self) -> String;
}

impl AsText for LinkState {
    fn as_text(&self) -> String {
        self.as_str().to_owned()
    }
}

impl AsText for LinkOrigin {
    fn as_text(&self) -> String {
        match self {
            LinkOrigin::Frontmatter => "frontmatter".to_owned(),
            LinkOrigin::Inline => "inline".to_owned(),
        }
    }
}

impl AsText for String {
    fn as_text(&self) -> String {
        self.clone()
    }
}

impl AsText for &str {
    fn as_text(&self) -> String {
        (*self).to_owned()
    }
}

/// One edge as the table holds it: the citing file's path and the edge's
/// position among its edges, the written end's nodes as `(path, ord)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRow {
    pub path: String,
    pub ord: usize,
    pub holder: usize,
    pub link_type: String,
    pub origin: String,
    pub names_source: bool,
    pub state: String,
    pub reason: Option<String>,
    pub label: Option<EdgeLabel>,
    pub at: usize,
    pub ends: Vec<(String, usize)>,
}

/// One document's `parent:` as the table holds it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParentRow {
    pub path: String,
    pub state: String,
    pub reason: Option<String>,
    pub ends: Vec<(String, usize)>,
}

/// What core gives for one input: the edge rows, the parent rows of every
/// document not breaking a cycle, and each breaker's path with its cycle.
pub struct CoreRows {
    pub edges: Vec<EdgeRow>,
    pub parents: Vec<ParentRow>,
    pub breakers: Vec<(String, Vec<(String, usize)>)>,
}

/// The stored rows of the handle's worktree, by `(path, ord)` and by path.
pub fn stored_rows(index: &SqliteIndex) -> (Vec<EdgeRow>, Vec<ParentRow>) {
    let stored = index.stored_edges().expect("stored_edges reads");
    let edges: Vec<EdgeRow> = stored
        .edges
        .iter()
        .map(|edge| EdgeRow {
            path: edge.path.clone(),
            ord: edge.ord,
            holder: edge.holder,
            link_type: edge.link_type.clone(),
            origin: edge.origin.as_text(),
            names_source: edge.names_source,
            state: edge.state.as_text(),
            reason: edge.reason.clone(),
            label: edge.label.clone(),
            at: edge.at,
            ends: edge.ends.clone(),
        })
        .collect();
    let parents: Vec<ParentRow> = stored
        .parents
        .iter()
        .map(|parent| ParentRow {
            path: parent.path.clone(),
            state: parent.state.as_text(),
            reason: parent.reason.clone(),
            ends: parent.ends.clone(),
        })
        .collect();
    let keys: Vec<(&str, usize)> = edges.iter().map(|e| (e.path.as_str(), e.ord)).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(keys, sorted, "stored_edges: by (path, ord), each once");
    let paths: Vec<&str> = parents.iter().map(|p| p.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(paths, sorted, "stored_edges: parents by path, each once");
    (edges, parents)
}

/// Core's rows for `input`: `SpecGraph::new`'s edges, numbered per file in
/// their order, the written end projected; each document's parent.
pub fn core_rows(input: &CheckInput, scheme: &IdScheme, paths: &Paths) -> CoreRows {
    let graph = SpecGraph::new(input, scheme, paths);
    let names = graph.paths();
    let at = |file: usize, ord: usize| (names[file].to_owned(), ord);
    let mut next: BTreeMap<usize, usize> = BTreeMap::new();
    let mut edges = Vec::new();
    for edge in graph.edges() {
        let ord = next.entry(edge.file).or_default();
        assert_eq!(edge.holder.file, edge.file, "a holder lies in its file");
        edges.push(EdgeRow {
            path: names[edge.file].to_owned(),
            ord: *ord,
            holder: edge.holder.ord,
            link_type: edge.link_type.clone(),
            origin: edge.origin.as_text(),
            names_source: edge.names_source,
            state: edge.state().as_str().to_owned(),
            reason: edge.reason().map(str::to_owned),
            label: edge.edge.clone(),
            at: edge.offset,
            ends: match edge.written_end() {
                Endpoint::Nodes(nodes) => nodes.iter().map(|n| at(n.file, n.ord)).collect(),
                _ => Vec::new(),
            },
        });
        *ord += 1;
    }
    edges.sort();
    let mut parents = Vec::new();
    let mut breakers = Vec::new();
    for document in graph.documents() {
        let path = names[document.file].to_owned();
        match graph.parent(document) {
            None | Some(Parent::None) => {}
            Some(Parent::Node { node, others }) => parents.push(ParentRow {
                path,
                state: "resolved".to_owned(),
                reason: None,
                ends: std::iter::once(node)
                    .chain(others)
                    .map(|n| at(n.file, n.ord))
                    .collect(),
            }),
            Some(Parent::Dangling { reason, .. }) => parents.push(ParentRow {
                path,
                state: "dangling".to_owned(),
                reason: Some(reason),
                ends: Vec::new(),
            }),
            Some(Parent::Skipped { .. }) => parents.push(ParentRow {
                path,
                state: "skipped".to_owned(),
                reason: None,
                ends: Vec::new(),
            }),
            Some(Parent::Cycle { members }) => {
                breakers.push((path, members.iter().map(|n| at(n.file, n.ord)).collect()))
            }
        }
    }
    parents.sort();
    CoreRows {
        edges,
        parents,
        breakers,
    }
}

fn by_file<T: Clone>(rows: &[T], path: impl Fn(&T) -> &str) -> BTreeMap<String, Vec<T>> {
    let mut out: BTreeMap<String, Vec<T>> = BTreeMap::new();
    for row in rows {
        out.entry(path(row).to_owned())
            .or_default()
            .push(row.clone());
    }
    out
}

/// Asserts the stored rows equal core's rows for `input`; `what` names the
/// input in a failure.
pub fn assert_rows_equal(
    stored: &(Vec<EdgeRow>, Vec<ParentRow>),
    input: &CheckInput,
    scheme: &IdScheme,
    paths: &Paths,
    context: &str,
    what: &str,
) {
    let core = core_rows(input, scheme, paths);
    let (edges, parents) = stored;
    let stored_files = by_file(edges, |row| row.path.as_str());
    let core_files = by_file(&core.edges, |row| row.path.as_str());
    let files: BTreeSet<&String> = stored_files.keys().chain(core_files.keys()).collect();
    for file in files {
        let left = stored_files.get(file).cloned().unwrap_or_default();
        let right = core_files.get(file).cloned().unwrap_or_default();
        assert!(
            left == right,
            "{context}: the stored edges of `{file}` differ from SpecGraph::new over {what}\n\
             stored: {left:#?}\ncore: {right:#?}"
        );
    }
    let breakers: BTreeMap<&str, &Vec<(String, usize)>> = core
        .breakers
        .iter()
        .map(|(path, members)| (path.as_str(), members))
        .collect();
    let (broken, plain): (Vec<&ParentRow>, Vec<&ParentRow>) = parents
        .iter()
        .partition(|row| breakers.contains_key(row.path.as_str()));
    let plain: Vec<ParentRow> = plain.into_iter().cloned().collect();
    assert!(
        plain == core.parents,
        "{context}: the stored parents differ from SpecGraph::new over {what}\n\
         stored: {plain:#?}\ncore: {:#?}",
        core.parents
    );
    for (path, members) in &breakers {
        let row = broken.iter().find(|row| row.path == *path);
        let Some(row) = row else {
            panic!("{context}: the cycle breaker `{path}` has no stored parent ({what})");
        };
        assert!(
            row.state == "resolved" && row.ends.first().is_some_and(|end| members.contains(end)),
            "{context}: the cycle breaker `{path}`'s stored parent {row:?} is no resolved \
             member of its cycle {members:?} ({what})"
        );
    }
}

/// AC-2 on one index: its stored rows equal `SpecGraph::new` over its own
/// `indexed_input` and over `check_input` of the working tree at `root`.
pub fn assert_stored_edges(
    index: &SqliteIndex,
    root: &Path,
    scheme: &IdScheme,
    paths: &Paths,
    context: &str,
) {
    let stored = stored_rows(index);
    let indexed = index.indexed_input().expect("indexed_input");
    assert_rows_equal(&stored, &indexed, scheme, paths, context, "indexed_input");
    let tree = WorkingTree::new(root, paths).expect("working tree");
    let bytes = check_input(&tree, scheme);
    assert_rows_equal(&stored, &bytes, scheme, paths, context, "the bytes");
}
