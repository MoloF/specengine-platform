//! docs/features/hub-turn-fairness.md (RM-0119) "Acceptance criteria" TF1-TF9,
//! the Rust half: on a hub an MCP call that takes the turn first waits in its
//! login's FIFO line for the project, so a person's page, another login and
//! the `--push` sync wait for the turn behind at most the call running.
//! Through the real daemon over git copies of `fixtures/card-press` (a print
//! shop) and, for the sync, a repository holding it beside `fixtures/spec-a`
//! (a game's brief), each over a hub at `--slow-ms 0` so its slow lines
//! (`common/slow.rs`) show who held the turn. The agent and the page send the
//! same `Remote-User`, `mira`.
//!
//! The holder is the `probes` build's `probe_hold {ms, tag}`, answering `{tag,
//! seq}` with `seq` a process-wide count of its runs from 1, counted at the
//! start of the run: every order below is read from those counters (and from
//! the order of stderr lines), never from a clock. Clocks only stage the
//! sends: each test's first holder holds long enough (3 s, 8 s for the sync)
//! that everything after it is queued while it runs, which is checked; the
//! spec's 500 ms stands for every other hold. The `probes` tests are
//! `#[ignore]`d and run by `cargo nextest run -p specengine-http --features
//! specengine-mcp/probes --test hub_fairness --run-ignored ignored-only`, as
//! `tests/hub_trust.rs`'s panic probe is.
//!
//! - TF1 `tf1_*`: 8 parallel probes, then a page `GET …/inbox`: its 200
//!   before the probe of `seq` 2, under 3 holds. M: the page given a line by
//!   its login; no line at all.
//! - TF2 `tf2_*`: `mira`'s 8, then `ann`'s one: `a1`=1, `b1`=2, `a2`..`a8`=3..9.
//!   M: one line for every login.
//! - TF3 `tf3_*`: the same 9 all answered, each once, and the next run is the
//!   10th. M: a busy refusal past N waiters; the line released before the run.
//! - TF4 `tf4_*`: a probe and an `ask_question` cancelled in their line run
//!   never: the next in line is `seq` 2, no proposal stored, the next raise
//!   the hub's first ID. M: the line's wait outside the cancellation race.
//! - TF5 `tf5_*`: a `--push` hub, a page confirm during the first of 4 probes:
//!   the sync's `held` line after `a1`'s and before `a2`'s. See its note on the
//!   second project. M: the sync given a line.
//! - TF6 `tf6_*`: one legacy session's 5 probes run in send order. M: the line
//!   as a set.
//! - TF8 `tf8_*`: during TF1, the page's line ends `class=page line_ms=-
//!   ahead=1`, each probe's has `class=mcp`, an integer `line_ms` within its
//!   `wait_ms` and `tool=probe_hold`; the sync's turn lines in TF5 `class=sync`.
//!   M: `line_ms` folded into `wait_ms`.
//! - TF9 `tf9_*` (the default build, not ignored): a hub lists its 18 tools and
//!   no probe; `probe_hold` is rmcp's `tool not found`.
//!
//! TF7 (the lines' map emptied) is a unit test where `Project.lines` lives.

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::net::{Shutdown, TcpStream};
use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use common::hub::{Hub, ORIGIN, Proxied, Setup, events_of, hub_args, through};
use common::mcp_http::{
    Session, Stateless, content_text, is_error, mcp_path, message, post, posting, result_of,
    stateless_meta, tool_names,
};
use common::slow::{
    Fair, LINE_WAIT, Turn, requests_of, slow_lines, slow_ms, turns, turns_by, wait_for,
};
use common::{Scratch, Server, copy_dir, fixture, propose_update, read_text, spec_json, write};

/// The print shop, where the probes hold.
const SLUG: &str = "card-press";
const BASE: &str = "/api/projects/card-press";

/// The spec's hold.
const HOLD: u64 = 500;

/// The first holder's, long enough for every send after it to land while it
/// runs.
const FIRST: u64 = 3000;

/// Between two sends that must reach the daemon in this order.
const STAGGER: Duration = Duration::from_millis(100);

/// A call's route, the holder of its turn lines.
const MCP: &str = "/mcp/{p}";

// ------------------------------------------------------------- the probes

/// One `probe_hold`'s answer: its tag, its `seq` and when it came.
#[derive(Debug, Clone)]
struct Held {
    tag: String,
    seq: u64,
    at: Duration,
}

/// A `tools/call` of `probe_hold`'s params.
fn hold(ms: u64, tag: &str) -> Value {
    json!({"name": "probe_hold", "arguments": {"ms": ms, "tag": tag}})
}

/// A `probe_hold` result read: `{tag, seq}`, the tag the one sent.
fn held_of(result: &Value, tag: &str, at: Duration) -> Held {
    assert!(!is_error(result), "{tag}: an error result: {result}");
    let document = if result["structuredContent"].is_object() {
        result["structuredContent"].clone()
    } else {
        serde_json::from_str(&content_text(result))
            .unwrap_or_else(|error| panic!("{tag}: no {{tag, seq}} ({error}): {result}"))
    };
    assert_eq!(document["tag"], json!(tag), "{document}");
    Held {
        tag: tag.to_owned(),
        seq: document["seq"]
            .as_u64()
            .unwrap_or_else(|| panic!("{tag}: no seq: {document}")),
        at,
    }
}

/// `probe_hold {ms, tag}` by `login` on the print shop, stateless, in a
/// thread: a 200 with a result, read when it comes (from `start`).
fn probe(port: u16, login: &str, ms: u64, tag: &str, start: Instant) -> JoinHandle<Held> {
    let login = login.to_owned();
    let tag = tag.to_owned();
    thread::spawn(move || {
        let mut client = Stateless::new(port, SLUG, &login);
        let (id, answer) = client.send("tools/call", hold(ms, &tag));
        let at = start.elapsed();
        assert_eq!(answer.status(), 200, "{tag}: {}", answer.text());
        let response = answer.expect_response(id, &tag);
        held_of(result_of(&response, &tag), &tag, at)
    })
}

#[track_caller]
fn joined(handles: Vec<JoinHandle<Held>>) -> Vec<Held> {
    handles
        .into_iter()
        .map(|handle| handle.join().expect("a probe's client"))
        .collect()
}

/// A stateless `tools/call` of `name` by `login` on the print shop, written
/// whole on a connection of its own and left open: the caller closes it.
fn sent(port: u16, login: &str, name: &str, arguments: Value) -> TcpStream {
    let mut params = json!({"name": name, "arguments": arguments});
    params["_meta"] = stateless_meta();
    let headers = posting(Stateless::headers_as(login, "tools/call", &params));
    let body = message(1, "tools/call", params).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the hub");
    stream
        .write_all(&headers.bytes("POST", &mcp_path(SLUG), body.as_bytes()))
        .expect("send the call");
    stream
}

/// The client leaves.
fn close(stream: TcpStream) {
    let _ = stream.shutdown(Shutdown::Both);
}

/// The `held` lines of the probes' calls on the print shop so far.
fn probe_holds(lines: &[common::slow::Slow]) -> usize {
    turns(lines)
        .iter()
        .filter(|turn| {
            turn.by == MCP
                && turn.project == SLUG
                && turn.tool == "probe_hold"
                && turn.state == "held"
        })
        .count()
}

/// Until the `n`-th probe has taken the turn (its `held` line at
/// `--slow-ms 0`, printed as the hold starts).
fn started(server: &Server, n: usize) {
    wait_for(
        server,
        LINE_WAIT,
        &format!("probe hold {n}'s held line"),
        |lines| (probe_holds(lines) >= n).then_some(()),
    );
}

/// A hub over the print shop at `--slow-ms 0`.
fn fair_hub(name: &str) -> Hub {
    let setup = Setup::new(name, "card-press");
    let server = Server::start(
        &setup.home,
        &setup.cwd,
        &slow_ms(hub_args(&[&setup.root], ORIGIN), "0"),
    );
    Hub { server, setup }
}

/// The probes build's premise: a hub lists `probe_hold`.
#[track_caller]
fn assert_probes(port: u16) {
    let tools = Stateless::new(port, SLUG, "mira").tools();
    assert!(
        tool_names(&tools).iter().any(|name| name == "probe_hold"),
        "premise: the probes build routes probe_hold on a hub: {:?}",
        tool_names(&tools)
    );
}

/// The seqs of `held` by tag.
fn seqs(held: &[Held]) -> Vec<(String, u64)> {
    let mut seqs: Vec<(String, u64)> = held.iter().map(|h| (h.tag.clone(), h.seq)).collect();
    seqs.sort();
    seqs
}

fn tagged(pairs: &[(&str, u64)]) -> Vec<(String, u64)> {
    let mut pairs: Vec<(String, u64)> = pairs
        .iter()
        .map(|(tag, seq)| ((*tag).to_owned(), *seq))
        .collect();
    pairs.sort();
    pairs
}

// ------------------------------------------------------------- TF1, TF8

/// TF1's run: 8 parallel probes of 500 ms by `mira`, tags `a1` to `a8`; once
/// the first holds (its `held` line), `mira`'s page sends `GET …/inbox`.
struct Burst {
    hub: Hub,
    probes: Vec<Held>,
    page_sent: Duration,
    page_at: Duration,
}

fn burst(name: &str) -> Burst {
    let hub = fair_hub(name);
    let port = hub.server.port;
    assert_probes(port);
    let start = Instant::now();
    let handles: Vec<_> = (1..=8)
        .map(|n| probe(port, "mira", HOLD, &format!("a{n}"), start))
        .collect();
    started(&hub.server, 1);
    let page_sent = start.elapsed();
    let reply = hub.get("mira", &format!("{BASE}/inbox"));
    let page_at = start.elapsed();
    assert_eq!(reply.status, 200, "the page's inbox: {}", reply.text());
    let probes = joined(handles);
    Burst {
        hub,
        probes,
        page_sent,
        page_at,
    }
}

/// TF1: the page goes second: its 200 comes before the answer of the probe
/// that ran second (`seq` 2), and within 3 holds of its send. M: the page
/// given a line keyed by its login; no line at all (it waits behind all 8).
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf1_the_page_goes_second_behind_an_agents_burst() {
    let burst = burst("fair-page");
    let second = burst
        .probes
        .iter()
        .find(|held| held.seq == 2)
        .unwrap_or_else(|| panic!("no probe ran second: {:?}", burst.probes));
    let report = format!(
        "page sent at {:?}, answered at {:?}; probes {:?}\n--- stderr\n{}",
        burst.page_sent,
        burst.page_at,
        burst.probes,
        burst.hub.server.stderr()
    );
    assert!(
        burst.page_at < second.at,
        "the page waited for a second agent call: {report}"
    );
    assert!(
        burst.page_at - burst.page_sent < Duration::from_millis(3 * HOLD),
        "the page waited 3 holds or more: {report}"
    );
}

/// TF8: during TF1 at `--slow-ms 0`, the page's request line ends
/// `class=page line_ms=- ahead=1`; each probe's line has `class=mcp`,
/// `tool=probe_hold` and an integer `line_ms` within its `wait_ms`, and the
/// last in line waited there at least 4 holds. M: `line_ms` folded into
/// `wait_ms` alone.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf8_the_slow_lines_name_class_line_and_ahead() {
    let burst = burst("fair-lines");
    let server = &burst.hub.server;
    let page = requests_of(server, "/api/projects/{p}/inbox", 1);
    assert_eq!(page.len(), 1, "{page:#?}");
    assert_eq!(
        page[0].fair,
        Some(Fair {
            class: "page".to_owned(),
            line_ms: None,
            ahead: Some(1),
        }),
        "{}",
        page[0].line
    );
    let calls: Vec<_> = requests_of(server, MCP, 8)
        .into_iter()
        .filter(|line| line.tool == "probe_hold")
        .collect();
    assert_eq!(calls.len(), 8, "{calls:#?}");
    let mut longest = 0;
    for call in &calls {
        let fair = call.fair.as_ref().expect("RM-0119's keys");
        assert_eq!(fair.class, "mcp", "{}", call.line);
        let line_ms = fair
            .line_ms
            .unwrap_or_else(|| panic!("line_ms is an integer: {}", call.line));
        let wait = call.wait_ms.expect("wait_ms");
        assert!(
            line_ms <= wait,
            "the line's wait is within wait_ms: {}",
            call.line
        );
        longest = longest.max(line_ms);
    }
    assert!(
        longest >= 4 * HOLD,
        "the last in line waited there {longest} ms, not its line's wait: {calls:#?}"
    );
}

// ------------------------------------------------------------- TF2, TF3

/// TF2's run: `mira`'s `a1` (3 s), then `a2` to `a8` (500 ms) and `ann`'s
/// `b1` (500 ms), each sent [`STAGGER`] after the one before, all while `a1`
/// holds; then a last probe `z`, sent after all answered.
struct Mixed {
    hub: Hub,
    held: Vec<Held>,
    z: Held,
}

fn mixed(name: &str) -> Mixed {
    let hub = fair_hub(name);
    let port = hub.server.port;
    assert_probes(port);
    let start = Instant::now();
    let mut handles = vec![probe(port, "mira", FIRST, "a1", start)];
    started(&hub.server, 1);
    for n in 2..=8 {
        thread::sleep(STAGGER);
        handles.push(probe(port, "mira", HOLD, &format!("a{n}"), start));
    }
    thread::sleep(STAGGER);
    handles.push(probe(port, "ann", HOLD, "b1", start));
    let all_sent = start.elapsed();
    let held = joined(handles);
    assert!(
        held[0].at > all_sent + STAGGER,
        "premise: every call was queued while a1 held: a1 answered at {:?}, the last sent at \
         {all_sent:?}",
        held[0].at
    );
    let z = joined(vec![probe(port, "mira", 0, "z", start)]).remove(0);
    Mixed { hub, held, z }
}

/// TF2: another login goes second: `a1`=1, `b1`=2, `a2` to `a8` = 3 to 9 in
/// tag order. M: one line for every login.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf2_another_login_goes_second() {
    let mixed = mixed("fair-logins");
    assert_eq!(
        seqs(&mixed.held),
        tagged(&[
            ("a1", 1),
            ("b1", 2),
            ("a2", 3),
            ("a3", 4),
            ("a4", 5),
            ("a5", 6),
            ("a6", 7),
            ("a7", 8),
            ("a8", 9),
        ]),
        "--- stderr\n{}",
        mixed.hub.server.stderr()
    );
}

/// TF3: all once, in order, none refused: each of TF2's 9 answered 200 with a
/// result ([`probe`]), the seqs 1 to 9 each once, and the counter ended at 9:
/// the next run is the 10th. M: a busy refusal past N waiters; a line
/// released before the run.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf3_every_call_runs_once_and_none_is_refused() {
    let mixed = mixed("fair-once");
    let mut ran: Vec<u64> = mixed.held.iter().map(|held| held.seq).collect();
    ran.sort_unstable();
    assert_eq!(ran, (1..=9).collect::<Vec<u64>>(), "{:?}", mixed.held);
    assert_eq!(mixed.z.seq, 10, "the counter ended at 9: {:?}", mixed.z);
    assert_eq!(
        probe_holds(&slow_lines(&mixed.hub.server.stderr())),
        10,
        "ten holds of the turn by the probes"
    );
}

// ------------------------------------------------------------------ TF4

/// TF4: cancelled in its line. `mira`'s `a1` holds (3 s); `a2`, then `a3`, are
/// sent behind it, and `a2`'s client closes its connection while it waits in
/// the line: `a1`=1, `a3`=2, and the next probe is 3 (the counter ended at
/// 2). Then `q1` holds and an `ask_question` sent behind it is closed in the
/// line: once `q1` is done, no proposal is stored, and the next raise is the
/// hub's first ID. M: the line's wait left outside the cancellation race.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf4_a_call_cancelled_in_its_line_never_runs() {
    let hub = fair_hub("fair-cancel");
    let port = hub.server.port;
    assert_probes(port);
    let start = Instant::now();
    let a1 = probe(port, "mira", FIRST, "a1", start);
    started(&hub.server, 1);
    thread::sleep(STAGGER);
    let a2 = sent(port, "mira", "probe_hold", json!({"ms": HOLD, "tag": "a2"}));
    thread::sleep(STAGGER);
    let a3 = probe(port, "mira", HOLD, "a3", start);
    thread::sleep(3 * STAGGER);
    close(a2);
    let closed_at = start.elapsed();
    let held = joined(vec![a1, a3]);
    assert!(
        closed_at + STAGGER < held[0].at,
        "premise: a2 left while a1 held: at {closed_at:?}, a1 answered at {:?}",
        held[0].at
    );
    assert_eq!(
        seqs(&held),
        tagged(&[("a1", 1), ("a3", 2)]),
        "--- stderr\n{}",
        hub.server.stderr()
    );
    let z = joined(vec![probe(port, "mira", 0, "z", start)]).remove(0);
    assert_eq!(z.seq, 3, "the counter ended at 2: a2 never ran");

    // A raise cancelled in its line stores nothing.
    let q1 = probe(port, "mira", FIRST, "q1", start);
    started(&hub.server, 4);
    thread::sleep(STAGGER);
    let question = json!({"node_ids": ["SHT-BACK"], "text": "Is the back page a full sheet?",
        "working_answer": "Keep it as written.", "price_of_other": "One sheet reset.",
        "author_role": "drafter", "author_model": "m-1", "run": "r-1"});
    let asked = sent(port, "mira", "ask_question", question.clone());
    thread::sleep(3 * STAGGER);
    close(asked);
    let closed_at = start.elapsed();
    let q1 = joined(vec![q1]).remove(0);
    assert!(
        closed_at + STAGGER < q1.at,
        "premise: the raise left while q1 held: at {closed_at:?}, q1 answered at {:?}",
        q1.at
    );
    assert_eq!(q1.seq, 4, "{q1:?}");
    thread::sleep(Duration::from_secs(1));
    let created = |hub: &Hub| -> Vec<Value> {
        events_of(&hub.setup.home, &hub.setup.root)
            .into_iter()
            .filter(|(kind, _)| kind == "proposal.created")
            .map(|(_, payload)| payload)
            .collect()
    };
    assert_eq!(
        created(&hub),
        Vec::<Value>::new(),
        "the cancelled raise stored a proposal"
    );
    let stored = Stateless::new(port, SLUG, "mira").call("ask_question", question);
    let stored = result_of(&stored, "ask_question");
    assert!(!is_error(stored), "{stored}");
    assert_eq!(
        stored["structuredContent"]["id"],
        json!("PR-100001"),
        "the next raise is the hub's first: {stored}"
    );
    assert_eq!(created(&hub).len(), 1, "one proposal, the last raise's");
}

// ------------------------------------------------------------------ TF5

/// The owner `mira` in a project's `[team]`.
const MIRA_OWNER: &str = "\n[team.mira]\nrole = \"owner\"\nemail = \"mira@example.org\"\n";

/// `git args` in `dir`, which must succeed.
fn git(scratch: &Scratch, dir: &Path, args: &[&str]) -> String {
    scratch.git().run(dir, args)
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// The status `GET <base>/sync` reads, by `mira`.
fn sync_status(server: &Server, base: &str) -> Value {
    let reply = through(
        server,
        "GET",
        &format!("{base}/sync"),
        &Proxied::fetch("mira"),
        b"",
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

/// The first `GET …/sync` whose `at` is set and differs from `before`'s.
fn sync_after(server: &Server, before: &Value) -> Value {
    let started = Instant::now();
    loop {
        let status = sync_status(server, BASE);
        if !status["at"].is_null() && status["at"] != before["at"] {
            return status;
        }
        assert!(
            started.elapsed() < LINE_WAIT,
            "no sync ended after {before}: {status}\n--- stderr\n{}",
            server.stderr()
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// TF5: the sync goes second. One repository holds the print shop (`press/`)
/// and the game's brief (`keep/`, `mira` its owner), pushed to a bare
/// `origin.git` in the scratch as `tests/hub_git.rs` does; the hub's checkout
/// a clone, served `press` then `keep`, `--push --slow-ms 0`. `mira` sends 4
/// probes of 1 s on the print shop, the first held 8 s; during it, her page
/// confirms a proposal on the brief, which commits and signals the sync
/// (docs/canon/hub-git.md "Sync", its **When**). The sync's merge takes the
/// turns of both projects in `--root` order: on the print shop its `held`
/// line comes after `a1`'s and before `a2`'s, while `a2` to `a4` wait in
/// `mira`'s line. Its turn lines carry `class=sync` (TF8).
///
/// The confirm is the second project's: a confirm on the print shop takes
/// its turn (`confirm.rs`), so it would queue behind `a1`, and `a2`, freed
/// from the line by `a1`'s end, would reach the turn before the sync the
/// confirm then signals; the spec's single-project wording cannot hold. M:
/// the sync given a line.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf5_the_sync_goes_second() {
    let scratch = Scratch::new("fair-sync");
    let dev = scratch.dir("dev");
    copy_dir(&fixture("card-press"), &dev.join("press"));
    copy_dir(&fixture("spec-a"), &dev.join("keep"));
    let mut config = read_text(&dev, "keep/specengine.toml");
    config.push_str(MIRA_OWNER);
    write(&dev, "keep/specengine.toml", config);
    git(
        &scratch,
        &dev,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    scratch.git().quiet(&dev);
    git(&scratch, &dev, &["config", "user.name", "Owner"]);
    git(
        &scratch,
        &dev,
        &["config", "user.email", "owner@example.invalid"],
    );
    git(&scratch, &dev, &["add", "-A"]);
    git(&scratch, &dev, &["commit", "-q", "-m", "two projects"]);
    let bare = scratch.dir("origin.git");
    git(
        &scratch,
        &bare,
        &[
            "init",
            "-q",
            "--bare",
            "--template=",
            "--initial-branch=main",
        ],
    );
    scratch.git().quiet(&bare);
    git(&scratch, &bare, &["config", "receive.autogc", "false"]);
    git(&scratch, &dev, &["remote", "add", "origin", text(&bare)]);
    git(&scratch, &dev, &["push", "-q", "-u", "origin", "main"]);
    let checkout = scratch.join("hub");
    git(
        &scratch,
        scratch.path(),
        &["clone", "-q", "--template=", text(&bare), text(&checkout)],
    );
    let checkout = fs::canonicalize(checkout).expect("the hub's checkout");
    scratch.git().quiet(&checkout);
    git(&scratch, &checkout, &["config", "user.name", "Owner"]);
    git(
        &scratch,
        &checkout,
        &["config", "user.email", "owner@example.invalid"],
    );
    let press = checkout.join("press");
    let keep = checkout.join("keep");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");

    // A proposal on the brief, raised on the hub's host.
    let shown = spec_json(&home, &cwd, &keep, &["show", "RULE-STAM-REGEN"]).json();
    let rule = shown["nodes"][0]["text"].as_str().expect("the rule's text");
    assert!(rule.contains("Base rate 10 units/s"), "{rule}");
    let id = propose_update(
        &home,
        &cwd,
        &keep,
        "RULE-STAM-REGEN",
        &rule.replacen("Base rate 10 units/s", "Base rate 12 units/s", 1),
    );

    let mut args = slow_ms(hub_args(&[&press, &keep], ORIGIN), "0");
    args.extend(["--push", "--push-deadline", "60"].map(OsStr::new));
    let server = Server::start(&home, &cwd, &args);
    let started_sync = sync_after(&server, &json!({"at": null}));
    assert_eq!(started_sync["state"], json!("ok"), "{started_sync}");
    assert_probes(server.port);

    // `mira` stages it and reads the confirm's question on her page.
    let page = "/api/projects/lantern-keep";
    let updated_at = spec_json(&home, &cwd, &keep, &["review", &id]).json()["updated_at"].clone();
    let stage = through(
        &server,
        "POST",
        &format!("{page}/proposals/{id}/decision"),
        &Proxied::write("mira"),
        json!({"decision": "approve", "updated_at": updated_at})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(stage.status, 200, "mira stages {id}: {}", stage.text());
    let route = format!("{page}/proposals/{id}/confirm");
    let read = through(&server, "GET", &route, &Proxied::fetch("mira"), b"");
    assert_eq!(read.status, 200, "mira reads {id}: {}", read.text());
    let read = read.json();
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]});

    let start = Instant::now();
    let mut handles = vec![probe(server.port, "mira", 8000, "a1", start)];
    started(&server, 1);
    for n in 2..=4 {
        thread::sleep(STAGGER);
        handles.push(probe(server.port, "mira", 1000, &format!("a{n}"), start));
    }
    let confirm = through(
        &server,
        "POST",
        &route,
        &Proxied::write("mira"),
        body.to_string().as_bytes(),
    );
    let confirmed_at = start.elapsed();
    assert_eq!(
        confirm.status,
        200,
        "mira confirms {id}: {}",
        confirm.text()
    );
    assert_eq!(
        confirm.json()["status"],
        json!("applied"),
        "{}",
        confirm.text()
    );
    let held = joined(handles);
    assert!(
        confirmed_at + Duration::from_secs(1) < held[0].at,
        "premise: the confirm ended while a1 held: at {confirmed_at:?}, a1 answered at {:?}",
        held[0].at
    );
    assert_eq!(
        seqs(&held),
        tagged(&[("a1", 1), ("a2", 2), ("a3", 3), ("a4", 4)])
    );
    let synced = sync_after(&server, &started_sync);
    assert_eq!(synced["state"], json!("ok"), "{synced}");

    let lines = slow_lines(&server.stderr());
    let order: Vec<String> = turns(&lines)
        .into_iter()
        .filter(|turn| turn.project == SLUG && turn.state == "held")
        .map(|turn| turn.by)
        .collect();
    assert_eq!(
        order,
        [MCP, "sync", MCP, MCP, MCP],
        "the print shop's holds in order\n--- stderr\n{}",
        server.stderr()
    );
    let sync: Vec<Turn> = turns_by(&server, "sync");
    assert_eq!(
        sync.len(),
        4,
        "held and released on each project: {sync:#?}"
    );
    for turn in &sync {
        assert_eq!(turn.class.as_deref(), Some("sync"), "TF8: {}", turn.line);
    }
}

// ------------------------------------------------------------------ TF6

/// TF6: order within a login. One legacy session of `mira` sends 5 probes
/// back to back, `p1` held 3 s and each next sent [`STAGGER`] after the one
/// before while it holds: their seqs follow the send order. M: the line as a
/// set, not FIFO.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_fairness --run-ignored ignored-only"]
fn tf6_a_sessions_calls_run_in_send_order() {
    let hub = fair_hub("fair-order");
    let port = hub.server.port;
    assert_probes(port);
    let session = Session::open(port, SLUG, "mira");
    let start = Instant::now();
    let send = |n: i64, ms: u64| {
        let headers = session.headers();
        let path = session.path.clone();
        let tag = format!("p{n}");
        thread::spawn(move || {
            let answer = post(
                port,
                &path,
                &headers,
                &message(n, "tools/call", hold(ms, &tag)),
            );
            let at = start.elapsed();
            assert_eq!(answer.status(), 200, "{tag}: {}", answer.text());
            let response = answer.expect_response(n, &tag);
            held_of(result_of(&response, &tag), &tag, at)
        })
    };
    let mut handles = vec![send(1, FIRST)];
    started(&hub.server, 1);
    for n in 2..=5 {
        thread::sleep(STAGGER);
        handles.push(send(n, HOLD));
    }
    let all_sent = start.elapsed();
    let held = joined(handles);
    assert!(
        all_sent + STAGGER < held[0].at,
        "premise: every call was sent while p1 held"
    );
    assert_eq!(
        held.iter().map(|h| h.seq).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5],
        "{held:?}"
    );
}

// ------------------------------------------------------------------ TF9

/// TF9, the default build (not ignored): a hub lists its 18 tools and no
/// probe, and `probe_hold` is rmcp's `-32602` `tool not found`. M:
/// `probe_hold` outside `cfg(feature = "probes")`.
#[test]
fn tf9_the_default_build_has_no_probe_hold() {
    let hub = fair_hub("fair-default");
    let mut client = Stateless::new(hub.server.port, SLUG, "mira");
    let names = tool_names(&client.tools());
    // 21 since docs/features/comments.md CM15.
    assert_eq!(names.len(), 21, "{names:?}");
    assert!(
        names.iter().all(|name| !name.starts_with("probe_")),
        "{names:?}"
    );
    let (id, answer) = client.send("tools/call", hold(HOLD, "x"));
    let response = answer.expect_response(id, "probe_hold");
    assert_eq!(response["error"]["code"], json!(-32602), "{response}");
    assert_eq!(
        response["error"]["message"],
        json!("tool not found"),
        "{response}"
    );
    assert_eq!(
        probe_holds(&slow_lines(&hub.server.stderr())),
        0,
        "nothing held the turn"
    );
}
