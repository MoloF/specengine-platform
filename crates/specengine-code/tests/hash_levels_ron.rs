//! docs/features/hash-levels-ron.md (5a-1b): the value hash of an anchored
//! RON value. The rust-developer criteria under the spec's test names, and
//! the test-engineer's TE-2:
//!
//! - RD-1 "RON value": `ron_comment_edit_keeps_the_value_hash`,
//!   `ron_sibling_edit_keeps_the_value_hash`,
//!   `ron_trailing_comma_keeps_the_value_hash`,
//!   `broken_ron_file_value_cannot_verify`,
//!   `ron_structure_error_refuses_every_value`,
//!   `ron_value_hash_equal_under_lf_and_crlf`, `ron_value_stream_is_pinned`;
//! - RD-2 "Entry point": `ron_value_hashes_linear_in_comments`,
//!   `unmarked_path_comments_are_keyed_as_comment_anchors`;
//! - TE-2: two runs, reversed file order, no offset, line or path in a
//!   stream;
//! - the spec's own example, the depth cap, and the readings of the test
//!   engineer's phase A report (the `L` tags).
//!
//! Every digest is checked against the recipe restated here:
//! BLAKE3(`level_header()` ‖ `put("ron")` ‖ `put(text)` per token), with the
//! header and `put` built by `levels_common` (5a-1's independent oracle) and
//! the token texts listed by hand, never read from the crate's lexer. TE-1
//! (the `.ron` fixture and its `expected.json`) is
//! `crates/specengine-eval/tests/hash_levels_ron_fixture.rs`.
//!
//! PHASE-B: `ron::value_hashes` and `ron::RonValues` by the spec's names
//! ("Entry point": `Hashed(Vec<(Range<usize>, Digest)>)` or
//! `CannotVerify(&'static str)`); `values` is the only function reading that
//! shape.

mod levels_common;

use std::ops::Range;
use std::thread;
use std::time::{Duration, Instant};

use levels_common::{crlf, digest_by_hand, fixture_root, put, read};
use specengine_code::Digest;
use specengine_code::ron::lexer::LexErrorKind;
use specengine_code::ron::{self, Anchor, MAX_DEPTH, RonValues};

/// Stack of a hashing thread: the size a caller could reasonably give a worker.
const TWO_MB: usize = 2 * 1024 * 1024;

// ------------------------------------------------------------ the entry point

/// What `ron::value_hashes` gives, copied out of `RonValues`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Values {
    Hashed(Vec<(Range<usize>, Digest)>),
    Refused(&'static str),
}

/// PHASE-B: the one place that reads the shape of `RonValues`.
fn values(source: &str) -> Values {
    match ron::value_hashes(source) {
        RonValues::Hashed(entries) => Values::Hashed(entries),
        RonValues::CannotVerify(detail) => Values::Refused(detail),
    }
}

/// The entries of a file that must hash.
fn hashed(source: &str, what: &str) -> Vec<(Range<usize>, Digest)> {
    match values(source) {
        Values::Hashed(entries) => entries,
        Values::Refused(detail) => panic!("{what}: refused ({detail}), expected hashed"),
    }
}

fn keys(entries: &[(Range<usize>, Digest)]) -> Vec<Range<usize>> {
    entries.iter().map(|(range, _)| range.clone()).collect()
}

/// The byte range of the one comment whose whole text is `text` (a line
/// comment without its line end; a CR before that line end is not text).
fn comment(source: &str, text: &str, what: &str) -> Range<usize> {
    let hits: Vec<Range<usize>> = ron::analyze(source)
        .comments
        .into_iter()
        .filter(|range| source[range.clone()].trim_end_matches('\r') == text)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{what}: exactly one comment {text:?}, found {hits:?}"
    );
    hits[0].clone()
}

/// The value hash keyed by the comment `text`.
fn value_of(source: &str, text: &str, what: &str) -> Digest {
    let key = comment(source, text, what);
    let entries = hashed(source, what);
    let hits: Vec<Digest> = entries
        .iter()
        .filter(|(range, _)| *range == key)
        .map(|(_, digest)| *digest)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{what}: one entry keyed by {text:?} at {key:?}; keys {:?}",
        keys(&entries)
    );
    hits[0]
}

/// The comments `comment_anchors` binds to a path, in source order.
fn path_comments(source: &str) -> Vec<Range<usize>> {
    ron::comment_anchors(source)
        .into_iter()
        .filter(|c| matches!(c.anchor, Anchor::Path { .. }))
        .map(|c| c.comment)
        .collect()
}

// ------------------------------------------------- the recipe, independently

/// "RON value" restated: BLAKE3(`level_header()` ‖ `put("ron")` ‖ `put(t)`
/// for each token text `t`), header and `put` built by hand.
fn oracle(tokens: &[&str]) -> [u8; 32] {
    let mut stream = Vec::new();
    for token in tokens {
        put(&mut stream, token.as_bytes());
    }
    digest_by_hand("ron", &stream)
}

/// The token texts of a canonical rendering whose tokens hold no space.
fn spaced(canonical: &str) -> Vec<&str> {
    canonical.split(' ').collect()
}

fn assert_oracle(got: Digest, tokens: &[&str], what: &str) {
    assert_eq!(
        got.as_bytes(),
        &oracle(tokens),
        "{what}: the value hash is not BLAKE3(level_header ‖ put(\"ron\") ‖ put(t)) over {tokens:?}"
    );
}

// ------------------------------------------------------------ the spec example

/// "RON value", Example, verbatim: `// m` anchors `root.b` and hashes
/// `[ 2 , 3 ]` (`/* x */` and the `,` before `]` dropped); `/* x */`
/// anchors `root.b[1]`, its own entry, the stream of `3`.
#[test]
fn spec_example_hashes_the_documented_streams() {
    let source = "(\n    // m\n    b: [2, /* x */ 3,],\n)\n";
    let entries = hashed(source, "spec example");
    assert_eq!(
        keys(&entries),
        [
            comment(source, "// m", "spec example"),
            comment(source, "/* x */", "spec example"),
        ],
        "one entry per path comment, keyed by the comment's range, in source order"
    );
    assert_oracle(entries[0].1, &["[", "2", ",", "3", "]"], "// m");
    assert_oracle(entries[1].1, &["3"], "/* x */");
}

// ------------------------------------------------------------------- RD-1

#[test]
fn ron_comment_edit_keeps_the_value_hash() {
    const BASE: &str = "Config(\n    // m\n    b: [2, /* x */ 3, // tail\n        4,\n    ],\n)\n";
    let want = ["[", "2", ",", "3", ",", "4", "]"];
    let variants: [(&str, String, &str); 9] = [
        ("base", BASE.to_owned(), "// m"),
        (
            "block comment text",
            BASE.replace("/* x */", "/* another note */"),
            "// m",
        ),
        (
            "block comment removed",
            BASE.replace("/* x */ ", ""),
            "// m",
        ),
        (
            "line comment text",
            BASE.replace("// tail", "// a longer tail @implements X-1@1"),
            "// m",
        ),
        ("line comment removed", BASE.replace(" // tail", ""), "// m"),
        (
            "nested block comment added",
            BASE.replace("4,", "4, /* outer /* inner */ outer */"),
            "// m",
        ),
        (
            "comment between the trailing comma and the closer",
            BASE.replace("4,\n    ],", "4,\n        // last\n    ],"),
            "// m",
        ),
        (
            "block comment over two lines",
            BASE.replace("/* x */", "/* x\n       spans lines */"),
            "// m",
        ),
        (
            "the anchoring comment's own text",
            BASE.replace("// m\n", "// @implements B-1@2 [sig] the note\n"),
            "// @implements B-1@2 [sig] the note",
        ),
    ];
    for (what, source, anchor) in &variants {
        assert!(
            *what == "base" || source != BASE,
            "{what}: the edit applies"
        );
        assert_oracle(value_of(source, anchor, what), &want, what);
    }
    assert_oracle(value_of(BASE, "/* x */", "base"), &["3"], "/* x */");
}

#[test]
fn ron_sibling_edit_keeps_the_value_hash() {
    const BASE: &str = "/* r */ Config(\n    a: 1,\n    // m\n    b: Inner(x: 1, y: [2, 3]),\n    c: \"z\",\n    d: [1, /* e */ (2, 3), 4],\n)\n";
    let want_m = spaced("Inner ( x : 1 , y : [ 2 , 3 ] )");
    let want_e = spaced("( 2 , 3 )");
    assert_oracle(value_of(BASE, "// m", "base"), &want_m, "base: // m");
    assert_oracle(value_of(BASE, "/* e */", "base"), &want_e, "base: /* e */");
    let base_root = value_of(BASE, "/* r */", "base");
    let variants = [
        ("a edited", BASE.replace("a: 1,", "a: 100,")),
        (
            "c edited into a struct",
            BASE.replace("c: \"z\",", "c: Other(z: [1, 2, 3]),"),
        ),
        ("a removed", BASE.replace("    a: 1,\n", "")),
        (
            "a sibling inserted after b",
            BASE.replace("    c: ", "    b2: 7,\n    c: "),
        ),
        (
            "the neighbours of /* e */ edited",
            BASE.replace("[1, /* e */ (2, 3), 4]", "[9, /* e */ (2, 3), 8, 7]"),
        ),
        (
            "c moved before b",
            BASE.replace(
                "    // m\n    b: Inner(x: 1, y: [2, 3]),\n    c: \"z\",\n",
                "    c: \"z\",\n    // m\n    b: Inner(x: 1, y: [2, 3]),\n",
            ),
        ),
    ];
    for (what, source) in &variants {
        assert_ne!(source, BASE, "{what}: the edit applies");
        assert_oracle(value_of(source, "// m", what), &want_m, what);
        assert_oracle(value_of(source, "/* e */", what), &want_e, what);
        assert_ne!(
            value_of(source, "/* r */", what),
            base_root,
            "{what}: the root holds the edit, so its hash moves"
        );
    }
}

#[test]
fn ron_trailing_comma_keeps_the_value_hash() {
    // (with a `,` before a closer, without it, the tokens both hash)
    let cases = [
        ("[2, 3,]", "[2, 3]", "[ 2 , 3 ]"),
        ("(1, 2,)", "(1, 2)", "( 1 , 2 )"),
        ("(a: 1,)", "(a: 1)", "( a : 1 )"),
        ("P(a: 1,)", "P(a: 1)", "P ( a : 1 )"),
        ("Some(3,)", "Some(3)", "Some ( 3 )"),
        ("{\"k\": 1,}", "{\"k\": 1}", "{ \"k\" : 1 }"),
        ("[[1,], (2,),]", "[[1], (2)]", "[ [ 1 ] , ( 2 ) ]"),
        ("[\n    2,\n    3,\n]", "[2, 3]", "[ 2 , 3 ]"),
        // The next non-comment token decides: comments between the `,` and
        // the closer do not keep it.
        ("[2, 3, // c\n]", "[2, 3]", "[ 2 , 3 ]"),
        ("[2, 3, /* c */]", "[2, 3]", "[ 2 , 3 ]"),
        ("[2, 3,\n    /* a */ /* b */\n]", "[2, 3]", "[ 2 , 3 ]"),
        // A `,` followed by a comment, then a value, is kept.
        ("[2, /* c */ 3]", "[2, 3]", "[ 2 , 3 ]"),
    ];
    for (with, without, tokens) in cases {
        let a = format!("/* v */ {with}\n");
        let b = format!("/* v */ {without}\n");
        let digest_a = value_of(&a, "/* v */", with);
        let digest_b = value_of(&b, "/* v */", without);
        assert_eq!(digest_a, digest_b, "{with:?} against {without:?}");
        assert_oracle(digest_a, &spaced(tokens), with);
    }
}

/// The lexer's categories, by the lexer's own names.
fn lexer_categories() -> [&'static str; 5] {
    [
        LexErrorKind::UnterminatedString.as_str(),
        LexErrorKind::UnterminatedBlockComment.as_str(),
        LexErrorKind::UnterminatedChar.as_str(),
        LexErrorKind::UnterminatedAttribute.as_str(),
        LexErrorKind::StrayToken.as_str(),
    ]
}

/// `source` is refused on LF and on its CRLF copy, the detail the first of
/// that text's `error_categories` (Q7); returns the LF categories.
fn assert_refused(what: &str, source: &str) -> Vec<&'static str> {
    let analysis = ron::analyze(source);
    assert!(
        analysis.has_error,
        "{what}: premise: RonAnalysis rejects the file"
    );
    for (mode, text) in [("LF", source.to_owned()), ("CRLF", crlf(source))] {
        let categories = ron::analyze(&text).error_categories;
        assert_eq!(
            values(&text),
            Values::Refused(categories[0]),
            "{what} ({mode}): every value refused, the detail the first of {categories:?}"
        );
    }
    analysis.error_categories
}

#[test]
fn broken_ron_file_value_cannot_verify() {
    let lexer = lexer_categories();
    // Each case: `// m` binds to `root.a`, far from the error.
    let cases = [
        (
            "a block comment never closed",
            "Config(\n    // m\n    a: 1,\n)\n/* never closed\n",
        ),
        (
            "an unterminated string",
            "Config(\n    // m\n    a: 1,\n    b: \"open,\n)\n",
        ),
        (
            "an unterminated char",
            "Config(\n    // m\n    a: 1,\n    b: 'xy,\n)\n",
        ),
        (
            "an unterminated attribute",
            "Config(\n    // m\n    a: 1,\n)\n#![enable(implicit_some)\n",
        ),
        (
            "a stray token",
            "Config(\n    // m\n    a: 1,\n    b: @,\n)\n",
        ),
    ];
    for (what, source) in cases {
        assert_eq!(
            path_comments(source),
            [comment(source, "// m", what)],
            "{what}: premise: `// m` binds to a path"
        );
        let categories = assert_refused(what, source);
        assert!(
            categories.iter().any(|c| lexer.contains(c)),
            "{what}: premise: a lexer error among {categories:?}"
        );
    }
    // A lexer error alone, no structure error beside it, still refuses.
    let (what, alone) = cases[0];
    assert_eq!(
        ron::analyze(alone).error_categories,
        ["unterminated_block_comment"],
        "{what}: premise: the only category"
    );
    assert_eq!(values(alone), Values::Refused("unterminated_block_comment"));
}

/// `[ // m` and `MAX_DEPTH` more openers: the innermost is skipped as
/// `nesting_too_deep`, the comment inside it `cannot_verify`.
fn too_deep() -> String {
    format!(
        "[ // m\n{} /* deep */ {}\n]\n",
        "[".repeat(MAX_DEPTH),
        "]".repeat(MAX_DEPTH)
    )
}

#[test]
fn ron_structure_error_refuses_every_value() {
    let lexer = lexer_categories();
    // (case, source, its categories by hand: a premise, not the subject)
    let cases: [(&str, String, &[&str]); 8] = [
        (
            "a missing separator",
            "Config(\n    // m\n    a: 1\n    b: 2,\n)\n".to_owned(),
            &["expected_separator"],
        ),
        (
            "a missing colon",
            "Config(\n    // m\n    a: 1,\n    b 2,\n)\n".to_owned(),
            &["expected_colon"],
        ),
        (
            "a missing field name",
            "Config(\n    // m\n    a: 1,\n    2,\n)\n".to_owned(),
            &["expected_field"],
        ),
        (
            "a mismatched closer",
            "Config(\n    // m\n    a: 1,\n    b: [2, 3),\n)\n".to_owned(),
            &["expected_separator", "unbalanced_delimiter"],
        ),
        (
            "an unclosed container",
            "Config(\n    // m\n    a: 1,\n".to_owned(),
            &["unbalanced_delimiter"],
        ),
        (
            "trailing content",
            "Config(\n    // m\n    a: 1,\n)\nextra\n".to_owned(),
            &["trailing_content"],
        ),
        (
            "a misplaced attribute",
            "Config(\n    // m\n    a: 1,\n    b: #![enable(x)] 2,\n)\n".to_owned(),
            &["misplaced_attribute"],
        ),
        ("nesting past MAX_DEPTH", too_deep(), &["nesting_too_deep"]),
    ];
    for (what, source, want) in &cases {
        assert_eq!(
            path_comments(source),
            [comment(source, "// m", what)],
            "{what}: premise: `// m` binds to a path"
        );
        let categories = assert_refused(what, source);
        assert!(
            !categories.iter().any(|c| lexer.contains(c)),
            "{what}: premise: no lexer error in {categories:?}"
        );
        assert_eq!(categories, *want, "{what}: premise: the categories by hand");
        assert_eq!(values(source), Values::Refused(want[0]), "{what}");
    }
    // An empty file is a rejection too: refused, not an empty `Hashed`.
    for source in ["", "\n", "// m\n", "/* only a comment */"] {
        assert_eq!(
            ron::analyze(source).error_categories,
            ["empty_file"],
            "{source:?}: premise"
        );
        assert_eq!(values(source), Values::Refused("empty_file"), "{source:?}");
    }
}

#[test]
fn ron_value_hash_equal_under_lf_and_crlf() {
    const SOURCE: &str = r##"Config(
    // text
    text: "line one
line two
",
    // raw
    raw: r#"a "b"
c"#,
    // bytes
    bytes: b"x
y",
    // list
    list: [
        "p
q", /* multi
        line */ 2,
    ],
)
"##;
    let want: [&[&str]; 5] = [
        &["\"line one\nline two\n\""],
        &["r#\"a \"b\"\nc\"#"],
        &["b\"x\ny\""],
        &["[", "\"p\nq\"", ",", "2", "]"],
        &["2"],
    ];
    let mixed = SOURCE.replace("line one\n", "line one\r\n");
    for (mode, text) in [
        ("LF", SOURCE.to_owned()),
        ("CRLF", crlf(SOURCE)),
        ("CRLF inside one string only", mixed),
    ] {
        let entries = hashed(&text, mode);
        assert_eq!(
            keys(&entries),
            path_comments(&text),
            "{mode}: keyed by the comments of this text"
        );
        assert_eq!(entries.len(), want.len(), "{mode}: one entry per comment");
        for ((_, digest), tokens) in entries.iter().zip(want) {
            assert_oracle(*digest, tokens, mode);
        }
    }
    // Only a CR right before an LF is dropped: a lone CR is text.
    let lone = "Config(\n    // cr\n    cr: \"a\rb\",\n)\n";
    for (mode, text) in [("LF", lone.to_owned()), ("CRLF", crlf(lone))] {
        assert_oracle(value_of(&text, "// cr", mode), &["\"a\rb\""], mode);
    }
}

/// One element per row: its source text and its tokens by hand. Each row is
/// one value of a list, led by `/* cN */`.
const PINNED: &[(&str, &[&str])] = &[
    ("3", &["3"]),
    ("-0.5e-3", &["-0.5e-3"]),
    ("+4", &["+4"]),
    (".5", &[".5"]),
    ("0x1F", &["0x1F"]),
    ("1_000", &["1_000"]),
    ("-inf", &["-inf"]),
    ("+NaN", &["+NaN"]),
    ("1e10", &["1e10"]),
    ("inf", &["inf"]),
    ("true", &["true"]),
    ("None", &["None"]),
    ("r#type", &["r#type"]),
    (r#""a \"q\" \\ b""#, &[r#""a \"q\" \\ b""#]),
    (r##"r#"x "y" z"#"##, &[r##"r#"x "y" z"#"##]),
    (r###"r##"a "# b"##"###, &[r###"r##"a "# b"##"###]),
    (r#"b"bytes""#, &[r#"b"bytes""#]),
    (r##"br#"raw "bytes""#"##, &[r##"br#"raw "bytes""#"##]),
    ("'c'", &["'c'"]),
    (r"'\''", &[r"'\''"]),
    (r"'\u{1F600}'", &[r"'\u{1F600}'"]),
    ("b'x'", &["b'x'"]),
    ("\"multi\nline\"", &["\"multi\nline\""]),
    ("Some(3)", &["Some", "(", "3", ")"]),
    (
        r#"Player(name: "hero", speed: 4.5,)"#,
        &[
            "Player", "(", "name", ":", "\"hero\"", ",", "speed", ":", "4.5", ")",
        ],
    ),
    (
        "(1, (2, 3),)",
        &["(", "1", ",", "(", "2", ",", "3", ")", ")"],
    ),
    (
        "[[], [1], [1, 2,],]",
        &[
            "[", "[", "]", ",", "[", "1", "]", ",", "[", "1", ",", "2", "]", "]",
        ],
    ),
    (
        r#"{"a": 1, (0, 1): [x], 2: Unit,}"#,
        &[
            "{", "\"a\"", ":", "1", ",", "(", "0", ",", "1", ")", ":", "[", "x", "]", ",", "2",
            ":", "Unit", "}",
        ],
    ),
    ("()", &["(", ")"]),
    ("[]", &["[", "]"]),
    ("{}", &["{", "}"]),
];

/// The lexer is outside `code_recipe`: a change to how it cuts or spells a
/// token changes these streams, and must bump `LEVEL_RECIPE`.
#[test]
fn ron_value_stream_is_pinned() {
    let mut source = String::from("[\n");
    for (index, (text, _)) in PINNED.iter().enumerate() {
        source.push_str(&format!("    /* c{index} */ {text},\n"));
    }
    source.push_str("]\n");
    let analysis = ron::analyze(&source);
    assert!(
        !analysis.has_error,
        "premise: the pinned list is clean RON: {:?}",
        analysis.rejected
    );
    let entries = hashed(&source, "pinned");
    assert_eq!(entries.len(), PINNED.len(), "one entry per led element");
    for (index, (text, tokens)) in PINNED.iter().enumerate() {
        let label = format!("/* c{index} */");
        assert_oracle(value_of(&source, &label, text), tokens, text);
        assert_eq!(
            entries[index].0,
            comment(&source, &label, text),
            "{text}: entries in source order"
        );
    }
}

// ------------------------------------------------------------------- RD-2

/// `f` over `source` on a 2 MB thread, the fastest of three runs, with its
/// (deterministic) result.
fn best_of_three(source: &str, f: fn(&str) -> usize) -> (usize, Duration) {
    let mut best: Option<(usize, Duration)> = None;
    for _ in 0..3 {
        let text = source.to_owned();
        let (out, elapsed) = thread::Builder::new()
            .name("ron-values-cost".to_owned())
            .stack_size(TWO_MB)
            .spawn(move || {
                let start = Instant::now();
                let out = f(&text);
                (out, start.elapsed())
            })
            .expect("thread spawns")
            .join()
            .expect("must not panic or overflow");
        best = Some(match best {
            None => (out, elapsed),
            Some((previous, time)) => {
                assert_eq!(previous, out, "deterministic");
                (out, time.min(elapsed))
            }
        });
    }
    best.expect("three runs")
}

/// One lex and one structure walk per file: hashing every path comment's
/// value costs the order of `comment_anchors` on the same text (one lex,
/// one walk), not a lex per comment.
#[test]
fn ron_value_hashes_linear_in_comments() {
    const ENTRIES: usize = 10_000;
    let mut source = String::from("Config(\n");
    for i in 0..ENTRIES {
        source.push_str(&format!("    // c{i}\n    a{i}: [{i}, {i}],\n"));
    }
    source.push_str(")\n");
    let (entries, values_time) = best_of_three(&source, |s| hashed(s, "linear").len());
    let (anchors, anchors_time) = best_of_three(&source, |s| ron::comment_anchors(s).len());
    assert_eq!(anchors, ENTRIES, "premise: one comment per entry");
    assert_eq!(
        entries, ENTRIES,
        "every own-line comment binds to its entry"
    );
    let bound = anchors_time * 10 + Duration::from_millis(250);
    eprintln!("value_hashes {values_time:?}, comment_anchors {anchors_time:?}, bound {bound:?}");
    assert!(
        values_time <= bound,
        "{values_time:?} for {ENTRIES} values against {anchors_time:?} for their anchors \
         (bound {bound:?}): not one lex and one walk per file"
    );
}

/// The `.ron` files of `fixtures/hash-levels/ron/` and `fixtures/ron/`
/// (both read only), labelled by their path below `fixtures/`, sorted.
fn ron_fixture_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (dir, label) in [
        (fixture_root().join("ron"), "hash-levels/ron"),
        (fixture_root().join("../ron"), "ron"),
    ] {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("a directory entry").path();
            if path.extension().is_some_and(|e| e == "ron") {
                let name = path
                    .file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned();
                let text = if label == "ron" {
                    std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                } else {
                    read(&format!("ron/{name}"))
                };
                out.push((format!("{label}/{name}"), text));
            }
        }
    }
    out.sort();
    assert!(out.len() >= 8, "both fixtures hold their files: {out:?}");
    out
}

#[test]
fn unmarked_path_comments_are_keyed_as_comment_anchors() {
    const MIXED: &str = "// @implements CFG-1@1\nConfig(\n    // plain note\n    a: 1, // @implements A-1@2 [sig] and a note\n    /* @implements B-1@1 */ b: [1, /* unmarked */ 2],\n    dup: 1,\n    // @implements D-1@1\n    dup: 2,\n    // closing note\n)\n";
    let mut sources = vec![("inline".to_owned(), MIXED.to_owned())];
    sources.extend(ron_fixture_files());
    let mut checked = 0;
    for (name, text) in &sources {
        for (mode, source) in [("LF", text.clone()), ("CRLF", crlf(text))] {
            let analysis = ron::analyze(&source);
            if analysis.has_error {
                // Refused files have no entry: `broken_*` and RD-1's tests.
                continue;
            }
            let entries = hashed(&source, name);
            let got = keys(&entries);
            assert_eq!(
                got,
                path_comments(&source),
                "{name} ({mode}): one entry per comment with a path anchor, marked or not, \
                 keyed by `CommentAnchor::comment`, in source order"
            );
            for marker in &analysis.markers {
                if matches!(marker.anchor, Anchor::Path { .. }) {
                    assert!(
                        got.contains(&marker.comment),
                        "{name} ({mode}): the marker comment at {:?} has its entry",
                        marker.comment
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(checked >= 8, "premise: clean sources checked: {checked}");
    // The inline source by hand: unmarked comments included, the ambiguous
    // and the unanchored ones left out.
    let want: Vec<Range<usize>> = [
        "// @implements CFG-1@1",
        "// plain note",
        "// @implements A-1@2 [sig] and a note",
        "/* @implements B-1@1 */",
        "/* unmarked */",
    ]
    .iter()
    .map(|text| comment(MIXED, text, "inline"))
    .collect();
    assert_eq!(keys(&hashed(MIXED, "inline")), want);
}

// ------------------------------------------------------------------- TE-2

/// Every entry of every file, one line each: name, key, digest in hex, or
/// the refusal. Two equal dumps are byte-for-byte equal results.
fn dump_one(name: &str, source: &str) -> String {
    match values(source) {
        Values::Hashed(entries) => entries
            .iter()
            .map(|(range, digest)| format!("{name}\t{}..{}\t{digest}\n", range.start, range.end))
            .collect(),
        Values::Refused(detail) => format!("{name}\tcannot_verify\t{detail}\n"),
    }
}

fn dump(files: &[(String, String)]) -> String {
    files
        .iter()
        .map(|(name, source)| dump_one(name, source))
        .collect()
}

#[test]
fn te2_two_runs_and_reversed_file_order_give_the_same_bytes() {
    for mode in ["LF", "CRLF"] {
        let files: Vec<(String, String)> = ron_fixture_files()
            .into_iter()
            .map(|(name, text)| {
                let text = if mode == "CRLF" { crlf(&text) } else { text };
                (name, text)
            })
            .collect();
        let first = dump(&files);
        let second = dump(&files);
        let mut backward: Vec<String> = files
            .iter()
            .rev()
            .map(|(name, source)| dump_one(name, source))
            .collect();
        backward.reverse();
        let moved = files.clone();
        let on_a_thread = thread::spawn(move || dump(&moved))
            .join()
            .expect("must not panic");
        assert!(first.lines().count() >= 30, "{mode}: every entry dumped");
        assert!(
            first.matches("\tcannot_verify\t").count() >= 3,
            "{mode}: the refused files dumped"
        );
        assert_eq!(first, second, "{mode}: two runs");
        assert_eq!(first, backward.concat(), "{mode}: reversed file order");
        assert_eq!(first, on_a_thread, "{mode}: another thread");
    }
}

#[test]
fn te2_no_offset_line_or_path_enters_a_value_hash() {
    // Blank lines above a file: the same digests, every key shifted.
    for (name, source) in ron_fixture_files() {
        let shifted = format!("\n\n\n{source}");
        match (values(&source), values(&shifted)) {
            (Values::Hashed(plain), Values::Hashed(moved)) => {
                assert_eq!(plain.len(), moved.len(), "{name}");
                for ((a, digest_a), (b, digest_b)) in plain.iter().zip(&moved) {
                    assert_eq!(digest_a, digest_b, "{name}: {a:?} moved by three lines");
                    assert_eq!((a.start + 3, a.end + 3), (b.start, b.end), "{name}: keys");
                }
            }
            (Values::Refused(a), Values::Refused(b)) => assert_eq!(a, b, "{name}"),
            pair => panic!("{name}: refusal changed with the offset: {pair:?}"),
        }
    }
    // One value text at three paths, depths and offsets: one digest.
    let want = spaced("[ 1 , ( x : 2 ) ]");
    let places = [
        ("the root", "/* a */ [1, (x: 2)]\n", "/* a */"),
        (
            "root.outer.inner",
            "Config(\n    outer: Outer(\n        // b\n        inner: [1, (x: 2)],\n    ),\n)\n",
            "// b",
        ),
        ("root[1]", "[0, /* c */ [1, (x: 2)]]\n", "/* c */"),
    ];
    for (what, source, anchor) in places {
        assert_oracle(value_of(source, anchor, what), &want, what);
    }
}

// --------------------------------------------------- depth, readings (L6, L9)

/// `MAX_DEPTH` nested lists, each with a comment trailing its opener: every
/// one hashed (`O(n × depth)`, "Risks") on a 2 MB stack, each the stream of
/// its own brackets around `1`.
#[test]
fn ron_values_at_max_depth_hash_on_a_two_megabyte_stack() {
    let mut source = String::new();
    for level in 0..MAX_DEPTH {
        source.push_str(&format!("[ // d{level}\n"));
    }
    source.push_str("1\n");
    source.push_str(&"]\n".repeat(MAX_DEPTH));
    assert!(
        !ron::analyze(&source).has_error,
        "premise: at the cap, not past it"
    );
    let text = source.clone();
    let entries = thread::Builder::new()
        .stack_size(TWO_MB)
        .spawn(move || hashed(&text, "max depth"))
        .expect("thread spawns")
        .join()
        .expect("must not panic or overflow");
    assert_eq!(keys(&entries), path_comments(&source));
    assert_eq!(entries.len(), MAX_DEPTH);
    for (level, (_, digest)) in entries.iter().enumerate() {
        let around = MAX_DEPTH - level;
        let mut tokens = vec!["["; around];
        tokens.push("1");
        tokens.extend(std::iter::repeat_n("]", around));
        assert_oracle(*digest, &tokens, &format!("// d{level}"));
    }
}

/// L6: a clean file whose comments bind to no path is hashed with no entry.
#[test]
fn clean_file_without_path_comments_hashes_no_value() {
    for source in [
        "Config(a: 1)\n",
        "Config(\n    a: 1,\n    // closing\n)\n",
        "Config(\n    // dup\n    a: 1,\n    a: 2,\n)\n",
    ] {
        assert!(!ron::analyze(source).has_error, "{source:?}: premise");
        assert!(path_comments(source).is_empty(), "{source:?}: premise");
        assert_eq!(values(source), Values::Hashed(Vec::new()), "{source:?}");
    }
}

/// L9: comments on one value are one entry each, with one digest.
#[test]
fn comments_on_one_value_are_one_entry_each() {
    let source = "Config(\n    // first\n    /* second */ speed: 4.5, // third\n)\n";
    let entries = hashed(source, "three comments");
    let want: Vec<Range<usize>> = ["// first", "/* second */", "// third"]
        .iter()
        .map(|text| comment(source, text, "three comments"))
        .collect();
    assert_eq!(keys(&entries), want);
    for (_, digest) in &entries {
        assert_oracle(*digest, &["4.5"], "root.speed");
    }
}

/// R2 (the developer's reading, beside L1): a comment the shipped walk
/// resolves to a map entry while the entry's key is parsed (here: trailing
/// a tuple key's opener) hashes the entry's value, never the key.
#[test]
fn comment_resolved_while_a_map_key_is_parsed_hashes_the_entrys_value() {
    let base = "{\n    ( // k\n        1, 2): [3, 4],\n    (5, 6): 7,\n}\n";
    let key = comment(base, "// k", "R2");
    let anchors = ron::comment_anchors(base);
    let on_the_entry = |anchor: &Anchor| match anchor {
        Anchor::Path { path, depth } => path.starts_with("root{(") && *depth == 1,
        _ => false,
    };
    assert!(
        anchors
            .iter()
            .any(|c| c.comment == key && on_the_entry(&c.anchor)),
        "premise: `// k` binds to the entry's path: {anchors:?}"
    );
    let want = spaced("[ 3 , 4 ]");
    assert_oracle(value_of(base, "// k", "R2"), &want, "R2");
    let key_edited = base.replace("1, 2)", "1, 9)");
    assert_oracle(
        value_of(&key_edited, "// k", "R2: key edited"),
        &want,
        "R2: the key is not the entry's value",
    );
    let value_edited = base.replace("[3, 4]", "[3, 8]");
    assert_ne!(
        value_of(&value_edited, "// k", "R2: value edited"),
        value_of(base, "// k", "R2"),
        "R2: the entry's value is hashed"
    );
}

/// R2b (owner pre-approved): a comment inside a key's elements names that
/// key element (`root{(/* m */ 0, 0)}.0`, the shipped path) and hashes it.
#[test]
fn comment_inside_a_map_keys_element_hashes_that_element() {
    let source = "{ (/* m */ 0, 0): 1 }\n";
    let key = comment(source, "/* m */", "R2b");
    let anchor = ron::comment_anchors(source)
        .into_iter()
        .find(|c| c.comment == key)
        .map(|c| c.anchor);
    assert_eq!(
        anchor,
        Some(Anchor::Path {
            path: "root{(/* m */ 0, 0)}.0".to_owned(),
            depth: 2,
        }),
        "premise: the shipped anchor"
    );
    assert_oracle(value_of(source, "/* m */", "R2b"), &["0"], "R2b");
}
