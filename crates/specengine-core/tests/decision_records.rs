//! docs/features/decision-apply.md, core's half ("Data"): the
//! `[decision_records]` table of `specengine.toml`, checked on load at its
//! line; the template (a front-matter first, slots known, free text in the
//! body but the title); the one-pass render; the record's title, ID and
//! numbering; the owner's choice as one-key JSON; the characters a record
//! never carries; the structure a rendered record must have. Over the
//! fixtures' own tables and committed templates (spec-a: a game, `DEC`;
//! spec-b: a command-line tool, `ADR`), pure functions only.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::fs;

use common::{corpus_scheme, fixture, walked_md_files};
use serde_json::json;
use specengine_core::record::{
    Choice, DecisionRecords, RecordDefect, Slot, SlotValues, TEMPLATE_MAX_BYTES, TITLE_MAX_BYTES,
    Template, canon_text, highest_record_number, is_escaped, record_defect, record_id,
    record_number, record_title, refused_char,
};
use specengine_core::{Paths, ProjectConfig};
use specengine_model::{IdScheme, ParsedFile};

/// `(fixture, prefix, dir)` of the fixtures' tables.
const TABLES: [(&str, &str, &str); 2] = [
    ("spec-a", "DEC", "docs/records/DEC"),
    ("spec-b", "ADR", "docs/records/ADR"),
];

fn config_text(name: &str) -> String {
    fs::read_to_string(fixture(name).join("specengine.toml")).expect("the fixture's config")
}

fn template_of(name: &str) -> String {
    fs::read_to_string(fixture(name).join("templates/decision.md")).expect("the template")
}

/// The 1-based line of the first line of `text` starting with `start`.
fn line_of(text: &str, start: &str) -> usize {
    text.lines()
        .position(|line| line.trim_start().starts_with(start))
        .unwrap_or_else(|| panic!("no line starts with {start:?}"))
        + 1
}

/// spec-a's config with its `[decision_records]` table's lines replaced by
/// `table` (the header kept).
fn with_table(table: &str) -> String {
    let config = config_text("spec-a");
    let at = config.find("[decision_records]\n").expect("the table");
    format!("{}[decision_records]\n{table}", &config[..at])
}

fn values() -> SlotValues {
    SlotValues {
        id: "DEC-0024".to_owned(),
        date: "2026-10-05".to_owned(),
        status: "accepted".to_owned(),
        canon: "RULE-STAM-REGEN".to_owned(),
        targets: vec!["RULE-STAM-REGEN".to_owned(), "Q-031".to_owned()],
        proposal: "PR-0001".to_owned(),
        title: "Change the spec".to_owned(),
        choice: "Change the spec".to_owned(),
        effect: "Regenerate while sprinting".to_owned(),
        cost: "a rule and a test".to_owned(),
        summary: "It regenerates while sprinting".to_owned(),
        options: vec!["- a | b | c".to_owned(), "- d | e | f".to_owned()],
        evidence: vec!["- src/a.rs:1 | x | y".to_owned()],
        note: "A note.".to_owned(),
        decided_by: "Owner <owner@example.invalid>".to_owned(),
    }
}

fn parse(text: &str, scheme: &IdScheme) -> ParsedFile {
    specengine_core::parse("docs/records/DEC/DEC-0024.md", text.as_bytes(), scheme)
}

// ------------------------------------------------------------ the fixtures

/// AC Setup: each fixture names its prefix, `dir` and template; the
/// template is committed outside the walked roots (no document of the
/// corpus), `dir` inside; it reads as a template holding every slot, the
/// title in the front-matter and the H1, `canon: {{canon}}`, `answers:
/// {{targets}}`, `ref: {{proposal}}`.
#[test]
fn the_fixtures_name_their_records_and_a_template_outside_the_walk() {
    for (name, prefix, dir) in TABLES {
        let text = config_text(name);
        let config = ProjectConfig::from_toml(&text).unwrap_or_else(|e| panic!("{}", e.at(name)));
        assert_eq!(
            config.decision_records,
            Some(DecisionRecords {
                prefix: prefix.to_owned(),
                dir: dir.to_owned(),
                template: "templates/decision.md".to_owned(),
            }),
            "{name}"
        );
        let paths = Paths::from_toml(&text).expect("paths");
        assert!(!paths.in_walk_scope("templates/decision.md"), "{name}");
        assert!(
            paths.in_walk_scope(&format!("{dir}/{prefix}-0099.md")),
            "{name}"
        );
        assert!(
            walked_md_files(&fixture(name))
                .iter()
                .all(|(path, _)| !path.starts_with("templates/")),
            "{name}"
        );
        let template_text = template_of(name);
        let template = Template::parse(&template_text).unwrap_or_else(|e| panic!("{name}: {e}"));
        for slot in Slot::ALL {
            assert!(template.holds(slot), "{name}: {}", slot.name());
        }
        let front: Vec<&str> = template_text
            .lines()
            .skip(1)
            .take_while(|line| *line != "---")
            .collect();
        for line in [
            "title: {{title}}",
            "canon: {{canon}}",
            "  answers: {{targets}}",
            "ref: {{proposal}}",
            "class: decision",
        ] {
            assert!(front.contains(&line), "{name}: {line}");
        }
        assert!(template_text.contains("\n# {{title}}\n"), "{name}");
    }
}

// ------------------------------------------------------------ the table

/// "Data", Config: three required strings, no other key; `prefix` an
/// `[ids]` entry of shape `number`, scope `project` (an `aliases_from`
/// name is an error naming the canonical prefix); `dir`, `template` in
/// `[paths]` grammar; each error at its line.
#[test]
fn the_table_is_checked_on_load_at_its_line() {
    let good =
        "prefix = \"DEC\"\ndir = \"docs/records/DEC\"\ntemplate = \"templates/decision.md\"\n";
    assert!(ProjectConfig::from_toml(&with_table(good)).is_ok());
    let cases: [(&str, String, &str, &str); 11] = [
        (
            "an alias",
            good.replace("\"DEC\"", "\"QST\""),
            "prefix =",
            "`decision_records.prefix`: `QST` is a legacy alias of `Q`: name the canonical prefix",
        ),
        (
            "a name-shape prefix",
            good.replace("\"DEC\"", "\"RULE\""),
            "prefix =",
            "`decision_records.prefix`: `[ids] RULE` is not of shape `number`",
        ),
        (
            "a feature-scoped prefix",
            good.replace("\"DEC\"", "\"AC\""),
            "prefix =",
            "`decision_records.prefix`: `[ids] AC` is feature-scoped",
        ),
        (
            "no such prefix",
            good.replace("\"DEC\"", "\"NOPE\""),
            "prefix =",
            "`decision_records.prefix`: `NOPE` is no `[ids]` prefix",
        ),
        (
            "`..` in dir",
            good.replace("\"docs/records/DEC\"", "\"docs/../out\""),
            "dir =",
            "`decision_records.dir`: \"docs/../out\" leaves the root through `..`",
        ),
        (
            "an absolute dir",
            good.replace("\"docs/records/DEC\"", "\"/abs\""),
            "dir =",
            "`decision_records.dir`: \"/abs\" is absolute",
        ),
        (
            "an empty template",
            good.replace("\"templates/decision.md\"", "\"\""),
            "template =",
            "`decision_records.template`: \"\" is empty",
        ),
        (
            "a template through `.`",
            good.replace("\"templates/decision.md\"", "\"./t.md\""),
            "template =",
            "`decision_records.template`:",
        ),
        (
            "an extra key",
            format!("{good}extra = \"x\"\n"),
            "extra =",
            "unknown field `extra`",
        ),
        (
            "a number",
            good.replace("\"DEC\"", "4"),
            "prefix =",
            "string",
        ),
        (
            "a missing key",
            "prefix = \"DEC\"\ndir = \"docs/records/DEC\"\n".to_owned(),
            "[decision_records]",
            "missing field `template`",
        ),
    ];
    for (label, table, at, want) in cases {
        let text = with_table(&table);
        let error = ProjectConfig::from_toml(&text).expect_err(label);
        assert!(error.message.contains(want), "{label}: {error:?}");
        if label != "a missing key" {
            assert_eq!(error.line, Some(line_of(&text, at)), "{label}: {error:?}");
        } else {
            assert!(error.line.is_some(), "{label}: {error:?}");
        }
        assert!(
            error
                .at("specengine.toml")
                .starts_with(&format!("specengine.toml:{}: ", error.line.unwrap())),
            "{label}"
        );
    }
    // A config without the table loads; it names no record shape.
    let config = config_text("spec-a");
    let at = config.find("[decision_records]").unwrap();
    let without = ProjectConfig::from_toml(&config[..at]).expect("no table");
    assert_eq!(without.decision_records, None);
}

// ------------------------------------------------------------ the template

/// "Data", Template: a front-matter first; every `{{` opens a known slot
/// (else an error at its line, the slots named); free text but the title
/// only in the body; at most 64 KiB.
#[test]
fn a_template_is_checked_at_its_line() {
    let base = template_of("spec-a");
    assert!(Template::parse(&base).is_ok());
    let error = Template::parse("# {{title}}\n").expect_err("no front-matter");
    assert_eq!(error.line, 1);
    assert!(
        error.message.contains("opens with a front-matter"),
        "{error}"
    );

    let unknown = base.replacen("Note:\n", "Note: {{nope}}\n", 1);
    let error = Template::parse(&unknown).expect_err("an unknown slot");
    assert_eq!(error.line, line_of(&unknown, "Note: {{nope}}"));
    assert!(
        error
            .message
            .starts_with("`{{nope}}` is no slot: the slots are `{{id}}`"),
        "{error}"
    );
    assert!(error.message.contains("`{{decided_by}}`"), "{error}");

    let open = base.replacen("Note:\n", "Note: {{ note\n", 1);
    let error = Template::parse(&open).expect_err("an unclosed slot");
    assert_eq!(error.line, line_of(&open, "Note: {{ note"));
    assert!(error.message.starts_with("`{{` opens no slot"), "{error}");

    let literal = base.replacen("Note:\n", "Note: {{\n", 1);
    assert!(Template::parse(&literal).is_err(), "no literal `{{{{`");

    for slot in Slot::ALL {
        let front = base.replacen(
            "scope: [decisions]\n",
            &format!("scope: [decisions]\nx: {{{{{}}}}}\n", slot.name()),
            1,
        );
        let parsed = Template::parse(&front);
        if slot.is_free_text() && slot != Slot::Title {
            let error = parsed.expect_err(slot.name());
            assert_eq!(error.line, line_of(&front, "x: {{"), "{}", slot.name());
            assert!(
                error.message.contains("free text goes in the body only"),
                "{error}"
            );
        } else {
            assert!(parsed.is_ok(), "{}: {parsed:?}", slot.name());
        }
    }

    let big = format!("{base}{}", "x".repeat(TEMPLATE_MAX_BYTES + 1 - base.len()));
    assert_eq!(big.len(), TEMPLATE_MAX_BYTES + 1);
    let error = Template::parse(&big).expect_err("too big");
    assert_eq!(error.line, 1);
    let fits = format!("{base}{}", "x".repeat(TEMPLATE_MAX_BYTES - base.len()));
    assert!(Template::parse(&fits).is_ok());
}

/// "Data": one left-to-right pass, values never read again (`{{id}}` in a
/// value stays that text); the title in the front-matter double-quoted
/// with `\` and `"` backslashed, raw in the body; `targets` `[A, B]` or
/// `[]`; list items joined by LF. M: a re-scan; raw substitution.
#[test]
fn the_render_is_one_pass_and_quotes_the_title() {
    let template = Template::parse(
        "---\nid: {{id}}\ntitle: {{title}}\nanswers: {{targets}}\n---\n\n# {{title}}\n\n\
         {{choice}}\n\n{{options}}\n\n{{note}}\n",
    )
    .expect("a template");
    let mut slots = values();
    slots.title = "say \"hi\" \\ {{id}}".to_owned();
    slots.choice = "{{date}} and {{proposal}}".to_owned();
    slots.note = "{{note}}".to_owned();
    assert_eq!(
        template.render(&slots),
        "---\nid: DEC-0024\ntitle: \"say \\\"hi\\\" \\\\ {{id}}\"\nanswers: [RULE-STAM-REGEN, Q-031]\n\
         ---\n\n# say \"hi\" \\ {{id}}\n\n{{date}} and {{proposal}}\n\n- a | b | c\n- d | e | f\n\n\
         {{note}}\n"
    );
    slots.targets = Vec::new();
    slots.options = Vec::new();
    let rendered = template.render(&slots);
    assert!(rendered.contains("\nanswers: []\n"), "{rendered}");
    assert!(
        rendered.contains("{{date}} and {{proposal}}\n\n\n\n{{note}}"),
        "{rendered}"
    );
}

// ------------------------------------------------------------ title, ID

/// "Data": the title is the label or the answer's first line, whitespace
/// runs one space, trimmed, at most 128 bytes (cut at a character, `…`).
#[test]
fn the_title_is_normalised_and_cut_at_a_character() {
    assert_eq!(
        record_title("  Keep \t the\u{3000}spec \n"),
        "Keep the spec"
    );
    let exact = "a".repeat(TITLE_MAX_BYTES);
    assert_eq!(record_title(&exact), exact);
    let long = "a".repeat(TITLE_MAX_BYTES + 1);
    let cut = record_title(&long);
    assert!(cut.len() <= TITLE_MAX_BYTES, "{}", cut.len());
    assert!(cut.ends_with('\u{2026}'), "{cut}");
    // Two-byte characters: never cut inside one.
    let cyrillic = "\u{0436}".repeat(100);
    let cut = record_title(&cyrillic);
    assert!(cut.len() <= TITLE_MAX_BYTES);
    assert!(cut.ends_with('\u{2026}'));
    assert!(
        cut.trim_end_matches('\u{2026}')
            .chars()
            .all(|c| c == '\u{0436}')
    );
}

/// "Data", ID: `<prefix>-<n>` padded to `width`, more digits past it; the
/// corpus's highest number of the prefix or its aliases (defined or in
/// `aliases:`): spec-a's `DEC` 23 (`DEC-0007`, `DEC-0023`), spec-b's `ADR`
/// 2, spec-a's `Q` 32 (`QST` its alias).
#[test]
fn ids_are_padded_and_numbered_from_the_corpus() {
    assert_eq!(record_id("DEC", 4, 24), "DEC-0024");
    assert_eq!(record_id("ADR", 4, 12_345), "ADR-12345");
    assert_eq!(record_id("X", 1, 7), "X-7");
    for (name, prefix, highest) in [
        ("spec-a", "DEC", 23),
        ("spec-b", "ADR", 2),
        ("spec-a", "Q", 32),
    ] {
        let corpus = fixture(name);
        let scheme = corpus_scheme(&corpus);
        let spec = scheme.prefix(prefix).expect("the prefix").clone();
        let parsed: Vec<ParsedFile> = walked_md_files(&corpus)
            .iter()
            .map(|(path, bytes)| specengine_core::parse(path, bytes, &scheme))
            .collect();
        assert_eq!(
            highest_record_number(&parsed, &spec),
            highest,
            "{name} {prefix}"
        );
    }
    let scheme = corpus_scheme(&fixture("spec-a"));
    let q = scheme.prefix("Q").unwrap();
    assert_eq!(record_number("Q-031", q), Some(31));
    assert_eq!(record_number("QST-040", q), Some(40), "an alias counts");
    assert_eq!(record_number("QX-040", q), None);
    assert_eq!(record_number("Q-04a", q), None);
    assert_eq!(record_number("Q-", q), None);
}

// ------------------------------------------------------------ choice

/// "Data", Queue: `choice` is JSON with one key: `{"option":1}`,
/// `{"working_answer":true}`, `{"answer":"…"}`; any other shape none.
#[test]
fn the_choice_is_one_key_json() {
    for (choice, value) in [
        (Choice::Option(1), json!({"option": 1})),
        (Choice::WorkingAnswer, json!({"working_answer": true})),
        (
            Choice::Answer("Yes \"so\"\n".to_owned()),
            json!({"answer": "Yes \"so\"\n"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&choice).unwrap(), value);
        assert_eq!(Choice::from_json(&value), Some(choice));
    }
    assert_eq!(
        serde_json::to_string(&Choice::Option(2)).unwrap(),
        "{\"option\":2}"
    );
    for other in [
        json!({"option": -1}),
        json!({"option": 1.5}),
        json!({"option": "1"}),
        json!({"working_answer": false}),
        json!({"answer": 1}),
        json!({"option": 1, "answer": "x"}),
        json!({}),
        json!([1]),
        json!("option"),
    ] {
        assert_eq!(Choice::from_json(&other), None, "{other}");
    }
    assert_eq!(Choice::Option(1).described(), "option 1");
    assert_eq!(Choice::WorkingAnswer.described(), "the working answer");
    assert_eq!(
        Choice::Answer("x".to_owned()).described(),
        "the given answer"
    );
}

/// "Data": a character the queue's terminal output escapes, but LF and
/// TAB, is never carried: C0 (CR too), DEL, C1, the bidirectional marks,
/// embeddings, overrides and isolates.
#[test]
fn a_record_never_carries_an_escaped_character() {
    for allowed in ['\n', '\t', 'a', '\u{0436}', '\u{2026}', '\u{3000}'] {
        assert!(!is_escaped(allowed), "{allowed:?}");
    }
    for refused in [
        '\r', '\u{0}', '\u{1b}', '\u{7f}', '\u{85}', '\u{9f}', '\u{061c}', '\u{200e}', '\u{200f}',
        '\u{202a}', '\u{202e}', '\u{2066}', '\u{2069}',
    ] {
        assert!(is_escaped(refused), "{:04X}", u32::from(refused));
    }
    assert_eq!(refused_char("ok\nline\tx"), None);
    assert_eq!(refused_char("a\u{202e}b\u{1b}"), Some('\u{202e}'));
}

// ------------------------------------------------------------ structure

/// "Data", Structure: one readable front-matter, `class: decision`, `id:`
/// the ID, `status: accepted`, defined IDs exactly the ID, `canon:` read
/// back as the slot; each defect named.
#[test]
fn a_rendered_record_has_a_records_structure() {
    let scheme = corpus_scheme(&fixture("spec-a"));
    let template = Template::parse(&template_of("spec-a")).unwrap();
    let render = |slots: &SlotValues| parse(&template.render(slots), &scheme);
    let good = values();
    let parsed = render(&good);
    assert_eq!(
        record_defect(&parsed, "DEC-0024", Some("RULE-STAM-REGEN"), &scheme),
        None
    );
    assert_eq!(canon_text(&parsed).as_deref(), Some("RULE-STAM-REGEN"));
    assert_eq!(
        record_defect(&parsed, "DEC-0025", None, &scheme),
        Some(RecordDefect::Id(Some("DEC-0024".to_owned())))
    );
    assert_eq!(
        record_defect(&parsed, "DEC-0024", Some("Q-031"), &scheme),
        Some(RecordDefect::Canon)
    );
    let mut rejected = values();
    rejected.status = "rejected".to_owned();
    assert_eq!(
        record_defect(&render(&rejected), "DEC-0024", None, &scheme),
        Some(RecordDefect::Status(Some("rejected".to_owned())))
    );
    let mut defining = values();
    defining.effect = "## X {#RULE-STAM-REGEN}".to_owned();
    assert_eq!(
        record_defect(&render(&defining), "DEC-0024", None, &scheme),
        Some(RecordDefect::Definitions(vec![
            "DEC-0024".to_owned(),
            "RULE-STAM-REGEN".to_owned()
        ]))
    );
    for (canon, text) in [
        (
            "docs/spec/movement/stamina.md#regeneration",
            "docs/spec/movement/stamina.md#regeneration",
        ),
        ("MEC-STAMINA#RULE-STAM-REGEN", "MEC-STAMINA#RULE-STAM-REGEN"),
        (
            "docs/spec/movement/stamina.md",
            "docs/spec/movement/stamina.md",
        ),
    ] {
        let mut slots = values();
        slots.canon = canon.to_owned();
        let parsed = render(&slots);
        assert_eq!(
            record_defect(&parsed, "DEC-0024", Some(canon), &scheme),
            None,
            "{canon}"
        );
        assert_eq!(canon_text(&parsed).as_deref(), Some(text));
    }
    let canon_class =
        Template::parse(&template_of("spec-a").replace("class: decision", "class: canon")).unwrap();
    assert_eq!(
        record_defect(
            &parse(&canon_class.render(&good), &scheme),
            "DEC-0024",
            None,
            &scheme
        ),
        Some(RecordDefect::Class(Some("canon".to_owned())))
    );
    let unreadable = parse(
        "---\nid: DEC-0024\nclass: decision\n  - : [\n---\n\n# x\n",
        &scheme,
    );
    assert_eq!(
        record_defect(&unreadable, "DEC-0024", None, &scheme),
        Some(RecordDefect::FrontMatter)
    );
    let none = parse("# x\n\nNo front-matter.\n", &scheme);
    assert_eq!(
        record_defect(&none, "DEC-0024", None, &scheme),
        Some(RecordDefect::FrontMatter)
    );
}
