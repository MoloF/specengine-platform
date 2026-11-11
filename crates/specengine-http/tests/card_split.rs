//! docs/features/department-split.md (RM-0100, v5 step 6c, ADR-0078)
//! "Acceptance criteria", S7, the daemon's half: `POST .../cards/split`
//! through the real daemon, local and as a hub, over git repositories of
//! `fixtures/card-press` (a print shop with two `[[card.split]]` rows). The
//! canon is docs/canon/card-split.md "The rule".
//!
//! - Local: S3's split as a body is a 200 with the split document, three
//!   `card.created` and nothing else; a dry run is a 200, byte for byte the
//!   CLI's `--dry-run` document, nothing stored; a `child` no flow takes is a
//!   409 with the reason last of `notes`, nothing stored; an unknown key and
//!   a bad `as` are 400s; a bad `[card]` a 503 with the CLI's line; GET is a
//!   405 `Allow: POST`; `GET .../cards/C-0001` still answers the card.
//! - A hub: a login outside `[team]` splits (200); `as` absent records the
//!   login on every `card.created`, and the reviewer the chain ends at is the
//!   login; `as` naming another is the plain class's 403, a dry run's too,
//!   nothing stored. M: the arm left out of `class` (403); `acting` not
//!   called (the event's `as` `local`).
//!
//! The door's half (`CARD_CALLS` 12, `card_writes_touch_only_the_queue`) is
//! `door.rs`; `class`'s own arms are `src/hub.rs`'s unit tests; `Allow` on
//! every method `methods.rs`. Every expected value is worked out from the
//! spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::path::{Path, PathBuf};

use common::hub::{Setup, assert_refused};
use common::{Reply, Scratch, Server, read_text, replace, spec, spec_json, write};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};

const P: &str = "/api/projects/card-press";
const ANA: &str = "Ana Ruiz";
const TITLE: &str = "The weekly run";

/// `PRN-WEEKLY`'s scope as `spec tree PRN-WEEKLY --depth 2` serves it.
const SCOPE: [(&str, &str); 8] = [
    ("PRN-WEEKLY", "print-run"),
    ("PRN-WEEKLY-DEADLINE", "print-run"),
    ("PRN-PLATES", "print-run"),
    ("SHT-BACK", "broadsheet"),
    ("SHT-FRONT", "broadsheet"),
    ("SHT-FRONT-HEAD", "broadsheet"),
    ("SHT-INNER", "broadsheet"),
    ("SHT-INNER-FOLD", "broadsheet"),
];

fn of_kinds(kinds: &[&str]) -> Vec<&'static str> {
    SCOPE
        .iter()
        .filter(|(_, kind)| kinds.contains(kind))
        .map(|(id, _)| *id)
        .collect()
}

/// S3's split as a body.
fn weekly() -> Value {
    json!({"target": ["PRN-WEEKLY"], "title": TITLE, "goal": null, "discipline": "copy",
           "parent": null, "child": ["copy", "plates"], "reviewer": ANA, "dry_run": false,
           "as": ANA})
}

/// S3's split document, `id` the parent's (the children follow it).
fn written(first: u32, created: bool) -> Value {
    let id = |at: u32| json!(format!("C-{at:04}"));
    json!({
        "dry_run": false,
        "parent": {"id": id(first), "created": created, "title": TITLE, "discipline": "copy",
                   "flow": "copy", "flow_title": "Copy desk", "column": "draft",
                   "targets": ["PRN-WEEKLY"]},
        "children": [
            {"id": id(first + 1), "discipline": "copy", "flow": "copy",
             "flow_title": "Copy desk", "column": "draft",
             "targets": of_kinds(&["broadsheet"]), "from": "kinds", "suggested": true},
            {"id": id(first + 2), "discipline": "plates", "flow": "plates",
             "flow_title": "plates", "column": "queued",
             "targets": of_kinds(&["broadsheet", "print-run"]), "from": "kinds",
             "suggested": true}
        ],
        "reviewer": ANA,
        "notes": []
    })
}

/// A daemon over a repository of `fixtures/card-press` in a `HOME` of its
/// own. `server` is first: it stops before the scratch goes.
struct Press {
    server: Server,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    _scratch: Scratch,
}

impl Press {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo("card-press", "press", "main");
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
        self.server.get(&format!("{P}/{path}"))
    }

    /// A same-origin page's `POST <P>/path` with the JSON `body`.
    fn post(&self, path: &str, body: &Value) -> Reply {
        self.server.stage(&format!("{P}/{path}"), &body.to_string())
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

fn created(id: &str, flow: &str, column: &str, position: u64, reviewer: &str, by: &str) -> Value {
    json!({"id": id, "flow": flow, "column": column, "position": position,
           "assignee": null, "reviewer": reviewer, "as": by})
}

/// The `card.created` payloads of `events`.
fn created_payloads(events: &[(String, Value)]) -> Vec<Value> {
    events
        .iter()
        .filter(|(kind, _)| kind == "card.created")
        .map(|(_, payload)| payload.clone())
        .collect()
}

// ------------------------------------------------------------- local

/// S7, local: S3's split as a body: 200, the split document, exactly three
/// `card.created` (the parent's first, `as` Ana Ruiz) and no other event;
/// `GET .../cards/C-0001` still answers the card, its children under it.
#[test]
fn s7_a_split_body_is_a_200_with_the_document() {
    let press = Press::new("split-route");
    let reply = press.post("cards/split", &weekly());
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8")
    );
    assert_eq!(reply.json(), written(1, true));
    let events = press.events();
    let kinds: Vec<&str> = events.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(kinds, ["card.created"; 3], "{events:?}");
    assert_eq!(
        created_payloads(&events),
        [
            created("C-0001", "copy", "draft", 1, ANA, ANA),
            created("C-0002", "copy", "draft", 2, ANA, ANA),
            created("C-0003", "plates", "queued", 1, ANA, ANA),
        ]
    );
    let card = press.get("cards/C-0001").status(200).json();
    assert_eq!(card["id"], json!("C-0001"));
    assert_eq!(card["title"], json!(TITLE));
    for id in ["C-0002", "C-0003"] {
        let child = press.get(&format!("cards/{id}")).status(200).json();
        assert_eq!(child["parent"], json!("C-0001"), "{child}");
        assert_eq!(child["reviewer"], json!(ANA), "{child}");
    }
    // A second split of the same document: no dedup.
    let again = press.post("cards/split", &weekly());
    assert_eq!(again.status, 200, "{}", again.text());
    assert_eq!(again.json(), written(4, true));
}

/// S7, local: a dry run is a 200, byte for byte the CLI's `--dry-run`
/// document, and stores nothing; `dry_run` `null` is false.
#[test]
fn s7_a_dry_run_is_a_200_and_stores_nothing() {
    let press = Press::new("split-route-dry");
    let before = press.state();
    let mut body = weekly();
    body["dry_run"] = json!(true);
    let reply = press.post("cards/split", &body);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let cli = spec_json(
        &press.home,
        &press.cwd,
        &press.root,
        &[
            "card",
            "split",
            "--target",
            "PRN-WEEKLY",
            "--title",
            TITLE,
            "--discipline",
            "copy",
            "--child",
            "copy",
            "--child",
            "plates",
            "--reviewer",
            ANA,
            "--as",
            ANA,
            "--dry-run",
        ],
    );
    cli.code(0);
    assert_eq!(reply.text(), cli.document(), "the CLI's document");
    let document = reply.json();
    assert_eq!(document["dry_run"], json!(true));
    assert_eq!(document["parent"]["id"], Value::Null);
    assert_eq!(press.state(), before, "nothing stored");
    // `dry_run` null and absent are false: a write.
    let mut body = weekly();
    body["dry_run"] = Value::Null;
    let reply = press.post("cards/split", &body);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json(), written(1, true));
}

/// S7, local: a `child` no flow takes is a 409 with the split's refusal
/// document, the reason last of `notes`, byte for byte the CLI's exit-1
/// document; an unknown key and a bad `as` are 400s; nothing stored.
#[test]
fn s7_refusals_are_a_409_and_a_400() {
    let press = Press::new("split-route-refused");
    let before = press.state();
    let mut body = weekly();
    body["child"] = json!(["copy", "nowhere"]);
    let reply = press.post("cards/split", &body);
    assert_eq!(reply.status, 409, "{}", reply.text());
    let reason = "child: no flow takes the discipline `nowhere`: none lists it, and every \
                  `[[card.flow]]` lists its disciplines";
    assert_eq!(
        reply.json(),
        json!({"dry_run": false, "parent": null, "children": [], "reviewer": null,
               "notes": [reason]})
    );
    let cli = spec_json(
        &press.home,
        &press.cwd,
        &press.root,
        &[
            "card",
            "split",
            "--target",
            "PRN-WEEKLY",
            "--title",
            TITLE,
            "--discipline",
            "copy",
            "--child",
            "copy",
            "--child",
            "nowhere",
            "--reviewer",
            ANA,
            "--as",
            ANA,
        ],
    );
    cli.code(1);
    assert_eq!(reply.text(), cli.document(), "the CLI's exit-1 document");
    assert_eq!(press.state(), before, "nothing stored");
    let mut parentless = weekly();
    parentless["target"] = Value::Null;
    parentless["title"] = Value::Null;
    parentless["discipline"] = Value::Null;
    parentless["parent"] = json!("C-0099");
    let reply = press.post("cards/split", &parentless);
    assert_eq!(reply.status, 409, "a missing parent: {}", reply.text());
    assert_eq!(
        reply.json()["notes"]
            .as_array()
            .and_then(|notes| notes.last()),
        Some(&json!("parent: no card C-0099"))
    );
    for (label, body) in [
        ("an unknown key", {
            let mut body = weekly();
            body["owner"] = json!("ana");
            body
        }),
        ("a bad `as`", {
            let mut body = weekly();
            body["as"] = json!("bell\u{7}");
            body
        }),
        ("a parent beside a title", {
            let mut body = weekly();
            body["parent"] = json!("C-0001");
            body
        }),
        ("no child, no dry run", {
            let mut body = weekly();
            body["child"] = json!([]);
            body
        }),
    ] {
        let reply = press.post("cards/split", &body);
        assert_eq!(reply.status, 400, "{label}: {}", reply.text());
        assert_eq!(press.state(), before, "{label}: nothing stored");
    }
}

/// S7, local: a bad `[card]` is a 503 with the CLI's line; GET on the
/// route is a 405 `Allow: POST`, and `GET .../cards/C-0001` still reads.
#[test]
fn s7_a_bad_card_table_is_a_503_and_get_a_405() {
    let press = Press::new("split-route-503");
    let made = press.post(
        "cards",
        &json!({"title": "Lead story", "discipline": "copy"}),
    );
    assert_eq!(made.status, 200, "{}", made.text());
    for method in ["GET", "PUT", "DELETE", "PATCH"] {
        let reply = press
            .server
            .request(method, &format!("{P}/cards/split"), &[]);
        assert_eq!(reply.status, 405, "{method}: {}", reply.text());
        assert_eq!(reply.header("allow"), Some("POST"), "{method}");
    }
    assert_eq!(
        press.get("cards/C-0001").status(200).json()["id"],
        json!("C-0001")
    );
    let before = press.events();
    replace(
        &press.root,
        "specengine.toml",
        "kind = \"print-run\"\ndisciplines = [\"plates\"]",
        "kind = \"print-run\"\ndisciplines = [\"voice\"]",
    );
    let line = press.cannot(&["card", "list"]);
    assert!(line.contains("`voice`"), "{line}");
    let reply = press.post("cards/split", &weekly());
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line);
    assert_eq!(press.events(), before, "nothing stored");
    // The rest of the project reads on.
    assert_eq!(press.get("tree").status, 200);
}

// ---------------------------------------------------------------- a hub

/// `mira` alone in `[team]`: `eve` is outside it.
const TEAM: &str = "\n[team.mira]\nname = \"Mira Vale\"\nrole = \"owner\"\nemail = \
                    \"mira@example.org\"\n";

/// A hub over card-press with [`TEAM`] committed.
fn teamed(name: &str) -> common::hub::Hub {
    let setup = Setup::new(name, "card-press");
    let mut text = read_text(&setup.root, "specengine.toml");
    text.push_str(TEAM);
    write(&setup.root, "specengine.toml", text);
    setup
        .scratch
        .git()
        .run(&setup.root, &["commit", "-qam", "the team"]);
    setup.start()
}

/// S7, a hub: `eve`, outside `[team]`, splits with no `as` and no
/// `reviewer`: 200, every `card.created` under `eve`, the reviewer the
/// chain's acting name, the login. M: the arm left out of `class` (a 403);
/// `acting` not called (`as` `local`).
#[test]
fn s7_a_hub_login_outside_the_team_splits_as_itself() {
    let hub = teamed("split-hub");
    let mut body = weekly();
    body["as"] = Value::Null;
    body["reviewer"] = Value::Null;
    let reply = hub.post("eve", "cards/split", &body);
    assert_eq!(
        reply.status,
        200,
        "M: the arm left out of class: {}",
        reply.text()
    );
    let document = reply.json();
    assert_eq!(document["reviewer"], json!("eve"), "{document}");
    assert_eq!(
        created_payloads(&hub.events()),
        [
            created("C-0001", "copy", "draft", 1, "eve", "eve"),
            created("C-0002", "copy", "draft", 2, "eve", "eve"),
            created("C-0003", "plates", "queued", 1, "eve", "eve"),
        ],
        "M: `acting` not called"
    );
    // `as` absent altogether: the login too.
    let mut body = weekly();
    body.as_object_mut().expect("an object").remove("as");
    let reply = hub.post("eve", "cards/split", &body);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let payloads = created_payloads(&hub.events());
    assert!(
        payloads[3..].iter().all(|payload| payload["as"] == "eve"),
        "{payloads:?}"
    );
}

/// S7, a hub: `as` naming another is the plain class's 403, a dry run's
/// too, nothing stored.
#[test]
fn s7_a_hub_refuses_another_name() {
    let hub = teamed("split-hub-other");
    let before = hub.board();
    for dry in [false, true] {
        let mut body = weekly();
        body["as"] = json!("tomas");
        body["dry_run"] = json!(dry);
        assert_refused(
            &hub.post("eve", "cards/split", &body),
            "refused: `as` names tomas; this request's login is eve",
            &format!("dry_run {dry}"),
        );
        assert_eq!(hub.board(), before, "dry_run {dry}: nothing stored");
    }
    // The login's own name passes, a dry run storing nothing.
    let mut body = weekly();
    body["as"] = json!("eve");
    body["dry_run"] = json!(true);
    let reply = hub.post("eve", "cards/split", &body);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(hub.board(), before);
}

/// S8, the wire's half: a hub's `/mcp` lists eighteen tools, `split_card`
/// among them, and its `split_card` acts as the login: every `card.created`
/// under `eve`, the reviewer the login when none is named. M: `split_card`
/// missing from `HUB_TOOLS`.
#[test]
fn s8_a_hubs_split_card_acts_as_the_login() {
    use common::mcp_http::{Session, content_text, is_error, result_of, tool_names};
    let hub = teamed("split-hub-mcp");
    let mut client = Session::open(hub.server.port, "card-press", "eve");
    let names = tool_names(&client.tools());
    assert_eq!(names.len(), 18, "{names:?}");
    assert!(
        names.iter().any(|name| name == "split_card"),
        "M: split_card missing from HUB_TOOLS: {names:?}"
    );
    let response = client.call(
        "split_card",
        json!({"target": ["PRN-WEEKLY"], "title": TITLE, "discipline": "copy",
               "child": ["copy", "plates"]}),
    );
    let result = result_of(&response, "split_card");
    assert!(!is_error(result), "{}", content_text(result));
    let document = &result["structuredContent"];
    assert_eq!(document["reviewer"], json!("eve"), "{document}");
    assert_eq!(document["parent"]["id"], json!("C-0001"), "{document}");
    assert_eq!(
        created_payloads(&hub.events()),
        [
            created("C-0001", "copy", "draft", 1, "eve", "eve"),
            created("C-0002", "copy", "draft", 2, "eve", "eve"),
            created("C-0003", "plates", "queued", 1, "eve", "eve"),
        ]
    );
    // `as` is no argument of a hub tool.
    let before = hub.events();
    let refused = client.call(
        "split_card",
        json!({"target": ["PRN-WEEKLY"], "title": TITLE, "child": ["copy"],
               "as": "tomas"}),
    );
    assert!(
        refused.get("error").is_some() || is_error(result_of(&refused, "split_card as")),
        "{refused}"
    );
    assert_eq!(hub.events(), before, "nothing stored");
}
