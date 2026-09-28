//! Document front-matter: the subset of YAML that the §8 contract needs:
//! `key: value`, `key: [a, b]` and a block list `key:` + lines `  - a`.
//! Anything else is an error with a line number, never a silent best guess.

/// Value of a front-matter key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    List(Vec<String>),
}

/// Parsed front-matter in the file's key order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontMatter {
    pub entries: Vec<(String, Value)>,
}

impl FrontMatter {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Str(s) => Some(s),
            Value::List(_) => None,
        }
    }

    /// A list; a scalar reads as a one-element list.
    pub fn list(&self, key: &str) -> Option<Vec<&str>> {
        match self.get(key)? {
            Value::Str(s) => Some(vec![s.as_str()]),
            Value::List(items) => Some(items.iter().map(String::as_str).collect()),
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
}

/// Splits front-matter from the body. `Ok(None)`: there is no front-matter.
pub fn split(text: &str) -> Result<Option<(FrontMatter, &str)>, String> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Ok(None);
    };
    let mut offset = 0;
    let mut header_end = None;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            header_end = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let (end, body_start) = header_end.ok_or("front-matter opened with `---` but never closed")?;
    let fm = parse(&rest[..end])?;
    Ok(Some((fm, &rest[body_start..])))
}

fn parse(header: &str) -> Result<FrontMatter, String> {
    let mut entries: Vec<(String, Value)> = Vec::new();
    let mut open_list: Option<usize> = None;

    for (i, raw) in header.lines().enumerate() {
        let n = i + 2; // line 1 is the opening `---`
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            let item = line.trim_start();
            let Some(item) = item.strip_prefix("- ") else {
                return Err(format!("line {n}: expected a list item `  - …`"));
            };
            let Some(idx) = open_list else {
                return Err(format!("line {n}: list item without a key"));
            };
            if let Value::List(items) = &mut entries[idx].1 {
                items.push(unquote(item.trim()).to_string());
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("line {n}: expected `key: value`"));
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(format!("line {n}: key `{key}` — only a-z, 0-9, `_`, `-`"));
        }
        if entries.iter().any(|(k, _)| k == key) {
            return Err(format!("line {n}: key `{key}` is repeated"));
        }
        let value = value.trim();
        let parsed = if value.is_empty() {
            open_list = Some(entries.len());
            Value::List(Vec::new())
        } else if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            open_list = None;
            Value::List(
                inner
                    .split(',')
                    .map(|s| unquote(s.trim()).to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
            )
        } else {
            open_list = None;
            Value::Str(unquote(value).to_string())
        };
        entries.push((key.to_string(), parsed));
    }
    Ok(FrontMatter { entries })
}

/// A comment starts with ` #` outside quotes; `path#anchor` is not a comment.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut prev_space = false;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '#') if prev_space || i == 0 => return &line[..i],
            _ => {}
        }
        prev_space = c == ' ';
    }
    line
}

fn unquote(s: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = s.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_inline_and_block_lists() {
        let text = "---\nid: ADR-0001\ncanon: docs/a.md#x   # see §5\nscope: [docs, arch]\nsupersedes:\n  - ADR-0002\n---\n# T\n";
        let (fm, body) = split(text).unwrap().unwrap();
        assert_eq!(fm.str("id"), Some("ADR-0001"));
        assert_eq!(fm.str("canon"), Some("docs/a.md#x"));
        assert_eq!(fm.list("scope"), Some(vec!["docs", "arch"]));
        assert_eq!(fm.list("supersedes"), Some(vec!["ADR-0002"]));
        assert_eq!(body, "# T\n");
    }

    #[test]
    fn rejects_unclosed_and_duplicate_keys() {
        assert!(split("---\nid: x\n").is_err());
        assert!(split("---\nid: x\nid: y\n---\n").is_err());
        assert!(split("no header").unwrap().is_none());
    }
}
