//! docs/features/card-store.md, the CLI half: `spec card` through the
//! binary on scratch git repositories of `fixtures/card-press` (a print
//! shop: `copy` with `moves` and a `by`, `plates` with no `moves` and a
//! `blocked` column that means nothing), and of `fixtures/spec-a`, `-b`,
//! `-c` (no `[card]`: the default flow). The store half is the store's
//! `queue_cards.rs`, the tools `mcp_cards.rs`; the door over all of them
//! `mcp_door.rs`.
//!
//! - AC-01: seven writes on one card, `git status --porcelain --ignored`
//!   empty and `HEAD` unchanged after each, one event each.
//! - AC-02: a move outside `moves` names the allowed ones, a stale
//!   `--revision` names the column, nothing changes.
//! - AC-03: `by` compares `--as`, the move records it.
//! - AC-04: `accept` on a pipe exits 2 first; on a pseudo-terminal the
//!   assignee, a non-reviewer, `agent` exit 1, the reviewer's `y` moves it;
//!   an accepted card refuses every write.
//! - AC-05: the default flow, `PLAIN_FLOW` pasted, a bad `[card]`.
//! - AC-06: two `HOME`s replaying one history at one clock.
//! - AC-07: the backup, its older formats and refusals.
//! - AC-08: a malformed card row leaves the gate and the task reads as they
//!   were.
//! - AC-09: look-alikes, `C` in `[ids]`, no `.git`, `due` raw, evidence.
//! - AC-10: one code for five projects' flows, no fixture word in the card
//!   sources (the tool half: `mcp_cards.rs`).
//! - AC-11: `cancel`.
//! - docs/features/board-ui.md: a usage defect stays exit 2's one line under
//!   `--json`, stdout empty (`or_usage_error`); a card's persons take a wider
//!   grammar: any script and inner spaces, one line after trimming, at most
//!   128 bytes, no control character, U+2028, U+2029 or bidi mark, compared
//!   exactly after trimming; `agent` and `local` stay no reviewers.
//!
//! The oracle of "nothing changed" is `SELECT * FROM cards` and
//! `SqliteQueue::dump()`, both raw. Every expected value below is worked
//! out from the spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::proposal::Pair;
use common::{Run, Scratch, data_dir, fixture, read_text, repository_root, with_ids_line, write};
use serde_json::{Value, json};
use specengine_cli::{
    CardMoveRequest, CardNewRequest, Env, Globals, card_list, card_move, card_new,
};
use task_common::Ordered;

/// The fixture of the two trades.
const PRESS: &str = "card-press";

/// The clock of the replayed history (AC-06).
const CLOCK: &str = "2026-10-30T09:00:00Z";

/// The empty board of a project with no `[[card.flow]]`.
const PLAIN_BOARD: &str = "flow plain | backlog 0 | ready 0 | in-progress 0 | review 0 | done 0\n";

/// The empty board of `fixtures/card-press`.
const PRESS_BOARD: &str = "flow copy | draft 0 | edit 0 | proof 0 | filed 0\n\
                           flow plates | queued 0 | blocked 0 | checked 0 | struck 0\n";

/// `PLAIN_FLOW` as "Data" gives it, written here from the spec, never read
/// from core: AC-05 pastes it.
const PLAIN_TOML: &str = "\n[[card.flow]]\nname = \"plain\"\ncolumns = [\"backlog\", \"ready\", \
                          \"in-progress\", \"review\", \"done\"]\ntitles = { backlog = \
                          \"Backlog\", ready = \"Ready\", in-progress = \"In progress\", review \
                          = \"Review\", done = \"Done\" }\naccept = { from = \"review\", to = \
                          \"done\" }\n";

/// A write's or a refusal's document, in order.
const WRITE_KEYS: [&str; 7] = [
    "id", "flow", "column", "position", "revision", "allowed", "notes",
];

/// `show`'s document: the columns in order, less `project`, `col` as
/// `column`, `--marks`' four keys after `targets`, `tasks` after `evidence`
/// (docs/canon/cards.md "Documents and events": each linked task as read now),
/// `items` after `tasks` (docs/features/card-questions.md D4), then
/// `allowed`, `history`, `context`, `notes`.
const SHOW_KEYS: [&str; 36] = [
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

/// `list`'s document.
const LIST_KEYS: [&str; 5] = ["flows", "cards", "notes", "truncated", "omitted"];

/// A row of `list`'s document.
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
    // docs/features/trace-views.md "The board tile": `null` without `--marks`.
    "mark",
    "on",
    "discrepancy",
    "correction",
    // docs/features/upstream-signal.md US-8: after `correction`.
    "upstream",
];

// ------------------------------------------------------------- helpers

/// `spec args` in the main worktree: an empty pipe on stdin, the pair's
/// scratch `HOME`.
fn run(pair: &Pair, args: &[&str]) -> Run {
    pair.spec_piped(&pair.main, args, b"")
}

/// Exit 0: stdout.
fn ok(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(0);
    done.stdout
}

/// `spec --json args`, whatever its exit.
fn json_run(pair: &Pair, args: &[&str]) -> Run {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    run(pair, &all)
}

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let done = json_run(pair, args);
    done.code(0);
    done.json()
}

/// Exit 1, stdout empty, one `spec: ` line on stderr: that line.
fn refused(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(1);
    assert_eq!(done.stdout, "", "{args:?}: {}", done.show());
    assert!(done.stderr.starts_with("spec: "), "{}", done.show());
    done.stderr
}

/// Exit 2, stdout empty: stderr.
fn cannot(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(2);
    assert_eq!(done.stdout, "", "{args:?}: {}", done.show());
    done.stderr
}

/// The card `args` make: its ID.
fn make(pair: &Pair, title: &str, flags: &[&str]) -> String {
    let mut args = vec!["card", "new", "--title", title];
    args.extend_from_slice(flags);
    doc(pair, &args)["id"]
        .as_str()
        .expect("a card's ID")
        .to_owned()
}

/// `show --json` of `id`.
fn show(pair: &Pair, id: &str) -> Value {
    doc(pair, &["card", "show", id])
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

/// Everything that names git's side of a run: the status with ignored
/// files, `HEAD`, the number of commits.
fn git_side(pair: &Pair) -> (String, String, String) {
    (
        pair.git_text(&pair.main, &["status", "--porcelain", "--ignored"]),
        pair.rev(&pair.main, "HEAD"),
        pair.git_text(&pair.main, &["rev-list", "--all", "--count"]),
    )
}

/// The lines of `list` that are cards.
fn card_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| !line.starts_with("flow ") && !line.starts_with('['))
        .collect()
}

/// `id` of every row of `list --json` `args`.
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

/// `(position, revision)` of `id` as `show` gives them.
fn place(pair: &Pair, id: &str) -> (i64, String) {
    let card = show(pair, id);
    (
        card["position"].as_i64().expect("a position"),
        card["revision"].as_str().expect("a revision").to_owned(),
    )
}

/// Appends `extra` to the root's config.
fn append_config(root: &Path, extra: &str) {
    let mut text = read_text(root, "specengine.toml");
    text.push_str(extra);
    write(root, "specengine.toml", text);
}

/// The press card at `proof`: `ana` its assignee, `ben` its reviewer.
fn in_proof(pair: &Pair) -> String {
    let id = make(pair, "Lead story", &["--discipline", "copy"]);
    ok(
        pair,
        &[
            "card",
            "assign",
            &id,
            "--assignee",
            "ana",
            "--reviewer",
            "ben",
        ],
    );
    ok(pair, &["card", "move", &id, "--to", "edit", "--as", "desk"]);
    ok(pair, &["card", "move", &id, "--to", "proof"]);
    id
}

/// Every write `spec card` has, on `id` (`accept` is the terminal's, run
/// apart).
fn writes(id: &str) -> [Vec<&str>; 5] {
    [
        vec!["card", "move", id, "--to", "edit", "--as", "desk"],
        vec!["card", "move", id, "--before", id],
        vec!["card", "edit", id, "--goal", "Again."],
        vec!["card", "assign", id, "--assignee", "cid"],
        vec!["card", "cancel", id, "--reason", "No."],
    ]
}

// --------------------------------------------------------------- AC-01

/// AC-01: new, assign, edit, move, move, edit, accept on one `card-press`
/// card: each a printed line, exactly one event with "Documents and
/// events"'s payload, `git status --porcelain --ignored` empty, `HEAD`,
/// every ref and every file as they were; nothing in `proposals` or
/// `tasks`. M: a card move writing a file (`mcp_door.rs` the tools' half).
#[test]
fn ac01_seven_writes_leave_git_alone_and_write_one_event_each() {
    let pair = Pair::new("cd-ac01", PRESS);
    let before = pair.state();
    let side = git_side(&pair);
    assert_eq!(side.0, "", "the repository starts clean");
    let target = "SHT-FRONT";
    let steps: [(Vec<&str>, &str, &str, Value); 6] = [
        (
            vec![
                "card",
                "new",
                "--title",
                "Lead story",
                "--discipline",
                "copy",
                "--target",
                target,
                "--criterion",
                "Two sources.",
                "--as",
                "writer",
            ],
            "created C-0001: copy/draft\n",
            "card.created",
            json!({"id": "C-0001", "flow": "copy", "column": "draft", "position": 1,
                   "assignee": null, "reviewer": null, "mandate": null, "as": "writer"}),
        ),
        (
            vec![
                "card",
                "assign",
                "C-0001",
                "--assignee",
                "ana",
                "--reviewer",
                "ben",
                "--as",
                "desk",
            ],
            "assigned C-0001: ana, reviewer ben\n",
            "card.assigned",
            json!({"id": "C-0001", "assignee": "ana", "reviewer": "ben", "as": "desk"}),
        ),
        (
            vec!["card", "edit", "C-0001", "--goal", "Ten inches."],
            "edited C-0001: copy/draft\n",
            "card.edited",
            json!({"id": "C-0001", "fields": ["goal"], "as": "local"}),
        ),
        (
            vec!["card", "move", "C-0001", "--to", "edit", "--as", "desk"],
            "moved C-0001: copy/edit\n",
            "card.moved",
            json!({"id": "C-0001", "from": "draft", "to": "edit", "position": 1, "as": "desk",
                   "note": null}),
        ),
        (
            vec!["card", "move", "C-0001", "--to", "proof"],
            "moved C-0001: copy/proof\n",
            "card.moved",
            json!({"id": "C-0001", "from": "edit", "to": "proof", "position": 1, "as": "local",
                   "note": null}),
        ),
        (
            vec![
                "card",
                "edit",
                "C-0001",
                "--evidence",
                "url=https://press.example.invalid/proof/1",
                "--as",
                "ana",
            ],
            "edited C-0001: copy/proof\n",
            "card.edited",
            json!({"id": "C-0001", "fields": ["evidence"], "as": "ana"}),
        ),
    ];
    for (count, (args, line, kind, payload)) in steps.into_iter().enumerate() {
        assert_eq!(ok(&pair, &args), line, "{args:?}");
        let seen = events(&pair);
        assert_eq!(seen.len(), count + 1, "{args:?}: one event: {seen:?}");
        assert_eq!(
            seen.last(),
            Some(&(kind.to_owned(), payload)),
            "{args:?}: {seen:?}"
        );
        assert_eq!(git_side(&pair), side, "{args:?}: git's side");
        assert_eq!(pair.state(), before, "{args:?}: files, refs, status");
    }
    let accepted = pair.spec_pty(
        &pair.main,
        &["card", "accept", "C-0001", "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert!(accepted.asked, "{}", accepted.output);
    assert!(
        accepted.output.contains("accepted C-0001: copy/filed\n"),
        "{}",
        accepted.output
    );
    let seen = events(&pair);
    assert_eq!(seen.len(), 7, "{seen:?}");
    assert_eq!(
        seen.last(),
        Some(&(
            "card.accepted".to_owned(),
            json!({"id": "C-0001", "from": "proof", "to": "filed", "position": 1, "as": "ben",
                "verdict": "briefed", "discrepancy": null})
        ))
    );
    assert_eq!(git_side(&pair), side);
    assert_eq!(pair.state(), before);
    assert_eq!(pair.sql("SELECT count(*) FROM tasks;").trim(), "0");
    assert_eq!(pair.sql("SELECT count(*) FROM proposals;").trim(), "0");
}

/// "CLI": the write's document, keys in order; the row's fields as the
/// flags gave them; `snapshot` each direct target frozen
/// (docs/features/card-trace.md); `revision` raised by each change.
#[test]
fn a_writes_document_and_the_fields_it_stored() {
    let pair = Pair::new("cd-doc", PRESS);
    let created = json_run(
        &pair,
        &[
            "card",
            "new",
            "--title",
            "Lead story",
            "--goal",
            "Ten inches.",
            "--discipline",
            "copy",
            "--target",
            "SHT-FRONT",
            "--target",
            "docs/spec/sht-back.md",
            "--criterion",
            "Two sources.",
            "--assignee",
            "ana",
            "--estimate",
            "2-5",
            "--due",
            "2026-11-05",
        ],
    );
    created.code(0);
    assert_eq!(task_common::keys(&created.stdout), WRITE_KEYS);
    let document = created.json();
    assert_eq!(document["id"], "C-0001");
    assert_eq!(document["flow"], "copy");
    assert_eq!(document["column"], "draft");
    assert_eq!(document["position"], 1);
    assert_eq!(document["revision"], "1");
    assert_eq!(document["notes"], json!([]));

    let text = json_run(&pair, &["card", "show", "C-0001"]);
    text.code(0);
    assert_eq!(task_common::keys(&text.stdout), SHOW_KEYS);
    let card = text.json();
    assert_eq!(card["title"], "Lead story");
    assert_eq!(card["goal"], "Ten inches.");
    assert_eq!(card["discipline"], "copy");
    assert_eq!(
        card["targets"],
        json!(["SHT-FRONT", "SHT-BACK"]),
        "canonical: a path reads as its node"
    );
    assert_eq!(card["criteria"], json!(["Two sources."]));
    assert_eq!(card["assignee"], "ana");
    assert_eq!(card["estimate"], "2-5");
    assert_eq!(card["due"], "2026-11-05");
    let frozen = card["snapshot"].as_array().expect("a snapshot");
    assert_eq!(frozen.len(), 2, "each direct target: {card}");
    assert_eq!(frozen[0]["id"], "SHT-FRONT");
    assert_eq!(frozen[1]["id"], "SHT-BACK");
    for absent in [
        "reviewer",
        "parent",
        "corrects",
        "acceptance",
        "cancellation",
    ] {
        assert_eq!(card[absent], Value::Null, "{absent}");
    }
    assert_eq!(card["evidence"], json!([]));
    assert_eq!(card["created_by"], "local");
    assert_eq!(card["history"], Value::Null, "no --history");
    assert_eq!(card["revision"], "1");
    assert_eq!(card["created_at"], card["updated_at"]);

    let moved = doc(
        &pair,
        &["card", "move", "C-0001", "--to", "edit", "--as", "desk"],
    );
    assert_eq!(moved["revision"], "2");
    assert_eq!(moved["column"], "edit");
    assert_eq!(
        pair.sql("SELECT typeof(position), typeof(revision), col FROM cards;")
            .trim(),
        "integer|text|edit"
    );
}

/// "CLI": an ID naming nothing: exit 1 with `no card C-0099`, `{id,
/// reason}`, each command.
#[test]
fn no_such_card_is_refused_with_its_two_keys() {
    let pair = Pair::new("cd-none", PRESS);
    make(&pair, "One", &["--discipline", "plates"]);
    let before = state(&pair);
    for args in [
        vec!["card", "show", "C-0099"],
        vec!["card", "move", "C-0099", "--to", "blocked"],
        vec!["card", "edit", "C-0099", "--goal", "G."],
        vec!["card", "assign", "C-0099", "--assignee", "ana"],
        vec!["card", "cancel", "C-0099", "--reason", "No."],
    ] {
        let reason = refused(&pair, &args);
        assert!(reason.contains("no card C-0099"), "{args:?}: {reason}");
        let shown = json_run(&pair, &args);
        shown.code(1);
        assert_eq!(
            task_common::keys(&shown.stdout),
            ["id", "reason"],
            "{args:?}"
        );
        assert_eq!(
            shown.json(),
            json!({"id": "C-0099", "reason": "no card C-0099"}),
            "{args:?}"
        );
    }
    assert_eq!(state(&pair), before);
}

/// "CLI": nothing asked of `move`, `edit`, `assign` exits 2, nothing
/// changed.
#[test]
fn nothing_asked_exits_2() {
    let pair = Pair::new("cd-nothing", PRESS);
    make(&pair, "One", &["--discipline", "plates"]);
    let before = state(&pair);
    for args in [
        vec!["card", "move", "C-0001"],
        vec!["card", "edit", "C-0001"],
        vec!["card", "assign", "C-0001"],
        vec!["card", "edit", "C-0001", "--revision", "1"],
        vec!["card", "move", "C-0001", "--as", "desk"],
    ] {
        cannot(&pair, &args);
        assert_eq!(state(&pair), before, "{args:?}");
    }
    assert_eq!(events(&pair).len(), 1);
}

/// docs/features/board-ui.md "Files in scope": nothing asked is a `CliError`
/// on the terminal, `--json` too: exit 2, its one line on stderr, stdout
/// empty, no `{id, reason}` document, which only the daemon's route reads.
/// M: `or_usage_error` dropped from `main.rs`.
#[test]
fn nothing_asked_under_json_is_the_line_alone() {
    let pair = Pair::new("cd-nothing-json", PRESS);
    make(&pair, "One", &["--discipline", "plates"]);
    let before = state(&pair);
    for (args, line) in [
        (
            vec!["card", "move", "C-0001"],
            "spec: `spec card move` asks nothing: give --to K, --before C or both\n",
        ),
        (
            vec!["card", "move", "C-0001", "--as", "desk"],
            "spec: `spec card move` asks nothing: give --to K, --before C or both\n",
        ),
        (
            vec!["card", "edit", "C-0001"],
            "spec: `spec card edit` asks nothing: give a field to change or --evidence\n",
        ),
        (
            vec!["card", "edit", "C-0001", "--revision", "1"],
            "spec: `spec card edit` asks nothing: give a field to change or --evidence\n",
        ),
        (
            vec!["card", "assign", "C-0001"],
            "spec: `spec card assign` asks nothing: give --assignee N, --reviewer N or both\n",
        ),
    ] {
        let done = json_run(&pair, &args);
        done.code(2);
        assert_eq!(done.stdout, "", "{args:?}: no document: {}", done.show());
        assert_eq!(done.stderr, line, "{args:?}");
        // The text mode says the same.
        let text = run(&pair, &args);
        text.code(2);
        assert_eq!(text.stdout, "", "{args:?}: {}", text.show());
        assert_eq!(text.stderr, line, "{args:?}");
    }
    assert_eq!(state(&pair), before, "nothing changed");
}

// --------------------------------------------------------------- AC-02

/// AC-02: `draft` to `proof` is no move of `copy`: exit 1 with the spec's
/// line, the row and the events as they were, the document that of the
/// card as it stands. From `edit` and `proof` the line names what is left;
/// a column that does not exist is a move outside the flow too. M:
/// `card_move` dropping the check.
#[test]
fn ac02_a_move_outside_the_flow_names_the_allowed_moves() {
    let pair = Pair::new("cd-ac02", PRESS);
    make(&pair, "Lead story", &["--discipline", "copy"]);
    let before = state(&pair);
    assert_eq!(
        refused(
            &pair,
            &["card", "move", "C-0001", "--to", "proof", "--as", "desk"]
        ),
        "spec: C-0001 is in draft: no move to `proof`; allowed: `edit`; nothing changed\n"
    );
    assert_eq!(state(&pair), before, "row and events unchanged");
    let shown = json_run(
        &pair,
        &["card", "move", "C-0001", "--to", "proof", "--as", "desk"],
    );
    shown.code(1);
    assert_eq!(task_common::keys(&shown.stdout), WRITE_KEYS);
    let document = shown.json();
    assert_eq!(document["id"], "C-0001");
    assert_eq!(document["column"], "draft");
    assert_eq!(document["position"], 1);
    assert_eq!(document["revision"], "1");
    assert_eq!(
        document["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!(
            "C-0001 is in draft: no move to `proof`; allowed: `edit`; nothing changed"
        ))
    );
    assert_eq!(state(&pair), before);

    assert_eq!(
        refused(
            &pair,
            &["card", "move", "C-0001", "--to", "nowhere", "--as", "desk"]
        ),
        "spec: C-0001 is in draft: no move to `nowhere`; allowed: `edit`; nothing changed\n"
    );
    assert_eq!(state(&pair), before);

    ok(
        &pair,
        &["card", "move", "C-0001", "--to", "edit", "--as", "desk"],
    );
    let before = state(&pair);
    assert_eq!(
        refused(&pair, &["card", "move", "C-0001", "--to", "draft"]),
        "spec: C-0001 is in edit: no move to `draft`; allowed: `proof`; nothing changed\n"
    );
    assert_eq!(state(&pair), before);

    ok(&pair, &["card", "move", "C-0001", "--to", "proof"]);
    let before = state(&pair);
    assert_eq!(
        refused(&pair, &["card", "move", "C-0001", "--to", "edit"]),
        "spec: C-0001 is in proof: no move to `edit`; allowed: `spec card accept`; nothing \
         changed\n"
    );
    assert_eq!(state(&pair), before);
}

/// "Rules and edge cases", the first bullet's line word for word: a
/// default-shaped flow with a `moves` list leaving `review` only for
/// `in-progress`.
#[test]
fn the_spec_line_of_a_refused_move() {
    let pair = Pair::new("cd-line", "spec-a");
    append_config(
        &pair.main,
        "\n[[card.flow]]\nname = \"plain\"\ncolumns = [\"backlog\", \"ready\", \"in-progress\", \
         \"review\", \"done\"]\naccept = { from = \"review\", to = \"done\" }\nmoves = [\n  { \
         from = \"backlog\", to = \"ready\" },\n  { from = \"ready\", to = \"in-progress\" },\n  \
         { from = \"in-progress\", to = \"review\" },\n  { from = \"review\", to = \
         \"in-progress\" },\n]\n",
    );
    make(&pair, "One", &[]);
    for column in ["ready", "in-progress", "review"] {
        ok(&pair, &["card", "move", "C-0001", "--to", column]);
    }
    assert_eq!(
        refused(&pair, &["card", "move", "C-0001", "--to", "ready"]),
        "spec: C-0001 is in review: no move to `ready`; allowed: `in-progress`, `spec card \
         accept`; nothing changed\n"
    );
}

/// "Data": no `moves` is every ordered pair of the columns but `accept.to`:
/// on `plates` all six stand, `blocked` as any other, and `struck` is
/// reached by no move.
#[test]
fn no_moves_is_every_ordered_pair_of_the_other_columns() {
    let pair = Pair::new("cd-pairs", PRESS);
    make(&pair, "Plate", &["--discipline", "plates"]);
    for (from, to) in [
        ("queued", "blocked"),
        ("blocked", "checked"),
        ("checked", "queued"),
        ("queued", "checked"),
        ("checked", "blocked"),
        ("blocked", "queued"),
    ] {
        assert_eq!(show(&pair, "C-0001")["column"], from);
        assert_eq!(
            ok(&pair, &["card", "move", "C-0001", "--to", to]),
            format!("moved C-0001: plates/{to}\n")
        );
    }
    let before = state(&pair);
    assert_eq!(
        refused(&pair, &["card", "move", "C-0001", "--to", "struck"]),
        "spec: C-0001 is in queued: no move to `struck`; allowed: `blocked`, `checked`; nothing \
         changed\n"
    );
    assert_eq!(state(&pair), before);
}

/// AC-02: `--revision 1` after a move exits 1 with tasks' line, `in
/// <column>` for the state, on every command that takes it; the row as it
/// was; the current revision passes.
#[test]
fn ac02_a_stale_revision_loses_the_compare_and_set() {
    let pair = Pair::new("cd-cas", PRESS);
    make(&pair, "Lead story", &["--discipline", "copy"]);
    assert_eq!(
        doc(
            &pair,
            &["card", "move", "C-0001", "--to", "edit", "--as", "desk"]
        )["revision"],
        "2"
    );
    let at = show(&pair, "C-0001")["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let line = format!(
        "spec: `C-0001` changed since this run read it: it is in edit since {at}; nothing \
         changed\n"
    );
    let before = state(&pair);
    for args in [
        vec!["card", "move", "C-0001", "--to", "proof", "--revision", "1"],
        vec!["card", "edit", "C-0001", "--goal", "G.", "--revision", "1"],
        vec![
            "card",
            "assign",
            "C-0001",
            "--assignee",
            "ana",
            "--revision",
            "1",
        ],
        vec![
            "card",
            "cancel",
            "C-0001",
            "--reason",
            "No.",
            "--revision",
            "1",
        ],
    ] {
        assert_eq!(refused(&pair, &args), line, "{args:?}");
        assert_eq!(state(&pair), before, "{args:?}");
    }
    assert_eq!(
        doc(
            &pair,
            &["card", "move", "C-0001", "--to", "proof", "--revision", "2"]
        )["revision"],
        "3"
    );
}

// --------------------------------------------------------------- AC-03

/// AC-03: `draft` to `edit` carries `by = ["desk"]`: no `--as` (it is
/// `local`) and another name exit 1 naming `desk`, nothing changed;
/// `--as desk` moves it and the event records it. `edit` to `proof` has no
/// `by`: anyone, `local` recorded. M: `by` ignored.
#[test]
fn ac03_a_by_list_stops_a_slip_and_records_who_moved() {
    let pair = Pair::new("cd-ac03", PRESS);
    make(&pair, "Lead story", &["--discipline", "copy"]);
    let before = state(&pair);
    for args in [
        vec!["card", "move", "C-0001", "--to", "edit"],
        vec!["card", "move", "C-0001", "--to", "edit", "--as", "ana"],
        vec!["card", "move", "C-0001", "--to", "edit", "--as", "agent"],
    ] {
        let reason = refused(&pair, &args);
        assert!(reason.contains("`desk`"), "{args:?}: {reason}");
        assert_eq!(state(&pair), before, "{args:?}");
    }
    ok(
        &pair,
        &["card", "move", "C-0001", "--to", "edit", "--as", "desk"],
    );
    let seen = events(&pair);
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[1].1["as"], "desk");
    ok(&pair, &["card", "move", "C-0001", "--to", "proof"]);
    assert_eq!(events(&pair)[2].1["as"], "local");
}

/// "Who acts": `--as` takes a person's grammar (one line after trimming, at
/// most 128 bytes, no control character, U+2028, U+2029 or bidi mark), and a
/// value outside it stores nothing: exit 2 for `--as`, exit 1 for
/// `--assignee` and `--reviewer`.
#[test]
fn as_takes_the_person_grammar() {
    let pair = Pair::new("cd-as", PRESS);
    make(&pair, "One", &["--discipline", "plates"]);
    let before = state(&pair);
    let long = "n".repeat(129);
    let wide = "\u{0430}".repeat(65);
    // 128 characters, 129 bytes: the limit counts bytes.
    let mixed = format!("{}\u{0430}", "n".repeat(127));
    let off = [
        "",
        "   ",
        "two\nlines",
        "tab\tinside",
        "bell\u{7}",
        "a\u{85}b",
        "a\u{61c}b",
        "a\u{2028}b",
        "a\u{2029}b",
        "a\u{200e}b",
        "a\u{200f}b",
        "a\u{202a}b",
        "a\u{202e}b",
        "a\u{2066}b",
        "a\u{2069}b",
        long.as_str(),
        wide.as_str(),
        mixed.as_str(),
    ];
    for name in off {
        let done = run(
            &pair,
            &["card", "move", "C-0001", "--to", "blocked", "--as", name],
        );
        done.code(2);
        assert!(done.stderr.contains("--as"), "{name:?}: {}", done.show());
        assert_eq!(state(&pair), before, "{name:?}");
    }
    // The limit's own words, in bytes, the trimmed name's.
    assert_eq!(
        cannot(
            &pair,
            &["card", "move", "C-0001", "--to", "blocked", "--as", &mixed]
        ),
        "spec: --as: 129 bytes; at most 128\n"
    );
    assert_eq!(
        refused(&pair, &["card", "assign", "C-0001", "--reviewer", &long]),
        "spec: reviewer: 129 bytes; at most 128\n"
    );
    // `--assignee` and `--reviewer` are refused by the same grammar, exit 1.
    for name in off {
        for (args, field) in [
            (
                vec!["card", "assign", "C-0001", "--assignee", name],
                "assignee",
            ),
            (
                vec!["card", "assign", "C-0001", "--reviewer", name],
                "reviewer",
            ),
            (
                vec![
                    "card",
                    "new",
                    "--title",
                    "T",
                    "--discipline",
                    "copy",
                    "--assignee",
                    name,
                ],
                "assignee",
            ),
        ] {
            let reason = refused(&pair, &args);
            assert!(reason.contains(field), "{name:?}: {reason}");
            assert_eq!(state(&pair), before, "{name:?}");
        }
    }
}

/// A full name in another script.
const FULL_NAME: &str =
    "\u{0410}\u{043d}\u{043d}\u{0430} \u{041f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";

/// Another person's, who reviews.
const OTHER_NAME: &str =
    "\u{0411}\u{043e}\u{0440}\u{0438}\u{0441} \u{0418}\u{0432}\u{0430}\u{043d}\u{043e}\u{0432}";

/// "Who acts": a full name in any script as `--as`, `--assignee`,
/// `--reviewer` and in a move's `by`, compared exactly after trimming; on a
/// terminal the assignee is still refused and the reviewer accepts. `agent`
/// and `local` stay no reviewers, trimmed or not. At 128 bytes, in any
/// script, a name passes.
#[test]
fn a_full_name_in_any_script_is_a_person_compared_exactly_after_trimming() {
    let pair = Pair::new("cd-names", PRESS);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        config.replace(
            "by = [\"desk\"]",
            &format!("by = [\"desk\", \"{FULL_NAME}\"]"),
        ),
    );
    let id = make(
        &pair,
        "Lead story",
        &[
            "--discipline",
            "copy",
            "--assignee",
            FULL_NAME,
            "--as",
            FULL_NAME,
        ],
    );
    let spaced_other = format!("  {OTHER_NAME} ");
    ok(&pair, &["card", "assign", &id, "--reviewer", &spaced_other]);
    let card = show(&pair, &id);
    assert_eq!(card["created_by"], FULL_NAME, "{card}");
    assert_eq!(card["assignee"], FULL_NAME, "{card}");
    assert_eq!(card["reviewer"], OTHER_NAME, "trimmed: {card}");
    assert_eq!(
        card["allowed"],
        json!([{"to": "edit", "by": ["desk", FULL_NAME]}]),
        "{card}"
    );

    // Exactly: the first name alone, another case: not `by`'s.
    let first = "\u{0410}\u{043d}\u{043d}\u{0430}";
    let lower =
        "\u{0430}\u{043d}\u{043d}\u{0430} \u{043f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";
    let before = state(&pair);
    for near in [first, lower] {
        assert_eq!(
            refused(&pair, &["card", "move", &id, "--to", "edit", "--as", near]),
            format!(
                "spec: {id} is in draft: the move to `edit` is `desk` or `{FULL_NAME}`'s, not \
                 `{near}`'s; nothing changed\n"
            )
        );
        assert_eq!(state(&pair), before, "{near}");
    }
    // After trimming, the same name, recorded trimmed.
    let spaced = format!("  {FULL_NAME}  ");
    ok(
        &pair,
        &["card", "move", &id, "--to", "edit", "--as", &spaced],
    );
    let moved = events(&pair).pop().expect("the move");
    assert_eq!(moved.1["as"], FULL_NAME, "{moved:?}");
    ok(&pair, &["card", "move", &id, "--to", "proof"]);
    assert_eq!(
        listed(&pair, &["--assignee", FULL_NAME]),
        [id.as_str()],
        "an exact filter"
    );

    // On a terminal: `accept`'s rules, then the reviewer's yes.
    let before = state(&pair);
    for name in [FULL_NAME, first, "agent", "local", " agent "] {
        let tried = pair.spec_pty(
            &pair.main,
            &["card", "accept", &id, "--as", name, "--briefed"],
            Some("y"),
        );
        assert_eq!(tried.code, 1, "{name}: {}", tried.output);
        assert_eq!(state(&pair), before, "{name}");
    }
    for name in ["agent", "local", " local "] {
        let reason = refused(&pair, &["card", "assign", &id, "--reviewer", name]);
        assert!(reason.contains("no reviewer"), "{name:?}: {reason}");
        assert_eq!(state(&pair), before, "{name:?}");
    }
    let accepted = pair.spec_pty(
        &pair.main,
        &["card", "accept", &id, "--as", OTHER_NAME, "--briefed"],
        Some("y"),
    );
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert_eq!(show(&pair, &id)["acceptance"]["by"], OTHER_NAME);

    // At the limit, in bytes: one, two and three bytes a letter.
    let ascii = "n".repeat(128);
    let wide = "\u{0430}".repeat(64);
    let wider = format!("{}nn", "\u{4e2d}".repeat(42));
    for name in [ascii.as_str(), wide.as_str(), wider.as_str()] {
        assert_eq!(name.len(), 128, "{name}");
        let card = make(
            &pair,
            "At the limit",
            &["--discipline", "plates", "--as", name, "--assignee", name],
        );
        let shown = show(&pair, &card);
        assert_eq!(shown["created_by"], name);
        assert_eq!(shown["assignee"], name);
        ok(&pair, &["card", "assign", &card, "--reviewer", name]);
        assert_eq!(show(&pair, &card)["reviewer"], name);
    }
    // The limit is the trimmed name's: spaces around it cost nothing.
    let spaced = format!("  {ascii}  ");
    let card = make(
        &pair,
        "Spaced",
        &["--discipline", "plates", "--as", &spaced],
    );
    assert_eq!(show(&pair, &card)["created_by"], ascii.as_str());
}

/// "Config": `by` is read as a person is written: trimmed, so a config's
/// stray spaces change no comparison and no document, and the longest name
/// a person may have fits there too.
#[test]
fn a_by_is_trimmed_from_the_config_and_holds_the_longest_name() {
    let pair = Pair::new("cd-by-trimmed", PRESS);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        config.replace(
            "by = [\"desk\"]",
            &format!("by = [\"  desk \", \" {FULL_NAME}  \"]"),
        ),
    );
    let id = make(&pair, "Lead story", &["--discipline", "copy"]);
    let by = json!(["desk", FULL_NAME]);
    assert_eq!(
        show(&pair, &id)["allowed"],
        json!([{"to": "edit", "by": by}])
    );
    assert_eq!(
        doc(&pair, &["card", "list"])["flows"][0]["moves"][0]["by"],
        by,
        "the board's flows, trimmed"
    );
    let before = state(&pair);
    assert_eq!(
        refused(&pair, &["card", "move", &id, "--to", "edit", "--as", "ana"]),
        format!(
            "spec: {id} is in draft: the move to `edit` is `desk` or `{FULL_NAME}`'s, not \
             `ana`'s; nothing changed\n"
        )
    );
    assert_eq!(state(&pair), before);
    ok(
        &pair,
        &["card", "move", &id, "--to", "edit", "--as", "desk"],
    );

    // The longest name, in either script, a move's `by` takes it.
    let ascii = "n".repeat(128);
    let wide = "\u{0430}".repeat(64);
    for name in [ascii.as_str(), wide.as_str()] {
        write(
            &pair.main,
            "specengine.toml",
            config.replace("by = [\"desk\"]", &format!("by = [\"{name}\"]")),
        );
        let card = make(&pair, "Longest", &["--discipline", "copy"]);
        ok(
            &pair,
            &["card", "move", &card, "--to", "edit", "--as", name],
        );
    }
}

/// "Config": `by` holds persons: a name off the grammar is a bad `[card]`,
/// exit 2 at its line; `spec card` alone.
#[test]
fn a_by_off_the_person_grammar_is_a_bad_card_table() {
    let pair = Pair::new("cd-by-grammar", PRESS);
    let config = read_text(&pair.main, "specengine.toml");
    let long = "n".repeat(129);
    let wide = "\u{0430}".repeat(65);
    for name in [
        "a\u{2028}b",
        "a\u{202e}b",
        "   ",
        long.as_str(),
        wide.as_str(),
    ] {
        write(
            &pair.main,
            "specengine.toml",
            config.replace("by = [\"desk\"]", &format!("by = [\"{name}\"]")),
        );
        let line = cannot(&pair, &["card", "list"]);
        assert!(line.contains("specengine.toml:"), "{name:?}: {line}");
        assert!(line.contains("`by`"), "{name:?}: {line}");
        ok(&pair, &["tree"]);
    }
}

/// "Store": `revision` is decimal text in every document; a `--revision`
/// that is no decimal cannot run, exit 2, on every command that takes it;
/// nothing changed.
#[test]
fn revision_is_decimal_text() {
    let pair = Pair::new("cd-revision", PRESS);
    let id = make(&pair, "One", &["--discipline", "plates"]);
    assert_eq!(
        doc(&pair, &["card", "move", &id, "--to", "blocked"])["revision"],
        "2"
    );
    assert_eq!(show(&pair, &id)["revision"], "2");
    assert_eq!(doc(&pair, &["card", "list"])["cards"][0]["revision"], "2");
    let before = state(&pair);
    for bad in ["two", "", "1.5", "-1"] {
        for args in [
            vec![
                "card",
                "move",
                id.as_str(),
                "--to",
                "checked",
                "--revision",
                bad,
            ],
            vec![
                "card",
                "edit",
                id.as_str(),
                "--goal",
                "G.",
                "--revision",
                bad,
            ],
            vec![
                "card",
                "assign",
                id.as_str(),
                "--assignee",
                "ana",
                "--revision",
                bad,
            ],
            vec![
                "card",
                "cancel",
                id.as_str(),
                "--reason",
                "No.",
                "--revision",
                bad,
            ],
        ] {
            cannot(&pair, &args);
            assert_eq!(state(&pair), before, "{args:?}");
        }
    }
}

// --------------------------------------------------------------- AC-04

/// AC-04, the pipe: `accept` with `y` on a piped stdin exits 2 with the
/// consent line and no prompt, whatever the card; first of all, before the
/// config is read. `--yes` does not exist.
#[test]
fn ac04_accept_on_a_pipe_exits_2_first() {
    let pair = Pair::new("cd-ac04-pipe", PRESS);
    let id = in_proof(&pair);
    let before = state(&pair);
    let line = "spec: `spec card accept` asks the owner for consent on a terminal, and stdin is \
                not one (a pipe, a script or an agent's shell): run it in a terminal; nothing \
                changed\n";
    for args in [
        vec!["card", "accept", id.as_str(), "--as", "ben"],
        vec!["card", "accept", "C-0099", "--as", "ben"],
        vec!["--json", "card", "accept", id.as_str(), "--as", "ben"],
    ] {
        let done = pair.spec_piped(&pair.main, &args, b"y\nyes\n");
        done.code(2);
        assert_eq!(done.stdout, "", "{}", done.show());
        assert_eq!(done.stderr, line, "{}", done.show());
        assert_eq!(state(&pair), before, "{args:?}");
    }
    // Before the config: a bad one changes nothing of it.
    append_config(&pair.main, "\n[card]\nbogus = 1\n");
    let done = pair.spec_piped(&pair.main, &["card", "accept", &id, "--as", "ben"], b"y\n");
    done.code(2);
    assert_eq!(done.stderr, line, "{}", done.show());
}

/// AC-04, the terminal: the assignee, a non-reviewer, `agent`, `local`
/// exit 1; a declined prompt exits 1 with no event; a card not in
/// `accept.from` exits 1; the reviewer's `y` moves it to `accept.to` with
/// the prompt of "CLI" and records `{by, at, note}`. Then every write
/// exits 1 naming `--corrects`, and `--corrects` itself works.
#[test]
fn ac04_only_the_reviewer_accepts_on_a_terminal() {
    let pair = Pair::new("cd-ac04-tty", PRESS);
    let id = make(&pair, "Lead story", &["--discipline", "copy"]);
    ok(
        &pair,
        &[
            "card",
            "assign",
            &id,
            "--assignee",
            "ana",
            "--reviewer",
            "ben",
        ],
    );
    // Not in `accept.from`.
    let before = state(&pair);
    let early = pair.spec_pty(
        &pair.main,
        &["card", "accept", &id, "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(early.code, 1, "{}", early.output);
    assert!(early.output.contains("proof"), "{}", early.output);
    assert_eq!(state(&pair), before, "not in accept.from");

    ok(
        &pair,
        &["card", "move", &id, "--to", "edit", "--as", "desk"],
    );
    ok(&pair, &["card", "move", &id, "--to", "proof"]);
    let before = state(&pair);
    for name in ["ana", "cid", "agent", "local"] {
        let tried = pair.spec_pty(
            &pair.main,
            &["card", "accept", &id, "--as", name, "--briefed"],
            Some("y"),
        );
        assert_eq!(tried.code, 1, "{name}: {}", tried.output);
        assert!(tried.output.contains("spec: "), "{name}: {}", tried.output);
        assert_eq!(state(&pair), before, "{name}");
    }
    let wrong = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "accept",
            &id,
            "--as",
            "ben",
            "--revision",
            "1",
            "--briefed",
        ],
        Some("y"),
    );
    assert_eq!(wrong.code, 1, "{}", wrong.output);
    assert_eq!(state(&pair), before, "a stale --revision");

    let declined = pair.spec_pty(
        &pair.main,
        &["card", "accept", &id, "--as", "ben", "--briefed"],
        Some("n"),
    );
    assert!(declined.asked, "{}", declined.output);
    assert!(
        declined
            .output
            .contains("accept C-0001 (Lead story) as ben? [y/N]"),
        "{}",
        declined.output
    );
    assert_eq!(declined.code, 1, "{}", declined.output);
    assert_eq!(state(&pair), before, "no event");

    let accepted = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "accept",
            &id,
            "--as",
            "ben",
            "--note",
            "Fine.",
            "--briefed",
        ],
        Some("y"),
    );
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert!(
        accepted.output.contains("accepted C-0001: copy/filed\n"),
        "{}",
        accepted.output
    );
    let card = show(&pair, &id);
    assert_eq!(card["column"], "filed");
    assert_eq!(card["acceptance"]["by"], "ben");
    assert_eq!(card["acceptance"]["note"], "Fine.");
    assert!(card["acceptance"]["at"].is_string(), "{card}");

    let before = state(&pair);
    for args in writes(&id) {
        let reason = refused(&pair, &args);
        assert!(reason.contains("--corrects"), "{args:?}: {reason}");
        assert_eq!(state(&pair), before, "{args:?}");
    }
    let again = pair.spec_pty(
        &pair.main,
        &["card", "accept", &id, "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(again.code, 1, "{}", again.output);
    assert!(again.output.contains("--corrects"), "{}", again.output);
    assert_eq!(state(&pair), before);

    let fix = make(
        &pair,
        "A correction",
        &["--discipline", "copy", "--corrects", &id],
    );
    assert_eq!(show(&pair, &fix)["corrects"], id.as_str());
}

/// AC-04: `--yes` is no flag of `accept`, and on a terminal it is not
/// taken for consent: clap's exit 2, no prompt, nothing changed. `--as` is
/// required.
#[test]
fn ac04_accept_has_no_yes_and_needs_as() {
    let pair = Pair::new("cd-ac04-yes", PRESS);
    let id = in_proof(&pair);
    ok(&pair, &["card", "assign", &id, "--reviewer", "ben"]);
    let before = state(&pair);
    let yes = pair.spec_pty(
        &pair.main,
        &["card", "accept", &id, "--as", "ben", "--yes"],
        None,
    );
    assert_eq!(yes.code, 2, "{}", yes.output);
    assert!(!yes.asked, "{}", yes.output);
    assert!(yes.output.contains("--yes"), "{}", yes.output);
    let bare = pair.spec_pty(&pair.main, &["card", "accept", &id], None);
    assert_eq!(bare.code, 2, "{}", bare.output);
    assert!(bare.output.contains("--as"), "{}", bare.output);
    assert_eq!(state(&pair), before);
}

/// AC-04: a card whose reviewer is its assignee can never be accepted by
/// them, and neither `agent` nor `local` can be its reviewer. M:
/// `move --to` reaching `accept.to` (the first refusal below).
#[test]
fn ac04_no_move_reaches_accept_to_and_no_self_acceptance() {
    let pair = Pair::new("cd-ac04-self", PRESS);
    let id = in_proof(&pair);
    let before = state(&pair);
    assert_eq!(
        refused(
            &pair,
            &["card", "move", &id, "--to", "filed", "--as", "desk"]
        ),
        "spec: C-0001 is in proof: no move to `filed`; allowed: `spec card accept`; nothing \
         changed\n"
    );
    assert_eq!(state(&pair), before);
    ok(&pair, &["card", "assign", &id, "--assignee", "ben"]);
    let before = state(&pair);
    let tried = pair.spec_pty(
        &pair.main,
        &["card", "accept", &id, "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(tried.code, 1, "{}", tried.output);
    assert_eq!(state(&pair), before, "the assignee is never the acceptor");
    for name in ["agent", "local"] {
        let reason = refused(&pair, &["card", "assign", &id, "--reviewer", name]);
        assert!(reason.contains(name), "{reason}");
    }
    assert_eq!(state(&pair), before);
}

/// "Rules and edge cases": `task approve` takes no `C-`, and an accepted
/// card stands nowhere near a task.
#[test]
fn task_approve_takes_no_card() {
    let pair = Pair::new("cd-approve", PRESS);
    let id = in_proof(&pair);
    let before = state(&pair);
    let tried = pair.spec_pty(&pair.main, &["task", "approve", &id], Some("y"));
    assert_ne!(tried.code, 0, "{}", tried.output);
    assert!(!tried.asked, "{}", tried.output);
    assert_eq!(state(&pair), before);
}

// --------------------------------------------------------------- AC-05

/// AC-05: no `[card]`: one flow line of five zeros, nothing seeded, no
/// `T-` shown though a task exists; a new card in `plain/backlog`; the
/// flow's titles and twelve moves in the document; `PLAIN_FLOW` pasted:
/// `list --json` byte-identical. M: a branch on a column word (with
/// `ac10_the_card_sources_name_no_word_of_a_project`).
#[test]
fn ac05_the_default_flow_and_plain_flow_pasted() {
    let pair = Pair::new("cd-ac05", "spec-a");
    ok(
        &pair,
        &["task", "new", "--nodes", "MEC-STAMINA", "--title", "A task"],
    );
    assert_eq!(ok(&pair, &["card", "list"]), PLAIN_BOARD);
    let empty = doc(&pair, &["card", "list"]);
    assert_eq!(empty["cards"], json!([]), "nothing seeded");
    assert_eq!(empty["truncated"], false);
    assert_eq!(empty["omitted"], Value::Null, "nothing cut");
    assert_eq!(
        ok(&pair, &["card", "new", "--title", "First"]),
        "created C-0001: plain/backlog\n"
    );
    let listed = json_run(&pair, &["card", "list"]);
    listed.code(0);
    assert_eq!(task_common::keys(&listed.stdout), LIST_KEYS);
    let document = listed.json();
    assert_eq!(document["flows"].as_array().map(Vec::len), Some(1));
    let plain = &document["flows"][0];
    assert_eq!(plain["name"], "plain");
    assert_eq!(plain["disciplines"], json!([]));
    assert_eq!(
        plain["columns"],
        json!([
            {"id": "backlog", "title": "Backlog"},
            {"id": "ready", "title": "Ready"},
            {"id": "in-progress", "title": "In progress"},
            {"id": "review", "title": "Review"},
            {"id": "done", "title": "Done"},
        ])
    );
    assert_eq!(plain["accept"], json!({"from": "review", "to": "done"}));
    assert_eq!(
        plain["moves"].as_array().map(Vec::len),
        Some(12),
        "every ordered pair of four columns: {plain}"
    );
    let plain_text = ok(&pair, &["card", "list"]);
    assert_eq!(
        plain_text,
        "flow plain | backlog 1 | ready 0 | in-progress 0 | review 0 | done 0\n\
         C-0001 | plain/backlog | 1 | First | - | - | - | -\n"
    );

    // Pasted: the same queue, the same bytes, the spec's text and core's.
    let base = read_text(&pair.main, "specengine.toml");
    for (label, flow) in [
        ("as the spec gives it", PLAIN_TOML),
        ("core's own", specengine_core::PLAIN_FLOW),
    ] {
        write(&pair.main, "specengine.toml", format!("{base}\n{flow}"));
        let pasted = json_run(&pair, &["card", "list"]);
        pasted.code(0);
        assert_eq!(pasted.stdout, listed.stdout, "PLAIN_FLOW pasted, {label}");
        assert_eq!(ok(&pair, &["card", "list"]), plain_text, "{label}");
    }
}

/// `fixtures/card-press/expected.json`: the board and the flows of "Data",
/// worked out by hand; `by: []` reads anyone. Since board-handoff each flow
/// also serves `return` (docs/canon/cards.md "Documents and events"), `null`
/// on card-press, whose fixture stays byte-equal, so the key is added here,
/// never there.
#[test]
fn the_press_board_is_its_expected_json() {
    let pair = Pair::new("cd-board", PRESS);
    let expected: Value =
        serde_json::from_str(&read_text(&fixture(PRESS), "expected.json")).expect("JSON");
    assert_eq!(expected["board"], PRESS_BOARD);
    assert_eq!(ok(&pair, &["card", "list"]), PRESS_BOARD);
    let flows = doc(&pair, &["card", "list"])["flows"].clone();
    let mut want = expected["flows"].clone();
    for flow in want.as_array_mut().expect("flows") {
        assert!(
            flow.get("return").is_none(),
            "the fixture predates `return`"
        );
        flow["return"] = Value::Null;
    }
    assert_eq!(flows, want);
}

/// One `[[card.flow]]` of columns `a`, `b`, `c`, accept `b` to `c`, with
/// `extra` lines.
fn flow_toml(name: &str, extra: &str) -> String {
    format!(
        "\n[[card.flow]]\nname = \"{name}\"\ncolumns = [\"a\", \"b\", \"c\"]\naccept = {{ from = \
         \"b\", to = \"c\" }}\n{extra}"
    )
}

/// The `[card]` tables "Data" refuses: `(label, the table, a word of the
/// message)`.
fn bad_tables() -> Vec<(&'static str, String, &'static str)> {
    let columns = |count: usize| {
        (0..count)
            .map(|index| format!("\"k{index}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut many = String::new();
    for index in 0..33 {
        many.push_str(&flow_toml(&format!("f{index}"), ""));
    }
    vec![
        ("a name out of grammar", flow_toml("Copy", ""), "Copy"),
        ("a name of 65 bytes", flow_toml(&"n".repeat(65), ""), "64"),
        ("an empty name", flow_toml("", ""), "name"),
        ("33 flows", many, "32"),
        (
            "one column",
            "\n[[card.flow]]\nname = \"f\"\ncolumns = [\"a\"]\naccept = { from = \"a\", to = \
             \"a\" }\n"
                .to_owned(),
            "columns",
        ),
        (
            "33 columns",
            format!(
                "\n[[card.flow]]\nname = \"f\"\ncolumns = [{}]\naccept = {{ from = \"k0\", to = \
                 \"k1\" }}\n",
                columns(33)
            ),
            "32",
        ),
        (
            "a column out of grammar",
            "\n[[card.flow]]\nname = \"f\"\ncolumns = [\"a\", \"B c\"]\naccept = { from = \"a\", \
             to = \"B c\" }\n"
                .to_owned(),
            "B c",
        ),
        (
            "a repeated column",
            "\n[[card.flow]]\nname = \"f\"\ncolumns = [\"a\", \"a\"]\naccept = { from = \"a\", \
             to = \"a\" }\n"
                .to_owned(),
            "a",
        ),
        (
            "no accept",
            "\n[[card.flow]]\nname = \"f\"\ncolumns = [\"a\", \"b\"]\n".to_owned(),
            "accept",
        ),
        (
            "accept of a column the flow lacks",
            "\n[[card.flow]]\nname = \"f\"\ncolumns = [\"a\", \"b\"]\naccept = { from = \"a\", \
             to = \"z\" }\n"
                .to_owned(),
            "z",
        ),
        (
            "accept of one column",
            "\n[[card.flow]]\nname = \"f\"\ncolumns = [\"a\", \"b\"]\naccept = { from = \"a\", \
             to = \"a\" }\n"
                .to_owned(),
            "accept",
        ),
        (
            "a move ending at accept.to",
            flow_toml("f", "moves = [{ from = \"a\", to = \"c\" }]\n"),
            "c",
        ),
        (
            "a move starting at accept.to",
            flow_toml("f", "moves = [{ from = \"c\", to = \"a\" }]\n"),
            "c",
        ),
        (
            "a move of a column the flow lacks",
            flow_toml("f", "moves = [{ from = \"a\", to = \"z\" }]\n"),
            "z",
        ),
        (
            "a title of 65 bytes",
            flow_toml("f", &format!("titles = {{ a = \"{}\" }}\n", "t".repeat(65))),
            "64",
        ),
        (
            "two flows of one name",
            format!("{}{}", flow_toml("f", ""), flow_toml("f", "")),
            "f",
        ),
        (
            "another key",
            flow_toml("f", "colour = \"red\"\n"),
            "colour",
        ),
        (
            "another key in [card]",
            "\n[card]\nbogus = 1\n".to_owned(),
            "bogus",
        ),
    ]
}

/// AC-05: each bad `[card]` exits 2 `<config>:<line>: <message>` for `spec
/// card` alone: `spec tree`, `spec check` and a task read print what they
/// printed, nothing changed in the queue.
#[test]
fn ac05_a_bad_card_table_stops_spec_card_alone() {
    let pair = Pair::new("cd-ac05-bad", "spec-a");
    make(&pair, "One", &[]);
    let base = read_text(&pair.main, "specengine.toml");
    let reads: [&[&str]; 3] = [&["tree"], &["check"], &["--json", "task", "list"]];
    let before: Vec<(i32, String, String)> = reads
        .iter()
        .map(|args| {
            let done = run(&pair, args);
            (done.code, done.stdout, done.stderr)
        })
        .collect();
    let queue = state(&pair);
    for (label, table, word) in bad_tables() {
        write(&pair.main, "specengine.toml", format!("{base}{table}"));
        for args in [
            vec!["card", "list"],
            vec!["card", "new", "--title", "Two"],
            vec!["card", "show", "C-0001"],
            vec!["card", "cancel", "C-0001", "--reason", "No."],
        ] {
            let message = cannot(&pair, &args);
            let at = message
                .find("specengine.toml:")
                .unwrap_or_else(|| panic!("{label}: {args:?}: {message}"));
            let rest = &message[at + "specengine.toml:".len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let line: usize = digits
                .parse()
                .unwrap_or_else(|_| panic!("{label}: {message}"));
            assert!(rest[digits.len()..].starts_with(": "), "{label}: {message}");
            assert!(
                line > base.lines().count(),
                "{label}: a line of the table: {message}"
            );
            assert!(message.contains(word), "{label}: {word}: {message}");
        }
        for (args, (code, stdout, stderr)) in reads.iter().zip(&before) {
            let done = run(&pair, args);
            assert_eq!(done.code, *code, "{label}: {args:?}: {}", done.show());
            assert_eq!(done.stdout, *stdout, "{label}: {args:?}");
            assert_eq!(done.stderr, *stderr, "{label}: {args:?}");
        }
        assert_eq!(state(&pair), queue, "{label}");
    }

    // `card` no table: above the first one, a key of the root.
    write(&pair.main, "specengine.toml", format!("card = 1\n{base}"));
    let message = cannot(&pair, &["card", "list"]);
    assert!(message.contains("specengine.toml:1: "), "{message}");
    assert!(message.contains("card"), "{message}");
    for (args, (code, stdout, _)) in reads.iter().zip(&before) {
        let done = run(&pair, args);
        assert_eq!(done.code, *code, "{args:?}: {}", done.show());
        assert_eq!(done.stdout, *stdout, "{args:?}");
    }
}

/// docs/features/departments.md DP-5: an older binary reads a flow's
/// `title` as an unknown key, so what an unknown key does is the rollout's
/// proof. `colour`, unknown before and after that slice, in a
/// `[[card.flow]]` four lines below its header: `spec card list` exits 2,
/// stdout empty, stderr starting `specengine.toml:<the colour line>: unknown
/// field `colour`, expected one of ` (the prefix only: the list grows by
/// `title`). M: the line taken from the flow's `[[card.flow]]` header.
#[test]
fn dp5_an_unknown_flow_key_exits_2_at_its_line() {
    let pair = Pair::new("cd-dp5", "spec-a");
    let base = read_text(&pair.main, "specengine.toml");
    let text = format!("{base}{}", flow_toml("f", "colour = \"red\"\n"));
    let header = 1 + text
        .lines()
        .position(|line| line == "[[card.flow]]")
        .expect("the flow's header");
    let colour = 1 + text
        .lines()
        .position(|line| line == "colour = \"red\"")
        .expect("the colour line");
    assert!(colour > header + 1, "the key is not the header's neighbour");
    write(&pair.main, "specengine.toml", text);
    let stderr = cannot(&pair, &["card", "list"]);
    let prefix = format!("specengine.toml:{colour}: unknown field `colour`, expected one of ");
    assert!(
        stderr.starts_with(&prefix),
        "want the prefix {prefix:?}, got {stderr:?}"
    );
    assert_eq!(stderr.lines().count(), 1, "one line: {stderr:?}");
}

/// "Data": the bounds themselves are taken: a 64-byte name and title, 2
/// and 32 columns, 32 flows, a title in another script; `titles` default to
/// the ID.
#[test]
fn the_config_bounds_are_taken() {
    let pair = Pair::new("cd-bounds", "spec-a");
    let base = read_text(&pair.main, "specengine.toml");
    let name = "n".repeat(64);
    let columns: Vec<String> = (0..32).map(|index| format!("\"k{index}\"")).collect();
    let mut flows = format!(
        "\n[[card.flow]]\nname = \"{name}\"\ncolumns = [{}]\naccept = {{ from = \"k0\", to = \
         \"k1\" }}\ntitles = {{ k0 = \"{}\", k1 = \"\u{0421}\u{0434}\u{0430}\u{0447}\u{0430}\" \
         }}\n",
        columns.join(", "),
        "t".repeat(64)
    );
    flows.push_str(
        "\n[[card.flow]]\nname = \"two\"\ncolumns = [\"a\", \"b\"]\naccept = { from = \"a\", to \
         = \"b\" }\n",
    );
    for index in 2..32 {
        flows.push_str(&flow_toml(&format!("f{index}"), ""));
    }
    write(&pair.main, "specengine.toml", format!("{base}{flows}"));
    let document = doc(&pair, &["card", "list"]);
    let read = document["flows"].as_array().expect("flows");
    assert_eq!(read.len(), 32);
    assert_eq!(read[0]["name"], name.as_str());
    assert_eq!(read[0]["columns"].as_array().map(Vec::len), Some(32));
    assert_eq!(read[0]["columns"][0]["title"], "t".repeat(64));
    assert_eq!(
        read[0]["columns"][1]["title"],
        "\u{0421}\u{0434}\u{0430}\u{0447}\u{0430}"
    );
    assert_eq!(
        read[0]["columns"][2]["title"], "k2",
        "the default is the ID"
    );
    assert_eq!(read[1]["columns"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        read[1]["moves"],
        json!([]),
        "one column left, no ordered pair"
    );
}

/// "Data": a new card takes the first column of the flow listing its
/// discipline, else of the one flow with no `disciplines`, else exit 1.
#[test]
fn a_new_card_finds_its_flow_by_discipline() {
    let pair = Pair::new("cd-route", PRESS);
    assert_eq!(
        ok(
            &pair,
            &["card", "new", "--title", "P", "--discipline", "plates"]
        ),
        "created C-0001: plates/queued\n"
    );
    assert_eq!(
        ok(
            &pair,
            &["card", "new", "--title", "C", "--discipline", "copy"]
        ),
        "created C-0002: copy/draft\n"
    );
    let before = state(&pair);
    refused(&pair, &["card", "new", "--title", "No trade"]);
    refused(
        &pair,
        &["card", "new", "--title", "X", "--discipline", "paint"],
    );
    assert_eq!(state(&pair), before);

    // The one flow with no `disciplines` takes the rest.
    append_config(&pair.main, &flow_toml("any", ""));
    assert_eq!(
        ok(
            &pair,
            &["card", "new", "--title", "Y", "--discipline", "paint"]
        ),
        "created C-0003: any/a\n"
    );
    assert_eq!(
        ok(&pair, &["card", "new", "--title", "Z"]),
        "created C-0004: any/a\n"
    );
    // Two such flows: none is the one.
    append_config(&pair.main, &flow_toml("other", ""));
    let before = state(&pair);
    refused(&pair, &["card", "new", "--title", "W"]);
    assert_eq!(state(&pair), before);

    // The default flow takes every discipline.
    let plain = Pair::new("cd-route-plain", "spec-a");
    assert_eq!(
        ok(
            &plain,
            &["card", "new", "--title", "V", "--discipline", "paint"]
        ),
        "created C-0001: plain/backlog\n"
    );
}

/// "Rules and edge cases": a `blocked` column means nothing: nothing stops a
/// card standing in it, and a project may call its columns anything.
#[test]
fn a_blocked_column_is_a_column() {
    let pair = Pair::new("cd-blocked", PRESS);
    make(&pair, "Plate", &["--discipline", "plates"]);
    ok(&pair, &["card", "move", "C-0001", "--to", "blocked"]);
    ok(&pair, &["card", "edit", "C-0001", "--goal", "Still here."]);
    ok(&pair, &["card", "assign", "C-0001", "--reviewer", "ben"]);
    ok(&pair, &["card", "move", "C-0001", "--to", "checked"]);
    let accepted = pair.spec_pty(
        &pair.main,
        &["card", "accept", "C-0001", "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert_eq!(show(&pair, "C-0001")["column"], "struck");
}

// --------------------------------------------------------------- AC-06

/// The columns of `replay`'s board, each a list of `(ID, position)`.
fn replay(pair: &Pair, home: &Path) -> (String, Vec<u8>) {
    let env = Env {
        cwd: pair.main.clone(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    };
    let globals = Globals::default();
    for title in ["First", "Second", "Third"] {
        let made = card_new(
            &env,
            &globals,
            &CardNewRequest {
                title: title.to_owned(),
                discipline: Some("plates".to_owned()),
                now: CLOCK.to_owned(),
                ..CardNewRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("new {title}: {error}"));
        assert_eq!(made.exit().code(), 0, "{made:?}");
    }
    let moved = card_move(
        &env,
        &globals,
        &CardMoveRequest {
            id: "C-0003".to_owned(),
            before: Some("C-0001".to_owned()),
            now: CLOCK.to_owned(),
            ..CardMoveRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("move: {error}"));
    assert_eq!(moved.exit().code(), 0, "{moved:?}");
    let list = card_list(&env, &globals, &specengine_cli::CardListRequest::default())
        .unwrap_or_else(|error| panic!("list: {error}"));
    let text = specengine_cli::render_json(&specengine_cli::Outcome::CardList(Box::new(list)));
    let out = pair.scratch.join(&format!(
        "{}.jsonl",
        home.file_name().and_then(|name| name.to_str()).unwrap()
    ));
    specengine_cli::export_state(
        &env,
        &globals,
        &specengine_cli::ExportStateRequest {
            out: Some(out.clone()),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export: {error}"));
    (text, fs::read(&out).expect("the dump"))
}

/// AC-06: `new`, `new`, `new`, `move --before` replayed in two `HOME`s at
/// one clock: `list --json` and `export state` byte-identical, position
/// order, the neighbours' `revision` unchanged. M: ordering by
/// `updated_at` (every row ties here, so that order is by ID).
#[test]
fn ac06_two_homes_replaying_one_history_are_byte_identical() {
    let pair = Pair::new("cd-ac06", PRESS);
    let first = pair.scratch.home("one");
    let second = pair.scratch.home("two");
    let one = replay(&pair, &first);
    let two = replay(&pair, &second);
    assert_eq!(one.0, two.0, "list --json");
    assert_eq!(one.1, two.1, "export state");
    let document: Value = serde_json::from_str(&one.0).expect("JSON");
    let order: Vec<(&str, i64, &str)> = document["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap(),
                row["position"].as_i64().unwrap(),
                row["revision"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [("C-0003", 1, "2"), ("C-0001", 2, "1"), ("C-0002", 3, "1")],
        "the mover first, its neighbours shifted and their revisions kept"
    );
}

/// "Data", Order: `new` and a move go last; `--before C` takes C's place in
/// C's column; both columns stay dense from 1; a shifted neighbour takes no
/// `revision` and no event, so its compare-and-set still holds; `--to` its
/// own column is a reorder under no rule, even where `moves` has no such
/// pair; `--before` a card of another column exits 1.
#[test]
fn order_stays_dense_and_a_shifted_neighbour_keeps_its_revision() {
    let pair = Pair::new("cd-order", PRESS);
    for title in ["A", "B", "C"] {
        make(&pair, title, &["--discipline", "plates"]);
    }
    let dense = |pair: &Pair, want: &[(&str, i64)]| {
        for (id, position) in want {
            assert_eq!(place(pair, id).0, *position, "{id}: {want:?}");
        }
    };
    dense(&pair, &[("C-0001", 1), ("C-0002", 2), ("C-0003", 3)]);

    let seen = events(&pair).len();
    ok(&pair, &["card", "move", "C-0003", "--before", "C-0001"]);
    assert_eq!(events(&pair).len(), seen + 1, "the mover's one event");
    assert_eq!(
        events(&pair).last().map(|event| event.1.clone()),
        Some(
            json!({"id": "C-0003", "from": "queued", "to": "queued", "position": 1,
                    "as": "local", "note": null})
        )
    );
    dense(&pair, &[("C-0003", 1), ("C-0001", 2), ("C-0002", 3)]);
    assert_eq!(place(&pair, "C-0001").1, "1", "no revision for a neighbour");
    assert_eq!(place(&pair, "C-0002").1, "1", "no revision for a neighbour");
    assert_eq!(place(&pair, "C-0003").1, "2");
    assert_eq!(listed(&pair, &[]), ["C-0003", "C-0001", "C-0002"]);
    // The neighbour's compare-and-set still holds.
    ok(
        &pair,
        &["card", "edit", "C-0001", "--goal", "G.", "--revision", "1"],
    );

    // Across columns: last there, the old column closes up.
    ok(&pair, &["card", "move", "C-0003", "--to", "blocked"]);
    dense(&pair, &[("C-0001", 1), ("C-0002", 2), ("C-0003", 1)]);
    ok(
        &pair,
        &[
            "card", "move", "C-0001", "--to", "blocked", "--before", "C-0003",
        ],
    );
    dense(&pair, &[("C-0002", 1), ("C-0001", 1), ("C-0003", 2)]);
    assert_eq!(
        listed(&pair, &["--column", "blocked"]),
        ["C-0001", "C-0003"]
    );

    // `--to` its own column: last, no rule. On `copy` no pair names `draft`
    // and leaving it takes the desk: staying takes nobody.
    make(&pair, "D", &["--discipline", "copy"]);
    make(&pair, "E", &["--discipline", "copy"]);
    assert_eq!(
        ok(&pair, &["card", "move", "C-0004", "--to", "draft"]),
        "moved C-0004: copy/draft\n"
    );
    dense(&pair, &[("C-0005", 1), ("C-0004", 2)]);
    ok(&pair, &["card", "move", "C-0004", "--before", "C-0005"]);
    dense(&pair, &[("C-0004", 1), ("C-0005", 2)]);

    // `--before` a card of another column, or the card itself.
    let before = state(&pair);
    let reason = refused(&pair, &["card", "move", "C-0002", "--before", "C-0002"]);
    assert!(reason.contains("--before"), "{reason}");
    refused(&pair, &["card", "move", "C-0002", "--before", "C-0003"]);
    refused(
        &pair,
        &[
            "card", "move", "C-0002", "--to", "checked", "--before", "C-0003",
        ],
    );
    refused(&pair, &["card", "move", "C-0002", "--before", "C-0099"]);
    assert_eq!(state(&pair), before);

    // Ties by ID: two rows written to one position.
    pair.sql("UPDATE cards SET position = 7 WHERE id IN ('C-0004', 'C-0005');");
    assert_eq!(listed(&pair, &["--flow", "copy"]), ["C-0004", "C-0005"]);
}

// --------------------------------------------------------------- AC-07

fn env_at(pair: &Pair, home: &Path) -> Env {
    Env {
        cwd: pair.main.clone(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

fn export(pair: &Pair, home: &Path, out: &Path) -> specengine_cli::ExportStateOutcome {
    specengine_cli::export_state(
        &env_at(pair, home),
        &Globals::default(),
        &specengine_cli::ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"))
}

type Imported = (
    Result<specengine_cli::ImportStateOutcome, specengine_cli::CliError>,
    Vec<String>,
);

fn import(pair: &Pair, home: &Path, file: &Path) -> Imported {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::import_state(
        &env_at(pair, home),
        &Globals::default(),
        &specengine_cli::ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    );
    (outcome, questions)
}

fn dump_in(home: &Path, slug: &str) -> String {
    specengine_store::SqliteQueue::open(data_dir(home).join(format!("{slug}.db")), slug)
        .expect("open")
        .dump()
        .expect("dump")
}

fn lines_of(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .expect("a dump")
        .lines()
        .map(str::to_owned)
        .collect()
}

fn write_lines(dir: &Path, name: &str, lines: &[String]) -> PathBuf {
    let file = dir.join(name);
    fs::write(&file, format!("{}\n", lines.join("\n"))).expect("a dump");
    file
}

/// A press board with a card of every shape: `C-0001` accepted, `C-0002`
/// cancelled, `C-0003` with every field, `C-0004` bare; a task beside them.
fn cards_queue(pair: &Pair) {
    let accepted = in_proof(pair);
    assert_eq!(accepted, "C-0001");
    let by_hand = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "accept",
            "C-0001",
            "--as",
            "ben",
            "--note",
            "Fine.",
            "--briefed",
        ],
        Some("y"),
    );
    assert_eq!(by_hand.code, 0, "{}", by_hand.output);
    make(pair, "Gone", &["--discipline", "plates"]);
    ok(pair, &["card", "cancel", "C-0002", "--reason", "Dropped."]);
    make(
        pair,
        "Full",
        &[
            "--discipline",
            "copy",
            "--goal",
            "All of it.",
            "--target",
            "SHT-BACK",
            "--criterion",
            "Holds.",
            "--estimate",
            "3",
            "--due",
            "2026-11-05",
            "--assignee",
            "ana",
            "--parent",
            "C-0001",
            "--corrects",
            "C-0001",
        ],
    );
    ok(
        pair,
        &[
            "card",
            "edit",
            "C-0003",
            "--evidence",
            "path=docs/spec/sht-back.md@0123456789abcdef0123456789abcdef01234567",
            "--evidence",
            "note=Read twice.",
        ],
    );
    make(pair, "Bare", &["--discipline", "plates"]);
    ok(
        pair,
        &["task", "new", "--nodes", "SHT-FRONT", "--title", "A task"],
    );
}

/// AC-07: a queue of every card shape dumps as format 4, schema 10 (format 3,
/// schema 9 until docs/canon/cards.md "Store" added `tasks`), the
/// header's eight keys in order, `{"cards":{…}}` rows by number after the
/// runs, their columns in `CARD_COLUMNS` order, `position` a number; the
/// count lines and documents gain `cards`; a fresh `HOME` restores it,
/// `dump()` equal, a re-export byte-identical; the restored queue holds
/// the board. M: `stored_rows` skipping `cards`.
#[test]
fn ac07_cards_round_trip_through_a_format_3_dump() {
    let pair = Pair::new("cd-ac07", PRESS);
    cards_queue(&pair);
    let before = pair.queue().dump().expect("dump");
    assert_eq!(
        before
            .lines()
            .filter(|line| line.starts_with("cards\t"))
            .count(),
        4
    );
    let event_rows = pair.events().len();
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    let exported = export(&pair, &pair.home, &file);
    assert_eq!(
        (
            exported.proposals,
            exported.tasks,
            exported.runs,
            exported.cards,
            exported.events
        ),
        (0, 1, 0, 4, event_rows as u64)
    );
    let outcome = specengine_cli::Outcome::StateExport(exported);
    assert_eq!(
        specengine_cli::render_text(&outcome),
        format!(
            "wrote {}: 0 proposal(s), 1 task(s), 0 run(s), 4 card(s), {event_rows} event(s)\n",
            file.display()
        )
    );
    assert_eq!(
        serde_json::from_str::<Value>(&specengine_cli::render_json(&outcome)).unwrap(),
        json!({"path": file.display().to_string(), "proposals": 0, "tasks": 1, "runs": 0,
               "cards": 4, "events": event_rows})
    );
    let bytes = fs::read(&file).expect("the dump");
    let lines = lines_of(&file);
    assert_eq!(
        lines[0],
        format!(
            "{{\"format\":6,\"queue_schema\":12,\"project\":\"card-press\",\"proposals\":0,\
             \"tasks\":1,\"runs\":0,\"cards\":4,\"events\":{event_rows}}}"
        )
    );
    assert_eq!(lines.len(), 1 + 1 + 4 + event_rows);
    let tables: Vec<String> = lines[1..]
        .iter()
        .map(|line| task_common::keys(line)[0].clone())
        .collect();
    let mut want = vec!["tasks"];
    want.extend(["cards"; 4]);
    want.extend(vec!["events"; event_rows]);
    assert_eq!(tables, want, "by table: cards after the runs");
    for (index, line) in lines[2..6].iter().enumerate() {
        assert_eq!(
            Ordered::of(line).get("cards").keys(),
            specengine_store::CARD_COLUMNS,
            "{line}"
        );
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(row["cards"]["id"], json!(format!("C-{:04}", index + 1)));
        assert!(row["cards"]["position"].is_i64(), "a number: {line}");
        assert!(row["cards"]["revision"].is_string(), "text: {line}");
        assert_eq!(row["cards"]["project"], "card-press");
        // docs/features/card-trace.md: the column as stored, the entries' JSON
        // text on a card with a target, `NULL` on a manual one.
        assert_eq!(
            row["cards"]["snapshot"].is_string(),
            row["cards"]["targets"] != "[]",
            "{line}"
        );
    }

    let fresh = pair.scratch.home("fresh");
    let (restored, questions) = import(&pair, &fresh, &file);
    let restored = restored.unwrap_or_else(|error| panic!("import: {error}"));
    let db = data_dir(&fresh).join("card-press.db");
    assert_eq!(
        questions,
        [format!(
            "restore 0 proposal(s), 1 task(s), 0 run(s), 4 card(s) and {event_rows} event(s) of \
             card-press from {} into {}? [y/N]",
            file.display(),
            db.display()
        )]
    );
    let outcome = specengine_cli::Outcome::StateImport(restored);
    assert_eq!(
        specengine_cli::render_text(&outcome),
        format!(
            "restored 0 proposal(s), 1 task(s), 0 run(s), 4 card(s), {event_rows} event(s) into \
             {}\n",
            db.display()
        )
    );
    assert_eq!(
        serde_json::from_str::<Value>(&specengine_cli::render_json(&outcome)).unwrap(),
        json!({"db": db.display().to_string(), "proposals": 0, "tasks": 1, "runs": 0,
               "cards": 4, "events": event_rows})
    );
    assert_eq!(dump_in(&fresh, "card-press"), before, "dump() equal");
    let again = dumps.join("again.jsonl");
    export(&pair, &fresh, &again);
    assert_eq!(fs::read(&again).unwrap(), bytes, "byte-identical");

    // The next ID follows the highest restored.
    let next = specengine_cli::card_new(
        &env_at(&pair, &fresh),
        &Globals::default(),
        &CardNewRequest {
            title: "After".to_owned(),
            discipline: Some("copy".to_owned()),
            now: CLOCK.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("new: {error}"));
    assert_eq!(next.id.as_deref(), Some("C-0005"));

    // A queue holding the cards refuses a second import, naming them.
    let (second, questions) = import(&pair, &fresh, &file);
    let message = second.expect_err("occupied").message;
    assert!(
        message.contains("1 task(s), 0 run(s), 5 card(s)"),
        "{message}"
    );
    assert!(questions.is_empty());
}

/// AC-07: a queue holding one card and nothing else is occupied.
#[test]
fn ac07_a_card_alone_occupies_a_queue() {
    let pair = Pair::new("cd-alone", PRESS);
    make(&pair, "One", &["--discipline", "copy"]);
    pair.sql("DELETE FROM events;");
    let before = pair.queue().dump().expect("dump");
    assert!(
        before.starts_with("cards\t") && !before.contains("events\t"),
        "{before}"
    );
    let other = Pair::new("cd-alone-other", PRESS);
    make(&other, "Another", &["--discipline", "copy"]);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    export(&other, &other.home, &file);
    let (outcome, questions) = import(&pair, &pair.home, &file);
    let message = outcome.expect_err("occupied").message;
    assert!(
        message.contains("holds 0 proposal(s), 0 task(s), 0 run(s), 1 card(s), 0 event(s)"),
        "{message}"
    );
    assert!(questions.is_empty());
    assert_eq!(pair.queue().dump().expect("dump"), before);
}

/// AC-07: a format-2 dump (seven header keys, no cards) restores into a
/// fresh queue and re-exports as format 5; a schema-8 database exports as
/// format 5 unmigrated, and steps to 11 when a command opens it (format 3,
/// schema 9 until docs/canon/cards.md "Store"; format 5, schema 11 since
/// docs/features/card-questions.md), rows
/// unchanged. Refused, exit 2 naming the file and the line, nothing made:
/// the refusals of "Backup".
#[test]
fn ac07_older_formats_restore_and_the_backup_refusals_exit_2() {
    let pair = Pair::new("cd-ac07-old", PRESS);
    ok(
        &pair,
        &["task", "new", "--nodes", "SHT-FRONT", "--title", "A task"],
    );
    let dumps = pair.scratch.dir("dumps");
    let current = dumps.join("v3.jsonl");
    export(&pair, &pair.home, &current);
    let v3 = fs::read_to_string(&current).unwrap();
    let v3_lines: Vec<String> = v3.lines().map(str::to_owned).collect();
    assert_eq!(
        v3_lines[0],
        "{\"format\":6,\"queue_schema\":12,\"project\":\"card-press\",\"proposals\":0,\"tasks\":1,\
         \"runs\":0,\"cards\":0,\"events\":1}"
    );
    let old_header = "{\"format\":2,\"queue_schema\":8,\"project\":\"card-press\",\"proposals\":0,\
                      \"tasks\":1,\"runs\":0,\"events\":1}";
    let mut v2_lines = v3_lines.clone();
    v2_lines[0] = old_header.to_owned();
    let v2 = write_lines(&dumps, "v2.jsonl", &v2_lines);
    let home = pair.scratch.home("v2");
    let (outcome, questions) = import(&pair, &home, &v2);
    outcome.unwrap_or_else(|error| panic!("import v2: {error}"));
    assert_eq!(questions.len(), 1);
    assert!(questions[0].contains("0 card(s)"), "{questions:?}");
    let again = dumps.join("v2-again.jsonl");
    export(&pair, &home, &again);
    assert_eq!(
        fs::read_to_string(&again).unwrap(),
        v3,
        "re-exported as format 5"
    );

    // A schema-8 database (no step-11 column either): exported unmigrated,
    // then stepped by a card.
    pair.sql(
        "ALTER TABLE proposals DROP COLUMN why; ALTER TABLE proposals DROP COLUMN card_id; \
         DROP TABLE cards; PRAGMA user_version = 8;",
    );
    let from_8 = dumps.join("from-8.jsonl");
    export(&pair, &pair.home, &from_8);
    assert_eq!(fs::read_to_string(&from_8).unwrap(), v3);
    assert_eq!(
        pair.sql("PRAGMA user_version;").trim(),
        "8",
        "nothing stepped"
    );
    let rows = pair.sql("SELECT * FROM tasks; SELECT * FROM events;");
    assert_eq!(ok(&pair, &["card", "list"]), PRESS_BOARD);
    // 12 since docs/features/task-mandate.md M5.
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "12", "stepped");
    assert_eq!(pair.sql("SELECT * FROM tasks; SELECT * FROM events;"), rows);

    // The refusals, on a dump holding one card.
    let cards = Pair::new("cd-ac07-cards", PRESS);
    make(&cards, "One", &["--discipline", "copy"]);
    let full = dumps.join("cards.jsonl");
    export(&cards, &cards.home, &full);
    let lines = lines_of(&full);
    assert!(lines[0].contains("\"cards\":1,"), "{}", lines[0]);
    let card_row = lines[1].clone();
    assert!(
        card_row.starts_with("{\"cards\":{\"id\":\"C-0001\""),
        "{card_row}"
    );
    let change = |line: usize, from: &str, to: &str| {
        let mut changed = lines.clone();
        assert!(changed[line].contains(from), "{from}: {}", changed[line]);
        changed[line] = changed[line].replacen(from, to, 1);
        changed
    };
    let mut repeated = lines.clone();
    repeated.insert(2, card_row.clone());
    repeated[0] = repeated[0].replacen("\"cards\":1,", "\"cards\":2,", 1);
    let mut format_2_with_a_card = lines.clone();
    format_2_with_a_card[0] = format_2_with_a_card[0]
        .replacen(
            "\"format\":6,\"queue_schema\":12,",
            "\"format\":2,\"queue_schema\":8,",
            1,
        )
        .replacen("\"cards\":1,", "", 1);
    let mut no_column = lines.clone();
    no_column[1] = card_row.replacen("\"goal\":null,", "", 1);
    let mut extra_column = lines.clone();
    extra_column[1] = card_row.replacen("\"goal\":null,", "\"goal\":null,\"bogus\":null,", 1);
    let cases: Vec<(&str, Vec<String>, usize)> = vec![
        // docs/features/card-questions.md: format 5, schema 11 the current;
        // docs/features/task-mandate.md M5: format 6, schema 12.
        ("format-6-of-seven-keys", change(0, "\"cards\":1,", ""), 1),
        (
            "format-6-of-schema-11",
            change(0, "\"queue_schema\":12,", "\"queue_schema\":11,"),
            1,
        ),
        (
            "format-2-of-eight-keys",
            change(
                0,
                "\"format\":6,\"queue_schema\":12,",
                "\"format\":2,\"queue_schema\":8,",
            ),
            1,
        ),
        // docs/features/task-mandate.md R1: format 6 refused at 8a43e5d, 7 here.
        ("format-7", change(0, "\"format\":6,", "\"format\":7,"), 1),
        (
            "schema-13",
            change(0, "\"queue_schema\":12,", "\"queue_schema\":13,"),
            1,
        ),
        ("a-card-in-format-2", format_2_with_a_card, 2),
        (
            "an-id-not-as-written",
            change(1, "\"C-0001\"", "\"C-1\""),
            2,
        ),
        ("an-id-of-a-task", change(1, "\"C-0001\"", "\"T-0001\""), 2),
        ("a-repeated-card", repeated, 3),
        (
            "a-position-as-text",
            change(1, "\"position\":1,", "\"position\":\"1\","),
            2,
        ),
        ("a-column-missing", no_column, 2),
        ("a-column-extra", extra_column, 2),
    ];
    for (name, lines, line) in cases {
        let file = write_lines(&dumps, &format!("{name}.jsonl"), &lines);
        let home = pair.scratch.home(name);
        let (outcome, questions) = import(&pair, &home, &file);
        let error = outcome.expect_err(name);
        assert_eq!(error.exit.code(), 2, "{name}: {}", error.message);
        assert!(
            error
                .message
                .contains(&format!("{}:{line}: ", file.display())),
            "{name}: {}",
            error.message
        );
        assert!(questions.is_empty(), "{name}");
        assert!(!data_dir(&home).exists(), "{name}: no queue made");
    }

    // A count off the header: both named, five each.
    let off = write_lines(
        &dumps,
        "off.jsonl",
        &change(0, "\"cards\":1,", "\"cards\":2,"),
    );
    let home = pair.scratch.home("off");
    let (outcome, questions) = import(&pair, &home, &off);
    let error = outcome.expect_err("a count off");
    assert_eq!(error.exit.code(), 2, "{}", error.message);
    assert!(error.message.contains("header counts"), "{}", error.message);
    assert!(questions.is_empty());
}

/// AC-07, the binary: `import-state` keeps its terminal, `export state`'s
/// `--json` names `cards`.
#[test]
fn ac07_the_binary_exports_and_imports_cards() {
    let pair = Pair::new("cd-ac07-bin", PRESS);
    make(&pair, "One", &["--discipline", "copy"]);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    let path = file.to_str().unwrap();
    let document = doc(&pair, &["export", "state", "--out", path]);
    assert_eq!(document["cards"], 1);
    assert_eq!(document["events"], 1);
    let done = run(&pair, &["import-state", path]);
    done.code(2);
    assert!(done.stderr.contains("terminal"), "{}", done.show());
}

// --------------------------------------------------------------- AC-08

/// `spec gate --worktree main --path src/a.rs --json`, whatever it answers:
/// its exit and both streams.
fn gate_answer(pair: &Pair) -> (i32, String, String) {
    let main = pair.main.to_str().unwrap();
    // `spec gate` answers within 3 s of wall clock (`GATE_DEADLINE`): a run a
    // loaded machine starves past that says `unavailable` and nothing of the
    // cards, so it is asked again (five tries; the last answer stands).
    let mut done = None;
    for _ in 0..5 {
        let run = run(
            pair,
            &["gate", "--worktree", main, "--path", "src/a.rs", "--json"],
        );
        let starved = run.stdout.contains("\"reason\":\"deadline\"")
            || run.stderr.contains("no answer within");
        done = Some(run);
        if !starved {
            break;
        }
    }
    let done = done.expect("a run");
    (done.code, done.stdout, done.stderr)
}

/// AC-08: bad JSON in `cards.evidence` and an `id` `X`: `spec gate --json`
/// answers as before, `task list` too; `card show` of the first exits 2
/// naming the column; `card list` shows the sound row and a note for each.
/// M: `GateQueue::tasks` reading `cards`.
#[test]
fn ac08_a_malformed_card_row_leaves_the_gate_open() {
    let pair = Pair::new("cd-ac08", PRESS);
    append_config(
        &pair.main,
        "\n[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"code\"]\n",
    );
    write(&pair.main, "src/a.rs", "fn a() {}\n");
    pair.commit_all(&pair.main, "the gate's scratch files");
    ok(
        &pair,
        &["task", "new", "--nodes", "SHT-FRONT", "--title", "A task"],
    );
    let gate = gate_answer(&pair);
    // A verdict the task rows gave, so the gate did read the queue.
    let verdict: Value = serde_json::from_str(&gate.1).expect("the gate's document");
    assert_eq!(gate.0, 2, "{gate:?}");
    assert_eq!(verdict["verdict"], "refuse", "{gate:?}");
    assert_eq!(verdict["cause"], Value::Null, "a real refusal: {gate:?}");
    assert_eq!(verdict["zone"], "code", "{gate:?}");
    let tasks = ok(&pair, &["--json", "task", "list"]);
    let task_text = ok(&pair, &["task", "list"]);
    for title in ["Bad evidence", "Bad id", "Sound"] {
        make(&pair, title, &["--discipline", "copy"]);
    }
    assert_eq!(gate_answer(&pair), gate, "a card changes no verdict");
    pair.sql(
        "UPDATE cards SET evidence = 'not json' WHERE id = 'C-0001'; UPDATE cards SET id = 'X' \
         WHERE id = 'C-0002';",
    );
    assert_eq!(gate_answer(&pair), gate, "spec gate --json as before");
    assert_eq!(ok(&pair, &["--json", "task", "list"]), tasks);
    assert_eq!(ok(&pair, &["task", "list"]), task_text);

    let message = cannot(&pair, &["card", "show", "C-0001"]);
    assert!(
        message.contains("C-0001") && message.contains("evidence"),
        "{message}"
    );
    let document = doc(&pair, &["card", "list"]);
    let ids: Vec<&str> = document["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, ["C-0003"], "the sound row alone");
    let notes = document["notes"].to_string();
    assert!(notes.contains("C-0001"), "{notes}");
    assert!(notes.contains('X'), "{notes}");
    // The text names them too, and a move on the sound card still runs.
    let text = run(&pair, &["card", "list"]);
    text.code(0);
    assert!(text.stderr.contains("C-0001"), "{}", text.show());
    ok(
        &pair,
        &["card", "move", "C-0003", "--to", "edit", "--as", "desk"],
    );
    assert_eq!(gate_answer(&pair), gate, "after a later write");
}

/// "Rules and edge cases": `spec card` reads no proposal, lock or task: a
/// corrupt task row, a garbage lock and an open question stop no card, and a
/// task read shows no card.
#[test]
fn cards_read_no_proposal_lock_or_task() {
    let pair = Pair::new("cd-apart", PRESS);
    ok(
        &pair,
        &["task", "new", "--nodes", "SHT-FRONT", "--title", "A task"],
    );
    pair.sql("UPDATE tasks SET status = 'bogus';");
    write(&pair.main, "spec.lock", "not a lock\n");
    write(&pair.main, "specengine.lock", "not a lock\n");
    let id = make(&pair, "Unstopped", &["--discipline", "plates"]);
    ok(&pair, &["card", "move", &id, "--to", "blocked"]);
    assert_eq!(listed(&pair, &[]), [id.as_str()]);
    assert!(
        !ok(&pair, &["task", "list"]).contains("C-0001"),
        "no card in a task read"
    );
}

// --------------------------------------------------------------- AC-09

/// AC-09: a U+0421 anywhere a card ID stands exits 2 naming the Latin form,
/// nothing changed.
#[test]
fn ac09_a_look_alike_id_exits_2_naming_the_latin_form() {
    let pair = Pair::new("cd-ac09-alike", PRESS);
    let id = make(&pair, "One", &["--discipline", "plates"]);
    let alike = "\u{0421}-0001";
    let before = state(&pair);
    for args in [
        vec!["card", "show", alike],
        vec!["card", "move", alike, "--to", "blocked"],
        vec!["card", "edit", alike, "--goal", "G."],
        vec!["card", "assign", alike, "--assignee", "ana"],
        vec!["card", "cancel", alike, "--reason", "No."],
        vec!["card", "move", &id, "--before", alike],
        vec![
            "card",
            "new",
            "--title",
            "T",
            "--parent",
            alike,
            "--discipline",
            "copy",
        ],
        vec![
            "card",
            "new",
            "--title",
            "T",
            "--corrects",
            alike,
            "--discipline",
            "copy",
        ],
        vec!["card", "edit", &id, "--parent", alike],
    ] {
        let message = cannot(&pair, &args);
        assert!(message.contains("`C-0001`"), "{args:?}: {message}");
        assert_eq!(state(&pair), before, "{args:?}");
    }
    let accept = pair.spec_pty(
        &pair.main,
        &["card", "accept", alike, "--as", "ben", "--briefed"],
        None,
    );
    assert_eq!(accept.code, 2, "{}", accept.output);
    assert!(accept.output.contains("`C-0001`"), "{}", accept.output);
    assert_eq!(state(&pair), before);
    // A look-alike digit too: a fullwidth one normalises or it is no ID.
    let run_digit = run(&pair, &["card", "show", "\u{0421}-\u{0660}"]);
    assert!(
        run_digit.code == 1 || run_digit.code == 2,
        "{}",
        run_digit.show()
    );
}

/// AC-09: `C` in `[ids]`, as a prefix or read through `aliases_from`, exits
/// 2 naming the config for `spec card` alone: every other command runs.
#[test]
fn ac09_c_in_ids_stops_spec_card_alone() {
    for (label, line) in [
        (
            "prefix",
            "C = { kind = \"chapter\", width = 4 }\n".to_owned(),
        ),
        (
            "alias",
            "CHP = { kind = \"chapter\", width = 4, aliases_from = [\"\u{0421}\"] }\n".to_owned(),
        ),
    ] {
        let pair = Pair::new(&format!("cd-ac09-ids-{label}"), PRESS);
        make(&pair, "One", &["--discipline", "plates"]);
        let before = state(&pair);
        let tree = run(&pair, &["tree"]);
        tree.code(0);
        let config = read_text(&pair.main, "specengine.toml");
        write(&pair.main, "specengine.toml", with_ids_line(&config, &line));
        for args in [
            vec!["card", "list"],
            vec!["card", "show", "C-0001"],
            vec!["card", "new", "--title", "T", "--discipline", "copy"],
            vec!["card", "move", "C-0001", "--to", "blocked"],
            vec!["card", "edit", "C-0001", "--goal", "G."],
            vec!["card", "assign", "C-0001", "--assignee", "ana"],
            vec!["card", "cancel", "C-0001", "--reason", "No."],
        ] {
            let message = cannot(&pair, &args);
            assert!(message.contains("`C`"), "{label}: {args:?}: {message}");
            assert_eq!(state(&pair), before, "{label}: {args:?}");
        }
        let accept = pair.spec_pty(
            &pair.main,
            &["card", "accept", "C-0001", "--as", "b", "--briefed"],
            None,
        );
        assert_eq!(accept.code, 2, "{label}: {}", accept.output);
        assert!(!accept.asked, "{label}: {}", accept.output);
        run(&pair, &["tree"]).code(0);
        run(&pair, &["task", "list"]).code(0);
        run(&pair, &["search", "masthead"]).code(0);
    }
}

/// AC-09: `spec card` needs no git: a root with no `.git` writes and reads,
/// also through `--root` from elsewhere, and `--target` resolves there.
#[test]
fn ac09_a_root_with_no_git_works() {
    let scratch = Scratch::new("cd-ac09-nogit");
    let home = scratch.home("h");
    let root = scratch.copy(PRESS, "copy");
    let outside = scratch.dir("outside");
    assert!(!root.join(".git").exists());
    let at = |cwd: &Path, args: &[&str]| {
        let done = common::spec(&home, cwd, args);
        done.code(0);
        done.stdout
    };
    assert_eq!(at(&root, &["card", "list"]), PRESS_BOARD);
    assert_eq!(
        at(
            &root,
            &[
                "card",
                "new",
                "--title",
                "No git",
                "--discipline",
                "copy",
                "--target",
                "SHT-FRONT"
            ]
        ),
        "created C-0001: copy/draft\n"
    );
    let text = root.to_str().unwrap();
    assert_eq!(
        at(
            &outside,
            &[
                "--root", text, "card", "move", "C-0001", "--to", "edit", "--as", "desk"
            ]
        ),
        "moved C-0001: copy/edit\n"
    );
    assert_eq!(
        at(&root.join("docs"), &["card", "list", "--column", "edit"])
            .lines()
            .nth(2),
        Some("C-0001 | copy/edit | 1 | No git | copy | - | - | SHT-FRONT")
    );
    assert!(!root.join(".git").exists(), "no repository made");
}

/// AC-09: `--due 2001-01-01` stays raw and no read derives anything of it
/// (the key sets are pinned, so no `overdue`); a bad date exits 1.
#[test]
fn ac09_due_is_raw_with_no_derived_key() {
    let pair = Pair::new("cd-ac09-due", PRESS);
    let id = make(
        &pair,
        "Late",
        &["--discipline", "copy", "--due", "2001-01-01"],
    );
    let shown = json_run(&pair, &["card", "show", &id]);
    shown.code(0);
    assert_eq!(task_common::keys(&shown.stdout), SHOW_KEYS);
    assert_eq!(shown.json()["due"], "2001-01-01");
    let listed_run = json_run(&pair, &["card", "list"]);
    listed_run.code(0);
    let row = match Ordered::of(&listed_run.stdout).get("cards") {
        Ordered::Array(rows) => rows[0].keys(),
        other => panic!("{other:?}"),
    };
    assert_eq!(row, ROW_KEYS);
    assert_eq!(listed_run.json()["cards"][0]["due"], "2001-01-01");
    assert!(
        ok(&pair, &["card", "list"]).contains(" | copy | - | 2001-01-01 | -\n"),
        "the line's due"
    );
    let before = state(&pair);
    for bad in ["2026-02-30", "2026-13-01", "soon", "2026-1-1", ""] {
        let reason = refused(
            &pair,
            &[
                "card",
                "new",
                "--title",
                "T",
                "--discipline",
                "copy",
                "--due",
                bad,
            ],
        );
        assert!(reason.contains("due"), "{bad}: {reason}");
        refused(&pair, &["card", "edit", &id, "--due", bad]);
        assert_eq!(state(&pair), before, "{bad}");
    }
    ok(&pair, &["card", "edit", &id, "--due", "-"]);
    assert_eq!(show(&pair, &id)["due"], Value::Null, "`-` clears");
}

/// AC-09: evidence: the four kinds taken, each `{kind, value, by, at}`;
/// `file=x`, a `data:` URL and the rest of "Data" exit 1, nothing stored.
#[test]
fn ac09_evidence_takes_four_kinds_and_no_binary() {
    let pair = Pair::new("cd-ac09-evidence", PRESS);
    let id = make(&pair, "With proof", &["--discipline", "copy"]);
    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--evidence",
            "url=https://press.example.invalid/a",
            "--evidence",
            "url=http://press.example.invalid/b",
            "--evidence",
            "path=docs/spec/sht-front.md@0123456789abcdef0123456789abcdef01234567",
            "--evidence",
            "build=run-41",
            "--evidence",
            "note=Read twice.",
            "--as",
            "ana",
        ],
    );
    let card = show(&pair, &id);
    let evidence = card["evidence"].as_array().expect("evidence");
    assert_eq!(evidence.len(), 5);
    assert_eq!(evidence[0]["kind"], "url");
    assert_eq!(evidence[0]["value"], "https://press.example.invalid/a");
    assert_eq!(evidence[2]["kind"], "path");
    assert_eq!(evidence[4]["value"], "Read twice.");
    assert_eq!(evidence[4]["by"], "ana");
    assert!(evidence[4]["at"].is_string(), "{card}");
    assert_eq!(events(&pair).len(), 2, "five in one write: one event");

    // A second write appends.
    ok(&pair, &["card", "edit", &id, "--evidence", "note=Again."]);
    assert_eq!(
        show(&pair, &id)["evidence"].as_array().map(Vec::len),
        Some(6)
    );

    let before = state(&pair);
    let two_lines = "note=one\ntwo";
    let long = format!("note={}", "n".repeat(1025));
    for bad in [
        "file=x",
        "url=data:text/plain;base64,aGk=",
        "url=ftp://press.example.invalid/a",
        "url=press.example.invalid",
        "path=docs/spec/sht-front.md",
        "path=docs/spec/sht-front.md@nothex",
        "note=",
        "bare",
        two_lines,
        long.as_str(),
    ] {
        refused(&pair, &["card", "edit", &id, "--evidence", bad]);
        assert_eq!(state(&pair), before, "{bad:?}");
    }
    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--evidence",
            &format!("note={}", "n".repeat(1024)),
        ],
    );

    // At most 32 entries.
    for index in 0..25 {
        ok(
            &pair,
            &[
                "card",
                "edit",
                &id,
                "--evidence",
                &format!("build=b{index}"),
            ],
        );
    }
    assert_eq!(
        show(&pair, &id)["evidence"].as_array().map(Vec::len),
        Some(32)
    );
    let before = state(&pair);
    refused(&pair, &["card", "edit", &id, "--evidence", "build=b33"]);
    assert_eq!(state(&pair), before);
}

/// "Data", Fields: caps as tasks', a required title, `estimate`, `parent`,
/// `corrects`; `-` clears, `--target` and `--criterion` replace.
#[test]
fn fields_keep_their_caps_and_edit_replaces_or_clears() {
    let pair = Pair::new("cd-fields", PRESS);
    let copy = ["--discipline", "copy"];
    let id = make(&pair, &"t".repeat(256), &copy);
    make(
        &pair,
        "Second",
        &[&copy[..], &["--goal", &"g".repeat(4096)]].concat(),
    );
    let before = state(&pair);
    let long_title = "t".repeat(257);
    let long_goal = "g".repeat(4097);
    let long_criterion = "c".repeat(1025);
    for flags in [
        vec!["--title", long_title.as_str()],
        vec!["--title", ""],
        vec!["--title", "T", "--goal", long_goal.as_str()],
        // `-` clears, and a new card has nothing to clear.
        vec!["--title", "T", "--assignee", "-"],
        vec!["--title", "T", "--criterion", long_criterion.as_str()],
        vec!["--title", "T", "--estimate", "3d"],
        vec!["--title", "T", "--estimate", "5-2"],
        vec!["--title", "T", "--parent", "C-0099"],
        vec!["--title", "T", "--corrects", "C-0099"],
        vec!["--title", "T", "--target", "SHT-NOPE"],
        // As `task new --nodes`: a node named twice is refused.
        vec![
            "--title",
            "T",
            "--target",
            "SHT-BACK",
            "--target",
            "docs/spec/sht-back.md",
        ],
    ] {
        let mut args = vec!["card", "new", "--discipline", "copy"];
        args.extend(flags);
        refused(&pair, &args);
        assert_eq!(state(&pair), before, "{args:?}");
    }
    // A control character is stored, as a task's title is, and never reaches
    // a terminal: every line escapes it.
    let raw = make(&pair, "a\u{1b}[31mb", &["--discipline", "plates"]);
    assert_eq!(show(&pair, &raw)["title"], "a\u{1b}[31mb");
    let printed = ok(&pair, &["card", "list"]);
    assert!(!printed.contains('\u{1b}'), "{printed:?}");
    assert!(!ok(&pair, &["card", "show", &raw]).contains('\u{1b}'));
    ok(&pair, &["card", "cancel", &raw, "--reason", "Out."]);
    let before = state(&pair);

    // 33 criteria.
    let mut args = vec!["card", "new", "--discipline", "copy", "--title", "T"];
    for _ in 0..33 {
        args.extend(["--criterion", "Same."]);
    }
    refused(&pair, &args);
    assert_eq!(state(&pair), before);
    // A title is required.
    cannot(&pair, &["card", "new", "--discipline", "copy"]);
    refused(&pair, &["card", "edit", &id, "--title", "-"]);
    // A look-alike target is the intake's exit 2.
    let alike = cannot(&pair, &["card", "edit", &id, "--target", "SHT-\u{0421}OPE"]);
    assert!(
        alike.contains("SHT-COPE") || alike.contains("--target"),
        "{alike}"
    );
    assert_eq!(state(&pair), before);

    // `--discipline`, `--assignee` are no flags of `edit`.
    cannot(&pair, &["card", "edit", &id, "--discipline", "plates"]);
    cannot(&pair, &["card", "edit", &id, "--assignee", "ana"]);

    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--title",
            "Short",
            "--target",
            "SHT-FRONT",
            "--target",
            "SHT-BACK",
            "--criterion",
            "One.",
            "--criterion",
            "Two.",
            "--estimate",
            "4",
        ],
    );
    let card = show(&pair, &id);
    assert_eq!(card["targets"], json!(["SHT-FRONT", "SHT-BACK"]));
    assert_eq!(card["criteria"], json!(["One.", "Two."]));
    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--target",
            "SHT-BACK",
            "--criterion",
            "Three.",
        ],
    );
    let card = show(&pair, &id);
    assert_eq!(card["targets"], json!(["SHT-BACK"]), "replaced");
    assert_eq!(card["criteria"], json!(["Three."]), "replaced");
    assert_eq!(
        events(&pair).last().map(|event| event.1["fields"].clone()),
        Some(json!(["targets", "criteria"]))
    );
    // docs/features/card-trace.md: `-` changes a held estimate too: beside
    // other fields exit 2, alone with its reason.
    let held = state(&pair);
    cannot(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--goal",
            "-",
            "--target",
            "-",
            "--criterion",
            "-",
            "--estimate",
            "-",
        ],
    );
    assert_eq!(state(&pair), held, "`--estimate -` beside other fields");
    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--goal",
            "-",
            "--target",
            "-",
            "--criterion",
            "-",
        ],
    );
    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--estimate",
            "-",
            "--reason",
            "Dropped.",
        ],
    );
    let card = show(&pair, &id);
    assert_eq!(card["goal"], Value::Null);
    assert_eq!(card["targets"], json!([]));
    assert_eq!(card["criteria"], json!([]));
    assert_eq!(card["estimate"], Value::Null);

    ok(
        &pair,
        &[
            "card",
            "assign",
            &id,
            "--assignee",
            "ana",
            "--reviewer",
            "ben",
        ],
    );
    assert_eq!(
        ok(&pair, &["card", "assign", &id, "--assignee", "-"]),
        "assigned C-0001: -, reviewer ben\n"
    );
    let card = show(&pair, &id);
    assert_eq!(card["assignee"], Value::Null);
    assert_eq!(card["reviewer"], "ben");
}

/// "Data", Fields: `parent` names an existing card and no cycle.
#[test]
fn a_parent_makes_no_cycle() {
    let pair = Pair::new("cd-parent", PRESS);
    let copy = ["--discipline", "copy"];
    let first = make(&pair, "One", &copy);
    let second = make(&pair, "Two", &[&copy[..], &["--parent", &first]].concat());
    let third = make(
        &pair,
        "Three",
        &[&copy[..], &["--parent", &second]].concat(),
    );
    let before = state(&pair);
    refused(&pair, &["card", "edit", &first, "--parent", &third]);
    refused(&pair, &["card", "edit", &first, "--parent", &first]);
    refused(&pair, &["card", "edit", &second, "--parent", &third]);
    let reason = refused(&pair, &["card", "edit", &first, "--corrects", &first]);
    assert!(reason.contains("corrects"), "{reason}");
    assert_eq!(state(&pair), before);
    ok(&pair, &["card", "edit", &third, "--parent", "-"]);
    assert_eq!(show(&pair, &third)["parent"], Value::Null);
    ok(&pair, &["card", "edit", &first, "--parent", &third]);
    assert_eq!(show(&pair, &first)["parent"], third.as_str());
}

/// "Data", IDs: `C-0001` first, the highest plus one, past `C-9999`, never
/// reused.
#[test]
fn ids_count_up_past_four_digits() {
    let pair = Pair::new("cd-ids", PRESS);
    let copy = ["--discipline", "copy"];
    assert_eq!(make(&pair, "One", &copy), "C-0001");
    assert_eq!(make(&pair, "Two", &copy), "C-0002");
    ok(&pair, &["card", "cancel", "C-0002", "--reason", "No."]);
    assert_eq!(
        make(&pair, "Three", &copy),
        "C-0003",
        "a cancelled ID stays taken"
    );
    pair.sql("UPDATE cards SET id = 'C-9999' WHERE id = 'C-0003';");
    assert_eq!(make(&pair, "Four", &copy), "C-10000");
    assert_eq!(make(&pair, "Five", &copy), "C-10001");
    assert_eq!(show(&pair, "C-10001")["title"], "Five");
    // The written forms a card's ID is not.
    for wrong in ["C-1", "C-00001", "c-0001", "T-0001"] {
        let done = run(&pair, &["card", "show", wrong]);
        assert!(done.code == 1 || done.code == 2, "{wrong}: {}", done.show());
    }
}

// --------------------------------------------------------------- reads

/// "CLI": the board's order (by flow, column in config order, position),
/// the line of "CLI", the filters ANDed and exact, `--no-target`.
#[test]
fn list_orders_and_filters() {
    let pair = Pair::new("cd-list", PRESS);
    make(
        &pair,
        "Plate one",
        &["--discipline", "plates", "--assignee", "ana"],
    );
    make(
        &pair,
        "Lead",
        &[
            "--discipline",
            "copy",
            "--target",
            "SHT-FRONT",
            "--target",
            "SHT-BACK",
        ],
    );
    make(
        &pair,
        "Second",
        &[
            "--discipline",
            "copy",
            "--assignee",
            "ana",
            "--due",
            "2026-11-05",
        ],
    );
    ok(
        &pair,
        &["card", "move", "C-0003", "--to", "edit", "--as", "desk"],
    );
    make(&pair, "Plate two", &["--discipline", "plates"]);
    assert_eq!(
        ok(&pair, &["card", "list"]),
        "flow copy | draft 1 | edit 1 | proof 0 | filed 0\n\
         flow plates | queued 2 | blocked 0 | checked 0 | struck 0\n\
         C-0002 | copy/draft | 1 | Lead | copy | - | - | SHT-FRONT, SHT-BACK\n\
         C-0003 | copy/edit | 1 | Second | copy | ana | 2026-11-05 | -\n\
         C-0001 | plates/queued | 1 | Plate one | plates | ana | - | -\n\
         C-0004 | plates/queued | 2 | Plate two | plates | - | - | -\n"
    );
    assert_eq!(listed(&pair, &[]), ["C-0002", "C-0003", "C-0001", "C-0004"]);
    for (args, want) in [
        (vec!["--flow", "plates"], vec!["C-0001", "C-0004"]),
        (vec!["--column", "edit"], vec!["C-0003"]),
        (vec!["--discipline", "copy"], vec!["C-0002", "C-0003"]),
        (vec!["--assignee", "ana"], vec!["C-0003", "C-0001"]),
        (vec!["--no-target"], vec!["C-0003", "C-0001", "C-0004"]),
        (
            vec!["--flow", "plates", "--assignee", "ana"],
            vec!["C-0001"],
        ),
        (vec!["--flow", "copy", "--no-target"], vec!["C-0003"]),
        (vec!["--flow", "pla"], vec![]),
        (vec!["--assignee", "an"], vec![]),
        (vec!["--flow", "copy", "--column", "queued"], vec![]),
    ] {
        assert_eq!(listed(&pair, &args), want, "{args:?}");
        let mut text = vec!["card", "list"];
        text.extend(&args);
        let printed = ok(&pair, &text);
        assert_eq!(
            card_lines(&printed).len(),
            want.len(),
            "{args:?}: {printed}"
        );
    }
}

/// "CLI": `list` keeps within the cap of `spec flow`: the flow lines always
/// print, the rows fit, the tail line counts the rest, JSON holds what is
/// printed, `omitted` and `truncated` agree.
#[test]
fn list_is_capped_with_a_tail() {
    let pair = Pair::new("cd-cap", PRESS);
    let title = "t".repeat(240);
    make(&pair, &title, &["--discipline", "copy"]);
    // 600 rows of one seeded card, written as the store writes them.
    pair.sql(
        "WITH RECURSIVE n(i) AS (SELECT 2 UNION ALL SELECT i + 1 FROM n WHERE i < 600) \
         INSERT INTO cards SELECT printf('C-%04d', i), project, flow, col, i, title, goal, \
         discipline, targets, snapshot, criteria, assignee, reviewer, estimate, due, parent, \
         corrects, evidence, acceptance, cancellation, created_by, created_at, updated_at, \
         revision, tasks, mandate FROM cards, n WHERE id = 'C-0001';",
    );
    let text = ok(&pair, &["card", "list"]);
    let last = text.lines().last().expect("a tail");
    let shown = card_lines(&text).len();
    assert!(shown < 600, "{shown}");
    assert_eq!(
        last,
        format!("[truncated: {} of 600 cards not shown]", 600 - shown)
    );
    assert!(
        text.starts_with("flow copy | draft 600 |"),
        "the whole count"
    );
    let body = text.len() - last.len() - 1;
    assert!(body <= 40_000, "{body} before the tail");
    let document = doc(&pair, &["card", "list"]);
    assert_eq!(document["truncated"], true);
    assert_eq!(document["omitted"], 600 - shown);
    assert_eq!(document["cards"].as_array().map(Vec::len), Some(shown));
    assert_eq!(ok(&pair, &["card", "list"]), text, "one state, one result");
    // A filter brings the rest back.
    let few = ok(&pair, &["card", "list", "--assignee", "nobody"]);
    assert!(!few.contains("[truncated"), "{few}");
}

/// "CLI": `show`'s `key: value` lines, in the document's order, cut at
/// `OUTPUT_CAP_CHARS`; `--history` the last 64 `card.*` events of this card
/// alone, `{seq, type, at, payload}`.
#[test]
fn show_prints_key_lines_and_history() {
    let pair = Pair::new("cd-show", PRESS);
    make(&pair, "One", &["--discipline", "copy"]);
    make(&pair, "Other", &["--discipline", "copy"]);
    let text = ok(&pair, &["card", "show", "C-0001"]);
    let mut at = 0;
    // `notes` go to stderr, as every command's.
    for key in &SHOW_KEYS[..SHOW_KEYS.len() - 1] {
        let Some(found) = text[at..]
            .lines()
            .position(|line| line.starts_with(&format!("{key}:")))
        else {
            panic!("no `{key}:` line after byte {at}:\n{text}");
        };
        at += text[at..]
            .lines()
            .take(found + 1)
            .map(|line| line.len() + 1)
            .sum::<usize>();
    }
    assert!(
        text.starts_with("id: C-0001\nflow: copy\ncolumn: draft\nposition: 1\ntitle: One\n"),
        "{text}"
    );
    assert!(
        !text.lines().any(|line| line.starts_with("notes:")),
        "no `notes:` line: {text}"
    );
    for index in 0..70 {
        ok(
            &pair,
            &[
                "card",
                "edit",
                "C-0001",
                "--goal",
                &format!("Goal {index}."),
            ],
        );
    }
    ok(
        &pair,
        &["card", "edit", "C-0002", "--goal", "Not this one."],
    );
    let document = doc(&pair, &["card", "show", "C-0001", "--history"]);
    let history = document["history"].as_array().expect("history");
    assert_eq!(history.len(), 64, "the last 64 of 71");
    assert_eq!(history[63]["type"], "card.edited");
    assert_eq!(history[63]["payload"]["id"], "C-0001");
    assert!(history[63]["seq"].is_u64(), "{document}");
    assert!(history[63]["at"].is_string(), "{document}");
    assert_eq!(history[63].as_object().map(serde_json::Map::len), Some(4));
    assert!(
        history
            .windows(2)
            .all(|pair| pair[0]["seq"].as_u64() < pair[1]["seq"].as_u64()),
        "in order"
    );
    assert!(
        history
            .iter()
            .all(|event| event["payload"]["id"] == "C-0001")
    );

    // Past the cap: goal, 32 criteria and 32 evidence entries of 1 KB.
    let goal = "g".repeat(4096);
    let criterion = "c".repeat(1024);
    let mut args = vec!["card", "edit", "C-0002", "--goal", goal.as_str()];
    for _ in 0..32 {
        args.extend(["--criterion", criterion.as_str()]);
    }
    let note = format!("note={}", "n".repeat(1024));
    for _ in 0..32 {
        args.extend(["--evidence", note.as_str()]);
    }
    ok(&pair, &args);
    let cut = ok(&pair, &["card", "show", "C-0002"]);
    let chars = cut.chars().count();
    assert!(chars <= 40_100, "{chars}");
    assert!(chars >= 39_000, "the output reaches the cap: {chars}");
}

/// "Rules and edge cases": a config that drops a card's column serves it
/// last with a note, `allowed` the first column; one that drops its flow,
/// `allowed` empty.
#[test]
fn a_card_outside_its_config_is_served_last_with_a_note() {
    let pair = Pair::new("cd-dropped", PRESS);
    make(&pair, "Stuck", &["--discipline", "plates"]);
    make(&pair, "Fine", &["--discipline", "plates"]);
    make(&pair, "Later", &["--discipline", "plates"]);
    ok(&pair, &["card", "move", "C-0001", "--to", "blocked"]);
    ok(&pair, &["card", "move", "C-0003", "--to", "checked"]);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        config.replace(
            "columns = [\"queued\", \"blocked\", \"checked\", \"struck\"]",
            "columns = [\"queued\", \"checked\", \"struck\"]",
        ),
    );
    let document = doc(&pair, &["card", "list"]);
    let ids: Vec<&str> = document["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["C-0002", "C-0003", "C-0001"], "served last");
    assert_eq!(document["cards"][2]["column"], "blocked");
    assert!(
        document["notes"].to_string().contains("C-0001"),
        "{document}"
    );
    let card = show(&pair, "C-0001");
    assert_eq!(card["allowed"].as_array().map(Vec::len), Some(1), "{card}");
    assert_eq!(card["allowed"][0]["to"], "queued");
    assert!(card["notes"].to_string().contains("blocked"), "{card}");
    // In text the note is stderr's; `show` prints no `notes:` line.
    let printed = run(&pair, &["card", "show", "C-0001"]);
    printed.code(0);
    assert!(
        !printed
            .stdout
            .lines()
            .any(|line| line.starts_with("notes:")),
        "{}",
        printed.show()
    );
    assert!(printed.stderr.contains("blocked"), "{}", printed.show());
    assert_eq!(
        ok(&pair, &["card", "move", "C-0001", "--to", "queued"]),
        "moved C-0001: plates/queued\n",
        "the first column takes it back"
    );

    // Its flow gone.
    ok(&pair, &["card", "move", "C-0001", "--to", "checked"]);
    let gone = read_text(&pair.main, "specengine.toml");
    let at = gone.rfind("[[card.flow]]").expect("the second flow");
    write(&pair.main, "specengine.toml", &gone[..at]);
    let card = show(&pair, "C-0001");
    assert_eq!(card["allowed"], json!([]), "{card}");
    assert!(card["notes"].to_string().contains("plates"), "{card}");
    let document = doc(&pair, &["card", "list"]);
    assert_eq!(document["flows"].as_array().map(Vec::len), Some(1));
    assert!(
        !document["notes"].as_array().expect("notes").is_empty(),
        "{document}"
    );
    let before = state(&pair);
    refused(&pair, &["card", "move", "C-0001", "--to", "queued"]);
    assert_eq!(state(&pair), before);
}

/// "Rules and edge cases": `list` gives one note per (flow, column) `[card]`
/// no longer places, with its count and the first card by position, never one
/// per card; a gone flow likewise. M: a stranded note per card.
#[test]
fn a_stranded_column_is_one_note_with_a_count() {
    let pair = Pair::new("cd-stranded", PRESS);
    for title in ["A", "B", "C", "D", "E"] {
        make(&pair, title, &["--discipline", "plates"]);
    }
    for id in ["C-0001", "C-0002", "C-0003"] {
        ok(&pair, &["card", "move", id, "--to", "blocked"]);
    }
    ok(&pair, &["card", "move", "C-0004", "--to", "checked"]);
    let base = read_text(&pair.main, "specengine.toml");
    let columns = "columns = [\"queued\", \"blocked\", \"checked\", \"struck\"]";
    assert_eq!(base.matches(columns).count(), 1);
    let accept = "accept = { from = \"checked\", to = \"struck\" }";
    assert_eq!(base.matches(accept).count(), 1);
    write(
        &pair.main,
        "specengine.toml",
        base.replace(columns, "columns = [\"queued\", \"struck\"]")
            .replace(accept, "accept = { from = \"queued\", to = \"struck\" }"),
    );
    let column = |name: &str| format!("the column `{name}` is no longer in the flow `plates`");
    assert_eq!(
        doc(&pair, &["card", "list"])["notes"],
        json!([
            format!(
                "3 card(s) in plates/blocked, from C-0001: {}: served last",
                column("blocked")
            ),
            format!(
                "1 card(s) in plates/checked, from C-0004: {}: served last",
                column("checked")
            ),
        ])
    );
    let printed = run(&pair, &["card", "list"]);
    printed.code(0);
    assert_eq!(
        printed.stderr.matches("served last").count(),
        2,
        "{}",
        printed.show()
    );

    // The flow gone: the same one note a pair, by column name: no config
    // orders them now.
    let at = base.rfind("[[card.flow]]").expect("the second flow");
    write(&pair.main, "specengine.toml", &base[..at]);
    let gone = "the flow `plates` is no longer in `[card]`: served last, no move open";
    assert_eq!(
        doc(&pair, &["card", "list"])["notes"],
        json!([
            format!("3 card(s) in plates/blocked, from C-0001: {gone}"),
            format!("1 card(s) in plates/checked, from C-0004: {gone}"),
            format!("1 card(s) in plates/queued, from C-0005: {gone}"),
        ])
    );
}

/// The owner's reading of "served last": last in its flow. A card whose
/// column `copy` dropped comes after `copy`'s placed cards and before
/// `plates`.
#[test]
fn served_last_means_last_in_its_flow() {
    let pair = Pair::new("cd-last", PRESS);
    make(&pair, "Stranded", &["--discipline", "copy"]);
    make(&pair, "Placed", &["--discipline", "copy"]);
    make(&pair, "Proofed", &["--discipline", "copy"]);
    make(&pair, "Plate", &["--discipline", "plates"]);
    ok(
        &pair,
        &["card", "move", "C-0001", "--to", "edit", "--as", "desk"],
    );
    ok(
        &pair,
        &["card", "move", "C-0003", "--to", "edit", "--as", "desk"],
    );
    ok(&pair, &["card", "move", "C-0003", "--to", "proof"]);
    let base = read_text(&pair.main, "specengine.toml");
    let columns = "columns = [\"draft\", \"edit\", \"proof\", \"filed\"]";
    let moves = "  { from = \"draft\", to = \"edit\", by = [\"desk\"] },\n  { from = \"edit\", to \
                 = \"proof\" },\n";
    assert_eq!(base.matches(columns).count(), 1);
    assert_eq!(base.matches(moves).count(), 1);
    write(
        &pair.main,
        "specengine.toml",
        base.replace(columns, "columns = [\"draft\", \"proof\", \"filed\"]")
            .replace(moves, "  { from = \"draft\", to = \"proof\" },\n"),
    );
    assert_eq!(
        listed(&pair, &[]),
        ["C-0002", "C-0003", "C-0001", "C-0004"],
        "after its flow's placed cards, before the next flow"
    );
}

/// "Rules and edge cases": a column holding no UTF-8: `list` a note naming
/// the card and the column, the other cards listed; `show` exit 2 naming the
/// column; a task read unmoved.
#[test]
fn a_column_of_invalid_utf8_is_a_note_and_an_exit_2() {
    for column in ["goal", "title", "assignee", "evidence"] {
        let pair = Pair::new(&format!("cd-utf8-{column}"), PRESS);
        make(&pair, "Bad", &["--discipline", "copy"]);
        make(&pair, "Sound", &["--discipline", "copy"]);
        pair.sql(&format!(
            "UPDATE cards SET {column} = CAST(x'ff' AS TEXT) WHERE id = 'C-0001';"
        ));
        let document = doc(&pair, &["card", "list"]);
        let ids: Vec<&str> = document["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .map(|row| row["id"].as_str().expect("an ID"))
            .collect();
        assert_eq!(ids, ["C-0002"], "{column}: the other card listed");
        let notes = document["notes"].to_string();
        assert!(
            notes.contains("C-0001") && notes.contains(&format!("`{column}`")),
            "{column}: {notes}"
        );
        let printed = run(&pair, &["card", "list"]);
        printed.code(0);
        assert!(printed.stderr.contains("C-0001"), "{}", printed.show());
        let message = cannot(&pair, &["card", "show", "C-0001"]);
        assert!(
            message.contains("C-0001") && message.contains(&format!("`{column}`")),
            "{column}: {message}"
        );
        run(&pair, &["card", "show", "C-0002"]).code(0);
    }
}

/// "Data", Evidence: `path=` takes lower-case hex of any length after the
/// last `@`; capitals, no hex at all and no blob exit 1.
#[test]
fn a_path_blob_is_lower_case_hex_of_any_length() {
    let pair = Pair::new("cd-blob", PRESS);
    let id = make(&pair, "With a blob", &["--discipline", "copy"]);
    let forty = "0123456789abcdef0123456789abcdef01234567";
    for (count, blob) in ["ab", "0123abcd", forty].into_iter().enumerate() {
        let value = format!("path=docs/spec/sht-front.md@{blob}");
        ok(&pair, &["card", "edit", &id, "--evidence", &value]);
        let card = show(&pair, &id);
        assert_eq!(card["evidence"].as_array().map(Vec::len), Some(count + 1));
        assert_eq!(
            card["evidence"][count]["value"],
            format!("docs/spec/sht-front.md@{blob}")
        );
    }
    let before = state(&pair);
    for blob in ["ABCD", "xyz", "abG", ""] {
        let value = format!("path=docs/spec/sht-front.md@{blob}");
        let reason = refused(&pair, &["card", "edit", &id, "--evidence", &value]);
        assert!(reason.contains("evidence"), "{blob:?}: {reason}");
        assert_eq!(state(&pair), before, "{blob:?}");
    }
}

// --------------------------------------------------------------- AC-10

/// One trade's walk: its fixture and flags, its open columns in order
/// (the last `accept.from`), `accept.to`, a node of the fixture.
struct Trade {
    label: &'static str,
    fixture: &'static str,
    discipline: &'static [&'static str],
    flow: &'static str,
    open: &'static [&'static str],
    done: &'static str,
    target: &'static str,
}

const TRADES: [Trade; 5] = [
    Trade {
        label: "copy",
        fixture: PRESS,
        discipline: &["--discipline", "copy"],
        flow: "copy",
        open: &["draft", "edit", "proof"],
        done: "filed",
        target: "SHT-FRONT",
    },
    Trade {
        label: "plates",
        fixture: PRESS,
        discipline: &["--discipline", "plates"],
        flow: "plates",
        open: &["queued", "blocked", "checked"],
        done: "struck",
        target: "SHT-BACK",
    },
    Trade {
        label: "spec-a",
        fixture: "spec-a",
        discipline: &[],
        flow: "plain",
        open: &["backlog", "ready", "in-progress", "review"],
        done: "done",
        target: "MEC-STAMINA",
    },
    Trade {
        label: "spec-b",
        fixture: "spec-b",
        discipline: &[],
        flow: "plain",
        open: &["backlog", "ready", "in-progress", "review"],
        done: "done",
        target: "REQ-001",
    },
    Trade {
        label: "spec-c",
        fixture: "spec-c",
        discipline: &[],
        flow: "plain",
        open: &["backlog", "ready", "in-progress", "review"],
        done: "done",
        target: "FEE-RIDE",
    },
];

/// AC-10: one code runs five projects' flows: the same walk, the same
/// lines, one event a step, the repository untouched, on two trades of a
/// print shop and three projects with no `[card]`.
#[test]
fn ac10_five_flows_run_one_code() {
    for trade in &TRADES {
        let pair = Pair::new(&format!("cd-ac10-{}", trade.label), trade.fixture);
        let side = git_side(&pair);
        let mut args = vec![
            "card",
            "new",
            "--title",
            "One walk",
            "--target",
            trade.target,
        ];
        args.extend_from_slice(trade.discipline);
        assert_eq!(
            ok(&pair, &args),
            format!("created C-0001: {}/{}\n", trade.flow, trade.open[0]),
            "{}",
            trade.label
        );
        ok(&pair, &["card", "assign", "C-0001", "--reviewer", "ben"]);
        for column in &trade.open[1..] {
            assert_eq!(
                ok(
                    &pair,
                    &["card", "move", "C-0001", "--to", column, "--as", "desk"]
                ),
                format!("moved C-0001: {}/{column}\n", trade.flow),
                "{}",
                trade.label
            );
        }
        let accepted = pair.spec_pty(
            &pair.main,
            &["card", "accept", "C-0001", "--as", "ben", "--briefed"],
            Some("y"),
        );
        assert_eq!(accepted.code, 0, "{}: {}", trade.label, accepted.output);
        assert!(
            accepted
                .output
                .contains(&format!("accepted C-0001: {}/{}\n", trade.flow, trade.done)),
            "{}: {}",
            trade.label,
            accepted.output
        );
        assert_eq!(
            events(&pair).len(),
            2 + trade.open.len(),
            "{}: new, assign, the moves, accept",
            trade.label
        );
        assert_eq!(git_side(&pair), side, "{}", trade.label);
        let card = show(&pair, "C-0001");
        assert_eq!(card["targets"], json!([trade.target]), "{}", trade.label);
        assert_eq!(card["column"], trade.done, "{}", trade.label);
    }
}

/// The card sources of "Files in scope".
const CARD_SOURCES: [&str; 8] = [
    "crates/specengine-model/src/card.rs",
    "crates/specengine-core/src/card.rs",
    "crates/specengine-core/src/card_toml.rs",
    "crates/specengine-store/src/queue/cards.rs",
    "crates/specengine-cli/src/card.rs",
    "crates/specengine-mcp/src/cards.rs",
    // docs/features/card-trace.md: a mark, a rollup, a clear.
    "crates/specengine-core/src/trace.rs",
    "crates/specengine-cli/src/trace.rs",
];

/// `text` up to its first `#[cfg(test)]` module.
fn before_tests(text: &str) -> &str {
    text.find("#[cfg(test)]").map_or(text, |at| &text[..at])
}

/// Whether `word` stands in `text` as a whole word.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let edge = |ch: Option<char>| !ch.is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        edge(text[..at].chars().next_back()) && edge(text[at + word.len()..].chars().next())
    })
}

/// The text of `PLAIN_FLOW` cut out of core's card source (a raw string).
fn outside_plain_flow(text: &str) -> String {
    let Some(start) = text.find("const PLAIN_FLOW") else {
        return text.to_owned();
    };
    let end = text[start..]
        .find("\";\n")
        .or_else(|| text[start..].find("\"#;\n"))
        .map_or(text.len(), |at| start + at);
    format!("{}{}", &text[..start], &text[end..])
}

/// AC-10, AC-05: no fixture word and no `blocked` in the card sources,
/// comments too; the five columns of the default stand in `PLAIN_FLOW`
/// alone. M: a branch on a column word.
#[test]
fn ac10_the_card_sources_name_no_word_of_a_project() {
    let root = repository_root();
    let mut found = Vec::new();
    for source in CARD_SOURCES {
        let path = root.join(source);
        let whole = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(whole.len() > 500, "{source} is read whole");
        let text = before_tests(&whole);
        for word in [
            "blocked",
            "plates",
            "struck",
            "queued",
            "filed",
            "desk",
            "card-press",
        ] {
            for (number, line) in text.lines().enumerate() {
                if has_word(line, word) {
                    found.push(format!("{source}:{}: {word}", number + 1));
                }
            }
        }
        let code = outside_plain_flow(text);
        for literal in [
            "\"copy\"",
            "\"draft\"",
            "\"edit\"",
            "\"proof\"",
            "\"checked\"",
            "\"backlog\"",
            "\"ready\"",
            "\"in-progress\"",
            "\"review\"",
            "\"done\"",
        ] {
            // `edit` is a command word of the CLI's and the tools' own,
            // never a column: a comparison with a column is what counts.
            for (number, line) in code.lines().enumerate() {
                let compared = line.contains(&format!("== {literal}"))
                    || line.contains(&format!("{literal} =>"))
                    || !matches!(literal, "\"edit\"" | "\"copy\"") && line.contains(literal);
                if compared && !line.trim_start().starts_with("//") {
                    found.push(format!("{source}:{}: {literal}", number + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "project words in the card sources:\n{}",
        found.join("\n")
    );
    // The scan sees what it must refuse.
    assert!(has_word("if column == \"blocked\" {", "blocked"));
    assert!(!has_word("an unblocked path", "blocked"));
}

/// AC-05's M: a branch on a word means a renamed column changes behaviour;
/// the same walk under other words runs alike.
#[test]
fn the_words_of_a_flow_mean_nothing_to_the_engine() {
    let pair = Pair::new("cd-words", "spec-a");
    append_config(
        &pair.main,
        "\n[[card.flow]]\nname = \"done\"\ncolumns = [\"review\", \"ready\", \"done\", \
         \"backlog\"]\naccept = { from = \"done\", to = \"backlog\" }\n",
    );
    make(&pair, "Upside down", &[]);
    assert_eq!(
        show(&pair, "C-0001")["column"],
        "review",
        "the first column"
    );
    ok(&pair, &["card", "assign", "C-0001", "--reviewer", "ben"]);
    ok(&pair, &["card", "move", "C-0001", "--to", "done"]);
    let accepted = pair.spec_pty(
        &pair.main,
        &["card", "accept", "C-0001", "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert_eq!(show(&pair, "C-0001")["column"], "backlog");
}

// --------------------------------------------------------------- AC-11

/// AC-11: `cancel` needs `--reason`; it takes the card out of its column
/// (`position` 0), the column closes up, one `card.cancelled`, the reason
/// kept; only `--with-cancelled` lists it, last in its column, ending
/// ` | cancelled`; every later write exits 1 naming `--corrects`.
#[test]
fn ac11_cancel_takes_a_card_out_and_freezes_it() {
    let pair = Pair::new("cd-ac11", PRESS);
    for title in ["A", "B", "C"] {
        make(&pair, title, &["--discipline", "plates"]);
    }
    let before = state(&pair);
    let missing = cannot(&pair, &["card", "cancel", "C-0002"]);
    assert!(missing.contains("--reason"), "{missing}");
    let long = "r".repeat(4097);
    refused(&pair, &["card", "cancel", "C-0002", "--reason", ""]);
    refused(&pair, &["card", "cancel", "C-0002", "--reason", &long]);
    assert_eq!(state(&pair), before, "a note's caps");

    assert_eq!(
        ok(
            &pair,
            &[
                "card", "cancel", "C-0002", "--reason", "Dropped.", "--as", "desk"
            ]
        ),
        "cancelled C-0002: plates/queued\n"
    );
    let seen = events(&pair);
    assert_eq!(seen.len(), 4, "{seen:?}");
    assert_eq!(
        seen.last(),
        Some(&(
            "card.cancelled".to_owned(),
            json!({"id": "C-0002", "column": "queued", "as": "desk"})
        ))
    );
    let card = show(&pair, "C-0002");
    assert_eq!(card["position"], 0);
    assert_eq!(card["cancellation"]["by"], "desk");
    assert_eq!(card["cancellation"]["reason"], "Dropped.");
    assert!(card["cancellation"]["at"].is_string(), "{card}");
    assert_eq!(place(&pair, "C-0001").0, 1);
    assert_eq!(place(&pair, "C-0003").0, 2, "the column closes up");
    assert_eq!(place(&pair, "C-0003").1, "1", "no revision for a neighbour");

    assert_eq!(listed(&pair, &[]), ["C-0001", "C-0003"]);
    let text = ok(&pair, &["card", "list"]);
    assert!(!text.contains("C-0002"), "{text}");
    assert!(
        text.starts_with(
            "flow copy | draft 0 | edit 0 | proof 0 | filed 0\nflow plates | queued 2 |"
        ),
        "{text}"
    );
    let with = ok(&pair, &["card", "list", "--with-cancelled"]);
    assert_eq!(
        card_lines(&with),
        [
            "C-0001 | plates/queued | 1 | A | plates | - | - | -",
            "C-0003 | plates/queued | 2 | C | plates | - | - | -",
            "C-0002 | plates/queued | 0 | B | plates | - | - | - | cancelled",
        ]
    );
    let document = doc(&pair, &["card", "list", "--with-cancelled"]);
    assert_eq!(document["cards"][2]["id"], "C-0002");
    assert_eq!(document["cards"][2]["cancelled"], true);
    assert_eq!(document["cards"][0]["cancelled"], false);
    assert_eq!(document["cards"][0]["accepted"], false);

    let before = state(&pair);
    for args in writes("C-0002") {
        let reason = refused(&pair, &args);
        assert!(reason.contains("--corrects"), "{args:?}: {reason}");
        assert_eq!(state(&pair), before, "{args:?}");
    }
    let tried = pair.spec_pty(
        &pair.main,
        &["card", "accept", "C-0002", "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(tried.code, 1, "{}", tried.output);
    assert!(tried.output.contains("--corrects"), "{}", tried.output);
    assert_eq!(state(&pair), before);

    // `--before` a cancelled card: it stands in no column.
    refused(&pair, &["card", "move", "C-0003", "--before", "C-0002"]);
    assert_eq!(state(&pair), before);
}

// ---------------------------------------------------------- determinism

/// One state, one result: every read twice, byte for byte, and the same
/// reads in another project of one `HOME` see nothing of this one.
#[test]
fn reads_repeat_and_stay_in_their_project() {
    let scratch = Scratch::new("cd-two");
    let home = scratch.home("h");
    let press = scratch.copy(PRESS, "press");
    let game = scratch.copy("spec-a", "game");
    let at = |cwd: &Path, args: &[&str]| {
        let done = common::spec(&home, cwd, args);
        done.code(0);
        done.stdout
    };
    at(
        &press,
        &["card", "new", "--title", "One", "--discipline", "copy"],
    );
    assert_eq!(at(&game, &["card", "list"]), PLAIN_BOARD);
    assert_eq!(
        at(&game, &["card", "new", "--title", "Two"]),
        "created C-0001: plain/backlog\n",
        "IDs per database"
    );
    for args in [
        vec!["card", "list"],
        vec!["--json", "card", "list"],
        vec!["card", "show", "C-0001"],
        vec!["--json", "card", "show", "C-0001", "--history"],
    ] {
        assert_eq!(at(&press, &args), at(&press, &args), "{args:?}");
    }
    assert!(at(&press, &["card", "show", "C-0001"]).contains("title: One"));
    assert!(at(&game, &["card", "show", "C-0001"]).contains("title: Two"));
}

/// The key sets stand for every shape of `allowed`: each `{to, by}` with
/// `by` naming who may.
#[test]
fn allowed_names_the_columns_and_who_moves() {
    let pair = Pair::new("cd-allowed", PRESS);
    make(&pair, "One", &["--discipline", "copy"]);
    let card = show(&pair, "C-0001");
    let allowed = card["allowed"].as_array().expect("allowed");
    assert_eq!(allowed[0]["to"], "edit", "{card}");
    assert_eq!(allowed[0]["by"], json!(["desk"]), "{card}");
    let names: BTreeSet<&str> = allowed
        .iter()
        .map(|entry| entry["to"].as_str().expect("a column"))
        .collect();
    assert!(names.contains("edit") && !names.contains("proof"), "{card}");
    assert!(
        allowed
            .iter()
            .all(|entry| entry.as_object().map(serde_json::Map::len) == Some(2))
    );

    // No `by`: `[]`, anyone. `allowed` holds moves only, never `accept.to`.
    ok(
        &pair,
        &["card", "move", "C-0001", "--to", "edit", "--as", "desk"],
    );
    assert_eq!(
        show(&pair, "C-0001")["allowed"],
        json!([{"to": "proof", "by": []}])
    );
    ok(&pair, &["card", "move", "C-0001", "--to", "proof"]);
    assert_eq!(
        show(&pair, "C-0001")["allowed"],
        json!([]),
        "only accept is left"
    );
    let plate = make(&pair, "Plate", &["--discipline", "plates"]);
    ok(&pair, &["card", "move", &plate, "--to", "checked"]);
    assert_eq!(
        doc(&pair, &["card", "move", &plate, "--to", "checked"])["allowed"],
        json!([{"to": "queued", "by": []}, {"to": "blocked", "by": []}]),
        "the write's document too"
    );
}

// --------------------------------------------- card-questions (RM-0097), K1

/// docs/features/card-questions.md K1: an item of `show`'s `items`, its
/// keys in order (key set 116).
const ITEM_KEYS: [&str; 16] = [
    "id",
    "kind",
    "status",
    "summary",
    "working_answer",
    "why",
    "author",
    "created_at",
    "updated_at",
    "staged_at",
    "decided_by",
    "decided_at",
    "decision_note",
    "record_id",
    "record_path",
    "record_title",
];

/// spec-a's decision of kind-decision.md "Data", on `RULE-STAM-REGEN`.
const CQ_TEXT: &str = "Does regeneration wait for rest?";
const CQ_WORKING: &str = "Yes: it starts 1.5 s after the last stamina use.";
const CQ_WHY: &str = "Two playtests read instant regeneration as a bug.";

/// A JSON object's keys, in the order written.
struct OrderedKeys(Vec<String>);

impl<'de> serde::Deserialize<'de> for OrderedKeys {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> serde::de::Visitor<'de> for Seen {
            type Value = OrderedKeys;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<OrderedKeys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, serde::de::IgnoredAny)) =
                    map.next_entry::<String, serde::de::IgnoredAny>()?
                {
                    keys.push(key);
                }
                Ok(OrderedKeys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

/// The top-level keys of the JSON object `text`, in order.
fn top_keys(text: &str) -> Vec<String> {
    serde_json::from_str::<OrderedKeys>(text)
        .unwrap_or_else(|error| panic!("{error}: {text}"))
        .0
}

/// The raw text of the array under the top-level `key` of `text` (strings
/// and escapes skipped while matching brackets).
fn raw_array<'t>(text: &'t str, key: &str) -> &'t str {
    let start = text
        .find(&format!("\"{key}\":["))
        .unwrap_or_else(|| panic!("no `{key}` array: {text}"))
        + key.len()
        + 3;
    let bytes = text.as_bytes();
    let (mut depth, mut inside, mut escaped) = (0_i32, false, false);
    for (at, byte) in bytes.iter().enumerate().skip(start) {
        if inside {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => inside = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => inside = true,
            b'[' | b'{' => depth += 1,
            b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[start..=at];
                }
            }
            _ => {}
        }
    }
    panic!("an unclosed `{key}`: {text}")
}

/// `spec --json propose question <targets> …` in `cwd`, a developer's,
/// `extra` last; stored: its ID.
fn cq_ask(pair: &Pair, cwd: &Path, targets: &[&str], text: &str, extra: &[&str]) -> String {
    let mut args = vec!["--json", "propose", "question"];
    args.extend_from_slice(targets);
    args.extend_from_slice(&[
        "--text",
        text,
        "--working-answer",
        "The harbour bell, as in chapter one.",
        "--price-of-other",
        "Two scenes rewritten.",
        "--author-role",
        "developer",
    ]);
    args.extend_from_slice(extra);
    let done = pair.spec_piped(cwd, &args, b"");
    done.code(0);
    let document = done.json();
    assert_eq!(document["created"], json!(true), "not stored: {document}");
    document["id"].as_str().expect("an ID").to_owned()
}

/// `spec --json propose decision --input -` of `document` in `cwd`,
/// `extra` last; stored: its ID.
fn cq_decide(pair: &Pair, cwd: &Path, document: &Value, extra: &[&str]) -> String {
    let mut args = vec![
        "--json",
        "propose",
        "decision",
        "--input",
        "-",
        "--author-role",
        "developer",
    ];
    args.extend_from_slice(extra);
    let done = pair.spec_piped(cwd, &args, document.to_string().as_bytes());
    done.code(0);
    let answer = done.json();
    assert_eq!(answer["created"], json!(true), "not stored: {answer}");
    answer["id"].as_str().expect("an ID").to_owned()
}

/// The library's stage (`fields`, `null` actor) or unstage of `id`.
fn cq_stage(pair: &Pair, id: &str, fields: Option<&str>) {
    let env = pair.env(&pair.main);
    let git = pair.git_env(&pair.main);
    let outcome = match fields {
        Some(fields) => {
            let updated_at = pair.proposal(id).updated_at;
            let body = format!("{{{fields},\"updated_at\":\"{updated_at}\"}}");
            specengine_cli::stage(
                &env,
                &Globals::default(),
                &specengine_cli::StageRequest {
                    id: id.to_owned(),
                    body: specengine_cli::StageBody::from_json(body.as_bytes())
                        .unwrap_or_else(|error| panic!("{body}: {error}")),
                    now: CLOCK.to_owned(),
                    git,
                    actor: None,
                },
            )
        }
        None => specengine_cli::unstage(
            &env,
            &Globals::default(),
            &specengine_cli::UnstageRequest {
                id: id.to_owned(),
                now: CLOCK.to_owned(),
                git,
                actor: None,
            },
        ),
    }
    .unwrap_or_else(|error| panic!("stage {id}: {error}"));
    assert_eq!(
        outcome.exit(),
        specengine_cli::Exit::Answered,
        "{outcome:?}"
    );
}

/// K1 on spec-a: `show --json` serves `items` right after `tasks`, each
/// item set 116's keys in order, by ID number: an applied decision with its
/// `why` and `record_*`, a rejected question with its `decision_note`, an
/// open question; another card's `items` is `[]`; from a root in no git
/// worktree it reads the same; the text's `items:` lines ("Data").
#[test]
fn card_questions_k1_show_serves_items_after_tasks() {
    let pair = Pair::new("cards-cq-items", "spec-a");
    let cwd = pair.linked.clone();
    assert_eq!(
        make(&pair, "Rest regeneration", &["--target", "RULE-STAM-REGEN"]),
        "C-0001"
    );
    assert_eq!(make(&pair, "No items", &[]), "C-0002");
    let decision = cq_decide(
        &pair,
        &cwd,
        &json!({"node_ids": ["RULE-STAM-REGEN"], "text": CQ_TEXT, "working_answer": CQ_WORKING,
            "price_of_other": "Without the wait R-28 is rebalanced.", "why": CQ_WHY,
            "distinct_from": ["DEC-0023"]}),
        &["--card", "C-0001"],
    );
    let rejected = cq_ask(
        &pair,
        &cwd,
        &["MEC-SPRINT"],
        "Does a sprint end at zero?",
        &["--card", "C-0001"],
    );
    let open = cq_ask(
        &pair,
        &cwd,
        &["EDGE-STAM-ZERO"],
        "Is zero stamina a stop?",
        &["--card", "C-0001"],
    );
    assert_eq!(
        [decision.as_str(), rejected.as_str(), open.as_str()],
        ["PR-0001", "PR-0002", "PR-0003"]
    );
    pair.decide_ok(
        &pair.main,
        &decision,
        &common::decision::flags(None, None, None),
        None,
    );
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No: it slows to a walk.", true);
    outcome.expect("rejected");

    let shown = json_run(&pair, &["card", "show", "C-0001"]);
    shown.code(0);
    let keys = top_keys(&shown.stdout);
    let tasks = keys.iter().position(|key| key == "tasks").expect("tasks");
    assert_eq!(keys[tasks + 1], "items", "{keys:?}");
    // 34 since docs/features/task-mandate.md M6: `subtask_items` after
    // `items`, `mandate` after `reviewer`; 35 since
    // docs/features/card-context.md CC2: `context` after `history`; 36
    // since docs/features/upstream-signal.md US-8: `upstream` after
    // `correction`.
    assert_eq!(keys[tasks + 2], "subtask_items", "{keys:?}");
    assert_eq!(keys.len(), 36, "{keys:?}");
    let items = raw_array(&shown.stdout, "items");
    for item_keys in serde_json::from_str::<Vec<OrderedKeys>>(items).expect("items") {
        assert_eq!(item_keys.0, ITEM_KEYS, "{items}");
    }
    let card = shown.json();
    let listed: Vec<&str> = card["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(listed, ["PR-0001", "PR-0002", "PR-0003"]);
    let applied = &card["items"][0];
    let review = doc(&pair, &["review", &decision]);
    for key in [
        "kind",
        "status",
        "summary",
        "working_answer",
        "why",
        "author",
        "created_at",
        "updated_at",
        "staged_at",
        "decided_by",
        "decided_at",
        "decision_note",
        "record_id",
        "record_path",
        "record_title",
    ] {
        assert_eq!(applied[key], review[key], "{key}: {applied}");
    }
    assert_eq!(applied["kind"], json!("decision"));
    assert_eq!(applied["status"], json!("applied"));
    assert_eq!(applied["why"], json!(CQ_WHY));
    assert_eq!(applied["record_id"], json!("DEC-0024"));
    assert_eq!(
        applied["record_path"],
        json!("docs/records/DEC/DEC-0024.md")
    );
    assert!(applied["record_title"].is_string(), "{applied}");
    let refused = &card["items"][1];
    assert_eq!(refused["status"], json!("rejected"));
    assert_eq!(refused["decision_note"], json!("No: it slows to a walk."));
    assert_eq!(refused["record_id"], Value::Null);
    assert_eq!(refused["why"], Value::Null);
    let waiting = &card["items"][2];
    assert_eq!(waiting["status"], json!("open"));
    assert_eq!(waiting["decided_by"], Value::Null);
    assert_eq!(show(&pair, "C-0002")["items"], json!([]));

    // The text: one line per item after `tasks:`; `items: -` with none.
    let text = ok(&pair, &["card", "show", "C-0001"]);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("items: "))
        .collect();
    assert_eq!(
        lines,
        [
            format!("items: PR-0001 | decision | applied | {CQ_TEXT} | record DEC-0024"),
            "items: PR-0002 | question | rejected | Does a sprint end at zero?".to_owned(),
            "items: PR-0003 | question | open | Is zero stamina a stop?".to_owned(),
        ],
        "{text}"
    );
    let at = |prefix: &str| {
        text.lines()
            .position(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("no {prefix}: {text}"))
    };
    assert!(at("tasks:") < at("items:"), "{text}");
    let none = ok(&pair, &["card", "show", "C-0002"]);
    assert!(none.lines().any(|line| line == "items: -"), "{none}");

    // From a root in no git worktree: the same document.
    let bare = pair.scratch.dir("bare");
    common::copy_dir(&pair.main.join("docs"), &bare.join("docs"));
    common::copy_file(
        pair.main.join("specengine.toml"),
        bare.join("specengine.toml"),
    );
    let outside = pair.spec_piped(&bare, &["--json", "card", "show", "C-0001"], b"");
    outside.code(0);
    assert_eq!(outside.json()["items"], card["items"]);
}

/// K1: `--history` on spec-a: 70 moves of `C-0001` and 70 stage or unstage
/// events of its decision, interleaved, give 128 entries: the card's newest
/// 64 `card.*` events and the item's newest 64 `proposal.*`, merged by
/// `seq`, oldest first, `[{seq, type, at, payload}]`; the decision's linked
/// update's events absent. Without `--history`, `null`. M: the two lists
/// capped together at 64.
#[test]
fn card_questions_k1_history_merges_each_lists_newest_64() {
    let pair = Pair::new("cards-cq-history", "spec-a");
    let cwd = pair.linked.clone();
    make(&pair, "Rest regeneration", &["--target", "RULE-STAM-REGEN"]);
    let (base, span) = pair.span(&cwd, "RULE-STAM-REGEN");
    let regen = span.replacen("Base rate 10 units/s,", "Base rate 10 units/s at rest,", 1);
    assert_ne!(regen, span);
    let decision = cq_decide(
        &pair,
        &cwd,
        &json!({"node_ids": ["RULE-STAM-REGEN"], "text": CQ_TEXT, "working_answer": CQ_WORKING,
            "price_of_other": "Without the wait R-28 is rebalanced.",
            "distinct_from": ["DEC-0023"],
            "proposed_patch": {"target": "RULE-STAM-REGEN", "base": base, "text": regen,
                "rationale": "Rest only."}}),
        &["--card", "C-0001"],
    );
    assert_eq!(decision, "PR-0001");
    assert_eq!(
        pair.sql("SELECT linked FROM proposals WHERE id = 'PR-0001';")
            .trim(),
        "PR-0002"
    );
    for round in 0..70 {
        let to = if round % 2 == 0 { "ready" } else { "backlog" };
        ok(&pair, &["card", "move", "C-0001", "--to", to]);
        if round % 2 == 0 {
            cq_stage(&pair, &decision, Some("\"decision\":\"approve\""));
        } else {
            cq_stage(&pair, &decision, None);
        }
    }
    // The linked update's own event, after every other.
    cq_stage(&pair, "PR-0002", Some("\"decision\":\"approve\""));

    let all = pair.events();
    let newest = |kind: &str, id: &str| -> Vec<i64> {
        let mut seqs: Vec<i64> = all
            .iter()
            .filter(|event| event.event_type.starts_with(kind) && event.payload["id"] == id)
            .map(|event| event.seq)
            .collect();
        let cut = seqs.len().saturating_sub(64);
        seqs.drain(..cut);
        seqs
    };
    let cards = newest("card.", "C-0001");
    let items = newest("proposal.", "PR-0001");
    assert_eq!((cards.len(), items.len()), (64, 64), "premise: 71 of each");
    let mut want: Vec<i64> = cards.iter().chain(&items).copied().collect();
    want.sort_unstable();

    let card = show(&pair, "C-0001");
    assert_eq!(card["history"], Value::Null, "no --history");
    let card = doc(&pair, &["card", "show", "C-0001", "--history"]);
    let history = card["history"].as_array().expect("history");
    assert_eq!(history.len(), 128);
    let seqs: Vec<i64> = history
        .iter()
        .map(|entry| entry["seq"].as_i64().expect("seq"))
        .collect();
    assert_eq!(seqs, want);
    for entry in history {
        let mut keys: Vec<&String> = entry.as_object().expect("an entry").keys().collect();
        keys.sort();
        assert_eq!(keys, ["at", "payload", "seq", "type"], "{entry}");
        assert_ne!(entry["payload"]["id"], json!("PR-0002"), "{entry}");
    }
}

/// K1: a carded question whose row does not read (a `why` set by SQL) is
/// left out of `items`, its note after the card's own notes, exit 0; the
/// other item still listed.
#[test]
fn card_questions_k1_a_corrupt_item_is_left_out_with_its_note() {
    let pair = Pair::new("cards-cq-corrupt", "studio-lamp");
    make(
        &pair,
        "The storm scene",
        &["--target", "SCN-STORM", "--discipline", "narrative"],
    );
    let first = cq_ask(
        &pair,
        &pair.main,
        &["SCN-STORM"],
        "Which bell rings at dusk?",
        &["--card", "C-0001"],
    );
    let second = cq_ask(
        &pair,
        &pair.main,
        &["SCN-ARRIVAL"],
        "Who meets the keeper at the pier?",
        &["--card", "C-0001"],
    );
    pair.sql(&format!(
        "UPDATE proposals SET why = 'Because.' WHERE id = '{first}';"
    ));
    let card = doc(&pair, &["card", "show", "C-0001"]);
    let listed: Vec<&str> = card["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(listed, [second.as_str()]);
    assert_eq!(
        card["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!(format!(
            "{first}: its row does not read (why: it is set on a question, which holds none); \
             not listed"
        ))),
        "{card}"
    );
}

/// K1: `spec card list --json` byte-equal before and after the card's items
/// are raised; and ADR-0012: twin projects, one card each, the first beside
/// an open question and an open decision: a move into the return column and
/// out, an assign, the move to `ready-for-dev`, the accept, and on a second
/// pair of cards a cancel, give documents and event payloads equal; `spec
/// gate --path` gives the same verdict before and after. M: `card_move`
/// refusing while an item is open.
#[test]
fn card_questions_k1_no_card_rule_reads_an_item() {
    let with = Pair::new("cards-cq-twin-a", "studio-lamp");
    let without = Pair::new("cards-cq-twin-b", "studio-lamp");
    for pair in [&with, &without] {
        for title in ["The storm scene", "The cove"] {
            make(
                pair,
                title,
                &[
                    "--target",
                    "SCN-STORM",
                    "--discipline",
                    "narrative",
                    "--reviewer",
                    "Ann",
                ],
            );
        }
    }
    let list = |pair: &Pair| json_run(pair, &["card", "list", "--with-cancelled"]).stdout;
    let gate = |pair: &Pair| {
        let done = run(
            pair,
            &[
                "gate",
                "--worktree",
                pair.main.to_str().expect("UTF-8"),
                "--path",
                "docs/spec/scn-storm.md",
            ],
        );
        (done.code, done.stdout, done.stderr)
    };
    let listed = list(&with);
    let verdict = gate(&with);
    for card in ["C-0001", "C-0002"] {
        cq_ask(
            &with,
            &with.main,
            &["SCN-STORM"],
            &format!("Which bell rings at dusk for {card}?"),
            &["--card", card],
        );
        cq_decide(
            &with,
            &with.main,
            &json!({"node_ids": ["SCN-STORM"], "text": format!("Does the lamp burn oil for {card}?"),
                "working_answer": "Yes.", "price_of_other": "The lamp room is redrawn."}),
            &["--card", card],
        );
    }
    assert_eq!(list(&with), listed, "the board's list unchanged");
    assert_eq!(gate(&with), verdict, "the gate unchanged");

    let writes: [&[&str]; 5] = [
        &[
            "card",
            "move",
            "C-0001",
            "--to",
            "sent-back",
            "--note",
            "Two endings contradict",
        ],
        &["card", "move", "C-0001", "--to", "outline"],
        &["card", "assign", "C-0001", "--assignee", "Rob"],
        &["card", "move", "C-0001", "--to", "ready-for-dev"],
        &["card", "cancel", "C-0002", "--reason", "Cut."],
    ];
    for args in writes {
        let a = json_run(&with, args);
        let b = json_run(&without, args);
        assert_eq!((a.code, &a.stdout), (b.code, &b.stdout), "{args:?}");
        a.code(0);
    }
    for pair in [&with, &without] {
        let done = pair.spec_pty(
            &pair.main,
            &["card", "accept", "C-0001", "--as", "Ann", "--briefed"],
            Some("y"),
        );
        assert_eq!(done.code, 0, "{}", done.output);
    }
    let payloads = |pair: &Pair| -> Vec<(String, Value)> {
        events(pair)
            .into_iter()
            .map(|(kind, mut payload)| {
                if let Some(object) = payload.as_object_mut() {
                    object.retain(|key, _| !key.ends_with("at"));
                }
                (kind, payload)
            })
            .collect()
    };
    assert_eq!(payloads(&with), payloads(&without));
    let strip = |mut card: Value| {
        let object = card.as_object_mut().expect("a card");
        for key in [
            "items",
            "created_at",
            "updated_at",
            "acceptance",
            "cancellation",
        ] {
            object.remove(key);
        }
        card
    };
    for id in ["C-0001", "C-0002"] {
        assert_eq!(strip(show(&with, id)), strip(show(&without, id)), "{id}");
        assert_eq!(
            show(&with, id)["items"].as_array().map(Vec::len),
            Some(2),
            "{id}"
        );
    }
    assert_eq!(gate(&with), verdict, "the gate after");
}
