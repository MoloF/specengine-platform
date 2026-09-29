//! AC-05 and AC-06 of docs/features/spec-parser.md, plus the front-matter
//! edge data: typed keys, unknown keys kept in `extra`, invalid YAML reported
//! at its file line without a guessed ID while the body is still read, and
//! every other front-matter error named with its line — never a panic.

mod common;

use specengine_model::{
    CanonTarget, DiagnosticCode, FmValue, IdScheme, ParsedFile, PrefixSpec, Severity,
};

use common::{corpus_scheme, fixture, md_files, parse_str, variants};

fn spec_a() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

/// The 05 §2.2 question record, verbatim from `fixtures/spec-a`.
fn question_record() -> String {
    let (_, bytes) = md_files(&fixture("spec-a"))
        .into_iter()
        .find(|(path, _)| path.ends_with("Q-031.md"))
        .expect("Q-031.md");
    String::from_utf8(bytes).expect("UTF-8")
}

fn codes(parsed: &ParsedFile) -> Vec<(String, usize)> {
    parsed
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str().to_owned(), d.line))
        .collect()
}

fn ids(parsed: &ParsedFile) -> Vec<Option<String>> {
    parsed.nodes.iter().map(|n| n.id.clone()).collect()
}

// ------------------------------------------------------------------- AC-05

#[test]
fn question_record_yields_typed_keys() {
    let parsed = parse_str(&question_record(), &spec_a());
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let document = parsed.document().unwrap();
    assert_eq!(document.id.as_deref(), Some("Q-031"));
    assert_eq!(document.kind.as_deref(), Some("question"));
    let fields = document.fields.as_ref().expect("a document has fields");
    // YAML comments after the values are not part of them.
    assert_eq!(fields.status.as_deref(), Some("open"));
    assert_eq!(fields.to.as_deref(), Some("customer"));
    assert_eq!(fields.severity.as_deref(), Some("normal"));
    let working = fields.working_answer.as_ref().expect("working_answer");
    assert_eq!(working.id, "A-101");
    let refs: Vec<&str> = fields
        .refs
        .as_ref()
        .expect("refs")
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(refs, ["R-12", "MEC-STAMINA"]);
    let raised_by = fields
        .raised_by
        .as_ref()
        .expect("raised_by is a generic map");
    let keys: Vec<&str> = raised_by.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["agent", "run", "model"], "source order");
    assert_eq!(document.extra.as_deref(), Some(&[][..]), "no untyped key");
}

#[test]
fn unknown_key_stays_in_extra_with_one_warning_and_no_error() {
    let record = question_record().replacen("severity:", "x_custom: 1\nseverity:", 1);
    let parsed = parse_str(&record, &spec_a());
    let unknown: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::UnknownKey)
        .collect();
    assert_eq!(unknown.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(unknown[0].line, 7, "the x_custom line");
    assert_eq!(unknown[0].severity, Severity::Warning);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let document = parsed.document().unwrap();
    let extra = document.extra.as_ref().expect("extra");
    assert_eq!(extra.len(), 1);
    assert_eq!(extra[0].key, "x_custom");
    assert_eq!(extra[0].value, FmValue::Int(1));
    // The typed keys around it are still read.
    assert_eq!(document.id.as_deref(), Some("Q-031"));
    let fields = document.fields.as_ref().unwrap();
    assert_eq!(fields.severity.as_deref(), Some("normal"));
    assert_eq!(fields.working_answer.as_ref().unwrap().id, "A-101");
    assert_eq!(fields.refs.as_ref().unwrap().len(), 2);
}

#[test]
fn unknown_keys_keep_source_order_and_mistyped_values_go_to_extra() {
    let text =
        "---\nzeta: [1, 2]\nid: R-12\ntier: two\nalpha: {b: 1, a: x}\nscope: movement\n---\n";
    let parsed = parse_str(text, &spec_a());
    let document = parsed.document().unwrap();
    assert_eq!(document.id.as_deref(), Some("R-12"));
    let keys: Vec<&str> = document
        .extra
        .as_ref()
        .unwrap()
        .iter()
        .map(|e| e.key.as_str())
        .collect();
    assert_eq!(keys, ["zeta", "tier", "alpha", "scope"], "source order");
    assert_eq!(
        codes(&parsed),
        [
            ("unknown-key".to_owned(), 2),
            ("frontmatter-type".to_owned(), 4),
            ("unknown-key".to_owned(), 5),
            ("frontmatter-type".to_owned(), 6),
        ]
    );
    assert_eq!(document.fields.as_ref().unwrap().tier, None);
}

// ------------------------------------------------------------------- AC-06

/// Line 3 holds `title: Hybrid: revision`: a plain scalar with `: `.
const INVALID: &str = "---\nid: X-1\ntitle: Hybrid: revision\nstatus: accepted\n---\n\n# Hybrid\n\n## Marker {#X-2}\n\nText of X-2.\n\n## Lock {#X-3}\n\nText of X-3, see X-2.\n";

fn x_scheme() -> IdScheme {
    IdScheme::new(vec![PrefixSpec::number("X", "x", 1)]).unwrap()
}

#[test]
fn invalid_yaml_is_one_diagnostic_at_its_file_line_and_the_body_is_read() {
    for (variant, bytes) in variants(INVALID.as_bytes()) {
        let parsed = specengine_core::parse("x.md", &bytes, &x_scheme());
        assert_eq!(
            codes(&parsed),
            [("frontmatter-yaml".to_owned(), 3)],
            "[{variant}] {:?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].severity, Severity::Error);
        assert!(
            parsed.front_matter.is_some(),
            "[{variant}] the block is still located"
        );
        let document = parsed
            .document()
            .unwrap_or_else(|| panic!("[{variant}] document"));
        assert_eq!(document.id, None, "[{variant}] no guessed ID");
        assert_eq!(document.kind, None, "[{variant}] no guessed kind");
        let fields = document.fields.as_ref().unwrap();
        assert_eq!(fields.status, None, "[{variant}] no value is kept");
        assert_eq!(
            document.title.as_deref(),
            Some("Hybrid"),
            "[{variant}] title from the H1"
        );
        assert_eq!(
            ids(&parsed),
            [None, Some("X-2".into()), Some("X-3".into())],
            "[{variant}] {{#ID}} sections present"
        );
        // Mentions are read; their source is the enclosing section.
        let mentions: Vec<String> = parsed.links.iter().map(common::render_link).collect();
        assert_eq!(
            mentions,
            [
                "X-2 mentions inline X-2",
                "X-3 mentions inline X-3",
                "X-3 mentions inline X-2"
            ],
            "[{variant}]"
        );
    }
}

#[test]
fn five_valid_siblings_are_clean() {
    let siblings = [
        INVALID.replace("title: Hybrid: revision", "title: \"Hybrid: revision\""),
        INVALID.replace("title: Hybrid: revision", "title: 'Hybrid: revision'"),
        INVALID.replace("title: Hybrid: revision", "title: Hybrid revision"),
        INVALID.replace("title: Hybrid: revision", "title: >-\n  Hybrid: revision"),
        INVALID.replace("title: Hybrid: revision\n", ""),
    ];
    for (index, text) in siblings.iter().enumerate() {
        let parsed = parse_str(text, &x_scheme());
        assert!(
            parsed.diagnostics.is_empty(),
            "sibling {index}: {:?}",
            parsed.diagnostics
        );
        let document = parsed.document().unwrap();
        assert_eq!(document.id.as_deref(), Some("X-1"), "sibling {index}");
        assert_eq!(parsed.sections().len(), 2, "sibling {index}");
        if index < 4 {
            assert_eq!(
                document.title.as_deref().map(|t| t.contains("Hybrid")),
                Some(true)
            );
        }
    }
}

#[test]
fn yaml_error_deep_in_the_block_names_that_line() {
    let text = "---\nid: X-1\nstatus: open\nrefs:\n  - X-2\n  - [unclosed\nowner: me\n---\n\n## A {#X-2}\n";
    let parsed = parse_str(text, &x_scheme());
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let diagnostic = &parsed.diagnostics[0];
    assert_eq!(diagnostic.code, DiagnosticCode::FrontmatterYaml);
    assert!(
        (6..=8).contains(&diagnostic.line),
        "the error is reported inside the block near line 6, got {}",
        diagnostic.line
    );
    assert!(!diagnostic.message.is_empty());
    assert_eq!(ids(&parsed), [None, Some("X-2".into())]);
}

#[test]
fn duplicate_key_is_a_yaml_error_not_a_silent_overwrite() {
    let parsed = parse_str("---\nid: X-1\nid: X-2\n---\n", &x_scheme());
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        [DiagnosticCode::FrontmatterYaml]
    );
    assert_eq!(parsed.document().unwrap().id, None);
}

// ------------------------------------------------------------ other layouts

#[test]
fn unclosed_front_matter_makes_the_whole_file_body() {
    let text = "---\nid: X-1\n\n# Title\n\n## A {#X-2}\n";
    let parsed = parse_str(text, &x_scheme());
    assert_eq!(codes(&parsed), [("frontmatter-unclosed".to_owned(), 1)]);
    assert_eq!(parsed.front_matter, None);
    assert_eq!(parsed.body.start, 0);
    assert_eq!(parsed.body.end, text.len());
    assert_eq!(ids(&parsed), [None, Some("X-2".into())]);
}

#[test]
fn fences_must_be_exactly_three_dashes() {
    for text in [
        "--- \nid: X-1\n---\n",
        "----\nid: X-1\n----\n",
        "\n---\nid: X-1\n---\n",
        " ---\nid: X-1\n---\n",
    ] {
        let parsed = parse_str(text, &x_scheme());
        assert_eq!(parsed.front_matter, None, "{text:?} opens no front-matter");
        assert_eq!(parsed.document().unwrap().id, None, "{text:?}");
    }
    // A closing fence with trailing text does not close the block.
    let parsed = parse_str("---\nid: X-1\n--- x\n", &x_scheme());
    assert_eq!(codes(&parsed), [("frontmatter-unclosed".to_owned(), 1)]);
    // `...` is no closing fence either.
    let parsed = parse_str("---\nid: X-1\n...\n", &x_scheme());
    assert_eq!(codes(&parsed), [("frontmatter-unclosed".to_owned(), 1)]);
}

#[test]
fn empty_and_non_mapping_front_matter() {
    let empty = parse_str("---\n---\n# T\n", &x_scheme());
    assert!(empty.diagnostics.is_empty(), "{:?}", empty.diagnostics);
    assert_eq!(empty.front_matter.map(|s| (s.start, s.end)), Some((0, 8)));
    assert_eq!(empty.document().unwrap().title.as_deref(), Some("T"));

    for (text, line) in [("---\n- a\n- b\n---\n", 2), ("---\njust text\n---\n", 2)] {
        let parsed = parse_str(text, &x_scheme());
        assert_eq!(
            codes(&parsed),
            [("frontmatter-not-mapping".to_owned(), line)],
            "{text:?}"
        );
    }
}

#[test]
fn wrong_types_are_reported_with_their_lines_and_kept_raw() {
    let text =
        "---\nid: 42\ntitle: [a]\nrev: x\ntier: 2\nscope: [a, 3]\nrefs: X-1\nlinks: [X-2]\n---\n";
    let parsed = parse_str(text, &x_scheme());
    let got = codes(&parsed);
    for (code, line) in [
        ("frontmatter-type", 2),
        ("frontmatter-type", 3),
        ("frontmatter-type", 4),
        ("frontmatter-type", 6),
        ("frontmatter-type", 7),
        ("frontmatter-type", 8),
    ] {
        assert!(
            got.contains(&(code.to_owned(), line)),
            "{code} at line {line} missing from {got:?}"
        );
    }
    assert!(
        got.iter().all(|(code, _)| code == "frontmatter-type"),
        "{got:?}"
    );
    let document = parsed.document().unwrap();
    assert_eq!(document.id, None);
    assert_eq!(document.fields.as_ref().unwrap().tier, Some(2));
    let raw: Vec<&str> = document
        .extra
        .as_ref()
        .unwrap()
        .iter()
        .map(|e| e.key.as_str())
        .collect();
    assert_eq!(raw, ["id", "title", "rev", "scope", "refs", "links"]);
}

#[test]
fn id_outside_the_scheme_leaves_the_document_without_an_id() {
    for (text, line) in [
        ("---\nid: ZZ-1\n---\n", 2),
        ("---\nkind: x\nid: X-1@2\n---\n", 3),
        ("---\nid: s/X-1\n---\n", 2),
        ("---\nid: [[X-1]]\n---\n", 2),
    ] {
        let parsed = parse_str(text, &x_scheme());
        let document = parsed.document().unwrap();
        assert_eq!(document.id, None, "{text:?}");
        let got = codes(&parsed);
        assert!(
            got.contains(&("id-not-in-scheme".to_owned(), line))
                || got.contains(&("frontmatter-type".to_owned(), line)),
            "{text:?}: {got:?}"
        );
    }
    // An alias is never a definition.
    let scheme = IdScheme::new(vec![
        PrefixSpec::number("Q", "question", 3).with_aliases(["QST"]),
    ])
    .unwrap();
    let parsed = parse_str("---\nid: QST-7\n---\n", &scheme);
    assert_eq!(parsed.document().unwrap().id, None);
    assert_eq!(codes(&parsed), [("id-not-in-scheme".to_owned(), 2)]);
}

#[test]
fn reference_keys_must_hold_exactly_one_reference() {
    let text = "---\nid: X-1\nparent: X-2 and X-3\nworking_answer: see X-4\nsupersedes: [X-5, nonsense]\n---\n";
    let parsed = parse_str(text, &x_scheme());
    let got = codes(&parsed);
    assert!(
        got.contains(&("unparsed-reference".to_owned(), 3)),
        "{got:?}"
    );
    assert!(
        got.contains(&("unparsed-reference".to_owned(), 4)),
        "{got:?}"
    );
    assert!(
        got.contains(&("unparsed-reference".to_owned(), 5)),
        "{got:?}"
    );
    let document = parsed.document().unwrap();
    assert_eq!(document.parent, None);
    assert_eq!(document.fields.as_ref().unwrap().working_answer, None);
}

#[test]
fn canon_is_a_reference_else_a_path_with_an_anchor() {
    let parsed = parse_str("---\nid: X-1\ncanon: X-2#X-3\n---\n", &x_scheme());
    match &parsed.document().unwrap().fields.as_ref().unwrap().canon {
        Some(CanonTarget::Reference(r)) => {
            assert_eq!((r.id.as_str(), r.section.as_deref()), ("X-2", Some("X-3")));
        }
        other => panic!("canon X-2#X-3 is a reference, got {other:?}"),
    }
    let parsed = parse_str(
        "---\nid: X-1\ncanon: docs/canon/architecture.md#layout\n---\n",
        &x_scheme(),
    );
    match &parsed.document().unwrap().fields.as_ref().unwrap().canon {
        Some(CanonTarget::Path(p)) => {
            assert_eq!(p.path, "docs/canon/architecture.md");
            assert_eq!(p.anchor.as_deref(), Some("layout"));
        }
        other => panic!("canon path#anchor, got {other:?}"),
    }
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn document_rev_and_bad_rev() {
    let parsed = parse_str("---\nid: X-1\nrev: 3\n---\n", &x_scheme());
    assert_eq!(parsed.document().unwrap().rev, Some(3));
    assert!(parsed.diagnostics.is_empty());
    let parsed = parse_str("---\nid: X-1\nrev: 1234567890\n---\n", &x_scheme());
    assert_eq!(parsed.document().unwrap().rev, None);
    assert_eq!(codes(&parsed), [("bad-rev".to_owned(), 3)]);
}

#[test]
fn empty_file_is_a_document_without_anything() {
    let parsed = parse_str("", &x_scheme());
    assert_eq!(parsed.nodes.len(), 1);
    assert!(parsed.diagnostics.is_empty());
    let document = parsed.document().unwrap();
    assert_eq!(document.tokens_est, 0);
    assert_eq!(document.id, None);
    assert_eq!(document.summary, None);
}

// ------------------------------------------ docs/features/phase1-cleanup.md

/// Every JSON object in `json`, at any depth, as its key lists in written
/// order (a repeated key stays visible, unlike `serde_json::Value`).
fn object_keys(json: &str) -> Vec<Vec<String>> {
    use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

    struct Walk<'a>(&'a mut Vec<Vec<String>>);

    impl<'de> DeserializeSeed<'de> for Walk<'_> {
        type Value = ();
        fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
            deserializer.deserialize_any(self)
        }
    }

    impl<'de> Visitor<'de> for Walk<'_> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("any JSON value")
        }
        fn visit_bool<E>(self, _: bool) -> Result<(), E> {
            Ok(())
        }
        fn visit_i64<E>(self, _: i64) -> Result<(), E> {
            Ok(())
        }
        fn visit_u64<E>(self, _: u64) -> Result<(), E> {
            Ok(())
        }
        fn visit_f64<E>(self, _: f64) -> Result<(), E> {
            Ok(())
        }
        fn visit_str<E>(self, _: &str) -> Result<(), E> {
            Ok(())
        }
        fn visit_unit<E>(self) -> Result<(), E> {
            Ok(())
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            while seq.next_element_seed(Walk(&mut *self.0))?.is_some() {}
            Ok(())
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            let mut keys = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                keys.push(key);
                map.next_value_seed(Walk(&mut *self.0))?;
            }
            self.0.push(keys);
            Ok(())
        }
    }

    let mut out = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_str(json);
    Walk(&mut out)
        .deserialize(&mut deserializer)
        .expect("valid JSON");
    out
}

/// No JSON object of the parse repeats a key.
fn assert_no_repeated_json_key(case: &str, parsed: &ParsedFile) {
    // The walk sees a repeated key (`serde_json::Value` would hide it).
    assert_eq!(
        object_keys(r#"{"a":1,"a":{"b":[{"c":2}]}}"#),
        [vec!["c"], vec!["b"], vec!["a", "a"]]
    );
    for keys in object_keys(&common::json(parsed)) {
        let unique: std::collections::BTreeSet<&String> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len(), "{case}: repeated key in {keys:?}");
    }
}

fn extra_value<'p>(parsed: &'p ParsedFile, key: &str) -> &'p FmValue {
    &parsed
        .document()
        .and_then(|d| d.extra.as_ref())
        .and_then(|extra| extra.iter().find(|e| e.key == key))
        .unwrap_or_else(|| panic!("`{key}` in extra: {parsed:#?}"))
        .value
}

/// AC-04 (P3): NaN and infinities, in any YAML spelling or as an
/// overflowing literal, are the strings `.nan`, `.inf`, `-.inf`; every
/// float left is finite, and the parse survives a JSON round trip.
#[test]
fn non_finite_floats_are_strings_and_round_trip_through_json() {
    let cases = [
        ("a_nan", ".nan", ".nan"),
        ("a_nan_cap", ".NaN", ".nan"),
        ("a_inf", ".inf", ".inf"),
        ("a_inf_plus", "+.inf", ".inf"),
        ("a_inf_upper", ".INF", ".inf"),
        ("a_neg_inf", "-.inf", "-.inf"),
        ("a_neg_inf_cap", "-.Inf", "-.inf"),
        ("a_big", "1e999", ".inf"),
        ("a_neg_big", "-1e999", "-.inf"),
    ];
    let mut text = String::from("---\nid: R-12\n");
    for (key, written, _) in cases {
        text.push_str(&format!("{key}: {written}\n"));
    }
    text.push_str("a_seq: [.nan, 1.5, -.inf]\na_map: {k: .inf}\nrev: .nan\n---\n# R\n");
    let parsed = parse_str(&text, &spec_a());
    for (key, written, want) in cases {
        assert_eq!(
            extra_value(&parsed, key),
            &FmValue::Str(want.to_owned()),
            "{key}: {written}"
        );
    }
    let s = |text: &str| FmValue::Str(text.to_owned());
    assert_eq!(
        extra_value(&parsed, "a_seq"),
        &FmValue::Seq(vec![s(".nan"), FmValue::Float(1.5), s("-.inf")])
    );
    match extra_value(&parsed, "a_map") {
        FmValue::Map(map) => assert_eq!(map.get("k"), Some(&s(".inf"))),
        other => panic!("a_map: {other:?}"),
    }
    // A mistyped typed key is kept in `extra` the same way.
    assert_eq!(extra_value(&parsed, "rev"), &s(".nan"));
    let json = common::json(&parsed);
    let back: ParsedFile = serde_json::from_str(&json).expect("the parse reads back");
    assert_eq!(back, parsed, "JSON round trip");
    assert_eq!(common::json(&back), json);
}

/// AC-05 (P4): every map the parser builds holds each key text once; an
/// entry repeating a key text, or with a collection as its key, is dropped
/// with one `frontmatter-type` at its line, the first entry stays. The
/// top-level `extra` is a list: `1:` and `"1":` both stay.
#[test]
fn repeated_or_collection_keys_are_dropped_once_each_and_the_first_stays() {
    let text = "---\nid: R-12\nx: {1: a, \"1\": b}\nraised_by: {1: a, \"1\": b}\nlinks: {1: [R-01], \"1\": [R-02]}\nz:\n  ? [k]\n  : a\n  c: d\nw: {m: {2: p, \"2\": q, 3: r}}\n---\n# R\n";
    let parsed = parse_str(text, &spec_a());
    let s = |text: &str| FmValue::Str(text.to_owned());
    let one = |key: &str, value: FmValue| {
        let mut map = specengine_model::OrderedMap::new();
        map.push(key, value);
        FmValue::Map(map)
    };
    assert_eq!(extra_value(&parsed, "x"), &one("1", s("a")));
    assert_eq!(extra_value(&parsed, "z"), &one("c", s("d")));
    assert_eq!(
        extra_value(&parsed, "w"),
        &one("m", {
            let mut map = specengine_model::OrderedMap::new();
            map.push("2", s("p"));
            map.push("3", s("r"));
            FmValue::Map(map)
        })
    );
    let fields = parsed.document().unwrap().fields.as_ref().unwrap();
    let raised_by = fields.raised_by.as_ref().expect("raised_by typed");
    assert_eq!(
        raised_by.iter().collect::<Vec<_>>(),
        [("1", &s("a"))],
        "raised_by: the first entry stays"
    );
    // `links` with a non-string key is mistyped, kept in `extra`, and its
    // repeated key is dropped there.
    assert_eq!(fields.links, None);
    assert_eq!(
        extra_value(&parsed, "links"),
        &one("1", FmValue::Seq(vec![s("R-01")]))
    );
    let dropped: Vec<(usize, &str)> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::FrontmatterType && d.message.contains("dropped"))
        .map(|d| (d.line, d.message.as_str()))
        .collect();
    assert_eq!(
        dropped.iter().map(|(line, _)| *line).collect::<Vec<_>>(),
        [3, 4, 5, 7, 10],
        "one per dropped entry at its line: {:#?}",
        parsed.diagnostics
    );
    assert_eq!(
        codes(&parsed),
        [
            ("unknown-key".to_owned(), 3),
            ("frontmatter-type".to_owned(), 3),
            ("frontmatter-type".to_owned(), 4),
            ("frontmatter-type".to_owned(), 5),
            ("frontmatter-type".to_owned(), 5),
            ("unknown-key".to_owned(), 6),
            ("frontmatter-type".to_owned(), 7),
            ("unknown-key".to_owned(), 10),
            ("frontmatter-type".to_owned(), 10),
        ],
        "{:#?}",
        parsed.diagnostics
    );
    assert_no_repeated_json_key("nested", &parsed);

    // The top level is a list: both entries stay, `unknown-key` each.
    let parsed = parse_str("---\n1: a\n\"1\": b\n---\n", &spec_a());
    let extra = parsed.document().unwrap().extra.as_ref().expect("extra");
    assert_eq!(
        extra
            .iter()
            .map(|e| (e.key.as_str(), &e.value))
            .collect::<Vec<_>>(),
        [("1", &s("a")), ("1", &s("b"))]
    );
    assert_eq!(
        codes(&parsed),
        [("unknown-key".to_owned(), 2), ("unknown-key".to_owned(), 3)]
    );
    assert_no_repeated_json_key("top level", &parsed);
}
