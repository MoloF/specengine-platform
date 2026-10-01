//! docs/features/spec-cli-introduced.md, the core half: AC-01 (the mode
//! `enforce-introduced`: its spelling in TOML and JSON, the ladder
//! `observe < enforce-introduced < enforce`, the default, an unknown mode
//! naming all three), AC-03 (no base: `enforce-introduced` judges as
//! `enforce`), and [`judge`] on real parsed documents — AC-04's key
//! (code, path, subject) by set membership, AC-08's blocking, AC-09's new
//! debt, the stricter mode, AC-16's field order. Everything runs in
//! memory: the base's findings are a real `check::run` over `HEAD`'s
//! files, with an empty baseline, as the store builds them.

mod common;

use std::collections::BTreeMap;

use common::check::{Config, TODAY, show};
use specengine_core::check::{Base, Baseline, CheckConfig, Finding, Mode, Report, Verdict, judge};
use specengine_model::Severity;

const TOML: &str =
    "[paths]\nroots = [\"docs\"]\n\n[ids]\nR = { kind = \"requirement\", width = 2 }\n";

/// `docs/a.md` whose one `ref-dangling` has the subject `R-98`.
const HEAD_A: &str = "---\nclass: generated\nid: R-01\nrefs: [R-98]\n---\n# R\n";

/// Another file's E1 (`ref-dangling`, `R-99`), its own ID.
const E1_ELSEWHERE: &str = "---\nclass: generated\nid: R-02\nrefs: [R-99]\n---\n# R\n";

/// `HEAD`'s `docs/a.md`: one `ref-dangling` (E1, subject `R-99`) on line 4.
const E1_ONCE: &str = "---\nclass: generated\nid: R-01\nrefs: [R-99]\n---\n# R\n";

/// The checked `docs/a.md`: E1's key on lines 5 and 7 (neither is line 4),
/// a new subject `R-98` (E2) on line 6.
const E1_TWICE_E2: &str =
    "---\nclass: generated\nid: R-01\nrefs:\n  - R-99\n  - R-98\n  - R-99\n---\n# R\n";

fn baseline(text: &str) -> Baseline {
    Baseline::from_toml(text).unwrap_or_else(|e| panic!("{}", e.at(".spec-debt.toml")))
}

fn entry(code: &str, path: &str, subject: &str, reason: &str, expires: &str) -> String {
    format!(
        "[[debt]]\ncode    = \"{code}\"\npath    = \"{path}\"\nsubject = \"{subject}\"\nreason  = \"{reason}\"\nexpires = \"{expires}\"\n\n"
    )
}

/// The base of `files` under `config`: `check::run` with an empty
/// baseline, its findings; `HEAD`'s baseline and mode as given.
fn base_of(
    config: &Config,
    files: &[(&str, &str)],
    head_baseline: Option<Baseline>,
    mode: Option<Mode>,
) -> Base {
    Base {
        findings: config.check(files).findings,
        baseline: head_baseline,
        mode,
    }
}

/// The checked run of `files` with `checked` as its baseline, judged.
fn judged(config: &Config, files: &[(&str, &str)], checked: &Baseline, base: &Base) -> Report {
    let report = config.run_with(&config.input(files), checked, TODAY);
    judge(report, checked, base)
}

fn dangling<'r>(report: &'r Report, subject: &str) -> Vec<&'r Finding> {
    report
        .findings
        .iter()
        .filter(|f| f.code == "ref-dangling" && f.subject == subject)
        .collect()
}

// ---------------------------------------------------------------------------
// AC-01: the mode.
// ---------------------------------------------------------------------------

/// AC-01: `"enforce-introduced"` reads as `EnforceIntroduced` from
/// `[check] mode` and through serde from TOML and JSON, and writes back
/// the same; `Observe < EnforceIntroduced < Enforce`; the default is
/// `enforce`.
#[test]
fn enforce_introduced_is_spelled_kebab_case_and_sits_between_the_two() {
    let config = CheckConfig::from_toml("[check]\nmode = \"enforce-introduced\"\n")
        .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert_eq!(config.mode, Mode::EnforceIntroduced);
    for (text, mode) in [
        ("observe", Mode::Observe),
        ("enforce-introduced", Mode::EnforceIntroduced),
        ("enforce", Mode::Enforce),
    ] {
        let toml_read: BTreeMap<String, Mode> =
            toml::from_str(&format!("mode = \"{text}\"\n")).expect("serde reads the TOML mode");
        assert_eq!(toml_read["mode"], mode, "TOML {text}");
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, format!("\"{text}\""), "JSON of {mode:?}");
        let back: Mode = serde_json::from_str(&json).expect("serde reads the JSON mode");
        assert_eq!(back, mode, "JSON {text}");
        assert_eq!(mode.as_str(), text);
        assert_eq!(mode.to_string(), text);
    }
    assert!(Mode::Observe < Mode::EnforceIntroduced);
    assert!(Mode::EnforceIntroduced < Mode::Enforce);
    assert_eq!(
        Mode::Observe.max(Mode::EnforceIntroduced),
        Mode::EnforceIntroduced
    );
    assert_eq!(Mode::EnforceIntroduced.max(Mode::Enforce), Mode::Enforce);
    assert_eq!(Mode::default(), Mode::Enforce);
    assert_eq!(
        CheckConfig::from_toml("[ids]\n").unwrap().mode,
        Mode::Enforce,
        "no [check]: enforce"
    );
    // The report's JSON carries the mode the same way.
    let report = Report::cannot(Mode::EnforceIntroduced, Vec::new());
    assert!(
        report
            .to_json()
            .starts_with("{\"mode\":\"enforce-introduced\","),
        "{}",
        report.to_json()
    );
}

/// AC-01: an unknown mode is an error at its line naming all three modes.
#[test]
fn an_unknown_mode_names_all_three_at_its_line() {
    let text = "[ids]\n\n[check]\nmode = \"strict\"\n";
    let error = CheckConfig::from_toml(text).expect_err("strict is no mode");
    let shown = error.at("specengine.toml");
    assert!(shown.starts_with("specengine.toml:4: "), "{shown}");
    for mode in ["observe", "enforce-introduced", "enforce", "strict"] {
        assert!(shown.contains(mode), "{mode} in {shown}");
    }
    // A near miss is unknown too.
    for near in [
        "enforce_introduced",
        "enforceintroduced",
        "Enforce-Introduced",
    ] {
        let text = format!("[check]\nmode = \"{near}\"\n");
        let error = CheckConfig::from_toml(&text).expect_err(near);
        assert!(
            error
                .at("specengine.toml")
                .starts_with("specengine.toml:2: "),
            "{near}: {}",
            error.at("specengine.toml")
        );
    }
}

// ---------------------------------------------------------------------------
// AC-03: no base.
// ---------------------------------------------------------------------------

/// AC-03 (2a.2 Q8): without a base (`introduced` absent) every report
/// judges alike under `enforce-introduced` and `enforce` — blocked by an
/// error, an expired entry, clean under live debt, observed... never; and
/// `without_base` shows `enforce-introduced` as `enforce`.
#[test]
fn without_a_base_enforce_introduced_is_enforce() {
    let config = Config::from_toml(TOML);
    let files = [("docs/a.md", E1_ONCE)];
    let live = baseline(&entry(
        "ref-dangling",
        "docs/a.md",
        "R-99",
        "r",
        "2999-12-31",
    ));
    let expired = baseline(&entry(
        "ref-dangling",
        "docs/a.md",
        "R-99",
        "r",
        "2000-01-01",
    ));
    let warned = [(
        "docs/w.md",
        "---\nclass: generated\nid: R-02\nflavour: x\n---\n# W\n",
    )];
    let cases: Vec<(&str, Report)> = vec![
        ("an error", config.check(&files)),
        (
            "live debt",
            config.run_with(&config.input(&files), &live, TODAY),
        ),
        (
            "expired debt",
            config.run_with(&config.input(&files), &expired, TODAY),
        ),
        ("a warning", config.check(&warned)),
        ("nothing", config.check(&[])),
    ];
    let mut seen = Vec::new();
    for (case, report) in &cases {
        assert!(
            report.findings.iter().all(|f| f.introduced.is_none()),
            "{case}: no base, no introduced"
        );
        let enforce = report.verdict_in(Mode::Enforce);
        assert_eq!(
            report.verdict_in(Mode::EnforceIntroduced),
            enforce,
            "{case}:\n{}",
            show(report)
        );
        seen.push(enforce);
        let mut shown = report.clone();
        shown.mode = Mode::EnforceIntroduced;
        let plain = shown.without_base();
        assert_eq!(plain.mode, Mode::Enforce, "{case}");
        assert_eq!(plain.verdict, enforce, "{case}");
    }
    assert_eq!(
        seen,
        [
            Verdict::Blocked,
            Verdict::Clean,
            Verdict::Blocked,
            Verdict::Clean,
            Verdict::Clean
        ]
    );
}

// ---------------------------------------------------------------------------
// AC-04: the key.
// ---------------------------------------------------------------------------

/// AC-04: `HEAD` holds E1 once; the checked file holds E1's key twice on
/// other lines and a new subject E2. Both E1-keyed findings are
/// pre-existing (the key has no line, a second occurrence is no count),
/// E2 is introduced (the subject is in the key); `counts.introduced` 1;
/// under `enforce-introduced` only E2 blocks.
#[test]
fn the_key_is_code_path_subject_by_set_membership() {
    let config = Config::from_toml(TOML);
    let base = base_of(
        &config,
        &[("docs/a.md", E1_ONCE)],
        Some(Baseline::empty()),
        None,
    );
    assert_eq!(base.findings.len(), 1, "{:?}", base.findings);
    assert_eq!(base.findings[0].line, 4);
    let mut report = judged(
        &config,
        &[("docs/a.md", E1_TWICE_E2)],
        &Baseline::empty(),
        &base,
    );
    let e1 = dangling(&report, "R-99");
    assert_eq!(
        e1.iter().map(|f| f.line).collect::<Vec<_>>(),
        [5, 7],
        "{}",
        show(&report)
    );
    assert!(
        e1.iter().all(|f| f.introduced == Some(false)),
        "E1's key twice: pre-existing\n{e1:?}"
    );
    let e2 = dangling(&report, "R-98");
    assert_eq!(e2.len(), 1, "{}", show(&report));
    assert_eq!(e2[0].introduced, Some(true), "E2: introduced");
    assert_eq!(report.counts.introduced, Some(1));
    assert!(report.findings.iter().all(|f| f.introduced.is_some()));
    let blocking: Vec<usize> = report
        .findings
        .iter()
        .filter(|f| f.blocks_in(Mode::EnforceIntroduced))
        .map(|f| f.line)
        .collect();
    assert_eq!(blocking, [6], "{}", show(&report));
    assert_eq!(report.verdict_in(Mode::EnforceIntroduced), Verdict::Blocked);
    assert_eq!(report.verdict_in(Mode::Enforce), Verdict::Blocked);

    // The same file at another path: everything introduced (a move).
    report = judged(
        &config,
        &[("docs/moved.md", E1_TWICE_E2)],
        &Baseline::empty(),
        &base,
    );
    assert!(
        report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .all(|f| f.introduced == Some(true)),
        "{}",
        show(&report)
    );
    assert_eq!(report.counts.introduced, Some(3));

    // The same code and path, another subject at HEAD: introduced.
    let other = base_of(
        &config,
        &[("docs/a.md", HEAD_A)],
        Some(Baseline::empty()),
        None,
    );
    let report = judged(
        &config,
        &[("docs/a.md", E1_ONCE)],
        &Baseline::empty(),
        &other,
    );
    assert_eq!(dangling(&report, "R-99")[0].introduced, Some(true));
}

// ---------------------------------------------------------------------------
// AC-08: blocking.
// ---------------------------------------------------------------------------

/// AC-08: under `enforce-introduced` a pre-existing error without debt does
/// not block (the run is `observed`, A2), nor one in live debt (`clean`);
/// one whose debt expired blocks (2a.2 Q4); an introduced error under an
/// entry `HEAD`'s baseline already holds does not block and is no new
/// debt. Under `enforce` every error outside live debt blocks.
#[test]
fn pre_existing_errors_block_only_when_their_debt_expired() {
    let config = Config::from_toml(TOML);
    let files = [("docs/a.md", E1_ONCE)];
    let head = base_of(&config, &files, Some(Baseline::empty()), None);

    // Pre-existing, no debt.
    let report = judged(&config, &files, &Baseline::empty(), &head);
    let e1 = &dangling(&report, "R-99")[0];
    assert_eq!(e1.introduced, Some(false));
    assert!(!e1.blocks_in(Mode::EnforceIntroduced));
    assert!(e1.blocks_in(Mode::Enforce));
    assert!(e1.is_pre_existing());
    assert_eq!(
        report.verdict_in(Mode::EnforceIntroduced),
        Verdict::Observed
    );
    assert_eq!(report.verdict_in(Mode::Enforce), Verdict::Blocked);
    assert_eq!(report.verdict_in(Mode::Observe), Verdict::Observed);
    assert_eq!(report.counts.introduced, Some(0));
    assert_eq!(report.counts.errors, 1);
    let lines = report.lines(true);
    assert!(
        lines.iter().any(|line| line.ends_with(" (pre-existing)")),
        "{lines:?}"
    );

    // Live debt, in HEAD's baseline too.
    let live_text = entry("ref-dangling", "docs/a.md", "R-99", "r", "2999-12-31");
    let live = baseline(&live_text);
    let head_live = base_of(&config, &files, Some(live.clone()), None);
    let report = judged(&config, &files, &live, &head_live);
    assert_eq!(report.verdict_in(Mode::EnforceIntroduced), Verdict::Clean);
    assert_eq!(report.new_debt.as_deref(), Some(&[][..]));

    // Expired debt over a pre-existing error: blocks.
    let expired_text = entry("ref-dangling", "docs/a.md", "R-99", "r", "2000-01-01");
    let expired = baseline(&expired_text);
    let head_expired = base_of(&config, &files, Some(expired.clone()), None);
    let report = judged(&config, &files, &expired, &head_expired);
    let e1 = &dangling(&report, "R-99")[0];
    assert_eq!(e1.introduced, Some(false));
    assert!(e1.debt.as_ref().is_some_and(|debt| debt.expired));
    assert!(e1.blocks_in(Mode::EnforceIntroduced));
    assert_eq!(report.verdict_in(Mode::EnforceIntroduced), Verdict::Blocked);
    assert_eq!(report.counts.expired, 1);

    // An introduced error under HEAD's existing (stale there) entry.
    let new_file = [("docs/a.md", E1_ONCE), ("docs/b.md", E1_ELSEWHERE)];
    let covering_b = baseline(&entry(
        "ref-dangling",
        "docs/b.md",
        "R-99",
        "r",
        "2999-12-31",
    ));
    let head_b = base_of(&config, &files, Some(covering_b.clone()), None);
    let report = judged(&config, &new_file, &covering_b, &head_b);
    let in_b: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.path == "docs/b.md" && f.code == "ref-dangling")
        .collect();
    assert_eq!(in_b.len(), 1, "{}", show(&report));
    assert_eq!(in_b[0].introduced, Some(true));
    assert!(in_b[0].is_live_debt());
    assert!(!in_b[0].blocks_in(Mode::EnforceIntroduced));
    assert_eq!(report.new_debt.as_deref(), Some(&[][..]));
    assert_eq!(
        report.counts.introduced,
        Some(0),
        "in live debt: not counted"
    );
    // `docs/a.md`'s E1 is pre-existing: observed, not blocked.
    assert_eq!(
        report.verdict_in(Mode::EnforceIntroduced),
        Verdict::Observed
    );
}

// ---------------------------------------------------------------------------
// AC-09: new debt.
// ---------------------------------------------------------------------------

/// The new-debt entries a case expects: path and `HEAD`'s `expires`.
type Expected<'a> = Vec<(&'a str, Option<&'a str>)>;

/// AC-09 (2a.2 Q5, Q7): against `HEAD`'s baseline, an added triple and a
/// later `expires` are new debt and block under `enforce-introduced` and
/// `enforce` (not `observe`); an earlier `expires`, a changed `reason`, a
/// removed entry, an unchanged one are not; an empty `HEAD` baseline (an
/// unborn `HEAD`) makes every entry new; a baseline not known (`None`)
/// lifts the rule.
#[test]
fn new_debt_is_an_added_triple_or_a_later_expiry() {
    let config = Config::from_toml(TOML);
    let files = [("docs/a.md", E1_ONCE)];
    let keep = entry("ref-dangling", "docs/a.md", "R-99", "covered", "2999-12-31");
    let stale = entry("budget", "docs/none.md", "", "stale", "2998-06-30");
    let head_text = format!("{keep}{stale}");
    let head_baseline = baseline(&head_text);
    let head = base_of(&config, &files, Some(head_baseline.clone()), None);

    let added = entry("ref-dangling", "docs/c.md", "R-97", "added", "2999-01-01");
    let cases: [(&str, String, Expected); 6] = [
        ("unchanged", head_text.clone(), vec![]),
        (
            "an added triple",
            format!("{keep}{stale}{added}"),
            vec![("docs/c.md", None)],
        ),
        (
            "a later expiry",
            format!(
                "{keep}{}",
                entry("budget", "docs/none.md", "", "stale", "2998-07-01")
            ),
            vec![("docs/none.md", Some("2998-06-30"))],
        ),
        (
            "an earlier expiry",
            format!(
                "{keep}{}",
                entry("budget", "docs/none.md", "", "stale", "2998-06-29")
            ),
            vec![],
        ),
        (
            "a changed reason",
            format!(
                "{keep}{}",
                entry("budget", "docs/none.md", "", "another reason", "2998-06-30")
            ),
            vec![],
        ),
        ("a removed entry", keep.clone(), vec![]),
    ];
    for (case, text, expected) in cases {
        let checked = baseline(&text);
        let report = judged(&config, &files, &checked, &head);
        let new: Vec<(&str, Option<&str>)> = report
            .new_debt
            .as_ref()
            .unwrap_or_else(|| panic!("{case}: HEAD's baseline is known"))
            .iter()
            .map(|new| (new.entry.path.as_str(), new.head_expires.as_deref()))
            .collect();
        assert_eq!(new, expected, "{case}");
        assert_eq!(report.counts.new_debt, Some(expected.len()), "{case}");
        let blocked = !expected.is_empty();
        for mode in [Mode::EnforceIntroduced, Mode::Enforce] {
            assert_eq!(
                report.verdict_in(mode) == Verdict::Blocked,
                blocked,
                "{case} under {mode}"
            );
        }
        assert_ne!(report.verdict_in(Mode::Observe), Verdict::Blocked, "{case}");
        if blocked {
            assert_eq!(
                report.verdict_in(Mode::Observe),
                Verdict::Observed,
                "{case}: new debt is observed"
            );
        }
    }

    // An empty HEAD baseline: every entry is new; not known: lifted.
    let checked = baseline(&format!("{keep}{stale}"));
    let empty = base_of(&config, &files, Some(Baseline::empty()), None);
    let report = judged(&config, &files, &checked, &empty);
    assert_eq!(report.new_debt.as_ref().map(Vec::len), Some(2));
    assert_eq!(report.verdict_in(Mode::Enforce), Verdict::Blocked);
    let lifted = base_of(&config, &files, None, None);
    let report = judged(&config, &files, &checked, &lifted);
    assert_eq!(report.new_debt, None);
    assert_eq!(report.counts.new_debt, None);
    assert_eq!(report.verdict_in(Mode::Enforce), Verdict::Clean);
    assert!(
        !report.to_json().contains("new_debt"),
        "{}",
        report.to_json()
    );
    assert!(report.to_json().contains("\"introduced\":"));
}

// ---------------------------------------------------------------------------
// The mode: the stricter one.
// ---------------------------------------------------------------------------

/// 2a.2 Q6: the judged mode is the stricter of the checked and the base's;
/// a base mode not known leaves the checked one.
#[test]
fn the_stricter_mode_applies() {
    let config = Config::from_toml(TOML);
    let files = [("docs/a.md", E1_ONCE)];
    for checked in Mode::ALL {
        for head in [
            None,
            Some(Mode::Observe),
            Some(Mode::EnforceIntroduced),
            Some(Mode::Enforce),
        ] {
            let base = base_of(&config, &files, Some(Baseline::empty()), head);
            let mut report = config.check(&files);
            report.mode = checked;
            let report = judge(report, &Baseline::empty(), &base);
            let expected = head.map_or(checked, |head| head.max(checked));
            assert_eq!(report.mode, expected, "checked {checked}, HEAD {head:?}");
            assert_eq!(report.verdict, report.verdict_in(expected));
        }
    }
}

// ---------------------------------------------------------------------------
// AC-16: the JSON field order with a base.
// ---------------------------------------------------------------------------

/// AC-16: with a base, `counts.introduced` and `counts.new_debt` follow
/// `stale` and precede `worst_w_bytes`, the last count; each finding's
/// `introduced` is its last field; `new_debt` follows `stale`; the summary
/// adds `, <i> introduced, <d> new debt` before `, worst W`.
#[test]
fn the_base_fields_keep_their_places() {
    let config = Config::from_toml(TOML);
    let head = base_of(
        &config,
        &[("docs/a.md", E1_ONCE)],
        Some(Baseline::empty()),
        None,
    );
    let checked = baseline(&entry("budget", "docs/none.md", "", "r", "2999-01-01"));
    let report = judged(&config, &[("docs/a.md", E1_TWICE_E2)], &checked, &head);
    let json = report.to_json();
    assert!(
        json.contains(",\"stale\":1,\"introduced\":1,\"new_debt\":1,\"worst_w_bytes\":"),
        "{json}"
    );
    assert!(json.contains(",\"introduced\":true}"), "{json}");
    assert!(json.contains(",\"introduced\":false}"), "{json}");
    let stale_at = json
        .find("],\"new_debt\":[{")
        .expect("new_debt after stale");
    let causes_at = json.find(",\"cannot_check\":").expect("cannot_check");
    assert!(stale_at < causes_at, "{json}");
    assert!(
        json.contains(
            "\"new_debt\":[{\"code\":\"budget\",\"path\":\"docs/none.md\",\"subject\":\"\",\"reason\":\"r\",\"expires\":\"2999-01-01\",\"line\":1}]"
        ),
        "{json}"
    );
    let lines = report.lines(false);
    let summary = lines.last().unwrap();
    assert!(
        summary.contains(" stale, 1 introduced, 1 new debt, worst W "),
        "{summary}"
    );
    assert!(
        lines.iter().any(|line| line
            == "new  docs/none.md: debt-new: the baseline entry at line 1 (budget, subject \"\") is not in HEAD's baseline (r; expires 2999-01-01)"),
        "{lines:?}"
    );
}
