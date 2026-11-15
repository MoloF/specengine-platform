//! docs/features/hub-auto.md (RM-0121) HA1, HA2, HA3, HA5, HA7: a hub
//! serving a git copy of `fixtures/auto-mini` (`[team]` names `ann`, the
//! sole `owner`, `ann@example.org`; `rules/hub.toml` committed: `docs`
//! covered, `narrative` and the `scenario` kind excluded), its proposals
//! raised over `/mcp/auto-mini` by the `common/mcp_http.rs` client as an
//! agent under a login. The delegated apply runs in the login's line after
//! the propose, so a later call of the same login is the barrier: once it
//! answers, the apply has run.
//!
//! - HA1: a covered `update` (and a `create` of a new `R-` section) by
//!   `ann` applies with no page request: one commit, author and committer
//!   `ann <ann@example.org>`, its trailers with `Delegated-by:` last, the
//!   events `by: "rule"`, the review document's `delegated`.
//! - HA2: each hold writes nothing but its one `auto.held` (a switch none):
//!   `narrative` (`excluded-zone`), an `SC-` section (`excluded-kind`),
//!   `notes/` (`uncovered`), a stored finding (`findings`), `mira` outside
//!   `[team]` (`author`), `max_applies = 1` (`cap`), a second `[team]`
//!   login (`team`), the stop file (`stopped`), a run refused by a dirty
//!   checkout (`rebases`, once); a question, a decision and a discrepancy
//!   are never judged (the seam hears only an `update` or
//!   a `create`): open, no event.
//! - HA3: a local daemon applies nothing (stdio MCP's update stays open).
//! - HA5: the table removed by a commit, or the stop file: the next covered
//!   proposal stays open with no restart; restored, the next applies.
//! - HA7: `delegated` on the delegated row alone (`spec review --json` and
//!   the route); a terminal-applied row and the open ones have no key.
//!
//! Not covered here: HA2's `staged` and `rebases` (on a hub the apply runs
//! right after its propose, which already refuses a stale base: no state
//! reaches either but by a race; the predicate's rows are
//! `crates/specengine-core/tests/auto_predicate.rs`); HA4 is
//! `tests/door.rs`, HA6 `tests/hub_fairness.rs`, HA8 the MCP crate's
//! `tests/mcp_hub.rs`, HA9 the owner's gate.

mod common;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::hub::{ORIGIN, Proxied, events_of, hub_args, through};
use common::mcp_http::{Session, content_text, is_error};
use common::slow::slow_ms;
use common::{Mcp, Reply, Scratch, Server, read_text, repository_root, spec, spec_json, write};
use serde_json::{Value, json};

const SLUG: &str = "auto-mini";
const ANN: &str = "ann <ann@example.org>";

const TEAM: &str = "[team.ann]\nrole  = \"owner\"\nemail = \"ann@example.org\"\n";

/// A hub over a git copy of auto-mini.
struct Hub {
    server: Server,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    scratch: Scratch,
}

fn rule(name: &str) -> String {
    read_text(&repository_root().join("fixtures/auto-mini/rules"), name)
}

impl Hub {
    /// `tables` appended to the fixture's config and committed, then served.
    fn new(name: &str, tables: &[&str]) -> Self {
        Self::with_args(name, tables, false)
    }

    /// As [`Hub::new`], the daemon at `--slow-ms 0` when `slow`.
    fn with_args(name: &str, tables: &[&str], slow: bool) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo("auto-mini", "a", "main");
        let git = scratch.git();
        git.run(&root, &["config", "user.name", "Owner"]);
        git.run(&root, &["config", "user.email", "owner@example.invalid"]);
        let mut config = read_text(&root, "specengine.toml").trim_end().to_owned();
        for table in tables {
            config.push_str("\n\n");
            config.push_str(table.trim_end());
        }
        config.push('\n');
        write(&root, "specengine.toml", config);
        git.run(&root, &["commit", "-qam", "the rule and the team"]);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let args = hub_args(&[&root], ORIGIN);
        let args = if slow { slow_ms(args, "0") } else { args };
        let server = Server::start(&home, &cwd, &args);
        Self {
            server,
            root,
            home,
            cwd,
            scratch,
        }
    }

    fn session(&self, login: &str) -> Session {
        Session::open(self.server.port, SLUG, login)
    }

    fn git(&self, args: &[&str]) -> String {
        self.scratch.git().run(&self.root, args).trim().to_owned()
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    fn events(&self) -> Vec<(String, Value)> {
        events_of(&self.home, &self.root)
    }

    fn held(&self) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|(kind, _)| kind == "auto.held")
            .map(|(_, payload)| payload)
            .collect()
    }

    fn review(&self, id: &str) -> Value {
        spec_json(&self.home, &self.cwd, &self.root, &["review", id]).json()
    }

    fn page(&self, login: &str, path: &str) -> Reply {
        through(&self.server, "GET", path, &Proxied::fetch(login), b"")
    }

    /// Every file of the checkout but git's own.
    fn files(&self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.file_name() == Some(OsStr::new(".git")) {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push((
                        path.strip_prefix(&self.root).unwrap().display().to_string(),
                        fs::read(&path).unwrap(),
                    ));
                }
            }
        }
        out.sort();
        out
    }

    /// The rule's hash at `HEAD`, as `spec auto show --surface hub` reads it.
    fn rule_hash(&self) -> String {
        let run = spec(
            &self.home,
            &self.root,
            &["--json", "auto", "show", "--surface", "hub"],
        );
        assert_eq!(run.code, 0, "{}", run.show());
        run.json()["auto"]["rule"]
            .as_str()
            .unwrap_or_else(|| panic!("no rule: {}", run.stdout))
            .to_owned()
    }
}

/// `tools/call`'s result (the JSON-RPC response's `result`).
fn call(session: &mut Session, name: &str, arguments: Value) -> Value {
    let response = session.call(name, arguments);
    response["result"].clone()
}

fn agent(mut args: Value) -> Value {
    args["author_role"] = json!("drafter");
    args["author_model"] = json!("m-1");
    args
}

/// `(span_hash, text)` of `id` over `/mcp`.
fn span(session: &mut Session, id: &str) -> (String, String) {
    let result = call(session, "get_node", json!({"id": id}));
    assert!(!is_error(&result), "{id}: {result}");
    let node = &result["structuredContent"]["nodes"][0];
    (
        node["span_hash"]
            .as_str()
            .unwrap_or_else(|| panic!("{result}"))
            .to_owned(),
        node["text"].as_str().expect("text").to_owned(),
    )
}

/// `propose_change` of `id`'s text with `from` become `to` (kind `kind`):
/// the stored ID.
fn propose(session: &mut Session, kind: &str, id: &str, from: &str, to: &str) -> String {
    let (base, text) = span(session, id);
    assert!(text.contains(from), "{id}: {from:?} in {text}");
    stored(
        session,
        "propose_change",
        agent(json!({"kind": kind, "target": id, "base": base,
            "text": text.replacen(from, to, 1), "rationale": "A test edit."})),
    )
}

/// A queue tool's call that must store: its ID (naming each hit in
/// `distinct_from` when the first call meets hits).
fn stored(session: &mut Session, tool: &str, args: Value) -> String {
    let result = call(session, tool, args.clone());
    assert!(!is_error(&result), "{tool}: {}", content_text(&result));
    let document = &result["structuredContent"];
    if let Some(id) = document["id"].as_str() {
        return id.to_owned();
    }
    let hits: Vec<Value> = document["hits"]
        .as_array()
        .unwrap_or_else(|| panic!("{tool}: not stored: {document}"))
        .iter()
        .map(|hit| {
            if hit["id"].is_string() {
                hit["id"].clone()
            } else {
                hit["path"].clone()
            }
        })
        .collect();
    let mut again = args;
    again["distinct_from"] = Value::Array(hits);
    let second = call(session, tool, again);
    second["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool}: not stored: {second}"))
        .to_owned()
}

/// The barrier: a read by the same login, behind the apply in its line.
fn settled(session: &mut Session) {
    let result = call(session, "get_tree", json!({}));
    assert!(!is_error(&result), "{result}");
}

/// HA1. M: `proposed` never called.
#[test]
fn ha1_a_covered_update_and_create_apply_with_no_click() {
    let hub = Hub::new("ha1", &[&rule("hub.toml"), TEAM]);
    let hash = hub.rule_hash();
    let mut ann = hub.session("ann");
    let before = hub.head();
    let id = propose(&mut ann, "update", "R-02", "five seconds", "six seconds");
    settled(&mut ann);
    let deadline = Instant::now() + Duration::from_secs(20);
    while hub.review(&id)["status"] != "applied" && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    let review = hub.review(&id);
    assert_eq!(review["status"], "applied", "{review}");
    let commit = review["applied_commit"]
        .as_str()
        .expect("a commit")
        .to_owned();
    assert_eq!(
        hub.git(&["rev-list", &format!("{before}..HEAD")]),
        commit,
        "one commit"
    );
    assert_eq!(
        hub.git(&["log", "-1", "--format=%an <%ae>|%cn <%ce>", &commit]),
        format!("{ANN}|{ANN}")
    );
    let message = hub.git(&["log", "-1", "--format=%B", &commit]);
    let trailers: Vec<&str> = message
        .lines()
        .filter(|line| {
            [
                "Proposal:",
                "Decided-by:",
                "Proposed-by:",
                "Base-commit:",
                "Delegated-by:",
            ]
            .iter()
            .any(|key| line.starts_with(key))
        })
        .collect();
    assert_eq!(trailers.len(), 5, "{message}");
    assert_eq!(trailers[0], format!("Proposal: {id}"));
    assert_eq!(trailers[1], format!("Decided-by: {ANN}"));
    assert!(
        trailers[2].starts_with("Proposed-by: agent ") && trailers[2].ends_with(" login=ann"),
        "{message}"
    );
    assert!(trailers[3].starts_with("Base-commit: "), "{message}");
    assert_eq!(
        message.lines().last(),
        Some(format!("Delegated-by: auto {hash}").as_str()),
        "last: {message}"
    );
    // The events: by rule, the hash, no pack.
    let decided: Vec<(String, Value)> = hub
        .events()
        .into_iter()
        .filter(|(kind, payload)| {
            (kind == "proposal.approved" || kind == "proposal.applied") && payload["id"] == id
        })
        .collect();
    assert_eq!(decided.len(), 2, "{decided:?}");
    for (kind, payload) in &decided {
        assert_eq!(payload["by"], "rule", "{kind}: {payload}");
        assert_eq!(payload["rule"], hash.as_str(), "{kind}");
        assert!(
            payload.get("pack").is_none(),
            "{kind}: no pack on a hub: {payload}"
        );
    }
    assert!(hub.held().is_empty());
    // The route's review document.
    let reply = hub.page("ann", &format!("/api/projects/{SLUG}/proposals/{id}"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.json()["delegated"],
        json!({"by": "rule", "rule": hash, "commit": commit})
    );

    // A create of a new R- section: likewise.
    let (base, text) = span(&mut ann, "R-01");
    let create = stored(
        &mut ann,
        "propose_change",
        agent(json!({"kind": "create", "target": "R-01", "base": base,
            "text": format!("{}\n\n### Slot names {{#R-100001}}\n\nEach slot has a name.\n",
                text.trim_end()),
            "rationale": "Names."})),
    );
    settled(&mut ann);
    let deadline = Instant::now() + Duration::from_secs(20);
    while hub.review(&create)["status"] != "applied" && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    let review = hub.review(&create);
    assert_eq!(review["status"], "applied", "{review}");
    assert!(read_text(&hub.root, "docs/spec/core.md").contains("{#R-100001}"));
    let message = hub.git(&["log", "-1", "--format=%B"]);
    assert!(
        message
            .trim_end()
            .ends_with(&format!("Delegated-by: auto {hash}")),
        "{message}"
    );
}

/// The raise of `args` by `login`'s session, then its barrier: the
/// stored ID, `HEAD`, the files and the held events unchanged but the new
/// holds, which are returned.
fn hold(
    hub: &Hub,
    session: &mut Session,
    raise: impl FnOnce(&mut Session) -> String,
) -> (String, Vec<Value>) {
    let head = hub.head();
    let files = hub.files();
    let held = hub.held().len();
    let id = raise(session);
    settled(session);
    // The apply ran in the line; give a held one's event a moment more.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(hub.review(&id)["status"], "open", "{id} stays open");
    assert_eq!(hub.head(), head, "{id}: HEAD unchanged");
    assert_eq!(hub.files(), files, "{id}: the files unchanged");
    (id, hub.held()[held..].to_vec())
}

fn one_hold(id: &str, new: &[Value], reason: &str, rule: &str) {
    assert_eq!(new.len(), 1, "{id}: one auto.held: {new:?}");
    assert_eq!(
        new[0],
        json!({"pack": null, "task": null, "line": null, "proposal": id, "act": "apply",
            "reason": reason, "detail": new[0]["detail"], "rule": rule}),
        "{id}"
    );
    assert!(new[0]["detail"].is_string());
}

/// HA2: each predicate's hold, one event each; the kinds the seam never
/// hears, none. M: any one predicate dropped.
#[test]
fn ha2_each_hold_writes_nothing_but_its_event() {
    let hub = Hub::new("ha2", &[&rule("hub.toml"), TEAM]);
    let hash = hub.rule_hash();
    let mut ann = hub.session("ann");
    let (id, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "SC-001", "at dusk", "at dawn")
    });
    one_hold(&id, &new, "excluded-zone", &hash);
    assert_eq!(
        new[0]["detail"],
        "`docs/narrative/intro.md` is in the excluded zone `narrative`"
    );
    let (id, new) = hold(&hub, &mut ann, |s| {
        let (base, text) = span(s, "R-02");
        stored(
            s,
            "propose_change",
            agent(json!({"kind": "create", "target": "R-02", "base": base,
                "text": format!("{}\n\n### A scene {{#SC-100001}}\n\nThe door creaks.\n",
                    text.trim_end()),
                "rationale": "A scene."})),
        )
    });
    one_hold(&id, &new, "excluded-kind", &hash);
    assert_eq!(
        new[0]["detail"],
        "`SC-100001` is a scenario, an excluded kind"
    );
    let (id, new) = hold(&hub, &mut ann, |s| {
        propose(
            s,
            "update",
            "R-03",
            "colour of the hour",
            "colour of the night",
        )
    });
    one_hold(&id, &new, "uncovered", &hash);
    assert_eq!(
        new[0]["detail"],
        "`notes/ideas.md` is in no zone the rule covers (docs)"
    );
    let (id, new) = hold(&hub, &mut ann, |s| {
        propose(
            s,
            "update",
            "R-01",
            "the time of its last save.",
            "the time of its last save (see R-99).",
        )
    });
    one_hold(&id, &new, "findings", &hash);
    // `mira`, a door login outside [team].
    let mut mira = hub.session("mira");
    let (id, new) = hold(&hub, &mut mira, |s| {
        propose(s, "update", "R-04", "save altar", "save shrine")
    });
    one_hold(&id, &new, "author", &hash);
    assert_eq!(
        new[0]["detail"],
        format!("`{id}` was not raised over /mcp by `ann`")
    );
    // The kinds the seam never hears: open, no event.
    for (tool, args) in [
        (
            "ask_question",
            json!({"node_ids": ["R-02"], "text": "Six seconds?", "working_answer": "Yes.",
                "price_of_other": "A rule case."}),
        ),
        (
            "report_decision",
            json!({"node_ids": ["R-02"], "text": "Does six hold?", "working_answer": "Yes.",
                "price_of_other": "A rule case."}),
        ),
    ] {
        let (_, new) = hold(&hub, &mut ann, |s| stored(s, tool, agent(args)));
        assert!(new.is_empty(), "{tool}: {new:?}");
    }
    let (_, new) = hold(&hub, &mut ann, |s| {
        stored(
            s,
            "report_discrepancy",
            agent(json!({
                "node_ids": ["R-05"], "summary": "The door never sticks.",
                "gap_type": "contradicts", "severity": "high",
                "evidence": [{"file": "notes/playtest.md", "lines": "3-9",
                    "observed": "opens dry", "documented": "sticks in the rain"}],
                "options": [{"label": "build", "effect": "follow the text", "price": "1 item"},
                    {"label": "text", "effect": "drop it", "price": "a line"}],
                "recommendation": 0
            })),
        )
    });
    assert!(new.is_empty(), "a discrepancy: {new:?}");
    // An interpretation (an update carrying the accepted DEC-0001) and an
    // amendment (a new file amending R-05): kinds of `propose_change` the
    // seam never hears.
    let (_, new) = hold(&hub, &mut ann, |s| {
        let (base, text) = span(s, "R-05");
        stored(
            s,
            "propose_change",
            agent(
                json!({"kind": "interpretation", "target": "R-05", "base": base,
                "text": text.replacen("sticks in the rain", "sticks when wet", 1),
                "rationale": "Read it so.",
                "decision": "DEC-0001", "how": "Wet means rain.", "why": "The words differ."}),
            ),
        )
    });
    assert!(new.is_empty(), "an interpretation: {new:?}");
    let (_, new) = hold(&hub, &mut ann, |s| {
        stored(
            s,
            "propose_change",
            agent(
                json!({"kind": "amendment", "target": "docs/spec/cellar-amend.md",
                "base": null,
                "text": "---\nclass: spec\nstatus: draft\nscope: [rooms]\nlinks:\n  amends: [R-05]\n---\n\n# The cellar, amended\n\nThe cellar door sticks in frost too.\n",
                "rationale": "Frost."}),
            ),
        )
    });
    assert!(new.is_empty(), "an amendment: {new:?}");
}

/// HA2's cap and switches: `max_applies = 1` with one rule apply today:
/// `cap`; a second `[team]` login: `team`, no event; the stop file:
/// `stopped`, no event. HA5: removed or restored, with no restart.
#[test]
fn ha2_ha5_the_cap_and_the_switches_act_on_the_next_proposal() {
    let capped = format!("{}max_applies   = 1\n", rule("hub.toml"));
    let hub = Hub::new("ha2-cap", &[&capped, TEAM]);
    let hash = hub.rule_hash();
    let mut ann = hub.session("ann");
    let first = propose(&mut ann, "update", "R-02", "five seconds", "six seconds");
    settled(&mut ann);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(hub.review(&first)["status"], "applied");
    let (id, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "R-04", "save altar", "save shrine")
    });
    one_hold(&id, &new, "cap", &hash);
    assert_eq!(
        new[0]["detail"],
        "1 delegated applies today (UTC), the cap max_applies 1"
    );

    // The stop file, written by `spec auto stop` on the hub's data directory.
    let hub = Hub::new("ha5-stop", &[&rule("hub.toml"), TEAM]);
    let mut ann = hub.session("ann");
    let stop = spec(&hub.home, &hub.cwd, &["auto", "stop"]);
    assert_eq!(stop.code, 0, "{}", stop.show());
    let (_, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "R-02", "five seconds", "six seconds")
    });
    assert!(new.is_empty(), "a switch logs nothing: {new:?}");
    let stop_file = stop
        .stdout
        .trim()
        .strip_prefix("stopped: ")
        .expect("the path")
        .to_owned();
    fs::remove_file(&stop_file).expect("the stop file");
    let next = propose(&mut ann, "update", "R-04", "save altar", "save shrine");
    settled(&mut ann);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        hub.review(&next)["status"],
        "applied",
        "resumed with no restart"
    );

    // The table removed by a commit: off; restored: on.
    let config = read_text(&hub.root, "specengine.toml");
    let without = config.replacen(&rule("hub.toml"), "", 1);
    assert_ne!(without, config);
    write(&hub.root, "specengine.toml", &without);
    hub.git(&["commit", "-qam", "auto off"]);
    let (_, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "R-05", "supplies", "stores")
    });
    assert!(new.is_empty(), "off logs nothing: {new:?}");
    write(&hub.root, "specengine.toml", &config);
    hub.git(&["commit", "-qam", "auto back"]);
    let next = propose(
        &mut ann,
        "update",
        "R-05",
        "Its door sticks",
        "Its door jams",
    );
    settled(&mut ann);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(hub.review(&next)["status"], "applied", "on with no restart");

    // A second [team] login: `team`, no event.
    let team = format!("{TEAM}\n[team.mira]\nrole  = \"member\"\nemail = \"mira@example.org\"\n");
    let hub = Hub::new("ha2-team", &[&rule("hub.toml"), &team]);
    let mut ann = hub.session("ann");
    let (_, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "R-02", "five seconds", "six seconds")
    });
    assert!(new.is_empty(), "a team logs nothing: {new:?}");
}

/// HA3: a local daemon serving the root with `rules/on.toml`: an update
/// raised over stdio MCP stays open, no `auto.*` event. M: `auto_apply`
/// reachable from a local daemon.
#[test]
fn ha3_a_local_daemon_applies_nothing() {
    let scratch = Scratch::new("ha3");
    let root = scratch.repo("auto-mini", "a", "main");
    let git = scratch.git();
    let config = read_text(&root, "specengine.toml");
    write(
        &root,
        "specengine.toml",
        format!("{}\n\n{}", config.trim_end(), rule("on.toml")),
    );
    git.run(&root, &["commit", "-qam", "the rule"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let mut mcp = Mcp::start(&home, &root);
    let shown = mcp.request(
        1,
        "tools/call",
        json!({"name": "get_node", "arguments": {"id": "R-02"}}),
    );
    let node = &shown["result"]["structuredContent"]["nodes"][0];
    let raised = mcp.request(
        2,
        "tools/call",
        json!({"name": "propose_change", "arguments": agent(json!({
            "kind": "update", "target": "R-02", "base": node["span_hash"],
            "text": node["text"].as_str().unwrap().replacen("five seconds", "six seconds", 1),
            "rationale": "Six."}))}),
    );
    let id = raised["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored: {raised}"))
        .to_owned();
    mcp.finish();
    // The daemon reads it (its tail) and still applies nothing.
    let reply = server.get(&format!("/api/projects/{SLUG}/proposals/{id}"));
    let _ = reply;
    std::thread::sleep(Duration::from_secs(2));
    let review = spec_json(&home, &cwd, &root, &["review", &id]).json();
    assert_eq!(review["status"], "open");
    assert!(review.get("delegated").is_none());
    let events = events_of(&home, &root);
    assert!(
        events.iter().all(|(kind, _)| !kind.starts_with("auto.")),
        "{events:?}"
    );
    drop(server);
}

/// HA7: `delegated` on the delegated row alone, in `spec review --json`
/// and the route; a terminal-applied row and the open ones have no key.
/// M: the key written `null` on every row.
#[test]
fn ha7_the_review_document_marks_the_delegated_row_alone() {
    let hub = Hub::new("ha7", &[&rule("hub.toml"), TEAM]);
    let hash = hub.rule_hash();
    let mut ann = hub.session("ann");
    let delegated = propose(&mut ann, "update", "R-02", "five seconds", "six seconds");
    let held = propose(&mut ann, "update", "SC-001", "at dusk", "at dawn");
    settled(&mut ann);
    std::thread::sleep(Duration::from_millis(300));
    let review = hub.review(&delegated);
    assert_eq!(review["status"], "applied");
    let commit = review["applied_commit"].as_str().unwrap().to_owned();
    let want = json!({"by": "rule", "rule": hash, "commit": commit});
    assert_eq!(review["delegated"], want);
    let text = spec(&hub.home, &hub.root, &["review", &delegated]);
    assert!(
        text.stdout
            .lines()
            .any(|line| line == format!("delegated: by rule {hash}, commit {}", &commit[..12])),
        "{}",
        text.stdout
    );
    let route = hub.page(
        "ann",
        &format!("/api/projects/{SLUG}/proposals/{delegated}"),
    );
    assert_eq!(route.json()["delegated"], want);
    // Over the hub's `/mcp` the review leaves it out, its text line too
    // (lane C's settled state: `tools/list` stays 0.46.1's).
    let over_mcp = call(&mut ann, "get_proposal", json!({"proposal_id": delegated}));
    assert!(!is_error(&over_mcp), "{over_mcp}");
    assert!(
        over_mcp["structuredContent"].get("delegated").is_none(),
        "{over_mcp}"
    );
    assert_eq!(over_mcp["structuredContent"]["status"], "applied");
    assert!(
        !content_text(&over_mcp).contains("delegated:"),
        "{}",
        content_text(&over_mcp)
    );
    // The held row: open, no key, on both.
    assert_eq!(hub.review(&held)["status"], "open");
    assert!(hub.review(&held).get("delegated").is_none());
    let route = hub.page("ann", &format!("/api/projects/{SLUG}/proposals/{held}"));
    assert!(route.json().get("delegated").is_none(), "{}", route.text());
    // A terminal-applied row: no key.
    let mut questions = Vec::new();
    let env = specengine_cli::Env {
        cwd: hub.root.clone(),
        home: Some(hub.home.clone().into_os_string()),
        xdg_data_home: None,
    };
    let outcome = specengine_cli::approve(
        &env,
        &specengine_cli::Globals::default(),
        &specengine_cli::ApproveRequest {
            id: held.clone(),
            note: None,
            now: "2026-11-16T08:00:00Z".to_owned(),
            git: specengine_cli::GitEnv::new(
                hub.root.as_path(),
                common::product_env(&hub.home)
                    .into_iter()
                    .chain([
                        ("GIT_AUTHOR_NAME".into(), "Owner".into()),
                        ("GIT_AUTHOR_EMAIL".into(), "owner@example.invalid".into()),
                        ("GIT_COMMITTER_NAME".into(), "Owner".into()),
                        ("GIT_COMMITTER_EMAIL".into(), "owner@example.invalid".into()),
                    ])
                    .collect::<Vec<_>>(),
            ),
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .expect("the terminal's approve");
    assert_eq!(
        outcome.document.status.as_deref(),
        Some("applied"),
        "{outcome:?}"
    );
    let terminal = hub.review(&held);
    assert_eq!(terminal["status"], "applied");
    assert!(terminal.get("delegated").is_none(), "{terminal}");
    let _ = Path::new("");
}

/// The daemon's one stderr line per delegated command
/// (docs/features/hub-auto.md "Data"), the slow line of the command (the
/// orchestrator's decision of 2026-11-15: `by=<login> id=<PR>
/// tool=auto_apply class=mcp`), and no line at all on a hub with no
/// `[auto]` table (its log 0.46.1's).
#[test]
fn ha1_ha2_the_daemons_lines() {
    let hub = Hub::with_args("ha-lines", &[&rule("hub.toml"), TEAM], true);
    let mut ann = hub.session("ann");
    let applied = propose(&mut ann, "update", "R-02", "five seconds", "six seconds");
    let held = propose(&mut ann, "update", "SC-001", "at dusk", "at dawn");
    settled(&mut ann);
    std::thread::sleep(Duration::from_millis(500));
    let commit = hub.review(&applied)["applied_commit"]
        .as_str()
        .expect("applied")
        .to_owned();
    let stderr = hub.server.stderr();
    let lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("specengine-http: auto "))
        .collect();
    assert_eq!(
        lines,
        [
            format!(
                "specengine-http: auto {SLUG} {applied} by ann: applied {}",
                &commit[..12]
            ),
            format!(
                "specengine-http: auto {SLUG} {held} by ann: held (excluded-zone): \
                 `docs/narrative/intro.md` is in the excluded zone `narrative`"
            ),
        ],
        "{stderr}"
    );
    for id in [&applied, &held] {
        assert!(
            stderr
                .lines()
                .any(|line| line.starts_with("specengine-http: slow turn ")
                    && line.contains(&format!(" by=ann id={id} tool=auto_apply "))
                    && line.ends_with(" class=mcp")),
            "the slow line of {id}'s apply: {stderr}"
        );
    }

    // No table: no line for it.
    let hub = Hub::new("ha-lines-none", &[TEAM]);
    let mut ann = hub.session("ann");
    let id = propose(&mut ann, "update", "R-02", "five seconds", "six seconds");
    settled(&mut ann);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(hub.review(&id)["status"], "open");
    let stderr = hub.server.stderr();
    assert!(
        !stderr.contains("specengine-http: auto "),
        "no [auto] table, no line: {stderr}"
    );
    assert!(hub.held().is_empty());
}

/// HA2's refused run: the hub's checkout holds an uncommitted change of the
/// target's file, so the apply's run cannot apply it: held `rebases`, logged
/// once, nothing written.
#[test]
fn ha2_a_refused_run_is_held_rebases_once() {
    let hub = Hub::new("ha2-refused", &[&rule("hub.toml"), TEAM]);
    let mut ann = hub.session("ann");
    let hash = hub.rule_hash();
    common::replace(
        &hub.root,
        "docs/spec/core.md",
        "A slot shows the time of its last save.",
        "A slot shows the date of its last save.",
    );
    let (id, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "R-02", "five seconds", "six seconds")
    });
    one_hold(&id, &new, "rebases", &hash);
    let all: Vec<Value> = hub
        .held()
        .into_iter()
        .filter(|held| held["proposal"] == id.as_str())
        .collect();
    assert_eq!(all.len(), 1, "logged once: {all:?}");
    assert!(
        hub.events()
            .iter()
            .all(|(kind, payload)| kind != "proposal.applied" || payload["id"] != id.as_str())
    );
}

/// HA6: under the `probes` build, as `tests/hub_fairness.rs` runs it, with
/// the rule on: 8 covered proposals raised by `ann` in parallel (4 updates,
/// 4 new files) while a page reads the Inbox: every page request answers
/// within 10 s and each of the 8 is applied once. M: the command given a
/// line of its own outside the login's (the page waits behind every apply).
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_auto --run-ignored ignored-only"]
fn ha6_the_page_stays_served_during_a_burst_of_applies() {
    let hub = Hub::new("ha6", &[&rule("hub.toml"), TEAM]);
    let mut ann = hub.session("ann");
    let mut calls: Vec<Value> = Vec::new();
    for (id, from, to) in [
        ("R-01", "three save slots", "four save slots"),
        ("R-02", "five seconds", "six seconds"),
        ("R-04", "save altar", "save shrine"),
        ("R-05", "supplies", "stores"),
    ] {
        let (base, text) = span(&mut ann, id);
        calls.push(agent(json!({"kind": "update", "target": id, "base": base,
            "text": text.replacen(from, to, 1), "rationale": "A burst."})));
    }
    for k in 1..=4 {
        calls.push(agent(json!({"kind": "create", "target": format!("docs/spec/burst-{k}.md"),
            "text": format!("---\nclass: canon\ntitle: Burst {k}\nowner: owner\nreviewed: 2026-11-15\n---\n\n# Burst {k}\n\nThe {k}th note of a burst.\n"),
            "rationale": "A burst."})));
    }
    let port = hub.server.port;
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let raisers: Vec<_> = calls
        .into_iter()
        .map(|args| {
            std::thread::spawn(move || {
                // Stateless calls: a login holds at most four sessions.
                let mut client = common::mcp_http::Stateless::new(port, SLUG, "ann");
                let first = client.request(
                    "tools/call",
                    json!({"name": "propose_change", "arguments": args.clone()}),
                );
                let document = &first["result"]["structuredContent"];
                if let Some(id) = document["id"].as_str() {
                    return id.to_owned();
                }
                let hits: Vec<Value> = document["hits"]
                    .as_array()
                    .unwrap_or_else(|| panic!("not stored: {first}"))
                    .iter()
                    .map(|hit| {
                        if hit["id"].is_string() {
                            hit["id"].clone()
                        } else {
                            hit["path"].clone()
                        }
                    })
                    .collect();
                let mut again = args;
                again["distinct_from"] = Value::Array(hits);
                let second = client.request(
                    "tools/call",
                    json!({"name": "propose_change", "arguments": again}),
                );
                second["result"]["structuredContent"]["id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("not stored: {second}"))
                    .to_owned()
            })
        })
        .collect();
    let page = {
        let done = std::sync::Arc::clone(&done);
        let server_port = port;
        std::thread::spawn(move || {
            let mut slowest = Duration::ZERO;
            let mut reads = 0;
            while !done.load(std::sync::atomic::Ordering::SeqCst) {
                let started = Instant::now();
                let reply = common::http_exchange(
                    server_port,
                    &Proxied::fetch("ann").bytes(
                        "GET",
                        &format!("/api/projects/{SLUG}/inbox"),
                        b"",
                    ),
                    Duration::from_secs(30),
                );
                slowest = slowest.max(started.elapsed());
                reads += 1;
                assert_eq!(reply.status, 200, "{}", reply.text());
                std::thread::sleep(Duration::from_millis(50));
            }
            (slowest, reads)
        })
    };
    let ids: Vec<String> = raisers.into_iter().map(|h| h.join().unwrap()).collect();
    let deadline = Instant::now() + Duration::from_secs(120);
    while ids.iter().any(|id| hub.review(id)["status"] != "applied") && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let (slowest, reads) = page.join().unwrap();
    assert!(reads > 0);
    assert!(
        slowest < Duration::from_secs(10),
        "a page read took {slowest:?}"
    );
    for id in &ids {
        assert_eq!(hub.review(id)["status"], "applied", "{id}");
        let applied = hub
            .events()
            .into_iter()
            .filter(|(kind, payload)| kind == "proposal.applied" && payload["id"] == id.as_str())
            .count();
        assert_eq!(applied, 1, "{id} applied once");
    }
}

/// The round-2 review's row: a hold where nothing ran (`excluded-zone`)
/// leaves the index trusted: the read after it is served `index=trusted`
/// (the slow line's field) as the read before it was.
#[test]
fn ha2_a_hold_leaves_the_index_trusted() {
    let hub = Hub::with_args("ha2-trust", &[&rule("hub.toml"), TEAM], true);
    let mut ann = hub.session("ann");
    settled(&mut ann);
    settled(&mut ann);
    let index_of_tree = |hub: &Hub| -> Vec<String> {
        hub.server
            .stderr()
            .lines()
            .filter(|line| {
                line.starts_with("specengine-http: slow request ")
                    && line.contains(" tool=get_tree ")
            })
            .filter_map(|line| line.split(" index=").nth(1))
            .map(|rest| rest.split(' ').next().unwrap_or_default().to_owned())
            .collect()
    };
    let before = index_of_tree(&hub);
    assert_eq!(
        before.last().map(String::as_str),
        Some("trusted"),
        "{before:?}"
    );
    let (id, new) = hold(&hub, &mut ann, |s| {
        propose(s, "update", "SC-001", "at dusk", "at dawn")
    });
    assert_eq!(new[0]["reason"], "excluded-zone", "{id}");
    settled(&mut ann);
    let after = index_of_tree(&hub);
    assert_eq!(
        after.last().map(String::as_str),
        Some("trusted"),
        "the read after the hold: {after:?}\n{}",
        hub.server.stderr()
    );
}
