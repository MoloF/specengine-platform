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
//!
//! File links (docs/features/spec-check-links.md): every inline link
//! (`[t](dest)`) outside an image description and every reference
//! definition (`[r]: dest`, used or not) whose destination is local — not
//! empty, not `//`-led, no URI scheme — with its path (before the first `#`,
//! cut at the first `?`) and anchor (after the first `#`) as CommonMark
//! gives them, and the span of the destination as written. Autolinks, raw
//! HTML, reference uses, images and code give none.

use std::collections::BTreeMap;
use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
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

/// One local Markdown link destination: an inline link's or a reference
/// definition's.
pub(crate) struct FileLink {
    /// Before the first `#`, cut at the first `?`; `""` for `#h`.
    pub path: String,
    /// After the first `#`; `None` when empty.
    pub anchor: Option<String>,
    /// The destination as written: `<…>` and the title excluded, `?query`
    /// and `#anchor` included.
    pub span: Span,
}

/// An inline link being read: its destination and where its text ends.
struct OpenLink {
    path: String,
    anchor: Option<String>,
    /// The link's whole range, `[` to `)`.
    range: Range<usize>,
    /// The furthest end of an event inside the link text: the `](` closing
    /// the text lies at or after it.
    text_end: usize,
}

pub(crate) struct Body {
    pub headings: Vec<Heading>,
    /// HTML anchors, in source order.
    pub html_anchors: Vec<HtmlAnchor>,
    /// Local file links, by span start.
    pub file_links: Vec<FileLink>,
    /// Merged source ranges to read references from, in order.
    pub regions: Vec<Range<usize>>,
    /// Index of the first level-1 heading.
    pub first_h1: Option<usize>,
    /// First paragraph after the first H1, before the next heading (no H1:
    /// before the first heading).
    pub summary: Option<Span>,
}

/// The pinned reader's options: one set for the parse ([`scan`]) and the
/// check's body rules ([`outline`]).
fn options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options
}

/// Scans `text[body]`; every offset returned is a file offset.
pub(crate) fn scan(text: &str, body: Span) -> Body {
    let source = &text[body.range()];
    let base = body.start;

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
    // Links open around the current event (1 inside a link), and the inline
    // link being read.
    let mut link_depth = 0usize;
    let mut open_link: Option<OpenLink> = None;

    let parser = Parser::new_ext(source, options());
    // Reference definitions, one per label (CommonMark: the first), used or
    // not; sorted by span below, never in map order.
    let mut file_links: Vec<FileLink> = parser
        .reference_definitions()
        .iter()
        .filter_map(|(_, definition)| {
            let (path, anchor) = local_destination(&definition.dest)?;
            let range = definition.span.start + base..definition.span.end + base;
            // The re-scan cannot mirror pulldown-cmark's container handling
            // exactly (a `>` on an indented continuation line is text, not
            // a blockquote marker): a destination it cannot locate is not
            // recorded, never a panic on user data.
            let span = definition_destination(text.as_bytes(), range)?;
            Some(FileLink { path, anchor, span })
        })
        .collect();

    for (event, range) in parser.into_offset_iter() {
        let range = range.start + base..range.end + base;
        if let Some(link) = &mut open_link {
            let closing = link_depth == 1 && matches!(event, Event::End(TagEnd::Link));
            if !closing {
                link.text_end = link.text_end.max(range.end);
            }
        }
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                link_depth += 1;
                if link_depth == 1
                    && images == 0
                    && link_type == LinkType::Inline
                    && let Some((path, anchor)) = local_destination(&dest_url)
                {
                    open_link = Some(OpenLink {
                        path,
                        anchor,
                        text_end: range.start + 1,
                        range,
                    });
                }
            }
            Event::End(TagEnd::Link) => {
                link_depth = link_depth.saturating_sub(1);
                if link_depth == 0
                    && let Some(link) = open_link.take()
                {
                    // As for a definition: a destination the re-scan cannot
                    // locate is not recorded.
                    let span = inline_destination(text.as_bytes(), &link.range, link.text_end);
                    if let Some(span) = span {
                        file_links.push(FileLink {
                            path: link.path,
                            anchor: link.anchor,
                            span,
                        });
                    }
                }
            }
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
    file_links.sort_by_key(|link| link.span.start);
    Body {
        headings,
        html_anchors,
        file_links,
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

/// A labelled place of the body (`docs/canon/spec-check-process.md`, "A rule"):
/// a heading of any level, or a lead-in — strong emphasis (`**…**`, `__…__`)
/// opening a paragraph, a list item or a line inside one, outside a link.
pub(crate) struct Label {
    /// The [`slug`] of its text, never de-duplicated: a heading's inline
    /// text as its anchor's (no image), a lead-in's inner text.
    pub slug: String,
    /// Where it starts: the heading, the lead-in's opening delimiter.
    pub start: usize,
    /// What it labels: a heading's, from after its line to the next heading
    /// of the same or a higher level (else the body's end); a lead-in's, from
    /// after it to the next lead-in line or the end of its paragraph or list
    /// item.
    pub content: Range<usize>,
}

/// A `Text` or `Code` event outside heading text and HTML.
pub(crate) struct Run {
    pub range: Range<usize>,
    /// Its text holds a letter or digit (`char::is_alphanumeric`).
    pub alnum: bool,
    /// In a link's text or an image's description.
    pub in_link: bool,
}

/// The body as the check's body rules read it, through the parse's reader.
pub(crate) struct Outline {
    /// Headings and lead-ins, in source order.
    pub labels: Vec<Label>,
    /// In source order.
    pub runs: Vec<Run>,
}

impl Outline {
    /// A run starting in `range` holds a letter or digit.
    pub fn filled(&self, range: &Range<usize>) -> bool {
        self.runs
            .iter()
            .any(|run| run.alnum && range.contains(&run.range.start))
    }
}

/// The labels and text runs of `text[body]` (file offsets). Code blocks and
/// HTML blocks hold no label; text in a code block is a run, text in HTML
/// (comments included) never is.
pub(crate) fn outline(text: &str, body: Span) -> Outline {
    struct OpenHeading {
        level: u8,
        start: usize,
        raw_end: usize,
        text: String,
    }
    struct Lead {
        start: usize,
        end: usize,
        block_end: usize,
        text: String,
        /// Strong emphasis open inside it, itself included.
        depth: usize,
    }
    let Some(source) = text.get(body.range()) else {
        return Outline {
            labels: Vec::new(),
            runs: Vec::new(),
        };
    };
    let base = body.start;
    // (level, start, raw end, slug).
    let mut headings: Vec<(u8, usize, usize, String)> = Vec::new();
    let mut leads: Vec<Lead> = Vec::new();
    let mut runs: Vec<Run> = Vec::new();
    // Open blocks, innermost last: whether a lead-in may open it (a
    // paragraph, a list item's inline content) and where it ends.
    let mut blocks: Vec<(bool, usize)> = Vec::new();
    let mut heading: Option<OpenHeading> = None;
    let mut lead: Option<Lead> = None;
    // The next event opens a line of the innermost block.
    let mut line_open = false;
    let mut html_blocks = 0usize;
    let mut links = 0usize;
    let mut images = 0usize;
    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        let range = range.start + base..range.end + base;
        let mut opens_line = false;
        match event {
            Event::Start(Tag::Strong) => match &mut lead {
                Some(open) => open.depth += 1,
                None => {
                    if line_open
                        && links == 0
                        && images == 0
                        && let Some(&(true, block_end)) = blocks.last()
                    {
                        lead = Some(Lead {
                            start: range.start,
                            end: range.end,
                            block_end,
                            text: String::new(),
                            depth: 1,
                        });
                    }
                }
            },
            Event::End(TagEnd::Strong) => {
                if lead.as_ref().is_some_and(|open| open.depth == 1) {
                    if let Some(mut open) = lead.take() {
                        open.end = open.end.max(range.end);
                        leads.push(open);
                    }
                } else if let Some(open) = &mut lead {
                    open.depth -= 1;
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                blocks.push((false, range.end));
                heading = Some(OpenHeading {
                    level: level as u8,
                    start: range.start,
                    raw_end: range.end,
                    text: String::new(),
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                blocks.pop();
                if let Some(open) = heading.take() {
                    headings.push((open.level, open.start, open.raw_end, slug(&open.text)));
                }
            }
            Event::Start(Tag::Paragraph | Tag::Item) => {
                blocks.push((true, range.end));
                opens_line = true;
            }
            Event::Start(Tag::HtmlBlock) => {
                html_blocks += 1;
                blocks.push((false, range.end));
            }
            Event::End(TagEnd::HtmlBlock) => {
                html_blocks = html_blocks.saturating_sub(1);
                blocks.pop();
            }
            Event::Start(Tag::Link { .. }) => links += 1,
            Event::End(TagEnd::Link) => links = links.saturating_sub(1),
            Event::Start(Tag::Image { .. }) => images += 1,
            Event::End(TagEnd::Image) => images = images.saturating_sub(1),
            Event::Start(
                Tag::Emphasis | Tag::Strikethrough | Tag::Superscript | Tag::Subscript,
            )
            | Event::End(
                TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Superscript | TagEnd::Subscript,
            ) => {}
            Event::Start(_) => blocks.push((false, range.end)),
            Event::End(_) => {
                blocks.pop();
            }
            Event::Text(content) | Event::Code(content) => {
                if html_blocks == 0 {
                    if let Some(open) = &mut lead {
                        open.text.push_str(&content);
                    }
                    match &mut heading {
                        Some(open) => {
                            if images == 0 {
                                open.text.push_str(&content);
                            }
                        }
                        None => runs.push(Run {
                            range,
                            alnum: content.chars().any(char::is_alphanumeric),
                            in_link: links > 0 || images > 0,
                        }),
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(open) = &mut heading {
                    open.text.push(' ');
                }
                if let Some(open) = &mut lead {
                    open.text.push(' ');
                }
                opens_line = true;
            }
            Event::TaskListMarker(_) => opens_line = line_open,
            _ => {}
        }
        line_open = opens_line;
    }

    let mut labels: Vec<Label> = Vec::with_capacity(headings.len() + leads.len());
    for (index, (level, start, raw_end, slug)) in headings.iter().enumerate() {
        let end = headings[index + 1..]
            .iter()
            .find(|(next, ..)| next <= level)
            .map_or(body.end, |&(_, next_start, ..)| next_start);
        labels.push(Label {
            slug: slug.clone(),
            start: *start,
            content: *raw_end..end.max(*raw_end),
        });
    }
    for (index, open) in leads.iter().enumerate() {
        let end = leads
            .get(index + 1)
            .filter(|next| next.start < open.block_end)
            .map_or(open.block_end, |next| next.start);
        labels.push(Label {
            slug: slug(&open.text),
            start: open.start,
            content: open.end..end.max(open.end),
        });
    }
    labels.sort_by_key(|label| label.start);
    Outline { labels, runs }
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

/// The path and anchor of a local destination (the census's
/// `local_target` rule, judged on the trimmed text): not empty, not
/// `//`-led, no URI scheme (`[A-Za-z][A-Za-z0-9+.-]*:`). Path = before the
/// first `#`, cut at the first `?`; anchor = after the first `#`, `None`
/// when empty; both as given. `None` also for an empty path without an
/// anchor (`#`, `?q`).
fn local_destination(dest: &str) -> Option<(String, Option<String>)> {
    let trimmed = dest.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") || has_scheme(trimmed) {
        return None;
    }
    let (before, anchor) = match dest.split_once('#') {
        Some((before, anchor)) => (before, Some(anchor).filter(|anchor| !anchor.is_empty())),
        None => (dest, None),
    };
    let path = before.split('?').next().unwrap_or(before);
    if path.is_empty() && anchor.is_none() {
        return None;
    }
    Some((path.to_owned(), anchor.map(str::to_owned)))
}

/// The text before its first `:` is a URI scheme.
fn has_scheme(dest: &str) -> bool {
    let Some((scheme, _)) = dest.split_once(':') else {
        return false;
    };
    scheme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Where a destination is looked for: an inline link's ends at an
/// unbalanced `)`, a definition's only at whitespace.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DestinationIn {
    Link,
    Definition,
}

/// The destination of the inline link spanning `link` (`[` to `)`): after
/// the first `](` at or past `text_end`, the end of the last event inside
/// the link text (so brackets and code spans in the text are passed over).
fn inline_destination(bytes: &[u8], link: &Range<usize>, text_end: usize) -> Option<Span> {
    let end = link.end.min(bytes.len());
    let mut at = text_end.max(link.start + 1);
    while at + 1 < end {
        if bytes[at] == b']' && bytes[at + 1] == b'(' {
            return destination_at(bytes, at + 2, end, DestinationIn::Link);
        }
        at += 1;
    }
    None
}

/// The destination of the reference definition spanning `definition`
/// (`[` of the label to the end of the destination or title): after the
/// label's `]:`, on the same line or the next.
fn definition_destination(bytes: &[u8], definition: Range<usize>) -> Option<Span> {
    let end = definition.end.min(bytes.len());
    let mut at = definition.start + 1;
    while at < end {
        match bytes[at] {
            b'\\' => at += 2,
            b']' => {
                return (bytes.get(at + 1) == Some(&b':'))
                    .then(|| destination_at(bytes, at + 2, end, DestinationIn::Definition))
                    .flatten();
            }
            _ => at += 1,
        }
    }
    None
}

/// Whitespace pulldown-cmark passes before a destination on one line
/// (`is_ascii_whitespace_no_nl`): space, tab, vertical tab, form feed.
fn is_blank_no_eol(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | 0x0B | 0x0C)
}

/// The destination starting after optional blanks ([`is_blank_no_eol`])
/// and at most one line ending (after which container markers `>` are
/// passed too) at `from`, before `end`: the text inside `<…>`, else the bare
/// run up to a byte `0x00..=0x20` (pulldown-cmark's `scan_link_dest`) or
/// (in a link) an unbalanced `)`.
fn destination_at(bytes: &[u8], from: usize, end: usize, kind: DestinationIn) -> Option<Span> {
    let mut at = from;
    while at < end && is_blank_no_eol(bytes[at]) {
        at += 1;
    }
    if at < end && matches!(bytes[at], b'\r' | b'\n') {
        if bytes[at] == b'\r' && bytes.get(at + 1) == Some(&b'\n') {
            at += 1;
        }
        at += 1;
        while at < end && (is_blank_no_eol(bytes[at]) || bytes[at] == b'>') {
            at += 1;
        }
    }
    if at >= end {
        return None;
    }
    if bytes[at] == b'<' {
        let start = at + 1;
        let mut close = start;
        while close < end {
            match bytes[close] {
                b'\\' => close += 2,
                b'>' => return Some(Span::new(start, close)),
                b'<' | b'\r' | b'\n' => return None,
                _ => close += 1,
            }
        }
        return None;
    }
    let start = at;
    let mut depth = 0usize;
    while at < end {
        let byte = bytes[at];
        if byte == b'\\' && bytes.get(at + 1).is_some_and(u8::is_ascii_punctuation) {
            at += 2;
            continue;
        }
        if byte <= 0x20 {
            break;
        }
        if kind == DestinationIn::Link {
            match byte {
                b'(' => depth += 1,
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                _ => {}
            }
        }
        at += 1;
    }
    (at > start).then(|| Span::new(start, at.min(end)))
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
