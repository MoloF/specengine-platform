//! docs/features/mixed-load-memory.md (RM-0134) MX3 [peak] and MX4 [closed
//! rows not fetched]: one inbox read's heap, measured by a counting global
//! allocator that lives in this test binary alone (the pattern of
//! `specengine-store/tests/check_memo_heap.rs`, WM7). The allocator keeps,
//! per thread, the bytes the allocator really holds for the live blocks
//! (`malloc_size` on macOS; glibc's `malloc_usable_size` plus its 8-byte
//! chunk header; elsewhere the size asked), their high-water mark, and the
//! number of allocations. SQLite's own pages are its own `malloc`'s, not
//! counted: what is measured is what SpecEngine's code holds.
//!
//! One read is `specengine_cli::inbox` (the current repository's `open`
//! and `approved` items, the notes) and its answer rendered as the daemon
//! sends it (`render_json`), after a first read has made the index. The
//! scene: a scratch git copy of `fixtures/spec-a`, its queue restored from
//! clones of one question raised through the store as a hub raises it (a
//! login, a role, a model, a run, six words of text), each clone its own ID
//! and text.
//!
//! - `mx3_one_reads_peak_rises_a_third_of_heads_per_item`: 1 000 and 4 000
//!   open items; the answer's bytes are 540db8ff's ([`HEAD_ANSWER_BYTES`]);
//!   on macOS the peak's rise per item between them is at most a third of
//!   [`HEAD_RISE_PER_ITEM`], 540db8ff's (measured first by this test with
//!   macOS's `malloc_size`; elsewhere the blocks are counted otherwise, so
//!   the rise is reported, not judged).
//!   M: the rows collected before decoding (540db8ff's `raw_rows_in`): red.
//! - `mx4_rejected_rows_cost_the_read_nothing`: 10 open items beside 0 and
//!   beside 4 000 rejected ones: the read's allocation counts differ by
//!   fewer than 1 000. M: the state prefilter dropped (each rejected row
//!   read into memory and skipped): red.
//!
//! Both print their numbers as one JSON line each (`--no-capture`).
//! At 540db8ff both are red by design: these are the slice's targets.

#![cfg(unix)]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::c_void;
use std::fs;

use common::proposal::Pair;
use serde_json::json;
use specengine_cli::{Globals, InboxRequest, Outcome, inbox, render_json};
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_store::{
    Decision, Intake, NewIntake, PROPOSAL_COLUMNS, Place, ProposalKind, ProposalQueue as _,
    Restore, SqliteQueue, StoredQueue,
};

// ------------------------------------------------------------ the allocator

/// The system allocator, counting.
struct Counting;

thread_local! {
    /// This thread's live bytes as the allocator holds them ([`block`]).
    /// `const`, no destructor: no allocation of its own.
    static HELD: Cell<i64> = const { Cell::new(0) };
    /// The high-water mark of [`HELD`] since the last [`reset_peak`].
    static PEAK: Cell<i64> = const { Cell::new(0) };
    /// This thread's allocations (`alloc`, `alloc_zeroed`, `realloc`).
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn malloc_size(ptr: *const c_void) -> usize;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_usable_size(ptr: *mut c_void) -> usize;
}

/// glibc's chunk header beside the usable bytes (`SIZE_SZ`, 64-bit).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const GLIBC_HEADER: usize = 8;

/// The bytes the allocator holds for the live block at `ptr`, asked
/// `asked`.
fn block(ptr: *mut u8, asked: usize) -> usize {
    #[cfg(target_os = "macos")]
    {
        let _ = asked;
        // SAFETY: `ptr` is a live block of the system allocator.
        unsafe { malloc_size(ptr.cast::<c_void>().cast_const()) }
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let _ = asked;
        // SAFETY: `ptr` is a live block of the system allocator.
        unsafe { malloc_usable_size(ptr.cast::<c_void>()) + GLIBC_HEADER }
    }
    #[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
    {
        let _ = ptr;
        asked
    }
}

fn signed(size: usize) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

fn count(held: i64, allocation: bool) {
    let _ = HELD.try_with(|live| {
        let now = live.get() + held;
        live.set(now);
        let _ = PEAK.try_with(|peak| {
            if now > peak.get() {
                peak.set(now);
            }
        });
    });
    if allocation {
        let _ = COUNT.try_with(|count| count.set(count.get() + 1));
    }
}

// SAFETY: every call is forwarded to `System` unchanged; the counting reads
// the block's size and touches const thread-local `Cell`s, never allocating.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            count(signed(block(ptr, layout.size())), true);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            count(signed(block(ptr, layout.size())), true);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let held = block(ptr, layout.size());
        // SAFETY: the caller's contract, forwarded.
        unsafe { System.dealloc(ptr, layout) };
        count(-signed(held), false);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old = block(ptr, layout.size());
        // SAFETY: the caller's contract, forwarded.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            count(signed(block(new, new_size)) - signed(old), true);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn held() -> i64 {
    HELD.with(Cell::get)
}

fn reset_peak() {
    let now = held();
    PEAK.with(|peak| peak.set(now));
}

fn peak() -> i64 {
    PEAK.with(Cell::get)
}

fn allocations() -> u64 {
    COUNT.with(Cell::get)
}

// ------------------------------------------------------------ the numbers

/// 540db8ff's peak rise per open item of one read between 1 000 and 4 000
/// open items, bytes, on macOS (`malloc_size`; this test's `mx3` line,
/// recorded before any `src/` change of the slice).
const HEAD_RISE_PER_ITEM: f64 = 3_652.352;

/// 540db8ff's answer bytes at 1 000 and 4 000 open items.
const HEAD_ANSWER_BYTES: [usize; 2] = [306_917, 1_230_917];

/// MX4's bound on the allocations 4 000 rejected rows may add to a read.
const MX4_MORE_ALLOCATIONS: u64 = 1_000;

// ------------------------------------------------------------ the scene

const RAISED: &str = "2026-11-20T09:00:00Z";
const DECIDED: &str = "2026-11-20T09:05:00Z";

/// A hub's question on `EDGE-SPRINT-EMPTY`, as the soak's fill raises it.
fn question(place: &Place, words: &str) -> NewIntake {
    let mut author = Author::new(
        Some("drafter".to_owned()),
        Some("m-1".to_owned()),
        Some("r-filler-3".to_owned()),
    )
    .expect("an agent");
    author.login = Some("filler-3".to_owned());
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/spec/movement/sprint.md".to_owned(),
        place: place.clone(),
        author,
        intake: Intake {
            target_ids: vec!["EDGE-SPRINT-EMPTY".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: format!("{words}?"),
            working_answer: Some("amber copper drift".to_owned()),
            price_of_other: Some("lumen ridge falcon".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        actor: None,
        card_id: None,
    }
}

fn column(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|known| *known == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

/// A spec-a pair whose queue holds `open` open questions, then `rejected`
/// rejected ones, of the current repository, by ID number (the open ones
/// the same at any `rejected`).
fn scene(label: &str, open: usize, rejected: usize) -> Pair {
    let pair = Pair::new(label, "spec-a");
    let common = fs::canonicalize(pair.main.join(".git")).expect("the common dir");
    let place = Place {
        git_common_dir: common.to_str().expect("UTF-8").to_owned(),
        worktree: pair.linked.to_str().expect("UTF-8").to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: pair.rev(&pair.main, "HEAD"),
    };
    let mut template =
        SqliteQueue::open(pair.scratch.join("template.db"), &pair.slug).expect("a template");
    for (n, words) in [
        (1, "harbor quill meadow ember sable tundra"),
        (2, "cinder violet harbor maple quartz ridge"),
    ] {
        let made = template
            .create_intake(&question(&place, words), &[], None, RAISED)
            .expect("an intake")
            .created
            .expect("stored");
        assert_eq!(made.id, format!("PR-{n:04}"));
    }
    let decision = Decision {
        decided_by: "Ann Owner <ann@example.invalid>".to_owned(),
        note: Some("Asked before.".to_owned()),
        staged_at: None,
    };
    template
        .reject("PR-0002", &decision, DECIDED)
        .expect("rejected");
    let rows = template.stored_rows().expect("the template's rows");
    let (open_row, rejected_row) = (&rows.proposals[0], &rows.proposals[1]);
    let mut state = StoredQueue::default();
    for at in 0..open + rejected {
        let mut row = if at < open {
            open_row.clone()
        } else {
            rejected_row.clone()
        };
        row.columns[column("id")] = Some(format!("PR-{:04}", at + 1));
        row.columns[column("summary")] =
            Some(format!("harbor quill meadow ember sable tundra {at}?"));
        state.proposals.push(row);
    }
    let db = pair.db();
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    let mut queue = SqliteQueue::open(&db, &pair.slug).expect("the queue");
    assert!(matches!(
        queue.restore(&state).expect("restored"),
        Restore::Restored
    ));
    pair
}

/// One read's numbers.
struct Measured {
    /// The peak of the bytes held above the read's start, through the
    /// answer rendered.
    peak: i64,
    /// What the read's outcome holds once it returns (the listed entries
    /// and the notes), and the render's own peak above that.
    outcome: i64,
    render: i64,
    /// Its allocations.
    allocations: u64,
    /// The answer's bytes and its listed items.
    answer: usize,
    listed: usize,
}

/// One inbox read of `pair` and its JSON answer, after a first read.
fn measure(pair: &Pair) -> Measured {
    let env = pair.env(&pair.main);
    let globals = Globals::default();
    let request = InboxRequest {
        all: false,
        git: pair.git_env(&pair.main),
    };
    let first = inbox(&env, &globals, &request).expect("a first read");
    drop(first);
    let start = held();
    let before = allocations();
    reset_peak();
    let outcome = inbox(&env, &globals, &request).expect("the read");
    let read = peak() - start;
    let listed = outcome.proposals.len();
    let wrapped = Outcome::Inbox(outcome);
    let kept = held() - start;
    reset_peak();
    let answer = render_json(&wrapped);
    let render = peak() - start - kept;
    let measured = Measured {
        peak: read.max(kept + render),
        outcome: kept,
        render,
        allocations: allocations() - before,
        answer: answer.len(),
        listed,
    };
    drop(wrapped);
    drop(answer);
    measured
}

/// MX3: the peak's rise per item between 1 000 and 4 000 open items is at
/// most a third of 540db8ff's; the answer is 540db8ff's size.
#[test]
fn mx3_one_reads_peak_rises_a_third_of_heads_per_item() {
    let small = measure(&scene("mx3-1000", 1_000, 0));
    let large = measure(&scene("mx3-4000", 4_000, 0));
    assert_eq!((small.listed, large.listed), (1_000, 4_000));
    #[allow(clippy::cast_precision_loss)]
    let rise = (large.peak - small.peak) as f64 / 3_000.0;
    println!(
        "{}",
        json!({
            "test": "mx3",
            "peak_1000": small.peak,
            "peak_4000": large.peak,
            "rise_per_item": rise,
            "outcome_1000": small.outcome,
            "outcome_4000": large.outcome,
            "render_1000": small.render,
            "render_4000": large.render,
            "allocations_1000": small.allocations,
            "allocations_4000": large.allocations,
            "answer_1000": small.answer,
            "answer_4000": large.answer,
        })
    );
    assert_eq!(
        [small.answer, large.answer],
        HEAD_ANSWER_BYTES,
        "the answer's bytes are 540db8ff's"
    );
    if !cfg!(target_os = "macos") {
        println!("mx3: the rise is judged on macOS alone (HEAD_RISE_PER_ITEM is malloc_size's)");
        return;
    }
    assert!(
        rise <= HEAD_RISE_PER_ITEM / 3.0,
        "the peak rises {rise:.0} B per item; 540db8ff's {HEAD_RISE_PER_ITEM:.0}, the bound a \
         third of it"
    );
}

/// MX4: 4 000 rejected rows beside 10 open ones add fewer than
/// [`MX4_MORE_ALLOCATIONS`] allocations to a read.
#[test]
fn mx4_rejected_rows_cost_the_read_nothing() {
    let alone = measure(&scene("mx4-alone", 10, 0));
    let beside = measure(&scene("mx4-beside", 10, 4_000));
    assert_eq!((alone.listed, beside.listed), (10, 10));
    assert_eq!(alone.answer, beside.answer, "one answer");
    println!(
        "{}",
        json!({
            "test": "mx4",
            "allocations_alone": alone.allocations,
            "allocations_beside_4000_rejected": beside.allocations,
            "peak_alone": alone.peak,
            "peak_beside_4000_rejected": beside.peak,
        })
    );
    assert!(
        beside.allocations < alone.allocations + MX4_MORE_ALLOCATIONS,
        "4 000 rejected rows add {} allocations to the read ({} beside {})",
        beside.allocations.saturating_sub(alone.allocations),
        beside.allocations,
        alone.allocations
    );
}
