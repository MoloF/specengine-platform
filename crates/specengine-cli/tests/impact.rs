//! docs/features/impact.md (7b-1): `spec impact REF | --symbol QPATH
//! [--depth N] [--json]`, the blast radius of a node or a symbol: the nodes
//! `spec graph --impact`'s walk reaches, the code bound to them (each row
//! 7a's, its `sync` as `spec verify --node` judges it), the tests to run
//! and the open tasks to warn. RD-1 to RD-7 (each test names the mutation
//! that turns it red), TE-1 (`fixtures/impact/expected.json`), TE-2 (the
//! differential over `code-a`, `anchor-states`, `verify-tests`,
//! `verify-ecs`), TE-3 (determinism), TE-5 (`fixtures/spec-c`) and TE-7's
//! writes. TE-4's cost: `impact_cost.rs`; TE-7's pilots: `impact_pilot.rs`.
//!
//! **Oracles.** The nodes and `left_out` are `spec graph REF --impact
//! --json`'s (Q: `nodes` and `left_out` equal graph's); the rows are the
//! union of `spec show <node> --bindings --json`'s over every reached node
//! (each lists its own rows and its sections'), split on `verifies`, each
//! row's judged part `spec verify --node`'s (`bindings_common::Oracle`) in
//! plain verify's order; `--symbol`'s matches are `spec symbols QUERY
//! --json`'s best non-empty tier. The fixture's hand-made oracle
//! (`expected.json`, built from the spec) names each form's nodes, rows,
//! seeds and tasks.
//!
//! **Text.** The line shapes of "Text" are this file's [`node_line`],
//! [`row_text`], [`task_line`], [`summary_line`], [`seed_header`] and
//! [`seed_line`];
//! `impact_text_prints_its_json` holds them against the JSON.
//!
//! Every repository is a scratch git copy with its own `HOME`; tasks move
//! through the CLI library (`task_new` and the owner's decisions, consent
//! yes), the states this build never enters set through the system's
//! `sqlite3`. Non-Latin characters in Rust sources are Unicode escapes
//! (ADR-0024).

#![cfg(unix)]

mod anchor_common;
mod bindings_common;
mod code_index_common;
mod common;
mod lock_common;
mod verify_common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use bindings_common::*;
use common::git::Sandbox;
use common::{copy_dir, data_dir, fixture, snapshot};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_cli::{
    CliError, Env, Globals, ProposedText, TaskClaimRequest, TaskCompleteRequest,
    TaskDecisionRequest, TaskNewRequest, TaskOutcome, TaskPlanRequest, TaskReportRequest,
    task_approve, task_cancel, task_changes, task_claim, task_complete, task_new, task_plan,
    task_report,
};
use specengine_store::{GitEnv, ProposalQueue as _, SqliteQueue};
use verify_common::{clone_dir, ecs, golden};

/// The fixture and its slug (the index's and the queue's database name).
const FIXTURE: &str = "impact";
const SLUG: &str = "ferry-lines";

/// docs/features/impact.md "Command" (A9): the usage line of not exactly
/// one input, stdout empty, exit 2;
/// docs/features/impact-since.md "Command and guard" adds `--since` to it.
const ONE_INPUT: &str = "spec: impact takes exactly one of REF, --symbol and --since";

/// `code-bindings.md` "The `[code]` table": the sentence of a config
/// without the table (`spec refs`' exit-2 line, without `spec: `).
const NO_CODE: &str =
    "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The task commands' clock.
const TASK_CLOCK: &str = "2026-10-21T09:00:00Z";

/// The look-alike of `ROUTE-CROSSING` (a Cyrillic O).
const CROSSING_LOOK_ALIKE: &str = "R\u{41E}UTE-CROSSING";

/// The top-level keys of the JSON document, in order (A5: `since`,
/// `revision` and `delta` present, null): docs/features/impact.md "JSON"
const TOP_KEYS: [&str; 14] = [
    "input",
    "reason",
    "revision",
    "depth",
    "seeds",
    "delta",
    "left_out",
    "nodes",
    "symbols",
    "tests",
    "tasks",
    "notes",
    "truncated",
    "code",
];

/// graph's node object (`spec-cli-graph.md` "spec graph").
const NODE_KEYS: [&str; 7] = [
    "id", "kind", "title", "path", "line", "distance", "archived",
];

/// A seed match (docs/features/impact.md "JSON").
const MATCH_KEYS: [&str; 6] = [
    "qpath",
    "item_kind",
    "path",
    "start_line",
    "end_line",
    "tier",
];

/// A task of the radius.
const TASK_KEYS: [&str; 4] = ["id", "status", "title", "nodes"];

/// `code`, as `spec refs`' (`spec-cli-refs.md`).
const CODE_KEYS: [&str; 4] = ["files", "rs", "ron", "skipped"];

// ------------------------------------------------------------- the oracle

/// `fixtures/impact/expected.json`.
fn expected() -> Value {
    serde_json::from_str(
        &fs::read_to_string(fixture(FIXTURE).join("expected.json")).expect("expected.json"),
    )
    .expect("expected.json is JSON")
}

/// The forms of `expected.json`.
fn forms() -> Vec<Value> {
    list(&expected()["forms"])
}

/// A form's arguments after `spec impact`.
fn args_of(form: &Value) -> Vec<String> {
    list(&form["args"])
        .iter()
        .map(|arg| text(arg).to_owned())
        .collect()
}

/// The named rows of `expected.json`, each its six key fields.
fn named_rows() -> BTreeMap<String, Value> {
    expected()["rows"]
        .as_object()
        .expect("rows")
        .iter()
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect()
}

/// The keys (`key_of`) of the rows `names` names, sorted.
fn keys_named(names: &Value) -> Vec<String> {
    let rows = named_rows();
    let mut keys: Vec<String> = list(names)
        .iter()
        .map(|name| key_of(&rows[text(name)]))
        .collect();
    keys.sort();
    keys
}

/// The task table of `expected.json` by ID.
fn task_table() -> BTreeMap<String, Value> {
    list(&expected()["tasks"])
        .into_iter()
        .map(|task| (text(&task["id"]).to_owned(), task))
        .collect()
}

/// A form's tasks as the JSON must hold them: `{id, status, title, nodes}`.
fn tasks_of_form(form: &Value) -> Value {
    let table = task_table();
    Value::Array(
        list(&form["tasks"])
            .iter()
            .map(|pair| {
                let id = text(&pair[0]);
                let task = &table[id];
                json!({
                    "id": id,
                    "status": task["status"],
                    "title": task["title"],
                    "nodes": pair[1],
                })
            })
            .collect(),
    )
}

/// A graph node's name: its ID, else its path.
fn name_of(node: &Value) -> String {
    node["id"]
        .as_str()
        .unwrap_or_else(|| text(&node["path"]))
        .to_owned()
}

/// `[name, distance]` of each node of a document.
fn walked(document: &Value) -> Value {
    Value::Array(
        list(&document["nodes"])
            .iter()
            .map(|node| json!([name_of(node), node["distance"]]))
            .collect(),
    )
}

/// The REF of a reached node: its ID (a feature-scoped one as
/// `<feature>/<ID>`), else its path.
fn node_ref(root: &Path, node: &Value) -> String {
    match node["id"].as_str() {
        Some(id) => reference_of(root, text(&node["path"]), id),
        None => text(&node["path"]).to_owned(),
    }
}

/// `spec graph REF --impact [--depth N] --json` from `cwd`: its document
/// (exit 0 or 1).
fn graph_impact(
    cwd: &Path,
    reference: &str,
    depth: Option<&str>,
    vars: &[(OsString, OsString)],
) -> Value {
    let mut args = vec!["graph", reference, "--impact"];
    if let Some(depth) = depth {
        args.extend(["--depth", depth]);
    }
    args.push("--json");
    let run = run_with(cwd, &args, vars);
    assert!(
        [0, 1].contains(&run.code),
        "premise: graph {reference}\n{}",
        run.show()
    );
    common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("graph {reference}: {problem}\n{}", run.show()))
}

/// `spec symbols QUERY --limit 200 --json`'s matches at their best
/// non-empty tier, as impact's seed matches.
fn best_tier(cwd: &Path, query: &str, vars: &[(OsString, OsString)]) -> Value {
    let run = run_with(cwd, &["symbols", query, "--limit", "200", "--json"], vars);
    assert_eq!(run.code, 0, "premise: symbols {query}\n{}", run.show());
    let document = common::one_json_document(&run.stdout).expect("symbols' JSON");
    let all = list(&document["symbols"]);
    let Some(tier) = all.first().map(|hit| hit["tier"].clone()) else {
        return json!([]);
    };
    Value::Array(
        all.iter()
            .filter(|hit| hit["tier"] == tier)
            .map(|hit| {
                json!({
                    "qpath": hit["qpath"],
                    "item_kind": hit["item_kind"],
                    "path": hit["path"],
                    "start_line": hit["start_line"],
                    "end_line": hit["end_line"],
                    "tier": hit["tier"],
                })
            })
            .collect(),
    )
}

/// A form's `seeds.symbols` as the JSON must hold them.
fn matches_of_form(form: &Value) -> Value {
    match form["seeds"]["symbols"].as_array() {
        None => Value::Null,
        Some(matches) => Value::Array(
            matches
                .iter()
                .map(|m| {
                    json!({
                        "qpath": m[0],
                        "item_kind": m[1],
                        "path": m[2],
                        "start_line": m[3],
                        "end_line": m[4],
                        "tier": m[5],
                    })
                })
                .collect(),
        ),
    }
}

/// The names of the seed nodes (`seeds.nodes`), as a set: a seed is an ID
/// string or a node object.
fn seed_names(document: &Value) -> BTreeSet<String> {
    list(&document["seeds"]["nodes"])
        .iter()
        .map(|seed| match seed.as_str() {
            Some(name) => name.to_owned(),
            None => name_of(seed),
        })
        .collect()
}

// ------------------------------------------------------------------- runs

/// `spec impact <args>` text, then `--json`, from `cwd` with `vars`.
fn impact_in(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Read {
    let mut all = vec!["impact"];
    all.extend_from_slice(args);
    read_with(cwd, &all, vars)
}

/// `spec impact <args>` from the root.
fn impact(repo: &LockRepo, args: &[&str]) -> Read {
    impact_in(&repo.root, args, &vars_of(repo))
}

/// [`impact`] over owned arguments.
fn impact_form(repo: &LockRepo, args: &[String]) -> Read {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    impact(repo, &args)
}

/// The rows of a document's `key` (`symbols` or `tests`), a list.
fn rows_at(read: &Read, key: &str) -> Vec<Value> {
    list(&read.json()[key])
}

/// The `note: ` lines of stderr.
fn notes(read: &Read) -> Vec<String> {
    read.text
        .stderr
        .lines()
        .filter(|line| line.starts_with("note: "))
        .map(str::to_owned)
        .collect()
}

/// The `note: tasks not shown: ` and other-repository notes.
fn task_notes(read: &Read) -> Vec<String> {
    notes(read)
        .into_iter()
        .filter(|line| {
            line.starts_with("note: tasks not shown: ") || line.contains("another repository")
        })
        .collect()
}

/// The note naming `n` open tasks of another repository of the fixture's
/// project (docs/features/impact.md "Rows and tasks", `spec roadmap`'s wording with
/// `spec impact`).
fn others_note(n: usize) -> String {
    format!(
        "note: {n} task(s) of another repository of the project `{SLUG}` not shown: \
         `spec impact` shows the current repository's"
    )
}

/// Exit 2 with stdout empty (text and JSON) and `line` on stderr.
#[track_caller]
fn assert_usage(read: &Read, line: &str, context: &str) {
    assert_eq!(read.code(), 2, "{context}\n{}", read.show());
    assert_eq!(read.text.stdout, "", "{context}: stdout empty");
    assert_eq!(
        read.json_run.stdout, "",
        "{context}: stdout empty with --json"
    );
    assert!(
        read.text.stderr.lines().any(|printed| printed == line),
        "{context}: want {line:?} on stderr\n{}",
        read.show()
    );
}

/// An exit-1 answer (docs/features/impact.md "Exits and degrade", "JSON"):
/// stdout empty, `spec: <reason>` the last stderr line; the document
/// keeps `input`, `depth`, `reason` and `notes`, `left_out` zeros,
/// `nodes`, `tasks` and `seeds.nodes` `[]`, `seeds.symbols` `[]` under
/// `--symbol` (else `null`), every other key `null`.
#[track_caller]
fn assert_names_nothing(read: &Read, reason: Option<&str>, symbol: bool, context: &str) {
    assert_eq!(read.code(), 1, "{context}\n{}", read.show());
    assert_eq!(read.text.stdout, "", "{context}: stdout empty");
    let document = read.json();
    let said = text(&document["reason"]).to_owned();
    assert!(!said.is_empty(), "{context}: a reason");
    if let Some(reason) = reason {
        assert_eq!(said, reason, "{context}: the reason");
    }
    assert_eq!(
        read.text.stderr.lines().last(),
        Some(format!("spec: {said}").as_str()),
        "{context}: the reason after the notes\n{}",
        read.show()
    );
    assert!(document["notes"].is_array(), "{context}: notes");
    assert!(document["input"].is_object(), "{context}: input");
    assert_eq!(
        document["left_out"],
        json!({"generated": 0, "tier3": 0}),
        "{context}"
    );
    assert_eq!(document["nodes"], json!([]), "{context}: nodes");
    assert_eq!(document["tasks"], json!([]), "{context}: tasks");
    assert_eq!(
        document["seeds"]["nodes"],
        json!([]),
        "{context}: seeds.nodes"
    );
    assert_eq!(
        document["seeds"]["symbols"],
        if symbol { json!([]) } else { Value::Null },
        "{context}: seeds.symbols"
    );
    for key in ["revision", "delta", "symbols", "tests", "truncated", "code"] {
        assert_eq!(document[key], Value::Null, "{context}: {key} null");
    }
}

// ------------------------------------------------------------ the fixture

/// `fixtures/impact` at `<scratch>/<dir>`, committed, `expected.json`'s
/// `lock` accepted (one lock commit each: a partial lock).
fn ferry_at(label: &str, dir: &str) -> LockRepo {
    let repo = LockRepo::of(label, FIXTURE, dir, &[]);
    for reference in list(&expected()["lock"]) {
        repo.accept_ok(text(&reference));
    }
    repo
}

fn ferry(label: &str) -> LockRepo {
    ferry_at(label, "repo")
}

/// Code moved past the lock: the fare doubled, the window's `opens` body
/// edited (both accepted nodes `code_ahead`), the slack's margin changed.
fn ferry_drift(repo: &LockRepo) {
    repo.replace(
        "crates/ferry/src/fares.rs",
        "    crossings\n}",
        "    crossings + 0\n}",
    );
    repo.replace(
        "crates/ferry/src/crossing.rs",
        "        self.open\n",
        "        self.open + 0\n",
    );
    repo.replace("data/fares.ron", "base: 1,", "base: 2,");
    repo.replace("crates/ferry/src/crossing.rs", "        5\n", "        6\n");
}

/// Where a task command runs: the data directory's `HOME`, the git
/// sandbox, the worktree.
struct Place<'a> {
    home: &'a Path,
    git: &'a Sandbox,
    cwd: &'a Path,
}

impl Place<'_> {
    fn env(&self) -> Env {
        Env {
            cwd: self.cwd.to_path_buf(),
            home: Some(self.home.as_os_str().to_owned()),
            xdg_data_home: None,
        }
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(self.cwd, self.git.vars())
    }

    fn db(&self) -> PathBuf {
        data_dir(self.home).join(format!("{SLUG}.db"))
    }
}

/// A task command that must answer.
#[track_caller]
fn moved(outcome: Result<TaskOutcome, CliError>, context: &str) -> TaskOutcome {
    let outcome = outcome.unwrap_or_else(|error| panic!("{context}: {error}"));
    assert_eq!(outcome.exit().code(), 0, "{context}: {outcome:?}");
    outcome
}

/// `sqlite3 <db> <statement>` (the system's client); it must succeed.
fn sql(db: &Path, statement: &str) -> String {
    let output = Command::new(common::proposal::sqlite3())
        .arg(db)
        .arg(statement)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statement}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// One task of `expected.json` made at `place` and moved to its status:
/// its ID.
fn made(place: &Place<'_>, task: &Value) -> String {
    let globals = Globals::default();
    let strings = |key: &str| -> Vec<String> {
        list(&task[key])
            .iter()
            .map(|value| text(value).to_owned())
            .collect()
    };
    let id = moved(
        task_new(
            &place.env(),
            &globals,
            &TaskNewRequest {
                nodes: strings("targets"),
                title: Some(text(&task["title"]).to_owned()),
                goal: Some("Keep the ferry on time.".to_owned()),
                author_role: None,
                author_model: None,
                run: None,
                now: TASK_CLOCK.to_owned(),
                git: place.git_env(),
            },
        ),
        "task new",
    )
    .id
    .expect("an ID");
    let plan = || {
        moved(
            task_plan(
                &place.env(),
                &globals,
                &TaskPlanRequest {
                    id: id.clone(),
                    plan: ProposedText::Given(b"1. Change the code.\n".to_vec()),
                    criteria: vec!["Holds.".to_owned()],
                    affected: strings("affected"),
                    now: TASK_CLOCK.to_owned(),
                    git: place.git_env(),
                },
            ),
            "task plan",
        );
    };
    let owner = |action: &str| {
        let mut yes = |_: &str| true;
        let request = TaskDecisionRequest {
            id: id.clone(),
            note: (action == "changes").then(|| "Narrow it.".to_owned()),
            now: TASK_CLOCK.to_owned(),
            git: place.git_env(),
        };
        let env = place.env();
        let outcome = match action {
            "approve" => task_approve(&env, &globals, &request, &mut yes),
            "changes" => task_changes(&env, &globals, &request, &mut yes),
            _ => task_cancel(&env, &globals, &request, &mut yes),
        };
        let outcome = moved(outcome, action);
        assert_eq!(outcome.refusal, None, "task {action} {id}: {outcome:?}");
    };
    let claim = || {
        moved(
            task_claim(
                &place.env(),
                &globals,
                &TaskClaimRequest {
                    id: id.clone(),
                    role: "developer".to_owned(),
                    worktree: place.cwd.to_path_buf(),
                    now: TASK_CLOCK.to_owned(),
                    git: place.git_env(),
                },
            ),
            "task claim",
        );
    };
    let finish = || {
        moved(
            task_report(
                &place.env(),
                &globals,
                &TaskReportRequest {
                    id: id.clone(),
                    outcome: "completed".to_owned(),
                    summary: "Done as planned.".to_owned(),
                    changed_files: vec!["crates/ferry/src/fares.rs".to_owned()],
                    now: TASK_CLOCK.to_owned(),
                    git: place.git_env(),
                },
            ),
            "task report",
        );
        moved(
            task_complete(
                &place.env(),
                &globals,
                &TaskCompleteRequest {
                    id: id.clone(),
                    now: TASK_CLOCK.to_owned(),
                    git: place.git_env(),
                },
            ),
            "task complete",
        );
    };
    let set = |status: &str| {
        sql(
            &place.db(),
            &format!("UPDATE tasks SET status = '{status}' WHERE id = '{id}';"),
        );
    };
    let status = text(&task["status"]);
    assert!(
        strings("affected").is_empty() || status == "review",
        "premise: affected nodes come with a plan"
    );
    match status {
        "draft" => {}
        "review" => plan(),
        "changes_requested" => {
            plan();
            owner("changes");
        }
        "ready" => owner("approve"),
        "in_progress" => {
            owner("approve");
            claim();
        }
        // Known, never entered by this build (`tasks.md` "Transitions").
        "analysis" => set("analysis"),
        "in_review" => {
            owner("approve");
            claim();
            set("in_review");
        }
        "done" => {
            owner("approve");
            claim();
            finish();
        }
        "accepted" => {
            owner("approve");
            claim();
            finish();
            set("accepted");
        }
        "cancelled" => owner("cancel"),
        other => panic!("no way to {other}"),
    }
    assert_eq!(
        sql(
            &place.db(),
            &format!("SELECT status FROM tasks WHERE id = '{id}';")
        )
        .trim(),
        status,
        "premise: {id} stored {status}"
    );
    id
}

/// Another git repository of the fixture's project at `<scratch>/<dir>`
/// (the same slug and `HOME`: one queue).
fn other_repository(repo: &LockRepo, dir: &str) -> PathBuf {
    let other = repo.scratch.copy(FIXTURE, dir);
    fs::remove_file(other.join("expected.json")).expect("no oracle in the copy");
    repo.git.init(&other);
    repo.git.add_all(&other);
    repo.git.commit(&other, "another repository");
    other
}

/// `expected.json`'s tasks in order, this repository's at the root and
/// the other's (when `with_other`) in [`other_repository`]: the other
/// repository's root.
fn ferry_tasks(repo: &LockRepo, with_other: bool) -> PathBuf {
    let other = other_repository(repo, "other");
    for task in list(&expected()["tasks"]) {
        let theirs = task["repository"] == "other";
        if theirs && !with_other {
            continue;
        }
        let cwd = if theirs { &other } else { &repo.root };
        let place = Place {
            home: &repo.home,
            git: &repo.git,
            cwd,
        };
        let id = made(&place, &task);
        if with_other {
            assert_eq!(id, text(&task["id"]), "premise: tasks in the table's order");
        }
    }
    other
}

// ---------------------------------------------------------- text shapes

/// A node's line (docs/features/impact.md "Text", (3)): `node ` + graph's
/// node line, `<distance> <name> | <kind or -> | <title or -> |
/// <path>:<line>[ | archived]` (`spec-cli-graph.md` "spec graph").
fn node_line(node: &Value) -> String {
    let mut line = format!(
        "node {} {} | {} | {} | {}:{}",
        node["distance"],
        name_of(node),
        node["kind"].as_str().unwrap_or("-"),
        node["title"].as_str().unwrap_or("-"),
        text(&node["path"]),
        node["line"]
    );
    if node["archived"] == json!(true) {
        line.push_str(" | archived");
    }
    line
}

/// A binding row's lines under `word` (`symbol` or `test`; "Text", (4)):
/// the bundle's `cap.rs` `row_line`, then `row_lines`' `  sig <signature>`,
/// `  moved from <file> | <binding>` and `  candidate <file> | <binding>`
/// lines, two spaces in.
fn row_text(word: &str, row: &Value, binding: &str) -> Vec<String> {
    let mut lines = vec![format!("{word} {}", row_line(row, binding))];
    if let Some(signature) = row["signature"].as_str() {
        lines.push(format!("  sig {signature}"));
    }
    let anchor = &row["anchor"];
    if !anchor["from"].is_null() {
        lines.push(format!(
            "  moved from {} | {}",
            text(&anchor["from"]["file"]),
            binding_of(&anchor["from"])
        ));
    }
    for candidate in anchor["candidates"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  candidate {} | {}",
            text(&candidate["file"]),
            binding_of(candidate)
        ));
    }
    lines
}

/// A row's `<binding>` field as verify prints it: the key, a keyless row
/// its state and category.
fn binding_text(row: &Value) -> String {
    if row["kind"].is_string() {
        binding_of(row)
    } else {
        format!("{} {}", text(&row["sync"]), text(&row["category"]))
    }
}

/// A task's line ("Text", (5)): `task <id> | <status> | <title or -> |
/// <names, comma-joined>`.
fn task_line(task: &Value) -> String {
    let nodes: Vec<String> = list(&task["nodes"])
        .iter()
        .map(|node| text(node).to_owned())
        .collect();
    format!(
        "task {} | {} | {} | {}",
        text(&task["id"]),
        text(&task["status"]),
        task["title"].as_str().unwrap_or("-"),
        nodes.join(", ")
    )
}

/// The summary ("Text", (2)): `impact: nodes <N>, symbols <S>, tests <T>,
/// tasks <K>`, totals (`-` for a `null` list), `; code files <f> (.rs <r>,
/// .ron <o>), skipped <s>` when `code` is set, `; left out: <g>
/// generated, <t> archived` with zero parts (and an all-zero suffix) left
/// out. `shown` the printed counts plus `truncated`'s.
fn summary_line(document: &Value) -> String {
    let count = |key: &str| match document[key].as_array() {
        None => "-".to_owned(),
        Some(items) => {
            let hidden = document["truncated"][key].as_u64().unwrap_or(0) as usize;
            (items.len() + hidden).to_string()
        }
    };
    let mut line = format!(
        "impact: nodes {}, symbols {}, tests {}, tasks {}",
        count("nodes"),
        count("symbols"),
        count("tests"),
        count("tasks")
    );
    let code = &document["code"];
    if code.is_object() {
        line.push_str(&format!(
            "; code files {} (.rs {}, .ron {}), skipped {}",
            code["files"], code["rs"], code["ron"], code["skipped"]
        ));
    }
    let mut parts = Vec::new();
    for (key, word) in [("generated", "generated"), ("tier3", "archived")] {
        let n = document["left_out"][key].as_u64().unwrap_or(0);
        if n > 0 {
            parts.push(format!("{n} {word}"));
        }
    }
    if !parts.is_empty() {
        line.push_str(&format!("; left out: {}", parts.join(", ")));
    }
    line
}

/// `--symbol`'s header ("Text", (1)): `seed symbol <QPATH>: <n> at <tier>`.
fn seed_header(query: &str, matches: usize, tier: &str) -> String {
    format!("seed symbol {query}: {matches} at {tier}")
}

/// A match's line ("Text", (6)): `seed <qpath> | <item_kind> |
/// <path>:<start>-<end>`.
fn seed_line(m: &Value) -> String {
    format!(
        "seed {} | {} | {}:{}-{}",
        text(&m["qpath"]),
        text(&m["item_kind"]),
        text(&m["path"]),
        m["start_line"],
        m["end_line"]
    )
}

/// The whole text an uncut answer prints, from its JSON, in "Text"'s
/// order: the seed header (`--symbol`), the summary, the nodes, the rows
/// (`symbols`, then `tests`), the tasks, the matches.
fn text_of(document: &Value) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(query) = document["input"]["symbol"].as_str() {
        let matches = list(&document["seeds"]["symbols"]);
        let tier = matches.first().map_or("-", |first| text(&first["tier"]));
        lines.push(seed_header(query, matches.len(), tier));
    }
    lines.push(summary_line(document));
    for node in list(&document["nodes"]) {
        lines.push(node_line(&node));
    }
    for (key, word) in [("symbols", "symbol"), ("tests", "test")] {
        for row in document[key].as_array().into_iter().flatten() {
            lines.extend(row_text(word, row, &binding_text(row)));
        }
    }
    for task in list(&document["tasks"]) {
        lines.push(task_line(&task));
    }
    for m in document["seeds"]["symbols"]
        .as_array()
        .into_iter()
        .flatten()
    {
        lines.push(seed_line(m));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// What a text answer prints, by kind of line.
#[derive(Debug, Default)]
struct Printed {
    seed_header: Option<String>,
    matches: Vec<String>,
    nodes: Vec<String>,
    symbols: usize,
    tests: usize,
    tasks: Vec<String>,
    summary: Option<String>,
    tail: Option<String>,
}

/// The lines of `stdout` sorted into [`Printed`]'s kinds by their first
/// word (a row line `symbol ` or `test `; the header `seed symbol `, a
/// match `seed `).
fn printed(stdout: &str) -> Printed {
    let mut out = Printed::default();
    for line in stdout.lines() {
        if line.starts_with("seed symbol ") {
            out.seed_header = Some(line.to_owned());
        } else if line.starts_with("seed ") {
            out.matches.push(line.to_owned());
        } else if line.starts_with("symbol ") {
            out.symbols += 1;
        } else if line.starts_with("test ") {
            out.tests += 1;
        } else if line.starts_with("node ") {
            out.nodes.push(line.to_owned());
        } else if line.starts_with("task ") {
            out.tasks.push(line.to_owned());
        } else if line.starts_with("impact: ") {
            out.summary = Some(line.to_owned());
        } else if line.starts_with("[truncated: ") {
            out.tail = Some(line.to_owned());
        }
    }
    out
}

// ------------------------------------------------------------------ TE-1

/// TE-1, the fixture's premises against the shipped reads (no `spec
/// impact` call): `spec check` is clean; expected.json's `rows` are plain
/// `spec verify --json`'s keys, every one; each REF form's nodes and
/// `left_out` are `spec graph --impact`'s; each `--symbol` form's matches
/// are `spec symbols`' best tier (none for the exit-1 query); each REF
/// form's rows are the union of `spec show --bindings`' over its nodes;
/// the tasks reach every `TaskStatus`, as expected.json's table says.
#[test]
fn fixture_matches_the_shipped_reads() {
    let repo = ferry("im-fixture");
    let vars = vars_of(&repo);
    let check = run_with(&repo.root, &["check"], &vars);
    assert_eq!(check.code, 0, "the fixture checks clean\n{}", check.show());
    let plain = run_with(&repo.root, &["verify", "--json"], &vars);
    let plain = common::one_json_document(&plain.stdout).expect("verify's JSON");
    let mut named: Vec<String> = named_rows().values().map(key_of).collect();
    named.sort();
    assert_eq!(
        keys_of(&list(&plain["rows"])),
        named,
        "expected.json's rows are verify's"
    );
    for form in forms() {
        let args = args_of(&form);
        if args[0] == "--symbol" {
            let hits = best_tier(&repo.root, &args[1], &vars);
            if form["exit"] == 1 {
                assert_eq!(hits, json!([]), "{args:?}: no match");
            } else {
                assert_eq!(
                    hits,
                    matches_of_form(&form),
                    "{args:?}: the lookup's best tier"
                );
            }
            continue;
        }
        let depth = args
            .iter()
            .position(|arg| arg == "--depth")
            .map(|at| args[at + 1].as_str());
        let graph = graph_impact(&repo.root, &args[0], depth, &vars);
        if form["exit"] == 1 {
            assert!(graph["reason"].is_string(), "{args:?}: graph names nothing");
            continue;
        }
        assert_eq!(walked(&graph), form["nodes"], "{args:?}: graph's nodes");
        assert_eq!(
            graph["left_out"], form["left_out"],
            "{args:?}: graph's left_out"
        );
        let mut shown: Vec<Value> = Vec::new();
        for node in list(&graph["nodes"]) {
            let reference = node_ref(&repo.root, &node);
            let show = read_with(&repo.root, &["show", &reference, "--bindings"], &vars);
            show.answered();
            for shown_node in list(&show.json()["nodes"]) {
                for row in list(&shown_node["bindings"]["rows"]) {
                    if !shown.contains(&row) {
                        shown.push(row);
                    }
                }
            }
        }
        let mut both: Vec<String> = keys_named(&form["symbols"]);
        both.extend(keys_named(&form["tests"]));
        both.sort();
        assert_eq!(
            keys_of(&shown),
            both,
            "{args:?}: show --bindings' rows over the nodes"
        );
    }
    ferry_tasks(&repo, true);
    let db = data_dir(&repo.home).join(format!("{SLUG}.db"));
    let stored: BTreeSet<String> = sql(&db, "SELECT status FROM tasks;")
        .lines()
        .map(str::to_owned)
        .collect();
    let every: BTreeSet<String> = specengine_cli::TaskStatus::ALL
        .iter()
        .map(|status| status.as_str().to_owned())
        .collect();
    assert_eq!(stored, every, "a task in every state");
}

// ------------------------------------------------------------------ RD-1

/// Every REF form of the fixture by every REF form of each node (ID,
/// `ID#SECTION`, path, `<feature>/<ID>`), at every depth, and every ID of
/// two projects of another nature (`fixtures/spec-a`, a game's design, no
/// `[code]`; `fixtures/code-a`, a counting library): `nodes` and
/// `left_out` are `spec graph REF --impact --json`'s, exit 1 its `reason`;
/// the fixture's forms are `expected.json`'s.
fn assert_walks_equal_graph(repo: &LockRepo, references: &[&str], context: &str) {
    let vars = vars_of(repo);
    for reference in references {
        for depth in [None, Some("0"), Some("1"), Some("2")] {
            let mut args = vec![*reference];
            if let Some(depth) = depth {
                args.extend(["--depth", depth]);
            }
            let read = impact(repo, &args);
            let graph = graph_impact(&repo.root, reference, depth, &vars);
            let what = format!("{context}: impact {args:?}");
            if graph["reason"].is_null() {
                read.answered();
                let document = read.json();
                assert_eq!(
                    document["nodes"],
                    graph["nodes"],
                    "{what}: nodes are graph --impact's\n{}",
                    read.show()
                );
                assert_eq!(document["left_out"], graph["left_out"], "{what}: left_out");
                assert_eq!(
                    document["depth"],
                    depth.map_or(Value::Null, |d| json!(d.parse::<u64>().unwrap())),
                    "{what}: depth"
                );
                assert_eq!(
                    document["input"],
                    json!({"ref": reference, "symbol": null, "since": null}),
                    "{what}: input"
                );
            } else {
                assert_names_nothing(&read, graph["reason"].as_str(), false, &what);
            }
        }
    }
}

/// RD-1. M: an outgoing `depends_on` followed (ROUTE-CROSSING reaches
/// ROUTE-DOCKS).
#[test]
fn impact_nodes_equal_graph_impact() {
    let repo = ferry("im-walk");
    let vars = vars_of(&repo);
    for form in forms() {
        let args = args_of(&form);
        if args[0] == "--symbol" {
            continue;
        }
        let read = impact_form(&repo, &args);
        let what = format!("impact {args:?} ({})", text(&form["name"]));
        assert_eq!(
            Some(i64::from(read.code())),
            form["exit"].as_i64(),
            "{what}\n{}",
            read.show()
        );
        if read.code() == 1 {
            assert_names_nothing(&read, None, false, &what);
            continue;
        }
        let document = read.json();
        let depth = args
            .iter()
            .position(|arg| arg == "--depth")
            .map(|at| args[at + 1].as_str());
        let graph = graph_impact(&repo.root, &args[0], depth, &vars);
        assert_eq!(document["nodes"], graph["nodes"], "{what}: graph's nodes");
        assert_eq!(
            document["left_out"], graph["left_out"],
            "{what}: graph's left_out"
        );
        assert_eq!(
            walked(&document),
            form["nodes"],
            "{what}: expected.json's nodes"
        );
        assert_eq!(
            document["left_out"], form["left_out"],
            "{what}: expected.json's left_out"
        );
        let seeds: BTreeSet<String> = list(&form["seeds"]["nodes"])
            .iter()
            .map(|name| text(name).to_owned())
            .collect();
        assert_eq!(
            seed_names(&document),
            seeds,
            "{what}: seeds.nodes are the holders"
        );
        assert_eq!(
            document["seeds"]["symbols"],
            Value::Null,
            "{what}: no match for a REF"
        );
    }
    let every = [
        "ROUTE-CROSSING",
        "docs/spec/crossing.md",
        "TIDE-WINDOW",
        "ROUTE-CROSSING#TIDE-WINDOW",
        "TIDE-SLACK",
        "ROUTE-DOCKS",
        "ROUTE-TIMETABLE",
        "TIDE-BOARD",
        "FARE-01",
        "FARE-02",
        "NOTE-001",
        "NOTE-002",
        "docs/features/ferry-check.md",
        "ferry-check/CHK-01",
        "ferry-check/CHK-02",
        "docs/features/old-crossing.md",
        "docs/spec/departures.md",
        "ROUTE-NOWHERE",
    ];
    assert_walks_equal_graph(&repo, &every, "ferry");

    // Several holders (docs/features/impact.md "Command", **REF**): all at
    // distance 0, graph's one `warning:` line, its nodes.
    let docks = repo.read("docs/spec/docks.md");
    repo.write("docs/spec/docks-twin.md", &docks);
    let read = impact(&repo, &["ROUTE-DOCKS"]);
    read.answered();
    let graph = run_with(&repo.root, &["graph", "ROUTE-DOCKS", "--impact"], &vars);
    let warnings = |stderr: &str| -> Vec<String> {
        stderr
            .lines()
            .filter(|line| line.starts_with("warning: "))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        warnings(&read.text.stderr),
        warnings(&graph.stderr),
        "graph's holders warning\n{}",
        read.show()
    );
    assert_eq!(warnings(&read.text.stderr).len(), 1, "one warning");
    assert_eq!(
        read.json()["nodes"],
        graph_impact(&repo.root, "ROUTE-DOCKS", None, &vars)["nodes"],
        "two holders: graph's nodes"
    );
    assert_eq!(
        list(&read.json()["nodes"])
            .iter()
            .filter(|node| node["distance"] == 0)
            .count(),
        2,
        "both holders at distance 0"
    );
    fs::remove_file(repo.root.join("docs/spec/docks-twin.md")).expect("the twin");

    let game = LockRepo::of("im-walk-a", "spec-a", "repo", &[]);
    assert_walks_equal_graph(
        &game,
        &[
            "MEC-STAMINA",
            "MEC-SPRINT",
            "RULE-STAM-REGEN",
            "EDGE-STAM-ZERO",
            "RULE-SPRINT-COST",
            "R-12",
            "A-101",
            "TERM-exhausted",
            "Q-031",
            "DEC-0023",
            "DOM-MOVEMENT",
            "DOM-GAME",
            "docs/spec/movement/stamina.md",
            "MEC-NOPE",
        ],
        "spec-a",
    );
    let library = golden("im-walk-code-a");
    verifying_record(&library);
    criterion(&library, 0);
    assert_walks_equal_graph(
        &library,
        &[
            "GOAL-01",
            "PIECE-COUNTER",
            "LAW-BOUND",
            "PIECE-LIMITS",
            "PREM-002",
            "cap-check/PASS-01",
        ],
        "code-a",
    );
}

// ------------------------------------------------------------------ RD-2

/// RD-2 (the CLI half): no input and two inputs exit 2 with the usage
/// line, stdout empty, text and `--json`, before discovery (outside any
/// project too); `--depth` not an integer at least 0 and a look-alike ID
/// (naming its Latin fix) exit 2 the same way. M: two inputs accepted.
#[test]
fn impact_takes_exactly_one_input() {
    let repo = ferry("im-one");
    for args in [
        &[][..],
        &["ROUTE-CROSSING", "--symbol", "opens"][..],
        &["--symbol", "opens", "TIDE-WINDOW", "--depth", "1"][..],
    ] {
        let read = impact(&repo, args);
        assert_usage(&read, ONE_INPUT, &format!("{args:?}"));
    }
    // Before discovery: a directory holding no project.
    let nowhere = repo.scratch.dir("nowhere");
    let read = impact_in(
        &nowhere,
        &["ROUTE-CROSSING", "--symbol", "opens"],
        &vars_of(&repo),
    );
    assert_usage(&read, ONE_INPUT, "outside a project");
    let read = impact_in(&nowhere, &[], &vars_of(&repo));
    assert_usage(&read, ONE_INPUT, "outside a project, no input");

    for depth in ["-1", "x", "1.5"] {
        let read = impact(&repo, &["ROUTE-CROSSING", "--depth", depth]);
        assert_eq!(read.code(), 2, "--depth {depth}\n{}", read.show());
        assert_eq!(read.text.stdout, "", "--depth {depth}: stdout empty");
        assert_eq!(
            read.json_run.stdout, "",
            "--depth {depth}: stdout empty (JSON)"
        );
    }
    let read = impact(&repo, &[CROSSING_LOOK_ALIKE]);
    assert_eq!(read.code(), 2, "a look-alike\n{}", read.show());
    assert_eq!(read.text.stdout, "", "a look-alike: stdout empty");
    assert!(
        read.text.stderr.contains("ROUTE-CROSSING"),
        "a look-alike names its Latin fix\n{}",
        read.show()
    );
}

// ------------------------------------------------------------- RD-3, TE-2

/// One state's differential over `args`' answers: the reached set is each
/// JSON node's set (its ID and every section within it, as `spec show
/// --json` lists them, checked against the parser); `symbols` hold that
/// set's non-`verifies` verify rows, `tests` its `verifies` rows, each in
/// plain verify's order and judged as `verify --node` judges it; together
/// they are the union of `spec show <node> --bindings --json`'s rows over
/// the reached nodes, whole (located fields included); every item row is
/// located as the code index stores it; a sound state notes nothing.
/// Every row kept, for the corpora's coverage.
fn impact_differential(repo: &LockRepo, slug: &str, forms: &[&[&str]]) -> Vec<Value> {
    let vars = vars_of(repo);
    let mut oracle = Oracle::new(repo);
    let mut kept = Vec::new();
    for args in forms {
        let read = impact(repo, args);
        read.answered();
        assert_eq!(
            read.binding_notes(),
            Vec::<String>::new(),
            "{}: a sound state, no bindings note\n{}",
            read.what,
            read.show()
        );
        let document = read.json();
        let mut set: Vec<String> = Vec::new();
        let mut shown: Vec<Value> = Vec::new();
        for node in list(&document["nodes"]) {
            let reference = node_ref(&repo.root, &node);
            for member in holder_set(&repo.root, &[&reference], &vars) {
                if !set.contains(&member) {
                    set.push(member);
                }
            }
            let show = read_with(&repo.root, &["show", &reference, "--bindings"], &vars);
            show.answered();
            for shown_node in list(&show.json()["nodes"]) {
                for row in list(&shown_node["bindings"]["rows"]) {
                    if !shown.contains(&row) {
                        shown.push(row);
                    }
                }
            }
        }
        let rows = oracle.set_rows(repo, &set);
        let stored = stored_symbols(&repo.home, slug);
        let mut both: Vec<Value> = Vec::new();
        for (key, verifies) in [("symbols", false), ("tests", true)] {
            let want: Vec<&Expected> = rows
                .iter()
                .filter(|expected| (expected.row["relation"] == "verifies") == verifies)
                .collect();
            let got = rows_at(&read, key);
            let judged: Vec<Value> = want.iter().map(|e| judged_view(&e.row)).collect();
            let views: Vec<Value> = got.iter().map(shown_view).collect();
            assert_eq!(
                views,
                judged,
                "{}: {key} over {set:?}: verify --node's rows in its order\n{}",
                read.what,
                rows_diff(&judged, &views)
            );
            for (row, expected) in got.iter().zip(&want) {
                let keys: Vec<&str> = row
                    .as_object()
                    .expect("a row object")
                    .keys()
                    .map(String::as_str)
                    .collect();
                let mut sorted_keys = ROW_KEYS.to_vec();
                sorted_keys.sort_unstable();
                let mut have = keys.clone();
                have.sort_unstable();
                assert_eq!(have, sorted_keys, "{}: the 14 row keys: {row}", read.what);
                assert_eq!(
                    shown_located(row),
                    located_view(row, &expected.row["markers"], &stored),
                    "{}: {row}: located as the code index stores it",
                    read.what
                );
            }
            both.extend(got);
        }
        assert_eq!(
            sorted(&both),
            sorted(&shown),
            "{}: the rows are show --bindings' over the reached nodes\n{}",
            read.what,
            rows_diff(&shown, &both)
        );
        kept.extend(both);
    }
    kept
}

/// RD-3 and TE-1 over the fixture (a partial lock, code moved past it):
/// every form's rows are its reached set's ([`impact_differential`]) and
/// `expected.json`'s named rows, `symbols` and `tests` split on
/// `verifies`; the walk's nodes without a row (NOTE-001, ROUTE-DOCKS) add
/// none, and TIDE-NOPE's row (an ID naming nothing) is in no radius. M:
/// the sections' rows dropped (ROUTE-CROSSING without TIDE-WINDOW's); a
/// `verifies` row under `symbols`.
#[test]
fn impact_rows_are_the_reached_sets_rows() {
    let repo = ferry("im-rows");
    ferry_drift(&repo);
    let code_forms: Vec<Vec<String>> = forms()
        .iter()
        .filter(|form| form["exit"] == 0)
        .map(args_of)
        .collect();
    let refs: Vec<Vec<&str>> = code_forms
        .iter()
        .map(|args| args.iter().map(String::as_str).collect())
        .collect();
    let slices: Vec<&[&str]> = refs.iter().map(Vec::as_slice).collect();
    let kept = impact_differential(&repo, SLUG, &slices);
    let states: BTreeSet<String> = values_of(&kept, |row| &row["sync"]);
    for state in ["ok", "code_ahead", "unlocked"] {
        assert!(states.contains(state), "premise: a {state} row: {states:?}");
    }
    for form in forms().iter().filter(|form| form["exit"] == 0) {
        let read = impact_form(&repo, &args_of(form));
        let what = format!("{} ({})", read.what, text(&form["name"]));
        assert_eq!(
            keys_of(&rows_at(&read, "symbols")),
            keys_named(&form["symbols"]),
            "{what}: expected.json's symbols"
        );
        assert_eq!(
            keys_of(&rows_at(&read, "tests")),
            keys_named(&form["tests"]),
            "{what}: expected.json's tests"
        );
        for row in rows_at(&read, "symbols") {
            assert_ne!(row["relation"], "verifies", "{what}: a test under symbols");
        }
        for row in rows_at(&read, "tests") {
            assert_eq!(
                row["relation"], "verifies",
                "{what}: a non-test under tests"
            );
        }
        let nope = key_of(&named_rows()["nope"]);
        for key in ["symbols", "tests"] {
            assert!(
                !rows_at(&read, key).iter().any(|row| key_of(row) == nope),
                "{what}: TIDE-NOPE names no node: in no radius"
            );
        }
    }
}

/// code-a's forms: every node by one or more REF forms, a verifying record
/// and a criterion linked in.
const GOLDEN_FORMS: [&[&str]; 14] = [
    &["PIECE-COUNTER"],
    &["docs/spec/counter.md"],
    &["LAW-BOUND"],
    &["PIECE-COUNTER#LAW-BOUND"],
    &["LAW-STEP"],
    &["LAW-ODD"],
    &["GOAL-01"],
    &["GOAL-01", "--depth", "0"],
    &["AIM-01"],
    &["PREM-001"],
    &["PREM-002"],
    &["cap-check/PASS-01"],
    &["PIECE-LIMITS"],
    &["LAW-DEFAULT"],
];

/// TE-2 (a): the golden with `drift`, `halves`, `nested`, a criterion and
/// a verifying record of GOAL-01.
fn corpus_golden() -> Vec<Value> {
    let repo = golden("im-te2-golden");
    drift(&repo);
    halves(&repo);
    nested(&repo);
    criterion(&repo, 0);
    verifying_record(&repo);
    let kept = impact_differential(&repo, CODE_A, &GOLDEN_FORMS);
    assert!(
        kept.iter()
            .any(|row| row["node"] == "PREM-002" && row["relation"] == "verifies"),
        "premise: GOAL-01's radius reaches the verifying record's test"
    );
    kept
}

/// TE-2 (b): `fixtures/anchor-states`, its baseline, then its 18
/// scenarios together.
fn corpus_anchor_states() -> Vec<Value> {
    let repo = anchor_common::base("im-te2-anchors");
    let mut refs: Vec<&str> = anchor_common::NODES.to_vec();
    refs.extend(["KITE-RIGGING", "docs/spec/kites.md"]);
    let forms: Vec<&[&str]> = refs.iter().map(std::slice::from_ref).collect();
    let mut kept = impact_differential(&repo, ANCHORS, &forms);
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&repo, name);
    }
    let renamed: Vec<&str> = refs
        .iter()
        .map(|id| if *id == "WISP-01" { "WISP-02" } else { id })
        .collect();
    let forms: Vec<&[&str]> = renamed.iter().map(std::slice::from_ref).collect();
    kept.extend(impact_differential(&repo, ANCHORS, &forms));
    kept
}

/// TE-2 (c): `fixtures/verify-tests` (no lock): the document by ID and
/// path, its sections, an alias, a feature criterion and its document.
fn corpus_kiln() -> Vec<Value> {
    let repo = LockRepo::of("im-te2-kiln", "verify-tests", "repo", &[]);
    impact_differential(
        &repo,
        KILN,
        &[
            &["KILN-FIRING"],
            &["docs/spec/kiln.md"],
            &["KILN-RAMP"],
            &["KILN-COOL"],
            &["CONE-01"],
            &["PYRO-01"],
            &["glaze-check/GLZ-01"],
            &["docs/features/glaze-check.md"],
        ],
    )
}

/// TE-2 (d): `verify_common::ecs` (an ECS game crate, a RON file) with a
/// body and a RON value edited.
fn corpus_ecs() -> Vec<Value> {
    let repo = ecs("im-te2-ecs");
    repo.replace(
        verify_common::STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(
        verify_common::MOVEMENT,
        "regen_per_second: 2.5,",
        "regen_per_second: 3.0,",
    );
    impact_differential(
        &repo,
        GAME,
        &[
            &["PERK-STAMINA"],
            &["docs/spec/stamina.md"],
            &["PERK-REGEN"],
            &["PERK-DRAIN"],
            &["TUNE-01"],
            &["TUNE-02"],
            &["PERK-MOVE"],
            &["docs/spec/movement.md"],
        ],
    )
}

type Corpus = fn() -> Vec<Value>;

/// Each corpus on a thread of its own: every row they keep.
fn every_corpus(corpora: &[(&str, Corpus)]) -> Vec<Value> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = corpora
            .iter()
            .map(|(name, corpus)| (*name, scope.spawn(corpus)))
            .collect();
        let mut kept = Vec::new();
        let mut failed = Vec::new();
        for (name, handle) in handles {
            match handle.join() {
                Ok(rows) => kept.extend(rows),
                Err(_) => failed.push(name),
            }
        }
        assert!(failed.is_empty(), "corpora failed (above): {failed:?}");
        kept
    })
}

/// TE-2: over code-a's golden, the 18 anchor scenarios, the kiln library
/// (no lock) and the ECS game with RON data, each REF's radius rows equal
/// `show --bindings`' over its reached nodes and their `sync` `verify
/// --node`'s ([`impact_differential`]); across them every state the
/// corpora give, a moved anchor, a RON row, a keyless row. M: rows of the
/// reached nodes' sections dropped.
#[test]
fn impact_rows_equal_show_bindings_on_every_corpus() {
    let kept = every_corpus(&[
        ("golden", corpus_golden),
        ("anchor-states", corpus_anchor_states),
        ("verify-tests", corpus_kiln),
        ("verify-ecs", corpus_ecs),
    ]);
    let states = values_of(&kept, |row| &row["sync"]);
    for state in ["ok", "code_ahead", "spec_ahead", "unlocked", "bad_marker"] {
        assert!(states.contains(state), "premise: a {state} row: {states:?}");
    }
    assert!(
        kept.iter().any(|row| row["anchor"]["state"] == "moved"),
        "premise: a moved row"
    );
    assert!(
        kept.iter().any(|row| row["kind"] == "ron"),
        "premise: a RON row"
    );
    assert!(
        kept.iter().any(|row| row["kind"].is_null()),
        "premise: a keyless row"
    );
}

// ------------------------------------------------------------------ RD-4

/// One `--symbol` form against `expected.json` and the oracles: exit 0;
/// `input`; `seeds.symbols` the lookup's best non-empty tier (`spec
/// symbols`' hits) and expected.json's; `seeds.nodes` expected.json's; a
/// single seed's walk `spec graph SEED --impact`'s; nodes, rows and their
/// split expected.json's.
fn assert_symbol_form(repo: &LockRepo, form: &Value) {
    let vars = vars_of(repo);
    let args = args_of(form);
    let query = args[1].as_str();
    let read = impact_form(repo, &args);
    let what = format!("{} ({})", read.what, text(&form["name"]));
    read.answered();
    let document = read.json();
    assert_eq!(
        document["input"],
        json!({"ref": null, "symbol": query, "since": null}),
        "{what}: input"
    );
    assert_eq!(
        document["seeds"]["symbols"],
        best_tier(&repo.root, query, &vars),
        "{what}: the lookup's best non-empty tier, every match"
    );
    assert_eq!(
        document["seeds"]["symbols"],
        matches_of_form(form),
        "{what}: expected.json's matches"
    );
    let seeds: BTreeSet<String> = list(&form["seeds"]["nodes"])
        .iter()
        .map(|name| text(name).to_owned())
        .collect();
    assert_eq!(seed_names(&document), seeds, "{what}: the seed nodes");
    assert_eq!(
        walked(&document),
        form["nodes"],
        "{what}: expected.json's nodes"
    );
    if let [seed] = list(&form["seeds"]["nodes"]).as_slice() {
        let graph = graph_impact(&repo.root, text(seed), None, &vars);
        assert_eq!(
            document["nodes"], graph["nodes"],
            "{what}: the seed's walk is graph --impact's"
        );
    }
    assert_eq!(
        keys_of(&rows_at(&read, "symbols")),
        keys_named(&form["symbols"]),
        "{what}: expected.json's symbols"
    );
    assert_eq!(
        keys_of(&rows_at(&read, "tests")),
        keys_named(&form["tests"]),
        "{what}: expected.json's tests"
    );
}

/// RD-4: each `--symbol` form of expected.json: a method reaches its
/// enclosing impl's node, a function in an inline mod in an inline mod
/// the outer mod's, a file-module marker seeds every item of its file, a
/// test's `@verifies` marker seeds (any relation), two matches at one
/// tier seed two nodes at distance 0, and the name tier's extra match is
/// left out when a better tier answers. Then a marker on the method
/// itself: the item's and its impl's both seed. M: the exact item only.
#[test]
fn symbol_impact_reaches_the_enclosing_binding() {
    let repo = ferry("im-symbol");
    for form in forms() {
        let args = args_of(&form);
        if args[0] != "--symbol" || form["exit"] != 0 {
            continue;
        }
        if form["seeds"]["nodes"].as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        assert_symbol_form(&repo, &form);
    }
    // A marker on `opens` itself: its own node and its impl's.
    repo.replace(
        "crates/ferry/src/crossing.rs",
        "impl Crossing {\n    pub fn opens",
        "impl Crossing {\n    // @implements FARE-01\n    pub fn opens",
    );
    let read = impact(&repo, &["--symbol", "opens"]);
    read.answered();
    let document = read.json();
    assert_eq!(
        seed_names(&document),
        ["FARE-01", "TIDE-WINDOW"]
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        "the item's marker and its enclosing impl's\n{}",
        read.show()
    );
    let at_zero: BTreeSet<String> = list(&document["nodes"])
        .iter()
        .filter(|node| node["distance"] == 0)
        .map(name_of)
        .collect();
    assert_eq!(
        at_zero,
        ["FARE-01", "TIDE-WINDOW"]
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        "both at distance 0"
    );
}

/// RD-4: an item bound only by a mention seeds nothing (exit 0, every
/// list empty), as do a marker whose ID names no node, a look-alike ID,
/// another project's ID and a file mention; expected.json's forms. M: a
/// mention seeds (dock_time reaches ROUTE-DOCKS and its radius).
#[test]
fn mentions_never_seed_impact() {
    let repo = ferry("im-mention");
    ferry_tasks(&repo, true);
    for form in forms() {
        let args = args_of(&form);
        if args[0] != "--symbol" || form["exit"] != 0 {
            continue;
        }
        if !form["seeds"]["nodes"].as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        assert_symbol_form(&repo, &form);
        let read = impact_form(&repo, &args);
        let document = read.json();
        assert_eq!(document["nodes"], json!([]), "{}: no node", read.what);
        assert_eq!(document["tasks"], json!([]), "{}: no task", read.what);
        assert_eq!(
            task_notes(&read),
            Vec::<String>::new(),
            "{}: no task note",
            read.what
        );
    }
    repo.write(
        "crates/ferry/src/notes.rs",
        "//! Notes beside ROUTE-DOCKS, a mention of the file.\n\n\
         // @implements ROUTE-D\u{41E}CKS\n\
         pub fn lookalike() -> u32 {\n    1\n}\n\n\
         // @implements other:ROUTE-DOCKS\n\
         pub fn elsewhere() -> u32 {\n    2\n}\n\n\
         pub fn noted() -> u32 {\n    3\n}\n",
    );
    repo.replace(
        "crates/ferry/src/lib.rs",
        "pub mod fares;\n",
        "pub mod fares;\npub mod notes;\n",
    );
    for query in ["lookalike", "elsewhere", "noted"] {
        let read = impact(&repo, &["--symbol", query]);
        read.answered();
        let document = read.json();
        assert_eq!(
            list(&document["seeds"]["symbols"]).len(),
            1,
            "{query}: one match\n{}",
            read.show()
        );
        for key in ["nodes", "symbols", "tests", "tasks"] {
            assert_eq!(
                document[key],
                json!([]),
                "{query}: nothing seeded, no {key}\n{}",
                read.show()
            );
        }
        assert_eq!(seed_names(&document), BTreeSet::new(), "{query}: no seed");
    }
}

/// RD-4: a query no symbol matches exits 1: stdout empty, `spec: --symbol
/// `<q>` names no symbol` on stderr; the JSON keeps the document, its
/// `reason` that line without `spec: `.
#[test]
fn symbol_naming_no_symbol_exits_1() {
    let repo = ferry("im-no-symbol");
    for form in forms() {
        let args = args_of(&form);
        if args[0] != "--symbol" || form["exit"] != 1 {
            continue;
        }
        let read = impact_form(&repo, &args);
        assert_names_nothing(&read, form["reason"].as_str(), true, &read.what);
        assert_eq!(
            read.json()["input"],
            json!({"ref": null, "symbol": args[1], "since": null}),
            "{}: input as given",
            read.what
        );
    }
}

// ------------------------------------------------------------------ RD-5

/// `verify --node REF`'s first run cause in `cwd`.
fn first_cause(cwd: &Path, reference: &str, vars: &[(OsString, OsString)]) -> String {
    let document = judged_with(cwd, reference, vars).document;
    let causes = list(&document["cannot"]);
    assert!(
        !causes.is_empty(),
        "premise: verify --node {reference} has a run cause here: {document}"
    );
    text(&causes[0]).to_owned()
}

/// A `git` that fails every call, first on `PATH`.
fn failing_git(dir: &Path) -> OsString {
    fs::create_dir_all(dir).expect("the shim's directory");
    let shim = dir.join("git");
    fs::write(
        &shim,
        "#!/bin/sh\necho 'fatal: refused by the test' >&2\nexit 128\n",
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
    dir.as_os_str().to_owned()
}

/// A REF read degraded by a code cause: exit 0, the nodes and tasks of the
/// sound state, one `note: bindings: <cause>` (`cause` `None`: no note and
/// both code lists `null`, the no-`[code]` read); `rows`: `None` both lists
/// `null`, else the keys listed, never judged.
fn assert_ref_degraded(
    sound: &Value,
    read: &Read,
    cause: Option<&str>,
    rows: Option<&[String]>,
    context: &str,
) {
    assert_eq!(
        read.code(),
        0,
        "{context}: the REF form answers\n{}",
        read.show()
    );
    let document = read.json();
    assert_eq!(
        document["nodes"], sound["nodes"],
        "{context}: the nodes unchanged"
    );
    assert_eq!(
        document["left_out"], sound["left_out"],
        "{context}: left_out unchanged"
    );
    match cause {
        None => assert_eq!(
            read.binding_notes(),
            Vec::<String>::new(),
            "{context}: silent"
        ),
        Some(cause) => {
            assert_eq!(
                read.binding_notes(),
                [format!("note: bindings: {cause}")],
                "{context}: one note\n{}",
                read.show()
            );
            assert!(
                list(&document["notes"]).contains(&json!(format!("bindings: {cause}"))),
                "{context}: the JSON's notes"
            );
        }
    }
    match rows {
        None => {
            for key in ["symbols", "tests"] {
                assert_eq!(document[key], Value::Null, "{context}: {key} null");
            }
        }
        Some(keys) => {
            let mut all = list(&document["symbols"]);
            all.extend(list(&document["tests"]));
            assert_eq!(keys_of(&all), keys, "{context}: the keys listed");
            for row in &all {
                for unjudged in ["sync", "category", "detail", "anchor"] {
                    assert_eq!(
                        row[unjudged],
                        Value::Null,
                        "{context}: {unjudged} null, never judged: {row}"
                    );
                }
            }
        }
    }
}

/// A copy of `base` in a merge whose `spec.lock` conflicts: both branches
/// changed the first entry's `verified_at`.
fn unmerged_copy(base: &LockRepo, dir: &str) -> PathBuf {
    let root = clone_dir(base, dir);
    let git = |args: &[&str]| base.git.git(&root, args);
    let lock = fs::read_to_string(root.join("spec.lock")).expect("the lock");
    let stamp = "verified_at = \"2026-10-08\"";
    assert!(
        lock.contains(stamp),
        "premise: an entry verified at the lock clock"
    );
    git(&["checkout", "-q", "-b", "side"]);
    fs::write(
        root.join("spec.lock"),
        lock.replacen(stamp, "verified_at = \"2026-10-09\"", 1),
    )
    .expect("side's lock");
    base.git.add_all(&root);
    base.git.commit(&root, "side lock");
    git(&["checkout", "-q", "main"]);
    fs::write(
        root.join("spec.lock"),
        lock.replacen(stamp, "verified_at = \"2026-10-07\"", 1),
    )
    .expect("main's lock");
    base.git.add_all(&root);
    base.git.commit(&root, "main lock");
    let merge = base
        .git
        .git_output(&root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: spec.lock conflicts");
    root
}

/// The rows of a sound read's two code lists.
fn all_rows(document: &Value) -> Vec<Value> {
    let mut all = list(&document["symbols"]);
    all.extend(list(&document["tests"]));
    all
}

/// RD-5: the REF form never fails on code, a leg per cause over the
/// fixture with its tasks and code moved past its lock: no `[code]`
/// (`symbols`, `tests`, `code` `null`, no bindings note, the queue's as
/// the sound read's); a refused `[code]`
/// key and a walk gap (no rows, `code` `null`, one note); a lock not UTF-8
/// and an unmerged lock (the markers' keys listed `sync -`, one note); no
/// git and a git that fails (the rows listed, one bindings note); each
/// read exit 0 with the sound read's `nodes`, `left_out` and (git
/// running) `tasks`, the note `verify --node`'s first `cannot:` line
/// there. M6: the REF form exits 2 on a walk gap.
#[test]
fn impact_ref_never_fails_on_code() {
    let repo = ferry("im-degrade");
    ferry_tasks(&repo, true);
    ferry_drift(&repo);
    let vars = vars_of(&repo);
    let root = repo.root.clone();
    let refs = ["ROUTE-CROSSING", "TIDE-WINDOW"];
    let sound: BTreeMap<&str, Value> = refs
        .iter()
        .map(|reference| {
            let read = impact(&repo, &[reference]);
            read.answered();
            let document = read.json();
            assert!(
                !list(&document["tasks"]).is_empty() && !all_rows(&document).is_empty(),
                "premise: {reference} has rows and tasks"
            );
            (*reference, document)
        })
        .collect();
    let config = repo.read("specengine.toml");
    let same_tasks = |read: &Read, reference: &str, context: &str| {
        assert_eq!(
            read.json()["tasks"],
            sound[reference]["tasks"],
            "{context}: {reference}'s tasks unchanged"
        );
    };

    // (2) no `[code]`: no bindings note, both lists and `code` null.
    without_code(&root, || {
        for reference in refs {
            let read = impact(&repo, &[reference]);
            assert_ref_degraded(&sound[reference], &read, None, None, "no [code]");
            // "no note": no `bindings:` note; the queue's own (the other
            // repository's task in the radius) as the sound read's.
            assert_eq!(
                read.json()["notes"],
                sound[reference]["notes"],
                "no [code]: the sound read's notes, no bindings note"
            );
            assert_eq!(read.json()["code"], Value::Null, "no [code]: code null");
            same_tasks(&read, reference, "no [code]");
        }
    });

    // (1) a refused `[code]` key; (3) a walk gap: no rows, one note.
    let roots = "roots = [\"crates\", \"data\"]";
    repo.replace(
        "specengine.toml",
        roots,
        &format!("{roots}\nlanguages = [\"rust\"]"),
    );
    for reference in refs {
        let cause = first_cause(&root, reference, &vars);
        let read = impact(&repo, &[reference]);
        assert_ref_degraded(
            &sound[reference],
            &read,
            Some(&cause),
            None,
            "a refused key",
        );
        assert_eq!(read.json()["code"], Value::Null, "a refused key: code null");
        same_tasks(&read, reference, "a refused key");
    }
    repo.write("specengine.toml", &config);
    let spare = "crates/ferry/src/spare.rs";
    repo.write(spare, "pub fn spare_part() {}\n");
    {
        let _mode = Mode::set(repo.root.join(spare), 0o000);
        for reference in refs {
            let cause = first_cause(&root, reference, &vars);
            let read = impact(&repo, &[reference]);
            assert_ref_degraded(&sound[reference], &read, Some(&cause), None, "a walk gap");
            assert_eq!(read.json()["code"], Value::Null, "a walk gap: code null");
            same_tasks(&read, reference, "a walk gap");
        }
    }
    fs::remove_file(repo.root.join(spare)).expect("the spare file");

    // (6) a lock not UTF-8: the markers' keys listed, never judged.
    let lock = repo.lock().expect("the lock");
    let bare = clone_dir(&repo, "bare");
    fs::remove_file(bare.join("spec.lock")).expect("no lock");
    let markers_only = |cwd: &Path, args: &[&str]| -> Vec<String> {
        let read = impact_in(cwd, args, &vars);
        read.answered();
        keys_of(&all_rows(&read.json()))
    };
    repo.write("spec.lock", b"\xff\xfe".as_slice());
    for reference in refs {
        let cause = first_cause(&root, reference, &vars);
        let keys = markers_only(&bare, &[reference]);
        let read = impact(&repo, &[reference]);
        assert_ref_degraded(
            &sound[reference],
            &read,
            Some(&cause),
            Some(&keys),
            "a bad lock",
        );
        same_tasks(&read, reference, "a bad lock");
    }
    // `--symbol` takes code as its input; a lock cause still only marks
    // its rows (ADR-0046).
    let read = impact(&repo, &["--symbol", "opens"]);
    let cause = first_cause(&root, "TIDE-WINDOW", &vars);
    let keys = markers_only(&bare, &["--symbol", "opens"]);
    let symbol_sound = impact_in(&bare, &["--symbol", "opens"], &vars).json();
    assert_ref_degraded(
        &symbol_sound,
        &read,
        Some(&cause),
        Some(&keys),
        "--symbol, a bad lock",
    );
    repo.write("spec.lock", lock);

    // (5) an unmerged `spec.lock`: the markers' keys listed, one note.
    let merged = unmerged_copy(&repo, "unmerged");
    for reference in refs {
        let cause = first_cause(&merged, reference, &vars);
        assert!(cause.contains("unmerged"), "premise: cause 5: {cause}");
        let keys = markers_only(&bare, &[reference]);
        let read = impact_in(&merged, &[reference], &vars);
        assert_ref_degraded(
            &sound[reference],
            &read,
            Some(&cause),
            Some(&keys),
            "an unmerged lock",
        );
    }

    // (4) no git, a git that fails: the rows listed, one bindings note.
    let nowhere = repo.scratch.dir("no-git-here");
    let no_git = with_var(&vars, "PATH", nowhere.as_os_str());
    let bad_git = with_var(&vars, "PATH", &failing_git(&repo.scratch.join("bad-git")));
    for (vars, context) in [(&no_git, "no git"), (&bad_git, "a git failure")] {
        for reference in refs {
            let cause = first_cause(&root, reference, vars);
            let read = impact_in(&root, &[reference], vars);
            assert_ref_degraded(
                &sound[reference],
                &read,
                Some(&cause),
                Some(&keys_of(&all_rows(&sound[reference]))),
                context,
            );
            for printed in [&read.text.stdout, &read.json_run.stdout, &read.text.stderr] {
                assert!(
                    !printed.contains(&root.display().to_string()),
                    "{context}: the root is never printed\n{}",
                    read.show()
                );
            }
        }
    }
}

/// RD-5: `--symbol` takes code as its input, so its code failures exit 2
/// as `spec symbols QUERY`'s, stdout empty (text and JSON): no `[code]`
/// (the `[code]` line), a refused `[code]` key, a walk gap (naming the
/// file); `spec symbols` exits 2 the same way there.
#[test]
fn symbol_impact_needs_code() {
    let repo = ferry("im-needs-code");
    let vars = vars_of(&repo);
    let root = repo.root.clone();
    without_code(&root, || {
        let read = impact(&repo, &["--symbol", "opens"]);
        assert_usage(
            &read,
            &format!("spec: {NO_CODE}"),
            "--symbol without [code]",
        );
    });
    let config = repo.read("specengine.toml");
    let roots = "roots = [\"crates\", \"data\"]";
    repo.replace(
        "specengine.toml",
        roots,
        &format!("{roots}\nlanguages = [\"rust\"]"),
    );
    let lookup = run_with(&root, &["symbols", "opens"], &vars);
    assert_eq!(
        lookup.code,
        2,
        "premise: spec symbols refuses the key\n{}",
        lookup.show()
    );
    let read = impact(&repo, &["--symbol", "opens"]);
    assert_usage(
        &read,
        lookup.stderr.lines().last().expect("symbols' line"),
        "--symbol, a refused key",
    );
    repo.write("specengine.toml", &config);
    let spare = "crates/ferry/src/spare.rs";
    repo.write(spare, "pub fn spare_part() {}\n");
    let _mode = Mode::set(repo.root.join(spare), 0o000);
    let lookup = run_with(&root, &["symbols", "opens"], &vars);
    assert_eq!(
        lookup.code,
        2,
        "premise: spec symbols over a walk gap\n{}",
        lookup.show()
    );
    let read = impact(&repo, &["--symbol", "opens"]);
    assert_usage(
        &read,
        lookup.stderr.lines().last().expect("symbols' line"),
        "--symbol over a walk gap",
    );
    assert!(
        read.text.stderr.contains(spare),
        "--symbol over a walk gap names the file\n{}",
        read.show()
    );
}

// ------------------------------------------------------------------ RD-6

/// RD-6 and TE-1: tasks in every state (expected.json's table, made in
/// order through the CLI library) and one of another repository: each
/// form lists exactly expected.json's open tasks of this repository whose
/// named nodes meet the radius, by ID, each `{id, status, title, nodes}`;
/// `done`, `accepted`, `cancelled` never. Read-only: the queue's dump and
/// its schema version as before. M: a terminal task listed.
#[test]
fn impact_lists_open_tasks_only() {
    let repo = ferry("im-tasks");
    ferry_tasks(&repo, true);
    let db = data_dir(&repo.home).join(format!("{SLUG}.db"));
    let dump = || {
        SqliteQueue::open(&db, SLUG)
            .expect("the queue")
            .dump()
            .expect("dump")
    };
    let before = dump();
    let version = sql(&db, "PRAGMA user_version;");
    let table = task_table();
    for form in forms().iter().filter(|form| form["exit"] == 0) {
        let read = impact_form(&repo, &args_of(form));
        let what = format!("{} ({})", read.what, text(&form["name"]));
        read.answered();
        let document = read.json();
        assert_eq!(
            document["tasks"],
            tasks_of_form(form),
            "{what}: expected.json's tasks"
        );
        for task in list(&document["tasks"]) {
            let keys: Vec<&str> = task
                .as_object()
                .expect("a task object")
                .keys()
                .map(String::as_str)
                .collect();
            let mut want = TASK_KEYS.to_vec();
            want.sort_unstable();
            let mut have = keys;
            have.sort_unstable();
            assert_eq!(have, want, "{what}: a task's keys");
            let status = text(&table[text(&task["id"])]["status"]);
            assert!(
                !["done", "accepted", "cancelled"].contains(&status),
                "{what}: a terminal task listed: {task}"
            );
        }
    }
    assert_eq!(dump(), before, "the queue read, never written");
    assert_eq!(sql(&db, "PRAGMA user_version;"), version, "no schema step");
}

/// RD-6: the other repository's open task in the radius is never listed:
/// one note counting it (`spec impact` named, the slug, no root), absent
/// where it does not meet the radius; from the other repository the
/// counts swap (T-0012 listed, this repository's seven noted). Outside a
/// git worktree: no task, no note. A fresh `HOME`: no task, no note, no
/// queue table made.
#[test]
fn impact_tasks_are_this_repositorys() {
    let repo = ferry("im-others");
    let other = ferry_tasks(&repo, true);
    let vars = vars_of(&repo);
    for form in forms().iter().filter(|form| form["exit"] == 0) {
        let read = impact_form(&repo, &args_of(form));
        let what = format!("{} ({})", read.what, text(&form["name"]));
        let others = form["others"].as_u64().expect("others") as usize;
        let want: Vec<String> = if others > 0 {
            vec![others_note(others)]
        } else {
            Vec::new()
        };
        assert_eq!(
            task_notes(&read),
            want,
            "{what}: the other repository's note"
        );
        assert!(
            list(&read.json()["notes"])
                .iter()
                .filter(|note| text(note).contains("another repository"))
                .count()
                == want.len(),
            "{what}: the JSON's notes hold it"
        );
        assert!(
            !list(&read.json()["tasks"])
                .iter()
                .any(|task| task["id"] == "T-0012"),
            "{what}: another repository's task listed"
        );
        let scratch = repo.scratch.path().display().to_string();
        assert!(
            !read.text.stderr.contains(&scratch),
            "{what}: no absolute path"
        );
    }
    let theirs = impact_in(&other, &["ROUTE-CROSSING"], &vars);
    theirs.answered();
    let ids: Vec<String> = list(&theirs.json()["tasks"])
        .iter()
        .map(|task| text(&task["id"]).to_owned())
        .collect();
    assert_eq!(ids, ["T-0012"], "the other repository lists its own");
    assert_eq!(
        task_notes(&theirs),
        [others_note(7)],
        "and notes this one's seven"
    );

    // Outside a git worktree: no task, no note.
    let loose = repo.scratch.join("loose");
    copy_dir(&repo.root, &loose);
    fs::remove_dir_all(loose.join(".git")).expect("no .git");
    let loose = fs::canonicalize(&loose).expect("the loose copy");
    let read = impact_in(&loose, &["ROUTE-CROSSING"], &vars);
    read.answered();
    assert_eq!(read.json()["tasks"], json!([]), "outside git: no task");
    assert_eq!(
        task_notes(&read),
        Vec::<String>::new(),
        "outside git: no note"
    );

    // A fresh `HOME`: the read makes no queue table.
    let fresh = repo.scratch.home("fresh");
    let fresh_vars = with_var(&vars, "HOME", fresh.as_os_str());
    let read = impact_in(&repo.root, &["ROUTE-CROSSING"], &fresh_vars);
    read.answered();
    assert_eq!(read.json()["tasks"], json!([]), "a fresh HOME: no task");
    assert_eq!(
        task_notes(&read),
        Vec::<String>::new(),
        "a fresh HOME: no note"
    );
    let fresh_db = data_dir(&fresh).join(format!("{SLUG}.db"));
    let tables = sql(
        &fresh_db,
        "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name;",
    );
    for queue in ["tasks", "runs", "proposals", "events"] {
        assert!(
            !tables.lines().any(|table| table == queue),
            "{queue} made by a read: {tables}"
        );
    }
}

/// docs/features/impact.md "Rows and tasks", **Tasks**: under
/// `GitPlace::Unnamed` the queue's and git's failures are fixed sentences.
const QUEUE_UNREAD: &str =
    "note: tasks not shown: the queue cannot be read (spec roadmap names the cause)";
const GIT_FAILED: &str =
    "note: tasks not shown: git in the project root failed (spec roadmap names the cause)";

/// RD-6, docs/features/impact.md "Rows and tasks": an unreadable queue (a
/// newer schema) and a git that fails each give no task and one fixed
/// note holding no path ([`QUEUE_UNREAD`], [`GIT_FAILED`]), on stderr and
/// in the JSON's `notes`; the exit, nodes and rows unchanged; nothing
/// written. `spec roadmap` keeps its detailed texts. M: the store's or
/// git's error text, a path in it.
#[test]
fn impact_task_notes_hold_no_root() {
    let repo = ferry("im-task-notes");
    ferry_tasks(&repo, true);
    let vars = vars_of(&repo);
    let sound = impact(&repo, &["ROUTE-CROSSING"]);
    sound.answered();
    let sound = sound.json();
    let root = repo.root.display().to_string();
    let scratch = repo.scratch.path().display().to_string();
    let db = data_dir(&repo.home).join(format!("{SLUG}.db"));
    let no_path = |read: &Read, context: &str| {
        for printed in [&read.text.stdout, &read.json_run.stdout, &read.text.stderr] {
            assert!(
                !printed.contains(&root) && !printed.contains(&scratch),
                "{context}: no absolute path printed\n{}",
                read.show()
            );
        }
    };

    let version = sql(&db, "PRAGMA user_version;");
    sql(&db, "PRAGMA user_version = 99;");
    let read = impact(&repo, &["ROUTE-CROSSING"]);
    read.answered();
    let document = read.json();
    assert_eq!(document["tasks"], json!([]), "a newer queue: no task");
    assert_eq!(
        document["nodes"], sound["nodes"],
        "a newer queue: the nodes"
    );
    assert_eq!(
        document["symbols"], sound["symbols"],
        "a newer queue: the rows"
    );
    assert_eq!(
        task_notes(&read),
        [QUEUE_UNREAD],
        "a newer queue: the fixed note"
    );
    assert!(
        list(&document["notes"]).contains(&json!(QUEUE_UNREAD.trim_start_matches("note: "))),
        "the JSON's notes hold it"
    );
    no_path(&read, "a newer queue");
    assert_eq!(
        sql(&db, "PRAGMA user_version;").trim(),
        "99",
        "nothing written"
    );
    // `spec roadmap` keeps its detailed text (it is no fixed sentence).
    let roadmap = run_with(&repo.root, &["roadmap"], &vars);
    assert!(
        !roadmap.stderr.contains(QUEUE_UNREAD),
        "spec roadmap's own text\n{}",
        roadmap.show()
    );
    sql(&db, &format!("PRAGMA user_version = {};", version.trim()));

    let bad_git = with_var(&vars, "PATH", &failing_git(&repo.scratch.join("bad-git")));
    let read = impact_in(&repo.root, &["ROUTE-CROSSING"], &bad_git);
    read.answered();
    let document = read.json();
    assert_eq!(document["tasks"], json!([]), "a git failure: no task");
    assert_eq!(
        document["nodes"], sound["nodes"],
        "a git failure: the nodes"
    );
    assert_eq!(
        task_notes(&read),
        [GIT_FAILED],
        "a git failure: the fixed note"
    );
    assert!(
        list(&document["notes"]).contains(&json!(GIT_FAILED.trim_start_matches("note: "))),
        "the JSON's notes hold it"
    );
    no_path(&read, "a git failure");
}

// ------------------------------------------------------------------ RD-7

/// A corpus past every cut: `HUB-CORE` with 1 200 spokes depending on it
/// (long titles), 300 items and 300 tests bound to it, three tasks on it;
/// `pkg/w/src/ticks.rs` with 1 000 inline mods each holding an unbound
/// `tick` (a 1 000-match tier that seeds nothing). Its own prefixes and
/// kinds.
fn wide_corpus(root: &Path) {
    let write = |relative: &str, body: String| {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
        fs::write(path, body).expect("write");
    };
    write(
        "specengine.toml",
        "[project]\nslug = \"wide-radius\"\n\n[ids]\nHUB = { kind = \"hub\", shape = \"name\" }\n\
         SPOKE = { kind = \"spoke\", width = 4 }\n\n[code]\nroots = [\"pkg\"]\n"
            .to_owned(),
    );
    write(
        "docs/spec/hub.md",
        "---\nclass: canon\nid: HUB-CORE\nowner: owner\nreviewed: 2026-10-21\n---\n\n# Core\n\nThe hub every spoke leans on.\n".to_owned(),
    );
    for n in 0..1_200 {
        write(
            &format!("docs/records/SPOKE-{n:04}.md"),
            format!(
                "---\nclass: canon\nid: SPOKE-{n:04}\nowner: owner\nreviewed: 2026-10-21\nlinks:\n  depends_on: [HUB-CORE]\n---\n\n\
                 # Spoke {n:04} of the wide radius, its title written long to fill a line\n\nIt leans on the hub.\n"
            ),
        );
    }
    write(
        "pkg/w/Cargo.toml",
        "[package]\nname = \"w\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n"
            .to_owned(),
    );
    write(
        "pkg/w/src/lib.rs",
        "pub mod bound;\npub mod ticks;\n".to_owned(),
    );
    let mut bound = String::new();
    for n in 0..300 {
        bound.push_str(&format!(
            "// @implements HUB-CORE\npub fn bound_item_with_a_long_name_{n:04}() -> u32 {{\n    {n}\n}}\n\n\
             // @verifies HUB-CORE\n#[test]\nfn bound_test_with_a_long_name_{n:04}() {{}}\n\n"
        ));
    }
    write("pkg/w/src/bound.rs", bound);
    let mut ticks = String::new();
    for n in 0..1_000 {
        ticks.push_str(&format!(
            "pub mod m{n:04} {{\n    pub fn tick() -> u32 {{\n        {n}\n    }}\n}}\n"
        ));
    }
    write("pkg/w/src/ticks.rs", ticks);
}

/// The text before the tail: every stdout line but the tail line (and the
/// notes, which stderr holds).
fn shown_chars(stdout: &str) -> usize {
    stdout
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("[truncated: "))
        .map(|line| line.chars().count())
        .sum()
}

/// The tail's word of a `truncated` key ("Cut").
fn tail_word(key: &str) -> &'static str {
    match key {
        "nodes" => "nodes",
        "symbols" => "symbols",
        "tests" => "tests",
        "tasks" => "tasks",
        "seeds" => "seed matches",
        other => panic!("no tail part for {other}"),
    }
}

/// One cut answer (docs/features/impact.md "Cut"): the text but its tail
/// within `OUTPUT_CAP_CHARS`, the header and summary printed; the tail
/// ``[truncated: <parts> not shown; lower --depth N or narrow the REF or
/// --symbol]``, each non-zero part `truncated`'s count; the JSON holds
/// exactly the printed items (as many nodes, rows, tasks and matches as
/// lines, each printed item the JSON's in order), `truncated` counting
/// the rest; printed plus `truncated` each list's whole; the summary the
/// totals.
fn assert_cut(read: &Read, totals: &[(&str, usize)], context: &str) {
    read.answered();
    let document = read.json();
    let shown = printed(&read.text.stdout);
    assert!(
        shown_chars(&read.text.stdout) <= CAP,
        "{context}: {} characters before the tail",
        shown_chars(&read.text.stdout)
    );
    let tail = shown
        .tail
        .clone()
        .unwrap_or_else(|| panic!("{context}: no tail\n{}", read.show()));
    assert!(
        tail.ends_with(" not shown; lower --depth N or narrow the REF or --symbol]"),
        "{context}: the tail names the narrowing: {tail}"
    );
    let truncated = &document["truncated"];
    assert!(
        truncated.is_object(),
        "{context}: truncated counts: {truncated}"
    );
    assert_eq!(
        truncated["delta"],
        Value::Null,
        "{context}: truncated.delta null"
    );
    let in_json = |key: &str| -> usize {
        match key {
            "seeds" => document["seeds"]["symbols"].as_array().map_or(0, Vec::len),
            _ => document[key].as_array().map_or(0, Vec::len),
        }
    };
    let lines = |key: &str| -> usize {
        match key {
            "nodes" => shown.nodes.len(),
            "symbols" => shown.symbols,
            "tests" => shown.tests,
            "tasks" => shown.tasks.len(),
            _ => shown.matches.len(),
        }
    };
    let mut parts = Vec::new();
    for (key, total) in totals {
        let listed = in_json(key);
        assert_eq!(
            listed,
            lines(key),
            "{context}: the JSON's {key} are the printed ones"
        );
        let hidden = truncated[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{context}: truncated.{key} a count: {truncated}"))
            as usize;
        assert_eq!(
            listed + hidden,
            *total,
            "{context}: {key} printed and not shown"
        );
        if hidden > 0 {
            parts.push(format!("{hidden} {}", tail_word(key)));
        }
    }
    assert!(
        tail.starts_with(&format!("[truncated: {} not shown;", parts.join(", "))),
        "{context}: the tail's parts {parts:?}: {tail}"
    );
    let nodes: Vec<String> = list(&document["nodes"]).iter().map(node_line).collect();
    assert_eq!(
        shown.nodes, nodes,
        "{context}: the printed nodes are the JSON's"
    );
    let matches: Vec<String> = document["seeds"]["symbols"]
        .as_array()
        .into_iter()
        .flatten()
        .map(seed_line)
        .collect();
    assert_eq!(
        shown.matches, matches,
        "{context}: the printed matches are the JSON's"
    );
    let distance_0: BTreeSet<String> = list(&document["nodes"])
        .iter()
        .filter(|node| node["distance"] == 0)
        .map(name_of)
        .collect();
    assert_eq!(
        seed_names(&document),
        distance_0,
        "{context}: seeds.nodes the printed ones"
    );
    assert_eq!(
        shown.summary.as_deref(),
        Some(summary_line(&document).as_str()),
        "{context}: the summary counts the totals"
    );
}

/// RD-7 (Q-F): past the cut, the REF form (1 201 nodes, 300 rows of each
/// kind, three tasks) and a 1 000-match `--symbol` (its matches last)
/// each stay within 40 000 characters before the tail, the tail names the
/// narrowing, the JSON holds the printed items only; a `--depth 0` read of
/// the same REF prints uncut. M: no tail; an unprinted item in the JSON.
#[test]
fn impact_cut_names_narrowing() {
    let scratch = common::Scratch::new("im-cut");
    let git = Sandbox::new(scratch.path());
    let root = scratch.dir("wide");
    wide_corpus(&root);
    git.init(&root);
    git.add_all(&root);
    git.commit(&root, "the wide corpus");
    let home = scratch.home("h");
    let mut vars: Vec<(OsString, OsString)> = git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), home.clone().into_os_string()));
    for _ in 0..3 {
        let place = Place {
            home: &home,
            git: &git,
            cwd: &root,
        };
        let env = place.env();
        moved(
            task_new(
                &env,
                &Globals::default(),
                &TaskNewRequest {
                    nodes: vec!["HUB-CORE".to_owned()],
                    title: Some("Turn the hub".to_owned()),
                    goal: None,
                    author_role: None,
                    author_model: None,
                    run: None,
                    now: TASK_CLOCK.to_owned(),
                    git: place.git_env(),
                },
            ),
            "task new",
        );
    }
    let read = impact_in(&root, &["HUB-CORE"], &vars);
    assert_cut(
        &read,
        &[
            ("nodes", 1_201),
            ("symbols", 300),
            ("tests", 300),
            ("tasks", 3),
        ],
        "HUB-CORE",
    );
    let summary = printed(&read.text.stdout).summary;
    assert!(
        summary.is_some(),
        "the summary prints before the lists\n{}",
        read.show()
    );

    let read = impact_in(&root, &["--symbol", "tick"], &vars);
    assert_cut(&read, &[("seeds", 1_000)], "--symbol tick");
    assert_eq!(read.json()["nodes"], json!([]), "the ticks seed nothing");

    // `--depth 0`: the hub alone, its rows still past the cut.
    let narrow = impact_in(&root, &["HUB-CORE", "--depth", "0"], &vars);
    assert_cut(
        &narrow,
        &[("nodes", 1), ("symbols", 300), ("tests", 300), ("tasks", 3)],
        "HUB-CORE --depth 0",
    );

    // Without `[code]` (docs/features/impact.md "JSON"): `truncated`'s
    // `symbols` and `tests` null for their null lists, `seeds` null for a
    // REF; the nodes still cut, the tasks behind them.
    let config = fs::read_to_string(root.join("specengine.toml")).expect("the config");
    fs::write(
        root.join("specengine.toml"),
        config.replace("\n[code]\nroots = [\"pkg\"]\n", "\n"),
    )
    .expect("no [code]");
    let read = impact_in(&root, &["HUB-CORE"], &vars);
    assert_cut(
        &read,
        &[("nodes", 1_201), ("tasks", 3)],
        "HUB-CORE, no [code]",
    );
    let truncated = &read.json()["truncated"];
    for key in ["symbols", "tests", "seeds", "delta"] {
        assert_eq!(
            truncated[key],
            Value::Null,
            "no [code]: truncated.{key} null"
        );
    }
    for key in ["symbols", "tests", "code"] {
        assert_eq!(read.json()[key], Value::Null, "no [code]: {key} null");
    }
    fs::write(root.join("specengine.toml"), config).expect("[code] back");
}

// ------------------------------------------------------------------ TE-3

/// The reversed-order copy of a worktree (`.git` included).
fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("type");
        if kind.is_dir() {
            copy_reversed(&entry.path(), &target);
        } else if kind.is_file() {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            let mode = fs::metadata(entry.path()).expect("meta").permissions();
            fs::set_permissions(&target, mode).expect("mode");
        }
    }
}

/// The code files of a worktree (`.rs`, `.ron`), `.git` aside.
fn code_paths(root: &Path) -> Vec<String> {
    snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some()
                && !path.starts_with(".git")
                && (path.ends_with(".rs") || path.ends_with(".ron"))
        })
        .map(|(path, _)| path)
        .collect()
}

/// This repository's tasks of expected.json made at `cwd` in `home`.
fn tasks_at(home: &Path, git: &Sandbox, cwd: &Path) {
    for task in list(&expected()["tasks"]) {
        if task["repository"] == "other" {
            continue;
        }
        made(&Place { home, git, cwd }, &task);
    }
}

/// The determinism forms: REF, a section, a depth, `--symbol` forms.
const STEADY: [&[&str]; 6] = [
    &["ROUTE-CROSSING"],
    &["TIDE-WINDOW"],
    &["ROUTE-DOCKS", "--depth", "2"],
    &["--symbol", "opens"],
    &["--symbol", "reach"],
    &["--symbol", "rows"],
];

/// TE-3: with the fixture drifted and this repository's tasks in each
/// place's own `HOME` (the same clock), every [`STEADY`] form's exit,
/// text, JSON and stderr are byte-identical repeated, from another root,
/// from another cwd, from a copy made in reverse creation order and from a
/// checkout of every committed code file CRLF. M: rows in rowid order; an
/// absolute path in the output; a line read with its CR.
#[test]
fn impact_is_byte_identical_across_roots_order_and_crlf() {
    let first = ferry_at("im-det-a", "repo");
    let second = ferry_at("im-det-b", "elsewhere/deeper");
    for repo in [&first, &second] {
        ferry_drift(repo);
        tasks_at(&repo.home, &repo.git, &repo.root);
    }
    let reversed = first.scratch.join("reversed");
    copy_reversed(&first.root, &reversed);
    let reversed = fs::canonicalize(&reversed).expect("the reversed copy");
    let reversed_home = first.scratch.home("reversed");
    tasks_at(&reversed_home, &first.git, &reversed);

    let crlf = clone_dir(&first, "crlf");
    first.git.git(&crlf, &["config", "core.autocrlf", "true"]);
    let committed: BTreeSet<String> = first
        .git_text(&["ls-files"])
        .lines()
        .map(str::to_owned)
        .collect();
    let dirty: BTreeSet<String> = first
        .git(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .split(|byte| *byte == 0)
        .filter(|entry| entry.len() > 3)
        .map(|entry| String::from_utf8_lossy(&entry[3..]).into_owned())
        .collect();
    let mut crlf_files = 0;
    for path in code_paths(&crlf) {
        if !committed.contains(&path) || dirty.contains(&path) {
            continue;
        }
        fs::remove_file(crlf.join(&path)).expect("remove");
        first.git.git(&crlf, &["checkout", "--", &path]);
        if fs::read(crlf.join(&path))
            .expect("checked out")
            .windows(2)
            .any(|pair| pair == b"\r\n")
        {
            crlf_files += 1;
        }
    }
    assert!(crlf_files > 0, "premise: CRLF code files in the worktree");
    let crlf_home = first.scratch.home("crlf");
    tasks_at(&crlf_home, &first.git, &crlf);

    let vars = vars_of(&first);
    let other_vars = vars_of(&second);
    let reversed_vars = with_var(&vars, "HOME", reversed_home.as_os_str());
    let crlf_vars = with_var(&vars, "HOME", crlf_home.as_os_str());
    let deeper = second.root.join("crates").join("ferry");
    for args in STEADY {
        let reference = impact(&first, args);
        reference.answered();
        assert!(
            !list(&reference.json()["tasks"]).is_empty() || args[0] == "--symbol",
            "premise: {args:?} lists tasks"
        );
        let runs = [
            ("a repeat run", impact(&first, args)),
            ("another root", impact(&second, args)),
            ("another cwd", impact_in(&deeper, args, &other_vars)),
            (
                "a reverse creation order",
                impact_in(&reversed, args, &reversed_vars),
            ),
            ("a CRLF checkout", impact_in(&crlf, args, &crlf_vars)),
        ];
        for (context, run) in runs {
            assert_eq!(
                run.bytes(),
                reference.bytes(),
                "{args:?}: {context}\n{}",
                first_difference(&run, &reference)
            );
        }
        for printed in [
            &reference.text.stdout,
            &reference.json_run.stdout,
            &reference.text.stderr,
        ] {
            assert!(
                !printed.contains(&first.scratch.path().display().to_string()),
                "{args:?}: no absolute path printed"
            );
        }
    }
}

// ------------------------------------------------------------------ TE-5

/// TE-5: `fixtures/spec-c` (a Python dock-hire service, no `[code]`):
/// every REF answers exit 0 with graph's nodes, both code lists `null` and
/// no bindings note; `--symbol` exits 2 naming `[code]`, stdout empty.
#[test]
fn impact_on_another_genre_reads_no_code() {
    let repo = LockRepo::of("im-genre", "spec-c", "repo", &[]);
    let vars = vars_of(&repo);
    let tree = run_with(&repo.root, &["tree", "--json"], &vars);
    assert_eq!(tree.code, 0, "{}", tree.show());
    let ids: Vec<String> = list(&common::one_json_document(&tree.stdout).expect("tree")["nodes"])
        .iter()
        .filter_map(|node| node["id"].as_str().map(str::to_owned))
        .collect();
    assert!(ids.len() > 3, "premise: spec-c's nodes: {ids:?}");
    for reference in &ids {
        if run_with(&repo.root, &["show", reference, "--json"], &vars).code != 0 {
            // A feature-scoped ID needs its feature: its document's REF
            // covers it.
            continue;
        }
        let read = impact(&repo, &[reference]);
        read.answered();
        let document = read.json();
        let graph = graph_impact(&repo.root, reference, None, &vars);
        assert_eq!(
            document["nodes"], graph["nodes"],
            "{reference}: graph's nodes"
        );
        for key in ["symbols", "tests"] {
            assert_eq!(document[key], Value::Null, "{reference}: {key} null");
        }
        assert_eq!(
            read.binding_notes(),
            Vec::<String>::new(),
            "{reference}: silent"
        );
    }
    let read = impact(&repo, &["--symbol", "fee"]);
    assert_usage(&read, &format!("spec: {NO_CODE}"), "spec-c --symbol");
}

// ------------------------------------------------------------------ TE-7

/// `git status --porcelain=v1 -z --untracked-files=all` without the
/// optional index refresh.
fn status(repo: &LockRepo) -> Vec<u8> {
    repo.git(&[
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ])
}

fn index_mtime(root: &Path) -> Option<(i64, i64)> {
    fs::metadata(root.join(".git").join("index"))
        .ok()
        .map(|meta| (meta.mtime(), meta.mtime_nsec()))
}

fn objects(root: &Path) -> usize {
    snapshot(&root.join(".git").join("objects"))
        .values()
        .filter(|bytes| bytes.is_some())
        .count()
}

/// The queue's rows and events' count.
fn queue_state(repo: &LockRepo) -> Vec<String> {
    let db = data_dir(&repo.home).join(format!("{SLUG}.db"));
    if !db.exists() {
        return Vec::new();
    }
    let queue = SqliteQueue::open(&db, SLUG).expect("the queue opens");
    let mut state: Vec<String> = queue
        .dump()
        .expect("the dump")
        .lines()
        .map(str::to_owned)
        .collect();
    state.push(format!("events {}", queue.events().expect("events").len()));
    state
}

/// What a read must leave as it found: `.git/index`'s mtime, the objects'
/// count, every byte under the worktree (`.git` included), `git status`,
/// `spec.lock`, the queue.
struct Untouched {
    index_mtime: Option<(i64, i64)>,
    objects: usize,
    files: BTreeMap<String, Option<Vec<u8>>>,
    status: Vec<u8>,
    lock: Option<Vec<u8>>,
    queue: Vec<String>,
}

impl Untouched {
    fn take(repo: &LockRepo) -> Self {
        let status = status(repo);
        Self {
            index_mtime: index_mtime(&repo.root),
            objects: objects(&repo.root),
            files: snapshot(&repo.root),
            status,
            lock: repo.lock(),
            queue: queue_state(repo),
        }
    }

    #[track_caller]
    fn still(&self, repo: &LockRepo, context: &str) {
        assert_eq!(
            index_mtime(&repo.root),
            self.index_mtime,
            "{context}: .git/index's mtime"
        );
        assert_eq!(objects(&repo.root), self.objects, "{context}: git objects");
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
        assert_eq!(repo.lock(), self.lock, "{context}: spec.lock's bytes");
        assert_eq!(queue_state(repo), self.queue, "{context}: the queue");
        assert_eq!(status(repo), self.status, "{context}: git status");
    }
}

/// TE-7 (writes): over the drifted fixture with its tasks, every form of
/// expected.json, a look-alike and a usage error, from the root, from a
/// subdirectory and repeated, then degraded (no git): `.git/index`'s
/// mtime, the objects' count, every byte under the worktree, `git
/// status`, `spec.lock` and the queue as before; each byte-identical from
/// the subdirectory and repeated; no absolute path printed. M: `git
/// status` or `git diff` run by the read (they refresh `.git/index`).
#[test]
fn impact_writes_nothing_under_the_root() {
    let repo = ferry("im-door");
    ferry_tasks(&repo, true);
    ferry_drift(&repo);
    let before = Untouched::take(&repo);
    assert!(before.queue.len() > 1, "premise: tasks in the queue");
    let mut calls: Vec<Vec<String>> = forms().iter().map(args_of).collect();
    calls.push(vec![CROSSING_LOOK_ALIKE.to_owned()]);
    calls.push(vec![
        "ROUTE-CROSSING".to_owned(),
        "--symbol".to_owned(),
        "opens".to_owned(),
    ]);
    let vars = vars_of(&repo);
    let deeper = repo.root.join("crates").join("ferry");
    for args in &calls {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = impact(&repo, &args);
        before.still(&repo, &run.what);
        let below = impact_in(&deeper, &args, &vars);
        before.still(&repo, &below.what);
        assert_eq!(below.bytes(), run.bytes(), "{args:?} from a subdirectory");
        let again = impact(&repo, &args);
        before.still(&repo, &again.what);
        assert_eq!(again.bytes(), run.bytes(), "{args:?} repeated");
        let scratch = repo.scratch.path().display().to_string();
        for printed in [&run.text.stdout, &run.json_run.stdout, &run.text.stderr] {
            assert!(
                !printed.contains(&scratch),
                "{args:?}: no absolute path printed"
            );
        }
    }
    let no_git = with_var(&vars, "PATH", repo.scratch.dir("no-git-here").as_os_str());
    let degraded = impact_in(&repo.root, &["ROUTE-CROSSING"], &no_git);
    degraded.answered();
    before.still(&repo, "a degraded read");
}

// ----------------------------------------------------- the JSON, the text

/// The keys of the first object of the list printed as `"<key>":[{` after
/// the first `after` in `raw`, in order.
fn list_object_keys(raw: &str, after: &str, key: &str) -> Option<Vec<String>> {
    let start = raw.find(after)?;
    let tail = &raw[start..];
    let needle = format!("\"{key}\":[{{");
    tail.find(&needle)
        .map(|at| raw_order(tail, at + needle.len() - 1))
}

/// docs/features/impact.md "JSON": every key present in order, absent
/// null; `revision` and `delta` null and `input.since` null (A5); `seeds`
/// `{nodes, symbols}`; each node graph's object, each row the 14 keys in
/// order, each task `{id, status, title, nodes}`, each match its six keys;
/// `code` `spec refs`' counts (null without `[code]`); `notes` a list.
#[test]
fn impact_json_holds_every_key() {
    let repo = ferry("im-keys");
    ferry_tasks(&repo, true);
    for args in [
        &["ROUTE-CROSSING"][..],
        &["--symbol", "opens"],
        &["--symbol", "rows"],
        &["ROUTE-NOWHERE"],
        &["--symbol", "nothing_here"],
    ] {
        let read = impact(&repo, args);
        let raw = &read.json_run.stdout;
        assert_eq!(
            raw_order(raw, 0),
            TOP_KEYS,
            "{args:?}: the keys in order\n{raw}"
        );
        let document = read.json();
        assert_eq!(
            raw_order(raw, raw.find("\"input\":{").expect("input") + 8),
            ["ref", "symbol", "since"],
            "{args:?}: input's keys"
        );
        assert_eq!(
            document["input"]["since"],
            Value::Null,
            "{args:?}: since null (A5)"
        );
        assert_eq!(
            document["revision"],
            Value::Null,
            "{args:?}: revision null (A5)"
        );
        assert_eq!(document["delta"], Value::Null, "{args:?}: delta null (A5)");
        assert!(document["notes"].is_array(), "{args:?}: notes a list");
        assert_eq!(
            raw_order(raw, raw.find("\"seeds\":{").expect("seeds") + 8),
            ["nodes", "symbols"],
            "{args:?}: seeds' keys"
        );
        if let Some(keys) = list_object_keys(raw, "\"left_out\":", "nodes") {
            assert_eq!(keys, NODE_KEYS, "{args:?}: graph's node object");
        }
        for (key, want) in [
            ("symbols", &ROW_KEYS[..]),
            ("tests", &ROW_KEYS[..]),
            ("tasks", &TASK_KEYS[..]),
        ] {
            if let Some(keys) = list_object_keys(raw, "\"left_out\":", key) {
                assert_eq!(keys, want, "{args:?}: {key}' keys in order");
            }
        }
        if document["seeds"]["symbols"]
            .as_array()
            .is_some_and(|matches| !matches.is_empty())
        {
            assert_eq!(
                list_object_keys(raw, "\"seeds\":", "symbols"),
                Some(MATCH_KEYS.iter().map(|key| (*key).to_owned()).collect()),
                "{args:?}: a match's keys"
            );
        }
        if read.code() == 0 {
            let code = &document["code"];
            let mut keys: Vec<&str> = code
                .as_object()
                .expect("code counts")
                .keys()
                .map(String::as_str)
                .collect();
            keys.sort_unstable();
            let mut want = CODE_KEYS.to_vec();
            want.sort_unstable();
            assert_eq!(keys, want, "{args:?}: code's keys");
        }
    }
    without_code(&repo.root, || {
        let read = impact(&repo, &["ROUTE-CROSSING"]);
        read.answered();
        assert_eq!(read.json()["code"], Value::Null, "no [code]: code null");
    });
}

/// The node lines of `spec graph REF --impact [--depth N]`'s text.
fn graph_node_lines(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Vec<String> {
    let mut all = vec!["graph", args[0], "--impact"];
    all.extend_from_slice(&args[1..]);
    let run = run_with(cwd, &all, vars);
    assert_eq!(run.code, 0, "premise: graph {args:?}\n{}", run.show());
    run.stdout
        .lines()
        .filter(|line| {
            line.split(' ')
                .next()
                .is_some_and(|word| word.parse::<u64>().is_ok())
        })
        .map(|line| format!("node {line}"))
        .collect()
}

/// docs/features/impact.md "Text": an uncut answer prints, in order, the
/// `--symbol` header, the summary (totals, the code counts, the left-out
/// suffix), each node as `node ` + graph's node line (equal to `spec graph
/// --impact`'s text for a REF and a single seed), each row under `symbol`
/// and `test` with its `sig` and anchor lines, each task and each match
/// ([`text_of`] over the JSON); a REF prints no seed line; no tail.
#[test]
fn impact_text_prints_its_json() {
    let repo = ferry("im-text");
    ferry_tasks(&repo, true);
    ferry_drift(&repo);
    let vars = vars_of(&repo);
    for form in forms().iter().filter(|form| form["exit"] == 0) {
        let args = args_of(form);
        let read = impact_form(&repo, &args);
        read.answered();
        let document = read.json();
        assert_eq!(document["truncated"], Value::Null, "{}: uncut", read.what);
        assert_eq!(
            read.text.stdout,
            text_of(&document),
            "{}: the text prints the JSON in Text's order",
            read.what
        );
        let shown = printed(&read.text.stdout);
        assert_eq!(
            shown.seed_header.is_some(),
            args[0] == "--symbol",
            "{}: a seed header for --symbol only",
            read.what
        );
        let start: Vec<String> = if args[0] == "--symbol" {
            match list(&form["seeds"]["nodes"]).as_slice() {
                [seed] => vec![text(seed).to_owned()],
                _ => continue,
            }
        } else {
            args.clone()
        };
        let start: Vec<&str> = start.iter().map(String::as_str).collect();
        assert_eq!(
            shown.nodes,
            graph_node_lines(&repo.root, &start, &vars),
            "{}: graph's node lines",
            read.what
        );
    }

    // A keyless row (a marker above a `use`: no item binds it): its
    // `<binding>` is the one `spec verify --node` prints for it.
    repo.replace(
        "crates/ferry/src/fares.rs",
        "//! Fares per crossing.\n",
        "//! Fares per crossing.\n\n// @implements FARE-02\nuse std::fmt as _;\n",
    );
    let read = impact(&repo, &["FARE-02"]);
    read.answered();
    let document = read.json();
    let keyless: Vec<Value> = list(&document["symbols"])
        .into_iter()
        .filter(|row| row["kind"].is_null())
        .collect();
    assert_eq!(
        keyless.len(),
        1,
        "premise: one keyless row\n{}",
        read.show()
    );
    let judged = judged_with(&repo.root, "FARE-02", &vars);
    let at = list(&judged.document["rows"])
        .iter()
        .position(|row| row["kind"].is_null())
        .expect("verify's keyless row");
    let binding = judged.bindings[at].clone();
    let mut want: Vec<String> = vec![summary_line(&document)];
    want.extend(list(&document["nodes"]).iter().map(node_line));
    for row in list(&document["symbols"]) {
        let binding = if row["kind"].is_null() {
            binding.clone()
        } else {
            binding_of(&row)
        };
        want.extend(row_text("symbol", &row, &binding));
    }
    for row in list(&document["tests"]) {
        want.extend(row_text("test", &row, &binding_of(&row)));
    }
    want.extend(list(&document["tasks"]).iter().map(task_line));
    assert_eq!(
        read.text.stdout,
        format!("{}\n", want.join("\n")),
        "the keyless row prints verify's binding words"
    );
}
