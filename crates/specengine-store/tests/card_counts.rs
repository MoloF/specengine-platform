//! docs/features/department-overview.md (RM-0086) "Acceptance criteria",
//! the store's half: `SqliteQueue::card_counts` through the public API over
//! rows written through the store with fixed `now`s (the raw steps, an
//! `applied` status and a corrupt `tasks`, by the system's `sqlite3`).
//!
//! - DO9 (store): the spec's scenario gives exactly its rows: per (flow,
//!   column) the open, accepted and cancelled cards and the open ones with a
//!   parent, a column the config no longer lists as stored; per parent's
//!   flow the ready parents; per card's flow the open items on its cards in
//!   any state (a cancelled card's too) with the least `created_at`; per
//!   flow the distinct `T-` IDs in review its open cards link. Without
//!   `tasks_here` no task is read. Through core's `counts_document` it is
//!   the spec's JSON byte for byte. A `tasks` that is no JSON list on an
//!   open card is passed over and named; a card with both stamps counts as
//!   cancelled alone. M: `open_subtasks` counting accepted children; an
//!   item on a cancelled task left out.
//! - DO8 (store): 300 parents across 3 flows, seeded deterministically, each
//!   with 0-4 children (open, accepted or cancelled), plus closed parents
//!   and grandchildren: `ready_parents` per flow equals a brute count by
//!   `docs/canon/card-progress.md`'s rule (an open parent, one child or
//!   more, every direct child accepted or cancelled; a grandchild never
//!   counts). M: a parent ready on any accepted child; a parent with no
//!   child counted; a grandchild counted.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/counts_scene/data.rs"]
mod data;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use specengine_core::card_counts::{CardCountRows, ColumnTally, OpenItems, counts_document};
use specengine_core::card_toml::{CardConfig, CardFlow};
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_core::trace::TargetNorms;
use specengine_model::TaskCriterion;
use specengine_store::{
    CardAccept, CardChange, CardEdit, Intake, NewCard, NewIntake, NewTask, Place, ProposalKind,
    ProposalQueue as _, SqliteQueue, TaskChange, TaskSnapshot,
};

const PROJECT: &str = "atlas-bindery";
const REVIEWER: &str = "bob";

/// "Data"'s three flows, as configured once C-0007 is in `proofing`.
const FLOWS: &str = r#"
[[card.flow]]
name = "copy"
title = "Copy"
disciplines = ["copy"]
columns = ["todo", "doing", "review", "returned", "done"]
accept = { from = "review", to = "done" }
return = "returned"

[[card.flow]]
name = "plates"
title = "Plates"
disciplines = ["plates"]
columns = ["todo", "drawing", "done"]
accept = { from = "drawing", to = "done" }

[[card.flow]]
name = "sound"
title = "Sound"
disciplines = ["sound"]
columns = ["todo", "check", "done"]
accept = { from = "check", to = "done" }
"#;

fn at(day: u32) -> String {
    format!("2026-11-{day:02}T09:00:00Z")
}

fn sqlite3(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn config() -> CardConfig {
    CardConfig::from_toml(FLOWS).expect("the scenario's [card]")
}

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "main".to_owned(),
        base_commit: "1".repeat(40),
    }
}

/// A card of `title` standing in `flow`/`column`, made by `local`.
fn card(flow: &str, column: &str, title: &str) -> NewCard {
    NewCard {
        flow: flow.to_owned(),
        column: column.to_owned(),
        title: title.to_owned(),
        goal: None,
        discipline: Some(flow.to_owned()),
        targets: Vec::new(),
        criteria: Vec::new(),
        assignee: None,
        reviewer: Some(REVIEWER.to_owned()),
        mandate: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: None,
        by: "local".to_owned(),
    }
}

/// `card` under `parent`.
fn child(flow: &str, column: &str, title: &str, parent: &str) -> NewCard {
    NewCard {
        parent: Some(parent.to_owned()),
        ..card(flow, column, title)
    }
}

/// An item of `kind` on `node` for `card`, as an agent raises it.
fn item(kind: ProposalKind, node: &str, text: &str, card: &str) -> NewIntake {
    NewIntake {
        kind,
        target_path: "docs/atlas.md".to_owned(),
        place: place(),
        author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: vec![node.to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some("As the first edition did.".to_owned()),
            price_of_other: Some("A page reprinted.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        card_id: Some(card.to_owned()),
        actor: None,
    }
}

/// The stored ID of an item that must be stored.
fn raised(queue: &mut SqliteQueue, intake: &NewIntake, now: &str) -> String {
    let result = queue
        .create_intake(intake, &[], None, now)
        .unwrap_or_else(|error| panic!("create_intake: {error}"));
    match result.created {
        Some(proposal) => proposal.id,
        None => panic!("not stored: hits {:?}", result.hits),
    }
}

/// The card `id` accepted by its reviewer in `flow`.
fn accept(queue: &mut SqliteQueue, flow: &CardFlow, id: &str, now: &str) {
    let norms = TargetNorms::new();
    queue
        .accept_card(
            id,
            None,
            &CardAccept {
                flow: Some(flow),
                note: None,
                norms: &norms,
            },
            REVIEWER,
            now,
        )
        .unwrap_or_else(|error| panic!("accept {id}: {error}"));
}

fn cancel(queue: &mut SqliteQueue, id: &str, now: &str) {
    queue
        .change_card(
            id,
            None,
            &CardChange::Cancel {
                reason: "Dropped from the edition.".to_owned(),
            },
            "ann",
            now,
        )
        .unwrap_or_else(|error| panic!("cancel {id}: {error}"));
}

fn edit(queue: &mut SqliteQueue, id: &str, edit: CardEdit, now: &str) {
    queue
        .change_card(id, None, &CardChange::Edit(Box::new(edit)), "ann", now)
        .unwrap_or_else(|error| panic!("edit {id}: {error}"));
}

fn made(queue: &mut SqliteQueue, new: &NewCard, now: &str) -> String {
    queue
        .create_card(new, now)
        .unwrap_or_else(|error| panic!("card {}: {error}", new.title))
        .id
}

/// The change of task `id` as read now.
fn task_change(queue: &mut SqliteQueue, id: &str, change: &TaskChange, now: &str) {
    let seen = queue.get_task(id).expect("get").expect("a task").seen();
    queue
        .change_task(id, &seen, change, now)
        .unwrap_or_else(|error| panic!("task {id}: {error}"));
}

fn plan() -> TaskChange {
    TaskChange::Plan {
        plan: "1. Read.\n2. Set.\n".to_owned(),
        criteria: vec![TaskCriterion {
            reference: None,
            text: Some("The page reads.".to_owned()),
        }],
        affected_nodes: Vec::new(),
    }
}

fn snapshot() -> TaskSnapshot {
    TaskSnapshot {
        at: at(6),
        by: "Owner <owner@example.invalid>".to_owned(),
        place: specengine_model::SnapshotPlace {
            worktree: "/r".to_owned(),
            root_rel: String::new(),
            branch: "main".to_owned(),
            commit: "1".repeat(40),
        },
        nodes: Vec::new(),
    }
}

/// "Data"'s scenario through the store: C-0001 to C-0007, T-0001 (review)
/// and T-0002 (ready), PR-0001 to PR-0004.
fn scenario(db: &Path) -> SqliteQueue {
    let config = config();
    let plates = config.flow("plates").expect("plates").clone();
    let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
    let q = &mut queue;
    assert_eq!(
        made(q, &card("copy", "review", "Write the foreword"), &at(1)),
        "C-0001"
    );
    assert_eq!(
        made(
            q,
            &child("plates", "drawing", "Draw the plates", "C-0001"),
            &at(1)
        ),
        "C-0002"
    );
    assert_eq!(
        made(q, &child("copy", "todo", "Cut the index", "C-0001"), &at(1)),
        "C-0003"
    );
    assert_eq!(
        made(q, &card("copy", "returned", "Proof the map"), &at(1)),
        "C-0004"
    );
    assert_eq!(
        made(q, &card("plates", "todo", "Print the atlas"), &at(1)),
        "C-0005"
    );
    assert_eq!(
        made(q, &card("sound", "todo", "Record the bell"), &at(1)),
        "C-0006"
    );
    assert_eq!(
        made(q, &card("copy", "proofing", "Set the type"), &at(1)),
        "C-0007"
    );
    accept(q, &plates, "C-0002", &at(3));
    cancel(q, "C-0003", &at(3));
    edit(
        q,
        "C-0004",
        CardEdit {
            parent: Some(Some("C-0005".to_owned())),
            ..CardEdit::default()
        },
        &at(4),
    );
    for title in ["Typeset the foreword", "Ink the plates"] {
        q.create_task(
            &NewTask {
                git_common_dir: "/elsewhere/.git".to_owned(),
                title: Some(title.to_owned()),
                goal: None,
                targets: vec!["ATL-FOREWORD".to_owned()],
                author: Author::human(),
            },
            &at(5),
        )
        .expect("a task");
    }
    task_change(q, "T-0001", &plan(), &at(5));
    task_change(q, "T-0002", &plan(), &at(5));
    task_change(
        q,
        "T-0002",
        &TaskChange::Approve {
            snapshot: snapshot(),
        },
        &at(6),
    );
    edit(
        q,
        "C-0001",
        CardEdit {
            tasks: Some(vec!["T-0001".to_owned()]),
            ..CardEdit::default()
        },
        &at(6),
    );
    edit(
        q,
        "C-0005",
        CardEdit {
            tasks: Some(vec!["T-0001".to_owned(), "T-0002".to_owned()]),
            ..CardEdit::default()
        },
        &at(6),
    );
    let question = ProposalKind::Question;
    assert_eq!(
        raised(
            q,
            &item(
                question,
                "ATL-FOREWORD",
                "Does the foreword thank the surveyors?",
                "C-0001"
            ),
            &at(2)
        ),
        "PR-0001"
    );
    assert_eq!(
        raised(
            q,
            &item(
                question,
                "ATL-MAP",
                "Which harbor names does the map keep?",
                "C-0004"
            ),
            &at(5)
        ),
        "PR-0002"
    );
    assert_eq!(
        raised(
            q,
            &item(
                ProposalKind::Decision,
                "ATL-TYPE",
                "Is the foreword set in a serif face?",
                "C-0001"
            ),
            &at(6)
        ),
        "PR-0003"
    );
    sqlite3(
        db,
        "UPDATE proposals SET status = 'applied' WHERE id = 'PR-0003';",
    );
    assert_eq!(
        raised(
            q,
            &item(
                question,
                "ATL-INDEX",
                "Is the old index kept as an appendix?",
                "C-0003"
            ),
            &at(7)
        ),
        "PR-0004"
    );
    queue
}

fn tally(open: u64, accepted: u64, cancelled: u64, open_subtasks: u64) -> ColumnTally {
    ColumnTally {
        open,
        accepted,
        cancelled,
        open_subtasks,
    }
}

fn key(flow: &str, column: &str) -> (String, String) {
    (flow.to_owned(), column.to_owned())
}

/// The scenario's rows, as "Data" gives them.
fn scenario_rows(tasks_here: bool) -> CardCountRows {
    let ids =
        |list: &[&str]| -> BTreeSet<String> { list.iter().map(|id| (*id).to_owned()).collect() };
    CardCountRows {
        columns: BTreeMap::from([
            (key("copy", "todo"), tally(0, 0, 1, 0)),
            (key("copy", "review"), tally(1, 0, 0, 0)),
            (key("copy", "returned"), tally(1, 0, 0, 1)),
            (key("copy", "proofing"), tally(1, 0, 0, 0)),
            (key("plates", "todo"), tally(1, 0, 0, 0)),
            (key("plates", "done"), tally(0, 1, 0, 0)),
            (key("sound", "todo"), tally(1, 0, 0, 0)),
        ]),
        ready_parents: BTreeMap::from([("copy".to_owned(), 1)]),
        open_items: BTreeMap::from([(
            "copy".to_owned(),
            OpenItems {
                count: 3,
                oldest: Some(at(2)),
            },
        )]),
        review_tasks: if tasks_here {
            BTreeMap::from([
                ("copy".to_owned(), ids(&["T-0001"])),
                ("plates".to_owned(), ids(&["T-0001"])),
            ])
        } else {
            BTreeMap::new()
        },
        unread_tasks: BTreeSet::new(),
        unread_names: Vec::new(),
    }
}

// ------------------------------------------------------------------ DO9

/// DO9, the store: the scenario's rows exactly; without `tasks_here` no
/// task is read; the read writes nothing (`dump` before and after). M:
/// `open_subtasks` counting accepted children; an item on a cancelled task
/// left out.
#[test]
fn do9_the_scenario_gives_its_rows() {
    let scratch = Scratch::new("cc-do9-rows");
    let db = scratch.db("q");
    let queue = scenario(&db);
    let dump = queue.dump().expect("dump");
    assert_eq!(
        queue.card_counts(true).expect("counts"),
        scenario_rows(true)
    );
    assert_eq!(
        queue.card_counts(false).expect("counts"),
        scenario_rows(false)
    );
    assert_eq!(queue.dump().expect("dump"), dump, "a read writes nothing");
    // Read-only, as the CLI opens it.
    drop(queue);
    let existing = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open")
        .expect("there");
    assert_eq!(
        existing.card_counts(true).expect("counts"),
        scenario_rows(true)
    );
}

/// DO9, store and core: the scenario's rows under its `[card]` are the
/// spec's JSON byte for byte, `flows` the config's `CardFlow`s as served.
#[test]
fn do9_the_rows_through_core_are_the_specs_json() {
    let scratch = Scratch::new("cc-do9-json");
    let db = scratch.db("q");
    let queue = scenario(&db);
    let config = config();
    let rows = queue.card_counts(true).expect("counts");
    let document = counts_document(&config.flows, &rows, true);
    let flows = serde_json::to_string(&config.flows).expect("flows");
    assert_eq!(
        serde_json::to_string(&document).expect("JSON"),
        data::with_flows(&flows)
    );
}

/// DO9: an open card whose `tasks` is no JSON list (`"x"`, an object) is
/// passed over and named, still counted in its column; a card with both
/// stamps counts as cancelled alone.
#[test]
fn do9_a_tasks_that_is_no_list_is_named_and_counted() {
    let scratch = Scratch::new("cc-do9-unread");
    let db = scratch.db("q");
    let queue = scenario(&db);
    sqlite3(&db, "UPDATE cards SET tasks = '\"x\"' WHERE id = 'C-0006';");
    sqlite3(
        &db,
        "UPDATE cards SET tasks = '{\"T-0001\": 1}' WHERE id = 'C-0007';",
    );
    let rows = queue.card_counts(true).expect("counts");
    let mut want = scenario_rows(true);
    want.unread_tasks = ["C-0006", "C-0007"]
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    assert_eq!(rows, want);
    // Without agent tasks, no link is read and none is named.
    assert_eq!(
        queue.card_counts(false).expect("counts"),
        scenario_rows(false)
    );

    // Both stamps: cancelled alone.
    sqlite3(
        &db,
        "UPDATE cards SET cancellation = (SELECT cancellation FROM cards WHERE id = 'C-0003') \
         WHERE id = 'C-0002';",
    );
    let rows = queue.card_counts(true).expect("counts");
    assert_eq!(rows.columns[&key("plates", "done")], tally(0, 0, 1, 0));
}

// ------------------------------------------------------------------ DO8

/// A deterministic generator (a 64-bit LCG, Knuth's constants).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

/// What a seeded card ends as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Open,
    Accepted,
    Cancelled,
}

/// DO8, the store: 300 parents over `copy`, `plates` and `sound` (100
/// each), each with 0-4 children of any flow and state; one parent in five
/// closed itself (accepted or cancelled); one child in four with 1-2
/// grandchildren of any state. `ready_parents` equals a brute count of the
/// written cards by `docs/canon/card-progress.md`'s rule. M: a parent
/// ready on any accepted child; a parent with no child counted; a
/// grandchild counted.
#[test]
fn do8_ready_parents_equal_a_brute_count() {
    let scratch = Scratch::new("cc-do8");
    let db = scratch.db("q");
    let config = config();
    let flows: Vec<CardFlow> = config.flows.clone();
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let mut random = Lcg(0x0D08_2026_1112);
    // (id, flow, parent, state), as written.
    let mut written: Vec<(String, usize, Option<String>, State)> = Vec::new();
    let now = at(1);
    let state_of = |random: &mut Lcg| match random.next(3) {
        0 => State::Open,
        1 => State::Accepted,
        _ => State::Cancelled,
    };
    let write = |queue: &mut SqliteQueue,
                 written: &mut Vec<(String, usize, Option<String>, State)>,
                 flow: usize,
                 parent: Option<&str>,
                 state: State| {
        let flow_of = &flows[flow];
        let column = if state == State::Accepted {
            flow_of.accept.from.clone()
        } else {
            flow_of.first_column().to_owned()
        };
        let mut new = card(&flow_of.name, &column, "A seeded sheet");
        new.parent = parent.map(str::to_owned);
        let id = made(queue, &new, &now);
        match state {
            State::Open => {}
            State::Accepted => accept(queue, flow_of, &id, &now),
            State::Cancelled => cancel(queue, &id, &now),
        }
        written.push((id.clone(), flow, parent.map(str::to_owned), state));
        id
    };
    for n in 0..300 {
        let flow = n % 3;
        let parent = write(&mut queue, &mut written, flow, None, State::Open);
        let children = random.next(5);
        let mut kids = Vec::new();
        for _ in 0..children {
            let child_flow = usize::try_from(random.next(3)).expect("a flow");
            let state = state_of(&mut random);
            let kid = write(&mut queue, &mut written, child_flow, Some(&parent), state);
            kids.push(kid);
        }
        for kid in &kids {
            if random.next(4) == 0 {
                for _ in 0..=random.next(2) {
                    let state = state_of(&mut random);
                    let grand_flow = usize::try_from(random.next(3)).expect("a flow");
                    write(&mut queue, &mut written, grand_flow, Some(kid), state);
                }
            }
        }
        // One parent in five closes itself once its children are written.
        if random.next(5) == 0 {
            let at_index = written
                .iter()
                .position(|(id, ..)| *id == parent)
                .expect("the parent");
            if random.next(2) == 0 {
                cancel(&mut queue, &parent, &now);
                written[at_index].3 = State::Cancelled;
            } else {
                // Accepted from its flow's `accept.from`: moved there first.
                let flow_of = &flows[flow];
                queue
                    .change_card(
                        &parent,
                        None,
                        &CardChange::Move {
                            flow: Some(flow_of),
                            to: Some(flow_of.accept.from.clone()),
                            before: None,
                            note: None,
                        },
                        "ann",
                        &now,
                    )
                    .expect("moved");
                accept(&mut queue, flow_of, &parent, &now);
                written[at_index].3 = State::Accepted;
            }
        }
    }
    // The brute count: every open card with a direct child and no open
    // direct child, in its own flow.
    let mut brute: BTreeMap<String, u64> = BTreeMap::new();
    for (id, flow, _, state) in &written {
        if *state != State::Open {
            continue;
        }
        let kids: Vec<State> = written
            .iter()
            .filter(|(_, _, parent, _)| parent.as_deref() == Some(id.as_str()))
            .map(|(.., state)| *state)
            .collect();
        if !kids.is_empty() && kids.iter().all(|state| *state != State::Open) {
            *brute.entry(flows[*flow].name.clone()).or_default() += 1;
        }
    }
    // The premise: every case occurs.
    let parents_with_none = written
        .iter()
        .filter(|(id, _, parent, state)| {
            parent.is_none()
                && *state == State::Open
                && !written
                    .iter()
                    .any(|(_, _, p, _)| p.as_deref() == Some(id.as_str()))
        })
        .count();
    assert!(parents_with_none > 0, "an open parent with no child");
    assert!(brute.values().sum::<u64>() > 20, "{brute:?}");
    assert!(
        written
            .iter()
            .filter(|(_, _, parent, _)| parent.is_none())
            .any(|(_, _, _, state)| *state != State::Open),
        "a closed parent"
    );
    let mixed = written.iter().any(|(id, _, _, state)| {
        *state == State::Open && {
            let kids: Vec<State> = written
                .iter()
                .filter(|(_, _, parent, _)| parent.as_deref() == Some(id.as_str()))
                .map(|(.., state)| *state)
                .collect();
            kids.contains(&State::Accepted) && kids.contains(&State::Open)
        }
    });
    assert!(mixed, "a parent with an accepted and an open child");
    let rows = queue.card_counts(true).expect("counts");
    assert_eq!(rows.ready_parents, brute);
    // The columns add up to every card written.
    let total: u64 = rows
        .columns
        .values()
        .map(|tally| tally.open + tally.accepted + tally.cancelled)
        .sum();
    assert_eq!(total, written.len() as u64);
    let open_subtasks: u64 = rows.columns.values().map(|tally| tally.open_subtasks).sum();
    assert_eq!(
        open_subtasks,
        written
            .iter()
            .filter(|(_, _, parent, state)| parent.is_some() && *state == State::Open)
            .count() as u64
    );
}

/// DO9 (the developer's iteration 2, beside "D2": a row the list would
/// note as corrupt still counts where it stands): a card whose stored `col`
/// is `NULL` counts under `NULL`, and so does one whose `flow` is `NULL`
/// (the table is strict: no BLOB is stored), each named; a card naming
/// itself as its parent is no open subtask and no child of its own.
#[test]
fn do9_a_name_that_is_no_text_still_counts_and_is_named() {
    let scratch = Scratch::new("cc-do9-names");
    let db = scratch.db("q");
    let queue = scenario(&db);
    sqlite3(&db, "UPDATE cards SET col = NULL WHERE id = 'C-0006';");
    sqlite3(&db, "UPDATE cards SET flow = NULL WHERE id = 'C-0007';");
    let rows = queue.card_counts(true).expect("counts");
    let mut want = scenario_rows(true);
    want.columns.remove(&key("sound", "todo"));
    want.columns.insert(key("sound", "NULL"), tally(1, 0, 0, 0));
    want.columns.remove(&key("copy", "proofing"));
    want.columns
        .insert(key("NULL", "proofing"), tally(1, 0, 0, 0));
    let mut named = rows.unread_names.clone();
    named.sort_by(|a, b| a.card.cmp(&b.card));
    let named: Vec<(String, String, String, String)> = named
        .into_iter()
        .map(|name| (name.card, name.column, name.reason, name.counted_as))
        .collect();
    assert_eq!(
        named,
        [
            (
                "C-0006".to_owned(),
                "col".to_owned(),
                "it is NULL".to_owned(),
                "NULL".to_owned()
            ),
            (
                "C-0007".to_owned(),
                "flow".to_owned(),
                "it is NULL".to_owned(),
                "NULL".to_owned()
            ),
        ]
    );
    let mut rest = rows.clone();
    rest.unread_names.clear();
    assert_eq!(rest, want);

    // A card naming itself: no open subtask, no child of its own.
    let scratch = Scratch::new("cc-do9-self");
    let db = scratch.db("q");
    let queue = scenario(&db);
    sqlite3(&db, "UPDATE cards SET parent = id WHERE id = 'C-0006';");
    assert_eq!(
        queue.card_counts(true).expect("counts"),
        scenario_rows(true)
    );
}

/// DO9, as the orchestrator re-pinned it at iteration 2 ("Data" said "not
/// a JSON list"): a `tasks` holding any entry that is no string, or a
/// string that is no task ID, does not read: passed over whole and named.
/// C-0001's `[7, "T-0001"]` and C-0005's `["nope", "T-0002"]` link no task
/// in review any more; both cards are named, still counted where they
/// stand. M: the old rule (any JSON list reads, its strings kept).
#[test]
fn do9_a_list_with_an_entry_that_is_no_task_id_does_not_read() {
    let scratch = Scratch::new("cc-do9-list");
    let db = scratch.db("q");
    let queue = scenario(&db);
    sqlite3(
        &db,
        "UPDATE cards SET tasks = '[7, \"T-0001\"]' WHERE id = 'C-0001';",
    );
    sqlite3(
        &db,
        "UPDATE cards SET tasks = '[\"nope\", \"T-0002\"]' WHERE id = 'C-0005';",
    );
    let mut want = scenario_rows(true);
    want.review_tasks.clear();
    want.unread_tasks = ["C-0001", "C-0005"]
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    assert_eq!(queue.card_counts(true).expect("counts"), want);
}
