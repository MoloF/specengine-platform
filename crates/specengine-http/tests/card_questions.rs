//! docs/features/card-questions.md, the daemon's half (RM-0097, ADR-0077):
//! the card door `POST /api/projects/:p/cards/:id/items` and the stage's
//! `as`, through the real daemon over a git repository of
//! `fixtures/studio-lamp` (a lighthouse game, five trades' flows), and of
//! `fixtures/spec-a` for the stage.
//!
//! - L3, raises: a question with `as: "dev"` is a 200, `created`, the intake
//!   document's seven keys in order; its review holds `card_id` and the
//!   human author; `proposal.created` is `{"id":"PR-0001","as":"dev"}`;
//!   `as: null` logs `"as":"local"`; a decision's `why` is stored; every
//!   other raise's `proposal.created` stays `{"id":…}`.
//! - L3, refusals, nothing stored: `C-0099` a 404 `{id, reason}` and no
//!   event, before any intake rule; the same question again a 200 not
//!   stored, its hit `PR-0001`, then stored naming it; a cap a 409, the
//!   reason last of `notes`; an unknown key, `kind: "update"` and `why` on a
//!   question 400s; a look-alike node a 503; non-JSON a 415; no same-origin
//!   page a 403; GET a 405 `Allow: POST`. M: the handler calling
//!   `propose_question`; the event without `as`; the card not read first.
//! - L4, the daemon: a local stage posted with `as: "ana"` logs
//!   `proposal.staged` ending `"as":"ana"`; without `as` or with `null` its
//!   payload is exactly `{"id","staged","staged_at"}`; a round answer's stage
//!   takes `as` per stage; `as` off the person grammar a 400; `spec approve`
//!   confirms the stage as before. M: `"as":null` written; the `as` stored in
//!   a column.
//!
//! The door's scan and `card_writes_touch_only_the_queue` are `door.rs`; a
//! hub's plain write and stage `hub_team.rs`. Every expected value below is
//! worked out from the spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Reply, Scratch, Server, ask, data_dir, spec_json};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use specengine_cli::{Env, EventLine, Globals, events_after};

const LAMP: &str = "/api/projects/studio-lamp";
const SPEC_A: &str = "/api/projects/lantern-keep";

/// "Data"'s question of the card door.
const BELL: &str = "Which bell rings at dusk?";
const BELL_ANSWER: &str = "The harbour bell, as in chapter one.";
const BELL_PRICE: &str = "Two scenes rewritten.";

/// The intake document's keys, in order ("Data", "The card door").
const INTAKE_KEYS: [&str; 7] = [
    "id",
    "created",
    "hits",
    "related",
    "linked",
    "diagnostics",
    "notes",
];

/// A JSON object's keys, in the order written.
struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> Visitor<'de> for Seen {
            type Value = Keys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

fn keys_of(json: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(json)
        .unwrap_or_else(|error| panic!("{error}: {json}"))
        .0
}

/// A daemon over a repository of `fixture` in a `HOME` of its own, an
/// identity in the repository so a write could commit. `server` is first:
/// it stops before the scratch goes.
struct Daemon {
    server: Server,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    base: &'static str,
    slug: &'static str,
    _scratch: Scratch,
}

impl Daemon {
    fn new(name: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo(fixture, "repo", "main");
        let git = scratch.git();
        git.run(&root, &["config", "user.name", "Owner"]);
        git.run(&root, &["config", "user.email", "owner@example.invalid"]);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let server = Server::serve(&home, &cwd, &[&root]);
        let (base, slug) = if fixture == "spec-a" {
            (SPEC_A, "lantern-keep")
        } else {
            (LAMP, "studio-lamp")
        };
        Self {
            server,
            root,
            home,
            cwd,
            base,
            slug,
            _scratch: scratch,
        }
    }

    /// studio-lamp with `C-0001`, a narrative card on `SCN-STORM`.
    fn lamp(name: &str) -> Self {
        let daemon = Self::new(name, "studio-lamp");
        let made = daemon.cli(&[
            "card",
            "new",
            "--title",
            "The storm scene",
            "--target",
            "SCN-STORM",
            "--discipline",
            "narrative",
        ]);
        made.code(0);
        assert_eq!(made.json()["id"], json!("C-0001"));
        daemon
    }

    fn cli(&self, args: &[&str]) -> common::Run {
        spec_json(&self.home, &self.cwd, &self.root, args)
    }

    fn get(&self, path: &str) -> Reply {
        self.server.get(&format!("{}/{path}", self.base))
    }

    /// A same-origin page's `POST <base>/path` with `body`.
    fn post(&self, path: &str, body: &Value) -> Reply {
        self.server
            .stage(&format!("{}/{path}", self.base), &body.to_string())
    }

    /// `POST <base>/cards/<card>/items`.
    fn raise(&self, card: &str, body: &Value) -> Reply {
        self.post(&format!("cards/{card}/items"), body)
    }

    /// The project's events as stored (raw payloads).
    fn events(&self) -> Vec<EventLine> {
        let env = Env {
            cwd: self.root.clone(),
            home: Some(self.home.as_os_str().to_os_string()),
            xdg_data_home: None,
        };
        let globals = Globals {
            root: Some(self.root.clone()),
            config: None,
        };
        events_after(&env, &globals, Some(0))
            .expect("the events read")
            .events
    }

    /// `(type, payload)` of every event, raw.
    fn event_lines(&self) -> Vec<(String, String)> {
        self.events()
            .into_iter()
            .map(|event| (event.event_type, event.payload))
            .collect()
    }

    fn db(&self) -> PathBuf {
        data_dir(&self.home).join(format!("{}.db", self.slug))
    }

    fn sql(&self, sql: &str) -> String {
        sqlite3(&self.db(), sql)
    }

    /// The review document of `id` as the daemon serves it.
    fn review(&self, id: &str) -> Value {
        self.get(&format!("proposals/{id}")).status(200).json()
    }
}

/// `sql` on `db` through the system's `sqlite3` (a 2 s busy timeout).
fn sqlite3(db: &Path, sql: &str) -> String {
    let output = Command::new("/usr/bin/sqlite3")
        .env_clear()
        .args(["-cmd", ".timeout 2000"])
        .arg(db)
        .arg(sql)
        .output()
        .expect("run sqlite3");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("sqlite3 output")
        .trim()
        .to_owned()
}

/// "Data"'s question body of the card door, every key, on `SCN-STORM`.
fn question(text: &str, actor: Value) -> Value {
    json!({"kind": "question", "node_ids": ["SCN-STORM"], "text": text,
        "working_answer": BELL_ANSWER, "price_of_other": BELL_PRICE,
        "why": null, "severity": null, "distinct_from": null, "as": actor})
}

/// A decision body with `why`, on `SCN-STORM`.
fn decision(text: &str, why: Value) -> Value {
    json!({"kind": "decision", "node_ids": ["SCN-STORM"], "text": text,
        "working_answer": "Yes: whale oil, as the keeper's log says.",
        "price_of_other": "The lamp room is redrawn.",
        "why": why, "severity": null, "distinct_from": null, "as": "dev"})
}

/// The last event's `(type, payload)`.
fn last(daemon: &Daemon) -> (String, String) {
    daemon.event_lines().pop().expect("an event")
}

// ------------------------------------------------------------------ L3

/// L3, raises: a question with `as: "dev"` is a 200, the intake document
/// (seven keys in order), `created`; the review holds `card_id` `C-0001`
/// and the human author with no login; `proposal.created` is exactly
/// `{"id":"PR-0001","as":"dev"}`. `as: null` logs `"as":"local"`. A decision
/// with `why` stores it. A CLI raise's `proposal.created` stays `{"id":…}`.
/// M: the event without `as`.
#[test]
fn l3_a_question_and_a_decision_raised_on_a_card_store_it() {
    let lamp = Daemon::lamp("cq-l3-raise");
    let reply = lamp.raise("C-0001", &question(BELL, json!("dev")));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(keys_of(reply.text()), INTAKE_KEYS, "{}", reply.text());
    let document = reply.json();
    assert_eq!(document["id"], json!("PR-0001"), "{document}");
    assert_eq!(document["created"], json!(true), "{document}");
    let review = lamp.review("PR-0001");
    assert_eq!(review["card_id"], json!("C-0001"), "{review}");
    assert_eq!(review["kind"], json!("question"));
    assert_eq!(
        review["author"],
        json!({"type": "human", "role": null, "model": null, "run": null}),
        "{review}"
    );
    assert_eq!(
        last(&lamp),
        (
            "proposal.created".to_owned(),
            "{\"id\":\"PR-0001\",\"as\":\"dev\"}".to_owned()
        )
    );

    let reply = lamp.raise(
        "C-0001",
        &question("Who meets the keeper at the pier?", Value::Null),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["id"], json!("PR-0002"));
    assert_eq!(
        last(&lamp).1,
        "{\"id\":\"PR-0002\",\"as\":\"local\"}",
        "`null` reads `local`"
    );

    let why = "Two playtests read instant regeneration as a bug.";
    let reply = lamp.raise("C-0001", &decision("Does the lamp burn oil?", json!(why)));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["id"], json!("PR-0003"));
    let review = lamp.review("PR-0003");
    assert_eq!(review["why"], json!(why), "{review}");
    assert_eq!(review["card_id"], json!("C-0001"));
    assert_eq!(review["kind"], json!("decision"));

    // A raise elsewhere keeps its event.
    let other = ask(
        &lamp.home,
        &lamp.root,
        &["ISL-LAMP"],
        "How many steps has the stair?",
    );
    assert_eq!(other, "PR-0004");
    assert_eq!(last(&lamp).1, "{\"id\":\"PR-0004\"}");
    assert_eq!(lamp.review("PR-0004")["card_id"], Value::Null);
}

/// L3, refusals, nothing stored by any: `C-0099` a 404, exactly `{"id":
/// "C-0099","reason":"no card C-0099"}`, even with a body an intake rule
/// would refuse (the card read first); the same question again a 200,
/// `created` false, its hit `PR-0001`, then stored naming it; a working
/// answer over the cap a 409, the reason last of `notes`; an unknown key,
/// `kind: "update"`, `why` on a question 400s; a look-alike node a 503;
/// non-JSON a 415; no same-origin page a 403; GET a 405 with `Allow: POST`.
/// M: the card not read first (`C-0099` stored or 409); the handler calling
/// `propose_question`.
#[test]
fn l3_refusals_store_nothing() {
    let lamp = Daemon::lamp("cq-l3-refused");
    lamp.raise("C-0001", &question(BELL, json!("dev")))
        .status(200);
    let rows = || lamp.sql("SELECT id FROM proposals ORDER BY id;");
    let mut stored = (rows(), lamp.event_lines());

    let nothing = |context: &str, stored: &(String, Vec<(String, String)>)| {
        assert_eq!(
            &(rows(), lamp.event_lines()),
            stored,
            "{context}: nothing stored"
        );
    };

    // A card no one made: 404 before any intake rule.
    for body in [
        question("Who meets the keeper at the pier?", json!("dev")),
        {
            let mut capped = question("Who meets the keeper at the pier?", json!("dev"));
            capped["working_answer"] = json!("a".repeat(2049));
            capped
        },
    ] {
        let reply = lamp.raise("C-0099", &body);
        assert_eq!(reply.status, 404, "{}", reply.text());
        assert_eq!(
            reply.text(),
            "{\"id\":\"C-0099\",\"reason\":\"no card C-0099\"}"
        );
        nothing("C-0099", &stored);
    }

    // The same question again: a hit, not stored; then named, stored.
    let reply = lamp.raise("C-0001", &question(BELL, json!("dev")));
    assert_eq!(reply.status, 200, "{}", reply.text());
    let document = reply.json();
    assert_eq!(document["created"], json!(false), "{document}");
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["hits"][0]["id"], json!("PR-0001"), "{document}");
    nothing("a hit", &stored);
    let mut named = question(BELL, json!("dev"));
    named["distinct_from"] = json!(["PR-0001"]);
    let reply = lamp.raise("C-0001", &named);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["id"], json!("PR-0002"));
    assert_eq!(reply.json()["created"], json!(true));
    stored = (rows(), lamp.event_lines());

    // A cap: 409, the intake document, its reason last.
    let mut capped = question("Is the cove reachable at high tide?", json!("dev"));
    capped["working_answer"] = json!("a".repeat(2049));
    let reply = lamp.raise("C-0001", &capped);
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(keys_of(reply.text()), INTAKE_KEYS);
    let document = reply.json();
    assert_eq!(
        document["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!("working_answer: 2049 bytes; at most 2048")),
        "{document}"
    );
    assert_eq!(document["created"], json!(false));
    nothing("a cap", &stored);

    // Bodies the route refuses: 400.
    let mut unknown = question("Is the cove reachable at high tide?", json!("dev"));
    unknown["task_id"] = json!("T-0001");
    let mut update = question("Is the cove reachable at high tide?", json!("dev"));
    update["kind"] = json!("update");
    let mut why = question("Is the cove reachable at high tide?", json!("dev"));
    why["why"] = json!("Because.");
    for (label, body, line) in [
        ("an unknown key", unknown, "task_id"),
        (
            "kind update",
            update,
            "`kind`: a card raises a `question` or a `decision`",
        ),
        ("why on a question", why, "`why` goes with a `decision`"),
    ] {
        let reply = lamp.raise("C-0001", &body);
        assert_eq!(reply.status, 400, "{label}: {}", reply.text());
        let message = reply.error_message();
        assert!(message.contains(line), "{label}: {message}");
        nothing(label, &stored);
    }

    // A look-alike node ("SCN-ST\u{041e}RM", a Cyrillic capital O): 503.
    let mut alike = question("Is the cove reachable at high tide?", json!("dev"));
    alike["node_ids"] = json!(["SCN-ST\u{041e}RM"]);
    let reply = lamp.raise("C-0001", &alike);
    assert_eq!(reply.status, 503, "{}", reply.text());
    nothing("a look-alike node", &stored);

    // The door: non-JSON, no same-origin page, another method.
    let path = format!("{LAMP}/cards/C-0001/items");
    let body = question("Is the cove reachable at high tide?", json!("dev")).to_string();
    let reply = lamp.server.send(
        "POST",
        &path,
        &[
            ("Sec-Fetch-Site", "same-origin"),
            ("Content-Type", "text/plain"),
        ],
        body.as_bytes(),
    );
    assert_eq!(reply.status, 415, "{}", reply.text());
    let reply = lamp.server.send(
        "POST",
        &path,
        &[("Content-Type", "application/json")],
        body.as_bytes(),
    );
    assert_eq!(reply.status, 403, "{}", reply.text());
    let reply = lamp.server.get(&path);
    assert_eq!(reply.status, 405, "{}", reply.text());
    assert_eq!(reply.header("allow"), Some("POST"));
    nothing("the door", &stored);
}

// ------------------------------------------------------------------ L4

/// The proposals route of `id`'s stage.
fn stage_path(daemon: &Daemon, id: &str) -> String {
    format!("{}/proposals/{id}/decision", daemon.base)
}

/// A same-origin page's stage of `fields` on `id` against the
/// `updated_at` it reads first.
fn stage_now(daemon: &Daemon, id: &str, fields: &str) -> Reply {
    let current = daemon.review(id);
    let updated_at = current["updated_at"].as_str().expect("updated_at");
    daemon.server.stage(
        &stage_path(daemon, id),
        &format!("{{{fields},\"updated_at\":\"{updated_at}\"}}"),
    )
}

/// The raw payload of the last `proposal.staged`.
fn last_staged(daemon: &Daemon) -> String {
    daemon
        .event_lines()
        .into_iter()
        .rev()
        .find(|(kind, _)| kind == "proposal.staged")
        .map(|(_, payload)| payload)
        .expect("a staged event")
}

/// L4, the daemon on spec-a: a local stage with `as: "ana"` logs
/// `proposal.staged` ending `"as":"ana"`, `staged` holding no `as`; without
/// `as`, and with `as: null`, the payload's keys are exactly `id`, `staged`,
/// `staged_at`; `as` off the person grammar is a 400, nothing staged;
/// `spec approve` confirms the stage as before. M: `"as":null` written; the
/// `as` stored in a column.
#[test]
fn l4_a_local_stage_takes_as_and_logs_it() {
    let daemon = Daemon::new("cq-l4-stage", "spec-a");
    let asked = ask(
        &daemon.home,
        &daemon.root,
        &["EDGE-STAM-ZERO"],
        "Is zero stamina a stop?",
    );
    let reply = stage_now(&daemon, &asked, "\"decision\":\"approve\",\"as\":\"ana\"");
    assert_eq!(reply.status, 200, "{}", reply.text());
    let payload = last_staged(&daemon);
    assert!(payload.ends_with(",\"as\":\"ana\"}"), "{payload}");
    assert_eq!(keys_of(&payload), ["id", "staged", "staged_at", "as"]);
    let staged = daemon.sql(&format!(
        "SELECT staged FROM proposals WHERE id = '{asked}';"
    ));
    assert!(!staged.contains("ana"), "no `as` in `staged`: {staged}");

    for fields in [
        "\"decision\":\"approve\"",
        "\"decision\":\"approve\",\"as\":null",
    ] {
        let reply = stage_now(&daemon, &asked, fields);
        assert_eq!(reply.status, 200, "{fields}: {}", reply.text());
        let payload = last_staged(&daemon);
        assert_eq!(
            keys_of(&payload),
            ["id", "staged", "staged_at"],
            "{fields}: {payload}"
        );
        assert_eq!(
            daemon.sql(&format!(
                "SELECT staged FROM proposals WHERE id = '{asked}';"
            )),
            staged,
            "{fields}: the same stage"
        );
    }

    // Off the person grammar: a line break, a bidi mark, blank.
    let events = daemon.event_lines();
    for actor in ["\"an\\nna\"", "\"a\\u202ena\"", "\"   \""] {
        let reply = stage_now(
            &daemon,
            &asked,
            &format!("\"decision\":\"approve\",\"as\":{actor}"),
        );
        assert_eq!(reply.status, 400, "{actor}: {}", reply.text());
        assert_eq!(daemon.event_lines(), events, "{actor}: nothing staged");
    }
}

/// L4: a round answer's stage takes `as` per stage (`RoundAnswersBody`
/// wraps `StageBody`): the one carrying `as` logs it, the other today's
/// payload.
#[test]
fn l4_a_round_answer_stages_under_its_as() {
    let daemon = Daemon::new("cq-l4-round", "spec-a");
    let first = ask(
        &daemon.home,
        &daemon.root,
        &["EDGE-STAM-ZERO"],
        "Is zero stamina a stop?",
    );
    let second = ask(
        &daemon.home,
        &daemon.root,
        &["MEC-SPRINT"],
        "Does a sprint end at zero?",
    );
    let at = |id: &str| {
        daemon.review(id)["updated_at"]
            .as_str()
            .expect("updated_at")
            .to_owned()
    };
    let body = json!({"project": "lantern-keep", "answers": [
        {"id": first, "stage": {"decision": "approve", "updated_at": at(&first), "as": "ana"}},
        {"id": second, "stage": {"decision": "approve", "updated_at": at(&second)}}
    ]});
    let reply = daemon.post("round/answers", &body);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let staged: Vec<(String, Vec<String>)> = daemon
        .event_lines()
        .into_iter()
        .filter(|(kind, _)| kind == "proposal.staged")
        .map(|(_, payload)| {
            let value: Value = serde_json::from_str(&payload).expect("JSON");
            (
                value["id"].as_str().expect("an ID").to_owned(),
                keys_of(&payload),
            )
        })
        .collect();
    assert_eq!(
        staged,
        [
            (
                first.clone(),
                vec![
                    "id".to_owned(),
                    "staged".to_owned(),
                    "staged_at".to_owned(),
                    "as".to_owned()
                ]
            ),
            (
                second.clone(),
                vec!["id".to_owned(), "staged".to_owned(), "staged_at".to_owned()]
            ),
        ]
    );
}
