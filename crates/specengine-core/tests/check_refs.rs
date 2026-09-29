//! AC-10 and AC-11 of docs/features/spec-check.md: front-matter references
//! and `canon:`. A reference resolves to a defined ID, to a document's
//! `aliases:` entry, or through `aliases_from` (prefix + written body, no
//! re-padding); `#Y` names a section of the ID's file; inline mentions,
//! `project:` and `slug/` are not judged here. A path-form `canon:`, on any
//! document, is `path#anchor` naming a walked canon document and one of its
//! anchors or section IDs.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use common::check::{Config, fixture_input, show, with_code};
use common::fixture;

/// `\u{0422}\u{0420}\u{0411}` (TRB), spec-b's legacy prefix of `REQ`.
const TRB: &str = "\u{0422}\u{0420}\u{0411}";
/// `\u{0412}\u{041e}\u{041f}` (VOP), spec-b's legacy prefix of `QN`.
const VOP: &str = "\u{0412}\u{041e}\u{041f}";

fn dangling(report: &specengine_core::check::Report) -> Vec<(String, String)> {
    with_code(report, "ref-dangling")
        .iter()
        .map(|f| (f.path.clone(), f.subject.clone()))
        .collect()
}

// ------------------------------------------------------------------ AC-10

#[test]
fn spec_a_references_resolve_including_the_legacy_alias() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let tuning = input
        .files
        .iter()
        .find(|f| f.path == "docs/features/stamina-tuning.md")
        .expect("stamina-tuning.md");
    let text = String::from_utf8(tuning.bytes.clone()).unwrap();
    assert!(
        text.contains("refs: [R-12, QST-031]"),
        "the fixture cites both"
    );
    let report = config.run(&input);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
    assert!(
        with_code(&report, "canon-anchor").is_empty(),
        "{}",
        show(&report)
    );
}

#[test]
fn spec_b_resolves_all_but_its_two_aliases_without_a_target() {
    let (config, input) = fixture_input(&fixture("spec-b"));
    for (path, needle) in [
        (
            "docs/records/ADR/ADR-0002.md",
            "status: superseded-by ADR-0001".to_owned(),
        ),
        (
            "docs/records/ADR/ADR-0001.md",
            "adrs: [ADR-0002]".to_owned(),
        ),
        ("docs/spec/cli.md", format!("{TRB}-002")),
        ("docs/features/dry-run.md", format!("{TRB}-003")),
        ("docs/records/QN/QN-07.md", format!("{VOP}-6")),
    ] {
        let file = input.files.iter().find(|f| f.path == path).unwrap();
        assert!(
            String::from_utf8_lossy(&file.bytes).contains(&needle),
            "{path} holds {needle}"
        );
    }
    let report = config.run(&input);
    assert_eq!(
        dangling(&report),
        [
            ("docs/features/dry-run.md".to_owned(), format!("{TRB}-003")),
            ("docs/records/QN/QN-07.md".to_owned(), format!("{VOP}-6")),
        ],
        "{}",
        show(&report)
    );
    let lines: Vec<usize> = with_code(&report, "ref-dangling")
        .iter()
        .map(|f| f.line)
        .collect();
    assert_eq!(lines, [6, 9], "the lines of the `refs:` values");
}

/// spec-b's scheme with its `aliases_from`, a record declaring a legacy
/// alias, and the command module with its sections.
fn spec_b_like() -> Config {
    Config::from_toml(&format!(
        "[ids]\nREQ = {{ kind = \"requirement\", width = 3, aliases_from = [\"{TRB}\"] }}\nQN = {{ kind = \"question\", width = 2, aliases_from = [\"{VOP}\"] }}\nMOD = {{ kind = \"module\", shape = \"name\" }}\nCMD = {{ kind = \"command\", shape = \"name\" }}\n"
    ))
}

fn module() -> (&'static str, &'static str) {
    (
        "docs/cli.md",
        "---\nid: MOD-CLI\n---\n# CLI\n\n## Sync {#CMD-SYNC}\n\ntext\n",
    )
}

#[test]
fn a_declared_alias_resolves_and_a_missing_section_dangles() {
    let config = spec_b_like();
    let question = format!("---\nid: QN-07\naliases: [{VOP}-7]\n---\n# Q\n");
    let citing = format!(
        "---\nid: REQ-001\nrefs: [{VOP}-7, MOD-CLI#CMD-SYNC, MOD-CLI#CMD-NOPE]\n---\n# R\n"
    );
    let report = config.check(&[
        module(),
        ("docs/qn-07.md", &question),
        ("docs/req-001.md", &citing),
    ]);
    assert_eq!(
        dangling(&report),
        [("docs/req-001.md".to_owned(), "MOD-CLI#CMD-NOPE".to_owned())],
        "{}",
        show(&report)
    );
    let finding = with_code(&report, "ref-dangling")[0];
    assert!(finding.message.contains("CMD-NOPE"), "{}", finding.message);
    assert_eq!(finding.line, 3);
}

#[test]
fn aliases_from_takes_the_written_body_without_re_padding() {
    let config = spec_b_like();
    let citing = format!("---\nid: REQ-001\nrefs: [{TRB}-002, {TRB}-2]\n---\n# R\n");
    let report = config.check(&[
        ("docs/req-002.md", "---\nid: REQ-002\n---\n# R2\n"),
        ("docs/req-001.md", &citing),
    ]);
    assert_eq!(
        dangling(&report),
        [("docs/req-001.md".to_owned(), format!("{TRB}-2"))],
        "{}",
        show(&report)
    );
}

#[test]
fn every_reference_key_is_judged() {
    let config = Config::from_toml(
        "[ids]\nADR = { kind = \"decision\", width = 4 }\nQ = { kind = \"question\", width = 3 }\nA = { kind = \"assumption\", width = 3 }\nR = { kind = \"requirement\", width = 2 }\n",
    );
    let report = config.check(&[
        (
            "docs/adr-0001.md",
            "---\nid: ADR-0001\nstatus: superseded-by ADR-0099\nsupersedes: [ADR-0098]\nadrs: [ADR-0097]\n---\n# D\n",
        ),
        (
            "docs/q-001.md",
            "---\nid: Q-001\nworking_answer: A-009\nrefs: [R-99]\nlinks:\n  depends_on: [R-98]\n---\n# Q\n",
        ),
        (
            "docs/r-01.md",
            "---\nid: R-01\nparent: R-97\ncanon: ADR-0096\n---\n# R\n",
        ),
    ]);
    let mut got = dangling(&report);
    got.sort();
    let want: Vec<(String, String)> = [
        ("docs/adr-0001.md", "ADR-0097"),
        ("docs/adr-0001.md", "ADR-0098"),
        ("docs/adr-0001.md", "ADR-0099"),
        ("docs/q-001.md", "A-009"),
        ("docs/q-001.md", "R-98"),
        ("docs/q-001.md", "R-99"),
        ("docs/r-01.md", "ADR-0096"),
        ("docs/r-01.md", "R-97"),
    ]
    .iter()
    .map(|(p, s)| ((*p).to_owned(), (*s).to_owned()))
    .collect();
    assert_eq!(got, want, "{}", show(&report));
}

#[test]
fn an_alias_parent_resolves_and_inline_mentions_project_and_slug_are_not_judged() {
    // spec-a's own scheme: `QST` is a legacy prefix of `Q`.
    let text = std::fs::read_to_string(fixture("spec-a").join("specengine.toml")).unwrap();
    let config = Config::from_toml(&text);
    let report = config.check(&[
        ("docs/records/Q/Q-031.md", "---\nid: Q-031\n---\n# Q\n"),
        (
            "docs/records/A/A-103.md",
            "---\nid: A-103\nparent: QST-031\nrefs: [shared:R-77, stamina-tuning/AC-07]\n---\n# A\n\nSee R-55 and QST-099 inline.\n",
        ),
    ]);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
    // The same parent without its target dangles: the alias is really read.
    let report = config.check(&[(
        "docs/records/A/A-103.md",
        "---\nid: A-103\nparent: QST-031\n---\n# A\n",
    )]);
    assert_eq!(
        dangling(&report),
        [("docs/records/A/A-103.md".to_owned(), "QST-031".to_owned())],
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------------ AC-11

#[test]
fn spec_b_canon_values_resolve_by_cyrillic_slug_and_by_reference() {
    let (config, input) = fixture_input(&fixture("spec-b"));
    let adr = |path: &str| {
        String::from_utf8(
            input
                .files
                .iter()
                .find(|f| f.path == path)
                .unwrap()
                .bytes
                .clone(),
        )
        .unwrap()
    };
    // `docs/spec/cli.md#` + the slug of the Cyrillic heading "Command sync".
    assert!(adr("docs/records/ADR/ADR-0001.md").contains(
        "canon: docs/spec/cli.md#\u{043a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430}-sync"
    ));
    assert!(adr("docs/records/ADR/ADR-0002.md").contains("canon: MOD-CLI#CMD-SYNC"));
    let report = config.run(&input);
    for code in ["canon-form", "canon-file", "canon-anchor", "canon-missing"] {
        assert!(
            with_code(&report, code).is_empty(),
            "{code}:\n{}",
            show(&report)
        );
    }
    assert!(
        dangling(&report)
            .iter()
            .all(|(path, _)| !path.starts_with("docs/records/ADR/")),
        "{}",
        show(&report)
    );
}

const CANON_DOC: &str = "\
---
class: canon
owner: owner
reviewed: 2026-09-01
---

# Architecture

<a id=\"storage\"></a>
## Storage rules

## Rule {#R-01}
";

#[test]
fn a_path_canon_names_a_walked_canon_document_and_one_of_its_anchors() {
    let config = Config::from_toml(
        "[ids]\nADR = { kind = \"decision\", width = 4 }\nR = { kind = \"requirement\", width = 2 }\n",
    );
    let decision = |n: u32, canon: &str| {
        (
            format!("docs/decisions/ADR-{n:04}.md"),
            format!(
                "---\nid: ADR-{n:04}\nclass: decision\nstatus: accepted\nscope: [x]\ncanon: {canon}\n---\n# D\n"
            ),
        )
    };
    let cases = [
        decision(1, "docs/canon/architecture.md#storage"),
        decision(2, "docs/canon/architecture.md#storage-rules"),
        decision(3, "docs/canon/architecture.md#R-01"),
        decision(4, "docs/canon/architecture.md"),
        decision(5, "docs/canon/missing.md#storage"),
        decision(6, "docs/features/spec.md#why"),
        decision(7, "docs/canon/architecture.md#nope"),
        // Class-less: the rule holds on any document.
        (
            "docs/decisions/ADR-0008.md".to_owned(),
            "---\nid: ADR-0008\ncanon: docs/canon/architecture.md\n---\n# D\n".to_owned(),
        ),
    ];
    let mut files: Vec<(&str, &str)> = cases
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    files.push(("docs/canon/architecture.md", CANON_DOC));
    files.push((
        "docs/features/spec.md",
        "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n# Spec\n\n## Why\n",
    ));
    let report = config.check(&files);
    let canon: Vec<(&str, &str, &str, usize)> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("canon-"))
        .map(|f| (f.path.as_str(), f.code.as_str(), f.subject.as_str(), f.line))
        .collect();
    assert_eq!(
        canon,
        [
            (
                "docs/decisions/ADR-0004.md",
                "canon-form",
                "docs/canon/architecture.md",
                6
            ),
            (
                "docs/decisions/ADR-0005.md",
                "canon-file",
                "docs/canon/missing.md#storage",
                6
            ),
            (
                "docs/decisions/ADR-0006.md",
                "canon-file",
                "docs/features/spec.md#why",
                6
            ),
            (
                "docs/decisions/ADR-0007.md",
                "canon-anchor",
                "docs/canon/architecture.md#nope",
                6
            ),
            (
                "docs/decisions/ADR-0008.md",
                "canon-form",
                "docs/canon/architecture.md",
                3
            ),
        ],
        "{}",
        show(&report)
    );
}
