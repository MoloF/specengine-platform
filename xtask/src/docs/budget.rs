//! §4 caps and the working-set W report (§3).
//!
//! Caps are bytes of the whole file, front-matter included. §4 values are taken as is;
//! the Tier 2 canon cap is this repository's calibration (`docs/README.md`, "Budgets").
//! Overflow moves content down a tier; caps are never raised (§4).

use std::io;
use std::path::Path;

use super::{Doc, INDEX_PATH};

pub const TIER0: usize = 16 * 1024;
pub const TIER1: usize = 10 * 1024;
pub const CANON_TIER2: usize = 12 * 1024;
pub const INDEX: usize = 10 * 1024;
pub const DECISION: usize = 1536;

/// Target working set of a task, §4: 20–40 KB.
const W_TARGET: usize = 40 * 1024;

/// Cap of a document; `None` means a class without a cap (spec, generated except the index).
pub fn cap(doc: &Doc) -> Option<usize> {
    if doc.path == INDEX_PATH {
        return Some(INDEX);
    }
    match (doc.class(), doc.tier()) {
        (Some("canon"), Some(0)) => Some(TIER0),
        (Some("canon"), Some(1)) => Some(TIER1),
        (Some("canon"), _) => Some(CANON_TIER2),
        (Some("decision"), _) => Some(DECISION),
        _ => None,
    }
}

pub fn run(root: &Path) -> io::Result<bool> {
    let docs = super::load(root)?;
    println!(
        "{:<62} {:>9} {:>7} {:>7}",
        "document", "class", "bytes", "cap"
    );
    for doc in &docs {
        let cap_text = cap(doc).map_or("—".to_string(), |c| c.to_string());
        let mark = match cap(doc) {
            Some(c) if doc.bytes > c => "  OVER CAP",
            _ => "",
        };
        println!(
            "{:<62} {:>9} {:>7} {:>7}{mark}",
            doc.path,
            doc.class().unwrap_or("?"),
            doc.bytes,
            cap_text
        );
    }

    let tier0 = docs
        .iter()
        .filter(|d| d.tier() == Some(0))
        .map(|d| d.bytes)
        .sum::<usize>();
    let tier1 = docs
        .iter()
        .filter(|d| d.tier() == Some(1))
        .map(|d| d.bytes)
        .max()
        .unwrap_or(0);
    let index = docs
        .iter()
        .find(|d| d.path == INDEX_PATH)
        .map_or(0, |d| d.bytes);
    let mut tier2: Vec<usize> = docs
        .iter()
        .filter(|d| d.path != INDEX_PATH && !d.is_archived())
        .filter(|d| !matches!(d.tier(), Some(0 | 1)))
        .map(|d| d.bytes)
        .collect();
    tier2.sort_unstable_by(|a, b| b.cmp(a));
    let k3: usize = tier2.iter().take(3).sum();
    let w = tier0 + tier1 + index + k3;
    println!(
        "\nW (worst case) = Tier 0 {tier0} + Tier 1 {tier1} + index {index} + three largest Tier 2 {k3} = {w} bytes; §4 target <= {W_TARGET}"
    );
    Ok(true)
}
