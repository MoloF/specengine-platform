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
//! Inputs are generated here, never committed.

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
