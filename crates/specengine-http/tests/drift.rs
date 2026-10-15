//! TE-11 of docs/features/drift-queue.md, the daemon's half ("Review,
//! inbox, round, stage"; RD-11): a drift item raised by `spec propose
//! drift` on `fixtures/code-a` (its golden lock `fixtures/spec-lock/
//! code-a.lock` committed, `CAP` edited and committed) is served by the
//! daemon as the CLI writes it: the review document's 48 keys, `drift`
//! `{hash, rows, more}`, `preview` `null` with the note naming `spec
//! approve`; its inbox entry; the round sheet leaves it off with the note.
//! A same-origin stage (approve or reject) is refused 409 naming the item,
//! before any preview runs: nothing staged, no event, `updated_at` kept.
//! M: the stage refusal dropped from CLI `stage::checked` (the store's
//! refusal then answers instead, no longer the 409 with this text).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Scratch, Server, data_dir, run_product, spec_bin};
use serde_json::{Value, json};

const CODE: &str = "/api/projects/code-a";

/// `sql` on `db` through the system's `sqlite3` (a 2 s busy timeout).
fn sqlite3(db: &Path, sql: &str) -> String {
    let output = Command::new("/usr/bin/sqlite3")
        .env_clear()
        .args(["-cmd", ".timeout 2000"])
        .arg(db)
        .arg(sql)
        .output()
        .expect("run sqlite3");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `fixtures/code-a` with its golden lock and `CAP` edited, committed; the
/// drift raised: the root and the drift document.
fn raised(scratch: &Scratch, home: &Path) -> (PathBuf, Value) {
    let root = scratch.repo("code-a", "code", "main");
    let git = scratch.git();
    fs::copy(
        common::fixture("spec-lock").join("code-a.lock"),
        root.join("spec.lock"),
    )
    .expect("the golden lock");
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "the lock"]);
    common::replace(
        &root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    git.run(&root, &["commit", "-q", "-a", "-m", "the cap"]);
    let run = run_product(
        &spec_bin(),
        home,
        &root,
        &["propose", "drift", "LAW-BOUND", "--json"],
        b"",
    );
    run.code(0);
    (root, run.json())
}

#[test]
fn drift_item_is_served_and_cannot_be_staged() {
    let scratch = Scratch::new("drift-http");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let (root, raised) = raised(&scratch, &home);
    assert_eq!(raised["created"][0]["id"], json!("PR-0001"), "{raised}");
    assert_eq!(raised["created"][0]["node"], json!("LAW-BOUND"), "{raised}");
    let db = data_dir(&home).join("code-a.db");
    let server = Server::serve(&home, &cwd, &[&root]);

    // The review document: 48 keys, the drift item's; `drift` after
    // `revisions`, its keys `hash`, `rows`, `more` in that order.
    let text = server
        .get(&format!("{CODE}/proposals/PR-0001"))
        .status(200)
        .text()
        .to_owned();
    let review: Value = serde_json::from_str(&text).expect("JSON");
    let object = review.as_object().expect("an object");
    assert_eq!(object.len(), 48, "{review}");
    assert_eq!(review["kind"], json!("drift"));
    assert_eq!(review["target_id"], json!("LAW-BOUND"));
    assert_eq!(review["target_path"], json!("spec.lock"));
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("no {needle} in {text}"))
    };
    assert!(
        at("\"revisions\":") < at("\"drift\":{\"hash\":\"b3:")
            && at("\"drift\":{") < at("\"decided_by\":"),
        "{text}"
    );
    let drift = review["drift"].as_object().expect("a drift object");
    let mut keys: Vec<&str> = drift.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["hash", "more", "rows"], "{review}");
    let inner = &text[at("\"drift\":{")..];
    assert!(
        inner.find("\"rows\":[") < inner.find("\"more\":"),
        "rows before more: {inner}"
    );
    let hash = drift["hash"].as_str().expect("a hash");
    assert!(hash.starts_with("b3:") && hash.len() == 67, "{hash}");
    assert!(
        drift["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .any(|row| row["state"] == "code_ahead" && row["category"] == "code_changed"),
        "{review}"
    );
    for key in [
        "preview",
        "diff",
        "conflict",
        "base_text",
        "new_text",
        "revisions",
    ] {
        assert_eq!(review[key], Value::Null, "{key}: {review}");
    }
    let notes: Vec<&str> = review["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        notes.contains(&"spec approve PR-0001 shows the rows before asking"),
        "{notes:?}"
    );

    // The inbox lists it; the round sheet leaves it off, with the note.
    let inbox = server.get(&format!("{CODE}/inbox")).status(200).json();
    let entries = inbox["proposals"].as_array().expect("proposals");
    assert_eq!(entries.len(), 1, "{inbox}");
    assert_eq!(entries[0]["kind"], json!("drift"));
    let sheet = server.get(&format!("{CODE}/round")).status(200).json();
    assert_eq!(sheet["items"], json!([]), "{sheet}");
    let sheet_notes: Vec<&str> = sheet["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        sheet_notes.contains(
            &"1 open drift item(s) not on this sheet: PR-0001; spec review <ID>, then spec \
              approve or spec reject"
        ),
        "{sheet_notes:?}"
    );

    // A same-origin stage, approve or reject: 409, nothing staged.
    let updated_at = review["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let events = sqlite3(&db, "SELECT count(*) FROM events;");
    for fields in [
        "\"decision\":\"approve\"",
        "\"decision\":\"approve\",\"editorial\":true",
        "\"decision\":\"reject\",\"reason\":\"not now\"",
    ] {
        let reply = server.stage(
            &format!("{CODE}/proposals/PR-0001/decision"),
            &format!("{{{fields},\"updated_at\":\"{updated_at}\"}}"),
        );
        assert_eq!(reply.status, 409, "{fields}: {}", reply.text());
        // A 409 answers the document, its reason the last note.
        let document = reply.json();
        assert_eq!(document["id"], json!("PR-0001"), "{fields}");
        assert_eq!(
            document["notes"].as_array().and_then(|notes| notes.last()),
            Some(&json!(
                "PR-0001 is a drift item, decided on a terminal only; nothing staged"
            )),
            "{fields}: {document}"
        );
    }
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE staged IS NOT NULL OR staged_at IS NOT NULL;"
        ),
        "0"
    );
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM events;"),
        events,
        "no event"
    );
    let after = server
        .get(&format!("{CODE}/proposals/PR-0001"))
        .status(200)
        .json();
    assert_eq!(after["updated_at"], json!(updated_at));
    assert_eq!(after["staged"], Value::Null);
    drop(server);
}
