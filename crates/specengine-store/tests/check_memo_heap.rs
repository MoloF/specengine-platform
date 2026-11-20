//! docs/features/write-memory.md (RM-0132) WM7 [fix]: the check memo's D
//! entries against the heap they really hold, measured by a counting global
//! allocator that lives in this test binary alone (no allocator crate).
//!
//! The allocator counts, per thread, the bytes each allocation holds minus
//! the bytes each free gives back, both as asked for (`Layout::size`) and
//! as the allocator really holds them (its block: `malloc_size` on macOS;
//! with glibc `malloc_usable_size` plus the chunk's 8-byte header, the
//! accounting's like; elsewhere the size asked). A memo's D is emptied by
//! [`CheckMemo::doubt`] on the measuring thread, so what that call frees is
//! the heap D's entries held: their map nodes, the keys' and the values'
//! buffers, every string and path they own. The real heap is the blocks.
//!
//! - `wm7_the_memo_holds_no_more_heap_than_it_counts`: on two corpora of
//!   different nature (`spec-a`, a game design; `plant-diary`, a household
//!   app), a daemon's memo ([`CheckMemo::turned`]) of [`CAP`] bytes is filled
//!   by writes, each a new D entry (fresh bytes; half of them introduce a
//!   dangling reference, half nothing), far past its cap. After every write
//!   its accounted bytes stay within the cap; at the end D's real heap is at
//!   most its accounted bytes, and those at most 1.5 times the real heap; D's
//!   count stops growing once the cap is reached.
//!   M: the old `findings_bytes` (at 0.49.1 about 300 B counted of about
//!   530 B held per entry here): the real heap exceeds the accounted bytes,
//!   red.
//! - `wm7_d_holds_no_root_path`: one corpus copied to a short and to a long
//!   root (400 bytes longer), each a daemon's memo, the same writes in each:
//!   D's entries hold the same real heap and count the same bytes, whatever
//!   the root's length (no root path in D's key).
//!   M: the root back in D's key: red.
//! - `wm7_d_keeps_the_login`: the same write under another login is a miss,
//!   under its own a hit (the login stays in D's key: "No findings cross
//!   logins").
//!   M (WM9): D's key without the login: red.
//! - `wm7_the_stdio_memos_keep_two_roots_apart`: a stdio server's memos
//!   ([`PerRoot`]) over one corpus copied to two roots, the same write in
//!   each: each root keeps its own D entry (no lookup crosses roots).
//!   M: one memo for every root: red.

#![cfg(unix)]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use common::{Scratch, copy_dir, fixture};
use specengine_store::check_memo::{self, PerRoot, scoped, scoped_per_root};
use specengine_store::{
    CheckMemo, IndexWriter as _, NamedBytes, SetupFiles, SqliteIndex, WorkingTree,
    default_baseline, introduced_from_rows, load_check,
};

// ------------------------------------------------------------ the allocator

/// The system allocator, counting.
struct Counting;

thread_local! {
    /// This thread's allocated bytes minus its freed bytes, as asked for
    /// (`Layout::size`). `const`, no destructor: no allocation of its own.
    static ASKED: Cell<i64> = const { Cell::new(0) };
    /// The same by the blocks the allocator really holds for them
    /// ([`block`]).
    static HELD: Cell<i64> = const { Cell::new(0) };
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
/// `asked`: macOS's `malloc_size`; glibc's chunk, `malloc_usable_size` and
/// its header, as the memo's glibc rounding counts it; elsewhere `asked`.
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

fn count(asked: i64, held: i64) {
    let _ = ASKED.try_with(|net| net.set(net.get() + asked));
    let _ = HELD.try_with(|net| net.set(net.get() + held));
}

fn signed(size: usize) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

// SAFETY: every call is forwarded to `System` unchanged; the counting reads
// the block's size and touches const thread-local `Cell`s, never allocating.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            count(signed(layout.size()), signed(block(ptr, layout.size())));
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            count(signed(layout.size()), signed(block(ptr, layout.size())));
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let held = block(ptr, layout.size());
        // SAFETY: the caller's contract, forwarded.
        unsafe { System.dealloc(ptr, layout) };
        count(-signed(layout.size()), -signed(held));
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old = block(ptr, layout.size());
        // SAFETY: the caller's contract, forwarded.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            count(
                signed(new_size) - signed(layout.size()),
                signed(block(new, new_size)) - signed(old),
            );
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// This thread's net (asked, held) bytes.
fn net() -> (i64, i64) {
    (ASKED.with(Cell::get), HELD.with(Cell::get))
}

// ------------------------------------------------------------ the world

const TODAY: &str = "2026-11-20";

/// The cap of the filled memo, bytes: room for a few hundred D entries.
const CAP: usize = 128 * 1024;

/// How far past its cap the memo is filled: the writes are at least this
/// many times the D entries it holds at the end.
const FILL: usize = 5;

/// One corpus, copied under `root`, its index walked once.
struct World {
    _scratch: Scratch,
    root: PathBuf,
    config: NamedBytes,
    baseline: Option<NamedBytes>,
    setup: specengine_store::CheckSetup,
    index: SqliteIndex,
    /// The document the writes edit, its text, and an ID it does not define.
    path: String,
    text: String,
    dangling: String,
}

impl World {
    /// `corpus` copied to `<scratch>/<under>/root`; the writes edit `path`
    /// and cite `dangling`.
    fn new((corpus, path, dangling): (&str, &str, &str), under: &str) -> Self {
        let scratch = Scratch::new(&format!("wm7-{corpus}"));
        let root = scratch.path().join(under).join("root");
        copy_dir(&fixture(corpus), &root);
        let root = fs::canonicalize(&root).expect("canonical root");
        let config = NamedBytes::read("specengine.toml", &root.join("specengine.toml"));
        let baseline = default_baseline(&root);
        let setup = load_check(&config, baseline.as_ref()).expect("the corpus's check setup");
        let slug = setup.project.slug().unwrap_or("corpus").to_owned();
        let mut index = SqliteIndex::open(scratch.db("index"), &slug, &root).expect("open");
        let tree = WorkingTree::new(&root, &setup.project.paths).expect("a tree");
        index
            .update(&tree, &setup.project.scheme)
            .expect("the walk");
        let text = fs::read_to_string(root.join(path)).expect("the edited document");
        Self {
            _scratch: scratch,
            root,
            config,
            baseline,
            setup,
            index,
            path: path.to_owned(),
            text,
            dangling: dangling.to_owned(),
        }
    }

    /// Write `n`'s new text: the document plus a fresh line; odd `n` also
    /// cites an ID nobody defines (a dangling reference, so its D entry
    /// holds findings), even `n` nothing new.
    fn edit(&self, n: usize) -> Vec<u8> {
        let line = if n % 2 == 1 {
            format!(
                "\nNote {n} on the night watch, see [[{}]].\n",
                self.dangling
            )
        } else {
            format!("\nNote {n} on the night watch.\n")
        };
        format!("{}{line}", self.text).into_bytes()
    }

    /// The check of write `n` with whatever memo is in force: the number
    /// of findings.
    fn check(&self, n: usize) -> usize {
        let tree = WorkingTree::new(&self.root, &self.setup.project.paths).expect("a tree");
        let files = SetupFiles {
            config: &self.config,
            baseline: self.baseline.as_ref(),
        };
        introduced_from_rows(
            Some(&self.index),
            &tree,
            &self.setup,
            files,
            TODAY,
            &self.path,
            self.edit(n),
        )
        .findings
        .len()
    }

    /// The check of write `n` under `login`, with `memo` in force inside
    /// its turn, as a daemon runs it: the number of findings.
    fn write(&self, memo: &Arc<CheckMemo>, login: &str, n: usize) -> usize {
        memo.hold_turn()
            .within(|| scoped(memo, login, || self.check(n)))
    }
}

/// What emptying a memo's D on this thread gave back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Emptied {
    /// D's entries.
    entries: usize,
    /// The bytes the memo counted for them.
    counted: usize,
    /// The heap freed, as asked for.
    asked: usize,
    /// The heap freed, as the allocator held it: the real heap.
    held: usize,
}

impl Emptied {
    fn line(&self) -> String {
        let each = |bytes: usize| bytes as f64 / self.entries.max(1) as f64;
        format!(
            "D {} entries, counted {} B ({:.0} B each), real heap {} B ({:.0} B each; {} B asked)",
            self.entries,
            self.counted,
            each(self.counted),
            self.held,
            each(self.held),
            self.asked
        )
    }
}

/// D emptied on this thread by `empty` (a memo's or the scope's doubt),
/// `memos` the memos it reaches: what it gave back.
fn emptied(memos: &[&CheckMemo], empty: impl FnOnce()) -> Emptied {
    let entries = || memos.iter().map(|memo| memo.entries()).sum::<usize>();
    let bytes = || memos.iter().map(|memo| memo.bytes()).sum::<usize>();
    let (entries_before, counted_before) = (entries(), bytes());
    let before = net();
    empty();
    let after = net();
    let freed = |at: i64, now: i64| usize::try_from(at - now).expect("emptying D frees");
    Emptied {
        entries: entries_before - entries(),
        counted: counted_before - bytes(),
        asked: freed(before.0, after.0),
        held: freed(before.1, after.1),
    }
}

/// `memo`'s D emptied ([`CheckMemo::doubt`]).
fn empty_d(memo: &CheckMemo) -> Emptied {
    emptied(&[memo], || memo.doubt())
}

// ------------------------------------------------------------ WM7

/// The two corpora, the document each edits and an ID it does not define.
const CORPORA: [(&str, &str, &str); 2] = [
    ("spec-a", "docs/spec/game.md", "DOM-NOWHERE"),
    ("plant-diary", "docs/spec/epc-watering.md", "EPC-NOWHERE"),
];

/// WM7: filled far past its cap, the memo counts at least the heap its D
/// holds and at most 1.5 times it, within its cap, its count bounded.
#[test]
fn wm7_the_memo_holds_no_more_heap_than_it_counts() {
    let mut broken = Vec::new();
    for spec in CORPORA {
        let corpus = spec.0;
        let world = World::new(spec, "short");
        let memo = Arc::new(CheckMemo::turned(CAP));
        let mut counts = Vec::new();
        let mut with_findings = 0;
        let mut n = 0;
        // Until the memo has evicted FILL times what it holds.
        while n < 200 || n < FILL * memo.entries() {
            if world.write(&memo, "writer-1", n) > 0 {
                with_findings += 1;
            }
            let bytes = memo.bytes();
            if bytes > CAP {
                broken.push(format!(
                    "{corpus}: after write {n} the memo counts {bytes} B > its cap {CAP} B"
                ));
            }
            counts.push(memo.entries());
            n += 1;
        }
        assert!(
            with_findings > 0 && with_findings < n,
            "{corpus}: {with_findings} of {n} writes with findings; the fill needs both"
        );
        let d = empty_d(&memo);
        assert!(d.entries > 0, "{corpus}: D holds nothing to measure");
        assert!(
            n >= FILL * d.entries,
            "{corpus}: {n} writes for {} entries kept: not far past the cap",
            d.entries
        );
        let line = format!("{corpus}: {n} writes, {}, cap {CAP} B", d.line());
        println!("{line}");
        if d.held > d.counted {
            broken.push(format!("the real heap > the counted bytes: {line}"));
        }
        if d.counted as f64 > 1.5 * d.held as f64 {
            broken.push(format!("the counted bytes > 1.5 x the real heap: {line}"));
        }
        let half = counts.len() / 2;
        let early = counts[..half].iter().copied().max().unwrap_or(0);
        let late = counts[half..].iter().copied().max().unwrap_or(0);
        if late > early + early / 10 + 1 {
            broken.push(format!(
                "{corpus}: D's count still grows: at most {early} entries over the first \
                 {half} writes, {late} over the rest"
            ));
        }
    }
    assert!(broken.is_empty(), "WM7:\n{}", broken.join("\n"));
}

/// The extra length of the long root, bytes.
const LONGER: usize = 400;

/// WM7, the smaller entry: D's heap and count do not follow the root's
/// length (no root path in D's key; a daemon's memo is its project's).
#[test]
fn wm7_d_holds_no_root_path() {
    let spec = CORPORA[0];
    let long_under = format!("{}/{}", "l".repeat(LONGER / 2), "o".repeat(LONGER / 2));
    let short = World::new(spec, "s");
    let long = World::new(spec, &long_under);
    let extra = long.root.as_os_str().len() - short.root.as_os_str().len();
    assert!(extra >= LONGER, "the long root is {extra} B longer");
    let writes = 40;
    let mut measured = Vec::new();
    for world in [&short, &long] {
        let memo = Arc::new(CheckMemo::turned(specengine_store::check_memo::BUDGET));
        for n in 0..writes {
            world.write(&memo, "writer-1", n);
        }
        measured.push(empty_d(&memo));
    }
    let (short_d, long_d) = (measured[0], measured[1]);
    assert_eq!(
        (short_d.entries, long_d.entries),
        (writes, writes),
        "each write one D entry"
    );
    let line = format!(
        "under a root {extra} B longer: {} -> {}",
        short_d.line(),
        long_d.line()
    );
    println!("{line}");
    assert!(
        long_d == short_d,
        "D's entries grow with the root's length (the root in D's key): {line}"
    );
}

/// WM7 and WM9, the login stays in D's key: the same write under another
/// login misses, under its own hits ("No findings cross logins").
#[test]
fn wm7_d_keeps_the_login() {
    let short = World::new(CORPORA[0], "s");
    let memo = Arc::new(CheckMemo::turned(specengine_store::check_memo::BUDGET));
    let findings = short.write(&memo, "ann", 1);
    assert_eq!(findings, short.write(&memo, "bob", 1), "the same findings");
    short.write(&memo, "ann", 1);
    let by_login = memo.lookups_by_login();
    assert_eq!(
        (by_login.get("ann"), by_login.get("bob")),
        (Some(&(1, 1)), Some(&(0, 1))),
        "ann: one miss, then a hit; bob: a miss, never ann's entry"
    );
    assert_eq!(memo.hits(), 1, "{by_login:?}");
}

/// WM7, a stdio server's memos ([`PerRoot`], no turn, no login): one corpus
/// copied to two roots (the same content, so the same walk digest and the
/// same D key), the same write in each. Each root's memo keeps its own
/// entry, so the scope's doubt frees twice the heap one root's write
/// leaves; the second root's write is no hit on the first's entry.
/// M: one memo for every root (D, without its root, shared): the second
/// write a hit, one entry freed: red.
#[test]
fn wm7_the_stdio_memos_keep_two_roots_apart() {
    let one = World::new(CORPORA[0], "one");
    let two = World::new(CORPORA[0], "two");
    let freed = |worlds: &[&World]| -> Emptied {
        let memos = Arc::new(PerRoot::new());
        scoped_per_root(&memos, "-", || {
            for world in worlds {
                world.check(1);
            }
            emptied(&[], check_memo::doubt)
        })
    };
    let alone = freed(&[&one]);
    let both = freed(&[&one, &two]);
    let line = format!(
        "one root's write frees {} B ({} B asked), the same write in two roots {} B ({} B asked)",
        alone.held, alone.asked, both.held, both.asked
    );
    println!("{line}");
    assert!(alone.held > 0, "a stdio write keeps its D entry: {line}");
    assert_eq!(
        (both.held, both.asked),
        (2 * alone.held, 2 * alone.asked),
        "each root its own D entry: {line}"
    );
}
