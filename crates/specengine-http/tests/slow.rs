//! docs/features/slow-requests.md (RM-0118) "Acceptance criteria" SR1-SR10:
//! the daemon names its slow requests and long turns on stderr. Through the
//! real binary, its stderr read as it runs, over git copies of
//! `fixtures/spec-a` (`lantern-keep`, a game's brief), `fixtures/spec-b`
//! (`zerkalo`, a worktree tool's spec), `fixtures/card-press` (a print shop,
//! the hub's) and a generated corpus of 4 000 node files whose first `tree`
//! is the forced slow holder (as `tests/turn.rs` makes one; the binary gets no
//! test hook). Every expected value is worked out from the spec's "Data".
//!
//! Every `specengine-http: slow ` line any test sees is parsed against
//! "Data"'s grammar (`common/slow.rs`): the keys in order, each value in its
//! charset, `at` to the millisecond, no byte outside 0x20-0x7E, never two
//! lines run together. docs/features/hub-turn-fairness.md (RM-0119) appends
//! `class=` to every line and `line_ms=`, `ahead=` to a request line; the
//! parser takes a line with or without them, but never a mix in one run.
//!
//! - SR1 `sr1_*`: one request line at `--slow-ms 0`, its keys, `wait_ms +
//!   run_ms <= ms`, `index=-` locally; one `held` then one `released` line
//!   by its template; four at once, at least one waiting, each sums within
//!   its `ms`. M: the raw path in `route`; `wait_ms` folded into `run_ms`.
//! - SR2 `sr2_*`: no `--slow-ms`, ten fast reads, no ` slow ` on stderr. M:
//!   the threshold ignored.
//! - SR3 `sr3_*`: a hub at `--slow-ms 0`, a search, a card write and an
//!   `/mcp` `search`: no login, query, header name, cookie, forwarded address
//!   or body byte on stderr; `tool=search`. M: the login added.
//! - SR4 `sr4_*`: the slow `tree`'s `held` line while it is unanswered; the
//!   inbox queued behind it waits, and runs briefly. M: `wait_ms` from the
//!   turn; `held` only at release.
//! - SR5 `sr5_*`: a queued request whose client leaves: `status=gone`,
//!   `run_ms=-`, `wait_ms >= 200`, no turn line. M: no line on drop.
//! - SR6 `sr6_*`: a hub's first `tree` `index=walked`, the second
//!   `index=trusted`. M: `index` from the route class.
//! - SR7 `sr7_*`: a `--push` hub over `tests/hub_git.rs`'s kind of remote (a
//!   bare repository in the scratch), two projects in one checkout, the
//!   start's sync merging: `by=sync` `held` and `released` for each, in
//!   `--root` order. M: the sync's hold unwatched.
//! - SR8 `sr8_*`: an open events tail, one keep-alive read, closed: no request
//!   line of its route. M: streams logged.
//! - SR9 `sr9_*`: `PR-0001%E2%80%AE` prints `id=?`; a 65-letter slug
//!   `project=?`, a 64-letter one itself. M: the raw segment printed.
//! - SR10 `sr10_*`: `--slow-ms -1`, `x`, `3600001` exit 2 with one stderr
//!   line, nothing created; `3600000` starts; `spec serve` passes no
//!   `--slow-ms`.
//!
//! SR11 is `tests/deploy.rs` `sr11_*`; SR12 is the gates' run.
//!
//! Beyond the criteria, from "Data" and "Rules and edge cases": a `held`
//! line's `held_ms` is the threshold (0 at `--slow-ms 0`); a turn line's `at`
//! is its hold's start, the same on `held` and `released`; a request that
//! takes no turn prints `wait_ms=- run_ms=-` (`…/sync`); a percent-encoded
//! slug is never decoded into the line ([`data_no_value_is_percent_decoded`]).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/write_cost_common/mod.rs"]
mod write_cost_common;

mod write_cost_hub;

use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use common::hub::{Hub, ORIGIN, P, Proxied, Setup, hub_args, through};
use common::mcp_http::Stateless;
use common::slow::{
    LINE_WAIT, Request, SETTLE, requests, requests_of, slow_lines, slow_ms, turns, turns_by,
    wait_for,
};
use common::{
    HTTP_TIMEOUT, Scratch, Server, Stream, ask, copy_dir, fixture, http_exchange, product_env,
    request_bytes, root_args, snapshot, spec_bin, write,
};

/// Node files in the generated corpus (two nodes each), as `tests/turn.rs`.
const FILES: usize = 4000;

const KEEP: &str = "/api/projects/lantern-keep";

const BIG: &str = "/api/projects/big-corpus";

/// `utc_now()`'s second, as `at`'s first 19 bytes are.
fn second_now() -> String {
    specengine_cli::utc_now()[..19].to_owned()
}

/// A local daemon over `roots` at `--slow-ms value`.
fn local(home: &Path, cwd: &Path, roots: &[&Path], value: &'static str) -> Server {
    Server::start(home, cwd, &slow_ms(root_args(roots), value))
}

/// A hub over the print shop at `--slow-ms 0`.
fn hub(name: &str) -> Hub {
    let setup = Setup::new(name, "card-press");
    let server = Server::start(
        &setup.home,
        &setup.cwd,
        &slow_ms(hub_args(&[&setup.root], ORIGIN), "0"),
    );
    Hub { server, setup }
}

/// `GET path` on `port` in a thread: its status and when its answer ended,
/// from `start`.
fn timed(port: u16, path: &str, start: Instant) -> thread::JoinHandle<(u16, Duration)> {
    let host = format!("127.0.0.1:{port}");
    let bytes = request_bytes("GET", path, &[("Host", &host)]);
    thread::spawn(move || {
        let reply = http_exchange(port, &bytes, HTTP_TIMEOUT);
        (reply.status, start.elapsed())
    })
}

/// A project of `FILES` generated mechanic files, committed once: one first
/// `tree` on a fresh `HOME` builds its whole index, the slow holder.
fn corpus(scratch: &Scratch) -> PathBuf {
    let root = scratch.dir("big");
    write(
        &root,
        "specengine.toml",
        "[project]\nname = \"big corpus\"\nslug = \"big-corpus\"\n\n[ids]\n\
         MEC = { kind = \"mechanic\", shape = \"name\" }\n\
         RULE = { kind = \"rule\", shape = \"name\" }\n",
    );
    for i in 0..FILES {
        write(
            &root,
            &format!("docs/spec/gen/g{i:05}.md"),
            format!(
                "---\nid: MEC-GEN-{i:05}\nclass: canon\nstatus: accepted\nowner: owner\n\
                 reviewed: 2026-09-20\n---\n\n# Generated mechanic {i}\n\n\
                 The lantern burns oil at rate {i} while the keeper walks the walls.\n\n\
                 ## Rule {{#RULE-GEN-{i:05}}}\n\n\
                 Stamina regenerates at rest after {} seconds, see MEC-GEN-{:05}.\n",
                i % 7,
                (i + 1) % FILES
            ),
        );
    }
    let git = scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "generated corpus"]);
    root
}

/// The slow `tree` of the generated corpus at `--slow-ms 200` (a fresh
/// `HOME`: it builds the whole index), sent at `start`, and the moment its
/// `held` line was first seen on stderr.
struct Slowed {
    server: Server,
    tree: thread::JoinHandle<(u16, Duration)>,
    start: Instant,
    held_seen: Duration,
    _scratch: Scratch,
}

fn slowed(name: &str) -> Slowed {
    let scratch = Scratch::new(name);
    let root = corpus(&scratch);
    let home = scratch.home("fresh");
    let server = local(&home, scratch.path(), &[&root], "200");
    let start = Instant::now();
    let tree = timed(server.port, &format!("{BIG}/tree"), start);
    let route = "/api/projects/{p}/tree";
    wait_for(&server, LINE_WAIT, "the slow tree's `held` line", |lines| {
        turns(lines)
            .iter()
            .any(|turn| turn.by == route && turn.state == "held")
            .then_some(())
    });
    let held_seen = start.elapsed();
    Slowed {
        server,
        tree,
        start,
        held_seen,
        _scratch: scratch,
    }
}

// ---------------------------------------------------------------- SR1

/// SR1: at `--slow-ms 0`, one `GET …/proposals/PR-0001` gives exactly one
/// request line: its method, template, segments, `tool=-`, `status=200`;
/// `wait_ms + run_ms <= ms`; `index=-` on a local daemon; `at` within the
/// request's second. One `held` (its `held_ms` 0) then one `released` line,
/// each `by=` the same template, `at` their hold's start. Then four at once:
/// at least one waited for the turn, and each line's two parts sum within its
/// `ms`. M: the raw path in `route`; `wait_ms` folded into `run_ms`.
#[test]
fn sr1_a_request_writes_one_line_in_the_grammar() {
    let scratch = Scratch::new("slow-line");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    assert_eq!(
        ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?"),
        "PR-0001"
    );
    let server = local(&home, scratch.path(), &[&a], "0");
    let template = "/api/projects/{p}/proposals/{id}";
    let before = second_now();
    let reply = server.get(&format!("{KEEP}/proposals/PR-0001"));
    let after = second_now();
    assert_eq!(reply.status, 200, "{}", reply.text());
    wait_for(&server, LINE_WAIT, "the released line", |lines| {
        turns(lines)
            .iter()
            .any(|turn| turn.state == "released")
            .then_some(())
    });
    let lines = requests_of(&server, template, 1);
    let all = slow_lines(&server.stderr());
    assert_eq!(
        requests(&all).len(),
        1,
        "exactly one request line:\n{}",
        server.stderr()
    );
    let line = &lines[0];
    assert_eq!(
        (
            line.method.as_str(),
            line.route.as_str(),
            line.project.as_str(),
            line.id.as_str(),
            line.tool.as_str(),
            line.status.as_str(),
            line.index.as_str(),
        ),
        ("GET", template, "lantern-keep", "PR-0001", "-", "200", "-"),
        "{}",
        line.line
    );
    let (Some(wait), Some(run)) = (line.wait_ms, line.run_ms) else {
        panic!("wait_ms and run_ms are integers: {}", line.line);
    };
    assert!(
        wait + run <= line.ms,
        "wait_ms + run_ms <= ms: {}",
        line.line
    );
    assert!(
        before.as_str() <= &line.at[..19] && &line.at[..19] <= after.as_str(),
        "at {} within [{before}, {after}]",
        line.at
    );

    let held = turns(&all);
    assert_eq!(
        held.iter()
            .map(|turn| turn.state.as_str())
            .collect::<Vec<_>>(),
        ["held", "released"],
        "one held, then one released:\n{}",
        server.stderr()
    );
    for turn in &held {
        assert_eq!(
            (
                turn.by.as_str(),
                turn.project.as_str(),
                turn.id.as_str(),
                turn.tool.as_str()
            ),
            (template, "lantern-keep", "PR-0001", "-"),
            "{}",
            turn.line
        );
    }
    assert_eq!(
        held[0].held_ms, 0,
        "`held` at --slow-ms 0: {}",
        held[0].line
    );
    assert!(
        held[1].held_ms <= line.ms,
        "the hold within the request: {} / {}",
        held[1].line,
        line.line
    );
    assert_eq!(held[0].at, held[1].at, "both name the hold's start");
    assert!(line.at <= held[0].at, "the request arrived before its hold");

    // Four at once: the turn makes three of them wait.
    let start = Instant::now();
    let four: Vec<_> = (0..4)
        .map(|_| timed(server.port, &format!("{KEEP}/proposals/PR-0001"), start))
        .collect();
    for one in four {
        assert_eq!(one.join().expect("a reader").0, 200);
    }
    let lines = requests_of(&server, template, 5);
    assert_eq!(lines.len(), 5, "one line each: {lines:#?}");
    for line in &lines {
        let (Some(wait), Some(run)) = (line.wait_ms, line.run_ms) else {
            panic!("wait_ms and run_ms are integers: {}", line.line);
        };
        assert!(
            wait + run <= line.ms,
            "wait_ms + run_ms <= ms: {}",
            line.line
        );
    }
    assert!(
        lines[1..].iter().any(|line| line.wait_ms > Some(0)),
        "four at once and none waited for the turn: {lines:#?}"
    );
}

// ---------------------------------------------------------------- SR2

/// SR2: with no `--slow-ms` (2 000), ten fast reads write no line holding
/// ` slow `. A first read builds the index and is not counted; each counted
/// read must take under 1.5 s at the client, else the machine is too busy to
/// judge. M: the threshold ignored.
#[test]
fn sr2_fast_reads_under_the_default_threshold_write_nothing() {
    let scratch = Scratch::new("slow-default");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&b]);
    let base = "/api/projects/zerkalo";
    server.get(&format!("{base}/tree"));
    thread::sleep(SETTLE);
    let offset = server.stderr().len();
    let mut times = Vec::new();
    for path in [
        "/api/projects".to_owned(),
        format!("{base}/tree"),
        format!("{base}/tree?depth=1"),
        format!("{base}/search?query=worktree"),
        format!("{base}/inbox"),
        format!("{base}/tasks"),
        format!("{base}/cards"),
        format!("{base}/cards/counts"),
        format!("{base}/workspaces"),
        format!("{base}/check"),
    ] {
        let started = Instant::now();
        let reply = server.get(&path);
        times.push((path, reply.status, started.elapsed()));
    }
    assert!(
        times
            .iter()
            .all(|(_, _, took)| *took < Duration::from_millis(1500)),
        "a read was not fast; the machine is too busy to judge the threshold: {times:?}"
    );
    thread::sleep(SETTLE);
    let stderr = server.stderr();
    assert!(
        !stderr[offset..].contains(" slow "),
        "a line under the default 2 000 ms: {times:?}\n--- stderr since the first read\n{}",
        &stderr[offset..]
    );
}

// ---------------------------------------------------------------- SR3

/// SR3: a hub at `--slow-ms 0` (`Remote-User: mira`, a session cookie, the
/// proxy's forwarded address) gets a search by `query=needle7`, one by
/// `q=needle7` (a 400), a card write whose body holds `"as":"mira"`, and an
/// `/mcp/{p}` `tools/call` of `search` with `needle8`. No stderr line holds
/// the login, the needles, `q=`, a header name, the cookie's value, the
/// forwarded address or a byte of a body; the MCP call's line has
/// `tool=search` (and its turn lines). M: the login added to the line.
#[test]
fn sr3_no_login_query_header_or_body_reaches_stderr() {
    let hub = hub("slow-privacy");
    let server = &hub.server;
    let reader = Proxied::fetch("mira").plus("Cookie", "authelia_session=crumb9");
    let search = through(
        server,
        "GET",
        &format!("{P}/search?query=needle7"),
        &reader,
        b"",
    );
    assert_eq!(search.status, 200, "{}", search.text());
    let unknown = through(
        server,
        "GET",
        &format!("{P}/search?q=needle7"),
        &reader,
        b"",
    );
    assert_eq!(unknown.status, 400, "{}", unknown.text());
    let body = json!({"title": "Quillfeather notice", "discipline": "copy", "as": "mira"});
    let card = through(
        server,
        "POST",
        &format!("{P}/cards"),
        &Proxied::write("mira").plus("Cookie", "authelia_session=crumb9"),
        body.to_string().as_bytes(),
    );
    assert_eq!(card.status, 200, "{}", card.text());
    let mut agent = Stateless::new(server.port, "card-press", "mira");
    let response = agent.call("search", json!({"query": "needle8"}));
    assert!(response.get("result").is_some(), "{response}");

    requests_of(server, "/api/projects/{p}/search", 2);
    requests_of(server, "/api/projects/{p}/cards", 1);
    let mcp = requests_of(server, "/mcp/{p}", 1);
    let calls: Vec<&Request> = mcp.iter().filter(|line| line.tool != "-").collect();
    assert_eq!(calls.len(), 1, "one tools/call: {mcp:#?}");
    assert_eq!(
        (
            calls[0].method.as_str(),
            calls[0].project.as_str(),
            calls[0].tool.as_str(),
            calls[0].status.as_str()
        ),
        ("POST", "card-press", "search", "200"),
        "{}",
        calls[0].line
    );
    let by_mcp = turns_by(server, "/mcp/{p}");
    assert!(
        !by_mcp.is_empty() && by_mcp.iter().all(|turn| turn.tool == "search"),
        "the call's turn lines name its tool: {by_mcp:#?}"
    );

    let stderr = server.stderr();
    for secret in [
        "mira",
        "needle7",
        "needle8",
        "q=",
        "Remote-User",
        "remote-user",
        "Cookie",
        "cookie",
        "crumb9",
        "authelia_session",
        "203.0.113.7",
        "X-Forwarded",
        "Quillfeather",
        "discipline",
        "title",
        "\"as\"",
        "jsonrpc",
        "arguments",
        "query",
    ] {
        assert!(
            !stderr.contains(secret),
            "stderr holds {secret:?}:\n{stderr}"
        );
    }
}

// ---------------------------------------------------------------- SR4

/// SR4: at `--slow-ms 200`, the generated corpus's first `tree` holds the
/// turn and a `GET …/inbox` queues behind it (sent once the `held` line is
/// seen, so it is behind). The `held` line (its `held_ms` the threshold) is on
/// stderr while the tree is unanswered, a quarter of its run before its
/// answer. The inbox's line: `wait_ms` at least half the tree's `run_ms`,
/// its own `run_ms` under a fifth of its `wait_ms`. M: the clock for
/// `wait_ms` started at the turn; `held` written only at release.
#[test]
fn sr4_a_queued_request_names_its_wait_and_the_holder_shows_while_held() {
    let slowed = slowed("slow-wait");
    let server = &slowed.server;
    assert!(
        !slowed.tree.is_finished(),
        "the tree answered before its `held` line was seen"
    );
    let inbox = timed(server.port, &format!("{BIG}/inbox"), slowed.start);
    let (status, tree_at) = slowed.tree.join().expect("the tree");
    assert_eq!(status, 200, "the tree");
    let (status, _) = inbox.join().expect("the inbox");
    assert_eq!(status, 200, "the inbox");

    let tree = requests_of(server, "/api/projects/{p}/tree", 1);
    let inbox = requests_of(server, "/api/projects/{p}/inbox", 1);
    let (tree, inbox) = (&tree[0], &inbox[0]);
    let tree_run = tree.run_ms.expect("the tree ran");
    let report = format!(
        "held seen at {:?}, the tree answered at {tree_at:?}\n{}\n{}\n--- stderr\n{}",
        slowed.held_seen,
        tree.line,
        inbox.line,
        server.stderr()
    );
    eprintln!(
        "SR4: held seen at {:?}, the tree answered at {tree_at:?}\n{}\n{}",
        slowed.held_seen, tree.line, inbox.line
    );
    assert!(
        tree_run >= 800,
        "the tree ran {tree_run} ms, too short to hold a queue at 200 ms; the corpus must \
         grow: {report}"
    );
    assert!(
        slowed.held_seen + Duration::from_millis(tree_run / 4) < tree_at,
        "the `held` line came with the answer, not during the hold: {report}"
    );
    let held = turns_by(server, "/api/projects/{p}/tree");
    assert_eq!(
        held.iter()
            .map(|turn| (turn.state.as_str(), turn.held_ms == 200))
            .collect::<Vec<_>>()
            .first(),
        Some(&("held", true)),
        "the first tree turn line is `held` at the threshold: {report}"
    );
    assert!(
        held.iter().any(|turn| turn.state == "released"),
        "its release: {report}"
    );
    let wait = inbox.wait_ms.expect("the inbox waited for the turn");
    let run = inbox.run_ms.expect("the inbox ran");
    assert!(
        wait >= tree_run / 2,
        "the inbox's wait_ms is not its queueing behind the tree: {report}"
    );
    assert!(
        run < wait / 5,
        "the inbox's run_ms holds its wait: {report}"
    );
    assert!(wait + run <= inbox.ms, "{report}");
}

// ---------------------------------------------------------------- SR5

/// SR5: at `--slow-ms 200`, behind the same slow `tree`, a queued `GET
/// …/tasks` whose client closes its connection 300 ms after sending writes
/// `status=gone`, `run_ms=-`, `wait_ms` at least 200 and within its `ms`,
/// which ends before the tree's run does; it takes no turn, so no turn line
/// names it. M: no line on drop.
#[test]
fn sr5_a_client_gone_while_queued_writes_status_gone() {
    let slowed = slowed("slow-gone");
    let server = &slowed.server;
    let host = format!("127.0.0.1:{}", server.port);
    let mut gone = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
    gone.write_all(&request_bytes(
        "GET",
        &format!("{BIG}/tasks"),
        &[("Host", &host)],
    ))
    .expect("send");
    thread::sleep(Duration::from_millis(300));
    let _ = gone.shutdown(Shutdown::Both);
    drop(gone);
    let closed_at = slowed.start.elapsed();
    let (status, tree_at) = slowed.tree.join().expect("the tree");
    assert_eq!(status, 200, "the tree");
    assert!(
        closed_at + Duration::from_millis(200) < tree_at,
        "the client left at {closed_at:?}, too near the tree's answer at {tree_at:?}; the \
         corpus must grow"
    );
    let lines = requests_of(server, "/api/projects/{p}/tasks", 1);
    let tree = requests_of(server, "/api/projects/{p}/tree", 1);
    let report = format!("{lines:#?}\n{tree:#?}\n--- stderr\n{}", server.stderr());
    eprintln!(
        "SR5: closed at {closed_at:?}, the tree answered at {tree_at:?}\n{}\n{}",
        lines[0].line, tree[0].line
    );
    assert_eq!(lines.len(), 1, "one line: {report}");
    let line = &lines[0];
    assert_eq!(line.status, "gone", "{report}");
    assert_eq!(line.run_ms, None, "run_ms=-: {report}");
    let wait = line.wait_ms.expect("wait_ms to the drop");
    assert!(wait >= 200, "wait_ms >= 200: {report}");
    assert!(wait <= line.ms, "{report}");
    assert!(
        line.ms < tree[0].run_ms.expect("the tree ran"),
        "measured to the drop, while the tree held the turn: {report}"
    );
    assert_eq!(
        turns_by(server, "/api/projects/{p}/tasks"),
        Vec::new(),
        "a request gone before its turn holds none: {report}"
    );
}

// ---------------------------------------------------------------- SR6

/// SR6: at `--slow-ms 0`, a hub's first `GET …/tree` since start walks
/// (`index=walked`), the second trusts (`index=trusted`). M: `index` from the
/// route class rather than the walk count.
#[test]
fn sr6_a_hubs_first_read_walks_and_the_next_trusts() {
    let hub = hub("slow-index");
    let server = &hub.server;
    for _ in 0..2 {
        let reply = through(
            server,
            "GET",
            &format!("{P}/tree"),
            &Proxied::fetch("mira"),
            b"",
        );
        assert_eq!(reply.status, 200, "{}", reply.text());
    }
    let lines = requests_of(server, "/api/projects/{p}/tree", 2);
    assert_eq!(
        lines
            .iter()
            .map(|line| line.index.as_str())
            .collect::<Vec<_>>(),
        ["walked", "trusted"],
        "{lines:#?}"
    );
}

// ---------------------------------------------------------------- SR7

/// `git args` in `dir`, which must succeed.
fn git(scratch: &Scratch, dir: &Path, args: &[&str]) -> String {
    scratch.git().run(dir, args)
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// SR7: one repository holding two projects, `press/` (the print shop) and
/// `keep/` (the game's brief), pushed to a bare `origin.git` in the scratch;
/// the hub's checkout a clone with a commit of its own, the remote a
/// developer's commit ahead, so the start's sync merges. Served `press` then
/// `keep` with `--push --slow-ms 0`: once `GET …/sync` reads `ok`, the merge
/// is on the remote, and stderr holds one `by=sync` `held` and one
/// `released` line per project, `id=-`, `tool=-`, each kind in `--root`
/// order, each project's `held` first. A `…/sync` read takes no turn:
/// `wait_ms=- run_ms=-`. M: the sync's hold unwatched.
#[test]
fn sr7_the_syncs_merge_names_each_projects_turn_in_root_order() {
    let scratch = Scratch::new("slow-sync");
    let dev = scratch.dir("dev");
    copy_dir(&fixture("card-press"), &dev.join("press"));
    copy_dir(&fixture("spec-a"), &dev.join("keep"));
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
    write(&checkout, "notes/hand.md", "A commit made on the host.\n");
    git(&scratch, &checkout, &["add", "-A"]);
    git(&scratch, &checkout, &["commit", "-q", "-m", "on the host"]);
    write(&dev, "notes/elsewhere.md", "A developer's note.\n");
    git(&scratch, &dev, &["add", "-A"]);
    git(
        &scratch,
        &dev,
        &["commit", "-q", "-m", "a developer's change"],
    );
    git(&scratch, &dev, &["push", "-q", "origin", "main"]);

    let press = checkout.join("press");
    let keep = checkout.join("keep");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let mut args = slow_ms(hub_args(&[&press, &keep], ORIGIN), "0");
    args.extend(["--push", "--push-deadline", "60"].map(OsStr::new));
    let server = Server::start(&home, &cwd, &args);

    let started = Instant::now();
    let status: Value = loop {
        let reply = through(
            &server,
            "GET",
            "/api/projects/card-press/sync",
            &Proxied::fetch("mira"),
            b"",
        );
        assert_eq!(reply.status, 200, "{}", reply.text());
        let status = reply.json();
        if !status["at"].is_null() {
            break status;
        }
        assert!(
            started.elapsed() < LINE_WAIT,
            "no sync ended: {status}\n--- stderr\n{}",
            server.stderr()
        );
        thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(
        status["state"],
        json!("ok"),
        "{status}\n--- stderr\n{}",
        server.stderr()
    );
    let tip = git(
        &scratch,
        &bare,
        &["rev-list", "--parents", "-n", "1", "main"],
    );
    assert_eq!(
        tip.split_whitespace().count(),
        3,
        "the remote's tip is the sync's merge commit: {tip}"
    );

    wait_for(&server, LINE_WAIT, "the sync's released lines", |lines| {
        let released = turns(lines)
            .iter()
            .filter(|turn| turn.by == "sync" && turn.state == "released")
            .count();
        (released >= 2).then_some(())
    });
    thread::sleep(SETTLE);
    let sync = turns_by(&server, "sync");
    let report = format!("{sync:#?}\n--- stderr\n{}", server.stderr());
    let of = |state: &str| -> Vec<String> {
        sync.iter()
            .filter(|turn| turn.state == state)
            .map(|turn| turn.project.clone())
            .collect()
    };
    assert_eq!(of("held"), ["card-press", "lantern-keep"], "{report}");
    assert_eq!(of("released"), ["card-press", "lantern-keep"], "{report}");
    for project in ["card-press", "lantern-keep"] {
        let states: Vec<&str> = sync
            .iter()
            .filter(|turn| turn.project == project)
            .map(|turn| turn.state.as_str())
            .collect();
        assert_eq!(states, ["held", "released"], "{project}: {report}");
    }
    for turn in &sync {
        assert_eq!(
            (turn.id.as_str(), turn.tool.as_str()),
            ("-", "-"),
            "{report}"
        );
        if turn.state == "held" {
            assert_eq!(turn.held_ms, 0, "{report}");
        }
        if let Some(class) = &turn.class {
            assert_eq!(class, "sync", "{report}");
        }
    }
    let polls = requests_of(&server, "/api/projects/{p}/sync", 1);
    for poll in &polls {
        assert_eq!(
            (poll.wait_ms, poll.run_ms, poll.index.as_str()),
            (None, None, "-"),
            "`…/sync` takes no turn: {}",
            poll.line
        );
    }
}

// ---------------------------------------------------------------- SR8

/// SR8: at `--slow-ms 0`, an open `GET …/events` read until its first
/// keep-alive comment (15 s), then closed; two events stored after the close
/// so the tail writes to the closed connection and ends; a read after them.
/// No request line has `route=/api/projects/{p}/events`, neither while it was
/// open nor after. M: streams logged.
#[test]
fn sr8_a_stream_writes_no_request_line() {
    let scratch = Scratch::new("slow-stream");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = local(&home, scratch.path(), &[&a], "0");
    let route = "/api/projects/{p}/events";
    let mut stream = Stream::open(server.port, &format!("{KEEP}/events"), &[]);
    assert_eq!(stream.status, 200);
    let started = Instant::now();
    loop {
        let frame = stream.next_frame(Duration::from_secs(1));
        if frame.is_some_and(|frame| !frame.comments.is_empty()) {
            break;
        }
        assert!(!stream.ended(), "the stream ended");
        assert!(
            started.elapsed() < Duration::from_secs(40),
            "no keep-alive comment within 40 s"
        );
    }
    let open = requests(&slow_lines(&server.stderr()));
    assert!(
        open.iter().all(|line| line.route != route),
        "a request line for the open stream: {open:#?}"
    );
    drop(stream);
    for text in ["Is the delay 1.5 s?", "Is the delay 2 s?"] {
        ask(&home, &a, &["EDGE-STAM-ZERO"], text);
        thread::sleep(Duration::from_secs(2));
    }
    let reply = server.get(&format!("{KEEP}/tree"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    requests_of(&server, "/api/projects/{p}/tree", 1);
    let after = requests(&slow_lines(&server.stderr()));
    assert!(
        after.iter().all(|line| line.route != route),
        "a request line for the closed stream: {after:#?}"
    );
}

// ---------------------------------------------------------------- SR9

/// SR9: at `--slow-ms 0`, `GET …/proposals/PR-0001%E2%80%AE` writes `id=?`; a
/// project segment of 65 letters writes `project=?`, one of 64 itself (the
/// bound). Every line holds no byte above 0x7E (`common/slow.rs`). M: the raw
/// segment printed.
#[test]
fn sr9_a_value_outside_its_charset_prints_a_question_mark() {
    let scratch = Scratch::new("slow-hygiene");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = local(&home, scratch.path(), &[&a], "0");
    let reply = server.get(&format!("{KEEP}/proposals/PR-0001%E2%80%AE"));
    assert_ne!(reply.status, 200, "{}", reply.text());
    let lines = requests_of(&server, "/api/projects/{p}/proposals/{id}", 1);
    assert_eq!(
        (lines[0].project.as_str(), lines[0].id.as_str()),
        ("lantern-keep", "?"),
        "{}",
        lines[0].line
    );
    let long = "k".repeat(65);
    let bound = "k".repeat(64);
    server.get(&format!("/api/projects/{long}/tree"));
    server.get(&format!("/api/projects/{bound}/tree"));
    let lines = requests_of(&server, "/api/projects/{p}/tree", 2);
    assert_eq!(
        lines
            .iter()
            .map(|line| line.project.as_str())
            .collect::<Vec<_>>(),
        ["?", bound.as_str()],
        "{lines:#?}"
    );
    let stderr = server.stderr();
    assert!(
        stderr.bytes().all(|byte| byte <= 0x7e),
        "a byte above 0x7E on stderr: {stderr:?}"
    );
    assert!(!stderr.contains("%E2"), "the raw segment: {stderr}");
}

/// "Data", **Value hygiene**: "No value is percent-decoded into the line". A
/// slug sent as `lantern%2Dkeep`, which the daemon serves as `lantern-keep`
/// (README "Path"), prints `project=?`, and an ID sent as `PR%2D0001` prints
/// `id=?`: neither the decoded value nor the raw one, whose `%` is outside
/// the charset.
#[test]
fn data_no_value_is_percent_decoded() {
    let scratch = Scratch::new("slow-decoded");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let server = local(&home, scratch.path(), &[&a], "0");
    server.get("/api/projects/lantern%2Dkeep/tree").status(200);
    server
        .get(&format!("{KEEP}/proposals/PR%2D0001"))
        .status(200);
    let tree = requests_of(&server, "/api/projects/{p}/tree", 1);
    assert_eq!(tree[0].project, "?", "{}", tree[0].line);
    let proposal = requests_of(&server, "/api/projects/{p}/proposals/{id}", 1);
    assert_eq!(
        (proposal[0].project.as_str(), proposal[0].id.as_str()),
        ("lantern-keep", "?"),
        "{}",
        proposal[0].line
    );
}

// ---------------------------------------------------------------- SR10

/// SR10: `--slow-ms -1`, `x` and `3600001` each exit 2 with exactly one
/// stderr line naming the flag, nothing on stdout, nothing created under a
/// fresh `HOME` (before listening). `--slow-ms 3600000` starts, and its fast
/// read writes no line. `spec serve` launches the daemon with no
/// `--slow-ms` (its argv, read by `ps`), so 2 000 holds.
#[test]
fn sr10_the_flag_is_judged_before_listening() {
    let scratch = Scratch::new("slow-flag");
    let a = scratch.repo("spec-a", "a", "main");
    let mut wrong = Vec::new();
    for (index, value) in ["-1", "x", "3600001"].into_iter().enumerate() {
        let home = scratch.home(&format!("refused-{index}"));
        let args = slow_ms(root_args(&[&a]), value);
        match Server::try_start(&home, scratch.path(), &args) {
            Ok(_) => wrong.push(format!("--slow-ms {value} started")),
            Err(refused) => {
                let lines: Vec<&str> = refused.stderr.lines().collect();
                if refused.code != Some(2)
                    || !refused.stdout.is_empty()
                    || lines.len() != 1
                    || !refused.stderr.ends_with('\n')
                    || !lines[0].starts_with("specengine-http: ")
                    || !lines[0].contains("--slow-ms")
                    || !snapshot(&home).is_empty()
                {
                    wrong.push(format!(
                        "--slow-ms {value}: {refused:?}, HOME {:?}",
                        snapshot(&home).keys().collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "exit 2, one stderr line naming the flag, nothing created:\n{}",
        wrong.join("\n")
    );

    let home = scratch.home("widest");
    let server = local(&home, scratch.path(), &[&a], "3600000");
    server.get(&format!("{KEEP}/tree")).status(200);
    drop(server);

    let home = scratch.home("serve");
    let server = Server::try_start_program(
        &spec_bin(),
        &product_env(&home),
        &a,
        &[OsStr::new("serve"), OsStr::new("--port"), OsStr::new("0")],
    )
    .unwrap_or_else(|refused| panic!("spec serve: {refused:?}"));
    let argv = Command::new("/bin/ps")
        .args(["-o", "args=", "-p", &server.pid().to_string()])
        .output()
        .expect("run ps");
    let argv = String::from_utf8_lossy(&argv.stdout).into_owned();
    assert!(
        argv.contains("specengine-http") && argv.contains("--root"),
        "the daemon's argv: {argv:?}"
    );
    assert!(
        !argv.contains("--slow-ms"),
        "spec serve passes --slow-ms: {argv:?}"
    );
}

// ------------------------------------------- write-cost WC15: five fields

/// The confirm route's template, as a request line names it.
const CONFIRM_ROUTE: &str = "/api/projects/{p}/proposals/{id}/confirm";

/// docs/features/write-cost.md (RM-0125) WC15, "Attribution: five fields",
/// over spec-a opted in to a hub (`--slow-ms 0`), every git child of the
/// daemon through the shim:
/// - every request line ends ` check_ms= index_ms= git_ms= git= write_ms=`
///   (the grammar of `tests/common/slow.rs`), and the four `_ms` fields sum
///   to at most `run_ms`;
/// - a question's confirm GET names in `git` exactly the shim's lines of that
///   request;
/// - the POST that commits shows `write_ms`;
/// - a trusting read shows `check_ms=- git_ms=- git=- write_ms=-`;
/// - turn lines keep their keys (no field of the five).
///
/// M: the counters not reset per call (the second GET shows the sum); a git
/// spawn inside the walk charged to both kinds (the sum passes `run_ms`).
#[test]
fn wc15_every_request_line_ends_in_five_fields() {
    use write_cost_hub::{
        confirm_get, confirm_post, git_shim, hub_env, logged, opted_in, raise_question, shimmed,
        slug, stage_approve, start_with,
    };
    let repo = opted_in("wc15", "spec-a");
    let found = write_cost_common::probe(&repo.env(), &repo.git_env())
        .expect("spec-a opens")
        .expect("spec-a has a node");
    let slug = slug(&repo).expect("a slug");
    let shim_dir = repo.scratch.dir("shim");
    let log = repo.scratch.path().join("git.log");
    git_shim(&shim_dir, &log);
    let server = start_with(
        &repo,
        &shimmed(&hub_env(&repo), &shim_dir),
        &["--slow-ms", "0"],
    );
    let (_, question) = raise_question(&repo, &found);
    let id = question.expect("the question is stored");
    stage_approve(&repo, &id);

    // Two GETs, each counted alone: the second shows its own count.
    let mut counted = Vec::new();
    for round in 0..2 {
        thread::sleep(SETTLE);
        let before = logged(&log).len();
        let read = confirm_get(&server, &slug, &id);
        assert_eq!(read.status, 200, "GET {round}: {}", read.text());
        counted.push(logged(&log).len() - before);
    }
    let gets: Vec<Request> = requests_of(&server, CONFIRM_ROUTE, 2)
        .into_iter()
        .filter(|line| line.method == "GET")
        .collect();
    assert_eq!(gets.len(), 2, "{gets:?}");
    for (line, spawned) in gets.iter().zip(&counted) {
        let cost = line.cost.as_ref().expect("the five fields");
        assert_eq!(
            cost.git,
            Some(u64::try_from(*spawned).expect("a count")),
            "a GET's git is the shim's lines of it: {}",
            line.line
        );
    }

    thread::sleep(SETTLE);
    let read = confirm_get(&server, &slug, &id);
    let written = confirm_post(&server, &slug, &id, &read);
    assert_eq!(written.status, 200, "POST: {}", written.text());
    let post = requests_of(&server, CONFIRM_ROUTE, 4)
        .into_iter()
        .find(|line| line.method == "POST")
        .expect("the POST's line");
    let cost = post.cost.as_ref().expect("the five fields");
    assert!(
        cost.write_ms.is_some(),
        "a POST that commits shows write_ms: {}",
        post.line
    );
    assert!(cost.git.is_some_and(|git| git > 0), "{}", post.line);

    // Reads of the tree: once trusted, nothing of the write kinds ran. (A
    // proposal's own read is no such read: its review renders the diff with
    // one `git diff --no-index`, as 0.47.0's did.)
    let tree = format!("/api/projects/{slug}/tree");
    let mut trusted = None;
    for _ in 0..3 {
        let reply = through(&server, "GET", &tree, &Proxied::fetch("ann"), b"");
        assert_eq!(reply.status, 200, "{}", reply.text());
        let lines = requests(&slow_lines(&server.stderr()));
        trusted = lines
            .into_iter()
            .find(|line| line.route == "/api/projects/{p}/tree" && line.index == "trusted");
        if trusted.is_some() {
            break;
        }
        thread::sleep(SETTLE);
    }
    let trusted = trusted.expect("a trusting read of the tree");
    let cost = trusted.cost.as_ref().expect("the five fields");
    assert_eq!(
        (cost.check_ms, cost.git_ms, cost.git, cost.write_ms),
        (None, None, None, None),
        "a trusting read: {}",
        trusted.line
    );

    thread::sleep(SETTLE);
    let lines = slow_lines(&server.stderr());
    let all = requests(&lines);
    assert!(!all.is_empty());
    for line in &all {
        let cost = line
            .cost
            .as_ref()
            .unwrap_or_else(|| panic!("no five fields: {}", line.line));
        if let Some(run_ms) = line.run_ms {
            let sum: u64 = [cost.check_ms, cost.index_ms, cost.git_ms, cost.write_ms]
                .iter()
                .flatten()
                .sum();
            assert!(
                sum <= run_ms,
                "the five fields sum to {sum} > run_ms {run_ms}: {}",
                line.line
            );
        }
    }
    let turns = turns(&lines);
    assert!(!turns.is_empty(), "turn lines at --slow-ms 0");
    for turn in &turns {
        for key in [
            " check_ms=",
            " index_ms=",
            " git_ms=",
            " git=",
            " write_ms=",
        ] {
            assert!(
                !turn.line.contains(key),
                "a turn line keeps its keys: {}",
                turn.line
            );
        }
    }
}
