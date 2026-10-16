//! Helpers of the docs/features/bevy-roles-reads.md tests (slice 10a-2,
//! roles in `spec symbols` and `find_symbols`): the `spec symbols` forms
//! read over scratch copies of `fixtures/code-bevy` (10a-1's corpus as it
//! stands, a module and a RON marker of `CHORE-02` written in), and what
//! its `expected.json` gives per target: the rows shown on an item
//! (the spec's "spec symbols", "On a target": the rows sitting on it and
//! the rows whose subject it is `linked` or `ambiguous` for,
//! docs/canon/code-roles.md "Links"), their lines and their JSON objects,
//! in the spec's order.
//!
//! The opt-in answer is derived from the answer without it: the same
//! text with each item target's role lines after its `sig` line, the
//! same document with each item target's `roles` the rows' objects and
//! each module or RON target's `[]`. So no 6a-3 line (target, `sig`,
//! binding, summary) is restated here: RD-7 keeps those equal.
//!
//! Readings (Phase A; the spec leaves them open, Phase B confirms them
//! against the first run, never by copying output in):
//!
//! - R1: JSON `form` and `origin` are the stored values (`path` and `code`
//!   too); only the text drops them ("`<form>` only `closure` or
//!   `factory`; `<origin>` only when not `code`").
//! - R2: JSON `candidates` is the count on an `ambiguous` row and `null` on
//!   every other; `detail` is the stored category (`null` but on an
//!   `uncertain` row).
//! - R3: `layer` is `a` on every row of 10a (layer B is 10b's, parked).
//! - R4: JSON `path` is the stored path as it is; the text's goes through
//!   `one_line` (as `search`'s, `names.rs`).
//!
//! Only the bevy-roles-reads test binaries include this module
//! (`mod bevy_roles_reads_common;`, after `mod common;`,
//! `mod code_index_common;` and `mod bevy_roles_common;`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::bevy_roles_common::{self, expected};
use crate::common::{Run, Scratch, replace, spec};

/// R3: the layer of every row of 10a.
pub const LAYER: &str = "a";

/// One role of `symbols[].roles[]`, in order ("spec symbols", JSON).
pub const ROLE_KEYS: [&str; 11] = [
    "role",
    "layer",
    "path",
    "line",
    "source",
    "form",
    "origin",
    "group",
    "state",
    "detail",
    "candidates",
];

/// A module marker and a RON marker of `CHORE-02` (code-bevy holds no
/// other), each (path, the line as committed, the line with the marker):
/// `--node CHORE-02` lists a module target and a RON target. `clock.rs`
/// holds no role row, so no row's line moves.
pub const MARKS: [(&str, &str, &str); 2] = [
    (
        "crates/farm/src/clock.rs",
        "//! The clock: what turns over every tick.\n",
        "//! The clock: what turns over every tick.\n//! @implements CHORE-02\n",
    ),
    (
        "assets/farm.ron",
        "    growth_per_tick: 1,\n",
        "    growth_per_tick: 1, // @configures CHORE-02\n",
    ),
];

/// Every `spec symbols` form read over code-bevy: QUERY (each tier, a
/// limit, zero rows), `--node` (an item, a module and a RON path, a REF
/// naming nothing: exit 1), `--file` of every walked code file, `--at`
/// (nested items, a macro body), and two exit-2 answers.
pub const FORMS: [&[&str]; 30] = [
    &["symbols", "tick"],
    &["symbols", "run"],
    &["symbols", "run", "--limit", "1"],
    &["symbols", "graze"],
    &["symbols", "Weather"],
    &["symbols", "Harvest"],
    &["symbols", "Quake"],
    &["symbols", "FarmPlugin"],
    &["symbols", "crates/farm::clock::run"],
    &["symbols", "build"],
    &["symbols", "no_such_item_anywhere"],
    &["symbols", "--node", "ALM-GROWTH"],
    &["symbols", "--node", "CHORE-01"],
    &["symbols", "--node", "CHORE-02"],
    &["symbols", "--node", "CHORE-99"],
    &["symbols", "--file", "assets/farm.ron"],
    &["symbols", "--file", "crates/farm-app/src/main.rs"],
    &["symbols", "--file", "crates/farm/src/clock.rs"],
    &["symbols", "--file", "crates/farm/src/crops.rs"],
    &["symbols", "--file", "crates/farm/src/fields.rs"],
    &["symbols", "--file", "crates/farm/src/herd.rs"],
    &["symbols", "--file", "crates/farm/src/lib.rs"],
    &["symbols", "--file", "crates/farm/src/news.rs"],
    &["symbols", "--file", "crates/farm/src/weather.rs"],
    &["symbols", "--at", "crates/farm/src/lib.rs:30"],
    &["symbols", "--at", "crates/farm/src/herd.rs:32"],
    &["symbols", "--at", "crates/farm/src/fields.rs:22"],
    &["symbols", "--at", "crates/farm/src/news.rs:28"],
    &["symbols", "--file", "docs/spec/almanac.md"],
    &["symbols", "--at", "crates/farm/src/lib.rs:0"],
];

/// `MARKS` written into `root`.
pub fn mark_non_items(root: &Path) {
    for (path, from, to) in MARKS {
        replace(root, path, from, to);
    }
}

/// A scratch copy of code-bevy at `<scratch>/<dir>` (no `expected.json`),
/// [`MARKS`] written; its `[code]` the fixture's (the opt-in on).
pub fn bevy_copy(scratch: &Scratch, dir: &str) -> PathBuf {
    let root = bevy_roles_common::copy(scratch, dir);
    mark_non_items(&root);
    root
}

/// `spec <args>` and `spec --json <args>` in `cwd`.
pub fn both(home: &Path, cwd: &Path, args: &[&str]) -> (Run, Run) {
    let mut json_args = vec!["--json"];
    json_args.extend_from_slice(args);
    (spec(home, cwd, args), spec(home, cwd, &json_args))
}

/// `text` with every line break the CLI's `one_line` turns into a space.
pub fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// Every role row of `fixtures/code-bevy/expected.json` (10a-1's: the
/// stored columns, then the per-read `state`, `candidates` as qpaths,
/// `sits_on`).
pub fn expected_roles() -> Vec<Value> {
    expected()["roles"]
        .as_array()
        .expect("expected.json `roles`")
        .clone()
}

/// The spec's order of the rows on one target: layer (one here, R3),
/// word, path bytes, line, source, `ord`.
fn order_key(row: &Value) -> (String, Vec<u8>, u64, String, u64) {
    (
        row["word"].as_str().expect("a word").to_owned(),
        row["path"].as_str().expect("a path").as_bytes().to_vec(),
        row["line"].as_u64().expect("a line"),
        row["source"].as_str().expect("a source").to_owned(),
        row["ord"].as_u64().expect("an ord"),
    )
}

/// Whether `row` shows on the item `qpath`: it sits on it (a row without a
/// subject, `certain` or `uncertain`), or its subject is `linked` or
/// `ambiguous` with `qpath` among its candidates. `unlinked` rows and
/// rows on nothing show nowhere.
pub fn shows_on(row: &Value, qpath: &str) -> bool {
    if row["sits_on"] == qpath {
        return true;
    }
    matches!(row["state"].as_str(), Some("linked" | "ambiguous"))
        && row["candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|c| c == qpath))
}

/// The rows shown on the item `qpath`, in the spec's order.
pub fn shown_on<'a>(rows: &'a [Value], qpath: &str) -> Vec<&'a Value> {
    let mut shown: Vec<&Value> = rows.iter().filter(|row| shows_on(row, qpath)).collect();
    shown.sort_by_key(|row| order_key(row));
    shown
}

/// A row's line under its target, two spaces first, no line end:
/// `role <word> | <layer> | <path>:<line> | <source>[ <form>][ <origin>] |
/// <group or -> | <state>[ <detail>]`.
pub fn role_line(row: &Value) -> String {
    let text = |key: &str| {
        row[key]
            .as_str()
            .unwrap_or_else(|| panic!("`{key}` of {row}"))
    };
    let mut source = text("source").to_owned();
    if let Some(form @ ("closure" | "factory")) = row["form"].as_str() {
        source.push(' ');
        source.push_str(form);
    }
    let origin = text("origin");
    if origin != "code" {
        source.push(' ');
        source.push_str(origin);
    }
    let group = row["group_text"].as_str().unwrap_or("-");
    let state = match text("state") {
        "uncertain" => match row["category"].as_str() {
            Some(category) => format!("uncertain {category}"),
            None => "uncertain".to_owned(),
        },
        "ambiguous" => format!(
            "ambiguous {}",
            row["candidates"].as_array().expect("candidates").len()
        ),
        other => other.to_owned(),
    };
    format!(
        "  role {} | {LAYER} | {}:{} | {source} | {} | {state}",
        text("word"),
        one_line(text("path")),
        row["line"],
        one_line(group)
    )
}

/// A row's JSON object (R1, R2, R3, R4).
pub fn role_json(row: &Value) -> Value {
    let state = row["state"].as_str().expect("a state");
    let candidates = if state == "ambiguous" {
        json!(row["candidates"].as_array().expect("candidates").len())
    } else {
        Value::Null
    };
    json!({
        "role": row["word"],
        "layer": LAYER,
        "path": row["path"],
        "line": row["line"],
        "source": row["source"],
        "form": row["form"],
        "origin": row["origin"],
        "group": row["group_text"],
        "state": state,
        "detail": row["category"],
        "candidates": candidates,
    })
}

/// The qpath of an item target line (`<qpath> | <item_kind> |
/// <path>:<start>-<end> | layer <l> | mentions <n>…`); `None` for a
/// module or RON target, an indented line, a tail or the summary.
pub fn item_qpath(line: &str) -> Option<&str> {
    if line.starts_with(' ') || line.starts_with("symbols: ") || line.starts_with("[truncated: ") {
        return None;
    }
    let fields: Vec<&str> = line.split(" | ").collect();
    (fields.len() >= 5 && !["module", "ron"].contains(&fields[1])).then_some(fields[0])
}

/// The text an opt-in run prints, from the same form's text without the
/// opt-in: after each item target's `sig` line, the lines of the rows
/// shown on it.
pub fn with_role_lines(text: &str, rows: &[Value]) -> String {
    let mut out = String::new();
    let mut pending: Option<Vec<String>> = None;
    for line in text.split_inclusive('\n') {
        out.push_str(line);
        let bare = line.trim_end_matches('\n');
        if let Some(qpath) = item_qpath(bare) {
            pending = Some(shown_on(rows, qpath).into_iter().map(role_line).collect());
        } else if bare.starts_with("  sig ") {
            for role in pending.take().unwrap_or_default() {
                out.push_str(&role);
                out.push('\n');
            }
        }
    }
    out
}

/// The document an opt-in run prints, from the same form's document
/// without the opt-in: each item target's `roles` the objects of the rows
/// shown on it, each module or RON target's `[]`.
pub fn with_roles(document: &Value, rows: &[Value]) -> Value {
    let mut document = document.clone();
    if let Some(targets) = document["symbols"].as_array_mut() {
        for target in targets {
            target["roles"] = if target["target"] == "item" {
                let qpath = target["qpath"].as_str().expect("a qpath");
                Value::Array(shown_on(rows, qpath).into_iter().map(role_json).collect())
            } else {
                json!([])
            };
        }
    }
    document
}

/// A text without its `  role ` lines.
pub fn text_without_roles(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| !line.starts_with("  role "))
        .collect()
}

/// A document without each target's `roles`.
pub fn json_without_roles(document: &Value) -> Value {
    let mut document = document.clone();
    if let Some(targets) = document["symbols"].as_array_mut() {
        for target in targets {
            if let Some(object) = target.as_object_mut() {
                object.remove("roles");
            }
        }
    }
    document
}

/// The role lines printed under the target whose qpath is `qpath` (the
/// first such target of `text`), in order.
pub fn role_lines_of(text: &str, qpath: &str) -> Vec<String> {
    let mut lines = text.lines();
    let found = lines
        .by_ref()
        .any(|line| line.split(" | ").next() == Some(qpath));
    assert!(found, "no target {qpath} in\n{text}");
    lines
        .take_while(|line| line.starts_with("  "))
        .filter(|line| line.starts_with("  role "))
        .map(str::to_owned)
        .collect()
}

/// The `roles` of the first JSON target whose qpath is `qpath`.
pub fn roles_of(document: &Value, qpath: &str) -> Value {
    document["symbols"]
        .as_array()
        .expect("symbols")
        .iter()
        .find(|target| target["qpath"] == qpath)
        .unwrap_or_else(|| panic!("no target {qpath} in {document}"))["roles"]
        .clone()
}
