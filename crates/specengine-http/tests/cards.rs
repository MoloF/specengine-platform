//! docs/features/board-ui.md, the daemon's half ("Data", "Routes"): the two
//! reads and the six writes of `.../cards` through the real daemon, over git
//! repositories of `fixtures/card-press` (a print shop: `copy` with `moves`
//! and a `by`, `plates` with no `moves` and a `blocked` column that means
//! nothing) and `fixtures/spec-a` (no `[card]`: the default flow).
//!
//! - AC-02: each POST from a same-origin page is a 200, its document, one
//!   `card.*` event with "Documents and events"'s payload, `git status`,
//!   `HEAD` and every ref as they were (the whole-state half, files and
//!   proposals too: `door.rs`, `card_writes_touch_only_the_queue`).
//! - AC-03: each POST with no `Sec-Fetch-Site`, `none`, `cross-site`,
//!   `same-site`, a foreign `Origin` or `Host` is a 403 and no event. M: a
//!   POST without `same_origin`.
//! - AC-04: a stale `revision`, a move outside `moves`, an `as` outside `by`,
//!   accept by the assignee are 409s with the write document, the reason last
//!   of `notes`, byte for byte the CLI's exit-1 document where the CLI can
//!   ask. M: `Outcome::Card` falling to exit 1's 404.
//! - AC-05, the daemon's half: new, assign, move, accept end accepted,
//!   `acceptance.by` the typed name, which survives a restart;
//!   `specengine-mcp` names no `/api/` and holds no HTTP client.
//! - "The door", in order, nothing read on a refusal: the served slug (404),
//!   a same-origin page (403), JSON (415), 1 MiB (413), a body the CLI
//!   decodes (400).
//! - "Answers": 400 usage, 404 `{id, reason}`, 409, 503 the CLI's line.
//! - Past the cap the daemon serves every row while the CLI cuts. M: `in_view`
//!   dropped.
//! - A card's persons: a full name in any script acts, is assigned, reviews
//!   and stands in `by`, compared exactly after trimming; off the grammar,
//!   from the page, a 400 with the CLI's line; `agent` and `local` stay
//!   refused as reviewers.
//! - docs/features/board-handoff.md (RM-0085) BH-12, BH-14 on
//!   `fixtures/studio-lamp`: `POST .../cards {reviewer}` stores the hand-off's
//!   reviewer (`agent` a 409, off the grammar a 400); `POST .../move {note}`
//!   into `sent-back` is a 409 without the note, a 200 with it, `card.moved`
//!   holding it; each byte for byte the CLI's.
//!
//! - docs/features/departments.md (RM-0096) DP-4: `GET .../cards` serves
//!   each flow's `title` right after `name` ("Copy desk", `plates` by its
//!   name), byte-equal to `spec card list --json`, a card or none.
//!
//! The byte-for-byte reads (AC-01) are `parity.rs`; methods, query, fence and
//! keys their own files. Every expected value below is worked out from
//! docs/canon/cards.md, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use common::{
    Reply, Scratch, Server, Stream, beyond_daemon, encode_component, replace, repository_root,
    request_bytes, spec, spec_json,
};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};

const P: &str = "/api/projects/card-press";

/// `same_origin()`'s line ("The fence stays").
const PAGE: &str = "a write needs a same-origin page (not authentication: ADR-0034)";

/// `CARD_BODY_MAX`, 1 MiB ("The door").
const CAP: usize = 1_048_576;

/// A full name in another script, as "Acting as" takes it.
const FULL_NAME: &str =
    "\u{0410}\u{043d}\u{043d}\u{0430} \u{041f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";

/// Another person's, who reviews.
const OTHER_NAME: &str =
    "\u{0411}\u{043e}\u{0440}\u{0438}\u{0441} \u{0418}\u{0432}\u{0430}\u{043d}\u{043e}\u{0432}";

/// A write's or a refusal's document, sorted.
const WRITE_KEYS: [&str; 7] = [
    "allowed", "column", "flow", "id", "notes", "position", "revision",
];

/// The closed line of `id`, `state` `accepted` or `cancelled`.
fn closed(id: &str, state: &str) -> String {
    format!(
        "{id} is {state}: it takes no write; a correction is a new card, `spec card new \
         --corrects {id}`; nothing changed"
    )
}

/// A daemon over a repository of `fixtures/card-press` in a `HOME` of its
/// own. `server` is first: it stops before the scratch goes.
struct Press {
    server: Server,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    scratch: Scratch,
}

impl Press {
    fn new(name: &str) -> Self {
        Self::of(name, "card-press")
    }

    fn of(name: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo(fixture, "press", "main");
        // An identity, as an owner's repository has one: a write through the
        // daemon could commit.
        let git = scratch.git();
        git.run(&root, &["config", "user.name", "Owner"]);
        git.run(&root, &["config", "user.email", "owner@example.invalid"]);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let server = Server::serve(&home, &cwd, &[&root]);
        Self {
            server,
            root,
            home,
            cwd,
            scratch,
        }
    }

    /// `GET <P>/path`.
    fn get(&self, path: &str) -> Reply {
        self.server.get(&format!("{P}/{path}"))
    }

    /// A same-origin page's `POST <P>/path` with the JSON `body`.
    fn post(&self, path: &str, body: &Value) -> Reply {
        self.server.stage(&format!("{P}/{path}"), &body.to_string())
    }

    /// As [`Press::post`], the answer a 200.
    fn done(&self, path: &str, body: &Value) -> Value {
        let reply = self.post(path, body);
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        reply.json()
    }

    /// `show`'s document, as a page reads it.
    fn show(&self, id: &str) -> Value {
        self.get(&format!("cards/{id}")).status(200).json()
    }

    /// The CLI's document of `args`, exit 0 or 1.
    fn cli(&self, args: &[&str]) -> common::Run {
        spec_json(&self.home, &self.cwd, &self.root, args)
    }

    /// The CLI's line of `args`, which exit 2.
    fn cannot(&self, args: &[&str]) -> String {
        let root = self.root.to_str().expect("UTF-8 root");
        let mut all = vec!["--root", root];
        all.extend_from_slice(args);
        all.push("--json");
        let run = spec(&self.home, &self.cwd, &all);
        run.code(2);
        run.stderr.trim_end_matches('\n').to_owned()
    }

    /// Every stored event, `(type, payload)`.
    fn events(&self) -> Vec<(String, Value)> {
        events_of(&self.home, &self.root)
    }

    /// `git status`, `HEAD`, every ref, the number of commits.
    fn git_side(&self) -> (String, String, String, String) {
        let git = self.scratch.git();
        (
            git.run(
                &self.root,
                &[
                    "status",
                    "--porcelain",
                    "--ignored",
                    "--untracked-files=all",
                ],
            ),
            git.run(&self.root, &["rev-parse", "HEAD"]),
            git.run(
                &self.root,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            ),
            git.run(&self.root, &["rev-list", "--all", "--count"]),
        )
    }

    /// What a refusal must leave: the events and the board as a page reads
    /// it.
    fn state(&self) -> (Vec<(String, Value)>, String) {
        let board = self.get("cards?with_cancelled=true").status(200).json();
        (self.events(), board.to_string())
    }

    /// `path` with `body` refused with `reason`: a 409, the write document,
    /// the reason last of `notes`, nothing changed; byte for byte the CLI's
    /// exit-1 document of `args` when given (`accept` has none: a person's
    /// terminal).
    fn refused(&self, path: &str, body: &Value, args: &[&str], reason: &str) {
        let before = self.state();
        let reply = self.post(path, body);
        let context = format!("{path} {body}");
        assert_eq!(reply.status, 409, "{context}: {}", reply.text());
        assert_eq!(
            reply.header("content-type"),
            Some("application/json; charset=utf-8"),
            "{context}"
        );
        let document = reply.json();
        let mut keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
        keys.sort();
        assert_eq!(keys, WRITE_KEYS, "{context}: {document}");
        assert_eq!(
            document["notes"].as_array().and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{context}: the reason, last of `notes`: {document}"
        );
        assert_eq!(self.state(), before, "{context}: nothing changed");
        if !args.is_empty() {
            let run = self.cli(args);
            run.code(1);
            assert_eq!(
                reply.text(),
                run.document(),
                "{context}: the CLI's document"
            );
            assert_eq!(self.state(), before, "{context}: nothing changed");
        }
    }
}

fn events_of(home: &Path, root: &Path) -> Vec<(String, Value)> {
    let env = Env {
        cwd: root.to_path_buf(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(root.to_path_buf()),
        config: None,
    };
    events_after(&env, &globals, Some(0))
        .expect("the events read")
        .events
        .into_iter()
        .map(|event| {
            let payload = serde_json::from_str(&event.payload).expect("a JSON payload");
            (event.event_type, payload)
        })
        .collect()
}

/// `POST path` with `headers`, `Content-Type: application/json` and `body`,
/// the `Host` among `headers`.
fn post_with(server: &Server, path: &str, headers: &[(&str, &str)], body: &str) -> Reply {
    let length = body.len().to_string();
    let mut all = headers.to_vec();
    all.push(("Content-Type", "application/json"));
    all.push(("Content-Length", length.as_str()));
    let mut bytes = request_bytes("POST", path, &all);
    bytes.extend_from_slice(body.as_bytes());
    server.raw(&bytes)
}

/// Every POST of "Routes", under `base`, with a body the CLI decodes.
fn posts(base: &str, id: &str) -> Vec<(String, Value)> {
    vec![
        (
            format!("{base}/cards"),
            json!({"title": "Side story", "discipline": "copy"}),
        ),
        (
            format!("{base}/cards/{id}/move"),
            json!({"to": "edit", "as": "desk"}),
        ),
        (format!("{base}/cards/{id}/edit"), json!({"goal": "Again."})),
        (
            format!("{base}/cards/{id}/assign"),
            json!({"assignee": "cid"}),
        ),
        (
            format!("{base}/cards/{id}/cancel"),
            json!({"reason": "Pulled."}),
        ),
        (
            format!("{base}/cards/{id}/accept"),
            json!({"briefed": true, "as": "ben", "note": "Fine."}),
        ),
    ]
}

// --------------------------------------------------------------- AC-02

/// AC-02: every POST of "Routes" from a same-origin page: 200, its document
/// (keys in order, the CLI's bytes), exactly one event with its payload, git
/// as it was, an identity in the repository so a commit would land.
#[test]
fn ac02_each_post_is_a_200_its_document_and_one_event() {
    let press = Press::new("cards-posts");
    let git = press.git_side();
    assert_eq!(git.0, "", "the repository starts clean");
    let draft = "\"allowed\":[{\"to\":\"edit\",\"by\":[\"desk\"]}],\"notes\":[]}";
    let write = |id: &str, flow: &str, column: &str, position: u64, revision: u64, rest: &str| {
        format!(
            "{{\"id\":\"{id}\",\"flow\":\"{flow}\",\"column\":\"{column}\",\"position\":\
             {position},\"revision\":\"{revision}\",{rest}"
        )
    };
    let none = "\"allowed\":[],\"notes\":[]}";
    let steps: Vec<(&str, Value, String, &str, Value)> = vec![
        (
            "cards",
            json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"],
                "as": "desk"}),
            write("C-0001", "copy", "draft", 1, 1, draft),
            "card.created",
            json!({"id": "C-0001", "flow": "copy", "column": "draft", "position": 1,
                "assignee": null, "reviewer": null, "as": "desk"}),
        ),
        (
            "cards/C-0001/assign",
            json!({"assignee": "ana", "reviewer": "ben", "revision": "1", "as": "desk"}),
            write("C-0001", "copy", "draft", 1, 2, draft),
            "card.assigned",
            json!({"id": "C-0001", "assignee": "ana", "reviewer": "ben", "as": "desk"}),
        ),
        (
            "cards/C-0001/edit",
            json!({"goal": "Fill the front.", "revision": "2", "as": "ana"}),
            write("C-0001", "copy", "draft", 1, 3, draft),
            "card.edited",
            json!({"id": "C-0001", "fields": ["goal"], "as": "ana"}),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "edit", "revision": "3", "as": "desk"}),
            write(
                "C-0001",
                "copy",
                "edit",
                1,
                4,
                "\"allowed\":[{\"to\":\"proof\",\"by\":[]}],\"notes\":[]}",
            ),
            "card.moved",
            json!({"id": "C-0001", "from": "draft", "to": "edit", "position": 1, "as": "desk",
                "note": null}),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "proof", "revision": "4", "as": "ana"}),
            write("C-0001", "copy", "proof", 1, 5, none),
            "card.moved",
            json!({"id": "C-0001", "from": "edit", "to": "proof", "position": 1, "as": "ana",
                "note": null}),
        ),
        (
            "cards/C-0001/accept",
            json!({"briefed": true, "as": "ben", "note": "Read twice.", "revision": "5"}),
            write("C-0001", "copy", "filed", 1, 6, none),
            "card.accepted",
            json!({"id": "C-0001", "from": "proof", "to": "filed", "position": 1, "as": "ben",
                "verdict": "briefed", "discrepancy": null}),
        ),
        (
            "cards",
            json!({"title": "Back page", "discipline": "plates", "as": null}),
            write(
                "C-0002",
                "plates",
                "queued",
                1,
                1,
                "\"allowed\":[{\"to\":\"blocked\",\"by\":[]},{\"to\":\"checked\",\"by\":[]}],\
                 \"notes\":[]}",
            ),
            "card.created",
            json!({"id": "C-0002", "flow": "plates", "column": "queued", "position": 1,
                "assignee": null, "reviewer": null, "as": "local"}),
        ),
        (
            "cards/C-0002/cancel",
            json!({"reason": "Pulled for space.", "revision": "1", "as": "desk"}),
            write("C-0002", "plates", "queued", 0, 2, none),
            "card.cancelled",
            json!({"id": "C-0002", "column": "queued", "as": "desk"}),
        ),
    ];
    for (path, body, answer, kind, payload) in steps {
        let seen = press.events().len();
        let reply = press.post(path, &body);
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        assert_eq!(reply.text(), answer, "{path} {body}");
        assert_eq!(
            reply.header("content-type"),
            Some("application/json; charset=utf-8"),
            "{path}"
        );
        assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
        assert!(reply.cors_headers().is_empty(), "{path}: {reply:?}");
        let events = press.events();
        assert_eq!(events.len(), seen + 1, "{path}: one event: {events:?}");
        assert_eq!(events.last(), Some(&(kind.to_owned(), payload)), "{path}");
        assert_eq!(press.git_side(), git, "{path}: git untouched");
    }

    // What was typed, as the panel reads it.
    let card = press.show("C-0001");
    assert_eq!(card["created_by"], json!("desk"));
    assert_eq!(card["acceptance"]["by"], json!("ben"));
    assert_eq!(card["acceptance"]["note"], json!("Read twice."));
    assert_eq!(card["allowed"], json!([]));
    let history: Vec<&str> = card["history"]
        .as_array()
        .expect("the panel's history: the daemon's show is `--history`")
        .iter()
        .map(|event| event["type"].as_str().expect("a type"))
        .collect();
    assert_eq!(
        history,
        [
            "card.created",
            "card.assigned",
            "card.edited",
            "card.moved",
            "card.moved",
            "card.accepted"
        ]
    );
    let other = press.show("C-0002");
    assert_eq!(
        other["created_by"],
        json!("local"),
        "a null `as` is `local`"
    );
    assert_eq!(other["cancellation"]["by"], json!("desk"));
    assert_eq!(other["cancellation"]["reason"], json!("Pulled for space."));
    assert_eq!(other["position"], json!(0));
}

/// "Routes": every key each body takes reaches the card, by name: a field a
/// handler forgets to hand on is a card the CLI would have stored otherwise.
#[test]
fn every_key_of_a_body_reaches_the_card() {
    let press = Press::new("cards-keys");
    press.done("cards", &json!({"title": "Parent", "discipline": "plates"}));
    press.done(
        "cards",
        &json!({"title": "Wrong one", "discipline": "plates"}),
    );
    let made = press.done(
        "cards",
        &json!({"title": "Lead story", "goal": "Fill the front.", "discipline": "copy",
            "target": ["SHT-FRONT", "docs/spec/sht-back.md"],
            "criterion": ["Fits the page.", "Names the town."], "assignee": "ana",
            "estimate": "2-3", "due": "2026-03-01", "parent": "C-0001", "corrects": "C-0002",
            "as": "desk"}),
    );
    assert_eq!(made["id"], json!("C-0003"), "{made}");
    let card = press.show("C-0003");
    for (key, want) in [
        ("title", json!("Lead story")),
        ("goal", json!("Fill the front.")),
        ("discipline", json!("copy")),
        ("targets", json!(["SHT-FRONT", "SHT-BACK"])),
        ("criteria", json!(["Fits the page.", "Names the town."])),
        ("assignee", json!("ana")),
        ("reviewer", json!(null)),
        ("estimate", json!("2-3")),
        ("due", json!("2026-03-01")),
        ("parent", json!("C-0001")),
        ("corrects", json!("C-0002")),
        ("created_by", json!("desk")),
    ] {
        assert_eq!(card[key], want, "{key}: {card}");
    }

    let edited = press.done(
        "cards/C-0003/edit",
        &json!({"title": "Lead", "goal": "-", "target": ["SHT-INNER"], "criterion": ["One."],
            "due": "2026-03-02", "parent": "-", "corrects": "-",
            "evidence": ["note=first pass", "url=https://example.invalid/proof"],
            "revision": "1", "as": "ana"}),
    );
    assert_eq!(edited["revision"], json!("2"), "{edited}");
    // docs/features/card-trace.md "Re-estimating": beside another field, on a
    // card holding an estimate, a 400; with its reason, alone, a 200.
    let reply = press.post(
        "cards/C-0003/edit",
        &json!({"estimate": "5", "goal": "Again.", "as": "ana"}),
    );
    assert_eq!(reply.status, 400, "{}", reply.text());
    let edited = press.done(
        "cards/C-0003/edit",
        &json!({"estimate": "5", "reason": "It grew.", "revision": "2", "as": "ana"}),
    );
    assert_eq!(edited["revision"], json!("3"), "{edited}");
    let card = press.show("C-0003");
    for (key, want) in [
        ("title", json!("Lead")),
        ("goal", json!(null)),
        ("targets", json!(["SHT-INNER"])),
        ("criteria", json!(["One."])),
        ("estimate", json!("5")),
        ("due", json!("2026-03-02")),
        ("parent", json!(null)),
        ("corrects", json!(null)),
    ] {
        assert_eq!(card[key], want, "{key}: {card}");
    }
    let evidence: Vec<(&str, &str, &str)> = card["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .map(|entry| {
            (
                entry["kind"].as_str().expect("a kind"),
                entry["value"].as_str().expect("a value"),
                entry["by"].as_str().expect("a name"),
            )
        })
        .collect();
    assert_eq!(
        evidence,
        [
            ("note", "first pass", "ana"),
            ("url", "https://example.invalid/proof", "ana")
        ]
    );

    press.done(
        "cards/C-0003/assign",
        &json!({"assignee": "-", "reviewer": "ben"}),
    );
    let card = press.show("C-0003");
    assert_eq!(card["assignee"], json!(null), "`-` clears: {card}");
    assert_eq!(card["reviewer"], json!("ben"), "{card}");

    // `before`, the page's Up and Down and a drop between two cards: a
    // reorder is no rule, no `by`; a neighbour keeps its revision.
    for title in ["Second", "Third"] {
        press.done("cards", &json!({"title": title, "discipline": "copy"}));
    }
    let first = press.show("C-0003")["revision"].clone();
    let moved = press.done("cards/C-0005/move", &json!({"before": "C-0003"}));
    assert_eq!(moved["column"], json!("draft"), "{moved}");
    assert_eq!(moved["position"], json!(1), "{moved}");
    let order: Vec<(String, u64)> = press.get("cards?flow=copy").status(200).json()["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("an ID").to_owned(),
                row["position"].as_u64().expect("a position"),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            ("C-0005".to_owned(), 1),
            ("C-0003".to_owned(), 2),
            ("C-0004".to_owned(), 3)
        ]
    );
    assert_eq!(press.show("C-0003")["revision"], first, "a neighbour");

    // `to` and `before` together; `as` reaches the `by`.
    press.done("cards/C-0003/move", &json!({"to": "edit", "as": "desk"}));
    let moved = press.done(
        "cards/C-0004/move",
        &json!({"to": "edit", "before": "C-0003", "as": "desk"}),
    );
    assert_eq!(moved["position"], json!(1), "{moved}");
    assert_eq!(press.show("C-0003")["position"], json!(2));

    // Cancelling and accepting: the rest of their bodies.
    press.done(
        "cards/C-0005/cancel",
        &json!({"reason": "Pulled.", "as": "ana"}),
    );
    let card = press.show("C-0005");
    assert_eq!(card["cancellation"]["reason"], json!("Pulled."), "{card}");
    assert_eq!(card["cancellation"]["by"], json!("ana"), "{card}");
}

// --------------------------------------------------------------- AC-03

/// AC-03: each POST with no `Sec-Fetch-Site`, `none`, a foreign `Origin` or
/// `Host`, `cross-site`, `same-site`: a 403, no `Allow`, no CORS header, no
/// event, the card as it was. A `Sec-Fetch-Site: same-origin` beside a
/// foreign `Origin` or `Host` is the fence's. M: a POST without `same_origin`.
#[test]
fn ac03_each_post_from_no_same_origin_page_is_a_403_and_no_event() {
    let press = Press::new("cards-fence");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "assignee": "ana"}),
    );
    let before = press.state();
    let port = press.server.port;
    let own = format!("127.0.0.1:{port}");
    let own_origin = format!("http://{own}");
    let evil_host = format!("evil.example:{port}");
    let same = ("Sec-Fetch-Site", "same-origin");
    let site_line = "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`";
    let origin_line = format!(
        "refused: the Origin header, when sent, must be http://{own} or http://localhost:{port}"
    );
    let host_line = format!(
        "refused: the Host header must be {own} or localhost:{port}: this server serves its \
         own origin only"
    );
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (vec![("Host", own.as_str())], PAGE),
        (
            vec![("Host", own.as_str()), ("Sec-Fetch-Site", "none")],
            PAGE,
        ),
        (
            vec![("Host", own.as_str()), ("Origin", own_origin.as_str())],
            PAGE,
        ),
        (
            vec![("Host", own.as_str()), ("Sec-Fetch-Site", "cross-site")],
            site_line,
        ),
        (
            vec![("Host", own.as_str()), ("Sec-Fetch-Site", "same-site")],
            site_line,
        ),
        (
            vec![
                ("Host", own.as_str()),
                same,
                ("Origin", "http://evil.example"),
            ],
            origin_line.as_str(),
        ),
        (vec![("Host", evil_host.as_str()), same], host_line.as_str()),
    ];
    for (path, body) in posts(P, "C-0001") {
        for (headers, line) in &cases {
            let reply = post_with(&press.server, &path, headers, &body.to_string());
            let context = format!("POST {path} {headers:?}");
            assert_eq!(reply.status, 403, "{context}: {}", reply.text());
            assert_eq!(reply.error_message(), *line, "{context}");
            assert_eq!(reply.header("allow"), None, "{context}");
            assert!(reply.cors_headers().is_empty(), "{context}: {reply:?}");
            assert_eq!(reply.header("cache-control"), Some("no-store"), "{context}");
        }
    }
    assert_eq!(press.state(), before, "no event, the card as it was");
    // The control: the same bodies from the page pass this door.
    let reply = press.post("cards/C-0001/edit", &json!({"goal": "Again."}));
    assert_eq!(reply.status, 200, "{}", reply.text());
}

// --------------------------------------------------------------- AC-04

/// AC-04 in `draft`: a move outside `moves` (the column `accept` reaches and
/// a column of no flow among them), an `as` outside `by` (a null one is
/// `local`), a stale `revision` on each of the four writes: a 409, the write
/// document, byte for byte the CLI's exit-1 document. M: `Outcome::Card`
/// falling to exit 1's 404.
#[test]
fn ac04_a_refused_write_is_a_409_with_the_clis_document() {
    let press = Press::new("cards-refused");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "assignee": "ana"}),
    );
    press.done("cards/C-0001/assign", &json!({"reviewer": "ben"}));
    let at = press.show("C-0001")["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let stale = format!(
        "`C-0001` changed since this run read it: it is in draft since {at}; nothing changed"
    );
    let cases: Vec<(&str, Value, Vec<&str>, String)> =
        vec![
        (
            "cards/C-0001/move",
            json!({"to": "proof", "as": "desk"}),
            vec!["card", "move", "C-0001", "--to", "proof", "--as", "desk"],
            "C-0001 is in draft: no move to `proof`; allowed: `edit`; nothing changed".to_owned(),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "filed", "as": "desk"}),
            vec!["card", "move", "C-0001", "--to", "filed", "--as", "desk"],
            "C-0001 is in draft: no move to `filed`; allowed: `edit`; nothing changed".to_owned(),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "queued", "as": "desk"}),
            vec!["card", "move", "C-0001", "--to", "queued", "--as", "desk"],
            "C-0001 is in draft: no move to `queued`; allowed: `edit`; nothing changed"
                .to_owned(),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "edit", "as": "ana"}),
            vec!["card", "move", "C-0001", "--to", "edit", "--as", "ana"],
            "C-0001 is in draft: the move to `edit` is `desk`'s, not `ana`'s; nothing changed"
                .to_owned(),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "edit", "as": null}),
            vec!["card", "move", "C-0001", "--to", "edit"],
            "C-0001 is in draft: the move to `edit` is `desk`'s, not `local`'s; nothing changed"
                .to_owned(),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "edit", "revision": "1", "as": "desk"}),
            vec![
                "card",
                "move",
                "C-0001",
                "--to",
                "edit",
                "--revision",
                "1",
                "--as",
                "desk",
            ],
            stale.clone(),
        ),
        (
            "cards/C-0001/edit",
            json!({"goal": "Late.", "revision": "1"}),
            vec!["card", "edit", "C-0001", "--goal", "Late.", "--revision", "1"],
            stale.clone(),
        ),
        (
            "cards/C-0001/assign",
            json!({"assignee": "cid", "revision": "1"}),
            vec!["card", "assign", "C-0001", "--assignee", "cid", "--revision", "1"],
            stale.clone(),
        ),
        (
            "cards/C-0001/cancel",
            json!({"reason": "Late.", "revision": "1"}),
            vec!["card", "cancel", "C-0001", "--reason", "Late.", "--revision", "1"],
            stale.clone(),
        ),
    ];
    for (path, body, args, reason) in &cases {
        press.refused(path, body, args, reason);
    }
    // The document names where the card stands now: a page redraws from it.
    let reply = press.post(
        "cards/C-0001/move",
        &json!({"to": "edit", "revision": "1", "as": "desk"}),
    );
    assert_eq!(
        reply.text(),
        format!(
            "{{\"id\":\"C-0001\",\"flow\":\"copy\",\"column\":\"draft\",\"position\":1,\
             \"revision\":\"2\",\"allowed\":[{{\"to\":\"edit\",\"by\":[\"desk\"]}}],\"notes\":\
             [{}]}}",
            json!(stale)
        )
    );

    // A card's fields: the CLI's refusals, as 409s. No card was made, so
    // every key but `allowed` and `notes` is null.
    let long = "t".repeat(257);
    for (body, args) in [
        (
            json!({"title": long}),
            vec!["card", "new", "--title", long.as_str()],
        ),
        (
            json!({"title": "   "}),
            vec!["card", "new", "--title", "   "],
        ),
        (
            json!({"title": "Late", "discipline": "copy", "due": "2026-02-30"}),
            vec![
                "card",
                "new",
                "--title",
                "Late",
                "--discipline",
                "copy",
                "--due",
                "2026-02-30",
            ],
        ),
        (
            json!({"title": "Late", "discipline": "copy", "estimate": "soon"}),
            vec![
                "card",
                "new",
                "--title",
                "Late",
                "--discipline",
                "copy",
                "--estimate",
                "soon",
            ],
        ),
        (
            json!({"title": "Late", "discipline": "copy", "target": ["NOPE-1"]}),
            vec![
                "card",
                "new",
                "--title",
                "Late",
                "--discipline",
                "copy",
                "--target",
                "NOPE-1",
            ],
        ),
        (
            json!({"title": "Late", "discipline": "copy", "parent": "C-0099"}),
            vec![
                "card",
                "new",
                "--title",
                "Late",
                "--discipline",
                "copy",
                "--parent",
                "C-0099",
            ],
        ),
        // Both flows list their disciplines, so a card naming none has none.
        (
            json!({"title": "Late"}),
            vec!["card", "new", "--title", "Late"],
        ),
    ] {
        let reply = press.post("cards", &body);
        assert_eq!(reply.status, 409, "{body}: {}", reply.text());
        let run = press.cli(&args);
        run.code(1);
        assert_eq!(reply.text(), run.document(), "{body}: the CLI's document");
    }
    // No flow lists the discipline; `agent` and `local` are no one's name, so
    // no reviewer ("Who acts").
    let reply = press.post("cards", &json!({"title": "Late", "discipline": "ink"}));
    assert_eq!(reply.status, 409, "{}", reply.text());
    let run = press.cli(&["card", "new", "--title", "Late", "--discipline", "ink"]);
    run.code(1);
    assert_eq!(reply.text(), run.document());
    for name in ["agent", "local"] {
        let reply = press.post("cards/C-0001/assign", &json!({"reviewer": name}));
        assert_eq!(reply.status, 409, "{name}: {}", reply.text());
        let run = press.cli(&["card", "assign", "C-0001", "--reviewer", name]);
        run.code(1);
        assert_eq!(reply.text(), run.document(), "{name}");
    }
    // An evidence the rules refuse: never fetched, a 409.
    let body = json!({"evidence": ["url=file:///etc/hosts"]});
    let reply = press.post("cards/C-0001/edit", &body);
    assert_eq!(reply.status, 409, "{}", reply.text());
    let run = press.cli(&[
        "card",
        "edit",
        "C-0001",
        "--evidence",
        "url=file:///etc/hosts",
    ]);
    run.code(1);
    assert_eq!(reply.text(), run.document());
}

/// AC-04 at `proof`: accept by the assignee, by someone else, by a name no
/// person holds, stale, outside `accept.from`, with no reviewer; then a
/// closed card refuses every write, accept too. The rules are the CLI's,
/// the page's dialog changing only who says yes.
#[test]
fn ac04_accept_keeps_the_clis_rules_and_a_closed_card_refuses_every_write() {
    let press = Press::new("cards-accept");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "assignee": "ana"}),
    );
    press.done("cards/C-0001/assign", &json!({"reviewer": "ben"}));
    press.done("cards/C-0001/move", &json!({"to": "edit", "as": "desk"}));
    press.done("cards/C-0001/move", &json!({"to": "proof"}));
    press.done("cards", &json!({"title": "Second", "discipline": "copy"}));
    press.done("cards", &json!({"title": "Third", "discipline": "copy"}));
    press.done("cards/C-0003/move", &json!({"to": "edit", "as": "desk"}));
    press.done("cards/C-0003/move", &json!({"to": "proof"}));
    let at = press.show("C-0001")["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let accept = "cards/C-0001/accept";
    for (path, body, reason) in [
        (
            accept,
            json!({"briefed": true, "as": "ana"}),
            "C-0001: `ana` is its assignee, who never accepts a result; nothing changed".to_owned(),
        ),
        (
            accept,
            json!({"briefed": true, "as": "cid", "note": "Looks fine."}),
            "C-0001: its reviewer is `ben`, and `--as` says `cid`; nothing changed".to_owned(),
        ),
        (
            accept,
            json!({"briefed": true, "as": null}),
            "C-0001: `local` can be no reviewer: a person accepts a result; nothing changed"
                .to_owned(),
        ),
        (
            accept,
            json!({"briefed": true, "as": "agent"}),
            "C-0001: `agent` can be no reviewer: a person accepts a result; nothing changed"
                .to_owned(),
        ),
        (
            accept,
            json!({"briefed": true, "as": "ben", "revision": "1"}),
            format!(
                "`C-0001` changed since this run read it: it is in proof since {at}; nothing \
                 changed"
            ),
        ),
        (
            "cards/C-0002/accept",
            json!({"briefed": true, "as": "ben"}),
            "C-0002 is in draft: `accept` needs proof; nothing changed".to_owned(),
        ),
        (
            "cards/C-0003/accept",
            json!({"briefed": true, "as": "ben"}),
            "C-0003 has no reviewer: `spec card assign C-0003 --reviewer N` names one; nothing \
             changed"
                .to_owned(),
        ),
    ] {
        press.refused(path, &body, &[], &reason);
    }
    // A note over its cap, well under the door's.
    let before = press.state();
    let reply = press.post(
        accept,
        &json!({"briefed": true, "as": "ben", "note": "n".repeat(70_000)}),
    );
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(press.state(), before, "nothing changed");

    // The reviewer's yes; then the closed card refuses all five, and a
    // cancelled one alike.
    let accepted = press.done(
        accept,
        &json!({"briefed": true, "as": "ben", "note": "Read twice."}),
    );
    assert_eq!(accepted["column"], json!("filed"), "{accepted}");
    let line = closed("C-0001", "accepted");
    for (path, body, args) in [
        (
            "cards/C-0001/move",
            json!({"to": "proof"}),
            vec!["card", "move", "C-0001", "--to", "proof"],
        ),
        (
            "cards/C-0001/edit",
            json!({"goal": "Again."}),
            vec!["card", "edit", "C-0001", "--goal", "Again."],
        ),
        (
            "cards/C-0001/assign",
            json!({"reviewer": "cid"}),
            vec!["card", "assign", "C-0001", "--reviewer", "cid"],
        ),
        (
            "cards/C-0001/cancel",
            json!({"reason": "Late."}),
            vec!["card", "cancel", "C-0001", "--reason", "Late."],
        ),
        (accept, json!({"briefed": true, "as": "ben"}), Vec::new()),
    ] {
        press.refused(path, &body, &args, &line);
    }
    press.done("cards/C-0002/cancel", &json!({"reason": "Pulled."}));
    press.refused(
        "cards/C-0002/move",
        &json!({"to": "edit", "as": "desk"}),
        &["card", "move", "C-0002", "--to", "edit", "--as", "desk"],
        &closed("C-0002", "cancelled"),
    );
    press.refused(
        "cards/C-0002/accept",
        &json!({"briefed": true, "as": "ben"}),
        &[],
        &closed("C-0002", "cancelled"),
    );
}

/// "Rules and edge cases": one revision, eight writers at once: the
/// project's turn lets one through, the rest lose the compare-and-set, each
/// a 409 and no event.
#[test]
fn one_revision_and_eight_writers_one_wins() {
    let press = Press::new("cards-race");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let shared = &press;
    let statuses: Vec<u16> = thread::scope(|scope| {
        let writers: Vec<_> = (0..8)
            .map(|writer| {
                scope.spawn(move || {
                    let body = json!({"goal": format!("Writer {writer}."), "revision": "1"});
                    shared.post("cards/C-0001/edit", &body).status
                })
            })
            .collect();
        writers
            .into_iter()
            .map(|writer| writer.join().expect("a writer"))
            .collect()
    });
    let mut sorted = statuses.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        [200, 409, 409, 409, 409, 409, 409, 409],
        "{statuses:?}"
    );
    let edits = press
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "card.edited")
        .count();
    assert_eq!(edits, 1, "one change, one event");
    assert_eq!(press.show("C-0001")["revision"], json!("2"));
}

// ---------------------------------------------------------- the answers

/// "Answers": no such card, on the two forms a reference has: a 404, exit
/// 1's `{id, reason}`, the CLI's document.
#[test]
fn a_card_that_is_not_there_is_a_404_with_id_and_reason() {
    let press = Press::new("cards-unknown");
    let writes: [(&str, Value, Vec<&str>); 4] = [
        ("move", json!({"to": "edit"}), vec!["--to", "edit"]),
        ("edit", json!({"goal": "Late."}), vec!["--goal", "Late."]),
        (
            "assign",
            json!({"assignee": "ana"}),
            vec!["--assignee", "ana"],
        ),
        (
            "cancel",
            json!({"reason": "Late."}),
            vec!["--reason", "Late."],
        ),
    ];
    for id in ["C-0099", "foo"] {
        let path = format!("cards/{id}");
        let reply = press.get(&path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        let run = press.cli(&["card", "show", id, "--history"]);
        run.code(1);
        assert_eq!(reply.text(), run.document(), "{path}");
        for (verb, body, flags) in &writes {
            let reply = press.post(&format!("{path}/{verb}"), body);
            assert_eq!(reply.status, 404, "{path}/{verb}: {}", reply.text());
            let mut args = vec!["card", *verb, id];
            args.extend_from_slice(flags);
            let run = press.cli(&args);
            run.code(1);
            assert_eq!(reply.text(), run.document(), "{path}/{verb}");
        }
        let reply = press.post(
            &format!("{path}/accept"),
            &json!({"briefed": true, "as": "ben"}),
        );
        assert_eq!(reply.status, 404, "{path}/accept: {}", reply.text());
    }
    assert_eq!(
        press
            .post(
                "cards/C-0099/accept",
                &json!({"briefed": true, "as": "ben"})
            )
            .text(),
        "{\"id\":\"C-0099\",\"reason\":\"no card C-0099\"}",
        "exactly `{{id, reason}}`"
    );
    assert_eq!(
        press.get("cards/foo").json()["reason"],
        json!(
            "no card `foo`: a card ID is `C-` and 4 or more digits, as `spec card list` lists it"
        )
    );
    // A path the CLI's argv could not hold, a number no ID reaches: no panic,
    // an answer naming what was asked.
    let reply = press.get("cards/C-99999999999999999999999");
    assert_eq!(reply.status, 404, "{}", reply.text());
    let run = press.cli(&["card", "show", "C-99999999999999999999999", "--history"]);
    run.code(1);
    assert_eq!(reply.text(), run.document());
    let reply = press.get("cards/C-%000001");
    assert_eq!(reply.status, 404, "a NUL in an ID: {}", reply.text());
    assert_eq!(
        reply.json().as_object().map(serde_json::Map::len),
        Some(2),
        "{}",
        reply.text()
    );
    assert!(press.events().is_empty(), "nothing stored");
}

/// A body reaches a card with what no `argv` carries: a NUL, a line break,
/// a quote, a script tag, another script. Each is stored and served as
/// written ("Panel": "text as stored"), and a list and a show stay JSON. A
/// title's line break is no second row.
#[test]
fn a_body_stores_what_it_says_and_the_reads_stay_json() {
    let press = Press::new("cards-text");
    let title =
        "Lead\u{0}story\n<script>alert(\"x\")</script> \u{0424}\u{043e}\u{043b}\u{0438}\u{043e}";
    let reply = press.post("cards", &json!({"title": title, "discipline": "copy"}));
    assert_eq!(reply.status, 200, "{}", reply.text());
    let card = press.show("C-0001");
    assert_eq!(card["title"], json!(title), "{card}");
    let board = press.get("cards").status(200).json();
    assert_eq!(board["cards"][0]["title"], json!(title), "{board}");
    assert_eq!(
        board["cards"].as_array().map(Vec::len),
        Some(1),
        "one row: {board}"
    );
    // The CLI reads the same row: its document byte for byte.
    let run = press.cli(&["card", "show", "C-0001", "--history"]);
    run.code(0);
    assert_eq!(press.get("cards/C-0001").text(), run.document());
}

/// "Answers": a move, an edit, an assign asking nothing are the CLI's usage
/// defect (exit 2 there, a `CliError`): a 400, the error body, its line.
#[test]
fn nothing_asked_is_a_400_with_the_clis_line() {
    let press = Press::new("cards-usage");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let before = press.state();
    for (verb, body) in [
        ("move", json!({})),
        ("move", json!({"revision": "1", "as": "desk"})),
        ("edit", json!({})),
        ("edit", json!({"revision": "1", "as": "desk"})),
        ("assign", json!({})),
        ("assign", json!({"revision": "1", "as": "desk"})),
    ] {
        let reply = press.post(&format!("cards/C-0001/{verb}"), &body);
        assert_eq!(reply.status, 400, "{verb} {body}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            press.cannot(&["card", verb, "C-0001"]),
            "{verb} {body}: the CLI's line"
        );
    }
    assert_eq!(press.state(), before, "nothing stored");
}

/// "Answers": what cannot run is a 503, the error body, the CLI's line
/// verbatim: a look-alike ID (a path's, a body's), a `revision` off its
/// grammar, which a machine writes and no person types. A person off the
/// grammar is a 400:
/// [`a_person_off_the_grammar_from_the_page_is_a_400_with_the_clis_line`].
#[test]
fn what_cannot_run_is_a_503_with_the_clis_line() {
    let press = Press::new("cards-cannot");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let before = press.state();
    let latin_look = "\u{0421}-0001";
    let encoded = encode_component(latin_look);
    let reply = press.get(&format!("cards/{encoded}"));
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        press.cannot(&["card", "show", latin_look, "--history"])
    );
    let cases: Vec<(String, Value, Vec<&str>)> = vec![
        (
            format!("cards/{encoded}/move"),
            json!({"to": "edit"}),
            vec!["card", "move", latin_look, "--to", "edit"],
        ),
        (
            "cards/C-0001/move".to_owned(),
            json!({"before": latin_look}),
            vec!["card", "move", "C-0001", "--before", latin_look],
        ),
        (
            "cards".to_owned(),
            json!({"title": "Late", "parent": latin_look}),
            vec!["card", "new", "--title", "Late", "--parent", latin_look],
        ),
        (
            "cards/C-0001/edit".to_owned(),
            json!({"goal": "Late.", "revision": "x"}),
            vec![
                "card",
                "edit",
                "C-0001",
                "--goal",
                "Late.",
                "--revision",
                "x",
            ],
        ),
        (
            "cards/C-0001/assign".to_owned(),
            json!({"assignee": "ana", "revision": "01"}),
            vec![
                "card",
                "assign",
                "C-0001",
                "--assignee",
                "ana",
                "--revision",
                "01",
            ],
        ),
        (
            "cards/C-0001/cancel".to_owned(),
            json!({"reason": "Late.", "revision": "0"}),
            vec![
                "card",
                "cancel",
                "C-0001",
                "--reason",
                "Late.",
                "--revision",
                "0",
            ],
        ),
    ];
    for (path, body, args) in &cases {
        let reply = press.post(path, body);
        assert_eq!(reply.status, 503, "{path} {body}: {}", reply.text());
        assert_eq!(reply.error_message(), press.cannot(args), "{path} {body}");
    }
    assert_eq!(press.state(), before, "nothing stored");
}

/// docs/features/trace-views.md AC-09, the route: `marks=true` is `spec card
/// list --marks --json`'s bytes: a clean card open, a suspect one on an
/// uncommitted edit named by its target, a card on a section taken out suspect
/// too, where `GET trace` has no node to stand it on. With no flag the
/// keys are `null`. `marks=x` is a 400 naming it. M: a tile drawn without its
/// mark.
#[test]
fn trace_views_marks_are_the_clis_document() {
    let press = Press::new("cards-marks");
    for (title, target) in [
        ("Notices", "SHT-INNER"),
        ("Lead story", "SHT-FRONT"),
        ("Masthead", "SHT-FRONT-HEAD"),
    ] {
        press.done(
            "cards",
            &json!({"title": title, "discipline": "copy", "target": [target]}),
        );
    }
    replace(
        &press.root,
        "docs/spec/sht-front.md",
        "## The masthead {#SHT-FRONT-HEAD}\n\nThe masthead stays the same from week to week.\n",
        "",
    );
    replace(
        &press.root,
        "docs/spec/sht-front.md",
        "the masthead and the lead story",
        "the masthead, the lead story and the weather",
    );
    let run = press.cli(&["card", "list", "--marks"]);
    run.code(0);
    let reply = press.get("cards?marks=true");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.text(), run.document(), "the CLI's bytes");
    let rows = reply.json()["cards"].clone();
    let marks: Vec<(Value, Value, Value)> = rows
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| (row["title"].clone(), row["mark"].clone(), row["on"].clone()))
        .collect();
    assert_eq!(
        marks,
        [
            (json!("Notices"), json!("open"), Value::Null),
            (json!("Lead story"), json!("suspect"), json!("SHT-FRONT")),
            (json!("Masthead"), json!("suspect"), json!("SHT-FRONT-HEAD")),
        ],
        "{}",
        reply.text()
    );
    for row in press.get("cards").status(200).json()["cards"]
        .as_array()
        .expect("cards")
    {
        for key in ["mark", "on", "discrepancy", "correction"] {
            assert_eq!(row[key], Value::Null, "{key}: {row}");
        }
    }
    assert_eq!(
        press.get("cards?marks=false").status(200).text(),
        press.get("cards").status(200).text(),
        "`false` is the plain list"
    );
    for query in [
        "cards?marks=yes",
        "cards?marks=",
        "cards?marks=true&marks=true",
    ] {
        let reply = press.get(query);
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        assert!(
            reply.error_message().contains("marks"),
            "{query}: {}",
            reply.error_message()
        );
    }
}

/// docs/features/trace-views.md "The board tile": no marks, no tiles: an index
/// that cannot be read is `GET cards?marks=true`'s 503, the CLI's line, so
/// `ErrorPanel` stands where a clean board would; the plain list, which reads
/// no index, stands. M: a tile drawn without its mark.
#[test]
fn trace_views_an_index_that_cannot_be_read_is_a_503() {
    let press = Press::new("cards-marks-503");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    let reply = press.get("cards?marks=true");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["cards"][0]["mark"], "open");
    let done = std::process::Command::new("/usr/bin/sqlite3")
        .arg(common::data_dir(&press.home).join("card-press.db"))
        .arg("ALTER TABLE files RENAME TO files_gone;")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(done.status.success(), "{done:?}");
    let reply = press.get("cards?marks=true");
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        press.cannot(&["card", "list", "--marks"])
    );
    assert_eq!(
        press.get("cards").status(200).json()["cards"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

/// docs/features/trace-truth.md AC-01 and AC-08, the board and the panel: text
/// appended under a section marks the document's card on `GET cards?marks=true`
/// (`suspect SHT-FRONT`); `GET cards/:id` holds the four keys `null`, plain
/// `spec card show`'s bytes, a read of no index: the panel keeps its marks from
/// the list; `spec card show --marks` equals the row. M: keys not joined in
/// `run_show`.
#[test]
fn trace_truth_the_board_marks_and_the_panel_holds_null() {
    let press = Press::new("cards-truth");
    let id = press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    )["id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    replace(
        &press.root,
        "docs/spec/sht-front.md",
        "The masthead stays the same from week to week.\n",
        "The masthead stays the same from week to week.\nThe date changes.\n",
    );

    let list = press.get("cards?marks=true").status(200).json();
    let row = &list["cards"][0];
    assert_eq!(row["mark"], "suspect", "{row}");
    assert_eq!(row["on"], "SHT-FRONT", "{row}");

    let panel = press.show(&id);
    for key in ["mark", "on", "discrepancy", "correction"] {
        assert!(panel.get(key).is_some(), "{key}: {panel}");
        assert_eq!(panel[key], Value::Null, "{key}: {panel}");
    }
    assert!(panel["snapshot"][0]["within_hash"].is_string(), "{panel}");
    // The panel's route asks `--history` too.
    let run = press.cli(&["card", "show", &id, "--history"]);
    run.code(0);
    assert_eq!(
        press.get(&format!("cards/{id}")).status(200).text(),
        run.document(),
        "plain `show`'s bytes"
    );

    let marked = press.cli(&["card", "show", &id, "--marks"]);
    marked.code(0);
    let marked = marked.json();
    for key in ["mark", "on", "discrepancy", "correction"] {
        assert_eq!(marked[key], row[key], "{key}: {marked}");
    }
}

/// "States": a bad `[card]` is that 503 on every card route, the CLI's
/// `<config>:<line>:` line, and fails no other read (docs/canon/cards.md
/// "Config"). The config is read again per call: no restart.
#[test]
fn a_bad_card_table_is_a_503_on_the_card_routes_alone() {
    let press = Press::new("cards-bad-table");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let before = press.events();
    replace(
        &press.root,
        "specengine.toml",
        "columns = [\"queued\", \"blocked\", \"checked\", \"struck\"]",
        "columns = [\"queued\"]",
    );
    let line = press.cannot(&["card", "list"]);
    assert!(
        line.contains("specengine.toml:"),
        "the config and its line: {line}"
    );
    for query in ["cards", "cards?with_cancelled=true", "cards/C-0001"] {
        let reply = press.get(query);
        assert_eq!(reply.status, 503, "{query}: {}", reply.text());
        assert_eq!(reply.error_message(), line, "{query}");
    }
    for (path, body) in posts(P, "C-0001") {
        let reply = press.server.stage(&path, &body.to_string());
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(reply.error_message(), line, "{path}");
    }
    for read in ["tree", "inbox", "tasks", "round"] {
        let reply = press.get(read);
        assert_eq!(reply.status, 200, "{read}: {}", reply.text());
    }
    assert_eq!(press.events(), before, "nothing stored");
}

/// docs/features/departments.md (RM-0096) DP-4: `GET .../cards` on
/// card-press is byte-equal to `spec card list --json`, empty and with a
/// card; each flow's `title` follows its `name`, `copy` titled "Copy desk",
/// untitled `plates` by its name. M: the mirror without `title` (MCP
/// `mcp_cards.rs`); `title` serialized after `disciplines`.
#[test]
fn departments_dp4_the_board_serves_each_flow_s_title_as_the_cli() {
    let press = Press::new("cards-titles");
    for step in ["empty", "a card"] {
        if step == "a card" {
            press.done(
                "cards",
                &json!({"title": "Lead story", "discipline": "copy"}),
            );
        }
        let reply = press.get("cards");
        reply.status(200);
        let run = press.cli(&["card", "list"]);
        run.code(0);
        assert_eq!(reply.text(), run.document(), "{step}: the CLI's bytes");
        let flows = reply.json()["flows"].clone();
        assert_eq!(flows[0]["name"], "copy", "{step}");
        assert_eq!(flows[0]["title"], "Copy desk", "{step}");
        assert_eq!(flows[1]["title"], "plates", "{step}: untitled, its name");
        assert!(
            reply
                .text()
                .contains("{\"name\":\"copy\",\"title\":\"Copy desk\",\"disciplines\":"),
            "{step}: `title` right after `name`: {}",
            reply.text()
        );
    }
}

// ------------------------------------------------------------- the door

/// "The door", in its order, each route, on a fresh `HOME`: nothing is read
/// on a refusal, so the data directory stays as the daemon's start left it.
#[test]
fn the_door_refuses_in_order_and_reads_nothing() {
    let scratch = Scratch::new("cards-door");
    let root = scratch.repo("card-press", "press", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&root]);
    let same = ("Sec-Fetch-Site", "same-origin");
    let json_type = ("Content-Type", "application/json");
    let plain = ("Content-Type", "text/plain");
    let body_line = "the body is JSON: send it with Content-Type: application/json";
    let over = (CAP + 1).to_string();
    let host = format!("127.0.0.1:{}", server.port);

    for (path, body) in posts(P, "C-0001") {
        let body = body.to_string();
        // Another project: 404, though the rest of the request is wrong too.
        let foreign = path.replace("/card-press/", "/nowhere/");
        for headers in [&[][..], &[plain][..], &[same, json_type][..]] {
            let reply = server.send("POST", &foreign, headers, body.as_bytes());
            assert_eq!(reply.status, 404, "{foreign} {headers:?}: {}", reply.text());
            assert_eq!(
                reply.error_message(),
                "no project `nowhere` is served here: served card-press",
                "{foreign}"
            );
        }
        // No page: 403, before the type.
        for headers in [&[][..], &[plain][..], &[json_type][..]] {
            let reply = server.send("POST", &path, headers, body.as_bytes());
            assert_eq!(reply.status, 403, "{path} {headers:?}: {}", reply.text());
            assert_eq!(reply.error_message(), PAGE, "{path} {headers:?}");
        }
        // Not JSON: 415, before the length.
        for headers in [
            &[same][..],
            &[same, plain][..],
            &[same, ("Content-Type", "application/json-patch+json")][..],
            &[same, json_type, plain][..],
        ] {
            let reply = server.send("POST", &path, headers, body.as_bytes());
            assert_eq!(reply.status, 415, "{path} {headers:?}: {}", reply.text());
            assert_eq!(reply.error_message(), body_line, "{path} {headers:?}");
        }
        let mut oversized = request_bytes(
            "POST",
            &path,
            &[
                ("Host", &host),
                same,
                plain,
                ("Content-Length", over.as_str()),
            ],
        );
        oversized.extend_from_slice(b"{}");
        let reply = server.raw(&oversized);
        assert_eq!(reply.status, 415, "{path}: {}", reply.text());
        // Over the cap, declared and unread: 413, before the body is judged.
        let mut oversized = request_bytes(
            "POST",
            &path,
            &[
                ("Host", &host),
                same,
                json_type,
                ("Content-Length", over.as_str()),
            ],
        );
        oversized.extend_from_slice(b"{}");
        let reply = server.raw(&oversized);
        assert_eq!(reply.status, 413, "{path}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            format!("the body is over {CAP} bytes"),
            "{path}"
        );
        // A body that does not decode: 400.
        let reply = server.send("POST", &path, &[same, json_type], b"{\"nope\":");
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        reply.error_message();
    }
    // A bad charset parameter is still JSON: the media type counts.
    let reply = server.send(
        "POST",
        &format!("{P}/cards"),
        &[same, ("Content-Type", "application/json; charset=utf-8")],
        b"{\"nope\":",
    );
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(
        beyond_daemon(&home).is_empty(),
        "a refusal read nothing: the fresh HOME holds {:?}",
        beyond_daemon(&home)
    );
}

/// `CARD_BODY_MAX` is 1 MiB, to the byte: a body of exactly the cap is read
/// and stored, one byte more is a 413. JSON's own white space pads it.
#[test]
fn a_body_of_exactly_the_cap_is_read_and_one_byte_more_is_a_413() {
    let press = Press::new("cards-cap");
    let same = ("Sec-Fetch-Site", "same-origin");
    let json_type = ("Content-Type", "application/json");
    let head = "{\"title\":\"Padded\",\"discipline\":\"copy\"";
    let path = format!("{P}/cards");
    let padded = |size: usize| format!("{head}{}}}", " ".repeat(size - head.len() - 1));
    let at_the_cap = padded(CAP);
    assert_eq!(at_the_cap.len(), CAP);
    let reply = press
        .server
        .send("POST", &path, &[same, json_type], at_the_cap.as_bytes());
    assert_eq!(reply.status, 200, "{:?}", reply.header("content-type"));
    assert_eq!(reply.json()["id"], json!("C-0001"));
    let host = format!("127.0.0.1:{}", press.server.port);
    let over = (CAP + 1).to_string();
    let mut bytes = request_bytes(
        "POST",
        &path,
        &[
            ("Host", &host),
            same,
            json_type,
            ("Content-Length", over.as_str()),
        ],
    );
    bytes.extend_from_slice(padded(CAP + 1).as_bytes());
    let reply = lenient(press.server.port, &bytes);
    assert_eq!(reply.0, 413, "{}", reply.1);
    assert_eq!(press.events().len(), 1, "the 413 stored nothing");
}

/// The status and text of an answer that may come before the body is read,
/// so the write may fail.
fn lenient(port: u16, bytes: &[u8]) -> (u16, String) {
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

/// "The door": `Card*Body`, `deny_unknown_fields`: what is no body of its
/// route is a 400 and no event: no key the CLI does not take, an edit naming
/// `discipline` or `assignee` ("Creating and editing": those are not edited),
/// a repeated key, a wrong type.
#[test]
fn a_body_that_is_not_the_routes_is_a_400() {
    let press = Press::new("cards-bodies");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "assignee": "ana"}),
    );
    let before = press.state();
    for (path, body) in [
        ("cards", ""),
        ("cards", "not json"),
        ("cards", "null"),
        ("cards", "[]"),
        ("cards", "\"Lead story\""),
        ("cards", "{}"),
        ("cards", "{\"title\":null}"),
        ("cards", "{\"title\":3}"),
        ("cards", "{\"title\":\"One\",\"owner\":\"ana\"}"),
        ("cards", "{\"title\":\"One\",\"target\":\"SHT-FRONT\"}"),
        ("cards", "{\"title\":\"One\",\"criterion\":\"Fits.\"}"),
        ("cards", "{\"title\":\"One\",\"title\":\"Two\"}"),
        ("cards", "{\"title\":\"One\",\"as\":7}"),
        ("cards/C-0001/move", "{\"to\":\"edit\",\"force\":true}"),
        ("cards/C-0001/move", "{\"to\":[\"edit\"]}"),
        ("cards/C-0001/move", "{\"to\":\"edit\",\"revision\":1}"),
        ("cards/C-0001/edit", "{\"discipline\":\"plates\"}"),
        ("cards/C-0001/edit", "{\"assignee\":\"ben\"}"),
        (
            "cards/C-0001/edit",
            "{\"goal\":\"One.\",\"reviewer\":\"ben\"}",
        ),
        (
            "cards/C-0001/assign",
            "{\"assignee\":\"ben\",\"title\":\"One\"}",
        ),
        ("cards/C-0001/assign", "{\"assignee\":[\"ben\"]}"),
        ("cards/C-0001/cancel", "{}"),
        ("cards/C-0001/cancel", "{\"reason\":null}"),
        (
            "cards/C-0001/cancel",
            "{\"reason\":\"Late.\",\"note\":\"n\"}",
        ),
        (
            "cards/C-0001/accept",
            "{\"as\":\"ben\",\"decision\":\"approve\"}",
        ),
        ("cards/C-0001/accept", "{\"as\":\"ben\",\"consent\":true}"),
        ("cards/C-0001/accept", "{\"as\":\"ben\",\"yes\":true}"),
    ] {
        let reply = press.server.stage(&format!("{P}/{path}"), body);
        assert_eq!(reply.status, 400, "{path} {body:?}: {}", reply.text());
        assert!(!reply.error_message().is_empty(), "{path} {body:?}");
    }
    // Bytes that are no UTF-8, a lone surrogate, nothing after the key.
    let same = ("Sec-Fetch-Site", "same-origin");
    let json_type = ("Content-Type", "application/json");
    for body in [
        &b"{\"title\":\"\xff\"}"[..],
        &b"{\"title\":\"\\ud800\"}"[..],
        &b"{\"title\""[..],
    ] {
        let reply = press
            .server
            .send("POST", &format!("{P}/cards"), &[same, json_type], body);
        assert_eq!(reply.status, 400, "{body:?}: {}", reply.text());
        assert!(!reply.error_message().is_empty(), "{body:?}");
    }
    assert_eq!(press.state(), before, "nothing stored");
}

/// A bad `%` in a path's ID is a 400, before anything is read (as a task's).
#[test]
fn a_path_id_that_does_not_decode_is_a_400() {
    let scratch = Scratch::new("cards-path");
    let root = scratch.repo("card-press", "press", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&root]);
    let reply = server.get(&format!("{P}/cards/%FF"));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(reply.error_message().contains("%FF"), "{reply:?}");
    let reply = server.stage(&format!("{P}/cards/%FF/move"), "{\"to\":\"edit\"}");
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(reply.error_message().contains("%FF"), "{reply:?}");
    assert!(
        beyond_daemon(&home).is_empty(),
        "nothing read: {:?}",
        beyond_daemon(&home)
    );
}

// --------------------------------------------------------------- AC-05

/// AC-05, the daemon's half, on one screen: new, assign, move, move, accept
/// through the routes alone, each write carrying the revision the last one
/// answered, end accepted, `acceptance.by` the typed name; a restart reads
/// it back, the CLI too. `git status` stays empty, `HEAD` and the commit
/// count stay. M: the name dropped from the body (the accept then is
/// `local`'s, refused).
#[test]
fn ac05_a_full_cycle_ends_accepted_and_leaves_git_alone() {
    let press = Press::new("cards-cycle");
    let git = press.git_side();
    assert_eq!(git.0, "", "the repository starts clean");
    let made = press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"],
            "assignee": "ana", "as": "desk"}),
    );
    let id = made["id"].as_str().expect("an ID").to_owned();
    assert_eq!(id, "C-0001");
    let mut revision = made["revision"].clone();
    let step = |path: &str, mut body: Value, revision: &mut Value| {
        body["revision"] = revision.clone();
        let reply = press.done(&format!("cards/{id}/{path}"), &body);
        *revision = reply["revision"].clone();
        reply
    };
    step(
        "assign",
        json!({"reviewer": "ben", "as": "desk"}),
        &mut revision,
    );
    step("move", json!({"to": "edit", "as": "desk"}), &mut revision);
    step("move", json!({"to": "proof", "as": "ana"}), &mut revision);
    // The name left out of the body: no accept, no event.
    let seen = press.events().len();
    let reply = press.post(
        &format!("cards/{id}/accept"),
        &json!({"briefed": true, "note": "No name.", "revision": revision}),
    );
    assert!(
        reply.status == 400 || reply.status == 409,
        "an accept with no name: {}",
        reply.text()
    );
    assert_eq!(press.events().len(), seen, "no event");
    assert_eq!(press.show(&id)["acceptance"], json!(null));
    let accepted = step(
        "accept",
        json!({"briefed": true, "as": "ben", "note": "Read twice."}),
        &mut revision,
    );
    assert_eq!(accepted["column"], json!("filed"), "{accepted}");
    assert_eq!(accepted["allowed"], json!([]), "{accepted}");

    let board = press.get("cards").status(200).json();
    let row = &board["cards"][0];
    assert_eq!(row["id"], json!(id), "{board}");
    assert_eq!(row["column"], json!("filed"), "{board}");
    assert_eq!(row["accepted"], json!(true), "{board}");
    let card = press.show(&id);
    assert_eq!(card["acceptance"]["by"], json!("ben"), "{card}");
    assert_eq!(card["acceptance"]["note"], json!("Read twice."), "{card}");
    assert_eq!(press.git_side(), git, "no file, no commit, no ref");
    assert_eq!(
        press
            .events()
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<Vec<_>>(),
        [
            "card.created",
            "card.assigned",
            "card.moved",
            "card.moved",
            "card.accepted"
        ],
        "no proposal, no task"
    );

    // A reload: the daemon gone, the queue kept.
    let Press {
        server,
        root,
        home,
        cwd,
        scratch,
    } = press;
    drop(server);
    let server = Server::serve(&home, &cwd, &[&root]);
    let read = server.get(&format!("{P}/cards/{id}")).status(200).json();
    assert_eq!(read["acceptance"], card["acceptance"], "the typed name");
    let cli = spec_json(&home, &cwd, &root, &["card", "show", &id, "--history"]);
    cli.code(0);
    assert_eq!(cli.json()["acceptance"]["by"], json!("ben"));
    drop(server);
    drop(scratch);
}

/// Every `.rs` under `dir`, with its text.
fn sources(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("a source directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = fs::read_to_string(&path).expect("source");
                files.push((path, text));
            }
        }
    }
    files
}

/// The HTTP client a dependency line of `Cargo.toml` names, if any.
fn http_client(line: &str) -> Option<&'static str> {
    let name = line.split(['=', '.']).next()?.trim();
    [
        "reqwest",
        "ureq",
        "hyper",
        "isahc",
        "surf",
        "attohttpc",
        "curl",
        "http",
    ]
    .into_iter()
    .find(|client| *client == name)
}

/// AC-05: no tool reaches the accept route: `specengine-mcp` names no
/// `/api/` and depends on no HTTP client ("The acting name, and acceptance").
#[test]
fn ac05_specengine_mcp_names_no_route_and_holds_no_http_client() {
    let krate = repository_root().join("crates/specengine-mcp");
    let files = sources(&krate.join("src"));
    assert!(files.len() >= 5, "the tools' sources: {}", files.len());
    for (path, text) in &files {
        assert!(!text.contains("/api/"), "{}: names /api/", path.display());
    }
    let manifest = fs::read_to_string(krate.join("Cargo.toml")).expect("Cargo.toml");
    let clients: Vec<&str> = manifest.lines().filter_map(http_client).collect();
    assert_eq!(clients, Vec::<&str>::new(), "{manifest}");
    // The control: a client's line is caught, a line about something else
    // is not.
    assert_eq!(http_client("reqwest = \"0.12\""), Some("reqwest"));
    assert_eq!(http_client("hyper.workspace = true"), Some("hyper"));
    assert_eq!(http_client("specengine-cli = { path = \"x\" }"), None);
}

// ------------------------------------------------------------- the tail

/// "Rules and edge cases": `WHEN a card.* event arrives`: the page's tail
/// carries each write, and a refused write nothing: the next frame is the
/// next stored event, in `seq` order.
#[test]
fn a_write_reaches_the_tail_and_a_refused_one_does_not() {
    let press = Press::new("cards-tail");
    let mut stream = Stream::open(
        press.server.port,
        &format!("{P}/events"),
        &[("Last-Event-ID", "0")],
    );
    assert_eq!(stream.status, 200);
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let wait = Duration::from_secs(20);
    let frame = stream.next_event(wait).expect("the created card");
    assert_eq!(frame.event.as_deref(), Some("card.created"), "{frame:?}");
    let first: i64 = frame
        .id
        .as_deref()
        .and_then(|id| id.parse().ok())
        .expect("a seq");
    let data: Value = serde_json::from_str(frame.data.as_deref().expect("data")).expect("JSON");
    assert_eq!(data["id"], json!("C-0001"), "{frame:?}");
    assert_eq!(data["as"], json!("local"), "{frame:?}");

    let refused = press.post("cards/C-0001/move", &json!({"to": "proof"}));
    assert_eq!(refused.status, 409, "{}", refused.text());
    press.done("cards/C-0001/move", &json!({"to": "edit", "as": "desk"}));
    let frame = stream.next_event(wait).expect("the move");
    assert_eq!(frame.event.as_deref(), Some("card.moved"), "{frame:?}");
    assert_eq!(
        frame.id,
        Some((first + 1).to_string()),
        "the refused write stored no event"
    );
}

// -------------------------------------------------------- other projects

/// ADR-0008: the routes know no project's words. A project with no `[card]`
/// walks `plain` (titles and all), its accept `review` to `done`; the
/// press's `blocked` is a column like any other (ADR-0012), which a card
/// enters, leaves and is accepted past.
#[test]
fn the_routes_walk_another_projects_flow_and_a_blocked_column_is_data() {
    let keep = Press::of("cards-plain", "spec-a");
    let base = "/api/projects/lantern-keep";
    let board = keep.server.get(&format!("{base}/cards")).status(200).json();
    assert_eq!(board["cards"], json!([]), "the board starts empty");
    let flows = board["flows"].as_array().expect("flows");
    assert_eq!(flows.len(), 1, "{board}");
    assert_eq!(flows[0]["name"], json!("plain"));
    assert_eq!(
        flows[0]["columns"],
        json!([
            {"id": "backlog", "title": "Backlog"}, {"id": "ready", "title": "Ready"},
            {"id": "in-progress", "title": "In progress"}, {"id": "review", "title": "Review"},
            {"id": "done", "title": "Done"}
        ])
    );
    let post = |path: &str, body: Value| {
        let reply = keep
            .server
            .stage(&format!("{base}/{path}"), &body.to_string());
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        reply
    };
    let made = post("cards", json!({"title": "Refill the oil"}));
    assert_eq!(
        made.text(),
        "{\"id\":\"C-0001\",\"flow\":\"plain\",\"column\":\"backlog\",\"position\":1,\
         \"revision\":\"1\",\"allowed\":[{\"to\":\"ready\",\"by\":[]},{\"to\":\"in-progress\",\
         \"by\":[]},{\"to\":\"review\",\"by\":[]}],\"notes\":[]}"
    );
    post("cards/C-0001/move", json!({"to": "review"}));
    post("cards/C-0001/assign", json!({"reviewer": "ben"}));
    let accepted = post("cards/C-0001/accept", json!({"briefed": true, "as": "ben"}));
    assert_eq!(accepted.json()["column"], json!("done"));
    assert_eq!(
        keep.scratch
            .git()
            .run(&keep.root, &["status", "--porcelain", "--ignored"]),
        "",
        "the other project's tree too"
    );

    let press = Press::new("cards-blocked");
    press.done(
        "cards",
        &json!({"title": "Front plate", "discipline": "plates"}),
    );
    let moved = press.done("cards/C-0001/move", &json!({"to": "blocked"}));
    assert_eq!(
        moved["allowed"],
        json!([{"to": "queued", "by": []}, {"to": "checked", "by": []}]),
        "{moved}"
    );
    press.done("cards/C-0001/assign", &json!({"reviewer": "ben"}));
    press.done("cards/C-0001/move", &json!({"to": "checked"}));
    let accepted = press.done(
        "cards/C-0001/accept",
        &json!({"briefed": true, "as": "ben"}),
    );
    assert_eq!(accepted["column"], json!("struck"), "{accepted}");
}

/// docs/canon/cards.md "Commands": `open_cards`, never `open_context`: no git
/// runs, so any root works. The daemon's card routes need no repository:
/// the whole cycle on a plain directory.
#[test]
fn the_routes_run_on_a_root_that_is_no_repository() {
    let scratch = Scratch::new("cards-no-git");
    let root = scratch.copy("card-press", "press");
    assert!(!root.join(".git").exists());
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&root]);
    let files = common::snapshot(&root);
    let post = |path: &str, body: Value| {
        let reply = server.stage(&format!("{P}/{path}"), &body.to_string());
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        reply.json()
    };
    post(
        "cards",
        json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    post("cards/C-0001/assign", json!({"reviewer": "ben"}));
    post("cards/C-0001/move", json!({"to": "edit", "as": "desk"}));
    post("cards/C-0001/move", json!({"to": "proof"}));
    let accepted = post("cards/C-0001/accept", json!({"briefed": true, "as": "ben"}));
    assert_eq!(accepted["column"], json!("filed"), "{accepted}");
    assert_eq!(common::snapshot(&root), files, "nothing under the root");
}

/// "Routes": `GET cards` is the browser view. Past `OUTPUT_CAP_CHARS` the CLI
/// cuts, `truncated` true and a count in `omitted`; the daemon serves every
/// row, `truncated` false, `omitted` null (the CLI's bytes up to the cut, and
/// a CLI oracle for each row past it: `parity.rs`). M: `in_view` dropped.
#[test]
fn past_the_cap_the_daemon_serves_every_row_while_the_cli_cuts() {
    let press = Press::new("cards-past-the-cap");
    let title = "t".repeat(240);
    for _ in 0..200 {
        press.done("cards", &json!({"title": title, "discipline": "copy"}));
    }
    for (query, args) in [
        ("cards", &["card", "list"][..]),
        (
            "cards?with_cancelled=true",
            &["card", "list", "--with-cancelled"][..],
        ),
    ] {
        let cut = press.cli(args);
        cut.code(0);
        let capped = cut.json();
        assert_eq!(capped["truncated"], json!(true), "{query}: the CLI cuts");
        let shown = capped["cards"].as_array().expect("cards").len();
        assert!(shown < 200, "{query}: {shown} rows");
        assert_eq!(capped["omitted"], json!(200 - shown), "{query}");

        let reply = press.get(query);
        assert_eq!(reply.status, 200, "{query}: {}", reply.text());
        let whole = reply.json();
        assert_eq!(
            whole["cards"].as_array().map(Vec::len),
            Some(200),
            "{query}: every row"
        );
        assert_eq!(whole["truncated"], json!(false), "{query}");
        assert_eq!(whole["omitted"], Value::Null, "{query}");
        let last = &whole["cards"][199];
        assert_eq!(last["id"], json!("C-0200"), "{query}: the last row");
        assert_eq!(last["position"], json!(200), "{query}");
    }
}

/// `name` for a card's person: a full name in any script, one line, at most
/// 128 bytes after trimming, compared exactly: the first name alone or
/// another case is someone else. As acting person, assignee, reviewer and in
/// a move's `by`, through every route that takes one, and in a filter.
#[test]
fn a_full_name_in_any_script_acts_and_is_compared_exactly_after_trimming() {
    let press = Press::new("cards-names");
    replace(
        &press.root,
        "specengine.toml",
        "by = [\"desk\"]",
        &format!("by = [\"desk\", \"{FULL_NAME}\"]"),
    );
    let made = press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy", "assignee": FULL_NAME,
            "as": FULL_NAME}),
    );
    assert_eq!(
        made["allowed"],
        json!([{"to": "edit", "by": ["desk", FULL_NAME]}]),
        "{made}"
    );
    press.done(
        "cards/C-0001/assign",
        &json!({"reviewer": format!("  {OTHER_NAME} "), "as": FULL_NAME}),
    );
    let card = press.show("C-0001");
    assert_eq!(card["created_by"], json!(FULL_NAME), "{card}");
    assert_eq!(card["assignee"], json!(FULL_NAME), "{card}");
    assert_eq!(card["reviewer"], json!(OTHER_NAME), "trimmed: {card}");

    // Exactly: her first name alone, her name in lower case: not `by`'s.
    let first = "\u{0410}\u{043d}\u{043d}\u{0430}";
    let lower =
        "\u{0430}\u{043d}\u{043d}\u{0430} \u{043f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";
    for near in [first, lower] {
        press.refused(
            "cards/C-0001/move",
            &json!({"to": "edit", "as": near}),
            &["card", "move", "C-0001", "--to", "edit", "--as", near],
            &format!(
                "C-0001 is in draft: the move to `edit` is `desk` or `{FULL_NAME}`'s, not \
                 `{near}`'s; nothing changed"
            ),
        );
    }
    // After trimming, the same name; the event records it trimmed.
    press.done(
        "cards/C-0001/move",
        &json!({"to": "edit", "as": format!("  {FULL_NAME}  ")}),
    );
    let (kind, payload) = press.events().pop().expect("the move");
    assert_eq!(kind, "card.moved");
    assert_eq!(payload["as"], json!(FULL_NAME), "trimmed: {payload}");
    press.done(
        "cards/C-0001/move",
        &json!({"to": "proof", "as": FULL_NAME}),
    );

    // A filter, as the page sends it and with `+` for the space.
    let encoded = encode_component(FULL_NAME);
    let run = press.cli(&["card", "list", "--assignee", FULL_NAME]);
    run.code(0);
    for query in [encoded.clone(), encoded.replace("%20", "+")] {
        let reply = press.get(&format!("cards?assignee={query}"));
        assert_eq!(reply.status, 200, "{query}: {}", reply.text());
        assert_eq!(reply.text(), run.document(), "{query}: the CLI's document");
        assert_eq!(reply.json()["cards"][0]["id"], json!("C-0001"), "{query}");
    }

    // Accept: the reviewer's own name, exactly, never the assignee's.
    let accept = "cards/C-0001/accept";
    press.refused(
        accept,
        &json!({"briefed": true, "as": format!(" {FULL_NAME} ")}),
        &[],
        &format!(
            "C-0001: `{FULL_NAME}` is its assignee, who never accepts a result; nothing changed"
        ),
    );
    press.refused(
        accept,
        &json!({"briefed": true, "as": first}),
        &[],
        &format!(
            "C-0001: its reviewer is `{OTHER_NAME}`, and `--as` says `{first}`; nothing changed"
        ),
    );
    // `agent` and `local` are no one's name, trimmed or not ("Who acts").
    for (written, name) in [
        ("agent", "agent"),
        ("local", "local"),
        (" agent ", "agent"),
        (" local ", "local"),
    ] {
        press.refused(
            accept,
            &json!({"briefed": true, "as": written}),
            &[],
            &format!(
                "C-0001: `{name}` can be no reviewer: a person accepts a result; nothing changed"
            ),
        );
        let before = press.state();
        let reply = press.post("cards/C-0001/assign", &json!({"reviewer": written}));
        assert_eq!(reply.status, 409, "{written:?}: {}", reply.text());
        assert_eq!(press.state(), before, "{written:?}: nothing changed");
    }
    let accepted = press.done(
        accept,
        &json!({"briefed": true, "as": format!(" {OTHER_NAME} "), "note": "Read twice."}),
    );
    assert_eq!(accepted["column"], json!("filed"), "{accepted}");
    let card = press.show("C-0001");
    assert_eq!(card["acceptance"]["by"], json!(OTHER_NAME), "{card}");
}

/// What no card's person may be: nothing after trimming, more than a line,
/// a control character, U+2028, U+2029, a bidi mark, over 128 bytes (in
/// bytes: 65 letters of two bytes each, 128 characters holding 129 bytes).
fn off_the_grammar() -> Vec<String> {
    let mut names: Vec<String> = [
        "",
        "   ",
        "two\nlines",
        "tab\tinside",
        "bell\u{7}",
        "a\u{85}b",
        "a\u{61c}b",
        "a\u{2028}b",
        "a\u{2029}b",
        "a\u{200e}b",
        "a\u{200f}b",
        "a\u{202a}b",
        "a\u{202e}b",
        "a\u{2066}b",
        "a\u{2069}b",
    ]
    .map(str::to_owned)
    .to_vec();
    names.push("n".repeat(129));
    names.push("\u{0430}".repeat(65));
    names.push(format!("{}\u{0430}", "n".repeat(127)));
    names
}

/// A person off the grammar from the page is a 400 with the CLI's line, not
/// a 503, wherever it stands: `as` on every write (exit 2 on the CLI),
/// `assignee` and `reviewer` (exit 1 there). Nothing stored. At the limit, 128
/// bytes in any script, a name passes, and the limit is the trimmed name's.
#[test]
fn a_person_off_the_grammar_from_the_page_is_a_400_with_the_clis_line() {
    let press = Press::new("cards-off-grammar");
    press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let before = press.state();
    let edit = "cards/C-0001/edit";
    for name in off_the_grammar() {
        let name = name.as_str();
        let reply = press.post(edit, &json!({"goal": "Late.", "as": name}));
        assert_eq!(reply.status, 400, "as {name:?}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            press.cannot(&["card", "edit", "C-0001", "--goal", "Late.", "--as", name]),
            "as {name:?}: the CLI's line"
        );
        for (path, mut body) in posts(P, "C-0001") {
            body["as"] = json!(name);
            let reply = press.server.stage(&path, &body.to_string());
            assert_eq!(reply.status, 400, "{path} {name:?}: {}", reply.text());
        }
        let cases: [(&str, Value, Vec<&str>); 3] = [
            (
                "cards",
                json!({"title": "Late", "discipline": "copy", "assignee": name}),
                vec![
                    "card",
                    "new",
                    "--title",
                    "Late",
                    "--discipline",
                    "copy",
                    "--assignee",
                    name,
                ],
            ),
            (
                "cards/C-0001/assign",
                json!({"assignee": name}),
                vec!["card", "assign", "C-0001", "--assignee", name],
            ),
            (
                "cards/C-0001/assign",
                json!({"reviewer": name}),
                vec!["card", "assign", "C-0001", "--reviewer", name],
            ),
        ];
        for (path, body, args) in cases {
            let reply = press.post(path, &body);
            assert_eq!(reply.status, 400, "{path} {body}: {}", reply.text());
            let run = press.cli(&args);
            run.code(1);
            assert_eq!(
                reply.error_message(),
                run.stderr.trim_end_matches('\n'),
                "{path} {body}: the CLI's line"
            );
        }
    }
    // What no `argv` can carry, from a page all the same.
    let reply = press.post(edit, &json!({"goal": "Late.", "as": "a\u{0}b"}));
    assert_eq!(reply.status, 400, "a NUL: {}", reply.text());
    assert_eq!(press.state(), before, "nothing stored");

    // The limit's own words, the CLI's line, in bytes.
    let over = format!("{}\u{0430}", "n".repeat(127));
    let reply = press.post(edit, &json!({"goal": "Late.", "as": over}));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert_eq!(reply.error_message(), "spec: --as: 129 bytes; at most 128");
    let reply = press.post("cards/C-0001/assign", &json!({"reviewer": "n".repeat(129)}));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        "spec: reviewer: 129 bytes; at most 128"
    );

    // At the limit, in bytes: one, two and three bytes a letter.
    let longest = [
        "n".repeat(128),
        "\u{0430}".repeat(64),
        format!("{}nn", "\u{4e2d}".repeat(42)),
    ];
    for name in &longest {
        assert_eq!(name.len(), 128, "{name}");
        let body = json!({"assignee": name, "reviewer": name, "as": name});
        let done = press.done("cards/C-0001/assign", &body);
        assert_eq!(done["id"], json!("C-0001"), "{name}");
        let card = press.show("C-0001");
        assert_eq!(card["assignee"], json!(name), "{card}");
        assert_eq!(card["reviewer"], json!(name), "{card}");
    }
    // The limit is the trimmed name's: spaces around it cost nothing.
    let spaced = format!("  {}  ", longest[0]);
    press.done("cards/C-0001/assign", &json!({"assignee": spaced}));
    assert_eq!(press.show("C-0001")["assignee"], json!(longest[0]));
}

/// A flow's `by` is read trimmed: a config's stray spaces reach no board and
/// no comparison, and the longest name fits there too.
#[test]
fn a_by_is_trimmed_from_the_config_and_holds_the_longest_name() {
    let press = Press::new("cards-by-trimmed");
    let config = common::read_text(&press.root, "specengine.toml");
    let by_of = |written: &str| {
        common::write(
            &press.root,
            "specengine.toml",
            config.replace("by = [\"desk\"]", written),
        );
    };
    by_of(&format!("by = [\"  desk \", \" {FULL_NAME}  \"]"));
    let made = press.done(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    let by = json!(["desk", FULL_NAME]);
    assert_eq!(made["allowed"], json!([{"to": "edit", "by": by}]), "{made}");
    let board = press.get("cards").status(200).json();
    assert_eq!(board["flows"][0]["moves"][0]["by"], by, "{board}");
    press.done("cards/C-0001/move", &json!({"to": "edit", "as": FULL_NAME}));

    let longest = ["n".repeat(128), "\u{0430}".repeat(64)];
    for name in &longest {
        by_of(&format!("by = [\"{name}\"]"));
        let made = press.done("cards", &json!({"title": "Longest", "discipline": "copy"}));
        let id = made["id"].as_str().expect("an ID");
        let moved = press.done(
            &format!("cards/{id}/move"),
            &json!({"to": "edit", "as": name}),
        );
        assert_eq!(moved["column"], json!("edit"), "{name}");
    }
    // One byte more is a bad `[card]`: the 503 of "States".
    by_of(&format!("by = [\"{}\"]", "n".repeat(129)));
    let line = press.cannot(&["card", "list"]);
    let reply = press.get("cards");
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line);
    assert!(line.contains("`by`"), "{line}");
}

/// "Store": a card belongs to its project's queue: two projects under one
/// `HOME` keep two boards and two ID ranges through one daemon.
#[test]
fn a_card_belongs_to_the_project_it_was_posted_to() {
    let scratch = Scratch::new("cards-two");
    let press = scratch.repo("card-press", "press", "main");
    let keep = scratch.repo("spec-a", "keep", "trunk");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&press, &keep]);
    let other = "/api/projects/lantern-keep";
    let reply = server.stage(
        &format!("{P}/cards"),
        "{\"title\":\"Lead story\",\"discipline\":\"copy\"}",
    );
    let reply = reply.status(200).json();
    assert_eq!(reply["flow"], json!("copy"), "{reply}");
    let reply = server.stage(&format!("{other}/cards"), "{\"title\":\"Refill\"}");
    let reply = reply.status(200).json();
    assert_eq!(reply["id"], json!("C-0001"), "a range of its own: {reply}");
    assert_eq!(reply["flow"], json!("plain"), "{reply}");
    let title = |base: &str| -> Value {
        server
            .get(&format!("{base}/cards/C-0001"))
            .status(200)
            .json()["title"]
            .clone()
    };
    assert_eq!(title(P), json!("Lead story"));
    assert_eq!(title(other), json!("Refill"));
    let reply = server.get(&format!("{other}/cards/C-0002"));
    assert_eq!(reply.status, 404, "{}", reply.text());
}

// ------------------------------------------------ board-handoff (RM-0085)

/// The game's project (`fixtures/studio-lamp`): the gamedev preset's flows,
/// each with `return = "sent-back"`.
const LAMP: &str = "/api/projects/studio-lamp";

/// The refusal of a move into the return column with no note
/// (docs/canon/card-flows.md "Return").
fn needs_note(id: &str) -> String {
    format!(
        "{id} enters `sent-back`, its flow's return column: say why with `--note`; nothing \
         changed"
    )
}

impl Press {
    /// A daemon over a repository of `fixtures/studio-lamp`.
    fn lamp(name: &str) -> Self {
        Self::of(name, "studio-lamp")
    }

    /// A same-origin page's `POST <LAMP>/path`.
    fn lamp_post(&self, path: &str, body: &Value) -> Reply {
        self.server
            .stage(&format!("{LAMP}/{path}"), &body.to_string())
    }

    /// As [`Press::lamp_post`], the answer a 200.
    fn lamp_done(&self, path: &str, body: &Value) -> Value {
        let reply = self.lamp_post(path, body);
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        reply.json()
    }

    /// The events and the game's board, every row.
    fn lamp_state(&self) -> (Vec<(String, Value)>, String) {
        let board = self
            .server
            .get(&format!("{LAMP}/cards?with_cancelled=true"))
            .status(200)
            .json();
        (self.events(), board.to_string())
    }

    /// `path` with `body` refused with `reason`: a 409, the write document
    /// with the reason last of `notes`, nothing changed, and byte for byte
    /// the CLI's exit-1 document of `args`.
    fn lamp_refused(&self, path: &str, body: &Value, args: &[&str], reason: &str) {
        let before = self.lamp_state();
        let reply = self.lamp_post(path, body);
        let context = format!("{path} {body}");
        assert_eq!(reply.status, 409, "{context}: {}", reply.text());
        let document = reply.json();
        let mut keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
        keys.sort();
        assert_eq!(keys, WRITE_KEYS, "{context}: {document}");
        assert_eq!(
            document["notes"].as_array().and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{context}: {document}"
        );
        assert_eq!(self.lamp_state(), before, "{context}: nothing changed");
        let run = self.cli(args);
        run.code(1);
        assert_eq!(
            reply.text(),
            run.document(),
            "{context}: the CLI's document"
        );
        assert_eq!(self.lamp_state(), before, "{context}: nothing changed");
    }
}

/// docs/features/board-handoff.md BH-12, the daemon's half: `POST .../cards
/// {…, reviewer, as}` is `spec card new --reviewer`: the hand-off stores its
/// reviewer and `card.created` carries it (`null` without); `agent` and
/// `local` are a 409 with the write document, as at `assign`; a reviewer off
/// the person grammar a 400 with the CLI's line; nothing stored on either.
/// The child at `verify` is accepted by its reviewer, refused to its
/// assignee. M14.
#[test]
fn board_handoff_bh12_a_body_names_the_reviewer() {
    let press = Press::lamp("cards-handoff-reviewer");
    let git = press.git_side();
    press.lamp_done(
        "cards",
        &json!({"title": "Lamp room scene", "discipline": "narrative", "as": "Ann"}),
    );
    assert_eq!(
        press.events().last().map(|event| event.1.clone()),
        Some(
            json!({"id": "C-0001", "flow": "narrative", "column": "backlog", "position": 1,
                    "assignee": null, "reviewer": null, "as": "Ann"})
        )
    );
    let made = press.lamp_post(
        "cards",
        &json!({"title": "Lamp room dialogue", "goal": "Two endings.", "discipline":
                "development", "target": ["SCN-ARRIVAL"], "parent": "C-0001", "reviewer":
                "Ann", "assignee": "Rob", "as": "Ann"}),
    );
    assert_eq!(made.status, 200, "{}", made.text());
    assert_eq!(made.json()["column"], json!("backlog"));
    assert_eq!(made.json()["flow"], json!("development"));
    assert_eq!(
        press.events().last().cloned(),
        Some((
            "card.created".to_owned(),
            json!({"id": "C-0002", "flow": "development", "column": "backlog",
                   "position": 1, "assignee": "Rob", "reviewer": "Ann", "as": "Ann"})
        ))
    );
    let card = press
        .server
        .get(&format!("{LAMP}/cards/C-0002"))
        .status(200)
        .json();
    assert_eq!(card["reviewer"], json!("Ann"), "{card}");
    assert_eq!(card["parent"], json!("C-0001"), "{card}");
    assert_eq!(card["targets"], json!(["SCN-ARRIVAL"]), "{card}");
    assert_eq!(press.git_side(), git, "git untouched");

    for name in ["agent", "local", " agent "] {
        let trimmed = name.trim();
        press.lamp_refused(
            "cards",
            &json!({"title": "No one's", "discipline": "development", "reviewer": name}),
            &[
                "card",
                "new",
                "--title",
                "No one's",
                "--discipline",
                "development",
                "--reviewer",
                name,
            ],
            &format!("reviewer: `{trimmed}` can be no reviewer: a person accepts a result"),
        );
    }
    let before = press.lamp_state();
    for name in off_the_grammar() {
        let reply = press.lamp_post(
            "cards",
            &json!({"title": "Late", "discipline": "development", "reviewer": name}),
        );
        assert_eq!(reply.status, 400, "{name:?}: {}", reply.text());
        let run = press.cli(&[
            "card",
            "new",
            "--title",
            "Late",
            "--discipline",
            "development",
            "--reviewer",
            &name,
        ]);
        run.code(1);
        assert_eq!(
            reply.error_message(),
            run.stderr.trim_end_matches('\n'),
            "{name:?}: the CLI's line"
        );
    }
    assert_eq!(press.lamp_state(), before, "nothing stored");

    // The child's result: its reviewer accepts it, its assignee never.
    press.lamp_done("cards/C-0002/move", &json!({"to": "verify", "as": "Rob"}));
    let accept = "cards/C-0002/accept";
    let before = press.lamp_state();
    let refused = press.lamp_post(accept, &json!({"briefed": true, "as": "Rob"}));
    assert_eq!(refused.status, 409, "{}", refused.text());
    assert_eq!(
        refused.json()["notes"]
            .as_array()
            .and_then(|notes| notes.last()),
        Some(&json!(
            "C-0002: `Rob` is its assignee, who never accepts a result; nothing changed"
        ))
    );
    assert_eq!(press.lamp_state(), before);
    let accepted = press.lamp_done(accept, &json!({"briefed": true, "as": "Ann"}));
    assert_eq!(accepted["column"], json!("done"), "{accepted}");
    assert_eq!(press.git_side(), git, "git untouched");
}

/// docs/features/board-handoff.md BH-14, the daemon's half: `POST
/// .../cards/:id/move {to, before, note, revision, as}` is `spec card move
/// --note`: into `sent-back` from another column without a note a 409 with
/// the write document, its reason last of `notes`, byte for byte the CLI's,
/// no event; with the note a 200 and `card.moved` holds it; a blank or
/// over-long note a 409 in `cancel --reason`'s words; a reorder inside the
/// column and a move out of it need none. M16, M17.
#[test]
fn board_handoff_bh14_a_move_body_takes_the_note() {
    let press = Press::lamp("cards-handoff-note");
    let git = press.git_side();
    press.lamp_done(
        "cards",
        &json!({"title": "Lamp room dialogue", "discipline": "development"}),
    );
    press.lamp_done(
        "cards",
        &json!({"title": "Lamp light shader", "discipline": "development"}),
    );
    let moved = press.lamp_done(
        "cards/C-0001/move",
        &json!({"to": "in-progress", "revision": "1", "as": "Rob"}),
    );
    assert_eq!(
        moved["column"],
        json!("in-progress"),
        "taking needs no note"
    );
    assert_eq!(
        press.events().last().map(|event| event.1.clone()),
        Some(
            json!({"id": "C-0001", "from": "backlog", "to": "in-progress", "position": 1,
                    "as": "Rob", "note": null})
        )
    );
    press.lamp_refused(
        "cards/C-0001/move",
        &json!({"to": "sent-back", "as": "Rob"}),
        &["card", "move", "C-0001", "--to", "sent-back", "--as", "Rob"],
        &needs_note("C-0001"),
    );
    press.lamp_refused(
        "cards/C-0001/move",
        &json!({"to": "sent-back", "note": null, "revision": "2"}),
        &[
            "card",
            "move",
            "C-0001",
            "--to",
            "sent-back",
            "--revision",
            "2",
        ],
        &needs_note("C-0001"),
    );
    let over = "n".repeat(4097);
    for (note, reason) in [
        ("  ", "note: blank; say why it moves"),
        (over.as_str(), "note: 4097 bytes; at most 4096"),
    ] {
        press.lamp_refused(
            "cards/C-0001/move",
            &json!({"to": "sent-back", "note": note}),
            &[
                "card",
                "move",
                "C-0001",
                "--to",
                "sent-back",
                "--note",
                note,
            ],
            reason,
        );
    }
    let seen = press.events().len();
    let back = press.lamp_done(
        "cards/C-0001/move",
        &json!({"to": "sent-back", "note": "Two endings contradict SCN-LAMP-ROOM",
                "revision": "2", "as": "Rob"}),
    );
    assert_eq!(back["column"], json!("sent-back"), "{back}");
    assert_eq!(back["revision"], json!("3"), "{back}");
    let events = press.events();
    assert_eq!(events.len(), seen + 1, "one event");
    assert_eq!(
        events.last(),
        Some(&(
            "card.moved".to_owned(),
            json!({"id": "C-0001", "from": "in-progress", "to": "sent-back", "position": 1,
                   "as": "Rob", "note": "Two endings contradict SCN-LAMP-ROOM"})
        ))
    );
    let shown = press
        .server
        .get(&format!("{LAMP}/cards/C-0001"))
        .status(200)
        .json();
    let last = shown["history"]
        .as_array()
        .and_then(|history| history.last())
        .expect("the panel's history")
        .clone();
    assert_eq!(
        last["payload"]["note"],
        json!("Two endings contradict SCN-LAMP-ROOM"),
        "{last}"
    );
    // A second card sent back, then a reorder inside the column: no note.
    press.lamp_done(
        "cards/C-0002/move",
        &json!({"to": "sent-back", "note": "Too dark"}),
    );
    press.lamp_done("cards/C-0002/move", &json!({"before": "C-0001"}));
    assert_eq!(
        press.events().last().map(|event| event.1.clone()),
        Some(
            json!({"id": "C-0002", "from": "sent-back", "to": "sent-back", "position": 1,
                    "as": "local", "note": null})
        )
    );
    // Out of it: none asked; a note on any move kept.
    press.lamp_done("cards/C-0001/move", &json!({"to": "ready"}));
    assert_eq!(
        press.events().last().map(|event| event.1["note"].clone()),
        Some(Value::Null)
    );
    press.lamp_done(
        "cards/C-0002/move",
        &json!({"to": "in-review", "note": "Lighter now"}),
    );
    assert_eq!(
        press.events().last().map(|event| event.1["note"].clone()),
        Some(json!("Lighter now"))
    );
    assert_eq!(press.git_side(), git, "git untouched");
}

/// docs/canon/board-ui.md "The door": an older body's keys still
/// decode, and a key neither body takes is a 400 before any call: `note` is
/// no key of `POST .../cards`, `reviewer` none of a move.
#[test]
fn board_handoff_each_body_takes_its_own_new_key_alone() {
    let press = Press::lamp("cards-handoff-keys");
    press.lamp_done(
        "cards",
        &json!({"title": "Lamp room dialogue", "discipline": "development"}),
    );
    let before = press.lamp_state();
    for (path, body) in [
        (
            "cards",
            json!({"title": "T", "discipline": "development", "note": "Why."}),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "ready", "reviewer": "Ann"}),
        ),
        (
            "cards/C-0001/move",
            json!({"to": "sent-back", "reason": "Why."}),
        ),
    ] {
        let reply = press.lamp_post(path, &body);
        assert_eq!(reply.status, 400, "{path} {body}: {}", reply.text());
        assert_eq!(press.lamp_state(), before, "{path} {body}: nothing changed");
    }
}

// ---------------------------------------------------- card-tasks (RM-0092)

/// The titles of the card-tasks example (CT-2).
const NORTH: &str = "Light the north lamp";
const SWITCH: &str = "Wire the switch";

/// `sqlite3 <db> <sql>` on `home`'s queue of `slug`; it must succeed.
fn queue_sql(home: &Path, slug: &str, sql: &str) {
    let done = std::process::Command::new("/usr/bin/sqlite3")
        .arg(common::data_dir(home).join(format!("{slug}.db")))
        .arg(sql)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(done.status.success(), "{sql}: {done:?}");
}

impl Press {
    /// `spec --json task new --nodes node --title title` in the served
    /// repository under the daemon's `HOME`: its ID.
    fn new_task(&self, node: &str, title: &str) -> String {
        let made = spec(
            &self.home,
            &self.root,
            &["--json", "task", "new", "--nodes", node, "--title", title],
        );
        made.code(0);
        made.json()["id"].as_str().expect("a task's ID").to_owned()
    }

    /// The game's `GET <LAMP>/cards/id`, a 200.
    fn lamp_show(&self, id: &str) -> Reply {
        let reply = self.server.get(&format!("{LAMP}/cards/{id}"));
        assert_eq!(reply.status, 200, "{id}: {}", reply.text());
        reply
    }
}

/// docs/features/card-tasks.md CT-1, the daemon's half: `POST
/// .../cards/C-0001/edit {"task": [...]}` is `spec card edit --task`: a 200,
/// the write document, the list stored in the order given, one `card.edited`
/// naming `tasks` (with `goal`, both in table order, `as` the body's), the
/// git side untouched; `["-"]` clears. No such task, a repeat and no task ID
/// are a 409 with the CLI's exit-1 document, nothing changed.
#[test]
fn card_tasks_ct1_an_edit_body_links_as_the_cli_does() {
    let press = Press::lamp("cards-tasks-link");
    assert_eq!(press.new_task("ISL-LAMP", NORTH), "T-0001");
    assert_eq!(press.new_task("ISL-LAMP-ROOM", SWITCH), "T-0002");
    let git = press.git_side();
    let made = press.lamp_done(
        "cards",
        &json!({"title": "Light the lamp", "discipline": "development"}),
    );
    assert_eq!(made["id"], json!("C-0001"));
    let ids = |press: &Press| -> Vec<Value> {
        press.lamp_show("C-0001").json()["tasks"]
            .as_array()
            .expect("tasks")
            .iter()
            .map(|entry| entry["id"].clone())
            .collect()
    };
    let edited = press.lamp_done("cards/C-0001/edit", &json!({"task": ["T-0002", "T-0001"]}));
    let mut keys: Vec<&String> = edited.as_object().expect("an object").keys().collect();
    keys.sort();
    assert_eq!(keys, WRITE_KEYS, "{edited}");
    assert_eq!(edited["revision"], json!("2"));
    assert_eq!(ids(&press), [json!("T-0002"), json!("T-0001")]);
    assert_eq!(
        press.events().last(),
        Some(&(
            "card.edited".to_owned(),
            json!({"id": "C-0001", "fields": ["tasks"], "as": "local"})
        ))
    );
    let edited = press.lamp_done(
        "cards/C-0001/edit",
        &json!({"task": ["T-0001"], "goal": "Before the storm.", "as": "Ann"}),
    );
    assert_eq!(edited["revision"], json!("3"));
    assert_eq!(ids(&press), [json!("T-0001")]);
    assert_eq!(
        press.events().last(),
        Some(&(
            "card.edited".to_owned(),
            json!({"id": "C-0001", "fields": ["goal", "tasks"], "as": "Ann"})
        ))
    );
    for (task, flags, reason) in [
        (
            json!(["T-0099"]),
            &["--task", "T-0099"][..],
            "task: no task T-0099; nothing changed",
        ),
        (
            json!(["T-0001", "T-0001"]),
            &["--task", "T-0001", "--task", "T-0001"][..],
            "task: T-0001 is listed twice; nothing changed",
        ),
        (
            json!(["foo"]),
            &["--task", "foo"][..],
            "task: \"foo\" is no task ID; nothing changed",
        ),
    ] {
        let mut args = vec!["card", "edit", "C-0001"];
        args.extend_from_slice(flags);
        press.lamp_refused("cards/C-0001/edit", &json!({"task": task}), &args, reason);
    }
    let cleared = press.lamp_done("cards/C-0001/edit", &json!({"task": ["-"]}));
    assert_eq!(cleared["revision"], json!("4"));
    assert_eq!(ids(&press), Vec::<Value>::new());
    assert_eq!(
        press.events().last(),
        Some(&(
            "card.edited".to_owned(),
            json!({"id": "C-0001", "fields": ["tasks"], "as": "local"})
        ))
    );
    assert_eq!(press.git_side(), git, "the queue alone");
}

/// docs/features/card-tasks.md CT-2, the daemon's half (D6, D7): `GET
/// .../cards/C-0001` serves `tasks` after `evidence`, each `{id, status,
/// title, stale}` read at request time (a plan shows `review`, the card's
/// `revision` unchanged), a corrupt row and a deleted one `null` with their
/// notes; its body byte for byte `spec card show C-0001 --history --json`.
#[test]
fn card_tasks_ct2_the_read_serves_tasks_as_the_cli_document() {
    let press = Press::lamp("cards-tasks-read");
    assert_eq!(press.new_task("ISL-LAMP", NORTH), "T-0001");
    assert_eq!(press.new_task("ISL-LAMP-ROOM", SWITCH), "T-0002");
    assert_eq!(press.new_task("ISL-LAMP", "Trim the wick"), "T-0003");
    press.lamp_done(
        "cards",
        &json!({"title": "Light the lamp", "discipline": "development",
                "target": ["ISL-LAMP"]}),
    );
    press.lamp_done(
        "cards/C-0001/edit",
        &json!({"task": ["T-0001", "T-0002", "T-0003"]}),
    );
    let same = |press: &Press| -> Value {
        let reply = press.lamp_show("C-0001");
        let run = press.cli(&["card", "show", "C-0001", "--history"]);
        run.code(0);
        assert_eq!(reply.text(), run.document(), "the CLI's document");
        reply.json()
    };
    let served = same(&press);
    assert_eq!(
        served["tasks"],
        json!([
            {"id": "T-0001", "status": "draft", "title": NORTH, "stale": null},
            {"id": "T-0002", "status": "draft", "title": SWITCH, "stale": null},
            {"id": "T-0003", "status": "draft", "title": "Trim the wick", "stale": null},
        ]),
        "{served}"
    );
    let text = press.lamp_show("C-0001").text().to_owned();
    let evidence = text.find("\"evidence\":").expect("evidence");
    let tasks = text.find("\"tasks\":").expect("tasks");
    let acceptance = text.find("\"acceptance\":").expect("acceptance");
    assert!(evidence < tasks && tasks < acceptance, "{text}");

    // Read at request time: a plan by the CLI shows at once.
    let plan = press.scratch.join("plan.md");
    fs::write(&plan, "1. Light it.\n").expect("a plan");
    let planned = spec(
        &press.home,
        &press.root,
        &[
            "task",
            "plan",
            "T-0001",
            "--plan-file",
            plan.to_str().expect("UTF-8"),
        ],
    );
    planned.code(0);
    let served = same(&press);
    assert_eq!(served["tasks"][0]["status"], json!("review"), "{served}");
    assert_eq!(served["revision"], json!("2"), "the card's revision stays");

    queue_sql(
        &press.home,
        "studio-lamp",
        "UPDATE tasks SET status = 'bogus' WHERE id = 'T-0002'; DELETE FROM tasks WHERE id = \
         'T-0003';",
    );
    let served = same(&press);
    assert_eq!(
        served["tasks"][1],
        json!({"id": "T-0002", "status": null, "title": null, "stale": null})
    );
    assert_eq!(
        served["tasks"][2],
        json!({"id": "T-0003", "status": null, "title": null, "stale": null})
    );
    let notes: Vec<&str> = served["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note"))
        .collect();
    assert!(
        notes.iter().any(
            |note| note.starts_with("T-0002: its row does not read (`status`: ")
                && note.ends_with("); no state shown")
        ),
        "{notes:?}"
    );
    assert!(
        notes.contains(&"T-0003: no such task in this queue; no state shown"),
        "{notes:?}"
    );
    // The board's read stays as it was: no task on a row (D9).
    let board = press
        .server
        .get(&format!("{LAMP}/cards"))
        .status(200)
        .json();
    let row = board["cards"][0].as_object().expect("a row");
    assert_eq!(row.len(), 21, "{board}");
    assert!(!row.contains_key("tasks"), "{board}");
}

// ------------------------------------------ card-questions (RM-0097), K1

/// docs/features/card-questions.md K1, the daemon's half on studio-lamp:
/// `GET .../cards` byte-equal before and after `C-0001`'s items are raised
/// (a question through the card's route, a decision through the CLI), and
/// to `spec card list --json`; `GET .../cards/C-0001` byte-equal to `spec
/// card show C-0001 --history --json`, its `items` after `tasks`, the
/// history holding the items' `proposal.created`. M: the two event lists
/// capped together at 64.
#[test]
fn card_questions_k1_the_card_read_serves_items_and_the_board_stays() {
    let press = Press::lamp("cards-cq-read");
    press.lamp_done(
        "cards",
        &json!({"title": "The storm scene", "discipline": "narrative",
            "target": ["SCN-STORM"]}),
    );
    let board = |press: &Press| {
        press
            .server
            .get(&format!("{LAMP}/cards"))
            .status(200)
            .text()
            .to_owned()
    };
    let before = board(&press);
    assert_eq!(before, press.cli(&["card", "list"]).document());

    let raised = press.lamp_done(
        "cards/C-0001/items",
        &json!({"kind": "question", "node_ids": ["SCN-STORM"],
            "text": "Which bell rings at dusk?",
            "working_answer": "The harbour bell, as in chapter one.",
            "price_of_other": "Two scenes rewritten.", "why": null, "severity": null,
            "distinct_from": null, "as": "dev"}),
    );
    assert_eq!(raised["id"], json!("PR-0001"), "{raised}");
    let root = press.root.to_str().expect("UTF-8 root").to_owned();
    let decided = common::run_product(
        &common::spec_bin(),
        &press.home,
        &press.cwd,
        &[
            "--root", &root, "--json", "propose", "decision", "--input", "-", "--card", "C-0001",
        ],
        json!({"node_ids": ["SCN-STORM"], "text": "Does the lamp burn oil?",
            "working_answer": "Yes: whale oil.", "price_of_other": "The lamp room is redrawn."})
        .to_string()
        .as_bytes(),
    );
    decided.code(0);
    assert_eq!(decided.json()["id"], json!("PR-0002"));

    assert_eq!(board(&press), before, "the board's read unchanged");
    assert_eq!(board(&press), press.cli(&["card", "list"]).document());

    let shown = press
        .server
        .get(&format!("{LAMP}/cards/C-0001"))
        .status(200)
        .text()
        .to_owned();
    let cli = press.cli(&["card", "show", "C-0001", "--history"]);
    cli.code(0);
    assert_eq!(shown, cli.document(), "byte for byte the CLI's");
    let card: Value = serde_json::from_str(&shown).expect("JSON");
    let ids: Vec<&str> = card["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, ["PR-0001", "PR-0002"]);
    let tasks = shown.find("\"tasks\":").expect("tasks");
    let items = shown.find("\"items\":").expect("items");
    let acceptance = shown.find("\"acceptance\":").expect("acceptance");
    assert!(tasks < items && items < acceptance, "{shown}");
    let created: Vec<&str> = card["history"]
        .as_array()
        .expect("history")
        .iter()
        .filter(|entry| entry["type"] == "proposal.created")
        .map(|entry| entry["payload"]["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(created, ["PR-0001", "PR-0002"]);
}
