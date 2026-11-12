//! docs/features/delivery-analytics.md (RM-0107 a) "Acceptance criteria",
//! DA-10 [measure]: one read per call at scale. A queue seeded through the
//! store over a 100-item plan: ten template cards written through the CLI
//! library at fixed clocks (each with ten `card.*` events: made, reordered,
//! taken, returned with a note, re-estimated, back for review, then
//! accepted or reordered again), one question raised on a card; their rows
//! then repeated through the store's `stored_rows` and `restore` into
//! `N` groups of ten cards (each group's own IDs, its odd cards on the
//! group's plan item, its even cards subtasks of the card before) and one
//! item per group.
//!
//! - `da10_the_counts_hold_at_1000_cards` (100 groups: 1 000 cards, 10 000
//!   card events, 100 items, in the default run): the document's counts are
//!   the seed's, worked out here, never read back: every card placed, half
//!   delivered, one in ten delivered late, one in ten overdue, lead 14 days;
//!   the CLI's cut JSON lists its first late rows, the browser view all.
//! - `da10_measure_20000_cards` (2 000 groups: 20 000 cards, 200 000 card
//!   events, 2 000 items; `#[ignore]`, release): prints the times of the
//!   release `spec delivery --as-of 2026-11-20 --json`, warm, of the
//!   library's whole read, and of the queue half alone (the three store
//!   reads), each the median of five after one warm-up run, then asserts
//!   the 1 s bar on the binary.
//!
//! Run the measure: `cargo build --release -p specengine-cli --bin spec`,
//! then `cargo nextest run -p specengine-eval --release --test
//! delivery_scale --run-ignored only --no-capture`.
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, with a `HOME` of its own. Every
//! name is invented.

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use git::Sandbox;
use serde_json::Value;
use specengine_cli::{
    CardAcceptRequest, CardEditRequest, CardMoveRequest, CardNewRequest, DeliveryRequest, Env,
    Exit, GitEnv, Globals, QuestionRequest, View, card_accept, card_edit, card_move, card_new,
    delivery_with_view, propose_question, render_json,
};
use specengine_store::{
    CARD_COLUMNS, EVENT_CARD_ACCEPTED, EVENT_CARD_MOVED, EVENT_CARD_REESTIMATED, PROPOSAL_COLUMNS,
    ProposalFilter, ProposalQueue as _, SqliteQueue, StoredCard, StoredEvent, StoredProposal,
};

const SLUG: &str = "harbor-ledger";
const AS_OF: &str = "2026-11-20";
/// Cards per group: the template's.
const GROUP: usize = 10;
/// The plan's items.
const ITEMS: usize = 100;
/// The bar of DA-10.
const BAR: Duration = Duration::from_secs(1);
/// Timed runs; the median is kept.
const RUNS: usize = 5;

const CONFIG: &str = r#"[project]
slug = "harbor-ledger"

[paths]
roots = ["docs"]

[ids]
RM = { kind = "plan-item", width = 4 }

[roadmap]
prefix     = "RM"
dir        = "docs/roadmap"
releases   = ["r1", "r2", "r3", "r4"]
priorities = ["must", "could"]

[[card.flow]]
name = "copy"
disciplines = ["copy"]
columns = ["todo", "doing", "review", "returned", "done"]
accept = { from = "review", to = "done" }
return = "returned"

[[workspace]]
name = "words"
title = "Words"
flows = ["copy"]

[[workspace]]
name = "print"
title = "Print"
flows = ["copy", "plates"]

[[card.flow]]
name = "plates"
disciplines = ["plates"]
columns = ["todo", "drawing", "done"]
accept = { from = "drawing", to = "done" }
"#;

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn at(day: u32) -> String {
    format!("2026-11-{day:02}T09:00:00Z")
}

/// Item `n` of the plan: 1-80 the four releases, 20 each; 81-90 the pool;
/// 91-95 a release not listed; 96-100 in r4, shipped.
fn item(n: usize) -> String {
    let (release, status) = match n {
        1..=80 => (format!("release: r{}\n", (n - 1) / 20 + 1), "draft"),
        81..=90 => (String::new(), "draft"),
        91..=95 => ("release: r9\n".to_owned(), "draft"),
        _ => ("release: r4\n".to_owned(), "shipped"),
    };
    format!(
        "---\nclass: spec\nid: RM-{n:04}\nstatus: {status}\nscope: [ledger]\n{release}---\n\n# \
         Ledger page {n}\n\nThe page's goal.\n"
    )
}

/// The scratch repository, its `HOME` and its queue, removed on drop.
struct Seeded {
    dir: PathBuf,
    root: PathBuf,
    home: PathBuf,
    groups: usize,
}

impl Drop for Seeded {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

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

/// The ID of template card `k` (1-based) in group `g` (0-based).
fn card_id(g: usize, k: usize) -> String {
    format!("C-{:04}", g * GROUP + k)
}

impl Seeded {
    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// The template through the library, then `groups` groups through the
    /// store.
    fn new(label: &str, groups: usize) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-da10-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let git = Sandbox::new(&dir);
        let root = dir.join("repo");
        git.init(&root);
        let root = fs::canonicalize(&root).expect("canonical");
        put(&root, "specengine.toml", CONFIG);
        for n in 1..=ITEMS {
            put(&root, &format!("docs/roadmap/RM-{n:04}.md"), &item(n));
        }
        git.add_all(&root);
        git.commit(&root, "the ledger plan");
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let seeded = Self {
            dir,
            root,
            home,
            groups,
        };
        seeded.template(&GitEnv::new(seeded.root.clone(), git.vars()));
        seeded.repeat();
        seeded
    }

    fn moved(&self, id: &str, to: &str, note: Option<&str>, day: u32) {
        let outcome = card_move(
            &self.env(),
            &Globals::default(),
            &CardMoveRequest {
                id: id.to_owned(),
                to: Some(to.to_owned()),
                note: note.map(str::to_owned),
                actor: Some("Ann".to_owned()),
                now: at(day),
                ..CardMoveRequest::default()
            },
        )
        .expect("card move");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    /// C-0001 to C-0010, ten `card.*` events each; PR-0001 on C-0001.
    fn template(&self, git: &GitEnv) {
        let env = self.env();
        for k in 1..=GROUP {
            let odd = k % 2 == 1;
            let outcome = card_new(
                &env,
                &Globals::default(),
                &CardNewRequest {
                    title: format!("Ledger line {k}"),
                    discipline: Some("copy".to_owned()),
                    targets: if odd {
                        vec!["RM-0001".to_owned()]
                    } else {
                        Vec::new()
                    },
                    reviewer: Some("Bob".to_owned()),
                    estimate: Some("3".to_owned()),
                    due: Some(if k <= 2 { "2026-11-10" } else { "2026-11-30" }.to_owned()),
                    parent: (!odd).then(|| card_id(0, k - 1)),
                    now: at(1),
                    ..CardNewRequest::default()
                },
            )
            .expect("card new");
            assert_eq!(outcome.id.as_deref(), Some(card_id(0, k).as_str()));
            let id = card_id(0, k);
            for day in 1..=3 {
                self.moved(&id, "todo", None, day);
            }
            self.moved(&id, "doing", None, 4);
            self.moved(&id, "review", None, 5);
            self.moved(&id, "returned", Some("Too long"), 6);
            let edited = card_edit(
                &env,
                &Globals::default(),
                &CardEditRequest {
                    id: id.clone(),
                    title: None,
                    goal: None,
                    targets: Vec::new(),
                    criteria: Vec::new(),
                    estimate: Some("5".to_owned()),
                    due: None,
                    parent: None,
                    corrects: None,
                    evidence: Vec::new(),
                    tasks: Vec::new(),
                    mandate: None,
                    reason: Some("The ledger doubled".to_owned()),
                    revision: None,
                    actor: Some("Ann".to_owned()),
                    now: at(7),
                    tasks_here: true,
                },
            )
            .expect("card edit");
            assert_eq!(edited.exit(), Exit::Answered, "{edited:?}");
            self.moved(&id, "review", None, 8);
            if odd {
                let mut yes = |_: &str| true;
                let accepted = card_accept(
                    &env,
                    &Globals::default(),
                    &CardAcceptRequest {
                        id: id.clone(),
                        actor: "Bob".to_owned(),
                        briefed: true,
                        now: at(15),
                        ..CardAcceptRequest::default()
                    },
                    &mut yes,
                )
                .expect("card accept");
                assert_eq!(accepted.exit(), Exit::Answered, "{accepted:?}");
            } else {
                self.moved(&id, "review", None, 9);
            }
        }
        let asked = propose_question(
            &env,
            &Globals::default(),
            &QuestionRequest {
                node_ids: vec!["RM-0001".to_owned()],
                text: "Does the ledger close weekly?".to_owned(),
                working_answer: "Yes, weekly.".to_owned(),
                price_of_other: "A page redone.".to_owned(),
                severity: None,
                distinct_from: Vec::new(),
                card: Some(card_id(0, 1)),
                author_role: Some("developer".to_owned()),
                author_model: Some("m".to_owned()),
                run: None,
                login: None,
                now: at(9),
                git: git.clone(),
            },
        )
        .expect("a question");
        assert_eq!(asked.document.id.as_deref(), Some("PR-0001"), "{asked:?}");
    }

    /// The template's rows repeated into `groups` groups and restored.
    fn repeat(&self) {
        let db = db_under(&self.home);
        let template = SqliteQueue::open(&db, SLUG)
            .expect("the queue")
            .stored_rows()
            .expect("the rows");
        assert_eq!(template.cards.len(), GROUP);
        let card_events: Vec<&StoredEvent> = template
            .events
            .iter()
            .filter(|event| {
                event.columns[1]
                    .as_deref()
                    .is_some_and(|kind| kind.starts_with("card."))
            })
            .collect();
        assert_eq!(card_events.len(), GROUP * 10, "ten card events a card");
        let mut rows = template.clone();
        rows.cards.clear();
        rows.events.clear();
        rows.proposals.clear();
        let (id, title, targets, parent) = (
            card_column("id"),
            card_column("title"),
            card_column("targets"),
            card_column("parent"),
        );
        let mut seq = 0;
        for g in 0..self.groups {
            let rename = |text: &str| {
                let mut out = text.to_owned();
                for k in (1..=GROUP).rev() {
                    out = out.replace(
                        &format!("\"{}\"", card_id(0, k)),
                        &format!("\"{}\"", card_id(g, k)),
                    );
                }
                out
            };
            for card in &template.cards {
                let mut row: StoredCard = card.clone();
                let k: usize = card.columns[id].as_deref().expect("an ID")[2..]
                    .parse()
                    .expect("a number");
                row.columns[id] = Some(card_id(g, k));
                row.columns[title] = Some(format!("Ledger line {}", g * GROUP + k));
                if let Some(written) = &row.columns[targets] {
                    row.columns[targets] =
                        Some(written.replace("RM-0001", &format!("RM-{:04}", g % ITEMS + 1)));
                }
                if row.columns[parent].is_some() {
                    row.columns[parent] = Some(card_id(g, k - 1));
                }
                row.position = card.position.map(|position| {
                    if position == 0 {
                        0
                    } else {
                        i64::try_from(g).expect("a group") * 5 + position
                    }
                });
                rows.cards.push(row);
            }
            for event in &card_events {
                seq += 1;
                let mut row = (*event).clone();
                row.seq = seq;
                row.columns[2] = row.columns[2].as_deref().map(rename);
                rows.events.push(row);
            }
        }
        let question = &template.proposals[0];
        let (pid, card) = (proposal_column("id"), proposal_column("card_id"));
        for g in 0..self.groups {
            let mut row: StoredProposal = question.clone();
            row.columns[pid] = Some(format!("PR-{:04}", g + 1));
            row.columns[card] = Some(card_id(g, 1));
            rows.proposals.push(row);
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
        let queue = SqliteQueue::open_existing(&db, SLUG)
            .expect("the queue")
            .expect("there");
        let listed = queue.list_existing_cards().expect("cards");
        assert!(
            listed.unreadable.is_empty(),
            "{:?}",
            listed.unreadable.first()
        );
        assert_eq!(listed.cards.len(), self.groups * GROUP);
    }

    /// The library's read at `AS_OF` in `view`, as JSON.
    fn read(&self, view: View) -> Value {
        let outcome = delivery_with_view(
            &self.env(),
            &Globals::default(),
            &DeliveryRequest {
                as_of: Some(AS_OF.to_owned()),
            },
            view,
        )
        .expect("delivery");
        serde_json::from_str(&render_json(&specengine_cli::Outcome::Delivery(Box::new(
            outcome,
        ))))
        .expect("JSON")
    }

    /// The queue half alone: the three store reads.
    fn queue_half(&self) -> usize {
        let queue = SqliteQueue::open_existing(db_under(&self.home), SLUG)
            .expect("the queue")
            .expect("there");
        let cards = queue.list_existing_cards().expect("cards").cards.len();
        let events = queue
            .events_of_types(&[
                EVENT_CARD_MOVED,
                EVENT_CARD_ACCEPTED,
                EVENT_CARD_REESTIMATED,
            ])
            .expect("events")
            .len();
        let items = queue
            .list_readable(&ProposalFilter {
                git_common_dir: None,
                statuses: Vec::new(),
            })
            .expect("items")
            .proposals
            .len();
        cards + events + items
    }
}

/// The counts the seed gives `cards` cards: every one placed, half
/// delivered (made 11-01, accepted 11-15: 14 days), one in ten delivered
/// late (due 11-10) and one in ten overdue at 11-20 (due 11-10).
fn expected_total(cards: usize) -> Value {
    serde_json::json!({"tasks": cards, "delivered": cards / 2, "delivered_late": cards / 10,
        "open": cards / 2, "overdue": cards / 10, "cancelled": 0,
        "lead_days": {"median": 14, "max": 14}})
}

/// DA-10 at 1 000 cards: the read's counts are the seed's; the CLI's JSON
/// is cut, the browser view lists every late row.
#[test]
fn da10_the_counts_hold_at_1000_cards() {
    let seeded = Seeded::new("counts", 100);
    let cards = seeded.groups * GROUP;
    let browser = seeded.read(View::Browser);
    assert_eq!(browser["notes"], serde_json::json!([]));
    assert_eq!(browser["total"], expected_total(cards));
    let placed: u64 = browser["releases"]
        .as_array()
        .expect("releases")
        .iter()
        .map(|row| {
            assert_ne!(row["group"], "none", "every card placed: {row}");
            row["tasks"].as_u64().expect("tasks")
        })
        .sum();
    assert_eq!(placed, cards as u64);
    let late = browser["late"].as_array().expect("late");
    assert_eq!(late.len(), cards / 5, "one late and one overdue a group");
    let first = &late[0];
    assert_eq!(first["days_late"], 10);
    assert_eq!(first["returns"].as_array().expect("returns").len(), 1);
    assert_eq!(
        first["reestimates"].as_array().expect("reestimates").len(),
        1
    );
    let capped = seeded.read(View::Capped);
    assert_eq!(capped["truncated"], true);
    let shown = capped["late"].as_array().expect("late");
    assert_eq!(
        shown.len() + capped["omitted"].as_u64().expect("omitted") as usize,
        late.len()
    );
    assert_eq!(shown[..], late[..shown.len()]);
    // Each card's nine moves, accepts and re-estimates (its `card.created`
    // is not read), every card and every item.
    assert_eq!(seeded.queue_half(), cards + cards * 9 + seeded.groups);
}

/// The release `spec` beside this test binary (`target.noindex/release`).
fn release_spec() -> PathBuf {
    if let Some(path) = std::env::var_os("SPECENGINE_DA10_SPEC") {
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

/// DA-10 [measure]: 20 000 cards, 200 000 card events, 2 000 items, a
/// 100-item plan; `spec delivery --as-of 2026-11-20 --json` in release,
/// warm, at most 1 s; the library's read and the queue half alone timed
/// beside it.
#[test]
#[ignore = "DA-10 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test delivery_scale --run-ignored \
            only --no-capture"]
fn da10_measure_20000_cards() {
    let started = Instant::now();
    let seeded = Seeded::new("measure", 2_000);
    let seeding = started.elapsed();
    let cards = seeded.groups * GROUP;
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
            .args(["delivery", "--as-of", AS_OF, "--json"])
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
    assert_eq!(document["total"], expected_total(cards));
    // Peak memory of one run, macOS `time -l`'s maximum resident set size
    // (bytes), where that tool is.
    let peak = Path::new("/usr/bin/time")
        .exists()
        .then(|| {
            let output = Command::new("/usr/bin/time")
                .arg("-l")
                .arg(&spec)
                .env_clear()
                .env("HOME", &seeded.home)
                .current_dir(&seeded.root)
                .args(["delivery", "--as-of", AS_OF, "--json"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .output()
                .expect("time runs");
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .find(|line| line.contains("maximum resident set size"))
                .and_then(|line| line.split_whitespace().next()?.parse::<u64>().ok())
        })
        .flatten();
    let (library, _) = timed(|| {
        let _ = seeded.read(View::Browser);
    });
    let (queue, _) = timed(|| {
        let _ = seeded.queue_half();
    });
    println!(
        "DA-10: {cards} cards, {} card events, {} items, {ITEMS} plan items; seeding {:.1} s; \
         spec delivery --json (release, warm) median {} ms, runs {:?} ms; library read median \
         {} ms; queue half alone median {} ms; peak resident {} MB",
        cards * 10,
        seeded.groups,
        seeding.as_secs_f64(),
        binary.as_millis(),
        binary_runs
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        library.as_millis(),
        queue.as_millis(),
        peak.map_or_else(
            || "-".to_owned(),
            |bytes| format!("{:.1}", bytes as f64 / 1_048_576.0)
        )
    );
    assert!(
        binary <= BAR,
        "DA-10's bar: {} ms > 1 s",
        binary.as_millis()
    );
}
