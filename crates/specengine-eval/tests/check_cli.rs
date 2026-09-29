//! `specengine-eval check` end to end (docs/features/spec-check.md AC-22)
//! on the real binary over `fixtures/spec-a` (the default) and spec-b: one
//! anonymous JSON envelope — counts per code and severity, the verdict under
//! both modes, `wall_ms` — with no path or ID on stdout; the report only in
//! `--out/check/<label>/findings.json`; the fixtures untouched. `--out`
//! under the corpus, an invalid config, baseline or `--today` are refused
//! with exit 2 and nothing written.
//!
//! Pilot runs are `#[ignore]` and owner-run (spec, "Out of scope"): the
//! corpus from `SPECENGINE_PILOT_A` / `_B`, the config from
//! `SPECENGINE_SCHEME_A` / `_B` (outside the repository, read-only); the
//! counts are informational.

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

const ENV: [&str; 4] = [
    "SPECENGINE_PILOT_A",
    "SPECENGINE_PILOT_B",
    "SPECENGINE_SCHEME_A",
    "SPECENGINE_SCHEME_B",
];

const TODAY: &str = "2026-09-29";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-check-{name}-{}",
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

fn eval() -> Command {
    let mut command = Command::new(BIN);
    command.current_dir(repository_root());
    for variable in ENV {
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
    assert_eq!(
        output.status.code(),
        Some(0),
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
    for (relative, bytes) in snapshot(from) {
        let target = to.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).expect("parent");
        fs::write(target, bytes).expect("copy");
    }
}

fn fixtures_git_status() -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .args([
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "fixtures/",
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The fixed keys; `result` of counts, verdicts and per-code counts; no
/// path, ID or corpus name on stdout.
fn assert_anonymous(output: &Output, envelope: &Value, label: &str, corpus: &Path) {
    let object = envelope.as_object().expect("envelope object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["label", "measurement", "result", "versions", "wall_ms"]
            .into_iter()
            .collect()
    );
    assert_eq!(envelope["measurement"], "check");
    assert_eq!(envelope["label"], label);
    assert!(
        envelope["wall_ms"].is_u64(),
        "wall_ms: {}",
        envelope["wall_ms"]
    );
    let result = envelope["result"].as_object().expect("result object");
    assert_eq!(
        result.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["codes", "expired", "files", "stale", "verdicts"]
            .into_iter()
            .collect(),
        "result keys"
    );
    for key in ["expired", "files", "stale"] {
        assert!(
            result[key].is_u64(),
            "result.{key} is a count: {}",
            result[key]
        );
    }
    let verdicts = result["verdicts"].as_object().expect("verdicts");
    assert_eq!(
        verdicts.keys().map(String::as_str).collect::<Vec<_>>(),
        ["enforce", "observe"]
    );
    for verdict in verdicts.values() {
        assert!(
            ["clean", "observed", "blocked", "cannot-check"]
                .contains(&verdict.as_str().expect("a verdict string")),
            "{verdict}"
        );
    }
    for (code, counts) in result["codes"].as_object().expect("codes") {
        assert!(
            code.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "a code, not a name: {code:?}"
        );
        let counts = counts.as_object().expect("per-code counts");
        assert_eq!(
            counts.keys().map(String::as_str).collect::<Vec<_>>(),
            ["debt", "error", "warning"],
            "{code}"
        );
        assert!(counts.values().all(Value::is_u64), "{code}: {counts:?}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for leak in [
        ".md",
        "/",
        "\\",
        "docs",
        "stamina",
        "R-12",
        "DEC-",
        "MEC-",
        "QST-",
        "REQ-",
        "MOD-",
        "QN-",
        "\u{0422}\u{0420}\u{0411}",
    ] {
        assert!(
            !stdout.contains(leak),
            "{leak:?} leaked to stdout: {stdout}"
        );
    }
    assert!(
        !stdout.contains(corpus.to_str().unwrap()),
        "corpus path on stdout"
    );
}

fn codes(envelope: &Value) -> BTreeMap<String, (u64, u64, u64)> {
    envelope["result"]["codes"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(code, counts)| {
            (
                code.clone(),
                (
                    counts["error"].as_u64().unwrap(),
                    counts["warning"].as_u64().unwrap(),
                    counts["debt"].as_u64().unwrap(),
                ),
            )
        })
        .collect()
}

#[test]
fn spec_a_by_default_gives_one_anonymous_envelope_and_the_report_under_out() {
    let before = fixtures_git_status();
    let scratch = Scratch::new("default");
    let out = scratch.join("out");
    let output = run(&["check", "--out", out.to_str().unwrap(), "--today", TODAY]);
    let envelope = envelope(&output);
    assert_anonymous(&output, &envelope, "fixtures", &fixture("spec-a"));
    let result = &envelope["result"];
    assert_eq!(result["files"], 14);
    assert_eq!(result["verdicts"]["observe"], "observed");
    assert_eq!(result["verdicts"]["enforce"], "blocked");
    assert_eq!(
        (result["expired"].as_u64(), result["stale"].as_u64()),
        (Some(0), Some(0))
    );
    let codes = codes(&envelope);
    assert_eq!(codes.get("frontmatter-type"), Some(&(1, 0, 0)), "{codes:?}");
    assert!(
        !codes.contains_key("class-missing"),
        "every fixture document declares its class"
    );
    assert!(
        codes
            .iter()
            .filter(|(code, _)| code.as_str() != "frontmatter-type")
            .all(|(_, (error, _, _))| *error == 0),
        "spec-a blocks only on its type error (AC-18): {codes:?}"
    );

    // The detail: the report's JSON, with the names stdout must not carry.
    let detail = out.join("check").join("fixtures").join("findings.json");
    let report: Value =
        serde_json::from_str(&fs::read_to_string(&detail).expect("findings.json")).unwrap();
    assert_eq!(report["mode"], "enforce");
    assert_eq!(report["verdict"], "blocked");
    assert_eq!(report["counts"]["documents"], 14);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "docs/features/stamina-tuning.md"
                && f["code"] == "frontmatter-type")
    );
    assert_eq!(fixtures_git_status(), before, "fixtures/ changed");
}

#[test]
fn spec_b_with_a_baseline_moves_its_errors_into_debt() {
    let before = fixtures_git_status();
    let scratch = Scratch::new("spec-b");
    let baseline = scratch.join("debt.toml");
    fs::write(
        &baseline,
        "[[debt]]\ncode = \"frontmatter-yaml\"\npath = \"docs/records/QN/QN-08.md\"\nreason = \"fixture\"\nexpires = \"2026-12-31\"\n\n[[debt]]\ncode = \"unknown-code\"\npath = \"docs/none.md\"\nreason = \"stale\"\nexpires = \"2026-12-31\"\n",
    )
    .unwrap();
    let out = scratch.join("out");
    let corpus = fixture("spec-b");
    let output = run(&[
        "check",
        "--pilot",
        corpus.to_str().unwrap(),
        "--label",
        "spec-b",
        "--baseline",
        baseline.to_str().unwrap(),
        "--today",
        TODAY,
        "--out",
        out.to_str().unwrap(),
    ]);
    let now = envelope(&output);
    assert_anonymous(&output, &now, "spec-b", &corpus);
    let counts = codes(&now);
    assert_eq!(
        counts.get("frontmatter-yaml"),
        Some(&(0, 0, 1)),
        "{counts:?}"
    );
    assert_eq!(counts.get("homoglyph"), Some(&(2, 0, 0)), "{counts:?}");
    assert_eq!(counts.get("ref-dangling"), Some(&(2, 0, 0)), "{counts:?}");
    assert!(!counts.contains_key("class-missing"), "{counts:?}");
    assert_eq!(now["result"]["stale"], 1);
    assert_eq!(now["result"]["verdicts"]["enforce"], "blocked");
    // Past the expiry the debt is an error again.
    let output = run(&[
        "check",
        "--pilot",
        corpus.to_str().unwrap(),
        "--label",
        "spec-b-later",
        "--baseline",
        baseline.to_str().unwrap(),
        "--today",
        "2027-01-01",
        "--out",
        out.to_str().unwrap(),
    ]);
    let later = envelope(&output);
    assert_eq!(codes(&later).get("frontmatter-yaml"), Some(&(1, 0, 0)));
    assert_eq!(later["result"]["expired"], 1);
    assert_eq!(fixtures_git_status(), before, "fixtures/ changed");
}

/// Exit 2, nothing on stdout, nothing under `out`, the corpus unchanged.
fn assert_refused(
    context: &str,
    output: &Output,
    out: &Path,
    corpus: &Path,
    before: &BTreeMap<PathBuf, Vec<u8>>,
) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr(output)
    );
    assert!(output.stdout.is_empty(), "{context}: stdout not empty");
    assert!(
        !out.join("check").exists(),
        "{context}: something was written under --out"
    );
    assert!(&snapshot(corpus) == before, "{context}: the corpus changed");
}

#[test]
fn out_under_the_corpus_is_refused_and_nothing_is_written() {
    let scratch = Scratch::new("out-under");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-a"), &corpus);
    let before = snapshot(&corpus);
    for out in [corpus.join("out"), corpus.join("docs")] {
        let output = run(&[
            "check",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ]);
        assert_refused("--out under the corpus", &output, &out, &corpus, &before);
        assert!(stderr(&output).contains("--out"), "{}", stderr(&output));
    }
}

#[test]
fn an_invalid_config_baseline_or_date_is_refused() {
    let scratch = Scratch::new("invalid");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-a"), &corpus);
    let before = snapshot(&corpus);
    let out = scratch.join("out");
    let bad_config = scratch.join("bad.toml");
    let text = fs::read_to_string(corpus.join("specengine.toml")).unwrap();
    fs::write(&bad_config, format!("{text}\n[check]\nmode = \"strict\"\n")).unwrap();
    let line = text.lines().count() + 3;
    let bad_baseline = scratch.join("debt.toml");
    fs::write(
        &bad_baseline,
        "[[debt]]\ncode = \"budget\"\npath = \"x.md\"\nexpires = \"2026-12-31\"\n",
    )
    .unwrap();
    let corpus_arg = corpus.to_str().unwrap();
    let out_arg = out.to_str().unwrap();
    let cases: [(&str, Vec<&str>, String); 4] = [
        (
            "invalid config",
            vec!["--scheme", bad_config.to_str().unwrap()],
            format!("bad.toml:{line}: "),
        ),
        (
            "missing config",
            vec!["--scheme", "/nonexistent/specengine.toml"],
            "cannot read the config".to_owned(),
        ),
        (
            "invalid baseline",
            vec!["--baseline", bad_baseline.to_str().unwrap()],
            "debt.toml:1: ".to_owned(),
        ),
        (
            "invalid date",
            vec!["--today", "2026-02-30"],
            "--today".to_owned(),
        ),
    ];
    for (context, extra, message) in cases {
        let mut args = vec!["check", "--pilot", corpus_arg, "--out", out_arg];
        if !extra.contains(&"--today") {
            args.extend(["--today", TODAY]);
        }
        args.extend(extra.iter().copied());
        let output = run(&args);
        assert_refused(context, &output, &out, &corpus, &before);
        assert!(
            stderr(&output).contains(&message),
            "{context}: {message:?} not in stderr:\n{}",
            stderr(&output)
        );
    }
}

fn pilot_run(pilot_variable: &str, scheme_variable: &str, label: &str) {
    let Some(pilot) = std::env::var_os(pilot_variable).filter(|v| !v.is_empty()) else {
        panic!("set {pilot_variable} to the pilot corpus (outside the repository)");
    };
    let Some(scheme) = std::env::var_os(scheme_variable).filter(|v| !v.is_empty()) else {
        panic!("set {scheme_variable} to the pilot's config (outside the repository)");
    };
    let pilot = fs::canonicalize(pilot).expect("pilot path exists");
    let snapshot_before = snapshot(&pilot);
    let scratch = Scratch::new(label);
    let out = scratch.join("out");
    let output = eval()
        .env(scheme_variable, &scheme)
        .args([
            "check",
            "--pilot",
            pilot.to_str().unwrap(),
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
    assert_anonymous(&output, &envelope, label, &pilot);
    eprintln!("{label} result (informational): {}", envelope["result"]);
    assert!(
        snapshot(&pilot) == snapshot_before,
        "the pilot must stay untouched"
    );
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A and SPECENGINE_SCHEME_A; read-only, owner-run, informational"]
fn pilot_a_check_runs_read_only() {
    pilot_run("SPECENGINE_PILOT_A", "SPECENGINE_SCHEME_A", "pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B and SPECENGINE_SCHEME_B; read-only, owner-run, informational"]
fn pilot_b_check_runs_read_only() {
    pilot_run("SPECENGINE_PILOT_B", "SPECENGINE_SCHEME_B", "pilot-b");
}

/// AC-21 of docs/features/phase1-cleanup.md (E6): `check` takes its
/// configuration from `--scheme` only; a `--config` is refused (exit 2)
/// before anything is written.
#[test]
fn check_refuses_config() {
    let scratch = Scratch::new("config");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-a"), &corpus);
    let before = snapshot(&corpus);
    let config = scratch.join("census.toml");
    fs::write(&config, "[corpus]\nroots = [\"docs\"]\n").unwrap();
    let out = scratch.join("out");
    let output = run(&[
        "check",
        "--pilot",
        corpus.to_str().unwrap(),
        "--config",
        config.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused("--config", &output, &out, &corpus, &before);
    assert!(!out.exists(), "--out was created");
    let message = stderr(&output);
    assert_eq!(
        message.trim_end(),
        format!(
            "check: refused: --config {} is not read by `check` (its configuration is --scheme); nothing written",
            config.display()
        )
    );
}
