//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the daemon's half: a local daemon of its own over the scene of
//! `specengine-cli`'s `tests/practices_scene` (a scratch git copy of
//! `fixtures/studio-lamp` with the practices, its queue made through the
//! library at fixed clocks), included by path with `common/git.rs`, under
//! the scene's `HOME`.
//!
//! - DP14: `GET …/cards/C-0001?practices=true` is `spec card show C-0001
//!   --history --practices --json` byte for byte (C-0001 has no parent, so
//!   the daemon's `upstream` is `null` as the CLI's); without the query and
//!   with `practices=false` it is `--history --json`, `practices` and
//!   `cited` `null`; `?practices=maybe` is a 400 naming the name, nothing
//!   read. M: the route ignoring the query.
//! - DP13: `GET …/proposals/PR-0001` is `spec review PR-0001 --json` byte
//!   for byte, its `cited` `PRA-ART-NORMALS` alone.
//! - "The library": `GET …/library`'s row of art.md holds `disciplines`
//!   `["art"]` as its 23rd key, after `tags`.
//!
//! The key sets (`Card` 38, `CardPractice`, `Cited`, `LibraryRow` 23) are
//! `daemon_keys.rs`' and `fixtures/daemon-keys.json`'s.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/practices_scene/mod.rs"]
mod practices_scene;

use common::{Server, spec};
use practices_scene::{ART, NORMALS, SLUG, Scene};
use serde_json::{Value, json};

/// A local daemon over the scene under its `HOME`.
fn serve(scene: &Scene) -> Server {
    let cwd = scene.dir("cwd");
    Server::serve(&scene.home, &cwd, &[&scene.root])
}

/// `spec --root R <args> --json` under the scene's `HOME`, exit 0: its
/// document less its LF.
fn cli(scene: &Scene, args: &[&str]) -> String {
    let root = scene.root.to_str().expect("a UTF-8 root");
    let mut all = vec!["--root", root];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(&scene.home, &scene.root, &all);
    run.code(0);
    run.document().to_owned()
}

/// `GET path`: 200, its body.
fn body(server: &Server, path: &str) -> String {
    let reply = server.get(path);
    assert_eq!(reply.status, 200, "{path}: {}", reply.text());
    reply.text().to_owned()
}

/// DP14: the card route with and without `practices`, byte for byte the
/// CLI's; `maybe` a 400. M: the route ignoring the query.
#[test]
fn dp14_the_card_route_asks_practices_on_request() {
    let scene = Scene::new("http-dp14");
    let server = serve(&scene);
    let route = format!("/api/projects/{SLUG}/cards/C-0001");
    let asked = body(&server, &format!("{route}?practices=true"));
    assert_eq!(
        asked,
        cli(
            &scene,
            &["card", "show", "C-0001", "--history", "--practices"]
        ),
        "byte for byte the CLI's"
    );
    let document: Value = serde_json::from_str(&asked).expect("JSON");
    assert_eq!(
        document["practices"].as_array().map(|list| list.len()),
        Some(2),
        "M: the route ignoring the query: {asked}"
    );
    assert_eq!(document["cited"][0]["name"], json!("PRA-ART-LOD"));
    assert_eq!(
        document["context"],
        Value::Null,
        "the daemon asks no context"
    );
    for query in ["", "?practices=false"] {
        let plain = body(&server, &format!("{route}{query}"));
        assert_eq!(
            plain,
            cli(&scene, &["card", "show", "C-0001", "--history"]),
            "{query:?}"
        );
        let document: Value = serde_json::from_str(&plain).expect("JSON");
        assert_eq!(document["practices"], Value::Null, "{query:?}");
        assert_eq!(document["cited"], Value::Null, "{query:?}");
    }
    let refused = server.get(&format!("{route}?practices=maybe"));
    assert_eq!(refused.status, 400, "{}", refused.text());
    assert!(refused.text().contains("practices"), "{}", refused.text());
}

/// DP13: the proposal route is the CLI's review document byte for byte,
/// `cited` the resolved citation alone.
#[test]
fn dp13_the_proposal_route_serves_cited() {
    let scene = Scene::new("http-dp13");
    let id = scene.propose_cited();
    let server = serve(&scene);
    let served = body(&server, &format!("/api/projects/{SLUG}/proposals/{id}"));
    assert_eq!(served, cli(&scene, &["review", &id]), "byte for byte");
    let document: Value = serde_json::from_str(&served).expect("JSON");
    assert_eq!(
        document["cited"],
        json!([{"written": NORMALS, "name": NORMALS, "title": "Normal maps", "path": ART}])
    );
}

/// "The library": art.md's row holds `disciplines` right after `tags`.
#[test]
fn the_library_row_holds_its_disciplines() {
    let scene = Scene::new("http-library");
    let server = serve(&scene);
    let served = body(&server, &format!("/api/projects/{SLUG}/library"));
    assert_eq!(served, cli(&scene, &["library"]), "byte for byte");
    let document: Value = serde_json::from_str(&served).expect("JSON");
    let row = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["path"] == ART)
        .unwrap_or_else(|| panic!("no art row: {served}"))
        .clone();
    assert_eq!(row["disciplines"], json!(["art"]));
    let raw_row = served
        .find(&format!("\"path\":\"{ART}\""))
        .expect("art's row");
    let tags = served[raw_row..].find("\"tags\":").expect("tags");
    let disciplines = served[raw_row..]
        .find("\"disciplines\":")
        .expect("disciplines");
    let rev = served[raw_row..].find("\"rev\":").expect("rev");
    assert!(
        tags < disciplines && disciplines < rev,
        "after tags, before rev"
    );
}
