//! docs/features/ui-round.md, the daemon's half ("Data", "Rules and edge
//! cases"): `GET /api/projects/:p/round` and `POST
//! /api/projects/:p/round/answers` on git copies of `fixtures/spec-a` (A),
//! every request a real one over TCP to the real binary, the queue read
//! back through the daemon's own reads, the CLI library and `sqlite3`, the
//! events through the CLI library.
//!
//! - R-01: A with an open question waited on by a `ready` task, another
//!   question, two discrepancies: `round` and `round?limit=1` byte-equal
//!   the CLI library's `round_new` document (`spec round new [--limit 1]
//!   --json` without its final LF); an empty queue 200, the library's too;
//!   no event. M: `limit` ignored.
//! - R-02: `limit=0`, `201`, `-3`, `x`, `limit=1&limit=2`, `depth=1`: each
//!   400 naming it, nothing run (a fresh `HOME` stays empty), no event. M:
//!   the range left to the library (503).
//! - R-04: the transport of the answers: an over-cap `Content-Length`
//!   with no `Sec-Fetch-Site` 403; `text/plain` 415; an over-cap JSON
//!   413, declared and unread (answered at once), or sent; an undecodable
//!   body, another slug, 201 answers 400 with the CLI's line; nothing
//!   staged, no event. 200 answers and none: 200. M: the body read before
//!   the origin check.
//! - R-05: shuffled answers (a question's working answer, another's own
//!   text with an LF, a discrepancy's option 1, another's reject): 200,
//!   four staged at one clock reading, the entries in request order, four
//!   `proposal.staged` in round order and no other event; `git status`,
//!   `HEAD`, the refs, the worktrees and the files unchanged (no record
//!   file). T-03: then the library's `round_answer --staged` with a
//!   scripted terminal answering `n` asks exactly those four in round
//!   order, each `, staged)` and `? [y/N]`, and keeps them all. M: staged
//!   in request order.
//! - R-06: two valid answers among eighteen refused ones, each refused
//!   alone, named; two staged; 200. Without `[decision_records]`: an
//!   approve `not applied (step 3): …; a reject still settles it`, a
//!   reject staged. M: the last duplicate wins; one refusal fails the
//!   request; `updated_at` read at POST time.
//! - R-07: an answer on a staged item: `replaced` the earlier
//!   `staged_at`, one event.
//!
//! R-03 is in `methods.rs` and `projects.rs`, R-08 in `turn.rs`, T-02 in
//! `door.rs`, T-04 in `daemon_keys.rs`. Non-Latin characters in Rust
//! sources are Unicode escapes (ADR-0024).

mod common;
mod task_state;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Reply, Scratch, Server, data_dir, http_exchange, request_bytes, snapshot};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveRequest, DiscrepancyInput, DiscrepancyRequest, Evidence, Exit, GapType, IntakeOption,
    IntakeSeverity, Outcome, QuestionRequest, RoundAnswerRequest, RoundNewRequest, RoundTerminal,
    StageBody, StageRequest, TaskStatus, WalkOutcome, events_after, propose_discrepancy,
    propose_question_with_task, render_json, round_answer, round_new, stage,
};
use task_state::Calls;

const SLUG: &str = "lantern-keep";
const P: &str = "/api/projects/lantern-keep";
const ANSWERS: &str = "/api/projects/lantern-keep/round/answers";

/// The clocks of the items: an older and a newer one.
const OLDER: &str = "2026-10-07T09:00:00Z";
const NEWER: &str = "2026-10-07T09:30:00Z";

/// A stage made before the daemon's (R-07).
const EARLIER: &str = "2026-10-07T10:00:00Z";

/// The 4 MiB cap of the answers (W-6).
const CAP: usize = 4_194_304;

// ---------------------------------------------------------------- setup

/// A question of a developer on `targets` at `now` (bound to `task`),
/// stored: when its first ask hits, asked again naming every hit.
fn question(
    calls: &Calls,
    targets: &[&str],
    text: &str,
    severity: Option<IntakeSeverity>,
    now: &str,
    task: Option<&str>,
) -> String {
    let mut request = QuestionRequest {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        text: text.to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        severity,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        now: now.to_owned(),
        git: calls.git_env(),
    };
    let ask = |request: &QuestionRequest| {
        propose_question_with_task(&calls.env(), &calls.globals(), request, task)
            .unwrap_or_else(|error| panic!("ask {text}: {error}"))
    };
    let mut outcome = ask(&request);
    if outcome.refusal.is_none() && !outcome.document.created {
        request.distinct_from = outcome
            .document
            .hits
            .iter()
            .map(|hit| hit.name().to_owned())
            .collect();
        outcome = ask(&request);
    }
    assert_eq!(outcome.exit(), Exit::Answered, "ask {text}: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("the question's ID")
}

/// A discrepancy of a developer on `targets` with three options (0-2,
/// recommendation 0), stored as [`question`] stores one.
fn gap(calls: &Calls, targets: &[&str], summary: &str, severity: IntakeSeverity) -> String {
    let input = DiscrepancyInput {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        summary: summary.to_owned(),
        gap_type: GapType::Contradicts,
        severity,
        evidence: vec![Evidence {
            file: "src/stamina.rs".to_owned(),
            qpath: Some("regen_system".to_owned()),
            lines: Some("10-20".to_owned()),
            observed: "regenerates while sprinting".to_owned(),
            documented: "regenerates only at rest".to_owned(),
        }],
        options: [
            ("Keep the spec", "Fix the code", "one work item"),
            ("Change the spec", "Allow it", "a new rule case"),
            ("Defer", "Nothing changes now", "the gap stays open"),
        ]
        .iter()
        .map(|(label, effect, price)| IntakeOption {
            label: (*label).to_owned(),
            effect: (*effect).to_owned(),
            price: (*price).to_owned(),
        })
        .collect(),
        recommendation: 0,
        working_answer: Some("Keep the spec".to_owned()),
        proposed_patch: None,
        distinct_from: None,
    };
    let report = |input: DiscrepancyInput| {
        propose_discrepancy(
            &calls.env(),
            &calls.globals(),
            &DiscrepancyRequest {
                input,
                author_role: Some("developer".to_owned()),
                author_model: Some("claude-opus-5-5".to_owned()),
                run: None,
                now: OLDER.to_owned(),
                git: calls.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("report {summary}: {error}"))
    };
    let mut outcome = report(input.clone());
    if outcome.refusal.is_none() && !outcome.document.created {
        let mut named = input;
        named.distinct_from = Some(
            outcome
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect(),
        );
        outcome = report(named);
    }
    assert_eq!(outcome.exit(), Exit::Answered, "{summary}: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("the discrepancy's ID")
}

/// A of R-01, R-05, R-07: a `ready` task on MEC-STAMINA; `q1` a question
/// on it bound to the task (waited on), `q2` a newer question, `d1` a high
/// discrepancy, `d2` a low one. Round order: `d1`, `q1`, `q2`, `d2`.
struct Four {
    scratch: Scratch,
    a: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    calls: Calls,
    task: String,
    q1: String,
    q2: String,
    d1: String,
    d2: String,
}

fn four(label: &str) -> Four {
    let scratch = Scratch::new(label);
    let a = scratch.repo("spec-a", "a", "main");
    // An identity, as an owner's repository has one: a decision taken
    // through the daemon could commit.
    let git = scratch.git();
    git.run(&a, &["config", "user.name", "Owner"]);
    git.run(&a, &["config", "user.email", "owner@example.invalid"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let calls = Calls::new(&scratch, &a, &home);
    let task = calls.new_task("MEC-STAMINA", "Tune the stamina", false);
    calls.owner("approve", &task, None, TaskStatus::Ready);
    let q1 = question(
        &calls,
        &["MEC-STAMINA"],
        "Does stamina drain at rest?",
        None,
        OLDER,
        Some(&task),
    );
    let q2 = question(
        &calls,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
        None,
        NEWER,
        None,
    );
    let d1 = gap(
        &calls,
        &["MEC-SPRINT"],
        "Walking drains stamina in the build.",
        IntakeSeverity::High,
    );
    let d2 = gap(
        &calls,
        &["EDGE-STAM-ZERO"],
        "The bar empties twice in the build.",
        IntakeSeverity::Low,
    );
    Four {
        scratch,
        a,
        home,
        cwd,
        calls,
        task,
        q1,
        q2,
        d1,
        d2,
    }
}

// ---------------------------------------------------------------- reads

/// The library's sheet as the daemon sends it: `spec round new [--limit
/// N] --json`'s stdout without its final LF.
fn library_sheet(calls: &Calls, limit: Option<i64>) -> String {
    let sheet = round_new(
        &calls.env(),
        &calls.globals(),
        &RoundNewRequest {
            limit,
            git: calls.git_env(),
        },
    )
    .unwrap_or_else(|error| panic!("round new: {error}"));
    let mut text = render_json(&Outcome::RoundSheet(Box::new(sheet)));
    assert!(text.ends_with('\n'), "{text}");
    text.pop();
    text
}

/// The project's events as stored, through the CLI library.
fn events(calls: &Calls) -> Vec<(i64, String, String)> {
    events_after(&calls.env(), &calls.globals(), Some(0))
        .expect("the events read")
        .events
        .into_iter()
        .map(|event| (event.seq, event.event_type, event.payload))
        .collect()
}

/// The events stored after the first `before` of them.
fn added(calls: &Calls, before: &[(i64, String, String)]) -> Vec<(String, Value)> {
    let after = events(calls);
    assert_eq!(after[..before.len()], before[..], "the earlier events kept");
    after[before.len()..]
        .iter()
        .map(|(_, kind, payload)| {
            (
                kind.clone(),
                serde_json::from_str(payload).expect("a JSON payload"),
            )
        })
        .collect()
}

/// Everything R-05 names about the repository: `git status`, `HEAD`, the
/// refs, the worktrees, every file but the index.
fn repository(scratch: &Scratch, root: &Path) -> Vec<String> {
    let git = scratch.git();
    let files: Vec<String> = snapshot(root)
        .into_iter()
        .filter(|(path, _)| path != ".git/index")
        .map(|(path, bytes)| format!("{path} {:?}", bytes.map(|bytes| bytes.len())))
        .collect();
    vec![
        git.run(root, &["status", "--porcelain", "--untracked-files=all"]),
        git.run(root, &["rev-parse", "HEAD"]),
        git.run(root, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        git.run(root, &["worktree", "list", "--porcelain"]),
        files.join("\n"),
    ]
}

/// The sheet the daemon serves (200).
fn sheet(server: &Server) -> Value {
    server
        .get(&format!("{P}/round?limit=200"))
        .status(200)
        .json()
}

/// The `updated_at` the sheet shows for `id`.
fn shown(sheet: &Value, id: &str) -> String {
    sheet["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["id"] == json!(id))
        .unwrap_or_else(|| panic!("{id} not on the sheet: {sheet}"))["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned()
}

/// `ids` as owned strings (to compare with [`item_ids`]).
fn owned(ids: &[&String]) -> Vec<String> {
    ids.iter().map(|id| (*id).clone()).collect()
}

fn item_ids(sheet: &Value) -> Vec<String> {
    sheet["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// The review document of `id` as the daemon serves it.
fn review(server: &Server, id: &str) -> Value {
    server
        .get(&format!("{P}/proposals/{id}"))
        .status(200)
        .json()
}

fn is_utc(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[10] == b'T'
        && bytes[19] == b'Z'
        && text
            .chars()
            .enumerate()
            .all(|(at, c)| matches!(at, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit())
}

// --------------------------------------------------------------- answers

/// An approve of `id` (each flag as given, `null` else) against `at`.
fn approve(id: &str, option: Option<u64>, answer: Option<&str>, at: &str) -> Value {
    json!({"id": id, "stage": {"decision": "approve", "option": option, "answer": answer,
        "canon": null, "note": null, "updated_at": at}})
}

/// A reject of `id` with `reason` against `at`.
fn reject(id: &str, reason: &str, at: &str) -> Value {
    json!({"id": id, "stage": {"decision": "reject", "reason": reason, "updated_at": at}})
}

/// A same-origin page's POST of `answers` for `project`.
fn post(server: &Server, project: &str, answers: &[Value]) -> Reply {
    let body = json!({"project": project, "answers": answers}).to_string();
    server.stage(ANSWERS, &body)
}

/// The answers document of a 200: its `answers`, checked for its frame.
fn entries(reply: &Reply) -> Vec<Value> {
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8")
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    assert!(reply.cors_headers().is_empty(), "{reply:?}");
    let document = reply.json();
    let keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
    assert_eq!(keys.len(), 3, "{document}");
    assert_eq!(document["project"], json!(SLUG));
    assert_eq!(document["notes"], json!([]), "{document}");
    document["answers"].as_array().expect("answers").clone()
}

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
    String::from_utf8(output.stdout)
        .expect("sqlite3 output")
        .trim()
        .to_owned()
}

/// `id|status|staged|staged_at|updated_at` of every row, by ID.
fn rows(home: &Path) -> String {
    sqlite3(
        &data_dir(home).join(format!("{SLUG}.db")),
        "SELECT id, status, coalesce(staged, '-'), coalesce(staged_at, '-'), updated_at \
         FROM proposals ORDER BY id;",
    )
}

/// Writes `bytes` (a write cut short by the daemon's early answer is no
/// failure) and reads the answer to its end within 60 s: its status and
/// its text.
fn lenient_exchange(port: u16, bytes: &[u8]) -> (u16, String) {
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_write_timeout(Some(Duration::from_secs(60)))
        .expect("timeout");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .expect("timeout");
    let _ = stream.write_all(bytes);
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line: {:?}", &text[..text.len().min(200)]));
    (status, text)
}

// ------------------------------------------------------------------ R-01

#[test]
fn r01_the_sheet_is_the_cli_documents_bytes_and_limit_cuts_it() {
    let s = four("round-r01");
    let before = events(&s.calls);
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let whole = server.get(&format!("{P}/round"));
    assert_eq!(whole.status, 200, "{}", whole.text());
    assert_eq!(
        whole.header("content-type"),
        Some("application/json; charset=utf-8")
    );
    assert_eq!(whole.header("cache-control"), Some("no-store"));
    assert!(whole.cors_headers().is_empty(), "{whole:?}");
    let one = server.get(&format!("{P}/round?limit=1"));
    assert_eq!(one.status, 200, "{}", one.text());
    let all = server.get(&format!("{P}/round?limit=200"));
    assert_eq!(all.status, 200, "{}", all.text());
    // The library's documents, after the daemon's reads (one index, one
    // queue): byte for byte.
    assert_eq!(whole.text(), library_sheet(&s.calls, None), "no limit");
    assert_eq!(one.text(), library_sheet(&s.calls, Some(1)), "limit=1");
    assert_eq!(all.text(), library_sheet(&s.calls, Some(200)), "limit=200");

    // What the bytes hold: the round order, the waiting task, the cut.
    let document = whole.json();
    assert_eq!(document["project"], json!(SLUG));
    assert_eq!(
        item_ids(&document),
        owned(&[&s.d1, &s.q1, &s.q2, &s.d2]),
        "severity, then waiting tasks, then age"
    );
    assert_eq!(document["more"], json!(0));
    assert_eq!(
        document["items"][1]["tasks"],
        json!([{"id": s.task, "status": "ready", "title": "Tune the stamina"}]),
        "{document}"
    );
    assert_eq!(document["items"][2]["tasks"], json!([]));
    let cut = one.json();
    assert_eq!(item_ids(&cut), owned(&[&s.d1]));
    assert_eq!(cut["more"], json!(3), "{cut}");
    assert!(
        cut["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| note == "3 more item(s) not on this sheet: spec round new --limit 4"),
        "{cut}"
    );
    assert_ne!(one.text(), whole.text(), "`limit` is read");
    assert_eq!(
        added(&s.calls, &before),
        Vec::<(String, Value)>::new(),
        "a sheet writes no event"
    );
    drop(server);

    // An empty queue: 200, the library's bytes.
    let scratch = Scratch::new("round-r01-empty");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let calls = Calls::new(&scratch, &a, &home);
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let empty = server.get(&format!("{P}/round"));
    assert_eq!(empty.status, 200, "{}", empty.text());
    assert_eq!(empty.text(), library_sheet(&calls, None));
    let document = empty.json();
    assert_eq!(document["items"], json!([]), "{document}");
    assert_eq!(document["more"], json!(0));
    assert!(events(&calls).is_empty(), "no event");
}

// ------------------------------------------------------------------ R-02

#[test]
fn r02_a_bad_limit_or_another_name_is_a_400_and_runs_nothing() {
    let scratch = Scratch::new("round-r02");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    for (query, message) in [
        ("limit=0", "spec: --limit 0: the limit is 1 to 200"),
        ("limit=201", "spec: --limit 201: the limit is 1 to 200"),
        ("limit=-3", "spec: --limit -3: the limit is 1 to 200"),
        ("limit=x", "`limit=x`: not a decimal integer"),
        (
            "limit=1&limit=2",
            "`limit` is given 2 times: it takes one value",
        ),
        (
            "depth=1",
            "`depth` is no query name of round: it takes limit",
        ),
    ] {
        let reply = server.get(&format!("{P}/round?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        assert_eq!(reply.error_message(), message, "{query}");
        assert!(reply.cors_headers().is_empty(), "{query}");
    }
    assert!(
        snapshot(&home).is_empty(),
        "a refused `limit` runs nothing: the fresh HOME holds {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
    // The bounds are taken (the control): 1 and 200.
    for query in ["limit=1", "limit=200"] {
        let reply = server.get(&format!("{P}/round?{query}"));
        assert_eq!(reply.status, 200, "{query}: {}", reply.text());
    }
    let calls = Calls::new(&scratch, &a, &home);
    assert!(events(&calls).is_empty(), "no event");
}

// ------------------------------------------------------------------ R-04

/// A transport case: its name, the headers, the body, the status, the
/// error body's message, whole (`true`) or its start when it ends in
/// serde's words (`false`).
type TransportCase<'a> = (&'a str, Vec<(&'a str, &'a str)>, String, u16, String, bool);

#[test]
fn r04_the_transport_refuses_before_anything_is_read_or_stored() {
    let s = four("round-r04");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let read = sheet(&server);
    let good = vec![approve(&s.q1, None, None, &shown(&read, &s.q1))];
    let good_body = json!({"project": SLUG, "answers": good}).to_string();
    let before_events = events(&s.calls);
    let before_rows = rows(&s.home);
    let host = format!("127.0.0.1:{}", server.port);
    let own = format!("http://127.0.0.1:{}", server.port);
    let same = ("Sec-Fetch-Site", "same-origin");
    let json_type = ("Content-Type", "application/json");
    let over = (CAP + 1).to_string();

    // An over-cap `Content-Length` from no page: the page is judged first
    // (403), the body never read; answered at once.
    let started = Instant::now();
    let mut bytes = request_bytes(
        "POST",
        ANSWERS,
        &[
            ("Host", &host),
            json_type,
            ("Content-Length", over.as_str()),
        ],
    );
    bytes.extend_from_slice(b"{}");
    let reply = http_exchange(server.port, &bytes, Duration::from_secs(20));
    assert_eq!(reply.status, 403, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        "staging needs a same-origin page (not authentication: ADR-0034)"
    );
    // The same from a same-origin page: 413, unread (the 4 MiB never sent).
    let mut bytes = request_bytes(
        "POST",
        ANSWERS,
        &[
            ("Host", &host),
            same,
            json_type,
            ("Content-Length", over.as_str()),
        ],
    );
    bytes.extend_from_slice(b"{}");
    let reply = http_exchange(server.port, &bytes, Duration::from_secs(20));
    assert_eq!(reply.status, 413, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        format!("the body is over {CAP} bytes")
    );
    let quick = started.elapsed();
    assert!(
        quick < Duration::from_secs(15),
        "a declared over-cap body is refused unread: {quick:?}"
    );

    // Sent over the cap with no length declared (chunked): 413 once the
    // bytes read pass it.
    let pad = format!(
        "{{\"project\":\"{SLUG}\",\"answers\":[],\"pad\":\"{}\"}}",
        "x".repeat(CAP)
    );
    let mut bytes = request_bytes(
        "POST",
        ANSWERS,
        &[
            ("Host", &host),
            same,
            json_type,
            ("Transfer-Encoding", "chunked"),
        ],
    );
    bytes.extend_from_slice(format!("{:x}\r\n", pad.len()).as_bytes());
    bytes.extend_from_slice(pad.as_bytes());
    bytes.extend_from_slice(b"\r\n0\r\n\r\n");
    let (status, answer) = lenient_exchange(server.port, &bytes);
    assert_eq!(status, 413, "{answer}");
    assert!(
        answer.contains(&format!("\"message\":\"the body is over {CAP} bytes\"")),
        "{answer}"
    );

    // The rest of the door, in its order.
    let two_hundred_one: Vec<Value> = (1..=201)
        .map(|n| approve(&format!("PR-{:04}", 1000 + n), None, None, OLDER))
        .collect();
    let two_hundred_one = json!({"project": SLUG, "answers": two_hundred_one}).to_string();
    let decode = |why: &str| {
        format!(
            "spec: the answers are not `{{\"project\",\"answers\":[{{\"id\",\"stage\"}}]}}` as \
             JSON: {why}"
        )
    };
    let cases: Vec<TransportCase<'_>> = vec![
        (
            "no Sec-Fetch-Site",
            vec![("Origin", own.as_str()), json_type],
            good_body.clone(),
            403,
            "staging needs a same-origin page (not authentication: ADR-0034)".to_owned(),
            true,
        ),
        (
            "Sec-Fetch-Site: none",
            vec![("Sec-Fetch-Site", "none"), json_type],
            good_body.clone(),
            403,
            "staging needs a same-origin page (not authentication: ADR-0034)".to_owned(),
            true,
        ),
        (
            "Sec-Fetch-Site: cross-site (the fence)",
            vec![("Sec-Fetch-Site", "cross-site"), json_type],
            good_body.clone(),
            403,
            "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`".to_owned(),
            true,
        ),
        (
            "text/plain",
            vec![same, ("Content-Type", "text/plain")],
            good_body.clone(),
            415,
            "the body is JSON: send it with Content-Type: application/json".to_owned(),
            true,
        ),
        (
            "no Content-Type",
            vec![same],
            good_body.clone(),
            415,
            "the body is JSON: send it with Content-Type: application/json".to_owned(),
            true,
        ),
        (
            "no project",
            vec![same, json_type],
            "{\"answers\":[{\"id\":\"PR-0001\"}]}".to_owned(),
            400,
            decode(""),
            false,
        ),
        (
            "not JSON",
            vec![same, json_type],
            "{\"project\":".to_owned(),
            400,
            decode(""),
            false,
        ),
        (
            "an unknown key",
            vec![same, json_type],
            format!("{{\"project\":\"{SLUG}\",\"answers\":[],\"consent\":true}}"),
            400,
            decode(""),
            false,
        ),
        (
            "an unknown key of an answer",
            vec![same, json_type],
            format!(
                "{{\"project\":\"{SLUG}\",\"answers\":[{{\"id\":\"PR-0001\",\"stage\":{{\
                 \"decision\":\"reject\",\"reason\":\"r\",\"updated_at\":\"{OLDER}\"}},\
                 \"apply\":true}}]}}"
            ),
            400,
            decode(""),
            false,
        ),
        (
            "a stage that is none",
            vec![same, json_type],
            format!(
                "{{\"project\":\"{SLUG}\",\"answers\":[{{\"id\":\"PR-0001\",\"stage\":{{\
                 \"decision\":\"defer\",\"updated_at\":\"{OLDER}\"}}}}]}}"
            ),
            400,
            decode(""),
            false,
        ),
        (
            "another slug",
            vec![same, json_type],
            "{\"project\":\"zerkalo\",\"answers\":[]}".to_owned(),
            400,
            "spec: the answers are for `zerkalo`, this is `lantern-keep`; nothing changed"
                .to_owned(),
            true,
        ),
        (
            "201 answers",
            vec![same, json_type],
            two_hundred_one,
            400,
            "spec: 201 answers; at most 200 at once, a sheet's most items: send the rest after; \
             nothing changed"
                .to_owned(),
            true,
        ),
    ];
    for (name, headers, body, status, message, exact) in &cases {
        let reply = server.send("POST", ANSWERS, headers, body.as_bytes());
        assert_eq!(reply.status, *status, "{name}: {}", reply.text());
        assert!(reply.cors_headers().is_empty(), "{name}: {reply:?}");
        assert_eq!(reply.header("allow"), None, "{name}");
        let got = reply.error_message();
        if *exact {
            assert_eq!(&got, message, "{name}");
        } else {
            assert!(
                got.starts_with(message.as_str()) && got.ends_with("; nothing changed"),
                "{name}: {got}"
            );
        }
        assert_eq!(rows(&s.home), before_rows, "{name}: nothing stored");
    }
    assert_eq!(
        added(&s.calls, &before_events),
        Vec::<(String, Value)>::new(),
        "no event"
    );

    // The bounds taken: no answer (W-4) and 200 answers, each of no
    // proposal: 200, nothing staged, no event.
    let reply = post(&server, SLUG, &[]);
    assert_eq!(
        reply.text(),
        "{\"project\":\"lantern-keep\",\"answers\":[],\"notes\":[]}",
        "{reply:?}"
    );
    let unknown: Vec<Value> = (1..=200)
        .map(|n| reject(&format!("PR-{:04}", 1000 + n), "gone", OLDER))
        .collect();
    let reply = post(&server, SLUG, &unknown);
    let answered = entries(&reply);
    assert_eq!(answered.len(), 200);
    for (n, entry) in (1..=200).zip(&answered) {
        let id = format!("PR-{:04}", 1000 + n);
        assert_eq!(
            entry,
            &json!({"id": id, "outcome": "refused", "stage": null, "staged_at": null,
                "replaced": null, "reason": format!("no proposal `{id}` in this project's queue")}),
        );
    }
    assert_eq!(rows(&s.home), before_rows, "nothing stored");
    assert_eq!(
        added(&s.calls, &before_events),
        Vec::<(String, Value)>::new(),
        "no event"
    );
}

// ------------------------------------------------------- R-05 and T-03

/// The owner's terminal of a library walk: `n` to every question.
#[derive(Default)]
struct No {
    reports: Vec<String>,
    questions: Vec<String>,
    told: Vec<String>,
}

impl RoundTerminal for No {
    fn report(&mut self, text: &str) {
        self.reports.push(text.to_owned());
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        self.questions.push(question.to_owned());
        Some(false)
    }

    fn tell(&mut self, line: &str) {
        self.told.push(line.to_owned());
    }
}

#[test]
fn r05_shuffled_answers_are_staged_in_round_order_and_change_nothing_else() {
    let s = four("round-r05");
    let before = repository(&s.scratch, &s.a);
    assert_eq!(before[0], "", "the copy is committed");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let read = sheet(&server);
    let order = [&s.d1, &s.q1, &s.q2, &s.d2];
    assert_eq!(item_ids(&read), owned(&order), "the round order");
    let stored = events(&s.calls);
    let own = "No: walking is 2 m/s.\nSprinting is 4 m/s.";
    let reason = "Not a gap: the bar empties once.";
    // Request order: q2, d2, q1, d1 (every item off its round place).
    let answers = [
        approve(&s.q2, None, Some(own), &shown(&read, &s.q2)),
        reject(&s.d2, reason, &shown(&read, &s.d2)),
        approve(&s.q1, None, None, &shown(&read, &s.q1)),
        approve(&s.d1, Some(1), None, &shown(&read, &s.d1)),
    ];
    let reply = post(&server, SLUG, &answers);
    let answered = entries(&reply);
    let at = answered[0]["staged_at"]
        .as_str()
        .expect("staged_at")
        .to_owned();
    assert!(is_utc(&at), "{at}");
    let stages = [
        json!({"decision": "approve", "option": null, "answer": own, "canon": null,
            "note": null, "span_hash": null}),
        json!({"decision": "reject", "reason": reason}),
        json!({"decision": "approve", "option": null, "answer": null, "canon": null,
            "note": null, "span_hash": null}),
        json!({"decision": "approve", "option": 1, "answer": null, "canon": null,
            "note": null, "span_hash": null}),
    ];
    let requested = [&s.q2, &s.d2, &s.q1, &s.d1];
    assert_eq!(answered.len(), 4, "{}", reply.text());
    for ((entry, id), stage) in answered.iter().zip(requested).zip(&stages) {
        assert_eq!(
            entry,
            &json!({"id": id, "outcome": "staged", "stage": stage, "staged_at": at,
                "replaced": null, "reason": null}),
            "{id}: request order, one clock reading"
        );
    }
    // The stages' bytes in the document, keys in the canon's order.
    assert!(
        reply.text().contains(
            "\"stage\":{\"decision\":\"approve\",\"option\":null,\"answer\":\"No: walking is 2 \
             m/s.\\nSprinting is 4 m/s.\",\"canon\":null,\"note\":null,\"span_hash\":null}"
        ),
        "{}",
        reply.text()
    );
    // Four `proposal.staged`, in round order, nothing else.
    let new = added(&s.calls, &stored);
    let kinds: Vec<&str> = new.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(kinds, ["proposal.staged"; 4], "{new:?}");
    let ids: Vec<&str> = new
        .iter()
        .map(|(_, payload)| payload["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(
        ids,
        order.map(String::as_str),
        "staged in round order, not request order"
    );
    for (_, payload) in &new {
        let id = payload["id"].as_str().unwrap();
        let index = requested.iter().position(|r| r.as_str() == id).unwrap();
        assert_eq!(
            payload,
            &json!({"id": id, "staged": stages[index], "staged_at": at}),
            "{id}'s event"
        );
    }
    // Each still open, staged as answered; the repository untouched.
    for (id, stage) in requested.iter().zip(&stages) {
        let document = review(&server, id);
        assert_eq!(document["status"], json!("open"), "{id}");
        assert_eq!(&document["staged"], stage, "{id}");
        assert_eq!(document["staged_at"], json!(at), "{id}");
        assert!(document["decided_by"].is_null() && document["applied_commit"].is_null());
    }
    assert_eq!(
        repository(&s.scratch, &s.a),
        before,
        "git and the files as they were: no record file, no commit"
    );
    drop(server);

    // T-03: the terminal asks exactly these four, in round order, each as
    // staged; `n` keeps them all.
    let mut terminal = No::default();
    let run = round_answer(
        &s.calls.env(),
        &s.calls.globals(),
        &RoundAnswerRequest {
            file: None,
            now: "2026-10-08T12:00:00Z".to_owned(),
            git: s.calls.git_env(),
        },
        &mut terminal,
    )
    .unwrap_or_else(|error| panic!("round answer --staged: {error}"));
    assert!(terminal.reports.is_empty(), "{:?}", terminal.reports);
    assert_eq!(terminal.questions.len(), 4, "{:#?}", terminal.questions);
    for (question, id) in terminal.questions.iter().zip(order) {
        let asked = if id == &s.d2 {
            format!("reject {id} (")
        } else {
            format!("apply {id} as ")
        };
        assert!(question.contains(&asked), "{id}: {question}");
        assert!(question.ends_with("? [y/N]"), "{id}: {question}");
        let last = question.lines().last().unwrap_or_default();
        assert!(last.contains(", staged)"), "{id}: {last}");
    }
    assert_eq!(
        run.walk
            .iter()
            .map(|step| (step.id.as_str(), step.outcome))
            .collect::<Vec<_>>(),
        order
            .iter()
            .map(|id| (id.as_str(), WalkOutcome::Kept))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        added(&s.calls, &stored).len(),
        4,
        "the walk added no event to the four"
    );
    assert_eq!(repository(&s.scratch, &s.a), before);
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    for (id, stage) in requested.iter().zip(&stages) {
        let document = review(&server, id);
        assert_eq!(document["status"], json!("open"), "{id} kept");
        assert_eq!(&document["staged"], stage, "{id} kept staged");
    }
}

// ------------------------------------------------------------------ R-06

#[test]
fn r06_each_unfit_answer_is_refused_alone_and_the_valid_ones_staged() {
    let scratch = Scratch::new("round-r06");
    let a = scratch.repo("spec-a", "a", "main");
    let git = scratch.git();
    git.run(&a, &["config", "user.name", "Owner"]);
    git.run(&a, &["config", "user.email", "owner@example.invalid"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let calls = Calls::new(&scratch, &a, &home);
    let ask = |text: &str, target: &str| question(&calls, &[target], text, None, OLDER, None);
    let valid_q = ask("Does regeneration wait for rest?", "RULE-STAM-REGEN");
    let valid_d = gap(
        &calls,
        &["MEC-SPRINT"],
        "Walking drains stamina in the build.",
        IntakeSeverity::High,
    );
    let stale = ask("Is the walk speed fixed?", "RULE-MOVE-SPEEDS");
    // An update, applied (the library's approve, consented).
    let applied = common::propose_update(
        &home,
        &cwd,
        &a,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.2 s.\n",
    );
    let outcome = specengine_cli::approve(
        &calls.env(),
        &calls.globals(),
        &ApproveRequest {
            id: applied.clone(),
            note: None,
            now: OLDER.to_owned(),
            git: calls.git_env(),
        },
        &mut |_: &str| true,
    )
    .expect("approve the update");
    let commit = outcome.document.applied_commit.clone().expect("applied");
    // An update held (`approved`, as a run left it after its step 7: the
    // status is judged before the kind).
    let held = common::propose_update(
        &home,
        &cwd,
        &a,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.4 s.\n",
    );
    // Another existing repository of the project; one that is gone.
    let other = scratch.repo("spec-a", "other", "main");
    let elsewhere = question(
        &Calls::new(&scratch, &other, &home),
        &["RULE-STAM-REGEN"],
        "Is regeneration linear?",
        None,
        OLDER,
        None,
    );
    let gone_root = scratch.repo("spec-a", "gone", "main");
    let orphan = question(
        &Calls::new(&scratch, &gone_root, &home),
        &["RULE-STAM-REGEN"],
        "Is regeneration capped?",
        None,
        OLDER,
        None,
    );
    std::fs::remove_dir_all(&gone_root).expect("the repository is gone");
    let update = common::propose_update(
        &home,
        &cwd,
        &a,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.3 s.\n",
    );
    let look = ask("Does a sprint end at zero?", "MEC-SPRINT");
    let twice = ask("Does an empty bar stop the run?", "EDGE-STAM-ZERO");
    let long_answer = ask("Is the stamina bar visible?", "MEC-STAMINA");
    let long_reason = ask("Is the sprint cost per second?", "RULE-SPRINT-COST");
    let bidi = ask("Does the keeper rest at night?", "RULE-CORE-LOOP");
    let cr = ask("Does the lantern burn oil?", "MEC-STAMINA");
    let blank = ask("Is the walk animation tied to speed?", "RULE-MOVE-SPEEDS");
    let option5 = gap(
        &calls,
        &["RULE-STAM-REGEN"],
        "Regeneration runs while sprinting.",
        IntakeSeverity::Normal,
    );
    let own_text = gap(
        &calls,
        &["RULE-SPRINT-COST"],
        "The sprint costs twice as much.",
        IntakeSeverity::Normal,
    );
    let path_only = ask(
        "Is the tuning plan complete?",
        "docs/features/stamina-tuning.md",
    );
    // `held` approved.
    let db = data_dir(&home).join(format!("{SLUG}.db"));
    sqlite3(
        &db,
        &format!(
            "UPDATE proposals SET status = 'approved', decided_by = 'Owner \
             <owner@example.invalid>', decided_at = '{OLDER}', updated_at = '{OLDER}' \
             WHERE id = '{held}';"
        ),
    );
    let server = Server::serve(&home, &cwd, &[&a]);
    let read = sheet(&server);
    let at = |id: &str| shown(&read, id);
    // The items not on the sheet, as the queue holds them.
    let stored_at = |id: &str| {
        sqlite3(
            &db,
            &format!("SELECT updated_at FROM proposals WHERE id = '{id}';"),
        )
    };
    let stale_at = "2026-01-01T00:00:00Z";
    let latin = look.clone();
    let look_alike = latin.replacen('P', "\u{0420}", 1);
    let answers = vec![
        approve(&stale, None, None, stale_at),
        approve(&valid_q, None, None, &at(&valid_q)),
        approve(&applied, None, None, &stored_at(&applied)),
        approve(&held, None, None, &stored_at(&held)),
        approve(&elsewhere, None, None, &stored_at(&elsewhere)),
        approve(&orphan, None, None, &stored_at(&orphan)),
        approve(&update, None, None, &stored_at(&update)),
        approve("PR-9999", None, None, OLDER),
        approve(&look_alike, None, None, &at(&latin)),
        approve(&twice, None, None, &at(&twice)),
        approve(
            &long_answer,
            None,
            Some(&"x".repeat(2049)),
            &at(&long_answer),
        ),
        reject(&long_reason, &"x".repeat(4097), &at(&long_reason)),
        approve(&bidi, None, Some("a\u{202e}b"), &at(&bidi)),
        reject(&cr, "one\rtwo", &at(&cr)),
        reject(&blank, " ", &at(&blank)),
        approve(&option5, Some(5), None, &at(&option5)),
        approve(&own_text, None, Some("Keep both."), &at(&own_text)),
        approve(&path_only, None, None, &at(&path_only)),
        reject(&twice, "Asked before.", &at(&twice)),
        approve(&valid_d, Some(2), None, &at(&valid_d)),
    ];
    let stored = events(&calls);
    let before_files = repository(&scratch, &a);
    let reply = post(&server, SLUG, &answers);
    let answered = entries(&reply);
    assert_eq!(answered.len(), answers.len(), "{}", reply.text());
    let stale_now = stored_at(&stale);
    let reasons: Vec<(&str, String)> = vec![
        (
            stale.as_str(),
            format!(
                "`{stale}` changed since it was read: updated at {stale_now}, the choice was made \
                 on the one of {stale_at}; read it again"
            ),
        ),
        (valid_q.as_str(), String::new()),
        (
            applied.as_str(),
            format!("`{applied}` is applied: commit {commit}"),
        ),
        (
            held.as_str(),
            format!(
                "`{held}` is held (approved: an apply in progress or stopped): `spec approve \
                 {held}` completes it"
            ),
        ),
        (elsewhere.as_str(), "another repository".to_owned()),
        (orphan.as_str(), "no longer exists".to_owned()),
        (
            update.as_str(),
            format!("`{update}` is a change proposal (update): `spec approve {update}` decides it"),
        ),
        (
            "PR-9999",
            "no proposal `PR-9999` in this project's queue".to_owned(),
        ),
        (
            look_alike.as_str(),
            format!(
                "`{look_alike}` uses look-alike letters or digits; proposal IDs are Latin only: \
                 write `{latin}`"
            ),
        ),
        (
            twice.as_str(),
            "answered at positions 10 and 19: give one answer per item".to_owned(),
        ),
        (
            long_answer.as_str(),
            "--answer: 2049 bytes; at most 2048".to_owned(),
        ),
        (
            long_reason.as_str(),
            "--reason: 4097 bytes; at most 4096".to_owned(),
        ),
        (
            bidi.as_str(),
            "--answer: holds U+202E: a staged choice never carries it".to_owned(),
        ),
        (
            cr.as_str(),
            "--reason: holds U+000D: a staged choice never carries it".to_owned(),
        ),
        (
            blank.as_str(),
            "--reason is empty: say why the proposal is rejected".to_owned(),
        ),
        (
            option5.as_str(),
            format!("--option 5: `{option5}` has options 0-2"),
        ),
        (
            own_text.as_str(),
            format!(
                "`--answer` answers a question, and `{own_text}` is a discrepancy: name the \
                 owner's choice with `--option N` (0-2)"
            ),
        ),
        (
            path_only.as_str(),
            format!(
                "`{path_only}` names no ID, only a path: its record needs --canon: `spec approve \
                 {path_only} --canon REF`"
            ),
        ),
        (
            twice.as_str(),
            "answered at positions 10 and 19: give one answer per item".to_owned(),
        ),
        (valid_d.as_str(), String::new()),
    ];
    let mut staged = Vec::new();
    for (index, (entry, (id, reason))) in answered.iter().zip(&reasons).enumerate() {
        let context = format!("answer {} ({id}): {entry}", index + 1);
        assert_eq!(entry["id"], json!(id), "{context}: `id` as sent");
        if reason.is_empty() {
            assert_eq!(entry["outcome"], json!("staged"), "{context}");
            assert!(entry["reason"].is_null(), "{context}");
            staged.push(id.to_string());
            continue;
        }
        assert_eq!(entry["outcome"], json!("refused"), "{context}");
        assert!(
            entry["stage"].is_null() && entry["staged_at"].is_null() && entry["replaced"].is_null(),
            "{context}"
        );
        let got = entry["reason"].as_str().expect("a reason");
        assert!(
            !got.ends_with("; nothing changed") && !got.ends_with("; nothing staged"),
            "{context}: the tail is the document's to say"
        );
        if [elsewhere.as_str(), orphan.as_str()].contains(id) {
            assert!(got.contains(reason.as_str()), "{context}");
        } else {
            assert_eq!(got, reason, "{context}");
        }
    }
    assert!(
        answered[5]["reason"]
            .as_str()
            .unwrap()
            .starts_with(&format!("`{orphan}` belongs to the repository ")),
        "{}",
        answered[5]
    );
    assert!(
        answered[4]["reason"]
            .as_str()
            .unwrap()
            .contains(other.to_str().unwrap()),
        "{}",
        answered[4]
    );
    assert!(
        answered[8]["reason"].as_str().unwrap() != reasons[9].1,
        "a look-alike alone is no duplicate"
    );
    assert_eq!(staged, [valid_q.clone(), valid_d.clone()]);
    // Two events, in round order (the high discrepancy first).
    let new = added(&calls, &stored);
    let ids: Vec<(&str, &str)> = new
        .iter()
        .map(|(kind, payload)| (kind.as_str(), payload["id"].as_str().unwrap()))
        .collect();
    assert_eq!(
        ids,
        [
            ("proposal.staged", valid_d.as_str()),
            ("proposal.staged", valid_q.as_str())
        ]
    );
    // Nothing stored for a refused one: its row as it was.
    for id in [&stale, &twice, &look, &long_answer, &option5, &path_only] {
        assert!(review(&server, id)["staged"].is_null(), "{id}");
    }
    assert_eq!(repository(&scratch, &a), before_files);
    drop(server);

    // Without `[decision_records]`: an approve stops at apply step 3, a
    // reject still settles it (W-16).
    let bare = scratch.copy("spec-a", "bare");
    let config = std::fs::read_to_string(bare.join("specengine.toml")).expect("config");
    let cut = config.find("[decision_records]").expect("the table");
    std::fs::write(bare.join("specengine.toml"), &config[..cut]).expect("config");
    git.run(
        &bare,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&bare);
    git.run(&bare, &["add", "-A"]);
    git.run(&bare, &["commit", "-q", "-m", "fixture"]);
    let home2 = scratch.home("h2");
    let calls2 = Calls::new(&scratch, &bare, &home2);
    let qa = question(
        &calls2,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
        None,
        OLDER,
        None,
    );
    let qb = question(
        &calls2,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
        None,
        OLDER,
        None,
    );
    let server = Server::serve(&home2, &cwd, &[&bare]);
    let read = sheet(&server);
    let reply = post(
        &server,
        SLUG,
        &[
            approve(&qa, None, None, &shown(&read, &qa)),
            reject(&qb, "Asked before.", &shown(&read, &qb)),
        ],
    );
    let answered = entries(&reply);
    assert_eq!(answered[0]["outcome"], json!("refused"), "{}", reply.text());
    let got = answered[0]["reason"].as_str().unwrap();
    assert!(
        got.starts_with("not applied (step 3): ")
            && got.contains("has no `[decision_records]`")
            && got.ends_with("; a reject still settles it"),
        "{got}"
    );
    assert!(!got.contains("`reject: <reason>`"), "{got}");
    assert_eq!(answered[1]["outcome"], json!("staged"), "{}", reply.text());
    assert_eq!(
        answered[1]["stage"],
        json!({"decision": "reject", "reason": "Asked before."})
    );
}

// ------------------------------------------------------------------ R-07

#[test]
fn r07_an_answer_on_a_staged_item_names_the_stage_it_replaced() {
    let s = four("round-r07");
    // q1 staged earlier (the decision route's library call at an earlier
    // clock).
    let current = {
        let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
        shown(&sheet(&server), &s.q1)
    };
    let staged = stage(
        &s.calls.env(),
        &s.calls.globals(),
        &StageRequest {
            id: s.q1.clone(),
            body: StageBody::Approve {
                option: None,
                answer: None,
                canon: None,
                note: None,
                updated_at: current,
            },
            now: EARLIER.to_owned(),
            git: s.calls.git_env(),
        },
    )
    .expect("stage");
    assert_eq!(staged.cause, None, "{staged:?}");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let read = sheet(&server);
    let item = read["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == json!(s.q1))
        .unwrap()
        .clone();
    assert_eq!(item["staged_at"], json!(EARLIER), "{item}");
    let stored = events(&s.calls);
    let reply = post(
        &server,
        SLUG,
        &[reject(&s.q1, "Asked before.", &shown(&read, &s.q1))],
    );
    let answered = entries(&reply);
    let first = answered[0]["staged_at"].as_str().unwrap().to_owned();
    assert_ne!(first, EARLIER);
    assert_eq!(
        answered[0],
        json!({"id": s.q1, "outcome": "staged",
            "stage": {"decision": "reject", "reason": "Asked before."},
            "staged_at": first, "replaced": EARLIER, "reason": null}),
    );
    let new = added(&s.calls, &stored);
    assert_eq!(
        new,
        [(
            "proposal.staged".to_owned(),
            json!({"id": s.q1, "staged": {"decision": "reject", "reason": "Asked before."},
                "staged_at": first})
        )],
        "one event"
    );
    // Answered again from the sheet read now: it replaces the POST's own.
    let read = sheet(&server);
    let reply = post(
        &server,
        SLUG,
        &[approve(&s.q1, None, Some("Yes."), &shown(&read, &s.q1))],
    );
    let answered = entries(&reply);
    assert_eq!(answered[0]["outcome"], json!("staged"), "{}", reply.text());
    assert_eq!(answered[0]["replaced"], json!(first));
    assert_eq!(added(&s.calls, &stored).len(), 2, "one event each");
    // A first stage replaces none.
    let reply = post(
        &server,
        SLUG,
        &[approve(&s.q2, None, None, &shown(&read, &s.q2))],
    );
    let answered = entries(&reply);
    assert_eq!(answered[0]["outcome"], json!("staged"), "{}", reply.text());
    assert!(answered[0]["replaced"].is_null(), "{}", reply.text());
}
