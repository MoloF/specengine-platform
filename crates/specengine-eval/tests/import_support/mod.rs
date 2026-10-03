//! Plumbing shared by `import_cli.rs` and `import_genre.rs`
//! (docs/features/import-records.md): the two invented fixtures
//! `fixtures/import-one` and `fixtures/import-two`, scratch directories, the
//! real `specengine-eval` binary, and the test-generated corpus of AC-05 with
//! its config (non-Latin text built from `\u{...}` escapes at test time, A4).
//!
//! Nothing here names a pilot.

// Each test file uses its own subset.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

pub const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

/// The two invented conventions.
pub const FIXTURES: [&str; 2] = ["import-one", "import-two"];

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

pub fn fixture_dir(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

/// The import config of a fixture: `census.toml` at its root.
pub fn fixture_config(name: &str) -> PathBuf {
    fixture_dir(name).join("census.toml")
}

/// A scratch directory under the system temp dir, removed on drop; unique
/// per process and per call, so tests sharing a name never share a directory
/// (`cargo test` runs them as threads of one process).
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-import-{name}-{}-{serial}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The binary with no pilot variable inherited, run from the repository root.
pub fn eval() -> Command {
    let mut command = Command::new(BIN);
    command.current_dir(repository_root());
    for variable in [
        "SPECENGINE_PILOT_A",
        "SPECENGINE_PILOT_B",
        "SPECENGINE_CENSUS_CONFIG_A",
        "SPECENGINE_CENSUS_CONFIG_B",
        "SPECENGINE_SCHEME_A",
        "SPECENGINE_SCHEME_B",
    ] {
        command.env_remove(variable);
    }
    command
}

pub fn run(args: &[&str]) -> Output {
    eval().args(args).output().expect("specengine-eval runs")
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The one JSON line on stdout of a successful run.
pub fn envelope(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "exit {:?}, stderr:\n{}",
        output.status.code(),
        stderr(output)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    let mut lines = stdout.lines();
    let line = lines.next().expect("one JSON line on stdout");
    assert!(
        lines.next().is_none(),
        "stdout must hold exactly one line:\n{stdout}"
    );
    serde_json::from_str(line).unwrap_or_else(|error| panic!("stdout is not JSON: {error}\n{line}"))
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
}

/// `value` without every object key ending in `_ms` (timings).
pub fn without_ms(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| !key.ends_with("_ms"))
                .map(|(key, value)| (key.clone(), without_ms(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Byte-for-byte snapshot of a directory tree.
pub fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(&path).expect("readable file"),
                );
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target directory");
    for entry in fs::read_dir(from).expect("readable") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy");
        }
    }
}

pub fn git_status(dir: &Path, pathspec: &str) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--",
            pathspec,
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every line of `git status --porcelain -- fixtures/` names one of the
/// `fixtures/import-*` fixtures this task adds (untracked until the owner
/// commits them), nothing else.
pub fn assert_fixtures_status_clean() {
    let status = git_status(&repository_root(), "fixtures/");
    let foreign: Vec<&str> = status
        .lines()
        .filter(|line| {
            let path = line.get(3..).unwrap_or("");
            !path.starts_with("fixtures/import-")
        })
        .collect();
    assert!(
        foreign.is_empty(),
        "git status -- fixtures/ shows more than fixtures/import-*:\n{}",
        foreign.join("\n")
    );
}

/// One run of `measurement` on `corpus` (a fixture name under `fixtures/` or
/// an absolute path), `--out` in `out`; returns the envelope.
pub fn measure(measurement: &str, corpus: &Path, out: &Path, extra: &[&str]) -> Value {
    let mut args = vec![
        measurement,
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    envelope(&run(&args))
}

/// `import` on a corpus; `--out` under `scratch`/`name`, label `pilot`.
pub struct ImportRun {
    pub result: Value,
    pub detail: PathBuf,
}

impl ImportRun {
    pub fn new(corpus: &Path, scratch: &Scratch, name: &str, extra: &[&str]) -> Self {
        let out = scratch.join(name);
        let envelope = measure("import", corpus, &out, extra);
        assert_eq!(envelope["measurement"], "import");
        Self {
            result: envelope["result"].clone(),
            detail: out.join("import").join("pilot"),
        }
    }

    pub fn file(&self, name: &str) -> Value {
        read_json(&self.detail.join(name))
    }

    pub fn records(&self) -> Vec<Value> {
        self.file("records.json")
            .as_array()
            .expect("records.json is an array")
            .clone()
    }

    /// The count at a `/`-separated path of `result`.
    pub fn count(&self, path: &str) -> u64 {
        let mut value = &self.result;
        for part in path.split('/') {
            value = &value[part];
        }
        value
            .as_u64()
            .unwrap_or_else(|| panic!("result/{path} is not a count: {value}"))
    }
}

// ------------------------------------------------------- AC-05 generated copy

/// A Cyrillic legacy prefix with a letter that has no Latin look-alike
/// (Te, Er, Be), mapped to `REQ` in the generated config.
pub fn legacy_prefix() -> String {
    "\u{0422}\u{0420}\u{0411}".to_owned()
}

/// A Cyrillic prefix whose letters are all Latin look-alikes (Te, Ie, Es):
/// normalising it would give `TEC`, which `ids.regex` accepts; it is mapped
/// nowhere, so it must be counted `legacy.unmapped`, never guessed.
pub fn look_alike_prefix() -> String {
    "\u{0422}\u{0415}\u{0421}".to_owned()
}

/// A Cyrillic prefix with no look-alike first letter (Pe, Er, Be), mapped
/// nowhere.
pub fn foreign_prefix() -> String {
    "\u{041F}\u{0420}\u{0411}".to_owned()
}

/// A Cyrillic front-matter key (the word "status"), mapped to `status`.
pub fn non_latin_key() -> String {
    "\u{0421}\u{0442}\u{0430}\u{0442}\u{0443}\u{0441}".to_owned()
}

/// The generated config: `fixtures/import-one/census.toml` plus a Cyrillic
/// legacy prefix and a Cyrillic key-map key.
pub fn generated_config_text() -> String {
    let base = fs::read_to_string(fixture_config("import-one")).expect("import-one config");
    let legacy_line = "\"SR\" = \"REQ\"";
    let key_line = "\"Phase\" = \"status\"";
    assert!(
        base.contains(legacy_line) && base.contains(key_line),
        "{base}"
    );
    base.replace(
        legacy_line,
        &format!("{legacy_line}\n\"{}\" = \"REQ\"", legacy_prefix()),
    )
    .replace(
        key_line,
        &format!("{key_line}\n\"{}\" = \"status\"", non_latin_key()),
    )
}

/// The document the generated copy adds to `spec/`, and its lines of interest.
pub fn generated_document() -> String {
    format!(
        "---\nkind: register\n{key}: Done\n---\n# Legacy\n\n| ID | Statement |\n|---|---|\n\
         | {legacy}-031 | A requirement under the Cyrillic legacy prefix. |\n\
         | {look}-032 | A prefix of look-alikes only, mapped nowhere. |\n\
         | {foreign}-033 | A prefix with no look-alike, mapped nowhere. |\n\
         | QP203 | A hyphenless code in the ID cell. |\n\n\
         **QP201:** A hyphenless code defined at a paragraph start.\n\n\
         - **QP204:** A hyphenless code defined in a list item.\n\n\
         QP202 is a mention, and so is QP201.\n",
        key = non_latin_key(),
        legacy = legacy_prefix(),
        look = look_alike_prefix(),
        foreign = foreign_prefix(),
    )
}

/// A scratch copy of `fixtures/import-one` with the generated document and
/// config; returns (corpus, config).
pub fn generated_copy(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let corpus = scratch.join("generated");
    copy_dir(&fixture_dir("import-one"), &corpus);
    fs::write(corpus.join("spec/legacy.md"), generated_document()).unwrap();
    let config = scratch.join("generated.toml");
    fs::write(&config, generated_config_text()).unwrap();
    (corpus, config)
}

// ---------------------------------------------------------- config values

/// Every string of a config that names its convention: the string values at
/// any depth and the keys of the map tables (`key_map`, `value_map` and its
/// tables, `ids.legacy`).
pub fn convention_strings(config_text: &str) -> BTreeSet<String> {
    let table: toml::Table = toml::from_str(config_text).expect("config is TOML");
    let mut out = BTreeSet::new();
    fn walk(value: &toml::Value, map_keys: bool, out: &mut BTreeSet<String>) {
        match value {
            toml::Value::String(text) => {
                out.insert(text.clone());
            }
            toml::Value::Array(items) => {
                for item in items {
                    walk(item, false, out);
                }
            }
            toml::Value::Table(table) => {
                for (key, value) in table {
                    if map_keys {
                        out.insert(key.clone());
                    }
                    let nested_map = matches!(key.as_str(), "key_map" | "value_map" | "legacy")
                        || (map_keys && value.is_table());
                    walk(value, nested_map, out);
                }
            }
            _ => {}
        }
    }
    walk(&toml::Value::Table(table), false, &mut out);
    out
}
