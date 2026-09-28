//! Depth bounds of the hash layer (05 §5.2, N4 and `MAX_USE_RUN_NESTING`;
//! `hash.rs` module docs, "Depth"; `items.rs` `collect`):
//!
//! - use runs nested through the attribute values of run members are followed
//!   `hash::MAX_USE_RUN_NESTING` deep; one more makes the item
//!   `cannot_verify` with category `nesting_too_deep`, although the item
//!   parses cleanly, and `normalize` reports the walk incomplete;
//! - item collection walks nested inline `mod`s with an explicit stack, so a
//!   deep file never bounds the thread stack.
//!
//! Inputs are generated here, never committed; walks run on threads with an
//! explicit stack size so that a regression to unbounded recursion overflows
//! this binary instead of passing on a generous default stack.

use std::thread;

use specengine_code::hash::{
    self, ErrorCategory, HashState, MAX_USE_RUN_NESTING, hash_item, name_node, normalize,
};
use specengine_code::{FileAnalysis, RustParser, analyze_tree};
use tree_sitter::Tree;

const TWO_MB: usize = 2 * 1024 * 1024;
const QUARTER_MB: usize = 256 * 1024;

fn parse(source: &str) -> Tree {
    RustParser::new()
        .expect("grammar loads")
        .parse(source)
        .expect("parse completes")
}

/// `fn f() { R_0 }` where `R_k` is a use run of two members whose first
/// member's attribute value holds `R_{k+1}`, and the innermost run is plain:
/// `levels` attributes deep, `levels + 1` use runs nested one inside another.
fn nested_use_runs(levels: usize) -> String {
    format!(
        "fn f() {{\n{}use x; use y;\n{}}}\n",
        "#[a = {\n".repeat(levels),
        "0 }] use x; use y;\n".repeat(levels)
    )
}

/// What the hash layer says about the single function of `source`.
struct Outcome {
    root_has_error: bool,
    item_has_error: bool,
    state: HashState,
    complete: bool,
}

fn hash_function(source: String, stack: usize) -> Outcome {
    thread::Builder::new()
        .name("hash-depth".to_owned())
        .stack_size(stack)
        .spawn(move || {
            let tree = parse(&source);
            let root = tree.root_node();
            let item = root.named_child(0).expect("one item");
            assert_eq!(item.kind(), "function_item");
            let mut stream = Vec::new();
            let complete = normalize(item, source.as_bytes(), name_node(item), &mut stream);
            Outcome {
                root_has_error: root.has_error(),
                item_has_error: hash::item_has_error(item),
                state: hash_item(item, source.as_bytes()),
                complete,
            }
        })
        .expect("hash thread spawns")
        .join()
        .expect("hashing must not panic or overflow")
}

fn assert_too_deep(outcome: &Outcome, what: &str) {
    assert!(
        !outcome.root_has_error,
        "{what}: the source must parse cleanly"
    );
    assert!(
        !outcome.item_has_error,
        "{what}: no parse error on the item"
    );
    assert_eq!(
        outcome.state,
        HashState::CannotVerify {
            categories: vec![ErrorCategory::NestingTooDeep],
        },
        "{what}: nesting past the cap is cannot_verify, never a hash"
    );
    assert!(
        !outcome.complete,
        "{what}: normalize must report the walk incomplete"
    );
}

#[test]
fn nested_use_runs_up_to_the_cap_are_hashed() {
    let outcome = hash_function(nested_use_runs(MAX_USE_RUN_NESTING - 1), TWO_MB);
    assert!(!outcome.root_has_error);
    assert!(!outcome.item_has_error);
    assert!(
        matches!(outcome.state, HashState::Hashed(_)),
        "{MAX_USE_RUN_NESTING} nested use runs are within the cap: {:?}",
        outcome.state
    );
    assert!(outcome.complete, "normalize must report the walk complete");
}

#[test]
fn one_nested_use_run_past_the_cap_is_cannot_verify() {
    let outcome = hash_function(nested_use_runs(MAX_USE_RUN_NESTING), TWO_MB);
    assert_too_deep(&outcome, "MAX_USE_RUN_NESTING + 1 runs");
}

#[test]
fn twenty_thousand_nested_use_runs_do_not_overflow() {
    let outcome = hash_function(nested_use_runs(20_000), TWO_MB);
    assert_too_deep(&outcome, "20 000 levels");
}

#[test]
fn nested_use_runs_hash_deterministically_and_differ_by_depth() {
    let digest = |levels| {
        hash_function(nested_use_runs(levels), TWO_MB)
            .state
            .digest()
            .expect("within the cap")
    };
    assert_eq!(digest(10), digest(10));
    assert_ne!(digest(10), digest(11));
}

#[test]
fn cannot_verify_through_analyze_tree_keeps_has_error_false() {
    let source = nested_use_runs(MAX_USE_RUN_NESTING);
    let analysis = thread::Builder::new()
        .stack_size(TWO_MB)
        .spawn(move || analyze_tree(&parse(&source), &source))
        .expect("thread spawns")
        .join()
        .expect("analysis must not panic");
    assert!(!analysis.has_error);
    assert_eq!(analysis.items.len(), 1);
    let item = &analysis.items[0];
    assert!(
        !item.has_error,
        "no parse error: the depth alone is the reason"
    );
    assert_eq!(
        item.state,
        HashState::CannotVerify {
            categories: vec![ErrorCategory::NestingTooDeep],
        }
    );
}

// ------------------------------------------------------ nested inline mods

fn analyze_on_stack(source: String, stack: usize) -> FileAnalysis {
    let tree = parse(&source);
    thread::Builder::new()
        .name("items-depth".to_owned())
        .stack_size(stack)
        .spawn(move || analyze_tree(&tree, &source))
        .expect("items thread spawns")
        .join()
        .expect("analyze_tree must not panic or overflow on deep mods")
}

#[test]
fn thousand_nested_mods_are_collected_in_pre_order_on_a_small_stack() {
    let depth = 1_000;
    let source = format!(
        "{}fn leaf() {{}}\n{}",
        "mod m {\n".repeat(depth),
        "}\n".repeat(depth)
    );
    let analysis = analyze_on_stack(source, QUARTER_MB);
    assert!(!analysis.has_error);
    assert_eq!(analysis.items.len(), depth + 1);
    for (index, item) in analysis.items.iter().enumerate() {
        let expected_kind = if index < depth {
            "mod_item"
        } else {
            "function_item"
        };
        assert_eq!(item.kind, expected_kind, "item {index}");
        assert_eq!(item.mod_path.len(), index, "item {index} mod_path length");
        assert!(item.mod_path.iter().all(|segment| segment == "m"));
        assert_eq!(
            item.line,
            index + 1,
            "pre-order: item {index} starts on its line"
        );
        assert!(
            matches!(item.state, HashState::Hashed(_)),
            "item {index}: {:?}",
            item.state
        );
    }
    assert_eq!(analysis.items[depth].label, "leaf");
}

#[test]
fn nested_mods_with_siblings_keep_pre_order() {
    // Each level: `mod m { fn a() {} <next level> fn z() {} }`.
    let depth = 300;
    let source = format!(
        "{}{}",
        "mod m { fn a() {}\n".repeat(depth),
        "fn z() {} }\n".repeat(depth)
    );
    let analysis = analyze_on_stack(source, QUARTER_MB);
    assert!(!analysis.has_error);
    let labels: Vec<(&str, usize)> = analysis
        .items
        .iter()
        .map(|item| (item.label.as_str(), item.mod_path.len()))
        .collect();
    let mut expected = Vec::new();
    for level in 0..depth {
        expected.push(("m", level));
        expected.push(("a", level + 1));
    }
    for level in (0..depth).rev() {
        expected.push(("z", level + 1));
    }
    assert_eq!(labels, expected);
}
