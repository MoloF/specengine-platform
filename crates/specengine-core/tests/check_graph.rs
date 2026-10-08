//! AC-10 and AC-11 of docs/features/spec-check-graph.md: the graph
//! warnings over live sources (neither `class: generated` nor Tier 3).
//!
//! AC-10 `depends-cycle`: nodes are documents, an edge A → B per
//! `links.depends_on` item of a live A that resolves to an ID B holds (no
//! name fallback, `#section` ignored); one finding per strongly connected
//! component of ≥ 2 documents, or of one with a self-edge; on the first
//! member by name, at its first `depends_on` item into the cycle; subject the
//! member names (ID, else path) sorted, joined by `, `.
//!
//! AC-11 `ref-superseded`: a live source referencing Y whose `status:` is
//! `superseded-by X` → one warning per occurrence ("`Y` is superseded by
//! X"), over `refs`, `adrs`, `links.*`, `parent`, `working_answer`, a
//! reference-form `canon:` and inline mentions (with the name fallback).
//! Exempt: `supersedes` items (the key and `links.supersedes`), the
//! `status:` value, and references in X's own files.
//!
//! AC-03 and AC-04 of docs/features/ref-superseded-records.md: a reference
//! of a live source whose readable front-matter is `class: decision`,
//! `status: accepted` with a `YYYY-MM-DD` `date:` D is exempt when X's first
//! holder has such a `date:` strictly later than D (text order); any other
//! source, a missing or unshaped date, or D on or after X's day still warns.

mod common;

use common::check::{Config, fixture_input, show, with_code};
use common::fixture;
use specengine_core::check::{Report, Verdict};
use specengine_model::Severity;

const TOML: &str = "\
[ids]
ADR = { kind = \"decision\", width = 4 }
MEC = { kind = \"mechanic\", shape = \"name\" }
";

fn config() -> Config {
    Config::from_toml(TOML)
}

/// A live canon document `id` depending on `on` (a flow list), with `extra`
/// front-matter lines.
fn node(id: &str, on: &[&str]) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  depends_on: [{}]\n---\n\n# {id}\n",
        on.join(", ")
    )
}

/// `(path, line, subject)` of every finding of `code`.
fn found(report: &Report, code: &str) -> Vec<(String, usize, String)> {
    with_code(report, code)
        .into_iter()
        .map(|f| (f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

fn at(path: &str, line: usize, subject: &str) -> (String, usize, String) {
    (path.to_owned(), line, subject.to_owned())
}

fn cycles(files: &[(&str, &str)]) -> (Vec<(String, usize, String)>, Report) {
    let report = config().check(files);
    (found(&report, "depends-cycle"), report)
}

// ------------------------------------------------------------------ AC-10

#[test]
fn spec_a_has_one_cycle() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let report = config.run(&input);
    assert_eq!(
        found(&report, "depends-cycle"),
        [at(
            "docs/spec/movement/sprint.md",
            9,
            "MEC-SPRINT, MEC-STAMINA"
        )],
        "{}",
        show(&report)
    );
    let cycle = &with_code(&report, "depends-cycle")[0];
    assert_eq!(cycle.severity, Severity::Warning);
    assert!(!cycle.blocks_when_enforced());
    assert_eq!(
        cycle.message,
        "`depends_on` forms a cycle through MEC-SPRINT, MEC-STAMINA"
    );
}

#[test]
fn spec_b_has_no_cycle() {
    let (config, input) = fixture_input(&fixture("spec-b"));
    let report = config.run(&input);
    assert!(
        found(&report, "depends-cycle").is_empty(),
        "{}",
        show(&report)
    );
}

#[test]
fn a_self_loop_is_one_cycle() {
    let a = node("MEC-A", &["MEC-A"]);
    let (found, report) = cycles(&[("docs/a.md", &a)]);
    assert_eq!(found, [at("docs/a.md", 7, "MEC-A")], "{}", show(&report));
    assert_eq!(report.verdict, Verdict::Clean, "warnings never block");
}

#[test]
fn a_three_cycle_is_one_finding_on_the_first_member_by_name() {
    // Paths in the reverse order of the names: the finding follows names.
    let c = node("MEC-C", &["MEC-A"]);
    let b = node("MEC-B", &["MEC-C"]);
    let a = node("MEC-A", &["MEC-B"]);
    let (found, report) = cycles(&[("docs/1.md", &c), ("docs/2.md", &b), ("docs/3.md", &a)]);
    assert_eq!(
        found,
        [at("docs/3.md", 7, "MEC-A, MEC-B, MEC-C")],
        "{}",
        show(&report)
    );
}

#[test]
fn two_cycles_sharing_a_node_are_one_component() {
    let a = node("MEC-A", &["MEC-B"]);
    let b = node("MEC-B", &["MEC-A", "MEC-C"]);
    let c = node("MEC-C", &["MEC-B"]);
    let (found, report) = cycles(&[("docs/a.md", &a), ("docs/b.md", &b), ("docs/c.md", &c)]);
    assert_eq!(
        found,
        [at("docs/a.md", 7, "MEC-A, MEC-B, MEC-C")],
        "{}",
        show(&report)
    );
}

#[test]
fn disjoint_cycles_are_one_finding_each() {
    let a = node("MEC-A", &["MEC-B"]);
    let b = node("MEC-B", &["MEC-A"]);
    let c = node("MEC-C", &["MEC-C"]);
    let d = node("MEC-D", &["MEC-E"]);
    let e = node("MEC-E", &["MEC-F"]);
    let f = node("MEC-F", &["MEC-D"]);
    let (found, report) = cycles(&[
        ("docs/a.md", &a),
        ("docs/b.md", &b),
        ("docs/c.md", &c),
        ("docs/d.md", &d),
        ("docs/e.md", &e),
        ("docs/f.md", &f),
    ]);
    assert_eq!(
        found,
        [
            at("docs/a.md", 7, "MEC-A, MEC-B"),
            at("docs/c.md", 7, "MEC-C"),
            at("docs/d.md", 7, "MEC-D, MEC-E, MEC-F"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn a_dag_has_no_cycle() {
    // A diamond, with a shared sink and a doubled edge.
    let a = node("MEC-A", &["MEC-B", "MEC-C", "MEC-B"]);
    let b = node("MEC-B", &["MEC-D"]);
    let c = node("MEC-C", &["MEC-D"]);
    let d = node("MEC-D", &[]);
    let (found, report) = cycles(&[
        ("docs/a.md", &a),
        ("docs/b.md", &b),
        ("docs/c.md", &c),
        ("docs/d.md", &d),
    ]);
    assert!(found.is_empty(), "{}", show(&report));
}

#[test]
fn the_line_is_the_first_depends_on_item_into_the_cycle() {
    let a = "---\nid: MEC-A\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  depends_on:\n    - MEC-OUT\n    - MEC-B\n    - MEC-B\n---\n\n# A\n";
    let b = node("MEC-B", &["MEC-A"]);
    let out = node("MEC-OUT", &[]);
    let (found, report) = cycles(&[("docs/a.md", a), ("docs/b.md", &b), ("docs/out.md", &out)]);
    assert_eq!(
        found,
        [at("docs/a.md", 9, "MEC-A, MEC-B")],
        "{}",
        show(&report)
    );
}

#[test]
fn a_member_without_an_id_is_named_by_its_path() {
    // The cycle reaches a document through a section ID it defines.
    let a = node("MEC-A", &["MEC-PART"]);
    let x = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  depends_on: [MEC-A]\n---\n\n# X\n\n## Part {#MEC-PART}\n";
    let (found, report) = cycles(&[("docs/a.md", &a), ("docs/x.md", x)]);
    assert_eq!(
        found,
        [at("docs/a.md", 7, "MEC-A, docs/x.md")],
        "{}",
        show(&report)
    );
}

#[test]
fn unresolved_targets_and_the_fallback_give_no_edge() {
    // `MEC-B-based` would fall back to `MEC-B` inline; `depends_on` never does.
    let a = node("MEC-A", &["MEC-B-based", "MEC-NOPE"]);
    let b = node("MEC-B", &["MEC-A"]);
    let (found, report) = cycles(&[("docs/a.md", &a), ("docs/b.md", &b)]);
    assert!(found.is_empty(), "{}", show(&report));
    // The front-matter reference stays an error of increment 1.
    assert_eq!(
        with_code(&report, "ref-dangling").len(),
        2,
        "{}",
        show(&report)
    );
}

#[test]
fn a_section_is_ignored_for_the_edge() {
    let a = node("MEC-A", &["MEC-B#MEC-NOSECTION"]);
    let b = node("MEC-B", &["MEC-A"]);
    let (found, report) = cycles(&[("docs/a.md", &a), ("docs/b.md", &b)]);
    assert_eq!(
        found,
        [at("docs/a.md", 7, "MEC-A, MEC-B")],
        "{}",
        show(&report)
    );
}

#[test]
fn tier3_and_generated_sources_give_no_edge() {
    let a = node("MEC-A", &["MEC-B"]);
    for (case, b) in [
        (
            "a shipped spec",
            "---\nid: MEC-B\nclass: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [x]\nlinks:\n  depends_on: [MEC-A]\n---\n",
        ),
        (
            "an abandoned spec",
            "---\nid: MEC-B\nclass: spec\nstatus: abandoned\nscope: [x]\nlinks:\n  depends_on: [MEC-A]\n---\n",
        ),
        (
            "a rejected decision",
            "---\nid: MEC-B\nclass: decision\nstatus: rejected\nscope: [x]\nlinks:\n  depends_on: [MEC-A]\n---\n",
        ),
        (
            "a generated document",
            "---\nid: MEC-B\nclass: generated\ngenerator: g\nsource: s\nlinks:\n  depends_on: [MEC-A]\n---\n",
        ),
    ] {
        let (found, report) = cycles(&[("docs/a.md", &a), ("docs/b.md", b)]);
        assert!(found.is_empty(), "{case}:\n{}", show(&report));
    }
    // A live counterpart closes the cycle.
    let b = "---\nid: MEC-B\nclass: spec\nstatus: draft\nscope: [x]\nlinks:\n  depends_on: [MEC-A]\n---\n";
    let (found, report) = cycles(&[("docs/a.md", &a), ("docs/b.md", b)]);
    assert_eq!(found.len(), 1, "{}", show(&report));
}

#[test]
fn other_link_types_are_not_depends_on() {
    let a = "---\nid: MEC-A\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  constrains: [MEC-B]\n---\n";
    let b = "---\nid: MEC-B\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  derived_from: [MEC-A]\n---\n";
    let (found, report) = cycles(&[("docs/a.md", a), ("docs/b.md", b)]);
    assert!(found.is_empty(), "{}", show(&report));
}

#[test]
fn cycles_are_independent_of_input_order() {
    let a = node("MEC-A", &["MEC-B"]);
    let b = node("MEC-B", &["MEC-C"]);
    let c = node("MEC-C", &["MEC-A", "MEC-C"]);
    let x = node("MEC-X", &["MEC-X"]);
    let mut files = vec![
        ("docs/z/c.md", c.as_str()),
        ("docs/a.md", a.as_str()),
        ("docs/m/b.md", b.as_str()),
        ("docs/x.md", x.as_str()),
    ];
    let config = config();
    let forward = config.check(&files);
    for _ in 0..files.len() {
        files.rotate_left(1);
        let other = config.check(&files);
        assert_eq!(other.lines(true), forward.lines(true));
        assert_eq!(other.to_json(), forward.to_json());
    }
    files.reverse();
    let backward = config.check(&files);
    assert_eq!(backward.to_json(), forward.to_json());
    assert_eq!(
        found(&forward, "depends-cycle"),
        [
            at("docs/a.md", 7, "MEC-A, MEC-B, MEC-C"),
            at("docs/x.md", 7, "MEC-X"),
        ],
        "{}",
        show(&forward)
    );
}

// ------------------------------------------------------------------ AC-11

/// Y = ADR-0001, superseded by X = ADR-0002; `MEC-OLD` superseded by
/// `MEC-NEW` (a name shape, for the inline fallback).
const SUPERSEDED: &[(&str, &str)] = &[
    (
        "docs/decisions/ADR-0001.md",
        "---\nid: ADR-0001\nclass: decision\ntitle: Old\nstatus: superseded-by ADR-0002\nscope: [x]\n---\n\n# Old\n\nCites ADR-0001 and MEC-OLD.\n",
    ),
    (
        "docs/decisions/ADR-0002.md",
        "---\nid: ADR-0002\nclass: decision\ntitle: New\nstatus: accepted\nscope: [x]\ncanon: docs/canon/c.md#c\nsupersedes: [ADR-0001]\nrefs: [ADR-0001]\n---\n\n# New\n\nReplaces ADR-0001.\n",
    ),
    (
        "docs/canon/c.md",
        "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# C\n",
    ),
    (
        "docs/m/old.md",
        "---\nid: MEC-OLD\nclass: canon\nowner: o\nreviewed: 2026-09-01\nstatus: superseded-by MEC-NEW\n---\n\n# Old mechanic\n",
    ),
    (
        "docs/m/new.md",
        "---\nid: MEC-NEW\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  replaces: [MEC-OLD]\n---\n\n# New mechanic\n\nMEC-OLD is gone.\n",
    ),
];

fn superseded_with(extra: &[(&str, &str)]) -> Report {
    let mut files = SUPERSEDED.to_vec();
    files.extend_from_slice(extra);
    config().check(&files)
}

#[test]
fn the_corpus_alone_is_quiet() {
    let report = superseded_with(&[]);
    assert!(
        found(&report, "ref-superseded").is_empty(),
        "X's own citations and Tier 3 sources are exempt:\n{}",
        show(&report)
    );
}

#[test]
fn every_reference_form_to_a_superseded_document_warns_once() {
    let source = "\
---
class: canon
owner: o
reviewed: 2026-09-01
refs: [ADR-0001]
adrs: [ADR-0001]
parent: ADR-0001
working_answer: ADR-0001
links:
  derived_from: [ADR-0001]
  depends_on: [MEC-OLD]
---

# Source

Inline ADR-0001 and [[ADR-0001]], and a MEC-OLD-based rule.
";
    let decision = "---\nid: ADR-0003\nclass: decision\ntitle: T\nstatus: accepted\nscope: [x]\ncanon: ADR-0001\n---\n";
    let report = superseded_with(&[
        ("docs/s.md", source),
        ("docs/decisions/ADR-0003.md", decision),
    ]);
    assert_eq!(
        found(&report, "ref-superseded"),
        [
            at("docs/decisions/ADR-0003.md", 7, "ADR-0001"),
            at("docs/s.md", 5, "ADR-0001"),
            at("docs/s.md", 6, "ADR-0001"),
            at("docs/s.md", 7, "ADR-0001"),
            at("docs/s.md", 8, "ADR-0001"),
            at("docs/s.md", 10, "ADR-0001"),
            at("docs/s.md", 11, "MEC-OLD"),
            at("docs/s.md", 16, "ADR-0001"),
            at("docs/s.md", 16, "MEC-OLD-based"),
            at("docs/s.md", 16, "[[ADR-0001]]"),
        ],
        "{}",
        show(&report)
    );
    for finding in with_code(&report, "ref-superseded") {
        assert_eq!(finding.severity, Severity::Warning);
        let by = if finding.subject.starts_with("MEC") {
            "MEC-NEW"
        } else {
            "ADR-0002"
        };
        assert_eq!(
            finding.message,
            format!("`{}` is superseded by {by}", finding.subject)
        );
    }
    assert_eq!(report.verdict, Verdict::Clean, "{}", show(&report));
}

#[test]
fn supersedes_items_and_status_values_are_exempt() {
    let source = "\
---
class: spec
status: superseded-by ADR-0001
scope: [x]
supersedes: [ADR-0001]
links:
  supersedes: [ADR-0001, MEC-OLD]
---

# Source
";
    let report = superseded_with(&[("docs/s.md", source)]);
    assert!(
        found(&report, "ref-superseded").is_empty(),
        "{}",
        show(&report)
    );
}

#[test]
fn x_s_own_files_are_exempt_other_files_are_not() {
    // ADR-0002 (X) cites ADR-0001 in `supersedes`, `refs` and its body, and
    // `docs/m/new.md` (MEC-NEW) cites MEC-OLD: all exempt (the corpus alone
    // is quiet); the same `refs` in another file warns.
    let other =
        "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nrefs: [ADR-0001]\n---\n\n# Other\n";
    let report = superseded_with(&[("docs/other.md", other)]);
    assert_eq!(
        found(&report, "ref-superseded"),
        [at("docs/other.md", 5, "ADR-0001")],
        "{}",
        show(&report)
    );
}

#[test]
fn tier3_and_generated_sources_are_exempt() {
    let sources: &[(&str, &str)] = &[
        (
            "docs/gen.md",
            "---\nclass: generated\ngenerator: g\nsource: s\n---\n\nCites ADR-0001.\n",
        ),
        (
            "docs/f-shipped.md",
            "---\nclass: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [x]\nadrs: [ADR-0001]\n---\n\nCites ADR-0001.\n",
        ),
        (
            "docs/decisions/ADR-0004.md",
            "---\nid: ADR-0004\nclass: decision\ntitle: R\nstatus: rejected\nscope: [x]\nrefs: [ADR-0001]\n---\n\nCites ADR-0001.\n",
        ),
    ];
    let report = superseded_with(sources);
    assert!(
        found(&report, "ref-superseded").is_empty(),
        "{}",
        show(&report)
    );
}

#[test]
fn a_failed_front_matter_source_is_live_for_its_body() {
    let broken = "---\nclass: canon\ntitle: a: b\nrefs: [ADR-0001]\n---\n\nCites ADR-0001.\n";
    let report = superseded_with(&[("docs/broken.md", broken)]);
    assert_eq!(
        found(&report, "ref-superseded"),
        [at("docs/broken.md", 7, "ADR-0001")],
        "{}",
        show(&report)
    );
}

#[test]
fn the_fixtures_have_no_superseded_reference() {
    // spec-b: ADR-0002 is superseded by ADR-0001, whose own `adrs:
    // [ADR-0002]` (fixtures/spec-b/docs/records/ADR/ADR-0001.md:8) is exempt.
    for corpus in ["spec-a", "spec-b"] {
        let (config, input) = fixture_input(&fixture(corpus));
        let report = config.run(&input);
        assert!(
            found(&report, "ref-superseded").is_empty(),
            "{corpus}:\n{}",
            show(&report)
        );
    }
}

#[test]
fn superseded_findings_are_independent_of_input_order() {
    let source = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nrefs: [ADR-0001]\n---\n\nADR-0001 and MEC-OLD-x.\n";
    let mut files = SUPERSEDED.to_vec();
    files.push(("docs/s.md", source));
    let config = config();
    let forward = config.check(&files);
    files.reverse();
    let backward = config.check(&files);
    assert_eq!(forward.to_json(), backward.to_json());
    assert_eq!(
        found(&forward, "ref-superseded").len(),
        3,
        "{}",
        show(&forward)
    );
}

// ------------------------------------------------------- dated decisions
// AC-03, AC-04 of docs/features/ref-superseded-records.md. X = ADR-0002 is
// dated `2026-09-10`; MEC-NEW (X of MEC-OLD) stays undated (case (f)). Each
// new source has `refs: [ADR-0001]`, `links.derived_from: [ADR-0001]` and
// the body line `Cites ADR-0001 and a MEC-OLD-based rule.`

/// X's path in `SUPERSEDED`.
const X_PATH: &str = "docs/decisions/ADR-0002.md";

/// `SUPERSEDED` with X's front-matter given `date: {date}` after its
/// `status:`; every other file unchanged.
fn superseded_x_dated(date: &str) -> Vec<(&'static str, String)> {
    SUPERSEDED
        .iter()
        .map(|&(path, text)| {
            if path == X_PATH {
                let dated = text.replacen(
                    "status: accepted\n",
                    &format!("status: accepted\ndate: {date}\n"),
                    1,
                );
                assert_ne!(dated, text, "X's `status: accepted` line moved");
                (path, dated)
            } else {
                (path, text.to_owned())
            }
        })
        .collect()
}

/// `ref-superseded` over `base` plus `extra`, and the report.
fn superseded_over(
    base: Vec<(&'static str, String)>,
    extra: &[(&str, String)],
) -> (Vec<(String, usize, String)>, Report) {
    let mut files: Vec<(&str, &str)> = base.iter().map(|(p, t)| (*p, t.as_str())).collect();
    files.extend(extra.iter().map(|(p, t)| (*p, t.as_str())));
    let report = config().check(&files);
    (found(&report, "ref-superseded"), report)
}

/// The dated corpus (X `2026-09-10`) plus `extra`.
fn dated_with(extra: &[(&str, String)]) -> (Vec<(String, usize, String)>, Report) {
    superseded_over(superseded_x_dated("2026-09-10"), extra)
}

/// A new source: `front` (whole lines), then `refs: [ADR-0001]` on line
/// `n + 2`, `derived_from: [ADR-0001]` on line `n + 4`, and the body line on
/// line `n + 9`, `n` the number of `front` lines.
fn cites(front: &str) -> String {
    format!(
        "---\n{front}refs: [ADR-0001]\nlinks:\n  derived_from: [ADR-0001]\n---\n\n# Source\n\nCites ADR-0001 and a MEC-OLD-based rule.\n"
    )
}

/// An accepted decision `ADR-{n}` with `canon: ADR-0001` on line 8 when
/// `date` is some (line 7 otherwise), `date:` on line 6.
fn accepted(n: &str, date: Option<&str>) -> String {
    let date = date.map_or(String::new(), |date| format!("date: {date}\n"));
    cites(&format!(
        "id: ADR-{n}\nclass: decision\ntitle: T\nstatus: accepted\n{date}scope: [x]\ncanon: ADR-0001\n"
    ))
}

/// The five references of a dated `accepted` decision at `path`
/// (`canon`, `refs`, `derived_from`, inline ×2), lines shifted by `shift`.
fn five(path: &str, shift: usize) -> Vec<(String, usize, String)> {
    vec![
        at(path, 8 - shift, "ADR-0001"),
        at(path, 9 - shift, "ADR-0001"),
        at(path, 11 - shift, "ADR-0001"),
        at(path, 16 - shift, "ADR-0001"),
        at(path, 16 - shift, "MEC-OLD-based"),
    ]
}

const ADR_3: &str = "docs/decisions/ADR-0003.md";
const ADR_4: &str = "docs/decisions/ADR-0004.md";

#[test]
fn an_accepted_decision_dated_before_x_keeps_only_the_undated_successor_s_finding() {
    // AC-03: D 2026-09-01 < X 2026-09-10: its four ADR-0001 references
    // (`canon:`, `refs`, `links.derived_from`, inline) are exempt; MEC-NEW
    // has no date, so `MEC-OLD-based` on the body line warns (case (f)).
    let (got, report) = dated_with(&[(ADR_3, accepted("0003", Some("2026-09-01")))]);
    assert_eq!(got, [at(ADR_3, 16, "MEC-OLD-based")], "{}", show(&report));
    let finding = with_code(&report, "ref-superseded")[0];
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.message, "`MEC-OLD-based` is superseded by MEC-NEW");
    // The source is what the rule reads: no other finding in it.
    let others: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.path == ADR_3 && f.code != "ref-superseded")
        .map(|f| f.code.as_str())
        .collect();
    assert!(others.is_empty(), "{}", show(&report));
}

#[test]
fn a_draft_spec_dated_before_x_still_warns() {
    // (a): not a decision, whatever its date: 2 declared, 2 inline.
    let path = "docs/features/draft.md";
    let spec = cites("class: spec\nstatus: draft\ndate: 2026-09-01\nscope: [x]\n");
    let (got, report) = dated_with(&[(path, spec)]);
    assert_eq!(
        got,
        [
            at(path, 6, "ADR-0001"),
            at(path, 8, "ADR-0001"),
            at(path, 13, "ADR-0001"),
            at(path, 13, "MEC-OLD-based"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn a_canon_dated_before_x_still_warns() {
    // (b): a canon document is edited, not superseded: 4.
    let path = "docs/canon/dated.md";
    let canon = cites("class: canon\nowner: o\nreviewed: 2026-09-01\ndate: 2026-09-01\n");
    let (got, report) = dated_with(&[(path, canon)]);
    assert_eq!(
        got,
        [
            at(path, 6, "ADR-0001"),
            at(path, 8, "ADR-0001"),
            at(path, 13, "ADR-0001"),
            at(path, 13, "MEC-OLD-based"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn a_decision_without_status_dated_before_x_still_warns() {
    // (c): no `status:` (`key-missing`), live, not accepted: 5.
    let decision = cites(
        "id: ADR-0003\nclass: decision\ntitle: T\ndate: 2026-09-01\nscope: [x]\ncanon: ADR-0001\n",
    );
    let (got, report) = dated_with(&[(ADR_3, decision)]);
    assert_eq!(got, five(ADR_3, 1), "{}", show(&report));
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.path == ADR_3 && f.code == "key-missing" && f.subject == "status"),
        "{}",
        show(&report)
    );
}

#[test]
fn an_accepted_decision_with_a_failed_front_matter_warns_for_its_body() {
    // (d): no readable front-matter, so no D and no declared reference: 2.
    let decision = cites(
        "id: ADR-0003\nclass: decision\ntitle: a: b\nstatus: accepted\ndate: 2026-09-01\nscope: [x]\ncanon: ADR-0001\n",
    );
    let (got, report) = dated_with(&[(ADR_3, decision)]);
    assert_eq!(
        got,
        [at(ADR_3, 16, "ADR-0001"), at(ADR_3, 16, "MEC-OLD-based")],
        "{}",
        show(&report)
    );
}

#[test]
fn accepted_decisions_dated_on_or_after_x_s_day_still_warn() {
    // (e): strictly later only; X's own day and a later day: 5 each.
    let (got, report) = dated_with(&[
        (ADR_3, accepted("0003", Some("2026-09-10"))),
        (ADR_4, accepted("0004", Some("2026-09-20"))),
    ]);
    let mut expected = five(ADR_3, 0);
    expected.extend(five(ADR_4, 0));
    assert_eq!(got, expected, "{}", show(&report));
}

#[test]
fn an_undated_x_exempts_no_accepted_decision() {
    // (f), for ADR-0001 as well: `SUPERSEDED` as is, X with no `date:`.
    let undated: Vec<(&'static str, String)> = SUPERSEDED
        .iter()
        .map(|&(path, text)| (path, text.to_owned()))
        .collect();
    let (got, report) = superseded_over(undated, &[(ADR_3, accepted("0003", Some("2026-09-01")))]);
    assert_eq!(got, five(ADR_3, 0), "{}", show(&report));
}

#[test]
fn an_undated_accepted_decision_still_warns() {
    // (g): no D: 5.
    let (got, report) = dated_with(&[(ADR_3, accepted("0003", None))]);
    assert_eq!(got, five(ADR_3, 1), "{}", show(&report));
}

#[test]
fn unshaped_dates_exempt_nothing() {
    // `2026-09-1` sorts before X's `2026-09-10` as text, and X's `2026-9-10`
    // after D's `2026-09-01`: neither is `YYYY-MM-DD`, so both still warn.
    let (got, report) = dated_with(&[(ADR_3, accepted("0003", Some("2026-09-1")))]);
    assert_eq!(got, five(ADR_3, 0), "{}", show(&report));
    let (got, report) = superseded_over(
        superseded_x_dated("2026-9-10"),
        &[(ADR_3, accepted("0003", Some("2026-09-01")))],
    );
    assert_eq!(got, five(ADR_3, 0), "{}", show(&report));
}

#[test]
fn x_s_date_is_its_first_holder_s_in_path_order() {
    // A second holder of ADR-0002 dated before D comes later by path:
    // X's date stays the first holder's `2026-09-10`, so D `2026-09-01` is
    // exempt, whatever the input order.
    let second = "---\nid: ADR-0002\nclass: decision\ntitle: Dup\nstatus: accepted\ndate: 2026-08-01\nscope: [x]\ncanon: docs/canon/c.md#c\n---\n\n# Dup\n";
    let extra = [
        ("docs/z/dup.md", second.to_owned()),
        (ADR_3, accepted("0003", Some("2026-09-01"))),
    ];
    let (forward, report) = dated_with(&extra);
    assert_eq!(
        forward,
        [at(ADR_3, 16, "MEC-OLD-based")],
        "{}",
        show(&report)
    );
    let mut base = superseded_x_dated("2026-09-10");
    base.extend(extra.iter().cloned());
    base.reverse();
    let (backward, reversed) = superseded_over(base, &[]);
    assert_eq!(forward, backward);
    assert_eq!(report.to_json(), reversed.to_json());
}

#[test]
fn the_dated_exemption_is_deterministic() {
    let extra = [
        (ADR_3, accepted("0003", Some("2026-09-01"))),
        (ADR_4, accepted("0004", Some("2026-09-10"))),
    ];
    let first = dated_with(&extra).1.to_json();
    let second = dated_with(&extra).1.to_json();
    assert_eq!(first, second);
}

// ------------------------------------------------------------------ scopes
// AC-07 of docs/features/spec-check-scopes.md (S8): a scoped `depends_on`
// item gives an edge to the feature document; a bare feature-scoped item
// outside its feature gives none; `ref-superseded` resolves by scope.

const SCOPED_TOML: &str = "\
[ids]
MEC = { kind = \"mechanic\", shape = \"name\" }
AC  = { kind = \"criterion\", width = 2, scope = \"feature\" }
";

/// A feature document without an `id:`, depending on `on`, defining `{#AC-01}`.
fn feature(on: &[&str]) -> String {
    format!(
        "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nlinks:\n  depends_on: [{}]\n---\n\n# Feat\n\n## One {{#AC-01}}\n",
        on.join(", ")
    )
}

#[test]
fn a_cycle_through_a_scoped_depends_on_is_one_finding() {
    let config = Config::from_toml(SCOPED_TOML);
    let a = node("MEC-A", &["feat/AC-01"]);
    let files = [
        ("docs/m/a.md", a.as_str()),
        ("docs/features/feat.md", &feature(&["MEC-A"])),
    ];
    let report = config.check(&files);
    assert_eq!(
        found(&report, "depends-cycle"),
        [at("docs/m/a.md", 7, "MEC-A, docs/features/feat.md")],
        "{}",
        show(&report)
    );
    assert!(
        report.findings.iter().all(|f| f.code == "depends-cycle"),
        "{}",
        show(&report)
    );
    // Whatever the input order.
    let mut reversed = files;
    reversed.reverse();
    assert_eq!(config.check(&reversed).findings, report.findings);
}

#[test]
fn a_scoped_depends_on_resolving_elsewhere_gives_no_edge() {
    let config = Config::from_toml(SCOPED_TOML);
    // Bare `AC-01` from a non-feature file, and `feat/AC-01` of a feature
    // lacking it (`other.md` defines it): neither is an edge.
    for target in ["AC-01", "other/AC-01"] {
        let a = node("MEC-A", &[target]);
        let report = config.check(&[
            ("docs/m/a.md", a.as_str()),
            ("docs/features/feat.md", &feature(&["MEC-A"])),
            (
                "docs/features/other.md",
                "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Other\n\n## Two {#AC-02}\n",
            ),
        ]);
        assert!(
            found(&report, "depends-cycle").is_empty(),
            "{target}:\n{}",
            show(&report)
        );
        assert_eq!(
            found(&report, "ref-dangling"),
            [at("docs/m/a.md", 7, target)],
            "{}",
            show(&report)
        );
    }
}

#[test]
fn a_superseded_feature_cited_by_its_slug_warns() {
    let config = Config::from_toml(SCOPED_TOML);
    let report = config.check(&[
        (
            "docs/features/old.md",
            "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nstatus: superseded-by MEC-NEW\n---\n\n# Old\n\n## One {#AC-01}\n",
        ),
        (
            "docs/m/new.md",
            "---\nid: MEC-NEW\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# New\n",
        ),
        (
            "docs/s.md",
            "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nrefs: [old/AC-01]\n---\n\nInline old/AC-01; bare AC-01 is not old's.\n",
        ),
    ]);
    assert_eq!(
        found(&report, "ref-superseded"),
        [
            at("docs/s.md", 5, "old/AC-01"),
            at("docs/s.md", 8, "old/AC-01")
        ],
        "{}",
        show(&report)
    );
    for finding in with_code(&report, "ref-superseded") {
        assert_eq!(finding.message, "`old/AC-01` is superseded by MEC-NEW");
    }
    // The bare `AC-01` of `docs/s.md` dangles instead of warning.
    assert_eq!(
        found(&report, "mention-dangling"),
        [at("docs/s.md", 8, "AC-01")],
        "{}",
        show(&report)
    );
}

/// AC-02 of docs/features/spec-check-links.md: a Markdown file link is a
/// `mentions` link with a path target; the graph rules read ID references
/// only, so a link to a superseded document's file (by path, by anchor, by
/// a definition) warns `ref-superseded` never, and gives no edge.
#[test]
fn file_links_to_a_superseded_document_are_not_ref_superseded() {
    let source = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n\
                  See [old](decisions/ADR-0001.md), [its heading](decisions/ADR-0001.md#old),\n\
                  [the mechanic](m/old.md) and [r].\n\n[r]: m/old.md#old-mechanic\n";
    let report = superseded_with(&[("docs/cites.md", source)]);
    assert!(
        found(&report, "ref-superseded").is_empty(),
        "{}",
        show(&report)
    );
    assert!(
        report.findings.iter().all(|f| !f.code.starts_with("link-")),
        "the links resolve:\n{}",
        show(&report)
    );
    // The same targets cited by ID do warn: the fixture is live.
    let by_id = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\nSee ADR-0001.\n";
    let report = superseded_with(&[("docs/cites.md", by_id)]);
    assert_eq!(
        found(&report, "ref-superseded").len(),
        1,
        "{}",
        show(&report)
    );
}
