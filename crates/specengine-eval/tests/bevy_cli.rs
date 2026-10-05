//! `specengine-eval bevy-detector` end to end (docs/features/phase-0-spikes.md,
//! layers A and B of 05 §5.1: AC-10 detector counts and dump comparison,
//! AC-02 read-only guard, AC-13 aggregates only on stdout) on the real binary
//! over `fixtures/bevy-mini` and over generated corpora and dumps.
//!
//! Pilot tests are `#[ignore]`: the corpus comes from `SPECENGINE_PILOT_A` /
//! `_B`, the schedule dump of its instrumented scratch copy (optional) from
//! `SPECENGINE_PILOT_A_DUMP` / `_B_DUMP`. No corpus path, name or system name
//! is written anywhere in this file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

const PILOT_VARIABLES: &[&str] = &[
    "SPECENGINE_PILOT_A",
    "SPECENGINE_PILOT_B",
    "SPECENGINE_PILOT_A_DUMP",
    "SPECENGINE_PILOT_B_DUMP",
];

/// The eight miss categories of the comparison, in classification order.
const MISS_CATEGORIES: &[&str] = &[
    "generic_instance",
    "repeated_site",
    "other_schedule",
    "macro_rules",
    "macro_call",
    "closure",
    "indirect",
    "not_in_source",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture_dir() -> PathBuf {
    repository_root().join("fixtures").join("bevy-mini")
}

/// The fixture dump, relative to the repository root (the binary's cwd).
const FIXTURE_DUMP: &str = "fixtures/bevy-mini/app_data.ron";

fn expected() -> Value {
    read_json(&fixture_dir().join("expected.json"))
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-bevy-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        // Canonical, so paths compare with what the harness prints.
        Self(path.canonicalize().expect("scratch canonical"))
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
    for variable in PILOT_VARIABLES {
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

fn envelope(output: &Output) -> Value {
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

/// Byte-for-byte snapshot of a directory tree.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
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
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target directory");
    for entry in fs::read_dir(from).expect("readable") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            // The bytes, not `fs::copy`: on macOS it clones, which a sandbox refuses.
            fs::write(target, fs::read(entry.path()).expect("read")).expect("copy");
        }
    }
}

fn git_status(dir: &Path, pathspec: Option<&str>) -> String {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(["--no-optional-locks", "status", "--porcelain"]);
    if let Some(pathspec) = pathspec {
        command.arg("--").arg(pathspec);
    }
    let output = command.output().expect("git runs");
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
}

fn count(value: &Value, what: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{what} must be a count, got {value}"))
}

/// Every `*.json` under `dir`, parsed, by file name.
fn out_files(dir: &Path) -> BTreeMap<String, Value> {
    fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.expect("entry").path())
        .map(|path| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                read_json(&path),
            )
        })
        .collect()
}

/// `result` without its timing (`detail.detector_ms`) and without
/// `detail.bytes`, which is returned.
fn strip_timing(result: &Value) -> (Value, u64) {
    let mut result = result.clone();
    let detail = result["detail"]
        .as_object_mut()
        .expect("result.detail is an object");
    count(
        &detail.remove("detector_ms").expect("detail.detector_ms"),
        "detector_ms",
    );
    let bytes = count(&detail.remove("bytes").expect("detail.bytes"), "bytes");
    (result, bytes)
}

/// The envelope without `wall_ms` and `result.detail.detector_ms`.
fn without_timing(envelope: &Value) -> Value {
    let mut envelope = envelope.clone();
    envelope
        .as_object_mut()
        .unwrap()
        .remove("wall_ms")
        .expect("wall_ms present");
    envelope["result"]["detail"]
        .as_object_mut()
        .unwrap()
        .remove("detector_ms")
        .expect("detector_ms present");
    envelope
}

fn assert_envelope_shape(envelope: &Value, label: &str) {
    assert_eq!(envelope["measurement"], "bevy-detector");
    assert_eq!(envelope["label"], label);
    assert_eq!(
        envelope["versions"]["tree-sitter"],
        specengine_code::grammar::TREE_SITTER_VERSION
    );
    assert_eq!(
        envelope["versions"]["tree-sitter-rust"],
        specengine_code::grammar::TREE_SITTER_RUST_VERSION
    );
    count(&envelope["versions"]["abi"], "abi");
    count(&envelope["wall_ms"], "wall_ms");
}

// ----------------------------------------------------- stdout: counts only

fn collect_keys(value: &Value, keys: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                keys.insert(key.clone());
                collect_keys(value, keys);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_keys(item, keys)),
        _ => {}
    }
}

/// The keys stdout may carry: the envelope's and those of the expected
/// fixture `result` and `dump` (every map there lists all its keys, zeros
/// included).
fn allowed_keys() -> BTreeSet<String> {
    let expected = expected();
    let mut keys: BTreeSet<String> = [
        "measurement",
        "label",
        "versions",
        "tree-sitter",
        "tree-sitter-rust",
        "abi",
        "wall_ms",
        "result",
        "detector_ms",
        "bytes",
    ]
    .iter()
    .map(|k| (*k).to_owned())
    .collect();
    collect_keys(&expected["result"], &mut keys);
    collect_keys(&expected["dump"], &mut keys);
    keys
}

/// stdout is counts only: every key is a schema key, every string value one
/// of the fixed strings of the envelope. No file, schedule, system, plugin or
/// crate name can pass this.
fn assert_stdout_whitelist(envelope: &Value, label: &str) {
    let keys = allowed_keys();
    let strings: BTreeSet<&str> = [
        "bevy-detector",
        label,
        specengine_code::grammar::TREE_SITTER_VERSION,
        specengine_code::grammar::TREE_SITTER_RUST_VERSION,
        "schedule+name",
        "bevy_dev_tools 0.19 schedule_data",
    ]
    .into_iter()
    .collect();
    fn walk(value: &Value, keys: &BTreeSet<String>, strings: &BTreeSet<&str>, at: &str) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    assert!(
                        keys.contains(key),
                        "stdout key `{key}` at {at} is not a schema key"
                    );
                    walk(value, keys, strings, &format!("{at}.{key}"));
                }
            }
            Value::Array(items) => {
                panic!("stdout carries no lists (lists hold names), got {at} = {items:?}")
            }
            Value::String(text) => assert!(
                strings.contains(text.as_str()),
                "stdout string `{text}` at {at} is not a fixed envelope string"
            ),
            _ => {}
        }
    }
    walk(envelope, &keys, &strings, "");
}

/// Every name-like string of the `--out` files: paths, texts, names,
/// enclosing functions, schedules, piped targets, registrations, crates,
/// dumped names and their `::` segments, schema field names.
fn names_in_out(files: &BTreeMap<String, Value>) -> BTreeSet<String> {
    const NAME_FIELDS: &[&str] = &[
        "path",
        "text",
        "name",
        "enclosing_fn",
        "schedule",
        "piped",
        "registration",
        "unknown_fields",
        "missing_fields",
    ];
    fn strings(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::String(text) => {
                out.insert(text.clone());
                for segment in text.split("::") {
                    out.insert(segment.to_owned());
                }
            }
            Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
            Value::Object(map) => map.values().for_each(|item| strings(item, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    for (file, value) in files {
        if file == "crates.json" {
            strings(value, &mut out);
            continue;
        }
        let records: Vec<&Value> = match value {
            Value::Array(items) => items.iter().collect(),
            other => vec![other],
        };
        for record in records {
            for field in NAME_FIELDS {
                if let Some(value) = record.get(field) {
                    strings(value, &mut out);
                }
            }
        }
    }
    out.retain(|name| !name.is_empty());
    out
}

// ------------------------------------------------------- AC-10 fixture

#[test]
fn fixture_without_dump_matches_expected_json_and_leaves_the_fixture_untouched() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let status_before = git_status(&repository_root(), Some("fixtures/bevy-mini"));
    let scratch = Scratch::new("fixture");
    let out = scratch.join("out");

    let output = run(&["bevy-detector", "--out", out.to_str().unwrap()]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "fixtures");
    let expected = expected();
    let (result, bytes) = strip_timing(&envelope["result"]);
    assert_eq!(
        result, expected["result"],
        "result differs from expected.json"
    );
    assert_eq!(
        bytes,
        fs::metadata(fixture.join("src/main.rs")).unwrap().len(),
        "detail.bytes is the size of the one source file"
    );

    let detail = out_files(&out.join("bevy").join("fixtures"));
    let names: Vec<&String> = detail.keys().collect();
    assert_eq!(
        names,
        vec![
            "crates.json",
            "plugin_uses.json",
            "plugins.json",
            "registrations.json",
            "uncertain.json"
        ],
        "without --dump there is no dump_match.json / dump_schema.json"
    );
    for (name, value) in &detail {
        assert_eq!(value, &expected["out"][name], "--out {name}");
    }

    assert_eq!(
        snapshot(&fixture),
        before,
        "the fixture must stay untouched"
    );
    assert_eq!(
        git_status(&repository_root(), Some("fixtures/bevy-mini")),
        status_before
    );
}

#[test]
fn fixture_with_dump_matches_expected_json() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let scratch = Scratch::new("fixture-dump");
    let out = scratch.join("out");

    let output = run(&[
        "bevy-detector",
        "--out",
        out.to_str().unwrap(),
        "--dump",
        FIXTURE_DUMP,
    ]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "fixtures");
    let expected = expected();
    let (mut result, _) = strip_timing(&envelope["result"]);
    assert_eq!(
        result["dump"], expected["dump"],
        "result.dump differs from expected.json"
    );
    result["dump"] = Value::Null;
    assert_eq!(
        result, expected["result"],
        "a dump changes nothing outside result.dump"
    );

    let detail = out_files(&out.join("bevy").join("fixtures"));
    assert_eq!(detail.len(), 7, "{:?}", detail.keys().collect::<Vec<_>>());
    for (name, value) in &detail {
        assert_eq!(value, &expected["out"][name], "--out {name}");
    }
    assert_eq!(
        snapshot(&fixture),
        before,
        "the fixture and its dump stay untouched"
    );
    let stderr = stderr(&output);
    assert!(
        stderr.contains("misses: 1 (macro_rules 1)"),
        "stderr summary names the miss categories:\n{stderr}"
    );
}

#[test]
fn the_bevy_alias_runs_the_same_measurement() {
    let scratch = Scratch::new("alias");
    let long = envelope(&run(&[
        "bevy-detector",
        "--out",
        scratch.join("long").to_str().unwrap(),
        "--dump",
        FIXTURE_DUMP,
    ]));
    let short = envelope(&run(&[
        "bevy",
        "--out",
        scratch.join("short").to_str().unwrap(),
        "--dump",
        FIXTURE_DUMP,
    ]));
    assert_eq!(short["measurement"], "bevy-detector");
    assert_eq!(without_timing(&short), without_timing(&long));
    assert!(
        scratch
            .join("short/bevy/fixtures/dump_match.json")
            .is_file()
    );
}

#[test]
fn fixture_run_is_deterministic() {
    let scratch = Scratch::new("determinism");
    let run_once = |name: &str| {
        envelope(&run(&[
            "bevy-detector",
            "--out",
            scratch.join(name).to_str().unwrap(),
            "--dump",
            FIXTURE_DUMP,
        ]))
    };
    let one = run_once("one");
    let two = run_once("two");
    assert_eq!(
        without_timing(&one),
        without_timing(&two),
        "stdout differs beyond wall_ms / detector_ms"
    );
    let one_files = snapshot(&scratch.join("one"));
    let two_files = snapshot(&scratch.join("two"));
    assert_eq!(one_files.len(), 7);
    assert_eq!(one_files, two_files, "--out files differ between two runs");
}

#[test]
fn stdout_carries_counts_only_and_no_name_of_the_out_files() {
    let scratch = Scratch::new("stdout");
    let out = scratch.join("out");
    let output = run(&[
        "bevy-detector",
        "--out",
        out.to_str().unwrap(),
        "--dump",
        FIXTURE_DUMP,
    ]);
    let envelope = envelope(&output);
    assert_stdout_whitelist(&envelope, "fixtures");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let names = names_in_out(&out_files(&out.join("bevy").join("fixtures")));
    for expected in [
        "src/main.rs",
        "tick",
        "on_spawn",
        "setup_plugin",
        "MiniPlugin",
        "DefaultPlugins",
        "bevy_mini",
        "Update",
    ] {
        assert!(
            names.contains(expected),
            "`{expected}` is in the --out files"
        );
    }
    for name in &names {
        assert!(
            !stdout.contains(name.as_str()),
            "`{name}` from --out leaked to stdout: {stdout}"
        );
    }
    assert!(!stdout.contains(fixture_dir().to_str().unwrap()));
}

// ------------------------------------------------------------ AC-02 guard

fn assert_refused(output: &Output, forbidden_out: &Path, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: expected exit 2, stderr:\n{}",
        stderr(output)
    );
    assert!(
        output.stdout.is_empty(),
        "{context}: nothing on stdout when refused, got {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !forbidden_out.exists(),
        "{context}: --out {} was created despite the refusal",
        forbidden_out.display()
    );
}

/// A private copy of the fixture to point `--pilot` at, with its snapshot.
struct GuardCorpus {
    scratch: Scratch,
    corpus: PathBuf,
    before: BTreeMap<PathBuf, Vec<u8>>,
}

impl GuardCorpus {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let corpus = scratch.join("corpus");
        copy_dir(&fixture_dir(), &corpus);
        let before = snapshot(&corpus);
        Self {
            scratch,
            corpus,
            before,
        }
    }

    fn arg(&self) -> &str {
        self.corpus.to_str().unwrap()
    }

    fn dump(&self) -> String {
        self.corpus
            .join("app_data.ron")
            .to_str()
            .unwrap()
            .to_owned()
    }

    fn refuse(&self, out: &Path, context: &str) {
        let out_arg = out.to_str().unwrap();
        let without = run(&["bevy-detector", "--pilot", self.arg(), "--out", out_arg]);
        assert_refused(&without, out, context);
        let dump = self.dump();
        let with = run(&[
            "bevy-detector",
            "--pilot",
            self.arg(),
            "--out",
            out_arg,
            "--dump",
            &dump,
        ]);
        assert_refused(&with, out, &format!("{context}, with --dump"));
        assert_eq!(
            snapshot(&self.corpus),
            self.before,
            "the corpus must stay byte-identical"
        );
    }
}

#[test]
fn guard_refuses_out_under_the_corpus_and_writes_nothing() {
    let corpus = GuardCorpus::new("guard");
    corpus.refuse(
        &corpus.corpus.join("scratch"),
        "--out directly under --pilot",
    );
    corpus.refuse(
        &corpus.corpus.join("deeper").join("out"),
        "--out nested under --pilot",
    );
    corpus.refuse(
        &corpus.corpus.join("missing").join("..").join("out"),
        "--out stepping back into --pilot",
    );
    assert!(
        !corpus.corpus.join("out").exists() && !corpus.corpus.join("missing").exists(),
        "--out stepping back into --pilot: nothing created under the corpus"
    );
    let output = run(&[
        "bevy-detector",
        "--pilot",
        corpus.arg(),
        "--out",
        corpus.arg(),
    ]);
    assert_eq!(output.status.code(), Some(2), "--out equal to --pilot");
    assert!(output.stdout.is_empty());
    assert_eq!(snapshot(&corpus.corpus), corpus.before);
}

#[cfg(unix)]
#[test]
fn guard_refuses_out_reaching_the_corpus_through_a_symlink() {
    let corpus = GuardCorpus::new("guard-symlink");
    let link = corpus.scratch.join("link");
    std::os::unix::fs::symlink(&corpus.corpus, &link).expect("symlink");
    corpus.refuse(&link.join("out"), "--out through a symlink into --pilot");
}

#[test]
fn out_beside_the_corpus_is_accepted_and_labelled_pilot() {
    let corpus = GuardCorpus::new("beside");
    let out = corpus.scratch.join("out");
    let dump = corpus.dump();
    let output = run(&[
        "bevy-detector",
        "--pilot",
        corpus.arg(),
        "--out",
        out.to_str().unwrap(),
        "--dump",
        &dump,
    ]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "pilot");
    assert_eq!(
        envelope["result"]["detected"],
        expected()["result"]["detected"]
    );
    assert_eq!(envelope["result"]["dump"], expected()["dump"]);
    assert!(out.join("bevy/pilot/dump_match.json").is_file());
    assert_eq!(snapshot(&corpus.corpus), corpus.before);
}

// ------------------------------------------------------ invalid dumps

/// Writes `content` as a dump, runs the fixture with it and asserts the
/// refusal: exit 2, nothing on stdout, `--out` not created, stderr naming
/// `<dump>:<line>:`.
fn assert_dump_refused_at(scratch: &Scratch, name: &str, content: &[u8], line: usize) {
    let dump = scratch.join(&format!("{name}.ron"));
    fs::write(&dump, content).expect("dump written");
    let out = scratch.join(&format!("out-{name}"));
    let output = run(&[
        "bevy-detector",
        "--out",
        out.to_str().unwrap(),
        "--dump",
        dump.to_str().unwrap(),
    ]);
    assert_refused(&output, &out, name);
    let stderr = stderr(&output);
    let location = format!("{}:{line}: ", dump.display());
    assert!(
        stderr.contains(&location),
        "{name}: stderr must name `{location}`, got:\n{stderr}"
    );
}

/// A Bevy 0.19 `schedule_data` dump: every schema field, one line each.
fn dump_ron(schedules: &[(&str, &[(&str, bool)])]) -> String {
    let mut out = String::from("(\n    schedules: [\n");
    for (schedule, systems) in schedules {
        out.push_str(&format!(
            "        (\n            name: \"{schedule}\",\n            systems: [\n"
        ));
        for (name, apply_deferred) in *systems {
            out.push_str(&format!(
                "                (\n                    name: \"{name}\",\n                    apply_deferred: {apply_deferred},\n                    exclusive: false,\n                    deferred: false,\n                ),\n"
            ));
        }
        out.push_str(
            "            ],\n            system_sets: [],\n            hierarchy: [],\n            dependency: [],\n            components: [],\n            conflicts: [],\n        ),\n",
        );
    }
    out.push_str("    ],\n)\n");
    out
}

#[test]
fn invalid_dumps_are_refused_naming_file_and_line_and_nothing_is_written() {
    let scratch = Scratch::new("invalid-dump");
    let too_deep = format!("(\n    schedules: [\n{}", "[\n".repeat(80));
    let valid = dump_ron(&[("Update", &[("bevy_mini::tick", false)])]);
    let trailing_line = valid.lines().count() + 1;
    let cases: Vec<(&str, String, usize)> = vec![
        (
            "unterminated_string",
            "(\n    schedules: [\n        (\n            name: \"Update,\n            systems: [],\n        ),\n    ],\n)\n".to_owned(),
            4,
        ),
        ("not_app_data", "[1, 2]\n".to_owned(), 1),
        ("empty", String::new(), 1),
        (
            "schedules_missing",
            "(\n    other: 1,\n)\n".to_owned(),
            1,
        ),
        (
            "schedule_lacks_systems",
            "(\n    schedules: [\n        (\n            name: \"Update\",\n        ),\n    ],\n)\n".to_owned(),
            3,
        ),
        (
            "schedule_name_not_a_string",
            "(\n    schedules: [\n        (\n            name: 3,\n            systems: [],\n        ),\n    ],\n)\n".to_owned(),
            3,
        ),
        (
            "system_without_name",
            "(\n    schedules: [\n        (\n            name: \"Update\",\n            systems: [\n                (\n                    apply_deferred: false,\n                ),\n            ],\n        ),\n    ],\n)\n".to_owned(),
            6,
        ),
        (
            "unbalanced",
            "(\n    schedules: [\n    ),\n)\n".to_owned(),
            3,
        ),
        ("unclosed", "(\n    schedules: [\n".to_owned(), 2),
        (
            "value_without_field_name",
            "(\n    schedules: [],\n    3,\n)\n".to_owned(),
            3,
        ),
        ("trailing_value", format!("{valid}()\n"), trailing_line),
        // The root `(` is line 1, `schedules: [` line 2, and each `[` from
        // line 3 on opens one more level: the 65th container is on line 65.
        ("nesting_past_max_depth", too_deep, 65),
    ];
    for (name, content, line) in &cases {
        assert_dump_refused_at(&scratch, name, content.as_bytes(), *line);
    }
}

// A schedule or system that is not a struct: the error must name the
// element's own line, not the line of the struct around it (a real dump is
// hundreds of kilobytes; line 1 points nowhere).

#[test]
fn a_schedule_that_is_not_a_struct_is_reported_at_its_own_line() {
    let scratch = Scratch::new("schedule-line");
    assert_dump_refused_at(
        &scratch,
        "schedule_not_a_struct",
        b"(\n    schedules: [\n        \"Update\",\n    ],\n)\n",
        3,
    );
}

#[test]
fn a_system_that_is_not_a_struct_is_reported_at_its_own_line() {
    let scratch = Scratch::new("system-line");
    assert_dump_refused_at(
        &scratch,
        "system_not_a_struct",
        b"(\n    schedules: [\n        (\n            name: \"Update\",\n            systems: [\n                \"tick\",\n            ],\n        ),\n    ],\n)\n",
        6,
    );
}

#[test]
fn a_root_or_element_after_leading_comments_is_reported_at_its_own_line() {
    // Comments and blank lines before the value move its line: the error
    // names the line of the offending value, never line 1 of the file.
    let scratch = Scratch::new("comment-lines");
    let cases: Vec<(&str, &str, usize)> = vec![
        (
            "root_list_after_line_comments",
            "// comment\n// comment\n[1, 2]\n",
            3,
        ),
        (
            "root_string_after_block_comment",
            "/* a block\n   comment */\n\n\"text\"\n",
            4,
        ),
        (
            "root_number_after_comment_and_attribute",
            "// comment\n#![enable(implicit_some)]\n\n3\n",
            4,
        ),
        (
            "schedules_missing_after_comments",
            "// comment\n\n(\n    other: 1,\n)\n",
            3,
        ),
        (
            "named_root_after_comments",
            "// comment\n\nAppData(\n    other: 1,\n)\n",
            3,
        ),
        (
            "schedule_after_comments",
            "(\n    schedules: [\n        // comment\n        /* comment */\n        \"Update\",\n    ],\n)\n",
            5,
        ),
        (
            "system_after_comments",
            "(\n    schedules: [\n        (\n            name: \"Update\",\n            systems: [\n                // comment\n\n                7,\n            ],\n        ),\n    ],\n)\n",
            8,
        ),
    ];
    for (name, content, line) in cases {
        assert_dump_refused_at(&scratch, name, content.as_bytes(), line);
    }
}

#[test]
fn a_missing_or_unreadable_dump_is_refused_and_nothing_is_written() {
    let scratch = Scratch::new("missing-dump");
    let non_utf8 = scratch.join("latin1.ron");
    fs::write(&non_utf8, b"(\n    schedules: [\xff],\n)\n").unwrap();
    let directory = scratch.join("a-directory.ron");
    fs::create_dir_all(&directory).unwrap();
    for (context, dump) in [
        ("missing dump", scratch.join("no-such-dump.ron")),
        ("non-UTF-8 dump", non_utf8),
        ("dump is a directory", directory),
    ] {
        let out = scratch.join(&format!("out-{}", context.replace(' ', "-")));
        let output = run(&[
            "bevy-detector",
            "--out",
            out.to_str().unwrap(),
            "--dump",
            dump.to_str().unwrap(),
        ]);
        assert_refused(&output, &out, context);
        let stderr = stderr(&output);
        assert!(
            stderr.contains(dump.to_str().unwrap()),
            "{context}: stderr must name the dump, got:\n{stderr}"
        );
    }
}

#[test]
fn schema_drift_is_counted_not_fatal_and_named_only_under_out() {
    let scratch = Scratch::new("schema-drift");
    let dump = scratch.join("drift.ron");
    // An unknown app field, an unknown system field, a system without
    // `deferred`, a schedule without `conflicts`.
    fs::write(
        &dump,
        "(\n    format_marker: 7,\n    schedules: [\n        (\n            name: \"Update\",\n            systems: [\n                (\n                    name: \"bevy_mini::tick\",\n                    apply_deferred: false,\n                    exclusive: false,\n                    drift_marker: 1,\n                ),\n            ],\n            system_sets: [],\n            hierarchy: [],\n            dependency: [],\n            components: [],\n        ),\n    ],\n)\n",
    )
    .unwrap();
    let out = scratch.join("out");
    let output = run(&[
        "bevy-detector",
        "--out",
        out.to_str().unwrap(),
        "--dump",
        dump.to_str().unwrap(),
    ]);
    let envelope = envelope(&output);
    let summary = &envelope["result"]["dump"];
    assert_eq!(summary["schema"]["unknown_fields"], 2);
    assert_eq!(summary["schema"]["missing_fields"], 2);
    assert_eq!(summary["systems"], 1);
    assert_eq!(summary["miss_categories"]["macro_rules"], 1);
    let schema = read_json(&out.join("bevy/fixtures/dump_schema.json"));
    assert_eq!(
        schema,
        serde_json::json!({
            "unknown_fields": ["app.format_marker", "system.drift_marker"],
            "missing_fields": ["schedule.conflicts", "system.deferred"],
        })
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for name in [
        "format_marker",
        "drift_marker",
        "schedule.conflicts",
        "system.deferred",
    ] {
        assert!(!stdout.contains(name), "`{name}` leaked to stdout");
    }
}

// ------------------------------------------------ comparison corpora

/// A temporary corpus: a crate `zapp` with the given files, and a dump.
struct Corpus {
    scratch: Scratch,
    root: PathBuf,
}

impl Corpus {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.join("corpus");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"zapp\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        for (path, text) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Self { scratch, root }
    }

    /// Runs with `dump` and returns the envelope and the `dump_match.json` rows.
    fn compare(&self, dump: &str) -> (Value, Vec<Value>) {
        let dump_path = self.scratch.join("app_data.ron");
        fs::write(&dump_path, dump).unwrap();
        let out = self.scratch.join("out");
        let _ = fs::remove_dir_all(&out);
        let before = snapshot(&self.root);
        let output = run(&[
            "bevy-detector",
            "--pilot",
            self.root.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--dump",
            dump_path.to_str().unwrap(),
        ]);
        let envelope = envelope(&output);
        assert_stdout_whitelist(&envelope, "pilot");
        let files = out_files(&out.join("bevy/pilot"));
        let stdout = String::from_utf8_lossy(&output.stdout);
        // `closure` (the text of a closure leaf) is also a schema key; the
        // whitelist above already bounds what stdout may carry.
        let keys = allowed_keys();
        for name in names_in_out(&files) {
            if name.len() >= 4 && !keys.contains(&name) {
                assert!(
                    !stdout.contains(&format!("\"{name}\"")),
                    "`{name}` leaked to stdout"
                );
            }
        }
        assert_eq!(snapshot(&self.root), before, "the corpus stays untouched");
        let rows = files["dump_match.json"].as_array().unwrap().clone();
        (envelope, rows)
    }
}

/// `(schedule, name, status, category)` of every `dump_match.json` row.
fn statuses(rows: &[Value]) -> Vec<(String, String, String, Option<String>)> {
    rows.iter()
        .map(|row| {
            (
                row["schedule"].as_str().unwrap().to_owned(),
                row["name"].as_str().unwrap().to_owned(),
                row["status"].as_str().unwrap().to_owned(),
                row["category"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

fn line_of(source: &str, needle: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("`{needle}` in the source"))
        + 1
}

const COMPARISON_LIB: &str = r#"//! Comparison corpus of the bevy-detector tests.
use bevy::prelude::*;

pub struct Foo;
pub trait Tr {
    fn trait_sys();
}
impl Tr for Foo {
    fn trait_sys() {}
}
pub struct A;
pub struct B;

fn pipe_src() -> u32 {
    1
}
fn pipe_dst(In(_value): In<u32>) {}
fn make_counter(step: u32) -> impl FnMut() {
    move || {
        let _ = step;
    }
}
fn generic_sys<T>() {}
fn twice() {}
fn late() {}
fn hidden() {}
fn lonely() {}
fn via_rules() {}
fn via_call() {}

macro_rules! register_rules {
    ($app:expr) => {
        $app.add_systems(Update, via_rules);
    };
}

macro_rules! wrap {
    ($($body:tt)*) => { $($body)* };
}

fn closure_home(app: &mut App) {
    app.add_systems(Startup, || {});
}

pub fn build(app: &mut App) {
    app.add_systems(Update, (pipe_src.pipe(pipe_dst), <Foo as Tr>::trait_sys, make_counter(3)));
    app.add_systems(Update, generic_sys::<A>);
    app.add_systems(Update, twice);
    app.add_systems(Update, late);
    let indirect = hidden;
    app.add_systems(Update, indirect);
    register_rules!(app);
    wrap! { app.add_systems(Update, via_call); }
    closure_home(app);
}
"#;

#[test]
fn comparison_matches_every_form_and_names_every_miss_category() {
    assert!(
        !COMPARISON_LIB.contains("ghost"),
        "the not_in_source name must not occur in the sources"
    );
    let corpus = Corpus::new("compare", &[("src/lib.rs", COMPARISON_LIB)]);
    let dump = dump_ron(&[
        (
            "Update",
            &[
                ("Pipe(zapp::pipe_src, zapp::pipe_dst)", false),
                ("<zapp::Foo as zapp::Tr>::trait_sys", false),
                ("zapp::make_counter::{{closure}}", false),
                ("zapp::generic_sys<zapp::A>", false),
                ("zapp::generic_sys<zapp::B>", false),
                ("zapp::twice", false),
                ("zapp::twice", false),
                ("zapp::via_rules", false),
                ("zapp::via_call", false),
                ("zapp::lonely::{{closure}}", false),
                ("zapp::hidden", false),
                ("zapp::ghost", false),
                ("bevy_ecs::schedule::executor::ApplyDeferred", true),
                ("bevy_time::time_system", false),
            ],
        ),
        ("PostUpdate", &[("zapp::late", false)]),
        ("Startup", &[("zapp::closure_home::{{closure}}", false)]),
    ]);
    let (envelope, rows) = corpus.compare(&dump);
    let exact = |s: &str, n: &str| (s.to_owned(), n.to_owned(), "matched_exact".to_owned(), None);
    let miss = |s: &str, n: &str, c: &str| {
        (
            s.to_owned(),
            n.to_owned(),
            "miss".to_owned(),
            Some(c.to_owned()),
        )
    };
    assert_eq!(
        statuses(&rows),
        vec![
            miss("PostUpdate", "zapp::late", "other_schedule"),
            exact("Startup", "zapp::closure_home::{{closure}}"),
            exact("Update", "<zapp::Foo as zapp::Tr>::trait_sys"),
            exact("Update", "Pipe(zapp::pipe_src, zapp::pipe_dst)"),
            exact("Update", "zapp::generic_sys<zapp::A>"),
            miss("Update", "zapp::generic_sys<zapp::B>", "generic_instance"),
            miss("Update", "zapp::ghost", "not_in_source"),
            miss("Update", "zapp::hidden", "indirect"),
            miss("Update", "zapp::lonely::{{closure}}", "closure"),
            exact("Update", "zapp::make_counter::{{closure}}"),
            exact("Update", "zapp::twice"),
            miss("Update", "zapp::twice", "repeated_site"),
            miss("Update", "zapp::via_call", "macro_call"),
            miss("Update", "zapp::via_rules", "macro_rules"),
        ],
        "the Pipe is keyed by its first system, `<T as Tr>::f` by `f`, closures \
         by their enclosing fn and factory calls by their callee; one miss per category"
    );
    let registration = |name: &str| -> String {
        rows.iter()
            .find(|row| row["name"] == name)
            .and_then(|row| row["registration"].as_str())
            .unwrap_or_else(|| panic!("{name} has a registration"))
            .to_owned()
    };
    let build_line = line_of(COMPARISON_LIB, "pipe_src.pipe(pipe_dst)");
    assert_eq!(
        registration("Pipe(zapp::pipe_src, zapp::pipe_dst)"),
        format!("src/lib.rs:{build_line}")
    );
    assert_eq!(
        registration("zapp::closure_home::{{closure}}"),
        format!(
            "src/lib.rs:{}",
            line_of(COMPARISON_LIB, "add_systems(Startup, || {})")
        )
    );

    let summary = &envelope["result"]["dump"];
    assert_eq!(summary["schedules"], 3);
    assert_eq!(summary["systems_total"], 16);
    assert_eq!(summary["apply_deferred"], 1);
    assert_eq!(summary["systems"], 14);
    assert_eq!(summary["matched"], 6);
    assert_eq!(summary["matched_schedule_exact"], 6);
    assert_eq!(summary["matched_schedule_unverified"], 0);
    assert_eq!(summary["misses"], 8);
    assert_eq!(summary["match_pct"], 42.9);
    assert_eq!(
        summary["name_found_pct"], 64.3,
        "matched + three name-found categories"
    );
    assert_eq!(summary["name_found_incl_macros_pct"], 78.6);
    assert_eq!(
        summary["detected_unmatched"], 2,
        "`late` under another schedule and the variable `indirect`"
    );
    for category in MISS_CATEGORIES {
        assert_eq!(
            summary["miss_categories"][category], 1,
            "one miss in category {category}"
        );
    }
    assert_eq!(
        summary["miss_categories"].as_object().unwrap().len(),
        MISS_CATEGORIES.len()
    );
}

#[test]
fn test_example_and_bench_registrations_are_taken_only_after_app_code() {
    // Every auxiliary file sorts before `src/`, so the order of files alone
    // would hand the dumped system to a test registration.
    let app = "use bevy::prelude::*;\nfn tick() {}\nfn only_in_example() {}\nfn main() {\n    App::new().add_systems(Update, tick).run();\n}\n";
    let corpus = Corpus::new(
        "auxiliary",
        &[
            (
                "a/tests/it.rs",
                "fn it(app: &mut App) {\n    app.add_systems(Update, tick);\n}\n",
            ),
            (
                "benches/bench.rs",
                "fn bench(app: &mut App) {\n    app.add_systems(Update, tick);\n}\n",
            ),
            (
                "examples/demo.rs",
                "fn main() {\n    App::new()\n        .add_systems(Update, tick)\n        .add_systems(Update, only_in_example)\n        .run();\n}\n",
            ),
            ("src/main.rs", app),
        ],
    );
    let dump = dump_ron(&[(
        "Update",
        &[("zapp::tick", false), ("zapp::only_in_example", false)],
    )]);
    let (envelope, rows) = corpus.compare(&dump);
    let taken: BTreeMap<String, String> = rows
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap().to_owned(),
                row["registration"].as_str().unwrap_or("none").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        taken["zapp::tick"],
        format!("src/main.rs:{}", line_of(app, "add_systems(Update, tick)")),
        "the app's own registration is taken before test, example and bench ones"
    );
    assert_eq!(
        taken["zapp::only_in_example"], "examples/demo.rs:4",
        "an auxiliary registration is still taken when the app has none"
    );
    let summary = &envelope["result"]["dump"];
    assert_eq!(summary["matched"], 2);
    assert_eq!(summary["detected_unmatched"], 3);
}

#[test]
fn non_literal_schedules_match_by_name_and_state_labels_are_normalized() {
    let lib = r#"use bevy::prelude::*;
fn var_sys() {}
fn one_arg() {}
fn enter_menu() {}
pub fn add_to(app: &mut App, schedule: impl ScheduleLabel) {
    app.add_systems(schedule, var_sys);
}
pub fn raw(schedule: &mut Schedule) {
    schedule.add_systems(one_arg);
}
pub fn states(app: &mut App) {
    app.add_systems(OnEnter(GameState::Menu), enter_menu);
}
"#;
    let corpus = Corpus::new("non-literal", &[("src/lib.rs", lib)]);
    let dump = dump_ron(&[
        ("FixedUpdate", &[("zapp::var_sys", false)]),
        ("Custom", &[("zapp::one_arg", false)]),
        ("OnEnter(Menu)", &[("zapp::enter_menu", false)]),
    ]);
    let (envelope, rows) = corpus.compare(&dump);
    let got: BTreeMap<String, String> = rows
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap().to_owned(),
                row["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(got["zapp::enter_menu"], "matched_exact");
    assert_eq!(got["zapp::var_sys"], "matched_schedule_unverified");
    assert_eq!(got["zapp::one_arg"], "matched_schedule_unverified");
    let summary = &envelope["result"]["dump"];
    assert_eq!(summary["matched_schedule_exact"], 1);
    assert_eq!(summary["matched_schedule_unverified"], 2);
    assert_eq!(summary["match_pct"], 100.0);
    assert_eq!(envelope["result"]["detail"]["schedule_not_literal"], 2);
}

#[test]
fn a_dump_of_foreign_crates_only_compares_nothing() {
    let corpus = Corpus::new("foreign", &[("src/lib.rs", "fn tick() {}\n")]);
    let dump = dump_ron(&[(
        "Update",
        &[
            ("bevy_time::time_system", false),
            ("bevy_ecs::schedule::executor::ApplyDeferred", true),
        ],
    )]);
    let (envelope, rows) = corpus.compare(&dump);
    assert!(rows.is_empty());
    let summary = &envelope["result"]["dump"];
    assert_eq!(summary["systems_total"], 2);
    assert_eq!(summary["systems"], 0);
    assert_eq!(summary["matched"], 0);
    assert_eq!(summary["misses"], 0);
    assert_eq!(summary["match_pct"], 0.0);
}

// ------------------------------------------------------------- edge data

#[test]
fn empty_corpus_measures_zero_and_broken_inputs_are_counted_not_fatal() {
    let scratch = Scratch::new("edge");
    let empty = scratch.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let output = run(&[
        "bevy-detector",
        "--pilot",
        empty.to_str().unwrap(),
        "--out",
        scratch.join("out-empty").to_str().unwrap(),
    ]);
    let empty_envelope = envelope(&output);
    let result = &empty_envelope["result"];
    assert_eq!(result["files"], 0);
    assert_eq!(
        result["detected"],
        serde_json::json!({"systems": 0, "observers": 0, "plugins": 0})
    );
    assert_eq!(result["detail"]["crates"], 0);

    let broken = scratch.join("broken");
    fs::create_dir_all(broken.join("src")).unwrap();
    fs::write(broken.join("Cargo.toml"), "[package\nname = ").unwrap();
    fs::write(
        broken.join("src/lib.rs"),
        "fn build(app: &mut App) {\n    app.add_systems(Update, (a b));\n    app.add_systems(Update, fine);\n}\n",
    )
    .unwrap();
    fs::write(broken.join("src/latin1.rs"), b"fn caf\xe9() {}\n").unwrap();
    let output = run(&[
        "bevy-detector",
        "--pilot",
        broken.to_str().unwrap(),
        "--out",
        scratch.join("out-broken").to_str().unwrap(),
    ]);
    let broken_envelope = envelope(&output);
    let result = &broken_envelope["result"];
    assert_eq!(result["files"], 1);
    assert_eq!(result["files_with_errors"], 1);
    assert_eq!(result["detail"]["files_skipped"], 1, "the non-UTF-8 file");
    assert_eq!(result["detail"]["manifests_unreadable"], 1);
    assert_eq!(result["detail"]["uncertain"]["parse_error"], 1);
    assert_eq!(result["detected"]["systems"], 1, "the valid call is read");
    let stderr = stderr(&output);
    assert!(
        stderr.contains("Cargo.toml") && stderr.contains("latin1.rs"),
        "stderr names the skipped files:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("latin1") && !stdout.contains("Cargo.toml"));
}

// ------------------------------------------------------------ AC-10 pilots

fn pilot_run(variable: &str, dump_variable: &str, label: &str) {
    let Some(pilot) = std::env::var_os(variable).filter(|v| !v.is_empty()) else {
        panic!("set {variable} to the pilot corpus to run this test");
    };
    let pilot = fs::canonicalize(pilot).expect("pilot path exists");
    let dump = std::env::var_os(dump_variable)
        .filter(|v| !v.is_empty())
        .map(|dump| fs::canonicalize(dump).expect("the dump exists"));
    let dump_before = dump
        .as_ref()
        .map(|dump| fs::read(dump).expect("dump readable"));
    let status_before = git_status(&pilot, None);
    let scratch = Scratch::new(label);
    let out = scratch.join("out");
    assert!(
        !out.starts_with(&pilot),
        "scratch must not lie under the pilot"
    );

    let mut command = eval();
    command.env(variable, &pilot).args([
        "bevy-detector",
        "--label",
        label,
        "--out",
        out.to_str().unwrap(),
        "--timeout",
        "600",
    ]);
    if let Some(dump) = &dump {
        command.arg("--dump").arg(dump);
    }
    let output = command.output().expect("specengine-eval runs");
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, label);
    assert_stdout_whitelist(&envelope, label);
    let result = &envelope["result"];
    assert!(
        result.is_object(),
        "the pilot run must finish under the timeout, got {result}"
    );
    assert!(count(&result["files"], "files") >= 1);
    assert!(count(&result["detected"]["systems"], "systems") >= 1);
    assert!(count(&result["detected"]["plugins"], "plugins") >= 1);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(pilot.to_str().unwrap()),
        "the pilot path leaked to stdout"
    );
    let name = pilot.file_name().unwrap().to_string_lossy();
    assert!(
        !stdout.contains(name.as_ref()),
        "the pilot name leaked to stdout"
    );

    let detail = out.join("bevy").join(label);
    assert!(detail.join("registrations.json").is_file());
    match &dump {
        Some(_) => {
            let summary = &result["dump"];
            let systems = count(&summary["systems"], "dump.systems");
            let matched = count(&summary["matched"], "dump.matched");
            let misses = count(&summary["misses"], "dump.misses");
            assert!(
                systems > 0,
                "AC-10: the dump holds N > 0 systems of the pilot"
            );
            assert_eq!(matched + misses, systems);
            let categorised: u64 = summary["miss_categories"]
                .as_object()
                .unwrap()
                .values()
                .map(|n| count(n, "miss category"))
                .sum();
            assert_eq!(categorised, misses, "every miss has a named category");
            let expected_pct = (matched as f64 * 1000.0 / systems as f64).round() / 10.0;
            assert_eq!(summary["match_pct"].as_f64(), Some(expected_pct));
            assert!(detail.join("dump_match.json").is_file());
        }
        None => {
            assert_eq!(result["dump"], Value::Null);
            eprintln!("{dump_variable} not set: detector counts only, no dump comparison");
        }
    }

    assert_eq!(
        git_status(&pilot, None),
        status_before,
        "the pilot must stay untouched"
    );
    if let (Some(dump), Some(before)) = (&dump, &dump_before) {
        assert_eq!(&fs::read(dump).unwrap(), before, "the dump stays untouched");
    }
    eprintln!("{label} result: {result}");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A (and SPECENGINE_PILOT_A_DUMP for the comparison); read-only"]
fn pilot_a_detector_counts_and_dump_comparison() {
    pilot_run("SPECENGINE_PILOT_A", "SPECENGINE_PILOT_A_DUMP", "pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B (and SPECENGINE_PILOT_B_DUMP for the comparison); read-only"]
fn pilot_b_detector_counts_and_dump_comparison() {
    pilot_run("SPECENGINE_PILOT_B", "SPECENGINE_PILOT_B_DUMP", "pilot-b");
}
