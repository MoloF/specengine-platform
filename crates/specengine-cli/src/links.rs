//! `spec show REF --links`: a shown node's links, one hop both ways, from
//! the spec graph (task spec `spec-cli-graph`).
//!
//! Outgoing: the links written in the node's span (a document: its
//! front-matter and every nested section), every state. Incoming: the links
//! resolving to the node or a nested section, written in a live file
//! (`--archive`: Tier 3 too; never generated; the shown node's own file
//! always); the others counted. `status: superseded-by X` in `D`: on `D`
//! `in supersedes X`, on `X` `out supersedes D`, live as `D`'s file. Each
//! list: strong types before `mentions`, then (type, path, line, column).
//!
//! The block printed after a node's header line and before its text, one
//! line per link, then the count:
//!
//! ```text
//!   <out|in> <type> <name, else written> | <path>:<line>[ | at <section>][ | as <written>][ | kind <kind>][ | when <when>][ | dangling: <reason> | skipped: another project | unchecked]
//!   links <o> out, <i> in; used by <u>, mentioned by <m>[; left out: <g> generated, <t> archived (--archive)]
//! ```
//!
//! `name`: the other end, its ID else its path; `at`: the nested section
//! that holds the link or that it lands on; `as`: the written form when it
//! is not the name of the node it names; `kind`, `when`: a labelled edge's.
//! The JSON alone adds `title` and `end_kind` after `name`: that node's title
//! and kind (task spec `doc-modal` DM-1).
//!
//! `used by`, `mentioned by` (JSON `usage`): core's `tally` over the In
//! rows, live ones only, so `--archive` widens In and never the counts
//! (task spec `usage-orphans` UO1, UO6).

use serde::Serialize;
use specengine_core::check::{Endpoint, LinkState, NodeAt, SpecGraph, Standing};
use specengine_core::usage::{UseRow, tally};
use specengine_model::{Direction, EdgeLabel, LinkOrigin, is_weak_link};

use crate::corpus::{Admission, LeftOut};
use crate::one_line;

/// One node's links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShownLinks {
    /// In print order.
    pub outgoing: Vec<ShownLink>,
    /// In print order.
    pub incoming: Vec<ShownLink>,
    /// Links written in files the live rule left out: incoming ones and
    /// an `out supersedes` from a left-out `superseded-by`.
    pub left_out: LeftOut,
    /// The counts of the In rows' live sources in other files (task spec
    /// `usage-orphans` UO1, UO6).
    pub usage: LinkUsage,
}

/// What uses a shown node: core's `tally` over its In rows, live ones only
/// (task spec `usage-orphans` UO1, UO6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LinkUsage {
    pub used_by: u32,
    pub mentioned_by: u32,
}

impl ShownLinks {
    /// The links listed, both ways.
    pub fn len(&self) -> usize {
        self.outgoing.len() + self.incoming.len()
    }

    /// No link listed.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One link as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownLink {
    pub direction: Direction,
    pub link_type: String,
    pub origin: LinkOrigin,
    /// The nested section that holds it (outgoing) or that it lands on
    /// (incoming).
    pub at: Option<String>,
    /// The other end, its ID else its path; `None` when unresolved.
    pub name: Option<String>,
    /// The title of the node `name` names; `None` when unresolved or
    /// untitled (task spec `doc-modal` DM-1).
    pub title: Option<String>,
    /// The kind of the node `name` names; `None` when unresolved or
    /// kindless. Not `kind`: that is a labelled edge's.
    pub end_kind: Option<String>,
    /// The reference or path as written.
    pub written: String,
    /// Where it is written.
    pub path: String,
    pub line: usize,
    pub state: LinkState,
    /// Why it dangles, or a remark on a resolved link (an anchor naming
    /// nothing).
    pub reason: Option<String>,
    /// A labelled edge's kind.
    pub kind: Option<String>,
    /// A labelled edge's condition.
    pub when: Option<String>,
    /// The name of the node its written form names, when resolved.
    named: Option<String>,
    /// Its byte offset: orders links on one line.
    offset: usize,
}

/// The links of `at` and its nested sections.
pub(crate) fn node_links(
    graph: &SpecGraph<'_>,
    at: NodeAt,
    admission: &Admission<'_, '_>,
) -> ShownLinks {
    let found = graph.links(at, |file| admission.admits(file));
    let edges = graph.edges();
    let first_node = |end: &Endpoint| match end {
        Endpoint::Nodes(nodes) => nodes.first().copied(),
        _ => None,
    };
    // `here`: the node of `at`'s span that holds it or that it names as its
    // source (outgoing), or that it lands on (incoming).
    let link = |index: usize, direction: Direction, here: NodeAt| {
        let edge = &edges[index];
        // The other end: outgoing the target, incoming the source.
        let (end, name, named) = match direction {
            Direction::Out => {
                let end = first_node(&edge.target);
                let name = end.map(|node| graph.name(node));
                let named = if edge.names_source {
                    Some(graph.name(here))
                } else {
                    name.clone()
                };
                (end, name, named)
            }
            Direction::In => {
                let end = first_node(&edge.source);
                let name = end.map(|node| graph.name(node));
                let named = if edge.names_source {
                    name.clone()
                } else {
                    Some(graph.name(here))
                };
                (end, name, named)
            }
        };
        // The end's title and kind, from the parse the graph already holds
        // for it: a subset read decodes every end file of the rows it reads
        // (canon `spec-read-subset`, "The subset"), so no file is read for
        // them (task spec `doc-modal` DM-1 and DM-4).
        let state = edge.state();
        let end_node = end
            .filter(|_| state == LinkState::Resolved)
            .and_then(|node| graph.node(node));
        let (kind, when) = label_strings(edge.edge.as_ref());
        ShownLink {
            direction,
            link_type: edge.link_type.clone(),
            origin: edge.origin,
            at: (here != at).then(|| graph.name(here)),
            name,
            title: end_node.and_then(|node| node.title.clone()),
            end_kind: end_node.and_then(|node| node.kind.clone()),
            written: edge.written.clone(),
            path: graph.paths()[edge.file].to_owned(),
            line: edge.line,
            state,
            reason: edge.reason().map(str::to_owned),
            kind,
            when,
            named,
            offset: edge.offset,
        }
    };
    let mut outgoing: Vec<ShownLink> = found
        .outgoing
        .iter()
        .map(|&(index, here)| link(index, Direction::Out, here))
        .collect();
    let mut incoming: Vec<ShownLink> = found
        .incoming
        .iter()
        .map(|&(index, landed)| link(index, Direction::In, landed))
        .collect();
    outgoing.sort_by(|a, b| order(a).cmp(&order(b)));
    incoming.sort_by(|a, b| order(a).cmp(&order(b)));
    let mut left_out = LeftOut::default();
    for &index in &found.left_out {
        left_out.add(graph.standing(edges[index].file));
    }
    let usage = usage_of_rows(graph, &incoming, graph.paths()[at.file]);
    ShownLinks {
        outgoing,
        incoming,
        left_out,
        usage,
    }
}

/// The counts of the In rows (task spec `usage-orphans` UO1, UO6): each a
/// [`UseRow`], its source the row's name, live when its file is; whatever
/// the call admitted, only live rows count.
fn usage_of_rows(graph: &SpecGraph<'_>, incoming: &[ShownLink], own_file: &str) -> LinkUsage {
    let (used_by, mentioned_by) = tally(
        incoming.iter().map(|link| UseRow {
            source: link.name.as_deref().unwrap_or(&link.path),
            file: &link.path,
            weak: is_weak_link(&link.link_type),
            live: graph
                .file_of(&link.path)
                .is_some_and(|file| graph.standing(file) == Standing::Live),
            resolved: link.state == LinkState::Resolved,
        }),
        own_file,
    );
    LinkUsage {
        used_by,
        mentioned_by,
    }
}

/// The sort key: strong before `mentions`, then (type, path, line,
/// column).
fn order(link: &ShownLink) -> (bool, &str, &str, usize, usize) {
    (
        is_weak_link(&link.link_type),
        &link.link_type,
        &link.path,
        link.line,
        link.offset,
    )
}

/// The block's lines, without line ends: one per link, outgoing first,
/// then the count and the uses (task spec `usage-orphans` UO6).
pub(crate) fn block_lines(links: &ShownLinks) -> Vec<String> {
    let mut lines: Vec<String> = links
        .outgoing
        .iter()
        .chain(&links.incoming)
        .map(link_line)
        .collect();
    lines.push(format!(
        "  links {} out, {} in; used by {}, mentioned by {}{}",
        links.outgoing.len(),
        links.incoming.len(),
        links.usage.used_by,
        links.usage.mentioned_by,
        links.left_out.suffix()
    ));
    lines
}

/// One link's line, two spaces in (`spec health` prints its out lines so).
pub(crate) fn link_line(link: &ShownLink) -> String {
    let shown = link.name.as_deref().unwrap_or(&link.written);
    let mut line = format!(
        "  {} {} {} | {}:{}",
        link.direction.as_str(),
        one_line(&link.link_type),
        one_line(shown),
        one_line(&link.path),
        link.line
    );
    if let Some(at) = &link.at {
        line.push_str(&format!(" | at {}", one_line(at)));
    }
    if link
        .named
        .as_deref()
        .is_some_and(|named| named != link.written)
    {
        line.push_str(&format!(" | as {}", one_line(&link.written)));
    }
    line.push_str(&label_suffix(link.kind.as_deref(), link.when.as_deref()));
    line.push_str(&state_suffix(link.state, link.reason.as_deref()));
    line
}

/// A label's kind and condition as strings. Also `spec graph`'s.
pub(crate) fn label_strings(label: Option<&EdgeLabel>) -> (Option<String>, Option<String>) {
    label.map_or((None, None), |label| {
        (
            label.kind.map(|kind| kind.as_str().to_owned()),
            label.when.clone(),
        )
    })
}

/// ` | kind <kind>`, ` | when <when>`, each when set. Also `spec graph`'s.
pub(crate) fn label_suffix(kind: Option<&str>, when: Option<&str>) -> String {
    let mut suffix = String::new();
    if let Some(kind) = kind {
        suffix.push_str(&format!(" | kind {}", one_line(kind)));
    }
    if let Some(when) = when {
        suffix.push_str(&format!(" | when {}", one_line(when)));
    }
    suffix
}

/// ` | dangling: <reason>`, ` | skipped: another project`, ` | unchecked`;
/// nothing for a resolved link. Also `spec graph`'s.
pub(crate) fn state_suffix(state: LinkState, reason: Option<&str>) -> String {
    match state {
        LinkState::Resolved => String::new(),
        LinkState::Dangling => format!(" | dangling: {}", one_line(reason.unwrap_or_default())),
        LinkState::Skipped => " | skipped: another project".to_owned(),
        LinkState::Unchecked => " | unchecked".to_owned(),
    }
}

#[derive(Serialize)]
pub(crate) struct LinksJson<'a> {
    outgoing: Vec<LinkJson<'a>>,
    incoming: Vec<LinkJson<'a>>,
    /// Uncut: the counts of every In row, whatever the cap shows.
    usage: LinkUsage,
    left_out: LeftOut,
    /// Links the cap cut (0 when nothing was).
    omitted: usize,
}

/// One link as JSON: `spec show --links`'s, and `spec health`'s. `kind` and
/// `when` are the one exception to "every key present": a plain link has
/// neither, so the documents of a corpus without labels keep their keys.
#[derive(Serialize)]
pub(crate) struct LinkJson<'a> {
    #[serde(rename = "type")]
    link_type: &'a str,
    origin: LinkOrigin,
    at: Option<&'a str>,
    name: Option<&'a str>,
    /// Always present, `null` when unresolved or untitled (task spec
    /// `doc-modal` DM-1).
    title: Option<&'a str>,
    end_kind: Option<&'a str>,
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

/// The first `kept` links (outgoing, then incoming) as JSON; `omitted`:
/// the links the cap cut.
pub(crate) fn links_json(links: &ShownLinks, kept: usize, omitted: usize) -> LinksJson<'_> {
    let outgoing = kept.min(links.outgoing.len());
    let incoming = (kept - outgoing).min(links.incoming.len());
    LinksJson {
        outgoing: links.outgoing[..outgoing].iter().map(link_json).collect(),
        incoming: links.incoming[..incoming].iter().map(link_json).collect(),
        usage: links.usage,
        left_out: links.left_out,
        omitted,
    }
}

pub(crate) fn link_json(link: &ShownLink) -> LinkJson<'_> {
    LinkJson {
        link_type: &link.link_type,
        origin: link.origin,
        at: link.at.as_deref(),
        name: link.name.as_deref(),
        title: link.title.as_deref(),
        end_kind: link.end_kind.as_deref(),
        written: &link.written,
        path: &link.path,
        line: link.line,
        state: link.state.as_str(),
        reason: link.reason.as_deref(),
        kind: link.kind.as_deref(),
        when: link.when.as_deref(),
    }
}
