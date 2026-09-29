//! AC-13 of docs/features/spec-check.md: the expiring debt baseline
//! (`.spec-debt.toml`). An entry matches every finding with its (code,
//! path, subject), never by line; a match is debt until `today > expires`,
//! then an error blocks again, counted `expired`, and a warning stays a
//! warning; an entry matching nothing goes to `Report.stale`, counted
//! `stale`, not in warnings; a
//! missing `reason` or `expires`, a bad date, an unknown key, a repeated
//! triple or bad TOML is an error `file:line: message` (the store turns it
//! into "cannot check", `tests/check_verdict.rs`).

mod common;

use common::check::{Config, show, with_code};
use specengine_core::check::{Baseline, Mode, Verdict};
use specengine_model::Severity;

const TOML: &str = "[ids]\nR = { kind = \"requirement\", width = 2 }\n";

/// `ref-dangling` for `R-99` on line 4.
const CITING: &str = "---\nclass: generated\nid: R-01\nrefs: [R-99]\n---\n# R\n";
/// The same with a line inserted above: `ref-dangling` on line 5.
const CITING_SHIFTED: &str =
    "---\nclass: generated\nid: R-01\nstatus: open\nrefs: [R-99]\n---\n# R\n";

/// The entry starts on line 4, the line of the finding in [`CITING`].
const BASELINE: &str = "\
# debt accepted by the owner
# (reviewed 2026-09-29)

[[debt]]
code    = \"ref-dangling\"
path    = \"docs/r-01.md\"
subject = \"R-99\"
reason  = \"core Q6\"
expires = \"2026-12-31\"
";

fn baseline(text: &str) -> Baseline {
    Baseline::from_toml(text).unwrap_or_else(|e| panic!("{}", e.at(".spec-debt.toml")))
}

#[test]
fn a_match_is_debt_and_survives_a_line_inserted_above() {
    let config = Config::from_toml(TOML);
    let baseline = baseline(BASELINE);
    assert_eq!(baseline.entries.len(), 1);
    // The entry's own line equals the finding's line in [`CITING`]: a match
    // by line would pass the first case and fail the shifted one.
    assert_eq!(baseline.entries[0].line, 4);
    for (case, text, line) in [("as is", CITING, 4), ("shifted", CITING_SHIFTED, 5)] {
        let input = config.input(&[("docs/r-01.md", text)]);
        let report = config.run_with(&input, &baseline, "2026-09-29");
        let found = with_code(&report, "ref-dangling");
        assert_eq!(found.len(), 1, "{case}:\n{}", show(&report));
        assert_eq!(found[0].line, line, "{case}");
        let debt = found[0]
            .debt
            .as_ref()
            .unwrap_or_else(|| panic!("{case}: no debt"));
        assert_eq!(
            (debt.reason.as_str(), debt.expires.as_str(), debt.expired),
            ("core Q6", "2026-12-31", false),
            "{case}"
        );
        assert!(found[0].is_live_debt() && !found[0].blocks_when_enforced());
        assert_eq!(
            (
                report.counts.debt,
                report.counts.errors,
                report.counts.stale
            ),
            (1, 0, 0),
            "{case}"
        );
        assert_eq!(report.verdict, Verdict::Clean, "{case}: debt never blocks");
        assert!(report.stale.is_empty());
    }
}

#[test]
fn past_expires_the_match_is_an_error_counted_expired() {
    let config = Config::from_toml(TOML);
    let baseline = baseline(BASELINE);
    let input = config.input(&[("docs/r-01.md", CITING)]);
    let on_the_day = config.run_with(&input, &baseline, "2026-12-31");
    assert_eq!(on_the_day.counts.debt, 1, "`today > expires`, not `>=`");
    assert_eq!(on_the_day.verdict, Verdict::Clean);
    let after = config.run_with(&input, &baseline, "2027-01-01");
    let found = with_code(&after, "ref-dangling");
    assert_eq!(found.len(), 1);
    assert!(found[0].debt.as_ref().is_some_and(|d| d.expired));
    assert_eq!(found[0].severity, Severity::Error);
    assert!(found[0].blocks_when_enforced());
    assert_eq!(
        (after.counts.debt, after.counts.errors, after.counts.expired),
        (0, 1, 1)
    );
    assert_eq!(after.verdict, Verdict::Blocked);
    assert_eq!(after.verdict_in(Mode::Observe), Verdict::Observed);
    let lines = after.lines(false);
    assert!(
        lines[0].starts_with("error  docs/r-01.md:4: ref-dangling: ")
            && lines[0].contains("debt expired 2026-12-31"),
        "{lines:?}"
    );
}

#[test]
fn an_unmatched_entry_is_stale_and_never_blocks() {
    let config = Config::from_toml(TOML);
    let baseline = baseline(BASELINE);
    let input = config.input(&[(
        "docs/r-01.md",
        "---\nclass: generated\nid: R-01\n---\n# R\n",
    )]);
    let report = config.run_with(&input, &baseline, "2026-09-29");
    assert!(report.findings.is_empty(), "{}", show(&report));
    assert_eq!(report.stale, baseline.entries);
    assert_eq!(report.counts.stale, 1);
    assert_eq!(report.counts.warnings, 0, "a stale entry is no warning");
    assert_eq!(report.verdict, Verdict::Clean);
    assert_eq!(
        report.lines(false).len(),
        1,
        "stale entries only with detail"
    );
    let detail = report.lines(true);
    assert!(
        detail
            .iter()
            .any(|line| line.starts_with("stale  docs/r-01.md: debt-stale: ")),
        "{detail:?}"
    );
    // Another path or subject does not match either.
    for other in [
        BASELINE.replace("docs/r-01.md", "docs/r-02.md"),
        BASELINE.replace("\"R-99\"", "\"R-98\""),
        BASELINE.replace("ref-dangling", "id-width"),
    ] {
        let input = config.input(&[("docs/r-01.md", CITING)]);
        let report = config.run_with(&input, &self::baseline(&other), "2026-09-29");
        assert_eq!(report.counts.debt, 0);
        assert_eq!(report.counts.stale, 1);
        assert_eq!(report.verdict, Verdict::Blocked);
    }
}

#[test]
fn one_entry_matches_every_finding_with_its_triple() {
    // Two files without a class: `class-missing` has the subject "" (the
    // default); one entry per path.
    let config = Config::from_toml(TOML);
    let input = config.input(&[("docs/a.md", "# A\n"), ("docs/b.md", "# B\n")]);
    let baseline = baseline(
        "[[debt]]\ncode = \"class-missing\"\npath = \"docs/a.md\"\nreason = \"legacy\"\nexpires = \"2026-12-31\"\n",
    );
    let report = config.run_with(&input, &baseline, "2026-09-29");
    let debt: Vec<(&str, bool)> = report
        .findings
        .iter()
        .map(|f| (f.path.as_str(), f.is_live_debt()))
        .collect();
    assert_eq!(debt, [("docs/a.md", true), ("docs/b.md", false)]);
    // Two findings with one triple in one file are both debt.
    let input = config.input(&[(
        "docs/r-01.md",
        "---\nid: R-01\nrefs: [R-99]\nworking_answer: R-99\n---\n",
    )]);
    let report = config.run_with(&input, &self::baseline(BASELINE), "2026-09-29");
    assert_eq!(report.counts.debt, 2, "{}", show(&report));
}

#[test]
fn a_matched_warning_is_debt_and_stays_a_warning_when_expired() {
    // An entry matches a warning too; past `expires` it is a warning again
    // (counted in warnings, not `expired`), never an error.
    let config = Config::from_toml(TOML);
    let input = config.input(&[(
        "docs/r-01.md",
        "---\nclass: generated\nid: R-01\nfoo: bar\n---\n",
    )]);
    let entry = "[[debt]]\ncode = \"unknown-key\"\npath = \"docs/r-01.md\"\nreason = \"x\"\nexpires = \"2026-12-31\"\n";
    let live = config.run_with(&input, &baseline(entry), "2026-09-29");
    assert_eq!(
        (live.counts.debt, live.counts.warnings),
        (1, 0),
        "{}",
        show(&live)
    );
    assert_eq!(live.lines(true)[0].split("  ").next(), Some("debt"));
    let expired = config.run_with(&input, &baseline(entry), "2027-01-01");
    let finding = &expired.findings[0];
    assert_eq!(finding.severity, Severity::Warning, "{}", show(&expired));
    assert!(finding.debt.as_ref().is_some_and(|d| d.expired));
    assert!(!finding.blocks_when_enforced());
    assert_eq!(
        (
            expired.counts.warnings,
            expired.counts.errors,
            expired.counts.expired,
            expired.counts.debt
        ),
        (1, 0, 0, 0)
    );
    assert_eq!(expired.verdict, Verdict::Clean);
    let detail = expired.lines(true);
    assert!(
        detail[0].starts_with("warning  docs/r-01.md:4: unknown-key: ")
            && detail[0].ends_with("(debt expired 2026-12-31: x)"),
        "{detail:?}"
    );
    assert_eq!(
        expired.lines(false).len(),
        1,
        "a warning is listed only with detail"
    );
}

#[test]
fn an_invalid_today_is_cannot_check() {
    let config = Config::from_toml(TOML);
    let input = config.input(&[("docs/r-01.md", CITING)]);
    for today in ["2026-13-01", "tomorrow", "2026-9-29", ""] {
        let report = config.run_with(&input, &Baseline::empty(), today);
        assert_eq!(report.verdict, Verdict::CannotCheck, "today {today:?}");
        assert_eq!(report.exit_code(), 2);
    }
}

/// `(case, text, line)`: each must fail at that line.
const INVALID: &[(&str, &str, usize)] = &[
    (
        "no reason",
        "[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nsubject = \"tier0\"\nexpires = \"2026-12-31\"\n",
        1,
    ),
    (
        "no expires",
        "\n[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nreason = \"x\"\n",
        2,
    ),
    (
        "bad date",
        "[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nreason = \"x\"\nexpires = \"2026-02-30\"\n",
        5,
    ),
    (
        "date of another shape",
        "[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nreason = \"x\"\nexpires = \"31.12.2026\"\n",
        5,
    ),
    (
        "unknown key",
        "[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nline = 3\nreason = \"x\"\nexpires = \"2026-12-31\"\n",
        4,
    ),
    (
        "repeated triple",
        "[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nreason = \"x\"\nexpires = \"2026-12-31\"\n\n[[debt]]\ncode = \"budget\"\npath = \"CLAUDE.md\"\nreason = \"y\"\nexpires = \"2027-12-31\"\n",
        7,
    ),
    ("bad TOML", "[[debt]]\ncode = \"budget\npath = 1\n", 2),
];

#[test]
fn an_invalid_baseline_fails_with_file_line_message() {
    let mut failures = Vec::new();
    for (case, text, line) in INVALID {
        match Baseline::from_toml(text) {
            Ok(baseline) => failures.push(format!("{case}: accepted as {baseline:?}")),
            Err(error) => {
                let shown = error.at(".spec-debt.toml");
                if !shown.starts_with(&format!(".spec-debt.toml:{line}: ")) {
                    failures.push(format!("{case}: {shown}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(Baseline::from_toml("").unwrap(), Baseline::empty());
}
