//! AC-01, AC-02, AC-03 of docs/features/ui-live.md: `GET
//! /api/projects/:p/graph`.
//!
//! AC-01, parity: on A, `MEC-SPRINT`, `MEC-STAMINA#RULE-STAM-REGEN`
//! (`%23`) and `docs/spec/movement/sprint.md` (`%2F`) × {none,
//! `impact=true`, `types=depends_on&types=constrains`, `depth=1`,
//! `archive=true`} answer 200, byte-equal to `spec --root A graph REF
//! <flags> --json` stdout less its final LF under the same `HOME`; so do
//! the spec's example query, the types in the other order (kept as given),
//! an unknown link type (never judged: ADR-0008) and explicit `false`s.
//! M: `impact` ignored.
//!
//! AC-02, uncut: A plus a generated document whose `depends_on` names 700
//! sections, so the CLI cuts (`truncated` true, `edges` `[]`): the
//! daemon's `truncated` is false and its node and edge counts are the CLI
//! text's `nodes <n>, edges <e>`; the CLI's nodes are a prefix of the
//! daemon's, and every byte before `truncated` is the CLI's. M:
//! `View::Capped` in the handler; `graph` given `Browser`.
//!
//! AC-03, the query: an unknown REF and `ref=` empty are a 404 byte-equal
//! to the CLI's exit-1 document; `ref` missing or twice, `impact=yes`,
//! `depth=x`, `x=1`, a bad `%`, a scalar twice, a CLI flag's name are a 400
//! naming the parameter, with a fresh `HOME` still empty after; `depth=-1`,
//! a look-alike ID, a `project:` REF are a 503 whose message is the CLI's
//! line verbatim. M: `ref` optional.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use common::{Reply, Run, Scratch, Server, encode_component, snapshot, spec, spec_json, write};
use serde_json::Value;

const P: &str = "/api/projects/lantern-keep";

/// The daemon's `path` against `spec --root root <args> --json` (exit 0 →
/// 200, exit 1 → 404): the same bytes, the JSON headers, the same bytes
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
        "{path} is byte-equal to `spec --root {} {} --json`",
        root.display(),
        args.join(" ")
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

/// `spec --root root <args> --json`, which must exit 2; its stderr less
/// the final LF.
fn cannot_run(home: &Path, cwd: &Path, root: &Path, args: &[&str]) -> String {
    let root = root.to_str().expect("UTF-8 root");
    let mut all = vec!["--root", root];
    all.extend_from_slice(args);
    all.push("--json");
    let run: Run = spec(home, cwd, &all);
    run.code(2);
    assert_eq!(run.stdout, "", "exit 2 prints no document: {}", run.show());
    run.stderr.trim_end_matches('\n').to_owned()
}

#[test]
fn ac01_every_graph_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("graph-parity");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a]);

    let options: [(&str, &[&str]); 5] = [
        ("", &[]),
        ("&impact=true", &["--impact"]),
        (
            "&types=depends_on&types=constrains",
            &["--type", "depends_on", "--type", "constrains"],
        ),
        ("&depth=1", &["--depth", "1"]),
        ("&archive=true", &["--archive"]),
    ];
    let mut bodies = BTreeSet::new();
    let mut edges = 0;
    for reference in [
        "MEC-SPRINT",
        "MEC-STAMINA#RULE-STAM-REGEN",
        "docs/spec/movement/sprint.md",
    ] {
        // `#` as `%23`, `/` as `%2F`.
        let encoded = encode_component(reference);
        assert_eq!(
            encoded,
            reference.replace('#', "%23").replace('/', "%2F"),
            "{reference}"
        );
        for (query, flags) in options {
            let path = format!("{P}/graph?ref={encoded}{query}");
            let mut args = vec!["graph", reference];
            args.extend_from_slice(flags);
            let reply = same(&server, &home, &cwd, &a, &path, &args);
            assert_eq!(reply.status, 200, "{path}");
            let document = reply.json();
            assert_eq!(document["ref"], Value::from(reference), "{path}");
            assert_eq!(document["truncated"], Value::Bool(false), "{path}");
            edges += document["edges"].as_array().expect("edges").len();
            bodies.insert(reply.text().to_owned());
        }
    }
    assert_eq!(bodies.len(), 15, "the fifteen answers differ (echoes)");
    assert!(
        edges > 0,
        "the walks on A have edges: the parity covers them"
    );

    // The spec's example query, on A.
    same(
        &server,
        &home,
        &cwd,
        &a,
        &format!(
            "{P}/graph?ref=MEC-STAMINA%23RULE-STAM-REGEN&impact=true&types=depends_on\
             &types=constrains&depth=3"
        ),
        &[
            "graph",
            "MEC-STAMINA#RULE-STAM-REGEN",
            "--impact",
            "--type",
            "depends_on",
            "--type",
            "constrains",
            "--depth",
            "3",
        ],
    );
    // The types as given, in order: the other order is another document.
    let forward = same(
        &server,
        &home,
        &cwd,
        &a,
        &format!("{P}/graph?ref=MEC-SPRINT&types=depends_on&types=constrains"),
        &[
            "graph",
            "MEC-SPRINT",
            "--type",
            "depends_on",
            "--type",
            "constrains",
        ],
    );
    let backward = same(
        &server,
        &home,
        &cwd,
        &a,
        &format!("{P}/graph?ref=MEC-SPRINT&types=constrains&types=depends_on"),
        &[
            "graph",
            "MEC-SPRINT",
            "--type",
            "constrains",
            "--type",
            "depends_on",
        ],
    );
    assert_ne!(forward.text(), backward.text(), "the order reaches the CLI");
    // A link type is never judged: an unknown one follows nothing, as on
    // the CLI (ADR-0008).
    let unknown = same(
        &server,
        &home,
        &cwd,
        &a,
        &format!("{P}/graph?ref=MEC-SPRINT&types=nonsense_type"),
        &["graph", "MEC-SPRINT", "--type", "nonsense_type"],
    );
    assert_eq!(unknown.json()["edges"], Value::Array(Vec::new()));
    // Explicit `false` is the flag absent; `impact` with `depth=0` too.
    same(
        &server,
        &home,
        &cwd,
        &a,
        &format!("{P}/graph?ref=MEC-SPRINT&impact=false&archive=false"),
        &["graph", "MEC-SPRINT"],
    );
    same(
        &server,
        &home,
        &cwd,
        &a,
        &format!("{P}/graph?depth=0&impact=true&ref=docs%2Fspec%2Fmovement%2Fstamina.md"),
        &[
            "graph",
            "docs/spec/movement/stamina.md",
            "--impact",
            "--depth",
            "0",
        ],
    );
}

/// The `nodes <n>, edges <e>` line of the CLI's text answer.
fn text_counts(run: &Run) -> (usize, usize) {
    let line = run
        .stdout
        .lines()
        .find(|line| line.starts_with("nodes "))
        .unwrap_or_else(|| panic!("no `nodes <n>, edges <e>` line: {}", run.show()));
    let (nodes, edges) = line
        .strip_prefix("nodes ")
        .and_then(|rest| rest.split_once(", edges "))
        .unwrap_or_else(|| panic!("{line:?}"));
    (
        nodes.parse().expect("a node count"),
        edges.parse().expect("an edge count"),
    )
}

/// Sections `MEC-BIG` depends on: far more lines than the CLI's cap.
const WIDE: usize = 700;

#[test]
fn ac02_the_daemon_sends_whole_the_graph_the_cli_cuts() {
    let scratch = Scratch::new("graph-uncut");
    let g = scratch.repo("spec-a", "g", "main");
    let ids: Vec<String> = (1..=WIDE).map(|i| format!("RULE-BIG-{i:03}")).collect();
    let mut text = format!(
        "---\nid: MEC-BIG\nclass: canon\ntier: 2\nparent: DOM-MOVEMENT\nowner: owner\n\
         reviewed: 2026-09-20\nlinks:\n  depends_on: [{}]\n---\n\n# Big\n\nA wide mechanic.\n",
        ids.join(", ")
    );
    for (i, id) in ids.iter().enumerate() {
        text.push_str(&format!(
            "\n## Item {:03} of the wide mechanic {{#{id}}}\n\nDepends on [[MEC-STAMINA]].\n",
            i + 1
        ));
    }
    write(&g, "docs/spec/movement/big.md", text);
    let git = scratch.git();
    git.run(&g, &["add", "-A"]);
    git.run(&g, &["commit", "-q", "-m", "a wide mechanic"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&g]);

    let cli = spec_json(&home, &cwd, &g, &["graph", "MEC-BIG"]);
    cli.code(0);
    let cut = cli.json();
    assert_eq!(
        cut["truncated"],
        Value::Bool(true),
        "the CLI cuts: {}",
        cli.stdout.len()
    );
    assert_eq!(
        cut["edges"],
        Value::Array(Vec::new()),
        "a node cut drops every edge"
    );
    let g_text = g.to_str().unwrap();
    let text_run = spec(&home, &cwd, &["--root", g_text, "graph", "MEC-BIG"]);
    text_run.code(0);
    let (nodes, edges) = text_counts(&text_run);
    assert_eq!((nodes, edges), (WIDE + 1, WIDE), "{}", text_run.stdout);

    let reply = server.get(&format!("{P}/graph?ref=MEC-BIG"));
    reply.status(200);
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    let whole = reply.json();
    assert_eq!(
        whole["truncated"],
        Value::Bool(false),
        "the browser view is uncut"
    );
    let daemon_nodes = whole["nodes"].as_array().expect("nodes");
    let daemon_edges = whole["edges"].as_array().expect("edges");
    assert_eq!(
        (daemon_nodes.len(), daemon_edges.len()),
        (nodes, edges),
        "the daemon's counts are the CLI text's `nodes {nodes}, edges {edges}`"
    );
    let cli_nodes = cut["nodes"].as_array().expect("nodes");
    assert!(cli_nodes.len() < nodes, "the CLI shows fewer nodes");
    assert_eq!(
        &daemon_nodes[..cli_nodes.len()],
        &cli_nodes[..],
        "the CLI's nodes are the daemon's first ones"
    );
    // Every byte before `truncated` (the echoes, notes, left_out) is the
    // CLI's; then the daemon's keys go on in the CLI's order.
    let at = cli
        .document()
        .find("\"truncated\":")
        .expect("the CLI's `truncated`");
    assert_eq!(&reply.text()[..at], &cli.document()[..at]);
    assert!(
        reply.text()[at..].starts_with("\"truncated\":false,\"nodes\":[{"),
        "{}",
        &reply.text()[at..at + 60]
    );
    let edges_at = reply
        .text()
        .find("],\"edges\":[{")
        .expect("`edges` after `nodes`");
    assert!(reply.text().ends_with("}]}"), "`edges` is the last key");
    assert!(edges_at > at);
    // Every edge is a `depends_on` from MEC-BIG to one of its sections.
    let targets: BTreeSet<&str> = daemon_edges
        .iter()
        .map(|edge| {
            assert_eq!(edge["src"], Value::from("MEC-BIG"));
            assert_eq!(edge["type"], Value::from("depends_on"));
            edge["dst"].as_str().expect("dst")
        })
        .collect();
    assert_eq!(targets, ids.iter().map(String::as_str).collect());
    // Deterministic: the same bytes again.
    let again = server.get(&format!("{P}/graph?ref=MEC-BIG"));
    assert_eq!(again.text(), reply.text());
}

#[test]
fn ac03_the_graph_query_404_400_503() {
    let scratch = Scratch::new("graph-query");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a]);

    // 400, nothing run: (target, the message, exactly or the name it
    // must quote in backticks).
    let names = "it takes ref, impact, types, depth, archive";
    let exact: [(&str, String); 7] = [
        ("", "`ref` is required".to_owned()),
        ("?impact=true&depth=2", "`ref` is required".to_owned()),
        (
            "?ref=MEC-SPRINT&ref=MEC-STAMINA",
            "`ref` is given 2 times: it takes one value".to_owned(),
        ),
        (
            "?ref=MEC-SPRINT&impact=yes",
            "`impact=yes`: not `true` or `false`".to_owned(),
        ),
        (
            "?ref=MEC-SPRINT&depth=x",
            "`depth=x`: not a decimal integer".to_owned(),
        ),
        (
            "?ref=MEC-SPRINT&x=1",
            format!("`x` is no query name of graph: {names}"),
        ),
        (
            "?ref=MEC-SPRINT&type=depends_on",
            format!("`type` is no query name of graph: {names}"),
        ),
    ];
    for (query, message) in &exact {
        let path = format!("{P}/graph{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
        assert_eq!(&reply.error_message(), message, "{path}");
    }
    for (query, name) in [
        ("?ref=MEC%2", "`ref`"),
        ("?ref=%FF", "`ref`"),
        ("?ref=MEC-SPRINT&impact=true&impact=false", "`impact`"),
        ("?ref=MEC-SPRINT&depth=1&depth=2", "`depth`"),
        ("?ref=MEC-SPRINT&archive=true&archive=true", "`archive`"),
        ("?ref=MEC-SPRINT&archive=1", "`archive=1`"),
        ("?ref=MEC-SPRINT&depth=1.5", "`depth=1.5`"),
        ("?ref=MEC-SPRINT&kinds=rule", "`kinds`"),
        ("?ref=MEC-SPRINT&types=depends_on&staged=true", "`staged`"),
    ] {
        let path = format!("{P}/graph{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(message.contains(name), "{path}: {message}");
    }
    assert!(
        snapshot(&home).is_empty(),
        "a 400 runs no call: the fresh HOME holds {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );

    // 404: the CLI's exit-1 document.
    for (query, reference) in [
        ("?ref=MEC-NOPE", "MEC-NOPE"),
        ("?ref=", ""),
        ("?ref=NOPE-1&impact=true", "NOPE-1"),
    ] {
        let mut args = vec!["graph", reference];
        if query.contains("impact") {
            args.push("--impact");
        }
        let reply = same(
            &server,
            &home,
            &cwd,
            &a,
            &format!("{P}/graph{query}"),
            &args,
        );
        assert_eq!(reply.status, 404);
        assert!(reply.json()["reason"].is_string(), "{}", reply.text());
    }

    // 503: the CLI cannot run; its line verbatim.
    let lookalike = "MEC-SPR\u{0406}NT";
    for (query, args) in [
        (
            "?ref=MEC-SPRINT&depth=-1".to_owned(),
            vec!["graph", "MEC-SPRINT", "--depth", "-1"],
        ),
        (
            format!("?ref={}", encode_component(lookalike)),
            vec!["graph", lookalike],
        ),
        (
            "?ref=shared%3ADEC-0023".to_owned(),
            vec!["graph", "shared:DEC-0023"],
        ),
    ] {
        let path = format!("{P}/graph{query}");
        let line = cannot_run(&home, &cwd, &a, &args);
        let reply = server.get(&path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(reply.error_message(), line, "{path}: the CLI's line");
    }
}
