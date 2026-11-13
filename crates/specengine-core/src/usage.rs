//! What uses a node and which documents nothing links, the pure half (task
//! spec `usage-orphans` UO1 to UO3): two counts and one flag over the
//! resolved spec graph, computed per read and never stored (UO7).
//!
//! - A **use** of `X` is a resolved link that lands on `X` or a node nested
//!   in it (its incoming rows, [`SpecGraph::links`]), written in a live file
//!   other than `X`'s ([`Standing::Live`]: neither Tier 3 nor generated),
//!   whatever a reader admits. Its source is the node the row names (the
//!   innermost ID section around the link, else the document, else the
//!   file's path). `used_by` counts distinct sources over the strong types,
//!   `mentioned_by` over the weak one ([`is_weak_link`]); a source may count
//!   in both. `parent:` is containment, never a use.
//! - An **orphan** is a live document linked to nothing: its `status:` is
//!   not the convention's word for set aside, both counts are 0, no resolved
//!   link written in its file ends on a node of another file (whatever that
//!   file's standing), no live file's resolved link names it as its source
//!   (`status: superseded-by` it), and no live document of another file has
//!   a resolved `parent:` ending on it or one of its sections. Its own
//!   `parent:` is not a link. A section, and a document of a file that is
//!   not live, is not judged (`None`).
//!
//! The core reads no kind, link type or class name: only the model's weak
//! type and the word above. One graph gives one answer whatever the order
//! its files came in: nodes and edges are in (path, position) order and
//! sources are counted as sets.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::is_weak_link;

use crate::check::{Endpoint, LinkState, NodeAt, Parent, SpecGraph, Standing};

/// The convention's `status:` for a document set aside: never an orphan
/// (task spec `usage-orphans` UO3).
const SET_ASIDE: &str = "abandoned";

/// One node's uses, per read, never stored (task spec `usage-orphans` UO1,
/// UO3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodeUsage {
    /// Distinct live sources in other files citing it or a nested section
    /// by a strong type.
    pub used_by: u32,
    /// The same, by the weak type.
    pub mentioned_by: u32,
    /// A live document linked to nothing; `None` for a section and for a
    /// document whose file is not live.
    pub orphan: Option<bool>,
}

/// One incoming row as [`tally`] reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseRow<'a> {
    /// The node it is counted by: its ID, else its file's path.
    pub source: &'a str,
    /// The file it is written in.
    pub file: &'a str,
    /// Its type is the weak one.
    pub weak: bool,
    /// Its file is live.
    pub live: bool,
    /// Its written end resolved.
    pub resolved: bool,
}

/// The two counts from one node's incoming rows: `(used_by, mentioned_by)`,
/// distinct sources over the resolved rows written in a live file other
/// than `own_file` (task spec `usage-orphans` UO1, UO2).
pub fn tally<'a>(rows: impl IntoIterator<Item = UseRow<'a>>, own_file: &str) -> (u32, u32) {
    let mut used: BTreeSet<&str> = BTreeSet::new();
    let mut mentioned: BTreeSet<&str> = BTreeSet::new();
    for row in rows {
        if !row.resolved || !row.live || row.file == own_file {
            continue;
        }
        if row.weak {
            mentioned.insert(row.source);
        } else {
            used.insert(row.source);
        }
    }
    (count(used.len()), count(mentioned.len()))
}

/// Every node of every file of the graph, keyed by place: its counts as
/// [`tally`] gives them over its incoming rows, and for a live document
/// whether it is an orphan (task spec `usage-orphans` UO1 to UO3). A node
/// of a file that is not live keeps its counts and is not judged.
pub fn usage_of(graph: &SpecGraph<'_>) -> BTreeMap<NodeAt, NodeUsage> {
    let linked = Linked::of(graph);
    let mut usage = BTreeMap::new();
    for (file, own_file) in graph.paths().iter().enumerate() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            let found = graph.links(at, |_| true);
            let (used_by, mentioned_by) = tally(
                found
                    .incoming
                    .iter()
                    .map(|&(index, _)| use_row(graph, index)),
                own_file,
            );
            let orphan = (ord == 0 && graph.standing(file) == Standing::Live).then(|| {
                used_by == 0
                    && mentioned_by == 0
                    && !set_aside(graph, at)
                    && !linked.files.contains(&file)
            });
            usage.insert(
                at,
                NodeUsage {
                    used_by,
                    mentioned_by,
                    orphan,
                },
            );
        }
    }
    usage
}

/// An edge as an incoming row: its source the first node of its source end
/// (the holder, or what `status: superseded-by` names), else its file's
/// path.
fn use_row<'g>(graph: &'g SpecGraph<'_>, index: usize) -> UseRow<'g> {
    let edge = &graph.edges()[index];
    let file = graph.paths()[edge.file];
    let source = match &edge.source {
        Endpoint::Nodes(nodes) => nodes.first().map_or(file, |&node| name_of(graph, node)),
        _ => file,
    };
    UseRow {
        source,
        file,
        weak: is_weak_link(&edge.link_type),
        live: graph.standing(edge.file) == Standing::Live,
        resolved: edge.state() == LinkState::Resolved,
    }
}

/// A node's ID, else its file's path: [`SpecGraph::name`], borrowed.
fn name_of<'g>(graph: &'g SpecGraph<'_>, at: NodeAt) -> &'g str {
    graph
        .node(at)
        .and_then(|node| node.id.as_deref())
        .unwrap_or(graph.paths()[at.file])
}

/// The document's `status:` is the convention's set-aside word.
fn set_aside(graph: &SpecGraph<'_>, at: NodeAt) -> bool {
    graph
        .node(at)
        .and_then(|node| node.fields.as_ref())
        .and_then(|fields| fields.status.as_deref())
        == Some(SET_ASIDE)
}

/// The files linked to another file other than by a use: a resolved link
/// written in them ends on a node of another file; a resolved link written
/// in a live other file names a node of theirs as its source; a live
/// document of another file has a resolved `parent:` ending on one of
/// their nodes.
struct Linked {
    files: BTreeSet<usize>,
}

impl Linked {
    fn of(graph: &SpecGraph<'_>) -> Self {
        let live = |file: usize| graph.standing(file) == Standing::Live;
        let mut files = BTreeSet::new();
        for edge in graph.edges() {
            if edge.state() != LinkState::Resolved {
                continue;
            }
            let ends = nodes_of(&edge.source).iter().chain(nodes_of(&edge.target));
            if ends.clone().any(|node| node.file != edge.file) {
                files.insert(edge.file);
            }
            if edge.names_source && live(edge.file) {
                files.extend(
                    nodes_of(&edge.source)
                        .iter()
                        .map(|node| node.file)
                        .filter(|&file| file != edge.file),
                );
            }
        }
        for child in graph.documents() {
            if !live(child.file) {
                continue;
            }
            if let Some(Parent::Node { node, others }) = graph.parent(child) {
                files.extend(
                    std::iter::once(node)
                        .chain(others)
                        .map(|holder| holder.file)
                        .filter(|&file| file != child.file),
                );
            }
        }
        // A cycle's breaker lost its resolved `parent:` to the cycle: it
        // ends on the one member no other member is listed under.
        for members in graph.cycles() {
            let Some(&breaker) = members.first() else {
                continue;
            };
            if !live(breaker.file) {
                continue;
            }
            let listed: BTreeSet<NodeAt> = members
                .iter()
                .filter_map(|&member| graph.listed_under(member))
                .collect();
            files.extend(
                members
                    .iter()
                    .filter(|member| !listed.contains(member))
                    .map(|member| member.file)
                    .filter(|&file| file != breaker.file),
            );
        }
        Self { files }
    }
}

/// An end's nodes; none unless resolved.
fn nodes_of(end: &Endpoint) -> &[NodeAt] {
    match end {
        Endpoint::Nodes(nodes) => nodes,
        _ => &[],
    }
}

/// A set's size as a count.
fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
