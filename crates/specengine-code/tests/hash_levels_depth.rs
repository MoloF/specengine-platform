//! docs/features/hash-levels.md (5a-1) RD-7: the level walks are iterative
//! and the quote is linear.
//!
//! - `deep_nesting_does_not_overflow_the_level_walks`: every input is parsed
//!   and walked on a 256 KiB thread (as `hash_depth.rs`), so a recursive
//!   walk overflows this binary instead of passing on a generous stack.
//! - `quote_cost_is_linear`: many items on one line after and before a long
//!   blank stretch cost the order of the same items one per line (best of
//!   three, as `marker_levels.rs`): a quote that scans for its context lines
//!   per item is quadratic there.
//!
//! - docs/features/hash-levels-use-lists.md (5a-1c) TE-6:
//!   `nested_use_lists_count_against_the_cap`: a `use` list of two or more
//!   members is a run (N6) and counts against `MAX_USE_RUN_NESTING` with the
//!   N4 and N5 runs around it; `hundred_thousand_nested_use_lists_are_refused_in_time`:
//!   100 000 nested lists are refused on a 256 KiB thread within
//!   [`DEEP_LIST_BOUND`]; `hundred_thousand_single_member_lists_are_hashed_in_time`:
//!   a chain of 100 000 single-member lists forms no run and is hashed
//!   within the same bound. M6 N6 runs not counted against the cap.
//!
//! Inputs are generated here, never committed.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use specengine_code::hash::{ErrorCategory, MAX_USE_RUN_NESTING};
use specengine_code::{LevelState, RustParser, item_levels, module_levels};

const QUARTER_MB: usize = 256 * 1024;

/// What the level walks say about item 0 and the module of `source`, on a
/// thread of `stack` bytes, and whether the source has a parse error.
fn levels_on(source: String, stack: usize) -> (LevelState, LevelState, bool) {
    thread::Builder::new()
        .name("level-depth".to_owned())
        .stack_size(stack)
        .spawn(move || {
            let tree = RustParser::new()
                .expect("grammar loads")
                .parse(&source)
                .expect("parse completes");
            assert!(!tree.root_node().has_error(), "the input parses cleanly");
            let item = item_levels(&tree, &source, &[0]).remove(0).expect("item 0");
            let module = module_levels(&tree, &source);
            (item, module, tree.root_node().has_error())
        })
        .expect("the thread spawns")
        .join()
        .expect("the level walks must not panic or overflow")
}

/// `fn f() { R_0 }`, `R_k` a use run whose first member's attribute value
/// holds `R_{k+1}`: `levels + 1` runs nested one inside another.
fn nested_use_runs(levels: usize) -> String {
    format!(
        "fn f() {{\n{}use x; use y;\n{}}}\n",
        "#[a = {\n".repeat(levels),
        "0 }] use x; use y;\n".repeat(levels)
    )
}

#[test]
fn deep_nesting_does_not_overflow_the_level_walks() {
    const DEEP: usize = 20_000;
    let hashed = [
        (
            "nested blocks",
            format!(
                "fn f() -> u8 {{ {}1{} }}\n",
                "{ ".repeat(DEEP),
                " }".repeat(DEEP)
            ),
        ),
        (
            "nested parentheses and calls",
            format!(
                "fn f() -> u8 {{ {}1{} }}\n",
                "g(".repeat(DEEP),
                ")".repeat(DEEP)
            ),
        ),
        (
            "nested closures",
            format!("fn f() {{ {}0; }}\n", "|| ".repeat(DEEP)),
        ),
        (
            "nested inline mods",
            format!(
                "{}fn h() {{}}{}\n",
                "mod m { ".repeat(2_000),
                " }".repeat(2_000)
            ),
        ),
    ];
    for (what, source) in hashed {
        let (item, module, _) = levels_on(source, QUARTER_MB);
        assert!(
            matches!(item, LevelState::Hashed(_)),
            "{what}: item 0 {item:?}"
        );
        assert!(
            matches!(module, LevelState::Hashed(_)),
            "{what}: the module {module:?}"
        );
    }
    // Runs nested past the cap: refused, never hashed, and no overflow.
    let too_deep = LevelState::CannotVerify {
        categories: vec![ErrorCategory::NestingTooDeep],
    };
    for levels in [MAX_USE_RUN_NESTING, 20_000] {
        let (item, module, has_error) = levels_on(nested_use_runs(levels), QUARTER_MB);
        assert!(!has_error);
        assert_eq!(item, too_deep, "{levels} levels: the item");
        assert_eq!(module, too_deep, "{levels} levels: the module");
    }
    // Within the cap: hashed.
    let (item, module, _) = levels_on(nested_use_runs(MAX_USE_RUN_NESTING - 1), QUARTER_MB);
    assert!(matches!(item, LevelState::Hashed(_)), "{item:?}");
    assert!(matches!(module, LevelState::Hashed(_)), "{module:?}");
}

/// `slow` costs the same order as `fast`: at most `factor` times it plus a
/// fixed allowance for scheduler noise.
fn assert_same_order(what: &str, slow: Duration, fast: Duration, factor: u32) {
    let bound = fast * factor + Duration::from_millis(250);
    eprintln!("{what}: {slow:?} against {fast:?}, bound {bound:?}");
    assert!(
        slow <= bound,
        "{what}: {slow:?} against {fast:?} (bound {bound:?}): the quote is not linear"
    );
}

/// Best of three `item_levels` over every one of `count` items of `source`.
fn best_of_three(source: &str, count: usize) -> Duration {
    let tree = RustParser::new()
        .expect("grammar loads")
        .parse(source)
        .expect("parse completes");
    let all: Vec<usize> = (0..count).collect();
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let levels = item_levels(&tree, source, &all);
        best = best.min(start.elapsed());
        assert!(
            levels
                .iter()
                .all(|state| matches!(state, Some(LevelState::Hashed(_))))
        );
    }
    assert_eq!(
        item_levels(&tree, source, &[count]),
        [None],
        "{count} items"
    );
    best
}

#[test]
fn quote_cost_is_linear() {
    const ITEMS: usize = 4_000;
    let stretch = "  \n\t\n".repeat(100_000);
    let one_per_line: String = (0..ITEMS).map(|i| format!("fn f{i}() {{}}\n")).collect();
    let one_line = format!(
        "{}\n",
        (0..ITEMS)
            .map(|i| format!("fn f{i}() {{}}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let baseline = best_of_three(&format!("{stretch}{one_per_line}{stretch}"), ITEMS);
    let crowded = best_of_three(&format!("{stretch}{one_line}{stretch}"), ITEMS);
    assert_same_order(
        "4 000 items on one line between blank stretches",
        crowded,
        baseline,
        10,
    );
    let packed = best_of_three(&one_line, ITEMS);
    assert_same_order("4 000 items on one line", packed, baseline, 10);
}

// ------------------------------------------------------- 5a-1c TE-6 (N6)

/// `fn f() { use x::{z, a::{z, a::{ … {z, y} … }}}; <tail> }`: `lists` use
/// lists nested one inside another, each of two members (an N6 run each).
/// `tail` after the `use` (another `use` makes the declaration an N4 run).
fn nested_use_lists(lists: usize, tail: &str) -> String {
    assert!(lists >= 1);
    format!(
        "fn f() {{\n    use x::{}{{z, y}}{};\n    {tail}\n}}\n",
        "{z, a::".repeat(lists - 1),
        "}".repeat(lists - 1)
    )
}

#[test]
fn nested_use_lists_count_against_the_cap() {
    let too_deep = LevelState::CannotVerify {
        categories: vec![ErrorCategory::NestingTooDeep],
    };
    // Lists alone: MAX_USE_RUN_NESTING lists hash, one more is refused.
    let (item, module, _) = levels_on(
        nested_use_lists(MAX_USE_RUN_NESTING, "let _ = 0;"),
        QUARTER_MB,
    );
    assert!(
        matches!(item, LevelState::Hashed(_)),
        "{MAX_USE_RUN_NESTING} nested lists are within the cap: {item:?}"
    );
    assert!(matches!(module, LevelState::Hashed(_)), "{module:?}");
    for lists in [MAX_USE_RUN_NESTING + 1, MAX_USE_RUN_NESTING + 2, 2_000] {
        let (item, module, has_error) =
            levels_on(nested_use_lists(lists, "let _ = 0;"), QUARTER_MB);
        assert!(!has_error);
        assert_eq!(item, too_deep, "{lists} nested lists: the item");
        assert_eq!(module, too_deep, "{lists} nested lists: the module");
    }
    // N4 and N6 counted together: the declaration and a second `use` are an
    // N4 run, so one list fewer fits.
    let (item, module, _) = levels_on(
        nested_use_lists(MAX_USE_RUN_NESTING - 1, "use w;"),
        QUARTER_MB,
    );
    assert!(
        matches!(item, LevelState::Hashed(_)),
        "an N4 run and {} lists: {item:?}",
        MAX_USE_RUN_NESTING - 1
    );
    assert!(matches!(module, LevelState::Hashed(_)), "{module:?}");
    let (item, module, _) = levels_on(nested_use_lists(MAX_USE_RUN_NESTING, "use w;"), QUARTER_MB);
    assert_eq!(
        item, too_deep,
        "an N4 run and {MAX_USE_RUN_NESTING} lists: the item"
    );
    assert_eq!(module, too_deep, "the module");
    // A list of one member is no run: it does not count.
    let single = format!(
        "fn f() {{\n    use x::{}{{z, y}}{};\n}}\n",
        "{a::".repeat(500),
        "}".repeat(500)
    );
    let (item, _, _) = levels_on(single, QUARTER_MB);
    assert!(
        matches!(item, LevelState::Hashed(_)),
        "500 single-member lists around one run: {item:?}"
    );
}

/// The level walks of 100 000 nested lists (parsing excluded) end within
/// this bound, debug build included: a refusal at the cap walks 65 lists,
/// a chain of single-member lists (no run) is walked once.
const DEEP_LIST_BOUND: Duration = Duration::from_secs(5);

/// Parses `source` and walks item 0's and the module's levels on a 256 KiB
/// thread; the walks (not the parse) timed. Waits at most twelve times
/// [`DEEP_LIST_BOUND`] for the thread, so a quadratic walk fails here
/// instead of hanging the run.
fn levels_in_time(what: &str, source: String) -> (LevelState, LevelState, Duration) {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("level-lists".to_owned())
        .stack_size(QUARTER_MB)
        .spawn(move || {
            let tree = RustParser::new()
                .expect("grammar loads")
                .parse(&source)
                .expect("parse completes");
            assert!(!tree.root_node().has_error(), "the input parses cleanly");
            let start = Instant::now();
            let item = item_levels(&tree, &source, &[0]).remove(0).expect("item 0");
            let module = module_levels(&tree, &source);
            let elapsed = start.elapsed();
            let _ = sender.send((item, module, elapsed));
        })
        .expect("the thread spawns");
    let (item, module, elapsed) = receiver
        .recv_timeout(DEEP_LIST_BOUND * 12)
        .unwrap_or_else(|error| {
            panic!(
                "{what}: the parse and the level walks must end within {:?}, without a panic: {error}",
                DEEP_LIST_BOUND * 12
            )
        });
    eprintln!("{what}: level walks {elapsed:?}");
    assert!(
        elapsed <= DEEP_LIST_BOUND,
        "{what}: the level walks took {elapsed:?}, bound {DEEP_LIST_BOUND:?}"
    );
    (item, module, elapsed)
}

#[test]
fn hundred_thousand_nested_use_lists_are_refused_in_time() {
    // A walk past the cap (M6) copies every member's buffer up through
    // every list and never ends in time.
    let (item, module, _) = levels_in_time(
        "100 000 nested two-member lists",
        nested_use_lists(100_000, "let _ = 0;"),
    );
    let too_deep = LevelState::CannotVerify {
        categories: vec![ErrorCategory::NestingTooDeep],
    };
    assert_eq!(item, too_deep, "the item");
    assert_eq!(module, too_deep, "the module");
}

/// `fn f() { use x::{a::{a::{ … {z} … }}}; }`: `lists` single-member lists
/// nested one inside another. None is a run (N6 needs two members), so the
/// cap never trips: the item and the module are hashed, in linear time
/// (a run search that copies the cursor per list is quadratic here).
#[test]
fn hundred_thousand_single_member_lists_are_hashed_in_time() {
    const LISTS: usize = 100_000;
    let source = format!(
        "fn f() {{\n    use x::{}{{z}}{};\n}}\n",
        "{a::".repeat(LISTS - 1),
        "}".repeat(LISTS - 1)
    );
    let (item, module, _) = levels_in_time("100 000 nested single-member lists", source);
    assert!(
        matches!(item, LevelState::Hashed(_)),
        "no run, no cap: the item {item:?}"
    );
    assert!(
        matches!(module, LevelState::Hashed(_)),
        "no run, no cap: the module {module:?}"
    );
}
