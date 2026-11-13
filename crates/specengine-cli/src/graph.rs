//! `spec graph (REF | --kind K…) [--both | --impact] [--type T]… [--depth
//! N] [--archive]` (task specs `spec-cli-graph`, `corpus-map`): a
//! breadth-first walk along typed links from `REF`'s holders (distance 0),
//! each node visited once, over the index-fed spec graph. A node's links
//! include its nested sections'.
//!
//! The types followed (one table for every project, `specengine-model`):
//! by default every strong type outgoing (`mentions` not followed); with
//! `--impact` what an edit reaches: `depends_on`, `derived_from`,
//! `verifies`, `uses_term`, `uses` incoming, `constrains`, `supplies`
//! outgoing. `--type T` replaces the set: outgoing; with `--impact` in its
//! table direction, else incoming (so `--impact --type follows` walks the
//! flow forward). `--both` takes each type of the set both ways, its `out`
//! listed before its `in` (task spec `corpus-map` CM4). `--depth N` stops
//! at distance N; default unbounded. Edges written in a file the live rule
//! leaves out are not followed, counted. A dangling, `project:` or
//! unchecked edge of a followed type is listed, never followed.
//!
//! `--kind K` (repeatable, matched whole as `spec tree --kind`) starts from
//! every node of those kinds in a file the live rule admits, in (path,
//! position) order, instead of a REF; a kind no node has is a note, never
//! an exit (CM5). A REF and `--kind` together, neither, or `--both` with
//! `--impact` exit 2 before anything is read ([`GraphRequest::refusal`],
//! the daemon's 400).
//!
//! ```text
//! <distance> <name> | <kind or -> | <title or -> | <path>:<line>[ | archived]
//! <src> --<type>--> <dst> | <path>:<line>[ | kind <kind>][ | when <when>][ | dangling: <reason> | skipped: another project | unchecked]
//! nodes <n>, edges <e>[; left out: <g> generated, <t> archived (--archive)]
//! [truncated: <k> of <n> nodes and <j> of <m> edges not shown; lower --depth or add --type]
//! ```
//!
//! Nodes by (distance, path, position); edges by (type, path, line,
//! column), each in link direction, an unresolved end as written. The CLI
//! prints the lines within [`crate::OUTPUT_CAP_CHARS`] ([`View::Capped`]:
//! nodes first, a node cut drops every edge); the daemon's
//! [`View::Browser`] shows every node and edge (task spec `ui-live`).

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;
use specengine_core::check::{Endpoint, LinkState, NodeAt, SpecGraph, Walk};
use specengine_model::link::is_link_type;
use specengine_model::{
    Direction, IMPACT_LINK_TYPES, LINK_TYPES, graph_direction, impact_direction, is_weak_link,
};

use crate::cap::{View, lines_within};
use crate::corpus::{Admission, LeftOut, depth_of, holders_warning, indexed, locate};
use crate::links::{label_strings, label_suffix, state_suffix};
use crate::project::discover;
use crate::search::notes;
use crate::show::classify;
use crate::{CliError, Env, Globals, Message, one_line};

/// The refusal of a REF given with `--kind` (task spec `corpus-map` CM5).
const REF_AND_KIND: &str = "spec graph: give a REF or `--kind`, not both";

/// The refusal of a call with neither a REF nor `--kind` (CM5).
const NO_START: &str = "spec graph: give a REF or `--kind`";

/// The refusal of `--both` with `--impact` (CM4).
const BOTH_AND_IMPACT: &str = "spec graph: `--both` and `--impact` exclude each other";

/// `spec graph` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphRequest {
    /// `REF` as given; `None` with `--kind`.
    pub reference: Option<String>,
    /// `--impact`: the impact table instead of the outgoing default.
    pub impact: bool,
    /// `--both`: each followed type both ways (task spec `corpus-map` CM4).
    pub both: bool,
    /// `--type`: the types to follow instead; empty: the table.
    pub types: Vec<String>,
    /// `--kind`: start from every admitted node of these kinds instead of
    /// a REF (CM5); empty: a REF.
    pub kinds: Vec<String>,
    /// `--depth N`; `None`: unbounded.
    pub depth: Option<i64>,
    /// `--archive`: edges written in Tier 3 files too; with `--kind`, Tier
    /// 3 starts too.
    pub archive: bool,
}

impl GraphRequest {
    /// Why the request cannot run (exit 2; the daemon's 400 with the same
    /// line), if it cannot: a REF and `--kind` together, neither, or
    /// `--both` with `--impact` (task spec `corpus-map` CM4, CM5).
    pub fn refusal(&self) -> Option<&'static str> {
        match (self.reference.is_some(), !self.kinds.is_empty()) {
            (true, true) => Some(REF_AND_KIND),
            (false, false) => Some(NO_START),
            _ if self.both && self.impact => Some(BOTH_AND_IMPACT),
            _ => None,
        }
    }
}

/// What `spec graph` reached, or why `REF` names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphOutcome {
    /// `REF` as given; `None` with `--kind`.
    pub reference: Option<String>,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    pub impact: bool,
    /// `--both`.
    pub both: bool,
    /// `--kind` as given; empty: a REF.
    pub kinds: Vec<String>,
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
    /// A labelled edge's kind.
    pub kind: Option<String>,
    /// A labelled edge's condition.
    pub when: Option<String>,
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
    if let Some(refusal) = request.refusal() {
        return Err(CliError::cannot(refusal));
    }
    let depth = depth_of(request.depth)?;
    let project = discover(env, globals)?;
    project.slug()?;
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = GraphOutcome {
        reference: request.reference.clone(),
        reason: None,
        messages: Vec::new(),
        impact: request.impact,
        both: request.both,
        kinds: request.kinds.clone(),
        types: Vec::new(),
        depth,
        archive: request.archive,
        left_out: LeftOut::default(),
        nodes: Vec::new(),
        edges: Vec::new(),
        shown_nodes: 0,
        shown_edges: 0,
    };
    let target = match request.reference.as_deref().map(str::trim) {
        None => None,
        Some(written) => match classify(
            written,
            scheme,
            &mut messages,
            "`spec graph` reads the current files",
        )? {
            Ok(target) => Some((target, written)),
            Err(reason) => return Ok(not_found(outcome, reason, messages)),
        },
    };

    let input = indexed(env, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&input, scheme, &project.config.paths);
    let starts = match target {
        Some((target, written)) => {
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
            starts
        }
        None => of_kinds(&graph, &request.kinds, request.archive, &mut messages),
    };
    outcome.types = followed(&graph, request);
    let Reach { walk, left_out } = reach(&graph, &starts, request, depth);
    outcome.left_out = left_out;
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

/// What one walk from the starts reached, and the edges of a followed type
/// it met in files the live rule leaves out, counted.
pub(crate) struct Reach {
    pub(crate) walk: Walk,
    pub(crate) left_out: LeftOut,
}

/// The one walk of `spec graph` and `spec impact`: breadth-first from
/// `starts` (distance 0) along the types `request` follows, at most
/// `depth`, over the files the live rule admits (the starts' always), so
/// `spec impact`'s nodes are `spec graph --impact`'s by construction.
pub(crate) fn reach(
    graph: &SpecGraph<'_>,
    starts: &[NodeAt],
    request: &GraphRequest,
    depth: Option<usize>,
) -> Reach {
    let admission = Admission::new(graph, request.archive, starts.iter().map(|at| at.file));
    let walk = if request.both {
        walk_both(
            graph,
            starts,
            |link_type| follows_both(request, link_type),
            depth,
            |file| admission.admits(file),
        )
    } else {
        graph.walk(
            starts,
            |link_type| follow(request, link_type),
            depth,
            |file| admission.admits(file),
        )
    };
    let mut left_out = LeftOut::default();
    for &edge in &walk.left_out {
        left_out.add(graph.standing(graph.edges()[edge].file));
    }
    Reach { walk, left_out }
}

/// `spec graph --impact`'s walk from `starts` (the impact table, no
/// `--type`, Tier 3 files' links left out): `spec impact`'s.
pub(crate) fn impact_reach(
    graph: &SpecGraph<'_>,
    starts: &[NodeAt],
    depth: Option<usize>,
) -> Reach {
    let request = GraphRequest {
        impact: true,
        ..GraphRequest::default()
    };
    reach(graph, starts, &request, depth)
}

/// `--kind K…`'s starts (task spec `corpus-map` CM5): every node whose
/// kind is one of `kinds`, matched whole, in a file the live rule admits
/// (Tier 3 with `archive`), in (path, position) order; a note per kind no
/// such node has.
fn of_kinds(
    graph: &SpecGraph<'_>,
    kinds: &[String],
    archive: bool,
    messages: &mut Vec<Message>,
) -> Vec<NodeAt> {
    let admission = Admission::new(graph, archive, Vec::new());
    let mut starts = Vec::new();
    let mut found: BTreeSet<&str> = BTreeSet::new();
    for file in (0..graph.paths().len()).filter(|&file| admission.admits(file)) {
        for (ord, node) in graph.nodes(file).iter().enumerate() {
            if let Some(kind) = node.kind.as_deref()
                && kinds.iter().any(|wanted| wanted == kind)
            {
                found.insert(kind);
                starts.push(NodeAt { file, ord });
            }
        }
    }
    let mut noted = BTreeSet::new();
    for kind in kinds {
        if !found.contains(kind.as_str()) && noted.insert(kind.as_str()) {
            messages.push(Message::Note(format!("graph: no node of kind `{kind}`")));
        }
    }
    starts
}

/// [`SpecGraph::walk`] with each followed type taken both ways (task spec
/// `corpus-map` CM4): breadth-first from `starts` (distance 0), from each
/// reached node and its nested sections ([`SpecGraph::links`]) out to the
/// targets and in to the sources of the edges whose type `follows` takes;
/// a node is visited once, an edge to a visited node still listed. Nodes at
/// distance `depth` are not expanded. Edges written in a file `admit`
/// refuses are counted, never followed.
fn walk_both(
    graph: &SpecGraph<'_>,
    starts: &[NodeAt],
    follows: impl Fn(&str) -> bool,
    depth: Option<usize>,
    admit: impl Fn(usize) -> bool,
) -> Walk {
    let mut distance: BTreeMap<NodeAt, usize> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &start in starts {
        if graph.node(start).is_some() && !distance.contains_key(&start) {
            distance.insert(start, 0);
            queue.push_back(start);
        }
    }
    let mut edges = BTreeSet::new();
    let mut left_out = BTreeSet::new();
    while let Some(at) = queue.pop_front() {
        let Some(&reached) = distance.get(&at) else {
            continue;
        };
        if depth.is_some_and(|depth| reached >= depth) {
            continue;
        }
        let links = graph.links(at, &admit);
        let out = links.outgoing.iter().map(|&(index, _)| (index, true));
        let back = links.incoming.iter().map(|&(index, _)| (index, false));
        for (index, forward) in out.chain(back) {
            let edge = &graph.edges()[index];
            if !follows(&edge.link_type) {
                continue;
            }
            if !admit(edge.file) {
                left_out.insert(index);
                continue;
            }
            edges.insert(index);
            let far = if forward { &edge.target } else { &edge.source };
            let Endpoint::Nodes(nodes) = far else {
                continue;
            };
            for &next in nodes {
                if let Entry::Vacant(entry) = distance.entry(next) {
                    entry.insert(reached + 1);
                    queue.push_back(next);
                }
            }
        }
        left_out.extend(
            links
                .left_out
                .iter()
                .copied()
                .filter(|&index| follows(&graph.edges()[index].link_type)),
        );
    }
    let mut nodes: Vec<(NodeAt, usize)> = distance.into_iter().collect();
    nodes.sort_by_key(|&(at, distance)| (distance, at));
    Walk {
        nodes,
        edges: edges.into_iter().collect(),
        left_out: left_out.into_iter().collect(),
    }
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

/// `--both` follows `link_type` (task spec `corpus-map` CM4): one of
/// `--type`, else a strong type.
fn follows_both(request: &GraphRequest, link_type: &str) -> bool {
    if request.types.is_empty() {
        graph_direction(link_type).is_some()
    } else {
        request.types.iter().any(|wanted| wanted == link_type)
    }
}

/// The pairs followed: [`one_way`]'s; with `--both` each type's `out`, then
/// its `in` (task spec `corpus-map` CM4).
fn followed(graph: &SpecGraph<'_>, request: &GraphRequest) -> Vec<FollowedType> {
    let pairs = one_way(graph, request);
    if !request.both {
        return pairs;
    }
    pairs
        .into_iter()
        .flat_map(|pair| {
            [Direction::Out, Direction::In].map(|direction| FollowedType {
                link_type: pair.link_type.clone(),
                direction,
            })
        })
        .collect()
}

/// The pairs followed one way: `--type` in the order given (repeats once),
/// the impact table, or the shared types and the corpus's unknown declared
/// ones, outgoing.
fn one_way(graph: &SpecGraph<'_>, request: &GraphRequest) -> Vec<FollowedType> {
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

/// A node reached at `distance`, as `spec graph` and `spec impact` list it.
pub(crate) fn node_of(graph: &SpecGraph<'_>, at: NodeAt, distance: usize) -> GraphNode {
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
    let (kind, when) = label_strings(edge.edge.as_ref());
    GraphEdge {
        src: name(&edge.source),
        link_type: edge.link_type.clone(),
        dst: name(&edge.target),
        written: edge.written.clone(),
        path: graph.paths()[edge.file].to_owned(),
        line: edge.line,
        state: edge.state(),
        reason: edge.reason().map(str::to_owned),
        kind,
        when,
    }
}

/// `<distance> <name> | <kind or -> | <title or -> | <path>:<line>[ |
/// archived]` (`spec impact`'s after its `node `).
pub(crate) fn node_line(node: &GraphNode) -> String {
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
        "{} --{}--> {} | {}:{}{}{}",
        end(&edge.src),
        one_line(&edge.link_type),
        end(&edge.dst),
        one_line(&edge.path),
        edge.line,
        label_suffix(edge.kind.as_deref(), edge.when.as_deref()),
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

/// The document's 13 keys, in this order: `both` and `kinds` (task spec
/// `corpus-map` CM4, CM5) after `impact`.
#[derive(Serialize)]
struct GraphJson<'a> {
    #[serde(rename = "ref")]
    reference: Option<&'a str>,
    reason: Option<&'a str>,
    impact: bool,
    both: bool,
    /// `--kind` as given; `null` with a REF.
    kinds: Option<&'a [String]>,
    types: &'a [FollowedType],
    depth: Option<usize>,
    archive: bool,
    notes: Vec<String>,
    left_out: LeftOut,
    truncated: bool,
    nodes: Vec<NodeJson<'a>>,
    edges: Vec<EdgeJson<'a>>,
}

/// A node's JSON object, `spec graph`'s and `spec impact`'s.
#[derive(Serialize)]
pub(crate) struct NodeJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    distance: usize,
    archived: bool,
}

impl<'a> NodeJson<'a> {
    pub(crate) fn of(node: &'a GraphNode) -> Self {
        Self {
            id: node.id.as_deref(),
            kind: node.kind.as_deref(),
            title: node.title.as_deref(),
            path: &node.path,
            line: node.line,
            distance: node.distance,
            archived: node.archived,
        }
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    when: Option<&'a str>,
}

fn view(outcome: &GraphOutcome) -> GraphJson<'_> {
    GraphJson {
        reference: outcome.reference.as_deref(),
        reason: outcome.reason.as_deref(),
        impact: outcome.impact,
        both: outcome.both,
        kinds: (!outcome.kinds.is_empty()).then_some(outcome.kinds.as_slice()),
        types: &outcome.types,
        depth: outcome.depth,
        archive: outcome.archive,
        notes: notes(&outcome.messages),
        left_out: outcome.left_out,
        truncated: outcome.truncated(),
        nodes: outcome.nodes[..outcome.shown_nodes.min(outcome.nodes.len())]
            .iter()
            .map(NodeJson::of)
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
                kind: edge.kind.as_deref(),
                when: edge.when.as_deref(),
            })
            .collect(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`, but an edge's `kind` and `when`, only on a labelled edge, so the
/// document of a corpus without labels keeps its keys.
impl Serialize for GraphOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
