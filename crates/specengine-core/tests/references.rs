//! AC-10 and AC-11 of docs/features/spec-parser.md: references are
//! recognised only for configured prefixes, with exact boundaries, where the
//! text is prose or inline code (never fences, HTML, link destinations,
//! attribute blocks, escapes); look-alike IDs are normalised with a
//! `homoglyph` finding carrying the Latin fix, while a configured Cyrillic
//! alias is taken verbatim (detection half of product AC-11).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use specengine_model::{
    DiagnosticCode, IdScheme, IdScript, LinkOrigin, LinkTarget, ParsedFile, PrefixSpec, Reference,
    Span,
};

use common::{numbers, parse_str, text_of};

/// Inline references of a parse: (verbatim text, reference).
fn inline(text: &str, parsed: &ParsedFile) -> Vec<(String, Reference)> {
    parsed
        .links
        .iter()
        .filter(|l| l.origin == LinkOrigin::Inline)
        .map(|l| match &l.dst {
            LinkTarget::Reference(r) => {
                let span = r.span.expect("an inline reference has a span");
                (text_of(text.as_bytes(), span).to_owned(), r.clone())
            }
            LinkTarget::Path(p) => panic!("inline path link {p:?}"),
        })
        .collect()
}

fn inline_ids(text: &str, scheme: &IdScheme) -> Vec<String> {
    inline(text, &parse_str(text, scheme))
        .into_iter()
        .map(|(verbatim, _)| verbatim)
        .collect()
}

// ------------------------------------------------------------------- AC-10

const PROSE: &str = "UTF-8, SHA-256, AC-1 and R-12";

#[test]
fn only_configured_prefixes_are_references() {
    let parsed = parse_str(PROSE, &numbers(&["R"]));
    let found = inline(PROSE, &parsed);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].1.id, "R-12");
    let start = PROSE.find("R-12").unwrap();
    assert_eq!(found[0].1.span, Some(Span::new(start, start + 4)));

    assert_eq!(inline_ids(PROSE, &numbers(&["R", "AC"])), ["AC-1", "R-12"]);
    // The lexer directly, same answer.
    let direct: Vec<String> = specengine_model::grammar::scan(PROSE, 0, &numbers(&["R", "AC"]))
        .into_iter()
        .map(|f| f.reference.id)
        .collect();
    assert_eq!(direct, ["AC-1", "R-12"]);
}

#[test]
fn boundaries_reject_glued_ids() {
    let scheme = numbers(&["R"]);
    for text in [
        "FOO-R-12",
        "R-12abc",
        "R-12-3",
        "x_R-12",
        "-R-12",
        "R-12_x",
        "R-",
        "R-x",
        "RR-12",
        "r-12",
        "\u{0444}R-12",
        "R-12\u{0444}",
        "R-\u{0661}\u{0662}",
    ] {
        assert!(
            inline_ids(text, &scheme).is_empty(),
            "{text:?} cites nothing: {:?}",
            inline_ids(text, &scheme)
        );
    }
    for (text, want) in [
        ("R-12.", "R-12"),
        ("(R-12)", "R-12"),
        ("R-12, R-3;", "R-12"),
        ("R-12- dash", "R-12"),
        ("\u{2014}R-12\u{2014}", "R-12"),
        ("R-012", "R-012"),
    ] {
        let found = inline_ids(text, &scheme);
        assert_eq!(found.first().map(String::as_str), Some(want), "{text:?}");
    }
    // width is never checked on recognition: R-012 and R-12 are distinct IDs.
    let parsed = parse_str("R-012 R-12", &scheme);
    let ids: Vec<String> = inline("R-012 R-12", &parsed)
        .into_iter()
        .map(|(_, r)| r.id)
        .collect();
    assert_eq!(ids, ["R-012", "R-12"]);
}

#[test]
fn fences_comments_destinations_escapes_and_attributes_are_not_read() {
    let scheme = numbers(&["R"]);
    let text = "\
Prose R-1 and `R-2` in inline code.

```
R-3 in a fence
```

    R-4 indented code

<!-- R-5 in a comment -->

<div>
R-6 in an HTML block
</div>

A [link to R-7](R-8) and <span>R-9</span> and R\\-10 escaped.

| ID | Text |
|---|---|
| R-11 | a `R-12` cell |

## Heading R-13 {#R-14}
";
    let found = inline_ids(text, &scheme);
    assert_eq!(
        found,
        ["R-1", "R-2", "R-7", "R-9", "R-11", "R-12", "R-13"],
        "prose, inline code, link text, inline text between tags, table cells, heading text"
    );
    let parsed = parse_str(text, &scheme);
    assert_eq!(
        parsed
            .sections()
            .iter()
            .map(|s| s.id.as_deref())
            .collect::<Vec<_>>(),
        [Some("R-14")],
        "the attribute block defines, it does not mention"
    );
}

#[test]
fn inline_code_counts_once_and_fences_never() {
    let scheme = numbers(&["R"]);
    assert_eq!(inline_ids("`R-12`", &scheme), ["R-12"]);
    assert!(inline_ids("```\nR-12\n```\n", &scheme).is_empty());
    assert!(inline_ids("~~~md\nR-12\n~~~\n", &scheme).is_empty());
    assert!(inline_ids("<!-- R-12 -->\n", &scheme).is_empty());
    assert!(inline_ids("text <!-- R-12 --> text\n", &scheme).is_empty());
}

#[test]
fn qualifiers_are_read_by_look_back_only_after_a_clean_boundary() {
    let scheme = numbers(&["R"]);
    let cases: [(&str, &str, Option<&str>, Option<&str>); 6] = [
        ("see slug/R-12 now", "slug/R-12", Some("slug"), None),
        ("see shared:R-12 now", "shared:R-12", None, Some("shared")),
        ("see p:s-2/R-12 now", "p:s-2/R-12", Some("s-2"), Some("p")),
        ("https://h.io/R-12", "R-12", None, None),
        ("a.b/R-12", "R-12", None, None),
        ("Slug/R-12", "R-12", None, None),
    ];
    for (text, verbatim, scope, project) in cases {
        let parsed = parse_str(text, &scheme);
        let found = inline(text, &parsed);
        assert_eq!(found.len(), 1, "{text:?}: {found:?}");
        let (got, reference) = &found[0];
        assert_eq!(got, verbatim, "{text:?}");
        assert_eq!(reference.scope.as_deref(), scope, "{text:?}");
        assert_eq!(reference.project.as_deref(), project, "{text:?}");
    }
}

// ------------------------------------------------------------------- AC-11

#[test]
fn look_alike_in_text_is_normalised_with_a_fix() {
    let scheme = IdScheme::new(vec![PrefixSpec::number("DEC", "decision", 4)]).unwrap();
    // "DEC-0023" with CYRILLIC CAPITAL LETTER IE (U+0415) for E.
    let written = "D\u{0415}C-0023";
    let text = format!("As decided in {written}.\n");
    let parsed = parse_str(&text, &scheme);
    let found = inline(&text, &parsed);
    assert_eq!(found.len(), 1);
    let (verbatim, reference) = &found[0];
    assert_eq!(verbatim, written);
    assert_eq!(reference.id, "DEC-0023");
    assert_eq!(reference.script, IdScript::Mixed);
    assert_eq!(reference.alias_of, None);
    let homoglyphs: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::Homoglyph)
        .collect();
    assert_eq!(homoglyphs.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(homoglyphs[0].fix.as_deref(), Some("DEC-0023"));
    assert_eq!(homoglyphs[0].line, 1);
    let span = homoglyphs[0].span.expect("the homoglyph span");
    assert_eq!(text_of(text.as_bytes(), span), written);
}

#[test]
fn look_alike_in_the_id_key_is_normalised_with_a_fix() {
    let scheme = IdScheme::new(vec![PrefixSpec::name("PAT", "pattern")]).unwrap();
    // "PAT-1" with CYRILLIC CAPITAL LETTER ER (U+0420) for P.
    let text = "---\nid: \u{0420}AT-1\n---\n";
    let parsed = parse_str(text, &scheme);
    let document = parsed.document().unwrap();
    assert_eq!(document.id.as_deref(), Some("PAT-1"));
    assert_eq!(document.script, Some(IdScript::Mixed));
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| (d.code, d.line))
            .collect::<Vec<_>>(),
        [(DiagnosticCode::Homoglyph, 2)]
    );
    let diagnostic = &parsed.diagnostics[0];
    assert_eq!(diagnostic.fix.as_deref(), Some("PAT-1"));
    let span = diagnostic.span.expect("span of the look-alike ID");
    assert_eq!(text_of(text.as_bytes(), span), "\u{0420}AT-1");
}

#[test]
fn fullwidth_and_greek_look_alikes_are_normalised_too() {
    let scheme = numbers(&["AB"]);
    for written in [
        "\u{FF21}B-12",        // FULLWIDTH LATIN CAPITAL LETTER A
        "\u{0391}B-12",        // GREEK CAPITAL LETTER ALPHA
        "A\u{0392}-12",        // GREEK CAPITAL LETTER BETA
        "AB-\u{FF11}\u{FF12}", // FULLWIDTH DIGITS
    ] {
        let text = format!("see {written} now");
        let parsed = parse_str(&text, &scheme);
        let found = inline(&text, &parsed);
        assert_eq!(found.len(), 1, "{written:?}");
        assert_eq!(found[0].1.id, "AB-12", "{written:?}");
        assert_eq!(found[0].0, written);
        let fixes: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter_map(|d| d.fix.as_deref())
            .collect();
        assert_eq!(fixes, ["AB-12"], "{written:?}");
    }
}

#[test]
fn a_configured_cyrillic_alias_is_taken_verbatim_without_a_finding() {
    let scheme = IdScheme::new(vec![
        PrefixSpec::number("Q", "question", 3).with_aliases(["\u{0412}\u{041E}\u{041F}"]),
    ])
    .unwrap();
    // "VOP-7" in Cyrillic: the legacy question prefix.
    let written = "\u{0412}\u{041E}\u{041F}-7";
    let text = format!("Open question {written} stays.\n");
    let parsed = parse_str(&text, &scheme);
    let found = inline(&text, &parsed);
    assert_eq!(found.len(), 1);
    let (verbatim, reference) = &found[0];
    assert_eq!(verbatim, written);
    assert_eq!(reference.id, written, "an alias keeps the ID as written");
    assert_eq!(reference.alias_of.as_deref(), Some("Q"));
    assert_eq!(reference.script, IdScript::NonLatin);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn a_latin_id_is_clean_and_latin() {
    let scheme = IdScheme::new(vec![PrefixSpec::number("DEC", "decision", 4)]).unwrap();
    let parsed = parse_str("DEC-0023", &scheme);
    let found = inline("DEC-0023", &parsed);
    assert_eq!(found[0].1.script, IdScript::Latin);
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn bad_revision_is_a_warning_and_the_reference_stands() {
    let scheme = numbers(&["R"]);
    let text = "R-12@1234567890 and R-3@7";
    let parsed = parse_str(text, &scheme);
    let found = inline(text, &parsed);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].1.id, "R-12");
    assert_eq!(found[0].1.rev, None);
    assert_eq!(found[1].1.rev, Some(7));
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        [DiagnosticCode::BadRev]
    );
}
