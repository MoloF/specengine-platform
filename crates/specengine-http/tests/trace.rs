//! docs/features/card-trace.md, the daemon's half: `GET .../trace?ref=`, the
//! browser view, `POST .../cards/:id/clear`, `POST .../trace/clear` and a
//! departing `POST .../accept`, through the real daemon over git repositories
//! of `fixtures/card-press`.
//!
//! - AC-09, the route: every form is `spec trace --json`'s bytes, suspect and
//!   departed rows a 200, a REF naming nothing the CLI's exit-1 document.
//!   Past the CLI's cut the daemon serves every row.
//! - AC-10: from a same-origin page a clear of each outcome and a group clear
//!   finish, the typed name in the event; no `Sec-Fetch-Site` is a 403 and no
//!   event.
//! - "Accepting", the page: the choice and its keys; a dedup hit refuses
//!   nothing; the item is made in the accept's transaction, git untouched.
//! - "Clearing", the refusals: a card not suspect, a stale revision, a body
//!   the CLI cannot decode.
//!
//! The door's scan and the whole-state compare are `door.rs`; methods, fence,
//! keys their own files. Every expected value below is worked out from the
//! spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::path::{Path, PathBuf};

use common::{Reply, Scratch, Server, encode_component, replace, spec, spec_json};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};

const P: &str = "/api/projects/card-press";

/// A full name in another script, as "Acting as" takes it.
const FULL_NAME: &str =
    "\u{0410}\u{043d}\u{043d}\u{0430} \u{041f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";

/// `same_origin()`'s line ("The fence stays").
const PAGE: &str = "a write needs a same-origin page (not authentication: ADR-0034)";

const FRONT_MD: &str = "docs/spec/sht-front.md";
const BACK_MD: &str = "docs/spec/sht-back.md";
const INNER_MD: &str = "docs/spec/sht-inner.md";
const RULING_MD: &str = "docs/records/STY/STY-0001.md";

/// A write's or a refusal's document, sorted.
const WRITE_KEYS: [&str; 7] = [
    "allowed", "column", "flow", "id", "notes", "position", "revision",
];

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

    /// `GET trace?ref=reference`, a 200.
    fn trace(&self, reference: &str) -> Value {
        self.get(&format!("trace?ref={}", encode_component(reference)))
            .status(200)
            .json()
    }

    /// A card of the `copy` flow on `targets`, `ana` its assignee, `ben` its
    /// reviewer, at `proof`: its ID.
    fn in_proof(&self, title: &str, targets: &[&str]) -> String {
        let id = self.make(title, targets);
        self.done(
            &format!("cards/{id}/assign"),
            &json!({"assignee": "ana", "reviewer": "ben"}),
        );
        self.done(
            &format!("cards/{id}/move"),
            &json!({"to": "edit", "as": "desk"}),
        );
        self.done(&format!("cards/{id}/move"), &json!({"to": "proof"}));
        id
    }

    /// A card of the `copy` flow on `targets`: its ID.
    fn make(&self, title: &str, targets: &[&str]) -> String {
        self.done(
            "cards",
            &json!({"title": title, "discipline": "copy", "target": targets}),
        )["id"]
            .as_str()
            .expect("an ID")
            .to_owned()
    }

    /// [`Press::in_proof`], accepted as briefed: its ID.
    fn accepted(&self, title: &str, targets: &[&str]) -> String {
        let id = self.in_proof(title, targets);
        self.done(
            &format!("cards/{id}/accept"),
            &json!({"as": "ben", "briefed": true}),
        );
        id
    }

    /// `from` for `to` in the root's `path`, by hand, committed: nobody
    /// vouched for it.
    fn hand_edit(&self, path: &str, from: &str, to: &str) {
        replace(&self.root, path, from, to);
        let git = self.scratch.git();
        git.run(&self.root, &["add", "-A"]);
        git.run(&self.root, &["commit", "-q", "-m", "by hand"]);
    }

    /// Every stored event, `(type, payload)`.
    fn events(&self) -> Vec<(String, Value)> {
        events_of(&self.home, &self.root)
    }

    /// The payloads of `kind`.
    fn events_of(&self, kind: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|(event, _)| event == kind)
            .map(|(_, payload)| payload)
            .collect()
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

    /// `path` with `body` answered `status`, nothing changed; a 409 is the
    /// write document with the reason last of `notes`.
    fn refused(&self, status: u16, path: &str, body: &Value) -> Reply {
        let before = self.state();
        let reply = self.post(path, body);
        let context = format!("{path} {body}");
        assert_eq!(reply.status, status, "{context}: {}", reply.text());
        if status == 409 {
            let document = reply.json();
            let mut keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
            keys.sort();
            assert_eq!(keys, WRITE_KEYS, "{context}: {document}");
            assert!(
                document["notes"]
                    .as_array()
                    .is_some_and(|notes| !notes.is_empty()),
                "{context}: the reason: {document}"
            );
        }
        assert_eq!(self.state(), before, "{context}: nothing changed");
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

/// The node `id` of a trace document.
fn node_of<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == id)
        .unwrap_or_else(|| panic!("no node {id}: {document}"))
}

/// A departure's keys, beside `as`.
fn departing(nodes: &[&str]) -> Value {
    json!({
        "as": "ben",
        "departs": nodes,
        "summary": "The result left the brief.",
        "observed": "The lead runs below the fold.",
        "documented": "The lead runs above the fold.",
    })
}

/// A board with a confirmed, a suspect and a departed node, and one in work.
fn marked(name: &str) -> Press {
    let press = Press::new(name);
    press.accepted("Lead story", &["SHT-FRONT"]);
    let notices = press.in_proof("Notices", &["SHT-BACK"]);
    press.done(
        &format!("cards/{notices}/accept"),
        &json!({"as": "ben", "briefed": true}),
    );
    let long = press.in_proof("Long read", &["SHT-INNER"]);
    press.done(&format!("cards/{long}/accept"), &departing(&["SHT-INNER"]));
    press.make("Masthead", &["SHT-FRONT-HEAD"]);
    press.hand_edit(BACK_MD, "the tide table", "the moon table");
    press
}

// --------------------------------------------------------------- AC-09

/// The daemon's `path` against `spec --root root <args> --json` (exit 0 ->
/// 200, exit 1 -> 404): the same bytes, the JSON headers, the same bytes on a
/// second request.
fn same(press: &Press, path: &str, args: &[&str]) -> Reply {
    let run = spec_json(&press.home, &press.cwd, &press.root, args);
    let reply = press.get(path);
    let want = if run.code == 0 { 200 } else { 404 };
    assert_eq!(
        reply.status,
        want,
        "{path} vs spec {args:?} (exit {}): {}",
        run.code,
        reply.text()
    );
    assert_eq!(reply.text(), run.document(), "{path}: the CLI's bytes");
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8"),
        "{path}"
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
    let again = press.get(path);
    assert_eq!(
        (again.status, again.text()),
        (reply.status, reply.text()),
        "{path}: a repeat answers the same bytes"
    );
    reply
}

/// AC-09: every form of the route is the CLI's document, suspect and
/// departed rows a 200, and a read writes nothing.
#[test]
fn ac09_every_trace_is_the_clis_document_byte_for_byte() {
    let press = marked("trace-parity");
    let git = press.git_side();
    let state = press.state();
    for (query, args) in [
        ("trace", vec!["trace"]),
        ("trace?ref=PRN-WEEKLY", vec!["trace", "PRN-WEEKLY"]),
        ("trace?ref=SHT-FRONT", vec!["trace", "SHT-FRONT"]),
        ("trace?ref=SHT-BACK", vec!["trace", "SHT-BACK"]),
        ("trace?ref=SHT-INNER", vec!["trace", "SHT-INNER"]),
        (
            "trace?ref=docs%2Fspec%2Fsht-inner.md",
            vec!["trace", INNER_MD],
        ),
        (
            "trace?ref=SHT-FRONT%23SHT-FRONT-HEAD",
            vec!["trace", "SHT-FRONT#SHT-FRONT-HEAD"],
        ),
    ] {
        let reply = same(&press, query, &args);
        assert_eq!(reply.status, 200, "{query}");
    }
    let shown = press.trace("PRN-WEEKLY");
    assert_eq!(node_of(&shown, "SHT-BACK")["rollup"], "suspect");
    assert_eq!(node_of(&shown, "SHT-INNER")["rollup"], "departed");
    assert_eq!(node_of(&shown, "SHT-FRONT")["rollup"], "in work");
    assert_eq!(press.git_side(), git, "a read touches no file");
    assert_eq!(press.state(), state, "a read writes nothing");
}

/// The query: a REF naming nothing and `ref=` empty are the CLI's exit-1
/// document, a 404; a bad query a 400 naming the parameter, before anything
/// runs; a look-alike a 503 with the CLI's line.
#[test]
fn the_query_of_the_route() {
    let press = Press::new("trace-query");
    for (query, args) in [
        ("trace?ref=SHT-NOPE", vec!["trace", "SHT-NOPE"]),
        ("trace?ref=", vec!["trace", ""]),
    ] {
        assert_eq!(same(&press, query, &args).status, 404, "{query}");
    }
    for (query, parameter) in [
        ("trace?ref=SHT-FRONT&ref=SHT-BACK", "ref"),
        ("trace?x=1", "x"),
        ("trace?root=SHT-FRONT", "root"),
        ("trace?depth=1", "depth"),
    ] {
        let reply = press.get(query);
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        assert!(
            reply.error_message().contains(parameter),
            "{query}: {}",
            reply.error_message()
        );
    }
    let look_alike = "S\u{041d}T-FRONT";
    let root = press.root.to_str().expect("UTF-8 root");
    let run = spec(
        &press.home,
        &press.cwd,
        &["--root", root, "trace", look_alike, "--json"],
    );
    run.code(2);
    let line = run.stderr.trim_end_matches('\n');
    let reply = press.get(&format!("trace?ref={}", encode_component(look_alike)));
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line);
}

/// "browser view": past the CLI's cut the daemon serves every node's every
/// card, `truncated` false, while the CLI's nodes are the same rows cut.
#[test]
fn past_the_cap_the_daemon_serves_every_row() {
    let press = Press::new("trace-uncut");
    let title = "x".repeat(250);
    for _ in 0..200 {
        press.make(&title, &["SHT-FRONT"]);
    }
    let run = spec_json(&press.home, &press.cwd, &press.root, &["trace"]);
    run.code(0);
    let cli = run.json();
    assert_eq!(cli["truncated"], json!(true), "{}", cli["omitted"]);
    let served = press.get("trace").status(200).json();
    assert_eq!(served["truncated"], json!(false));
    assert_eq!(served["omitted"], Value::Null);
    assert_eq!(
        node_of(&served, "SHT-FRONT")["cards"]
            .as_array()
            .expect("cards")
            .len(),
        200
    );
    assert_eq!(served["counts"], cli["counts"], "the counts stay whole");
}

// --------------------------------------------------------------- AC-10

/// AC-10: a clear of each outcome and a group clear finish from a same-origin
/// page, the typed name, any script, in each event; the correction is made;
/// git stays as it was; no proposal is made.
#[test]
fn ac10_each_clear_finishes_from_a_page_the_typed_name_in_the_event() {
    let press = Press::new("trace-clear");
    let lead = press.make("Lead story", &["SHT-FRONT"]);
    let notices = press.accepted("Notices", &["SHT-BACK"]);
    let long = press.make("Long read", &["SHT-INNER"]);
    let fold = press.make("Fold", &["SHT-INNER-FOLD"]);
    let outside = press.make("Outside", &["STY-0001"]);
    press.hand_edit(
        FRONT_MD,
        "the lead story.",
        "the lead story and the weather.",
    );
    press.hand_edit(BACK_MD, "the tide table", "the moon table");
    press.hand_edit(
        INNER_MD,
        "carries the long reads",
        "carries the short reads",
    );
    press.hand_edit(INNER_MD, "No headline crosses", "One headline crosses");
    press.hand_edit(RULING_MD, "whatever else runs", "unless a storm runs");
    let git = press.git_side();

    // No effect.
    let answer = press.done(
        &format!("cards/{lead}/clear"),
        &json!({"outcome": "no_effect", "reason": "Only the weather.", "as": FULL_NAME}),
    );
    assert_eq!(answer["id"], lead.as_str());
    let cleared = press.events_of("card.cleared");
    assert_eq!(cleared.len(), 1, "{cleared:?}");
    assert_eq!(cleared[0]["id"], lead.as_str());
    assert_eq!(cleared[0]["outcome"], "no_effect");
    assert_eq!(cleared[0]["reason"], "Only the weather.");
    assert_eq!(cleared[0]["as"], FULL_NAME, "the typed name");
    assert_eq!(cleared[0]["correction"], Value::Null);
    assert_eq!(
        node_of(&press.trace("SHT-FRONT"), "SHT-FRONT")["rollup"],
        "in work"
    );

    // Needs correction.
    let answer = press.done(
        &format!("cards/{notices}/clear"),
        &json!({"outcome": "needs_correction", "reason": "The table moved.",
            "estimate": "3", "title": "Fix the notices", "as": FULL_NAME}),
    );
    assert_eq!(answer["id"], notices.as_str(), "{answer}");
    assert_eq!(answer["column"], "filed", "the original stays: {answer}");
    let cleared = press.events_of("card.cleared");
    assert_eq!(cleared.len(), 2, "{cleared:?}");
    assert_eq!(cleared[1]["id"], notices.as_str());
    assert_eq!(cleared[1]["outcome"], "needs_correction");
    assert_eq!(cleared[1]["as"], FULL_NAME, "the typed name");
    let fix = cleared[1]["correction"].as_str().expect("the correction");
    let card = press.show(fix);
    assert_eq!(card["corrects"], notices.as_str());
    assert_eq!(card["title"], "Fix the notices");
    assert_eq!(card["estimate"], "3");
    assert_eq!(
        node_of(&press.trace("SHT-BACK"), "SHT-BACK")["rollup"],
        "in work"
    );

    // The group.
    let answer = press.done(
        "trace/clear",
        &json!({"group": "PRN-WEEKLY", "reason": "A reprint.", "as": FULL_NAME}),
    );
    assert_eq!(
        answer,
        json!({"group": "PRN-WEEKLY", "cleared": [long.as_str(), fold.as_str()], "notes": []}),
    );
    let cleared = press.events_of("card.cleared");
    let ids: Vec<&str> = cleared[2..]
        .iter()
        .map(|event| event["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, [long.as_str(), fold.as_str()], "one event each");
    for event in &cleared[2..] {
        assert_eq!(event["outcome"], "no_effect", "{event}");
        assert_eq!(event["as"], FULL_NAME, "the typed name");
    }
    assert_eq!(
        node_of(&press.trace("STY-0001"), "STY-0001")["rollup"],
        "suspect",
        "{outside}: outside the group"
    );

    assert_eq!(press.git_side(), git, "git as it was");
    assert_eq!(
        press.events_of("proposal.created"),
        Vec::<Value>::new(),
        "no proposal"
    );
}

/// AC-10: no `Sec-Fetch-Site`, `none`, `cross-site`, `same-site`, a foreign
/// `Origin`: 403, no event, nothing read. M: a POST without `same_origin`.
#[test]
fn ac10_no_page_is_a_403_and_no_event() {
    let press = Press::new("trace-door");
    let id = press.make("Lead story", &["SHT-FRONT"]);
    press.hand_edit(
        FRONT_MD,
        "the lead story.",
        "the lead story and the weather.",
    );
    let state = press.state();
    let json_type = ("Content-Type", "application/json");
    let same = ("Sec-Fetch-Site", "same-origin");
    for (path, body) in [
        (
            format!("{P}/cards/{id}/clear"),
            json!({"outcome": "no_effect", "reason": "R.", "as": "ben"}),
        ),
        (
            format!("{P}/trace/clear"),
            json!({"group": "PRN-WEEKLY", "reason": "R.", "as": "ben"}),
        ),
    ] {
        let bytes = body.to_string();
        for headers in [
            &[json_type][..],
            &[json_type, ("Sec-Fetch-Site", "none")][..],
            &[json_type, ("Sec-Fetch-Site", "cross-site")][..],
            &[json_type, ("Sec-Fetch-Site", "same-site")][..],
            &[same, json_type, ("Origin", "http://evil.example")][..],
        ] {
            let reply = press.server.send("POST", &path, headers, bytes.as_bytes());
            assert_eq!(reply.status, 403, "{path} {headers:?}: {}", reply.text());
            assert!(reply.cors_headers().is_empty(), "{reply:?}");
            assert_eq!(press.state(), state, "{path} {headers:?}");
        }
        let reply = press
            .server
            .send("POST", &path, &[json_type], bytes.as_bytes());
        assert_eq!(reply.error_message(), PAGE, "{path}");
        // The door's order: JSON next.
        let reply = press.server.send(
            "POST",
            &path,
            &[same, ("Content-Type", "text/plain")],
            bytes.as_bytes(),
        );
        assert_eq!(reply.status, 415, "{path}: {}", reply.text());
        assert_eq!(press.state(), state, "{path}");
    }
}

/// "Clearing", the page: a card not suspect, no such card and a stale
/// `revision` answer as their CLI exit does; a body the CLI cannot decode is a
/// 400; an accepted-only correction of an open card a 409; a group naming
/// nothing a 404.
#[test]
fn the_refusals_of_a_clear() {
    let press = Press::new("trace-refused");
    let clean = press.make("Clean", &["SHT-BACK"]);
    let id = press.make("Lead story", &["SHT-FRONT"]);
    press.hand_edit(
        FRONT_MD,
        "the lead story.",
        "the lead story and the weather.",
    );
    let clear = |card: &str| format!("cards/{card}/clear");
    let no_effect = json!({"outcome": "no_effect", "reason": "R.", "as": "ben"});
    press.refused(409, &clear(&clean), &no_effect);
    press.refused(
        409,
        &clear(&id),
        &json!({"outcome": "no_effect", "reason": "R.", "revision": "9", "as": "ben"}),
    );
    press.refused(
        409,
        &clear(&id),
        &json!({"outcome": "needs_correction", "reason": "R.", "estimate": "3", "as": "ben"}),
    );
    press.refused(404, &clear("C-0099"), &no_effect);
    for body in [
        json!({"reason": "R.", "as": "ben"}),
        json!({"outcome": "no_effect", "as": "ben"}),
        json!({"outcome": "bogus", "reason": "R.", "as": "ben"}),
        json!({"outcome": "needs_correction", "reason": "R.", "as": "ben"}),
        json!({"outcome": "no_effect", "reason": "R.", "as": "ben", "owner": "ana"}),
    ] {
        press.refused(400, &clear(&id), &body);
    }
    for body in [
        json!({"reason": "R.", "as": "ben"}),
        json!({"group": "PRN-WEEKLY", "as": "ben"}),
        json!({"group": "PRN-WEEKLY", "reason": "R.", "as": "ben", "outcome": "no_effect"}),
    ] {
        press.refused(400, "trace/clear", &body);
    }
    press.refused(
        404,
        "trace/clear",
        &json!({"group": "SHT-NOPE", "reason": "R.", "as": "ben"}),
    );

    // No name is `local`, who clears nothing: a person's act; a blank reason;
    // a group with nothing suspect under it, its document `{group, cleared,
    // notes}`.
    for (body, why) in [
        (
            json!({"outcome": "no_effect", "reason": "R."}),
            "--as: `local` clears no mark: a person answers one; nothing changed",
        ),
        (
            json!({"outcome": "no_effect", "reason": "R.", "as": "agent"}),
            "--as: `agent` clears no mark: a person answers one; nothing changed",
        ),
        (
            json!({"outcome": "no_effect", "reason": "   ", "as": "ben"}),
            "reason: blank; say why the mark clears",
        ),
    ] {
        let reply = press.refused(409, &clear(&id), &body);
        assert_eq!(
            reply.json()["notes"]
                .as_array()
                .and_then(|notes| notes.last()),
            Some(&json!(why)),
            "{body}"
        );
    }
    for body in [
        json!({"group": "SHT-BACK", "reason": "R.", "as": "ben"}),
        json!({"group": "PRN-WEEKLY", "reason": "R."}),
    ] {
        let before = press.state();
        let reply = press.post("trace/clear", &body);
        assert_eq!(reply.status, 409, "{body}: {}", reply.text());
        let document = reply.json();
        assert_eq!(document["cleared"], json!([]), "{document}");
        assert_eq!(document["group"], body["group"], "{document}");
        assert!(
            document["notes"]
                .as_array()
                .is_some_and(|notes| !notes.is_empty()),
            "{document}"
        );
        assert_eq!(press.state(), before, "{body}: nothing changed");
    }
}

// ------------------------------------------------------------- accepting

/// "Accepting", the page: `card_accept` makes the item: one `open`
/// discrepancy, events `proposal.created`, `card.accepted`, git as it was, the
/// node `departed`; a dedup hit refuses nothing, the item names it, a note
/// lists it.
#[test]
fn a_departure_from_a_page_makes_its_item() {
    let press = Press::new("trace-departs");
    let id = press.in_proof("Lead story", &["SHT-FRONT"]);
    let git = press.git_side();
    let answer = press.done(&format!("cards/{id}/accept"), &departing(&["SHT-FRONT"]));
    assert!(
        answer["notes"].to_string().contains("STY-0001"),
        "a note lists it: {answer}"
    );
    assert_eq!(press.git_side(), git, "git is read, only read");
    let card = press.show(&id);
    assert_eq!(card["acceptance"]["verdict"], "departs");
    assert_eq!(card["acceptance"]["departs"], json!(["SHT-FRONT"]));
    let item = card["acceptance"]["discrepancy"]
        .as_str()
        .expect("the item");
    assert_eq!(item, "PR-0001");
    let kinds: Vec<String> = press.events().into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(
        kinds[kinds.len() - 2..],
        ["proposal.created", "card.accepted"],
        "{kinds:?}"
    );
    let review = press.get(&format!("proposals/{item}")).status(200).json();
    assert_eq!(review["status"], "open", "{review}");
    assert_eq!(review["distinct_from"], json!(["STY-0001"]), "{review}");
    assert_eq!(
        node_of(&press.trace("SHT-FRONT"), "SHT-FRONT")["rollup"],
        "departed"
    );
}

/// "Accepting", the page: one of the two, else a 400, as the CLI's exit 2;
/// nothing changed.
#[test]
fn an_accept_from_a_page_takes_one_verdict() {
    let press = Press::new("trace-verdict");
    let id = press.in_proof("Long read", &["SHT-INNER"]);
    let accept = format!("cards/{id}/accept");
    press.refused(400, &accept, &json!({"as": "ben"}));
    let mut both = departing(&["SHT-INNER"]);
    both["briefed"] = json!(true);
    press.refused(400, &accept, &both);
    press.refused(
        400,
        &accept,
        &json!({"as": "ben", "departs": ["SHT-INNER"], "summary": "S."}),
    );
    press.done(&accept, &json!({"as": "ben", "briefed": true}));
    assert_eq!(press.show(&id)["acceptance"]["verdict"], "briefed");
}
