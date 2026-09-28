//! Spike group 2 (docs/features/phase-0-spikes.md, "Rules and edge cases":
//! broken or hostile input yields a category or `cannot_verify`, never a
//! panic and never a hang): the cost of `ron::analyze` stays linear in the
//! input on broken RON, and every path a marker resolves to stays bounded.
//!
//! The contract under test (`ron` module and structure-walk docs): a field
//! name or a map key keeps at most `ron::MAX_SEGMENT_BYTES` source bytes, cut
//! at a character boundary, plus `ron::TRUNCATION_MARK`; the key's extent is
//! looked for no further; a path is rendered only when a marker is pending;
//! a marker's line is a lookup, not a scan from the start of the file.
//!
//! Every input is generated here, never committed. Timings are compared
//! against a clean input of the same size analysed in the same process, with
//! a generous factor: a debug build on a busy laptop must stay green, a
//! quadratic walk (seconds to minutes at these sizes) must not.
//!
//! The non-ASCII characters are Unicode escapes (ADR-0024).

use std::thread;
use std::time::{Duration, Instant};

use specengine_code::ron::lexer::{self, LexErrorKind, TokenKind};
use specengine_code::ron::{
    self, Anchor, MAX_DEPTH, MAX_SEGMENT_BYTES, RonAnalysis, TRUNCATION_MARK,
};

/// Stack of the analysis thread: the size a caller could reasonably give a worker.
const TWO_MB: usize = 2 * 1024 * 1024;

/// Entries of the generated tuple-key maps.
const ENTRIES: usize = 10_000;

/// Runs `ron::analyze` on a thread with a 2 MB stack, timed.
fn timed(source: &str) -> (RonAnalysis, Duration) {
    let source = source.to_owned();
    thread::Builder::new()
        .name("ron-cost".to_owned())
        .stack_size(TWO_MB)
        .spawn(move || {
            let start = Instant::now();
            let analysis = ron::analyze(&source);
            (analysis, start.elapsed())
        })
        .expect("analysis thread spawns")
        .join()
        .expect("ron::analyze must not panic or overflow")
}

/// The fastest of three runs: the analysis and its time.
fn best_of_three(source: &str) -> (RonAnalysis, Duration) {
    let (analysis, mut best) = timed(source);
    for _ in 0..2 {
        let (again, elapsed) = timed(source);
        assert_eq!(again, analysis, "analysis must be deterministic");
        best = best.min(elapsed);
    }
    (analysis, best)
}

/// `broken` costs the same order as `clean`: at most `factor` times it plus
/// a fixed allowance for scheduler noise.
fn assert_same_order(what: &str, broken: Duration, clean: Duration, factor: u32) {
    let bound = clean * factor + Duration::from_millis(250);
    eprintln!("{what}: broken {broken:?}, clean {clean:?}, bound {bound:?}");
    assert!(
        broken <= bound,
        "{what}: {broken:?} on broken input against {clean:?} on clean input \
         (bound {bound:?}) — the walk is not linear"
    );
}

/// `{ (0, 0): 0, (1, 1): 1, … }`, one entry per line, each preceded by
/// `prefix(i)`; with `broken`, the first key misses its `)`.
fn tuple_key_map(broken: bool, prefix: impl Fn(usize) -> String) -> String {
    let mut source = String::from("{\n");
    for i in 0..ENTRIES {
        source.push_str(&prefix(i));
        if i == 0 && broken {
            source.push_str(&format!("({i}, {i}: {i},\n"));
        } else {
            source.push_str(&format!("({i}, {i}): {i},\n"));
        }
    }
    source.push_str("}\n");
    source
}

fn marker_line(i: usize) -> String {
    format!("// @implements E{i}@1\n")
}

/// The one path of a single-marker analysis.
fn only_path(analysis: &RonAnalysis) -> String {
    assert_eq!(analysis.markers.len(), 1, "{:?}", analysis.markers);
    match &analysis.markers[0].anchor {
        Anchor::Path { path, .. } => path.clone(),
        other => panic!("expected a path, got {other:?}"),
    }
}

/// `n` bytes of `c` (ASCII).
fn ascii(c: char, n: usize) -> String {
    std::iter::repeat_n(c, n).collect()
}

fn truncated(kept: &str) -> String {
    format!("{kept}{TRUNCATION_MARK}")
}

// ------------------------------------------------- broken tuple-key map (B)

#[test]
fn broken_tuple_key_map_costs_the_order_of_the_clean_map() {
    let clean = tuple_key_map(false, |_| String::new());
    let broken = tuple_key_map(true, |_| String::new());
    let (clean_analysis, clean_time) = best_of_three(&clean);
    let (broken_analysis, broken_time) = best_of_three(&broken);
    assert!(
        !clean_analysis.has_error,
        "{:?}",
        clean_analysis.error_categories
    );
    assert_eq!(
        broken_analysis.error_categories,
        [
            "expected_colon",
            "expected_separator",
            "expected_value",
            "unbalanced_delimiter"
        ]
    );
    assert_same_order("tuple-key map", broken_time, clean_time, 10);
}

#[test]
fn broken_tuple_key_map_with_a_marker_per_entry_keeps_every_path_bounded() {
    let clean = tuple_key_map(false, marker_line);
    let broken = tuple_key_map(true, marker_line);
    let (clean_analysis, clean_time) = best_of_three(&clean);
    let (broken_analysis, broken_time) = best_of_three(&broken);
    assert!(
        !clean_analysis.has_error,
        "{:?}",
        clean_analysis.error_categories
    );
    assert_eq!(clean_analysis.markers.len(), ENTRIES);
    assert_eq!(
        broken_analysis.error_categories,
        [
            "expected_colon",
            "expected_separator",
            "expected_value",
            "unbalanced_delimiter"
        ]
    );
    assert_eq!(broken_analysis.markers.len(), ENTRIES);

    // `root{<key>}.<index>`: the key runs to the end of input (its `)` is
    // missing), so it is cut to its first MAX_SEGMENT_BYTES bytes.
    let key_start = broken.find("(0, 0:").unwrap();
    let kept = broken[key_start..key_start + MAX_SEGMENT_BYTES]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let prefix = format!("root{{{}}}", truncated(&kept));
    let bound = "root".len()
        + 2
        + MAX_SEGMENT_BYTES
        + TRUNCATION_MARK.len_utf8()
        + ".".len()
        + ENTRIES.to_string().len();
    let longest = broken_analysis
        .markers
        .iter()
        .map(|m| m.anchor.as_str().len())
        .max()
        .unwrap();
    eprintln!("longest path {longest} bytes, bound {bound}");
    for (i, marker) in broken_analysis.markers.iter().enumerate() {
        let path = marker.anchor.as_str();
        assert!(
            path.len() <= bound,
            "marker {i}: path of {} bytes exceeds {bound}: {}…",
            path.len(),
            &path[..path.len().min(200)]
        );
        // The first marker precedes the key; the rest precede an element of
        // the key's tuple, from its third element on.
        let want = if i == 0 {
            prefix.clone()
        } else {
            format!("{prefix}.{}", i + 1)
        };
        assert_eq!(path, want, "marker {i}");
    }
    assert_same_order(
        "tuple-key map, marker per entry",
        broken_time,
        clean_time,
        10,
    );
}

// ------------------------------------------ deep nesting without markers (B)

/// `depth` nested maps, each keyed by a string of `key_bytes` bytes, around a
/// list of `elements` numbers.
fn deep_maps_around_a_list(depth: usize, key_bytes: usize, elements: usize) -> String {
    let key = format!("\"{}\"", ascii('k', key_bytes - 2));
    let mut source = String::new();
    for _ in 0..depth {
        source.push('{');
        source.push_str(&key);
        source.push(':');
    }
    source.push('[');
    for _ in 0..elements {
        source.push_str("1,");
    }
    source.push(']');
    source.push_str(&"}".repeat(depth));
    source.push('\n');
    source
}

#[test]
fn deep_file_without_markers_renders_no_path() {
    // 500 levels of 128-byte keys around 60 000 elements: rendering a path
    // for every element would copy 500 segments each time.
    const ELEMENTS: usize = 60_000;
    let depth = MAX_DEPTH - 12;
    let deep = deep_maps_around_a_list(depth, MAX_SEGMENT_BYTES, ELEMENTS);
    // The same bytes and tokens at depth 1: the keys become list elements.
    let flat = format!(
        "[{}{}]\n",
        format!("\"{}\",", ascii('k', MAX_SEGMENT_BYTES - 2)).repeat(depth),
        "1,".repeat(ELEMENTS)
    );
    let (deep_analysis, deep_time) = best_of_three(&deep);
    let (flat_analysis, flat_time) = best_of_three(&flat);
    assert!(
        !deep_analysis.has_error,
        "{:?}",
        deep_analysis.error_categories
    );
    assert!(
        !flat_analysis.has_error,
        "{:?}",
        flat_analysis.error_categories
    );
    assert_same_order("deep maps, no marker", deep_time, flat_time, 10);
}

// ------------------------------------------------ unbalanced openers (B)

#[test]
fn hundred_thousand_unbalanced_braces_finish_fast_and_report_both_categories() {
    let n = 100_000;
    let braces = "{".repeat(n);
    let clean = format!("[{}]\n", "1,".repeat(n / 2));
    let (analysis, time) = best_of_three(&braces);
    let (_, clean_time) = best_of_three(&clean);
    for category in ["nesting_too_deep", "unbalanced_delimiter"] {
        assert!(
            analysis.error_categories.contains(&category),
            "{category} missing from {:?}",
            analysis.error_categories
        );
    }
    eprintln!("categories: {:?}", analysis.error_categories);
    assert_same_order("100 000 unbalanced braces", time, clean_time, 10);
}

#[test]
fn marker_a_few_levels_into_unbalanced_braces_gets_a_bounded_path() {
    let depth = 3;
    let source = format!(
        "{}\n// @implements DEEP@1\n{}",
        "{".repeat(depth),
        "{".repeat(100_000)
    );
    let (analysis, _) = timed(&source);
    let path = only_path(&analysis);
    let bound = 5 + depth * (MAX_SEGMENT_BYTES + 5);
    eprintln!("path {} bytes, bound {bound}", path.len());
    assert!(
        path.len() <= bound,
        "path of {} bytes exceeds {bound}: {}…",
        path.len(),
        &path[..path.len().min(300)]
    );
    assert!(
        matches!(analysis.markers[0].anchor, Anchor::Path { depth: d, .. } if d == depth),
        "{:?}",
        analysis.markers[0].anchor
    );
    // Each key runs to the end of input: cut, and marked as cut.
    assert_eq!(
        path.matches(TRUNCATION_MARK).count(),
        depth,
        "one truncation mark per key: {path}"
    );
}

// ------------------------------------------------------------ segment cap

/// `Config(<field>: 1)` with a marker before the field: the field's path.
fn field_path(field: &str) -> String {
    only_path(&ron::analyze(&format!(
        "Config(\n    // @implements F@1\n    {field}: 1,\n)\n"
    )))
}

/// `{<key>: 1}` with a marker before the key: the key's path.
fn key_path(key: &str) -> String {
    only_path(&ron::analyze(&format!(
        "{{\n    // @implements K@1\n    {key}: 1,\n}}\n"
    )))
}

#[test]
fn field_of_exactly_max_segment_bytes_is_kept_whole() {
    let field = ascii('a', MAX_SEGMENT_BYTES);
    assert_eq!(field_path(&field), format!("root.{field}"));
}

#[test]
fn key_of_exactly_max_segment_bytes_is_kept_whole() {
    let key = format!("\"{}\"", ascii('k', MAX_SEGMENT_BYTES - 2));
    assert_eq!(key.len(), MAX_SEGMENT_BYTES);
    assert_eq!(key_path(&key), format!("root{{{key}}}"));
    // A tuple key with whitespace inside: collapsed, not cut.
    let tuple = format!("({},  1)", ascii('9', MAX_SEGMENT_BYTES - 6));
    assert_eq!(tuple.len(), MAX_SEGMENT_BYTES);
    assert_eq!(
        key_path(&tuple),
        format!("root{{({}, 1)}}", ascii('9', MAX_SEGMENT_BYTES - 6))
    );
}

#[test]
fn field_one_byte_over_is_cut_to_max_segment_bytes_plus_the_mark() {
    let field = ascii('a', MAX_SEGMENT_BYTES + 1);
    assert_eq!(
        field_path(&field),
        format!("root.{}", truncated(&field[..MAX_SEGMENT_BYTES]))
    );
    let long = ascii('b', 10_000);
    assert_eq!(
        field_path(&long),
        format!("root.{}", truncated(&long[..MAX_SEGMENT_BYTES]))
    );
}

#[test]
fn long_key_is_cut_to_max_segment_bytes_plus_the_mark() {
    let key = format!("\"{}\"", ascii('k', 500));
    assert_eq!(
        key_path(&key),
        format!("root{{{}}}", truncated(&key[..MAX_SEGMENT_BYTES]))
    );
    // A container key past the cap: its end is not looked for.
    let tuple = format!("({})", "1, ".repeat(100));
    let kept = tuple[..MAX_SEGMENT_BYTES].trim_end().to_owned();
    assert_eq!(key_path(&tuple), format!("root{{{}}}", truncated(&kept)));
}

#[test]
fn a_character_straddling_the_cap_is_not_split() {
    // `\u{e9}` is two bytes: bytes 127 and 128 of the segment.
    let field = format!(
        "{}\u{e9}{}",
        ascii('a', MAX_SEGMENT_BYTES - 1),
        ascii('a', 10)
    );
    assert!(!field.is_char_boundary(MAX_SEGMENT_BYTES));
    assert_eq!(
        field_path(&field),
        format!("root.{}", truncated(&ascii('a', MAX_SEGMENT_BYTES - 1)))
    );
    // A four-byte character over the cut, in a string key.
    let key = format!("\"{}\u{1F600}tail\"", ascii('k', MAX_SEGMENT_BYTES - 3));
    assert!(!key.is_char_boundary(MAX_SEGMENT_BYTES));
    assert_eq!(
        key_path(&key),
        format!("root{{{}}}", truncated(&key[..MAX_SEGMENT_BYTES - 2]))
    );
    // A two-byte character ending exactly at the cut is kept.
    let edge = format!(
        "{}\u{e9}{}",
        ascii('a', MAX_SEGMENT_BYTES - 2),
        ascii('a', 5)
    );
    assert!(edge.is_char_boundary(MAX_SEGMENT_BYTES));
    assert_eq!(
        field_path(&edge),
        format!("root.{}", truncated(&edge[..MAX_SEGMENT_BYTES]))
    );
}

#[test]
fn unbalanced_key_shorter_than_the_cap_ending_at_eof_is_kept_whole() {
    let source = "{\n// @implements K@1\n(1,  2";
    let analysis = ron::analyze(source);
    assert!(analysis.has_error);
    assert!(
        analysis.error_categories.contains(&"unbalanced_delimiter"),
        "{:?}",
        analysis.error_categories
    );
    assert_eq!(only_path(&analysis), "root{(1, 2}");
    // The same with a trailing newline: whitespace is collapsed and trimmed.
    let analysis = ron::analyze(&format!("{source}\n"));
    assert_eq!(only_path(&analysis), "root{(1, 2}");
    // At the cap exactly, still whole.
    let inner = ascii('7', MAX_SEGMENT_BYTES - 1);
    let analysis = ron::analyze(&format!("{{\n// @implements K@1\n({inner}"));
    assert_eq!(only_path(&analysis), format!("root{{({inner}}}"));
    // One byte over, cut.
    let inner = ascii('7', MAX_SEGMENT_BYTES);
    let analysis = ron::analyze(&format!("{{\n// @implements K@1\n({inner}"));
    assert_eq!(
        only_path(&analysis),
        format!(
            "root{{{}}}",
            truncated(&format!("({}", &inner[..MAX_SEGMENT_BYTES - 1]))
        )
    );
}

// ------------------------------------------------------------ line numbers

/// 1-based line of `offset`, by a scan (the reference).
fn naive_line(source: &str, offset: usize) -> usize {
    source[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

#[test]
fn marker_lines_match_a_naive_count_in_a_large_file() {
    let source = tuple_key_map(false, marker_line);
    let analysis = ron::analyze(&source);
    assert_eq!(analysis.markers.len(), ENTRIES);
    // One running count over the ascending comment offsets (a scan from the
    // start per marker would make this reference itself quadratic).
    let (mut line, mut counted) = (1, 0);
    for marker in &analysis.markers {
        let start = marker.comment.start;
        assert!(start >= counted, "markers must be in source order");
        line += source[counted..start]
            .bytes()
            .filter(|b| *b == b'\n')
            .count();
        counted = start;
        assert_eq!(marker.line, line, "{marker:?}");
    }
    assert_eq!(analysis.markers[0].line, 2);
    assert_eq!(analysis.markers[ENTRIES - 1].line, 2 * ENTRIES);
}

#[test]
fn marker_on_the_first_line_is_line_one() {
    let analysis = ron::analyze("// @implements FIRST@1\nConfig(speed: 1)\n");
    assert_eq!(analysis.markers[0].line, 1);
    assert_eq!(analysis.markers[0].anchor.as_str(), "root");
    let analysis = ron::analyze("/* @implements FIRST@1 */ 1");
    assert_eq!(analysis.markers[0].line, 1);
}

#[test]
fn marker_on_the_last_line_without_a_newline() {
    let body = tuple_key_map(false, |_| String::new());
    let source = format!("{body}// @implements LAST@1");
    let analysis = ron::analyze(&source);
    let last = analysis.markers.last().expect("the marker is found");
    assert_eq!(last.marker.id, "LAST");
    assert_eq!(last.line, source.lines().count());
    assert_eq!(last.line, naive_line(&source, last.comment.start));
    assert_eq!(last.anchor, Anchor::Unanchored);
}

#[test]
fn marker_after_the_trailing_newline_of_a_large_file() {
    let body = tuple_key_map(false, marker_line);
    assert!(body.ends_with('\n'));
    let source = format!("{body}// @implements AFTER@1\n");
    let analysis = ron::analyze(&source);
    let last = analysis.markers.last().expect("the marker is found");
    assert_eq!(last.marker.id, "AFTER");
    let newlines_before = body.matches('\n').count();
    assert_eq!(last.line, newlines_before + 1);
    assert_eq!(last.line, naive_line(&source, last.comment.start));
    assert_eq!(last.anchor, Anchor::Unanchored);
}

// ------------------------------------------------------------------ lexer

#[test]
fn fifty_thousand_unclosed_unicode_escapes_lex_in_linear_time() {
    let n = 50_000;
    let broken = "'\\u{".repeat(n);
    // The clean reference: the same bytes as closed char literals.
    let clean = "'\\u{41}'".repeat(n / 2);
    let lex_best = |source: &str| {
        (0..3)
            .map(|_| {
                let start = Instant::now();
                let out = lexer::lex(source);
                (out, start.elapsed())
            })
            .min_by_key(|(_, t)| *t)
            .unwrap()
    };
    let ((tokens, errors), broken_time) = lex_best(&broken);
    let ((clean_tokens, clean_errors), clean_time) = lex_best(&clean);
    assert!(clean_errors.is_empty(), "{clean_errors:?}");
    assert_eq!(clean_tokens.len(), n / 2);
    assert_eq!(
        errors
            .iter()
            .filter(|e| e.kind == LexErrorKind::UnterminatedChar)
            .count(),
        n,
        "one unterminated_char per quote"
    );
    assert!(!tokens.is_empty());
    assert_same_order("lex 50 000 x '\\u{", broken_time, clean_time, 10);
    // The whole analysis stays linear too.
    let (analysis, analysis_time) = timed(&broken);
    let (_, clean_analysis_time) = timed(&clean);
    assert!(analysis.error_categories.contains(&"unterminated_char"));
    assert_same_order(
        "analyze 50 000 x '\\u{",
        analysis_time,
        clean_analysis_time,
        10,
    );
}

#[test]
fn closed_unicode_escape_is_one_char_token() {
    let source = "'\\u{1F600}'";
    let (tokens, errors) = lexer::lex(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(tokens.len(), 1, "{tokens:?}");
    assert_eq!(tokens[0].kind, TokenKind::Char);
    assert_eq!(tokens[0].range, 0..source.len());
    let analysis = ron::analyze(&format!("// @implements C@1\n{source}\n"));
    assert!(!analysis.has_error, "{:?}", analysis.error_categories);
    assert_eq!(analysis.markers[0].anchor.as_str(), "root");
}

#[test]
fn unclosed_unicode_escape_at_eof_is_unterminated_char() {
    let source = "'\\u{12";
    let (_, errors) = lexer::lex(source);
    let first = errors.first().expect("an error");
    assert_eq!(first.kind, LexErrorKind::UnterminatedChar);
    assert_eq!(first.offset, 0);
    let analysis = ron::analyze(source);
    assert!(analysis.has_error);
    assert!(
        analysis.error_categories.contains(&"unterminated_char"),
        "{:?}",
        analysis.error_categories
    );
    assert_eq!(analysis.rejected[0].category, "unterminated_char");
    assert_eq!(analysis.rejected[0].offset, 0);
}
