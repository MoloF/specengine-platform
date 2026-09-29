//! What the checks read from a file's bytes beside its parse: lines of
//! spans, the top-level front-matter keys as written (with their lines),
//! the text under a span; and the `YYYY-MM-DD` dates.

use specengine_model::{ParsedFile, Span};

use crate::front_matter;
use crate::lines::LineIndex;

/// The 1-based line of `offset` in `text`.
pub(crate) fn line_of_str(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

/// One file's bytes, for lines, key positions and span text.
pub(crate) struct FileText<'a> {
    bytes: &'a [u8],
    lines: LineIndex,
    /// Top-level front-matter keys as written, with their 1-based lines,
    /// in source order.
    keys: Vec<(String, usize)>,
}

impl<'a> FileText<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let lines = LineIndex::new(bytes);
        let keys = std::str::from_utf8(bytes)
            .map(|text| top_level_keys(text, &lines))
            .unwrap_or_default();
        Self { bytes, lines, keys }
    }

    /// The line of a byte offset; 1 without bytes.
    pub fn line(&self, offset: usize) -> usize {
        if self.bytes.is_empty() {
            1
        } else {
            self.lines.line(offset)
        }
    }

    /// The text under `span`, lossy for bytes that are not UTF-8; empty
    /// when the span lies outside the bytes.
    pub fn text(&self, span: Span) -> String {
        self.bytes
            .get(span.range())
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .unwrap_or_default()
    }

    /// The line of top-level key `key`, when written.
    pub fn key_line(&self, key: &str) -> Option<usize> {
        self.keys
            .iter()
            .find(|(name, _)| name == key)
            .map(|&(_, line)| line)
    }

    /// Top-level keys as written, in source order.
    pub fn keys(&self) -> impl Iterator<Item = (&str, usize)> {
        self.keys.iter().map(|(name, line)| (name.as_str(), *line))
    }

    /// Bytes were given.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// The top-level keys of the front-matter block: lines at the indentation
/// of the block's first entry holding `key:` (a plain or quoted key before
/// `:` followed by a space or the line end). Only meaningful when the block
/// parsed as a mapping.
fn top_level_keys(text: &str, lines: &LineIndex) -> Vec<(String, usize)> {
    let layout = front_matter::split(text);
    let Some(block) = layout.block else {
        return Vec::new();
    };
    let yaml = &text[block.yaml.clone()];
    let mut keys = Vec::new();
    let mut indent: Option<usize> = None;
    let mut offset = block.yaml.start;
    for raw in yaml.split_inclusive('\n') {
        let line_start = offset;
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        let content = line.trim_start_matches(' ');
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        let this_indent = line.len() - content.len();
        let top = *indent.get_or_insert(this_indent);
        if this_indent != top {
            continue;
        }
        if let Some(key) = key_of(content) {
            keys.push((key, lines.line(line_start)));
        }
    }
    keys
}

/// `key` of a `key: value` / `key:` line; quoted keys unquoted.
fn key_of(content: &str) -> Option<String> {
    let (key, rest) = match content.as_bytes().first()? {
        quote @ (b'"' | b'\'') => {
            let close = content[1..].find(*quote as char)? + 1;
            (&content[1..close], &content[close + 1..])
        }
        b'-' | b'?' | b'{' | b'[' | b'&' | b'*' | b'!' | b'|' | b'>' => return None,
        _ => {
            let colon = find_key_colon(content)?;
            (content[..colon].trim_end(), &content[colon..])
        }
    };
    let after = rest.strip_prefix(':')?;
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    (!key.is_empty()).then(|| key.to_owned())
}

/// The first `:` followed by whitespace or the end.
fn find_key_colon(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    (0..bytes.len()).find(|&at| {
        bytes[at] == b':'
            && bytes
                .get(at + 1)
                .is_none_or(|next| matches!(next, b' ' | b'\t'))
    })
}

/// The parse failed to read the front-matter (or the file): not UTF-8, an
/// unclosed block, YAML that does not parse or is no mapping.
pub(crate) fn front_matter_failed(parsed: &ParsedFile) -> bool {
    use specengine_model::DiagnosticCode as Code;
    parsed.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code,
            Code::NotUtf8
                | Code::FrontmatterUnclosed
                | Code::FrontmatterYaml
                | Code::FrontmatterNotMapping
        )
    })
}

/// `YYYY-MM-DD` by shape: four digits, `-`, two, `-`, two.
pub fn is_date_shaped(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

/// A calendar date `YYYY-MM-DD`: shaped, month 01–12, day within the month.
pub fn is_calendar_date(text: &str) -> bool {
    if !is_date_shaped(text) {
        return false;
    }
    let number = |range: std::ops::Range<usize>| text[range].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day)) = (number(0..4), number(5..7), number(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// The UTC calendar date of a count of days since 1970-01-01, as
/// `YYYY-MM-DD` (the civil-from-days algorithm; no date crate).
pub fn date_from_unix_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
