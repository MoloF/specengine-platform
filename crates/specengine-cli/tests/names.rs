//! Iteration 3, item (4) of the spec-cli verification: a file name holding
//! a line break prints on one line in a `search` hit line, the `show`
//! header and the `show` tail line; JSON keeps the raw path. And the
//! Streams rule of docs/features/spec-cli.md ("Data"): every stderr line is
//! one of its forms, also when a message cites such a path.

#![cfg(unix)]

mod common;

use common::{Scratch, index, spec, write};

/// Two feature documents defining `{#AC-50}`, both named with a line
/// break; the first is long enough to be cut.
fn corpus(scratch: &Scratch) -> (std::path::PathBuf, std::path::PathBuf) {
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let mut big =
        String::from("---\nclass: spec\nstatus: draft\n---\n\n# Aa big\n\n### Big {#AC-50}\n\n");
    for k in 0..3000 {
        big.push_str(&format!("Filler line {k} zephyrquartz.\n"));
    }
    write(&root, "docs/features/two\nparts.md", &big);
    write(
        &root,
        "docs/features/zz\nsmall.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Zz\n\n### Small {#AC-50}\n\nShort.\n",
    );
    index(&home, &root);
    (home, root)
}

#[test]
fn a_newline_in_a_file_name_prints_on_one_line() {
    let scratch = Scratch::new("names");
    let (home, root) = corpus(&scratch);

    let run = spec(&home, &root, &["search", "zephyrquartz"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "one hit, two lines, the summary\n{}",
        run.show()
    );
    assert_eq!(
        lines[0],
        "AC-50 | criterion | Big | docs/features/two parts.md:8"
    );
    let json = spec(&home, &root, &["--json", "search", "zephyrquartz"]).json();
    assert_eq!(
        json["hits"][0]["path"], "docs/features/two\nparts.md",
        "JSON keeps the raw path"
    );

    // A document without an ID: its path names it in the header.
    let small = spec(&home, &root, &["show", "docs/features/zz\nsmall.md"]);
    small.code(0);
    assert!(
        small
            .stdout
            .starts_with("docs/features/zz small.md | - | Zz | docs/features/zz small.md:1 | "),
        "{}",
        small.show()
    );

    // Two holders, the first cut: the header and the tail each on one line.
    let run = spec(&home, &root, &["show", "AC-50"]);
    run.code(0);
    let first = run.stdout.lines().next().unwrap();
    assert!(
        first.starts_with("AC-50 | criterion | Big | docs/features/two parts.md:8 | "),
        "{first}"
    );
    let tail = run.stdout.lines().last().unwrap();
    assert!(
        tail.starts_with("[truncated: docs/features/two parts.md lines ")
            && tail.ends_with("; holders not shown: docs/features/zz small.md:8]"),
        "{tail}"
    );
    assert_eq!(run.stdout.matches("[truncated: ").count(), 1);
    let json = spec(&home, &root, &["--json", "show", "AC-50"]).json();
    assert_eq!(json["nodes"][0]["path"], "docs/features/two\nparts.md");
    assert_eq!(
        json["nodes"][0]["omitted"]["holders"],
        serde_json::json!(["docs/features/zz\nsmall.md:8"])
    );
}

/// Every stderr line keeps its form (`spec: `, `warning: `, `note: `) when
/// the message cites a path or a reference holding a line break: the
/// several-holders warning and an exit-1 reason.
#[test]
fn a_newline_in_a_cited_path_keeps_stderr_lines_whole() {
    let scratch = Scratch::new("names-stderr");
    let (home, root) = corpus(&scratch);
    for (args, exit) in [
        (&["show", "AC-50"][..], 0),
        (&["show", "docs/features/no\nsuch.md"], 1),
    ] {
        let run = spec(&home, &root, args);
        run.code(exit);
        for line in run.stderr_lines() {
            assert!(
                line.starts_with("spec: ")
                    || line.starts_with("warning: ")
                    || line.starts_with("note: "),
                "{args:?}: a stderr line of no known form: {line:?}\n{}",
                run.stderr
            );
        }
    }
}

/// Iteration 4, item (1): exit-2 messages quoting a path with a line break
/// (`--root`, `--config`, `HOME`, the data directory) print one stderr line,
/// in text and with `--json` (which prints nothing on stdout for exit 2).
#[test]
fn a_newline_in_an_exit_2_message_stays_on_one_line() {
    let scratch = Scratch::new("names-exit-2");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let elsewhere = scratch.dir("elsewhere");
    let missing_root = scratch.path().join("no\nroot");
    let bare_root = scratch.dir("has\nline");
    let bad_config = scratch.path().join("bad\nslug.toml");
    std::fs::write(&bad_config, "[project]\nslug = \"Bad\"\n").unwrap();
    let missing_config = scratch.path().join("missing\nfile.toml");
    let inside_home = root.join("in\nside");
    let blocked_home = scratch.home("blocked\nhome");
    let data = common::data_dir(&blocked_home);
    std::fs::create_dir_all(data.parent().unwrap()).unwrap();
    std::fs::write(&data, "a file").unwrap();
    let path = |p: &std::path::Path| p.to_str().unwrap().to_owned();
    let cases: Vec<(&str, std::path::PathBuf, Vec<String>, &str)> = vec![
        (
            "--root missing",
            home.clone(),
            vec!["--root".into(), path(&missing_root), "index".into()],
            "no root",
        ),
        (
            "--root without config",
            home.clone(),
            vec!["--root".into(), path(&bare_root), "index".into()],
            "spec init",
        ),
        (
            "--config bad slug",
            home.clone(),
            vec![
                "--root".into(),
                path(&root),
                "--config".into(),
                path(&bad_config),
                "index".into(),
            ],
            "bad slug.toml:2: ",
        ),
        (
            "--config missing",
            home.clone(),
            vec![
                "--root".into(),
                path(&root),
                "--config".into(),
                path(&missing_config),
                "show".into(),
                "R-12".into(),
            ],
            "missing file.toml",
        ),
        (
            "HOME inside the root",
            inside_home.clone(),
            vec![
                "--root".into(),
                path(&root),
                "search".into(),
                "stamina".into(),
            ],
            "in side",
        ),
        (
            "data directory blocked",
            blocked_home.clone(),
            vec!["--root".into(), path(&root), "index".into()],
            "blocked home",
        ),
        (
            "init --root missing",
            home.clone(),
            vec!["--root".into(), path(&missing_root), "init".into()],
            "no root",
        ),
    ];
    for (what, home, args, needle) in cases {
        for json in [false, true] {
            let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
            if json {
                args.insert(0, "--json");
            }
            let run = spec(&home, &elsewhere, &args);
            run.code(2);
            assert_eq!(run.stdout, "", "{what}");
            assert!(run.stderr.ends_with('\n'), "{what}");
            let lines = run.stderr_lines();
            assert_eq!(lines.len(), 1, "{what}: one stderr line\n{}", run.stderr);
            assert!(
                lines[0].starts_with("spec: ") || lines[0].contains(".toml:2: "),
                "{what}: {}",
                lines[0]
            );
            assert!(
                lines[0].contains(needle),
                "{what}: want {needle:?} in {}",
                lines[0]
            );
        }
    }
    assert!(!inside_home.exists(), "nothing created under the root");
}

/// Exit 1 with a line break in `REF`: JSON `reason` is flattened and equals
/// the stderr line; JSON `ref` stays as given.
#[test]
fn a_newline_in_an_exit_1_reference_json_ref_raw_reason_flat() {
    let scratch = Scratch::new("names-exit-1");
    let (home, root) = corpus(&scratch);
    for written in ["docs/features/no\nsuch.md", "R-12\nR-13"] {
        let run = spec(&home, &root, &["--json", "show", written]);
        run.code(1);
        let json = run.json();
        assert_eq!(json["ref"], written, "ref as given");
        let reason = json["reason"].as_str().unwrap();
        assert!(!reason.contains('\n'), "{reason:?}");
        assert_eq!(run.stderr, format!("spec: {reason}\n"));
        let text = spec(&home, &root, &["show", written]);
        text.code(1);
        assert_eq!(text.stderr, run.stderr, "{written:?}");
    }
    // JSON `notes` equal the stderr note lines (the several-holders
    // warning is stderr only).
    let run = spec(&home, &root, &["--json", "search", "a", "zephyrquartz"]);
    run.code(0);
    let json = run.json();
    let notes = json["notes"].as_array().unwrap();
    let lines: Vec<&str> = run
        .stderr_lines()
        .into_iter()
        .filter(|line| line.starts_with("note: "))
        .collect();
    assert!(
        !notes.is_empty() && notes.len() == lines.len(),
        "{json}\n{}",
        run.stderr
    );
    for (note, line) in notes.iter().zip(lines) {
        assert_eq!(format!("note: {}", note.as_str().unwrap()), line);
    }
}
