//! docs/features/task-mandate.md (RM-0104, v5 step 6d, ADR-0079)
//! "Acceptance criteria", the daemon's half: the three bodies' `mandate`
//! through the real daemon, local and as a hub, over git repositories of
//! `fixtures/studio-lamp` (the preset's flows). The canon is
//! docs/canon/card-mandate.md "The rule".
//!
//! - M7, local: `POST .../cards` with `mandate: "open"` and a reviewer is a
//!   200 and stores it; without a reviewer a 409 with the line last of
//!   `notes`, nothing stored; `mandate: "maybe"` a 400. `POST
//!   .../cards/C-0002/edit {mandate: "binding", as: "Ana Ruiz"}` a 200 that
//!   adds exactly one `card.edited` and nothing else; `as: "Ben Ode"` a 409,
//!   byte for byte the CLI's exit-1 document; `mandate: null` leaves it.
//!   `POST .../cards/split` with `mandate: ["sound=open"]` a 200;
//!   `["voice=open"]` a 409. M: `mandate` missing from `CardEditBody` (a
//!   400).
//! - M7, a hub: any login creates and splits with a mandate; the edit with
//!   `mandate` is a 200 to the reviewer's login and a 409 to another login;
//!   an `as` naming another a 403 with nothing stored. M: the hub's edit
//!   acting as `local`.
//! - M6 (the daemon part): `GET .../cards/C-0001` is byte for byte `spec
//!   card show C-0001 --history --json`, `subtask_items` in it; `GET
//!   .../cards` byte-equal before and after items are raised on the
//!   children.
//!
//! `CARD_CALLS` 12 stays `door.rs`'s pin, unchanged. Every expected value is
//! worked out from the spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::path::{Path, PathBuf};

use common::hub::{Hub, Proxied, Setup, assert_refused, through};
use common::{Reply, Scratch, Server, spec_json};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};

/// The game's project.
const L: &str = "/api/projects/studio-lamp";
const ANA: &str = "Ana Ruiz";
const BEN: &str = "Ben Ode";

/// The first line of "Messages".
const NO_REVIEWER: &str =
    "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer";

/// A daemon over a repository of `fixtures/studio-lamp` in a `HOME` of its
/// own. `server` is first: it stops before the scratch goes.
struct Lamp {
    server: Server,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    _scratch: Scratch,
}

impl Lamp {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo("studio-lamp", "lamp", "main");
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
            _scratch: scratch,
        }
    }

    fn get(&self, path: &str) -> Reply {
        self.server.get(&format!("{L}/{path}"))
    }

    /// A same-origin page's `POST <L>/path` with the JSON `body`.
    fn post(&self, path: &str, body: &Value) -> Reply {
        self.server.stage(&format!("{L}/{path}"), &body.to_string())
    }

    /// As [`Lamp::post`], the answer a 200.
    fn done(&self, path: &str, body: &Value) -> Value {
        let reply = self.post(path, body);
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        reply.json()
    }

    fn show(&self, id: &str) -> Value {
        self.get(&format!("cards/{id}")).status(200).json()
    }

    /// The CLI's document of `args`, exit 0 or 1.
    fn cli(&self, args: &[&str]) -> common::Run {
        spec_json(&self.home, &self.cwd, &self.root, args)
    }

    fn events(&self) -> Vec<(String, Value)> {
        events_of(&self.home, &self.root)
    }

    /// The events and every card as a page reads them.
    fn state(&self) -> (Vec<(String, Value)>, String) {
        let board = self
            .get("cards?with_cancelled=true")
            .status(200)
            .text()
            .to_owned();
        (self.events(), board)
    }

    /// `path` with `body`: a 409 with `reason` last of `notes`, nothing
    /// changed.
    fn refused(&self, path: &str, body: &Value, reason: &str) -> Reply {
        let before = self.state();
        let reply = self.post(path, body);
        assert_eq!(reply.status, 409, "{path} {body}: {}", reply.text());
        let document = reply.json();
        assert_eq!(
            document["notes"].as_array().and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{path} {body}: {document}"
        );
        assert_eq!(self.state(), before, "{path} {body}: nothing changed");
        reply
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

/// M1's parent as a body.
fn parent_body() -> Value {
    json!({"title": "Lamp room scene", "discipline": "narrative", "target": ["SCN-ARRIVAL"],
           "as": ANA})
}

/// M1's hand-off as a body, `mandate` as given.
fn hand_off_body(mandate: Value) -> Value {
    json!({"title": "Keeper's voice", "discipline": "sound", "target": ["SCN-ARRIVAL"],
           "parent": "C-0001", "reviewer": ANA, "mandate": mandate, "as": ANA})
}

/// M2's split as a body under the stored `parent`, `mandate` as given.
fn split_body(mandate: Value) -> Value {
    json!({"parent": "C-0001", "child": ["sound"], "mandate": mandate, "as": ANA})
}

// ------------------------------------------------------------- local

/// M7, local: `POST .../cards` with `mandate: "open"` and a reviewer: 200,
/// stored, `card.created` carrying it; `null` and absent store none; without
/// a reviewer a 409 with the line, byte for byte the CLI's exit-1 document;
/// `"maybe"` and `"Open"` 400s, nothing stored.
#[test]
fn m7_post_cards_takes_a_mandate_with_a_reviewer() {
    let lamp = Lamp::new("cm-http-new");
    lamp.done("cards", &parent_body());
    let made = lamp.done("cards", &hand_off_body(json!("open")));
    assert_eq!(made["id"], json!("C-0002"), "{made}");
    assert_eq!(lamp.show("C-0002")["mandate"], json!("open"));
    assert_eq!(
        lamp.events().last().cloned(),
        Some((
            "card.created".to_owned(),
            json!({"id": "C-0002", "flow": "sound", "column": "backlog", "position": 1,
                   "assignee": null, "reviewer": ANA, "mandate": "open", "as": ANA})
        ))
    );
    lamp.done("cards", &hand_off_body(Value::Null));
    assert_eq!(lamp.show("C-0003")["mandate"], Value::Null);
    let mut absent = hand_off_body(Value::Null);
    absent.as_object_mut().expect("a body").remove("mandate");
    lamp.done("cards", &absent);
    assert_eq!(lamp.show("C-0004")["mandate"], Value::Null);
    assert_eq!(
        lamp.events()
            .last()
            .map(|(_, payload)| payload["mandate"].clone()),
        Some(Value::Null),
        "`mandate` null on every card.created"
    );

    let mut no_reviewer = hand_off_body(json!("binding"));
    no_reviewer["reviewer"] = Value::Null;
    let reply = lamp.refused("cards", &no_reviewer, NO_REVIEWER);
    let run = lamp.cli(&[
        "card",
        "new",
        "--title",
        "Keeper's voice",
        "--discipline",
        "sound",
        "--target",
        "SCN-ARRIVAL",
        "--parent",
        "C-0001",
        "--mandate",
        "binding",
        "--as",
        ANA,
    ]);
    run.code(1);
    assert_eq!(reply.text(), run.document(), "the CLI's document");

    for word in ["maybe", "Open", "-"] {
        let before = lamp.state();
        let reply = lamp.post("cards", &hand_off_body(json!(word)));
        assert_eq!(reply.status, 400, "{word}: {}", reply.text());
        assert_eq!(lamp.state(), before, "{word}: nothing stored");
    }
    assert_eq!(lamp.done("cards", &parent_body())["id"], json!("C-0005"));
}

/// M7, local: `POST .../cards/C-0002/edit {mandate: "binding", as: "Ana
/// Ruiz"}`: 200, exactly one new event, `card.edited` with `mandate`, and
/// nothing else; `as: "Ben Ode"` a 409, the CLI's exit-1 document; `mandate:
/// null` with a title leaves the mandate and keeps today's payload. M:
/// `mandate` missing from `CardEditBody` (a 400).
#[test]
fn m7_post_edit_changes_the_mandate_for_the_reviewer_alone() {
    let lamp = Lamp::new("cm-http-edit");
    lamp.done("cards", &parent_body());
    lamp.done("cards", &hand_off_body(json!("open")));
    let before = lamp.events();

    let other = format!(
        "mandate: only C-0002's reviewer `{ANA}` changes its mandate, not `{BEN}`; nothing \
         changed"
    );
    let reply = lamp.refused(
        "cards/C-0002/edit",
        &json!({"mandate": "binding", "as": BEN}),
        &other,
    );
    let run = lamp.cli(&[
        "card",
        "edit",
        "C-0002",
        "--mandate",
        "binding",
        "--as",
        BEN,
    ]);
    run.code(1);
    assert_eq!(reply.text(), run.document(), "the CLI's document");
    lamp.refused(
        "cards/C-0002/edit",
        &json!({"title": "X", "mandate": "binding", "as": BEN}),
        &other,
    );

    let reply = lamp.post(
        "cards/C-0002/edit",
        &json!({"mandate": "binding", "as": ANA}),
    );
    assert_eq!(
        reply.status,
        200,
        "M: `mandate` missing from CardEditBody: {}",
        reply.text()
    );
    assert_eq!(reply.json()["revision"], json!("2"));
    let after = lamp.events();
    assert_eq!(after.len(), before.len() + 1, "one event and nothing else");
    assert_eq!(after[..before.len()], before[..]);
    assert_eq!(
        after[before.len()],
        (
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": ANA, "mandate": "binding"})
        )
    );
    assert_eq!(lamp.show("C-0002")["mandate"], json!("binding"));

    // `null` or absent: unchanged, today's payload.
    lamp.done(
        "cards/C-0002/edit",
        &json!({"title": "The keeper's voice", "mandate": null, "as": BEN}),
    );
    assert_eq!(lamp.show("C-0002")["mandate"], json!("binding"));
    assert_eq!(
        lamp.events().last().cloned(),
        Some((
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["title"], "as": BEN})
        ))
    );
    let reply = lamp.post("cards/C-0002/edit", &json!({"mandate": "maybe", "as": ANA}));
    assert_eq!(reply.status, 400, "{}", reply.text());
}

/// M7, local: `POST .../cards/split` with `mandate: ["sound=open"]`: 200,
/// the child holding it, `mandate` last of the child; `["voice=open"]` a
/// 409 with the line; `[]` and `null` name none; a `mandate` with no
/// `child` a 400.
#[test]
fn m7_post_split_takes_a_mandate_per_child() {
    let lamp = Lamp::new("cm-http-split");
    lamp.done("cards", &parent_body());
    let made = lamp.done("cards/split", &split_body(json!(["sound=open"])));
    assert_eq!(made["children"][0]["id"], json!("C-0002"), "{made}");
    assert_eq!(made["children"][0]["mandate"], json!("open"), "{made}");
    let keys: Vec<&String> = made["children"][0]
        .as_object()
        .expect("a child")
        .keys()
        .collect();
    assert_eq!(keys.len(), 9, "{made}");
    assert_eq!(lamp.show("C-0002")["mandate"], json!("open"));

    lamp.refused(
        "cards/split",
        &split_body(json!(["voice=open"])),
        "mandate: `voice` is no --child of this split",
    );
    lamp.refused(
        "cards/split",
        &split_body(json!(["sound=open", "sound=binding"])),
        "mandate: `sound` is named twice",
    );
    for none in [json!([]), Value::Null] {
        let made = lamp.done("cards/split", &split_body(none.clone()));
        assert_eq!(made["children"][0]["mandate"], Value::Null, "{none}");
    }
    let before = lamp.state();
    let reply = lamp.post(
        "cards/split",
        &json!({"parent": "C-0001", "mandate": ["sound=open"], "as": ANA}),
    );
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert_eq!(lamp.state(), before);
}

/// M6, the daemon: `GET .../cards/C-0001` is byte for byte `spec card show
/// C-0001 --history --json`, its children's items in `subtask_items`; `GET
/// .../cards` byte-equal before and after the items.
#[test]
fn m6_get_card_serves_subtask_items_as_the_cli() {
    let lamp = Lamp::new("cm-http-read");
    lamp.done("cards", &parent_body());
    lamp.done("cards", &hand_off_body(json!("open")));
    let mut binding = hand_off_body(json!("binding"));
    binding["discipline"] = json!("art");
    lamp.done("cards", &binding);
    let board = lamp.get("cards").status(200).text().to_owned();
    let all = lamp
        .get("cards?with_cancelled=true")
        .status(200)
        .text()
        .to_owned();
    for (card, kind, text, why) in [
        (
            "C-0002",
            "decision",
            "Does the keeper sing in the storm?",
            Value::Null,
        ),
        (
            "C-0003",
            "decision",
            "Is the keeper's coat oilskin?",
            json!("The brief names wool."),
        ),
        (
            "C-0002",
            "question",
            "Which accent does the keeper speak?",
            Value::Null,
        ),
    ] {
        let body = json!({"kind": kind, "node_ids": ["SCN-STORM"], "text": text,
            "working_answer": "Yes.", "price_of_other": "A scene redrawn.", "why": why,
            "severity": null, "distinct_from": null, "as": ANA});
        let raised = lamp.done(&format!("cards/{card}/items"), &body);
        assert_eq!(raised["created"], json!(true), "{raised}");
    }
    let reply = lamp.get("cards/C-0001");
    assert_eq!(reply.status, 200, "{}", reply.text());
    let run = lamp.cli(&["card", "show", "C-0001", "--history"]);
    run.code(0);
    assert_eq!(reply.text(), run.document(), "the CLI's document");
    let listed: Vec<(Value, Value)> = reply.json()["subtask_items"]
        .as_array()
        .expect("subtask_items")
        .iter()
        .map(|item| (item["card_id"].clone(), item["id"].clone()))
        .collect();
    assert_eq!(
        listed,
        [
            (json!("C-0002"), json!("PR-0001")),
            (json!("C-0003"), json!("PR-0002")),
            (json!("C-0002"), json!("PR-0003")),
        ]
    );
    assert_eq!(lamp.show("C-0002")["subtask_items"], json!([]));
    assert_eq!(lamp.get("cards").status(200).text(), board);
    assert_eq!(
        lamp.get("cards?with_cancelled=true").status(200).text(),
        all
    );
}

// ---------------------------------------------------------------- a hub

/// A hub over studio-lamp, no `[team]`: every login writes plainly.
fn hub(name: &str) -> Hub {
    Setup::new(name, "studio-lamp").start()
}

/// `POST <L>/path` with `body` by `login` through the hub's proxy.
fn hub_post(hub: &Hub, login: &str, path: &str, body: &Value) -> Reply {
    through(
        &hub.server,
        "POST",
        &format!("{L}/{path}"),
        &Proxied::write(login),
        body.to_string().as_bytes(),
    )
}

fn hub_done(hub: &Hub, login: &str, path: &str, body: &Value) -> Value {
    let reply = hub_post(hub, login, path, body);
    assert_eq!(reply.status, 200, "{login} {path} {body}: {}", reply.text());
    reply.json()
}

/// The events and every card as a page reads them, through the hub.
fn hub_board(hub: &Hub) -> (Vec<(String, Value)>, String) {
    let board = hub
        .get("mira", &format!("{L}/cards?with_cancelled=true"))
        .status(200)
        .text()
        .to_owned();
    (hub.events(), board)
}

fn hub_show(hub: &Hub, id: &str) -> Value {
    hub.get("mira", &format!("{L}/cards/{id}"))
        .status(200)
        .json()
}

/// M7, a hub: `eve` creates a hand-off reviewed by `mira` with a mandate,
/// and splits with one; the edit with `mandate` is a 200 to `mira`'s login,
/// a 409 to `eve`'s; `as` naming another a 403 with nothing stored. M: the
/// hub's edit acting as `local` (`mira` refused).
#[test]
fn m7_a_hub_judges_the_mandate_edit_by_the_login() {
    let hub = hub("cm-hub");
    hub_done(
        &hub,
        "eve",
        "cards",
        &json!({"title": "Lamp room scene", "discipline": "narrative",
                "target": ["SCN-ARRIVAL"]}),
    );
    hub_done(
        &hub,
        "eve",
        "cards",
        &json!({"title": "Keeper's voice", "discipline": "sound", "target": ["SCN-ARRIVAL"],
                "parent": "C-0001", "reviewer": "mira", "mandate": "open"}),
    );
    assert_eq!(hub_show(&hub, "C-0002")["mandate"], json!("open"));
    let created = hub.events().last().cloned().expect("an event");
    assert_eq!(created.1["mandate"], json!("open"));
    assert_eq!(created.1["as"], json!("eve"));
    let split = hub_done(
        &hub,
        "eve",
        "cards/split",
        &json!({"parent": "C-0001", "child": ["art"], "mandate": ["art=binding"],
                "reviewer": "mira"}),
    );
    assert_eq!(split["children"][0]["mandate"], json!("binding"), "{split}");

    let before = hub_board(&hub);
    let reply = hub_post(
        &hub,
        "eve",
        "cards/C-0002/edit",
        &json!({"mandate": "binding"}),
    );
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(
        reply.json()["notes"]
            .as_array()
            .and_then(|notes| notes.last())
            .cloned(),
        Some(json!(
            "mandate: only C-0002's reviewer `mira` changes its mandate, not `eve`; nothing \
             changed"
        ))
    );
    assert_eq!(hub_board(&hub), before, "nothing stored");
    assert_refused(
        &hub_post(
            &hub,
            "eve",
            "cards/C-0002/edit",
            &json!({"mandate": "binding", "as": "mira"}),
        ),
        "refused: `as` names mira; this request's login is eve",
        "as naming another",
    );
    assert_eq!(hub_board(&hub), before, "nothing stored");

    let edited = hub_post(
        &hub,
        "mira",
        "cards/C-0002/edit",
        &json!({"mandate": "binding"}),
    );
    assert_eq!(
        edited.status,
        200,
        "M: the hub's edit acting as `local`: {}",
        edited.text()
    );
    assert_eq!(hub_show(&hub, "C-0002")["mandate"], json!("binding"));
    assert_eq!(
        hub.events().last().cloned(),
        Some((
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": "mira", "mandate": "binding"})
        ))
    );
}

/// M7, a hub's `/mcp`: `create_card` with a reviewer and a mandate stores
/// it as the login; `edit_card {card_id, mandate}` is an error result for
/// another login and stands for the reviewer's, acting as that login. M:
/// the hub's edit acting as `local`.
#[test]
fn m7_a_hubs_mcp_edit_card_acts_as_the_login() {
    use common::mcp_http::{Session, content_text, is_error, result_of};
    let hub = hub("cm-hub-mcp");
    let mut eve = Session::open(hub.server.port, "studio-lamp", "eve");
    let made = eve.call(
        "create_card",
        json!({"title": "Lamp room scene", "discipline": "narrative",
               "target": ["SCN-ARRIVAL"]}),
    );
    assert!(!is_error(result_of(&made, "create_card")), "{made}");
    let made = eve.call(
        "create_card",
        json!({"title": "Keeper's voice", "discipline": "sound", "target": ["SCN-ARRIVAL"],
               "parent": "C-0001", "reviewer": "mira", "mandate": "open"}),
    );
    let answer = result_of(&made, "create_card");
    assert!(!is_error(answer), "{}", content_text(answer));
    let created = hub.events().last().cloned().expect("an event");
    assert_eq!(created.1["mandate"], json!("open"));
    assert_eq!(created.1["as"], json!("eve"));

    let before = hub_board(&hub);
    let refused = eve.call(
        "edit_card",
        json!({"card_id": "C-0002", "mandate": "binding"}),
    );
    let answer = result_of(&refused, "edit_card by eve");
    assert!(is_error(answer), "{refused}");
    assert!(
        content_text(answer).contains(
            "mandate: only C-0002's reviewer `mira` changes its mandate, not `eve`; nothing \
             changed"
        ),
        "{}",
        content_text(answer)
    );
    assert_eq!(hub_board(&hub), before, "nothing stored");

    let mut mira = Session::open(hub.server.port, "studio-lamp", "mira");
    let edited = mira.call(
        "edit_card",
        json!({"card_id": "C-0002", "mandate": "binding"}),
    );
    let answer = result_of(&edited, "edit_card by mira");
    assert!(
        !is_error(answer),
        "M: the hub's edit acting as `local`: {}",
        content_text(answer)
    );
    assert_eq!(hub_show(&hub, "C-0002")["mandate"], json!("binding"));
    assert_eq!(
        hub.events().last().cloned(),
        Some((
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": "mira", "mandate": "binding"})
        ))
    );
}
