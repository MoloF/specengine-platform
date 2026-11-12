//! docs/features/department-overview.md (RM-0086) "Acceptance criteria",
//! the CLI half: the `spec` binary over the spec's scenario (the shared
//! scene `tests/counts_scene`, a scratch git repository with its queue
//! written through the library at fixed clocks), and over variants of it.
//!
//! - DO1: 700 cards with 120-character titles cut `spec card list --json`
//!   (`truncated` true); `spec card counts --json` still sums `open` +
//!   `accepted` + `cancelled` to 700, uncut, in text too. M: the counts
//!   taken from `card_list`'s rows.
//! - DO5: two runs over one queue are byte-identical, text and JSON, at
//!   another root, another `HOME` and another `TZ` too; no key holds an
//!   age; the read writes nothing (the data directory's files byte for
//!   byte), and no queue yet makes none.
//! - DO9: the spec's text and JSON byte for byte (`flows` byte-equal to
//!   `spec card list --json`'s); a flow no `[card]` lists prints its `not
//!   configured` line, its columns `not in the flow`; no `[card]` table:
//!   the Board's default flow (the orchestrator's ruling), stored columns
//!   of other flows stranded, the default flow's own cards in its columns;
//!   no queue: zeros, exit 0; a bad `[card]`: exit 2 with its line; a usage
//!   error exit 2; a card whose `tasks` is `"x"`: its note on stderr and in
//!   `notes`, still counted, exit 0. M: a stranded column dropped;
//!   `open_subtasks` counting accepted children; an item on a cancelled
//!   task left out.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run of the read under test.

#![cfg(unix)]

mod common;
mod counts_scene;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::git;
use common::{Run, spec, spec_with};
use counts_scene::{
    COPY, DATA_TEXT, DEPARTMENTS, HEAD, PLATES, SOUND, Scene, compact, db_under, new_card,
    with_flows,
};
use serde_json::{Value, json};
use specengine_cli::CardNewRequest;

/// `spec card counts args…` in the scene's root, under its `HOME`.
fn counts(scene: &Scene, args: &[&str]) -> Run {
    let mut all = vec!["card", "counts"];
    all.extend_from_slice(args);
    spec(&scene.home, &scene.root, &all)
}

/// `spec card list --json` in the scene's root: its raw stdout.
fn list_raw(scene: &Scene) -> String {
    let run = spec(&scene.home, &scene.root, &["card", "list", "--json"]);
    run.code(0);
    run.stdout
}

/// The raw text of the top-level `key`'s value in the one-line JSON
/// `document`, as printed (strings skipped while the brackets are matched).
fn raw_value(document: &str, key: &str) -> String {
    let needle = format!("\"{key}\":");
    let bytes = document.as_bytes();
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            at += 1;
            continue;
        }
        if depth == 1 && document[at..].starts_with(&needle) {
            let start = at + needle.len();
            let (mut inner, mut in_str, mut esc) = (0usize, false, false);
            for (offset, letter) in document[start..].char_indices() {
                if in_str {
                    if esc {
                        esc = false;
                    } else if letter == '\\' {
                        esc = true;
                    } else if letter == '"' {
                        in_str = false;
                    }
                    continue;
                }
                match letter {
                    '"' => in_str = true,
                    '[' | '{' => inner += 1,
                    ']' | '}' => {
                        if inner == 0 {
                            return document[start..start + offset].to_owned();
                        }
                        inner -= 1;
                        if inner == 0 {
                            return document[start..=start + offset].to_owned();
                        }
                    }
                    ',' if inner == 0 => return document[start..start + offset].to_owned(),
                    _ => {}
                }
            }
            panic!("`{key}` runs to the end: {document}");
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        at += 1;
    }
    panic!("no top-level `{key}` in {document}");
}

/// A column row of the JSON, all four counts.
fn column(flow: &str, name: &str, counts: [u64; 4]) -> Value {
    json!({"flow": flow, "column": name, "open": counts[0], "accepted": counts[1],
        "cancelled": counts[2], "open_subtasks": counts[3]})
}

/// A flow row of the JSON.
fn by_flow(
    flow: &str,
    items: u64,
    oldest: Option<&str>,
    ready: u64,
    review: Option<&[&str]>,
) -> Value {
    json!({"flow": flow, "open_items": items, "oldest_open_item": oldest,
        "ready_parents": ready, "review_agent_tasks": review})
}

/// The scenario's `columns` and `by_flow`, as "Data" pins them.
fn scenario_columns() -> Vec<Value> {
    vec![
        column("copy", "todo", [0, 0, 1, 0]),
        column("copy", "doing", [0, 0, 0, 0]),
        column("copy", "review", [1, 0, 0, 0]),
        column("copy", "returned", [1, 0, 0, 1]),
        column("copy", "done", [0, 0, 0, 0]),
        column("copy", "proofing", [1, 0, 0, 0]),
        column("plates", "todo", [1, 0, 0, 0]),
        column("plates", "drawing", [0, 0, 0, 0]),
        column("plates", "done", [0, 1, 0, 0]),
        column("sound", "todo", [1, 0, 0, 0]),
        column("sound", "check", [0, 0, 0, 0]),
        column("sound", "done", [0, 0, 0, 0]),
    ]
}

/// Every key of `value`, at any depth.
fn all_keys(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                out.push(key.clone());
                all_keys(inner, out);
            }
        }
        Value::Array(items) => {
            for inner in items {
                all_keys(inner, out);
            }
        }
        _ => {}
    }
}

/// Every file under `dir` with its bytes.
fn files_under(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let bytes = fs::read(&path).expect("a file");
                out.insert(
                    path.strip_prefix(dir)
                        .expect("under")
                        .to_string_lossy()
                        .into_owned(),
                    bytes,
                );
            }
        }
    }
    out
}

// ------------------------------------------------------------------ DO9

/// DO9: `spec card counts` prints "Text"'s lines byte for byte, no note,
/// exit 0; `--json` prints "JSON"'s document byte for byte, its `flows`
/// byte-equal to `spec card list --json`'s. M: a stranded column dropped
/// (`copy/proofing`); `open_subtasks` counting accepted children
/// (`plates/done` would read 1, C-0002 under C-0001); an item on a
/// cancelled task left out (`copy`'s `open_items` would read 2).
#[test]
fn do9_the_scenario_prints_the_spec_byte_for_byte() {
    let scene = Scene::scenario("do9-pinned");
    let text = counts(&scene, &[]);
    text.code(0);
    assert_eq!(text.stdout, DATA_TEXT, "{}", text.show());
    assert_eq!(text.stderr, "", "no note");
    let json = counts(&scene, &["--json"]);
    json.code(0);
    assert_eq!(json.stderr, "", "no note");
    let list = list_raw(&scene);
    let flows = raw_value(list.trim_end(), "flows");
    assert_eq!(
        raw_value(json.stdout.trim_end(), "flows"),
        flows,
        "`flows` byte-equal to `spec card list --json`'s"
    );
    assert_eq!(
        json.stdout,
        format!("{}\n", with_flows(&flows)),
        "{}",
        json.show()
    );
    // The flows are the three configured, in config order, each as the
    // list serves `CardFlow`.
    let document = json.json();
    let names: Vec<&str> = document["flows"]
        .as_array()
        .expect("flows")
        .iter()
        .map(|flow| flow["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(names, ["copy", "plates", "sound"]);
    assert_eq!(document["columns"], json!(scenario_columns()));
}

/// DO9: a flow no `[card]` lists (`sound`, dropped from the config once
/// C-0006 stands in it) prints `flow sound | - | … | not configured`, after
/// the configured flows, and only its stored column, `not in the flow`;
/// the header counts the two configured flows. JSON: no `flows` entry,
/// its stored column and a zero `by_flow` row. M: a stranded column
/// dropped.
#[test]
fn do9_a_flow_no_card_lists_prints_not_configured() {
    let scene = Scene::scenario("do9-unlisted");
    scene.set_config(&format!("{HEAD}{COPY}{PLATES}{DEPARTMENTS}"));
    let text = counts(&scene, &[]);
    text.code(0);
    let want = DATA_TEXT
        .replace(
            "card counts | flows 3 |",
            "card counts | flows 2 |",
        )
        .replace(
            "flow sound | Sound | open items 0, oldest - | ready parents 0 | agent tasks in review -\n\
             \x20 sound/todo | open 1, accepted 0, cancelled 0, open subtasks 0\n\
             \x20 sound/check | open 0, accepted 0, cancelled 0, open subtasks 0\n\
             \x20 sound/done | open 0, accepted 0, cancelled 0, open subtasks 0\n",
            "flow sound | - | open items 0, oldest - | ready parents 0 | agent tasks in review - \
             | not configured\n\
             \x20 sound/todo | open 1, accepted 0, cancelled 0, open subtasks 0 | not in the flow\n",
        );
    assert_ne!(want, DATA_TEXT, "the premise: both lines replaced");
    assert_eq!(text.stdout, want, "{}", text.show());
    assert_eq!(text.stderr, "");
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    let names: Vec<&str> = document["flows"]
        .as_array()
        .expect("flows")
        .iter()
        .map(|flow| flow["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(names, ["copy", "plates"]);
    let mut columns = scenario_columns();
    columns.truncate(9);
    columns.push(column("sound", "todo", [1, 0, 0, 0]));
    assert_eq!(document["columns"], json!(columns));
    assert_eq!(
        document["by_flow"][2],
        by_flow("sound", 0, None, 0, Some(&[]))
    );
    assert_eq!(document["by_flow"].as_array().map(Vec::len), Some(3));
}

/// DO9, as the orchestrator ruled on 2026-11-12 (D1, agreement with the
/// Board by construction; the spec's "Exit" said `flows` empty): no
/// `[card]` table: `flows` is `spec card list --json`'s, byte for byte (core's
/// default `plain` flow), `plain`'s columns at 0 in its order, then every
/// stored column of the flows `[card]` no longer lists, by flow and column
/// name, each flow `not configured`, each column `not in the flow`; exit 0.
#[test]
fn do9_no_card_table_counts_under_the_boards_default_flow() {
    let scene = Scene::scenario("do9-no-card");
    scene.set_config(HEAD);
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let list = list_raw(&scene);
    let flows = raw_value(list.trim_end(), "flows");
    assert_eq!(
        raw_value(json.stdout.trim_end(), "flows"),
        flows,
        "`flows` byte-equal to `spec card list --json`'s"
    );
    let document = json.json();
    let listed: Value = serde_json::from_str(&flows).expect("the flows");
    assert_eq!(listed.as_array().map(Vec::len), Some(1), "one default flow");
    let plain = listed[0]["name"].as_str().expect("a name").to_owned();
    let mut want: Vec<Value> = listed[0]["columns"]
        .as_array()
        .expect("its columns")
        .iter()
        .map(|entry| column(&plain, entry["id"].as_str().expect("an id"), [0, 0, 0, 0]))
        .collect();
    want.extend([
        column("copy", "proofing", [1, 0, 0, 0]),
        column("copy", "returned", [1, 0, 0, 1]),
        column("copy", "review", [1, 0, 0, 0]),
        column("copy", "todo", [0, 0, 1, 0]),
        column("plates", "done", [0, 1, 0, 0]),
        column("plates", "todo", [1, 0, 0, 0]),
        column("sound", "todo", [1, 0, 0, 0]),
    ]);
    assert_eq!(document["columns"], json!(want), "{}", json.show());
    let flows_named: Vec<&str> = document["by_flow"]
        .as_array()
        .expect("by_flow")
        .iter()
        .map(|row| row["flow"].as_str().expect("a flow"))
        .collect();
    assert_eq!(flows_named, [plain.as_str(), "copy", "plates", "sound"]);
    let text = counts(&scene, &[]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    assert_eq!(
        lines[0],
        "card counts | flows 1 | cards 7: open 5, accepted 1, cancelled 1 | agent tasks counted",
        "{}",
        text.show()
    );
    assert!(
        lines[1].starts_with(&format!("flow {plain} | ")) && !lines[1].ends_with("not configured"),
        "{}",
        text.stdout
    );
    let copy = lines
        .iter()
        .position(|line| line.starts_with("flow copy | "))
        .expect("copy's line");
    assert_eq!(
        lines[copy],
        "flow copy | - | open items 3, oldest 2026-11-02T09:00:00Z | ready parents 1 | agent \
         tasks in review T-0001 | not configured"
    );
    assert!(
        lines[copy..]
            .iter()
            .filter(|line| line.starts_with("  "))
            .all(|line| line.ends_with(" | not in the flow")),
        "{}",
        text.stdout
    );
    assert!(
        lines[2..copy]
            .iter()
            .all(|line| !line.ends_with(" | not in the flow")),
        "{}",
        text.stdout
    );
    assert_eq!(lines.len(), 1 + 4 + 5 + 7, "{}", text.stdout);
}

/// DO9, the ruling's other half: a project that never had a `[card]` table
/// stores its cards under the default flow, and they count in its columns,
/// none stranded.
#[test]
fn do9_cards_of_the_default_flow_count_in_its_columns() {
    let scene = Scene::with_config("do9-plain", HEAD);
    let first = scene.card(&CardNewRequest {
        title: "Bind the atlas".to_owned(),
        now: counts_scene::at(1),
        ..CardNewRequest::default()
    });
    scene.card(&CardNewRequest {
        title: "Gild the spine".to_owned(),
        parent: Some(first.clone()),
        now: counts_scene::at(1),
        ..CardNewRequest::default()
    });
    let list = list_raw(&scene);
    let listed: Value = serde_json::from_str(&raw_value(list.trim_end(), "flows")).expect("flows");
    let plain = listed[0]["name"].as_str().expect("a name").to_owned();
    let first_column = listed[0]["columns"][0]["id"]
        .as_str()
        .expect("a column")
        .to_owned();
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    let rows = document["columns"].as_array().expect("columns");
    assert_eq!(
        rows.len(),
        listed[0]["columns"].as_array().map_or(0, Vec::len),
        "no stranded column: {}",
        json.show()
    );
    assert_eq!(rows[0], column(&plain, &first_column, [2, 0, 0, 1]));
    assert_eq!(document["by_flow"].as_array().map(Vec::len), Some(1));
    let text = counts(&scene, &[]);
    text.code(0);
    assert!(!text.stdout.contains("not configured"), "{}", text.stdout);
    assert!(!text.stdout.contains("not in the flow"), "{}", text.stdout);
}

/// DO9: no queue yet: every configured column at 0, every flow a zero
/// row, exit 0, in text and JSON; no queue made.
#[test]
fn do9_no_queue_answers_zeros() {
    let scene = Scene::new("do9-no-queue");
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    let columns: Vec<Value> = scenario_columns()
        .into_iter()
        .filter(|row| row["column"] != "proofing")
        .map(|mut row| {
            for key in ["open", "accepted", "cancelled", "open_subtasks"] {
                row[key] = json!(0);
            }
            row
        })
        .collect();
    assert_eq!(document["columns"], json!(columns), "{}", json.show());
    assert_eq!(
        document["by_flow"],
        json!([
            by_flow("copy", 0, None, 0, Some(&[])),
            by_flow("plates", 0, None, 0, Some(&[])),
            by_flow("sound", 0, None, 0, Some(&[])),
        ])
    );
    assert_eq!(document["agent_tasks"], "counted");
    assert_eq!(document["truncated"], false);
    assert_eq!(document["omitted"], Value::Null);
    let text = counts(&scene, &[]);
    text.code(0);
    assert_eq!(
        text.stdout.lines().next(),
        Some(
            "card counts | flows 3 | cards 0: open 0, accepted 0, cancelled 0 | agent tasks counted"
        ),
        "{}",
        text.show()
    );
    assert_eq!(text.stdout.lines().count(), 1 + 3 + 11);
    assert!(
        !db_under(&scene.home).exists(),
        "a read makes no queue: {}",
        db_under(&scene.home).display()
    );
}

/// DO9: a bad `[card]` exits 2 with `<config>:<line>: <message>`, stdout
/// empty, text and JSON alike, as every `spec card` command; a usage error
/// (a flag the command has not, a stray argument) exits 2.
#[test]
fn do9_a_bad_card_table_exits_2_with_its_line() {
    let scene = Scene::scenario("do9-bad-card");
    let bad = format!(
        "{HEAD}{COPY}{PLATES}{SOUND}\n[[card.flow]]\nname = \"bad\"\ncolumns = [\"todo\"]\n"
    );
    // The bad flow's lines: its table, `name` and `columns`.
    let last = bad.lines().count();
    let first_line = last - 2;
    scene.set_config(&bad);
    let list = spec(&scene.home, &scene.root, &["card", "list"]);
    list.code(2);
    let listed = list.stderr.lines().next().unwrap_or_default().to_owned();
    assert!(
        listed.starts_with("specengine.toml:"),
        "the list's line: {}",
        list.show()
    );
    for args in [&[][..], &["--json"][..]] {
        let run = counts(&scene, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: {}", run.show());
        let first = run.stderr.lines().next().unwrap_or_default();
        assert!(
            first.starts_with("specengine.toml:"),
            "{args:?}: {}",
            run.show()
        );
        let at: usize = first
            .split(':')
            .nth(1)
            .and_then(|number| number.parse().ok())
            .unwrap_or_else(|| panic!("a line number: {first}"));
        assert!(
            (first_line..=last).contains(&at),
            "{args:?}: line {at} of the bad flow ({first_line}..={last}): {first}"
        );
        assert_eq!(first, listed, "{args:?}: as `spec card list` says it");
    }
    for args in [&["--flow", "copy"][..], &["extra"][..]] {
        let run = counts(&scene, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
    }
}

/// DO9: a card whose `tasks` is `"x"` (C-0006, open): exit 0, its note on
/// stderr as `note: …` and in `notes`, the card still counted in its
/// column; C-0001's `"x"` drops copy's `T-0001` (the card passed over), the
/// notes sorted by card ID.
#[test]
fn do9_a_tasks_value_that_does_not_read_is_a_note() {
    let note =
        |id: &str| format!("{id}: its `tasks` does not read; its agent tasks are not counted");
    let scene = Scene::scenario("do9-tasks-x");
    scene.sql("UPDATE cards SET tasks = '\"x\"' WHERE id = 'C-0006';");
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    assert_eq!(
        document["notes"],
        json!([note("C-0006")]),
        "{}",
        json.show()
    );
    assert_eq!(json.stderr, format!("note: {}\n", note("C-0006")));
    assert_eq!(document["columns"], json!(scenario_columns()));
    let text = counts(&scene, &[]);
    text.code(0);
    assert_eq!(text.stdout, DATA_TEXT);
    assert_eq!(text.stderr, format!("note: {}\n", note("C-0006")));

    scene.sql("UPDATE cards SET tasks = '\"x\"' WHERE id = 'C-0001';");
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    assert_eq!(
        document["notes"],
        json!([note("C-0001"), note("C-0006")]),
        "{}",
        json.show()
    );
    assert_eq!(
        json.stderr,
        format!("note: {}\nnote: {}\n", note("C-0001"), note("C-0006"))
    );
    assert_eq!(
        document["by_flow"][0]["review_agent_tasks"],
        json!([]),
        "C-0001 passed over"
    );
    assert_eq!(
        document["by_flow"][1]["review_agent_tasks"],
        json!(["T-0001"])
    );
    assert_eq!(document["columns"], json!(scenario_columns()));
}

// ------------------------------------------------------------------ DO1

/// DO1: 700 open cards with 120-character titles, spread over the three
/// flows: `spec card list --json` is cut (`truncated` true, fewer cards
/// shown), `spec card counts --json` is not: its counts sum to 700,
/// `truncated` false, `omitted` null, and its text header says 700. M: the
/// counts taken from `card_list`'s rows.
#[test]
fn do1_the_counts_are_uncapped_where_the_list_is_cut() {
    let scene = Scene::new("do1-uncapped");
    let disciplines = ["copy", "plates", "sound"];
    for n in 0..700 {
        let title = format!("Atlas sheet {n:04} {}", "x".repeat(120));
        let title: String = title.chars().take(120).collect();
        assert_eq!(title.len(), 120);
        scene.card(&new_card(&title, disciplines[n % 3], 1));
    }
    let list = spec(&scene.home, &scene.root, &["card", "list", "--json"]);
    list.code(0);
    let listed = list.json();
    assert_eq!(listed["truncated"], true, "the premise: the list is cut");
    assert!(listed["cards"].as_array().expect("cards").len() < 700);

    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    assert_eq!(document["truncated"], false);
    assert_eq!(document["omitted"], Value::Null);
    let sum: u64 = document["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .map(|row| {
            row["open"].as_u64().expect("open")
                + row["accepted"].as_u64().expect("accepted")
                + row["cancelled"].as_u64().expect("cancelled")
        })
        .sum();
    assert_eq!(sum, 700);
    let firsts: Vec<(String, u64)> = document["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .filter(|row| row["open"].as_u64() != Some(0))
        .map(|row| {
            (
                format!(
                    "{}/{}",
                    row["flow"].as_str().unwrap_or("?"),
                    row["column"].as_str().unwrap_or("?")
                ),
                row["open"].as_u64().expect("open"),
            )
        })
        .collect();
    assert_eq!(
        firsts,
        [
            ("copy/todo".to_owned(), 234),
            ("plates/todo".to_owned(), 233),
            ("sound/todo".to_owned(), 233)
        ]
    );
    let text = counts(&scene, &[]);
    text.code(0);
    assert_eq!(
        text.stdout.lines().next(),
        Some(
            "card counts | flows 3 | cards 700: open 700, accepted 0, cancelled 0 | agent tasks counted"
        )
    );
    assert!(!text.stdout.contains("[truncated:"), "{}", text.stdout);
}

// ------------------------------------------------------------------ DO5

/// DO5: one queue and config state gives one text and one JSON: twice in
/// place, then from a copy of the repository at another root under another
/// `HOME` (the queue's file copied there) and another `TZ`, byte for byte;
/// no key of the document holds an age (`…days`, `age`); the read writes
/// nothing: the data directory's every file byte for byte, `git status`
/// clean.
#[test]
fn do5_one_state_gives_one_answer_anywhere_and_writes_nothing() {
    let scene = Scene::scenario("do5-twice");
    let data = db_under(&scene.home)
        .parent()
        .expect("the data directory")
        .to_path_buf();
    let before = files_under(&data);
    let text = counts(&scene, &[]);
    text.code(0);
    let json = counts(&scene, &["--json"]);
    json.code(0);
    assert_eq!(counts(&scene, &[]).stdout, text.stdout);
    assert_eq!(counts(&scene, &["--json"]).stdout, json.stdout);
    assert_eq!(files_under(&data), before, "a read writes nothing");
    let status = scene.git.git_text(&scene.root, &["status", "--porcelain"]);
    assert_eq!(status, "", "nothing under the root");

    let mut keys = Vec::new();
    all_keys(&json.json(), &mut keys);
    for key in &keys {
        assert!(
            !key.split('_')
                .any(|word| ["age", "days", "ago"].contains(&word)),
            "no age is served: `{key}`"
        );
    }

    // Another root, another HOME, another TZ.
    let other = tempdir_beside(&scene.root, "elsewhere");
    copy_tree(&scene.root, &other.join("repo"));
    let home = other.join("home");
    let db = db_under(&home);
    fs::create_dir_all(db.parent().expect("a parent")).expect("a data directory");
    fs::copy(db_under(&scene.home), &db).expect("the queue copied");
    let root = other.join("repo");
    for (args, want) in [(&[][..], &text.stdout), (&["--json"][..], &json.stdout)] {
        let mut all = vec!["card", "counts"];
        all.extend_from_slice(args);
        let run = spec_with(
            &root,
            &all,
            &[
                ("HOME", home.as_os_str()),
                ("TZ", std::ffi::OsStr::new("Pacific/Kiritimati")),
            ],
        );
        run.code(0);
        assert_eq!(
            &run.stdout,
            want,
            "{args:?} at another root: {}",
            run.show()
        );
    }
    let _ = fs::remove_dir_all(&other);
}

/// A fresh directory beside `root`'s parent, named by `label`.
fn tempdir_beside(root: &Path, label: &str) -> std::path::PathBuf {
    let dir = root.parent().expect("the scene's directory").join(label);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a directory");
    dir
}

/// `from` copied to `to`, every file and directory (`.git` too).
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a directory") {
        let entry = entry.expect("an entry");
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).expect("a file copied");
        }
    }
}

/// The scene's pinned JSON parses, so the pin itself is a document.
#[test]
fn do9_the_pinned_json_is_one_document() {
    let pinned: Value = serde_json::from_str(&with_flows("[]")).expect("JSON");
    assert_eq!(pinned["columns"], json!(scenario_columns()));
    assert_eq!(
        pinned["by_flow"],
        json!([
            by_flow(
                "copy",
                3,
                Some("2026-11-02T09:00:00Z"),
                1,
                Some(&["T-0001"])
            ),
            by_flow("plates", 0, None, 0, Some(&["T-0001"])),
            by_flow("sound", 0, None, 0, Some(&[])),
        ])
    );
    assert_eq!(compact("{ \"a\" : [1, 2] }"), "{\"a\":[1,2]}");
}

/// The premise of every test here: the scene's queue holds "Data"'s
/// scenario, read by SQL: each card's flow, column, state, parent and
/// links; each item's card, kind, status and time; each task's status.
#[test]
fn the_scene_is_the_specs_scenario() {
    let scene = Scene::scenario("do-scene");
    assert_eq!(
        scene.sql(
            "SELECT id, flow, col, acceptance IS NOT NULL, cancellation IS NOT NULL, \
             COALESCE(parent, '-'), COALESCE(tasks, '-') FROM cards ORDER BY id;"
        ),
        "C-0001|copy|review|0|0|-|[\"T-0001\"]\n\
         C-0002|plates|done|1|0|C-0001|[]\n\
         C-0003|copy|todo|0|1|C-0001|[]\n\
         C-0004|copy|returned|0|0|C-0005|[]\n\
         C-0005|plates|todo|0|0|-|[\"T-0001\",\"T-0002\"]\n\
         C-0006|sound|todo|0|0|-|[]\n\
         C-0007|copy|proofing|0|0|-|[]"
    );
    assert_eq!(
        scene.sql("SELECT id, card_id, kind, status, created_at FROM proposals ORDER BY id;"),
        "PR-0001|C-0001|question|open|2026-11-02T09:00:00Z\n\
         PR-0002|C-0004|question|open|2026-11-05T09:00:00Z\n\
         PR-0003|C-0001|decision|applied|2026-11-06T09:00:00Z\n\
         PR-0004|C-0003|question|open|2026-11-07T09:00:00Z"
    );
    assert_eq!(
        scene.sql("SELECT id, status FROM tasks ORDER BY id;"),
        "T-0001|review\nT-0002|ready"
    );
    let config = fs::read_to_string(scene.root.join("specengine.toml")).expect("config");
    assert!(
        !config.contains("proofing"),
        "the config no longer lists it"
    );
}

/// DO9 (the developer's iteration 2, "D2": a row the list would note as
/// corrupt still counts where it stands): C-0006's stored `col` `NULL`
/// counts under `NULL`, a column `sound` does not list, its note on stderr
/// and in `notes` before C-0007's `tasks` note (by card ID), exit 0.
#[test]
fn do9_a_column_that_is_null_counts_under_null_with_its_note() {
    let scene = Scene::scenario("do9-null-col");
    scene.sql("UPDATE cards SET col = NULL WHERE id = 'C-0006';");
    scene.sql("UPDATE cards SET tasks = '\"x\"' WHERE id = 'C-0007';");
    let named = "C-0006: the stored `col` cannot be read: it is NULL; counted under `NULL`";
    let unread = "C-0007: its `tasks` does not read; its agent tasks are not counted";
    let json = counts(&scene, &["--json"]);
    json.code(0);
    let document = json.json();
    assert_eq!(document["notes"], json!([named, unread]), "{}", json.show());
    assert_eq!(json.stderr, format!("note: {named}\nnote: {unread}\n"));
    let sound: Vec<&Value> = document["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .filter(|row| row["flow"] == "sound")
        .collect();
    assert_eq!(
        sound,
        [
            &column("sound", "todo", [0, 0, 0, 0]),
            &column("sound", "check", [0, 0, 0, 0]),
            &column("sound", "done", [0, 0, 0, 0]),
            &column("sound", "NULL", [1, 0, 0, 0]),
        ]
    );
    let text = counts(&scene, &[]);
    text.code(0);
    assert!(
        text.stdout.contains(
            "\n  sound/NULL | open 1, accepted 0, cancelled 0, open subtasks 0 | not in the flow\n"
        ),
        "{}",
        text.stdout
    );
}
