//! docs/features/verify-task.md (8a-2): `spec verify --task T [--changed]
//! [--json]`, plain verify's rows kept to 8a-1's task node set. RD-1..RD-6
//! (each test names the mutation `Mn` that turns it red) and TE-1..TE-4.
//!
//! **The oracle is plain verify** (no golden, no `expected.json` change):
//! on one state, plain `spec verify` and `spec verify --task T` run
//! through the library (`verify`, `render_text`, `render_json`); the test
//! knows each task's set (the canonical IDs of its nodes now, sections
//! included) and keeps plain's rows whose `node` names a set node (a
//! look-alike under its Latin fix). Everything else is derived from those
//! rows by the canon's own rules (`spec-verify.md` "Verdict and exit",
//! `spec-verify-output.md` "Text", "Hints", "JSON"), each derivation first
//! checked against plain's own document (a premise: a red premise is this
//! file's model, not the developer's code). The binary runs where clap,
//! stdin or stdout is the subject (RD-1, RD-6's "stdout empty" cells).
//!
//! TE-4 runs inside every `--task` run ([`Untouched`]): every byte under
//! the worktree (`.git` too), `git status --porcelain=v1 -z
//! --untracked-files=all`, `spec.lock`, the queue's dump (tasks, runs,
//! events, proposals) and each task's `revision`, `updated_at` and runs.
//!
//! Fixtures, reused (no new fixture): `code-a` with the golden lock
//! (`verify_common::golden`), `anchor-states` (`anchor_common`),
//! `verify-tests` (a kiln library, no lock) and `verify-ecs`
//! (`verify_common::ecs`). Each repository has its own `HOME`, so the
//! tasks of one test number from `T-0001`. Non-Latin characters in Rust
//! sources are Unicode escapes (ADR-0024).
//!
//! docs/features/tested-evidence.md (8b-1) RD-4: `--task T`'s `tested` is
//! plain's entries of its set (the differential, `task_tested_is_its_sets`).

mod anchor_common;
mod common;
mod lock_common;
mod tested_common;
mod verify_common;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{RUN_TIMEOUT, SPEC, copy_dir, data_dir, fixture, snapshot};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_cli::{
    Env, Globals, Outcome, VerifiedTree, VerifyRequest, render_json, render_text, verify,
};
use specengine_store::{ProposalQueue as _, SqliteQueue};
use verify_common::{
    BOUNDS, COUNTER, COUNTER_MD, ECS_LIB, LIMITS, MOVEMENT, STAMINA, TOP_KEYS, approve_task,
    claim_task, ecs, golden, golden_file, new_task, new_task_in, plan_task, set_mode,
};

/// The slugs of the fixtures: the queue databases' names.
const CODE_A: &str = "code-a";
const ANCHORS: &str = "anchor-states";
const KILN: &str = "kiln-works";
const GAME: &str = "verify-ecs";

/// `HEAD`'s note under plain `enforce-introduced` (`spec-verify.md`
/// "Verdict and exit").
const NO_BASE: &str =
    "mode `enforce-introduced` has no base without --changed: judged as `enforce`";

/// The states a drift item is raised for (core `drift::raises`, a sweep):
/// the `hint: drift:` line.
const DRIFTING: [&str; 4] = ["code_ahead", "spec_ahead", "conflict", "broken_binding"];

/// "The set": a ref of the task's columns naming nothing now.
fn note_gone(reference: &str) -> String {
    format!("verify: {reference} names nothing now")
}

/// Q2: rows the rule ties to no node of the corpus.
fn note_no_node(rows: usize) -> String {
    format!("verify --task: {rows} rows name no node now; plain spec verify lists them")
}

/// Q3: set nodes no kept row names.
fn note_no_binding(nodes: usize, task: &str) -> String {
    format!("verify --task: {nodes} nodes of {task} have no binding")
}

// ------------------------------------------------------------------- runs

/// One `spec verify [--changed] [--task T]` through the library: its exit
/// and what `main` would print.
struct Run {
    what: String,
    code: i32,
    outcome: Option<Outcome>,
    /// The failure's message (stdout empty), when the call failed.
    error: Option<String>,
}

impl Run {
    /// stdout without `--json` (empty when the call failed).
    fn text(&self) -> String {
        self.outcome.as_ref().map(render_text).unwrap_or_default()
    }

    /// stdout with `--json` (empty when the call failed).
    fn json_text(&self) -> String {
        self.outcome.as_ref().map(render_json).unwrap_or_default()
    }

    fn json(&self) -> Value {
        let text = self.json_text();
        assert!(
            !text.is_empty(),
            "{}: no document:\n{}",
            self.what,
            self.show()
        );
        common::one_json_document(&text)
            .unwrap_or_else(|problem| panic!("{}: {problem}\n{}", self.what, self.show()))
    }

    /// stderr's lines (the notes; the failure's message).
    fn stderr_lines(&self) -> Vec<String> {
        match (&self.outcome, &self.error) {
            (Some(outcome), _) => outcome.stderr_lines(),
            (None, Some(error)) => error.lines().map(str::to_owned).collect(),
            (None, None) => Vec::new(),
        }
    }

    fn rows(&self) -> Vec<Value> {
        self.json()["rows"]
            .as_array()
            .expect("`rows` is an array")
            .clone()
    }

    fn notes(&self) -> Vec<String> {
        self.json()["notes"]
            .as_array()
            .expect("`notes` is an array")
            .iter()
            .map(|note| note.as_str().expect("a note text").to_owned())
            .collect()
    }

    /// stdout's last line.
    fn summary(&self) -> String {
        self.text().lines().last().unwrap_or_default().to_owned()
    }

    fn show(&self) -> String {
        format!(
            "{}: exit {}\n--- stdout\n{}--- stderr\n{}",
            self.what,
            self.code,
            self.text(),
            self.stderr_lines().join("\n")
        )
    }

    /// Exit `code`, a report on stdout.
    fn answered(&self, code: i32) -> &Self {
        assert!(self.outcome.is_some(), "a report:\n{}", self.show());
        assert_eq!(self.code, code, "{}", self.show());
        self
    }

    /// Exit 2, stdout empty (`--json` too): the failure's message.
    fn failed(&self) -> String {
        assert!(
            self.outcome.is_none(),
            "{}: stdout must stay empty:\n{}",
            self.what,
            self.show()
        );
        assert_eq!(self.code, 2, "{}", self.show());
        self.error.clone().expect("a failure's message")
    }

    /// Every printed byte: the exit, stdout (text, JSON), stderr.
    fn bytes(&self) -> (i32, String, String, Vec<String>) {
        (
            self.code,
            self.text(),
            self.json_text(),
            self.stderr_lines(),
        )
    }
}

/// `spec verify` through the library from `cwd`, the repository's `HOME`
/// and git environment, `globals` as given.
fn verify_with(
    repo: &LockRepo,
    env: &Env,
    globals: &Globals,
    tree: VerifiedTree,
    task: Option<&str>,
) -> Run {
    let what = format!(
        "spec verify {tree:?} task {task:?} in {}",
        env.cwd.display()
    );
    let result = verify(
        env,
        globals,
        &VerifyRequest {
            tree,
            task: task.map(str::to_owned),
            node: None,
            symbol: None,
            git: repo.git_env(&env.cwd),
        },
    );
    match result {
        Ok(outcome) => Run {
            what,
            code: i32::from(outcome.exit().code()),
            outcome: Some(Outcome::Verify(Box::new(outcome))),
            error: None,
        },
        Err(error) => Run {
            what,
            code: i32::from(error.exit.code()),
            outcome: None,
            error: Some(error.message),
        },
    }
}

/// [`verify_with`] from `cwd`, no `--root`, no `--config`.
fn verify_at(repo: &LockRepo, cwd: &Path, tree: VerifiedTree, task: Option<&str>) -> Run {
    verify_with(repo, &repo.env(cwd), &Globals::default(), tree, task)
}

/// Plain verify from the root.
fn plain(repo: &LockRepo, tree: VerifiedTree) -> Run {
    verify_at(repo, &repo.root, tree, None)
}

/// `--task <task>` from the root, TE-4 asserted around it.
fn scoped(repo: &LockRepo, slug: &str, tree: VerifiedTree, task: &str) -> Run {
    let before = Untouched::take(repo, slug);
    let run = verify_at(repo, &repo.root, tree, Some(task));
    before.still(repo, slug, &run.what);
    run
}

// ------------------------------------------------------------------ TE-4

/// What a `--task` run must leave as it was (TE-4).
#[derive(Debug, PartialEq, Eq)]
struct Untouched {
    /// Every byte under the worktree, `.git` too.
    files: BTreeMap<String, Option<Vec<u8>>>,
    /// `git status --porcelain=v1 -z --untracked-files=all`.
    status: Vec<u8>,
    lock: Option<Vec<u8>>,
    /// The slug's queue: `SqliteQueue::dump` (proposals, tasks, runs,
    /// events), then per task its `revision`, `updated_at`, status and
    /// runs, then the event count.
    queue: Vec<String>,
}

impl Untouched {
    fn take(repo: &LockRepo, slug: &str) -> Self {
        // `git status` first: its index refresh lands before the snapshot.
        let status = repo.porcelain();
        let files = snapshot(&repo.root);
        Self {
            files,
            status,
            lock: repo.lock(),
            queue: queue_state(repo, slug),
        }
    }

    fn still(&self, repo: &LockRepo, slug: &str, context: &str) {
        let files = snapshot(&repo.root);
        let changed: Vec<&String> = self
            .files
            .keys()
            .chain(files.keys())
            .filter(|path| self.files.get(*path) != files.get(*path))
            .collect();
        assert!(
            changed.is_empty(),
            "{context} wrote under the worktree: {changed:?}"
        );
        assert_eq!(repo.porcelain(), self.status, "{context}: git status");
        assert_eq!(repo.lock(), self.lock, "{context}: spec.lock's bytes");
        assert_eq!(queue_state(repo, slug), self.queue, "{context}: the queue");
    }
}

/// The slug's queue as [`Untouched::queue`] reads it.
fn queue_state(repo: &LockRepo, slug: &str) -> Vec<String> {
    let db = data_dir(&repo.home).join(format!("{slug}.db"));
    assert!(db.exists(), "the queue {slug} exists: its tasks are made");
    let queue = SqliteQueue::open(&db, slug).expect("the queue opens");
    let mut state: Vec<String> = queue
        .dump()
        .expect("the dump")
        .lines()
        .map(str::to_owned)
        .collect();
    for number in 1..=9 {
        let id = format!("T-{number:04}");
        if let Some(task) = queue.get_task(&id).expect("a task read") {
            state.push(format!(
                "{id} {:?} revision {} updated_at {} runs {}",
                task.status,
                task.revision,
                task.updated_at,
                task.runs.len()
            ));
        }
    }
    state.push(format!(
        "events {}",
        queue.events().expect("the events").len()
    ));
    state
}

// ---------------------------------------------------------------- oracle

/// A task on one state: its ID and its set now.
struct Case<'a> {
    id: &'a str,
    /// The canonical IDs of the set's nodes now (targets, criteria refs,
    /// affected nodes, the sections they contain).
    set: &'a [&'a str],
    /// A row `node` naming a set node by another spelling: (as keyed, the
    /// node's canonical ID).
    spelled: &'a [(&'a str, &'a str)],
    /// The refs of its columns naming nothing now.
    gone: &'a [&'a str],
}

impl<'a> Case<'a> {
    fn of(id: &'a str, set: &'a [&'a str]) -> Self {
        Self {
            id,
            set,
            spelled: &[],
            gone: &[],
        }
    }

    /// The set node `node` (as a row keys it) names, if any.
    fn names(&self, node: &str) -> Option<&'a str> {
        let canonical = self
            .spelled
            .iter()
            .find(|(written, _)| *written == node)
            .map_or(node, |(_, canonical)| canonical);
        self.set.iter().copied().find(|id| *id == canonical)
    }

    fn keeps(&self, row: &Value) -> bool {
        self.names(text(&row["node"])).is_some()
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Q2's rows: a marker naming no node (dangling, bare) or an entry naming
/// no node's canonical ID (`node_gone`: deleted, renamed, an alias now).
fn names_no_node(row: &Value) -> bool {
    row["category"] == "dangling" || row["category"] == "node_gone" || row["node"] == ""
}

/// A row's six key fields.
fn key_of(row: &Value) -> String {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
    })
    .to_string()
}

fn count(rows: &[Value], wanted: impl Fn(&Value) -> bool) -> usize {
    rows.iter().filter(|row| wanted(row)).count()
}

/// "JSON" `counts` over `rows`.
fn counts_of(rows: &[Value], skipped: &Value) -> Value {
    json!({
        "rows": rows.len(),
        "ok": count(rows, |row| row["class"].is_null()),
        "fix": count(rows, |row| row["class"] == "fix"),
        "check": count(rows, |row| row["class"] == "check"),
        "cannot": count(rows, |row| row["class"] == "cannot"),
        "pre_existing": count(rows, |row| row["pre_existing"] == true),
        "skipped": skipped,
    })
}

/// "JSON" `cascade` over `rows`.
fn cascade_of(rows: &[Value]) -> Value {
    json!({
        "by_blob": count(rows, |row| row["compared"] == "blob"),
        "by_ast": count(rows, |row| row["compared"] == "ast"),
    })
}

/// "Verdict and exit" over `rows` under `mode` (the summary's: plain
/// `enforce-introduced` reads `enforce`), no run cause.
fn verdict_of(mode: &Value, rows: &[Value]) -> (&'static str, i32) {
    if rows.iter().any(|row| row["class"] == "cannot") {
        return ("cannot-verify", 2);
    }
    let blocking = match mode.as_str() {
        Some("enforce") => rows.iter().any(|row| row["class"] == "fix"),
        Some("enforce-introduced") => rows
            .iter()
            .any(|row| row["class"] == "fix" && row["pre_existing"] == false),
        _ => false,
    };
    if blocking {
        ("blocked", 1)
    } else if rows.iter().all(|row| row["class"].is_null()) {
        ("clean", 0)
    } else {
        ("observed", 0)
    }
}

/// `delta` over `rows`: plain's lists, each kept to the keys of `rows`
/// (`moved` by its `to`), `affected_nodes` to the nodes of `rows` not
/// `ok` and not Pre-existing; `tests_reaching` (docs/features/show-bindings.md
/// "delta.tests_reaching") over `all`, every row the run judged (before
/// `--task`'s filter): the six-field keys, distinct, in row order, of the
/// `verifies` rows whose node is affected. These corpora hold no
/// `verifies` link, so the linked half adds none (plain's own premise
/// checks it).
fn delta_of(plain: &Value, rows: &[Value], all: &[Value], case: Option<&Case<'_>>) -> Value {
    let keys: Vec<String> = rows.iter().map(key_of).collect();
    let mut delta = plain.clone();
    let bindings = &plain["bindings"];
    delta["bindings"]["moved"] = Value::Array(
        list(&bindings["moved"])
            .into_iter()
            .filter(|pair| keys.contains(&key_of(&pair["to"])))
            .collect(),
    );
    for name in ["ambiguous", "lost", "stale"] {
        delta["bindings"][name] = Value::Array(
            list(&bindings[name])
                .into_iter()
                .filter(|key| keys.contains(&key_of(key)))
                .collect(),
        );
    }
    delta["affected_nodes"] = Value::Array(
        list(&plain["affected_nodes"])
            .into_iter()
            .filter(|node| {
                rows.iter().any(|row| {
                    !row["class"].is_null()
                        && row["pre_existing"] == false
                        && (row["node"] == *node
                            || case.and_then(|case| case.names(text(&row["node"])))
                                == node.as_str())
                })
            })
            .collect(),
    );
    let affected = list(&delta["affected_nodes"]);
    let mut reaching: Vec<Value> = Vec::new();
    for row in all.iter().filter(|row| row["relation"] == "verifies") {
        let key = json!({
            "node": row["node"],
            "relation": row["relation"],
            "file": row["file"],
            "kind": row["kind"],
            "symbol": row["symbol"],
            "item_kind": row["item_kind"],
        });
        if affected.contains(&row["node"]) && !reaching.contains(&key) {
            reaching.push(key);
        }
    }
    delta["tests_reaching"] = Value::Array(reaching);
    delta
}

fn list(value: &Value) -> Vec<Value> {
    value.as_array().expect("a list").clone()
}

/// The `hint:` line `line` stays over `rows` ("Hints": one per state and
/// category present; `moved` when a row moved; `drift` when a row would
/// raise an item).
fn hint_kept(line: &str, rows: &[Value]) -> bool {
    let label = line
        .strip_prefix("hint: ")
        .and_then(|rest| rest.split_once(": "))
        .map(|(label, _)| label)
        .unwrap_or_else(|| panic!("a hint line: {line:?}"));
    match label {
        "moved" => rows.iter().any(|row| row["anchor"]["state"] == "moved"),
        "drift" => rows
            .iter()
            .any(|row| DRIFTING.contains(&text(&row["state"]))),
        _ => rows
            .iter()
            .filter(|row| !row["class"].is_null())
            .any(|row| {
                let category = if row["detail"] == "key_problem" {
                    "key_problem"
                } else {
                    text(&row["category"])
                };
                let mut own = text(&row["state"]).to_owned();
                if !category.is_empty() {
                    own.push(' ');
                    own.push_str(category);
                }
                own == label
            }),
    }
}

/// The summary after its prefix ("Text"), from a document.
fn summary_of(document: &Value) -> String {
    let counts = &document["counts"];
    let mode = document["mode"]
        .as_str()
        .map(|mode| format!(" [{mode}]"))
        .unwrap_or_default();
    let pre_existing = if document["base"].is_null() {
        String::new()
    } else {
        format!(" ({} pre-existing)", counts["pre_existing"])
    };
    let rows = list(&document["rows"]);
    let moved = count(&rows, |row| {
        row["class"].is_null() && row["anchor"]["state"] == "moved"
    });
    let moved = if moved > 0 {
        format!(" ({moved} moved)")
    } else {
        String::new()
    };
    format!(
        "{}{mode}: {} fix, {} check{pre_existing}, {} cannot, {} ok{moved}; {} by blob, {} by ast; {} skipped",
        text(&document["verdict"]),
        counts["fix"],
        counts["check"],
        counts["cannot"],
        counts["ok"],
        document["cascade"]["by_blob"],
        document["cascade"]["by_ast"],
        counts["skipped"],
    )
}

/// Plain's text split: each non-`ok` row's block (its line and the
/// `moved from` / `candidate` lines under it), the `cannot:` lines, the
/// hints, the `tested | …` lines (after the hints, before the summary:
/// docs/features/tested-evidence.md "Plain verify").
fn plain_parts(plain: &Run) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let (mut blocks, mut causes, mut hints, mut tested): (
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
    ) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let text = plain.text();
    let mut lines: Vec<&str> = text.lines().collect();
    lines.pop(); // the summary
    for line in lines {
        if line.starts_with("cannot: ") {
            causes.push(line.to_owned());
        } else if line.starts_with("hint: ") {
            hints.push(line.to_owned());
        } else if line.starts_with("tested | ") {
            tested.push(line.to_owned());
        } else if line.starts_with("  ") {
            let block = blocks
                .last_mut()
                .unwrap_or_else(|| panic!("a row before {line:?}"));
            block.push_str(line);
            block.push('\n');
        } else {
            blocks.push(format!("{line}\n"));
        }
    }
    (blocks, causes, hints, tested)
}

/// The text `rows` (a subsequence of plain's) print under `prefix`:
/// plain's blocks of those rows, plain's `cannot:` lines, plain's hints
/// those rows keep, plain's `tested` lines of the nodes `node_kept` keeps,
/// the summary of `document`.
fn text_of(
    plain: &Run,
    keep: impl Fn(&Value) -> bool,
    node_kept: impl Fn(&str) -> bool,
    prefix: &str,
    document: &Value,
) -> String {
    let (blocks, causes, hints, tested) = plain_parts(plain);
    let findings: Vec<Value> = plain
        .rows()
        .into_iter()
        .filter(|row| !row["class"].is_null())
        .collect();
    assert_eq!(
        blocks.len(),
        findings.len(),
        "premise: one block per plain row not ok\n{}",
        plain.show()
    );
    let kept: Vec<Value> = plain.rows().into_iter().filter(|row| keep(row)).collect();
    let mut out = String::new();
    for (block, row) in blocks.iter().zip(&findings) {
        if keep(row) {
            out.push_str(block);
        }
    }
    for line in causes
        .iter()
        .chain(hints.iter().filter(|line| hint_kept(line, &kept)))
        .chain(
            tested
                .iter()
                .filter(|line| line.rsplit(" | ").next().is_some_and(&node_kept)),
        )
    {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(prefix);
    out.push_str(&summary_of(document));
    out.push('\n');
    out
}

/// The depth-1 keys of the JSON object `raw` opens with, in order.
fn top_keys(raw: &str) -> Vec<String> {
    let bytes = raw.as_bytes();
    let (mut depth, mut in_string, mut escaped, mut start) = (0usize, false, false, 0usize);
    let mut keys = Vec::new();
    for (at, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                if depth == 1 && bytes.get(at + 1) == Some(&b':') {
                    keys.push(raw[start..at].to_owned());
                }
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                start = at + 1;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            _ => {}
        }
    }
    keys
}

/// The premises: this file's model reproduces plain's own document and
/// text (no run cause; `task`, and since docs/features/verify-node.md
/// `node`, `symbol` and `omitted`, `null`).
fn plain_premises(plain: &Run) {
    let document = plain.json();
    let rows = plain.rows();
    for key in ["task", "node", "symbol", "omitted"] {
        assert_eq!(
            document[key],
            Value::Null,
            "plain's `{key}`: {}",
            plain.what
        );
    }
    assert_eq!(top_keys(&plain.json_text()), TOP_KEYS, "plain's keys");
    assert_eq!(
        document["cannot"],
        json!([]),
        "no run cause:\n{}",
        plain.show()
    );
    assert_eq!(
        counts_of(&rows, &document["counts"]["skipped"]),
        document["counts"],
        "premise: counts"
    );
    assert_eq!(cascade_of(&rows), document["cascade"], "premise: cascade");
    let (verdict, code) = verdict_of(&document["mode"], &rows);
    assert_eq!(
        (verdict, code),
        (text(&document["verdict"]), plain.code),
        "premise: the verdict\n{}",
        plain.show()
    );
    assert_eq!(
        delta_of(&document["delta"], &rows, &rows, None),
        document["delta"],
        "premise: delta"
    );
    assert_eq!(
        plain.text(),
        text_of(plain, |_| true, |_| true, "verify: ", &document),
        "premise: plain's text"
    );
}

/// TE-1's differential (RD-2): `--task`'s rows are plain's rows the set
/// keeps, each object whole and in order; `task`, the counts, cascade,
/// delta, verdict, exit, text and notes follow from them; `mode`, `base`
/// and `cannot` are plain's. The rows kept.
fn differential(plain: &Run, run: &Run, case: &Case<'_>) -> Vec<Value> {
    let context = format!("{} ({})", run.what, case.id);
    assert!(run.outcome.is_some(), "{context}: a report\n{}", run.show());
    let base = plain.json();
    let document = run.json();
    let kept: Vec<Value> = plain
        .rows()
        .into_iter()
        .filter(|row| case.keeps(row))
        .collect();
    assert_eq!(
        document["rows"],
        Value::Array(kept.clone()),
        "{context}: the rows are plain's kept to the set\n{}",
        run.show()
    );
    assert_eq!(top_keys(&run.json_text()), TOP_KEYS, "{context}: keys");
    assert_eq!(document["task"], case.id, "{context}: `task`");
    for key in ["node", "symbol", "omitted"] {
        assert_eq!(
            document[key],
            Value::Null,
            "{context}: `{key}` (verify-node)"
        );
    }
    for key in ["mode", "base", "cannot"] {
        assert_eq!(document[key], base[key], "{context}: `{key}` is plain's");
    }
    // docs/features/tested-evidence.md "Plain verify": `--task T`'s
    // `tested` is plain's entries of its set (`null` under `--changed`).
    let set_tested = base["tested"].as_array().map(|entries| {
        entries
            .iter()
            .filter(|entry| case.set.iter().any(|id| entry["node"] == *id))
            .cloned()
            .collect::<Vec<Value>>()
    });
    assert_eq!(
        document["tested"],
        json!(set_tested),
        "{context}: `tested` is plain's of the set"
    );
    let (verdict, code) = verdict_of(&document["mode"], &kept);
    assert_eq!(document["verdict"], verdict, "{context}: the verdict");
    assert_eq!(run.code, code, "{context}: the exit\n{}", run.show());
    assert_eq!(
        document["counts"],
        counts_of(&kept, &json!(0)),
        "{context}: counts over the kept rows, `skipped` 0"
    );
    assert_eq!(document["cascade"], cascade_of(&kept), "{context}: cascade");
    assert_eq!(
        document["delta"],
        delta_of(&base["delta"], &kept, &plain.rows(), Some(case)),
        "{context}: delta over the kept rows"
    );

    // Notes: plain's, and the set's (their place among plain's not pinned).
    let mut wanted: Vec<String> = plain.notes();
    wanted.extend(case.gone.iter().map(|reference| note_gone(reference)));
    let orphans = count(&plain.rows(), names_no_node);
    if orphans > 0 {
        wanted.push(note_no_node(orphans));
    }
    let named: Vec<&str> = kept
        .iter()
        .filter_map(|row| case.names(text(&row["node"])))
        .collect();
    let without = case.set.iter().filter(|id| !named.contains(id)).count();
    if without > 0 {
        wanted.push(note_no_binding(without, case.id));
    }
    let mut got = run.notes();
    got.sort();
    wanted.sort();
    assert_eq!(got, wanted, "{context}: notes");
    let stderr = run.stderr_lines();
    for note in &wanted {
        assert!(
            stderr.contains(&format!("note: {note}")),
            "{context}: `note: {note}` on stderr: {stderr:?}"
        );
    }

    assert_eq!(
        run.text(),
        text_of(
            plain,
            |row| case.keeps(row),
            |node| case.set.contains(&node),
            &format!("verify --task {}: ", case.id),
            &document
        ),
        "{context}: the text"
    );
    kept
}

/// Plain and each case's `--task` on one state under `tree`, each through
/// [`differential`]: the `--task` runs, in case order.
fn compare(repo: &LockRepo, slug: &str, tree: VerifiedTree, cases: &[Case<'_>]) -> Vec<Run> {
    let reference = plain(repo, tree);
    plain_premises(&reference);
    let mut runs = Vec::new();
    let mut narrower = false;
    for case in cases {
        let run = scoped(repo, slug, tree, case.id);
        let kept = differential(&reference, &run, case);
        narrower |= kept.len() < reference.rows().len();
        runs.push(run);
    }
    assert!(
        narrower,
        "premise: a set keeps fewer rows than plain ({tree:?})"
    );
    runs
}

// ----------------------------------------------------------------- edits

/// `verify_output.rs`'s `drift()` over the golden, repeated: `CAP` and the
/// RON cap edited, LAW-BOUND's text edited, a marker removed, a non-UTF-8
/// file (a note), `enforce-introduced` (a note when plain).
fn drift(repo: &LockRepo) {
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    repo.replace(BOUNDS, "//! @verifies PIECE-COUNTER\n", "");
    repo.write("crates/tally/src/odd.rs", b"// \xff\n".as_slice());
    set_mode(&repo.root, "enforce-introduced");
}

/// The four tasks of the golden: a document (its three sections), a
/// feature-scoped ID, a document without a binding, a planned task whose
/// criterion and affected node (a path) widen it.
fn golden_tasks(repo: &LockRepo) -> [String; 4] {
    let counter = new_task(repo, &["PIECE-COUNTER"]);
    let pass = new_task(repo, &["cap-check/PASS-01"]);
    let limits = new_task(repo, &["docs/spec/limits.md"]);
    let premise = new_task(repo, &["PREM-001"]);
    plan_task(repo, &premise, &["GOAL-01"], &["docs/records/PREM-001.md"]);
    [counter, pass, limits, premise]
}

const COUNTER_SET: [&str; 4] = ["PIECE-COUNTER", "LAW-BOUND", "LAW-STEP", "LAW-ODD"];
const PASS_SET: [&str; 1] = ["cap-check/PASS-01"];
const LIMITS_SET: [&str; 3] = ["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"];
const PREMISE_SET: [&str; 2] = ["PREM-001", "GOAL-01"];

// ------------------------------------------------------------------ RD-2

/// RD-2 / TE-1 (a), TE-4: over the golden with `drift()`'s edits
/// (`enforce-introduced`, Fix, Check and Cannot rows, three rows naming
/// no node), plain and `--changed`: each task's rows are plain's kept to
/// its set, whole and in order, and every other key follows from them;
/// nothing written. Exits 2 (a kept Cannot row), 1 (a kept Fix row under
/// plain `enforce-introduced`), 0 (a set without a row: `clean`, Q3's
/// note). M2: a row outside the set kept, or a set row dropped.
#[test]
fn verify_task_rows_equal_plain_rows_filtered() {
    let repo = golden("vt8-golden");
    let [counter, pass, limits, premise] = golden_tasks(&repo);
    drift(&repo);
    let cases = [
        Case::of(&counter, &COUNTER_SET),
        Case::of(&pass, &PASS_SET),
        Case::of(&limits, &LIMITS_SET),
        Case::of(&premise, &PREMISE_SET),
    ];
    let runs = compare(&repo, CODE_A, VerifiedTree::WorkingTree, &cases);
    let codes: Vec<i32> = runs.iter().map(|run| run.code).collect();
    assert_eq!(codes, [2, 1, 0, 2], "the scenario's exits");
    assert!(
        runs[2].rows().is_empty() && runs[2].json()["verdict"] == "clean",
        "{}",
        runs[2].show()
    );
    for run in &runs {
        assert!(run.notes().contains(&NO_BASE.to_owned()), "{}", run.show());
    }
    let changed = compare(&repo, CODE_A, VerifiedTree::Changed, &cases);
    assert!(
        changed
            .iter()
            .any(|run| run.rows().iter().any(|row| row["pre_existing"] == true)),
        "premise: --changed marks rows Pre-existing"
    );
}

/// TE-1 (b): over `fixtures/anchor-states`, its baseline (plain), then its
/// 18 scenarios applied together (plain and `--changed`): merged (moved)
/// rows, ambiguous groups, lost and lock-only rows, a `hidden_by_error`
/// Cannot row, an ID renamed to an alias (the task's target resolves to
/// the new ID; the old ID's `node_gone` entry is dropped and counted by
/// Q2's note). A document target by path. M2.
#[test]
fn verify_task_rows_equal_plain_rows_filtered_anchor_states() {
    let repo = anchor_common::base("vt8-anchors");
    let paired = new_task(&repo, &["KITE-STRAY", "KITE-CLONE", "KITE-CONTEST"]);
    let wisp = new_task(&repo, &["WISP-01"]);
    let rigging = new_task(&repo, &["docs/spec/rigging.md"]);
    let moved = new_task(&repo, &["KITE-RENAME", "KITE-TOOL", "KITE-FOG", "KITE-LAG"]);
    let paired_set = ["KITE-STRAY", "KITE-CLONE", "KITE-CONTEST"];
    let rigging_set = ["KITE-RIGGING", "KITE-RIG", "KITE-GAUGE", "KITE-TETHER"];
    let moved_set = ["KITE-RENAME", "KITE-TOOL", "KITE-FOG", "KITE-LAG"];

    compare(
        &repo,
        ANCHORS,
        VerifiedTree::WorkingTree,
        &[
            Case::of(&paired, &paired_set),
            Case::of(&wisp, &["WISP-01"]),
            Case::of(&rigging, &rigging_set),
            Case::of(&moved, &moved_set),
        ],
    );

    for name in anchor_common::SCENARIOS {
        anchor_common::change(&repo, name);
    }
    let cases = [
        Case::of(&paired, &paired_set),
        Case::of(&wisp, &["WISP-02"]),
        Case::of(&rigging, &rigging_set),
        Case::of(&moved, &moved_set),
    ];
    for tree in [VerifiedTree::WorkingTree, VerifiedTree::Changed] {
        let runs = compare(&repo, ANCHORS, tree, &cases);
        let rows = runs[0].rows();
        assert!(
            rows.iter().any(|row| row["anchor"]["state"] == "ambiguous"),
            "premise: an ambiguous group kept\n{}",
            runs[0].show()
        );
        assert!(
            rows.iter()
                .any(|row| row["category"] == "symbol_gone" && row["markers"] == json!([])),
            "premise: a lock-only row kept\n{}",
            runs[0].show()
        );
        let nodes: Vec<Value> = runs[1]
            .rows()
            .iter()
            .map(|row| row["node"].clone())
            .collect();
        assert_eq!(nodes, [json!("WISP-02")], "{}", runs[1].show());
        assert!(
            runs[1].notes().contains(&note_no_node(1)),
            "the old ID's entry: {:?}",
            runs[1].notes()
        );
        assert!(
            runs[3].summary().contains(" moved); "),
            "premise: moved rows kept\n{}",
            runs[3].show()
        );
        assert_eq!(runs[3].code, 2, "KITE-FOG's Cannot row\n{}", runs[3].show());
    }
}

/// TE-1 (c): over `fixtures/verify-tests` (no lock: marker rows): a
/// document target (`KILN-FIRING`, its sections), `CONE-01` whose
/// markers are one written as its legacy alias `PYRO-01` (a task names the
/// canonical ID: `task new` refuses the alias), a claimed task whose `plan` gave `glaze-check/GLZ-01` and
/// an affected node, a `draft` task whose ref is renamed away (its note,
/// its markers then dangling: Q2's note), read in every state from the
/// columns now. M2.
#[test]
fn verify_task_rows_equal_plain_rows_filtered_kiln() {
    let repo = LockRepo::of("vt8-kiln", "verify-tests", "repo", &[]);
    let firing = new_task(&repo, &["KILN-FIRING"]);
    let cone = new_task(&repo, &["CONE-01"]);
    let glaze = new_task(&repo, &["CONE-02"]);
    plan_task(
        &repo,
        &glaze,
        &["glaze-check/GLZ-01", "The glaze survives a full hold."],
        &["KILN-COOL"],
    );
    approve_task(&repo, &glaze);
    claim_task(&repo, &glaze);
    let hold = new_task(&repo, &["KILN-HOLD"]);
    let glaze_set = ["CONE-02", "glaze-check/GLZ-01", "KILN-COOL"];

    compare(
        &repo,
        KILN,
        VerifiedTree::WorkingTree,
        &[
            Case::of(
                &firing,
                &["KILN-FIRING", "KILN-RAMP", "KILN-HOLD", "KILN-COOL"],
            ),
            Case::of(&cone, &["CONE-01"]),
            Case::of(&glaze, &glaze_set),
            Case::of(&hold, &["KILN-HOLD"]),
        ],
    );

    repo.replace("docs/spec/kiln.md", "{#KILN-HOLD}", "{#KILN-DWELL}");
    repo.commit_all("the hold is a dwell now");
    let cases = [
        Case::of(
            &firing,
            &["KILN-FIRING", "KILN-RAMP", "KILN-DWELL", "KILN-COOL"],
        ),
        Case::of(&cone, &["CONE-01"]),
        Case::of(&glaze, &glaze_set),
        Case {
            gone: &["KILN-HOLD"],
            ..Case::of(&hold, &[])
        },
    ];
    let runs = compare(&repo, KILN, VerifiedTree::WorkingTree, &cases);
    assert_eq!(
        runs[1].rows().len(),
        2,
        "CONE-01's two rows\n{}",
        runs[1].show()
    );
    assert!(runs[3].rows().is_empty(), "{}", runs[3].show());
    assert!(
        runs[3].notes().contains(&note_no_node(2)),
        "the renamed node's markers: {:?}",
        runs[3].notes()
    );
}

/// TE-1 (d): over `verify_common::ecs` (an ECS game crate, a RON file)
/// with a body edited, a RON value edited and a marker raised past its
/// node, plain and `--changed`: a document's set, a RON-bound record, a
/// document without a binding, a planned task's criterion and affected
/// path. M2.
#[test]
fn verify_task_rows_equal_plain_rows_filtered_ecs() {
    let repo = ecs("vt8-ecs");
    let stamina = new_task(&repo, &["PERK-STAMINA"]);
    let tune = new_task(&repo, &["TUNE-01"]);
    let movement = new_task(&repo, &["docs/spec/movement.md"]);
    let delay = new_task(&repo, &["TUNE-02"]);
    plan_task(&repo, &delay, &["PERK-DRAIN"], &["docs/records/TUNE-03.md"]);
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(MOVEMENT, "regen_per_second: 2.5,", "regen_per_second: 3.0,");
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@2\n",
    );
    let cases = [
        Case::of(&stamina, &["PERK-STAMINA", "PERK-REGEN", "PERK-DRAIN"]),
        Case::of(&tune, &["TUNE-01"]),
        Case::of(
            &movement,
            &["PERK-MOVE", "PERK-SPRINT", "PERK-SPRINT-COST", "PERK-WALK"],
        ),
        Case::of(&delay, &["TUNE-02", "PERK-DRAIN", "TUNE-03"]),
    ];
    for tree in [VerifiedTree::WorkingTree, VerifiedTree::Changed] {
        let runs = compare(&repo, GAME, tree, &cases);
        assert!(
            runs[2].notes().contains(&note_no_binding(4, &movement)),
            "{:?}",
            runs[2].notes()
        );
    }
}

// ------------------------------------------------------------------ RD-3

/// RD-3, Q2: both halves of "a row in a set" over the golden: a lock-only
/// `symbol_gone` row (LAW-BOUND's Display impl removed), a lock-only
/// `marker_gone` row (PIECE-COUNTER's `//!` marker in the test file
/// removed; GOAL-01's left by a homoglyph), a homoglyph marker (a Cyrillic
/// O, keyed as written, kept under its Latin fix's node), an alias marker
/// (`AIM-01`, keyed GOAL-01); an agent's typo (`LAW-BUOND`) and the
/// golden's dangling and bare markers never kept, counted by Q2's note in
/// every task. M3: the entry half dropped, or the marker half matched by
/// exact ID.
#[test]
fn verify_task_keeps_lock_only_and_look_alike_rows() {
    let repo = golden("vt8-halves");
    let goal = new_task(&repo, &["GOAL-01"]);
    let bound = new_task(&repo, &["LAW-BOUND"]);
    let counter = new_task(&repo, &["PIECE-COUNTER"]);
    let look_alike = "G\u{41E}AL-01";
    repo.replace(
        COUNTER,
        "@implements AIM-01@1",
        &format!("@implements {look_alike}@1"),
    );
    repo.replace(
        COUNTER,
        "impl fmt::Display for Tally {\n    // @implements PIECE-COUNTER#LAW-BOUND@2\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        write!(f, \"{}\", self.count)\n    }\n}\n",
        "",
    );
    repo.replace(BOUNDS, "//! @verifies PIECE-COUNTER\n", "");
    repo.replace(
        COUNTER,
        "pub fn total(",
        "// @implements LAW-BUOND\npub fn total(",
    );
    repo.append(
        COUNTER,
        "\n// @implements AIM-01\npub fn spare() -> u32 {\n    1\n}\n",
    );

    let reference = plain(&repo, VerifiedTree::WorkingTree);
    let typo = reference
        .rows()
        .into_iter()
        .filter(|row| row["node"] == "LAW-BUOND")
        .count();
    assert_eq!(typo, 1, "premise: the typo's row\n{}", reference.show());
    let spelled = [(look_alike, "GOAL-01")];
    let cases = [
        Case {
            spelled: &spelled,
            ..Case::of(&goal, &["GOAL-01"])
        },
        Case::of(&bound, &["LAW-BOUND"]),
        Case::of(&counter, &COUNTER_SET),
    ];
    let runs = compare(&repo, CODE_A, VerifiedTree::WorkingTree, &cases);
    let row = |run: &Run, node: &str, relation: &str, file: &str, symbol: &str| -> Value {
        let found: Vec<Value> = run
            .rows()
            .into_iter()
            .filter(|row| {
                row["node"] == node
                    && row["relation"] == relation
                    && row["file"] == file
                    && row["symbol"] == symbol
            })
            .collect();
        assert_eq!(found.len(), 1, "one row {node} {symbol}\n{}", run.show());
        found[0].clone()
    };
    let state = |row: &Value| {
        (
            text(&row["state"]).to_owned(),
            text(&row["category"]).to_owned(),
        )
    };
    let lock_only = |row: &Value| row["markers"] == json!([]) && !row["lock"].is_null();

    let homoglyph = row(
        &runs[0],
        look_alike,
        "implements",
        COUNTER,
        "crates/tally::counter::Tally::value",
    );
    assert_eq!(state(&homoglyph), ("bad_marker".into(), "homoglyph".into()));
    let left = row(
        &runs[0],
        "GOAL-01",
        "implements",
        COUNTER,
        "crates/tally::counter::Tally::value",
    );
    assert_eq!(
        state(&left),
        ("broken_binding".into(), "marker_gone".into())
    );
    assert!(lock_only(&left), "{left}");
    let alias = row(
        &runs[0],
        "GOAL-01",
        "implements",
        COUNTER,
        "crates/tally::counter::spare",
    );
    assert_eq!(state(&alias), ("unlocked".into(), String::new()));

    let gone = row(
        &runs[1],
        "LAW-BOUND",
        "implements",
        COUNTER,
        "crates/tally::counter::<Tally as fmt::Display>::fmt",
    );
    assert_eq!(
        state(&gone),
        ("broken_binding".into(), "symbol_gone".into())
    );
    assert!(lock_only(&gone), "{gone}");

    let module = row(
        &runs[2],
        "PIECE-COUNTER",
        "verifies",
        BOUNDS,
        "crates/tally::test:bounds",
    );
    assert_eq!(
        state(&module),
        ("broken_binding".into(), "marker_gone".into())
    );
    assert!(lock_only(&module), "{module}");

    // The golden's bare `@implements`, `GOAL-99`, `ZZZ-1`, and the typo.
    for run in &runs {
        assert!(
            run.notes().contains(&note_no_node(4)),
            "{}: {:?}",
            run.what,
            run.notes()
        );
        assert!(
            run.rows().iter().all(|row| row["node"] != "LAW-BUOND"),
            "{}",
            run.show()
        );
    }
}

// ------------------------------------------------------------------ RD-4

/// RD-4, Q3: over `verify_common::ecs` under `enforce`, a Fix row outside
/// the set (TUNE-01's RON marker raised past its record), a Check row in
/// it (PERK-DRAIN's body edited) and a marker of another project
/// (skipped): plain is blocked (exit 1, `skipped` 1, both nodes affected);
/// `--task` over PERK-STAMINA's document is observed (exit 0, `skipped` 0,
/// `affected_nodes` its own, the `verify --task <T>: ` prefix); a set with
/// no binding is `clean` with Q3's note. A run cause (a bad `[verify]
/// mode`, a lock not UTF-8) stays the whole run's: exit 2, plain's
/// `cannot:` lines, `rows` `[]`. M4: the filter after the verdict, or
/// `skipped` kept.
#[test]
fn verify_task_counts_its_rows_only() {
    let repo = ecs("vt8-counts");
    let stamina = new_task(&repo, &["PERK-STAMINA"]);
    let movement = new_task(&repo, &["docs/spec/movement.md"]);
    repo.replace(
        MOVEMENT,
        "// @configures TUNE-01@1",
        "// @configures TUNE-01@9",
    );
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(
        ECS_LIB,
        "//! @implements PERK-STAMINA@2\n",
        "//! @implements PERK-STAMINA@2\n// @implements other:PERK-STAMINA\n",
    );
    set_mode(&repo.root, "enforce");
    let stamina_set = ["PERK-STAMINA", "PERK-REGEN", "PERK-DRAIN"];
    let movement_set = ["PERK-MOVE", "PERK-SPRINT", "PERK-SPRINT-COST", "PERK-WALK"];
    let cases = [
        Case::of(&stamina, &stamina_set),
        Case::of(&movement, &movement_set),
    ];
    let runs = compare(&repo, GAME, VerifiedTree::WorkingTree, &cases);

    let whole = plain(&repo, VerifiedTree::WorkingTree);
    whole.answered(1);
    let document = whole.json();
    assert_eq!(document["verdict"], "blocked");
    assert_eq!(
        document["counts"]["skipped"], 1,
        "premise: a skipped marker"
    );
    assert_eq!(
        document["delta"]["affected_nodes"],
        json!(["PERK-DRAIN", "TUNE-01"]),
        "premise"
    );

    let own = runs[0].answered(0).json();
    assert_eq!(own["verdict"], "observed", "{}", runs[0].show());
    assert_eq!(own["counts"]["skipped"], 0);
    assert_eq!(own["counts"]["fix"], 0);
    assert_eq!(own["counts"]["check"], 1);
    assert_eq!(own["delta"]["affected_nodes"], json!(["PERK-DRAIN"]));
    let summary = runs[0].summary();
    let prefix =
        format!("verify --task {stamina}: observed [enforce]: 0 fix, 1 check, 0 cannot, 3 ok; ");
    assert!(
        summary.starts_with(&prefix) && summary.ends_with("; 0 skipped"),
        "{summary:?}"
    );
    assert!(
        !runs[0]
            .notes()
            .iter()
            .any(|note| note.ends_with("have no binding")),
        "every node of the set is bound: {:?}",
        runs[0].notes()
    );

    let none = runs[1].answered(0).json();
    assert_eq!(none["verdict"], "clean", "{}", runs[1].show());
    assert_eq!(none["rows"], json!([]));
    assert!(
        runs[1]
            .stderr_lines()
            .contains(&format!("note: {}", note_no_binding(4, &movement))),
        "{:?}",
        runs[1].stderr_lines()
    );

    // Run causes: never filtered.
    repo.replace("specengine.toml", "mode = \"enforce\"", "mode = \"strict\"");
    run_cause_is_plain(&repo, &stamina, "a bad [verify] mode");
    repo.replace("specengine.toml", "mode = \"strict\"", "mode = \"enforce\"");
    repo.write("spec.lock", b"\xff\xfe".as_slice());
    run_cause_is_plain(&repo, &stamina, "a lock not UTF-8");
}

/// A run cause's report under `--task` is plain's: exit 2, `rows` `[]`,
/// the same `cannot` lines and the same text but the summary's prefix.
fn run_cause_is_plain(repo: &LockRepo, task: &str, context: &str) {
    let whole = plain(repo, VerifiedTree::WorkingTree);
    whole.answered(2);
    let base = whole.json();
    assert_ne!(
        base["cannot"],
        json!([]),
        "premise ({context}): a run cause\n{}",
        whole.show()
    );
    let run = scoped(repo, GAME, VerifiedTree::WorkingTree, task);
    run.answered(2);
    let document = run.json();
    assert_eq!(
        document["cannot"],
        base["cannot"],
        "{context}\n{}",
        run.show()
    );
    assert_eq!(document["rows"], json!([]), "{context}");
    assert_eq!(document["verdict"], "cannot-verify", "{context}");
    assert_eq!(document["task"], task, "{context}");
    assert_eq!(
        document["counts"]["skipped"], 0,
        "{context}: \"Counts\": `skipped` 0 (plain's {})",
        base["counts"]["skipped"]
    );
    let lines = |run: &Run| -> Vec<String> {
        let mut lines: Vec<String> = run.text().lines().map(str::to_owned).collect();
        lines.pop();
        lines
    };
    assert_eq!(
        lines(&run),
        lines(&whole),
        "{context}: plain's `cannot:` lines"
    );
    assert!(
        lines(&run).iter().any(|line| line.starts_with("cannot: ")),
        "{context}\n{}",
        run.show()
    );
    assert_eq!(
        run.summary(),
        format!("verify --task {task}: {}", summary_of(&document)),
        "{context}"
    );
}

// ------------------------------------------------------------------ RD-5

/// RD-5 / TE-2: under `enforce-introduced` committed, PERK-DRAIN's marker
/// raised past its node in `HEAD` (a Pre-existing Fix row of the task's
/// node), TUNE-01's raised in the worktree only (an introduced Fix row
/// outside the set). Plain `--task` is judged as `enforce` with plain's
/// note: blocked, exit 1. `--task --changed`: `base` `HEAD`, the row
/// `pre_existing` true, observed, exit 0, while plain `--changed` stays
/// blocked. M5: `--changed` ignored under `--task`.
#[test]
fn verify_task_changed_marks_pre_existing() {
    let repo = ecs("vt8-changed");
    let drain = new_task(&repo, &["PERK-DRAIN"]);
    repo.replace(
        STAMINA,
        "// @implements PERK-DRAIN\n",
        "// @implements PERK-DRAIN@9\n",
    );
    set_mode(&repo.root, "enforce-introduced");
    repo.commit_all("a marker past its node, enforce-introduced");
    repo.replace(
        MOVEMENT,
        "// @configures TUNE-01@1",
        "// @configures TUNE-01@9",
    );
    let cases = [Case::of(&drain, &["PERK-DRAIN"])];

    let runs = compare(&repo, GAME, VerifiedTree::WorkingTree, &cases);
    let blocked = runs[0].answered(1).json();
    assert_eq!(blocked["verdict"], "blocked");
    assert_eq!(blocked["mode"], "enforce");
    assert_eq!(blocked["base"], Value::Null);
    assert!(
        runs[0].notes().contains(&NO_BASE.to_owned()),
        "{}",
        runs[0].show()
    );
    let predated: Vec<Value> = runs[0]
        .rows()
        .into_iter()
        .filter(|row| row["state"] == "predated")
        .collect();
    assert_eq!(predated.len(), 1, "{}", runs[0].show());
    assert_eq!(predated[0]["pre_existing"], false);

    let runs = compare(&repo, GAME, VerifiedTree::Changed, &cases);
    let document = runs[0].answered(0).json();
    assert_eq!(document["verdict"], "observed", "{}", runs[0].show());
    assert_eq!(document["mode"], "enforce-introduced");
    assert_eq!(document["base"], "HEAD");
    let predated: Vec<Value> = runs[0]
        .rows()
        .into_iter()
        .filter(|row| row["state"] == "predated")
        .collect();
    assert_eq!(predated.len(), 1, "{}", runs[0].show());
    assert_eq!(predated[0]["pre_existing"], true);
    assert!(
        runs[0].summary().contains(" (1 pre-existing), "),
        "{}",
        runs[0].summary()
    );
    plain(&repo, VerifiedTree::Changed).answered(1);
}

// ------------------------------------------------------------------ RD-6

/// RD-6, Q1: before `T` resolves, every failure exits 2 with stdout
/// empty, `--json` too, the error on stderr, and the queue unchanged:
/// `T` unknown, a look-alike (its Latin form named), not a task ID,
/// another repository's (the same `HOME` and slug), `T` in `[ids]`; a
/// `--config` other than the root's own; no `HOME`; a root in no
/// worktree; a `HOME` without the queue (made, then `T` unknown). Plain
/// verify reports the last three as run causes on stdout. `--config`
/// naming the root's own file runs. M6: an unknown or another
/// repository's `T` at exit 1, or run as plain.
#[test]
fn verify_task_unknown_is_exit_2() {
    let repo = ecs("vt8-unknown");
    let own = new_task(&repo, &["PERK-DRAIN"]);
    assert_eq!(own, "T-0001", "premise: the first task");
    let other = repo.scratch.join("other");
    copy_dir(&repo.root, &other);
    let other = fs::canonicalize(&other).expect("the other repository");
    let theirs = new_task_in(&repo, &other, &["PERK-DRAIN"]);
    let before = queue_state(&repo, GAME);

    let refused = |task: &str, words: &str| {
        let run = verify_at(&repo, &repo.root, VerifiedTree::WorkingTree, Some(task));
        let message = run.failed();
        assert!(
            message.contains(words),
            "{task}: {message:?} names {words:?}"
        );
        let changed = verify_at(&repo, &repo.root, VerifiedTree::Changed, Some(task));
        assert_eq!(changed.failed(), message, "{task} --changed");
        binary_refuses(&repo, &["verify", "--task", task, "--json"], words);
    };
    refused("T-0099", "no task T-0099 in this repository");
    refused("\u{0422}-0001", "T-0001");
    refused("ramp", "no task `ramp`");
    refused(&theirs, "belongs to another repository");

    // `--config`: only the root's own file.
    fs::write(repo.root.join("other.toml"), repo.read("specengine.toml")).expect("a second config");
    let elsewhere = Globals {
        root: None,
        config: Some(PathBuf::from("other.toml")),
    };
    let run = verify_with(
        &repo,
        &repo.env(&repo.root),
        &elsewhere,
        VerifiedTree::WorkingTree,
        Some(&own),
    );
    let message = run.failed();
    assert!(message.contains("--config other.toml"), "{message:?}");
    binary_refuses(
        &repo,
        &["verify", "--config", "other.toml", "--task", &own, "--json"],
        "--config",
    );
    let own_config = Globals {
        root: None,
        config: Some(PathBuf::from("specengine.toml")),
    };
    let named = verify_with(
        &repo,
        &repo.env(&repo.root),
        &own_config,
        VerifiedTree::WorkingTree,
        Some(&own),
    );
    let unnamed = verify_at(&repo, &repo.root, VerifiedTree::WorkingTree, Some(&own));
    unnamed.answered(0);
    assert_eq!(
        named.bytes(),
        unnamed.bytes(),
        "--config naming the root's own file"
    );
    fs::remove_file(repo.root.join("other.toml")).expect("the second config removed");

    // No `HOME`: plain reports cause 8; `--task` cannot resolve `T`.
    let homeless = Env {
        cwd: repo.root.clone(),
        home: None,
        xdg_data_home: None,
    };
    verify_with(
        &repo,
        &homeless,
        &Globals::default(),
        VerifiedTree::WorkingTree,
        None,
    )
    .answered(2);
    let run = verify_with(
        &repo,
        &homeless,
        &Globals::default(),
        VerifiedTree::WorkingTree,
        Some(&own),
    );
    run.failed();
    let vars: Vec<(OsString, OsString)> = repo
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME" && name != "XDG_DATA_HOME")
        .collect();
    let binary = spawn_spec(&repo.root, &["verify", "--task", &own, "--json"], vars, b"");
    assert_eq!(binary.code, 2, "no HOME\n{}", binary.show());
    assert_eq!(binary.stdout, "", "no HOME: stdout empty");
    assert!(!binary.stderr.is_empty(), "no HOME: the error on stderr");

    // A root in no worktree: plain reports cause 4.
    let loose = repo.scratch.join("loose");
    copy_dir(&repo.root, &loose);
    fs::remove_dir_all(loose.join(".git")).expect("no .git");
    let loose = fs::canonicalize(&loose).expect("the loose copy");
    verify_at(&repo, &loose, VerifiedTree::WorkingTree, None).answered(2);
    verify_at(&repo, &loose, VerifiedTree::WorkingTree, Some(&own)).failed();

    // A `HOME` without the queue: made, `T` unknown there.
    let fresh = Env {
        cwd: repo.root.clone(),
        home: Some(repo.scratch.home("fresh").into_os_string()),
        xdg_data_home: None,
    };
    let message = verify_with(
        &repo,
        &fresh,
        &Globals::default(),
        VerifiedTree::WorkingTree,
        Some(&own),
    )
    .failed();
    assert!(
        message.contains("no task T-0001 in this repository"),
        "{message:?}"
    );

    assert_eq!(queue_state(&repo, GAME), before, "the queue unchanged");

    // `T` in `[ids]`, last: the config changes.
    let config = repo.read("specengine.toml");
    repo.write(
        "specengine.toml",
        common::with_ids_line(&config, "T = { kind = \"trial-run\", width = 4 }\n"),
    );
    repo.commit_all("a T prefix");
    let run = verify_at(&repo, &repo.root, VerifiedTree::WorkingTree, Some(&own));
    let message = run.failed();
    assert!(message.contains("specengine.toml"), "{message:?}");
    binary_refuses(
        &repo,
        &["verify", "--task", &own, "--json"],
        "specengine.toml",
    );
    assert_eq!(queue_state(&repo, GAME), before, "the queue unchanged");
}

/// `spec args` from the root (stdin a pipe, closed empty): exit 2, stdout
/// empty, stderr naming `words`.
fn binary_refuses(repo: &LockRepo, args: &[&str], words: &str) {
    let run = spawn_spec(&repo.root, args, spec_vars(repo), b"");
    assert_eq!(run.code, 2, "{args:?}\n{}", run.show());
    assert_eq!(run.stdout, "", "{args:?}: stdout empty");
    assert!(
        run.stderr.contains(words),
        "{args:?}: stderr names {words:?}:\n{}",
        run.stderr
    );
}

// ------------------------------------------------------------------ RD-1

/// RD-1 (P1): `--task` with `--staged`, either order, `--json` too: clap's
/// conflict, exit 2, stdout empty, stderr naming `--task` and `--staged`
/// (the library refuses it too). Plain `--task` and `--task --changed`,
/// stdin a pipe holding `y` (an agent's shell): no question, exit 0, the
/// report on stdout. Replaces 8a-1's `verify_task_without_tests_is_usage`.
/// M1: `--task --staged` accepted.
#[test]
fn verify_task_with_staged_is_usage() {
    let repo = ecs("vt8-usage");
    let task = new_task(&repo, &["PERK-DRAIN"]);
    let before = Untouched::take(&repo, GAME);
    for args in [
        &["verify", "--task", task.as_str(), "--staged"][..],
        &["verify", "--staged", "--task", task.as_str()][..],
        &["verify", "--task", task.as_str(), "--staged", "--json"][..],
        &["verify", "--staged", "--json", "--task", task.as_str()][..],
    ] {
        let run = spawn_spec(&repo.root, args, spec_vars(&repo), b"y\n");
        assert_eq!(run.code, 2, "{args:?}\n{}", run.show());
        assert_eq!(run.stdout, "", "{args:?}: stdout empty");
        assert!(
            run.stderr.contains("--task") && run.stderr.contains("--staged"),
            "{args:?}: the usage error names both flags:\n{}",
            run.stderr
        );
    }
    let staged = verify_at(&repo, &repo.root, VerifiedTree::Staged, Some(&task));
    staged.failed();
    before.still(&repo, GAME, "--task --staged");

    for (args, tree) in [
        (
            &["verify", "--task", task.as_str()][..],
            VerifiedTree::WorkingTree,
        ),
        (
            &["verify", "--task", task.as_str(), "--changed"][..],
            VerifiedTree::Changed,
        ),
    ] {
        let run = spawn_spec(&repo.root, args, spec_vars(&repo), b"y\nyes\n");
        assert_eq!(run.code, 0, "{args:?}\n{}", run.show());
        assert!(
            !run.stderr.contains("[y/N]"),
            "{args:?}: no question:\n{}",
            run.stderr
        );
        let library = verify_at(&repo, &repo.root, tree, Some(&task));
        assert_eq!(run.stdout, library.text(), "{args:?}: the report");
        assert!(
            run.stdout
                .lines()
                .last()
                .is_some_and(|line| line.starts_with(&format!("verify --task {task}: clean ["))),
            "{args:?}:\n{}",
            run.stdout
        );
        let mut json_args = args.to_vec();
        json_args.push("--json");
        let json = spawn_spec(&repo.root, &json_args, spec_vars(&repo), b"y\n");
        assert_eq!(json.code, 0, "{json_args:?}\n{}", json.show());
        assert_eq!(json.stdout, library.json_text(), "{json_args:?}");
        assert_eq!(top_keys(&json.stdout), TOP_KEYS, "{json_args:?}");
        before.still(&repo, GAME, &format!("{args:?}"));
    }
}

// ------------------------------------------------------------------ TE-3

/// TE-3: the golden with `drift()`'s edits and the same tasks made in
/// three repositories, one `HOME` each (clones share a slug's queue):
/// `<scratch>/repo`, `<scratch>/elsewhere/deeper`, and one whose files
/// were written in reverse order; from the root and a subdirectory; plain
/// and `--changed`: every task's exit, text, JSON and stderr byte-identical,
/// a repeat too, and no absolute path printed.
#[test]
fn verify_task_is_byte_identical_across_roots_and_order() {
    let first = golden("vt8-det-a");
    let second = LockRepo::of("vt8-det-b", "code-a", "elsewhere/deeper", &[]);
    lock_it(&second);
    let third = LockRepo::uncommitted("vt8-det-c");
    fs::remove_dir_all(&third.root).expect("clear");
    let mut files: Vec<_> = snapshot(&fixture("code-a"))
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && path != "expected.json")
        .collect();
    files.reverse();
    for (path, bytes) in files {
        third.write(&path, bytes.expect("a file"));
    }
    third.git.init(&third.root);
    third.commit_all("the fixture");
    lock_it(&third);
    let tasks = golden_tasks(&first);
    for repo in [&second, &third] {
        assert_eq!(repo.head(), first.head(), "the same commits");
        assert_eq!(golden_tasks(repo), tasks, "the same task IDs");
    }
    for repo in [&first, &second, &third] {
        drift(repo);
    }

    for tree in [VerifiedTree::WorkingTree, VerifiedTree::Changed] {
        for task in &tasks {
            let reference = scoped(&first, CODE_A, tree, task);
            assert!(reference.outcome.is_some(), "{}", reference.show());
            let runs = [
                ("a repeat run", scoped(&first, CODE_A, tree, task)),
                ("another root", scoped(&second, CODE_A, tree, task)),
                (
                    "another cwd",
                    verify_at(
                        &second,
                        &second.root.join("crates").join("tally"),
                        tree,
                        Some(task),
                    ),
                ),
                ("reverse creation order", scoped(&third, CODE_A, tree, task)),
            ];
            for (context, run) in runs {
                assert_eq!(run.bytes(), reference.bytes(), "{context} {tree:?} {task}");
            }
            for repo in [&first, &second, &third] {
                let root = repo.root.display().to_string();
                let (_, text, json, stderr) = reference.bytes();
                assert!(
                    !text.contains(&root)
                        && !json.contains(&root)
                        && !stderr.join("\n").contains(&root),
                    "no absolute path printed"
                );
            }
        }
    }
}

/// The golden lock committed over `repo` as `the lock`.
fn lock_it(repo: &LockRepo) {
    repo.write(
        "spec.lock",
        fs::read(golden_file()).expect("the golden lock"),
    );
    repo.commit_all("the lock");
}

// ---------------------------------------------------------------- binary

/// The variables a `spec` process gets: the sandbox's, this `HOME`.
fn spec_vars(repo: &LockRepo) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = repo
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), repo.home.clone().into_os_string()));
    vars
}

/// `spec args` in `cwd` with exactly `vars`, stdin a pipe holding `input`
/// (then closed), with a watchdog.
fn spawn_spec(
    cwd: &Path,
    args: &[&str],
    vars: Vec<(OsString, OsString)>,
    input: &[u8],
) -> common::Run {
    let mut child = Command::new(SPEC)
        .env_clear()
        .envs(vars)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn spec");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        // The child may exit before reading: a broken pipe is fine.
        let _ = stdin.write_all(input);
    }
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on spec") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec {args:?} ran longer than {RUN_TIMEOUT:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    common::Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(out.join().expect("stdout reader")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr reader")).expect("UTF-8 stderr"),
    }
}

// ------------------------------------------- tested-evidence (8b-1) RD-4

/// RD-4 ("Plain verify": `--task T: its set's`; M: every block listed
/// under a task, or a set node's block missing): over the ECS corpus with
/// a block per state (`tested_common::ecs_with_blocks`), PERK-STAMINA's
/// task (its sections PERK-REGEN and PERK-DRAIN in its set) lists the
/// three blocks of its set, plain's entries, never GONE-01's; TUNE-01's
/// task `[]`; under `--changed` `null`; the verdict, counts and exit as
/// the differential derives them from plain's rows.
#[test]
fn task_tested_is_its_sets() {
    let (repo, _, _) = tested_common::ecs_with_blocks("vt8-tested-sets");
    let stamina = new_task(&repo, &["PERK-STAMINA"]);
    let tune = new_task(&repo, &["TUNE-01"]);
    let cases = [
        Case::of(&stamina, &["PERK-STAMINA", "PERK-REGEN", "PERK-DRAIN"]),
        Case::of(&tune, &["TUNE-01"]),
    ];
    let reference = plain(&repo, VerifiedTree::WorkingTree);
    assert_eq!(list(&reference.json()["tested"]).len(), 4);
    let runs = compare(&repo, GAME, VerifiedTree::WorkingTree, &cases);
    let nodes: Vec<Value> = list(&runs[0].json()["tested"])
        .iter()
        .map(|entry| entry["node"].clone())
        .collect();
    assert_eq!(
        nodes,
        [
            json!("PERK-DRAIN"),
            json!("PERK-REGEN"),
            json!("PERK-STAMINA")
        ]
    );
    assert_eq!(runs[1].json()["tested"], json!([]));
    let changed = compare(&repo, GAME, VerifiedTree::Changed, &cases);
    for run in &changed {
        assert_eq!(run.json()["tested"], Value::Null, "--changed: null");
    }
}
