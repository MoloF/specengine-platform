//! The flow read of one corpus (task spec `flow-read`): what `spec flow`
//! prints, over the spec graph and `[flow]`. Pure: the graph and the config
//! in, the rows out; it names no kind, class, path or key of any project,
//! and what it finds is no finding: a gap changes no exit, status or gate.
//!
//! - **Scope.** `ROOT`'s documents and every document within them by
//!   `parent:`; no `ROOT`: every live document holding or receiving a flow
//!   link or selected by a `[flow]` rule, with its `parent:` ancestors.
//!   Documents only: a section lands on its document. A file the live rule
//!   leaves out stays out, counted, but a `ROOT`'s own.
//! - **Links.** Every resolved link of [`FLOW_LINK_TYPES`], in reading
//!   direction (`from`, `to`), written in an admitted file and landing in
//!   one. Those with both ends in the scope are its edges.
//! - **Steps.** `[[flow.step]]`'s, every document holding or receiving
//!   `follows`, the starts and the ends. A `follows` that does not resolve
//!   (dangling, another project's) gives no edge, and its live document
//!   stays a step, so a typo never drops one from the read. **Starts**: the
//!   marked ones in the scope, else the steps no other step of the scope
//!   leads to.
//! - **Depth** folds a document deeper than N into its ancestor at N: an
//!   edge with both ends in one shown node joins its `inner`, any other is
//!   merged by (`from`, `to`, type, kind) with its count. A document's edge
//!   to itself stays an edge while the document is shown. A shown node's
//!   `distance` is the least of what it holds, itself included, so a screen
//!   orders the top level without reading the whole flow; `start` and `end`
//!   are true when it or anything it holds is one, so the overview marks
//!   where a flow begins and ends. Its `stage` stays its own. Its `gaps`
//!   counts the documents this read's rows name among what it holds, itself
//!   included, each once: rows before any fold or cap, so a depth never
//!   moves the sum over a read's nodes.
//! - **Gaps.** All ten, always in [`GapCode::ALL`]'s order, computed before
//!   any fold, so a depth never changes one. A `follows` edge crossing the
//!   scope is an entry or a way out, so a sub-flow is not called broken;
//!   `unjoined-parallel` walks every admitted document, so branches joining
//!   outside the scope are joined; `definition-*` and `supply-unfed` read
//!   every admitted document for the links, the scope for what they list.
//!   A row needing a table with no rule is [`GapState::NotConfigured`], a
//!   table that cannot be read [`GapState::Cannot`]: neither is ever 0.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use specengine_model::{Direction, EdgeKind, FLOW_LINK_TYPES, FOLLOWS, SUPPLIES, USES};

use crate::check::{Endpoint, LinkState, NodeAt, SpecGraph, Standing, documents_selected};
use crate::flow_toml::{FlowConfig, FlowTable};

/// The gap rows, in the order they print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GapCode {
    NoStart,
    TwoStarts,
    Unreachable,
    NoWayToEnd,
    TrapCycle,
    UnjoinedParallel,
    CycleNoRepeat,
    DefinitionUnused,
    DefinitionOnce,
    SupplyUnfed,
}

impl GapCode {
    /// Every row, in table order.
    pub const ALL: [Self; 10] = [
        Self::NoStart,
        Self::TwoStarts,
        Self::Unreachable,
        Self::NoWayToEnd,
        Self::TrapCycle,
        Self::UnjoinedParallel,
        Self::CycleNoRepeat,
        Self::DefinitionUnused,
        Self::DefinitionOnce,
        Self::SupplyUnfed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoStart => "no-start",
            Self::TwoStarts => "two-starts",
            Self::Unreachable => "unreachable",
            Self::NoWayToEnd => "no-way-to-end",
            Self::TrapCycle => "trap-cycle",
            Self::UnjoinedParallel => "unjoined-parallel",
            Self::CycleNoRepeat => "cycle-no-repeat",
            Self::DefinitionUnused => "definition-unused",
            Self::DefinitionOnce => "definition-once",
            Self::SupplyUnfed => "supply-unfed",
        }
    }
}

/// How a row fared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GapState {
    /// Counted: 0 means none.
    Measured,
    /// It needs a table with no rule.
    NotConfigured,
    /// It could not be counted.
    Cannot,
}

impl GapState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::NotConfigured => "not_configured",
            Self::Cannot => "cannot",
        }
    }
}

/// A table a row can need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlowNeed {
    Start,
    End,
    Definition,
}

/// What a row's `cause` is about: why it was not counted, or, for a row that
/// found something, the code whose remedy the CLI words (D11).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GapCause {
    /// The table holds no rule.
    NoRule(FlowNeed),
    /// `<config>:<line>: <message>`: the table does not read.
    Config(String),
    /// `no-start` fired: there is nothing to walk from.
    NoStart,
    /// The row reads above 0.
    Found(GapCode),
}

/// One gap row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowGap {
    pub code: GapCode,
    pub state: GapState,
    /// The items; `None` unless [`GapState::Measured`].
    pub count: Option<usize>,
    /// By name, each item's members by name.
    pub items: Vec<Vec<String>>,
    /// The files its items name, ascending; `no-start`'s scope is no
    /// document, so that row holds none.
    pub files: Vec<usize>,
    /// Set on a row not counted and on one above 0; `None` on a row at 0.
    pub cause: Option<GapCause>,
}

/// A shown node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowNode {
    /// The file it stands for, an index into the graph's paths.
    pub file: usize,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// Links of `follows` from the starts, the least of what it holds,
    /// itself included; `None`: nothing it holds is reached.
    pub distance: Option<usize>,
    /// The name of the shown node holding it.
    pub group: Option<String>,
    /// Its own `stage:`, as written; a fold keeps what it holds out of it.
    pub stage: Option<String>,
    /// It or something it holds is a source of the walk.
    pub start: bool,
    /// It or something it holds is an end.
    pub end: bool,
    /// The documents folded into it.
    pub holds: usize,
    /// The edges with both ends in it, per type, by type.
    pub inner: Vec<FlowInner>,
    /// The documents this read's gap rows name among what it holds, itself
    /// included, each once.
    pub gaps: usize,
}

/// An `inner` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowInner {
    pub link_type: &'static str,
    pub count: usize,
}

/// A shown edge, in reading direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub link_type: &'static str,
    pub kind: Option<EdgeKind>,
    /// Only at a count of 1.
    pub when: Option<String>,
    pub count: usize,
    /// Where the first of them is written.
    pub path: String,
    pub line: usize,
}

/// What a flow read is asked.
#[derive(Debug, Clone, Copy)]
pub struct FlowAsk<'r> {
    /// `ROOT`'s holders; `None`: every flow document.
    pub roots: Option<&'r [NodeAt]>,
    pub depth: Option<usize>,
    pub config: &'r FlowConfig,
    /// How a cause names the config.
    pub config_label: &'r str,
    /// How `no-start` names the scope.
    pub scope_name: &'r str,
}

/// What a flow read found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowRead {
    /// The left-out files met, each once, by file.
    pub left_out: Vec<usize>,
    /// All ten, in [`GapCode::ALL`]'s order.
    pub gaps: Vec<FlowGap>,
    /// By (distance, unreached last; path).
    pub nodes: Vec<FlowNode>,
    /// By (`from`, `to`, type, kind).
    pub edges: Vec<FlowEdge>,
}

/// What a walk found, by file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowWalk {
    /// The steps of the scope.
    pub steps: BTreeSet<usize>,
    /// Each step reached and its links of `follows` from the starts.
    pub distance: BTreeMap<usize, usize>,
}

/// Successors, or predecessors, of a file.
type Adjacency = BTreeMap<usize, BTreeSet<usize>>;

/// One flow link between admitted documents.
struct Link<'a> {
    link_type: &'static str,
    /// Reading direction.
    from: usize,
    to: usize,
    kind: Option<EdgeKind>,
    when: Option<&'a str>,
    /// The file it is written in.
    file: usize,
    line: usize,
}

/// A flow link with an end the read does not hold: left out by the live
/// rule, or none, for a live document's `follows` that does not resolve.
struct Stray {
    link_type: &'static str,
    /// The file holding it.
    source: usize,
    target: Option<usize>,
}

/// What each table selects.
struct Selected {
    step: BTreeSet<usize>,
    start: BTreeSet<usize>,
    end: BTreeSet<usize>,
    definition: BTreeSet<usize>,
}

impl Selected {
    fn of(graph: &SpecGraph<'_>, config: &FlowConfig) -> Self {
        Self {
            step: documents_selected(graph, &config.step.rules),
            start: documents_selected(graph, &config.start.rules),
            end: documents_selected(graph, &config.end.rules),
            definition: documents_selected(graph, &config.definition.rules),
        }
    }

    fn all(&self) -> impl Iterator<Item = usize> + '_ {
        self.step
            .iter()
            .chain(&self.start)
            .chain(&self.end)
            .chain(&self.definition)
            .copied()
    }
}

/// The documents read.
struct Scope {
    members: BTreeSet<usize>,
    /// Depth 0.
    roots: Vec<usize>,
    left_out: BTreeSet<usize>,
}

/// The containment of the scope.
struct Tree {
    /// A member's parent, itself a member.
    parent: BTreeMap<usize, usize>,
    /// Roots at 0.
    level: BTreeMap<usize, usize>,
    /// The member each was reached from.
    up: BTreeMap<usize, usize>,
}

/// The `follows` graph of the scope.
struct Steps {
    steps: BTreeSet<usize>,
    /// The marked ones in the scope.
    marked: BTreeSet<usize>,
    /// The walk's sources: `marked`, else the steps nothing leads to.
    starts: BTreeSet<usize>,
    ends: BTreeSet<usize>,
    next: Adjacency,
    prev: Adjacency,
    /// Targets of `follows` edges from outside the scope.
    entering: BTreeSet<usize>,
    /// Sources of `follows` edges to outside the scope.
    leaving: BTreeSet<usize>,
    distance: BTreeMap<usize, usize>,
}

/// Reads the flow of `graph` (see the module documentation).
pub fn flow_read(graph: &SpecGraph<'_>, ask: &FlowAsk<'_>) -> FlowRead {
    let asked: BTreeSet<usize> = ask.roots.into_iter().flatten().map(|at| at.file).collect();
    let admitted = |file: usize| graph.standing(file) == Standing::Live || asked.contains(&file);
    let (links, strays) = read_links(graph, &admitted);
    let selected = Selected::of(graph, ask.config);
    let parents = parents_of(graph);
    let scope = scope_of(
        graph, ask.roots, &links, &strays, &selected, &admitted, &parents,
    );
    let steps = steps_of(&scope.members, &links, &strays, &selected);
    let gaps = gaps_of(graph, ask, &scope.members, &links, &steps, &selected);
    let named: BTreeSet<usize> = gaps.iter().flat_map(|gap| gap.files.clone()).collect();
    let tree = tree_of(&parents, &scope);
    let (nodes, edges) = shown(graph, ask.depth, &scope, &tree, &links, &steps, &named);
    FlowRead {
        left_out: scope.left_out.into_iter().collect(),
        gaps,
        nodes,
        edges,
    }
}

/// The walk of the read over `roots` (`None`: every flow document), before
/// any fold, with none of [`flow_read`]'s rows computed.
pub fn flow_walk(graph: &SpecGraph<'_>, roots: Option<&[NodeAt]>, config: &FlowConfig) -> FlowWalk {
    let asked: BTreeSet<usize> = roots.into_iter().flatten().map(|at| at.file).collect();
    let admitted = |file: usize| graph.standing(file) == Standing::Live || asked.contains(&file);
    let (links, strays) = read_links(graph, &admitted);
    let selected = Selected::of(graph, config);
    let parents = parents_of(graph);
    let scope = scope_of(
        graph, roots, &links, &strays, &selected, &admitted, &parents,
    );
    let steps = steps_of(&scope.members, &links, &strays, &selected);
    FlowWalk {
        steps: steps.steps,
        distance: steps.distance,
    }
}

/// The `stage:` of `file`'s document, a section's too, as written.
pub fn stage_of<'g>(graph: &SpecGraph<'g>, file: usize) -> Option<&'g str> {
    let fields = graph.node(document(file))?.fields.as_ref()?;
    fields.stage.as_deref()
}

/// The document of `file`.
fn document(file: usize) -> NodeAt {
    NodeAt { file, ord: 0 }
}

/// Every resolved flow link: those between admitted documents, in written
/// order, and those with an end left out; and each `follows` of an admitted
/// document that does not resolve, its other end none.
fn read_links<'g>(
    graph: &'g SpecGraph<'_>,
    admitted: &impl Fn(usize) -> bool,
) -> (Vec<Link<'g>>, Vec<Stray>) {
    let mut links = Vec::new();
    let mut strays = Vec::new();
    for edge in graph.edges() {
        let Some(&(link_type, direction)) = FLOW_LINK_TYPES
            .iter()
            .find(|(name, _)| *name == edge.link_type)
        else {
            continue;
        };
        if edge.state() != LinkState::Resolved {
            if link_type == FOLLOWS && admitted(edge.file) {
                strays.push(Stray {
                    link_type,
                    source: edge.file,
                    target: None,
                });
            }
            continue;
        }
        let (Endpoint::Nodes(sources), Endpoint::Nodes(targets)) = (&edge.source, &edge.target)
        else {
            continue;
        };
        let Some(source) = sources.first().map(|at| at.file) else {
            continue;
        };
        // One holder per link: the first the read admits.
        let target = targets
            .iter()
            .map(|at| at.file)
            .find(|&file| admitted(file))
            .or_else(|| targets.first().map(|at| at.file));
        let Some(target) = target else {
            continue;
        };
        if !admitted(source) || !admitted(target) {
            strays.push(Stray {
                link_type,
                source,
                target: Some(target),
            });
            continue;
        }
        let (from, to) = match direction {
            Direction::Out => (source, target),
            Direction::In => (target, source),
        };
        let label = edge.edge.as_ref();
        links.push(Link {
            link_type,
            from,
            to,
            kind: label.and_then(|label| label.kind),
            when: label.and_then(|label| label.when.as_deref()),
            file: edge.file,
            line: edge.line,
        });
    }
    (links, strays)
}

/// The documents read: `ROOT`'s subtree, or every flow document with its
/// ancestors, climbed through `parents` ([`parents_of`]).
fn scope_of(
    graph: &SpecGraph<'_>,
    roots: Option<&[NodeAt]>,
    links: &[Link<'_>],
    strays: &[Stray],
    selected: &Selected,
    admitted: &impl Fn(usize) -> bool,
    parents: &[Option<usize>],
) -> Scope {
    let mut members = BTreeSet::new();
    let mut left_out = BTreeSet::new();
    let roots: Vec<usize> = match roots {
        Some(roots) => {
            let files: BTreeSet<usize> = roots.iter().map(|at| at.file).collect();
            let mut stack: Vec<NodeAt> = files.iter().map(|&file| document(file)).collect();
            let mut seen: BTreeSet<NodeAt> = stack.iter().copied().collect();
            members.extend(files.iter().copied());
            while let Some(at) = stack.pop() {
                for &child in graph.children(at) {
                    if !admitted(child.file) {
                        left_out.insert(child.file);
                    } else if seen.insert(child) {
                        members.insert(child.file);
                        stack.push(child);
                    }
                }
            }
            for stray in strays {
                if members.contains(&stray.source) {
                    left_out.extend(stray.target);
                }
                if stray.target.is_some_and(|target| members.contains(&target)) {
                    left_out.insert(stray.source);
                }
            }
            files.into_iter().collect()
        }
        None => {
            for link in links {
                members.extend([link.from, link.to]);
            }
            for file in selected.all() {
                if admitted(file) {
                    members.insert(file);
                } else {
                    left_out.insert(file);
                }
            }
            for stray in strays {
                for file in std::iter::once(stray.source).chain(stray.target) {
                    if !admitted(file) {
                        left_out.insert(file);
                    } else if file == stray.source {
                        members.insert(file);
                    }
                }
            }
            // Everything above an ancestor already walked is in `members` or
            // `left_out` already, so the walk stops there: each file is
            // climbed from at most once per read. A file past the map has no
            // parent document, so its walk is empty.
            let mut walked = vec![false; parents.len()];
            for member in members.clone() {
                let mut at = member;
                while let Some(done) = walked.get_mut(at) {
                    if std::mem::replace(done, true) {
                        break;
                    }
                    let Some(parent) = parent_document(parents, at) else {
                        break;
                    };
                    if !admitted(parent) {
                        left_out.insert(parent);
                        break;
                    }
                    members.insert(parent);
                    at = parent;
                }
            }
            let parented = |file: usize| {
                parent_document(parents, file).is_some_and(|parent| members.contains(&parent))
            };
            members
                .iter()
                .copied()
                .filter(|&file| !parented(file))
                .collect()
        }
    };
    Scope {
        members,
        roots,
        left_out,
    }
}

/// Each file's parent document, by file: the document it is listed under,
/// through a section of it or not. Computed once per read and read by the
/// scope's walk, `parented` and [`tree_of`], so a read stays linear in
/// `parent:` depth (`docs/canon/spec-flow.md` "Code").
fn parents_of(graph: &SpecGraph<'_>) -> Vec<Option<usize>> {
    (0..graph.paths().len())
        .map(|file| listed_outside(graph, file))
        .collect()
}

/// The file of the first node on `file`'s document's chain outside `file`.
/// The chain leaves a file once and never comes back: a `parent:` cycle is
/// cut at a document (`SpecGraph::cycles`), and a section climbs to its own
/// document. So the chain is read only up to that node, never past it.
fn listed_outside(graph: &SpecGraph<'_>, file: usize) -> Option<usize> {
    // Bounded by the file's own nodes, as `SpecGraph::ancestors` is by a
    // repeat: a chain still in `file` after them all has no way out.
    let mut within = graph.nodes(file).len();
    let mut current = graph.listed_under(document(file));
    while let Some(at) = current {
        if at.file != file {
            return Some(at.file);
        }
        within = within.checked_sub(1)?;
        current = graph.listed_under(at);
    }
    None
}

/// `file`'s parent document in `parents` ([`parents_of`]).
fn parent_document(parents: &[Option<usize>], file: usize) -> Option<usize> {
    parents.get(file).copied().flatten()
}

/// The levels of the scope, breadth-first from its roots.
fn tree_of(parents: &[Option<usize>], scope: &Scope) -> Tree {
    let parent: BTreeMap<usize, usize> = scope
        .members
        .iter()
        .filter_map(|&file| {
            parent_document(parents, file)
                .filter(|parent| scope.members.contains(parent))
                .map(|parent| (file, parent))
        })
        .collect();
    let mut kids: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (&child, &of) in &parent {
        kids.entry(of).or_default().push(child);
    }
    let mut level = BTreeMap::new();
    let mut up = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &root in &scope.roots {
        if level.insert(root, 0).is_none() {
            queue.push_back(root);
        }
    }
    // Every member hangs under a root; one that does not stands as a root.
    let mut pending = scope.members.iter();
    loop {
        while let Some(at) = queue.pop_front() {
            let below = level.get(&at).copied().unwrap_or(0) + 1;
            for &kid in kids.get(&at).into_iter().flatten() {
                if let Entry::Vacant(free) = level.entry(kid) {
                    free.insert(below);
                    up.insert(kid, at);
                    queue.push_back(kid);
                }
            }
        }
        let Some(&next) = pending.find(|file| !level.contains_key(file)) else {
            break;
        };
        level.insert(next, 0);
        queue.push_back(next);
    }
    Tree { parent, level, up }
}

/// The steps of the scope and the `follows` graph over them.
fn steps_of(
    members: &BTreeSet<usize>,
    links: &[Link<'_>],
    strays: &[Stray],
    selected: &Selected,
) -> Steps {
    let within =
        |set: &BTreeSet<usize>| -> BTreeSet<usize> { set.intersection(members).copied().collect() };
    let marked = within(&selected.start);
    let ends = within(&selected.end);
    let mut steps = within(&selected.step);
    steps.extend(marked.iter().chain(&ends).copied());
    let mut next = Adjacency::new();
    let mut prev = Adjacency::new();
    let mut entering = BTreeSet::new();
    let mut leaving = BTreeSet::new();
    for link in links.iter().filter(|link| link.link_type == FOLLOWS) {
        match (members.contains(&link.from), members.contains(&link.to)) {
            (true, true) => {
                steps.extend([link.from, link.to]);
                next.entry(link.from).or_default().insert(link.to);
                prev.entry(link.to).or_default().insert(link.from);
            }
            (true, false) => {
                steps.insert(link.from);
                leaving.insert(link.from);
            }
            (false, true) => {
                steps.insert(link.to);
                entering.insert(link.to);
            }
            (false, false) => {}
        }
    }
    for stray in strays.iter().filter(|stray| stray.link_type == FOLLOWS) {
        if members.contains(&stray.source) {
            steps.insert(stray.source);
        }
    }
    let starts = if marked.is_empty() {
        let entered = |step: &usize| {
            prev.get(step)
                .is_some_and(|sources| sources.iter().any(|source| source != step))
        };
        steps
            .iter()
            .copied()
            .filter(|step| !entered(step))
            .collect()
    } else {
        marked.clone()
    };
    let distance = distances(&starts, &next);
    Steps {
        steps,
        marked,
        starts,
        ends,
        next,
        prev,
        entering,
        leaving,
        distance,
    }
}

/// Breadth-first from `starts`.
fn distances(starts: &BTreeSet<usize>, next: &Adjacency) -> BTreeMap<usize, usize> {
    let mut distance: BTreeMap<usize, usize> = starts.iter().map(|&start| (start, 0)).collect();
    let mut queue: VecDeque<usize> = starts.iter().copied().collect();
    while let Some(at) = queue.pop_front() {
        let far = distance.get(&at).copied().unwrap_or(0) + 1;
        for &step in next.get(&at).into_iter().flatten() {
            if let Entry::Vacant(free) = distance.entry(step) {
                free.insert(far);
                queue.push_back(step);
            }
        }
    }
    distance
}

/// What `next` reaches from `sources`, themselves included.
fn reach(sources: impl IntoIterator<Item = usize>, next: &Adjacency) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<usize> = sources.into_iter().collect();
    while let Some(at) = stack.pop() {
        if !seen.insert(at) {
            continue;
        }
        stack.extend(next.get(&at).into_iter().flatten().copied());
    }
    seen
}

/// Tarjan's components of `nodes` over `next`, walked without recursion: a
/// chain as long as the corpus stays within the stack. Only the cycles:
/// two or more nodes, or one with an edge to itself.
fn cycles(nodes: &BTreeSet<usize>, next: &Adjacency) -> Vec<Vec<usize>> {
    let empty = BTreeSet::new();
    let edges = |node: usize| next.get(&node).unwrap_or(&empty).iter();
    let mut index: BTreeMap<usize, usize> = BTreeMap::new();
    let mut low: BTreeMap<usize, usize> = BTreeMap::new();
    let mut held: Vec<usize> = Vec::new();
    let mut on_stack: BTreeSet<usize> = BTreeSet::new();
    let mut found = Vec::new();
    for &root in nodes {
        if index.contains_key(&root) {
            continue;
        }
        index.insert(root, index.len());
        low.insert(root, index[&root]);
        held.push(root);
        on_stack.insert(root);
        let mut work = vec![(root, edges(root))];
        while let Some((node, pending)) = work.last_mut() {
            let node = *node;
            if let Some(&far) = pending.next() {
                if !nodes.contains(&far) {
                    continue;
                }
                match index.get(&far).copied() {
                    None => {
                        index.insert(far, index.len());
                        low.insert(far, index[&far]);
                        held.push(far);
                        on_stack.insert(far);
                        work.push((far, edges(far)));
                    }
                    Some(seen) if on_stack.contains(&far) => {
                        let lowest = low[&node].min(seen);
                        low.insert(node, lowest);
                    }
                    Some(_) => {}
                }
                continue;
            }
            work.pop();
            if let Some((above, _)) = work.last() {
                let lowest = low[above].min(low[&node]);
                low.insert(*above, lowest);
            }
            if low[&node] != index[&node] {
                continue;
            }
            let mut component = Vec::new();
            while let Some(member) = held.pop() {
                on_stack.remove(&member);
                component.push(member);
                if member == node {
                    break;
                }
            }
            let looped = next.get(&node).is_some_and(|far| far.contains(&node));
            if component.len() > 1 || looped {
                found.push(component);
            }
        }
    }
    found
}

/// A counted row of documents: named and sorted, each item's members too;
/// above 0 it carries [`GapCause::Found`].
fn measured(graph: &SpecGraph<'_>, code: GapCode, items: &[Vec<usize>]) -> FlowGap {
    let files: BTreeSet<usize> = items.iter().flatten().copied().collect();
    let mut named: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            let mut members: Vec<String> = item
                .iter()
                .map(|&file| graph.name(document(file)))
                .collect();
            members.sort();
            members
        })
        .collect();
    named.sort();
    FlowGap {
        code,
        state: GapState::Measured,
        count: Some(named.len()),
        items: named,
        files: files.into_iter().collect(),
        cause: (!items.is_empty()).then_some(GapCause::Found(code)),
    }
}

/// A row with no count.
fn unmeasured(code: GapCode, state: GapState, cause: GapCause) -> FlowGap {
    FlowGap {
        code,
        state,
        count: None,
        items: Vec::new(),
        files: Vec::new(),
        cause: Some(cause),
    }
}

/// The row `code` when `table` stops it: unreadable, or, for a `need`, with
/// no rule.
fn stopped(
    code: GapCode,
    table: &FlowTable,
    need: Option<FlowNeed>,
    label: &str,
) -> Option<FlowGap> {
    if let Some(error) = &table.error {
        let cause = GapCause::Config(error.at(label));
        return Some(unmeasured(code, GapState::Cannot, cause));
    }
    match need {
        Some(need) if table.rules.is_empty() => Some(unmeasured(
            code,
            GapState::NotConfigured,
            GapCause::NoRule(need),
        )),
        _ => None,
    }
}

/// The ten rows.
fn gaps_of(
    graph: &SpecGraph<'_>,
    ask: &FlowAsk<'_>,
    members: &BTreeSet<usize>,
    links: &[Link<'_>],
    steps: &Steps,
    selected: &Selected,
) -> Vec<FlowGap> {
    let config = ask.config;
    let label = ask.config_label;
    let start = |code| stopped(code, &config.start, Some(FlowNeed::Start), label);
    let end = |code| stopped(code, &config.end, Some(FlowNeed::End), label);
    let definition = |code| stopped(code, &config.definition, Some(FlowNeed::Definition), label);
    let step = |code| stopped(code, &config.step, None, label);

    let headless = steps.marked.is_empty() && steps.entering.is_empty();
    let no_start = start(GapCode::NoStart).unwrap_or_else(|| {
        let mut row = measured(graph, GapCode::NoStart, &[]);
        if headless {
            row.items = vec![vec![ask.scope_name.to_owned()]];
            row.count = Some(1);
            row.cause = Some(GapCause::Found(GapCode::NoStart));
        }
        row
    });
    let two_starts = start(GapCode::TwoStarts).unwrap_or_else(|| {
        let items = if steps.marked.len() >= 2 {
            vec![steps.marked.iter().copied().collect()]
        } else {
            Vec::new()
        };
        measured(graph, GapCode::TwoStarts, &items)
    });
    let unreachable = start(GapCode::Unreachable)
        .or_else(|| step(GapCode::Unreachable))
        .unwrap_or_else(|| {
            if headless {
                return unmeasured(GapCode::Unreachable, GapState::Cannot, GapCause::NoStart);
            }
            let sources = steps.marked.iter().chain(&steps.entering).copied();
            let reached = reach(sources, &steps.next);
            let items = one_each(steps.steps.difference(&reached).copied());
            measured(graph, GapCode::Unreachable, &items)
        });

    let finishing = reach(
        steps.ends.iter().chain(&steps.leaving).copied(),
        &steps.prev,
    );
    let traps: Vec<Vec<usize>> = cycles(&steps.steps, &steps.next)
        .into_iter()
        .filter(|cycle| !cycle.iter().any(|member| finishing.contains(member)))
        .collect();
    let trapped: BTreeSet<usize> = traps.iter().flatten().copied().collect();
    let no_way = end(GapCode::NoWayToEnd)
        .or_else(|| step(GapCode::NoWayToEnd))
        .unwrap_or_else(|| {
            let stuck = steps
                .steps
                .iter()
                .copied()
                .filter(|step| !finishing.contains(step) && !trapped.contains(step));
            measured(graph, GapCode::NoWayToEnd, &one_each(stuck))
        });
    let trap_cycle =
        end(GapCode::TrapCycle).unwrap_or_else(|| measured(graph, GapCode::TrapCycle, &traps));

    let unjoined = unjoined_splits(members, links, &follows_forward(links));
    let loose = cycles(&steps.steps, &without_repeats(members, links));
    let users = users_of(links);
    let used_by = |wanted: usize| {
        let defined = selected.definition.intersection(members).copied();
        let matching = defined.filter(|file| users.get(file).map_or(0, BTreeSet::len) == wanted);
        one_each(matching)
    };
    vec![
        no_start,
        two_starts,
        unreachable,
        no_way,
        trap_cycle,
        measured(graph, GapCode::UnjoinedParallel, &one_each(unjoined)),
        measured(graph, GapCode::CycleNoRepeat, &loose),
        definition(GapCode::DefinitionUnused)
            .unwrap_or_else(|| measured(graph, GapCode::DefinitionUnused, &used_by(0))),
        definition(GapCode::DefinitionOnce)
            .unwrap_or_else(|| measured(graph, GapCode::DefinitionOnce, &used_by(1))),
        measured(graph, GapCode::SupplyUnfed, &unfed(members, links)),
    ]
}

/// One item per file.
fn one_each(files: impl IntoIterator<Item = usize>) -> Vec<Vec<usize>> {
    files.into_iter().map(|file| vec![file]).collect()
}

/// Every admitted `follows` link, forward.
fn follows_forward(links: &[Link<'_>]) -> Adjacency {
    let mut next = Adjacency::new();
    for link in links.iter().filter(|link| link.link_type == FOLLOWS) {
        next.entry(link.from).or_default().insert(link.to);
    }
    next
}

/// The `follows` graph of the scope with its `repeat` edges removed.
fn without_repeats(members: &BTreeSet<usize>, links: &[Link<'_>]) -> Adjacency {
    let mut next = Adjacency::new();
    for link in links.iter().filter(|link| {
        link.link_type == FOLLOWS
            && link.kind != Some(EdgeKind::Repeat)
            && members.contains(&link.from)
            && members.contains(&link.to)
    }) {
        next.entry(link.from).or_default().insert(link.to);
    }
    next
}

/// The splits of the scope whose `parallel` branches do not join: one
/// branch, or branches with no step in common, each walk stopping at the
/// split.
fn unjoined_splits(
    members: &BTreeSet<usize>,
    links: &[Link<'_>],
    forward: &Adjacency,
) -> Vec<usize> {
    let mut splits = Adjacency::new();
    for link in links.iter().filter(|link| {
        link.link_type == FOLLOWS
            && link.kind == Some(EdgeKind::Parallel)
            && members.contains(&link.from)
    }) {
        splits.entry(link.from).or_default().insert(link.to);
    }
    splits
        .iter()
        .filter(|&(&split, branches)| branches.len() < 2 || !joined(split, branches, forward))
        .map(|(&split, _)| split)
        .collect()
}

/// `branches` share a later step: one breadth-first walk per branch, each
/// taking one step in turn and none entering `split`, until a step all of
/// them reached, or none has a step left. A branch counts as its own later
/// step. The cost is what lies before the join, not the rest of the flow.
fn joined(split: usize, branches: &BTreeSet<usize>, forward: &Adjacency) -> bool {
    let mut walks: Vec<(BTreeSet<usize>, VecDeque<usize>)> = branches
        .iter()
        .map(|&branch| (BTreeSet::new(), VecDeque::from([branch])))
        .collect();
    let mut reached: BTreeMap<usize, usize> = BTreeMap::new();
    let mut walking = true;
    while walking {
        walking = false;
        for (seen, queue) in &mut walks {
            let Some(at) = queue.pop_front() else {
                continue;
            };
            walking = true;
            if at == split || !seen.insert(at) {
                continue;
            }
            let walks_here = reached.entry(at).or_default();
            *walks_here += 1;
            if *walks_here == branches.len() {
                return true;
            }
            queue.extend(forward.get(&at).into_iter().flatten().copied());
        }
    }
    false
}

/// Each `uses` target and the documents using it.
fn users_of(links: &[Link<'_>]) -> Adjacency {
    let mut users = Adjacency::new();
    for link in links.iter().filter(|link| link.link_type == USES) {
        users.entry(link.to).or_default().insert(link.from);
    }
    users
}

/// The `supplies` cycles touching the scope that no node outside feeds.
fn unfed(members: &BTreeSet<usize>, links: &[Link<'_>]) -> Vec<Vec<usize>> {
    let supplies: Vec<&Link<'_>> = links
        .iter()
        .filter(|link| link.link_type == SUPPLIES)
        .collect();
    let mut feeds = Adjacency::new();
    let mut nodes = BTreeSet::new();
    for link in &supplies {
        feeds.entry(link.from).or_default().insert(link.to);
        nodes.extend([link.from, link.to]);
    }
    let unfed = |cycle: &Vec<usize>| {
        let inside: BTreeSet<usize> = cycle.iter().copied().collect();
        !supplies
            .iter()
            .any(|link| inside.contains(&link.to) && !inside.contains(&link.from))
    };
    cycles(&nodes, &feeds)
        .into_iter()
        .filter(|cycle| cycle.iter().any(|member| members.contains(member)))
        .filter(unfed)
        .collect()
}

/// The shown nodes and edges at `depth`.
fn shown(
    graph: &SpecGraph<'_>,
    depth: Option<usize>,
    scope: &Scope,
    tree: &Tree,
    links: &[Link<'_>],
    steps: &Steps,
    named: &BTreeSet<usize>,
) -> (Vec<FlowNode>, Vec<FlowEdge>) {
    let fold = folded(tree, depth);
    let shown_as = |file: usize| -> usize { fold.get(&file).copied().unwrap_or(file) };
    let mut holds: BTreeMap<usize, usize> = BTreeMap::new();
    let mut least: BTreeMap<usize, usize> = BTreeMap::new();
    let mut gaps: BTreeMap<usize, usize> = BTreeMap::new();
    let mut starting = BTreeSet::new();
    let mut ending = BTreeSet::new();
    for &member in &scope.members {
        let shown = shown_as(member);
        if shown != member {
            *holds.entry(shown).or_default() += 1;
        }
        if named.contains(&member) {
            *gaps.entry(shown).or_default() += 1;
        }
        if let Some(&distance) = steps.distance.get(&member) {
            least
                .entry(shown)
                .and_modify(|lowest| *lowest = distance.min(*lowest))
                .or_insert(distance);
        }
        if steps.starts.contains(&member) {
            starting.insert(shown);
        }
        if steps.ends.contains(&member) {
            ending.insert(shown);
        }
    }
    let mut inner: BTreeMap<usize, BTreeMap<&'static str, usize>> = BTreeMap::new();
    let mut merged: BTreeMap<EdgeKey, Merged<'_>> = BTreeMap::new();
    let name = |file: usize| graph.name(document(file));
    for link in links
        .iter()
        .filter(|link| scope.members.contains(&link.from) && scope.members.contains(&link.to))
    {
        let (from, to) = (shown_as(link.from), shown_as(link.to));
        let own_loop = link.from == link.to && link.from == from;
        if from == to && !own_loop {
            *inner
                .entry(from)
                .or_default()
                .entry(link.link_type)
                .or_default() += 1;
            continue;
        }
        let key = EdgeKey {
            from_name: name(from),
            to_name: name(to),
            link_type: link.link_type,
            kind: link.kind,
            from,
            to,
        };
        merged
            .entry(key)
            .and_modify(|edge| edge.count += 1)
            .or_insert(Merged {
                count: 1,
                when: link.when,
                file: link.file,
                line: link.line,
            });
    }
    let mut nodes: Vec<FlowNode> = scope
        .members
        .iter()
        .copied()
        .filter(|&member| shown_as(member) == member)
        .map(|file| {
            let at = document(file);
            let node = graph.node(at);
            FlowNode {
                file,
                id: node.and_then(|node| node.id.clone()),
                kind: node.and_then(|node| node.kind.clone()),
                title: node.and_then(|node| node.title.clone()),
                path: graph.paths()[file].to_owned(),
                line: graph.line(at),
                distance: least.get(&file).copied(),
                group: tree.parent.get(&file).map(|&parent| name(parent)),
                stage: stage_of(graph, file).map(str::to_owned),
                start: starting.contains(&file),
                end: ending.contains(&file),
                holds: holds.get(&file).copied().unwrap_or(0),
                inner: inner
                    .get(&file)
                    .into_iter()
                    .flatten()
                    .map(|(&link_type, &count)| FlowInner { link_type, count })
                    .collect(),
                gaps: gaps.get(&file).copied().unwrap_or(0),
            }
        })
        .collect();
    nodes.sort_by(|a, b| {
        (a.distance.is_none(), a.distance, &a.path).cmp(&(
            b.distance.is_none(),
            b.distance,
            &b.path,
        ))
    });
    let edges = merged
        .into_iter()
        .map(|(key, edge)| FlowEdge {
            from: key.from_name,
            to: key.to_name,
            link_type: key.link_type,
            kind: key.kind,
            when: edge.when.filter(|_| edge.count == 1).map(str::to_owned),
            count: edge.count,
            path: graph.paths()[edge.file].to_owned(),
            line: edge.line,
        })
        .collect();
    (nodes, edges)
}

/// Each member's shown node at `depth`: itself at a level of at most
/// `depth` or with no `up`, else its `up`'s shown node. Filled in level
/// order, so an `up` is filled before what hangs under it and no member
/// climbs more than once per read. Empty with no depth: a member missing
/// from it shows as itself.
fn folded(tree: &Tree, depth: Option<usize>) -> BTreeMap<usize, usize> {
    let mut shown = BTreeMap::new();
    let Some(depth) = depth else {
        return shown;
    };
    let mut order: Vec<(usize, usize)> = tree
        .level
        .iter()
        .map(|(&file, &level)| (level, file))
        .collect();
    order.sort_unstable();
    for (level, file) in order {
        let above = if level > depth {
            tree.up.get(&file)
        } else {
            None
        };
        let at = above.map_or(file, |above| shown.get(above).copied().unwrap_or(*above));
        shown.insert(file, at);
    }
    shown
}

/// An edge's merge key, in print order; the files tell two holders of one
/// name apart.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    from_name: String,
    to_name: String,
    link_type: &'static str,
    kind: Option<EdgeKind>,
    from: usize,
    to: usize,
}

/// What a merged edge keeps of its links: the first written.
struct Merged<'a> {
    count: usize,
    when: Option<&'a str>,
    file: usize,
    line: usize,
}

/// docs/features/flow-linear.md AC-10 "The forest differential": today's
/// `scope_of`, `parent_of`, `tree_of` and `shown_as` (the parent of this
/// slice, `e863186`) kept verbatim as the reference, and 300 seeded forests
/// read by both. Non-Latin characters are Unicode escapes (ADR-0024).
#[cfg(test)]
mod forest_differential {
    use std::collections::btree_map::Entry;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    use specengine_model::IdScheme;

    use super::{
        Link, NodeAt, Scope, Selected, SpecGraph, Standing, Stray, Tree, document, folded,
        parents_of, read_links, scope_of, tree_of,
    };
    // A parse of bytes, named apart from the source scan's word for file
    // access (core does no file I/O; `specengine-eval` `build_graph.rs`).
    use crate::check::{CheckFile as Parsed, CheckInput};
    use crate::flow_toml::FlowConfig;
    use crate::{IdSchemeToml as _, Paths};

    /// The read before this slice, verbatim but for the closure `shown_as`
    /// (`shown`), here a function of the tree and the depth.
    mod reference {
        use super::*;

        /// The documents read: `ROOT`'s subtree, or every flow document with its
        /// ancestors.
        pub(super) fn scope_of(
            graph: &SpecGraph<'_>,
            roots: Option<&[NodeAt]>,
            links: &[Link<'_>],
            strays: &[Stray],
            selected: &Selected,
            admitted: &impl Fn(usize) -> bool,
        ) -> Scope {
            let mut members = BTreeSet::new();
            let mut left_out = BTreeSet::new();
            let roots: Vec<usize> = match roots {
                Some(roots) => {
                    let files: BTreeSet<usize> = roots.iter().map(|at| at.file).collect();
                    let mut stack: Vec<NodeAt> = files.iter().map(|&file| document(file)).collect();
                    let mut seen: BTreeSet<NodeAt> = stack.iter().copied().collect();
                    members.extend(files.iter().copied());
                    while let Some(at) = stack.pop() {
                        for &child in graph.children(at) {
                            if !admitted(child.file) {
                                left_out.insert(child.file);
                            } else if seen.insert(child) {
                                members.insert(child.file);
                                stack.push(child);
                            }
                        }
                    }
                    for stray in strays {
                        if members.contains(&stray.source) {
                            left_out.extend(stray.target);
                        }
                        if stray.target.is_some_and(|target| members.contains(&target)) {
                            left_out.insert(stray.source);
                        }
                    }
                    files.into_iter().collect()
                }
                None => {
                    for link in links {
                        members.extend([link.from, link.to]);
                    }
                    for file in selected.all() {
                        if admitted(file) {
                            members.insert(file);
                        } else {
                            left_out.insert(file);
                        }
                    }
                    for stray in strays {
                        for file in std::iter::once(stray.source).chain(stray.target) {
                            if !admitted(file) {
                                left_out.insert(file);
                            } else if file == stray.source {
                                members.insert(file);
                            }
                        }
                    }
                    for member in members.clone() {
                        for at in graph.ancestors(document(member)) {
                            if at.file == member {
                                continue;
                            }
                            if !admitted(at.file) {
                                left_out.insert(at.file);
                                break;
                            }
                            members.insert(at.file);
                        }
                    }
                    let parented = |file: usize| {
                        parent_of(graph, file).is_some_and(|parent| members.contains(&parent))
                    };
                    members
                        .iter()
                        .copied()
                        .filter(|&file| !parented(file))
                        .collect()
                }
            };
            Scope {
                members,
                roots,
                left_out,
            }
        }

        /// The document `file` is listed under, through a section of it or not.
        pub(super) fn parent_of(graph: &SpecGraph<'_>, file: usize) -> Option<usize> {
            graph
                .ancestors(document(file))
                .into_iter()
                .map(|at| at.file)
                .find(|&other| other != file)
        }

        /// The levels of the scope, breadth-first from its roots.
        pub(super) fn tree_of(graph: &SpecGraph<'_>, scope: &Scope) -> Tree {
            let parent: BTreeMap<usize, usize> = scope
                .members
                .iter()
                .filter_map(|&file| {
                    parent_of(graph, file)
                        .filter(|parent| scope.members.contains(parent))
                        .map(|parent| (file, parent))
                })
                .collect();
            let mut kids: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (&child, &of) in &parent {
                kids.entry(of).or_default().push(child);
            }
            let mut level = BTreeMap::new();
            let mut up = BTreeMap::new();
            let mut queue = VecDeque::new();
            for &root in &scope.roots {
                if level.insert(root, 0).is_none() {
                    queue.push_back(root);
                }
            }
            // Every member hangs under a root; one that does not stands as a root.
            let mut pending = scope.members.iter();
            loop {
                while let Some(at) = queue.pop_front() {
                    let below = level.get(&at).copied().unwrap_or(0) + 1;
                    for &kid in kids.get(&at).into_iter().flatten() {
                        if let Entry::Vacant(free) = level.entry(kid) {
                            free.insert(below);
                            up.insert(kid, at);
                            queue.push_back(kid);
                        }
                    }
                }
                let Some(&next) = pending.find(|file| !level.contains_key(file)) else {
                    break;
                };
                level.insert(next, 0);
                queue.push_back(next);
            }
            Tree { parent, level, up }
        }

        /// `shown`'s closure `shown_as`, its body verbatim.
        pub(super) fn shown_as(tree: &Tree, depth: Option<usize>, file: usize) -> usize {
            let Some(depth) = depth else {
                return file;
            };
            let mut at = file;
            while tree.level.get(&at).is_some_and(|&level| level > depth) {
                match tree.up.get(&at) {
                    Some(&above) => at = above,
                    None => break,
                }
            }
            at
        }
    }

    /// The forests' config: three prefixes, the `stage` kind a flow step.
    const CONFIG: &str = "[project]\nslug = \"forest\"\n\n[ids]\n\
                          DOC = { kind = \"group\", width = 2 }\n\
                          STP = { kind = \"stage\", width = 2 }\n\
                          SEC = { kind = \"part\", width = 2 }\n\n\
                          [[flow.step]]\nkinds = [\"stage\"]\n";

    /// Seeded forests read.
    const FORESTS: u64 = 300;

    /// A forest's documents at most, and its chains' depth at most.
    const MOST_DOCUMENTS: usize = 60;
    const MOST_DEPTH: usize = 30;

    struct Lcg(u64);

    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(
                seed.wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407),
            )
        }

        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }

        fn below(&mut self, n: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(n.max(1)).expect("small")).expect("small")
        }
    }

    /// How a document stands.
    #[derive(Clone, Copy, PartialEq)]
    enum Class {
        Live,
        Generated,
        Tier3,
    }

    /// One document of a forest.
    struct Doc {
        step: bool,
        class: Class,
        section: bool,
        /// The document its `parent:` names, and whether through its section.
        parent: Option<(usize, bool)>,
        /// Flow links: (type, target document, through its section).
        links: Vec<(&'static str, usize, bool)>,
    }

    struct Forest {
        docs: Vec<Doc>,
        /// The input order of the files: a shuffle of the documents.
        order: Vec<usize>,
        cycle: bool,
        depth: usize,
    }

    fn id(docs: &[Doc], doc: usize) -> String {
        let prefix = if docs[doc].step { "STP" } else { "DOC" };
        format!("{prefix}-{:02}", doc + 1)
    }

    fn section_id(doc: usize) -> String {
        format!("SEC-{:02}", doc + 1)
    }

    /// `doc`, or its section when `through` and it has one.
    fn named(docs: &[Doc], doc: usize, through: bool) -> String {
        if through && docs[doc].section {
            section_id(doc)
        } else {
            id(docs, doc)
        }
    }

    impl Forest {
        fn random(seed: u64) -> Self {
            let mut lcg = Lcg::new(seed);
            let count = 2 + lcg.below(MOST_DOCUMENTS - 1);
            let mut docs: Vec<Doc> = Vec::new();
            let mut depth: Vec<usize> = Vec::new();
            for doc in 0..count {
                let class = match lcg.below(16) {
                    0 => Class::Generated,
                    1 => Class::Tier3,
                    _ => Class::Live,
                };
                let parent = if doc > 0 && lcg.below(8) != 0 {
                    // Chains: mostly the one before, so depth grows.
                    let above = if lcg.below(4) != 0 {
                        doc - 1
                    } else {
                        lcg.below(doc)
                    };
                    (depth[above] < MOST_DEPTH).then(|| (above, lcg.below(2) == 0))
                } else {
                    None
                };
                depth.push(parent.map_or(0, |(above, _)| depth[above] + 1));
                docs.push(Doc {
                    step: lcg.below(5) == 0,
                    class,
                    section: lcg.below(3) == 0,
                    parent,
                    links: Vec::new(),
                });
            }
            // Flow links on a random subset.
            for doc in &mut docs {
                if lcg.below(4) == 0 {
                    let link_type = if lcg.below(3) == 0 { "uses" } else { "follows" };
                    let to = lcg.below(count);
                    let through = lcg.below(4) == 0;
                    doc.links.push((link_type, to, through));
                }
            }
            // A cycle planted: a chain's top names a document below it.
            let mut cycle = false;
            if lcg.below(3) == 0 {
                let low = lcg.below(count);
                let mut top = low;
                while let Some((above, _)) = docs[top].parent {
                    top = above;
                }
                if top != low {
                    docs[top].parent = Some((low, lcg.below(2) == 0));
                    cycle = true;
                }
            }
            // Now and then a document under its own section.
            if lcg.below(12) == 0 {
                let doc = lcg.below(count);
                docs[doc].section = true;
                docs[doc].parent = Some((doc, true));
                cycle = true;
            }
            let mut order: Vec<usize> = (0..count).collect();
            for at in (1..count).rev() {
                order.swap(at, lcg.below(at + 1));
            }
            let depth = depth.into_iter().max().unwrap_or(0);
            Self {
                docs,
                order,
                cycle,
                depth,
            }
        }

        fn text(&self, doc: usize) -> String {
            let docs = &self.docs;
            let me = &docs[doc];
            let class = match me.class {
                Class::Live => "class: canon\n",
                Class::Generated => "class: generated\n",
                Class::Tier3 => "class: spec\nstatus: shipped\n",
            };
            let parent = me.parent.map_or_else(String::new, |(above, through)| {
                format!("parent: {}\n", named(docs, above, through))
            });
            let links = if me.links.is_empty() {
                String::new()
            } else {
                let mut by_type: BTreeMap<&str, Vec<String>> = BTreeMap::new();
                for &(link_type, to, through) in &me.links {
                    by_type
                        .entry(link_type)
                        .or_default()
                        .push(named(docs, to, through));
                }
                let lines: String = by_type
                    .into_iter()
                    .map(|(link_type, ids)| format!("  {link_type}: [{}]\n", ids.join(", ")))
                    .collect();
                format!("links:\n{lines}")
            };
            let section = if me.section {
                format!("\n## A part {{#{}}}\n\nA part.\n", section_id(doc))
            } else {
                String::new()
            };
            format!(
                "---\nid: {}\n{class}title: A document\n{parent}owner: owner\n\
                 reviewed: 2026-10-01\n{links}---\n\n# A document\n\nA body.\n{section}",
                id(docs, doc)
            )
        }

        /// The parsed corpus, its files in [`Forest::order`].
        fn input(&self, scheme: &IdScheme) -> CheckInput {
            CheckInput {
                files: self
                    .order
                    .iter()
                    .map(|&doc| {
                        let path = format!("forest/{}.md", id(&self.docs, doc).to_lowercase());
                        Parsed::parse(&path, self.text(doc).into_bytes(), scheme)
                    })
                    .collect(),
                problems: Vec::new(),
            }
        }
    }

    /// What a seed reached, so a generator that never plants a case is seen.
    #[derive(Default, Debug)]
    struct Reached {
        /// The whole read took in an ancestor no link or rule names.
        walked: usize,
        /// ... two levels of them on one chain: the walk goes past one parent.
        walked_twice: usize,
        /// ... and left out a non-admitted ancestor with a live one above it.
        stopped_below_live: usize,
        cycles: usize,
        through_section: usize,
        deep: usize,
        folded: usize,
    }

    /// The scope, the tree and the shown nodes of one read, both ways.
    fn compare(
        graph: &SpecGraph<'_>,
        selected: &Selected,
        parents: &[Option<usize>],
        roots: Option<&[NodeAt]>,
        context: &str,
        reached: &mut Reached,
    ) {
        let asked: BTreeSet<usize> = roots.into_iter().flatten().map(|at| at.file).collect();
        let admitted =
            |file: usize| graph.standing(file) == Standing::Live || asked.contains(&file);
        let (links, strays): (Vec<Link<'_>>, Vec<Stray>) = read_links(graph, &admitted);
        let got = scope_of(graph, roots, &links, &strays, selected, &admitted, parents);
        let want = reference::scope_of(graph, roots, &links, &strays, selected, &admitted);
        assert_eq!(got.members, want.members, "{context}: members");
        assert_eq!(got.roots, want.roots, "{context}: roots");
        assert_eq!(got.left_out, want.left_out, "{context}: left_out");
        let got_tree: Tree = tree_of(parents, &got);
        let want_tree: Tree = reference::tree_of(graph, &want);
        assert_eq!(got_tree.parent, want_tree.parent, "{context}: parent");
        assert_eq!(got_tree.level, want_tree.level, "{context}: level");
        assert_eq!(got_tree.up, want_tree.up, "{context}: up");
        for depth in [None, Some(0), Some(1), Some(2), Some(3)] {
            let fold = folded(&got_tree, depth);
            for &member in &want.members {
                let shown = fold.get(&member).copied().unwrap_or(member);
                let before = reference::shown_as(&want_tree, depth, member);
                assert_eq!(shown, before, "{context}: {member} at depth {depth:?}");
                if depth.is_some() && before != member {
                    reached.folded += 1;
                }
            }
        }
        if roots.is_none() {
            note_the_walk(graph, &links, &strays, selected, &want, reached);
        }
    }

    /// What the whole read's ancestor walk did, from the reference alone.
    fn note_the_walk(
        graph: &SpecGraph<'_>,
        links: &[Link<'_>],
        strays: &[Stray],
        selected: &Selected,
        want: &Scope,
        reached: &mut Reached,
    ) {
        let mut named: BTreeSet<usize> = selected.all().collect();
        for link in links {
            named.extend([link.from, link.to]);
        }
        named.extend(strays.iter().map(|stray| stray.source));
        let walked: BTreeSet<usize> = want.members.difference(&named).copied().collect();
        if !walked.is_empty() {
            reached.walked += 1;
        }
        let parent = |file: usize| reference::parent_of(graph, file);
        if walked
            .iter()
            .any(|&file| parent(file).is_some_and(|above| walked.contains(&above)))
        {
            reached.walked_twice += 1;
        }
        let live_above = |file: usize| {
            let mut at = parent(file);
            let mut seen = BTreeSet::from([file]);
            while let Some(above) = at {
                if !seen.insert(above) {
                    return false;
                }
                if graph.standing(above) == Standing::Live {
                    return true;
                }
                at = parent(above);
            }
            false
        };
        if want.left_out.iter().any(|&file| {
            graph.standing(file) != Standing::Live
                && want
                    .members
                    .iter()
                    .any(|&member| parent(member) == Some(file))
                && live_above(file)
        }) {
            reached.stopped_below_live += 1;
        }
    }

    /// AC-10 "The forest differential": over 300 seeded forests (up to 60
    /// documents, chains up to 30 deep, `parent:` naming a document or its
    /// ID section, one document in eight generated or Tier 3, planted
    /// cycles, flow links on a random subset), each file's parent document
    /// and, for the whole read and the read under each document as ROOT,
    /// the scope (`members`, `roots`, `left_out`), the tree (`parent`,
    /// `level`, `up`) and every member's shown node at no depth and depths
    /// 0 to 3 are the reference's. M: the walk stopping after one parent;
    /// the walk passing a non-admitted ancestor.
    #[test]
    fn every_forest_reads_as_the_read_before() {
        let scheme = IdScheme::from_toml(CONFIG).expect("[ids]");
        let paths = Paths::from_toml(CONFIG).expect("[paths]");
        let flow = FlowConfig::from_toml(CONFIG, &scheme);
        let mut reached = Reached::default();
        let mut not_live = 0;
        for seed in 0..FORESTS {
            let forest = Forest::random(seed);
            let input = forest.input(&scheme);
            let graph = SpecGraph::new(&input, &scheme, &paths);
            let selected = Selected::of(&graph, &flow);
            let parents = parents_of(&graph);
            let files = graph.paths().len();
            assert_eq!(parents.len(), files, "seed {seed}");
            for (file, parent) in parents.iter().enumerate() {
                assert_eq!(
                    *parent,
                    reference::parent_of(&graph, file),
                    "seed {seed}: the parent document of {}",
                    graph.paths()[file]
                );
            }
            not_live += (0..files)
                .filter(|&file| graph.standing(file) != Standing::Live)
                .count();
            if forest.cycle {
                assert!(!graph.cycles().is_empty(), "seed {seed}: a planted cycle");
                reached.cycles += 1;
            }
            if forest.docs.iter().any(|doc| {
                doc.parent
                    .is_some_and(|(above, through)| through && forest.docs[above].section)
            }) {
                reached.through_section += 1;
            }
            if forest.depth >= 10 {
                reached.deep += 1;
            }
            compare(
                &graph,
                &selected,
                &parents,
                None,
                &format!("seed {seed}, whole"),
                &mut reached,
            );
            for file in 0..files {
                let roots = [document(file)];
                compare(
                    &graph,
                    &selected,
                    &parents,
                    Some(&roots),
                    &format!("seed {seed}, ROOT {}", graph.paths()[file]),
                    &mut reached,
                );
            }
        }
        // The seeds reach every case: a generator that never planted one
        // proves nothing about it.
        assert!(not_live >= 300, "{not_live} files not live");
        for (name, count) in [
            ("walked", reached.walked),
            ("walked_twice", reached.walked_twice),
            ("stopped_below_live", reached.stopped_below_live),
            ("cycles", reached.cycles),
            ("through_section", reached.through_section),
            ("deep", reached.deep),
            ("folded", reached.folded),
        ] {
            assert!(count >= 20, "{name}: only {count}: {reached:?}");
        }
    }
}
