//! AC-12 of docs/features/spec-parser.md: declared links come from
//! front-matter (`links:` items, `supersedes`, `working_answer`, `canon`,
//! `status: superseded-by X`), `refs`/`adrs` are mentions, `parent:` is
//! containment on the node and no link; body citations are `mentions`,
//! origin `inline`, from the innermost ID section around them.
//!
//! The 05 §2.1 mechanic is `fixtures/spec-a/docs/spec/movement/stamina.md`
//! (with `uses_term`, and `R-12` cited inside `{#RULE-STAM-REGEN}`).

mod common;

use specengine_model::{
    DiagnosticCode, IdScheme, Link, LinkOrigin, LinkTarget, ParsedFile, Severity,
};

use common::{corpus_scheme, fixture, md_files, render_link, text_of};

fn spec_a() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

fn spec_a_file(suffix: &str) -> (String, Vec<u8>) {
    md_files(&fixture("spec-a"))
        .into_iter()
        .find(|(p, _)| p.ends_with(suffix))
        .unwrap_or_else(|| panic!("no {suffix} in spec-a"))
}

fn parse(path: &str, bytes: &[u8]) -> ParsedFile {
    specengine_core::parse(path, bytes, &spec_a())
}

fn dst_id(link: &Link) -> &str {
    match &link.dst {
        LinkTarget::Reference(r) => &r.id,
        LinkTarget::Path(p) => &p.path,
    }
}

#[test]
fn mechanic_declares_its_links_in_front_matter_with_spans() {
    let (path, bytes) = spec_a_file("movement/stamina.md");
    let parsed = parse(&path, &bytes);
    let declared: Vec<&Link> = parsed
        .links
        .iter()
        .filter(|l| l.origin == LinkOrigin::Frontmatter)
        .collect();
    let rendered: Vec<String> = declared.iter().map(|l| render_link(l)).collect();
    assert_eq!(
        rendered,
        [
            "MEC-STAMINA derived_from frontmatter R-12",
            "MEC-STAMINA derived_from frontmatter A-101",
            "MEC-STAMINA depends_on frontmatter MEC-SPRINT",
            "MEC-STAMINA uses_term frontmatter TERM-exhausted",
        ]
    );
    for link in &declared {
        let LinkTarget::Reference(reference) = &link.dst else {
            panic!("reference expected")
        };
        let span = reference
            .span
            .expect("a verbatim front-matter reference has a span");
        assert_eq!(text_of(&bytes, span), reference.id);
        assert!(
            parsed.front_matter.unwrap().contains(span),
            "the span lies in the front-matter"
        );
    }
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn parent_is_containment_on_the_node_not_a_link() {
    let (path, bytes) = spec_a_file("movement/stamina.md");
    let parsed = parse(&path, &bytes);
    let parent = parsed.document().unwrap().parent.as_ref().expect("parent");
    assert_eq!(parent.id, "DOM-MOVEMENT");
    assert_eq!(text_of(&bytes, parent.span.expect("span")), "DOM-MOVEMENT");
    assert!(
        parsed.links.iter().all(|l| dst_id(l) != "DOM-MOVEMENT"),
        "no link to the parent: {:?}",
        parsed.links
    );
    assert!(parsed.links.iter().all(|l| l.link_type != "parent"));
}

#[test]
fn a_citation_in_a_section_is_a_mention_from_that_section() {
    let (path, bytes) = spec_a_file("movement/stamina.md");
    let parsed = parse(&path, &bytes);
    let inline: Vec<&Link> = parsed
        .links
        .iter()
        .filter(|l| l.origin == LinkOrigin::Inline)
        .collect();
    assert_eq!(inline.len(), 1, "{inline:?}");
    let link = inline[0];
    assert_eq!(link.link_type, "mentions");
    assert_eq!(link.src.as_deref(), Some("RULE-STAM-REGEN"));
    assert_eq!(dst_id(link), "R-12");
    let LinkTarget::Reference(reference) = &link.dst else {
        unreachable!()
    };
    let span = reference.span.unwrap();
    let section = &parsed.sections()[0];
    assert_eq!(section.id.as_deref(), Some("RULE-STAM-REGEN"));
    assert!(
        section.body.unwrap().contains(span),
        "cited inside the section body"
    );
}

#[test]
fn innermost_section_is_the_source_and_outside_sections_the_document() {
    let (path, bytes) = spec_a_file("movement/sprint.md");
    let parsed = parse(&path, &bytes);
    let sources: Vec<(Option<&str>, &str)> = parsed
        .links
        .iter()
        .filter(|l| l.origin == LinkOrigin::Inline)
        .map(|l| (l.src.as_deref(), dst_id(l)))
        .collect();
    assert_eq!(
        sources,
        [
            (Some("MEC-SPRINT"), "MEC-STAMINA"),
            (Some("RULE-SPRINT-COST"), "R-12"),
            (Some("RULE-SPRINT-COST"), "A-101"),
            (Some("EDGE-SPRINT-EMPTY"), "EDGE-STAM-ZERO"),
            (Some("EDGE-SPRINT-EMPTY"), "Q-031"),
            (Some("MEC-SPRINT"), "AC-07"),
            (Some("MEC-SPRINT"), "DEC-0023"),
        ]
    );
}

#[test]
fn unknown_link_type_is_a_warning_and_the_link_is_kept() {
    let (path, bytes) = spec_a_file("movement/stamina.md");
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace("uses_term:", "uses_terms:");
    let parsed = parse(&path, text.as_bytes());
    let unknown: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::UnknownLinkType)
        .collect();
    assert_eq!(unknown.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(unknown[0].severity, Severity::Warning);
    assert_eq!(unknown[0].line, 14, "the uses_terms line");
    assert!(
        parsed
            .links
            .iter()
            .any(|l| render_link(l) == "MEC-STAMINA uses_terms frontmatter TERM-exhausted")
    );
    let links = parsed
        .document()
        .unwrap()
        .fields
        .as_ref()
        .unwrap()
        .links
        .as_ref()
        .unwrap();
    let types: Vec<&str> = links.iter().map(|(t, _)| t).collect();
    assert_eq!(
        types,
        ["derived_from", "depends_on", "uses_terms"],
        "kept in source order"
    );
}

#[test]
fn every_closed_link_type_is_accepted() {
    let mut yaml = String::from("---\nid: MEC-X\nlinks:\n");
    for link_type in specengine_model::LINK_TYPES {
        yaml.push_str(&format!("  {link_type}: [R-12]\n"));
    }
    yaml.push_str("---\n");
    let parsed = parse("x.md", yaml.as_bytes());
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let types: Vec<&str> = parsed.links.iter().map(|l| l.link_type.as_str()).collect();
    assert_eq!(types, specengine_model::LINK_TYPES);
}

#[test]
fn decision_links_canon_supersedes_and_superseded_by() {
    let (path, bytes) = spec_a_file("DEC/DEC-0023.md");
    let parsed = parse(&path, &bytes);
    let canon = parsed
        .links
        .iter()
        .find(|l| l.link_type == "canon")
        .expect("canon link");
    let LinkTarget::Path(target) = &canon.dst else {
        panic!("canon: path#anchor is a path target")
    };
    assert_eq!(target.path, "docs/spec/movement/stamina.md");
    assert_eq!(target.anchor.as_deref(), Some("regeneration"));
    assert_eq!(
        text_of(&bytes, target.span.unwrap()),
        "docs/spec/movement/stamina.md#regeneration"
    );

    let (path, bytes) = spec_a_file("DEC/DEC-0007.md");
    let parsed = parse(&path, &bytes);
    let link = &parsed.links[0];
    assert_eq!(
        render_link(link),
        "DEC-0023 supersedes frontmatter DEC-0007"
    );
    assert_eq!(
        text_of(&bytes, link.src_span.expect("src_span")),
        "DEC-0023"
    );
    let LinkTarget::Reference(reference) = &link.dst else {
        unreachable!()
    };
    assert_eq!(
        text_of(&bytes, reference.span.unwrap()),
        "DEC-0007",
        "the id: value"
    );
}

#[test]
fn refs_and_adrs_are_mentions_from_front_matter() {
    let (path, bytes) = spec_a_file("Q/Q-031.md");
    let parsed = parse(&path, &bytes);
    let declared: Vec<String> = parsed
        .links
        .iter()
        .filter(|l| l.origin == LinkOrigin::Frontmatter)
        .map(render_link)
        .collect();
    assert_eq!(
        declared,
        [
            "Q-031 working_answer frontmatter A-101",
            "Q-031 mentions frontmatter R-12",
            "Q-031 mentions frontmatter MEC-STAMINA",
        ]
    );
}
