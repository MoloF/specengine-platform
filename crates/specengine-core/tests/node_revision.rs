//! docs/features/norm-hash.md TE-5 ("Revision", ADR-0040): every row of
//! the revision table through core's `revision` and the parse; a section's
//! revision is its own `rev=N`, a document's its front-matter `rev:`, never
//! inherited; none written is 1, one written but unreadable is unknown.
//! `bad-rev` stays a warning (the parser's and the check's severity), and
//! the reference grammar is unchanged: `R-1@0` parses with revision 0.
//!
//! M: 0 accepted (`rev_zero_is_bad_rev`); the first `rev=` wins
//! (`repeated_rev_is_bad_rev`); unreadable read as 1; a section inheriting
//! its document's; `@0` made bad (`r1_at_0_parses`).

mod common;

use common::check::Config;
use common::{corpus_scheme, fixture};
use specengine_core::check::parser_severity;
use specengine_core::{Revision, norm_input, revision};
use specengine_model::{DiagnosticCode, IdScheme, LinkTarget, ParsedFile, Severity};

fn scheme() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

fn parse(text: &str) -> ParsedFile {
    specengine_core::parse("docs/spec/r.md", text.as_bytes(), &scheme())
}

fn ord_of(parsed: &ParsedFile, id: &str) -> usize {
    parsed
        .nodes
        .iter()
        .position(|node| node.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id}: {:?}", parsed.nodes))
}

/// `(code, line, severity)` of every finding of the parse.
fn findings(parsed: &ParsedFile) -> Vec<(DiagnosticCode, usize, Severity)> {
    parsed
        .diagnostics
        .iter()
        .map(|found| (found.code, found.line, found.severity))
        .collect()
}

/// A document with front-matter `head` (its lines, without fences) and a
/// section whose heading attributes are `attrs`.
fn document(head: &str, attrs: &str) -> String {
    format!(
        "---\nid: MEC-REV\n{head}---\n\n# Rev\n\nText.\n\n## Part {{#RULE-REV-PART{attrs}}}\n\nPart text.\n"
    )
}

/// The section's revision and findings in `document(head, attrs)`.
fn section(head: &str, attrs: &str) -> (Revision, Vec<(DiagnosticCode, usize, Severity)>) {
    let parsed = parse(&document(head, attrs));
    let ord = ord_of(&parsed, "RULE-REV-PART");
    (revision(&parsed, ord), findings(&parsed))
}

/// The document's revision and findings for front-matter `head`.
fn doc(head: &str) -> (Revision, Vec<(DiagnosticCode, usize, Severity)>) {
    let parsed = parse(&document(head, ""));
    (revision(&parsed, 0), findings(&parsed))
}

/// The section heading's line in `document("", attrs)`.
const SECTION_LINE: usize = 9;

fn bad_rev_at(line: usize) -> Vec<(DiagnosticCode, usize, Severity)> {
    vec![(DiagnosticCode::BadRev, line, Severity::Warning)]
}

#[test]
fn nothing_written_is_revision_1() {
    let (rev, found) = section("", "");
    assert_eq!(rev, Revision::Unwritten);
    assert_eq!(rev.number(), Some(1));
    assert!(found.is_empty(), "{found:?}");
    let (rev, found) = doc("");
    assert_eq!(rev, Revision::Unwritten);
    assert_eq!(rev.number(), Some(1));
    assert!(found.is_empty(), "{found:?}");
    // A file without front-matter: its document has revision 1 too.
    let parsed = parse("# Plain\n\nText.\n");
    assert_eq!(revision(&parsed, 0), Revision::Unwritten);
}

#[test]
fn a_written_revision_is_read() {
    assert_eq!(doc("rev: 3\n"), (Revision::Written(3), vec![]));
    assert_eq!(section("", " rev=3"), (Revision::Written(3), vec![]));
    assert_eq!(Revision::Written(3).number(), Some(3));
    // The spec's own row: `## Cost {#RULE-SPRINT-COST rev=3}` in spec-a.
    let bytes = std::fs::read(fixture("spec-a").join("docs/spec/movement/sprint.md")).unwrap();
    let parsed = specengine_core::parse("docs/spec/movement/sprint.md", &bytes, &scheme());
    assert_eq!(
        revision(&parsed, ord_of(&parsed, "RULE-SPRINT-COST")),
        Revision::Written(3)
    );
    // Its nested section writes none: 1, not its parent's 3.
    assert_eq!(
        revision(&parsed, ord_of(&parsed, "EDGE-SPRINT-EMPTY")),
        Revision::Unwritten
    );
    assert_eq!(revision(&parsed, 0), Revision::Unwritten);
    // The largest revision, nine digits.
    assert_eq!(
        section("", " rev=999999999"),
        (Revision::Written(999_999_999), vec![])
    );
    assert_eq!(
        doc("rev: 999999999\n"),
        (Revision::Written(999_999_999), vec![])
    );
}

#[test]
fn leading_zeros_are_read() {
    assert_eq!(section("", " rev=007"), (Revision::Written(7), vec![]));
    assert_eq!(
        section("", " rev=000000001"),
        (Revision::Written(1), vec![])
    );
}

/// RD-2's named test: `rev=0` and `rev: 0` are `bad-rev` and unknown. M:
/// `0..=` kept in `front_matter.rs` or `section_rev`.
#[test]
fn rev_zero_is_bad_rev() {
    let (rev, found) = section("", " rev=0");
    assert_eq!(rev, Revision::Unreadable, "rev=0");
    assert_eq!(rev.number(), None);
    assert_eq!(found, bad_rev_at(SECTION_LINE), "rev=0");
    assert_eq!(
        section("", " rev=000"),
        (Revision::Unreadable, bad_rev_at(SECTION_LINE))
    );
    let (rev, found) = doc("rev: 0\n");
    assert_eq!(rev, Revision::Unreadable, "rev: 0");
    assert_eq!(found, bad_rev_at(3), "rev: 0");
    let parsed = parse(&document("rev: 0\n", ""));
    assert_eq!(parsed.nodes[0].rev, None, "no revision stored");
    // Kept in `extra`, as a mistyped value is.
    assert!(
        parsed.nodes[0]
            .extra
            .iter()
            .flatten()
            .any(|entry| entry.key == "rev"),
        "{:?}",
        parsed.nodes[0].extra
    );
}

#[test]
fn malformed_revisions_are_bad_rev_and_unknown() {
    for attrs in [" rev=3a", " rev", " rev=1234567890", " rev=-1", " rev="] {
        assert_eq!(
            section("", attrs),
            (Revision::Unreadable, bad_rev_at(SECTION_LINE)),
            "{attrs:?}"
        );
    }
    for head in ["rev: -1\n", "rev: 1234567890\n"] {
        assert_eq!(doc(head), (Revision::Unreadable, bad_rev_at(3)), "{head:?}");
    }
}

/// RD-2's named test: `rev=2 rev=3` on one heading is `bad-rev` and
/// unknown, never 2. M: the first `rev=` wins.
#[test]
fn repeated_rev_is_bad_rev() {
    let (rev, found) = section("", " rev=2 rev=3");
    assert_eq!(rev, Revision::Unreadable);
    assert_eq!(found, bad_rev_at(SECTION_LINE));
    // The same value twice is still a repeat.
    assert_eq!(
        section("", " rev=2 rev=2"),
        (Revision::Unreadable, bad_rev_at(SECTION_LINE))
    );
    // `rev:` twice in front-matter: YAML's own finding if it refuses the
    // repeat, else `bad-rev` at the second; unknown either way.
    let (rev, found) = doc("rev: 2\nrev: 3\n");
    assert_eq!(rev, Revision::Unreadable);
    assert!(
        found == [(DiagnosticCode::FrontmatterYaml, found[0].1, Severity::Error)]
            || found == bad_rev_at(4),
        "{found:?}"
    );
}

#[test]
fn a_mistyped_revision_is_frontmatter_type_and_unknown() {
    for head in ["rev: \"3\"\n", "rev: [3]\n", "rev: 2.5\n", "rev: {n: 3}\n"] {
        let (rev, found) = doc(head);
        assert_eq!(rev, Revision::Unreadable, "{head:?}");
        assert_eq!(
            found.iter().map(|f| f.0).collect::<Vec<_>>(),
            [DiagnosticCode::FrontmatterType],
            "{head:?}"
        );
    }
}

/// A failed front-matter leaves the document's revision unknown, even
/// when a `rev:` line is readable; its sections keep their own.
#[test]
fn a_failed_front_matter_is_unknown() {
    let cases = [
        (
            DiagnosticCode::FrontmatterUnclosed,
            "---\nid: MEC-REV\nrev: 3\n\n# Rev\n\n## Part {#RULE-REV-PART rev=4}\n\nPart.\n",
        ),
        (
            DiagnosticCode::FrontmatterYaml,
            "---\nid: MEC-REV\nrev: 3\nlinks: [open\n---\n\n## Part {#RULE-REV-PART rev=4}\n\nPart.\n",
        ),
        (
            DiagnosticCode::FrontmatterNotMapping,
            "---\n- rev\n- 3\n---\n\n## Part {#RULE-REV-PART rev=4}\n\nPart.\n",
        ),
    ];
    for (code, text) in cases {
        let parsed = parse(text);
        assert!(
            parsed.diagnostics.iter().any(|found| found.code == code),
            "{code}: {:?}",
            parsed.diagnostics
        );
        assert_eq!(revision(&parsed, 0), Revision::Unreadable, "{code}");
        assert_eq!(
            revision(&parsed, ord_of(&parsed, "RULE-REV-PART")),
            Revision::Written(4),
            "{code}: the section keeps its own"
        );
    }
    // Only those three fail the front-matter: a `frontmatter-type` on
    // another key leaves a sound `rev:` read (the table's
    // `frontmatter-type` row is a mistyped `rev` itself).
    let parsed = parse("---\nid: MEC-REV\nrev: 3\ntier: high\n---\n\n# Rev\n");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|found| found.code == DiagnosticCode::FrontmatterType),
        "{:?}",
        parsed.diagnostics
    );
    assert_eq!(revision(&parsed, 0), Revision::Written(3));
}

/// A section never inherits: no `rev=` under a document `rev: 5` is 1;
/// a section's `rev=` never gives the document one either. M: a section
/// inheriting its document's.
#[test]
fn a_section_never_inherits_its_documents_revision() {
    assert_eq!(section("rev: 5\n", ""), (Revision::Unwritten, vec![]));
    assert_eq!(
        section("rev: 5\n", " rev=2"),
        (Revision::Written(2), vec![])
    );
    assert_eq!(doc(""), (Revision::Unwritten, vec![]));
    let parsed = parse(&document("", " rev=8"));
    assert_eq!(revision(&parsed, 0), Revision::Unwritten);
    // Nested: a section under a `rev=4` section writes none of its own.
    let parsed = parse(
        "---\nid: MEC-REV\nrev: 5\n---\n\n## Outer {#RULE-REV-OUTER rev=4}\n\n### Inner {#EDGE-REV-INNER}\n\nInner.\n",
    );
    assert_eq!(
        revision(&parsed, ord_of(&parsed, "EDGE-REV-INNER")),
        Revision::Unwritten
    );
    assert_eq!(
        revision(&parsed, ord_of(&parsed, "RULE-REV-OUTER")),
        Revision::Written(4)
    );
    assert_eq!(revision(&parsed, 0), Revision::Written(5));
}

/// Unreadable is never read as 1: every unreadable row has no number,
/// and a position past the last node is unknown too. M: unreadable read
/// as 1.
#[test]
fn unreadable_is_never_one() {
    for attrs in [" rev=0", " rev=x", " rev=1 rev=1"] {
        let (rev, _) = section("", attrs);
        assert_eq!(rev.number(), None, "{attrs:?}");
        assert_ne!(rev, Revision::Unwritten, "{attrs:?}");
    }
    for head in ["rev: 0\n", "rev: \"1\"\n", "rev: -5\n"] {
        let (rev, _) = doc(head);
        assert_eq!(rev.number(), None, "{head:?}");
    }
    let parsed = parse(&document("", ""));
    assert_eq!(revision(&parsed, parsed.nodes.len()), Revision::Unreadable);
    assert_eq!(Revision::Unreadable.number(), None);
    // A file that is not UTF-8 has no node: unknown.
    let parsed = specengine_core::parse("docs/x.md", b"rev \xff", &scheme());
    assert_eq!(revision(&parsed, 0), Revision::Unreadable);
}

/// `bad-rev` stays a warning: the parser's severity, the check's table,
/// and a check report of a `rev=0` file (no error, nothing blocked).
#[test]
fn bad_rev_stays_a_warning() {
    assert_eq!(DiagnosticCode::BadRev.severity(), Severity::Warning);
    assert_eq!(parser_severity(DiagnosticCode::BadRev), Severity::Warning);
    let config = Config::from_toml(
        &std::fs::read_to_string(fixture("spec-a").join("specengine.toml")).unwrap(),
    );
    let text = document("rev: 0\n", " rev=2 rev=3");
    let report = config.check(&[("docs/spec/r.md", text.as_str())]);
    let bad: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.code == "bad-rev")
        .map(|finding| (finding.line, finding.severity))
        .collect();
    assert_eq!(
        bad,
        [
            (3, Severity::Warning),
            (SECTION_LINE + 1, Severity::Warning)
        ],
        "{:?}",
        report.findings
    );
}

/// `R-1@0` parses: the grammar keeps `@0` (a marker's `@0` is
/// `spec_ahead`, never a finding), inline and in front-matter, and the
/// recipe renders it. M: `@0` made bad.
#[test]
fn r1_at_0_parses() {
    let r = IdScheme::new(vec![specengine_model::PrefixSpec::number(
        "R",
        "requirement",
        1,
    )])
    .unwrap();
    let found = specengine_model::grammar::scan("R-1@0", 0, &r);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].reference.id, "R-1");
    assert_eq!(found[0].reference.rev, Some(0));
    assert_eq!(found[0].bad_rev, None);
    let one = specengine_model::grammar::parse_reference("R-1@0", 0, &r).expect("one reference");
    assert_eq!(one.reference.rev, Some(0));

    let text = "---\nid: MEC-REV\nlinks:\n  derived_from: [R-12@0]\n---\n\n# Rev\n\nSee R-12@0 and R-12@00.\n";
    let parsed = parse(text);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let revs: Vec<_> = parsed
        .links
        .iter()
        .map(|link| match &link.dst {
            LinkTarget::Reference(reference) => (link.origin, reference.rev),
            LinkTarget::Path(path) => panic!("{path:?}"),
        })
        .collect();
    assert_eq!(revs.len(), 3, "{:?}", parsed.links);
    assert!(revs.iter().all(|(_, rev)| *rev == Some(0)), "{revs:?}");
    let input = norm_input(&parsed, text.as_bytes(), 0).expect("hashed");
    let rendered = b"derived_from ref:R-12@0";
    assert!(
        input
            .windows(rendered.len())
            .any(|window| window == rendered),
        "{:?}",
        String::from_utf8_lossy(&input)
    );
}
