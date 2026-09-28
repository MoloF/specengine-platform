//! The single documentation check, §11 of `docs/canon/documentation-system.md`:
//!
//! 1. budgets — Tier 0, every Tier 1, the index, canon and decisions within their caps;
//! 2. front-matter is present and matches the schema of its class (§8);
//! 3. every `accepted` decision has a `canon:` that resolves to a canon section (§5);
//! 4. every `superseded-by` and `supersedes` target exists;
//! 5. the index is regenerated and matches the file (§9);
//! 6. generated documents are regenerated and unchanged (§7).
//!
//! Output is one line per finding plus one summary line: a gate that prints a wall of
//! text on every commit gets switched off.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;

use super::{Doc, INDEX_PATH, budget, index};

const CLASSES: &[&str] = &["canon", "decision", "spec", "generated"];
const SPEC_STATUS: &[&str] = &["draft", "in-progress", "shipped", "abandoned"];

/// Known generators: `generator:` value → the path it writes.
const GENERATORS: &[(&str, &str)] = &[("cargo xtask docs index --write", INDEX_PATH)];

struct Findings(Vec<(String, String)>);

impl Findings {
    fn add(&mut self, doc: &Doc, msg: impl Into<String>) {
        self.0.push((doc.path.clone(), msg.into()));
    }
}

pub fn run(root: &Path) -> io::Result<bool> {
    let docs = super::load(root)?;
    let mut f = Findings(Vec::new());

    let by_path: HashMap<&str, &Doc> = docs.iter().map(|d| (d.path.as_str(), d)).collect();
    let mut adr_ids: HashMap<String, &Doc> = HashMap::new();
    for doc in &docs {
        if let (Some("decision"), Some(id)) = (doc.class(), doc.fm().and_then(|fm| fm.str("id")))
            && let Some(prev) = adr_ids.insert(id.to_string(), doc)
        {
            f.add(doc, format!("id {id} is already taken by {}", prev.path));
        }
    }

    for doc in &docs {
        schema(doc, &mut f);
        budgets(doc, &mut f);
        references(doc, &by_path, &adr_ids, &mut f);
    }
    generated(root, &docs, &mut f)?;

    for (path, msg) in &f.0 {
        println!("error  {path}: {msg}");
    }
    let errors = f.0.len();
    println!(
        "docs check: {} documents, {errors} errors{}",
        docs.len(),
        if errors == 0 { " — clean" } else { "" }
    );
    Ok(errors == 0)
}

/// §8: front-matter exists, the class is known, required keys are present, no extra keys.
fn schema(doc: &Doc, f: &mut Findings) {
    let fm = match &doc.front_matter {
        Err(err) => return f.add(doc, format!("front-matter does not parse: {err}")),
        Ok(None) => {
            return f.add(doc, "no front-matter (§8): the first line must be `---`");
        }
        Ok(Some(fm)) => fm,
    };
    let Some(class) = fm.str("class") else {
        return f.add(
            doc,
            "no `class:` — one of canon | decision | spec | generated",
        );
    };
    if !CLASSES.contains(&class) {
        return f.add(
            doc,
            format!("class `{class}` is unknown — canon | decision | spec | generated"),
        );
    }

    let (required, optional): (&[&str], &[&str]) = match class {
        "canon" => (&["class", "tier", "scope", "owner", "reviewed"], &[]),
        "decision" => (
            &["class", "id", "title", "status", "date", "scope"],
            &["canon", "supersedes", "ref"],
        ),
        "spec" => (&["class", "status", "scope"], &["ref", "shipped", "adrs"]),
        _ => (&["class", "generator", "source"], &[]),
    };
    for key in required {
        if fm.get(key).is_none() {
            f.add(
                doc,
                format!("missing required key `{key}` for class {class}"),
            );
        }
    }
    for key in fm.keys() {
        if !required.contains(&key) && !optional.contains(&key) {
            f.add(
                doc,
                format!("key `{key}` is not part of the {class} contract (§8)"),
            );
        }
    }
    if fm.list("scope").is_some_and(|s| s.is_empty()) {
        f.add(doc, "`scope` is empty — routing depends on it");
    }
    for key in ["reviewed", "date", "shipped"] {
        if let Some(value) = fm.str(key)
            && !is_date(value)
        {
            f.add(
                doc,
                format!("`{key}: {value}` — expected a YYYY-MM-DD date"),
            );
        }
    }

    match class {
        "canon" => canon_schema(doc, f),
        "decision" => decision_schema(doc, f),
        "spec" => {
            let status = fm.str("status").unwrap_or("");
            if !SPEC_STATUS.contains(&status) {
                f.add(
                    doc,
                    format!("status `{status}` — draft | in-progress | shipped | abandoned"),
                );
            }
            if status == "shipped" && fm.get("shipped").is_none() {
                f.add(doc, "shipped spec without a `shipped:` date");
            }
        }
        _ => {
            let generator = fm.str("generator").unwrap_or("");
            match GENERATORS.iter().find(|(g, _)| *g == generator) {
                None => f.add(doc, format!("generator `{generator}` is unknown to xtask")),
                Some((_, path)) if *path != doc.path => f.add(
                    doc,
                    format!("generator `{generator}` writes {path}, not this file"),
                ),
                _ => {}
            }
        }
    }
}

fn canon_schema(doc: &Doc, f: &mut Findings) {
    let tier = doc.fm().and_then(|fm| fm.str("tier")).unwrap_or("");
    match tier {
        "0" if doc.path != "CLAUDE.md" => f.add(doc, "Tier 0 is only the root CLAUDE.md"),
        "1" if doc.file_name() != "README.md" => f.add(
            doc,
            "Tier 1 is subtree canon: a README.md next to what it describes",
        ),
        "0" | "1" | "2" => {}
        _ => f.add(
            doc,
            format!("tier `{tier}` — canon is 0, 1 or 2 (Tier 3 is a status)"),
        ),
    }
    if doc.path == "CLAUDE.md" && tier != "0" {
        f.add(doc, "CLAUDE.md is the root canon, tier: 0");
    }
}

fn decision_schema(doc: &Doc, f: &mut Findings) {
    let Some(fm) = doc.fm() else { return };
    let id = fm.str("id").unwrap_or("");
    if !is_adr_id(id) {
        f.add(doc, format!("id `{id}` — expected ADR-NNNN"));
    } else if !doc.file_name().starts_with(id) {
        f.add(doc, format!("file name must start with {id}"));
    }
    let status = fm.str("status").unwrap_or("");
    let ok = matches!(status, "accepted" | "rejected")
        || status.strip_prefix("superseded-by ").is_some_and(is_adr_id);
    if !ok {
        f.add(
            doc,
            format!("status `{status}` — accepted | rejected | superseded-by ADR-NNNN"),
        );
    }
    if status == "accepted" && fm.get("canon").is_none() {
        f.add(
            doc,
            "accepted decision without `canon:` — promotion rule §5",
        );
    }
}

fn budgets(doc: &Doc, f: &mut Findings) {
    if let Some(cap) = budget::cap(doc)
        && doc.bytes > cap
    {
        f.add(
            doc,
            format!(
                "{} bytes over the cap of {cap} (§4): move detail down a tier; caps are never raised",
                doc.bytes
            ),
        );
    }
}

/// §5 and references between decisions and specs.
fn references(
    doc: &Doc,
    by_path: &HashMap<&str, &Doc>,
    adr_ids: &HashMap<String, &Doc>,
    f: &mut Findings,
) {
    let Some(fm) = doc.fm() else { return };

    if let Some(canon) = fm.str("canon") {
        match canon.split_once('#') {
            None => f.add(
                doc,
                format!("`canon: {canon}` — expected path#anchor of a section"),
            ),
            Some((path, anchor)) => match by_path.get(path) {
                None => f.add(doc, format!("`canon:` points at a missing {path}")),
                Some(target) if target.class() != Some("canon") => f.add(
                    doc,
                    format!("`canon:` points at {path}, which is not canon"),
                ),
                Some(target) if !target.anchors.contains(anchor) => {
                    f.add(doc, format!("{path} has no section #{anchor}"))
                }
                Some(_) => {}
            },
        }
    }

    let mut targets: Vec<&str> = Vec::new();
    if let Some(target) = fm
        .str("status")
        .and_then(|s| s.strip_prefix("superseded-by "))
    {
        targets.push(target);
    }
    for key in ["supersedes", "adrs"] {
        targets.extend(fm.list(key).unwrap_or_default());
    }
    let unique: HashSet<&str> = targets.into_iter().collect();
    let mut unique: Vec<&str> = unique.into_iter().collect();
    unique.sort_unstable();
    for id in unique {
        if !adr_ids.contains_key(id) {
            f.add(doc, format!("reference to {id}, no such decision"));
        }
    }
}

/// §7 and §9: every generated document matches what its generator produces now.
fn generated(root: &Path, docs: &[Doc], f: &mut Findings) -> io::Result<()> {
    let expected = index::render(docs);
    match docs.iter().find(|d| d.path == INDEX_PATH) {
        None => f.0.push((
            INDEX_PATH.to_string(),
            "index is missing — `cargo xtask docs index --write`".to_string(),
        )),
        Some(doc) => {
            let actual = fs::read_to_string(root.join(INDEX_PATH))?;
            if actual != expected {
                f.add(
                    doc,
                    "drifted from front-matter (edited by hand or not regenerated) — `cargo xtask docs index --write`",
                );
            }
        }
    }
    Ok(())
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn is_adr_id(s: &str) -> bool {
    s.strip_prefix("ADR-")
        .is_some_and(|n| n.len() == 4 && n.bytes().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_ids() {
        assert!(is_date("2026-09-28"));
        assert!(!is_date("2026-9-28"));
        assert!(is_adr_id("ADR-0042"));
        assert!(!is_adr_id("ADR-42"));
        assert!(!is_adr_id("\u{0410}DR-0042")); // Cyrillic A homoglyph
    }
}
