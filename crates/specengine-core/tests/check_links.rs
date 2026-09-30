//! AC-05, AC-06 and AC-07 of docs/features/spec-check-links.md: the file
//! link warnings `link-dangling` and `link-anchor`.
//!
//! A checked destination (a percent-decoded path ending exactly in `.md`,
//! or `#anchor` alone) resolves to the first candidate naming a walked
//! document: a `/`-led path from the root; else C1 = the linking file's
//! directory + path, then, only when C1 names nothing and `[paths]
//! link_base` is set, C2 = the base + path. No candidate walked and one in
//! the walk scope → `link-dangling`; else nothing. A resolved link's
//! decoded anchor must be an anchor (slug, attr, html) or a section ID of
//! the target, else `link-anchor`. Sources are live documents only;
//! warnings never block, baseline as usual, independent of input order.
//!
//! Every file is parsed by the real parser; the check reads no file.
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use common::check::{Config, show};
use specengine_core::check::{Baseline, CheckFile, CheckInput, Mode, Report, Verdict};
use specengine_model::Severity;

/// spec-b's layout, with `link_base` and one exclude glob.
const TOML: &str = "\
[ids]
REQ = { kind = \"requirement\", width = 3 }
ASM = { kind = \"assumption\",  width = 2 }
CMD = { kind = \"command\",     shape = \"name\" }

[paths]
roots     = [\"docs\"]
exclude   = [\"docs/private/**\", \"docs/records/REQ/internal/**\"]
link_base = \"docs\"
";

/// The same without `link_base`.
const TOML_NO_BASE: &str = "\
[ids]
REQ = { kind = \"requirement\", width = 3 }
ASM = { kind = \"assumption\",  width = 2 }
CMD = { kind = \"command\",     shape = \"name\" }

[paths]
roots   = [\"docs\"]
exclude = [\"docs/private/**\", \"docs/records/REQ/internal/**\"]
";

const FROM: &str = "docs/records/REQ/REQ-001.md";

/// The linking file with `body` after its front-matter (6 lines + blank:
/// the body starts on line 8). Its own anchors: `requirement-one` (slug),
/// `own-attr` (attr), `own-html` (html) and the section `REQ-005`.
fn linking(body: &str) -> String {
    format!(
        "---\nid: REQ-001\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n{body}\n\n\
         # Requirement one\n\n<a id=\"own-html\"></a>\n\n## Detail {{#own-attr}}\n\nText.\n\n\
         ## Five {{#REQ-005}}\n\nText.\n"
    )
}

/// The walked targets every case may link to.
fn targets() -> Vec<(&'static str, String)> {
    vec![
        (
            "docs/records/REQ/REQ-002.md",
            "---\nid: REQ-002\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Two\n".into(),
        ),
        (
            "docs/records/ASM/ASM-01.md",
            "---\nid: ASM-01\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n\
             # Assumption one\n\n<a id=\"html-h\"></a>\n\n## Heading {#attr-h}\n\nText.\n\n\
             ## Two {#ASM-02}\n\nText.\n"
                .into(),
        ),
        (
            "docs/spec/cli.md",
            "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# CLI\n\n## Sync {#CMD-SYNC}\n\nText.\n".into(),
        ),
    ]
}

/// `(line, code, subject)` of every link finding, and the report.
type Found = Vec<(usize, String, String)>;

fn link_findings(report: &Report) -> Found {
    report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("link-"))
        .map(|f| (f.line, f.code.clone(), f.subject.clone()))
        .collect()
}

fn run(toml: &str, files: &[(&str, &str)]) -> Report {
    Config::from_toml(toml).check(files)
}

/// The linking file with `body` among [`targets`] and `extra`.
fn check_body_with(toml: &str, body: &str, extra: &[(&str, &str)]) -> (Found, Report) {
    let source = linking(body);
    let targets = targets();
    let mut files: Vec<(&str, &str)> = targets.iter().map(|(p, t)| (*p, t.as_str())).collect();
    files.push((FROM, source.as_str()));
    files.extend_from_slice(extra);
    let report = run(toml, &files);
    (link_findings(&report), report)
}

fn check_body(body: &str) -> (Found, Report) {
    check_body_with(TOML, body, &[])
}

fn f(line: usize, code: &str, subject: &str) -> (usize, String, String) {
    (line, code.to_owned(), subject.to_owned())
}

/// The message of the only link finding.
fn message(report: &Report) -> String {
    let found: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("link-"))
        .map(|f| f.message.as_str())
        .collect();
    assert_eq!(found.len(), 1, "{}", show(report));
    found[0].to_owned()
}

fn percent_encode(text: &str) -> String {
    text.bytes().map(|byte| format!("%{byte:02X}")).collect()
}

// ------------------------------------------------------------------ AC-05

/// Every "Data" resolution row that resolves gives no finding.
#[test]
fn resolving_rows_of_the_table_give_nothing() {
    let rows = [
        "[a](REQ-002.md)",
        "[a](../ASM/ASM-01.md#assumption-one)",
        "[a](spec/cli.md#CMD-SYNC)",
        "[a](/docs/spec/cli.md)",
        "[a](/docs/spec/cli.md#CMD-SYNC)",
        "[a](#requirement-one)",
        "[a](../../../README.md)",
        "[a](LICENSE)",
        "[a](../)",
        "[a](x.rs)",
        "[a](x.MD)",
        "[a](https://h/x.md)",
        "[a](//h/x.md)",
        "[a](mailto:a)",
        "[a](../REQ/./REQ-002.md)",
        "[a](../../records/REQ/REQ-002.md)",
        "[a](.//REQ-002.md)",
        "[a](REQ%2D002.md)",
        "[a](REQ-002.md?plain=1)",
        "[a][r]\n\n[r]: ../ASM/ASM-01.md",
    ];
    for row in rows {
        let (found, report) = check_body(row);
        assert!(found.is_empty(), "{row}:\n{}", show(&report));
    }
}

#[test]
fn a_missing_target_is_dangling_after_both_candidates() {
    let (found, report) = check_body("[a](spec/gone.md)");
    assert_eq!(
        found,
        [f(8, "link-dangling", "spec/gone.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`spec/gone.md` names no walked document (tried `docs/records/REQ/spec/gone.md`, `docs/spec/gone.md`)"
    );
    // The detail line of the spec's "Data" (line 14 there; 8 here).
    let line = "warning  docs/records/REQ/REQ-001.md:8: link-dangling: `spec/gone.md` names no walked document (tried `docs/records/REQ/spec/gone.md`, `docs/spec/gone.md`)";
    assert!(
        report.lines(true).iter().any(|l| l == line),
        "{:#?}",
        report.lines(true)
    );
}

#[test]
fn a_percent_encoded_path_is_decoded_before_resolution() {
    let body = "[a](a%20b.md?plain=1)";
    let (found, report) = check_body(body);
    assert_eq!(
        found,
        [f(8, "link-dangling", "a%20b.md?plain=1")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`a%20b.md?plain=1` names no walked document (tried `docs/records/REQ/a b.md`, `docs/a b.md`)"
    );
    // Either candidate walked: resolved.
    for walked in ["docs/records/REQ/a b.md", "docs/a b.md"] {
        let (found, report) = check_body_with(TOML, body, &[(walked, "# A b\n")]);
        assert!(found.is_empty(), "{walked}:\n{}", show(&report));
    }
    // A Cyrillic file name, percent-encoded.
    let name = "\u{0434}\u{043e}\u{043a}";
    let walked = format!("docs/records/REQ/{name}.md");
    let body = format!("[a]({}.md)", percent_encode(name));
    let (found, report) = check_body_with(TOML, &body, &[(walked.as_str(), "# D\n")]);
    assert!(found.is_empty(), "{}", show(&report));
    // A malformed `%` stays as written.
    let (found, report) = check_body_with(
        TOML,
        "[a](100%.md)",
        &[("docs/records/REQ/100%.md", "# P\n")],
    );
    assert!(found.is_empty(), "{}", show(&report));
}

#[test]
fn the_linking_files_directory_wins_over_the_base() {
    // Both candidates walked: C1 resolves, so the anchor is judged there.
    let c1 = (
        "docs/records/REQ/spec/cli.md",
        "# Nested CLI\n\n## Only here {#only-c1}\n",
    );
    let (found, report) = check_body_with(TOML, "[a](spec/cli.md#only-c1)", &[c1]);
    assert!(found.is_empty(), "{}", show(&report));
    let (found, report) = check_body_with(TOML, "[a](spec/cli.md#CMD-SYNC)", &[c1]);
    assert_eq!(
        found,
        [f(8, "link-anchor", "spec/cli.md#CMD-SYNC")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`spec/cli.md#CMD-SYNC`: `docs/records/REQ/spec/cli.md` has no anchor or section `#CMD-SYNC`"
    );
}

#[test]
fn the_base_is_tried_only_after_the_linking_files_directory() {
    // `REQ-002.md` exists beside the linking file only: C1 resolves; the
    // base's `docs/REQ-002.md` would not.
    let (found, report) = check_body("[a](REQ-002.md)");
    assert!(found.is_empty(), "{}", show(&report));
    // A base-only target: resolved through C2, its anchor judged there.
    let (found, report) = check_body("[a](spec/cli.md#nope)");
    assert_eq!(
        found,
        [f(8, "link-anchor", "spec/cli.md#nope")],
        "{}",
        show(&report)
    );
    assert!(message(&report).contains("`docs/spec/cli.md` has no anchor"));
}

#[test]
fn without_a_base_a_root_relative_link_from_a_nested_file_dangles() {
    let (found, report) = check_body_with(TOML_NO_BASE, "[a](spec/cli.md)", &[]);
    assert_eq!(
        found,
        [f(8, "link-dangling", "spec/cli.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`spec/cli.md` names no walked document (tried `docs/records/REQ/spec/cli.md`)"
    );
    // The same link from a file in the docs root resolves file-relative.
    let top = "[a](spec/cli.md)\n";
    let report = run(
        TOML_NO_BASE,
        &[("docs/top.md", top), ("docs/spec/cli.md", "# C\n")],
    );
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn a_rooted_path_is_one_candidate_from_the_root() {
    let (found, report) = check_body("[a](/docs/gone.md)");
    assert_eq!(
        found,
        [f(8, "link-dangling", "/docs/gone.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`/docs/gone.md` names no walked document (tried `docs/gone.md`)",
        "no base candidate for a `/`-led path"
    );
    // `/spec/cli.md` is not the base's `docs/spec/cli.md`.
    let (found, report) = check_body("[a](/spec/cli.md)");
    assert!(found.is_empty(), "outside every root: {}", show(&report));
    // Popping above the root: nothing.
    let (found, report) = check_body("[a](/../docs/spec/cli.md)");
    assert!(found.is_empty(), "{}", show(&report));
}

#[test]
fn candidates_leaving_the_root_are_not_listed_and_equal_ones_once() {
    // C1 `docs/gone.md` in scope, C2 leaves the root: listed without it.
    let (found, report) = check_body("[a](../../gone.md)");
    assert_eq!(
        found,
        [f(8, "link-dangling", "../../gone.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`../../gone.md` names no walked document (tried `docs/gone.md`)"
    );
    // From the base itself, C2 equals C1: tried and listed once.
    let report = run(TOML, &[("docs/top.md", "[a](gone.md)\n")]);
    assert_eq!(
        link_findings(&report),
        [f(1, "link-dangling", "gone.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`gone.md` names no walked document (tried `docs/gone.md`)"
    );
}

#[test]
fn anchors_match_a_slug_an_attr_an_html_anchor_or_a_section_id() {
    for anchor in ["assumption-one", "heading", "attr-h", "html-h", "ASM-02"] {
        let body = format!("[a](../ASM/ASM-01.md#{anchor})");
        let (found, report) = check_body(&body);
        assert!(found.is_empty(), "{body}:\n{}", show(&report));
    }
    // Alone: the linking file itself.
    for anchor in [
        "requirement-one",
        "detail",
        "own-attr",
        "own-html",
        "REQ-005",
    ] {
        let body = format!("[a](#{anchor})");
        let (found, report) = check_body(&body);
        assert!(found.is_empty(), "{body}:\n{}", show(&report));
    }
    // A section ID through the base (the spec-b case).
    let (found, report) = check_body("[a](spec/cli.md#CMD-SYNC)");
    assert!(found.is_empty(), "{}", show(&report));
}

#[test]
fn an_unknown_anchor_is_link_anchor_exact_and_case_sensitive() {
    let cases = [
        (
            "[a](../ASM/ASM-01.md#nope)",
            "../ASM/ASM-01.md#nope",
            "docs/records/ASM/ASM-01.md",
            "nope",
        ),
        (
            "[a](../ASM/ASM-01.md#Heading)",
            "../ASM/ASM-01.md#Heading",
            "docs/records/ASM/ASM-01.md",
            "Heading",
        ),
        (
            "[a](../ASM/ASM-01.md#asm-02)",
            "../ASM/ASM-01.md#asm-02",
            "docs/records/ASM/ASM-01.md",
            "asm-02",
        ),
        ("[a](#nope)", "#nope", FROM, "nope"),
    ];
    for (body, subject, target, anchor) in cases {
        let (found, report) = check_body(body);
        assert_eq!(
            found,
            [f(8, "link-anchor", subject)],
            "{body}:\n{}",
            show(&report)
        );
        assert_eq!(
            message(&report),
            format!("`{subject}`: `{target}` has no anchor or section `#{anchor}`"),
            "{body}"
        );
    }
}

#[test]
fn a_percent_encoded_cyrillic_anchor_matches_its_slug() {
    // "Sinkhronizatsiya" heading, slug in lower case.
    let heading = "\u{0421}\u{0438}\u{043d}\u{0445}\u{0440}\u{043e}\u{043d}\u{0438}\u{0437}\u{0430}\u{0446}\u{0438}\u{044f}";
    let slug = heading.to_lowercase();
    let target = format!("# CLI\n\n## {heading}\n\nText.\n");
    let extra = [("docs/records/REQ/ru.md", target.as_str())];
    let body = format!("[a](ru.md#{})", percent_encode(&slug));
    let (found, report) = check_body_with(TOML, &body, &extra);
    assert!(found.is_empty(), "{body}:\n{}", show(&report));
    // As written (not encoded) too.
    let body = format!("[a](ru.md#{slug})");
    let (found, report) = check_body_with(TOML, &body, &extra);
    assert!(found.is_empty(), "{body}:\n{}", show(&report));
    // Alone, in the linking file.
    let own = format!("[a](#{})\n\n## {heading}\n", percent_encode(&slug));
    let report = run(TOML, &[("docs/own.md", own.as_str())]);
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
    // A wrong encoded anchor is reported.
    let body = format!("[a](ru.md#{})", percent_encode("x"));
    let (found, report) = check_body_with(TOML, &body, &extra);
    assert_eq!(found.len(), 1, "{}", show(&report));
    assert_eq!(found[0].1, "link-anchor");
}

#[test]
fn a_target_without_a_readable_parse_resolves_but_its_anchor_is_not_checked() {
    let config = Config::from_toml(TOML);
    let source = linking("[a](bin.md#nope) [b](gone-read.md#nope)");
    let mut input = config.input(&[(FROM, source.as_str())]);
    input.files.push(CheckFile::parse(
        "docs/records/REQ/bin.md",
        b"# Bin\n\xff\xfe\n".to_vec(),
        &config.scheme,
    ));
    input.files.push(CheckFile::unreadable(
        "docs/records/REQ/gone-read.md",
        "permission denied",
    ));
    let report = config.run(&input);
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn any_class_or_tier_resolves_a_link() {
    let extra: &[(&str, &str)] = &[
        (
            "docs/records/REQ/gen.md",
            "---\nclass: generated\ngenerator: g\nsource: s\n---\n\n# Gen\n",
        ),
        (
            "docs/records/REQ/old.md",
            "---\nclass: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [x]\n---\n\n# Old\n",
        ),
        (
            "docs/records/REQ/broken.md",
            "---\nclass: canon\ntitle: a: b\n---\n\n# Broken\n",
        ),
    ];
    let (found, report) = check_body_with(
        TOML,
        "[a](gen.md#gen) [b](old.md#old) [c](broken.md#broken)",
        extra,
    );
    assert!(found.is_empty(), "{}", show(&report));
}

// ------------------------------------------------------------------ AC-06

#[test]
fn unchecked_and_out_of_scope_targets_give_nothing() {
    let rows = [
        "[a](LICENSE)",
        "[a](../../decisions/)",
        "[a](../../decisions)",
        "[a](x.rs)",
        "[a](x.MD)",
        "[a](x.md.bak)",
        // Not ending exactly in `.md`: never checked, even when it would
        // normalise to a walked document or a missing one.
        "[a](REQ-002.md/#nope)",
        "[a](gone.md/)",
        "[a](gone.md.)",
        // Outside every root: C1 `src/x.md`, C2 leaves the root.
        "[a](../../../src/x.md)",
        // Above the root.
        "[a](../../../../x.md)",
    ];
    for row in rows {
        let (found, report) = check_body(row);
        assert!(found.is_empty(), "{row}:\n{}", show(&report));
    }
    // Excluded and below a `.`-named directory, from the base itself (C1 =
    // C2); outside every root without a base.
    let files: &[(&str, &str)] = &[(
        "docs/top.md",
        "[a](private/x.md) [b](.hidden/x.md) [c](sub/.git/x.md) [d](../README.md)\n",
    )];
    let report = run(TOML, files);
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
    let report = run(TOML_NO_BASE, files);
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn the_same_rows_in_scope_are_flagged() {
    // The counterparts of the out-of-scope rows, inside the walk scope: the
    // scope rule, not a blanket silence, keeps the rows above quiet.
    let report = run(
        TOML,
        &[(
            "docs/top.md",
            "[a](public/x.md) [b](hidden/x.md) [c](sub/git/x.md)\n",
        )],
    );
    assert_eq!(
        link_findings(&report),
        [
            f(1, "link-dangling", "hidden/x.md"),
            f(1, "link-dangling", "public/x.md"),
            f(1, "link-dangling", "sub/git/x.md"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn an_excluded_c1_with_a_missing_in_scope_c2_is_the_accepted_false_warning() {
    // Rule 8: C1 is excluded (never walked), C2 `docs/internal/x.md` is in
    // scope and missing.
    let (found, report) = check_body("[a](internal/x.md)");
    assert_eq!(
        found,
        [f(8, "link-dangling", "internal/x.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`internal/x.md` names no walked document (tried `docs/records/REQ/internal/x.md`, `docs/internal/x.md`)"
    );
    // Both candidates excluded: nothing.
    let (found, report) = check_body("[a](../../private/x.md)");
    assert!(found.is_empty(), "{}", show(&report));
}

#[test]
fn generated_and_tier3_sources_are_not_checked_a_failed_front_matter_is() {
    let quiet: &[(&str, &str)] = &[
        (
            "docs/gen.md",
            "---\nclass: generated\ngenerator: g\nsource: s\n---\n\n[a](gone.md) [b](#nope)\n",
        ),
        (
            "docs/f-shipped.md",
            "---\nclass: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [x]\n---\n\n[a](gone.md)\n",
        ),
        (
            "docs/f-abandoned.md",
            "---\nclass: spec\nstatus: abandoned\nscope: [x]\n---\n\n[a](gone.md)\n",
        ),
        (
            "docs/d-rejected.md",
            "---\nclass: decision\nstatus: rejected\nscope: [x]\n---\n\n[a](gone.md)\n",
        ),
    ];
    let report = run(TOML, quiet);
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
    let live: &[(&str, &str)] = &[
        (
            "docs/broken.md",
            "---\nclass: generated\ntitle: a: b\n---\n\n[a](gone.md)\n",
        ),
        (
            "docs/f-draft.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n[a](gone.md)\n",
        ),
        ("docs/plain.md", "[a](gone.md)\n"),
    ];
    let report = run(TOML, live);
    let found: Vec<(String, usize, String)> = report
        .findings
        .iter()
        .filter(|f| f.code == "link-dangling")
        .map(|f| (f.path.clone(), f.line, f.subject.clone()))
        .collect();
    assert_eq!(
        found,
        [
            ("docs/broken.md".to_owned(), 6, "gone.md".to_owned()),
            ("docs/f-draft.md".to_owned(), 7, "gone.md".to_owned()),
            ("docs/plain.md".to_owned(), 1, "gone.md".to_owned()),
        ],
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------------ AC-07

#[test]
fn findings_are_warnings_with_the_subject_as_written_on_the_destinations_line() {
    let body = "\
[a](<x y.md>)
[b](x\\_y.md)
[c
d](gone.md#h)
[e][r]

[r]:
  ref-gone.md";
    let (found, report) = check_body(body);
    assert_eq!(
        found,
        [
            f(8, "link-dangling", "x y.md"),
            f(9, "link-dangling", "x\\_y.md"),
            f(11, "link-dangling", "gone.md#h"),
            f(15, "link-dangling", "ref-gone.md"),
        ],
        "{}",
        show(&report)
    );
    for finding in report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("link-"))
    {
        assert_eq!(finding.severity, Severity::Warning, "{finding:?}");
        assert_eq!(finding.path, FROM);
        assert!(!finding.blocks_when_enforced());
    }
    assert_eq!(report.counts.errors, 0, "{}", show(&report));
    assert_eq!(report.counts.warnings, 4);
    assert_eq!(report.verdict_in(Mode::Enforce), Verdict::Clean);
    assert_eq!(report.verdict, Verdict::Clean, "warnings never block");
}

#[test]
fn a_baseline_entry_turns_a_link_finding_into_debt() {
    let body = "[a](spec/gone.md) [b](#nope)";
    let config = Config::from_toml(TOML);
    let source = linking(body);
    let targets = targets();
    let mut files: Vec<(&str, &str)> = targets.iter().map(|(p, t)| (*p, t.as_str())).collect();
    files.push((FROM, source.as_str()));
    let input = config.input(&files);
    let baseline = Baseline::from_toml(
        "[[debt]]\ncode = \"link-dangling\"\npath = \"docs/records/REQ/REQ-001.md\"\nsubject = \"spec/gone.md\"\nreason = \"moved\"\nexpires = \"2026-12-31\"\n\n\
         [[debt]]\ncode = \"link-anchor\"\npath = \"docs/records/REQ/REQ-001.md\"\nsubject = \"#nope\"\nreason = \"slug\"\nexpires = \"2026-12-31\"\n",
    )
    .unwrap_or_else(|e| panic!("{}", e.at(".spec-debt.toml")));
    let report = config.run_with(&input, &baseline, common::check::TODAY);
    let mut debts: Vec<(&str, bool)> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("link-"))
        .map(|f| (f.code.as_str(), f.is_live_debt()))
        .collect();
    debts.sort();
    assert_eq!(
        debts,
        [("link-anchor", true), ("link-dangling", true)],
        "{}",
        show(&report)
    );
    assert_eq!(
        (
            report.counts.debt,
            report.counts.warnings,
            report.counts.stale
        ),
        (2, 0, 0)
    );
    assert!(report.lines(true).iter().any(|line| line.starts_with(
        "debt  docs/records/REQ/REQ-001.md:8: link-dangling: `spec/gone.md` names no walked document"
    )));
}

#[test]
fn link_findings_are_independent_of_input_order() {
    let a = "[a](b.md#nope) [x](gone.md)\n";
    let b = "[a](a.md#top) [y](../gone.md#h)\n\n# B\n";
    let c = "# Top\n\n[a](sub/a.md) [z](#missing)\n";
    let mut files = vec![("docs/sub/a.md", a), ("docs/sub/b.md", b), ("docs/c.md", c)];
    let config = Config::from_toml(TOML);
    let forward = config.check(&files);
    assert_eq!(link_findings(&forward).len(), 5, "{}", show(&forward));
    for _ in 0..files.len() {
        files.rotate_left(1);
        let other = config.check(&files);
        assert_eq!(other.lines(true), forward.lines(true));
        assert_eq!(other.to_json(), forward.to_json());
    }
    files.reverse();
    let backward = config.check(&files);
    assert_eq!(backward.lines(true), forward.lines(true));
    assert_eq!(backward.to_json(), forward.to_json());
}

/// The spec's second "Data" finding, on corpus-mini's notes with a wrong
/// anchor (the fixture itself resolves: `eval` `links_census.rs`).
#[test]
fn the_data_link_anchor_line() {
    let toml = "[ids]\nZR = { kind = \"rule\", width = 3 }\n\n[paths]\nroots = [\"design\"]\n";
    let root = common::fixture("corpus-mini");
    let notes = std::fs::read_to_string(root.join("design/notes.md"))
        .expect("notes")
        .replace(
            "sections.md#a-rule-written-as-a-section",
            "sections.md#nope",
        );
    let sections = std::fs::read_to_string(root.join("design/sections.md")).expect("sections");
    let rules = std::fs::read_to_string(root.join("design/rules.md")).expect("rules");
    let report = run(
        toml,
        &[
            ("design/notes.md", notes.as_str()),
            ("design/rules.md", rules.as_str()),
            ("design/sections.md", sections.as_str()),
        ],
    );
    let lines = report.lines(true);
    assert!(
        lines.iter().any(|line| line
            == "warning  design/notes.md:8: link-anchor: `sections.md#nope`: `design/sections.md` has no anchor or section `#nope`"),
        "{lines:#?}"
    );
    assert!(
        lines.iter().any(|line| line
            == "warning  design/sections.md:10: link-dangling: `missing.md` names no walked document (tried `design/missing.md`)"),
        "{lines:#?}"
    );
}

#[test]
fn check_input_with_only_problems_has_no_link_findings() {
    let config = Config::from_toml(TOML);
    let report = config.run(&CheckInput::default());
    assert!(link_findings(&report).is_empty(), "{}", show(&report));
}

/// The `link-anchor` message names the anchor percent-decoded (the one
/// compared), the subject keeps the destination as written.
#[test]
fn the_link_anchor_message_shows_the_decoded_anchor() {
    let (found, report) = check_body("[a](../ASM/ASM-01.md#no%20pe)");
    assert_eq!(
        found,
        [f(8, "link-anchor", "../ASM/ASM-01.md#no%20pe")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`../ASM/ASM-01.md#no%20pe`: `docs/records/ASM/ASM-01.md` has no anchor or section `#no pe`"
    );
    let word = "\u{043d}\u{0435}\u{0442}";
    let body = format!("[a](#{})", percent_encode(word));
    let (found, report) = check_body(&body);
    let written = format!("#{}", percent_encode(word));
    assert_eq!(found, [f(8, "link-anchor", &written)], "{}", show(&report));
    assert_eq!(
        message(&report),
        format!("`{written}`: `{FROM}` has no anchor or section `#{word}`")
    );
    // A malformed `%` stays in the message as written.
    let (_, report) = check_body("[a](#100%)");
    assert_eq!(
        message(&report),
        format!("`#100%`: `{FROM}` has no anchor or section `#100%`")
    );
}

/// The check's scope is the walk's compiled scope (`WalkScope`): a target
/// the scope excludes gives nothing, the same target outside the exclude
/// globs dangles.
#[test]
fn the_check_scope_is_the_compiled_walk_scope() {
    let config = Config::from_toml(TOML);
    let scope = config.paths.walk_scope();
    for (candidate, in_scope) in [
        ("docs/private/x.md", false),
        ("docs/records/REQ/internal/x.md", false),
        ("docs/public/x.md", true),
        ("docs/.hidden/x.md", false),
        ("README.md", false),
    ] {
        assert_eq!(scope.in_walk_scope(candidate), in_scope, "{candidate}");
    }
    let report = run(
        TOML,
        &[("docs/top.md", "[a](private/x.md) [b](public/x.md)\n")],
    );
    assert_eq!(
        link_findings(&report),
        [f(1, "link-dangling", "public/x.md")],
        "{}",
        show(&report)
    );
}

/// The parser's known limit (`file_links.rs`,
/// `a_gt_on_an_indented_continuation_line_is_not_located`) at the check: the
/// path `>x.md` is resolved, the subject shows the span's `x.md`.
#[test]
fn the_indented_gt_limit_shows_the_span_as_subject() {
    let report = run(
        TOML,
        &[
            ("docs/top.md", "[a](\n    >x.md)\n"),
            ("docs/x.md", "# X\n"),
        ],
    );
    assert_eq!(
        link_findings(&report),
        [f(2, "link-dangling", "x.md")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report),
        "`x.md` names no walked document (tried `docs/>x.md`)"
    );
}
