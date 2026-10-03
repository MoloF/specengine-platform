//! `specengine-eval ast-hash` end to end (docs/features/phase-0-spikes.md,
//! AC-02 read-only guard, AC-03 stability, AC-04 `cannot_verify`, AC-05 `syn`,
//! AC-06 pilot runs) on the real binary over `fixtures/ast-hash`.
//!
//! Pilot tests are `#[ignore]` and read their corpus from `SPECENGINE_PILOT_A`
//! / `_B`; no corpus path or name is written anywhere in this file.

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
    repository_root().join("fixtures").join("ast-hash")
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
            std::env::temp_dir().join(format!("specengine-eval-{name}-{}", std::process::id()));
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

/// Every measured row of the `ast-hash` result must be a number
/// (rustfmt rows are `null` only when no rustfmt is installed, which fails here).
fn assert_all_rows_numeric(result: &Value) {
    for key in [
        "files",
        "items",
        "items_error",
        "items_error_pct",
        "files_with_errors",
        "cannot_verify",
        "qpath_ambiguous_pct",
        "path_attrs",
    ] {
        number(&result[key], key);
    }
    for key in ["fmt_default", "fmt_contrast", "comments"] {
        number(&result["stable_pct"][key], &format!("stable_pct.{key}"));
    }
    for key in ["fmt_default", "fmt_contrast"] {
        count(
            &result["files_changed"][key],
            &format!("files_changed.{key}"),
        );
    }
    assert!(result["cannot_verify_distinct"].is_boolean());
    assert!(result["error_categories"].is_array());
    assert!(
        result["detail"]["rustfmt"]["available"] == Value::Bool(true),
        "rustfmt must be installed for the fmt rows"
    );
    count(
        &result["detail"]["orphan_error_regions"],
        "detail.orphan_error_regions",
    );
    assert!(result["detail"]["stability"].is_object());
    // `docs/canon/code-identity.md` "Eval outputs": items per unit kind
    // (`primary`, `shared`, `unrooted` always there) and package dirs per
    // target source.
    let units = result["detail"]["qpath_units"]
        .as_object()
        .expect("detail.qpath_units is an object");
    for key in ["primary", "shared", "unrooted"] {
        assert!(units.contains_key(key), "detail.qpath_units.{key} missing");
    }
    for (kind, value) in units {
        count(value, &format!("detail.qpath_units.{kind}"));
    }
    for key in ["metadata", "layout"] {
        count(
            &result["detail"]["targets_from"][key],
            &format!("detail.targets_from.{key}"),
        );
    }
}

fn assert_envelope_shape(envelope: &Value, label: &str) {
    assert_eq!(envelope["measurement"], "ast-hash");
    assert_eq!(envelope["label"], label);
    assert_eq!(
        envelope["versions"]["tree-sitter"],
        specengine_code::grammar::TREE_SITTER_VERSION
    );
    assert_eq!(
        envelope["versions"]["tree-sitter-rust"],
        specengine_code::grammar::TREE_SITTER_RUST_VERSION
    );
    assert_eq!(
        count(&envelope["versions"]["abi"], "abi") as usize,
        specengine_code::grammar_info().abi
    );
    count(&envelope["wall_ms"], "wall_ms");
}

// ------------------------------------------------------------ AC-03 / AC-04

#[test]
fn fixture_run_matches_expected_json_and_leaves_the_fixture_untouched() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let status_before = git_status(&repository_root(), Some("fixtures/ast-hash"));
    let scratch = Scratch::new("fixture");
    let out = scratch.join("out");

    let output = run(&["ast-hash", "--out", out.to_str().unwrap()]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "fixtures");
    let result = &envelope["result"];
    assert!(result.is_object(), "result must be an object, got {result}");
    assert_all_rows_numeric(result);

    let expected = expected();
    for key in [
        "files",
        "items",
        "items_error",
        "files_with_errors",
        "cannot_verify",
        "path_attrs",
    ] {
        assert_eq!(result[key], expected[key], "result.{key}");
    }
    for key in ["fmt_default", "fmt_contrast", "comments"] {
        assert_eq!(
            result["stable_pct"][key], expected["stable_pct"][key],
            "stable_pct.{key} (detail: {})",
            result["detail"]["stability"][key]
        );
    }
    for key in ["fmt_default", "fmt_contrast"] {
        let changed = count(&result["files_changed"][key], key);
        let min = count(&expected["files_changed_min"][key], key);
        assert!(
            changed >= min,
            "files_changed.{key} = {changed}, expected >= {min}"
        );
    }
    assert_eq!(
        result["cannot_verify_distinct"],
        expected["cannot_verify_distinct"]
    );
    assert_eq!(result["error_categories"], expected["error_categories"]);
    assert_eq!(
        result["detail"]["orphan_error_regions"],
        expected["orphan_error_regions"]
    );
    assert_eq!(
        result["detail"]["qpath_ambiguity"],
        expected["qpath_ambiguity"]
    );
    #[cfg(not(feature = "syn"))]
    assert_eq!(
        result["syn"],
        Value::Null,
        "syn is null without the feature"
    );

    // Per-file detail lands in --out, never in the corpus: broken items have
    // no hash at all, and nothing was unstable.
    let detail = out.join("ast-hash").join("fixtures");
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(detail.join("manifest.json")).unwrap()).unwrap();
    let mut errored = 0;
    for file in manifest["files"].as_array().unwrap() {
        for item in file["items"].as_array().unwrap() {
            if item["has_error"] == Value::Bool(true) {
                errored += 1;
                assert_eq!(
                    item["hash"],
                    Value::Null,
                    "{item}: cannot_verify is never hashed"
                );
                assert!(
                    !item["error_categories"].as_array().unwrap().is_empty(),
                    "{item}"
                );
            } else {
                assert_eq!(item["hash"].as_str().map(str::len), Some(64), "{item}");
            }
        }
    }
    assert_eq!(errored, 3, "three items with parse errors in the fixture");
    let unstable: Value =
        serde_json::from_str(&fs::read_to_string(detail.join("unstable.json")).unwrap()).unwrap();
    assert_eq!(
        unstable,
        Value::Array(Vec::new()),
        "no unstable items on the fixture"
    );

    // stdout carries aggregates only: no file name and no corpus path.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(".rs"),
        "a file name leaked to stdout: {stdout}"
    );
    assert!(
        !stdout.contains(fixture.to_str().unwrap()),
        "the corpus path leaked to stdout"
    );

    assert_eq!(
        snapshot(&fixture),
        before,
        "the fixture must be byte-identical after a run"
    );
    assert_eq!(
        git_status(&repository_root(), Some("fixtures/ast-hash")),
        status_before,
        "git status of the fixture changed"
    );
}

#[test]
fn fixture_run_is_deterministic() {
    let scratch = Scratch::new("determinism");
    let first = run(&["ast-hash", "--out", scratch.join("one").to_str().unwrap()]);
    let second = run(&["ast-hash", "--out", scratch.join("two").to_str().unwrap()]);
    let mut a = envelope(&first)["result"].clone();
    let mut b = envelope(&second)["result"].clone();
    // The only timings in `result`: `detail.hash_ms` and, with the feature, `syn.time_ratio`.
    for value in [&mut a, &mut b] {
        value["detail"]
            .as_object_mut()
            .unwrap()
            .remove("hash_ms")
            .expect("detail.hash_ms present");
        if let Some(syn) = value["syn"].as_object_mut() {
            syn.remove("time_ratio").expect("syn.time_ratio present");
        }
    }
    assert_eq!(a, b, "result differs between two runs");
    for name in [
        "manifest.json",
        "unstable.json",
        "rustfmt-default.toml",
        "rustfmt-contrast.toml",
    ] {
        let one = fs::read(scratch.join("one").join("ast-hash/fixtures").join(name)).unwrap();
        let two = fs::read(scratch.join("two").join("ast-hash/fixtures").join(name)).unwrap();
        assert_eq!(one, two, "{name} differs between two runs");
    }
}

/// Lines that open a match-arm block or a closure-body block.
fn block_openers(text: &str) -> (usize, usize) {
    let arms = text
        .lines()
        .filter(|l| l.trim_end().ends_with("=> {"))
        .count();
    let closures = text
        .lines()
        .filter(|l| l.trim_end().ends_with("| {"))
        .count();
    (arms, closures)
}

/// Recipe v2 on the fixture: `src/blocks.rs` is rustfmt-clean under the
/// default configuration, and the contrasting configuration written by the
/// harness rewrites it by wrapping match-arm values and closure bodies in
/// `{ }` (the shapes v2 makes transparent) — so `stable_pct.fmt_contrast`
/// = 100 on the fixture is a statement about v2, not about an untouched file.
#[test]
fn contrast_config_wraps_fixture_blocks_and_every_hash_stays_stable() {
    let scratch = Scratch::new("blocks");
    let out = scratch.join("out");
    let output = run(&["ast-hash", "--out", out.to_str().unwrap()]);
    let result = &envelope(&output)["result"];
    assert_eq!(
        number(&result["stable_pct"]["fmt_contrast"], "fmt_contrast"),
        100.0
    );
    assert_eq!(
        number(&result["stable_pct"]["fmt_default"], "fmt_default"),
        100.0
    );
    assert!(count(&result["files_changed"]["fmt_contrast"], "fmt_contrast") >= 2);

    let detail = out.join("ast-hash").join("fixtures");
    let unstable: Value =
        serde_json::from_str(&fs::read_to_string(detail.join("unstable.json")).unwrap()).unwrap();
    assert_eq!(unstable, Value::Array(Vec::new()), "no unstable items");

    // The same rustfmt the harness used, with the configs it wrote to --out:
    // the same command, a relative path resolved from the harness's
    // directory (the repository root here), every call run at `/` as the
    // harness runs it (docs/features/pointer-sweep.md); the version it
    // reports proves it.
    let source_path = fixture_dir().join("src").join("blocks.rs");
    let source = fs::read_to_string(&source_path).expect("fixture file readable");
    let rustfmt = std::env::var("SPECENGINE_RUSTFMT")
        .ok()
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from("rustfmt"), PathBuf::from);
    let rustfmt = if rustfmt.is_relative() && rustfmt.components().count() > 1 {
        repository_root().join(rustfmt)
    } else {
        rustfmt
    };
    let version = Command::new(&rustfmt)
        .arg("--version")
        .current_dir("/")
        .output()
        .expect("rustfmt --version runs");
    assert_eq!(
        result["detail"]["rustfmt"]["version"],
        String::from_utf8_lossy(&version.stdout).trim(),
        "the comparison rustfmt is not the one the harness used"
    );
    // Through stdin, as the harness does: with a file argument rustfmt would
    // prefix the output with the file's name.
    let format = |config: &str| -> String {
        use std::io::Write;
        use std::process::Stdio;
        let mut child = Command::new(&rustfmt)
            .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
            .arg(detail.join(config))
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("rustfmt spawns");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .expect("rustfmt reads stdin");
        let out = child.wait_with_output().expect("rustfmt finishes");
        assert!(
            out.status.success(),
            "rustfmt failed on the fixture file: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(
        format("rustfmt-default.toml"),
        source,
        "src/blocks.rs must be rustfmt-clean under the default configuration"
    );
    let contrast = format("rustfmt-contrast.toml");
    assert_ne!(
        contrast, source,
        "the contrasting configuration must rewrite src/blocks.rs"
    );
    let (arms_before, closures_before) = block_openers(&source);
    let (arms_after, closures_after) = block_openers(&contrast);
    assert!(
        arms_after > arms_before,
        "contrast must wrap match-arm values in blocks: {arms_before} -> {arms_after}"
    );
    assert!(
        closures_after > closures_before,
        "contrast must wrap closure bodies in blocks: {closures_before} -> {closures_after}"
    );

    // Every item of the file was hashed and compared under both perturbations.
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(detail.join("manifest.json")).unwrap()).unwrap();
    let file = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"].as_str().unwrap().ends_with("blocks.rs"))
        .expect("blocks.rs in the manifest");
    let items = file["items"].as_array().unwrap();
    assert!(
        items.len() >= 10,
        "blocks.rs holds several items: {}",
        items.len()
    );
    for item in items {
        assert_eq!(item["has_error"], Value::Bool(false), "{item}");
        let hash = item["hash"].as_str().expect("hashed");
        for perturbation in ["fmt_default", "fmt_contrast", "comments"] {
            assert_eq!(
                item["perturbed"][perturbation].as_str(),
                Some(hash),
                "{}: hash must survive {perturbation}",
                item["label"]
            );
        }
    }
}

// ------------------------------------------------------------ AC-02

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

    /// Runs `ast-hash --pilot <corpus> --out <out>` and demands the refusal.
    fn refuse(&self, out: &Path, forbidden: &Path, context: &str) {
        let output = run(&[
            "ast-hash",
            "--pilot",
            self.arg(),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_refused(&output, forbidden, context);
        self.assert_untouched();
    }

    fn assert_untouched(&self) {
        assert_eq!(
            snapshot(&self.corpus),
            self.before,
            "the corpus must stay byte-identical"
        );
    }
}

#[test]
fn guard_refuses_out_directly_under_pilot() {
    let corpus = GuardCorpus::new("guard-direct");
    let out = corpus.corpus.join("scratch");
    corpus.refuse(&out, &out, "--out directly under --pilot");
}

#[test]
fn guard_refuses_out_nested_under_pilot() {
    let corpus = GuardCorpus::new("guard-nested");
    let out = corpus.corpus.join("src").join("deeper").join("out");
    corpus.refuse(&out, &out, "--out nested under --pilot");
}

#[test]
fn guard_refuses_out_equal_to_pilot() {
    let corpus = GuardCorpus::new("guard-equal");
    let output = run(&["ast-hash", "--pilot", corpus.arg(), "--out", corpus.arg()]);
    assert_eq!(output.status.code(), Some(2), "--out equal to --pilot");
    assert!(output.stdout.is_empty());
    corpus.assert_untouched();
}

#[test]
fn guard_refuses_out_reaching_pilot_through_dotdot() {
    let corpus = GuardCorpus::new("guard-dotdot");
    // `elsewhere` does not exist: the `..` must be resolved lexically.
    let out = corpus
        .scratch
        .join("elsewhere")
        .join("..")
        .join("corpus")
        .join("via-dotdot");
    corpus.refuse(
        &out,
        &corpus.corpus.join("via-dotdot"),
        "--out reaching the corpus through ..",
    );
}

#[test]
fn guard_refuses_out_reaching_pilot_through_existing_dotdot() {
    let corpus = GuardCorpus::new("guard-dotdot-existing");
    // `src/..` exists, so canonicalisation alone should catch it.
    let out = corpus
        .corpus
        .join("src")
        .join("..")
        .join("via-existing-dotdot");
    corpus.refuse(
        &out,
        &corpus.corpus.join("via-existing-dotdot"),
        "--out reaching the corpus through an existing ..",
    );
}

#[test]
fn guard_refuses_out_reaching_pilot_through_symlink() {
    let corpus = GuardCorpus::new("guard-symlink");
    let link = corpus.scratch.join("link");
    std::os::unix::fs::symlink(&corpus.corpus, &link).expect("symlink");
    let out = link.join("via-link");
    corpus.refuse(
        &out,
        &corpus.corpus.join("via-link"),
        "--out reaching the corpus through a symlink",
    );
}

#[test]
fn guard_refuses_relative_out_inside_pilot() {
    let corpus = GuardCorpus::new("guard-relative");
    let output = eval()
        .current_dir(&corpus.corpus)
        .args(["ast-hash", "--pilot", ".", "--out", "out-relative"])
        .output()
        .unwrap();
    assert_refused(
        &output,
        &corpus.corpus.join("out-relative"),
        "relative --out inside the corpus",
    );
    corpus.assert_untouched();
}

#[test]
fn guard_refuses_out_under_the_fixture_corpus() {
    let fixture = fixture_dir();
    let before = snapshot(&fixture);
    let out = fixture.join("scratch-out");
    let output = run(&["ast-hash", "--out", out.to_str().unwrap()]);
    assert_refused(&output, &out, "--out under the default fixture corpus");
    assert_eq!(snapshot(&fixture), before);
}

#[test]
fn out_beside_the_corpus_is_accepted_and_labelled_pilot() {
    let corpus = GuardCorpus::new("guard-beside");
    let beside = corpus.scratch.join("beside");
    let output = run(&[
        "ast-hash",
        "--pilot",
        corpus.arg(),
        "--out",
        beside.to_str().unwrap(),
    ]);
    let measured = envelope(&output);
    assert_envelope_shape(&measured, "pilot");
    assert_eq!(measured["result"]["items"], expected()["items"]);
    assert!(
        beside
            .join("ast-hash")
            .join("pilot")
            .join("manifest.json")
            .is_file()
    );
    corpus.assert_untouched();

    let labelled = corpus.scratch.join("labelled");
    let output = run(&[
        "ast-hash",
        "--pilot",
        corpus.arg(),
        "--label",
        "fixtures",
        "--out",
        labelled.to_str().unwrap(),
    ]);
    assert_eq!(envelope(&output)["label"], "fixtures");
}

#[test]
fn unreadable_pilot_and_unset_pilot_label_are_refused() {
    let scratch = Scratch::new("refusals");
    let out = scratch.join("out");
    let missing = scratch.join("does-not-exist");
    let output = run(&[
        "ast-hash",
        "--pilot",
        missing.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_refused(&output, &out, "missing --pilot");

    let file = scratch.join("a-file.rs");
    fs::write(&file, "fn f() {}").unwrap();
    let output = run(&[
        "ast-hash",
        "--pilot",
        file.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_refused(&output, &out, "--pilot is a file");

    for label in ["pilot-a", "pilot-b"] {
        let output = run(&["ast-hash", "--label", label, "--out", out.to_str().unwrap()]);
        assert_refused(
            &output,
            &out,
            "--label without --pilot or the environment variable",
        );
    }
}

#[test]
fn timeout_reports_the_string_timeout_with_exit_zero() {
    let scratch = Scratch::new("timeout");
    let output = run(&[
        "ast-hash",
        "--out",
        scratch.join("out").to_str().unwrap(),
        "--timeout",
        "0",
    ]);
    let envelope = envelope(&output);
    assert_envelope_shape(&envelope, "fixtures");
    assert_eq!(envelope["result"], Value::String("timeout".to_owned()));
}

// ------------------------------------------------------------ AC-05

#[cfg(feature = "syn")]
#[test]
fn syn_comparison_prints_failures_stability_and_time_ratio() {
    let scratch = Scratch::new("syn");
    let output = run(&["ast-hash", "--out", scratch.join("out").to_str().unwrap()]);
    let envelope = envelope(&output);
    let syn = &envelope["result"]["syn"];
    assert!(
        syn.is_object(),
        "syn must be an object with the feature, got {syn}"
    );
    assert_eq!(
        syn["files_failed"],
        expected()["syn"]["files_failed"],
        "whole-file failures"
    );
    for key in ["naive_stable_pct", "normalized_stable_pct"] {
        let value = number(&syn[key], key);
        assert!((0.0..=100.0).contains(&value), "{key} = {value}");
    }
    assert!(number(&syn["time_ratio"], "time_ratio") > 0.0);
    for key in ["naive_by_perturbation", "normalized_by_perturbation"] {
        let map = syn[key]
            .as_object()
            .unwrap_or_else(|| panic!("{key} is an object"));
        assert_eq!(
            map.len(),
            3,
            "{key} covers the three perturbations: {map:?}"
        );
        for (perturbation, value) in map {
            let value = number(value, perturbation);
            assert!(
                (0.0..=100.0).contains(&value),
                "{key}.{perturbation} = {value}"
            );
        }
    }
    let naive = number(&syn["naive_stable_pct"], "naive");
    let normalized = number(&syn["normalized_stable_pct"], "normalized");
    assert!(
        normalized >= naive,
        "normalization must not lose stability: {naive} -> {normalized}"
    );
    eprintln!("syn summary on the fixture: {syn}");
}

#[cfg(not(feature = "syn"))]
#[test]
fn syn_summary_is_null_without_the_feature() {
    let scratch = Scratch::new("no-syn");
    let output = run(&["ast-hash", "--out", scratch.join("out").to_str().unwrap()]);
    assert_eq!(envelope(&output)["result"]["syn"], Value::Null);
}

// ------------------------------------------------------------ AC-06 (pilots, read-only)

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
            "ast-hash",
            "--label",
            label,
            "--out",
            out.to_str().unwrap(),
            "--timeout",
            "300",
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
    assert_all_rows_numeric(result);
    assert!(
        count(&result["items"], "items") >= 1000,
        "sanity floor: items >= 1000"
    );
    assert!(count(&result["files"], "files") >= 1);
    assert!(
        result["detail"]["stability"]["fmt_default"]["compared"]
            .as_u64()
            .unwrap()
            >= 1000
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
    assert!(!stdout.contains(".rs"), "a file name leaked to stdout");

    assert_eq!(
        git_status(&pilot, None),
        status_before,
        "the pilot must stay untouched"
    );
    eprintln!("{label} result: {result}");

    // docs/features/layer-a-identity.md AC-06: every remaining `duplicate`
    // group lies within one file (adjacent impls, `cfg` twins); a group
    // across files is a unit the rules failed to tell apart. Counts only.
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(out.join("ast-hash").join(label).join("manifest.json"))
            .expect("manifest.json"),
    )
    .expect("manifest is JSON");
    let mut groups: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    for file in manifest["files"].as_array().expect("files") {
        for item in file["items"].as_array().expect("items") {
            if item["ambiguity"] == "duplicate" {
                groups
                    .entry(item["qpath"].as_str().unwrap().to_owned())
                    .or_default()
                    .insert(file["path"].as_str().unwrap().to_owned());
            }
        }
    }
    let across = groups.values().filter(|files| files.len() > 1).count();
    eprintln!(
        "{label} duplicate groups: {}, across files: {across}",
        groups.len()
    );
    assert_eq!(across, 0, "duplicate groups spanning several files");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A; read-only run over a real corpus"]
fn pilot_a_finishes_with_every_field_numeric() {
    pilot_run("SPECENGINE_PILOT_A", "pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B; read-only run over a real corpus"]
fn pilot_b_finishes_with_every_field_numeric() {
    pilot_run("SPECENGINE_PILOT_B", "pilot-b");
}
