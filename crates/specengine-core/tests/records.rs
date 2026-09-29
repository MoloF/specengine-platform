//! AC-13 of docs/features/spec-parser.md: a record takes its kind from the
//! `[ids]` prefix of its ID; a record declaring another `kind` keeps the
//! declared one with a `kind-mismatch` warning; the body span is exactly the
//! bytes after the front-matter.

mod common;

use std::collections::BTreeMap;

use specengine_model::{DiagnosticCode, IdScheme, ParsedFile, Severity};

use common::{corpus_scheme, fixture, md_files, text_of, variants};

fn spec_a() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

fn records() -> Vec<(String, Vec<u8>, ParsedFile)> {
    md_files(&fixture("spec-a"))
        .into_iter()
        .filter(|(p, _)| p.starts_with("docs/records/"))
        .map(|(p, b)| {
            let parsed = specengine_core::parse(&p, &b, &spec_a());
            (p, b, parsed)
        })
        .collect()
}

fn declares_kind(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes)
        .lines()
        .skip(1)
        .take_while(|l| *l != "---")
        .any(|l| l.starts_with("kind:"))
}

#[test]
fn one_record_per_prefix_takes_its_kind_from_ids() {
    let scheme = spec_a();
    let mut by_prefix: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, bytes, parsed) in records() {
        let document = parsed.document().unwrap();
        let id = document
            .id
            .clone()
            .unwrap_or_else(|| panic!("{path}: record ID"));
        // The record's directory is its prefix (the target layout).
        let prefix = path.split('/').nth(2).unwrap().to_owned();
        assert!(id.starts_with(&format!("{prefix}-")), "{path}: {id}");
        if declares_kind(&bytes) {
            continue;
        }
        assert_eq!(
            document.kind.as_deref(),
            scheme.kind_of_id(&id),
            "{path}: kind from [ids]"
        );
        by_prefix.entry(prefix).or_default().push(path.clone());
    }
    let record_prefixes = ["A", "AC", "DEC", "Q", "R", "TERM"];
    assert_eq!(
        by_prefix.keys().map(String::as_str).collect::<Vec<_>>(),
        record_prefixes,
        "an undeclared-kind record for every record prefix"
    );
    let kinds: Vec<&str> = record_prefixes
        .iter()
        .map(|p| scheme.prefix(p).unwrap().kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        [
            "assumption",
            "criterion",
            "decision",
            "question",
            "requirement",
            "term"
        ]
    );
}

#[test]
fn a_declared_kind_that_differs_is_kept_with_a_kind_mismatch() {
    let (path, _, parsed) = records()
        .into_iter()
        .find(|(p, _, _)| p.ends_with("A-102.md"))
        .expect("A-102.md");
    let document = parsed.document().unwrap();
    assert_eq!(document.id.as_deref(), Some("A-102"));
    assert_eq!(
        document.kind.as_deref(),
        Some("requirement"),
        "{path}: declared kept"
    );
    let mismatches: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::KindMismatch)
        .collect();
    assert_eq!(mismatches.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(mismatches[0].line, 3, "the kind: line");
    assert_eq!(mismatches[0].severity, Severity::Warning);
    assert_eq!(parsed.diagnostics.len(), 1);
}

#[test]
fn a_declared_kind_equal_to_the_prefix_kind_is_clean() {
    let (_, _, parsed) = records()
        .into_iter()
        .find(|(p, _, _)| p.ends_with("Q-031.md"))
        .expect("Q-031.md");
    assert_eq!(parsed.document().unwrap().kind.as_deref(), Some("question"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn body_span_is_the_bytes_after_the_front_matter() {
    for (path, lf, _) in records() {
        for (variant, bytes) in variants(&lf) {
            let parsed = specengine_core::parse(&path, &bytes, &spec_a());
            let front = parsed.front_matter.expect("records have front-matter");
            assert_eq!(parsed.body.start, front.end, "{path} [{variant}]");
            assert_eq!(parsed.body.end, bytes.len(), "{path} [{variant}]");
            let head = text_of(&bytes, front);
            let closing = if variant.contains("crlf") {
                "\r\n---\r\n"
            } else {
                "\n---\n"
            };
            assert!(head.ends_with(closing), "{path} [{variant}]: {head:?}");
            // The body text is the file minus BOM and front-matter.
            let skip = front.end;
            assert_eq!(text_of(&bytes, parsed.body).as_bytes(), &bytes[skip..]);
        }
    }
}

#[test]
fn a_document_without_id_takes_no_kind_and_a_declared_kind_alone_is_kept() {
    let scheme = spec_a();
    let parsed = specengine_core::parse("x.md", b"---\nkind: note\n---\n", &scheme);
    assert_eq!(parsed.document().unwrap().kind.as_deref(), Some("note"));
    assert!(parsed.diagnostics.is_empty());
    let parsed = specengine_core::parse("x.md", b"# Just a file\n", &scheme);
    assert_eq!(parsed.document().unwrap().kind, None);
}
