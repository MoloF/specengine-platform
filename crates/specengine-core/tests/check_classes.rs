//! AC-06 of docs/features/spec-check.md: class contracts, and the class
//! rules of "Rules and edge cases". Default contracts are open (an untyped
//! extra key is only the parser's `unknown-key` warning); `closed = true`
//! adds `key-extra`; a document without `class:` (with a scheme `id:`,
//! without one, or without front-matter) is `class-missing` at line 1 with
//! no contract finding and no class cap, its IDs, references and `canon:`
//! still checked (Q-4, owner's answer); a front-matter failure gives only
//! its parser finding.

mod common;

use common::check::{Config, codes, show, triples, with_code};
use specengine_core::check::Verdict;
use specengine_model::Severity;

const DEFAULT: &str =
    "[ids]\nADR = { kind = \"decision\", width = 4 }\nR = { kind = \"requirement\", width = 2 }\n";

fn default_config() -> Config {
    Config::from_toml(DEFAULT)
}

fn check_one(config: &Config, path: &str, text: &str) -> specengine_core::check::Report {
    config.check(&[(path, text)])
}

const CANON_OK: &str = "---\nclass: canon\nowner: owner\nreviewed: 2026-09-01\n---\n# C\n";

#[test]
fn a_document_meeting_its_default_contract_is_clean() {
    let config = default_config();
    for (path, text) in [
        ("docs/c.md", CANON_OK),
        (
            "docs/decisions/ADR-0001.md",
            "---\nid: ADR-0001\nclass: decision\nstatus: rejected\nscope: [x]\n---\n# D\n",
        ),
        (
            "docs/features/f.md",
            "---\nclass: spec\nstatus: draft\nscope: [core]\n---\n# F\n",
        ),
        ("docs/g.md", "---\nclass: generated\n---\n# G\n"),
    ] {
        let report = check_one(&config, path, text);
        assert!(report.findings.is_empty(), "{path}:\n{}", show(&report));
        assert_eq!(report.verdict, Verdict::Clean);
    }
}

#[test]
fn canon_without_reviewed_is_key_missing() {
    let report = check_one(
        &default_config(),
        "docs/c.md",
        "---\nclass: canon\nowner: owner\n---\n# C\n",
    );
    assert_eq!(
        triples(&report),
        [("docs/c.md".into(), "key-missing".into(), "reviewed".into())]
    );
    assert_eq!(report.findings[0].severity, Severity::Error);
    assert_eq!(report.verdict, Verdict::Blocked);
}

#[test]
fn every_default_required_key_is_enforced() {
    let config = default_config();
    let missing = |text: &str| -> Vec<String> {
        with_code(&check_one(&config, "docs/x.md", text), "key-missing")
            .iter()
            .map(|f| f.subject.clone())
            .collect()
    };
    assert_eq!(missing("---\nclass: canon\n---\n"), ["owner", "reviewed"]);
    assert_eq!(
        missing("---\nclass: decision\n---\n"),
        ["id", "scope", "status"]
    );
    assert_eq!(missing("---\nclass: spec\n---\n"), ["scope", "status"]);
    assert!(missing("---\nclass: generated\n---\n").is_empty());
}

#[test]
fn an_untyped_extra_key_under_an_open_contract_is_only_unknown_key() {
    let text = "---\nclass: canon\nowner: owner\nreviewed: 2026-09-01\nfoo: bar\n---\n# C\n";
    let report = check_one(&default_config(), "docs/c.md", text);
    assert_eq!(codes(&report), ["unknown-key"], "{}", show(&report));
    assert_eq!(report.findings[0].severity, Severity::Warning);
    assert_eq!(report.findings[0].line, 5);
    assert_eq!(report.verdict, Verdict::Clean, "a warning never blocks");
}

#[test]
fn a_closed_contract_reports_every_other_key_as_key_extra() {
    let config = Config::from_toml(&format!(
        "{DEFAULT}\n[classes]\ncanon = {{ required = [\"owner\"], optional = [\"tier\"], closed = true }}\n"
    ));
    let text =
        "---\nclass: canon\nowner: owner\ntier: 2\nreviewed: 2026-09-01\nfoo: bar\n---\n# C\n";
    let report = check_one(&config, "docs/c.md", text);
    let extra: Vec<(usize, &str)> = with_code(&report, "key-extra")
        .iter()
        .map(|f| (f.line, f.subject.as_str()))
        .collect();
    // `class` is always allowed; `reviewed` is known to the parser but not
    // in this contract.
    assert_eq!(extra, [(5, "reviewed"), (6, "foo")], "{}", show(&report));
    assert!(with_code(&report, "key-missing").is_empty());
    // The same file under the default (open) contract: no key-extra.
    let open = check_one(&default_config(), "docs/c.md", text);
    assert!(with_code(&open, "key-extra").is_empty(), "{}", show(&open));
}

const CLASS_MISSING: &str = "no `class:`: declare one of canon | decision | spec | generated";

#[test]
fn no_class_is_class_missing_with_an_id_without_one_and_without_front_matter() {
    let config = default_config();
    let report = config.check(&[
        (
            "docs/records/R-12.md",
            "---\nid: R-12\nstatus: open\n---\n# R\n",
        ),
        (
            "docs/decisions/ADR-0005.md",
            "---\nid: ADR-0005\ntitle: T\n---\n# D\n",
        ),
        ("docs/notes.md", "---\ntitle: Notes\n---\n# Notes\n"),
        ("docs/bare.md", "# Bare\n"),
    ]);
    let found: Vec<(&str, &str, usize, &str, &str, Severity)> = report
        .findings
        .iter()
        .map(|f| {
            (
                f.path.as_str(),
                f.code.as_str(),
                f.line,
                f.subject.as_str(),
                f.message.as_str(),
                f.severity,
            )
        })
        .collect();
    let want = |path: &'static str| (path, "class-missing", 1, "", CLASS_MISSING, Severity::Error);
    assert_eq!(
        found,
        [
            want("docs/bare.md"),
            want("docs/decisions/ADR-0005.md"),
            want("docs/notes.md"),
            want("docs/records/R-12.md"),
        ],
        "{}",
        show(&report)
    );
    assert_eq!(report.counts.errors, 4);
    assert_eq!(report.counts.documents, 4);
    assert_eq!(report.verdict, Verdict::Blocked);
}

#[test]
fn a_class_less_file_gets_no_contract_finding_and_no_class_cap_but_its_ids_refs_and_canon() {
    let config = default_config();
    // Everything a decision contract or cap would flag: no scope, a bad
    // status, an empty scope, accepted without `canon:`, 20 kB.
    let big = format!(
        "---\nid: ADR-0006\nstatus: accepted\nscope: []\nreviewed: 2026-9-1\n---\n# D\n\n{}\n",
        "x".repeat(20_000)
    );
    let report = config.check(&[("docs/decisions/ADR-0006.md", big.as_str())]);
    assert_eq!(codes(&report), ["class-missing"], "{}", show(&report));
    // IDs, references and a path `canon:` are still judged.
    let report = config.check(&[(
        "docs/decisions/ADR-0007.md",
        "---\nid: ADR-007\nsupersedes: [ADR-0099]\ncanon: docs/canon/a.md\n---\n# D\n",
    )]);
    let mut got = codes(&report);
    got.sort_unstable();
    assert_eq!(
        got,
        ["canon-form", "class-missing", "id-width", "ref-dangling"],
        "{}",
        show(&report)
    );
}

#[test]
fn an_unknown_class_is_class_unknown() {
    let report = check_one(
        &default_config(),
        "docs/m.md",
        "---\ntitle: M\nclass: memo\n---\n# M\n",
    );
    let found = with_code(&report, "class-unknown");
    assert_eq!(found.len(), 1, "{}", show(&report));
    assert_eq!((found[0].line, found[0].subject.as_str()), (3, "memo"));
    assert!(
        with_code(&report, "class-missing").is_empty(),
        "a declared class, if unknown, is not missing"
    );
}

#[test]
fn a_front_matter_failure_gives_only_its_parser_finding() {
    let config = default_config();
    let failures = [
        (
            "frontmatter-yaml",
            "---\nid: ADR-0003\nclass: decision\nstatus: accepted\ntitle: A: B\n---\n# D\n",
        ),
        (
            "frontmatter-unclosed",
            "---\nid: ADR-0003\nclass: decision\n# D\n",
        ),
        (
            "frontmatter-not-mapping",
            "---\n- class\n- decision\n---\n# D\n",
        ),
    ];
    for (code, text) in failures {
        let report = check_one(&config, "docs/decisions/ADR-0003.md", text);
        assert_eq!(codes(&report), [code], "{}", show(&report));
        assert_eq!(report.findings[0].severity, Severity::Error);
    }
    let mut not_utf8 = b"---\nclass: decision\n---\n# D ".to_vec();
    not_utf8.extend_from_slice(&[0xff, 0xfe, b'\n']);
    let input = specengine_core::check::CheckInput {
        files: vec![specengine_core::check::CheckFile::parse(
            "docs/decisions/ADR-0004.md",
            not_utf8,
            &config.scheme,
        )],
        problems: Vec::new(),
    };
    let report = config.run(&input);
    assert_eq!(codes(&report), ["not-utf8"], "{}", show(&report));
}

#[test]
fn scope_dates_and_statuses() {
    let config = default_config();
    let run = |path: &str, text: &str| {
        let report = check_one(&config, path, text);
        report
            .findings
            .iter()
            .map(|f| (f.code.clone(), f.subject.clone(), f.line))
            .collect::<Vec<_>>()
    };
    let t = |code: &str, subject: &str, line: usize| (code.to_owned(), subject.to_owned(), line);
    assert_eq!(
        run(
            "docs/features/f.md",
            "---\nclass: spec\nstatus: draft\nscope: []\n---\n"
        ),
        [t("scope-empty", "scope", 4)]
    );
    assert_eq!(
        run(
            "docs/c.md",
            "---\nclass: canon\nowner: o\nreviewed: 2026-9-1\n---\n"
        ),
        [t("date-invalid", "reviewed", 4)]
    );
    assert_eq!(
        run(
            "docs/features/f.md",
            "---\nclass: spec\nstatus: shipped\nshipped: 2026/09/01\nscope: [x]\n---\n"
        ),
        [t("date-invalid", "shipped", 4)]
    );
    assert_eq!(
        run(
            "docs/features/f.md",
            "---\nclass: spec\nstatus: done\nscope: [x]\n---\n"
        ),
        [t("status-invalid", "status", 3)]
    );
    assert_eq!(
        run(
            "docs/features/f.md",
            "---\nclass: spec\nscope: [x]\nstatus: shipped\n---\n"
        ),
        [t("shipped-missing", "shipped", 4)]
    );
    assert_eq!(
        run(
            "docs/decisions/ADR-0001.md",
            "---\nid: ADR-0001\nclass: decision\nstatus: accepted\nscope: [x]\n---\n"
        ),
        [t("canon-missing", "canon", 4)]
    );
    assert_eq!(
        run(
            "docs/decisions/ADR-0001.md",
            "---\nid: ADR-0001\nclass: decision\nstatus: proposed\nscope: [x]\ndate: 2026-13-45\n---\n"
        ),
        // Front-matter dates are judged by shape only.
        [t("status-invalid", "status", 4)]
    );
    let report = config.check(&[
        (
            "docs/decisions/ADR-0001.md",
            "---\nid: ADR-0001\nclass: decision\nstatus: superseded-by ADR-0002\nscope: [x]\n---\n",
        ),
        (
            "docs/decisions/ADR-0002.md",
            "---\nid: ADR-0002\nclass: decision\nstatus: rejected\nscope: [x]\n---\n",
        ),
    ]);
    assert!(report.findings.is_empty(), "{}", show(&report));
}

#[test]
fn canon_tiers() {
    let config = Config::from_toml(&format!(
        "{DEFAULT}\n[paths]\ntier0 = \"CLAUDE.md\"\ntier1_name = \"README.md\"\n"
    ));
    let canon =
        |tier: &str| format!("---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n{tier}---\n# C\n");
    let tier_findings = |path: &str, text: &str| -> usize {
        with_code(&check_one(&config, path, text), "tier-invalid").len()
    };
    assert_eq!(tier_findings("CLAUDE.md", &canon("tier: 0\n")), 0);
    assert_eq!(tier_findings("docs/README.md", &canon("tier: 1\n")), 0);
    assert_eq!(tier_findings("docs/a.md", &canon("tier: 2\n")), 0);
    assert_eq!(tier_findings("docs/a.md", &canon("")), 0);
    assert_eq!(tier_findings("docs/a.md", &canon("tier: 3\n")), 1, "tier 3");
    assert_eq!(
        tier_findings("docs/a.md", &canon("tier: 0\n")),
        1,
        "0 off tier0"
    );
    assert_eq!(
        tier_findings("docs/a.md", &canon("tier: 1\n")),
        1,
        "1 off README"
    );
    // The tier0 file gets only its own rule (K2 of
    // docs/features/phase1-cleanup.md): one wrong tier is one finding.
    assert_eq!(
        tier_findings("CLAUDE.md", &canon("tier: 1\n")),
        1,
        "tier0 at 1"
    );
    assert_eq!(
        tier_findings("CLAUDE.md", &canon("tier: 2\n")),
        1,
        "tier0 at 2"
    );
    assert_eq!(
        tier_findings(
            "CLAUDE.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n"
        ),
        1,
        "tier0 not canon"
    );
    // Each rule is off while its key is absent.
    let open = default_config();
    for (path, tier) in [("docs/a.md", "tier: 0\n"), ("docs/a.md", "tier: 1\n")] {
        let report = check_one(&open, path, &canon(tier));
        assert!(
            report.findings.is_empty(),
            "{path} {tier}: {}",
            show(&report)
        );
    }
}

// ------------------------------------------ docs/features/phase1-cleanup.md

/// AC-07 (K2): the `[paths] tier0` file gets only its own tier rule, so
/// one wrong tier is one `tier-invalid`, whatever other rule it breaks.
#[test]
fn the_tier0_file_gets_one_tier_invalid_per_wrong_tier() {
    let config = Config::from_toml(&format!(
        "{DEFAULT}\n[paths]\ntier0 = \"CLAUDE.md\"\ntier1_name = \"README.md\"\n"
    ));
    let canon =
        |tier: &str| format!("---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n{tier}---\n# C\n");
    for (tier, want) in [("tier: 0\n", 0), ("tier: 1\n", 1), ("tier: 5\n", 1)] {
        let report = check_one(&config, "CLAUDE.md", &canon(tier));
        let found = with_code(&report, "tier-invalid");
        assert_eq!(found.len(), want, "{tier}: {}", show(&report));
        for finding in found {
            assert_eq!(
                finding.message, "CLAUDE.md is the tier 0 file: tier: 0",
                "{tier}: the tier0 rule speaks"
            );
            assert_eq!((finding.line, finding.subject.as_str()), (5, "tier"));
        }
    }
}

/// `canon-missing` of an accepted decision with the given `canon:` lines.
fn canon_missing(canon: &str) -> Vec<String> {
    let text = format!(
        "---\nid: ADR-0001\nclass: decision\nstatus: accepted\nscope: [x]\n{canon}---\n# D\n"
    );
    let report = check_one(&default_config(), "docs/decisions/ADR-0001.md", &text);
    with_code(&report, "canon-missing")
        .into_iter()
        .map(|f| {
            assert_eq!(
                (f.line, f.subject.as_str()),
                (4, "canon"),
                "{canon:?}: line and subject unchanged"
            );
            f.message.clone()
        })
        .collect()
}

/// AC-08 (K3): a `canon:` the parser could not read is "unreadable", not
/// "has no `canon:`"; an absent, null, empty or blank `canon:` has none.
#[test]
fn an_unreadable_canon_is_not_reported_as_absent() {
    const ABSENT: &str = "an accepted decision has no `canon:` (the promotion rule)";
    const UNREADABLE: &str =
        "the written `canon:` of an accepted decision is unreadable (the promotion rule)";
    for canon in [
        "canon: docs/x.md#\n",
        "canon: \"#\"\n",
        "canon: [a]\n",
        "canon: {a: b}\n",
    ] {
        assert_eq!(canon_missing(canon), [UNREADABLE], "{canon:?}");
    }
    for canon in [
        "",
        "canon:\n",
        "canon: ~\n",
        "canon: null\n",
        "canon: \"\"\n",
        "canon: ''\n",
        "canon: '  '\n",
        "canon: \" \"\n",
    ] {
        assert_eq!(canon_missing(canon), [ABSENT], "{canon:?}");
    }
    // Known limit: an escaped all-whitespace string has no span to judge
    // the written text by, so it stays unreadable.
    assert_eq!(canon_missing("canon: \"\\t\"\n"), [UNREADABLE]);
    // A readable `canon:` has no finding.
    assert!(canon_missing("canon: docs/x.md#layout\n").is_empty());
}

/// K1 of docs/features/phase1-cleanup.md: a top-level key after a tag or an
/// anchor, or an explicit scalar key, is a written key for the class
/// contract too: it meets `required` and is judged by `closed`.
#[test]
fn tagged_anchored_and_explicit_keys_are_seen_by_the_class_contract() {
    let config = Config::from_toml(&format!(
        "{DEFAULT}\n[classes]\ngenerated = {{ required = [\"owner\"], closed = true }}\n"
    ));
    let t = |code: &str, line: usize, subject: &str| (code.to_owned(), line, subject.to_owned());
    let run = |text: &str| -> Vec<(String, usize, String)> {
        check_one(&config, "docs/g.md", text)
            .findings
            .iter()
            .filter(|f| f.code.starts_with("key-"))
            .map(|f| (f.code.clone(), f.line, f.subject.clone()))
            .collect()
    };
    // The plain form, for contrast.
    assert_eq!(
        run("---\nclass: generated\nx_extra: 1\n---\n"),
        [t("key-missing", 1, "owner"), t("key-extra", 3, "x_extra")]
    );
    for (case, text) in [
        (
            "tagged required, anchored extra",
            "---\nclass: generated\n!!str owner: me\n&a x_extra: 1\n---\n",
        ),
        (
            "anchored and tagged required, tagged and anchored extra",
            "---\nclass: generated\n&o !!str owner: me\n!!str &a x_extra: 1\n---\n",
        ),
        (
            "explicit required, explicit extra",
            "---\nclass: generated\n? owner\n: me\n? x_extra\n: 1\n---\n",
        ),
    ] {
        let extra_line = if case.starts_with("explicit") { 5 } else { 4 };
        assert_eq!(
            run(text),
            [t("key-extra", extra_line, "x_extra")],
            "{case}: `owner` is written, `x_extra` is extra"
        );
    }
}
