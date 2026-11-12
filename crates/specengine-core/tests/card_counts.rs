//! docs/features/department-overview.md (RM-0086) "Acceptance criteria",
//! core's half: `counts_document` is pure, the store's grouped rows in, the
//! counts document out, ordered by `[card]`.
//!
//! - DO9 (core): configured flows in config order, each configured column
//!   in config order with zeros, then the columns it no longer lists by
//!   name; then the flows no `[card]` lists, by name, their stored columns
//!   alone by name; every flow a `by_flow` row, zeros too (a flow known
//!   only from its items too); IDs sorted by number, not by text (`T-9999`
//!   before `T-10000`), notes by card number. M: a stranded column dropped.
//! - DO4/DO10 (core): without `tasks_here`, `agent_tasks` `local_only`,
//!   every `review_agent_tasks` `null` and no note, whatever the rows hold.
//!   M: `[]` served on a hub.
//! - DO5 (core): the three key sets in the spec's order, no age; one input,
//!   one document. M: an `oldest_open_item_days` key served.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use specengine_core::card_counts::{
    AgentTasks, CardCountRows, ColumnTally, OpenItems, counts_document,
};
use specengine_core::card_toml::{CardConfig, CardFlow};

/// Two flows, `inks` before `bindery` in config order (not by name).
const FLOWS: &str = r#"
[[card.flow]]
name = "inks"
title = "Inks"
disciplines = ["inks"]
columns = ["mix", "test", "shelf"]
accept = { from = "test", to = "shelf" }

[[card.flow]]
name = "bindery"
disciplines = ["bindery"]
columns = ["fold", "sew", "glue", "bound"]
accept = { from = "glue", to = "bound" }
"#;

fn flows() -> Vec<CardFlow> {
    CardConfig::from_toml(FLOWS).expect("the [card]").flows
}

fn tally(open: u64, accepted: u64, cancelled: u64, open_subtasks: u64) -> ColumnTally {
    ColumnTally {
        open,
        accepted,
        cancelled,
        open_subtasks,
    }
}

fn key(flow: &str, column: &str) -> (String, String) {
    (flow.to_owned(), column.to_owned())
}

fn ids(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

/// Rows over both flows, a column `inks` no longer lists (`drying`, and
/// `aging` before it by name), and two flows no `[card]` lists (`zinc`,
/// `cloth`), one known only from its items.
fn rows() -> CardCountRows {
    CardCountRows {
        columns: BTreeMap::from([
            (key("inks", "mix"), tally(2, 0, 1, 1)),
            (key("inks", "drying"), tally(1, 0, 0, 0)),
            (key("inks", "aging"), tally(0, 1, 0, 0)),
            (key("inks", "shelf"), tally(0, 3, 0, 0)),
            (key("bindery", "sew"), tally(4, 0, 0, 2)),
            (key("zinc", "etch"), tally(1, 0, 0, 0)),
            (key("zinc", "bath"), tally(0, 0, 2, 0)),
        ]),
        ready_parents: BTreeMap::from([("bindery".to_owned(), 2)]),
        open_items: BTreeMap::from([
            (
                "inks".to_owned(),
                OpenItems {
                    count: 2,
                    oldest: Some("2026-11-03T09:00:00Z".to_owned()),
                },
            ),
            (
                "cloth".to_owned(),
                OpenItems {
                    count: 1,
                    oldest: Some("2026-11-04T09:00:00Z".to_owned()),
                },
            ),
        ]),
        review_tasks: BTreeMap::from([
            ("inks".to_owned(), ids(&["T-10000", "T-9999", "T-0002"])),
            ("zinc".to_owned(), ids(&["T-0007"])),
        ]),
        unread_tasks: ids(&["C-10000", "C-9999"]),
        unread_names: Vec::new(),
    }
}

fn column(flow: &str, name: &str, counts: [u64; 4]) -> Value {
    json!({"flow": flow, "column": name, "open": counts[0], "accepted": counts[1],
        "cancelled": counts[2], "open_subtasks": counts[3]})
}

fn by_flow(flow: &str, items: u64, oldest: Option<&str>, ready: u64, review: Value) -> Value {
    json!({"flow": flow, "open_items": items, "oldest_open_item": oldest,
        "ready_parents": ready, "review_agent_tasks": review})
}

/// The keys of the JSON object `text`, in the order written.
fn keys_in_order(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    let mut current = String::new();
    let mut after_string_at_depth_one = false;
    for letter in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
                current.push(letter);
            } else if letter == '\\' {
                escaped = true;
            } else if letter == '"' {
                in_string = false;
                after_string_at_depth_one = depth == 1;
            } else {
                current.push(letter);
            }
            continue;
        }
        match letter {
            '"' => {
                in_string = true;
                current.clear();
            }
            ':' if after_string_at_depth_one => keys.push(current.clone()),
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
        if letter != '"' {
            after_string_at_depth_one = false;
        }
    }
    keys
}

/// DO9 (core): the order "Data" fixes, rows and flows; IDs and notes by
/// number. M: a stranded column dropped.
#[test]
fn do9_the_document_follows_the_config_then_names() {
    let flows = flows();
    let document = counts_document(&flows, &rows(), true);
    let served = serde_json::to_value(&document).expect("JSON");
    assert_eq!(
        served["flows"],
        serde_json::to_value(&flows).expect("flows")
    );
    assert_eq!(
        served["columns"],
        json!([
            column("inks", "mix", [2, 0, 1, 1]),
            column("inks", "test", [0, 0, 0, 0]),
            column("inks", "shelf", [0, 3, 0, 0]),
            column("inks", "aging", [0, 1, 0, 0]),
            column("inks", "drying", [1, 0, 0, 0]),
            column("bindery", "fold", [0, 0, 0, 0]),
            column("bindery", "sew", [4, 0, 0, 2]),
            column("bindery", "glue", [0, 0, 0, 0]),
            column("bindery", "bound", [0, 0, 0, 0]),
            column("zinc", "bath", [0, 0, 2, 0]),
            column("zinc", "etch", [1, 0, 0, 0]),
        ])
    );
    assert_eq!(
        served["by_flow"],
        json!([
            by_flow(
                "inks",
                2,
                Some("2026-11-03T09:00:00Z"),
                0,
                json!(["T-0002", "T-9999", "T-10000"])
            ),
            by_flow("bindery", 0, None, 2, json!([])),
            by_flow("cloth", 1, Some("2026-11-04T09:00:00Z"), 0, json!([])),
            by_flow("zinc", 0, None, 0, json!(["T-0007"])),
        ])
    );
    assert_eq!(served["agent_tasks"], "counted");
    assert_eq!(document.agent_tasks, AgentTasks::Counted);
    assert_eq!(
        served["notes"],
        json!([
            "C-9999: its `tasks` does not read; its agent tasks are not counted",
            "C-10000: its `tasks` does not read; its agent tasks are not counted",
        ])
    );
    assert_eq!(served["truncated"], false);
    assert_eq!(served["omitted"], Value::Null);
}

/// DO4/DO10 (core): without `tasks_here` the document says `local_only`,
/// every `review_agent_tasks` `null` (never `[]`), no note; every other
/// value as with it. M: `[]` served on a hub.
#[test]
fn do4_a_hub_document_is_local_only_and_null() {
    let flows = flows();
    let here = serde_json::to_value(counts_document(&flows, &rows(), true)).expect("JSON");
    let hub = serde_json::to_value(counts_document(&flows, &rows(), false)).expect("JSON");
    assert_eq!(hub["agent_tasks"], "local_only");
    for row in hub["by_flow"].as_array().expect("by_flow") {
        assert_eq!(row["review_agent_tasks"], Value::Null, "{row}");
    }
    assert_eq!(hub["notes"], json!([]));
    let mut want = here.clone();
    want["agent_tasks"] = json!("local_only");
    want["notes"] = json!([]);
    for row in want["by_flow"].as_array_mut().expect("by_flow") {
        row["review_agent_tasks"] = Value::Null;
    }
    assert_eq!(hub, want);
}

/// DO9 (core): no rows: each configured column a zero row, each configured
/// flow a zero row, `flows` as given; no `[card]` flow: `flows` empty and
/// every stored column stranded, by flow and column name.
#[test]
fn do9_no_rows_and_no_flows() {
    let flows = flows();
    let empty = serde_json::to_value(counts_document(&flows, &CardCountRows::default(), true))
        .expect("JSON");
    assert_eq!(empty["columns"].as_array().map(Vec::len), Some(7));
    assert!(
        empty["columns"]
            .as_array()
            .expect("columns")
            .iter()
            .all(|row| row["open"] == 0 && row["accepted"] == 0 && row["cancelled"] == 0)
    );
    assert_eq!(
        empty["by_flow"],
        json!([
            by_flow("inks", 0, None, 0, json!([])),
            by_flow("bindery", 0, None, 0, json!([])),
        ])
    );
    let bare = serde_json::to_value(counts_document(&[], &rows(), true)).expect("JSON");
    assert_eq!(bare["flows"], json!([]));
    let order: Vec<String> = bare["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .map(|row| {
            format!(
                "{}/{}",
                row["flow"].as_str().unwrap_or("?"),
                row["column"].as_str().unwrap_or("?")
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            "bindery/sew",
            "inks/aging",
            "inks/drying",
            "inks/mix",
            "inks/shelf",
            "zinc/bath",
            "zinc/etch"
        ]
    );
    let flows_named: Vec<&str> = bare["by_flow"]
        .as_array()
        .expect("by_flow")
        .iter()
        .map(|row| row["flow"].as_str().expect("a flow"))
        .collect();
    assert_eq!(flows_named, ["bindery", "cloth", "inks", "zinc"]);
}

/// DO5 (core): the three key sets in the spec's order (`CardCounts` 7,
/// `CardColumnCount` 6, `CardFlowCount` 5), none an age; one input, one
/// document, byte for byte. M: an `oldest_open_item_days` key served.
#[test]
fn do5_the_key_sets_hold_no_age_and_one_input_gives_one_document() {
    let flows = flows();
    let text = serde_json::to_string(&counts_document(&flows, &rows(), true)).expect("JSON");
    assert_eq!(
        serde_json::to_string(&counts_document(&flows, &rows(), true)).expect("JSON"),
        text
    );
    assert_eq!(
        keys_in_order(&text),
        [
            "flows",
            "columns",
            "by_flow",
            "agent_tasks",
            "notes",
            "truncated",
            "omitted"
        ]
    );
    let document: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(document["columns"].as_array().map(Vec::len), Some(11));
    // The rows' own order, as serialized by the struct.
    let row_text =
        serde_json::to_string(&counts_document(&flows, &rows(), true).columns[0]).expect("a row");
    assert_eq!(
        keys_in_order(&row_text),
        [
            "flow",
            "column",
            "open",
            "accepted",
            "cancelled",
            "open_subtasks"
        ]
    );
    let flow_text =
        serde_json::to_string(&counts_document(&flows, &rows(), true).by_flow[0]).expect("a row");
    assert_eq!(
        keys_in_order(&flow_text),
        [
            "flow",
            "open_items",
            "oldest_open_item",
            "ready_parents",
            "review_agent_tasks"
        ]
    );
    for key in keys_in_order(&text)
        .into_iter()
        .chain(keys_in_order(&row_text))
        .chain(keys_in_order(&flow_text))
    {
        assert!(
            !key.split('_')
                .any(|word| ["age", "days", "ago"].contains(&word)),
            "an age: `{key}`"
        );
    }
}
