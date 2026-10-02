//! What the checks read from a file's bytes beside its parse: lines of
//! spans, the top-level front-matter keys as written (with their lines) and
//! their values ([`Written`], for the process rules), the text under a
//! span; and the `YYYY-MM-DD` dates.

use specengine_model::{FmValue, ParsedFile, Span};

use crate::front_matter;
use crate::lines::LineIndex;
use crate::yaml::{self, YValue};

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
    /// Top-level front-matter entries as written: the key (`None` for a key
    /// that is no scalar on its line: `? [a]`, `[a]:`, `*x :`) and the
    /// 1-based line the entry starts on, in source order.
    entries: Vec<(Option<String>, usize)>,
    /// The last line of the front-matter YAML; 0 without one.
    yaml_last_line: usize,
}

impl<'a> FileText<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let lines = LineIndex::new(bytes);
        let (entries, yaml_last_line) = std::str::from_utf8(bytes)
            .map(|text| top_level_entries(text, &lines))
            .unwrap_or_default();
        Self {
            bytes,
            lines,
            entries,
            yaml_last_line,
        }
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
        self.keys()
            .find(|&(name, _)| name == key)
            .map(|(_, line)| line)
    }

    /// Top-level keys as written, in source order.
    pub fn keys(&self) -> impl Iterator<Item = (&str, usize)> {
        self.entries
            .iter()
            .filter_map(|(name, line)| name.as_deref().map(|name| (name, *line)))
    }

    /// The written top-level key whose entry holds `line`; `None` outside
    /// the front-matter, before its first entry, in an entry whose key is no
    /// scalar (`? [a]`, `[a]:`, `*x :`), or without bytes.
    pub fn key_at(&self, line: usize) -> Option<&str> {
        if line > self.yaml_last_line {
            return None;
        }
        let after = self.entries.partition_point(|&(_, start)| start <= line);
        self.entries.get(after.checked_sub(1)?)?.0.as_deref()
    }

    /// Bytes were given.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The top-level front-matter entries as written, read back through the
    /// parse's YAML reader: (key, value) in source order, an entry whose key
    /// is no scalar left out. `None` without bytes, without a block, or when
    /// the block does not read as a mapping (the parse reported it).
    pub fn values(&self) -> Option<Vec<(String, Written)>> {
        let text = std::str::from_utf8(self.bytes).ok()?;
        let block = front_matter::split(text).block?;
        let root = yaml::parse(text.get(block.yaml)?).ok()?;
        match root.value {
            YValue::Null => Some(Vec::new()),
            YValue::Map(entries) => Some(
                entries
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let key = key.value.scalar_text()?.into_owned();
                        Some((key, Written::from_yaml(value.value)))
                    })
                    .collect(),
            ),
            _ => None,
        }
    }
}

/// A front-matter value as the process rules read it
/// (`docs/features/spec-check-process.md`, "Terms").
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Written {
    Null,
    Str(String),
    /// A number or a boolean.
    Scalar,
    Seq(Vec<Written>),
    /// A mapping: whether it holds no entry.
    Map {
        empty: bool,
    },
}

impl Written {
    /// Null, blank (empty or whitespace), `[]` or `{}`.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Null => true,
            Self::Str(text) => text.trim().is_empty(),
            Self::Scalar => false,
            Self::Seq(items) => items.is_empty(),
            Self::Map { empty } => *empty,
        }
    }

    fn from_yaml(value: YValue) -> Self {
        match value {
            YValue::Null => Self::Null,
            YValue::Str(text) => Self::Str(text),
            YValue::Bool(_) | YValue::Int(_) | YValue::UInt(_) | YValue::Float(_) => Self::Scalar,
            YValue::Seq(items) => Self::Seq(
                items
                    .into_iter()
                    .map(|item| Self::from_yaml(item.value))
                    .collect(),
            ),
            YValue::Map(entries) => Self::Map {
                empty: entries.is_empty(),
            },
        }
    }

    /// An untyped or mistyped value as the parse kept it (`extra`).
    pub fn from_kept(value: &FmValue) -> Self {
        match value {
            FmValue::Null => Self::Null,
            FmValue::Str(text) => Self::Str(text.clone()),
            FmValue::Bool(_) | FmValue::Int(_) | FmValue::UInt(_) | FmValue::Float(_) => {
                Self::Scalar
            }
            FmValue::Seq(items) => Self::Seq(items.iter().map(Self::from_kept).collect()),
            FmValue::Map(map) => Self::Map {
                empty: map.is_empty(),
            },
        }
    }
}

/// The top-level entries of the front-matter block and its last YAML line:
/// an entry starts on a line at the indentation of the block's first entry
/// holding `key:` (a plain or quoted key before `:` followed by a space or
/// the line end, after any `!tag` and `&anchor`), an explicit key `?` (its
/// key when it is a scalar on that line, else `None`), or a flow or alias
/// key (`[`, `{`, `*`: `None`); other lines at that indentation
/// (`- item`, `: value`) continue the entry before. Only meaningful when the
/// block parsed as a mapping.
fn top_level_entries(text: &str, lines: &LineIndex) -> (Vec<(Option<String>, usize)>, usize) {
    let layout = front_matter::split(text);
    let Some(block) = layout.block else {
        return (Vec::new(), 0);
    };
    let yaml = &text[block.yaml.clone()];
    let last_line = if yaml.is_empty() {
        0
    } else {
        lines.line(block.yaml.end - 1)
    };
    let mut entries = Vec::new();
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
        let content = without_properties(content);
        let entry = if let Some(key) = key_of(content) {
            Some(Some(key))
        } else if let Some(explicit) = explicit_key(content) {
            Some(explicit_scalar(explicit))
        } else if content.starts_with(['[', '{', '*']) {
            Some(None)
        } else {
            None
        };
        if let Some(key) = entry {
            entries.push((key, lines.line(line_start)));
        }
    }
    (entries, last_line)
}

/// `content` without its leading node properties: a `!tag` and an
/// `&anchor`, in either order, each followed by whitespace; empty when the
/// line holds nothing else.
fn without_properties(content: &str) -> &str {
    let mut rest = content;
    for _ in 0..2 {
        if !rest.starts_with(['!', '&']) {
            break;
        }
        rest = match rest.find([' ', '\t']) {
            Some(end) => rest[end..].trim_start_matches([' ', '\t']),
            None => "",
        };
    }
    rest
}

/// The text after `?` when `content` is an explicit mapping key (`?`
/// followed by whitespace or the line end).
fn explicit_key(content: &str) -> Option<&str> {
    let after = content.strip_prefix('?')?;
    (after.is_empty() || after.starts_with([' ', '\t']))
        .then(|| after.trim_start_matches([' ', '\t']))
}

/// The key of an explicit `? key` when it is a scalar written on that line
/// (plain or quoted, after any properties, a comment allowed); `None` for a
/// collection (`[a]`, `{a: 1}`, `- a`, `a: b`), an alias, a block scalar or
/// a key on the lines below.
fn explicit_scalar(after: &str) -> Option<String> {
    let text = without_properties(after);
    let rest_is_comment = |rest: &str| {
        let rest = rest.trim_start_matches([' ', '\t']);
        rest.is_empty() || rest.starts_with('#')
    };
    match text.as_bytes().first()? {
        quote @ (b'"' | b'\'') => {
            let close = text[1..].find(*quote as char)? + 1;
            rest_is_comment(&text[close + 1..]).then(|| text[1..close].to_owned())
        }
        b'[' | b'{' | b'*' | b'|' | b'>' | b'!' | b'&' | b'#' => None,
        b'-' if text[1..].is_empty() || text[1..].starts_with([' ', '\t']) => None,
        _ => {
            let end = text
                .char_indices()
                .find(|&(at, c)| c == '#' && text[..at].ends_with([' ', '\t']))
                .map_or(text.len(), |(at, _)| at);
            let key = text[..end].trim_end_matches([' ', '\t']);
            (!key.is_empty() && find_key_colon(key).is_none()).then(|| key.to_owned())
        }
    }
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
