//! The output cap of `spec show` and its two renderings.
//!
//! Text: per node a header line (ending ` | span b3:<hex>`, the node's
//! `span_hash`, after ` | norm b3:<hex>`, its `norm_hash`, when not null),
//! then its text (a line end added when it has none), an
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
//!
//! With `--bindings` (task spec `show-bindings`) each node's bindings
//! block follows its links block, before its text, and counts toward the
//! cap with it: one line per row (`  <node> | <relation> | <place> |
//! <binding> | layer <layer> | sync <sync>[ <category>][ <detail>]`), the
//! lines under it four spaces in (`sig`, `moved from`, `candidate`), then
//! the closing count, then one `  tested | <state>[ <category>] | <node>`
//! line per `[[tested]]` block of the node and its sections (task spec
//! `tested-reads`), by node bytes, counted with the closing line as one
//! unit. When the cut falls in the block it falls after the last whole
//! row (its line and the lines under it), or after the closing unit, that
//! fits; the text is not shown and the tail gains `; bindings not shown:
//! <k>` after its links part (every row of the cut node and of the nodes
//! after it that is not printed). JSON then takes the text's cut: a
//! node's `bindings` `{rows, omitted, tested}` holds the printed rows,
//! `omitted` the tail's `<k>` on the cut node, 0 on the others, and the
//! printed blocks (`null` when not judged).

use serde::Serialize;
use specengine_core::verify::Class;
use specengine_model::{BindingPlace, BindingRow, TestedRow};

use crate::links::{LinksJson, block_lines, links_json};
use crate::one_line;
use crate::search::notes;
use crate::show::{NestedSection, ShowOutcome, ShownBindings, ShownNode};
use crate::tested::tested_line;
use crate::verify::keyed_binding;

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
    /// Whole rows of the node's bindings block printed; JSON keeps as
    /// many rows.
    rows: usize,
    /// The block's closing line is printed (the cut falls in the text).
    closing: bool,
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
/// ` | norm b3:<hex>` (its `norm_hash`, when not null) and
/// ` | span b3:<hex>` (its `span_hash`). [`header`], shared with
/// `spec bundle`, carries neither.
fn shown_header(node: &ShownNode) -> String {
    let mut line = header(node);
    if let Some(norm) = &node.norm_hash {
        line.push_str(" | norm ");
        line.push_str(&one_line(norm));
    }
    line.push_str(" | span ");
    line.push_str(&one_line(&node.span_hash));
    line
}

/// The lines of a node's links block (none without `--links`).
fn block(node: &ShownNode) -> Vec<String> {
    node.links.as_ref().map(block_lines).unwrap_or_default()
}

/// A node's bindings block (none without `--bindings` or when no row was
/// read): each row's lines, then the closing line and its `tested` lines.
struct BindingsBlock {
    rows: Vec<Vec<String>>,
    /// The closing line, then one line per block (task spec
    /// `tested-reads`): one unit of the cap.
    closing: Vec<String>,
}

impl BindingsBlock {
    fn of(node: &ShownNode) -> Self {
        let Some(bindings) = &node.bindings else {
            return Self {
                rows: Vec::new(),
                closing: Vec::new(),
            };
        };
        let rows = bindings
            .rows
            .iter()
            .zip(&bindings.shown)
            .map(|(row, (binding, _))| row_lines(row, binding))
            .collect();
        let mut closing = vec![closing_line(bindings)];
        closing.extend(
            bindings
                .tested
                .iter()
                .flatten()
                .map(|row| format!("  {}", tested_line(row))),
        );
        Self { rows, closing }
    }

    /// Each row's characters, line ends included.
    fn row_costs(&self) -> Vec<usize> {
        self.rows.iter().map(|lines| chars(lines)).collect()
    }

    /// The closing line's and its `tested` lines' characters, line ends
    /// included.
    fn closing_cost(&self) -> usize {
        chars(&self.closing)
    }
}

/// The characters of `lines`, a line end after each.
fn chars(lines: &[String]) -> usize {
    lines.iter().map(|line| line.chars().count() + 1).sum()
}

/// A row's line and the lines under it: `sig`, a moved row's old place,
/// an ambiguous row's candidates. Also the brief's Bindings section (task
/// spec `bundle-bindings`).
pub(crate) fn row_lines(row: &BindingRow, binding: &str) -> Vec<String> {
    let mut lines = vec![format!("  {}", row_line(row, binding))];
    if let Some(signature) = &row.signature {
        lines.push(format!("    sig {}", one_line(signature)));
    }
    if let Some(anchor) = &row.anchor {
        if let Some(from) = &anchor.from {
            lines.push(format!("    moved from {}", place_text(from)));
        }
        for candidate in anchor.candidates.iter().flatten() {
            lines.push(format!("    candidate {}", place_text(candidate)));
        }
    }
    lines
}

/// A row's line without its indent: `<node> | <relation> | <place> |
/// <binding> | layer <layer> | sync <sync>[ <category>][ <detail>]`, `-`
/// for `null` (also a bundle row's first line, task spec
/// `bundle-bindings`).
pub(crate) fn row_line(row: &BindingRow, binding: &str) -> String {
    let place = match (row.line, row.end_line) {
        (Some(line), Some(end)) => format!("{}:{line}-{end}", one_line(&row.file)),
        (Some(line), None) => format!("{}:{line}", one_line(&row.file)),
        _ => one_line(&row.file),
    };
    let mut sync = row.sync.as_deref().map_or_else(|| "-".to_owned(), one_line);
    for part in [&row.category, &row.detail].into_iter().flatten() {
        sync.push(' ');
        sync.push_str(&one_line(part));
    }
    format!(
        "{} | {} | {place} | {} | layer {} | sync {sync}",
        one_line(&row.node),
        one_line(&row.relation),
        one_line(binding),
        row.layer
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
    )
}

/// `<file> | <kind> <symbol>[ <item_kind>]`.
fn place_text(place: &BindingPlace) -> String {
    let binding = keyed_binding(
        Some(&place.kind),
        Some(&place.symbol),
        place.item_kind.as_deref(),
    )
    .unwrap_or_default();
    format!("{} | {}", one_line(&place.file), one_line(&binding))
}

/// `  bindings <n>[, <f> fix][, <c> check][, <k> cannot]`; rows listed,
/// never judged: `  bindings <n>, sync -`.
fn closing_line(bindings: &ShownBindings) -> String {
    let mut line = format!("  bindings {}", bindings.rows.len());
    if !bindings.judged {
        line.push_str(", sync -");
        return line;
    }
    for class in [Class::Fix, Class::Check, Class::Cannot] {
        let word = class.as_str();
        let count = bindings
            .shown
            .iter()
            .filter(|(_, of)| *of == Some(word))
            .count();
        if count > 0 {
            line.push_str(&format!(", {count} {word}"));
        }
    }
    line
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
        let links = chars(&block);
        let rows = BindingsBlock::of(node);
        let row_costs = rows.row_costs();
        let bindings = row_costs.iter().sum::<usize>() + rows.closing_cost();
        let body = node.text.chars().count() + usize::from(!node.text.ends_with('\n'));
        if used + head + links + bindings + body <= OUTPUT_CAP_CHARS {
            used += head + links + bindings + body;
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
                rows: 0,
                closing: false,
                shown: 0,
            });
        }
        if used + head + links > OUTPUT_CAP_CHARS {
            // In the links block, at the end of the last line that fits.
            let room = OUTPUT_CAP_CHARS - used - head;
            let costs = block.iter().map(|line| line.chars().count() + 1);
            return Some(Cut {
                node: index,
                header: Header::Whole,
                links: whole_within(costs, room),
                rows: 0,
                closing: false,
                shown: 0,
            });
        }
        if used + head + links + bindings > OUTPUT_CAP_CHARS {
            // In the bindings block, after the last whole row that fits.
            let room = OUTPUT_CAP_CHARS - used - head - links;
            return Some(Cut {
                node: index,
                header: Header::Whole,
                links: block.len(),
                rows: whole_within(row_costs.iter().copied(), room),
                closing: false,
                shown: 0,
            });
        }
        let room = OUTPUT_CAP_CHARS - used - head - links - bindings;
        return Some(Cut {
            node: index,
            header: Header::Whole,
            links: block.len(),
            rows: row_costs.len(),
            closing: true,
            shown: cut_text(&node.text, room, true),
        });
    }
    None
}

/// How many of `costs`, from the first, fit whole in `room`.
fn whole_within(costs: impl IntoIterator<Item = usize>, mut room: usize) -> usize {
    let mut fit = 0;
    for cost in costs {
        if cost > room {
            break;
        }
        room -= cost;
        fit += 1;
    }
    fit
}

/// Link lines of the text rendering not printed: the cut node's beyond
/// the block lines printed, and every one of the nodes after it.
fn links_not_shown(nodes: &[ShownNode], cut: Cut) -> usize {
    let count = |node: &ShownNode| node.links.as_ref().map_or(0, |links| links.len());
    let cut_node = count(&nodes[cut.node]);
    cut_node - cut.links.min(cut_node) + nodes[cut.node + 1..].iter().map(count).sum::<usize>()
}

/// Rows of the text rendering not printed: the cut node's beyond the
/// whole rows printed, and every one of the nodes after it.
fn bindings_not_shown(nodes: &[ShownNode], cut: Cut) -> usize {
    let count = |node: &ShownNode| node.bindings.as_ref().map_or(0, |rows| rows.rows.len());
    let cut_node = count(&nodes[cut.node]);
    cut_node - cut.rows.min(cut_node) + nodes[cut.node + 1..].iter().map(count).sum::<usize>()
}

/// The cut of the JSON rendering without `--links` or rows (the sum of
/// the texts), if any.
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
            rows: 0,
            closing: false,
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
        let rows = BindingsBlock::of(node);
        let printed = this_cut.map_or(rows.rows.len(), |cut| cut.rows.min(rows.rows.len()));
        for line in rows.rows[..printed].iter().flatten() {
            out.push_str(line);
            out.push('\n');
        }
        if this_cut.is_none_or(|cut| cut.closing) {
            for line in &rows.closing {
                out.push_str(line);
                out.push('\n');
            }
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
        let mut links = if nodes.iter().any(|node| node.links.is_some()) {
            format!("; links not shown: {}", links_not_shown(nodes, cut))
        } else {
            String::new()
        };
        if nodes.iter().any(|node| node.bindings.is_some()) {
            links.push_str(&format!(
                "; bindings not shown: {}",
                bindings_not_shown(nodes, cut)
            ));
        }
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
    norm_hash: Option<&'a str>,
    span_hash: &'a str,
    text: &'a str,
    truncated: bool,
    omitted: Option<OmittedJson>,
    /// `null` without `--links`.
    links: Option<LinksJson<'a>>,
    /// `null` without `--bindings` or when no row could be read.
    bindings: Option<BindingsJson<'a>>,
}

/// A node's rows: those printed, and the rows the cap cut (0 when none
/// was); its `[[tested]]` blocks printed (none when the cap cut its
/// closing line), `null` when not judged.
#[derive(Serialize)]
struct BindingsJson<'a> {
    rows: &'a [BindingRow],
    omitted: usize,
    tested: Option<&'a [TestedRow]>,
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
    // With `--links` or rows: the text's cut, so the JSON holds the
    // printed links and rows.
    let cut = if outcome.view == View::Browser {
        None
    } else if nodes
        .iter()
        .any(|node| node.links.is_some() || node.bindings.is_some())
    {
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
                norm_hash: node.norm_hash.as_deref(),
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
                bindings: node.bindings.as_ref().map(|bindings| match this_cut {
                    Some(cut) => BindingsJson {
                        rows: &bindings.rows[..cut.rows.min(bindings.rows.len())],
                        omitted: bindings_not_shown(nodes, cut),
                        tested: bindings
                            .tested
                            .as_deref()
                            .map(|tested| if cut.closing { tested } else { &[] }),
                    },
                    None => BindingsJson {
                        rows: &bindings.rows,
                        omitted: 0,
                        tested: bindings.tested.as_deref(),
                    },
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
