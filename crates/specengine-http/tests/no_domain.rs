//! AC-08 of docs/features/ui-live.md: no domain in the daemon. No string
//! literal in `crates/specengine-http/src` holds, as a word, a verdict
//! (`clean`, `observed`, `blocked`, `cannot-check`), a `LINK_TYPES` name or
//! a kind: the check exception is keyed on the outcome being a check,
//! never on what it found (ADR-0008). M: a branch on `"blocked"`.
//!
//! The lists, read when the test runs, not copied: `LINK_TYPES` from its
//! definition in `crates/specengine-model/src/link.rs`; the kinds are every
//! `kind = "…"` of every `specengine.toml` the repository holds (its own
//! and each under `fixtures/`), since the core has no kind of its own:
//! a kind is a project's word, and these are the projects the tests know.
//! A word is a run of letters, digits, `_` and `-`, compared without case;
//! a literal's escapes are decoded first.
//!
//! The one exemption, narrow and explicit: `decision` as a whole segment
//! of a literal that is a route path (`/` first, no whitespace), the
//! decision route's own name (`/api/projects/{p}/proposals/{id}/decision`);
//! a verdict or a link type is never exempt, nor `decision` in prose.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::repository_root;

/// A string literal: its line and its text, escapes decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Literal {
    line: usize,
    text: String,
}

/// The escape at `chars[at]` (just after the `\`): what it stands for and
/// the index after it.
fn unescape(chars: &[char], at: usize) -> (String, usize) {
    match chars.get(at) {
        Some('n') | Some('t') | Some('r') | Some('0') => (" ".to_owned(), at + 1),
        Some('u') if chars.get(at + 1) == Some(&'{') => {
            let end = (at + 2..chars.len())
                .find(|&i| chars[i] == '}')
                .unwrap_or(chars.len() - 1);
            let hex: String = chars[at + 2..end].iter().collect();
            let decoded = u32::from_str_radix(&hex, 16)
                .ok()
                .and_then(char::from_u32)
                .map_or_else(|| " ".to_owned(), String::from);
            (decoded, end + 1)
        }
        Some('x') => {
            let hex: String = chars[at + 1..(at + 3).min(chars.len())].iter().collect();
            let decoded = u8::from_str_radix(&hex, 16)
                .ok()
                .map_or_else(|| " ".to_owned(), |byte| char::from(byte).to_string());
            (decoded, at + 3)
        }
        // A line continuation: the line end and the next line's leading
        // whitespace are dropped.
        Some('\n') => {
            let mut next = at + 1;
            while chars.get(next).is_some_and(|c| c.is_whitespace()) {
                next += 1;
            }
            (String::new(), next)
        }
        Some(&other) => (other.to_string(), at + 1),
        None => (String::new(), at),
    }
}

/// Every string literal of `text` (plain, raw, byte), comments and char
/// literals skipped.
fn literals(text: &str) -> Vec<Literal> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut line = 1;
    let mut at = 0;
    let count_lines = |from: usize, to: usize, chars: &[char]| {
        chars[from..to.min(chars.len())]
            .iter()
            .filter(|&&c| c == '\n')
            .count()
    };
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '\n' {
            line += 1;
            at += 1;
            continue;
        }
        if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            let start = at;
            at += 2;
            while at + 1 < chars.len() && !(chars[at] == '*' && chars[at + 1] == '/') {
                at += 1;
            }
            at += 2;
            line += count_lines(start, at, &chars);
            continue;
        }
        // An identifier: `br"…"`'s `b` starts one, `r#"…"#` too.
        let previous_is_ident = at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '_');
        let mut probe = at;
        if !previous_is_ident && c == 'b' {
            probe += 1;
        }
        if !previous_is_ident && chars.get(probe) == Some(&'r') {
            let mut hashes = 0;
            let mut quote = probe + 1;
            while chars.get(quote) == Some(&'#') {
                hashes += 1;
                quote += 1;
            }
            if chars.get(quote) == Some(&'"') {
                let start = quote + 1;
                let mut end = start;
                while end < chars.len()
                    && !(chars[end] == '"'
                        && (0..hashes).all(|offset| chars.get(end + 1 + offset) == Some(&'#')))
                {
                    end += 1;
                }
                found.push(Literal {
                    line,
                    text: chars[start..end.min(chars.len())].iter().collect(),
                });
                line += count_lines(at, end, &chars);
                at = end + 1 + hashes;
                continue;
            }
        }
        if !previous_is_ident && chars.get(probe) == Some(&'"') {
            let start_line = line;
            let mut index = probe + 1;
            let mut decoded = String::new();
            while index < chars.len() && chars[index] != '"' {
                if chars[index] == '\\' {
                    if chars.get(index + 1) == Some(&'\n') {
                        line += 1;
                    }
                    let (piece, after) = unescape(&chars, index + 1);
                    line += count_lines(index + 2, after, &chars);
                    decoded.push_str(&piece);
                    index = after;
                    continue;
                }
                if chars[index] == '\n' {
                    line += 1;
                }
                decoded.push(chars[index]);
                index += 1;
            }
            found.push(Literal {
                line: start_line,
                text: decoded,
            });
            at = index + 1;
            continue;
        }
        // A char literal ('x', '\n', '\u{…}', '"'), not a lifetime.
        if c == '\'' {
            if next == Some('\\') {
                let mut end = at + 2;
                while end < chars.len() && chars[end] != '\'' {
                    end += 1;
                }
                at = end + 1;
                continue;
            }
            if chars.get(at + 2) == Some(&'\'') {
                at += 3;
                continue;
            }
        }
        at += 1;
    }
    found
}

/// The words of `text`: runs of letters, digits, `_` and `-`, lowercased,
/// outer `-` trimmed.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .map(|word| word.trim_matches('-').to_lowercase())
        .filter(|word| !word.is_empty())
        .collect()
}

/// `LINK_TYPES` as `crates/specengine-model/src/link.rs` defines it.
fn link_types() -> Vec<String> {
    let path = repository_root().join("crates/specengine-model/src/link.rs");
    let text = fs::read_to_string(&path).expect("link.rs");
    let start = text
        .find("pub const LINK_TYPES")
        .expect("`pub const LINK_TYPES` in link.rs");
    let body = &text[start..];
    let open = body.find("= [").expect("its array") + 3;
    let close = body[open..].find("];").expect("its end") + open;
    let names: Vec<String> = body[open..close]
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    let declared: usize = body["pub const LINK_TYPES: [&str; ".len()..]
        .split(']')
        .next()
        .and_then(|count| count.trim().parse().ok())
        .expect("the array's declared length");
    assert_eq!(names.len(), declared, "{names:?}");
    names
}

/// Every `specengine.toml` under `dir`, `target*` and `node_modules` and
/// hidden directories skipped.
fn configs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !(name.starts_with('.') || name.starts_with("target") || name == "node_modules") {
                configs(&path, out);
            }
        } else if name == "specengine.toml" {
            out.push(path);
        }
    }
}

/// The `kind = "…"` values of the repository's config and every config
/// under `fixtures/`.
fn kinds() -> (BTreeSet<String>, Vec<PathBuf>) {
    let root = repository_root();
    let mut files = vec![root.join("specengine.toml")];
    configs(&root.join("fixtures"), &mut files);
    files.sort();
    let mut kinds = BTreeSet::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("a config");
        for line in text.lines() {
            let mut rest = line;
            while let Some(at) = rest.find("kind") {
                let after = &rest[at + 4..];
                let boundary = at == 0
                    || !rest[..at]
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_');
                let value = after
                    .trim_start()
                    .strip_prefix('=')
                    .map(str::trim_start)
                    .and_then(|v| v.strip_prefix('"'))
                    .and_then(|v| v.split_once('"'))
                    .map(|(kind, _)| kind);
                if boundary && let Some(kind) = value {
                    kinds.insert(kind.to_lowercase());
                }
                rest = after;
            }
        }
    }
    (kinds, files)
}

const VERDICTS: [&str; 4] = ["clean", "observed", "blocked", "cannot-check"];

/// A literal that is a route path: `/` first, no whitespace.
fn route_path(text: &str) -> bool {
    text.starts_with('/') && !text.chars().any(char::is_whitespace)
}

/// The words of `literal` that are a verdict, a link type or a kind
/// (with the reason), and whether the exemption was used.
fn domain_words(
    literal: &str,
    links: &[String],
    kinds: &BTreeSet<String>,
) -> (Vec<(String, &'static str)>, bool) {
    let mut hits = Vec::new();
    let mut exempted = false;
    for word in words(literal) {
        if VERDICTS.contains(&word.as_str()) {
            hits.push((word, "a verdict"));
        } else if links.contains(&word) {
            hits.push((word, "a LINK_TYPES name"));
        } else if kinds.contains(&word) {
            let segment = route_path(literal) && literal.split('/').any(|part| part == word);
            if word == "decision" && segment {
                exempted = true;
            } else {
                hits.push((word, "a kind"));
            }
        }
    }
    (hits, exempted)
}

fn sources() -> Vec<(PathBuf, String)> {
    let src = repository_root().join("crates/specengine-http/src");
    let mut files = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = fs::read_to_string(&path).expect("source");
                files.push((path, text));
            }
        }
    }
    files.sort();
    assert!(files.len() >= 2, "the daemon's sources: {files:?}");
    files
}

#[test]
fn ac08_no_literal_in_the_daemon_names_a_verdict_a_link_type_or_a_kind() {
    let links = link_types();
    assert!(
        links.len() == 12 && links.contains(&"depends_on".to_owned()),
        "{links:?}"
    );
    let (kinds, configs) = kinds();
    assert!(configs.len() >= 3, "{configs:?}");
    for kind in ["mechanic", "rule", "decision", "edge-case", "requirement"] {
        assert!(kinds.contains(kind), "spec-a's `{kind}` is read: {kinds:?}");
    }
    let mut found = Vec::new();
    let mut exempt = Vec::new();
    let mut scanned = 0;
    for (path, text) in sources() {
        let relative = path
            .strip_prefix(repository_root())
            .unwrap_or(&path)
            .display()
            .to_string();
        for literal in literals(&text) {
            scanned += 1;
            let (hits, exempted) = domain_words(&literal.text, &links, &kinds);
            for (word, why) in hits {
                found.push(format!(
                    "{relative}:{}: {:?} holds `{word}`, {why}",
                    literal.line, literal.text
                ));
            }
            if exempted {
                exempt.push(format!("{relative}:{}: {:?}", literal.line, literal.text));
            }
        }
    }
    eprintln!(
        "{scanned} literals scanned; {} kinds from {} configs: {kinds:?}; exempt route paths: \
         {exempt:#?}",
        kinds.len(),
        configs.len()
    );
    assert!(scanned > 50, "the scan reads the literals: {scanned}");
    assert!(
        found.is_empty(),
        "the daemon's literals name the domain: {found:#?}"
    );
}

#[test]
fn ac08_the_scan_sees_a_literal_in_every_form_and_only_literals() {
    let links = vec!["depends_on".to_owned(), "answers".to_owned()];
    let kinds: BTreeSet<String> = ["decision", "rule", "edge-case"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let probe = r####"
// A comment saying blocked, depends_on, rule is no literal.
/* nor "blocked" in a block comment */
fn f(verdict: &str) -> u16 {
    let quote = '"';
    let _x: &'static str = "plain";
    if verdict == "blocked" { return 1; }
    if verdict.contains(r#"Cannot-Check"#) { return 2; }
    let _ = b"depends_on";
    let _ = "a line\nanswers";
    let _ = "an \u{6f}bserved one";
    let _ = "cleaned, clean-up: no verdict";
    let _ = "the rule-set, the rules";
    let _ = "/api/projects/{p}/proposals/{id}/decision";
    let _ = "decisions are made on a terminal";
    let _ = "a decision";
    let _ = "/api/projects/{p}/rule";
    let _ = "/api/blocked";
    let _ = "edge-case";
    let _ = 'x';
    0
}
"####;
    let mut hits = Vec::new();
    let mut exempt = Vec::new();
    for literal in literals(probe) {
        let (found, exempted) = domain_words(&literal.text, &links, &kinds);
        for (word, _) in found {
            hits.push((literal.line, word));
        }
        if exempted {
            exempt.push(literal.text.clone());
        }
    }
    assert_eq!(
        hits,
        [
            (7, "blocked".to_owned()),
            (8, "cannot-check".to_owned()),
            (9, "depends_on".to_owned()),
            (10, "answers".to_owned()),
            (11, "observed".to_owned()),
            (16, "decision".to_owned()),
            (17, "rule".to_owned()),
            (18, "blocked".to_owned()),
            (19, "edge-case".to_owned()),
        ]
    );
    assert_eq!(exempt, ["/api/projects/{p}/proposals/{id}/decision"]);
}
