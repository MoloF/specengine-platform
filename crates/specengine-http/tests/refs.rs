//! AC-04 of docs/features/daemon-read.md: REF decoding and snippets.
//! Everything after `nodes/` is percent-decoded once as UTF-8 and given
//! to `show` as is: a path, a `<slug>%2F<ID>`, an `ID%23SECTION` answer as
//! `spec show` of the decoded REF; `%2523` reaches the CLI as `%23`; an
//! unknown ID is a 404 byte-equal to the CLI's exit-1 document; a bad `%`
//! or non-UTF-8 is a 400. A section holding `**bold**` and the searched
//! term gives exactly one hit segment, `**bold**` plain. Also "Data"
//! "Query": the 400s of a query string. M: decoding twice; 404 without
//! the document; `**` markers reused.

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, Server, replace, spec_json};
use serde_json::{Value, json};

struct One {
    _scratch: Scratch,
    a: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

fn one(name: &str) -> One {
    let scratch = Scratch::new(name);
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    One {
        _scratch: scratch,
        a,
        home,
        cwd,
    }
}

/// `GET path` equals `spec --root a show <reference> --json`, status by
/// exit (0: 200, 1: 404).
fn shows(server: &Server, one: &One, path: &str, reference: &str) -> Value {
    let run = spec_json(&one.home, &one.cwd, &one.a, &["show", reference]);
    let reply = server.get(path);
    let want = if run.code == 0 { 200 } else { 404 };
    assert_eq!(reply.status, want, "{path}: {}", reply.text());
    assert_eq!(
        reply.text(),
        run.document(),
        "{path} answers as `spec show {reference}`"
    );
    reply.json()
}

#[test]
fn ac04_a_ref_is_percent_decoded_once_and_given_to_show_as_is() {
    let one = one("refs-decode");
    let server = Server::serve(&one.home, &one.cwd, &[&one.a]);
    let base = "/api/projects/lantern-keep/nodes";

    let path = shows(
        &server,
        &one,
        &format!("{base}/docs%2Fspec%2Fmovement%2Fstamina.md"),
        "docs/spec/movement/stamina.md",
    );
    assert_eq!(
        path["nodes"][0]["path"],
        json!("docs/spec/movement/stamina.md")
    );
    // The same path with its slashes as written.
    shows(
        &server,
        &one,
        &format!("{base}/docs/spec/movement/stamina.md"),
        "docs/spec/movement/stamina.md",
    );
    let slugged = shows(
        &server,
        &one,
        &format!("{base}/stamina-tuning%2FAC-07"),
        "stamina-tuning/AC-07",
    );
    assert_eq!(slugged["nodes"][0]["id"], json!("AC-07"));
    let section = shows(
        &server,
        &one,
        &format!("{base}/MEC-STAMINA%23RULE-STAM-REGEN"),
        "MEC-STAMINA#RULE-STAM-REGEN",
    );
    assert_eq!(section["nodes"][0]["id"], json!("RULE-STAM-REGEN"));
    // Lower-case escapes decode as well.
    shows(
        &server,
        &one,
        &format!("{base}/MEC-STAMINA%23rule-stam-regen"),
        "MEC-STAMINA#rule-stam-regen",
    );

    // `%2523`: decoded once, the CLI gets `%23`.
    let once = shows(
        &server,
        &one,
        &format!("{base}/MEC-STAMINA%2523X"),
        "MEC-STAMINA%23X",
    );
    assert_eq!(once["ref"], json!("MEC-STAMINA%23X"), "{once}");
    let twice = spec_json(&one.home, &one.cwd, &one.a, &["show", "MEC-STAMINA#X"]);
    assert_ne!(
        server.get(&format!("{base}/MEC-STAMINA%2523X")).text(),
        twice.document(),
        "decoded twice it would be `MEC-STAMINA#X`"
    );

    // An unknown ID: the CLI's exit-1 document, as a 404.
    let unknown = shows(&server, &one, &format!("{base}/MEC-NOPE"), "MEC-NOPE");
    assert!(
        unknown["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty())
    );
    assert_eq!(unknown["nodes"], json!([]));
    assert_eq!(
        unknown
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()
            .len(),
        4,
        "the show document, not the error body: {unknown}"
    );

    // A bad escape or a non-UTF-8 REF: 400, the error body.
    for raw in ["X%zz", "X%2", "%FF", "MEC-STAMINA%", "%C3%28"] {
        let reply = server.get(&format!("{base}/{raw}"));
        assert_eq!(reply.status, 400, "{raw}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(raw),
            "{raw}: the message names it: {message}"
        );
    }
    assert!(
        server.get("/api/projects/lantern-keep/nodes/").status == 404,
        "an empty REF names no node"
    );
}

#[test]
fn ac04_a_corpus_bold_is_text_and_the_term_the_one_hit() {
    let one = one("refs-snippet");
    replace(
        &one.a,
        "docs/spec/movement/stamina.md",
        "- While `Exhausted`, rate \u{d7} 0.5.\n",
        "- While `Exhausted`, rate \u{d7} 0.5.\n- At the campfire the keeper rests while **idle**.\n",
    );
    let server = Server::serve(&one.home, &one.cwd, &[&one.a]);
    let reply = server.get("/api/projects/lantern-keep/search?query=campfire");
    reply.status(200);
    let hits = reply.json()["hits"].as_array().cloned().expect("hits");
    let hit = hits
        .iter()
        .find(|hit| hit["id"] == json!("RULE-STAM-REGEN"))
        .unwrap_or_else(|| panic!("the section is a hit: {}", reply.text()));
    let snippet = &hit["snippet"];
    let segments = snippet["segments"].as_array().expect("segments");
    let hit_segments: Vec<&Value> = segments
        .iter()
        .filter(|segment| segment["hit"] == json!(true))
        .collect();
    assert_eq!(
        hit_segments,
        [&json!({"text": "campfire", "hit": true})],
        "exactly one hit segment, the term: {snippet}"
    );
    let plain: String = segments
        .iter()
        .filter(|segment| segment["hit"] == json!(false))
        .map(|segment| segment["text"].as_str().unwrap())
        .collect();
    assert!(
        plain.contains("**idle**"),
        "the corpus `**idle**` is plain text: {snippet}"
    );
    assert!(snippet["cut_start"].is_boolean() && snippet["cut_end"].is_boolean());
    // The capped view renders it back: the term between `**`, the corpus
    // `**idle**` as written.
    let cli = spec_json(&one.home, &one.cwd, &one.a, &["search", "campfire"]).json();
    let cli_hit = cli["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hit| hit["id"] == json!("RULE-STAM-REGEN"))
        .expect("the CLI's hit")
        .clone();
    let text = cli_hit["snippet"].as_str().expect("a string");
    assert!(
        text.contains("**campfire**") && text.contains("**idle**"),
        "{text}"
    );
}

/// The 400s of a query string: an unknown name (listing the endpoint's),
/// a scalar repeated, a bad value, `query` or `node_ids` missing.
#[test]
fn a_query_string_is_checked_before_any_call() {
    let one = one("refs-query");
    let server = Server::serve(&one.home, &one.cwd, &[&one.a]);
    let p = "/api/projects/lantern-keep";
    for (path, named) in [
        (format!("{p}/tree?deep=1"), "root, depth, kinds, archive"),
        (format!("{p}/tree?depth=1&depth=2"), "depth"),
        (format!("{p}/tree?depth=1.5"), "depth=1.5"),
        (format!("{p}/tree?archive=yes"), "archive=yes"),
        (
            format!("{p}/nodes/MEC-STAMINA?with=parents"),
            "with=parents",
        ),
        (format!("{p}/nodes/MEC-STAMINA?links=true"), "with, archive"),
        (format!("{p}/search"), "query"),
        (format!("{p}/search?query=stamina&query=sprint"), "query"),
        (format!("{p}/search?query=stamina&limit=ten"), "limit=ten"),
        (
            format!("{p}/search?q=stamina"),
            "query, kinds, limit, archive",
        ),
        (format!("{p}/bundle"), "node_ids"),
        (
            format!("{p}/bundle?node_ids=MEC-STAMINA&budget=1e3"),
            "budget=1e3",
        ),
        (format!("{p}/inbox?all=true"), "all"),
        (format!("{p}/proposals/PR-0001?brief=true"), "brief"),
        (format!("{p}/events?after=3"), "after"),
        ("/api/projects?x=1".to_owned(), "x"),
    ] {
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(named),
            "{path}: {message:?} names {named:?}"
        );
    }
    assert!(
        common::files_ending(&one.home, ".db").is_empty(),
        "a refused query ran no call"
    );
    // A `+` in a query value is a space (form decoding): one search of
    // two terms.
    let run = spec_json(&one.home, &one.cwd, &one.a, &["search", "lantern keep"]);
    let reply = server.get(&format!("{p}/search?query=lantern+keep"));
    assert_eq!(reply.status, if run.code == 0 { 200 } else { 404 });
    let (cli, daemon) = (run.json(), reply.json());
    assert_eq!(cli["query"], daemon["query"]);
    assert_eq!(
        cli["hits"].as_array().map(Vec::len),
        daemon["hits"].as_array().map(Vec::len)
    );
    let _: &Path = &one.a;
}
