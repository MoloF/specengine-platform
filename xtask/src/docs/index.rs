//! The generated index, §9: one line per document — id, title, scope, status.
//! Built from front-matter, so it cannot disagree with reality.
//! Tier 3 (shipped and abandoned specs, superseded and rejected decisions) is a separate
//! section: excluded from default retrieval, but reachable by id. Its lines carry only
//! the id and the status, whole (`superseded-by ADR-0026`); title and scope are not read
//! (ADR-0028).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use super::{Doc, INDEX_PATH};

const HEADER: &str = "\
---
class: generated
generator: cargo xtask docs index --write
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `cargo xtask docs index --write`. Manual edits are overwritten on rebuild, and `cargo xtask docs check` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.
";

pub fn render(docs: &[Doc]) -> String {
    let live: Vec<&Doc> = docs
        .iter()
        .filter(|d| d.path != INDEX_PATH && !d.is_archived())
        .collect();
    let archived: Vec<&Doc> = docs.iter().filter(|d| d.is_archived()).collect();

    let mut out = String::from(HEADER);
    section(
        &mut out,
        "Canon",
        live.iter().filter(|d| d.class() == Some("canon")),
    );
    section(
        &mut out,
        "Decisions",
        live.iter().filter(|d| d.class() == Some("decision")),
    );
    section(
        &mut out,
        "Specs",
        live.iter().filter(|d| d.class() == Some("spec")),
    );
    section(
        &mut out,
        "No class — fix",
        live.iter()
            .filter(|d| !matches!(d.class(), Some("canon" | "decision" | "spec" | "generated"))),
    );
    section(&mut out, "Archive — Tier 3, by id only", archived.iter());
    out
}

fn section<'a>(out: &mut String, title: &str, docs: impl Iterator<Item = &'a &'a Doc>) {
    let lines: Vec<String> = docs.map(|d| line(d)).collect();
    if lines.is_empty() {
        return;
    }
    let _ = write!(out, "\n## {title}\n\n");
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
}

/// `- [label](link) title · scope · status`; Tier 3: `- [label](link) status`.
fn line(doc: &Doc) -> String {
    let fm = doc.fm();
    let link = doc
        .path
        .strip_prefix("docs/")
        .map_or_else(|| format!("../{}", doc.path), str::to_string);
    let status = match doc.class() {
        Some("canon") => format!("tier {}", doc.tier().map_or("?".into(), |t| t.to_string())),
        _ => fm
            .and_then(|fm| fm.str("status"))
            .unwrap_or("?")
            .to_string(),
    };
    let (label, title) = match (doc.class(), fm.and_then(|fm| fm.str("id"))) {
        (Some("decision"), Some(id)) => (id, fm.and_then(|fm| fm.str("title"))),
        _ => (doc.path.as_str(), doc.title.as_deref()),
    };
    if doc.is_archived() {
        return format!("- [{label}]({link}) {status}");
    }
    let title = title.unwrap_or("?");
    let scope = fm
        .and_then(|fm| fm.list("scope"))
        .map(|s| s.join(", "))
        .unwrap_or_default();
    format!("- [{label}]({link}) {title} · {scope} · {status}")
}

pub fn run(root: &Path, write: bool) -> io::Result<bool> {
    let docs = super::load(root)?;
    let text = render(&docs);
    if write {
        fs::write(root.join(INDEX_PATH), &text)?;
        println!("wrote {INDEX_PATH}: {} bytes", text.len());
    } else {
        print!("{text}");
    }
    Ok(true)
}
