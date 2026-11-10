//! docs/features/flow-read.md AC-07, the route: `GET
//! /api/projects/:p/flow?root=&depth=`.
//!
//! Parity: on `fixtures/flow-gaps`, `fixtures/flow-relay` and
//! `fixtures/spec-c`, no query, `root` and `depth` answer 200, byte-equal to
//! `spec --root R flow <flags> --json` stdout less its final LF under the
//! same `HOME`, a gap of any count included; a repeat answers the same
//! bytes.
//!
//! Uncut: a corpus past the CLI's cut: the daemon's `truncated` is false,
//! its counts the CLI text's header, the CLI's nodes a prefix of the
//! daemon's, every gap row whole with all its items.
//!
//! The query: `root` naming nothing and `root=` empty are a 404 byte-equal
//! to the CLI's exit-1 document; `root` twice, `depth=x`, a scalar twice,
//! an unknown parameter, a CLI flag's name are a 400 naming the parameter,
//! a fresh `HOME` still empty after; `depth=-1`, a look-alike, a `project:`
//! REF are a 503 whose message is the CLI's line verbatim.
//!
//! docs/features/trace-views.md AC-03, the route: `trace=true` is `spec flow
//! --trace --json`'s bytes on `fixtures/trace-almanac` with cards made through
//! the routes, `rollup` and `counts` `null` with no flag or `trace=false`, and
//! `null` with a note on a newer queue, a 200. M: `rollup` filled without the
//! flag.
//!
//! docs/features/flow-scope.md (RM-0083) FS-6, the route: `stage=` on
//! `fixtures/trace-almanac` served as its own slug is `spec flow --stage S
//! --json`'s bytes, `-` included and beside the other parameters; a
//! repeated `stage` is a 400 with the spec's line. M8: the route parsing
//! `stage` but not passing it on (the whole read answered).
//!
//! docs/features/flow-scenario.md (RM-0094) FS-10, the route: `in_flow=` on
//! `fixtures/flow-aside` is `spec flow --in-flow --json`'s bytes, beside
//! `stage`, `trace` and no ROOT; another value is a 400 with the spec's
//! words; `nodes/{*ref}` carries `ancestors`. M: the route not passing
//! `in_flow` on (145 nodes).
//!
//! Key sets are `daemon_keys.rs`; the route's list, `methods.rs` and
//! `projects.rs`.

mod common;

use std::path::Path;

use common::{
    Reply, Run, Scratch, Server, beyond_daemon, data_dir, encode_component, spec, spec_json, write,
};
use serde_json::{Value, json};

const P: &str = "/api/projects/flow-gaps";

/// The almanac's project.
const ALMANAC: &str = "/api/projects/trace-almanac";

/// The daemon's `path` against `spec --root root <args> --json` (exit 0 ->
/// 200, exit 1 -> 404): the same bytes, the JSON headers, the same bytes
/// again on a second request.
fn same(server: &Server, home: &Path, cwd: &Path, root: &Path, path: &str, args: &[&str]) -> Reply {
    let run = spec_json(home, cwd, root, args);
    let reply = server.get(path);
    let want = if run.code == 0 { 200 } else { 404 };
    assert_eq!(
        reply.status,
        want,
        "{path} vs spec {args:?} (exit {}): {}",
        run.code,
        reply.text()
    );
    assert_eq!(
        reply.text(),
        run.document(),
        "{path} is byte-equal to `spec flow`"
    );
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8"),
        "{path}"
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
    let again = server.get(path);
    assert_eq!(
        (again.status, again.text()),
        (reply.status, reply.text()),
        "{path}: a repeat answers the same bytes"
    );
    reply
}

/// `spec --root root flow <args> --json`, which must exit 2; its stderr
/// less the final LF.
fn cannot_run(home: &Path, cwd: &Path, root: &Path, args: &[&str]) -> String {
    let root = root.to_str().expect("UTF-8 root");
    let mut all = vec!["--root", root, "flow"];
    all.extend_from_slice(args);
    all.push("--json");
    let run: Run = spec(home, cwd, &all);
    run.code(2);
    assert_eq!(run.stdout, "", "exit 2 prints no document: {}", run.show());
    run.stderr.trim_end_matches('\n').to_owned()
}

/// A daemon over a repository of `fixtures/trace-almanac` holding an accepted
/// card on `STEP-CALL` and an open one on `STEP-SET`, each made by a route.
fn almanac(
    scratch: &Scratch,
) -> (
    Server,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let root = scratch.repo("trace-almanac", "almanac", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let cards = format!("{ALMANAC}/cards");
    for (path, body) in [
        (
            "",
            json!({"title": "Notice posted", "discipline": "editing",
                "target": ["STEP-CALL"]}),
        ),
        (
            "/C-0001/assign",
            json!({"assignee": "ana", "reviewer": "ben"}),
        ),
        ("/C-0001/move", json!({"to": "review"})),
        ("/C-0001/accept", json!({"as": "ben", "briefed": true})),
        (
            "",
            json!({"title": "Pages set", "discipline": "editing", "target": ["STEP-SET"]}),
        ),
    ] {
        server
            .stage(&format!("{cards}{path}"), &body.to_string())
            .status(200);
    }
    (server, root, home, cwd)
}

/// AC-03, the route: every form of `trace` is the CLI's document byte for
/// byte, at every depth, and a read with no flag holds no `rollup`.
/// M: `rollup` filled without the flag.
#[test]
fn trace_views_the_flag_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("flow-trace");
    let (server, root, home, cwd) = almanac(&scratch);
    let forms: [(&str, &[&str]); 7] = [
        ("", &[]),
        ("?trace=false", &[]),
        ("?trace=true", &["--trace"]),
        (
            "?root=BOOK-ALMANAC&trace=true",
            &["BOOK-ALMANAC", "--trace"],
        ),
        (
            "?root=BOOK-ALMANAC&depth=1&trace=true",
            &["BOOK-ALMANAC", "--depth", "1", "--trace"],
        ),
        (
            "?trace=true&depth=0&root=BOOK-ALMANAC",
            &["BOOK-ALMANAC", "--depth", "0", "--trace"],
        ),
        (
            "?root=BOOK-ALMANAC&depth=1",
            &["BOOK-ALMANAC", "--depth", "1"],
        ),
    ];
    for (query, flags) in forms {
        let mut args = vec!["flow"];
        args.extend_from_slice(flags);
        same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{ALMANAC}/flow{query}"),
            &args,
        );
    }
    let document: Value = server.get(&format!("{ALMANAC}/flow")).json();
    for node in document["nodes"].as_array().expect("nodes") {
        assert_eq!(node["rollup"], Value::Null, "{node}");
        assert_eq!(node["counts"], Value::Null, "{node}");
    }
    let folded: Value = server
        .get(&format!(
            "{ALMANAC}/flow?root=BOOK-ALMANAC&depth=1&trace=true"
        ))
        .json();
    let part = folded["nodes"][0].clone();
    assert_eq!(part["id"], "PART-GATHER", "{folded}");
    assert_eq!(part["start"], json!(true), "{part}");
    assert_eq!(part["rollup"], "no card", "{part}");
    assert_eq!(
        part["counts"],
        json!({"no_card": 0, "in_work": 1, "suspect": 0, "departed": 0, "confirmed": 1}),
        "the daemon counts what it holds: {part}"
    );

    for (query, parameter) in [
        ("?trace=yes", "trace"),
        ("?trace=", "trace"),
        ("?trace=true&trace=false", "trace"),
    ] {
        let path = format!("{ALMANAC}/flow{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        assert!(
            reply.error_message().contains(parameter),
            "{path}: {}",
            reply.error_message()
        );
    }
}

/// "Rules and edge cases": `trace=true` with a newer queue is a 200, `rollup`
/// and `counts` `null` with a note, the CLI's bytes, never a failed flow.
#[test]
fn trace_views_a_newer_queue_is_a_note() {
    let scratch = Scratch::new("flow-trace-schema");
    let (server, root, home, cwd) = almanac(&scratch);
    let done = std::process::Command::new("/usr/bin/sqlite3")
        .arg(data_dir(&home).join("trace-almanac.db"))
        .arg("PRAGMA user_version = 9999;")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(done.status.success(), "{done:?}");
    let reply = same(
        &server,
        &home,
        &cwd,
        &root,
        &format!("{ALMANAC}/flow?trace=true"),
        &["flow", "--trace"],
    );
    let document: Value = reply.json();
    assert_eq!(
        document["notes"].as_array().map(Vec::len),
        Some(1),
        "{document}"
    );
    for node in document["nodes"].as_array().expect("nodes") {
        assert_eq!(node["rollup"], Value::Null, "{node}");
        assert_eq!(node["counts"], Value::Null, "{node}");
    }
}

/// AC-07: every form is the CLI's document byte for byte, on the planted
/// corpus: a whole read, a ROOT in each of its forms, each depth. The
/// planted rows change no status: 200 with every row counted.
#[test]
fn every_flow_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("flow-parity");
    let root = scratch.repo("flow-gaps", "gaps", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);

    let forms: [(&str, &[&str]); 9] = [
        ("", &[]),
        ("?depth=0", &["--depth", "0"]),
        ("?depth=1", &["--depth", "1"]),
        ("?root=WING-BROKEN", &["WING-BROKEN"]),
        ("?root=WING-LEND", &["WING-LEND"]),
        ("?root=WING-HEADLESS", &["WING-HEADLESS"]),
        (
            "?root=WING-LIBRARY&depth=1",
            &["WING-LIBRARY", "--depth", "1"],
        ),
        (
            "?depth=2&root=WING-LIBRARY",
            &["WING-LIBRARY", "--depth", "2"],
        ),
        (
            "?root=docs%2Fspec%2Fwing-lend.md",
            &["docs/spec/wing-lend.md"],
        ),
    ];
    for (query, flags) in forms {
        let mut args = vec!["flow"];
        args.extend_from_slice(flags);
        same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{P}/flow{query}"),
            &args,
        );
    }
    let section = encode_component("LND-ASK#LND-ASK-LINE");
    same(
        &server,
        &home,
        &cwd,
        &root,
        &format!("{P}/flow?root={section}"),
        &["flow", "LND-ASK#LND-ASK-LINE"],
    );

    let reply = server.get(&format!("{P}/flow?root=WING-BROKEN"));
    let document: Value = serde_json::from_str(reply.text()).expect("JSON");
    assert_eq!(document["gaps"].as_array().expect("gaps").len(), 10);
    assert_eq!(
        document["gaps"][7],
        json!({"code": "definition-unused", "state": "measured", "count": 3,
            "items": [["PLT-FEED-A"], ["PLT-FEED-B"], ["PLT-UNUSED"]],
            "cause": "no document uses it: add a uses link to it, or drop it from \
                      [[flow.definition]]"}),
        "{document}"
    );
}

/// AC-06: the route answers on a project with no `[flow]` and one with no
/// flow link: 200 both, `spec-c`'s `nodes` empty.
#[test]
fn the_route_answers_on_other_projects() {
    let scratch = Scratch::new("flow-other");
    let relay = scratch.repo("flow-relay", "relay", "main");
    let hire = scratch.repo("spec-c", "hire", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&relay, &hire]);
    same(
        &server,
        &home,
        &cwd,
        &relay,
        "/api/projects/flow-relay/flow",
        &["flow"],
    );
    let reply = same(
        &server,
        &home,
        &cwd,
        &hire,
        "/api/projects/dock-hire/flow",
        &["flow"],
    );
    let document: Value = serde_json::from_str(reply.text()).expect("JSON");
    assert_eq!(document["nodes"], json!([]), "{document}");
    assert_eq!(document["gaps"].as_array().expect("gaps").len(), 10);
}

/// AC-07: past the CLI's cut the daemon is uncut: every node, every edge,
/// every item; the CLI's counts and a prefix of its order.
#[test]
fn a_corpus_past_the_cap_comes_whole() {
    let scratch = Scratch::new("flow-uncut");
    let root = scratch.repo("flow-gaps", "gaps", "main");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"flow-gaps\"\n\n[ids]\nPACE = { kind = \"pace\", width = 4 }\n\n\
         [[flow.step]]\nkinds = [\"pace\"]\n\n[[flow.start]]\n\
         paths = [\"docs/spec/pace-0001.md\"]\n",
    );
    let title = "A long way round the block, and back again along the river";
    let count = 700;
    for step in 1..=count {
        let links = if step % 2 == 0 {
            format!("links:\n  follows: [PACE-{:04}]\n", step - 1)
        } else {
            String::new()
        };
        write(
            &root,
            &format!("docs/spec/pace-{step:04}.md"),
            format!(
                "---\nid: PACE-{step:04}\nclass: canon\ntitle: {title} {step}\nowner: owner\n\
                 reviewed: 2026-10-01\n{links}---\n\n# {title} {step}\n\nA step.\n"
            ),
        );
    }
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);

    let run = spec_json(&home, &cwd, &root, &["flow"]);
    run.code(0);
    let cli = run.json();
    assert_eq!(cli["truncated"], json!(true), "the CLI cuts this corpus");

    let reply = server.get(&format!("{P}/flow"));
    reply.status(200);
    let served: Value = serde_json::from_str(reply.text()).expect("JSON");
    assert_eq!(served["truncated"], json!(false));
    assert_eq!(served["omitted_nodes"], json!(0));
    assert_eq!(served["omitted_edges"], json!(0));
    let nodes = served["nodes"].as_array().expect("nodes");
    let edges = served["edges"].as_array().expect("edges");
    assert_eq!(nodes.len(), count, "every node");
    assert_eq!(edges.len(), count / 2, "every edge");

    let cli_nodes = cli["nodes"].as_array().expect("nodes");
    assert!(cli_nodes.len() < count, "{} nodes", cli_nodes.len());
    assert_eq!(
        &nodes[..cli_nodes.len()],
        cli_nodes.as_slice(),
        "the CLI's nodes are a prefix"
    );
    assert_eq!(
        served["nodes"].as_array().expect("nodes").len(),
        cli_nodes.len() + cli["omitted_nodes"].as_u64().expect("a count") as usize,
        "the CLI's header counts the daemon's nodes"
    );
    // The row's `count` is the whole in both; only the items differ. Steps
    // pair up (1, 2), (3, 4) and on, so the start reaches the first pair alone.
    let unreachable = |document: &Value| document["gaps"][2].clone();
    assert_eq!(unreachable(&served)["count"], unreachable(&cli)["count"]);
    assert_eq!(unreachable(&served)["count"], json!(count - 2));
    assert_eq!(
        unreachable(&served)["items"]
            .as_array()
            .expect("items")
            .len(),
        count - 2,
        "every item"
    );
    assert_eq!(
        unreachable(&cli)["items"].as_array().expect("items").len(),
        50,
        "the CLI's printed items"
    );
}

/// The query: a ROOT naming nothing is the CLI's exit-1 document; a bad
/// query a 400 before anything runs; what the CLI refuses a 503 with its
/// line.
#[test]
fn the_query_of_the_route() {
    let scratch = Scratch::new("flow-query");
    let root = scratch.repo("flow-gaps", "gaps", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);

    for (query, reference) in [("?root=LND-NOPE", "LND-NOPE"), ("?root=", "")] {
        let reply = same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{P}/flow{query}"),
            &["flow", reference],
        );
        assert_eq!(reply.status, 404, "{query}");
    }

    for (query, parameter) in [
        ("?root=WING-LEND&root=WING-BROKEN", "root"),
        ("?depth=1&depth=2", "depth"),
        ("?depth=x", "depth"),
        ("?depth=", "depth"),
        ("?x=1", "x"),
        ("?type=follows", "type"),
        ("?archive=true", "archive"),
        ("?ref=WING-LEND", "ref"),
    ] {
        let path = format!("{P}/flow{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        assert!(
            reply.error_message().contains(parameter),
            "{path}: {}",
            reply.error_message()
        );
    }
    let bad = server.get(&format!("{P}/flow?root=%ZZ"));
    assert_eq!(bad.status, 400, "a bad escape: {}", bad.text());

    for (query, flags) in [
        ("?depth=-1", &["--depth", "-1"][..]),
        ("?root=LND-%D0%90SK", &["LND-\u{0410}SK"]),
        ("?root=other:LND-ASK", &["other:LND-ASK"]),
    ] {
        let path = format!("{P}/flow{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        let message = cannot_run(&home, &cwd, &root, flags);
        assert_eq!(reply.error_message(), message, "{path}");
    }

    // An unknown slug and a refused query reach no project.
    let fresh = scratch.home("fresh");
    let server = Server::serve(&fresh, &cwd, &[&root]);
    let before = beyond_daemon(&fresh);
    assert_eq!(server.get("/api/projects/nope/flow").status, 404);
    assert_eq!(server.get(&format!("{P}/flow?depth=x")).status, 400);
    assert_eq!(beyond_daemon(&fresh), before, "no database opened");
}

/// docs/features/flow-scope.md FS-6, the route: every form of `stage` is the
/// CLI's document byte for byte; the kept nodes are the stage's; a repeated
/// `stage` is the 400 "Data" writes, a fresh `HOME` still empty after. M8:
/// the route parsing `stage` but not passing it on.
#[test]
fn fs6_the_stage_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("flow-stage");
    let root = scratch.repo("trace-almanac", "almanac", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let forms: [(&str, &[&str]); 7] = [
        ("?stage=gather", &["--stage", "gather"]),
        ("?stage=-", &["--stage", "-"]),
        ("?stage=print", &["--stage", "print"]),
        ("?stage=bind", &["--stage", "bind"]),
        (
            "?stage=gather&trace=true",
            &["--stage", "gather", "--trace"],
        ),
        (
            "?root=PART-PRINT&depth=1&stage=gather",
            &["PART-PRINT", "--depth", "1", "--stage", "gather"],
        ),
        (
            "?root=NOPE-404&stage=gather",
            &["NOPE-404", "--stage", "gather"],
        ),
    ];
    for (query, flags) in forms {
        let mut args = vec!["flow"];
        args.extend_from_slice(flags);
        same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{ALMANAC}/flow{query}"),
            &args,
        );
    }
    let ids = |query: &str| -> Vec<String> {
        let document: Value = server.get(&format!("{ALMANAC}/flow{query}")).json();
        document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["id"].as_str().expect("an id").to_owned())
            .collect()
    };
    assert_eq!(ids("?stage=gather"), ["STEP-CALL", "STEP-SORT"]);
    assert_eq!(ids("?stage=-"), ["STEP-READ"]);
    assert_eq!(ids("?stage=print"), ["STEP-SET", "STEP-BIND"]);
    assert!(ids("?stage=bind").is_empty());
    let whole: Value = server.get(&format!("{ALMANAC}/flow")).json();
    let gather: Value = server.get(&format!("{ALMANAC}/flow?stage=gather")).json();
    assert_eq!(gather["gaps"], whole["gaps"], "the rows stand");

    let path = format!("{ALMANAC}/flow?stage=gather&stage=print");
    let reply = server.get(&path);
    assert_eq!(reply.status, 400, "{path}: {}", reply.text());
    assert_eq!(
        reply.error_message(),
        "`stage` is given 2 times: it takes one value",
        "{path}"
    );
    let fresh = scratch.home("fresh");
    let server = Server::serve(&fresh, &cwd, &[&root]);
    let before = beyond_daemon(&fresh);
    assert_eq!(server.get(&path).status, 400);
    assert_eq!(beyond_daemon(&fresh), before, "no database opened");
}

// ------------------------------------------------------------------ flow-scenario

/// The pottery's project, `fixtures/flow-aside` (flow-scenario, RM-0094).
const ASIDE: &str = "/api/projects/flow-aside";

/// docs/features/flow-scenario.md FS-10, the route: `in_flow=true` is `spec
/// flow --in-flow`'s document byte for byte (WBAY-KILN at depth 1: its four
/// nodes in the flow, 141 left out), beside `stage`, `trace` and no ROOT;
/// `in_flow=false` and no `in_flow` are the uncut read (145 nodes,
/// `not_in_flow` `null`); `in_flow=yes` is a 400 with the words of "Data",
/// a fresh `HOME` still empty after; `GET .../nodes/HUNCH-0007` carries
/// `ancestors`, `spec show`'s byte for byte. M: the route not passing
/// `in_flow` (145 nodes).
#[test]
fn fs10_in_flow_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("flow-aside");
    let root = scratch.repo("flow-aside", "aside", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let forms: [(&str, &[&str]); 7] = [
        (
            "?root=WBAY-KILN&depth=1&in_flow=true",
            &["WBAY-KILN", "--depth", "1", "--in-flow"],
        ),
        (
            "?root=WBAY-KILN&depth=1&in_flow=false",
            &["WBAY-KILN", "--depth", "1"],
        ),
        ("?root=WBAY-KILN&depth=1", &["WBAY-KILN", "--depth", "1"]),
        (
            "?root=ATL-POTTERY&depth=1&in_flow=true&trace=true",
            &["ATL-POTTERY", "--depth", "1", "--in-flow", "--trace"],
        ),
        (
            "?root=KRUN-FIRING&stage=glazing&in_flow=true",
            &["KRUN-FIRING", "--stage", "glazing", "--in-flow"],
        ),
        ("?depth=0&in_flow=true", &["--depth", "0", "--in-flow"]),
        (
            "?root=WBAY-NOWHERE&in_flow=true",
            &["WBAY-NOWHERE", "--in-flow"],
        ),
    ];
    for (query, flags) in forms {
        let mut args = vec!["flow"];
        args.extend_from_slice(flags);
        same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{ASIDE}/flow{query}"),
            &args,
        );
    }
    let read = |query: &str| -> Value { server.get(&format!("{ASIDE}/flow{query}")).json() };
    let names = |document: &Value| -> Vec<String> {
        document["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["id"].as_str().expect("an id").to_owned())
            .collect()
    };
    let cut = read("?root=WBAY-KILN&depth=1&in_flow=true");
    assert_eq!(
        names(&cut),
        ["HUNCH-0007", "WBAY-KILN", "CONE-BISQUE", "CONE-GLOST"],
        "{cut}"
    );
    assert_eq!(cut["not_in_flow"], json!(141), "{cut}");
    assert_eq!(
        cut["types"],
        json!(["follows", "uses", "supplies"]),
        "{cut}"
    );
    for query in [
        "?root=WBAY-KILN&depth=1&in_flow=false",
        "?root=WBAY-KILN&depth=1",
    ] {
        let whole = read(query);
        assert_eq!(
            whole["nodes"].as_array().map(Vec::len),
            Some(145),
            "{query}"
        );
        assert_eq!(whole["not_in_flow"], Value::Null, "{query}");
    }

    let path = format!("{ASIDE}/flow?root=WBAY-KILN&in_flow=yes");
    let reply = server.get(&path);
    assert_eq!(reply.status, 400, "{path}: {}", reply.text());
    assert_eq!(
        reply.error_message(),
        "`in_flow=yes`: not `true` or `false`",
        "{path}"
    );
    let fresh = scratch.home("fresh");
    let other = Server::serve(&fresh, &cwd, &[&root]);
    let before = beyond_daemon(&fresh);
    assert_eq!(other.get(&path).status, 400);
    assert_eq!(beyond_daemon(&fresh), before, "no database opened");

    for (reference, want) in [
        ("HUNCH-0007", json!(["ATL-POTTERY", "WBAY-KILN"])),
        (
            "CONE-GLOST-SHELF",
            json!(["ATL-POTTERY", "WBAY-KILN", "CONE-GLOST"]),
        ),
        ("ATL-POTTERY", json!([])),
    ] {
        let path = format!("{ASIDE}/nodes/{reference}");
        let run = spec_json(&home, &cwd, &root, &["show", reference]);
        let reply = server.get(&path);
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        assert_eq!(reply.text(), run.document(), "{path} is `spec show`'s");
        let node = &reply.json()["nodes"][0];
        let chain: Vec<&str> = node["ancestors"]
            .as_array()
            .unwrap_or_else(|| panic!("{path}: no `ancestors`: {node}"))
            .iter()
            .map(|entry| entry["id"].as_str().expect("an id"))
            .collect();
        assert_eq!(json!(chain), want, "{path}");
    }
}
