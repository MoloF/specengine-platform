//! AC-08 and AC-09 of docs/features/spec-check-graph.md: `mention-dangling`,
//! a warning, one per inline mention of a live source (neither
//! `class: generated` nor Tier 3; a failed front-matter counts as live) that
//! resolves to nothing, on its line, the subject as written. Resolution is
//! increment 1's (a defined ID, an `aliases:` entry, `aliases_from` + the
//! written body, `#Y` in the ID's file); `project:` and `slug/` are skipped.
//! An inline mention of a `shape = "name"` prefix that does not resolve drops
//! its last `-segment` and retries while the prefix and one segment remain;
//! never for front-matter (which keeps `ref-dangling`) or number shapes.
//!
//! The lexer joins `#` only before an ID-shaped section, so a missing
//! section is written `MEC-X#MEC-MISSING`, not `MEC-X#MISSING`.

mod common;

use common::check::{Config, fixture_input, show, with_code};
use common::fixture;
use specengine_core::check::{Report, Resolution, Resolver, Verdict};
use specengine_model::Severity;

const TOML: &str = "\
[ids]
R    = { kind = \"requirement\", width = 2 }
Q    = { kind = \"question\",    width = 3, aliases_from = [\"QST\"] }
MEC  = { kind = \"mechanic\",    shape = \"name\" }
TERM = { kind = \"term\",        shape = \"name\" }
AC   = { kind = \"criterion\",   width = 2, scope = \"feature\" }
";

fn config() -> Config {
    Config::from_toml(TOML)
}

/// The definitions every case cites.
const DEFINED: &[(&str, &str)] = &[
    (
        "docs/r/R-01.md",
        "---\nid: R-01\nclass: canon\nowner: o\nreviewed: 2026-09-01\naliases: [R-1]\n---\n\n# R one\n\n## Detail {#R-05}\n\nText.\n",
    ),
    (
        "docs/q/Q-031.md",
        "---\nid: Q-031\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Q\n",
    ),
    (
        "docs/m/stamina.md",
        "---\nid: MEC-STAMINA\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Stamina\n\n## Regen {#MEC-REGEN}\n\nText.\n",
    ),
    (
        "docs/t/exhausted.md",
        "---\nid: TERM-exhausted\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Exhausted\n",
    ),
];

/// A live canon source with `body` after its front-matter (5 lines + blank:
/// the body starts on line 7).
fn source(body: &str) -> String {
    format!("---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n{body}")
}

fn check_with(extra: &[(&str, &str)]) -> Report {
    let mut files: Vec<(&str, &str)> = DEFINED.to_vec();
    files.extend_from_slice(extra);
    config().check(&files)
}

/// `(path, line, subject)` of every `mention-dangling`.
fn dangling(report: &Report) -> Vec<(String, usize, String)> {
    with_code(report, "mention-dangling")
        .into_iter()
        .map(|f| (f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

fn at(path: &str, line: usize, subject: &str) -> (String, usize, String) {
    (path.to_owned(), line, subject.to_owned())
}

// ------------------------------------------------------------------ AC-08

#[test]
fn resolving_mentions_give_nothing() {
    let body = "\
Defined: R-01 and [[R-01]] and MEC-STAMINA and TERM-exhausted.
An alias: R-1. Through aliases_from: QST-031.
Sections: R-01#R-05 and MEC-STAMINA#MEC-REGEN and a section ID alone: R-05.
With a revision: R-01@2.
";
    let text = source(body);
    let report = check_with(&[("docs/s.md", &text)]);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
    assert!(report.findings.is_empty(), "{}", show(&report));
    assert_eq!(report.verdict, Verdict::Clean);
}

#[test]
fn an_undefined_id_or_section_is_a_warning_on_its_line() {
    let body = "\
First line.
Cites R-99 here.

Then MEC-STAMINA#MEC-MISSING, R-01#R-06 and [[R-98]].
A five-digit number is a real mention: R-00001. No re-padding: R-001, QST-31.
";
    let text = source(body);
    let report = check_with(&[("docs/s.md", &text)]);
    assert_eq!(
        dangling(&report),
        [
            at("docs/s.md", 8, "R-99"),
            at("docs/s.md", 10, "MEC-STAMINA#MEC-MISSING"),
            at("docs/s.md", 10, "R-01#R-06"),
            at("docs/s.md", 10, "[[R-98]]"),
            at("docs/s.md", 11, "QST-31"),
            at("docs/s.md", 11, "R-00001"),
            at("docs/s.md", 11, "R-001"),
        ],
        "{}",
        show(&report)
    );
    for finding in with_code(&report, "mention-dangling") {
        assert_eq!(finding.severity, Severity::Warning, "{}", finding.subject);
        assert!(!finding.blocks_when_enforced());
    }
    let r99 = &with_code(&report, "mention-dangling")[0];
    assert_eq!(
        r99.message,
        "`mentions`: `R-99` resolves to no ID and no alias"
    );
    let section = with_code(&report, "mention-dangling")
        .into_iter()
        .find(|f| f.subject == "R-01#R-06")
        .unwrap();
    assert!(section.message.contains("#R-06"), "{}", section.message);
    // Warnings never block, and are counted.
    assert_eq!(report.verdict, Verdict::Clean, "{}", show(&report));
    assert_eq!(report.counts.warnings, 7);
    assert_eq!(report.counts.errors, 0);
    let lines = report.lines(true).join("\n");
    assert!(lines.contains("R-99"), "{lines}");
    assert!(!report.lines(false).join("\n").contains("R-99"));
}

#[test]
fn qualified_mentions_are_skipped() {
    let body = "Elsewhere: other:R-99, feat/AC-99, other:feat/AC-98 and feat/R-97.\n";
    let text = source(body);
    let report = check_with(&[("docs/s.md", &text)]);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
}

#[test]
fn generated_and_tier3_sources_are_not_scanned() {
    let sources: &[(&str, &str)] = &[
        (
            "docs/gen.md",
            "---\nclass: generated\ngenerator: g\nsource: s\n---\n\nCites R-99.\n",
        ),
        (
            "docs/f-shipped.md",
            "---\nclass: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [x]\n---\n\nCites R-99.\n",
        ),
        (
            "docs/f-abandoned.md",
            "---\nclass: spec\nstatus: abandoned\nscope: [x]\n---\n\nCites R-99.\n",
        ),
        (
            "docs/d-rejected.md",
            "---\nclass: decision\nstatus: rejected\nscope: [x]\n---\n\nCites R-99.\n",
        ),
        (
            "docs/d-superseded.md",
            "---\nclass: decision\nstatus: superseded-by R-01\nscope: [x]\n---\n\nCites R-99.\n",
        ),
    ];
    let report = check_with(sources);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
    // Live counterparts are scanned: a draft spec, an accepted decision and
    // one without a status.
    let live: &[(&str, &str)] = &[
        (
            "docs/f-draft.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\nCites R-99.\n",
        ),
        (
            "docs/d-accepted.md",
            "---\nclass: decision\nstatus: accepted\nscope: [x]\ncanon: docs/r/R-01.md#detail\n---\n\nCites R-99.\n",
        ),
        (
            "docs/d-open.md",
            "---\nclass: decision\nscope: [x]\n---\n\nCites R-99.\n",
        ),
        ("docs/plain.md", "No front-matter. Cites R-99.\n"),
    ];
    let report = check_with(live);
    assert_eq!(
        dangling(&report),
        [
            at("docs/d-accepted.md", 8, "R-99"),
            at("docs/d-open.md", 6, "R-99"),
            at("docs/f-draft.md", 7, "R-99"),
            at("docs/plain.md", 1, "R-99"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn a_failed_front_matter_source_is_checked() {
    let text = "---\nclass: generated\ntitle: a: b\n---\n\nCites R-99.\n";
    let report = check_with(&[("docs/broken.md", text)]);
    assert_eq!(
        dangling(&report),
        [at("docs/broken.md", 6, "R-99")],
        "{}",
        show(&report)
    );
}

#[test]
fn front_matter_references_keep_ref_dangling() {
    let text = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nrefs: [R-99]\n---\n\nText.\n";
    let report = check_with(&[("docs/s.md", text)]);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
    let found = with_code(&report, "ref-dangling");
    assert_eq!(found.len(), 1, "{}", show(&report));
    assert_eq!(found[0].severity, Severity::Error);
}

#[test]
fn spec_a_has_no_dangling_mention() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let report = config.run(&input);
    assert!(dangling(&report).is_empty(), "{}", show(&report));
}

#[test]
fn spec_b_has_exactly_the_mixed_script_mention() {
    // fixtures/spec-b/expected.json:51, "FLAG-DRY-RUN mentions inline
    // REQ-003 {mixed}": written with a Cyrillic `E` (U+0415), it resolves to nothing.
    let (config, input) = fixture_input(&fixture("spec-b"));
    let report = config.run(&input);
    assert_eq!(
        dangling(&report),
        [at("docs/spec/cli.md", 25, "R\u{0415}Q-003")],
        "{}",
        show(&report)
    );
    assert_eq!(
        with_code(&report, "mention-dangling")[0].severity,
        Severity::Warning
    );
}

// ------------------------------------------------------------------ AC-09

#[test]
fn a_greedy_name_falls_back_to_its_longest_defined_prefix() {
    let body = "\
A MEC-STAMINA-based rule and a MEC-STAMINA-fast-regen one.
A MEC-NOPE-based rule.
An undefined MEC-X.
A MEC-STAMINA-based#MEC-REGEN section and a MEC-STAMINA-based#MEC-NONE one.
A lowercase real ID: TERM-exhausted-ish.
";
    let text = source(body);
    let report = check_with(&[("docs/s.md", &text)]);
    assert_eq!(
        dangling(&report),
        [
            at("docs/s.md", 8, "MEC-NOPE-based"),
            at("docs/s.md", 9, "MEC-X"),
            at("docs/s.md", 10, "MEC-STAMINA-based#MEC-NONE"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn the_fallback_stops_at_the_prefix_and_one_segment() {
    // `MEC-STAMINA` is defined, `MEC` alone is no ID: `MEC-A-B-C` tries
    // `MEC-A-B`, `MEC-A`, and stops.
    let text = source("A MEC-A-B-C mention.\n");
    let report = check_with(&[("docs/s.md", &text)]);
    assert_eq!(
        dangling(&report),
        [at("docs/s.md", 7, "MEC-A-B-C")],
        "{}",
        show(&report)
    );
    // The first (longest) resolving ID wins.
    let mut files = DEFINED.to_vec();
    let longer =
        "---\nid: MEC-STAMINA-fast\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Fast\n";
    files.push(("docs/m/fast.md", longer));
    let config = config();
    let input = config.input(&files);
    let resolver = Resolver::new(&input, &config.scheme);
    let probe = config.input(&[("docs/probe.md", "MEC-STAMINA-fast-regen\n")]);
    let reference = probe.files[0]
        .parsed
        .as_ref()
        .unwrap()
        .links
        .iter()
        .find_map(|link| match &link.dst {
            specengine_model::LinkTarget::Reference(reference) => Some(reference.clone()),
            _ => None,
        })
        .expect("a mention");
    let fast = resolver
        .paths()
        .iter()
        .position(|path| *path == "docs/m/fast.md")
        .unwrap();
    assert_eq!(
        resolver.resolve_mention(&reference, "MEC-STAMINA-fast-regen"),
        Resolution::Resolved(vec![fast])
    );
    // A declared reference never falls back.
    assert!(matches!(
        resolver.resolve(&reference, "MEC-STAMINA-fast-regen"),
        Resolution::Dangling(_)
    ));
}

#[test]
fn front_matter_never_falls_back() {
    let text = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nrefs: [MEC-STAMINA-based]\n---\n\nText.\n";
    let report = check_with(&[("docs/s.md", text)]);
    let found = with_code(&report, "ref-dangling");
    assert_eq!(found.len(), 1, "{}", show(&report));
    assert_eq!(
        (found[0].line, found[0].subject.as_str()),
        (5, "MEC-STAMINA-based")
    );
    assert!(dangling(&report).is_empty(), "{}", show(&report));
}

#[test]
fn number_shapes_never_fall_back() {
    // `R-01-x` is not lexed as a reference at all (a `-` before a letter is
    // no right boundary); `Q-031` stays whole: no fallback exists for it.
    let config = config();
    let probe = config.input(&[("docs/p.md", "Q-0311 and R-01-x\n")]);
    let references: Vec<String> = probe.files[0]
        .parsed
        .as_ref()
        .unwrap()
        .links
        .iter()
        .filter_map(|link| match &link.dst {
            specengine_model::LinkTarget::Reference(reference) => Some(reference.id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(references, ["Q-0311"]);
    let input = {
        let mut files = DEFINED.to_vec();
        files.push(("docs/p.md", "Q-0311 and R-01-x\n"));
        config.input(&files)
    };
    let report = config.run(&input);
    assert_eq!(
        dangling(&report),
        [at("docs/p.md", 1, "Q-0311")],
        "{}",
        show(&report)
    );
}

#[test]
fn mentions_are_independent_of_input_order() {
    let body = "R-99, MEC-NOPE-based, MEC-STAMINA-based, R-01#R-06.\n";
    let text = source(body);
    let mut files: Vec<(&str, &str)> = DEFINED.to_vec();
    files.push(("docs/s.md", &text));
    files.push(("docs/a.md", &text));
    let config = config();
    let forward = config.check(&files);
    files.reverse();
    let backward = config.check(&files);
    assert_eq!(forward.lines(true), backward.lines(true));
    assert_eq!(forward.to_json(), backward.to_json());
    assert_eq!(dangling(&forward).len(), 6, "{}", show(&forward));
}
