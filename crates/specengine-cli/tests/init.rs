//! AC-06 of docs/features/spec-cli.md: `spec init` writes
//! `[project]\nslug = "<slug>"\n` and, since docs/features/flow-authoring.md,
//! the comments of its "`spec init`", with `create_new`, derives the slug from
//! the directory name, validates `--slug`, never walks, and writes nothing
//! else (no directory, `.gitignore`, database or active `[ids]`).
//!
//! docs/features/flow-authoring.md AC-04: a rerun exits 2
//! (`init_never_overwrites`); the parsed config equals the bare one; the
//! example with its `# ` removed parses, and reads; `genre.rs` passes. M: a
//! `docs/` literal (`genre.rs`); an active line.

#![cfg(unix)]

mod common;

use common::{Scratch, data_dir, read, read_text, snapshot, spec, write};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;

/// What follows the two lines, "`spec init`" verbatim: one prose comment, then
/// the example.
const SCAFFOLD: &str = "\n\
    # Commit this file: a proposal needs a commit, and the project's rules live here.\n\
    # [ids]\n\
    # NODE = { kind = \"node\", shape = \"name\" }\n\
    # [[flow.start]]\n\
    # paths = [\"docs/spec/first.md\"]\n\
    # [[flow.end]]\n\
    # paths = [\"docs/spec/last.md\"]\n";

/// The two lines `spec init` has always written.
fn bare(slug: &str) -> String {
    format!("[project]\nslug = \"{slug}\"\n")
}

fn config_bytes(slug: &str) -> Vec<u8> {
    format!("{}{SCAFFOLD}", bare(slug)).into_bytes()
}

#[test]
fn init_in_my_project_2_writes_the_data_bytes_then_index_answers() {
    let scratch = Scratch::new("init");
    let home = scratch.home("h");
    let dir = scratch.dir("My Project_2");
    let run = spec(&home, &dir, &["init"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "created specengine.toml with slug my-project-2\n"
    );
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(read(&dir, "specengine.toml"), config_bytes("my-project-2"));
    let entries: Vec<String> = snapshot(&dir).into_keys().collect();
    assert_eq!(entries, ["specengine.toml"], "init writes only its file");
    assert!(snapshot(&home).is_empty(), "init creates no data directory");

    // Then `index` answers on the empty project.
    let indexed = spec(&home, &dir, &["index"]);
    indexed.code(0);
    assert!(
        indexed.stdout.starts_with(
            "indexed my-project-2: walked 0, parsed 0, unchanged 0, removed 0, unreadable 0\n"
        ),
        "{}",
        indexed.show()
    );
    assert!(data_dir(&home).join("my-project-2.db").is_file());
}

#[test]
fn init_json_is_path_and_slug() {
    let scratch = Scratch::new("init-json");
    let home = scratch.home("h");
    let dir = scratch.dir("proj");
    let run = spec(&home, &dir, &["--json", "init"]);
    run.code(0);
    assert_eq!(
        run.json(),
        serde_json::json!({"path": "specengine.toml", "slug": "proj"})
    );
}

/// A rerun or a pre-existing file → exit 2, the bytes unchanged.
#[test]
fn init_never_overwrites() {
    let scratch = Scratch::new("init-again");
    let home = scratch.home("h");
    let dir = scratch.dir("again");
    spec(&home, &dir, &["init"]).code(0);
    let first = read(&dir, "specengine.toml");
    for args in [
        &["init"][..],
        &["init", "--slug", "other"],
        &["--json", "init"],
    ] {
        let run = spec(&home, &dir, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        assert!(run.stderr.starts_with("spec: "), "{}", run.show());
        assert_eq!(
            read(&dir, "specengine.toml"),
            first,
            "{args:?}: bytes unchanged"
        );
    }
    let other = scratch.dir("existing");
    let own = b"# the owner's own file\n[ids]\n".to_vec();
    write(&other, "specengine.toml", &own);
    let run = spec(&home, &other, &["init"]);
    run.code(2);
    assert_eq!(
        read(&other, "specengine.toml"),
        own,
        "a pre-existing file is kept"
    );
    assert_eq!(snapshot(&other).len(), 1);
    assert!(snapshot(&home).is_empty());
}

/// A directory name that gives no slug, and a bad `--slug`, are exit 2
/// naming `--slug`, with nothing written; `--slug x-1` is kept as given.
#[test]
fn init_validates_the_slug() {
    let scratch = Scratch::new("init-slug");
    let home = scratch.home("h");
    // "123" starts with a digit; the Cyrillic name has no ASCII letter or
    // digit; "---" trims to empty; 65 ASCII letters are too long.
    let long = "a".repeat(65);
    for name in [
        "123",
        "\u{041f}\u{0440}\u{043e}\u{0435}\u{043a}\u{0442}",
        "---",
        long.as_str(),
    ] {
        let dir = scratch.dir(name);
        let run = spec(&home, &dir, &["init"]);
        run.code(2);
        assert_eq!(run.stdout, "", "{name}");
        assert!(
            run.stderr.starts_with("spec: ") && run.stderr.contains("--slug"),
            "{name}: {}",
            run.show()
        );
        assert!(snapshot(&dir).is_empty(), "{name}: nothing written");
    }
    let dir = scratch.dir("123");
    let run = spec(&home, &dir, &["init", "--slug", "x-1"]);
    run.code(0);
    assert_eq!(run.stdout, "created specengine.toml with slug x-1\n");
    assert_eq!(read(&dir, "specengine.toml"), config_bytes("x-1"));

    for bad in ["Bad", "1x", "a/b", "../x", "", "a_b", "\u{0430}-1"] {
        let dir = scratch.dir("fine-name");
        let run = spec(&home, &dir, &["init", "--slug", bad]);
        run.code(2);
        assert_eq!(run.stdout, "", "--slug {bad:?}");
        assert!(
            run.stderr.starts_with("spec: ") && run.stderr.contains("--slug"),
            "--slug {bad:?}: {}",
            run.show()
        );
        assert!(snapshot(&dir).is_empty(), "--slug {bad:?}: nothing written");
    }
    assert!(snapshot(&home).is_empty());
}

/// A non-UTF-8 directory name: every run of other bytes becomes `-`.
#[test]
fn init_derives_a_slug_from_a_non_utf8_name() {
    use std::os::unix::ffi::OsStrExt as _;
    // The derivation itself, through the library (APFS refuses such names).
    assert_eq!(specengine_cli::derive_slug(b"Ab\xff\xfeCd 9"), "ab-cd-9");
    assert_eq!(specengine_cli::derive_slug(b"My Project_2"), "my-project-2");
    assert_eq!(specengine_cli::derive_slug(b"__x__"), "x");
    assert_eq!(specengine_cli::derive_slug(b"\xff"), "");
    let scratch = Scratch::new("init-bytes");
    let home = scratch.home("h");
    let name = std::ffi::OsStr::from_bytes(b"Ab\xff\xfeCd 9");
    let dir = scratch.path().join(name);
    if std::fs::create_dir(&dir).is_err() {
        // A file system that refuses non-UTF-8 names (APFS does): the
        // library assertions above stand for the spawned run.
        eprintln!("the file system refuses non-UTF-8 names; skipped");
        return;
    }
    let run = spec(&home, &dir, &["init"]);
    run.code(0);
    assert_eq!(read(&dir, "specengine.toml"), config_bytes("ab-cd-9"));
}

/// `init` never walks: under a configured project it creates its own file
/// with one `warning:`; `--config` is refused; `--root` names the
/// directory, and a `--root` that is a file is refused.
#[test]
fn init_does_not_walk_and_takes_no_config() {
    let scratch = Scratch::new("init-walk");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let below = root.join("docs/spec");
    let before = read(&root, "specengine.toml");
    let run = spec(&home, &below, &["init", "--slug", "inner"]);
    run.code(0);
    assert_eq!(read(&below, "specengine.toml"), config_bytes("inner"));
    assert_eq!(
        read(&root, "specengine.toml"),
        before,
        "the ancestor's config untouched"
    );
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{}", run.show());
    assert!(
        lines[0].starts_with("warning: ") && lines[0].contains("specengine.toml"),
        "{}",
        run.show()
    );

    let elsewhere = scratch.dir("elsewhere");
    let target = scratch.dir("target");
    let config = root.join("specengine.toml");
    let refused = spec(
        &home,
        &elsewhere,
        &["--config", config.to_str().unwrap(), "init"],
    );
    refused.code(2);
    assert_eq!(refused.stdout, "");
    assert!(refused.stderr.contains("--config"), "{}", refused.show());
    assert!(snapshot(&elsewhere).is_empty());

    let rooted = spec(
        &home,
        &elsewhere,
        &["--root", target.to_str().unwrap(), "init"],
    );
    rooted.code(0);
    assert_eq!(read(&target, "specengine.toml"), config_bytes("target"));
    assert!(
        snapshot(&elsewhere).is_empty(),
        "--root: nothing in the current directory"
    );

    let file = root.join("specengine.toml");
    let over_file = spec(
        &home,
        &elsewhere,
        &["--root", file.to_str().unwrap(), "init"],
    );
    over_file.code(2);
    assert_eq!(over_file.stdout, "");
    assert_eq!(read(&root, "specengine.toml"), before);
    assert!(
        snapshot(&home).is_empty(),
        "init never creates the data directory"
    );
}

/// Iteration 2, item (6): a write that fails after the exclusive create
/// leaves no partial `specengine.toml`. The failure is injected in the
/// spawned `spec` only: its file-size limit is 0 (`RLIMIT_FSIZE`) and
/// `SIGXFSZ` is ignored, so the create succeeds and the first write fails
/// with `EFBIG`.
#[test]
fn a_failed_write_leaves_no_partial_config() {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};

    #[repr(C)]
    struct RLimit {
        cur: u64,
        max: u64,
    }
    // RLIMIT_FSIZE, SIGXFSZ and SIG_IGN have these values on macOS and Linux.
    const RLIMIT_FSIZE: i32 = 1;
    const SIGXFSZ: i32 = 25;
    const SIG_IGN: usize = 1;
    unsafe extern "C" {
        fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
        fn signal(signal: i32, handler: usize) -> usize;
    }

    let scratch = Scratch::new("init-full");
    let home = scratch.home("h");
    let dir = scratch.dir("limited");
    let mut command = Command::new(common::SPEC);
    command
        .env_clear()
        .env("HOME", &home)
        .current_dir(&dir)
        .arg("init")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        command.pre_exec(|| {
            let limit = RLimit { cur: 0, max: 0 };
            if setrlimit(RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            signal(SIGXFSZ, SIG_IGN);
            Ok(())
        });
    }
    let output = command.output().expect("spawn spec");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(output.stdout.is_empty());
    assert!(
        stderr.starts_with("spec: ") && stderr.contains("specengine.toml"),
        "{stderr}"
    );
    assert!(
        snapshot(&dir).is_empty(),
        "a partial file was left: {:?}",
        snapshot(&dir).keys().collect::<Vec<_>>()
    );
    // Without the limit, the rerun is not refused.
    let run = spec(&home, &dir, &["init"]);
    run.code(0);
    assert_eq!(read(&dir, "specengine.toml"), config_bytes("limited"));
}

/// docs/features/flow-authoring.md AC-04: the scaffold's comments parse to
/// nothing: the whole table and `ProjectConfig` equal the bare file's, and a
/// project reads the same bytes with either. M: an active line.
#[test]
fn flow_authoring_ac04_the_parsed_config_equals_the_bare_one() {
    let scratch = Scratch::new("init-parsed");
    let home = scratch.home("h");
    let written = scratch.dir("scaffolded");
    let run = spec(&home, &written, &["init", "--slug", "same"]);
    run.code(0);
    let text = read_text(&written, "specengine.toml");
    let plain = bare("same");
    assert_ne!(text, plain, "the comments are there");
    assert_eq!(
        toml::from_str::<toml::Table>(&text).expect("the scaffold parses"),
        toml::from_str::<toml::Table>(&plain).expect("the bare file parses"),
        "no table, no key"
    );
    assert_eq!(
        ProjectConfig::from_toml(&text).expect("the scaffold reads"),
        ProjectConfig::from_toml(&plain).expect("the bare file reads"),
    );

    let other_home = scratch.home("other");
    let plain_root = scratch.dir("bare");
    write(&plain_root, "specengine.toml", &plain);
    for args in [
        &["tree"][..],
        &["flow"],
        &["flow", "--json"],
        &["check", "--json"],
    ] {
        let (one, other) = (
            spec(&home, &written, args),
            spec(&other_home, &plain_root, args),
        );
        assert_eq!(
            (one.code, one.stdout, one.stderr),
            (other.code, other.stdout, other.stderr),
            "{args:?}"
        );
    }
}

/// The scaffold's example: its lines with their `# ` removed, the prose line
/// above it, which is no TOML, left out; an active line is a problem.
fn uncommented(scaffold: &str) -> Result<String, String> {
    let mut example = String::new();
    let mut lines = scaffold.lines();
    if lines.next() != Some("") {
        return Err(format!("no blank line first: {scaffold}"));
    }
    let prose = lines.next().unwrap_or_default();
    if !prose.starts_with("# Commit this file: ") {
        return Err(format!("no prose line: {prose}"));
    }
    for line in lines {
        let active = line
            .strip_prefix("# ")
            .ok_or_else(|| format!("an active line: {line:?}"))?;
        example.push_str(active);
        example.push('\n');
    }
    Ok(example)
}

/// docs/features/flow-authoring.md AC-04: the example with its `# ` removed
/// parses, and reads: on a project of its two files the start and end rows
/// are measured at 0, `[[flow.definition]]` alone left unconfigured, and the
/// check is clean.
#[test]
fn flow_authoring_ac04_the_example_uncommented_parses_and_reads() {
    let scratch = Scratch::new("init-example");
    let home = scratch.home("h");
    let root = scratch.dir("project");
    spec(&home, &root, &["init"]).code(0);
    let written = read_text(&root, "specengine.toml");
    let scaffold = written
        .strip_prefix(&bare("project"))
        .expect("the two lines first");
    let example = uncommented(scaffold).expect("the scaffold's example");
    let text = format!("{}{example}", bare("project"));

    let table = toml::from_str::<toml::Table>(&text).expect("the example parses");
    assert!(table["ids"].get("NODE").is_some(), "{table:?}");
    assert!(table["flow"].get("start").is_some(), "{table:?}");
    assert!(table["flow"].get("end").is_some(), "{table:?}");
    let config = ProjectConfig::from_toml(&text).expect("the example reads");
    assert_eq!(config.project.slug.as_deref(), Some("project"));

    write(&root, "specengine.toml", &text);
    let document = |id: &str, links: &str| {
        format!(
            "---\nid: {id}\nclass: canon\nowner: owner\nreviewed: 2026-01-01\n{links}---\n\n# \
             A step\n"
        )
    };
    write(&root, "docs/spec/first.md", document("NODE-first", ""));
    write(
        &root,
        "docs/spec/last.md",
        document("NODE-last", "links:\n  follows: [NODE-first]\n"),
    );
    let check = spec(&home, &root, &["check", "--json"]);
    check.code(0);
    assert_eq!(
        check.json()["counts"]["warnings"],
        json!(0),
        "{}",
        check.show()
    );
    assert_eq!(
        check.json()["counts"]["errors"],
        json!(0),
        "{}",
        check.show()
    );

    let flow = spec(&home, &root, &["flow", "--json"]);
    flow.code(0);
    let flow = flow.json();
    for gap in flow["gaps"].as_array().expect("gaps") {
        let code = gap["code"].as_str().expect("a code");
        if code.starts_with("definition-") {
            assert_eq!(gap["state"], "not_configured", "{gap}");
        } else {
            assert_eq!(gap["state"], "measured", "{gap}");
            assert_eq!(gap["count"], json!(0), "{gap}");
            assert_eq!(gap["cause"], Value::Null, "{gap}");
        }
    }
}

/// M: an active line: the reader of the test above names it, whatever the
/// line, so the example cannot pass by skipping lines.
#[test]
fn flow_authoring_ac04_an_active_line_is_named() {
    let active = SCAFFOLD.replacen("# [ids]\n", "[ids]\n", 1);
    let problem = uncommented(&active).expect_err("the reader skipped an active line");
    assert!(problem.contains("[ids]"), "{problem}");
    assert_eq!(
        uncommented(SCAFFOLD).expect("the scaffold").lines().next(),
        Some("[ids]"),
        "the prose line left out"
    );
}
