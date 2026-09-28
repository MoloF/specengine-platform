//! `specengine-eval ra` end to end (AC-11 of docs/features/phase-0-spikes.md on
//! `fixtures/ra-mini`, layer C of 05 §5.1; the read-only guard and the
//! self-terminating per-load timeout of `crates/specengine-eval/README.md`,
//! "Rules"; anonymous stdout, determinism) on the real binary.
//!
//! Compiled only with feature `ra`, whose `ra_ap_*` set needs rustc >= 1.98
//! (stable 1.98.1): `cargo nextest run -p specengine-eval --features ra --test ra_cli`.
//! Default-feature runs build this file empty. The fixture is analysed with
//! its own toolchain (the worker drops `RUSTUP_TOOLCHAIN`), so the proc-macro
//! server is the default toolchain's `rust-analyzer-proc-macro-srv`.

#![cfg(all(feature = "ra", unix))]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

/// Budget of each fixture load: generous for a debug build of `ra_ap_*`.
const FIXTURE_BUDGET_S: &str = "600";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture_dir() -> PathBuf {
    repository_root().join("fixtures").join("ra-mini")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("specengine-eval-ra-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(fs::canonicalize(&path).expect("canonical scratch directory"))
    }

    fn path(&self) -> &Path {
        &self.0
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

fn eval() -> Command {
    let mut command = Command::new(BIN);
    command.current_dir(repository_root());
    for variable in ["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"] {
        command.env_remove(variable);
    }
    command
}

fn run(args: &[&str]) -> Output {
    eval().args(args).output().expect("specengine-eval runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn envelope(output: &Output) -> (String, Value) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "exit {:?}, stderr:\n{}",
        output.status.code(),
        stderr(output)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    let value: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|error| panic!("stdout is one JSON envelope ({error}):\n{stdout}"));
    (stdout, value)
}

/// `ra` on `corpus` with both loads, detail under `out`.
fn run_ra(corpus: &Path, out: &Path, extra: &[&str]) -> Output {
    let corpus = corpus.to_str().expect("UTF-8 corpus path");
    let out = out.to_str().expect("UTF-8 out path");
    let mut args = vec!["ra", "--pilot", corpus, "--out", out, "--label", "fixtures"];
    args.extend_from_slice(extra);
    run(&args)
}

// ---------------------------------------------------------------------------
// Tree snapshots: "nothing written" and "byte-identical" are whole-tree facts.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Dir,
    File {
        bytes: Vec<u8>,
        modified: SystemTime,
    },
    Link(PathBuf),
}

/// Every entry under `root` (hidden ones included, symlinks not followed).
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Entry> {
    let mut entries = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable directory") {
            let entry = entry.expect("directory entry");
            let path = entry.path();
            let relative = path.strip_prefix(root).expect("under root").to_path_buf();
            let file_type = entry.file_type().expect("file type");
            if file_type.is_symlink() {
                entries.insert(relative, Entry::Link(fs::read_link(&path).expect("link")));
            } else if file_type.is_dir() {
                entries.insert(relative, Entry::Dir);
                stack.push(path);
            } else {
                let metadata = entry.metadata().expect("metadata");
                entries.insert(
                    relative,
                    Entry::File {
                        bytes: fs::read(&path).expect("readable file"),
                        modified: metadata.modified().expect("mtime"),
                    },
                );
            }
        }
    }
    entries
}

fn assert_unchanged(root: &Path, before: &BTreeMap<PathBuf, Entry>, context: &str) {
    let after = snapshot(root);
    let added: Vec<&PathBuf> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    let removed: Vec<&PathBuf> = before.keys().filter(|k| !after.contains_key(*k)).collect();
    let changed: Vec<&PathBuf> = before
        .iter()
        .filter(|(k, v)| after.get(*k).is_some_and(|now| now != *v))
        .map(|(k, _)| k)
        .collect();
    assert!(
        added.is_empty() && removed.is_empty() && changed.is_empty(),
        "{context}: {} changed — added {added:?}, removed {removed:?}, modified {changed:?}",
        root.display()
    );
}

/// A copy of `fixtures/ra-mini` at `to` (files and directories only).
fn copy_fixture(to: &Path) {
    let from = fixture_dir();
    for (relative, entry) in snapshot(&from) {
        let target = to.join(&relative);
        match entry {
            Entry::Dir => fs::create_dir_all(&target).expect("copy dir"),
            Entry::File { bytes, .. } => {
                fs::create_dir_all(target.parent().expect("parent")).expect("copy parent");
                fs::write(&target, bytes).expect("copy file");
            }
            Entry::Link(_) => panic!("the fixture holds no symlinks: {}", relative.display()),
        }
    }
}

// ---------------------------------------------------------------------------
// Refusals: exit 2, nothing written anywhere.
// ---------------------------------------------------------------------------

fn assert_refused(output: &Output, needle: &str, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: expected a refusal (exit 2), stderr:\n{}",
        stderr(output)
    );
    assert!(
        output.stdout.is_empty(),
        "{context}: a refusal prints nothing on stdout, got:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr(output).contains(needle),
        "{context}: stderr does not name `{needle}`:\n{}",
        stderr(output)
    );
}

#[test]
fn refuses_a_corpus_without_cargo_toml_and_writes_nothing() {
    let scratch = Scratch::new("no-manifest");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(corpus.join("src")).expect("corpus");
    fs::write(corpus.join("src/lib.rs"), "pub fn lonely() {}\n").expect("source");
    let before = snapshot(scratch.path());
    let out = scratch.join("out");
    let output = run_ra(&corpus, &out, &[]);
    assert_refused(&output, "Cargo.toml", "no Cargo.toml at the corpus root");
    assert!(!out.exists(), "--out was created on a refusal");
    assert_unchanged(scratch.path(), &before, "no Cargo.toml refusal");
}

/// Spellings of `<corpus>/<leaf>` that do not start with the corpus path
/// literally or pass through a directory that does not exist: `missing/..`
/// inside and outside the corpus, a relative path, a symlink to the corpus
/// and `..` after a symlink into the corpus (the OS resolves that `..`
/// physically, a lexical normaliser would land outside).
fn spellings_under_the_corpus(scratch: &Scratch, corpus: &Path, leaf: &str) -> Vec<String> {
    let link = scratch.join("link-to-corpus");
    let deep_link = scratch.join("link-into-corpus");
    if !link.exists() {
        symlink(corpus, &link).expect("symlink to the corpus");
        symlink(corpus.join("arena"), &deep_link).expect("symlink into the corpus");
    }
    let corpus = corpus.to_str().expect("UTF-8");
    let scratch_path = scratch.path().to_str().expect("UTF-8");
    let corpus_name = Path::new(corpus)
        .file_name()
        .and_then(|n| n.to_str())
        .expect("corpus name");
    vec![
        format!("{corpus}/{leaf}"),
        format!("{corpus}/missing/../{leaf}"),
        format!("{scratch_path}/side/missing/../../{corpus_name}/{leaf}"),
        format!("{}/{leaf}", link.display()),
        format!("{}/missing/../{leaf}", link.display()),
        format!("{}/../{leaf}", deep_link.display()),
        // Relative to the command's working directory (the repository root).
        format!(
            "{}/{corpus_name}/{leaf}",
            relative_to(&repository_root(), scratch.path())
        ),
    ]
}

/// `to` as a `../..`-style path relative to `from`.
fn relative_to(from: &Path, to: &Path) -> String {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_owned(); from.len() - common];
    parts.extend(
        to[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[test]
fn refuses_out_under_the_corpus_in_every_spelling_and_writes_nothing() {
    let scratch = Scratch::new("out-under");
    let corpus = scratch.join("corpus");
    copy_fixture(&corpus);
    let spellings = spellings_under_the_corpus(&scratch, &corpus, "ra-out");
    let before = snapshot(scratch.path());
    for out in &spellings {
        let output = run_ra(&corpus, Path::new(out), &[]);
        assert_refused(&output, "lies under the corpus", &format!("--out {out}"));
        assert_unchanged(scratch.path(), &before, &format!("--out {out}"));
    }
}

#[test]
fn refuses_cargo_target_dir_under_the_corpus_in_every_spelling_and_writes_nothing() {
    let scratch = Scratch::new("target-under");
    let corpus = scratch.join("corpus");
    copy_fixture(&corpus);
    let spellings = spellings_under_the_corpus(&scratch, &corpus, "target");
    let out = scratch.join("out");
    let before = snapshot(scratch.path());
    for target in &spellings {
        let output = run_ra(&corpus, &out, &["--cargo-target-dir", target]);
        assert_refused(
            &output,
            "--cargo-target-dir",
            &format!("--cargo-target-dir {target}"),
        );
        assert!(!out.exists(), "--out was created on a refusal ({target})");
        assert_unchanged(
            scratch.path(),
            &before,
            &format!("--cargo-target-dir {target}"),
        );
    }
}

#[test]
fn worker_refuses_target_or_detail_under_the_corpus() {
    let scratch = Scratch::new("worker-under");
    let corpus = scratch.join("corpus");
    copy_fixture(&corpus);
    let before = snapshot(scratch.path());
    let inside = corpus.join("inside");
    let outside = scratch.join("outside");
    for (target, detail) in [(&inside, &outside), (&outside, &inside)] {
        let output = eval()
            .args(["ra-worker", "--root"])
            .arg(&corpus)
            .arg("--cargo-target-dir")
            .arg(target)
            .arg("--detail")
            .arg(detail)
            .args(["--mode", "without", "--budget", "5"])
            .stdin(Stdio::null())
            .output()
            .expect("worker runs");
        assert_refused(&output, "lies under the corpus", "ra-worker guard");
        assert_unchanged(scratch.path(), &before, "ra-worker guard");
    }
}

/// A per-load budget whose deadlines the system clock cannot represent is
/// refused before anything is written, with or without `--pilot`.
#[test]
fn unrepresentable_timeout_is_refused_and_writes_nothing() {
    let fixture = fixture_dir();
    let fixture_before = snapshot(&fixture);
    let scratch = Scratch::new("huge-timeout");
    let corpus = scratch.join("corpus");
    copy_fixture(&corpus);
    let out = scratch.join("out");
    let before = snapshot(scratch.path());
    // u64::MAX, and i64::MAX (still far beyond the clock once doubled).
    for timeout in ["18446744073709551615", "9223372036854775807"] {
        for with_pilot in [true, false] {
            let mut args = vec![
                "ra",
                "--out",
                out.to_str().expect("UTF-8 out"),
                "--timeout",
                timeout,
            ];
            if with_pilot {
                args.extend(["--pilot", corpus.to_str().expect("UTF-8 corpus")]);
            }
            let context = format!("--timeout {timeout} (--pilot: {with_pilot})");
            let output = run(&args);
            assert_refused(&output, "--timeout", &context);
            assert!(!out.exists(), "{context}: --out was created");
            assert_unchanged(scratch.path(), &before, &context);
        }
    }
    assert_unchanged(&fixture, &fixture_before, "unrepresentable --timeout");
}

// ---------------------------------------------------------------------------
// A measured run on the fixture itself.
// ---------------------------------------------------------------------------

fn load<'a>(result: &'a Value, mode: &str) -> &'a Value {
    let load = &result[mode];
    assert!(load.is_object(), "no `{mode}` load object: {result}");
    load
}

/// The group reading of a load that finished (`ok`) on macOS or Linux:
/// `group_peak_rss_mb` a positive number (never 0, never `null` where there is
/// a reading), sampled every 100 ms, and `group_peak_rss_floor_mb` exactly the
/// largest of the sampled group peak, `peak_rss_mb` and
/// `detail.children_peak_rss_mb`. Returns the group peak, MiB.
fn assert_group_rss_of_a_completed_load(mode: &str, load: &Value) -> f64 {
    let detail = &load["detail"];
    assert_eq!(
        detail["group_rss_sample_ms"], 100,
        "{mode}: detail.group_rss_sample_ms: {detail}"
    );
    let group = load["group_peak_rss_mb"]
        .as_f64()
        .filter(|mb| *mb > 0.0)
        .unwrap_or_else(|| {
            panic!(
                "{mode}: group_peak_rss_mb {} is not a positive number",
                load["group_peak_rss_mb"]
            )
        });
    let peak = load["peak_rss_mb"]
        .as_f64()
        .unwrap_or_else(|| panic!("{mode}: peak_rss_mb {}", load["peak_rss_mb"]));
    let children = &detail["children_peak_rss_mb"];
    assert!(
        children.is_null() || children.is_number(),
        "{mode}: detail.children_peak_rss_mb {children}"
    );
    let expected = [Some(group), Some(peak), children.as_f64()]
        .into_iter()
        .flatten()
        .fold(f64::NEG_INFINITY, f64::max);
    assert_eq!(
        load["group_peak_rss_floor_mb"].as_f64(),
        Some(expected),
        "{mode}: group_peak_rss_floor_mb must be max(group {group}, peak {peak}, children \
         {children}): {load}"
    );
    group
}

/// Without `--pilot` and `--label`, `ra` measures `fixtures/ra-mini` under
/// the label `fixtures`; a second run into the same `--out` reuses the first
/// run's target directory, which `target_dir_fresh` reports.
#[test]
fn default_run_on_ra_mini_is_read_only_and_loads_proc_macros_only_with_the_server() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    assert!(
        before.contains_key(Path::new("Cargo.lock")),
        "the fixture carries its own Cargo.lock"
    );
    let scratch = Scratch::new("fixture");
    let out = scratch.join("out");
    let out_arg = out.to_str().expect("UTF-8 out path");
    let output = run(&["ra", "--out", out_arg, "--timeout", FIXTURE_BUDGET_S]);
    let (_, envelope) = envelope(&output);

    // Read-only: byte-identical (mtimes too), its lock included, no `target/`.
    assert_unchanged(&fixture, &before, "ra on fixtures/ra-mini");
    assert!(
        !fixture.join("target").exists(),
        "a target/ appeared in the fixture"
    );

    assert_eq!(envelope["measurement"], "ra");
    assert_eq!(envelope["label"], "fixtures");
    let result = &envelope["result"];
    assert_eq!(result["ra_ap"], "0.0.352");
    assert_eq!(result["timeout_s"], 600);
    // The test build of the harness has debug assertions on.
    assert_eq!(result["profile"], "debug", "{result}");

    for mode in ["without", "with"] {
        let load = load(result, mode);
        assert_eq!(load["status"], "ok", "{mode}: {load}");
        let detail = &load["detail"];
        assert_eq!(
            detail["target_dir_fresh"], true,
            "{mode}: a new --out has no build output yet: {detail}"
        );
        assert!(detail["exit"].is_null(), "{mode}: {detail}");
        assert_eq!(
            detail["metadata"]["metadata_degraded"], false,
            "{mode}: the fixture's lock resolves under --locked --offline: {detail}"
        );
        assert_eq!(
            detail["metadata"]["sysroot_error"], false,
            "{mode}: {detail}"
        );
        assert!(detail["load_error"].is_null(), "{mode}: {detail}");
        assert!(
            load["cold_ms"].is_u64(),
            "{mode}: cold_ms {}",
            load["cold_ms"]
        );
        assert!(
            load["warm_ms"].is_u64(),
            "{mode}: warm_ms {}",
            load["warm_ms"]
        );
        assert!(
            load["peak_rss_mb"].as_f64().is_some_and(|mb| mb > 0.0),
            "{mode}: peak_rss_mb {}",
            load["peak_rss_mb"]
        );
        assert_group_rss_of_a_completed_load(mode, load);
        assert_eq!(load["panics"], 0, "{mode}");
        let counts = &detail["first_pass"]["counts"];
        assert_eq!(
            counts["files"], 6,
            "{mode}: the fixture's .rs files: {counts}"
        );
        assert_eq!(
            counts["by_status"]["not_loaded"], 1,
            "{mode}: tools/probe.rs lies outside every package: {counts}"
        );
        // 29 source items: 26 with a moniker; unresolved: the `#[cfg(windows)]`
        // function and the one in the file outside the module tree; not loaded:
        // the file outside every package.
        assert_eq!(load["items"], 29, "{mode}: {counts}");
        assert_eq!(load["items_with_moniker"], 26, "{mode}: {counts}");
        assert_eq!(load["moniker_pct"], 89.7, "{mode}: {load}");
        assert_eq!(counts["by_status"]["unresolved"], 2, "{mode}: {counts}");
        assert_eq!(counts["files_loaded"], 5, "{mode}: {counts}");
        assert_eq!(counts["files_in_crate"], 4, "{mode}: {counts}");

        let detail_dir = out.join("ra").join("fixtures").join(mode);
        for file in ["items.json", "errors.json", "warm.json"] {
            assert!(
                detail_dir.join(file).is_file(),
                "{mode}: detail file {file} missing under --out"
            );
        }
        let items: Value = serde_json::from_str(
            &fs::read_to_string(detail_dir.join("items.json")).expect("items.json"),
        )
        .expect("items.json is JSON");
        let status_of = |path: &str, name: &str| -> String {
            items
                .as_array()
                .expect("items.json is a list")
                .iter()
                .find(|item| item["path"] == path && item["name"] == name)
                .unwrap_or_else(|| panic!("{mode}: no item {path} {name}"))["status"]
                .as_str()
                .expect("status")
                .to_owned()
        };
        assert_eq!(
            status_of("arena/src/lib.rs", "windows_overlay"),
            "unresolved",
            "{mode}"
        );
        assert_eq!(
            status_of("arena/src/stray.rs", "stray_helper"),
            "unresolved",
            "{mode}"
        );
        assert_eq!(status_of("tools/probe.rs", "main"), "not_loaded", "{mode}");
        assert_eq!(status_of("arena/src/lib.rs", "Health"), "moniker", "{mode}");
        assert_eq!(
            status_of("arena-derive/src/lib.rs", "derive_label"),
            "moniker",
            "{mode}"
        );
    }

    let with = &load(result, "with")["detail"];
    assert_eq!(with["database"]["proc_macro_server"], "running", "{with}");
    assert!(
        with["database"]["proc_macro_crates_loaded"]
            .as_u64()
            .is_some_and(|n| n >= 1),
        "the with load loads the fixture's derive crate: {with}"
    );
    assert_eq!(with["build_scripts"]["ran"], true, "{with}");
    assert_eq!(with["build_scripts"]["errors"], false, "{with}");

    let without = &load(result, "without")["detail"];
    assert_eq!(
        without["database"]["proc_macro_server"], "disabled",
        "{without}"
    );
    assert_eq!(
        without["database"]["proc_macro_crates_loaded"], 0,
        "the without load loads no proc-macro crate: {without}"
    );
    assert!(
        without["build_scripts"].is_null(),
        "the without load runs no build script: {without}"
    );

    // cargo wrote only under --out (the default target directory).
    assert!(
        out.join("ra").join("fixtures").join("target").is_dir(),
        "the default cargo target directory is <out>/ra/<label>/target"
    );

    // A re-run into the same --out: the target directory holds the first
    // run's build output, so neither load starts from a fresh one.
    let output = run(&["ra", "--out", out_arg, "--timeout", FIXTURE_BUDGET_S]);
    let (_, again) = crate::envelope(&output);
    assert_eq!(again["label"], "fixtures");
    for mode in ["without", "with"] {
        let load = load(&again["result"], mode);
        assert_eq!(load["status"], "ok", "re-run {mode}: {load}");
        assert_eq!(load["items"], 29, "re-run {mode}: {load}");
        assert_eq!(
            load["detail"]["target_dir_fresh"], false,
            "re-run {mode}: the target directory holds the first run's output: {load}"
        );
    }
    assert_unchanged(&fixture, &before, "ra re-run on fixtures/ra-mini");
}

/// `target_dir_fresh` of an explicit `--cargo-target-dir`, decided before the
/// load starts (so a one-second budget suffices): absent, empty or holding only
/// cargo's bookkeeping files is fresh; anything else is not; an unreadable
/// directory is `null`.
#[test]
fn explicit_cargo_target_dir_freshness_is_decided_by_its_contents() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let scratch = Scratch::new("target-fresh");
    let cases: [(&str, &[&str], &[&str], Value); 6] = [
        ("absent", &[], &[], Value::Bool(true)),
        ("empty", &[], &[], Value::Bool(true)),
        (
            "bookkeeping",
            &[".rustc_info.json", "CACHEDIR.TAG"],
            &[],
            Value::Bool(true),
        ),
        (
            "bookkeeping-and-debug",
            &[".rustc_info.json", "CACHEDIR.TAG"],
            &["debug"],
            Value::Bool(false),
        ),
        ("one-file", &["leftover"], &[], Value::Bool(false)),
        ("unreadable", &[], &[], Value::Null),
    ];
    for (name, files, dirs, expected) in cases {
        let target = scratch.join(&format!("target-{name}"));
        if name != "absent" {
            fs::create_dir_all(&target).expect("target dir");
        }
        for file in files {
            fs::write(target.join(file), "{}").expect("target file");
        }
        for dir in dirs {
            fs::create_dir_all(target.join(dir)).expect("target subdir");
        }
        if name == "unreadable" {
            fs::set_permissions(&target, fs::Permissions::from_mode(0o000)).expect("chmod 000");
        }
        let out = scratch.join(&format!("out-{name}"));
        let output = run_ra(
            &fixture,
            &out,
            &[
                "--cargo-target-dir",
                target.to_str().expect("UTF-8 target"),
                "--proc-macros",
                "without",
                "--timeout",
                "1",
            ],
        );
        if name == "unreadable" {
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("chmod back");
        }
        let (_, envelope) = envelope(&output);
        let without = load(&envelope["result"], "without");
        assert_eq!(
            without["detail"]["target_dir_fresh"], expected,
            "--cargo-target-dir {name}: {without}"
        );
    }
    assert_unchanged(&fixture, &before, "ra with explicit target directories");
}

/// The envelope with every time and memory field removed: what must repeat.
fn stable_fields(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| {
                    !(key.as_str() == "ms" || key.ends_with("_ms") || key.ends_with("_mb"))
                })
                .map(|(key, value)| (key.clone(), stable_fields(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(stable_fields).collect()),
        other => other.clone(),
    }
}

/// Every item name the detail files of `out` record.
fn item_names(out: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for mode in ["with", "without"] {
        let path = out
            .join("ra")
            .join("fixtures")
            .join(mode)
            .join("items.json");
        let text = fs::read_to_string(&path).expect("items.json");
        collect_names(
            &serde_json::from_str(&text).expect("items.json is JSON"),
            &mut names,
        );
    }
    names
}

fn collect_names(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "name"
                    && let Some(name) = value.as_str()
                {
                    names.insert(name.to_owned());
                }
                collect_names(value, names);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_names(item, names)),
        _ => {}
    }
}

#[test]
fn stdout_is_anonymous_and_non_time_fields_repeat_across_runs() {
    let fixture = fixture_dir();
    let scratch = Scratch::new("determinism");
    let mut runs = Vec::new();
    for index in 0..2 {
        let out = scratch.join(&format!("out-{index}"));
        let output = run_ra(&fixture, &out, &["--timeout", FIXTURE_BUDGET_S]);
        let (stdout, envelope) = envelope(&output);
        runs.push((out, stdout, envelope));
    }

    for (out, stdout, _) in &runs {
        // No absolute or relative path of any kind, no machine-specific string.
        assert!(
            !stdout.contains('/') && !stdout.contains('\\'),
            "stdout carries a path:\n{stdout}"
        );
        for needle in [
            fixture.to_string_lossy().into_owned(),
            out.to_string_lossy().into_owned(),
            "ra-mini".to_owned(),
            "arena".to_owned(),
        ] {
            assert!(
                !stdout.contains(&needle),
                "stdout names `{needle}`:\n{stdout}"
            );
        }
        let names = item_names(out);
        for probe in [
            "Health",
            "regenerate",
            "stray_helper",
            "derive_label",
            "windows_overlay",
        ] {
            assert!(
                names.contains(probe),
                "items.json under --out lacks `{probe}` (names: {names:?})"
            );
        }
        let leaked: Vec<&String> = names
            .iter()
            .filter(|name| {
                stdout.contains(&format!("\"{name}\""))
                    || (name.len() >= 5 && stdout.contains(name.as_str()))
            })
            .collect();
        assert!(
            leaked.is_empty(),
            "item names on stdout: {leaked:?}\n{stdout}"
        );
    }

    let first = stable_fields(&runs[0].2);
    let second = stable_fields(&runs[1].2);
    assert_eq!(
        first, second,
        "non-time fields differ between two runs:\nfirst:  {first}\nsecond: {second}"
    );
    // The detail files repeat too (item order, statuses, monikers).
    for mode in ["with", "without"] {
        let read = |out: &Path| {
            fs::read(
                out.join("ra")
                    .join("fixtures")
                    .join(mode)
                    .join("items.json"),
            )
            .expect("items.json")
        };
        assert!(
            read(&runs[0].0) == read(&runs[1].0),
            "{mode}/items.json differs between two runs"
        );
    }
}

// ---------------------------------------------------------------------------
// The per-load budget: `"timeout"` fields, and no process of the load survives.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Process {
    pid: u32,
    ppid: u32,
    pgid: u32,
    rss_kib: u64,
    command: String,
}

fn processes() -> Vec<Process> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,ppid=,pgid=,rss=,command="])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok()?;
            let pgid = parts.next()?.parse().ok()?;
            let rss_kib = parts.next()?.parse().ok()?;
            let command = parts.collect::<Vec<_>>().join(" ");
            Some(Process {
                pid,
                ppid,
                pgid,
                rss_kib,
                command,
            })
        })
        .collect()
}

/// What a polled run saw: the workers (by `tag` in their arguments), their
/// process groups and every descendant observed while the run lasted.
#[derive(Default, Debug)]
struct Observed {
    workers: BTreeSet<u32>,
    groups: BTreeSet<u32>,
    /// Every command line seen per pid: a process caught mid-`exec` shows as
    /// `(name)` first, its full command line on a later poll.
    descendants: BTreeMap<u32, BTreeSet<String>>,
    /// Largest RSS (KiB) sampled per descendant.
    peak_rss_kib: BTreeMap<u32, u64>,
}

impl Observed {
    fn saw(&self, needle: &str) -> bool {
        self.descendants
            .values()
            .flatten()
            .any(|command| command.contains(needle))
    }
}

/// The worker's own backstop kills its group `GRACE` = 10 s past its budget
/// (`ra/worker.rs`); a harness that returns well before that killed the load
/// itself. `loads` budgets of `budget_s` each, plus a margin for a busy machine.
fn assert_killed_by_the_harness(elapsed: Duration, loads: u64, budget_s: u64) {
    let bound = Duration::from_secs(loads * budget_s + 7);
    assert!(
        elapsed < bound,
        "{loads} load(s) of {budget_s} s took {elapsed:?} (bound {bound:?}): the harness did \
         not kill the load at its budget; only the worker's own backstop ended it"
    );
}

/// Runs `command` to completion while polling the process table.
fn run_observed(mut command: Command, tag: &str) -> (Output, Observed) {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("specengine-eval starts");
    let harness = child.id();
    let (sender, receiver) = std::sync::mpsc::channel();
    let waiter = thread::spawn(move || {
        let output = child.wait_with_output().expect("specengine-eval exits");
        let _ = sender.send(());
        output
    });
    let mut observed = Observed::default();
    loop {
        let table = processes();
        for process in &table {
            if process.ppid == harness
                && process.command.contains("ra-worker")
                && process.command.contains(tag)
            {
                observed.workers.insert(process.pid);
                observed.groups.insert(process.pgid);
            }
        }
        // Descendants of any worker seen so far, through the parent links of this poll.
        let mut frontier: Vec<u32> = observed.workers.iter().copied().collect();
        let mut visited: BTreeSet<u32> = frontier.iter().copied().collect();
        while let Some(parent) = frontier.pop() {
            for process in table.iter().filter(|p| p.ppid == parent) {
                let commands = observed.descendants.entry(process.pid).or_default();
                commands.insert(process.command.clone());
                let peak = observed.peak_rss_kib.entry(process.pid).or_default();
                *peak = (*peak).max(process.rss_kib);
                if !visited.contains(&process.pid) {
                    visited.insert(process.pid);
                    frontier.push(process.pid);
                }
            }
        }
        for process in table.iter().filter(|p| observed.groups.contains(&p.pgid)) {
            observed
                .descendants
                .entry(process.pid)
                .or_default()
                .insert(process.command.clone());
            let peak = observed.peak_rss_kib.entry(process.pid).or_default();
            *peak = (*peak).max(process.rss_kib);
        }
        if receiver.recv_timeout(Duration::from_millis(50)).is_ok() {
            break;
        }
    }
    (waiter.join().expect("waiter thread"), observed)
}

/// Waits up to five seconds for every observed worker, group member and
/// descendant to be gone; the survivors otherwise. Matched by pid: macOS
/// allocates pids in sequence, so none is reused within seconds.
fn survivors(observed: &Observed) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let alive: Vec<String> = processes()
            .into_iter()
            .filter(|p| {
                observed.workers.contains(&p.pid)
                    || observed.groups.contains(&p.pgid)
                    || observed.descendants.contains_key(&p.pid)
            })
            .map(|p| format!("{} (pgid {}): {}", p.pid, p.pgid, p.command))
            .collect();
        if alive.is_empty() || Instant::now() >= deadline {
            return alive;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn one_second_budget_reports_timeout_fields_and_leaves_no_process() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let scratch = Scratch::new("timeout");
    let out = scratch.join("out");
    let mut command = eval();
    command
        .args(["ra", "--pilot"])
        .arg(&fixture)
        .arg("--out")
        .arg(&out)
        .args(["--label", "fixtures", "--timeout", "1"]);
    let started = Instant::now();
    let (output, observed) = run_observed(command, &out.to_string_lossy());
    let elapsed = started.elapsed();
    let (_, envelope) = envelope(&output);
    assert!(
        !observed.workers.is_empty(),
        "no ra-worker was observed during the run"
    );
    assert_killed_by_the_harness(elapsed, 2, 1);
    let result = &envelope["result"];
    assert_eq!(result["timeout_s"], 1);
    for mode in ["without", "with"] {
        let load = load(result, mode);
        assert_eq!(load["status"], "timeout", "{mode}: {load}");
        for field in [
            "cold_ms",
            "peak_rss_mb",
            "warm_ms",
            "moniker_pct",
            "items",
            "items_with_moniker",
            "panics",
        ] {
            assert_eq!(load[field], "timeout", "{mode}.{field}: {load}");
        }
        // The group's peak of an unfinished load is not known: `"timeout"`
        // (the first sample is taken at the worker's start, so a macOS or
        // Linux run always has one — never `null` here, never 0).
        assert_eq!(
            load["group_peak_rss_mb"], "timeout",
            "{mode}.group_peak_rss_mb: {load}"
        );
        assert_eq!(
            load["detail"]["group_rss_sample_ms"], 100,
            "{mode}.detail.group_rss_sample_ms: {load}"
        );
        // The floor keeps the peak sampled until the kill: a number even on
        // timeout (`null` only when nothing at all was read).
        let floor = &load["group_peak_rss_floor_mb"];
        assert!(
            floor.is_null() || floor.as_f64().is_some_and(|mb| mb > 0.0),
            "{mode}.group_peak_rss_floor_mb is neither a positive number nor null: {load}"
        );
        assert!(
            floor.is_number(),
            "{mode}: group_peak_rss_mb is \"timeout\", so at least one group sample was \
             read, yet the floor that keeps it is {floor}: {load}"
        );
    }
    let alive = survivors(&observed);
    assert!(
        alive.is_empty(),
        "processes of a timed-out load survived (seen: {:?}):\n{}",
        observed.descendants,
        alive.join("\n")
    );
    assert_unchanged(&fixture, &before, "timed-out ra on fixtures/ra-mini");
}

/// A build script that outlives the budget (it sleeps; the self-terminating
/// form, so a failed kill still ends on its own): the budget kills cargo and
/// the running build script with the worker.
#[test]
fn budget_overrun_kills_a_running_build_script_with_the_worker() {
    let scratch = Scratch::new("slow-build-script");
    let corpus = scratch.join("corpus");
    copy_fixture(&corpus);
    fs::write(
        corpus.join("arena/build.rs"),
        "fn main() {\n    std::thread::sleep(std::time::Duration::from_secs(120));\n}\n",
    )
    .expect("slow build script");
    let before = snapshot(&corpus);
    let out = scratch.join("out");
    let mut command = eval();
    command
        .args(["ra", "--pilot"])
        .arg(&corpus)
        .arg("--out")
        .arg(&out)
        .args([
            "--label",
            "fixtures",
            "--proc-macros",
            "with",
            "--timeout",
            "20",
        ]);
    let started = Instant::now();
    let (output, observed) = run_observed(command, &out.to_string_lossy());
    assert_killed_by_the_harness(started.elapsed(), 1, 20);
    let (_, envelope) = envelope(&output);
    let with = load(&envelope["result"], "with");
    assert_eq!(with["status"], "timeout", "{with}");
    assert!(
        with["detail"]["metadata"].is_object(),
        "the metadata step finished inside the budget: {with}"
    );
    assert!(
        with["detail"]["build_scripts"].is_null(),
        "the build-script step never finished: {with}"
    );
    assert!(
        envelope["result"]["without"].is_null(),
        "only `with` was requested"
    );
    assert!(
        observed.saw("build-script-build"),
        "the sleeping build script was never observed running (seen: {:?})",
        observed.descendants
    );
    let alive = survivors(&observed);
    assert!(
        alive.is_empty(),
        "processes of the timed-out load survived:\n{}",
        alive.join("\n")
    );
    assert_unchanged(&corpus, &before, "timed-out ra on a copy of the fixture");
}

/// An overrun in the first pass of the `with` load, when the proc-macro
/// server is up (the debug build's pass over the fixture takes far longer
/// than the budget): the server dies with the worker.
#[test]
fn budget_overrun_in_the_first_pass_kills_the_proc_macro_server() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let scratch = Scratch::new("server-alive");
    let out = scratch.join("out");
    let mut command = eval();
    command
        .args(["ra", "--pilot"])
        .arg(&fixture)
        .arg("--out")
        .arg(&out)
        .args([
            "--label",
            "fixtures",
            "--proc-macros",
            "with",
            "--timeout",
            "6",
        ]);
    let started = Instant::now();
    let (output, observed) = run_observed(command, &out.to_string_lossy());
    assert_killed_by_the_harness(started.elapsed(), 1, 6);
    let (_, envelope) = envelope(&output);
    let with = load(&envelope["result"], "with");
    assert_eq!(with["status"], "timeout", "{with}");
    assert_eq!(
        with["detail"]["database"]["proc_macro_server"], "running",
        "the database step (server started) finished inside the budget: {with}"
    );
    assert!(
        with["detail"]["first_pass"].is_null(),
        "the first pass never finished: {with}"
    );
    // `rust-analyzer-proc-macro-srv`, or its truncated name `(rust-analyzer-pr)`
    // while caught mid-`exec`.
    assert!(
        observed.saw("rust-analyzer-pr"),
        "the proc-macro server was never observed running (seen: {:?})",
        observed.descendants
    );
    let alive = survivors(&observed);
    assert!(
        alive.is_empty(),
        "processes of the timed-out load survived:\n{}",
        alive.join("\n")
    );
    assert_unchanged(&fixture, &before, "timed-out ra on fixtures/ra-mini");
}

/// Ballast of the proc-macro server in `with_load_reaps_the_proc_macro_server_before_its_report`.
const BALLAST_MIB: u64 = 512;

/// The fixture's derive, preceded by a one-off allocation of `BALLAST_MIB`
/// that only the proc-macro server makes (never rustc), and keeps.
fn derive_with_server_ballast() -> String {
    format!(
        r#"use proc_macro::{{TokenStream, TokenTree}};

fn server_ballast() {{
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {{
        let in_server = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.file_name().map(|name| name.to_string_lossy().contains("proc-macro-srv")))
            .unwrap_or(false);
        if in_server {{
            let ballast = vec![0xA5u8; {BALLAST_MIB} << 20];
            std::hint::black_box(&ballast);
            std::mem::forget(ballast);
        }}
    }});
}}

#[proc_macro_derive(Label)]
pub fn derive_label(input: TokenStream) -> TokenStream {{
    server_ballast();
    let mut tokens = input.into_iter();
    let mut name = None;
    while let Some(token) = tokens.next() {{
        if let TokenTree::Ident(ident) = &token
            && ident.to_string() == "struct"
            && let Some(TokenTree::Ident(next)) = tokens.next()
        {{
            name = Some(next.to_string());
            break;
        }}
    }}
    let name = name.unwrap_or_else(|| "Unknown".to_owned());
    format!("impl {{name}} {{{{ pub fn label(&self) -> &'static str {{{{ \"{{name}}\" }}}} }}}}")
        .parse()
        .unwrap_or_default()
}}
"#
    )
}

/// A completed `with` load: the worker reaps the proc-macro server before it
/// reads `RUSAGE_CHILDREN`, so `children_peak_rss_mb` covers the server — made
/// visible by a ballast only the server allocates, larger than anything else
/// the worker starts — and no server survives the worker's report.
#[test]
fn with_load_reaps_the_proc_macro_server_before_its_report() {
    let scratch = Scratch::new("server-reaped");
    let corpus = scratch.join("corpus");
    copy_fixture(&corpus);
    fs::write(
        corpus.join("arena-derive/src/lib.rs"),
        derive_with_server_ballast(),
    )
    .expect("derive with ballast");
    let before = snapshot(&corpus);
    let out = scratch.join("out");
    let mut command = eval();
    command
        .args(["ra", "--pilot"])
        .arg(&corpus)
        .arg("--out")
        .arg(&out)
        .args([
            "--label",
            "fixtures",
            "--proc-macros",
            "with",
            "--timeout",
            FIXTURE_BUDGET_S,
        ]);
    let (output, observed) = run_observed(command, &out.to_string_lossy());
    // Right after the harness returns, before any grace: no server is left.
    let servers: BTreeSet<u32> = observed
        .descendants
        .iter()
        .filter(|(_, commands)| commands.iter().any(|c| c.contains("rust-analyzer-pr")))
        .map(|(pid, _)| *pid)
        .collect();
    let right_after: Vec<String> = processes()
        .into_iter()
        .filter(|p| servers.contains(&p.pid))
        .map(|p| format!("{} (pgid {}): {}", p.pid, p.pgid, p.command))
        .collect();
    let (_, result) = envelope(&output);
    let with = load(&result["result"], "with");
    assert_eq!(with["status"], "ok", "{with}");
    // The fixture's 29 items plus `server_ballast`.
    assert_eq!(with["items"], 30, "{with}");
    assert!(
        with["detail"]["exit"].is_null(),
        "the worker exited on its own after its report: {with}"
    );
    assert!(
        !servers.is_empty(),
        "the proc-macro server was never observed (seen: {:?})",
        observed.descendants
    );
    assert!(
        right_after.is_empty(),
        "a proc-macro server outlived the run:\n{}",
        right_after.join("\n")
    );

    let ballast_kib = BALLAST_MIB * 1024;
    let server_peak = servers
        .iter()
        .filter_map(|pid| observed.peak_rss_kib.get(pid))
        .max()
        .copied()
        .unwrap_or(0);
    assert!(
        server_peak >= ballast_kib,
        "the server's sampled RSS {server_peak} KiB never reached the {BALLAST_MIB} MiB ballast"
    );
    let others: Vec<(u32, u64)> = observed
        .peak_rss_kib
        .iter()
        .filter(|(pid, _)| !servers.contains(pid) && !observed.workers.contains(pid))
        .map(|(pid, kib)| (*pid, *kib))
        .collect();
    assert!(
        others.iter().all(|(_, kib)| *kib < ballast_kib),
        "inconclusive: another child of the worker reached the ballast size: {others:?}"
    );
    let children_mib = with["detail"]["children_peak_rss_mb"]
        .as_f64()
        .unwrap_or_else(|| panic!("children_peak_rss_mb is not a number: {with}"));
    assert!(
        children_mib >= BALLAST_MIB as f64,
        "children_peak_rss_mb {children_mib} < the server's {BALLAST_MIB} MiB ballast: the \
         server was not reaped before RUSAGE_CHILDREN was read ({with})"
    );
    // The verdict's measure sums the live processes of the worker's group: the
    // running server, holding its ballast, is in the sum.
    let group_mib = assert_group_rss_of_a_completed_load("with", with);
    // A sum of the worker alone never exceeds the worker's own peak; the
    // server holds its ballast while the worker runs, so the group's sum must
    // exceed it — whatever the worker's own size relative to the ballast.
    let worker_mib = with["peak_rss_mb"].as_f64().expect("peak_rss_mb");
    assert!(
        group_mib > worker_mib,
        "group_peak_rss_mb {group_mib} <= the worker's own peak_rss_mb {worker_mib}: nothing \
         but the worker is in the group sum ({with})"
    );
    assert!(
        group_mib >= BALLAST_MIB as f64,
        "group_peak_rss_mb {group_mib} < the server's {BALLAST_MIB} MiB ballast: the live \
         proc-macro server is missing from the group sum ({with})"
    );

    let alive = survivors(&observed);
    assert!(
        alive.is_empty(),
        "processes of the load survived:\n{}",
        alive.join("\n")
    );
    assert_unchanged(&corpus, &before, "ra with a ballast derive");
}
