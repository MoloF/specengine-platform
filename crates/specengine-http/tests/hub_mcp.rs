//! docs/features/hub-mcp.md (RM-0053, Phase 7 step 11), the daemon's half: a
//! hub serves MCP's Streamable HTTP at `/mcp/<slug>` under each request's
//! login, in both protocol eras, with seventeen tools that read, raise
//! proposals and write cards and decide nothing. Through the real daemon over
//! git repositories of `fixtures/spec-a` (`lantern-keep`, a game's brief, with
//! three card flows and an owner `tomas` committed) and `fixtures/spec-b`
//! (`zerkalo`, a worktree tool's spec in another script), so the route is
//! tried on two projects of different nature (ADR-0008). The client is
//! `common/mcp_http.rs`: raw HTTP/1.1 on loopback through the test proxy's
//! headers (`common/hub.rs`), no client crate. Every expected value is worked
//! out from the spec, never read back from a run.
//!
//! - H1: the fence and the route: no, two or a bad `Remote-User`, another
//!   method, a query, the paths around it, an unknown slug; a local daemon's
//!   unknown-route 404 on every method. M: mounting it on a local daemon;
//!   judging the query after GET's return.
//! - H2: a legacy session: `initialize`, its one id, `notifications/initialized`
//!   202, a GET stream still open after 20 s with a keep-alive, the 17 tools, a
//!   DELETE and then rmcp's 404 on every method. M: `legacy_session_mode:
//!   false`; rmcp's default `allowed_hosts`.
//! - H3: a 2026-07-28 call alone: the tree, no session; `server/discover` is
//!   `HUB_INSTRUCTIONS`. M: the hub served as `Lifecycle::Legacy`.
//! - H4: a session is its login's and its project's: another login, another
//!   project, a never-issued id, all one 404, nothing run. M: no binding;
//!   binding by id alone.
//! - H5: `Author.login` the request's, `Proposed-by` and `Decided-by` on a
//!   confirm; the stdio twins store no `login`. M: the login dropped; taken from
//!   an argument.
//! - H6: no `login`, `as`, `actor` or `task_id` in a hub schema, each refused as
//!   an argument by all seven writes. M: `task_id` kept in the hub schema.
//! - H7: a card write acts as the login, and a flow's `by` holds against it;
//!   over stdio as `agent`. M: `TOOL_ACTOR` on the hub.
//! - H8: one door: all 17 tools, valid and invalid, both eras, `dry_run`, both
//!   resources; the repository untouched, every new path under `HOME`. M: a
//!   hub tool writing under the root; a new tool with no call.
//! - H9: the 17 and no deciding word. M: `review_proposal` or `report_drift` in
//!   the hub set.
//! - H10: the turn: a confirm held in its `pre-commit` hook holds its project's
//!   reads, not the other project's, nor the handshake, the list or a ping. M:
//!   the call outside the turn; `tools/list` inside it.
//! - H11, the wire's half: the instructions and the descriptions' budgets and
//!   words, the ten shared descriptions the stdio ones. M: `claim_task` named in
//!   `HUB_INSTRUCTIONS`.
//! - H13: `Authorization` outside `/mcp` is a 403 at step 5, on `/mcp` it is
//!   unread; a local daemon never reads it. M: the check dropped; refusing it on
//!   `/mcp` too.
//! - H14: parity with `specengine-mcp --root`, the cut and its tail. M: the hub
//!   reading with `View::Browser`.
//! - H15, the daemon's half: a discrepancy raised over `/mcp` reaches a page's
//!   tail within 1 s and the inbox. M: the hub server on another data
//!   directory.
//! - H16: no issued session id in the dump, the log, any file under the root
//!   or `HOME`, or the daemon's stderr. M: `run` defaulting to the session id.
//! - H17: four sessions a login a project, the least recently used leaving;
//!   64 a hub, the 65th a 503; a DELETE frees a place; a restart ends them all.
//!   M: no cap; evicting another login's session; counting ended sessions.
//! - H18 (iteration 2): two `Mcp-Session-Id` values in one request, on every
//!   method, are the unknown-session 404, and both sessions live on. M:
//!   unbinding every named value and handing the request to rmcp.
//! - H19 (iteration 2): 1 001 unknown tool names in both eras are rmcp's
//!   `-32602` `tool not found`, nothing stored. M: handing them to rmcp (its
//!   unit half, the counting seam, kills that one).
//! - H20 (iteration 2): a call cancelled while it waits for the turn, by a
//!   closed connection or `notifications/cancelled`, stores nothing. M: the
//!   wait ignoring the cancellation.
//!
//! Elsewhere: H12 is a unit of `src/hub.rs`; H17's idle, H18's count and H19's
//! counting seam units of `src/mcp.rs`; H9's `door.rs` scan is
//! `tests/door.rs`; H11's genre scan `crates/specengine-mcp/tests/mcp_genre.rs`
//! and its constants
//! `crates/specengine-mcp/tests/mcp_hub.rs`; H15's card is
//! `ui/src/inbox/ProposalCard.test.tsx`; `GET_ROUTES` and the fence's order are
//! `tests/hub.rs`; the graph pins `crates/specengine-eval/tests/build_graph.rs`.
//!
//! The spec's `https://hub.example.org` is the harness's `ORIGIN`,
//! `https://hub.example.test`: the fence's lines name it.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::hub::{ORIGIN, Proxied, assert_refused, events_of, hub_args, through};
use common::mcp_http::{
    Answer, BAD_ARGUMENTS, Client, ERAS, Era, LEGACY, NEVER_ISSUED, NO_SESSION, QUICK,
    SESSION_HEADER, STATELESS, Session, Stateless, content_text, exchange, is_error, mcp_path,
    message, post, proxied, result_of, tool, tool_names,
};
use common::{
    HTTP_TIMEOUT, Mcp, Reply, Scratch, Server, Stream, http_exchange, propose_update, read_text,
    snapshot, spec, spec_json, write,
};
use serde_json::{Value, json};

const KEEP: &str = "lantern-keep";
const ZERKALO: &str = "zerkalo";

/// The hub's tools, sorted (docs/features/hub-mcp.md "Data"): seventeen,
/// eighteen since docs/features/department-split.md "Acceptance criteria", S8
/// (`split_card`).
const HUB_TOOLS: [&str; 18] = [
    "ask_question",
    "create_card",
    "edit_card",
    "get_card",
    "get_context_bundle",
    "get_flow",
    "get_node",
    "get_proposal",
    "get_roadmap",
    "get_trace",
    "get_tree",
    "list_cards",
    "move_card",
    "propose_change",
    "report_decision",
    "report_discrepancy",
    "search",
    "split_card",
];

/// What a hub's `tools/list` names, sorted: [`HUB_TOOLS`], and in the
/// measurement build its probes (`specengine_mcp::HUB_PROBES`, empty in
/// every other build; docs/features/read-trust.md D9).
fn hub_listed() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = HUB_TOOLS
        .iter()
        .chain(specengine_mcp::HUB_PROBES.iter())
        .copied()
        .collect();
    names.sort_unstable();
    names
}

/// The twelve stdio tools a hub leaves out, sorted.
const LEFT_OUT: [&str; 14] = [
    "check_binding",
    "claim_task",
    "complete_task",
    "find_symbols",
    // docs/features/collision-pass.md "Acceptance criteria", CP8 (D3): the
    // pass keeps a local cache a hub does not serve.
    "get_collisions",
    "get_impact",
    "get_task",
    "mark_pairs",
    "refs",
    "report_drift",
    "report_run",
    "review_proposal",
    "submit_plan",
    "unmapped",
];

/// The nine tools whose schemas are the stdio twin's, and whose description
/// is too but for [`HUB_READ_TEXTED`]'s read tail.
const SHARED: [&str; 9] = [
    "get_card",
    "get_context_bundle",
    "get_flow",
    "get_node",
    "get_proposal",
    "get_trace",
    "get_tree",
    "list_cards",
    "search",
];

/// The shared reads whose hub text is the stdio twin's with its read tail
/// replaced (docs/features/read-trust.md, review iteration 3): they trust
/// the hub's index.
const HUB_READ_TEXTED: [&str; 4] = ["get_context_bundle", "get_node", "get_tree", "search"];

/// Stdio's read tail and the hub's in its place.
const STDIO_READ_TAIL: &str = "Reads the files as they are now: refreshes SpecEngine's index in \
                               its data directory, writes nothing under the project root. \
                               Deterministic: one state, one result; no LLM inside.";
const HUB_READ_TAIL: &str = "Reads the hub's index, which each confirm and merge updates and a \
                             read checks against the files when the last check is 5 minutes \
                             old; a file it prints is checked as it is now. Writes nothing \
                             under the project root. Deterministic: one state, one result; no \
                             LLM inside.";

/// The eight whose descriptions are the hub's variants (docs/features/hub-mcp.md
/// "Data", iteration 2): `get_roadmap`, the four queue writes and the three card
/// writes; department-split S8: `split_card`, a fourth card write.
const VARIANTS: [&str; 9] = [
    "ask_question",
    "create_card",
    "edit_card",
    "get_roadmap",
    "move_card",
    "propose_change",
    "report_decision",
    "report_discrepancy",
    "split_card",
];

/// The four queue writes and the four card writes (`split_card` since
/// department-split S8).
const QUEUE_WRITES: [&str; 4] = [
    "ask_question",
    "propose_change",
    "report_decision",
    "report_discrepancy",
];
const CARD_WRITES: [&str; 4] = ["create_card", "edit_card", "move_card", "split_card"];

/// Words no hub tool's name may hold (H9).
const DECIDING: [&str; 11] = [
    "approve", "reject", "stage", "accept", "clear", "confirm", "claim", "drift", "review",
    "round", "task",
];

/// The keys no hub schema holds (H6).
const IDENTITY_KEYS: [&str; 4] = ["login", "as", "actor", "task_id"];

/// The sentence every description holds.
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

/// The card writes' sentence on a hub (docs/features/hub-mcp.md "Data").
const ACTS_AS_LOGIN: &str = "You act as your person's login: a move a flow gives to named \
                             people refuses you unless it lists that login";

/// Claude Code's cut of an `instructions` or a description.
const TEXT_LIMIT: usize = 2048;

/// The fence's lines (docs/canon/hub.md "Fence", docs/canon/hub-team.md
/// "Answers") and this slice's ("Data").
const NO_LOGIN_LINE: &str = "refused: this hub needs one Remote-User header from its proxy";
const TWO_LOGINS_LINE: &str = "refused: the Remote-User header is sent more than once";
const OFF_LIST_LINE: &str = "refused: this hub does not serve this route";
const NO_QUERY_LINE: &str = "refused: this route takes no query";
const AUTHORIZATION_LINE: &str = "refused: the Authorization header is for /mcp alone: this route takes the login door's session";
const CAP_LINE: &str = "refused: this hub holds 64 MCP sessions, its cap: try again once one ends";

/// `mira:x`, in Basic.
const BASIC: &str = "Basic bWlyYTp4";

/// The agent "Data" raises a proposal as, and what is stored of it.
const ROLE: &str = "drafter";
const MODEL: &str = "m-1";
const RUN: &str = "r-1";

/// The game's three card flows and its owner, committed before any hub runs:
/// `lore`'s first move is `tomas`'s, `quests`' is `mira`'s, `sketch` lists no
/// move, so every pair of its open columns stands (docs/canon/cards.md
/// "Config").
const KEEP_TABLES: &str = r#"
[[card.flow]]
name = "lore"
disciplines = ["lore"]
columns = ["draft", "read", "kept"]
accept = { from = "read", to = "kept" }
moves = [
  { from = "draft", to = "read", by = ["tomas"] },
  { from = "read", to = "draft" },
]

[[card.flow]]
name = "quests"
disciplines = ["quests"]
columns = ["draft", "read", "kept"]
accept = { from = "read", to = "kept" }
moves = [
  { from = "draft", to = "read", by = ["mira"] },
  { from = "read", to = "draft" },
]

[[card.flow]]
name = "sketch"
disciplines = ["sketch"]
columns = ["open", "shaped", "kept"]
accept = { from = "shaped", to = "kept" }

[zones]
sprint = ["docs/spec/movement/sprint.md"]

[team.tomas]
role = "owner"
email = "tomas@example.org"
"#;

/// `tomas`'s commit identity.
const TOMAS: &str = "tomas <tomas@example.org>";

// ------------------------------------------------------------------ a world

/// A hub serving `lantern-keep` and `zerkalo`. `server` is first: it stops
/// before the scratch goes.
struct World {
    server: Server,
    keep: PathBuf,
    zerkalo: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    scratch: Scratch,
}

impl World {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let keep = scratch.repo("spec-a", "keep", "main");
        let zerkalo = scratch.repo("spec-b", "zerkalo", "main");
        let git = scratch.git();
        for root in [&keep, &zerkalo] {
            git.run(root, &["config", "user.name", "Owner"]);
            git.run(root, &["config", "user.email", "owner@example.invalid"]);
        }
        let mut config = read_text(&keep, "specengine.toml");
        config.push_str(KEEP_TABLES);
        write(&keep, "specengine.toml", config);
        git.run(&keep, &["commit", "-qam", "the flows and the team"]);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let server = Self::hub(&home, &cwd, &keep, &zerkalo);
        Self {
            server,
            keep,
            zerkalo,
            home,
            cwd,
            scratch,
        }
    }

    fn hub(home: &Path, cwd: &Path, keep: &Path, zerkalo: &Path) -> Server {
        Server::start(home, cwd, &hub_args(&[keep, zerkalo], ORIGIN))
    }

    fn port(&self) -> u16 {
        self.server.port
    }

    /// SIGTERM, reaped, and a new hub on the same roots and `HOME`.
    fn restart(&mut self) {
        self.server.signal_and_reap("TERM");
        self.server = Self::hub(&self.home, &self.cwd, &self.keep, &self.zerkalo);
    }

    fn session(&self, slug: &str, login: &str) -> Session {
        Session::open(self.port(), slug, login)
    }

    fn stateless(&self, slug: &str, login: &str) -> Stateless {
        Stateless::new(self.port(), slug, login)
    }

    /// Every event of both projects' queues.
    fn events(&self) -> Vec<(String, Value)> {
        let mut events = events_of(&self.home, &self.keep);
        events.extend(events_of(&self.home, &self.zerkalo));
        events
    }

    /// `git status`, `HEAD`, the refs, the worktrees and every file of `root`
    /// but git's own index.
    fn repository(&self, root: &Path) -> Vec<String> {
        let git = self.scratch.git();
        let files: Vec<String> = snapshot(root)
            .into_iter()
            .filter(|(path, _)| path != ".git/index")
            .map(|(path, bytes)| format!("{path} {bytes:?}"))
            .collect();
        vec![
            git.run(
                root,
                &[
                    "status",
                    "--porcelain=v1",
                    "-z",
                    "--untracked-files=all",
                    "--ignored",
                ],
            ),
            git.run(root, &["rev-parse", "HEAD"]),
            git.run(root, &["for-each-ref", "--format=%(refname) %(objectname)"]),
            git.run(root, &["worktree", "list", "--porcelain"]),
            files.join("\n"),
        ]
    }

    /// `spec review <id> --json` on `lantern-keep`.
    fn review(&self, id: &str) -> Value {
        spec_json(&self.home, &self.cwd, &self.keep, &["review", id]).json()
    }

    /// A page's `GET path` by `login`.
    fn page(&self, login: &str, path: &str) -> Reply {
        through(&self.server, "GET", path, &Proxied::fetch(login), b"")
    }

    /// A page's `POST path` of `body` by `login`.
    fn page_post(&self, login: &str, path: &str, body: &Value) -> Reply {
        through(
            &self.server,
            "POST",
            path,
            &Proxied::write(login),
            body.to_string().as_bytes(),
        )
    }

    /// `tomas` stages an approve of `id` and confirms it in one click: the
    /// review document of the 200.
    fn confirmed_by_tomas(&self, id: &str) -> Value {
        let base = format!("/api/projects/{KEEP}/proposals/{id}");
        let stage = json!({"decision": "approve", "updated_at": self.review(id)["updated_at"]});
        let staged = self.page_post("tomas", &format!("{base}/decision"), &stage);
        assert_eq!(staged.status, 200, "the stage: {}", staged.text());
        let read = self.page("tomas", &format!("{base}/confirm"));
        assert_eq!(read.status, 200, "the read: {}", read.text());
        let read = read.json();
        let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
        let reply = self.page_post("tomas", &format!("{base}/confirm"), &body);
        assert_eq!(reply.status, 200, "the confirm: {}", reply.text());
        reply.json()
    }

    /// The values of `key` in the last commit's message of `lantern-keep`.
    fn trailers(&self, key: &str) -> Vec<String> {
        let prefix = format!("{key}: ");
        self.scratch
            .git()
            .run(&self.keep, &["log", "-1", "--format=%B"])
            .lines()
            .filter_map(|line| line.strip_prefix(prefix.as_str()))
            .map(str::to_owned)
            .collect()
    }
}

/// The unknown-route 404's message for `path`, worked out from the server's
/// answer to a path it surely does not serve, as B8 of hub-confirm does.
fn unknown_route_message(of: &Reply, path: &str) -> String {
    of.error_message().replace("/no/such/route", path)
}

/// An agent's identity arguments, `run` included.
fn agent() -> Value {
    json!({"author_role": ROLE, "author_model": MODEL, "run": RUN})
}

/// `base` with `extra`'s keys added.
fn with(base: Value, extra: &Value) -> Value {
    let mut out = base;
    for (key, value) in extra.as_object().expect("an object") {
        out[key] = value.clone();
    }
    out
}

/// A tool result's document, which must not be an error result.
#[track_caller]
fn answered(result: &Value, context: &str) -> Value {
    assert!(!is_error(result), "{context}: an error result: {result}");
    result["structuredContent"].clone()
}

/// A tool result that is rmcp's refusal of its arguments.
#[track_caller]
fn assert_bad_arguments(result: &Value, context: &str) {
    assert!(is_error(result), "{context}: not an error result: {result}");
    let text = content_text(result);
    assert!(
        text.starts_with(BAD_ARGUMENTS),
        "{context}: want {BAD_ARGUMENTS:?}, got {text:?}"
    );
}

/// `(span_hash, text)` of `id` through `get_node`.
fn span(client: &mut Client, id: &str) -> (String, String) {
    let result = client.call("get_node", json!({"id": id}));
    let document = answered(&result, id);
    let node = &document["nodes"][0];
    (
        node["span_hash"].as_str().expect("span_hash").to_owned(),
        node["text"].as_str().expect("text").to_owned(),
    )
}

/// An update of `id`'s text, `from` become `to`.
fn update_args(client: &mut Client, id: &str, from: &str, to: &str) -> Value {
    let (base, text) = span(client, id);
    assert!(text.contains(from), "{id} holds no {from:?}: {text}");
    json!({"kind": "update", "target": id, "base": base, "text": text.replacen(from, to, 1),
        "rationale": "A clearer line."})
}

/// A question about `node`.
fn question(node: &str, text: &str) -> Value {
    json!({"node_ids": [node], "text": text, "working_answer": "Keep it as written.",
        "price_of_other": "One rule rewritten."})
}

/// A discrepancy about `node`.
fn discrepancy(node: &str, summary: &str) -> Value {
    json!({
        "node_ids": [node], "summary": summary, "gap_type": "contradicts", "severity": "high",
        "evidence": [{"file": "notes/playtest.md", "lines": "3-9",
            "observed": "regenerates while walking", "documented": "only at rest"}],
        "options": [{"label": "build", "effect": "follow the text", "price": "1 item"},
            {"label": "text", "effect": "allow walking", "price": "a rebalance"}],
        "recommendation": 0
    })
}

/// A decision about `node`.
fn decision(node: &str, text: &str) -> Value {
    json!({"node_ids": [node], "text": text, "working_answer": "Yes, at once.",
        "price_of_other": "A rule case."})
}

/// A queue tool's call that must store: when the first call meets hits, the
/// same call naming each in `distinct_from` (docs/canon/agent-intake.md). The
/// stored ID.
fn raised(client: &mut Client, tool: &str, args: Value) -> String {
    let first = client.call(tool, args.clone());
    let document = answered(&first, tool);
    if let Some(id) = document["id"].as_str() {
        return id.to_owned();
    }
    let hits: Vec<Value> = document["hits"]
        .as_array()
        .unwrap_or_else(|| panic!("{tool}: not stored and no hit: {document}"))
        .iter()
        .map(|hit| {
            if hit["id"].is_string() {
                hit["id"].clone()
            } else {
                hit["path"].clone()
            }
        })
        .collect();
    assert!(
        !hits.is_empty(),
        "{tool}: not stored and no hit: {document}"
    );
    let mut again = args;
    again["distinct_from"] = Value::Array(hits);
    let second = client.call(tool, again);
    answered(&second, tool)["id"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool}: not stored: {second}"))
        .to_owned()
}

/// Every key of every `properties` object in `schema`, nested ones too.
fn property_keys(schema: &Value, out: &mut BTreeSet<String>) {
    match schema {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get("properties") {
                out.extend(properties.keys().cloned());
            }
            for value in object.values() {
                property_keys(value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                property_keys(item, out);
            }
        }
        _ => {}
    }
}

/// Whether `text` holds `word` whole: no letter, digit or `_` either side.
fn has_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let wordy = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    text.match_indices(word).any(|(at, _)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + word.len()).copied();
        !before.is_some_and(wordy) && !after.is_some_and(wordy)
    })
}

/// The stdio server's `tools/list` result over `root`, under `home`.
fn stdio_tools(home: &Path, root: &Path) -> Value {
    let mut mcp = Mcp::start_with(home, root, &[OsStr::new("--root"), root.as_os_str()]);
    let tools = mcp.request(1, "tools/list", json!({}));
    mcp.finish();
    result_of(&tools, "stdio tools/list").clone()
}

// ----------------------------------------------------------------------- H1

/// H1, a hub: the fence's three ways of refusing a login, another method, a
/// query on every method (a bare `?` too), the paths around the route, and an
/// unknown slug; none of them stores anything.
#[test]
fn h1_the_fence_and_the_route_on_a_hub() {
    let world = World::new("mcp-h1");
    let path = mcp_path(KEEP);
    let init = message(0, "initialize", common::mcp_http::initialize_params()).to_string();
    let before = world.events();
    let repository = world.repository(&world.keep);

    let no_login = proxied("mira").without("Remote-User");
    let two = proxied("mira").plus("Remote-User", "mira");
    let posting = |proxied: &Proxied| common::mcp_http::posting(proxied.clone());
    let reply = through(
        &world.server,
        "POST",
        &path,
        &posting(&no_login),
        init.as_bytes(),
    );
    assert_refused(&reply, NO_LOGIN_LINE, "no Remote-User");
    let reply = through(
        &world.server,
        "POST",
        &path,
        &posting(&two),
        init.as_bytes(),
    );
    assert_refused(&reply, TWO_LOGINS_LINE, "two Remote-User");
    for word in ["agent", "AGENT", "local"] {
        let reply = through(
            &world.server,
            "POST",
            &path,
            &posting(&proxied(word)),
            init.as_bytes(),
        );
        assert_eq!(reply.status, 403, "{word}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.starts_with("refused: Remote-User") || message == NO_LOGIN_LINE,
            "{word}: {message}"
        );
    }

    for method in ["PUT", "PATCH"] {
        let reply = through(
            &world.server,
            method,
            &path,
            &posting(&proxied("mira")),
            init.as_bytes(),
        );
        assert_refused(&reply, OFF_LIST_LINE, method);
    }

    for query in ["?x=1", "?"] {
        let queried = format!("{path}{query}");
        for (method, body) in [("GET", ""), ("POST", init.as_str()), ("DELETE", "")] {
            let headers = if method == "POST" {
                posting(&proxied("mira"))
            } else {
                proxied("mira")
                    .plus("Accept", "text/event-stream")
                    .plus(SESSION_HEADER, NEVER_ISSUED)
            };
            let reply = through(&world.server, method, &queried, &headers, body.as_bytes());
            assert_refused(&reply, NO_QUERY_LINE, &format!("{method} {queried}"));
        }
    }

    let unknown = through(
        &world.server,
        "GET",
        "/no/such/route",
        &proxied("mira"),
        b"",
    );
    assert_eq!(unknown.status, 404, "{}", unknown.text());
    for around in ["/mcp", "/mcp/", "/mcp/lantern-keep/x"] {
        let reply = through(&world.server, "GET", around, &proxied("mira"), b"");
        assert_eq!(reply.status, 404, "GET {around}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            unknown_route_message(&unknown, around),
            "GET {around}"
        );
    }
    let reply = through(
        &world.server,
        "POST",
        "/mcp/lantern-keep/x",
        &posting(&proxied("mira")),
        init.as_bytes(),
    );
    assert_refused(&reply, OFF_LIST_LINE, "POST /mcp/lantern-keep/x");

    let slug_404 = world.page("mira", "/api/projects/nope/tree");
    assert_eq!(slug_404.status, 404, "{}", slug_404.text());
    let unknown_slug = slug_404.error_message();
    assert!(unknown_slug.contains("nope"), "{unknown_slug}");
    for (method, headers, body) in [
        ("POST", posting(&proxied("mira")), init.as_str()),
        (
            "GET",
            proxied("mira")
                .plus("Accept", "text/event-stream")
                .plus(SESSION_HEADER, NEVER_ISSUED),
            "",
        ),
        (
            "DELETE",
            proxied("mira").plus(SESSION_HEADER, NEVER_ISSUED),
            "",
        ),
    ] {
        let reply = through(
            &world.server,
            method,
            "/mcp/nope",
            &headers,
            body.as_bytes(),
        );
        assert_eq!(reply.status, 404, "{method} /mcp/nope: {}", reply.text());
        assert_eq!(reply.error_message(), unknown_slug, "{method} /mcp/nope");
        assert!(
            reply.header(SESSION_HEADER).is_none(),
            "{method} /mcp/nope: {:?}",
            reply.headers
        );
    }

    assert_eq!(world.events(), before, "no refusal stored anything");
    assert_eq!(
        world.repository(&world.keep),
        repository,
        "no refusal touched the repository"
    );
}

/// H1, a local daemon: every method on `/mcp/lantern-keep`, a `Remote-User`
/// too, is the unknown-route 404, its line with its own path; nothing stored.
#[test]
fn h1_a_local_daemon_serves_no_mcp_route() {
    let scratch = Scratch::new("mcp-h1-local");
    let keep = scratch.repo("spec-a", "keep", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&keep]);
    let path = mcp_path(KEEP);
    let init = message(0, "initialize", common::mcp_http::initialize_params()).to_string();
    let unknown_get = server.get("/no/such/route");
    assert_eq!(unknown_get.status, 404, "{}", unknown_get.text());
    let json_headers = [
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];
    let unknown_post = server.send("POST", "/no/such/route", &json_headers, init.as_bytes());
    assert_eq!(unknown_post.status, 404, "{}", unknown_post.text());
    let before = events_of(&home, &keep);

    for login in [None, Some("mira")] {
        let mut headers: Vec<(&str, &str)> = json_headers.to_vec();
        if let Some(login) = login {
            headers.push(("Remote-User", login));
        }
        let context = |method: &str| format!("{method} {path}, Remote-User {login:?}");
        let reply = server.send("POST", &path, &headers, init.as_bytes());
        assert_eq!(reply.status, 404, "{}: {}", context("POST"), reply.text());
        assert_eq!(
            reply.error_message(),
            unknown_route_message(&unknown_post, &path),
            "{}",
            context("POST")
        );
        let reply = server.request("GET", &path, &headers);
        assert_eq!(reply.status, 404, "{}: {}", context("GET"), reply.text());
        assert_eq!(
            reply.error_message(),
            unknown_route_message(&unknown_get, &path),
            "{}",
            context("GET")
        );
        for method in ["DELETE", "PUT", "PATCH", "HEAD", "OPTIONS"] {
            let reply = server.send(method, &path, &headers, b"");
            assert_eq!(reply.status, 404, "{}: {}", context(method), reply.text());
        }
    }
    assert_eq!(events_of(&home, &keep), before, "nothing stored");
}

// ----------------------------------------------------------------------- H2

/// H2: a 2025-11-25 session as `mira`: one id, its stream kept open with a
/// keep-alive comment, the seventeen tools, a DELETE, then rmcp's 404 on every
/// method naming it.
#[test]
fn h2_a_legacy_session_opens_streams_lists_and_ends() {
    let world = World::new("mcp-h2");
    let port = world.port();
    let path = mcp_path(KEEP);
    let answer = Session::initialize(port, &path, "mira");
    assert_eq!(answer.status(), 200, "{}", answer.text());
    assert_eq!(answer.sessions().len(), 1, "{:?}", answer.reply.headers);
    let response = answer.expect_response(0, "initialize");
    let result = result_of(&response, "initialize");
    assert_eq!(result["protocolVersion"], json!(LEGACY), "{result}");
    let mut session = Session::opened(port, &path, "mira", &answer);

    let mut stream = session.stream_as("mira");
    assert_eq!(stream.status, 200, "the GET stream");
    assert!(
        stream
            .header("content-type")
            .is_some_and(|kind| kind.starts_with("text/event-stream")),
        "{:?}",
        stream.headers
    );
    let frames = stream.frames_within(Duration::from_secs(20));
    assert!(!stream.ended(), "the stream ended within 20 s: {frames:?}");
    assert!(
        frames
            .iter()
            .any(|frame| frame.data.is_none() && !frame.comments.is_empty()),
        "no keep-alive comment within 20 s: {frames:?}"
    );

    let names = tool_names(&session.tools());
    assert_eq!(names, hub_listed(), "the hub's tools/list");

    let deleted = session.delete_as("mira");
    assert_eq!(deleted.status(), 202, "DELETE: {}", deleted.text());
    drop(stream);

    let (_, listed) = session.send("tools/list", json!({}));
    for (method, answer) in [
        ("POST", listed),
        ("GET", session.get_as("mira")),
        ("DELETE", session.delete_as("mira")),
        (
            "a notification",
            session.notify_as("mira", "notifications/initialized"),
        ),
    ] {
        assert_eq!(
            answer.status(),
            404,
            "{method} after DELETE: {}",
            answer.text()
        );
        assert_eq!(answer.text(), NO_SESSION, "{method} after DELETE");
    }
}

// ----------------------------------------------------------------------- H3

/// H3: a 2026-07-28 `tools/call get_tree {}` alone, `_meta` and
/// `MCP-Protocol-Version`, no session: the tree a session reads, no
/// `Mcp-Session-Id`; `server/discover` answers `HUB_INSTRUCTIONS` byte for byte,
/// the text a legacy `initialize` carries, and not the stdio `INSTRUCTIONS`.
#[test]
fn h3_a_stateless_call_is_answered_alone_and_discover_is_the_hub_text() {
    let world = World::new("mcp-h3");
    let mut client = world.stateless(KEEP, "mira");
    let (id, answer) = client.send("tools/call", json!({"name": "get_tree", "arguments": {}}));
    assert_eq!(answer.status(), 200, "{}", answer.text());
    assert_eq!(
        answer.sessions(),
        Vec::<String>::new(),
        "a stateless session"
    );
    let response = answer.expect_response(id, "get_tree");
    let stateless_tree = result_of(&response, "get_tree").clone();
    assert!(!is_error(&stateless_tree), "{stateless_tree}");

    let mut session = world.session(KEEP, "mira");
    let legacy_tree = result_of(&session.call("get_tree", json!({})), "get_tree").clone();
    assert_eq!(
        content_text(&stateless_tree),
        content_text(&legacy_tree),
        "the tree in both eras"
    );
    assert_eq!(
        stateless_tree["structuredContent"], legacy_tree["structuredContent"],
        "the tree's document in both eras"
    );

    let discover = client.request("server/discover", json!({}));
    let discovered = result_of(&discover, "server/discover")["instructions"]
        .as_str()
        .unwrap_or_else(|| panic!("no instructions: {discover}"))
        .to_owned();
    assert_eq!(
        discovered,
        specengine_mcp::HUB_INSTRUCTIONS,
        "server/discover's instructions"
    );
    let answer = Session::initialize(world.port(), &mcp_path(KEEP), "mira");
    let initialized = answer.expect_response(0, "initialize");
    assert_eq!(
        result_of(&initialized, "initialize")["instructions"],
        json!(discovered),
        "initialize's instructions"
    );
    assert_ne!(
        discovered,
        specengine_mcp::INSTRUCTIONS,
        "the hub's text is its own"
    );

    // A stateless GET has no stream to open, and a stateless request names
    // no session: neither opens one.
    let headers = proxied("mira")
        .plus("MCP-Protocol-Version", STATELESS)
        .plus("Accept", "text/event-stream");
    let reply = exchange(world.port(), "GET", &mcp_path(KEEP), &headers, b"", QUICK);
    assert_ne!(reply.status(), 200, "{}", reply.text());
    assert_eq!(reply.sessions(), Vec::<String>::new());
}

// ----------------------------------------------------------------------- H4

/// H4: on a session `mira` opened, `tomas` is answered as an id never issued,
/// on POST (a card write), GET and DELETE, and runs nothing; the id at
/// `zerkalo` is the same 404; `mira`'s session lives on.
#[test]
fn h4_a_session_belongs_to_its_login_and_its_project() {
    let world = World::new("mcp-h4");
    let port = world.port();
    let mut mira = world.session(KEEP, "mira");
    let before = world.events();
    let card = json!({"name": "create_card",
        "arguments": {"title": "Lamp oil", "discipline": "sketch"}});

    let never = mira.named(NEVER_ISSUED);
    let (_, tomas_post) = mira.send_as("tomas", "tools/call", card.clone());
    let mut never_session = never.clone();
    let (_, never_post) = never_session.send_as("tomas", "tools/call", card.clone());
    for (label, theirs, unissued) in [
        ("POST", tomas_post, never_post),
        ("GET", mira.get_as("tomas"), never.get_as("tomas")),
        ("DELETE", mira.delete_as("tomas"), never.delete_as("tomas")),
    ] {
        assert_eq!(theirs.status(), 404, "{label} as tomas: {}", theirs.text());
        assert_eq!(theirs.text(), NO_SESSION, "{label} as tomas");
        assert_eq!(
            (theirs.status(), &theirs.reply.body),
            (unissued.status(), &unissued.reply.body),
            "{label}: tomas on mira's session is a never-issued id"
        );
        assert!(theirs.sessions().is_empty(), "{label}");
    }
    assert_eq!(world.events(), before, "tomas's card was not stored");
    assert!(mira.answers(), "mira's session after tomas's DELETE");

    let zerkalo_path = mcp_path(ZERKALO);
    let (_, foreign) = mira.send_at(&zerkalo_path, "mira", "tools/list", json!({}));
    assert_eq!(foreign.status(), 404, "at zerkalo: {}", foreign.text());
    assert_eq!(foreign.text(), NO_SESSION, "at zerkalo");
    let foreign_delete = exchange(port, "DELETE", &zerkalo_path, &mira.headers(), b"", QUICK);
    assert_eq!(foreign_delete.status(), 404, "{}", foreign_delete.text());
    assert!(mira.answers(), "mira's session after a DELETE at zerkalo");

    let made = mira.call("create_card", card["arguments"].clone());
    let made = answered(result_of(&made, "create_card"), "create_card");
    assert!(made["id"].is_string(), "{made}");
    assert_eq!(world.events().len(), before.len() + 1, "mira's card");
}

// ----------------------------------------------------------------------- H5

/// H5: `propose_change` as `mira` stores "Data"'s author, `login` last, and the
/// commit `tomas` confirms carries "Data"'s `Proposed-by`; the three intake
/// tools store her login too; the four over stdio store no `login`.
#[test]
fn h5_the_request_login_is_the_author_and_stdio_stays_as_it_was() {
    let world = World::new("mcp-h5");
    let mut client = Client::open(Era::Legacy, world.port(), KEEP, "mira");
    let hub_author = json!({"type": "agent", "role": ROLE, "model": MODEL, "run": RUN,
        "login": "mira"});

    let update = update_args(&mut client, "EDGE-STAM-ZERO", "immediately", "at once");
    let update_id = raised(&mut client, "propose_change", with(update, &agent()));
    let review = world.review(&update_id);
    assert_eq!(review["author"], hub_author, "{review}");
    // The key order is the bytes' alone: a parsed `Value` sorts its keys. Core
    // `Author` writes `type, role, model, run, login`, in the stored row and in
    // the review's document.
    let raw_author = "{\"type\":\"agent\",\"role\":\"drafter\",\"model\":\"m-1\",\
                      \"run\":\"r-1\",\"login\":\"mira\"}";
    assert!(
        every_byte(&common::data_dir(&world.home))
            .iter()
            .any(|(_, bytes)| holds(bytes, raw_author)),
        "the stored row's bytes hold {raw_author}"
    );
    let printed = spec_json(
        &world.home,
        &world.cwd,
        &world.keep,
        &["review", &update_id],
    );
    assert!(
        printed.stdout.contains(&format!("\"author\":{raw_author}")),
        "the review's bytes: {}",
        printed.stdout
    );
    let mut intake = Vec::new();
    for (tool, args) in [
        (
            "ask_question",
            question("EDGE-SPRINT-EMPTY", "Does the sprint end at once?"),
        ),
        (
            "report_discrepancy",
            discrepancy("RULE-STAM-REGEN", "Regeneration runs while walking."),
        ),
        (
            "report_decision",
            decision("RULE-SPRINT-COST", "Does the cost round down?"),
        ),
    ] {
        let id = raised(&mut client, tool, with(args, &agent()));
        assert_eq!(world.review(&id)["author"], hub_author, "{tool} {id}");
        intake.push(id);
    }

    let document = world.confirmed_by_tomas(&update_id);
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(
        world.trailers("Proposed-by"),
        ["agent role=drafter model=m-1 run=r-1 login=mira"]
    );
    assert_eq!(world.trailers("Decided-by"), [TOMAS]);

    // The stdio twins, under the same HOME: four keys, no login.
    let stdio_author = json!({"type": "agent", "role": ROLE, "model": MODEL, "run": RUN});
    let mut stdio = Mcp::start(&world.home, &world.keep);
    let shown = stdio.request(
        1,
        "tools/call",
        json!({"name": "get_node",
        "arguments": {"id": "EDGE-SPRINT-EMPTY"}}),
    );
    let node = &shown["result"]["structuredContent"]["nodes"][0];
    let text = node["text"].as_str().expect("text");
    let mut stdio_ids = Vec::new();
    let calls = [
        (
            "propose_change",
            json!({"kind": "update", "target": "EDGE-SPRINT-EMPTY", "base": node["span_hash"],
                "text": text.replacen("the sprint ends", "the sprint stops", 1),
                "rationale": "A plainer verb."}),
        ),
        (
            "ask_question",
            question("EDGE-STAM-ZERO", "Is the tank empty at zero?"),
        ),
        (
            "report_discrepancy",
            discrepancy("EDGE-STAM-ZERO", "Zero is reached while resting."),
        ),
        (
            "report_decision",
            decision("EDGE-STAM-ZERO", "Does zero end the sprint?"),
        ),
    ];
    for (n, (tool, args)) in calls.into_iter().enumerate() {
        let id = 10 + i64::try_from(n).expect("small");
        let mut args = with(args, &agent());
        let mut reply = stdio.request(id, "tools/call", json!({"name": tool, "arguments": args}));
        let mut document = reply["result"]["structuredContent"].clone();
        if document["id"].is_null() {
            let hits: Vec<Value> = document["hits"]
                .as_array()
                .unwrap_or_else(|| panic!("stdio {tool}: {reply}"))
                .iter()
                .map(|hit| hit["id"].clone())
                .collect();
            args["distinct_from"] = Value::Array(hits);
            reply = stdio.request(
                id + 100,
                "tools/call",
                json!({"name": tool, "arguments": args}),
            );
            document = reply["result"]["structuredContent"].clone();
        }
        let stored = document["id"]
            .as_str()
            .unwrap_or_else(|| panic!("stdio {tool}: not stored: {reply}"))
            .to_owned();
        assert_eq!(
            world.review(&stored)["author"],
            stdio_author,
            "stdio {tool}"
        );
        stdio_ids.push(stored);
    }
    stdio.finish();

    let dump = world.scratch.join("state.jsonl");
    let run = spec(
        &world.home,
        &world.cwd,
        &[
            "--root",
            world.keep.to_str().expect("UTF-8"),
            "export",
            "state",
            "--out",
            dump.to_str().expect("UTF-8"),
        ],
    );
    run.code(0);
    let dumped = fs::read_to_string(&dump).expect("the dump");
    for id in &stdio_ids {
        let lines: Vec<&str> = dumped
            .lines()
            .filter(|line| line.contains(&format!("\"{id}\"")))
            .collect();
        assert!(!lines.is_empty(), "{id} is in the dump");
        for line in lines {
            assert!(!line.contains("login"), "a stdio row holds a login: {line}");
        }
    }
    for id in intake.iter().chain([&update_id]) {
        assert!(
            dumped
                .lines()
                .any(|line| line.contains(&format!("\"{id}\"")) && line.contains("mira")),
            "{id}'s row holds mira"
        );
    }
}

// ----------------------------------------------------------------------- H6

/// H6: no input schema of the hub's list holds `login`, `as`, `actor` or
/// `task_id`; each of the seven writes refuses each of them as an argument, in
/// both eras, and stores nothing.
#[test]
fn h6_no_identity_or_task_argument_reaches_a_hub_write() {
    let world = World::new("mcp-h6");
    let mut setup = Client::open(Era::Legacy, world.port(), KEEP, "mira");
    let made = setup.call(
        "create_card",
        json!({"title": "Lamp oil", "discipline": "sketch"}),
    );
    let card_id = answered(&made, "create_card")["id"]
        .as_str()
        .expect("a card")
        .to_owned();
    let list = setup.tools();
    for name in HUB_TOOLS {
        let mut keys = BTreeSet::new();
        property_keys(&tool(&list, name)["inputSchema"], &mut keys);
        for key in IDENTITY_KEYS {
            assert!(
                !keys.contains(key),
                "{name}'s input schema holds {key}: {keys:?}"
            );
        }
    }

    for era in ERAS {
        let mut client = Client::open(era, world.port(), KEEP, "mira");
        let update = update_args(&mut client, "EDGE-STAM-ZERO", "immediately", "at once");
        let writes = [
            ("propose_change", with(update, &agent())),
            (
                "ask_question",
                with(question("EDGE-STAM-ZERO", "Asked with a key?"), &agent()),
            ),
            (
                "report_discrepancy",
                with(
                    discrepancy("EDGE-STAM-ZERO", "Reported with a key."),
                    &agent(),
                ),
            ),
            (
                "report_decision",
                with(decision("EDGE-STAM-ZERO", "Decided with a key?"), &agent()),
            ),
            (
                "create_card",
                json!({"title": "Lamp wick", "discipline": "sketch"}),
            ),
            ("edit_card", json!({"card_id": card_id, "goal": "Trim it."})),
            ("move_card", json!({"card_id": card_id, "to": "shaped"})),
        ];
        let before = world.events();
        for (name, args) in &writes {
            for key in IDENTITY_KEYS {
                let mut sent = args.clone();
                sent[key] = json!(if key == "task_id" { "T-0001" } else { "mira" });
                let result = client.call(name, sent);
                assert_bad_arguments(&result, &format!("{era:?} {name} with {key}"));
            }
        }
        assert_eq!(world.events(), before, "{era:?}: nothing stored");
    }
}

// ----------------------------------------------------------------------- H7

/// H7: `create_card` and `move_card` as `mira` record her; a move `by =
/// ["tomas"]` refuses her, `by = ["mira"]` lets her; over stdio the same calls
/// record `agent`.
#[test]
fn h7_card_writes_act_as_the_login() {
    let world = World::new("mcp-h7");
    for era in ERAS {
        let mut client = Client::open(era, world.port(), KEEP, "mira");
        let made = client.call(
            "create_card",
            json!({"title": format!("Quest board {era:?}"), "discipline": "quests"}),
        );
        let quest = answered(&made, "create_card")["id"]
            .as_str()
            .expect("a card")
            .to_owned();
        let (kind, payload) = world.events().pop().expect("an event");
        assert_eq!(kind, "card.created", "{payload}");
        assert_eq!(payload["as"], json!("mira"), "{era:?}: {payload}");
        let shown = world.page("tomas", &format!("/api/projects/{KEEP}/cards/{quest}"));
        assert_eq!(shown.status, 200, "{}", shown.text());
        assert_eq!(shown.json()["created_by"], json!("mira"), "{era:?}");

        let moved = client.call("move_card", json!({"card_id": quest, "to": "read"}));
        assert_eq!(answered(&moved, "move")["column"], json!("read"), "{moved}");
        let (kind, payload) = world.events().pop().expect("an event");
        assert_eq!(kind, "card.moved", "{payload}");
        assert_eq!(payload["as"], json!("mira"), "{era:?}: {payload}");

        let made = client.call(
            "create_card",
            json!({"title": format!("Old tales {era:?}"), "discipline": "lore"}),
        );
        let lore = answered(&made, "create_card")["id"]
            .as_str()
            .expect("a card")
            .to_owned();
        let before = world.events();
        let refused = client.call("move_card", json!({"card_id": lore, "to": "read"}));
        assert!(
            is_error(&refused),
            "{era:?}: tomas's move by mira: {refused}"
        );
        let text = content_text(&refused);
        assert!(
            text.contains("`tomas`'s, not `mira`'s"),
            "{era:?}: the refusal names both: {text}"
        );
        assert_eq!(
            world.events(),
            before,
            "{era:?}: a refused move stores nothing"
        );
    }

    let mut stdio = Mcp::start(&world.home, &world.keep);
    let made = stdio.request(
        1,
        "tools/call",
        json!({"name": "create_card",
            "arguments": {"title": "Map corner", "discipline": "sketch"}}),
    );
    let sketch = made["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("{made}"))
        .to_owned();
    let (kind, payload) = world.events().pop().expect("an event");
    assert_eq!(
        (kind.as_str(), &payload["as"]),
        ("card.created", &json!("agent"))
    );
    let moved = stdio.request(
        2,
        "tools/call",
        json!({"name": "move_card", "arguments": {"card_id": sketch, "to": "shaped"}}),
    );
    assert_eq!(
        moved["result"]["structuredContent"]["column"],
        json!("shaped"),
        "{moved}"
    );
    stdio.finish();
    let (kind, payload) = world.events().pop().expect("an event");
    assert_eq!(
        (kind.as_str(), &payload["as"]),
        ("card.moved", &json!("agent"))
    );
}

// ----------------------------------------------------------------------- H8

/// An argument no tool takes.
fn unknown_key() -> Value {
    json!({"zz_unknown": true})
}

/// `name` called with `args`, which its schema takes, then with an unknown key
/// added, which it refuses; `name` recorded in `called`. The valid result.
fn both(
    client: &mut Client,
    called: &mut BTreeSet<String>,
    label: &str,
    name: &str,
    args: Value,
) -> Value {
    let valid = client.call(name, args.clone());
    assert!(
        !content_text(&valid).starts_with(BAD_ARGUMENTS),
        "{label} {name} {args}: {valid}"
    );
    let invalid = client.call(name, with(args, &unknown_key()));
    assert_bad_arguments(&invalid, &format!("{label} {name} with an unknown key"));
    called.insert(name.to_owned());
    valid
}

/// Every tool of the hub, called once with valid and once with invalid
/// arguments (an unknown key), the two resources read and a foreign one
/// refused, on `client`: the names called both ways.
fn every_tool(client: &mut Client, world: &World, label: &str) -> BTreeSet<String> {
    let mut called = BTreeSet::new();
    let unknown = unknown_key();

    for (name, args) in [
        ("get_tree", json!({})),
        ("get_node", json!({"id": "MEC-STAMINA"})),
        ("search", json!({"query": "stamina"})),
        ("get_context_bundle", json!({"node_ids": ["MEC-STAMINA"]})),
        ("get_roadmap", json!({})),
        ("get_flow", json!({})),
        ("get_trace", json!({})),
        ("list_cards", json!({})),
    ] {
        both(client, &mut called, label, name, args);
    }

    let (node, from, to) = if label.contains("Legacy") {
        ("EDGE-STAM-ZERO", "immediately", "at once")
    } else {
        ("EDGE-SPRINT-EMPTY", "the sprint ends", "the sprint stops")
    };
    let update = with(update_args(client, node, from, to), &agent());
    let seen = world.events().len();
    let rehearsed = both(
        client,
        &mut called,
        label,
        "propose_change",
        with(update.clone(), &json!({"dry_run": true})),
    );
    assert!(!is_error(&rehearsed), "{label}: the dry run: {rehearsed}");
    let created = json!({"kind": "create", "target": format!("docs/spec/notes-{}.md",
        label.to_lowercase()), "base": null, "text": "# Notes\n\nA page of notes.\n",
        "rationale": "A new page.", "dry_run": true});
    client.call("propose_change", with(created, &agent()));
    assert_eq!(
        world.events().len(),
        seen,
        "{label}: a dry run stores nothing"
    );
    let stored = client.call("propose_change", update);
    let proposal = answered(&stored, "propose_change")["id"]
        .as_str()
        .expect("stored")
        .to_owned();

    let asked = raised(
        client,
        "ask_question",
        with(
            question("EDGE-STAM-ZERO", &format!("Asked in {label}?")),
            &agent(),
        ),
    );
    let mut invalid = with(question("EDGE-STAM-ZERO", "Unknown?"), &agent());
    invalid["zz_unknown"] = json!(true);
    assert_bad_arguments(&client.call("ask_question", invalid), "ask_question");
    called.insert("ask_question".to_owned());
    for (name, args) in [
        (
            "report_discrepancy",
            discrepancy("EDGE-STAM-ZERO", &format!("Departs in {label}.")),
        ),
        (
            "report_decision",
            decision("EDGE-STAM-ZERO", &format!("Decided in {label}?")),
        ),
    ] {
        raised(client, name, with(args.clone(), &agent()));
        let invalid = with(with(args, &agent()), &unknown);
        assert_bad_arguments(&client.call(name, invalid), name);
        called.insert(name.to_owned());
    }
    for id in [&proposal, &asked] {
        both(
            client,
            &mut called,
            label,
            "get_proposal",
            json!({"proposal_id": id}),
        );
    }

    let made = both(
        client,
        &mut called,
        label,
        "create_card",
        json!({"title": format!("Lamp {label}"), "discipline": "sketch"}),
    );
    let card = answered(&made, "create_card")["id"]
        .as_str()
        .expect("a card")
        .to_owned();
    both(
        client,
        &mut called,
        label,
        "get_card",
        json!({"card_id": card}),
    );
    both(
        client,
        &mut called,
        label,
        "edit_card",
        json!({"card_id": card, "goal": "Trim the wick."}),
    );
    both(
        client,
        &mut called,
        label,
        "move_card",
        json!({"card_id": card, "to": "shaped"}),
    );
    // docs/features/department-split.md S8: a split under the stored card,
    // its reviewer the login the chain ends at.
    both(
        client,
        &mut called,
        label,
        "split_card",
        json!({"parent": card, "child": ["sketch"]}),
    );

    for uri in [
        format!("spec://{KEEP}/tree"),
        format!("spec://{KEEP}/node/MEC-STAMINA"),
    ] {
        let read = client.request("resources/read", json!({"uri": uri}));
        let contents = &result_of(&read, &uri)["contents"];
        assert!(
            contents.as_array().is_some_and(|items| !items.is_empty()),
            "{label} {uri}: {read}"
        );
    }
    let foreign = client.request(
        "resources/read",
        json!({"uri": format!("spec://{ZERKALO}/tree")}),
    );
    assert!(foreign.get("error").is_some(), "{label}: {foreign}");
    called
}

/// Every path under `dir` outside `home`, relative.
fn paths_outside(dir: &Path, home: &Path) -> BTreeSet<String> {
    let home = home
        .strip_prefix(dir)
        .expect("HOME in the scratch")
        .to_string_lossy()
        .into_owned();
    snapshot(dir)
        .into_keys()
        .filter(|path| *path != home && !path.starts_with(&format!("{home}/")))
        .collect()
}

/// H8 (08 AC-3): all seventeen tools in both eras, valid and invalid, a dry run
/// too, both resources: the repositories as committed, every new path under
/// `HOME`. A listed tool this test does not call fails it.
#[test]
fn h8_one_door_every_tool_in_both_eras_writes_under_home_alone() {
    let world = World::new("mcp-h8");
    let repositories = [
        world.repository(&world.keep),
        world.repository(&world.zerkalo),
    ];
    let outside = paths_outside(world.scratch.path(), &world.home);
    for era in ERAS {
        let mut client = Client::open(era, world.port(), KEEP, "mira");
        let listed = tool_names(&client.tools());
        let called = every_tool(&mut client, &world, &format!("{era:?}"));
        let missed: Vec<&String> = listed
            .iter()
            .filter(|name| !called.contains(*name))
            .filter(|name| !specengine_mcp::HUB_PROBES.contains(&name.as_str()))
            .collect();
        assert!(
            missed.is_empty(),
            "{era:?}: listed and never called here: {missed:?}"
        );
    }
    for root in [&world.keep, &world.zerkalo] {
        let status = world.scratch.git().run(
            root,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored",
            ],
        );
        assert_eq!(status, "", "{}", root.display());
    }
    assert_eq!(
        [
            world.repository(&world.keep),
            world.repository(&world.zerkalo),
        ],
        repositories,
        "HEAD, the refs, the worktrees and the files as committed"
    );
    assert_eq!(
        paths_outside(world.scratch.path(), &world.home),
        outside,
        "a path created outside HOME"
    );
}

// ----------------------------------------------------------------------- H9

/// H9: the hub lists exactly the seventeen, none named for deciding; the
/// stdio server still lists its twenty-nine.
#[test]
fn h9_nothing_on_a_hub_decides() {
    let world = World::new("mcp-h9");
    for era in ERAS {
        let mut client = Client::open(era, world.port(), KEEP, "mira");
        let names = tool_names(&client.tools());
        assert_eq!(names, hub_listed(), "{era:?}");
        for name in &names {
            for word in DECIDING {
                assert!(!name.contains(word), "{era:?}: {name} holds {word}");
            }
        }
    }
    let stdio = stdio_tools(&world.home, &world.keep);
    let mut stdio_names = tool_names(&stdio);
    // The measurement build (its hub probes are listed) runs a stdio server
    // of that build too, whose own probes are not product tools.
    if !specengine_mcp::HUB_PROBES.is_empty() {
        stdio_names.retain(|name| !name.starts_with("probe_"));
    }
    // department-split S8: thirty with `split_card`, on both servers;
    // thirty-two on stdio since collision-pass CP8, the hub's 18 unchanged.
    assert_eq!(stdio_names.len(), 32, "{stdio_names:?}");
    let mut union: Vec<&str> = HUB_TOOLS.iter().chain(LEFT_OUT.iter()).copied().collect();
    union.sort_unstable();
    assert_eq!(stdio_names, union, "the 18 and the 14 are stdio's 32");
}

// ---------------------------------------------------------------------- H10

/// `body` as the hook `name` of `root`.
fn hook(root: &Path, name: &str, body: &str) {
    let dir = root.join(".git").join("hooks");
    fs::create_dir_all(&dir).expect("hooks directory");
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("hook");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable hook");
}

/// A confirm on `lantern-keep` ready to be held: `tomas`'s approve of an
/// update staged and read, the request's bytes, and the `pre-commit` hook of a
/// gate the test opens (`entered` once the commit waits, `go` to let it end),
/// in place of a clock: nothing races.
struct Held {
    confirm: Vec<u8>,
    entered: PathBuf,
    go: PathBuf,
}

fn held_confirm(world: &World) -> Held {
    let shown = spec_json(
        &world.home,
        &world.cwd,
        &world.keep,
        &["show", "EDGE-STAM-ZERO"],
    )
    .json();
    let node_text = shown["nodes"][0]["text"].as_str().expect("text").to_owned();
    assert!(node_text.contains("immediately"), "{node_text}");
    let id = propose_update(
        &world.home,
        &world.cwd,
        &world.keep,
        "EDGE-STAM-ZERO",
        &node_text.replacen("immediately", "at once", 1),
    );
    let base = format!("/api/projects/{KEEP}/proposals/{id}");
    let stage = json!({"decision": "approve", "updated_at": world.review(&id)["updated_at"]});
    world
        .page_post("tomas", &format!("{base}/decision"), &stage)
        .status(200);
    let read = world
        .page("tomas", &format!("{base}/confirm"))
        .status(200)
        .json();
    let confirm = Proxied::write("tomas").bytes(
        "POST",
        &format!("{base}/confirm"),
        json!({"question": read["question"], "updated_at": read["updated_at"]})
            .to_string()
            .as_bytes(),
    );
    let gate = world.scratch.dir("gate");
    let (entered, go) = (gate.join("entered"), gate.join("go"));
    hook(
        &world.keep,
        "pre-commit",
        &format!(
            "touch '{}'\ni=0\nwhile [ ! -f '{}' ] && [ $i -lt 1200 ]; do sleep 0.05; \
             i=$((i+1)); done",
            entered.display(),
            go.display()
        ),
    );
    Held {
        confirm,
        entered,
        go,
    }
}

/// Waits until the held confirm's hook runs: the turn is held.
fn wait_entered(entered: &Path) {
    let waited = Instant::now();
    while !entered.exists() {
        assert!(
            waited.elapsed() < Duration::from_secs(60),
            "the hook never ran"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// H10: a confirm on `lantern-keep` held in its `pre-commit` hook (a gate the
/// test opens, in place of a clock: nothing races) holds that project's turn.
/// Meanwhile `get_tree` there waits past 300 ms and answers once the confirm
/// has; `get_tree` on `zerkalo`, and `tools/list`, `ping` and `initialize` on
/// `lantern-keep`, answer within 1 s.
#[test]
fn h10_a_call_waits_for_its_project_turn_and_nothing_else_does() {
    let world = World::new("mcp-h10");
    let port = world.port();
    let Held {
        confirm,
        entered,
        go,
    } = held_confirm(&world);

    // Warm: each project's index refreshed, each session open, before the hold.
    let mut tree_keep = world.session(KEEP, "mira");
    let lists = world.session(KEEP, "mira");
    let pings = world.session(KEEP, "mira");
    let mut tree_zerkalo = world.session(ZERKALO, "mira");
    for session in [&mut tree_keep, &mut tree_zerkalo] {
        let result = session.call("get_tree", json!({}));
        assert!(!is_error(result_of(&result, "warm")), "{result}");
    }

    type Probe = (&'static str, Duration, Instant, Answer);
    let (sender, results) = mpsc::channel::<Probe>();
    let mut seen: Vec<Probe> = Vec::new();
    let (holder, go_at, early) = thread::scope(|scope| {
        let holder = scope.spawn(|| http_exchange(port, &confirm, HTTP_TIMEOUT));
        wait_entered(&entered);
        let probe =
            |label: &'static str, mut session: Session, method: &'static str, params: Value| {
                let sender = sender.clone();
                scope.spawn(move || {
                    let started = Instant::now();
                    let (_, answer) = session.send(method, params);
                    let _ = sender.send((label, started.elapsed(), Instant::now(), answer));
                });
            };
        let tree = json!({"name": "get_tree", "arguments": {}});
        probe(
            "keep get_tree",
            tree_keep.clone(),
            "tools/call",
            tree.clone(),
        );
        probe("zerkalo get_tree", tree_zerkalo.clone(), "tools/call", tree);
        probe("keep tools/list", lists.clone(), "tools/list", json!({}));
        probe("keep ping", pings.clone(), "ping", json!({}));
        {
            let sender = sender.clone();
            scope.spawn(move || {
                let started = Instant::now();
                let answer = Session::initialize(port, &mcp_path(KEEP), "mira");
                let _ = sender.send(("keep initialize", started.elapsed(), Instant::now(), answer));
            });
        }
        thread::sleep(Duration::from_millis(300));
        let early: Vec<&'static str> = results
            .try_iter()
            .map(|probe| {
                let label = probe.0;
                seen.push(probe);
                label
            })
            .collect();
        let until = Instant::now() + Duration::from_millis(1500);
        while seen.len() < 4 {
            let left = until.saturating_duration_since(Instant::now());
            match results.recv_timeout(left) {
                Ok(probe) => seen.push(probe),
                Err(_) => break,
            }
        }
        fs::write(&go, b"").expect("the gate");
        let go_at = Instant::now();
        let holder = holder.join().expect("the holder");
        (holder, go_at, early)
    });
    drop(sender);
    seen.extend(results.iter());
    fs::remove_file(world.keep.join(".git/hooks/pre-commit")).expect("the hook");

    assert_eq!(holder.status, 200, "the confirm: {}", holder.text());
    assert!(
        !early.contains(&"keep get_tree"),
        "get_tree on lantern-keep answered within 300 ms of a held turn: {early:?}"
    );
    for (label, elapsed, at, answer) in &seen {
        if *label == "keep get_tree" {
            assert!(*at >= go_at, "{label} answered before the confirm let go");
            assert_eq!(answer.status(), 200, "{label}: {}", answer.text());
            let response = answer
                .messages
                .iter()
                .find(|m| m.get("result").is_some())
                .unwrap_or_else(|| panic!("{label}: {}", answer.text()));
            assert!(!is_error(&response["result"]), "{label}: {response}");
        } else {
            assert!(
                *elapsed < Duration::from_secs(1),
                "{label} took {elapsed:?} while lantern-keep's turn was held"
            );
            assert_eq!(answer.status(), 200, "{label}: {}", answer.text());
        }
    }
    let labels: BTreeSet<&str> = seen.iter().map(|probe| probe.0).collect();
    assert_eq!(labels.len(), 5, "every probe answered: {labels:?}");
}

// ---------------------------------------------------------------------- H11

/// H11, the wire: `HUB_INSTRUCTIONS` as `initialize` sends it is ASCII, at most
/// 2 048 bytes, names each of the seventeen, none of the twelve nor `task_id`;
/// every description is ASCII, at most 2 048 bytes, holds the determinism
/// sentence; the eight variants (iteration 2: `get_roadmap` joins the seven
/// writes) differ from their stdio twins and drop the terminal's verbs,
/// `task_id`, `get_task` and `agent`, `get_roadmap`'s also the owner's
/// decision on a terminal; the card writes act as the login; the nine others
/// are the stdio twins' byte for byte, their schemas too; a variant's schema
/// is its twin's but `task_id`, `get_roadmap`'s whole. M: stdio's
/// `get_roadmap` description served on the hub.
#[test]
fn h11_the_hub_texts_hold_their_budget_and_their_words() {
    let world = World::new("mcp-h11");
    let answer = Session::initialize(world.port(), &mcp_path(KEEP), "mira");
    let response = answer.expect_response(0, "initialize");
    let instructions = result_of(&response, "initialize")["instructions"]
        .as_str()
        .expect("instructions")
        .to_owned();
    assert!(instructions.is_ascii(), "{instructions}");
    assert!(
        instructions.len() <= TEXT_LIMIT,
        "{} bytes",
        instructions.len()
    );
    for name in HUB_TOOLS {
        assert!(
            has_word(&instructions, name),
            "the instructions omit {name}"
        );
    }
    for name in LEFT_OUT.iter().chain(["task_id"].iter()) {
        assert!(
            !has_word(&instructions, name),
            "the instructions name {name}"
        );
    }

    let mut session = world.session(KEEP, "mira");
    let hub = session.tools();
    let stdio = stdio_tools(&world.home, &world.keep);
    for name in HUB_TOOLS {
        let description = tool(&hub, name)["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: no description"));
        assert!(description.is_ascii(), "{name}");
        assert!(
            description.len() <= TEXT_LIMIT,
            "{name}: {} bytes",
            description.len()
        );
        assert!(description.contains(DETERMINISM), "{name}: {description}");
    }
    for name in SHARED {
        let twin = tool(&stdio, name)["description"]
            .as_str()
            .unwrap_or_else(|| panic!("stdio {name}: no description"));
        if HUB_READ_TEXTED.contains(&name) {
            // docs/features/read-trust.md (review iteration 3): stdio's text
            // with its read tail become the hub's, byte for byte.
            assert!(
                twin.ends_with(STDIO_READ_TAIL),
                "premise: stdio {name} ends with its read tail: {twin}"
            );
            assert_eq!(
                tool(&hub, name)["description"],
                json!(twin.replace(STDIO_READ_TAIL, HUB_READ_TAIL)),
                "{name}: the stdio twin's description, the hub's read tail"
            );
        } else {
            assert_eq!(
                tool(&hub, name)["description"],
                tool(&stdio, name)["description"],
                "{name}: the stdio twin's description"
            );
        }
        assert_eq!(
            tool(&hub, name)["inputSchema"],
            tool(&stdio, name)["inputSchema"],
            "{name}: the stdio twin's input schema"
        );
        assert_eq!(
            tool(&hub, name)["outputSchema"],
            tool(&stdio, name)["outputSchema"],
            "{name}: the stdio twin's output schema"
        );
    }
    let mut shared_and_variants: Vec<&str> =
        SHARED.iter().chain(VARIANTS.iter()).copied().collect();
    shared_and_variants.sort_unstable();
    assert_eq!(
        shared_and_variants, HUB_TOOLS,
        "nine shared and nine variants"
    );
    for name in VARIANTS {
        let description = tool(&hub, name)["description"].as_str().expect("text");
        assert_ne!(
            tool(&hub, name)["description"],
            tool(&stdio, name)["description"],
            "{name}: a hub variant"
        );
        for word in [
            "spec approve",
            "spec reject",
            "task_id",
            "get_task",
            "You act as `agent`",
        ] {
            assert!(!description.contains(word), "{name} holds {word:?}");
        }
    }
    let roadmap = tool(&hub, "get_roadmap")["description"]
        .as_str()
        .expect("text");
    assert!(
        !roadmap.contains("the owner's decision on a terminal"),
        "get_roadmap's variant: {roadmap}"
    );
    for key in ["inputSchema", "outputSchema"] {
        assert_eq!(
            tool(&hub, "get_roadmap")[key],
            tool(&stdio, "get_roadmap")[key],
            "get_roadmap: the stdio twin's {key}"
        );
    }
    for name in QUEUE_WRITES.iter().chain(CARD_WRITES.iter()) {
        let mut twin = tool(&stdio, name)["inputSchema"]["properties"].clone();
        if let Some(properties) = twin.as_object_mut() {
            properties.remove("task_id");
        }
        assert_eq!(
            tool(&hub, name)["inputSchema"]["properties"],
            twin,
            "{name}: the stdio twin's properties but task_id"
        );
    }
    for name in QUEUE_WRITES {
        let description = tool(&hub, name)["description"].as_str().expect("text");
        assert!(
            description.contains("Confirm"),
            "{name}: a person's Confirm: {description}"
        );
    }
    for name in CARD_WRITES {
        let description = tool(&hub, name)["description"].as_str().expect("text");
        assert!(description.contains(ACTS_AS_LOGIN), "{name}: {description}");
    }
}

// ---------------------------------------------------------------------- H13

/// H13: `Authorization` with a valid `Remote-User` is a 403 at step 5 on the
/// page, a read, a stage and a confirm, once or twice sent, nothing stored; on
/// `/mcp/lantern-keep` it is answered as without it.
#[test]
fn h13_authorization_is_for_mcp_alone_on_a_hub() {
    let world = World::new("mcp-h13");
    let id = common::ask(
        &world.home,
        &world.keep,
        &["EDGE-STAM-ZERO"],
        "Is zero an end?",
    );
    let base = format!("/api/projects/{KEEP}/proposals/{id}");
    let stored = world.review(&id);
    let before = world.events();
    let stage = json!({"decision": "approve", "updated_at": stored["updated_at"]}).to_string();
    let confirm = json!({"question": "any", "updated_at": stored["updated_at"]}).to_string();
    let tree = format!("/api/projects/{KEEP}/tree");
    for times in [1, 2] {
        let mut read = Proxied::fetch("mira");
        let mut writes = Proxied::write("tomas");
        for _ in 0..times {
            read = read.plus("Authorization", BASIC);
            writes = writes.plus("Authorization", BASIC);
        }
        for (method, path, proxied, body) in [
            ("GET", "/".to_owned(), &read, ""),
            ("GET", tree.clone(), &read, ""),
            ("POST", format!("{base}/decision"), &writes, stage.as_str()),
            ("POST", format!("{base}/confirm"), &writes, confirm.as_str()),
        ] {
            let reply = through(&world.server, method, &path, proxied, body.as_bytes());
            assert_refused(
                &reply,
                AUTHORIZATION_LINE,
                &format!("{method} {path}, Authorization x{times}"),
            );
        }
    }
    assert_eq!(world.events(), before, "nothing stored");
    assert_eq!(world.review(&id), stored, "the proposal as it was");
    world.page("mira", &tree).status(200);

    let path = mcp_path(KEEP);
    let with_header = Session::initialize_by(
        world.port(),
        &path,
        &proxied("mira").plus("Authorization", BASIC),
    );
    let mut opened = Session::opened(world.port(), &path, "mira", &with_header);
    let (list_id, listed) = {
        let id = opened.next_id();
        let answer = post(
            world.port(),
            &path,
            &opened.headers().plus("Authorization", BASIC),
            &message(id, "tools/list", json!({})),
        );
        (id, answer)
    };
    assert_eq!(listed.status(), 200, "{}", listed.text());
    let mut plain = world.session(KEEP, "mira");
    assert_eq!(
        listed.expect_response(list_id, "tools/list")["result"],
        json!(plain.tools()),
        "tools/list with Authorization is tools/list without it"
    );
}

/// H13, a local daemon: every route answers as without `Authorization`.
#[test]
fn h13_a_local_daemon_never_reads_authorization() {
    let scratch = Scratch::new("mcp-h13-local");
    let keep = scratch.repo("spec-a", "keep", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&keep]);
    let id = common::ask(&home, &keep, &["EDGE-STAM-ZERO"], "Is zero an end?");
    let page = format!("http://127.0.0.1:{}", server.port);
    for (method, path, body) in [
        ("GET", "/".to_owned(), ""),
        ("GET", "/api/projects".to_owned(), ""),
        ("GET", format!("/api/projects/{KEEP}/tree"), ""),
        ("GET", format!("/api/projects/{KEEP}/inbox"), ""),
        ("GET", format!("/api/projects/{KEEP}/proposals/{id}"), ""),
        ("GET", mcp_path(KEEP), ""),
        ("POST", mcp_path(KEEP), "{}"),
        (
            "POST",
            format!("/api/projects/{KEEP}/proposals/{id}/decision"),
            "not a stage",
        ),
    ] {
        let headers = [
            ("Sec-Fetch-Site", "same-origin"),
            ("Origin", page.as_str()),
            ("Content-Type", "application/json"),
        ];
        let mut authorized = headers.to_vec();
        authorized.push(("Authorization", BASIC));
        let without = server.send(method, &path, &headers, body.as_bytes());
        let with_it = server.send(method, &path, &authorized, body.as_bytes());
        assert_eq!(
            (with_it.status, &with_it.body),
            (without.status, &without.body),
            "{method} {path}"
        );
    }
}

// ---------------------------------------------------------------------- H14

/// A scratch corpus whose tree passes the cut: 2 000 one-line pages and one
/// ledger of 600 sections, each holding `lamp`, committed.
fn wide_corpus(scratch: &Scratch) -> PathBuf {
    let root = scratch.dir("wide");
    write(&root, "specengine.toml", "[project]\nslug = \"wide\"\n");
    for n in 0..2_000 {
        write(
            &root,
            &format!("docs/spec/p{n:04}.md"),
            format!("# Page {n} lamp\n\nThe lamp of page {n}.\n"),
        );
    }
    let mut ledger = String::from("# Ledger lamp\n\nEvery lamp of the keep.\n\n");
    for n in 0..600 {
        ledger.push_str(&format!(
            "## Entry {n} lamp\n\nThe lamp entry {n} holds a line of text long enough to count.\n\n"
        ));
    }
    write(&root, "docs/spec/ledger.md", ledger);
    let git = scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "wide"]);
    root
}

/// H14: over a corpus past the 40 000-character cut, `get_tree {}`, `get_node`
/// of its largest document and `search` of a common term answer over the
/// hub's `/mcp/wide` what `specengine-mcp --root` answers under the same
/// `HOME`: `content` byte for byte, `structuredContent` equal, the cut and its
/// tail included.
#[test]
fn h14_the_hub_reads_as_the_stdio_server_reads() {
    let scratch = Scratch::new("mcp-h14");
    let root = wide_corpus(&scratch);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::start(&home, &cwd, &hub_args(&[&root], ORIGIN));
    let mut session = Session::open(server.port, "wide", "mira");
    let calls = [
        ("get_tree", json!({})),
        ("get_node", json!({"id": "docs/spec/ledger.md"})),
        ("search", json!({"query": "lamp"})),
    ];
    // Warm both sides on one index state: neither compared call refreshes.
    session.call("get_tree", json!({}));
    let root_arg = root.as_os_str();
    let mut stdio = Mcp::start_with(&home, &root, &[OsStr::new("--root"), root_arg]);
    stdio.request(
        1,
        "tools/call",
        json!({"name": "get_tree", "arguments": {}}),
    );

    for (n, (name, args)) in calls.iter().enumerate() {
        let hub = result_of(&session.call(name, args.clone()), name).clone();
        let id = 10 + i64::try_from(n).expect("small");
        let twin = stdio.request(id, "tools/call", json!({"name": name, "arguments": args}));
        let twin = result_of(&twin, name).clone();
        assert!(!is_error(&hub), "{name}: {hub}");
        assert_eq!(content_text(&hub), content_text(&twin), "{name}: content");
        assert_eq!(
            hub["content"], twin["content"],
            "{name}: the content blocks"
        );
        assert_eq!(
            hub["structuredContent"], twin["structuredContent"],
            "{name}: structuredContent"
        );
        if *name != "search" {
            assert!(
                content_text(&hub).contains("[truncated: "),
                "{name}: the corpus passes the cut, its tail shown: {} characters",
                content_text(&hub).chars().count()
            );
        }
    }
    stdio.finish();
}

// ---------------------------------------------------------------------- H15

/// H15, the daemon's half: with `GET .../events` open as `tomas`, a
/// `report_discrepancy` as `mira` over `/mcp` arrives on it within 1 s; the
/// inbox lists it, and its read holds `author.login` `mira`.
#[test]
fn h15_a_raise_over_mcp_is_live_on_the_page() {
    let world = World::new("mcp-h15");
    let mut tail = Stream::open_raw(
        world.port(),
        &format!("/api/projects/{KEEP}/events"),
        &Proxied::fetch("tomas").pairs(),
    );
    assert_eq!(tail.status, 200, "the tail");
    let mut client = Client::open(Era::Legacy, world.port(), KEEP, "mira");
    let id = raised(
        &mut client,
        "report_discrepancy",
        with(
            discrepancy("EDGE-STAM-ZERO", "Zero arrives while resting."),
            &agent(),
        ),
    );
    let answered_at = Instant::now();
    let mut arrived = None;
    while answered_at.elapsed() < Duration::from_secs(1) {
        let left = Duration::from_secs(1).saturating_sub(answered_at.elapsed());
        let Some(frame) = tail.next_event(left) else {
            break;
        };
        if frame.event.as_deref() == Some("proposal.created")
            && frame.data.as_deref().is_some_and(|data| data.contains(&id))
        {
            arrived = Some(answered_at.elapsed());
            break;
        }
    }
    assert!(
        arrived.is_some(),
        "{id}'s proposal.created within 1 s on tomas's tail"
    );

    let inbox = world
        .page("tomas", &format!("/api/projects/{KEEP}/inbox"))
        .status(200)
        .json();
    let listed = inbox["proposals"]
        .as_array()
        .expect("proposals")
        .iter()
        .find(|entry| entry["id"] == json!(id))
        .unwrap_or_else(|| panic!("{id} not in the inbox: {inbox}"))
        .clone();
    assert_eq!(listed["kind"], json!("discrepancy"), "{listed}");
    let read = world
        .page("tomas", &format!("/api/projects/{KEEP}/proposals/{id}"))
        .status(200)
        .json();
    assert_eq!(read["author"]["login"], json!("mira"), "{read}");
}

// ---------------------------------------------------------------------- H16

/// Every file's bytes under `dir`, `.git` and SQLite's files included.
fn every_byte(dir: &Path) -> Vec<(String, Vec<u8>)> {
    snapshot(dir)
        .into_iter()
        .filter_map(|(path, bytes)| bytes.map(|bytes| (path, bytes)))
        .collect()
}

/// Whether `haystack` holds `needle`.
fn holds(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// H16: after sessions that read, raise with no `run`, write cards, stream,
/// end, and a confirm of one raise, no issued session id is in `spec export
/// state`'s dump, `git log --all`, any file under either root or `HOME` (the
/// SQLite files too), or the daemon's stderr.
#[test]
fn h16_no_session_id_is_stored_anywhere() {
    let world = World::new("mcp-h16");
    let mut issued = Vec::new();
    let mut mira = world.session(KEEP, "mira");
    issued.push(mira.id.clone());
    let identity = json!({"author_role": ROLE});
    let mut client = Client::Legacy(mira.clone());
    client.call("get_tree", json!({}));
    let update = update_args(&mut client, "EDGE-STAM-ZERO", "immediately", "at once");
    let update_id = raised(&mut client, "propose_change", with(update, &identity));
    raised(
        &mut client,
        "ask_question",
        with(question("EDGE-SPRINT-EMPTY", "Does it stop?"), &identity),
    );
    raised(
        &mut client,
        "report_discrepancy",
        with(discrepancy("EDGE-STAM-ZERO", "It departs."), &identity),
    );
    raised(
        &mut client,
        "report_decision",
        with(decision("RULE-SPRINT-COST", "Rounded?"), &identity),
    );
    let made = client.call(
        "create_card",
        json!({"title": "Lamp oil", "discipline": "sketch"}),
    );
    let card = answered(&made, "create_card")["id"]
        .as_str()
        .expect("a card")
        .to_owned();
    client.call("edit_card", json!({"card_id": card, "goal": "Fill it."}));
    client.call("move_card", json!({"card_id": card, "to": "shaped"}));
    let mut stream = mira.stream_as("mira");
    assert_eq!(stream.status, 200);
    stream.frames_within(Duration::from_millis(300));
    drop(stream);
    assert!(mira.answers());

    let mut tomas = world.session(KEEP, "tomas");
    issued.push(tomas.id.clone());
    tomas.tools();
    assert_eq!(tomas.delete_as("tomas").status(), 202);
    let mut far = world.session(ZERKALO, "mira");
    issued.push(far.id.clone());
    far.call("get_tree", json!({}));
    let refused = Session::initialize(world.port(), &mcp_path(KEEP), "agent");
    assert_eq!(refused.status(), 403);

    let document = world.confirmed_by_tomas(&update_id);
    assert_eq!(document["status"], json!("applied"), "{document}");

    let dump = world.scratch.join("state.jsonl");
    spec(
        &world.home,
        &world.cwd,
        &[
            "--root",
            world.keep.to_str().expect("UTF-8"),
            "export",
            "state",
            "--out",
            dump.to_str().expect("UTF-8"),
        ],
    )
    .code(0);
    let dumped = fs::read(&dump).expect("the dump");
    let git = world.scratch.git();
    let logs: Vec<String> = [&world.keep, &world.zerkalo]
        .iter()
        .map(|root| git.run(root, &["log", "--all", "--format=%B"]))
        .collect();
    let mut files = every_byte(&world.home);
    files.extend(every_byte(&world.keep));
    files.extend(every_byte(&world.zerkalo));
    assert!(
        files.iter().any(|(path, _)| path.ends_with(".db")),
        "the SQLite files are read"
    );
    let stderr = world.server.stderr();
    for id in &issued {
        assert_eq!(id.len(), 36, "a UUID: {id}");
        assert!(!holds(&dumped, id), "{id} in the dump");
        for log in &logs {
            assert!(!log.contains(id.as_str()), "{id} in git log");
        }
        for (path, bytes) in &files {
            assert!(!holds(bytes, id), "{id} in {path}");
        }
        assert!(!stderr.contains(id.as_str()), "{id} in the daemon's stderr");
    }
}

// ---------------------------------------------------------------------- H17

/// H17, a login's cap: `mira`'s fifth `initialize` on `lantern-keep` is a 200
/// with a new id and ends her least recently used session there, not her
/// oldest; her other three, the new one and `tomas`'s live; her fifth on
/// `zerkalo` ends nothing on `lantern-keep`; a session ended by DELETE holds
/// no place.
#[test]
fn h17_a_login_holds_four_sessions_a_project_and_the_least_recently_used_leaves() {
    let world = World::new("mcp-h17-login");
    let port = world.port();
    let path = mcp_path(KEEP);
    let mut mira: Vec<Session> = (0..4).map(|_| world.session(KEEP, "mira")).collect();
    let mut tomas: Vec<Session> = (0..2).map(|_| world.session(KEEP, "tomas")).collect();
    assert!(mira[0].answers(), "the oldest, used last");
    let answer = Session::initialize(port, &path, "mira");
    assert_eq!(answer.status(), 200, "the fifth: {}", answer.text());
    let mut fifth = Session::opened(port, &path, "mira", &answer);
    assert!(
        mira.iter().all(|session| session.id != fifth.id),
        "a new id"
    );
    assert!(!mira[1].answers(), "her least recently used session ended");
    for n in [0, 2, 3] {
        assert!(mira[n].answers(), "her session {n} lives");
    }
    assert!(fifth.answers(), "the new one lives");
    for (n, session) in tomas.iter_mut().enumerate() {
        assert!(session.answers(), "tomas's session {n} untouched");
    }

    let mut far: Vec<Session> = (0..4).map(|_| world.session(ZERKALO, "mira")).collect();
    let mut far_fifth = world.session(ZERKALO, "mira");
    assert!(
        !far[0].answers(),
        "her least recently used on zerkalo ended"
    );
    for session in far.iter_mut().skip(1) {
        assert!(session.answers(), "zerkalo");
    }
    assert!(far_fifth.answers());
    for n in [0, 2, 3] {
        assert!(
            mira[n].answers(),
            "lantern-keep's {n} after zerkalo's fifth"
        );
    }
    assert!(fifth.answers());

    // A DELETE frees its place: rmcp's ended session is not counted.
    tomas.push(world.session(KEEP, "tomas"));
    tomas.push(world.session(KEEP, "tomas"));
    for n in [1, 2, 3, 0] {
        assert!(tomas[n].answers(), "tomas {n}");
    }
    assert_eq!(tomas[0].delete_as("tomas").status(), 202);
    let mut again = world.session(KEEP, "tomas");
    for n in [1, 2, 3] {
        assert!(
            tomas[n].answers(),
            "tomas {n}: three live and one new are four"
        );
    }
    assert!(again.answers());
}

/// H17, the hub's cap: sixteen logins at four sessions fill it; a seventeenth's
/// `initialize` is the 503 with "Data"'s line and no id, and the 64 still
/// answer; a stateless call takes no place; a login at its own cap still gets
/// one, by its own least recently used; a DELETE frees a place; after a
/// restart every old id is rmcp's 404.
#[test]
fn h17_sixteen_logins_fill_the_hub_and_a_seventeenth_is_refused() {
    let mut world = World::new("mcp-h17-hub");
    let path = mcp_path(KEEP);
    let mut all: Vec<Session> = Vec::new();
    for n in 1..=16 {
        for _ in 0..4 {
            all.push(world.session(KEEP, &format!("l{n:02}")));
        }
    }
    let refused = Session::initialize(world.port(), &path, "l17");
    assert_eq!(refused.status(), 503, "{}", refused.text());
    assert_eq!(refused.reply.error_message(), CAP_LINE);
    assert!(refused.sessions().is_empty(), "{:?}", refused.reply.headers);
    for session in &mut all {
        assert!(session.answers(), "{} {}", session.login, session.id);
    }

    let mut alone = world.stateless(KEEP, "l17");
    let tree = alone.call("get_tree", json!({}));
    assert!(!is_error(result_of(&tree, "get_tree")), "{tree}");

    let mut own = world.session(KEEP, "l02");
    assert!(!all[4].answers(), "l02's least recently used");
    for session in all.iter_mut().skip(5).take(3) {
        assert!(session.answers(), "l02's others");
    }
    assert!(own.answers());
    for (n, session) in all.iter_mut().enumerate() {
        if !(4..8).contains(&n) {
            assert!(session.answers(), "{} {}", session.login, session.id);
        }
    }

    assert_eq!(all[0].delete_as("l01").status(), 202);
    let mut late = world.session(KEEP, "l17");
    assert!(late.answers());
    let full = Session::initialize(world.port(), &path, "l18");
    assert_eq!(full.status(), 503, "{}", full.text());

    world.restart();
    let port = world.port();
    for session in all.iter_mut().chain([&mut own, &mut late]) {
        session.port = port;
        assert!(!session.answers(), "{} after a restart", session.id);
    }
    let mut fresh = world.session(KEEP, "l01");
    assert!(fresh.answers(), "a new session after the restart");
}

// ------------------------------------------------------------ the harness

/// The client's own controls: SSE frames with a priming event and a
/// keep-alive yield the one message; a JSON body is one; nothing else none.
#[test]
fn the_client_reads_sse_and_json_answers() {
    let body = b"data: \nid: 0\nretry: 3000\n\n:\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n";
    assert_eq!(
        common::mcp_http::sse_messages(body),
        [json!({"jsonrpc": "2.0", "id": 1, "result": {}})]
    );
    assert!(has_word("a get_tree(b)", "get_tree"));
    assert!(!has_word("a get_tree_x b", "get_tree"));
    assert!(!has_word("prefs", "refs"));
    let mut keys = BTreeSet::new();
    property_keys(
        &json!({"properties": {"a": {}, "b": {"properties": {"task_id": {}}}}}),
        &mut keys,
    );
    assert_eq!(keys.into_iter().collect::<Vec<_>>(), ["a", "b", "task_id"]);
}

// ---------------------------------------------------------------------- H18

/// H18 (iteration 2): `mira` opens sessions A and B; a POST `tools/list`, a GET
/// and a DELETE, each naming two `Mcp-Session-Id` values (A and B, then A and
/// A), are each "Data"'s unknown-session 404, and afterwards A and B each
/// answer her `tools/list`. The count of rmcp's sessions against the bound ones
/// is a unit of `src/mcp.rs`. M: unbinding every named value and handing the
/// request to rmcp.
#[test]
fn h18_two_session_ids_in_one_request_are_the_unknown_session_404() {
    let world = World::new("mcp-h18");
    let port = world.port();
    let path = mcp_path(KEEP);
    let mut a = world.session(KEEP, "mira");
    let mut b = world.session(KEEP, "mira");
    let before = world.events();
    for (first, second) in [(a.id.clone(), b.id.clone()), (a.id.clone(), a.id.clone())] {
        let two = proxied("mira")
            .plus(SESSION_HEADER, &first)
            .plus(SESSION_HEADER, &second)
            .plus("MCP-Protocol-Version", LEGACY);
        let listed = exchange(
            port,
            "POST",
            &path,
            &common::mcp_http::posting(two.clone()),
            message(9, "tools/list", json!({})).to_string().as_bytes(),
            QUICK,
        );
        let streamed = exchange(
            port,
            "GET",
            &path,
            &two.clone().plus("Accept", "text/event-stream"),
            b"",
            QUICK,
        );
        let deleted = exchange(port, "DELETE", &path, &two, b"", QUICK);
        let pair = if first == second { "A, A" } else { "A, B" };
        for (method, answer) in [("POST", listed), ("GET", streamed), ("DELETE", deleted)] {
            assert_eq!(
                answer.status(),
                404,
                "{method} naming {pair}: {}",
                answer.text()
            );
            assert_eq!(answer.text(), NO_SESSION, "{method} naming {pair}");
            assert!(answer.sessions().is_empty(), "{method} naming {pair}");
        }
        assert!(a.answers(), "A after the requests naming {pair}");
        assert!(b.answers(), "B after the requests naming {pair}");
    }
    assert_eq!(world.events(), before, "nothing ran");
}

// ---------------------------------------------------------------------- H19

/// H19 (iteration 2): a `tools/call` naming `claim_task`, then 1 000 naming
/// `x-0` to `x-999`, in both eras: each is rmcp's unknown-tool error, JSON-RPC
/// `-32602` `tool not found` with the request's `id`, and nothing is stored.
/// That rmcp's handler never sees them, so its schema cache never takes a name,
/// is a unit of `src/mcp.rs` (its counting seam). Iteration 3 adds the two
/// shapes a typed filter misses: `arguments` no object (the error, its `id`)
/// and the same as a notification (a 202). M: handing the request to rmcp;
/// filtering only a typed `CallToolRequest`.
#[test]
fn h19_an_unknown_tool_name_is_rmcps_unknown_tool_error_and_nothing_runs() {
    let world = World::new("mcp-h19");
    let before = world.events();
    let names: Vec<String> = std::iter::once("claim_task".to_owned())
        .chain((0..1_000).map(|n| format!("x-{n}")))
        .collect();
    for era in ERAS {
        let mut client = Client::open(era, world.port(), KEEP, "mira");
        for name in &names {
            let params = json!({"name": name, "arguments": {}});
            let (id, answer) = match &mut client {
                Client::Legacy(session) => session.send("tools/call", params),
                Client::Stateless(stateless) => stateless.send("tools/call", params),
            };
            assert!(
                matches!(answer.status(), 200 | 400),
                "{era:?} {name}: status {}: {}",
                answer.status(),
                answer.text()
            );
            assert!(answer.sessions().is_empty(), "{era:?} {name}: a session");
            let response = answer.expect_response(id, name);
            assert_eq!(response["id"], json!(id), "{era:?} {name}");
            assert_eq!(
                response["error"]["code"],
                json!(-32602),
                "{era:?} {name}: {response}"
            );
            assert_eq!(
                response["error"]["message"],
                json!("tool not found"),
                "{era:?} {name}: {response}"
            );
        }
        // The client still works: a known tool answers after the 1 001.
        let tree = client.call("get_tree", json!({}));
        assert!(!is_error(&tree), "{era:?}: {tree}");
    }

    // Iteration 3: the shapes a typed filter misses, each with
    // `MCP-Protocol-Version: 2026-07-28`: (a) `arguments` no object, which
    // rmcp's parser takes for a custom request, is the unknown-tool error with
    // its `id`; (b) the same with no `id`, a notification, is a 202. That
    // neither reaches rmcp is the unit's (`Mounts::handled`).
    let path = mcp_path(KEEP);
    for sep in [false, true] {
        let mut headers =
            common::mcp_http::posting(proxied("mira").plus("MCP-Protocol-Version", STATELESS));
        if sep {
            headers = headers
                .plus("Mcp-Method", "tools/call")
                .plus("Mcp-Name", "x-1");
        }
        let malformed = exchange(
            world.port(),
            "POST",
            &path,
            &headers,
            br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x-1","arguments":5}}"#,
            QUICK,
        );
        assert!(
            matches!(malformed.status(), 200 | 400),
            "(a), SEP headers {sep}: {} {}",
            malformed.status(),
            malformed.text()
        );
        let response = malformed.expect_response(1, "(a)");
        assert_eq!(
            response["error"]["code"],
            json!(-32602),
            "(a) {sep}: {response}"
        );
        assert_eq!(
            response["error"]["message"],
            json!("tool not found"),
            "(a) {sep}: {response}"
        );
        let notified = exchange(
            world.port(),
            "POST",
            &path,
            &headers,
            br#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"x-1","arguments":5}}"#,
            QUICK,
        );
        assert_eq!(
            notified.status(),
            202,
            "(b), SEP headers {sep}: {}",
            notified.text()
        );
    }
    assert_eq!(world.events(), before, "nothing stored");
}

// ---------------------------------------------------------------------- H20

/// The `card.*` events of both projects.
fn card_events(world: &World) -> Vec<(String, Value)> {
    world
        .events()
        .into_iter()
        .filter(|(kind, _)| kind.starts_with("card."))
        .collect()
}

/// H20 (iteration 2): while a held confirm holds `lantern-keep`'s turn, a
/// stateless `create_card` whose connection closes after 300 ms and a session's
/// `create_card` followed by `notifications/cancelled` for its id never run:
/// once the confirm ends no card is stored and no `card.*` event written by
/// either, and a third `create_card` sent after the turn frees stores its card.
/// The turn is FIFO, so the third runs after anything that still waited. M: the
/// wait ignoring the cancellation, both cards stored once the turn frees.
#[test]
fn h20_a_call_cancelled_while_it_waits_for_the_turn_stores_nothing() {
    let world = World::new("mcp-h20");
    let port = world.port();
    let path = mcp_path(KEEP);
    let Held {
        confirm,
        entered,
        go,
    } = held_confirm(&world);
    let mut session = world.session(KEEP, "mira");
    let before = card_events(&world);

    let card = |title: &str| json!({"name": "create_card", "arguments": {"title": title, "discipline": "sketch"}});
    let stateless_call = {
        let mut params = card("Dropped by its client");
        params["_meta"] = common::mcp_http::stateless_meta();
        let headers =
            common::mcp_http::posting(Stateless::headers_as("mira", "tools/call", &params));
        headers.bytes(
            "POST",
            &path,
            message(1, "tools/call", params).to_string().as_bytes(),
        )
    };
    let cancelled_id = session.next_id();
    let session_call = common::mcp_http::posting(session.headers()).bytes(
        "POST",
        &path,
        message(cancelled_id, "tools/call", card("Cancelled by its client"))
            .to_string()
            .as_bytes(),
    );

    let (holder, cancel) = thread::scope(|scope| {
        let holder = scope.spawn(|| http_exchange(port, &confirm, HTTP_TIMEOUT));
        wait_entered(&entered);
        let mut dropped = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        dropped
            .write_all(&stateless_call)
            .expect("the stateless call");
        let mut waiting = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        waiting
            .write_all(&session_call)
            .expect("the session's call");
        thread::sleep(Duration::from_millis(300));
        drop(dropped);
        let cancel = post(
            port,
            &path,
            &session.headers(),
            &json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
                "params": {"requestId": cancelled_id, "reason": "The person stopped it."}}),
        );
        thread::sleep(Duration::from_millis(300));
        fs::write(&go, b"").expect("the gate");
        let holder = holder.join().expect("the holder");
        drop(waiting);
        (holder, cancel)
    });
    fs::remove_file(world.keep.join(".git/hooks/pre-commit")).expect("the hook");
    assert_eq!(holder.status, 200, "the confirm: {}", holder.text());
    assert_eq!(cancel.status(), 202, "the cancellation: {}", cancel.text());

    let third = session.call(
        "create_card",
        card("Sent once the turn freed")["arguments"].clone(),
    );
    let made = answered(result_of(&third, "create_card"), "create_card");
    let made_id = made["id"].as_str().expect("a card").to_owned();
    let added: Vec<(String, Value)> = card_events(&world).into_iter().skip(before.len()).collect();
    assert_eq!(added.len(), 1, "only the third's card: {added:?}");
    assert_eq!(added[0].0, "card.created", "{added:?}");
    let listed = session.call("list_cards", json!({}));
    let cards = answered(result_of(&listed, "list_cards"), "list_cards")["cards"].clone();
    let ids: Vec<&str> = cards
        .as_array()
        .expect("cards")
        .iter()
        .filter_map(|card| card["id"].as_str())
        .collect();
    assert_eq!(ids, [made_id.as_str()], "one card on the board");
}

/// docs/canon/card-flows.md "MCP and hub": on a hub too,
/// `create_card`'s text names `reviewer`, `move_card`'s `note` and
/// `list_cards`' `parent`, each takes its key, and the hub's tools stay the
/// seventeen, ASCII and within 2 048 bytes (H11 judges every text).
#[test]
fn board_handoff_the_hub_card_texts_name_the_new_keys() {
    let world = World::new("mcp-handoff");
    let mut session = world.session(KEEP, "mira");
    let hub = session.tools();
    let names: Vec<&str> = hub["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name"))
        .collect();
    // department-split S8: eighteen.
    assert_eq!(names.len(), 18, "{names:?}");
    for (name, key) in [
        ("create_card", "reviewer"),
        ("move_card", "note"),
        ("list_cards", "parent"),
    ] {
        let description = tool(&hub, name)["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: no description"));
        assert!(has_word(description, key), "{name}: {description}");
        assert!(description.is_ascii(), "{name}");
        assert!(
            description.len() <= TEXT_LIMIT,
            "{name}: {}",
            description.len()
        );
        assert!(
            tool(&hub, name)["inputSchema"]["properties"]
                .as_object()
                .is_some_and(|properties| properties.contains_key(key)),
            "{name} takes {key}"
        );
    }
}

// ------------------------------------------------ card-tasks (RM-0092) CT-7

/// The note of each linked task a hub serves (card-tasks CT-7), after
/// `<id>: `.
const HUB_TASK_NOTE: &str = "a hub reads no task; its state is on the terminal that holds it";

/// docs/features/card-tasks.md CT-7 over `/mcp`, both eras: in a hub-served
/// `lantern-keep` whose queue holds `T-0001` (`review`, by SQL), `edit_card
/// {task: ["T-0001", "T-0042"]}` by `mira` is stored, `card.edited` acting as
/// her; `get_card` serves every entry `null` with the hub's note per ID, the
/// same `tasks` and notes a page's `GET .../cards/C` serves; `["X-1"]`, a
/// repeat and 33 entries are error results with the local reason, nothing
/// stored. The hub's `edit_card` text names `task` and says the hub checks
/// none, ASCII within 2 048 bytes, among 17 tools. M13, M14.
#[test]
fn card_tasks_ct7_the_hub_tools_keep_the_ids_and_read_no_task() {
    let world = World::new("mcp-card-tasks");
    let made = spec(
        &world.home,
        &world.keep,
        &[
            "--json",
            "task",
            "new",
            "--nodes",
            "MEC-STAMINA",
            "--title",
            "Stamina pass",
        ],
    );
    made.code(0);
    assert_eq!(made.json()["id"], json!("T-0001"), "{}", made.show());
    let done = std::process::Command::new("/usr/bin/sqlite3")
        .arg(common::data_dir(&world.home).join(format!("{KEEP}.db")))
        .arg("UPDATE tasks SET status = 'review' WHERE id = 'T-0001';")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(done.status.success(), "{done:?}");
    let many: Vec<String> = (1..=33).map(|n| format!("T-{n:04}")).collect();
    for era in ERAS {
        let mut client = Client::open(era, world.port(), KEEP, "mira");
        let made = client.call(
            "create_card",
            json!({"title": format!("Linked {era:?}"), "discipline": "sketch"}),
        );
        let card = answered(&made, "create_card")["id"]
            .as_str()
            .expect("a card")
            .to_owned();
        let edited = client.call(
            "edit_card",
            json!({"card_id": card, "task": ["T-0001", "T-0042"]}),
        );
        answered(&edited, "edit_card");
        assert_eq!(
            world.events().pop(),
            Some((
                "card.edited".to_owned(),
                json!({"id": card, "fields": ["tasks"], "as": "mira"})
            )),
            "{era:?}"
        );
        let got = client.call("get_card", json!({"card_id": card}));
        let document = answered(&got, "get_card");
        assert_eq!(
            document["tasks"],
            json!([
                {"id": "T-0001", "status": null, "title": null, "stale": null},
                {"id": "T-0042", "status": null, "title": null, "stale": null},
            ]),
            "{era:?}: {document}"
        );
        assert_eq!(
            document["notes"],
            json!([
                format!("T-0001: {HUB_TASK_NOTE}"),
                format!("T-0042: {HUB_TASK_NOTE}"),
            ]),
            "{era:?}: {document}"
        );
        let page = world.page("tomas", &format!("/api/projects/{KEEP}/cards/{card}"));
        assert_eq!(page.status, 200, "{}", page.text());
        let page = page.json();
        assert_eq!(page["tasks"], document["tasks"], "{era:?}: the page's read");
        assert_eq!(page["notes"], document["notes"], "{era:?}: the page's read");

        for (task, reason) in [
            (
                json!(["X-1"]),
                "task: \"X-1\" is no task ID; nothing changed",
            ),
            (
                json!(["T-0042", "T-0042"]),
                "task: T-0042 is listed twice; nothing changed",
            ),
            (json!(many), "task: 33 tasks, at most 32; nothing changed"),
        ] {
            let before = world.events();
            let refused = client.call("edit_card", json!({"card_id": card, "task": task}));
            assert!(is_error(&refused), "{era:?} {task}: {refused}");
            let text = content_text(&refused);
            assert!(
                text.trim_end().ends_with(reason),
                "{era:?} {task}: {text:?}"
            );
            assert_eq!(world.events(), before, "{era:?} {task}: nothing stored");
        }
    }

    let mut session = world.session(KEEP, "mira");
    let hub = session.tools();
    // department-split S8: eighteen.
    assert_eq!(tool_names(&hub).len(), 18);
    let description = tool(&hub, "edit_card")["description"]
        .as_str()
        .expect("a description");
    assert!(has_word(description, "task"), "{description}");
    assert!(
        description.contains("This hub holds no task: it checks none"),
        "{description}"
    );
    assert!(description.is_ascii());
    assert!(description.len() <= TEXT_LIMIT, "{}", description.len());
}

// ---------------------------------------------- card-questions (RM-0097)

/// docs/features/card-questions.md H3, the daemon's half: over a hub's
/// `/mcp`, `ask_question` with `card_id` stores it under the login (its
/// author's `login` the request's); `report_decision` takes `card_id` and
/// `why`; `report_discrepancy` takes `card_id`; the three hub schemas name
/// `card_id`, `report_decision` `why`, none `task_id`; a missing card is an
/// error result, nothing stored. M: `HubQuestionArgs` without `card_id`.
#[test]
fn card_questions_h3_the_hub_tools_take_card_id_and_why() {
    let world = World::new("mcp-cq-h3");
    let made = world.page_post(
        "mira",
        &format!("/api/projects/{KEEP}/cards"),
        &json!({"title": "Rest regeneration", "discipline": "sketch",
            "target": ["RULE-STAM-REGEN"]}),
    );
    assert_eq!(made.status, 200, "{}", made.text());
    assert_eq!(made.json()["id"], json!("C-0001"));
    let mut client = Client::open(Era::Legacy, world.port(), KEEP, "mira");
    let card = json!({"card_id": "C-0001"});

    let asked = raised(
        &mut client,
        "ask_question",
        with(
            with(
                question("EDGE-SPRINT-EMPTY", "Does the sprint end at once?"),
                &agent(),
            ),
            &card,
        ),
    );
    let review = world.review(&asked);
    assert_eq!(review["card_id"], json!("C-0001"), "{review}");
    assert_eq!(review["author"]["login"], json!("mira"), "{review}");
    assert_eq!(review["why"], Value::Null);

    let why = "Two playtests read instant regeneration as a bug.";
    let decided = raised(
        &mut client,
        "report_decision",
        with(
            with(
                decision("EDGE-STAM-ZERO", "Is zero stamina a stop?"),
                &agent(),
            ),
            &json!({"card_id": "C-0001", "why": why}),
        ),
    );
    let review = world.review(&decided);
    assert_eq!(review["card_id"], json!("C-0001"), "{review}");
    assert_eq!(review["why"], json!(why), "{review}");
    assert_eq!(review["author"]["login"], json!("mira"));

    let reported = raised(
        &mut client,
        "report_discrepancy",
        with(
            with(
                discrepancy("MEC-SPRINT", "Sprinting drains while walking."),
                &agent(),
            ),
            &card,
        ),
    );
    assert_eq!(world.review(&reported)["card_id"], json!("C-0001"));

    let before = world.events();
    let result = client.call(
        "ask_question",
        with(
            with(question("MEC-STAMINA", "Is stamina one bar?"), &agent()),
            &json!({"card_id": "C-0099"}),
        ),
    );
    assert!(is_error(&result), "{result}");
    assert!(
        content_text(&result).contains("card_id: no card C-0099"),
        "{result}"
    );
    assert_eq!(world.events(), before, "nothing stored");

    let list = client.tools();
    for name in ["ask_question", "report_discrepancy", "report_decision"] {
        let schema = &tool(&list, name)["inputSchema"];
        let properties = schema["properties"].as_object().expect("properties");
        assert_eq!(
            properties["card_id"]["type"],
            json!(["string", "null"]),
            "{name}"
        );
        assert!(!properties.contains_key("task_id"), "{name}: {schema}");
        assert_eq!(schema["additionalProperties"], json!(false), "{name}");
        let description = tool(&list, name)["description"]
            .as_str()
            .expect("a description");
        assert!(description.is_ascii(), "{name}");
        assert!(description.contains("card_id"), "{name}: {description}");
    }
    let decision_schema = &tool(&list, "report_decision")["inputSchema"]["properties"];
    assert_eq!(decision_schema["why"]["type"], json!(["string", "null"]));
    assert!(
        tool(&list, "ask_question")["inputSchema"]["properties"]
            .get("why")
            .is_none(),
        "a question takes no why"
    );
}
