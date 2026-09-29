//! The body through `pulldown-cmark` with offsets: headings with their
//! attribute blocks, the text regions references are read from, the first H1
//! and the summary paragraph.
//!
//! Regions are the source ranges of text and inline-code events outside code
//! blocks and HTML, merged where they touch: references are read from the
//! original bytes (so `R\-12` is none) and never from fenced or indented
//! code, HTML (comments included), link destinations or attribute blocks.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use specengine_model::Span;

/// One heading of the body.
pub(crate) struct Heading {
    pub level: u8,
    /// The heading line(s), trailing whitespace and line ending excluded.
    pub span: Span,
    /// End of the heading's raw range: after its line ending.
    pub raw_end: usize,
    /// The `{#…}` id and the file offset of its text.
    pub id: Option<(String, Option<usize>)>,
    pub classes: Vec<String>,
    pub attrs: Vec<(String, Option<String>)>,
    /// Text of the heading without markup or attribute block.
    pub text: String,
}

pub(crate) struct Body {
    pub headings: Vec<Heading>,
    /// Merged source ranges to read references from, in order.
    pub regions: Vec<Range<usize>>,
    /// Index of the first level-1 heading.
    pub first_h1: Option<usize>,
    /// First paragraph after the first H1, before the next heading (no H1:
    /// before the first heading).
    pub summary: Option<Span>,
}

/// Scans `text[body]`; every offset returned is a file offset.
pub(crate) fn scan(text: &str, body: Span) -> Body {
    let source = &text[body.range()];
    let base = body.start;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut headings: Vec<Heading> = Vec::new();
    let mut regions: Vec<Range<usize>> = Vec::new();
    // First paragraph seen after each count of headings: (headings seen, span).
    let mut paragraphs: Vec<(usize, Span)> = Vec::new();
    let mut code_blocks = 0usize;
    let mut html_blocks = 0usize;
    let mut in_heading = false;

    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        let range = range.start + base..range.end + base;
        match event {
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => {
                let span = trimmed(text, range.clone());
                let id = id.map(|id| {
                    let id = id.to_string();
                    let offset = id_offset(text, span, &id);
                    (id, offset)
                });
                headings.push(Heading {
                    level: level as u8,
                    span,
                    raw_end: range.end,
                    id,
                    classes: classes.iter().map(ToString::to_string).collect(),
                    attrs: attrs
                        .iter()
                        .map(|(key, value)| {
                            (key.to_string(), value.as_ref().map(ToString::to_string))
                        })
                        .collect(),
                    text: String::new(),
                });
                in_heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                if let Some(heading) = headings.last_mut() {
                    let text = heading.text.trim();
                    if text.len() != heading.text.len() {
                        heading.text = text.to_owned();
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code_blocks += 1,
            Event::End(TagEnd::CodeBlock) => code_blocks = code_blocks.saturating_sub(1),
            Event::Start(Tag::HtmlBlock) => html_blocks += 1,
            Event::End(TagEnd::HtmlBlock) => html_blocks = html_blocks.saturating_sub(1),
            Event::Start(Tag::Paragraph) => {
                let seen = headings.len();
                if paragraphs.last().is_none_or(|&(last, _)| last != seen) {
                    paragraphs.push((seen, trimmed(text, range)));
                }
            }
            Event::Text(content) => {
                if code_blocks == 0 && html_blocks == 0 {
                    push_region(&mut regions, range);
                    if in_heading && let Some(heading) = headings.last_mut() {
                        heading.text.push_str(&content);
                    }
                }
            }
            Event::Code(content) => {
                push_region(&mut regions, range);
                if in_heading && let Some(heading) = headings.last_mut() {
                    heading.text.push_str(&content);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_heading && let Some(heading) = headings.last_mut() {
                    heading.text.push(' ');
                }
            }
            _ => {}
        }
    }

    let first_h1 = headings.iter().position(|heading| heading.level == 1);
    let wanted = first_h1.map_or(0, |index| index + 1);
    let summary = paragraphs
        .iter()
        .find(|&&(seen, _)| seen == wanted)
        .map(|&(_, span)| span);
    Body {
        headings,
        regions,
        first_h1,
        summary,
    }
}

/// Appends a range, merging it into the previous one when they touch.
fn push_region(regions: &mut Vec<Range<usize>>, range: Range<usize>) {
    if range.is_empty() {
        return;
    }
    match regions.last_mut() {
        Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
        _ => regions.push(range),
    }
}

/// The range without trailing whitespace (line endings included).
pub(crate) fn trimmed(text: &str, range: Range<usize>) -> Span {
    let slice = &text[range.clone()];
    let kept = slice.trim_end_matches([' ', '\t', '\r', '\n']).len();
    Span::new(range.start, range.start + kept)
}

/// File offset of the `{#id}` text inside the heading's attribute block.
fn id_offset(text: &str, heading: Span, id: &str) -> Option<usize> {
    let source = &text[heading.range()];
    let open = source.rfind('{')?;
    let block = &source[open..];
    let mut from = 0;
    while let Some(found) = block[from..].find('#') {
        let at = from + found + 1;
        let rest = &block[at..];
        if rest.starts_with(id)
            && rest[id.len()..]
                .chars()
                .next()
                .is_none_or(|c| c.is_ascii_whitespace() || c == '}')
        {
            return Some(heading.start + open + at);
        }
        from = at;
    }
    None
}
