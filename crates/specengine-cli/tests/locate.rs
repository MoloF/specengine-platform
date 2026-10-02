//! The CLI library's public `locate` (task spec `mcp-read`, MCP
//! `resources/list`: "no project" is `locate` failing): it finds the root
//! and the config file without reading the config, so a config that is not
//! UTF-8 or not TOML is located and only `discover` refuses it, with the
//! line `spec` itself prints; it fails exactly where `discover` fails for
//! want of a project (an unusable `--root`, no `specengine.toml`), with the
//! same message.

#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, snapshot, spec, write};
use specengine_cli::{CONFIG_FILE, Env, Exit, Globals, Located, discover, locate};

fn env(cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: None,
        xdg_data_home: None,
    }
}

fn globals(root: Option<&Path>, config: Option<&Path>) -> Globals {
    Globals {
        root: root.map(Path::to_path_buf),
        config: config.map(Path::to_path_buf),
    }
}

/// A non-UTF-8, broken `specengine.toml` is located from the root, from a
/// subdirectory, by `--root` (absolute and relative) from elsewhere, and
/// with `--config` (the root canonical, the file as given); `discover`
/// refuses it with the line `spec tree` prints at exit 2. Both fixtures.
#[test]
fn locate_finds_the_project_without_reading_its_config() {
    let scratch = Scratch::new("locate");
    let home = scratch.home("h");
    let elsewhere = scratch.dir("elsewhere");
    for (fixture, below) in [
        ("spec-a", "docs/spec/movement"),
        ("spec-b", "docs/records/REQ"),
    ] {
        let dir = format!("{fixture}-latin1");
        let root = scratch.copy(fixture, &dir);
        write(&root, CONFIG_FILE, b"# caf\xe9\n[[[ broken\n");
        let want = Located {
            root: root.clone(),
            config_file: root.join(CONFIG_FILE),
            config_label: CONFIG_FILE.to_owned(),
        };
        for (cwd, flags) in [
            (root.clone(), globals(None, None)),
            (root.join(below), globals(None, None)),
            (elsewhere.clone(), globals(Some(&root), None)),
            (
                scratch.path().to_path_buf(),
                globals(Some(Path::new(&dir)), None),
            ),
        ] {
            let context = format!("{fixture} from {} {flags:?}", cwd.display());
            assert_eq!(
                locate(&env(&cwd), &flags).expect(&context),
                want,
                "{context}"
            );
            let error = discover(&env(&cwd), &flags).expect_err(&context);
            assert_eq!(error.exit, Exit::CannotRun, "{context}");
            assert_eq!(
                error.message, "spec: specengine.toml: the file is not UTF-8",
                "{context}"
            );
        }
        let run = spec(&home, &root.join(below), &["tree"]);
        run.code(2);
        assert_eq!(run.stdout, "", "{fixture}");
        assert_eq!(
            run.stderr, "spec: specengine.toml: the file is not UTF-8\n",
            "{fixture}"
        );
        // --config beside --root: the root canonical, the file as given.
        let config = scratch.join(&format!("{fixture}-outside.toml"));
        write(
            scratch.path(),
            &format!("{fixture}-outside.toml"),
            "[[[ broken\n",
        );
        let flags = globals(Some(Path::new(&dir)), Some(&config));
        let located = locate(&env(scratch.path()), &flags).expect("located");
        assert_eq!(
            located,
            Located {
                root: root.clone(),
                config_file: config.clone(),
                config_label: config.display().to_string(),
            },
            "{fixture}"
        );
        let error = discover(&env(scratch.path()), &flags).expect_err("discover");
        assert!(
            error
                .message
                .starts_with(&format!("{}:1: ", config.display())),
            "{fixture}: {}",
            error.message
        );
    }
    // --config naming a missing file, no --root: located at the current
    // directory; reading it is `discover`'s failure.
    let flags = globals(None, Some(Path::new("nope.toml")));
    assert_eq!(
        locate(&env(&elsewhere), &flags).expect("located"),
        Located {
            root: elsewhere.clone(),
            config_file: elsewhere.join("nope.toml"),
            config_label: "nope.toml".to_owned(),
        }
    );
    let error = discover(&env(&elsewhere), &flags).expect_err("discover");
    assert!(
        error.message.starts_with("spec: cannot read nope.toml: "),
        "{}",
        error.message
    );
    assert!(snapshot(&home).is_empty(), "nothing created under HOME");
}

/// Without a project `locate` fails, exit 2, with the message `discover`
/// gives and `spec tree` prints: an empty directory (naming `spec init`),
/// `--root` missing, a file, a directory without `specengine.toml` (an
/// ancestor's is not walked to), a `specengine.toml` that is a directory.
#[test]
fn locate_fails_where_no_project_is_found() {
    let scratch = Scratch::new("locate-none");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let empty = scratch.dir("nothing/below");
    write(scratch.path(), "a-file", "not a directory\n");
    let file = scratch.join("a-file");
    let missing = scratch.join("missing");
    let config_dir = scratch.dir("config-is-a-directory");
    std::fs::create_dir_all(config_dir.join(CONFIG_FILE)).expect("mkdir");
    let docs = root.join("docs");
    let cases: [(&str, PathBuf, Globals, &str); 5] = [
        (
            "an empty directory",
            empty.clone(),
            globals(None, None),
            "spec init",
        ),
        (
            "--root missing",
            root.clone(),
            globals(Some(&missing), None),
            "--root",
        ),
        (
            "--root a file",
            root.clone(),
            globals(Some(&file), None),
            "not a directory",
        ),
        (
            "--root without specengine.toml",
            root.clone(),
            globals(Some(&docs), None),
            "--root",
        ),
        (
            "specengine.toml a directory",
            config_dir.clone(),
            globals(None, None),
            "spec init",
        ),
    ];
    for (what, cwd, flags, needle) in cases {
        let error = locate(&env(&cwd), &flags).expect_err(what);
        assert_eq!(error.exit, Exit::CannotRun, "{what}");
        assert!(
            error.message.starts_with("spec: ") && error.message.contains(needle),
            "{what}: {}",
            error.message
        );
        let discovered = discover(&env(&cwd), &flags).expect_err(what);
        assert_eq!(discovered, error, "{what}");
        let mut args: Vec<String> = Vec::new();
        if let Some(root) = &flags.root {
            args.extend(["--root".to_owned(), root.display().to_string()]);
        }
        args.push("tree".to_owned());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = spec(&home, &cwd, &args);
        run.code(2);
        assert_eq!(run.stderr, format!("{}\n", error.message), "{what}");
    }
    assert!(snapshot(&home).is_empty(), "nothing created under HOME");
}
