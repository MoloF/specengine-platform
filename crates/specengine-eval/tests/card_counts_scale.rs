//! docs/features/department-overview.md (RM-0086) "Acceptance criteria",
//! DO11 [measure]: one read at scale. A queue seeded through the store: a
//! template (an open card, an accepted one, a cancelled one, a question on
//! a card, a task in review) written through the store's API at fixed
//! clocks, its rows then repeated through `stored_rows` and `restore` into
//! `N` cards over 6 flows (about one in eight accepted, one in eight
//! cancelled, the rest open over three columns), `N / 5` with a parent (the
//! card before, one child in four accepted and one cancelled),
//! `N / 10` items (one in four applied, the rest open), `N / 40` tasks (odd
//! in review, even ready) and `N / 20` links (`N / 40` open cards linking
//! two tasks each).
//!
//! - `do11_the_counts_hold_at_1000_cards` (in the default run): `spec card
//!   counts --json`'s document is the seed's, worked out here from the
//!   seeded rows, never read back: every (flow, column) row, every flow's
//!   items, oldest, ready parents and tasks in review.
//! - `do11_measure_20000_cards` (`#[ignore]`, release): 20 000 cards, 4 000
//!   with a parent, 2 000 items, 500 tasks with 1 000 links; prints the
//!   times of the release `spec card counts --json`, warm, of the library's
//!   whole read and of the store call alone, each the median of five after
//!   one warm-up run, then asserts the 300 ms bar on the binary.
//!
//! The store's statements per call (the same at any size) are the store's
//! own `#[cfg(test)]` seam: `specengine-store`'s
//! `src/queue/card_counts_seam.rs`.
//!
//! Run the measure: `cargo build --release -p specengine-cli --bin spec`,
//! then `cargo nextest run -p specengine-eval --release --test
//! card_counts_scale --run-ignored only --no-capture`.
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, with a `HOME` of its own. Every
//! name is invented.

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specengine_cli::{CardCountsRequest, Env, Globals, Outcome, View, card_counts, render_json};
use specengine_core::card_toml::CardConfig;
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_core::trace::TargetNorms;
use specengine_model::TaskCriterion;
use specengine_store::{
    CARD_COLUMNS, CardAccept, CardChange, Intake, NewCard, NewIntake, NewTask, PROPOSAL_COLUMNS,
    Place, ProposalKind, ProposalQueue as _, SqliteQueue, StoredCard, StoredProposal, StoredTask,
    TASK_COLUMNS, TaskChange,
};

const SLUG: &str = "harbor-press";
/// The bar of DO11.
const BAR: Duration = Duration::from_millis(300);
/// Timed runs; the median is kept.
const RUNS: usize = 5;
/// The six flows, in config order.
const FLOW_NAMES: [&str; 6] = ["copy", "plates", "sound", "maps", "bind", "ink"];
/// Each flow's columns; `check` to `done` is the accept.
const COLUMNS: [&str; 4] = ["todo", "doing", "check", "done"];
const T0: &str = "2026-11-01T09:00:00Z";

fn config() -> String {
    let mut text = format!("[project]\nslug = \"{SLUG}\"\n\n[paths]\nroots = [\"docs\"]\n");
    for name in FLOW_NAMES {
        text.push_str(&format!(
            "\n[[card.flow]]\nname = \"{name}\"\ndisciplines = [\"{name}\"]\ncolumns = [\"todo\", \
             \"doing\", \"check\", \"done\"]\naccept = {{ from = \"check\", to = \"done\" }}\n"
        ));
    }
    text
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn put(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

fn db_under(home: &Path) -> PathBuf {
    let data = if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    };
    data.join(format!("{SLUG}.db"))
}

/// The index of `name` in a `StoredCard`'s columns (`position` apart).
fn card_column(name: &str) -> usize {
    let at = CARD_COLUMNS
        .iter()
        .position(|known| *known == name)
        .unwrap_or_else(|| panic!("a card column `{name}`"));
    let position = CARD_COLUMNS
        .iter()
        .position(|known| *known == "position")
        .expect("position");
    assert_ne!(at, position);
    if at > position { at - 1 } else { at }
}

fn proposal_column(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|known| *known == name)
        .unwrap_or_else(|| panic!("a proposal column `{name}`"))
}

fn task_column(name: &str) -> usize {
    TASK_COLUMNS
        .iter()
        .position(|known| *known == name)
        .unwrap_or_else(|| panic!("a task column `{name}`"))
}

/// What a seeded card ends as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Open,
    Accepted,
    Cancelled,
}

/// Card `k` (1-based) of the seed: its flow, column, state and parent.
fn seeded_card(k: usize) -> (usize, &'static str, State, Option<usize>) {
    let flow = k % FLOW_NAMES.len();
    // A child (`k % 5 == 3`) is accepted at `k % 20 == 3` and cancelled at
    // `k % 20 == 13`, so one parent in two is ready.
    let (column, state) = match (k % 20, k % 10) {
        (3, _) | (_, 0) => ("done", State::Accepted),
        (13, _) | (_, 1) => ("todo", State::Cancelled),
        _ => (COLUMNS[(k / FLOW_NAMES.len()) % 3], State::Open),
    };
    // One in five has a parent: the card before it.
    let parent = (k % 5 == 3).then(|| k - 1);
    (flow, column, state, parent)
}

fn card_id(k: usize) -> String {
    format!("C-{k:05}")
}

fn task_id(t: usize) -> String {
    format!("T-{t:04}")
}

/// The scratch repository, its `HOME` and its queue, removed on drop.
struct Seeded {
    dir: PathBuf,
    root: PathBuf,
    home: PathBuf,
    cards: usize,
}

impl Drop for Seeded {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Seeded {
    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    fn new(label: &str, cards: usize) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-do11-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let root = dir.join("repo");
        put(&root, "specengine.toml", &config());
        put(&root, "docs/press.md", "# The press\n\nIts sheets.\n");
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let seeded = Self {
            dir,
            root,
            home,
            cards,
        };
        seeded.seed();
        seeded
    }

    /// The template through the store's API, then its rows repeated.
    fn seed(&self) {
        let db = db_under(&self.home);
        fs::create_dir_all(db.parent().expect("a parent")).expect("a data directory");
        let flows = CardConfig::from_toml(&config()).expect("[card]").flows;
        let template = {
            let mut queue = SqliteQueue::open(&db, SLUG).expect("a queue");
            let new = |column: &str| NewCard {
                flow: "copy".to_owned(),
                column: column.to_owned(),
                title: "A sheet".to_owned(),
                goal: None,
                discipline: Some("copy".to_owned()),
                targets: Vec::new(),
                criteria: Vec::new(),
                assignee: None,
                reviewer: Some("bob".to_owned()),
                mandate: None,
                estimate: None,
                due: None,
                parent: None,
                corrects: None,
                by: "local".to_owned(),
            };
            queue.create_card(&new("todo"), T0).expect("open");
            queue.create_card(&new("check"), T0).expect("to accept");
            queue.create_card(&new("todo"), T0).expect("to cancel");
            let norms = TargetNorms::new();
            queue
                .accept_card(
                    "C-0002",
                    None,
                    &CardAccept {
                        flow: Some(&flows[0]),
                        note: None,
                        norms: &norms,
                    },
                    "bob",
                    T0,
                )
                .expect("accepted");
            queue
                .change_card(
                    "C-0003",
                    None,
                    &CardChange::Cancel {
                        reason: "Dropped.".to_owned(),
                    },
                    "ann",
                    T0,
                )
                .expect("cancelled");
            let intake = NewIntake {
                kind: ProposalKind::Question,
                target_path: "docs/press.md".to_owned(),
                place: Place {
                    git_common_dir: "/r/.git".to_owned(),
                    worktree: "/r".to_owned(),
                    root_rel: String::new(),
                    branch: "main".to_owned(),
                    base_commit: "1".repeat(40),
                },
                author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
                intake: Intake {
                    target_ids: vec!["PRS-SHEET".to_owned()],
                    severity: IntakeSeverity::Normal,
                    gap_type: None,
                    summary: "Which paper does the sheet take?".to_owned(),
                    working_answer: Some("The house stock.".to_owned()),
                    price_of_other: Some("A reprint.".to_owned()),
                    evidence: Vec::new(),
                    options: Vec::new(),
                    recommendation: None,
                    distinct_from: Vec::new(),
                    why: None,
                },
                card_id: Some("C-0001".to_owned()),
                actor: None,
            };
            queue
                .create_intake(&intake, &[], None, T0)
                .expect("a question")
                .created
                .expect("stored");
            queue
                .create_task(
                    &NewTask {
                        git_common_dir: "/r/.git".to_owned(),
                        title: Some("Set the sheet".to_owned()),
                        goal: None,
                        targets: vec!["PRS-SHEET".to_owned()],
                        author: Author::human(),
                    },
                    T0,
                )
                .expect("a task");
            let seen = queue
                .get_task("T-0001")
                .expect("get")
                .expect("a task")
                .seen();
            queue
                .change_task(
                    "T-0001",
                    &seen,
                    &TaskChange::Plan {
                        plan: "1. Set.\n".to_owned(),
                        criteria: vec![TaskCriterion {
                            reference: None,
                            text: Some("It reads.".to_owned()),
                        }],
                        affected_nodes: Vec::new(),
                    },
                    T0,
                )
                .expect("planned");
            queue.stored_rows().expect("the rows")
        };
        assert_eq!(template.cards.len(), 3);
        let accepted = template.cards[1].columns[card_column("acceptance")].clone();
        let cancelled = template.cards[2].columns[card_column("cancellation")].clone();
        assert!(accepted.is_some() && cancelled.is_some());
        let mut rows = template.clone();
        rows.cards.clear();
        rows.events.clear();
        rows.proposals.clear();
        rows.tasks.clear();
        rows.runs.clear();
        let (id, flow, col, title, parent, tasks, acceptance, cancellation) = (
            card_column("id"),
            card_column("flow"),
            card_column("col"),
            card_column("title"),
            card_column("parent"),
            card_column("tasks"),
            card_column("acceptance"),
            card_column("cancellation"),
        );
        let linking = self.linking();
        for k in 1..=self.cards {
            let (f, column, state, up) = seeded_card(k);
            let mut row: StoredCard = template.cards[0].clone();
            row.columns[id] = Some(card_id(k));
            row.columns[flow] = Some(FLOW_NAMES[f].to_owned());
            row.columns[col] = Some(column.to_owned());
            row.columns[title] = Some(format!("Sheet {k}"));
            row.columns[parent] = up.map(card_id);
            row.columns[acceptance] = (state == State::Accepted)
                .then(|| accepted.clone())
                .flatten();
            row.columns[cancellation] = (state == State::Cancelled)
                .then(|| cancelled.clone())
                .flatten();
            row.columns[tasks] = Some(match linking.get(&k) {
                Some(pair) => serde_json::to_string(pair).expect("JSON"),
                None => "[]".to_owned(),
            });
            row.position = Some(i64::try_from(k).expect("a position"));
            rows.cards.push(row);
        }
        let (pid, card, status, created) = (
            proposal_column("id"),
            proposal_column("card_id"),
            proposal_column("status"),
            proposal_column("created_at"),
        );
        for (i, (on, open, at)) in self.items().into_iter().enumerate() {
            let mut row: StoredProposal = template.proposals[0].clone();
            row.columns[pid] = Some(format!("PR-{:05}", i + 1));
            row.columns[card] = Some(card_id(on));
            row.columns[status] = Some(if open { "open" } else { "applied" }.to_owned());
            row.columns[created] = Some(at);
            rows.proposals.push(row);
        }
        let (tid, tstatus) = (task_column("id"), task_column("status"));
        for t in 1..=self.cards / 40 {
            let mut row: StoredTask = template.tasks[0].clone();
            row.columns[tid] = Some(task_id(t));
            row.columns[tstatus] = Some(if t % 2 == 1 { "review" } else { "ready" }.to_owned());
            rows.tasks.push(row);
        }
        for suffix in ["", "-wal", "-shm"] {
            let mut name = db.clone().into_os_string();
            name.push(suffix);
            let _ = fs::remove_file(PathBuf::from(name));
        }
        SqliteQueue::open(&db, SLUG)
            .expect("a fresh queue")
            .restore(&rows)
            .expect("restored");
    }

    /// The open cards linking two tasks each: card `40 j + 5` (open, as
    /// `k % 10 == 5`) links `T-j` and `T-(j + 1)` wrapped, `j` from 1.
    fn linking(&self) -> BTreeMap<usize, Vec<String>> {
        let tasks = self.cards / 40;
        (1..=tasks)
            .map(|j| {
                let k = 40 * (j - 1) + 5;
                (k, vec![task_id(j), task_id(j % tasks + 1)])
            })
            .collect()
    }

    /// The items: item `i` (0-based) on card `10 i + 7`, open unless `i % 4
    /// == 3`, made on day `1 + i % 28`.
    fn items(&self) -> Vec<(usize, bool, String)> {
        (0..self.cards / 10)
            .map(|i| {
                (
                    10 * i + 7,
                    i % 4 != 3,
                    format!("2026-10-{:02}T09:00:00Z", 1 + i % 28),
                )
            })
            .collect()
    }

    /// The document the seed must give, worked out from the seed alone.
    fn expected(&self) -> Value {
        let mut columns: BTreeMap<(usize, &str), [u64; 4]> = BTreeMap::new();
        let mut children: BTreeMap<usize, Vec<State>> = BTreeMap::new();
        for k in 1..=self.cards {
            let (f, column, state, up) = seeded_card(k);
            let tally = columns.entry((f, column)).or_default();
            match state {
                State::Open => tally[0] += 1,
                State::Accepted => tally[1] += 1,
                State::Cancelled => tally[2] += 1,
            }
            if state == State::Open && up.is_some() {
                tally[3] += 1;
            }
            if let Some(p) = up {
                children.entry(p).or_default().push(state);
            }
        }
        let mut ready = [0u64; 6];
        for (p, states) in &children {
            let (f, _, state, _) = seeded_card(*p);
            if state == State::Open && states.iter().all(|s| *s != State::Open) {
                ready[f] += 1;
            }
        }
        let mut items: [(u64, Option<String>); 6] = Default::default();
        for (on, open, at) in self.items() {
            if !open {
                continue;
            }
            let (f, ..) = seeded_card(on);
            items[f].0 += 1;
            let oldest = &mut items[f].1;
            if oldest.as_ref().is_none_or(|held| at < *held) {
                *oldest = Some(at);
            }
        }
        let mut review: [BTreeSet<String>; 6] = Default::default();
        for (k, pair) in self.linking() {
            let (f, _, state, _) = seeded_card(k);
            assert_eq!(state, State::Open, "a linking card is open");
            for id in pair {
                let t: usize = id[2..].parse().expect("a number");
                if t % 2 == 1 {
                    review[f].insert(id);
                }
            }
        }
        let mut rows = Vec::new();
        for (f, name) in FLOW_NAMES.iter().enumerate() {
            for column in COLUMNS {
                let tally = columns.get(&(f, column)).copied().unwrap_or_default();
                rows.push(json!({"flow": name, "column": column, "open": tally[0],
                    "accepted": tally[1], "cancelled": tally[2], "open_subtasks": tally[3]}));
            }
        }
        let by_flow: Vec<Value> = FLOW_NAMES
            .iter()
            .enumerate()
            .map(|(f, name)| {
                json!({"flow": name, "open_items": items[f].0, "oldest_open_item": items[f].1,
                    "ready_parents": ready[f],
                    "review_agent_tasks": review[f].iter().collect::<Vec<_>>()})
            })
            .collect();
        json!({"columns": rows, "by_flow": by_flow})
    }

    /// The library's read in the browser view, as JSON.
    fn read(&self) -> Value {
        let outcome = card_counts(
            &self.env(),
            &Globals::default(),
            &CardCountsRequest { tasks_here: true },
        )
        .expect("card counts");
        serde_json::from_str(&render_json(&Outcome::CardCounts(Box::new(
            outcome.in_view(View::Browser),
        ))))
        .expect("JSON")
    }

    /// The store call alone.
    fn store_call(&self) -> usize {
        let queue = SqliteQueue::open_existing(db_under(&self.home), SLUG)
            .expect("the queue")
            .expect("there");
        queue.card_counts(true).expect("counts").columns.len()
    }
}

/// The release `spec` beside this test binary (`target.noindex/release`).
fn release_spec() -> PathBuf {
    if let Some(path) = std::env::var_os("SPECENGINE_DO11_SPEC") {
        return PathBuf::from(path);
    }
    let exe = std::env::current_exe().expect("the test binary");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("the profile directory");
    dir.join("spec")
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

/// Five timed runs of `run` after one warm-up.
fn timed(mut run: impl FnMut()) -> (Duration, Vec<Duration>) {
    run();
    let times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect();
    (median(times.clone()), times)
}

/// The document's two count lists.
fn counted(document: &Value) -> Value {
    json!({"columns": document["columns"], "by_flow": document["by_flow"]})
}

/// DO11 at 1 000 cards: the document is the seed's, every row; nothing cut
/// in the browser view; the counts sum to every card.
#[test]
fn do11_the_counts_hold_at_1000_cards() {
    let seeded = Seeded::new("counts", 1_000);
    let document = seeded.read();
    assert_eq!(document["notes"], json!([]));
    assert_eq!(document["truncated"], false);
    assert_eq!(counted(&document), seeded.expected());
    let sum: u64 = document["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .map(|row| {
            row["open"].as_u64().expect("open")
                + row["accepted"].as_u64().expect("accepted")
                + row["cancelled"].as_u64().expect("cancelled")
        })
        .sum();
    assert_eq!(sum, 1_000);
    let ready: u64 = document["by_flow"]
        .as_array()
        .expect("by_flow")
        .iter()
        .map(|row| row["ready_parents"].as_u64().expect("ready"))
        .sum();
    assert!(ready > 0, "the premise: some parent is ready");
    assert_eq!(seeded.store_call(), 6 * 4);
}

/// DO11 [measure]: 20 000 cards in 6 flows, 4 000 with a parent, 2 000
/// items, 500 tasks with 1 000 links; `spec card counts --json` in release,
/// warm, at most 300 ms; the library's read and the store call alone timed
/// beside it.
#[test]
#[ignore = "DO11 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test card_counts_scale \
            --run-ignored only --no-capture"]
fn do11_measure_20000_cards() {
    let started = Instant::now();
    let seeded = Seeded::new("measure", 20_000);
    let seeding = started.elapsed();
    let parents = (1..=seeded.cards)
        .filter(|k| seeded_card(*k).3.is_some())
        .count();
    let links: usize = seeded.linking().values().map(Vec::len).sum();
    assert_eq!(
        (parents, seeded.items().len(), seeded.cards / 40, links),
        (4_000, 2_000, 500, 1_000),
        "the spec's seed"
    );
    let spec = release_spec();
    assert!(
        spec.exists(),
        "{}: build the release `spec` first",
        spec.display()
    );
    let mut stdout = String::new();
    let (binary, binary_runs) = timed(|| {
        let output = Command::new(&spec)
            .env_clear()
            .env("HOME", &seeded.home)
            .current_dir(&seeded.root)
            .args(["card", "counts", "--json"])
            .stdin(Stdio::null())
            .output()
            .expect("spec runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout = String::from_utf8(output.stdout).expect("UTF-8");
    });
    let document: Value = serde_json::from_str(&stdout).expect("JSON");
    assert_eq!(counted(&document), seeded.expected());
    let (library, _) = timed(|| {
        let _ = seeded.read();
    });
    let (store, _) = timed(|| {
        let _ = seeded.store_call();
    });
    println!(
        "DO11: {} cards, {parents} with a parent, {} items, {} tasks, {links} links; seeding {:.1} \
         s; spec card counts --json (release, warm) median {} ms, runs {:?} ms; library read \
         median {} ms; store call alone median {} ms",
        seeded.cards,
        seeded.items().len(),
        seeded.cards / 40,
        seeding.as_secs_f64(),
        binary.as_millis(),
        binary_runs
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        library.as_millis(),
        store.as_millis(),
    );
    assert!(
        binary <= BAR,
        "DO11's bar: {} ms > 300 ms",
        binary.as_millis()
    );
}
