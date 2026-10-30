//! docs/features/flow-links.md (RM-0042, ADR-0055), core: the three flow
//! link types and the label a `follows` item may carry, on the real parser
//! and the real check, over `fixtures/flow-relay` (an invented game) and
//! `fixtures/flow-hire` (a hired bicycle, no game word), each read through
//! its own `specengine.toml` alone (ADR-0008):
//!
//! - the model: `EdgeKind`, `EdgeLabel`, `Reference.edge` and their JSON;
//! - "Data": an item is a string or a mapping of `to`, `kind`, `when`, the
//!   eight item rules with their `<problem>` verbatim, the first failing
//!   winning (AC-02); a mapping under any other type is today's message;
//! - "Rules and edge cases": `ref-dangling` at the `to` value's line, a
//!   `to` naming no reference, a step following itself, `follows` beside
//!   `depends_on`, two items on one target, `ID#SECTION`;
//! - AC-01: loops are no finding, a `depends_on` loop still is; a walk
//!   visits a node once;
//! - AC-06: the label reaches `SpecGraph`'s edges, and no literal has one.
//!
//! The hash half is `norm_recipe.rs`, the tables `spec_graph.rs` and
//! `bundle_layers.rs`, the reads and exits CLI `tests/flow_links.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::collections::BTreeSet;
use std::fs;

use common::check::{Config, fixture_input, show, with_code};
use common::{corpus_scheme, fixture, render_diagnostics, render_link, text_of, walked_md_files};
use serde_json::Value;
use specengine_core::check::{Endpoint, NodeAt, SpecGraph, Standing, Verdict};
use specengine_core::norm_input;
use specengine_model::{
    Diagnostic, DiagnosticCode, Direction, EdgeKind, EdgeLabel, FOLLOWS, IdScheme, Link,
    LinkTarget, OrderedMap, ParsedFile, Reference, Severity,
};

// ------------------------------------------------------------------ helpers

const RELAY: &str = "flow-relay";
const HIRE: &str = "flow-hire";
const RUN: &str = "docs/spec/leg-run.md";

/// `follows[1]` of `leg-run.md`, written at this line.
const SECOND_ITEM: &str = "    - {to: LEG-FALL, kind: repeat, when: \"the torch is relit\"}\n";
const SECOND_ITEM_LINE: usize = 10;

/// The `links:` value of `leg-run.md` starts at this line.
const LINKS_VALUE_LINE: usize = 8;

/// The message of a `links` value of the wrong shape, as it reads at
/// 87e9b14 (the spec's "today's message").
const TODAY: &str = "front-matter `links` is a mapping, expected a mapping of link type to \
                     references; kept in `extra`";

fn relay() -> IdScheme {
    corpus_scheme(&fixture(RELAY))
}

fn leg_run() -> String {
    fs::read_to_string(fixture(RELAY).join(RUN)).expect("leg-run.md")
}

fn parse(bytes: &str) -> ParsedFile {
    specengine_core::parse(RUN, bytes.as_bytes(), &relay())
}

/// `leg-run.md` with `follows[1]` replaced by `item` at its indent.
fn with_item(item: &str) -> String {
    let base = leg_run();
    assert_eq!(base.matches(SECOND_ITEM).count(), 1, "{base}");
    base.replacen(SECOND_ITEM, &format!("    - {item}\n"), 1)
}

/// The labelled references of `follows`, written order.
fn follows(parsed: &ParsedFile) -> Vec<Reference> {
    declared(parsed)
        .expect("links read")
        .get(FOLLOWS)
        .expect("follows read")
        .clone()
}

/// The `links` map of the document, when it was read.
fn declared(parsed: &ParsedFile) -> Option<&OrderedMap<Vec<Reference>>> {
    parsed.nodes[0]
        .fields
        .as_ref()
        .and_then(|fields| fields.links.as_ref())
}

/// `(diagnostic, its line)` of one code.
fn of_code(parsed: &ParsedFile, code: DiagnosticCode) -> Vec<&Diagnostic> {
    parsed
        .diagnostics
        .iter()
        .filter(|found| found.code == code)
        .collect()
}

/// Every `.md` of `corpus`, one replaced.
fn corpus_with(corpus: &str, path: &str, text: &str) -> Vec<(String, String)> {
    let mut replaced = false;
    let files: Vec<(String, String)> = walked_md_files(&fixture(corpus))
        .into_iter()
        .map(|(file, bytes)| {
            if file == path {
                replaced = true;
                (file, text.to_owned())
            } else {
                (file, String::from_utf8(bytes).expect("UTF-8 fixture"))
            }
        })
        .collect();
    assert!(replaced, "{path} in {corpus}");
    files
}

fn borrowed(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect()
}

fn relay_config() -> Config {
    Config::from_toml(
        &fs::read_to_string(fixture(RELAY).join("specengine.toml")).expect("specengine.toml"),
    )
}

/// A short document of the relay's scheme with `links` as its front-matter
/// tail (indented as written).
fn doc(id: &str, links: &str, body: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntitle: A step\nowner: owner\nreviewed: 2026-10-01\n\
         {links}---\n\n# A step\n\n{body}"
    )
}

fn find(graph: &SpecGraph<'_>, name: &str) -> NodeAt {
    let mut found = Vec::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            if graph.name(at) == name {
                found.push(at);
            }
        }
    }
    assert_eq!(found.len(), 1, "{name}: {found:?}");
    found[0]
}

/// `<src> --<type>--> <dst> @<line>[ kind][ when]` of every edge, sorted.
fn edge_rows(graph: &SpecGraph<'_>) -> Vec<String> {
    let end = |endpoint: &Endpoint, written: &str| match endpoint {
        Endpoint::Nodes(nodes) => nodes
            .iter()
            .map(|at| graph.name(*at))
            .collect::<Vec<_>>()
            .join("+"),
        Endpoint::Dangling(_) => format!("!{written}"),
        Endpoint::Skipped => format!("~{written}"),
        Endpoint::Unchecked => format!("?{written}"),
    };
    let mut rows: Vec<String> = graph
        .edges()
        .iter()
        .map(|edge| {
            let mut row = format!(
                "{} --{}--> {} @{}",
                end(&edge.source, &edge.written),
                edge.link_type,
                end(&edge.target, &edge.written),
                edge.line
            );
            if let Some(label) = &edge.edge {
                if let Some(kind) = label.kind {
                    row.push_str(&format!(" kind {}", kind.as_str()));
                }
                if let Some(when) = &label.when {
                    row.push_str(&format!(" when {when}"));
                }
            }
            row
        })
        .collect();
    rows.sort();
    rows
}

/// [`render_link`] and the label, as the fixtures' `expected.json` writes
/// it.
fn labelled_link(link: &Link) -> String {
    let mut out = render_link(link);
    if let LinkTarget::Reference(reference) = &link.dst
        && let Some(label) = &reference.edge
    {
        if let Some(kind) = label.kind {
            out.push_str(&format!(" | kind {}", kind.as_str()));
        }
        if let Some(when) = &label.when {
            out.push_str(&format!(" | when {when}"));
        }
    }
    out
}

// ---------------------------------------------------------------- the model

/// "Data": a closed set of three, `ALL`, `as_str`, `parse`, lowercase in
/// JSON. M3 (no closed set of kinds) turns rule 4 red below.
#[test]
fn edge_kinds_are_a_closed_set_of_three_lowercase_words() {
    assert_eq!(
        EdgeKind::ALL,
        [EdgeKind::Alternative, EdgeKind::Parallel, EdgeKind::Repeat]
    );
    for (kind, word) in [
        (EdgeKind::Alternative, "alternative"),
        (EdgeKind::Parallel, "parallel"),
        (EdgeKind::Repeat, "repeat"),
    ] {
        assert_eq!(kind.as_str(), word);
        assert_eq!(EdgeKind::parse(word), Some(kind));
        assert_eq!(
            serde_json::to_value(kind).expect("JSON"),
            Value::String(word.to_owned())
        );
    }
    for word in ["loop", "Repeat", "REPEAT", " repeat", "", "repeats"] {
        assert_eq!(EdgeKind::parse(word), None, "{word:?}");
    }
}

/// "Data": `edge`, `kind` and `when` are skipped when `None`, so a plain
/// reference serialises as it did; a labelled one comes back from JSON
/// whole (the index stores `Link` as JSON).
#[test]
fn a_label_is_absent_from_the_json_of_a_plain_reference_and_round_trips() {
    let parsed = parse(&leg_run());
    let references = follows(&parsed);
    assert_eq!(references.len(), 2);
    let plain = serde_json::to_value(&references[0]).expect("JSON");
    assert!(plain.get("edge").is_none(), "{plain}");
    let labelled = serde_json::to_value(&references[1]).expect("JSON");
    assert_eq!(
        labelled["edge"],
        serde_json::json!({"kind": "repeat", "when": "the torch is relit"}),
        "{labelled}"
    );
    let kind_alone = EdgeLabel {
        kind: Some(EdgeKind::Parallel),
        when: None,
    };
    assert_eq!(
        serde_json::to_value(&kind_alone).expect("JSON"),
        serde_json::json!({"kind": "parallel"})
    );
    let when_alone = EdgeLabel {
        kind: None,
        when: Some("the runner arrives".to_owned()),
    };
    assert_eq!(
        serde_json::to_value(&when_alone).expect("JSON"),
        serde_json::json!({"when": "the runner arrives"})
    );

    let text = serde_json::to_string(&parsed).expect("JSON");
    let back: ParsedFile = serde_json::from_str(&text).expect("the stored form reads back");
    assert_eq!(back, parsed);
}

// ---------------------------------------------------------- the two corpora

/// ADR-0008: one function, each corpus against its own `expected.json`.
fn corpus_is_read_as_expected(name: &str) {
    let corpus = fixture(name);
    let scheme = corpus_scheme(&corpus);
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(corpus.join("expected.json")).expect("expected.json"),
    )
    .expect("JSON");
    let files = expected["files"].as_object().expect("files");
    let mut seen = BTreeSet::new();
    for (path, bytes) in walked_md_files(&corpus) {
        let want = files
            .get(&path)
            .unwrap_or_else(|| panic!("{name}: {path} is not in expected.json"));
        let parsed = specengine_core::parse(&path, &bytes, &scheme);
        let node = parsed.document().expect("a document");
        assert_eq!(node.id.as_deref(), want["id"].as_str(), "{path}");
        assert_eq!(node.kind.as_deref(), want["kind"].as_str(), "{path}");
        assert_eq!(node.title.as_deref(), want["title"].as_str(), "{path}");
        assert_eq!(
            path.to_lowercase(),
            format!(
                "docs/spec/{}.md",
                want["id"].as_str().unwrap().to_lowercase()
            ),
            "one document per node at its lowercase ID"
        );
        let sections: Vec<String> = parsed
            .sections()
            .iter()
            .filter_map(|section| section.id.clone())
            .collect();
        let want_sections: Vec<String> = want["sections"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|section| section.as_str().expect("a string").to_owned())
            .collect();
        assert_eq!(sections, want_sections, "{path}");
        let links: Vec<String> = parsed.links.iter().map(labelled_link).collect();
        let want_links: Vec<String> = want["links"]
            .as_array()
            .expect("links")
            .iter()
            .map(|link| link.as_str().expect("a string").to_owned())
            .collect();
        assert_eq!(links, want_links, "{path}");
        assert_eq!(render_diagnostics(&parsed), Vec::<String>::new(), "{path}");
        seen.insert(path);
    }
    assert_eq!(
        seen,
        files.keys().cloned().collect::<BTreeSet<_>>(),
        "{name}: expected.json names every document"
    );
    let prefixes: Vec<String> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.prefix.clone())
        .collect();
    let want: Vec<String> = expected["prefixes"]
        .as_array()
        .expect("prefixes")
        .iter()
        .map(|prefix| prefix.as_str().expect("a string").to_owned())
        .collect();
    assert_eq!(prefixes, want, "{name}");
}

#[test]
fn the_game_corpus_reads_as_expected() {
    corpus_is_read_as_expected(RELAY);
}

#[test]
fn the_hire_corpus_reads_as_expected() {
    corpus_is_read_as_expected(HIRE);
}

/// The two corpora share no prefix or kind: they differ in nature
/// (ADR-0008). `FEE`, `JRN`, `journey`, `tariff` are spec-c's on purpose;
/// flow-hire stands beside it.
#[test]
fn the_two_corpora_share_no_word() {
    let (one, other) = (relay(), corpus_scheme(&fixture(HIRE)));
    let words = |scheme: &IdScheme| -> BTreeSet<String> {
        scheme
            .prefixes()
            .iter()
            .flat_map(|spec| [spec.prefix.clone(), spec.kind.clone()])
            .collect()
    };
    let shared: Vec<String> = words(&one).intersection(&words(&other)).cloned().collect();
    assert!(shared.is_empty(), "{shared:?}");
}

// ------------------------------------------------------------ an item's form

/// "Data": `{to: X}` alone equals the plain `X`: no label, the same link,
/// the same input. M: a label of two `None`s for `to` alone.
#[test]
fn to_alone_is_the_plain_reference() {
    let plain = parse(&leg_run());
    let base = leg_run();
    let spelled = base.replacen("    - LEG-DRAW\n", "    - {to: LEG-DRAW}\n", 1);
    assert_ne!(spelled, base);
    let parsed = parse(&spelled);
    assert_eq!(render_diagnostics(&parsed), Vec::<String>::new());
    assert_eq!(follows(&parsed)[0].edge, None);
    assert_eq!(follows(&parsed)[0].id, "LEG-DRAW");
    assert_eq!(
        parsed.links.iter().map(labelled_link).collect::<Vec<_>>(),
        plain.links.iter().map(labelled_link).collect::<Vec<_>>()
    );
}

/// "Data": either of `kind` and `when` alone is a label of that one key.
#[test]
fn kind_and_when_are_each_optional() {
    let parsed = parse(&with_item("{to: LEG-FALL, kind: parallel}"));
    assert_eq!(render_diagnostics(&parsed), Vec::<String>::new());
    assert_eq!(
        follows(&parsed)[1].edge,
        Some(EdgeLabel {
            kind: Some(EdgeKind::Parallel),
            when: None
        })
    );
    let parsed = parse(&with_item("{to: LEG-FALL, when: \"the torch is relit\"}"));
    assert_eq!(render_diagnostics(&parsed), Vec::<String>::new());
    assert_eq!(
        follows(&parsed)[1].edge,
        Some(EdgeLabel {
            kind: None,
            when: Some("the torch is relit".to_owned())
        })
    );
}

/// "Data": every kind reads; a mapping's keys in any order and block style
/// give the same label.
#[test]
fn an_item_reads_alike_in_any_key_order_and_style() {
    for kind in EdgeKind::ALL {
        let parsed = parse(&with_item(&format!(
            "{{to: LEG-FALL, kind: {}}}",
            kind.as_str()
        )));
        assert_eq!(
            follows(&parsed)[1].edge.as_ref().and_then(|edge| edge.kind),
            Some(kind)
        );
    }
    let want = Some(EdgeLabel {
        kind: Some(EdgeKind::Repeat),
        when: Some("the torch is relit".to_owned()),
    });
    for item in [
        "{when: \"the torch is relit\", kind: repeat, to: LEG-FALL}",
        "{kind: repeat, to: LEG-FALL, when: the torch is relit}",
        "to: LEG-FALL\n      kind: repeat\n      when: \"the torch is relit\"",
        "when: 'the torch is relit'\n      to: \"LEG-FALL\"\n      kind: \"repeat\"",
    ] {
        let parsed = parse(&with_item(item));
        assert_eq!(render_diagnostics(&parsed), Vec::<String>::new(), "{item}");
        let references = follows(&parsed);
        assert_eq!(references[1].id, "LEG-FALL", "{item}");
        assert_eq!(references[1].edge, want, "{item}");
    }
}

/// "Data": `when` is stored trimmed, is prose (any script, any `|`), and
/// its bound is in bytes, of what is stored. M: the bound in characters.
#[test]
fn when_is_stored_trimmed_and_bounded_in_bytes() {
    let when_of = |item: &str| {
        let parsed = parse(&with_item(item));
        assert_eq!(render_diagnostics(&parsed), Vec::<String>::new(), "{item}");
        follows(&parsed)[1]
            .edge
            .as_ref()
            .and_then(|edge| edge.when.clone())
    };
    assert_eq!(
        when_of("{to: LEG-FALL, when: \"  the torch is relit \"}").as_deref(),
        Some("the torch is relit")
    );
    assert_eq!(
        when_of("{to: LEG-FALL, when: \"a | b - c\"}").as_deref(),
        Some("a | b - c")
    );
    assert_eq!(
        when_of("{to: LEG-FALL, when: \"\u{043e}\u{0433}\u{043e}\u{043d}\u{044c}\"}").as_deref(),
        Some("\u{043e}\u{0433}\u{043e}\u{043d}\u{044c}"),
        "prose in any script"
    );
    let at_the_bound = "x".repeat(256);
    assert_eq!(
        when_of(&format!("{{to: LEG-FALL, when: \"{at_the_bound}\"}}")),
        Some(at_the_bound.clone())
    );
    // 128 two-byte characters are 256 bytes.
    let wide = "\u{0436}".repeat(128);
    assert_eq!(
        when_of(&format!("{{to: LEG-FALL, when: \"{wide}\"}}")),
        Some(wide)
    );
    // Padding around a value at the bound is not stored.
    assert_eq!(
        when_of(&format!("{{to: LEG-FALL, when: \"  {at_the_bound}  \"}}")),
        Some(at_the_bound)
    );
}

/// "Rules": `to` keeps its line and its span through the fifth `Located`
/// level, in flow and block style, wherever it stands in the item. M: the
/// level dropped (the span lost, the line the item's).
#[test]
fn to_keeps_its_span_and_line() {
    let cases = [
        (leg_run(), SECOND_ITEM_LINE),
        (
            with_item("kind: repeat\n      when: \"x\"\n      to: LEG-FALL"),
            SECOND_ITEM_LINE + 2,
        ),
        (
            with_item("{kind: repeat, to: LEG-FALL, when: \"x\"}"),
            SECOND_ITEM_LINE,
        ),
    ];
    for (text, line) in cases {
        let parsed = parse(&text);
        assert_eq!(render_diagnostics(&parsed), Vec::<String>::new(), "{text}");
        let span = follows(&parsed)[1].span.expect("the span of `to`");
        assert_eq!(text_of(text.as_bytes(), span), "LEG-FALL", "{text}");
        let before = &text.as_bytes()[..span.start];
        assert_eq!(
            before.iter().filter(|&&byte| byte == b'\n').count() + 1,
            line,
            "{text}"
        );
        let files = corpus_with(RELAY, RUN, &text);
        let config = relay_config();
        let input = config.input(&borrowed(&files));
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let edge = graph
            .edges()
            .iter()
            .find(|edge| edge.written == "LEG-FALL" && edge.link_type == FOLLOWS)
            .expect("the labelled edge");
        assert_eq!(edge.line, line, "the edge's line is the `to` value's");
    }
}

// ---------------------------------------------------------- the item rules

/// The eight rules in order: one row each, the rule applied alone to
/// `follows[1]`, and `<problem>` verbatim.
fn the_eight_rules() -> Vec<(String, &'static str)> {
    vec![
        (
            "{to: LEG-FALL, by: me}".to_owned(),
            "unknown key `by`: an item takes `to`, `when`, `kind`",
        ),
        ("{kind: repeat}".to_owned(), "no `to`"),
        ("{to: 5}".to_owned(), "`to` is not a string"),
        (
            "{to: LEG-FALL, kind: loop}".to_owned(),
            "`kind` `loop` is none of `alternative`, `parallel`, `repeat`",
        ),
        (
            "{to: LEG-FALL, when: 5}".to_owned(),
            "`when` is not a string",
        ),
        (
            "{to: LEG-FALL, when: \"   \"}".to_owned(),
            "`when` is blank",
        ),
        (
            format!("{{to: LEG-FALL, when: \"{}\"}}", "x".repeat(257)),
            "`when` is over 256 bytes",
        ),
        (
            "{to: LEG-FALL, when: \"a\\tb\"}".to_owned(),
            "`when` holds a control character",
        ),
    ]
}

/// The item's message, as the spec's "Outputs" prints it.
fn message_of(index: usize, problem: &str) -> String {
    format!("front-matter `links`: `follows[{index}]`: {problem}; kept in `extra`")
}

/// One malformed item: exactly one `frontmatter-type`, at `line`, with
/// `problem`; nothing else found; the whole map in `extra`; the document's
/// hash null.
fn assert_refused(text: &str, index: usize, line: usize, problem: &str) {
    let parsed = parse(text);
    let types = of_code(&parsed, DiagnosticCode::FrontmatterType);
    assert_eq!(types.len(), 1, "{text}\n{:?}", parsed.diagnostics);
    assert_eq!(types[0].message, message_of(index, problem), "{text}");
    assert_eq!(types[0].line, line, "{text}");
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(declared(&parsed), None, "{text}");
    assert!(
        parsed.nodes[0]
            .extra
            .iter()
            .flatten()
            .any(|entry| entry.key == "links"),
        "`links` is kept in `extra`: {text}"
    );
    assert!(
        !parsed
            .links
            .iter()
            .any(|link| link.origin == specengine_model::LinkOrigin::Frontmatter),
        "no declared link stands: {text}"
    );
    assert_eq!(norm_input(&parsed, text.as_bytes(), 0), None, "{text}");
}

/// AC-02, the parser's half, each rule alone. M3: no closed set of kinds.
#[test]
fn ac02_each_item_rule_gives_its_problem_at_the_item_s_line() {
    for (item, problem) in the_eight_rules() {
        assert_refused(&with_item(&item), 1, SECOND_ITEM_LINE, problem);
    }
}

/// "Data": the first failing rule gives `<problem>`, in the order the spec
/// numbers them, whatever else is wrong with the item.
#[test]
fn the_first_failing_rule_speaks() {
    let unknown = "unknown key `by`: an item takes `to`, `when`, `kind`";
    let kinds = "`kind` `loop` is none of `alternative`, `parallel`, `repeat`";
    let long = "x".repeat(257);
    let long_with_a_tab = format!("{long}\\t{long}");
    for (item, problem) in [
        ("{by: me}".to_owned(), unknown),
        ("{by: me, kind: loop, when: 5}".to_owned(), unknown),
        ("{to: 5, by: me}".to_owned(), unknown),
        ("{kind: loop, when: 5}".to_owned(), "no `to`"),
        ("{}".to_owned(), "no `to`"),
        ("{to: 5, kind: loop}".to_owned(), "`to` is not a string"),
        ("{to: [LEG-FALL]}".to_owned(), "`to` is not a string"),
        ("{to: {id: LEG-FALL}}".to_owned(), "`to` is not a string"),
        ("{to: ~}".to_owned(), "`to` is not a string"),
        ("{to: LEG-FALL, kind: loop, when: 5}".to_owned(), kinds),
        ("{to: LEG-FALL, kind: loop, when: \"\"}".to_owned(), kinds),
        (
            "{to: LEG-FALL, when: [a]}".to_owned(),
            "`when` is not a string",
        ),
        (
            "{to: LEG-FALL, when: ~}".to_owned(),
            "`when` is not a string",
        ),
        ("{to: LEG-FALL, when: \"\"}".to_owned(), "`when` is blank"),
        (
            format!("{{to: LEG-FALL, when: \"{long_with_a_tab}\"}}"),
            "`when` is over 256 bytes",
        ),
    ] {
        assert_refused(&with_item(&item), 1, SECOND_ITEM_LINE, problem);
    }
}

/// Rule 7 at its edge: 257 bytes refused, and the two-byte count of 129
/// characters is 258. M: the bound in characters.
#[test]
fn rule_7_counts_bytes() {
    let wide = "\u{0436}".repeat(129);
    assert_refused(
        &with_item(&format!("{{to: LEG-FALL, when: \"{wide}\"}}")),
        1,
        SECOND_ITEM_LINE,
        "`when` is over 256 bytes",
    );
}

/// Rule 8: a tab, a line break (a block scalar's inner one too), BEL, DEL
/// and a C1 control. M: only ASCII controls looked at.
#[test]
fn rule_8_covers_every_control_character() {
    for item in [
        "{to: LEG-FALL, when: \"a\\tb\"}",
        "{to: LEG-FALL, when: \"a\\nb\"}",
        "{to: LEG-FALL, when: \"a\\x07b\"}",
        "{to: LEG-FALL, when: \"a\\x7fb\"}",
        "{to: LEG-FALL, when: \"a\\x85b\"}",
        "to: LEG-FALL\n      when: |\n        one\n        two\n",
    ] {
        assert_refused(
            &with_item(item.trim_end_matches('\n')),
            1,
            SECOND_ITEM_LINE,
            "`when` holds a control character",
        );
    }
}

/// The problem names the index of the failing item, and the line is that
/// item's, in block style too.
#[test]
fn the_message_names_the_index_and_the_line_of_the_item() {
    let base = leg_run();
    let first = base.replacen("    - LEG-DRAW\n", "    - {to: LEG-DRAW, by: me}\n", 1);
    assert_refused(
        &first,
        0,
        SECOND_ITEM_LINE - 1,
        "unknown key `by`: an item takes `to`, `when`, `kind`",
    );
    assert_refused(
        &with_item("to: LEG-FALL\n      kind: loop"),
        1,
        SECOND_ITEM_LINE,
        "`kind` `loop` is none of `alternative`, `parallel`, `repeat`",
    );
    let third = with_item("{to: LEG-FALL}\n    - {to: LEG-START, kind: loop}");
    assert_refused(
        &third,
        2,
        SECOND_ITEM_LINE + 1,
        "`kind` `loop` is none of `alternative`, `parallel`, `repeat`",
    );
}

/// Data with no rule of its own never panics and is never green: a kind
/// that is no string, keys that are no strings, a repeated key. Each is a
/// finding on the document and nulls its hash; the wording is not pinned
/// (the spec gives none). M: a panic on a non-string key.
#[test]
fn odd_items_are_a_finding_not_a_panic() {
    for item in [
        "{to: LEG-FALL, kind: 5}",
        "{to: LEG-FALL, kind: [repeat]}",
        "{to: LEG-FALL, kind: {a: b}}",
        "{to: LEG-FALL, 5: x}",
        "{to: LEG-FALL, ? [a] : b}",
        "{to: LEG-FALL, to: LEG-RUN}",
        "{to: LEG-FALL, kind: repeat, kind: parallel}",
    ] {
        let text = with_item(item);
        let parsed = parse(&text);
        assert!(!parsed.diagnostics.is_empty(), "{item}: read as clean");
        assert_eq!(declared(&parsed), None, "{item}");
        assert_eq!(norm_input(&parsed, text.as_bytes(), 0), None, "{item}");
    }
}

/// "Rules": a mapping under any type but `follows` is today's message, at
/// today's line, the `links` value's. M2: a mapping accepted under every
/// type.
#[test]
fn ac02_a_mapping_under_another_type_is_today_s_message() {
    let base = leg_run();
    let shared = [
        "uses",
        "supplies",
        "depends_on",
        "derived_from",
        "constrains",
        "uses_term",
        "verifies",
        // No shared type, no label either.
        "made_up",
    ];
    for link_type in shared {
        for item in ["[{to: KIT-TORCH}]", "[{to: KIT-TORCH, kind: repeat}]"] {
            let text = base.replacen(
                "  uses: [KIT-TORCH]\n",
                &format!("  {link_type}: {item}\n"),
                1,
            );
            assert_ne!(text, base);
            let parsed = parse(&text);
            let types = of_code(&parsed, DiagnosticCode::FrontmatterType);
            assert_eq!(types.len(), 1, "{text}\n{:?}", parsed.diagnostics);
            assert_eq!(types[0].message, TODAY, "{link_type}");
            assert_eq!(types[0].line, LINKS_VALUE_LINE, "{link_type}");
            assert_eq!(declared(&parsed), None, "{link_type}");
            assert_eq!(norm_input(&parsed, text.as_bytes(), 0), None);
        }
    }
}

/// A `follows` item that is neither a string nor a mapping stays today's
/// message too.
#[test]
fn a_follows_item_of_another_type_is_today_s_message() {
    for item in ["5", "[LEG-FALL]", "true", "~", "1.5"] {
        let text = with_item(item);
        let parsed = parse(&text);
        let types = of_code(&parsed, DiagnosticCode::FrontmatterType);
        assert_eq!(types.len(), 1, "{text}\n{:?}", parsed.diagnostics);
        assert_eq!(types[0].message, TODAY, "{item}");
        assert_eq!(types[0].line, LINKS_VALUE_LINE, "{item}");
    }
}

/// A `follows` value that is no list stays today's message at today's
/// line; a null one is an empty list, as for any type.
#[test]
fn a_follows_value_that_is_no_list_is_today_s_message() {
    let base = leg_run();
    let list = "  follows:\n    - LEG-DRAW\n".to_owned() + SECOND_ITEM;
    assert_eq!(base.matches(&list).count(), 1, "{base}");
    for value in ["LEG-DRAW", "{to: LEG-DRAW}", "5"] {
        let text = base.replacen(&list, &format!("  follows: {value}\n"), 1);
        let parsed = parse(&text);
        let types = of_code(&parsed, DiagnosticCode::FrontmatterType);
        assert_eq!(types.len(), 1, "{text}\n{:?}", parsed.diagnostics);
        assert_eq!(types[0].message, TODAY, "{value}");
        assert_eq!(norm_input(&parsed, text.as_bytes(), 0), None, "{value}");
    }
    let text = base.replacen(&list, "  follows:\n", 1);
    let parsed = parse(&text);
    assert_eq!(render_diagnostics(&parsed), Vec::<String>::new());
    assert_eq!(follows(&parsed), Vec::<Reference>::new());
}

/// The depth budget holds through an item: at the cap (root, `links`,
/// `follows` and the item are four levels) a nested `to` is rule 3 on a 2
/// MiB stack, one level more one `frontmatter-yaml`. M: the fifth level
/// costing more stack than the budget leaves.
#[test]
fn a_deep_value_in_an_item_stays_within_the_depth_budget() {
    let under = specengine_core::MAX_DEPTH - 4;
    let nested = |depth: usize| {
        with_item(&format!(
            "{{to: {}1{}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        ))
    };
    let (at_the_cap, over_the_cap) = (nested(under), nested(under + 1));
    let worker = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            assert_refused(&at_the_cap, 1, SECOND_ITEM_LINE, "`to` is not a string");
            let parsed = parse(&over_the_cap);
            let codes: Vec<DiagnosticCode> =
                parsed.diagnostics.iter().map(|found| found.code).collect();
            assert_eq!(codes, [DiagnosticCode::FrontmatterYaml]);
        })
        .expect("a worker thread");
    worker.join().expect("the parses");
}

// --------------------------------------------------------- what `to` names

/// "Data": a `to` naming no reference is `unparsed-reference`, as a plain
/// item is; its document's hash is null.
#[test]
fn a_to_naming_no_reference_is_unparsed_reference() {
    for item in [
        "{to: \"not a reference\", kind: repeat}",
        "{to: \"\", when: \"x\"}",
        "{to: \"LEG-FALL LEG-RUN\"}",
        "not a reference",
    ] {
        let text = with_item(item);
        let parsed = parse(&text);
        let found = of_code(&parsed, DiagnosticCode::UnparsedReference);
        assert_eq!(found.len(), 1, "{item}: {:?}", parsed.diagnostics);
        assert_eq!(found[0].line, SECOND_ITEM_LINE, "{item}");
        assert_eq!(norm_input(&parsed, text.as_bytes(), 0), None, "{item}");
    }
}

/// "Rules": `ref-dangling` at the `to` value's line, blocked, in both
/// styles. M: the item's line.
#[test]
fn a_dangling_to_is_ref_dangling_at_the_to_value_s_line() {
    let config = relay_config();
    for (item, line) in [
        (
            "{to: LEG-NOPE, kind: repeat, when: \"x\"}",
            SECOND_ITEM_LINE,
        ),
        (
            "kind: repeat\n      when: \"x\"\n      to: LEG-NOPE",
            SECOND_ITEM_LINE + 2,
        ),
        ("to: LEG-NOPE\n      kind: repeat", SECOND_ITEM_LINE),
    ] {
        let files = corpus_with(RELAY, RUN, &with_item(item));
        let report = config.check(&borrowed(&files));
        let found = with_code(&report, "ref-dangling");
        assert_eq!(found.len(), 1, "{item}\n{}", show(&report));
        assert_eq!(found[0].path, RUN);
        assert_eq!(found[0].line, line, "{item}");
        assert_eq!(found[0].subject, "LEG-NOPE", "{item}");
        assert_eq!(report.verdict, Verdict::Blocked, "{item}");
    }
}

/// "Rules": `to` may name `ID#SECTION`: the edge lands on the section, its
/// label kept, its written form whole; a section naming nothing dangles.
#[test]
fn to_may_name_a_section() {
    let config = relay_config();
    let target = doc(
        "LEG-HOME",
        "",
        "Lead-in.\n\n## Part {#LEG-PART}\n\nThe part.\n",
    );
    let later = doc(
        "LEG-AWAY",
        "links:\n  follows:\n    - {to: LEG-HOME#LEG-PART, kind: repeat, when: \"x\"}\n",
        "Away.\n",
    );
    let files = [
        ("docs/spec/leg-away.md", later.as_str()),
        ("docs/spec/leg-home.md", target.as_str()),
    ];
    let report = config.check(&files);
    assert_eq!(report.verdict, Verdict::Clean, "{}", show(&report));
    assert!(report.findings.is_empty(), "{}", show(&report));
    let input = config.input(&files);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    assert_eq!(
        edge_rows(&graph),
        ["LEG-AWAY --follows--> LEG-PART @9 kind repeat when x"]
    );
    assert_eq!(graph.edges()[0].written, "LEG-HOME#LEG-PART");

    let wrong = later.replace("LEG-HOME#LEG-PART", "LEG-HOME#LEG-NONE");
    let report = config.check(&[
        ("docs/spec/leg-away.md", wrong.as_str()),
        ("docs/spec/leg-home.md", target.as_str()),
    ]);
    assert_eq!(report.verdict, Verdict::Blocked, "{}", show(&report));
    assert_eq!(
        with_code(&report, "ref-dangling")[0].line,
        9,
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------ loops are fine

/// AC-01, AC-07 (core half): both corpora, one function, `[ids]` alone:
/// clean, not a single finding. M1: `depends_cycle` also reading `follows`.
fn corpus_is_clean(name: &str) {
    let (config, input) = fixture_input(&fixture(name));
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::Clean, "{name}: {}", show(&report));
    assert!(report.findings.is_empty(), "{name}: {}", show(&report));
}

#[test]
fn ac01_the_game_corpus_is_clean_with_its_two_loops() {
    corpus_is_clean(RELAY);
}

#[test]
fn ac01_the_hire_corpus_is_clean_with_its_retry() {
    corpus_is_clean(HIRE);
}

/// The relay's documents with every `follows` item a plain `depends_on`.
fn relay_in_depends_on() -> Vec<(String, String)> {
    walked_md_files(&fixture(RELAY))
        .into_iter()
        .map(|(path, bytes)| {
            let text = String::from_utf8(bytes).expect("UTF-8 fixture");
            let rewritten: String = text
                .split_inclusive('\n')
                .map(|line| {
                    if line.trim_start().starts_with("follows:") {
                        return line.replace("follows:", "depends_on:");
                    }
                    match line.split_once("{to: ") {
                        Some((head, rest)) => {
                            let target = rest.split([',', '}']).next().expect("a target");
                            format!("{head}{target}\n")
                        }
                        None => line.to_owned(),
                    }
                })
                .collect();
            (path, rewritten)
        })
        .collect()
}

/// AC-01's other side: the same two loops in `depends_on` are still a
/// `depends-cycle` warning, so the clean above is `follows`' doing, and
/// `depends_on` keeps its meaning (ADR-0055).
#[test]
fn ac01_the_same_loops_in_depends_on_stay_a_finding() {
    let config = relay_config();
    let files = relay_in_depends_on();
    let report = config.check(&borrowed(&files));
    let found = with_code(&report, "depends-cycle");
    assert!(!found.is_empty(), "{}", show(&report));
    assert!(found.iter().all(|f| f.severity == Severity::Warning));
    assert_eq!(report.findings.len(), found.len(), "{}", show(&report));
}

/// "Rules": a step may follow itself; `follows` beside `depends_on` on one
/// pair is no finding; two items on one target are two edges.
#[test]
fn self_follow_a_shared_pair_and_a_doubled_target_are_no_finding() {
    let config = relay_config();
    let one = doc(
        "LEG-ONE",
        "links:\n  follows:\n    - {to: LEG-ONE, kind: repeat, when: \"again\"}\n",
        "One.\n",
    );
    let two = doc(
        "LEG-TWO",
        "links:\n  follows: [LEG-ONE]\n  depends_on: [LEG-ONE]\n",
        "Two.\n",
    );
    let three = doc(
        "LEG-THREE",
        "links:\n  follows:\n    - LEG-TWO\n    - {to: LEG-TWO, kind: alternative, when: \"a\"}\n    \
         - {to: LEG-TWO, kind: alternative, when: \"b\"}\n",
        "Three.\n",
    );
    let files = [
        ("docs/spec/leg-one.md", one.as_str()),
        ("docs/spec/leg-three.md", three.as_str()),
        ("docs/spec/leg-two.md", two.as_str()),
    ];
    let report = config.check(&files);
    assert_eq!(report.verdict, Verdict::Clean, "{}", show(&report));
    assert!(report.findings.is_empty(), "{}", show(&report));
    let input = config.input(&files);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    assert_eq!(
        edge_rows(&graph),
        [
            "LEG-ONE --follows--> LEG-ONE @9 kind repeat when again",
            "LEG-THREE --follows--> LEG-TWO @10 kind alternative when a",
            "LEG-THREE --follows--> LEG-TWO @11 kind alternative when b",
            "LEG-THREE --follows--> LEG-TWO @9",
            "LEG-TWO --depends_on--> LEG-ONE @9",
            "LEG-TWO --follows--> LEG-ONE @8",
        ]
    );
}

/// AC-01: a walk over a loop ends, each node once: forward from LEG-RUN
/// against the link, the five nodes at their distances and the six edges.
/// The spec's `nodes 5, edges 6`. M: no visited set hangs the walk (the
/// CLI's deadline catches a hang; here the rows do).
#[test]
fn ac01_a_walk_over_the_loops_visits_each_node_once() {
    let (config, input) = fixture_input(&fixture(RELAY));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let run = find(&graph, "LEG-RUN");
    let follow_in = |link_type: &str| (link_type == FOLLOWS).then_some(Direction::In);
    let walk = graph.walk(&[run], follow_in, None, |file| {
        graph.standing(file) == Standing::Live
    });
    let reached: Vec<(String, usize)> = walk
        .nodes
        .iter()
        .map(|(at, distance)| (graph.name(*at), *distance))
        .collect();
    assert_eq!(
        reached,
        [
            ("LEG-RUN".to_owned(), 0),
            ("LEG-FALL".to_owned(), 1),
            ("LEG-SCORE".to_owned(), 1),
            ("LEG-DRAW".to_owned(), 2),
            ("LEG-END".to_owned(), 2),
        ]
    );
    assert_eq!(walk.edges.len(), 6, "{:?}", walk.edges);
}

// ---------------------------------------------------------- labels on edges

/// AC-06, core half: `SpecGraph`'s edges carry the label of the item, and
/// every plain one none. M: `edge: None` on the front-matter edges.
#[test]
fn ac06_the_label_reaches_the_edges_of_the_graph() {
    let (config, input) = fixture_input(&fixture(RELAY));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    assert_eq!(
        edge_rows(&graph),
        [
            "KIT-OIL --supplies--> KIT-TORCH @8",
            "LEG-DRAW --follows--> LEG-SCORE @10 kind repeat when rounds remain",
            "LEG-DRAW --follows--> LEG-START @9",
            "LEG-END --follows--> LEG-SCORE @9 kind alternative when no round remains",
            "LEG-FALL --follows--> LEG-RUN @9 kind alternative when the torch goes out",
            "LEG-FALL --uses--> KIT-TORCH @10",
            "LEG-RUN --follows--> LEG-DRAW @9",
            "LEG-RUN --follows--> LEG-FALL @10 kind repeat when the torch is relit",
            "LEG-RUN --uses--> KIT-TORCH @11",
            "LEG-SCORE --follows--> LEG-RUN @9 when the runner arrives",
        ]
    );
    let (config, input) = fixture_input(&fixture(HIRE));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    assert_eq!(
        edge_rows(&graph),
        [
            "JRN-METER --follows--> JRN-UNLOCK @9 kind parallel",
            "JRN-NEAREST --follows--> JRN-RETURN @9 kind alternative when dock full",
            "JRN-NO-RELEASE --follows--> JRN-UNLOCK @9 kind alternative when not released",
            "JRN-RETURN --follows--> JRN-NEAREST @10 kind repeat when a slot is near",
            "JRN-RETURN --follows--> JRN-RIDE @9",
            "JRN-RETURN --supplies--> FEE-RIDE @12",
            "JRN-RETURN --uses--> JRN-SCAN @11",
            "JRN-RIDE --follows--> JRN-UNLOCK @9 kind alternative when released",
            "JRN-UNLOCK --uses--> JRN-SCAN @8",
        ]
    );
}

/// AC-06: no edge of the three older corpora has a label, literals
/// (`canon:`, `superseded-by`, a Markdown link, a mention) included.
#[test]
fn ac06_no_earlier_corpus_has_a_label() {
    for name in ["spec-a", "spec-b", "spec-c"] {
        let (config, input) = fixture_input(&fixture(name));
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        assert!(!graph.edges().is_empty(), "{name}");
        for edge in graph.edges() {
            assert_eq!(edge.edge, None, "{name}: {}", edge.written);
        }
    }
}

// ---------------------------------------------------------------- stability

/// One input, one result: the JSON of a labelled parse, its findings and
/// its input do not move between runs.
#[test]
fn a_labelled_parse_is_deterministic() {
    let text = leg_run();
    let first = serde_json::to_string(&parse(&text)).expect("JSON");
    let hash = norm_input(&parse(&text), text.as_bytes(), 0).expect("an input");
    for round in 0..20 {
        assert_eq!(
            serde_json::to_string(&parse(&text)).expect("JSON"),
            first,
            "round {round}"
        );
        assert_eq!(
            norm_input(&parse(&text), text.as_bytes(), 0).as_ref(),
            Some(&hash),
            "round {round}"
        );
    }
}
