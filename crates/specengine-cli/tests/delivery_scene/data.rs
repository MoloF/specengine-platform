//! docs/features/delivery-analytics.md (RM-0107 a): the scenario's pinned
//! output, the spec's pinned JSON and text at 2026-11-20, shared by the scene
//! (`mod.rs`) and `specengine-core`'s `tests/delivery.rs`, which includes
//! this file by path. Self-contained: no crate but `std`.
//!
//! docs/features/delivery-timeline.md (RM-0107 b) "Acceptance criteria",
//! DT1 and DT4: part a's document holds `"timeline": null` after `late`;
//! [`TIMELINE_JSON`] and [`TIMELINE_TEXT`] are that spec's timeline of the
//! scenario at 2026-11-20, as written there: C-0005 made 2026-11-06, and
//! C-0004 taken by its move to `review` on 11-03 (`copy` accepts from
//! `review` alone; part a's take, DA-2), so the `none` marker runs 11-03 to
//! 11-05.

#![allow(dead_code)]

/// The spec's pinned JSON document of the scenario at 2026-11-20, as written
/// there (the CLI prints it compact: [`compact`]), with delivery-timeline's
/// `"timeline": null` (DT4).
pub const DATA_JSON: &str = r#"{"as_of": "2026-11-20", "notes": [],
 "releases": [
  {"group": "release", "release": "r1", "tasks": 1, "delivered": 1, "delivered_late": 1, "open": 0, "overdue": 0, "cancelled": 1, "lead_days": {"median": 14, "max": 14}},
  {"group": "release", "release": "r2", "tasks": 1, "delivered": 0, "delivered_late": 0, "open": 1, "overdue": 1, "cancelled": 0, "lead_days": null},
  {"group": "none", "release": null, "tasks": 2, "delivered": 1, "delivered_late": 0, "open": 1, "overdue": 0, "cancelled": 0, "lead_days": {"median": 3, "max": 3}}],
 "departments": [
  {"name": "words", "title": "Words", "tasks": 2, "delivered": 2, "delivered_late": 1, "open": 0, "overdue": 0, "cancelled": 0, "lead_days": {"median": 3, "max": 14}},
  {"name": "print", "title": "Print", "tasks": 3, "delivered": 2, "delivered_late": 1, "open": 1, "overdue": 1, "cancelled": 1, "lead_days": {"median": 3, "max": 14}}],
 "total": {"tasks": 4, "delivered": 2, "delivered_late": 1, "open": 2, "overdue": 1, "cancelled": 1, "lead_days": {"median": 3, "max": 14}},
 "late": [
  {"id": "C-0003", "title": "Engrave the harbor map", "flow": "plates", "column": "drawing", "parent": null,
   "group": "release", "release": "r2", "departments": ["print"],
   "created_at": "2026-11-04T09:00:00Z", "taken_at": "2026-11-05T09:00:00Z", "accepted_at": null,
   "due": "2026-11-12", "days_late": 8, "lead_days": null, "estimate": null,
   "returns": [], "reestimates": [], "cancelled_subtasks": [], "items": [],
   "more": {"returns": 0, "reestimates": 0, "cancelled_subtasks": 0, "items": 0}},
  {"id": "C-0001", "title": "Write the tide table foreword", "flow": "copy", "column": "done", "parent": null,
   "group": "release", "release": "r1", "departments": ["words", "print"],
   "created_at": "2026-11-01T09:00:00Z", "taken_at": "2026-11-03T09:00:00Z", "accepted_at": "2026-11-15T09:00:00Z",
   "due": "2026-11-10", "days_late": 5, "lead_days": 14, "estimate": "5",
   "returns": [{"at": "2026-11-06T09:00:00Z", "from": "review", "note": "Too long"},
               {"at": "2026-11-08T09:00:00Z", "from": "review", "note": "Wrong tone"}],
   "reestimates": [{"at": "2026-11-07T09:00:00Z", "old": "3", "new": "5", "reason": "The tables doubled"}],
   "cancelled_subtasks": [{"id": "C-0002", "title": "Draw the tide plates", "at": "2026-11-09T09:00:00Z", "reason": "Plates reused from last year"}],
   "items": [{"id": "PR-0040", "kind": "question", "status": "applied"}],
   "more": {"returns": 0, "reestimates": 0, "cancelled_subtasks": 0, "items": 0}}],
 "timeline": null,
 "truncated": false, "omitted": null}"#;

/// delivery-timeline's `timeline` object of the scenario at 2026-11-20, as
/// the spec writes it (DT1, DT2, DT5).
pub const TIMELINE_JSON: &str = r#"{"from": "2026-11-01", "to": "2026-11-20",
 "markers": [
  {"group": "release", "release": "r1", "first_taken_at": "2026-11-03T09:00:00Z", "last_accepted_at": "2026-11-15T09:00:00Z"},
  {"group": "release", "release": "r2", "first_taken_at": "2026-11-05T09:00:00Z", "last_accepted_at": null},
  {"group": "none", "release": null, "first_taken_at": "2026-11-03T09:00:00Z", "last_accepted_at": "2026-11-05T09:00:00Z"}],
 "tasks": [
  {"id": "C-0001", "title": "Write the tide table foreword", "flow": "copy", "group": "release", "release": "r1", "departments": ["words", "print"],
   "created_at": "2026-11-01T09:00:00Z", "taken_at": "2026-11-03T09:00:00Z", "accepted_at": "2026-11-15T09:00:00Z", "due": "2026-11-10", "days_late": 5},
  {"id": "C-0003", "title": "Engrave the harbor map", "flow": "plates", "group": "release", "release": "r2", "departments": ["print"],
   "created_at": "2026-11-04T09:00:00Z", "taken_at": "2026-11-05T09:00:00Z", "accepted_at": null, "due": "2026-11-12", "days_late": 8},
  {"id": "C-0004", "title": "Fix the index", "flow": "copy", "group": "none", "release": null, "departments": ["words", "print"],
   "created_at": "2026-11-02T09:00:00Z", "taken_at": "2026-11-03T09:00:00Z", "accepted_at": "2026-11-05T09:00:00Z", "due": null, "days_late": null},
  {"id": "C-0005", "title": "Record the foghorn", "flow": "sound", "group": "none", "release": null, "departments": [],
   "created_at": "2026-11-06T09:00:00Z", "taken_at": null, "accepted_at": null, "due": null, "days_late": null}]}"#;

/// The scenario's document with `--timeline` at 2026-11-20: [`DATA_JSON`]
/// with its `"timeline": null` replaced by [`TIMELINE_JSON`].
pub fn data_timeline_json() -> String {
    let null = "\"timeline\": null";
    assert_eq!(DATA_JSON.matches(null).count(), 1, "one timeline key");
    DATA_JSON.replace(null, &format!("\"timeline\": {TIMELINE_JSON}"))
}

/// delivery-timeline's text lines of the scenario at 2026-11-20, printed
/// after part a's late rows with `--timeline` (DT1).
pub const TIMELINE_TEXT: &str = "\
timeline | 2026-11-01 to 2026-11-20 | tasks 4
marker release r1 | first taken 2026-11-03T09:00:00Z | last accepted 2026-11-15T09:00:00Z
marker release r2 | first taken 2026-11-05T09:00:00Z | last accepted -
marker no release | first taken 2026-11-03T09:00:00Z | last accepted 2026-11-05T09:00:00Z
task C-0001 | r1 | created 2026-11-01T09:00:00Z | taken 2026-11-03T09:00:00Z | accepted 2026-11-15T09:00:00Z | due 2026-11-10 | late 5d | Write the tide table foreword
task C-0003 | r2 | created 2026-11-04T09:00:00Z | taken 2026-11-05T09:00:00Z | accepted - | due 2026-11-12 | overdue 8d | Engrave the harbor map
task C-0004 | - | created 2026-11-02T09:00:00Z | taken 2026-11-03T09:00:00Z | accepted 2026-11-05T09:00:00Z | due - | - | Fix the index
task C-0005 | - | created 2026-11-06T09:00:00Z | taken - | accepted - | due - | - | Record the foghorn
";

/// The spec's pinned text of the scenario at 2026-11-20, line for line.
pub const DATA_TEXT: &str = "\
delivery | as of 2026-11-20 | tasks 4, delivered 2, late 1, open 2, overdue 1, cancelled 1 | lead median 3d, max 14d
release r1 | tasks 1, delivered 1, late 1, open 0, overdue 0, cancelled 1 | lead median 14d, max 14d
release r2 | tasks 1, delivered 0, late 0, open 1, overdue 1, cancelled 0 | lead -
no release | tasks 2, delivered 1, late 0, open 1, overdue 0, cancelled 0 | lead median 3d, max 3d
department words | Words | tasks 2, delivered 2, late 1, open 0, overdue 0, cancelled 0 | lead median 3d, max 14d
department print | Print | tasks 3, delivered 2, late 1, open 1, overdue 1, cancelled 1 | lead median 3d, max 14d
late C-0003 | plates/drawing | r2 | due 2026-11-12 | open | overdue 8d | Engrave the harbor map
late C-0001 | copy/done | r1 | due 2026-11-10 | accepted 2026-11-15T09:00:00Z | late 5d | Write the tide table foreword
  returned 2026-11-06T09:00:00Z | from review | Too long
  returned 2026-11-08T09:00:00Z | from review | Wrong tone
  re-estimated 2026-11-07T09:00:00Z | 3 -> 5 | The tables doubled
  subtask C-0002 cancelled 2026-11-09T09:00:00Z | Plates reused from last year
  item PR-0040 | question | applied
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
