//! Marker grammar (`docs/canon/code-identity.md` "Marker grammar";
//! docs/features/layer-a-identity.md AC-07 levels, AC-08 level-list errors,
//! AC-09 ID boundaries): every case under `\n` and `\r\n` line ends, through
//! `markers_in` and through a `.rs` / `.ron` comment as the walkers see it.
//!
//! Timing compares a 1 MB level list with a 1 MB note in the same process,
//! with a generous factor (a debug build on a busy laptop stays green, a
//! quadratic scan does not).

use std::time::{Duration, Instant};

use specengine_code::markers::DEFAULT_LEVELS;
use specengine_code::ron::{self, Anchor};
use specengine_code::{Level, LevelError, Levels, Marker, Relation, markers_in};

/// `text` as written and with every `\n` turned into `\r\n`, each ending in
/// a line end so the end-of-line handling is exercised.
fn variants(text: &str) -> [(String, &'static str); 2] {
    let lf = format!("{text}\n");
    let crlf = lf.replace('\n', "\r\n");
    [(lf, "LF"), (crlf, "CRLF")]
}

/// The single marker of `text`, checked to be the same under LF and CRLF.
fn one(text: &str) -> Marker {
    let mut seen: Option<Marker> = None;
    for (source, ending) in variants(text) {
        let found = markers_in(&source);
        assert_eq!(found.len(), 1, "{text:?} [{ending}]: {found:?}");
        let mut marker = found.into_iter().next().unwrap();
        marker.offset = 0;
        if let Some(previous) = &seen {
            assert_eq!(&marker, previous, "{text:?}: LF and CRLF differ");
        }
        seen = Some(marker);
    }
    seen.unwrap()
}

/// Every marker of `text`, checked to be the same under LF and CRLF
/// (offsets aside).
fn all(text: &str) -> Vec<Marker> {
    let [(lf, _), (crlf, _)] = variants(text);
    let strip = |mut markers: Vec<Marker>| {
        for marker in &mut markers {
            marker.offset = 0;
        }
        markers
    };
    let lf = strip(markers_in(&lf));
    assert_eq!(lf, strip(markers_in(&crlf)), "{text:?}: LF and CRLF differ");
    lf
}

fn declared(levels: &[Level]) -> Levels {
    Levels::Declared(levels.to_vec())
}

fn invalid(error: LevelError) -> Levels {
    Levels::Invalid(error)
}

// ------------------------------------------------------------------ AC-07

#[test]
fn level_list_after_id_and_rev() {
    let m = one("// @implements X@3 [sig]");
    assert_eq!(m.relation, Relation::Implements);
    assert_eq!(m.id, "X");
    assert_eq!(m.rev, Some(3));
    assert_eq!(m.levels, declared(&[Level::Sig]));
    assert_eq!(m.note, None);
    assert!(m.id_latin);
}

#[test]
fn levels_are_kept_in_canonical_order() {
    assert_eq!(
        one("// @implements X [body, sig]").levels,
        declared(&[Level::Sig, Level::Body])
    );
    assert_eq!(
        one("// @implements X [deps, body, path, sig]").levels,
        declared(&[Level::Path, Level::Sig, Level::Body, Level::Deps])
    );
    assert_eq!(
        one("// @implements X [path, sig, body, deps]").levels,
        declared(&Level::ALL)
    );
}

#[test]
fn no_list_is_the_default_told_apart_from_a_declared_one() {
    let m = one("// @implements X@2");
    assert_eq!(m.levels, Levels::Default);
    assert_eq!(m.rev, Some(2));
    assert_eq!(m.note, None);
    assert_eq!(m.levels.effective(), Some(&DEFAULT_LEVELS[..]));
    assert_eq!(m.levels.state(), "default");
    let written = one("// @implements X@2 [sig, body]");
    assert_eq!(written.levels.effective(), Some(&DEFAULT_LEVELS[..]));
    assert_eq!(written.levels.state(), "declared");
    assert_ne!(written.levels, m.levels);
}

#[test]
fn glued_list_ends_the_id() {
    for text in ["// @implements X@3[sig]", "// @implements X[sig]"] {
        let m = one(text);
        assert_eq!(m.id, "X", "{text}");
        assert!(m.id_latin, "{text}");
        assert_eq!(m.levels, declared(&[Level::Sig]), "{text}");
        assert_eq!(m.note, None, "{text}");
    }
    assert_eq!(one("// @implements X@3[sig]").rev, Some(3));
    assert_eq!(one("// @implements X[sig]").rev, None);
}

#[test]
fn note_follows_the_list_and_a_later_bracket_is_note_text() {
    let m = one("// @implements X [sig] mutation: \"y\"");
    assert_eq!(m.levels, declared(&[Level::Sig]));
    assert_eq!(m.note.as_deref(), Some("mutation: \"y\""));

    let m = one("// @implements X see [docs]");
    assert_eq!(m.levels, Levels::Default);
    assert_eq!(m.note.as_deref(), Some("see [docs]"));

    let m = one("// @implements X [sig] see [docs]");
    assert_eq!(m.levels, declared(&[Level::Sig]));
    assert_eq!(m.note.as_deref(), Some("see [docs]"));
}

#[test]
fn several_markers_keep_their_own_lists_and_notes() {
    let found = all("// @implements A [sig] @verifies B [body] n");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].id, "A");
    assert_eq!(found[0].levels, declared(&[Level::Sig]));
    assert_eq!(found[0].note, None);
    assert_eq!(found[1].relation, Relation::Verifies);
    assert_eq!(found[1].id, "B");
    assert_eq!(found[1].levels, declared(&[Level::Body]));
    assert_eq!(found[1].note.as_deref(), Some("n"));

    // Across the lines of one block comment.
    let found = all("/* @implements A [sig]\n   @verifies B@4 [deps] why\n*/");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].levels, declared(&[Level::Sig]));
    assert_eq!(found[0].note, None);
    assert_eq!(found[1].rev, Some(4));
    assert_eq!(found[1].levels, declared(&[Level::Deps]));
    assert_eq!(found[1].note.as_deref(), Some("why"));
}

#[test]
fn block_comment_close_is_not_a_note() {
    let m = one("/* @configures C [sig] */");
    assert_eq!(m.relation, Relation::Configures);
    assert_eq!(m.id, "C");
    assert_eq!(m.levels, declared(&[Level::Sig]));
    assert_eq!(m.note, None);
}

#[test]
fn trailing_comma_and_blanks_are_tolerated() {
    for text in [
        "// @implements X [sig, body,]",
        "// @implements X [ sig , body , ]",
        "// @implements X\t[\tsig,\tbody\t]",
        "// @implements X [sig ,body]",
    ] {
        assert_eq!(
            one(text).levels,
            declared(&[Level::Sig, Level::Body]),
            "{text:?}"
        );
    }
}

// ------------------------------------------------------------------ AC-08

#[test]
fn malformed_lists_carry_their_category() {
    let cases: [(&str, LevelError); 12] = [
        ("[]", LevelError::Empty),
        ("[ ]", LevelError::Empty),
        ("[\t]", LevelError::Empty),
        ("[sig, sig]", LevelError::Duplicate),
        ("[sgi]", LevelError::Unknown),
        ("[Sig]", LevelError::Unknown),
        ("[sig body]", LevelError::Unknown),
        ("[sig,,body]", LevelError::Unknown),
        ("[,]", LevelError::Unknown),
        ("[sig, sig, x]", LevelError::Unknown),
        ("[,sig]", LevelError::Unknown),
        ("[sig,,]", LevelError::Unknown),
    ];
    let mut failures = Vec::new();
    for (list, error) in cases {
        let text = format!("// @implements X-1@7 {list} keep");
        let m = one(&text);
        if m.levels != invalid(error) || m.id != "X-1" || m.rev != Some(7) {
            failures.push(format!("{text:?}: {m:?}"));
        }
        if m.note.as_deref() != Some("keep") {
            failures.push(format!("{text:?}: the note must survive, got {:?}", m.note));
        }
        if m.levels.effective().is_some() || m.levels.state() != error.as_str() {
            failures.push(format!("{text:?}: invalid is never default or declared"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn unclosed_list_stops_at_line_end_block_close_or_next_keyword() {
    let m = one("// @implements X@1 [sig");
    assert_eq!(m.levels, invalid(LevelError::Unclosed));
    assert_eq!(
        (m.id.as_str(), m.rev, m.note.as_deref()),
        ("X", Some(1), None)
    );

    let m = one("/* @implements X@1 [sig */");
    assert_eq!(m.levels, invalid(LevelError::Unclosed));
    assert_eq!(m.note, None);

    // The `]` on the next line does not close it.
    let found = all("/* @implements X [sig\n   body] */");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].levels, invalid(LevelError::Unclosed));
    assert_eq!(found[0].note, None);

    let found = all("// @implements X@2 [sig @verifies Y@3 [body] n");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].levels, invalid(LevelError::Unclosed));
    assert_eq!((found[0].id.as_str(), found[0].rev), ("X", Some(2)));
    assert_eq!(found[0].note, None);
    assert_eq!(found[1].id, "Y");
    assert_eq!(found[1].rev, Some(3));
    assert_eq!(found[1].levels, declared(&[Level::Body]));
    assert_eq!(found[1].note.as_deref(), Some("n"));
}

#[test]
fn an_invalid_list_leaves_the_next_marker_intact() {
    for list in ["[]", "[sig, sig]", "[sgi]", "[sig"] {
        let text = format!("/* @implements A@2 {list}\n   @verifies B@1 [path] ok */");
        let found = all(&text);
        assert_eq!(found.len(), 2, "{text:?}: {found:?}");
        assert!(
            matches!(found[0].levels, Levels::Invalid(_)),
            "{text:?}: {found:?}"
        );
        assert_eq!((found[0].id.as_str(), found[0].rev), ("A", Some(2)));
        assert_eq!(found[1].id, "B");
        assert_eq!(found[1].rev, Some(1));
        assert_eq!(found[1].levels, declared(&[Level::Path]));
        assert_eq!(found[1].note.as_deref(), Some("ok"));
    }
}

#[test]
fn the_error_holds_no_text_from_the_file() {
    let m = one("// @implements X [secretword]");
    assert_eq!(m.levels, invalid(LevelError::Unknown));
    let shown = format!("{:?} {}", m.levels, m.levels.state());
    assert!(!shown.contains("secretword"), "{shown}");
    let names: Vec<&str> = [
        LevelError::Empty,
        LevelError::Duplicate,
        LevelError::Unknown,
        LevelError::Unclosed,
    ]
    .iter()
    .map(|e| e.as_str())
    .collect();
    assert_eq!(names, ["empty", "duplicate", "unknown", "unclosed"]);
}

/// `slow` costs the same order as `fast`: at most `factor` times it plus a
/// fixed allowance for scheduler noise.
fn assert_same_order(what: &str, slow: Duration, fast: Duration, factor: u32) {
    let bound = fast * factor + Duration::from_millis(250);
    eprintln!("{what}: {slow:?} against {fast:?}, bound {bound:?}");
    assert!(
        slow <= bound,
        "{what}: {slow:?} against {fast:?} (bound {bound:?}) — the scan is not linear"
    );
}

fn best_of_three(text: &str) -> (Vec<Marker>, Duration) {
    let mut best = Duration::MAX;
    let mut result = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        result = markers_in(text);
        best = best.min(start.elapsed());
    }
    (result, best)
}

#[test]
fn megabyte_level_list_costs_the_order_of_a_megabyte_note() {
    const MB: usize = 1024 * 1024;
    let items = "sig, ".repeat(MB / 5);
    let list = format!("// @implements X@1 [{items}x]\n");
    let unclosed = format!("// @implements X@1 [{items}\n");
    let note = format!("// @implements X@1 {}\n", "n".repeat(MB));
    let (note_markers, note_time) = best_of_three(&note);
    assert_eq!(note_markers.len(), 1);
    assert_eq!(note_markers[0].note.as_ref().map(String::len), Some(MB));

    let (markers, list_time) = best_of_three(&list);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].levels, invalid(LevelError::Unknown));
    assert_eq!((markers[0].id.as_str(), markers[0].rev), ("X", Some(1)));
    assert_same_order("1 MB level list", list_time, note_time, 10);

    let (markers, unclosed_time) = best_of_three(&unclosed);
    assert_eq!(markers[0].levels, invalid(LevelError::Unclosed));
    assert_same_order("1 MB unclosed list", unclosed_time, note_time, 10);

    // Many markers on one long line: each line end is looked for once.
    let crowded = format!("// {}\n", "@implements X [sig ".repeat(MB / 19));
    let (markers, crowded_time) = best_of_three(&crowded);
    assert_eq!(markers.len(), MB / 19);
    assert!(
        markers
            .iter()
            .all(|m| m.levels == invalid(LevelError::Unclosed))
    );
    assert_same_order("1 MB of unclosed markers", crowded_time, note_time, 10);
}

// ------------------------------------------------------------------ AC-09

#[test]
fn block_close_ends_the_id() {
    let m = one("/* @implements X*/");
    assert_eq!(m.id, "X");
    assert!(m.id_latin);
    assert_eq!(m.note, None);
    let m = one("/* @implements X@2*/");
    assert_eq!((m.id.as_str(), m.rev), ("X", Some(2)));
}

#[test]
fn slug_slash_id_is_latin() {
    let m = one("// @verifies feat/AC-07@2");
    assert_eq!(m.relation, Relation::Verifies);
    assert_eq!(m.id, "feat/AC-07");
    assert_eq!(m.rev, Some(2));
    assert!(m.id_latin);
    let m = one("// @verifies spec.v2:REQ_1-a/b");
    assert!(m.id_latin, "{m:?}");
}

#[test]
fn keyword_at_line_end_is_an_empty_latin_id() {
    let m = one("// @implements");
    assert_eq!(m.id, "");
    assert!(m.id_latin, "an empty ID is counted apart, never non-Latin");
    assert_eq!(m.rev, None);
    assert_eq!(m.levels, Levels::Default);
    assert_eq!(m.note, None);
    let m = one("// @implements   ");
    assert_eq!(m.id, "");
    // The ID never reaches into the next line.
    let found = all("/* @implements\n   X@1 */");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "");
}

#[test]
fn non_latin_id_is_flagged() {
    // `X` followed by a Cyrillic look-alike of `x`.
    let m = one("// @implements X\u{0445}-1@1 [sig]");
    assert_eq!(m.id, "X\u{0445}-1");
    assert!(!m.id_latin);
    assert_eq!(m.rev, Some(1));
    assert_eq!(m.levels, declared(&[Level::Sig]));
    let m = one("// @implements \u{0425}");
    assert!(!m.id_latin);
}

// --------------------------------------------- through the language walkers

/// `(id, rev, levels, note, anchor)` of one RON marker.
type RonRow<'a> = (&'a str, Option<u32>, &'a Levels, Option<&'a str>, &'a str);

#[test]
fn ron_comments_carry_levels_under_lf_and_crlf() {
    let source = "Config(\n    // @implements A@1 [body, sig]\n    a: 1,\n    b: 2, // @implements B@2 [sgi] n\n    /* @implements C@3 [path */\n    c: 3,\n    // @implements\n    d: 4,\n)\n";
    for (text, ending) in [
        (source.to_owned(), "LF"),
        (source.replace('\n', "\r\n"), "CRLF"),
    ] {
        let analysis = ron::analyze(&text);
        assert!(!analysis.has_error, "[{ending}] {analysis:?}");
        let got: Vec<RonRow<'_>> = analysis
            .markers
            .iter()
            .map(|m| {
                (
                    m.marker.id.as_str(),
                    m.marker.rev,
                    &m.marker.levels,
                    m.marker.note.as_deref(),
                    m.anchor.as_str(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    "A",
                    Some(1),
                    &declared(&[Level::Sig, Level::Body]),
                    None,
                    "root.a"
                ),
                (
                    "B",
                    Some(2),
                    &invalid(LevelError::Unknown),
                    Some("n"),
                    "root.b"
                ),
                ("C", Some(3), &invalid(LevelError::Unclosed), None, "root.c"),
                ("", None, &Levels::Default, None, "root.d"),
            ],
            "[{ending}]"
        );
        assert!(
            analysis
                .markers
                .iter()
                .all(|m| matches!(m.anchor, Anchor::Path { .. }))
        );
    }
}

/// The comment texts of a Rust source as tree-sitter delimits them.
fn rust_comment_texts(source: &str) -> Vec<String> {
    let tree = specengine_code::RustParser::new()
        .expect("grammar loads")
        .parse(source)
        .expect("parse completes");
    let mut texts = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "line_comment" | "block_comment") {
            texts.push(source[node.byte_range()].to_owned());
            continue;
        }
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    texts
}

#[test]
fn rust_comments_carry_levels_under_lf_and_crlf() {
    let source = "// @implements A@1 [sig]\nfn a() {}\n\n/// @verifies B [body] doc\nfn b() {}\n/* @implements C [path\n */\nfn c() {}\n// @implements D@2 [sgi]\nfn d() {}\n";
    for (text, ending) in [
        (source.to_owned(), "LF"),
        (source.replace('\n', "\r\n"), "CRLF"),
    ] {
        let comments = rust_comment_texts(&text);
        assert_eq!(comments.len(), 4, "[{ending}] {comments:?}");
        let markers: Vec<Marker> = comments
            .iter()
            .flat_map(|comment| markers_in(comment))
            .collect();
        assert_eq!(markers.len(), 4, "[{ending}] {markers:?}");
        assert_eq!(markers[0].levels, declared(&[Level::Sig]), "[{ending}]");
        assert_eq!(markers[0].note, None, "[{ending}]");
        assert_eq!(markers[1].levels, declared(&[Level::Body]), "[{ending}]");
        assert_eq!(markers[1].note.as_deref(), Some("doc"), "[{ending}]");
        assert_eq!(
            markers[2].levels,
            invalid(LevelError::Unclosed),
            "[{ending}]"
        );
        assert_eq!(markers[2].note, None, "[{ending}]");
        assert_eq!(
            markers[3].levels,
            invalid(LevelError::Unknown),
            "[{ending}]"
        );
        assert_eq!((markers[3].id.as_str(), markers[3].rev), ("D", Some(2)));
        assert_eq!(markers[3].note, None, "[{ending}]");
    }
}
