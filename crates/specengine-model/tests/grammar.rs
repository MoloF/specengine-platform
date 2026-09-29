//! AC-09 of docs/features/spec-parser.md: the one reference grammar.
//! Table-driven splits of every "Reference grammar" example and of
//! `[[R-12|label]]`; `canon:` as path + anchor; `status: superseded-by X` as
//! a `supersedes` link; `#` joins only before an ID (`ADR-0002#layout` is
//! `ADR-0002`). Plus the `[ids]` scheme rules of the "Data" section:
//! validation of prefixes, widths, shapes and aliases, and `file:line`
//! errors of `IdScheme::from_toml`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

use specengine_core::IdSchemeToml;
use specengine_model::grammar::{
    Canon, parse_canon, parse_definition, parse_reference, scan, split_superseded_by,
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
    let cases: [(&str, usize); 9] = [
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
