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
//!   <out|in> <type> <name, else written> | <path>:<line>[ | at <section>][ | as <written>][ | dangling: <reason> | skipped: another project | unchecked]
//!   links <o> out, <i> in[; left out: <g> generated, <t> archived (--archive)]
//! ```
//!
//! `name`: the other end, its ID else its path; `at`: the nested section
//! that holds the link or that it lands on; `as`: the written form when it
//! is not the name of the node it names.

use serde::Serialize;
use specengine_core::check::{Endpoint, LinkState, NodeAt, SpecGraph};
use specengine_model::{Direction, LinkOrigin, is_weak_link};

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
    /// The reference or path as written.
    pub written: String,
    /// Where it is written.
    pub path: String,
    pub line: usize,
    pub state: LinkState,
    /// Why it dangles, or a remark on a resolved link (an anchor naming
    /// nothing).
    pub reason: Option<String>,
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
    let first_name = |end: &Endpoint| match end {
        Endpoint::Nodes(nodes) => nodes.first().map(|&node| graph.name(node)),
        _ => None,
    };
    // `here`: the node of `at`'s span that holds it or that it names as its
    // source (outgoing), or that it lands on (incoming).
    let link = |index: usize, direction: Direction, here: NodeAt| {
        let edge = &edges[index];
        let (name, named) = match direction {
            Direction::Out => {
                let name = first_name(&edge.target);
                let named = if edge.names_source {
                    Some(graph.name(here))
                } else {
                    name.clone()
                };
                (name, named)
            }
            Direction::In => {
                let name = first_name(&edge.source);
                let named = if edge.names_source {
                    name.clone()
                } else {
                    Some(graph.name(here))
                };
                (name, named)
            }
        };
        ShownLink {
            direction,
            link_type: edge.link_type.clone(),
            origin: edge.origin,
            at: (here != at).then(|| graph.name(here)),
            name,
            written: edge.written.clone(),
            path: graph.paths()[edge.file].to_owned(),
            line: edge.line,
            state: edge.state(),
            reason: edge.reason().map(str::to_owned),
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
    ShownLinks {
        outgoing,
        incoming,
        left_out,
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
/// then the count.
pub(crate) fn block_lines(links: &ShownLinks) -> Vec<String> {
    let mut lines: Vec<String> = links
        .outgoing
        .iter()
        .chain(&links.incoming)
        .map(link_line)
        .collect();
    lines.push(format!(
        "  links {} out, {} in{}",
        links.outgoing.len(),
        links.incoming.len(),
        links.left_out.suffix()
    ));
    lines
}

fn link_line(link: &ShownLink) -> String {
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
    line.push_str(&state_suffix(link.state, link.reason.as_deref()));
    line
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
    left_out: LeftOut,
    /// Links the cap cut (0 when nothing was).
    omitted: usize,
}

#[derive(Serialize)]
struct LinkJson<'a> {
    #[serde(rename = "type")]
    link_type: &'a str,
    origin: LinkOrigin,
    at: Option<&'a str>,
    name: Option<&'a str>,
    written: &'a str,
    path: &'a str,
    line: usize,
    state: &'static str,
    reason: Option<&'a str>,
}

/// The first `kept` links (outgoing, then incoming) as JSON; `omitted`:
/// the links the cap cut.
pub(crate) fn links_json(links: &ShownLinks, kept: usize, omitted: usize) -> LinksJson<'_> {
    let outgoing = kept.min(links.outgoing.len());
    let incoming = (kept - outgoing).min(links.incoming.len());
    LinksJson {
        outgoing: links.outgoing[..outgoing].iter().map(link_json).collect(),
        incoming: links.incoming[..incoming].iter().map(link_json).collect(),
        left_out: links.left_out,
        omitted,
    }
}

fn link_json(link: &ShownLink) -> LinkJson<'_> {
    LinkJson {
        link_type: &link.link_type,
        origin: link.origin,
        at: link.at.as_deref(),
        name: link.name.as_deref(),
        written: &link.written,
        path: &link.path,
        line: link.line,
        state: link.state.as_str(),
        reason: link.reason.as_deref(),
    }
}
