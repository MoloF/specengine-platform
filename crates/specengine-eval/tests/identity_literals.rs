//! docs/features/layer-a-identity.md AC-15 (`docs/canon/code-identity.md`,
//! opening paragraph: unit sources hold no directory or package literal
//! beyond Cargo's; no pilot layout special-cased, ADR-0008): every word-like
//! string literal of `specengine-code`'s `qpath.rs`, the harness's
//! target-table code (`ast_hash/targets.rs`) and the qpath step of
//! `ast_hash/mod.rs` (from its `// 5. qpath` comment to `// 6.`, both
//! required to exist) is a Cargo layout name, a Cargo target kind,
//! `Cargo.toml`, an output category, a piece of the one `cargo metadata`
//! call or a key of its answer. Messages and format strings (whitespace or
//! `{`) are not names.
//!
//! The literals are read with a small lexer that skips comments, so a doc
//! comment may name anything.

use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// The 1-based line of the only line of `source` holding `marker`, trimmed
/// at its start; panics when there is none or more than one.
fn marker_line(source: &str, marker: &str) -> usize {
    let hits: Vec<usize> = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(marker))
        .map(|(at, _)| at + 1)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{STEP_SOURCE}: expected exactly one line starting with {marker:?}, found {hits:?}"
    );
    hits[0]
}

/// The qpath step of `ast_hash/mod.rs` and the line it starts on.
fn qpath_step(root: &Path) -> (String, usize) {
    let source = std::fs::read_to_string(root.join(STEP_SOURCE)).expect("source readable");
    let start = marker_line(&source, STEP_START);
    let end = marker_line(&source, STEP_END);
    assert!(
        start < end,
        "{STEP_START:?} (line {start}) must come before {STEP_END:?} (line {end})"
    );
    let text: Vec<&str> = source.lines().skip(start - 1).take(end - start).collect();
    (text.join("\n"), start)
}

/// Every scanned text: `(name, text, first line)`.
fn scanned(root: &Path) -> Vec<(String, String, usize)> {
    let mut out: Vec<(String, String, usize)> = SOURCES
        .iter()
        .map(|relative| {
            let text = std::fs::read_to_string(root.join(relative)).expect("source readable");
            ((*relative).to_owned(), text, 1)
        })
        .collect();
    let (step, start) = qpath_step(root);
    out.push((format!("{STEP_SOURCE} (qpath step)"), step, start));
    out
}

/// The sources under the rule, scanned whole.
const SOURCES: [&str; 2] = [
    "crates/specengine-code/src/qpath.rs",
    "crates/specengine-eval/src/ast_hash/targets.rs",
];

/// The qpath step of the harness: from the line holding `STEP_START` up to
/// the line holding `STEP_END`.
const STEP_SOURCE: &str = "crates/specengine-eval/src/ast_hash/mod.rs";
const STEP_START: &str = "// 5. qpath";
const STEP_END: &str = "// 6.";

/// Cargo's own layout names (the "Rules" list), the `.rs` suffix and the
/// manifest name.
const CARGO_LAYOUT: [&str; 11] = [
    "src",
    "bin",
    "examples",
    "tests",
    "benches",
    "build.rs",
    "main.rs",
    "lib.rs",
    "mod.rs",
    ".rs",
    "Cargo.toml",
];

/// Cargo's target kinds and the build script's target name.
const CARGO_KINDS: [&str; 7] = [
    "lib",
    "bin",
    "example",
    "test",
    "bench",
    "custom-build",
    "build-script-build",
];

/// Output categories: target sources, ambiguity reasons, unit kinds.
const CATEGORIES: [&str; 7] = [
    "metadata",
    "layout",
    "path_attribute",
    "unrooted",
    "duplicate",
    "primary",
    "shared",
];

/// The `cargo metadata` call, its override variable, the prefix of its
/// error lines and the keys of its answer.
const CARGO_CALL: [&str; 19] = [
    "cargo",
    "SPECENGINE_CARGO",
    "metadata",
    "--format-version",
    "1",
    "--no-deps",
    "--offline",
    "--color",
    "never",
    "--manifest-path",
    "error",
    "workspace_root",
    "packages",
    "manifest_path",
    "targets",
    "kind",
    "name",
    "src_path",
    "impl_item",
];

/// Separators and the root label.
const SEPARATORS: [&str; 4] = ["", "/", "_", "."];

fn allowed(literal: &str) -> bool {
    CARGO_LAYOUT
        .iter()
        .chain(&CARGO_KINDS)
        .chain(&CATEGORIES)
        .chain(&CARGO_CALL)
        .chain(&SEPARATORS)
        .any(|word| *word == literal)
}

/// A literal that could name a directory, a file or a package: no
/// whitespace, no format placeholder.
fn name_like(literal: &str) -> bool {
    !literal.chars().any(char::is_whitespace) && !literal.contains('{')
}

/// Every string literal of `source` (contents, raw, byte and raw byte
/// strings), with its 1-based line; comments, char literals and lifetimes
/// skipped.
fn string_literals(source: &str) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    let line_at = |at: usize| source[..at].matches('\n').count() + 1;
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    while i < bytes.len() {
        let b = bytes[i];
        let prev_ident = i > 0 && ident(bytes[i - 1]);
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mut depth = 0;
            while i < bytes.len() {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if !prev_ident && (b == b'r' || (b == b'b' && bytes.get(i + 1) == Some(&b'r'))) {
            // A raw string `r#"…"#` / `br"…"`, else an identifier.
            let mut j = i + if b == b'b' { 2 } else { 1 };
            let mut hashes = 0;
            while bytes.get(j) == Some(&b'#') {
                hashes += 1;
                j += 1;
            }
            if bytes.get(j) == Some(&b'"') {
                let start = j + 1;
                let close = format!("\"{}", "#".repeat(hashes));
                let end = start + source[start..].find(&close).expect("raw string closes");
                found.push((line_at(i), source[start..end].to_owned()));
                i = end + close.len();
            } else {
                i += 1;
                while i < bytes.len() && ident(bytes[i]) {
                    i += 1;
                }
            }
        } else if b == b'"' || (!prev_ident && b == b'b' && bytes.get(i + 1) == Some(&b'"')) {
            let start = if b == b'b' { i + 2 } else { i + 1 };
            let mut j = start;
            while bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            found.push((line_at(i), source[start..j].to_owned()));
            i = j + 1;
        } else if b == b'\'' {
            // `'\…'` or `'x'` is a char literal; else a lifetime.
            if bytes.get(i + 1) == Some(&b'\\') {
                let mut j = i + 3;
                while bytes[j] != b'\'' {
                    j += 1;
                }
                i = j + 1;
            } else if let Some(c) = source[i + 1..].chars().next()
                && source[i + 1 + c.len_utf8()..].starts_with('\'')
            {
                i += 1 + c.len_utf8() + 1;
            } else {
                i += 1;
            }
        } else if ident(b) {
            while i < bytes.len() && ident(bytes[i]) {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    found
}

#[test]
fn the_lexer_reads_literals_and_skips_comments() {
    let sample = "// \"xtask\" in a comment\n/* \"vendor\" /* nested */ */\nconst A: &str = \"src\";\nlet c = '\"'; let d = '\\''; fn f<'a>(x: &'a str) {}\nlet r = r#\"raw \"q\" x\"#; let b = b\"bytes\"; let e = \"esc\\\"aped\";\nlet for_ = r; // r#\"not\"#\n";
    let literals: Vec<String> = string_literals(sample)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    assert_eq!(
        literals,
        ["src", "raw \"q\" x", "bytes", "esc\\\"aped"],
        "{literals:?}"
    );
}

#[test]
fn unit_sources_name_only_cargo_layout_kinds_and_categories() {
    let root = repository_root();
    let mut offending = Vec::new();
    let texts = scanned(&root);
    assert_eq!(texts.len(), 3, "two whole sources and the qpath step");
    for (name, source, first_line) in &texts {
        let literals = string_literals(source);
        assert!(
            literals.len() >= 4,
            "{name}: the lexer found only {} literals",
            literals.len()
        );
        for (line, literal) in literals {
            if name_like(&literal) && !allowed(&literal) {
                offending.push(format!("{name}:{}: {literal:?}", line + first_line - 1));
            }
        }
    }
    assert!(
        offending.is_empty(),
        "directory or package literals beyond Cargo's:\n{}",
        offending.join("\n")
    );
}

#[test]
fn the_scan_sees_the_cargo_names_it_allows() {
    // Not vacuous: the layout names really are literals of `qpath.rs`, the
    // call's pieces of `targets.rs`.
    let root = repository_root();
    let words = |relative: &str| -> Vec<String> {
        let source = std::fs::read_to_string(root.join(relative)).unwrap();
        string_literals(&source)
            .into_iter()
            .map(|(_, s)| s)
            .collect()
    };
    let qpath = words(SOURCES[0]);
    for name in [
        "src", "tests", "examples", "benches", "build.rs", "main.rs", "mod.rs",
    ] {
        assert!(qpath.iter().any(|w| w == name), "qpath.rs lacks {name:?}");
    }
    let targets = words(SOURCES[1]);
    for name in [
        "--no-deps",
        "--offline",
        "--color",
        "never",
        "Cargo.toml",
        "SPECENGINE_CARGO",
    ] {
        assert!(
            targets.iter().any(|w| w == name),
            "targets.rs lacks {name:?}"
        );
    }
}

#[test]
fn the_qpath_step_of_ast_hash_is_bounded_by_its_step_comments() {
    // Both step comments exist once, in order, and the step between them
    // holds the qpath code (its literals), so the scan above is not empty.
    let (step, start) = qpath_step(&repository_root());
    assert!(start > 1);
    assert!(step.trim_start().starts_with(STEP_START), "{step}");
    let words: Vec<String> = string_literals(&step).into_iter().map(|(_, s)| s).collect();
    for name in ["Cargo.toml", "primary", "shared", "unrooted"] {
        assert!(
            words.iter().any(|w| w == name),
            "the qpath step lacks {name:?}: {words:?}"
        );
    }
    assert!(
        step.contains("file_role") && step.contains("targets::tables"),
        "the qpath step holds the role and table code"
    );
}

#[test]
fn a_foreign_literal_in_any_scanned_text_is_reported() {
    // The scan reports what it should: an `"xtask"` literal planted in each
    // scanned text is found and named with its source.
    for (name, source, _) in scanned(&repository_root()) {
        let planted = format!("{source}\nconst PLANTED: &str = \"xtask\";\n");
        let found: Vec<String> = string_literals(&planted)
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| name_like(s) && !allowed(s))
            .collect();
        assert_eq!(found, ["xtask"], "{name}");
    }
}
