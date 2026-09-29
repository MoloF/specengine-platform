//! The body through `pulldown-cmark` with offsets: headings with their
//! attribute blocks, the text regions references are read from, the first H1
//! and the summary paragraph.
//!
//! Regions are the source ranges of text and inline-code events outside code
//! blocks and HTML, merged where they touch: references are read from the
//! original bytes (so `R\-12` is none) and never from fenced or indented
//! code, HTML (comments included), link destinations or attribute blocks.
//!
//! Anchors: each heading's GitHub slug is computed from its inline text
//! (text and code, link text included; no destination, image or HTML), and
//! `<a id>` / `<a name>` start tags are read from HTML events only, so
//! nothing comes from code blocks or HTML comments.

use std::collections::BTreeMap;
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
    /// What the slug is made of: `text` without image alt text.
    pub slug_text: String,
}

/// One `<a id>` / `<a name>` value and its start tag.
pub(crate) struct HtmlAnchor {
    pub name: String,
    pub span: Span,
}

pub(crate) struct Body {
    pub headings: Vec<Heading>,
    /// HTML anchors, in source order.
    pub html_anchors: Vec<HtmlAnchor>,
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
    let mut html_anchors: Vec<HtmlAnchor> = Vec::new();
    let mut regions: Vec<Range<usize>> = Vec::new();
    // First paragraph seen after each count of headings: (headings seen, span).
    let mut paragraphs: Vec<(usize, Span)> = Vec::new();
    let mut code_blocks = 0usize;
    let mut html_blocks = 0usize;
    let mut images = 0usize;
    let mut in_heading = false;
    // An HTML comment left open by a line of an HTML block.
    let mut in_comment = false;

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
                    slug_text: String::new(),
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
            Event::Start(Tag::HtmlBlock) => {
                html_blocks += 1;
                in_comment = false;
            }
            Event::End(TagEnd::HtmlBlock) => html_blocks = html_blocks.saturating_sub(1),
            Event::Start(Tag::Image { .. }) => images += 1,
            Event::End(TagEnd::Image) => images = images.saturating_sub(1),
            Event::Html(_) => {
                if code_blocks == 0 {
                    scan_html(
                        &text[range.clone()],
                        range.start,
                        &mut in_comment,
                        &mut html_anchors,
                    );
                }
            }
            Event::InlineHtml(_) => {
                // One event holds one whole tag or comment.
                let mut comment = false;
                scan_html(
                    &text[range.clone()],
                    range.start,
                    &mut comment,
                    &mut html_anchors,
                );
            }
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
                        if images == 0 {
                            heading.slug_text.push_str(&content);
                        }
                    }
                }
            }
            Event::Code(content) => {
                push_region(&mut regions, range);
                if in_heading && let Some(heading) = headings.last_mut() {
                    heading.text.push_str(&content);
                    if images == 0 {
                        heading.slug_text.push_str(&content);
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_heading && let Some(heading) = headings.last_mut() {
                    heading.text.push(' ');
                    heading.slug_text.push(' ');
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
        html_anchors,
        regions,
        first_h1,
        summary,
    }
}

/// GitHub's heading slugs, repeats included: the first `x` stays `x`, the
/// next is `x-1`, then `x-2`, skipping any slug already given (github-slugger).
#[derive(Default)]
pub(crate) struct Slugger {
    given: BTreeMap<String, usize>,
}

impl Slugger {
    /// The slug of a heading's inline text; `None` when it slugs to nothing.
    pub fn next(&mut self, text: &str) -> Option<String> {
        let base = slug(text);
        if base.is_empty() {
            return None;
        }
        let mut result = base.clone();
        while self.given.contains_key(&result) {
            let count = self.given.entry(base.clone()).or_insert(0);
            *count += 1;
            result = format!("{base}-{count}");
        }
        self.given.insert(result.clone(), 0);
        Some(result)
    }
}

/// Lower-cased; letters and digits of any script, `-` and `_` kept;
/// whitespace becomes `-`; everything else is dropped.
pub(crate) fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.trim().chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

/// `<a id>` / `<a name>` start tags in `source` (file offset `base`),
/// skipping HTML comments; `in_comment` carries an open comment from one
/// HTML line to the next.
fn scan_html(source: &str, base: usize, in_comment: &mut bool, out: &mut Vec<HtmlAnchor>) {
    let mut at = 0;
    loop {
        if *in_comment {
            match source[at..].find("-->") {
                Some(close) => {
                    at += close + 3;
                    *in_comment = false;
                }
                None => return,
            }
        }
        let Some(open) = source[at..].find('<') else {
            return;
        };
        let start = at + open;
        let rest = &source[start..];
        if rest.starts_with("<!--") {
            // The `-->` scan resumes after `<!`, so `<!-->` and `<!--->` are
            // complete comments (CommonMark 0.31).
            *in_comment = true;
            at = start + 2;
            continue;
        }
        match a_tag(rest) {
            Some((names, len)) => {
                for name in names {
                    out.push(HtmlAnchor {
                        name,
                        span: Span::new(base + start, base + start + len),
                    });
                }
                at = start + len;
            }
            None => at = start + 1,
        }
    }
}

/// `tag` starts with `<`: when it opens an `a` start tag, the non-empty
/// `id` and `name` values in attribute order and the tag's length through
/// `>`.
fn a_tag(tag: &str) -> Option<(Vec<String>, usize)> {
    let bytes = tag.as_bytes();
    if !matches!(bytes.get(1), Some(b'a' | b'A')) || !bytes.get(2)?.is_ascii_whitespace() {
        return None;
    }
    let mut names = Vec::new();
    let mut at = 2;
    loop {
        while bytes.get(at)?.is_ascii_whitespace() {
            at += 1;
        }
        match bytes.get(at)? {
            b'>' => return Some((names, at + 1)),
            b'/' if bytes.get(at + 1) == Some(&b'>') => return Some((names, at + 2)),
            _ => {}
        }
        let name_start = at;
        while !matches!(bytes.get(at)?, b'=' | b'>' | b'/') && !bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at == name_start {
            // A stray `/`: not part of any attribute.
            at += 1;
            continue;
        }
        let key = &tag[name_start..at];
        let mut after = at;
        while bytes.get(after)?.is_ascii_whitespace() {
            after += 1;
        }
        if bytes[after] != b'=' {
            continue;
        }
        at = after + 1;
        while bytes.get(at)?.is_ascii_whitespace() {
            at += 1;
        }
        let value = match bytes[at] {
            quote @ (b'"' | b'\'') => {
                let close = tag[at + 1..].find(quote as char)?;
                let value = &tag[at + 1..at + 1 + close];
                at += close + 2;
                value
            }
            _ => {
                let value_start = at;
                while !matches!(bytes.get(at)?, b'>') && !bytes[at].is_ascii_whitespace() {
                    at += 1;
                }
                &tag[value_start..at]
            }
        };
        if !value.is_empty() && (key.eq_ignore_ascii_case("id") || key.eq_ignore_ascii_case("name"))
        {
            names.push(value.to_owned());
        }
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
