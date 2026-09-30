//! AC-02, AC-03, AC-04, AC-05 and AC-08 of docs/features/spec-cli-check.md:
//! `spec check [--baseline F] [--debt]` on copies of spec-a and spec-b,
//! each with its own `HOME`, compared with "the library" (`check_worktree`
//! on the same copy, `today_utc()`): stdout is its `lines(--debt)`, one per
//! line, or `to_json()` and a line end; the exit is its verdict's. The
//! baseline is the copy's `.spec-debt.toml` unless `--baseline` (as typed,
//! relative to the current directory) replaces it; every failure after
//! discovery is a `cannot` cause of the printed report, the config's at
//! `<config as typed>:<line>`.

#![cfg(unix)]

mod common;

use std::path::Path;

use common::check::{
    FAR, PAST, baseline_covering, blocking, dangling_document, json, library, library_with,
    line_of, quoted, text, with_project_key, without_project,
};
use common::{FIXTURES, Scratch, read_text, spec, write};
use specengine_core::check::Verdict;

/// The summary line: the last of stdout.
fn summary(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or_default()
}

/// AC-02: stdout = the library's `lines(false)`, exit = its `exit_code()`,
/// through blocked, clean (a baseline covering every blocking finding), one
/// dangling front-matter reference, and `observe`; `--debt` =
/// `lines(true)`.
#[test]
fn stdout_and_exit_are_the_library_s_for_every_verdict() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-verdicts");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");

        let blocked = library(&root);
        assert_eq!(blocked.verdict, Verdict::Blocked, "{fixture} as copied");
        for (args, detail) in [(&["check"][..], false), (&["check", "--debt"][..], true)] {
            let run = spec(&home, &root, args);
            assert_eq!(
                run.code,
                i32::from(blocked.exit_code()),
                "{fixture} {args:?}\n{}",
                run.show()
            );
            assert_eq!(run.stdout, text(&blocked, detail), "{fixture} {args:?}");
            assert_eq!(run.stderr, "", "{fixture} {args:?}");
        }
        assert!(summary(&spec(&home, &root, &["check"]).stdout).ends_with(" — blocked"));

        // A baseline covering every blocking finding: clean.
        write(&root, ".spec-debt.toml", baseline_covering(&blocked, FAR));
        let clean = library(&root);
        assert_eq!(clean.verdict, Verdict::Clean, "{fixture} covered");
        let run = spec(&home, &root, &["check"]);
        run.code(0);
        assert_eq!(run.stdout, text(&clean, false), "{fixture} covered");
        assert!(summary(&run.stdout).ends_with(" — clean"), "{}", run.stdout);
        let run = spec(&home, &root, &["check", "--debt"]);
        run.code(0);
        assert_eq!(run.stdout, text(&clean, true), "{fixture} covered --debt");

        // Then one front-matter reference to an undefined ID.
        write(
            &root,
            "docs/spec/zz-dangling.md",
            dangling_document(fixture),
        );
        let dangling = library(&root);
        let run = spec(&home, &root, &["check"]);
        run.code(1);
        assert_eq!(run.stdout, text(&dangling, false), "{fixture} dangling");
        let errors: Vec<&str> = run
            .stdout
            .lines()
            .filter(|line| line.starts_with("error  "))
            .collect();
        assert_eq!(errors.len(), 1, "{fixture}: {}", run.stdout);
        assert!(
            errors[0].starts_with("error  docs/spec/zz-dangling.md:")
                && errors[0].contains(": ref-dangling: "),
            "{fixture}: {}",
            errors[0]
        );
        assert!(summary(&run.stdout).ends_with(" — blocked"));

        // `observe`: the same errors, exit 0.
        let config = read_text(&root, "specengine.toml");
        write(
            &root,
            "specengine.toml",
            format!("{config}\n[check]\nmode = \"observe\"\n"),
        );
        let observed = library(&root);
        assert_eq!(observed.verdict, Verdict::Observed, "{fixture}");
        let run = spec(&home, &root, &["check"]);
        run.code(0);
        assert_eq!(run.stdout, text(&observed, false), "{fixture} observe");
        assert!(summary(&run.stdout).ends_with(" — observed"));
        let run = spec(&home, &root, &["check", "--debt"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            text(&observed, true),
            "{fixture} observe --debt"
        );
    }
}

/// AC-03: the copy's `.spec-debt.toml` applies; `--baseline` replaces it
/// (relative to the current directory); a missing one is a cause named as
/// typed; an expired entry blocks again, a far one is debt.
#[test]
fn the_baseline_default_override_missing_and_expiry() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-baseline");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let outside = scratch.dir("outside");
        let config = root.join("specengine.toml");
        let blocked = library(&root);

        // The copy's baseline applies.
        write(&root, ".spec-debt.toml", baseline_covering(&blocked, FAR));
        let covered = library(&root);
        assert_eq!(covered.verdict, Verdict::Clean);
        let run = spec(&home, &root, &["check"]);
        run.code(0);
        assert_eq!(run.stdout, text(&covered, false), "{fixture} default");

        // `--baseline` without the entries replaces it: one stale entry.
        write(
            &outside,
            "other.toml",
            "[[debt]]\ncode    = \"ref-dangling\"\npath    = \"docs/none.md\"\nreason  = \"elsewhere\"\nexpires = \"2999-12-31\"\n",
        );
        let other = outside.join("other.toml");
        let replaced = library_with(&root, &config, Some(&other));
        assert_eq!(replaced.verdict, Verdict::Blocked);
        let from_root = spec(
            &home,
            &root,
            &["check", "--baseline", "../outside/other.toml"],
        );
        from_root.code(1);
        assert_eq!(
            from_root.stdout,
            text(&replaced, false),
            "{fixture} override"
        );
        assert!(summary(&from_root.stdout).contains(", 1 stale — blocked"));
        let root_arg = root.to_str().unwrap();
        let from_outside = spec(
            &home,
            &outside,
            &["--root", root_arg, "check", "--baseline", "other.toml"],
        );
        from_outside.code(1);
        assert_eq!(
            from_outside.stdout, from_root.stdout,
            "{fixture}: cwd-relative"
        );

        // A missing `--baseline`: exit 2, the cause as typed.
        for typed in ["nope.toml", "./sub/../nope.toml"] {
            let run = spec(&home, &root, &["check", "--baseline", typed]);
            run.code(2);
            let causes: Vec<&str> = run
                .stdout
                .lines()
                .filter(|line| line.starts_with("cannot  "))
                .collect();
            assert_eq!(causes.len(), 1, "{fixture} {typed}: {}", run.stdout);
            assert!(
                causes[0].starts_with(&format!("cannot  {typed}: cannot read the baseline: ")),
                "{fixture}: {}",
                causes[0]
            );
            assert!(summary(&run.stdout).ends_with(" — cannot-check"));
            assert_eq!(run.stderr, "");
        }

        // An expired entry blocks again.
        write(&root, ".spec-debt.toml", baseline_covering(&blocked, PAST));
        let expired = library(&root);
        assert_eq!(expired.counts.expired, 1, "{fixture}: one expired finding");
        let run = spec(&home, &root, &["check"]);
        run.code(1);
        assert_eq!(run.stdout, text(&expired, false), "{fixture} expired");
        assert!(
            summary(&run.stdout).contains(", 1 expired, "),
            "{}",
            run.stdout
        );
        assert!(
            run.stdout
                .contains(&format!("(debt expired {PAST}: test debt)")),
            "{}",
            run.stdout
        );

        // A far entry is debt.
        write(&root, ".spec-debt.toml", baseline_covering(&blocked, FAR));
        let run = spec(&home, &root, &["check", "--debt"]);
        run.code(0);
        let debt: Vec<&str> = run
            .stdout
            .lines()
            .filter(|line| line.starts_with("debt  "))
            .collect();
        assert_eq!(debt.len(), blocking(&blocked).len(), "{}", run.stdout);
        assert!(
            debt.iter()
                .all(|line| line.ends_with(&format!("(debt until {FAR}: test debt)"))),
            "{debt:?}"
        );
    }
}

/// AC-04: `--json` is exactly `to_json()` and a line end for clean,
/// blocked, observed and cannot-check; `--json --debt` the same bytes and a
/// `note:`; no config up the tree: exit 2, nothing on stdout, one stderr
/// line naming `spec init`.
#[test]
fn json_is_the_report_s_own_for_every_verdict() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-json");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let outside = scratch.dir("outside");
        let config = root.join("specengine.toml");

        let blocked = library(&root);
        let run = spec(&home, &root, &["--json", "check"]);
        run.code(1);
        assert_eq!(run.stdout, json(&blocked), "{fixture} blocked");
        assert_eq!(run.stderr, "");
        let debt = spec(&home, &root, &["check", "--debt", "--json"]);
        debt.code(1);
        assert_eq!(debt.stdout, run.stdout, "{fixture}: --json --debt");
        let notes = debt.stderr_lines();
        assert_eq!(notes.len(), 1, "{}", debt.stderr);
        assert!(notes[0].starts_with("note: "), "{}", debt.stderr);

        // Clean: a baseline outside the copy covers every blocking finding.
        write(&outside, "debt.toml", baseline_covering(&blocked, FAR));
        let clean = library_with(&root, &config, Some(&outside.join("debt.toml")));
        assert_eq!(clean.verdict, Verdict::Clean);
        let run = spec(
            &home,
            &root,
            &["--json", "check", "--baseline", "../outside/debt.toml"],
        );
        run.code(0);
        assert_eq!(run.stdout, json(&clean), "{fixture} clean");

        // Cannot-check: a missing baseline, named as typed.
        let cannot = library_with(&root, &config, Some(Path::new("nope.toml")));
        assert_eq!(cannot.verdict, Verdict::CannotCheck);
        let run = spec(
            &home,
            &root,
            &["check", "--json", "--baseline", "nope.toml"],
        );
        run.code(2);
        assert_eq!(run.stdout, json(&cannot), "{fixture} cannot-check");
        let run = spec(
            &home,
            &root,
            &["check", "--json", "--debt", "--baseline", "nope.toml"],
        );
        run.code(2);
        assert_eq!(run.stdout, json(&cannot), "{fixture} cannot-check --debt");

        // Observed.
        let text = read_text(&root, "specengine.toml");
        write(
            &root,
            "specengine.toml",
            format!("{text}\n[check]\nmode = \"observe\"\n"),
        );
        let observed = library(&root);
        assert_eq!(observed.verdict, Verdict::Observed);
        let run = spec(&home, &root, &["--json", "check"]);
        run.code(0);
        assert_eq!(run.stdout, json(&observed), "{fixture} observed");

        // Cannot-check: a config error.
        write(
            &root,
            "specengine.toml",
            format!("{text}\n[bogus]\nx = 1\n"),
        );
        let broken = library(&root);
        assert_eq!(broken.verdict, Verdict::CannotCheck);
        let run = spec(&home, &root, &["--json", "check"]);
        run.code(2);
        assert_eq!(run.stdout, json(&broken), "{fixture} config error");
    }

    let scratch = Scratch::new("check-json-none");
    let home = scratch.home("h");
    let empty = scratch.dir("empty");
    for args in [&["--json", "check"][..], &["check"][..]] {
        let run = spec(&home, &empty, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{}", run.stderr);
        assert!(
            lines[0].starts_with("spec: ") && lines[0].contains("spec init"),
            "{}",
            run.stderr
        );
    }
}

/// AC-05: an unknown top-level table, an unknown `[project]` key (at
/// `spec index`'s line), `tier0_bytes = 0`, `mode = "strict"`: exit 2, one
/// cause each at `specengine.toml:<line>` (`--config` as typed); no
/// `[project]` table checks normally.
#[test]
fn config_errors_are_one_cause_each_at_their_line() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-config");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let config = read_text(&root, "specengine.toml");

        let bogus = format!("{config}\n[bogus]\nx = 1\n");
        let project_key = with_project_key(&config, "flavour = 1");
        let tier0 = format!("{config}\n[budgets]\ntier0_bytes = 0\n");
        let strict = format!("{config}\n[check]\nmode = {}\n", quoted("strict"));
        let cases = [
            ("unknown table", bogus.clone(), line_of(&bogus, "[bogus]")),
            (
                "unknown [project] key",
                project_key.clone(),
                line_of(&project_key, "flavour = 1"),
            ),
            (
                "tier0_bytes = 0",
                tier0.clone(),
                line_of(&tier0, "tier0_bytes"),
            ),
            ("mode = strict", strict.clone(), line_of(&strict, "mode = ")),
        ];
        for (what, bad, line) in cases {
            write(&root, "specengine.toml", &bad);
            let run = spec(&home, &root, &["check"]);
            run.code(2);
            let causes: Vec<&str> = run
                .stdout
                .lines()
                .filter(|line| line.starts_with("cannot  "))
                .collect();
            assert_eq!(causes.len(), 1, "{fixture} {what}:\n{}", run.stdout);
            assert!(
                causes[0].starts_with(&format!("cannot  specengine.toml:{line}: ")),
                "{fixture} {what}: {}",
                causes[0]
            );
            assert_eq!(run.stdout.lines().count(), 2, "{fixture} {what}");
            assert!(summary(&run.stdout).ends_with(" — cannot-check"));
            assert_eq!(run.stdout, text(&library(&root), false), "{fixture} {what}");
            assert_eq!(run.stderr, "", "{fixture} {what}");

            // `--config` as typed.
            write(&root, "conf/alt.toml", &bad);
            for typed in ["conf/alt.toml", "./conf/alt.toml"] {
                let run = spec(&home, &root, &["--config", typed, "check"]);
                run.code(2);
                assert!(
                    run.stdout.starts_with(&format!("cannot  {typed}:{line}: ")),
                    "{fixture} {what} {typed}: {}",
                    run.stdout
                );
                assert_eq!(run.stdout.lines().count(), 2, "{fixture} {what} {typed}");
            }
            if what == "unknown [project] key" {
                // `spec index` names the same line.
                let index = spec(&home, &root, &["index"]);
                index.code(2);
                assert!(
                    index
                        .stderr
                        .starts_with(&format!("specengine.toml:{line}: ")),
                    "{fixture}: spec index says {}",
                    index.stderr
                );
            }
        }

        // No `[project]`: checks normally.
        write(&root, "specengine.toml", without_project(&config));
        let report = library(&root);
        assert_ne!(report.verdict, Verdict::CannotCheck, "{fixture}");
        let run = spec(&home, &root, &["check"]);
        assert_eq!(run.code, i32::from(report.exit_code()), "{}", run.show());
        assert_eq!(
            run.stdout,
            text(&report, false),
            "{fixture} without [project]"
        );
    }
}

/// AC-08: a file named with a line break and holding a blocking error:
/// stdout has one line per blocking finding plus the summary; the JSON
/// `path` keeps the LF.
#[test]
fn a_line_break_in_a_file_name_keeps_one_line_per_finding() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-names");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let name = "docs/spec/a\nb.md";
        write(&root, name, dangling_document(fixture));
        let report = library(&root);
        let blocks = report
            .findings
            .iter()
            .filter(|finding| report.blocks(finding))
            .count();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.path == name && report.blocks(finding)),
            "{fixture}: the named file blocks"
        );
        let run = spec(&home, &root, &["check"]);
        run.code(1);
        assert_eq!(run.stdout.lines().count(), blocks + 1, "{}", run.stdout);
        assert_eq!(run.stdout, text(&report, false), "{fixture}");
        assert!(
            run.stdout
                .lines()
                .any(|line| line.starts_with("error  docs/spec/a b.md:")),
            "{}",
            run.stdout
        );

        let run = spec(&home, &root, &["--json", "check"]);
        run.code(1);
        assert_eq!(run.stdout, json(&report), "{fixture}");
        let document = run.json();
        assert!(
            document["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["path"] == name),
            "{fixture}: the raw path in JSON"
        );
    }
}

/// Iteration 2 (discrepancy 2 of iteration 1): a mode-000 root and no
/// `--baseline`: the one cause is the root, `.` ("directory cannot be
/// listed"; JSON path `""`), never a `.spec-debt.toml` the check cannot
/// see, whether or not one is there. A root that can be listed but not
/// searched, holding a `.spec-debt.toml`, keeps the baseline's cause.
#[test]
fn an_unreadable_root_is_the_cause_not_a_phantom_baseline() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-locked-root");
        let home = scratch.home("h");
        let bare = scratch.copy(fixture, "bare");
        let with_debt = scratch.copy(fixture, "with-debt");
        write(
            &with_debt,
            ".spec-debt.toml",
            "[[debt]]\ncode    = \"ref-dangling\"\npath    = \"docs/none.md\"\nreason  = \"r\"\nexpires = \"2999-12-31\"\n",
        );
        write(
            scratch.path(),
            "cfg/specengine.toml",
            read_text(&bare, "specengine.toml"),
        );
        for root in [&bare, &with_debt] {
            let name = root.file_name().unwrap().to_str().unwrap();
            fs::set_permissions(root, fs::Permissions::from_mode(0o000)).unwrap();
            let args = ["--root", name, "--config", "cfg/specengine.toml", "check"];
            let run = spec(&home, scratch.path(), &args);
            let json = spec(
                &home,
                scratch.path(),
                &[&["--json"][..], &args[..]].concat(),
            );
            fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap();
            run.code(2);
            assert_eq!(
                run.stdout,
                "cannot  .: directory cannot be listed; its files are unchecked\n\
                 spec check [enforce]: 0 documents, 0 errors, 0 warnings, 0 debt, 0 expired, 0 stale — cannot-check\n",
                "{fixture} {name}"
            );
            assert!(!run.stdout.contains(".spec-debt.toml"), "{fixture} {name}");
            assert_eq!(run.stderr, "");
            json.code(2);
            let document = json.json();
            assert_eq!(
                document["cannot_check"],
                serde_json::json!([{"path": "", "message": "directory cannot be listed; its files are unchecked"}]),
                "{fixture} {name}"
            );
        }

        // Listed, not searchable, the baseline there: its cause.
        fs::set_permissions(&with_debt, fs::Permissions::from_mode(0o444)).unwrap();
        let run = spec(
            &home,
            scratch.path(),
            &[
                "--root",
                "with-debt",
                "--config",
                "cfg/specengine.toml",
                "check",
            ],
        );
        fs::set_permissions(&with_debt, fs::Permissions::from_mode(0o755)).unwrap();
        run.code(2);
        let causes: Vec<&str> = run
            .stdout
            .lines()
            .filter(|line| line.starts_with("cannot  "))
            .collect();
        assert_eq!(
            causes,
            ["cannot  .spec-debt.toml: cannot read the baseline: Permission denied (os error 13)"],
            "{fixture}"
        );
    }
}

/// The stdout of a check whose one cause is the unlistable `docs`.
const DOCS_UNCHECKED: &str = "cannot  docs: directory cannot be listed; its files are unchecked\n\
    spec check [enforce]: 0 documents, 0 errors, 0 warnings, 0 debt, 0 expired, 0 stale — cannot-check\n";

/// `docs` as the JSON report's one cause.
fn docs_cause() -> serde_json::Value {
    serde_json::json!([{"path": "docs", "message": "directory cannot be listed; its files are unchecked"}])
}

/// Edge data (iteration 2's finding, fixed in iteration 3): a directory on
/// the way to the walked roots that cannot be listed (`docs`, mode 000)
/// leaves every document unchecked: `spec check` names it, `cannot-check`,
/// exit 2, with default role roots (spec-a) as with written ones (spec-b);
/// never `clean`, never a baseline cause.
#[test]
fn an_unlistable_directory_above_the_roots_is_not_clean() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-locked-docs");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let docs = root.join("docs");
        fs::set_permissions(&docs, fs::Permissions::from_mode(0o000)).unwrap();
        let run = spec(&home, &root, &["check"]);
        let json = spec(&home, &root, &["--json", "check"]);
        fs::set_permissions(&docs, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(run.code, 2, "{fixture}: docs/ mode 000\n{}", run.show());
        assert_eq!(run.stdout, DOCS_UNCHECKED, "{fixture}");
        assert_eq!(run.stderr, "", "{fixture}");
        json.code(2);
        assert_eq!(json.json()["cannot_check"], docs_cause(), "{fixture}");

        // Readable again: the verdict of the library, no cause.
        let report = library(&root);
        let run = spec(&home, &root, &["check"]);
        assert_eq!(run.code, i32::from(report.exit_code()), "{fixture}");
        assert_eq!(run.stdout, text(&report, false), "{fixture}");
        assert!(report.cannot_check.is_empty(), "{fixture}");
    }
}

/// Iteration 3: a root that can be listed but not searched (mode 0444),
/// the config outside it: the cause is `docs`, the directory the walk
/// could not enter; no baseline is blamed.
#[test]
fn a_root_listed_but_not_searched_names_the_directory_on_the_way() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("check-0444-root");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        write(
            scratch.path(),
            "cfg/specengine.toml",
            read_text(&root, "specengine.toml"),
        );
        let args = ["--root", "copy", "--config", "cfg/specengine.toml", "check"];
        fs::set_permissions(&root, fs::Permissions::from_mode(0o444)).unwrap();
        let run = spec(&home, scratch.path(), &args);
        let json = spec(
            &home,
            scratch.path(),
            &[&["--json"][..], &args[..]].concat(),
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(run.code, 2, "{fixture}\n{}", run.show());
        assert_eq!(run.stdout, DOCS_UNCHECKED, "{fixture}");
        assert!(!run.stdout.contains(".spec-debt.toml"), "{fixture}");
        json.code(2);
        assert_eq!(json.json()["cannot_check"], docs_cause(), "{fixture}");
    }
}

/// Iteration 3: `spec index` in the same setup answers (exit 0) with one
/// warning naming `docs`; the incremental update after `chmod 000 docs`
/// removes every row under it, as a rebuild in the same state holds none:
/// both answer every query alike, with nothing found.
#[test]
fn spec_index_warns_and_drops_the_rows_of_an_unlistable_directory() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("index-locked-docs");
        let incremental = scratch.home("incremental");
        let rebuilt = scratch.home("rebuilt");
        let root = scratch.copy(fixture, "copy");
        let first = spec(&incremental, &root, &["index"]);
        first.code(0);
        let walked = library(&root).counts.documents;
        assert!(
            first.stdout.contains(&format!("walked {walked}, ")),
            "{}",
            first.stdout
        );

        let docs = root.join("docs");
        fs::set_permissions(&docs, fs::Permissions::from_mode(0o000)).unwrap();
        let update = spec(&incremental, &root, &["index"]);
        let full = spec(&rebuilt, &root, &["index", "--full"]);
        let (id, word) = if fixture == "spec-a" {
            ("R-12", "stamina")
        } else {
            ("REQ-001", "sync")
        };
        let queries: [&[&str]; 3] = [
            &["--json", "show", id],
            &["--json", "search", word],
            &["--json", "search", word, "--archive"],
        ];
        let answers: Vec<(common::Run, common::Run)> = queries
            .iter()
            .map(|args| (spec(&incremental, &root, args), spec(&rebuilt, &root, args)))
            .collect();
        fs::set_permissions(&docs, fs::Permissions::from_mode(0o755)).unwrap();

        let warning = "warning: cannot list `docs`: its files are left out of the index\n";
        update.code(0);
        assert_eq!(update.stderr, warning, "{fixture}");
        assert!(
            update.stdout.contains(&format!(
                "walked 0, parsed 0, unchanged 0, removed {walked}, "
            )),
            "{fixture}: {}",
            update.stdout
        );
        full.code(0);
        assert_eq!(full.stderr, warning, "{fixture}");
        assert!(full.stdout.contains("walked 0, "), "{}", full.stdout);
        for ((one, two), args) in answers.iter().zip(queries) {
            assert_eq!(one.code, two.code, "{fixture} {args:?}");
            assert_eq!(one.stdout, two.stdout, "{fixture} {args:?}");
            assert_eq!(one.stderr, two.stderr, "{fixture} {args:?}");
        }
        assert_eq!(answers[0].0.code, 1, "{fixture}: {id} still indexed");
        assert_eq!(
            answers[1].0.json()["hits"],
            serde_json::json!([]),
            "{fixture}"
        );
    }
}
