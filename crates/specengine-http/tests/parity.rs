//! AC-02 of docs/features/daemon-read.md: parity. With `--root A --root
//! B` under one `HOME`, `tree`, `nodes/<every tree ID and path>`,
//! `inbox`, `proposals/<id>`, `bundle?node_ids=…` answer byte-equal to
//! `spec --root R <cmd> --json` stdout without its final LF (exit 0 →
//! 200, exit 1 → 404); `search` equal but `snippet`, whose structure
//! rendered (`…` per cut flag, a hit between `**`) is the CLI's string; a
//! document over 40 000 characters comes whole (`truncated` false) while
//! `spec show` cuts it; a question on two targets lists both in
//! `target_ids`, in the CLI and the daemon. M: pretty JSON; `notes`
//! dropped; the cap kept; `target_ids` only in the daemon (MCP given the
//! browser view: `specengine-mcp`'s `mcp_read`, `mcp_size`).
//!
//! docs/features/flow-read.md AC-07: `flow`, on both A and B: no query, a
//! depth, a ROOT of each tree. Its own forms are `flow.rs`.
//!
//! AC-01 of docs/features/board-ui.md: over `fixtures/card-press` `GET cards`
//! with every filter is `spec card list --json`, `GET cards/C` is `spec card
//! show C --history --json`, byte for byte (exit 1 a 404); past the cap the
//! list is the browser view ("Routes"): the CLI's rows byte for byte, then
//! the rest, `truncated` false, `omitted` null, where `spec card list` cuts;
//! a show past the cap is still the CLI's document. A, B and the press share
//! one `HOME`, one daemon and a board each (ADR-0008). M: `card_list`
//! called with the capped view.

mod common;

use std::path::{Path, PathBuf};

use common::{
    Reply, Scratch, Server, ask, encode_component, propose_update, run_product, spec_bin,
    spec_json, write,
};
use serde_json::{Map, Value, json};

/// A, B (both repositories), one `HOME`, a cwd outside both.
struct Pair {
    scratch: Scratch,
    a: PathBuf,
    b: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl Pair {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let a = scratch.repo("spec-a", "a", "main");
        let b = scratch.repo("spec-b", "b", "trunk");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            scratch,
            a,
            b,
            home,
            cwd,
        }
    }

    /// The CLI's document of `args` (with `--root root --json`).
    fn cli(&self, root: &Path, args: &[&str]) -> common::Run {
        spec_json(&self.home, &self.cwd, root, args)
    }

    /// Asserts the daemon's `path` is the CLI's `args` byte for byte.
    fn same(&self, server: &Server, root: &Path, path: &str, args: &[&str]) -> Reply {
        let run = self.cli(root, args);
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
        reply
    }

    /// Asserts the CLI cannot run `args` (exit 2) and the daemon's `path`
    /// is a 503 whose message is the CLI's stderr verbatim.
    fn cannot(&self, server: &Server, root: &Path, path: &str, args: &[&str]) {
        let root_text = root.to_str().unwrap();
        let mut all = vec!["--root", root_text];
        all.extend_from_slice(args);
        all.push("--json");
        let run = common::spec(&self.home, &self.cwd, &all);
        run.code(2);
        let reply = server.get(path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            run.stderr.trim_end_matches('\n'),
            "{path}: the CLI's line verbatim"
        );
    }
}

/// A discrepancy's input on `node_ids`.
fn discrepancy(node_ids: &[&str], summary: &str) -> String {
    json!({
        "node_ids": node_ids, "summary": summary, "gap_type": "contradicts",
        "severity": "high",
        "evidence": [{"file": "src/stamina.rs", "qpath": "stamina::drain", "lines": "3-9",
            "observed": "drains while walking", "documented": "only while sprinting"}],
        "options": [{"label": "code", "effect": "fix the code", "price": "1 item"},
            {"label": "spec", "effect": "allow walking", "price": "a rebalance"}],
        "recommendation": 0
    })
    .to_string()
}

/// A tree's IDs and its distinct paths, in order.
fn ids_and_paths(tree: &Value) -> (Vec<String>, Vec<String>) {
    let mut ids = Vec::new();
    let mut paths: Vec<String> = Vec::new();
    for node in tree["nodes"].as_array().expect("nodes") {
        ids.push(node["id"].as_str().expect("id").to_owned());
        let path = node["path"].as_str().expect("path").to_owned();
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    (ids, paths)
}

#[test]
fn ac02_every_document_is_the_clis_byte_for_byte() {
    let pair = Pair::new("parity-docs");
    let (a, b) = (&pair.a, &pair.b);
    // The queue: on A a question on two targets, an update, a
    // discrepancy; on B a question.
    let question = ask(
        &pair.home,
        a,
        &["MEC-SPRINT", "EDGE-STAM-ZERO"],
        "Does a sprint at zero stamina end at once?",
    );
    let update = propose_update(
        &pair.home,
        &pair.cwd,
        a,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.2 s.\n",
    );
    let run = run_product(
        &spec_bin(),
        &pair.home,
        a,
        &[
            "--root",
            a.to_str().unwrap(),
            "propose",
            "discrepancy",
            "--input",
            "-",
        ],
        discrepancy(&["MEC-SPRINT"], "Walking drains stamina in the build.").as_bytes(),
    );
    run.code(0);
    let report = run.stdout.lines().next().unwrap().to_owned();
    let b_question = ask(
        &pair.home,
        b,
        &["CMD-STATUS"],
        "Does it print the plan first?",
    );

    let server = Server::serve(&pair.home, &pair.cwd, &[a, b]);
    for (root, slug, ids_of_queue) in [
        (
            a,
            "lantern-keep",
            vec![question.as_str(), update.as_str(), report.as_str()],
        ),
        (b, "zerkalo", vec![b_question.as_str()]),
    ] {
        let base = format!("/api/projects/{slug}");
        let tree = pair.same(&server, root, &format!("{base}/tree"), &["tree"]);
        pair.same(
            &server,
            root,
            &format!("{base}/tree?depth=1"),
            &["tree", "--depth", "1"],
        );
        pair.same(
            &server,
            root,
            &format!("{base}/tree?depth=0&archive=true"),
            &["tree", "--depth", "0", "--archive"],
        );
        let (ids, paths) = ids_and_paths(&tree.json());
        assert!(ids.len() >= 4, "{slug}: the tree lists its nodes: {ids:?}");
        let kinds: Vec<String> = tree.json()["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["kind"].as_str().unwrap().to_owned())
            .collect();
        pair.same(
            &server,
            root,
            &format!(
                "{base}/tree?kinds={}&kinds={}",
                kinds[0],
                kinds[kinds.len() - 1]
            ),
            &[
                "tree",
                "--kind",
                &kinds[0],
                "--kind",
                &kinds[kinds.len() - 1],
            ],
        );
        pair.same(
            &server,
            root,
            &format!("{base}/tree?root={}", encode_component(&ids[1])),
            &["tree", &ids[1]],
        );
        pair.same(&server, root, &format!("{base}/flow"), &["flow"]);
        pair.same(
            &server,
            root,
            &format!("{base}/flow?depth=1"),
            &["flow", "--depth", "1"],
        );
        pair.same(
            &server,
            root,
            &format!("{base}/flow?root={}", encode_component(&ids[1])),
            &["flow", &ids[1]],
        );
        for id in &ids {
            pair.same(
                &server,
                root,
                &format!("{base}/nodes/{}", encode_component(id)),
                &["show", id],
            );
        }
        for path in &paths {
            pair.same(
                &server,
                root,
                &format!("{base}/nodes/{}", encode_component(path)),
                &["show", path],
            );
        }
        // Records, aliases, `slug/ID`, `ID#SECTION`, a legacy alias.
        let extra: Vec<String> = if slug == "lantern-keep" {
            [
                "R-12",
                "A-101",
                "Q-031",
                "QST-031",
                "DEC-0023",
                "TERM-exhausted",
                "stamina-tuning/AC-07",
                "MEC-STAMINA#RULE-STAM-REGEN",
            ]
            .map(str::to_owned)
            .to_vec()
        } else {
            vec![
                "REQ-001".to_owned(),
                "ADR-0001".to_owned(),
                "GLS-worktree".to_owned(),
                "dry-run/CRIT-01".to_owned(),
                // A legacy alias of REQ-002 in Cyrillic letters (TRB-002).
                "\u{422}\u{420}\u{411}-002".to_owned(),
            ]
        };
        for reference in &extra {
            pair.same(
                &server,
                root,
                &format!("{base}/nodes/{}", encode_component(reference)),
                &["show", reference],
            );
        }
        for id in ids.iter().take(3) {
            pair.same(
                &server,
                root,
                &format!("{base}/nodes/{}?with=links", encode_component(id)),
                &["show", id, "--links"],
            );
            pair.same(
                &server,
                root,
                &format!(
                    "{base}/nodes/{}?with=links&archive=true",
                    encode_component(id)
                ),
                &["show", id, "--links", "--archive"],
            );
        }
        // An unknown ID: the exit-1 document, as a 404.
        let unknown = pair.same(
            &server,
            root,
            &format!("{base}/nodes/NOPE-404"),
            &["show", "NOPE-404"],
        );
        assert_eq!(unknown.status, 404);
        assert!(unknown.json()["reason"].is_string(), "{}", unknown.text());

        // The CLI cannot run: 503, its line verbatim.
        pair.cannot(
            &server,
            root,
            &format!("{base}/search?query=st"),
            &["search", "st"],
        );
        pair.cannot(
            &server,
            root,
            &format!("{base}/tree?depth=-1"),
            &["tree", "--depth", "-1"],
        );
        // MEC-STAMINA, REQ-003 with a Cyrillic E (U+0415) among Latin letters.
        let mixed = if slug == "lantern-keep" {
            "M\u{415}C-STAMINA"
        } else {
            "R\u{415}Q-003"
        };
        pair.cannot(
            &server,
            root,
            &format!("{base}/nodes/{}", encode_component(mixed)),
            &["show", mixed],
        );

        let inbox = pair.same(&server, root, &format!("{base}/inbox"), &["inbox"]);
        let listed: Vec<String> = inbox.json()["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            listed, ids_of_queue,
            "{slug}: the inbox lists its project's queue"
        );
        for id in &ids_of_queue {
            let review = pair.same(
                &server,
                root,
                &format!("{base}/proposals/{id}"),
                &["review", id],
            );
            assert_eq!(review.json()["id"], json!(id));
        }
        pair.same(
            &server,
            root,
            &format!("{base}/proposals/PR-9999"),
            &["review", "PR-9999"],
        );

        pair.same(
            &server,
            root,
            &format!("{base}/bundle?node_ids={}", encode_component(&ids[2])),
            &["bundle", &ids[2]],
        );
        pair.same(
            &server,
            root,
            &format!(
                "{base}/bundle?node_ids={}&node_ids={}&budget=600",
                encode_component(&ids[1]),
                encode_component(&ids[3])
            ),
            &["bundle", &ids[1], &ids[3], "--budget", "600"],
        );
    }

    // D1: the question on two targets lists both, the CLI and the daemon.
    let cli = pair.cli(a, &["inbox"]).json();
    let entry = cli["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == json!(question))
        .expect("the question is listed")
        .clone();
    assert_eq!(entry["target_id"], json!("MEC-SPRINT"));
    assert_eq!(
        entry["target_ids"],
        json!(["MEC-SPRINT", "EDGE-STAM-ZERO"]),
        "{entry}"
    );
    let daemon = server.get("/api/projects/lantern-keep/inbox").json();
    let daemon_entry = daemon["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == json!(question))
        .expect("the question is listed by the daemon")
        .clone();
    assert_eq!(
        daemon_entry["target_ids"],
        json!(["MEC-SPRINT", "EDGE-STAM-ZERO"])
    );
    // An update's target_ids is [target_id]; the key comes right after
    // target_id.
    let raw = pair.cli(a, &["inbox"]).stdout;
    let update_at = raw
        .find(&format!("\"id\":\"{update}\""))
        .expect("update listed");
    assert!(
        raw[update_at..]
            .contains("\"target_id\":\"EDGE-STAM-ZERO\",\"target_ids\":[\"EDGE-STAM-ZERO\"],"),
        "{raw}"
    );
    let _ = &pair.scratch;
}

/// `segments` rendered as the capped view writes them: `…` per cut flag,
/// a hit between `**`.
fn rendered(snippet: &Value) -> String {
    if snippet.is_null() {
        return String::new();
    }
    let object = snippet.as_object().expect("a snippet object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>().len(),
        3,
        "{snippet}"
    );
    let mut out = String::new();
    if snippet["cut_start"].as_bool().expect("cut_start") {
        out.push('\u{2026}');
    }
    for segment in snippet["segments"].as_array().expect("segments") {
        let keys: Vec<&str> = segment
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys.len(), 2, "{segment}");
        let text = segment["text"].as_str().expect("text");
        if segment["hit"].as_bool().expect("hit") {
            out.push_str("**");
            out.push_str(text);
            out.push_str("**");
        } else {
            out.push_str(text);
        }
    }
    if snippet["cut_end"].as_bool().expect("cut_end") {
        out.push('\u{2026}');
    }
    out
}

fn without(value: &Value, key: &str) -> Map<String, Value> {
    let mut object = value.as_object().expect("object").clone();
    object.remove(key);
    object
}

/// A search: the project's root and slug, the query pairs, the CLI's
/// arguments.
type SearchCase<'a> = (&'a Path, &'a str, Vec<(&'a str, String)>, Vec<String>);

#[test]
fn ac02_search_is_the_clis_but_its_structured_snippet() {
    let pair = Pair::new("parity-search");
    let (a, b) = (&pair.a, &pair.b);
    let server = Server::serve(&pair.home, &pair.cwd, &[a, b]);
    // Cyrillic: "command" and "command line".
    let command = "\u{43a}\u{43e}\u{43c}\u{430}\u{43d}\u{434}\u{430}";
    let phrase = "\u{43a}\u{43e}\u{43c}\u{430}\u{43d}\u{434}\u{43d}\u{430}\u{44f} \u{441}\u{442}\u{440}\u{43e}\u{43a}\u{430}";
    let cases: Vec<SearchCase> = vec![
        (
            a,
            "lantern-keep",
            vec![("query", "stamina".into())],
            vec!["search".into(), "stamina".into()],
        ),
        (
            a,
            "lantern-keep",
            vec![("query", "sprint".into()), ("limit", "2".into())],
            vec![
                "search".into(),
                "sprint".into(),
                "--limit".into(),
                "2".into(),
            ],
        ),
        (
            a,
            "lantern-keep",
            vec![
                ("query", "stamina".into()),
                ("kinds", "rule".into()),
                ("kinds", "question".into()),
            ],
            vec![
                "search".into(),
                "stamina".into(),
                "--kind".into(),
                "rule".into(),
                "--kind".into(),
                "question".into(),
            ],
        ),
        (
            a,
            "lantern-keep",
            vec![("query", "lantern".into()), ("archive", "true".into())],
            vec!["search".into(), "lantern".into(), "--archive".into()],
        ),
        (
            a,
            "lantern-keep",
            vec![("query", "zzzqqq".into())],
            vec!["search".into(), "zzzqqq".into()],
        ),
        (
            b,
            "zerkalo",
            vec![("query", "sync".into())],
            vec!["search".into(), "sync".into()],
        ),
        (
            b,
            "zerkalo",
            vec![("query", command.into())],
            vec!["search".into(), command.into()],
        ),
        (
            b,
            "zerkalo",
            vec![("query", phrase.into())],
            vec!["search".into(), phrase.into()],
        ),
    ];
    let mut snippets_seen = 0;
    for (root, slug, query, args) in cases {
        let query_text: Vec<String> = query
            .iter()
            .map(|(name, value)| format!("{name}={}", encode_component(value)))
            .collect();
        let path = format!("/api/projects/{slug}/search?{}", query_text.join("&"));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = pair.cli(root, &args);
        let reply = server.get(&path);
        assert_eq!(
            reply.status,
            if run.code == 0 { 200 } else { 404 },
            "{path}: {}",
            reply.text()
        );
        let (cli, daemon) = (run.json(), reply.json());
        assert_eq!(
            without(&cli, "hits"),
            without(&daemon, "hits"),
            "{path}: the document but its hits"
        );
        // The same keys in the same order, compact.
        let strip = |text: &str| -> String {
            let value: Value = serde_json::from_str(text).unwrap();
            let mut keys = Vec::new();
            for (key, _) in value.as_object().unwrap() {
                keys.push(key.clone());
            }
            keys.join(",")
        };
        assert_eq!(strip(run.document()), strip(reply.text()));
        assert!(
            !reply.text().contains('\n') && !reply.text().starts_with("{ "),
            "{path}: compact"
        );
        let cli_hits = cli["hits"].as_array().cloned().unwrap_or_default();
        let daemon_hits = daemon["hits"].as_array().cloned().unwrap_or_default();
        assert_eq!(cli_hits.len(), daemon_hits.len(), "{path}");
        for (cli_hit, daemon_hit) in cli_hits.iter().zip(&daemon_hits) {
            assert_eq!(
                without(cli_hit, "snippet"),
                without(daemon_hit, "snippet"),
                "{path}"
            );
            let cli_snippet = cli_hit["snippet"]
                .as_str()
                .expect("the CLI's snippet is a string");
            assert_eq!(
                rendered(&daemon_hit["snippet"]),
                cli_snippet,
                "{path}: the structure renders to the CLI's text: {}",
                daemon_hit["snippet"]
            );
            if !daemon_hit["snippet"].is_null() {
                snippets_seen += 1;
                assert!(
                    daemon_hit["snippet"]["segments"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|segment| segment["hit"] == json!(true)),
                    "{path}: a hit segment: {}",
                    daemon_hit["snippet"]
                );
            }
        }
        // The raw daemon hit: the snippet as an object, the keys in order.
        if let Some(first) = daemon_hits.first() {
            let at = reply.text().find("\"snippet\":").expect("snippet key");
            let next = reply.text()[at + 10..].chars().next();
            assert!(
                matches!(next, Some('{') | Some('n')),
                "{path}: the browser snippet is an object or null: {first}"
            );
        }
    }
    assert!(
        snippets_seen >= 6,
        "the cases carry snippets: {snippets_seen}"
    );
}

/// About 56 000 characters of one node.
fn long_document() -> String {
    let mut text = String::from(
        "---\nid: MEC-LONG\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Long watch\n\
         parent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # Long watch\n\nThe keeper walks the wall all night.\n\n",
    );
    for line in 0..800 {
        text.push_str(&format!(
            "- Step {line:04}: the lantern keeps burning while the keeper walks.\n"
        ));
    }
    text
}

#[test]
fn ac02_a_document_over_40000_characters_comes_whole_while_spec_show_cuts() {
    let pair = Pair::new("parity-long");
    let a = &pair.a;
    let long = long_document();
    assert!(long.chars().count() > 50_000);
    write(a, "docs/spec/movement/long.md", &long);
    let server = Server::serve(&pair.home, &pair.cwd, &[a]);

    let run = pair.cli(a, &["show", "MEC-LONG"]);
    run.code(0);
    let cli = run.json();
    let reply = server.get("/api/projects/lantern-keep/nodes/MEC-LONG");
    reply.status(200);
    let daemon = reply.json();
    assert!(reply.text().chars().count() > 50_000);
    let keys =
        |value: &Value| -> Vec<String> { value.as_object().unwrap().keys().cloned().collect() };
    assert_eq!(keys(&cli), keys(&daemon), "the same keys");
    assert_eq!(
        keys(&cli["nodes"][0]),
        keys(&daemon["nodes"][0]),
        "the same node keys"
    );
    assert_eq!(
        cli["nodes"][0]["truncated"],
        json!(true),
        "spec show cuts: {}",
        &run.stdout[..300]
    );
    assert!(
        cli["nodes"][0]["omitted"].is_object(),
        "the CLI names what it cut"
    );
    assert_eq!(
        daemon["nodes"][0]["truncated"],
        json!(false),
        "the browser view is whole"
    );
    assert_eq!(
        daemon["nodes"][0]["omitted"],
        Value::Null,
        "nothing omitted"
    );
    let text = daemon["nodes"][0]["text"]
        .as_str()
        .expect("the node's text");
    assert_eq!(text, long, "the node's text whole");
    let cli_text = cli["nodes"][0]["text"].as_str().unwrap_or_default();
    assert!(cli_text.chars().count() < 40_000, "the CLI's text is cut");
    let mut cli_node = without(&cli["nodes"][0], "text");
    let mut daemon_node = without(&daemon["nodes"][0], "text");
    for key in ["truncated", "omitted"] {
        cli_node.remove(key);
        daemon_node.remove(key);
    }
    assert_eq!(cli_node, daemon_node, "the node but its text and cut");
    assert_eq!(without(&cli, "nodes"), without(&daemon, "nodes"));

    // With the long node in it, the rest of the parity holds: a small
    // node is byte-equal still.
    let small = pair.cli(a, &["show", "MEC-STAMINA"]);
    assert_eq!(
        server
            .get("/api/projects/lantern-keep/nodes/MEC-STAMINA")
            .text(),
        small.document()
    );
}

// -------------------------------------------------------------- lock-diff

/// `GET <path>` against `spec --root <root> <args> --json`: exit 0 a 200,
/// exit 1 a 404, the body the CLI's document byte for byte less its LF,
/// JSON, `no-store`, the same bytes repeated.
fn lock_diff_same(
    server: &Server,
    home: &Path,
    cwd: &Path,
    root: &Path,
    path: &str,
    args: &[&str],
) -> Reply {
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

/// docs/features/lock-diff.md RD-7 ("Route"): `GET
/// /api/projects/code-a/lock-diff?ref=<REF>` over a git copy of
/// `fixtures/code-a` holding the golden lock, every entry's `commit` set to
/// the fixture's commit (so both sides are shown), LAW-BOUND's sentence
/// and `CAP` edited, uncommitted. Each 400 of "Route" first, nothing run
/// (the fresh `HOME` holds nothing beyond the daemon's); each 200 body is
/// `spec --root R lock diff REF --json` byte for byte less its final LF
/// (LAW-BOUND, its document, a path REF, a written `@rev`, a REF with
/// blanks; two REFs, two bodies); a REF naming nothing a 404 with the
/// exit-1 document; a look-alike and a `project:` REF a 503 with the CLI's
/// exit-2 line. M7 (the route ignoring `ref`) red.
#[test]
fn rd7_lock_diff_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("parity-lock-diff");
    let root = scratch.repo("code-a", "code", "main");
    let git = scratch.git();
    let head = git.run(&root, &["rev-parse", "HEAD"]).trim().to_owned();
    let golden = common::read_text(&common::fixture("spec-lock"), "code-a.lock");
    assert!(
        golden.contains("\ncommit = \""),
        "premise: the golden's commits"
    );
    let lock: String = golden
        .lines()
        .map(|line| {
            if line.starts_with("commit = ") {
                format!("commit = \"{head}\"\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    write(&root, "spec.lock", &lock);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "the lock"]);
    common::replace(
        &root,
        "docs/spec/counter.md",
        "The count never exceeds the cap.",
        "The count never exceeds the cap, twelve by default.",
    );
    common::replace(
        &root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let p = "/api/projects/code-a/lock-diff";

    // 400, nothing run.
    for (query, message) in [
        ("", "`ref` is required"),
        (
            "?ref=LAW-BOUND&ref=LAW-STEP",
            "`ref` is given 2 times: it takes one value",
        ),
        (
            "?ref=LAW-BOUND&x=1",
            "`x` is no query name of lock-diff: it takes ref",
        ),
        (
            "?ref=LAW-BOUND&json=true",
            "`json` is no query name of lock-diff: it takes ref",
        ),
    ] {
        let path = format!("{p}{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
        assert_eq!(reply.error_message(), message, "{path}");
    }
    for query in ["?ref=LAW%2", "?ref=%FF"] {
        let path = format!("{p}{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(message.contains("`ref`"), "{path}: {message}");
    }
    assert!(
        common::beyond_daemon(&home).is_empty(),
        "a 400 runs no call: the fresh HOME holds {:?}",
        common::beyond_daemon(&home)
    );

    // 200: the CLI's exit-0 document.
    let mut bodies = Vec::new();
    for (query, reference) in [
        ("LAW-BOUND", "LAW-BOUND"),
        ("PIECE-COUNTER", "PIECE-COUNTER"),
        ("docs%2Fspec%2Fcounter.md", "docs/spec/counter.md"),
        ("LAW-BOUND%402", "LAW-BOUND@2"),
        ("%20LAW-BOUND%20", " LAW-BOUND "),
    ] {
        let reply = lock_diff_same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{p}?ref={query}"),
            &["lock", "diff", reference],
        );
        assert_eq!(reply.status, 200, "{query}");
        assert_eq!(reply.json()["ref"], reference, "`ref` echoed raw");
        bodies.push(reply.text().to_owned());
    }
    assert_ne!(bodies[0], bodies[1], "premise: two REFs, two documents");
    assert!(
        bodies[0].contains("\"spec\":{\"commit\":")
            && bodies[0].contains("\"code\":{\"blob\":")
            && bodies[0].contains("\"diff\":\"@@"),
        "premise: both sides shown: {}",
        bodies[0]
    );

    // 404: the CLI's exit-1 document.
    for (query, reference) in [("NOPE-9", "NOPE-9"), ("", "")] {
        let reply = lock_diff_same(
            &server,
            &home,
            &cwd,
            &root,
            &format!("{p}?ref={query}"),
            &["lock", "diff", reference],
        );
        assert_eq!(reply.status, 404, "{query:?}");
        let document = reply.json();
        assert!(document["reason"].is_string(), "{}", reply.text());
        assert_eq!(document["nodes"], json!([]), "{}", reply.text());
    }

    // 503: the CLI cannot run; its line verbatim.
    let lookalike = "L\u{410}W-BOUND";
    for (query, reference) in [
        (encode_component(lookalike), lookalike),
        ("other%3ALAW-BOUND".to_owned(), "other:LAW-BOUND"),
    ] {
        let path = format!("{p}?ref={query}");
        let root_text = root.to_str().expect("UTF-8 root");
        let run = common::spec(
            &home,
            &cwd,
            &["--root", root_text, "lock", "diff", reference, "--json"],
        );
        run.code(2);
        assert_eq!(run.stdout, "", "exit 2 prints no document: {}", run.show());
        let reply = server.get(&path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            run.stderr.trim_end_matches('\n'),
            "{path}: the CLI's line verbatim"
        );
    }
}

// ---------------------------------------------------------------- cards

/// The press's routes.
const PRESS: &str = "/api/projects/card-press";

/// A card through `spec card new` under the pair's `HOME`; its ID.
fn new_card(pair: &Pair, root: &Path, flags: &[&str]) -> String {
    let mut args = vec!["card", "new"];
    args.extend_from_slice(flags);
    let run = pair.cli(root, &args);
    run.code(0);
    run.json()["id"].as_str().expect("an ID").to_owned()
}

/// A same-origin page's POST, the answer a 200.
fn page_post(server: &Server, path: &str, body: &Value) {
    let reply = server.stage(path, &body.to_string());
    assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
}

#[test]
fn board_ac01_the_board_and_a_card_are_the_clis_byte_for_byte() {
    let pair = Pair::new("parity-cards");
    let (a, b) = (&pair.a, &pair.b);
    let press = pair.scratch.repo("card-press", "press", "main");
    let server = Server::serve(&pair.home, &pair.cwd, &[a, b, &press]);

    // The empty board: its flows, nothing seeded; a project with no `[card]`
    // the default flow; the same bytes on all three.
    let cards = format!("{PRESS}/cards");
    let empty = pair.same(&server, &press, &cards, &["card", "list"]);
    let expected: Value = serde_json::from_str(&common::read_text(
        &common::fixture("card-press"),
        "expected.json",
    ))
    .expect("expected.json");
    // docs/canon/cards.md "Documents and events": `return` after `accept`,
    // `null` on card-press, whose fixture stays byte-equal (board-handoff).
    let mut flows = expected["flows"].clone();
    for flow in flows.as_array_mut().expect("flows") {
        assert!(
            flow.get("return").is_none(),
            "the fixture predates `return`"
        );
        flow["return"] = Value::Null;
    }
    assert_eq!(empty.json()["flows"], flows, "the fixture's");
    assert_eq!(empty.json()["cards"], json!([]));
    assert_eq!(empty.json()["truncated"], json!(false));
    assert_eq!(empty.json()["omitted"], Value::Null);
    for (root, slug) in [(a, "lantern-keep"), (b, "zerkalo")] {
        let path = format!("/api/projects/{slug}/cards");
        let reply = pair.same(&server, root, &path, &["card", "list"]);
        assert_eq!(reply.json()["flows"][0]["name"], json!("plain"), "{slug}");
    }

    // One card of each ending, written by the CLI and by the page.
    let lead = new_card(
        &pair,
        &press,
        &[
            "--title",
            "Lead story",
            "--discipline",
            "copy",
            "--target",
            "SHT-FRONT",
            "--assignee",
            "ana",
            "--estimate",
            "2-3",
            "--due",
            "2026-03-01",
            "--criterion",
            "Fits the page.",
            "--as",
            "desk",
        ],
    );
    let plain = new_card(
        &pair,
        &press,
        &["--title", "Back page", "--discipline", "copy"],
    );
    let plate = new_card(
        &pair,
        &press,
        &["--title", "Front plate", "--discipline", "plates"],
    );
    let gone = new_card(
        &pair,
        &press,
        &["--title", "Spare plate", "--discipline", "plates"],
    );
    let done = new_card(
        &pair,
        &press,
        &[
            "--title",
            "Weather box",
            "--discipline",
            "copy",
            "--assignee",
            "ana",
        ],
    );
    pair.cli(&press, &["card", "move", &plate, "--to", "blocked"])
        .code(0);
    pair.cli(&press, &["card", "cancel", &gone, "--reason", "Pulled."])
        .code(0);
    pair.cli(
        &press,
        &["card", "edit", &lead, "--evidence", "note=first pass"],
    )
    .code(0);
    pair.cli(&press, &["card", "assign", &done, "--reviewer", "ben"])
        .code(0);
    pair.cli(
        &press,
        &["card", "move", &done, "--to", "edit", "--as", "desk"],
    )
    .code(0);
    pair.cli(&press, &["card", "move", &done, "--to", "proof"])
        .code(0);
    page_post(
        &server,
        &format!("{cards}/{done}/accept"),
        &json!({"briefed": true, "as": "ben", "note": "Read twice."}),
    );
    new_card(&pair, a, &["--title", "Refill the oil"]);
    new_card(&pair, b, &["--title", "Mend the net"]);

    for (query, flags) in [
        ("", &[][..]),
        ("?with_cancelled=true", &["--with-cancelled"][..]),
        ("?with_cancelled=false", &[][..]),
        ("?no_target=false", &[][..]),
        ("?flow=copy", &["--flow", "copy"][..]),
        ("?flow=nope", &["--flow", "nope"][..]),
        ("?column=draft", &["--column", "draft"][..]),
        ("?column=blocked", &["--column", "blocked"][..]),
        ("?discipline=plates", &["--discipline", "plates"][..]),
        ("?assignee=ana", &["--assignee", "ana"][..]),
        ("?no_target=true", &["--no-target"][..]),
        (
            "?no_target=true&with_cancelled=true",
            &["--no-target", "--with-cancelled"][..],
        ),
        (
            "?flow=copy&assignee=ana&column=filed",
            &["--flow", "copy", "--assignee", "ana", "--column", "filed"][..],
        ),
    ] {
        let mut args = vec!["card", "list"];
        args.extend_from_slice(flags);
        let path = format!("{cards}{query}");
        let first = pair.same(&server, &press, &path, &args);
        assert_eq!(first.text(), server.get(&path).text(), "{path}: one state");
    }
    let whole = pair.same(
        &server,
        &press,
        &format!("{cards}?with_cancelled=true"),
        &["card", "list", "--with-cancelled"],
    );
    // By flow, column in config order, a cancelled card last in its column.
    let ids: Vec<String> = whole.json()["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID").to_owned())
        .collect();
    assert_eq!(
        ids,
        [
            lead.as_str(),
            plain.as_str(),
            done.as_str(),
            gone.as_str(),
            plate.as_str()
        ],
        "{}",
        whole.text()
    );
    assert_eq!(whole.json()["cards"][2]["accepted"], json!(true));
    assert_eq!(whole.json()["cards"][3]["cancelled"], json!(true));
    for id in [&lead, &plain, &plate, &gone, &done] {
        let path = format!("{cards}/{id}");
        pair.same(&server, &press, &path, &["card", "show", id, "--history"]);
    }
    let shown = server.get(&format!("{cards}/{done}")).status(200).json();
    assert_eq!(shown["acceptance"]["by"], json!("ben"), "{shown}");
    assert!(shown["history"].is_array(), "uncut, `--history`: {shown}");
    for id in ["C-0099", "foo"] {
        pair.same(
            &server,
            &press,
            &format!("{cards}/{id}"),
            &["card", "show", id, "--history"],
        );
    }
    pair.same(
        &server,
        a,
        "/api/projects/lantern-keep/cards",
        &["card", "list"],
    );
    pair.same(
        &server,
        b,
        "/api/projects/zerkalo/cards/C-0001",
        &["card", "show", "C-0001", "--history"],
    );
    // A look-alike is the CLI's exit 2: a 503, its line.
    let look_alike = "\u{0421}-0001";
    pair.cannot(
        &server,
        &press,
        &format!("{cards}/{}", encode_component(look_alike)),
        &["card", "show", look_alike, "--history"],
    );
}

/// "Rules and edge cases": a card whose column `[card]` no longer lists: the
/// daemon's list and show stay the CLI's, the note in `notes`, its row
/// where a page draws the last lane. The config is read per call.
#[test]
fn board_a_card_outside_its_flow_is_served_as_the_cli_serves_it() {
    let pair = Pair::new("parity-cards-stranded");
    let press = pair.scratch.repo("card-press", "press", "main");
    let server = Server::serve(&pair.home, &pair.cwd, &[&press]);
    let cards = format!("{PRESS}/cards");
    let stuck = new_card(
        &pair,
        &press,
        &["--title", "Stuck", "--discipline", "plates"],
    );
    new_card(
        &pair,
        &press,
        &["--title", "Fine", "--discipline", "plates"],
    );
    pair.cli(&press, &["card", "move", &stuck, "--to", "blocked"])
        .code(0);
    common::replace(
        &press,
        "specengine.toml",
        "columns = [\"queued\", \"blocked\", \"checked\", \"struck\"]",
        "columns = [\"queued\", \"checked\", \"struck\"]",
    );
    let reply = pair.same(&server, &press, &cards, &["card", "list"]);
    let board = reply.json();
    assert_eq!(board["cards"][1]["id"], json!(stuck), "last: {board}");
    assert_eq!(board["cards"][1]["column"], json!("blocked"), "{board}");
    assert!(
        board["notes"].to_string().contains(&stuck),
        "the daemon's note: {board}"
    );
    let path = format!("{cards}/{stuck}");
    pair.same(
        &server,
        &press,
        &path,
        &["card", "show", &stuck, "--history"],
    );
}

/// AC-01 and "Routes": past `OUTPUT_CAP_CHARS`. The CLI cuts `list`; the
/// daemon's browser view does not, and holds the CLI's bytes up to the cut;
/// each half of the board, under the cap, is the CLI's byte for byte, so
/// every row past the cut has an oracle too. A show past the cap is the
/// CLI's document: only its text is cut. M: the capped view.
#[test]
fn board_ac01_past_the_cap_the_board_comes_whole_and_a_card_stays_the_clis() {
    let pair = Pair::new("parity-cards-cap");
    let press = pair.scratch.repo("card-press", "press", "main");
    let server = Server::serve(&pair.home, &pair.cwd, &[&press]);
    let cards = format!("{PRESS}/cards");
    let title = "t".repeat(240);
    for card in 0..200 {
        let assignee = if card % 2 == 0 { "ana" } else { "ben" };
        page_post(
            &server,
            &cards,
            &json!({"title": title, "discipline": "copy", "assignee": assignee}),
        );
    }

    let cut = pair.cli(&press, &["card", "list", "--with-cancelled"]);
    cut.code(0);
    let capped = cut.json();
    assert_eq!(capped["truncated"], json!(true), "the CLI cuts");
    let omitted = capped["omitted"].as_u64().expect("the CLI's count");
    let shown = capped["cards"].as_array().expect("cards").len();
    assert_eq!(shown as u64 + omitted, 200);
    assert!(omitted > 0 && shown > 0, "{shown} + {omitted}");

    let reply = server.get(&format!("{cards}?with_cancelled=true"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(reply.text().chars().count() > 40_000, "past the cap");
    let whole = reply.json();
    assert_eq!(
        whole["truncated"],
        json!(false),
        "the browser view is whole"
    );
    assert_eq!(whole["omitted"], Value::Null, "nothing omitted");
    assert_eq!(whole["cards"].as_array().map(Vec::len), Some(200));
    assert_eq!(whole["flows"], capped["flows"]);
    assert_eq!(whole["notes"], capped["notes"]);
    let keys =
        |value: &Value| -> Vec<String> { value.as_object().unwrap().keys().cloned().collect() };
    assert_eq!(keys(&whole), keys(&capped), "the same keys");
    let rows = cut
        .document()
        .split_once("],\"notes\":")
        .expect("the document's rows")
        .0;
    assert!(
        reply.text().starts_with(rows),
        "the CLI's bytes, then the rest"
    );
    assert!(
        reply
            .text()
            .ends_with("],\"notes\":[],\"truncated\":false,\"omitted\":null}"),
        "{}",
        &reply.text()[reply.text().len() - 80..]
    );

    // A filter brings each half under the cap: the CLI's bytes, and with
    // them every row of the whole board.
    let mut halves: Vec<Value> = Vec::new();
    for assignee in ["ana", "ben"] {
        let half = pair.same(
            &server,
            &press,
            &format!("{cards}?assignee={assignee}"),
            &["card", "list", "--assignee", assignee],
        );
        let half = half.json();
        assert_eq!(
            half["truncated"],
            json!(false),
            "{assignee} is under the cap"
        );
        halves.extend(half["cards"].as_array().expect("cards").iter().cloned());
    }
    let mut rows: Vec<Value> = whole["cards"].as_array().expect("cards").clone();
    let id = |row: &Value| row["id"].as_str().expect("an ID").to_owned();
    rows.sort_by_key(id);
    halves.sort_by_key(id);
    assert_eq!(rows, halves, "every row is a row of the CLI's");

    // A card past the cap: goal, 32 criteria and 32 evidence entries of 1 KB.
    let criteria = vec!["c".repeat(1024); 32];
    let evidence = vec![format!("note={}", "n".repeat(1024)); 32];
    page_post(
        &server,
        &format!("{cards}/C-0002/edit"),
        &json!({"goal": "g".repeat(4096), "criterion": criteria, "evidence": evidence}),
    );
    let reply = pair.same(
        &server,
        &press,
        &format!("{cards}/C-0002"),
        &["card", "show", "C-0002", "--history"],
    );
    assert!(reply.text().chars().count() > 40_000, "past the cap");
}
