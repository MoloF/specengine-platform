//! docs/features/pilot-w.md AC-01…AC-08 through the real
//! `specengine-eval w` binary, on the two invented conventions
//! `fixtures/pilot-w/one` (a requirements notebook: a Tier 0, an index root
//! with two shards, a Tier 1 key table, record files, a generated document
//! inside the task globs) and `fixtures/pilot-w/two` (a decision log: no
//! index, no Tier 0, the default Tier 1 only, a page moved by its slug, a
//! decision defined in two documents, an even task count), on test-time
//! copies of them (non-Latin text built from `\u{...}` escapes here, never
//! stored in a fixture), and through the CLI library the measurement calls
//! (`specengine_cli::bundle`, `show`, `render_text`), run here with the
//! test's own `Env` on the tree `w` wrote. AC-09's genre and dependency
//! checks are in `import_genre.rs` and `build_graph.rs`; AC-10's owner-run
//! pilot tests are the `#[ignore]` ones of `mod pilots`.
//!
//! Nothing here names a pilot; every run writes only under a scratch `--out`.

mod import_support;
#[cfg(unix)]
mod pilot;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use import_support::*;
use serde_json::{Value, json};
use specengine_cli::{
    Bundle, BundleOutcome, BundleRequest, CliError, Env, Exit, Globals, ItemForm, Outcome,
    ShowRequest, render_text,
};
use specengine_core::check::BundleLayer;
use specengine_model::{DiagnosticCode, IdScheme};

/// The two conventions, relative to `fixtures/`.
const ONE: &str = "pilot-w/one";
const TWO: &str = "pilot-w/two";
const W_FIXTURES: [&str; 2] = [ONE, TWO];

/// The run date of every test run (both `debt_expires` lie far after it).
const TODAY: &str = "2026-10-04";

/// The budget without `--budget` (07 §5 `bundle_task`).
const DEFAULT_BUDGET: u32 = 10_000;
/// `one` at this budget: outlined and header-only targets, a not-included
/// list, exactly one task with more than three follow-ups (the needs, with
/// its moved records), none refused.
const OUTLINE_BUDGET: u32 = 600;
/// `one` at this budget: exactly one task, the one with the most targets
/// (the needs, with its moved records), below its bundle's minimum.
const REFUSING_BUDGET: u32 = 300;

/// The task sources of each fixture in task order (source-path byte order:
/// uppercase before lowercase, `/` before letters).
const ONE_TASKS: [&str; 7] = [
    "book/bare.md",
    "book/broken.md",
    "book/checkout.md",
    "book/flows/login.md",
    "book/flows/payment.md",
    "book/needs.md",
    "book/plan.md",
];
const TWO_TASKS: [&str; 6] = [
    "log/DEC-0040.md",
    "log/OLD-0041.md",
    "log/decisions.md",
    "pages/Bulk_Export/index.md",
    "pages/billing/index.md",
    "pages/overview.md",
];

/// The `result` keys of `docs/canon/w-measurement.md` "CLI", leaf by leaf.
const WHITELIST: [&str; 46] = [
    "tasks",
    "budget/tokens",
    "budget/source",
    "w_before/median",
    "w_before/p90",
    "w_before/max",
    "w_after/median",
    "w_after/p90",
    "w_after/max",
    "w_after_followups/median",
    "w_after_followups/p90",
    "w_after_followups/max",
    "docs_needed/median",
    "docs_needed/p90",
    "docs_needed/max",
    "third_step",
    "worst/w_before",
    "worst/w_after",
    "worst/w_after_followups",
    "slots/tier0",
    "slots/index",
    "slots/shard",
    "slots/tier1/key",
    "slots/tier1/default",
    "slots/tier1/none",
    "slots/unmapped",
    "targets/refs",
    "targets/text",
    "targets/outline",
    "targets/header",
    "targets/not_included",
    "citations/unresolved",
    "citations/unchecked",
    "citations/wiki_links",
    "followups/refs",
    "followups/bytes",
    "followups/truncated",
    "followups/incomplete",
    "refused",
    "failed",
    "nondeterministic",
    "bytes_per_token",
    "fill_percent/median",
    "fill_percent/max",
    "bundle_ms/median",
    "bundle_ms/p90",
];

// ------------------------------------------------------------------ plumbing

/// One `w` run: its stdout envelope and its detail directory.
struct WRun {
    output: Output,
    envelope: Value,
    detail: PathBuf,
    corpus: PathBuf,
}

impl WRun {
    /// `w --pilot <corpus> --out <scratch>/<name> --today TODAY <extra>`.
    fn new(corpus: &Path, scratch: &Scratch, name: &str, extra: &[&str]) -> Self {
        let out = scratch.join(name);
        let mut args = vec![
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ];
        args.extend_from_slice(extra);
        let output = run(&args);
        let envelope = envelope(&output);
        assert_eq!(envelope["measurement"], "w");
        assert_eq!(envelope["label"], "pilot");
        Self {
            output,
            envelope,
            detail: out.join("w").join("pilot"),
            corpus: corpus.to_path_buf(),
        }
    }

    fn of(name: &str, scratch: &Scratch, extra: &[&str]) -> Self {
        let dir = format!("{}{}", name, extra.join("-")).replace('/', "-");
        Self::new(&fixture_dir(name), scratch, &dir, extra)
    }

    fn at_budget(name: &str, scratch: &Scratch, budget: u32) -> Self {
        Self::of(name, scratch, &["--budget", &budget.to_string()])
    }

    fn result(&self) -> &Value {
        &self.envelope["result"]
    }

    fn budget(&self) -> u32 {
        u32::try_from(self.result()["budget"]["tokens"].as_u64().unwrap()).unwrap()
    }

    fn tree(&self) -> PathBuf {
        self.detail.join("tree")
    }

    fn tasks(&self) -> Vec<Value> {
        read_json(&self.detail.join("tasks.json"))
            .as_array()
            .expect("tasks.json is an array")
            .clone()
    }

    fn task(&self, source: &str) -> Value {
        self.tasks()
            .into_iter()
            .find(|task| task["source"] == source)
            .unwrap_or_else(|| panic!("no task of {source}"))
    }

    fn body(&self, task: &Value) -> String {
        let path = self
            .detail
            .join("bundles")
            .join(format!("{}.txt", task["task"].as_str().unwrap()));
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    fn stdout(&self) -> String {
        String::from_utf8(self.output.stdout.clone()).expect("stdout is UTF-8")
    }
}

fn expected(name: &str) -> Value {
    read_json(&fixture_dir(name).join("expected.json"))
}

/// A scratch copy of a fixture.
fn copy_of(name: &str, scratch: &Scratch, as_name: &str) -> PathBuf {
    let corpus = scratch.join(as_name);
    copy_dir(&fixture_dir(name), &corpus);
    corpus
}

/// `path`'s text with `from` replaced once; `from` must be there.
fn edit(path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(path).expect("readable");
    assert!(text.contains(from), "{from:?} not in {}", path.display());
    fs::write(path, text.replacen(from, to, 1)).expect("writable");
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

/// The count at a `/`-separated path of `value`.
fn count(value: &Value, path: &str) -> u64 {
    let mut at = value;
    for part in path.split('/') {
        at = &at[part];
    }
    at.as_u64()
        .unwrap_or_else(|| panic!("{path} is not a count: {at}"))
}

/// The size of `root/path`; `None` when there is no such file.
fn file_size(root: &Path, path: &str) -> Option<u64> {
    fs::metadata(root.join(path))
        .ok()
        .filter(fs::Metadata::is_file)
        .map(|meta| meta.len())
}

/// The corpus bytes of a slot or source path; 0 when it is missing.
fn corpus_bytes(corpus: &Path, path: &str) -> u64 {
    file_size(corpus, path).unwrap_or(0)
}

/// A task's slot `slot` on `side`: its path and bytes as `tasks.json`
/// records them.
fn slot(task: &Value, slot: &str, side: &str) -> Option<(String, u64)> {
    task["slots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["slot"] == slot && entry["side"] == side)
        .map(|entry| {
            (
                entry["path"].as_str().unwrap().to_owned(),
                entry["bytes"].as_u64().unwrap(),
            )
        })
}

/// Every source document a task's targets name, distinct.
fn target_sources(task: &Value) -> BTreeSet<String> {
    task["targets"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|target| strings(&target["sources"]))
        .collect()
}

fn leaves(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}/{key}")
                };
                leaves(value, &path, out);
            }
        }
        other => out.push((prefix.to_owned(), other.clone())),
    }
}

/// Exit 2, nothing on stdout, `--out` not created, no panic.
fn assert_refused(output: &Output, out: &Path, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: {}",
        stderr(output)
    );
    assert!(output.stdout.is_empty(), "{context}: stdout");
    assert!(!out.exists(), "{context}: --out created");
    assert!(!stderr(output).contains("panicked"), "{context}");
}

// ------------------------------------------------- the CLI library, own Env

/// The test's own `Env` on `tree`: the tree as the current directory, a
/// scratch home of its own, no `XDG_DATA_HOME`.
fn library_env(tree: &Path, home: &Path) -> Env {
    fs::create_dir_all(home).expect("the test's library home");
    Env {
        cwd: tree.to_path_buf(),
        home: Some(OsString::from(home.as_os_str())),
        xdg_data_home: None,
    }
}

fn library_globals(tree: &Path) -> Globals {
    Globals {
        root: Some(tree.to_path_buf()),
        config: None,
    }
}

fn library_bundle(
    tree: &Path,
    home: &Path,
    references: Vec<String>,
    budget: u32,
) -> Result<BundleOutcome, CliError> {
    specengine_cli::bundle(
        &library_env(tree, home),
        &library_globals(tree),
        &BundleRequest {
            references,
            budget: Some(i64::from(budget)),
        },
    )
}

/// The library bundle of a task `w` bundled; panics on anything else.
fn library_bundle_of(tree: &Path, home: &Path, task: &Value, budget: u32) -> Bundle {
    match library_bundle(tree, home, strings(&task["refs"]), budget) {
        Ok(BundleOutcome {
            bundle: Some(bundle),
            ..
        }) => bundle,
        Ok(outcome) => panic!("{}: no bundle: {:?}", task["task"], outcome.reason),
        Err(error) => panic!("{}: {}", task["task"], error.message),
    }
}

/// `spec show REF` as `spec show` prints it: its bytes and whether its cap
/// cut it.
fn library_show(tree: &Path, home: &Path, reference: &str) -> (usize, bool) {
    let outcome = specengine_cli::show(
        &library_env(tree, home),
        &library_globals(tree),
        &ShowRequest {
            reference: reference.to_owned(),
            links: false,
            archive: false,
        },
    )
    .unwrap_or_else(|error| panic!("spec show {reference}: {}", error.message));
    assert!(
        outcome.reason.is_none(),
        "spec show {reference}: {:?}",
        outcome.reason
    );
    let text = render_text(&Outcome::Show(outcome));
    let truncated = text.lines().any(|line| line.starts_with("[truncated: "));
    (text.len(), truncated)
}

/// The targets layer of a bundle: (name, path, line, form) per item.
fn targets_layer(bundle: &Bundle) -> Vec<(String, String, usize, ItemForm)> {
    bundle
        .layers
        .iter()
        .filter(|(layer, _)| *layer == BundleLayer::Targets)
        .flat_map(|(_, items)| items)
        .map(|item| (item.name.clone(), item.path.clone(), item.line, item.form))
        .collect()
}

fn form_word(form: ItemForm) -> &'static str {
    match form {
        ItemForm::Text => "text",
        ItemForm::Outline => "outline",
        ItemForm::Header => "header",
        ItemForm::Summary => "summary",
    }
}

/// Tier 0 + Tier 1 on the after side, computed here (A4): the tree's file
/// when the tree holds the path, else the corpus's.
fn after_slots(run: &WRun, task: &Value, tier0: Option<&str>) -> u64 {
    let tier1 = slot(task, "tier1", "before").map(|(path, _)| path);
    tier0
        .map(str::to_owned)
        .into_iter()
        .chain(tier1)
        .map(|path| {
            file_size(&run.tree(), &path).unwrap_or_else(|| corpus_bytes(&run.corpus, &path))
        })
        .sum()
}

/// The before scheme's `[paths] tier0` of a corpus.
fn tier0_of(corpus: &Path) -> Option<String> {
    let text = fs::read_to_string(corpus.join("specengine.toml")).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    table["paths"]
        .get("tier0")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
}

// ---------------------------------------------------------- the aggregates

/// The nearest-rank `p`-th percentile over every task
/// (`docs/canon/w-measurement.md` "Aggregates"): bytes ascending, a word
/// ("refused", "failed") above every number in task order; v[⌈p·n/100⌉]; with
/// the task it lands on.
fn nearest_rank(values: &[Value], p: usize) -> (Value, usize) {
    let mut ranked: Vec<(usize, &Value)> = values.iter().enumerate().collect();
    ranked.sort_by_key(|(task, value)| match value.as_u64() {
        Some(bytes) => (0u8, bytes, *task),
        None => (1, 0, *task),
    });
    let rank = (p * ranked.len()).div_ceil(100).max(1);
    let (task, value) = ranked[rank - 1];
    (value.clone(), task)
}

fn statistic(values: &[Value]) -> Value {
    json!({
        "median": nearest_rank(values, 50).0,
        "p90": nearest_rank(values, 90).0,
        "max": nearest_rank(values, 100).0,
    })
}

fn column(tasks: &[Value], key: &str) -> Vec<Value> {
    tasks.iter().map(|task| task[key].clone()).collect()
}

/// One decimal, half up, of `numerator / denominator`.
fn tenths(numerator: u64, denominator: u64) -> f64 {
    let tenths = (numerator * 20 + denominator) / (denominator * 2);
    tenths as f64 / 10.0
}

// --------------------------------------------------------------- AC-01

/// AC-01 base: `w` prints each fixture's `expected.json` (timings aside);
/// without `--pilot` it runs on `fixtures/pilot-w/one` under the label
/// `fixtures`; the detail sits under `<out>/w/<label>/`, the tree without
/// a debt baseline.
#[test]
fn w_on_each_fixture_matches_expected_json() {
    let scratch = Scratch::new("w-expected");
    let out = scratch.join("default");
    let envelope = envelope(&run(&[
        "w",
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]));
    assert_eq!(envelope["measurement"], "w");
    assert_eq!(envelope["label"], "fixtures", "the default label");
    assert_eq!(
        without_ms(&envelope["result"]),
        expected(ONE),
        "w without --pilot runs on fixtures/pilot-w/one"
    );
    assert!(out.join("w").join("fixtures").join("tasks.json").is_file());
    for name in W_FIXTURES {
        let run = WRun::of(name, &scratch, &[]);
        assert_eq!(
            without_ms(run.result()),
            expected(name),
            "{name}: result minus *_ms differs from expected.json"
        );
        assert_eq!(
            run.result()["budget"],
            json!({"tokens": DEFAULT_BUDGET, "source": "default"}),
            "{name}"
        );
        for dir in ["tree", "home-1", "home-2", "bundles"] {
            assert!(run.detail.join(dir).is_dir(), "{name}: {dir}/");
        }
        assert!(run.tree().join("specengine.toml").is_file(), "{name}");
        assert!(
            !run.tree().join(".spec-debt.toml").exists(),
            "{name}: w writes no baseline"
        );
        // The detail of every task, `docs/canon/w-measurement.md` "CLI".
        let keys = BTreeSet::from([
            "after",
            "bundle",
            "bundle_2_ms",
            "bundle_hash_2",
            "bundle_ms",
            "citations",
            "docs_needed",
            "followups",
            "message",
            "outcome",
            "refs",
            "show_ms",
            "slots",
            "source",
            "targets",
            "task",
            "w_after",
            "w_after_followups",
            "w_before",
        ]);
        for task in run.tasks() {
            let got: BTreeSet<&str> = task
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(got, keys, "{name}: {}", task["task"]);
            for target in task["targets"].as_array().unwrap() {
                assert!(target["sources"].is_array(), "{name}: {target}");
            }
            assert!(task["bundle"]["bundle_hash"].is_string(), "{name}: {task}");
            assert!(task["bundle"]["forms"].is_array(), "{name}: {task}");
        }
    }
}

/// Exit 0 measured, `"timeout"` on an overrun of `--timeout` (after
/// `prepare`).
#[test]
fn an_overrun_prints_timeout_and_exits_0() {
    let scratch = Scratch::new("w-timeout");
    let out = scratch.join("out");
    let output = run(&[
        "w",
        "--pilot",
        fixture_dir(ONE).to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--timeout",
        "0",
        "--today",
        TODAY,
    ]);
    let envelope = envelope(&output);
    assert_eq!(envelope["result"], "timeout", "{envelope}");
}

/// AC-01: every document the globs match is a task — a header core
/// rejects, a document without front-matter, a shipped one (bundled
/// ` | archived`) — but a `class: generated` one; an excluded or unmatched
/// document is not; `task-N` by source-path byte order. M1 (a liveness or
/// parsed-header filter) and M2 (the generated document a task) turn the
/// exact lists red.
#[test]
fn every_matching_document_but_a_generated_one_is_a_task_in_source_path_order() {
    let scratch = Scratch::new("w-tasks");
    let mut runs = Vec::new();
    for (name, sources) in [(ONE, &ONE_TASKS[..]), (TWO, &TWO_TASKS[..])] {
        let run = WRun::of(name, &scratch, &[]);
        let tasks = run.tasks();
        let got: Vec<&str> = tasks
            .iter()
            .map(|task| task["source"].as_str().unwrap())
            .collect();
        assert_eq!(got, sources, "{name}: the tasks");
        let mut sorted = got.clone();
        sorted.sort_unstable();
        assert_eq!(got, sorted, "{name}: source-path byte order");
        for (index, task) in tasks.iter().enumerate() {
            assert_eq!(task["task"], format!("task-{}", index + 1), "{name}");
        }
        assert_eq!(count(run.result(), "tasks"), tasks.len() as u64, "{name}");
        runs.push(run);
    }
    let run = &runs[0];

    // A header core rejects: the tree's file fails core's front-matter
    // parse, and the task is bundled all the same.
    let bytes = fs::read(run.tree().join("book/broken.md")).unwrap();
    let parsed = specengine_core::parse("book/broken.md", &bytes, &IdScheme::default());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::FrontmatterYaml),
        "core accepts the broken header: {:?}",
        parsed.diagnostics
    );
    assert_eq!(run.task("book/broken.md")["outcome"], "bundled");

    // No front-matter at all, in the corpus.
    let bare = fs::read_to_string(run.corpus.join("book/bare.md")).unwrap();
    assert!(!bare.starts_with("---"), "book/bare.md has front-matter");
    assert_eq!(run.task("book/bare.md")["outcome"], "bundled");

    // Shipped: a named Tier 3 target, bundled ` | archived`.
    let payment = run.task("book/flows/payment.md");
    let body = run.body(&payment);
    let header = body
        .lines()
        .find(|line| line.starts_with("book/flows/payment.md |"))
        .unwrap_or_else(|| panic!("no header of the shipped task:\n{body}"));
    assert!(header.contains("status shipped"), "{header}");
    assert!(header.ends_with(" | archived"), "{header}");

    // Not tasks: the generated document inside the include globs, the
    // excluded lexicon, a walked document no include glob matches.
    let generated = fs::read_to_string(run.tree().join("book/notes-generated.md")).unwrap();
    assert!(generated.contains("class: generated"), "{generated}");
    for path in [
        "book/notes-generated.md",
        "book/lexicon.md",
        "book/areas/README.md",
    ] {
        assert!(run.corpus.join(path).is_file(), "{path} in the fixture");
        assert!(
            run.tree().join(path).is_file(),
            "{path} is a walked document"
        );
        assert!(
            run.tasks().iter().all(|task| task["source"] != path),
            "{path} is a task"
        );
    }
}

/// The bad tasks configs of AC-01: (case, text, line of the error).
fn tasks_refusals() -> Vec<(&'static str, &'static str, usize)> {
    vec![
        (
            "an unknown key in [tasks]",
            "[tasks]\ninclude = [\"book/*.md\"]\nincludes = [\"book/flows/**\"]\n",
            3,
        ),
        (
            "an unknown table",
            "[tasks]\ninclude = [\"book/*.md\"]\n\n[tier2]\nkey = \"area\"\n",
            4,
        ),
        (
            "an unknown key in [tier1]",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\nkey = \"area\"\nfallback = \"engine/README.md\"\n",
            5,
        ),
        (
            "include of the wrong type",
            "[tasks]\ninclude = \"book/*.md\"\n",
            2,
        ),
        (
            "exclude of the wrong type",
            "\n\n[tasks]\ninclude = [\"book/*.md\"]\nexclude = \"book/lexicon.md\"\n",
            5,
        ),
        (
            "key of the wrong type",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\nkey = 3\n",
            4,
        ),
        ("an empty include", "[tasks]\ninclude = []\n", 2),
        ("no [tasks]", "[tier1]\nkey = \"area\"\n", 1),
        (
            "a TOML syntax error",
            "[tasks]\ninclude = [\"book/*.md\"\n",
            2,
        ),
        (
            "an include glob matching no walked source document",
            "[tasks]\ninclude = [\n  \"book/*.md\",\n  \"nowhere/**\",\n]\n",
            4,
        ),
        (
            "an include glob matching only code",
            "[tasks]\ninclude = [\"engine/*.rs\"]\n",
            2,
        ),
        (
            "an exclude glob matching no walked source document",
            "[tasks]\ninclude = [\"book/*.md\"]\nexclude = [\"book/nothing-here.md\"]\n",
            3,
        ),
        (
            "[tier1.paths] without key",
            "[tasks]\ninclude = [\"book/*.md\"]\n\n[tier1]\ndefault = \"book/areas/README.md\"\n\n[tier1.paths]\n\"input\" = \"engine/README.md\"\n",
            7,
        ),
        (
            "a default climbing out of the corpus",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\ndefault = \"../GUIDE.md\"\n",
            4,
        ),
        (
            "an absolute default",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\ndefault = \"/etc/hosts\"\n",
            4,
        ),
        (
            "a default with a dot component",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\ndefault = \"engine/./README.md\"\n",
            4,
        ),
        (
            "a default that is no corpus file",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\ndefault = \"engine/missing.md\"\n",
            4,
        ),
        (
            "a [tier1.paths] value that is a directory",
            "[tasks]\ninclude = [\"book/*.md\"]\n[tier1]\nkey = \"area\"\n[tier1.paths]\n\"input\" = \"engine/README.md\"\n\"store\" = \"engine\"\n",
            7,
        ),
    ]
}

/// AC-01: each refusal of the tasks config → exit 2 at `<tasks>:<line>`,
/// `--out` not created (M3: `deny_unknown_fields` off → the unknown keys
/// pass: red); a missing config → line 1; without `--tasks` the corpus
/// root's `tasks.toml`, its absence refused.
#[test]
fn each_tasks_config_refusal_names_its_line_and_writes_nothing() {
    let scratch = Scratch::new("w-tasks-refused");
    let corpus = fixture_dir(ONE);
    let mut cases: Vec<(String, PathBuf, usize)> = Vec::new();
    for (index, (case, text, line)) in tasks_refusals().into_iter().enumerate() {
        let config = scratch.join(&format!("tasks-{index}.toml"));
        fs::write(&config, text).unwrap();
        cases.push((case.to_owned(), config, line));
    }
    cases.push((
        "a missing tasks config".to_owned(),
        scratch.join("missing.toml"),
        1,
    ));
    for (index, (case, config, line)) in cases.iter().enumerate() {
        let out = scratch.join(&format!("out-{index}"));
        let output = run(&[
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--tasks",
            config.to_str().unwrap(),
            "--today",
            TODAY,
        ]);
        assert_refused(&output, &out, case);
        let at = format!("{}:{line}: ", config.display());
        assert!(
            stderr(&output).contains(&at),
            "{case}: no {at:?} in:\n{}",
            stderr(&output)
        );
    }

    // No --tasks: the corpus root's tasks.toml, else refused.
    let bare = copy_of(ONE, &scratch, "no-tasks");
    fs::remove_file(bare.join("tasks.toml")).unwrap();
    let out = scratch.join("out-no-tasks");
    let output = run(&[
        "w",
        "--pilot",
        bare.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &out, "no tasks.toml");
    assert!(
        stderr(&output).contains("tasks.toml"),
        "{}",
        stderr(&output)
    );
}

/// AC-01: a glob matching only the generated document is no refusal (it
/// matches a walked source document): zero tasks, every statistic 0, no
/// panic.
#[test]
fn a_task_set_of_only_generated_documents_is_zero_tasks() {
    let scratch = Scratch::new("w-zero-tasks");
    let config = scratch.join("generated-only.toml");
    fs::write(
        &config,
        "[tasks]\ninclude = [\"book/notes-generated.md\"]\n",
    )
    .unwrap();
    let run = WRun::of(ONE, &scratch, &["--tasks", config.to_str().unwrap()]);
    let result = run.result();
    assert_eq!(count(result, "tasks"), 0, "{result}");
    assert_eq!(result["w_before"], json!({"median": 0, "p90": 0, "max": 0}));
    assert_eq!(result["w_after"], json!({"median": 0, "p90": 0, "max": 0}));
    assert!(run.tasks().is_empty());
}

/// The tasks config from the label's variable: `SPECENGINE_TASKS_A` under
/// `--label pilot-a` wins over the corpus root's (absent here); without it
/// the refusal names the variable.
#[test]
fn the_tasks_variable_of_the_label_is_read() {
    let scratch = Scratch::new("w-tasks-variable");
    let corpus = copy_of(ONE, &scratch, "corpus");
    let tasks = scratch.join("tasks-elsewhere.toml");
    fs::rename(corpus.join("tasks.toml"), &tasks).unwrap();
    for (label, variable) in [
        ("pilot-a", "SPECENGINE_TASKS_A"),
        ("pilot-b", "SPECENGINE_TASKS_B"),
    ] {
        let out = scratch.join(&format!("out-{label}"));
        let args = [
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--label",
            label,
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ];
        let output = eval()
            .env(variable, &tasks)
            .args(args)
            .output()
            .expect("specengine-eval runs");
        let envelope = envelope(&output);
        assert_eq!(envelope["label"], label);
        assert_eq!(without_ms(&envelope["result"]), expected(ONE), "{label}");
        assert!(out.join("w").join(label).join("tasks.json").is_file());

        let out = scratch.join(&format!("out-{label}-unset"));
        let mut unset = args;
        unset[6] = out.to_str().unwrap();
        let output = eval().args(unset).output().expect("specengine-eval runs");
        assert_refused(&output, &out, label);
        assert!(
            stderr(&output).contains(variable),
            "{label}: {}",
            stderr(&output)
        );
    }
}

// --------------------------------------------------------------- AC-02

/// AC-02: the REFs are canonical — a front-matter reference, an inline ID,
/// a legacy alias, `<slug>/ID`, a `../` file link, the document's own ID in
/// its front-matter, its own section by a bare feature-scoped ID another
/// feature defines too (`<slug>/ID`, M3) — never the written form (M1:
/// written forms as REFs → `failed` ≥ 1); a dangling citation, a wiki link
/// and a non-document link are counted (M2) and none a REF; every REF names
/// a node through the test's own `spec show`.
#[test]
fn refs_are_canonical_and_citations_are_counted_not_followed() {
    let scratch = Scratch::new("w-refs");
    let wanted: [(&str, &str, &[&str]); 13] = [
        (
            ONE,
            "book/bare.md",
            &[
                "ASK-012",
                "NEED-01",
                "book/bare.md",
                "login/CK-01",
                "RULE-01",
                "NEED-30",
            ],
        ),
        (ONE, "book/broken.md", &["book/broken.md", "RULE-01"]),
        (
            ONE,
            "book/checkout.md",
            &[
                "NEED-02",
                "NEED-05",
                "book/flows/checkout.md",
                "checkout/CK-01",
                "login/CK-02",
            ],
        ),
        (
            ONE,
            "book/flows/login.md",
            &["book/flows/login.md", "login/CK-01", "login/CK-02"],
        ),
        (
            ONE,
            "book/flows/payment.md",
            &[
                "ASK-013",
                "NEED-01",
                "book/flows/payment.md",
                "book/needs.md",
            ],
        ),
        // The records layout moves out of the needs (both NEED-02
        // holders under one REF), the login flow and the plan a moved
        // record links (N1), the needs and a self-cited rule.
        (
            ONE,
            "book/needs.md",
            &[
                "ASK-012",
                "ASK-013",
                "NEED-01",
                "NEED-02",
                "NEED-05",
                "NEED-40",
                "book/flows/login.md",
                "book/needs.md",
                "RULE-02",
                "NEED-30",
            ],
        ),
        // `[gate](needs.md#NEED-40)`: the anchor names the record layout
        // moved out of the notebook, so the link lands on NEED-40's node,
        // not on the notebook's residue (R2).
        (
            ONE,
            "book/plan.md",
            &["NEED-05", "NEED-40", "book/index.md", "NEED-30"],
        ),
        (TWO, "log/DEC-0040.md", &["DEC-0040"]),
        (TWO, "log/OLD-0041.md", &["DEC-0041"]),
        // Every decision layout moves out of the log (PRB-0002 nested in
        // DEC-0003's record file), the log, and the billing page its link
        // names, moved by its slug (N2).
        (
            TWO,
            "log/decisions.md",
            &[
                "DEC-0001",
                "DEC-0002",
                "DEC-0003",
                "PRB-0002",
                "DEC-0006",
                "DEC-0007",
                "log/decisions.md",
                "topics/billing.md",
            ],
        ),
        (
            TWO,
            "pages/Bulk_Export/index.md",
            &["pages/Bulk_Export/index.md"],
        ),
        (
            TWO,
            "pages/billing/index.md",
            &[
                "DEC-0003",
                "pages/overview.md",
                "topics/billing.md",
                "billing/CRT-0001",
            ],
        ),
        (
            TWO,
            "pages/overview.md",
            &["DEC-0001", "DEC-0003", "pages/overview.md"],
        ),
    ];
    let runs: BTreeMap<&str, WRun> = W_FIXTURES
        .iter()
        .map(|name| (*name, WRun::of(name, &scratch, &[])))
        .collect();
    let home = scratch.join("library-home");
    for (name, source, refs) in wanted {
        let run = &runs[name];
        let task = run.task(source);
        assert_eq!(strings(&task["refs"]), refs, "{name} {source}: refs");
        for reference in refs {
            library_show(&run.tree(), &home, reference);
        }
        for reference in strings(&task["refs"]) {
            assert!(
                !reference.starts_with("CK-") && !reference.starts_with("CRT-"),
                "{source}: a bare feature-scoped REF {reference}"
            );
        }
    }
    for (name, run) in &runs {
        assert_eq!(count(run.result(), "failed"), 0, "{name}");
        // Each written form a REF would name nothing: the canonical forms
        // are what `w` asks.
        for task in run.tasks() {
            let refs = strings(&task["refs"]);
            for written in ["OLDR-05", "OLD-0003", "../needs.md", "CK-01", "needs.md"] {
                assert!(
                    !refs.iter().any(|r| r == written),
                    "{source}: {written}",
                    source = task["source"]
                );
            }
        }
    }

    // The citations of the payment task: one each, none a REF.
    let payment = runs[ONE].task("book/flows/payment.md");
    let citations = &payment["citations"];
    assert_eq!(
        citations["unresolved"],
        json!([{"written": "NEED-99", "line": 9}]),
        "{citations}"
    );
    assert_eq!(
        citations["unchecked"],
        json!([{"written": "../../engine/lib.rs", "line": 11}]),
        "{citations}"
    );
    assert_eq!(citations["wiki_links"], json!([10]), "{citations}");
    let result = runs[ONE].result();
    assert_eq!(count(result, "citations/unresolved"), 1, "{result}");
    assert_eq!(count(result, "citations/unchecked"), 1, "{result}");
    // The payment's wiki link and the one inside the gate record layout
    // moved out of the needs (N1: written in the needs task).
    assert_eq!(count(result, "citations/wiki_links"), 2, "{result}");
    let refs = strings(&payment["refs"]);
    for written in [
        "NEED-99",
        "Old Wiki Page",
        "engine/lib.rs",
        "../../engine/lib.rs",
    ] {
        assert!(
            !refs.iter().any(|reference| reference.contains(written)),
            "{written} became a REF: {refs:?}"
        );
    }
    // None followed: the needs document's own citations are not the
    // payment task's targets (its rules are not REFs).
    assert!(!refs.iter().any(|reference| reference.starts_with("RULE-")));

    // The duplicate of one: two targets, one REF.
    let checkout = runs[ONE].task("book/checkout.md");
    let need_02 = checkout["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| target["reference"] == "NEED-02")
        .count();
    assert_eq!(need_02, 2, "both holders of NEED-02 are targets");
}

/// AC-02: a legacy alias and a feature-scoped ID written in a task each ask
/// the CLI the canonical form; the written forms the CLI would not answer
/// (a bare feature-scoped ID another feature defines, a file link relative
/// to the task) are not what the bundle receives.
#[test]
fn the_written_forms_are_not_what_the_bundle_receives() {
    let scratch = Scratch::new("w-written");
    let run = WRun::of(ONE, &scratch, &[]);
    let home = scratch.join("library-home");
    // A written file link, relative to the task, names nothing for the CLI:
    // as a REF it would fail the task (M1).
    let outcome = library_bundle(
        &run.tree(),
        &home,
        vec!["../needs.md".to_owned()],
        DEFAULT_BUDGET,
    );
    let named_nothing = match outcome {
        Ok(outcome) => outcome.bundle.is_none(),
        Err(_) => true,
    };
    assert!(named_nothing, "a written relative link answered");
    // A bare feature-scoped ID names every feature's holder (M3): the
    // bundle of `CK-01` differs from the task's own `checkout/CK-01`.
    let bare = library_bundle(&run.tree(), &home, vec!["CK-01".to_owned()], DEFAULT_BUDGET);
    let scoped = library_bundle_of(
        &run.tree(),
        &home,
        &json!({"task": "probe", "refs": ["checkout/CK-01"]}),
        DEFAULT_BUDGET,
    );
    if let Ok(BundleOutcome {
        bundle: Some(bare), ..
    }) = bare
    {
        assert_ne!(bare.bundle_hash, scoped.bundle_hash);
    }
}

/// A task's target naming `reference` at `path`; panics when there is none.
fn target_at<'a>(task: &'a Value, reference: &str, path: &str) -> &'a Value {
    let targets = task["targets"].as_array().unwrap();
    targets
        .iter()
        .find(|target| target["reference"] == reference && target["path"] == path)
        .unwrap_or_else(|| {
            panic!(
                "{}: no target {reference} at {path}: {targets:?}",
                task["task"]
            )
        })
}

/// The corpus bytes of a task's before slots plus `sources`, each path
/// once.
fn before_bytes(corpus: &Path, task: &Value, sources: &[&str]) -> u64 {
    let paths: BTreeSet<String> = task["slots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["side"] == "before")
        .map(|entry| entry["path"].as_str().unwrap().to_owned())
        .chain(sources.iter().map(|source| (*source).to_owned()))
        .collect();
    paths.iter().map(|path| corpus_bytes(corpus, path)).sum()
}

/// AC-02 (M4) and AC-03, `docs/canon/w-measurement.md` "Targets N": each record
/// layout moves out of a task document into a record file is a target of
/// that task at its node there (both holders of a duplicate; a problem
/// nested in a moved section, at its line), the task's source its only
/// source, counted once in W_before; the links written inside a moved
/// record count as written in the task: a file link relative to the
/// task's source (dangling beside the record in the tree, read on the
/// corpus) and an inline ID; its `[[…]]` counts at its line in the record
/// file; the record bodies and the linked documents reach `Bundle.bytes`.
#[test]
fn records_moved_out_of_a_task_and_the_links_inside_them_are_its_targets() {
    let scratch = Scratch::new("w-moved");
    let one = WRun::of(ONE, &scratch, &[]);
    let corpus = fixture_dir(ONE);
    let needs = one.task("book/needs.md");
    let context = "one book/needs.md";
    for (reference, path) in [
        ("ASK-012", "book/atoms/ASK/ASK-012.md"),
        ("ASK-013", "book/atoms/ASK/ASK-013.md"),
        ("NEED-01", "book/atoms/NEED/NEED-01.md"),
        ("NEED-02", "book/atoms/NEED/NEED-02.md"),
        ("NEED-02", "book/atoms/NEED/NEED-02-2.md"),
        ("NEED-05", "book/atoms/NEED/NEED-05.md"),
        ("NEED-40", "book/atoms/NEED/NEED-40.md"),
    ] {
        let target = target_at(&needs, reference, path);
        assert_eq!(target["line"], 1, "{context}: {target}");
        assert_eq!(target["sources"], json!(["book/needs.md"]), "{context}");
        assert!(one.tree().join(path).is_file(), "{path}");
    }

    // The links inside the gate record: `flows/login.md`, written relative
    // to the needs, names nothing beside the record in the tree and the
    // login flow on the corpus; the plan by its serial. The residue of the
    // needs holds neither: they are the record's.
    let gate = fs::read_to_string(one.tree().join("book/atoms/NEED/NEED-40.md")).unwrap();
    assert!(gate.contains("](flows/login.md)"), "{gate}");
    assert!(gate.contains("NEED-30"), "{gate}");
    assert!(!one.tree().join("book/atoms/NEED/flows/login.md").exists());
    let residue = fs::read_to_string(one.tree().join("book/needs.md")).unwrap();
    for written in ["flows/login.md", "NEED-30", "[[", "The gate text"] {
        assert!(!residue.contains(written), "the residue holds {written}");
    }
    assert_eq!(
        target_at(&needs, "book/flows/login.md", "book/flows/login.md")["sources"],
        json!(["book/flows/login.md"])
    );
    assert_eq!(
        target_at(&needs, "NEED-30", "book/plan.md")["sources"],
        json!(["book/plan.md"])
    );
    // Its wiki link, at its line in the record file; nothing dangles.
    let line = gate
        .lines()
        .position(|line| line.contains("[[Gate History]]"))
        .expect("the gate record keeps its wiki link")
        + 1;
    assert_eq!(
        needs["citations"],
        json!({
            "unresolved": [],
            "unchecked": [],
            "wiki_links": [{"path": "book/atoms/NEED/NEED-40.md", "line": line}],
        }),
        "{context}"
    );

    // W_before: the needs' source once for its nine targets there, plus
    // the two documents the record links.
    let sources = ["book/flows/login.md", "book/needs.md", "book/plan.md"];
    let from_needs = needs["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| target["sources"] == json!(["book/needs.md"]))
        .count();
    assert!(from_needs > 1, "{context}: {from_needs}");
    assert_eq!(
        target_sources(&needs),
        sources.iter().map(|s| (*s).to_owned()).collect()
    );
    assert_eq!(
        needs["w_before"],
        before_bytes(&corpus, &needs, &sources),
        "{context}"
    );
    assert_eq!(needs["docs_needed"], 3, "{context}");

    // W_after: the bundle carries the record bodies (none of them in the
    // residue) and the documents the record links.
    let body = one.body(&needs);
    assert_eq!(needs["bundle"]["bytes"], body.len(), "{context}");
    for text in [
        "Which name does the notebook print?",
        "The engine keeps every byte it reads.",
        "A second definition of an existing need.",
        "The gate text moves to a record file.",
        "see [[Gate History]].",
        "# Login",
        "The user signs in with a name.",
        "The plan is a document record",
    ] {
        assert!(body.contains(text), "{context}: the bundle lacks {text:?}");
    }
    let tier0 = tier0_of(&corpus);
    assert_eq!(
        needs["w_after"],
        after_slots(&one, &needs, tier0.as_deref()) + body.len() as u64,
        "{context}"
    );

    // `two`: every decision layout moves out of the log, the nested
    // problem at its line in DEC-0003's record file; the log's source
    // once; the moved bodies in the bundle, not in the residue.
    let two = WRun::of(TWO, &scratch, &[]);
    let log = two.task("log/decisions.md");
    let record = fs::read_to_string(two.tree().join("ledger/DEC/DEC-0003.md")).unwrap();
    let nested = record
        .lines()
        .position(|line| line.contains("{#PRB-0002}"))
        .expect("the problem moved with its section")
        + 1;
    for (reference, path, line) in [
        ("DEC-0001", "ledger/DEC/DEC-0001.md", 1),
        ("DEC-0001", "ledger/DEC/DEC-0001-2.md", 1),
        ("DEC-0002", "ledger/DEC/DEC-0002.md", 1),
        ("DEC-0003", "ledger/DEC/DEC-0003.md", 1),
        ("PRB-0002", "ledger/DEC/DEC-0003.md", nested),
        ("DEC-0006", "ledger/DEC/DEC-0006.md", 1),
        ("DEC-0007", "ledger/DEC/DEC-0007.md", 1),
    ] {
        let target = target_at(&log, reference, path);
        assert_eq!(target["line"], line, "two log/decisions.md: {target}");
        assert_eq!(target["sources"], json!(["log/decisions.md"]));
    }
    // The other holder of DEC-0001 (the problems page) is not the log's.
    assert!(!target_sources(&log).contains("log/problems.md"), "{log}");
    let sources = ["log/decisions.md", "pages/billing/index.md"];
    assert_eq!(
        log["w_before"],
        before_bytes(&fixture_dir(TWO), &log, &sources),
        "two log/decisions.md"
    );
    let body = two.body(&log);
    let residue = fs::read_to_string(two.tree().join("log/decisions.md")).unwrap();
    for text in [
        "A legacy section moves to its record file.",
        "a problem nested in the moved section",
        "Keep the log append-only.",
    ] {
        assert!(body.contains(text), "two: the bundle lacks {text:?}");
        assert!(!residue.contains(text), "two: the residue holds {text:?}");
    }
}

/// AC-02 (M5), `docs/canon/w-measurement.md` "Targets N": a file link that
/// resolves on the corpus but dangles in the tree because layout moved its
/// end (the log's link to the billing page `slug` moved to
/// `topics/billing.md`; the gate record's link written relative to the
/// needs) is the moved document's node, a target; a link dangling on the
/// corpus too stays `unresolved`.
#[test]
fn a_link_to_a_document_layout_moved_is_a_target_not_unresolved() {
    let scratch = Scratch::new("w-relocated");
    let two = WRun::of(TWO, &scratch, &[]);
    let log = two.task("log/decisions.md");
    let residue = fs::read_to_string(two.tree().join("log/decisions.md")).unwrap();
    assert!(
        residue.contains("](../pages/billing/index.md)"),
        "{residue}"
    );
    assert!(!two.tree().join("pages/billing/index.md").exists());
    assert!(fixture_dir(TWO).join("pages/billing/index.md").is_file());
    let billing = target_at(&log, "topics/billing.md", "topics/billing.md");
    assert_eq!(billing["line"], 1);
    assert_eq!(billing["sources"], json!(["pages/billing/index.md"]));
    assert!(strings(&log["refs"]).contains(&"topics/billing.md".to_owned()));
    // Only the mention dangling on the corpus too is unresolved.
    let unresolved: Vec<&str> = log["citations"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["written"].as_str().unwrap())
        .collect();
    assert_eq!(unresolved, ["DEC-0005"], "{log}");
    assert_eq!(log["citations"]["unchecked"], json!([]), "{log}");
    assert_eq!(count(two.result(), "citations/unresolved"), 3);

    let one = WRun::of(ONE, &scratch, &[]);
    let needs = one.task("book/needs.md");
    assert!(strings(&needs["refs"]).contains(&"book/flows/login.md".to_owned()));
    assert_eq!(needs["citations"]["unresolved"], json!([]), "{needs}");
    // The payment's NEED-99 dangles on the corpus too: unresolved.
    let payment = one.task("book/flows/payment.md");
    assert_eq!(
        payment["citations"]["unresolved"],
        json!([{"written": "NEED-99", "line": 9}])
    );
}

/// `docs/canon/w-measurement.md` "Targets N": a file link the tree leaves
/// unchecked (the billing page's `../overview.md`, moved with the page
/// under `topics/`, leaves the tree's walk scope) that resolves on the
/// corpus from the task's source is the linked document's node, a target,
/// not `unchecked`.
#[test]
fn a_link_the_tree_leaves_unchecked_is_read_on_the_corpus() {
    let scratch = Scratch::new("w-unchecked");
    let two = WRun::of(TWO, &scratch, &[]);
    let billing = two.task("pages/billing/index.md");
    assert_eq!(billing["after"], "topics/billing.md");
    let after = fs::read_to_string(two.tree().join("topics/billing.md")).unwrap();
    assert!(after.contains("](../overview.md)"), "{after}");
    assert!(!two.tree().join("overview.md").exists());
    assert!(fixture_dir(TWO).join("pages/overview.md").is_file());
    let overview = target_at(&billing, "pages/overview.md", "pages/overview.md");
    assert_eq!(overview["line"], 1);
    assert_eq!(overview["sources"], json!(["pages/overview.md"]));
    assert_eq!(billing["citations"]["unchecked"], json!([]), "{billing}");
    assert_eq!(billing["citations"]["unresolved"], json!([]), "{billing}");
    assert_eq!(count(two.result(), "citations/unchecked"), 0);
    // The overview is a document the before reader opens too.
    assert!(target_sources(&billing).contains("pages/overview.md"));
    assert!(two.body(&billing).contains("# Overview"));
}

/// `docs/canon/w-measurement.md` "Targets N" (R1) on a copy: two file links
/// written in the legacy section layout moves out of the log, both
/// dangling beside its record file in the tree, read on the corpus from
/// the log as core would: `../tools/README.md` names a corpus file, but
/// its only candidate lies outside the before walk scope (`roots` log,
/// pages, ledger), so core never checks it: `unchecked`; `../pages/
/// nowhere.md`, a candidate inside the scope, dangles: `unresolved`.
/// Neither is a REF.
#[test]
fn a_dangling_link_in_a_moved_record_with_no_candidate_in_the_before_scope_is_unchecked() {
    let scratch = Scratch::new("w-out-of-scope");
    let corpus = copy_of(TWO, &scratch, "corpus");
    edit(
        &corpus.join("log/decisions.md"),
        "A legacy section moves to its record file.\n",
        "A legacy section moves to its record file. See [code](../tools/README.md) and \
         [gone](../pages/nowhere.md).\n",
    );
    let scheme: toml::Table =
        toml::from_str(&fs::read_to_string(corpus.join("specengine.toml")).unwrap()).unwrap();
    let roots: Vec<&str> = scheme["paths"]["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|root| root.as_str().unwrap())
        .collect();
    assert_eq!(roots, ["log", "pages", "ledger"], "tools/ is outside");
    assert!(corpus.join("tools/README.md").is_file());
    assert!(!corpus.join("pages/nowhere.md").exists());

    let run = WRun::new(&corpus, &scratch, "out", &[]);
    let fixture = WRun::of(TWO, &scratch, &[]);
    let record = fs::read_to_string(run.tree().join("ledger/DEC/DEC-0003.md")).unwrap();
    assert!(record.contains("](../tools/README.md)"), "{record}");
    assert!(record.contains("](../pages/nowhere.md)"), "{record}");
    // Both dangle in the tree: beside the record, inside its `ledger` root.
    assert!(!run.tree().join("ledger/tools/README.md").exists());
    assert!(!run.tree().join("ledger/pages/nowhere.md").exists());

    let log = run.task("log/decisions.md");
    let at =
        |written: &str| json!({"written": written, "line": 8, "path": "ledger/DEC/DEC-0003.md"});
    assert_eq!(
        log["citations"]["unchecked"],
        json!([at("../tools/README.md")]),
        "{log}"
    );
    assert_eq!(
        log["citations"]["unresolved"],
        json!([at("../pages/nowhere.md"), {"written": "DEC-0005", "line": 17}]),
        "{log}"
    );
    assert_eq!(log["refs"], fixture.task("log/decisions.md")["refs"]);
    assert_eq!(count(run.result(), "citations/unchecked"), 1);
    assert_eq!(
        count(run.result(), "citations/unresolved"),
        count(fixture.result(), "citations/unresolved") + 1
    );
    assert_eq!(count(run.result(), "failed"), 0);
}

/// `docs/canon/w-measurement.md` "Targets N" (R2) on copies: a link whose
/// anchor names nothing in the tree, landing on a document a record was
/// moved out of, reads on the corpus the section that anchor named: an
/// inline `#OLD-0003` (DEC-0003's legacy alias, the only route to it) and a
/// path-form `canon:` naming `#NEED-40` (the notebook's moved gate) land on
/// the record nodes, not on the residues; a heading-slug anchor of the
/// same moved section (`#superseded`) names neither ID nor alias and stays
/// the log's document (a known limit).
#[test]
fn an_anchor_naming_a_record_moved_out_lands_on_the_record() {
    let scratch = Scratch::new("w-moved-anchor");
    let two = copy_of(TWO, &scratch, "two");
    edit(
        &two.join("log/DEC-0040.md"),
        "The decision to archive is a document record.\n",
        "The decision to archive is a document record, after [the superseded \
         one](decisions.md#OLD-0003).\n",
    );
    fs::write(
        two.join("pages/Bulk_Export/index.md"),
        format!(
            "{}\nSee [the rows](../../log/decisions.md#superseded).\n",
            fs::read_to_string(two.join("pages/Bulk_Export/index.md")).unwrap()
        ),
    )
    .unwrap();
    let run = WRun::new(&two, &scratch, "two-out", &[]);
    let residue = fs::read_to_string(run.tree().join("log/decisions.md")).unwrap();
    assert!(
        !residue.contains("OLD-0003") && !residue.contains("Superseded"),
        "the section left the log: {residue}"
    );

    let archive = run.task("log/DEC-0040.md");
    assert_eq!(
        strings(&archive["refs"]),
        ["DEC-0003", "DEC-0040"],
        "{archive}"
    );
    let record = target_at(&archive, "DEC-0003", "ledger/DEC/DEC-0003.md");
    assert_eq!(record["sources"], json!(["log/decisions.md"]));
    assert_eq!(archive["citations"]["unresolved"], json!([]), "{archive}");
    let body = run.body(&archive);
    assert!(
        body.contains("A legacy section moves to its record file."),
        "{body}"
    );
    assert!(
        !body.contains("The log keeps one row per decision."),
        "{body}"
    );

    let export = run.task("pages/Bulk_Export/index.md");
    assert_eq!(
        strings(&export["refs"]),
        ["log/decisions.md", "pages/Bulk_Export/index.md"],
        "{export}"
    );
    assert_eq!(
        export["citations"]["unresolved"],
        json!([{"written": "CRT-0003", "line": 6}]),
        "{export}"
    );
    assert_eq!(count(run.result(), "failed"), 0);

    // A path-form `canon:` (root-relative) naming the notebook's moved gate.
    let one = copy_of(ONE, &scratch, "one");
    edit(
        &one.join("book/flows/login.md"),
        "sort: flow\n",
        "sort: flow\ncanon: book/needs.md#NEED-40\n",
    );
    let run = WRun::new(&one, &scratch, "one-out", &[]);
    let login = fs::read_to_string(run.tree().join("book/flows/login.md")).unwrap();
    assert!(login.contains("canon: book/needs.md#NEED-40"), "{login}");
    let task = run.task("book/flows/login.md");
    assert_eq!(
        strings(&task["refs"]),
        [
            "NEED-40",
            "book/flows/login.md",
            "login/CK-01",
            "login/CK-02"
        ],
        "{task}"
    );
    let gate = target_at(&task, "NEED-40", "book/atoms/NEED/NEED-40.md");
    assert_eq!(gate["sources"], json!(["book/needs.md"]));
    let body = run.body(&task);
    assert!(
        body.contains("The gate text moves to a record file."),
        "{body}"
    );
    assert!(!body.contains("# Needs"), "{body}");
    assert_eq!(count(run.result(), "failed"), 0);
}

// --------------------------------------------------------------- AC-03

/// One task's expected reading protocol: (fixture, source, shard, Tier 1,
/// source documents).
type Protocol = (
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static [&'static str],
);

/// AC-03: each task's slots follow the reading protocol, checked here
/// against the fixture's files: Tier 0, the index root, the first shard in
/// path order linking the task (the second shard for the checkout; a
/// percent-encoded and a `/`-led link), the Tier 1 of the key table (the
/// larger of two matched READMEs, a larger one unmatched: M3), the default
/// for an unmapped value or no front-matter, both sources of a duplicate;
/// W_before = the corpus bytes (M2) of the distinct slot and source paths,
/// a document cited twice counted once (M1); `docs_needed`, `third_step`.
#[test]
fn before_slots_follow_the_protocol_and_count_each_document_once() {
    let scratch = Scratch::new("w-before");
    // (fixture, source, shard, tier1, sources)
    let wanted: [Protocol; 10] = [
        (
            ONE,
            "book/bare.md",
            Some("book/index-a.md"),
            Some("book/areas/README.md"),
            &[
                "book/bare.md",
                "book/flows/login.md",
                "book/needs.md",
                "book/plan.md",
            ],
        ),
        (
            ONE,
            "book/broken.md",
            None,
            Some("book/areas/README.md"),
            &["book/broken.md", "book/needs.md"],
        ),
        (
            ONE,
            "book/checkout.md",
            Some("book/index-b.md"),
            Some("engine/README.md"),
            &["book/checkout.md", "book/flows/login.md", "book/needs.md"],
        ),
        (
            ONE,
            "book/flows/login.md",
            Some("book/index-a.md"),
            Some("book/areas/README.md"),
            &["book/flows/login.md"],
        ),
        (
            ONE,
            "book/flows/payment.md",
            Some("book/index-a.md"),
            Some("engine/store/README.md"),
            &["book/flows/payment.md", "book/needs.md"],
        ),
        (
            ONE,
            "book/needs.md",
            Some("book/index-b.md"),
            Some("book/areas/README.md"),
            &["book/flows/login.md", "book/needs.md", "book/plan.md"],
        ),
        (
            ONE,
            "book/plan.md",
            Some("book/index-b.md"),
            Some("book/areas/README.md"),
            &["book/needs.md", "book/plan.md"],
        ),
        (
            TWO,
            "pages/overview.md",
            None,
            Some("tools/README.md"),
            &["log/decisions.md", "log/problems.md", "pages/overview.md"],
        ),
        (
            TWO,
            "pages/billing/index.md",
            None,
            Some("tools/README.md"),
            &[
                "log/decisions.md",
                "pages/billing/index.md",
                "pages/overview.md",
            ],
        ),
        (
            TWO,
            "log/decisions.md",
            None,
            Some("tools/README.md"),
            &["log/decisions.md", "pages/billing/index.md"],
        ),
    ];
    let runs: BTreeMap<&str, WRun> = W_FIXTURES
        .iter()
        .map(|name| (*name, WRun::of(name, &scratch, &[])))
        .collect();
    for (name, source, shard, tier1, sources) in wanted {
        let run = &runs[name];
        let corpus = fixture_dir(name);
        let task = run.task(source);
        let context = format!("{name} {source}");
        assert_eq!(
            slot(&task, "shard", "before").map(|(path, _)| path),
            shard.map(str::to_owned),
            "{context}: shard"
        );
        assert_eq!(
            slot(&task, "tier1", "before").map(|(path, _)| path),
            tier1.map(str::to_owned),
            "{context}: tier1"
        );
        let got: BTreeSet<String> = target_sources(&task);
        let want: BTreeSet<String> = sources.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(got, want, "{context}: sources");

        // W_before over the distinct paths, from the fixture's own bytes.
        let tier0 = tier0_of(&corpus);
        let index = {
            let text = fs::read_to_string(corpus.join("specengine.toml")).unwrap();
            let table: toml::Table = toml::from_str(&text).unwrap();
            table["paths"]
                .get("index")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        };
        let slots: BTreeSet<String> = [
            tier0,
            tier1.map(str::to_owned),
            index,
            shard.map(str::to_owned),
        ]
        .into_iter()
        .flatten()
        .collect();
        let paths: BTreeSet<&String> = slots.iter().chain(want.iter()).collect();
        let w_before: u64 = paths.iter().map(|path| corpus_bytes(&corpus, path)).sum();
        assert_eq!(task["w_before"], w_before, "{context}: W_before");
        let needed = want.iter().filter(|path| !slots.contains(*path)).count();
        assert_eq!(task["docs_needed"], needed, "{context}: docs_needed");
        for entry in task["slots"].as_array().unwrap() {
            if entry["side"] == "before" {
                let path = entry["path"].as_str().unwrap();
                assert_eq!(
                    entry["bytes"],
                    corpus_bytes(&corpus, path),
                    "{context}: slot {path}"
                );
            }
        }
    }

    // Every task of both fixtures: the same rule over what tasks.json
    // records; `third_step` = tasks needing more than three documents.
    for (name, run) in &runs {
        let corpus = fixture_dir(name);
        let tasks = run.tasks();
        for task in &tasks {
            let slots: BTreeSet<String> = task["slots"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|entry| entry["side"] == "before")
                .map(|entry| entry["path"].as_str().unwrap().to_owned())
                .collect();
            let sources = target_sources(task);
            let paths: BTreeSet<&String> = slots.iter().chain(sources.iter()).collect();
            let w_before: u64 = paths.iter().map(|path| corpus_bytes(&corpus, path)).sum();
            assert_eq!(task["w_before"], w_before, "{name} {}", task["task"]);
        }
        let third = tasks
            .iter()
            .filter(|task| task["docs_needed"].as_u64().unwrap() > 3)
            .count();
        assert_eq!(count(run.result(), "third_step"), third as u64, "{name}");
    }
    assert_eq!(count(runs[ONE].result(), "third_step"), 1);
    assert_eq!(runs[ONE].task("book/bare.md")["docs_needed"], 4);

    // M2's witness: the tree's bytes differ from the corpus's for a
    // source and for the default Tier 1.
    for path in ["book/needs.md", "book/areas/README.md"] {
        assert_ne!(
            file_size(&runs[ONE].tree(), path),
            file_size(&fixture_dir(ONE), path),
            "{path}"
        );
    }
    // M3's witness: the unmatched README is the table's largest.
    let corpus = fixture_dir(ONE);
    assert!(
        corpus_bytes(&corpus, "engine/out/README.md") > corpus_bytes(&corpus, "engine/README.md")
    );
    assert!(
        corpus_bytes(&corpus, "engine/README.md") > corpus_bytes(&corpus, "engine/store/README.md")
    );
    // The slot counts.
    let result = runs[ONE].result();
    assert_eq!(count(result, "slots/shard"), 6, "{result}");
    assert_eq!(count(result, "slots/tier1/key"), 2, "{result}");
    assert_eq!(count(result, "slots/tier1/default"), 5, "{result}");
    assert_eq!(count(result, "slots/unmapped"), 1, "{result}");
    assert_eq!(
        count(result, "slots/tier0"),
        corpus_bytes(&corpus, "GUIDE.md")
    );
    assert_eq!(
        count(result, "slots/index"),
        corpus_bytes(&corpus, "book/index.md")
    );
    let result = runs[TWO].result();
    assert_eq!(count(result, "slots/tier0"), 0, "{result}");
    assert_eq!(count(result, "slots/index"), 0, "{result}");
    assert_eq!(count(result, "slots/shard"), 0, "{result}");
    assert_eq!(count(result, "slots/tier1/default"), 6, "{result}");
}

/// AC-03 edges of the Tier 1 rule on a copy: a typed list key (`scope`,
/// read through `Node.fields`) with two matched READMEs of equal size →
/// the first in path order; a key value no table entry maps and no
/// `default` → no Tier 1 slot (`slots.tier1.none`).
#[test]
fn tier1_ties_go_by_path_order_and_a_typed_key_is_read() {
    let scratch = Scratch::new("w-tier1");
    let corpus = copy_of(ONE, &scratch, "corpus");
    fs::create_dir_all(corpus.join("engine/b")).unwrap();
    fs::create_dir_all(corpus.join("engine/a")).unwrap();
    fs::write(
        corpus.join("engine/b/README.md"),
        "# B area\n\nSame size.\n",
    )
    .unwrap();
    fs::write(
        corpus.join("engine/a/README.md"),
        "# A area\n\nSame size.\n",
    )
    .unwrap();
    edit(
        &corpus.join("book/checkout.md"),
        "area: [input, store]\n",
        "scope: [beta, alpha]\n",
    );
    fs::write(
        corpus.join("tasks.toml"),
        "[tasks]\ninclude = [\"book/*.md\", \"book/flows/**\"]\nexclude = [\"book/lexicon.md\"]\n\n\
         [tier1]\nkey = \"scope\"\n\n[tier1.paths]\n\"beta\" = \"engine/b/README.md\"\n\"alpha\" = \"engine/a/README.md\"\n",
    )
    .unwrap();
    let run = WRun::new(&corpus, &scratch, "out", &[]);
    let checkout = run.task("book/checkout.md");
    assert_eq!(
        slot(&checkout, "tier1", "before").map(|(path, _)| path),
        Some("engine/a/README.md".to_owned()),
        "a tie goes to the first path"
    );
    assert_eq!(
        slot(&checkout, "tier1", "after").map(|(path, _)| path),
        Some("engine/a/README.md".to_owned()),
        "one path, both sides"
    );
    let result = run.result();
    assert_eq!(count(result, "slots/tier1/key"), 1, "{result}");
    assert_eq!(count(result, "slots/tier1/none"), 6, "{result}");
    assert_eq!(count(result, "slots/tier1/default"), 0, "{result}");
    let bare = run.task("book/bare.md");
    assert!(
        slot(&bare, "tier1", "before").is_none(),
        "no default: no slot"
    );
}

/// The before slots of a task: (slot, path, bytes) in recorded order.
fn before_slots(task: &Value) -> Vec<(String, String, u64)> {
    task["slots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["side"] == "before")
        .map(|entry| {
            (
                entry["slot"].as_str().unwrap().to_owned(),
                entry["path"].as_str().unwrap().to_owned(),
                entry["bytes"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// AC-03 (M4) on a copy, `docs/canon/w-measurement.md` "W_before": with no
/// `index = true` generator, the generator whose `writes` holds `[paths]
/// index` supplies the shards (its other outputs the corpus holds, the
/// shard paths in `writes` alone, as pilot A registers it): every task's
/// before slots, W_before and `docs_needed` equal the `index = true`
/// fixture's, the checkout's shard still the second one.
#[test]
fn an_a_style_generator_supplies_the_same_shards() {
    let scratch = Scratch::new("w-a-style");
    let corpus = copy_of(ONE, &scratch, "corpus");
    let scheme = corpus.join("specengine.toml");
    edit(&scheme, "index   = true\n", "");
    let text = fs::read_to_string(&scheme).unwrap();
    let shards = text
        .lines()
        .find(|line| line.starts_with("shards"))
        .expect("the fixture names its shards")
        .to_owned();
    edit(&scheme, &format!("{shards}\n"), "");
    let table: toml::Table = toml::from_str(&fs::read_to_string(&scheme).unwrap()).unwrap();
    let generator = &table["generators"].as_array().unwrap()[0];
    assert!(generator.get("index").is_none() && generator.get("shards").is_none());
    let writes: Vec<&str> = generator["writes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap())
        .collect();
    assert_eq!(
        writes,
        ["book/index.md", "book/index-a.md", "book/index-b.md"]
    );

    let a_style = WRun::new(&corpus, &scratch, "a-style", &[]);
    let fixture = WRun::of(ONE, &scratch, &[]);
    // No index generator runs: the tree holds no rendered index.
    assert!(!a_style.tree().join("book/index.md").exists());
    assert!(fixture.tree().join("book/index.md").is_file());
    for source in ONE_TASKS {
        let theirs = fixture.task(source);
        let ours = a_style.task(source);
        assert_eq!(
            before_slots(&ours),
            before_slots(&theirs),
            "{source}: slots"
        );
        assert_eq!(ours["w_before"], theirs["w_before"], "{source}: W_before");
        assert_eq!(ours["docs_needed"], theirs["docs_needed"], "{source}");
    }
    assert_eq!(
        slot(&a_style.task("book/checkout.md"), "shard", "before").map(|(path, _)| path),
        Some("book/index-b.md".to_owned())
    );
    for key in ["slots/shard", "slots/index", "slots/tier0", "third_step"] {
        assert_eq!(
            count(a_style.result(), key),
            count(fixture.result(), key),
            "{key}"
        );
    }
    assert_eq!(count(a_style.result(), "slots/shard"), 6);
    assert_eq!(a_style.result()["w_before"], fixture.result()["w_before"]);
}

/// `docs/canon/w-measurement.md` "W_before" (R3) on a copy: an A-style
/// generator's output that is not a Markdown document is never a shard,
/// though the corpus holds it, it sorts first and it links every task:
/// each task's before slots equal the `index = true` fixture's.
#[test]
fn a_non_markdown_write_of_an_a_style_generator_is_never_a_shard() {
    let scratch = Scratch::new("w-a-style-txt");
    let corpus = copy_of(ONE, &scratch, "corpus");
    let scheme = corpus.join("specengine.toml");
    edit(&scheme, "index   = true\n", "");
    let text = fs::read_to_string(&scheme).unwrap();
    let shards = text
        .lines()
        .find(|line| line.starts_with("shards"))
        .expect("the fixture names its shards")
        .to_owned();
    edit(&scheme, &format!("{shards}\n"), "");
    edit(
        &scheme,
        "writes  = [\"book/index.md\", ",
        "writes  = [\"book/index.md\", \"book/index-0.txt\", ",
    );
    let links: String = ONE_TASKS
        .iter()
        .map(|source| format!("- [{source}](/{source})\n"))
        .collect();
    fs::write(
        corpus.join("book/index-0.txt"),
        format!("# Shard 0\n\n{links}"),
    )
    .unwrap();
    let table: toml::Table = toml::from_str(&fs::read_to_string(&scheme).unwrap()).unwrap();
    let generator = &table["generators"].as_array().unwrap()[0];
    assert!(generator.get("index").is_none());
    assert_eq!(
        generator["writes"].as_array().unwrap()[1].as_str(),
        Some("book/index-0.txt")
    );

    let run = WRun::new(&corpus, &scratch, "out", &[]);
    let fixture = WRun::of(ONE, &scratch, &[]);
    for source in ONE_TASKS {
        let ours = run.task(source);
        assert_eq!(
            before_slots(&ours),
            before_slots(&fixture.task(source)),
            "{source}: slots"
        );
        assert_ne!(
            slot(&ours, "shard", "before").map(|(path, _)| path),
            Some("book/index-0.txt".to_owned()),
            "{source}"
        );
    }
    assert_eq!(count(run.result(), "slots/shard"), 6);
    assert_eq!(run.result()["w_before"], fixture.result()["w_before"]);
}

/// `docs/canon/w-measurement.md` "W_before" (core's resolution of a shard's
/// links) on copies: a `%` not followed by two hex digits stays as written
/// (`%+f` names a page whose file name holds it, not a decoded byte); a
/// link that names nothing from the shard's directory is read from `[paths]
/// link_base`.
#[test]
fn shard_links_resolve_as_core_does() {
    let scratch = Scratch::new("w-shard-links");
    // `%+f`: no hex pair, no decoding.
    let corpus = copy_of(ONE, &scratch, "percent");
    fs::write(
        corpus.join("book/odd%+f.md"),
        "# Odd\n\nA page whose file name holds a percent sign.\n",
    )
    .unwrap();
    edit(
        &corpus.join("book/index-a.md"),
        "- [Bare notes](b%61re.md)\n",
        "- [Bare notes](b%61re.md)\n- [Odd](odd%+f.md)\n",
    );
    let run = WRun::new(&corpus, &scratch, "percent-out", &[]);
    let odd = run.task("book/odd%+f.md");
    assert_eq!(
        slot(&odd, "shard", "before").map(|(path, _)| path),
        Some("book/index-a.md".to_owned()),
        "{odd}"
    );
    assert_eq!(
        slot(&run.task("book/bare.md"), "shard", "before").map(|(path, _)| path),
        Some("book/index-a.md".to_owned()),
        "%61 still decodes"
    );

    // `[paths] link_base`: the login link of shard A, written from the
    // flows directory, names the login flow only from there.
    let corpus = copy_of(ONE, &scratch, "link-base");
    edit(
        &corpus.join("specengine.toml"),
        "index = \"book/index.md\"\n",
        "index = \"book/index.md\"\nlink_base = \"book/flows\"\n",
    );
    edit(
        &corpus.join("book/index-a.md"),
        "- [Login](flows/login.md)\n",
        "- [Login](login.md)\n",
    );
    assert!(!corpus.join("book/login.md").exists());
    let run = WRun::new(&corpus, &scratch, "link-base-out", &[]);
    assert_eq!(
        slot(&run.task("book/flows/login.md"), "shard", "before").map(|(path, _)| path),
        Some("book/index-a.md".to_owned()),
        "the first shard links the login flow from link_base"
    );
}

/// docs/features/pilot-w.md AC-01 wording: `w --help` names every task
/// document, not a "live" one, and cites the spec's AC-01 for `--tasks`.
#[test]
fn the_help_names_every_task_document_and_cites_ac_01() {
    let output = run(&["w", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let help = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(help.contains("AC-01"), "{help}");
    assert!(!help.contains("live"), "{help}");
}

// --------------------------------------------------------------- AC-04

/// AC-04: per bundled task the test's own `specengine_cli::bundle` on
/// `tree/` gives `bytes` and `bundle_hash` equal to `tasks.json` and the
/// body of `bundles/task-N.txt`; W_after = after Tier 0 + Tier 1 + the
/// bundle's bytes. At the default budget, with a multi-byte task (M1:
/// chars or tokens for bytes → red), at a budget with a not-included list
/// (M2: the list stripped → red), and on a copy with Cyrillic text.
#[test]
fn w_after_is_the_library_bundle_on_the_tree() {
    let scratch = Scratch::new("w-after");
    let cyrillic = copy_of(ONE, &scratch, "cyrillic");
    edit(
        &cyrillic.join("book/checkout.md"),
        "Closing prose of the flow:",
        "\u{0417}\u{0430}\u{043a}\u{0440}\u{044b}\u{0442}\u{0438}\u{0435} \u{043f}\u{043e}\u{0442}\u{043e}\u{043a}\u{0430}. Closing prose of the flow:",
    );
    let runs = [
        WRun::of(ONE, &scratch, &[]),
        WRun::at_budget(ONE, &scratch, OUTLINE_BUDGET),
        WRun::of(TWO, &scratch, &[]),
        WRun::new(&cyrillic, &scratch, "cyrillic-out", &[]),
    ];
    let mut multibyte = 0;
    let mut not_included = 0;
    for (index, run) in runs.iter().enumerate() {
        let home = scratch.join(&format!("library-home-{index}"));
        let tier0 = tier0_of(&run.corpus);
        let mut bodies = BTreeSet::new();
        for task in run.tasks() {
            let context = format!("run {index} {}", task["task"]);
            if task["outcome"] != "bundled" {
                continue;
            }
            let bundle = library_bundle_of(&run.tree(), &home, &task, run.budget());
            let recorded = &task["bundle"];
            assert_eq!(recorded["bytes"], bundle.bytes, "{context}: bytes");
            assert_eq!(
                recorded["bundle_hash"], bundle.bundle_hash,
                "{context}: hash"
            );
            assert_eq!(recorded["chars"], bundle.chars, "{context}: chars");
            assert_eq!(recorded["tokens"], bundle.tokens, "{context}: tokens");
            assert_eq!(recorded["not_included"], bundle.not_included(), "{context}");
            assert_eq!(run.body(&task), bundle.body, "{context}: body");
            assert_eq!(bundle.body.len(), bundle.bytes, "{context}");
            let after = after_slots(run, &task, tier0.as_deref()) + bundle.bytes as u64;
            assert_eq!(task["w_after"], after, "{context}: W_after");
            if bundle.chars != bundle.bytes {
                multibyte += 1;
            }
            if bundle.not_included() > 0 {
                not_included += 1;
                assert!(
                    bundle.body.lines().count() > 1,
                    "{context}: the not-included list is in the body"
                );
            }
            bodies.insert(format!("{}.txt", task["task"].as_str().unwrap()));
        }
        let files: BTreeSet<String> = fs::read_dir(run.detail.join("bundles"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, bodies, "run {index}: one body per bundled task");
    }
    assert!(multibyte >= 2, "a multi-byte task in one and in the copy");
    assert!(not_included > 0, "a not-included list at {OUTLINE_BUDGET}");
    // The after side reads Tier 1 from the tree when it holds the path.
    let bare = runs[0].task("book/bare.md");
    let (_, before) = slot(&bare, "tier1", "before").unwrap();
    let (_, after) = slot(&bare, "tier1", "after").unwrap();
    assert_ne!(before, after, "the tree's Tier 1 differs from the corpus's");
}

/// AC-04: `tree/` is the tree `layout` writes for the same corpus, byte for
/// byte, `.spec-debt.toml` aside (M3: no index outputs → red).
#[test]
fn the_tree_is_layouts_tree_without_the_debt_baseline() {
    let scratch = Scratch::new("w-tree");
    for name in W_FIXTURES {
        let run = WRun::of(name, &scratch, &[]);
        let out = scratch.join(&format!("layout-{}", name.replace('/', "-")));
        let layout = envelope(&run_layout(&fixture_dir(name), &out));
        assert_eq!(layout["measurement"], "layout");
        let mut theirs = snapshot(&out.join("layout").join("pilot").join("tree"));
        assert!(
            theirs.remove(Path::new(".spec-debt.toml")).is_some(),
            "{name}: layout writes a baseline"
        );
        let ours = snapshot(&run.tree());
        assert_eq!(
            ours.keys().collect::<Vec<_>>(),
            theirs.keys().collect::<Vec<_>>(),
            "{name}: the tree's files"
        );
        assert!(ours == theirs, "{name}: a tree file's bytes differ");
    }
    // The index outputs are part of it.
    let run = WRun::of(ONE, &scratch, &["--budget", "10000"]);
    for index in ["book/index.md", "book/index-a.md", "book/index-b.md"] {
        let text = fs::read_to_string(run.tree().join(index)).unwrap();
        assert!(text.contains("spec export index"), "{index}");
    }
}

fn run_layout(corpus: &Path, out: &Path) -> Output {
    run(&[
        "layout",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ])
}

// --------------------------------------------------------------- AC-05

/// AC-05: at a budget that outlines, the forms are the library bundle's
/// targets layer; the follow-ups are its outlined and header-only items
/// (as the targets' REFs, once each) and their bytes are the test's own
/// `show` + `render_text` (M3: from `tokens_est` → red); `incomplete` 1;
/// W_after_followups = W_after + them.
#[test]
fn forms_are_the_targets_layer_and_followups_are_show_bytes() {
    let scratch = Scratch::new("w-followups");
    let run = WRun::at_budget(ONE, &scratch, OUTLINE_BUDGET);
    let home = scratch.join("library-home");
    let result = run.result().clone();
    let mut totals: BTreeMap<&str, u64> = BTreeMap::new();
    let mut followup_bytes = 0;
    let mut followup_refs = 0;
    let mut incomplete = 0;
    for task in run.tasks() {
        let context = format!("{}", task["task"]);
        assert_eq!(task["outcome"], "bundled", "{context}: none refused here");
        let bundle = library_bundle_of(&run.tree(), &home, &task, OUTLINE_BUDGET);
        let layer = targets_layer(&bundle);
        let forms: Vec<Value> = layer
            .iter()
            .map(|(name, path, line, form)| {
                json!({"name": name, "path": path, "line": line, "form": form_word(*form)})
            })
            .collect();
        assert_eq!(task["bundle"]["forms"], json!(forms), "{context}: forms");
        for (_, _, _, form) in &layer {
            let key = match form {
                ItemForm::Text => "text",
                ItemForm::Outline => "outline",
                ItemForm::Header | ItemForm::Summary => "header",
            };
            *totals.entry(key).or_default() += 1;
        }
        *totals.entry("not_included").or_default() += bundle.not_included() as u64;

        // The follow-ups: outlined and header-only items as the REFs of
        // the targets they are, once each, in print order.
        let targets = task["targets"].as_array().unwrap();
        let mut wanted: Vec<String> = Vec::new();
        for (name, path, line, form) in &layer {
            if !matches!(form, ItemForm::Outline | ItemForm::Header) {
                continue;
            }
            let reference = targets
                .iter()
                .find(|target| target["path"] == path.as_str() && target["line"] == *line)
                .map_or_else(
                    || name.clone(),
                    |target| target["reference"].as_str().unwrap().to_owned(),
                );
            if !wanted.contains(&reference) {
                wanted.push(reference);
            }
        }
        let recorded = task["followups"]["refs"].as_array().unwrap();
        let got: Vec<&str> = recorded
            .iter()
            .map(|entry| entry["reference"].as_str().unwrap())
            .collect();
        assert_eq!(got, wanted, "{context}: follow-up REFs");
        let mut bytes = 0;
        for entry in recorded {
            let reference = entry["reference"].as_str().unwrap();
            let (shown, truncated) = library_show(&run.tree(), &home, reference);
            assert_eq!(entry["bytes"], shown, "{context}: show {reference}");
            assert_eq!(entry["truncated"], truncated, "{context}: {reference}");
            bytes += shown as u64;
        }
        assert_eq!(task["followups"]["bytes"], bytes, "{context}");
        let w_after = task["w_after"].as_u64().unwrap();
        assert_eq!(task["w_after_followups"], w_after + bytes, "{context}");
        followup_bytes += bytes;
        followup_refs += recorded.len() as u64;
        if recorded.len() > 3 {
            incomplete += 1;
        }
    }
    for key in ["text", "outline", "header", "not_included"] {
        assert_eq!(
            count(&result, &format!("targets/{key}")),
            totals.get(key).copied().unwrap_or(0),
            "targets/{key}: {result}"
        );
    }
    assert!(count(&result, "targets/outline") > 0 && count(&result, "targets/header") > 0);
    assert_eq!(
        count(&result, "followups/bytes"),
        followup_bytes,
        "{result}"
    );
    assert_eq!(count(&result, "followups/refs"), followup_refs, "{result}");
    assert_eq!(
        count(&result, "followups/incomplete"),
        incomplete,
        "{result}"
    );
    assert_eq!(incomplete, 1, "{result}");
    assert_eq!(count(&result, "refused"), 0, "{result}");
}

/// AC-05: at a small budget the task with the most targets is refused by
/// the CLI's frame (its minimum); it stays a task (M1: statistics over the
/// bundled tasks only → red), its W_after the word `refused` and the max
/// (M2: refused → 0 → red), its follow-ups every REF of it, its
/// W_after_followups Tier 0 + Tier 1 + them.
#[test]
fn a_refused_task_stays_in_the_denominator() {
    let scratch = Scratch::new("w-refused");
    let run = WRun::at_budget(ONE, &scratch, REFUSING_BUDGET);
    let home = scratch.join("library-home");
    let result = run.result().clone();
    let tasks = run.tasks();
    assert_eq!(count(&result, "tasks"), 7, "{result}");
    assert_eq!(count(&result, "refused"), 1, "{result}");
    assert_eq!(count(&result, "failed"), 0, "{result}");
    let refused: Vec<&Value> = tasks
        .iter()
        .filter(|task| task["outcome"] == "refused")
        .collect();
    assert_eq!(refused.len(), 1);
    let task = refused[0];
    assert_eq!(task["source"], "book/needs.md");
    assert_eq!(task["w_after"], "refused");
    assert!(task["bundle"].is_null(), "{task}");
    let message = task["message"].as_str().unwrap();
    assert!(message.contains("below this bundle's minimum"), "{message}");

    // The CLI refuses it the same way through the test's own Env.
    match library_bundle(&run.tree(), &home, strings(&task["refs"]), REFUSING_BUDGET) {
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun);
            assert_eq!(error.message, message);
        }
        Ok(outcome) => panic!("the CLI bundled the refused task: {:?}", outcome.reason),
    }

    // Its follow-ups: every REF, each read with `show`.
    let refs = strings(&task["refs"]);
    let recorded: Vec<String> = task["followups"]["refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["reference"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(recorded, refs, "a refused task's follow-ups");
    let shown: u64 = refs
        .iter()
        .map(|reference| library_show(&run.tree(), &home, reference).0 as u64)
        .sum();
    let slots = after_slots(&run, task, tier0_of(&run.corpus).as_deref());
    assert_eq!(task["w_after_followups"], slots + shown);
    assert_eq!(task["followups"]["bytes"], shown);
    // Its moved records are follow-ups too: the gate record's `show` (the
    // record holding a link) is in `followups.bytes`.
    for reference in [
        "ASK-012", "ASK-013", "NEED-01", "NEED-02", "NEED-05", "NEED-40",
    ] {
        assert!(
            recorded.iter().any(|r| r == reference),
            "{reference}: {recorded:?}"
        );
    }
    let gate = task["followups"]["refs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["reference"] == "NEED-40")
        .expect("the gate record is a follow-up");
    let (gate_bytes, _) = library_show(&run.tree(), &home, "NEED-40");
    assert!(gate_bytes > 0);
    assert_eq!(gate["bytes"], gate_bytes, "{gate}");
    let total: u64 = tasks
        .iter()
        .map(|task| task["followups"]["bytes"].as_u64().unwrap())
        .sum();
    assert_eq!(count(&result, "followups/bytes"), total, "{result}");

    // The statistics over every task, the refused one ranked on top.
    let w_after = column(&tasks, "w_after");
    assert_eq!(result["w_after"], statistic(&w_after), "{result}");
    assert_eq!(result["w_after"]["max"], "refused");
    assert_eq!(result["worst"]["w_after"], task["task"]);
    assert_eq!(
        result["w_after_followups"],
        statistic(&column(&tasks, "w_after_followups"))
    );
    // M1's witness: over the bundled tasks only the median would differ.
    let bundled: Vec<Value> = w_after.iter().filter(|v| v.is_u64()).cloned().collect();
    assert_ne!(statistic(&bundled)["median"], result["w_after"]["median"]);
    assert!(count(&result, "followups/incomplete") >= 1, "{result}");
}

/// AC-05 risk (4): a follow-up `show` over the 40 000-character cap is
/// `truncated`, its bytes still the printed output's; a task whose own
/// title outgrows the ceiling is refused by the frame's ceiling.
#[test]
fn a_capped_show_is_truncated_and_a_frame_over_the_ceiling_is_refused() {
    let scratch = Scratch::new("w-truncated");
    let corpus = copy_of(ONE, &scratch, "corpus");
    let paragraph = "The huge page repeats one long sentence so that its text outgrows every cap. ";
    let mut huge = String::from("# Huge\n\n");
    for _ in 0..12 {
        huge.push_str(&paragraph.repeat(50));
        huge.push_str("\n\n");
    }
    assert!(huge.len() > 45_000);
    fs::write(corpus.join("book/huge.md"), &huge).unwrap();
    let title = "Long ".repeat(8_500);
    fs::write(
        corpus.join("book/long.md"),
        format!("# {title}\n\nA title past the ceiling.\n"),
    )
    .unwrap();
    let run = WRun::new(&corpus, &scratch, "out", &[]);
    let home = scratch.join("library-home");
    let huge_task = run.task("book/huge.md");
    assert_eq!(huge_task["outcome"], "bundled", "{huge_task}");
    let followups = huge_task["followups"]["refs"].as_array().unwrap();
    let entry = followups
        .iter()
        .find(|entry| entry["reference"] == "book/huge.md")
        .unwrap_or_else(|| panic!("the huge page is no follow-up: {huge_task}"));
    assert_eq!(entry["truncated"], true);
    let (shown, truncated) = library_show(&run.tree(), &home, "book/huge.md");
    assert!(truncated);
    assert_eq!(entry["bytes"], shown);
    assert!(count(run.result(), "followups/truncated") >= 1);

    let long_task = run.task("book/long.md");
    assert_eq!(long_task["outcome"], "refused", "{long_task}");
    let message = long_task["message"].as_str().unwrap();
    assert!(message.contains("-character ceiling"), "{message}");
    assert_eq!(count(run.result(), "refused"), 1);
}

// --------------------------------------------------------------- AC-06

/// AC-06: the aggregates are nearest rank over every task, recomputed here
/// from `tasks.json` — on `two`'s even count the median is v[n/2], not an
/// average (M1) — with `worst` the max's task, the fill and bytes per token
/// over the bundled tasks; and `two` equals its `expected.json`.
#[test]
fn aggregates_are_nearest_rank_over_every_task() {
    let scratch = Scratch::new("w-aggregates");
    for name in W_FIXTURES {
        let run = WRun::of(name, &scratch, &[]);
        let tasks = run.tasks();
        let result = run.result();
        let names: Vec<String> = tasks
            .iter()
            .map(|task| task["task"].as_str().unwrap().to_owned())
            .collect();
        for key in ["w_before", "w_after", "w_after_followups", "docs_needed"] {
            let values = column(&tasks, key);
            assert_eq!(result[key], statistic(&values), "{name}: {key}");
            if key != "docs_needed" {
                let (_, worst) = nearest_rank(&values, 100);
                assert_eq!(result["worst"][key], names[worst], "{name}: worst {key}");
            }
        }
        let bundled: Vec<&Value> = tasks
            .iter()
            .filter(|task| task["outcome"] == "bundled")
            .map(|task| &task["bundle"])
            .collect();
        let mut fills: Vec<f64> = bundled
            .iter()
            .map(|bundle| {
                tenths(
                    bundle["tokens"].as_u64().unwrap() * 100,
                    u64::from(DEFAULT_BUDGET),
                )
            })
            .collect();
        fills.sort_by(f64::total_cmp);
        let rank = |p: usize| fills[(p * fills.len()).div_ceil(100).max(1) - 1];
        assert_eq!(result["fill_percent"]["median"], rank(50), "{name}");
        assert_eq!(result["fill_percent"]["max"], rank(100), "{name}");
        let bytes: u64 = bundled.iter().map(|b| b["bytes"].as_u64().unwrap()).sum();
        let tokens: u64 = bundled.iter().map(|b| b["tokens"].as_u64().unwrap()).sum();
        assert_eq!(result["bytes_per_token"], tenths(bytes, tokens), "{name}");
        let refused = tasks.iter().filter(|t| t["outcome"] == "refused").count();
        let failed = tasks.iter().filter(|t| t["outcome"] == "failed").count();
        assert_eq!(count(result, "refused"), refused as u64, "{name}");
        assert_eq!(count(result, "failed"), failed as u64, "{name}");
        assert_eq!(
            count(result, "targets/refs"),
            tasks
                .iter()
                .map(|t| t["refs"].as_array().unwrap().len() as u64)
                .sum::<u64>(),
            "{name}"
        );
    }

    // `two`: an even count, and the averaged median would differ.
    let run = WRun::of(TWO, &scratch, &["--budget", "10000"]);
    let tasks = run.tasks();
    assert_eq!(tasks.len() % 2, 0, "two has an even task count");
    assert_eq!(without_ms(run.result()), {
        let mut expected = expected(TWO);
        expected["budget"]["source"] = json!("flag");
        expected
    });
    let mut values: Vec<u64> = column(&tasks, "w_before")
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    values.sort_unstable();
    let n = values.len();
    assert_eq!(run.result()["w_before"]["median"], values[n / 2 - 1]);
    assert_ne!(
        values[n / 2 - 1] * 2,
        values[n / 2 - 1] + values[n / 2],
        "the two middle values differ, so an average is visible"
    );
}

/// AC-06: stdout is the whitelist — counts, statistics (bytes or the word
/// `refused`/`failed`), `task-N` names, the budget's source — and carries
/// no path, ID, REF, glob or key of the corpus, the tree or the tasks
/// config (M2: a path on stdout → red).
#[test]
fn stdout_is_the_whitelist_and_names_no_path_id_glob_or_key() {
    let scratch = Scratch::new("w-whitelist");
    let runs = [
        WRun::of(ONE, &scratch, &[]),
        WRun::of(TWO, &scratch, &[]),
        WRun::at_budget(ONE, &scratch, REFUSING_BUDGET),
    ];
    let whitelist: BTreeSet<&str> = WHITELIST.into_iter().collect();
    for (index, run) in runs.iter().enumerate() {
        let top: BTreeSet<&str> = run
            .envelope
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            top,
            BTreeSet::from(["label", "measurement", "result", "versions", "wall_ms"]),
            "run {index}"
        );
        let mut found = Vec::new();
        leaves(run.result(), "", &mut found);
        let paths: BTreeSet<&str> = found.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, whitelist, "run {index}: exactly the whitelist");
        for (path, value) in &found {
            if path.starts_with("worst/") {
                let name = value.as_str().unwrap_or_default();
                assert!(
                    name.strip_prefix("task-")
                        .is_some_and(|n| n.parse::<u32>().is_ok()),
                    "run {index}: {path} = {value}"
                );
            } else if path == "budget/source" {
                assert!(value == "default" || value == "flag", "{value}");
            } else if path.starts_with("w_") {
                assert!(
                    value.is_u64() || value == "refused" || value == "failed",
                    "run {index}: {path} = {value}"
                );
            } else {
                assert!(value.is_number(), "run {index}: {path} = {value}");
            }
        }

        // No corpus string: the tasks' sources, after paths, REFs, target
        // and slot paths, the tree's files, the tasks config's strings.
        let mut strings_seen: BTreeSet<String> = BTreeSet::new();
        for task in run.tasks() {
            for key in ["source", "after"] {
                strings_seen.insert(task[key].as_str().unwrap().to_owned());
            }
            strings_seen.extend(strings(&task["refs"]));
            for target in task["targets"].as_array().unwrap() {
                strings_seen.insert(target["path"].as_str().unwrap().to_owned());
            }
            for entry in task["slots"].as_array().unwrap() {
                strings_seen.insert(entry["path"].as_str().unwrap().to_owned());
            }
        }
        for path in snapshot(&run.tree()).into_keys() {
            let path = path.to_string_lossy().replace('\\', "/");
            strings_seen.insert(path.rsplit('/').next().unwrap().to_owned());
            strings_seen.insert(path);
        }
        let tasks_text = fs::read_to_string(run.corpus.join("tasks.toml")).unwrap();
        strings_seen.extend(tasks_config_strings(&tasks_text));
        let stdout = run.stdout();
        let leaked: Vec<&String> = strings_seen
            .iter()
            .filter(|text| {
                let shaped = text.contains(['/', '.']) || text.chars().any(|c| c.is_ascii_digit());
                stdout.contains(&format!("\"{text}\""))
                    || (shaped && stdout.contains(text.as_str()))
            })
            .collect();
        assert!(
            leaked.is_empty(),
            "run {index}: corpus strings on stdout: {leaked:?}"
        );
        assert!(strings_seen.len() > 20, "run {index}");
        for leak in [".md", "/"] {
            let result = serde_json::to_string(run.result()).unwrap();
            assert!(
                !result.contains(leak),
                "run {index}: {leak:?} in the result"
            );
        }
    }
}

/// Every string of a tasks config: its globs, key, paths and the
/// `[tier1.paths]` keys (the area values).
fn tasks_config_strings(text: &str) -> BTreeSet<String> {
    let table: toml::Table = toml::from_str(text).expect("a tasks config");
    let mut out = BTreeSet::new();
    fn walk(value: &toml::Value, out: &mut BTreeSet<String>) {
        match value {
            toml::Value::String(text) => {
                out.insert(text.clone());
            }
            toml::Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            toml::Value::Table(table) => table.values().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    walk(&toml::Value::Table(table.clone()), &mut out);
    if let Some(paths) = table
        .get("tier1")
        .and_then(|tier1| tier1.get("paths"))
        .and_then(toml::Value::as_table)
    {
        out.extend(paths.keys().cloned());
    }
    out
}

// --------------------------------------------------------------- AC-07

/// AC-07: every bundle computed twice (the second pass in reverse order on
/// a fresh data directory) gives the same hash (`nondeterministic` 0); two
/// runs into different `--out` write the same `tasks.json` but `*_ms`, the
/// same bodies and the same tree (M: a `HashMap` for tasks or REFs → red).
#[test]
fn two_runs_give_identical_detail_but_timings() {
    let scratch = Scratch::new("w-twice");
    for name in W_FIXTURES {
        for extra in [&[][..], &["--budget", "250"][..]] {
            let first = WRun::new(
                &fixture_dir(name),
                &scratch,
                &format!("{}-a{}", name.replace('/', "-"), extra.join("")),
                extra,
            );
            let second = WRun::new(
                &fixture_dir(name),
                &scratch,
                &format!("{}-b{}", name.replace('/', "-"), extra.join("")),
                extra,
            );
            let context = format!("{name} {extra:?}");
            assert_eq!(count(first.result(), "nondeterministic"), 0, "{context}");
            assert_eq!(
                without_ms(first.result()),
                without_ms(second.result()),
                "{context}"
            );
            let one = read_json(&first.detail.join("tasks.json"));
            let two = read_json(&second.detail.join("tasks.json"));
            let strip = |value: &Value| -> Vec<Value> {
                value.as_array().unwrap().iter().map(without_ms).collect()
            };
            assert_eq!(strip(&one), strip(&two), "{context}: tasks.json but *_ms");
            assert!(
                snapshot(&first.detail.join("bundles")) == snapshot(&second.detail.join("bundles")),
                "{context}: the bodies"
            );
            assert!(
                snapshot(&first.tree()) == snapshot(&second.tree()),
                "{context}: the tree"
            );
            for task in first.tasks() {
                if task["outcome"] == "bundled" {
                    assert_eq!(
                        task["bundle_hash_2"], task["bundle"]["bundle_hash"],
                        "{context}"
                    );
                } else {
                    assert!(task["bundle_hash_2"].is_null(), "{context}");
                }
                for key in ["bundle_ms", "bundle_2_ms", "show_ms"] {
                    assert!(task.get(key).is_some(), "{context}: {key}");
                }
            }
        }
    }
}

// --------------------------------------------------------------- AC-08

/// Every file under `dir` whose name ends in `.db`, relative.
fn databases_under(dir: &Path) -> Vec<String> {
    snapshot(dir)
        .into_keys()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .filter(|path| path.ends_with(".db"))
        .collect()
}

/// AC-08: the tree, both data directories and every database land under
/// `<out>/w/<label>/`; a scratch `HOME` and `XDG_DATA_HOME` stay empty (M:
/// `Env::from_process` → the database under `HOME`: red); the corpus and
/// `fixtures/` are unchanged.
#[test]
fn everything_lands_under_out_and_home_stays_empty() {
    let scratch = Scratch::new("w-isolation");
    let corpus = copy_of(ONE, &scratch, "corpus");
    let home = scratch.join("home");
    let xdg = scratch.join("xdg");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&xdg).unwrap();
    let out = scratch.join("out");
    let corpus_before = snapshot(&corpus);
    let fixtures_before = git_status(&repository_root(), "fixtures/");
    let repository_before = git_status(&repository_root(), ".");
    let output = eval()
        .env("HOME", &home)
        .env("XDG_DATA_HOME", &xdg)
        .args([
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ])
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output);
    assert_eq!(without_ms(&envelope["result"]), expected(ONE));
    assert!(
        snapshot(&home).is_empty(),
        "HOME: {:?}",
        snapshot(&home).keys()
    );
    assert!(
        snapshot(&xdg).is_empty(),
        "XDG_DATA_HOME: {:?}",
        snapshot(&xdg).keys()
    );
    assert!(snapshot(&corpus) == corpus_before, "the corpus changed");
    assert_eq!(git_status(&repository_root(), "fixtures/"), fixtures_before);
    assert_eq!(
        git_status(&repository_root(), "."),
        repository_before,
        "the repository changed"
    );
    let databases = databases_under(&out);
    assert!(databases.len() >= 2, "{databases:?}");
    for database in &databases {
        assert!(
            database.starts_with("w/pilot/home-1/") || database.starts_with("w/pilot/home-2/"),
            "{database} outside the data directories"
        );
    }
    assert!(databases.iter().any(|d| d.starts_with("w/pilot/home-1/")));
    assert!(databases.iter().any(|d| d.starts_with("w/pilot/home-2/")));
    for path in snapshot(&out).into_keys() {
        assert!(
            path.starts_with("w/pilot"),
            "{} outside <out>/w/pilot",
            path.display()
        );
    }
    assert!(databases_under(&corpus).is_empty());
}

/// AC-08: the corpus under `--out` (the deadly case at `<out>/w/pilot`, the
/// directory a run empties) or `--out` under the corpus → exit 2, nothing
/// written or deleted.
#[test]
fn a_corpus_and_out_nested_either_way_are_refused_and_nothing_is_deleted() {
    let scratch = Scratch::new("w-nested");
    let out = scratch.join("o");
    let corpus = out.join("w").join("pilot");
    copy_dir(&fixture_dir(ONE), &corpus);
    let elsewhere = scratch.join("corpus");
    copy_dir(&fixture_dir(ONE), &elsewhere);
    let before = snapshot(&scratch.0);
    let inside = elsewhere.join("out");
    let cases: [(&str, &Path, &Path); 3] = [
        ("the corpus at <out>/w/pilot", &corpus, &out),
        ("--out under the corpus", &elsewhere, &inside),
        ("--out equal to the corpus", &elsewhere, &elsewhere),
    ];
    for (what, corpus, out) in cases {
        let output = run(&[
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ]);
        assert_eq!(output.status.code(), Some(2), "{what}: {}", stderr(&output));
        assert!(output.stdout.is_empty(), "{what}: stdout");
        assert!(!stderr(&output).contains("panicked"), "{what}");
        assert!(snapshot(&scratch.0) == before, "{what}: a byte changed");
    }
    assert!(!inside.exists());
}

/// AC-08: a symlinked `<out>/w` or `<out>/w/<label>` → exit 2; the link is
/// neither followed nor removed, its target byte for byte as before.
#[cfg(unix)]
#[test]
fn a_symlinked_w_directory_is_refused_and_neither_followed_nor_removed() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("w-symlinks");
    let victim = scratch.join("victim");
    fs::create_dir_all(victim.join("pilot")).unwrap();
    fs::write(victim.join("keep.txt"), "kept\n").unwrap();
    fs::write(victim.join("pilot").join("keep.txt"), "kept too\n").unwrap();
    let before = snapshot(&victim);
    let corpus = fixture_dir(ONE);
    for (index, link_at) in ["w", "w/pilot"].into_iter().enumerate() {
        let out = scratch.join(&format!("out-{index}"));
        let link = out.join(link_at);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        let target = if link_at == "w" {
            victim.clone()
        } else {
            victim.join("pilot")
        };
        symlink(&target, &link).unwrap();
        let output = run(&[
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{link_at}: {}",
            stderr(&output)
        );
        assert!(output.stdout.is_empty(), "{link_at}");
        assert!(
            fs::symlink_metadata(&link).is_ok_and(|meta| meta.file_type().is_symlink()),
            "{link_at}: the link was removed"
        );
        assert_eq!(snapshot(&victim), before, "{link_at}: the target changed");
    }
}

/// A before scheme without `[project] slug` is refused at its file and
/// line, nothing written; so is a corpus without its census config
/// (`layout`'s own refusal).
#[test]
fn a_scheme_without_a_slug_and_a_missing_census_config_are_refused() {
    let scratch = Scratch::new("w-slug");
    let corpus = copy_of(ONE, &scratch, "no-slug");
    edit(&corpus.join("specengine.toml"), "slug = \"notebook\"\n", "");
    let out = scratch.join("out-slug");
    let output = run(&[
        "w",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &out, "no slug");
    let scheme = corpus.join("specengine.toml");
    assert!(
        stderr(&output).contains(&format!("{}:", scheme.display())),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("nothing written"),
        "{}",
        stderr(&output)
    );

    let corpus = copy_of(ONE, &scratch, "no-census");
    fs::remove_file(corpus.join("census.toml")).unwrap();
    let out = scratch.join("out-census");
    let output = run(&[
        "w",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &out, "no census config");
}

/// The `--budget` bounds: 1…4294967295 run (`source` `flag`), 0, 2^32, a
/// negative and a non-number are refused (exit 2, nothing written); at 1
/// every task is refused and stays a task.
#[test]
fn the_budget_is_bounded_and_recorded() {
    let scratch = Scratch::new("w-budget");
    let corpus = fixture_dir(TWO);
    for (index, bad) in ["0", "4294967296", "-1", "ten"].into_iter().enumerate() {
        let out = scratch.join(&format!("out-{index}"));
        let output = run(&[
            "w",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--budget",
            bad,
            "--today",
            TODAY,
        ]);
        assert_refused(&output, &out, &format!("--budget {bad}"));
    }
    let top = WRun::at_budget(TWO, &scratch, u32::MAX);
    assert_eq!(
        top.result()["budget"],
        json!({"tokens": u32::MAX, "source": "flag"})
    );
    assert_eq!(count(top.result(), "tasks"), 6);
    let bottom = WRun::at_budget(TWO, &scratch, 1);
    assert_eq!(
        bottom.result()["budget"],
        json!({"tokens": 1, "source": "flag"})
    );
    assert_eq!(count(bottom.result(), "refused"), 6, "{}", bottom.result());
    assert_eq!(count(bottom.result(), "tasks"), 6);
    assert_eq!(bottom.result()["w_after"]["median"], "refused");
    assert_eq!(bottom.result()["bytes_per_token"], 0.0);
}

// --------------------------------------------------- AC-10 (owner-run)

/// AC-10 plumbing: the `#[ignore]` pilot runs of `w` (owner-run, one pilot
/// at a time; the corpus, scheme, census config and tasks config from the
/// label's variables, all outside the repository, the corpus read-only),
/// and the same helper on an invented setup so the plumbing itself runs.
#[cfg(unix)]
mod pilots {
    use super::*;
    use std::process::Command;

    fn tasks_variable(label: &str) -> &'static str {
        match label {
            "pilot-a" => "SPECENGINE_TASKS_A",
            "pilot-b" => "SPECENGINE_TASKS_B",
            other => panic!("{other:?} is not a pilot label"),
        }
    }

    struct PilotInputs<'a> {
        corpus: &'a Path,
        scheme: &'a Path,
        config: &'a Path,
        tasks: &'a Path,
    }

    /// `w --label <label> --out <scratch>/out --timeout 3600` with only
    /// `PATH`, `HOME` = the empty `<scratch>/home` (also the working
    /// directory) and the label's four variables; the read-only proof over
    /// the scheme's and the census config's roots equal before and after;
    /// exit 0; `HOME` still empty; then the AC-10 invariants: a result
    /// within the timeout, the whitelist, no path on stdout,
    /// `nondeterministic` 0, each failed task printed (A6), everything
    /// under `<out>/w/<label>`.
    fn w_read_only(label: &str, scratch: &Path, inputs: &PilotInputs) -> Value {
        let variables = pilot::variables(label);
        let mut roots = pilot::scheme_roots(inputs.scheme, variables.scheme);
        roots.extend(pilot::census_roots(inputs.config, variables.config));
        roots.sort();
        roots.dedup();
        let home = scratch.join("home");
        let out = scratch.join("out");
        fs::create_dir_all(&home).unwrap();
        assert!(snapshot(&home).is_empty() && !out.exists());
        let before = pilot::proof(inputs.corpus, &roots);
        let mut command = Command::new(BIN);
        command.env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command
            .current_dir(&home)
            .env("HOME", &home)
            .env(variables.corpus, inputs.corpus)
            .env(variables.scheme, inputs.scheme)
            .env(variables.config, inputs.config)
            .env(tasks_variable(label), inputs.tasks);
        let output = command
            .args([
                "w",
                "--label",
                label,
                "--out",
                out.to_str().unwrap(),
                "--timeout",
                "3600",
            ])
            .output()
            .expect("specengine-eval runs");
        let after = pilot::proof(inputs.corpus, &roots);
        assert_eq!(
            output.status.code(),
            Some(0),
            "w --label {label}: {}",
            stderr(&output)
        );
        assert!(
            before == after,
            "w --label {label}: the read-only proof changed"
        );
        assert!(
            snapshot(&home).is_empty(),
            "w --label {label}: HOME written"
        );
        let envelope = envelope(&output);
        assert_eq!(envelope["measurement"], "w");
        assert_eq!(envelope["label"], label);
        let result = &envelope["result"];
        assert!(result.is_object(), "{label}: within --timeout: {result}");
        let whitelist: BTreeSet<&str> = WHITELIST.into_iter().collect();
        let mut found = Vec::new();
        leaves(result, "", &mut found);
        for (path, _) in &found {
            assert!(whitelist.contains(path.as_str()), "{label}: {path}");
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for leak in [".md", "/", "\\"] {
            assert!(!stdout.contains(leak), "{label}: {leak:?} on stdout");
        }
        assert_eq!(count(result, "nondeterministic"), 0, "{label}: {result}");
        let detail = out.join("w").join(label);
        let tasks = read_json(&detail.join("tasks.json"));
        for task in tasks.as_array().unwrap() {
            if task["outcome"] == "failed" {
                eprintln!("{label}: failed {}: {}", task["task"], task["message"]);
            }
        }
        for path in snapshot(&out).into_keys() {
            assert!(
                path.starts_with(Path::new("w").join(label)),
                "{}",
                path.display()
            );
        }
        eprintln!("{label} result: {result}");
        envelope
    }

    fn pilot_from_environment(label: &str) {
        let variables = pilot::variables(label);
        let canonical = |variable: &str, what: &str| {
            fs::canonicalize(pilot::required(variable, what))
                .unwrap_or_else(|error| panic!("{variable}: {error}"))
        };
        let corpus = canonical(variables.corpus, "the pilot corpus");
        let scheme = canonical(variables.scheme, "the pilot scheme");
        let config = canonical(
            variables.config,
            "the pilot census config with its [layout]",
        );
        let tasks = canonical(tasks_variable(label), "the pilot tasks config");
        for (variable, path) in [
            (variables.scheme, &scheme),
            (variables.config, &config),
            (tasks_variable(label), &tasks),
        ] {
            assert!(
                !path.starts_with(repository_root()),
                "{variable}: a pilot config lives outside the repository"
            );
        }
        let scratch = Scratch::new(label);
        assert!(!scratch.0.starts_with(&corpus));
        w_read_only(
            label,
            &scratch.0,
            &PilotInputs {
                corpus: &corpus,
                scheme: &scheme,
                config: &config,
                tasks: &tasks,
            },
        );
    }

    #[test]
    #[ignore = "needs SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CENSUS_CONFIG_A, SPECENGINE_TASKS_A; read-only, owner-run"]
    fn pilot_a_w_read_only() {
        pilot_from_environment("pilot-a");
    }

    #[test]
    #[ignore = "needs SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CENSUS_CONFIG_B, SPECENGINE_TASKS_B; read-only, owner-run"]
    fn pilot_b_w_read_only() {
        pilot_from_environment("pilot-b");
    }

    /// The helper on an invented setup: a `git init`ed copy of `one` with
    /// its three configs moved out of it (only the variables name them);
    /// `result` = its `expected.json`.
    #[test]
    fn the_pilot_helper_runs_w_on_an_invented_setup_read_only() {
        for label in ["pilot-a", "pilot-b"] {
            let scratch = Scratch::new("w-pilot-setup");
            let corpus = copy_of(ONE, &scratch, "corpus");
            let scheme = scratch.join("scheme-pilot.toml");
            let config = scratch.join("census-pilot.toml");
            let tasks = scratch.join("tasks-pilot.toml");
            fs::rename(corpus.join("specengine.toml"), &scheme).unwrap();
            fs::rename(corpus.join("census.toml"), &config).unwrap();
            fs::rename(corpus.join("tasks.toml"), &tasks).unwrap();
            let init = Command::new("git")
                .current_dir(&corpus)
                .args(["init", "-q"])
                .output()
                .expect("git runs");
            assert!(init.status.success(), "git init: {}", stderr(&init));
            let envelope = w_read_only(
                label,
                &scratch.0,
                &PilotInputs {
                    corpus: &corpus,
                    scheme: &scheme,
                    config: &config,
                    tasks: &tasks,
                },
            );
            assert_eq!(
                without_ms(&envelope["result"]),
                expected(ONE),
                "{label}: the configs came from the variables"
            );
        }
    }
}
