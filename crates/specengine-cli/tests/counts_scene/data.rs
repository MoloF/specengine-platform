//! docs/features/department-overview.md (RM-0086) "Acceptance criteria",
//! DO9: the scenario's pinned output, the spec's JSON (its `flows` elided:
//! the scene fills in `spec card list --json`'s) and its text, shared by the
//! scene (`mod.rs`). Self-contained: no crate but `std`.

#![allow(dead_code)]

/// The spec's pinned JSON document of the scenario, as written there, but
/// `flows`: `"flows": FLOWS` stands where the spec elides it ([`with_flows`]).
pub const DATA_JSON: &str = r#"{"flows": FLOWS,
 "columns": [
  {"flow": "copy", "column": "todo", "open": 0, "accepted": 0, "cancelled": 1, "open_subtasks": 0},
  {"flow": "copy", "column": "doing", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "copy", "column": "review", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "copy", "column": "returned", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 1},
  {"flow": "copy", "column": "done", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "copy", "column": "proofing", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "plates", "column": "todo", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "plates", "column": "drawing", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "plates", "column": "done", "open": 0, "accepted": 1, "cancelled": 0, "open_subtasks": 0},
  {"flow": "sound", "column": "todo", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "sound", "column": "check", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "sound", "column": "done", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0}],
 "by_flow": [
  {"flow": "copy", "open_items": 3, "oldest_open_item": "2026-11-02T09:00:00Z", "ready_parents": 1, "review_agent_tasks": ["T-0001"]},
  {"flow": "plates", "open_items": 0, "oldest_open_item": null, "ready_parents": 0, "review_agent_tasks": ["T-0001"]},
  {"flow": "sound", "open_items": 0, "oldest_open_item": null, "ready_parents": 0, "review_agent_tasks": []}],
 "agent_tasks": "counted", "notes": [], "truncated": false, "omitted": null}"#;

/// The spec's pinned text of the scenario's `spec card counts`, line for
/// line.
pub const DATA_TEXT: &str = "\
card counts | flows 3 | cards 7: open 5, accepted 1, cancelled 1 | agent tasks counted
flow copy | Copy | open items 3, oldest 2026-11-02T09:00:00Z | ready parents 1 | agent tasks in review T-0001
  copy/todo | open 0, accepted 0, cancelled 1, open subtasks 0
  copy/doing | open 0, accepted 0, cancelled 0, open subtasks 0
  copy/review | open 1, accepted 0, cancelled 0, open subtasks 0
  copy/returned | open 1, accepted 0, cancelled 0, open subtasks 1
  copy/done | open 0, accepted 0, cancelled 0, open subtasks 0
  copy/proofing | open 1, accepted 0, cancelled 0, open subtasks 0 | not in the flow
flow plates | Plates | open items 0, oldest - | ready parents 0 | agent tasks in review T-0001
  plates/todo | open 1, accepted 0, cancelled 0, open subtasks 0
  plates/drawing | open 0, accepted 0, cancelled 0, open subtasks 0
  plates/done | open 0, accepted 1, cancelled 0, open subtasks 0
flow sound | Sound | open items 0, oldest - | ready parents 0 | agent tasks in review -
  sound/todo | open 1, accepted 0, cancelled 0, open subtasks 0
  sound/check | open 0, accepted 0, cancelled 0, open subtasks 0
  sound/done | open 0, accepted 0, cancelled 0, open subtasks 0
";

/// `text` with every whitespace outside a string dropped: the one-line
/// form `--json` prints.
pub fn compact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut in_string, mut escaped) = (false, false);
    for ch in text.chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if !ch.is_whitespace() {
            out.push(ch);
        }
    }
    out
}

/// [`DATA_JSON`] compact, `flows` the raw `flows` array given (as `spec card
/// list --json` prints it, compact).
pub fn with_flows(flows: &str) -> String {
    compact(DATA_JSON).replacen("FLOWS", flows, 1)
}
