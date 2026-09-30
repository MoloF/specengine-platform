//! AC-01 of docs/features/spec-check-links.md: the parser records every
//! inline link and every reference definition (used or not) whose
//! destination is local as one `mentions` link with a path target, as
//! written (never resolved, never percent-decoded), with the span of the
//! destination's bytes (`<…>` and the title excluded, `?query` and
//! `#anchor` included); `src` is the innermost ID section, else the
//! document ID, else absent; the order is declared links, then the body's
//! ID mentions and file links by span start. External URLs, images,
//! autolinks, raw HTML, reference uses, code and wiki links give nothing.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use specengine_model::{
    IdScheme, Link, LinkOrigin, LinkTarget, MENTIONS, ParsedFile, PathTarget, PrefixSpec,
};

use common::{fixture, parse_str, scheme, text_of, variants};

fn rule_scheme() -> IdScheme {
    scheme(vec![
        PrefixSpec::number("R", "requirement", 2),
        PrefixSpec::name("RULE", "rule"),
        PrefixSpec::name("DOM", "domain"),
    ])
}

/// One recorded file link: `(src, path, anchor, the span's text)`.
type Seen = (Option<String>, String, Option<String>, String);

/// The inline path links of `parsed` over `bytes`; every one is a
/// `mentions` link with a span.
fn file_links(bytes: &[u8], parsed: &ParsedFile) -> Vec<Seen> {
    parsed
        .links
        .iter()
        .filter_map(|link| match &link.dst {
            LinkTarget::Path(target) if link.origin == LinkOrigin::Inline => {
                assert_eq!(link.link_type, MENTIONS, "{link:?}");
                let span = target
                    .span
                    .unwrap_or_else(|| panic!("a file link has a span: {link:?}"));
                Some((
                    link.src.clone(),
                    target.path.clone(),
                    target.anchor.clone(),
                    text_of(bytes, span).to_owned(),
                ))
            }
            _ => None,
        })
        .collect()
}

fn links_of(text: &str) -> Vec<Seen> {
    file_links(text.as_bytes(), &parse_str(text, &rule_scheme()))
}

/// One recorded destination without `src`: `(path, anchor, span text)`.
type Dest = (String, Option<String>, String);

/// `(path, anchor, span text)` of each link, without `src`.
fn dests(text: &str) -> Vec<Dest> {
    links_of(text)
        .into_iter()
        .map(|(_, path, anchor, span)| (path, anchor, span))
        .collect()
}

fn one(path: &str, anchor: Option<&str>, span: &str) -> Vec<Dest> {
    vec![(path.to_owned(), anchor.map(str::to_owned), span.to_owned())]
}

// -------------------------------------------------------------- recorded

#[test]
fn each_local_inline_destination_is_one_link_spanning_the_destination() {
    let cases: &[(&str, &str, Option<&str>, &str)] = &[
        ("[a](x.md)\n", "x.md", None, "x.md"),
        ("[a](../d/x.md#h)\n", "../d/x.md", Some("h"), "../d/x.md#h"),
        ("[a](<x y.md>)\n", "x y.md", None, "x y.md"),
        ("[a](<x y.md#h> \"t\")\n", "x y.md", Some("h"), "x y.md#h"),
        ("[a](x.md \"t\")\n", "x.md", None, "x.md"),
        ("[a](x.md 't')\n", "x.md", None, "x.md"),
        ("[a](x.md (t))\n", "x.md", None, "x.md"),
        ("[a](   x.md   )\n", "x.md", None, "x.md"),
        ("[a](x\\_y.md)\n", "x_y.md", None, "x\\_y.md"),
        ("[a](x&amp;y.md)\n", "x&y.md", None, "x&amp;y.md"),
        ("[a](x%20y.md)\n", "x%20y.md", None, "x%20y.md"),
        ("[a](x.md?plain=1)\n", "x.md", None, "x.md?plain=1"),
        (
            "[a](x.md?plain=1#L3)\n",
            "x.md",
            Some("L3"),
            "x.md?plain=1#L3",
        ),
        ("[a](x.md#)\n", "x.md", None, "x.md#"),
        ("[a](#h)\n", "", Some("h"), "#h"),
        ("[a](?q#h)\n", "", Some("h"), "?q#h"),
        ("[a](/docs/x.md)\n", "/docs/x.md", None, "/docs/x.md"),
        ("[a](x(1).md)\n", "x(1).md", None, "x(1).md"),
        ("[a](LICENSE)\n", "LICENSE", None, "LICENSE"),
        ("[a](../)\n", "../", None, "../"),
        ("[a](x.rs)\n", "x.rs", None, "x.rs"),
        ("[a](x.MD)\n", "x.MD", None, "x.MD"),
        ("*[a](x.md)*\n", "x.md", None, "x.md"),
        ("> [a](x.md)\n", "x.md", None, "x.md"),
        ("- item [a](x.md)\n", "x.md", None, "x.md"),
        ("[`](y.md` c](x.md)\n", "x.md", None, "x.md"),
        ("[a [b] c](x.md)\n", "x.md", None, "x.md"),
        ("[a\nb](x.md)\n", "x.md", None, "x.md"),
        ("[a](\n  x.md)\n", "x.md", None, "x.md"),
        ("[![i](i.png)](x.md)\n", "x.md", None, "x.md"),
        ("[](x.md)\n", "x.md", None, "x.md"),
        ("[a\\](b](x.md)\n", "x.md", None, "x.md"),
        ("[a <b>c</b> d](x.md)\n", "x.md", None, "x.md"),
        ("[a](\r\n  x.md)\r\n", "x.md", None, "x.md"),
        (
            "| a | b |\n|---|---|\n| [c](x.md) | d |\n",
            "x.md",
            None,
            "x.md",
        ),
    ];
    for &(text, path, anchor, span) in cases {
        assert_eq!(dests(text), one(path, anchor, span), "{text:?}");
    }
}

/// The span is exactly the destination's bytes: its start is where the
/// written destination starts in the file (not the start of the link, not
/// an earlier `](` inside the text).
#[test]
fn the_span_starts_at_the_destination_not_at_the_link() {
    let cases: &[(&str, &str)] = &[
        ("Intro [a](x.md) end.\n", "x.md"),
        ("[`](y.md` c](x.md)\n", "(x.md"),
        ("[a](y.md) [b](x.md)\n", "(x.md"),
        ("[a](<x y.md>)\n", "x y.md"),
        ("[a](x.md \"x.md\")\n", "x.md"),
    ];
    for &(text, marker) in cases {
        let parsed = parse_str(text, &rule_scheme());
        let spans: Vec<(usize, usize)> = parsed
            .links
            .iter()
            .filter_map(|link| match &link.dst {
                LinkTarget::Path(PathTarget { span: Some(s), .. }) => Some((s.start, s.end)),
                _ => None,
            })
            .collect();
        let at = text.find(marker).expect("marker") + usize::from(marker.starts_with('('));
        let last = *spans.last().expect("a link");
        assert_eq!(
            last,
            (at, at + 4 + 2 * usize::from(marker == "x y.md")),
            "{text:?}"
        );
    }
}

#[test]
fn a_heading_link_is_recorded_and_its_slug_is_unchanged() {
    let text = "## Title [c](x.md)\n";
    let parsed = parse_str(text, &rule_scheme());
    assert_eq!(dests(text), one("x.md", None, "x.md"));
    let plain = parse_str("## Title c\n", &rule_scheme());
    let slugs = |parsed: &ParsedFile| -> Vec<String> {
        parsed.anchors.iter().map(|a| a.name.clone()).collect()
    };
    assert_eq!(slugs(&parsed), slugs(&plain));
    assert_eq!(slugs(&parsed), ["title-c"]);
}

#[test]
fn reference_definitions_are_recorded_once_used_or_not() {
    let cases: &[(&str, Vec<Dest>)] = &[
        ("[r]: x.md\n", one("x.md", None, "x.md")),
        ("[a][r]\n\n[r]: x.md\n", one("x.md", None, "x.md")),
        (
            "[a][r], [r][] and [r].\n\n[r]: x.md#h\n",
            one("x.md", Some("h"), "x.md#h"),
        ),
        ("[r]: <x y.md> \"t\"\n", one("x y.md", None, "x y.md")),
        ("[r]:\n  x.md\n", one("x.md", None, "x.md")),
        ("> [r]: x.md\n", one("x.md", None, "x.md")),
        ("> [r]:\n> x.md\n", one("x.md", None, "x.md")),
        ("[a\\]b]: x.md\n", one("x.md", None, "x.md")),
        ("[r]: x.md\n  \"a title\"\n", one("x.md", None, "x.md")),
        ("- item\n\n  [r]: x.md\n", one("x.md", None, "x.md")),
        ("[a\nb]: x.md\n", one("x.md", None, "x.md")),
        // A label defined twice: the first definition (CommonMark).
        ("[r]: a.md\n[r]: b.md\n", one("a.md", None, "a.md")),
        ("[r]: https://h/x.md\n", vec![]),
        ("[r]: #\n", vec![]),
    ];
    for (text, want) in cases {
        assert_eq!(&dests(text), want, "{text:?}");
    }
}

#[test]
fn definitions_come_in_source_order_between_inline_links() {
    let text = "[z]: z.md\n[a]: a.md\n[m]: m.md\n\nText [b](b.md).\n\n[c]: c.md\n";
    let paths: Vec<String> = dests(text).into_iter().map(|(path, ..)| path).collect();
    assert_eq!(paths, ["z.md", "a.md", "m.md", "b.md", "c.md"]);
}

#[test]
fn a_crlf_definition_on_the_next_line_keeps_its_bytes() {
    let text = "Use [a][r].\r\n\r\n[r]:\r\n   x.md\r\n";
    let links = dests(text);
    assert_eq!(links, one("x.md", None, "x.md"));
    let parsed = parse_str(text, &rule_scheme());
    let LinkTarget::Path(target) = &parsed.links[0].dst else {
        panic!("a path link");
    };
    let at = text.find("x.md").expect("x.md");
    let span = target.span.expect("span");
    assert_eq!((span.start, span.end), (at, at + 4));
}

/// Every LF / CRLF / BOM variant keeps byte-exact spans.
#[test]
fn spans_are_file_offsets_in_every_variant() {
    let lf = "---\nid: DOM-X\n---\n\n# X\n\nSee [a](x.md#h)\nand [b][r].\n\n[r]:\n  ../y.md\n\n## S {#RULE-S}\n\n[c](<z z.md>).\n";
    for (name, bytes) in variants(lf.as_bytes()) {
        let parsed = specengine_core::parse("docs/x.md", &bytes, &rule_scheme());
        let links = file_links(&bytes, &parsed);
        let want: Vec<Seen> = vec![
            (
                Some("DOM-X".into()),
                "x.md".into(),
                Some("h".into()),
                "x.md#h".into(),
            ),
            (
                Some("DOM-X".into()),
                "../y.md".into(),
                None,
                "../y.md".into(),
            ),
            (
                Some("RULE-S".into()),
                "z z.md".into(),
                None,
                "z z.md".into(),
            ),
        ];
        assert_eq!(links, want, "{name}");
    }
}

// --------------------------------------------------------------- not recorded

#[test]
fn external_empty_image_autolink_html_code_and_wiki_give_nothing() {
    let cases: &[&str] = &[
        "[a](https://h/x.md)\n",
        "[a](HTTP://h/x.md)\n",
        "[a](mailto:a)\n",
        "[a](c:x.md)\n",
        "[a](//h/x.md)\n",
        "[a]()\n",
        "[a](<>)\n",
        "[a](#)\n",
        "[a](?q)\n",
        "![i](x.md)\n",
        "![a [b](x.md)](i.png)\n",
        "<https://h>\n",
        "<a@b.example>\n",
        "<x.md>\n",
        "<a href=\"x.md\">x</a>\n",
        "Text <a href=\"x.md\">x</a> text.\n",
        "<div>\n<a href=\"x.md\">x</a>\n</div>\n",
        "```\n[a](x.md)\n```\n",
        "~~~md\n[r]: x.md\n~~~\n",
        "    [a](x.md)\n",
        "`[a](x.md)`\n",
        "[[x]]\n",
        "[[x.md]]\n",
        "[[x.md|label]]\n",
        "[t][r]\n",
        "[r]\n",
        "<!-- [a](x.md) -->\n",
        "\\[a\\](x.md)\n",
        "---\nnote: \"[a](x.md)\"\n---\n\nBody.\n",
    ];
    for text in cases {
        assert_eq!(dests(text), [], "{text:?}");
    }
}

#[test]
fn a_destination_is_never_an_id_reference() {
    let text = "[R-01](R-02.md) and [see](#R-03) and R-04.\n";
    let parsed = parse_str(text, &rule_scheme());
    let ids: Vec<&str> = parsed
        .links
        .iter()
        .filter_map(|link| match &link.dst {
            LinkTarget::Reference(reference) => Some(reference.id.as_str()),
            LinkTarget::Path(_) => None,
        })
        .collect();
    assert_eq!(
        ids,
        ["R-01", "R-04"],
        "the link text is read, the destination never"
    );
    assert_eq!(
        dests(text),
        [
            ("R-02.md".to_owned(), None, "R-02.md".to_owned()),
            (String::new(), Some("R-03".to_owned()), "#R-03".to_owned()),
        ]
    );
}

#[test]
fn a_link_inside_a_link_text_is_the_inner_one() {
    // CommonMark: links do not nest; the inner link wins.
    assert_eq!(dests("[a [b](y.md) c](x.md)\n"), one("y.md", None, "y.md"));
}

// ------------------------------------------------------------- src and order

#[test]
fn src_is_the_innermost_id_section_then_the_document_then_none() {
    let text = "\
---
id: DOM-MOVEMENT
---

# Movement

Top [a](a.md).

## Speeds {#RULE-MOVE-SPEEDS}

Inside [b](b.md).

### Deeper {#RULE-DEEP}

Deepest [c](c.md).

## Plain

After [d](d.md).

[e]: e.md
";
    let links = links_of(text);
    let srcs: Vec<(Option<&str>, &str)> = links
        .iter()
        .map(|(src, path, ..)| (src.as_deref(), path.as_str()))
        .collect();
    assert_eq!(
        srcs,
        [
            (Some("DOM-MOVEMENT"), "a.md"),
            (Some("RULE-MOVE-SPEEDS"), "b.md"),
            (Some("RULE-DEEP"), "c.md"),
            (Some("DOM-MOVEMENT"), "d.md"),
            (Some("DOM-MOVEMENT"), "e.md"),
        ]
    );
    // No document ID, outside every ID section: no `src`.
    let loose = links_of("# Notes\n\nSee [a](a.md).\n\n## S {#RULE-S}\n\n[b](b.md)\n");
    let srcs: Vec<Option<&str>> = loose.iter().map(|(src, ..)| src.as_deref()).collect();
    assert_eq!(srcs, [None, Some("RULE-S")]);
}

#[test]
fn declared_links_first_then_mentions_and_file_links_by_position() {
    let text = "\
---
id: DOM-X
refs: [R-09]
---

# X

R-01 then [a](a.md) then R-02 [b][r] R-03.

[r]: b.md
";
    let parsed = parse_str(text, &rule_scheme());
    let order: Vec<String> = parsed
        .links
        .iter()
        .map(|link: &Link| {
            let origin = match link.origin {
                LinkOrigin::Frontmatter => "fm",
                LinkOrigin::Inline => "in",
            };
            let dst = match &link.dst {
                LinkTarget::Reference(reference) => reference.id.clone(),
                LinkTarget::Path(target) => target.path.clone(),
            };
            format!("{origin} {dst}")
        })
        .collect();
    assert_eq!(
        order,
        [
            "fm R-09", "in R-01", "in a.md", "in R-02", "in R-03", "in b.md"
        ]
    );
}

// ------------------------------------------------------------ fixture data

/// The spec's "Data" JSON: spec-a's movement README (ord 0 and 1, before
/// the `MEC-STAMINA` mention) and corpus-mini's ID-less notes.
#[test]
fn fixture_links_serialise_as_the_spec_data() {
    let spec_a = fixture("spec-a");
    let scheme = common::corpus_scheme(&spec_a);
    let path = "docs/spec/movement/README.md";
    let bytes = std::fs::read(spec_a.join(path)).expect("README");
    let parsed = specengine_core::parse(path, &bytes, &scheme);
    let json: Vec<String> = parsed
        .links
        .iter()
        .map(|link| serde_json::to_string(link).expect("JSON"))
        .collect();
    assert_eq!(
        json[..2],
        [
            r#"{"src":"DOM-MOVEMENT","type":"mentions","origin":"inline","dst":{"path":"stamina.md","span":[180,190]}}"#,
            r#"{"src":"DOM-MOVEMENT","type":"mentions","origin":"inline","dst":{"path":"sprint.md","span":[202,211]}}"#,
        ]
    );
    assert_eq!(json.len(), 3, "then the MEC-STAMINA mention: {json:#?}");

    let mini = fixture("corpus-mini");
    let scheme = common::corpus_scheme(&mini);
    let path = "design/notes.md";
    let bytes = std::fs::read(mini.join(path)).expect("notes");
    let parsed = specengine_core::parse(path, &bytes, &scheme);
    let json: Vec<String> = parsed
        .links
        .iter()
        .map(|link| serde_json::to_string(link).expect("JSON"))
        .collect();
    assert_eq!(
        json,
        [
            r#"{"type":"mentions","origin":"inline","dst":{"path":"rules.md","span":[80,88]}}"#,
            r#"{"type":"mentions","origin":"inline","dst":{"path":"sections.md","anchor":"a-rule-written-as-a-section","span":[111,150]}}"#,
        ]
    );
}

/// A Cyrillic destination keeps its bytes: the path as written and the
/// span over the raw UTF-8.
#[test]
fn a_non_latin_destination_is_kept_as_written() {
    let text = "[a](\u{0434}\u{043e}\u{043a}.md#\u{0440}\u{0430}\u{0437})\n";
    assert_eq!(
        dests(text),
        one(
            "\u{0434}\u{043e}\u{043a}.md",
            Some("\u{0440}\u{0430}\u{0437}"),
            "\u{0434}\u{043e}\u{043a}.md#\u{0440}\u{0430}\u{0437}"
        )
    );
}

// ------------------------------------------------ iteration 3 regressions

/// Vertical tab and form feed are blanks before a destination (as for
/// pulldown-cmark), before and after its line ending and after `>`
/// markers: one link each, the span exactly the destination. In a debug
/// build an unlocated destination is a `debug_assert!` panic, so these also
/// pin "no panic".
#[test]
fn vertical_tab_and_form_feed_before_a_destination_are_passed_over() {
    let cases: &[(&str, &str)] = &[
        ("[a](\x0Cx.md)\n", "x.md"),
        ("[a](\x0B\tx.md)\n", "x.md"),
        ("[a](\n\x0C x.md)\n", "x.md"),
        ("[a](\x0B\n\x0Bx.md)\n", "x.md"),
        ("[r]:\x0Cx.md\n", "x.md"),
        ("[r]:\x0B\n\x0Cx.md\n", "x.md"),
        ("> [r]:\n>\x0Cq.md\n", "q.md"),
        ("> [a](\n>\x0B q.md)\n", "q.md"),
    ];
    for &(text, want) in cases {
        let parsed = parse_str(text, &rule_scheme());
        let spans: Vec<(usize, usize, String)> = parsed
            .links
            .iter()
            .filter_map(|link| match &link.dst {
                LinkTarget::Path(target) => {
                    let span = target.span.expect("a span");
                    Some((span.start, span.end, target.path.clone()))
                }
                LinkTarget::Reference(_) => None,
            })
            .collect();
        let at = text.rfind(want).expect("the destination");
        assert_eq!(spans, [(at, at + want.len(), want.to_owned())], "{text:?}");
    }
}

/// DEL (`0x7F`) is part of a bare destination (it ends at a byte
/// `<= 0x20`, as pulldown-cmark's): path and span keep it.
#[test]
fn del_is_part_of_a_bare_destination() {
    let cases: &[(&str, &str)] = &[
        ("[a](x\x7F.md)\n", "x\x7F.md"),
        ("[r]: x\x7Fy.md\n", "x\x7Fy.md"),
        ("[a](x\x7F.md#h\x7F)\n", "x\x7F.md#h\x7F"),
    ];
    for &(text, written) in cases {
        let links = dests(text);
        assert_eq!(links.len(), 1, "{text:?}: {links:?}");
        let (path, _, span) = &links[0];
        assert_eq!(span, written, "{text:?}");
        assert!(
            path.contains('\x7F') && path.ends_with(".md"),
            "{text:?}: {path:?}"
        );
    }
}

// ------------------------------------------------ iteration 4: known limit

/// The destination re-scan passes `>` after a line ending as a blockquote
/// marker; pulldown-cmark reads a `>` on an indented continuation line as
/// destination text. A destination the re-scan cannot locate is silently
/// not recorded (no panic in any build); one it locates past the `>` is
/// recorded with the path pulldown-cmark gives and the span after the `>`.
#[test]
fn a_gt_on_an_indented_continuation_line_is_not_located() {
    for text in [
        "[a](\n    >)\n",
        "[r]:\n    >\n",
        "[a](\r\n    >)\r\n",
        "Text.\n\n[r]:\n    >\n\nMore.\n",
    ] {
        let parsed = parse_str(text, &rule_scheme());
        let paths: Vec<&LinkTarget> = parsed
            .links
            .iter()
            .map(|link| &link.dst)
            .filter(|dst| matches!(dst, LinkTarget::Path(_)))
            .collect();
        assert!(paths.is_empty(), "{text:?}: {paths:?}");
    }
    // Known limit: the path is `>x.md` (pulldown-cmark's destination), the
    // span, hence the subject a finding would show, is `x.md`.
    for text in ["[a](\n    >x.md)\n", "[r]:\n    >x.md\n"] {
        assert_eq!(dests(text), one(">x.md", None, "x.md"), "{text:?}");
    }
}
