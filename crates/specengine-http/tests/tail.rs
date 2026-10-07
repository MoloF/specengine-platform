//! docs/features/daemon-read.md "Data" "SSE", iteration 2 of the daemon's
//! live tail:
//! - the clamp: a `Last-Event-ID` above the highest `seq` (the database
//!   wiped or made anew, or none yet) starts from the highest (0 when no
//!   database): the opening frame says so and the new database's events
//!   still come; no database is created meanwhile;
//! - a full page: rows read count skipped ones (a NULL `type`), so a page
//!   of them is followed at once, not 250 ms later;
//! - one connection per stream, kept from poll to poll (the `-wal` file of
//!   an idle subscriber's database keeps its inode), dropped when the
//!   stream ends (the last one closed: `-wal` and `-shm` gone);
//! - a database file replaced under a live stream is opened again: its
//!   events come;
//! - a newer build's `user_version` set under a live stream ends it.
//!
//! M: `.min(highest)` removed; `full` as events kept; the connection
//! dropped at each poll; the file identity check skipped; the
//! `user_version` check in the poll removed.
//!
//! AC-06 of docs/features/ui-live-tasks.md, the tail's half: a stream on A
//! receives `task.created`, `task.approved`, `task.claimed`,
//! `task.run_reported` raised through the CLI library, each within a
//! second, `id: <seq>`, `event: <type>`, `data` the stored payload byte
//! for byte (`{id}`, `+ run` for the claim and the report), in order, and
//! nothing else. M: the tail keeping only `proposal.`.

mod common;
mod task_state;

use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Frame, Scratch, Server, Stream, ask, data_dir, snapshot};
use serde_json::{Value, json};

/// Within this of the storing process's exit.
const LIVE: Duration = Duration::from_secs(1);

const EVENTS: &str = "/api/projects/lantern-keep/events";

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

fn db(home: &Path) -> PathBuf {
    data_dir(home).join("lantern-keep.db")
}

/// `db` with `suffix` appended to its file name (`-wal`, `-shm`).
fn beside(db: &Path, suffix: &str) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// The opening frame: `id: <start>` and nothing else; the start.
fn opening(stream: &mut Stream) -> i64 {
    assert_eq!(stream.status, 200, "the stream opens");
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

/// An event frame's seq, type and payload.
fn parsed(frame: &Frame) -> (i64, String, Value) {
    let seq = frame.id.as_deref().expect("id").parse().expect("seq");
    let data: Value =
        serde_json::from_str(frame.data.as_deref().expect("data")).expect("JSON payload");
    (seq, frame.event.clone().expect("event"), data)
}

/// The next event, within `LIVE` of `since`.
fn arrives(stream: &mut Stream, since: Instant, what: &str) -> (i64, String, Value) {
    let frame = stream
        .next_event(Duration::from_secs(5))
        .unwrap_or_else(|| panic!("{what}: no event within 5 s"));
    let late = since.elapsed();
    assert!(
        late <= LIVE,
        "{what}: the event came {late:?} after (at most {LIVE:?})"
    );
    parsed(&frame)
}

/// Every frame within `window` carries no event.
fn quiet(stream: &mut Stream, window: Duration, what: &str) {
    let frames = stream.frames_within(window);
    assert!(
        frames
            .iter()
            .all(|frame| frame.event.is_none() && frame.data.is_none()),
        "{what}: no event: {frames:?}"
    );
    assert!(!stream.ended(), "{what}: the stream stays open");
}

fn highest(db: &Path) -> i64 {
    sqlite3(db, "SELECT max(seq) FROM events;")
        .trim()
        .parse()
        .expect("a highest seq")
}

#[test]
fn a_last_event_id_above_the_highest_starts_from_the_highest() {
    let scratch = Scratch::new("tail-clamp");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);

    // No database yet: `Last-Event-ID: 7` opens at 0, nothing is created,
    // and the first event of the database made after comes.
    let mut stream = Stream::open(server.port, EVENTS, &[("Last-Event-ID", "7")]);
    assert_eq!(opening(&mut stream), 0, "no database: the start is 0");
    quiet(&mut stream, Duration::from_millis(1000), "no database");
    assert!(
        snapshot(&home).is_empty(),
        "a subscriber creates nothing: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
    let first = ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let exited = Instant::now();
    let (seq, kind, payload) = arrives(&mut stream, exited, "the first event of a new database");
    assert_eq!((seq, kind.as_str()), (1, "proposal.created"));
    assert_eq!(payload["id"], json!(first));

    // A database with a highest h: a Last-Event-ID above it opens at h,
    // no replay, and the next event comes.
    ask(&home, &a, &["MEC-SPRINT"], "Does the sprint key toggle?");
    ask(&home, &a, &["RULE-CORE-LOOP"], "Does the loop end at dawn?");
    let h = highest(&db(&home));
    assert_eq!(h, 3);
    let mut streams: Vec<Stream> = Vec::new();
    for resume in [h, h + 1, 999, i64::MAX] {
        let mut stream = Stream::open(
            server.port,
            EVENTS,
            &[("Last-Event-ID", &resume.to_string())],
        );
        assert_eq!(
            opening(&mut stream),
            h,
            "Last-Event-ID {resume}: opens at the highest {h}"
        );
        quiet(
            &mut stream,
            Duration::from_millis(600),
            &format!("Last-Event-ID {resume}"),
        );
        streams.push(stream);
    }
    let next = ask(&home, &a, &["DOM-MOVEMENT"], "Is walking a mechanic?");
    let exited = Instant::now();
    for stream in &mut streams {
        let (seq, _, payload) = arrives(stream, exited, "the event after the highest");
        assert_eq!(seq, h + 1);
        assert_eq!(payload["id"], json!(next));
    }
}

#[test]
fn a_full_page_of_skipped_rows_is_followed_at_once() {
    let scratch = Scratch::new("tail-full");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let db = db(&home);
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut stream = Stream::open(server.port, EVENTS, &[]);
    assert_eq!(opening(&mut stream), 1);
    quiet(&mut stream, Duration::from_millis(400), "before the rows");
    // 4096 rows of the project with a NULL type (8 full pages of 512
    // skipped rows), then one event, in one transaction.
    sqlite3(
        &db,
        "BEGIN; \
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 4096) \
         INSERT INTO events (project, type, payload, at) \
         SELECT 'lantern-keep', NULL, '{}', '2026-10-06T00:00:00Z' FROM n; \
         INSERT INTO events (project, type, payload, at) VALUES \
         ('lantern-keep', 'proposal.created', '{\"id\":\"PR-0777\"}', '2026-10-06T00:00:01Z'); \
         COMMIT;",
    );
    let committed = Instant::now();
    let (seq, kind, payload) = arrives(&mut stream, committed, "the event after 4096 skipped rows");
    assert_eq!((seq, kind.as_str()), (4098, "proposal.created"));
    assert_eq!(payload, json!({"id": "PR-0777"}));
    // Nothing else: the skipped rows are never sent.
    quiet(&mut stream, Duration::from_millis(500), "after the event");
}

/// The `-wal` file's inode, sampled every 5 ms for `window`; `None` when
/// it was absent.
fn wal_samples(wal: &Path, window: Duration) -> Vec<Option<u64>> {
    let started = Instant::now();
    let mut samples = Vec::new();
    while started.elapsed() < window {
        samples.push(fs::metadata(wal).ok().map(|meta| meta.ino()));
        std::thread::sleep(Duration::from_millis(5));
    }
    samples
}

/// One inode, present at every sample.
fn one_inode(samples: &[Option<u64>], what: &str) -> u64 {
    let first = samples.first().copied().flatten();
    let missing = samples.iter().filter(|sample| sample.is_none()).count();
    let changed = samples.iter().filter(|sample| **sample != first).count();
    assert!(
        first.is_some() && missing == 0 && changed == 0,
        "{what}: the -wal file was absent at {missing} and another file at {changed} of {} \
         samples (a connection opened and closed per poll)",
        samples.len()
    );
    first.unwrap()
}

#[test]
fn a_stream_keeps_one_connection_and_drops_it_when_it_ends() {
    let scratch = Scratch::new("tail-reuse");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let db = db(&home);
    let (wal, shm) = (beside(&db, "-wal"), beside(&db, "-shm"));
    assert!(
        !wal.exists() && !shm.exists(),
        "no connection is open: no -wal, no -shm"
    );
    let server = Server::serve(&home, scratch.path(), &[&a]);

    let mut first = Stream::open(server.port, EVENTS, &[]);
    assert_eq!(opening(&mut first), 1);
    std::thread::sleep(Duration::from_millis(300));
    let inode = one_inode(
        &wal_samples(&wal, Duration::from_secs(2)),
        "one idle subscriber",
    );

    // A second subscriber; the first one leaves: the second's connection
    // keeps the file.
    let mut second = Stream::open(server.port, EVENTS, &[]);
    assert_eq!(opening(&mut second), 1);
    drop(first);
    let kept = one_inode(
        &wal_samples(&wal, Duration::from_millis(1500)),
        "the second subscriber after the first left",
    );
    assert_eq!(kept, inode, "the same -wal file throughout");

    // Both streams still deliver on their kept connection.
    let id = ask(&home, &a, &["MEC-SPRINT"], "Does the sprint key toggle?");
    let exited = Instant::now();
    let (seq, _, payload) = arrives(&mut second, exited, "an event on the kept connection");
    assert_eq!((seq, payload["id"].clone()), (2, json!(id)));

    // The last subscriber leaves: its connection closes, SQLite removes
    // the -wal and -shm files.
    drop(second);
    let started = Instant::now();
    while (wal.exists() || shm.exists()) && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !wal.exists() && !shm.exists(),
        "every subscriber gone: the connection is closed within 5 s (-wal {}, -shm {})",
        wal.exists(),
        shm.exists()
    );
    assert!(db.exists(), "the database stays");
    assert_eq!(highest(&db), 2);
}

#[test]
fn a_database_replaced_under_a_live_stream_still_delivers() {
    let scratch = Scratch::new("tail-replaced");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    ask(&home, &a, &["MEC-SPRINT"], "Does the sprint key toggle?");
    let db = db(&home);
    let (wal, shm) = (beside(&db, "-wal"), beside(&db, "-shm"));
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut stream = Stream::open(server.port, EVENTS, &[]);
    assert_eq!(opening(&mut stream), 2);
    let third = ask(&home, &a, &["RULE-CORE-LOOP"], "Does the loop end at dawn?");
    let exited = Instant::now();
    let (seq, _, payload) = arrives(&mut stream, exited, "the third question");
    assert_eq!((seq, payload["id"].clone()), (3, json!(third)));

    // The database file replaced by a byte copy of itself (a new inode,
    // the same highest seq): everything checkpointed into the main file,
    // the three files removed, the copy written.
    assert_eq!(
        sqlite3(&db, "PRAGMA wal_checkpoint(TRUNCATE);").trim(),
        "0|0|0",
        "the checkpoint is not busy"
    );
    let bytes = fs::read(&db).expect("the database's bytes");
    let old_inode = fs::metadata(&db).expect("the database").ino();
    for file in [&db, &wal, &shm] {
        let _ = fs::remove_file(file);
    }
    fs::write(&db, &bytes).expect("the copy");
    let new_inode = fs::metadata(&db).expect("the copy").ino();
    assert_ne!(old_inode, new_inode, "the copy is another file");
    // The stream's next poll sees another file and opens it (its -wal
    // appears); then a new item is stored in it.
    let started = Instant::now();
    while !wal.exists() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let reopened = wal.exists();
    let fourth = ask(&home, &a, &["DOM-MOVEMENT"], "Is walking a mechanic?");
    let exited = Instant::now();
    let (seq, kind, payload) = arrives(&mut stream, exited, "an event of the replaced database");
    assert_eq!((seq, kind.as_str()), (4, "proposal.created"));
    assert_eq!(payload["id"], json!(fourth));
    assert!(
        reopened,
        "the stream opened the new file within 3 s (its -wal appeared)"
    );
    assert_eq!(fs::metadata(&db).expect("the database").ino(), new_inode);
}

#[test]
fn a_newer_user_version_under_a_live_stream_ends_it() {
    let scratch = Scratch::new("tail-newer");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let db = db(&home);
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut stream = Stream::open(server.port, EVENTS, &[]);
    assert_eq!(opening(&mut stream), 1);
    quiet(&mut stream, Duration::from_millis(400), "before");
    let version: i64 = sqlite3(&db, "PRAGMA user_version;")
        .trim()
        .parse()
        .expect("user_version");
    sqlite3(&db, &format!("PRAGMA user_version = {};", version + 7));
    let set = Instant::now();
    let frames = stream.frames_within(Duration::from_secs(3));
    assert!(
        stream.ended(),
        "a newer build's queue ends the live stream (its kept connection checks \
         user_version per poll); frames meanwhile: {frames:?}"
    );
    assert!(
        set.elapsed() < Duration::from_secs(2),
        "ended {:?} after the version was set",
        set.elapsed()
    );
    assert!(
        frames.iter().all(|frame| frame.event.is_none()),
        "{frames:?}"
    );
    // The browser's reconnect meets the error.
    let reply = server.request_within("GET", EVENTS, &[], Duration::from_secs(20));
    assert_eq!(reply.status, 503, "{}", reply.text());
    reply.error_message();
}

#[test]
fn ac06_a_stream_receives_the_task_events_raised_through_the_library() {
    let scratch = Scratch::new("tail-tasks");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut stream = Stream::open(server.port, EVENTS, &[]);
    assert_eq!(opening(&mut stream), 0, "no database yet");
    let calls = task_state::Calls::new(&scratch, &a, &home);
    let mut received = Vec::new();
    for want in [
        "task.created",
        "task.approved",
        "task.claimed",
        "task.run_reported",
    ] {
        match want {
            "task.created" => {
                assert_eq!(calls.new_task("MEC-STAMINA", "Live", false), "T-0001");
            }
            "task.approved" => {
                calls.owner("approve", "T-0001", None, specengine_cli::TaskStatus::Ready);
            }
            "task.claimed" => calls.claim("T-0001"),
            _ => calls.report("T-0001"),
        }
        let stored = Instant::now();
        let (seq, kind, payload) = arrives(&mut stream, stored, want);
        assert_eq!(kind, want, "the events come in the order raised");
        received.push((seq, kind, payload));
    }
    assert_eq!(
        received,
        [
            (1, "task.created".to_owned(), json!({"id": "T-0001"})),
            (2, "task.approved".to_owned(), json!({"id": "T-0001"})),
            (
                3,
                "task.claimed".to_owned(),
                json!({"id": "T-0001", "run": 1})
            ),
            (
                4,
                "task.run_reported".to_owned(),
                json!({"id": "T-0001", "run": 1})
            ),
        ]
    );
    quiet(&mut stream, Duration::from_millis(500), "after the four");
    drop(stream);

    // Byte for byte: a stream from the start sends each stored row as
    // stored.
    let stored = stored_rows(&home, &a);
    assert_eq!(stored.len(), 4, "{stored:?}");
    let mut replay = Stream::open(server.port, EVENTS, &[("Last-Event-ID", "0")]);
    assert_eq!(opening(&mut replay), 0);
    for (seq, kind, payload) in &stored {
        let frame = replay
            .next_event(Duration::from_secs(5))
            .unwrap_or_else(|| panic!("event {seq}: not replayed"));
        assert_eq!(
            frame.raw,
            format!("id: {seq}\nevent: {kind}\ndata: {payload}"),
            "event {seq}: the stored row"
        );
    }
}

/// `(seq, type, payload as stored)` of the project's events, read through
/// the CLI library (not through the daemon).
fn stored_rows(home: &Path, root: &Path) -> Vec<(i64, String, String)> {
    let env = specengine_cli::Env {
        cwd: root.to_path_buf(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = specengine_cli::Globals {
        root: Some(root.to_path_buf()),
        config: None,
    };
    specengine_cli::events_after(&env, &globals, Some(0))
        .expect("the events read")
        .events
        .into_iter()
        .map(|event| (event.seq, event.event_type, event.payload))
        .collect()
}
