//! docs/features/roadmap-position.md (Phase 6, slice 2a) RD-2 and RD-3,
//! the CLI half: `spec roadmap` joined, read-only, with this repository's
//! task rows over scratch git copies of `fixtures/roadmap-a` (a harbor
//! game's plan) and over a generated 505-item corpus. Every task moves
//! through the real library commands (`task_common`'s `Tasks` on a
//! `Pair` whose slug is the fixture's) on the real queue; `spec roadmap`
//! runs as the binary.
//!
//! - RD-2: an open task (not `done`, `accepted`, `cancelled`) executing an
//!   item is shown on it by number with its state, an `in_progress` one
//!   marks the item in progress; the item's own `status` stays the
//!   file's; the git-derived keys survive the database's loss byte for
//!   byte; other repositories' tasks are one note; closed tasks are not
//!   listed; outside a git worktree, or with no queue table, no task and
//!   no note, and `spec roadmap` creates no queue table; an unreadable
//!   queue (a newer schema, a corrupt row, `T` or `PR` in `[ids]`, git
//!   failing) is one note, never an exit.
//! - Pool tasks (Q-C): a task executing no open item prints after the
//!   pool's items, JSON `pool_tasks`, and only for `--pool` or no flag;
//!   its agent-written title prints on one line, controls escaped.
//! - RD-3: the cut's `n` and `k` count the pool tasks' lines, and each
//!   task line costs its characters (`pool_task_lines_fill_the_cap`).
//!
//! Named mutations: M3 (a claimed task ignored) turns
//! `claimed_task_marks_its_item_in_progress` red; M4 (task lines not
//! counted in the cut) `roadmap_cut_counts_task_lines`.
//!
//! The text shapes of the join (the item line's task cell, the pool task
//! line) are this file's [`cell`] and [`pool_line`]; every other line is
//! 1a's, from the fixture's `expected.txt`.

#![cfg(unix)]

mod common;
mod task_common;

use std::fs;
use std::path::Path;

use common::git::Sandbox;
use common::proposal::{Pair, sqlite3};
use common::{Run, Scratch, fixture, write};
use serde_json::{Value, json};
use task_common::{CLOCK, Tasks as _};

/// The fixture of every case but the cut and its slug.
const FIXTURE: &str = "roadmap-a";
const SLUG: &str = "tide-runner";

/// The character cap of a read's output (`cap::OUTPUT_CAP_CHARS`).
const CAP: usize = 40_000;

// ------------------------------------------------------------------ oracle

fn expected_text() -> String {
    fs::read_to_string(fixture(FIXTURE).join("expected.txt")).expect("expected.txt")
}

fn expected_json() -> Value {
    serde_json::from_str(&fs::read_to_string(fixture(FIXTURE).join("expected.json")).unwrap())
        .expect("expected.json is JSON")
}

/// The plan's own notes, as `expected.json` lists them.
fn plan_notes() -> Vec<String> {
    expected_json()["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note.as_str().unwrap().to_owned())
        .collect()
}

/// The task cell an item's line gains: its open tasks by number, each
/// with its state.
fn cell(tasks: &[(&str, &str)]) -> String {
    let named: Vec<String> = tasks
        .iter()
        .map(|(id, status)| format!("{id} {status}"))
        .collect();
    format!(" | task {}", named.join(", "))
}

/// The title of every task made here (`Tasks::new_ok`).
const TITLE: &str = "Tune the regeneration";

/// A pool task: its ID, state and targets as stored.
type Pooled<'a> = (&'a str, &'a str, &'a [&'a str]);

/// A pool task's line, after the pool's items: `  - <T> | <state> |
/// <title or -> | <targets or ->`.
fn pool_line((id, status, targets): Pooled<'_>) -> String {
    let targets = if targets.is_empty() {
        "-".to_owned()
    } else {
        targets.join(", ")
    };
    format!("  - {id} | {status} | {TITLE} | {targets}")
}

/// The ID of an item line (`  <n>. <ID> | …`, `  - <ID> | …`).
fn line_id(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("  ")?;
    let rest = rest.strip_prefix("- ").unwrap_or(rest);
    let rest = rest.split_once(". ").map_or(rest, |(_, rest)| rest);
    rest.split(" | ").next()
}

/// `expected.txt` with `cells` (item ID, its tasks) appended to their
/// item lines and `pool` (task ID, state) printed after the pool's items.
fn text_with(cells: &[(&str, &[(&str, &str)])], pool: &[Pooled<'_>]) -> String {
    let mut out = String::new();
    let mut in_pool = false;
    for line in expected_text().lines() {
        if !line.starts_with("  ") {
            if in_pool {
                for task in pool {
                    out.push_str(&pool_line(*task));
                    out.push('\n');
                }
            }
            in_pool = line.starts_with("pool | ");
        }
        out.push_str(line);
        if let Some((_, tasks)) = cells
            .iter()
            .find(|(id, _)| line.starts_with("  ") && line_id(line) == Some(*id))
        {
            out.push_str(&cell(tasks));
        }
        out.push('\n');
    }
    assert!(!in_pool, "roadmap-a lists unplaced after the pool");
    out
}

/// The JSON of `{id, status}` pairs.
fn task_values(tasks: &[(&str, &str)]) -> Value {
    Value::Array(
        tasks
            .iter()
            .map(|(id, status)| json!({"id": id, "status": status}))
            .collect(),
    )
}

/// The JSON `pool_tasks` of `pool`: `{id, status, title, targets}`.
fn pool_values(pool: &[Pooled<'_>]) -> Value {
    Value::Array(
        pool.iter()
            .map(|(id, status, targets)| {
                json!({"id": id, "status": status, "title": TITLE, "targets": targets})
            })
            .collect(),
    )
}

/// `expected.json` with the items' `tasks` of `cells` and `pool_tasks`.
fn json_with(cells: &[(&str, &[(&str, &str)])], pool: &[Pooled<'_>]) -> Value {
    let mut value = expected_json();
    for group in value["groups"].as_array_mut().unwrap() {
        for item in group["items"].as_array_mut().unwrap() {
            let id = item["id"].as_str().unwrap().to_owned();
            if let Some((_, tasks)) = cells.iter().find(|(want, _)| *want == id) {
                item["tasks"] = task_values(tasks);
            }
        }
    }
    value["pool_tasks"] = pool_values(pool);
    value
}

/// A document without the task keys of the join: each item's `tasks`
/// and the top-level `pool_tasks` dropped (the git-derived keys).
fn git_keys(value: &Value) -> Value {
    let mut value = value.clone();
    value.as_object_mut().unwrap().remove("pool_tasks");
    for group in value["groups"].as_array_mut().unwrap() {
        for item in group["items"].as_array_mut().unwrap() {
            item.as_object_mut().unwrap().remove("tasks");
        }
    }
    value
}

/// The stderr `note:` lines of a run, without the prefix.
fn notes(run: &Run) -> Vec<String> {
    run.stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .map(str::to_owned)
        .collect()
}

/// The notes of a run that are not the plan's own.
fn queue_notes(run: &Run) -> Vec<String> {
    let plan = plan_notes();
    notes(run)
        .into_iter()
        .filter(|note| !plan.contains(note))
        .collect()
}

#[track_caller]
fn assert_text(run: &Run, want: &str, context: &str) {
    run.code(0);
    assert!(
        run.stdout == want,
        "{context}: stdout differs\n--- got\n{}--- want\n{want}--- stderr\n{}",
        run.stdout,
        run.stderr
    );
}

#[track_caller]
fn assert_json(run: &Run, want: &Value, context: &str) {
    run.code(0);
    let got = run.json();
    if &got != want {
        panic!(
            "{context}: JSON differs\n--- got\n{}\n--- want\n{}\n--- stderr\n{}",
            serde_json::to_string_pretty(&got).unwrap(),
            serde_json::to_string_pretty(want).unwrap(),
            run.stderr
        );
    }
}

// ------------------------------------------------------------------- setup

/// A git repository of a scratch copy of [`FIXTURE`] (one commit on its
/// default branch) and a `HOME` of its own; the "linked" worktree is the
/// main one (no case here needs a second).
fn plan_pair(label: &str) -> Pair {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let main = scratch.copy(FIXTURE, "main");
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the plan");
    let home = scratch.home("h");
    Pair {
        scratch,
        git,
        main: main.clone(),
        linked: main,
        home,
        slug: SLUG.to_owned(),
    }
}

/// Another repository of the same project: a copy of [`FIXTURE`] at
/// `<scratch>/<dir>`, one commit; the pair's `HOME` (one queue).
fn other_repository(pair: &Pair, dir: &str) -> std::path::PathBuf {
    let other = pair.scratch.copy(FIXTURE, dir);
    pair.git.init(&other);
    pair.git.add_all(&other);
    pair.git.commit(&other, "another repository");
    other
}

/// `spec roadmap <args>` (the binary) in `cwd`, the pair's `HOME` and git
/// sandbox, stdin no terminal.
fn roadmap(pair: &Pair, cwd: &Path, args: &[&str]) -> Run {
    let mut all = vec!["roadmap"];
    all.extend_from_slice(args);
    pair.spec_piped(cwd, &all, b"")
}

/// `spec --json roadmap <args>`.
fn roadmap_json(pair: &Pair, cwd: &Path, args: &[&str]) -> Run {
    let mut all = vec!["--json", "roadmap"];
    all.extend_from_slice(args);
    pair.spec_piped(cwd, &all, b"")
}

/// A `draft` task of `cwd`'s repository targeting `nodes`: its ID.
fn drafted(pair: &Pair, cwd: &Path, nodes: &[&str]) -> String {
    pair.new_ok(cwd, nodes)
}

/// A `review` task (planned).
fn reviewed(pair: &Pair, cwd: &Path, nodes: &[&str]) -> String {
    let id = drafted(pair, cwd, nodes);
    pair.plan_ok(cwd, &id, &["Holds."], &[]);
    id
}

/// A `ready` task (approved by the owner from `draft`).
fn ready(pair: &Pair, cwd: &Path, nodes: &[&str]) -> String {
    let id = drafted(pair, cwd, nodes);
    pair.approve_task(cwd, &id);
    id
}

/// An `in_progress` task (claimed in `cwd`).
fn claimed(pair: &Pair, cwd: &Path, nodes: &[&str]) -> String {
    let id = ready(pair, cwd, nodes);
    pair.claim_ok(cwd, &id, cwd);
    id
}

/// A `done` task (claimed, reported, completed).
fn completed(pair: &Pair, cwd: &Path, nodes: &[&str]) -> String {
    let id = claimed(pair, cwd, nodes);
    pair.report_ok(cwd, &id);
    let outcome = pair
        .complete(cwd, &id)
        .unwrap_or_else(|error| panic!("task complete {id}: {error}"));
    assert_eq!(
        outcome.status.map(|s| s.as_str()),
        Some("done"),
        "{outcome:?}"
    );
    id
}

/// A `cancelled` task (cancelled by the owner from `draft`).
fn cancelled(pair: &Pair, cwd: &Path, nodes: &[&str]) -> String {
    let id = drafted(pair, cwd, nodes);
    let (outcome, questions) = pair.owner(cwd, "cancel", &id, None, true, CLOCK);
    let outcome = outcome.unwrap_or_else(|error| panic!("task cancel {id}: {error}"));
    assert_eq!(outcome.refusal, None, "task cancel {id}: {outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    id
}

/// The stored state of task `id`, read back through `spec task list`.
fn stored_status(pair: &Pair, id: &str) -> String {
    let listed = pair.list_json(&pair.main);
    listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == id)
        .unwrap_or_else(|| panic!("{id} listed: {listed}"))["status"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The names of the tables of the database at `db`, sorted.
fn tables(db: &Path) -> Vec<String> {
    let output = std::process::Command::new(sqlite3())
        .arg(db)
        .arg("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name;")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The database's `user_version`.
fn user_version(pair: &Pair) -> String {
    pair.sql("PRAGMA user_version;").trim().to_owned()
}

// ------------------------------------------------------------------- RD-2

/// M3 (Q-B): a `ready` task executing RM-0011 is shown on its line with
/// its state; claimed, it shows `in_progress` (the item in progress); a
/// second, `draft` task on the same item follows it by number. The item's
/// own `status` column stays the file's (`draft`), the first line and
/// every other line are 1a's, JSON `tasks` per item, `pool_tasks` empty.
#[test]
fn claimed_task_marks_its_item_in_progress() {
    let pair = plan_pair("rp-claimed");
    let main = pair.main.clone();

    let first = ready(&pair, &main, &["RM-0011"]);
    assert_eq!(first, "T-0001");
    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0011", &[("T-0001", "ready")])];
    assert_text(
        &roadmap(&pair, &main, &[]),
        &text_with(cells, &[]),
        "a ready task",
    );
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &json_with(cells, &[]),
        "a ready task",
    );

    pair.claim_ok(&main, &first, &main);
    assert_eq!(stored_status(&pair, &first), "in_progress");
    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0011", &[("T-0001", "in_progress")])];
    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &text_with(cells, &[]), "the claimed task");
    assert_eq!(queue_notes(&run), Vec::<String>::new(), "{}", run.stderr);
    let line = run
        .stdout
        .lines()
        .find(|line| line_id(line) == Some("RM-0011"))
        .expect("RM-0011's line");
    assert!(
        line.starts_with("  5. RM-0011 | Boat physics | draft | should | 3 | "),
        "the item's own status stays the file's: {line}"
    );
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &json_with(cells, &[]),
        "the claimed task",
    );

    let second = drafted(&pair, &main, &["RM-0011"]);
    assert_eq!(second, "T-0002");
    let cells: &[(&str, &[(&str, &str)])] =
        &[("RM-0011", &[("T-0001", "in_progress"), ("T-0002", "draft")])];
    assert_text(
        &roadmap(&pair, &main, &[]),
        &text_with(cells, &[]),
        "two tasks by number",
    );
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &json_with(cells, &[]),
        "two tasks by number",
    );

    // `--release beta` carries the cell; `--release launch` none.
    let run = roadmap(&pair, &main, &["--release", "beta"]);
    run.code(0);
    assert!(
        run.stdout
            .contains(&cell(&[("T-0001", "in_progress"), ("T-0002", "draft")])),
        "{}",
        run.stdout
    );
    let run = roadmap(&pair, &main, &["--release", "launch"]);
    run.code(0);
    assert!(!run.stdout.contains("T-000"), "{}", run.stdout);
}

/// Q-B: every open state shows on its item, by number: `draft`, `review`,
/// `changes_requested`, `ready`, `in_progress`, each its own item, and
/// one item holding three of them.
#[test]
fn every_open_state_is_shown_by_number() {
    let pair = plan_pair("rp-states");
    let main = pair.main.clone();

    let draft = drafted(&pair, &main, &["RM-0017"]);
    let review = reviewed(&pair, &main, &["RM-0013"]);
    let returned = reviewed(&pair, &main, &["RM-0014"]);
    let (outcome, _) = pair.owner(&main, "changes", &returned, Some("Narrow it."), true, CLOCK);
    assert_eq!(outcome.expect("changes").refusal, None, "changes");
    let approved = ready(&pair, &main, &["RM-0030"]);
    let working = claimed(&pair, &main, &["RM-0040"]);
    // RM-0012: three tasks, made out of number order of their states.
    let late_ready = ready(&pair, &main, &["RM-0012"]);
    let late_draft = drafted(&pair, &main, &["RM-0012"]);
    let late_claimed = claimed(&pair, &main, &["RM-0012"]);
    assert_eq!(
        [
            &draft,
            &review,
            &returned,
            &approved,
            &working,
            &late_ready,
            &late_draft,
            &late_claimed
        ],
        [
            "T-0001", "T-0002", "T-0003", "T-0004", "T-0005", "T-0006", "T-0007", "T-0008"
        ]
    );
    assert_eq!(stored_status(&pair, &returned), "changes_requested");

    let cells: &[(&str, &[(&str, &str)])] = &[
        ("RM-0017", &[("T-0001", "draft")]),
        ("RM-0013", &[("T-0002", "review")]),
        ("RM-0014", &[("T-0003", "changes_requested")]),
        ("RM-0030", &[("T-0004", "ready")]),
        ("RM-0040", &[("T-0005", "in_progress")]),
        (
            "RM-0012",
            &[
                ("T-0006", "ready"),
                ("T-0007", "draft"),
                ("T-0008", "in_progress"),
            ],
        ),
    ];
    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &text_with(cells, &[]), "every open state");
    let json = roadmap_json(&pair, &main, &[]);
    assert_json(&json, &json_with(cells, &[]), "every open state");
    // One state, one result: a second read is byte-identical.
    assert_eq!(roadmap(&pair, &main, &[]).stdout, run.stdout);
    assert_eq!(roadmap_json(&pair, &main, &[]).stdout, json.stdout);
}

/// Q-C: a task executes the first open item among its targets in the
/// listing's order (releases in config order by position, then the pool,
/// then unplaced), whatever order its targets were written in; one
/// targeting only done items or non-item nodes is a pool task, printed
/// after the pool's items for `--pool` and no flag, JSON `pool_tasks`,
/// none under `--release R`.
#[test]
fn pool_tasks_print_after_the_pool_items() {
    let pair = plan_pair("rp-pool");
    let main = pair.main.clone();

    // RM-0050 (unplaced), RM-0041 (pool), RM-0022 (launch 4), RM-0011
    // (beta 5): beta's RM-0011 is the best placed.
    let spread = drafted(&pair, &main, &["RM-0050", "RM-0041", "RM-0022", "RM-0011"]);
    // A non-item document and the section with the prefix.
    let harbor = ready(&pair, &main, &["BAY-HARBOR"]);
    // Done items only: RM-0001 shipped, RM-0002 abandoned, RM-0042 shipped.
    let done_only = drafted(&pair, &main, &["RM-0001", "RM-0002", "RM-0042"]);
    // A done item before an open pool one: the pool item.
    let to_pool_item = drafted(&pair, &main, &["RM-0019", "RM-0041"]);
    // Unplaced only.
    let unplaced = drafted(&pair, &main, &["RM-0050"]);
    assert_eq!(
        [&spread, &harbor, &done_only, &to_pool_item, &unplaced],
        ["T-0001", "T-0002", "T-0003", "T-0004", "T-0005"]
    );

    let cells: &[(&str, &[(&str, &str)])] = &[
        ("RM-0011", &[("T-0001", "draft")]),
        ("RM-0041", &[("T-0004", "draft")]),
        ("RM-0050", &[("T-0005", "draft")]),
    ];
    let pool: &[Pooled<'_>] = &[
        ("T-0002", "ready", &["BAY-HARBOR"]),
        ("T-0003", "draft", &["RM-0001", "RM-0002", "RM-0042"]),
    ];
    let whole = roadmap(&pair, &main, &[]);
    assert_text(&whole, &text_with(cells, pool), "no flag");
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &json_with(cells, pool),
        "no flag",
    );

    // `--pool`: the pool block and the unplaced one of the whole output.
    let run = roadmap(&pair, &main, &["--pool"]);
    run.code(0);
    let first = whole.stdout.lines().next().unwrap();
    let from_pool = &whole.stdout[whole.stdout.find("\npool | ").unwrap() + 1..];
    assert_eq!(run.stdout, format!("{first}\n{from_pool}"), "--pool");
    let json = roadmap_json(&pair, &main, &["--pool"]).json();
    assert_eq!(json["pool_tasks"], pool_values(pool), "--pool JSON");

    // `--release beta`: RM-0011's cell, no pool task.
    let run = roadmap(&pair, &main, &["--release", "beta"]);
    run.code(0);
    assert!(
        run.stdout.contains(&cell(&[("T-0001", "draft")])),
        "{}",
        run.stdout
    );
    assert!(!run.stdout.contains("T-0002"), "{}", run.stdout);
    assert!(!run.stdout.contains("T-0003"), "{}", run.stdout);
    let json = roadmap_json(&pair, &main, &["--release", "beta"]).json();
    assert_eq!(json["pool_tasks"], json!([]), "--release beta JSON");

    // The request filters, never re-places: T-0001 (targets in launch
    // too) is on beta's RM-0011 alone.
    let run = roadmap(&pair, &main, &["--release", "launch"]);
    run.code(0);
    assert!(!run.stdout.contains("T-000"), "{}", run.stdout);

    // Exit 1 (`--release` naming no release): no group, no pool task.
    let run = roadmap_json(&pair, &main, &["--release", "someday"]);
    run.code(1);
    let json = run.json();
    assert_eq!(json["groups"], json!([]));
    assert_eq!(json["pool_tasks"], json!([]));
}

/// The database deleted: text byte-identical to 1a's `expected.txt`, the
/// JSON `expected.json` (the task keys empty), and the git-derived keys of
/// the document with tasks byte-equal to it.
#[test]
fn database_loss_keeps_the_plan() {
    let pair = plan_pair("rp-loss");
    let main = pair.main.clone();
    claimed(&pair, &main, &["RM-0011"]);
    ready(&pair, &main, &["BAY-HARBOR"]);

    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0011", &[("T-0001", "in_progress")])];
    let pool: &[Pooled<'_>] = &[("T-0002", "ready", &["BAY-HARBOR"])];
    let with = roadmap_json(&pair, &main, &[]);
    assert_json(&with, &json_with(cells, pool), "before the loss");
    assert_text(
        &roadmap(&pair, &main, &[]),
        &text_with(cells, pool),
        "before the loss",
    );

    let db = pair.db();
    assert!(db.exists(), "{}", db.display());
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let path = std::path::PathBuf::from(format!("{}{suffix}", db.display()));
        if path.exists() {
            fs::remove_file(&path).expect("remove the database");
        }
    }

    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &expected_text(), "after the loss");
    assert_eq!(notes(&run), plan_notes(), "{}", run.stderr);
    let without = roadmap_json(&pair, &main, &[]);
    assert_json(&without, &expected_json(), "after the loss");
    assert_eq!(
        serde_json::to_string(&git_keys(&with.json())).unwrap(),
        serde_json::to_string(&git_keys(&without.json())).unwrap(),
        "the git-derived keys"
    );
    for args in [&["--release", "beta"][..], &["--pool"][..]] {
        let before = git_keys(&json_with(cells, pool));
        let run = roadmap_json(&pair, &main, args);
        run.code(0);
        let got = run.json();
        assert_eq!(got["pool_tasks"], json!([]), "{args:?}");
        assert_eq!(got["counts"], before["counts"], "{args:?}");
    }
    assert!(
        !tables(&pair.db()).iter().any(|table| table == "tasks"),
        "the read made no queue table"
    );
}

/// The join's note on other repositories' open tasks, `n` of them.
fn others_note(n: usize) -> String {
    format!(
        "{n} task(s) of another repository of the project `{SLUG}` not shown: `spec roadmap` \
         shows the current repository's"
    )
}

/// Other repositories' tasks (same slug, `HOME`, queue): not on the plan,
/// one note counting the open ones; their own repository shows them,
/// noting this one's open ones instead; with none open, no note.
#[test]
fn other_repository_tasks_are_noted() {
    let pair = plan_pair("rp-other");
    let main = pair.main.clone();
    let other = other_repository(&pair, "other");
    let theirs = [
        claimed(&pair, &other, &["RM-0011"]),
        drafted(&pair, &other, &["BAY-HARBOR"]),
        cancelled(&pair, &other, &["RM-0013"]),
    ];
    let ours = [
        drafted(&pair, &main, &["RM-0013"]),
        completed(&pair, &main, &["RM-0017"]),
    ];
    assert_eq!(theirs, ["T-0001", "T-0002", "T-0003"]);
    assert_eq!(ours, ["T-0004", "T-0005"]);

    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0013", &[("T-0004", "draft")])];
    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &text_with(cells, &[]), "this repository");
    assert_eq!(queue_notes(&run), [others_note(2)], "{}", run.stderr);
    let json = roadmap_json(&pair, &main, &[]);
    assert_json(&json, &json_with(cells, &[]), "this repository");
    assert_eq!(queue_notes(&json), [others_note(2)], "--json: stderr");

    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0011", &[("T-0001", "in_progress")])];
    let pool: &[Pooled<'_>] = &[("T-0002", "draft", &["BAY-HARBOR"])];
    let run = roadmap(&pair, &other, &[]);
    assert_text(&run, &text_with(cells, pool), "the other repository");
    assert_eq!(queue_notes(&run), [others_note(1)], "{}", run.stderr);

    // The other repository's tasks closed: no note.
    for id in &theirs[..2] {
        let (outcome, _) = pair.owner(&other, "cancel", id, None, true, CLOCK);
        assert_eq!(outcome.expect("cancel").refusal, None, "cancel {id}");
    }
    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0013", &[("T-0004", "draft")])];
    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &text_with(cells, &[]), "the others closed");
    assert_eq!(queue_notes(&run), Vec::<String>::new(), "{}", run.stderr);
}

/// `done`, `cancelled` and `accepted` tasks (the last set through SQL:
/// this build never enters it) are not listed, on an item or in the pool;
/// an open one beside them is.
#[test]
fn closed_tasks_are_not_listed() {
    let pair = plan_pair("rp-closed");
    let main = pair.main.clone();
    let done = completed(&pair, &main, &["RM-0011"]);
    let gone = cancelled(&pair, &main, &["RM-0013"]);
    let gone_pool = cancelled(&pair, &main, &["BAY-HARBOR"]);
    let worked = claimed(&pair, &main, &["RM-0017"]);
    let accepted = completed(&pair, &main, &["RM-0040"]);
    pair.sql(&format!(
        "UPDATE tasks SET status = 'accepted' WHERE id = '{accepted}';"
    ));
    assert_eq!(stored_status(&pair, &done), "done");
    assert_eq!(stored_status(&pair, &gone), "cancelled");
    assert_eq!(stored_status(&pair, &gone_pool), "cancelled");
    assert_eq!(stored_status(&pair, &accepted), "accepted");
    // Cancel the claimed one too: every task is closed now.
    let (outcome, _) = pair.owner(&main, "cancel", &worked, None, true, CLOCK);
    assert_eq!(
        outcome.expect("cancel").refusal,
        None,
        "cancel the claimed task"
    );

    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &expected_text(), "closed tasks only");
    assert_eq!(notes(&run), plan_notes(), "{}", run.stderr);
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &expected_json(),
        "closed tasks only",
    );

    // An open one beside them is listed.
    let open = ready(&pair, &main, &["RM-0011"]);
    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0011", &[(open.as_str(), "ready")])];
    assert_text(
        &roadmap(&pair, &main, &[]),
        &text_with(cells, &[]),
        "an open task beside the closed",
    );
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &json_with(cells, &[]),
        "an open task beside the closed",
    );
}

/// Outside a git worktree (a plain copy, the same `HOME`, slug and queue
/// holding tasks): no task, no note, 1a's bytes, exit 0.
#[test]
fn no_git_worktree_reads_no_tasks() {
    let pair = plan_pair("rp-no-git");
    let main = pair.main.clone();
    claimed(&pair, &main, &["RM-0011"]);
    ready(&pair, &main, &["BAY-HARBOR"]);
    let loose = pair.scratch.copy(FIXTURE, "loose");
    assert!(!loose.join(".git").exists());
    let probe = pair
        .git
        .git_output(&loose, &["rev-parse", "--git-dir"], &[]);
    assert!(!probe.status.success(), "the copy is in no git worktree");

    let run = roadmap(&pair, &loose, &[]);
    assert_text(&run, &expected_text(), "outside git");
    assert_eq!(notes(&run), plan_notes(), "no note: {}", run.stderr);
    assert_json(
        &roadmap_json(&pair, &loose, &[]),
        &expected_json(),
        "outside git",
    );
    let pool = roadmap(&pair, &loose, &["--pool"]);
    pool.code(0);
    assert!(!pool.stdout.contains("T-000"), "{}", pool.stdout);
}

/// Q-G: no schema step. A fresh `HOME` in a git worktree: `spec roadmap`
/// (text and JSON) answers 1a's bytes with no note, and leaves the
/// database with the index's tables alone (as `spec tree` leaves them in
/// another fresh `HOME`): no queue table, `user_version` 0.
#[test]
fn roadmap_creates_no_queue_table() {
    let pair = plan_pair("rp-no-table");
    let main = pair.main.clone();

    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &expected_text(), "a fresh HOME");
    assert_eq!(notes(&run), plan_notes(), "no note: {}", run.stderr);
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &expected_json(),
        "a fresh HOME",
    );
    let after_roadmap = tables(&pair.db());
    assert_eq!(user_version(&pair), "0", "no queue schema step");
    for queue in ["tasks", "runs", "proposals", "events"] {
        assert!(
            !after_roadmap.iter().any(|table| table == queue),
            "{queue} created: {after_roadmap:?}"
        );
    }

    // The index's tables alone: those `spec tree` leaves.
    let tree_home = pair.scratch.home("tree");
    let mut command = std::process::Command::new(common::SPEC);
    let tree = command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", &tree_home)
        .current_dir(&main)
        .args(["tree"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("spec tree");
    assert!(tree.status.success(), "{tree:?}");
    let tree_db = common::data_dir(&tree_home).join(format!("{SLUG}.db"));
    assert_eq!(after_roadmap, tables(&tree_db), "the index's tables alone");

    // With the queue made by a task command and every task closed, the
    // read still answers 1a's bytes.
    let gone = cancelled(&pair, &main, &["RM-0011"]);
    assert_eq!(gone, "T-0001");
    assert_text(
        &roadmap(&pair, &main, &[]),
        &expected_text(),
        "a closed queue",
    );
}

/// An unreadable queue (a newer schema; a row that does not decode; `T`
/// or `PR` in `[ids]`): exit 0, the plan's lines, one note: `tasks not
/// shown: <why>` alone, or the corrupt row's `<row>; not shown` beside the
/// rows that read.
#[test]
fn unreadable_queue_is_one_note() {
    // A newer schema.
    let pair = plan_pair("rp-unreadable");
    let main = pair.main.clone();
    claimed(&pair, &main, &["RM-0011"]);
    ready(&pair, &main, &["BAY-HARBOR"]);
    let version = user_version(&pair);
    pair.sql("PRAGMA user_version = 99;");
    for args in [&[][..], &["--pool"][..], &["--release", "beta"][..]] {
        let run = roadmap(&pair, &main, args);
        run.code(0);
        let noted = queue_notes(&run);
        assert_eq!(noted.len(), 1, "newer schema {args:?}\n{}", run.stderr);
        assert!(noted[0].starts_with("tasks not shown: "), "{}", noted[0]);
        assert!(!run.stdout.contains("T-000"), "{}", run.stdout);
    }
    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &expected_text(), "newer schema");
    let json = roadmap_json(&pair, &main, &[]);
    assert_json(&json, &expected_json(), "newer schema");
    assert_eq!(user_version(&pair), "99", "nothing written");
    pair.sql(&format!("PRAGMA user_version = {version};"));

    // A row that does not decode: named, skipped; the other row joins.
    pair.sql("UPDATE tasks SET status = 'flying' WHERE id = 'T-0002';");
    let cells: &[(&str, &[(&str, &str)])] = &[("RM-0011", &[("T-0001", "in_progress")])];
    let run = roadmap(&pair, &main, &[]);
    assert_text(&run, &text_with(cells, &[]), "a corrupt row");
    let noted = queue_notes(&run);
    assert_eq!(noted.len(), 1, "a corrupt row\n{}", run.stderr);
    assert!(
        noted[0].contains("T-0002") && noted[0].ends_with("; not shown"),
        "the note names the row: {}",
        noted[0]
    );
    assert_json(
        &roadmap_json(&pair, &main, &[]),
        &json_with(cells, &[]),
        "a corrupt row",
    );
    pair.sql("UPDATE tasks SET status = 'ready' WHERE id = 'T-0002';");

    // `T`, then `PR`, in `[ids]` (the working tree's config).
    let config = fs::read_to_string(main.join("specengine.toml")).unwrap();
    for prefix in ["T", "PR"] {
        let with = config.replacen(
            "BAY = { kind = \"harbor-zone\", shape = \"name\" }\n",
            &format!(
                "BAY = {{ kind = \"harbor-zone\", shape = \"name\" }}\n{prefix} = {{ kind = \"tally-sheet\", width = 4 }}\n"
            ),
            1,
        );
        assert_ne!(with, config);
        write(&main, "specengine.toml", &with);
        let run = roadmap(&pair, &main, &[]);
        assert_text(&run, &expected_text(), &format!("{prefix} in [ids]"));
        let noted = queue_notes(&run);
        assert_eq!(noted.len(), 1, "{prefix} in [ids]\n{}", run.stderr);
        assert!(
            noted[0].starts_with("tasks not shown: specengine.toml"),
            "{}",
            noted[0]
        );
    }
    write(&main, "specengine.toml", &config);
}

// ------------------------------------------------------------------- RD-3

/// The generated corpus of the cut (as 1a's `bulk_corpus`): releases
/// `bulk` (500 open items, long titles) and `spare` (3), 2 pool items;
/// every item `high`, `1-2`; plus `plan/NT-0001.md`, a non-item node of
/// its own prefix (the pool tasks' target).
fn bulk_corpus(root: &Path) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"bulk-plan\"\n\n[paths]\nroots = [\"plan\"]\n\n[ids]\nBK = { kind = \"bulk-item\", width = 4 }\nNT = { kind = \"note-page\", width = 4 }\n\n[roadmap]\nprefix = \"BK\"\ndir = \"plan\"\nreleases = [\"bulk\", \"spare\"]\npriorities = [\"high\", \"low\"]\nunit = \"hours\"\n",
    );
    for n in 1..=505 {
        let release = match n {
            1..=500 => "release: bulk\n",
            501..=503 => "release: spare\n",
            _ => "",
        };
        write(
            root,
            &format!("plan/BK-{n:04}.md"),
            format!(
                "---\nclass: spec\nid: BK-{n:04}\nstatus: draft\nscope: [bulk]\n{release}priority: high\nestimate: 1-2\n---\n\n# {}\n\nGoal.\n",
                bulk_title(n)
            ),
        );
    }
    write(
        root,
        "plan/NT-0001.md",
        "---\nclass: spec\nid: NT-0001\nstatus: draft\nscope: [bulk]\n---\n\n# Notes\n\nNot a plan item.\n",
    );
}

fn bulk_title(n: usize) -> String {
    format!("Bulk item {n:04} with a long title that fills the line of the plan view")
}

/// The text line item `n` of `bulk` prints as, without a task cell.
fn bulk_line(n: usize) -> String {
    format!(
        "  {n}. BK-{n:04} | {} | draft | high | 1-2 | plan/BK-{n:04}.md",
        bulk_title(n)
    )
}

/// The printed part of a cut output and its tail `(k, n)`.
#[track_caller]
fn cut_parts(stdout: &str) -> (&str, usize, usize) {
    let body = stdout.strip_suffix('\n').expect("a line end");
    let (before, tail) = body.rsplit_once('\n').expect("a tail line");
    let rest = tail
        .strip_prefix("[truncated: ")
        .unwrap_or_else(|| panic!("the tail: {tail:?}"));
    let (k, rest) = rest.split_once(" of ").expect("`k of n`");
    let (n, rest) = rest.split_once(' ').expect("`n items`");
    assert_eq!(
        rest, "items not shown; add --release R or --pool]",
        "{tail}"
    );
    let before = &stdout[..before.len() + 1];
    (before, k.parse().unwrap(), n.parse().unwrap())
}

/// M4: 505 items and 3 pool tasks; BK-0001 carries a task cell. With no
/// flag the tail's `n` is 508 (the items listed plus the pool tasks) and
/// `k` = `n` - the lines shown; everything before it at most 40 000
/// characters, the cell counted, the next line passing the cap; JSON
/// `omitted` = `k`, `pool_tasks` empty (the pool's header never prints).
/// `--release bulk` lists no pool task (`n` 500); `--pool` prints its 2
/// items and 3 tasks uncut.
#[test]
fn roadmap_cut_counts_task_lines() {
    let scratch = Scratch::new("rp-cut");
    let git = Sandbox::new(scratch.path());
    let main = scratch.dir("bulk");
    bulk_corpus(&main);
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the bulk plan");
    let home = scratch.home("h");
    let pair = Pair {
        scratch,
        git,
        main: main.clone(),
        linked: main.clone(),
        home,
        slug: "bulk-plan".to_owned(),
    };
    let on_first = claimed(&pair, &main, &["BK-0001"]);
    let pool: Vec<String> = (0..3)
        .map(|_| drafted(&pair, &main, &["NT-0001"]))
        .collect();
    assert_eq!(on_first, "T-0001");
    assert_eq!(pool, ["T-0002", "T-0003", "T-0004"]);
    let first_line = format!("{}{}", bulk_line(1), cell(&[("T-0001", "in_progress")]));

    for (request, listed) in [(&[][..], 508), (&["--release", "bulk"][..], 500)] {
        let run = roadmap(&pair, &main, request);
        run.code(0);
        let (before, k, n) = cut_parts(&run.stdout);
        assert_eq!(n, listed, "{request:?}: n counts the pool tasks");
        assert!(
            before.chars().count() <= CAP,
            "{request:?}: {} characters before the tail",
            before.chars().count()
        );
        let shown: Vec<&str> = before
            .lines()
            .filter(|line| line.starts_with("  "))
            .collect();
        assert!(k > 0, "{request:?}: cut");
        assert_eq!(k + shown.len(), listed, "{request:?}: k + shown");
        assert_eq!(shown[0], first_line, "{request:?}: the cell, counted");
        for (at, line) in shown.iter().enumerate().skip(1) {
            assert_eq!(*line, bulk_line(at + 1), "{request:?}: whole lines");
        }
        let next = bulk_line(shown.len() + 1);
        assert!(
            before.chars().count() + next.chars().count() + 1 > CAP,
            "{request:?}: the next line would fit"
        );

        let json = roadmap_json(&pair, &main, request).json();
        assert_eq!(json["truncated"], true, "{request:?}");
        assert_eq!(json["omitted"], k as u64, "{request:?}");
        assert_eq!(json["pool_tasks"], json!([]), "{request:?}: none printed");
        let items = json["groups"][0]["items"].as_array().unwrap();
        assert_eq!(items.len(), shown.len(), "{request:?}");
        assert_eq!(
            items[0]["tasks"],
            task_values(&[("T-0001", "in_progress")]),
            "{request:?}"
        );
    }

    let run = roadmap(&pair, &main, &["--pool"]);
    run.code(0);
    assert!(!run.stdout.contains("[truncated: "), "{}", run.stdout);
    let pool_lines: Vec<String> = pool
        .iter()
        .map(|id| pool_line((id, "draft", &["NT-0001"])))
        .collect();
    let lines: Vec<&str> = run.stdout.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("pool | "))
        .expect("the pool header");
    assert_eq!(
        lines[at + 3..at + 6],
        pool_lines.iter().map(String::as_str).collect::<Vec<_>>()[..],
        "after the pool's two items\n{}",
        run.stdout
    );
    let json = roadmap_json(&pair, &main, &["--pool"]).json();
    assert_eq!(json["truncated"], false);
    assert_eq!(json["omitted"], 0);
    assert_eq!(
        json["pool_tasks"],
        pool_values(&[
            ("T-0002", "draft", &["NT-0001"]),
            ("T-0003", "draft", &["NT-0001"]),
            ("T-0004", "draft", &["NT-0001"]),
        ])
    );
}

/// A corpus of `count` pool items (no release has one; `bulk` is
/// empty), long titles, and `plan/NT-0001.md`, a non-item node.
fn pool_corpus(root: &Path, count: usize) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"pool-plan\"\n\n[paths]\nroots = [\"plan\"]\n\n[ids]\nBK = { kind = \"bulk-item\", width = 4 }\nNT = { kind = \"note-page\", width = 4 }\n\n[roadmap]\nprefix = \"BK\"\ndir = \"plan\"\nreleases = [\"bulk\"]\npriorities = [\"high\", \"low\"]\nunit = \"hours\"\n",
    );
    for n in 1..=count {
        write(
            root,
            &format!("plan/BK-{n:04}.md"),
            format!(
                "---\nclass: spec\nid: BK-{n:04}\nstatus: draft\nscope: [bulk]\npriority: high\nestimate: 1-2\n---\n\n# {}\n\nGoal.\n",
                bulk_title(n)
            ),
        );
    }
    write(
        root,
        "plan/NT-0001.md",
        "---\nclass: spec\nid: NT-0001\nstatus: draft\nscope: [bulk]\n---\n\n# Notes\n\nNot a plan item.\n",
    );
}

/// RD-3, the characters: `--pool` over 320 pool items that nearly fill
/// the cap, then five pool tasks whose titles are sized (from the
/// measured room) so that two lines fit: the two print whole after the
/// items, the third would pass the cap, the unplaced header does not
/// print, the tail counts `k` 3 of `n` 325; JSON `pool_tasks` the two
/// printed, `omitted` 3, the pool group alone.
#[test]
fn pool_task_lines_fill_the_cap() {
    const ITEMS: usize = 320;
    const TASKS: usize = 5;
    let scratch = Scratch::new("rp-pool-cap");
    let git = Sandbox::new(scratch.path());
    let main = scratch.dir("pool");
    pool_corpus(&main, ITEMS);
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the pool plan");
    let home = scratch.home("h");
    let pair = Pair {
        scratch,
        git,
        main: main.clone(),
        linked: main.clone(),
        home,
        slug: "pool-plan".to_owned(),
    };

    // The room left after the pool's items, with no task.
    let bare = roadmap(&pair, &main, &["--pool"]);
    bare.code(0);
    assert!(!bare.stdout.contains("[truncated: "), "{}", bare.stdout);
    let lines: Vec<&str> = bare.stdout.lines().collect();
    assert_eq!(lines.len(), ITEMS + 3, "first, pool, items, unplaced");
    assert!(
        lines[ITEMS + 2].starts_with("unplaced | "),
        "{}",
        lines[ITEMS + 2]
    );
    let used: usize = lines[..ITEMS + 2]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    let room = CAP - used;
    // A task line of `width` characters plus its line end: two fit, three
    // do not.
    let width = room * 2 / 5 - 1;
    let fixed = pool_line(("T-0001", "draft", &["NT-0001"])).chars().count() - TITLE.len();
    let title_len = width - fixed;
    assert!(
        (11..=256).contains(&title_len),
        "room {room}: a title of {title_len}"
    );
    let title = format!("Pool task {}", "x".repeat(title_len - 10));
    assert_eq!(title.chars().count(), title_len);
    let ids: Vec<String> = (0..TASKS)
        .map(|_| {
            let made = pair
                .new_task(&main, &["NT-0001"], Some(&title), Some("Make it so."))
                .expect("task new");
            made.id.expect("an ID")
        })
        .collect();
    assert_eq!(ids[0], "T-0001");
    let line_of = |id: &str| format!("  - {id} | draft | {title} | NT-0001");
    assert_eq!(line_of("T-0001").chars().count(), width);
    assert!(2 * (width + 1) <= room && 3 * (width + 1) > room);

    let run = roadmap(&pair, &main, &["--pool"]);
    run.code(0);
    let (before, k, n) = cut_parts(&run.stdout);
    assert_eq!(n, ITEMS + TASKS, "n: the pool's items and tasks");
    assert_eq!(k, TASKS - 2, "k: the tasks not shown");
    assert!(before.chars().count() <= CAP, "{}", before.chars().count());
    let printed: Vec<&str> = before.lines().collect();
    assert_eq!(printed.len(), ITEMS + 4, "first, pool, items, two tasks");
    assert_eq!(
        printed[..ITEMS + 2],
        lines[..ITEMS + 2],
        "the items as before"
    );
    assert_eq!(printed[ITEMS + 2], line_of("T-0001"));
    assert_eq!(printed[ITEMS + 3], line_of("T-0002"));
    assert!(
        before.chars().count() + line_of("T-0003").chars().count() + 1 > CAP,
        "the next task line would fit"
    );

    let json = roadmap_json(&pair, &main, &["--pool"]).json();
    assert_eq!(json["truncated"], true);
    assert_eq!(json["omitted"], (TASKS - 2) as u64);
    let groups = json["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "the pool group alone");
    assert_eq!(groups[0]["group"], "pool");
    assert_eq!(groups[0]["items"].as_array().unwrap().len(), ITEMS);
    let want: Vec<Value> = ["T-0001", "T-0002"]
        .iter()
        .map(|id| json!({"id": id, "status": "draft", "title": title, "targets": ["NT-0001"]}))
        .collect();
    assert_eq!(json["pool_tasks"], Value::Array(want));

    // No flag: `bulk`'s empty header first, the tasks still counted.
    let whole = roadmap(&pair, &main, &[]);
    whole.code(0);
    let (before, k, n) = cut_parts(&whole.stdout);
    assert_eq!(n, ITEMS + TASKS, "{}", whole.stdout);
    let shown = before.lines().filter(|line| line.starts_with("  ")).count();
    assert_eq!(k + shown, n, "k + shown");
    assert!(k >= TASKS - 2, "{k}");
    assert!(before.chars().count() <= CAP);
}

/// A pool task's agent-written title (a line break, an ESC sequence, a
/// BEL, a bidi override, Greek) prints on one line with every control
/// escaped (`\u{XX}`), the owner's terminal untouched; the JSON keeps it
/// verbatim.
#[test]
fn pool_task_title_prints_on_one_line_escaped() {
    let pair = plan_pair("rp-escaped");
    let main = pair.main.clone();
    let title = "Tide\nbell \u{1b}[2J\u{7} \u{202e}\u{3a9}\u{3bc}\u{3ad}\u{3b3}\u{3b1}";
    let made = pair
        .new_task(&main, &["BAY-HARBOR"], Some(title), Some("Make it so."))
        .expect("task new");
    assert_eq!(made.id.as_deref(), Some("T-0001"), "{made:?}");

    let run = roadmap(&pair, &main, &["--pool"]);
    run.code(0);
    common::assert_plain(&run.stdout, "the pool task's title");
    assert!(
        !run.stdout.chars().any(|c| c.is_control() && c != '\n'),
        "a raw control character: {:?}",
        run.stdout
    );
    let want = "  - T-0001 | draft | Tide bell \\u{1b}[2J\\u{7} \\u{202e}\u{3a9}\u{3bc}\u{3ad}\u{3b3}\u{3b1} | BAY-HARBOR";
    assert!(
        run.stdout.lines().any(|line| line == want),
        "want {want:?} in\n{}",
        run.stdout
    );
    let json = roadmap_json(&pair, &main, &["--pool"]).json();
    assert_eq!(json["pool_tasks"][0]["title"], title, "verbatim in JSON");
}

/// Git failing at the project root (a `git` first on `PATH` that answers
/// every command with an error that is not "not a git repository"): with
/// no database, then an index-only one, git is not run (no note); with a
/// task row, the plan's lines, exit 0, one note `tasks not shown: …`; no
/// task.
#[test]
fn failing_git_is_one_note() {
    use std::os::unix::fs::PermissionsExt as _;

    let pair = plan_pair("rp-git-fails");
    let main = pair.main.clone();
    let bin = pair.scratch.dir("failing-bin");
    let script = bin.join("git");
    fs::write(
        &script,
        "#!/bin/sh\necho 'error: the probe git fails' >&2\nexit 3\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let path = pair.git.path_with(&bin);
    let mut env: Vec<(String, std::ffi::OsString)> = pair
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "PATH" && name != "HOME")
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .collect();
    env.push(("PATH".to_owned(), path));
    env.push(("HOME".to_owned(), pair.home.clone().into_os_string()));
    let pairs: Vec<(&str, &std::ffi::OsStr)> = env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();

    for state in ["no database", "an index-only database"] {
        let run = common::spec_with(&main, &["roadmap"], &pairs);
        assert_text(&run, &expected_text(), state);
        assert_eq!(notes(&run), plan_notes(), "{state}: git not run");
    }
    claimed(&pair, &main, &["RM-0011"]);

    let run = common::spec_with(&main, &["roadmap"], &pairs);
    assert_text(&run, &expected_text(), "git failing");
    let noted = queue_notes(&run);
    assert_eq!(noted.len(), 1, "one note\n{}", run.stderr);
    assert!(
        noted[0].starts_with("tasks not shown: ") && noted[0].contains("git"),
        "{}",
        noted[0]
    );
    let json = common::spec_with(&main, &["--json", "roadmap"], &pairs);
    assert_json(&json, &expected_json(), "git failing");
}
