//! `specengine-eval code-index` end to end (docs/features/symbol-index.md
//! "AC-10 measurement", RD-12; Q10, Q13, Q25) on the real binary over
//! `fixtures/code-index` (its default corpus): one anonymous envelope of
//! counts and times; `noop_parsed` 0 and `one_file_parsed` 1 with the line
//! appended to the largest `.rs`, never the first; `items` the stored
//! symbols; `twins`, `signatures` the fixture's `expected.json` counts;
//! `roles` each Rust file's role under the layout against a cargo
//! stand-in's `cargo metadata` table (`SPECENGINE_CARGO`, as
//! `ast_hash_units.rs`): `differ` counts a changed role, `null` when no
//! package answered; the fixtures untouched.
//!
//! docs/features/bevy-roles.md "Eval" (E1, SR6): `result` gains
//! `framework_roles` (the `code.roles` object of the last refresh; `null`
//! with the opt-in off) and `roles_ms`; the scheme's `frameworks` is kept
//! (`--pilot` over a scratch copy of `fixtures/code-bevy`), and under a
//! pilot label `SPECENGINE_CODE_FRAMEWORKS_A` joins
//! `SPECENGINE_CODE_ROOTS_A`'s table (unset or empty: off; an unknown name:
//! exit 2).
//!
//! As built (the spec leaves them open): where the harness keeps its scratch copy and database
//! (`--out/code-index/<label>/corpus`, `…/index.db`, as `index.rs`'s
//! `--out/index/<label>/`), and the shape of `twins` (`{cfg, ordinal}` as
//! `{groups, items}`, `cross_file` a count) and `roles` (`{files,
//! metadata, differ}`) follow the spec's words. As built (iteration 1):
//! `roles_ms` is `null` with the opt-in off (no role read).

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

/// The variables a pilot run reads: never inherited here.
const ENV: [&str; 11] = [
    "SPECENGINE_PILOT_A",
    "SPECENGINE_PILOT_B",
    "SPECENGINE_SCHEME_A",
    "SPECENGINE_SCHEME_B",
    "SPECENGINE_CODE_ROOTS_A",
    "SPECENGINE_CODE_ROOTS_B",
    "SPECENGINE_CODE_FRAMEWORKS_A",
    "SPECENGINE_CODE_FRAMEWORKS_B",
    "SPECENGINE_CENSUS_CONFIG_A",
    "SPECENGINE_CENSUS_CONFIG_B",
    "SPECENGINE_CARGO",
];

/// The spec's fields ("AC-10 measurement"; bevy-roles "Eval"), exactly.
const RESULT_KEYS: [&str; 18] = [
    "bytes",
    "db_bytes",
    "framework_roles",
    "full_ms",
    "hash_ms",
    "items",
    "kloc",
    "noop_ms",
    "noop_parsed",
    "one_file_ms",
    "one_file_parsed",
    "refs_ms",
    "roles",
    "roles_ms",
    "ron_files",
    "rs_files",
    "signatures",
    "twins",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture() -> PathBuf {
    repository_root().join("fixtures").join("code-index")
}

fn expected() -> Value {
    serde_json::from_str(&fs::read_to_string(fixture().join("expected.json")).unwrap())
        .expect("fixtures/code-index/expected.json")
}

/// `fixtures/code-bevy/expected.json` (bevy-roles TE-1).
fn bevy_expected() -> Value {
    let path = repository_root().join("fixtures/code-bevy/expected.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap())
        .expect("fixtures/code-bevy/expected.json")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-code-index-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(fs::canonicalize(&path).expect("canonical scratch"))
    }

    fn join(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An executable shell script at `path`.
fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("script");
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// A `SPECENGINE_CARGO` stand-in that never answers (exit 1).
fn silent_cargo(scratch: &Scratch) -> PathBuf {
    let path = scratch.join("cargo-silent.sh");
    script(&path, "echo 'no cargo here' >&2\nexit 1");
    path
}

/// A `SPECENGINE_CARGO` stand-in answering `cargo metadata` for the
/// package at `--manifest-path` (its last argument) with the layout's
/// targets, plus `extra` (JSON target objects, `$dir` the package dir).
fn answering_cargo(scratch: &Scratch, extra: &str) -> PathBuf {
    let path = scratch.join("cargo-answers.sh");
    script(
        &path,
        &format!(
            "for last in \"$@\"; do :; done\n\
             dir=$(dirname \"$last\")\n\
             cat <<EOF\n\
             {{\"workspace_root\":\"$dir\",\"packages\":[{{\"name\":\"inkwell-core\",\"manifest_path\":\"$last\",\"targets\":[\
             {{\"kind\":[\"lib\"],\"name\":\"inkwell_core\",\"src_path\":\"$dir/src/lib.rs\"}},\
             {{\"kind\":[\"bin\"],\"name\":\"inkwell-core\",\"src_path\":\"$dir/src/main.rs\"}},\
             {{\"kind\":[\"bin\"],\"name\":\"stamp\",\"src_path\":\"$dir/src/bin/stamp.rs\"}},\
             {{\"kind\":[\"test\"],\"name\":\"flow\",\"src_path\":\"$dir/tests/flow.rs\"}}{extra}]}}]}}\n\
             EOF"
        ),
    );
    path
}

/// `specengine-eval code-index --out <out>` over the default fixture, the
/// pilot variables removed, `cargo` the stand-in.
fn code_index(out: &Path, cargo: &Path) -> Output {
    code_index_with(out, cargo, &[], &[])
}

/// [`code_index`] with `args` before `--out` and `env` set (after the
/// pilot variables are removed).
fn code_index_with(out: &Path, cargo: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(BIN);
    command.current_dir(repository_root());
    for variable in ENV {
        command.env_remove(variable);
    }
    for (variable, value) in env {
        command.env(variable, value);
    }
    command
        .env("SPECENGINE_CARGO", cargo)
        .arg("code-index")
        .args(args)
        .args([
            "--out",
            out.to_str().expect("UTF-8 scratch"),
            "--timeout",
            "300",
        ])
        .output()
        .expect("specengine-eval runs")
}

fn envelope(output: &Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "exit {:?}, stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    let mut lines = stdout.lines();
    let line = lines.next().expect("one JSON line on stdout");
    assert!(lines.next().is_none(), "one line:\n{stdout}");
    serde_json::from_str(line).unwrap_or_else(|error| panic!("not JSON: {error}\n{line}"))
}

fn fixtures_git_status() -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "fixtures/",
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every file under `dir`, relative, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(&path).expect("readable file"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The fixture's `.rs` and `.ron` files under its `[code]` roots.
fn code_files() -> BTreeMap<String, Vec<u8>> {
    snapshot(&fixture())
        .into_iter()
        .filter(|(path, _)| {
            (path.starts_with("crates/") || path.starts_with("data/"))
                && (path.ends_with(".rs") || path.ends_with(".ron"))
        })
        .collect()
}

/// `sql` on `db` through the system's `sqlite3 -json`.
fn query(db: &Path, sql: &str) -> Vec<BTreeMap<String, Value>> {
    let sqlite3 = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(sqlite3)
        .env_clear()
        .arg("-json")
        .arg(db)
        .arg(sql)
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "{sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    if text.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&text).expect("sqlite3 -json")
}

/// RD-12. M: vacuous timings; `twins`, `signatures` off. The envelope over
/// the fixture: its keys, every count `expected.json`'s, `noop_parsed` 0,
/// `one_file_parsed` 1, `kloc` the `.rs` newlines / 1000, `bytes` the code
/// files' bytes, no path, ID or project word on stdout; a cargo that never
/// answers leaves `roles.metadata` 0 and `differ` `null`; the fixtures
/// untouched. bevy-roles E1: code-index has no `frameworks`, so
/// `framework_roles` is `null`.
#[test]
fn code_index_envelope_counts_the_fixture() {
    let scratch = Scratch::new("envelope");
    let before = fixtures_git_status();
    let output = code_index(&scratch.join("out"), &silent_cargo(&scratch));
    let envelope = envelope(&output);
    assert_eq!(
        envelope
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ["label", "measurement", "result", "versions", "wall_ms"]
            .into_iter()
            .collect()
    );
    assert_eq!(envelope["measurement"], "code-index");
    assert_eq!(envelope["label"], "fixtures");
    let result = &envelope["result"];
    let keys: BTreeSet<&str> = result
        .as_object()
        .expect("result object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, RESULT_KEYS.into_iter().collect(), "result keys");
    let want = &expected()["eval"];
    for key in [
        "rs_files",
        "ron_files",
        "items",
        "twins",
        "signatures",
        "roles",
    ] {
        assert_eq!(result[key], want[key], "result.{key}");
    }
    assert!(
        result["framework_roles"].is_null(),
        "off without `frameworks`: {result}"
    );
    assert!(
        result["roles_ms"].is_null(),
        "no role read with the opt-in off: {result}"
    );
    let kloc = result["kloc"].as_f64().expect("kloc");
    assert!((kloc - 0.156).abs() < 1e-9, "kloc {kloc}");
    let files = code_files();
    let newlines: usize = files
        .iter()
        .filter(|(path, _)| path.ends_with(".rs"))
        .map(|(_, bytes)| bytes.iter().filter(|&&b| b == b'\n').count())
        .sum();
    assert_eq!(newlines, 156, "the fixture's .rs newlines");
    let bytes: usize = files.values().map(Vec::len).sum();
    assert_eq!(result["bytes"], bytes, "the code files' bytes");
    assert_eq!(result["noop_parsed"], 0);
    assert_eq!(result["one_file_parsed"], 1);
    for key in [
        "hash_ms",
        "full_ms",
        "noop_ms",
        "one_file_ms",
        "db_bytes",
        "refs_ms",
    ] {
        assert!(
            result[key].is_u64(),
            "result.{key} is a count: {}",
            result[key]
        );
    }
    assert!(
        result["db_bytes"].as_u64().unwrap() > 0,
        "a database was written"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for leak in [
        "/",
        ".rs",
        ".ron",
        "inkwell",
        "TENET",
        "SLAB",
        "QUILL",
        "Sheet",
        "separator",
        "probe",
    ] {
        assert!(!stdout.contains(leak), "{leak:?} on stdout: {stdout}");
    }
    assert_eq!(fixtures_git_status(), before, "the fixtures are only read");
}

/// RD-12. M: no edit before the one-file timing; the first file edited.
/// The harness's scratch copy (`--out/code-index/fixtures/
/// corpus`) holds every code file as the fixture has it but the largest
/// `.rs` (`expected.json` `largest_rs`, never the first in walk order),
/// which ends with one line more; its database holds `items` symbols.
#[test]
fn one_file_timing_edits_the_largest_rs_and_items_are_stored() {
    let scratch = Scratch::new("one-file");
    let out = scratch.join("out");
    let envelope = envelope(&code_index(&out, &silent_cargo(&scratch)));
    let want = expected();
    let largest = want["eval"]["largest_rs"].as_str().unwrap();
    let first = want["eval"]["first_rs"].as_str().unwrap();
    let files = code_files();
    let by_size = files
        .iter()
        .filter(|(path, _)| path.ends_with(".rs"))
        .max_by_key(|(_, bytes)| bytes.len())
        .map(|(path, _)| path.as_str())
        .unwrap();
    assert_eq!(by_size, largest, "expected.json names the largest .rs");
    assert_ne!(largest, first);
    let copy = snapshot(&out.join("code-index/fixtures/corpus"));
    for (path, bytes) in &files {
        let copied = copy
            .get(path)
            .unwrap_or_else(|| panic!("{path} copied: {:?}", copy.keys()));
        if path == largest {
            assert!(
                copied.len() > bytes.len() && copied.starts_with(bytes),
                "{path}: a line appended"
            );
            let appended = &copied[bytes.len()..];
            assert_eq!(
                appended.iter().filter(|&&b| b == b'\n').count(),
                1,
                "{path}: one line: {:?}",
                String::from_utf8_lossy(appended)
            );
        } else {
            assert_eq!(copied, bytes, "{path}: untouched");
        }
    }
    let db = out.join("code-index/fixtures/index.db");
    let stored = query(&db, "SELECT count(*) AS n FROM symbols");
    assert_eq!(
        stored[0]["n"], envelope["result"]["items"],
        "items are the stored symbols"
    );
}

/// RD-12, Q25. M: `roles` compares layout with layout. A cargo stand-in
/// whose table makes `src/twins.rs` a bin root (another role than the
/// layout's module): `roles.metadata` counts the files whose package
/// answered (all 9), `differ` 1; a stand-in answering the layout's own
/// table: `differ` 0.
#[test]
fn roles_differ_counts_a_changed_role() {
    let scratch = Scratch::new("roles");
    let other = answering_cargo(
        &scratch,
        ",{\"kind\":[\"bin\"],\"name\":\"twins\",\"src_path\":\"$dir/src/twins.rs\"}",
    );
    let changed = envelope(&code_index(&scratch.join("out-changed"), &other));
    assert_eq!(
        changed["result"]["roles"],
        serde_json::json!({"files": 9, "metadata": 9, "differ": 1}),
        "{changed}"
    );
    let same = answering_cargo(&scratch, "");
    let alike = envelope(&code_index(&scratch.join("out-same"), &same));
    assert_eq!(
        alike["result"]["roles"],
        serde_json::json!({"files": 9, "metadata": 9, "differ": 0}),
        "{alike}"
    );
}

/// Q25: no package answered (cargo missing): `differ` is `null`, never 0.
#[test]
fn roles_differ_is_null_without_a_metadata_answer() {
    let scratch = Scratch::new("roles-null");
    let missing = scratch.join("no-such-cargo");
    let envelope = envelope(&code_index(&scratch.join("out"), &missing));
    assert_eq!(envelope["result"]["roles"]["metadata"], 0, "{envelope}");
    assert!(
        envelope["result"]["roles"]["differ"].is_null(),
        "{envelope}"
    );
}

/// The run is a function of the corpus: two runs give the same counts
/// (times aside).
#[test]
fn counts_repeat_byte_for_byte() {
    let scratch = Scratch::new("repeat");
    let cargo = silent_cargo(&scratch);
    let counts = |out: &str| {
        let envelope = envelope(&code_index(&scratch.join(out), &cargo));
        let mut result = envelope["result"].clone();
        let object = result.as_object_mut().unwrap();
        for key in [
            "hash_ms",
            "full_ms",
            "noop_ms",
            "one_file_ms",
            "refs_ms",
            "roles_ms",
            "db_bytes",
        ] {
            object.remove(key);
        }
        result.to_string()
    };
    assert_eq!(counts("one"), counts("two"));
}

// ------------------------------------------------- bevy-roles "Eval" (E1)

/// Copies `fixtures/code-bevy` to `to`, `expected.json` left out, the
/// config's `[code]` (its last table) replaced by `code` (`None`: kept).
fn copy_code_bevy(to: &Path, code: Option<&str>) {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("target");
        for entry in fs::read_dir(from).expect("fixture").flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type").is_dir() {
                copy(&entry.path(), &target);
            } else if entry.file_name() != "expected.json" {
                fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            }
        }
    }
    copy(&repository_root().join("fixtures/code-bevy"), to);
    if let Some(code) = code {
        let config = fs::read_to_string(to.join("specengine.toml")).expect("config");
        let at = config.find("\n[code]\n").expect("a [code] table") + 1;
        fs::write(
            to.join("specengine.toml"),
            format!("{}{code}", &config[..at]),
        )
        .expect("the config");
    }
}

/// bevy-roles E1 (TE-1's figures). M: the scheme's `frameworks` dropped;
/// `framework_roles` not the last refresh's `code.roles`. Over a scratch
/// copy of code-bevy (`--pilot`, its own scheme, `frameworks =
/// ["bevy"]`): `framework_roles` is `expected.json`'s `code_roles`,
/// `roles_ms` a count; stdout names no path, ID or project word; the
/// fixtures untouched.
#[test]
fn framework_roles_count_the_code_bevy_fixture() {
    let scratch = Scratch::new("framework-roles");
    let before = fixtures_git_status();
    let corpus = scratch.join("corpus");
    copy_code_bevy(&corpus, None);
    let corpus_text = corpus.to_str().expect("UTF-8 scratch").to_owned();
    let output = code_index_with(
        &scratch.join("out"),
        &silent_cargo(&scratch),
        &["--pilot", corpus_text.as_str()],
        &[],
    );
    let envelope = envelope(&output);
    let result = &envelope["result"];
    assert_eq!(
        result["framework_roles"],
        bevy_expected()["code_roles"],
        "{result}"
    );
    assert!(result["roles_ms"].is_u64(), "roles_ms a count: {result}");
    assert_eq!(result["items"], 41, "{result}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for leak in ["/", ".rs", "farm", "ALM", "CHORE", "Crop", "spawn_every"] {
        assert!(!stdout.contains(leak), "{leak:?} on stdout: {stdout}");
    }
    assert_eq!(fixtures_git_status(), before, "the fixtures are only read");
}

/// bevy-roles E1 (`harness::PILOT_CODE_FRAMEWORKS`). M: the variable
/// ignored; a framework read from the scheme under a roots variable. Under
/// `--label pilot-a` with `SPECENGINE_CODE_ROOTS_A` (the scheme's own
/// `[code]` without `frameworks`): `SPECENGINE_CODE_FRAMEWORKS_A=bevy`
/// gives `expected.json`'s `code_roles`; unset or empty, `null`; an
/// unknown name is exit 2 naming it.
#[test]
fn pilot_frameworks_variable_joins_the_roots_table() {
    let scratch = Scratch::new("framework-variable");
    let corpus = scratch.join("corpus");
    copy_code_bevy(&corpus, Some("[code]\nroots = [\"crates\", \"assets\"]\n"));
    let corpus_text = corpus.to_str().expect("UTF-8 scratch").to_owned();
    let cargo = silent_cargo(&scratch);
    let run = |out: &str, frameworks: Option<&str>| {
        let mut env = vec![("SPECENGINE_CODE_ROOTS_A", "crates,assets")];
        if let Some(frameworks) = frameworks {
            env.push(("SPECENGINE_CODE_FRAMEWORKS_A", frameworks));
        }
        code_index_with(
            &scratch.join(out),
            &cargo,
            &["--pilot", corpus_text.as_str(), "--label", "pilot-a"],
            &env,
        )
    };
    let on = envelope(&run("on", Some("bevy")));
    assert_eq!(
        on["result"]["framework_roles"],
        bevy_expected()["code_roles"],
        "{on}"
    );
    for (out, frameworks) in [("unset", None), ("empty", Some(""))] {
        let off = envelope(&run(out, frameworks));
        assert!(off["result"]["framework_roles"].is_null(), "{out}: {off}");
    }
    let unknown = run("unknown", Some("loom"));
    assert_eq!(unknown.status.code(), Some(2), "{unknown:?}");
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("loom"),
        "{}",
        String::from_utf8_lossy(&unknown.stderr)
    );
}

/// Review 1 target, built in iteration 2 (B4). M: the frameworks
/// variable ignored without the roots variable (a TE-7 run measuring with
/// roles off by mistake). Under `--label pilot-a`,
/// `SPECENGINE_CODE_FRAMEWORKS_A` set while `SPECENGINE_CODE_ROOTS_A` is
/// not: exit 2, nothing on stdout, the refusal on stderr.
#[test]
fn frameworks_variable_without_roots_is_refused() {
    let scratch = Scratch::new("framework-variable-alone");
    let corpus = scratch.join("corpus");
    copy_code_bevy(&corpus, None);
    let corpus_text = corpus.to_str().expect("UTF-8 scratch").to_owned();
    let output = code_index_with(
        &scratch.join("out"),
        &silent_cargo(&scratch),
        &["--pilot", corpus_text.as_str(), "--label", "pilot-a"],
        &[("SPECENGINE_CODE_FRAMEWORKS_A", "bevy")],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(!output.stderr.is_empty(), "{output:?}");
}
