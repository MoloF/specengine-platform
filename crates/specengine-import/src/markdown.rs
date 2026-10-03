//! A line scanner for the parts of Markdown the census reads: ATX headings,
//! pipe tables, link destinations and, when asked, wiki links `[[target]]`.
//! Fenced code blocks and HTML comments hide everything inside them; code
//! spans hide links. Setext headings, indented code blocks and raw HTML links
//! are out of its reach.

use std::borrow::Cow;
use std::ops::Range;

/// An ATX heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading<'a> {
    pub level: usize,
    /// 1-based line number.
    pub line: usize,
    /// Byte offset of the heading line in the document.
    pub start: usize,
    /// The heading line with comments removed, without its terminator.
    pub text: Cow<'a, str>,
}

/// A pipe table — a header line, a delimiter row, then data rows — or a
/// headerless block of consecutive lines that open with `|` (not a table in
/// GFM, but where some corpora keep appended record rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table<'a> {
    /// `None` for a headerless block.
    pub header: Option<Vec<String>>,
    /// 1-based line of the header row (of the first row when headerless).
    pub header_line: usize,
    /// Only blank lines and headings precede the table in the body: it opens
    /// the body (where a document may keep a field/value header table).
    pub opens_body: bool,
    pub rows: Vec<Row<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    /// 1-based line number.
    pub line: usize,
    /// The row exactly as written, without its line terminator.
    pub verbatim: &'a str,
    /// Trimmed cells, comments removed.
    pub cells: Vec<String>,
}

/// A link or image destination, a link reference definition or a wiki link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// 1-based line number.
    pub line: usize,
    pub kind: LinkKind,
    /// The destination; for a wiki link the text before `|`.
    pub destination: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// `[text](destination)`, `![alt](destination)`, `[label]: destination`.
    Markdown,
    /// `[[target]]`, `[[target|alias]]`, `![[target]]`.
    Wiki,
}

#[derive(Debug, Default)]
pub struct Scan<'a> {
    pub headings: Vec<Heading<'a>>,
    pub tables: Vec<Table<'a>>,
    pub links: Vec<Link>,
    /// Every line of the body, in order.
    pub lines: Vec<Line<'a>>,
}

/// One line of the body.
#[derive(Debug)]
pub struct Line<'a> {
    /// 1-based line number.
    pub number: usize,
    /// Byte offset of the line in the document.
    pub start: usize,
    /// The line as written, without its terminator (one CR before LF dropped).
    pub raw: &'a str,
    /// `None` inside a fenced code block (fence lines included); otherwise
    /// the line with HTML comments removed (borrowed when nothing was removed).
    pub visible: Option<Cow<'a, str>>,
    /// The line starts inside an HTML comment opened on an earlier line.
    pub opens_in_comment: bool,
}

impl Line<'_> {
    /// The byte offset in `raw` of byte `offset` of `visible`: where a
    /// removed comment sits at `offset`, the offset before it, so that a tail
    /// taken from there keeps the comment.
    pub(crate) fn raw_offset(&self, offset: usize) -> usize {
        if !matches!(self.visible, Some(Cow::Owned(_))) {
            return offset.min(self.raw.len());
        }
        let mut in_comment = self.opens_in_comment;
        let mut before = 0;
        for range in kept_ranges(self.raw, &mut in_comment) {
            if offset <= before + range.len() {
                return range.start + (offset - before);
            }
            before += range.len();
        }
        self.raw.len()
    }
}

/// Scans the body of a document that starts at byte `body_start`, whose first
/// line is line `body_line`; `wiki` enables `[[target]]` links.
pub fn scan(text: &str, body_start: usize, body_line: usize, wiki: bool) -> Scan<'_> {
    let lines = classify_lines(text, body_start, body_line);
    let mut result = Scan::default();
    let mut index = 0;
    // Whether a line other than a blank one or a heading came before.
    let mut content_before = false;
    while index < lines.len() {
        let line = &lines[index];
        let Some(visible) = &line.visible else {
            content_before = true;
            index += 1;
            continue;
        };
        if visible.trim().is_empty() {
            index += 1;
            continue;
        }
        if let Some(level) = heading_level(visible) {
            collect_links(visible, line.number, wiki, &mut result.links);
            result.headings.push(Heading {
                level,
                line: line.number,
                start: line.start,
                text: visible.clone(),
            });
            index += 1;
            continue;
        }
        let delimiter = lines
            .get(index + 1)
            .and_then(|next| next.visible.as_deref());
        let header = table_header(visible, delimiter);
        if header.is_none() && !is_pipe_row(visible) {
            collect_links(visible, line.number, wiki, &mut result.links);
            content_before = true;
            index += 1;
            continue;
        }
        let mut table = Table {
            header,
            header_line: line.number,
            opens_body: !content_before,
            rows: Vec::new(),
        };
        content_before = true;
        if table.header.is_some() {
            collect_links(visible, line.number, wiki, &mut result.links);
            index += 2;
        }
        while let Some(row) = lines.get(index) {
            let Some(visible) = &row.visible else { break };
            if visible.trim().is_empty() || heading_level(visible).is_some() {
                break;
            }
            if table.header.is_some() {
                if !visible.contains('|') {
                    break;
                }
            } else {
                let next = lines
                    .get(index + 1)
                    .and_then(|next| next.visible.as_deref());
                if !is_pipe_row(visible) || table_header(visible, next).is_some() {
                    break;
                }
                if is_delimiter_row(visible) {
                    index += 1;
                    continue;
                }
            }
            collect_links(visible, row.number, wiki, &mut result.links);
            table.rows.push(Row {
                line: row.number,
                verbatim: row.raw,
                cells: split_cells(visible),
            });
            index += 1;
        }
        if table.header.is_some() || !table.rows.is_empty() {
            result.tables.push(table);
        }
    }
    result.lines = lines;
    result
}

/// Splits the body into lines and hides fenced code and HTML comments.
fn classify_lines(text: &str, body_start: usize, body_line: usize) -> Vec<Line<'_>> {
    let body = text.get(body_start..).unwrap_or("");
    let mut lines = Vec::new();
    let mut offset = body_start;
    let mut fence: Option<(u8, usize)> = None;
    let mut in_comment = false;
    for (index, chunk) in body.split_inclusive('\n').enumerate() {
        let raw = chunk.strip_suffix('\n').unwrap_or(chunk);
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let number = body_line + index;
        let start = offset;
        offset += chunk.len();
        let opens_in_comment = in_comment;
        if let Some((marker, length)) = fence {
            if closes_fence(raw, marker, length) {
                fence = None;
            }
            lines.push(Line {
                number,
                start,
                raw,
                visible: None,
                opens_in_comment,
            });
            continue;
        }
        let visible = strip_comments(raw, &mut in_comment);
        if let Some(open) = opens_fence(&visible) {
            fence = Some(open);
            lines.push(Line {
                number,
                start,
                raw,
                visible: None,
                opens_in_comment,
            });
            continue;
        }
        lines.push(Line {
            number,
            start,
            raw,
            visible: Some(visible),
            opens_in_comment,
        });
    }
    lines
}

/// Up to three spaces of indentation, as CommonMark allows for block starts.
pub(crate) fn block_content(line: &str) -> Option<&str> {
    let content = line.trim_start_matches(' ');
    (line.len() - content.len() <= 3).then_some(content)
}

pub(crate) fn opens_fence(line: &str) -> Option<(u8, usize)> {
    let content = block_content(line)?;
    let marker = *content.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = content.bytes().take_while(|&b| b == marker).count();
    if length < 3 {
        return None;
    }
    if marker == b'`' && content[length..].contains('`') {
        return None;
    }
    Some((marker, length))
}

pub(crate) fn closes_fence(line: &str, marker: u8, length: usize) -> bool {
    let Some(content) = block_content(line) else {
        return false;
    };
    let run = content.bytes().take_while(|&b| b == marker).count();
    run >= length && content[run..].trim().is_empty()
}

pub(crate) fn heading_level(line: &str) -> Option<usize> {
    let content = block_content(line)?;
    let level = content.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    match content.as_bytes().get(level) {
        None | Some(b' ' | b'\t') => Some(level),
        Some(_) => None,
    }
}

/// The header cells when `line` + `delimiter` open a table: both hold a
/// pipe, the delimiter row is `:?-+:?` cells, and the cell counts agree.
fn table_header(line: &str, delimiter: Option<&str>) -> Option<Vec<String>> {
    let delimiter = delimiter?;
    if line.trim().is_empty() || !line.contains('|') || !is_delimiter_row(delimiter) {
        return None;
    }
    let header = split_cells(line);
    (header.len() == split_cells(delimiter).len()).then_some(header)
}

/// A line of `:?-+:?` cells holding at least one pipe.
fn is_delimiter_row(line: &str) -> bool {
    line.contains('|')
        && split_cells(line).iter().all(|cell| {
            let inner = cell.strip_prefix(':').unwrap_or(cell);
            let inner = inner.strip_suffix(':').unwrap_or(inner);
            !inner.is_empty() && inner.bytes().all(|b| b == b'-')
        })
}

/// A line that opens with `|` after indentation.
fn is_pipe_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// Cells of a pipe-table line: outer pipes optional, `\|` does not split.
fn split_cells(line: &str) -> Vec<String> {
    cell_ranges(line)
        .into_iter()
        .map(|range| line[range].trim().to_owned())
        .collect()
}

/// Byte ranges of the cells of a pipe-table line, untrimmed: outer pipes
/// optional, `\|` does not split. Always at least one cell.
pub(crate) fn cell_ranges(line: &str) -> Vec<Range<usize>> {
    let mut from = line.len() - line.trim_start().len();
    let mut to = line.trim_end().len().max(from);
    if line[from..to].starts_with('|') {
        from += 1;
    }
    if let Some(rest) = line[from..to].strip_suffix('|')
        && !rest.ends_with('\\')
    {
        to -= 1;
    }
    let mut cells = Vec::new();
    let mut start = from;
    let mut escaped = false;
    for (offset, byte) in line[from..to].bytes().enumerate() {
        if byte == b'|' && !escaped {
            cells.push(start..from + offset);
            start = from + offset + 1;
        }
        escaped = byte == b'\\' && !escaped;
    }
    cells.push(start..to);
    cells
}

/// Removes `<!-- … -->` spans (possibly across lines) outside code spans.
fn strip_comments<'a>(line: &'a str, in_comment: &mut bool) -> Cow<'a, str> {
    if !*in_comment && !line.contains("<!--") {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    for range in kept_ranges(line, in_comment) {
        out.push_str(&line[range]);
    }
    Cow::Owned(out)
}

/// The byte ranges of `line` that [`strip_comments`] keeps, in order and
/// merged; `in_comment` carries the comment state from line to line.
fn kept_ranges(line: &str, in_comment: &mut bool) -> Vec<Range<usize>> {
    let mut kept: Vec<Range<usize>> = Vec::new();
    let mut keep = |range: Range<usize>| match kept.last_mut() {
        Some(last) if last.end == range.start => last.end = range.end,
        _ => kept.push(range),
    };
    let mut spans: Option<CodeSpans> = None;
    let mut index = 0;
    while index < line.len() {
        let rest = &line[index..];
        if *in_comment {
            match rest.find("-->") {
                Some(end) => {
                    index += end + 3;
                    *in_comment = false;
                }
                None => index = line.len(),
            }
            continue;
        }
        if rest.starts_with('`') {
            let span = spans
                .get_or_insert_with(|| CodeSpans::new(line))
                .len_at(line, index);
            keep(index..index + span);
            index += span;
            continue;
        }
        if rest.starts_with("<!--") {
            *in_comment = true;
            index += 4;
            continue;
        }
        let Some(c) = rest.chars().next() else { break };
        keep(index..index + c.len_utf8());
        index += c.len_utf8();
    }
    kept
}

/// `line` with every emphasis delimiter run (`*`, `_`) that can open or
/// close emphasis by CommonMark's flanking rules read as blanks, byte for
/// byte (offsets keep); runs inside code spans and backslash-escaped
/// delimiters stay as written.
pub(crate) fn emphasis_blanks(line: &str) -> Cow<'_, str> {
    if !line.contains(['*', '_']) {
        return Cow::Borrowed(line);
    }
    let bytes = line.as_bytes();
    let mut runs: Vec<Range<usize>> = Vec::new();
    let mut spans: Option<CodeSpans> = None;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if bytes.get(index + 1).is_some_and(u8::is_ascii_punctuation) => index += 2,
            b'`' => {
                index += spans
                    .get_or_insert_with(|| CodeSpans::new(line))
                    .len_at(line, index);
            }
            marker @ (b'*' | b'_') => {
                let run = bytes[index..].iter().take_while(|&&b| b == marker).count();
                let before = line[..index].chars().next_back();
                let after = line[index + run..].chars().next();
                if emphasis_delimiter(marker, before, after) {
                    runs.push(index..index + run);
                }
                index += run;
            }
            _ => index += 1,
        }
    }
    if runs.is_empty() {
        return Cow::Borrowed(line);
    }
    let mut out = bytes.to_vec();
    for run in runs {
        out[run].fill(b' ');
    }
    // Only ASCII delimiters became ASCII blanks: still UTF-8.
    Cow::Owned(String::from_utf8(out).unwrap_or_else(|_| line.to_owned()))
}

/// Whether a run of `marker` between `before` and `after` (`None`: the
/// line's edge) can open or close emphasis (CommonMark "left-flanking" and
/// "right-flanking" delimiter runs; `_` not inside a word).
fn emphasis_delimiter(marker: u8, before: Option<char>, after: Option<char>) -> bool {
    let blank = |c: Option<char>| c.is_none_or(char::is_whitespace);
    let punctuation =
        |c: Option<char>| c.is_some_and(|c| !c.is_alphanumeric() && !c.is_whitespace());
    let left = !blank(after) && (!punctuation(after) || blank(before) || punctuation(before));
    let right = !blank(before) && (!punctuation(before) || blank(after) || punctuation(after));
    if marker == b'*' {
        left || right
    } else {
        (left && (!right || punctuation(before))) || (right && (!left || punctuation(after)))
    }
}

/// The backtick runs of one line, collected once so that closing a code span
/// is a binary search rather than a rescan of the rest of the line (a line of
/// unclosed runs would otherwise cost O(L^1.5)).
struct CodeSpans {
    /// Maximal backtick runs as `(length, start)`, sorted.
    runs: Vec<(usize, usize)>,
}

impl CodeSpans {
    fn new(line: &str) -> Self {
        let bytes = line.as_bytes();
        let mut runs = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'`' {
                let run = bytes[index..].iter().take_while(|&&b| b == b'`').count();
                runs.push((run, index));
                index += run;
            } else {
                index += 1;
            }
        }
        runs.sort_unstable();
        Self { runs }
    }

    /// Length of the code span whose opening run starts at `start` of the
    /// line the runs came from (possibly inside a longer run, after an
    /// escape): up to the next run of equal length, or just the opening run
    /// when unclosed.
    fn len_at(&self, line: &str, start: usize) -> usize {
        let rest = line.as_bytes().get(start..).unwrap_or_default();
        let run = rest.iter().take_while(|&&b| b == b'`').count();
        let next = self.runs.partition_point(|&key| key <= (run, start));
        match self.runs.get(next) {
            Some(&(length, close)) if length == run => close + run - start,
            _ => run,
        }
    }
}

/// Inline link and image destinations, link reference definitions and, with
/// `wiki`, wiki links.
fn collect_links(line: &str, number: usize, wiki: bool, out: &mut Vec<Link>) {
    if let Some(destination) = reference_definition(line) {
        out.push(Link {
            line: number,
            kind: LinkKind::Markdown,
            destination,
        });
        return;
    }
    let bytes = line.as_bytes();
    let mut open = 0usize;
    // Built on first use; the scan only moves forward, so a `]]` missing
    // after one `[[` is missing after every later one.
    let mut spans: Option<CodeSpans> = None;
    let mut inline: Option<InlineIndex> = None;
    let mut wiki_closable = wiki;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index += 2;
                continue;
            }
            b'`' => {
                index += spans
                    .get_or_insert_with(|| CodeSpans::new(line))
                    .len_at(line, index);
                continue;
            }
            b'[' if wiki && bytes.get(index + 1) == Some(&b'[') => {
                let close = if wiki_closable {
                    line[index + 2..].find("]]")
                } else {
                    None
                };
                if let Some(close) = close {
                    let inner = &line[index + 2..index + 2 + close];
                    let target = inner.split('|').next().unwrap_or(inner);
                    out.push(Link {
                        line: number,
                        kind: LinkKind::Wiki,
                        destination: target.trim().trim_end_matches('\\').trim().to_owned(),
                    });
                    index += 2 + close + 2;
                    continue;
                }
                wiki_closable = false;
                open += 1;
            }
            b'[' => open += 1,
            b']' if open > 0 => {
                open -= 1;
                if bytes.get(index + 1) == Some(&b'(')
                    && let Some((destination, end)) = inline
                        .get_or_insert_with(|| InlineIndex::new(line))
                        .destination(line, index + 2)
                {
                    out.push(Link {
                        line: number,
                        kind: LinkKind::Markdown,
                        destination,
                    });
                    index = end;
                    continue;
                }
            }
            _ => {}
        }
        index += 1;
    }
}

/// `[label]: destination` (footnotes `[^x]:` excluded).
fn reference_definition(line: &str) -> Option<String> {
    let content = block_content(line)?;
    let rest = content.strip_prefix('[')?;
    if rest.starts_with('^') {
        return None;
    }
    let close = rest.find("]:")?;
    if close == 0 || rest[..close].contains('[') {
        return None;
    }
    let target = rest[close + 2..].trim_start();
    let destination = match target.strip_prefix('<') {
        Some(inner) => &inner[..inner.find('>')?],
        None => target.split_whitespace().next()?,
    };
    Some(destination.to_owned())
}

/// What parsing `destination [title])` after a `](` looks up on one line,
/// collected in one pass so that each `](` costs a few binary searches
/// instead of a scan that may run to the end of the line (a line of `](`
/// that never close would otherwise be rescanned from every one of them).
///
/// Escapes are resolved once for the line: a byte is escaped when an odd run
/// of backslashes precedes it. Every lookup starts right after a byte that is
/// not a backslash (`(`, a blank or a title quote), so a scan from there
/// would see exactly these escapes.
struct InlineIndex {
    /// Maximal runs of spaces and tabs as `(start, end)`, escapes ignored.
    blank_runs: Vec<(usize, usize)>,
    /// Every `>`: a `<…>` destination knows no escapes.
    angles: Vec<usize>,
    /// Unescaped spaces and tabs.
    blanks: Vec<usize>,
    /// Unescaped `"`.
    double_quotes: Vec<usize>,
    /// Unescaped `'`.
    single_quotes: Vec<usize>,
    /// Unescaped `)`.
    close_parens: Vec<usize>,
    /// Unescaped `(` and `)`, in order.
    parens: Vec<usize>,
    /// Per entry of `parens`: the first `)` from that entry on that closes a
    /// `(` opened before it — where a bare destination beginning just before
    /// the entry ends — or the line length when there is none.
    unbalanced: Vec<usize>,
}

impl InlineIndex {
    fn new(line: &str) -> Self {
        let bytes = line.as_bytes();
        let mut index = Self {
            blank_runs: Vec::new(),
            angles: Vec::new(),
            blanks: Vec::new(),
            double_quotes: Vec::new(),
            single_quotes: Vec::new(),
            close_parens: Vec::new(),
            parens: Vec::new(),
            unbalanced: Vec::new(),
        };
        // Nesting before each entry of `parens`, then after the last one.
        let mut depths = vec![0isize];
        let mut depth = 0isize;
        let mut run_start = None;
        let mut escaped = false;
        for (at, &byte) in bytes.iter().enumerate() {
            let blank = matches!(byte, b' ' | b'\t');
            match run_start {
                None if blank => run_start = Some(at),
                Some(start) if !blank => {
                    index.blank_runs.push((start, at));
                    run_start = None;
                }
                _ => {}
            }
            if byte == b'>' {
                index.angles.push(at);
            }
            if escaped {
                escaped = false;
                continue;
            }
            match byte {
                b'\\' => escaped = true,
                b' ' | b'\t' => index.blanks.push(at),
                b'"' => index.double_quotes.push(at),
                b'\'' => index.single_quotes.push(at),
                b'(' | b')' => {
                    index.parens.push(at);
                    if byte == b'(' {
                        depth += 1;
                    } else {
                        depth -= 1;
                        index.close_parens.push(at);
                    }
                    depths.push(depth);
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            index.blank_runs.push((start, bytes.len()));
        }
        // The next lower nesting, right to left: depth moves by one per
        // paren, so the first lower depth follows the `)` that ends the scan.
        index.unbalanced = vec![bytes.len(); index.parens.len()];
        let mut higher: Vec<usize> = Vec::new();
        for entry in (0..depths.len()).rev() {
            while higher
                .last()
                .is_some_and(|&next| depths[next] >= depths[entry])
            {
                higher.pop();
            }
            if let (Some(&next), Some(slot)) = (higher.last(), index.unbalanced.get_mut(entry)) {
                *slot = index.parens[next - 1];
            }
            higher.push(entry);
        }
        index
    }

    /// First byte at or after `from` that is not a space or a tab.
    fn skip_blanks(&self, from: usize) -> usize {
        let run = self.blank_runs.partition_point(|&(start, _)| start <= from);
        match run.checked_sub(1).and_then(|run| self.blank_runs.get(run)) {
            Some(&(_, end)) if from < end => end,
            _ => from,
        }
    }

    /// Parses `destination [title])` from `start` of the line the index came
    /// from; returns the destination and the index after `)`, or `None` when
    /// this is not an inline link.
    fn destination(&self, line: &str, start: usize) -> Option<(String, usize)> {
        let bytes = line.as_bytes();
        let begin = self.skip_blanks(start);
        let (destination, end) = if bytes.get(begin) == Some(&b'<') {
            let close = first_from(&self.angles, begin + 1)?;
            (line.get(begin + 1..close)?, close + 1)
        } else {
            // Up to the first unescaped blank or the first `)` that closes
            // no `(` opened after `begin`.
            let blank = first_from(&self.blanks, begin).unwrap_or(bytes.len());
            let paren = self.parens.partition_point(|&at| at < begin);
            let unbalanced = self.unbalanced.get(paren).copied().unwrap_or(bytes.len());
            let end = blank.min(unbalanced);
            (line.get(begin..end)?, end)
        };
        let index = self.skip_blanks(end);
        match bytes.get(index) {
            Some(b')') => Some((destination.to_owned(), index + 1)),
            Some(&quote @ (b'"' | b'\'' | b'(')) => {
                let closings = match quote {
                    b'"' => &self.double_quotes,
                    b'\'' => &self.single_quotes,
                    _ => &self.close_parens,
                };
                let cursor = first_from(closings, index + 1)?;
                let after = self.skip_blanks(cursor + 1);
                (bytes.get(after) == Some(&b')')).then(|| (destination.to_owned(), after + 1))
            }
            _ => None,
        }
    }
}

/// The first of the sorted `positions` at or after `from`.
fn first_from(positions: &[usize], from: usize) -> Option<usize> {
    positions
        .get(positions.partition_point(|&at| at < from))
        .copied()
}
