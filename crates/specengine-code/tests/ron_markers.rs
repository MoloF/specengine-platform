//! AC-07 of docs/features/phase-0-spikes.md, binding rules 1-5 of 05 §5.3: a
//! marker in a `.ron` comment resolves to a field path through the own lexer
//! (`ron::analyze`), on `fixtures/ron` and on inline edge data. Nothing here
//! writes into the fixture.
//!
//! The mixed-script ID is assembled from a Unicode escape so no non-Latin
//! letter sits in the repository (ADR-0024, checked by the anonymity test).

use std::fs;
use std::path::{Path, PathBuf};

use specengine_code::markers::Relation;
use specengine_code::ron::{self, Anchor, RonAnalysis};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("ron")
}

fn fixture(name: &str) -> String {
    let path = fixture_dir().join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `(line, anchor)` of every marker, in source order.
fn anchors(analysis: &RonAnalysis) -> Vec<(usize, String)> {
    analysis
        .markers
        .iter()
        .map(|m| (m.line, m.anchor.as_str().to_owned()))
        .collect()
}

fn depth(anchor: &Anchor) -> Option<usize> {
    match anchor {
        Anchor::Path { depth, .. } => Some(*depth),
        _ => None,
    }
}

/// Every comment range is a comment in the source, ranges ascend without
/// overlapping, and every `@implements` in the source lies inside one.
fn assert_comment_ranges(source: &str, analysis: &RonAnalysis) {
    let mut last_end = 0;
    for range in &analysis.comments {
        assert!(
            range.start >= last_end && range.end <= source.len(),
            "comment ranges must ascend without overlap: {range:?} after {last_end}"
        );
        let text = &source[range.clone()];
        assert!(
            text.starts_with("//") || (text.starts_with("/*") && text.ends_with("*/")),
            "not a comment: {text:?}"
        );
        last_end = range.end;
    }
    let mut at = 0;
    while let Some(found) = source[at..].find("@implements") {
        let offset = at + found;
        assert!(
            analysis.comments.iter().any(|r| r.contains(&offset)),
            "the marker at byte {offset} lies outside every comment range"
        );
        at = offset + 1;
    }
}

/// The three markers of `fixtures/ron/config.ron` (its `expected.json`): the
/// two nested anchors and the one adjacent to no field.
const CONFIG_ANCHORS: [(usize, &str, Option<usize>); 3] = [
    (7, "root.player.speed", Some(2)),
    (14, "root.waves[2]", Some(2)),
    (19, "unanchored", None),
];

/// Every extra anchor shape: `(line, id, anchor, depth)`.
const ANCHOR_CASES: &str = r#"// @implements TOP@1
Config(
    name: "hero", // @implements TRAIL@1
    speed: // @implements MID@1
        4.5,
    weapon: Some(Weapon(
        // @implements OPT@1
        damage: 10,
    )),
    spawns: {
        "fire": 3,
        // @implements MAP@1
        "ice": 2,
    },
    pair: (
        1,
        // @implements TUP@1
        2,
    ),
    /* @assumes ASM-1 */
    volume: 0.5,
)
"#;

const ANCHOR_EXPECTED: [(usize, &str, &str, Option<usize>); 7] = [
    (1, "TOP", "root", Some(0)),
    (3, "TRAIL", "root.name", Some(1)),
    (4, "MID", "unanchored", None),
    (7, "OPT", "root.weapon.0.damage", Some(3)),
    (12, "MAP", "root.spawns{\"ice\"}", Some(2)),
    (17, "TUP", "root.pair.1", Some(2)),
    (20, "ASM-1", "root.volume", Some(1)),
];

const UNTERMINATED_STRING: &str = "Config(\n    name: \"hero,\n    speed: 4.5,\n)\n";
const COMMENT_ONLY: &str = "// @implements LONELY@1\n";

/// `X` followed by a Cyrillic look-alike of `x`, then `-1`: a mixed-script ID.
fn mixed_script_id() -> String {
    "X\u{0445}-1".to_owned()
}

fn mixed_script_source() -> String {
    format!(
        "Config(\n    // @implements {}@1\n    speed: 4.5,\n)\n",
        mixed_script_id()
    )
}

// ------------------------------------------------------------------ AC-07

#[test]
fn fixture_config_resolves_nested_struct_list_and_unanchored() {
    let source = fixture("config.ron");
    let analysis = ron::analyze(&source);
    assert!(
        !analysis.has_error,
        "config.ron must lex clean: {:?}",
        analysis.error_categories
    );
    assert!(analysis.rejected.is_empty());
    assert_eq!(
        anchors(&analysis),
        CONFIG_ANCHORS
            .iter()
            .map(|(line, anchor, _)| (*line, (*anchor).to_owned()))
            .collect::<Vec<_>>()
    );
    for (marker, (_, _, expected_depth)) in analysis.markers.iter().zip(CONFIG_ANCHORS) {
        assert_eq!(depth(&marker.anchor), expected_depth, "{marker:?}");
        assert_eq!(marker.marker.relation, Relation::Implements);
        assert_eq!(marker.marker.id, "X");
        assert_eq!(marker.marker.rev, Some(1));
        assert!(marker.marker.id_latin);
        assert_eq!(marker.marker.note, None);
        assert_eq!(&source[marker.comment.clone()], "// @implements X@1");
    }
    assert_eq!(analysis.markers[2].anchor, Anchor::Unanchored);
    assert!(analysis.has_nested_anchor());
    assert_eq!(analysis.comments.len(), 6);
    assert_comment_ranges(&source, &analysis);
}

#[test]
fn fixture_extensions_lex_clean_without_markers() {
    let source = fixture("extensions.ron");
    let analysis = ron::analyze(&source);
    assert!(
        !analysis.has_error,
        "the lexer accepts `#![enable(...)]` and raw strings: {:?}",
        analysis.error_categories
    );
    assert!(analysis.markers.is_empty());
    assert_eq!(analysis.comments.len(), 2);
    assert_comment_ranges(&source, &analysis);
}

#[test]
fn every_anchor_shape_resolves_to_its_path() {
    let analysis = ron::analyze(ANCHOR_CASES);
    assert!(!analysis.has_error, "{:?}", analysis.error_categories);
    let got: Vec<(usize, &str, &str, Option<usize>)> = analysis
        .markers
        .iter()
        .map(|m| {
            (
                m.line,
                m.marker.id.as_str(),
                m.anchor.as_str(),
                depth(&m.anchor),
            )
        })
        .collect();
    assert_eq!(got, ANCHOR_EXPECTED);
    let assumes = analysis.markers.last().unwrap();
    assert_eq!(assumes.marker.relation, Relation::Assumes);
    assert_eq!(assumes.marker.rev, None);
    assert_eq!(assumes.marker.note, None, "`*/` is not a note");
    assert_eq!(
        &ANCHOR_CASES[assumes.comment.clone()],
        "/* @assumes ASM-1 */"
    );
    assert_comment_ranges(ANCHOR_CASES, &analysis);
}

#[test]
fn several_markers_in_one_comment_share_the_anchor() {
    let source = "Config(\n    // @implements A@2 @verifies B the note\n    speed: 1,\n)\n";
    let analysis = ron::analyze(source);
    assert!(!analysis.has_error);
    assert_eq!(analysis.markers.len(), 2);
    for marker in &analysis.markers {
        assert_eq!(marker.anchor.as_str(), "root.speed");
        assert_eq!(marker.line, 2);
    }
    assert_eq!(analysis.markers[0].marker.id, "A");
    assert_eq!(analysis.markers[0].marker.rev, Some(2));
    assert_eq!(analysis.markers[1].marker.relation, Relation::Verifies);
    assert_eq!(analysis.markers[1].marker.id, "B");
    assert_eq!(analysis.markers[1].marker.note.as_deref(), Some("the note"));
}

/// A `{key}` segment is the key's source text (05 §5.3): a string key keeps
/// its quotes, and whitespace runs collapse to one space — across lines and
/// inside strings too. Named mutation: `segment_text` returning the raw slice
/// instead of `collapse_whitespace(..)` turns this red.
#[test]
fn map_key_segment_keeps_quotes_and_collapses_whitespace() {
    const SOURCE: &str = "{\n    // @implements TUPLE@1\n    (1,\n      2): \"a\",\n    \
        // @implements SPACED@1\n    \"fire  ice\": 3,\n}\n";
    for source in [SOURCE.to_owned(), SOURCE.replace('\n', "\r\n")] {
        let analysis = ron::analyze(&source);
        assert!(
            !analysis.has_error,
            "{source:?}: {:?}",
            analysis.error_categories
        );
        let got: Vec<(&str, &str, Option<usize>)> = analysis
            .markers
            .iter()
            .map(|m| (m.marker.id.as_str(), m.anchor.as_str(), depth(&m.anchor)))
            .collect();
        assert_eq!(
            got,
            [
                ("TUPLE", "root{(1, 2)}", Some(1)),
                ("SPACED", "root{\"fire ice\"}", Some(1)),
            ],
            "{source:?}"
        );
    }
}

#[test]
fn analysis_is_deterministic() {
    for source in [
        fixture("config.ron"),
        fixture("extensions.ron"),
        ANCHOR_CASES.to_owned(),
        UNTERMINATED_STRING.to_owned(),
        mixed_script_source(),
    ] {
        assert_eq!(ron::analyze(&source), ron::analyze(&source));
    }
}

// --------------------------------------------------------------- edge data

#[test]
fn unterminated_string_reports_both_categories_with_the_offset() {
    let analysis = ron::analyze(UNTERMINATED_STRING);
    assert!(analysis.has_error);
    assert_eq!(
        analysis.error_categories,
        ["unbalanced_delimiter", "unterminated_string"]
    );
    let quote = UNTERMINATED_STRING.find('"').unwrap();
    assert!(
        analysis
            .rejected
            .iter()
            .any(|r| r.category == "unterminated_string" && r.offset == quote),
        "the unterminated string must be reported at its opening quote: {:?}",
        analysis.rejected
    );
    assert!(analysis.markers.is_empty());
}

#[test]
fn comment_only_file_is_empty_file_and_its_marker_unanchored() {
    let analysis = ron::analyze(COMMENT_ONLY);
    assert!(analysis.has_error);
    assert_eq!(analysis.error_categories, ["empty_file"]);
    assert_eq!(analysis.comments.len(), 1);
    assert_eq!(anchors(&analysis), [(1, "unanchored".to_owned())]);
}

#[test]
fn empty_file_is_empty_file() {
    let analysis = ron::analyze("");
    assert!(analysis.has_error);
    assert_eq!(analysis.error_categories, ["empty_file"]);
    assert!(analysis.comments.is_empty());
    assert!(analysis.markers.is_empty());
}

#[test]
fn broken_input_never_panics_and_is_never_clean() {
    for source in [
        ")))",
        "(((",
        "[1, 2, 3",
        "{ \"k\": }",
        "{:,}",
        "#![enable(",
        "/* open",
        "'x",
        "\"open",
        "Config(a: 1 b: 2)",
        "Config(a: 1,, b: 2)",
        "Config(: 1)",
        "@implements",
        "Config() trailing",
        "Config(\n    // @implements X@1\n",
        "// @implements X@1\n)",
        "\u{0}\u{1}\u{2}",
    ] {
        let analysis = ron::analyze(source);
        assert!(analysis.has_error, "{source:?} must not count as clean");
        assert!(
            !analysis.error_categories.is_empty(),
            "{source:?}: a broken file names at least one category"
        );
        for rejected in &analysis.rejected {
            assert!(
                rejected.offset <= source.len(),
                "{source:?}: offset {} beyond the end",
                rejected.offset
            );
        }
        for marker in &analysis.markers {
            assert!(marker.line >= 1, "{source:?}: {marker:?}");
            assert!(!marker.anchor.as_str().is_empty());
        }
    }
}

#[test]
fn mixed_script_id_is_reported_and_still_resolves() {
    let source = mixed_script_source();
    let analysis = ron::analyze(&source);
    assert!(
        !analysis.has_error,
        "a mixed-script ID is not a parse error"
    );
    assert_eq!(analysis.markers.len(), 1);
    let marker = &analysis.markers[0];
    assert!(
        !marker.marker.id_latin,
        "ADR-0009: mixed scripts are flagged"
    );
    assert_eq!(marker.marker.id, mixed_script_id());
    assert_eq!(marker.marker.rev, Some(1));
    assert_eq!(marker.anchor.as_str(), "root.speed");
    let latin = ron::analyze("Config(\n    // @implements X-1@1\n    speed: 4.5,\n)\n");
    assert!(latin.markers[0].marker.id_latin);
}

// ------------------------------------------- adjacency (owner decision)

/// One adjacency case of the owner's binding rules 1-5 (05 §5.3):
/// `source` with every `@` expanded to `@implements ` (so `// @M` is the
/// marker `M`), the `(id, anchor)` of every marker in source order, and
/// whether the file must lex and walk clean.
struct Adjacency {
    name: &'static str,
    source: &'static str,
    expected: &'static [(&'static str, &'static str)],
    clean: bool,
}

const ADJACENCY: &[Adjacency] = &[
    Adjacency {
        name: "trailing after the comma",
        source: "Config(\n    speed: 1, // @M\n)\n",
        expected: &[("M", "root.speed")],
        clean: true,
    },
    Adjacency {
        name: "trailing before a comma on the next line",
        source: "Config(\n    a: 1 // @M\n    ,\n    b: 2,\n)\n",
        expected: &[("M", "root.a")],
        clean: true,
    },
    Adjacency {
        name: "block comment after the comma",
        source: "Config(\n    a: 1, /* @M */\n)\n",
        expected: &[("M", "root.a")],
        clean: true,
    },
    Adjacency {
        name: "block comment before the comma",
        source: "Config(\n    a: 1 /* @M */,\n)\n",
        expected: &[("M", "root.a")],
        clean: true,
    },
    Adjacency {
        name: "block comment between two entries on one line",
        source: "Config(\n    a: 1, /* @M */ b: 2,\n)\n",
        expected: &[("M", "root.b")],
        clean: true,
    },
    Adjacency {
        name: "block and line comment trailing one entry",
        source: "Config(\n    a: 1, /* @M */ // @N\n)\n",
        expected: &[("M", "root.a"), ("N", "root.a")],
        clean: true,
    },
    Adjacency {
        name: "trailing then own line",
        source: "Config(\n    a: 1, // @M\n    // @N\n    b: 2,\n)\n",
        expected: &[("M", "root.a"), ("N", "root.b")],
        clean: true,
    },
    Adjacency {
        name: "after a stray comma on its own line",
        source: "Config(\n    a: 1\n    , // @M\n    b: 2,\n)\n",
        expected: &[("M", "unanchored")],
        clean: true,
    },
    Adjacency {
        name: "trailing a list element",
        source: "Config(\n    waves: [1, 2, // @M\n        3],\n)\n",
        expected: &[("M", "root.waves[1]")],
        clean: true,
    },
    Adjacency {
        name: "trailing a closed list, closer on the next line",
        source: "Config(\n    waves: [1, 2, 3], // @M\n)\n",
        expected: &[("M", "root.waves")],
        clean: true,
    },
    Adjacency {
        name: "block comment before the list closer",
        source: "Config(\n    waves: [1, 2, 3 /* @M */],\n)\n",
        expected: &[("M", "root.waves[2]")],
        clean: true,
    },
    Adjacency {
        name: "trailing a struct opener",
        source: "Config(\n    player: Player( // @M\n        name: \"hero\",\n    ),\n)\n",
        expected: &[("M", "root.player")],
        clean: true,
    },
    Adjacency {
        name: "trailing a list opener",
        source: "Config(\n    list: [ // @M\n        1,\n    ],\n)\n",
        expected: &[("M", "root.list")],
        clean: true,
    },
    Adjacency {
        name: "block comment right after a tuple opener",
        source: "Config(\n    d: (/* @M */ 1, 2),\n)\n",
        expected: &[("M", "root.d.0")],
        clean: true,
    },
    Adjacency {
        name: "trailing a struct closer",
        source: "Config(\n    player: Player(\n        name: \"hero\",\n    ), // @M\n)\n",
        expected: &[("M", "root.player")],
        clean: true,
    },
    Adjacency {
        name: "trailing nested closers",
        source: "Config(\n    f: Some(Weapon(\n        damage: 10,\n    )), // @M\n)\n",
        expected: &[("M", "root.f")],
        clean: true,
    },
    Adjacency {
        name: "trailing the root opener",
        source: "Config( // @M\n    a: 1,\n)\n",
        expected: &[("M", "root")],
        clean: true,
    },
    Adjacency {
        name: "trailing the root closer at the end of input",
        source: "Config(\n    a: 1,\n) // @M",
        expected: &[("M", "root")],
        clean: true,
    },
    Adjacency {
        name: "four block comments on one line",
        source: "/* @A1 */ Config( /* @A2 */ x: 1 /* @A3 */ ) /* @A4 */",
        expected: &[
            ("A1", "root"),
            ("A2", "root.x"),
            ("A3", "root.x"),
            ("A4", "root"),
        ],
        clean: true,
    },
    Adjacency {
        name: "trailing an extension attribute",
        source: "#![enable(implicit_some)] // @M\nConfig(\n    a: 1,\n)\n",
        expected: &[("M", "root")],
        clean: true,
    },
    Adjacency {
        name: "trailing a map entry",
        source: "{\n    \"fire\": 3, // @M\n    \"ice\": 2,\n}\n",
        expected: &[("M", "root{\"fire\"}")],
        clean: true,
    },
    Adjacency {
        name: "between a map key and its colon",
        source: "{\n    \"fire\": 3,\n    \"ice\" /* @M */ : 2,\n}\n",
        expected: &[("M", "unanchored")],
        clean: true,
    },
    Adjacency {
        name: "trailing a map value's opener",
        source: "{\n    \"b\": [ // @M\n        1,\n    ],\n}\n",
        expected: &[("M", "root{\"b\"}")],
        clean: true,
    },
    Adjacency {
        name: "trailing a multi-line string on its last line",
        source: "Config(\n    g: \"multi\nline\", // @M\n    h: 1,\n)\n",
        expected: &[("M", "root.g")],
        clean: true,
    },
    Adjacency {
        name: "after a colon",
        source: "Config(\n    speed: // @M\n        4.5,\n)\n",
        expected: &[("M", "unanchored")],
        clean: true,
    },
    Adjacency {
        name: "after error recovery consumed the line",
        source: "Config(\n    a: 1 2, // @M\n    b: 3,\n)\n",
        expected: &[("M", "unanchored")],
        clean: false,
    },
    Adjacency {
        name: "trailing an element of an unclosed list at the end of input",
        source: "Config(\n    a: [1, // @M",
        expected: &[("M", "root.a[0]")],
        clean: false,
    },
    Adjacency {
        name: "own line inside an unclosed list at the end of input",
        source: "Config(\n    a: [1\n    // @M",
        expected: &[("M", "unanchored")],
        clean: false,
    },
    Adjacency {
        name: "trailing content after the root value",
        source: "Config(a: 1) trailing // @M",
        expected: &[("M", "unanchored")],
        clean: false,
    },
    // A multi-line block comment trails by the line of its `/*` and leads
    // by the line of its `*/`.
    Adjacency {
        name: "block comment from a value's line into the next line",
        source: "Config(\n    a: 1, /* @M\n    still the comment */ b: 2,\n)\n",
        expected: &[("M", "root.b")],
        clean: true,
    },
    Adjacency {
        name: "block comment from a value's line, marker on a later line",
        source: "Config(\n    a: 1, /* note\n    @M */\n    b: 2,\n)\n",
        expected: &[("M", "root.a")],
        clean: true,
    },
    Adjacency {
        name: "block comment from its own line onto the next entry's line",
        source: "Config(\n    a: 1,\n    /* @M\n    */ b: 2,\n)\n",
        expected: &[("M", "root.b")],
        clean: true,
    },
    // Leading a value (owner decision): a block comment followed, on the
    // line of its `*/`, by the token a value begins with binds to that value.
    Adjacency {
        name: "leading tuple elements",
        source: "Config(\n    pos: (/* @A */ 10, /* @B */ 20),\n)\n",
        expected: &[("A", "root.pos.0"), ("B", "root.pos.1")],
        clean: true,
    },
    Adjacency {
        name: "leading an entry's value",
        source: "Config(\n    speed: /* @M */ 4.5,\n)\n",
        expected: &[("M", "root.speed")],
        clean: true,
    },
    Adjacency {
        name: "leading a map key after the opener",
        source: "{ /* @M */ \"fire\": 3 }",
        expected: &[("M", "root{\"fire\"}")],
        clean: true,
    },
    Adjacency {
        name: "leading a map value",
        source: "{\n    \"fire\": /* @M */ 3,\n}\n",
        expected: &[("M", "root{\"fire\"}")],
        clean: true,
    },
    Adjacency {
        name: "own-line block leading the value after a colon",
        source: "Config(\n    speed:\n        /* @M */ 4.5,\n)\n",
        expected: &[("M", "root.speed")],
        clean: true,
    },
    Adjacency {
        name: "own-line block after a colon, value on the next line",
        source: "Config(\n    speed:\n        /* @M */\n        4.5,\n)\n",
        expected: &[("M", "unanchored")],
        clean: true,
    },
    Adjacency {
        name: "two blocks leading one entry",
        source: "Config(\n    a: 1, /* @M */ /* @N */ b: 2,\n)\n",
        expected: &[("M", "root.b"), ("N", "root.b")],
        clean: true,
    },
    Adjacency {
        name: "multi-line comment between it and the entry",
        source: "Config(\n    a: 1, /* @M */ /* x\n */ b: 2,\n)\n",
        expected: &[("M", "root.a")],
        clean: true,
    },
    Adjacency {
        name: "leading the first field after a struct opener",
        source: "Config(\n    player: Player( /* @M */ name: \"hero\",\n    ),\n)\n",
        expected: &[("M", "root.player.name")],
        clean: true,
    },
    Adjacency {
        name: "leading a tuple struct's element",
        source: "Config(\n    f: Some(/* @M */ Weapon(damage: 1)),\n)\n",
        expected: &[("M", "root.f.0")],
        clean: true,
    },
    Adjacency {
        name: "leading a map key and a list element",
        source: "{\n    \"a\": 1, /* @M */ \"b\": [ /* @N */ 2 ],\n}\n",
        expected: &[("M", "root{\"b\"}"), ("N", "root{\"b\"}[0]")],
        clean: true,
    },
    Adjacency {
        name: "own-line block before a closer",
        source: "Config(\n    a: 1,\n    /* @M */ )\n",
        expected: &[("M", "unanchored")],
        clean: true,
    },
    Adjacency {
        name: "between a type name and its opener",
        source: "Config(\n    p: Player /* @M */ (x: 1),\n)\n",
        expected: &[("M", "unanchored")],
        clean: true,
    },
    Adjacency {
        name: "after an extension attribute",
        source: "#![enable(implicit_some)] /* @M */ Config(\n    a: 1,\n)\n",
        expected: &[("M", "root")],
        clean: true,
    },
    Adjacency {
        name: "multi-line strings on both sides",
        source: "Config(\n    s: \"multi\nline\" /* @M */ , t: /* @N */ \"x\ny\",\n)\n",
        expected: &[("M", "root.s"), ("N", "root.t")],
        clean: true,
    },
    // A leading comment whose next token begins no value falls back to the
    // value it trails.
    Adjacency {
        name: "fallback, missing separator",
        source: "Config(\n    a: 1 /* @M */ 2, b: 3,\n)\n",
        expected: &[("M", "root.a")],
        clean: false,
    },
    Adjacency {
        name: "fallback, string in a field position",
        source: "Config(\n    a: 1, /* @M */ \"x\": 2,\n)\n",
        expected: &[("M", "root.a")],
        clean: false,
    },
    Adjacency {
        name: "fallback, trailing content",
        source: "Config(a: 1) /* @M */ trailing",
        expected: &[("M", "root")],
        clean: false,
    },
    Adjacency {
        name: "missing colon",
        source: "Config(\n    a: 1,\n    speed /* @M */ 4.5,\n)\n",
        expected: &[("M", "root.speed")],
        clean: false,
    },
    Adjacency {
        name: "followed by `:` in a list",
        source: "[1, /* @M */ : 2]",
        expected: &[("M", "root[0]")],
        clean: false,
    },
    Adjacency {
        name: "followed by an attribute in a field position",
        source: "Config(\n    a: 1, /* @M */ #![enable(x)] b: 2,\n)\n",
        expected: &[("M", "root.a")],
        clean: false,
    },
];

/// `@` → `@implements `: every table source is a marker per `@ID`.
fn expand(source: &str) -> String {
    source.replace('@', "@implements ")
}

/// `(id, anchor)` of every marker, in source order.
fn id_anchors(analysis: &RonAnalysis) -> Vec<(String, String)> {
    analysis
        .markers
        .iter()
        .map(|m| (m.marker.id.clone(), m.anchor.as_str().to_owned()))
        .collect()
}

/// Every case under both `\n` and `\r\n` line ends; all mismatches are
/// reported at once.
#[test]
fn same_line_marker_binds_to_the_entry_on_its_own_line() {
    let mut failures = Vec::new();
    for case in ADJACENCY {
        for (ending, eol) in [("LF", "\n"), ("CRLF", "\r\n")] {
            let source = expand(case.source).replace('\n', eol);
            let analysis = ron::analyze(&source);
            let got = id_anchors(&analysis);
            let expected: Vec<(String, String)> = case
                .expected
                .iter()
                .map(|(id, anchor)| ((*id).to_owned(), (*anchor).to_owned()))
                .collect();
            if got != expected {
                failures.push(format!(
                    "{} [{ending}]: expected {expected:?}, got {got:?}",
                    case.name
                ));
            }
            if analysis.has_error == case.clean {
                failures.push(format!(
                    "{} [{ending}]: has_error {} ({:?})",
                    case.name, analysis.has_error, analysis.error_categories
                ));
            }
            assert_comment_ranges(&source, &analysis);
            assert_eq!(analysis, ron::analyze(&source), "{} [{ending}]", case.name);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The line reported for a marker is the line its comment starts on, also
/// when the comment spans lines and under CRLF.
#[test]
fn marker_line_is_the_comment_start_line_under_crlf() {
    let source = expand("Config(\r\n    a: 1, /* note\r\n    @M */\r\n    b: 2, // @N\r\n)\r\n");
    let analysis = ron::analyze(&source);
    assert!(!analysis.has_error, "{:?}", analysis.error_categories);
    assert_eq!(
        anchors(&analysis),
        [(2, "root.a".to_owned()), (4, "root.b".to_owned())]
    );
    assert_eq!(analysis.markers[1].marker.id, "N");
    assert_eq!(analysis.markers[1].marker.note, None, "`\\r` is not a note");
}

/// 600 nested lists: the marker inside the part past [`ron::MAX_DEPTH`] is
/// `cannot_verify`, the one trailing the last closer binds to the entry, and
/// an own-line marker after it still binds to the next entry.
#[test]
fn trailing_marker_after_a_too_deep_value_binds_to_its_entry() {
    let levels = 600;
    assert!(levels > ron::MAX_DEPTH);
    let source = format!(
        "Config(\n    deep: {} // @implements M\n1{}, // @implements N\n    // @implements O\n    next: 2,\n)\n",
        "[".repeat(levels),
        "]".repeat(levels)
    );
    let analysis = ron::analyze(&source);
    assert_eq!(analysis.error_categories, ["nesting_too_deep"]);
    let got = id_anchors(&analysis);
    assert_eq!(
        got,
        [
            ("M".to_owned(), "cannot_verify".to_owned()),
            ("N".to_owned(), "root.deep".to_owned()),
            ("O".to_owned(), "root.next".to_owned()),
        ]
    );
    assert_eq!(
        analysis.markers.iter().map(|m| m.line).collect::<Vec<_>>(),
        [2, 3, 4]
    );
}

/// docs/features/layer-a-identity.md AC-12: the 53 adjacency cases hold no
/// colliding siblings, so none resolves `Ambiguous` under either line end
/// (the case test above compares path texts, which an ambiguous anchor
/// shares).
#[test]
fn adjacency_cases_never_resolve_ambiguous() {
    assert_eq!(ADJACENCY.len(), 53);
    for case in ADJACENCY {
        for eol in ["\n", "\r\n"] {
            let analysis = ron::analyze(&expand(case.source).replace('\n', eol));
            assert!(
                !analysis
                    .markers
                    .iter()
                    .any(|m| matches!(m.anchor, Anchor::Ambiguous { .. })),
                "{} {eol:?}: {:?}",
                case.name,
                analysis.markers
            );
            assert_eq!(analysis.colliding_groups, 0, "{} {eol:?}", case.name);
        }
    }
}
