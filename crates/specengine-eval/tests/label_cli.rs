//! `--label` names the detail directory `<out>/<measurement>/<label>`, so every
//! subcommand refuses a label that is not exactly one plain path component
//! (`crates/specengine-eval/README.md`, "CLI contract": exit 2 = refused,
//! nothing written) — before resolving the corpus, reading a config or
//! creating `--out`.
//!
//! `ra` exists only with feature `ra`; its cases run under
//! `cargo nextest run -p specengine-eval --features ra --test label_cli`.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

/// Every spelling that is not one plain component: parent escapes, absolute,
/// the current and parent directory, a nested path, a trailing separator, empty.
const BAD_LABELS: [&str; 7] = ["../../p", "/abs", ".", "..", "a/b", "a/", ""];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// `(subcommand, fixture directory under fixtures/)` for every measurement.
fn subcommands() -> Vec<(&'static str, &'static str)> {
    #[allow(unused_mut)]
    let mut list = vec![
        ("ast-hash", "ast-hash"),
        ("ron", "ron"),
        ("census", "corpus-mini"),
        ("parse", "corpus-mini"),
        ("index", "spec-b"),
        ("bevy-detector", "bevy-mini"),
        ("bevy", "bevy-mini"),
    ];
    #[cfg(feature = "ra")]
    list.push(("ra", "ra-mini"));
    list
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-label-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(fs::canonicalize(&path).expect("canonical scratch directory"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every entry under `root` with its bytes (directories as `None`).
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut entries = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable directory") {
            let path = entry.expect("directory entry").path();
            let relative = path.strip_prefix(root).expect("under root").to_path_buf();
            if path.is_dir() {
                entries.insert(relative, None);
                stack.push(path);
            } else {
                entries.insert(relative, Some(fs::read(&path).expect("readable file")));
            }
        }
    }
    entries
}

fn copy_tree(from: &Path, to: &Path) {
    for (relative, bytes) in snapshot(from) {
        let target = to.join(&relative);
        match bytes {
            None => fs::create_dir_all(&target).expect("copy dir"),
            Some(bytes) => {
                fs::create_dir_all(target.parent().expect("parent")).expect("copy parent");
                fs::write(&target, bytes).expect("copy file");
            }
        }
    }
}

fn run(args: &[&str]) -> Output {
    let mut command = Command::new(BIN);
    command.current_dir(repository_root());
    for variable in ["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"] {
        command.env_remove(variable);
    }
    command.args(args).output().expect("specengine-eval runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn every_subcommand_refuses_a_label_that_is_not_one_plain_component() {
    let scratch = Scratch::new("refusal");
    for (subcommand, fixture) in subcommands() {
        // A copy of the measurement's own fixture: with an accepted label the
        // run would succeed, so the refusal can only come from `--label`.
        let corpus = scratch.0.join("corpus").join(subcommand);
        copy_tree(&repository_root().join("fixtures").join(fixture), &corpus);
        // `--out` two levels deep, so `../../p` from `<out>/<measurement>`
        // would still land inside the scratch directory.
        let out = scratch.0.join("deep").join("er").join(subcommand);
        let out_arg = out.to_str().expect("UTF-8 out");
        let corpus_arg = corpus.to_str().expect("UTF-8 corpus");
        let before = snapshot(&scratch.0);
        for label in BAD_LABELS {
            for with_pilot in [true, false] {
                let mut args = vec![subcommand, "--out", out_arg, "--timeout", "30"];
                if with_pilot {
                    args.extend(["--pilot", corpus_arg]);
                }
                let label_arg = format!("--label={label}");
                args.push(&label_arg);
                let context = format!(
                    "{subcommand} --label {label:?} ({})",
                    if with_pilot {
                        "--pilot a copy of its fixture"
                    } else {
                        "default fixture"
                    }
                );
                let output = run(&args);
                assert_eq!(
                    output.status.code(),
                    Some(2),
                    "{context}: expected a refusal (exit 2), stdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    stderr(&output)
                );
                assert!(
                    output.stdout.is_empty(),
                    "{context}: a refusal prints nothing on stdout, got:\n{}",
                    String::from_utf8_lossy(&output.stdout)
                );
                assert!(
                    stderr(&output).contains("--label"),
                    "{context}: the refusal does not name --label:\n{}",
                    stderr(&output)
                );
                assert!(!out.exists(), "{context}: --out was created");
                assert_eq!(
                    snapshot(&scratch.0),
                    before,
                    "{context}: the scratch tree changed"
                );
                assert!(
                    !Path::new("/abs").exists(),
                    "{context}: /abs exists after the run"
                );
            }
        }
    }
}

/// Control: a plain component is accepted and names the detail directory, so
/// the refusals above are not a refusal of every label.
#[test]
fn a_plain_component_label_names_the_detail_directory() {
    let scratch = Scratch::new("accepted");
    let out = scratch.0.join("out");
    for label in ["run-1", "run.1", "fixtures"] {
        let output = run(&[
            "ron",
            "--out",
            out.to_str().expect("UTF-8 out"),
            "--label",
            label,
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "ron --label {label}: stderr:\n{}",
            stderr(&output)
        );
        let envelope: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope");
        assert_eq!(envelope["label"], label);
        assert!(
            out.join("ron").join(label).is_dir(),
            "ron --label {label}: no detail directory <out>/ron/{label}"
        );
    }
}
