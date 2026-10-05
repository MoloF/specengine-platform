//! `specengine-eval parse` end to end (docs/features/spec-parser.md AC-20)
//! on the real binary over `fixtures/corpus-mini` and its `specengine.toml`:
//! one anonymous envelope with the documented `result` keys, no panic, no
//! section disagreement with the census, detail only under `--out`, the
//! fixtures untouched; a missing or invalid scheme is refused (exit 2) with
//! `file:line: message`; `--scheme` and `SPECENGINE_SCHEME_A` / `_B` choose
//! the scheme.
//!
//! Pilot runs are `#[ignore]` and owner-run: the corpus comes from
//! `SPECENGINE_PILOT_A` / `_B`, the scheme from `SPECENGINE_SCHEME_A` / `_B`,
//! the census config from `SPECENGINE_CENSUS_CONFIG_A` / `_B`, all outside
//! the repository and read-only. Their helper
//! (docs/features/pilot-schemes.md) fails, never skips: a scheme without a
//! `[paths]` table fails naming its variable; the run must exit 0 with files
//! parsed and no panic, leave the read-only proof equal and the scratch
//! `HOME` empty. Non-ignored tests run the same helper on an invented setup.

#![cfg(unix)]

mod pilot;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

const ENV: [&str; 6] = [
    "SPECENGINE_PILOT_A",
    "SPECENGINE_PILOT_B",
    "SPECENGINE_SCHEME_A",
    "SPECENGINE_SCHEME_B",
    "SPECENGINE_CENSUS_CONFIG_A",
    "SPECENGINE_CENSUS_CONFIG_B",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn corpus_mini() -> PathBuf {
    repository_root().join("fixtures").join("corpus-mini")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-parse-{name}-{}",
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

fn copy_dir(from: &Path, to: &Path, skip: &[&str]) {
    for (relative, bytes) in snapshot(from) {
        if skip.iter().any(|s| relative == Path::new(s)) {
            continue;
        }
        let target = to.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).expect("parent");
        fs::write(target, bytes).expect("copy");
    }
}

fn git_status(dir: &Path, pathspec: &str) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            pathspec,
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
}

/// Every key of an object, recursively, as `a.b.c` paths.
fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    if let Value::Object(map) = value {
        for (key, child) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            out.insert(path.clone());
            key_paths(child, &path, out);
        }
    }
}

const CODES: [&str; 13] = [
    "not-utf8",
    "frontmatter-unclosed",
    "frontmatter-yaml",
    "frontmatter-not-mapping",
    "frontmatter-type",
    "id-not-in-scheme",
    "unknown-key",
    "unknown-link-type",
    "unparsed-reference",
    "homoglyph",
    "kind-mismatch",
    "duplicate-id",
    "bad-rev",
];

fn expected_result_keys() -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = [
        "files",
        "unreadable",
        "panics",
        "not_utf8",
        "front_matter",
        "front_matter.present",
        "diagnostics",
        "sections",
        "sections.parsed",
        "sections.census_id_sections",
        "sections.differ",
        "heading_attrs_not_section",
        "references",
        "references.inline",
        "references.declared",
        "references.homoglyph",
        "references.alias",
        "tokens_est",
        "tokens_est.total",
        "tokens_est.max_node",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    // Every parser code at the top level (E2 of
    // docs/features/phase1-cleanup.md), zeros included.
    for code in CODES {
        keys.insert(format!("diagnostics.{code}"));
    }
    keys
}

#[test]
fn codes_are_every_parser_code() {
    assert_eq!(
        CODES.iter().copied().collect::<BTreeSet<_>>(),
        specengine_model::DiagnosticCode::ALL
            .iter()
            .map(|code| code.as_str())
            .collect::<BTreeSet<_>>()
    );
}

/// Keys from the fixed schema; values counts, the label, or versions.
fn assert_anonymous(output: &Output, envelope: &Value, label: &str, corpus: &Path) {
    let object = envelope.as_object().expect("envelope object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["label", "measurement", "result", "versions", "wall_ms"]
            .into_iter()
            .collect()
    );
    assert_eq!(envelope["measurement"], "parse");
    assert_eq!(envelope["label"], label);
    let mut keys = BTreeSet::new();
    key_paths(&envelope["result"], "", &mut keys);
    assert_eq!(keys, expected_result_keys(), "result keys");
    fn all_counts(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.values().all(all_counts),
            Value::Number(n) => n.is_u64(),
            _ => false,
        }
    }
    assert!(
        all_counts(&envelope["result"]),
        "every result value is a count"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for leak in [".md", "/", "\\", "ZR-", "ZN-", "design", "rules", "notes"] {
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

// ------------------------------------------------------------------- AC-20

#[test]
fn corpus_mini_parses_into_one_anonymous_envelope() {
    let scratch = Scratch::new("mini");
    let out = scratch.join("out");
    let fixtures_before = snapshot(&corpus_mini());
    let status_before = git_status(&repository_root(), "fixtures/");
    let output = eval()
        .args(["parse", "--out", out.to_str().unwrap(), "--timeout", "60"])
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output);
    assert_anonymous(&output, &envelope, "fixtures", &corpus_mini());
    let result = &envelope["result"];
    assert_eq!(result["files"], 3);
    assert_eq!(result["unreadable"], 0);
    assert_eq!(result["panics"], 0);
    assert_eq!(result["not_utf8"], 0);
    assert_eq!(result["front_matter"]["present"], 3);
    assert_eq!(result["sections"]["parsed"], 1);
    assert_eq!(result["sections"]["census_id_sections"], 1);
    assert_eq!(result["sections"]["differ"], 0);
    assert_eq!(result["heading_attrs_not_section"], 0);
    // rules.md cites ZR-001, ZR-002, ZN-001 and a Greek-Zeta ZR-003: one
    // look-alike reference and its one `homoglyph` diagnostic.
    assert_eq!(result["references"]["inline"], 4);
    assert_eq!(result["references"]["declared"], 0);
    assert_eq!(result["references"]["homoglyph"], 1);
    assert_eq!(result["references"]["alias"], 0);
    for code in CODES {
        let want = u64::from(code == "homoglyph");
        assert_eq!(result["diagnostics"][code], want, "{code}");
    }

    // tokens_est agrees with the library over the same files.
    let scheme_text = fs::read_to_string(corpus_mini().join("specengine.toml")).unwrap();
    let scheme =
        <specengine_model::IdScheme as specengine_core::IdSchemeToml>::from_toml(&scheme_text)
            .unwrap();
    let mut total = 0u64;
    let mut max_node = 0u32;
    for (relative, bytes) in snapshot(&corpus_mini().join("design")) {
        let parsed = specengine_core::parse(relative.to_str().unwrap(), &bytes, &scheme);
        total += u64::from(parsed.nodes[0].tokens_est);
        max_node = max_node.max(parsed.nodes.iter().map(|n| n.tokens_est).max().unwrap());
    }
    assert_eq!(result["tokens_est"]["total"], total);
    assert_eq!(result["tokens_est"]["max_node"], max_node);

    // Detail only under --out/parse/<label>/.
    let detail = out.join("parse").join("fixtures");
    for file in [
        "files.json",
        "diagnostics.json",
        "sections_differ.json",
        "panics.json",
        "problems.json",
    ] {
        assert!(detail.join(file).is_file(), "{file} under --out");
    }
    let diagnostics = read_json(&detail.join("diagnostics.json"));
    assert_eq!(diagnostics[0]["path"], "design/rules.md");
    assert_eq!(diagnostics[0]["code"], "homoglyph");
    assert_eq!(diagnostics[0]["line"], 15);
    assert_eq!(
        read_json(&detail.join("panics.json")),
        serde_json::json!([])
    );
    assert_eq!(
        read_json(&detail.join("sections_differ.json")),
        serde_json::json!([])
    );
    let files = read_json(&detail.join("files.json"));
    assert_eq!(files.as_array().map(Vec::len), Some(3));

    assert_eq!(
        snapshot(&corpus_mini()),
        fixtures_before,
        "fixture bytes untouched"
    );
    assert_eq!(
        git_status(&repository_root(), "fixtures/"),
        status_before,
        "git status -- fixtures/ unchanged by the run"
    );
}

#[test]
fn the_same_run_twice_prints_the_same_result() {
    let scratch = Scratch::new("twice");
    let mut results = Vec::new();
    for round in ["a", "b"] {
        let out = scratch.join(round);
        let output = eval()
            .args(["parse", "--out", out.to_str().unwrap()])
            .output()
            .unwrap();
        results.push(envelope(&output)["result"].clone());
        let detail = out.join("parse").join("fixtures");
        results.push(read_json(&detail.join("files.json")));
    }
    assert_eq!(results[0], results[2]);
    assert_eq!(results[1], results[3]);
}

// --------------------------------------------------------------- the scheme

fn assert_refused(output: &Output, out: &Path, context: &str) -> String {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: exit 2 expected, stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr(output)
    );
    assert!(output.stdout.is_empty(), "{context}: nothing on stdout");
    assert!(!out.exists(), "{context}: --out was created");
    stderr(output)
}

#[test]
fn invalid_scheme_is_refused_with_file_line_message() {
    let scratch = Scratch::new("bad-scheme");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &["specengine.toml"]);
    let scheme = corpus.join("specengine.toml");
    fs::write(
        &scheme,
        "[ids]\nZR = { kind = \"rule\", width = 3 }\nZN = { kind = \"note\" }\n",
    )
    .unwrap();
    let before = snapshot(&corpus);
    let out = scratch.join("out");
    let output = eval()
        .args([
            "parse",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            "bad",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let message = assert_refused(&output, &out, "missing width");
    let located = format!("{}:3: ", scheme.display());
    assert!(
        message.contains(&located),
        "stderr names {located:?}:\n{message}"
    );
    assert_eq!(snapshot(&corpus), before, "the corpus is untouched");
}

#[test]
fn missing_scheme_is_refused() {
    let scratch = Scratch::new("no-scheme");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &["specengine.toml"]);
    let out = scratch.join("out");
    let output = eval()
        .args([
            "parse",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            "none",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let message = assert_refused(&output, &out, "no specengine.toml");
    assert!(message.contains("specengine.toml"), "{message}");
}

#[test]
fn explicit_scheme_flag_is_used() {
    let scratch = Scratch::new("flag");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &["specengine.toml"]);
    // Only ZN: the ZR-004 section is no longer an ID section of the scheme.
    let scheme = scratch.join("ids.toml");
    fs::write(&scheme, "[ids]\nZN = { kind = \"note\", width = 3 }\n").unwrap();
    let out = scratch.join("out");
    let output = eval()
        .args([
            "parse",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            "flag",
            "--scheme",
            scheme.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let envelope = envelope(&output);
    let result = &envelope["result"];
    assert_eq!(result["sections"]["parsed"], 0);
    assert_eq!(
        result["heading_attrs_not_section"], 1,
        "{{#ZR-004}} is an anchor now"
    );
    assert_eq!(result["references"]["inline"], 1, "only ZN-001");
    assert_eq!(
        result["sections"]["differ"], 1,
        "the census still sees ZR-004"
    );
}

#[test]
fn pilot_labels_take_the_scheme_from_the_environment() {
    let scratch = Scratch::new("env");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &["specengine.toml"]);
    let good = scratch.join("scheme-a.toml");
    fs::write(&good, "[ids]\nZR = { kind = \"rule\", width = 3 }\n").unwrap();
    let bad = scratch.join("scheme-b.toml");
    fs::write(
        &bad,
        "[ids]\n\nZR = { kind = \"rule\", width = 3, script = \"latin\" }\n",
    )
    .unwrap();
    for (label, variable, file, ok) in [
        ("pilot-a", "SPECENGINE_SCHEME_A", &good, true),
        ("pilot-b", "SPECENGINE_SCHEME_B", &bad, false),
    ] {
        let out = scratch.join(&format!("out-{label}"));
        let output = eval()
            .env(variable, file)
            .args([
                "parse",
                "--pilot",
                corpus.to_str().unwrap(),
                "--label",
                label,
                "--out",
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        if ok {
            let envelope = envelope(&output);
            assert_eq!(envelope["label"], label);
            assert_eq!(envelope["result"]["sections"]["parsed"], 1);
            assert_eq!(envelope["result"]["references"]["inline"], 3, "ZR only");
        } else {
            let message = assert_refused(&output, &out, label);
            let located = format!("{}:3: ", file.display());
            assert!(
                message.contains(&located),
                "{label}: {located:?} in\n{message}"
            );
        }
    }
    // The environment is read only for the pilot labels.
    let out = scratch.join("out-other");
    let output = eval()
        .env("SPECENGINE_SCHEME_A", &bad)
        .args([
            "parse",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            "other",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_refused(&output, &out, "label other, no specengine.toml");
}

// ---------------------------------------------------------------- pilots

/// One `parse` run over `inputs`
/// (`crates/specengine-eval/README.md` "Pilot runs and tests"):
/// `pilot::run_read_only` (the `[paths]` requirement, the proof, the child
/// environment with the census-config variable, exit 0, `HOME` empty), the
/// anonymous envelope, files parsed, no panic.
fn pilot_run(label: &str, inputs: &pilot::Inputs, scratch: &Path) -> Value {
    assert!(inputs.config.is_some(), "parse needs the census config");
    let output = pilot::run_read_only("parse", label, scratch, inputs);
    let envelope = envelope(&output);
    assert_anonymous(&output, &envelope, label, inputs.corpus);
    let result = &envelope["result"];
    eprintln!("{label} result: {result}");
    assert!(
        result["files"].as_u64().is_some_and(|files| files > 0),
        "parse reads files: {result}"
    );
    assert_eq!(result["panics"], 0, "{result}");
    envelope
}

/// The `#[ignore]` pilot test: the corpus, the scheme and the census config
/// from the label's variables, each required.
fn pilot_from_environment(label: &str) {
    let variables = pilot::variables(label);
    let corpus = pilot::required(variables.corpus, "the pilot corpus");
    let corpus = fs::canonicalize(corpus).unwrap_or_else(|error| {
        panic!(
            "{}: the pilot corpus is unreadable: {error}",
            variables.corpus
        )
    });
    let scheme = pilot::required(variables.scheme, "the pilot's scheme");
    let config = pilot::required(variables.config, "the pilot's census config");
    let scratch = Scratch::new(label);
    let inputs = pilot::Inputs {
        corpus: &corpus,
        scheme: &scheme,
        config: Some(&config),
    };
    pilot_run(label, &inputs, &scratch.0);
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A (with [paths]), SPECENGINE_CENSUS_CONFIG_A; read-only, owner-run"]
fn pilot_a_parse_prints_anonymous_counts() {
    pilot_from_environment("pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B (with [paths]), SPECENGINE_CENSUS_CONFIG_B; read-only, owner-run"]
fn pilot_b_parse_prints_anonymous_counts() {
    pilot_from_environment("pilot-b");
}

/// docs/features/pilot-schemes.md AC-01, AC-02: the pilot helper on the
/// invented setup (a spec-b copy, `git init`, a scheme with every recipe
/// table, the three variables on the child, no `--config`) is green; the
/// scheme's escape-written alias prefix is read; the detail lands only under
/// `--out/parse/pilot-a`.
#[test]
fn the_pilot_helper_parses_the_invented_setup_read_only() {
    let scratch = Scratch::new("pilot-setup");
    let setup = pilot::invented_setup(&scratch.0, true);
    let envelope = pilot_run("pilot-a", &setup.inputs(), &scratch.0);
    let result = &envelope["result"];
    let documents = snapshot(&setup.corpus.join("docs"))
        .into_keys()
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .count();
    assert_eq!(
        result["files"], documents,
        "the census config's docs/: {result}"
    );
    assert!(
        result["references"]["alias"]
            .as_u64()
            .is_some_and(|alias| alias > 0),
        "references through the aliases_from prefix: {result}"
    );
    let out = scratch.join("out");
    assert!(
        snapshot(&out)
            .into_keys()
            .all(|path| path.starts_with("parse/pilot-a")),
        "only under --out/parse/pilot-a"
    );
}

/// docs/features/pilot-schemes.md AC-02: without `[paths]` the helper fails
/// naming the scheme variable, before anything runs; it does not skip.
#[test]
#[should_panic(expected = "SPECENGINE_SCHEME_A: the scheme sets no [paths] table")]
fn the_pilot_helper_fails_on_a_scheme_without_paths() {
    let scratch = Scratch::new("pilot-no-paths");
    let setup = pilot::invented_setup(&scratch.0, false);
    pilot_run("pilot-a", &setup.inputs(), &scratch.0);
}

/// docs/features/pilot-schemes.md AC-01 (the "Runs" line gives `parse`
/// without `--config`): for `--label pilot-a` / `pilot-b` the census config
/// defaults to `SPECENGINE_CENSUS_CONFIG_A` / `_B` when set and not empty;
/// `--config` beats it; otherwise `census.toml` at the corpus root (none
/// here: refused). Only the census-config variable is set.
#[test]
fn pilot_labels_take_the_census_config_from_the_environment() {
    let scratch = Scratch::new("census-env");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &["census.toml"]);
    assert!(!corpus.join("census.toml").exists());
    let good = scratch.join("census-a.toml");
    fs::write(&good, fs::read(corpus_mini().join("census.toml")).unwrap()).unwrap();
    let bad = scratch.join("census-bad.toml");
    fs::write(&bad, "[corpus]\nroots = [\"design\"]\nsurprise = true\n").unwrap();
    let parse =
        |label: &str, variable: Option<(&str, &Path)>, config: Option<&Path>, name: &str| {
            let out = scratch.join(name);
            let mut command = eval();
            if let Some((variable, value)) = variable {
                command.env(variable, value);
            }
            command.args([
                "parse",
                "--pilot",
                corpus.to_str().unwrap(),
                "--label",
                label,
                "--out",
                out.to_str().unwrap(),
            ]);
            if let Some(config) = config {
                command.args(["--config", config.to_str().unwrap()]);
            }
            (command.output().expect("specengine-eval runs"), out)
        };
    let (flag, _) = parse("pilot-a", None, Some(&good), "out-flag");
    let by_flag = envelope(&flag)["result"].clone();
    assert_eq!(by_flag["sections"]["parsed"], 1, "{by_flag}");

    // The variable alone: the same result as the flag.
    for (label, variable) in [
        ("pilot-a", "SPECENGINE_CENSUS_CONFIG_A"),
        ("pilot-b", "SPECENGINE_CENSUS_CONFIG_B"),
    ] {
        let (output, _) = parse(
            label,
            Some((variable, &good)),
            None,
            &format!("out-{label}"),
        );
        assert_eq!(envelope(&output)["result"], by_flag, "{variable} alone");
    }
    // --config beats the variable.
    let (output, _) = parse(
        "pilot-a",
        Some(("SPECENGINE_CENSUS_CONFIG_A", &bad)),
        Some(&good),
        "out-beats",
    );
    assert_eq!(
        envelope(&output)["result"],
        by_flag,
        "--config beats the variable"
    );
    // The variable is read: a bad config there is refused at its line.
    let (output, out) = parse(
        "pilot-a",
        Some(("SPECENGINE_CENSUS_CONFIG_A", &bad)),
        None,
        "out-bad",
    );
    let message = assert_refused(&output, &out, "a bad config in the variable");
    let located = format!("{}:3: ", bad.display());
    assert!(message.contains(&located), "{located:?} in\n{message}");
    // Empty, the other label's or a non-pilot label: no config, refused.
    let empty = PathBuf::new();
    for (label, variable, value, name) in [
        (
            "pilot-a",
            "SPECENGINE_CENSUS_CONFIG_A",
            empty.as_path(),
            "out-empty",
        ),
        (
            "pilot-b",
            "SPECENGINE_CENSUS_CONFIG_A",
            good.as_path(),
            "out-other-b",
        ),
        (
            "pilot-a",
            "SPECENGINE_CENSUS_CONFIG_B",
            good.as_path(),
            "out-other-a",
        ),
        (
            "other",
            "SPECENGINE_CENSUS_CONFIG_A",
            good.as_path(),
            "out-other",
        ),
    ] {
        let (output, out) = parse(label, Some((variable, value)), None, name);
        let message = assert_refused(&output, &out, &format!("{label} with {variable}"));
        assert!(message.contains("census.toml"), "{label}: {message}");
    }
}

/// docs/features/pilot-schemes.md: a pilot label's refusal names the variable
/// it looked for — the census config first, then the scheme — any other label
/// (or none) keeps the old text.
#[test]
fn pilot_label_refusals_name_the_variable_they_looked_for() {
    let scratch = Scratch::new("refusal-names");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &["census.toml", "specengine.toml"]);
    let before = snapshot(&corpus);
    let config = scratch.join("census-a.toml");
    fs::write(
        &config,
        fs::read(corpus_mini().join("census.toml")).unwrap(),
    )
    .unwrap();
    let census_text = |letter: &str| {
        format!(
            "parse: refused: no --config given, no SPECENGINE_CENSUS_CONFIG_{letter} and no census.toml at the corpus root"
        )
    };
    let scheme_text = |letter: &str| {
        format!(
            "parse: refused: no --scheme given, no SPECENGINE_SCHEME_{letter} and no specengine.toml at the corpus root"
        )
    };
    let old_census = "parse: refused: no --config given and no census.toml at the corpus root";
    let old_scheme = "parse: refused: no --scheme given and no specengine.toml at the corpus root";
    // (label, variable, --config, expected stderr)
    let cases = [
        (Some("pilot-a"), None, false, census_text("A")),
        (Some("pilot-b"), None, false, census_text("B")),
        (
            Some("pilot-b"),
            Some(("SPECENGINE_SCHEME_B", config.as_path())),
            false,
            census_text("B"),
        ),
        (Some("other"), None, false, old_census.to_owned()),
        (None, None, false, old_census.to_owned()),
        (Some("pilot-a"), None, true, scheme_text("A")),
        (
            Some("pilot-b"),
            Some(("SPECENGINE_CENSUS_CONFIG_B", config.as_path())),
            false,
            scheme_text("B"),
        ),
        (
            Some("pilot-a"),
            Some(("SPECENGINE_SCHEME_B", config.as_path())),
            true,
            scheme_text("A"),
        ),
        (Some("other"), None, true, old_scheme.to_owned()),
        (None, None, true, old_scheme.to_owned()),
    ];
    for (index, (label, variable, flag, expected)) in cases.into_iter().enumerate() {
        let context = format!("{label:?}, {variable:?}, --config {flag}");
        let out = scratch.join(&format!("out-{index}"));
        let mut command = eval();
        if let Some((variable, value)) = variable {
            command.env(variable, value);
        }
        command.args([
            "parse",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        if let Some(label) = label {
            command.args(["--label", label]);
        }
        if flag {
            command.args(["--config", config.to_str().unwrap()]);
        }
        let output = command.output().expect("specengine-eval runs");
        let message = assert_refused(&output, &out, &context);
        assert_eq!(message.trim_end(), expected, "{context}");
    }
    assert_eq!(snapshot(&corpus), before, "the corpus is untouched");
}

// ------------------------------------------ docs/features/phase1-cleanup.md

/// A scratch corpus: corpus-mini's census convention with IDs of `digits`
/// digits, the scheme `ZR`, `ZN` of that width, and `files` under `design/`.
fn scratch_corpus(scratch: &Scratch, digits: usize, files: &[(&str, &str)]) -> PathBuf {
    let corpus = scratch.join("corpus");
    fs::create_dir_all(corpus.join("design")).unwrap();
    let census = fs::read_to_string(corpus_mini().join("census.toml")).unwrap();
    assert!(census.contains("[0-9]{3}"), "corpus-mini census regex");
    fs::write(
        corpus.join("census.toml"),
        census.replace("[0-9]{3}", &format!("[0-9]{{{digits}}}")),
    )
    .unwrap();
    fs::write(
        corpus.join("specengine.toml"),
        format!(
            "[ids]\nZR = {{ kind = \"rule\", width = {digits} }}\nZN = {{ kind = \"note\", width = {digits} }}\n"
        ),
    )
    .unwrap();
    for (relative, text) in files {
        fs::write(corpus.join("design").join(relative), text).unwrap();
    }
    corpus
}

fn parse_scratch(corpus: &Path, out: &Path, timeout: &str) -> Value {
    let output = eval()
        .args([
            "parse",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            "scratch",
            "--out",
            out.to_str().unwrap(),
            "--timeout",
            timeout,
        ])
        .output()
        .expect("specengine-eval runs");
    envelope(&output)
}

/// AC-17 (E1): one file of 50 000 ID sections is measured within the
/// budget (heading lines by one line index per file), and the census
/// agrees on every section.
#[test]
fn fifty_thousand_sections_in_one_file_are_measured_within_the_budget() {
    const SECTIONS: usize = 50_000;
    let scratch = Scratch::new("big");
    let mut text = String::from("---\nkind: rule\n---\n\n# Rules\n\n");
    for n in 1..=SECTIONS {
        text.push_str(&format!(
            "## Rule {n} {{#ZR-{n:05}}}\n\nThe text of rule {n}, a sentence of ordinary length.\n\n"
        ));
    }
    let corpus = scratch_corpus(&scratch, 5, &[("big.md", &text)]);
    let started = std::time::Instant::now();
    let envelope = parse_scratch(&corpus, &scratch.join("out"), "60");
    let result = &envelope["result"];
    assert!(
        result.is_object(),
        "result is {result} after {:?}",
        started.elapsed()
    );
    eprintln!(
        "parse of {} bytes, {SECTIONS} sections: wall {} ms (run {:?})",
        text.len(),
        envelope["wall_ms"],
        started.elapsed()
    );
    assert_eq!(result["files"], 1);
    assert_eq!(result["sections"]["parsed"], SECTIONS);
    assert_eq!(result["sections"]["census_id_sections"], SECTIONS);
    assert_eq!(result["sections"]["differ"], 0);
}

/// AC-18 (E3): a look-alike only in `id:` is a `homoglyph` diagnostic of
/// a definition, not a look-alike reference.
#[test]
fn a_look_alike_definition_is_a_diagnostic_but_no_reference() {
    let scratch = Scratch::new("lookalike-id");
    // `ZR-005` with a Greek capital Zeta.
    let corpus = scratch_corpus(
        &scratch,
        3,
        &[(
            "defined.md",
            "---\nid: \u{0396}R-005\nkind: rule\n---\n\n# Defined\n\nPlain text, no reference.\n",
        )],
    );
    let envelope = parse_scratch(&corpus, &scratch.join("out"), "60");
    let result = &envelope["result"];
    assert_eq!(result["diagnostics"]["homoglyph"], 1, "{result}");
    assert_eq!(result["references"]["homoglyph"], 0, "{result}");
    assert_eq!(result["references"]["inline"], 0, "{result}");
    assert_eq!(result["references"]["declared"], 0, "{result}");
}

/// AC-19 (E4): a document that cannot be read is counted `unreadable`;
/// `files` still counts every document walked.
#[test]
fn an_unreadable_document_is_counted_and_still_walked() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("unreadable");
    let corpus = scratch.join("corpus");
    copy_dir(&corpus_mini(), &corpus, &[]);
    let locked = corpus.join("design").join("notes.md");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&locked).is_ok() {
        // Running as root: mode 000 does not stop reads, nothing to measure.
        eprintln!("mode 000 is readable here (root?); skipped");
        return;
    }
    let envelope = parse_scratch(&corpus, &scratch.join("out"), "60");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    let result = &envelope["result"];
    assert_eq!(result["files"], 3, "{result}");
    assert_eq!(result["unreadable"], 1, "{result}");
    assert_eq!(result["panics"], 0, "{result}");
}
