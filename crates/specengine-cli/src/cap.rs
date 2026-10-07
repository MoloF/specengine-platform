//! The output cap of `spec show` and its two renderings.
//!
//! Text: per node a header line (ending ` | span b3:<hex>`, the node's
//! `span_hash`), then its text (a line end added when it has none), an
//! empty line between nodes. Everything before the tail line
//! is at most [`OUTPUT_CAP_CHARS`] characters (JSON: the sum of the `text`
//! values). The node where the cap falls is cut at the last line end within
//! it; when its first line alone is longer than the room left, at the cap
//! itself (text: one character earlier, so the added line end fits; a
//! header line longer than the whole cap, the output's first line, is cut
//! the same way). Then, in text, one tail line:
//!
//! ```text
//! [truncated: <path> lines <a>-<b> not shown; sections not shown: <IDs or none>; holders not shown: <path:line, … or none>]
//! ```
//!
//! A section is not shown when its heading line is cut. Each list names at
//! most [`SHOW_TAIL_NAMES`]: the first hidden sections (source order), the
//! first holders not shown (print order); a longer one ends `, <k> more`,
//! `k` the rest. JSON gives the cut node `truncated: true`, `sections` only
//! the IDs whose heading line ends within its `text`, and `omitted: {lines,
//! sections, sections_more, holders, holders_more}` (the names by the same
//! rule at the JSON's cut, `*_more` the `k`, 0 when every name is listed),
//! and drops the nodes after it.
//!
//! With `--links` each node's links block follows its header line, before
//! its text, and counts toward the cap: the cut falls in the text first;
//! when it falls in the block, at a line end, the text is not shown and the
//! tail gains `; links not shown: <k>` (every link line of the cut node and
//! of the nodes after it that is not printed; `0` when none). JSON then
//! takes the text's cut, so it holds exactly what the text prints: the same
//! nodes, the cut node's text as printed, each node's `links` (`null`
//! without `--links`) the printed ones (outgoing, then incoming) and
//! `omitted` the tail's `<k>` on the cut node, 0 on the others.

use serde::Serialize;

use crate::links::{LinksJson, block_lines, links_json};
use crate::one_line;
use crate::search::notes;
use crate::show::{NestedSection, ShowOutcome, ShownNode};

/// The most characters a `show` or `search` answer prints before its tail
/// line (07 §1.1: bounded answers).
pub const OUTPUT_CAP_CHARS: usize = 40_000;

/// The most names each list of a `show` tail gives (hidden sections,
/// holders not shown), as its JSON `omitted`; the rest are counted.
pub const SHOW_TAIL_NAMES: usize = 20;

/// How a `tree`, `show`, `search` or `graph` answer is bounded (task specs
/// `daemon-read`, `ui-live`, "Data"): the CLI and MCP read
/// [`View::Capped`], the daemon [`View::Browser`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum View {
    /// Cut at [`OUTPUT_CAP_CHARS`] as the module documentation says; a
    /// hit's snippet one text, matches in `**`, cuts `…`.
    #[default]
    Capped,
    /// Never cut (`truncated` false, `omitted` null, the same keys); a
    /// hit's snippet its structure ([`specengine_store::Snippet`], `null`
    /// when empty). An uncut answer costs an agent's context: not for MCP.
    Browser,
}

/// Where the cap falls.
#[derive(Debug, Clone, Copy)]
struct Cut {
    /// The node it falls in.
    node: usize,
    /// The text rendering's: how much of the node's header line is
    /// printed.
    header: Header,
    /// Lines of the node's links block printed (all of them when the cut
    /// falls in its text); JSON with `--links` keeps as many links.
    links: usize,
    /// Bytes of the node's text shown.
    shown: usize,
}

/// The cut node's header line in the text rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Header {
    /// Printed whole.
    Whole,
    /// The output's first line, longer than the cap: its first bytes
    /// (a character boundary), then a line end.
    Cut(usize),
    /// Not printed: the cut falls at the line end before it.
    Dropped,
}

/// What a cut leaves out.
struct Omitted {
    /// First and last line of the cut node not shown.
    lines: [usize; 2],
    /// The first [`SHOW_TAIL_NAMES`] hidden sections, in source order.
    sections: Vec<String>,
    /// Hidden sections past them.
    sections_more: usize,
    /// `path:line` of the first [`SHOW_TAIL_NAMES`] nodes after the cut
    /// one, in print order.
    holders: Vec<String>,
    /// Nodes after the cut one past them.
    holders_more: usize,
}

/// `<ID else path> | <kind or -> | <title or -> | <path>:<line> | <n> tokens`
/// and, when they apply, ` | status <s>`, ` | rev <n>`, ` | archived`,
/// ` | not UTF-8`. Also `spec bundle`'s target headers, which carry no
/// span hash (task spec `proposal-apply` names `spec show` only).
pub(crate) fn header(node: &ShownNode) -> String {
    let name = node.id.as_deref().unwrap_or(&node.path);
    let kind = node.kind.as_deref().unwrap_or("-");
    let title = node
        .title
        .as_deref()
        .map_or_else(|| "-".to_owned(), one_line);
    let mut header = format!(
        "{} | {} | {title} | {}:{} | {} tokens",
        one_line(name),
        one_line(kind),
        one_line(&node.path),
        node.line,
        node.tokens_est
    );
    if let Some(status) = &node.status {
        header.push_str(&format!(" | status {}", one_line(status)));
    }
    if let Some(rev) = node.rev {
        header.push_str(&format!(" | rev {rev}"));
    }
    if node.archived {
        header.push_str(" | archived");
    }
    if !node.utf8 {
        header.push_str(" | not UTF-8");
    }
    header
}

/// `spec show`'s header line of a node: [`header`], then
/// ` | span b3:<hex>` (its `span_hash`).
fn shown_header(node: &ShownNode) -> String {
    format!("{} | span {}", header(node), one_line(&node.span_hash))
}

/// The lines of a node's links block (none without `--links`).
fn block(node: &ShownNode) -> Vec<String> {
    node.links.as_ref().map(block_lines).unwrap_or_default()
}

/// `spec tree` and `spec graph`: how many of `lines` (their characters,
/// line end included), from the first, fit in [`OUTPUT_CAP_CHARS`] after
/// `used` characters, and the characters then used; `first_whole`: never
/// fewer than one when there is a line.
pub(crate) fn lines_within(
    lines: impl IntoIterator<Item = usize>,
    mut used: usize,
    first_whole: bool,
) -> (usize, usize) {
    let mut fit = 0;
    for cost in lines {
        if used + cost > OUTPUT_CAP_CHARS && !(first_whole && fit == 0) {
            break;
        }
        used += cost;
        fit += 1;
    }
    (fit, used)
}

/// The cut of the text rendering, if any.
fn plan_text(nodes: &[ShownNode]) -> Option<Cut> {
    let mut used = 0;
    for (index, node) in nodes.iter().enumerate() {
        let head = usize::from(index > 0) + shown_header(node).chars().count() + 1;
        let block = block(node);
        let links: usize = block.iter().map(|line| line.chars().count() + 1).sum();
        let body = node.text.chars().count() + usize::from(!node.text.ends_with('\n'));
        if used + head + links + body <= OUTPUT_CAP_CHARS {
            used += head + links + body;
            continue;
        }
        if used + head > OUTPUT_CAP_CHARS {
            // No line end before it within the cap: the header is the first
            // line, cut at the cap less its line end. Else the cut falls at
            // the line end that ends the previous node.
            let header = if used == 0 {
                Header::Cut(cut_text(&shown_header(node), OUTPUT_CAP_CHARS, true))
            } else {
                Header::Dropped
            };
            return Some(Cut {
                node: index,
                header,
                links: 0,
                shown: 0,
            });
        }
        if used + head + links > OUTPUT_CAP_CHARS {
            // In the links block, at the end of the last line that fits.
            let mut room = OUTPUT_CAP_CHARS - used - head;
            let mut printed = 0;
            for line in &block {
                let cost = line.chars().count() + 1;
                if cost > room {
                    break;
                }
                room -= cost;
                printed += 1;
            }
            return Some(Cut {
                node: index,
                header: Header::Whole,
                links: printed,
                shown: 0,
            });
        }
        let room = OUTPUT_CAP_CHARS - used - head - links;
        return Some(Cut {
            node: index,
            header: Header::Whole,
            links: block.len(),
            shown: cut_text(&node.text, room, true),
        });
    }
    None
}

/// Link lines of the text rendering not printed: the cut node's beyond
/// the block lines printed, and every one of the nodes after it.
fn links_not_shown(nodes: &[ShownNode], cut: Cut) -> usize {
    let count = |node: &ShownNode| node.links.as_ref().map_or(0, |links| links.len());
    let cut_node = count(&nodes[cut.node]);
    cut_node - cut.links.min(cut_node) + nodes[cut.node + 1..].iter().map(count).sum::<usize>()
}

/// The cut of the JSON rendering without `--links` (the sum of the
/// texts), if any.
fn plan_json(nodes: &[ShownNode]) -> Option<Cut> {
    let mut used = 0;
    for (index, node) in nodes.iter().enumerate() {
        let body = node.text.chars().count();
        if used + body <= OUTPUT_CAP_CHARS {
            used += body;
            continue;
        }
        return Some(Cut {
            node: index,
            header: Header::Whole,
            links: 0,
            shown: cut_text(&node.text, OUTPUT_CAP_CHARS - used, false),
        });
    }
    None
}

/// Bytes of `text` to show in `room` characters: through the last line end
/// within them; with none, `room` characters (one fewer when a line end
/// must still fit after them).
fn cut_text(text: &str, room: usize, newline_after: bool) -> usize {
    let mut last_end = None;
    for (offset, char) in text.char_indices().take(room) {
        if char == '\n' {
            last_end = Some(offset + 1);
        }
    }
    last_end.unwrap_or_else(|| {
        let chars = if newline_after {
            room.saturating_sub(1)
        } else {
            room
        };
        text.char_indices()
            .nth(chars)
            .map_or(text.len(), |(offset, _)| offset)
    })
}

/// The names of the first [`SHOW_TAIL_NAMES`] of `items` and how many items
/// follow them (counted, not named).
fn first_names<T>(
    mut items: impl Iterator<Item = T>,
    name: impl FnMut(T) -> String,
) -> (Vec<String>, usize) {
    let first = items.by_ref().take(SHOW_TAIL_NAMES).map(name).collect();
    (first, items.count())
}

/// A nested section's heading line ends within the cut node's shown bytes.
fn section_shown(section: &NestedSection, cut: Cut) -> bool {
    section.heading_end <= cut.shown
}

fn omitted(nodes: &[ShownNode], cut: Cut) -> Omitted {
    let node = &nodes[cut.node];
    let shown = &node.text.as_bytes()[..cut.shown];
    let first = node.line + shown.iter().filter(|&&byte| byte == b'\n').count();
    let (sections, sections_more) = first_names(
        node.sections
            .iter()
            .filter(|section| !section_shown(section, cut)),
        |section| section.id.clone(),
    );
    let (holders, holders_more) = first_names(nodes[cut.node + 1..].iter(), |node| {
        format!("{}:{}", node.path, node.line)
    });
    Omitted {
        lines: [first, node.end_line.max(first)],
        sections,
        sections_more,
        holders,
        holders_more,
    }
}

/// `none`, the names, or the names then `, <more> more`.
fn list_or_none(items: &[String], more: usize) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else if more == 0 {
        items.join(", ")
    } else {
        format!("{}, {more} more", items.join(", "))
    }
}

pub(crate) fn render_text(outcome: &ShowOutcome) -> String {
    let nodes = &outcome.nodes;
    let cut = match outcome.view {
        View::Capped => plan_text(nodes),
        View::Browser => None,
    };
    let mut out = String::new();
    for (index, node) in nodes.iter().enumerate() {
        let this_cut = cut.filter(|cut| cut.node == index);
        let header = shown_header(node);
        match this_cut.map(|cut| cut.header) {
            Some(Header::Dropped) => break,
            Some(Header::Cut(bytes)) => {
                // Only ever the first node: nothing precedes it.
                out.push_str(&header[..bytes]);
                out.push('\n');
                break;
            }
            Some(Header::Whole) | None => {}
        }
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&header);
        out.push('\n');
        let block = block(node);
        let printed = this_cut.map_or(block.len(), |cut| cut.links.min(block.len()));
        for line in &block[..printed] {
            out.push_str(line);
            out.push('\n');
        }
        let text = match this_cut {
            Some(cut) => &node.text[..cut.shown],
            None => node.text.as_str(),
        };
        out.push_str(text);
        if !text.is_empty() && !text.ends_with('\n') {
            out.push('\n');
        }
        if this_cut.is_some() {
            break;
        }
    }
    if let Some(cut) = cut {
        let omitted = omitted(nodes, cut);
        let links = if nodes.iter().any(|node| node.links.is_some()) {
            format!("; links not shown: {}", links_not_shown(nodes, cut))
        } else {
            String::new()
        };
        // One line whatever the paths hold.
        out.push_str(&one_line(&format!(
            "[truncated: {} lines {}-{} not shown; sections not shown: {}; holders not shown: {}{links}]",
            nodes[cut.node].path,
            omitted.lines[0],
            omitted.lines[1],
            list_or_none(&omitted.sections, omitted.sections_more),
            list_or_none(&omitted.holders, omitted.holders_more)
        )));
        out.push('\n');
    }
    out
}

#[derive(Serialize)]
struct ShowJson<'a> {
    #[serde(rename = "ref")]
    reference: &'a str,
    reason: Option<&'a str>,
    notes: Vec<String>,
    nodes: Vec<NodeJson<'a>>,
}

#[derive(Serialize)]
struct NodeJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    end_line: usize,
    status: Option<&'a str>,
    rev: Option<u32>,
    tokens_est: u32,
    archived: bool,
    utf8: bool,
    sections: Vec<&'a str>,
    span_hash: &'a str,
    text: &'a str,
    truncated: bool,
    omitted: Option<OmittedJson>,
    /// `null` without `--links`.
    links: Option<LinksJson<'a>>,
}

#[derive(Serialize)]
struct OmittedJson {
    lines: [usize; 2],
    sections: Vec<String>,
    sections_more: usize,
    holders: Vec<String>,
    holders_more: usize,
}

fn view(outcome: &ShowOutcome) -> ShowJson<'_> {
    let nodes = &outcome.nodes;
    // With `--links`: the text's cut, so the JSON holds the printed links.
    let cut = if outcome.view == View::Browser {
        None
    } else if nodes.iter().any(|node| node.links.is_some()) {
        plan_text(nodes)
    } else {
        plan_json(nodes)
    };
    let shown = cut.map_or(nodes.len(), |cut| cut.node + 1);
    let json_nodes = nodes[..shown]
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let this_cut = cut.filter(|cut| cut.node == index);
            NodeJson {
                id: node.id.as_deref(),
                kind: node.kind.as_deref(),
                title: node.title.as_deref(),
                path: &node.path,
                line: node.line,
                end_line: node.end_line,
                status: node.status.as_deref(),
                rev: node.rev,
                tokens_est: node.tokens_est,
                archived: node.archived,
                utf8: node.utf8,
                // A cut node: the sections whose heading line it prints.
                sections: node
                    .sections
                    .iter()
                    .filter(|section| this_cut.is_none_or(|cut| section_shown(section, cut)))
                    .map(|section| section.id.as_str())
                    .collect(),
                span_hash: &node.span_hash,
                text: match this_cut {
                    Some(cut) => &node.text[..cut.shown],
                    None => &node.text,
                },
                truncated: this_cut.is_some(),
                omitted: this_cut.map(|cut| {
                    let omitted = omitted(nodes, cut);
                    OmittedJson {
                        lines: omitted.lines,
                        sections: omitted.sections,
                        sections_more: omitted.sections_more,
                        holders: omitted.holders,
                        holders_more: omitted.holders_more,
                    }
                }),
                links: node.links.as_ref().map(|links| match this_cut {
                    Some(cut) => links_json(links, cut.links, links_not_shown(nodes, cut)),
                    None => links_json(links, usize::MAX, 0),
                }),
            }
        })
        .collect();
    ShowJson {
        reference: &outcome.reference,
        reason: outcome.reason.as_deref(),
        notes: notes(&outcome.messages),
        nodes: json_nodes,
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for ShowOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
