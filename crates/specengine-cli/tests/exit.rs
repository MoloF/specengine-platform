//! AC-14 of docs/features/spec-cli.md: every exit case of "Data" across
//! `init`, `index`, `search` and `show`, in text and `--json`: 0 answered
//! (zero hits included), 1 `show` found nothing, 2 could not run; with
//! `--json` one document on stdout for 0 and 1 and nothing for 2; errors,
//! warnings and notes only on stderr, in their forms; no ANSI escape, no
//! timing.

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Run, Scratch, assert_plain, data_dir, index, spec, spec_with, write};

/// How a case runs: its current directory, its `HOME` (`None`: unset).
struct Case {
    what: &'static str,
    cwd: PathBuf,
    home: Option<PathBuf>,
    args: Vec<String>,
    exit: i32,
    /// A clap usage error: only its first stderr line is `spec: …`.
    usage: bool,
}

fn run_case(case: &Case, json: bool) -> Run {
    let mut args: Vec<&str> = case.args.iter().map(String::as_str).collect();
    if json {
        args.insert(0, "--json");
    }
    match &case.home {
        Some(home) => spec(home, &case.cwd, &args),
        None => spec_with(&case.cwd, &args, &[]),
    }
}

/// Each stderr line is `spec: …`, `<config>:<line>: …`, `warning: …` or
/// `note: …`.
fn assert_stderr_forms(run: &Run, case: &Case) {
    let lines = run.stderr_lines();
    let checked: &[&str] = if case.usage {
        &lines[..1.min(lines.len())]
    } else {
        &lines
    };
    for line in checked {
        let config_form = line
            .split_once(':')
            .and_then(|(file, rest)| {
                let (number, message) = rest.split_once(": ")?;
                (file.ends_with(".toml") && number.parse::<usize>().is_ok() && !message.is_empty())
                    .then_some(())
            })
            .is_some();
        assert!(
            line.starts_with("spec: ")
                || line.starts_with("warning: ")
                || line.starts_with("note: ")
                || config_form,
            "{}: a stderr line of no known form: {line:?}\n{}",
            case.what,
            run.show()
        );
    }
    if case.exit == 2 {
        assert!(!lines.is_empty(), "{}: exit 2 says why", case.what);
    }
}

fn cases(scratch: &Scratch) -> Vec<Case> {
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let empty = scratch.dir("empty");
    let fresh = scratch.dir("fresh");
    let bad_slug = scratch.copy("spec-a", "bad-slug");
    fs::write(
        bad_slug.join("specengine.toml"),
        "[project]\nslug = \"Bad\"\n",
    )
    .unwrap();
    let no_slug = scratch.copy("spec-a", "no-slug");
    fs::write(no_slug.join("specengine.toml"), "[project]\nname = \"x\"\n").unwrap();
    // A database that is no SQLite file: a store error.
    let broken_home = scratch.home("broken-db");
    fs::create_dir_all(data_dir(&broken_home)).unwrap();
    fs::write(
        data_dir(&broken_home).join("lantern-keep.db"),
        vec![0x55; 8192],
    )
    .unwrap();
    // A data directory that cannot be created: a file in its place.
    let blocked_home = scratch.home("blocked");
    let blocked = data_dir(&blocked_home);
    fs::create_dir_all(blocked.parent().unwrap()).unwrap();
    fs::write(&blocked, "a file").unwrap();
    let config = root.join("specengine.toml");

    let case =
        |what: &'static str, cwd: &Path, home: Option<&Path>, args: &[&str], exit: i32| Case {
            what,
            cwd: cwd.to_path_buf(),
            home: home.map(Path::to_path_buf),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            exit,
            usage: false,
        };
    let usage = |what: &'static str, args: &[&str]| Case {
        usage: true,
        ..case(what, &root, Some(&home), args, 2)
    };
    let h = Some(home.as_path());
    let root_arg = root.to_str().unwrap().to_owned();
    let config_arg = config.to_str().unwrap().to_owned();
    vec![
        // 0: answered.
        case("init", &fresh, h, &["init"], 0),
        case("index", &root, h, &["index"], 0),
        case("index --full", &root, h, &["index", "--full"], 0),
        case("search with hits", &root, h, &["search", "stamina"], 0),
        case("search, zero hits", &root, h, &["search", "zzqqxxwwvv"], 0),
        case(
            "search --archive",
            &root,
            h,
            &["search", "replaced", "--archive"],
            0,
        ),
        case("show by ID", &root, h, &["show", "R-12"], 0),
        case("show by path", &root, h, &["show", "docs/spec/game.md"], 0),
        case("show with a rev note", &root, h, &["show", "R-12@2"], 0),
        // 1: show found nothing.
        case("show dangling", &root, h, &["show", "R-99"], 1),
        case("show no configured prefix", &root, h, &["show", "FOO-1"], 1),
        case(
            "show no reference at all",
            &root,
            h,
            &["show", "stamina"],
            1,
        ),
        case(
            "show a path not indexed",
            &root,
            h,
            &["show", "docs/nope.md"],
            1,
        ),
        case(
            "show a path outside the roots",
            &root,
            h,
            &["show", "README.md"],
            1,
        ),
        // 2: could not run.
        usage("usage: unknown command", &["bogus"]),
        usage("usage: show without REF", &["show"]),
        usage("usage: search without QUERY", &["search"]),
        usage("usage: unknown flag", &["index", "--nope"]),
        usage("usage: no command", &[]),
        usage(
            "usage: a non-number limit",
            &["search", "stamina", "--limit", "x"],
        ),
        case("no project", &empty, h, &["index"], 2),
        case("no project (show)", &empty, h, &["show", "R-12"], 2),
        case("config error", &bad_slug, h, &["index"], 2),
        case("no slug", &no_slug, h, &["show", "R-12"], 2),
        case("HOME unset", &root, None, &["index"], 2),
        case(
            "HOME unset (search)",
            &root,
            None,
            &["search", "stamina"],
            2,
        ),
        case(
            "data directory in the root",
            &root,
            Some(root.as_path()),
            &["index"],
            2,
        ),
        case(
            "data directory blocked by a file",
            &root,
            Some(&blocked_home),
            &["index"],
            2,
        ),
        case("store error", &root, Some(&broken_home), &["index"], 2),
        case(
            "store error (show)",
            &root,
            Some(&broken_home),
            &["show", "R-12"],
            2,
        ),
        case(
            "store error (search)",
            &root,
            Some(&broken_home),
            &["search", "stamina"],
            2,
        ),
        case("init over an existing file", &root, h, &["init"], 2),
        case(
            "init --root a file",
            &empty,
            h,
            &["--root", &config_arg, "init"],
            2,
        ),
        case("init bad slug", &fresh, h, &["init", "--slug", "Bad"], 2),
        case(
            "init --config",
            &empty,
            h,
            &["--config", &config_arg, "init"],
            2,
        ),
        case(
            "--config missing",
            &empty,
            h,
            &["--config", "none.toml", "index"],
            2,
        ),
        case("--root missing", &empty, h, &["--root", "none", "index"], 2),
        case(
            "--root a file",
            &empty,
            h,
            &["--root", &config_arg, "index"],
            2,
        ),
        case(
            "no term of 3+ characters",
            &root,
            h,
            &["search", "ab", "c"],
            2,
        ),
        case(
            "--limit 0",
            &root,
            h,
            &["search", "stamina", "--limit", "0"],
            2,
        ),
        case(
            "--limit 201",
            &root,
            h,
            &["search", "stamina", "--limit", "201"],
            2,
        ),
        case("project:", &root, h, &["show", "other:R-12"], 2),
        case("look-alike ID", &root, h, &["show", "\u{0410}-101"], 2),
        case(
            "mixed-script ID",
            &root,
            h,
            &["show", "M\u{0415}C-STAMINA"],
            2,
        ),
        case("non-clean path ..", &root, h, &["show", "../x.md"], 2),
        case(
            "non-clean path absolute",
            &root,
            h,
            &["show", "/etc/x.md"],
            2,
        ),
        case(
            "non-clean path .",
            &root,
            h,
            &["show", "docs/./spec/game.md"],
            2,
        ),
        case(
            "non-clean path //",
            &root,
            h,
            &["show", "docs//spec/game.md"],
            2,
        ),
    ]
    .into_iter()
    .map(|mut case| {
        // `--root` of the copy, from elsewhere, must not change the verdict.
        if case.what == "index" {
            case.args = vec!["--root".into(), root_arg.clone(), "index".into()];
            case.cwd = empty.clone();
        }
        case
    })
    .collect()
}

#[test]
fn every_exit_case_in_text_and_json() {
    let scratch = Scratch::new("exit");
    let cases = cases(&scratch);
    for case in &cases {
        for json in [false, true] {
            if case.what == "init" && json {
                // `init` succeeds once; its JSON is covered by init.rs.
                continue;
            }
            let run = run_case(case, json);
            assert_eq!(
                run.code,
                case.exit,
                "{} (json {json}): {:?}\n{}",
                case.what,
                case.args,
                run.show()
            );
            match (case.exit, json) {
                (2, _) => assert_eq!(
                    run.stdout, "",
                    "{}: exit 2 prints nothing on stdout",
                    case.what
                ),
                (_, true) => {
                    run.json();
                }
                (0, false) => assert!(!run.stdout.is_empty(), "{}: an answer", case.what),
                (1, false) => {
                    assert_eq!(run.stdout, "", "{}: exit 1 text prints no node", case.what)
                }
                _ => unreachable!(),
            }
            assert_stderr_forms(&run, case);
            assert!(
                !run.stdout.contains('\u{1b}') && !run.stderr.contains('\u{1b}'),
                "{}: ANSI",
                case.what
            );
            assert_plain(&run.stderr, case.what);
            if case.args.iter().any(|arg| arg == "index") {
                assert_plain(&run.stdout, case.what);
            }
            // Nothing of stderr leaks into stdout.
            for line in run.stderr_lines() {
                if !line.is_empty() {
                    assert!(
                        !run.stdout.contains(line),
                        "{}: {line:?} on stdout",
                        case.what
                    );
                }
            }
        }
    }
}

/// Exit 1 in JSON: the document says why; in text, the reason is the one
/// `spec: ` line on stderr.
#[test]
fn exit_1_carries_the_reason() {
    let scratch = Scratch::new("exit-1");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");
    index(&home, &root);
    let run = spec(&home, &root, &["--json", "show", "REQ-999"]);
    run.code(1);
    let json = run.json();
    let reason = json["reason"].as_str().unwrap();
    assert!(reason.contains("REQ-999"), "{json}");
    assert_eq!(json["nodes"], serde_json::json!([]));
    assert_eq!(run.stderr, format!("spec: {reason}\n"));
    let text = spec(&home, &root, &["show", "REQ-999"]);
    text.code(1);
    assert_eq!(text.stderr, run.stderr);
}

/// `--json` goes anywhere on the line.
#[test]
fn json_anywhere_on_the_line() {
    let scratch = Scratch::new("exit-json");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let first = spec(&home, &root, &["--json", "show", "R-12"]);
    let last = spec(&home, &root, &["show", "R-12", "--json"]);
    let middle = spec(&home, &root, &["show", "--json", "R-12"]);
    assert_eq!(first.stdout, last.stdout);
    assert_eq!(first.stdout, middle.stdout);
    first.json();
    let _ = OsStr::new("");
    write(&root, "docs/spec/extra.md", "# Extra\n");
    let run = spec(&home, &root, &["index", "--json"]);
    run.code(0);
    run.json();
}

// ------------------------------------------------------------------ pass 2a.1

/// Each stderr line of a non-usage run is `spec: …`, `warning: …`,
/// `note: …` or `<config>:<line>: …`; a usage error starts with `spec: …`,
/// clap's text (its `Usage:` block, when it gives one) after it. No ANSI
/// escape anywhere.
fn assert_streams(what: &str, run: &Run, usage: bool) {
    let lines = run.stderr_lines();
    if usage {
        assert!(
            lines.first().is_some_and(|line| line.starts_with("spec: ")),
            "{what}: {}",
            run.stderr
        );
    } else {
        for line in &lines {
            let config_form = line.split_once(':').is_some_and(|(file, rest)| {
                file.ends_with(".toml")
                    && rest.split_once(": ").is_some_and(|(number, message)| {
                        number.parse::<usize>().is_ok() && !message.is_empty()
                    })
            });
            assert!(
                line.starts_with("spec: ")
                    || line.starts_with("warning: ")
                    || line.starts_with("note: ")
                    || config_form,
                "{what}: a stderr line of no known form: {line:?}\n{}",
                run.show()
            );
        }
    }
    assert!(
        !run.stdout.contains('\u{1b}') && !run.stderr.contains('\u{1b}'),
        "{what}: ANSI"
    );
    assert_plain(&run.stderr, what);
}

/// A report on stdout: text ending in the verdict's summary, or one JSON
/// document with that verdict.
fn assert_report(what: &str, run: &Run, json: bool, verdict: &str) {
    if json {
        assert_eq!(run.json()["verdict"], verdict, "{what}");
    } else {
        let last = run.stdout.lines().last().unwrap_or_default();
        assert!(
            last.starts_with("spec check [") && last.ends_with(&format!(" — {verdict}")),
            "{what}: {}",
            run.show()
        );
    }
    // Nothing but `note:` on stderr beside a report.
    for line in run.stderr_lines() {
        assert!(
            line.starts_with("note: "),
            "{what}: {line:?} beside a report"
        );
    }
}

fn with_json<'a>(json: bool, args: &[&'a str]) -> Vec<&'a str> {
    let mut all = args.to_vec();
    if json {
        all.insert(0, "--json");
    }
    all
}

/// AC-12 of docs/features/spec-cli-check.md: every row of "Exit codes and
/// streams" for `check` and `export index`, in text and `--json`.
#[test]
fn check_and_export_rows_in_text_and_json() {
    use common::check::{baseline_covering, index_path, library, registered};
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("exit-check");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let outside = scratch.dir("outside");
    let base = fs::read_to_string(root.join("specengine.toml")).unwrap();
    write(
        &outside,
        "cover.toml",
        baseline_covering(&library(&root), "2999-12-31"),
    );
    write(
        &outside,
        "observe.toml",
        format!("{base}\n[check]\nmode = \"observe\"\n"),
    );
    write(&outside, "bogus.toml", format!("{base}\n[bogus]\nx = 1\n"));
    write(&outside, "specengine.toml", &base);
    write(&outside, "empty.toml", "");
    let locked = scratch.copy("spec-a", "locked");
    let unreadable = scratch.copy("spec-a", "unreadable");
    fs::set_permissions(
        unreadable.join("docs/spec/game.md"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let empty = scratch.dir("empty");

    // `spec check`: every verdict prints its report.
    let reports: [(&str, &Path, Vec<&str>, i32, &str); 8] = [
        (
            "check clean",
            &root,
            vec!["check", "--baseline", "../outside/cover.toml"],
            0,
            "clean",
        ),
        (
            "check observed",
            &root,
            vec!["--config", "../outside/observe.toml", "check"],
            0,
            "observed",
        ),
        ("check blocked", &root, vec!["check"], 1, "blocked"),
        (
            "check blocked --debt",
            &root,
            vec!["check", "--debt"],
            1,
            "blocked",
        ),
        (
            "check cannot: config",
            &root,
            vec!["--config", "../outside/bogus.toml", "check"],
            2,
            "cannot-check",
        ),
        (
            "check cannot: baseline",
            &root,
            vec!["check", "--baseline", "nope.toml"],
            2,
            "cannot-check",
        ),
        (
            "check cannot: root",
            scratch.path(),
            vec![
                "--root",
                "locked",
                "--config",
                "outside/specengine.toml",
                "check",
                "--baseline",
                "outside/empty.toml",
            ],
            2,
            "cannot-check",
        ),
        (
            "check cannot: walk",
            &unreadable,
            vec!["check"],
            2,
            "cannot-check",
        ),
    ];
    for (what, cwd, args, exit, verdict) in &reports {
        for json in [false, true] {
            let run = spec(&home, cwd, &with_json(json, args));
            assert_eq!(run.code, *exit, "{what} (json {json})\n{}", run.show());
            assert_report(what, &run, json, verdict);
            assert_streams(what, &run, false);
            let notes = run.stderr_lines().len();
            let debt = args.contains(&"--debt");
            assert_eq!(
                notes,
                usize::from(json && debt),
                "{what} (json {json}): notes"
            );
        }
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    // Usage errors and discovery failures: nothing on stdout.
    let failures: [(&str, &Path, Vec<&str>, bool); 9] = [
        (
            "usage: unknown check flag",
            &root,
            vec!["check", "--nope"],
            true,
        ),
        (
            "usage: --baseline without F",
            &root,
            vec!["check", "--baseline"],
            true,
        ),
        ("usage: bare export", &root, vec!["export"], true),
        ("usage: unknown export", &root, vec!["export", "nope"], true),
        (
            "usage: export index extra",
            &root,
            vec!["export", "index", "extra"],
            true,
        ),
        ("discovery: no config (check)", &empty, vec!["check"], false),
        (
            "discovery: no config (export)",
            &empty,
            vec!["export", "index"],
            false,
        ),
        (
            "discovery: --root missing",
            &empty,
            vec!["--root", "none", "check"],
            false,
        ),
        (
            "discovery: --root missing (export)",
            &empty,
            vec!["--root", "none", "export", "index"],
            false,
        ),
    ];
    for (what, cwd, args, usage) in &failures {
        for json in [false, true] {
            let run = spec(&home, cwd, &with_json(json, args));
            assert_eq!(run.code, 2, "{what} (json {json})\n{}", run.show());
            assert_eq!(run.stdout, "", "{what} (json {json})");
            assert_streams(what, &run, *usage);
            if !usage {
                assert_eq!(run.stderr_lines().len(), 1, "{what}: {}", run.stderr);
            }
        }
    }
    // `--stdout` with `--json`: every order in `stdout_json_conflict_in_every_order`.
    let run = spec(&home, &root, &["export", "index", "--stdout", "--json"]);
    assert_eq!(run.code, 2, "{}", run.show());
    assert_eq!(run.stdout, "");
    assert_eq!(run.stderr, STDOUT_JSON_CONFLICT);
    assert_streams("--stdout --json", &run, true);
    let run = spec(&home, &root, &["export"]);
    assert!(
        run.stderr_lines()
            .iter()
            .any(|line| line.starts_with("Usage: spec export")),
        "{}",
        run.stderr
    );

    // `spec export index`: wrote, unchanged, --stdout; a warning.
    let index = index_path("spec-a");
    write(
        &root,
        "specengine.toml",
        registered(&base, index, "gen-index", None),
    );
    for (what, json, expected) in [
        ("export wrote", false, "wrote"),
        ("export unchanged", false, "unchanged"),
        ("export unchanged (json)", true, "unchanged"),
    ] {
        let run = spec(&home, &root, &with_json(json, &["export", "index"]));
        run.code(0);
        if json {
            assert_eq!(run.json()["written"], false, "{what}");
        } else {
            assert!(
                run.stdout.starts_with(&format!("{expected} {index}: ")),
                "{what}: {}",
                run.stdout
            );
        }
        assert_eq!(run.stderr, "", "{what}");
    }
    fs::remove_file(root.join(index)).unwrap();
    let run = spec(&home, &root, &["--json", "export", "index"]);
    run.code(0);
    assert_eq!(run.json()["written"], true);
    let run = spec(&home, &root, &["export", "index", "--stdout"]);
    run.code(0);
    assert!(
        run.stdout.starts_with("---\nclass: generated\n"),
        "{}",
        run.stdout
    );
    assert_eq!(run.stderr, "");
    // `[paths] index` outside the walk: written, one `warning:`.
    write(
        &root,
        "specengine.toml",
        registered(&base, "docs/outside.md", "gen-index", None),
    );
    for json in [false, true] {
        let run = spec(&home, &root, &with_json(json, &["export", "index"]));
        run.code(0);
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{}", run.stderr);
        assert!(lines[0].starts_with("warning: "), "{}", run.stderr);
        assert_streams("export warning", &run, false);
        if json {
            run.json();
        }
    }
    assert!(root.join("docs/outside.md").is_file());

    // Config error: one `<config>:<line>:` line per cause; refusal: one
    // `spec:` line.
    let bogus = format!(
        "{}\n[bogus]\nx = 1\n",
        registered(&base, index, "gen-index", None)
    );
    write(&root, "specengine.toml", &bogus);
    for json in [false, true] {
        let run = spec(&home, &root, &with_json(json, &["export", "index"]));
        assert_eq!(run.code, 2, "{}", run.show());
        assert_eq!(run.stdout, "");
        assert!(
            run.stderr_lines()
                .iter()
                .all(|line| line.starts_with("specengine.toml:")),
            "{}",
            run.stderr
        );
        assert_streams("export config error", &run, false);
        let run = spec(
            &home,
            &root,
            &with_json(
                json,
                &["--config", "../outside/bogus.toml", "export", "index"],
            ),
        );
        assert_eq!(run.code, 2, "{}", run.show());
        assert!(
            run.stderr.starts_with("../outside/bogus.toml:"),
            "{}",
            run.stderr
        );
        assert_streams("export config error (--config)", &run, false);
    }
    write(&root, "specengine.toml", &base);
    for json in [false, true] {
        let run = spec(&home, &root, &with_json(json, &["export", "index"]));
        assert_eq!(run.code, 2, "{}", run.show());
        assert_eq!(run.stdout, "");
        assert_eq!(run.stderr_lines().len(), 1, "{}", run.stderr);
        assert!(run.stderr.starts_with("spec: "), "{}", run.stderr);
        assert_streams("export refusal", &run, false);
    }
    assert!(
        !data_dir(&home).exists(),
        "check and export touched the data directory"
    );
}

/// The whole stderr of `export index --stdout` with `--json`, in any order.
const STDOUT_JSON_CONFLICT: &str = "spec: the argument '--stdout' cannot be used with '--json'\n\nUsage: spec export index [OPTIONS]\n\nFor more information, try '--help'.\n";

/// Iteration 2 of docs/features/spec-cli-check.md (the review's major):
/// `--stdout` with `--json` is a usage error in every argument order,
/// `--json` first included, other globals interleaved, in a project whose
/// registry would otherwise answer (so no later refusal can mask it) and
/// outside any project: exit 2, nothing on stdout, the same stderr bytes,
/// nothing written.
#[test]
fn stdout_json_conflict_in_every_order() {
    use common::check::{index_path, registered};

    let scratch = Scratch::new("exit-conflict");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let base = fs::read_to_string(root.join("specengine.toml")).unwrap();
    write(
        &root,
        "specengine.toml",
        registered(&base, index_path("spec-a"), "gen-index", None),
    );
    // The registry answers: `--stdout` alone prints the render.
    let alone = spec(&home, &root, &["export", "index", "--stdout"]);
    alone.code(0);
    assert!(alone.stdout.starts_with("---\nclass: generated\n"));
    let before = common::snapshot(scratch.path());
    let empty = scratch.dir("empty");

    let orders: [&[&str]; 11] = [
        &["--json", "export", "index", "--stdout"],
        &["export", "--json", "index", "--stdout"],
        &["export", "index", "--json", "--stdout"],
        &["export", "index", "--stdout", "--json"],
        &["--root", ".", "--json", "export", "index", "--stdout"],
        &["--json", "--root", ".", "export", "index", "--stdout"],
        &["--json", "export", "--root", ".", "index", "--stdout"],
        &["export", "--json", "index", "--root", ".", "--stdout"],
        &["export", "index", "--stdout", "--root", ".", "--json"],
        &[
            "--config",
            "specengine.toml",
            "export",
            "index",
            "--json",
            "--stdout",
        ],
        &[
            "--json",
            "export",
            "index",
            "--stdout",
            "--config",
            "specengine.toml",
        ],
    ];
    for args in orders {
        for cwd in [&root, &empty] {
            let run = spec(&home, cwd, args);
            assert_eq!(run.code, 2, "{args:?} in {}\n{}", cwd.display(), run.show());
            assert_eq!(run.stdout, "", "{args:?}");
            assert_eq!(run.stderr, STDOUT_JSON_CONFLICT, "{args:?}");
        }
    }
    let empty_before = common::snapshot(&empty);
    assert!(empty_before.is_empty());
    let mut after = common::snapshot(scratch.path());
    after.remove("empty");
    let mut expected = before;
    expected.remove("empty");
    assert_eq!(after, expected, "a conflicting run wrote");
    assert!(!data_dir(&home).exists());
}
