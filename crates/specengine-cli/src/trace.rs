//! `spec trace [REF]` (task spec `card-trace`): each node's computed state
//! against the cards that answer it, and what a change of it marks. Core's
//! `trace` judges; this file reads the root's index and the queue, resolves
//! `REF`, bounds and prints.
//!
//! ```text
//! trace <REF or all> | nodes <n>, cards <c> | no card <a>, in work <b>, suspect <s>, departed <d>, confirmed <f>
//! <state> | <name> | <title> | <path> | cards <c>
//!   card <id> | <flow>/<column> | <mark> | <title>
//!   uses <name> | <state> | cards <c>
//! [truncated: <k> of <c> cards of <name>, <j> of <n> nodes not shown; give a narrower REF]
//! ```
//!
//! Rows only, and every state is per read, of this root's index: nothing
//! here is stored, and it changes no status, finding, claim or gate. Scope:
//! `REF`'s holders and the documents they hold by `parent:`; no `REF`: every
//! live document and every section a card targets. A node's cards have a
//! target in `SpecGraph::within(node)`, cancelled ones never read. `uses` is
//! one level of incoming links, a consequence and never a mark.
//!
//! The cut, at [`crate::OUTPUT_CAP_CHARS`]: nodes whole, the first that does
//! not fit and every later one left out; a first node over the cap alone
//! shows its row and the card and `uses` lines that fit, so an answer is never
//! empty. The tail counts what is cut, JSON holds the printed rows, `omitted`
//! counts them, and the header's and a row's counts stay whole.
//!
//! Exit 0 whatever is listed; 1 only a `REF` naming nothing; 2 as `spec
//! flow`. The queue is read as `spec roadmap` reads it, no schema step; no
//! queue: `no card`. A corrupt card row is skipped with a note, an
//! unreadable discrepancy reads unsettled with a note.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_core::check::{Endpoint, NodeAt, SpecGraph, Standing};
use specengine_core::revision;
use specengine_core::trace::{
    CardMark, MarkFacts, Rollup, RollupCounts, TargetNorms, TargetNow, card_mark, mark_notes,
    rollup,
};
use specengine_model::{Card, CardVerdict, USES};
use specengine_store::{NodeNorm, ProposalQueue as _, ProposalStatus, SqliteIndex, SqliteQueue};

use crate::cap::{View, lines_within};
use crate::corpus::{Admission, holders_warning, indexed_in, locate};
use crate::location::{db_path, open_index};
use crate::project::{ProjectRoot, discover};
use crate::proposals::queue_cannot;
use crate::search::notes;
use crate::show::classify;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line, store_error};

/// Why a REF's `@rev` is ignored.
const REV_IGNORED: &str = "`spec trace` reads the current files";

/// How the header names a read with no `REF`.
const ALL: &str = "all";

/// What a tail may take beside the name of the node it cuts.
const TAIL_ROOM: usize = 160;

/// `spec trace` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceRequest {
    /// `REF` as given; `None`: every live document.
    pub reference: Option<String>,
}

/// One card of a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceCard {
    pub id: String,
    pub flow: String,
    pub column: String,
    pub title: String,
    pub mark: CardMark,
}

/// One node with an incoming `uses`: a consequence of a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceUse {
    pub id: String,
    pub rollup: Rollup,
    pub cards: usize,
}

/// One node's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
    /// Its ID, else its path.
    pub id: String,
    pub title: Option<String>,
    pub path: String,
    pub rollup: Rollup,
    /// Over the node and the documents it holds by `parent:`.
    pub counts: RollupCounts,
    pub cards: Vec<TraceCard>,
    pub uses: Vec<TraceUse>,
}

/// How much of a first node over the cap prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Partial {
    cards: usize,
    uses: usize,
}

/// What `spec trace` read, or why `REF` names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceOutcome {
    /// `REF` as given.
    pub reference: Option<String>,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    /// Over the listed nodes.
    pub counts: RollupCounts,
    /// The cards the listed nodes hold, each once.
    pub cards: usize,
    /// By (path, position).
    pub nodes: Vec<TraceNode>,
    /// How many of `nodes` are printed (text and JSON alike).
    pub shown: usize,
    /// The first node, when it alone is over the cap: its rows that print.
    partial: Option<Partial>,
}

impl TraceOutcome {
    /// Exit 0 whatever is listed; 1 a `REF` naming nothing.
    pub fn exit(&self) -> Exit {
        if self.reason.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    /// The nodes not shown.
    fn omitted_nodes(&self) -> usize {
        self.nodes.len().saturating_sub(self.shown)
    }

    /// The cards, then the `uses`, of a node shown in part not shown.
    fn omitted_rows(&self) -> (usize, usize) {
        match (self.partial, self.nodes.first()) {
            (Some(partial), Some(node)) => (
                node.cards.len().saturating_sub(partial.cards),
                node.uses.len().saturating_sub(partial.uses),
            ),
            _ => (0, 0),
        }
    }

    /// Every row the cut left out: nodes, and a node's cards and `uses`.
    fn omitted(&self) -> usize {
        let (cards, uses) = self.omitted_rows();
        self.omitted_nodes() + cards + uses
    }

    fn shown_nodes(&self) -> &[TraceNode] {
        &self.nodes[..self.shown.min(self.nodes.len())]
    }

    /// How many of `node`'s cards and `uses` print.
    fn shown_rows(&self, node: &TraceNode, at: usize) -> Partial {
        match self.partial {
            Some(partial) if at == 0 => partial,
            _ => Partial {
                cards: node.cards.len(),
                uses: node.uses.len(),
            },
        }
    }
}

// ------------------------------------------------------------- the reads

/// What the queue says of the project's cards, read once.
#[derive(Debug, Default)]
pub(crate) struct QueueRead {
    /// Every readable card but the cancelled, by number.
    pub cards: Vec<Card>,
    /// Discrepancies `applied` or `rejected`.
    pub settled: BTreeSet<String>,
    /// Each card a live correction corrects, and the first such correction.
    pub corrections: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

/// The queue as `spec roadmap` reads it, no schema step; no queue: empty.
/// A failure is the caller's: `spec trace` exits, `spec impact` notes it.
pub(crate) fn queue_read(env: &Env, slug: &str) -> Result<QueueRead, CliError> {
    let db = db_path(env, slug)?;
    let queue = SqliteQueue::open_existing(&db, slug).map_err(queue_cannot)?;
    let Some(queue) = queue else {
        return Ok(QueueRead::default());
    };
    let listed = queue.list_existing_cards().map_err(queue_cannot)?;
    let mut read = QueueRead::default();
    read.notes.extend(
        listed
            .unreadable
            .iter()
            .map(|row| one_line(&format!("{row}; not read"))),
    );
    for card in listed.cards {
        if card.cancellation.is_some() {
            continue;
        }
        if card.acceptance.is_none()
            && let Some(original) = &card.corrects
        {
            read.corrections
                .entry(original.clone())
                .or_insert_with(|| card.id.clone());
        }
        settled_of(&queue, &card, &mut read);
        read.cards.push(card);
    }
    Ok(read)
}

/// `card`'s discrepancy into [`QueueRead::settled`] when it is `applied` or
/// `rejected`; one that does not read stays unsettled, with a note.
fn settled_of(queue: &SqliteQueue, card: &Card, read: &mut QueueRead) {
    let Some(acceptance) = &card.acceptance else {
        return;
    };
    if acceptance.verdict != CardVerdict::Departs {
        return;
    }
    let Some(discrepancy) = &acceptance.discrepancy else {
        read.notes.push(format!(
            "{}: its accept names no discrepancy; read as unsettled",
            card.id
        ));
        return;
    };
    match queue.get(discrepancy) {
        Ok(Some(proposal)) => {
            if matches!(
                proposal.status,
                ProposalStatus::Applied | ProposalStatus::Rejected
            ) {
                read.settled.insert(discrepancy.clone());
            }
        }
        Ok(None) => read.notes.push(format!(
            "{}: its discrepancy {discrepancy} is not in the queue; read as unsettled",
            card.id
        )),
        Err(error) => read.notes.push(one_line(&format!(
            "{}: its discrepancy {discrepancy} cannot be read: {error}; read as unsettled",
            card.id
        ))),
    }
}

/// Each stored target's nodes in `graph`, by (path, position): empty when it
/// no longer resolves. A stored target is canonical, so a defect of its form
/// reads as that too.
pub(crate) fn target_places<'t>(
    graph: &SpecGraph<'_>,
    targets: impl IntoIterator<Item = &'t String>,
) -> BTreeMap<String, Vec<NodeAt>> {
    let scheme = graph.scheme();
    let mut places = BTreeMap::new();
    for target in targets {
        if places.contains_key(target) {
            continue;
        }
        let mut quiet = Vec::new();
        let found = match classify(target, scheme, &mut quiet, REV_IGNORED) {
            Ok(Ok(found)) => locate(graph, &found, target)
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        places.insert(target.clone(), found);
    }
    places
}

/// What each target with a node reads now, its first node's revision and
/// stored `norm_hash`: a target with none is absent.
pub(crate) fn target_norms(
    graph: &SpecGraph<'_>,
    stored: &[NodeNorm],
    places: &BTreeMap<String, Vec<NodeAt>>,
) -> TargetNorms {
    let hashes: BTreeMap<(&str, usize), Option<&String>> = stored
        .iter()
        .map(|node| ((node.path.as_str(), node.ord), node.norm_hash.as_ref()))
        .collect();
    places
        .iter()
        .filter_map(|(target, nodes)| {
            let at = nodes.first()?;
            let path = graph.paths()[at.file];
            let parsed = graph.file(at.file)?.parsed.as_ref()?;
            Some((
                target.clone(),
                TargetNow {
                    rev: revision(parsed, at.ord).number(),
                    norm_hash: hashes.get(&(path, at.ord)).copied().flatten().cloned(),
                },
            ))
        })
        .collect()
}

/// [`target_norms`] of `targets` over a handle whose index was refreshed for
/// `input`'s corpus.
pub(crate) fn norms_read<'t>(
    index: &SqliteIndex,
    graph: &SpecGraph<'_>,
    targets: impl IntoIterator<Item = &'t String>,
) -> Result<TargetNorms, CliError> {
    let stored = index.node_norms().map_err(store_error)?;
    let places = target_places(graph, targets);
    Ok(target_norms(graph, &stored, &places))
}

/// What `targets` read now in this root's index, refreshed first; none
/// asked, no index is opened.
pub(crate) fn fresh_norms(
    env: &Env,
    project: &ProjectRoot,
    targets: &BTreeSet<&String>,
    messages: &mut Vec<Message>,
) -> Result<TargetNorms, CliError> {
    if targets.is_empty() {
        return Ok(TargetNorms::new());
    }
    let mut open = open_index(env, project)?;
    let input = indexed_in(&mut open.index, project, messages, false)?;
    let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
    norms_read(&open.index, &graph, targets.iter().copied())
}

/// The documents `at` holds by `parent:`, through sections or not, live
/// ones, by (path, position).
pub(crate) fn held_documents(graph: &SpecGraph<'_>, at: NodeAt) -> Vec<NodeAt> {
    let mut seen = BTreeSet::from([at]);
    let mut stack = vec![at];
    let mut found = Vec::new();
    while let Some(node) = stack.pop() {
        for &child in graph.children(node) {
            if child.ord == 0 && graph.standing(child.file) != Standing::Live {
                continue;
            }
            if seen.insert(child) {
                stack.push(child);
                if child.ord == 0 {
                    found.push(child);
                }
            }
        }
    }
    found.sort_unstable();
    found
}

/// The nodes of a corpus each card holds, per read.
struct Reading<'a> {
    graph: &'a SpecGraph<'a>,
    cards: &'a [Card],
    facts: MarkFacts<'a>,
    /// Per node, the cards with a target on it and those targets.
    held: BTreeMap<NodeAt, Vec<(usize, &'a String)>>,
}

impl Reading<'_> {
    /// The cards with a target in `within(at)`, by number, each with those
    /// targets.
    fn cards_of(&self, at: NodeAt) -> BTreeMap<usize, Vec<String>> {
        let mut found: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for node in self.graph.within(at) {
            for &(card, target) in self.held.get(&node).into_iter().flatten() {
                let on = found.entry(card).or_default();
                if !on.contains(target) {
                    on.push(target.clone());
                }
            }
        }
        found
    }

    /// `at`'s cards with their marks over it, and its state.
    fn state(&self, at: NodeAt) -> (Rollup, Vec<(&Card, CardMark)>) {
        let marked: Vec<(&Card, CardMark)> = self
            .cards_of(at)
            .into_iter()
            .map(|(card, on)| {
                let card = &self.cards[card];
                (card, card_mark(card, &on, &self.facts))
            })
            .collect();
        (rollup(marked.iter().map(|(_, mark)| mark)), marked)
    }

    /// The nodes using `at` or a section of it, one level, outside it, each
    /// once, with what they read now.
    fn uses_of(&self, at: NodeAt) -> Vec<TraceUse> {
        let graph = self.graph;
        let admission = Admission::new(graph, false, [at.file]);
        let links = graph.links(at, |file| admission.admits(file));
        let within = graph.within(at);
        let mut using = BTreeSet::new();
        for (index, _) in links.incoming {
            let edge = &graph.edges()[index];
            if edge.link_type != USES {
                continue;
            }
            if let Endpoint::Nodes(nodes) = &edge.source {
                using.extend(nodes.iter().filter(|node| !within.contains(node)).copied());
            }
        }
        using
            .into_iter()
            .map(|node| {
                let (state, cards) = self.state(node);
                TraceUse {
                    id: graph.name(node),
                    rollup: state,
                    cards: cards.len(),
                }
            })
            .collect()
    }

    fn row(&self, at: NodeAt) -> TraceNode {
        let graph = self.graph;
        let (state, marked) = self.state(at);
        let mut counts = RollupCounts::default();
        counts.add(state);
        for document in held_documents(graph, at) {
            counts.add(self.state(document).0);
        }
        TraceNode {
            id: graph.name(at),
            title: graph.node(at).and_then(|node| node.title.clone()),
            path: graph.paths()[at.file].to_owned(),
            rollup: state,
            counts,
            cards: marked
                .into_iter()
                .map(|(card, mark)| TraceCard {
                    id: card.id.clone(),
                    flow: card.flow.clone(),
                    column: card.column.clone(),
                    title: card.title.clone(),
                    mark,
                })
                .collect(),
            uses: self.uses_of(at),
        }
    }
}

/// `spec trace`, cut at the output cap.
pub fn trace(
    env: &Env,
    globals: &Globals,
    request: &TraceRequest,
) -> Result<TraceOutcome, CliError> {
    trace_with_view(env, globals, request, View::Capped)
}

/// [`trace`], its answer bounded by `view` (the daemon's [`View::Browser`]:
/// every node, `truncated` false).
pub fn trace_with_view(
    env: &Env,
    globals: &Globals,
    request: &TraceRequest,
    view: View,
) -> Result<TraceOutcome, CliError> {
    let project = discover(env, globals)?;
    let slug = project.slug()?.to_owned();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = TraceOutcome {
        reference: request.reference.clone(),
        reason: None,
        messages: Vec::new(),
        counts: RollupCounts::default(),
        cards: 0,
        nodes: Vec::new(),
        shown: 0,
        partial: None,
    };
    let target = match request.reference.as_deref().map(str::trim) {
        None => None,
        Some(written) => match classify(written, scheme, &mut messages, REV_IGNORED)? {
            Ok(target) => Some((target, written)),
            Err(reason) => return Ok(not_found(outcome, &reason, messages)),
        },
    };
    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, false)?;
    let graph = SpecGraph::new(&input, scheme, &project.config.paths);
    let roots = match target {
        Some((target, written)) => match locate(&graph, &target, written)? {
            Ok(nodes) => {
                messages.extend(holders_warning(&graph, written, &nodes, "all read"));
                Some(nodes)
            }
            Err(reason) => return Ok(not_found(outcome, &reason, messages)),
        },
        None => None,
    };
    let read = queue_read(env, &slug)?;
    let stored = open.index.node_norms().map_err(store_error)?;
    let places = target_places(&graph, read.cards.iter().flat_map(|card| &card.targets));
    let norms = target_norms(&graph, &stored, &places);
    messages.extend(read.notes.iter().cloned().map(Message::Note));
    let mut held: BTreeMap<NodeAt, Vec<(usize, &String)>> = BTreeMap::new();
    for (index, card) in read.cards.iter().enumerate() {
        for target in &card.targets {
            for &at in places.get(target).into_iter().flatten() {
                held.entry(at).or_default().push((index, target));
            }
        }
    }
    let reading = Reading {
        graph: &graph,
        cards: &read.cards,
        facts: MarkFacts {
            norms: &norms,
            settled: &read.settled,
            corrections: &read.corrections,
        },
        held,
    };
    let scope = listed(&reading, roots.as_deref());
    let mut cards = BTreeSet::new();
    for &at in &scope {
        let row = reading.row(at);
        outcome.counts.add(row.rollup);
        cards.extend(row.cards.iter().map(|card| card.id.clone()));
        outcome.nodes.push(row);
    }
    outcome.cards = cards.len();
    // A target gone stands on no node: with no `REF`, every card speaks.
    for card in &read.cards {
        if roots.is_none() || cards.contains(&card.id) {
            messages.extend(mark_notes(card, &norms).into_iter().map(Message::Note));
        }
    }
    outcome.messages = messages;
    (outcome.shown, outcome.partial) = match view {
        View::Capped => capped(&outcome),
        View::Browser => (outcome.nodes.len(), None),
    };
    Ok(outcome)
}

/// The characters of `lines`, a line end each.
fn cost(lines: &[String]) -> usize {
    lines.iter().map(|line| line.chars().count() + 1).sum()
}

/// How many nodes print whole within the cap; none fitting, the first in
/// part: its row, then its card and `uses` lines while they fit beside the
/// tail, so the answer is never empty.
fn capped(outcome: &TraceOutcome) -> (usize, Option<Partial>) {
    let used = header(outcome).chars().count() + 1;
    let whole = |node: &TraceNode| Partial {
        cards: node.cards.len(),
        uses: node.uses.len(),
    };
    let blocks = outcome
        .nodes
        .iter()
        .map(|node| cost(&node_lines(node, whole(node))));
    let (fit, _) = lines_within(blocks, used, false);
    let Some(first) = outcome.nodes.first().filter(|_| fit == 0) else {
        return (fit, None);
    };
    let lines = node_lines(first, whole(first));
    let (row, rows) = lines.split_at(1);
    // The tail names the node twice when it cuts both cards and `uses`.
    let room = used + cost(row) + TAIL_ROOM + 2 * first.id.chars().count();
    let (rows, _) = lines_within(
        rows.iter().map(|line| line.chars().count() + 1),
        room,
        false,
    );
    let cards = rows.min(first.cards.len());
    (
        1,
        Some(Partial {
            cards,
            uses: rows - cards,
        }),
    )
}

/// The nodes a read lists, by (path, position): `roots` and the documents
/// they hold by `parent:`; none given, every live document and every section
/// a card targets.
fn listed(reading: &Reading<'_>, roots: Option<&[NodeAt]>) -> Vec<NodeAt> {
    let graph = reading.graph;
    let mut scope: BTreeSet<NodeAt> = BTreeSet::new();
    match roots {
        Some(roots) => {
            for &root in roots {
                scope.insert(root);
                scope.extend(held_documents(graph, root));
            }
        }
        None => {
            scope.extend(
                graph
                    .documents()
                    .filter(|at| graph.standing(at.file) == Standing::Live),
            );
            scope.extend(
                reading
                    .held
                    .keys()
                    .filter(|at| graph.standing(at.file) == Standing::Live),
            );
        }
    }
    scope.into_iter().collect()
}

fn not_found(mut outcome: TraceOutcome, reason: &str, messages: Vec<Message>) -> TraceOutcome {
    outcome.reason = Some(one_line(reason));
    outcome.messages = messages;
    outcome
}

// -------------------------------------------------------------- the text

fn header(outcome: &TraceOutcome) -> String {
    let counts = outcome.counts;
    format!(
        "trace {} | nodes {}, cards {} | no card {}, in work {}, suspect {}, departed {}, \
         confirmed {}",
        outcome
            .reference
            .as_deref()
            .map_or_else(|| ALL.to_owned(), one_line),
        outcome.nodes.len(),
        outcome.cards,
        counts.no_card,
        counts.in_work,
        counts.suspect,
        counts.departed,
        counts.confirmed
    )
}

/// A node's row and `shown` of its card and `uses` lines.
fn node_lines(node: &TraceNode, shown: Partial) -> Vec<String> {
    let mut lines = vec![format!(
        "{} | {} | {} | {} | cards {}",
        node.rollup.as_str(),
        one_line(&node.id),
        node.title
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
        one_line(&node.path),
        node.cards.len()
    )];
    for card in node.cards.iter().take(shown.cards) {
        lines.push(format!(
            "  card {} | {}/{} | {} | {}",
            card.id,
            card.flow,
            card.column,
            one_line(&card.mark.text()),
            one_line(&card.title)
        ));
    }
    for using in node.uses.iter().take(shown.uses) {
        lines.push(format!(
            "  uses {} | {} | cards {}",
            one_line(&using.id),
            using.rollup.as_str(),
            using.cards
        ));
    }
    lines
}

pub(crate) fn render_text(outcome: &TraceOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let mut out = header(outcome);
    out.push('\n');
    for (at, node) in outcome.shown_nodes().iter().enumerate() {
        for line in node_lines(node, outcome.shown_rows(node, at)) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    if outcome.omitted() > 0 {
        out.push_str(&tail(outcome));
        out.push('\n');
    }
    escape_controls(&out)
}

/// `[truncated: <parts> not shown; give a REF]`, zero parts omitted; a REF
/// given, a narrower one.
fn tail(outcome: &TraceOutcome) -> String {
    let (cards, uses) = outcome.omitted_rows();
    let mut parts = Vec::new();
    if let Some(node) = outcome.nodes.first().filter(|_| cards + uses > 0) {
        let name = one_line(&node.id);
        if cards > 0 {
            parts.push(format!("{cards} of {} cards of {name}", node.cards.len()));
        }
        if uses > 0 {
            parts.push(format!("{uses} of {} uses of {name}", node.uses.len()));
        }
    }
    let nodes = outcome.omitted_nodes();
    if nodes > 0 {
        parts.push(format!("{nodes} of {} nodes", outcome.nodes.len()));
    }
    let narrow = if outcome.reference.is_some() {
        "give a narrower REF"
    } else {
        "give a REF"
    };
    format!("[truncated: {} not shown; {narrow}]", parts.join(", "))
}

// -------------------------------------------------------------- the JSON

#[derive(Serialize)]
struct TraceJson<'a> {
    #[serde(rename = "ref")]
    reference: Option<&'a str>,
    reason: Option<&'a str>,
    notes: Vec<String>,
    counts: RollupCounts,
    nodes: Vec<NodeJson<'a>>,
    truncated: bool,
    omitted: Option<usize>,
}

#[derive(Serialize)]
struct NodeJson<'a> {
    id: &'a str,
    title: Option<&'a str>,
    path: &'a str,
    rollup: Rollup,
    counts: RollupCounts,
    cards: Vec<CardMarkJson<'a>>,
    uses: Vec<UseJson<'a>>,
}

/// A card with its mark: `spec trace`'s row, and the head of `spec impact`'s.
#[derive(Serialize)]
pub(crate) struct CardMarkJson<'a> {
    pub id: &'a str,
    pub flow: &'a str,
    pub column: &'a str,
    pub title: &'a str,
    pub mark: &'static str,
    pub on: Option<&'a str>,
    pub discrepancy: Option<&'a str>,
    pub correction: Option<&'a str>,
}

impl<'a> CardMarkJson<'a> {
    pub(crate) fn of(
        id: &'a str,
        flow: &'a str,
        column: &'a str,
        title: &'a str,
        mark: &'a CardMark,
    ) -> Self {
        Self {
            id,
            flow,
            column,
            title,
            mark: mark.word(),
            on: mark.on(),
            discrepancy: mark.discrepancy(),
            correction: mark.correction(),
        }
    }
}

#[derive(Serialize)]
struct UseJson<'a> {
    id: &'a str,
    rollup: Rollup,
    cards: usize,
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for TraceOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let omitted = self.omitted();
        TraceJson {
            reference: self.reference.as_deref(),
            reason: self.reason.as_deref(),
            notes: notes(&self.messages),
            counts: self.counts,
            nodes: self
                .shown_nodes()
                .iter()
                .enumerate()
                .map(|(at, node)| (node, self.shown_rows(node, at)))
                .map(|(node, shown)| NodeJson {
                    id: &node.id,
                    title: node.title.as_deref(),
                    path: &node.path,
                    rollup: node.rollup,
                    counts: node.counts,
                    cards: node
                        .cards
                        .iter()
                        .take(shown.cards)
                        .map(|card| {
                            CardMarkJson::of(
                                &card.id,
                                &card.flow,
                                &card.column,
                                &card.title,
                                &card.mark,
                            )
                        })
                        .collect(),
                    uses: node
                        .uses
                        .iter()
                        .take(shown.uses)
                        .map(|using| UseJson {
                            id: &using.id,
                            rollup: using.rollup,
                            cards: using.cards,
                        })
                        .collect(),
                })
                .collect(),
            truncated: omitted > 0,
            omitted: (omitted > 0).then_some(omitted),
        }
        .serialize(serializer)
    }
}
