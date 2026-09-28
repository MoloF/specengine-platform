//! Model of the documentation corpus: file discovery, front-matter, headings and anchors.
//!
//! The norm is `docs/canon/documentation-system.md`; how it is applied in this
//! repository is `docs/README.md`.

pub mod budget;
pub mod check;
pub mod frontmatter;
pub mod index;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use frontmatter::FrontMatter;

/// Directories without documents: build output, dependencies, agent configuration.
/// `.claude/` holds role and command prompts, not documentation: they carry Claude Code front-matter.
/// `fixtures/` holds test corpora with foreign conventions, not documents of this repository.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".claude",
    "fixtures",
    ".github",
    "target",
    "target.noindex",
    "node_modules",
    "dist",
];

/// Path of the generated index (§9).
pub const INDEX_PATH: &str = "docs/index.md";

/// A corpus document: path relative to the root, size and parsed front-matter.
#[derive(Debug)]
pub struct Doc {
    pub path: String,
    pub bytes: usize,
    pub front_matter: Result<Option<FrontMatter>, String>,
    pub title: Option<String>,
    pub anchors: HashSet<String>,
}

impl Doc {
    pub fn fm(&self) -> Option<&FrontMatter> {
        self.front_matter.as_ref().ok().and_then(Option::as_ref)
    }

    pub fn class(&self) -> Option<&str> {
        self.fm().and_then(|fm| fm.str("class"))
    }

    pub fn tier(&self) -> Option<u8> {
        self.fm()
            .and_then(|fm| fm.str("tier"))
            .and_then(|t| t.parse().ok())
    }

    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Whether the document is excluded from default retrieval (Tier 3, §6: status, not folder).
    pub fn is_archived(&self) -> bool {
        let Some(fm) = self.fm() else { return false };
        match (fm.str("class"), fm.str("status")) {
            (Some("spec"), Some("shipped" | "abandoned")) => true,
            (Some("decision"), Some(status)) => status != "accepted",
            _ => false,
        }
    }
}

/// All documents of the repository in a deterministic order.
/// Files starting with `_` are templates, not documents.
pub fn load(root: &Path) -> io::Result<Vec<Doc>> {
    let mut paths = Vec::new();
    walk(root, root, &mut paths)?;
    paths.sort();

    let mut docs = Vec::with_capacity(paths.len());
    for rel in paths {
        let text = fs::read_to_string(root.join(&rel))?;
        let front_matter = frontmatter::split(&text).map(|parsed| parsed.map(|(fm, _)| fm));
        let body = match frontmatter::split(&text) {
            Ok(Some((_, body))) => body,
            _ => text.as_str(),
        };
        docs.push(Doc {
            bytes: text.len(),
            title: first_heading(body),
            anchors: anchors(body),
            front_matter,
            path: rel,
        });
    }
    Ok(docs)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !SKIP_DIRS.contains(&name.as_ref()) {
                walk(root, &path, out)?;
            }
        } else if name.ends_with(".md") && !name.starts_with('_') {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

fn first_heading(body: &str) -> Option<String> {
    code_free_lines(body)
        .find_map(|line| line.strip_prefix("# "))
        .map(|t| strip_attr(t).trim().to_string())
}

/// Document anchors: GitHub heading slugs, a trailing `{#id}` in a heading, and `<a id="…">`.
pub fn anchors(body: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in code_free_lines(body) {
        if let Some(text) = heading_text(line) {
            if let Some(id) = explicit_attr(text) {
                set.insert(id.to_string());
            }
            let base = slug(strip_attr(text));
            let n = seen.entry(base.clone()).or_insert(0);
            let s = if *n == 0 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            *n += 1;
            set.insert(s);
        }
        let mut rest = line;
        while let Some(pos) = rest.find("<a ") {
            rest = &rest[pos + 3..];
            for attr in ["id=\"", "name=\""] {
                if let Some(start) = rest.find(attr) {
                    let value = &rest[start + attr.len()..];
                    if let Some(end) = value.find('"') {
                        set.insert(value[..end].to_string());
                    }
                }
            }
        }
    }
    set
}

fn heading_text(line: &str) -> Option<&str> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some(line[hashes..].trim())
    } else {
        None
    }
}

fn explicit_attr(text: &str) -> Option<&str> {
    let text = text.trim_end();
    let inner = text.strip_suffix('}')?;
    let start = inner.rfind("{#")?;
    Some(&inner[start + 2..])
}

fn strip_attr(text: &str) -> &str {
    match text.trim_end().rfind("{#") {
        Some(pos) if text.trim_end().ends_with('}') => text[..pos].trim_end(),
        _ => text,
    }
}

/// Heading slug by GitHub's rule: lowercase, letters and digits of any script,
/// `-` and `_` are kept, a space becomes `-`, other punctuation is dropped.
pub fn slug(text: &str) -> String {
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

/// Lines outside fenced code blocks.
fn code_free_lines(body: &str) -> impl Iterator<Item = &str> {
    let mut fence: Option<&str> = None;
    body.lines().filter(move |line| {
        let trimmed = line.trim_start();
        for marker in ["```", "~~~"] {
            if trimmed.starts_with(marker) {
                match fence {
                    Some(open) if open == marker => fence = None,
                    None => fence = Some(marker),
                    _ => {}
                }
                return false;
            }
        }
        fence.is_none()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_github_for_cyrillic_and_punctuation() {
        assert_eq!(slug("1. Αποθήκευση"), "1-αποθήκευση"); // non-Latin script
        assert_eq!(slug("Tier 0 — `CLAUDE.md`"), "tier-0--claudemd");
    }

    #[test]
    fn anchors_include_explicit_ids_and_skip_code() {
        let body = "# Title\n<a id=\"storage\"></a>\n## Κανόνες {#RULE-1}\n```\n## not a heading\n```\n## Κανόνες\n";
        let set = anchors(body);
        assert!(set.contains("storage"));
        assert!(set.contains("RULE-1"));
        assert!(set.contains("κανόνες"));
        assert!(set.contains("κανόνες-1"));
        assert!(!set.contains("not-a-heading"));
    }
}
