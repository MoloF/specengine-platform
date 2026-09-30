//! The output cap of `spec show` and its two renderings.
//!
//! Text: per node a header line, then its text (a line end added when it
//! has none), an empty line between nodes. Everything before the tail line
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
//! A section is not shown when its heading line is cut. JSON gives the cut
//! node `truncated: true` and `omitted: {lines, sections, holders}` and
//! drops the nodes after it.

use serde::Serialize;

use crate::one_line;
use crate::search::notes;
use crate::show::{ShowOutcome, ShownNode};

/// The most characters a `show` or `search` answer prints before its tail
/// line (07 §1.1: bounded answers).
pub const OUTPUT_CAP_CHARS: usize = 40_000;

/// Where the cap falls.
#[derive(Debug, Clone, Copy)]
struct Cut {
    /// The node it falls in.
    node: usize,
    /// Text only: how much of the node's header line is printed.
    header: Header,
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
    sections: Vec<String>,
    /// `path:line` of every node after the cut one.
    holders: Vec<String>,
}

/// `<ID else path> | <kind or -> | <title or -> | <path>:<line> | <n> tokens`
/// and, when they apply, ` | status <s>`, ` | rev <n>`, ` | archived`,
/// ` | not UTF-8`.
fn header(node: &ShownNode) -> String {
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

/// The cut of the text rendering, if any.
fn plan_text(nodes: &[ShownNode]) -> Option<Cut> {
    let mut used = 0;
    for (index, node) in nodes.iter().enumerate() {
        let head = usize::from(index > 0) + header(node).chars().count() + 1;
        let body = node.text.chars().count() + usize::from(!node.text.ends_with('\n'));
        if used + head + body <= OUTPUT_CAP_CHARS {
            used += head + body;
            continue;
        }
        if used + head > OUTPUT_CAP_CHARS {
            // No line end before it within the cap: the header is the first
            // line, cut at the cap less its line end. Else the cut falls at
            // the line end that ends the previous node.
            let header = if used == 0 {
                Header::Cut(cut_text(&header(node), OUTPUT_CAP_CHARS, true))
            } else {
                Header::Dropped
            };
            return Some(Cut {
                node: index,
                header,
                shown: 0,
            });
        }
        let room = OUTPUT_CAP_CHARS - used - head;
        return Some(Cut {
            node: index,
            header: Header::Whole,
            shown: cut_text(&node.text, room, true),
        });
    }
    None
}

/// The cut of the JSON rendering (the sum of the texts), if any.
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

fn omitted(nodes: &[ShownNode], cut: Cut) -> Omitted {
    let node = &nodes[cut.node];
    let shown = &node.text.as_bytes()[..cut.shown];
    let first = node.line + shown.iter().filter(|&&byte| byte == b'\n').count();
    Omitted {
        lines: [first, node.end_line.max(first)],
        sections: node
            .sections
            .iter()
            .filter(|section| section.heading_end > cut.shown)
            .map(|section| section.id.clone())
            .collect(),
        holders: nodes[cut.node + 1..]
            .iter()
            .map(|node| format!("{}:{}", node.path, node.line))
            .collect(),
    }
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

pub(crate) fn render_text(outcome: &ShowOutcome) -> String {
    let nodes = &outcome.nodes;
    let cut = plan_text(nodes);
    let mut out = String::new();
    for (index, node) in nodes.iter().enumerate() {
        let this_cut = cut.filter(|cut| cut.node == index);
        let header = header(node);
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
        // One line whatever the paths hold.
        out.push_str(&one_line(&format!(
            "[truncated: {} lines {}-{} not shown; sections not shown: {}; holders not shown: {}]",
            nodes[cut.node].path,
            omitted.lines[0],
            omitted.lines[1],
            list_or_none(&omitted.sections),
            list_or_none(&omitted.holders)
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
    text: &'a str,
    truncated: bool,
    omitted: Option<OmittedJson>,
}

#[derive(Serialize)]
struct OmittedJson {
    lines: [usize; 2],
    sections: Vec<String>,
    holders: Vec<String>,
}

fn view(outcome: &ShowOutcome) -> ShowJson<'_> {
    let nodes = &outcome.nodes;
    let cut = plan_json(nodes);
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
                sections: node
                    .sections
                    .iter()
                    .map(|section| section.id.as_str())
                    .collect(),
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
                        holders: omitted.holders,
                    }
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
