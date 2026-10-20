//! RD-6 of docs/features/roadmap-items.md (ADR-0008, "Rules and edge
//! cases": the core takes a release, priority, unit or prefix from
//! `[roadmap]` only): no string literal of core `src/roadmap.rs`,
//! `src/roadmap_toml.rs` or CLI `src/roadmap.rs` equals a prefix, kind,
//! release, priority or unit of either fixture (`roadmap-a`,
//! `roadmap-b`) or of this repository's `[roadmap]` (its values as the
//! spec's "This repository" gives them, and the root `specengine.toml`'s
//! table once it has one).
//!
//! M6: a hard-coded `"mvp"` turns `roadmap_sources_name_no_vocabulary`
//! red. A module's own `#[cfg(test)] mod` (over fixtures, by design) ends
//! what is scanned, as in the other genre scans.

mod common;

use std::collections::BTreeSet;
use std::fs;

use common::{fixture, repository_root};

/// The three sources of the rule.
const SOURCES: [&str; 3] = [
    "crates/specengine-core/src/roadmap.rs",
    "crates/specengine-core/src/roadmap_toml.rs",
    "crates/specengine-cli/src/roadmap.rs",
];

/// This repository's `[ids] RM` and `[roadmap]` (the spec's "This
/// repository"): prefix, kind, releases, priorities, unit.
const THIS_REPOSITORY: [&str; 9] = [
    "RM",
    "plan-item",
    "mvp",
    "v1",
    "v2",
    "must",
    "should",
    "could",
    "days",
];

/// The words of one `specengine.toml`: every `[ids]` prefix, alias and
/// kind, and `[roadmap]`'s prefix, releases, priorities and unit.
fn words_of(text: &str, out: &mut BTreeSet<String>) {
    let table: toml::Table = text.parse().expect("a TOML config");
    if let Some(ids) = table.get("ids").and_then(toml::Value::as_table) {
        for (prefix, spec) in ids {
            out.insert(prefix.clone());
            if let Some(kind) = spec.get("kind").and_then(toml::Value::as_str) {
                out.insert(kind.to_owned());
            }
            for alias in spec
                .get("aliases_from")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
            {
                out.insert(alias.to_owned());
            }
        }
    }
    if let Some(roadmap) = table.get("roadmap").and_then(toml::Value::as_table) {
        for key in ["prefix", "unit"] {
            if let Some(value) = roadmap.get(key).and_then(toml::Value::as_str) {
                out.insert(value.to_owned());
            }
        }
        for key in ["releases", "priorities"] {
            for value in roadmap
                .get(key)
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
            {
                out.insert(value.to_owned());
            }
        }
    }
}

fn vocabulary() -> BTreeSet<String> {
    let mut words: BTreeSet<String> = THIS_REPOSITORY.iter().map(|w| (*w).to_owned()).collect();
    for name in ["roadmap-a", "roadmap-b"] {
        let text = fs::read_to_string(fixture(name).join("specengine.toml")).expect(name);
        words_of(&text, &mut words);
    }
    let root = fs::read_to_string(repository_root().join("specengine.toml")).expect("root config");
    words_of(&root, &mut words);
    for expected in [
        "RM",
        "WORK",
        "plan-item",
        "work-package",
        "alpha",
        "beta",
        "launch",
        "now",
        "next",
        "later",
        "must",
        "p0",
        "p2",
        "days",
        "mvp",
    ] {
        assert!(words.contains(expected), "{expected} in {words:?}");
    }
    words
}

/// Every string literal of `source` (`"…"`, `b"…"`, raw `r#"…"#`) with
/// its 1-based line, outside comments and character literals; escapes
/// kept as written (no vocabulary word holds one).
fn string_literals(source: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut line = 1;
    let mut at = 0;
    let count_lines = |text: &[char]| text.iter().filter(|c| **c == '\n').count();
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        match c {
            '\n' => {
                line += 1;
                at += 1;
            }
            '/' if next == Some('/') => {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
            '/' if next == Some('*') => {
                let start = at;
                let mut depth = 0;
                while at < chars.len() {
                    if chars[at] == '/' && chars.get(at + 1) == Some(&'*') {
                        depth += 1;
                        at += 2;
                    } else if chars[at] == '*' && chars.get(at + 1) == Some(&'/') {
                        depth -= 1;
                        at += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        at += 1;
                    }
                }
                line += count_lines(&chars[start..at.min(chars.len())]);
            }
            '\'' => {
                // A character literal (`'"'`, `'\''`, `'\u{…}'`) or a lifetime.
                if next == Some('\\') {
                    let mut end = at + 2;
                    while end < chars.len() && chars[end] != '\'' {
                        end += 1;
                    }
                    at = end + 1;
                } else if chars.get(at + 2) == Some(&'\'') {
                    at += 3;
                } else {
                    at += 1;
                }
            }
            'r' if (next == Some('"') || next == Some('#'))
                && !chars[..at]
                    .last()
                    .is_some_and(|p| p.is_alphanumeric() || *p == '_') =>
            {
                let mut hashes = 0;
                let mut cursor = at + 1;
                while chars.get(cursor) == Some(&'#') {
                    hashes += 1;
                    cursor += 1;
                }
                if chars.get(cursor) != Some(&'"') {
                    at += 1;
                    continue;
                }
                let start = cursor + 1;
                let mut end = start;
                loop {
                    if end >= chars.len() {
                        break;
                    }
                    if chars[end] == '"'
                        && (0..hashes).all(|k| chars.get(end + 1 + k) == Some(&'#'))
                    {
                        break;
                    }
                    end += 1;
                }
                let text: String = chars[start..end.min(chars.len())].iter().collect();
                out.push((line, text));
                line += count_lines(&chars[at..end.min(chars.len())]);
                at = end + 1 + hashes;
            }
            '"' => {
                let start = at + 1;
                let mut end = start;
                while end < chars.len() && chars[end] != '"' {
                    if chars[end] == '\\' {
                        end += 1;
                    }
                    end += 1;
                }
                let text: String = chars[start..end.min(chars.len())].iter().collect();
                out.push((line, text));
                line += count_lines(&chars[at..end.min(chars.len())]);
                at = end + 1;
            }
            _ => at += 1,
        }
    }
    out
}

/// The scan itself finds what it must: a plain, a raw and a byte literal,
/// not a comment, not a character literal.
#[test]
fn the_literal_scan_reads_strings_only() {
    let source = "// \"mvp\" in a comment\nlet a = \"mvp\";\nlet b = r#\"days\"#;\nlet c = '\"';\n/* \"v1\" */ let d = b\"must\"; let e = \"x\\\"y\";\nfn f<'a>(x: &'a str) {}\nlet g = \"later\";\n";
    let found = string_literals(source);
    assert_eq!(
        found,
        [
            (2, "mvp".to_owned()),
            (3, "days".to_owned()),
            (5, "must".to_owned()),
            (5, "x\\\"y".to_owned()),
            (7, "later".to_owned()),
        ]
    );
}

#[test]
fn roadmap_sources_name_no_vocabulary() {
    let words = vocabulary();
    let mut offenders = Vec::new();
    for relative in SOURCES {
        let path = repository_root().join(relative);
        let whole = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative}: {error}"));
        let code = whole
            .find("\n#[cfg(test)]\nmod ")
            .map_or(whole.as_str(), |at| &whole[..at]);
        assert!(code.len() > 1000, "{relative}: the code is scanned");
        let literals = string_literals(code);
        assert!(!literals.is_empty(), "{relative}: literals found");
        for (line, literal) in literals {
            if words.contains(&literal) {
                offenders.push(format!("{relative}:{line}: {literal:?}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "project vocabulary hard-coded in the roadmap sources:\n{}",
        offenders.join("\n")
    );
}
