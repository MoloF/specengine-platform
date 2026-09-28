//! `specengine-eval census` end to end (docs/features/phase-0-spikes.md, AC-12
//! counts on `fixtures/corpus-mini`, AC-02 read-only guard and refusals, AC-13
//! anonymous stdout) on the real binary.
//!
//! Pilot tests are `#[ignore]`: the corpus comes from `SPECENGINE_PILOT_A` /
//! `_B` and the census config from `SPECENGINE_CENSUS_CONFIG_A` / `_B`; the
//! configs live outside the repository and no corpus path, name or prefix is
//! written anywhere in this file. Non-Latin characters are Unicode escapes
//! (ADR-0024).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture_dir() -> PathBuf {
    repository_root().join("fixtures").join("corpus-mini")
}

fn expected() -> Value {
    read_json(&fixture_dir().join("expected.json"))
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-census-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
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
    for variable in [
        "SPECENGINE_PILOT_A",
        "SPECENGINE_PILOT_B",
        "SPECENGINE_CENSUS_CONFIG_A",
        "SPECENGINE_CENSUS_CONFIG_B",
    ] {
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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
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

fn git_status(dir: &Path, pathspec: Option<&str>) -> String {
    let mut command = Command::new("git");
    command.current_dir(dir).args(["status", "--porcelain"]);
    if let Some(pathspec) = pathspec {
        command.arg("--").arg(pathspec);
    }
    let output = command.output().expect("git runs");
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// ------------------------------------------------------- stdout anonymity

const RESULT_KEYS: [&str; 10] = [
    "documents",
    "with_front_matter",
    "id_rows",
    "rows_without_id",
    "id_sections",
    "mixed_script_ids",
    "non_latin_ids",
    "broken_links",
    "records_hashed",
    "detail",
];

const DETAIL_KEYS: [&str; 13] = [
    "files_skipped",
    "front_matter_unclosed",
    "roots_missing",
    "bytes",
    "tables",
    "headerless_blocks",
    "headerless_id_rows",
    "record_tables",
    "links_checked",
    "other_anchors",
    "duplicate_ids",
    "diagnostics",
    "census_ms",
];

fn is_anonymous_label(key: &str, stem: &str) -> bool {
    key.strip_prefix(stem)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

/// Every key and every string of the envelope is from the fixed schema, an
/// anonymous label or a version: no corpus string can reach stdout.
fn assert_envelope_is_anonymous(envelope: &Value, label: &str) {
    let envelope_keys = ["measurement", "label", "versions", "wall_ms", "result"];
    let object = envelope.as_object().expect("envelope is an object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        envelope_keys.into_iter().collect()
    );
    assert_eq!(envelope["measurement"], "census");
    assert_eq!(envelope["label"], label);
    assert_eq!(
        envelope["versions"],
        serde_json::json!({"tree-sitter": "0.27.0", "tree-sitter-rust": "0.24.2", "abi": 15})
    );
    assert!(envelope["wall_ms"].is_u64());

    let result = envelope["result"].as_object().expect("result is an object");
    assert_eq!(
        result.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        RESULT_KEYS.into_iter().collect(),
        "result rows"
    );
    for key in RESULT_KEYS
        .iter()
        .filter(|k| !matches!(**k, "detail" | "with_front_matter" | "id_rows"))
    {
        assert!(
            result[*key].is_u64(),
            "result.{key} is a count: {}",
            result[*key]
        );
    }
    let detail = result["detail"].as_object().expect("detail is an object");
    assert_eq!(
        detail.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        DETAIL_KEYS.into_iter().collect(),
        "detail rows"
    );
    assert!(
        detail.values().all(Value::is_u64),
        "detail holds counts only"
    );

    for (bucket, inner, stem) in [
        ("with_front_matter", "per_class", "class"),
        ("id_rows", "per_prefix", "prefix"),
    ] {
        let object = result[bucket].as_object().expect("bucket object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            ["total", inner].into_iter().collect()
        );
        let total = object["total"].as_u64().expect("total is a count");
        let buckets = object[inner].as_object().expect("buckets object");
        for (key, count) in buckets {
            assert!(
                is_anonymous_label(key, stem) || (stem == "class" && key == "unclassified"),
                "{bucket}.{inner} key {key:?} is not an anonymous label"
            );
            assert!(count.as_u64().is_some_and(|c| c > 0), "{key}: {count}");
        }
        let sum: u64 = buckets.values().filter_map(Value::as_u64).sum();
        assert_eq!(sum, total, "{bucket}: buckets sum to the total");
    }
}

/// `labels.json` rows: `(label, value, count)`, labels numbered by descending count.
fn label_rows(labels: &Value, group: &str) -> Vec<(String, String, u64)> {
    labels[group]
        .as_array()
        .unwrap_or_else(|| panic!("labels.json {group} is an array"))
        .iter()
        .map(|row| {
            (
                row["label"].as_str().unwrap().to_owned(),
                row["value"].as_str().unwrap().to_owned(),
                row["count"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn assert_stdout_has_no_label_value(output: &Output, labels: &Value) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    for group in ["classes", "prefixes"] {
        for (_, value, _) in label_rows(labels, group) {
            assert!(
                !stdout.contains(&format!("\"{value}\"")),
                "a {group} value leaked to stdout as a JSON string"
            );
        }
    }
}

// ------------------------------------------------------------ AC-12 fixture

/// Every row of `expected.json` equals the result; all mismatches reported at once.
fn assert_counts_match(result: &Value, expected: &serde_json::Map<String, Value>) {
    let mismatches: Vec<String> = expected
        .iter()
        .filter(|(key, want)| &result[key.as_str()] != *want)
        .map(|(key, want)| {
            format!(
                "result.{key}: got {}, expected {want}",
                result[key.as_str()]
            )
        })
        .collect();
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn fixture_run_matches_expected_json_and_leaves_the_fixture_untouched() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let status_before = git_status(&repository_root(), Some("fixtures/"));
    let scratch = Scratch::new("fixture");
    let out = scratch.join("out");

    let output = run(&["census", "--out", out.to_str().unwrap()]);
    let envelope = envelope(&output);
    assert_envelope_is_anonymous(&envelope, "fixtures");
    let result = &envelope["result"];
    let expected = expected();
    let expected = expected.as_object().expect("expected.json is an object");
    assert_eq!(
        expected.len(),
        RESULT_KEYS.len() - 1,
        "expected.json names every row but `detail`"
    );
    assert_counts_match(result, expected);
    let detail = &result["detail"];
    for (key, want) in [
        ("files_skipped", 0),
        ("front_matter_unclosed", 0),
        ("roots_missing", 0),
        ("tables", 1),
        ("headerless_blocks", 0),
        ("headerless_id_rows", 0),
        ("record_tables", 1),
        ("links_checked", 3),
        ("other_anchors", 0),
        ("duplicate_ids", 0),
        ("diagnostics", 0),
    ] {
        assert_eq!(detail[key], want, "detail.{key}");
    }

    // The label mapping and every per-file list land in --out only.
    let detail_dir = out.join("census").join("fixtures");
    let mut files: Vec<String> = fs::read_dir(&detail_dir)
        .expect("detail directory")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        [
            "broken_links.json",
            "diagnostics.json",
            "documents.json",
            "labels.json",
            "records.json",
            "rows_without_id.json",
        ]
    );
    let labels = read_json(&detail_dir.join("labels.json"));
    assert_eq!(
        label_rows(&labels, "classes"),
        [
            ("class-1".to_owned(), "rule".to_owned(), 2),
            ("class-2".to_owned(), "note".to_owned(), 1),
        ]
    );
    assert_eq!(
        label_rows(&labels, "prefixes"),
        [
            ("prefix-1".to_owned(), "ZR".to_owned(), 3),
            ("prefix-2".to_owned(), "ZN".to_owned(), 1),
        ]
    );

    let records = read_json(&detail_dir.join("records.json"));
    let records = records.as_array().expect("records.json is an array");
    let rows: Vec<(&str, u64, &str, &str, &str, &str, &str)> = records
        .iter()
        .map(|r| {
            (
                r["path"].as_str().unwrap(),
                r["line"].as_u64().unwrap(),
                r["kind"].as_str().unwrap(),
                r["verbatim_id"].as_str().unwrap(),
                r["id"].as_str().unwrap(),
                r["prefix"].as_str().unwrap(),
                r["script"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                "design/rules.md",
                11,
                "row",
                "ZR-001",
                "ZR-001",
                "ZR",
                "latin"
            ),
            (
                "design/rules.md",
                12,
                "row",
                "ZR-002",
                "ZR-002",
                "ZR",
                "latin"
            ),
            (
                "design/rules.md",
                13,
                "row",
                "ZN-001",
                "ZN-001",
                "ZN",
                "latin"
            ),
            (
                "design/rules.md",
                15,
                "row",
                "\u{0396}R-003",
                "ZR-003",
                "ZR",
                "mixed-script"
            ),
            (
                "design/sections.md",
                7,
                "section",
                "ZR-004",
                "ZR-004",
                "ZR",
                "latin"
            ),
        ]
    );
    let hashes: BTreeSet<&str> = records
        .iter()
        .map(|r| r["blake3"].as_str().unwrap())
        .collect();
    assert!(
        hashes
            .iter()
            .all(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())),
        "BLAKE3 hex digests: {hashes:?}"
    );
    assert_eq!(hashes.len(), 5, "five distinct hashes");

    assert_eq!(
        read_json(&detail_dir.join("rows_without_id.json")),
        serde_json::json!([{"path": "design/rules.md", "line": 14}])
    );
    assert_eq!(
        read_json(&detail_dir.join("broken_links.json")),
        serde_json::json!([{"path": "design/sections.md", "line": 10, "target": "missing.md"}])
    );
    assert_eq!(
        read_json(&detail_dir.join("diagnostics.json")),
        serde_json::json!([])
    );
    let documents = read_json(&detail_dir.join("documents.json"));
    let documents: Vec<(&str, &str, Option<&str>)> = documents
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["path"].as_str().unwrap(),
                d["front_matter"].as_str().unwrap(),
                d["class"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        documents,
        [
            ("design/notes.md", "present", Some("note")),
            ("design/rules.md", "present", Some("rule")),
            ("design/sections.md", "present", Some("rule")),
        ]
    );

    // stdout: counts and anonymous labels only.
    let stdout = String::from_utf8_lossy(&output.stdout);
    for word in ["ZR", "ZN", "rule", "note", "design", ".md", "/", "\\"] {
        assert!(
            !stdout.contains(word),
            "{word:?} leaked to stdout: {stdout}"
        );
    }
    assert!(!stdout.contains(fixture.to_str().unwrap()));
    assert_stdout_has_no_label_value(&output, &labels);

    assert_eq!(
        snapshot(&fixture),
        before,
        "the fixture must be byte-identical after a run"
    );
    assert_eq!(
        git_status(&repository_root(), Some("fixtures/")),
        status_before,
        "git status of fixtures/ changed"
    );
}

#[test]
fn fixture_run_is_deterministic() {
    let scratch = Scratch::new("determinism");
    let first = run(&["census", "--out", scratch.join("one").to_str().unwrap()]);
    let config = fixture_dir().join("census.toml");
    let second = run(&[
        "census",
        "--config",
        config.to_str().unwrap(),
        "--out",
        scratch.join("two").to_str().unwrap(),
    ]);
    let mut one = envelope(&first);
    let mut two = envelope(&second);
    for value in [&mut one, &mut two] {
        value
            .as_object_mut()
            .unwrap()
            .remove("wall_ms")
            .expect("wall_ms");
        value["result"]["detail"]
            .as_object_mut()
            .unwrap()
            .remove("census_ms")
            .expect("detail.census_ms");
    }
    assert_eq!(one, two, "envelope differs beyond timings");
    for name in [
        "records.json",
        "documents.json",
        "rows_without_id.json",
        "broken_links.json",
        "diagnostics.json",
        "labels.json",
    ] {
        let a = fs::read(scratch.join("one/census/fixtures").join(name)).unwrap();
        let b = fs::read(scratch.join("two/census/fixtures").join(name)).unwrap();
        assert_eq!(a, b, "{name} differs between two runs");
    }
}

/// The fixture copied elsewhere and measured as a pilot (with its own
/// `census.toml` at the root) takes the same code path and gives the same counts.
#[test]
fn a_copy_measured_as_a_pilot_gives_the_same_counts() {
    let scratch = Scratch::new("copy");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture_dir(), &corpus);
    let before = snapshot(&corpus);
    let out = scratch.join("out");
    let output = run(&[
        "census",
        "--pilot",
        corpus.to_str().unwrap(),
        "--label",
        "pilot-a",
        "--out",
        out.to_str().unwrap(),
    ]);
    let envelope = envelope(&output);
    assert_envelope_is_anonymous(&envelope, "pilot-a");
    assert_counts_match(&envelope["result"], expected().as_object().unwrap());
    assert!(out.join("census/pilot-a/labels.json").is_file());
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains(corpus.to_str().unwrap()),
        "the corpus path leaked to stdout"
    );
    assert_eq!(snapshot(&corpus), before, "the corpus stays byte-identical");
}

// ------------------------------------------------- AC-02 guard and refusals

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

    fn without_census_toml(name: &str) -> Self {
        let mut guard = Self::new(name);
        fs::remove_file(guard.corpus.join("census.toml")).unwrap();
        guard.before = snapshot(&guard.corpus);
        guard
    }

    fn arg(&self) -> &str {
        self.corpus.to_str().unwrap()
    }

    fn refuse(&self, extra: &[&str], out: &Path, context: &str) -> String {
        let mut args = vec![
            "census",
            "--pilot",
            self.arg(),
            "--out",
            out.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        let output = run(&args);
        assert_refused(&output, out, context);
        assert_eq!(
            snapshot(&self.corpus),
            self.before,
            "{context}: the corpus must stay byte-identical"
        );
        stderr(&output)
    }
}

#[test]
fn guard_refuses_out_under_pilot_and_writes_nothing() {
    let guard = GuardCorpus::new("guard");
    guard.refuse(
        &[],
        &guard.corpus.join("scratch"),
        "--out directly under --pilot",
    );
    guard.refuse(
        &[],
        &guard.corpus.join("deeper").join("out"),
        "--out nested under --pilot",
    );
    guard.refuse(
        &[],
        &guard.corpus.join("absent").join("..").join("out"),
        "--out under --pilot through `..`",
    );
    let output = run(&["census", "--pilot", guard.arg(), "--out", guard.arg()]);
    assert_eq!(output.status.code(), Some(2), "--out equal to --pilot");
    assert!(output.stdout.is_empty());
    assert_eq!(snapshot(&guard.corpus), guard.before);
    // The guard runs before the config is read: a broken config changes nothing.
    let bad = guard.scratch.join("bad.toml");
    fs::write(&bad, "[corpus]\nroots = [\"..\"]\n").unwrap();
    guard.refuse(
        &["--config", bad.to_str().unwrap()],
        &guard.corpus.join("out"),
        "--out under --pilot with a broken config",
    );
}

#[test]
fn an_invalid_config_is_refused_with_file_and_line() {
    let guard = GuardCorpus::new("bad-config");
    let cases = [
        (
            "unknown-key.toml",
            "[corpus]\nroots = [\"design\"]\ncolour = \"blue\"\n[ids]\nregex = '^A$'\n",
            3,
        ),
        (
            "escaping-root.toml",
            "[corpus]\nroots = [\"design\", \"../outside\"]\n[ids]\nregex = '^A$'\n",
            2,
        ),
        (
            "bad-regex.toml",
            "[corpus]\nroots = [\"design\"]\n[ids]\nregex = '^[A-Z'\n",
            4,
        ),
    ];
    for (name, text, line) in cases {
        let config = guard.scratch.join(name);
        fs::write(&config, text).unwrap();
        let out = guard.scratch.join(&format!("out-{name}"));
        let stderr = guard.refuse(&["--config", config.to_str().unwrap()], &out, name);
        let location = format!("{}:{line}:", config.display());
        assert!(
            stderr.contains(&location),
            "{name}: stderr must name {location}, got:\n{stderr}"
        );
    }
    // A broken census.toml at the corpus root is refused the same way.
    fs::write(
        guard.corpus.join("census.toml"),
        "[corpus]\nroots = [\"design\"]\n[ids]\nregex = '^[A-Z]*$'\n",
    )
    .unwrap();
    let guard = GuardCorpus {
        before: snapshot(&guard.corpus),
        ..guard
    };
    let out = guard.scratch.join("out-default");
    let stderr = guard.refuse(&[], &out, "broken census.toml");
    assert!(
        stderr.contains("census.toml:4:"),
        "stderr must name census.toml:4, got:\n{stderr}"
    );
}

#[test]
fn no_config_and_no_census_toml_is_refused() {
    let guard = GuardCorpus::without_census_toml("no-config");
    let out = guard.scratch.join("out");
    let stderr = guard.refuse(&[], &out, "no --config, no census.toml");
    assert!(stderr.contains("census.toml"), "{stderr}");
    assert!(stderr.contains("--config"), "{stderr}");

    let missing = guard.scratch.join("absent.toml");
    let stderr = guard.refuse(
        &["--config", missing.to_str().unwrap()],
        &out,
        "--config names a missing file",
    );
    assert!(stderr.contains("absent.toml"), "{stderr}");
}

#[test]
fn an_unreadable_pilot_is_refused() {
    let scratch = Scratch::new("absent-pilot");
    let out = scratch.join("out");
    let output = run(&[
        "census",
        "--pilot",
        scratch.join("absent").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_refused(&output, &out, "missing --pilot");
}

// ---------------------------------------------------------------- pilots

fn pilot_run(pilot_variable: &str, config_variable: &str, label: &str) {
    let Some(pilot) = std::env::var_os(pilot_variable).filter(|v| !v.is_empty()) else {
        panic!("set {pilot_variable} to the pilot corpus to run this test");
    };
    let Some(config) = std::env::var_os(config_variable).filter(|v| !v.is_empty()) else {
        panic!("set {config_variable} to the pilot's census config (outside the repository)");
    };
    let pilot = fs::canonicalize(pilot).expect("pilot path exists");
    let config = fs::canonicalize(config).expect("config path exists");
    let root = repository_root();
    assert!(
        !config.starts_with(&root),
        "a pilot census config must live outside the repository"
    );
    let status_before = git_status(&pilot, None);
    let scratch = Scratch::new(label);
    let out = scratch.join("out");
    assert!(
        !out.starts_with(&pilot),
        "scratch must not lie under the pilot"
    );

    let output = eval()
        .args([
            "census",
            "--pilot",
            pilot.to_str().unwrap(),
            "--label",
            label,
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--timeout",
            "600",
        ])
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output);
    let result = &envelope["result"];
    assert!(
        result.is_object(),
        "the pilot run must finish under the timeout, got {result}"
    );
    assert_envelope_is_anonymous(&envelope, label);
    assert!(
        result["documents"].as_u64().unwrap() >= 1,
        "the configured roots hold documents"
    );
    let detail_dir = out.join("census").join(label);
    let labels = read_json(&detail_dir.join("labels.json"));
    assert_stdout_has_no_label_value(&output, &labels);
    let records = read_json(&detail_dir.join("records.json"));
    assert_eq!(
        records.as_array().unwrap().len() as u64,
        result["records_hashed"].as_u64().unwrap()
    );

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
    for word in [".md", "/", "\\"] {
        assert!(!stdout.contains(word), "{word:?} leaked to stdout");
    }
    assert_eq!(
        git_status(&pilot, None),
        status_before,
        "the pilot must stay untouched"
    );
    eprintln!("{label} result: {result}");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A and SPECENGINE_CENSUS_CONFIG_A; read-only run over a real corpus"]
fn pilot_a_census_prints_anonymous_counts() {
    pilot_run(
        "SPECENGINE_PILOT_A",
        "SPECENGINE_CENSUS_CONFIG_A",
        "pilot-a",
    );
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B and SPECENGINE_CENSUS_CONFIG_B; read-only run over a real corpus"]
fn pilot_b_census_prints_anonymous_counts() {
    pilot_run(
        "SPECENGINE_PILOT_B",
        "SPECENGINE_CENSUS_CONFIG_B",
        "pilot-b",
    );
}
