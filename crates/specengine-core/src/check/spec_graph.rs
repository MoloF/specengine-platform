//! The spec graph of one corpus, for reading (task spec `spec-cli-graph`):
//! containment and links, each resolved from its citing file the way
//! `spec check` resolves it, once per call. `spec tree`, `spec graph`,
//! `spec show --links`, later MCP stdio and the context bundle read it.
//!
//! - Nodes: every document and `{#ID}` section of every parse, named
//!   [`NodeAt`] (file, position), ordered by (path, position).
//! - Containment: a section hangs under its nearest enclosing ID section,
//!   else its document (by spans); a document under its `parent:`, read
//!   through the check's front-matter references and resolved from its
//!   file ([`Resolver::resolve`]); several holders → the first in (path,
//!   position) order; dangling, `project:` or no `parent:` → a root; a
//!   `parent:` cycle is broken at its first member in (path, position)
//!   order, which becomes a root ([`Parent::Cycle`]).
//! - Links ([`Edge`]): the front-matter references the check reads
//!   (`parent:` aside), a path-form `canon:` and the inline mentions and
//!   Markdown file links of the body. A reference resolves as the check
//!   resolves it (the name fallback for inline mentions); a `canon:` path
//!   by the `canon-file` rules, a file link by the `link-dangling` rules;
//!   an anchor lands on the innermost ID section holding it, else the
//!   document. The source is the innermost ID section around the link,
//!   else the document; `status: superseded-by X` in `D` names its source
//!   `X`: the edge `X` supersedes `D`, written in `D` (its liveness `D`'s),
//!   incoming on `D`, outgoing on `X`.
//! - Walks: a node's links with its nested sections' ([`SpecGraph::links`]),
//!   the ancestor chain, and a breadth-first walk over a (type, direction)
//!   rule with an optional depth and a visited set ([`SpecGraph::walk`]).
//!
//! Lines and written forms come from each file's bytes; without them every
//! line is 1 and a written form is rebuilt from the parse. It reads no file.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use specengine_model::{
    CanonTarget, Direction, IdScheme, LinkOrigin, LinkTarget, MENTIONS, Node, ParsedFile,
    PathTarget, Reference,
};

use super::config::DocClass;
use super::input::{CheckFile, CheckInput};
use super::links::{self, percent_decode, resolve_link_path, was_read};
use super::render::{is_tier3, readable_fields};
use super::resolve::{Resolution, Resolver, Won, declared_references, reference_line, written};
use super::text::{FileText, front_matter_failed};
use crate::{DOCUMENT_EXTENSION, Paths, WalkScope};

/// The link type of `status: superseded-by X` (`X` supersedes the
/// document) and of a `supersedes:` item.
const SUPERSEDES: &str = "supersedes";

/// A node of the corpus: its file (an index into [`SpecGraph::paths`], path
/// order) and its position in that file's parse (0: the document). Ordered
/// by (path, position).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeAt {
    pub file: usize,
    pub ord: usize,
}

/// A file under the read commands' live rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Standing {
    /// Neither `class: generated` nor Tier 3 (a failed front-matter
    /// included): [`super::is_live`].
    Live,
    /// `class: generated`.
    Generated,
    /// Tier 3 ([`is_tier3`]).
    Tier3,
}

/// Where a node hangs in the containment tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parent {
    /// A document without `parent:` (or whose front-matter failed): a root.
    None,
    /// Listed under `node`: a section's nearest enclosing ID section, else
    /// its document; a document's resolved `parent:`, the first holder in
    /// (path, position) order, `others` the further holders.
    Node { node: NodeAt, others: Vec<NodeAt> },
    /// `parent:` resolves to nothing: a root.
    Dangling { written: String, reason: String },
    /// `parent:` names another project: a root.
    Skipped { written: String },
    /// The first member in (path, position) order of a `parent:` cycle,
    /// broken here: a root. `members` in (path, position) order.
    Cycle { members: Vec<NodeAt> },
}

/// One end of a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// Its nodes, one per holder file, in path order; never empty.
    Nodes(Vec<NodeAt>),
    /// It resolves to nothing: why.
    Dangling(String),
    /// `project:`: another project's.
    Skipped,
    /// A path the check never checks: not a document, outside the walk, a
    /// `canon:` without `#anchor`, a target without a node.
    Unchecked,
}

/// How the written end of a link fared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkState {
    Resolved,
    Dangling,
    Skipped,
    Unchecked,
}

impl LinkState {
    /// `resolved`, `dangling`, `skipped` or `unchecked`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Dangling => "dangling",
            Self::Skipped => "skipped",
            Self::Unchecked => "unchecked",
        }
    }
}

/// One link, resolved from the file it is written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// A member of the shared link types, `mentions`, or an unknown
    /// declared type.
    pub link_type: String,
    pub origin: LinkOrigin,
    /// The file it is written in.
    pub file: usize,
    /// The node whose span holds it: the document for the front-matter,
    /// the innermost ID section around an inline link, else the document.
    pub holder: NodeAt,
    /// The holder, but for `status: superseded-by X`: `X`.
    pub source: Endpoint,
    pub target: Endpoint,
    /// What is written names the source (`status: superseded-by X`), not
    /// the target.
    pub names_source: bool,
    /// The reference or path as written.
    pub written: String,
    /// Where it is written (1 without bytes).
    pub line: usize,
    /// The byte offset it is written at (0 when unknown): orders links on
    /// one line.
    pub offset: usize,
    /// A remark on a resolved or unchecked link: an anchor naming nothing
    /// (the link lands on the document), a `canon:` without `#anchor`.
    pub note: Option<String>,
}

impl Edge {
    /// The end its written text names.
    pub fn written_end(&self) -> &Endpoint {
        if self.names_source {
            &self.source
        } else {
            &self.target
        }
    }

    /// How its written end fared.
    pub fn state(&self) -> LinkState {
        match self.written_end() {
            Endpoint::Nodes(_) => LinkState::Resolved,
            Endpoint::Dangling(_) => LinkState::Dangling,
            Endpoint::Skipped => LinkState::Skipped,
            Endpoint::Unchecked => LinkState::Unchecked,
        }
    }

    /// Why it dangles, else its note.
    pub fn reason(&self) -> Option<&str> {
        match self.written_end() {
            Endpoint::Dangling(reason) => Some(reason),
            _ => self.note.as_deref(),
        }
    }
}

/// One node's links ([`SpecGraph::links`]); edge indexes into
/// [`SpecGraph::edges`], ascending.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeLinks {
    /// Written in the node or a nested section, every state, but a link
    /// naming its source (`status: superseded-by X`: incoming there); and
    /// a link naming the node or a nested section as its source, written in
    /// an admitted file: the edge and the node of the span it leaves.
    pub outgoing: Vec<(usize, NodeAt)>,
    /// Resolving to the node or a nested section, written in an admitted
    /// file: the edge and the node it lands on.
    pub incoming: Vec<(usize, NodeAt)>,
    /// Incoming or naming it as their source, but written in a file not
    /// admitted.
    pub left_out: Vec<usize>,
}

/// What a breadth-first walk reached ([`SpecGraph::walk`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Walk {
    /// The nodes reached and their distance, by (distance, path, position).
    pub nodes: Vec<(NodeAt, usize)>,
    /// The edges of a followed type met from a reached node in their
    /// direction, written in admitted files: resolved, dangling, skipped or
    /// unchecked (only resolved ones are followed). Ascending.
    pub edges: Vec<usize>,
    /// Such edges written in files not admitted. Ascending.
    pub left_out: Vec<usize>,
}

/// The containment tree and the resolved links of one corpus.
pub struct SpecGraph<'a> {
    resolver: Resolver<'a>,
    scheme: &'a IdScheme,
    link_base: Option<&'a str>,
    scope: WalkScope,
    /// Path order.
    files: Vec<&'a CheckFile>,
    texts: Vec<FileText<'a>>,
    /// Path → file (the first, should a path repeat).
    by_path: BTreeMap<&'a str, usize>,
    standing: Vec<Standing>,
    /// Per file, per node: the position of the ID section or document a
    /// section is nested in; `None` for the document.
    enclosing: Vec<Vec<Option<usize>>>,
    /// Documents, and the breaker of a cycle: their parent.
    parents: BTreeMap<NodeAt, Parent>,
    /// Children in tree order: nested sections by position, then child
    /// documents by path.
    children: BTreeMap<NodeAt, Vec<NodeAt>>,
    /// `parent:` cycles, each in (path, position) order, by first member.
    cycles: Vec<Vec<NodeAt>>,
    edges: Vec<Edge>,
    /// Node → the edges with it as a resolved source.
    from: BTreeMap<NodeAt, Vec<usize>>,
    /// Node → the edges with it as a resolved target.
    into: BTreeMap<NodeAt, Vec<usize>>,
    /// Node → the edges its span holds.
    held: BTreeMap<NodeAt, Vec<usize>>,
}

impl<'a> SpecGraph<'a> {
    /// The graph of `input`'s files, whatever their order; `paths` gives
    /// the feature documents, the link base and the walk scope.
    pub fn new(input: &'a CheckInput, scheme: &'a IdScheme, paths: &'a Paths) -> Self {
        let mut files: Vec<&CheckFile> = input.files.iter().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut by_path = BTreeMap::new();
        for (index, file) in files.iter().enumerate() {
            by_path.entry(file.path.as_str()).or_insert(index);
        }
        let mut graph = SpecGraph {
            resolver: Resolver::of_sorted(&files, scheme, &paths.features),
            scheme,
            link_base: paths.link_base.as_deref(),
            scope: paths.walk_scope(),
            texts: files
                .iter()
                .map(|file| FileText::new(&file.bytes))
                .collect(),
            standing: files
                .iter()
                .map(|file| standing_of(file.parsed.as_ref()))
                .collect(),
            enclosing: files
                .iter()
                .map(|file| enclosing_of(file.parsed.as_ref()))
                .collect(),
            files,
            by_path,
            parents: BTreeMap::new(),
            children: BTreeMap::new(),
            cycles: Vec::new(),
            edges: Vec::new(),
            from: BTreeMap::new(),
            into: BTreeMap::new(),
            held: BTreeMap::new(),
        };
        for file in 0..graph.files.len() {
            let (parent, edges) = graph.file_links(file);
            if let Some(parent) = parent {
                graph.parents.insert(NodeAt { file, ord: 0 }, parent);
            }
            graph.edges.extend(edges);
        }
        graph.break_cycles();
        graph.link_children();
        graph.index_edges();
        graph
    }

    /// The files, in path order: what [`NodeAt::file`] indexes.
    pub fn paths(&self) -> &[&'a str] {
        self.resolver.paths()
    }

    /// The file at `path`, when walked.
    pub fn file_of(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    /// A file as given: its bytes (the source of every span) and its
    /// parse.
    pub fn file(&self, file: usize) -> Option<&'a CheckFile> {
        self.files.get(file).copied()
    }

    /// The `[ids]` scheme the graph resolves by.
    pub fn scheme(&self) -> &'a IdScheme {
        self.scheme
    }

    /// The file under the live rule.
    pub fn standing(&self, file: usize) -> Standing {
        self.standing.get(file).copied().unwrap_or(Standing::Live)
    }

    /// The file is Tier 3.
    pub fn is_tier3(&self, file: usize) -> bool {
        self.standing(file) == Standing::Tier3
    }

    /// The nodes of a file's parse; none without one (not read, not UTF-8).
    pub fn nodes(&self, file: usize) -> &'a [Node] {
        self.files
            .get(file)
            .and_then(|file| file.parsed.as_ref())
            .map_or(&[], |parsed| parsed.nodes.as_slice())
    }

    /// The node at `at`.
    pub fn node(&self, at: NodeAt) -> Option<&'a Node> {
        self.nodes(at.file).get(at.ord)
    }

    /// The document of a file, when it has one.
    pub fn document(&self, file: usize) -> Option<NodeAt> {
        (!self.nodes(file).is_empty()).then_some(NodeAt { file, ord: 0 })
    }

    /// Every document, in path order.
    pub fn documents(&self) -> impl Iterator<Item = NodeAt> + '_ {
        (0..self.files.len()).filter_map(|file| self.document(file))
    }

    /// Its ID, else its file's path.
    pub fn name(&self, at: NodeAt) -> String {
        self.node(at)
            .and_then(|node| node.id.clone())
            .unwrap_or_else(|| self.paths()[at.file].to_owned())
    }

    /// The line its span starts on (1 without bytes).
    pub fn line(&self, at: NodeAt) -> usize {
        self.node(at)
            .map_or(1, |node| self.texts[at.file].line(node.span.start))
    }

    /// The nodes a reference with no citing file names
    /// ([`Resolver::resolve_detached`]): per holder file, the node holding
    /// the ID, its `aliases_from` target, the section, or the document for
    /// an `aliases:` entry.
    pub fn locate(&self, reference: &Reference, written: &str) -> Endpoint {
        self.locate_from(None, reference, written, false)
    }

    /// Where `at` hangs; `None` for no node.
    pub fn parent(&self, at: NodeAt) -> Option<Parent> {
        self.node(at)?;
        if let Some(parent) = self.parents.get(&at) {
            return Some(parent.clone());
        }
        Some(match self.enclosing_node(at) {
            Some(node) => Parent::Node {
                node,
                others: Vec::new(),
            },
            None => Parent::None,
        })
    }

    /// The node `at` is listed under in the tree, if any.
    pub fn listed_under(&self, at: NodeAt) -> Option<NodeAt> {
        match self.parents.get(&at) {
            Some(Parent::Node { node, .. }) => Some(*node),
            Some(_) => None,
            None => self.enclosing_node(at),
        }
    }

    /// Its children in tree order: nested sections by position, then child
    /// documents by path.
    pub fn children(&self, at: NodeAt) -> &[NodeAt] {
        self.children.get(&at).map_or(&[], Vec::as_slice)
    }

    /// The `parent:` cycles, each in (path, position) order, by their first
    /// member (the root it is broken at).
    pub fn cycles(&self) -> &[Vec<NodeAt>] {
        &self.cycles
    }

    /// The nodes `at` is listed under, nearest first.
    pub fn ancestors(&self, at: NodeAt) -> Vec<NodeAt> {
        let mut seen = BTreeSet::from([at]);
        let mut chain = Vec::new();
        let mut current = self.listed_under(at);
        while let Some(node) = current {
            if !seen.insert(node) {
                break;
            }
            chain.push(node);
            current = self.listed_under(node);
        }
        chain
    }

    /// The node and the ID sections nested in it, by position.
    pub fn within(&self, at: NodeAt) -> Vec<NodeAt> {
        let nodes = self.nodes(at.file);
        let Some(node) = nodes.get(at.ord) else {
            return Vec::new();
        };
        let mut within = vec![at];
        for (ord, other) in nodes.iter().enumerate().skip(at.ord + 1) {
            // Sections come in source order: none after this one's end.
            if at.ord != 0 && other.span.start >= node.span.end {
                break;
            }
            if at.ord == 0 || node.span.contains(other.span) {
                within.push(NodeAt { file: at.file, ord });
            }
        }
        within
    }

    /// Every link of the corpus, in (path, written order).
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The links of `at` and its nested sections: outgoing ones written in
    /// their span every state; incoming ones and those naming it as their
    /// source (`status: superseded-by X` on `X`: outgoing) written in a
    /// file `admit` takes, the others counted.
    pub fn links(&self, at: NodeAt, admit: impl Fn(usize) -> bool) -> NodeLinks {
        let mut outgoing = BTreeMap::new();
        let mut incoming = BTreeMap::new();
        let mut left_out = BTreeSet::new();
        for node in self.within(at) {
            for &edge in self.held.get(&node).into_iter().flatten() {
                if !self.edges[edge].names_source {
                    outgoing.entry(edge).or_insert(node);
                }
            }
            for &edge in self.from.get(&node).into_iter().flatten() {
                if !self.edges[edge].names_source {
                    continue;
                }
                if admit(self.edges[edge].file) {
                    outgoing.entry(edge).or_insert(node);
                } else {
                    left_out.insert(edge);
                }
            }
            for &edge in self.into.get(&node).into_iter().flatten() {
                if admit(self.edges[edge].file) {
                    incoming.entry(edge).or_insert(node);
                } else {
                    left_out.insert(edge);
                }
            }
        }
        NodeLinks {
            outgoing: outgoing.into_iter().collect(),
            incoming: incoming.into_iter().collect(),
            left_out: left_out.into_iter().collect(),
        }
    }

    /// Breadth-first from `starts` (distance 0): from each reached node and
    /// its nested sections, the edges whose type `follow` gives a direction,
    /// out to their targets or in to their sources; a node is visited once,
    /// an edge to a visited node still listed. Nodes at distance `depth`
    /// are not expanded. Edges written in a file `admit` refuses are
    /// counted, never followed.
    pub fn walk(
        &self,
        starts: &[NodeAt],
        follow: impl Fn(&str) -> Option<Direction>,
        depth: Option<usize>,
        admit: impl Fn(usize) -> bool,
    ) -> Walk {
        let mut distance: BTreeMap<NodeAt, usize> = BTreeMap::new();
        let mut queue = VecDeque::new();
        for &start in starts {
            if self.node(start).is_some() && !distance.contains_key(&start) {
                distance.insert(start, 0);
                queue.push_back(start);
            }
        }
        let mut edges = BTreeSet::new();
        let mut left_out = BTreeSet::new();
        while let Some(at) = queue.pop_front() {
            let reached = distance[&at];
            if depth.is_some_and(|depth| reached >= depth) {
                continue;
            }
            for node in self.within(at) {
                let follow = &follow;
                let out = self
                    .from
                    .get(&node)
                    .into_iter()
                    .flatten()
                    .filter(|&&edge| follow(&self.edges[edge].link_type) == Some(Direction::Out))
                    .map(|&edge| (edge, &self.edges[edge].target));
                let back = self
                    .into
                    .get(&node)
                    .into_iter()
                    .flatten()
                    .filter(|&&edge| follow(&self.edges[edge].link_type) == Some(Direction::In))
                    .map(|&edge| (edge, &self.edges[edge].source));
                for (edge, far) in out.chain(back) {
                    if !admit(self.edges[edge].file) {
                        left_out.insert(edge);
                        continue;
                    }
                    edges.insert(edge);
                    let Endpoint::Nodes(nodes) = far else {
                        continue;
                    };
                    for &next in nodes {
                        if let std::collections::btree_map::Entry::Vacant(entry) =
                            distance.entry(next)
                        {
                            entry.insert(reached + 1);
                            queue.push_back(next);
                        }
                    }
                }
            }
        }
        let mut nodes: Vec<(NodeAt, usize)> = distance.into_iter().collect();
        nodes.sort_by_key(|&(at, distance)| (distance, at));
        Walk {
            nodes,
            edges: edges.into_iter().collect(),
            left_out: left_out.into_iter().collect(),
        }
    }

    /// A section's nearest enclosing ID section, else its document.
    fn enclosing_node(&self, at: NodeAt) -> Option<NodeAt> {
        let ord = self
            .enclosing
            .get(at.file)?
            .get(at.ord)
            .copied()
            .flatten()?;
        Some(NodeAt { file: at.file, ord })
    }

    /// The innermost ID section of `file` holding the byte at `offset`,
    /// else the document (position 0).
    fn holder_at(&self, file: usize, offset: usize) -> usize {
        let mut found = 0;
        for (ord, node) in self.nodes(file).iter().enumerate().skip(1) {
            if node.span.start > offset {
                break;
            }
            if offset < node.span.end {
                found = ord;
            }
        }
        found
    }

    /// The nodes a reference names, cited from `from` (`None`: no citing
    /// file); `mention`: an inline mention, with the name fallback.
    fn locate_from(
        &self,
        from: Option<&str>,
        reference: &Reference,
        written: &str,
        mention: bool,
    ) -> Endpoint {
        let (resolution, won) = self
            .resolver
            .resolve_named(from, reference, written, mention);
        match resolution {
            Resolution::Resolved(holders) => Endpoint::Nodes(
                holders
                    .into_iter()
                    .map(|file| NodeAt {
                        file,
                        ord: won
                            .as_ref()
                            .and_then(|won| self.node_in(file, won, reference))
                            .unwrap_or(0),
                    })
                    .collect(),
            ),
            Resolution::Dangling(reason) => Endpoint::Dangling(reason),
            Resolution::Skipped => Endpoint::Skipped,
        }
    }

    /// The node of a holder file that a resolved reference names, in the
    /// resolver's order: the section; the ID itself; the document for an
    /// `aliases:` entry; the `aliases_from` target.
    fn node_in(&self, file: usize, won: &Won, reference: &Reference) -> Option<usize> {
        let nodes = self.nodes(file);
        let with_id = |id: &str, skip: usize| {
            nodes
                .iter()
                .enumerate()
                .skip(skip)
                .find(|(_, node)| node.id.as_deref() == Some(id))
                .map(|(ord, _)| ord)
        };
        if let Some(section) = &reference.section {
            return with_id(section, 1);
        }
        if reference.alias_of.is_none()
            && let Some(ord) = with_id(&won.id, 0)
        {
            return Some(ord);
        }
        let declared = nodes
            .first()
            .and_then(|document| document.fields.as_ref())
            .and_then(|fields| fields.aliases.as_ref())
            .is_some_and(|aliases| {
                aliases
                    .iter()
                    .any(|alias| *alias == won.id || *alias == won.bare)
            });
        if declared {
            return Some(0);
        }
        let prefix = reference.alias_of.as_deref()?;
        let (_, body) = won.id.split_once('-')?;
        with_id(&format!("{prefix}-{body}"), 0)
    }

    /// One file's `parent:` (documents with a readable front-matter) and
    /// its links, in written order: front-matter references, a path-form
    /// `canon:`, then the body's inline links.
    fn file_links(&self, file: usize) -> (Option<Parent>, Vec<Edge>) {
        let Some(parsed) = self.files[file].parsed.as_ref() else {
            return (None, Vec::new());
        };
        let Some(document) = parsed.document() else {
            return (None, Vec::new());
        };
        let text = &self.texts[file];
        let from = self.paths()[file];
        let at = NodeAt { file, ord: 0 };
        let mut parent = Parent::None;
        let mut edges = Vec::new();
        if !front_matter_failed(parsed) {
            for declared in declared_references(document, self.scheme, text) {
                let reference = &declared.reference;
                let written = written(text, reference);
                let end = self.locate_from(Some(from), reference, &written, false);
                if declared.key == "parent" {
                    parent = match end {
                        Endpoint::Nodes(mut nodes) => {
                            nodes.sort_unstable();
                            let node = nodes.remove(0);
                            Parent::Node {
                                node,
                                others: nodes,
                            }
                        }
                        Endpoint::Dangling(reason) => Parent::Dangling { written, reason },
                        Endpoint::Skipped | Endpoint::Unchecked => Parent::Skipped { written },
                    };
                    continue;
                }
                let names_source = declared.key == "status";
                let link_type = match declared.key {
                    "status" => SUPERSEDES.to_owned(),
                    "adrs" | "refs" => MENTIONS.to_owned(),
                    "links" => declared.link_type.clone().unwrap_or_default(),
                    key => key.to_owned(),
                };
                let (source, target) = if names_source {
                    (end, Endpoint::Nodes(vec![at]))
                } else {
                    (Endpoint::Nodes(vec![at]), end)
                };
                edges.push(Edge {
                    link_type,
                    origin: LinkOrigin::Frontmatter,
                    file,
                    holder: at,
                    source,
                    target,
                    names_source,
                    line: reference_line(text, reference, declared.key),
                    offset: reference.span.map_or(0, |span| span.start),
                    written,
                    note: None,
                });
            }
            if let Some(CanonTarget::Path(target)) = document
                .fields
                .as_ref()
                .and_then(|fields| fields.canon.as_ref())
            {
                let (end, note) = self.canon_path(target);
                edges.push(Edge {
                    link_type: "canon".to_owned(),
                    origin: LinkOrigin::Frontmatter,
                    file,
                    holder: at,
                    source: Endpoint::Nodes(vec![at]),
                    target: end,
                    names_source: false,
                    written: canon_written(text, target),
                    line: target.span.filter(|_| !text.is_empty()).map_or_else(
                        || text.key_line("canon").unwrap_or(1),
                        |span| text.line(span.start),
                    ),
                    offset: target.span.map_or(0, |span| span.start),
                    note,
                });
            }
        }
        for link in parsed
            .links
            .iter()
            .filter(|link| link.origin == LinkOrigin::Inline)
        {
            let (written, line, offset, end, note) = match &link.dst {
                LinkTarget::Reference(reference) => {
                    let written = written(text, reference);
                    let end = self.locate_from(Some(from), reference, &written, true);
                    let line = reference
                        .span
                        .filter(|_| !text.is_empty())
                        .map_or(1, |span| text.line(span.start));
                    let offset = reference.span.map_or(0, |span| span.start);
                    (written, line, offset, end, None)
                }
                LinkTarget::Path(target) => {
                    let (end, note) = self.file_link(file, target);
                    let offset = target.span.map_or(0, |span| span.start);
                    (
                        links::written(text, target),
                        links::line(text, target),
                        offset,
                        end,
                        note,
                    )
                }
            };
            let holder = NodeAt {
                file,
                ord: self.holder_at(file, offset),
            };
            edges.push(Edge {
                link_type: link.link_type.clone(),
                origin: LinkOrigin::Inline,
                file,
                holder,
                source: Endpoint::Nodes(vec![holder]),
                target: end,
                names_source: false,
                written,
                line,
                offset,
                note,
            });
        }
        (Some(parent), edges)
    }

    /// A path-form `canon:`, by the check's rules: without `#anchor` it
    /// lands on the document, unchecked when there is none (the check:
    /// `canon-form`); a file that is not walked, not read or not
    /// canon dangles (`canon-file`); an anchor naming nothing lands on the
    /// document (`canon-anchor`).
    fn canon_path(&self, target: &PathTarget) -> (Endpoint, Option<String>) {
        let path = target.path.as_str();
        let Some(anchor) = target.anchor.as_deref() else {
            return match self.file_of(path).and_then(|file| self.document(file)) {
                Some(document) => (
                    Endpoint::Nodes(vec![document]),
                    Some("`canon:` names no #anchor; lands on the document".to_owned()),
                ),
                None => (
                    Endpoint::Unchecked,
                    Some("`canon:` names no #anchor; not checked".to_owned()),
                ),
            };
        };
        let Some(file) = self.file_of(path) else {
            return (
                Endpoint::Dangling(format!("names {path}, which is no walked document")),
                None,
            );
        };
        let Some(parsed) = self.files[file].parsed.as_ref() else {
            return (
                Endpoint::Dangling(format!("names {path}, which could not be read")),
                None,
            );
        };
        let class = parsed
            .document()
            .and_then(|document| document.fields.as_ref())
            .and_then(|fields| fields.class.as_deref());
        if class != Some(DocClass::Canon.as_str()) {
            return (
                Endpoint::Dangling(format!("names {path}, which is not canon")),
                None,
            );
        }
        self.anchored(file, parsed, anchor)
    }

    /// A Markdown file link written in `file`, by the check's rules: a
    /// checked path (a document, or `#anchor` alone) names a walked file,
    /// else dangles when a candidate lies in the walk scope
    /// (`link-dangling`); anything else is never checked. An anchor of a
    /// read target lands on its section, or names nothing (`link-anchor`).
    fn file_link(&self, file: usize, target: &PathTarget) -> (Endpoint, Option<String>) {
        let from = self.paths()[file];
        let path = percent_decode(&target.path);
        let resolved = if path.is_empty() {
            if target.anchor.is_none() {
                return (Endpoint::Unchecked, None);
            }
            file
        } else if path.ends_with(DOCUMENT_EXTENSION) {
            match resolve_link_path(&self.by_path, self.link_base, from, &path) {
                Ok(found) => found,
                Err(tried) => {
                    let tried: Vec<&str> = tried.iter().flatten().map(String::as_str).collect();
                    if !tried
                        .iter()
                        .any(|candidate| self.scope.in_walk_scope(candidate))
                    {
                        return (Endpoint::Unchecked, None);
                    }
                    let tried = tried
                        .iter()
                        .map(|candidate| format!("`{candidate}`"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return (
                        Endpoint::Dangling(format!("names no walked document (tried {tried})")),
                        None,
                    );
                }
            }
        } else {
            return (Endpoint::Unchecked, None);
        };
        let Some(document) = self.document(resolved) else {
            return (Endpoint::Unchecked, None);
        };
        let (Some(anchor), Some(parsed)) = (
            target.anchor.as_deref(),
            self.files[resolved]
                .parsed
                .as_ref()
                .filter(|parsed| was_read(parsed)),
        ) else {
            return (Endpoint::Nodes(vec![document]), None);
        };
        self.anchored(resolved, parsed, &percent_decode(anchor))
    }

    /// `anchor` of `file`: its section of that ID, else the innermost ID
    /// section holding the anchor of that name, else the document, the note
    /// naming the miss (the `canon-anchor` predicate).
    fn anchored(
        &self,
        file: usize,
        parsed: &ParsedFile,
        anchor: &str,
    ) -> (Endpoint, Option<String>) {
        let nodes = &parsed.nodes;
        if nodes.is_empty() {
            return (Endpoint::Unchecked, None);
        }
        if let Some(ord) = nodes
            .iter()
            .skip(1)
            .position(|node| node.id.as_deref() == Some(anchor))
        {
            return (Endpoint::Nodes(vec![NodeAt { file, ord: ord + 1 }]), None);
        }
        if let Some(found) = parsed.anchors.iter().find(|known| known.name == anchor) {
            let ord = self.holder_at(file, found.span.start);
            return (Endpoint::Nodes(vec![NodeAt { file, ord }]), None);
        }
        (
            Endpoint::Nodes(vec![NodeAt { file, ord: 0 }]),
            Some(format!(
                "`{}` has no anchor or section `#{anchor}`",
                self.paths()[file]
            )),
        )
    }

    /// Breaks every `parent:` cycle at its first member in (path,
    /// position) order. A cycle runs through a document (a section's chain
    /// ends at its own), so starting from each document finds them all.
    fn break_cycles(&mut self) {
        let mut done: BTreeSet<NodeAt> = BTreeSet::new();
        let mut cycles = Vec::new();
        let documents: Vec<NodeAt> = self.documents().collect();
        for start in documents {
            let mut path: Vec<NodeAt> = Vec::new();
            let mut on_path: BTreeMap<NodeAt, usize> = BTreeMap::new();
            let mut current = Some(start);
            while let Some(at) = current {
                if let Some(&position) = on_path.get(&at) {
                    cycles.push(path[position..].to_vec());
                    break;
                }
                if done.contains(&at) {
                    break;
                }
                on_path.insert(at, path.len());
                path.push(at);
                current = self.listed_under(at);
            }
            done.extend(path);
        }
        for mut members in cycles {
            members.sort_unstable();
            self.parents.insert(
                members[0],
                Parent::Cycle {
                    members: members.clone(),
                },
            );
            self.cycles.push(members);
        }
        self.cycles.sort();
    }

    fn link_children(&mut self) {
        let mut children: BTreeMap<NodeAt, Vec<NodeAt>> = BTreeMap::new();
        for file in 0..self.files.len() {
            for ord in 0..self.nodes(file).len() {
                let at = NodeAt { file, ord };
                if let Some(parent) = self.listed_under(at) {
                    children.entry(parent).or_default().push(at);
                }
            }
        }
        for list in children.values_mut() {
            // Nested sections (same file, by position), then documents by
            // path.
            list.sort_by_key(|&child| (child.ord == 0, child));
        }
        self.children = children;
    }

    fn index_edges(&mut self) {
        for (index, edge) in self.edges.iter().enumerate() {
            self.held.entry(edge.holder).or_default().push(index);
            if let Endpoint::Nodes(nodes) = &edge.source {
                for &node in nodes {
                    self.from.entry(node).or_default().push(index);
                }
            }
            if let Endpoint::Nodes(nodes) = &edge.target {
                for &node in nodes {
                    self.into.entry(node).or_default().push(index);
                }
            }
        }
    }
}

/// A file under the live rule; no parse or a failed front-matter is live.
fn standing_of(parsed: Option<&ParsedFile>) -> Standing {
    match parsed.and_then(readable_fields) {
        Some(fields) if fields.class.as_deref() == Some(DocClass::Generated.as_str()) => {
            Standing::Generated
        }
        Some(fields) if is_tier3(fields) => Standing::Tier3,
        _ => Standing::Live,
    }
}

/// Per node of a parse: the position of the ID section or document a
/// section is nested in (sections nest by span, in source order).
fn enclosing_of(parsed: Option<&ParsedFile>) -> Vec<Option<usize>> {
    let Some(parsed) = parsed else {
        return Vec::new();
    };
    let mut enclosing = Vec::with_capacity(parsed.nodes.len());
    let mut open: Vec<usize> = Vec::new();
    for (ord, node) in parsed.nodes.iter().enumerate() {
        if ord == 0 {
            enclosing.push(None);
            continue;
        }
        while open
            .last()
            .is_some_and(|&top| !parsed.nodes[top].span.contains(node.span))
        {
            open.pop();
        }
        enclosing.push(Some(open.last().copied().unwrap_or(0)));
        open.push(ord);
    }
    enclosing
}

/// A path-form `canon:` as written: the text under its span, else
/// `path#anchor` (the check's form).
fn canon_written(text: &FileText<'_>, target: &PathTarget) -> String {
    match (target.span, &target.anchor) {
        (Some(span), _) if !text.is_empty() => text.text(span),
        (_, Some(anchor)) => format!("{}#{anchor}", target.path),
        (_, None) => target.path.clone(),
    }
}
