//! docs/features/usage-orphans.md (RM-0111) "Acceptance criteria", the
//! daemon's half, through the real daemon over git repositories of
//! `fixtures/spec-a` (only read; the copies are scratch):
//!
//! - UO4: `GET …/health` is byte-equal to `spec --root R health --json`
//!   under one `HOME` once its queue holds a card on A-102 (the row reads
//!   `tasks 1, open 1`), and again with the queue at a newer schema (the
//!   rows `tasks: null` with the note, a 200). M: a queue error failing the
//!   read.
//! - UO5: `GET …/catalog?kinds=mechanic` and `?kinds=rule` are byte-equal to
//!   `spec catalog --json`, the rows with `used_by`, `mentioned_by` and
//!   `orphan`.
//! - UO6, the hub's half: `get_node` with `links` over a hub's `/mcp/<slug>`
//!   answers what `specengine-mcp --root` answers under the same `HOME`,
//!   `usage` the CLI's (MEC-STAMINA `{2, 6}`), `archive` too.
//!
//! Every expected count is the spec's table ("Data"), worked out by hand.

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::hub::{ORIGIN, hub_args};
use common::mcp_http::{Session, content_text, is_error, result_of};
use common::{Mcp, Scratch, Server, spec, spec_json};
use serde_json::{Value, json};

const KEEP: &str = "lantern-keep";

/// `sqlite3 db sql`, which must answer.
fn sql_at(db: &Path, sql: &str) {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The queue's file under `home`, as the host rule places it.
fn db_under(home: &Path) -> PathBuf {
    let data = if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    };
    data.join(format!("{KEEP}.db"))
}

/// `spec --root root card new --title title --target target`, exit 0.
fn card(home: &Path, cwd: &Path, root: &Path, title: &str, target: &str) {
    let root = root.to_str().expect("UTF-8 root");
    let run = spec(
        home,
        cwd,
        &[
            "--root", root, "card", "new", "--title", title, "--target", target,
        ],
    );
    run.code(0);
}

/// UO4: the health route is the CLI's document byte for byte with a card on
/// A-102 and with a queue at a newer schema, one `HOME` for both.
#[test]
fn uo4_get_health_is_the_clis_document_with_tasks() {
    let scratch = Scratch::new("uo4-health");
    let root = scratch.repo("spec-a", "keep", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    card(&home, &cwd, &root, "Tune the stamina", "MEC-STAMINA");
    card(&home, &cwd, &root, "Bring the lantern in", "A-102");
    let server = Server::serve(&home, &cwd, &[&root]);
    let path = format!("/api/projects/{KEEP}/health");

    let run = spec_json(&home, &cwd, &root, &["health"]);
    run.code(0);
    let reply = server.get(&path);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.text(), run.document(), "{path}");
    let orphans = &reply.json()["orphans"];
    assert_eq!(orphans["live"], 12, "{orphans}");
    assert_eq!(orphans["notes"], json!([]), "{orphans}");
    assert_eq!(
        (
            &orphans["rows"][0]["id"],
            &orphans["rows"][0]["tasks"],
            &orphans["rows"][0]["tasks_open"]
        ),
        (&json!("A-102"), &json!(1), &json!(1)),
        "{orphans}"
    );

    sql_at(&db_under(&home), "PRAGMA user_version = 99;");
    let run = spec_json(&home, &cwd, &root, &["health"]);
    run.code(0);
    let reply = server.get(&path);
    assert_eq!(
        reply.status,
        200,
        "a queue error never fails the read: {}",
        reply.text()
    );
    assert_eq!(reply.text(), run.document(), "{path}: newer queue");
    let orphans = &reply.json()["orphans"];
    assert_eq!(orphans["rows"][0]["tasks"], Value::Null, "{orphans}");
    let note = orphans["notes"][0].as_str().unwrap_or_default();
    assert!(note.starts_with("orphans: tasks not read: "), "{orphans}");
}

/// UO5: the catalog route is the CLI's document byte for byte, the rows
/// with the three new keys.
#[test]
fn uo5_get_catalog_is_the_clis_document_with_the_counts() {
    let scratch = Scratch::new("uo5-catalog");
    let root = scratch.repo("spec-a", "keep", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    for (kind, id, counts) in [
        ("mechanic", "MEC-STAMINA", json!([2, 6, false])),
        ("rule", "RULE-STAM-REGEN", json!([2, 3, null])),
    ] {
        let run = spec_json(&home, &cwd, &root, &["catalog", "--kind", kind]);
        run.code(0);
        let reply = server.get(&format!("/api/projects/{KEEP}/catalog?kinds={kind}"));
        assert_eq!(reply.status, 200, "{}", reply.text());
        assert_eq!(reply.text(), run.document(), "{kind}");
        let document = reply.json();
        let row = document["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("{id}: {document}"))
            .clone();
        assert_eq!(
            json!([row["used_by"], row["mentioned_by"], row["orphan"]]),
            counts,
            "{row}"
        );
    }
}

/// UO6, the hub's half: `get_node` with links over `/mcp/lantern-keep`
/// answers as the stdio server under the same `HOME`, `usage` the CLI's.
#[test]
fn uo6_a_hubs_get_node_serves_usage_as_stdio() {
    let scratch = Scratch::new("uo6-hub");
    let root = scratch.repo("spec-a", "keep", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::start(&home, &cwd, &hub_args(&[&root], ORIGIN));
    let mut session = Session::open(server.port, KEEP, "mira");
    // Warm both sides on one index state: neither compared call refreshes.
    session.call("get_tree", json!({}));
    let root_arg = root.as_os_str();
    let mut stdio = Mcp::start_with(&home, &root, &[OsStr::new("--root"), root_arg]);
    stdio.request(
        1,
        "tools/call",
        json!({"name": "get_tree", "arguments": {}}),
    );
    let calls = [
        (
            json!({"id": "MEC-STAMINA", "with": ["links"]}),
            json!({"used_by": 2, "mentioned_by": 6}),
        ),
        (
            json!({"id": "MEC-STAMINA", "with": ["links"], "archive": true}),
            json!({"used_by": 2, "mentioned_by": 6}),
        ),
        (
            json!({"id": "DEC-0023", "with": ["links"], "archive": true}),
            json!({"used_by": 1, "mentioned_by": 1}),
        ),
        (
            json!({"id": "A-102", "with": ["links"]}),
            json!({"used_by": 0, "mentioned_by": 0}),
        ),
    ];
    for (n, (args, usage)) in calls.iter().enumerate() {
        let hub = result_of(&session.call("get_node", args.clone()), "get_node").clone();
        let id = 10 + i64::try_from(n).expect("small");
        let twin = stdio.request(
            id,
            "tools/call",
            json!({"name": "get_node", "arguments": args}),
        );
        let twin = result_of(&twin, "get_node").clone();
        assert!(!is_error(&hub), "{args}: {hub}");
        assert_eq!(content_text(&hub), content_text(&twin), "{args}: content");
        assert_eq!(
            hub["structuredContent"], twin["structuredContent"],
            "{args}: structuredContent"
        );
        assert_eq!(
            hub["structuredContent"]["nodes"][0]["links"]["usage"], *usage,
            "{args}"
        );
        let reference = args["id"].as_str().expect("an ID");
        let mut cli_args = vec!["show", reference, "--links"];
        if args["archive"] == json!(true) {
            cli_args.push("--archive");
        }
        let cli = spec_json(&home, &cwd, &root, &cli_args);
        cli.code(0);
        assert_eq!(
            cli.json()["nodes"][0]["links"]["usage"],
            *usage,
            "{args}: the CLI's"
        );
    }
    stdio.finish();
}
