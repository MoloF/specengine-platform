//! AC-09 of docs/features/spec-parser.md: the one reference grammar.
//! Table-driven splits of every "Reference grammar" example and of
//! `[[R-12|label]]`; `canon:` as path + anchor; `status: superseded-by X` as
//! a `supersedes` link; `#` joins only before an ID (`ADR-0002#layout` is
//! `ADR-0002`). Plus the `[ids]` scheme rules of the "Data" section:
//! validation of prefixes, widths, shapes and aliases, and `file:line`
//! errors of `IdScheme::from_toml`.
//!
//! docs/features/release-tail.md RT9 and RT10's Rust half: rule (7) takes
//! a `#ID` and its `@rev` right after a guide citation's ID into the
//! citation, in one pass; one table of odd texts, `fixtures/
//! guide-citations.json`, byte for byte the UI's `ui/src/markdown/
//! guide-citations.json`, is read here as the UI's `markdown/guide` test
//! reads it.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

use specengine_core::IdSchemeToml;
use specengine_model::grammar::{
    Canon, is_slug, parse_canon, parse_definition, parse_reference, scan, split_superseded_by,
};
use specengine_model::{
    IdScheme, IdScope, IdScript, LinkTarget, PrefixSpec, RefForm, SchemeField, Shape, Span,
};

fn scheme() -> IdScheme {
    IdScheme::new(vec![
        PrefixSpec::number("R", "requirement", 2),
        PrefixSpec::number("AC", "criterion", 2),
        PrefixSpec::number("ADR", "decision", 4),
        PrefixSpec::name("PAT", "pattern"),
        PrefixSpec::name("MEC", "mechanic"),
        PrefixSpec::name("RULE", "rule"),
        PrefixSpec::name("TERM", "term"),
        PrefixSpec::number("Q", "question", 3).with_aliases(["QST", "\u{0412}\u{041E}\u{041F}"]),
    ])
    .expect("valid scheme")
}

/// Expected split of one reference.
#[derive(Debug, Default)]
struct Want {
    id: &'static str,
    project: Option<&'static str>,
    scope: Option<&'static str>,
    section: Option<&'static str>,
    rev: Option<u32>,
    wiki: bool,
    label: Option<&'static str>,
    alias_of: Option<&'static str>,
}

#[test]
fn every_grammar_example_splits_as_documented() {
    let table: Vec<(&str, Want)> = vec![
        (
            "R-12@3",
            Want {
                id: "R-12",
                rev: Some(3),
                ..Want::default()
            },
        ),
        (
            "slug/AC-07",
            Want {
                id: "AC-07",
                scope: Some("slug"),
                ..Want::default()
            },
        ),
        (
            "shared:PAT-PROBES@3",
            Want {
                id: "PAT-PROBES",
                project: Some("shared"),
                rev: Some(3),
                ..Want::default()
            },
        ),
        (
            "MEC-STAMINA#RULE-STAM-REGEN",
            Want {
                id: "MEC-STAMINA",
                section: Some("RULE-STAM-REGEN"),
                ..Want::default()
            },
        ),
        (
            "[[R-12]]",
            Want {
                id: "R-12",
                wiki: true,
                ..Want::default()
            },
        ),
        (
            "[[R-12|label]]",
            Want {
                id: "R-12",
                wiki: true,
                label: Some("label"),
                ..Want::default()
            },
        ),
        (
            "shared:slug/MEC-STAMINA#RULE-STAM-REGEN@12",
            Want {
                id: "MEC-STAMINA",
                project: Some("shared"),
                scope: Some("slug"),
                section: Some("RULE-STAM-REGEN"),
                rev: Some(12),
                ..Want::default()
            },
        ),
        (
            "[[p-1:s-2/R-12#AC-1@9|the label]]",
            Want {
                id: "R-12",
                project: Some("p-1"),
                scope: Some("s-2"),
                section: Some("AC-1"),
                rev: Some(9),
                wiki: true,
                label: Some("the label"),
                ..Want::default()
            },
        ),
        (
            "TERM-exhausted",
            Want {
                id: "TERM-exhausted",
                ..Want::default()
            },
        ),
        (
            "R-12@123456789",
            Want {
                id: "R-12",
                rev: Some(123_456_789),
                ..Want::default()
            },
        ),
        (
            "QST-31",
            Want {
                id: "QST-31",
                alias_of: Some("Q"),
                ..Want::default()
            },
        ),
    ];
    let scheme = scheme();
    for (text, want) in table {
        let found = parse_reference(text, 100, &scheme)
            .unwrap_or_else(|| panic!("{text:?} is exactly one reference"));
        let r = &found.reference;
        assert_eq!(r.id, want.id, "{text}: id");
        assert_eq!(r.project.as_deref(), want.project, "{text}: project");
        assert_eq!(r.scope.as_deref(), want.scope, "{text}: scope");
        assert_eq!(r.section.as_deref(), want.section, "{text}: section");
        assert_eq!(r.rev, want.rev, "{text}: rev");
        assert_eq!(r.form == RefForm::Wiki, want.wiki, "{text}: form");
        assert_eq!(r.label.as_deref(), want.label, "{text}: label");
        assert_eq!(r.alias_of.as_deref(), want.alias_of, "{text}: alias_of");
        assert_eq!(r.script, IdScript::Latin, "{text}: script");
        assert_eq!(
            r.span,
            Some(Span::new(100, 100 + text.len())),
            "{text}: span covers the whole occurrence, offset by base"
        );
        assert!(
            found.homoglyphs.is_empty() && found.bad_rev.is_none(),
            "{text}"
        );
    }
}

#[test]
fn hash_joins_only_before_an_id() {
    let scheme = scheme();
    let text = "See ADR-0002#layout for the rule.";
    let found = scan(text, 0, &scheme);
    assert_eq!(found.len(), 1);
    let r = &found[0].reference;
    assert_eq!(r.id, "ADR-0002");
    assert_eq!(r.section, None, "`layout` is no ID");
    let start = text.find("ADR").unwrap();
    assert_eq!(r.span, Some(Span::new(start, start + "ADR-0002".len())));
    // As a whole scalar it is no single reference.
    assert!(parse_reference("ADR-0002#layout", 0, &scheme).is_none());
    // `#` before an unconfigured prefix does not join either.
    let found = scan("ADR-0002#FOO-1", 0, &scheme);
    assert_eq!(found[0].reference.section, None);
    // `@` joins only before 1–9 digits.
    for (text, rev, end) in [
        ("R-12@x", None, 4),
        ("R-12@", None, 4),
        ("R-12@0", Some(0), 6),
    ] {
        let found = scan(text, 0, &scheme);
        assert_eq!(found.len(), 1, "{text}");
        assert_eq!(found[0].reference.rev, rev, "{text}");
        assert_eq!(found[0].reference.span, Some(Span::new(0, end)), "{text}");
    }
    let found = scan("R-12@1234567890", 0, &scheme);
    assert_eq!(found[0].reference.rev, None);
    assert_eq!(found[0].bad_rev, Some(Span::new(4, 15)));
}

#[test]
fn recognition_rules_one_to_six() {
    let scheme = scheme();
    let ids = |text: &str| -> Vec<String> {
        scan(text, 0, &scheme)
            .into_iter()
            .map(|f| f.reference.id)
            .collect()
    };
    // (1) not preceded by `_` or `-`.
    assert!(ids("FOO-R-12").is_empty());
    assert!(ids("x_R-12").is_empty());
    // (4) right boundary.
    assert!(ids("R-12abc").is_empty());
    assert!(ids("R-12-3").is_empty());
    assert!(ids("R-12_").is_empty());
    assert_eq!(ids("R-12-"), ["R-12"]);
    // Name shape is greedy over `-alnum` segments.
    assert_eq!(ids("RULE-STAM-REGEN-2 and"), ["RULE-STAM-REGEN-2"]);
    // (5) qualifiers only after a clean boundary.
    let found = scan("https://h.io/R-12", 0, &scheme);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].reference.scope, None);
    assert_eq!(found[0].reference.span, Some(Span::new(13, 17)));
    let found = scan("(slug/R-12)", 0, &scheme);
    assert_eq!(found[0].reference.scope.as_deref(), Some("slug"));
    // A slug starts with a lowercase letter.
    let found = scan(" 9slug/R-12", 0, &scheme);
    assert_eq!(found[0].reference.scope, None);
    // (2) alias verbatim first, no homoglyph; (6) script of the verbatim ID.
    let alias = "\u{0412}\u{041E}\u{041F}-7";
    let found = scan(alias, 0, &scheme);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].reference.alias_of.as_deref(), Some("Q"));
    assert_eq!(found[0].reference.script, IdScript::NonLatin);
    assert!(found[0].homoglyphs.is_empty());
    let mixed = "R\u{0415}Q";
    let scheme_req = IdScheme::new(vec![PrefixSpec::number("REQ", "r", 3)]).unwrap();
    let found = scan(&format!("{mixed}-001"), 0, &scheme_req);
    assert_eq!(found[0].reference.id, "REQ-001");
    assert_eq!(found[0].reference.script, IdScript::Mixed);
    assert_eq!(found[0].homoglyphs.len(), 1);
    assert_eq!(found[0].homoglyphs[0].fix, "REQ-001");
    assert_eq!(IdScript::of("R-12"), IdScript::Latin);
    assert_eq!(IdScript::of(mixed), IdScript::Mixed);
    assert_eq!(IdScript::of(alias), IdScript::NonLatin);
}

#[test]
fn definitions_are_bare_ids_of_configured_prefixes() {
    let scheme = scheme();
    let definition = parse_definition("RULE-STAM-REGEN", 7, &scheme).expect("definition");
    assert_eq!(definition.id, "RULE-STAM-REGEN");
    assert_eq!(definition.kind, "rule");
    assert_eq!(definition.span, Span::new(7, 7 + 15));
    for text in [
        "slug/R-12",
        "p:R-12",
        "R-12@3",
        "R-12#AC-1",
        "[[R-12]]",
        "QST-31",
        "R-12 ",
        "FOO-1",
    ] {
        assert!(
            parse_definition(text, 0, &scheme).is_none(),
            "{text:?} defines nothing"
        );
    }
    // A look-alike definition is normalised and reported.
    let definition = parse_definition("\u{0420}AT-X", 0, &scheme).expect("look-alike");
    assert_eq!(definition.id, "PAT-X");
    assert_eq!(
        definition.homoglyph.map(|h| h.fix),
        Some("PAT-X".to_owned())
    );
}

#[test]
fn canon_is_a_reference_else_path_and_anchor() {
    let scheme = scheme();
    match parse_canon("docs/canon/architecture.md#layout", 10, &scheme) {
        Some(Canon::Path(target)) => {
            assert_eq!(target.path, "docs/canon/architecture.md");
            assert_eq!(target.anchor.as_deref(), Some("layout"));
            assert_eq!(target.span, Some(Span::new(10, 10 + 33)));
        }
        other => panic!("path + anchor expected, got {other:?}"),
    }
    match parse_canon("docs/canon/architecture.md", 0, &scheme) {
        Some(Canon::Path(target)) => assert_eq!(target.anchor, None),
        other => panic!("path expected, got {other:?}"),
    }
    match parse_canon("ADR-0002", 0, &scheme) {
        Some(Canon::Reference(found)) => assert_eq!(found.reference.id, "ADR-0002"),
        other => panic!("reference expected, got {other:?}"),
    }
    for bad in ["", "a b", "#x", "a#", "a#b#c"] {
        assert!(parse_canon(bad, 0, &scheme).is_none(), "{bad:?}");
    }
}

#[test]
fn superseded_by_splits_the_target() {
    assert_eq!(
        split_superseded_by("superseded-by ADR-0042"),
        Some((14, "ADR-0042"))
    );
    assert_eq!(
        split_superseded_by("superseded-by   ADR-0042 "),
        Some((16, "ADR-0042"))
    );
    assert_eq!(split_superseded_by("superseded-byADR-0042"), None);
    assert_eq!(split_superseded_by("superseded-by "), None);
    assert_eq!(split_superseded_by("accepted"), None);
}

/// Through the parser: `canon:` and `status: superseded-by` in front-matter.
#[test]
fn front_matter_canon_and_superseded_by_through_the_parser() {
    let scheme = scheme();
    let text = "---\nid: ADR-0007\nstatus: superseded-by ADR-0042\ncanon: docs/canon/architecture.md#layout\n---\n";
    let parsed = specengine_core::parse("ADR-0007.md", text.as_bytes(), &scheme);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let supersedes: Vec<_> = parsed
        .links
        .iter()
        .filter(|l| l.link_type == "supersedes")
        .collect();
    assert_eq!(supersedes.len(), 1);
    let link = supersedes[0];
    assert_eq!(link.src.as_deref(), Some("ADR-0042"));
    let at = text.find("ADR-0042").unwrap();
    assert_eq!(link.src_span, Some(Span::new(at, at + 8)));
    match &link.dst {
        LinkTarget::Reference(r) => assert_eq!(r.id, "ADR-0007"),
        other => panic!("{other:?}"),
    }
    let canon: Vec<_> = parsed
        .links
        .iter()
        .filter(|l| l.link_type == "canon")
        .collect();
    match &canon[0].dst {
        LinkTarget::Path(target) => {
            assert_eq!(target.path, "docs/canon/architecture.md");
            assert_eq!(target.anchor.as_deref(), Some("layout"));
        }
        other => panic!("{other:?}"),
    }
    // `superseded-by` with something that is no reference.
    let text = "---\nid: ADR-0007\nstatus: superseded-by nothing\n---\n";
    let parsed = specengine_core::parse("ADR-0007.md", text.as_bytes(), &scheme);
    assert!(parsed.links.is_empty());
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        ["unparsed-reference"]
    );
}

// ---------------------------------------------------------------- the scheme

#[test]
fn scheme_validation_names_the_entry_and_field() {
    let cases: Vec<(Vec<PrefixSpec>, usize, SchemeField)> = vec![
        (
            vec![PrefixSpec::number("r", "x", 2)],
            0,
            SchemeField::Prefix,
        ),
        (
            vec![PrefixSpec::number("R-1", "x", 2)],
            0,
            SchemeField::Prefix,
        ),
        (
            vec![PrefixSpec::number("\u{0420}", "x", 2)],
            0,
            SchemeField::Prefix,
        ),
        (
            vec![PrefixSpec::number("1R", "x", 2)],
            0,
            SchemeField::Prefix,
        ),
        (vec![PrefixSpec::number("R", "x", 0)], 0, SchemeField::Width),
        (
            vec![
                PrefixSpec::number("R", "x", 2),
                PrefixSpec::number("R", "y", 2),
            ],
            1,
            SchemeField::Prefix,
        ),
        (
            vec![
                PrefixSpec::number("R", "x", 2),
                PrefixSpec::number("Q", "y", 2).with_aliases(["R"]),
            ],
            1,
            SchemeField::Alias(0),
        ),
        (
            vec![
                PrefixSpec::number("R", "x", 2).with_aliases(["OLD"]),
                PrefixSpec::number("Q", "y", 2).with_aliases(["NEW", "OLD"]),
            ],
            1,
            SchemeField::Alias(1),
        ),
        (
            vec![PrefixSpec::number("R", "x", 2).with_aliases(["O-LD"])],
            0,
            SchemeField::Alias(0),
        ),
    ];
    for (entries, entry, field) in cases {
        let shown = format!("{entries:?}");
        let problem = IdScheme::new(entries).expect_err(&shown);
        assert_eq!(
            (problem.entry, problem.field),
            (entry, field),
            "{shown}: {problem}"
        );
        assert!(!problem.message.is_empty());
    }
    // The name shape carries no width; the number shape needs one.
    let mut no_width = PrefixSpec::number("R", "x", 2);
    no_width.width = None;
    assert!(IdScheme::new(vec![no_width]).is_err());
    let mut named_width = PrefixSpec::name("T", "t");
    named_width.width = Some(2);
    assert!(IdScheme::new(vec![named_width]).is_err());
}

#[test]
fn from_toml_reads_only_ids_and_every_field() {
    let text = "\
[project]
name = \"x\"

[ids]
R    = { kind = \"requirement\", width = 2, immutable_text = true }
Q    = { kind = \"question\",    width = 3, aliases_from = [\"QST\", \"\u{0412}\u{041E}\u{041F}\"] }
AC   = { kind = \"criterion\",   width = 2, scope = \"feature\" }
TERM = { kind = \"term\",        shape = \"name\" }

[paths]
roots = [\"docs\"]
";
    let scheme = IdScheme::from_toml(text).expect("the spec's sample loads");
    let r = scheme.prefix("R").unwrap();
    assert_eq!(
        (r.kind.as_str(), r.width, r.immutable_text),
        ("requirement", Some(2), true)
    );
    let q = scheme.prefix("Q").unwrap();
    assert_eq!(q.aliases_from, ["QST", "\u{0412}\u{041E}\u{041F}"]);
    assert_eq!(
        scheme
            .alias("\u{0412}\u{041E}\u{041F}")
            .map(|s| s.prefix.as_str()),
        Some("Q")
    );
    assert_eq!(scheme.prefix("AC").unwrap().scope, IdScope::Feature);
    assert_eq!(scheme.prefix("R").unwrap().scope, IdScope::Project);
    let term = scheme.prefix("TERM").unwrap();
    assert_eq!((term.shape, term.width), (Shape::Name, None));
    // No [ids]: an empty scheme, nothing is a reference.
    let empty = IdScheme::from_toml("[project]\nname = \"x\"\n").unwrap();
    assert!(empty.is_empty());
    assert!(scan("R-12", 0, &empty).is_empty());
}

#[test]
fn from_toml_errors_name_the_line_and_load_nothing() {
    let cases: [(&str, usize); 11] = [
        (
            "[ids]\nR = { kind = \"r\", width = 2 }\nQ = { kind = \"q\", width = 2, script = \"latin\" }\n",
            3,
        ),
        ("[ids]\nR = { kind = \"r\" }\n", 2),
        (
            "[ids]\nR = { kind = \"r\", width = 2 }\nT = { kind = \"t\", shape = \"name\", width = 2 }\n",
            3,
        ),
        ("[ids]\nr = { kind = \"r\", width = 2 }\n", 2),
        (
            "[ids]\nR = { kind = \"r\", width = 2 }\n\nQ = { kind = \"q\", width = 2, aliases_from = [\"R\"] }\n",
            4,
        ),
        ("[ids]\nR = { kind = \"r\", width = 0 }\n", 2),
        (
            "[ids]\nR = { kind = \"r\", width = 2, shape = \"blob\" }\n",
            2,
        ),
        ("[ids]\nR = { width = 2 }\n", 2),
        (
            "[ids]\nR = { kind = \"r\", width = 2, scope = \"team\" }\n",
            2,
        ),
        // Two bad entries: the first written is reported, not the first by
        // name (P1 of docs/features/phase1-cleanup.md).
        (
            "[ids]\nZ = { kind = \"z\", width = 2, shape = \"blob\" }\nA = { kind = \"a\", width = 2, shape = \"blob\" }\n",
            2,
        ),
        ("[ids]\nZ = { kind = \"z\" }\nA = { kind = \"a\" }\n", 2),
    ];
    for (text, line) in cases {
        let error = IdScheme::from_toml(text).expect_err(text);
        assert_eq!(error.line, Some(line), "{text:?}: {error}");
        let shown = error.at("specengine.toml");
        assert!(
            shown.starts_with(&format!("specengine.toml:{line}: ")),
            "{text:?}: {shown}"
        );
    }
    // Broken TOML syntax names its line too.
    let error = IdScheme::from_toml("[ids]\nR = { kind = \n").expect_err("syntax");
    assert!(error.line.is_some(), "{error}");
}

/// AC-01 of docs/features/phase1-cleanup.md (M1): an alias match is never a
/// `homoglyph`, even when its body holds look-alike digits; the body is
/// still normalised into `id`. A configured prefix with look-alike digits
/// still is one.
#[test]
fn an_alias_with_look_alike_digits_is_no_homoglyph() {
    let scheme = scheme();
    let body = "-\u{FF10}\u{FF13}\u{FF11}";
    for alias in ["QST", "\u{0412}\u{041E}\u{041F}"] {
        let text = format!("See {alias}{body}.");
        let found = scan(&text, 0, &scheme);
        assert_eq!(found.len(), 1, "{alias}: one reference");
        let reference = &found[0].reference;
        assert_eq!(reference.alias_of.as_deref(), Some("Q"), "{alias}");
        assert_eq!(reference.id, format!("{alias}-031"), "{alias}: id");
        assert!(
            found[0].homoglyphs.is_empty(),
            "{alias}: no homoglyph: {:?}",
            found[0].homoglyphs
        );
        // Through the parser: no `homoglyph` diagnostic either.
        let parsed = specengine_core::parse("a.md", format!("{text}\n").as_bytes(), &scheme);
        assert!(
            parsed
                .diagnostics
                .iter()
                .all(|d| d.code != specengine_model::DiagnosticCode::Homoglyph),
            "{alias}: {:?}",
            parsed.diagnostics
        );
    }
    // The configured prefix with a look-alike digit is still a homoglyph.
    let found = scan("See Q-\u{FF10}31.", 0, &scheme);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].reference.id, "Q-031");
    assert_eq!(found[0].reference.alias_of, None);
    assert_eq!(found[0].homoglyphs.len(), 1);
    assert_eq!(found[0].homoglyphs[0].fix, "Q-031");
}

// ------------------------------------------------------------ is_slug
// docs/canon/spec-check-links.md, "API": `is_slug` is the grammar's
// `slug`, `[a-z][a-z0-9-]*`, one rule for the `project:` and `slug/`
// qualifiers and for the stem of a feature document (ADR-0026).

/// Candidate slugs: the grammar's, and every way to miss it.
const SLUG_CANDIDATES: &[(&str, bool)] = &[
    ("a", true),
    ("feat", true),
    ("stamina-tuning", true),
    ("dry-run", true),
    ("s-2", true),
    ("a1", true),
    ("a-", true),
    ("a--b", true),
    ("", false),
    ("Feat", false),
    ("fEat", false),
    ("README", false),
    ("1a", false),
    ("9slug", false),
    ("-a", false),
    ("a_b", false),
    ("a.b", false),
    ("a b", false),
    ("\u{00E9}t\u{00E9}", false),
    ("f\u{0435}at", false),
];

#[test]
fn is_slug_is_the_grammar_slug() {
    for &(text, want) in SLUG_CANDIDATES {
        assert_eq!(is_slug(text), want, "{text:?}");
    }
    assert!(!is_slug("a/b"), "one segment");
    assert!(!is_slug("a.md"), "a stem, not a file name");
}

/// The lexer's qualifiers are unchanged: after a clean boundary, `c/ID`
/// carries the scope `c` and `c:ID` the project `c` exactly when `c` is a
/// slug; otherwise the ID is read without the qualifier.
#[test]
fn qualifier_recognition_agrees_with_is_slug() {
    let scheme = scheme();
    for &(candidate, slug) in SLUG_CANDIDATES {
        // A space is a clean boundary: `a b/R-12` is the qualifier `b`.
        if candidate.is_empty() || candidate.contains(' ') {
            continue;
        }
        for (separator, project) in [('/', false), (':', true)] {
            let text = format!("({candidate}{separator}R-12)");
            let found = scan(&text, 0, &scheme);
            assert_eq!(found.len(), 1, "{text:?}");
            let reference = &found[0].reference;
            assert_eq!(reference.id, "R-12", "{text:?}");
            let (got, other) = if project {
                (&reference.project, &reference.scope)
            } else {
                (&reference.scope, &reference.project)
            };
            let want = slug.then(|| candidate.to_owned());
            assert_eq!(got, &want, "{text:?}");
            assert_eq!(other, &None, "{text:?}");
        }
    }
    // Both qualifiers, as before: `project:` then `slug/`.
    let found = scan("(shared:stamina-tuning/AC-07#AC-08@2)", 0, &scheme);
    let reference = &found[0].reference;
    assert_eq!(reference.project.as_deref(), Some("shared"));
    assert_eq!(reference.scope.as_deref(), Some("stamina-tuning"));
    assert_eq!(reference.id, "AC-07");
    assert_eq!(reference.section.as_deref(), Some("AC-08"));
    assert_eq!(reference.rev, Some(2));
    // A qualifier after a letter, digit, `_ - . / :` is no qualifier.
    for text in ["Xfeat/R-12", "_feat/R-12", "a.feat/R-12", "a/feat/R-12"] {
        let found = scan(text, 0, &scheme);
        assert_eq!(found.len(), 1, "{text:?}");
        assert_eq!(found[0].reference.scope, None, "{text:?}");
    }
}

/// docs/features/specengine-guide.md "Rules and edge cases": recognition
/// rule (7), a guide citation `guide:<topic>#<ID>` is no reference in any
/// project, its `<ID>` dropped whatever the project's `[ids]` hold. The
/// rows dropped have `guide:` standing alone (rule (5)'s left boundary), a
/// slug, `#` and the ID; the rows kept break one of those. `R` and `S` are
/// configured here, so every kept row's ID is a reference.
fn guide_scheme() -> IdScheme {
    IdScheme::new(vec![
        PrefixSpec::number("R", "requirement", 2),
        PrefixSpec::number("S", "section", 1),
    ])
    .expect("valid scheme")
}

/// Rule (7): the dropped rows give no reference; each kept row gives its one
/// ID, bare (no section, scope or project). M: the early return of
/// `cites_the_guide` removed (the dropped rows read their IDs).
#[test]
fn rule_seven_drops_the_guides_citations_alone() {
    let scheme = guide_scheme();
    let ids = |text: &str| -> Vec<String> {
        scan(text, 0, &scheme)
            .into_iter()
            .map(|found| found.reference.id)
            .collect()
    };
    for dropped in [
        "guide:x#R-12@3",
        "[[guide:x#R-1|l]]",
        "(guide:x-1#R-2)",
        "See guide:records#R-12 for the rule.",
    ] {
        assert_eq!(ids(dropped), Vec::<String>::new(), "dropped: {dropped:?}");
    }
    for (kept, id) in [
        ("guide: see #R-12", "R-12"),
        ("guide:#R-1", "R-1"),
        ("a-guide:x#R-1", "R-1"),
        ("myguide:x#R-1", "R-1"),
        ("foo:guide:x#R-1", "R-1"),
        ("https://h.io/guide:x#R-1", "R-1"),
        ("guide:Ab#R-1", "R-1"),
        ("guide:9x#R-1", "R-1"),
    ] {
        let found = scan(kept, 0, &scheme);
        assert_eq!(
            found
                .iter()
                .map(|found| found.reference.id.as_str())
                .collect::<Vec<_>>(),
            [id],
            "kept: {kept:?}"
        );
        let reference = &found[0].reference;
        assert_eq!(
            (
                reference.section.as_deref(),
                reference.scope.as_deref(),
                reference.project.as_deref()
            ),
            (None, None, None),
            "kept bare: {kept:?}"
        );
    }
}

/// One reference as the table and the RT9 rows read it: the ID, its
/// section, its revision and its span.
type Read = (String, Option<String>, Option<u32>, Option<Span>);

fn read_all(text: &str, scheme: &IdScheme) -> Vec<Read> {
    scan(text, 0, scheme)
        .into_iter()
        .map(|found| {
            let reference = found.reference;
            assert_eq!(
                (reference.scope.as_deref(), reference.project.as_deref()),
                (None, None),
                "no qualifier in {text:?}"
            );
            (
                reference.id,
                reference.section,
                reference.rev,
                reference.span,
            )
        })
        .collect()
}

fn bare(id: &str, start: usize) -> Read {
    (
        id.to_owned(),
        None,
        None,
        Some(Span::new(start, start + id.len())),
    )
}

/// docs/features/release-tail.md RT9 (a), the decided answer to rule (7)'s
/// edge (`specengine-guide` pinned `S-2` bare): when rule (7) fires, a `#ID`
/// and its `@rev` right after the citation's ID are part of the citation,
/// so `guide:x#R-1#S-2` and `guide:x#R-1#S-2@3` give no reference, `#ID`
/// then `@rev` joining as (3) joins them to a reference. The tail is
/// consumed only when rule (7) fires: `R-1#S-2`
/// is still `R-1`, section `S-2`; `.guide:x#R-1#S-2` (no citation, rule
/// (5)'s left boundary) is `R-1` with its section; a `#ID` after a space
/// (`guide:x#R-1 #S-2`) or a word after one (`guide:x#R-1 S-2`) keeps its
/// meaning, `S-2` bare. One `#ID` joins, as to a reference: a second one,
/// `guide:x#R-1#S-2#S-3`, reads `S-3` as `R-1#S-2#S-3` does. M: the skip
/// removed (the scan resuming right after the citation's ID).
#[test]
fn rt09_the_citations_tail_is_no_reference() {
    let scheme = guide_scheme();
    for dropped in [
        "guide:x#R-1",
        "guide:x#R-1#S-2",
        "guide:x#R-1#S-2@3",
        "guide:x#R-1@2",
        "See guide:records#R-12#S-2@3 for the rule.",
        "(guide:x#R-1#S-2)",
        "[[guide:x#R-1#S-2|the rule]]",
        "guide:x#R-1#S-2.",
    ] {
        assert_eq!(
            read_all(dropped, &scheme),
            Vec::<Read>::new(),
            "{dropped:?}"
        );
    }
    assert_eq!(
        read_all("R-1#S-2", &scheme),
        [(
            "R-1".to_owned(),
            Some("S-2".to_owned()),
            None,
            Some(Span::new(0, "R-1#S-2".len()))
        )],
        "a plain reference's section"
    );
    assert_eq!(
        read_all(".guide:x#R-1#S-2@3", &scheme),
        [(
            "R-1".to_owned(),
            Some("S-2".to_owned()),
            Some(3),
            Some(Span::new(".guide:x#".len(), ".guide:x#R-1#S-2@3".len()))
        )],
        "no citation: the reference keeps its section and revision"
    );
    for (text, id) in [
        ("guide:x#R-1 S-2", "S-2"),
        ("guide:x#R-1 #S-2", "S-2"),
        ("guide:x#R-1, S-2", "S-2"),
        ("guide:x#R-1#S-2 S-2", "S-2"),
    ] {
        let start = text.rfind(id).expect("the kept ID");
        assert_eq!(read_all(text, &scheme), [bare(id, start)], "{text:?}");
    }
    // One `#ID` joins, as to a reference: the developer's probe of
    // 2026-11-20, pinned beside the plain reference it mirrors.
    let plain = read_all("R-1#S-2#S-3", &scheme);
    assert_eq!(
        plain.iter().map(|read| read.0.as_str()).collect::<Vec<_>>(),
        ["R-1", "S-3"],
        "premise: a plain reference's second `#`: {plain:?}"
    );
    assert_eq!(
        read_all("guide:x#R-1#S-2#S-3", &scheme),
        [bare("S-3", "guide:x#R-1#S-2#".len())]
    );

    // The tail is read by shape, not by the project's `[ids]`: with `OLD`
    // an `aliases_from` prefix of `S`, `guide:x#R-1#OLD-5` (and a prefix no
    // entry knows, `ZZZ`) gives no reference, while the plain `R-1#OLD-5`
    // keeps rule (3)'s answer: `R-1` bare (a legacy alias joins as no
    // section), then `OLD-5`, the alias of `S`.
    let aliased = IdScheme::new(vec![
        PrefixSpec::number("R", "requirement", 2),
        PrefixSpec::number("S", "section", 1).with_aliases(["OLD"]),
    ])
    .expect("valid scheme");
    for dropped in [
        "guide:x#R-1#OLD-5",
        "guide:x#R-1#OLD-5@2",
        "guide:x#R-1#ZZZ-5",
    ] {
        assert_eq!(
            read_all(dropped, &aliased),
            Vec::<Read>::new(),
            "{dropped:?}"
        );
    }
    let plain = scan("R-1#OLD-5", 0, &aliased);
    assert_eq!(
        plain
            .iter()
            .map(|found| {
                let reference = &found.reference;
                (
                    reference.id.as_str(),
                    reference.section.as_deref(),
                    reference.alias_of.as_deref(),
                    reference.span,
                )
            })
            .collect::<Vec<_>>(),
        [
            ("R-1", None, None, Some(Span::new(0, 3))),
            ("OLD-5", None, Some("S"), Some(Span::new(4, 9))),
        ],
        "the plain reference's rule (3) answer"
    );
}

/// One adversarial input of [`rt09_the_scan_stays_one_pass`]: its label,
/// its text at `n` copies, and the references found in it.
type Adversary = (&'static str, fn(usize) -> String, fn(usize) -> usize);

/// RT9: "The scan stays one pass". Three adversarial shapes, each at 1 000
/// and at 16 000 copies: citations with their tails, one citation whose ID
/// runs on for the whole input, and one citation followed by sections. The
/// larger input costs under 64 times the smaller (linear is 16, a rescan
/// per citation 256: four times of room either side, for a loaded
/// machine), the smaller the best of seven and floored at 200 us, the
/// larger passing in one of three tries; the answer is the expected count
/// (after the citation's own `#S-2`, each `S-2#S-2` is one reference). M:
/// the tail read by a rescan from the text's start.
#[test]
fn rt09_the_scan_stays_one_pass() {
    let scheme = guide_scheme();
    let shapes: [Adversary; 3] = [
        (
            "citations with tails",
            |n| "guide:x#R-1#S-2@3 ".repeat(n),
            |_| 0,
        ),
        (
            "one ID running on",
            |n| format!("guide:x#R{}", "-1".repeat(n)),
            |_| 0,
        ),
        (
            "one citation, then sections",
            |n| format!("guide:x#R-1{}", "#S-2".repeat(n)),
            |n| n / 2,
        ),
    ];
    let time = |text: &str| {
        let at = std::time::Instant::now();
        let found = scan(text, 0, &scheme).len();
        (at.elapsed(), found)
    };
    for (label, make, count) in shapes {
        let small_text = make(1_000);
        let (small, found) = (0..7).map(|_| time(&small_text)).min().expect("seven runs");
        assert_eq!(found, count(1_000), "{label}: 1 000");
        let bound = small.max(std::time::Duration::from_micros(200)) * 64;
        let large_text = make(16_000);
        let mut tries = Vec::new();
        for _ in 0..3 {
            let (took, found) = time(&large_text);
            assert_eq!(found, count(16_000), "{label}: 16 000");
            tries.push(took);
            if took < bound {
                break;
            }
        }
        assert!(
            tries.last().is_some_and(|took| *took < bound),
            "{label}: 16 times the input took {tries:?} against {small:?}"
        );
    }
}

/// The shared table's file, the model's copy (RT10).
fn shared_table() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/guide-citations.json")
}

/// The UI's copy, read by its `markdown/guide` test through Vite's `?raw`.
fn ui_table() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/src/markdown/guide-citations.json")
}

/// docs/features/release-tail.md RT10: the UI's tests read only inside
/// `ui/`, so the table lives at `ui/src/markdown/guide-citations.json` and
/// the model's copy, `fixtures/guide-citations.json`, is the same bytes.
/// M: a row edited on one side.
#[test]
fn rt10_the_two_copies_of_the_table_are_one() {
    let model = std::fs::read(shared_table()).expect("fixtures/guide-citations.json");
    let ui = std::fs::read(ui_table()).expect("ui/src/markdown/guide-citations.json");
    assert!(
        model == ui,
        "fixtures/guide-citations.json and ui/src/markdown/guide-citations.json differ"
    );
}

/// RT10 (b), the Rust half: every row of the shared table `{text, refs,
/// guide_link}` scans to exactly its `refs` (IDs, in order, under `[ids]` R
/// and S), and the row's `guide_link` agrees with the grammar: where Rust
/// keeps `R-1` a reference the UI links no guide, and where Rust reads the
/// citation `guide:x#R-1` (R-1 no reference) standing alone on its right
/// (the end, or a char rule (4) ends an ID at that no `#ID` or `@rev`
/// follows) the UI links it. The table covers `#`, `@`, `.`, `:`, `/`, `-`
/// and `(` before `guide`, and the RT9 forms, each text once. M: the old
/// left boundary (a row's `refs` no longer what Rust reads).
#[test]
fn rt10_both_sides_read_the_table_alike() {
    let scheme = guide_scheme();
    let text = std::fs::read_to_string(shared_table()).expect("the shared table");
    let rows: Vec<serde_json::Value> = serde_json::from_str(&text).expect("a JSON list");
    let mut seen = std::collections::BTreeSet::new();
    let mut linked = 0;
    let mut kept = 0;
    for row in &rows {
        let object = row.as_object().expect("a row is an object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<Vec<_>>(),
            ["guide_link", "refs", "text"],
            "exactly {{text, refs, guide_link}}: {row}"
        );
        let text = row["text"].as_str().expect("text");
        let refs: Vec<&str> = row["refs"]
            .as_array()
            .expect("refs")
            .iter()
            .map(|id| id.as_str().expect("an ID"))
            .collect();
        let link = row["guide_link"].as_bool().expect("guide_link");
        assert!(seen.insert(text.to_owned()), "{text:?} twice");
        let read: Vec<String> = scan(text, 0, &scheme)
            .into_iter()
            .map(|found| found.reference.id)
            .collect();
        assert_eq!(read, refs, "{text:?}: the IDs Rust keeps");
        if refs.contains(&"R-1") {
            kept += 1;
            assert!(
                !link,
                "{text:?}: R-1 a reference, yet the UI links the guide"
            );
        }
        if let Some(at) = text.find("guide:x#R-1") {
            let after = &text[at + "guide:x#R-1".len()..];
            let mut chars = after.chars();
            let alone = match chars.next() {
                None => true,
                Some(c) if c.is_alphanumeric() || c == '_' => false,
                Some('-') | Some('#') => !chars.next().is_some_and(char::is_alphanumeric),
                Some('@') => !chars.next().is_some_and(|c| c.is_ascii_digit()),
                Some(_) => true,
            };
            if !refs.contains(&"R-1") && alone {
                linked += 1;
                assert!(
                    link,
                    "{text:?}: a citation standing alone, yet no guide link"
                );
            }
        }
    }
    assert!(
        kept > 0 && linked > 0,
        "both kinds of row: {kept} kept, {linked} linked"
    );
    for before in ['#', '@', '.', ':', '/', '-', '('] {
        assert!(
            seen.iter()
                .any(|text| text.starts_with(&format!("{before}guide:x#R-1"))),
            "a row with {before:?} before guide"
        );
    }
    for form in [
        "guide:x#R-1",
        "guide:x#R-1#S-2",
        "guide:x#R-1#S-2@3",
        "guide:x#R-1 S-2",
        "R-1#S-2",
    ] {
        assert!(seen.contains(form), "the RT9 form {form:?}");
    }
}
