//! The whole corpus as one map, the pure half (task spec `corpus-map` CM1,
//! CM2): every node of the admitted files, the resolved links between
//! them, the documents' parents and the unresolved targets, with each
//! node's uses from [`usage_of`]. Computed per read, never stored.
//!
//! - **Admitted**: a live file ([`Standing::Live`]), a Tier 3 one when the
//!   reader asks for the archive, never a generated one.
//! - **Nodes**: each node of each admitted file, documents and ID sections,
//!   in (path, position) order; a section names its document by index.
//! - **Edges**: the resolved links whose two ends are admitted nodes, the
//!   source not the target, one per (source, target, type), `places` the
//!   links merged. An end with several holders lands on the first admitted
//!   one by (path, position), so a Tier 3 holder ahead of a live one never
//!   drops an edge `usage_of` counts; none admitted, the edge is none.
//!   Sorted by (source, target, type).
//! - **Parents**: each admitted document whose `parent:` resolves to an
//!   admitted node (its first admitted holder, as an edge's end), by child.
//! - **Ghosts**: the written ends of the dangling links of the admitted
//!   files, one per written text (a wiki mention's without its brackets and
//!   label), its sources the nodes holding them, as a set; its reason the
//!   first place's. Skipped and unchecked ends are none. Sorted by written
//!   text.
//! - **Uses**: [`usage_of`]'s, which count live sources alone whatever the
//!   reader admits.
//! - **Names** ([`node_names`], [`MapNaming`]): a node's canonical
//!   reference, as `collision-names` names one: `slug/ID` in a feature
//!   document for a feature-scoped ID or an ID more than one node of the
//!   graph holds; else its ID; else, ID-less, its path. A feature-scoped
//!   reference resolves within its feature document, so its edges already
//!   land on that holder.
//!
//! The core reads no kind, link type or class name: only the model's weak
//! type. One graph gives one map whatever the order its files came in.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::{IdScheme, is_weak_link};

use crate::check::resolve::feature_stem;
use crate::check::{Endpoint, LinkState, NodeAt, Parent, SpecGraph, Standing, feature_scoped};
use crate::usage::{NodeUsage, usage_of};

/// What the map names a node by (task spec `corpus-map` CM1, as task spec
/// `collision-names` names one): the project's ID scheme and its features
/// directory, read through [`feature_stem`] and [`feature_scoped`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapNaming<'a> {
    /// `[ids]`: which IDs are feature-scoped.
    pub scheme: &'a IdScheme,
    /// `[paths] features`: a document directly under it is a feature
    /// document, its stem the slug.
    pub features: &'a str,
}

/// Each node's name, parallel to `map.nodes` (task spec `corpus-map` CM1):
/// `slug/ID` when it lies in a feature document and its ID is
/// feature-scoped or held by more than one node of `graph` (any file, as
/// the resolver counts holders, whatever the map admits); else its ID;
/// else its file's path. Two nodes share a name only when one ID is held
/// more than once outside feature documents, or twice in one.
pub fn node_names(graph: &SpecGraph<'_>, map: &CorpusMap, naming: &MapNaming<'_>) -> Vec<String> {
    let mut held: BTreeMap<&str, usize> = BTreeMap::new();
    for file in 0..graph.paths().len() {
        for node in graph.nodes(file) {
            if let Some(id) = node.id.as_deref() {
                *held.entry(id).or_insert(0) += 1;
            }
        }
    }
    map.nodes
        .iter()
        .map(|node| {
            let path = graph.paths()[node.at.file];
            let Some(id) = graph.node(node.at).and_then(|found| found.id.as_deref()) else {
                return path.to_owned();
            };
            let shared = held.get(id).is_some_and(|&holders| holders > 1);
            match feature_stem(naming.features, path) {
                Some(slug) if shared || feature_scoped(naming.scheme, id) => {
                    format!("{slug}/{id}")
                }
                _ => id.to_owned(),
            }
        })
        .collect()
}

/// One node of the map (task spec `corpus-map` CM1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapNode {
    pub at: NodeAt,
    /// A section's document, as an index into [`CorpusMap::nodes`]; `None`
    /// for a document.
    pub of: Option<usize>,
    /// Its uses and orphan flag, [`usage_of`]'s.
    pub usage: NodeUsage,
}

/// The resolved links of one (source, target, type), merged (task spec
/// `corpus-map` CM1). Ends index [`CorpusMap::nodes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEdge {
    pub src: usize,
    pub dst: usize,
    pub link_type: String,
    /// Its type is the weak one.
    pub weak: bool,
    /// The written places merged, at least 1.
    pub places: usize,
}

/// A document under its resolved `parent:` (task spec `corpus-map` CM1).
/// Both index [`CorpusMap::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapParent {
    pub child: usize,
    pub parent: usize,
}

/// An unresolved target, one per written text (task spec `corpus-map`
/// CM2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapGhost {
    /// The reference or path as written.
    pub written: String,
    /// Why its first place dangles.
    pub reason: String,
    /// The nodes holding its places, as indexes into [`CorpusMap::nodes`],
    /// ascending, each once.
    pub sources: Vec<usize>,
    /// Its written places.
    pub places: usize,
}

/// The map of one graph (task spec `corpus-map` CM1, CM2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorpusMap {
    /// In (path, position) order.
    pub nodes: Vec<MapNode>,
    /// By (source, target, type).
    pub edges: Vec<MapEdge>,
    /// By child.
    pub parents: Vec<MapParent>,
    /// By written text.
    pub ghosts: Vec<MapGhost>,
}

/// One ghost while it gathers its places.
struct Gathered<'g> {
    reason: &'g str,
    sources: BTreeSet<usize>,
    places: usize,
}

/// The map of `graph`'s admitted files: the live ones, and the Tier 3 ones
/// when `archive` (task spec `corpus-map` CM1, CM2).
pub fn corpus_map(graph: &SpecGraph<'_>, archive: bool) -> CorpusMap {
    let admitted = |file: usize| match graph.standing(file) {
        Standing::Live => true,
        Standing::Tier3 => archive,
        Standing::Generated => false,
    };
    let usage = usage_of(graph);
    let mut index: BTreeMap<NodeAt, usize> = BTreeMap::new();
    let mut nodes = Vec::new();
    for file in (0..graph.paths().len()).filter(|&file| admitted(file)) {
        let document = nodes.len();
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            index.insert(at, nodes.len());
            nodes.push(MapNode {
                at,
                of: (ord > 0).then_some(document),
                usage: usage.get(&at).copied().unwrap_or_default(),
            });
        }
    }

    let mut merged: BTreeMap<(usize, usize, &str), usize> = BTreeMap::new();
    let mut ghosts: BTreeMap<&str, Gathered<'_>> = BTreeMap::new();
    for edge in graph.edges() {
        if !admitted(edge.file) {
            continue;
        }
        match edge.state() {
            LinkState::Resolved => {
                let ends = (
                    first_admitted(&index, holders(&edge.source)),
                    first_admitted(&index, holders(&edge.target)),
                );
                if let (Some(src), Some(dst)) = ends
                    && src != dst
                {
                    *merged
                        .entry((src, dst, edge.link_type.as_str()))
                        .or_insert(0) += 1;
                }
            }
            LinkState::Dangling => {
                let Some(&source) = index.get(&edge.holder) else {
                    continue;
                };
                let ghost = ghosts
                    .entry(written_target(&edge.written))
                    .or_insert_with(|| Gathered {
                        reason: edge.reason().unwrap_or_default(),
                        sources: BTreeSet::new(),
                        places: 0,
                    });
                ghost.sources.insert(source);
                ghost.places += 1;
            }
            LinkState::Skipped | LinkState::Unchecked => {}
        }
    }
    let edges = merged
        .into_iter()
        .map(|((src, dst, link_type), places)| MapEdge {
            src,
            dst,
            link_type: link_type.to_owned(),
            weak: is_weak_link(link_type),
            places,
        })
        .collect();
    let ghosts = ghosts
        .into_iter()
        .map(|(written, ghost)| MapGhost {
            written: written.to_owned(),
            reason: ghost.reason.to_owned(),
            sources: ghost.sources.into_iter().collect(),
            places: ghost.places,
        })
        .collect();

    let parents = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.at.ord == 0)
        .filter_map(|(child, node)| match graph.parent(node.at)? {
            Parent::Node {
                node: holder,
                others,
            } => first_admitted(&index, std::iter::once(&holder).chain(&others))
                .map(|parent| MapParent { child, parent }),
            _ => None,
        })
        .collect();

    CorpusMap {
        nodes,
        edges,
        parents,
        ghosts,
    }
}

/// An end's holders; none unless resolved.
fn holders(end: &Endpoint) -> &[NodeAt] {
    match end {
        Endpoint::Nodes(nodes) => nodes,
        _ => &[],
    }
}

/// The first admitted node of `holders` by (path, position), as its index
/// into the map's nodes (served in that order); none admitted, none.
fn first_admitted<'a>(
    index: &BTreeMap<NodeAt, usize>,
    holders: impl IntoIterator<Item = &'a NodeAt>,
) -> Option<usize> {
    holders
        .into_iter()
        .filter_map(|at| index.get(at).copied())
        .min()
}

/// The target a link's written form names, as written: a wiki form's
/// brackets and `|label` dropped (the model's `RefForm::Wiki`), so one
/// target written as a front-matter reference and as a wiki mention is
/// one ghost (task spec `corpus-map` CM2).
fn written_target(written: &str) -> &str {
    match written
        .strip_prefix("[[")
        .and_then(|inner| inner.strip_suffix("]]"))
    {
        Some(inner) => inner.split_once('|').map_or(inner, |(target, _)| target),
        None => written,
    }
}
