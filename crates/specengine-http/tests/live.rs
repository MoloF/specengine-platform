//! AC-07 of docs/features/daemon-read.md: the live tail. A subscriber on
//! A: a separate process (`spec propose question`, then `specengine-mcp`
//! `ask_question`) stores an item and `proposal.created` comes with its
//! `seq` within 1 s of that process's exit; `Last-Event-ID: n` gives
//! exactly the events with `seq > n`; B's events (another database, or a
//! foreign row in A's) never reach A's stream; meanwhile `PRAGMA
//! wal_checkpoint(TRUNCATE)` on another connection is never busy. Also
//! "Data" "SSE": the opening `id: <start>` frame without data, a `:`
//! comment every 15 s, no database created by a subscriber, a bad
//! `Last-Event-ID` a 400. M: a 2 s poll; resume ignored; slug filter off;
//! one read transaction across polls.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Frame, Mcp, Scratch, Server, Stream, ask, data_dir, snapshot};
use serde_json::{Value, json};

/// Within this of the storing process's exit.
const LIVE: Duration = Duration::from_secs(1);

/// `sql` on `db` through the `sqlite3` shell, a 2 s busy timeout; its
/// stdout.
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
    String::from_utf8(output.stdout).expect("sqlite3 output")
}

fn db(home: &Path, slug: &str) -> PathBuf {
    data_dir(home).join(format!("{slug}.db"))
}

/// The opening frame: `id: <start>` and nothing else.
fn opening(stream: &mut Stream) -> i64 {
    let frame = stream
        .next_frame(Duration::from_secs(10))
        .expect("the opening frame");
    assert!(
        frame.event.is_none() && frame.data.is_none() && frame.comments.is_empty(),
        "the opening frame carries no data: {frame:?}"
    );
    let id = frame.id.as_deref().expect("the opening frame's id");
    assert_eq!(frame.raw, format!("id: {id}"), "exactly `id: <start>`");
    id.parse().expect("a decimal start")
}

/// An event frame: `id: <seq>`, `event: <type>`, `data: <payload>`, in
/// that order.
fn parsed(frame: &Frame) -> (i64, String, Value) {
    let lines: Vec<&str> = frame.raw.split('\n').collect();
    assert_eq!(lines.len(), 3, "id, event, data: {frame:?}");
    assert!(
        lines[0].starts_with("id: ")
            && lines[1].starts_with("event: ")
            && lines[2].starts_with("data: ")
    );
    let seq = frame.id.as_deref().unwrap().parse().expect("seq");
    let data: Value = serde_json::from_str(frame.data.as_deref().unwrap()).expect("JSON payload");
    (seq, frame.event.clone().unwrap(), data)
}

/// The next event within `LIVE` of `exited`; its seq, type, payload.
fn arrives(stream: &mut Stream, exited: Instant, what: &str) -> (i64, String, Value) {
    let frame = stream
        .next_event(Duration::from_secs(5))
        .unwrap_or_else(|| panic!("{what}: no event within 5 s"));
    let late = exited.elapsed();
    assert!(
        late <= LIVE,
        "{what}: the event came {late:?} after the process's exit (at most {LIVE:?})"
    );
    parsed(&frame)
}

/// The rows of `events` as stored: seq, project, type, payload.
fn rows(db: &Path) -> Vec<(i64, String, String, String)> {
    sqlite3(
        db,
        "SELECT seq, project, type, payload FROM events ORDER BY seq;",
    )
    .lines()
    .map(|line| {
        let mut fields = line.splitn(4, '|');
        (
            fields.next().unwrap().parse().unwrap(),
            fields.next().unwrap().to_owned(),
            fields.next().unwrap().to_owned(),
            fields.next().unwrap().to_owned(),
        )
    })
    .collect()
}

#[test]
fn ac07_a_stored_item_reaches_the_tail_within_a_second_of_its_exit() {
    let scratch = Scratch::new("live-created");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a, &b]);

    let mut on_a = Stream::open(server.port, "/api/projects/lantern-keep/events", &[]);
    assert_eq!(on_a.status, 200);
    assert_eq!(on_a.header("content-type"), Some("text/event-stream"));
    assert_eq!(on_a.header("cache-control"), Some("no-store"));
    let mut on_b = Stream::open(server.port, "/api/projects/zerkalo/events", &[]);
    assert_eq!(opening(&mut on_a), 0, "no database: the start is 0");
    assert_eq!(opening(&mut on_b), 0);
    // A subscriber creates nothing: a second of polls, HOME still empty.
    assert!(on_a.frames_within(Duration::from_secs(1)).is_empty());
    assert!(
        snapshot(&home).is_empty(),
        "a subscriber created {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );

    // `spec propose question`, a separate process.
    let id = ask(
        &home,
        &a,
        &["EDGE-STAM-ZERO"],
        "Does exhaustion end the sprint at once?",
    );
    let exited = Instant::now();
    let (seq, kind, payload) = arrives(&mut on_a, exited, "spec propose question");
    assert_eq!(kind, "proposal.created");
    assert_eq!(payload["id"], json!(id), "{payload}");
    let stored = rows(&db(&home, "lantern-keep"));
    assert_eq!(
        stored.last().map(|row| row.0),
        Some(seq),
        "the event's seq is its row's"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stored.last().unwrap().3).unwrap(),
        payload,
        "the payload as stored"
    );

    // `specengine-mcp` `ask_question`, a separate process.
    let mut mcp = Mcp::start(&home, &a);
    let reply = mcp.request(
        1,
        "tools/call",
        json!({"name": "ask_question", "arguments": {
            "node_ids": ["MEC-SPRINT"], "text": "Does a sprint stop at the wall?",
            "working_answer": "yes", "price_of_other": "a new rule", "author_role": "writer"}}),
    );
    let asked = reply["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("ask_question stored an item: {reply}"))
        .to_owned();
    mcp.finish();
    let exited = Instant::now();
    let (mcp_seq, kind, payload) = arrives(&mut on_a, exited, "specengine-mcp ask_question");
    assert_eq!(kind, "proposal.created");
    assert_eq!(payload["id"], json!(asked));
    assert!(mcp_seq > seq);

    // B's question: on B's stream, never on A's.
    let b_id = ask(&home, &b, &["CMD-STATUS"], "Does status print the branch?");
    let exited = Instant::now();
    let (_, kind, payload) = arrives(&mut on_b, exited, "B's question");
    assert_eq!(
        (kind.as_str(), &payload["id"]),
        ("proposal.created", &json!(b_id))
    );
    let on_a_meanwhile = on_a.frames_within(Duration::from_millis(1500));
    assert!(
        on_a_meanwhile.iter().all(|frame| frame.event.is_none()),
        "B's event reached A's stream: {on_a_meanwhile:?}"
    );
    assert!(
        on_b.frames_within(Duration::from_millis(300))
            .iter()
            .all(|f| f.event.is_none())
    );
}

#[test]
fn ac07_last_event_id_resumes_exactly_after_it_and_never_another_projects_row() {
    let scratch = Scratch::new("live-resume");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let first = ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let second = ask(&home, &a, &["MEC-SPRINT"], "Does the sprint key toggle?");
    let a_db = db(&home, "lantern-keep");
    // A row of another project in A's database: the slug filter's case.
    sqlite3(
        &a_db,
        "INSERT INTO events (project, type, payload, at) VALUES \
         ('zerkalo', 'proposal.created', '{\"id\":\"PR-0099\"}', '2026-10-06T00:00:00Z');",
    );
    let third = ask(&home, &a, &["RULE-CORE-LOOP"], "Does the loop end at dawn?");
    let stored = rows(&a_db);
    let ours: Vec<(i64, String)> = stored
        .iter()
        .filter(|row| row.1 == "lantern-keep")
        .map(|row| (row.0, row.3.clone()))
        .collect();
    assert_eq!(ours.len(), 3, "{stored:?}");
    let foreign = stored
        .iter()
        .find(|row| row.1 == "zerkalo")
        .expect("the foreign row")
        .0;
    let highest = stored.iter().map(|row| row.0).max().unwrap();

    let server = Server::serve(&home, scratch.path(), &[&a]);
    let path = "/api/projects/lantern-keep/events";
    // For each n: exactly the project's events with seq > n, at once.
    for n in [0, ours[0].0, foreign, ours[2].0] {
        let mut stream = Stream::open(server.port, path, &[("Last-Event-ID", &n.to_string())]);
        assert_eq!(stream.status, 200);
        assert_eq!(
            opening(&mut stream),
            n,
            "the opening id is the Last-Event-ID"
        );
        let started = Instant::now();
        let frames = stream.frames_within(Duration::from_millis(1200));
        let got: Vec<(i64, String)> = frames
            .iter()
            .filter(|frame| frame.event.is_some())
            .map(|frame| {
                let (seq, kind, payload) = parsed(frame);
                assert_eq!(kind, "proposal.created");
                (seq, payload.to_string())
            })
            .collect();
        let want: Vec<(i64, String)> = ours
            .iter()
            .filter(|(seq, _)| *seq > n)
            .map(|(seq, payload)| {
                (
                    *seq,
                    serde_json::from_str::<Value>(payload).unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(got, want, "Last-Event-ID: {n}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    let ids: Vec<Value> = ours
        .iter()
        .map(|(_, payload)| serde_json::from_str::<Value>(payload).unwrap()["id"].clone())
        .collect();
    assert_eq!(ids, [json!(first), json!(second), json!(third)]);

    // No Last-Event-ID: the start is the highest seq now, no replay.
    let mut fresh = Stream::open(server.port, path, &[]);
    assert_eq!(opening(&mut fresh), highest);
    assert!(
        fresh
            .frames_within(Duration::from_millis(800))
            .iter()
            .all(|f| f.event.is_none()),
        "no replay"
    );
    // A new item after that start: exactly it.
    let fourth = ask(&home, &a, &["DOM-MOVEMENT"], "Is walking a mechanic?");
    let exited = Instant::now();
    let (seq, _, payload) = arrives(&mut fresh, exited, "the fourth question");
    assert_eq!(payload["id"], json!(fourth));
    assert!(seq > highest);

    // Not a non-negative integer: 400, the error body.
    for bad in [
        "abc",
        "-1",
        "1.5",
        "",
        "3 4",
        "+3",
        "0x10",
        "99999999999999999999999",
    ] {
        let reply = server.request_within(
            "GET",
            path,
            &[("Last-Event-ID", bad)],
            Duration::from_secs(20),
        );
        assert_eq!(reply.status, 400, "Last-Event-ID {bad:?}: {}", reply.text());
        reply.error_message();
    }
}

#[test]
fn ac07_a_checkpoint_meanwhile_is_never_busy() {
    let scratch = Scratch::new("live-checkpoint");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let a_db = db(&home, "lantern-keep");
    assert_eq!(
        sqlite3(&a_db, "PRAGMA journal_mode;").trim(),
        "wal",
        "the queue is in WAL mode"
    );
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut streams: Vec<Stream> = (0..3)
        .map(|_| {
            Stream::open(
                server.port,
                "/api/projects/lantern-keep/events",
                &[("Last-Event-ID", "0")],
            )
        })
        .collect();
    for stream in &mut streams {
        opening(stream);
        assert!(
            stream.next_event(Duration::from_secs(5)).is_some(),
            "the stored event"
        );
    }
    // Rounds of: an item stored by a separate process (its frames in the
    // WAL), every stream polling it, a few more polls, then a TRUNCATE
    // checkpoint on another connection; and checkpoints every 100 ms
    // between. A read transaction a stream kept open across polls would
    // pin the WAL and make one busy.
    let started = Instant::now();
    let mut results = Vec::new();
    for round in 0..5 {
        let id = ask(
            &home,
            &a,
            &["EDGE-STAM-ZERO"],
            &format!("Round {round}: does exhaustion last {round}.5 s?"),
        );
        let exited = Instant::now();
        for stream in &mut streams {
            let (_, _, payload) = arrives(stream, exited, "a round's question");
            assert_eq!(payload["id"], json!(id));
        }
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(100));
            let result = sqlite3(&a_db, "PRAGMA wal_checkpoint(TRUNCATE);");
            results.push(result.trim().to_owned());
        }
    }
    assert!(results.len() >= 10, "{results:?}");
    let busy: Vec<&String> = results
        .iter()
        .filter(|result| !result.starts_with("0|"))
        .collect();
    assert!(
        busy.is_empty(),
        "a checkpoint was busy: {busy:?} of {}",
        results.len()
    );
    assert!(started.elapsed() < Duration::from_secs(60), "{results:?}");
    // The streams are still live: a new item reaches each.
    let id = ask(&home, &a, &["MEC-SPRINT"], "Is the sprint key held?");
    let exited = Instant::now();
    for stream in &mut streams {
        let (_, _, payload) = arrives(stream, exited, "after the checkpoints");
        assert_eq!(payload["id"], json!(id));
    }
}

#[test]
fn a_quiet_stream_gets_a_comment_every_15_seconds() {
    let scratch = Scratch::new("live-keepalive");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut stream = Stream::open(server.port, "/api/projects/lantern-keep/events", &[]);
    opening(&mut stream);
    let started = Instant::now();
    let mut seen = Vec::new();
    let at = loop {
        let left = Duration::from_secs(17).saturating_sub(started.elapsed());
        assert!(!left.is_zero(), "no `:` comment within 17 s: {seen:?}");
        if let Some(frame) = stream.next_frame(left) {
            if !frame.comments.is_empty() {
                assert!(frame.event.is_none() && frame.data.is_none(), "{frame:?}");
                break started.elapsed();
            }
            seen.push(frame);
        }
        assert!(!stream.ended(), "the stream ended: {seen:?}");
    };
    assert!(at >= Duration::from_secs(13), "the first comment at {at:?}");
    assert!(seen.is_empty(), "nothing else came: {seen:?}");
    assert!(!stream.ended(), "the stream stays open");
}
