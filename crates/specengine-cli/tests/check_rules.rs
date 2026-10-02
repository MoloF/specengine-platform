//! docs/features/spec-check-process.md (ADR-0031), the CLI half: the
//! project's `[[check.rules]]` through the `spec` binary on scratch copies
//! of `fixtures/spec-a`, `-b`, of this repository's documents, and on a
//! small project written here; `--staged` and `--changed` in scratch git
//! repositories. Every spawned `spec` gets a scratch `HOME`; this
//! repository is only read. The keys and labels of these configs are
//! listed in `crates/specengine-core/tests/common/rules.rs` (AC-14).
//!
//! AC-01 (config errors in every mode; read commands accept valid rules),
//! AC-08 (spec-b's Cyrillic label), AC-11 (severity, the base judged with
//! the rules), AC-15 (`parent-cycle` against `spec tree`'s warning),
//! AC-16 (this repository's shipping rules).

mod common;

use common::check::FAR;
use common::graph::{repository_copy, spec30};
use common::staged::Repo;
use common::{Run, Scratch, read_text, replace, spec, write};
use serde_json::Value;

/// A small project: questions `Q`, decisions `D`, its documents under
/// `docs/`; `MODE` is replaced by the check mode.
const MINI: &str = "\
[project]
slug = \"rules-mini\"

[paths]
roots = [\"docs\"]

[ids]
Q = { kind = \"question\", width = 3 }
D = { kind = \"decision\", width = 3 }

[check]
mode = \"MODE\"
";

/// Every question needs a filled `to`.
const TO_RULE: &str = "\n[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n";

fn mini(mode: &str) -> String {
    MINI.replace("MODE", mode)
}

/// A question, `to: owner` unless `to` is `None`.
fn question(number: &str, to: Option<&str>) -> String {
    let to = to.map(|to| format!("to: {to}\n")).unwrap_or_default();
    format!(
        "---\nid: Q-{number}\nclass: canon\nstatus: open\n{to}owner: owner\nreviewed: 2026-09-20\n---\n\n# Question {number}\n\n**Working answer:** yes, {number}.\n"
    )
}

/// A scratch repository holding the small project in `mode`, two clean
/// questions, committed.
fn mini_repo(label: &str, mode: &str) -> Repo {
    let repo = Repo::empty(label);
    write(&repo.top, "specengine.toml", mini(mode));
    write(&repo.top, "docs/Q-001.md", question("001", Some("owner")));
    write(&repo.top, "docs/Q-002.md", question("002", Some("team")));
    repo.add_all();
    repo.git.commit(&repo.top, "clean");
    repo
}

fn summary(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or_default()
}

fn labelled<'a>(stdout: &'a str, label: &str) -> Vec<&'a str> {
    stdout
        .lines()
        .filter(|line| line.starts_with(&format!("{label}  ")))
        .collect()
}

/// `(code, path:line, subject, severity)` of every finding of the JSON.
fn findings(json: &Value) -> Vec<(String, String, String, String)> {
    json["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| {
            (
                f["code"].as_str().unwrap_or_default().to_owned(),
                format!("{}:{}", f["path"].as_str().unwrap_or_default(), f["line"]),
                f["subject"].as_str().unwrap_or_default().to_owned(),
                f["severity"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn with_code(json: &Value, code: &str) -> Vec<(String, String, String, String)> {
    findings(json).into_iter().filter(|f| f.0 == code).collect()
}

// ------------------------------------------------------------------ AC-01

/// Malformed rules: (rules, a marker on the line the error names, a
/// fragment of the message).
const MALFORMED: [(&str, &str, &str); 12] = [
    (
        "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nflavour = 1   # HERE\n",
        "HERE",
        "unknown field `flavour`",
    ),
    (
        "[[check.rules]]   # HERE\nkeys = [\"to\"]\n",
        "HERE",
        "check rule without a selector: give `kinds`, `classes` or `paths`",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\n\n[[check.rules]]   # HERE\nkinds = [\"question\"]\n",
        "HERE",
        "check rule without a requirement: give `keys`, `values`, `parts` or `text = true`",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nparts = []   # HERE\n",
        "HERE",
        "check rule `parts` is empty",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nwhen = {}   # HERE\nkeys = [\"to\"]\n",
        "HERE",
        "check rule `when` is empty",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\", \"requirement\"]   # HERE\nkeys = [\"to\"]\n",
        "HERE",
        "check rule `kinds`: `requirement` is the kind of no `[ids]` prefix",
    ),
    (
        "[[check.rules]]\nclasses = [\"canon\", \"record\"]   # HERE\nkeys = [\"to\"]\n",
        "HERE",
        "`record` is none of canon | decision | spec | generated",
    ),
    (
        "[[check.rules]]\npaths = [\"docs/../x.md\"]   # HERE\nkeys = [\"to\"]\n",
        "HERE",
        "check rule `paths`: ",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nparts = [\"**\"]   # HERE\n",
        "HERE",
        "has an empty slug",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nseverity = \"high\"   # HERE\n",
        "HERE",
        "check rule `severity` `high` is not \"error\" or \"warning\"",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nwhen = { status = 3 }   # HERE\nkeys = [\"to\"]\n",
        "HERE",
        "check rule `when` `status`: a string or a list of strings, not an integer",
    ),
    (
        "[[check.rules]]\nkinds = [\"question\"]\nvalues = { to = [\"owner\", true] }   # HERE\n",
        "HERE",
        "check rule `values` `to`: a list of strings",
    ),
];

/// The command forms of AC-01: plain, `--json`, `--debt`, `--staged`,
/// `--changed`.
const FORMS: [&[&str]; 7] = [
    &["check"],
    &["--json", "check"],
    &["check", "--debt"],
    &["check", "--staged"],
    &["--json", "check", "--staged"],
    &["check", "--changed"],
    &["--json", "check", "--changed"],
];

/// AC-01: each malformed rule is cannot-check, exit 2, one cause
/// `specengine.toml:<line>: message` at the value (else the entry's
/// header), in every command form and every `[check] mode`; the config is
/// on disk and staged. M: the declared-kind check dropped; a selector-less
/// rule accepted.
#[test]
fn a_malformed_rule_is_cannot_check_at_its_line_in_every_mode() {
    for mode in ["observe", "enforce-introduced", "enforce"] {
        let repo = mini_repo(&format!("rules-bad-{mode}"), mode);
        for (rules, marker, says) in MALFORMED {
            let config = format!("{}\n{rules}", mini(mode));
            let line = config
                .lines()
                .position(|l| l.contains(marker))
                .expect("the marker")
                + 1;
            write(&repo.top, "specengine.toml", &config);
            repo.add_all();
            for form in FORMS {
                let run = repo.spec(form);
                let context = format!("{mode} {form:?} {rules}\n{}", run.show());
                assert_eq!(run.code, 2, "{context}");
                if form.contains(&"--json") {
                    let json = run.json();
                    assert_eq!(json["verdict"], "cannot-check", "{context}");
                    let causes = json["cannot_check"].as_array().expect("causes");
                    assert_eq!(causes.len(), 1, "{context}");
                    assert_eq!(
                        causes[0]["path"],
                        format!("specengine.toml:{line}"),
                        "{context}"
                    );
                    assert!(
                        causes[0]["message"].as_str().unwrap().contains(says),
                        "{context}"
                    );
                } else {
                    let causes = labelled(&run.stdout, "cannot");
                    assert_eq!(causes.len(), 1, "{context}");
                    assert!(
                        causes[0].starts_with(&format!("cannot  specengine.toml:{line}: ")),
                        "{context}"
                    );
                    assert!(causes[0].contains(says), "{context}");
                    assert!(
                        summary(&run.stdout).ends_with(" — cannot-check"),
                        "{context}"
                    );
                }
            }
        }
        // The valid config back: the same forms answer.
        write(
            &repo.top,
            "specengine.toml",
            format!("{}{TO_RULE}", mini(mode)),
        );
        repo.add_all();
        for form in FORMS {
            repo.spec(form).code(0);
        }
    }
}

/// docs/canon/spec-check-cli.md "Cannot check" (iteration 3): a config
/// error is reported in mode `enforce`, the mode being read only when all
/// of `CheckConfig` is valid. `[budgets] tier0_bytes = 0` (no rule) and
/// each malformed rule beside `[check] mode = "observe"` or
/// `"enforce-introduced"`: exit 2, the summary `spec check [enforce]: … —
/// cannot-check`, the JSON `"mode": "enforce"`, in every command form; the
/// valid config back prints its own mode again. M: the label read from
/// `[check] mode` alone (iteration 2).
#[test]
fn a_config_error_is_reported_in_mode_enforce_whatever_mode_is_written() {
    let budget = "[budgets]\ntier0_bytes = 0\n";
    for mode in ["observe", "enforce-introduced"] {
        let repo = mini_repo(&format!("rules-mode-{mode}"), mode);
        let bad = std::iter::once(budget).chain(MALFORMED.iter().map(|(rules, _, _)| *rules));
        for part in bad {
            write(
                &repo.top,
                "specengine.toml",
                format!("{}\n{part}", mini(mode)),
            );
            repo.add_all();
            for form in FORMS {
                let run = repo.spec(form);
                let context = format!("{mode} {form:?} {part}\n{}", run.show());
                assert_eq!(run.code, 2, "{context}");
                if form.contains(&"--json") {
                    let json = run.json();
                    assert_eq!(json["verdict"], "cannot-check", "{context}");
                    assert_eq!(json["mode"], "enforce", "{context}");
                    assert!(run.stdout.contains("\"mode\":\"enforce\""), "{context}");
                } else {
                    let line = summary(&run.stdout);
                    assert!(line.starts_with("spec check [enforce]: "), "{context}");
                    assert!(line.ends_with(" — cannot-check"), "{context}");
                    assert_eq!(labelled(&run.stdout, "cannot").len(), 1, "{context}");
                }
            }
        }
        // The valid config back: its own mode, exit 0.
        write(
            &repo.top,
            "specengine.toml",
            format!("{}\n[budgets]\ntier0_bytes = 1\n{TO_RULE}", mini(mode)),
        );
        repo.add_all();
        let plain = repo.spec(&["check"]);
        let staged = repo.spec(&["check", "--staged"]);
        let json = repo.spec(&["--json", "check", "--changed"]);
        let context = format!("{mode}\n{}{}{}", plain.show(), staged.show(), json.show());
        assert_eq!((plain.code, staged.code, json.code), (0, 0, 0), "{context}");
        let (plain_label, base_label) = match mode {
            "observe" => ("[observe]", "[observe]"),
            _ => ("[enforce]", "[enforce-introduced]"),
        };
        assert!(
            summary(&plain.stdout).starts_with(&format!("spec check {plain_label}: ")),
            "{context}"
        );
        assert!(
            summary(&staged.stdout).starts_with(&format!("spec check {base_label}: ")),
            "{context}"
        );
        assert_eq!(json.json()["mode"], mode, "{context}");
    }
}

/// AC-01, AC-16's last sentence, AC-08: the fixture-style rules on spec-a
/// and spec-b's Cyrillic part rule add no finding — `spec check` prints
/// the same bytes with and without them, in text, detail and JSON — and
/// `show`, `tree`, `bundle`, `search`, `graph` answer, exit 0, with the
/// same bytes as without rules (read commands do not judge).
#[test]
fn valid_rules_judge_nothing_here_and_the_read_commands_answer() {
    let spec_a_rules = "\
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
    let spec_b_rules = read_text(&common::fixture("spec-b"), "check-rules.toml");
    let reads: [(&str, Vec<&[&str]>); 2] = [
        (
            "spec-a",
            vec![
                &["show", "Q-031"],
                &["--json", "show", "Q-031"],
                &["tree"],
                &["--json", "tree"],
                &["bundle", "Q-031"],
                &["search", "stamina"],
                &["graph", "MEC-STAMINA"],
            ],
        ),
        (
            "spec-b",
            vec![
                &["show", "QN-07"],
                &["tree"],
                &["bundle", "QN-07"],
                &["graph", "MOD-CLI"],
            ],
        ),
    ];
    for (fixture, commands) in reads {
        let rules = if fixture == "spec-a" {
            spec_a_rules.to_owned()
        } else {
            spec_b_rules.clone()
        };
        let scratch = Scratch::new("rules-read");
        let plain = scratch.copy(fixture, "plain");
        let ruled = scratch.copy(fixture, "ruled");
        let config = read_text(&ruled, "specengine.toml");
        write(&ruled, "specengine.toml", format!("{config}\n{rules}"));
        let home_plain = scratch.home("plain");
        let home_ruled = scratch.home("ruled");
        for args in [&["check"][..], &["check", "--debt"], &["--json", "check"]] {
            let a = spec(&home_plain, &plain, args);
            let b = spec(&home_ruled, &ruled, args);
            assert_eq!(a.code, b.code, "{fixture} {args:?}\n{}", b.show());
            assert_eq!(a.stdout, b.stdout, "{fixture} {args:?}");
            assert_eq!(a.stderr, b.stderr, "{fixture} {args:?}");
        }
        for args in commands {
            let a = spec30(&home_plain, &plain, args);
            let b = spec30(&home_ruled, &ruled, args);
            b.code(0);
            assert_eq!(a.code, 0, "{fixture} {args:?}\n{}", a.show());
            assert_eq!(a.stdout, b.stdout, "{fixture} {args:?}");
        }
    }
}

/// AC-08: spec-b's criteria section emptied in a copy → one `part-empty`
/// at its heading, the label as written.
#[test]
fn spec_b_s_cyrillic_part_emptied_is_one_part_empty() {
    let scratch = Scratch::new("rules-b");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");
    let rules = read_text(&root, "check-rules.toml");
    let config = read_text(&root, "specengine.toml");
    write(&root, "specengine.toml", format!("{config}\n{rules}"));
    let dry = "docs/features/dry-run.md";
    let text = read_text(&root, dry);
    let kept: String = text.lines().take(14).map(|l| format!("{l}\n")).collect();
    write(&root, dry, &kept);
    let json = spec(&home, &root, &["--json", "check"]).json();
    let label = "\u{41a}\u{440}\u{438}\u{442}\u{435}\u{440}\u{438}\u{438}";
    assert_eq!(
        with_code(&json, "part-empty"),
        [(
            "part-empty".to_owned(),
            format!("{dry}:14"),
            label.to_owned(),
            "error".to_owned()
        )],
        "{json}"
    );
}

// ------------------------------------------------------------------ AC-11

/// AC-11: a `severity = "warning"` rule never blocks, counts in
/// `warnings`, prints only with `--debt`; the default blocks under
/// `enforce`. M: `severity` ignored.
#[test]
fn a_warning_rule_never_blocks_and_prints_only_in_detail() {
    let scratch = Scratch::new("rules-sev");
    let home = scratch.home("h");
    let root = scratch.dir("mini");
    write(&root, "docs/Q-001.md", question("001", Some("owner")));
    write(&root, "docs/Q-002.md", question("002", None));
    for (severity, code, warnings, errors) in [("warning", 0, 1, 0), ("error", 1, 0, 1)] {
        let rule = format!("{TO_RULE}severity = \"{severity}\"\n");
        write(
            &root,
            "specengine.toml",
            format!("{}{rule}", mini("enforce")),
        );
        let plain = spec(&home, &root, &["check"]);
        plain.code(code);
        let finding = "docs/Q-002.md:1: key-missing: key `to` is required by a check rule";
        assert_eq!(
            plain.stdout.contains(finding),
            severity == "error",
            "{}",
            plain.show()
        );
        assert!(
            summary(&plain.stdout).contains(&format!(", {errors} errors, {warnings} warnings, ")),
            "{}",
            plain.show()
        );
        let detail = spec(&home, &root, &["check", "--debt"]);
        detail.code(code);
        assert!(
            detail.stdout.contains(&format!("{severity}  {finding}")),
            "{}",
            detail.show()
        );
        let json = spec(&home, &root, &["--json", "check"]).json();
        assert_eq!(json["counts"]["warnings"], warnings);
        assert_eq!(json["counts"]["errors"], errors);
        assert_eq!(
            with_code(&json, "key-missing"),
            [(
                "key-missing".to_owned(),
                "docs/Q-002.md:1".to_owned(),
                "to".to_owned(),
                severity.to_owned()
            )]
        );
    }
    // Under `observe` the error rule's finding is printed, nothing blocks.
    write(
        &root,
        "specengine.toml",
        format!("{}{TO_RULE}", mini("observe")),
    );
    spec(&home, &root, &["check"]).code(0);
}

/// AC-11 in git, `enforce-introduced`: a commit adding only a rule passes
/// (`--staged` and `--changed`), the violation `HEAD` already holds shown
/// `(pre-existing)`; a new violation blocks; under `enforce` the old one
/// blocks too. M: the base judged without rules.
#[test]
fn the_base_is_judged_with_the_rules() {
    let repo = Repo::empty("rules-base");
    write(&repo.top, "specengine.toml", mini("enforce-introduced"));
    write(&repo.top, "docs/Q-001.md", question("001", Some("owner")));
    write(&repo.top, "docs/Q-002.md", question("002", None));
    repo.add_all();
    repo.git.commit(&repo.top, "a question without `to`");

    // Only the rule added.
    write(
        &repo.top,
        "specengine.toml",
        format!("{}{TO_RULE}", mini("enforce-introduced")),
    );
    let old =
        "error  docs/Q-002.md:1: key-missing: key `to` is required by a check rule (pre-existing)";
    for (form, label) in [
        (&["--changed"][..], "changed"),
        (&["--staged"][..], "staged"),
    ] {
        if label == "staged" {
            repo.add_all();
        }
        let mut args = vec!["check"];
        args.extend_from_slice(form);
        let run = repo.spec(&args);
        run.code(0);
        assert!(
            !run.stdout.contains("Q-002"),
            "{label}: not blocking\n{}",
            run.show()
        );
        assert!(
            summary(&run.stdout).ends_with(", 1 errors, 0 warnings, 0 debt, 0 expired, 0 stale, 0 introduced, 0 new debt, worst W 260 B — observed"),
            "{label}\n{}",
            run.show()
        );
        let mut detail = args.clone();
        detail.push("--debt");
        let run = repo.spec(&detail);
        run.code(0);
        assert!(run.stdout.contains(old), "{label}\n{}", run.show());
        let mut json_args = vec!["--json"];
        json_args.extend_from_slice(&args);
        let json = repo.spec(&json_args).json();
        let found: Vec<&Value> = json["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["code"] == "key-missing")
            .collect();
        assert_eq!(found.len(), 1, "{json}");
        assert_eq!(found[0]["introduced"], false, "{json}");
    }
    // Plain `check` (no base) blocks on it.
    repo.spec(&["check"]).code(1);
    repo.git.commit(&repo.top, "the rule");

    // A new violation blocks; the old stays pre-existing.
    write(&repo.top, "docs/Q-003.md", question("003", None));
    let run = repo.spec(&["check", "--changed", "--debt"]);
    run.code(1);
    assert!(run.stdout.contains(old), "{}", run.show());
    assert!(
        run.stdout
            .lines()
            .any(|l| l
                == "error  docs/Q-003.md:1: key-missing: key `to` is required by a check rule"),
        "{}",
        run.show()
    );
    repo.add_all();
    repo.spec(&["check", "--staged"]).code(1);

    // Under `enforce` the violation `HEAD` holds blocks as well.
    let strict = Repo::empty("rules-base-enforce");
    write(&strict.top, "specengine.toml", mini("enforce"));
    write(&strict.top, "docs/Q-001.md", question("001", Some("owner")));
    write(&strict.top, "docs/Q-002.md", question("002", None));
    strict.add_all();
    strict.git.commit(&strict.top, "a question without `to`");
    write(
        &strict.top,
        "specengine.toml",
        format!("{}{TO_RULE}", mini("enforce")),
    );
    strict.spec(&["check", "--changed"]).code(1);
    strict.add_all();
    strict.spec(&["check", "--staged"]).code(1);
}

/// AC-12 through the CLI: a baseline entry `(key-missing, path, to)` makes
/// the rule's finding debt until it expires.
#[test]
fn a_rule_finding_goes_into_debt() {
    let scratch = Scratch::new("rules-debt");
    let home = scratch.home("h");
    let root = scratch.dir("mini");
    write(
        &root,
        "specengine.toml",
        format!("{}{TO_RULE}", mini("enforce")),
    );
    write(&root, "docs/Q-001.md", question("001", Some("owner")));
    write(&root, "docs/Q-002.md", question("002", None));
    let entry = |expires: &str| {
        format!(
            "[[debt]]\ncode = \"key-missing\"\npath = \"docs/Q-002.md\"\nsubject = \"to\"\nreason = \"later\"\nexpires = \"{expires}\"\n"
        )
    };
    write(&root, ".spec-debt.toml", entry(FAR));
    let run = spec(&home, &root, &["check"]);
    run.code(0);
    assert!(
        summary(&run.stdout).contains(", 1 debt, 0 expired, "),
        "{}",
        run.show()
    );
    write(&root, ".spec-debt.toml", entry("2000-01-01"));
    let run = spec(&home, &root, &["check"]);
    run.code(1);
    assert!(
        summary(&run.stdout).contains(", 0 debt, 1 expired, "),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------ AC-15

/// The members of `spec tree`'s cycle warning: `… through A, B; …`.
fn tree_members(warning: &str) -> String {
    let after = warning
        .split(" through ")
        .nth(1)
        .unwrap_or_else(|| panic!("a cycle warning: {warning}"));
    after.split(';').next().unwrap().to_owned()
}

/// AC-15: spec-a with `DOM-GAME` parented on `MEC-STAMINA`: one
/// `parent-cycle` warning at `docs/spec/game.md`'s `parent:` line, its
/// subject the members of `spec tree`'s warning on the same copy; nothing
/// else changes and the verdict stays. spec-b's self-parent: one. The
/// fixtures and this repository: none. M: severity error; another member
/// placed.
#[test]
fn a_parent_cycle_is_one_warning_naming_the_tree_s_members() {
    let scratch = Scratch::new("rules-cycle");
    let home = scratch.home("h");
    for (fixture, path, edit, line, subject) in [
        (
            "spec-a",
            "docs/spec/game.md",
            "parent: MEC-STAMINA",
            5,
            "DOM-GAME, DOM-MOVEMENT, MEC-STAMINA",
        ),
        (
            "spec-b",
            "docs/spec/cli.md",
            "parent: MOD-CLI",
            5,
            "MOD-CLI",
        ),
    ] {
        let root = scratch.copy(fixture, fixture);
        let before = spec30(&home, &root, &["--json", "check"]);
        assert!(
            with_code(&before.json(), "parent-cycle").is_empty(),
            "{fixture}"
        );
        let text = read_text(&root, path);
        let tier = text
            .lines()
            .find(|l| l.starts_with("tier: "))
            .expect("a tier line")
            .to_owned();
        replace(
            &root,
            path,
            &format!("{tier}\n"),
            &format!("{tier}\n{edit}\n"),
        );
        let after = spec30(&home, &root, &["--json", "check"]);
        let json = after.json();
        assert_eq!(
            with_code(&json, "parent-cycle"),
            [(
                "parent-cycle".to_owned(),
                format!("{path}:{line}"),
                subject.to_owned(),
                "warning".to_owned()
            )],
            "{fixture}: {json}"
        );
        assert_eq!(after.code, before.code, "{fixture}: never blocks");
        assert_eq!(json["verdict"], before.json()["verdict"]);
        assert_eq!(
            json["counts"]["warnings"],
            before.json()["counts"]["warnings"].as_u64().unwrap() + 1
        );
        let tree = spec30(&home, &root, &["tree"]);
        tree.code(0);
        let warnings = tree.stderr_lines();
        assert_eq!(warnings.len(), 1, "{fixture}\n{}", tree.show());
        assert_eq!(tree_members(warnings[0]), subject, "{fixture}");
        let text = spec30(&home, &root, &["check", "--debt"]);
        assert!(
            text.stdout.lines().any(|l| l
                == format!(
                    "warning  {path}:{line}: parent-cycle: `parent:` forms a cycle through {subject}"
                )),
            "{fixture}\n{}",
            text.show()
        );
        let plain = spec30(&home, &root, &["check"]);
        assert!(!plain.stdout.contains("parent-cycle"), "only in detail");
    }
    let repository = repository_copy(&scratch, "repository");
    let json = spec30(&home, &repository, &["--json", "check"]).json();
    assert!(
        findings(&json).iter().all(|f| f.0 != "parent-cycle"),
        "{json}"
    );
}

/// One document of the planned cycle corpus.
struct Planned {
    path: String,
    id: Option<String>,
    /// `canon` (live), `tier3` (a shipped spec) or `generated`.
    standing: &'static str,
    parent: Option<String>,
    /// `(heading level, section ID)`, in order.
    sections: Vec<(usize, String)>,
}

impl Planned {
    fn text(&self) -> String {
        let mut text = String::from("---\n");
        if let Some(id) = &self.id {
            text.push_str(&format!("id: {id}\n"));
        }
        text.push_str(match self.standing {
            "tier3" => "class: spec\nstatus: shipped\n",
            "generated" => "class: generated\n",
            _ => "class: canon\nowner: owner\n",
        });
        if let Some(parent) = &self.parent {
            text.push_str(&format!("parent: {parent}\n"));
        }
        text.push_str("---\n\n# A node\n\nSome words.\n");
        for (level, id) in &self.sections {
            text.push_str(&format!(
                "\n{} Part {id} {{#{id}}}\n\nMore words.\n",
                "#".repeat(*level)
            ));
        }
        text
    }

    fn parent_line(&self) -> usize {
        self.text()
            .lines()
            .position(|line| line.starts_with("parent: "))
            .expect("a parent")
            + 1
    }
}

/// The planned corpus: many `parent:` cycles of every shape, and the
/// cycles themselves as `(document number, section ordinal)` members
/// (0: the document, n: its n-th section).
struct CycleCorpus {
    docs: Vec<Planned>,
    cycles: Vec<Vec<(usize, usize)>>,
}

impl CycleCorpus {
    fn add(&mut self, standing: &'static str, sections: usize) -> usize {
        let n = self.docs.len();
        self.docs.push(Planned {
            // Paths scrambled against the chain order.
            path: format!("docs/spec/q{:06}.md", (n * 7919 + 17) % 100_003),
            id: Some(format!("N-{n:04}")),
            standing,
            parent: None,
            sections: (0..sections).map(|k| (2, format!("S-{n:03}{k}"))).collect(),
        });
        n
    }

    fn id(&self, n: usize) -> String {
        self.docs[n].id.clone().expect("an ID")
    }

    fn section(&self, n: usize, k: usize) -> String {
        self.docs[n].sections[k - 1].1.clone()
    }

    fn name(&self, (n, k): (usize, usize)) -> String {
        if k > 0 {
            return self.section(n, k);
        }
        self.docs[n]
            .id
            .clone()
            .unwrap_or_else(|| self.docs[n].path.clone())
    }

    fn build() -> Self {
        let mut c = Self {
            docs: Vec::new(),
            cycles: Vec::new(),
        };
        // Rings of 1 to 7 documents; live, Tier 3 only, generated only,
        // or mixed with one live member; a tail into every fourth.
        for ring in 0..42 {
            let len = 1 + ring % 7;
            let members: Vec<usize> = (0..len)
                .map(|k| {
                    let standing = match (ring % 6, k) {
                        (3, _) => "tier3",
                        (4, _) => "generated",
                        (5, k) if k + 1 == len => "canon",
                        (5, k) if k % 2 == 0 => "generated",
                        (5, _) => "tier3",
                        _ => "canon",
                    };
                    c.add(standing, 0)
                })
                .collect();
            for (k, &n) in members.iter().enumerate() {
                c.docs[n].parent = Some(c.id(members[(k + 1) % len]));
            }
            c.cycles.push(members.iter().map(|&n| (n, 0)).collect());
            if ring % 4 == 0 {
                let tail = c.add("canon", 0);
                c.docs[tail].parent = Some(c.id(members[0]));
            }
        }
        // A parent on another document's section: A, its section, B.
        let (a, b) = (c.add("canon", 1), c.add("canon", 0));
        c.docs[b].parent = Some(c.section(a, 1));
        c.docs[a].parent = Some(c.id(b));
        c.cycles.push(vec![(a, 0), (a, 1), (b, 0)]);
        // A document parented on its own section.
        let a = c.add("canon", 1);
        c.docs[a].parent = Some(c.section(a, 1));
        c.cycles.push(vec![(a, 0), (a, 1)]);
        // A nested section: both sections are members.
        let (a, b) = (c.add("canon", 2), c.add("canon", 0));
        c.docs[a].sections[1].0 = 3;
        c.docs[b].parent = Some(c.section(a, 2));
        c.docs[a].parent = Some(c.id(b));
        c.cycles.push(vec![(a, 0), (a, 1), (a, 2), (b, 0)]);
        // The `ID#SECTION` form.
        let (a, b) = (c.add("canon", 1), c.add("canon", 0));
        c.docs[b].parent = Some(format!("{}#{}", c.id(a), c.section(a, 1)));
        c.docs[a].parent = Some(c.id(b));
        c.cycles.push(vec![(a, 0), (a, 1), (b, 0)]);
        // A document without an ID, named by its path.
        let (z, b) = (c.add("canon", 1), c.add("canon", 0));
        c.docs[z].id = None;
        c.docs[b].parent = Some(c.section(z, 1));
        c.docs[z].parent = Some(c.id(b));
        c.cycles.push(vec![(z, 0), (z, 1), (b, 0)]);
        // A sibling section outside the cycle.
        let (a, b) = (c.add("canon", 2), c.add("canon", 0));
        c.docs[b].parent = Some(c.section(a, 2));
        c.docs[a].parent = Some(c.id(b));
        c.cycles.push(vec![(a, 0), (a, 2), (b, 0)]);
        // A generated document's section, a live member.
        let (g, b) = (c.add("generated", 1), c.add("canon", 0));
        c.docs[b].parent = Some(c.section(g, 1));
        c.docs[g].parent = Some(c.id(b));
        c.cycles.push(vec![(g, 0), (g, 1), (b, 0)]);
        // Dangling and `other:` parents: roots, no cycle.
        let d = c.add("canon", 0);
        c.docs[d].parent = Some("N-9999".to_owned());
        let p = c.add("canon", 0);
        c.docs[p].parent = Some(format!("other:{}", c.id(0)));
        let q = c.add("canon", 0);
        c.docs[q].parent = Some(format!("other:{}", c.id(q)));
        // An ID two documents hold: the parent is the first in path order;
        // a cycle through it, the second a tail.
        let (h1, h2, x) = (c.add("canon", 0), c.add("canon", 0), c.add("canon", 0));
        c.docs[h1].path = "docs/spec/a-dup.md".to_owned();
        c.docs[h2].path = "docs/spec/z-dup.md".to_owned();
        c.docs[h2].id = c.docs[h1].id.clone();
        c.docs[x].parent = c.docs[h1].id.clone();
        c.docs[h1].parent = Some(c.id(x));
        c.docs[h2].parent = Some(c.id(x));
        c.cycles.push(vec![(h1, 0), (x, 0)]);
        // The second holder parented on its child: no cycle.
        let (e1, e2, y) = (c.add("canon", 0), c.add("canon", 0), c.add("canon", 0));
        c.docs[e1].path = "docs/spec/a-e.md".to_owned();
        c.docs[e2].path = "docs/spec/z-e.md".to_owned();
        c.docs[e2].id = c.docs[e1].id.clone();
        c.docs[y].parent = c.docs[e1].id.clone();
        c.docs[e2].parent = Some(c.id(y));
        c
    }

    /// A cycle's members in (path, position) order.
    fn ordered(&self, cycle: &[(usize, usize)]) -> Vec<(usize, usize)> {
        let mut members = cycle.to_vec();
        members.sort_by(|a, b| (&self.docs[a.0].path, a.1).cmp(&(&self.docs[b.0].path, b.1)));
        members
    }

    fn live(&self, cycle: &[(usize, usize)]) -> bool {
        cycle.iter().any(|&(n, _)| self.docs[n].standing == "canon")
    }

    /// The check's `(path:line, subject)` per cycle with a live member.
    fn check_findings(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .cycles
            .iter()
            .filter(|cycle| self.live(cycle))
            .map(|cycle| {
                let root = self.ordered(cycle)[0].0;
                let mut named: Vec<(String, String)> = cycle
                    .iter()
                    .map(|&member| (self.name(member), self.docs[member.0].path.clone()))
                    .collect();
                named.sort();
                let subject: Vec<String> = named.into_iter().map(|(name, _)| name).collect();
                (
                    format!("{}:{}", self.docs[root].path, self.docs[root].parent_line()),
                    subject.join(", "),
                )
            })
            .collect();
        out.sort();
        out
    }

    /// `spec tree --archive`'s cycle warnings: every cycle whose root (its
    /// first member) the tree admits, i.e. is not generated.
    fn tree_warnings(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .cycles
            .iter()
            .map(|cycle| self.ordered(cycle))
            .filter(|members| self.docs[members[0].0].standing != "generated")
            .map(|members| {
                let names: Vec<String> = members.iter().map(|&m| self.name(m)).collect();
                format!(
                    "warning: `parent:` forms a cycle through {}; `{}` is listed as a root",
                    names.join(", "),
                    names[0]
                )
            })
            .collect();
        out.sort();
        out
    }
}

/// AC-15 at scale, against `spec tree` on the same copy: a project of ~200
/// documents holding 50 `parent:` cycles — rings of one to seven
/// documents (live, Tier 3 only, generated only, mixed), cycles through
/// another document's section, the document's own section, a nested
/// section, the `ID#SECTION` form, a document without an ID, a generated
/// document's section; dangling and `other:` parents; an ID two documents
/// hold. The check's `parent-cycle` warnings are exactly the planned
/// cycles with a live member, at the first member's `parent:` line,
/// subject sorted by name; `spec tree --archive` warns about exactly the
/// planned cycles whose first member it lists (not generated), members in
/// (path, position) order; and every check warning whose first member the
/// tree lists names the members and the root of one tree warning. M: the
/// check's parents-only tree skipping section parents, or taking another
/// holder of a duplicated ID.
#[test]
fn many_parent_cycles_match_the_tree_s_warnings() {
    let scratch = Scratch::new("rules-cycles-many");
    let home = scratch.home("h");
    let root = scratch.dir("project");
    let corpus = CycleCorpus::build();
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"cycles-many\"\n\n[paths]\nroots = [\"docs\"]\n\n[ids]\nN = { kind = \"domain\", width = 4 }\nS = { kind = \"rule\", width = 4 }\n",
    );
    for doc in &corpus.docs {
        write(&root, &doc.path, doc.text());
    }
    let check = spec30(&home, &root, &["--json", "check"]);
    let json = check.json();
    let found = with_code(&json, "parent-cycle");
    assert!(found.iter().all(|f| f.3 == "warning"), "{found:?}");
    let mut found: Vec<(String, String)> = found.into_iter().map(|f| (f.1, f.2)).collect();
    found.sort();
    let want = corpus.check_findings();
    assert_eq!(
        want.len(),
        36,
        "28 rings with a live member, 8 section and holder cycles"
    );
    assert_eq!(found, want, "the check's cycles");

    let tree = spec30(&home, &root, &["tree", "--archive"]);
    tree.code(0);
    let mut warned: Vec<String> = tree
        .stderr_lines()
        .into_iter()
        .filter(|line| line.contains("forms a cycle through"))
        .map(str::to_owned)
        .collect();
    warned.sort();
    assert_eq!(warned, corpus.tree_warnings(), "the tree's cycles");
    // Holder warnings are the tree's only other ones.
    assert!(
        tree.stderr_lines()
            .iter()
            .all(|line| line.contains("forms a cycle through") || line.contains(" holders; ")),
        "{}",
        tree.show()
    );

    // Check against tree directly: same root, same members.
    let path_of: std::collections::BTreeMap<String, String> = corpus
        .docs
        .iter()
        .map(|doc| {
            let name = doc.id.clone().unwrap_or_else(|| doc.path.clone());
            (name, doc.path.clone())
        })
        .filter(|(_, path)| !path.ends_with("z-dup.md") && !path.ends_with("z-e.md"))
        .collect();
    let tree_cycles: Vec<(String, Vec<String>)> = warned
        .iter()
        .map(|line| {
            let (_, rest) = line.split_once(" through ").expect("members");
            let (members, root) = rest.split_once("; `").expect("a root");
            let root = root.trim_end_matches("` is listed as a root");
            let mut members: Vec<String> = members.split(", ").map(str::to_owned).collect();
            members.sort();
            (path_of[root].clone(), members)
        })
        .collect();
    let generated: Vec<&str> = corpus
        .docs
        .iter()
        .filter(|doc| doc.standing == "generated")
        .map(|doc| doc.path.as_str())
        .collect();
    let mut placed_in_tree = 0;
    for (at, subject) in &found {
        let path = at.rsplit_once(':').unwrap().0;
        if generated.contains(&path) {
            continue;
        }
        let mut members: Vec<String> = subject.split(", ").map(str::to_owned).collect();
        members.sort();
        assert!(
            tree_cycles.contains(&(path.to_owned(), members.clone())),
            "{at} {subject}: no tree warning with that root and those members\n{}",
            tree.show()
        );
        placed_in_tree += 1;
    }
    assert!(placed_in_tree >= 30, "{placed_in_tree}");
}

// ------------------------------------------------------------------ AC-16

/// The root rules of the spec's Data, "This repository at shipping".
const ROOT_RULES: &str = "
[[check.rules]]                     # decisions
kinds = [\"decision\"]
parts = [\"Cost\"]
text  = true

[[check.rules]]                     # shipped features
paths = [\"docs/features/*.md\"]
when  = { status = \"shipped\" }
parts = [\"Implementation\"]
";

/// `export index && check --json` in `root`; the check's run.
fn export_and_check(home: &std::path::Path, root: &std::path::Path) -> Run {
    spec30(home, root, &["export", "index"]).code(0);
    spec30(home, root, &["--json", "check"])
}

/// AC-16: this repository's documents with the root rules on: clean, 0
/// errors, warnings, debt; ADR-0030 without its `**Cost.**` → one
/// `part-missing`; a shipped spec's Implementation emptied → one
/// `part-empty` at its heading. M: the loader dropping rules.
#[test]
fn this_repository_is_clean_under_its_shipping_rules() {
    let scratch = Scratch::new("rules-repo");
    let home = scratch.home("h");
    let with_rules = |dir: &str| {
        let root = repository_copy(&scratch, dir);
        let config = read_text(&root, "specengine.toml");
        write(&root, "specengine.toml", format!("{config}{ROOT_RULES}"));
        root
    };

    let root = with_rules("clean");
    let run = export_and_check(&home, &root);
    run.code(0);
    let json = run.json();
    assert_eq!(json["verdict"], "clean", "{json}");
    for count in ["errors", "warnings", "debt", "expired", "stale"] {
        assert_eq!(json["counts"][count], 0, "{count}: {json}");
    }
    assert!(json["counts"]["documents"].as_u64().unwrap() > 50);

    let root = with_rules("cost");
    let adr = "docs/decisions/ADR-0030.md";
    replace(&root, adr, "**Cost.** ", "");
    let json = export_and_check(&home, &root).json();
    assert_eq!(
        findings(&json),
        [(
            "part-missing".to_owned(),
            format!("{adr}:1"),
            "Cost".to_owned(),
            "error".to_owned()
        )],
        "{json}"
    );

    let root = with_rules("implementation");
    let spec_path = "docs/features/index-compaction.md";
    let text = read_text(&root, spec_path);
    let heading = text
        .lines()
        .position(|l| l == "## Implementation")
        .expect("the heading")
        + 1;
    let kept: String = text
        .lines()
        .take(heading)
        .map(|l| format!("{l}\n"))
        .collect();
    write(&root, spec_path, &kept);
    let json = export_and_check(&home, &root).json();
    assert_eq!(
        findings(&json),
        [(
            "part-empty".to_owned(),
            format!("{spec_path}:{heading}"),
            "Implementation".to_owned(),
            "error".to_owned()
        )],
        "{json}"
    );
}
