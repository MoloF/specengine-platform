//! AC-20 of docs/features/spec-index.md (ADR-0008): the store knows no
//! subject domain. spec-a (English game design, `[paths]` defaults) and
//! spec-b (Russian CLI tooling, `roots`) pass the same battery — projection,
//! exact lookup, search by each node's own title, kind filters, equality
//! with a fresh and a relocated index, byte-identical repeat dumps — and no
//! string literal of the store's `src` (nor of the core's `[paths]` reader)
//! is an ID prefix, a kind, a fixture path or a language name.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Corpus, Scratch, assert_equals_fresh, blake3_hex, repository_root};
use specengine_core::IdSchemeToml;
use specengine_model::IdScheme;
use specengine_store::{IndexWriter, SearchQuery, SpecIndex};

fn battery(name: &str) -> BTreeSet<String> {
    let scratch = Scratch::new(&format!("genre-{name}"));
    let corpus = Corpus::copy_of(name, &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    let report = corpus.update(&mut index);
    assert_eq!(report.parsed, report.walked, "{name}: {report:?}");
    assert_eq!(
        index.files().expect("files"),
        corpus.listing().paths,
        "{name}"
    );

    let mut kinds = BTreeSet::new();
    let mut titled = 0;
    for path in index.files().expect("files") {
        let bytes = corpus.bytes(&path);
        let parsed = specengine_core::parse(&path, &bytes, &corpus.scheme);
        let file = index.file(&path).expect("file").expect("stored");
        assert_eq!(file.parsed.as_ref(), Some(&parsed), "{name}: {path}");
        assert_eq!(file.blake3, Some(blake3_hex(&bytes)), "{name}: {path}");
        for (ord, node) in parsed.nodes.iter().enumerate() {
            if let Some(id) = &node.id {
                let hits = index.lookup_id(id).expect("lookup_id");
                assert!(
                    hits.iter().any(|hit| hit.path == path && hit.ord == ord),
                    "{name}: lookup_id({id})"
                );
            }
            if let Some(kind) = &node.kind {
                kinds.insert(kind.clone());
            }
            // Every node is found by its own title, also within its kind.
            let Some(title) = &node.title else { continue };
            if !title
                .split_whitespace()
                .any(|term| term.chars().count() >= 3)
            {
                continue;
            }
            titled += 1;
            let mut query = SearchQuery::new(title.clone());
            query.limit = 200;
            let hits = index.search(&query).expect("search").hits;
            assert!(
                hits.iter().any(|hit| hit.path == path && hit.ord == ord),
                "{name}: {path}#{ord} not found by its title {title:?}"
            );
            if let Some(kind) = &node.kind {
                query.kinds = vec![kind.clone()];
                let hits = index.search(&query).expect("search").hits;
                assert!(
                    hits.iter().any(|hit| hit.path == path && hit.ord == ord),
                    "{name}: {path}#{ord} not found within its kind {kind}"
                );
                assert!(
                    hits.iter()
                        .all(|hit| hit.kind.as_deref() == Some(kind.as_str()))
                );
            }
        }
    }
    assert!(titled > 3, "{name}: titled nodes searched: {titled}");

    // A fresh index, a rebuild and a repeat run dump byte for byte alike.
    assert_equals_fresh(&index, &corpus, &scratch, name);
    let dump = index.dump().expect("dump");
    index
        .rebuild(&corpus.tree(), &corpus.scheme)
        .expect("rebuild");
    assert_eq!(index.dump().expect("dump"), dump, "{name}: rebuild");
    assert_eq!(
        corpus.fresh_dump(&scratch),
        corpus.fresh_dump(&scratch),
        "{name}: repeat"
    );
    // A relocated copy differs only by its root.
    let moved = Corpus::copy_of(name, &scratch, "moved");
    let moved_dump = moved.fresh_dump(&scratch);
    let mask = |dump: &str, root: &Path| dump.replace(root.to_str().unwrap(), "<root>");
    assert_eq!(
        mask(&moved_dump, &moved.root),
        mask(&dump, &corpus.root),
        "{name}: relocated"
    );
    kinds
}

#[test]
fn spec_a_and_spec_b_pass_the_same_battery() {
    let a = battery("spec-a");
    let b = battery("spec-b");
    assert_ne!(a, b, "the two corpora have kinds of their own");
    assert!(
        a.contains("mechanic") && b.contains("command"),
        "{a:?} / {b:?}"
    );
}

/// Every string literal of a Rust source (plain, raw, byte), comments and
/// char literals skipped.
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
            // A char literal `'x'` / `'\n'` / `'\u{…}'`, else a lifetime.
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
            loop {
                if j >= chars.len() {
                    break;
                }
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

fn scheme_of(fixture: &str) -> IdScheme {
    let path = repository_root()
        .join("fixtures")
        .join(fixture)
        .join("specengine.toml");
    IdScheme::from_toml(&fs::read_to_string(&path).expect("scheme")).expect("valid scheme")
}

/// What no literal may be or contain: the prefixes (with their `-`) and
/// the kinds of both fixture schemes, the fixture names, language names.
fn domain_offence(
    literal: &str,
    prefixes: &BTreeSet<String>,
    kinds: &BTreeSet<String>,
) -> Option<String> {
    let lower = literal.to_lowercase();
    for prefix in prefixes {
        if literal == prefix {
            return Some(format!("the prefix {prefix}"));
        }
        let dashed = format!("{prefix}-");
        let found = literal.match_indices(&dashed).any(|(at, _)| {
            !literal[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
        });
        if found {
            return Some(format!("the prefix {dashed}"));
        }
    }
    if kinds.contains(&lower) {
        return Some(format!("the kind {lower}"));
    }
    for fixture in ["spec-a", "spec-b", "fixtures/", "lantern", "zerkalo"] {
        if lower.contains(fixture) {
            return Some(format!("the fixture name {fixture}"));
        }
    }
    for language in ["ru", "en", "russian", "english", "cyrillic", "latin"] {
        if lower == language {
            return Some(format!("the language {language}"));
        }
    }
    None
}

fn domain_literals(files: &[PathBuf]) -> Vec<String> {
    let mut prefixes = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for fixture in ["spec-a", "spec-b"] {
        for spec in scheme_of(fixture).prefixes() {
            prefixes.insert(spec.prefix.clone());
            kinds.insert(spec.kind.to_lowercase());
        }
    }
    assert!(prefixes.contains("RULE") && prefixes.contains("REQ"));
    let mut offenders = Vec::new();
    for file in files {
        let text = fs::read_to_string(file).expect("UTF-8 source");
        for (line, literal) in string_literals(&text) {
            if let Some(offence) = domain_offence(&literal, &prefixes, &kinds) {
                offenders.push(format!(
                    "{}:{line}: {literal:?} ({offence})",
                    file.display()
                ));
            }
        }
    }
    offenders
}

fn store_sources() -> Vec<PathBuf> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = fs::read_dir(&src)
        .expect("src")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.push(repository_root().join("crates/specengine-core/src/paths_toml.rs"));
    files.sort();
    assert!(files.len() > 5);
    files
}

#[test]
fn no_prefix_kind_fixture_or_language_literal_in_src() {
    let offenders = domain_literals(&store_sources());
    assert!(
        offenders.is_empty(),
        "domain literals in src:\n{}",
        offenders.join("\n")
    );
}

/// The scan finds a special case like the named mutation, and reads
/// literals correctly.
#[test]
fn the_literal_scan_sees_a_special_cased_prefix() {
    let literals: Vec<String> = string_literals(
        "fn f<'a>(x: &'a str) -> bool {\n    let q = '\"'; // \"RULE-\" in a comment\n    x.starts_with(\"RULE-\") || x == r#\"say \"hi\"\"# || x == \"a\\\"b\"\n}\n",
    )
    .into_iter()
    .map(|(_, literal)| literal)
    .collect();
    assert_eq!(literals, ["RULE-", "say \"hi\"", "a\\\"b"]);

    let scratch = Scratch::new("genre-scan");
    let file = scratch.join("special.rs");
    fs::write(
        &file,
        "fn f(id: &str) -> bool { id.starts_with(\"RULE-\") || id == \"requirement\" }\n\
         const SQL: &str = \"SELECT x.term FROM t\";\n",
    )
    .unwrap();
    let found = domain_literals(&[file]);
    assert_eq!(found.len(), 2, "{found:?}");
}
