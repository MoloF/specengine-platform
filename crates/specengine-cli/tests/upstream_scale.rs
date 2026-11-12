//! docs/features/upstream-signal.md (RM-0105) US-14 [measure], the seeder:
//! an ignored test that seeds a queue at the criterion's scale over a
//! corpus `specengine-eval scale --corpus-only` wrote, for `spec card list
//! --marks --json` to be timed by hand, debug and release.
//!
//! `SPECENGINE_US14_TREE`: the generated `n<N>/tree`, only read;
//! `SPECENGINE_US14_OUT`: an empty scratch directory outside the repository,
//! which receives `repo/` (the tree copied, a `DEC` prefix and a
//! `[decision_records]` table added, one commit) and `home/` (its queue).
//!
//! The seed: 500 parents, each on a live document, with 4 subtasks each on
//! the document's sections (cards through the library, so every snapshot is
//! frozen as a terminal freezes one); one decision raised and approved
//! through the library, its applied row then repeated through the store's
//! `stored_rows` and `restore` to 2 000 applied decisions on the corpus's
//! nodes, each with its `proposal.applied`; the parents' goals edited 20
//! times each, the subtasks reordered 12 times each and 1 500 of them given
//! a due date, through the store, so the queue holds 40 000 events.
//!
//! Run: `SPECENGINE_US14_TREE=… SPECENGINE_US14_OUT=… cargo nextest run -p
//! specengine-cli --test upstream_scale --run-ignored only`.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::{copy_dir, data_dir, read_text, write};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CardNewRequest, DecisionInput, DecisionRequest, Env, Exit,
    Globals, approve_with, card_new, propose_decision,
};
use specengine_core::card_toml::CardConfig;
use specengine_store::{
    CardChange, CardEdit, EVENT_APPLIED, PROPOSAL_COLUMNS, ProposalQueue as _, SqliteQueue,
    StoredEvent, StoredProposal,
};

const PARENTS: usize = 500;
const SUBTASKS: usize = 4;
const DECISIONS: usize = 2000;
const EVENTS: usize = 40_000;
const NOW: &str = "2026-11-12T09:00:00Z";

const RECORDS: &str = "\n[decision_records]\nprefix   = \"DEC\"\ndir      = \"areas/records\"\n\
                       template = \"templates/decision.md\"\n";

const TEMPLATE: &str = "---\nid: {{id}}\nclass: decision\nstatus: {{status}}\ndate: {{date}}\n\
                        title: {{title}}\ncanon: {{canon}}\nlinks:\n  answers: {{targets}}\n\
                        ref: {{proposal}}\nscope: [decisions]\n---\n\n# {{title}}\n\nChoice:\n\n\
                        {{choice}}\n\nEffect:\n\n{{effect}}\n\nCost:\n\n{{cost}}\n\nSummary:\n\n\
                        {{summary}}\n\nOptions:\n\n{{options}}\n\nEvidence:\n\n{{evidence}}\n\n\
                        Note:\n\n{{note}}\n\nDecided by:\n\n{{decided_by}}\n";

/// A live part document's ID, its sections' IDs and its path, by path
/// order.
fn documents(root: &Path) -> Vec<(String, Vec<String>, String)> {
    let mut paths = Vec::new();
    let mut stack = vec![root.join("areas")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("P-"))
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).expect("a document");
            if text.contains("\nstatus: shipped\n") {
                return None;
            }
            let id = text
                .lines()
                .find_map(|line| line.strip_prefix("id: "))
                .expect("an id")
                .to_owned();
            let sections = text
                .match_indices("{#P-")
                .map(|(at, _)| text[at + 2..at + 10].to_owned())
                .collect();
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned();
            Some((id, sections, relative))
        })
        .collect()
}

fn column(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|known| *known == name)
        .unwrap_or_else(|| panic!("a proposal column `{name}`"))
}

#[test]
#[ignore = "US-14's seeder: SPECENGINE_US14_TREE and SPECENGINE_US14_OUT"]
fn us14_seed_the_scale_queue() {
    let tree = PathBuf::from(std::env::var_os("SPECENGINE_US14_TREE").expect("the tree"));
    let out = PathBuf::from(std::env::var_os("SPECENGINE_US14_OUT").expect("the out dir"));
    assert!(
        !out.starts_with(env!("CARGO_MANIFEST_DIR")),
        "outside the repository"
    );
    let root = out.join("repo");
    copy_dir(&tree, &root);
    let config = read_text(&root, "specengine.toml").replace(
        "P = { kind = \"node\", width = 6 }",
        "P = { kind = \"node\", width = 6 }\nDEC = { kind = \"decision\", width = 4 }",
    );
    write(&root, "specengine.toml", format!("{config}{RECORDS}"));
    write(&root, "templates/decision.md", TEMPLATE);
    let git = Sandbox::new(&out);
    git.init(&root);
    git.add_all(&root);
    git.commit(&root, "the synthetic corpus");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let env = Env {
        cwd: root.clone(),
        home: Some(home.clone().into_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals::default();
    let docs = documents(&root);
    assert!(docs.len() >= 50, "{} live documents", docs.len());
    let nodes: Vec<(String, String)> = docs
        .iter()
        .flat_map(|(id, sections, path)| {
            std::iter::once(id.clone())
                .chain(sections.iter().cloned())
                .map(|node| (node, path.clone()))
        })
        .collect();

    let mut parents = Vec::new();
    let mut subtasks = Vec::new();
    for p in 0..PARENTS {
        let (doc, sections, _) = &docs[p % docs.len()];
        let made = |title: String, target: &str, parent: Option<&str>| {
            let outcome = card_new(
                &env,
                &globals,
                &CardNewRequest {
                    title,
                    targets: vec![target.to_owned()],
                    reviewer: Some("Ann".to_owned()),
                    parent: parent.map(str::to_owned),
                    now: NOW.to_owned(),
                    ..CardNewRequest::default()
                },
            )
            .expect("card new");
            assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
            outcome.id.expect("an ID")
        };
        let parent = made(format!("Table {p}"), doc, None);
        for s in 0..SUBTASKS {
            let target = sections.get(s % sections.len().max(1)).unwrap_or(doc);
            subtasks.push(made(format!("Table {p} part {s}"), target, Some(&parent)));
        }
        parents.push(parent);
    }

    let raised = propose_decision(
        &env,
        &globals,
        &DecisionRequest {
            input: DecisionInput {
                node_ids: vec![nodes[0].0.clone()],
                text: "Do the tables print weekly?".to_owned(),
                working_answer: "Yes, weekly.".to_owned(),
                price_of_other: "A reprint.".to_owned(),
                why: None,
                severity: None,
                proposed_patch: None,
                distinct_from: None,
            },
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: NOW.to_owned(),
            git: specengine_store::GitEnv::new(root.clone(), git.vars()),
        },
    )
    .expect("a decision");
    let first = raised.document.id.clone().expect("stored");
    let mut yes = |_: &str| true;
    let approved = approve_with(
        &env,
        &globals,
        &ApproveRequest {
            id: first.clone(),
            note: None,
            now: NOW.to_owned(),
            git: specengine_store::GitEnv::new(root.clone(), git.vars()),
        },
        &ApproveFlags {
            option: None,
            answer: None,
            canon: None,
        },
        &mut yes,
    )
    .expect("approved");
    assert_eq!(approved.exit(), Exit::Answered, "{approved:?}");

    let db = data_dir(&home).join("synthetic.db");
    let mut queue = SqliteQueue::open(&db, "synthetic").expect("the queue");
    let cards = CardConfig::from_toml(&read_text(&root, "specengine.toml")).expect("[card]");
    let stored = queue.list_cards().expect("cards").cards;
    let column_of = |id: &str| {
        let card = stored.iter().find(|card| card.id == id).expect("a card");
        (card.flow.clone(), card.column.clone())
    };
    let goal = |n: usize| {
        CardChange::Edit(Box::new(CardEdit {
            goal: Some(Some(format!("Goal {n}."))),
            ..CardEdit::default()
        }))
    };
    for (index, parent) in parents.iter().enumerate() {
        for n in 0..20 {
            queue
                .change_card(parent, None, &goal(index * 20 + n), "Ann", NOW)
                .expect("an edit");
        }
    }
    for child in &subtasks {
        let (flow, column) = column_of(child);
        let flow = cards.flow(&flow).expect("the flow");
        for _ in 0..12 {
            queue
                .change_card(
                    child,
                    None,
                    &CardChange::Move {
                        flow: Some(flow),
                        to: Some(column.clone()),
                        before: None,
                        note: None,
                    },
                    "Bea",
                    NOW,
                )
                .expect("a reorder");
        }
    }
    // The subtasks' own due edits fill the log to 40 000 with the applied
    // events to come (each due edit ends nothing).
    let due = EVENTS - (DECISIONS - 1) - queue.events().expect("events").len();
    assert!(due <= subtasks.len(), "{due} due edits");
    for child in subtasks.iter().take(due) {
        queue
            .change_card(
                child,
                None,
                &CardChange::Edit(Box::new(CardEdit {
                    due: Some(Some("2026-12-01".to_owned())),
                    ..CardEdit::default()
                })),
                "Bea",
                NOW,
            )
            .expect("a due edit");
    }

    let mut rows = queue.stored_rows().expect("the rows");
    let template: StoredProposal = rows
        .proposals
        .iter()
        .find(|row| row.id() == Some(first.as_str()))
        .expect("the applied decision")
        .clone();
    let applied = rows
        .events
        .iter()
        .find(|event| {
            event.columns[1].as_deref() == Some(EVENT_APPLIED)
                && event.columns[2]
                    .as_deref()
                    .is_some_and(|payload| payload.contains(&first))
        })
        .expect("its applied event")
        .clone();
    let last = rows.events.iter().map(|event| event.seq).max().unwrap_or(0);
    for (seq, n) in (last + 1..).zip(1..DECISIONS) {
        let id = format!("PR-{:04}", n + 1);
        let record = format!("DEC-{:04}", n + 1);
        let mut row = template.clone();
        row.columns[column("id")] = Some(id.clone());
        let (node, path) = &nodes[(n * 7) % nodes.len()];
        row.columns[column("target_id")] = Some(node.clone());
        row.columns[column("target_path")] = Some(path.clone());
        row.columns[column("target_ids")] = Some(format!("[\"{node}\"]"));
        row.columns[column("record_id")] = Some(record.clone());
        row.columns[column("record_path")] = Some(format!("areas/records/{record}.md"));
        rows.proposals.push(row);
        let payload = applied.columns[2]
            .clone()
            .expect("a payload")
            .replace(&first, &id);
        rows.events.push(StoredEvent {
            seq,
            columns: [
                applied.columns[0].clone(),
                applied.columns[1].clone(),
                Some(payload),
                applied.columns[3].clone(),
            ],
        });
    }
    assert_eq!(rows.events.len(), EVENTS);
    drop(queue);
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db.clone().into_os_string();
        name.push(suffix);
        let _ = fs::remove_file(PathBuf::from(name));
    }
    let mut fresh = SqliteQueue::open(&db, "synthetic").expect("a fresh queue");
    fresh.restore(&rows).expect("restored");
    drop(fresh);
    let queue = SqliteQueue::open(&db, "synthetic").expect("the queue");
    assert_eq!(queue.events().expect("events").len(), EVENTS);
    assert_eq!(
        queue.list_cards().expect("cards").cards.len(),
        PARENTS * (SUBTASKS + 1)
    );
    eprintln!(
        "US-14 seeded: {} cards, {} events, {} proposals, {} nodes on {} live documents",
        PARENTS * (SUBTASKS + 1),
        EVENTS,
        DECISIONS,
        nodes.len(),
        docs.len()
    );
    // Every seeded row reads: no applied row is left unjudged.
    let readable = queue
        .list_readable(&specengine_store::ProposalFilter::default())
        .expect("the rows");
    assert!(
        readable.unreadable.is_empty(),
        "{:?}",
        readable.unreadable.first()
    );
    assert_eq!(readable.proposals.len(), DECISIONS);
}
