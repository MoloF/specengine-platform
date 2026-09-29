//! AC-08 of docs/features/spec-parser.md: a section is a heading whose `id`
//! attribute is a definable ID; it runs to the next heading of the same or a
//! higher level (nested sections included), heading and body are disjoint,
//! attributes are exposed; `{#ID}` in a fence, an HTML comment or a
//! paragraph is no section; a repeated ID is `duplicate-id` with both kept.

mod common;

use specengine_model::{DiagnosticCode, IdScheme, Node, ParsedFile, PrefixSpec, Span};

use common::{corpus_scheme, fixture, md_files, text_of};

fn x_scheme() -> IdScheme {
    IdScheme::new(vec![PrefixSpec::number("X", "x", 1)]).unwrap()
}

const TEXT: &str = "\
# Doc

Intro.

## A {#X-1 .rule key=val rev=4 flag}

Text of A.

### B {#X-2}

Text of B.

## C

Text of C.

```text
## Fenced {#X-3}
```

<!--
## Commented {#X-3}
-->

A paragraph with {#X-3} in it.
";

fn parse(text: &str) -> ParsedFile {
    specengine_core::parse("sections.md", text.as_bytes(), &x_scheme())
}

fn section<'a>(parsed: &'a ParsedFile, id: &str) -> &'a Node {
    parsed
        .sections()
        .iter()
        .find(|s| s.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("no section {id}"))
}

fn at(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in text"))
}

#[test]
fn only_heading_attributes_define_sections() {
    let parsed = parse(TEXT);
    let ids: Vec<&str> = parsed
        .sections()
        .iter()
        .map(|s| s.id.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["X-1", "X-2"], "exactly X-1 and X-2");
    assert!(parsed.anchors.is_empty(), "{:?}", parsed.anchors);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn a_section_holds_its_subsections_and_ends_before_the_next_sibling() {
    let parsed = parse(TEXT);
    let a = section(&parsed, "X-1");
    let b = section(&parsed, "X-2");
    let text_a = text_of(TEXT.as_bytes(), a.span);
    assert!(text_a.starts_with("## A {#X-1"), "{text_a:?}");
    assert!(
        text_a.contains("### B {#X-2}"),
        "X-1 holds ### B: {text_a:?}"
    );
    assert!(
        text_a.ends_with("Text of B."),
        "trailing whitespace trimmed: {text_a:?}"
    );
    assert!(a.span.end < at(TEXT, "## C"), "X-1 ends before ## C");
    assert_eq!(a.span.end, at(TEXT, "Text of B.") + "Text of B.".len());
    assert_eq!(b.span.end, a.span.end, "B ends where A ends");
    assert_eq!(a.level, Some(2));
    assert_eq!(b.level, Some(3));
    assert_eq!(b.parent.as_ref().map(|p| p.id.as_str()), Some("X-1"));
    assert_eq!(a.parent, None, "no document ID, no enclosing section");
}

#[test]
fn heading_and_body_spans_are_disjoint_and_exact() {
    let parsed = parse(TEXT);
    for id in ["X-1", "X-2"] {
        let node = section(&parsed, id);
        let heading = node.heading.unwrap();
        let body = node.body.unwrap();
        assert!(heading.end < body.start, "{id}: disjoint");
        assert_eq!(
            &TEXT[heading.end..body.start],
            "\n",
            "{id}: body on the next line"
        );
        assert_eq!(node.span, Span::new(heading.start, body.end), "{id}");
    }
    let a = section(&parsed, "X-1");
    assert_eq!(
        text_of(TEXT.as_bytes(), a.heading.unwrap()),
        "## A {#X-1 .rule key=val rev=4 flag}"
    );
    assert!(text_of(TEXT.as_bytes(), a.body.unwrap()).starts_with("\nText of A."));
    // A rename touches the heading only: the body bytes stay the same.
    let renamed = TEXT.replace("## A {#X-1", "## A renamed {#X-1");
    let again = parse(&renamed);
    let a2 = section(&again, "X-1");
    assert_eq!(
        text_of(renamed.as_bytes(), a2.body.unwrap()),
        text_of(TEXT.as_bytes(), a.body.unwrap())
    );
    assert_eq!(a2.title.as_deref(), Some("A renamed"));
}

#[test]
fn attribute_block_is_exposed() {
    let parsed = parse(TEXT);
    let a = section(&parsed, "X-1");
    assert_eq!(
        a.title.as_deref(),
        Some("A"),
        "title without the attribute block"
    );
    assert_eq!(a.classes, ["rule"]);
    assert_eq!(
        a.attrs,
        [
            ("key".to_owned(), Some("val".to_owned())),
            ("rev".to_owned(), Some("4".to_owned())),
            ("flag".to_owned(), None),
        ]
    );
    assert_eq!(a.rev, Some(4), "rev=N attribute (Q3)");
    assert_eq!(a.kind.as_deref(), Some("x"), "kind of the prefix");
    let b = section(&parsed, "X-2");
    assert!(b.attrs.is_empty() && b.classes.is_empty() && b.rev.is_none());
}

#[test]
fn id_in_a_paragraph_is_a_mention_and_in_fence_or_comment_nothing() {
    let parsed = parse(TEXT);
    let x3: Vec<Span> = parsed
        .links
        .iter()
        .filter_map(|l| match &l.dst {
            specengine_model::LinkTarget::Reference(r) if r.id == "X-3" => r.span,
            _ => None,
        })
        .collect();
    assert_eq!(x3.len(), 1, "one X-3 mention: {:?}", parsed.links);
    assert_eq!(x3[0].start, at(TEXT, "X-3} in it"), "the paragraph one");
    assert_eq!(parsed.links[0].link_type, "mentions");
}

#[test]
fn a_repeated_id_is_duplicate_id_and_both_are_kept() {
    let text = format!("{TEXT}\n## Again {{#X-1}}\n\nSecond.\n");
    let parsed = parse(&text);
    let ids: Vec<&str> = parsed
        .sections()
        .iter()
        .map(|s| s.id.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["X-1", "X-2", "X-1"]);
    let duplicates: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::DuplicateId)
        .collect();
    assert_eq!(duplicates.len(), 1, "{:?}", parsed.diagnostics);
    let line = text[..at(&text, "## Again")].matches('\n').count() + 1;
    assert_eq!(duplicates[0].line, line);
    // A section repeating the document's own ID is a duplicate too.
    let parsed = parse("---\nid: X-5\n---\n\n## S {#X-5}\n");
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        [DiagnosticCode::DuplicateId]
    );
    assert_eq!(parsed.sections().len(), 1);
}

#[test]
fn non_id_attributes_are_anchors_and_do_not_close_or_parent_sections() {
    let text = "---\nid: X-9\n---\n\n## A {#X-1}\n\n### Notes {#notes}\n\nsee X-4\n\n## B {#b-anchor}\n\nsee X-5\n";
    let parsed = parse(text);
    let names: Vec<&str> = parsed.anchors.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["notes", "b-anchor"]);
    let a = section(&parsed, "X-1");
    assert!(text_of(text.as_bytes(), a.span).contains("see X-4"));
    assert_eq!(a.parent.as_ref().map(|p| p.id.as_str()), Some("X-9"));
    let sources: Vec<(Option<&str>, &str)> = parsed
        .links
        .iter()
        .map(|l| match &l.dst {
            specengine_model::LinkTarget::Reference(r) => (l.src.as_deref(), r.id.as_str()),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(sources, [(Some("X-1"), "X-4"), (Some("X-9"), "X-5")]);
}

#[test]
fn heading_levels_close_sections_by_rank() {
    let text = "#### Deep {#X-1}\n\na\n\n## Up {#X-2}\n\nb\n\n###### Down {#X-3}\n\nc\n\n# Top {#X-4}\n\nd\n";
    let parsed = parse(text);
    let span = |id| text_of(text.as_bytes(), section(&parsed, id).span).to_owned();
    assert_eq!(span("X-1"), "#### Deep {#X-1}\n\na");
    assert_eq!(span("X-2"), "## Up {#X-2}\n\nb\n\n###### Down {#X-3}\n\nc");
    assert_eq!(span("X-3"), "###### Down {#X-3}\n\nc");
    assert_eq!(span("X-4"), "# Top {#X-4}\n\nd");
    assert_eq!(
        section(&parsed, "X-3")
            .parent
            .as_ref()
            .map(|p| p.id.as_str()),
        Some("X-2")
    );
    assert_eq!(section(&parsed, "X-1").parent, None);
}

#[test]
fn corpus_mini_sections_file_has_only_zr_004_ending_before_the_afterword() {
    let dir = fixture("corpus-mini");
    let scheme = corpus_scheme(&dir);
    let (path, bytes) = md_files(&dir)
        .into_iter()
        .find(|(p, _)| p == "design/sections.md")
        .expect("design/sections.md");
    let text = std::str::from_utf8(&bytes).unwrap();
    // Scheme {ZR} alone, as the criterion states; and the corpus's own {ZR, ZN}.
    let only_zr = IdScheme::new(vec![PrefixSpec::number("ZR", "rule", 3)]).unwrap();
    for scheme in [only_zr, scheme] {
        let parsed = specengine_core::parse(&path, &bytes, &scheme);
        let ids: Vec<&str> = parsed
            .sections()
            .iter()
            .map(|s| s.id.as_deref().unwrap())
            .collect();
        assert_eq!(ids, ["ZR-004"]);
        let zr = &parsed.sections()[0];
        assert!(
            zr.span.end < at(text, "## Afterword"),
            "ends before ## Afterword"
        );
        assert!(
            text_of(&bytes, zr.span).ends_with("[x](missing.md)."),
            "{:?}",
            text_of(&bytes, zr.span)
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    }
}
