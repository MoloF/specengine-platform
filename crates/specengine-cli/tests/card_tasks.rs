//! docs/features/card-tasks.md (RM-0092, v4 step 3b, ADR-0075), the CLI
//! half: `spec card edit --task` and `spec card show` through the binary on
//! scratch git repositories of `fixtures/studio-lamp` (a lighthouse game
//! holding the gamedev preset's flows), its `T-` tasks made, planned,
//! approved, claimed, reported and completed through the library at the
//! injected clock (`task_common`), the owner's consent yes.
//!
//! - CT-1, link: `--task T...` replaces the card's whole list in the order
//!   given, `'[]'` at `new`, a lone `-` clears it, as one `card.edited
//!   {id, fields: ["tasks"], as}` and `revision` one higher, even when the
//!   list is unchanged; `--task` with another field is one event, `fields`
//!   in table order. Refused, exit 1, the reason the last note and nothing
//!   changed (`SqliteQueue::dump()` byte-equal): no such task, a repeat, no
//!   task ID (a `-` beside others too), the 33rd. Exit 2: a look-alike
//!   naming its Latin form; `T` claimed by `[ids]`, only when `--task` is
//!   given. A re-estimate takes no `--task`, as it takes no other field.
//!   M1: `fields()` omits `tasks`. M2: the existence check removed.
//! - CT-2, read: `show --json` serves `tasks` after `evidence`, each `{id,
//!   status, title, stale}` in link order, read now as `spec task list`
//!   reads it, with its notes after the card's own; a plan shows `review`
//!   with the card's `revision` unchanged; an approved task whose target is
//!   edited on disk is `stale: true`. A corrupt row, another repository's
//!   task, a deleted row and a root outside git each give `null`s and the
//!   table's note, `show` exit 0. The text lines are "Data"'s. An unlinked
//!   card at a root outside git reads as before. `list --json` rows keep
//!   their 21 keys. M3: the `same_repository` test dropped. M4: a corrupt
//!   task row propagated.
//! - CT-6, it decides nothing: twin projects, one linking `T-0001`: the
//!   task's approve and claim, every move to `accept.from`, the report and
//!   the completion and the card's accept answer byte-equal across the
//!   twins; the report and the completion leave the linked card as it was
//!   and add no `card.*` event. R7: an accept keeps the links, a closed card
//!   takes no link edit, a correction starts with none. M11: a move refused
//!   while a linked task is not `done`. M12: completing a task moves its
//!   cards.
//!
//! - CT-4, its newer-schema case: a queue at `user_version` 11 makes `show`
//!   and `edit` exit 2 naming 11 and 10, nothing changed.
//!
//! CT-3 (the gate) is `gate.rs`; the rest of the store and backup (CT-4) are
//! `specengine-store`'s `queue_cards.rs`, `queue_state.rs`, `queue_gate.rs`
//! and this crate's `queue_state.rs`; the tools `specengine-mcp`'s
//! `mcp_cards.rs`, `mcp_hub.rs`; the daemon and the hub (CT-7)
//! `specengine-http`'s `cards.rs`, `daemon_keys.rs`, `hub.rs`, `hub_mcp.rs`.
//! Every expected value below is worked out from the spec, never read back
//! from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::fs;
use std::path::Path;

use common::proposal::Pair;
use common::{read_text, replace, with_ids_line, write};
use serde_json::{Value, json};
use task_common::{CLOCK, Ordered, Tasks, document, line};

/// The game whose cards link the tasks.
const LAMP: &str = "studio-lamp";

/// The titles of "Data"'s example.
const NORTH: &str = "Light the north lamp";
const SWITCH: &str = "Wire the switch";

/// `spec task list`'s note of a task with no snapshot (docs/canon/tasks.md
/// "Commands", `task-package.md`).
const NO_SNAPSHOT: &str =
    "stale unknown: no snapshot (the owner's `spec task approve` freezes one)";

/// `show --json`'s keys in order: docs/canon/cards.md
/// "Documents and events" with `tasks` after `evidence` (card-tasks CT-2) and
/// `items` after `tasks` (docs/features/card-questions.md D4), `context`
/// after `history` (docs/features/card-context.md CC2).
const CARD_KEYS: [&str; 36] = [
    "id",
    "flow",
    "column",
    "position",
    "title",
    "goal",
    "discipline",
    "targets",
    "mark",
    "on",
    "discrepancy",
    "correction",
    // docs/features/upstream-signal.md US-8: after `correction`.
    "upstream",
    "snapshot",
    "criteria",
    "assignee",
    "reviewer",
    // docs/features/task-mandate.md M1.
    "mandate",
    "estimate",
    "due",
    "parent",
    "corrects",
    "evidence",
    "tasks",
    "items",
    // docs/features/task-mandate.md M6.
    "subtask_items",
    "acceptance",
    "cancellation",
    "created_by",
    "created_at",
    "updated_at",
    "revision",
    "allowed",
    "history",
    // docs/features/card-context.md CC2: `null` unless asked.
    "context",
    "notes",
];

/// A linked task's keys in order ("Documents", `CardTask`).
const TASK_KEYS: [&str; 4] = ["id", "status", "title", "stale"];

/// A write's or a refusal's document, sorted (unchanged, D9).
const WRITE_KEYS: [&str; 7] = [
    "allowed", "column", "flow", "id", "notes", "position", "revision",
];

/// A list row's 21 keys (D9: unchanged).
const ROW_KEYS: [&str; 23] = [
    "id",
    "flow",
    "column",
    "position",
    "title",
    "discipline",
    "targets",
    "assignee",
    "reviewer",
    // docs/features/task-mandate.md M1.
    "mandate",
    "estimate",
    "due",
    "parent",
    "corrects",
    "updated_at",
    "revision",
    "accepted",
    "cancelled",
    "mark",
    "on",
    "discrepancy",
    "correction",
    // docs/features/upstream-signal.md US-8: after `correction`.
    "upstream",
];

// ------------------------------------------------------------- helpers

/// `spec args` in `cwd`: an empty pipe on stdin, the pair's scratch `HOME`.
fn run_in(pair: &Pair, cwd: &Path, args: &[&str]) -> common::Run {
    pair.spec_piped(cwd, args, b"")
}

/// `spec args` in the main worktree.
fn run(pair: &Pair, args: &[&str]) -> common::Run {
    run_in(pair, &pair.main, args)
}

/// Exit 0: stdout.
fn ok(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(0);
    done.stdout
}

/// `spec --json args` in `cwd`, exit 0: the document and its raw text.
fn doc_in(pair: &Pair, cwd: &Path, args: &[&str]) -> (Value, String) {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = run_in(pair, cwd, &all);
    done.code(0);
    (done.json(), done.stdout)
}

/// `spec --json args` in the main worktree, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    doc_in(pair, &pair.main, args).0
}

/// Exit 1, stdout empty: stderr's last line.
fn refused(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(1);
    assert_eq!(done.stdout, "", "{args:?}: {}", done.show());
    done.stderr_lines()
        .last()
        .map(|line| (*line).to_owned())
        .unwrap_or_default()
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

/// A `draft` task on `nodes` titled `title` (none: no title), made in the
/// main worktree through the library: its ID.
fn task(pair: &Pair, title: Option<&str>, nodes: &[&str]) -> String {
    let outcome = pair
        .new_task(&pair.main, nodes, title, None)
        .unwrap_or_else(|error| panic!("task new {nodes:?}: {error}"));
    outcome.id.expect("a task's ID")
}

/// `spec card edit card --task T...`, exit 0: its line.
fn link(pair: &Pair, card: &str, tasks: &[&str]) -> String {
    let mut args = vec!["card", "edit", card];
    for id in tasks {
        args.extend(["--task", id]);
    }
    ok(pair, &args)
}

/// What a refusal must leave: the cards' rows raw, every row of the queue.
fn state(pair: &Pair) -> (String, String) {
    (
        pair.sql("SELECT * FROM cards ORDER BY id;"),
        pair.queue().dump().expect("dump"),
    )
}

/// `(type, payload)` of every `card.*` event, in `seq` order.
fn card_events(pair: &Pair) -> Vec<(String, Value)> {
    pair.events()
        .into_iter()
        .filter(|event| event.event_type.starts_with("card."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The `tasks` column of `card` as stored.
fn row_tasks(pair: &Pair, card: &str) -> String {
    pair.sql(&format!("SELECT tasks FROM cards WHERE id = '{card}';"))
        .trim_end_matches('\n')
        .to_owned()
}

/// One `card.edited` of `C-0001` naming `fields`, acting as `actor`.
fn edited(fields: &[&str], actor: &str) -> (String, Value) {
    (
        "card.edited".to_owned(),
        json!({"id": "C-0001", "fields": fields, "as": actor}),
    )
}

/// The `tasks:` lines of `show`'s text: every line after the last
/// `evidence:` line and before the first `items:` (before `acceptance:`
/// until card-questions K1).
fn tasks_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let evidence = lines
        .iter()
        .rposition(|line| line.starts_with("evidence: "))
        .unwrap_or_else(|| panic!("an evidence line: {text}"));
    // docs/features/card-questions.md K1: `items:` lines follow the tasks'.
    let items = lines
        .iter()
        .position(|line| line.starts_with("items: "))
        .unwrap_or_else(|| panic!("an items line: {text}"));
    lines[evidence + 1..items]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

/// A linked task's entry.
fn entry(id: &str, status: Option<&str>, title: Option<&str>, stale: Option<bool>) -> Value {
    json!({"id": id, "status": status, "title": title, "stale": stale})
}

/// A linked task this queue cannot read: `null` thrice.
fn unread(id: &str) -> Value {
    entry(id, None, None, None)
}

/// `show C --json` in `cwd`: the document, checked for its key order and
/// each entry's, and its text.
fn shown_in(pair: &Pair, cwd: &Path, card: &str) -> (Value, Vec<String>) {
    let (document, raw) = doc_in(pair, cwd, &["card", "show", card]);
    assert_eq!(task_common::keys(&raw), CARD_KEYS, "{raw}");
    let ordered = Ordered::of(&raw);
    let count = document["tasks"].as_array().expect("tasks").len();
    for index in 0..count {
        assert_eq!(
            ordered.get("tasks").at(index).keys(),
            TASK_KEYS,
            "{card}'s task {index}: {raw}"
        );
    }
    let text = run_in(pair, cwd, &["card", "show", card]);
    text.code(0);
    (document, tasks_lines(&text.stdout))
}

/// `show C` in the main worktree.
fn shown(pair: &Pair, card: &str) -> (Value, Vec<String>) {
    shown_in(pair, &pair.main, card)
}

/// The notes of `document` as strings.
fn notes(document: &Value) -> Vec<String> {
    document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

// ----------------------------------------------------------------- CT-1

/// CT-1, R1: `spec card edit C-0001 --task T-0001 --task T-0002` exits 0 with
/// the unchanged line; the row's `tasks` is `["T-0001","T-0002"]` (`'[]'` at
/// `new`), one new event, `card.edited` with `fields: ["tasks"]`, `revision`
/// one higher. The list is replaced in the order given; the same list again
/// is one more event and revision (D5); `--task` beside `--goal` is one event,
/// `fields` in table order; `--task -` stores `[]`. The write document keeps
/// its seven keys. M1: `fields()` omits `tasks`.
#[test]
fn ct1_edit_replaces_the_list_in_order_as_one_card_edited() {
    let pair = Pair::new("ct1-link", LAMP);
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    assert_eq!(task(&pair, Some(SWITCH), &["ISL-LAMP-ROOM"]), "T-0002");
    let card = make(
        &pair,
        "Light the lamp",
        &["--discipline", "development", "--target", "ISL-LAMP"],
    );
    assert_eq!(card, "C-0001");
    assert_eq!(row_tasks(&pair, "C-0001"), "[]", "'[]' at new");
    let revision = |pair: &Pair| doc(pair, &["card", "show", "C-0001"])["revision"].clone();
    assert_eq!(revision(&pair), json!("1"));

    let mut events = card_events(&pair);
    for (tasks, stored, rev) in [
        (&["T-0001", "T-0002"][..], "[\"T-0001\",\"T-0002\"]", "2"),
        (&["T-0002", "T-0001"][..], "[\"T-0002\",\"T-0001\"]", "3"),
        // The same list again: one event, one revision more (D5).
        (&["T-0002", "T-0001"][..], "[\"T-0002\",\"T-0001\"]", "4"),
    ] {
        assert_eq!(
            link(&pair, "C-0001", tasks),
            "edited C-0001: development/backlog\n",
            "{tasks:?}"
        );
        assert_eq!(row_tasks(&pair, "C-0001"), stored, "{tasks:?}");
        events.push(edited(&["tasks"], "local"));
        assert_eq!(
            card_events(&pair),
            events,
            "{tasks:?}: exactly one new event"
        );
        assert_eq!(revision(&pair), json!(rev), "{tasks:?}");
        let listed: Vec<Value> = doc(&pair, &["card", "show", "C-0001"])["tasks"]
            .as_array()
            .expect("tasks")
            .iter()
            .map(|entry| entry["id"].clone())
            .collect();
        assert_eq!(listed, tasks.iter().map(|id| json!(id)).collect::<Vec<_>>());
    }

    // With another field: one event, `fields` in table order, the write
    // document's seven keys.
    let mut args = vec!["--json", "card", "edit", "C-0001", "--task", "T-0001"];
    args.extend(["--goal", "Light it before the storm.", "--as", "Ann"]);
    let written = run(&pair, &args);
    written.code(0);
    let document = written.json();
    let mut keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
    keys.sort();
    assert_eq!(keys, WRITE_KEYS, "{document}");
    assert_eq!(document["revision"], json!("5"), "{document}");
    events.push(edited(&["goal", "tasks"], "Ann"));
    assert_eq!(card_events(&pair), events, "one event for both fields");
    assert_eq!(row_tasks(&pair, "C-0001"), "[\"T-0001\"]");

    // A lone `-` clears.
    assert_eq!(
        link(&pair, "C-0001", &["-"]),
        "edited C-0001: development/backlog\n"
    );
    assert_eq!(row_tasks(&pair, "C-0001"), "[]");
    events.push(edited(&["tasks"], "local"));
    assert_eq!(card_events(&pair), events);
    assert_eq!(revision(&pair), json!("6"));
    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(document["tasks"], json!([]));
    assert_eq!(lines, ["tasks: -"]);
}

/// CT-1, R2: each refusal exits 1 with its reason as the last line of stderr
/// and the last note of the `--json` document, nothing changed. M2: the
/// existence check removed (`T-0099` stored).
#[test]
fn ct1_a_refused_link_exits_1_and_changes_nothing() {
    let pair = Pair::new("ct1-refused", LAMP);
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    make(&pair, "Light the lamp", &["--discipline", "development"]);
    link(&pair, "C-0001", &["T-0001"]);
    let before = state(&pair);
    for (tasks, reason) in [
        (&["T-0099"][..], "task: no task T-0099; nothing changed"),
        (
            &["T-0001", "T-0099"][..],
            "task: no task T-0099; nothing changed",
        ),
        (
            &["T-0001", "T-0001"][..],
            "task: T-0001 is listed twice; nothing changed",
        ),
        (&["foo"][..], "task: \"foo\" is no task ID; nothing changed"),
        (
            &["T-0001", "-"][..],
            "task: \"-\" is no task ID; nothing changed",
        ),
        (
            &["-", "T-0001"][..],
            "task: \"-\" is no task ID; nothing changed",
        ),
    ] {
        let mut args = vec!["card", "edit", "C-0001"];
        for id in tasks {
            args.extend(["--task", id]);
        }
        assert_eq!(
            refused(&pair, &args),
            format!("spec: {reason}"),
            "{tasks:?}"
        );
        assert_eq!(state(&pair), before, "{tasks:?}: nothing changed");
        let mut json_args = vec!["--json"];
        json_args.extend_from_slice(&args);
        let answer = run(&pair, &json_args);
        answer.code(1);
        let document = answer.json();
        let mut keys: Vec<&String> = document.as_object().expect("an object").keys().collect();
        keys.sort();
        assert_eq!(keys, WRITE_KEYS, "{tasks:?}: {document}");
        assert_eq!(
            notes(&document).last().map(String::as_str),
            Some(reason),
            "{tasks:?}: {document}"
        );
        assert_eq!(document["revision"], json!("2"), "{tasks:?}");
        assert_eq!(state(&pair), before, "{tasks:?}: nothing changed");
    }

    // A look-alike: exit 2 naming its Latin form, as `written_task`.
    let message = cannot(
        &pair,
        &["card", "edit", "C-0001", "--task", "\u{0422}-0001"],
    );
    assert_eq!(
        message,
        "spec: `\u{0422}-0001` uses look-alike letters or digits; task IDs are Latin only: \
         write `T-0001`\n"
    );
    assert_eq!(state(&pair), before, "a look-alike changes nothing");
}

/// CT-1, D3: 33 entries exit 1 naming the cap, nothing changed; 32 are
/// stored, in order.
#[test]
fn ct1_thirty_two_links_are_stored_and_the_thirty_third_refused() {
    let pair = Pair::new("ct1-cap", LAMP);
    let ids: Vec<String> = (1..=33)
        .map(|_| task(&pair, Some(NORTH), &["ISL-LAMP"]))
        .collect();
    assert_eq!(ids.last().map(String::as_str), Some("T-0033"));
    make(&pair, "Light the lamp", &["--discipline", "development"]);
    let before = state(&pair);
    let all: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut args = vec!["card", "edit", "C-0001"];
    for id in &all {
        args.extend(["--task", id]);
    }
    assert_eq!(
        refused(&pair, &args),
        "spec: task: 33 tasks, at most 32; nothing changed"
    );
    assert_eq!(state(&pair), before, "nothing changed");
    link(&pair, "C-0001", &all[..32]);
    let stored: Vec<String> = serde_json::from_str(&row_tasks(&pair, "C-0001")).expect("JSON");
    assert_eq!(stored, ids[..32], "32 stored, in order");
    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(document["tasks"].as_array().map(Vec::len), Some(32));
    assert_eq!(lines.len(), 32, "{lines:?}");
}

/// CT-1, R2: `T` claimed by `[ids]`, as a prefix or through an alias, exits
/// 2 with `task_prefix_clash`'s line only when `--task` is given; an edit
/// without it, and `show`, still answer.
#[test]
fn ct1_t_in_ids_stops_a_link_alone() {
    for (label, line, needle) in [
        (
            "prefix",
            "T = { kind = \"tide\", width = 4 }\n",
            "configures the prefix `T`, which is SpecEngine's task prefix",
        ),
        (
            "alias",
            "TDE = { kind = \"tide\", width = 4, aliases_from = [\"\u{0422}\"] }\n",
            "which reads as SpecEngine's task prefix `T`",
        ),
    ] {
        let pair = Pair::new(&format!("ct1-ids-{label}"), LAMP);
        assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
        make(&pair, "Light the lamp", &["--discipline", "development"]);
        let config = read_text(&pair.main, "specengine.toml");
        write(&pair.main, "specengine.toml", with_ids_line(&config, line));
        let before = state(&pair);
        let message = cannot(&pair, &["card", "edit", "C-0001", "--task", "T-0001"]);
        assert!(message.contains(needle), "{label}: {message}");
        assert_eq!(state(&pair), before, "{label}: nothing changed");
        let message = cannot(&pair, &["card", "edit", "C-0001", "--task", "-"]);
        assert!(message.contains(needle), "{label}: a clear too: {message}");
        assert_eq!(
            ok(&pair, &["card", "edit", "C-0001", "--goal", "G."]),
            "edited C-0001: development/backlog\n",
            "{label}: without --task"
        );
        run(&pair, &["card", "show", "C-0001"]).code(0);
    }
}

/// CT-1: beside a re-estimate `--task` is refused as any other field is:
/// the same exit, stdout and stderr as `--goal` there, nothing changed.
#[test]
fn ct1_a_re_estimate_takes_no_task() {
    let pair = Pair::new("ct1-estimate", LAMP);
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    make(
        &pair,
        "Light the lamp",
        &["--discipline", "development", "--estimate", "3"],
    );
    let before = state(&pair);
    for json in [false, true] {
        let mut prefix: Vec<&str> = if json { vec!["--json"] } else { vec![] };
        prefix.extend([
            "card",
            "edit",
            "C-0001",
            "--estimate",
            "5",
            "--reason",
            "The lamp room is bigger.",
        ]);
        let mut with_goal = prefix.clone();
        with_goal.extend(["--goal", "G."]);
        let mut with_task = prefix.clone();
        with_task.extend(["--task", "T-0001"]);
        let goal = run(&pair, &with_goal);
        let task = run(&pair, &with_task);
        assert_ne!(goal.code, 0, "premise: {}", goal.show());
        assert_eq!(
            (task.code, &task.stdout, &task.stderr),
            (goal.code, &goal.stdout, &goal.stderr),
            "json={json}"
        );
        assert_eq!(state(&pair), before, "json={json}: nothing changed");
    }
}

// ----------------------------------------------------------------- CT-2

/// CT-2, R4, D6: `show` serves each linked task in link order, read now as
/// `spec task list` reads it (a `draft` with no snapshot: `stale` null and
/// the list's note after `<id>: `; no title: `-`); after `plan` the next read
/// serves `review` while the card's `revision` and `updated_at` stay; after
/// `approve` it is `ready`, `stale: false`; once a target's text changes on
/// disk, `stale: true` and the text line ends `| stale`. One task linked from
/// two cards shows on each. A read writes nothing and repeats byte for byte.
#[test]
fn ct2_show_serves_each_linked_task_read_now() {
    let pair = Pair::new("ct2-read", LAMP);
    let main = pair.main.clone();
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    assert_eq!(task(&pair, Some(SWITCH), &["ISL-LAMP-ROOM"]), "T-0002");
    assert_eq!(task(&pair, None, &["SCN-STORM"]), "T-0003");
    make(
        &pair,
        "Light the lamp",
        &["--discipline", "development", "--target", "ISL-LAMP"],
    );
    link(&pair, "C-0001", &["T-0001", "T-0002", "T-0003"]);
    make(&pair, "Storm scene", &["--discipline", "narrative"]);
    link(&pair, "C-0002", &["T-0001"]);
    let first = doc(&pair, &["card", "show", "C-0001"]);
    let (revision, updated_at) = (first["revision"].clone(), first["updated_at"].clone());
    assert_eq!(revision, json!("2"));

    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(
        document["tasks"],
        json!([
            entry("T-0001", Some("draft"), Some(NORTH), None),
            entry("T-0002", Some("draft"), Some(SWITCH), None),
            entry("T-0003", Some("draft"), None, None),
        ])
    );
    assert_eq!(
        notes(&document),
        [
            format!("T-0001: {NO_SNAPSHOT}"),
            format!("T-0002: {NO_SNAPSHOT}"),
            format!("T-0003: {NO_SNAPSHOT}"),
        ],
        "each task's note after the card's own, in link order"
    );
    assert_eq!(
        lines,
        [
            format!("tasks: T-0001 | draft | {NORTH}"),
            format!("tasks: T-0002 | draft | {SWITCH}"),
            "tasks: T-0003 | draft | -".to_owned(),
        ]
    );
    // As `spec task list` reads them.
    let list = pair.list_json(&main);
    for (index, id) in ["T-0001", "T-0002", "T-0003"].iter().enumerate() {
        let listed = &list["tasks"][index];
        assert_eq!(listed["id"], json!(id));
        assert_eq!(document["tasks"][index]["status"], listed["status"], "{id}");
        assert_eq!(document["tasks"][index]["title"], listed["title"], "{id}");
        assert_eq!(document["tasks"][index]["stale"], listed["stale"], "{id}");
        let note = format!("{id}: {NO_SNAPSHOT}");
        assert!(
            list["notes"]
                .as_array()
                .is_some_and(|all| all.contains(&json!(note))),
            "{list}"
        );
    }

    // The plan: `review`, the card untouched.
    let events = card_events(&pair);
    let dump = pair.queue().dump().expect("dump");
    let (again, _) = shown(&pair, "C-0001");
    assert_eq!(again, document, "one state, one result");
    assert_eq!(
        pair.queue().dump().expect("dump"),
        dump,
        "a read writes nothing"
    );
    pair.plan_ok(&main, "T-0001", &["The lamp lights."], &[]);
    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(
        document["tasks"][0],
        entry("T-0001", Some("review"), Some(NORTH), None)
    );
    assert_eq!(lines[0], format!("tasks: T-0001 | review | {NORTH}"));
    assert_eq!(document["revision"], revision, "the card's revision stays");
    assert_eq!(document["updated_at"], updated_at, "and its updated_at");
    assert_eq!(card_events(&pair), events, "no card event");

    // Approved: `ready`, its snapshot not stale.
    pair.approve_task(&main, "T-0001");
    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(
        document["tasks"][0],
        entry("T-0001", Some("ready"), Some(NORTH), Some(false))
    );
    assert_eq!(lines[0], format!("tasks: T-0001 | ready | {NORTH}"));
    assert_eq!(
        notes(&document),
        [
            format!("T-0002: {NO_SNAPSHOT}"),
            format!("T-0003: {NO_SNAPSHOT}"),
        ]
    );

    // A target's text edited on disk: stale.
    replace(
        &main,
        "docs/spec/isl-lamp.md",
        "Four mirrors ring the lamp",
        "Five mirrors ring the lamp",
    );
    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(
        document["tasks"][0],
        entry("T-0001", Some("ready"), Some(NORTH), Some(true))
    );
    assert_eq!(
        lines,
        [
            format!("tasks: T-0001 | ready | {NORTH} | stale"),
            format!("tasks: T-0002 | draft | {SWITCH}"),
            "tasks: T-0003 | draft | -".to_owned(),
        ]
    );
    let list = pair.list_json(&main);
    assert_eq!(
        list["tasks"][0]["stale"],
        json!(true),
        "as task list: {list}"
    );
    assert_eq!(document["revision"], revision);
    assert_eq!(card_events(&pair), events, "no card event");

    // One task linked from two cards shows on each.
    let (other, other_lines) = shown(&pair, "C-0002");
    assert_eq!(other["tasks"], json!([document["tasks"][0].clone()]));
    assert_eq!(
        other_lines,
        [format!("tasks: T-0001 | ready | {NORTH} | stale")]
    );
}

/// CT-2, D7: a corrupt row (`status` `bogus`), another repository's task
/// (`git_common_dir` another repository's), a deleted row: each `null`
/// thrice with the table's note after `<id>: `, the sound task served,
/// `show` exit 0 and the text lines `<id> | - | -`. `list --json` rows keep
/// their 21 keys. M3: the `same_repository` test dropped (`T-0003` served).
/// M4: a corrupt task row propagated (exit 2).
#[test]
fn ct2_a_task_this_queue_cannot_read_is_null_with_its_note() {
    let pair = Pair::new("ct2-null", LAMP);
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    assert_eq!(task(&pair, Some(SWITCH), &["ISL-LAMP-ROOM"]), "T-0002");
    assert_eq!(task(&pair, Some("Trim the wick"), &["ISL-LAMP"]), "T-0003");
    assert_eq!(
        task(&pair, Some("Polish the mirrors"), &["ISL-LAMP"]),
        "T-0004"
    );
    make(
        &pair,
        "Light the lamp",
        &["--discipline", "development", "--target", "ISL-LAMP"],
    );
    link(&pair, "C-0001", &["T-0001", "T-0002", "T-0003", "T-0004"]);

    // Another repository of the slug: a copy with its own `.git`.
    let other = pair.scratch.copy(LAMP, "other");
    pair.git.init(&other);
    pair.git.add_all(&other);
    pair.git.commit(&other, "the fixture");
    let common_dir = fs::canonicalize(other.join(".git")).expect("the other common dir");
    let common_dir = common_dir.to_str().expect("UTF-8").to_owned();
    pair.sql(&format!(
        "UPDATE tasks SET status = 'bogus' WHERE id = 'T-0002'; UPDATE tasks SET \
         git_common_dir = '{common_dir}' WHERE id = 'T-0003'; DELETE FROM tasks WHERE id = \
         'T-0004';"
    ));
    let dump = pair.queue().dump().expect("dump");

    let (document, lines) = shown(&pair, "C-0001");
    assert_eq!(
        document["tasks"],
        json!([
            entry("T-0001", Some("draft"), Some(NORTH), None),
            unread("T-0002"),
            unread("T-0003"),
            unread("T-0004"),
        ])
    );
    let all = notes(&document);
    assert_eq!(all.len(), 4, "{all:?}");
    assert_eq!(all[0], format!("T-0001: {NO_SNAPSHOT}"));
    assert!(
        all[1].starts_with("T-0002: its row does not read (`status`: ")
            && all[1].ends_with("); no state shown"),
        "{all:?}"
    );
    assert_eq!(
        all[2],
        "T-0003: a task of another repository of the project `studio-lamp`; its state is read \
         there (`spec task show T-0003`)"
    );
    assert_eq!(all[3], "T-0004: no such task in this queue; no state shown");
    assert_eq!(
        lines,
        [
            format!("tasks: T-0001 | draft | {NORTH}"),
            "tasks: T-0002 | - | -".to_owned(),
            "tasks: T-0003 | - | -".to_owned(),
            "tasks: T-0004 | - | -".to_owned(),
        ]
    );
    assert_eq!(
        pair.queue().dump().expect("dump"),
        dump,
        "a read writes nothing"
    );

    // `list` reads no task: its rows keep their 21 keys.
    let listed = run(&pair, &["--json", "card", "list"]);
    listed.code(0);
    let rows = Ordered::of(&listed.stdout);
    let count = listed.json()["cards"].as_array().map_or(0, Vec::len);
    assert_eq!(count, 1);
    assert_eq!(rows.get("cards").at(0).keys(), ROW_KEYS);
}

/// CT-2, D7, D8, R5: at a root outside git, a linked task reads `null` with
/// `no state shown: <git's line>`, the line `spec task list` exits 2 with
/// there; `show` exits 0. An unlinked card there reads as before: exit 0,
/// byte for byte its read in the repository, `tasks: -`.
#[test]
fn ct2_a_root_outside_git_reads_null_and_an_unlinked_card_as_before() {
    let pair = Pair::new("ct2-nogit", LAMP);
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    make(&pair, "Light the lamp", &["--discipline", "development"]);
    link(&pair, "C-0001", &["T-0001"]);
    make(
        &pair,
        "Storm scene",
        &["--discipline", "narrative", "--target", "SCN-STORM"],
    );
    let loose = pair.scratch.copy(LAMP, "loose");
    assert!(!loose.join(".git").exists(), "premise: no repository");

    let listed = run_in(&pair, &loose, &["task", "list"]);
    listed.code(2);
    let git_line = listed
        .stderr
        .strip_prefix("spec: ")
        .and_then(|rest| rest.strip_suffix('\n'))
        .filter(|rest| !rest.contains('\n'))
        .unwrap_or_else(|| panic!("one line: {}", listed.show()))
        .to_owned();

    let (document, lines) = shown_in(&pair, &loose, "C-0001");
    assert_eq!(document["tasks"], json!([unread("T-0001")]));
    assert_eq!(
        notes(&document),
        [format!("T-0001: no state shown: {git_line}")]
    );
    assert_eq!(lines, ["tasks: T-0001 | - | -"]);

    for json in [false, true] {
        let args: &[&str] = if json {
            &["--json", "card", "show", "C-0002"]
        } else {
            &["card", "show", "C-0002"]
        };
        let here = run(&pair, args);
        here.code(0);
        let there = run_in(&pair, &loose, args);
        there.code(0);
        assert_eq!(there.stdout, here.stdout, "json={json}");
        assert_eq!(there.stderr, here.stderr, "json={json}");
    }
    let (document, lines) = shown_in(&pair, &loose, "C-0002");
    assert_eq!(document["tasks"], json!([]));
    assert_eq!(notes(&document), Vec::<String>::new());
    assert_eq!(lines, ["tasks: -"]);
}

// ----------------------------------------------------------------- CT-6

/// One twin's answers: `(step, exit, stdout or line, stderr or document)`.
type Answers = Vec<(String, i32, String, String)>;

/// A task command's answer, its question (paths of the twin's scratch made
/// `<root>`) beside it.
fn task_answer(
    pair: &Pair,
    step: &str,
    outcome: &specengine_cli::TaskOutcome,
    questions: &[String],
) -> (String, i32, String, String) {
    let root = pair.scratch.path().to_str().expect("UTF-8").to_owned();
    (
        format!("{step} {}", questions.join(" | ").replace(&root, "<root>")),
        i32::from(outcome.exit().code()),
        line(outcome),
        document(outcome).to_string(),
    )
}

/// `(column, position, revision, updated_at)` of `C-0001` and its events:
/// what a task's command must leave.
fn card_as_stored(pair: &Pair) -> (String, Vec<(String, Value)>) {
    (
        pair.sql("SELECT col, position, revision, updated_at FROM cards WHERE id = 'C-0001';"),
        card_events(pair),
    )
}

/// CT-6, R3, ADR-0075: twin projects, identical but for `C-0001` linking
/// `T-0001` in one and none in the other (its list cleared, so both edits
/// leave one event and one revision). The task's plan, approve and claim,
/// each move of the card to `accept.from` while the task is `in_progress`,
/// the report and the completion, and the reviewer's accept give the same
/// exit, output and document on both. On the linked twin the report and the
/// completion leave the card's column, position, revision, `updated_at` and
/// events as they were. R7 after the accept: the link stays (`done`, served),
/// a link edit is the closed refusal, a correction starts with none. M11,
/// M12.
#[test]
fn ct6_the_link_decides_nothing_on_twin_projects() {
    let twins = [Pair::new("ct6-plain", LAMP), Pair::new("ct6-linked", LAMP)];
    let mut answers: [Answers; 2] = [Vec::new(), Vec::new()];
    for (index, pair) in twins.iter().enumerate() {
        let main = pair.main.clone();
        let out = &mut answers[index];
        assert_eq!(task(pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
        make(
            pair,
            "Light the lamp",
            &[
                "--discipline",
                "development",
                "--target",
                "ISL-LAMP",
                "--reviewer",
                "Ann",
                "--assignee",
                "Rob",
            ],
        );
        let linked = if index == 1 { "T-0001" } else { "-" };
        link(pair, "C-0001", &[linked]);
        assert_eq!(
            row_tasks(pair, "C-0001"),
            if index == 1 { "[\"T-0001\"]" } else { "[]" }
        );

        let planned = pair.plan_ok(&main, "T-0001", &["The lamp lights."], &[]);
        out.push(task_answer(pair, "plan", &planned, &[]));
        let (approved, questions) = pair.owner(&main, "approve", "T-0001", None, true, CLOCK);
        let approved = approved.unwrap_or_else(|error| panic!("approve: {error}"));
        out.push(task_answer(pair, "approve", &approved, &questions));
        let claimed = pair.claim_ok(&main, "T-0001", &main);
        out.push(task_answer(pair, "claim", &claimed, &[]));

        // Every move to `accept.from` while the task is `in_progress`.
        for column in ["ready", "in-progress", "in-review", "verify"] {
            let moved = run(pair, &["card", "move", "C-0001", "--to", column]);
            out.push((
                format!("move {column}"),
                moved.code,
                moved.stdout.clone(),
                moved.stderr.clone(),
            ));
        }
        let board = run(pair, &["card", "list"]);
        out.push(("list".to_owned(), board.code, board.stdout, board.stderr));

        // The report and the completion leave the card as it was.
        let card = card_as_stored(pair);
        let reported = pair
            .report(&main, "T-0001", "completed", "The lamp lights.", &[])
            .unwrap_or_else(|error| panic!("report: {error}"));
        out.push(task_answer(pair, "report", &reported, &[]));
        let completed = pair
            .complete(&main, "T-0001")
            .unwrap_or_else(|error| panic!("complete: {error}"));
        out.push(task_answer(pair, "complete", &completed, &[]));
        assert_eq!(
            document(&completed)["status"],
            json!("done"),
            "{completed:?}"
        );
        assert_eq!(
            card_as_stored(pair),
            card,
            "twin {index}: the card's column, position, revision, updated_at and events"
        );

        // The reviewer's accept.
        let accepted = pair.spec_pty(
            &main,
            &["card", "accept", "C-0001", "--briefed", "--as", "Ann"],
            Some("y"),
        );
        out.push((
            format!("accept asked={}", accepted.asked),
            accepted.code,
            accepted.output.clone(),
            String::new(),
        ));
        assert_eq!(accepted.code, 0, "twin {index}: {}", accepted.output);
    }
    assert_eq!(answers[1], answers[0], "the link decides nothing");

    // R7, on the linked twin: the accept left the link, `done` is served.
    let pair = &twins[1];
    assert_eq!(row_tasks(pair, "C-0001"), "[\"T-0001\"]");
    let (document, lines) = shown(pair, "C-0001");
    assert_eq!(document["column"], json!("done"));
    assert_eq!(
        document["tasks"],
        json!([entry("T-0001", Some("done"), Some(NORTH), Some(false))])
    );
    assert_eq!(lines, [format!("tasks: T-0001 | done | {NORTH}")]);
    let before = state(pair);
    assert_eq!(
        refused(pair, &["card", "edit", "C-0001", "--task", "-"]),
        "spec: C-0001 is accepted: it takes no write; a correction is a new card, `spec card new \
         --corrects C-0001`; nothing changed"
    );
    assert_eq!(state(pair), before, "a closed card takes no link edit");
    let correction = make(
        pair,
        "Light it brighter",
        &["--discipline", "development", "--corrects", "C-0001"],
    );
    assert_eq!(correction, "C-0002");
    assert_eq!(
        row_tasks(pair, "C-0002"),
        "[]",
        "a correction starts with none"
    );
}

// ----------------------------------------------------------------- CT-4

/// CT-4, the path an older build meets at 10: a queue at `user_version` 11
/// makes `spec card show` (and `edit --task`) exit 2 with the `SchemaTooNew`
/// line naming 11 and this build's 10, the queue's rows and version left
/// alone (the name kept, cited by docs/features/card-tasks.md: since
/// docs/features/card-questions.md the probe is 12 over this build's 11;
/// since docs/features/task-mandate.md, R1 recorded at 8a43e5d, 13 over
/// 12).
/// The gate's no verdict is `gate.rs`' AC-06.
#[test]
fn ct4_a_queue_of_schema_11_is_refused_naming_11_and_10() {
    let pair = Pair::new("ct4-newer", LAMP);
    assert_eq!(task(&pair, Some(NORTH), &["ISL-LAMP"]), "T-0001");
    make(&pair, "Light the lamp", &["--discipline", "development"]);
    link(&pair, "C-0001", &["T-0001"]);
    pair.sql("PRAGMA user_version = 13;");
    let rows = pair.sql("SELECT * FROM cards; SELECT * FROM tasks; SELECT * FROM events;");
    for args in [
        &["card", "show", "C-0001"][..],
        &["--json", "card", "show", "C-0001"][..],
        &["card", "edit", "C-0001", "--task", "-"][..],
    ] {
        let message = cannot(&pair, args);
        assert!(
            message.contains("schema version 13, this build knows 12"),
            "{args:?}: {message}"
        );
        assert_eq!(pair.sql("PRAGMA user_version;").trim(), "13", "{args:?}");
        assert_eq!(
            pair.sql("SELECT * FROM cards; SELECT * FROM tasks; SELECT * FROM events;"),
            rows,
            "{args:?}: nothing changed"
        );
    }
}
