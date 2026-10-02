//! A node's own text, as spans: its body minus every ID section inside it.
//! One definition for the index's `own_text` column (the store joins the
//! pieces) and the check's `text` rule (task spec `spec-check-process`).

use specengine_model::{ParsedFile, Span};

/// The pieces of the node at `index`'s own text, in source order, none
/// empty: its body (a section's `body`; the document's the file's `body`)
/// minus every ID section inside it. Nested sections are indexed on their
/// own, so a word belongs to exactly one node. No node at `index`: none.
///
/// Sections come in source order, so only those after `index` can lie in
/// its range, and the scan stops at the first starting at or after the
/// range's end: linear in the node's descendants, not in the file's nodes.
pub fn own_spans(parsed: &ParsedFile, index: usize) -> Vec<Span> {
    let Some(node) = parsed.nodes.get(index) else {
        return Vec::new();
    };
    let range = if index == 0 {
        parsed.body
    } else {
        node.body.unwrap_or(Span::new(node.span.end, node.span.end))
    };
    let mut pieces = Vec::new();
    let mut push = |start: usize, end: usize| {
        if start < end {
            pieces.push(Span::new(start, end));
        }
    };
    let mut cursor = range.start;
    for section in parsed.nodes.iter().skip(index + 1) {
        if section.span.start >= range.end {
            break;
        }
        if !range.contains(section.span) || section.span.start < cursor {
            continue;
        }
        push(cursor, section.span.start);
        cursor = section.span.end;
    }
    push(cursor, range.end);
    pieces
}
