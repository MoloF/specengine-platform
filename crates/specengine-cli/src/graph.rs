//! `spec graph REF [--impact] [--type T]… [--depth N] [--archive]` (task
//! spec `spec-cli-graph`): a breadth-first walk along typed links from
//! `REF`'s holders (distance 0), each node visited once, over the
//! index-fed spec graph. A node's links include its nested sections'.
//!
//! The types followed (one table for every project, `specengine-model`):
//! by default every strong type outgoing (`mentions` not followed); with
//! `--impact` what an edit reaches: `depends_on`, `derived_from`,
//! `verifies`, `uses_term` incoming, `constrains` outgoing. `--type T`
//! replaces the set: outgoing; with `--impact` in its table direction,
//! else incoming. `--depth N` stops at distance N; default unbounded.
//! Edges written in a file the live rule leaves out are not followed,
//! counted. A dangling, `project:` or unchecked edge of a followed type is
//! listed, never followed.
//!
//! ```text
//! <distance> <name> | <kind or -> | <title or -> | <path>:<line>[ | archived]
//! <src> --<type>--> <dst> | <path>:<line>[ | dangling: <reason> | skipped: another project | unchecked]
//! nodes <n>, edges <e>[; left out: <g> generated, <t> archived (--archive)]
//! [truncated: <k> of <n> nodes and <j> of <m> edges not shown; lower --depth or add --type]
//! ```
//!
//! Nodes by (distance, path, position); edges by (type, path, line,
//! column), each in link direction, an unresolved end as written. The CLI
//! prints the lines within [`crate::OUTPUT_CAP_CHARS`] ([`View::Capped`]:
//! nodes first, a node cut drops every edge); the daemon's
//! [`View::Browser`] shows every node and edge (task spec `ui-live`).

use std::collections::BTreeSet;

use serde::Serialize;
use specengine_core::check::{Endpoint, LinkState, NodeAt, SpecGraph};
use specengine_model::link::is_link_type;
use specengine_model::{
    Direction, IMPACT_LINK_TYPES, LINK_TYPES, graph_direction, impact_direction, is_weak_link,
};

use crate::cap::{View, lines_within};
use crate::corpus::{Admission, LeftOut, depth_of, holders_warning, indexed, locate};
use crate::links::state_suffix;
use crate::project::discover;
use crate::search::notes;
use crate::show::classify;
use crate::{CliError, Env, Globals, Message, one_line};

/// `spec graph` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphRequest {
    /// `REF` as given.
    pub reference: String,
    /// `--impact`: the impact table instead of the outgoing default.
    pub impact: bool,
    /// `--type`: the types to follow instead; empty: the table.
    pub types: Vec<String>,
    /// `--depth N`; `None`: unbounded.
    pub depth: Option<i64>,
    /// `--archive`: edges written in Tier 3 files too.
    pub archive: bool,
}

/// What `spec graph` reached, or why `REF` names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphOutcome {
    /// `REF` as given.
    pub reference: String,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    pub impact: bool,
    /// The (type, direction) pairs followed, in table or given order; the
    /// default lists the shared types, then the unknown declared types of
    /// the corpus, by name.
    pub types: Vec<FollowedType>,
    pub depth: Option<usize>,
    pub archive: bool,
    /// Edges of a followed type written in left-out files.
    pub left_out: LeftOut,
    /// By (distance, path, position).
    pub nodes: Vec<GraphNode>,
    /// By (type, path, line, column).
    pub edges: Vec<GraphEdge>,
    /// How many of `nodes`, then of `edges`, are printed (text and JSON
    /// alike).
    pub shown_nodes: usize,
    pub shown_edges: usize,
}

impl GraphOutcome {
    pub fn truncated(&self) -> bool {
        self.shown_nodes < self.nodes.len() || self.shown_edges < self.edges.len()
    }
}

/// A link type and the way it is followed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FollowedType {
    #[serde(rename = "type")]
    pub link_type: String,
    pub direction: Direction,
}

/// A node reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub distance: usize,
    /// Its file is Tier 3.
    pub archived: bool,
}

/// An edge met, in link direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    /// The source's name (ID else path); `None` when unresolved.
    pub src: Option<String>,
    pub link_type: String,
    /// The target's name; `None` when unresolved.
    pub dst: Option<String>,
    /// The reference or path as written.
    pub written: String,
    /// Where it is written.
    pub path: String,
    pub line: usize,
    pub state: LinkState,
    pub reason: Option<String>,
}

/// `spec graph`: updates the index, then walks from `REF`.
pub fn graph(
    env: &Env,
    globals: &Globals,
    request: &GraphRequest,
) -> Result<GraphOutcome, CliError> {
    graph_with_view(env, globals, request, View::Capped)
}

/// [`graph`], its answer bounded by `view` (the daemon's
/// [`View::Browser`]: every node and edge shown, `truncated` false).
pub fn graph_with_view(
    env: &Env,
    globals: &Globals,
    request: &GraphRequest,
    view: View,
) -> Result<GraphOutcome, CliError> {
    let depth = depth_of(request.depth)?;
    let project = discover(env, globals)?;
    project.slug()?;
    let scheme = &project.config.scheme;
    let written = request.reference.trim();
    let mut messages = Vec::new();
    let mut outcome = GraphOutcome {
        reference: request.reference.clone(),
        reason: None,
        messages: Vec::new(),
        impact: request.impact,
        types: Vec::new(),
        depth,
        archive: request.archive,
        left_out: LeftOut::default(),
        nodes: Vec::new(),
        edges: Vec::new(),
        shown_nodes: 0,
        shown_edges: 0,
    };
    let target = match classify(
        written,
        scheme,
        &mut messages,
        "`spec graph` reads the current files",
    )? {
        Ok(target) => target,
        Err(reason) => return Ok(not_found(outcome, reason, messages)),
    };

    let input = indexed(env, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&input, scheme, &project.config.paths);
    let starts = match locate(&graph, &target, written)? {
        Ok(nodes) => nodes,
        Err(reason) => return Ok(not_found(outcome, reason, messages)),
    };
    messages.extend(holders_warning(
        &graph,
        written,
        &starts,
        "all at distance 0",
    ));
    let admission = Admission::new(&graph, request.archive, starts.iter().map(|at| at.file));
    outcome.types = followed(&graph, request);
    let walk = graph.walk(
        &starts,
        |link_type| follow(request, link_type),
        depth,
        |file| admission.admits(file),
    );
    for &edge in &walk.left_out {
        outcome
            .left_out
            .add(graph.standing(graph.edges()[edge].file));
    }
    outcome.nodes = walk
        .nodes
        .iter()
        .map(|&(at, distance)| node_of(&graph, at, distance))
        .collect();
    let mut edges = walk.edges.clone();
    edges.sort_by_key(|&index| {
        let edge = &graph.edges()[index];
        (
            edge.link_type.clone(),
            edge.file,
            edge.line,
            edge.offset,
            index,
        )
    });
    outcome.edges = edges
        .into_iter()
        .map(|index| edge_of(&graph, index))
        .collect();
    outcome.messages = messages;
    (outcome.shown_nodes, outcome.shown_edges) = match view {
        View::Capped => capped(&outcome),
        View::Browser => (outcome.nodes.len(), outcome.edges.len()),
    };
    Ok(outcome)
}

/// How many nodes, then edges, fit within the cap: the nodes first; a node
/// cut drops every edge.
fn capped(outcome: &GraphOutcome) -> (usize, usize) {
    let summary = summary(outcome).chars().count() + 1;
    let (shown_nodes, used) = lines_within(
        outcome
            .nodes
            .iter()
            .map(|node| node_line(node).chars().count() + 1),
        summary,
        true,
    );
    let shown_edges = if shown_nodes < outcome.nodes.len() {
        0
    } else {
        lines_within(
            outcome
                .edges
                .iter()
                .map(|edge| edge_line(edge).chars().count() + 1),
            used,
            false,
        )
        .0
    };
    (shown_nodes, shown_edges)
}

fn not_found(mut outcome: GraphOutcome, reason: String, messages: Vec<Message>) -> GraphOutcome {
    outcome.reason = Some(one_line(&reason));
    outcome.messages = messages;
    outcome
}

/// The direction `link_type` is followed in, if at all.
fn follow(request: &GraphRequest, link_type: &str) -> Option<Direction> {
    if request.types.is_empty() {
        return if request.impact {
            impact_direction(link_type)
        } else {
            graph_direction(link_type)
        };
    }
    if !request.types.iter().any(|wanted| wanted == link_type) {
        return None;
    }
    Some(if request.impact {
        impact_direction(link_type).unwrap_or(Direction::In)
    } else {
        Direction::Out
    })
}

/// The pairs followed: `--type` in the order given (repeats once), the
/// impact table, or the shared types and the corpus's unknown declared
/// ones, outgoing.
fn followed(graph: &SpecGraph<'_>, request: &GraphRequest) -> Vec<FollowedType> {
    let pair = |link_type: &str, direction| FollowedType {
        link_type: link_type.to_owned(),
        direction,
    };
    if !request.types.is_empty() {
        let mut seen = BTreeSet::new();
        return request
            .types
            .iter()
            .filter(|link_type| seen.insert(link_type.as_str()))
            .filter_map(|link_type| {
                follow(request, link_type).map(|direction| pair(link_type, direction))
            })
            .collect();
    }
    if request.impact {
        return IMPACT_LINK_TYPES
            .iter()
            .map(|&(link_type, direction)| pair(link_type, direction))
            .collect();
    }
    let unknown: BTreeSet<&str> = graph
        .edges()
        .iter()
        .map(|edge| edge.link_type.as_str())
        .filter(|link_type| !is_link_type(link_type) && !is_weak_link(link_type))
        .collect();
    LINK_TYPES
        .iter()
        .copied()
        .chain(unknown)
        .map(|link_type| pair(link_type, Direction::Out))
        .collect()
}

fn node_of(graph: &SpecGraph<'_>, at: NodeAt, distance: usize) -> GraphNode {
    let node = graph.node(at);
    GraphNode {
        id: node.and_then(|node| node.id.clone()),
        kind: node.and_then(|node| node.kind.clone()),
        title: node.and_then(|node| node.title.clone()),
        path: graph.paths()[at.file].to_owned(),
        line: graph.line(at),
        distance,
        archived: graph.is_tier3(at.file),
    }
}

fn edge_of(graph: &SpecGraph<'_>, index: usize) -> GraphEdge {
    let edge = &graph.edges()[index];
    let name = |end: &Endpoint| match end {
        Endpoint::Nodes(nodes) => nodes.first().map(|&node| graph.name(node)),
        _ => None,
    };
    GraphEdge {
        src: name(&edge.source),
        link_type: edge.link_type.clone(),
        dst: name(&edge.target),
        written: edge.written.clone(),
        path: graph.paths()[edge.file].to_owned(),
        line: edge.line,
        state: edge.state(),
        reason: edge.reason().map(str::to_owned),
    }
}

fn node_line(node: &GraphNode) -> String {
    let name = node.id.as_deref().unwrap_or(&node.path);
    let mut line = format!(
        "{} {} | {} | {} | {}:{}",
        node.distance,
        one_line(name),
        node.kind
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
        node.title
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
        one_line(&node.path),
        node.line
    );
    if node.archived {
        line.push_str(" | archived");
    }
    line
}

fn edge_line(edge: &GraphEdge) -> String {
    let end = |name: &Option<String>| one_line(name.as_deref().unwrap_or(&edge.written));
    format!(
        "{} --{}--> {} | {}:{}{}",
        end(&edge.src),
        one_line(&edge.link_type),
        end(&edge.dst),
        one_line(&edge.path),
        edge.line,
        state_suffix(edge.state, edge.reason.as_deref())
    )
}

fn summary(outcome: &GraphOutcome) -> String {
    format!(
        "nodes {}, edges {}{}",
        outcome.nodes.len(),
        outcome.edges.len(),
        outcome.left_out.suffix()
    )
}

pub(crate) fn render_text(outcome: &GraphOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let mut out = String::new();
    for node in &outcome.nodes[..outcome.shown_nodes.min(outcome.nodes.len())] {
        out.push_str(&node_line(node));
        out.push('\n');
    }
    for edge in &outcome.edges[..outcome.shown_edges.min(outcome.edges.len())] {
        out.push_str(&edge_line(edge));
        out.push('\n');
    }
    out.push_str(&summary(outcome));
    out.push('\n');
    if outcome.truncated() {
        out.push_str(&format!(
            "[truncated: {} of {} nodes and {} of {} edges not shown; lower --depth or add --type]\n",
            outcome.nodes.len() - outcome.shown_nodes.min(outcome.nodes.len()),
            outcome.nodes.len(),
            outcome.edges.len() - outcome.shown_edges.min(outcome.edges.len()),
            outcome.edges.len()
        ));
    }
    out
}

#[derive(Serialize)]
struct GraphJson<'a> {
    #[serde(rename = "ref")]
    reference: &'a str,
    reason: Option<&'a str>,
    impact: bool,
    types: &'a [FollowedType],
    depth: Option<usize>,
    archive: bool,
    notes: Vec<String>,
    left_out: LeftOut,
    truncated: bool,
    nodes: Vec<NodeJson<'a>>,
    edges: Vec<EdgeJson<'a>>,
}

#[derive(Serialize)]
struct NodeJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    distance: usize,
    archived: bool,
}

#[derive(Serialize)]
struct EdgeJson<'a> {
    src: Option<&'a str>,
    #[serde(rename = "type")]
    link_type: &'a str,
    dst: Option<&'a str>,
    written: &'a str,
    path: &'a str,
    line: usize,
    state: &'static str,
    reason: Option<&'a str>,
}

fn view(outcome: &GraphOutcome) -> GraphJson<'_> {
    GraphJson {
        reference: &outcome.reference,
        reason: outcome.reason.as_deref(),
        impact: outcome.impact,
        types: &outcome.types,
        depth: outcome.depth,
        archive: outcome.archive,
        notes: notes(&outcome.messages),
        left_out: outcome.left_out,
        truncated: outcome.truncated(),
        nodes: outcome.nodes[..outcome.shown_nodes.min(outcome.nodes.len())]
            .iter()
            .map(|node| NodeJson {
                id: node.id.as_deref(),
                kind: node.kind.as_deref(),
                title: node.title.as_deref(),
                path: &node.path,
                line: node.line,
                distance: node.distance,
                archived: node.archived,
            })
            .collect(),
        edges: outcome.edges[..outcome.shown_edges.min(outcome.edges.len())]
            .iter()
            .map(|edge| EdgeJson {
                src: edge.src.as_deref(),
                link_type: &edge.link_type,
                dst: edge.dst.as_deref(),
                written: &edge.written,
                path: &edge.path,
                line: edge.line,
                state: edge.state.as_str(),
                reason: edge.reason.as_deref(),
            })
            .collect(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for GraphOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
