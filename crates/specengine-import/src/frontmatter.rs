//! Front-matter of a corpus document, read leniently: an imported corpus may
//! use any YAML, so only the block's extent and one top-level scalar (the
//! class key) are read. The strict reader is the spec parser of
//! `specengine-core` (`spec check`); the census does not share it.

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

/// One top-level `key: value` entry of a closed front-matter block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    /// 1-based line number.
    pub line: usize,
    /// The key as written, quotes removed.
    pub key: &'a str,
    /// The value when it is a single-line scalar on the key line: comment
    /// stripped, trimmed, quotes removed; `None` for an empty value, a block
    /// scalar, an unclosed quote or a value continued on indented lines.
    pub value: Option<&'a str>,
}

/// The top-level entries of the front-matter block, in order; none when the
/// block is absent or unclosed. Indented lines, comments, sequence items and
/// lines without a `key:` are not entries.
pub fn entries(text: &str) -> Vec<Entry<'_>> {
    let block = block(text);
    let mut entries = Vec::new();
    for (position, &(line, content)) in block.iter().enumerate() {
        if content.trim().is_empty()
            || content.starts_with([' ', '\t', '#'])
            || content
                .strip_prefix('-')
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
        {
            continue;
        }
        let Some((key, rest)) = split_key(content) else {
            continue;
        };
        let continued = block
            .get(position + 1)
            .is_some_and(|&(_, next)| next.starts_with([' ', '\t']) && !next.trim().is_empty());
        let value = strip_comment(rest).trim();
        let unclosed_quote = ['"', '\'']
            .iter()
            .any(|&quote| value.starts_with(quote) && (value.len() < 2 || !value.ends_with(quote)));
        let value =
            if value.is_empty() || continued || unclosed_quote || value.starts_with(['|', '>']) {
                None
            } else {
                Some(unquote(value).trim())
            };
        entries.push(Entry { line, key, value });
    }
    entries
}

/// The values of a closed front-matter block, by line: the scalar after a
/// `key:` (top-level or nested), a sequence item, or a line continuing a
/// scalar; YAML comments stripped, keys and comment lines left out. Each
/// line once, as written otherwise.
pub fn values(text: &str) -> Vec<(usize, &str)> {
    let mut values = Vec::new();
    for (line, content) in block(text) {
        let mut rest = content.trim_start_matches([' ', '\t']);
        if rest.starts_with('#') {
            continue;
        }
        while let Some(item) = rest
            .strip_prefix('-')
            .filter(|item| item.is_empty() || item.starts_with([' ', '\t']))
        {
            rest = item.trim_start_matches([' ', '\t']);
        }
        let value = split_key(rest).map_or(rest, |(_, value)| value);
        let value = strip_comment(value);
        if !value.trim().is_empty() {
            values.push((line, value));
        }
    }
    values
}

/// The lines of a closed front-matter block as (1-based line, content
/// without terminator); none when the block is absent or unclosed.
fn block(text: &str) -> Vec<(usize, &str)> {
    let bom = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    let mut lines = text[bom..].split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Vec::new();
    };
    if first.trim_end() != "---" {
        return Vec::new();
    }
    let mut block: Vec<(usize, &str)> = Vec::new();
    for (index, line) in lines.enumerate() {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        if content.trim_end() == "---" || content.trim_end() == "..." {
            return block;
        }
        block.push((index + 2, content));
    }
    Vec::new()
}

/// `key` and the rest after `key:` of an entry line: a quoted key up to its
/// closing quote, else up to the first `:` followed by a blank or the end.
fn split_key(content: &str) -> Option<(&str, &str)> {
    if let Some(quote) = content.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        let close = content[1..].find(quote)? + 1;
        let after = content[close + 1..].trim_start_matches([' ', '\t']);
        let rest = after.strip_prefix(':')?;
        if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
            return None;
        }
        let key = &content[1..close];
        return (!key.is_empty()).then_some((key, rest));
    }
    let mut from = 0;
    while let Some(offset) = content[from..].find(':') {
        let colon = from + offset;
        let rest = &content[colon + 1..];
        if rest.is_empty() || rest.starts_with([' ', '\t']) {
            let key = content[..colon].trim_end();
            return (!key.is_empty()).then_some((key, rest));
        }
        from = colon + 1;
    }
    None
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

/// A YAML comment starts with `#` after whitespace, outside quotes. A quote
/// opens a quoted scalar only where the scalar or a flow item (after `[`,
/// `{`, `,` or `:` inside a flow collection) starts, past any tag (`!…`) or
/// anchor (`&…`) and its blank: in `don't # c` the `'` is plain text and
/// `# c` a comment; in `!!str 'x # y'` the quotes hold the `#`. `''` inside
/// single quotes and a `\`-escaped character inside double quotes do not
/// close them.
fn strip_comment(value: &str) -> &str {
    let mut quote = None;
    let mut flow = 0usize;
    let mut item_start = true;
    let mut previous_space = true;
    // Inside a tag or an anchor token, which runs to the next blank.
    let mut property = false;
    let mut chars = value.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if property {
            property = !c.is_whitespace();
            previous_space = c.is_whitespace();
            continue;
        }
        if let Some(open) = quote {
            if open == '"' && c == '\\' {
                chars.next();
            } else if c == open {
                if open == '\'' && chars.peek().is_some_and(|&(_, next)| next == '\'') {
                    chars.next();
                } else {
                    quote = None;
                }
            }
            item_start = false;
            previous_space = false;
            continue;
        }
        match c {
            '"' | '\'' if item_start => quote = Some(c),
            '!' | '&' if item_start => {
                property = true;
                previous_space = false;
                continue;
            }
            '#' if previous_space => return &value[..index],
            '[' | '{' if item_start || flow > 0 => flow += 1,
            ']' | '}' if flow > 0 => flow -= 1,
            _ => {}
        }
        if !c.is_whitespace() {
            item_start = flow > 0 && matches!(c, '[' | '{' | ',' | ':');
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
