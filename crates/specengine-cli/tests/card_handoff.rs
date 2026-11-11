//! docs/features/board-handoff.md (RM-0085, v4 step 3a, ADR-0074), the CLI
//! half: `spec card` through the binary on scratch git repositories of
//! `fixtures/studio-lamp` (a lighthouse game holding the gamedev preset's
//! bytes: five trades' flows, each with the return column `sent-back`) and
//! of `fixtures/card-press` (a print shop whose flows name no `return`, and
//! whose `blocked` column is only a word), so the hand-off and the return run
//! on one project and stay absent on another (ADR-0008).
//!
//! - BH-12: `spec card new --reviewer N` stores the child's reviewer and
//!   `card.created` carries it (`null` without the flag); `agent`, `local` and
//!   a name off the person grammar exit 1 with `assign`'s own texts, writing
//!   nothing; the child at `verify` is accepted by its reviewer and refused
//!   to its assignee. M14: `card_new` dropping `reviewer`.
//! - BH-13: `flows[]` serves `return` after `accept`, `"sent-back"` on
//!   studio-lamp and `null` on card-press, every other byte of card-press's
//!   document as before; each of the five errors of docs/canon/card-flows.md
//!   "Config" exits 2 at its line for `spec card` alone (`spec tree`, `spec check` and `spec
//!   workspace` answer as they did, the last with the card's note). M15: the
//!   first column admitted as `return`.
//! - BH-14: a move into `sent-back` from another column without `--note`
//!   exits 1 with the line of docs/canon/card-flows.md "Return", no event,
//!   nothing changed; with a
//!   note it moves and `card.moved` holds it; a note on any other move is
//!   kept, none is `null`; a reorder inside the column and a move out of it
//!   need none; a blank note and one of 4 097 bytes exit 1 in `cancel
//!   --reason`'s grammar. M16: the note required on every move. M17: the note
//!   never required.
//! - BH-17: no card write moves, creates, refuses or re-reads another card: a
//!   parent with an open child and a returned one moves and is accepted; a
//!   child moves and is accepted under an accepted parent; a child's writes
//!   add no event of its parent and leave its `revision`. M20: accept
//!   refused while a child is open.
//! - BH-19: `spec card list --parent C` lists exactly C's children with
//!   every flow line; no child, or no such card, lists none, exit 0; a C off
//!   the card-ID grammar and a look-alike exit 2. M22: the filter ignored.
//!
//! The store half is `specengine-store`'s `queue_cards.rs`, the tools
//! `mcp_cards.rs`, the daemon `specengine-http`'s `cards.rs`, `query.rs`,
//! `door.rs`, `hub_team.rs`. The oracle of "nothing changed" is `SELECT *
//! FROM cards` and `SqliteQueue::dump()`, both raw. Every expected value
//! below is worked out from the spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use common::proposal::Pair;
use common::{fixture, read_text, write};
use serde_json::{Value, json};
use task_common::Ordered;

/// The game of five trades, each flow with `return = "sent-back"`.
const LAMP: &str = "studio-lamp";

/// The print shop, whose flows name no `return`.
const PRESS: &str = "card-press";

/// The return column of every preset flow (W11).
const BACK: &str = "sent-back";

/// The reason of BH-14.
const REASON: &str = "Two endings contradict";

/// card-press's `spec card list --json` on an empty queue: the board-ui
/// document of docs/canon/cards.md "Documents and events", columns titled by
/// their IDs, `copy`'s moves as written, `plates`' every ordered pair, and
/// `"return":null` after each `accept` (docs/canon/cards.md
/// "Documents and events"): no other byte moves.
const PRESS_LIST_JSON: &str = "{\"flows\":[{\"name\":\"copy\",\"title\":\"Copy desk\",\
     \"disciplines\":[\"copy\"],\
     \"columns\":[{\"id\":\"draft\",\"title\":\"draft\"},{\"id\":\"edit\",\"title\":\"edit\"},\
     {\"id\":\"proof\",\"title\":\"proof\"},{\"id\":\"filed\",\"title\":\"filed\"}],\"moves\":\
     [{\"from\":\"draft\",\"to\":\"edit\",\"by\":[\"desk\"]},{\"from\":\"edit\",\"to\":\"proof\",\
     \"by\":[]}],\"accept\":{\"from\":\"proof\",\"to\":\"filed\"},\"return\":null},{\"name\":\
     \"plates\",\"title\":\"plates\",\"disciplines\":[\"plates\"],\"columns\":[{\"id\":\"queued\",\"title\":\"queued\"},\
     {\"id\":\"blocked\",\"title\":\"blocked\"},{\"id\":\"checked\",\"title\":\"checked\"},{\"id\":\
     \"struck\",\"title\":\"struck\"}],\"moves\":[{\"from\":\"queued\",\"to\":\"blocked\",\"by\":[]},\
     {\"from\":\"queued\",\"to\":\"checked\",\"by\":[]},{\"from\":\"blocked\",\"to\":\"queued\",\
     \"by\":[]},{\"from\":\"blocked\",\"to\":\"checked\",\"by\":[]},{\"from\":\"checked\",\"to\":\
     \"queued\",\"by\":[]},{\"from\":\"checked\",\"to\":\"blocked\",\"by\":[]}],\"accept\":{\"from\":\
     \"checked\",\"to\":\"struck\"},\"return\":null}],\"cards\":[],\"notes\":[],\"truncated\":false,\
     \"omitted\":null}\n";

/// The preset's flows, `(name, columns, accept.from, accept.to)`, as
/// docs/canon/workspaces.md "Preset" and docs/canon/card-flows.md "Config"
/// give them: `sent-back` second in each. Six since
/// docs/features/department-split.md "Acceptance criteria", S9 (`sound`).
const LAMP_FLOWS: [(&str, &[&str], &str, &str); 6] = [
    (
        "narrative",
        &[
            "backlog",
            "sent-back",
            "outline",
            "draft",
            "review",
            "approved",
            "ready-for-dev",
            "in-game",
        ],
        "ready-for-dev",
        "in-game",
    ),
    (
        "development",
        &[
            "backlog",
            "sent-back",
            "ready",
            "in-progress",
            "in-review",
            "verify",
            "done",
        ],
        "verify",
        "done",
    ),
    (
        "art",
        &[
            "waiting",
            "sent-back",
            "ready",
            "in-progress",
            "pending-review",
            "approved",
            "in-engine",
        ],
        "approved",
        "in-engine",
    ),
    (
        "level-design",
        &[
            "layout",
            "sent-back",
            "blockout",
            "playtest",
            "scripting",
            "lighting",
            "art-pass",
            "final",
        ],
        "art-pass",
        "final",
    ),
    (
        "production",
        &["to-do", "sent-back", "in-progress", "review", "done"],
        "review",
        "done",
    ),
    (
        "sound",
        &[
            "backlog",
            "sent-back",
            "design",
            "recording",
            "implementation",
            "review",
            "in-game",
        ],
        "review",
        "in-game",
    ),
];

// ------------------------------------------------------------- helpers

/// `spec args` in the main worktree: an empty pipe on stdin, the pair's
/// scratch `HOME`.
fn run(pair: &Pair, args: &[&str]) -> common::Run {
    pair.spec_piped(&pair.main, args, b"")
}

/// Exit 0: stdout.
fn ok(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(0);
    done.stdout
}

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = run(pair, &all);
    done.code(0);
    done.json()
}

/// Exit 1, stdout empty: stderr.
fn refused(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(1);
    assert_eq!(done.stdout, "", "{args:?}: {}", done.show());
    done.stderr
}

/// Exit 2, stdout empty: stderr.
fn cannot(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(2);
    assert_eq!(done.stdout, "", "{args:?}: {}", done.show());
    done.stderr
}

/// The card `card new --title <title> <flags>` makes: its ID.
fn make(pair: &Pair, title: &str, flags: &[&str]) -> String {
    let mut args = vec!["card", "new", "--title", title];
    args.extend_from_slice(flags);
    doc(pair, &args)["id"]
        .as_str()
        .expect("a card's ID")
        .to_owned()
}

/// `show --history --json` of `id`.
fn show(pair: &Pair, id: &str) -> Value {
    doc(pair, &["card", "show", id, "--history"])
}

/// What a refusal must leave: the cards' rows raw, every row of the queue.
fn state(pair: &Pair) -> (String, String) {
    (
        pair.sql("SELECT * FROM cards ORDER BY id;"),
        pair.queue().dump().expect("dump"),
    )
}

/// `(type, payload)` of every `card.*` event, in `seq` order.
fn events(pair: &Pair) -> Vec<(String, Value)> {
    pair.events()
        .into_iter()
        .filter(|event| event.event_type.starts_with("card."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// `(type, payload)` of every `card.*` event whose `id` is `id`.
fn events_of(pair: &Pair, id: &str) -> Vec<(String, Value)> {
    events(pair)
        .into_iter()
        .filter(|(_, payload)| payload["id"] == id)
        .collect()
}

/// The last `card.*` event.
fn last_event(pair: &Pair) -> (String, Value) {
    events(pair).pop().expect("a card event")
}

/// `(column, revision, updated_at)` of `id`, and its own events: everything
/// another card's write must leave as it was.
fn untouched(pair: &Pair, id: &str) -> (String, String, String, Vec<(String, Value)>) {
    let card = doc(pair, &["card", "show", id]);
    (
        card["column"].as_str().expect("a column").to_owned(),
        card["revision"].as_str().expect("a revision").to_owned(),
        card["updated_at"].as_str().expect("updated_at").to_owned(),
        events_of(pair, id),
    )
}

/// The IDs of `card list --json <args>`.
fn listed(pair: &Pair, args: &[&str]) -> Vec<String> {
    let mut all = vec!["card", "list"];
    all.extend_from_slice(args);
    doc(pair, &all)["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// The refusal line of a move into the return column with no note, as
/// docs/canon/card-flows.md "Return" words it, on stderr.
fn needs_note(id: &str) -> String {
    format!(
        "spec: {id} enters `{BACK}`, its flow's return column: say why with `--note`; nothing \
         changed\n"
    )
}

/// `spec card accept id --as name --briefed` on a terminal, answering `y`.
fn accept(pair: &Pair, id: &str, name: &str) -> common::decision::PtyRun {
    pair.spec_pty(
        &pair.main,
        &["card", "accept", id, "--as", name, "--briefed"],
        Some("y"),
    )
}

/// studio-lamp's empty board: one line per flow, every column at 0.
fn lamp_board() -> String {
    LAMP_FLOWS
        .iter()
        .map(|(name, columns, _, _)| {
            let counts: Vec<String> = columns.iter().map(|column| format!("{column} 0")).collect();
            format!("flow {name} | {}\n", counts.join(" | "))
        })
        .collect()
}

/// [`lamp_board`] with `count` in `flow`'s `column` for each of `counts`.
fn lamp_board_with(counts: &[(&str, &str, usize)]) -> String {
    LAMP_FLOWS
        .iter()
        .map(|(name, columns, _, _)| {
            let cells: Vec<String> = columns
                .iter()
                .map(|column| {
                    let count = counts
                        .iter()
                        .find(|(flow, at, _)| flow == name && at == column)
                        .map_or(0, |(_, _, count)| *count);
                    format!("{column} {count}")
                })
                .collect();
            format!("flow {name} | {}\n", cells.join(" | "))
        })
        .collect()
}

/// The parent and its child of BH-12, as that criterion runs them:
/// `C-0001` a narrative card made `--as Ann`, `C-0002` the hand-off into
/// development, `Ann` its reviewer, `Rob` its assignee.
fn handed_off(pair: &Pair) -> (String, String) {
    let parent = make(pair, "T", &["--discipline", "narrative", "--as", "Ann"]);
    let child = make(
        pair,
        "T",
        &[
            "--parent",
            &parent,
            "--discipline",
            "development",
            "--reviewer",
            "Ann",
            "--assignee",
            "Rob",
            "--as",
            "Ann",
        ],
    );
    (parent, child)
}

// --------------------------------------------------------------- BH-12

/// BH-12: on studio-lamp a narrative card, then its hand-off into
/// development with `--reviewer Ann --as Ann`: the line of a new card, the
/// child stored with reviewer `Ann` and its parent, `card.created` holding
/// `"reviewer":"Ann"`; a card made without the flag holds `null`. M14.
#[test]
fn bh12_new_names_the_reviewer_and_card_created_carries_it() {
    let pair = Pair::new("ho-bh12", LAMP);
    assert_eq!(
        ok(
            &pair,
            &[
                "card",
                "new",
                "--title",
                "T",
                "--discipline",
                "narrative",
                "--as",
                "Ann"
            ]
        ),
        "created C-0001: narrative/backlog\n"
    );
    assert_eq!(
        last_event(&pair),
        (
            "card.created".to_owned(),
            json!({"id": "C-0001", "flow": "narrative", "column": "backlog", "position": 1,
                   "assignee": null, "reviewer": null, "mandate": null, "as": "Ann"})
        ),
        "no --reviewer: `reviewer` null"
    );
    assert_eq!(
        ok(
            &pair,
            &[
                "card",
                "new",
                "--title",
                "T",
                "--parent",
                "C-0001",
                "--discipline",
                "development",
                "--reviewer",
                "Ann",
                "--as",
                "Ann",
            ]
        ),
        "created C-0002: development/backlog\n",
        "the write's line is unchanged"
    );
    assert_eq!(
        last_event(&pair),
        (
            "card.created".to_owned(),
            json!({"id": "C-0002", "flow": "development", "column": "backlog", "position": 1,
                   "assignee": null, "reviewer": "Ann", "mandate": null, "as": "Ann"})
        )
    );
    assert_eq!(events(&pair).len(), 2, "one event per write");
    let child = show(&pair, "C-0002");
    assert_eq!(child["reviewer"], json!("Ann"), "{child}");
    assert_eq!(child["parent"], json!("C-0001"), "{child}");
    assert_eq!(child["assignee"], Value::Null, "{child}");
    assert_eq!(child["created_by"], json!("Ann"), "{child}");
    assert_eq!(
        pair.sql("SELECT reviewer, parent FROM cards WHERE id = 'C-0002';"),
        "Ann|C-0001\n",
        "the row holds it"
    );
    assert_eq!(
        pair.sql("SELECT reviewer IS NULL FROM cards WHERE id = 'C-0001';"),
        "1\n"
    );
    // The list row and the write document, as before: the reviewer is a row
    // key already, the write document has none.
    let rows = doc(&pair, &["card", "list"]);
    assert_eq!(rows["cards"][1]["id"], json!("C-0002"));
    assert_eq!(rows["cards"][1]["reviewer"], json!("Ann"));
    assert_eq!(rows["cards"][1]["parent"], json!("C-0001"));
    let written = run(
        &pair,
        &[
            "--json",
            "card",
            "new",
            "--title",
            "U",
            "--discipline",
            "art",
            "--reviewer",
            "Ann",
        ],
    );
    written.code(0);
    assert_eq!(
        task_common::keys(&written.stdout),
        [
            "id", "flow", "column", "position", "revision", "allowed", "notes"
        ],
        "{}",
        written.stdout
    );
    assert_eq!(
        last_event(&pair).1["reviewer"],
        json!("Ann"),
        "`--as` absent: `local` acts, the reviewer is still stored"
    );
    assert_eq!(last_event(&pair).1["as"], json!("local"));
}

/// BH-12: `--reviewer` on `new` takes `assign`'s checks and texts: `agent`
/// and `local` (trimmed or not) exit 1 with ``reviewer: `agent` can be no
/// reviewer: a person accepts a result``, a name off the person grammar with
/// `assign`'s line, each writing nothing; the same flags on `assign` give the
/// same stderr.
#[test]
fn bh12_a_reviewer_that_is_no_person_exits_1_with_assigns_text() {
    let pair = Pair::new("ho-bh12-refused", LAMP);
    let parent = make(&pair, "T", &["--discipline", "narrative", "--as", "Ann"]);
    for (name, line) in [
        (
            "agent",
            "spec: reviewer: `agent` can be no reviewer: a person accepts a result\n",
        ),
        (
            "local",
            "spec: reviewer: `local` can be no reviewer: a person accepts a result\n",
        ),
        (
            " local ",
            "spec: reviewer: `local` can be no reviewer: a person accepts a result\n",
        ),
    ] {
        let before = state(&pair);
        let stderr = refused(
            &pair,
            &[
                "card",
                "new",
                "--title",
                "T",
                "--parent",
                &parent,
                "--discipline",
                "development",
                "--reviewer",
                name,
                "--as",
                "Ann",
            ],
        );
        assert_eq!(stderr, line, "{name:?}");
        assert_eq!(state(&pair), before, "{name:?}: nothing written");
        assert_eq!(
            refused(&pair, &["card", "assign", &parent, "--reviewer", name]),
            line,
            "{name:?}: assign's own text"
        );
        assert_eq!(state(&pair), before, "{name:?}");
    }
    let long = "n".repeat(129);
    for name in ["  ", "a\u{2028}b", "a\u{202e}b", "a\nb", long.as_str()] {
        let before = state(&pair);
        let from_new = refused(
            &pair,
            &[
                "card",
                "new",
                "--title",
                "T",
                "--discipline",
                "development",
                "--reviewer",
                name,
            ],
        );
        assert!(
            from_new.starts_with("spec: reviewer: "),
            "{name:?}: {from_new}"
        );
        assert_eq!(state(&pair), before, "{name:?}: nothing written");
        assert_eq!(
            refused(&pair, &["card", "assign", &parent, "--reviewer", name]),
            from_new,
            "{name:?}: assign's text"
        );
    }
    // docs/canon/card-flows.md "Hand-off": `-`, which clears on `assign`, is
    // refused on `new` as `--assignee -` is: a new card has no name to clear.
    let before = state(&pair);
    assert_eq!(
        refused(
            &pair,
            &[
                "card",
                "new",
                "--title",
                "T",
                "--discipline",
                "development",
                "--reviewer",
                "-",
            ],
        ),
        "spec: reviewer: `-` clears a name: a new card has none\n"
    );
    assert_eq!(state(&pair), before, "`-`: nothing written");
    assert_eq!(events(&pair).len(), 1, "the parent's event alone");
}

/// BH-12: the child moved to `verify` is refused to its assignee `Rob` and
/// accepted by its reviewer `Ann`, who also made its parent: the hand-off's
/// asking side accepts its result (ADR-0074).
#[test]
fn bh12_the_child_is_accepted_by_its_reviewer_and_refused_to_its_assignee() {
    let pair = Pair::new("ho-bh12-accept", LAMP);
    let (_, child) = handed_off(&pair);
    assert_eq!(
        ok(
            &pair,
            &["card", "move", &child, "--to", "verify", "--as", "Rob"]
        ),
        format!("moved {child}: development/verify\n")
    );
    let before = state(&pair);
    let tried = accept(&pair, &child, "Rob");
    assert_eq!(tried.code, 1, "{}", tried.output);
    assert!(
        tried.output.contains(&format!(
            "{child}: `Rob` is its assignee, who never accepts a result; nothing changed"
        )),
        "{}",
        tried.output
    );
    assert_eq!(state(&pair), before, "the assignee changes nothing");
    let other = accept(&pair, &child, "Bea");
    assert_eq!(other.code, 1, "{}", other.output);
    assert_eq!(state(&pair), before, "nor another name");
    let accepted = accept(&pair, &child, "Ann");
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert!(accepted.asked, "{}", accepted.output);
    assert!(
        accepted
            .output
            .contains(&format!("accepted {child}: development/done\n")),
        "{}",
        accepted.output
    );
    let card = show(&pair, &child);
    assert_eq!(card["acceptance"]["by"], json!("Ann"), "{card}");
    assert_eq!(card["column"], json!("done"));
}

// --------------------------------------------------------------- BH-13

/// BH-13: `spec card list --json` serves each flow `{name, disciplines,
/// columns, moves, accept, return}`, in that order: on studio-lamp
/// `"return":"sent-back"` in all five, `sent-back` titled "Returned", second;
/// on card-press `"return":null` with every other byte as before. Since
/// docs/features/departments.md (D3, DP-1) `title` follows `name`: the
/// preset's titles on studio-lamp, "Copy desk" and `plates` on card-press.
#[test]
fn bh13_flows_serve_return_after_accept() {
    let pair = Pair::new("ho-bh13-lamp", LAMP);
    let listed = run(&pair, &["--json", "card", "list"]);
    listed.code(0);
    let document = listed.json();
    let ordered = Ordered::of(&listed.stdout);
    let flows = document["flows"].as_array().expect("flows");
    assert_eq!(flows.len(), 6, "department-split S9: `sound` sixth");
    for (index, (flow, (name, columns, from, to))) in flows.iter().zip(LAMP_FLOWS).enumerate() {
        assert_eq!(
            ordered.get("flows").at(index).keys(),
            [
                "name",
                "title",
                "disciplines",
                "columns",
                "moves",
                "accept",
                "return"
            ],
            "{name}"
        );
        assert_eq!(flow["name"], json!(name));
        assert_eq!(flow["return"], json!(BACK), "{name}");
        let ids: Vec<&str> = flow["columns"]
            .as_array()
            .expect("columns")
            .iter()
            .map(|column| column["id"].as_str().expect("an id"))
            .collect();
        assert_eq!(ids, columns, "{name}");
        assert_eq!(
            flow["columns"][1],
            json!({"id": BACK, "title": "Returned"}),
            "{name}"
        );
        assert_eq!(flow["accept"], json!({"from": from, "to": to}), "{name}");
        // The bytes: `return` right after `accept`, closing the flow.
        assert!(
            listed.stdout.contains(&format!(
                "\"accept\":{{\"from\":\"{from}\",\"to\":\"{to}\"}},\"return\":\"{BACK}\"}}"
            )),
            "{name}: {}",
            listed.stdout
        );
        // No `moves`: every ordered pair of the open columns, the return
        // column's both ways included.
        let open = columns.len() - 1;
        assert_eq!(
            flow["moves"].as_array().map(Vec::len),
            Some(open * (open - 1)),
            "{name}"
        );
    }
    // The text: one line per flow, the return column among the others.
    assert_eq!(ok(&pair, &["card", "list"]), lamp_board());

    let press = Pair::new("ho-bh13-press", PRESS);
    let listed = run(&press, &["--json", "card", "list"]);
    listed.code(0);
    assert_eq!(listed.stdout, PRESS_LIST_JSON, "card-press: `return` null");
    assert_eq!(listed.stderr, "");
    assert_eq!(
        ok(&press, &["card", "list"]),
        "flow copy | draft 0 | edit 0 | proof 0 | filed 0\n\
         flow plates | queued 0 | blocked 0 | checked 0 | struck 0\n",
        "card-press's text is as it was"
    );
}

/// studio-lamp's config with `development`'s `return` line replaced by
/// `line`: the text and the 1-based line it stands on.
fn lamp_with_return(config: &str, line: &str) -> (String, usize) {
    let at = config
        .lines()
        .enumerate()
        .filter(|(_, text)| *text == "return = \"sent-back\"")
        .nth(1)
        .map(|(index, _)| index + 1)
        .expect("development's return line, the second");
    let lines: Vec<&str> = config.lines().collect();
    assert_eq!(
        lines[at - 2],
        "accept = { from = \"verify\", to = \"done\" }",
        "the line after development's accept"
    );
    let mut changed: Vec<&str> = lines.clone();
    changed[at - 1] = line;
    let mut text = changed.join("\n");
    text.push('\n');
    (text, at)
}

/// BH-13: each of the five errors of docs/canon/card-flows.md "Config" exits
/// 2 `<config>:<line>: <message>` at the `return` line, for `spec card` alone: `spec tree` and
/// `spec check` print what they printed, `spec workspace` its text with the
/// card's error as its note, and the queue is unchanged. M15.
#[test]
fn bh13_each_return_error_exits_2_at_its_line_for_spec_card_alone() {
    let pair = Pair::new("ho-bh13-errors", LAMP);
    let id = make(&pair, "T", &["--discipline", "development"]);
    let config = read_text(&pair.main, "specengine.toml");
    let reads: [&[&str]; 2] = [&["tree"], &["check"]];
    let before: Vec<(i32, String, String)> = reads
        .iter()
        .map(|args| {
            let done = run(&pair, args);
            (done.code, done.stdout, done.stderr)
        })
        .collect();
    let workspaces = run(&pair, &["workspace"]);
    workspaces.code(0);
    let expected: Value =
        serde_json::from_str(&read_text(&fixture(LAMP), "expected.json")).expect("JSON");
    assert_eq!(json!(workspaces.stdout), expected["workspaces"]);
    let queue = state(&pair);
    for (line, message) in [
        ("return = 7", "`return` is a string, not an integer"),
        ("return = false", "`return` is a string, not a boolean"),
        (
            "return = [\"sent-back\"]",
            "`return` is a string, not an array",
        ),
        (
            "return = \"shelved\"",
            "`return`: `shelved` is no column of this flow",
        ),
        (
            "return = \"outline\"",
            "`return`: `outline` is no column of this flow",
        ),
        (
            "return = \"backlog\"",
            "`return`: `backlog` is the first column, where a new card starts",
        ),
        (
            "return = \"verify\"",
            "`return`: `verify` is `accept.from`, where a card waits for its accept",
        ),
        (
            "return = \"done\"",
            "`return`: `done` is `accept.to`, reached only by an accept",
        ),
    ] {
        let (text, at) = lamp_with_return(&config, line);
        write(&pair.main, "specengine.toml", &text);
        let want = format!("specengine.toml:{at}: {message}\n");
        for args in [
            vec!["card", "list"],
            vec!["--json", "card", "list"],
            vec!["card", "new", "--title", "Two"],
            vec!["card", "show", id.as_str()],
            vec!["card", "move", id.as_str(), "--to", "ready"],
            vec!["card", "move", id.as_str(), "--to", BACK, "--note", REASON],
        ] {
            assert_eq!(cannot(&pair, &args), want, "{line}: {args:?}");
        }
        for (args, (code, stdout, stderr)) in reads.iter().zip(&before) {
            let done = run(&pair, args);
            assert_eq!(done.code, *code, "{line}: {args:?}: {}", done.show());
            assert_eq!(done.stdout, *stdout, "{line}: {args:?}");
            assert_eq!(done.stderr, *stderr, "{line}: {args:?}");
        }
        let served = run(&pair, &["workspace"]);
        served.code(0);
        assert_eq!(served.stdout, workspaces.stdout, "{line}: served unjudged");
        assert_eq!(
            served.stderr,
            format!(
                "note: specengine.toml:{at}: {message}; the workspaces' flows are served \
                 unjudged\n"
            ),
            "{line}"
        );
        assert_eq!(state(&pair), queue, "{line}: nothing changed");
    }
    // The sound copy back: the card reads again.
    write(&pair.main, "specengine.toml", &config);
    assert_eq!(listed(&pair, &[]), [id]);
}

// --------------------------------------------------------------- BH-14

/// BH-14: a development card taken to `in-progress`, then sent back: without
/// `--note` exit 1 with the line of docs/canon/card-flows.md "Return", stdout
/// empty, no event,
/// nothing changed (under `--json` the write document, the line last of
/// `notes`); with the note it moves and `card.moved` holds the note; the
/// card's history keeps it. M16, M17.
#[test]
fn bh14_a_return_needs_a_note_and_card_moved_keeps_it() {
    let pair = Pair::new("ho-bh14", LAMP);
    let id = make(
        &pair,
        "Lamp room dialogue",
        &["--discipline", "development"],
    );
    assert_eq!(
        ok(
            &pair,
            &["card", "move", &id, "--to", "in-progress", "--as", "Rob"]
        ),
        format!("moved {id}: development/in-progress\n"),
        "taking it needs no note (M16)"
    );
    assert_eq!(
        last_event(&pair),
        (
            "card.moved".to_owned(),
            json!({"id": id, "from": "backlog", "to": "in-progress", "position": 1,
                   "as": "Rob", "note": null})
        ),
        "no note: `note` null"
    );
    let before = state(&pair);
    let count = events(&pair).len();
    assert_eq!(
        refused(&pair, &["card", "move", &id, "--to", BACK, "--as", "Rob"]),
        needs_note(&id),
        "M17"
    );
    assert_eq!(state(&pair), before, "nothing changed");
    assert_eq!(events(&pair).len(), count, "no event");
    // Under `--json`: the write document of a refusal, the line last.
    let json_run = run(&pair, &["--json", "card", "move", &id, "--to", BACK]);
    json_run.code(1);
    let document = json_run.json();
    assert_eq!(document["column"], json!("in-progress"), "{document}");
    assert_eq!(
        document["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!(
            needs_note(&id)
                .trim_start_matches("spec: ")
                .trim_end_matches('\n')
        )),
        "{document}"
    );
    assert_eq!(state(&pair), before, "nothing changed");
    // A `--revision` that is current does not stand in for the note.
    let revision = doc(&pair, &["card", "show", &id])["revision"]
        .as_str()
        .expect("a revision")
        .to_owned();
    assert_eq!(
        refused(
            &pair,
            &["card", "move", &id, "--to", BACK, "--revision", &revision]
        ),
        needs_note(&id)
    );
    assert_eq!(state(&pair), before);

    assert_eq!(
        ok(
            &pair,
            &[
                "card", "move", &id, "--to", BACK, "--note", REASON, "--as", "Rob"
            ]
        ),
        format!("moved {id}: development/{BACK}\n"),
        "the write's line is unchanged"
    );
    assert_eq!(events(&pair).len(), count + 1, "one event");
    assert_eq!(
        last_event(&pair),
        (
            "card.moved".to_owned(),
            json!({"id": id, "from": "in-progress", "to": BACK, "position": 1, "as": "Rob",
                   "note": REASON})
        )
    );
    let card = show(&pair, &id);
    assert_eq!(card["column"], json!(BACK));
    let history = card["history"].as_array().expect("--history");
    let last = history.last().expect("an entry");
    assert_eq!(last["type"], json!("card.moved"));
    assert_eq!(last["payload"]["note"], json!(REASON), "{last}");
    assert_eq!(last["payload"]["to"], json!(BACK), "{last}");
    // 3a adds no card column: the reason lives in the event alone (W9).
    let row = pair.sql("SELECT * FROM cards ORDER BY id;");
    assert!(!row.contains(REASON), "{row}");
}

/// BH-14: from the first column straight into the return column is a move
/// "from another column" too: refused without a note; a card made in
/// `backlog` sent back with one moves.
#[test]
fn bh14_the_first_column_into_the_return_column_needs_a_note() {
    let pair = Pair::new("ho-bh14-first", LAMP);
    let id = make(&pair, "Shore map", &["--discipline", "art"]);
    let before = state(&pair);
    assert_eq!(
        refused(&pair, &["card", "move", &id, "--to", BACK]),
        needs_note(&id)
    );
    assert_eq!(state(&pair), before);
    ok(
        &pair,
        &["card", "move", &id, "--to", BACK, "--note", REASON],
    );
    assert_eq!(
        last_event(&pair).1,
        json!({"id": id, "from": "waiting", "to": BACK, "position": 1, "as": "local",
               "note": REASON})
    );
}

/// BH-14: a note on any other move is recorded; none is `null`; a reorder
/// inside `sent-back` and a move out of it need none. M16.
#[test]
fn bh14_a_note_goes_with_any_move_and_only_entering_needs_one() {
    let pair = Pair::new("ho-bh14-any", LAMP);
    let first = make(&pair, "First", &["--discipline", "development"]);
    let second = make(&pair, "Second", &["--discipline", "development"]);
    ok(
        &pair,
        &[
            "card",
            "move",
            &first,
            "--to",
            "ready",
            "--note",
            "Scoped with art",
        ],
    );
    assert_eq!(
        last_event(&pair).1,
        json!({"id": first, "from": "backlog", "to": "ready", "position": 1, "as": "local",
               "note": "Scoped with art"}),
        "a note on an ordinary move is kept"
    );
    ok(
        &pair,
        &["card", "move", &first, "--to", BACK, "--note", REASON],
    );
    ok(
        &pair,
        &["card", "move", &second, "--to", BACK, "--note", "Too long"],
    );
    // A reorder inside the return column, `--to` its own column or none.
    assert_eq!(
        ok(
            &pair,
            &["card", "move", &second, "--to", BACK, "--before", &first]
        ),
        format!("moved {second}: development/{BACK}\n")
    );
    assert_eq!(
        last_event(&pair).1,
        json!({"id": second, "from": BACK, "to": BACK, "position": 1, "as": "local",
               "note": null}),
        "a reorder needs no note"
    );
    ok(&pair, &["card", "move", &first, "--before", &second]);
    assert_eq!(
        last_event(&pair).1,
        json!({"id": first, "from": BACK, "to": BACK, "position": 1, "as": "local",
               "note": null})
    );
    // Out of it: no note asked.
    ok(&pair, &["card", "move", &first, "--to", "in-progress"]);
    assert_eq!(
        last_event(&pair).1,
        json!({"id": first, "from": BACK, "to": "in-progress", "position": 1, "as": "local",
               "note": null}),
        "leaving the return column needs no note"
    );
    // A note with a move out of it is kept as well.
    ok(
        &pair,
        &["card", "move", &second, "--to", "ready", "--note", "Fixed"],
    );
    assert_eq!(last_event(&pair).1["note"], json!("Fixed"));
}

/// BH-14: `--note` takes `cancel --reason`'s grammar: blank (after trimming)
/// exits 1 `note: blank; say why it moves`, 4 097 bytes `note: 4097 bytes;
/// at most 4096`, on a return and on any move, nothing changed; 4 096 bytes
/// and a second line are a note.
#[test]
fn bh14_a_blank_or_over_long_note_exits_1_in_cancels_grammar() {
    let pair = Pair::new("ho-bh14-grammar", LAMP);
    let id = make(&pair, "T", &["--discipline", "development"]);
    let over = "n".repeat(4097);
    let wide = format!("{}\u{0436}", "n".repeat(4095));
    assert_eq!(wide.len(), 4097, "4 096 characters, 4 097 bytes");
    let before = state(&pair);
    for to in [BACK, "ready"] {
        for (note, line) in [
            ("", "spec: note: blank; say why it moves\n"),
            ("   ", "spec: note: blank; say why it moves\n"),
            ("\n\t", "spec: note: blank; say why it moves\n"),
            (over.as_str(), "spec: note: 4097 bytes; at most 4096\n"),
            (wide.as_str(), "spec: note: 4097 bytes; at most 4096\n"),
        ] {
            assert_eq!(
                refused(&pair, &["card", "move", &id, "--to", to, "--note", note]),
                line,
                "{to}: {} bytes",
                note.len()
            );
            assert_eq!(state(&pair), before, "{to}: nothing changed");
        }
    }
    let most = "n".repeat(4096);
    ok(&pair, &["card", "move", &id, "--to", BACK, "--note", &most]);
    assert_eq!(last_event(&pair).1["note"], json!(most), "4 096 bytes kept");
    ok(
        &pair,
        &[
            "card",
            "move",
            &id,
            "--to",
            "ready",
            "--note",
            "Two lines:\nboth kept",
        ],
    );
    assert_eq!(
        last_event(&pair).1["note"],
        json!("Two lines:\nboth kept"),
        "a reason takes lines, as `cancel --reason` does"
    );
}

/// BH-14, R3 on another project: card-press names no return column, so a
/// move into `blocked` (a word, not a meaning) needs no note and records
/// `null`; a note given is kept.
#[test]
fn bh14_a_flow_with_no_return_column_asks_no_note() {
    let pair = Pair::new("ho-bh14-press", PRESS);
    let id = make(&pair, "Back page", &["--discipline", "plates"]);
    ok(&pair, &["card", "move", &id, "--to", "blocked"]);
    assert_eq!(
        last_event(&pair).1,
        json!({"id": id, "from": "queued", "to": "blocked", "position": 1, "as": "local",
               "note": null})
    );
    ok(
        &pair,
        &["card", "move", &id, "--to", "checked", "--note", "Inks dry"],
    );
    assert_eq!(last_event(&pair).1["note"], json!("Inks dry"));
}

// --------------------------------------------------------------- BH-17

/// BH-17: a parent with one open child and one returned child still moves
/// and is accepted; a child then moves and is accepted under the accepted
/// parent; creating, moving, returning or accepting a child adds no event of
/// its parent and leaves its `revision` (R4, ADR-0074). M20.
#[test]
fn bh17_nothing_waits_on_another_card() {
    let pair = Pair::new("ho-bh17", LAMP);
    let parent = make(
        &pair,
        "Lamp room scene",
        &[
            "--discipline",
            "narrative",
            "--assignee",
            "Wren",
            "--reviewer",
            "Ann",
            "--as",
            "Ann",
        ],
    );
    let quiet = untouched(&pair, &parent);
    assert_eq!(quiet.3.len(), 1, "the parent's own `card.created`");
    let hand_off = |title: &str| {
        make(
            &pair,
            title,
            &[
                "--parent",
                &parent,
                "--discipline",
                "development",
                "--reviewer",
                "Ann",
                "--assignee",
                "Rob",
                "--as",
                "Ann",
            ],
        )
    };
    let open = hand_off("Lamp room dialogue");
    let back = hand_off("Lamp light shader");
    assert_eq!(
        untouched(&pair, &parent),
        quiet,
        "creating a child leaves its parent"
    );
    ok(
        &pair,
        &["card", "move", &back, "--to", "in-progress", "--as", "Rob"],
    );
    ok(
        &pair,
        &[
            "card", "move", &back, "--to", BACK, "--note", REASON, "--as", "Rob",
        ],
    );
    assert_eq!(
        untouched(&pair, &parent),
        quiet,
        "taking and returning a child leave its parent"
    );

    // The parent moves and is accepted, a child open and one returned.
    let children = (untouched(&pair, &open), untouched(&pair, &back));
    assert_eq!(
        ok(
            &pair,
            &[
                "card",
                "move",
                &parent,
                "--to",
                "ready-for-dev",
                "--as",
                "Wren"
            ]
        ),
        format!("moved {parent}: narrative/ready-for-dev\n"),
        "an open child stops no move"
    );
    let accepted = accept(&pair, &parent, "Ann");
    assert_eq!(accepted.code, 0, "M20: {}", accepted.output);
    assert!(
        accepted
            .output
            .contains(&format!("accepted {parent}: narrative/in-game\n")),
        "{}",
        accepted.output
    );
    assert_eq!(
        (untouched(&pair, &open), untouched(&pair, &back)),
        children,
        "the parent's move and accept leave its children"
    );

    // A child moves and is accepted under the accepted parent.
    let done = untouched(&pair, &parent);
    assert_eq!(done.0, "in-game");
    ok(
        &pair,
        &["card", "move", &open, "--to", "verify", "--as", "Rob"],
    );
    let child = accept(&pair, &open, "Ann");
    assert_eq!(child.code, 0, "{}", child.output);
    assert!(
        child
            .output
            .contains(&format!("accepted {open}: development/done\n")),
        "{}",
        child.output
    );
    // The returned one moves on from the return column.
    ok(
        &pair,
        &["card", "move", &back, "--to", "verify", "--as", "Rob"],
    );
    assert_eq!(
        untouched(&pair, &parent),
        done,
        "a child's move and accept leave its accepted parent"
    );
    // A closed parent refuses a write of its own and nothing of its
    // children's.
    let refusal = refused(&pair, &["card", "move", &parent, "--to", "review"]);
    assert!(refusal.contains("is accepted"), "{refusal}");
    let rows = doc(&pair, &["card", "list", "--parent", &parent]);
    let columns: Vec<(&str, &str)> = rows["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("an ID"),
                row["column"].as_str().expect("a column"),
            )
        })
        .collect();
    assert_eq!(
        columns,
        [(back.as_str(), "verify"), (open.as_str(), "done")],
        "each child where its own writes put it"
    );
}

/// BH-17: cancelling a parent or a child touches only that card; an edit and
/// an assign of the child leave the parent too.
#[test]
fn bh17_cancel_edit_and_assign_touch_one_card() {
    let pair = Pair::new("ho-bh17-cancel", LAMP);
    let (parent, child) = handed_off(&pair);
    let quiet = untouched(&pair, &parent);
    ok(&pair, &["card", "edit", &child, "--goal", "Ten lines."]);
    ok(&pair, &["card", "assign", &child, "--assignee", "Sol"]);
    ok(
        &pair,
        &["card", "cancel", &child, "--reason", "Cut from the build"],
    );
    assert_eq!(untouched(&pair, &parent), quiet, "the child's writes");
    let (parent2, child2) = handed_off(&pair);
    let kept = untouched(&pair, &child2);
    ok(
        &pair,
        &["card", "cancel", &parent2, "--reason", "Scene cut"],
    );
    assert_eq!(
        untouched(&pair, &child2),
        kept,
        "a cancelled parent cancels no child"
    );
}

// --------------------------------------------------------------- BH-19

/// BH-19: `spec card list --parent C-0001` lists exactly its children, in
/// the list's order, with every flow line (the counts the listed cards'); a
/// grandchild, another card's child and the parent itself stay out; filters
/// AND; `--with-cancelled` brings a cancelled child. M22.
#[test]
fn bh19_list_parent_lists_exactly_its_children() {
    let pair = Pair::new("ho-bh19", LAMP);
    let parent = make(&pair, "Lamp room scene", &["--discipline", "narrative"]);
    assert_eq!(parent, "C-0001");
    let dev = make(
        &pair,
        "Lamp room dialogue",
        &["--parent", "C-0001", "--discipline", "development"],
    );
    let art = make(
        &pair,
        "Lamp glow",
        &[
            "--parent",
            "C-0001",
            "--discipline",
            "art",
            "--assignee",
            "Sol",
        ],
    );
    let other = make(&pair, "Cove scene", &["--discipline", "narrative"]);
    let grandchild = make(
        &pair,
        "Dialogue lines",
        &["--parent", &dev, "--discipline", "development"],
    );
    let elsewhere = make(
        &pair,
        "Cove dialogue",
        &["--parent", &other, "--discipline", "development"],
    );
    let cancelled = make(
        &pair,
        "Lamp sound",
        &["--parent", "C-0001", "--discipline", "production"],
    );
    ok(&pair, &["card", "cancel", &cancelled, "--reason", "Cut"]);
    assert_eq!(
        [&dev, &art, &other, &grandchild, &elsewhere, &cancelled],
        ["C-0002", "C-0003", "C-0004", "C-0005", "C-0006", "C-0007"]
    );

    assert_eq!(
        listed(&pair, &["--parent", "C-0001"]),
        ["C-0002", "C-0003"],
        "exactly its children, by flow"
    );
    assert_eq!(
        ok(&pair, &["card", "list", "--parent", "C-0001"]),
        format!(
            "{}C-0002 | development/backlog | 1 | Lamp room dialogue | development | - | - | -\n\
             C-0003 | art/waiting | 1 | Lamp glow | art | Sol | - | -\n",
            lamp_board_with(&[("development", "backlog", 1), ("art", "waiting", 1)])
        )
    );
    let document = doc(&pair, &["card", "list", "--parent", "C-0001"]);
    // department-split S9: the preset's six flows, `sound` the sixth.
    assert_eq!(
        document["flows"].as_array().map(Vec::len),
        Some(LAMP_FLOWS.len())
    );
    assert!(
        document["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .all(|row| row["parent"] == "C-0001"),
        "{document}"
    );
    assert_eq!(listed(&pair, &["--parent", &dev]), [grandchild.as_str()]);
    assert_eq!(listed(&pair, &["--parent", &other]), [elsewhere.as_str()]);
    assert_eq!(
        listed(&pair, &["--parent", "C-0001", "--with-cancelled"]),
        ["C-0002", "C-0003", "C-0007"]
    );
    assert_eq!(
        listed(&pair, &["--parent", "C-0001", "--flow", "art"]),
        ["C-0003"],
        "filters AND"
    );
    assert_eq!(
        listed(&pair, &["--parent", "C-0001", "--assignee", "Sol"]),
        ["C-0003"]
    );
    assert_eq!(
        listed(&pair, &["--parent", "C-0001", "--discipline", "narrative"]),
        Vec::<String>::new()
    );
    // Unfiltered, all seven but the cancelled one: the flag narrows, M22.
    assert_eq!(listed(&pair, &[]).len(), 6);
}

/// BH-19: a card with no child, and no such card, list none, exit 0, with
/// every flow line and no note; a `C` off the card-ID grammar exits 2, a
/// look-alike exits 2 naming its Latin form, as every `C` argument does;
/// nothing is written.
#[test]
fn bh19_no_child_lists_none_and_a_bad_id_exits_2() {
    let pair = Pair::new("ho-bh19-none", LAMP);
    let lonely = make(&pair, "Cove scene", &["--discipline", "narrative"]);
    for id in [lonely.as_str(), "C-9999"] {
        let done = run(&pair, &["card", "list", "--parent", id]);
        done.code(0);
        assert_eq!(done.stdout, lamp_board(), "{id}: every flow line");
        assert_eq!(done.stderr, "", "{id}");
        let document = doc(&pair, &["card", "list", "--parent", id]);
        assert_eq!(document["cards"], json!([]), "{id}");
        assert_eq!(document["notes"], json!([]), "{id}");
        assert_eq!(
            document["flows"].as_array().map(Vec::len),
            Some(LAMP_FLOWS.len()),
            "{id}"
        );
    }
    let before = state(&pair);
    let look_alike = "\u{0421}-0001";
    assert_eq!(
        cannot(&pair, &["card", "list", "--parent", look_alike]),
        format!(
            "spec: `{look_alike}` uses look-alike letters or digits; card IDs are Latin only: \
             write `C-0001`\n"
        )
    );
    for bad in ["C-1", "c-0001", "C-00x1", "T-0001", ""] {
        let stderr = cannot(&pair, &["card", "list", "--parent", bad]);
        assert!(stderr.starts_with("spec: "), "{bad:?}: {stderr}");
        assert!(!stderr.contains('\n') || stderr.ends_with('\n'), "{stderr}");
        let json_run = run(&pair, &["--json", "card", "list", "--parent", bad]);
        json_run.code(2);
        assert_eq!(json_run.stdout, "", "{bad:?}: exit 2 prints no document");
    }
    assert_eq!(state(&pair), before, "nothing written");
}
