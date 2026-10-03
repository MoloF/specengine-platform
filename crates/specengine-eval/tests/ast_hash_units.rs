//! `specengine-eval ast-hash` and Cargo target units end to end
//! (docs/features/layer-a-identity.md AC-01 rule 3 without targets, AC-02
//! `fixtures/cargo-units` with `cargo metadata`, shared items and their
//! flags, AC-03 `fixtures/ast-hash` unchanged, AC-04 the layout fallback and
//! a hanging cargo killed, AC-05 the cargo call itself, its working
//! directory, its scrubbed failure line and a read-only fixture run).
//!
//! Every run of `fixtures/cargo-units` but one works on a scratch copy; the
//! one in-place run proves nothing is written into the fixture. A stub
//! `SPECENGINE_CARGO` logs its argv and working directory, then writes to
//! stderr, fails, hangs or execs the real cargo. `HOME` is whatever the test
//! process has (the owner's runs set a scratch `HOME`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

/// The cargo that built this test; a stub execs it.
const REAL_CARGO: &str = env!("CARGO");

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn units_fixture() -> PathBuf {
    repository_root().join("fixtures").join("cargo-units")
}

fn expected() -> Value {
    let path = units_fixture().join("expected.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("expected.json readable"))
        .expect("expected.json is JSON")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        Self::under(
            &std::env::temp_dir(),
            &format!("specengine-eval-units-{name}-{}", std::process::id()),
        )
    }

    /// A scratch directory under `/tmp`: a corpus root short enough that a
    /// failure line holding it several times stays under the 200-char cut.
    fn short(name: &str) -> Self {
        Self::under(
            Path::new("/tmp"),
            &format!("se-{name}-{}", std::process::id()),
        )
    }

    fn under(base: &Path, name: &str) -> Self {
        let path = base.join(name);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn join(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    /// A fresh copy of `fixtures/cargo-units` at `<scratch>/corpus`.
    fn corpus(&self) -> PathBuf {
        let corpus = self.join("corpus");
        copy_dir(&units_fixture(), &corpus);
        corpus
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
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

/// Byte-for-byte snapshot of a directory tree.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
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

/// No `Cargo.lock` and no `target*` directory anywhere under `dir`.
fn assert_no_cargo_leftovers(dir: &Path) {
    for path in snapshot(dir).keys() {
        assert!(
            path.file_name().is_none_or(|n| n != "Cargo.lock"),
            "a Cargo.lock appeared: {}",
            path.display()
        );
        assert!(
            !path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with("target")),
            "a build directory appeared: {}",
            path.display()
        );
    }
}

fn git_status_of_fixtures() -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--",
            "fixtures/",
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `ast-hash` over `corpus` (or the default fixture), detail under `out`.
fn ast_hash(corpus: Option<&Path>, out: &Path, cwd: &Path, cargo: Option<&Path>) -> Output {
    ast_hash_with(corpus, out, cwd, cargo, &[])
}

/// [`ast_hash`] with further arguments (`--timeout`).
fn ast_hash_with(
    corpus: Option<&Path>,
    out: &Path,
    cwd: &Path,
    cargo: Option<&Path>,
    more: &[&str],
) -> Output {
    let mut command = Command::new(BIN);
    command
        .current_dir(cwd)
        .env_remove("SPECENGINE_PILOT_A")
        .env_remove("SPECENGINE_PILOT_B")
        .env_remove("SPECENGINE_CARGO");
    if let Some(cargo) = cargo {
        command.env("SPECENGINE_CARGO", cargo);
    }
    command.arg("ast-hash");
    if let Some(corpus) = corpus {
        command.arg("--pilot").arg(corpus);
    }
    command
        .arg("--out")
        .arg(out)
        .args(more)
        .output()
        .expect("specengine-eval runs")
}

/// Exit 0 and exactly one JSON line on stdout; its `result`.
fn result_of(output: &Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    let mut lines = stdout.lines();
    let line = lines.next().expect("one JSON line on stdout");
    assert!(
        lines.next().is_none(),
        "stdout must hold exactly one line:\n{stdout}"
    );
    let envelope: Value = serde_json::from_str(line)
        .unwrap_or_else(|error| panic!("stdout is not JSON: {error}\n{line}"));
    envelope["result"].clone()
}

fn manifest(out: &Path, label: &str) -> Value {
    let path = out.join("ast-hash").join(label).join("manifest.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("manifest.json")).expect("JSON")
}

/// The manifest in the shape of `expected.json`'s `manifest`.
fn compact(manifest: &Value) -> Vec<Value> {
    manifest["files"]
        .as_array()
        .expect("files")
        .iter()
        .map(|file| {
            let items: Vec<Value> = file["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|item| json!([item["qpath"], item["ambiguity"]]))
                .collect();
            json!({
                "path": file["path"],
                "package": file["package"],
                "role": file["role"],
                "unit": file["unit"],
                "targets": file["targets"],
                "items": items,
            })
        })
        .collect()
}

/// Every name that must never reach stdout: target and package names,
/// units, file names.
fn assert_stdout_has_no_names(output: &Output, corpus: &Path) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    for leak in [
        ".rs",
        "Cargo.toml",
        "app-cli",
        "unit-app",
        "unit_app",
        "binonly",
        "build-script-build",
        "bin:",
        "shared:",
        "tests/common",
        "STUB",
    ] {
        assert!(
            !stdout.contains(leak),
            "{leak:?} leaked to stdout: {stdout}"
        );
    }
    assert!(
        !stdout.contains(corpus.to_str().unwrap()),
        "the corpus path leaked to stdout"
    );
}

/// A `SPECENGINE_CARGO` stand-in: appends its argv (one per line, then
/// `--`) to `log`, writes `stderr_line` to stderr, then execs the real cargo
/// or exits with `fail`.
fn stub(scratch: &Scratch, log: &Path, stderr_line: &str, fail: Option<u8>) -> PathBuf {
    let tail = match fail {
        Some(code) => format!("exit {code}"),
        None => format!("exec '{REAL_CARGO}' \"$@\""),
    };
    stub_with(scratch, log, &format!("echo '{stderr_line}' >&2\n{tail}"))
}

/// Where a stub logs its physical working directory, one line per call.
fn cwd_log(log: &Path) -> PathBuf {
    log.with_extension("cwd")
}

/// A stub that logs its argv to `log` and its working directory to
/// [`cwd_log`], then runs the shell `body`.
fn stub_with(scratch: &Scratch, log: &Path, body: &str) -> PathBuf {
    let path = scratch.join("cargo-stub.sh");
    let script = format!(
        "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\" >> '{log}'; done\nprintf -- '--\\n' >> '{log}'\npwd -P >> '{cwd}'\n{body}\n",
        log = log.display(),
        cwd = cwd_log(log).display(),
    );
    fs::write(&path, script).expect("stub written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("stub executable");
    }
    path
}

/// The argv of every stub call, in order.
fn calls(log: &Path) -> Vec<Vec<String>> {
    let text = fs::read_to_string(log).unwrap_or_default();
    let mut calls = Vec::new();
    let mut current = Vec::new();
    for line in text.lines() {
        if line == "--" {
            calls.push(std::mem::take(&mut current));
        } else {
            current.push(line.to_owned());
        }
    }
    calls
}

fn metadata_argv(manifest: &Path) -> Vec<String> {
    [
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--offline",
        "--color",
        "never",
        "--manifest-path",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([manifest.to_string_lossy().into_owned()])
    .collect()
}

// ------------------------------------------------------------------ AC-02

#[test]
fn metadata_run_matches_expected_json() {
    let scratch = Scratch::new("metadata");
    let corpus = scratch.corpus();
    let before = snapshot(&corpus);
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, None);
    let result = result_of(&output);
    let expected = expected();
    for key in ["files", "items", "path_attrs"] {
        assert_eq!(result[key], expected[key], "result.{key}");
    }
    for key in ["qpath_ambiguity", "qpath_units", "targets_from"] {
        assert_eq!(result["detail"][key], expected[key], "detail.{key}");
    }
    let got = compact(&manifest(&out, "pilot"));
    let want = expected["manifest"].as_array().expect("manifest");
    assert_eq!(got.len(), want.len(), "one manifest row per file");
    for (got, want) in got.iter().zip(want) {
        assert_eq!(got, want, "manifest row of {}", want["path"]);
    }
    assert_stdout_has_no_names(&output, &corpus);
    assert_eq!(snapshot(&corpus), before, "the copy is read only");
    assert_no_cargo_leftovers(&corpus);
}

#[test]
fn each_main_and_setup_sits_in_its_own_unit() {
    let scratch = Scratch::new("own-unit");
    let corpus = scratch.corpus();
    let out = scratch.join("out");
    result_of(&ast_hash(Some(&corpus), &out, &scratch.0, None));
    let manifest = manifest(&out, "pilot");
    // label → [(file, qpath, unit, ambiguity)]
    let mut by_label: BTreeMap<&str, Vec<(&str, &str, &str, &Value)>> = BTreeMap::new();
    for file in manifest["files"].as_array().unwrap() {
        for item in file["items"].as_array().unwrap() {
            by_label
                .entry(item["label"].as_str().unwrap())
                .or_default()
                .push((
                    file["path"].as_str().unwrap(),
                    item["qpath"].as_str().unwrap(),
                    file["unit"].as_str().unwrap_or("null"),
                    &item["ambiguity"],
                ));
        }
    }
    let check = |label: &str, want: &[(&str, &str)]| {
        let rows = &by_label[label];
        let got: Vec<(&str, &str)> = rows.iter().map(|(f, _, u, _)| (*f, *u)).collect();
        assert_eq!(got, want, "`fn {label}`: file and unit");
        let qpaths: std::collections::BTreeSet<&str> = rows.iter().map(|(_, q, _, _)| *q).collect();
        assert_eq!(
            qpaths.len(),
            rows.len(),
            "every `fn {label}` has its own qpath"
        );
        assert!(
            rows.iter().all(|(_, _, _, a)| a.is_null()),
            "no `fn {label}` is ambiguous: {rows:?}"
        );
    };
    check(
        "main",
        &[
            ("app/benches/b.rs", "bench:b"),
            ("app/build.rs", "custom-build:build-script-build"),
            ("app/examples/d.rs", "example:d"),
            ("app/examples/e/main.rs", "example:e"),
            ("app/src/bin/m/main.rs", "bin:m"),
            ("app/src/bin/t.rs", "bin:t"),
            ("app/src/main.rs", "bin:app-cli"),
            ("app/tools/x.rs", "bin:x"),
            ("binonly/src/main.rs", "primary"),
        ],
    );
    check(
        "setup",
        &[
            ("app/examples/d.rs", "example:d"),
            ("app/examples/e/main.rs", "example:e"),
            ("app/tests/a.rs", "test:a"),
            ("app/tests/s/main.rs", "test:s"),
        ],
    );
    // The library's items carry no unit.
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        if path.starts_with("app/src/")
            && !path.starts_with("app/src/bin/")
            && path != "app/src/main.rs"
        {
            assert_eq!(file["unit"], "primary", "{path}");
            for item in file["items"].as_array().unwrap() {
                let qpath = item["qpath"].as_str().unwrap();
                assert!(
                    qpath.split("::").all(|segment| !segment.contains(':')),
                    "a library item carries no unit: {qpath}"
                );
            }
        }
    }
}

#[test]
fn repeat_run_is_byte_identical() {
    let scratch = Scratch::new("repeat");
    let corpus = scratch.corpus();
    let first = scratch.join("out-1");
    let second = scratch.join("out-2");
    let one = result_of(&ast_hash(Some(&corpus), &first, &scratch.0, None));
    let two = result_of(&ast_hash(Some(&corpus), &second, &scratch.0, None));
    let read =
        |out: &Path| fs::read(out.join("ast-hash").join("pilot").join("manifest.json")).unwrap();
    assert_eq!(
        read(&first),
        read(&second),
        "manifest.json differs between runs"
    );
    for key in ["qpath_units", "targets_from", "qpath_ambiguity"] {
        assert_eq!(one["detail"][key], two["detail"][key], "{key}");
    }
}

/// Every item row of the manifest: `(file, qpath, ambiguity)`.
fn item_rows(manifest: &Value) -> Vec<(String, String, Value)> {
    let mut rows = Vec::new();
    for file in manifest["files"].as_array().expect("files") {
        let path = file["path"].as_str().expect("path");
        for item in file["items"].as_array().expect("items") {
            rows.push((
                path.to_owned(),
                item["qpath"].as_str().expect("qpath").to_owned(),
                item["ambiguity"].clone(),
            ));
        }
    }
    rows
}

#[test]
fn shared_items_are_flagged_duplicate_but_never_path_attribute() {
    // Spec "Units and `qpath`": `duplicate` = equal rendered `qpath`, shared
    // ones too; shared items skip `path_attribute`.
    let scratch = Scratch::new("shared-flags");
    let corpus = scratch.corpus();
    let out = scratch.join("out");
    let result = result_of(&ast_hash(Some(&corpus), &out, &scratch.0, None));
    let manifest = manifest(&out, "pilot");
    let rows = item_rows(&manifest);
    let flagged = |reason: &str| -> Vec<(String, String)> {
        rows.iter()
            .filter(|(_, _, ambiguity)| ambiguity == reason)
            .map(|(file, qpath, _)| (file.clone(), qpath.clone()))
            .collect()
    };
    let house = "app::shared:tests/common::house::impl House";
    let pair = "app::impl Pair";
    assert_eq!(
        flagged("duplicate"),
        [
            ("app/src/lib.rs".to_owned(), pair.to_owned()),
            ("app/src/lib.rs".to_owned(), pair.to_owned()),
            ("app/tests/common/house.rs".to_owned(), house.to_owned()),
            ("app/tests/common/house.rs".to_owned(), house.to_owned()),
        ],
        "exactly the two adjacent-impl pairs, the shared one included"
    );
    assert_eq!(
        flagged("path_attribute"),
        [(
            "app/src/vendored/relocated.rs".to_owned(),
            "app::vendored::relocated::relocated".to_owned()
        )],
        "only the `#[path]` target outside a shared dir"
    );
    // The second `#[path]` target is there, shared and unflagged.
    assert_eq!(result["path_attrs"], 2, "two `#[path]` attributes");
    let extra = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "app/tests/common/extra.rs")
        .expect("the shared `#[path]` target is in the manifest");
    assert_eq!(extra["unit"], "shared:tests/common");
    assert_eq!(extra["role"], "module:extra");
    for item in extra["items"].as_array().unwrap() {
        assert_eq!(item["ambiguity"], Value::Null, "{item}");
    }
    assert_eq!(result["detail"]["qpath_ambiguity"]["duplicate"], 4);
    assert_eq!(result["detail"]["qpath_ambiguity"]["path_attribute"], 1);
}

/// The two files a test adds at the virtual workspace root of the copy: rule
/// 3 dirs (`tests/common/`, `examples/x/`) with no target anywhere.
const ROOT_RULE_THREE: [(&str, &str); 2] = [
    ("examples/x/y.rs", "pub fn y() {}\n"),
    ("tests/common/mod.rs", "pub fn root_helper() {}\n"),
];

/// The manifest rows of the root files: unrooted, every item `unrooted`.
fn assert_root_files_unrooted(out: &Path, source: &str) {
    let manifest = manifest(out, "pilot");
    let rows: Vec<&Value> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|file| {
            ROOT_RULE_THREE
                .iter()
                .any(|(path, _)| file["path"] == *path)
        })
        .collect();
    assert_eq!(rows.len(), ROOT_RULE_THREE.len(), "{manifest}");
    for row in rows {
        let path = &row["path"];
        assert_eq!(row["role"], "unrooted", "{path}");
        assert_eq!(row["unit"], Value::Null, "{path}");
        assert_eq!(row["targets"], source, "{path}");
        for item in row["items"].as_array().unwrap() {
            assert_eq!(item["ambiguity"], "unrooted", "{path}: {item}");
        }
    }
}

#[test]
fn rule_three_dirs_of_a_target_less_root_are_unrooted_from_metadata_and_layout() {
    // Spec "Units and `qpath`": no targets (a virtual root's empty Metadata
    // table, a target-less layout) → rules 2–4 give `Unrooted`.
    let scratch = Scratch::new("virtual-root");
    let corpus = scratch.corpus();
    for (relative, text) in ROOT_RULE_THREE {
        let path = corpus.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    // Metadata: the root's run covers the virtual root with an empty table.
    let out = scratch.join("out-metadata");
    let result = result_of(&ast_hash(Some(&corpus), &out, &scratch.0, None));
    assert_eq!(
        result["detail"]["targets_from"],
        json!({"layout": 0, "metadata": 3})
    );
    assert_eq!(result["detail"]["qpath_units"]["unrooted"], 2);
    assert_eq!(
        result["detail"]["qpath_units"]["shared"], 8,
        "no new shared unit"
    );
    assert_root_files_unrooted(&out, "metadata");
    // The members keep the rows of expected.json.
    let members: Vec<Value> = compact(&manifest(&out, "pilot"))
        .into_iter()
        .filter(|row| ROOT_RULE_THREE.iter().all(|(path, _)| row["path"] != *path))
        .collect();
    assert_eq!(members, expected()["manifest"].as_array().unwrap().clone());

    // Layout: a broken root manifest; the root's own layout has no target.
    fs::write(
        corpus.join("Cargo.toml"),
        "[workspace\nmembers = [\"app\"\n",
    )
    .unwrap();
    let out = scratch.join("out-layout");
    let result = result_of(&ast_hash(Some(&corpus), &out, &scratch.0, None));
    assert_eq!(
        result["detail"]["targets_from"],
        json!({"layout": 3, "metadata": 0})
    );
    assert_eq!(
        result["detail"]["qpath_units"]["shared"], 8,
        "no new shared unit"
    );
    assert_root_files_unrooted(&out, "layout");
    assert_no_cargo_leftovers(&corpus);
}

// ------------------------------------------------------------------ AC-04

/// The layout fallback: every file as in the metadata run but the two of
/// `layout.changed`, every `targets` = `layout`.
fn assert_layout_manifest(out: &Path, result: &Value, renamed_bin: &str) {
    let expected = expected();
    let layout = &expected["layout"];
    assert_eq!(result["detail"]["targets_from"], layout["targets_from"]);
    assert_eq!(
        result["detail"]["qpath_ambiguity"],
        layout["qpath_ambiguity"]
    );
    assert_eq!(result["detail"]["qpath_units"], layout["qpath_units"]);
    let changed: BTreeMap<&str, &Value> = layout["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["path"].as_str().unwrap(), row))
        .collect();
    let got = compact(&manifest(out, "pilot"));
    let want = expected["manifest"].as_array().unwrap();
    assert_eq!(got.len(), want.len());
    for (got, want) in got.iter().zip(want) {
        let path = want["path"].as_str().unwrap();
        assert_eq!(got["path"], want["path"]);
        assert_eq!(got["targets"], "layout", "{path}");
        let reference = changed.get(path).copied().unwrap_or(want);
        for key in ["role", "unit", "items"] {
            let mut wanted = reference[key].clone();
            if path == "app/src/main.rs" {
                // The renamed bin takes the package name, or the directory
                // name when the manifest does not parse.
                let text = wanted.to_string().replace("unit-app", renamed_bin);
                wanted = serde_json::from_str(&text).unwrap();
            }
            assert_eq!(got[key], wanted, "{key} of {path}");
        }
    }
}

#[test]
fn broken_root_manifest_falls_back_to_layout_with_exit_zero() {
    let scratch = Scratch::new("broken-root");
    let corpus = scratch.corpus();
    fs::write(
        corpus.join("Cargo.toml"),
        "[workspace\nmembers = [\"app\"\n",
    )
    .unwrap();
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, None);
    let result = result_of(&output);
    assert_layout_manifest(&out, &result, "unit-app");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("layout targets used"),
        "the fallback is told on stderr:\n{stderr}"
    );
    assert_stdout_has_no_names(&output, &corpus);
    assert_no_cargo_leftovers(&corpus);
}

#[test]
fn missing_cargo_falls_back_to_layout_with_exit_zero() {
    let scratch = Scratch::new("no-cargo");
    let corpus = scratch.corpus();
    let out = scratch.join("out");
    let missing = scratch.join("no-such-cargo");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, Some(&missing));
    let result = result_of(&output);
    assert_layout_manifest(&out, &result, "unit-app");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("cannot run").count(),
        1,
        "one note for a missing cargo:\n{stderr}"
    );
    assert_stdout_has_no_names(&output, &corpus);
}

#[test]
fn broken_package_manifest_names_the_default_bin_after_its_directory() {
    let scratch = Scratch::new("broken-package");
    let corpus = scratch.corpus();
    fs::write(
        corpus.join("app").join("Cargo.toml"),
        "[package\nname = \"unit-app\"\n",
    )
    .unwrap();
    let out = scratch.join("out");
    let result = result_of(&ast_hash(Some(&corpus), &out, &scratch.0, None));
    // The workspace cannot load a broken member: both packages fall back.
    assert_layout_manifest(&out, &result, "app");
}

/// Every stub process logged in `pids` that still runs `sleep <nap>`, and
/// every other `sleep <nap>` `pgrep` finds; each is killed, so a red run
/// leaves nothing behind.
fn kill_survivors(pids: &Path, nap: &str) -> Vec<String> {
    let mut survivors = Vec::new();
    let wanted = format!("sleep {nap}");
    let mut kill = |pid: &str, how: &str| {
        survivors.push(format!("{how} {pid}"));
        let _ = Command::new("kill").args(["-9", pid]).status();
    };
    for pid in fs::read_to_string(pids).unwrap_or_default().lines() {
        let ps = Command::new("ps")
            .args(["-p", pid, "-o", "command="])
            .output()
            .expect("ps runs");
        if String::from_utf8_lossy(&ps.stdout).trim() == wanted {
            kill(pid, "logged pid");
        }
    }
    let pattern = format!("^sleep {}$", nap.replace('.', "\\."));
    let pgrep = Command::new("pgrep")
        .args(["-f", &pattern])
        .output()
        .expect("pgrep runs");
    for pid in String::from_utf8_lossy(&pgrep.stdout).split_whitespace() {
        kill(pid, "pgrep");
    }
    survivors
}

#[test]
fn hanging_cargo_is_called_once_killed_and_every_dir_falls_back() {
    // A stub that never answers; a short `--timeout` cuts its budget to
    // what the run leaves less 1 s (instead of 60 s).
    let scratch = Scratch::new("stub-hang");
    let corpus = scratch.corpus();
    let log = scratch.join("argv.log");
    let pids = scratch.join("pids.log");
    // Unique and bounded: were it never killed, it would still end by itself.
    let nap = format!("29.{}", std::process::id());
    let cargo = stub_with(
        &scratch,
        &log,
        &format!(
            "printf '%s\\n' \"$$\" >> '{}'\nexec sleep {nap}",
            pids.display()
        ),
    );
    let out = scratch.join("out");
    let started = Instant::now();
    let output = ast_hash_with(
        Some(&corpus),
        &out,
        &scratch.0,
        Some(&cargo),
        &["--timeout", "8"],
    );
    let elapsed = started.elapsed();
    let survivors = kill_survivors(&pids, &nap);
    assert!(
        survivors.is_empty(),
        "a cargo child outlived the run: {survivors:?}"
    );
    let result = result_of(&output);
    assert!(
        result.is_object(),
        "a measurement, not \"timeout\": {result}"
    );
    assert!(
        elapsed < Duration::from_secs(20),
        "the run waited for the stub to end by itself: {elapsed:?}"
    );
    let root = corpus.canonicalize().unwrap();
    assert_eq!(
        calls(&log),
        [metadata_argv(&root.join("app").join("Cargo.toml"))],
        "one call, none after the overrun"
    );
    assert_eq!(
        fs::read_to_string(&pids).unwrap().lines().count(),
        1,
        "one stub process"
    );
    assert_layout_manifest(&out, &result, "unit-app");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let cargo_lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("cargo metadata"))
        .collect();
    assert_eq!(
        cargo_lines.len(),
        1,
        "one note: the overrun ends the cargo calls of the run:\n{stderr}"
    );
    assert!(
        cargo_lines[0].contains("gave no answer within") && cargo_lines[0].contains("killed"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("leaves no time"),
        "no further package dir reached the cargo step:\n{stderr}"
    );
    assert_stdout_has_no_names(&output, &corpus);
    assert_no_cargo_leftovers(&corpus);
}

// ------------------------------------------------------------------ AC-05

#[test]
fn stub_cargo_gets_exactly_metadata_no_deps_offline_and_its_stderr_stays_off_stdout() {
    let scratch = Scratch::new("stub");
    let corpus = scratch.corpus();
    let log = scratch.join("argv.log");
    let cargo = stub(&scratch, &log, "STUB-STDERR-line", None);
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, Some(&cargo));
    let result = result_of(&output);
    let root = corpus.canonicalize().unwrap();
    assert_eq!(
        calls(&log),
        [metadata_argv(&root.join("app").join("Cargo.toml"))],
        "one call: the first package dir's run covers the workspace"
    );
    assert_eq!(result["detail"]["targets_from"], expected()["targets_from"]);
    assert_eq!(
        compact(&manifest(&out, "pilot")),
        expected()["manifest"].as_array().unwrap().clone()
    );
    assert_stdout_has_no_names(&output, &corpus);
    assert_no_cargo_leftovers(&corpus);
}

#[test]
fn failing_cargo_reason_reaches_stderr_never_stdout() {
    let scratch = Scratch::new("stub-fail");
    let corpus = scratch.corpus();
    let log = scratch.join("argv.log");
    let cargo = stub(&scratch, &log, "error: STUB-FAILURE-reason", Some(101));
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, Some(&cargo));
    let result = result_of(&output);
    let root = corpus.canonicalize().unwrap();
    assert_eq!(
        calls(&log),
        [
            metadata_argv(&root.join("app").join("Cargo.toml")),
            metadata_argv(&root.join("binonly").join("Cargo.toml")),
        ],
        "a failed run covers nothing: every package dir is tried"
    );
    assert_layout_manifest(&out, &result, "unit-app");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("STUB-FAILURE-reason").count(),
        2,
        "one reason line per package dir:\n{stderr}"
    );
    assert_stdout_has_no_names(&output, &corpus);
}

#[test]
fn cargo_runs_outside_the_corpus_even_when_the_harness_starts_inside_it() {
    let scratch = Scratch::new("stub-cwd");
    let corpus = scratch.corpus();
    // Picked up through the working directory, this config breaks every
    // cargo call ("could not load Cargo configuration"): a metadata answer
    // proves cargo did not run inside the corpus.
    fs::create_dir_all(corpus.join(".cargo")).unwrap();
    fs::write(
        corpus.join(".cargo").join("config.toml"),
        "[build\nbroken =\n",
    )
    .unwrap();
    let before = snapshot(&corpus);
    let log = scratch.join("argv.log");
    let cargo = stub(&scratch, &log, "STUB-STDERR-line", None);
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &corpus, Some(&cargo));
    let result = result_of(&output);
    let root = corpus.canonicalize().unwrap();
    let calls = calls(&log);
    assert_eq!(calls, [metadata_argv(&root.join("app").join("Cargo.toml"))]);
    assert!(
        calls[0].windows(7).any(|w| w
            == [
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--offline",
                "--color",
                "never"
            ]),
        "{calls:?}"
    );
    let cwds = fs::read_to_string(cwd_log(&log)).expect("the stub logged its cwd");
    let cwds: Vec<&str> = cwds.lines().collect();
    assert_eq!(cwds.len(), 1, "{cwds:?}");
    for cwd in cwds {
        let cwd = Path::new(cwd);
        assert!(cwd.is_absolute() && cwd.is_dir(), "{}", cwd.display());
        assert!(
            !cwd.starts_with(&root) && !cwd.starts_with(&corpus),
            "cargo ran inside the corpus: {}",
            cwd.display()
        );
    }
    assert_eq!(
        result["detail"]["targets_from"],
        expected()["targets_from"],
        "the corpus's .cargo/config.toml reached cargo"
    );
    assert_stdout_has_no_names(&output, &corpus);
    assert_eq!(snapshot(&corpus), before, "the copy is read only");
    assert_no_cargo_leftovers(&corpus);
}

/// The rustup `cargo` proxy (`$CARGO_HOME/bin/cargo`, next to `rustup`): it
/// picks its toolchain from a `rust-toolchain.toml` in its working directory
/// or above, unless `RUSTUP_TOOLCHAIN` names one.
fn rustup_cargo_proxy() -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")))?;
    let bin = home.join("bin");
    (bin.join("cargo").is_file() && bin.join("rustup").is_file()).then(|| bin.join("cargo"))
}

#[test]
fn cargo_runs_at_the_filesystem_root_never_in_the_temp_dir_inside_or_outside_the_corpus() {
    let scratch = Scratch::new("stub-cwd-root");
    let corpus = scratch.corpus();
    // A foreign toolchain: its cargo only leaves a trace and fails, so a
    // metadata answer and no trace prove the rustup proxy never ran in the
    // directory holding the planted `rust-toolchain.toml`.
    let planted = scratch.join("planted");
    let trace = scratch.join("planted.log");
    fs::create_dir_all(planted.join("bin")).unwrap();
    for tool in ["cargo", "rustc"] {
        let path = planted.join("bin").join(tool);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\necho PLANTED >> '{}'\nexit 101\n",
                trace.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let legs = [
        ("inside", corpus.join("app"), corpus.clone()),
        ("outside", scratch.join("tmpdir"), scratch.0.clone()),
    ];
    for (_, tmpdir, _) in &legs {
        fs::create_dir_all(tmpdir).unwrap();
        fs::write(
            tmpdir.join("rust-toolchain.toml"),
            format!("[toolchain]\npath = \"{}\"\n", planted.display()),
        )
        .unwrap();
    }
    let before = snapshot(&corpus);
    let proxy = rustup_cargo_proxy();
    if proxy.is_none() {
        eprintln!("no rustup cargo proxy: the stub execs {REAL_CARGO}");
    }
    let next = proxy.unwrap_or_else(|| PathBuf::from(REAL_CARGO));
    let root = corpus.canonicalize().unwrap();
    for (leg, tmpdir, start) in &legs {
        let log = scratch.join(&format!("argv-{leg}.log"));
        let cargo = stub_with(&scratch, &log, &format!("exec '{}' \"$@\"", next.display()));
        let out = scratch.join(&format!("out-{leg}"));
        let output = Command::new(BIN)
            .current_dir(start)
            .env_remove("SPECENGINE_PILOT_A")
            .env_remove("SPECENGINE_PILOT_B")
            // Set by the rustup proxy that started this test; it would hide
            // a planted `rust-toolchain.toml` from the proxy the stub runs.
            .env_remove("RUSTUP_TOOLCHAIN")
            .env_remove("RUSTUP_TOOLCHAIN_SOURCE")
            .env("SPECENGINE_CARGO", &cargo)
            .env("TMPDIR", tmpdir)
            .arg("ast-hash")
            .arg("--pilot")
            .arg(&corpus)
            .arg("--out")
            .arg(&out)
            .output()
            .expect("specengine-eval runs");
        let result = result_of(&output);
        let cwds = fs::read_to_string(cwd_log(&log)).expect("the stub logged its cwd");
        assert_eq!(
            cwds.lines().collect::<Vec<_>>(),
            ["/"],
            "{leg}: TMPDIR {}",
            tmpdir.display()
        );
        assert!(
            !trace.exists(),
            "{leg}: the planted toolchain ran:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            calls(&log),
            [metadata_argv(&root.join("app").join("Cargo.toml"))],
            "{leg}"
        );
        assert_eq!(
            result["detail"]["targets_from"],
            expected()["targets_from"],
            "{leg}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(snapshot(&corpus), before, "nothing written into the corpus");
    assert_no_cargo_leftovers(&corpus);
}

/// A `SPECENGINE_RUSTFMT` stand-in at `path` (docs/features/pointer-sweep.md,
/// Data): appends `<kind> <pwd -P>` to `log`, `kind` = `version` for
/// `--version` (then prints `rustfmt 0.0.0-stub`), else `format` (then
/// echoes stdin, its arguments ignored); exit 0.
fn rustfmt_stub(path: &Path, log: &Path) {
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = --version ]; then\n  printf 'version %s\\n' \"$(pwd -P)\" >> '{log}'\n  echo 'rustfmt 0.0.0-stub'\nelse\n  printf 'format %s\\n' \"$(pwd -P)\" >> '{log}'\n  cat\nfi\nexit 0\n",
        log = log.display(),
    );
    fs::write(path, script).expect("rustfmt stub written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .expect("rustfmt stub executable");
    }
}

/// docs/features/pointer-sweep.md AC-06: every rustfmt call, `--version`
/// included, runs at `/` like the cargo call, even when the harness starts
/// inside the corpus; the version reported is the one taken there. Two legs:
/// the stub by absolute path, and by a path relative to the harness's own
/// directory (resolved before any spawn, `ast_hash/fmt.rs`). Named
/// mutations: `current_dir` dropped from the format spawn or from the
/// `--version` spawn turns this red.
#[test]
fn rustfmt_runs_at_the_filesystem_root_even_when_the_harness_starts_inside_the_corpus() {
    let scratch = Scratch::new("rustfmt-cwd");
    let corpus = scratch.join("corpus");
    copy_dir(
        &repository_root().join("fixtures").join("ast-hash"),
        &corpus,
    );
    let before = snapshot(&corpus);
    let stub = scratch.join("rustfmt-stub.sh");
    let relative = Path::new("..").join("rustfmt-stub.sh");
    // Resolved from `/` instead of the harness's directory, it names nothing.
    assert!(!Path::new("/").join(&relative).exists());
    for (leg, command) in [("absolute", stub.clone()), ("relative", relative)] {
        let log = scratch.join(&format!("rustfmt-{leg}.log"));
        rustfmt_stub(&stub, &log);
        let out = scratch.join(&format!("out-{leg}"));
        let output = Command::new(BIN)
            .current_dir(&corpus)
            .env_remove("SPECENGINE_PILOT_A")
            .env_remove("SPECENGINE_PILOT_B")
            .env_remove("SPECENGINE_CARGO")
            .env("SPECENGINE_RUSTFMT", &command)
            .arg("ast-hash")
            .arg("--pilot")
            .arg(&corpus)
            .arg("--out")
            .arg(&out)
            .output()
            .expect("specengine-eval runs");
        let result = result_of(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let calls = fs::read_to_string(&log)
            .unwrap_or_else(|_| panic!("{leg}: the stub never ran; stderr:\n{stderr}"));
        let calls: Vec<&str> = calls.lines().collect();
        let versions = calls.iter().filter(|call| **call == "version /").count();
        let formats = calls.iter().filter(|call| **call == "format /").count();
        assert_eq!(versions, 1, "{leg}: one `--version`, at `/`: {calls:?}");
        assert!(formats >= 1, "{leg}: no format call at `/`: {calls:?}");
        assert_eq!(
            versions + formats,
            calls.len(),
            "{leg}: a rustfmt call ran elsewhere: {calls:?}"
        );
        assert_eq!(
            result["detail"]["rustfmt"]["version"], "rustfmt 0.0.0-stub",
            "{leg}: {}",
            result["detail"]["rustfmt"]
        );
        assert_eq!(result["detail"]["rustfmt"]["available"], true, "{leg}");
    }
    assert_eq!(snapshot(&corpus), before, "nothing written into the corpus");
    assert_no_cargo_leftovers(&corpus);
}

#[test]
fn a_failure_line_names_the_corpus_by_its_label_never_its_root() {
    let scratch = Scratch::new("stub-scrub");
    let corpus = scratch.corpus();
    let log = scratch.join("argv.log");
    // Cargo's wording for a broken manifest, with the absolute path it got.
    let cargo = stub_with(
        &scratch,
        &log,
        r#"for last; do :; done
echo "error: failed to parse manifest at \`$last\`" >&2
exit 101"#,
    );
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, Some(&cargo));
    let result = result_of(&output);
    assert_eq!(calls(&log).len(), 2, "a failed run covers nothing");
    assert_layout_manifest(&out, &result, "unit-app");
    let stderr = String::from_utf8_lossy(&output.stderr);
    for package in ["app", "binonly"] {
        let line = format!(
            "cargo metadata failed for package dir {package}: error: failed to parse manifest at `pilot/{package}/Cargo.toml`"
        );
        assert_eq!(stderr.matches(&line).count(), 1, "{line}\n{stderr}");
    }
    let root = corpus.canonicalize().unwrap();
    for path in [root.to_str().unwrap(), corpus.to_str().unwrap()] {
        assert!(
            !stderr.contains(path),
            "the corpus root reached stderr:\n{stderr}"
        );
    }
    assert_stdout_has_no_names(&output, &corpus);
}

#[test]
fn a_failure_line_scrubs_the_corpus_root_only_where_it_stands_as_a_whole_path() {
    let scratch = Scratch::short("scrub");
    let corpus = scratch.corpus();
    let root = corpus.canonicalize().unwrap();
    let log = scratch.join("argv.log");
    // `app`: the root as a whole path behind a backtick, `(`, a quote, `=`
    // and as the whole path ahead of `:`, `)`, a quote, `/`; `binonly`:
    // other paths holding the root, then the root alone at the end.
    let cargo = stub_with(
        &scratch,
        &log,
        r##"for last; do :; done
root=$(dirname "$(dirname "$last")")
case "$last" in
  */app/Cargo.toml) echo "error: \`$root/x\` ($root) '$root' \"$root/a\" =$root:12 /mnt$root/y" >&2 ;;
  *) echo "error: $root.bak $root-old ${root}_2 /mnt$root/y $root" >&2 ;;
esac
exit 101"##,
    );
    let out = scratch.join("out");
    let output = ast_hash(Some(&corpus), &out, &scratch.0, Some(&cargo));
    let result = result_of(&output);
    assert_eq!(calls(&log).len(), 2, "a failed run covers nothing");
    assert_layout_manifest(&out, &result, "unit-app");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let root = root.to_str().unwrap();
    let near_misses = [
        format!("/mnt{root}/y"),
        format!("{root}.bak"),
        format!("{root}-old"),
        format!("{root}_2"),
    ];
    for line in [
        format!(
            "cargo metadata failed for package dir app: error: `pilot/x` (pilot) 'pilot' \"pilot/a\" =pilot:12 {}; layout targets used",
            near_misses[0]
        ),
        format!(
            "cargo metadata failed for package dir binonly: error: {} {} {} {} pilot; layout targets used",
            near_misses[1], near_misses[2], near_misses[3], near_misses[0]
        ),
    ] {
        assert_eq!(stderr.matches(&line).count(), 1, "{line}\n{stderr}");
    }
    let mut rest = stderr.into_owned();
    for near_miss in &near_misses {
        rest = rest.replace(near_miss.as_str(), "");
    }
    for path in [root, corpus.to_str().unwrap()] {
        assert!(
            !rest.contains(path),
            "the corpus root reached stderr as a whole path:\n{rest}"
        );
    }
    assert_stdout_has_no_names(&output, &corpus);
}

#[test]
fn in_place_runs_write_nothing_into_the_fixtures() {
    let fixture = units_fixture();
    let before = snapshot(&fixture);
    let status_before = git_status_of_fixtures();
    let scratch = Scratch::new("in-place");
    let out = scratch.join("out");
    let result = result_of(&ast_hash(Some(&fixture), &out, &repository_root(), None));
    assert_eq!(result["detail"]["targets_from"], expected()["targets_from"]);
    let default_out = scratch.join("default-out");
    let output = ast_hash(None, &default_out, &repository_root(), None);
    result_of(&output);
    assert_stdout_has_no_names(&output, &repository_root().join("fixtures"));
    assert_eq!(snapshot(&fixture), before, "fixtures/cargo-units changed");
    assert_no_cargo_leftovers(&fixture);
    assert_no_cargo_leftovers(&repository_root().join("fixtures").join("ast-hash"));
    let status_after = git_status_of_fixtures();
    assert_eq!(
        status_after, status_before,
        "git status -- fixtures/ changed"
    );
    assert!(
        !status_after.contains("Cargo.lock") && !status_after.contains("target"),
        "{status_after}"
    );
}

// ------------------------------------------------------------------ AC-03

#[test]
fn ast_hash_fixture_keeps_its_expected_json_and_today_s_qpaths() {
    let root = repository_root();
    let committed = Command::new("git")
        .current_dir(&root)
        .args(["show", "HEAD:fixtures/ast-hash/expected.json"])
        .output()
        .expect("git runs");
    assert!(committed.status.success());
    assert_eq!(
        fs::read(root.join("fixtures/ast-hash/expected.json")).unwrap(),
        committed.stdout,
        "fixtures/ast-hash/expected.json must stay as committed"
    );
    let scratch = Scratch::new("ast-hash");
    let out = scratch.join("out");
    let result = result_of(&ast_hash(None, &out, &root, None));
    assert_eq!(
        result["detail"]["qpath_units"],
        json!({"primary": 69, "shared": 0, "unrooted": 0})
    );
    assert_eq!(
        result["detail"]["targets_from"],
        json!({"metadata": 1, "layout": 0})
    );
    let manifest = manifest(&out, "fixtures");
    let mut qpaths = Vec::new();
    for file in manifest["files"].as_array().unwrap() {
        assert_eq!(file["unit"], "primary", "{}", file["path"]);
        assert_eq!(file["targets"], "metadata", "{}", file["path"]);
        for item in file["items"].as_array().unwrap() {
            qpaths.push(item["qpath"].as_str().unwrap().to_owned());
        }
    }
    assert_eq!(qpaths, AST_HASH_QPATHS, "no unit, today's modules");
}

/// Every `qpath` of `fixtures/ast-hash` in manifest order: all items of a
/// library package, so no unit; module paths from `src/`.
const AST_HASH_QPATHS: [&str; 69] = [
    ".::blocks::Command",
    ".::blocks::Report",
    ".::blocks::describe_move_with_full_context",
    ".::blocks::rename_with_prefix_and_suffix",
    ".::blocks::halted_weight",
    ".::blocks::has_weight",
    ".::blocks::every",
    ".::blocks::summarize",
    ".::blocks::weight_or_default",
    ".::blocks::total_weight",
    ".::blocks::labels_with_prefix",
    ".::blocks::first_heavy_index",
    ".::blocks::count_until_halt",
    ".::blocks::skip_renames",
    ".::blocks::heavy_labels",
    ".::blocks::labels_in_macro",
    ".::blocks::all_have_weight",
    ".::broken::broken_one",
    ".::broken::broken_two",
    ".::broken::intact_neighbour",
    ".::comments::Counter",
    ".::comments::impl Counter",
    ".::comments::Counter::tick",
    ".::comments::Counter::reset",
    ".::comments::Named",
    ".::comments::Named::name",
    ".::comments::Named::shout",
    ".::comments::impl <Counter as Named>",
    ".::comments::<Counter as Named>::name",
    ".::comments::COMMENTED",
    ".::blocks",
    ".::broken",
    ".::comments",
    ".::macro_error",
    ".::trailing",
    ".::unformatted",
    ".::relocated",
    ".::ANSWER",
    ".::GREETING",
    ".::Pair",
    ".::entry",
    ".::nested",
    ".::nested::inner",
    ".::nested::deeper",
    ".::nested::deeper::deepest",
    ".::tests",
    ".::tests::it_adds",
    ".::macro_error::intact_before",
    ".::macro_error::tilde",
    ".::macro_error::intact_after",
    ".::trailing::Point",
    ".::trailing::Shape",
    ".::trailing::origin",
    ".::trailing::shifted",
    ".::trailing::corners",
    ".::trailing::side_count",
    ".::trailing::combine_three_values",
    ".::trailing::describe_everything",
    ".::trailing::longest_call_site",
    ".::trailing::generic_pair",
    ".::unformatted::Config",
    ".::unformatted::impl Config",
    ".::unformatted::Config::area",
    ".::unformatted::Config::square",
    ".::unformatted::clamp_wide",
    ".::unformatted::Mode",
    ".::unformatted::spaced_out",
    ".::vendored::relocated::relocated_helper",
    ".::vendored::relocated::Relocated",
];
