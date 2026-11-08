//! `spec flow [ROOT] [--depth N] [--stage S] [--trace]` (task specs
//! `flow-read`, `trace-views`, `flow-scope`): what a flow diagram draws,
//! over the index-fed spec graph and `[flow]`, read here alone. Core's
//! [`flow_read`] finds the scope, the typed edges, the walk and the ten gap
//! rows, and cuts the shown nodes to `--stage`'s after the fold; this file
//! resolves `ROOT`, maps `-` to the steps with no stage, bounds and prints.
//! `--trace` joins `spec trace`'s `rollup` and `counts` to each shown node
//! here, over every node the read without `--stage` shows, so a stage moves
//! no note: core's read stays pure and reads no queue. A stage echoes
//! nothing: the header and the JSON keys are the read's, counting what is
//! kept.
//!
//! ```text
//! flow <ROOT or all> | nodes <n>, edges <e>, depth <N or ->[; left out: …]
//! gap <code> | <state> | <count>[ | <item>, …][ | <cause>]
//! <distance or -> <name> | <kind or -> | <title or -> | <path>:<line>[ | start][ | end][ | group <name>][ | stage <s>][ | holds <h>, inner <i>][ | gaps <g>][ | rollup <state>]
//! <from> -> <to> | <type> | x<count>[ | kind <kind>][ | when <when>] | <path>:<line>
//! [truncated: <k> of <n> nodes and <j> of <m> edges not shown; give a ROOT or lower --depth]
//! ```
//!
//! All ten gap rows come first, so a cut never hides one: a row prints at
//! most [`GAP_ITEMS_SHOWN`] items, then `, … and <k> more`, `count` the
//! whole; a row not measured prints `-` for its count, never 0. Nodes and
//! edges then fit within [`crate::OUTPUT_CAP_CHARS`] ([`View::Capped`]:
//! nodes first, a node cut drops every edge), JSON holding the printed
//! items; the daemon's [`View::Browser`] shows everything.
//!
//! Exit 0 answered, zero nodes and any gap included; 1 a `ROOT` naming
//! nothing; 2 usage, a bad `--depth`, a look-alike ID, a config error
//! outside `[flow]`, no `HOME`, a store error. A bad `[flow]` table is a
//! row's cause, never an exit; a queue `--trace` cannot read, a note.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_core::trace::{Rollup, RollupCounts};
use specengine_core::{
    FlowAsk, FlowConfig, FlowEdge, FlowGap, FlowNeed, FlowNode, FlowStage, GapCause, GapCode,
    flow_read,
};

use crate::cap::{View, lines_within};
use crate::corpus::{LeftOut, depth_of, holders_warning, indexed_in, locate};
use crate::location::open_index;
use crate::project::discover_with_text;
use crate::search::notes;
use crate::show::classify;
use crate::trace::{NO_STAGE, TraceBadge, flow_badges};
use crate::{CliError, Env, Globals, Message, one_line};

/// The most items a gap row prints, and its JSON lists, when capped.
pub const GAP_ITEMS_SHOWN: usize = 50;

/// How the header and `no-start` name a read with no `ROOT`.
const ALL: &str = "all";

/// `spec flow` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowRequest {
    /// `ROOT` as given; `None`: every flow document.
    pub root: Option<String>,
    /// `--depth N`; `None`: unbounded.
    pub depth: Option<i64>,
    /// `--stage S`: keep the shown nodes of this stage, byte for byte, `-`
    /// the flow steps with none; `None`: every shown node.
    pub stage: Option<String>,
    /// `--trace`: each shown node's `rollup` and `counts`.
    pub trace: bool,
}

impl FlowRequest {
    /// `--stage` as core's cut.
    fn stage(&self) -> Option<FlowStage<'_>> {
        self.stage.as_deref().map(|stage| match stage {
            NO_STAGE => FlowStage::Unstaged,
            named => FlowStage::Named(named),
        })
    }
}

/// What `spec flow` read, or why `ROOT` names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowOutcome {
    /// `ROOT` as given.
    pub root: Option<String>,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    pub depth: Option<usize>,
    pub left_out: LeftOut,
    /// All ten, in table order, each with all its items.
    pub gaps: Vec<FlowGap>,
    /// By (distance, unreached last; path).
    pub nodes: Vec<FlowNode>,
    /// By (`from`, `to`, type, kind).
    pub edges: Vec<FlowEdge>,
    /// `--trace`: by a node's `file`; none joined, empty.
    pub badges: BTreeMap<usize, TraceBadge>,
    /// How many of `nodes`, then of `edges`, are printed (text and JSON
    /// alike).
    pub shown_nodes: usize,
    pub shown_edges: usize,
    /// How many of a row's items are printed; `None`: all.
    pub gap_items: Option<usize>,
}

impl FlowOutcome {
    fn omitted_nodes(&self) -> usize {
        self.nodes.len().saturating_sub(self.shown_nodes)
    }

    fn omitted_edges(&self) -> usize {
        self.edges.len().saturating_sub(self.shown_edges)
    }

    pub fn truncated(&self) -> bool {
        self.omitted_nodes() > 0 || self.omitted_edges() > 0
    }
}

/// `spec flow`: updates the index, then reads the flow.
pub fn flow(env: &Env, globals: &Globals, request: &FlowRequest) -> Result<FlowOutcome, CliError> {
    flow_with_view(env, globals, request, View::Capped)
}

/// [`flow`], its answer bounded by `view` (the daemon's
/// [`View::Browser`]: every node, edge and item shown, `truncated` false).
pub fn flow_with_view(
    env: &Env,
    globals: &Globals,
    request: &FlowRequest,
    view: View,
) -> Result<FlowOutcome, CliError> {
    let depth = depth_of(request.depth)?;
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = FlowOutcome {
        root: request.root.clone(),
        reason: None,
        messages: Vec::new(),
        depth,
        left_out: LeftOut::default(),
        gaps: Vec::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        badges: BTreeMap::new(),
        shown_nodes: 0,
        shown_edges: 0,
        gap_items: match view {
            View::Capped => Some(GAP_ITEMS_SHOWN),
            View::Browser => None,
        },
    };
    let target = match request.root.as_deref().map(str::trim) {
        None => None,
        Some(written) => {
            match classify(
                written,
                scheme,
                &mut messages,
                "`spec flow` reads the current files",
            )? {
                Ok(target) => Some((target, written)),
                Err(reason) => return Ok(not_found(outcome, reason, messages)),
            }
        }
    };

    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&input, scheme, &project.config.paths);
    let roots: Option<Vec<NodeAt>> = match target {
        Some((target, written)) => match locate(&graph, &target, written)? {
            Ok(nodes) => {
                messages.extend(holders_warning(&graph, written, &nodes, "all read"));
                Some(nodes)
            }
            Err(reason) => return Ok(not_found(outcome, reason, messages)),
        },
        None => None,
    };
    let config = FlowConfig::from_toml(&text, scheme);
    let read = flow_read(
        &graph,
        &FlowAsk {
            roots: roots.as_deref(),
            depth,
            stage: request.stage(),
            config: &config,
            config_label: &project.config_label,
            scope_name: outcome.root.as_deref().unwrap_or(ALL),
        },
    );
    for file in read.left_out {
        outcome.left_out.add(graph.standing(file));
    }
    if request.trace {
        // Over every node the read without `--stage` shows: a rooted read's
        // notes name the cards of what they hold, and a stage moves none.
        outcome.badges = flow_badges(
            env,
            &open.slug,
            &open.index,
            &graph,
            &read.shown,
            roots.is_none(),
            &mut messages,
        );
        if request.stage.is_some() {
            let kept: BTreeSet<usize> = read.nodes.iter().map(|node| node.file).collect();
            outcome.badges.retain(|file, _| kept.contains(file));
        }
    }
    outcome.gaps = read.gaps;
    outcome.nodes = read.nodes;
    outcome.edges = read.edges;
    outcome.messages = messages;
    (outcome.shown_nodes, outcome.shown_edges) = match view {
        View::Capped => capped(&outcome),
        View::Browser => (outcome.nodes.len(), outcome.edges.len()),
    };
    Ok(outcome)
}

fn not_found(mut outcome: FlowOutcome, reason: String, messages: Vec<Message>) -> FlowOutcome {
    outcome.reason = Some(one_line(&reason));
    outcome.messages = messages;
    outcome
}

/// How many nodes, then edges, fit within the cap after the header and the
/// gap rows, which always print: the nodes first; a node cut drops every
/// edge.
fn capped(outcome: &FlowOutcome) -> (usize, usize) {
    let mut used = header(outcome).chars().count() + 1;
    for gap in &outcome.gaps {
        used += gap_line(gap, outcome.gap_items).chars().count() + 1;
    }
    let (shown_nodes, used) = lines_within(
        outcome.nodes.iter().map(|node| {
            node_line(node, outcome.badges.get(&node.file))
                .chars()
                .count()
                + 1
        }),
        used,
        false,
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

/// `; left out: <g> generated, <t> archived`, zero parts omitted: this
/// command has no flag that takes them in.
fn left_out_suffix(left_out: LeftOut) -> String {
    let mut parts = Vec::new();
    if left_out.generated > 0 {
        parts.push(format!("{} generated", left_out.generated));
    }
    if left_out.tier3 > 0 {
        parts.push(format!("{} archived", left_out.tier3));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("; left out: {}", parts.join(", "))
    }
}

fn header(outcome: &FlowOutcome) -> String {
    format!(
        "flow {} | nodes {}, edges {}, depth {}{}",
        outcome
            .root
            .as_deref()
            .map_or_else(|| ALL.to_owned(), one_line),
        outcome.nodes.len(),
        outcome.edges.len(),
        outcome
            .depth
            .map_or_else(|| "-".to_owned(), |depth| depth.to_string()),
        left_out_suffix(outcome.left_out)
    )
}

/// A row's one line: why it stands where it does and, where it can, how to
/// clear it. A row at 0 has none to say.
fn cause_text(cause: &GapCause) -> String {
    match cause {
        GapCause::NoRule(FlowNeed::Start) => {
            "no [[flow.start]] rule: add one naming the first step by paths".to_owned()
        }
        GapCause::NoRule(FlowNeed::End) => {
            "no [[flow.end]] rule: add one naming the last step by paths".to_owned()
        }
        GapCause::NoRule(FlowNeed::Definition) => {
            "no [[flow.definition]] rule: add one naming what steps share, by paths or kinds"
                .to_owned()
        }
        GapCause::Config(message) => message.clone(),
        GapCause::NoStart => "no start and no entering edge to walk from".to_owned(),
        GapCause::Found(code) => remedy(*code).to_owned(),
    }
}

/// What a row above 0 means and how to clear it.
const fn remedy(code: GapCode) -> &'static str {
    match code {
        GapCode::NoStart => {
            "no step here is a start: match the flow's first step in [[flow.start]], or, reading \
             a part, add a follows link into it from outside"
        }
        GapCode::TwoStarts => "more than one start: narrow [[flow.start]] to one",
        GapCode::Unreachable => {
            "no path from a start reaches it: add a follows link on it, naming a step before it"
        }
        GapCode::NoWayToEnd => {
            "no path from it reaches an end: have a later step follow it, or match it in \
             [[flow.end]]"
        }
        GapCode::TrapCycle => {
            "a loop with no way out: have a step outside it follow a member, or match a member \
             in [[flow.end]]"
        }
        GapCode::UnjoinedParallel => {
            "a parallel fork with one branch, or branches that never meet: add a branch, or a \
             later step following them all"
        }
        GapCode::CycleNoRepeat => {
            "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}"
        }
        GapCode::DefinitionUnused => {
            "no document uses it: add a uses link to it, or drop it from [[flow.definition]]"
        }
        GapCode::DefinitionOnce => {
            "only one document uses it: add a uses link from a second document, or fold it into \
             that document"
        }
        GapCode::SupplyUnfed => {
            "these only supply each other: add a supplies link into them from a node outside"
        }
    }
}

/// `gap <code> | <state> | <count>[ | <item>, …][ | <cause>]`.
fn gap_line(gap: &FlowGap, limit: Option<usize>) -> String {
    let mut line = format!(
        "gap {} | {} | {}",
        gap.code.as_str(),
        gap.state.as_str(),
        gap.count
            .map_or_else(|| "-".to_owned(), |count| count.to_string())
    );
    if !gap.items.is_empty() {
        let shown = limit.map_or(gap.items.len(), |limit| limit.min(gap.items.len()));
        let items: Vec<String> = gap.items[..shown]
            .iter()
            .map(|item| {
                let members: Vec<String> = item.iter().map(|name| one_line(name)).collect();
                members.join(" + ")
            })
            .collect();
        line.push_str(" | ");
        line.push_str(&items.join(", "));
        if shown < gap.items.len() {
            line.push_str(&format!(", … and {} more", gap.items.len() - shown));
        }
    }
    if let Some(cause) = &gap.cause {
        line.push_str(" | ");
        line.push_str(&one_line(&cause_text(cause)));
    }
    line
}

/// The node's line, no line end.
fn node_line(node: &FlowNode, badge: Option<&TraceBadge>) -> String {
    let name = node.id.as_deref().unwrap_or(&node.path);
    let mut line = format!(
        "{} {} | {} | {} | {}:{}",
        node.distance
            .map_or_else(|| "-".to_owned(), |distance| distance.to_string()),
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
    if node.start {
        line.push_str(" | start");
    }
    if node.end {
        line.push_str(" | end");
    }
    if let Some(group) = &node.group {
        line.push_str(&format!(" | group {}", one_line(group)));
    }
    if let Some(stage) = &node.stage {
        line.push_str(&format!(" | stage {}", one_line(stage)));
    }
    let inner: usize = node.inner.iter().map(|inner| inner.count).sum();
    if node.holds > 0 || inner > 0 {
        line.push_str(&format!(" | holds {}, inner {inner}", node.holds));
    }
    if node.gaps > 0 {
        line.push_str(&format!(" | gaps {}", node.gaps));
    }
    if let Some(badge) = badge {
        line.push_str(&format!(" | rollup {}", badge.rollup.as_str()));
    }
    line
}

/// The edge's line, no line end.
fn edge_line(edge: &FlowEdge) -> String {
    let mut line = format!(
        "{} -> {} | {} | x{}",
        one_line(&edge.from),
        one_line(&edge.to),
        edge.link_type,
        edge.count
    );
    if let Some(kind) = edge.kind {
        line.push_str(&format!(" | kind {}", kind.as_str()));
    }
    if let Some(when) = &edge.when {
        line.push_str(&format!(" | when {}", one_line(when)));
    }
    line.push_str(&format!(" | {}:{}", one_line(&edge.path), edge.line));
    line
}

pub(crate) fn render_text(outcome: &FlowOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let mut out = header(outcome);
    out.push('\n');
    for gap in &outcome.gaps {
        out.push_str(&gap_line(gap, outcome.gap_items));
        out.push('\n');
    }
    for node in &outcome.nodes[..outcome.shown_nodes.min(outcome.nodes.len())] {
        out.push_str(&node_line(node, outcome.badges.get(&node.file)));
        out.push('\n');
    }
    for edge in &outcome.edges[..outcome.shown_edges.min(outcome.edges.len())] {
        out.push_str(&edge_line(edge));
        out.push('\n');
    }
    if outcome.truncated() {
        out.push_str(&format!(
            "[truncated: {} of {} nodes and {} of {} edges not shown; give a ROOT or lower --depth]\n",
            outcome.omitted_nodes(),
            outcome.nodes.len(),
            outcome.omitted_edges(),
            outcome.edges.len()
        ));
    }
    out
}

#[derive(Serialize)]
struct FlowJson<'a> {
    root: Option<&'a str>,
    reason: Option<&'a str>,
    depth: Option<usize>,
    notes: Vec<String>,
    left_out: LeftOut,
    truncated: bool,
    omitted_nodes: usize,
    omitted_edges: usize,
    gaps: Vec<GapJson<'a>>,
    nodes: Vec<NodeJson<'a>>,
    edges: Vec<EdgeJson<'a>>,
}

#[derive(Serialize)]
struct GapJson<'a> {
    code: &'static str,
    state: &'static str,
    count: Option<usize>,
    items: &'a [Vec<String>],
    cause: Option<String>,
}

#[derive(Serialize)]
struct NodeJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    distance: Option<usize>,
    group: Option<&'a str>,
    stage: Option<&'a str>,
    start: bool,
    end: bool,
    holds: usize,
    inner: Vec<InnerJson>,
    gaps: usize,
    rollup: Option<Rollup>,
    counts: Option<RollupCounts>,
}

#[derive(Serialize)]
struct InnerJson {
    #[serde(rename = "type")]
    link_type: &'static str,
    count: usize,
}

#[derive(Serialize)]
struct EdgeJson<'a> {
    from: &'a str,
    to: &'a str,
    #[serde(rename = "type")]
    link_type: &'static str,
    kind: Option<&'static str>,
    when: Option<&'a str>,
    count: usize,
    path: &'a str,
    line: usize,
}

fn gap_json(gap: &FlowGap, limit: Option<usize>) -> GapJson<'_> {
    let shown = limit.map_or(gap.items.len(), |limit| limit.min(gap.items.len()));
    GapJson {
        code: gap.code.as_str(),
        state: gap.state.as_str(),
        count: gap.count,
        items: &gap.items[..shown],
        cause: gap.cause.as_ref().map(cause_text),
    }
}

fn node_json<'a>(node: &'a FlowNode, badge: Option<&TraceBadge>) -> NodeJson<'a> {
    NodeJson {
        id: node.id.as_deref(),
        kind: node.kind.as_deref(),
        title: node.title.as_deref(),
        path: &node.path,
        line: node.line,
        distance: node.distance,
        group: node.group.as_deref(),
        stage: node.stage.as_deref(),
        start: node.start,
        end: node.end,
        holds: node.holds,
        inner: node
            .inner
            .iter()
            .map(|inner| InnerJson {
                link_type: inner.link_type,
                count: inner.count,
            })
            .collect(),
        gaps: node.gaps,
        rollup: badge.map(|badge| badge.rollup),
        counts: badge.map(|badge| badge.counts),
    }
}

fn edge_json(edge: &FlowEdge) -> EdgeJson<'_> {
    EdgeJson {
        from: &edge.from,
        to: &edge.to,
        link_type: edge.link_type,
        kind: edge.kind.map(|kind| kind.as_str()),
        when: edge.when.as_deref(),
        count: edge.count,
        path: &edge.path,
        line: edge.line,
    }
}

fn view(outcome: &FlowOutcome) -> FlowJson<'_> {
    FlowJson {
        root: outcome.root.as_deref(),
        reason: outcome.reason.as_deref(),
        depth: outcome.depth,
        notes: notes(&outcome.messages),
        left_out: outcome.left_out,
        truncated: outcome.truncated(),
        omitted_nodes: outcome.omitted_nodes(),
        omitted_edges: outcome.omitted_edges(),
        gaps: outcome
            .gaps
            .iter()
            .map(|gap| gap_json(gap, outcome.gap_items))
            .collect(),
        nodes: outcome.nodes[..outcome.shown_nodes.min(outcome.nodes.len())]
            .iter()
            .map(|node| node_json(node, outcome.badges.get(&node.file)))
            .collect(),
        edges: outcome.edges[..outcome.shown_edges.min(outcome.edges.len())]
            .iter()
            .map(edge_json)
            .collect(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for FlowOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
