//! AC-06 of docs/features/spec-cli.md: `spec init` writes exactly
//! `[project]\nslug = "<slug>"\n` with `create_new`, derives the slug from
//! the directory name, validates `--slug`, never walks, and writes nothing
//! else (no directory, `.gitignore`, database or `[ids]`).

#![cfg(unix)]

mod common;

use common::{Scratch, data_dir, read, snapshot, spec, write};

fn config_bytes(slug: &str) -> Vec<u8> {
    format!("[project]\nslug = \"{slug}\"\n").into_bytes()
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
