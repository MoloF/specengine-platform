//! `spec tree [ROOT] [--depth N] [--kind K]… [--archive]` (task spec
//! `spec-cli-graph`): the containment tree, `parent:` and section nesting
//! only, over the index-fed spec graph.
//!
//! Roots: `ROOT`'s holders (any `spec show` form; several → all, by (path,
//! position), one warning); none given → every live document under
//! `[paths] spec` with no resolving parent, by path (none → a note). Pre-
//! order: a node's nested sections by position, then its child documents
//! by path; `--depth N` counts from the roots (0). A node of a file the
//! live rule leaves out is not listed and takes its subtree with it,
//! counted. `--kind` filters the lines after the walk; `depth` and
//! `parent` stay those of the whole tree. A dangling `parent:`, a `parent:`
//! cycle (broken at its first member, one warning each) or several parent
//! holders (the first taken, one warning) never change the exit code. The
//! summary counts the lines listed, `roots` those at depth 0.
//!
//! ```text
//! <2 spaces × depth><name> | <kind or -> | <title or -> | <path>:<line>[ | status <s>][ | archived][ | parent <written> dangling | parent cycle]
//! nodes <n>, roots <r>[; left out: <g> generated, <t> archived (--archive)]
//! [truncated: <k> of <n> nodes not shown; give a ROOT, lower --depth or add --kind]
//! ```

use std::collections::BTreeSet;

use serde::Serialize;
use specengine_core::check::{NodeAt, Parent, SpecGraph};
use specengine_core::is_under;

use crate::cap::lines_within;
use crate::corpus::{Admission, LeftOut, depth_of, holders_warning, indexed, locate};
use crate::project::discover;
use crate::search::notes;
use crate::show::classify;
use crate::{CliError, Env, Globals, Message, one_line};

/// `spec tree` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeRequest {
    /// `ROOT` as given; `None`: the default roots.
    pub root: Option<String>,
    /// `--depth N`; `None`: unbounded.
    pub depth: Option<i64>,
    /// `--kind`: kinds of the lines to keep; empty: any.
    pub kinds: Vec<String>,
    /// `--archive`: Tier 3 files too.
    pub archive: bool,
}

/// What `spec tree` listed, or why `ROOT` names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeOutcome {
    /// `ROOT` as given.
    pub root: Option<String>,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    pub depth: Option<usize>,
    pub kinds: Vec<String>,
    pub archive: bool,
    /// Nodes of left-out files met under a listed node (or as default
    /// roots), each counted once with nothing under it.
    pub left_out: LeftOut,
    /// The lines listed at depth 0, `--kind` applied.
    pub roots: usize,
    /// Every line, in pre-order, `--kind` applied.
    pub nodes: Vec<TreeNode>,
    /// How many of `nodes` are printed (text and JSON alike).
    pub shown: usize,
}

impl TreeOutcome {
    pub fn truncated(&self) -> bool {
        self.shown < self.nodes.len()
    }
}

/// One line of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// Roots at 0.
    pub depth: usize,
    /// The name of the node it is listed under; `None` for a root.
    pub parent: Option<String>,
    pub mark: Option<TreeMark>,
    /// A document's `status:`.
    pub status: Option<String>,
    pub rev: Option<u32>,
    pub tokens_est: u32,
    /// Its file is Tier 3.
    pub archived: bool,
}

/// Why a node is a root although it declares a parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeMark {
    /// `parent:` resolves to nothing; as written.
    DanglingParent { written: String },
    /// The cycle of `parent:` is broken here.
    ParentCycle,
}

impl TreeMark {
    fn as_str(&self) -> &'static str {
        match self {
            Self::DanglingParent { .. } => "dangling-parent",
            Self::ParentCycle => "parent-cycle",
        }
    }
}

/// `spec tree`: updates the index, then walks the containment tree.
pub fn tree(env: &Env, globals: &Globals, request: &TreeRequest) -> Result<TreeOutcome, CliError> {
    let depth = depth_of(request.depth)?;
    let project = discover(env, globals)?;
    project.slug()?;
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = TreeOutcome {
        root: request.root.clone(),
        reason: None,
        messages: Vec::new(),
        depth,
        kinds: request.kinds.clone(),
        archive: request.archive,
        left_out: LeftOut::default(),
        roots: 0,
        nodes: Vec::new(),
        shown: 0,
    };
    let target = match request.root.as_deref().map(str::trim) {
        None => None,
        Some(written) => {
            match classify(
                written,
                scheme,
                &mut messages,
                "`spec tree` reads the current files",
            )? {
                Ok(target) => Some((target, written)),
                Err(reason) => return Ok(not_found(outcome, reason, messages)),
            }
        }
    };

    let input = indexed(env, &project, &mut messages, true)?;
    let paths = &project.config.paths;
    let graph = SpecGraph::new(&input, scheme, paths);
    let roots: Vec<NodeAt> = match target {
        Some((target, written)) => match locate(&graph, &target, written)? {
            Ok(nodes) => {
                messages.extend(holders_warning(
                    &graph,
                    written,
                    &nodes,
                    "all listed as roots",
                ));
                nodes
            }
            Err(reason) => return Ok(not_found(outcome, reason, messages)),
        },
        None => Vec::new(),
    };
    let asked: Vec<usize> = roots.iter().map(|at| at.file).collect();
    let admission = Admission::new(&graph, request.archive, asked);
    let roots = if request.root.is_some() {
        roots
    } else {
        default_roots(
            &graph,
            &admission,
            &paths.spec,
            &mut outcome.left_out,
            &mut messages,
        )
    };

    let mut listed = Vec::new();
    let mut visited = BTreeSet::new();
    let mut stack: Vec<(NodeAt, usize, Option<NodeAt>)> =
        roots.iter().rev().map(|&root| (root, 0, None)).collect();
    while let Some((at, level, parent)) = stack.pop() {
        if !visited.insert(at) {
            continue;
        }
        messages.extend(parent_warning(&graph, at));
        listed.push(line_of(&graph, at, level, parent));
        if depth.is_some_and(|depth| level >= depth) {
            continue;
        }
        for &child in graph.children(at).iter().rev() {
            if visited.contains(&child) {
                continue;
            }
            if !admission.admits(child.file) {
                outcome.left_out.add(graph.standing(child.file));
                continue;
            }
            stack.push((child, level + 1, Some(at)));
        }
    }
    if !request.kinds.is_empty() {
        listed.retain(|node: &TreeNode| {
            node.kind
                .as_deref()
                .is_some_and(|kind| request.kinds.iter().any(|wanted| wanted == kind))
        });
    }
    outcome.roots = listed.iter().filter(|node| node.depth == 0).count();
    outcome.nodes = listed;
    outcome.messages = messages;
    let summary = summary(&outcome).chars().count() + 1;
    let costs = outcome
        .nodes
        .iter()
        .map(|node| node_line(node).chars().count() + 1);
    outcome.shown = lines_within(costs, summary, true).0;
    Ok(outcome)
}

fn not_found(mut outcome: TreeOutcome, reason: String, messages: Vec<Message>) -> TreeOutcome {
    outcome.reason = Some(one_line(&reason));
    outcome.messages = messages;
    outcome
}

/// Every admitted document under `[paths] spec` that hangs under nothing,
/// by path; left-out ones counted; a note when there is none.
fn default_roots(
    graph: &SpecGraph<'_>,
    admission: &Admission<'_, '_>,
    spec: &str,
    left_out: &mut LeftOut,
    messages: &mut Vec<Message>,
) -> Vec<NodeAt> {
    let mut under = 0;
    let mut roots = Vec::new();
    for document in graph.documents() {
        let path = graph.paths()[document.file];
        if !(is_under(path, spec) || path == spec) {
            continue;
        }
        let root = graph.listed_under(document).is_none();
        if !admission.admits(document.file) {
            if root {
                left_out.add(graph.standing(document.file));
            }
            continue;
        }
        under += 1;
        if root {
            roots.push(document);
        }
    }
    if roots.is_empty() {
        let note = if under == 0 {
            format!("no document under [paths] spec \"{spec}\"; give a ROOT")
        } else {
            format!("no document under [paths] spec \"{spec}\" is a root; give a ROOT")
        };
        messages.push(Message::Note(note));
    }
    roots
}

/// The warning of a node met in the walk: the cycle broken at it, or the
/// several holders of its parent.
fn parent_warning(graph: &SpecGraph<'_>, at: NodeAt) -> Option<Message> {
    match graph.parent(at)? {
        Parent::Cycle { members } => {
            let names: Vec<String> = members.iter().map(|&node| graph.name(node)).collect();
            Some(Message::Warning(format!(
                "`parent:` forms a cycle through {}; `{}` is listed as a root",
                names.join(", "),
                graph.name(at)
            )))
        }
        Parent::Node { node, others } if !others.is_empty() => {
            let holders: Vec<String> = std::iter::once(node)
                .chain(others)
                .map(|holder| format!("{}:{}", graph.paths()[holder.file], graph.line(holder)))
                .collect();
            Some(Message::Warning(format!(
                "the parent of `{}` has {} holders; listed under the first: {}",
                graph.name(at),
                holders.len(),
                holders.join(", ")
            )))
        }
        _ => None,
    }
}

fn line_of(graph: &SpecGraph<'_>, at: NodeAt, depth: usize, parent: Option<NodeAt>) -> TreeNode {
    let node = graph.node(at);
    let mark = match graph.parent(at) {
        Some(Parent::Dangling { written, .. }) => Some(TreeMark::DanglingParent { written }),
        Some(Parent::Cycle { .. }) => Some(TreeMark::ParentCycle),
        _ => None,
    };
    TreeNode {
        id: node.and_then(|node| node.id.clone()),
        kind: node.and_then(|node| node.kind.clone()),
        title: node.and_then(|node| node.title.clone()),
        path: graph.paths()[at.file].to_owned(),
        line: graph.line(at),
        depth,
        parent: parent.map(|parent| graph.name(parent)),
        mark,
        status: node
            .filter(|_| at.ord == 0)
            .and_then(|node| node.fields.as_ref())
            .and_then(|fields| fields.status.clone()),
        rev: node.and_then(|node| node.rev),
        tokens_est: node.map_or(0, |node| node.tokens_est),
        archived: graph.is_tier3(at.file),
    }
}

/// The node's line, no line end.
fn node_line(node: &TreeNode) -> String {
    let name = node.id.as_deref().unwrap_or(&node.path);
    let mut line = format!(
        "{}{} | {} | {} | {}:{}",
        "  ".repeat(node.depth),
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
    if let Some(status) = &node.status {
        line.push_str(&format!(" | status {}", one_line(status)));
    }
    if node.archived {
        line.push_str(" | archived");
    }
    match &node.mark {
        Some(TreeMark::DanglingParent { written }) => {
            line.push_str(&format!(" | parent {} dangling", one_line(written)));
        }
        Some(TreeMark::ParentCycle) => line.push_str(" | parent cycle"),
        None => {}
    }
    line
}

fn summary(outcome: &TreeOutcome) -> String {
    format!(
        "nodes {}, roots {}{}",
        outcome.nodes.len(),
        outcome.roots,
        outcome.left_out.suffix()
    )
}

pub(crate) fn render_text(outcome: &TreeOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let mut out = String::new();
    for node in &outcome.nodes[..outcome.shown.min(outcome.nodes.len())] {
        out.push_str(&node_line(node));
        out.push('\n');
    }
    out.push_str(&summary(outcome));
    out.push('\n');
    if outcome.truncated() {
        out.push_str(&format!(
            "[truncated: {} of {} nodes not shown; give a ROOT, lower --depth or add --kind]\n",
            outcome.nodes.len() - outcome.shown,
            outcome.nodes.len()
        ));
    }
    out
}

#[derive(Serialize)]
struct TreeJson<'a> {
    #[serde(rename = "ref")]
    root: Option<&'a str>,
    reason: Option<&'a str>,
    notes: Vec<String>,
    depth: Option<usize>,
    kinds: &'a [String],
    archive: bool,
    left_out: LeftOut,
    truncated: bool,
    nodes: Vec<NodeJson<'a>>,
}

#[derive(Serialize)]
struct NodeJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    depth: usize,
    parent: Option<&'a str>,
    mark: Option<&'static str>,
    status: Option<&'a str>,
    rev: Option<u32>,
    tokens_est: u32,
    archived: bool,
}

fn view(outcome: &TreeOutcome) -> TreeJson<'_> {
    TreeJson {
        root: outcome.root.as_deref(),
        reason: outcome.reason.as_deref(),
        notes: notes(&outcome.messages),
        depth: outcome.depth,
        kinds: &outcome.kinds,
        archive: outcome.archive,
        left_out: outcome.left_out,
        truncated: outcome.truncated(),
        nodes: outcome.nodes[..outcome.shown.min(outcome.nodes.len())]
            .iter()
            .map(|node| NodeJson {
                id: node.id.as_deref(),
                kind: node.kind.as_deref(),
                title: node.title.as_deref(),
                path: &node.path,
                line: node.line,
                depth: node.depth,
                parent: node.parent.as_deref(),
                mark: node.mark.as_ref().map(TreeMark::as_str),
                status: node.status.as_deref(),
                rev: node.rev,
                tokens_est: node.tokens_est,
                archived: node.archived,
            })
            .collect(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for TreeOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
