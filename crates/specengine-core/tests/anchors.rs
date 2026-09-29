//! AC-02 of docs/features/spec-check.md: anchors. Every heading gives a
//! `slug` (GitHub's: the inline text, `{#…}` removed, lower-cased, letters
//! and digits of any script, `-`, `_` kept, whitespace → `-`, the rest
//! dropped; repeats `-1`, `-2`; empty → none), a non-ID `{#…}` gives an
//! `attr`, `<a id>` / `<a name>` give an `html` whose span is the start tag;
//! nothing comes from code blocks, code spans or HTML comments. Anchors are
//! in source order, a heading's slug before its attribute.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use specengine_model::{Anchor, AnchorOrigin, IdScheme, ParsedFile, PrefixSpec};

use common::text_of;

fn x_scheme() -> IdScheme {
    IdScheme::new(vec![PrefixSpec::number("X", "x", 1)]).unwrap()
}

fn parse(text: &str) -> ParsedFile {
    specengine_core::parse("anchors.md", text.as_bytes(), &x_scheme())
}

fn names(parsed: &ParsedFile) -> Vec<(AnchorOrigin, &str)> {
    parsed
        .anchors
        .iter()
        .map(|a| (a.origin, a.name.as_str()))
        .collect()
}

fn slugs(text: &str) -> Vec<String> {
    parse(text)
        .anchors
        .iter()
        .filter(|a| a.origin == AnchorOrigin::Slug)
        .map(|a| a.name.clone())
        .collect()
}

use AnchorOrigin::{Attr, Html, Slug};

#[test]
fn every_heading_has_a_slug_with_its_level_and_heading_span() {
    let text = "# Lantern Keep\n\nIntro.\n\n## Core loop\n\ntext\n\n### Deep dive\n\nSetext title\n------------\n";
    let parsed = parse(text);
    let got: Vec<(AnchorOrigin, &str, Option<u8>, &str)> = parsed
        .anchors
        .iter()
        .map(|a| {
            (
                a.origin,
                a.name.as_str(),
                a.level,
                text_of(text.as_bytes(), a.span),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (Slug, "lantern-keep", Some(1), "# Lantern Keep"),
            (Slug, "core-loop", Some(2), "## Core loop"),
            (Slug, "deep-dive", Some(3), "### Deep dive"),
            (Slug, "setext-title", Some(2), "Setext title\n------------"),
        ]
    );
}

#[test]
fn repeats_get_numbered_suffixes_like_github() {
    assert_eq!(slugs("# A\n\n## A\n\n## A\n"), ["a", "a-1", "a-2"]);
    // github-slugger: a slug already given is skipped when numbering.
    assert_eq!(slugs("## A\n\n## A\n\n## A-1\n"), ["a", "a-1", "a-1-1"]);
    assert_eq!(
        slugs("## Notes\n\n## Notes\n\n## notes\n\n## NOTES\n"),
        ["notes", "notes-1", "notes-2", "notes-3"]
    );
}

#[test]
fn the_slug_is_made_of_the_inline_text_only() {
    // Link text stays, the destination goes; code keeps its text; inline
    // HTML and the attribute block go; punctuation is dropped, `-` and `_`
    // kept, each space a `-`.
    assert_eq!(
        slugs("## See [the guide](https://example.org/a_b) now\n"),
        ["see-the-guide-now"]
    );
    assert_eq!(slugs("## The `foo_bar` key\n"), ["the-foo_bar-key"]);
    assert_eq!(slugs("## Title <em>x</em> end\n"), ["title-x-end"]);
    assert_eq!(slugs("## What's new? (v2.0)\n"), ["whats-new-v20"]);
    assert_eq!(slugs("## Flag --dry-run\n"), ["flag---dry-run"]);
    assert_eq!(slugs("## Rules {#X-1}\n"), ["rules"]);
    assert_eq!(slugs("## Rules {#rules-here .note}\n"), ["rules"]);
    assert_eq!(slugs("## A *bold* _move_\n"), ["a-bold-move"]);
}

#[test]
fn letters_of_any_script_are_kept_and_lower_cased() {
    // "Command Sync" and "Fir 2" in Russian, each with a Cyrillic capital.
    let text = "## \u{041a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430} Sync\n\n## \u{0401}\u{043b}\u{043a}\u{0430} 2\n";
    assert_eq!(
        slugs(text),
        [
            "\u{043a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430}-sync",
            "\u{0451}\u{043b}\u{043a}\u{0430}-2",
        ]
    );
}

#[test]
fn a_heading_that_slugs_to_nothing_has_no_slug() {
    assert!(slugs("## ???\n\n## !\n").is_empty());
    // An empty slug takes no number from the next one.
    assert_eq!(slugs("## ???\n\n## A\n\n## A\n"), ["a", "a-1"]);
}

#[test]
fn a_non_id_attribute_is_an_attr_after_its_slug_and_an_id_attribute_is_none() {
    let text = "## Glossary {#glossary}\n\n## Rules {#X-1}\n\n### Notes {#notes .x}\n";
    let parsed = parse(text);
    let got: Vec<(AnchorOrigin, &str, Option<u8>)> = parsed
        .anchors
        .iter()
        .map(|a| (a.origin, a.name.as_str(), a.level))
        .collect();
    assert_eq!(
        got,
        [
            (Slug, "glossary", Some(2)),
            (Attr, "glossary", Some(2)),
            (Slug, "rules", Some(2)),
            (Slug, "notes", Some(3)),
            (Attr, "notes", Some(3)),
        ]
    );
    // The attr's span is its heading, the same as the slug's.
    let glossary: Vec<&Anchor> = parsed.anchors.iter().take(2).collect();
    assert_eq!(glossary[0].span, glossary[1].span);
    assert_eq!(
        text_of(text.as_bytes(), glossary[1].span),
        "## Glossary {#glossary}"
    );
}

#[test]
fn html_a_id_and_a_name_are_anchors_spanning_the_start_tag() {
    let text = "# Doc\n\n<a id=\"process\"></a>\n## Process\n\nText <a name='legacy'>here</a> and <A ID=upper>x</A>.\n\n<a href=\"#x\">not an anchor</a> <a id=\"\"></a>\n";
    let parsed = parse(text);
    assert_eq!(
        names(&parsed),
        [
            (Slug, "doc"),
            (Html, "process"),
            (Slug, "process"),
            (Html, "legacy"),
            (Html, "upper"),
        ],
        "{:?}",
        parsed.anchors
    );
    let tags: Vec<(&str, Option<u8>)> = parsed
        .anchors
        .iter()
        .filter(|a| a.origin == Html)
        .map(|a| (text_of(text.as_bytes(), a.span), a.level))
        .collect();
    assert_eq!(
        tags,
        [
            ("<a id=\"process\">", None),
            ("<a name='legacy'>", None),
            ("<A ID=upper>", None),
        ]
    );
}

#[test]
fn nothing_comes_from_code_or_html_comments() {
    let text = "\
# Real

```markdown
# Fenced heading
<a id=\"fenced\"></a>
```

    # Indented code
    <a id=\"indented\"></a>

<!--
# Commented heading
<a id=\"commented\"></a>
-->

Inline `<a id=\"code-span\">` and <!-- <a id=\"inline-comment\"> --> text.

<div>
<!-- <a id=\"block-comment\"> -->
<a id=\"kept\"></a>
</div>
";
    let parsed = parse(text);
    assert_eq!(
        names(&parsed),
        [(Slug, "real"), (Html, "kept")],
        "{:?}",
        parsed.anchors
    );
}

#[test]
fn anchors_serialise_with_origin_level_and_span() {
    let parsed = parse("## License\n\n<a id=\"x\"></a>\n");
    let json = serde_json::to_value(&parsed.anchors).unwrap();
    assert_eq!(
        json,
        serde_json::json!([
            {"name": "license", "origin": "slug", "level": 2, "span": [0, 10]},
            {"name": "x", "origin": "html", "span": [12, 22]},
        ])
    );
}

#[test]
fn the_fixture_canon_targets_resolve_through_slugs() {
    // spec-a DEC-0023 `canon: docs/spec/movement/stamina.md#regeneration` and
    // spec-b ADR-0001 `canon: docs/spec/cli.md#<Cyrillic slug>`: both headings
    // carry an ID attribute, so only the slug can be the anchor.
    for (corpus, file, anchor) in [
        ("spec-a", "docs/spec/movement/stamina.md", "regeneration"),
        (
            "spec-b",
            "docs/spec/cli.md",
            "\u{043a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430}-sync",
        ),
    ] {
        let root = common::fixture(corpus);
        let scheme = common::corpus_scheme(&root);
        let bytes = std::fs::read(root.join(file)).unwrap();
        let parsed = specengine_core::parse(file, &bytes, &scheme);
        assert!(
            parsed
                .anchors
                .iter()
                .any(|a| a.origin == Slug && a.name == anchor),
            "{corpus}/{file}: no slug {anchor:?} in {:?}",
            parsed.anchors
        );
    }
}

/// AC-10 of docs/features/phase1-cleanup.md (K5): `<!-->` and `<!--->` are
/// complete comments (CommonMark 0.31), so an `<a id>` after one is an
/// anchor; one inside `<!-- … -->` still is not.
#[test]
fn empty_comments_close_themselves() {
    for comment in ["<!-->", "<!--->"] {
        for text in [
            format!("<div>\n{comment}\n<a id=\"after\"></a>\n</div>\n"),
            format!("Text {comment} and <a id=\"after\"></a> here.\n"),
        ] {
            let parsed = parse(&text);
            assert_eq!(
                names(&parsed),
                [(Html, "after")],
                "{text:?}: {:?}",
                parsed.anchors
            );
            let anchor = &parsed.anchors[0];
            assert_eq!(text_of(text.as_bytes(), anchor.span), "<a id=\"after\">");
        }
    }
    for text in [
        "<div>\n<!-- <a id=\"inside\"></a> -->\n<a id=\"after\"></a>\n</div>\n",
        "<div>\n<!--\n<a id=\"inside\"></a>\n--->\n<a id=\"after\"></a>\n</div>\n",
    ] {
        let parsed = parse(text);
        assert_eq!(
            names(&parsed),
            [(Html, "after")],
            "{text:?}: {:?}",
            parsed.anchors
        );
    }
}
