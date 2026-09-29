//! AC-15, AC-16, AC-17 of docs/features/spec-check.md: the report. Lines:
//! one per finding blocking in the mode, one per cannot-check cause, then
//! the summary; `detail` adds the rest. Every parser code reaches the
//! report through the one table (`homoglyph`, `duplicate-id` raised to
//! error), and parser errors block. Output is sorted by (path, line, code,
//! subject, message) whatever the input order: byte-identical lines and
//! JSON.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use common::check::{Config, fixture_input, show};
use common::fixture;
use specengine_core::check::{
    self, Baseline, CheckFile, CheckInput, Mode, PARSER_SEVERITY, Problem, ProblemKind, Report,
    Verdict,
};
use specengine_model::{DiagnosticCode, Severity};

const TOML: &str = "\
[ids]
R   = { kind = \"requirement\", width = 2 }
ADR = { kind = \"decision\", width = 4 }
";

fn config_in(mode: &str) -> Config {
    Config::from_toml(&format!("{TOML}\n[check]\nmode = \"{mode}\"\n"))
}

/// Two warnings (`unknown-key`), one error in debt (`ref-dangling`).
const QUIET: &[(&str, &str)] = &[
    (
        "docs/a.md",
        "---\nclass: generated\nid: R-01\nfoo: 1\nbar: 2\n---\n# A\n",
    ),
    (
        "docs/b.md",
        "---\nclass: generated\nid: R-02\nrefs: [R-99]\n---\n# B\n",
    ),
];
const QUIET_DEBT: &str = "[[debt]]\ncode = \"ref-dangling\"\npath = \"docs/b.md\"\nsubject = \"R-99\"\nreason = \"later\"\nexpires = \"2026-12-31\"\n\n[[debt]]\ncode = \"budget\"\npath = \"docs/z.md\"\nreason = \"gone\"\nexpires = \"2026-12-31\"\n";

// ------------------------------------------------------------------ AC-15

#[test]
fn only_debt_and_warnings_give_the_summary_line_alone() {
    let config = config_in("enforce");
    let baseline = Baseline::from_toml(QUIET_DEBT).unwrap();
    let report = config.run_with(&config.input(QUIET), &baseline, "2026-09-29");
    assert_eq!(
        (
            report.counts.warnings,
            report.counts.debt,
            report.counts.stale
        ),
        (2, 1, 1),
        "{}",
        show(&report)
    );
    let lines = report.lines(false);
    assert_eq!(
        lines,
        [
            "spec check [enforce]: 2 documents, 0 errors, 2 warnings, 1 debt, 0 expired, 1 stale \u{2014} clean"
        ]
    );
    // `detail` adds the warnings, the debt and the stale entry.
    let detail = report.lines(true);
    assert_eq!(detail.len(), 5, "{detail:#?}");
    let labels: Vec<&str> = detail[..4]
        .iter()
        .map(|line| line.split("  ").next().unwrap())
        .collect();
    assert_eq!(
        labels,
        ["warning", "warning", "debt", "stale"],
        "{detail:#?}"
    );
    assert!(
        detail[2].contains("(debt until 2026-12-31: later)"),
        "{}",
        detail[2]
    );
    assert_eq!(detail[4], lines[0]);
}

#[test]
fn k_blocking_findings_give_k_plus_one_lines_and_observe_gives_one() {
    let files: &[(&str, &str)] = &[
        (
            "docs/a.md",
            "---\nclass: generated\nid: R-01\nfoo: 1\n---\n# A\n",
        ),
        (
            "docs/b.md",
            "---\nclass: generated\nid: R-02\nrefs: [R-97, R-98]\n---\n# B\n",
        ),
        ("docs/c.md", "# C\n"),
    ];
    let enforce = config_in("enforce").check(files);
    let blocking: Vec<&check::Finding> = enforce
        .findings
        .iter()
        .filter(|f| enforce.blocks(f))
        .collect();
    assert_eq!(blocking.len(), 3, "{}", show(&enforce));
    let lines = enforce.lines(false);
    assert_eq!(lines.len(), 4, "{lines:#?}");
    assert_eq!(
        &lines[..3],
        [
            "error  docs/b.md:4: ref-dangling: `refs`: `R-97` resolves to no ID and no alias",
            "error  docs/b.md:4: ref-dangling: `refs`: `R-98` resolves to no ID and no alias",
            "error  docs/c.md:1: class-missing: no `class:`: declare one of canon | decision | spec | generated",
        ]
    );
    assert!(lines[3].ends_with("\u{2014} blocked"), "{}", lines[3]);
    assert_eq!(enforce.exit_code(), 1);
    assert_eq!(enforce.lines(true).len(), 5, "detail adds the warning");

    let observe = config_in("observe").check(files);
    assert_eq!(observe.verdict, Verdict::Observed);
    assert_eq!(observe.exit_code(), 0);
    let lines = observe.lines(false);
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert!(
        lines[0].starts_with("spec check [observe]: 3 documents, 3 errors, 1 warnings")
            && lines[0].ends_with("\u{2014} observed"),
        "{}",
        lines[0]
    );
    let detail = observe.lines(true);
    assert_eq!(detail.len(), 5);
    assert_eq!(
        detail.iter().filter(|l| l.starts_with("error  ")).count(),
        3,
        "{detail:#?}"
    );
}

#[test]
fn cannot_check_causes_are_listed_and_win() {
    let config = config_in("observe");
    let mut input = config.input(QUIET);
    input
        .files
        .push(CheckFile::unreadable("docs/locked.md", "permission denied"));
    input.problems.push(Problem {
        kind: ProblemKind::UnreadableDir,
        path: "docs/private".to_owned(),
    });
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::CannotCheck);
    assert_eq!(report.exit_code(), 2);
    assert_eq!(report.counts.documents, 3, "an unreadable file is walked");
    let lines = report.lines(false);
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(
        lines[0].starts_with("cannot  docs/locked.md: "),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("cannot  docs/private: "),
        "{}",
        lines[1]
    );
    assert!(lines[2].ends_with("\u{2014} cannot-check"), "{}", lines[2]);
    let cannot = Report::cannot(
        Mode::Enforce,
        vec![check::Cause {
            path: String::new(),
            message: "no config".to_owned(),
        }],
    );
    assert_eq!(cannot.lines(false)[0], "cannot  .: no config");
    assert_eq!(cannot.exit_code(), 2);
}

#[test]
fn a_skipped_name_is_a_warning_with_an_empty_path() {
    let config = config_in("enforce");
    let mut input = config.input(&[("docs/a.md", "---\nclass: generated\nid: R-01\n---\n")]);
    input.problems.push(Problem {
        kind: ProblemKind::SkippedName,
        path: String::new(),
    });
    let report = config.run(&input);
    let skipped: Vec<(&str, Severity)> = report
        .findings
        .iter()
        .map(|f| (f.code.as_str(), f.severity))
        .collect();
    assert_eq!(skipped, [("name-skipped", Severity::Warning)]);
    assert_eq!(report.findings[0].path, "");
    assert_eq!(report.verdict, Verdict::Clean);
    assert!(report.lines(true)[0].starts_with("warning  .:1: name-skipped: "));
}

#[test]
fn the_json_has_the_documented_shape() {
    let config = config_in("enforce");
    let baseline = Baseline::from_toml(QUIET_DEBT).unwrap();
    let report = config.run_with(&config.input(QUIET), &baseline, "2026-09-29");
    let json = report.to_json();
    assert!(!json.contains('\n'), "one line");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut want = vec![
        "cannot_check",
        "counts",
        "findings",
        "mode",
        "stale",
        "verdict",
    ];
    want.sort();
    let mut got = keys.clone();
    got.sort();
    assert_eq!(got, want);
    assert!(
        json.starts_with("{\"mode\":\"enforce\",\"verdict\":\"clean\",\"counts\":{"),
        "{json}"
    );
    let counts: Vec<&str> = value["counts"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut counts = counts;
    counts.sort();
    assert_eq!(
        counts,
        [
            "debt",
            "documents",
            "errors",
            "expired",
            "stale",
            "warnings"
        ]
    );
    // In the documented order; `without_class` is gone (class-missing counts it).
    assert!(
        json.contains(
            "\"counts\":{\"documents\":2,\"errors\":0,\"warnings\":2,\"debt\":1,\"expired\":0,\"stale\":1}"
        ),
        "{json}"
    );
    let debt = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "ref-dangling")
        .unwrap();
    assert_eq!(
        debt["debt"],
        serde_json::json!({"reason": "later", "expires": "2026-12-31", "expired": false})
    );
    assert_eq!(debt["severity"], "error");
    assert!(debt.get("fix").is_none(), "no fix: the key is absent");
    assert_eq!(value["stale"][0]["path"], "docs/z.md");
    for (verdict, text) in [
        (Verdict::Clean, "\"clean\""),
        (Verdict::Observed, "\"observed\""),
        (Verdict::Blocked, "\"blocked\""),
        (Verdict::CannotCheck, "\"cannot-check\""),
    ] {
        assert_eq!(serde_json::to_string(&verdict).unwrap(), text);
    }
    assert_eq!(
        [
            Verdict::Clean,
            Verdict::Observed,
            Verdict::Blocked,
            Verdict::CannotCheck
        ]
        .map(Verdict::exit_code),
        [0, 0, 1, 2]
    );
}

// ------------------------------------------------------------------ AC-16

/// One input per parser code.
fn one_per_parser_code() -> Vec<(DiagnosticCode, &'static str, Vec<u8>)> {
    use DiagnosticCode as C;
    let mut not_utf8 = b"# Title \xff\n".to_vec();
    not_utf8.extend_from_slice(b"more\n");
    vec![
        (C::NotUtf8, "docs/01.md", not_utf8),
        (
            C::FrontmatterUnclosed,
            "docs/02.md",
            b"---\nid: R-02\n# T\n".to_vec(),
        ),
        (
            C::FrontmatterYaml,
            "docs/03.md",
            b"---\ntitle: a: b\n---\n".to_vec(),
        ),
        (
            C::FrontmatterNotMapping,
            "docs/04.md",
            b"---\n- a\n---\n".to_vec(),
        ),
        (
            C::FrontmatterType,
            "docs/05.md",
            b"---\nid: R-05\ntier: two\n---\n".to_vec(),
        ),
        (
            C::IdNotInScheme,
            "docs/06.md",
            b"---\nid: ZZZ-1\n---\n".to_vec(),
        ),
        (
            C::UnknownKey,
            "docs/07.md",
            b"---\nid: R-07\nfoo: bar\n---\n".to_vec(),
        ),
        (
            C::UnknownLinkType,
            "docs/08.md",
            b"---\nid: R-08\nlinks:\n  frobs: [R-07]\n---\n".to_vec(),
        ),
        (
            C::UnparsedReference,
            "docs/09.md",
            b"---\nid: R-09\nparent: R-07 and R-08\n---\n".to_vec(),
        ),
        (
            C::Homoglyph,
            "docs/10.md",
            "---\nid: \u{0410}DR-0010\n---\n".as_bytes().to_vec(),
        ),
        (
            C::KindMismatch,
            "docs/11.md",
            b"---\nid: R-11\nkind: decision\n---\n".to_vec(),
        ),
        (
            C::DuplicateId,
            "docs/12.md",
            b"---\nid: R-12\n---\n# T\n\n## Again {#R-12}\n".to_vec(),
        ),
        (
            C::BadRev,
            "docs/13.md",
            b"---\nid: R-13\nrev: 1234567890\n---\n".to_vec(),
        ),
    ]
}

#[test]
fn the_one_table_covers_every_parser_code() {
    assert_eq!(PARSER_SEVERITY.len(), DiagnosticCode::ALL.len());
    for code in DiagnosticCode::ALL {
        let entries: Vec<Severity> = PARSER_SEVERITY
            .iter()
            .filter(|(known, _)| *known == code)
            .map(|&(_, severity)| severity)
            .collect();
        let want = match code {
            DiagnosticCode::Homoglyph | DiagnosticCode::DuplicateId => Severity::Error,
            other => other.severity(),
        };
        assert_eq!(entries, [want], "{}", code.as_str());
        assert_eq!(check::parser_severity(code), want);
    }
    // The check's own codes are the spec's (Data, "Finding"): 18 errors and
    // the warning `name-skipped`; a stale entry is no finding.
    assert_eq!(
        check::CHECK_CODES,
        [
            "class-missing",
            "class-unknown",
            "key-missing",
            "key-extra",
            "scope-empty",
            "date-invalid",
            "status-invalid",
            "shipped-missing",
            "canon-missing",
            "tier-invalid",
            "id-width",
            "id-taken",
            "file-name",
            "budget",
            "canon-form",
            "canon-file",
            "canon-anchor",
            "ref-dangling",
            "name-skipped",
        ]
    );
    // No check code shadows a parser code.
    for code in DiagnosticCode::ALL {
        assert!(
            !check::CHECK_CODES.contains(&code.as_str()),
            "{}",
            code.as_str()
        );
    }
}

#[test]
fn every_parser_code_reaches_the_report_and_parser_errors_block() {
    let config = config_in("enforce");
    let cases = one_per_parser_code();
    assert_eq!(cases.len(), DiagnosticCode::ALL.len());
    let mut failures = Vec::new();
    for (code, path, bytes) in cases {
        let input = CheckInput {
            files: vec![CheckFile::parse(path, bytes, &config.scheme)],
            problems: Vec::new(),
        };
        let report = config.run(&input);
        let found: Vec<&check::Finding> = report
            .findings
            .iter()
            .filter(|f| f.code == code.as_str())
            .collect();
        let severity = check::parser_severity(code);
        if found.is_empty() {
            failures.push(format!(
                "{}: not in the report:\n{}",
                code.as_str(),
                show(&report)
            ));
            continue;
        }
        if found
            .iter()
            .any(|f| f.severity != severity || f.path != path)
        {
            failures.push(format!(
                "{}: severity or path:\n{}",
                code.as_str(),
                show(&report)
            ));
        }
        let blocks = found.iter().any(|f| report.blocks(f));
        if blocks != (severity == Severity::Error) {
            failures.push(format!("{}: blocks = {blocks}", code.as_str()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ------------------------------------------------------------------ AC-17

fn assert_same(a: &Report, b: &Report, what: &str) {
    assert_eq!(a.lines(true), b.lines(true), "{what}: lines");
    assert_eq!(a.lines(false), b.lines(false), "{what}: lines");
    assert_eq!(a.to_json(), b.to_json(), "{what}: JSON");
}

#[test]
fn reversed_input_gives_byte_identical_lines_and_json() {
    let config = config_in("enforce");
    let mut files: Vec<CheckFile> = one_per_parser_code()
        .into_iter()
        .map(|(_, path, bytes)| CheckFile::parse(path, bytes, &config.scheme))
        .collect();
    for (path, text) in [
        (
            "docs/b.md",
            "---\nid: R-20\nrefs: [R-99, R-98, R-97]\nfoo: 1\nbar: 2\n---\n# B\n",
        ),
        ("docs/a.md", "---\nid: R-20\n---\n# A\n\n## S {#R-21}\n"),
        ("docs/c.md", "---\nid: R-21\nclass: memo\n---\n"),
        ("docs/d.md", "# D\n"),
    ] {
        files.push(CheckFile::parse(
            path,
            text.as_bytes().to_vec(),
            &config.scheme,
        ));
    }
    files.push(CheckFile::unreadable("docs/x.md", "gone"));
    let problems = vec![
        Problem {
            kind: ProblemKind::UnreadableDir,
            path: "docs/q".to_owned(),
        },
        Problem {
            kind: ProblemKind::SkippedName,
            path: String::new(),
        },
        Problem {
            kind: ProblemKind::UnreadableDir,
            path: "docs/p".to_owned(),
        },
    ];
    let baseline = Baseline::from_toml(
        "[[debt]]\ncode = \"ref-dangling\"\npath = \"docs/b.md\"\nsubject = \"R-98\"\nreason = \"r\"\nexpires = \"2026-12-31\"\n\n[[debt]]\ncode = \"a\"\npath = \"z\"\nreason = \"r\"\nexpires = \"2026-12-31\"\n\n[[debt]]\ncode = \"a\"\npath = \"b\"\nreason = \"r\"\nexpires = \"2026-12-31\"\n",
    )
    .unwrap();
    let forward = CheckInput {
        files: files.clone(),
        problems: problems.clone(),
    };
    let mut reversed_files = files;
    reversed_files.reverse();
    let mut reversed_problems = problems;
    reversed_problems.reverse();
    let reversed = CheckInput {
        files: reversed_files,
        problems: reversed_problems,
    };
    let mut reversed_baseline = baseline.clone();
    reversed_baseline.entries.reverse();

    let a = config.run_with(&forward, &baseline, "2026-09-29");
    let b = config.run_with(&reversed, &reversed_baseline, "2026-09-29");
    assert!(a.findings.len() >= 20, "{}", show(&a));
    assert_same(&a, &b, "reversed");
    // Repeated runs too.
    assert_same(
        &a,
        &config.run_with(&forward, &baseline, "2026-09-29"),
        "repeat",
    );

    // Sorted by (path, line, code, subject, message).
    let keys: Vec<(&str, usize, &str, &str, &str)> = a
        .findings
        .iter()
        .map(|f| {
            (
                f.path.as_str(),
                f.line,
                f.code.as_str(),
                f.subject.as_str(),
                f.message.as_str(),
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    let stale: Vec<(&str, &str)> = a
        .stale
        .iter()
        .map(|e| (e.path.as_str(), e.code.as_str()))
        .collect();
    assert_eq!(stale, [("b", "a"), ("z", "a")]);
    let causes: Vec<&str> = a.cannot_check.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(causes, ["docs/p", "docs/q", "docs/x.md"]);
}

#[test]
fn the_fixture_reports_are_deterministic() {
    for corpus in ["spec-a", "spec-b"] {
        let (config, input) = fixture_input(&fixture(corpus));
        let mut reversed = input.clone();
        reversed.files.reverse();
        assert_same(&config.run(&input), &config.run(&reversed), corpus);
    }
}
