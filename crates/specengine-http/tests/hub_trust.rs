//! docs/features/read-trust.md (RM-0054) D9, the hub's MCP turn
//! (`HubSeam::turn` in `src/mcp.rs`): a call that cannot run, or that
//! panics, inside the project's turn doubts the project, so the next read
//! walks and shows a change the trusted index lacks. Through a real hub
//! over `fixtures/card-press` (its `[team]` added), a hand edit on the host
//! made after a walking read, which a trusted `search` does not see:
//! - `d9_*cannot_run*`: with the config's slug changed on disk, a hub MCP
//!   call finds no project and cannot run; the slug restored, the next
//!   `search` finds the edit. M: the doubt on `cannot_run` dropped.
//! - `d9_*panic*`, `#[ignore]`d, in the `probes` build alone: the hub's
//!   `probe_panic` panics inside the turn; the next `search` finds the
//!   edit. M: the panic's doubt dropped.
//!
//! - `d9_*exit_2*`: a hub MCP read whose command cannot run doubts
//!   (`doubt_in_scope`).
//! - `rt_*`: only reads trust (review iteration 2): an MCP write is judged
//!   from the disk; a route and a tool off the read lists walk; trace data
//!   (`flow?trace=true`, `cards?marks=true`, MCP `list_cards` with `marks`)
//!   walks while its plain forms trust (iteration 3), seen through a file
//!   added by hand, which no trusted read's byte check can see.
//!
//! `answer::run`'s doubts are the daemon's unit tests
//! (`src/read_trust_tests.rs`). Between the event and the read that shows
//! the edit no other read runs, so only the event can have marked the
//! project.

#![cfg(unix)]

mod common;

use common::hub::{Hub, Setup};
use common::mcp_http::{Stateless, content_text, is_error, result_of, tool_names};
use common::{read_text, replace, write};
use serde_json::json;

/// The project, its route base and the one login of its `[team]`.
const SLUG: &str = "card-press";
const BASE: &str = "/api/projects/card-press";
const TEAM: &str = "\n[team.ann]\nrole = \"owner\"\nemail = \"ann@example.org\"\n";
const LOGIN: &str = "ann";
/// The hand edit: the back sheet's tide table renamed.
const BACK_MD: &str = "docs/spec/sht-back.md";
const BACK_ID: &str = "SHT-BACK";
const EDITED_WORD: &str = "zavoqui";

/// A hub over the print shop with its `[team]`, after a first read, which
/// walks.
fn team_hub(name: &str) -> Hub {
    let setup = Setup::new(name, "card-press");
    let mut config = read_text(&setup.root, "specengine.toml");
    config.push_str(TEAM);
    write(&setup.root, "specengine.toml", config);
    setup
        .scratch
        .git()
        .run(&setup.root, &["commit", "-qam", "the team"]);
    let hub = setup.start();
    assert!(
        search(&hub, "notices").contains(BACK_ID),
        "premise: the first read walks"
    );
    hub
}

/// A hand edit on the host, which a trusted `search` misses.
fn hand_edit(hub: &Hub) {
    replace(
        &hub.setup.root,
        BACK_MD,
        "the tide table",
        &format!("the {EDITED_WORD} table"),
    );
    assert!(
        !search(hub, EDITED_WORD).contains(BACK_ID),
        "premise: a trusted search misses the hand edit"
    );
}

/// [`team_hub`], then [`hand_edit`].
fn edited_hub(name: &str) -> Hub {
    let hub = team_hub(name);
    hand_edit(&hub);
    hub
}

/// `GET <base>/search?query=<word>`: a 200, its body.
fn search(hub: &Hub, word: &str) -> String {
    let reply = hub.get(LOGIN, &format!("{BASE}/search?query={word}"));
    assert_eq!(reply.status, 200, "search {word}: {}", reply.text());
    reply.text().to_owned()
}

#[test]
fn d9_a_hub_mcp_call_that_cannot_run_doubts_the_project() {
    let hub = edited_hub("trust-cannot-run");
    let root = hub.setup.root.clone();
    let config = read_text(&root, "specengine.toml");
    let renamed = config.replacen(
        &format!("slug = \"{SLUG}\""),
        &format!("slug = \"{SLUG}-moved\""),
        1,
    );
    assert_ne!(renamed, config, "premise: the slug line");
    write(&root, "specengine.toml", &renamed);
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let response = client.call("get_node", json!({"id": BACK_ID}));
    let result = result_of(&response, "get_node");
    assert!(
        is_error(result),
        "premise: with the slug moved the call cannot run: {result}"
    );
    write(&root, "specengine.toml", &config);
    assert!(
        search(&hub, EDITED_WORD).contains(BACK_ID),
        "the call that could not run doubted the project: the next read walked"
    );
}

#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test hub_trust --run-ignored ignored-only"]
fn d9_a_panicking_hub_mcp_call_doubts_the_project() {
    let hub = edited_hub("trust-panic");
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let tools = client.tools();
    assert!(
        tool_names(&tools).iter().any(|name| name == "probe_panic"),
        "premise: the probes build routes probe_panic on a hub: {:?}",
        tool_names(&tools)
    );
    let (id, answer) = client.send(
        "tools/call",
        json!({"name": "probe_panic", "arguments": {}}),
    );
    let response = answer.expect_response(id, "probe_panic");
    assert_eq!(
        response["error"]["message"],
        json!("internal error: the call failed"),
        "premise: the call reached the turn and panicked there (the plan's answer), \
         not a refusal: {response}"
    );
    assert!(
        search(&hub, EDITED_WORD).contains(BACK_ID),
        "the call that panicked doubted the project: the next read walked\n\
         probe_panic answered {response}\n--- the daemon's stderr\n{}",
        hub.server.stderr()
    );
}

/// A hub MCP read whose own command cannot run (exit 2: a look-alike ID)
/// doubts every trust in its scope (`specengine_cli::doubt_in_scope`): the
/// next read walks. The read itself trusts, so it indexes nothing. M: the
/// MCP path's doubt on its command's exit 2 dropped.
#[test]
fn d9_a_hub_mcp_read_whose_command_cannot_run_doubts_the_project() {
    let hub = edited_hub("trust-mcp-exit-2");
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let look_alike = format!("\u{0405}{}", &BACK_ID[1..]);
    let response = client.call("get_node", json!({"id": look_alike}));
    let result = result_of(&response, "get_node");
    assert!(
        is_error(result),
        "premise: a look-alike ID cannot run: {result}"
    );
    assert!(
        search(&hub, EDITED_WORD).contains(BACK_ID),
        "the MCP read that could not run doubted the project: the next read walked"
    );
}

/// A hub MCP write walks while reads trust: `get_node` (a trusted read)
/// gives the span's hash, a hand edit moves the span, and `propose_change`
/// on that hash is judged from the disk: refused as stale. M: the write's
/// freshness step trusting the index.
#[test]
fn rt_a_hub_mcp_write_is_judged_from_the_disk() {
    let hub = team_hub("trust-mcp-write");
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let response = client.call("get_node", json!({"id": BACK_ID}));
    let node = result_of(&response, "get_node")["structuredContent"]["nodes"][0].clone();
    let base = node["span_hash"].as_str().expect("span_hash").to_owned();
    let text = node["text"].as_str().expect("text").to_owned();
    assert!(text.contains("notices"), "premise: {text}");
    hand_edit(&hub);
    let response = client.call(
        "propose_change",
        json!({"kind": "update", "target": BACK_ID, "base": base,
            "text": text.replacen("notices", "bulletins", 1),
            "rationale": "A clearer line.", "author_role": "drafter"}),
    );
    let result = result_of(&response, "propose_change");
    assert!(
        is_error(result) && content_text(result).contains("is stale"),
        "the write judged the span as it is on disk: {result}"
    );
}

/// The node a contrasting read shows: in another file than the edit's.
const INNER_ID: &str = "SHT-INNER";

/// A route off the read list walks: after a hand edit, `GET .../nodes/`
/// of a node in another file (a read; the edited file is not in its
/// subset) leaves the index as it was, `GET .../trace` walks, and the next
/// trusted `search` finds the edit. M: `trace` on the read list.
#[test]
fn rt_a_route_off_the_read_list_walks() {
    let hub = edited_hub("trust-route-walks");
    let shown = hub.get(LOGIN, &format!("{BASE}/nodes/{INNER_ID}"));
    assert_eq!(shown.status, 200, "{}", shown.text());
    assert!(
        !search(&hub, EDITED_WORD).contains(BACK_ID),
        "premise: `nodes`, a read, trusted the index"
    );
    let trace = hub.get(LOGIN, &format!("{BASE}/trace?ref={BACK_ID}"));
    assert_eq!(trace.status, 200, "{}", trace.text());
    assert!(
        search(&hub, EDITED_WORD).contains(BACK_ID),
        "`trace` walked and updated the index"
    );
}

/// A tool off the hub's read list walks: after a hand edit, `get_node` of a
/// node in another file (a read) leaves the index as it was, `get_trace`
/// walks, and the next trusted `search` finds the edit. M: `get_trace`
/// among the read tools.
#[test]
fn rt_a_tool_off_the_read_list_walks() {
    let hub = edited_hub("trust-tool-walks");
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let response = client.call("get_node", json!({"id": INNER_ID}));
    assert!(!is_error(result_of(&response, "get_node")), "{response}");
    assert!(
        !search(&hub, EDITED_WORD).contains(BACK_ID),
        "premise: `get_node`, a read, trusted the index"
    );
    let response = client.call("get_trace", json!({"ref": BACK_ID}));
    assert!(!is_error(result_of(&response, "get_trace")), "{response}");
    assert!(
        search(&hub, EDITED_WORD).contains(BACK_ID),
        "`get_trace` walked and updated the index"
    );
}

/// A node added by hand on the host: a file the index does not hold, so
/// no trusted read's check of the bytes it prints can see it.
const ADDED_MD: &str = "docs/spec/sht-zeta.md";
const ADDED_ID: &str = "SHT-ZETA";
const ADDED_WORD: &str = "qorvexa";

/// A card on the back sheet, made through the hub's MCP (a write, which
/// walks): its marks read the index (a card with no target reads none).
fn card_on_back(hub: &Hub) {
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let response = client.call(
        "create_card",
        json!({"title": "Tide lines", "discipline": "copy", "target": [BACK_ID]}),
    );
    let result = result_of(&response, "create_card");
    assert!(!is_error(result), "premise: a card on {BACK_ID}: {result}");
}

fn add_by_hand(hub: &Hub) {
    write(
        &hub.setup.root,
        ADDED_MD,
        format!(
            "---\nid: {ADDED_ID}\nclass: canon\ntitle: The zeta sheet\nparent: PRN-WEEKLY\n\
             owner: owner\nreviewed: 2026-10-01\n---\n\n# The zeta sheet\n\n\
             The zeta sheet carries the {ADDED_WORD} notices.\n"
        ),
    );
    assert!(
        !search(hub, ADDED_WORD).contains(ADDED_ID),
        "premise: a trusted search misses the added file"
    );
}

/// Trace data walks (review iteration 3): after a file added by hand,
/// plain `flow` and `cards` trust the index and leave it as it was, while
/// `flow?trace=true` judges the disk at once: the next trusted `search`
/// finds the added node. M: `flow` trusting with `trace`.
#[test]
fn rt_flow_with_trace_walks_while_plain_flow_trusts() {
    let hub = team_hub("trust-flow-trace");
    card_on_back(&hub);
    add_by_hand(&hub);
    for route in ["flow", "cards"] {
        let reply = hub.get(LOGIN, &format!("{BASE}/{route}"));
        assert_eq!(reply.status, 200, "{route}: {}", reply.text());
        assert!(
            !search(&hub, ADDED_WORD).contains(ADDED_ID),
            "plain `{route}` trusts the index"
        );
    }
    let reply = hub.get(LOGIN, &format!("{BASE}/flow?trace=true"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(
        search(&hub, ADDED_WORD).contains(ADDED_ID),
        "`flow?trace=true` walked"
    );
}

/// `cards?marks=true` walks (review iteration 3). M: `cards` trusting
/// with `marks`.
#[test]
fn rt_cards_with_marks_walks() {
    let hub = team_hub("trust-cards-marks");
    card_on_back(&hub);
    add_by_hand(&hub);
    let reply = hub.get(LOGIN, &format!("{BASE}/cards?marks=true"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(
        search(&hub, ADDED_WORD).contains(ADDED_ID),
        "`cards?marks=true` walked"
    );
}

/// The hub's MCP `list_cards` trusts, and with `marks: true` walks
/// (review iteration 3). M: `reads_trust` ignoring `marks`.
#[test]
fn rt_mcp_list_cards_with_marks_walks() {
    let hub = team_hub("trust-mcp-marks");
    card_on_back(&hub);
    add_by_hand(&hub);
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let response = client.call("list_cards", json!({}));
    assert!(!is_error(result_of(&response, "list_cards")), "{response}");
    assert!(
        !search(&hub, ADDED_WORD).contains(ADDED_ID),
        "plain `list_cards` trusts the index"
    );
    let response = client.call("list_cards", json!({"marks": true}));
    assert!(!is_error(result_of(&response, "list_cards")), "{response}");
    assert!(
        search(&hub, ADDED_WORD).contains(ADDED_ID),
        "`list_cards` with marks walked"
    );
}

/// A hub MCP card write walks while reads trust: a node added by hand on
/// the host is a target `create_card` resolves at once (its targets are
/// read from the index, through the freshness step). M: a write tool
/// classed as a read.
#[test]
fn rt_a_hub_mcp_card_write_sees_a_node_added_by_hand() {
    let hub = team_hub("trust-mcp-card-write");
    add_by_hand(&hub);
    let mut client = Stateless::new(hub.server.port, SLUG, LOGIN);
    let response = client.call(
        "create_card",
        json!({"title": "Zeta lines", "discipline": "copy", "target": [ADDED_ID]}),
    );
    let result = result_of(&response, "create_card");
    assert!(
        !is_error(result),
        "the write resolved the node added by hand: {result}"
    );
}
