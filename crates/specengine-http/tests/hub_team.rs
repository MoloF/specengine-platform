//! docs/features/hub-team.md (RM-0071, step 9a), the daemon's half: a `[team]`
//! login accepts, clears and stages on a hub, through the real daemon over git
//! repositories of `fixtures/card-press` (a print shop) and `fixtures/spec-a` (a
//! game's brief), so the mode is tried on two projects of different nature
//! (ADR-0008). Every expected value below is worked out from the spec and from
//! docs/canon/hub.md, never read back from a run.
//!
//! - A2: another name on accept, clear, group clear: 403, nothing changed; a
//!   null `as` records the login; a stage and an unstage log `as`; a local event
//!   stays byte for byte. M: dropping `acting` from any of the three; `as` on a
//!   local event.
//! - A3: a login outside `[team]` is refused the six acts and keeps the five
//!   writes and every read; a `member`'s group clear and a `lead` outside its
//!   zones are 403, an `owner` a 200. M: an absent entry read as `member`.
//! - A4: two roots, a team each; a bad `[team]` is a 503 on those routes and
//!   plain writes stand. M: a team cached in `App`; a plain write reading
//!   `[team]`.
//! - A5: the confirm class takes no query and needs an `Origin`; every raw form
//!   stays refused. M: `Hub::fenced` reading the path alone.
//! - A6: a null `revision` or `count` is a 400; a count that differs a 409, no
//!   card cleared. M: dropping the compare.
//! - A7: on a local daemon `[team]` changes nothing, a bad one included, and
//!   `GET .../team` is the unknown-route 404. M: judging `[team]` with no
//!   `Login`. Its terminal half is `specengine-cli`'s `tests/team_config.rs`,
//!   its `git diff` half a command.
//! - A8: a suspect card moves and is accepted; a retarget or cancel of an open
//!   one reads `no card`; an accepted one refuses both. M: opening `edit` on a
//!   closed card.
//!
//! A1 is core's `tests/team_toml.rs` and `specengine-cli`'s
//! `tests/team_config.rs`; A9 the UI's. The raw table of `accept` stays in
//! `tests/hub.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::path::Path;
use std::sync::Barrier;
use std::thread;

use common::hub::{
    Hub, ORIGIN, P, Proxied, Setup, assert_plain, assert_refused, five_writes, hub_args, through,
    through_in_two_steps,
};
use common::{
    HTTP_TIMEOUT, Reply, Scratch, Server, Stream, ask, encode_component, http_exchange, read_text,
    replace, spec_json, write,
};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};

/// The game's project (`fixtures/spec-a`).
const KEEP: &str = "/api/projects/lantern-keep";

/// The lines of docs/features/hub-team.md "Data".
const OFF_LIST_LINE: &str = "refused: this hub does not serve this route";
const NO_QUERY_LINE: &str = "refused: this route takes no query";
const NO_ORIGIN_LINE: &str = "refused: this route needs the Origin header";
const WRONG_ORIGIN_LINE: &str =
    "refused: the Origin header, when sent, must be https://hub.example.test";
const NO_TEAM_LINE: &str = "refused: this project has no `[team]`";

/// A login in another script, spoken with a space.
const FULL_NAME: &str =
    "\u{0410}\u{043d}\u{043d}\u{0430} \u{041f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";

const FRONT_MD: &str = "docs/spec/sht-front.md";
const BACK_MD: &str = "docs/spec/sht-back.md";
const INNER_MD: &str = "docs/spec/sht-inner.md";
const RULING_MD: &str = "docs/records/STY/STY-0001.md";

/// A hand's change of meaning in each of the print shop's files.
const FRONT_CHANGE: (&str, &str, &str) = (
    FRONT_MD,
    "the lead story.",
    "the lead story and the weather.",
);
const BACK_CHANGE: (&str, &str, &str) = (BACK_MD, "the tide table", "the moon table");
const INNER_CHANGE: (&str, &str, &str) = (
    INNER_MD,
    "carries the long reads",
    "carries the short reads",
);
const RULING_CHANGE: (&str, &str, &str) = (RULING_MD, "whatever else runs", "unless a storm runs");

/// The print shop's team: `olga` the owner; `lena` leads every sheet, `mira`
/// the front alone; `tomas` a member. `eve`, in no test's table, is the login
/// outside it, and `ana` assigns: the five writes are any login's. The zones
/// sit under no `[gate]`: the table is read here whether or not one is on.
const TEAM: &str = r#"
[zones]
sheets = ["docs/spec/**"]
front = ["docs/spec/sht-front.md"]

[team.olga]
name = "Olga R"
role = "owner"
email = "olga@example.org"

[team.lena]
name = "Lena V"
role = "lead"
zones = ["sheets"]
email = "lena@example.org"

[team.mira]
name = "Mira K"
role = "lead"
zones = ["front"]
email = "mira@example.org"

[team.tomas]
name = "Tomas B"
role = "member"
email = "tomas@example.org"
"#;

/// A `[team]` no reader takes: a role off the three.
const BAD_TEAM: &str = r#"
[team.mira]
name = "Mira K"
role = "editor"
email = "mira@example.org"
"#;

/// The game's team: `mira` alone, under another name than the print shop's.
const KEEP_TEAM: &str = r#"
[team.mira]
name = "Mira of the Keep"
role = "owner"
email = "mira@example.org"
"#;

/// `tomas`, on the game's team.
const KEEP_TOMAS: &str = r#"
[team.tomas]
name = "Tomas of the Keep"
role = "member"
email = "tomas@example.org"
"#;

// ------------------------------------------------------------------ helpers

/// `table` at the end of `root`'s config.
fn append_config(root: &Path, table: &str) {
    let mut text = read_text(root, "specengine.toml");
    text.push_str(table);
    write(root, "specengine.toml", text);
}

/// The 1-based line of `text` holding `needle`.
fn line_holding(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds {needle:?}"))
        + 1
}

/// A git repository of `fixture` that opted in: [`TEAM`], by a commit.
fn teamed(name: &str, fixture: &str) -> Setup {
    let setup = Setup::new(name, fixture);
    append_config(&setup.root, TEAM);
    let git = setup.scratch.git();
    git.run(&setup.root, &["commit", "-qam", "the team"]);
    setup
}

/// A login's line outside `[team]`.
fn outside_line(login: &str) -> String {
    format!("refused: {login} is not in this project's `[team]`: reads and plain writes")
}

/// A `plates` card on `targets`, in `queued`, `ana` its assignee and `tomas` its
/// reviewer: its ID. `plates` has no `moves`, so any pair of columns stands.
fn card_on(hub: &Hub, title: &str, targets: &[&str]) -> String {
    let made = hub.done(
        "ana",
        "cards",
        &json!({"title": title, "discipline": "plates", "target": targets, "assignee": "ana"}),
    );
    let id = made["id"].as_str().expect("a card's ID").to_owned();
    hub.done(
        "ana",
        &format!("cards/{id}/assign"),
        &json!({"reviewer": "tomas"}),
    );
    id
}

/// [`card_on`], at `accept.from`.
fn ready(hub: &Hub, title: &str, targets: &[&str]) -> String {
    let id = card_on(hub, title, targets);
    hub.done(
        "ana",
        &format!("cards/{id}/move"),
        &json!({"to": "checked"}),
    );
    id
}

/// The card's `revision`, a string on the wire.
fn revision(hub: &Hub, id: &str) -> String {
    hub.show(id)["revision"]
        .as_str()
        .expect("a revision")
        .to_owned()
}

/// A change by hand, committed: nobody vouched for it.
fn hand_edit(hub: &Hub, (path, from, to): (&str, &str, &str)) {
    let root = &hub.setup.root;
    replace(root, path, from, to);
    let git = hub.setup.scratch.git();
    git.run(root, &["add", "-A"]);
    git.run(root, &["commit", "-q", "-m", "by hand"]);
}

/// The rollup of the node `id`, as a page reads it.
fn rollup(hub: &Hub, id: &str) -> String {
    let path = format!("{P}/trace?ref={}", encode_component(id));
    let document = hub.get("mira", &path).status(200).json();
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == id)
        .unwrap_or_else(|| panic!("no node {id}: {document}"))["rollup"]
        .as_str()
        .expect("a rollup")
        .to_owned()
}

/// A reject of `id`, as its reader saw it.
fn stage_body(hub: &Hub, id: &str) -> Value {
    let at = hub.proposal(id)["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    json!({"decision": "reject", "reason": "Asked before.", "updated_at": at})
}

/// `DELETE <P>/path` by `login`.
fn delete(hub: &Hub, login: &str, path: &str) -> Reply {
    through(
        &hub.server,
        "DELETE",
        &format!("{P}/{path}"),
        &Proxied::write(login),
        b"",
    )
}

/// The payloads of `kind`, in order.
fn payloads(hub: &Hub, kind: &str) -> Vec<Value> {
    hub.events()
        .into_iter()
        .filter(|(event, _)| event == kind)
        .map(|(_, payload)| payload)
        .collect()
}

/// The last stored event, `(type, payload)`.
fn last_event(hub: &Hub) -> (String, Value) {
    hub.events().pop().expect("an event")
}

/// The bytes of each stored `kind`'s payload, as the queue holds them.
fn raw_payloads(home: &Path, root: &Path, kind: &str) -> Vec<String> {
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
        .filter(|event| event.event_type == kind)
        .map(|event| event.payload)
        .collect()
}

/// A group's refusal: `status`, its document `{group, cleared, notes}`, nothing
/// cleared, `line` last of `notes`.
fn assert_group_refused(reply: &Reply, status: u16, group: &str, line: &str, context: &str) {
    assert_eq!(reply.status, status, "{context}: {}", reply.text());
    assert_plain(reply, context);
    let document = reply.json();
    let mut keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
    keys.sort();
    assert_eq!(keys, ["cleared", "group", "notes"], "{context}: {document}");
    assert_eq!(document["group"], json!(group), "{context}: {document}");
    assert_eq!(document["cleared"], json!([]), "{context}: {document}");
    assert_eq!(
        document["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!(line)),
        "{context}: {document}"
    );
}

/// `GET .../team`'s row: `{login, name, allowed}`.
fn team_row(login: &str, name: &str, allowed: &[&str]) -> String {
    let list: Vec<String> = allowed.iter().map(|act| format!("\"{act}\"")).collect();
    format!(
        "{{\"login\":\"{login}\",\"name\":\"{name}\",\"allowed\":[{}]}}",
        list.join(",")
    )
}

/// `GET .../team` of a login no `[team]` names.
fn no_row(login: &str) -> String {
    format!("{{\"login\":\"{login}\",\"name\":null,\"allowed\":[]}}")
}

const MEMBER_ACTS: [&str; 3] = ["stage", "accept", "clear"];
const LEAD_ACTS: [&str; 5] = ["stage", "accept", "clear", "group_clear", "confirm"];

// --------------------------------------------------------------- the six acts

/// One act past the plain writes, a request that lands by `lander`.
#[derive(Debug, Clone)]
struct Act {
    name: &'static str,
    method: &'static str,
    path: String,
    body: String,
    lander: &'static str,
    /// A `Consent` act: the fence's confirm class.
    confirm: bool,
}

impl Act {
    fn post(name: &'static str, path: String, body: &Value, lander: &'static str) -> Self {
        Self {
            name,
            method: "POST",
            path: format!("{P}/{path}"),
            body: body.to_string(),
            lander,
            confirm: false,
        }
    }

    fn confirming(mut self) -> Self {
        self.confirm = true;
        self
    }

    /// This request by `login`, as a page sends it.
    fn by(&self, hub: &Hub, login: &str) -> Reply {
        self.proxied(hub, &Proxied::write(login))
    }

    /// This request, with the headers of `proxied`.
    fn proxied(&self, hub: &Hub, proxied: &Proxied) -> Reply {
        through(
            &hub.server,
            self.method,
            &self.path,
            proxied,
            self.body.as_bytes(),
        )
    }

    fn body_value(&self) -> Value {
        serde_json::from_str(&self.body).expect("a JSON body")
    }

    /// This request with `key` set to `value` in its body.
    fn with(&self, key: &str, value: Value) -> Self {
        let mut body = self.body_value();
        body[key] = value;
        Self {
            body: body.to_string(),
            ..self.clone()
        }
    }

    /// This request with no `key` in its body.
    fn without(&self, key: &str) -> Self {
        let mut body = self.body_value();
        body.as_object_mut().expect("an object").remove(key);
        Self {
            body: body.to_string(),
            ..self.clone()
        }
    }

    /// The key whose absence a hub refuses: `revision`, or `count`.
    fn seen_key(&self) -> &'static str {
        if self.name == "group clear" {
            "count"
        } else {
            "revision"
        }
    }
}

/// A project where each of the six acts has something to land on: two open
/// questions, the second staged by `tomas`; `ready`, at `accept.from`, `tomas`
/// its reviewer; `suspect` on SHT-FRONT for a clear, `inner` on SHT-INNER for a
/// group's, each marked by a hand's edit.
struct Waiting {
    hub: Hub,
    asked: String,
    staged: String,
    ready: String,
    suspect: String,
}

impl Waiting {
    fn new(name: &str) -> Self {
        let setup = teamed(name, "card-press");
        let asked = ask(
            &setup.home,
            &setup.root,
            &["SHT-BACK"],
            "Does the tide table keep its rule?",
        );
        let staged = ask(
            &setup.home,
            &setup.root,
            &["SHT-INNER"],
            "Which typeface carries the inner column rules?",
        );
        let hub = setup.start();
        let ready = ready(&hub, "Back page", &["SHT-BACK"]);
        let suspect = card_on(&hub, "Lead story", &["SHT-FRONT"]);
        card_on(&hub, "Long read", &["SHT-INNER"]);
        hand_edit(&hub, FRONT_CHANGE);
        hand_edit(&hub, INNER_CHANGE);
        let stage = stage_body(&hub, &staged);
        hub.done("tomas", &format!("proposals/{staged}/decision"), &stage);
        Self {
            hub,
            asked,
            staged,
            ready,
            suspect,
        }
    }

    fn ids(&self) -> [&str; 2] {
        [&self.asked, &self.staged]
    }

    /// The six acts as they would land now.
    fn acts(&self) -> Vec<Act> {
        let hub = &self.hub;
        let stage = stage_body(hub, &self.asked);
        vec![
            Act::post(
                "stage",
                format!("proposals/{}/decision", self.asked),
                &stage,
                "tomas",
            ),
            Act {
                name: "unstage",
                method: "DELETE",
                path: format!("{P}/proposals/{}/decision", self.staged),
                body: String::new(),
                lander: "tomas",
                confirm: false,
            },
            Act::post(
                "round answers",
                "round/answers".to_owned(),
                &json!({"project": "card-press", "answers": [{"id": self.asked, "stage": stage}]}),
                "tomas",
            ),
            Act::post(
                "accept",
                format!("cards/{}/accept", self.ready),
                &json!({"briefed": true, "revision": revision(hub, &self.ready)}),
                "tomas",
            )
            .confirming(),
            Act::post(
                "clear",
                format!("cards/{}/clear", self.suspect),
                &json!({"outcome": "no_effect", "reason": "A reprint.",
                    "revision": revision(hub, &self.suspect)}),
                "tomas",
            )
            .confirming(),
            Act::post(
                "group clear",
                "trace/clear".to_owned(),
                &json!({"group": "SHT-INNER", "reason": "A reprint.", "count": 1}),
                "olga",
            )
            .confirming(),
        ]
    }

    /// The acts taking a `Consent`.
    fn confirm(&self) -> Vec<Act> {
        self.acts().into_iter().filter(|act| act.confirm).collect()
    }
}

// -------------------------------------------------------------------- A2

/// A2: on each of the three acts a well-formed name other than the login is a
/// 403 naming both, and nothing changes, whichever of two logins asks: `mira`
/// naming `tomas`, the card's reviewer and a clear's anyone, would land if the
/// name stood, so a dropped compare is a 200 here and not another refusal.
/// M: dropping `acting` from any of the three.
#[test]
fn a2_another_name_on_accept_clear_and_group_clear_is_a_403_and_changes_nothing() {
    let waiting = Waiting::new("team-as-other");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    for act in waiting.confirm() {
        let (login, named) = if act.name == "group clear" {
            ("lena", "olga")
        } else {
            ("mira", "tomas")
        };
        for (login, named) in [(login, named), (named, login)] {
            let reply = act.with("as", json!(named)).by(hub, login);
            let line = format!("refused: `as` names {named}; this request's login is {login}");
            assert_refused(&reply, &line, act.name);
            assert_eq!(hub.stored(&ids), before, "{}: nothing changed", act.name);
        }
        // Byte for byte: case and a space count.
        for named in ["Tomas", "tomas ", "local", "agent"] {
            let reply = act.with("as", json!(named)).by(hub, "tomas");
            assert_eq!(reply.status, 403, "{} {named}: {}", act.name, reply.text());
            assert_eq!(hub.stored(&ids), before, "{} {named}", act.name);
        }
    }
}

/// A2: null, absent or the login's own `as` records the login: on the card, in
/// `card.accepted`, in each `card.cleared`. `CardAcceptBody` defaults to `local`,
/// who accepts nothing, so `hub::acting` must have run before `.request()`.
/// M: dropping `acting` from any of the three.
#[test]
fn a2_a_null_as_records_the_login_on_accept_clear_and_group_clear() {
    let hub = teamed("team-as-null", "card-press").start();
    let forms = [Some(Value::Null), Some(json!("tomas")), None];
    let with_as = |mut body: Value, form: &Option<Value>| {
        if let Some(form) = form {
            body["as"] = form.clone();
        }
        body
    };

    let mut accepted = Vec::new();
    for form in &forms {
        accepted.push((ready(&hub, "Long read", &["SHT-INNER"]), form));
    }
    for (id, form) in accepted {
        let body = with_as(
            json!({"briefed": true, "revision": revision(&hub, &id)}),
            form,
        );
        hub.done("tomas", &format!("cards/{id}/accept"), &body);
        let (kind, payload) = last_event(&hub);
        assert_eq!(kind, "card.accepted", "{form:?}: {payload}");
        assert_eq!(payload["as"], json!("tomas"), "{form:?}: {payload}");
        let card = hub.show(&id);
        assert_eq!(card["acceptance"]["by"], json!("tomas"), "{form:?}: {card}");
    }

    let cards: Vec<String> = (0..3)
        .map(|_| card_on(&hub, "Lead story", &["SHT-FRONT"]))
        .collect();
    hand_edit(&hub, FRONT_CHANGE);
    for (id, form) in cards.iter().zip(&forms) {
        let body = with_as(
            json!({"outcome": "no_effect", "reason": "Only the weather.",
                "revision": revision(&hub, id)}),
            form,
        );
        hub.done("tomas", &format!("cards/{id}/clear"), &body);
        let (kind, payload) = last_event(&hub);
        assert_eq!(kind, "card.cleared", "{form:?}: {payload}");
        assert_eq!(payload["as"], json!("tomas"), "{form:?}: {payload}");
    }

    let group_forms = [Some(Value::Null), Some(json!("olga")), None];
    let notices = [
        card_on(&hub, "Notices", &["SHT-BACK"]),
        card_on(&hub, "Tides", &["SHT-BACK"]),
    ];
    let edits = [
        ("the tide table", "the moon table"),
        ("the moon table", "the sun table"),
        ("the sun table", "the star table"),
    ];
    for (form, (from, to)) in group_forms.iter().zip(edits) {
        hand_edit(&hub, (BACK_MD, from, to));
        let seen = payloads(&hub, "card.cleared").len();
        let body = with_as(
            json!({"group": "SHT-BACK", "reason": "A reprint.", "count": 2}),
            form,
        );
        let done = hub.done("olga", "trace/clear", &body);
        assert_eq!(done["cleared"], json!(notices), "{form:?}: {done}");
        let cleared = payloads(&hub, "card.cleared");
        assert_eq!(cleared.len(), seen + 2, "{form:?}: {cleared:?}");
        for event in &cleared[seen..] {
            assert_eq!(event["as"], json!("olga"), "{form:?}: {event}");
        }
    }
}

/// A2: a stage, an unstage and a round's answers log `as`, the login, each by
/// another person, the rest of the payload as ever. M: the login dropped on its
/// way to `proposal.staged`.
#[test]
fn a2_a_stage_and_an_unstage_log_the_login() {
    let setup = teamed("team-stage-as", "card-press");
    let asked = ask(
        &setup.home,
        &setup.root,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let hub = setup.start();
    let decision = format!("proposals/{asked}/decision");

    let stage = stage_body(&hub, &asked);
    hub.done("tomas", &decision, &stage);
    let (kind, payload) = last_event(&hub);
    assert_eq!(kind, "proposal.staged", "{payload}");
    assert_eq!(payload["as"], json!("tomas"), "{payload}");
    assert_eq!(payload["id"], json!(asked), "{payload}");
    assert_eq!(
        payload["staged"]["decision"],
        json!("reject"),
        "the stage stands: {payload}"
    );
    assert!(payload["staged_at"].is_string(), "{payload}");
    let raw = raw_payloads(&hub.setup.home, &hub.setup.root, "proposal.staged");
    assert!(
        raw[0].ends_with(",\"as\":\"tomas\"}"),
        "`as` closes the payload: {}",
        raw[0]
    );

    delete(&hub, "mira", &decision).status(200);
    let (kind, payload) = last_event(&hub);
    assert_eq!(kind, "proposal.unstaged", "{payload}");
    assert_eq!(payload, json!({"id": asked, "as": "mira"}));
    assert_eq!(
        raw_payloads(&hub.setup.home, &hub.setup.root, "proposal.unstaged"),
        [format!("{{\"id\":\"{asked}\",\"as\":\"mira\"}}")],
    );

    let stage = stage_body(&hub, &asked);
    let answers = json!({"project": "card-press", "answers": [{"id": asked, "stage": stage}]});
    hub.done("lena", "round/answers", &answers);
    let (kind, payload) = last_event(&hub);
    assert_eq!(kind, "proposal.staged", "{payload}");
    assert_eq!(payload["as"], json!("lena"), "{payload}");
    assert_eq!(payload["id"], json!(asked), "{payload}");
}

/// A2: a local event stays byte for byte: a local daemon's stage, round answer
/// and unstage store no `as`, in a project with a `[team]` too, a `Remote-User`
/// sent or not: `{"id":...,"staged":{...},"staged_at":...}` in that order, and
/// `{"id":...}`. M: `as` on a local event.
#[test]
fn a2_a_local_event_stays_byte_for_byte() {
    let scratch = Scratch::new("team-local-events");
    let root = scratch.repo("card-press", "served", "main");
    append_config(&root, TEAM);
    let git = scratch.git();
    git.run(&root, &["commit", "-qam", "the team"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let asked = ask(
        &home,
        &root,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let server = Server::serve(&home, &cwd, &[&root]);
    let decision = format!("{P}/proposals/{asked}/decision");
    let at = |server: &Server| {
        server
            .get(&format!("{P}/proposals/{asked}"))
            .status(200)
            .json()["updated_at"]
            .as_str()
            .expect("updated_at")
            .to_owned()
    };
    let own = format!("http://127.0.0.1:{}", server.port);
    let page = [
        ("Sec-Fetch-Site", "same-origin"),
        ("Origin", own.as_str()),
        ("Content-Type", "application/json"),
        ("Remote-User", "tomas"),
    ];

    let stage = json!({"decision": "reject", "reason": "Asked before.", "updated_at": at(&server)});
    server
        .send("POST", &decision, &page, stage.to_string().as_bytes())
        .status(200);
    server.unstage(&decision).status(200);
    let stage = json!({"decision": "reject", "reason": "Asked before.", "updated_at": at(&server)});
    let answers = json!({"project": "card-press", "answers": [{"id": asked, "stage": stage}]});
    server
        .stage(&format!("{P}/round/answers"), &answers.to_string())
        .status(200);

    let staged = raw_payloads(&home, &root, "proposal.staged");
    assert_eq!(staged.len(), 2, "{staged:?}");
    for payload in &staged {
        let parsed: Value = serde_json::from_str(payload).expect("a JSON payload");
        let mut keys: Vec<&String> = parsed.as_object().expect("an object").keys().collect();
        keys.sort();
        assert_eq!(keys, ["id", "staged", "staged_at"], "{payload}");
        let at = parsed["staged_at"].as_str().expect("staged_at");
        assert!(
            payload.starts_with(&format!("{{\"id\":\"{asked}\",\"staged\":{{"))
                && payload.ends_with(&format!(",\"staged_at\":\"{at}\"}}")),
            "keys in their order: {payload}"
        );
    }
    assert_eq!(
        raw_payloads(&home, &root, "proposal.unstaged"),
        [format!("{{\"id\":\"{asked}\"}}")],
    );
}

// -------------------------------------------------------------------- A3

/// A3: a login outside `[team]`, with a good page and every act ready to land,
/// is a 403 on the six, naming itself, and nothing is stored; `olga`'s same
/// requests land, one by one, so the refusal is the login's alone. M: an absent
/// entry read as `member`.
#[test]
fn a3_a_login_outside_team_is_refused_the_six_acts_and_nothing_is_stored() {
    let waiting = Waiting::new("team-outside");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    for act in waiting.acts() {
        assert_refused(&act.by(hub, "eve"), &outside_line("eve"), act.name);
        assert_eq!(hub.stored(&ids), before, "{}: nothing stored", act.name);
    }
    for index in 0..6 {
        let act = &waiting.acts()[index];
        let reply = act.by(hub, act.lander);
        assert_eq!(reply.status, 200, "{}: {}", act.name, reply.text());
    }
}

/// What `eve` must keep: every read that answers 200 on the print shop whoever
/// asks.
fn served_reads(card: &str, proposal: &str) -> Vec<String> {
    let mut reads = vec!["/api/identity".to_owned(), "/api/projects".to_owned()];
    reads.extend(
        [
            "tree".to_owned(),
            "nodes/SHT-FRONT".to_owned(),
            "search?query=tide".to_owned(),
            "bundle?node_ids=SHT-FRONT".to_owned(),
            "trace?ref=SHT-FRONT".to_owned(),
            "graph?ref=SHT-FRONT".to_owned(),
            "check".to_owned(),
            "health".to_owned(),
            "inbox".to_owned(),
            format!("proposals/{proposal}"),
            "tasks".to_owned(),
            "cards".to_owned(),
            format!("cards/{card}"),
            "round".to_owned(),
            "team".to_owned(),
        ]
        .map(|read| format!("{P}/{read}")),
    );
    reads
}

/// The reads the print shop's config answers otherwise (no `[roadmap]`, no
/// `spec.lock`, no task, a page perhaps not built): a person's answer does not
/// depend on `[team]` either.
fn other_reads() -> Vec<String> {
    let mut reads = vec!["/".to_owned(), "/assets/x.js".to_owned()];
    reads.extend(
        ["flow", "roadmap", "lock-diff?ref=SHT-FRONT", "tasks/T-0001"]
            .map(|read| format!("{P}/{read}")),
    );
    reads
}

/// Every read of [`served_reads`] and [`other_reads`] by `login`: a 200, or what
/// a login on the team is answered; the event stream opens.
fn assert_reads(waiting: &Waiting, login: &str, context: &str) {
    let hub = &waiting.hub;
    for path in served_reads(&waiting.ready, &waiting.asked) {
        let reply = hub.get(login, &path);
        assert_eq!(
            reply.status,
            200,
            "{context}: {login} GET {path}: {}",
            String::from_utf8_lossy(&reply.body)
        );
    }
    for path in other_reads() {
        let reply = hub.get(login, &path);
        let member = hub.get("tomas", &path);
        assert_ne!(reply.status, 403, "{context}: {login} GET {path}");
        assert_eq!(
            reply.status, member.status,
            "{context}: {login} GET {path}: as a login on the team"
        );
    }
    let tail = Stream::open_raw(
        hub.server.port,
        &format!("{P}/events"),
        &Proxied::fetch(login).pairs(),
    );
    assert_eq!(tail.status, 200, "{context}: {login} GET events");
}

/// A3: a login outside `[team]` makes the five writes and reads everything, and
/// `GET .../team` is its 200 with `allowed` empty: a login outside the table
/// keeps what any login has. M: an absent entry read as `member`; a plain write
/// reading `[team]`.
#[test]
fn a3_a_login_outside_team_keeps_the_five_writes_and_every_read() {
    let waiting = Waiting::new("team-outside-reads");
    let hub = &waiting.hub;
    let id = "C-0004";
    for (path, body, kind) in five_writes(id) {
        hub.done("eve", &path, &body);
        let (stored, payload) = last_event(hub);
        assert_eq!(stored.as_str(), kind, "{path}");
        assert_eq!(payload["as"], json!("eve"), "{path}: {payload}");
    }
    assert_reads(&waiting, "eve", "outside `[team]`");
    assert_eq!(
        hub.get("eve", &format!("{P}/team")).status(200).text(),
        no_row("eve")
    );
}

/// "Data": `GET .../team` is the asker's own row, keys in order, exact bytes:
/// the first three acts for a `member`, four for a `lead` and an `owner`, and a
/// repeat of the same bytes. M: an absent entry read as `member`.
#[test]
fn a3_team_answers_the_askers_own_row() {
    let hub = teamed("team-row", "card-press").start();
    let cases = [
        ("olga", team_row("olga", "Olga R", &LEAD_ACTS)),
        ("lena", team_row("lena", "Lena V", &LEAD_ACTS)),
        ("mira", team_row("mira", "Mira K", &LEAD_ACTS)),
        ("tomas", team_row("tomas", "Tomas B", &MEMBER_ACTS)),
        ("eve", no_row("eve")),
    ];
    for (login, row) in &cases {
        for _ in 0..2 {
            let reply = hub.get(login, &format!("{P}/team"));
            reply.status(200);
            assert_eq!(reply.text(), row, "{login}");
            assert_plain(&reply, login);
        }
    }
}

/// "Data": `name` is optional: an entry with none answers `name` null and its
/// acts all the same. M: a missing `name` read as outside `[team]`.
#[test]
fn an_entry_with_no_name_answers_name_null() {
    let setup = teamed("team-no-name", "card-press");
    append_config(
        &setup.root,
        "\n[team.nora]\nrole = \"member\"\nemail = \"nora@example.org\"\n",
    );
    let hub = setup.start();
    let reply = hub.get("nora", &format!("{P}/team"));
    reply.status(200);
    assert_eq!(
        reply.text(),
        "{\"login\":\"nora\",\"name\":null,\"allowed\":[\"stage\",\"accept\",\"clear\"]}"
    );
}

/// "Data": no `[team]` is a 200 too: `name` null, `allowed` empty, for anyone.
/// `GET .../team` reads and stores nothing else. M: a project with no table read
/// as an empty door, a 403.
#[test]
fn a_project_with_no_team_answers_an_empty_row() {
    let hub = Setup::new("team-no-table", "card-press").start();
    let reply = hub.get("mira", &format!("{P}/team"));
    reply.status(200);
    assert_eq!(reply.text(), no_row("mira"));
    assert!(hub.events().is_empty(), "{:?}", hub.events());
}

/// "A login key": the key is the login byte for byte, quoted when not bare: a
/// case apart is someone else, and a name in another script with a space is a
/// login like any other, on the row and on an act. M: a login folded before the
/// lookup.
#[test]
fn a_login_is_compared_byte_for_byte() {
    let setup = teamed("team-bytes", "card-press");
    append_config(
        &setup.root,
        &format!(
            "\n[team.\"{FULL_NAME}\"]\nname = \"Anna P\"\nrole = \"member\"\nemail = \
             \"anna@example.org\"\n"
        ),
    );
    let git = setup.scratch.git();
    git.run(&setup.root, &["commit", "-qam", "one more"]);
    let asked = ask(
        &setup.home,
        &setup.root,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let hub = setup.start();
    let team = format!("{P}/team");
    for other in ["Mira", "MIRA", "mira.k"] {
        let reply = hub.get(other, &team);
        reply.status(200);
        assert_eq!(reply.text(), no_row(other), "{other}");
    }
    let row = hub.get(FULL_NAME, &team).status(200).json();
    assert_eq!(row["login"], json!(FULL_NAME), "{row}");
    assert_eq!(row["name"], json!("Anna P"), "{row}");
    assert_eq!(row["allowed"], json!(MEMBER_ACTS), "{row}");

    let decision = format!("proposals/{asked}/decision");
    assert_refused(
        &hub.post("Mira", &decision, &stage_body(&hub, &asked)),
        &outside_line("Mira"),
        "a case apart",
    );
    hub.done(FULL_NAME, &decision, &stage_body(&hub, &asked));
    let (_, payload) = last_event(&hub);
    assert_eq!(payload["as"], json!(FULL_NAME), "{payload}");
}

/// A3, "Who": a `member` takes five of the six acts, and its group clear is a
/// 403 with nothing stored. The table's order: any `[team]` login stages,
/// unstages, answers and clears; accept is the reviewer's. M: a `member` read as
/// a `lead`.
#[test]
fn a3_a_member_may_every_act_but_the_group_clear() {
    let waiting = Waiting::new("team-member");
    let hub = &waiting.hub;
    for index in 0..5 {
        let act = &waiting.acts()[index];
        let reply = act.by(hub, "tomas");
        assert_eq!(reply.status, 200, "{}: {}", act.name, reply.text());
    }
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    let group = &waiting.acts()[5];
    assert_refused(
        &group.by(hub, "tomas"),
        "refused: tomas is a `member`: this act needs a `lead`",
        "a member's group clear",
    );
    assert_eq!(hub.stored(&ids), before, "nothing stored");
}

/// A3: on one group, one body: a `member` and a `lead` outside its zones are
/// 403s, nothing cleared, and an `owner` clears both cards. The refused lead
/// covers one of the two nodes: every node must lie inside. M: a lead's cover
/// read as "any node"; an absent entry read as `member`.
#[test]
fn a3_a_member_and_a_lead_outside_its_zones_are_refused_a_group_clear_an_owner_is_not() {
    let hub = teamed("team-group", "card-press").start();
    let front = card_on(&hub, "Lead story", &["SHT-FRONT"]);
    let back = card_on(&hub, "Notices", &["SHT-BACK"]);
    hand_edit(&hub, FRONT_CHANGE);
    hand_edit(&hub, BACK_CHANGE);
    let body = json!({"group": "PRN-WEEKLY", "reason": "A reprint.", "count": 2});
    let before = hub.board();

    assert_refused(
        &hub.post("tomas", "trace/clear", &body),
        "refused: tomas is a `member`: this act needs a `lead`",
        "a member",
    );
    assert_eq!(hub.board(), before, "a member cleared nothing");
    assert_group_refused(
        &hub.post("mira", "trace/clear", &body),
        403,
        "PRN-WEEKLY",
        "refused: mira leads front; docs/spec/sht-back.md lies outside them",
        "a lead outside its zones",
    );
    assert_eq!(hub.board(), before, "a lead outside cleared nothing");
    assert_refused(
        &hub.post("eve", "trace/clear", &body),
        &outside_line("eve"),
        "outside `[team]`",
    );
    assert_eq!(hub.board(), before, "outside cleared nothing");

    let done = hub.done("olga", "trace/clear", &body);
    assert_eq!(done["cleared"], json!([front, back]), "{done}");
    assert_eq!(payloads(&hub, "card.cleared").len(), 2);
}

/// "Roles": a `lead` clears inside its zones, whatever else the card is marked
/// on: only the group's entries are frozen again, so a mark outside stays and is
/// no bar. A path in no zone is the `owner`'s alone, and a lead with `**` over
/// the sheets covers each of them. M: the cover judged over a card's every
/// target; an unzoned path read as covered.
#[test]
fn a_lead_clears_inside_its_zones_and_a_path_in_no_zone_is_the_owners() {
    let hub = teamed("team-zones", "card-press").start();
    let front = card_on(&hub, "Lead story", &["SHT-FRONT"]);
    let both = card_on(&hub, "Both", &["SHT-FRONT", "STY-0001"]);
    let ruling = card_on(&hub, "Ruling", &["STY-0001"]);
    let notices = card_on(&hub, "Notices", &["SHT-BACK"]);
    hand_edit(&hub, FRONT_CHANGE);
    hand_edit(&hub, RULING_CHANGE);
    hand_edit(&hub, BACK_CHANGE);

    let done = hub.done(
        "mira",
        "trace/clear",
        &json!({"group": "SHT-FRONT", "reason": "Only the weather.", "count": 2}),
    );
    assert_eq!(done["cleared"], json!([front, both]), "{done}");
    assert_eq!(rollup(&hub, "SHT-FRONT"), "in work");
    assert_eq!(
        rollup(&hub, "STY-0001"),
        "suspect",
        "the mark outside stays"
    );

    let ruled = json!({"group": "STY-0001", "reason": "A reprint.", "count": 2});
    let before = hub.board();
    assert_group_refused(
        &hub.post("lena", "trace/clear", &ruled),
        403,
        "STY-0001",
        "refused: lena leads sheets; docs/records/STY/STY-0001.md lies outside them",
        "a path in no zone",
    );
    assert_eq!(hub.board(), before, "a path in no zone: nothing cleared");
    let done = hub.done("olga", "trace/clear", &ruled);
    assert_eq!(done["cleared"], json!([both, ruling]), "{done}");

    let done = hub.done(
        "lena",
        "trace/clear",
        &json!({"group": "PRN-WEEKLY", "reason": "A reprint.", "count": 1}),
    );
    assert_eq!(done["cleared"], json!([notices]), "{done}");
}

/// "Classes": in the handler membership, then `hub::acting`, then `revision` or
/// `count`: each refusal answers for the first thing wrong, on all three confirm
/// routes. M: `revision` judged before the name.
#[test]
fn the_handlers_judge_membership_then_the_name_then_what_was_seen() {
    let waiting = Waiting::new("team-order");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    for act in waiting.confirm() {
        let wrong = act.with("as", json!("ana")).without(act.seen_key());
        assert_refused(&wrong.by(hub, "eve"), &outside_line("eve"), act.name);
        let line = format!(
            "refused: `as` names ana; this request's login is {}",
            act.lander
        );
        assert_refused(&wrong.by(hub, act.lander), &line, act.name);
        let reply = act.without(act.seen_key()).by(hub, act.lander);
        assert_eq!(reply.status, 400, "{}: {}", act.name, reply.text());
    }
    assert_eq!(hub.stored(&ids), before, "nothing stored");
}

/// "Who": an accept stays the card's reviewer's, on a login of the team, and
/// never its assignee's: `[team]` adds a door and takes no rule away. The
/// answers are the CLI's 409, nothing changed. M: membership read as the
/// reviewer's right.
#[test]
fn an_accept_stays_the_reviewers_and_never_the_assignees() {
    let hub = teamed("team-reviewer", "card-press").start();
    let made = hub.done(
        "ana",
        "cards",
        &json!({"title": "Back page", "discipline": "plates", "assignee": "mira",
            "target": ["SHT-BACK"]}),
    );
    let id = made["id"].as_str().expect("a card's ID").to_owned();
    hub.done(
        "ana",
        &format!("cards/{id}/assign"),
        &json!({"reviewer": "tomas"}),
    );
    hub.done(
        "ana",
        &format!("cards/{id}/move"),
        &json!({"to": "checked"}),
    );
    let accept = format!("cards/{id}/accept");
    let body = json!({"briefed": true, "revision": revision(&hub, &id)});
    let before = hub.board();
    for login in ["mira", "lena", "olga"] {
        let reply = hub.post(login, &accept, &body);
        assert_eq!(reply.status, 409, "{login}: {}", reply.text());
        assert_eq!(hub.board(), before, "{login}: nothing changed");
    }
    let accepted = hub.done("tomas", &accept, &body);
    assert_eq!(accepted["column"], json!("struck"), "{accepted}");
}

/// "Until 9b": a departing accept on a hub names the login in `card.accepted`,
/// and its one item stores `Author::human()`: no role, model or run; the
/// proposal is made in the accept's transaction, git untouched. M: the login
/// written into `Author`, before RM-0050.
#[test]
fn a_departing_accept_names_the_login_and_stores_a_human_author() {
    let hub = teamed("team-departing", "card-press").start();
    let id = ready(&hub, "Lead story", &["SHT-FRONT"]);
    let git = hub.repository();
    let body = json!({
        "departs": ["SHT-FRONT"],
        "summary": "The result left the brief.",
        "observed": "The lead runs below the fold.",
        "documented": "The lead runs above the fold.",
        "revision": revision(&hub, &id),
    });
    hub.done("tomas", &format!("cards/{id}/accept"), &body);
    assert_eq!(hub.repository(), git, "git is read, only read");

    let accepted = payloads(&hub, "card.accepted");
    assert_eq!(accepted.len(), 1, "{accepted:?}");
    assert_eq!(accepted[0]["as"], json!("tomas"), "{}", accepted[0]);
    let created = payloads(&hub, "proposal.created");
    assert_eq!(created.len(), 1, "{created:?}");
    let item = created[0]["id"].as_str().expect("the item's ID");
    let setup = &hub.setup;
    let review = spec_json(&setup.home, &setup.cwd, &setup.root, &["review", item]).json();
    assert_eq!(review["author"]["type"], json!("human"), "{review}");
    assert_eq!(review["author"]["role"], Value::Null, "{review}");
    assert_eq!(review["author"]["model"], Value::Null, "{review}");
    assert_eq!(review["author"]["run"], Value::Null, "{review}");
}

// -------------------------------------------------- from the security review

/// A departure's four keys, by the reviewer of its card.
fn departure(node: &str) -> Value {
    json!({
        "departs": [node],
        "summary": "The result left the brief.",
        "observed": "The lead runs below the fold.",
        "documented": "The lead runs above the fold.",
    })
}

/// The note of a card the group passes over (`group_marks`).
const PASSED_OVER: &str = "C-0001 reads departed PR-0001: passed over; clear it alone once that is \
                           settled";

/// A group's document holds `note`.
fn assert_noted(reply: &Reply, note: &str, context: &str) {
    let document = reply.json();
    let notes = document["notes"].as_array().expect("notes");
    assert!(notes.contains(&json!(note)), "{context}: {document}");
}

/// The departed sequence on a hub: C-0001 accepted departing SHT-FRONT, C-0002
/// left open on it, the node edited. The group takes what a row of `spec trace`
/// reads `suspect`, so it holds one card: `count: 2` is a 409 and `count: 1` a
/// 200 clearing C-0002 alone, each with the note naming C-0001, which stands as
/// it was. The terminal's form is `specengine-cli`'s `tests/team_config.rs`.
/// M: `chosen` taking a departed card; the note dropped.
#[test]
fn a_departed_card_is_no_part_of_a_group_clear() {
    let hub = teamed("team-departed", "card-press").start();
    let departed = ready(&hub, "Lead story", &["SHT-FRONT"]);
    assert_eq!(departed, "C-0001");
    let mut body = departure("SHT-FRONT");
    body["revision"] = json!(revision(&hub, &departed));
    hub.done("tomas", &format!("cards/{departed}/accept"), &body);
    let open = card_on(&hub, "Masthead", &["SHT-FRONT"]);
    assert_eq!(open, "C-0002");
    hand_edit(&hub, FRONT_CHANGE);
    let held = hub.show(&departed);

    let group =
        |count: u64| json!({"group": "PRN-WEEKLY", "reason": "Only the weather.", "count": count});
    let before = hub.board();
    let reply = hub.post("olga", "trace/clear", &group(2));
    assert_group_refused(
        &reply,
        409,
        "PRN-WEEKLY",
        "`PRN-WEEKLY` holds 1 suspect cards, not 2; nothing changed",
        "the departed card counted",
    );
    assert_noted(&reply, PASSED_OVER, "on the refusal");
    assert_eq!(hub.board(), before, "a count of two changed nothing");

    let reply = hub.post("olga", "trace/clear", &group(1));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["cleared"], json!([open]), "{}", reply.text());
    assert_noted(&reply, PASSED_OVER, "on the clear");
    let cleared = payloads(&hub, "card.cleared");
    assert_eq!(cleared.len(), 1, "{cleared:?}");
    assert_eq!(cleared[0]["id"], json!(open), "{}", cleared[0]);
    assert_eq!(hub.show(&departed), held, "the departed card as it was");
}

/// The departed sequence on a local daemon: its route, a typed name: `count: 2`
/// the same 409 and note, and no `count` a 200 clearing C-0002 alone with the
/// note. M: `chosen` taking a departed card.
#[test]
fn a_departed_card_is_no_part_of_a_local_group_clear() {
    let scratch = Scratch::new("team-departed-local");
    let root = scratch.repo("card-press", "served", "main");
    let git = scratch.git();
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let post = |path: &str, body: &Value| {
        let reply = server.stage(&format!("{P}/{path}"), &body.to_string());
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        reply.json()
    };
    let card = json!({"title": "Lead story", "discipline": "plates", "assignee": "ana",
        "as": "ana", "target": ["SHT-FRONT"]});
    post("cards", &card);
    post(
        "cards/C-0001/assign",
        &json!({"reviewer": "ben", "as": "ana"}),
    );
    post("cards/C-0001/move", &json!({"to": "checked", "as": "ana"}));
    let mut body = departure("SHT-FRONT");
    body["as"] = json!("ben");
    post("cards/C-0001/accept", &body);
    post("cards", &card);
    let (path, from, to) = FRONT_CHANGE;
    replace(&root, path, from, to);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "by hand"]);
    let held = server.get(&format!("{P}/cards/C-0001")).status(200).json();

    let group = json!({"group": "PRN-WEEKLY", "reason": "Only the weather.", "as": "ben"});
    let mut counted = group.clone();
    counted["count"] = json!(2);
    let trace_clear = format!("{P}/trace/clear");
    let reply = server.stage(&trace_clear, &counted.to_string());
    assert_group_refused(
        &reply,
        409,
        "PRN-WEEKLY",
        "`PRN-WEEKLY` holds 1 suspect cards, not 2; nothing changed",
        "the departed card counted",
    );
    assert_noted(&reply, PASSED_OVER, "on the refusal");

    let reply = server.stage(&trace_clear, &group.to_string());
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.json()["cleared"],
        json!(["C-0002"]),
        "{}",
        reply.text()
    );
    assert_noted(&reply, PASSED_OVER, "on the clear");
    assert_eq!(
        server.get(&format!("{P}/cards/C-0001")).status(200).json(),
        held,
        "the departed card as it was"
    );
}

/// The known residual, pinned as it stands: a card departed on one row and
/// suspect on another is taken: a row reading `suspect` is enough, no note, the
/// group holding it once. M: any `departed` row passing a card over.
#[test]
fn a_card_departed_on_one_row_and_suspect_on_another_is_taken() {
    let hub = teamed("team-departed-residual", "card-press").start();
    let id = ready(&hub, "Lead story", &["SHT-FRONT", "SHT-BACK"]);
    let mut body = departure("SHT-FRONT");
    body["revision"] = json!(revision(&hub, &id));
    hub.done("tomas", &format!("cards/{id}/accept"), &body);
    hand_edit(&hub, FRONT_CHANGE);
    hand_edit(&hub, BACK_CHANGE);

    let reply = hub.post(
        "olga",
        "trace/clear",
        &json!({"group": "PRN-WEEKLY", "reason": "A reprint.", "count": 1}),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["cleared"], json!([id]), "{}", reply.text());
    assert!(
        !reply.text().contains("passed over"),
        "no note: {}",
        reply.text()
    );
}

/// "Before their body is read": a login outside `[team]` is refused on the head:
/// a body no reader takes changes nothing of its answer, and a login on the team
/// meets the reader's 400, so the 403 is the first judgement's. M: dropping the
/// first judgement.
#[test]
fn the_first_judgement_comes_before_the_body() {
    let waiting = Waiting::new("team-first-judgement");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    for index in [0, 2, 3, 4, 5] {
        let act = Act {
            body: "{".to_owned(),
            ..waiting.acts()[index].clone()
        };
        assert_refused(&act.by(hub, "eve"), &outside_line("eve"), act.name);
        let reply = act.by(hub, act.lander);
        assert_eq!(reply.status, 400, "{}: {}", act.name, reply.text());
    }
    assert_eq!(hub.stored(&ids), before, "nothing stored");
}

/// A login taken off `[team]` while its body is on the way: the head passes the
/// first judgement, `[team]` is rewritten, the body arrives: a 403 naming the
/// login, nothing changed, on each act with a body. The same two steps with the
/// file left alone land, so the refusal is the rewrite's. The `100` is the
/// signal, no clock: past it the handler is reading the body. M: dropping the
/// second judgement.
#[test]
fn a_login_removed_between_the_headers_and_the_body_is_refused() {
    let waiting = Waiting::new("team-slow-body");
    let hub = &waiting.hub;
    let root = &hub.setup.root;
    let ids = waiting.ids();
    let good = read_text(root, "specengine.toml");
    let entry = |login: &str, name: &str, role: &str| {
        format!(
            "[team.{login}]\nname = \"{name}\"\nrole = \"{role}\"\nemail = \
             \"{login}@example.org\"\n"
        )
    };
    let without = |login: &str| {
        let line = match login {
            "olga" => entry("olga", "Olga R", "owner"),
            _ => entry("tomas", "Tomas B", "member"),
        };
        assert_eq!(good.matches(&line).count(), 1, "{login}'s entry");
        good.replace(&line, "")
    };
    let with_body = [0, 2, 3, 4, 5];

    let before = hub.stored(&ids);
    for index in with_body {
        let act = &waiting.acts()[index];
        let page = Proxied::write(act.lander);
        let (reply, continued) = through_in_two_steps(
            &hub.server,
            act.method,
            &act.path,
            &page,
            act.body.as_bytes(),
            || write(root, "specengine.toml", without(act.lander)),
        );
        assert!(continued, "{}: past the head: {}", act.name, reply.text());
        write(root, "specengine.toml", &good);
        assert_refused(&reply, &outside_line(act.lander), act.name);
        assert_eq!(hub.stored(&ids), before, "{}: nothing changed", act.name);
    }

    for index in with_body {
        let act = &waiting.acts()[index];
        let (reply, continued) = through_in_two_steps(
            &hub.server,
            act.method,
            &act.path,
            &Proxied::write(act.lander),
            act.body.as_bytes(),
            || {},
        );
        assert!(continued, "{}: past the head", act.name);
        assert_eq!(reply.status, 200, "{}: {}", act.name, reply.text());
    }
}

/// A group clear judges the entry read last: `lena` passes the head a `lead`
/// over the sheets, and past the `100` is a `member`, then a `lead` of the front
/// alone: each the refusal of what `[team]` says now, nothing cleared. Left as it
/// was, the same request clears. M: dropping the second judgement; the entry of
/// the first judgement kept.
#[test]
fn a_group_clear_judges_the_entry_read_after_the_body() {
    let waiting = Waiting::new("team-slow-group");
    let hub = &waiting.hub;
    let root = &hub.setup.root;
    let ids = waiting.ids();
    let good = read_text(root, "specengine.toml");
    let act = &waiting.acts()[5];
    let page = Proxied::write("lena");
    let keys = "role = \"lead\"\nzones = [\"sheets\"]\nemail = \"lena@";
    assert_eq!(good.matches(keys).count(), 1, "lena's keys");
    let before = hub.stored(&ids);

    let member = good.replace(keys, "role = \"member\"\nemail = \"lena@");
    let (reply, continued) = through_in_two_steps(
        &hub.server,
        act.method,
        &act.path,
        &page,
        act.body.as_bytes(),
        || write(root, "specengine.toml", &member),
    );
    assert!(continued, "past the head: {}", reply.text());
    write(root, "specengine.toml", &good);
    assert_refused(
        &reply,
        "refused: lena is a `member`: this act needs a `lead`",
        "a lead no longer",
    );
    assert_eq!(
        hub.stored(&ids),
        before,
        "a lead no longer: nothing changed"
    );

    let front = good.replace(
        keys,
        "role = \"lead\"\nzones = [\"front\"]\nemail = \"lena@",
    );
    let (reply, continued) = through_in_two_steps(
        &hub.server,
        act.method,
        &act.path,
        &page,
        act.body.as_bytes(),
        || write(root, "specengine.toml", &front),
    );
    assert!(continued, "past the head: {}", reply.text());
    write(root, "specengine.toml", &good);
    assert_group_refused(
        &reply,
        403,
        "SHT-INNER",
        "refused: lena leads front; docs/spec/sht-inner.md lies outside them",
        "a zone narrowed",
    );
    assert_eq!(hub.stored(&ids), before, "a zone narrowed: nothing changed");

    let (reply, continued) = through_in_two_steps(
        &hub.server,
        act.method,
        &act.path,
        &page,
        act.body.as_bytes(),
        || {},
    );
    assert!(continued, "past the head");
    assert_eq!(reply.status, 200, "{}", reply.text());
}

/// Reviewed behaviour: four accepts of one card at one `revision`, at once, by
/// its reviewer: one 200, three 409s, one event, on each of three cards: a
/// project's turn takes them one by one, the store's compare under it. M: the
/// turn and every compare under it gone.
#[test]
fn four_racing_accepts_give_one_winner() {
    let hub = teamed("team-race", "card-press").start();
    for round in 0..3 {
        let id = ready(&hub, "Back page", &["SHT-BACK"]);
        let body = json!({"briefed": true, "revision": revision(&hub, &id)}).to_string();
        let path = format!("{P}/cards/{id}/accept");
        let bytes = Proxied::write("tomas").bytes("POST", &path, body.as_bytes());
        let port = hub.server.port;
        let start = Barrier::new(4);
        let replies: Vec<Reply> = thread::scope(|scope| {
            let racers: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        http_exchange(port, &bytes, HTTP_TIMEOUT)
                    })
                })
                .collect();
            racers
                .into_iter()
                .map(|racer| racer.join().expect("a racer"))
                .collect()
        });
        let mut statuses: Vec<u16> = replies.iter().map(|reply| reply.status).collect();
        statuses.sort_unstable();
        assert_eq!(statuses, [200, 409, 409, 409], "round {round}: {replies:?}");
        let accepted = payloads(&hub, "card.accepted");
        assert_eq!(accepted.len(), round + 1, "round {round}: {accepted:?}");
        assert_eq!(accepted[round]["id"], json!(id), "{}", accepted[round]);
        assert_eq!(hub.show(&id)["column"], json!("struck"), "round {round}");
    }
}

/// Reviewed behaviour: `as` is no key of a stage or of a round's answers, the
/// login's own name included: the login is the `as` a hub logs, so a body never
/// names one: a 400 from the body's reader, nothing stored. M: `as` taken on a
/// stage body.
#[test]
fn as_on_a_stage_or_a_round_body_is_a_400() {
    let waiting = Waiting::new("team-stage-body");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    let acts = waiting.acts();
    let (stage, round) = (&acts[0], &acts[2]);
    for named in ["tomas", "mira"] {
        let mut in_answer = round.body_value();
        in_answer["answers"][0]["stage"]["as"] = json!(named);
        let in_answer = Act {
            body: in_answer.to_string(),
            ..round.clone()
        };
        for act in [
            stage.with("as", json!(named)),
            round.with("as", json!(named)),
            in_answer,
        ] {
            let reply = act.by(hub, "tomas");
            let context = format!("{} naming {named}: {}", act.name, act.body);
            assert_eq!(reply.status, 400, "{context}: {}", reply.text());
            let message = reply.error_message();
            assert!(!message.starts_with("refused: "), "{context}: {message}");
            assert!(message.contains("`as`"), "{context}: {message}");
            assert_eq!(hub.stored(&ids), before, "{context}: nothing stored");
        }
    }
}

// -------------------------------------------------------------------- A4

/// A4: two roots of two trades in one process, a team each: `tomas`, on the
/// print shop's alone, accepts there and is refused on `lantern-keep`; each root
/// reads its own table, whichever was read first, under each name; a commit's
/// worth of change is seen on the next request, in and out, with no restart.
/// M: a team cached in `App`.
#[test]
fn a4_each_root_reads_its_own_team_on_every_request() {
    let setup = teamed("team-two-roots", "card-press");
    let keep = setup.scratch.repo("spec-a", "keep", "main");
    let git = setup.scratch.git();
    git.run(&keep, &["config", "user.name", "Owner"]);
    git.run(&keep, &["config", "user.email", "owner@example.invalid"]);
    append_config(&keep, KEEP_TEAM);
    git.run(&keep, &["commit", "-qam", "the team"]);
    let question = ask(
        &setup.home,
        &keep,
        &["MEC-SPRINT", "EDGE-STAM-ZERO"],
        "Does a sprint end at zero?",
    );
    let server = Server::start(
        &setup.home,
        &setup.cwd,
        &hub_args(&[&keep, &setup.root], ORIGIN),
    );
    let hub = Hub { server, setup };
    let row = |login: &str, project: &str| {
        hub.get(login, &format!("{project}/team"))
            .status(200)
            .text()
            .to_owned()
    };
    let tomas_here = team_row("tomas", "Tomas B", &MEMBER_ACTS);
    for _ in 0..2 {
        assert_eq!(row("tomas", KEEP), no_row("tomas"), "the game first");
        assert_eq!(row("tomas", P), tomas_here, "then the print shop");
    }
    assert_eq!(row("mira", P), team_row("mira", "Mira K", &LEAD_ACTS));
    assert_eq!(
        row("mira", KEEP),
        team_row("mira", "Mira of the Keep", &LEAD_ACTS)
    );

    let id = ready(&hub, "Back page", &["SHT-BACK"]);
    let body = json!({"briefed": true, "revision": revision(&hub, &id)});
    let reply = through(
        &hub.server,
        "POST",
        &format!("{KEEP}/cards/C-0001/accept"),
        &Proxied::write("tomas"),
        body.to_string().as_bytes(),
    );
    assert_refused(&reply, &outside_line("tomas"), "tomas on the game");
    let accepted = hub.done("tomas", &format!("cards/{id}/accept"), &body);
    assert_eq!(accepted["column"], json!("struck"), "{accepted}");

    let stage_now = || {
        let read = hub
            .get("mira", &format!("{KEEP}/proposals/{question}"))
            .status(200)
            .json();
        json!({"decision": "reject", "reason": "Asked before.",
            "updated_at": read["updated_at"]})
        .to_string()
    };
    let stage = stage_now();
    let decision = format!("{KEEP}/proposals/{question}/decision");
    let decide = |login: &str, method: &str, body: &str| {
        through(
            &hub.server,
            method,
            &decision,
            &Proxied::write(login),
            body.as_bytes(),
        )
    };
    assert_refused(
        &decide("tomas", "POST", &stage),
        &outside_line("tomas"),
        "tomas stages on the game",
    );
    decide("mira", "POST", &stage).status(200);

    // No restart: the next request reads the file.
    let before = read_text(&keep, "specengine.toml");
    append_config(&keep, KEEP_TOMAS);
    assert_eq!(
        row("tomas", KEEP),
        team_row("tomas", "Tomas of the Keep", &MEMBER_ACTS)
    );
    decide("tomas", "DELETE", "").status(200);
    decide("mira", "POST", &stage_now()).status(200);
    write(&keep, "specengine.toml", before);
    assert_eq!(row("tomas", KEEP), no_row("tomas"));
    assert_refused(
        &decide("tomas", "DELETE", ""),
        &outside_line("tomas"),
        "tomas gone from the game",
    );
}

/// A4: a bad `[team]`, broken under a running hub: the six acts and `GET .../team`
/// are a 503 at the file's line, for a login on the table and off it, nothing
/// stored; the five writes and the reads go on, by the same logins; the mended
/// file is read at once. M: a team cached in `App`; a plain write reading
/// `[team]`.
#[test]
fn a4_a_bad_team_is_a_503_on_those_routes_and_plain_writes_stand() {
    let waiting = Waiting::new("team-bad");
    let hub = &waiting.hub;
    let root = &hub.setup.root;
    let ids = waiting.ids();
    let good = read_text(root, "specengine.toml");
    let bad = good.replace("role = \"member\"", "role = \"editor\"");
    assert_ne!(bad, good, "the member's role");
    write(root, "specengine.toml", bad.as_bytes());
    let opening = format!("specengine.toml:{}: ", line_holding(&bad, "\"editor\""));

    let before = hub.stored(&ids);
    let acts = waiting.acts();
    for login in ["olga", "tomas", "eve"] {
        for act in &acts {
            let reply = act.by(hub, login);
            let context = format!("{login} {}", act.name);
            assert_eq!(reply.status, 503, "{context}: {}", reply.text());
            let message = reply.error_message();
            assert!(message.starts_with(&opening), "{context}: {message}");
            assert_plain(&reply, &context);
            assert_eq!(hub.stored(&ids), before, "{context}: nothing stored");
        }
        let reply = hub.get(login, &format!("{P}/team"));
        assert_eq!(reply.status, 503, "{login}: {}", reply.text());
        let message = reply.error_message();
        assert!(message.starts_with(&opening), "{login}: {message}");
    }

    for (login, id) in [("tomas", "C-0004"), ("eve", "C-0005")] {
        for (path, body, _) in five_writes(id) {
            hub.done(login, &path, &body);
        }
    }
    let reads: Vec<String> = served_reads(&waiting.ready, &waiting.asked)
        .into_iter()
        .filter(|read| !read.ends_with("/team"))
        .collect();
    for path in &reads {
        let reply = hub.get("tomas", path);
        assert_eq!(
            reply.status,
            200,
            "GET {path}: {}",
            String::from_utf8_lossy(&reply.body)
        );
    }

    write(root, "specengine.toml", good.as_bytes());
    assert_eq!(
        hub.get("tomas", &format!("{P}/team")).status(200).text(),
        team_row("tomas", "Tomas B", &MEMBER_ACTS),
        "the mended file, at once"
    );
    let act = &waiting.acts()[0];
    let reply = act.by(hub, "tomas");
    assert_eq!(reply.status, 200, "{}", reply.text());
}

/// "Rules": a hub starts on a bad `[team]`: it is judged by the acts that need
/// it, never at the door of the process. M: `[team]` read at the start.
#[test]
fn a_hub_starts_on_a_bad_team() {
    let setup = Setup::new("team-bad-start", "card-press");
    append_config(&setup.root, BAD_TEAM);
    let git = setup.scratch.git();
    git.run(&setup.root, &["commit", "-qam", "a typo"]);
    let hub = setup.start();
    hub.done(
        "mira",
        "cards",
        &json!({"title": "Back page", "discipline": "plates"}),
    );
    let reply = hub.get("mira", &format!("{P}/team"));
    assert_eq!(reply.status, 503, "{}", reply.text());
}

/// "Data": a project with `[team]` and one with none answer the same act apart:
/// the table's own line names no login.
#[test]
fn no_team_is_its_own_line_on_the_six_acts() {
    let setup = Setup::new("team-none", "card-press");
    let asked = ask(
        &setup.home,
        &setup.root,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let staged = ask(
        &setup.home,
        &setup.root,
        &["SHT-INNER"],
        "Which typeface carries the inner column rules?",
    );
    let hub = setup.start();
    let revision_now = {
        hub.done(
            "ana",
            "cards",
            &json!({"title": "Back page", "discipline": "plates"}),
        );
        revision(&hub, "C-0001")
    };
    let stage = stage_body(&hub, &asked);
    let bodies = [
        (format!("proposals/{asked}/decision"), stage.clone()),
        (
            "round/answers".to_owned(),
            json!({"project": "card-press", "answers": [{"id": asked, "stage": stage}]}),
        ),
        (
            "cards/C-0001/accept".to_owned(),
            json!({"briefed": true, "revision": revision_now}),
        ),
        (
            "cards/C-0001/clear".to_owned(),
            json!({"outcome": "no_effect", "reason": "R.", "revision": revision_now}),
        ),
        (
            "trace/clear".to_owned(),
            json!({"group": "PRN-WEEKLY", "reason": "R.", "count": 1}),
        ),
    ];
    let ids = [asked.as_str(), staged.as_str()];
    let before = hub.stored(&ids);
    for (path, body) in &bodies {
        assert_refused(&hub.post("olga", path, body), NO_TEAM_LINE, path);
    }
    assert_refused(
        &delete(&hub, "olga", &format!("proposals/{staged}/decision")),
        NO_TEAM_LINE,
        "unstage",
    );
    assert_eq!(hub.stored(&ids), before, "nothing stored");
}

// -------------------------------------------------------------------- A5

/// A5: a query on the confirm class is a 403 in the fence, with a login that
/// would land the same request bare: `accept?x=/move`, the form a `$`-anchored
/// proxy rule misses, each other spelling, a bare `?`, and the absolute form of
/// a request line. The bare requests land last. M: `Hub::fenced` reading the
/// path alone.
#[test]
fn a5_a_query_on_the_confirm_class_is_refused_with_a_good_login() {
    let waiting = Waiting::new("team-query");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    let acts = waiting.confirm();
    for act in &acts {
        let page = Proxied::write(act.lander);
        for query in [
            "?x=/move",
            "?x=1",
            "?",
            "?revision=9",
            "?/move",
            "?&",
            "?a=b&c=d",
        ] {
            let path = format!("{}{query}", act.path);
            let reply = through(&hub.server, "POST", &path, &page, act.body.as_bytes());
            assert_refused(&reply, NO_QUERY_LINE, &format!("POST {path}"));
        }
        let path = format!("https://hub.example.test{}?x=1", act.path);
        let reply = through(&hub.server, "POST", &path, &page, act.body.as_bytes());
        assert_refused(&reply, NO_QUERY_LINE, &format!("POST {path}"));
    }
    assert_eq!(hub.stored(&ids), before, "no query reached a handler");

    for act in &acts {
        let reply = act.by(hub, act.lander);
        assert_eq!(reply.status, 200, "{}: {}", act.name, reply.text());
    }
}

/// A5: the confirm class needs an `Origin`: absent is a 403, a foreign or
/// differently spelled one the fence's other line, two of them refused, byte for
/// byte; `Sec-Fetch-Site: same-origin` and JSON stay asked. A team route and a
/// plain write keep `Origin` optional: only the acts taking a `Consent` are the
/// class. M: `Origin` left optional on the class; asked on every listed route.
#[test]
fn a5_no_origin_on_the_confirm_class_is_refused_and_only_there() {
    let waiting = Waiting::new("team-origin");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    for act in waiting.confirm() {
        let page = Proxied::write(act.lander);
        assert_refused(
            &act.proxied(hub, &page.clone().without("Origin")),
            NO_ORIGIN_LINE,
            act.name,
        );
        for origin in [
            "https://hub.example.test/",
            "https://HUB.example.test",
            "http://hub.example.test",
            "https://hub.example.test:443",
            "https://evil.example.test",
            "null",
        ] {
            let reply = act.proxied(hub, &page.clone().set("Origin", origin));
            assert_refused(&reply, WRONG_ORIGIN_LINE, &format!("{} {origin}", act.name));
        }
        assert_refused(
            &act.proxied(hub, &page.clone().plus("Origin", ORIGIN)),
            "refused: the Origin header is sent more than once",
            act.name,
        );
        let reply = act.proxied(hub, &page.clone().without("Sec-Fetch-Site"));
        assert_eq!(reply.status, 403, "{}: {}", act.name, reply.text());
        let reply = act.proxied(hub, &page.clone().set("Content-Type", "text/plain"));
        assert_eq!(reply.status, 415, "{}: {}", act.name, reply.text());
    }
    assert_eq!(hub.stored(&ids), before, "nothing stored");

    let stage = &waiting.acts()[0];
    let reply = stage.proxied(hub, &Proxied::write("tomas").without("Origin"));
    assert_eq!(reply.status, 200, "a team route: {}", reply.text());
    let reply = through(
        &hub.server,
        "POST",
        &format!("{P}/cards/{}/move", waiting.ready),
        &Proxied::write("ana").without("Origin"),
        json!({"to": "queued"}).to_string().as_bytes(),
    );
    assert_eq!(reply.status, 200, "a plain write: {}", reply.text());
}

/// A5, the routes this slice lists: exact raw segments, so no spelling of
/// `clear`, the group's, the round's or the stage's reaches its handler, with a
/// good login and a request that would land; the methods of each stay exact.
/// `accept`'s own table is `tests/hub.rs`. M: a listed route matched after
/// decoding; a trailing slash taken.
#[test]
fn a5_every_raw_form_of_a_listed_route_stays_refused() {
    let waiting = Waiting::new("team-raw");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    let asked = &waiting.asked;
    let staged = &waiting.staged;
    let acts = waiting.acts();
    let card = format!("{P}/cards/{}", waiting.suspect);
    let forms: Vec<(usize, Vec<String>)> = vec![
        (
            0,
            vec![
                format!("{P}/proposals/{asked}/decision/"),
                format!("{P}/proposals/{asked}//decision"),
                format!("{P}/proposals/{asked}/%64ecision"),
                format!("{P}/proposals/{asked}/DECISION"),
                format!("{P}/proposals/{asked}%2Fdecision"),
                format!("{P}/proposals/{asked}/decision;x"),
                format!("{P}/proposals/{asked}/decision%00"),
                format!("{P}/proposals/{asked}/x/../decision"),
                format!("{P}/proposals//decision"),
                "/api/projects//proposals/PR-0001/decision".to_owned(),
            ],
        ),
        (
            2,
            vec![
                format!("{P}/round/answers/"),
                format!("{P}/round//answers"),
                format!("{P}/round/%61nswers"),
                format!("{P}/round/ANSWERS"),
                format!("{P}/round%2Fanswers"),
                format!("{P}/round/answers;x"),
                format!("{P}/x/../round/answers"),
                "/api/projects//round/answers".to_owned(),
            ],
        ),
        (
            4,
            vec![
                format!("{card}/clear/"),
                format!("{card}//clear"),
                format!("{card}/%63lear"),
                format!("{card}/CLEAR"),
                format!("{card}/move/../clear"),
                format!("{card}/./clear"),
                format!("{card}%2Fclear"),
                format!("{card}/move%2Fclear"),
                format!("{card}/clear;x"),
                format!("{card}/clear%00"),
                format!("{P}/cards//clear"),
                "/api/projects//cards/C-0001/clear".to_owned(),
            ],
        ),
        (
            5,
            vec![
                format!("{P}/trace/clear/"),
                format!("{P}/trace//clear"),
                format!("{P}/trace/%63lear"),
                format!("{P}/trace/CLEAR"),
                format!("{P}/trace%2Fclear"),
                format!("{P}/trace/clear;x"),
                format!("{P}/x/../trace/clear"),
                "/api/projects//trace/clear".to_owned(),
            ],
        ),
    ];
    for (index, paths) in &forms {
        let act = &acts[*index];
        let page = Proxied::write(act.lander);
        for path in paths {
            let reply = through(&hub.server, "POST", path, &page, act.body.as_bytes());
            assert_refused(&reply, OFF_LIST_LINE, &format!("POST {path}"));
        }
    }
    for form in ["decision/", "%64ecision", "DECISION"] {
        let path = format!("{P}/proposals/{staged}/{form}");
        let reply = through(&hub.server, "DELETE", &path, &Proxied::write("tomas"), b"");
        assert_refused(&reply, OFF_LIST_LINE, &format!("DELETE {path}"));
    }
    assert_eq!(hub.stored(&ids), before, "no form reached a handler");
}

// -------------------------------------------------------------------- A6

/// A6: a null or absent `revision` on accept or clear and a null or absent
/// `count` on a group clear are a 400, each act's own line, nothing stored: a
/// hub asks what the page read. The request with it lands. M: dropping the
/// check.
#[test]
fn a6_a_null_revision_or_count_is_a_400_and_stores_nothing() {
    let waiting = Waiting::new("team-null");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    let acts = waiting.confirm();
    for act in &acts {
        let key = act.seen_key();
        let line = format!("a hub needs `{key}`");
        for form in [act.without(key), act.with(key, Value::Null)] {
            let reply = form.by(hub, form.lander);
            assert_eq!(reply.status, 400, "{}: {}", act.name, reply.text());
            assert_eq!(reply.error_message(), line, "{}", act.name);
            assert_plain(&reply, act.name);
            assert_eq!(hub.stored(&ids), before, "{}: nothing stored", act.name);
        }
    }
    for act in &acts {
        let reply = act.by(hub, act.lander);
        assert_eq!(reply.status, 200, "{}: {}", act.name, reply.text());
    }
}

/// A6: a count that differs from the set, above it or below it, is a 409 naming
/// both, no card cleared and no event; the right one clears both. A third card
/// marked while the dialog stood open is the case the compare is for: the same
/// body now differs. M: dropping the compare.
#[test]
fn a6_a_count_that_differs_is_a_409_and_no_card_is_cleared() {
    let hub = teamed("team-count", "card-press").start();
    let front = card_on(&hub, "Lead story", &["SHT-FRONT"]);
    let back = card_on(&hub, "Notices", &["SHT-BACK"]);
    let late = card_on(&hub, "Long read", &["SHT-INNER"]);
    hand_edit(&hub, FRONT_CHANGE);
    hand_edit(&hub, BACK_CHANGE);
    let before = hub.board();
    let body = |count: u64| json!({"group": "PRN-WEEKLY", "reason": "A reprint.", "count": count});
    for count in [1, 3, 0, 4096] {
        assert_group_refused(
            &hub.post("olga", "trace/clear", &body(count)),
            409,
            "PRN-WEEKLY",
            &format!("`PRN-WEEKLY` holds 2 suspect cards, not {count}; nothing changed"),
            &format!("count {count}"),
        );
        assert_eq!(hub.board(), before, "count {count}: nothing changed");
    }

    hand_edit(&hub, INNER_CHANGE);
    let before = hub.board();
    assert_group_refused(
        &hub.post("lena", "trace/clear", &body(2)),
        409,
        "PRN-WEEKLY",
        "`PRN-WEEKLY` holds 3 suspect cards, not 2; nothing changed",
        "a third mark since the dialog opened",
    );
    assert_eq!(hub.board(), before, "a stale count: nothing changed");

    let done = hub.done("lena", "trace/clear", &body(3));
    assert_eq!(done["cleared"], json!([front, back, late]), "{done}");
}

/// A6, beside it: a stale `revision` stays the CLI's 409 on a hub, on accept and
/// clear, nothing changed. M: `revision` read for presence alone.
#[test]
fn a_stale_revision_stays_a_409_on_a_hub() {
    let waiting = Waiting::new("team-stale");
    let hub = &waiting.hub;
    let ids = waiting.ids();
    let before = hub.stored(&ids);
    for act in waiting.confirm() {
        if act.seen_key() != "revision" {
            continue;
        }
        let reply = act.with("revision", json!("99")).by(hub, act.lander);
        assert_eq!(reply.status, 409, "{}: {}", act.name, reply.text());
        assert_eq!(hub.stored(&ids), before, "{}: nothing changed", act.name);
    }
}

// -------------------------------------------------------------------- A7

/// A7: with `[team]` in the file, good or bad, a local daemon stands as it was:
/// a typed `as` in no table accepts, clears and group-clears, no `revision`, no
/// `count`, no `Origin`; a stage, an unstage and a round's answers land; a
/// `Remote-User` is unread. M: judging `[team]` with no `Login`.
#[test]
fn a7_on_a_local_daemon_team_changes_nothing_for_a_typed_as() {
    for (label, table) in [("good", TEAM), ("bad", BAD_TEAM)] {
        let scratch = Scratch::new(&format!("team-local-{label}"));
        let root = scratch.repo("card-press", "served", "main");
        append_config(&root, table);
        let git = scratch.git();
        git.run(&root, &["commit", "-qam", "the team"]);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let asked = ask(
            &home,
            &root,
            &["SHT-BACK"],
            "Does the tide table keep its rule?",
        );
        let server = Server::serve(&home, &cwd, &[&root]);
        let headers = [
            ("Sec-Fetch-Site", "same-origin"),
            ("Content-Type", "application/json"),
            ("Remote-User", "eve"),
        ];
        let post = |path: &str, body: &Value| {
            let reply = server.send(
                "POST",
                &format!("{P}/{path}"),
                &headers,
                body.to_string().as_bytes(),
            );
            assert_eq!(reply.status, 200, "{label} {path} {body}: {}", reply.text());
            reply.json()
        };
        let edit = |(path, from, to): (&str, &str, &str)| {
            replace(&root, path, from, to);
            git.run(&root, &["add", "-A"]);
            git.run(&root, &["commit", "-q", "-m", "by hand"]);
        };

        for (title, target) in [
            ("Back page", "SHT-BACK"),
            ("Lead story", "SHT-FRONT"),
            ("Long read", "SHT-INNER"),
        ] {
            post(
                "cards",
                &json!({"title": title, "discipline": "plates", "assignee": "ana", "as": "ana",
                    "target": [target]}),
            );
        }
        post(
            "cards/C-0001/assign",
            &json!({"reviewer": "ben", "as": "ana"}),
        );
        post("cards/C-0001/move", &json!({"to": "checked", "as": "ana"}));
        edit(FRONT_CHANGE);
        edit(INNER_CHANGE);

        let accepted = post(
            "cards/C-0001/accept",
            &json!({"briefed": true, "as": "ben"}),
        );
        assert_eq!(accepted["column"], json!("struck"), "{label}: {accepted}");
        post(
            "cards/C-0002/clear",
            &json!({"outcome": "no_effect", "reason": "Only the weather.", "as": "ben"}),
        );
        let done = post(
            "trace/clear",
            &json!({"group": "SHT-INNER", "reason": "A reprint.", "as": "ben"}),
        );
        assert_eq!(done["cleared"], json!(["C-0003"]), "{label}: {done}");

        let at = |server: &Server| {
            server
                .get(&format!("{P}/proposals/{asked}"))
                .status(200)
                .json()["updated_at"]
                .clone()
        };
        let stage =
            json!({"decision": "reject", "reason": "Asked before.", "updated_at": at(&server)});
        post(&format!("proposals/{asked}/decision"), &stage);
        server
            .unstage(&format!("{P}/proposals/{asked}/decision"))
            .status(200);
        let stage =
            json!({"decision": "reject", "reason": "Asked before.", "updated_at": at(&server)});
        post(
            "round/answers",
            &json!({"project": "card-press", "answers": [{"id": asked, "stage": stage}]}),
        );

        let events = common::hub::events_of(&home, &root);
        let typed: Vec<&Value> = events
            .iter()
            .filter(|(kind, _)| matches!(kind.as_str(), "card.accepted" | "card.cleared"))
            .map(|(_, payload)| &payload["as"])
            .collect();
        assert_eq!(
            typed,
            [&json!("ben"), &json!("ben"), &json!("ben")],
            "{label}: {events:?}"
        );
    }
}

/// A7: on a local daemon `GET .../team` is the unknown-route 404, its line with
/// its own path, on every method, with `[team]` in the file or none: the route
/// is a hub's alone.
/// M: serving the row with no `Login`.
#[test]
fn a7_on_a_local_daemon_the_team_route_is_the_unknown_route_404() {
    for table in ["", TEAM] {
        let scratch = Scratch::new("team-local-route");
        let root = scratch.repo("card-press", "served", "main");
        append_config(&root, table);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let server = Server::serve(&home, &cwd, &[&root]);
        let path = format!("{P}/team");
        let reply = server.get(&path);
        assert_eq!(reply.status, 404, "{}", reply.text());
        let unknown = server.get("/no/such/route");
        assert_eq!(unknown.status, 404, "{}", unknown.text());
        assert_eq!(
            reply.error_message(),
            unknown.error_message().replace("/no/such/route", &path),
            "the unknown route's line"
        );
        let reply = server.request("GET", &path, &[("Remote-User", "mira")]);
        assert_eq!(reply.status, 404, "`Remote-User` unread: {}", reply.text());
        for method in ["POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            let reply = server.send(method, &path, &[("Sec-Fetch-Site", "same-origin")], b"");
            assert_eq!(reply.status, 404, "{method}");
        }
    }
}

// -------------------------------------------------------------------- A8

/// A8: a suspect mark blocks nothing: the suspect card moves, the reviewer
/// accepts it, the accept freezes it anew and the node reads `confirmed`.
/// M: a mark refusing a move or an accept (ADR-0012).
#[test]
fn a8_a_suspect_card_moves_and_is_accepted() {
    let hub = teamed("team-suspect", "card-press").start();
    let id = card_on(&hub, "Lead story", &["SHT-FRONT"]);
    hand_edit(&hub, FRONT_CHANGE);
    assert_eq!(rollup(&hub, "SHT-FRONT"), "suspect");

    let moved = hub.done(
        "ana",
        &format!("cards/{id}/move"),
        &json!({"to": "checked"}),
    );
    assert_eq!(moved["column"], json!("checked"), "{moved}");
    assert_eq!(
        rollup(&hub, "SHT-FRONT"),
        "suspect",
        "a move clears nothing"
    );

    let accepted = hub.done(
        "tomas",
        &format!("cards/{id}/accept"),
        &json!({"briefed": true, "revision": revision(&hub, &id)}),
    );
    assert_eq!(accepted["column"], json!("struck"), "{accepted}");
    assert_eq!(rollup(&hub, "SHT-FRONT"), "confirmed");
}

/// A8: a retarget of an open suspect card leaves its node `no card`, the new
/// target `in work`, and the entry kept, so a returning target marks again; a
/// cancel leaves `no card`. Neither reads `confirmed`, and neither made a
/// `card.cleared`. M: `retargeted` dropping a removed target's entry.
#[test]
fn a8_a_retarget_or_a_cancel_of_an_open_card_reads_no_card_never_confirmed() {
    let hub = teamed("team-retarget", "card-press").start();
    let retargeted = card_on(&hub, "Lead story", &["SHT-FRONT"]);
    let cancelled = card_on(&hub, "Notices", &["SHT-BACK"]);
    hand_edit(&hub, FRONT_CHANGE);
    hand_edit(&hub, BACK_CHANGE);
    assert_eq!(rollup(&hub, "SHT-FRONT"), "suspect");
    assert_eq!(rollup(&hub, "SHT-BACK"), "suspect");

    let edit = format!("cards/{retargeted}/edit");
    hub.done("ana", &edit, &json!({"target": ["SHT-INNER"]}));
    assert_eq!(rollup(&hub, "SHT-FRONT"), "no card", "a retarget");
    assert_eq!(rollup(&hub, "SHT-INNER"), "in work", "its new target");

    hub.done(
        "ana",
        &format!("cards/{cancelled}/cancel"),
        &json!({"reason": "Pulled for space."}),
    );
    assert_eq!(rollup(&hub, "SHT-BACK"), "no card", "a cancel");

    hub.done("ana", &edit, &json!({"target": ["SHT-FRONT"]}));
    assert_eq!(
        rollup(&hub, "SHT-FRONT"),
        "suspect",
        "the entry was kept: a returning target marks again"
    );
    assert_eq!(payloads(&hub, "card.cleared"), Vec::<Value>::new());
}

/// A8: an accepted card refuses `edit` and `cancel`, a 409 with nothing changed,
/// so a mark on it cannot leave but by a clear, and `confirmed` is never reached
/// by the back door. M: opening `edit` on a closed card.
#[test]
fn a8_an_accepted_card_refuses_edit_and_cancel() {
    let hub = teamed("team-closed", "card-press").start();
    let id = ready(&hub, "Lead story", &["SHT-FRONT"]);
    hub.done(
        "tomas",
        &format!("cards/{id}/accept"),
        &json!({"briefed": true, "revision": revision(&hub, &id)}),
    );
    assert_eq!(rollup(&hub, "SHT-FRONT"), "confirmed");
    hand_edit(&hub, FRONT_CHANGE);
    assert_eq!(rollup(&hub, "SHT-FRONT"), "suspect");

    let before = hub.board();
    for (path, body) in [
        (format!("cards/{id}/edit"), json!({"target": ["SHT-BACK"]})),
        (
            format!("cards/{id}/edit"),
            json!({"goal": "Fill the front."}),
        ),
        (
            format!("cards/{id}/cancel"),
            json!({"reason": "Pulled for space."}),
        ),
    ] {
        let reply = hub.post("ana", &path, &body);
        assert_eq!(reply.status, 409, "{path} {body}: {}", reply.text());
        assert_eq!(hub.board(), before, "{path} {body}: nothing changed");
    }
    assert_eq!(rollup(&hub, "SHT-FRONT"), "suspect", "the mark stands");
}
