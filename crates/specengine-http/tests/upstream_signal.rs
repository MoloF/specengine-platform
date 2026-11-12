//! docs/features/upstream-signal.md (RM-0105) "Acceptance criteria", the
//! daemon's half: `GET /api/projects/tide-table/cards/:id` and
//! `GET …/cards?marks=true` over the spec's scenario (the shared scene of
//! `specengine-cli`'s `tests/upstream_scene`), a daemon of its own under the
//! scene's `HOME`.
//!
//! - US-12: the card route serves C-0002's `upstream` as the spec's JSON gives it,
//!   its `mark` `null` (the route asks no mark) and its `history`; a card
//!   with no parent `null`. The list with `marks=true` carries each row's
//!   flag, `null` without. An index that cannot be read: the card route
//!   answers 200 for a card with no parent (`upstream: null`) and for a
//!   closed subtask, 503 with the CLI's line for an open subtask; the list
//!   with marks 503, without 200. M: the card route not asking `upstream`;
//!   the index read before the applicability check.
//! - US-7 over the route: the parent's mark alone, then `null` once the
//!   parent is cleared.
//!
//! The key sets (`Card` 36, `CardRow` 23, `CardUpstream`,
//! `CardUpstreamChange`) are `daemon_keys.rs`'s.

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/upstream_scene/mod.rs"]
mod upstream_scene;

use common::{Server, spec};
use serde_json::{Value, json};
use upstream_scene::{Scene, Stop, T_APPLY, data_upstream, marked_upstream};

const BASE: &str = "/api/projects/tide-table/cards";

/// Each row's `upstream`, by ID.
fn flags(document: &Value) -> Vec<(String, Value)> {
    document["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("an ID").to_owned(),
                row["upstream"].clone(),
            )
        })
        .collect()
}

fn serve(scene: &Scene) -> Server {
    Server::serve(&scene.home, &scene.root, &[&scene.root])
}

/// US-12: the card route serves the spec's JSON `upstream` with `mark` `null` and
/// the history; C-0001 (no parent) `null`; the list's rows flag with
/// `marks=true`, `null` without; every read equal to the CLI's `upstream`.
/// M: the card route not asking `upstream`.
#[test]
fn us12_the_card_route_serves_upstream() {
    let scene = Scene::scenario("http-us12", Stop::Data);
    let server = serve(&scene);
    let child = server.get(&format!("{BASE}/C-0002")).status(200).json();
    assert_eq!(child["upstream"], data_upstream(), "{child}");
    assert_eq!(child["mark"], Value::Null, "the route asks no mark");
    assert!(child["history"].is_array(), "{child}");
    let root = scene.root.to_str().expect("UTF-8 root");
    let cli = spec(
        &scene.home,
        &scene.root,
        &[
            "--root", root, "--json", "card", "show", "C-0002", "--marks",
        ],
    );
    cli.code(0);
    assert_eq!(cli.json()["upstream"], child["upstream"], "the CLI's");

    let parent = server.get(&format!("{BASE}/C-0001")).status(200).json();
    assert_eq!(parent["upstream"], Value::Null, "{parent}");
    assert_eq!(parent["mark"], Value::Null);

    let marked = server.get(&format!("{BASE}?marks=true")).status(200).json();
    assert_eq!(
        flags(&marked),
        [
            ("C-0001".to_owned(), json!(false)),
            ("C-0002".to_owned(), json!(true)),
            ("C-0003".to_owned(), json!(true)),
        ]
    );
    for query in ["", "?marks=false"] {
        let plain = server.get(&format!("{BASE}{query}")).status(200).json();
        for (id, flag) in flags(&plain) {
            assert_eq!(flag, Value::Null, "{id} {query:?}");
        }
    }
}

/// US-12: the index unreadable (its `files` table renamed): the card route
/// answers 200 for C-0001, no parent, `upstream: null`, and for a cancelled
/// subtask; 503 for the open subtask C-0002, the CLI's line; the list with
/// marks 503, without 200. M: the index read before the applicability
/// check.
#[test]
fn us12_only_an_open_subtask_pays_the_index() {
    let scene = Scene::scenario("http-us12-503", Stop::Data);
    let root = scene.root.to_str().expect("UTF-8 root").to_owned();
    let cancelled = spec(
        &scene.home,
        &scene.root,
        &[
            "--root",
            &root,
            "card",
            "cancel",
            "C-0003",
            "--reason",
            "Not needed.",
            "--as",
            "Ann",
        ],
    );
    cancelled.code(0);
    let server = serve(&scene);
    server.get(&format!("{BASE}/C-0002")).status(200);
    scene.sql("ALTER TABLE files RENAME TO files_gone;");

    let parent = server.get(&format!("{BASE}/C-0001")).status(200).json();
    assert_eq!(parent["upstream"], Value::Null, "{parent}");
    let closed = server.get(&format!("{BASE}/C-0003")).status(200).json();
    assert_eq!(closed["upstream"], Value::Null, "{closed}");

    let reply = server.get(&format!("{BASE}/C-0002"));
    assert_eq!(reply.status, 503, "{}", reply.text());
    let cli = spec(
        &scene.home,
        &scene.root,
        &[
            "--root", &root, "card", "show", "C-0002", "--marks", "--json",
        ],
    );
    cli.code(2);
    assert_eq!(reply.error_message(), cli.stderr.trim_end_matches('\n'));

    let list = server.get(&format!("{BASE}?marks=true"));
    assert_eq!(list.status, 503, "{}", list.text());
    server.get(BASE).status(200);
}

/// US-7 over the route: the parent's mark alone, then, once the parent is
/// cleared, `null` and a row `false`.
#[test]
fn us7_the_parents_mark_over_the_route() {
    let scene = Scene::scenario("http-us7", Stop::Marked);
    let server = serve(&scene);
    let child = server.get(&format!("{BASE}/C-0002")).status(200).json();
    assert_eq!(child["upstream"], marked_upstream(), "{child}");
    scene.clear("C-0001", T_APPLY);
    let child = server.get(&format!("{BASE}/C-0002")).status(200).json();
    assert_eq!(child["upstream"], Value::Null, "{child}");
    let marked = server.get(&format!("{BASE}?marks=true")).status(200).json();
    assert_eq!(flags(&marked)[1], ("C-0002".to_owned(), json!(false)));
}

/// The `notes` of a document, as strings.
fn notes(document: &Value) -> Vec<String> {
    document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

/// Iteration 2, the route's notes: with another card's row unreadable, C-0003
/// here, the CLI's `card show C-0002 --marks` keeps its `; not read` note
/// as before the slice, while the card route, which asks no mark, names no
/// row it did not read; it keeps the upstream notes alone: the no-anchor
/// note and an undecodable applied row's, in that order.
#[test]
fn us12_the_route_keeps_the_upstream_notes_alone() {
    let scene = Scene::scenario("http-us12-notes", Stop::Data);
    scene.sql("UPDATE cards SET targets = 'not json' WHERE id = 'C-0003';");
    scene.sql(
        "DELETE FROM events WHERE type = 'card.created' AND json_extract(payload, '$.id') = \
         'C-0002';",
    );
    scene.sql("UPDATE proposals SET card_id = 'X-7' WHERE id = 'PR-0040';");
    let root = scene.root.to_str().expect("UTF-8 root");
    let cli = spec(
        &scene.home,
        &scene.root,
        &[
            "--root", root, "--json", "card", "show", "C-0002", "--marks",
        ],
    );
    cli.code(0);
    let cli_notes = notes(&cli.json());
    assert!(
        cli_notes.iter().any(|note| note.ends_with("; not read")),
        "the marked read names the row as before: {cli_notes:?}"
    );
    let server = serve(&scene);
    let routed = server.get(&format!("{BASE}/C-0002")).status(200).json();
    assert_eq!(
        notes(&routed),
        [
            "upstream: C-0002: no card.created event; only its parent's mark is read",
            "upstream: C-0002: proposal PR-0040: the stored `card_id` cannot be read: `X-7` is no \
             card ID; not judged",
        ],
        "{routed}"
    );
    assert_eq!(
        routed["upstream"],
        json!({"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK", "correction": null,
               "changes": [], "more": 0})
    );
}
