//! `specengine-eval ron` end to end (docs/features/phase-0-spikes.md, AC-07
//! marker resolution and parse-clean share, AC-02 read-only guard, AC-13
//! aggregates only on stdout) on the real binary over `fixtures/ron` and over
//! generated edge corpora.
//!
//! Pilot tests are `#[ignore]` and read their corpus from `SPECENGINE_PILOT_A`
//! / `_B`; no corpus path or name is written anywhere in this file. The
//! mixed-script ID is assembled from a Unicode escape (ADR-0024).

use std::collections::BTreeMap;
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
    repository_root().join("fixtures").join("ron")
}

fn expected() -> Value {
    let path = fixture_dir().join("expected.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("expected.json readable"))
        .expect("expected.json is JSON")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("specengine-eval-ron-{name}-{}", std::process::id()));
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
    command.env_remove("SPECENGINE_PILOT_A");
    command.env_remove("SPECENGINE_PILOT_B");
    command
}

fn run(args: &[&str]) -> Output {
    eval().args(args).output().expect("specengine-eval runs")
}

fn envelope(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "exit {:?}, stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
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
    command
        .current_dir(dir)
        .args(["--no-optional-locks", "status", "--porcelain"]);
    if let Some(pathspec) = pathspec {
        command.arg("--").arg(pathspec);
    }
    let output = command.output().expect("git runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
}

fn number(value: &Value, what: &str) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{what} must be a number, got {value}"))
}

fn count(value: &Value, what: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{what} must be a count, got {value}"))
}

fn assert_envelope_shape(envelope: &Value, label: &str) {
    assert_eq!(envelope["measurement"], "ron");
    assert_eq!(envelope["label"], label);
    assert_eq!(
        envelope["versions"]["tree-sitter"],
        specengine_code::grammar::TREE_SITTER_VERSION
    );
    count(&envelope["versions"]["abi"], "abi");
    count(&envelope["wall_ms"], "wall_ms");
}

/// Every row of the `ron` result is present (shape: `crates/specengine-eval/
/// README.md`, "CLI contract"): the lexer side always as a value; the removed
/// grammar side (spike verdict `lexer`) keeps its keys, every one `null`, and
/// `grammar.built/loads` are `false`.
fn assert_all_rows_present(result: &Value) {
    let grammar = &result["grammar"];
    assert_eq!(grammar["built"], Value::Bool(false), "grammar.built");
    assert_eq!(grammar["loads"], Value::Bool(false), "grammar.loads");
    assert!(grammar["note"].is_string());
    count(&result["files"], "files");
    count(&result["files_with_comments"], "files_with_comments");
    count(&result["parse_clean"]["lexer"], "parse_clean.lexer");
    number(&result["parse_clean_pct"]["lexer"], "parse_clean_pct.lexer");
    assert!(result["rejected_categories"]["lexer"].is_array());
    assert_eq!(
        result["comment_byte_ranges"]["lexer"],
        Value::Bool(true),
        "comment byte ranges are what the lexer is for"
    );
    let markers = &result["markers"];
    for key in ["total", "nested", "id_not_latin"] {
        count(&markers[key], &format!("markers.{key}"));
    }
    assert!(markers["by_relation"].is_object());
    for key in ["anchored", "unanchored", "cannot_verify"] {
        count(&markers[key]["lexer"], &format!("markers.{key}.lexer"));
    }
    assert!(
        result["nested_marker_resolves"]["lexer"].is_boolean()
            || result["nested_marker_resolves"]["lexer"].is_null()
    );
    assert_eq!(result["recommendation"], "lexer", "the spike verdict");
    let detail = &result["detail"];
    count(&detail["files_skipped"], "detail.files_skipped");
    count(&detail["bytes"], "detail.bytes");
    count(&detail["lexer_ms"], "detail.lexer_ms");
    count(&detail["comments"]["lexer"], "detail.comments.lexer");
    count(&detail["files_with_markers"], "detail.files_with_markers");
    assert!(detail["rejected_files"]["lexer"].is_object());

    let grammar_side = [
        &result["parse_clean"]["grammar"],
        &result["parse_clean_pct"]["grammar"],
        &result["rejected_categories"]["grammar"],
        &result["comment_byte_ranges"]["grammar"],
        &result["comment_byte_ranges"]["files_agreeing_pct"],
        &markers["anchored"]["grammar"],
        &markers["unanchored"]["grammar"],
        &markers["cannot_verify"]["grammar"],
        &grammar["version"],
        &grammar["abi"],
        &grammar["second_runtime"],
        &detail["grammar_ms"],
        &detail["comments"]["grammar"],
        &detail["rejected_files"]["grammar"],
        &markers["agree_pct"],
        &result["nested_marker_resolves"]["grammar"],
    ];
    for value in grammar_side {
        assert!(
            value.is_null(),
            "grammar-side field {value} after the path was removed"
        );
    }
    for (object, name) in [
        (&result["parse_clean"], "parse_clean"),
        (&result["parse_clean_pct"], "parse_clean_pct"),
        (&result["rejected_categories"], "rejected_categories"),
        (&markers["anchored"], "markers.anchored"),
        (&markers["unanchored"], "markers.unanchored"),
        (&markers["cannot_verify"], "markers.cannot_verify"),
        (&result["nested_marker_resolves"], "nested_marker_resolves"),
        (&detail["comments"], "detail.comments"),
        (&detail["rejected_files"], "detail.rejected_files"),
    ] {
        assert!(
            object
                .as_object()
                .is_some_and(|o| o.contains_key("grammar")),
            "{name}.grammar key must stay in the envelope shape"
        );
    }
}

fn assert_stdout_carries_aggregates_only(output: &Output, corpus: &Path) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(".ron"),
        "a file name leaked to stdout: {stdout}"
    );
    assert!(
        !stdout.contains(corpus.to_str().unwrap()),
        "the corpus path leaked to stdout"
    );
}

// ------------------------------------------------------------ AC-07 fixture

#[test]
fn fixture_run_matches_expected_json_and_leaves_the_fixture_untouched() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let status_before = git_status(&repository_root(), Some("fixtures/ron"));
    let scratch = Scratch::new("fixture");
    let out = scratch.join("out");

    let output = run(&["ron", "--out", out.to_str().unwrap()]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "fixtures");
    let result = &envelope["result"];
    assert!(result.is_object(), "result must be an object, got {result}");
    assert_all_rows_present(result);

    let expected = expected();
    for key in ["files", "files_with_comments", "recommendation"] {
        assert_eq!(result[key], expected[key], "result.{key}");
    }
    for approach in ["lexer", "grammar"] {
        for key in ["parse_clean", "parse_clean_pct", "rejected_categories"] {
            assert_eq!(
                result[key][approach], expected[key][approach],
                "{key}.{approach}"
            );
        }
        assert_eq!(
            result["comment_byte_ranges"][approach], expected["comment_byte_ranges"][approach],
            "comment_byte_ranges.{approach}"
        );
        for key in ["anchored", "unanchored", "cannot_verify"] {
            assert_eq!(
                result["markers"][key][approach], expected["markers"][key][approach],
                "markers.{key}.{approach}"
            );
        }
        assert_eq!(
            result["nested_marker_resolves"][approach],
            expected["nested_marker_resolves"][approach],
            "nested_marker_resolves.{approach}"
        );
    }
    for key in ["total", "by_relation", "nested", "id_not_latin"] {
        assert_eq!(
            result["markers"][key], expected["markers"][key],
            "markers.{key}"
        );
    }
    assert_eq!(
        result["comment_byte_ranges"]["files_agreeing_pct"],
        expected["comment_byte_ranges"]["files_agreeing_pct"]
    );
    assert_eq!(
        result["markers"]["agree_pct"], expected["markers"]["agree_pct"],
        "markers.agree_pct"
    );

    // Per-file detail lands in --out, never in the corpus.
    let detail = out.join("ron").join("fixtures");
    let markers = read_json(&detail.join("markers.json"));
    let rows = markers.as_array().expect("markers.json is an array");
    let anchors = expected["anchors"].as_array().unwrap();
    assert_eq!(rows.len(), anchors.len(), "one row per marker");
    for (row, want) in rows.iter().zip(anchors) {
        for key in ["path", "line", "relation", "id", "rev"] {
            assert_eq!(row[key], want[key], "markers.json {key}: {row}");
        }
        assert_eq!(row["id_latin"], Value::Bool(true));
        assert_eq!(row["note"], Value::Null);
        assert_eq!(row["lexer"], want["anchor"], "lexer anchor: {row}");
        assert_eq!(row["grammar"], Value::Null, "grammar anchor: {row}");
    }
    let files = read_json(&detail.join("files.json"));
    let files = files.as_array().expect("files.json is an array");
    let comments = expected["comments"].as_object().unwrap();
    assert_eq!(files.len(), comments.len());
    for row in files {
        let path = row["path"].as_str().unwrap();
        assert_eq!(
            row["comments"]["lexer"], comments[path],
            "comments of {path}"
        );
        assert_eq!(row["clean"]["lexer"], Value::Bool(true), "{path}");
        assert_eq!(row["categories"]["lexer"], Value::Array(Vec::new()));
        let markers_in_file = anchors.iter().filter(|a| a["path"] == path).count();
        assert_eq!(count(&row["markers"], "markers") as usize, markers_in_file);
        for key in ["comments", "clean", "categories"] {
            assert_eq!(row[key]["grammar"], Value::Null, "{key}.grammar of {path}");
        }
    }
    let rejected = read_json(&detail.join("rejected.json"));
    assert_eq!(rejected["lexer"], Value::Object(Default::default()));
    assert_eq!(
        rejected["grammar"],
        Value::Object(Default::default()),
        "rejected.json keeps an empty grammar side"
    );

    assert_stdout_carries_aggregates_only(&output, &fixture);
    assert_eq!(
        snapshot(&fixture),
        before,
        "the fixture must be byte-identical after a run"
    );
    assert_eq!(
        git_status(&repository_root(), Some("fixtures/ron")),
        status_before,
        "git status of the fixture changed"
    );
}

#[test]
fn fixture_run_is_deterministic() {
    let scratch = Scratch::new("determinism");
    let first = run(&["ron", "--out", scratch.join("one").to_str().unwrap()]);
    let second = run(&["ron", "--out", scratch.join("two").to_str().unwrap()]);
    let one = envelope(&first);
    let two = envelope(&second);
    let mut a = one["result"].clone();
    let mut b = two["result"].clone();
    // The only timing in `result`: `detail.lexer_ms` (`detail.grammar_ms`
    // stays in the shape and is always `null`, so it is compared).
    for value in [&mut a, &mut b] {
        let detail = value["detail"].as_object_mut().unwrap();
        detail.remove("lexer_ms").expect("detail.lexer_ms present");
        assert_eq!(detail["grammar_ms"], Value::Null, "detail.grammar_ms");
    }
    assert_eq!(a, b, "result differs between two runs");
    let mut one_rest = one.clone();
    let mut two_rest = two.clone();
    for value in [&mut one_rest, &mut two_rest] {
        let envelope = value.as_object_mut().unwrap();
        envelope.remove("wall_ms").expect("wall_ms present");
        envelope.remove("result");
    }
    assert_eq!(one_rest, two_rest, "envelope differs beyond wall_ms");
    for name in ["files.json", "markers.json", "rejected.json"] {
        let one = fs::read(scratch.join("one").join("ron/fixtures").join(name)).unwrap();
        let two = fs::read(scratch.join("two").join("ron/fixtures").join(name)).unwrap();
        assert_eq!(one, two, "{name} differs between two runs");
    }
}

// ------------------------------------------------------------- AC-02 guard

fn assert_refused(output: &Output, forbidden_out: &Path, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: expected exit 2, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
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

    fn refuse(&self, out: &Path, context: &str) {
        let output = run(&["ron", "--pilot", self.arg(), "--out", out.to_str().unwrap()]);
        assert_refused(&output, out, context);
        assert_eq!(
            snapshot(&self.corpus),
            self.before,
            "the corpus must stay byte-identical"
        );
    }
}

#[test]
fn guard_refuses_out_under_pilot_and_writes_nothing() {
    let corpus = GuardCorpus::new("guard");
    corpus.refuse(
        &corpus.corpus.join("scratch"),
        "--out directly under --pilot",
    );
    corpus.refuse(
        &corpus.corpus.join("deeper").join("out"),
        "--out nested under --pilot",
    );
    let output = run(&["ron", "--pilot", corpus.arg(), "--out", corpus.arg()]);
    assert_eq!(output.status.code(), Some(2), "--out equal to --pilot");
    assert!(output.stdout.is_empty());
    assert_eq!(snapshot(&corpus.corpus), corpus.before);
}

#[test]
fn out_beside_the_corpus_is_accepted_and_labelled_pilot() {
    let corpus = GuardCorpus::new("beside");
    let out = corpus.scratch.join("out");
    let output = run(&[
        "ron",
        "--pilot",
        corpus.arg(),
        "--out",
        out.to_str().unwrap(),
    ]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "pilot");
    assert_eq!(envelope["result"]["markers"]["total"], 3);
    assert!(out.join("ron").join("pilot").join("markers.json").is_file());
    assert_eq!(snapshot(&corpus.corpus), corpus.before);
}

// ---------------------------------------------------------------- edge data

/// `X` followed by a Cyrillic look-alike of `x`, then `-1`: a mixed-script ID.
fn mixed_script_id() -> String {
    "X\u{0445}-1".to_owned()
}

#[test]
fn edge_corpus_reports_categories_and_mixed_script_without_blocking() {
    let scratch = Scratch::new("edge");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(corpus.join("nested")).unwrap();
    fs::write(
        corpus.join("broken.ron"),
        "Config(\n    name: \"hero,\n    speed: 4.5,\n)\n",
    )
    .unwrap();
    fs::write(corpus.join("lonely.ron"), "// @implements LONELY@1\n").unwrap();
    fs::write(corpus.join("empty.ron"), "").unwrap();
    fs::write(
        corpus.join("nested").join("mixed.ron"),
        format!(
            "Config(\n    // @implements {}@1\n    speed: 4.5,\n)\n",
            mixed_script_id()
        ),
    )
    .unwrap();
    fs::write(corpus.join("not-ron.txt"), "// @implements IGNORED@1\n").unwrap();
    let before = snapshot(&corpus);
    let out = scratch.join("out");

    let output = run(&[
        "ron",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "pilot");
    let result = &envelope["result"];
    assert_all_rows_present(result);
    assert_eq!(result["files"], 4, "only .ron files count");
    assert_eq!(result["files_with_comments"], 2);
    assert_eq!(result["parse_clean"]["lexer"], 1);
    assert_eq!(result["parse_clean_pct"]["lexer"], 25.0);
    assert_eq!(
        result["rejected_categories"]["lexer"],
        serde_json::json!(["empty_file", "unbalanced_delimiter", "unterminated_string"])
    );
    assert_eq!(
        result["detail"]["rejected_files"]["lexer"],
        serde_json::json!({"empty_file": 2, "unbalanced_delimiter": 1, "unterminated_string": 1})
    );
    assert_eq!(result["detail"]["files_skipped"], 0);
    assert_eq!(result["markers"]["total"], 2);
    assert_eq!(
        result["markers"]["by_relation"],
        serde_json::json!({"implements": 2})
    );
    assert_eq!(result["markers"]["anchored"]["lexer"], 1);
    assert_eq!(result["markers"]["unanchored"]["lexer"], 1);
    assert_eq!(result["markers"]["cannot_verify"]["lexer"], 0);
    assert_eq!(
        result["markers"]["id_not_latin"], 1,
        "ADR-0009: reported, never fatal"
    );
    assert_eq!(result["markers"]["nested"], 0);
    assert_eq!(result["nested_marker_resolves"]["lexer"], Value::Null);
    assert_eq!(result["detail"]["files_with_markers"], 2);

    let detail = out.join("ron").join("pilot");
    let rejected = read_json(&detail.join("rejected.json"));
    let lexer = rejected["lexer"].as_object().unwrap();
    let unterminated = lexer["unterminated_string"].as_array().unwrap();
    assert_eq!(unterminated.len(), 1);
    assert_eq!(unterminated[0]["path"], "broken.ron");
    assert_eq!(
        unterminated[0]["line"], 2,
        "the unterminated string starts on line 2"
    );
    assert!(
        unterminated[0]["snippet"]
            .as_str()
            .unwrap()
            .starts_with("\"hero,"),
        "{}",
        unterminated[0]
    );
    let empty: Vec<&str> = lexer["empty_file"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(empty, ["empty.ron", "lonely.ron"]);
    let markers = read_json(&detail.join("markers.json"));
    let rows = markers.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["path"], "lonely.ron");
    assert_eq!(rows[0]["lexer"], "unanchored");
    assert_eq!(rows[0]["id_latin"], Value::Bool(true));
    assert_eq!(rows[1]["path"], "nested/mixed.ron");
    assert_eq!(rows[1]["id"], mixed_script_id());
    assert_eq!(rows[1]["id_latin"], Value::Bool(false));
    assert_eq!(rows[1]["lexer"], "root.speed");
    let files = read_json(&detail.join("files.json"));
    let paths: Vec<&str> = files
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        ["broken.ron", "empty.ron", "lonely.ron", "nested/mixed.ron"],
        "files.json is sorted by path"
    );

    // Aggregates only: no ID, no file name, no path.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&mixed_script_id()),
        "an ID leaked to stdout"
    );
    assert!(!stdout.contains("LONELY"), "an ID leaked to stdout");
    assert_stdout_carries_aggregates_only(&output, &corpus);
    assert_eq!(snapshot(&corpus), before, "the corpus must stay untouched");
}

/// Generated, never committed: 100 000 nested lists with a marker at the
/// bottom. The lexer path caps the walk (`ron::MAX_DEPTH`), so the run exits 0
/// with the file rejected as `nesting_too_deep` and the marker `cannot_verify`.
#[test]
fn deeply_nested_file_is_rejected_not_crashed() {
    let depth = 100_000;
    let scratch = Scratch::new("deep");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    fs::write(
        corpus.join("deep.ron"),
        format!(
            "{}\n// @implements DEEP@1\n1\n{}\n",
            "[".repeat(depth),
            "]".repeat(depth)
        ),
    )
    .unwrap();
    let before = snapshot(&corpus);
    let out = scratch.join("out");

    let output = run(&[
        "ron",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "pilot");
    let result = &envelope["result"];
    assert_all_rows_present(result);
    assert_eq!(result["files"], 1);
    assert_eq!(result["parse_clean"]["lexer"], 0);
    assert_eq!(
        result["rejected_categories"]["lexer"],
        serde_json::json!(["nesting_too_deep"])
    );
    assert_eq!(
        result["detail"]["rejected_files"]["lexer"],
        serde_json::json!({"nesting_too_deep": 1})
    );
    assert_eq!(result["markers"]["total"], 1);
    assert_eq!(result["markers"]["cannot_verify"]["lexer"], 1);
    assert_eq!(result["markers"]["anchored"]["lexer"], 0);
    assert_eq!(result["markers"]["unanchored"]["lexer"], 0);

    let detail = out.join("ron").join("pilot");
    let rejected = read_json(&detail.join("rejected.json"));
    let rows = rejected["lexer"]["nesting_too_deep"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["path"], "deep.ron");
    assert_eq!(rows[0]["line"], 1);
    assert!(
        rows[0]["snippet"].as_str().unwrap().chars().count() <= 80,
        "the snippet stays bounded on a huge line"
    );
    let markers = read_json(&detail.join("markers.json"));
    assert_eq!(markers[0]["lexer"], "cannot_verify");
    assert_stdout_carries_aggregates_only(&output, &corpus);
    assert_eq!(snapshot(&corpus), before, "the corpus must stay untouched");
}

#[test]
fn empty_corpus_measures_zero_files() {
    let scratch = Scratch::new("empty-corpus");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    let out = scratch.join("out");
    let output = run(&[
        "ron",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    let envelope = envelope(&output);
    let result = &envelope["result"];
    assert_all_rows_present(result);
    assert_eq!(result["files"], 0);
    assert_eq!(result["parse_clean_pct"]["lexer"], 0.0);
    assert_eq!(result["markers"]["total"], 0);
    assert_eq!(result["nested_marker_resolves"]["lexer"], Value::Null);
    assert_eq!(
        read_json(&out.join("ron/pilot/markers.json")),
        Value::Array(Vec::new())
    );
}

// ---------------------------------------------------------------- AC-07 pilots

fn pilot_run(variable: &str, label: &str) {
    let Some(pilot) = std::env::var_os(variable).filter(|v| !v.is_empty()) else {
        panic!("set {variable} to the pilot corpus to run this test");
    };
    let pilot = fs::canonicalize(pilot).expect("pilot path exists");
    let status_before = git_status(&pilot, None);
    let scratch = Scratch::new(label);
    let out = scratch.join("out");
    assert!(
        !out.starts_with(&pilot),
        "scratch must not lie under the pilot"
    );

    let output = eval()
        .env(variable, &pilot)
        .args([
            "ron",
            "--label",
            label,
            "--out",
            out.to_str().unwrap(),
            "--timeout",
            "600",
        ])
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, label);
    let result = &envelope["result"];
    assert!(
        result.is_object(),
        "the pilot run must finish under the timeout, got {result}"
    );
    assert_all_rows_present(result);
    assert!(
        count(&result["files"], "files") >= 1,
        "the pilot holds .ron files"
    );
    let lexer_pct = number(&result["parse_clean_pct"]["lexer"], "parse_clean_pct.lexer");
    assert!((0.0..=100.0).contains(&lexer_pct));
    assert!(out.join("ron").join(label).join("files.json").is_file());

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
    assert!(!stdout.contains(".ron"), "a file name leaked to stdout");

    assert_eq!(
        git_status(&pilot, None),
        status_before,
        "the pilot must stay untouched"
    );
    eprintln!("{label} result: {result}");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A; read-only run over a real corpus"]
fn pilot_a_prints_parse_clean_share_and_categories() {
    pilot_run("SPECENGINE_PILOT_A", "pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B; read-only run over a real corpus"]
fn pilot_b_prints_parse_clean_share_and_categories() {
    pilot_run("SPECENGINE_PILOT_B", "pilot-b");
}
