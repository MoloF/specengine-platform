//! AC-07, AC-08, AC-09 of docs/features/spec-check.md: ID definitions.
//! A number-shape definition (`id:`, `{#ID}`) has its prefix's `width`
//! digits (references never judged); a mixed-script ID is the error
//! `homoglyph` with its Latin fix as data (the bytes stay); an ID defined in
//! two files is `id-taken` on each later file by path, naming the first
//! (feature-scoped prefixes exempt); under `records` a file holding `id: X`
//! is named `X` + `.` or `-`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use common::check::{Config, fixture_input, show, triples, with_code};
use common::fixture;
use specengine_core::check::{CheckFile, CheckInput, Verdict};
use specengine_model::Severity;

const TOML: &str = "\
[paths]
records = \"docs/decisions\"

[ids]
ADR = { kind = \"decision\", width = 4 }
REP = { kind = \"report\",   width = 3 }
R   = { kind = \"requirement\", width = 2 }
AC  = { kind = \"criterion\", width = 2, scope = \"feature\" }
";

fn config() -> Config {
    Config::from_toml(TOML)
}

// ------------------------------------------------------------------ AC-07

#[test]
fn a_definition_with_the_wrong_width_is_id_width() {
    let report = config().check(&[
        (
            "docs/decisions/ADR-001.md",
            "---\nid: ADR-001\nsupersedes: [ADR-00002]\n---\n# D\n\n## Part {#R-123}\n",
        ),
        (
            "docs/decisions/ADR-00002.md",
            "---\nid: ADR-00002\n---\n# E\n",
        ),
        (
            "docs/decisions/ADR-0003.md",
            "---\nid: ADR-0003\n---\n# F\n",
        ),
    ]);
    let found: Vec<(&str, usize, &str)> = with_code(&report, "id-width")
        .iter()
        .map(|f| (f.path.as_str(), f.line, f.subject.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            ("docs/decisions/ADR-00002.md", 2, "ADR-00002"),
            ("docs/decisions/ADR-001.md", 2, "ADR-001"),
            ("docs/decisions/ADR-001.md", 7, "R-123"),
        ],
        "{}",
        show(&report)
    );
    assert!(
        found
            .iter()
            .all(|(path, _, _)| *path != "docs/decisions/ADR-0003.md")
    );
}

#[test]
fn a_reference_is_never_judged_by_width() {
    // `R-7` is written with one digit in a reference: resolved or dangling,
    // never `id-width`.
    let report = config().check(&[
        ("docs/r.md", "---\nid: R-07\n---\n# R\n"),
        (
            "docs/s.md",
            "---\nid: R-08\nrefs: [R-7, R-07]\n---\n# S\n\nSee R-7.\n",
        ),
    ]);
    assert!(
        with_code(&report, "id-width").is_empty(),
        "{}",
        show(&report)
    );
}

/// `RE\u{0420}-001`: Cyrillic Er for the Latin P.
const DEFINED_MIXED: &str = "RE\u{0420}-001";
/// `R\u{0415}P-001`: Cyrillic Ie for the Latin E.
const REFERENCED_MIXED: &str = "R\u{0415}P-001";

#[test]
fn a_mixed_script_definition_and_reference_are_homoglyph_errors_with_a_latin_fix() {
    let config = config();
    let defining = format!("---\nid: {DEFINED_MIXED}\n---\n# Report\n");
    let citing = format!("---\nid: R-01\nrefs: [{REFERENCED_MIXED}]\n---\n# R\n");
    let input = config.input(&[("docs/rep.md", &defining), ("docs/r.md", &citing)]);
    let before = input.clone();
    let report = config.run(&input);
    assert_eq!(input, before, "the input is untouched");
    let homoglyphs = with_code(&report, "homoglyph");
    assert_eq!(homoglyphs.len(), 2, "{}", show(&report));
    for finding in &homoglyphs {
        assert_eq!(finding.severity, Severity::Error, "a homoglyph blocks");
        let fix = finding.fix.as_ref().expect("the Latin fix");
        assert_eq!(fix.text, "REP-001");
        let file = input
            .files
            .iter()
            .find(|file| file.path == finding.path)
            .unwrap();
        let written = std::str::from_utf8(&file.bytes[fix.span.range()]).unwrap();
        assert!(
            written == DEFINED_MIXED || written == REFERENCED_MIXED,
            "{}: the span holds the ID as written, got {written:?}",
            finding.path
        );
        assert_eq!(
            finding.subject, written,
            "the subject is the text as written"
        );
    }
    let paths: Vec<&str> = homoglyphs.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["docs/r.md", "docs/rep.md"]);
    assert_eq!(report.verdict, Verdict::Blocked);
    assert!(
        report.to_json().contains("\"fix\":{\"span\":["),
        "the fix is data in the JSON"
    );
}

// ------------------------------------------------------------------ AC-08

#[test]
fn an_id_in_two_files_is_id_taken_on_the_later_naming_the_first() {
    let config = config();
    let files = [
        ("docs/b.md", "---\nid: R-02\n---\n# B\n\n## Twice {#R-01}\n"),
        ("docs/a.md", "---\nid: R-01\n---\n# A\n"),
        ("docs/c.md", "# C\n\n## Thrice {#R-01}\n"),
    ];
    let report = config.check(&files);
    let taken: Vec<(&str, usize, &str, &str)> = with_code(&report, "id-taken")
        .iter()
        .map(|f| {
            (
                f.path.as_str(),
                f.line,
                f.subject.as_str(),
                f.message.as_str(),
            )
        })
        .collect();
    assert_eq!(taken.len(), 2, "{}", show(&report));
    assert_eq!(
        (taken[0].0, taken[0].1, taken[0].2),
        ("docs/b.md", 6, "R-01")
    );
    assert_eq!(
        (taken[1].0, taken[1].1, taken[1].2),
        ("docs/c.md", 3, "R-01")
    );
    for (_, _, _, message) in &taken {
        assert!(message.contains("docs/a.md"), "names the first: {message}");
    }
    assert!(
        with_code(&report, "id-taken")
            .iter()
            .all(|f| f.path != "docs/a.md"),
        "the first holder is not reported"
    );
    // Whatever the input order.
    let mut reversed = files;
    reversed.reverse();
    assert_eq!(config.check(&reversed).findings, report.findings);
}

#[test]
fn a_feature_scoped_id_in_two_files_is_no_finding() {
    let report = config().check(&[
        ("docs/features/one.md", "# One\n\n## Crit {#AC-01}\n"),
        ("docs/features/two.md", "# Two\n\n## Crit {#AC-01}\n"),
    ]);
    assert!(
        with_code(&report, "id-taken").is_empty(),
        "{}",
        show(&report)
    );
    assert!(
        report.findings.iter().all(|f| f.code == "class-missing"),
        "{}",
        show(&report)
    );
}

#[test]
fn a_repeat_inside_one_file_is_the_parser_duplicate_id_error() {
    let report = config().check(&[(
        "docs/a.md",
        "---\nclass: generated\nid: R-01\n---\n# A\n\n## Again {#R-01}\n",
    )]);
    assert_eq!(
        triples(&report),
        [("docs/a.md".into(), "duplicate-id".into(), "R-01".into())],
        "{}",
        show(&report)
    );
    assert_eq!(
        report.findings[0].severity,
        Severity::Error,
        "raised to error"
    );
}

// ------------------------------------------------------------------ AC-09

#[test]
fn the_fixture_records_are_named_after_their_ids() {
    for corpus in ["spec-a", "spec-b"] {
        let (config, input) = fixture_input(&fixture(corpus));
        let report = config.run(&input);
        assert!(
            with_code(&report, "file-name").is_empty(),
            "{corpus}:\n{}",
            show(&report)
        );
        assert!(
            input
                .files
                .iter()
                .filter(|f| f.path.starts_with("docs/records/"))
                .count()
                >= 9,
            "{corpus}: the records role is walked"
        );
    }
}

#[test]
fn a_record_named_after_another_id_is_file_name() {
    let config = Config::from_toml(
        "[paths]\nrecords = \"docs/records\"\n\n[ids]\nQ = { kind = \"question\", width = 3 }\nADR = { kind = \"decision\", width = 4 }\n",
    );
    let report = config.check(&[
        ("docs/records/Q/Q-32.md", "---\nid: Q-031\n---\n# Q\n"),
        (
            "docs/records/ADR/ADR-00011.md",
            "---\nid: ADR-0001\n---\n# A\n",
        ),
        (
            "docs/records/ADR/ADR-0002-sync.md",
            "---\nid: ADR-0002\n---\n# B\n",
        ),
        (
            "docs/records/ADR/ADR-0003.md",
            "---\nid: ADR-0003\n---\n# C\n",
        ),
        ("docs/elsewhere/x.md", "---\nid: ADR-0004\n---\n# D\n"),
    ]);
    let named: Vec<(&str, usize, &str)> = with_code(&report, "file-name")
        .iter()
        .map(|f| (f.path.as_str(), f.line, f.subject.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            ("docs/records/ADR/ADR-00011.md", 2, "ADR-0001"),
            ("docs/records/Q/Q-32.md", 2, "Q-031"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn check_input_bytes_are_those_given() {
    // `CheckFile::parse` keeps the bytes it parsed: the check reads lines
    // and keys from them, never from disk.
    let config = config();
    let bytes = format!("---\nid: {DEFINED_MIXED}\n---\n").into_bytes();
    let file = CheckFile::parse("docs/rep.md", bytes.clone(), &config.scheme);
    assert_eq!(file.bytes, bytes);
    assert_eq!(file.size, bytes.len() as u64);
    let input = CheckInput {
        files: vec![file],
        problems: Vec::new(),
    };
    let report = config.run(&input);
    assert_eq!(input.files[0].bytes, bytes);
    assert_eq!(with_code(&report, "homoglyph").len(), 1);
}
