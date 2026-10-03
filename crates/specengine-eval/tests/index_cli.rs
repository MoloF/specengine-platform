//! `specengine-eval index` end to end (docs/features/spec-index.md AC-21)
//! on the real binary over `fixtures/spec-b` (and a copy of spec-a through
//! `--scheme`): one anonymous envelope of counts and times, with
//! `one_file_parsed` 1 and `noop_parsed` 0, the update reports only under
//! `--out`, the fixtures untouched; `--out` under the corpus, an unknown
//! `[paths]` key (`file:line: message`), a `..` root or a wrong type is
//! refused with exit 2 and nothing under `--out`.
//!
//! Pilot runs are `#[ignore]` and owner-run: the corpus comes from
//! `SPECENGINE_PILOT_A` / `_B`, the scheme from `SPECENGINE_SCHEME_A` / `_B`
//! (both outside the repository, read-only). Their helper
//! (docs/features/pilot-schemes.md) fails, never skips: a scheme without a
//! `[paths]` table fails naming its variable; the run must leave the
//! read-only proof equal and the scratch `HOME` empty, find every root and
//! meet 08 AC-10: `full_ms` ≤ 10 000, `one_file_ms` ≤ 200. Non-ignored
//! tests run the same helper on an invented setup.

#![cfg(unix)]

mod pilot;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

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

fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-index-{name}-{}",
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
            "--no-optional-locks",
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

fn md_count(dir: &Path) -> usize {
    snapshot(dir)
        .keys()
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .count()
}

const RESULT_KEYS: [&str; 12] = [
    "diagnostics",
    "files",
    "full_ms",
    "links",
    "missing_roots",
    "nodes",
    "noop_ms",
    "noop_parsed",
    "one_file_ms",
    "one_file_parsed",
    "one_path_ms",
    "unreadable",
];

/// The fixed keys; every result value a count; no path, ID or corpus name
/// on stdout.
fn assert_anonymous(output: &Output, envelope: &Value, label: &str, corpus: &Path) {
    let object = envelope.as_object().expect("envelope object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["label", "measurement", "result", "versions", "wall_ms"]
            .into_iter()
            .collect()
    );
    assert_eq!(envelope["measurement"], "index");
    assert_eq!(envelope["label"], label);
    let result = envelope["result"].as_object().expect("result object");
    assert_eq!(
        result.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        RESULT_KEYS.into_iter().collect(),
        "result keys"
    );
    for (key, value) in result {
        assert!(value.is_u64(), "result.{key} is a count: {value}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for leak in [
        ".md", "/", "\\", "docs", "spec", "REQ-", "MOD-", "CMD-", "FLAG-", "RULE-", "MEC-",
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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
}

// ------------------------------------------------------------------ AC-21

#[test]
fn spec_b_indexes_into_one_anonymous_envelope() {
    let status_before = fixtures_git_status();
    let fixture_before = snapshot(&fixture("spec-b"));
    let scratch = Scratch::new("spec-b");
    let out = scratch.join("out");
    let output = eval()
        .args(["index", "--out", out.to_str().unwrap()])
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output);
    assert_anonymous(&output, &envelope, "fixtures", &fixture("spec-b"));
    let result = &envelope["result"];
    let files = md_count(&fixture("spec-b").join("docs"));
    assert_eq!(result["files"], files, "every spec-b file");
    assert_eq!(result["one_file_parsed"], 1);
    assert_eq!(result["noop_parsed"], 0);
    assert_eq!(result["unreadable"], 0);
    assert_eq!(result["missing_roots"], 0);
    assert!(result["nodes"].as_u64().unwrap() > files as u64);
    assert!(result["links"].as_u64().unwrap() > 0);

    // The detail lives under --out only.
    let detail = out.join("index").join("fixtures");
    let reports = read_json(&detail.join("reports.json"));
    for key in ["full", "noop", "one_file", "one_path"] {
        assert!(reports[key].is_object(), "reports.json has {key}");
    }
    assert_eq!(reports["full"]["parsed"], files);
    assert_eq!(reports["noop"]["parsed"], 0);
    assert_eq!(reports["one_file"]["parsed"], 1);
    assert_eq!(reports["one_path"]["parsed"], 1);
    assert!(
        detail.join("index.db").is_file(),
        "the scratch DB under --out"
    );
    assert_eq!(
        md_count(&detail.join("corpus")),
        files,
        "the copy under --out"
    );

    // The same run twice gives the same counts.
    let again = envelope_of(eval().args(["index", "--out", out.to_str().unwrap()]));
    for key in [
        "files",
        "nodes",
        "links",
        "diagnostics",
        "one_file_parsed",
        "noop_parsed",
    ] {
        assert_eq!(again["result"][key], result[key], "{key} on a repeat run");
    }

    assert_eq!(
        snapshot(&fixture("spec-b")),
        fixture_before,
        "spec-b is untouched"
    );
    assert_eq!(
        fixtures_git_status(),
        status_before,
        "git status -- fixtures/"
    );
}

fn envelope_of(command: &mut Command) -> Value {
    envelope(&command.output().expect("specengine-eval runs"))
}

#[test]
fn an_explicit_scheme_gives_ids_and_paths() {
    let scratch = Scratch::new("scheme");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-a"), &corpus);
    let scheme = scratch.join("scheme.toml");
    fs::rename(corpus.join("specengine.toml"), &scheme).unwrap();
    let out = scratch.join("out");
    let envelope = envelope_of(eval().args([
        "index",
        "--pilot",
        corpus.to_str().unwrap(),
        "--scheme",
        scheme.to_str().unwrap(),
        "--label",
        "spec-a-copy",
        "--out",
        out.to_str().unwrap(),
    ]));
    let docs = corpus.join("docs");
    let walked = md_count(&docs.join("spec"))
        + md_count(&docs.join("records"))
        + md_count(&docs.join("features"));
    assert_eq!(
        envelope["result"]["files"], walked,
        "spec-a by the defaults"
    );
    assert_eq!(
        envelope["result"]["missing_roots"], 1,
        "spec-a has no docs/archive"
    );
    assert_eq!(envelope["result"]["one_file_parsed"], 1);
}

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
fn out_under_the_corpus_is_refused() {
    let scratch = Scratch::new("out-under");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-b"), &corpus);
    let before = snapshot(&corpus);
    for out in [corpus.join("out"), corpus.join("docs/out")] {
        let output = eval()
            .args([
                "index",
                "--pilot",
                corpus.to_str().unwrap(),
                "--label",
                "under",
                "--out",
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        let message = assert_refused(&output, &out, "--out under the corpus");
        assert!(message.contains("--out"), "stderr names --out:\n{message}");
        assert_eq!(snapshot(&corpus), before, "the corpus is untouched");
    }
}

#[test]
fn a_bad_paths_table_is_refused_with_file_line_message() {
    let scratch = Scratch::new("bad-paths");
    let cases = [
        (
            "[paths]\nroots = [\"docs\"]\nsurprise = true\n",
            Some(3),
            "an unknown key",
        ),
        (
            "[paths]\nroots = [\"docs\", \"../elsewhere\"]\n",
            Some(2),
            "a .. root",
        ),
        (
            "[paths]\nroots = [\"/docs\"]\n",
            Some(2),
            "an absolute root",
        ),
        ("[paths]\n\nroots = \"docs\"\n", Some(3), "a wrong type"),
    ];
    for (index, (paths, line, context)) in cases.into_iter().enumerate() {
        let corpus = scratch.join(&format!("corpus-{index}"));
        copy_dir(&fixture("spec-b"), &corpus);
        let scheme = corpus.join("specengine.toml");
        let ids = fs::read_to_string(&scheme).unwrap();
        let ids = &ids[ids.find("\n[ids]\n").expect("an [ids] table") + 1..];
        fs::write(&scheme, format!("{paths}\n{ids}")).unwrap();
        let before = snapshot(&corpus);
        let out = scratch.join(&format!("out-{index}"));
        let output = eval()
            .args([
                "index",
                "--pilot",
                corpus.to_str().unwrap(),
                "--label",
                "bad",
                "--out",
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        let message = assert_refused(&output, &out, context);
        if let Some(line) = line {
            let located = format!("{}:{line}: ", scheme.display());
            assert!(
                message.contains(&located),
                "{context}: stderr names {located:?}:\n{message}"
            );
        }
        assert_eq!(
            snapshot(&corpus),
            before,
            "{context}: the corpus is untouched"
        );
    }
}

// ---------------------------------------------------------------- pilots

/// One `index` run over `inputs`
/// (`crates/specengine-eval/README.md` "Pilot runs and tests"):
/// `pilot::run_read_only` (the `[paths]` requirement, the proof, the child
/// environment, exit 0, `HOME` empty), the anonymous envelope, every root
/// found, one file parsed by the one-file update, 08 AC-10's budgets.
fn pilot_run(label: &str, inputs: &pilot::Inputs, scratch: &Path) -> Value {
    let output = pilot::run_read_only("index", label, scratch, inputs);
    let envelope = envelope(&output);
    assert_anonymous(&output, &envelope, label, inputs.corpus);
    let result = &envelope["result"];
    eprintln!("{label} result: {result}");
    assert_eq!(
        result["missing_roots"], 0,
        "every scheme root exists: {result}"
    );
    assert_eq!(result["one_file_parsed"], 1, "{result}");
    assert!(
        result["full_ms"].as_u64().is_some_and(|ms| ms <= 10_000),
        "08 AC-10: full_ms {} > 10 000",
        result["full_ms"]
    );
    assert!(
        result["one_file_ms"].as_u64().is_some_and(|ms| ms <= 200),
        "08 AC-10: one_file_ms {} > 200",
        result["one_file_ms"]
    );
    envelope
}

/// The `#[ignore]` pilot test: the inputs from the label's variables (the
/// corpus and the scheme required, the census config passed on when set).
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
    let config = pilot::optional(variables.config);
    let scratch = Scratch::new(label);
    let inputs = pilot::Inputs {
        corpus: &corpus,
        scheme: &scheme,
        config: config.as_deref(),
    };
    pilot_run(label, &inputs, &scratch.0);
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A and SPECENGINE_SCHEME_A (with [paths]); read-only, owner-run"]
fn pilot_a_index_meets_the_budgets() {
    pilot_from_environment("pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B and SPECENGINE_SCHEME_B (with [paths]); read-only, owner-run"]
fn pilot_b_index_meets_the_budgets() {
    pilot_from_environment("pilot-b");
}

/// docs/features/pilot-schemes.md AC-01, AC-02: the pilot helper on the
/// invented setup (a spec-b copy, `git init`, a scheme with every recipe
/// table, the three variables on the child) is green; the detail lands
/// only under `--out/index/pilot-a`.
#[test]
fn the_pilot_helper_indexes_the_invented_setup_read_only() {
    let scratch = Scratch::new("pilot-setup");
    let setup = pilot::invented_setup(&scratch.0, true);
    let envelope = pilot_run("pilot-a", &setup.inputs(), &scratch.0);
    let result = &envelope["result"];
    assert_eq!(
        result["files"],
        pilot::walked_files(&setup),
        "the scheme's roots, the template excluded: {result}"
    );
    assert_eq!(result["unreadable"], 0, "{result}");
    let out = scratch.join("out");
    let names = |dir: &Path| {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(names(&out), BTreeSet::from(["index".to_owned()]));
    assert_eq!(
        names(&out.join("index")),
        BTreeSet::from(["pilot-a".to_owned()])
    );
    assert_eq!(
        names(&out.join("index").join("pilot-a")),
        ["corpus", "index.db", "reports.json"]
            .map(str::to_owned)
            .into_iter()
            .collect::<BTreeSet<_>>()
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

/// docs/features/pilot-schemes.md AC-03: two proofs of an untouched corpus
/// are equal; an mtime moved by one second on a file under a directory root,
/// or on a single-file root, makes them unequal (the listing sees it, git
/// status does not).
#[test]
fn the_proof_sees_an_mtime_moved_under_a_root() {
    let scratch = Scratch::new("proof-mtime");
    let setup = pilot::invented_setup(&scratch.0, true);
    let roots = pilot::scheme_roots(&setup.scheme, "SPECENGINE_SCHEME_A");
    assert_eq!(roots, ["AGENTS.md", "docs", "crates/engine/README.md"]);
    for file in ["docs/records/REQ/REQ-001.md", "AGENTS.md"] {
        let before = pilot::proof(&setup.corpus, &roots);
        assert_eq!(
            pilot::proof(&setup.corpus, &roots),
            before,
            "nothing changed: the proofs are equal"
        );
        let path = setup.corpus.join(file);
        let moved = fs::metadata(&path).unwrap().modified().unwrap() + Duration::from_secs(1);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(moved)
            .unwrap();
        let after = pilot::proof(&setup.corpus, &roots);
        assert_eq!(
            after.status, before.status,
            "{file}: git status sees no mtime"
        );
        assert_ne!(after, before, "{file}: the moved mtime is seen");
    }
}

/// docs/features/pilot-schemes.md AC-03: a file created at the corpus root,
/// outside every scheme root, makes the proofs unequal (git status sees it,
/// the listing cannot).
#[test]
fn the_proof_sees_a_file_created_outside_every_root() {
    let scratch = Scratch::new("proof-stray");
    let setup = pilot::invented_setup(&scratch.0, true);
    let roots = pilot::scheme_roots(&setup.scheme, "SPECENGINE_SCHEME_A");
    let before = pilot::proof(&setup.corpus, &roots);
    fs::write(setup.corpus.join("pilot-a.db"), b"a stray database").unwrap();
    let after = pilot::proof(&setup.corpus, &roots);
    assert_eq!(after.entries, before.entries, "outside every root");
    assert_ne!(after, before, "the stray file is seen");
}

/// `crates/specengine-eval/README.md` "Pilot runs and tests": no git
/// worktree, no proof; the helper fails.
#[test]
#[should_panic(expected = "must be a git worktree")]
fn the_proof_needs_a_git_worktree() {
    let scratch = Scratch::new("proof-no-git");
    let setup = pilot::invented_setup(&scratch.0, true);
    fs::remove_dir_all(setup.corpus.join(".git")).unwrap();
    let roots = pilot::scheme_roots(&setup.scheme, "SPECENGINE_SCHEME_A");
    pilot::proof(&setup.corpus, &roots);
}

// ------------------------------------------ docs/features/phase1-cleanup.md

/// AC-20 (E5): a corpus `.md` that cannot be read is left out of the copy
/// and counted `unreadable`.
#[test]
fn an_unreadable_corpus_file_is_counted_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("unreadable");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-b"), &corpus);
    let files = md_count(&corpus.join("docs"));
    let locked = snapshot(&corpus.join("docs"))
        .into_keys()
        .find(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| corpus.join("docs").join(path))
        .expect("a spec-b document");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&locked).is_ok() {
        eprintln!("mode 000 is readable here (root?); skipped");
        return;
    }
    let out = scratch.join("out");
    let output = eval()
        .args([
            "index",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            "locked",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("specengine-eval runs");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    let envelope = envelope(&output);
    let result = &envelope["result"];
    assert_eq!(result["unreadable"], 1, "{result}");
    assert_eq!(result["files"], files - 1, "the rest is indexed: {result}");
}

/// AC-21 (E6): `index` takes its configuration from `--scheme` only; a
/// `--config` is refused (exit 2) before anything is written.
/// docs/features/pilot-schemes.md: with no scheme anywhere a pilot label's
/// refusal names the variable it looked for (unset, empty or only the other
/// label's set); any other label (or none) keeps the old text.
#[test]
fn pilot_label_refusals_name_the_scheme_variable() {
    let scratch = Scratch::new("refusal-names");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-b"), &corpus);
    let scheme = scratch.join("scheme.toml");
    fs::rename(corpus.join("specengine.toml"), &scheme).unwrap();
    let before = snapshot(&corpus);
    let pilot_text = |letter: &str| {
        format!(
            "index: refused: no --scheme given, no SPECENGINE_SCHEME_{letter} and no specengine.toml at the corpus root"
        )
    };
    let old_text = "index: refused: no --scheme given and no specengine.toml at the corpus root";
    let empty = PathBuf::new();
    let cases = [
        (Some("pilot-a"), None, pilot_text("A")),
        (Some("pilot-b"), None, pilot_text("B")),
        (
            Some("pilot-a"),
            Some(("SPECENGINE_SCHEME_A", empty.as_path())),
            pilot_text("A"),
        ),
        (
            Some("pilot-a"),
            Some(("SPECENGINE_SCHEME_B", scheme.as_path())),
            pilot_text("A"),
        ),
        (
            Some("pilot-b"),
            Some(("SPECENGINE_SCHEME_A", scheme.as_path())),
            pilot_text("B"),
        ),
        (
            Some("other"),
            Some(("SPECENGINE_SCHEME_A", scheme.as_path())),
            old_text.to_owned(),
        ),
        (
            Some("pilot"),
            Some(("SPECENGINE_SCHEME_A", scheme.as_path())),
            old_text.to_owned(),
        ),
        (
            None,
            Some(("SPECENGINE_SCHEME_A", scheme.as_path())),
            old_text.to_owned(),
        ),
    ];
    for (index, (label, variable, expected)) in cases.into_iter().enumerate() {
        let context = format!("{label:?} with {variable:?}");
        let out = scratch.join(&format!("out-{index}"));
        let mut command = eval();
        if let Some((variable, value)) = variable {
            command.env(variable, value);
        }
        command.args([
            "index",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        if let Some(label) = label {
            command.args(["--label", label]);
        }
        let output = command.output().expect("specengine-eval runs");
        let message = assert_refused(&output, &out, &context);
        assert_eq!(message.trim_end(), expected, "{context}");
    }
    assert_eq!(snapshot(&corpus), before, "the corpus is untouched");
}

#[test]
fn index_refuses_config() {
    let scratch = Scratch::new("config");
    let corpus = scratch.join("corpus");
    copy_dir(&fixture("spec-b"), &corpus);
    let before = snapshot(&corpus);
    let config = scratch.join("census.toml");
    fs::write(&config, "[corpus]\nroots = [\"docs\"]\n").unwrap();
    for pilot in [None, Some(&corpus)] {
        let out = scratch.join("out");
        let mut command = eval();
        command.arg("index");
        if let Some(pilot) = pilot {
            command.args(["--pilot", pilot.to_str().unwrap(), "--label", "copy"]);
        }
        let output = command
            .args([
                "--config",
                config.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
            ])
            .output()
            .expect("specengine-eval runs");
        let message = assert_refused(&output, &out, "--config");
        assert!(
            message.contains("--config"),
            "stderr names --config:\n{message}"
        );
        assert_eq!(
            message.trim_end(),
            format!(
                "index: refused: --config {} is not read by `index` (its configuration is --scheme); nothing written",
                config.display()
            )
        );
    }
    assert_eq!(snapshot(&corpus), before, "the corpus is untouched");
}
