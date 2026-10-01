//! AC-16 of docs/features/spec-cli-changed.md (ADR-0008): the new store
//! code of `spec check --changed` — `check_changed_with_notes` and its
//! helpers (`src/check.rs`), `Staged::head_only` (`src/git.rs`) and
//! `Head::walk_by`, `HeadWalk::findings_by_bytes`, `run_base`
//! (`src/base.rs`) — names no path or file of a project: no string
//! literal of those modules holds `"docs/"` or this repository's file
//! names. `HEAD`'s files are found by the checked config's paths, never by
//! a name. (`genre.rs` checks these names in `src/base.rs` only.)

use std::fs;
use std::path::PathBuf;

/// The names no literal of the scanned modules may hold.
const PROJECT_NAMES: [&str; 8] = [
    "docs/",
    "CLAUDE.md",
    "README.md",
    "index.md",
    "fixtures",
    "xtask",
    "specengine-cli",
    ".githooks",
];

/// Every string literal of a Rust source (plain, raw, byte), comments and
/// char literals skipped, with its first line.
fn string_literals(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
        } else if c == '\'' {
            if chars.get(i + 1) == Some(&'\\') {
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if chars.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if c == 'r'
            && (chars.get(i + 1) == Some(&'"') || chars.get(i + 1) == Some(&'#'))
            && !chars
                .get(i.wrapping_sub(1))
                .is_some_and(|p| p.is_alphanumeric() || *p == '_')
        {
            let mut hashes = 0;
            let mut j = i + 1;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            let start_line = line;
            j += 1;
            let mut literal = String::new();
            while j < chars.len() {
                if chars[j] == '"' && (0..hashes).all(|k| chars.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                if chars[j] == '\n' {
                    line += 1;
                }
                literal.push(chars[j]);
                j += 1;
            }
            out.push((start_line, literal));
            i = j;
        } else if c == '"' {
            let start_line = line;
            let mut j = i + 1;
            let mut literal = String::new();
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' {
                    literal.push(chars[j]);
                    j += 1;
                }
                if j < chars.len() {
                    if chars[j] == '\n' {
                        line += 1;
                    }
                    literal.push(chars[j]);
                }
                j += 1;
            }
            out.push((start_line, literal));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// `name:line: literal` for every literal of `text` holding a project name.
fn project_names(name: &str, text: &str) -> Vec<String> {
    string_literals(text)
        .into_iter()
        .filter(|(_, literal)| PROJECT_NAMES.iter().any(|word| literal.contains(word)))
        .map(|(line, literal)| format!("{name}:{line}: {literal:?}"))
        .collect()
}

#[test]
fn the_changed_check_s_modules_name_no_project_path() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let needles = [
        ("check.rs", "pub fn check_changed_with_notes("),
        ("git.rs", "pub(crate) fn head_only("),
        ("base.rs", "pub(crate) fn findings_by_bytes("),
    ];
    let mut found = Vec::new();
    for (module, needle) in needles {
        let text = fs::read_to_string(src.join(module)).expect("a store source");
        assert!(
            text.contains(needle),
            "{module} no longer holds {needle}: the scan's target moved"
        );
        found.extend(project_names(module, &text));
    }
    assert!(found.is_empty(), "project names in the store: {found:?}");
}

/// The scan sees a `"docs/"` literal (the named mutation) and skips one in
/// a comment.
#[test]
fn the_scan_sees_a_docs_literal() {
    let code = "// see docs/features/x.md\nfn f() { let probe = \"docs/\"; }\n";
    assert_eq!(project_names("x.rs", code), ["x.rs:2: \"docs/\""]);
    assert!(project_names("y.rs", "/// docs/ in a doc comment\nfn g() {}\n").is_empty());
}
