//! RON depth cap (`docs/canon/code-identity.md` "RON binding";
//! `crates/specengine-code/README.md`, "Rules": broken or hostile input
//! yields a category or `cannot_verify`, never a panic): the lexer path's
//! structure walk is capped at `ron::MAX_DEPTH` open containers. A container
//! past the cap is a `nesting_too_deep` rejection,
//! skipped through its matching closer; markers inside it are
//! `cannot_verify`; the walk resumes after the closer.
//!
//! Deep inputs are generated here, never committed. Every walk runs on a
//! thread with an explicit stack size, so a regression to unbounded
//! recursion is a stack overflow of this test binary, not a green run on a
//! lucky default stack.

use std::thread;

use specengine_code::ron::{self, Anchor, MAX_DEPTH, RonAnalysis};

/// Stack of the analysis thread: the size a caller could reasonably give a worker.
const TWO_MB: usize = 2 * 1024 * 1024;

const ROOT_MARKER: &str = "// @implements ROOT@1\n";
const BOTTOM_MARKER: &str = "\n// @implements BOTTOM@1\n";

/// Runs `ron::analyze` on a thread with `stack` bytes of stack.
fn analyze_on_stack(source: String, stack: usize) -> RonAnalysis {
    thread::Builder::new()
        .name("ron-depth".to_owned())
        .stack_size(stack)
        .spawn(move || ron::analyze(&source))
        .expect("analysis thread spawns")
        .join()
        .expect("ron::analyze must not panic or overflow on deep input")
}

/// `(id, anchor)` of every marker, in source order.
fn anchors(analysis: &RonAnalysis) -> Vec<(String, String)> {
    analysis
        .markers
        .iter()
        .map(|m| (m.marker.id.clone(), m.anchor.as_str().to_owned()))
        .collect()
}

fn offsets_of(analysis: &RonAnalysis, category: &str) -> Vec<usize> {
    analysis
        .rejected
        .iter()
        .filter(|r| r.category == category)
        .map(|r| r.offset)
        .collect()
}

/// Byte offset of the `n`-th (1-based) occurrence of `needle` in `source`.
fn nth_offset(source: &str, needle: char, n: usize) -> usize {
    source
        .match_indices(needle)
        .nth(n - 1)
        .map(|(at, _)| at)
        .expect("the occurrence exists")
}

/// A marker before the root, `depth` nested lists, a marker and a value at the bottom.
fn nested_lists(depth: usize) -> String {
    format!(
        "{ROOT_MARKER}{}{BOTTOM_MARKER}1\n{}\n",
        "[".repeat(depth),
        "]".repeat(depth)
    )
}

// ------------------------------------------------------------- deep lists

fn assert_deep_lists(depth: usize) {
    let source = nested_lists(depth);
    let analysis = analyze_on_stack(source.clone(), TWO_MB);
    assert!(analysis.has_error, "a skipped container is a rejection");
    assert_eq!(
        analysis.error_categories,
        ["nesting_too_deep"],
        "balanced deep lists reject nothing but the depth"
    );
    assert_eq!(
        offsets_of(&analysis, "nesting_too_deep"),
        [ROOT_MARKER.len() + MAX_DEPTH],
        "one rejection, at the first opener past the cap"
    );
    assert_eq!(
        anchors(&analysis),
        [
            ("ROOT".to_owned(), "root".to_owned()),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
        ]
    );
    assert_eq!(
        analysis.markers[0].anchor,
        Anchor::Path {
            path: "root".to_owned(),
            depth: 0,
        }
    );
    assert_eq!(analysis.comments.len(), 2);
}

#[test]
fn twenty_thousand_nested_lists_are_capped_on_a_two_megabyte_stack() {
    assert_deep_lists(20_000);
}

#[test]
fn hundred_thousand_nested_lists_are_capped_on_a_two_megabyte_stack() {
    assert_deep_lists(100_000);
}

#[test]
fn hundred_thousand_unclosed_lists_are_capped_and_unbalanced() {
    let source = format!("{ROOT_MARKER}{}{BOTTOM_MARKER}", "[".repeat(100_000));
    let analysis = analyze_on_stack(source.clone(), TWO_MB);
    assert_eq!(
        analysis.error_categories,
        ["nesting_too_deep", "unbalanced_delimiter"]
    );
    assert_eq!(
        offsets_of(&analysis, "nesting_too_deep"),
        [ROOT_MARKER.len() + MAX_DEPTH]
    );
    assert!(
        offsets_of(&analysis, "unbalanced_delimiter")
            .iter()
            .all(|at| *at == source.len()),
        "an unclosed container is unbalanced at the end of input"
    );
    assert_eq!(
        anchors(&analysis),
        [
            ("ROOT".to_owned(), "root".to_owned()),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
        ],
        "a comment after the last token still lies inside the skipped container"
    );
}

#[test]
fn deep_analysis_is_deterministic() {
    let source = nested_lists(20_000);
    let first = analyze_on_stack(source.clone(), TWO_MB);
    let second = analyze_on_stack(source, TWO_MB);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------- boundary

#[test]
fn exactly_max_depth_lists_are_walked_whole() {
    let source = nested_lists(MAX_DEPTH);
    let analysis = analyze_on_stack(source, TWO_MB);
    assert!(
        !analysis.has_error,
        "MAX_DEPTH containers are within the cap: {:?}",
        analysis.rejected
    );
    assert!(analysis.error_categories.is_empty());
    assert_eq!(analysis.markers.len(), 2);
    assert_eq!(analysis.markers[0].anchor.as_str(), "root");
    assert_eq!(
        analysis.markers[1].anchor,
        Anchor::Path {
            path: format!("root{}", "[0]".repeat(MAX_DEPTH)),
            depth: MAX_DEPTH,
        },
        "the value inside the innermost allowed list keeps its full path"
    );
}

#[test]
fn one_past_max_depth_is_exactly_one_rejection_at_the_extra_opener() {
    let before_extra = "\n// @implements EDGE@1\n";
    let source = format!(
        "{ROOT_MARKER}{}{before_extra}[{BOTTOM_MARKER}1\n]{}\n",
        "[".repeat(MAX_DEPTH),
        "]".repeat(MAX_DEPTH)
    );
    let extra = nth_offset(&source, '[', MAX_DEPTH + 1);
    let analysis = analyze_on_stack(source, TWO_MB);
    assert_eq!(analysis.error_categories, ["nesting_too_deep"]);
    assert_eq!(
        analysis.rejected.len(),
        1,
        "exactly one rejection: {:?}",
        analysis.rejected
    );
    assert_eq!(analysis.rejected[0].offset, extra);
    assert_eq!(
        anchors(&analysis),
        [
            ("ROOT".to_owned(), "root".to_owned()),
            (
                "EDGE".to_owned(),
                format!("root{}", "[0]".repeat(MAX_DEPTH))
            ),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
        ],
        "a marker before the skipped opener still names its element; one inside cannot be verified"
    );
}

// ---------------------------------------------------------------- recovery

#[test]
fn walk_resumes_after_the_skipped_container() {
    let source = format!(
        "Config(\n    // @implements DEEP@1\n    deep: {}{BOTTOM_MARKER}1\n{},\n    // @implements AFTER@1\n    after: 1,\n)\n",
        "[".repeat(600),
        "]".repeat(600)
    );
    // The struct is container 1, so the cap is hit at the MAX_DEPTH-th `[`.
    let extra = nth_offset(&source, '[', MAX_DEPTH);
    let analysis = analyze_on_stack(source, TWO_MB);
    assert_eq!(analysis.error_categories, ["nesting_too_deep"]);
    assert_eq!(offsets_of(&analysis, "nesting_too_deep"), [extra]);
    assert_eq!(
        anchors(&analysis),
        [
            ("DEEP".to_owned(), "root.deep".to_owned()),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
            ("AFTER".to_owned(), "root.after".to_owned()),
        ]
    );
}

// ---------------------------------------------------- other container kinds

#[test]
fn unclosed_maps_are_capped_with_markers_inside_unverifiable() {
    // Each `{` opens a map whose first key is the next `{`: map keys nest.
    let depth = 5_000;
    let source = format!(
        "{ROOT_MARKER}{}\n// @implements MIDDLE@1\n{}{BOTTOM_MARKER}",
        "{".repeat(depth / 2),
        "{".repeat(depth / 2)
    );
    let analysis = analyze_on_stack(source.clone(), TWO_MB);
    assert!(
        analysis.error_categories.contains(&"nesting_too_deep"),
        "{:?}",
        analysis.error_categories
    );
    assert!(
        analysis.error_categories.contains(&"unbalanced_delimiter"),
        "{:?}",
        analysis.error_categories
    );
    assert_eq!(
        offsets_of(&analysis, "nesting_too_deep"),
        [ROOT_MARKER.len() + MAX_DEPTH]
    );
    assert_eq!(
        anchors(&analysis),
        [
            ("ROOT".to_owned(), "root".to_owned()),
            ("MIDDLE".to_owned(), "cannot_verify".to_owned()),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
        ]
    );
}

#[test]
fn nested_map_keys_are_capped_and_the_walk_stays_clean_otherwise() {
    // K(0) = `1`, K(d) = `{ K(d-1): 1 }`: every key is itself a map.
    let depth = 1_000;
    let source = format!(
        "{ROOT_MARKER}{}{BOTTOM_MARKER}1{}\n",
        "{".repeat(depth),
        ": 1}".repeat(depth)
    );
    let extra = nth_offset(&source, '{', MAX_DEPTH + 1);
    let analysis = analyze_on_stack(source, TWO_MB);
    assert_eq!(analysis.error_categories, ["nesting_too_deep"]);
    assert_eq!(offsets_of(&analysis, "nesting_too_deep"), [extra]);
    assert_eq!(
        anchors(&analysis),
        [
            ("ROOT".to_owned(), "root".to_owned()),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
        ]
    );
}

#[test]
fn nested_tuple_structs_are_capped() {
    let depth = 20_000;
    let source = format!(
        "{ROOT_MARKER}{}{BOTTOM_MARKER}1{}\n",
        "A(".repeat(depth),
        ")".repeat(depth)
    );
    let extra = nth_offset(&source, '(', MAX_DEPTH + 1);
    let analysis = analyze_on_stack(source, TWO_MB);
    assert_eq!(analysis.error_categories, ["nesting_too_deep"]);
    assert_eq!(offsets_of(&analysis, "nesting_too_deep"), [extra]);
    assert_eq!(
        anchors(&analysis),
        [
            ("ROOT".to_owned(), "root".to_owned()),
            ("BOTTOM".to_owned(), "cannot_verify".to_owned()),
        ]
    );
}

#[test]
fn tuple_structs_at_max_depth_keep_their_path() {
    let source = format!(
        "{ROOT_MARKER}{}{BOTTOM_MARKER}1{}\n",
        "A(".repeat(MAX_DEPTH),
        ")".repeat(MAX_DEPTH)
    );
    let analysis = analyze_on_stack(source, TWO_MB);
    assert!(!analysis.has_error, "{:?}", analysis.rejected);
    assert_eq!(
        analysis.markers[1].anchor,
        Anchor::Path {
            path: format!("root{}", ".0".repeat(MAX_DEPTH)),
            depth: MAX_DEPTH,
        }
    );
}

// ------------------------------------------------------- attribute loops

#[test]
fn hundred_thousand_misplaced_attributes_do_not_overflow() {
    let count = 100_000;
    let source = format!("[{}1]", "#![enable(x)]".repeat(count));
    let analysis = analyze_on_stack(source, TWO_MB);
    assert_eq!(analysis.error_categories, ["misplaced_attribute"]);
    assert_eq!(
        analysis.rejected.len(),
        count,
        "one rejection per misplaced attribute"
    );
}

#[test]
fn hundred_thousand_leading_attributes_stay_clean() {
    let source = format!("{}{ROOT_MARKER}[1]", "#![enable(x)]\n".repeat(100_000));
    let analysis = analyze_on_stack(source, TWO_MB);
    assert!(!analysis.has_error, "{:?}", analysis.error_categories);
    assert_eq!(anchors(&analysis), [("ROOT".to_owned(), "root".to_owned())]);
}
