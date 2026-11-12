//! docs/features/department-overview.md "Acceptance criteria", DO11: the
//! test seam that counts the store's statements per call of
//! [`SqliteQueue::card_counts`]. SQLite's own statement trace
//! (`sqlite3_trace_v2`, `SQLITE_TRACE_STMT`: once as each prepared
//! statement starts) is set on the queue's connection for one call and
//! cleared after it. The count is the same over the spec's scenario (7
//! cards, 4 items, 2 tasks) and over 2 000 cards with parents, items and
//! links: no statement runs per card, item or task. M: items counted per
//! card.

use std::ffi::{c_int, c_uint, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use rusqlite::ffi;
use specengine_core::card_counts::CardCountRows;
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;

use super::{
    CardChange, CardEdit, Intake, NewCard, NewIntake, NewTask, Place, ProposalKind,
    ProposalQueue as _, SqliteQueue,
};

/// The trace's callback: one statement started; `ctx` is the call's
/// counter.
unsafe extern "C" fn one_more(
    _event: c_uint,
    ctx: *mut c_void,
    _statement: *mut c_void,
    _text: *mut c_void,
) -> c_int {
    // SAFETY: `ctx` is the `AtomicUsize` of `statements`, alive while the
    // trace is set.
    let counter = unsafe { &*(ctx as *const AtomicUsize) };
    counter.fetch_add(1, Ordering::SeqCst);
    0
}

/// The statements one `card_counts(tasks_here)` call starts, and its rows.
fn statements(queue: &SqliteQueue, tasks_here: bool) -> (usize, CardCountRows) {
    let counter = AtomicUsize::new(0);
    // SAFETY: the handle is the queue's open connection; the trace is
    // cleared before `counter` goes out of scope.
    let handle = unsafe { queue.conn.handle() };
    unsafe {
        ffi::sqlite3_trace_v2(
            handle,
            ffi::SQLITE_TRACE_STMT,
            Some(one_more),
            (&raw const counter).cast_mut().cast::<c_void>(),
        );
    }
    let rows = queue.card_counts(tasks_here).expect("counts");
    unsafe {
        ffi::sqlite3_trace_v2(handle, 0, None, std::ptr::null_mut());
    }
    (counter.load(Ordering::SeqCst), rows)
}

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-store-counts-seam-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn card(flow: &str, column: &str, parent: Option<String>) -> NewCard {
    NewCard {
        flow: flow.to_owned(),
        column: column.to_owned(),
        title: "A seeded card".to_owned(),
        goal: None,
        discipline: Some(flow.to_owned()),
        targets: Vec::new(),
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        mandate: None,
        estimate: None,
        due: None,
        parent,
        corrects: None,
        by: "local".to_owned(),
    }
}

fn question(card: &str, n: usize) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/seam.md".to_owned(),
        place: Place {
            git_common_dir: "seam/.git".to_owned(),
            worktree: "seam".to_owned(),
            root_rel: String::new(),
            branch: "main".to_owned(),
            base_commit: "1".repeat(40),
        },
        author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: vec![format!("SEAM-{n:04}")],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: format!("Is seeded item {n} still open?"),
            working_answer: Some("Yes.".to_owned()),
            price_of_other: Some("None.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
        },
        card_id: Some(card.to_owned()),
        actor: None,
    }
}

/// `cards` cards over three flows (neutral names: no department's or
/// discipline's literal stands in `src`, `workspace_names.rs`), one in five under the card before, one
/// item in ten cards, one task per forty cards linked from an open card.
fn seeded(queue: &mut SqliteQueue, cards: usize) {
    let flows = ["lane-1", "lane-2", "lane-3"];
    let now = "2026-11-02T09:00:00Z";
    for k in 1..=cards {
        let parent = (k % 5 == 3).then(|| format!("C-{:04}", k - 1));
        queue
            .create_card(&card(flows[k % 3], "todo", parent), now)
            .expect("a card");
    }
    for n in 0..cards / 10 {
        let on = format!("C-{:04}", 10 * n + 7);
        queue
            .create_intake(&question(&on, n), &[], None, now)
            .expect("an item");
    }
    for t in 1..=(cards / 40).max(1) {
        queue
            .create_task(
                &NewTask {
                    git_common_dir: "seam/.git".to_owned(),
                    title: Some(format!("Seeded task {t}")),
                    goal: None,
                    targets: vec!["SEAM-0001".to_owned()],
                    author: Author::human(),
                },
                now,
            )
            .expect("a task");
        let id = format!("C-{:04}", 40 * (t - 1) + 5);
        let edit = CardEdit {
            tasks: Some(vec![format!("T-{t:04}")]),
            ..CardEdit::default()
        };
        queue
            .change_card(&id, None, &CardChange::Edit(Box::new(edit)), "ann", now)
            .expect("linked");
    }
}

/// DO11: the statements one call starts are the same at 7 cards and at
/// 2 000, with agent tasks and without; the trace counted something (a
/// positive control: a second call counts the same again).
#[test]
fn do11_one_read_runs_the_same_statements_at_any_size() {
    let scratch = Scratch::new("do11");
    let mut small = SqliteQueue::open(scratch.0.join("small.db"), "seam-queue").expect("open");
    seeded(&mut small, 7);
    let mut large = SqliteQueue::open(scratch.0.join("large.db"), "seam-queue").expect("open");
    seeded(&mut large, 2_000);
    for tasks_here in [true, false] {
        let (few, few_rows) = statements(&small, tasks_here);
        let (many, many_rows) = statements(&large, tasks_here);
        println!(
            "DO11 seam: tasks_here {tasks_here}: {few} statements at 7 cards, {many} at 2 000"
        );
        assert!(few > 0, "the trace counts: {few}");
        assert_eq!(
            few, many,
            "tasks_here {tasks_here}: {few} statements at 7 cards, {many} at 2 000"
        );
        assert_eq!(
            statements(&large, tasks_here).0,
            many,
            "one call, one count"
        );
        let total =
            |rows: &CardCountRows| -> u64 { rows.columns.values().map(|tally| tally.open).sum() };
        assert_eq!(total(&few_rows), 7);
        assert_eq!(total(&many_rows), 2_000);
        let items: u64 = many_rows.open_items.values().map(|items| items.count).sum();
        assert_eq!(items, 200);
    }
    // The figure itself, pinned at iteration 2: the transaction's opening
    // and closing, the schema probes and the six grouped statements
    // (`COUNT_UNNAMED` the sixth); without agent tasks the two task
    // statements and the `tasks` table's probe do not run.
    assert_eq!(statements(&large, true).0, 12, "with agent tasks");
    assert_eq!(statements(&large, false).0, 9, "without");
}
