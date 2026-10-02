//! docs/features/spec-check-process.md (ADR-0031): the project's
//! `[[check.rules]]` through the core API, over the real parser on the
//! fixtures `spec-a` (a game) and `spec-b` (a command-line tool, Russian
//! prose) and on documents written here. Each rule set is appended to the
//! fixture's own `specengine.toml`; what a rule set *adds* is the report
//! with it minus the report of the same files without it (rules only add).
//! Every config names only words of `common::rules::RULE_WORDS` (AC-14).
//!
//! Covers AC-01 (the config errors, core half), AC-02 (no rule, no rule
//! finding), AC-03…AC-10, AC-12, AC-13, AC-15 (core half), AC-18 (codes
//! emitted are listed). The CLI halves live in
//! `crates/specengine-cli/tests/check_rules.rs`.

mod common;

use common::check::{Config, TODAY, show};
use common::rules::{spec_b_rules, unlisted};
use common::{fixture, md_files, to_crlf, with_bom};
use specengine_core::check::{
    self, Baseline, CheckConfig, CheckFile, CheckInput, Finding, Report, Verdict,
};
use specengine_model::Severity;

type Files = Vec<(String, Vec<u8>)>;

/// The six codes of the task (Data, "Findings"); `key-missing` is reused.
const RULE_CODES: [&str; 6] = [
    "key-empty",
    "value-invalid",
    "part-missing",
    "part-empty",
    "text-empty",
    "parent-cycle",
];

/// A finding as `(severity, code, path, line, subject, message)`.
type Seen = (Severity, String, String, usize, String, String);

fn seen(finding: &Finding) -> Seen {
    (
        finding.severity,
        finding.code.clone(),
        finding.path.clone(),
        finding.line,
        finding.subject.clone(),
        finding.message.clone(),
    )
}

fn err(code: &str, path: &str, line: usize, subject: &str, message: &str) -> Seen {
    (
        Severity::Error,
        code.to_owned(),
        path.to_owned(),
        line,
        subject.to_owned(),
        message.to_owned(),
    )
}

fn warn(code: &str, path: &str, line: usize, subject: &str, message: &str) -> Seen {
    let mut seen = err(code, path, line, subject, message);
    seen.0 = Severity::Warning;
    seen
}

/// A fixture's `specengine.toml` and every `.md` file under it.
fn corpus(name: &str) -> (String, Files) {
    let dir = fixture(name);
    let toml = std::fs::read_to_string(dir.join("specengine.toml")).expect("specengine.toml");
    (toml, md_files(&dir))
}

/// `toml` with `rules` appended; the config must read and name only listed
/// words.
fn ruled(toml: &str, rules: &str) -> Config {
    let text = format!("{toml}\n{rules}");
    let config = Config::from_toml(&text);
    let unlisted = unlisted(&config.check);
    assert!(
        unlisted.is_empty(),
        "words missing from RULE_WORDS: {unlisted:?}"
    );
    config
}

fn input(config: &Config, files: &Files) -> CheckInput {
    CheckInput {
        files: files
            .iter()
            .map(|(path, bytes)| CheckFile::parse(path.clone(), bytes.clone(), &config.scheme))
            .collect(),
        problems: Vec::new(),
    }
}

/// The findings `rules` add to `files`: the report with them minus the
/// report without them; every finding of the latter is kept.
fn added(toml: &str, rules: &str, files: &Files) -> Vec<Seen> {
    let plain = Config::from_toml(toml);
    let with = ruled(toml, rules);
    let before = plain.run(&input(&plain, files));
    let after = with.run(&input(&with, files));
    let mut rest: Vec<Seen> = after.findings.iter().map(seen).collect();
    for finding in before.findings.iter().map(seen) {
        let at = rest
            .iter()
            .position(|kept| *kept == finding)
            .unwrap_or_else(|| panic!("the rules removed {finding:?}\n{}", show(&after)));
        rest.remove(at);
    }
    for (_, code, ..) in &rest {
        assert!(
            check::CHECK_CODES.contains(&code.as_str()),
            "AC-18: `{code}` is emitted but not listed"
        );
    }
    rest
}

/// `files` with `from` replaced by `to` in `path` (exactly one match).
fn edit(files: &Files, path: &str, from: &str, to: &str) -> Files {
    let mut out = files.clone();
    let file = out
        .iter_mut()
        .find(|(p, _)| p == path)
        .unwrap_or_else(|| panic!("{path} in the corpus"));
    let text = String::from_utf8(file.1.clone()).expect("UTF-8");
    assert_eq!(text.matches(from).count(), 1, "{from:?} once in {path}");
    file.1 = text.replacen(from, to, 1).into_bytes();
    out
}

/// `files` with `path` written as `text` (added or replaced).
fn put(files: &Files, path: &str, text: &str) -> Files {
    put_bytes(files, path, text.as_bytes().to_vec())
}

fn put_bytes(files: &Files, path: &str, bytes: Vec<u8>) -> Files {
    let mut out: Files = files.iter().filter(|(p, _)| p != path).cloned().collect();
    out.push((path.to_owned(), bytes));
    out.sort();
    out
}

fn spec_a() -> (String, Files) {
    corpus("spec-a")
}

const Q31: &str = "docs/records/Q/Q-031.md";
const Q32: &str = "docs/records/Q/Q-032.md";

// ------------------------------------------------------------------ AC-01

/// One malformed rule appended to spec-a's config: its error is at
/// `specengine.toml:<line of marker>`, its message holding `says`.
fn assert_refused(rules: &str, marker: &str, says: &str) {
    let (toml, _) = spec_a();
    let text = format!("{toml}\n{rules}");
    let line = text
        .lines()
        .position(|line| line.contains(marker))
        .unwrap_or_else(|| panic!("{marker:?} in {rules}"))
        + 1;
    let error = match CheckConfig::from_toml(&text) {
        Ok(config) => panic!("accepted:\n{rules}\n{:?}", config.rules),
        Err(error) => error,
    };
    let at = error.at("specengine.toml");
    assert!(
        at.starts_with(&format!("specengine.toml:{line}: ")),
        "{rules}\nwant line {line} ({marker:?}), got {at}"
    );
    assert!(at.contains(says), "{rules}\nwant {says:?} in {at}");
}

/// The malformed rules of "Rules and edge cases", each with the line its
/// error must name (a marker on it) and a fragment of the message. Shared
/// in spirit with the CLI test, which runs them through every mode.
fn malformed() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // An unknown key: at the key.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nflavour = 1\n",
            "flavour",
            "flavour",
        ),
        // No selector, no requirement: at the header.
        (
            "[[check.rules]]   # HEADER\nkeys = [\"to\"]\n",
            "HEADER",
            "check rule without a selector: give `kinds`, `classes` or `paths`",
        ),
        (
            "[[check.rules]]   # HEADER\nkinds = [\"question\"]\nseverity = \"warning\"\n",
            "HEADER",
            "check rule without a requirement: give `keys`, `values`, `parts` or `text = true`",
        ),
        (
            "[[check.rules]]   # HEADER\nkinds = [\"question\"]\ntext = false\n",
            "HEADER",
            "without a requirement",
        ),
        // The second entry's header, not the first's.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n\n[[check.rules]]   # HEADER\nparts = [\"Cost\"]\n",
            "HEADER",
            "without a selector",
        ),
        // Empty lists and tables: at the value.
        (
            "[[check.rules]]\nkinds = []   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = []   # HERE\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nparts = []   # HERE\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\nclasses = []   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\npaths = []   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nwhen = {}   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nvalues = {}   # HERE\n",
            "HERE",
            "is empty",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nvalues = { to = [] }   # HERE\n",
            "HERE",
            "an empty list",
        ),
        // A kind no `[ids]` prefix declares (a prefix is no kind).
        (
            "[[check.rules]]\nkinds = [\n  \"question\",\n  \"quest\",   # HERE\n]\nkeys = [\"to\"]\n",
            "HERE",
            "check rule `kinds`: `quest` is the kind of no `[ids]` prefix",
        ),
        (
            "[[check.rules]]\nkinds = [\"Q\"]   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "`Q` is the kind of no `[ids]` prefix",
        ),
        // A class outside the four.
        (
            "[[check.rules]]\nclasses = [\"record\"]   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "`record`",
        ),
        // Bad globs: absolute, `..`, empty, an empty component.
        (
            "[[check.rules]]\npaths = [\"/docs/*.md\"]   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "absolute",
        ),
        (
            "[[check.rules]]\npaths = [\"../x/*.md\"]   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "`..`",
        ),
        (
            "[[check.rules]]\npaths = [\"\"]   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "empty",
        ),
        (
            "[[check.rules]]\npaths = [\"docs//x.md\"]   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "empty component",
        ),
        // A label whose slug is empty.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nparts = [\"Cost\", \"?!\"]   # HERE\n",
            "HERE",
            "empty slug",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nparts = [\"  \"]   # HERE\n",
            "HERE",
            "empty slug",
        ),
        // A bad severity (case counts).
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nseverity = \"fatal\"   # HERE\n",
            "HERE",
            "`severity` `fatal` is not \"error\" or \"warning\"",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nseverity = \"Error\"   # HERE\n",
            "HERE",
            "`severity` `Error`",
        ),
        // `when` / `values` values neither a string nor a list of strings.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nwhen = { status = 3 }   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "a string or a list of strings, not an integer",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nwhen = { status = [\"open\", 1] }   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "a list of strings",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nvalues = { to = true }   # HERE\n",
            "HERE",
            "not a boolean",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\n[check.rules.values]\nto = { a = \"b\" }   # HERE\n",
            "HERE",
            "not a table",
        ),
        // Wrong TOML types of the rule's own keys.
        (
            "[[check.rules]]\nkinds = \"question\"   # HERE\nkeys = [\"to\"]\n",
            "HERE",
            "",
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\ntext = \"yes\"   # HERE\n",
            "HERE",
            "",
        ),
    ]
}

#[test]
fn each_malformed_rule_is_a_config_error_at_its_line() {
    for (rules, marker, says) in malformed() {
        assert_refused(rules, marker, says);
    }
}

/// Of several bad `when` or `values` entries, the error names the first
/// written, at its line, never another: the entries are written in an
/// order that is neither alphabetical (`owner` < `status` < `to`) nor its
/// reverse, and the first bad one is not the last bad one. Sub-tables,
/// dotted keys and one-line inline tables alike. M: alphabetical (the
/// `BTreeMap`'s) order; reverse order; the last bad entry.
#[test]
fn of_two_bad_when_or_values_entries_the_first_written_is_named() {
    // (rules, marker on the line named, the first written key, the
    // message for it, the keys that must not be named)
    let cases: [(&str, &str, &str, &str, &[&str]); 9] = [
        // A sub-table: `to` first, `status` alphabetically first and last.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n[check.rules.when]\nto = 1   # HERE\nowner = \"owner\"\nstatus = true\n",
            "HERE",
            "to",
            "check rule `when` `to`: a string or a list of strings, not an integer",
            &["status"],
        ),
        // `owner` first: alphabetically first too, but neither the last
        // written (`status`) nor the reverse-alphabetical first (`to`).
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n[check.rules.when]\nowner = 2   # HERE\nto = 3\nstatus = 4\n",
            "HERE",
            "owner",
            "check rule `when` `owner`: a string or a list of strings, not an integer",
            &["`to`", "status"],
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\n[check.rules.values]\nto = [\"owner\", 3]   # HERE\nowner = [\"owner\"]\nstatus = {}\n",
            "HERE",
            "to",
            "check rule `values` `to`: a list of strings, not one holding an integer",
            &["status"],
        ),
        // Dotted keys on their own lines.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nwhen.to = []   # HERE\nwhen.status = 1\n",
            "HERE",
            "to",
            "check rule `when` `to`: an empty list",
            &["status"],
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nvalues.to = 1.5   # HERE\nvalues.status = [true]\n",
            "HERE",
            "to",
            "check rule `values` `to`: a string or a list of strings, not a float",
            &["status"],
        ),
        // One line: the message names the first written.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nwhen = { to = 1, status = 2 }   # HERE\n",
            "HERE",
            "to",
            "check rule `when` `to`: a string or a list of strings, not an integer",
            &["status"],
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nvalues = { to = [1], owner = [\"owner\"], status = [2] }   # HERE\n",
            "HERE",
            "to",
            "check rule `values` `to`: a list of strings, not one holding an integer",
            &["status"],
        ),
        // An empty key name sorts first; the first written still wins,
        // whichever of the two it is.
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n[check.rules.when]\nto = {}   # HERE\n\"\" = \"open\"\n",
            "HERE",
            "to",
            "check rule `when` `to`: a string or a list of strings, not a table",
            &["empty key name"],
        ),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n[check.rules.when]\nstatus = \"open\"\n\"\" = \"open\"   # HERE\nowner = 1\n",
            "HERE",
            "",
            "check rule `when`: an empty key name",
            &["`owner`"],
        ),
    ];
    for (rules, marker, first, says, not) in cases {
        assert_refused(rules, marker, says);
        let (toml, _) = spec_a();
        let at = CheckConfig::from_toml(&format!("{toml}\n{rules}"))
            .unwrap_err()
            .at("specengine.toml");
        if !first.is_empty() {
            assert!(at.contains(&format!("`{first}`")), "{rules}\n{at}");
        }
        for other in not {
            assert!(!at.contains(other), "{rules}\nnames {other:?}: {at}");
        }
    }
    // Valid entries written out of order still read by key.
    let (toml, _) = spec_a();
    let config = ruled(
        &toml,
        "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n[check.rules.when]\nto = \"owner\"\nstatus = [\"open\", \"answered\"]\nowner = \"owner\"\n[check.rules.values]\nto = [\"team\"]\nstatus = \"open\"\n",
    );
    let rule = &config.check.rules[0];
    let keys = |table: &[(String, Vec<String>)]| -> Vec<String> {
        table.iter().map(|(key, _)| key.clone()).collect()
    };
    assert_eq!(keys(&rule.when), ["owner", "status", "to"]);
    assert_eq!(
        rule.when[1],
        (
            "status".to_owned(),
            vec!["open".to_owned(), "answered".to_owned()]
        )
    );
    assert_eq!(keys(&rule.values), ["status", "to"]);
}

#[test]
fn the_fixture_style_rules_read_as_written() {
    let (toml, _) = spec_a();
    let config = ruled(&toml, FIXTURE_RULES);
    let rules = &config.check.rules;
    assert_eq!(rules.len(), 2, "{rules:?}");
    assert_eq!(rules[0].kinds, ["question"]);
    assert_eq!(
        rules[0].values,
        [
            (
                "status".to_owned(),
                vec![
                    "open".to_owned(),
                    "answered".to_owned(),
                    "deferred".to_owned(),
                    "dropped".to_owned()
                ]
            ),
            (
                "to".to_owned(),
                vec!["customer".to_owned(), "owner".to_owned(), "team".to_owned()]
            ),
        ]
    );
    assert!(rules[0].text);
    assert_eq!(rules[0].severity, Severity::Error, "default error");
    assert_eq!(
        rules[1].when,
        [("status".to_owned(), vec!["open".to_owned()])]
    );
    assert_eq!(rules[1].keys, ["to", "working_answer"]);
    assert_eq!(rules[1].parts, ["Working answer"]);
    assert_eq!(rules[1].severity, Severity::Warning);
    let header = |n: usize| {
        let text = format!("{toml}\n{FIXTURE_RULES}");
        text.lines()
            .enumerate()
            .filter(|(_, line)| line.starts_with("[[check.rules]]"))
            .nth(n)
            .map(|(at, _)| at + 1)
            .unwrap()
    };
    assert_eq!((rules[0].line, rules[1].line), (header(0), header(1)));
}

// ------------------------------------------------------------------ AC-02

/// No rule: no rule finding in either fixture, nor in documents that would
/// break every rule of this file (no `to`, no parts, no own text).
#[test]
fn no_rule_no_rule_finding() {
    for name in ["spec-a", "spec-b"] {
        let (toml, files) = corpus(name);
        let config = Config::from_toml(&toml);
        assert!(config.check.rules.is_empty(), "{name}");
        let report = config.run(&input(&config, &files));
        let found: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| RULE_CODES.contains(&f.code.as_str()))
            .collect();
        assert!(found.is_empty(), "{name}: {found:?}");
    }
    // A document of each class, live and Tier 3, with only a heading and a
    // link (no own text, no filled part), and an empty `to`.
    let (toml, files) = spec_a();
    let mut bare = edit(
        &files,
        Q31,
        "to: customer              # customer | owner | team\n",
        "to: \"\"\n",
    );
    for (path, front) in [
        (
            "docs/features/bare.md",
            "class: spec\nstatus: shipped\nscope: [x]",
        ),
        (
            "docs/features/live.md",
            "class: spec\nstatus: draft\nscope: [x]",
        ),
        (
            "docs/records/DEC/DEC-0031.md",
            "id: DEC-0031\nclass: decision\nstatus: accepted\ndate: 2026-09-01\nscope: [x]",
        ),
        (
            "docs/records/DEC/DEC-0032.md",
            "id: DEC-0032\nclass: decision\nstatus: proposed\ndate: 2026-09-01\nscope: [x]",
        ),
        (
            "docs/records/R/R-14.md",
            "id: R-14\nclass: canon\nowner: owner\nreviewed: 2026-09-20\nto: \"\"",
        ),
        (
            "docs/records/R/R-15.md",
            "id: R-15\nclass: canon\nowner: owner\nreviewed: 2026-09-20",
        ),
    ] {
        bare = put(
            &bare,
            path,
            &format!("---\n{front}\n---\n\n# Bare\n\n## Implementation\n\n[x](archive/r.md)\n"),
        );
    }
    let bare = put(
        &bare,
        "docs/records/R/R-15.md",
        "---\nid: R-15\nclass: canon\nowner: owner\nreviewed: 2026-09-20\n---\n",
    );
    let config = Config::from_toml(&toml);
    let report = config.run(&input(&config, &bare));
    assert!(
        report
            .findings
            .iter()
            .all(|f| !RULE_CODES.contains(&f.code.as_str()) && f.code != "key-missing"),
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------------ AC-03

#[test]
fn keys_are_required_written_and_filled() {
    let (toml, files) = spec_a();
    let rules = "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\", \"working_answer\"]\n";
    assert_eq!(added(&toml, rules, &files), [], "spec-a is clean");

    let to_line = "to: customer              # customer | owner | team\n";
    let without = edit(&files, Q31, to_line, "");
    assert_eq!(
        added(&toml, rules, &without),
        [err(
            "key-missing",
            Q31,
            1,
            "to",
            "key `to` is required by a check rule"
        )]
    );
    for empty in [
        "to: \"\"\n",
        "to:\n",
        "to: ~\n",
        "to: null\n",
        "to: \"   \"\n",
        "to: []\n",
        "to: {}\n",
    ] {
        let files = edit(&files, Q31, to_line, empty);
        assert_eq!(
            added(&toml, rules, &files),
            [err(
                "key-empty",
                Q31,
                6,
                "to",
                "key `to` is empty; a check rule requires it filled"
            )],
            "{empty:?}"
        );
    }
    // Filled: a list, a map, a number.
    for filled in [
        "to: [owner]\n",
        "to: {who: owner}\n",
        "to: 0\n",
        "to: false\n",
    ] {
        let files = edit(&files, Q31, to_line, filled);
        assert_eq!(added(&toml, rules, &files), [], "{filled:?}");
    }

    // Written keys include `id`, `kind` and `title`; a CRLF+BOM file keeps
    // the key's line.
    let written = |key: &str| {
        format!("[[check.rules]]\npaths = [\"docs/records/Q/*.md\"]\nkeys = [\"{key}\"]\n")
    };
    let kind_missing = |path: &str| {
        err(
            "key-missing",
            path,
            1,
            "kind",
            "key `kind` is required by a check rule",
        )
    };
    assert_eq!(added(&toml, &written("kind"), &files), [kind_missing(Q32)]);
    assert_eq!(added(&toml, &written("owner"), &files), []);
    let crlf = edit(&files, Q31, to_line, "to: \"\"\n");
    let bytes = crlf.iter().find(|(p, _)| p == Q31).unwrap().1.clone();
    let crlf = put_bytes(&crlf, Q31, with_bom(&to_crlf(&bytes)));
    assert_eq!(
        added(&toml, rules, &crlf),
        [err(
            "key-empty",
            Q31,
            6,
            "to",
            "key `to` is empty; a check rule requires it filled"
        )],
        "CRLF+BOM"
    );

    // A-102 declares `kind: requirement` under the assumption prefix: the
    // declared kind selects. R-12 is a requirement by its prefix.
    let nope = |kind: &str| format!("[[check.rules]]\nkinds = [\"{kind}\"]\nkeys = [\"nope\"]\n");
    let missing = |path: &str| {
        err(
            "key-missing",
            path,
            1,
            "nope",
            "key `nope` is required by a check rule",
        )
    };
    assert_eq!(
        added(&toml, &nope("requirement"), &files),
        [
            missing("docs/records/A/A-102.md"),
            missing("docs/records/R/R-12.md")
        ]
    );
    assert_eq!(
        added(&toml, &nope("assumption"), &files),
        [missing("docs/records/A/A-101.md")]
    );
}

// ------------------------------------------------------------------ AC-04

#[test]
fn values_are_exact_and_absent_is_none() {
    let (toml, files) = spec_a();
    let rules = "[[check.rules]]\nkinds = [\"question\"]\nvalues = { to = [\"customer\", \"owner\", \"team\"] }\n";
    assert_eq!(added(&toml, rules, &files), [], "spec-a is clean");
    let to_line = "to: customer              # customer | owner | team\n";
    let listed = "key `to` is `{}`; allowed: customer, owner, team";
    let invalid = |shown: &str| err("value-invalid", Q31, 6, "to", &listed.replace("{}", shown));
    let not_string = err("value-invalid", Q31, 6, "to", "key `to` is not a string");
    let cases: Vec<(&str, Vec<Seen>)> = vec![
        ("to: Owner\n", vec![invalid("Owner")]),
        ("to: \"owner \"\n", vec![invalid("owner ")]),
        ("to: [owner, x]\n", vec![invalid("x")]),
        ("to: [x, owner, y]\n", vec![invalid("x"), invalid("y")]),
        ("to: [x, x]\n", vec![invalid("x")]),
        ("to: 3\n", vec![not_string.clone()]),
        ("to: true\n", vec![not_string.clone()]),
        ("to: {a: b}\n", vec![not_string.clone()]),
        ("to: [owner, 3]\n", vec![not_string.clone()]),
        ("to: \"3\"\n", vec![invalid("3")]),
        ("to: \"\"\n", vec![]),
        ("to: []\n", vec![]),
        ("", vec![]),
        ("to: team\n", vec![]),
        ("to: [team, owner]\n", vec![]),
    ];
    for (written, want) in cases {
        let files = edit(&files, Q31, to_line, written);
        let mut want = want;
        want.sort_by(|a, b| a.5.cmp(&b.5));
        let mut got = added(&toml, rules, &files);
        got.sort_by(|a, b| a.5.cmp(&b.5));
        assert_eq!(got, want, "{written:?}");
    }
}

// ------------------------------------------------------------------ AC-05

#[test]
fn when_selects_by_exact_string() {
    let (toml, files) = spec_a();
    let to31 = "to: customer              # customer | owner | team\n";
    let both = edit(&edit(&files, Q31, to31, ""), Q32, "to: owner\n", "");
    let rule = |when: &str| {
        format!("[[check.rules]]\nkinds = [\"question\"]\nwhen = {when}\nkeys = [\"to\"]\n")
    };
    let missing = |path: &str| {
        err(
            "key-missing",
            path,
            1,
            "to",
            "key `to` is required by a check rule",
        )
    };
    assert_eq!(
        added(&toml, &rule("{ status = \"open\" }"), &both),
        [missing(Q31)],
        "Q-032 is answered"
    );
    assert_eq!(
        added(&toml, &rule("{ status = [\"open\", \"answered\"] }"), &both),
        [missing(Q31), missing(Q32)]
    );
    // No prefix or case match; absent or non-string never selects.
    for when in [
        "{ status = \"ope\" }",
        "{ status = \"Open\" }",
        "{ status = \"open \" }",
        "{ status = \"1\" }",
    ] {
        assert_eq!(added(&toml, &rule(when), &both), [], "{when}");
    }
    let status = "status: open              # open | answered | deferred | dropped\n";
    for written in ["status: 1\n", "status: [open]\n", "status: true\n", ""] {
        let files = edit(&both, Q31, status, written);
        assert_eq!(
            added(&toml, &rule("{ status = [\"open\", \"1\"] }"), &files),
            [],
            "{written:?}"
        );
    }
    // Two `when` keys AND.
    assert_eq!(
        added(
            &toml,
            &rule("{ status = \"open\", owner = \"owner\" }"),
            &both
        ),
        [missing(Q31)]
    );
    assert_eq!(
        added(
            &toml,
            &rule("{ status = \"open\", owner = \"team\" }"),
            &both
        ),
        []
    );
}

// ------------------------------------------------------------------ AC-06

const SHIPPED: &str = "docs/features/probe.md";
const IMPLEMENTATION: &str = "[[check.rules]]\npaths = [\"docs/features/*.md\"]\nwhen = { status = \"shipped\" }\nparts = [\"Implementation\"]\n";

/// A shipped spec whose body is `body` (front-matter 6 lines: the body's
/// first line is line 7).
fn shipped(body: &str) -> String {
    format!("---\nclass: spec\nstatus: shipped\nscope: [x]\n---\n\n{body}")
}

fn implementation_of(body: &str, crlf: bool) -> Vec<Seen> {
    let (toml, files) = spec_a();
    let mut bytes = shipped(body).into_bytes();
    if crlf {
        bytes = to_crlf(&bytes);
    }
    added(&toml, IMPLEMENTATION, &put_bytes(&files, SHIPPED, bytes))
}

#[test]
fn a_heading_part_is_present_and_filled() {
    let empty = |line: usize| {
        vec![err(
            "part-empty",
            SHIPPED,
            line,
            "Implementation",
            "part `Implementation` is empty",
        )]
    };
    let missing = vec![err(
        "part-missing",
        SHIPPED,
        1,
        "Implementation",
        "part `Implementation` is missing",
    )];
    let cases: Vec<(&str, &str, Vec<Seen>)> = vec![
        (
            "filled",
            "# P\n\n## Implementation\n\nShipped in pass 3.\n",
            vec![],
        ),
        (
            "whitespace",
            "# P\n\n## Implementation\n\n   \n\t\n",
            empty(9),
        ),
        ("nothing at the end", "# P\n\n## Implementation\n", empty(9)),
        (
            "an HTML comment",
            "# P\n\n## Implementation\n\n<!-- filled after shipping -->\n",
            empty(9),
        ),
        (
            "an inline HTML comment",
            "# P\n\n## Implementation\n\n <!-- a --> <!-- b -->\n",
            empty(9),
        ),
        ("none", "# P\n\nText.\n", missing.clone()),
        (
            "text only under a nested heading",
            "# P\n\n## Implementation\n\n### x\n\nText under the nested heading.\n",
            vec![],
        ),
        (
            "text only after the next heading of its level",
            "# P\n\n## Implementation\n\n## Next\n\nText after.\n",
            empty(9),
        ),
        (
            "text only after a higher heading",
            "# P\n\n## Implementation\n\n# Top\n\nText after.\n",
            empty(9),
        ),
        (
            "a nested heading's own text is no content",
            "# P\n\n## Implementation\n\n### Pass 3 shipped\n\n## Next\n\nx\n",
            empty(9),
        ),
        ("level 3", "# P\n\n### Implementation\n\nDone.\n", vec![]),
        ("level 1", "# Implementation\n\nDone.\n", vec![]),
        ("setext", "# P\n\nImplementation\n---\n\nDone.\n", vec![]),
        (
            "an attribute",
            "# P\n\n## Implementation {#impl}\n\nDone.\n",
            vec![],
        ),
        (
            "closing hashes",
            "# P\n\n## Implementation ##\n\nDone.\n",
            vec![],
        ),
        (
            "case and punctuation",
            "# P\n\n## IMPLEMENTATION:\n\nDone.\n",
            vec![],
        ),
        (
            "another slug",
            "# P\n\n## Implementation notes\n\nDone.\n",
            missing.clone(),
        ),
        (
            "only in a fenced block",
            "# P\n\n```\n## Implementation\n\nDone.\n```\n",
            missing.clone(),
        ),
        (
            "only in an HTML block",
            "# P\n\n<div>\n## Implementation\nDone.\n</div>\n",
            missing.clone(),
        ),
        (
            "the first match decides",
            "# P\n\n## Implementation\n\n## Implementation\n\nDone.\n",
            empty(9),
        ),
        (
            "a code block fills",
            "# P\n\n## Implementation\n\n```\ncrates/x\n```\n",
            vec![],
        ),
        (
            "a list fills",
            "# P\n\n## Implementation\n\n- crates/x\n",
            vec![],
        ),
        ("a digit fills", "# P\n\n## Implementation\n\n3\n", vec![]),
        (
            "punctuation does not",
            "# P\n\n## Implementation\n\n- --- ...\n",
            empty(9),
        ),
    ];
    for (what, body, want) in cases {
        assert_eq!(implementation_of(body, false), want, "{what}");
        assert_eq!(implementation_of(body, true), want, "{what} (CRLF)");
    }
    // Not shipped, not under `docs/features/`: not selected.
    let (toml, files) = spec_a();
    let draft = shipped("# P\n").replace("status: shipped", "status: draft");
    assert_eq!(
        added(&toml, IMPLEMENTATION, &put(&files, SHIPPED, &draft)),
        []
    );
    assert_eq!(
        added(
            &toml,
            IMPLEMENTATION,
            &put(&files, "docs/specs/probe.md", &shipped("# P\n"))
        ),
        []
    );
}

// ------------------------------------------------------------------ AC-07

const DEC: &str = "docs/records/DEC/DEC-0023.md";
const COST: &str = "[[check.rules]]\nkinds = [\"decision\"]\nparts = [\"Cost\"]\n";

/// DEC-0023 with `extra` after its body (its line 16 is the first added).
fn decision_with(extra: &str) -> Vec<Seen> {
    decision_bytes(extra, |bytes| bytes)
}

/// [`decision_with`], DEC-0023's bytes passed through `variant`.
fn decision_bytes(extra: &str, variant: impl Fn(Vec<u8>) -> Vec<u8>) -> Vec<Seen> {
    let (toml, files) = spec_a();
    let files = edit(
        &files,
        DEC,
        "(A-101 becomes the rule).\n",
        &format!("(A-101 becomes the rule).\n{extra}"),
    );
    let bytes = files.iter().find(|(p, _)| p == DEC).unwrap().1.clone();
    added(&toml, COST, &put_bytes(&files, DEC, variant(bytes)))
}

#[test]
fn a_lead_in_part_opens_a_paragraph_list_item_or_line() {
    let (toml, files) = spec_a();
    // DEC-0007 (superseded: Tier 3) and DEC-0023 both lack it.
    let missing = |path: &str| err("part-missing", path, 1, "Cost", "part `Cost` is missing");
    assert_eq!(
        added(&toml, COST, &files),
        [missing("docs/records/DEC/DEC-0007.md"), missing(DEC)],
        "a Tier 3 decision is judged too"
    );
    let only_0007 = vec![missing("docs/records/DEC/DEC-0007.md")];
    let empty = |line: usize| {
        let mut out = only_0007.clone();
        out.push(err("part-empty", DEC, line, "Cost", "part `Cost` is empty"));
        out
    };
    let absent = {
        let mut out = only_0007.clone();
        out.push(missing(DEC));
        out
    };
    let cases: Vec<(&str, &str, Vec<Seen>)> = vec![
        ("filled", "\n**Cost.** We pay X.\n", only_0007.clone()),
        (
            "alone, then a paragraph",
            "\n**Cost.**\n\nA paragraph.\n",
            empty(17),
        ),
        ("alone at the end", "\n**Cost.**\n", empty(17)),
        ("a colon", "\n**Cost:** x\n", only_0007.clone()),
        ("underscores", "\n__Cost__ x\n", only_0007.clone()),
        ("a heading", "\n## Cost\n\nx\n", only_0007.clone()),
        ("lower case", "\n**cost** paid\n", only_0007.clone()),
        (
            "a list item",
            "\n- **Cost.** We pay X.\n",
            only_0007.clone(),
        ),
        (
            "a later line of a paragraph",
            "\nWhy: because.\n**Cost.** We pay X.\n",
            only_0007.clone(),
        ),
        (
            "content runs to the next lead-in line",
            "\n**Cost.**\n**Why.** Because.\n",
            empty(17),
        ),
        (
            "content continues on the next line",
            "\n**Cost.**\nWe pay X.\n",
            only_0007.clone(),
        ),
        (
            "in a fenced block",
            "\n```\n**Cost.** We pay X.\n```\n",
            absent.clone(),
        ),
        (
            "in an HTML block",
            "\n<div>\n**Cost.** We pay X.\n</div>\n",
            absent.clone(),
        ),
        ("mid-line", "\nx **Cost.** y\n", absent.clone()),
        ("italic", "\n*Cost.* We pay X.\n", absent.clone()),
        ("plain", "\nCost: we pay X.\n", absent.clone()),
        (
            "in a code span",
            "\n`**Cost.** We pay X.`\n",
            absent.clone(),
        ),
        (
            "in a link",
            "\n[**Cost.**](x.md) We pay X.\n",
            absent.clone(),
        ),
        ("another slug", "\n**Costs.** We pay X.\n", absent.clone()),
        (
            "a comment is no content",
            "\n**Cost.** <!-- later -->\n",
            empty(17),
        ),
    ];
    for (what, extra, want) in cases {
        assert_eq!(decision_with(extra), want, "{what}");
        assert_eq!(
            decision_bytes(extra, |b| to_crlf(&b)),
            want,
            "{what} (CRLF)"
        );
        assert_eq!(
            decision_bytes(extra, |b| with_bom(&to_crlf(&b))),
            want,
            "{what} (CRLF+BOM)"
        );
    }

    // Q-031's `**Working answer:**` opens line 2 of a paragraph.
    let rule = "[[check.rules]]\nkinds = [\"question\"]\nwhen = { status = \"open\" }\nparts = [\"Working answer\"]\n";
    assert_eq!(added(&toml, rule, &files), []);
    let emptied = edit(
        &files,
        Q31,
        "**Working answer:** only at rest; walking does not reset the delay (A-101).\n",
        "**Working answer:**\n",
    );
    assert_eq!(
        added(&toml, rule, &emptied),
        [err(
            "part-empty",
            Q31,
            15,
            "Working answer",
            "part `Working answer` is empty"
        )]
    );
}

// ------------------------------------------------------------------ AC-08

#[test]
fn a_cyrillic_label_is_found_by_its_slug() {
    let (toml, files) = corpus("spec-b");
    let rules = spec_b_rules();
    assert_eq!(added(&toml, &rules, &files), [], "spec-b is clean");
    let dry = "docs/features/dry-run.md";
    let text = String::from_utf8(files.iter().find(|(p, _)| p == dry).unwrap().1.clone()).unwrap();
    let heading = text.lines().position(|l| l.starts_with("## ")).unwrap() + 1;
    let kept: String = text
        .lines()
        .take(heading)
        .map(|l| format!("{l}\n"))
        .collect();
    let emptied = put(&files, dry, &kept);
    let label = "\u{41a}\u{440}\u{438}\u{442}\u{435}\u{440}\u{438}\u{438}";
    assert_eq!(
        added(&toml, &rules, &emptied),
        [err(
            "part-empty",
            dry,
            heading,
            label,
            &format!("part `{label}` is empty")
        )]
    );
    assert_eq!(heading, 14);
}

// ------------------------------------------------------------------ AC-09

const R13: &str = "docs/records/R/R-13.md";
const OWN: &str = "[[check.rules]]\nkinds = [\"requirement\"]\ntext = true\n";

fn record(body: &str) -> String {
    format!(
        "---\nid: R-13\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n{body}"
    )
}

#[test]
fn own_text_drops_headings_links_and_references() {
    let (toml, files) = spec_a();
    assert_eq!(
        added(&toml, OWN, &files),
        [],
        "spec-a's requirements have text"
    );
    let empty = vec![err(
        "text-empty",
        R13,
        1,
        "R-13",
        "no own text: only headings, links or references",
    )];
    let cases: Vec<(&str, &str, Vec<Seen>)> = vec![
        ("a link", "[x](archive/r.md)\n", empty.clone()),
        ("headings", "# Stamina\n\n## More\n", empty.clone()),
        ("references", "R-12, A-101\n", empty.clone()),
        (
            "references and punctuation",
            "R-12; A-101 (Q-031).\n",
            empty.clone(),
        ),
        ("a wiki link", "[[R-12]]\n", empty.clone()),
        (
            "a wiki link with text",
            "[[R-12|the regen rule]]\n",
            empty.clone(),
        ),
        ("a numbered link", "1. [x](a.md)\n", empty.clone()),
        ("an image", "![a diagram](d.png)\n", empty.clone()),
        ("an autolink", "<https://example.com/a1>\n", empty.clone()),
        ("a comment", "<!-- text later -->\n", empty.clone()),
        ("nothing", "", empty.clone()),
        (
            "a heading and a link",
            "# Title\n\nSee [R 12](r.md).\n",
            vec![],
        ),
        ("text", "Regeneration waits for R-12.\n", vec![]),
        ("emphasis", "*Waits.*\n", vec![]),
        ("a code span", "`regen_delay`\n", vec![]),
        ("a digit", "# T\n\n3\n", vec![]),
        (
            "text only in a nested section",
            "# T\n\n## Note {#A-150}\n\nSome text of its own.\n",
            empty.clone(),
        ),
        (
            "text before a nested section",
            "# T\n\nOwn words.\n\n## Note {#A-150}\n\nSome text.\n",
            vec![],
        ),
    ];
    for (what, body, want) in cases {
        let files = put(&files, R13, &record(body));
        assert_eq!(added(&toml, OWN, &files), want, "{what}");
    }
    // No ID: the subject is "".
    let doc = shipped("[x](archive/r.md)\n");
    let rule = "[[check.rules]]\npaths = [\"docs/features/*.md\"]\ntext = true\n";
    assert_eq!(
        added(&toml, rule, &put(&files, SHIPPED, &doc)),
        [err(
            "text-empty",
            SHIPPED,
            1,
            "",
            "no own text: only headings, links or references"
        )]
    );
}

/// AC-09 at scale: many unclosed `[[` (on many lines, or many on one line)
/// and long reference lists give the same answers as their short forms.
/// An unclosed `[[` is text, a closed one on its line is a wiki link
/// (`[[` … `]]` on one line, the rules module's reading: one spanning
/// lines is no link); references dropped one by one, however many share a
/// run; own text resumes after a nested section. A correctness test only:
/// no time bound (the quadratic search fit in milliseconds at this size).
#[test]
fn own_text_survives_many_unclosed_wiki_links_and_long_reference_lists() {
    let (toml, files) = spec_a();
    let empty = vec![err(
        "text-empty",
        R13,
        1,
        "R-13",
        "no own text: only headings, links or references",
    )];
    let many = |line: &str, n: usize| line.repeat(n);
    let cases: Vec<(&str, String, Vec<Seen>)> = vec![
        ("unclosed, no letter", many("[[\n", 3000), empty.clone()),
        (
            "unclosed, then a wiki link",
            many("[[ \n", 3000) + "[[R-12]]\n",
            empty.clone(),
        ),
        (
            "unclosed, then words",
            many("[[ \n", 3000) + "Real words.\n",
            vec![],
        ),
        ("unclosed with a word", many("[[ open\n", 3000), vec![]),
        (
            "many on one line, then a letter",
            many("[[ ", 3000) + "x\n",
            vec![],
        ),
        (
            "many on one line, closed at its end",
            many("[[ ", 3000) + "]]\n",
            empty.clone(),
        ),
        (
            "closed after a word on the line",
            "[[a [[R-12]]\n".to_owned(),
            empty.clone(),
        ),
        ("a close before the open", "x]] [[y\n".to_owned(), vec![]),
        (
            "a close on the next line",
            "[[\nWords of its own\n]]\n".to_owned(),
            vec![],
        ),
        (
            "unclosed lines between wiki links",
            many("[[R-12]] [[ \n[[A-101|the label]]\n", 1500),
            empty.clone(),
        ),
        (
            "a long reference list",
            many("- R-12, A-101 (Q-031).\n", 4000),
            empty.clone(),
        ),
        (
            "a long reference list, words in its middle",
            many("- R-12, A-101 (Q-031).\n", 2000)
                + "Own words.\n"
                + &many("- R-12, A-101 (Q-031).\n", 2000),
            vec![],
        ),
        (
            "references, links and wiki links mixed",
            many("R-12 [[A-101]] [x](a.md), Q-031\n", 2000),
            empty.clone(),
        ),
        (
            "many references in one run",
            many("R-12, ", 3000) + "A-101\n",
            empty.clone(),
        ),
        (
            "many references in one run, one word",
            many("R-12, ", 1500) + "ok " + &many("A-101, ", 1500) + "\n",
            vec![],
        ),
        (
            "the list, words only in a nested section",
            many("- R-12, A-101\n", 2000) + "\n## Note {#A-150}\n\nSome text of its own.\n",
            empty.clone(),
        ),
        (
            "the list, a nested section, own words after it",
            many("- R-12, A-101\n", 2000)
                + "\n## Note {#A-150}\n\nSome text, R-12.\n\n# Back\n\nTail words.\n",
            vec![],
        ),
        (
            "the list, a nested section, only references after it",
            many("- R-12, A-101\n", 2000)
                + "\n## Note {#A-150}\n\nSome text, R-12.\n\n# Back\n\n[[R-12]] A-101 [[\n",
            empty.clone(),
        ),
    ];
    for (what, body, want) in cases {
        let files = put(&files, R13, &record(&body));
        assert_eq!(added(&toml, OWN, &files), want, "{what}");
        // CRLF line ends: the same answer.
        let crlf = put_bytes(&files, R13, to_crlf(record(&body).as_bytes()));
        assert_eq!(added(&toml, OWN, &crlf), want, "{what}, CRLF");
    }
}

/// AC-09, iteration 3's cached line end in the wiki-link search: many
/// closed `[[x]]` on one line, then an unclosed `[[` — with words after it
/// (own text: clean) or none (`text-empty`) — then later lines whose
/// `[[`…`]]` are links only when closed on their own line. The line end is
/// searched again for every later line: a `]]` on the next line closes
/// nothing, a `[[y]]` there is a link. Correctness only, no time bound;
/// LF and CRLF. M: the line end kept from the first line (not searched
/// again past it); the line end never searched (the range's end).
#[test]
fn many_closed_wiki_links_then_an_unclosed_one_keep_each_line_s_end() {
    let (toml, files) = spec_a();
    let empty = vec![err(
        "text-empty",
        R13,
        1,
        "R-13",
        "no own text: only headings, links or references",
    )];
    let closed = |n: usize| "[[x]] ".repeat(n);
    let tight = |n: usize| "[[x]]".repeat(n);
    let lines = |line: String, n: usize| line.repeat(n);
    let cases: Vec<(&str, String, Vec<Seen>)> = vec![
        (
            "closed, then unclosed and words",
            closed(2000) + "[[ tail words\n",
            vec![],
        ),
        (
            "closed, then unclosed, no letter",
            closed(2000) + "[[ ...\n",
            empty.clone(),
        ),
        (
            "closed back to back, then unclosed and a letter",
            tight(2000) + "[[y\n",
            vec![],
        ),
        (
            "closed back to back, then unclosed and no letter",
            tight(2000) + "[[\n",
            empty.clone(),
        ),
        (
            "closed, unclosed, a wiki link on the next line",
            closed(2000) + "[[ \n[[y]]\n",
            empty.clone(),
        ),
        (
            "closed, unclosed, words on the next line",
            closed(2000) + "[[ \nnext words\n",
            vec![],
        ),
        (
            "closed, unclosed, closed only on the next line",
            closed(2000) + "[[ \nword]]\n",
            vec![],
        ),
        (
            "closed, unclosed and words, a wiki link on the next line",
            closed(2000) + "[[ tail\n[[y]]\n",
            vec![],
        ),
        (
            "closed, unclosed, a close and a word before a link on the next line",
            closed(2000) + "[[ \n]] z [[y]]\n",
            vec![],
        ),
        (
            "closed, unclosed, a lone close on the next line",
            closed(2000) + "[[\n]]\n",
            empty.clone(),
        ),
        (
            "closed, an unclosed then a closed on the line",
            closed(2000) + "[[ [[y]]\n",
            empty.clone(),
        ),
        (
            "closed, an unclosed then a closed and a word on the line",
            closed(2000) + "[[ [[y]] after\n",
            vec![],
        ),
        (
            "forty such lines, then links only",
            lines(closed(500) + "[[ \n", 40) + &closed(2000) + "\n",
            empty.clone(),
        ),
        (
            "forty such lines, a word after the last unclosed",
            lines(closed(500) + "[[ \n", 40) + &closed(500) + "[[ last\n",
            vec![],
        ),
        (
            "forty such lines, a word between two of them",
            lines(closed(500) + "[[ \n", 20) + "middle\n" + &lines(closed(500) + "[[ \n", 20),
            vec![],
        ),
        (
            "closed, unclosed at the very end, a word",
            closed(2000) + "[[ end",
            vec![],
        ),
        (
            "closed, unclosed at the very end, no letter",
            closed(2000) + "[[",
            empty.clone(),
        ),
        (
            "closed, unclosed with a Cyrillic word, a link next",
            closed(2000) + "[[ \u{441}\u{43b}\u{43e}\u{432}\u{43e}\n[[y]]\n",
            vec![],
        ),
        (
            "closed, unclosed with a dash, a link next",
            closed(2000) + "[[ — …\n[[y]]\n",
            empty.clone(),
        ),
        (
            "closed with multibyte inside, unclosed, a link next",
            "[[\u{43a}\u{43b}\u{44e}\u{447}]] ".repeat(2000) + "[[ \n[[y]]\n",
            empty.clone(),
        ),
        (
            "own text split by a nested section, links only around it",
            closed(2000)
                + "[[ \n\n## Note {#A-150}\n\nSome text.\n\n# Back\n\n"
                + &closed(2000)
                + "[[\n[[y]]\n",
            empty.clone(),
        ),
        (
            "own text split by a nested section, a word after it",
            closed(2000)
                + "[[ \n\n## Note {#A-150}\n\nSome text.\n\n# Back\n\n"
                + &closed(2000)
                + "[[ back\n[[y]]\n",
            vec![],
        ),
    ];
    for (what, body, want) in cases {
        let files = put(&files, R13, &record(&body));
        assert_eq!(added(&toml, OWN, &files), want, "{what}");
        let crlf = put_bytes(&files, R13, to_crlf(record(&body).as_bytes()));
        assert_eq!(added(&toml, OWN, &crlf), want, "{what}, CRLF");
    }
}

// ------------------------------------------------------------------ AC-10

#[test]
fn failed_front_matter_and_generated_files_are_never_judged() {
    let (toml, files) = corpus("spec-b");
    let missing = |path: &str| {
        err(
            "key-missing",
            path,
            1,
            "nope",
            "key `nope` is required by a check rule",
        )
    };
    // QN-08's front-matter fails; QN-07 is judged.
    let questions =
        "[[check.rules]]\npaths = [\"docs/records/QN/*.md\"]\nkeys = [\"nope\"]\ntext = true\n";
    assert_eq!(
        added(&toml, questions, &files),
        [missing("docs/records/QN/QN-07.md")]
    );
    // GLS-task-branch is generated; GLS-worktree is judged.
    let terms =
        "[[check.rules]]\npaths = [\"docs/records/GLS/*.md\"]\nkeys = [\"nope\"]\ntext = true\n";
    assert_eq!(
        added(&toml, terms, &files),
        [missing("docs/records/GLS/GLS-worktree.md")]
    );
    let generated = "[[check.rules]]\nclasses = [\"generated\"]\nkeys = [\"nope\"]\ntext = true\n";
    assert_eq!(added(&toml, generated, &files), []);
    // A shipped spec (Tier 3) is judged.
    let shipped_b = edit(
        &files,
        "docs/features/dry-run.md",
        "status: draft",
        "status: shipped",
    );
    let features = "[[check.rules]]\nclasses = [\"spec\"]\nkeys = [\"nope\"]\n";
    assert_eq!(
        added(&toml, features, &shipped_b),
        [missing("docs/features/dry-run.md")]
    );
}

#[test]
fn empty_bytes_judge_no_part_and_no_text() {
    let (toml, files) = spec_a();
    let rules = "[[check.rules]]\nkinds = [\"question\"]\nparts = [\"Cost\"]\ntext = true\nkeys = [\"to\", \"nope\"]\n";
    let empty_q32 = edit(&files, Q32, "No: only sprinting drains stamina.\n", "");
    let config = ruled(&toml, rules);
    let mut fed = input(&config, &empty_q32);
    // Bytes kept: every question misses the part, Q-032 has no own text.
    let full = config.run(&fed);
    let codes_of = |report: &Report| -> Vec<(String, String)> {
        report
            .findings
            .iter()
            .filter(|f| f.path.starts_with("docs/records/Q/"))
            .map(|f| (f.path.clone(), f.code.clone()))
            .collect()
    };
    let full_codes = codes_of(&full);
    assert!(
        full_codes.iter().any(|(_, code)| code == "part-missing")
            && full_codes.iter().any(|(_, code)| code == "text-empty"),
        "{}",
        show(&full)
    );
    for file in &mut fed.files {
        file.bytes.clear();
    }
    let report = config.run(&fed);
    let found = codes_of(&report);
    assert!(
        found
            .iter()
            .all(|(_, code)| !code.starts_with("part-") && code != "text-empty"),
        "{}",
        show(&report)
    );
    // Keys are still read (from the parse): `nope` is missing, `to` is not.
    assert_eq!(
        found,
        [
            (Q31.to_owned(), "key-missing".to_owned()),
            (Q32.to_owned(), "key-missing".to_owned())
        ],
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------------ AC-12

#[test]
fn debt_matches_code_path_and_subject_whatever_the_line() {
    let (toml, files) = spec_a();
    let config = ruled(
        &toml,
        &format!("[check]\nmode = \"enforce\"\n\n{IMPLEMENTATION}"),
    );
    let debt = |code: &str, expires: &str| {
        Baseline::from_toml(&format!(
            "[[debt]]\ncode = \"{code}\"\npath = \"{SHIPPED}\"\nsubject = \"Implementation\"\nreason = \"later\"\nexpires = \"{expires}\"\n"
        ))
        .expect("a baseline")
    };
    let state = |report: &Report| {
        let found: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.code.starts_with("part-"))
            .collect();
        assert_eq!(found.len(), 1, "{}", show(report));
        (
            found[0].code.clone(),
            found[0].line,
            found[0].is_live_debt(),
            report.blocks(found[0]),
        )
    };
    // part-missing at line 1.
    let missing = put(&files, SHIPPED, &shipped("# P\n\nText.\n"));
    let live = config.run_with(
        &input(&config, &missing),
        &debt("part-missing", "2026-12-31"),
        TODAY,
    );
    assert_eq!(state(&live), ("part-missing".to_owned(), 1, true, false));
    assert_eq!((live.counts.debt, live.counts.expired), (1, 0));
    let expired = config.run_with(
        &input(&config, &missing),
        &debt("part-missing", "2026-12-31"),
        "2027-01-01",
    );
    assert_eq!(state(&expired), ("part-missing".to_owned(), 1, false, true));
    assert_eq!(expired.counts.expired, 1);
    assert_eq!(expired.verdict, Verdict::Blocked);
    // part-empty: the heading moves down three lines, the entry still holds.
    for (pad, line) in [("", 9), ("Intro.\n\nMore.\n", 12)] {
        let files = put(
            &files,
            SHIPPED,
            &shipped(&format!("# P\n\n{pad}## Implementation\n")),
        );
        let report = config.run_with(
            &input(&config, &files),
            &debt("part-empty", "2026-12-31"),
            TODAY,
        );
        assert_eq!(
            state(&report),
            ("part-empty".to_owned(), line, true, false),
            "{pad:?}"
        );
        // Another code at the same path and subject is no match.
        let other = config.run_with(
            &input(&config, &files),
            &debt("part-missing", "2026-12-31"),
            TODAY,
        );
        assert_eq!(state(&other), ("part-empty".to_owned(), line, false, true));
        assert_eq!(other.counts.stale, 1, "{}", show(&other));
    }
}

// ------------------------------------------------------------------ AC-13

const FIXTURE_RULES: &str = "\
[[check.rules]]                     # every question
kinds  = [\"question\"]
values = { status = [\"open\", \"answered\", \"deferred\", \"dropped\"], to = [\"customer\", \"owner\", \"team\"] }
text   = true
[[check.rules]]                     # an open question
kinds    = [\"question\"]
when     = { status = \"open\" }
keys     = [\"to\", \"working_answer\"]
parts    = [\"Working answer\"]       # a `**Working answer:**` lead-in or a heading
severity = \"warning\"                # default \"error\"
";

/// Four rules, each finding something in the files of
/// [`ordering_files`].
const ORDER_RULES: [&str; 4] = [
    "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\", \"working_answer\"]\nseverity = \"warning\"\n",
    "[[check.rules]]\nkinds = [\"question\"]\nvalues = { to = [\"customer\", \"owner\", \"team\"] }\n",
    "[[check.rules]]\nkinds = [\"decision\"]\nparts = [\"Cost\"]\ntext = true\n",
    "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nparts = [\"Working answer\"]\n",
];

fn ordering_files() -> Files {
    let (_, files) = spec_a();
    let files = edit(
        &files,
        Q31,
        "to: customer              # customer | owner | team\n",
        "",
    );
    let files = edit(&files, Q32, "to: owner\n", "to: [Owner, x]\n");
    edit(
        &files,
        "docs/records/DEC/DEC-0007.md",
        "Replaced by DEC-0023.\n",
        "",
    )
}

fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for rest in permutations(n - 1) {
        for at in 0..=rest.len() {
            let mut one = rest.clone();
            one.insert(at, n - 1);
            out.push(one);
        }
    }
    out
}

#[test]
fn shuffled_files_and_permuted_rules_give_byte_identical_output() {
    let (toml, _) = spec_a();
    let files = ordering_files();
    let mut first: Option<(Vec<String>, String)> = None;
    let mut runs = 0;
    for order in permutations(ORDER_RULES.len()) {
        let rules: String = order
            .iter()
            .map(|&i| ORDER_RULES[i])
            .collect::<Vec<_>>()
            .join("\n");
        let config = ruled(&toml, &rules);
        let forward = input(&config, &files);
        let mut shuffled = vec![forward.clone()];
        let mut reversed = forward.clone();
        reversed.files.reverse();
        shuffled.push(reversed);
        for shift in [3, 7] {
            let mut rotated = forward.clone();
            rotated.files.rotate_left(shift);
            shuffled.push(rotated);
        }
        let mut interleaved = forward.clone();
        let (even, odd): (Vec<_>, Vec<_>) = interleaved
            .files
            .drain(..)
            .enumerate()
            .partition(|(i, _)| i % 2 == 0);
        interleaved.files = odd
            .into_iter()
            .chain(even.into_iter().rev())
            .map(|(_, f)| f)
            .collect();
        shuffled.push(interleaved);
        for fed in shuffled {
            let report = config.run(&fed);
            let out = (report.lines(true), report.to_json());
            runs += 1;
            match &first {
                None => first = Some(out),
                Some(want) => assert_eq!(&out, want, "rule order {order:?}"),
            }
        }
    }
    assert_eq!(runs, 24 * 5);
    let (lines, _) = first.unwrap();
    for code in ["key-missing", "value-invalid", "part-missing", "text-empty"] {
        assert!(
            lines
                .iter()
                .any(|line| line.contains(&format!(": {code}: "))),
            "{code} in\n{}",
            lines.join("\n")
        );
    }
}

#[test]
fn one_finding_from_two_rules_is_once_an_error_over_a_warning() {
    let (toml, files) = spec_a();
    let files = edit(
        &files,
        Q31,
        "to: customer              # customer | owner | team\n",
        "",
    );
    let warning =
        "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nseverity = \"warning\"\n";
    let error = "[[check.rules]]\npaths = [\"docs/records/Q/*.md\"]\nkeys = [\"to\"]\n";
    let one = |severity: Severity| {
        let mut want = err(
            "key-missing",
            Q31,
            1,
            "to",
            "key `to` is required by a check rule",
        );
        want.0 = severity;
        vec![want]
    };
    assert_eq!(
        added(&toml, &format!("{warning}\n{error}"), &files),
        one(Severity::Error)
    );
    assert_eq!(
        added(&toml, &format!("{error}\n{warning}"), &files),
        one(Severity::Error)
    );
    assert_eq!(
        added(&toml, &format!("{warning}\n{warning}"), &files),
        one(Severity::Warning)
    );
    assert_eq!(
        added(&toml, &format!("{error}\n{error}"), &files),
        one(Severity::Error)
    );
}

#[test]
fn a_warning_rule_counts_and_never_blocks() {
    let (toml, files) = spec_a();
    let files = edit(
        &files,
        Q31,
        "to: customer              # customer | owner | team\n",
        "",
    );
    let rule = |severity: &str| {
        format!(
            "[check]\nmode = \"enforce\"\n\n[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nseverity = \"{severity}\"\n"
        )
    };
    let plain = Config::from_toml(&format!("{toml}\n[check]\nmode = \"enforce\"\n"));
    let base = plain.run(&input(&plain, &files));
    let warned = ruled(&toml, &rule("warning"));
    let report = warned.run(&input(&warned, &files));
    assert_eq!(report.counts.warnings, base.counts.warnings + 1);
    assert_eq!(report.counts.errors, base.counts.errors);
    let found = report
        .findings
        .iter()
        .find(|f| f.path == Q31 && f.code == "key-missing")
        .unwrap();
    assert!(!report.blocks(found));
    assert!(
        !report.lines(false).iter().any(|line| line.contains(Q31)),
        "only with detail"
    );
    assert!(report.lines(true).iter().any(|line| line.contains(Q31)));
    let errored = ruled(&toml, &rule("error"));
    let report = errored.run(&input(&errored, &files));
    assert_eq!(report.counts.errors, base.counts.errors + 1);
    let found = report
        .findings
        .iter()
        .find(|f| f.path == Q31 && f.code == "key-missing")
        .unwrap();
    assert!(report.blocks(found));
    assert!(report.lines(false).iter().any(|line| line.contains(Q31)));
}

// ------------------------------------------------------------------ AC-15

#[test]
fn a_parent_cycle_is_one_warning_at_its_first_member() {
    let (toml, files) = spec_a();
    let config = Config::from_toml(&toml);
    let game = "docs/spec/game.md";
    let cyclic = edit(&files, game, "tier: 0\n", "tier: 0\nparent: MEC-STAMINA\n");
    let report = config.run(&input(&config, &cyclic));
    let subject = "DOM-GAME, DOM-MOVEMENT, MEC-STAMINA";
    let cycles: Vec<Seen> = report
        .findings
        .iter()
        .filter(|f| f.code == "parent-cycle")
        .map(seen)
        .collect();
    assert_eq!(
        cycles,
        [warn(
            "parent-cycle",
            game,
            5,
            subject,
            &format!("`parent:` forms a cycle through {subject}")
        )]
    );
    // The members are those `SpecGraph::cycles()` gives (the tree's).
    let plain = config.run(&input(&config, &files));
    assert_eq!(report.verdict, plain.verdict, "never blocks");
    assert_eq!(report.counts.errors, plain.counts.errors);
    assert_eq!(report.counts.warnings, plain.counts.warnings + 1);

    // With rules or none: the same one.
    let with = ruled(&toml, OWN);
    let ruled_report = with.run(&input(&with, &cyclic));
    assert_eq!(
        ruled_report
            .findings
            .iter()
            .filter(|f| f.code == "parent-cycle")
            .count(),
        1
    );

    // A self-parent (spec-b's MOD-CLI) is one.
    let (toml_b, files_b) = corpus("spec-b");
    let config_b = Config::from_toml(&toml_b);
    let cli = "docs/spec/cli.md";
    let selfish = edit(&files_b, cli, "tier: 1\n", "tier: 1\nparent: MOD-CLI\n");
    let report = config_b.run(&input(&config_b, &selfish));
    let cycles: Vec<Seen> = report
        .findings
        .iter()
        .filter(|f| f.code == "parent-cycle")
        .map(seen)
        .collect();
    assert_eq!(
        cycles,
        [warn(
            "parent-cycle",
            cli,
            5,
            "MOD-CLI",
            "`parent:` forms a cycle through MOD-CLI"
        )]
    );
    // The fixtures as they are: none.
    for name in ["spec-a", "spec-b"] {
        let (toml, files) = corpus(name);
        let config = Config::from_toml(&toml);
        let report = config.run(&input(&config, &files));
        assert!(
            report.findings.iter().all(|f| f.code != "parent-cycle"),
            "{name}"
        );
    }
}

#[test]
fn a_cycle_of_tier_3_members_only_is_no_finding() {
    let (toml, files) = spec_a();
    let config = Config::from_toml(&toml);
    // A shipped spec parented on itself: Tier 3, not live.
    let doc = "---\nid: MEC-OLD\nclass: spec\nstatus: shipped\nscope: [x]\nparent: MEC-OLD\n---\n\n# Old\n\nGone.\n";
    let report = config.run(&input(&config, &put(&files, "docs/features/old.md", doc)));
    assert!(
        report.findings.iter().all(|f| f.code != "parent-cycle"),
        "{}",
        show(&report)
    );
    // The same live: one.
    let live = doc.replace("status: shipped", "status: draft");
    let report = config.run(&input(&config, &put(&files, "docs/features/old.md", &live)));
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.code == "parent-cycle")
            .count(),
        1,
        "{}",
        show(&report)
    );
}

/// A small deterministic generator (xorshift64*), for the random corpora.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % n.max(1)
    }
}

/// The `[ids]` of the random corpora: documents `N`, sections `S`.
const CYCLE_IDS: &str =
    "[ids]\nN = { kind = \"domain\", width = 4 }\nS = { kind = \"rule\", width = 4 }\n";

/// A random corpus of `count` documents under `docs/spec/`, paths
/// scrambled against IDs: parents on documents, on sections (bare and
/// `N-…#S-…`), on the document itself or its own section, dangling,
/// `other:`-qualified, on an ID two documents hold, or none; rings of up
/// to seven documents; documents without an ID; Tier 3 and generated
/// members; `parent:` at varying lines.
fn cycle_corpus(seed: u64, count: usize) -> Vec<(String, String)> {
    let mut rng = Rng::new(seed);
    // Plan: each document's ID and its sections (level, ID).
    let mut ids: Vec<Option<String>> = Vec::with_capacity(count);
    let mut sections: Vec<Vec<(usize, String)>> = Vec::with_capacity(count);
    let mut section_count = 0;
    for i in 0..count {
        let id = match rng.below(100) {
            0..=9 => None,
            10..=15 if i > 0 => ids[rng.below(i)].clone(),
            _ => Some(format!("N-{i:04}")),
        };
        ids.push(id);
        let mut own = Vec::new();
        for _ in 0..rng.below(3) {
            own.push((2 + rng.below(2), format!("S-{section_count:04}")));
            section_count += 1;
        }
        sections.push(own);
    }
    // Rings of 2 to 7 documents for a third of the groups: long cycles.
    let mut ring: Vec<Option<usize>> = vec![None; count];
    let mut start = 0;
    while start < count {
        let size = (2 + rng.below(6)).min(count - start);
        if rng.below(3) == 0 {
            for j in 0..size {
                ring[start + j] = Some(start + (j + 1) % size);
            }
        }
        start += size;
    }
    let all_sections: Vec<String> = sections
        .iter()
        .flatten()
        .map(|(_, id)| id.clone())
        .collect();
    let mut files = Vec::with_capacity(count);
    for i in 0..count {
        let doc_id = |rng: &mut Rng| format!("N-{:04}", rng.below(count));
        let ringed = ring[i].and_then(|next| match (&ids[next], sections[next].last()) {
            (Some(id), _) if rng.below(4) != 0 => Some(id.clone()),
            (_, Some((_, section))) => Some(section.clone()),
            (id, None) => id.clone(),
        });
        let parent = match rng.below(100) {
            _ if ringed.is_some() => ringed,
            0..=7 => None,
            8..=54 => Some(doc_id(&mut rng)),
            55..=69 if !all_sections.is_empty() => {
                Some(all_sections[rng.below(all_sections.len())].clone())
            }
            70..=74 => {
                let target = rng.below(count);
                match (&ids[target], sections[target].first()) {
                    (Some(id), Some((_, section))) => Some(format!("{id}#{section}")),
                    _ => Some(doc_id(&mut rng)),
                }
            }
            75..=79 => Some(format!("N-9{:03}", rng.below(1000))),
            80..=84 => Some(format!("other:{}", doc_id(&mut rng))),
            85..=89 => match (sections[i].first(), &ids[i]) {
                (Some((_, section)), _) if rng.below(2) == 0 => Some(section.clone()),
                (_, Some(id)) => Some(id.clone()),
                _ => Some(doc_id(&mut rng)),
            },
            _ => Some(format!("N-{:04}", rng.below(8))),
        };
        let mut text = String::from("---\n");
        if let Some(id) = &ids[i] {
            text.push_str(&format!("id: {id}\n"));
        }
        match rng.below(20) {
            0..=3 => text.push_str("class: spec\nstatus: shipped\n"),
            4..=6 => text.push_str("class: generated\n"),
            _ => text.push_str("class: canon\n"),
        }
        if rng.below(2) == 0 {
            text.push_str("owner: owner\nreviewed: 2026-09-20\n");
        }
        if let Some(parent) = parent {
            text.push_str(&format!("parent: {parent}\n"));
        }
        text.push_str(&format!("---\n\n# Title {i}\n\nWords of {i}.\n"));
        for (level, id) in &sections[i] {
            text.push_str(&format!(
                "\n{} Part {id} {{#{id}}}\n\nMore words.\n",
                "#".repeat(*level)
            ));
        }
        let path = format!(
            "docs/spec/p{:06}.md",
            (i * 7919 + seed as usize * 13) % 100_003
        );
        files.push((path, text));
    }
    files
}

/// AC-15 against the read commands' tree: on random corpora with many
/// cycles (section parents, nested sections, self-parents, dangling and
/// `other:` parents, duplicate holders, documents without an ID, Tier 3
/// and generated members), the check's `parent-cycle` findings are exactly
/// one per cycle of `SpecGraph::new(..).cycles()` (what `spec tree` breaks
/// and warns about) holding a live member: at its first member's
/// `parent:` line, subject the members' names sorted by name then path —
/// whatever the order the files come in. The coverage of each shape is
/// counted, so the corpora cannot drift into exercising none. M: the
/// check's parents-only tree skipping section parents, or taking another
/// holder of a duplicated ID.
#[test]
fn parent_cycles_are_the_read_graph_s_cycles_on_random_corpora() {
    use specengine_core::check::{SpecGraph, Standing};
    use std::collections::BTreeMap;

    let config = Config::from_toml(CYCLE_IDS);
    let (mut cycles, mut reported, mut with_section, mut not_live, mut by_path, mut long) =
        (0, 0, 0, 0, 0, 0);
    let (mut held_twice, mut root_not_live) = (0, 0);
    for seed in 1..=60_u64 {
        let files = cycle_corpus(seed, 150);
        let borrowed: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let input = config.input(&borrowed);
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let mut holders: BTreeMap<String, usize> = BTreeMap::new();
        for file in 0..graph.paths().len() {
            if let Some(document) = graph.document(file) {
                *holders.entry(graph.name(document)).or_default() += 1;
            }
        }
        let mut want: Vec<Seen> = Vec::new();
        for members in graph.cycles() {
            cycles += 1;
            held_twice += usize::from(
                members
                    .iter()
                    .any(|&member| holders.get(&graph.name(member)).is_some_and(|&n| n > 1)),
            );
            long += usize::from(members.len() >= 3);
            with_section += usize::from(members.iter().any(|member| member.ord > 0));
            if !members
                .iter()
                .any(|member| graph.standing(member.file) == Standing::Live)
            {
                not_live += 1;
                continue;
            }
            let mut named: Vec<(String, &str)> = members
                .iter()
                .map(|&member| (graph.name(member), graph.paths()[member.file]))
                .collect();
            by_path += usize::from(named.iter().any(|(name, path)| name == path));
            root_not_live += usize::from(graph.standing(members[0].file) != Standing::Live);
            named.sort();
            let subject = named
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let path = graph.paths()[members[0].file];
            let text = &files.iter().find(|(p, _)| p == path).unwrap().1;
            let line = text
                .lines()
                .position(|line| line.starts_with("parent: "))
                .expect("a cycle's first member has a parent")
                + 1;
            want.push(warn(
                "parent-cycle",
                path,
                line,
                &subject,
                &format!("`parent:` forms a cycle through {subject}"),
            ));
        }
        want.sort();
        reported += want.len();
        let found = |input: &CheckInput| {
            let mut found: Vec<Seen> = config
                .run(input)
                .findings
                .iter()
                .filter(|finding| finding.code == "parent-cycle")
                .map(seen)
                .collect();
            found.sort();
            found
        };
        assert_eq!(found(&input), want, "seed {seed}");
        // The files in another order: the same findings.
        let mut shuffled = borrowed.clone();
        let mut rng = Rng::new(seed + 1000);
        for at in (1..shuffled.len()).rev() {
            shuffled.swap(at, rng.below(at + 1));
        }
        assert_eq!(
            found(&config.input(&shuffled)),
            want,
            "seed {seed}, shuffled"
        );
    }
    // Every shape is met, many times.
    let counts = format!(
        "cycles {cycles}, reported {reported}, with a section {with_section}, no live member {not_live}, a member named by path {by_path}, three or more members {long}, a member's ID held twice {held_twice}, the first member not live {root_not_live}"
    );
    assert!(cycles >= 500 && reported >= 400, "{counts}");
    for shape in [
        with_section,
        not_live,
        by_path,
        long,
        held_twice,
        root_not_live,
    ] {
        assert!(shape >= 50, "{counts}");
    }
    println!("{counts}");
}
