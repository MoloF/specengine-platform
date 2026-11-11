//! docs/features/department-split.md (RM-0100, v5 step 6c, ADR-0078)
//! "Acceptance criteria", the CLI half: `spec card split` through the binary
//! on scratch git repositories of `fixtures/card-press` (a print shop with
//! two `[[card.split]]` rows: `broadsheet` pre-selects `copy` and `plates`,
//! `print-run` `plates`), the canon docs/canon/card-split.md "The rule".
//!
//! - S1 (the CLI's part): a copy whose row names `voice`, which no flow
//!   lists, stops `spec card list` with exit 2 and the line; `spec show` and
//!   `spec check` answer as they did.
//! - S3: a split from `PRN-WEEKLY`: `C-0001` the parent, `C-0002` `copy` on
//!   the broadsheets, `C-0003` `plates` on both kinds' nodes, each child's
//!   parent, reviewer, title and goal the parent's, no assignee; `list
//!   --parent`; a snapshot per target; three `card.created`, the parent's
//!   first; the text lines. M: children inserted before the parent; a
//!   child's reviewer left null; a child's `goal` dropped.
//! - S4: `--parent C-0001`: one child, the stored parent's reviewer, the
//!   parent's row, `revision`, `updated_at` byte-equal, no event naming it;
//!   the usage, the missing parent, a look-alike, a reviewer the chain ends
//!   at `local`. M: the parent's `revision` raised; the chain skipping the
//!   stored parent's reviewer.
//! - S5 (the CLI's part): a child no flow takes and a repeated child exit 1,
//!   nothing written.
//! - S6: `--dry-run` plans and writes nothing; no `--child` plans every
//!   discipline; `STY-0001` suggests none; persons are not judged, every
//!   other rule is; a bad `[card]` exits 2; a repeat run byte-equal. M: a dry
//!   run storing; a dry run judging the reviewer.
//! - S9 (the CLI's part): an unknown `[card]` key fails `spec card list`
//!   alone, `spec show` and `spec check` answering as before: the
//!   older-binary pin.
//! - K1: no cascade (ADR-0074, ADR-0012): a child's moves, accept and cancel
//!   change no other card's row; a cancelled parent leaves its children open;
//!   a parent with open children moves and is accepted; accepting both
//!   children moves no parent. M: a split raising the stored parent's
//!   `revision`; an accept refusing a parent with open children.
//!
//! The oracle of "nothing written" is `SELECT * FROM cards` and
//! `SqliteQueue::dump()`, both raw. The scope's node lists come from
//! `spec tree PRN-WEEKLY --depth 2` on the fixture, written here in its
//! order. Every expected value is worked out from the spec, never read back
//! from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use common::proposal::Pair;
use common::{read_text, write};
use serde_json::{Value, json};
use task_common::Ordered;

/// The print shop with its two rows.
const PRESS: &str = "card-press";

/// The person who splits and reviews.
const ANA: &str = "Ana Ruiz";

/// The parent's title of S3.
const TITLE: &str = "The weekly run";

/// The split document's keys, in order.
const SPLIT_KEYS: [&str; 5] = ["dry_run", "parent", "children", "reviewer", "notes"];

/// The parent's keys, in order.
const PARENT_KEYS: [&str; 8] = [
    "id",
    "created",
    "title",
    "discipline",
    "flow",
    "flow_title",
    "column",
    "targets",
];

/// A child's keys, in order.
const CHILD_KEYS: [&str; 8] = [
    "id",
    "discipline",
    "flow",
    "flow_title",
    "column",
    "targets",
    "from",
    "suggested",
];

/// `PRN-WEEKLY`'s scope as `spec tree PRN-WEEKLY --depth 2` serves it: the
/// target, its ID'd section, its two-level children; `(ID, kind)`.
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

/// The scope's nodes of `kinds`, scope order.
fn of_kinds(kinds: &[&str]) -> Vec<&'static str> {
    SCOPE
        .iter()
        .filter(|(_, kind)| kinds.contains(kind))
        .map(|(id, _)| *id)
        .collect()
}

/// `copy`'s targets: `broadsheet`'s nodes, no `PRN-` among them.
fn copy_targets() -> Vec<&'static str> {
    of_kinds(&["broadsheet"])
}

/// `plates`' targets: both rows list it, so both kinds' nodes.
fn plates_targets() -> Vec<&'static str> {
    of_kinds(&["broadsheet", "print-run"])
}

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

/// `spec --json args`, exit 0: the run.
fn json_run(pair: &Pair, args: &[&str]) -> common::Run {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = run(pair, &all);
    done.code(0);
    done
}

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    json_run(pair, args).json()
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

/// `card new --title <title> <flags>`: its ID.
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
    if !pair.db().exists() {
        return (String::new(), String::new());
    }
    (
        pair.sql("SELECT * FROM cards ORDER BY id;"),
        pair.queue().dump().expect("dump"),
    )
}

/// `(type, payload)` of every event, in `seq` order.
fn events(pair: &Pair) -> Vec<(String, Value)> {
    pair.events()
        .into_iter()
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The raw row of `id`.
fn row(pair: &Pair, id: &str) -> String {
    pair.sql(&format!("SELECT * FROM cards WHERE id = '{id}';"))
}

/// The raw rows of every card but `id`, every column but `position`: a
/// card leaving a column closes its gap, the neighbour shifted taking
/// neither `revision` nor `updated_at` nor an event (docs/canon/cards.md
/// "Store"), which is no write on it.
fn others(pair: &Pair, id: &str) -> String {
    pair.sql(&format!(
        "SELECT id, project, flow, col, title, goal, discipline, targets, snapshot, criteria, \
         assignee, reviewer, estimate, due, parent, corrects, evidence, acceptance, \
         cancellation, created_by, created_at, updated_at, revision, tasks FROM cards WHERE id \
         <> '{id}' ORDER BY id;"
    ))
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

/// S3's command, `extra` appended.
fn weekly<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
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
    ];
    args.extend_from_slice(extra);
    args
}

/// `spec card accept id --as name --briefed` on a terminal, answering `y`.
fn accept(pair: &Pair, id: &str, name: &str) -> common::decision::PtyRun {
    pair.spec_pty(
        &pair.main,
        &["card", "accept", id, "--as", name, "--briefed"],
        Some("y"),
    )
}

/// `card.created`'s payload of "Documents and events".
fn created(id: &str, flow: &str, column: &str, position: u64) -> (String, Value) {
    (
        "card.created".to_owned(),
        json!({"id": id, "flow": flow, "column": column, "position": position,
               "assignee": null, "reviewer": ANA, "as": ANA}),
    )
}

/// The key orders of a split document: its own, the parent's (when one),
/// each child's.
fn assert_key_orders(stdout: &str) {
    let document = Ordered::of(stdout);
    assert_eq!(document.keys(), SPLIT_KEYS, "{stdout}");
    if let Ordered::Object(_) = document.get("parent") {
        assert_eq!(document.get("parent").keys(), PARENT_KEYS, "{stdout}");
    }
    if let Ordered::Array(children) = document.get("children") {
        for child in children {
            assert_eq!(child.keys(), CHILD_KEYS, "{stdout}");
        }
    }
}

/// The scratch config: `from` replaced by `to`, once, uncommitted.
fn edit_config(pair: &Pair, from: &str, to: &str) -> String {
    let text = read_text(&pair.main, "specengine.toml");
    assert_eq!(text.matches(from).count(), 1, "{from:?} once in the config");
    let edited = text.replacen(from, to, 1);
    write(&pair.main, "specengine.toml", edited.clone());
    edited
}

/// The 1-based line of `text` holding `needle`.
fn line_holding(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds {needle:?}"))
        + 1
}

/// `(exit, stdout)` of `spec show PRN-WEEKLY` and `spec check`.
fn reads(pair: &Pair) -> Vec<(i32, String)> {
    [&["show", "PRN-WEEKLY"][..], &["check"][..]]
        .iter()
        .map(|args| {
            let done = run(pair, args);
            (done.code, done.stdout)
        })
        .collect()
}

// ------------------------------------------------------------------ S3

/// S3: the split from `PRN-WEEKLY`, `--json`: exit 0; the document, every
/// key in order; `C-0001` `copy/draft` on `[PRN-WEEKLY]`, `C-0002`
/// `copy/draft` on the broadsheets, `C-0003` `plates/queued` on both kinds'
/// nodes, both by kind and suggested; no note.
#[test]
fn s3_a_split_from_a_document_serves_its_document() {
    let pair = Pair::new("split-s3-doc", PRESS);
    let done = json_run(&pair, &weekly(&[]));
    assert_key_orders(&done.stdout);
    assert_eq!(
        done.json(),
        json!({
            "dry_run": false,
            "parent": {"id": "C-0001", "created": true, "title": TITLE, "discipline": "copy",
                       "flow": "copy", "flow_title": "Copy desk", "column": "draft",
                       "targets": ["PRN-WEEKLY"]},
            "children": [
                {"id": "C-0002", "discipline": "copy", "flow": "copy", "flow_title": "Copy desk",
                 "column": "draft", "targets": copy_targets(), "from": "kinds",
                 "suggested": true},
                {"id": "C-0003", "discipline": "plates", "flow": "plates", "flow_title": "plates",
                 "column": "queued", "targets": plates_targets(), "from": "kinds",
                 "suggested": true}
            ],
            "reviewer": ANA,
            "notes": []
        })
    );
    assert!(
        copy_targets().iter().all(|id| !id.starts_with("PRN-")),
        "no PRN- in copy's"
    );
}

/// S3: the cards as stored: the parent with the split's reviewer and no
/// assignee; each child under `C-0001`, its reviewer the split's, its title
/// and goal the parent's, no assignee, criteria, estimate, due or evidence,
/// a snapshot per target; `list --parent C-0001` lists both; the event log
/// gains exactly three `card.created`, `C-0001` first, `as` Ana Ruiz. M:
/// children inserted before the parent; a child's reviewer left null; a
/// child's `goal` dropped.
#[test]
fn s3_a_split_stores_a_parent_and_a_subtask_per_discipline() {
    let pair = Pair::new("split-s3-rows", PRESS);
    assert!(events(&pair).is_empty());
    json_run(&pair, &weekly(&["--goal", "Out by Thursday."]));
    let parent = show(&pair, "C-0001");
    assert_eq!(parent["flow"], json!("copy"), "{parent}");
    assert_eq!(parent["column"], json!("draft"));
    assert_eq!(parent["title"], json!(TITLE));
    assert_eq!(parent["goal"], json!("Out by Thursday."));
    assert_eq!(parent["discipline"], json!("copy"));
    assert_eq!(parent["targets"], json!(["PRN-WEEKLY"]));
    assert_eq!(parent["reviewer"], json!(ANA));
    assert_eq!(parent["assignee"], Value::Null);
    assert_eq!(parent["parent"], Value::Null);
    assert_eq!(parent["created_by"], json!(ANA));
    for (id, flow, column, targets) in [
        ("C-0002", "copy", "draft", copy_targets()),
        ("C-0003", "plates", "queued", plates_targets()),
    ] {
        let card = show(&pair, id);
        assert_eq!(card["parent"], json!("C-0001"), "{card}");
        assert_eq!(card["flow"], json!(flow), "{card}");
        assert_eq!(card["column"], json!(column), "{card}");
        assert_eq!(card["discipline"], json!(flow), "{card}");
        assert_eq!(card["targets"], json!(targets), "{card}");
        assert_eq!(
            card["reviewer"],
            json!(ANA),
            "M: a child's reviewer left null"
        );
        assert_eq!(card["title"], json!(TITLE), "{card}");
        assert_eq!(
            card["goal"],
            json!("Out by Thursday."),
            "M: a child's goal dropped"
        );
        assert_eq!(card["created_by"], json!(ANA), "{card}");
        for none in ["assignee", "estimate", "due", "corrects"] {
            assert_eq!(card[none], Value::Null, "{id} {none}");
        }
        assert_eq!(card["criteria"], json!([]), "{card}");
        assert_eq!(card["evidence"], json!([]), "{card}");
        let frozen: Vec<&str> = card["snapshot"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: a snapshot: {card}"))
            .iter()
            .map(|entry| entry["id"].as_str().expect("a frozen ID"))
            .collect();
        assert_eq!(frozen, targets, "{id}: a snapshot per target");
    }
    assert_eq!(listed(&pair, &["--parent", "C-0001"]), ["C-0002", "C-0003"]);
    assert_eq!(
        events(&pair),
        [
            created("C-0001", "copy", "draft", 1),
            created("C-0002", "copy", "draft", 2),
            created("C-0003", "plates", "queued", 1),
        ],
        "M: children inserted before the parent"
    );
}

/// S3: the text output, one line a card, `by kind, suggested` on each child.
#[test]
fn s3_the_text_lines() {
    let pair = Pair::new("split-s3-text", PRESS);
    let copy = copy_targets().join(", ");
    let plates = plates_targets().join(", ");
    assert_eq!(
        ok(&pair, &weekly(&[])),
        format!(
            "split C-0001 new: copy/draft | {TITLE} | PRN-WEEKLY | reviewer {ANA}\n\
             subtask C-0002: copy/draft | copy | {copy} | by kind, suggested\n\
             subtask C-0003: plates/queued | plates | {plates} | by kind, suggested\n"
        )
    );
}

// ------------------------------------------------------------------ S4

/// S4: a split under the stored `C-0001`: one child under it, the stored
/// parent's reviewer (no `--reviewer`, no `--as`), `created` false with the
/// stored fields; `C-0001`'s row, `revision` and `updated_at` byte-equal and
/// no new event naming it. M: the parent's `revision` raised; the chain
/// skipping the stored parent's reviewer.
#[test]
fn s4_a_split_under_a_stored_task() {
    let pair = Pair::new("split-s4", PRESS);
    assert_eq!(
        make(&pair, "T", &["--discipline", "copy", "--reviewer", ANA]),
        "C-0001"
    );
    let parent_row = row(&pair, "C-0001");
    let before = show(&pair, "C-0001");
    let count = events(&pair).len();
    let done = json_run(
        &pair,
        &["card", "split", "--parent", "C-0001", "--child", "plates"],
    );
    assert_key_orders(&done.stdout);
    assert_eq!(
        done.json(),
        json!({
            "dry_run": false,
            "parent": {"id": "C-0001", "created": false, "title": "T", "discipline": "copy",
                       "flow": "copy", "flow_title": "Copy desk", "column": "draft",
                       "targets": []},
            "children": [
                {"id": "C-0002", "discipline": "plates", "flow": "plates",
                 "flow_title": "plates", "column": "queued", "targets": [], "from": "parent",
                 "suggested": false}
            ],
            "reviewer": ANA,
            "notes": []
        })
    );
    assert_eq!(
        row(&pair, "C-0001"),
        parent_row,
        "the parent's row byte-equal"
    );
    let after = show(&pair, "C-0001");
    for key in ["revision", "updated_at", "history"] {
        assert_eq!(after[key], before[key], "{key}");
    }
    let child = show(&pair, "C-0002");
    assert_eq!(child["parent"], json!("C-0001"));
    assert_eq!(child["reviewer"], json!(ANA));
    assert_eq!(child["title"], json!("T"));
    assert_eq!(child["created_by"], json!("local"));
    let all = events(&pair);
    assert_eq!(all.len(), count + 1, "{all:?}");
    assert_eq!(all[count].0, "card.created");
    assert_eq!(all[count].1["id"], json!("C-0002"));
    assert_eq!(all[count].1["reviewer"], json!(ANA));
    assert_eq!(all[count].1["as"], json!("local"));
    assert!(
        all[count..]
            .iter()
            .all(|(_, payload)| payload["id"] != "C-0001"),
        "no event names the parent"
    );
}

/// S4: `--parent` with a new parent's flag exits 2; a parent that is not
/// there exits 1 with `parent: no card C-0099`, nothing written, the next
/// `new` taking `C-0002`; a Cyrillic look-alike exits 2 naming `C-0001`; a
/// stored parent with no reviewer, no `--reviewer` and no `--as` exits 1,
/// the chain ending at `local`.
#[test]
fn s4_a_stored_parent_is_asked_for_in_order() {
    let pair = Pair::new("split-s4-refused", PRESS);
    make(&pair, "T", &["--discipline", "copy"]);
    let before = state(&pair);
    for extra in [
        &["--title", "X"][..],
        &["--target", "PRN-WEEKLY"][..],
        &["--goal", "G"][..],
        &["--discipline", "copy"][..],
    ] {
        let mut args = vec![
            "card",
            "split",
            "--parent",
            "C-0001",
            "--child",
            "plates",
            "--reviewer",
            ANA,
        ];
        args.extend_from_slice(extra);
        cannot(&pair, &args);
        assert_eq!(state(&pair), before, "{extra:?}: nothing written");
    }
    assert_eq!(
        refused(
            &pair,
            &[
                "card",
                "split",
                "--parent",
                "C-0099",
                "--child",
                "plates",
                "--reviewer",
                ANA
            ]
        ),
        "spec: parent: no card C-0099\n"
    );
    assert_eq!(state(&pair), before, "nothing written");
    let look_alike = "\u{0421}-0001";
    let line = cannot(
        &pair,
        &[
            "card",
            "split",
            "--parent",
            look_alike,
            "--child",
            "plates",
            "--reviewer",
            ANA,
        ],
    );
    assert!(line.contains("`C-0001`"), "the Latin form named: {line}");
    assert_eq!(state(&pair), before);
    assert_eq!(
        refused(
            &pair,
            &["card", "split", "--parent", "C-0001", "--child", "plates"]
        ),
        "spec: reviewer: `local` can be no reviewer: a person accepts a result\n",
        "C-0001 has no reviewer: the chain ends at the acting name"
    );
    assert_eq!(state(&pair), before);
    assert_eq!(make(&pair, "U", &["--discipline", "copy"]), "C-0002");
}

// ------------------------------------------------------------------ S5

/// S5, the CLI's part: a child no flow takes, and a child named twice, exit
/// 1 with their lines, nothing written; `--json` serves the refusal's
/// document: `parent` null, `children` `[]`, `reviewer` null, the reason
/// last of `notes`.
#[test]
fn s5_a_refused_child_writes_nothing() {
    let pair = Pair::new("split-s5", PRESS);
    make(&pair, "T", &["--discipline", "copy"]);
    let before = state(&pair);
    let nowhere = "child: no flow takes the discipline `nowhere`: none lists it, and every \
                   `[[card.flow]]` lists its disciplines";
    let base = [
        "card",
        "split",
        "--target",
        "PRN-WEEKLY",
        "--title",
        TITLE,
        "--reviewer",
        ANA,
    ];
    for (children, reason) in [
        (&["--child", "copy", "--child", "nowhere"][..], nowhere),
        (
            &["--child", "copy", "--child", "copy"][..],
            "child: `copy` is named twice",
        ),
    ] {
        let mut args = base.to_vec();
        args.extend_from_slice(children);
        assert_eq!(refused(&pair, &args), format!("spec: {reason}\n"));
        assert_eq!(state(&pair), before, "{children:?}: nothing written");
        let mut all = vec!["--json"];
        all.extend_from_slice(&args);
        let done = run(&pair, &all);
        done.code(1);
        assert_key_orders(&done.stdout);
        assert_eq!(
            done.json(),
            json!({"dry_run": false, "parent": null, "children": [], "reviewer": null,
                   "notes": [reason]}),
            "{children:?}"
        );
        assert_eq!(state(&pair), before, "{children:?}: nothing written");
    }
    assert_eq!(make(&pair, "U", &["--discipline", "copy"]), "C-0002");
}

// ------------------------------------------------------------------ S6

/// S6: `--dry-run` with no `--child`: exit 0, `dry_run` true, every `id`
/// null, `copy` then `plates`, both suggested, the targets of the write; no
/// card row, no event, the last `seq` unchanged; the same run twice
/// byte-equal. M: a dry run storing.
#[test]
fn s6_a_dry_run_plans_every_discipline_and_writes_nothing() {
    let pair = Pair::new("split-s6", PRESS);
    make(&pair, "T", &["--discipline", "plates"]);
    let before = state(&pair);
    let last = pair.events().last().map(|event| event.seq);
    let args = [
        "card",
        "split",
        "--target",
        "PRN-WEEKLY",
        "--title",
        "T",
        "--discipline",
        "copy",
        "--as",
        ANA,
        "--dry-run",
    ];
    let done = json_run(&pair, &args);
    assert_key_orders(&done.stdout);
    assert_eq!(
        done.json(),
        json!({
            "dry_run": true,
            "parent": {"id": null, "created": true, "title": "T", "discipline": "copy",
                       "flow": "copy", "flow_title": "Copy desk", "column": "draft",
                       "targets": ["PRN-WEEKLY"]},
            "children": [
                {"id": null, "discipline": "copy", "flow": "copy", "flow_title": "Copy desk",
                 "column": "draft", "targets": copy_targets(), "from": "kinds",
                 "suggested": true},
                {"id": null, "discipline": "plates", "flow": "plates", "flow_title": "plates",
                 "column": "queued", "targets": plates_targets(), "from": "kinds",
                 "suggested": true}
            ],
            "reviewer": ANA,
            "notes": []
        })
    );
    assert_eq!(state(&pair), before, "M: a dry run storing");
    assert_eq!(pair.events().last().map(|event| event.seq), last);
    assert_eq!(
        json_run(&pair, &args).stdout,
        done.stdout,
        "one state, one plan"
    );
    let mut text_args = args.to_vec();
    text_args.retain(|arg| *arg != "--json");
    let text = ok(&pair, &text_args);
    assert!(
        text.ends_with("dry run: nothing written\n"),
        "the text's last line: {text}"
    );
    assert!(
        text.starts_with("plan - new: copy/draft | T | PRN-WEEKLY"),
        "{text}"
    );
    assert_eq!(state(&pair), before);
}

/// S6: `STY-0001`, a `style-ruling` no row names: nothing suggested, each
/// child on `[STY-0001]`, `from` `parent`.
#[test]
fn s6_a_document_no_row_names_suggests_nothing() {
    let pair = Pair::new("split-s6-sty", PRESS);
    let document = doc(
        &pair,
        &[
            "card",
            "split",
            "--target",
            "STY-0001",
            "--title",
            "T",
            "--discipline",
            "copy",
            "--dry-run",
        ],
    );
    let children = document["children"].as_array().expect("children");
    let rows: Vec<(&str, &Value, &Value, &Value)> = children
        .iter()
        .map(|child| {
            (
                child["discipline"].as_str().expect("a discipline"),
                &child["targets"],
                &child["from"],
                &child["suggested"],
            )
        })
        .collect();
    let sty = json!(["STY-0001"]);
    let parent = json!("parent");
    assert_eq!(
        rows,
        [
            ("copy", &sty, &parent, &json!(false)),
            ("plates", &sty, &parent, &json!(false))
        ]
    );
    assert!(!pair.db().exists() || state(&pair).0.is_empty(), "no card");
}

/// S6: a dry run judges no person: `--reviewer agent --dry-run` exits 0
/// with `agent` served unjudged; the same without `--dry-run` exits 1. Every
/// other rule is the write's: `--child nowhere` exits 1 with its line; a bad
/// `[card]` exits 2. M: a dry run judging the reviewer.
#[test]
fn s6_a_dry_run_judges_every_rule_but_the_persons() {
    let pair = Pair::new("split-s6-rules", PRESS);
    let mut args = weekly(&[]);
    let at = args
        .iter()
        .position(|arg| *arg == "--reviewer")
        .expect("--reviewer");
    args[at + 1] = "agent";
    let mut dry = args.clone();
    dry.push("--dry-run");
    let planned = doc(&pair, &dry);
    assert_eq!(planned["dry_run"], json!(true));
    assert_eq!(planned["reviewer"], json!("agent"), "unjudged");
    assert_eq!(state(&pair).0, "", "no card");
    assert_eq!(
        refused(&pair, &args),
        "spec: reviewer: `agent` can be no reviewer: a person accepts a result\n"
    );
    let before = state(&pair);
    let nowhere = [
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
        "--dry-run",
    ];
    let line = refused(&pair, &nowhere);
    assert!(
        line.starts_with("spec: child: no flow takes the discipline `nowhere`"),
        "{nowhere:?}: {line}"
    );
    assert_eq!(state(&pair), before);
    edit_config(
        &pair,
        "[[card.split]]\nkind = \"print-run\"",
        "[[card.split]]\nkind = 7",
    );
    cannot(&pair, &weekly(&["--dry-run"]));
    assert_eq!(state(&pair), before);
}

/// S6 (usage): no `--child` without `--dry-run`, no `--title` without
/// `--parent`, each exit 2, nothing written.
#[test]
fn s6_a_write_names_each_child() {
    let pair = Pair::new("split-s6-usage", PRESS);
    let line = cannot(
        &pair,
        &[
            "card",
            "split",
            "--target",
            "PRN-WEEKLY",
            "--title",
            "T",
            "--reviewer",
            ANA,
        ],
    );
    assert!(
        line.contains("name each subtask's discipline with --child, or plan with --dry-run"),
        "{line}"
    );
    cannot(
        &pair,
        &[
            "card",
            "split",
            "--target",
            "PRN-WEEKLY",
            "--child",
            "copy",
            "--reviewer",
            ANA,
        ],
    );
    assert_eq!(state(&pair).0, "");
}

// ------------------------------------------------------------- S1, S9

/// S1, the CLI's part: a copy whose `print-run` row names `voice`, which no
/// flow lists, stops `spec card list` with exit 2 and the line; `spec show
/// PRN-WEEKLY` and `spec check` exit and print as before.
#[test]
fn s1_a_row_naming_no_flows_discipline_stops_the_card_surfaces_alone() {
    let pair = Pair::new("split-s1-voice", PRESS);
    let before = reads(&pair);
    let text = edit_config(
        &pair,
        "kind = \"print-run\"\ndisciplines = [\"plates\"]",
        "kind = \"print-run\"\ndisciplines = [\"voice\"]",
    );
    let line = line_holding(&text, "[\"voice\"]");
    let stderr = cannot(&pair, &["card", "list"]);
    assert!(
        stderr.contains(&format!(
            "specengine.toml:{line}: `[[card.split]]` `print-run`: no `[[card.flow]]` lists the \
             discipline `voice`"
        )),
        "{stderr}"
    );
    cannot(&pair, &weekly(&[]));
    assert_eq!(reads(&pair), before, "show and check as before");
}

/// S9: an unknown `[card]` key, as an older build reads `split`, fails
/// `spec card list` alone with the unknown-field line; `spec show` and
/// `spec check` answer as before.
#[test]
fn s9_an_unknown_card_key_fails_the_card_surfaces_alone() {
    let pair = Pair::new("split-s9-older", PRESS);
    let before = reads(&pair);
    let text = edit_config(
        &pair,
        "\n[[card.flow]]\nname = \"copy\"",
        "\n[card]\nbogus = 1\n\n[[card.flow]]\nname = \"copy\"",
    );
    let line = line_holding(&text, "bogus = 1");
    let stderr = cannot(&pair, &["card", "list"]);
    assert!(
        stderr.contains(&format!(
            "specengine.toml:{line}: unknown field `bogus`, expected `flow` or `split`"
        )),
        "{stderr}"
    );
    assert_eq!(reads(&pair), before, "show and check as before");
}

// ------------------------------------------------------------------ K1

/// K1: after S3's split, `C-0002` moves to `proof` and is accepted by its
/// reviewer, each step leaving every other card's row; `C-0003` is accepted
/// too and `C-0001` stays where it was.
#[test]
fn k1_a_childs_writes_touch_no_other_card() {
    let pair = Pair::new("split-k1-child", PRESS);
    json_run(&pair, &weekly(&[]));
    let rest = others(&pair, "C-0002");
    for (to, by) in [("edit", "desk"), ("proof", ANA)] {
        ok(&pair, &["card", "move", "C-0002", "--to", to, "--as", by]);
        assert_eq!(others(&pair, "C-0002"), rest, "moving C-0002 to {to}");
    }
    let accepted = accept(&pair, "C-0002", ANA);
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert_eq!(others(&pair, "C-0002"), rest, "accepting C-0002");
    let parent = row(&pair, "C-0001");
    ok(&pair, &["card", "move", "C-0003", "--to", "checked"]);
    let accepted = accept(&pair, "C-0003", ANA);
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert_eq!(
        row(&pair, "C-0001"),
        parent,
        "accepting both children moves no parent"
    );
}

/// K1: cancelling a child changes no other card; cancelling the parent
/// leaves both children open.
#[test]
fn k1_a_cancel_touches_one_card() {
    let pair = Pair::new("split-k1-cancel", PRESS);
    json_run(&pair, &weekly(&[]));
    let rest = others(&pair, "C-0002");
    ok(&pair, &["card", "cancel", "C-0002", "--reason", "Pulled."]);
    assert_eq!(others(&pair, "C-0002"), rest, "cancelling C-0002");
    let rest = others(&pair, "C-0001");
    ok(&pair, &["card", "cancel", "C-0001", "--reason", "Pulled."]);
    assert_eq!(others(&pair, "C-0001"), rest, "cancelling the parent");
    assert_eq!(show(&pair, "C-0003")["column"], json!("queued"), "open");
}

/// K1: the parent moves to `proof` and is accepted with both children open;
/// neither child's row changes. M: an accept refusing a parent with open
/// children.
#[test]
fn k1_a_parent_with_open_children_is_accepted() {
    let pair = Pair::new("split-k1-parent", PRESS);
    json_run(&pair, &weekly(&[]));
    let rest = others(&pair, "C-0001");
    for (to, by) in [("edit", "desk"), ("proof", ANA)] {
        ok(&pair, &["card", "move", "C-0001", "--to", to, "--as", by]);
    }
    let accepted = accept(&pair, "C-0001", ANA);
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    assert_eq!(show(&pair, "C-0001")["column"], json!("filed"));
    assert_eq!(others(&pair, "C-0001"), rest, "both children as they were");
}
