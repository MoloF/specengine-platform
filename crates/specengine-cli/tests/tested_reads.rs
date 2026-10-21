//! docs/features/tested-reads.md (8b-2): the reads an agent works from
//! show each node's `tested` state, 8b-1's plain-verify entries
//! (docs/canon/spec-lock-tested.md), computed by the one function `spec
//! verify` calls. RD-1 (agreement), RD-2 (degrade), RD-3 (plain `show`),
//! RD-4 (evidence-free reads unchanged), TE-2 (determinism) and TE-3 (the
//! cost, a record). RD-5 to RD-7 and RD-9: `show_bindings.rs`,
//! `bundle_bindings.rs`, `package.rs`, `impact.rs`; RD-8 and TE-1's MCP
//! half: `specengine-mcp`'s `mcp_read.rs`, `mcp_tasks.rs`,
//! `mcp_impact.rs`; TE-5: `specengine-http`'s `daemon_keys.rs`; TE-7:
//! `tested_reads_pilot.rs`.
//!
//! **The oracle is plain `spec verify --json`** (Q: "every read's entries
//! equal plain `spec verify --json`'s for that node, key for key"), with
//! `verify --node` for a leaf's list and `verify --task T` for the
//! package's. A read's set is checked against the parser
//! (`bindings_common::holder_set`: a node's ID and its sections, each as
//! the REF verify takes); the line is `tested_common::entry_line`
//! (`tested | <state>[ <category>] | <node>`).
//!
//! The corpus: `tested_kiln::evidence_kiln`, a scratch copy of
//! `fixtures/verify-tests` (8a's kiln, its fake runner committed, never
//! run here) with KILN-RAMP's and GLZ-01's markers pinned at `@1`, GLZ-01
//! naming KILN-HOLD (a layer-4 candidate of its bundle), and a format-3
//! lock holding the four blocks a passing run writes (built from "Data",
//! never from a command's output) plus one of a node no file holds;
//! `tested_kiln::EvidenceState` drives it through `verified`, `stale
//! spec_changed`, `stale code_changed` and `unverifiable spec_ahead`.
//! Each test names the mutation that turns it red (`M:`). Every
//! repository has its own `HOME`. Non-Latin characters in Rust sources
//! are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bindings_common;
mod code_index_common;
mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bindings_common::{
    Mode, Read, first_object_keys, holder_set, list, read_program, read_with, reference_of,
    run_with, text, vars_of, with_var,
};
use common::{copy_dir, repository_root, snapshot};
use lock_common::LockRepo;
use serde_json::{Value, json};
use tested_common::{
    TESTED_KEYS, entries_by_node, entry_line, plain, read_tested_lines, without_blocks,
    without_read_tested,
};
use tested_kiln::{EVIDENCE_STATES, EvidenceState, GONE, evidence_kiln, pin_and_mention};
use verify_tests_common::{COOL, FIXTURE, Kiln, RAMP, RUNNER, SCHEDULE, SLUG};

type Vars = Vec<(OsString, OsString)>;

// ------------------------------------------------------------------ reads

/// The shown REFs: the document (its three sections), two sections, a
/// record, its alias, the feature document and its criterion.
const SHOWN: [&str; 7] = [
    "KILN-FIRING",
    "KILN-RAMP",
    "KILN-HOLD",
    "CONE-01",
    "PYRO-01",
    "docs/features/glaze-check.md",
    "glaze-check/GLZ-01",
];

/// The leaves: a node without sections, its show list `verify --node`'s.
const LEAVES: [&str; 4] = ["KILN-RAMP", "KILN-HOLD", "CONE-01", "glaze-check/GLZ-01"];

/// The bundles' REFs: KILN-HOLD (GLZ-01 its placed criterion), the
/// document, a record with a section beside it.
const BUNDLED: [&[&str]; 3] = [&["KILN-HOLD"], &["KILN-FIRING"], &["CONE-01", "KILN-RAMP"]];

/// The impact REFs.
/// The impact forms ("Per read": REF, `--symbol` and `--since` alike).
const IMPACTED: [&[&str]; 5] = [
    &["KILN-RAMP"],
    &["KILN-FIRING"],
    &["glaze-check/GLZ-01"],
    &["--symbol", "crates/kiln::ramp::next_temperature"],
    &["--since", "HEAD"],
];

/// `spec show REF --bindings`, text and `--json`.
fn show(cwd: &Path, reference: &str, vars: &Vars) -> Read {
    read_with(cwd, &["show", reference, "--bindings"], vars)
}

/// `spec bundle <refs> --budget 10000`, text and `--json`.
fn bundle(cwd: &Path, refs: &[&str], vars: &Vars) -> Read {
    bindings_common::bundle_in(cwd, refs, "10000", vars)
}

/// `spec task show <task>`: the brief, then the package.
fn task_show(cwd: &Path, task: &str, vars: &Vars) -> Read {
    bindings_common::task_show_in(cwd, task, vars)
}

/// `spec impact <args>`, text and `--json`.
fn impact(cwd: &Path, args: &[&str], vars: &Vars) -> Read {
    let mut all = vec!["impact"];
    all.extend_from_slice(args);
    read_with(cwd, &all, vars)
}

/// `spec verify <args> --json` from `cwd`: its document.
fn verify_json(cwd: &Path, args: &[&str], vars: &Vars) -> Value {
    let mut args: Vec<&str> = [&["verify"][..], args].concat();
    args.push("--json");
    let run = run_with(cwd, &args, vars);
    common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("spec {args:?}: {problem}\n{}", run.show()))
}

/// A shown node's set as verify names it: its ID, then its sections, each
/// as the REF `verify --node` takes (`<feature>/<ID>` when scoped).
fn shown_set(cwd: &Path, node: &Value) -> BTreeSet<String> {
    let path = text(&node["path"]);
    let mut set: BTreeSet<String> = node["id"]
        .as_str()
        .map(|id| reference_of(cwd, path, id))
        .into_iter()
        .collect();
    set.extend(
        list(&node["sections"])
            .iter()
            .map(|id| reference_of(cwd, path, text(id))),
    );
    set
}

/// Verify's entries of `set`'s nodes, by node bytes.
fn entries_of(by_node: &BTreeMap<String, Value>, set: &BTreeSet<String>) -> Vec<Value> {
    by_node
        .iter()
        .filter(|(node, _)| set.contains(*node))
        .map(|(_, entry)| entry.clone())
        .collect()
}

/// The `(state, category)` words of `entries`.
fn words(entries: &[Value]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| {
            let line = entry_line(entry);
            line.rsplit_once(" | ").expect("a line").0["tested | ".len()..].to_owned()
        })
        .collect()
}

/// The raw key order of the first `tested` entry printed, if any.
fn assert_entry_keys(read: &Read, context: &str) {
    if let Some(keys) = first_object_keys(&read.json_run.stdout, "tested") {
        assert_eq!(keys, TESTED_KEYS, "{context}: an entry's keys, in order");
    }
}

/// RD-1's show half: each shown node's `bindings` is `{rows, omitted,
/// tested}`, its `tested` plain verify's entries of its set (its ID and
/// its sections) by node bytes, key for key; the text prints each as `
/// tested | …` right after the node's closing `  bindings` line, in node
/// order. The entries shown.
fn assert_show_agrees(
    cwd: &Path,
    reference: &str,
    vars: &Vars,
    by_node: &BTreeMap<String, Value>,
    context: &str,
) -> Vec<Value> {
    let read = show(cwd, reference, vars);
    read.answered();
    let context = format!("{context}: show {reference} --bindings");
    let mut printed: Vec<String> = Vec::new();
    let mut shown: Vec<Value> = Vec::new();
    for node in list(&read.json()["nodes"]) {
        let bindings = &node["bindings"];
        let keys: BTreeSet<&str> = bindings
            .as_object()
            .unwrap_or_else(|| panic!("{context}: `bindings` an object\n{}", read.show()))
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["omitted", "rows", "tested"]),
            "{context}"
        );
        let want = entries_of(by_node, &shown_set(cwd, &node));
        assert_eq!(
            list(&bindings["tested"]),
            want,
            "{context}: {}'s entries are plain verify's\n{}",
            text(&node["id"]),
            read.show()
        );
        printed.extend(want.iter().map(|entry| format!("  {}", entry_line(entry))));
        shown.extend(want);
    }
    let lines: Vec<&str> = read.text.stdout.lines().collect();
    let tested: Vec<String> = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("tested | "))
        .map(|line| (*line).to_owned())
        .collect();
    assert_eq!(tested, printed, "{context}: the lines\n{}", read.show());
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("  tested | ") {
            let above = lines[at - 1];
            assert!(
                above.starts_with("  bindings ") || above.starts_with("  tested | "),
                "{context}: {line:?} after the closing line, not after {above:?}"
            );
        }
    }
    assert_entry_keys(&read, &context);
    shown
}

/// RD-1's bundle half: the body's `tested | ` lines are the line function
/// over plain verify's entries of the targets' set and of every placed
/// criterion, each right under a header line (or another tested line);
/// the JSON keeps its 13 keys, no `tested` key anywhere. The entries'
/// lines.
fn assert_bundle_agrees(
    cwd: &Path,
    refs: &[&str],
    vars: &Vars,
    by_node: &BTreeMap<String, Value>,
    context: &str,
) -> Vec<String> {
    let read = bundle(cwd, refs, vars);
    read.answered();
    let context = format!("{context}: bundle {refs:?}");
    let document = read.json();
    let mut set: BTreeSet<String> = holder_set(cwd, refs, vars).into_iter().collect();
    for criterion in read.layer("criteria") {
        set.insert(reference_of(
            cwd,
            text(&criterion["path"]),
            text(&criterion["name"]),
        ));
    }
    let mut want: Vec<String> = entries_of(by_node, &set).iter().map(entry_line).collect();
    let body = read.body();
    let mut got = read_tested_lines(&body);
    want.sort();
    got.sort();
    assert_eq!(got, want, "{context}: the body's lines\n{}", read.show());
    let lines: Vec<&str> = body.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("tested | ") {
            let above = lines[at - 1];
            assert!(
                above.starts_with("tested | ")
                    || (above.contains(" | ") && !above.starts_with("  ")),
                "{context}: {line:?} under a header, not under {above:?}"
            );
        }
    }
    let keys: Vec<String> = document
        .as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys.len(), 13, "{context}: the bundle's 13 keys: {keys:?}");
    assert!(
        !read.json_run.stdout.contains("\"tested\":"),
        "{context}: no `tested` key in the bundle's JSON"
    );
    want
}

/// RD-1's package half: `tested`, the 27th key, is `spec verify --task T
/// --json`'s, each entry plain verify's; the brief prints each entry's
/// line. The entries.
fn assert_package_agrees(
    cwd: &Path,
    task: &str,
    vars: &Vars,
    by_node: &BTreeMap<String, Value>,
    context: &str,
) -> Vec<Value> {
    let read = task_show(cwd, task, vars);
    read.answered();
    let context = format!("{context}: task show {task}");
    let package = read.json();
    let want = verify_json(cwd, &["--task", task], vars)["tested"].clone();
    assert!(want.is_array(), "{context}: premise: verify --task judged");
    assert_eq!(package["tested"], want, "{context}: verify --task's");
    for entry in list(&package["tested"]) {
        assert_eq!(
            Some(&entry),
            by_node.get(text(&entry["node"])),
            "{context}: plain verify's entry"
        );
    }
    assert_eq!(
        read_tested_lines(&read.text.stdout),
        list(&want).iter().map(entry_line).collect::<Vec<_>>(),
        "{context}: the brief's lines\n{}",
        read.show()
    );
    assert_eq!(package["schema_version"], json!(1), "{context}");
    assert_entry_keys(&read, &context);
    list(&want)
}

/// RD-1's impact half (a REF, `--symbol`, `--since HEAD` over the
/// worktree's edits): `tested` is plain verify's entries of the reached
/// nodes and the nodes within them, by node bytes, uncut here; the text
/// prints each line after the `node` lines. The entries.
fn assert_impact_agrees(
    cwd: &Path,
    args: &[&str],
    vars: &Vars,
    by_node: &BTreeMap<String, Value>,
    context: &str,
) -> Vec<Value> {
    let read = impact(cwd, args, vars);
    read.answered();
    let context = format!("{context}: impact {args:?}");
    let document = read.json();
    let mut set: BTreeSet<String> = BTreeSet::new();
    for node in list(&document["nodes"]) {
        let id = reference_of(cwd, text(&node["path"]), text(&node["id"]));
        set.extend(holder_set(cwd, &[id.as_str()], vars));
    }
    let want = entries_of(by_node, &set);
    assert_eq!(
        list(&document["tested"]),
        want,
        "{context}: the reached nodes' entries\n{}",
        read.show()
    );
    assert_eq!(document["truncated"], Value::Null, "{context}: uncut");
    let lines: Vec<&str> = read.text.stdout.lines().collect();
    let tested: Vec<String> = lines
        .iter()
        .filter(|line| line.starts_with("tested | "))
        .map(|line| (*line).to_owned())
        .collect();
    assert_eq!(
        tested,
        want.iter().map(entry_line).collect::<Vec<_>>(),
        "{context}: the lines\n{}",
        read.show()
    );
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("tested | ") {
            let above = lines[at - 1];
            assert!(
                above.starts_with("node ") || above.starts_with("tested | "),
                "{context}: {line:?} after the node lines, not after {above:?}"
            );
        }
    }
    assert_entry_keys(&read, &context);
    want
}

// ------------------------------------------------------------------- RD-1

/// RD-1 **M1** (a read judging on its own: the cut lock, its own tree, or
/// no lockability): over the evidence kiln in each [`EvidenceState`]
/// (`verified`, `stale spec_changed`, `stale code_changed`, `unverifiable
/// spec_ahead`, then one of each, GLZ-01 in every one), every entry of
/// `show --bindings`, `task show` and `impact` equals plain `spec verify
/// --json`'s entry for that node, key for key, over the read's set; a
/// leaf's show list equals `verify --node --json`'s; the package's list
/// equals `verify --task T --json`'s; the bundle's lines are the line
/// function over those entries. `node_gone` never shows in a read (a read's
/// set holds only present nodes). Each read shows every state word once.
#[test]
fn read_and_verify_agree_on_tested() {
    let kiln = evidence_kiln("tr-agree");
    let task = kiln.new_task(&["KILN-RAMP"]);
    kiln.plan_task(&task, &["glaze-check/GLZ-01"], &["KILN-HOLD"]);
    let root = kiln.root().to_path_buf();
    let vars = vars_of(&kiln.repo);
    let mut seen: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut glaze: BTreeSet<&str> = BTreeSet::new();
    for state in EVIDENCE_STATES {
        state.write(&kiln);
        let context = format!("{state:?}");
        let verify = plain(&kiln.repo, &[]);
        for (node, word, category) in state.aimed() {
            assert_eq!(
                tested_common::state_of(&verify, node),
                (word.to_owned(), category.unwrap_or_default().to_owned()),
                "{context}: premise: plain verify's {node}\n{}",
                verify.text.show()
            );
        }
        let by_node = entries_by_node(&verify);
        let present: BTreeMap<String, Value> = by_node
            .iter()
            .filter(|(node, _)| node.as_str() != GONE)
            .map(|(node, entry)| (node.clone(), entry.clone()))
            .collect();

        for reference in SHOWN {
            let shown = assert_show_agrees(&root, reference, &vars, &by_node, &context);
            seen.entry("show").or_default().extend(words(&shown));
            if shown
                .iter()
                .any(|entry| entry["node"] == "glaze-check/GLZ-01")
            {
                glaze.insert("show");
            }
        }
        for leaf in LEAVES {
            let read = show(&root, leaf, &vars);
            assert_eq!(
                read.json()["nodes"][0]["bindings"]["tested"],
                verify_json(&root, &["--node", leaf], &vars)["tested"],
                "{context}: show {leaf} --bindings lists verify --node's"
            );
        }
        for refs in BUNDLED {
            let lines = assert_bundle_agrees(&root, refs, &vars, &present, &context);
            seen.entry("bundle").or_default().extend(
                lines
                    .iter()
                    .map(|line| line.rsplit_once(" | ").expect("a line").0[9..].to_owned()),
            );
            if lines
                .iter()
                .any(|line| line.ends_with("| glaze-check/GLZ-01"))
            {
                glaze.insert("bundle");
            }
        }
        let packaged = assert_package_agrees(&root, &task, &vars, &by_node, &context);
        let nodes: Vec<&str> = packaged.iter().map(|entry| text(&entry["node"])).collect();
        assert_eq!(
            nodes,
            ["KILN-HOLD", "KILN-RAMP", "glaze-check/GLZ-01"],
            "{context}: the task's set: its target, criterion and affected node"
        );
        seen.entry("package").or_default().extend(words(&packaged));
        glaze.insert("package");
        for args in IMPACTED {
            let reached = assert_impact_agrees(&root, args, &vars, &by_node, &context);
            seen.entry("impact").or_default().extend(words(&reached));
            if reached
                .iter()
                .any(|entry| entry["node"] == "glaze-check/GLZ-01")
            {
                glaze.insert("impact");
            }
        }
        state.undo(&kiln);
    }
    let every: BTreeSet<String> = [
        "verified",
        "stale spec_changed",
        "stale code_changed",
        "unverifiable spec_ahead",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for (read, words) in &seen {
        assert!(
            every.is_subset(words),
            "{read} showed every state: {words:?}"
        );
        assert!(!words.contains("node_gone"), "{read}: never node_gone");
    }
    assert_eq!(
        glaze,
        BTreeSet::from(["bundle", "impact", "package", "show"]),
        "GLZ-01 in every read"
    );
}

// ------------------------------------------------------------------- RD-2

/// An edit of the config's text.
type ConfigEdit = fn(&str) -> String;

/// The kiln's `[code]` roots line.
const ROOTS: &str = "roots = [\"crates\"]";

/// A `git` that fails every call, first on `PATH`.
fn failing_git(dir: &Path) -> OsString {
    shim(
        dir,
        "#!/bin/sh\necho 'fatal: refused by the test' >&2\nexit 128\n",
    );
    dir.as_os_str().to_owned()
}

/// `body` as an executable `git` in `dir`.
fn shim(dir: &Path, body: &str) {
    fs::create_dir_all(dir).expect("the shim's directory");
    let path = dir.join("git");
    fs::write(&path, body).expect("the shim");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
}

/// `repo`'s sandbox git, its object session (`cat-file`) ending with exit
/// 1: the session's end fails, and only that.
fn session_end_failing_git(repo: &LockRepo, dir: &Path) -> OsString {
    let real = repo.git.git_program().display().to_string();
    shim(
        dir,
        &format!(
            "#!/bin/sh\ncase \" $* \" in *\" cat-file \"*) '{real}' \"$@\"; exit 1;; esac\nexec '{real}' \"$@\"\n"
        ),
    );
    repo.git.path_with(dir)
}

/// `repo`'s sandbox git refusing `hash-object` (exit 128).
fn hash_refusing_git(repo: &LockRepo, dir: &Path) -> OsString {
    let real = repo.git.git_program().display().to_string();
    shim(
        dir,
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = hash-object ]; then\n    echo 'fatal: hash-object refused by the test' >&2\n    exit 128\n  fi\ndone\nexec '{real}' \"$@\"\n"
        ),
    );
    repo.git.path_with(dir)
}

/// One evidence kiln and its twin: the same commits but a lock without
/// blocks (format 2, no entry), the same task on each.
struct Twins {
    with: Kiln,
    without: Kiln,
    task: String,
}

impl Twins {
    fn new(label: &str) -> Self {
        let with = evidence_kiln(&format!("{label}-with"));
        let without = evidence_kiln(&format!("{label}-without"));
        without
            .repo
            .write("spec.lock", without_blocks(&without.repo.lock_text()));
        without.repo.commit_all("no evidence");
        let task = with.new_task(&["KILN-RAMP"]);
        with.plan_task(&task, &["glaze-check/GLZ-01"], &["KILN-HOLD"]);
        assert_eq!(without.new_task(&["KILN-RAMP"]), task, "the same task");
        without.plan_task(&task, &["glaze-check/GLZ-01"], &["KILN-HOLD"]);
        Self {
            with,
            without,
            task,
        }
    }

    /// Both kilns.
    fn both(&self) -> [&Kiln; 2] {
        [&self.with, &self.without]
    }
}

/// The reads RD-2 degrades, by name: `(name, args)`.
fn degraded_reads(task: &str) -> Vec<(&'static str, Vec<String>)> {
    let owned =
        |args: &[&str]| -> Vec<String> { args.iter().map(|arg| (*arg).to_owned()).collect() };
    vec![
        (
            "show document",
            owned(&["show", "KILN-FIRING", "--bindings"]),
        ),
        ("show leaf", owned(&["show", "KILN-RAMP", "--bindings"])),
        (
            "show criterion",
            owned(&["show", "glaze-check/GLZ-01", "--bindings"]),
        ),
        (
            "bundle",
            owned(&["bundle", "KILN-HOLD", "--budget", "10000"]),
        ),
        ("task show", owned(&["task", "show", task])),
        ("impact", owned(&["impact", "KILN-RAMP"])),
    ]
}

/// Every `tested` value of a read's document: each shown node's
/// `bindings.tested` (when `bindings` is an object), the package's and
/// impact's top-level one.
fn tested_values(name: &str, document: &Value) -> Vec<Value> {
    match name {
        "bundle" => Vec::new(),
        "task show" | "impact" => vec![document["tested"].clone()],
        _ => list(&document["nodes"])
            .iter()
            .filter(|node| node["bindings"].is_object())
            .map(|node| node["bindings"]["tested"].clone())
            .collect(),
    }
}

/// RD-2's one read under one cause, `(read, its scratch)` with its twin's: no
/// `verified` anywhere (text, JSON), no `tested | ` line, every `tested`
/// `null`; no `tested:` note unless `hash` (then exactly one, on stderr and
/// in the JSON's notes); and every byte the evidence-free twin's under the
/// same cause (the `tested:` note aside, each twin's scratch directory
/// masked where a message names a path): exit, rows, spec part. The read's
/// exit.
fn assert_degraded_read(
    with: (&Read, &Path),
    without: (&Read, &Path),
    name: &str,
    hash: bool,
    context: &str,
) -> i32 {
    let ((with, with_root), (without, without_root)) = (with, without);
    let context = format!("{context}: {name}");
    for printed in [&with.text.stdout, &with.json_run.stdout] {
        assert!(
            !printed.contains("tested | ") && !printed.contains("\"state\":\"verified\""),
            "{context}: no evidence printed\n{}",
            with.show()
        );
    }
    let notes: Vec<&str> = with
        .text
        .stderr
        .lines()
        .filter(|line| line.starts_with("note: tested: "))
        .collect();
    assert_eq!(
        notes.len(),
        usize::from(hash),
        "{context}: a `tested:` note only for the hash failure\n{}",
        with.show()
    );
    let document = common::one_json_document(&with.json_run.stdout).ok();
    if let Some(document) = &document {
        for value in tested_values(name, document) {
            assert_eq!(
                value,
                Value::Null,
                "{context}: `tested` null\n{}",
                with.show()
            );
        }
        if hash {
            let all_notes = list(&document["notes"]);
            let json_notes: Vec<&str> = all_notes
                .iter()
                .map(text)
                .filter(|note| note.starts_with("tested: "))
                .collect();
            assert_eq!(
                json_notes.len(),
                1,
                "{context}: the note in the JSON too\n{}",
                with.show()
            );
            assert_eq!(format!("note: {}", json_notes[0]), notes[0]);
        }
    }
    let strip = |read: &Read| -> (i32, String, String, String) {
        let note = |line: &&str| !line.starts_with("note: tested: ");
        let mut json = read.json_run.stdout.clone();
        if hash {
            // Not judged here, judged with no block in the twin.
            json = json.replace(",\"tested\":null", ",\"tested\":[]");
        }
        if let Some(line) = read
            .text
            .stderr
            .lines()
            .find(|line| line.starts_with("note: tested: "))
        {
            let quoted = serde_json::to_string(&line["note: ".len()..]).expect("a string");
            for form in [format!(",{quoted}"), format!("{quoted},"), quoted.clone()] {
                if json.contains(&form) {
                    json = json.replacen(&form, "", 1);
                    break;
                }
            }
        }
        (
            read.code(),
            read.text.stdout.clone(),
            json,
            read.text
                .stderr
                .lines()
                .filter(note)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };
    // A message naming a path names each twin's own scratch directory (no
    // worktree; a task of another repository).
    let masked = |(code, text, json, stderr): (i32, String, String, String), root: &Path| {
        let mut out = (code, text, json, stderr);
        let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        for root in [canonical.display().to_string(), root.display().to_string()] {
            out.1 = out.1.replace(&root, "<scratch>");
            out.2 = out.2.replace(&root, "<scratch>");
            out.3 = out.3.replace(&root, "<scratch>");
        }
        out
    };
    let got = masked(strip(with), with_root);
    let want = masked(strip(without), without_root);
    assert_eq!(
        got,
        want,
        "{context}: every byte the evidence-free read's\n{}\n{}",
        with.show(),
        without.show()
    );
    with.code()
}

/// A copy of `kiln`'s root in a merge whose `spec.lock` conflicts: both
/// branches changed the lock's first line after the header's.
fn unmerged_copy(kiln: &Kiln, dir: &str) -> PathBuf {
    let root = verify_common::clone_dir(&kiln.repo, dir);
    let git = |args: &[&str]| kiln.repo.git.git(&root, args);
    let lock = fs::read_to_string(root.join("spec.lock")).expect("the lock");
    let first = lock.lines().next().expect("the header").to_owned();
    git(&["checkout", "-q", "-b", "side"]);
    fs::write(
        root.join("spec.lock"),
        lock.replacen(&first, &format!("{first} (side)"), 1),
    )
    .expect("side's lock");
    kiln.repo.git.add_all(&root);
    kiln.repo.git.commit(&root, "side lock");
    git(&["checkout", "-q", "main"]);
    fs::write(
        root.join("spec.lock"),
        lock.replacen(&first, &format!("{first} (main)"), 1),
    )
    .expect("main's lock");
    kiln.repo.git.add_all(&root);
    kiln.repo.git.commit(&root, "main lock");
    let merge = kiln
        .repo
        .git
        .git_output(&root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: spec.lock conflicts");
    root
}

/// RD-2 **M2** (`tested` kept past a cause): over the evidence kiln (every
/// block `verified`) and its evidence-free twin, under each cause of
/// docs/canon/code-reads.md "Degrade" a read reaches: (1) a refused
/// `[code]` key and an unknown framework, (2) no `[code]`, (3) a walk gap,
/// (4) no `git`, a failing `git`, no worktree, (5) an unmerged `spec.lock`,
/// (6) a lock not UTF-8 and one of another `code_recipe` (its blocks
/// intact), the git session's end failing, and a `git` refusing
/// `hash-object`: `show --bindings` (a document, a leaf, the criterion),
/// the bundle, the brief and package, and impact print no `verified`, no
/// `tested | ` line, every `tested` `null`; their exit, rows and spec part
/// (every byte) are the twin's; a `tested:` note appears only for the hash
/// failure. (8) is not reachable from outside the binary (the code
/// refresh's store failing alone).
#[test]
fn degraded_read_never_verified() {
    let twins = Twins::new("tr-degrade");
    let reads = degraded_reads(&twins.task);
    // Premise: healthy, the evidence shows.
    let healthy = show(twins.with.root(), "KILN-RAMP", &vars_of(&twins.with.repo));
    assert_eq!(
        healthy.json()["nodes"][0]["bindings"]["tested"][0]["state"],
        json!("verified"),
        "premise: the evidence shows when nothing degrades\n{}",
        healthy.show()
    );

    // Each cause: (context, the edit on both kilns, vars and cwd per kiln,
    // whether only the hash fails).
    let run = |context: &str,
               hash: bool,
               cwd_of: &dyn Fn(&Kiln) -> PathBuf,
               vars_of_kiln: &dyn Fn(&Kiln) -> Vars| {
        let mut codes = BTreeMap::new();
        for (name, args) in &reads {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let (with_root, without_root) = (
                twins.with.repo.scratch.path().to_path_buf(),
                twins.without.repo.scratch.path().to_path_buf(),
            );
            let with = read_with(&cwd_of(&twins.with), &args, &vars_of_kiln(&twins.with));
            let without = read_with(
                &cwd_of(&twins.without),
                &args,
                &vars_of_kiln(&twins.without),
            );
            codes.insert(
                *name,
                assert_degraded_read(
                    (&with, &with_root),
                    (&without, &without_root),
                    name,
                    hash,
                    context,
                ),
            );
        }
        codes
    };
    let at_root = |kiln: &Kiln| kiln.root().to_path_buf();
    let plain_vars = |kiln: &Kiln| vars_of(&kiln.repo);

    // (1), (2): the config.
    let configs: Vec<String> = twins
        .both()
        .iter()
        .map(|kiln| kiln.repo.read("specengine.toml"))
        .collect();
    let edits: [(&str, ConfigEdit); 3] = [
        ("(1) a refused [code] key", |config| {
            config.replacen(ROOTS, &format!("{ROOTS}\nlanguages = [\"rust\"]"), 1)
        }),
        ("(1) an unknown framework", |config| {
            config.replacen(
                ROOTS,
                &format!("{ROOTS}\nframeworks = [\"no-such-framework\"]"),
                1,
            )
        }),
        ("(2) no [code]", bindings_common::without_code_table),
    ];
    for (context, edit) in edits {
        for (kiln, config) in twins.both().iter().zip(&configs) {
            assert!(config.contains(ROOTS), "premise: {ROOTS}");
            kiln.repo.write("specengine.toml", edit(config));
        }
        run(context, false, &at_root, &plain_vars);
        for (kiln, config) in twins.both().iter().zip(&configs) {
            kiln.repo.write("specengine.toml", config);
        }
    }

    // (3) a walk gap: an unreadable file under the code roots.
    let spare = "crates/kiln/src/spare.rs";
    {
        let mut modes = Vec::new();
        for kiln in twins.both() {
            kiln.repo.write(spare, "pub fn spare_part() {}\n");
            modes.push(Mode::set(kiln.root().join(spare), 0o000));
        }
        run("(3) a walk gap", false, &at_root, &plain_vars);
    }
    for kiln in twins.both() {
        fs::remove_file(kiln.root().join(spare)).expect("the spare file");
    }

    // (4) no git, a failing git, no worktree.
    let no_git = |kiln: &Kiln| {
        let nowhere = kiln.repo.scratch.dir("no-git-here");
        with_var(&vars_of(&kiln.repo), "PATH", nowhere.as_os_str())
    };
    run("(4) no git", false, &at_root, &no_git);
    let bad_git = |kiln: &Kiln| {
        let dir = kiln.repo.scratch.join("bad-git");
        with_var(&vars_of(&kiln.repo), "PATH", &failing_git(&dir))
    };
    run("(4) a failing git", false, &at_root, &bad_git);
    for kiln in twins.both() {
        let loose = kiln.repo.scratch.join("loose");
        copy_dir(kiln.root(), &loose);
        fs::remove_dir_all(loose.join(".git")).expect("no .git");
    }
    let loose = |kiln: &Kiln| fs::canonicalize(kiln.repo.scratch.join("loose")).expect("the copy");
    run("(4) no worktree", false, &loose, &plain_vars);

    // (5) an unmerged lock.
    for kiln in twins.both() {
        unmerged_copy(kiln, "unmerged");
    }
    let unmerged =
        |kiln: &Kiln| fs::canonicalize(kiln.repo.scratch.join("unmerged")).expect("the merge");
    run("(5) an unmerged lock", false, &unmerged, &plain_vars);

    // (6) a lock not UTF-8; a lock of another code recipe, its blocks kept.
    let locks: Vec<String> = twins
        .both()
        .iter()
        .map(|kiln| kiln.repo.lock_text())
        .collect();
    for kiln in twins.both() {
        kiln.repo.write("spec.lock", b"\xff\xfe".as_slice());
    }
    run("(6) a lock not UTF-8", false, &at_root, &plain_vars);
    for (kiln, lock) in twins.both().iter().zip(&locks) {
        let other = lock.replacen("abi/15\"", "abi/14\"", 1);
        assert_ne!(&other, lock, "premise: the code recipe edited");
        kiln.repo.write("spec.lock", other);
    }
    assert!(
        twins.with.repo.lock_text().contains("[[tested]]"),
        "premise: the blocks kept"
    );
    run("(6) another code recipe", false, &at_root, &plain_vars);
    for (kiln, lock) in twins.both().iter().zip(&locks) {
        kiln.repo.write("spec.lock", lock);
    }

    // The session's end failing.
    let session = |kiln: &Kiln| {
        let path = session_end_failing_git(&kiln.repo, &kiln.repo.scratch.join("session-git"));
        with_var(&vars_of(&kiln.repo), "PATH", &path)
    };
    run("the session's end failing", false, &at_root, &session);

    // `hash-object` refused: judged rows, blocks not judged.
    let refusing = |kiln: &Kiln| {
        let path = hash_refusing_git(&kiln.repo, &kiln.repo.scratch.join("hash-git"));
        with_var(&vars_of(&kiln.repo), "PATH", &path)
    };
    let codes = run("hash-object refused", true, &at_root, &refusing);
    assert!(
        codes.values().all(|code| *code == 0),
        "the hash failure changes no exit: {codes:?}"
    );
}

// ------------------------------------------------------------------- RD-3

/// RD-3 **M3** (the lock read without `--bindings`): over the evidence
/// kiln, `spec show REF` and `spec show REF --links` (a document, a
/// section, the criterion, an alias) with a `git` that fails every call,
/// then with a lock that is not UTF-8, then with one whose blocks are not
/// TOML, print every byte (exit, text, JSON, stderr) the evidence-free
/// twin prints with a sound `git` and lock, no note; each node's
/// `bindings` `null`.
#[test]
fn plain_show_reads_no_evidence() {
    let twins = Twins::new("tr-plain");
    let refs = ["KILN-FIRING", "KILN-RAMP", "glaze-check/GLZ-01", "PYRO-01"];
    let reference: BTreeMap<String, Read> = refs
        .iter()
        .flat_map(|id| [vec!["show", *id], vec!["show", *id, "--links"]])
        .map(|args| {
            let read = read_with(twins.without.root(), &args, &vars_of(&twins.without.repo));
            read.answered();
            (args.join(" "), read)
        })
        .collect();
    let check = |context: &str, vars: &Vars| {
        for (args, want) in &reference {
            let split: Vec<&str> = args.split(' ').collect();
            let got = read_with(twins.with.root(), &split, vars);
            assert_eq!(
                got.bytes(),
                want.bytes(),
                "{context}: spec {args}\n{}\n{}",
                got.show(),
                want.show()
            );
            assert!(
                !got.text.stderr.contains("note: "),
                "{context}: spec {args}: no note"
            );
            for node in list(&got.json()["nodes"]) {
                assert_eq!(node["bindings"], Value::Null, "{context}: spec {args}");
            }
        }
    };
    let vars = vars_of(&twins.with.repo);
    check("a sound git and lock", &vars);
    let bad = with_var(
        &vars,
        "PATH",
        &failing_git(&twins.with.repo.scratch.join("bad-git")),
    );
    check("a failing git", &bad);
    let lock = twins.with.repo.lock_text();
    twins.with.repo.write("spec.lock", b"\xff\xfe".as_slice());
    check("a lock not UTF-8", &vars);
    twins
        .with
        .repo
        .write("spec.lock", lock.replace("[[tested]]", "[[tested"));
    check("blocks that are not TOML", &vars);
    twins.with.repo.write("spec.lock", &lock);
}

// ------------------------------------------------------------------- RD-4

/// The record of the bytes before 8b-2, beside this test.
fn before_file() -> PathBuf {
    repository_root().join("crates/specengine-cli/tests/tested_reads_before.json")
}

/// One evidence-free read: its case name, where it runs, its arguments
/// and how many `tested` keys 8b-2 adds to its JSON.
struct BeforeCase {
    name: String,
    cwd: PathBuf,
    vars: Vars,
    args: Vec<String>,
    keys: usize,
}

/// The repositories the cases run in, kept alive with them.
#[derive(Default)]
struct Held {
    _repos: Vec<LockRepo>,
    _kilns: Vec<Kiln>,
}

/// The evidence-free cases, each repository a lock without a block (or
/// none): `fixtures/code-a` with its format-2 golden (a criterion linked
/// to GOAL-01, a task on GOAL-01 with that criterion and LAW-STEP
/// affected); the ECS corpus with every node accepted (a task on
/// PERK-STAMINA); the kiln without a lock and with a format-2 lock of no
/// entry (a task on KILN-RAMP, GLZ-01 its criterion, KILN-HOLD affected;
/// one on CONE-02, which binds nothing: no Bindings section). Per corpus
/// `show --bindings` (a document and a node), the bundle, the brief and
/// impact.
fn before_cases(tag: &str) -> (Held, Vec<BeforeCase>) {
    let mut held = Held::default();
    let mut extra = Vec::new();
    let mut cases = Vec::new();
    let mut add =
        |corpus: &str, cwd: &Path, vars: &Vars, task: &str, shown: [&str; 2], target: &str| {
            let reads: [(String, Vec<String>, usize); 6] = [
                (
                    format!("{corpus}: show {} --bindings", shown[0]),
                    vec!["show".into(), shown[0].into(), "--bindings".into()],
                    1,
                ),
                (
                    format!("{corpus}: show {} --bindings", shown[1]),
                    vec!["show".into(), shown[1].into(), "--bindings".into()],
                    1,
                ),
                (
                    format!("{corpus}: bundle {target}"),
                    vec![
                        "bundle".into(),
                        target.into(),
                        "--budget".into(),
                        "10000".into(),
                    ],
                    0,
                ),
                (
                    format!("{corpus}: task show"),
                    vec!["task".into(), "show".into(), task.into()],
                    1,
                ),
                (
                    format!("{corpus}: impact {target}"),
                    vec!["impact".into(), target.into()],
                    1,
                ),
                (
                    format!("{corpus}: impact {}", shown[0]),
                    vec!["impact".into(), shown[0].into()],
                    1,
                ),
            ];
            for (name, args, keys) in reads {
                cases.push(BeforeCase {
                    name,
                    cwd: cwd.to_path_buf(),
                    vars: vars.clone(),
                    args,
                    keys,
                });
            }
        };

    let golden = verify_common::golden(&format!("tr-before-a-{tag}"));
    bindings_common::criterion(&golden, 0);
    golden.commit_all("a criterion");
    let task = verify_common::new_task(&golden, &["GOAL-01"]);
    verify_common::plan_task(&golden, &task, &["cap-check/PASS-01"], &["LAW-STEP"]);
    add(
        "code-a",
        &golden.root,
        &vars_of(&golden),
        &task,
        ["PIECE-COUNTER", "GOAL-01"],
        "GOAL-01",
    );
    held._repos.push(golden);

    let game = verify_common::ecs(&format!("tr-before-ecs-{tag}"));
    let task = verify_common::new_task(&game, &["PERK-STAMINA"]);
    add(
        "ecs",
        &game.root,
        &vars_of(&game),
        &task,
        ["docs/spec/stamina.md", "PERK-STAMINA"],
        "PERK-STAMINA",
    );
    held._repos.push(game);

    for (corpus, lock) in [
        ("kiln without a lock", false),
        ("kiln, a lock of no entry", true),
    ] {
        let kiln = Kiln::new(&format!("tr-before-kiln-{}-{tag}", usize::from(lock)));
        if lock {
            kiln.repo
                .write("spec.lock", without_blocks(&tested_common::golden_text()));
            kiln.repo.commit_all("a lock of no entry");
        }
        let task = kiln.new_task(&["KILN-RAMP"]);
        kiln.plan_task(&task, &["glaze-check/GLZ-01"], &["KILN-HOLD"]);
        add(
            corpus,
            kiln.root(),
            &vars_of(&kiln.repo),
            &task,
            ["KILN-FIRING", "glaze-check/GLZ-01"],
            "KILN-HOLD",
        );
        // A task whose node binds nothing: its brief has no Bindings
        // section.
        let bare = kiln.new_task(&["CONE-02"]);
        extra.push(BeforeCase {
            name: format!("{corpus}: task show, no row"),
            cwd: kiln.root().to_path_buf(),
            vars: vars_of(&kiln.repo),
            args: vec!["task".into(), "show".into(), bare],
            keys: 1,
        });
        held._kilns.push(kiln);
    }
    cases.extend(extra);
    (held, cases)
}

/// Each case's exit and the b3 of its text stdout, JSON stdout (with
/// `strip`, the keys 8b-2 adds taken out first) and stderr, by `program`.
fn before_record(program: &Path, strip: bool, tag: &str) -> Vec<Value> {
    let (_held, cases) = before_cases(tag);
    cases
        .iter()
        .map(|case| {
            let args: Vec<&str> = case.args.iter().map(String::as_str).collect();
            let read = read_program(program, &case.cwd, &args, &case.vars);
            let json = if strip {
                without_read_tested(&read.json_run.stdout, case.keys)
            } else {
                read.json_run.stdout.clone()
            };
            json!({
                "case": case.name,
                "exit": read.code(),
                "text": specengine_store::b3_hash(read.text.stdout.as_bytes()),
                "json": specengine_store::b3_hash(json.as_bytes()),
                "stderr": specengine_store::b3_hash(read.text.stderr.as_bytes()),
            })
        })
        .collect()
}

/// Records [`before_file`] with `SPECENGINE_BEFORE_SPEC`, the `spec`
/// binary of 35f366c (8b-1 shipped, 8b-2 not started); never re-recorded
/// from a later binary.
#[test]
#[ignore = "records the bytes before 8b-2: SPECENGINE_BEFORE_SPEC names that binary"]
fn record_the_bytes_before_tested_reads() {
    let program = PathBuf::from(
        std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC: the binary"),
    );
    let cases = before_record(&program, false, "one");
    assert_eq!(
        cases,
        before_record(&program, false, "two"),
        "stable answers from another root"
    );
    let lines: Vec<String> = cases.iter().map(Value::to_string).collect();
    fs::write(
        before_file(),
        format!(
            "{{\"recorded_with\": \"spec at 35f366c (8b-1 shipped, 8b-2 not started)\",\n\"cases\": [\n{}\n]}}\n",
            lines.join(",\n")
        ),
    )
    .expect("the record");
}

/// RD-4 **M4** (an evidence-free read's bytes changed; "Rules": WHEN no
/// block exists, every read's text SHALL be byte-identical to 8b-1's, and
/// its JSON SHALL differ only by the new keys): over locks without a block
/// (and no lock), `show --bindings`, the bundle (text and JSON outright),
/// the brief and impact print the text, stderr and exit the binary before
/// 8b-2 printed (its record), and their JSON once the `tested` keys
/// (`[]`: each shown node's, the package's, impact's) are taken out.
#[test]
fn evidence_free_reads_unchanged() {
    let record: Value =
        serde_json::from_str(&fs::read_to_string(before_file()).expect("tested_reads_before.json"))
            .expect("the record parses");
    let got = before_record(Path::new(common::SPEC), true, "now");
    let want = list(&record["cases"]);
    assert_eq!(got.len(), want.len(), "the cases");
    for (got, want) in got.iter().zip(&want) {
        assert_eq!(got, want, "the bytes before 8b-2: {}", want["case"]);
    }
}

// ------------------------------------------------------------------- TE-2

/// The reads TE-2 compares: show (document, criterion), the bundle, the
/// brief and package, impact.
fn determinism_reads(task: &str) -> Vec<Vec<String>> {
    degraded_reads(task)
        .into_iter()
        .map(|(_, args)| args)
        .collect()
}

/// One read's printed bytes: (args, exit, text, JSON, stderr).
type ReadBytes = (String, i32, String, String, String);

/// Every printed byte of each read from `cwd`.
fn read_bytes(cwd: &Path, vars: &Vars, task: &str) -> Vec<ReadBytes> {
    determinism_reads(task)
        .iter()
        .map(|args| {
            let split: Vec<&str> = args.iter().map(String::as_str).collect();
            let read = read_with(cwd, &split, vars);
            assert_eq!(read.code(), 0, "{}", read.show());
            (
                args.join(" "),
                read.code(),
                read.text.stdout.clone(),
                read.json_run.stdout.clone(),
                read.text.stderr.clone(),
            )
        })
        .collect()
}

/// A kiln made by `make`, its evidence written as `lock` (the reference
/// kiln's bytes) or computed, the [`EvidenceState::Mixed`] state, the
/// task: the kiln and its task.
fn determinism_kiln(make: Kiln, lock: Option<&str>) -> (Kiln, String) {
    let kiln = match lock {
        None => tested_kiln::with_evidence(make),
        Some(lock) => {
            pin_and_mention(&make);
            make.repo
                .commit_all("pinned at 1, the criterion names KILN-HOLD");
            make.repo.write("spec.lock", lock);
            make.repo.commit_all("the evidence");
            make
        }
    };
    let task = kiln.new_task(&["KILN-RAMP"]);
    kiln.plan_task(&task, &["glaze-check/GLZ-01"], &["KILN-HOLD"]);
    EvidenceState::Mixed.write(&kiln);
    (kiln, task)
}

/// TE-2 (M: a path or a date inside a line): over the evidence kiln in its
/// mixed state, each read (show of a document and the criterion, the
/// bundle, the brief and package, impact) prints the same bytes repeated,
/// from another root (`elsewhere/deeper`, its own `HOME`), from another
/// cwd (`crates/kiln`), over the fixture written in reverse creation
/// order, over a `core.autocrlf=true` checkout of every code and spec
/// file, and over a SHA-256 repository holding the same lock bytes.
#[test]
fn tested_reads_are_byte_identical_across_roots_order_and_crlf() {
    let (one, task) = determinism_kiln(Kiln::new("tr-det-one"), None);
    let lock = one.repo.lock_text();
    let want = read_bytes(one.root(), &vars_of(&one.repo), &task);
    assert!(
        want.iter()
            .all(|(_, _, text, _, _)| text.contains("tested | ")),
        "premise: the evidence shows in every read: {want:#?}"
    );
    assert_eq!(
        read_bytes(one.root(), &vars_of(&one.repo), &task),
        want,
        "a repeat"
    );

    let (two, task_two) = determinism_kiln(Kiln::at("tr-det-two", "elsewhere/deeper"), None);
    assert_eq!(task_two, task);
    assert_eq!(two.repo.lock_text(), lock, "the same evidence");
    assert_eq!(
        read_bytes(two.root(), &vars_of(&two.repo), &task),
        want,
        "another root"
    );
    assert_eq!(
        read_bytes(&two.root().join("crates/kiln"), &vars_of(&two.repo), &task),
        want,
        "another cwd"
    );

    let (reversed, _) = determinism_kiln(Kiln::reversed("tr-det-reversed"), None);
    assert_eq!(
        read_bytes(reversed.root(), &vars_of(&reversed.repo), &task),
        want,
        "a reverse creation order"
    );

    let crlf = Kiln::new("tr-det-crlf");
    let (crlf, _) = determinism_kiln(crlf, None);
    crlf.repo.git(&["config", "core.autocrlf", "true"]);
    EvidenceState::Mixed.undo(&crlf);
    for file in [RAMP, SCHEDULE, COOL, "crates/kiln/src/lib.rs"] {
        fs::remove_file(crlf.root().join(file)).expect("a checked-out file");
        crlf.repo.git(&["checkout", "--", file]);
    }
    assert!(crlf.repo.read(RAMP).contains("\r\n"), "a CRLF checkout");
    assert!(crlf.repo.read(SCHEDULE).contains("\r\n"), "a CRLF checkout");
    EvidenceState::Mixed.write(&crlf);
    assert_eq!(
        read_bytes(crlf.root(), &vars_of(&crlf.repo), &task),
        want,
        "core.autocrlf=true"
    );

    let sha256 = Kiln::bare(
        LockRepo::of(
            "tr-det-sha256",
            FIXTURE,
            "repo",
            &["--object-format=sha256"],
        ),
        SLUG,
    );
    sha256.write_runner(RUNNER, 0o755);
    sha256.repo.commit_all("the runner");
    assert_eq!(sha256.repo.head().len(), 64, "a SHA-256 repository");
    let (sha256, _) = determinism_kiln(sha256, Some(&lock));
    assert_eq!(
        read_bytes(sha256.root(), &vars_of(&sha256.repo), &task),
        want,
        "a SHA-256 repository"
    );
}

// ------------------------------------------------------------------- TE-3

/// The median of seven timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..7).map(|_| call()).collect();
    times.sort();
    times[3]
}

/// TE-3, a record, not a gate (docs/features/tested-reads.md "Cost"):
/// over the evidence kiln with its four blocks (and one gone), then the
/// same commits' lock without the blocks, the median of seven warm runs
/// of each read (`show KILN-FIRING --bindings --json`, `bundle KILN-HOLD
/// --json`, `task show T --json`, `impact KILN-RAMP --json`); prints the
/// timings and their ratios only.
#[test]
#[ignore = "a record (8b-2 cost): run with --run-ignored ignored-only --no-capture"]
fn tested_reads_cost() {
    let twins = Twins::new("tr-cost");
    let reads: [(&str, Vec<&str>); 4] = [
        (
            "show --bindings",
            vec!["show", "KILN-FIRING", "--bindings", "--json"],
        ),
        (
            "bundle",
            vec!["bundle", "KILN-HOLD", "--budget", "10000", "--json"],
        ),
        (
            "task show",
            vec!["task", "show", twins.task.as_str(), "--json"],
        ),
        ("impact", vec!["impact", "KILN-RAMP", "--json"]),
    ];
    let blocks = tested_common::block_nodes(&twins.with.repo.lock_text()).len();
    for (name, args) in &reads {
        let time = |kiln: &Kiln| {
            let vars = vars_of(&kiln.repo);
            let _warm = run_with(kiln.root(), args, &vars);
            median(|| {
                let started = Instant::now();
                let run = run_with(kiln.root(), args, &vars);
                let elapsed = started.elapsed();
                assert_eq!(run.code, 0, "{}", run.show());
                elapsed
            })
        };
        let with = time(&twins.with);
        let without = time(&twins.without);
        eprintln!(
            "8b-2 read cost: {name}: {blocks} blocks: median {with:?}; without blocks {without:?}; ratio {:.2}",
            with.as_secs_f64() / without.as_secs_f64().max(f64::EPSILON)
        );
    }
}

/// docs/features/tested-reads.md "Description and interactions" (nothing
/// new is written: no lock write, queue row, tool, argument or route):
/// over the evidence kiln in its mixed state, every read with blocks
/// leaves every byte under the root (`.git` too) as it was. M: a read
/// writing the lock or the index.
#[test]
fn evidence_reads_write_nothing_under_the_root() {
    let twins = Twins::new("tr-writes");
    EvidenceState::Mixed.write(&twins.with);
    let vars = vars_of(&twins.with.repo);
    let before = snapshot(twins.with.root());
    for args in determinism_reads(&twins.task) {
        let split: Vec<&str> = args.iter().map(String::as_str).collect();
        read_with(twins.with.root(), &split, &vars).answered();
    }
    assert_eq!(
        snapshot(twins.with.root()),
        before,
        "the reads wrote under the root"
    );
}

// ------------------------------------------------------- Rules: duplicates

/// docs/features/tested-reads.md "Description and interactions" (Decided:
/// duplicate IDs on the first holder) and "Rules and edge cases" (a read
/// and verify agree key for key): a second holder of KILN-RAMP in
/// `docs/spec/zz-kiln.md` (after the first by path) with a text of its
/// own: `show KILN-RAMP --bindings` lists
/// both holders, each with plain verify's one entry of the block, as
/// `verify --node KILN-RAMP` lists it. M: a holder judged on its own text.
#[test]
fn duplicate_holders_list_the_first_holders_entry() {
    let kiln = evidence_kiln("tr-duplicate");
    kiln.repo.write(
        "docs/spec/zz-kiln.md",
        "---\nclass: canon\nid: KILN-COPY\nowner: owner\nreviewed: 2026-10-16\n---\n\n# A copy\n\n## Ramp {#KILN-RAMP}\n\nA second holder with a text of its own.\n",
    );
    let vars = vars_of(&kiln.repo);
    let verify = plain(&kiln.repo, &[]);
    // Whatever verify makes of a twice-held ID (here a binding naming it
    // is not lockable now), the reads say the same.
    let entry = tested_common::tested_of(&verify, "KILN-RAMP");
    let read = show(kiln.root(), "KILN-RAMP", &vars);
    read.answered();
    let nodes = list(&read.json()["nodes"]);
    assert_eq!(nodes.len(), 2, "two holders\n{}", read.show());
    for node in &nodes {
        assert_eq!(
            node["bindings"]["tested"],
            json!([entry.clone()]),
            "{}: the first holder's entry",
            text(&node["path"])
        );
    }
    assert_eq!(
        verify_json(kiln.root(), &["--node", "KILN-RAMP"], &vars)["tested"],
        json!([entry]),
        "verify --node alike"
    );
}
