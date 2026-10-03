//! Ambiguous RON paths (`docs/canon/code-identity.md` "RON ambiguity";
//! docs/features/layer-a-identity.md AC-11, AC-12): a marker whose value or
//! ancestor shares its segment text with a sibling in the same struct or map
//! — cut at `MAX_SEGMENT_BYTES`, whitespace collapsed, or a field or key
//! written twice — is `Anchor::Ambiguous`, keeps its path text, is never
//! merged; a marker on the container itself stays `Path`. Every case runs
//! under LF and CRLF.

use specengine_code::ron::{self, Anchor, MAX_SEGMENT_BYTES, RonAnalysis, TRUNCATION_MARK};

/// `source` analysed under `\n` and `\r\n`; both must agree.
fn analyze(source: &str) -> RonAnalysis {
    let lf = ron::analyze(source);
    let crlf = ron::analyze(&source.replace('\n', "\r\n"));
    let shape = |a: &RonAnalysis| -> Vec<(String, Anchor)> {
        a.markers
            .iter()
            .map(|m| (m.marker.id.clone(), m.anchor.clone()))
            .collect()
    };
    assert_eq!(shape(&lf), shape(&crlf), "LF and CRLF differ: {source:?}");
    assert_eq!(lf.colliding_groups, crlf.colliding_groups);
    assert!(!lf.has_error, "{source:?}: {:?}", lf.error_categories);
    lf
}

fn ambiguous(path: &str, depth: usize) -> Anchor {
    Anchor::Ambiguous {
        path: path.to_owned(),
        depth,
    }
}

fn path(path: &str, depth: usize) -> Anchor {
    Anchor::Path {
        path: path.to_owned(),
        depth,
    }
}

fn anchors(analysis: &RonAnalysis) -> Vec<(&str, &Anchor)> {
    analysis
        .markers
        .iter()
        .map(|m| (m.marker.id.as_str(), &m.anchor))
        .collect()
}

/// A quoted key of `MAX_SEGMENT_BYTES + 2` source bytes: the shared prefix
/// fills the cap, `tail` lies past it.
fn long_key(tail: char) -> String {
    format!("\"{}{tail}\"", "k".repeat(MAX_SEGMENT_BYTES))
}

fn cut_key() -> String {
    let full = long_key('a');
    format!("{}{TRUNCATION_MARK}", &full[..MAX_SEGMENT_BYTES])
}

#[test]
fn keys_sharing_the_first_cap_bytes_are_both_ambiguous_with_one_path() {
    let source = format!(
        "{{\n    // @implements A@1\n    {}: 1,\n    // @implements B@1\n    {}: 2,\n}}\n",
        long_key('a'),
        long_key('b')
    );
    let analysis = analyze(&source);
    let expected = format!("root{{{}}}", cut_key());
    assert_eq!(
        anchors(&analysis),
        [
            ("A", &ambiguous(&expected, 1)),
            ("B", &ambiguous(&expected, 1))
        ],
        "both markers kept, never merged, same path text"
    );
    assert_eq!(analysis.colliding_groups, 1);
    assert_eq!(analysis.markers[0].anchor.as_str(), expected);
}

#[test]
fn a_marker_on_the_first_sibling_only_is_ambiguous() {
    let source = format!(
        "{{\n    // @implements A@1\n    {}: 1,\n    {}: 2,\n}}\n",
        long_key('a'),
        long_key('b')
    );
    let analysis = analyze(&source);
    assert_eq!(analysis.markers.len(), 1);
    assert!(
        matches!(analysis.markers[0].anchor, Anchor::Ambiguous { .. }),
        "decided when the map closes, not when the marker is read: {:?}",
        analysis.markers[0].anchor
    );
    // On the second only.
    let source = format!(
        "{{\n    {}: 1,\n    // @implements B@1\n    {}: 2,\n}}\n",
        long_key('a'),
        long_key('b')
    );
    assert!(matches!(
        analyze(&source).markers[0].anchor,
        Anchor::Ambiguous { .. }
    ));
}

#[test]
fn whitespace_collapsed_keys_collide() {
    let analysis = analyze(
        "{\n    \"fire  ice\": 1,\n    // @implements A@1\n    \"fire ice\": 2,\n    \"water\": 3, // @implements W@1\n}\n",
    );
    assert_eq!(
        anchors(&analysis),
        [
            ("A", &ambiguous("root{\"fire ice\"}", 1)),
            ("W", &path("root{\"water\"}", 1)),
        ]
    );
    assert_eq!(analysis.colliding_groups, 1);
    let tabs =
        analyze("{\n    // @implements A@1\n    \"fire\tice\": 1,\n    \"fire ice\": 2,\n}\n");
    assert!(matches!(tabs.markers[0].anchor, Anchor::Ambiguous { .. }));
}

#[test]
fn a_repeated_field_collides() {
    let analysis = analyze(
        "Config(\n    speed: 1, // @implements S1@1\n    speed: 2, // @implements S2@1\n    other: 3, // @implements O@1\n)\n",
    );
    assert_eq!(
        anchors(&analysis),
        [
            ("S1", &ambiguous("root.speed", 1)),
            ("S2", &ambiguous("root.speed", 1)),
            ("O", &path("root.other", 1)),
        ]
    );
    assert_eq!(analysis.colliding_groups, 1);
}

#[test]
fn a_lone_cut_key_and_keys_differing_within_the_cap_stay_paths() {
    let lone = format!(
        "{{\n    // @implements A@1\n    {}: 1,\n}}\n",
        long_key('a')
    );
    let analysis = analyze(&lone);
    assert_eq!(
        anchors(&analysis),
        [("A", &path(&format!("root{{{}}}", cut_key()), 1))],
        "truncation alone is no collision"
    );
    assert_eq!(analysis.colliding_groups, 0);

    // Differing at the last byte inside the cap.
    let a = format!("\"{}a\"", "k".repeat(MAX_SEGMENT_BYTES - 3));
    let b = format!("\"{}b\"", "k".repeat(MAX_SEGMENT_BYTES - 3));
    assert_eq!(a.len(), MAX_SEGMENT_BYTES);
    let analysis = analyze(&format!(
        "{{\n    // @implements A@1\n    {a}: 1,\n    // @implements B@1\n    {b}: 2,\n}}\n"
    ));
    assert_eq!(
        anchors(&analysis),
        [
            ("A", &path(&format!("root{{{a}}}"), 1)),
            ("B", &path(&format!("root{{{b}}}"), 1)),
        ]
    );
    assert_eq!(analysis.colliding_groups, 0);
}

#[test]
fn deep_under_a_colliding_sibling_is_ambiguous_on_the_map_it_is_not() {
    let source = "Config(\n    // @implements MAP@1\n    deep: {\n        \"k\": Inner(\n            // @implements DEEP@1\n            x: 1,\n            list: [\n                // @implements EL@1\n                5,\n            ],\n        ),\n        \"k\": 2,\n        \"other\": Inner(\n            // @implements CLEAR@1\n            x: 1,\n        ),\n    },\n)\n";
    let analysis = analyze(source);
    assert_eq!(
        anchors(&analysis),
        [
            ("MAP", &path("root.deep", 1)),
            ("DEEP", &ambiguous("root.deep{\"k\"}.x", 3)),
            ("EL", &ambiguous("root.deep{\"k\"}.list[0]", 4)),
            ("CLEAR", &path("root.deep{\"other\"}.x", 3)),
        ]
    );
    assert_eq!(analysis.colliding_groups, 1);
    // An ambiguous anchor at depth >= 2 is no nested anchor.
    let only_ambiguous = analyze(
        "Config(\n    deep: {\n        \"k\": Inner(\n            // @implements DEEP@1\n            x: 1,\n        ),\n        \"k\": 2,\n    },\n)\n",
    );
    assert!(!only_ambiguous.has_nested_anchor());
}

#[test]
fn the_same_name_in_different_containers_is_no_collision() {
    let analysis = analyze(
        "Config(\n    a: A(\n        // @implements X1@1\n        x: 1,\n    ),\n    b: B(\n        // @implements X2@1\n        x: 2,\n    ),\n    list: [\n        // @implements L0@1\n        1,\n        // @implements L1@1\n        1,\n    ],\n    pair: (\n        1, // @implements T0@1\n        1, // @implements T1@1\n    ),\n)\n",
    );
    assert!(
        analysis
            .markers
            .iter()
            .all(|m| matches!(m.anchor, Anchor::Path { .. })),
        "{:?}",
        anchors(&analysis)
    );
    assert_eq!(analysis.colliding_groups, 0);
}

#[test]
fn colliding_groups_count_each_group_once_marked_or_not() {
    let analysis = analyze(
        "Config(\n    a: 1,\n    a: 2,\n    a: 3,\n    b: {\n        \"x\": 1,\n        \"x\": 2,\n        \"y\": 3,\n    },\n    c: C(\n        z: 1,\n        z: 2,\n    ),\n)\n",
    );
    assert!(analysis.markers.is_empty());
    assert_eq!(analysis.colliding_groups, 3);
}

#[test]
fn end_of_input_closes_the_rest() {
    // Unbalanced: the root struct never closes; its colliding fields are
    // still decided.
    let source = "Config(\n    // @implements A@1\n    a: 1,\n    a: 2,\n";
    let analysis = ron::analyze(source);
    assert!(analysis.has_error);
    assert_eq!(analysis.markers.len(), 1);
    assert!(
        matches!(analysis.markers[0].anchor, Anchor::Ambiguous { .. }),
        "{:?}",
        analysis.markers[0].anchor
    );
    assert_eq!(analysis.colliding_groups, 1);
}

#[test]
fn ambiguous_is_not_path_and_keeps_the_path_text() {
    let analysis = analyze("Config(\n    v: 1, // @implements A@1\n    v: 2,\n)\n");
    let anchor = &analysis.markers[0].anchor;
    assert!(!matches!(anchor, Anchor::Path { .. }));
    assert_eq!(anchor, &ambiguous("root.v", 1));
    assert_eq!(anchor.as_str(), "root.v");
}

#[test]
fn trailing_and_opener_markers_follow_their_entry() {
    let analysis = analyze(
        "Config(\n    player: Player( // @implements OPEN@1\n        hp: 1,\n    ), // @implements CLOSE@1\n    player: 2,\n)\n",
    );
    assert_eq!(
        anchors(&analysis),
        [
            ("OPEN", &ambiguous("root.player", 1)),
            ("CLOSE", &ambiguous("root.player", 1)),
        ]
    );
}

#[test]
fn analysis_with_collisions_is_deterministic() {
    let source = format!(
        "{{\n    // @implements A@1\n    {}: 1,\n    // @implements B@1\n    {}: 2,\n}}\n",
        long_key('a'),
        long_key('b')
    );
    assert_eq!(ron::analyze(&source), ron::analyze(&source));
}
