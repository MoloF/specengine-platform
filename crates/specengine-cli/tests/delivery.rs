//! docs/features/delivery-analytics.md (RM-0107 a) "Acceptance criteria",
//! the CLI half: the `spec` binary over the spec's scenario (the shared
//! scene `tests/delivery_scene`, a scratch git repository with its queue
//! written through the library at fixed clocks), and over variants of it.
//!
//! - DA-1: C-0001 reordered 70 times in `todo`, moved to `doing`, then
//!   reordered 70 times there: the whole JSON is the scenario's, `taken_at`
//!   the move's. M: the events read through `card_history(…, 64)`.
//! - DA-2: the scenario's take past two reorders; a card accepted straight
//!   from its first column; a card only reordered. M: the first
//!   `card.moved` of any kind.
//! - DA-3: a card's release through its parent, its first target in plan
//!   order (both written orders), a done item, the pool, an unplaced item,
//!   no item: `none`; an unreadable parent ends the walk. M: the parent
//!   fallback dropped; the last target's group; done items ignored.
//! - DA-4: departments on the scenario; with no `[[workspace]]` one row
//!   `all` equal to `total`. M: `total` as the sum of the departments.
//! - DA-5: overdue against `as_of` (11-01, 11-12, 11-20), delivered late
//!   whatever `as_of`; one answer twice, at another root and `HOME`;
//!   without `--as-of` today's UTC date. M: the clock read when `--as-of`
//!   is given; `<=` for `<`.
//! - DA-6: no key holding `estimate` but a late row's, no key naming a
//!   person, no text line naming `Ann` or `Bob`. M: an `estimate_sum` key;
//!   `assignee` on a late row.
//! - DA-8: the scenario's reasons; a noted forward move and a noted reorder
//!   inside `returned` are no return; twenty returns list the oldest 16,
//!   `more.returns` 4, the text ending `  more returns 4`; a re-estimate to
//!   a clear. M: any move with a note taken as a return; the lists uncapped.
//! - DA-9: the spec's text and JSON byte for byte; no `[roadmap]`; an
//!   unreadable row; a bad `--as-of`; no queue; a newer queue. M: the upper
//!   middle as the median.
//!
//! docs/features/delivery-timeline.md (RM-0107 b) "Acceptance criteria",
//! the CLI half, `--timeline`:
//!
//! - DT1: the scenario's text is part a's pin then the timeline's lines,
//!   the JSON part a's with the timeline, each byte for byte (C-0004 taken
//!   by its move to `review`); group words for
//!   the pool, an unplaced item and no release; values on one line; past
//!   `OUTPUT_CAP_CHARS` whole timeline lines then `[truncated: <k> of <n>
//!   timeline tasks not shown]`, the JSON never cut. M: cancelled cards as
//!   rows; rows by `created_at` alone.
//! - DT4: without `--timeline` the text is part a's pin and the JSON that
//!   pin with `"timeline": null`. M: the timeline built whatever the flag.
//! - DT5: `from` 2026-11-01, `to` 2026-11-20, at `--as-of 2026-11-01` `to`
//!   2026-11-15; one text and JSON twice, at another root and `HOME`; no
//!   `--as-of`: today's UTC date (D3). M: `to` as `as_of` alone.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run of the read under test.

#![cfg(unix)]

mod common;
mod delivery_scene;

use std::fs;
use std::path::Path;

use common::git;
use common::{Run, copy_dir, spec};
use delivery_scene::{
    AS_OF, DATA_JSON, DATA_TEXT, DEPARTMENTS, Extra, FLOWS, HEAD, ROADMAP, Scene, TIMELINE_JSON,
    TIMELINE_TEXT, at, compact, data_timeline_json, db_under, item, new_card, put, sql_at,
};
use serde_json::{Value, json};
use specengine_cli::CardNewRequest;

/// `spec delivery args…` in the scene's root, under its `HOME`.
fn delivery(scene: &Scene, args: &[&str]) -> Run {
    let mut all = vec!["delivery"];
    all.extend_from_slice(args);
    spec(&scene.home, &scene.root, &all)
}

/// The JSON at `as_of`, which must answer 0.
fn document(scene: &Scene, as_of: &str) -> Value {
    let run = delivery(scene, &["--as-of", as_of, "--json"]);
    run.code(0);
    run.json()
}

/// A late row by ID.
fn late<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["late"]
        .as_array()
        .expect("late")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no late row {id}: {document}"))
}

/// The late rows' IDs, in served order.
fn late_ids(document: &Value) -> Vec<String> {
    document["late"]
        .as_array()
        .expect("late")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// The release rows as (group, release).
fn groups(document: &Value) -> Vec<(String, Value)> {
    document["releases"]
        .as_array()
        .expect("releases")
        .iter()
        .map(|row| {
            (
                row["group"].as_str().expect("a group").to_owned(),
                row["release"].clone(),
            )
        })
        .collect()
}

/// The scenario's pinned JSON as a value.
fn data() -> Value {
    serde_json::from_str(DATA_JSON).expect("the spec's JSON")
}

// ------------------------------------------------------------------ DA-9

/// DA-9: `spec delivery --as-of 2026-11-20` prints the pinned text byte for
/// byte, `--json` the pinned document byte for byte (compact, one line), no
/// note, exit 0; the median over 3 and 14 is 3. M: the upper middle as the
/// median.
#[test]
fn da9_the_scenario_prints_the_spec_byte_for_byte() {
    let scene = Scene::scenario("da9-pinned");
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    assert_eq!(text.stdout, DATA_TEXT, "{}", text.show());
    assert_eq!(text.stderr, "", "no note");
    let json = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    json.code(0);
    assert_eq!(
        json.stdout,
        format!("{}\n", compact(DATA_JSON)),
        "{}",
        json.show()
    );
    assert_eq!(json.stderr, "", "no note");
    assert_eq!(
        json.json()["total"]["lead_days"],
        json!({"median": 3, "max": 14})
    );
}

/// DA-9: no `[roadmap]` table: every row is `none`, the one note on
/// stderr and in `notes`, exit 0; the late rows' release `-` in the text.
#[test]
fn da9_no_roadmap_every_task_is_in_no_release() {
    let scene = Scene::with_config("da9-no-roadmap", &format!("{HEAD}{FLOWS}{DEPARTMENTS}"));
    scene.play(&Extra::default());
    let note = "delivery: no [roadmap] table: every task is in no release";
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    assert_eq!(run.stderr, format!("note: {note}\n"));
    let document = run.json();
    assert_eq!(document["notes"], json!([note]));
    let total = data()["total"].clone();
    let mut none = json!({"group": "none", "release": null});
    for (key, value) in total.as_object().expect("total") {
        none[key] = value.clone();
    }
    assert_eq!(document["releases"], json!([none]), "{document}");
    for id in ["C-0003", "C-0001"] {
        assert_eq!(late(&document, id)["group"], "none");
        assert_eq!(late(&document, id)["release"], Value::Null);
    }
    assert_eq!(document["departments"], data()["departments"]);
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    assert_eq!(text.stderr, format!("note: {note}\n"));
    let lines: Vec<&str> = text.stdout.lines().collect();
    assert_eq!(
        lines[1],
        "no release | tasks 4, delivered 2, late 1, open 2, overdue 1, cancelled 1 | lead median \
         3d, max 14d"
    );
    assert!(
        lines.contains(
            &"late C-0003 | plates/drawing | - | due 2026-11-12 | open | overdue 8d | Engrave the \
              harbor map"
        ),
        "{}",
        text.stdout
    );
}

/// DA-9: C-0005's row made unreadable in SQL: its note, it is not counted,
/// exit 0.
#[test]
fn da9_an_unreadable_row_is_a_note_and_not_counted() {
    let scene = Scene::scenario("da9-unreadable");
    scene.sql("UPDATE cards SET targets = 'not json' WHERE id = 'C-0005';");
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    let document = run.json();
    let notes: Vec<&str> = document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note"))
        .collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("C-0005: its row does not read (targets: ")
            && notes[0].ends_with("); not counted"),
        "{notes:?}"
    );
    assert_eq!(run.stderr, format!("note: {}\n", notes[0]));
    assert_eq!(document["total"]["tasks"], 3, "{document}");
    assert_eq!(document["total"]["open"], 1);
    let none = &document["releases"][2];
    assert_eq!(none["group"], "none");
    assert_eq!(none["tasks"], 1, "C-0004 alone: {none}");
    assert_eq!(none["open"], 0);
    assert_eq!(document["departments"], data()["departments"]);
}

/// DA-9: a bad `--as-of` is exit 2 naming it, nothing on stdout, nothing
/// read: the same line over a queue too new to read and outside any
/// project.
#[test]
fn da9_a_bad_as_of_is_exit_2_and_nothing_read() {
    let scene = Scene::scenario("da9-bad-as-of");
    for bad in [
        "2026-13-01",
        "2026-11-31",
        "20261120",
        "2026-11-20T09:00:00Z",
        "",
    ] {
        let run = delivery(&scene, &["--as-of", bad]);
        run.code(2);
        assert_eq!(run.stdout, "", "{bad:?}");
        assert!(
            run.stderr.contains(&format!(
                "`--as-of`: `{bad}` is no calendar date, YYYY-MM-DD"
            )),
            "{bad:?}: {}",
            run.show()
        );
    }
    // A queue a newer build wrote: exit 2 on its own, but a bad `--as-of`
    // is refused first, so nothing was read.
    scene.sql("PRAGMA user_version = 99;");
    let newer = delivery(&scene, &["--as-of", AS_OF]);
    newer.code(2);
    assert_eq!(newer.stdout, "");
    let bad = delivery(&scene, &["--as-of", "2026-13-01"]);
    bad.code(2);
    assert!(
        bad.stderr
            .contains("`--as-of`: `2026-13-01` is no calendar date, YYYY-MM-DD"),
        "{}",
        bad.show()
    );
    assert_ne!(bad.stderr, newer.stderr);
    let empty = scene.home.join("empty");
    fs::create_dir_all(&empty).expect("a directory");
    let outside = spec(&scene.home, &empty, &["delivery", "--as-of", "2026-13-01"]);
    outside.code(2);
    assert!(
        outside
            .stderr
            .contains("`--as-of`: `2026-13-01` is no calendar date, YYYY-MM-DD"),
        "{}",
        outside.show()
    );
    let nothing = spec(&scene.home, &empty, &["delivery", "--as-of", AS_OF]);
    nothing.code(2);
}

/// DA-9 (notes): no queue yet: the note, every count 0, every
/// department row there, exit 0, and no queue made; `"timeline": null`
/// (delivery-timeline DT4). With `--timeline`, no task: `from`, `to`
/// `null`, the text `timeline | - to - | tasks 0`.
#[test]
fn da9_no_queue_yet_is_a_note() {
    let scene = Scene::new("da9-no-queue");
    assert!(!scene.db().exists());
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    let note = "delivery: no queue yet: no card read";
    assert_eq!(run.stderr, format!("note: {note}\n"));
    let zeros = json!({"tasks": 0, "delivered": 0, "delivered_late": 0, "open": 0, "overdue": 0,
                       "cancelled": 0, "lead_days": null});
    let department = |name: &str, title: &str| {
        let mut row = json!({"name": name, "title": title});
        for (key, value) in zeros.as_object().expect("zeros") {
            row[key] = value.clone();
        }
        row
    };
    assert_eq!(
        run.json(),
        json!({"as_of": AS_OF, "notes": [note], "releases": [],
               "departments": [department("words", "Words"), department("print", "Print")],
               "total": zeros, "late": [], "timeline": null, "truncated": false,
               "omitted": null})
    );
    assert!(!scene.db().exists(), "the read made no queue");
    let json = delivery(&scene, &["--as-of", AS_OF, "--timeline", "--json"]);
    json.code(0);
    assert_eq!(
        json.json()["timeline"],
        json!({"from": null, "to": null, "markers": [], "tasks": []})
    );
    let text = delivery(&scene, &["--as-of", AS_OF, "--timeline"]);
    text.code(0);
    assert_eq!(
        text.stdout.lines().last(),
        Some("timeline | - to - | tasks 0"),
        "{}",
        text.stdout
    );
    assert!(!scene.db().exists(), "the read made no queue");
}

/// DA-9 (Exit): a queue a newer build wrote cannot be read: exit 2.
#[test]
fn da9_a_newer_queue_is_exit_2() {
    let scene = Scene::scenario("da9-newer");
    scene.sql("PRAGMA user_version = 99;");
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(!run.stderr.is_empty());
}

// ------------------------------------------------------------------ DA-1

/// DA-1: 70 reorders in `todo` before the take and 70 in `doing` after it
/// change nothing: the whole document is the scenario's, C-0001's
/// `taken_at` the move to `doing` (11-03), its returns both there. With
/// the events read through `card_history(…, 64)` the take falls outside
/// C-0001's last 64 `card.*` events.
#[test]
fn da1_the_events_read_is_uncapped() {
    let scene = Scene::scenario_with(
        "da1-uncapped",
        &Extra {
            reorders_before_take: 70,
            reorders_after_take: 70,
            ..Extra::default()
        },
    );
    let moved: i64 = scene
        .sql(
            "SELECT COUNT(*) FROM events WHERE type = 'card.moved' AND \
             json_extract(payload, '$.id') = 'C-0001';",
        )
        .parse()
        .expect("a count");
    assert_eq!(moved, 2 + 70 + 1 + 70 + 5, "the moves written");
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    assert_eq!(late(&run.json(), "C-0001")["taken_at"], at(3));
    assert_eq!(
        run.stdout,
        format!("{}\n", compact(DATA_JSON)),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------ DA-2

/// DA-2: C-0001's two reorders before 11-03 are no take; C-0006, accepted
/// straight from `sound`'s first column, is taken by the accept; C-0007,
/// only reordered, is never taken. M: the first `card.moved` of any kind.
#[test]
fn da2_a_reorder_is_no_take() {
    let scene = Scene::scenario("da2-take");
    let bell = scene.card(&CardNewRequest {
        reviewer: Some("Bob".to_owned()),
        due: Some("2026-11-05".to_owned()),
        ..new_card("Tune the bell", "sound", 1)
    });
    assert_eq!(bell, "C-0006");
    scene.accept("C-0006", &at(8));
    let gun = scene.card(&CardNewRequest {
        due: Some("2026-11-10".to_owned()),
        ..new_card("Ring the noon gun", "copy", 1)
    });
    assert_eq!(gun, "C-0007");
    scene.move_to("C-0007", "todo", None, &at(2));
    scene.move_to("C-0007", "todo", Some("Kept for the noon"), &at(3));
    let document = document(&scene, AS_OF);
    assert_eq!(late(&document, "C-0001")["taken_at"], at(3));
    let bell = late(&document, "C-0006");
    assert_eq!(bell["taken_at"], at(8), "{bell}");
    assert_eq!(bell["accepted_at"], at(8));
    assert_eq!(bell["column"], "done");
    assert_eq!(bell["days_late"], 3);
    assert_eq!(bell["lead_days"], 7);
    let gun = late(&document, "C-0007");
    assert_eq!(gun["taken_at"], Value::Null, "{gun}");
    assert_eq!(gun["column"], "todo");
    assert_eq!(gun["days_late"], 10);
    assert_eq!(gun["returns"], json!([]), "a noted reorder is no return");
    assert_eq!(
        late_ids(&document),
        ["C-0007", "C-0003", "C-0001", "C-0006"],
        "by days late, then ID"
    );
}

// ------------------------------------------------------------------ DA-3

/// The DA-3 corpus: the scenario's items plus RM-0003 (r2, shipped),
/// RM-0004 (the pool) and RM-0005 (`r9`, a release not listed).
fn da3_scene(label: &str) -> Scene {
    let scene = Scene::new(label);
    put(
        &scene.root,
        "docs/roadmap/RM-0003.md",
        &item("RM-0003", "The neap almanac", Some("r2"), "shipped"),
    );
    put(
        &scene.root,
        "docs/roadmap/RM-0004.md",
        &item("RM-0004", "The lantern notes", None, "draft"),
    );
    put(
        &scene.root,
        "docs/roadmap/RM-0005.md",
        &item("RM-0005", "The ferry times", Some("r9"), "draft"),
    );
    scene.commit("three items more");
    scene.play(&Extra::default());
    let overdue = |title: &str, targets: &[&str], parent: Option<&str>| {
        scene.card(&CardNewRequest {
            targets: targets.iter().map(|id| (*id).to_owned()).collect(),
            parent: parent.map(str::to_owned),
            due: Some("2026-11-10".to_owned()),
            ..new_card(title, "copy", 1)
        })
    };
    assert_eq!(
        overdue("Bind both", &["RM-0002", "RM-0001"], None),
        "C-0006"
    );
    assert_eq!(
        overdue("Bind both again", &["RM-0001", "RM-0002"], None),
        "C-0007"
    );
    assert_eq!(overdue("Reprint the neap", &["RM-0003"], None), "C-0008");
    assert_eq!(overdue("Trim the lanterns", &["RM-0004"], None), "C-0009");
    assert_eq!(overdue("Time the ferry", &["RM-0005"], None), "C-0010");
    assert_eq!(overdue("Polish a lantern", &[], Some("C-0009")), "C-0011");
    assert_eq!(overdue("Polish its glass", &[], Some("C-0011")), "C-0012");
    scene
}

/// DA-3: C-0002 (no target) counts under r1 through its parent; a card on
/// RM-0002 and RM-0001, in either written order, is r1; one on a done item
/// of r2 is r2; a pool item `pool`, a release not listed `unplaced`, a
/// parent's and a grandparent's pool item `pool`; no item up the chain
/// `none`. Rows in group order. M: the parent fallback dropped; the last
/// target's group; done items ignored.
#[test]
fn da3_a_cards_release() {
    let scene = da3_scene("da3-release");
    let document = document(&scene, AS_OF);
    assert_eq!(
        groups(&document),
        [
            ("release".to_owned(), json!("r1")),
            ("release".to_owned(), json!("r2")),
            ("pool".to_owned(), Value::Null),
            ("unplaced".to_owned(), Value::Null),
            ("none".to_owned(), Value::Null),
        ],
        "{document}"
    );
    let r1 = &document["releases"][0];
    assert_eq!(r1["cancelled"], 1, "C-0002 through its parent: {r1}");
    assert_eq!(r1["tasks"], 3, "C-0001, C-0006, C-0007: {r1}");
    assert_eq!(document["releases"][1]["tasks"], 2, "C-0003, C-0008");
    assert_eq!(
        document["releases"][2]["tasks"], 3,
        "C-0009, C-0011, C-0012"
    );
    assert_eq!(document["releases"][3]["tasks"], 1, "C-0010");
    assert_eq!(document["releases"][4]["tasks"], 2, "C-0004, C-0005");
    assert_eq!(document["releases"][4]["cancelled"], 0);
    for (id, group, release) in [
        ("C-0006", "release", json!("r1")),
        ("C-0007", "release", json!("r1")),
        ("C-0008", "release", json!("r2")),
        ("C-0009", "pool", Value::Null),
        ("C-0010", "unplaced", Value::Null),
        ("C-0011", "pool", Value::Null),
        ("C-0012", "pool", Value::Null),
    ] {
        let row = late(&document, id);
        assert_eq!(
            (&row["group"], &row["release"]),
            (&json!(group), &release),
            "{id}: {row}"
        );
    }
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    assert_eq!(
        lines[3],
        "pool | tasks 3, delivered 0, late 0, open 3, overdue 3, cancelled 0 | lead -"
    );
    assert_eq!(
        lines[4],
        "unplaced | tasks 1, delivered 0, late 0, open 1, overdue 1, cancelled 0 | lead -"
    );
    for line in [
        "late C-0009 | copy/todo | pool | due 2026-11-10 | open | overdue 10d | Trim the lanterns",
        "late C-0010 | copy/todo | unplaced | due 2026-11-10 | open | overdue 10d | Time the ferry",
    ] {
        assert!(lines.contains(&line), "{line}\n{}", text.stdout);
    }
}

/// DA-3: an unreadable parent ends the walk at `none`: C-0009's row made
/// unreadable, C-0011 and C-0012 fall to `none`, C-0009 counted nowhere.
#[test]
fn da3_an_unreadable_parent_ends_the_walk() {
    let scene = da3_scene("da3-unreadable");
    scene.sql("UPDATE cards SET targets = 'not json' WHERE id = 'C-0009';");
    let document = document(&scene, AS_OF);
    for id in ["C-0011", "C-0012"] {
        assert_eq!(late(&document, id)["group"], "none", "{id}");
    }
    assert!(!late_ids(&document).contains(&"C-0009".to_owned()));
    assert!(
        !groups(&document).contains(&("pool".to_owned(), Value::Null)),
        "no card left in the pool: {document}"
    );
}

// ------------------------------------------------------------------ DA-4

/// DA-4: `words` counts C-0001 and C-0004, `print` C-0001 to C-0004,
/// `total` counts C-0001 once and C-0005 (`sound`) only there; the rows
/// are the scenario's. M: `total` as the sum of the departments.
#[test]
fn da4_departments_and_the_total() {
    let scene = Scene::scenario("da4-departments");
    let document = document(&scene, AS_OF);
    assert_eq!(document["departments"], data()["departments"]);
    assert_eq!(document["total"], data()["total"]);
    assert_eq!(
        late(&document, "C-0001")["departments"],
        json!(["words", "print"])
    );
    assert_eq!(late(&document, "C-0003")["departments"], json!(["print"]));
}

/// DA-4: with no `[[workspace]]`, one department row, the implicit `all`,
/// its counts equal to `total`'s, every late row in it.
#[test]
fn da4_no_workspace_is_one_row_all() {
    let scene = Scene::with_config("da4-all", &format!("{HEAD}{ROADMAP}{FLOWS}"));
    scene.play(&Extra::default());
    let document = document(&scene, AS_OF);
    let rows = document["departments"].as_array().expect("departments");
    assert_eq!(rows.len(), 1, "{document}");
    assert_eq!(rows[0]["name"], "all");
    assert_eq!(rows[0]["title"], "All sections");
    for (key, value) in document["total"].as_object().expect("total") {
        assert_eq!(&rows[0][key], value, "{key}");
    }
    assert_eq!(document["total"], data()["total"]);
    for id in ["C-0003", "C-0001"] {
        assert_eq!(late(&document, id)["departments"], json!(["all"]));
    }
}

// ------------------------------------------------------------------ DA-5

/// DA-5: C-0003 (due 11-12) is overdue at 11-20 by 8, not at 11-01 nor at
/// 11-12; C-0001 is delivered late by 5 at every `as_of`; `as_of` is
/// printed and served as given. M: the clock read when `--as-of` is given;
/// `<=` for `<`.
#[test]
fn da5_as_of_judges_open_cards_only() {
    let scene = Scene::scenario("da5-as-of");
    for (as_of, overdue) in [("2026-11-01", false), ("2026-11-12", false), (AS_OF, true)] {
        let document = document(&scene, as_of);
        assert_eq!(document["as_of"], as_of);
        let ids = late_ids(&document);
        assert_eq!(
            ids.contains(&"C-0003".to_owned()),
            overdue,
            "{as_of}: {ids:?}"
        );
        assert_eq!(document["total"]["overdue"], u64::from(overdue), "{as_of}");
        assert_eq!(
            document["releases"][1]["overdue"],
            u64::from(overdue),
            "{as_of}"
        );
        let c1 = late(&document, "C-0001");
        assert_eq!(c1["days_late"], 5, "{as_of}");
        assert_eq!(document["total"]["delivered_late"], 1, "{as_of}");
        let text = delivery(&scene, &["--as-of", as_of]);
        text.code(0);
        assert!(
            text.stdout.starts_with(&format!(
                "delivery | as of {as_of} | tasks 4, delivered 2, late 1, "
            )),
            "{}",
            text.stdout
        );
    }
    assert_eq!(late(&document(&scene, AS_OF), "C-0003")["days_late"], 8);
    let later = document(&scene, "2027-01-01");
    assert_eq!(
        late(&later, "C-0003")["days_late"],
        50,
        "across a year's end"
    );
    assert_eq!(late(&later, "C-0001")["days_late"], 5);
}

/// DA-5: the same queue with `--as-of 2026-11-20` gives one text and one
/// JSON, twice and at another root and `HOME` (the repository and the
/// queue copied).
#[test]
fn da5_one_answer_at_any_root_and_home() {
    let scene = Scene::scenario("da5-roots");
    let runs = |home: &Path, root: &Path| {
        let text = spec(home, root, &["delivery", "--as-of", AS_OF]);
        text.code(0);
        let json = spec(home, root, &["delivery", "--as-of", AS_OF, "--json"]);
        json.code(0);
        (text.stdout, text.stderr, json.stdout, json.stderr)
    };
    let first = runs(&scene.home, &scene.root);
    assert_eq!(runs(&scene.home, &scene.root), first, "twice");
    let other = scene.home.parent().expect("the scene").join("elsewhere");
    let (root, home) = (other.join("copy"), other.join("home2"));
    copy_dir(&scene.root, &root);
    let source = db_under(&scene.home);
    let target = db_under(&home);
    fs::create_dir_all(target.parent().expect("a data dir")).expect("a data dir");
    sql_at(&source, "PRAGMA wal_checkpoint(TRUNCATE);");
    fs::copy(&source, &target).expect("the queue copied");
    assert_eq!(runs(&home, &root), first, "another root and HOME");
    assert_eq!(first.0, DATA_TEXT);
}

/// DA-5: without `--as-of`, `as_of` is the store's `today_utc()`, read
/// before or after the run, in the text's header too.
#[test]
fn da5_without_as_of_it_is_today() {
    let scene = Scene::scenario("da5-today");
    let before = specengine_store::today_utc();
    let run = delivery(&scene, &["--json"]);
    let text = delivery(&scene, &[]);
    let after = specengine_store::today_utc();
    run.code(0);
    text.code(0);
    let as_of = run.json()["as_of"].as_str().expect("as_of").to_owned();
    assert!(
        as_of == before || as_of == after,
        "{as_of} {before} {after}"
    );
    assert!(
        text.stdout
            .starts_with(&format!("delivery | as of {as_of} | "))
            || text
                .stdout
                .starts_with(&format!("delivery | as of {after} | ")),
        "{}",
        text.stdout
    );
}

// ------------------------------------------------------------------ DA-6

/// Every key of `value` with its path.
fn keys(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                out.push((path.to_owned(), key.clone()));
                keys(inner, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(items) => {
            for inner in items {
                keys(inner, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// DA-6: a walk of the scenario's JSON finds no key holding `estimate` but
/// a late row's `estimate` (and its `reestimates` lists, which hold the
/// word), none named `as`, `by`, `assignee`, `reviewer`, `created_by` or
/// `decided_by`; no line of the text holds `Ann` or `Bob`. M: an
/// `estimate_sum` key; `assignee` on a late row.
#[test]
fn da6_no_sum_no_person() {
    let scene = Scene::scenario("da6-no-person");
    let document = document(&scene, AS_OF);
    let mut found = Vec::new();
    keys(&document, "", &mut found);
    let estimates: Vec<&(String, String)> = found
        .iter()
        .filter(|(_, key)| key.contains("estimate"))
        .collect();
    for (path, key) in &estimates {
        let allowed = (path == ".late[]" && (key == "estimate" || key == "reestimates"))
            || (path == ".late[].more" && key == "reestimates");
        assert!(allowed, "`{key}` at `{path}`");
    }
    assert!(
        estimates
            .iter()
            .any(|(path, key)| path == ".late[]" && key == "estimate")
    );
    for (path, key) in &found {
        assert!(
            ![
                "as",
                "by",
                "assignee",
                "reviewer",
                "created_by",
                "decided_by"
            ]
            .contains(&key.as_str()),
            "`{key}` at `{path}`"
        );
    }
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    for line in text.stdout.lines().chain(text.stderr.lines()) {
        assert!(!line.contains("Ann") && !line.contains("Bob"), "{line}");
    }
    let raw = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    assert!(!raw.stdout.contains("Ann") && !raw.stdout.contains("Bob"));
}

// ------------------------------------------------------------------ DA-8

/// DA-8: a noted move from `doing` to `review` and a noted reorder inside
/// `returned` are no returns: the document is the scenario's. M: any move
/// with a note taken as a return.
#[test]
fn da8_a_noted_move_is_no_return() {
    let scene = Scene::scenario_with(
        "da8-noted",
        &Extra {
            forward_note: Some("Ready for a read"),
            reorder_in_return: Some("Still too long"),
            ..Extra::default()
        },
    );
    let notes = scene.sql(
        "SELECT COUNT(*) FROM events WHERE type = 'card.moved' AND \
         json_extract(payload, '$.note') IS NOT NULL;",
    );
    assert_eq!(notes, "4", "two returns and two noted moves written");
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    assert_eq!(
        late(&run.json(), "C-0001")["returns"],
        data()["late"][1]["returns"]
    );
    assert_eq!(
        run.stdout,
        format!("{}\n", compact(DATA_JSON)),
        "{}",
        run.show()
    );
}

/// DA-8: twenty returns list the oldest 16, `more.returns` 4, the text's
/// late row ending `  more returns 4`; a re-estimate to a clear is `new`
/// `null`, the late row's `estimate` `null`, the text `3 -> -`. M: the
/// lists uncapped.
#[test]
fn da8_twenty_returns_list_the_oldest_16() {
    let scene = Scene::new("da8-twenty");
    let id = scene.card(&CardNewRequest {
        estimate: Some("3".to_owned()),
        due: Some("2026-11-10".to_owned()),
        reviewer: Some("Bob".to_owned()),
        ..new_card("Set the long table", "copy", 1)
    });
    assert_eq!(id, "C-0001");
    scene.move_to(&id, "review", None, &at(2));
    for n in 1..=20 {
        scene.move_to(&id, "returned", Some(&format!("Return {n}")), &at(3));
        scene.move_to(&id, "review", None, &at(3));
    }
    scene.reestimate(&id, None, "Unknown now", &at(4));
    scene.accept(&id, &at(12));
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    let row = late(&run.json(), "C-0001").clone();
    let returns = row["returns"].as_array().expect("returns");
    assert_eq!(returns.len(), 16);
    for (n, entry) in returns.iter().enumerate() {
        assert_eq!(
            entry,
            &json!({"at": at(3), "from": "review", "note": format!("Return {}", n + 1)})
        );
    }
    assert_eq!(
        row["more"],
        json!({"returns": 4, "reestimates": 0, "cancelled_subtasks": 0, "items": 0})
    );
    assert_eq!(
        row["reestimates"],
        json!([{"at": at(4), "old": "3", "new": null, "reason": "Unknown now"}])
    );
    assert_eq!(row["estimate"], Value::Null);
    assert_eq!(row["days_late"], 2);
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    assert!(
        text.stdout.ends_with("\n  more returns 4\n"),
        "{}",
        text.stdout
    );
    let lines: Vec<&str> = text.stdout.lines().collect();
    assert!(
        lines.contains(&format!("  re-estimated {} | 3 -> - | Unknown now", at(4)).as_str()),
        "{}",
        text.stdout
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("  returned "))
            .count(),
        16
    );
    assert_eq!(
        lines[lines.len() - 2],
        format!("  re-estimated {} | 3 -> - | Unknown now", at(4))
    );
}

// ------------------------------------------- DA-9: notes, one line, the cut

/// The notes of a run's JSON.
fn notes_of(document: &Value) -> Vec<String> {
    document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

/// DA-9 (notes): a `[roadmap]` that does not read, a `[card]`
/// that does not read and a workspace naming a flow no one configured,
/// each its note, sorted, each once, on stderr as `note: <n>` in the same
/// order, exit 0; with no `[card]` no return is counted, and every task is
/// in no release.
#[test]
fn da9_config_notes_sorted_each_once() {
    let scene = Scene::scenario("da9-config-notes");
    let broken = format!(
        "{HEAD}{}{}{}",
        ROADMAP.replace("releases   = [\"r1\", \"r2\"]", "releases   = []"),
        FLOWS.replacen(
            "return = \"returned\"",
            "return = \"returned\"\ncolour = \"red\"",
            1
        ),
        DEPARTMENTS.replace(
            "flows = [\"copy\", \"plates\"]",
            "flows = [\"copy\", \"plates\", \"inks\"]"
        ),
    );
    put(&scene.root, "specengine.toml", &broken);
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    let document = run.json();
    let notes = notes_of(&document);
    let mut sorted = notes.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(notes, sorted, "sorted, each once");
    assert_eq!(
        run.stderr,
        notes
            .iter()
            .map(|note| format!("note: {note}\n"))
            .collect::<String>()
    );
    assert!(
        notes.contains(&"delivery: [card] does not read: no return is counted".to_owned()),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|note| note
            .starts_with("delivery: [roadmap] does not read: specengine.toml:")
            && note.ends_with("; every task is in no release")),
        "{notes:?}"
    );
    assert_eq!(
        notes.len(),
        3,
        "the roadmap's, the card's, the workspaces': {notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|note| note.starts_with("specengine.toml:")),
        "the workspaces' note as `spec workspace` prints it: {notes:?}"
    );
    let workspace = spec(&scene.home, &scene.root, &["workspace", "--json"]);
    workspace.code(0);
    for note in notes_of(&workspace.json()) {
        assert!(notes.contains(&note), "{note} in {notes:?}");
    }
    assert_eq!(late(&document, "C-0001")["returns"], json!([]));
    assert_eq!(late(&document, "C-0001")["release"], Value::Null);
    assert_eq!(groups(&document), [("none".to_owned(), Value::Null)]);
}

/// DA-9 (one line): a value passes `one_line`: a title holding a line feed
/// prints on one line in the text, as written in the JSON.
#[test]
fn da9_values_print_on_one_line() {
    let scene = Scene::scenario("da9-one-line");
    let id = scene.card(&CardNewRequest {
        due: Some("2026-11-10".to_owned()),
        ..new_card("Ring the\nnoon bell", "copy", 1)
    });
    let json = document(&scene, AS_OF);
    assert_eq!(late(&json, &id)["title"], "Ring the\nnoon bell");
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    let line = text
        .stdout
        .lines()
        .find(|line| line.starts_with(&format!("late {id} | ")))
        .unwrap_or_else(|| panic!("{}", text.stdout));
    assert!(line.ends_with(" | Ring the noon bell"), "{line}");
}

/// DA-9 (the cut): 200 overdue cards with long titles pass
/// `OUTPUT_CAP_CHARS`: the header, group and department lines whole, then
/// whole late rows while they fit, then `[truncated: <k> of <n> late tasks
/// not shown]`; the JSON lists the same rows, `truncated` true, `omitted`
/// k.
#[test]
fn da9_the_cut_keeps_whole_late_rows() {
    let scene = Scene::new("da9-cut");
    let title = |n: usize| {
        format!(
            "Chart {n:03} {}",
            "of the long harbor wall ".repeat(8).trim_end()
        )
    };
    for n in 1..=200 {
        let id = scene.card(&CardNewRequest {
            due: Some("2026-11-10".to_owned()),
            ..new_card(&title(n), "copy", 1)
        });
        assert_eq!(id, format!("C-{n:04}"));
    }
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let last = lines.last().expect("a line");
    let shown = lines
        .iter()
        .filter(|line| line.starts_with("late "))
        .count();
    assert!(shown > 0 && shown < 200, "{shown}");
    assert_eq!(
        *last,
        format!("[truncated: {} of 200 late tasks not shown]", 200 - shown)
    );
    let body: usize = lines[..lines.len() - 1]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(body <= specengine_cli::OUTPUT_CAP_CHARS, "{body}");
    let next = format!(
        "late C-{:04} | copy/todo | - | due 2026-11-10 | open | overdue 10d | {}",
        shown + 1,
        title(shown + 1)
    );
    assert!(
        body + next.chars().count() + 1 > specengine_cli::OUTPUT_CAP_CHARS,
        "the next row would have fit: {body}"
    );
    for (n, line) in lines
        .iter()
        .filter(|line| line.starts_with("late "))
        .enumerate()
    {
        assert!(line.ends_with(&title(n + 1)), "a whole row: {line}");
    }
    let json = delivery(&scene, &["--as-of", AS_OF, "--json"]).json();
    assert_eq!(json["truncated"], true);
    assert_eq!(json["omitted"], 200 - shown);
    assert_eq!(json["late"].as_array().expect("late").len(), shown);
    assert_eq!(json["total"]["overdue"], 200, "the counts are uncut");
}

/// DA-9 (notes), iteration 2: an item row that does not decode is named,
/// `<PR>: its row does not read (<column>: <reason>); not listed`, and left
/// out of C-0001's items; every other key is the scenario's; exit 0.
#[test]
fn da9_an_unreadable_item_is_a_note_and_not_listed() {
    let scene = Scene::scenario("da9-item");
    scene.sql("UPDATE proposals SET card_id = 'X-7' WHERE id = 'PR-0040';");
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    let mut document = run.json();
    let notes = notes_of(&document);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("PR-0040: its row does not read (card_id: ")
            && notes[0].ends_with("); not listed"),
        "{notes:?}"
    );
    assert_eq!(run.stderr, format!("note: {}\n", notes[0]));
    let mut expected = data();
    expected["late"][1]["items"] = json!([]);
    expected["notes"] = json!(notes);
    document["notes"] = json!(notes);
    assert_eq!(document, expected);
}

/// Sets `path`'s mode bits.
fn chmod(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
}

/// DA-9 (notes), iterations 2 and 3: a plan file under `[roadmap] dir`
/// the walk does not read is named and its item places no task: RM-0002's
/// front matter broken (C-0003 falls to no release), RM-0001's file
/// unreadable (C-0001 and, through it, C-0002 too); a directory inside the
/// plan dir not listed is named too; a directory outside it (`docs/closed`)
/// is the check's alone, no note. Each note once, sorted, exit 0.
#[test]
fn da9_a_plan_place_not_read_is_a_note() {
    let scene = Scene::scenario("da9-plan");
    put(
        &scene.root,
        "docs/roadmap/RM-0002.md",
        "---\nclass: spec\nid: RM-0002\nstatus: draft\nrelease: [r2\n---\n\n# The harbor \
         charts\n\nThe goal.\n",
    );
    let unreadable = scene.root.join("docs/roadmap/RM-0001.md");
    let closed = scene.root.join("docs/closed");
    let archive = scene.root.join("docs/roadmap/archive");
    put(&scene.root, "docs/closed/notes.md", "# Notes\n\nNone.\n");
    put(
        &scene.root,
        "docs/roadmap/archive/notes.md",
        "# Notes\n\nNone.\n",
    );
    chmod(&unreadable, 0o000);
    chmod(&closed, 0o000);
    chmod(&archive, 0o000);
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    chmod(&unreadable, 0o644);
    chmod(&closed, 0o755);
    chmod(&archive, 0o755);
    run.code(0);
    let document = run.json();
    let notes = notes_of(&document);
    assert_eq!(notes.len(), 3, "{notes:?}");
    assert!(
        notes[0].starts_with("delivery: plan docs/roadmap/RM-0001.md does not read (")
            && notes[0].ends_with("); its item places no task"),
        "{notes:?}"
    );
    assert!(
        notes[1].starts_with("delivery: plan docs/roadmap/RM-0002.md does not read (line ")
            && notes[1].ends_with("); its item places no task"),
        "{notes:?}"
    );
    assert_eq!(
        notes[2],
        "delivery: plan docs/roadmap/archive does not read (the directory cannot be listed); its \
         items place no task"
    );
    assert!(
        notes.iter().all(|note| !note.contains("docs/closed")),
        "outside the plan dir: {notes:?}"
    );
    assert_eq!(
        run.stderr,
        notes
            .iter()
            .map(|note| format!("note: {note}\n"))
            .collect::<String>()
    );
    assert_eq!(groups(&document), [("none".to_owned(), Value::Null)]);
    for id in ["C-0003", "C-0001"] {
        assert_eq!(late(&document, id)["group"], "none", "{id}");
    }
    assert_eq!(
        document["releases"][0]["cancelled"], 1,
        "C-0002 with its parent"
    );
}

/// DA-9 (notes), iteration 2: a garbled item row is named only when the
/// queue holds a card: with none, nothing past the card list is read and
/// no note is given.
#[test]
fn da9_an_unreadable_item_with_no_card_is_not_read() {
    let scene = Scene::new("da9-item-no-card");
    let asked = scene.question("RM-0001", "Is the almanac bound in cloth?", None);
    scene.sql(&format!(
        "UPDATE proposals SET card_id = 'X-7' WHERE id = '{asked}';"
    ));
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    assert_eq!(run.stderr, "");
    assert_eq!(run.json()["notes"], json!([]));
    assert_eq!(run.json()["total"]["tasks"], 0);
}

/// DA-9 (notes), iteration 3: a written `[paths] roots` entry naming no
/// directory and no `.md` file, outside `[roadmap] dir`, is the check's
/// alone: no note; the document is the scenario's. M: a note for every
/// walk problem.
#[test]
fn da9_a_missing_root_outside_the_plan_dir_gives_no_note() {
    let scene = Scene::scenario("da9-root");
    let config =
        delivery_scene::config().replace("roots = [\"docs\"]", "roots = [\"docs\", \"atlas\"]");
    put(&scene.root, "specengine.toml", &config);
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    assert_eq!(run.stderr, "");
    assert_eq!(
        run.stdout,
        format!("{}\n", compact(DATA_JSON)),
        "{}",
        run.show()
    );
}

/// DA-8, iteration 2: a return whose stored `note` is no text reads as no
/// note; the return is kept, the document otherwise the scenario's.
#[test]
fn da8_a_note_that_is_no_text_is_no_note() {
    let scene = Scene::scenario("da8-note-type");
    scene.sql(
        "UPDATE events SET payload = json_set(payload, '$.note', 7) WHERE type = 'card.moved' \
         AND json_extract(payload, '$.note') = 'Too long';",
    );
    let run = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    run.code(0);
    let mut expected = data();
    expected["late"][1]["returns"][0]["note"] = Value::Null;
    assert_eq!(run.json(), expected);
}

// ----------------------------------------------------- delivery-timeline

/// The JSON with `--timeline` at `as_of`, which must answer 0.
fn timeline_document(scene: &Scene, as_of: &str) -> Value {
    let run = delivery(scene, &["--as-of", as_of, "--timeline", "--json"]);
    run.code(0);
    run.json()
}

/// The timeline's rows' IDs, in served order.
fn timeline_ids(document: &Value) -> Vec<String> {
    document["timeline"]["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// DT1: `spec delivery --as-of 2026-11-20 --timeline` prints part a's pin,
/// then the timeline's lines, byte for byte; `--json` part a's document
/// with the timeline in place of `null`, byte for byte (C-0004 taken by its
/// move to `review`); no note, exit 0. M:
/// cancelled cards as rows; rows by `created_at` alone.
#[test]
fn dt1_the_scenarios_timeline_byte_for_byte() {
    let scene = Scene::scenario("dt1-pinned");
    let text = delivery(&scene, &["--as-of", AS_OF, "--timeline"]);
    text.code(0);
    assert_eq!(
        text.stdout,
        format!("{DATA_TEXT}{TIMELINE_TEXT}"),
        "{}",
        text.show()
    );
    assert_eq!(text.stderr, "", "no note");
    let json = delivery(&scene, &["--as-of", AS_OF, "--timeline", "--json"]);
    json.code(0);
    assert_eq!(
        json.stdout,
        format!("{}\n", compact(&data_timeline_json())),
        "{}",
        json.show()
    );
    assert_eq!(json.stderr, "", "no note");
    let document = json.json();
    assert_eq!(
        timeline_ids(&document),
        ["C-0001", "C-0003", "C-0004", "C-0005"]
    );
    let expected: Value = serde_json::from_str(TIMELINE_JSON).expect("the timeline");
    assert_eq!(document["timeline"], expected);
    // The flag before `--as-of` reads the same.
    let flipped = delivery(&scene, &["--timeline", "--json", "--as-of", AS_OF]);
    flipped.code(0);
    assert_eq!(flipped.stdout, json.stdout);
}

/// DT4: without `--timeline` the text is part a's pin byte for byte and the
/// JSON that pin with `"timeline": null` after `late`. M: the timeline
/// built whatever the flag.
#[test]
fn dt4_without_the_flag_it_is_part_as_document() {
    let scene = Scene::scenario("dt4-off");
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    assert_eq!(text.stdout, DATA_TEXT, "{}", text.show());
    assert!(!text.stdout.contains("timeline"), "{}", text.stdout);
    let json = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    json.code(0);
    assert_eq!(json.stdout, format!("{}\n", compact(DATA_JSON)));
    assert!(
        json.stdout.contains("\"late\":[{\"id\":\"C-0003\"")
            && json
                .stdout
                .contains("}],\"timeline\":null,\"truncated\":false,\"omitted\":null}"),
        "{}",
        json.stdout
    );
}

/// DT5: `from` the first creation's day, `to` 2026-11-20 at 2026-11-20 and
/// 2026-11-15 (C-0001's accept) at 2026-11-01; one text and one JSON,
/// twice, at another root and `HOME` (the repository and the queue copied).
/// M: `to` as `as_of` alone.
#[test]
fn dt5_the_axis_at_any_root_and_home() {
    let scene = Scene::scenario("dt5-roots");
    for (as_of, to) in [(AS_OF, AS_OF), ("2026-11-01", "2026-11-15")] {
        let timeline = &timeline_document(&scene, as_of)["timeline"];
        assert_eq!(timeline["from"], "2026-11-01", "{as_of}");
        assert_eq!(timeline["to"], to, "{as_of}");
        let text = delivery(&scene, &["--as-of", as_of, "--timeline"]);
        text.code(0);
        assert!(
            text.stdout
                .contains(&format!("\ntimeline | 2026-11-01 to {to} | tasks 4\n")),
            "{}",
            text.stdout
        );
    }
    let runs = |home: &Path, root: &Path| {
        let mut out = Vec::new();
        for as_of in [AS_OF, "2026-11-01"] {
            let text = spec(home, root, &["delivery", "--as-of", as_of, "--timeline"]);
            text.code(0);
            let json = spec(
                home,
                root,
                &["delivery", "--as-of", as_of, "--timeline", "--json"],
            );
            json.code(0);
            out.push((text.stdout, text.stderr, json.stdout, json.stderr));
        }
        out
    };
    let first = runs(&scene.home, &scene.root);
    assert_eq!(runs(&scene.home, &scene.root), first, "twice");
    let other = scene.home.parent().expect("the scene").join("elsewhere");
    let (root, home) = (other.join("copy"), other.join("home2"));
    copy_dir(&scene.root, &root);
    let source = db_under(&scene.home);
    let target = db_under(&home);
    fs::create_dir_all(target.parent().expect("a data dir")).expect("a data dir");
    sql_at(&source, "PRAGMA wal_checkpoint(TRUNCATE);");
    fs::copy(&source, &target).expect("the queue copied");
    assert_eq!(runs(&home, &root), first, "another root and HOME");
    assert_eq!(first[0].0, format!("{DATA_TEXT}{TIMELINE_TEXT}"));
}

/// DT5 (D3): the CLI keeps part a's default with `--timeline`: no
/// `--as-of`, today's UTC date, and `to` never before it.
#[test]
fn dt5_without_as_of_the_timeline_is_today() {
    let scene = Scene::scenario("dt5-today");
    let before = specengine_store::today_utc();
    let run = delivery(&scene, &["--timeline", "--json"]);
    let after = specengine_store::today_utc();
    run.code(0);
    let document = run.json();
    let as_of = document["as_of"].as_str().expect("as_of").to_owned();
    assert!(
        as_of == before || as_of == after,
        "{as_of} {before} {after}"
    );
    let to = document["timeline"]["to"].as_str().expect("a day");
    assert!(to >= as_of.as_str() && to >= "2026-11-15", "{to} {as_of}");
    assert_eq!(document["timeline"]["from"], "2026-11-01");
}

/// DT1: the group words of the text: a marker `release <name>`, `pool`,
/// `unplaced`, `no release`; a task's release its name, `pool`,
/// `unplaced`, `-`; markers in group order; rows by group, then
/// `created_at`, then ID number.
#[test]
fn dt1_group_words_in_the_text() {
    let scene = da3_scene("dt1-groups");
    let document = timeline_document(&scene, AS_OF);
    assert_eq!(
        timeline_ids(&document),
        [
            "C-0001", "C-0006", "C-0007", "C-0008", "C-0003", "C-0009", "C-0011", "C-0012",
            "C-0010", "C-0004", "C-0005"
        ],
        "{}",
        document["timeline"]
    );
    let text = delivery(&scene, &["--as-of", AS_OF, "--timeline"]);
    text.code(0);
    let lines: Vec<&str> = text
        .stdout
        .lines()
        .skip_while(|line| !line.starts_with("timeline | "))
        .collect();
    assert_eq!(lines[0], "timeline | 2026-11-01 to 2026-11-20 | tasks 11");
    let markers: Vec<&str> = lines[1..6]
        .iter()
        .map(|line| line.split(" | ").next().expect("a head"))
        .collect();
    assert_eq!(
        markers,
        [
            "marker release r1",
            "marker release r2",
            "marker pool",
            "marker unplaced",
            "marker no release"
        ],
        "{}",
        text.stdout
    );
    assert_eq!(
        lines[3], "marker pool | first taken - | last accepted -",
        "C-0009, C-0011, C-0012 untaken"
    );
    for line in [
        "task C-0006 | r1 | created 2026-11-01T09:00:00Z | taken - | accepted - | due 2026-11-10 | overdue 10d | Bind both",
        "task C-0009 | pool | created 2026-11-01T09:00:00Z | taken - | accepted - | due 2026-11-10 | overdue 10d | Trim the lanterns",
        "task C-0010 | unplaced | created 2026-11-01T09:00:00Z | taken - | accepted - | due 2026-11-10 | overdue 10d | Time the ferry",
        "task C-0005 | - | created 2026-11-06T09:00:00Z | taken - | accepted - | due - | - | Record the foghorn",
    ] {
        assert!(lines.contains(&line), "{line}\n{}", text.stdout);
    }
    assert_eq!(lines.len(), 1 + 5 + 11, "{}", text.stdout);
}

/// DT1: a title with a line break prints on one line in a task line, as
/// written in the JSON.
#[test]
fn dt1_values_print_on_one_line() {
    let scene = Scene::scenario("dt1-one-line");
    let id = scene.card(&new_card("Ring the\nnoon bell", "copy", 1));
    let json = timeline_document(&scene, AS_OF);
    let row = json["timeline"]["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .find(|row| row["id"] == id.as_str())
        .unwrap_or_else(|| panic!("no row {id}: {json}"))
        .clone();
    assert_eq!(row["title"], "Ring the\nnoon bell");
    let text = delivery(&scene, &["--as-of", AS_OF, "--timeline"]);
    text.code(0);
    let line = text
        .stdout
        .lines()
        .find(|line| line.starts_with(&format!("task {id} | ")))
        .unwrap_or_else(|| panic!("{}", text.stdout));
    assert_eq!(
        line,
        format!(
            "task {id} | - | created 2026-11-01T09:00:00Z | taken - | accepted - | due - | - | \
             Ring the noon bell"
        )
    );
}

/// DT1 (the cut): 200 tasks with long titles and no due day pass
/// `OUTPUT_CAP_CHARS`: part a's lines whole, the timeline's head and marker
/// whole, then whole task lines while they fit, then `[truncated: <k> of
/// <n> timeline tasks not shown]`; the JSON lists every row, `truncated`
/// false and `omitted` `null` (they count late rows alone).
#[test]
fn dt1_the_cut_keeps_whole_timeline_lines() {
    let scene = Scene::new("dt1-cut");
    let title = |n: usize| {
        format!(
            "Chart {n:03} {}",
            "of the long harbor wall ".repeat(8).trim_end()
        )
    };
    for n in 1..=200 {
        let id = scene.card(&new_card(&title(n), "copy", 1));
        assert_eq!(id, format!("C-{n:04}"));
    }
    let text = delivery(&scene, &["--as-of", AS_OF, "--timeline"]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let last = lines.last().expect("a line");
    let shown = lines
        .iter()
        .filter(|line| line.starts_with("task "))
        .count();
    assert!(shown > 0 && shown < 200, "{shown}");
    assert_eq!(
        *last,
        format!(
            "[truncated: {} of 200 timeline tasks not shown]",
            200 - shown
        )
    );
    assert!(
        lines.contains(&"timeline | 2026-11-01 to 2026-11-20 | tasks 200"),
        "{}",
        text.stdout
    );
    assert!(
        lines.contains(&"marker no release | first taken - | last accepted -"),
        "{}",
        text.stdout
    );
    assert!(
        !lines.iter().any(|line| line.starts_with("late ")),
        "no late row"
    );
    let body: usize = lines[..lines.len() - 1]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(body <= specengine_cli::OUTPUT_CAP_CHARS, "{body}");
    let row = |n: usize| {
        format!(
            "task C-{n:04} | - | created 2026-11-01T09:00:00Z | taken - | accepted - | due - | - \
             | {}",
            title(n)
        )
    };
    assert!(
        body + row(shown + 1).chars().count() + 1 > specengine_cli::OUTPUT_CAP_CHARS,
        "the next row would have fit: {body}"
    );
    for (n, line) in lines
        .iter()
        .filter(|line| line.starts_with("task "))
        .enumerate()
    {
        assert_eq!(*line, row(n + 1), "a whole row in order");
    }
    let json = timeline_document(&scene, AS_OF);
    assert_eq!(json["truncated"], false);
    assert_eq!(json["omitted"], Value::Null);
    assert_eq!(
        json["timeline"]["tasks"].as_array().expect("tasks").len(),
        200,
        "the JSON is never cut"
    );
}

/// DT1 (part a's D6, no person, no estimate): the timeline holds no key
/// naming a person and none holding `estimate`; neither its text nor its
/// JSON names `Ann` or `Bob`.
#[test]
fn dt1_the_timeline_names_no_person_and_no_estimate() {
    let scene = Scene::scenario("dt1-no-person");
    let document = timeline_document(&scene, AS_OF);
    let mut found = Vec::new();
    keys(&document["timeline"], ".timeline", &mut found);
    assert!(!found.is_empty());
    for (path, key) in &found {
        assert!(
            !key.contains("estimate")
                && ![
                    "as",
                    "by",
                    "assignee",
                    "reviewer",
                    "created_by",
                    "decided_by"
                ]
                .contains(&key.as_str()),
            "`{key}` at `{path}`"
        );
    }
    for args in [
        vec!["--as-of", AS_OF, "--timeline"],
        vec!["--as-of", AS_OF, "--timeline", "--json"],
    ] {
        let run = delivery(&scene, &args);
        run.code(0);
        let timeline = run
            .stdout
            .find("timeline")
            .map_or("", |at| &run.stdout[at..]);
        assert!(!timeline.is_empty(), "{}", run.stdout);
        assert!(
            !timeline.contains("Ann") && !timeline.contains("Bob"),
            "{timeline}"
        );
    }
}

/// DT1 (iteration 2): C-0005 (open) and C-0004 (delivered) made to hold a
/// `created_at` that does not read, in SQL, under a config with no
/// `[roadmap]`. With `--timeline` C-0005 is noted ``C-0005: `<created_at>`
/// is no timestamp; not on the timeline``, one line and escaped as part a's
/// notes, sorted before the config's `delivery:` note; C-0004 keeps part
/// a's one `no lead time` note; neither has a row, both still count. Without
/// `--timeline` no such note. M: the note dropped.
#[test]
fn dt1_an_open_card_off_the_timeline_is_noted() {
    let scene = Scene::with_config("dt1-off-timeline", &format!("{HEAD}{FLOWS}{DEPARTMENTS}"));
    scene.play(&Extra::default());
    scene.sql(
        "UPDATE cards SET created_at = 'yester' || char(10) || 'day' || char(7) WHERE id = \
         'C-0005'; UPDATE cards SET created_at = 'last week' WHERE id = 'C-0004';",
    );
    let part_a = "C-0004: last week is no timestamp; no lead time";
    let off = "C-0005: `yester day\u{7}` is no timestamp; not on the timeline";
    let config = "delivery: no [roadmap] table: every task is in no release";
    let stderr = |notes: &[&str]| -> String {
        notes
            .iter()
            .map(|note| format!("note: {}\n", note.replace('\u{7}', "\\u{7}")))
            .collect()
    };

    let with = delivery(&scene, &["--as-of", AS_OF, "--timeline", "--json"]);
    with.code(0);
    let document = with.json();
    assert_eq!(notes_of(&document), [part_a, off, config], "{document}");
    assert_eq!(with.stderr, stderr(&[part_a, off, config]));
    assert_eq!(
        timeline_ids(&document),
        ["C-0001", "C-0003"],
        "{}",
        document["timeline"]
    );
    assert_eq!(document["total"]["tasks"], 4, "both still count");
    let text = delivery(&scene, &["--as-of", AS_OF, "--timeline"]);
    text.code(0);
    assert_eq!(text.stderr, with.stderr);
    assert!(
        text.stdout
            .contains("\ntimeline | 2026-11-01 to 2026-11-20 | tasks 2\n"),
        "{}",
        text.stdout
    );

    let without = delivery(&scene, &["--as-of", AS_OF, "--json"]);
    without.code(0);
    let plain = without.json();
    assert_eq!(notes_of(&plain), [part_a, config], "{plain}");
    assert_eq!(without.stderr, stderr(&[part_a, config]));
    assert_eq!(plain["timeline"], Value::Null);
    let mut rest = document.clone();
    rest["timeline"] = Value::Null;
    rest["notes"] = plain["notes"].clone();
    assert_eq!(rest, plain, "the flag adds the timeline and its note alone");
    let text = delivery(&scene, &["--as-of", AS_OF]);
    text.code(0);
    assert_eq!(text.stderr, without.stderr);
}
