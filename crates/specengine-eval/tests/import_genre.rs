//! docs/features/import-records.md AC-01 (the genre test):
//! the import engine (`crates/specengine-import/src`) and the eval `import`
//! module hold no raw non-ASCII letter, no numero sign (raw or escaped), no
//! Cyrillic escape outside `script.rs`'s look-alike table, and no string
//! literal equal (case-insensitive) to a key, regex, map entry, prefix,
//! separator or path of either fixture config, of the AC-05 generated
//! config, or to a class, prefix or pattern of the fixtures' runs; and every
//! recognizer is non-zero in both fixtures' `expected.json`. The
//! `[documents]` keys and the titled lead-ins, document records and
//! document precedence of docs/features/import-gaps.md (AC-09) are covered
//! the same way.
//!
//! String literals are read with a small lexer that skips comments (the one
//! of `identity_literals.rs`), so a doc comment may cite anything; the
//! character checks cover comments too.
//!
//! docs/features/import-layout.md AC-10 extends the scan to the layout
//! emitter (`crates/specengine-import/src/layout/`, under the walked
//! `src`) and the eval `layout` module, and the forbidden set to both
//! `fixtures/import-layout/*/census.toml` configs with their `[layout]`;
//! each layout rule is non-zero in both layout fixtures' `expected.json`.

mod import_support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use import_support::*;

/// The two invented conventions of docs/features/import-layout.md.
const LAYOUT_FIXTURES: [&str; 2] = ["import-layout/one", "import-layout/two"];

/// Every `.rs` file of the import engine, and the eval `import` module.
fn scanned_sources() -> Vec<PathBuf> {
    let root = repository_root();
    let mut sources = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("readable source directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }
    walk(&root.join("crates/specengine-import/src"), &mut sources);
    sources.push(root.join("crates/specengine-eval/src/import.rs"));
    sources.push(root.join("crates/specengine-eval/src/layout.rs"));
    sources.sort();
    sources
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .unwrap_or(path)
        .display()
        .to_string()
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

/// A string literal as the program sees it: the common escapes resolved.
fn unescaped(literal: &str) -> String {
    let mut out = String::new();
    let mut chars = literal.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') if chars.peek() == Some(&'{') => {
                chars.next();
                let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[test]
fn the_lexer_reads_literals_and_skips_comments() {
    let sample = "// \"spec\" in a comment\n/* \"docs\" /* nested */ */\nconst A: &str = \"src\";\nlet c = '\"'; let d = '\\''; fn f<'a>(x: &'a str) {}\nlet r = r#\"raw \"q\" x\"#; let b = b\"bytes\"; let e = \"esc\\\"aped\";\n";
    let literals: Vec<String> = string_literals(sample)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    assert_eq!(literals, ["src", "raw \"q\" x", "bytes", "esc\\\"aped"]);
    assert_eq!(unescaped("a\\u{2116}b\\\\"), "a\u{2116}b\\");
}

/// Every string naming a convention of the fixtures: config values and map
/// keys of both fixture configs and the generated one, and the classes,
/// prefixes and hyphenless patterns their runs label.
fn forbidden() -> BTreeSet<String> {
    let mut strings = BTreeSet::new();
    for name in FIXTURES {
        strings.extend(convention_strings(
            &fs::read_to_string(fixture_config(name)).expect("fixture config"),
        ));
    }
    strings.extend(convention_strings(&generated_config_text()));
    for name in LAYOUT_FIXTURES {
        strings.extend(convention_strings(
            &fs::read_to_string(fixture_config(name)).expect("layout fixture config"),
        ));
    }
    let scratch = Scratch::new("genre");
    for name in FIXTURES {
        let run = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let labels = run.file("labels.json");
        for group in ["classes", "prefixes", "patterns"] {
            for row in labels[group].as_array().expect("label rows") {
                strings.insert(row["value"].as_str().expect("label value").to_owned());
            }
        }
    }
    strings
}

#[test]
fn the_forbidden_set_holds_every_kind_of_convention_string() {
    let forbidden = forbidden();
    for sample in [
        "^Field$",       // header regex
        "Phase",         // key-map key
        "summary",       // key-map target
        "Done",          // value-map entry
        "AC",            // feature prefix
        "SR",            // legacy prefix
        ":",             // separator
        " -",            // separator
        "spec/map.md",   // reference path
        "log/",          // strip prefix
        "tools",         // code root
        "register",      // class
        "DEC",           // Latin prefix
        "^No\\.[0-9]+$", // local number
    ] {
        assert!(
            forbidden.contains(sample),
            "{sample:?} is in the forbidden set"
        );
    }
    // docs/features/import-gaps.md AC-09: the `[documents]` values and the
    // key-map entries reaching them.
    for sample in [
        "ident",  // documents.id_key (import-one), a key-map target
        "Number", // key-map key reaching documents.id_key
        "docid",  // documents.id_key (import-two)
        "Doc",    // field-table key reaching documents.id_key
        "owner",  // identity key-map entry
        r"^log/(?P<id>[A-Z]{3}-[0-9]{4})\.markdown$", // documents.id_path
    ] {
        assert!(
            forbidden.contains(sample),
            "{sample:?} is in the forbidden set"
        );
    }
    assert!(
        forbidden.contains(&legacy_prefix()),
        "the generated legacy prefix"
    );
    assert!(forbidden.contains(&non_latin_key()), "the generated key");
}

/// The literals of the scanned sources equal (case-insensitive) to one of
/// `forbidden`, and how many literals were read.
fn convention_literals(forbidden: &BTreeSet<String>) -> (Vec<String>, usize) {
    let forbidden: Vec<(String, &String)> = forbidden
        .iter()
        .map(|text| (text.to_lowercase(), text))
        .collect();
    let mut offending = Vec::new();
    let mut total = 0;
    for path in scanned_sources() {
        let source = fs::read_to_string(&path).expect("source readable");
        for (line, literal) in string_literals(&source) {
            total += 1;
            let value = unescaped(&literal).to_lowercase();
            if let Some((_, original)) = forbidden.iter().find(|(lower, _)| *lower == value) {
                offending.push(format!(
                    "{}:{line}: {literal:?} equals the convention string {original:?}",
                    relative(&path)
                ));
            }
        }
    }
    (offending, total)
}

#[test]
fn no_literal_of_the_engine_names_a_fixture_convention() {
    let (offending, total) = convention_literals(&forbidden());
    assert!(total >= 100, "the lexer found only {total} literals");
    assert!(
        offending.is_empty(),
        "convention literals in the engine (ADR-0008):\n{}",
        offending.join("\n")
    );
}

/// Not vacuous: generic literals the engine does hold are found, whatever
/// their case, escapes resolved.
#[test]
fn the_literal_check_finds_literals_the_engine_holds() {
    let probes = BTreeSet::from([
        "KEBAB-CASE".to_owned(),
        "records.json".to_owned(),
        "\r\n".to_owned(),
    ]);
    let (found, _) = convention_literals(&probes);
    for probe in ["KEBAB-CASE", "records.json", "\\r\\n"] {
        assert!(
            found.iter().any(|hit| hit.contains(probe)),
            "{probe:?} not found: {found:?}"
        );
    }
}

/// The code points of the `\u{...}` escapes of a line.
fn unicode_escapes(line: &str) -> Vec<u32> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("\\u{") {
        rest = &rest[at + 3..];
        if let Some(end) = rest.find('}')
            && let Ok(code) = u32::from_str_radix(&rest[..end].replace('_', ""), 16)
        {
            found.push(code);
        }
    }
    found
}

#[test]
fn the_escape_reader_finds_code_points() {
    assert_eq!(
        unicode_escapes("'\\u{0422}' | '\\u{2116}' and \\u{FEFF}"),
        [0x0422, 0x2116, 0xFEFF]
    );
}

#[test]
fn no_raw_non_ascii_letter_and_no_numero_sign() {
    let mut offending = Vec::new();
    let sources = scanned_sources();
    assert!(sources.len() >= 14, "{sources:?}");
    for path in &sources {
        let source = fs::read_to_string(path).expect("source readable");
        for (index, line) in source.lines().enumerate() {
            if let Some(c) = line.chars().find(|c| !c.is_ascii() && c.is_alphabetic()) {
                offending.push(format!(
                    "{}:{}: raw non-ASCII letter U+{:04X}",
                    relative(path),
                    index + 1,
                    u32::from(c)
                ));
            }
            if line.contains('\u{2116}')
                || unicode_escapes(line).contains(&0x2116)
                || line.to_ascii_lowercase().contains("0x2116")
            {
                offending.push(format!("{}:{}: numero sign", relative(path), index + 1));
            }
        }
    }
    assert!(offending.is_empty(), "{}", offending.join("\n"));
}

#[test]
fn cyrillic_escapes_only_in_the_look_alike_table_of_script_rs() {
    let mut offending = Vec::new();
    let mut in_script = 0;
    for path in scanned_sources() {
        let source = fs::read_to_string(&path).expect("source readable");
        let is_script = path.ends_with("crates/specengine-import/src/script.rs");
        for (index, line) in source.lines().enumerate() {
            if unicode_escapes(line)
                .iter()
                .any(|code| (0x0400..=0x052F).contains(code))
            {
                if is_script {
                    in_script += 1;
                } else {
                    offending.push(format!(
                        "{}:{}: {}",
                        relative(&path),
                        index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        in_script >= 10,
        "the scan sees script.rs's table ({in_script} lines)"
    );
    assert!(
        offending.is_empty(),
        "Cyrillic escapes outside script.rs:\n{}",
        offending.join("\n")
    );
}

/// The recognizer counts each fixture must exercise; zero is allowed only
/// for `front_matter.unclosed`, `records.empty_text`, `broken_links.wiki`,
/// `code.roots_missing` and `detail.*`.
const RECOGNIZERS: [&str; 33] = [
    "records/titled",
    "records/per_form/table_row",
    "records/per_form/headerless_row",
    "records/per_form/list_item",
    "records/per_form/section",
    "records/per_form/document",
    "front_matter/yaml",
    "front_matter/field_table",
    "front_matter/non_latin_keys",
    "front_matter/keys/mapped",
    "front_matter/keys/kept",
    "front_matter/keys/unmapped",
    "front_matter/values/mapped",
    "front_matter/values/kept",
    "front_matter/values/unmapped",
    "definitions",
    "references/total",
    "references/unresolved",
    "references/by_document",
    "duplicate_definitions",
    "rows_without_id/local_number",
    "rows_without_id/none",
    "legacy/mapped",
    "legacy/unmapped",
    "legacy/homoglyph_fixes",
    "legacy/hyphenless/definitions",
    "legacy/hyphenless/mentions",
    "id_like/claimed",
    "id_like/unclaimed",
    "id_like/feature_outside",
    "broken_links/file",
    "broken_links/resolved_by_base",
    "code/citations",
];

#[test]
fn every_recognizer_is_non_zero_in_both_fixtures() {
    for name in FIXTURES {
        let expected = read_json(&fixture_dir(name).join("expected.json"));
        let mut zero = Vec::new();
        for path in RECOGNIZERS {
            let mut value = &expected;
            for part in path.split('/') {
                value = &value[part];
            }
            match value.as_u64() {
                Some(0) => zero.push(path),
                Some(_) => {}
                None => panic!("{name}: expected.json has no count at {path}"),
            }
        }
        assert!(zero.is_empty(), "{name}: zero recognizers {zero:?}");
        for map in ["records/per_prefix", "legacy/hyphenless/per_pattern"] {
            let mut value = &expected;
            for part in map.split('/') {
                value = &value[part];
            }
            let counts = value.as_object().expect("label map");
            assert!(
                !counts.is_empty() && counts.values().all(|count| count.as_u64() > Some(0)),
                "{name}: {map} {counts:?}"
            );
        }
    }
}

// ------------------------------------------- docs/features/import-layout.md

/// AC-10: the scan reads the layout emitter and the eval `layout` module.
#[test]
fn the_scan_covers_the_layout_emitter_and_the_eval_layout_module() {
    let scanned: Vec<String> = scanned_sources()
        .iter()
        .map(|path| relative(path))
        .collect();
    for wanted in [
        "crates/specengine-import/src/layout/mod.rs",
        "crates/specengine-import/src/layout/body.rs",
        "crates/specengine-import/src/layout/header.rs",
        "crates/specengine-import/src/layout/records.rs",
        "crates/specengine-import/src/layout/scheme.rs",
        "crates/specengine-import/src/layout/toml_out.rs",
        "crates/specengine-import/src/layout/yaml.rs",
        "crates/specengine-eval/src/layout.rs",
    ] {
        assert!(
            scanned.iter().any(|path| path == wanted),
            "{wanted} is not scanned: {scanned:?}"
        );
    }
    // Not vacuous: a literal only the eval layout module holds is found.
    let probes = BTreeSet::from(["import-layout source debt".to_owned()]);
    let (found, _) = convention_literals(&probes);
    assert!(
        found
            .iter()
            .any(|hit| hit.starts_with("crates/specengine-eval/src/layout.rs:")),
        "{found:?}"
    );
}

/// AC-10: the forbidden set holds the `[layout]` values and the other
/// convention strings of both layout fixtures.
#[test]
fn the_forbidden_set_holds_the_layout_fixture_conventions() {
    let forbidden = forbidden();
    for sample in [
        "book/atoms",                           // layout.records (one)
        "book/flows",                           // layout.features (one)
        "book/flows/**",                        // layout.classes glob (one)
        "ticked",                               // layout.task_box_key (one)
        "2999-12-31",                           // layout.debt_expires (one)
        "register",                             // value_map.class entry (one)
        "Keeper",                               // key-map key (one)
        "ledger",                               // layout.records (two)
        "topics",                               // layout.features (two)
        r"^pages/(?P<slug>[^/]+)/index\.md$",   // layout.slug (two)
        "*/**",                                 // layout.classes glob (two)
        "steward",                              // key-map target (two)
        r"^log/(?P<id>[A-Z]{3}-[0-9]{4})\.md$", // documents.id_path (two)
    ] {
        assert!(
            forbidden.contains(sample),
            "{sample:?} is in the forbidden set"
        );
    }
}

/// The layout counts each layout fixture must exercise.
const LAYOUT_RULES: [&str; 16] = [
    "tree/documents",
    "tree/record_files",
    "tree/feature_documents",
    "tree/moved",
    "tree/reshaped",
    "hashes/matched",
    "titles/matched",
    "fields/matched",
    "prose/documents",
    "extents/total",
    "header/documents",
    "before/duplicates",
    "dangling/links/after",
    "code/moved_cited",
    "code/citations_to_moved",
    "index/files",
];

/// The layout counts one convention exercises and the other shows zero:
/// the task box carried (a `task_box_key`) or dropped (none), and the
/// emitter's reasons and header conflicts.
const LAYOUT_RULES_EITHER: [&str; 6] = [
    "task_box/carried",
    "task_box/dropped",
    "reasons/path_taken",
    "reasons/prefix_unknown",
    "reasons/slug",
    "header/conflicts",
];

fn layout_count(expected: &serde_json::Value, path: &str, name: &str) -> u64 {
    let mut value = expected;
    for part in path.split('/') {
        value = &value[part];
    }
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{name}: expected.json has no count at {path}"))
}

/// AC-10: each layout rule is non-zero in both layout fixtures'
/// `expected.json`; the rest in at least one of them.
#[test]
fn every_layout_rule_is_non_zero_in_both_layout_fixtures() {
    let expected: Vec<(&str, serde_json::Value)> = LAYOUT_FIXTURES
        .iter()
        .map(|name| (*name, read_json(&fixture_dir(name).join("expected.json"))))
        .collect();
    for (name, value) in &expected {
        let zero: Vec<&str> = LAYOUT_RULES
            .iter()
            .copied()
            .filter(|path| layout_count(value, path, name) == 0)
            .collect();
        assert!(zero.is_empty(), "{name}: zero layout rules {zero:?}");
    }
    for path in LAYOUT_RULES_EITHER {
        assert!(
            expected
                .iter()
                .any(|(name, value)| layout_count(value, path, name) > 0),
            "{path} is zero in both layout fixtures"
        );
    }
}
