//! Strong lead-ins: a list item (after an optional task box) or a paragraph
//! opening with `**…**` / `__…__`, and the extent of a list item's text
//! (`docs/features/import-records.md` AC-03, AC-07); a span read as ID
//! [separator [title]] (`docs/features/import-gaps.md` AC-01). Markdown
//! syntax only; what counts as an ID is the config's.

use std::collections::HashSet;
use std::ops::Range;

use crate::markdown::{Line, block_content, closes_fence, heading_level, opens_fence};

/// A strong span opening a list item or a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LeadIn {
    /// Indentation of the list marker in columns (a tab to the next
    /// multiple of four); 0 for a paragraph.
    pub marker_indent: usize,
    /// The span's content, between the delimiters, untrimmed.
    pub content: Range<usize>,
    /// Byte offset right after the closing delimiter.
    pub after: usize,
    /// A list item's task box: `[ ]` → `false`, `[x]` / `[X]` → `true`;
    /// `None` without one (and for a paragraph).
    pub task_box: Option<bool>,
}

/// A list item whose content opens (after an optional `[ ]`, `[x]`, `[X]`)
/// with a strong span.
pub(crate) fn list_item(line: &str) -> Option<LeadIn> {
    let indent_bytes = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[indent_bytes..];
    let marker = match rest.as_bytes().first()? {
        b'-' | b'*' | b'+' => 1,
        b'0'..=b'9' => {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 9 || !matches!(rest.as_bytes().get(digits), Some(b'.' | b')')) {
                return None;
            }
            digits + 1
        }
        _ => return None,
    };
    let after_marker = indent_bytes + marker;
    let mut at = skip_blanks(line, after_marker);
    if at == after_marker {
        return None;
    }
    let mut task_box = None;
    for (task, checked) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
        if line[at..].starts_with(task) {
            let after_task = at + task.len();
            let next = skip_blanks(line, after_task);
            if next > after_task {
                at = next;
                task_box = Some(checked);
            }
            break;
        }
    }
    let (content, after) = strong_span(line, at)?;
    Some(LeadIn {
        marker_indent: columns(&line[..indent_bytes]),
        content,
        after,
        task_box,
    })
}

/// A paragraph line opening (after at most three spaces) with a strong span.
pub(crate) fn paragraph(line: &str) -> Option<LeadIn> {
    let at = line.len() - line.trim_start_matches(' ').len();
    if at > 3 {
        return None;
    }
    let (content, after) = strong_span(line, at)?;
    Some(LeadIn {
        marker_indent: 0,
        content,
        after,
        task_box: None,
    })
}

/// `**content**` or `__content__` starting at `at`: the content range and
/// the offset after the closing delimiter; the content is not blank.
fn strong_span(line: &str, at: usize) -> Option<(Range<usize>, usize)> {
    let rest = &line[at..];
    let delimiter = ["**", "__"]
        .into_iter()
        .find(|delimiter| rest.starts_with(delimiter))?;
    let open = at + delimiter.len();
    let close = open + line[open..].find(delimiter)?;
    if line[open..close].trim().is_empty() {
        return None;
    }
    Some((open..close, close + delimiter.len()))
}

fn skip_blanks(line: &str, from: usize) -> usize {
    from + (line[from..].len() - line[from..].trim_start_matches([' ', '\t']).len())
}

/// Leading indentation of a line in columns.
pub(crate) fn indent_columns(line: &str) -> usize {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    columns(&line[..indent])
}

fn columns(blanks: &str) -> usize {
    blanks.chars().fold(0, |column, c| {
        if c == '\t' {
            column + 4 - column % 4
        } else {
            column + 1
        }
    })
}

/// The lines after the list item at `index` that belong to its text, as
/// written: lines indented deeper than its marker (blank lines only before
/// such a line) and lazy lines (CommonMark: a line at the marker's
/// indentation or shallower directly continuing the paragraph, unless it
/// opens another block). A heading, any other line at the marker's
/// indentation or shallower, and a nested list item `nested_record` accepts
/// (a record of its own) end it. Block syntax is read past the indentation
/// the item's container owns (up to the marker's column), so an item
/// indented four columns or by a tab ends where one at column 0 would, and
/// a fence opened on a deeper line holds code (no heading, no paragraph) up
/// to its closing fence. A line blank as written is blank. A line opening
/// inside a comment the item's text opened (the item's own line opens in
/// none: no record is read there) is the item's paragraph text whatever
/// follows `-->`: no heading, list marker, nested record or fence is read
/// on it, and it ends nothing (`docs/features/import-gaps.md` gaps 5-6).
pub(crate) fn item_continuation<'l, 'a>(
    lines: &'l [Line<'a>],
    index: usize,
    marker_indent: usize,
    table_lines: &HashSet<usize>,
    nested_record: &dyn Fn(&str) -> bool,
) -> Vec<&'l Line<'a>> {
    let mut taken = Vec::new();
    let mut pending = Vec::new();
    // The last line taken, or the item's own, is paragraph text.
    let mut paragraph = true;
    // A fence opened on a deeper line past the marker's columns: the line
    // classification only sees one at most three columns in.
    let mut fence: Option<(u8, usize)> = None;
    for line in lines.iter().skip(index + 1) {
        if line.raw.trim().is_empty() {
            pending.push(line);
            continue;
        }
        // Inside a fence a comment is code; the line classification hides
        // a fence's lines (`visible` is `None`).
        if fence.is_none() && line.opens_in_comment && line.visible.is_some() {
            taken.append(&mut pending);
            taken.push(line);
            paragraph = true;
            continue;
        }
        let table = table_lines.contains(&line.number);
        let visible = line.visible.as_deref().filter(|_| !table);
        if fence.is_none()
            && line.visible.as_deref().is_some_and(|visible| {
                heading_level(past_columns(visible, marker_indent)).is_some()
            })
        {
            break;
        }
        if indent_columns(line.raw) <= marker_indent {
            // At or left of the marker the indentation is the container's;
            // a `<!--` opening the line as written starts an HTML block,
            // though the comment-free line no longer shows it.
            let lazy = pending.is_empty()
                && paragraph
                && !html_block(line.raw.trim_start_matches([' ', '\t']))
                && visible.is_some_and(|text| !opens_block(text.trim_start_matches([' ', '\t'])));
            if !lazy {
                break;
            }
            taken.push(line);
            continue;
        }
        if visible.is_some_and(nested_record) {
            break;
        }
        taken.append(&mut pending);
        taken.push(line);
        if let Some((marker, length)) = fence {
            // Code: `paragraph` stays false up to the closing fence.
            if closes_fence(past_columns(line.raw, marker_indent), marker, length) {
                fence = None;
            }
            continue;
        }
        fence = visible.and_then(|text| opens_fence(past_columns(text, marker_indent)));
        paragraph = fence.is_none()
            && !html_block(past_columns(line.raw, marker_indent))
            && visible.is_some_and(|text| {
                let text = past_columns(text, marker_indent);
                !thematic_break(text) && !html_block(text)
            });
    }
    taken
}

/// `line` past at most `columns` columns of leading blanks (a tab to the
/// next multiple of four; one that would cross `columns` stays).
fn past_columns(line: &str, columns: usize) -> &str {
    let mut column = 0;
    for (offset, c) in line.char_indices() {
        let next = match c {
            ' ' => column + 1,
            '\t' => column + 4 - column % 4,
            _ => return &line[offset..],
        };
        if next > columns {
            return &line[offset..];
        }
        column = next;
    }
    ""
}

/// A line that opens a block other than a paragraph continuation: a list
/// marker, a heading, a fence, a table row, a thematic break, an HTML block
/// or a block quote.
fn opens_block(line: &str) -> bool {
    let Some(content) = block_content(line) else {
        return false;
    };
    heading_level(line).is_some()
        || list_marker(content)
        || content.starts_with(['|', '>'])
        || content.starts_with("```")
        || content.starts_with("~~~")
        || thematic_break(line)
        || html_block(line)
}

/// A bullet (`-`, `*`, `+`) or an ordered marker (1–9 digits, `.` or `)`)
/// followed by a blank or the end.
fn list_marker(content: &str) -> bool {
    let marker = match content.as_bytes().first() {
        Some(b'-' | b'*' | b'+') => 1,
        Some(b'0'..=b'9') => {
            let digits = content.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 9 || !matches!(content.as_bytes().get(digits), Some(b'.' | b')')) {
                return false;
            }
            digits + 1
        }
        _ => return false,
    };
    matches!(content.as_bytes().get(marker), None | Some(b' ' | b'\t'))
}

/// Three or more `*`, `-` or `_` of one kind, blanks between allowed.
fn thematic_break(line: &str) -> bool {
    let Some(content) = block_content(line) else {
        return false;
    };
    let mut marks = content.bytes().filter(|&b| b != b' ' && b != b'\t');
    let Some(first) = marks.next() else {
        return false;
    };
    matches!(first, b'*' | b'-' | b'_') && marks.clone().all(|b| b == first) && marks.count() >= 2
}

/// `<` followed by a letter, `/`, `!` or `?`: a tag, a comment, a
/// declaration or a processing instruction opening an HTML block.
fn html_block(line: &str) -> bool {
    block_content(line).is_some_and(|content| {
        content
            .strip_prefix('<')
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'))
    })
}

/// One reading of a strong span's content as ID [separator [title]], in
/// offsets of the line (`docs/features/import-gaps.md` AC-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Split {
    /// The ID candidate, trimmed, not empty.
    pub id: Range<usize>,
    /// Right after the separator ending the ID, when the rest of the
    /// content up to the closing delimiter holds a title (not blank).
    pub title_from: Option<usize>,
    /// The ID ends right before a separator and no title follows it: the
    /// span held the separator, none is stripped after it.
    pub stripped_inside: bool,
}

/// The readings of a lead-in span's trimmed content, longest ID candidate
/// first: the whole content, then the part before each start of a listed
/// separator from the right (the longest separator starting there), each
/// candidate trimmed; empty candidates are none.
pub(crate) fn splits(line: &str, lead_in: &LeadIn, separators: &[String]) -> Vec<Split> {
    let content = &line[lead_in.content.clone()];
    let start = lead_in.content.start + (content.len() - content.trim_start().len());
    let trimmed = content.trim();
    let mut splits = vec![Split {
        id: start..start + trimmed.len(),
        title_from: None,
        stripped_inside: false,
    }];
    for at in (1..trimmed.len()).rev() {
        if !trimmed.is_char_boundary(at) {
            continue;
        }
        let Some(separator) = separators
            .iter()
            .filter(|separator| {
                !separator.is_empty() && trimmed[at..].starts_with(separator.as_str())
            })
            .max_by_key(|separator| separator.len())
        else {
            continue;
        };
        let id = trimmed[..at].trim_end();
        if id.is_empty() {
            continue;
        }
        let after = at + separator.len();
        let titled = !trimmed[after..].trim().is_empty();
        splits.push(Split {
            id: start..start + id.len(),
            title_from: titled.then_some(start + after),
            stripped_inside: !titled,
        });
    }
    splits
}

/// Where the text after a lead-in span starts in `line`: past one separator
/// unless the span already held it, each separator tried right after the
/// span (so one opening with a blank matches there) and then after the
/// blanks that follow; leading blanks of the text are the caller's.
pub(crate) fn after_span(
    line: &str,
    lead_in: &LeadIn,
    separators: &[String],
    stripped_inside: bool,
) -> usize {
    let after = lead_in.after.min(line.len());
    if stripped_inside {
        return after;
    }
    for from in [after, skip_blanks(line, after)] {
        for separator in separators {
            if line[from..].starts_with(separator.as_str()) {
                return from + separator.len();
            }
        }
    }
    after
}
