//! Front-matter of a corpus document, read leniently: an imported corpus may
//! use any YAML, so only the block's extent and one top-level scalar (the
//! class key) are read. The strict reader of this repository's own documents
//! lives in `xtask`, which stays dependency-free and is not shared.

/// What the start of a document holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontMatter {
    /// The document does not open with `---`.
    Absent,
    /// Opened with `---` and never closed: read as body, reported.
    Unclosed,
    /// A closed block.
    Present {
        /// Value of the class key; `None` when the key is absent or empty.
        class: Option<String>,
        /// Byte offset where the body starts (after the closing line).
        body_start: usize,
        /// 1-based line number of the body's first line.
        body_line: usize,
    },
}

/// Reads the front-matter block: `---` on the first line (after an optional
/// BOM), closed by `---` or `...`.
pub fn read(text: &str, class_key: Option<&str>) -> FrontMatter {
    let bom = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    let mut lines = text[bom..].split_inclusive('\n');
    let Some(first) = lines.next() else {
        return FrontMatter::Absent;
    };
    if first.trim_end() != "---" {
        return FrontMatter::Absent;
    }
    let mut offset = bom + first.len();
    let mut class = None;
    for (index, line) in lines.enumerate() {
        let content = line.trim_end();
        if content == "---" || content == "..." {
            return FrontMatter::Present {
                class,
                body_start: offset + line.len(),
                // Line 1 is the opening `---`, `index` 0 is line 2.
                body_line: index + 3,
            };
        }
        if let Some(key) = class_key
            && class.is_none()
        {
            class = class_value(content, key);
        }
        offset += line.len();
    }
    FrontMatter::Unclosed
}

/// The scalar of a top-level `key: value` line when `key` matches.
fn class_value(line: &str, key: &str) -> Option<String> {
    if line.starts_with([' ', '\t']) {
        return None;
    }
    let (name, value) = line.split_once(':')?;
    if unquote(name.trim()) != key {
        return None;
    }
    let value = unquote(strip_comment(value).trim()).trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// A YAML comment starts with `#` after whitespace, outside quotes.
fn strip_comment(value: &str) -> &str {
    let mut quote = None;
    let mut previous_space = true;
    for (index, c) in value.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, '#') if previous_space => return &value[..index],
            _ => {}
        }
        previous_space = c.is_whitespace();
    }
    value
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}
