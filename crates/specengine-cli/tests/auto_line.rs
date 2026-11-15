//! docs/features/auto-land.md LD9 (a line binds once): `spec task new`
//! under `SPECENGINE_AUTO_LINE=<pack>/<n>`, over a scratch copy of
//! `fixtures/auto-mini` with `rules/on.toml` committed and a pack of two
//! lines recorded through the store.
//!
//! Mutation: binding without checking the line unbound.

#![cfg(unix)]

mod auto_common;
mod common;

use std::ffi::OsStr;

use auto_common::Auto;
use serde_json::{Value, json};

fn line_env(value: &str) -> [(&'static str, &OsStr); 1] {
    [("SPECENGINE_AUTO_LINE", OsStr::new(value))]
}

/// LD9. M: binding without checking the line unbound.
#[test]
fn ld9_a_line_binds_once() {
    let auto = Auto::with_rule("ld9", "on.toml");
    let pack = auto.pack(&[], &["Weigh the items", "Sort them"]);
    let value = format!("{pack}/1");

    // The first task binds.
    let run = auto.run(
        &auto.main,
        &["task", "new", "--nodes", "R-01", "--title", "Weigh"],
        &line_env(&value),
    );
    run.code(0);
    let first = auto.events_of_kind("task.created").pop().unwrap().json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        run.stdout.contains(&format!("created {first}"))
            && run.stdout.contains(&format!("bound to pack {pack} line 1")),
        "{}",
        run.show()
    );
    let created = auto.events_of_kind("task.created").pop().unwrap();
    assert_eq!(
        created.json(),
        json!({"id": first}),
        "task.created stays {{id}}"
    );
    let bound = auto.events_of_kind("auto.bound");
    assert_eq!(bound.len(), 1);
    assert_eq!(
        bound[0].payload,
        format!("{{\"pack\":{pack},\"line\":1,\"task\":\"{first}\"}}")
    );
    assert_eq!(
        bound[0].seq,
        created.seq + 1,
        "in the same transaction, after"
    );

    // A second task, the same value: unbound, the note.
    let run = auto.task_new(&auto.main, &["R-01"], "Weigh again", &line_env(&value));
    run.code(0);
    let doc = run.json();
    let second = doc["id"].as_str().unwrap().to_owned();
    let notes: Vec<&str> = doc["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        notes.contains(
            &format!(
                "SPECENGINE_AUTO_LINE `{value}`: line {pack}/1 is bound to {first}; {second} is bound to no line"
            )
            .as_str()
        ),
        "{doc}"
    );
    assert_eq!(auto.events_of_kind("auto.bound").len(), 1);

    // `<pack>/9`: not bound.
    let nine = format!("{pack}/9");
    let run = auto.task_new(&auto.main, &["R-01"], "No such line", &line_env(&nine));
    run.code(0);
    let doc = run.json();
    let id = doc["id"].as_str().unwrap();
    assert!(
        doc["notes"].as_array().unwrap().iter().any(|note| note
            == &format!(
                "SPECENGINE_AUTO_LINE `{nine}`: pack {pack} has no line 9; {id} is bound to no line"
            )),
        "{doc}"
    );
    // Not two numbers; another pack.
    for (value, why) in [
        ("x/1".to_owned(), "no <pack>/<line>".to_owned()),
        (
            format!("{}/1", pack + 1000),
            format!("no live pack {} here", pack + 1000),
        ),
    ] {
        let run = auto.task_new(&auto.main, &["R-01"], "Odd", &line_env(&value));
        run.code(0);
        let doc = run.json();
        let id = doc["id"].as_str().unwrap();
        assert!(
            doc["notes"].as_array().unwrap().iter().any(|note| note
                == &format!("SPECENGINE_AUTO_LINE `{value}`: {why}; {id} is bound to no line")),
            "{value}: {doc}"
        );
    }

    // The stop file: not bound, the switch's why.
    auto.spec(&["auto", "stop"]).code(0);
    let two = format!("{pack}/2");
    let run = auto.task_new(&auto.main, &["R-01"], "Stopped", &line_env(&two));
    run.code(0);
    let doc = run.json();
    let id = doc["id"].as_str().unwrap();
    assert!(
        doc["notes"].as_array().unwrap().iter().any(|note| {
            let note = note.as_str().unwrap();
            note.starts_with(&format!(
                "SPECENGINE_AUTO_LINE `{two}`: auto is off: the stop file "
            )) && note.ends_with(&format!("; {id} is bound to no line"))
        }),
        "{doc}"
    );
    assert_eq!(auto.events_of_kind("auto.bound").len(), 1);

    // Empty: today's command, byte for byte (no note).
    let run = auto.task_new(&auto.main, &["R-01"], "Plain", &line_env(""));
    run.code(0);
    assert_eq!(run.json()["notes"], json!([]), "{}", run.show());
}
