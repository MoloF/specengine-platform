//! docs/features/task-mandate.md (RM-0104, v5 step 6d, ADR-0079)
//! "Acceptance criteria", the CLI half: `spec card` through the binary on
//! scratch git repositories of `fixtures/studio-lamp` (the preset's flows,
//! `sound`'s return column `sent-back`), the canon
//! docs/canon/card-mandate.md "The rule".
//!
//! - M1: `card new --mandate open` with a reviewer stores it, `show --json`
//!   holds it right after `reviewer`, both `card.created` carry `mandate`
//!   (`null` and `open`); `card list` ends `C-0002`'s line `| mandate open`
//!   and prints `C-0001`'s byte-equal to 8a43e5d's; a row is 22 keys;
//!   `--mandate binding` with no reviewer exits 1 with the line, nothing
//!   written, the next card `C-0003`; `--mandate maybe` exits 2; a stored
//!   `maybe` stops `show` (exit 2 naming `mandate`) and is a note in
//!   `list`. M: the store not asking `mandate_problem`; `card.created`
//!   carrying `mandate` only when set; the list suffix printed for null.
//! - M2: the split's `--mandate D=M` per child by discipline; the three
//!   `card.created`; the children's `mandate` last; the text's subtask
//!   lines; each refusal with its line, nothing written; a dry run judges
//!   it; `--mandate` with no `--child` exits 2; a child named by none holds
//!   `null`; a stored parent byte-equal. M: mandates paired by position; a
//!   child without one taking its neighbour's.
//! - M3: only the reviewer changes it: one `card.edited` with `mandate`;
//!   another name, no `--as` (`local`), and a title with it refused with no
//!   event; `--mandate -` exits 2; another field's edit keeps its payload;
//!   `assign` leaves the mandate; a cancelled card's closed line. M: the
//!   compare reading `assignee`; the check skipped when the edit holds
//!   another field; `card.edited` without `mandate`.
//! - M4: nothing reads it: three hand-offs alike but for the mandate run
//!   the same moves, return, accept and cancel with equal documents;
//!   decisions without `why` and questions stored on each; the gate's
//!   verdict the same before and after. M: `allowed` dropping the return
//!   move on a binding card; `create_intake` requiring `why` on an open
//!   card's decision.
//! - M6 (the CLI part): `show --json` serves `subtask_items` after `items`
//!   in set 122's order, `[]` with no child; the text lines; `list`
//!   byte-equal before and after items are raised on the children. M:
//!   `subtask_items` taking grandchildren.
//!
//! The oracle of "nothing written" is `SELECT * FROM cards` and
//! `SqliteQueue::dump()`, both raw. Every expected value is worked out from
//! the spec, never read back from a run; 8a43e5d's list line was recorded
//! from that build before this slice's code.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use common::proposal::Pair;
use common::{read_text, write};
use serde_json::{Value, json};
use task_common::Ordered;

/// The game of the preset's flows.
const LAMP: &str = "studio-lamp";

/// The writer who hands off and reviews.
const ANA: &str = "Ana Ruiz";

/// Another person.
const BEN: &str = "Ben Ode";

/// The first line of "Messages".
const NO_REVIEWER: &str =
    "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer";

/// A `list` row's 22 keys, `mandate` after `reviewer` (set 91).
const ROW_KEYS: [&str; 22] = [
    "id",
    "flow",
    "column",
    "position",
    "title",
    "discipline",
    "targets",
    "assignee",
    "reviewer",
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
];

/// The card document's 34 keys, `mandate` after `reviewer`,
/// `subtask_items` after `items` (set 92).
const CARD_KEYS: [&str; 34] = [
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
    "snapshot",
    "criteria",
    "assignee",
    "reviewer",
    "mandate",
    "estimate",
    "due",
    "parent",
    "corrects",
    "evidence",
    "tasks",
    "items",
    "subtask_items",
    "acceptance",
    "cancellation",
    "created_by",
    "created_at",
    "updated_at",
    "revision",
    "allowed",
    "history",
    "notes",
];

/// A subtask item's 17 keys: `card_id`, then `CardItem`'s 16 (set 122).
const SUBTASK_ITEM_KEYS: [&str; 17] = [
    "card_id",
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

/// A split child's 9 keys, `mandate` last (set 121).
const CHILD_KEYS: [&str; 9] = [
    "id",
    "discipline",
    "flow",
    "flow_title",
    "column",
    "targets",
    "from",
    "suggested",
    "mandate",
];

/// studio-lamp's board lines of `spec card list` with C-0001 in
/// `narrative/backlog` and C-0002 in `sound/backlog`, as 8a43e5d printed
/// them.
const BOARD_ONE_EACH: &str = "flow narrative | backlog 1 | sent-back 0 | outline 0 | draft 0 | \
     review 0 | approved 0 | ready-for-dev 0 | in-game 0\n\
     flow development | backlog 0 | sent-back 0 | ready 0 | in-progress 0 | in-review 0 | verify \
     0 | done 0\n\
     flow art | waiting 0 | sent-back 0 | ready 0 | in-progress 0 | pending-review 0 | approved 0 \
     | in-engine 0\n\
     flow level-design | layout 0 | sent-back 0 | blockout 0 | playtest 0 | scripting 0 | \
     lighting 0 | art-pass 0 | final 0\n\
     flow production | to-do 0 | sent-back 0 | in-progress 0 | review 0 | done 0\n\
     flow sound | backlog 1 | sent-back 0 | design 0 | recording 0 | implementation 0 | review 0 \
     | in-game 0\n";

/// `C-0001`'s `list` line, recorded from 8a43e5d's build.
const C1_LINE: &str = "C-0001 | narrative/backlog | 1 | Lamp room scene | narrative | - | - | \
                       SCN-ARRIVAL";

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

/// `(type, payload)` of every `card.*` event, in `seq` order.
fn events(pair: &Pair) -> Vec<(String, Value)> {
    pair.events()
        .into_iter()
        .filter(|event| event.event_type.starts_with("card."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The last event's `seq`, `0` with none.
fn last_seq(pair: &Pair) -> i64 {
    pair.events().last().map_or(0, |event| event.seq)
}

/// The raw row of `id`.
fn row(pair: &Pair, id: &str) -> String {
    pair.sql(&format!("SELECT * FROM cards WHERE id = '{id}';"))
}

/// `show --history --json` of `id`.
fn show(pair: &Pair, id: &str) -> Value {
    doc(pair, &["card", "show", id, "--history"])
}

/// The stored `revision` of `id`, as the document serves it.
fn revision(pair: &Pair, id: &str) -> String {
    doc(pair, &["card", "show", id])["revision"]
        .as_str()
        .expect("a revision")
        .to_owned()
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

/// M1's two cards: `C-0001` the narrative card, `C-0002` its sound hand-off
/// to Ana Ruiz with `--mandate open`; the run of the second.
fn m1_cards(pair: &Pair) -> common::Run {
    let parent = make(
        pair,
        "Lamp room scene",
        &[
            "--discipline",
            "narrative",
            "--target",
            "SCN-ARRIVAL",
            "--as",
            ANA,
        ],
    );
    assert_eq!(parent, "C-0001");
    json_run(
        pair,
        &[
            "card",
            "new",
            "--title",
            "Keeper's voice",
            "--discipline",
            "sound",
            "--target",
            "SCN-ARRIVAL",
            "--parent",
            "C-0001",
            "--reviewer",
            ANA,
            "--mandate",
            "open",
            "--as",
            ANA,
        ],
    )
}

/// A sound hand-off of `C-0001` to Ana Ruiz, `mandate` when given: its ID.
fn hand_off(pair: &Pair, title: &str, mandate: Option<&str>) -> String {
    let mut flags = vec![
        "--discipline",
        "sound",
        "--target",
        "SCN-ARRIVAL",
        "--parent",
        "C-0001",
        "--reviewer",
        ANA,
        "--as",
        ANA,
    ];
    if let Some(mandate) = mandate {
        flags.extend_from_slice(&["--mandate", mandate]);
    }
    make(pair, title, &flags)
}

/// `spec card accept id --as name --briefed` on a terminal, answering `y`.
fn accept(pair: &Pair, id: &str, name: &str) -> common::decision::PtyRun {
    pair.spec_pty(
        &pair.main,
        &["card", "accept", id, "--as", name, "--briefed"],
        Some("y"),
    )
}

/// M2's split command, `extra` appended.
fn storm<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "card",
        "split",
        "--target",
        "SCN-STORM",
        "--title",
        "A new keeper",
        "--discipline",
        "narrative",
    ];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--reviewer", ANA, "--as", ANA]);
    args
}

/// The lines of `text` starting with `prefix`.
fn lines_of<'t>(text: &'t str, prefix: &str) -> Vec<&'t str> {
    text.lines()
        .filter(|line| line.starts_with(prefix))
        .collect()
}

/// The 0-based line of `text` that starts with `prefix`.
fn at(text: &str, prefix: &str) -> usize {
    text.lines()
        .position(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("no {prefix:?} in {text}"))
}

// ------------------------------------------------------------------ M1

/// M1: the criterion's two cards: `C-0002` holds `mandate` `open` right
/// after `reviewer` in a document of 34 keys; the two `card.created` carry
/// `null` and `open`. M: `card.created` carrying `mandate` only when set.
#[test]
fn m1_new_names_the_mandate_and_card_created_carries_it() {
    let pair = Pair::new("cm-m1-new", LAMP);
    let made = m1_cards(&pair);
    assert_eq!(made.json()["id"], json!("C-0002"), "{}", made.stdout);

    let shown = json_run(&pair, &["card", "show", "C-0002"]);
    let ordered = Ordered::of(&shown.stdout);
    assert_eq!(ordered.keys(), CARD_KEYS, "{}", shown.stdout);
    let card = shown.json();
    assert_eq!(card["mandate"], json!("open"));
    assert_eq!(card["reviewer"], json!(ANA));
    assert_eq!(show(&pair, "C-0001")["mandate"], Value::Null);
    assert_eq!(
        events(&pair),
        [
            (
                "card.created".to_owned(),
                json!({"id": "C-0001", "flow": "narrative", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": null,
                       "mandate": null, "as": ANA})
            ),
            (
                "card.created".to_owned(),
                json!({"id": "C-0002", "flow": "sound", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": ANA,
                       "mandate": "open", "as": ANA})
            ),
        ]
    );

    // The text: `mandate:` after `reviewer:`, `-` for none.
    let text = ok(&pair, &["card", "show", "C-0002"]);
    assert_eq!(
        at(&text, "mandate: "),
        at(&text, "reviewer: ") + 1,
        "{text}"
    );
    assert_eq!(lines_of(&text, "mandate: "), ["mandate: open"], "{text}");
    let none = ok(&pair, &["card", "show", "C-0001"]);
    assert_eq!(lines_of(&none, "mandate: "), ["mandate: -"], "{none}");
    assert_eq!(
        at(&none, "mandate: "),
        at(&none, "reviewer: ") + 1,
        "{none}"
    );
}

/// M1: `card list`: `C-0002`'s line ends `| mandate open`; `C-0001`'s line
/// and the board are byte-equal to 8a43e5d's; the JSON row holds 22 keys,
/// `mandate` after `reviewer`. A mandate on a card with no parent prints
/// the same suffix. M: the list suffix printed for null (`| mandate -`).
#[test]
fn m1_list_prints_the_suffix_only_when_set() {
    let pair = Pair::new("cm-m1-list", LAMP);
    m1_cards(&pair);
    assert_eq!(
        ok(&pair, &["card", "list"]),
        format!(
            "{BOARD_ONE_EACH}{C1_LINE}\nC-0002 | sound/backlog | 1 | Keeper's voice | sound | - \
             | - | SCN-ARRIVAL | mandate open\n"
        )
    );
    let listed = json_run(&pair, &["card", "list"]);
    let ordered = Ordered::of(&listed.stdout);
    for index in 0..2 {
        assert_eq!(
            ordered.get("cards").at(index).keys(),
            ROW_KEYS,
            "{}",
            listed.stdout
        );
    }
    let rows = listed.json();
    assert_eq!(rows["cards"][0]["mandate"], Value::Null);
    assert_eq!(rows["cards"][1]["mandate"], json!("open"));

    // A mandate and no parent: nothing differs but the word.
    let alone = make(
        &pair,
        "Foghorn",
        &[
            "--discipline",
            "sound",
            "--reviewer",
            ANA,
            "--mandate",
            "binding",
            "--as",
            ANA,
        ],
    );
    assert_eq!(alone, "C-0003");
    let text = ok(&pair, &["card", "list"]);
    assert!(
        text.lines().any(|line| line
            == "C-0003 | sound/backlog | 2 | Foghorn | sound | - | - | - | mandate binding"),
        "{text}"
    );
    assert!(!text.contains("mandate -"), "{text}");
}

/// M1: `--mandate binding` without `--reviewer` exits 1 with the line,
/// nothing written, and the next card takes `C-0003`; `--mandate maybe` and
/// `--mandate Binding` exit 2 (clap's possible values). M: the store not
/// asking `mandate_problem` is the store's test; the CLI's check is this.
#[test]
fn m1_a_mandate_without_a_reviewer_is_refused() {
    let pair = Pair::new("cm-m1-refused", LAMP);
    m1_cards(&pair);
    let before = state(&pair);
    let base = [
        "card",
        "new",
        "--title",
        "Keeper's coat",
        "--discipline",
        "art",
        "--parent",
        "C-0001",
        "--as",
        ANA,
    ];
    let mut args = base.to_vec();
    args.extend_from_slice(&["--mandate", "binding"]);
    assert_eq!(refused(&pair, &args), format!("spec: {NO_REVIEWER}\n"));
    assert_eq!(state(&pair), before, "nothing written");
    let mut json_args = vec!["--json"];
    json_args.extend_from_slice(&args);
    run(&pair, &json_args).code(1);
    assert_eq!(state(&pair), before, "nothing written");

    for word in ["maybe", "Binding", "-", ""] {
        let mut args = base.to_vec();
        args.extend_from_slice(&["--reviewer", ANA, "--mandate", word]);
        let message = cannot(&pair, &args);
        assert!(message.contains("--mandate"), "{word:?}: {message}");
        assert_eq!(state(&pair), before, "{word:?}: nothing written");
    }
    assert_eq!(
        make(&pair, "Keeper's coat", &["--discipline", "art"]),
        "C-0003"
    );
}

/// M1: `UPDATE cards SET mandate = 'maybe'`: `show` exits 2 naming
/// `mandate`; `list` exits 0 with a note naming the card and lists the
/// other.
#[test]
fn m1_a_corrupt_mandate_stops_show_and_is_a_note_in_list() {
    let pair = Pair::new("cm-m1-corrupt", LAMP);
    m1_cards(&pair);
    pair.sql("UPDATE cards SET mandate = 'maybe' WHERE id = 'C-0002';");
    let message = cannot(&pair, &["card", "show", "C-0002"]);
    assert!(
        message.contains("C-0002") && message.contains("mandate"),
        "{message}"
    );
    let document = doc(&pair, &["card", "list"]);
    let ids: Vec<&str> = document["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, ["C-0001"]);
    let notes = document["notes"].to_string();
    assert!(
        notes.contains("C-0002") && notes.contains("mandate"),
        "{notes}"
    );
    let text = run(&pair, &["card", "list"]);
    text.code(0);
    assert!(text.stderr.contains("C-0002"), "{}", text.show());

    pair.sql("UPDATE cards SET mandate = 'maybe';");
    let all = doc(&pair, &["card", "list"]);
    assert_eq!(all["cards"], json!([]));
    assert_eq!(all["notes"].as_array().map(Vec::len), Some(2));
}

// ------------------------------------------------------------------ M2

/// M2: the criterion's split: `C-0001` the parent with no mandate,
/// `C-0002` `art` with `binding`, `C-0003` `sound` with `open`, though
/// `--mandate` names `sound` first; three `card.created` with `null`,
/// `binding`, `open`; the children's `mandate` last; the text's subtask
/// lines end with the word. M: mandates paired by position (`art` then
/// reads `open`).
#[test]
fn m2_a_split_names_a_mandate_per_subtask() {
    let pair = Pair::new("cm-m2-split", LAMP);
    let args = storm(&[
        "--child",
        "art",
        "--child",
        "sound",
        "--mandate",
        "sound=open",
        "--mandate",
        "art=binding",
    ]);
    let done = json_run(&pair, &args);
    let ordered = Ordered::of(&done.stdout);
    let document = done.json();
    let children = document["children"].as_array().expect("children");
    assert_eq!(children.len(), 2, "{document}");
    for index in 0..2 {
        assert_eq!(
            ordered.get("children").at(index).keys(),
            CHILD_KEYS,
            "{}",
            done.stdout
        );
    }
    assert_eq!(children[0]["id"], json!("C-0002"));
    assert_eq!(children[0]["discipline"], json!("art"));
    assert_eq!(children[0]["mandate"], json!("binding"));
    assert_eq!(children[1]["id"], json!("C-0003"));
    assert_eq!(children[1]["discipline"], json!("sound"));
    assert_eq!(children[1]["mandate"], json!("open"));

    assert_eq!(show(&pair, "C-0001")["mandate"], Value::Null);
    assert_eq!(show(&pair, "C-0002")["mandate"], json!("binding"));
    assert_eq!(show(&pair, "C-0003")["mandate"], json!("open"));
    let created: Vec<(Value, Value)> = events(&pair)
        .into_iter()
        .filter(|(kind, _)| kind == "card.created")
        .map(|(_, payload)| (payload["id"].clone(), payload["mandate"].clone()))
        .collect();
    assert_eq!(
        created,
        [
            (json!("C-0001"), Value::Null),
            (json!("C-0002"), json!("binding")),
            (json!("C-0003"), json!("open")),
        ]
    );
    for (_, payload) in events(&pair) {
        assert!(payload.get("mandate").is_some(), "{payload}");
    }
}

/// M2: the text's subtask lines end ` | mandate <m>` when set, and with no
/// suffix when not: the same split with `art` named by no `--mandate`.
#[test]
fn m2_the_texts_subtask_lines_end_with_the_mandate() {
    let pair = Pair::new("cm-m2-text", LAMP);
    let text = ok(
        &pair,
        &storm(&[
            "--child",
            "art",
            "--child",
            "sound",
            "--mandate",
            "sound=open",
            "--mandate",
            "art=binding",
        ]),
    );
    let subtasks = lines_of(&text, "subtask ");
    assert_eq!(subtasks.len(), 2, "{text}");
    assert!(subtasks[0].starts_with("subtask C-0002: art/"), "{text}");
    assert!(subtasks[0].ends_with(" | mandate binding"), "{text}");
    assert!(subtasks[1].starts_with("subtask C-0003: sound/"), "{text}");
    assert!(subtasks[1].ends_with(" | mandate open"), "{text}");
    assert!(!lines_of(&text, "split C-0001").is_empty(), "{text}");
    assert!(
        !lines_of(&text, "split ")[0].contains("mandate"),
        "the parent takes none: {text}"
    );

    let other = Pair::new("cm-m2-text-none", LAMP);
    let text = ok(
        &other,
        &storm(&[
            "--child",
            "art",
            "--child",
            "sound",
            "--mandate",
            "sound=open",
        ]),
    );
    let subtasks = lines_of(&text, "subtask ");
    assert!(!subtasks[0].contains("mandate"), "{text}");
    assert!(subtasks[1].ends_with(" | mandate open"), "{text}");
}

/// M2: each refusal exits 1 with its line, nothing written, the `--json`
/// document's last note the line.
#[test]
fn m2_each_refusal_exits_1_with_its_line() {
    let pair = Pair::new("cm-m2-refused", LAMP);
    make(&pair, "T", &["--discipline", "narrative"]);
    let before = state(&pair);
    let grammar = "mandate: `Art` is not a name: lower-case ASCII letters, digits, `_` and `-`";
    for (mandates, reason) in [
        (
            &["--mandate", "voice=open"][..],
            "mandate: `voice` is no --child of this split",
        ),
        (
            &["--mandate", "art=open", "--mandate", "art=binding"][..],
            "mandate: `art` is named twice",
        ),
        (
            &["--mandate", "art"][..],
            "mandate: `art` is not `<discipline>=binding` or `<discipline>=open`",
        ),
        (&["--mandate", "Art=open"][..], grammar),
        (
            &["--mandate", "art=maybe"][..],
            "mandate: `art=maybe` is not `<discipline>=binding` or `<discipline>=open`",
        ),
    ] {
        let mut extra = vec!["--child", "art", "--child", "sound"];
        extra.extend_from_slice(mandates);
        let args = storm(&extra);
        assert_eq!(
            refused(&pair, &args),
            format!("spec: {reason}\n"),
            "{mandates:?}"
        );
        assert_eq!(state(&pair), before, "{mandates:?}: nothing written");
        let mut all = vec!["--json"];
        all.extend_from_slice(&args);
        let done = run(&pair, &all);
        done.code(1);
        let notes = done.json()["notes"].clone();
        assert_eq!(
            notes.as_array().and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{mandates:?}: {}",
            done.stdout
        );
        assert_eq!(state(&pair), before, "{mandates:?}: nothing written");
    }
    assert_eq!(make(&pair, "U", &["--discipline", "narrative"]), "C-0002");
}

/// M2: a dry run judges the mandates and writes nothing: the plan's child
/// holds `binding`, the last event's `seq` unchanged; a refused mandate
/// exits 1 in a dry run too; `--mandate` with no `--child` exits 2 with the
/// usage line.
#[test]
fn m2_a_dry_run_judges_the_mandates() {
    let pair = Pair::new("cm-m2-dry", LAMP);
    make(&pair, "T", &["--discipline", "narrative"]);
    let seq = last_seq(&pair);
    let before = state(&pair);
    let plan = doc(
        &pair,
        &storm(&["--dry-run", "--child", "art", "--mandate", "art=binding"]),
    );
    assert_eq!(plan["dry_run"], json!(true));
    assert_eq!(plan["children"][0]["discipline"], json!("art"));
    assert_eq!(plan["children"][0]["mandate"], json!("binding"));
    assert_eq!(last_seq(&pair), seq, "nothing logged");
    assert_eq!(state(&pair), before, "nothing written");

    let unnamed = doc(&pair, &storm(&["--dry-run", "--child", "art"]));
    assert_eq!(unnamed["children"][0]["mandate"], Value::Null);

    assert_eq!(
        refused(
            &pair,
            &storm(&["--dry-run", "--child", "art", "--mandate", "voice=open"])
        ),
        "spec: mandate: `voice` is no --child of this split\n"
    );
    let message = cannot(&pair, &storm(&["--dry-run", "--mandate", "art=binding"]));
    assert!(
        message.contains("--mandate names a subtask: give its --child too"),
        "{message}"
    );
    assert_eq!(state(&pair), before, "nothing written");
}

/// M2, usage: `--mandate` with no `--child`, with `--dry-run` or without it,
/// exits 2 with the mandate's own line (the review's nit: it wins over the
/// no-`--child` line); with neither flag the old line stays. Nothing written.
/// M: the no-`--child` line checked first.
#[test]
fn m2_a_mandate_without_a_child_is_a_usage_error() {
    let pair = Pair::new("cm-m2-usage", LAMP);
    make(&pair, "T", &["--discipline", "narrative"]);
    let before = state(&pair);
    for extra in [
        &["--mandate", "art=binding"][..],
        &["--dry-run", "--mandate", "art=binding"][..],
        &["--mandate", "Art"][..],
    ] {
        let message = cannot(&pair, &storm(extra));
        assert_eq!(
            message, "spec: --mandate names a subtask: give its --child too\n",
            "{extra:?}"
        );
        assert_eq!(state(&pair), before, "{extra:?}: nothing written");
    }
    let message = cannot(&pair, &storm(&[]));
    assert!(
        message.contains("name each subtask's discipline with --child, or plan with --dry-run"),
        "{message}"
    );
    assert_eq!(state(&pair), before);
}

/// M2: a child named by no `--mandate` holds `null`, its neighbours their
/// own; a split under the stored parent `C-0001` leaves its row and
/// `revision` byte-equal. M: a child without one taking its neighbour's.
#[test]
fn m2_a_child_named_by_none_holds_null_and_a_stored_parent_is_unwritten() {
    let pair = Pair::new("cm-m2-none", LAMP);
    let document = doc(
        &pair,
        &storm(&[
            "--child",
            "art",
            "--child",
            "level-design",
            "--child",
            "sound",
            "--mandate",
            "sound=open",
        ]),
    );
    let mandates: Vec<Value> = document["children"]
        .as_array()
        .expect("children")
        .iter()
        .map(|child| child["mandate"].clone())
        .collect();
    assert_eq!(mandates, [Value::Null, Value::Null, json!("open")]);
    assert_eq!(show(&pair, "C-0002")["mandate"], Value::Null);
    assert_eq!(show(&pair, "C-0003")["mandate"], Value::Null);
    assert_eq!(show(&pair, "C-0004")["mandate"], json!("open"));

    let parent = row(&pair, "C-0001");
    let parent_revision = revision(&pair, "C-0001");
    let under = doc(
        &pair,
        &[
            "card",
            "split",
            "--parent",
            "C-0001",
            "--child",
            "sound",
            "--mandate",
            "sound=open",
            "--as",
            ANA,
        ],
    );
    assert_eq!(under["children"][0]["id"], json!("C-0005"));
    assert_eq!(under["children"][0]["mandate"], json!("open"));
    assert_eq!(row(&pair, "C-0001"), parent, "the stored parent's row");
    assert_eq!(revision(&pair, "C-0001"), parent_revision);
    assert_eq!(show(&pair, "C-0005")["mandate"], json!("open"));
}

// ------------------------------------------------------------------ M3

/// M3: after M1's two cards, Ana Ruiz, the reviewer, sets `binding`:
/// `revision` 2 and exactly one new event, `card.edited` `{id, fields:
/// ["mandate"], as, mandate}`. Ben Ode, no `--as` (`local`), and a title
/// with the mandate by Ben are refused with their lines, no event,
/// `revision` unchanged, the title unwritten. M: the compare reading
/// `assignee`; the check skipped when the edit holds another field;
/// `card.edited` without `mandate`.
#[test]
fn m3_only_the_reviewer_changes_the_mandate() {
    let pair = Pair::new("cm-m3-edit", LAMP);
    m1_cards(&pair);
    // Ben is the assignee: the compare must read the reviewer.
    ok(
        &pair,
        &["card", "assign", "C-0002", "--assignee", BEN, "--as", ANA],
    );
    let before_edit = events(&pair).len();
    let revision_before = revision(&pair, "C-0002");
    assert_eq!(revision_before, "2");

    let other = |actor: &str| {
        format!(
            "spec: mandate: only C-0002's reviewer `{ANA}` changes its mandate, not `{actor}`; \
             nothing changed\n"
        )
    };
    let before = state(&pair);
    for (args, actor) in [
        (
            vec![
                "card",
                "edit",
                "C-0002",
                "--mandate",
                "binding",
                "--as",
                BEN,
            ],
            BEN,
        ),
        (
            vec!["card", "edit", "C-0002", "--mandate", "binding"],
            "local",
        ),
        (
            vec![
                "card",
                "edit",
                "C-0002",
                "--title",
                "X",
                "--mandate",
                "open",
                "--as",
                BEN,
            ],
            BEN,
        ),
    ] {
        assert_eq!(refused(&pair, &args), other(actor), "{args:?}");
        assert_eq!(state(&pair), before, "{args:?}: nothing written");
    }
    assert_eq!(show(&pair, "C-0002")["title"], json!("Keeper's voice"));
    assert_eq!(revision(&pair, "C-0002"), revision_before);

    let message = cannot(&pair, &["card", "edit", "C-0002", "--mandate", "-"]);
    assert!(message.contains("--mandate"), "{message}");
    assert_eq!(state(&pair), before);

    let edited = doc(
        &pair,
        &[
            "card",
            "edit",
            "C-0002",
            "--mandate",
            "binding",
            "--as",
            ANA,
        ],
    );
    assert_eq!(edited["revision"], json!("3"), "{edited}");
    let after = events(&pair);
    assert_eq!(after.len(), before_edit + 1, "exactly one new event");
    assert_eq!(
        after[before_edit],
        (
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": ANA, "mandate": "binding"})
        )
    );
    assert_eq!(show(&pair, "C-0002")["mandate"], json!("binding"));
}

/// M3, the criterion as written (no assignee step): `edit --mandate binding
/// --as "Ana Ruiz" --json` reads `revision` 2.
#[test]
fn m3_the_reviewers_edit_is_revision_2() {
    let pair = Pair::new("cm-m3-rev", LAMP);
    m1_cards(&pair);
    let edited = doc(
        &pair,
        &[
            "card",
            "edit",
            "C-0002",
            "--mandate",
            "binding",
            "--as",
            ANA,
        ],
    );
    assert_eq!(edited["revision"], json!("2"), "{edited}");
    assert_eq!(edited["id"], json!("C-0002"));
}

/// M3: another field's edit keeps today's payload exactly; a title and the
/// mandate by the reviewer name both fields; the same mandate again is an
/// edit like any.
#[test]
fn m3_card_edited_gains_mandate_only_when_fields_hold_it() {
    let pair = Pair::new("cm-m3-payload", LAMP);
    m1_cards(&pair);
    ok(
        &pair,
        &["card", "edit", "C-0002", "--title", "X", "--as", BEN],
    );
    assert_eq!(
        events(&pair).last().cloned(),
        Some((
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["title"], "as": BEN})
        ))
    );
    ok(
        &pair,
        &[
            "card",
            "edit",
            "C-0002",
            "--title",
            "Y",
            "--mandate",
            "open",
            "--as",
            ANA,
        ],
    );
    assert_eq!(
        events(&pair).last().cloned(),
        Some((
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["title", "mandate"], "as": ANA,
                   "mandate": "open"})
        ))
    );
    assert_eq!(revision(&pair, "C-0002"), "3");
    // The mandate it already holds: written as any edit.
    ok(
        &pair,
        &["card", "edit", "C-0002", "--mandate", "open", "--as", ANA],
    );
    assert_eq!(revision(&pair, "C-0002"), "4");
    assert_eq!(
        events(&pair).last().cloned(),
        Some((
            "card.edited".to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": ANA, "mandate": "open"})
        ))
    );
}

/// M3: `assign --reviewer -` leaves the mandate `open`, and Ana's edit then
/// gives the no-reviewer line; with Ben Ode assigned, Ben's edit stands. A
/// re-estimate takes no `--mandate`. A cancelled card gives today's closed
/// line.
#[test]
fn m3_assign_leaves_the_mandate_and_a_closed_card_refuses() {
    let pair = Pair::new("cm-m3-assign", LAMP);
    m1_cards(&pair);
    ok(
        &pair,
        &["card", "assign", "C-0002", "--reviewer", "-", "--as", ANA],
    );
    let card = show(&pair, "C-0002");
    assert_eq!(card["reviewer"], Value::Null);
    assert_eq!(card["mandate"], json!("open"), "the mandate stays");
    let before = state(&pair);
    assert_eq!(
        refused(
            &pair,
            &[
                "card",
                "edit",
                "C-0002",
                "--mandate",
                "binding",
                "--as",
                ANA
            ]
        ),
        "spec: mandate: C-0002 has no reviewer, who alone changes its mandate: name one with \
         `spec card assign C-0002 --reviewer N` first; nothing changed\n"
    );
    assert_eq!(state(&pair), before);

    ok(
        &pair,
        &["card", "assign", "C-0002", "--reviewer", BEN, "--as", ANA],
    );
    assert_eq!(show(&pair, "C-0002")["mandate"], json!("open"));
    ok(
        &pair,
        &[
            "card",
            "edit",
            "C-0002",
            "--mandate",
            "binding",
            "--as",
            BEN,
        ],
    );
    assert_eq!(show(&pair, "C-0002")["mandate"], json!("binding"));

    // A re-estimate takes no other field, `--mandate` included.
    ok(
        &pair,
        &["card", "edit", "C-0002", "--estimate", "2", "--as", BEN],
    );
    let before = state(&pair);
    let reestimate = run(
        &pair,
        &[
            "card",
            "edit",
            "C-0002",
            "--estimate",
            "3",
            "--reason",
            "Two more takes",
            "--mandate",
            "open",
            "--as",
            BEN,
        ],
    );
    assert_ne!(reestimate.code, 0, "{}", reestimate.show());
    assert_eq!(state(&pair), before, "{}", reestimate.show());

    ok(
        &pair,
        &["card", "cancel", "C-0002", "--reason", "Cut", "--as", BEN],
    );
    let before = state(&pair);
    assert_eq!(
        refused(
            &pair,
            &["card", "edit", "C-0002", "--mandate", "open", "--as", BEN]
        ),
        "spec: C-0002 is cancelled: it takes no write; a correction is a new card, `spec card \
         new --corrects C-0002`; nothing changed\n"
    );
    assert_eq!(state(&pair), before);
}

// ------------------------------------------------------------------ M4

/// A card's document with its mandate and every time set aside, at any
/// depth: what must be equal across hand-offs alike but for the mandate.
fn without_mandate(mut card: Value) -> Value {
    assert!(card.get("mandate").is_some(), "{card}");
    strip(&mut card);
    card
}

fn strip(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for key in ["mandate", "at", "created_at", "updated_at"] {
                object.remove(key);
            }
            object.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// One hand-off's life on its own pair, `mandate` its only difference:
/// every step's write document, its `allowed`, column and position, and
/// the show after each, then a second hand-off cancelled.
fn life(label: &str, mandate: Option<&str>) -> Vec<Value> {
    let pair = Pair::new(label, LAMP);
    make(
        &pair,
        "Lamp room scene",
        &[
            "--discipline",
            "narrative",
            "--target",
            "SCN-ARRIVAL",
            "--as",
            ANA,
        ],
    );
    let id = hand_off(&pair, "Keeper's voice", mandate);
    let cut = hand_off(&pair, "Keeper's cough", mandate);
    assert_eq!((id.as_str(), cut.as_str()), ("C-0002", "C-0003"));
    let mut seen = vec![without_mandate(show(&pair, &id))];
    let steps: [&[&str]; 3] = [
        &["card", "move", &id, "--to", "design", "--as", "Rob"],
        &[
            "card",
            "move",
            &id,
            "--to",
            "sent-back",
            "--note",
            "The voice is too bright",
            "--as",
            "Rob",
        ],
        &["card", "move", &id, "--to", "review", "--as", "Rob"],
    ];
    for step in steps {
        seen.push(doc(&pair, step));
        seen.push(without_mandate(show(&pair, &id)));
    }
    let accepted = accept(&pair, &id, ANA);
    assert_eq!(accepted.code, 0, "{}", accepted.output);
    seen.push(without_mandate(show(&pair, &id)));
    seen.push(doc(
        &pair,
        &[
            "card",
            "cancel",
            &cut,
            "--reason",
            "Cut from the build",
            "--as",
            ANA,
        ],
    ));
    seen.push(without_mandate(show(&pair, &cut)));
    // A decision without `why`, and a question, stored on each.
    let decided = decide(&pair, &id, "Does the keeper's voice crack in the storm?");
    let asked = ask(&pair, &id, "Which accent does the keeper speak?");
    assert_eq!((decided.as_str(), asked.as_str()), ("PR-0001", "PR-0002"));
    seen.push(json!([decided, asked]));
    let card = show(&pair, &id);
    let items = card["items"].as_array().expect("items").len();
    assert_eq!(items, 2, "both stored on the card");
    seen.push(without_mandate(card));
    seen
}

/// M4: three hand-offs of `C-0001` in `sound`, alike but for `mandate`
/// (null, `binding`, `open`), each on its own copy so `C-0002`'s position
/// is its own: the move to `design`, the return to `sent-back` with a note,
/// the move to `review`, Ana's accept, a second hand-off's cancel, a
/// decision without `why` and a question: every document and show equal,
/// the mandate aside. M: `allowed` dropping the return move on a binding
/// card; `create_intake` requiring `why` on an open card's decision.
#[test]
fn m4_nothing_reads_the_mandate() {
    let none = life("cm-m4-none", None);
    let binding = life("cm-m4-binding", Some("binding"));
    let open = life("cm-m4-open", Some("open"));
    assert_eq!(none.len(), binding.len());
    for (index, ((a, b), c)) in none.iter().zip(&binding).zip(&open).enumerate() {
        assert_eq!(a, b, "step {index}: null and binding");
        assert_eq!(a, c, "step {index}: null and open");
    }
    // The return move stands on each.
    assert!(
        none[1]["allowed"]
            .as_array()
            .expect("allowed")
            .iter()
            .any(|step| step["to"] == json!("sent-back")),
        "{}",
        none[1]
    );
}

/// M4: with a `selective` gate, `spec gate` on a task's path answers the
/// same before any card, after a card with a mandate, after a mandate edit,
/// and with a corrupt stored mandate: the gate reads `tasks` alone.
#[test]
fn m4_the_gate_answers_the_same_before_and_after() {
    let pair = Pair::new("cm-m4-gate", LAMP);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        format!(
            "{config}\n[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = \
             [\"code\"]\n"
        ),
    );
    write(&pair.main, "src/a.rs", "fn a() {}\n");
    pair.commit_all(&pair.main, "the gate's scratch files");
    ok(
        &pair,
        &["task", "new", "--nodes", "SCN-ARRIVAL", "--title", "A task"],
    );
    let gate = gate_answer(&pair);
    let verdict: Value = serde_json::from_str(&gate.1).expect("the gate's document");
    assert_eq!(verdict["verdict"], "refuse", "{gate:?}");
    assert_eq!(verdict["cause"], Value::Null, "a real refusal: {gate:?}");

    m1_cards(&pair);
    assert_eq!(gate_answer(&pair), gate, "a mandate changes no verdict");
    ok(
        &pair,
        &[
            "card",
            "edit",
            "C-0002",
            "--mandate",
            "binding",
            "--as",
            ANA,
        ],
    );
    assert_eq!(
        gate_answer(&pair),
        gate,
        "a mandate edit changes no verdict"
    );
    pair.sql("UPDATE cards SET mandate = 'maybe' WHERE id = 'C-0002';");
    assert_eq!(gate_answer(&pair), gate, "a corrupt mandate changes none");
}

/// `spec gate --worktree main --path src/a.rs --json`: its exit and both
/// streams; a run a loaded machine starves past the deadline is asked
/// again (five tries; the last stands), as cards.rs asks it.
fn gate_answer(pair: &Pair) -> (i32, String, String) {
    let main = pair.main.to_str().expect("a UTF-8 path");
    let mut done = None;
    for _ in 0..5 {
        let answer = run(
            pair,
            &["gate", "--worktree", main, "--path", "src/a.rs", "--json"],
        );
        let starved = answer.stdout.contains("\"reason\":\"deadline\"")
            || answer.stderr.contains("no answer within");
        done = Some(answer);
        if !starved {
            break;
        }
    }
    let done = done.expect("a run");
    (done.code, done.stdout, done.stderr)
}

// ------------------------------------------------------------------ M6

/// The names `distinct_from` takes for every hit of an intake document.
fn hit_names(document: &Value) -> Vec<String> {
    document["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|hit| {
            hit["id"]
                .as_str()
                .or_else(|| hit["path"].as_str())
                .expect("a hit's name")
                .to_owned()
        })
        .collect()
}

/// `spec --json propose question SCN-STORM --text <text> …` for `card`,
/// naming `distinct`.
fn ask_run(pair: &Pair, card: &str, text: &str, distinct: &[String]) -> Value {
    let mut args = vec![
        "propose",
        "question",
        "SCN-STORM",
        "--text",
        text,
        "--working-answer",
        "A low alto.",
        "--price-of-other",
        "Two scenes re-recorded.",
        "--author-role",
        "developer",
        "--card",
        card,
    ];
    for name in distinct {
        args.extend_from_slice(&["--distinct-from", name]);
    }
    doc(pair, &args)
}

/// A question for `card` on `SCN-STORM`, stored (asked again naming every
/// hit when it hits): its ID.
fn ask(pair: &Pair, card: &str, text: &str) -> String {
    let mut raised = ask_run(pair, card, text, &[]);
    if raised["created"] != json!(true) {
        raised = ask_run(pair, card, text, &hit_names(&raised));
    }
    assert_eq!(raised["created"], json!(true), "{raised}");
    raised["id"].as_str().expect("an ID").to_owned()
}

/// `spec --json propose decision --input -` for `card` on `SCN-STORM`, no
/// `why`, naming `distinct`.
fn decide_run(pair: &Pair, card: &str, text: &str, distinct: &[String]) -> Value {
    let input = json!({
        "node_ids": ["SCN-STORM"], "text": text, "working_answer": "Yes.",
        "price_of_other": "A scene redrawn.", "distinct_from": distinct
    });
    let done = pair.spec_piped(
        &pair.main,
        &[
            "--json",
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
            "--card",
            card,
        ],
        input.to_string().as_bytes(),
    );
    done.code(0);
    done.json()
}

/// A decision for `card` with no `why`, stored (raised again naming every
/// hit when it hits): its ID.
fn decide(pair: &Pair, card: &str, text: &str) -> String {
    let mut raised = decide_run(pair, card, text, &[]);
    if raised["created"] != json!(true) {
        raised = decide_run(pair, card, text, &hit_names(&raised));
    }
    assert_eq!(raised["created"], json!(true), "{raised}");
    raised["id"].as_str().expect("an ID").to_owned()
}

/// M6, the CLI: the split's parent `C-0001` and its children `C-0002`
/// (`art`, binding) and `C-0003` (`sound`, open); a grandchild `C-0004`
/// under `C-0002`. `show C-0001 --json` serves `subtask_items` right after
/// `items`, each entry in set 122's order, the children's items by ID with
/// their `card_id`, the parent's own and the grandchild's left out; a card
/// with no child serves `[]`; the text's lines after `items:`; `list`
/// byte-equal before and after. M: `subtask_items` taking grandchildren.
#[test]
fn m6_show_serves_the_childrens_items() {
    let pair = Pair::new("cm-m6-show", LAMP);
    ok(
        &pair,
        &storm(&[
            "--child",
            "art",
            "--child",
            "sound",
            "--mandate",
            "art=binding",
            "--mandate",
            "sound=open",
        ]),
    );
    make(
        &pair,
        "The stair",
        &[
            "--discipline",
            "level-design",
            "--target",
            "SCN-STORM",
            "--parent",
            "C-0002",
            "--reviewer",
            ANA,
            "--as",
            ANA,
        ],
    );
    let list_text = ok(&pair, &["card", "list"]);
    let list_json = json_run(&pair, &["card", "list"]).stdout;

    let own = ask(&pair, "C-0001", "Which lamp burns in the storm?");
    let sound = decide(&pair, "C-0003", "Does the keeper sing in the storm?");
    let grandchild = decide(&pair, "C-0004", "Does the stair turn left?");
    let art = ask(&pair, "C-0002", "Is the keeper's coat oilskin?");
    assert_eq!(
        [own.as_str(), &sound, &grandchild, &art],
        ["PR-0001", "PR-0002", "PR-0003", "PR-0004"],
        "premise"
    );

    let shown = json_run(&pair, &["card", "show", "C-0001"]);
    let ordered = Ordered::of(&shown.stdout);
    assert_eq!(ordered.keys(), CARD_KEYS, "{}", shown.stdout);
    for index in 0..2 {
        assert_eq!(
            ordered.get("subtask_items").at(index).keys(),
            SUBTASK_ITEM_KEYS,
            "{}",
            shown.stdout
        );
    }
    let card = shown.json();
    let listed: Vec<(&str, &str)> = card["subtask_items"]
        .as_array()
        .expect("subtask_items")
        .iter()
        .map(|item| {
            (
                item["card_id"].as_str().expect("card_id"),
                item["id"].as_str().expect("id"),
            )
        })
        .collect();
    assert_eq!(listed, [("C-0003", "PR-0002"), ("C-0002", "PR-0004")]);
    let items: Vec<&str> = card["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("id"))
        .collect();
    assert_eq!(items, ["PR-0001"], "its own items stay in `items`");
    // Each entry is the child's `items` entry with `card_id` first.
    let child_items = show(&pair, "C-0003")["items"].clone();
    let mut entry = card["subtask_items"][0].clone();
    entry.as_object_mut().expect("an entry").remove("card_id");
    assert_eq!(json!([entry]), child_items);
    assert_eq!(card["subtask_items"][0]["kind"], json!("decision"));
    assert_eq!(card["subtask_items"][0]["why"], Value::Null);
    assert_eq!(card["subtask_items"][0]["status"], json!("open"));

    // No child, or children with no item: `[]`.
    assert_eq!(show(&pair, "C-0003")["subtask_items"], json!([]));
    assert_eq!(
        show(&pair, "C-0002")["subtask_items"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "C-0002 reads its own child C-0004"
    );

    // The text: one line per entry right after the `items:` lines.
    let text = ok(&pair, &["card", "show", "C-0001"]);
    assert_eq!(
        lines_of(&text, "subtask_items: "),
        [
            "subtask_items: C-0003 | PR-0002 | decision | open | Does the keeper sing in the \
             storm?",
            "subtask_items: C-0002 | PR-0004 | question | open | Is the keeper's coat oilskin?",
        ],
        "{text}"
    );
    assert_eq!(
        at(&text, "subtask_items: "),
        at(&text, "items: ") + 1,
        "{text}"
    );
    let none = ok(&pair, &["card", "show", "C-0003"]);
    assert_eq!(
        lines_of(&none, "subtask_items: "),
        ["subtask_items: -"],
        "{none}"
    );

    // `list` never reads them.
    assert_eq!(ok(&pair, &["card", "list"]), list_text);
    assert_eq!(json_run(&pair, &["card", "list"]).stdout, list_json);
}

/// M6: a card with no children at all serves `subtask_items: []` and the
/// text line `subtask_items: -`.
#[test]
fn m6_a_card_with_no_child_serves_none() {
    let pair = Pair::new("cm-m6-none", LAMP);
    m1_cards(&pair);
    assert_eq!(show(&pair, "C-0002")["subtask_items"], json!([]));
    assert_eq!(show(&pair, "C-0001")["subtask_items"], json!([]));
    let text = ok(&pair, &["card", "show", "C-0002"]);
    assert_eq!(lines_of(&text, "subtask_items: "), ["subtask_items: -"]);
}
