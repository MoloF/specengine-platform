//! Task spec `code-refs` "States" (iteration 2 of its review): a marker's
//! token ends at the first whitespace, `[`, `*/`, line end or `@` that
//! begins a keyword; an `@` that begins none stays in the token (its
//! revision as written). So `@implements @verifies X` and
//! `@implements X@@verifies Y` hold two markers each, and an empty ID never
//! panics, whatever follows it.
//!
//! Named mutations: drop the keyword stop (a token no longer ends at an
//! `@` beginning a keyword) turns `a_keyword_ends_the_token_before_it`
//! red; drop the `token_end > cursor` guard turns
//! `an_empty_id_never_panics` red (a panic).

use specengine_code::{Marker, Relation, markers_in, ron};

/// `(relation, id, token, rev, note)` of one marker.
type Parts = (Relation, String, String, Option<u32>, Option<String>);

/// The [`Parts`] of every marker of `comment`.
fn parts(comment: &str) -> Vec<Parts> {
    markers_in(comment)
        .into_iter()
        .map(|marker: Marker| {
            let token = marker.token(comment).to_owned();
            (marker.relation, marker.id, token, marker.rev, marker.note)
        })
        .collect()
}

fn one(relation: Relation, id: &str, token: &str, rev: Option<u32>, note: Option<&str>) -> Parts {
    (
        relation,
        id.to_owned(),
        token.to_owned(),
        rev,
        note.map(str::to_owned),
    )
}

/// M: drop the keyword stop — the empty first token runs over
/// `@verifies` and the second marker is lost.
#[test]
fn a_keyword_ends_the_token_before_it() {
    for comment in [
        "// @implements @verifies GOAL-01",
        "/* @implements @verifies GOAL-01 */",
        "//! @implements @verifies GOAL-01",
    ] {
        assert_eq!(
            parts(comment),
            [
                one(Relation::Implements, "", "", None, None),
                one(Relation::Verifies, "GOAL-01", "GOAL-01", None, None),
            ],
            "{comment}"
        );
    }
}

/// `X@@verifies Y`: the first token is `X@` (a malformed revision: no rev,
/// no note), the second marker stands.
#[test]
fn a_glued_keyword_after_an_at_is_a_second_marker() {
    assert_eq!(
        parts("// @implements GOAL-02@@verifies GOAL-01 [body] the note"),
        [
            one(Relation::Implements, "GOAL-02", "GOAL-02@", None, None),
            one(
                Relation::Verifies,
                "GOAL-01",
                "GOAL-01",
                None,
                Some("the note")
            ),
        ]
    );
    assert_eq!(
        parts("// @implements GOAL-02@3@verifies GOAL-01@2"),
        [
            one(Relation::Implements, "GOAL-02", "GOAL-02@3", Some(3), None),
            one(Relation::Verifies, "GOAL-01", "GOAL-01@2", Some(2), None),
        ]
    );
}

/// An `@` that begins no keyword stays in the token: the revision as
/// written, read only as 1-9 digits ending the token.
#[test]
fn an_at_beginning_no_keyword_stays_in_the_token() {
    for (comment, token, rev, note) in [
        ("// @implements X-1@3", "X-1@3", Some(3), None),
        ("// @implements X-1@3a later", "X-1@3a", None, Some("later")),
        ("// @implements X-1@", "X-1@", None, None),
        (
            "// @implements X-1@1234567890",
            "X-1@1234567890",
            None,
            None,
        ),
        (
            "// @implements X-1@123456789",
            "X-1@123456789",
            Some(123_456_789),
            None,
        ),
        (
            "// @implements X-1@user@example",
            "X-1@user@example",
            None,
            None,
        ),
        (
            "// @implements X-1@implementsy",
            "X-1@implementsy",
            None,
            None,
        ),
    ] {
        assert_eq!(
            parts(comment),
            [one(Relation::Implements, "X-1", token, rev, note)],
            "{comment}"
        );
    }
}

/// M: drop the `token_end > cursor` guard — an empty ID right before a
/// keyword slices backwards and panics. Probes, then every sequence of up
/// to four pieces of an alphabet of keywords, `@`, IDs, digits, blanks,
/// line ends, brackets and `*/`, each marker's token read as well.
#[test]
fn an_empty_id_never_panics() {
    for probe in [
        "// @implements @3",
        "// @implements @verifies X-1",
        "// @implements @@verifies X-1",
        "/* @implements @*/",
        "@implements @",
        "@implements",
        "@implements\n@verifies",
        "@implements@verifies X",
        "// @assumes @configures",
    ] {
        for marker in markers_in(probe) {
            let range = marker.token_range(probe);
            assert!(
                range.start <= range.end && range.end <= probe.len(),
                "{probe:?}"
            );
            let _ = marker.token(probe);
        }
    }
    let pieces = [
        "@implements",
        "@verifies",
        "@",
        "X-1",
        "3",
        " ",
        "\n",
        "[",
        "]",
        "*/",
        "a",
        "@@",
    ];
    let mut checked = 0usize;
    let mut stack: Vec<(String, usize)> = vec![(String::new(), 0)];
    while let Some((text, count)) = stack.pop() {
        for marker in markers_in(&text) {
            let range = marker.token_range(&text);
            assert!(
                range.start <= range.end && range.end <= text.len(),
                "{text:?}"
            );
            let _ = marker.token(&text);
        }
        checked += 1;
        if count < 4 {
            for piece in pieces {
                stack.push((format!("{text}{piece}"), count + 1));
            }
        }
    }
    assert_eq!(
        checked,
        1 + 12 + 144 + 1728 + 20_736,
        "every sequence of up to four pieces"
    );
}

/// The RON path reads the same markers: two in one comment, both bound
/// to the value below.
#[test]
fn ron_comments_see_both_markers() {
    let analysis = ron::analyze(
        "Config(\n    // @implements @verifies GOAL-01\n    speed: 4.5,\n    // @implements X-1@@verifies Y-2\n    hp: 1,\n)\n",
    );
    let got: Vec<(String, String, String)> = analysis
        .markers
        .iter()
        .map(|m| {
            (
                m.marker.relation.as_str().to_owned(),
                m.marker.id.clone(),
                m.anchor.as_str().to_owned(),
            )
        })
        .collect();
    let want = [
        ("implements", "", "root.speed"),
        ("verifies", "GOAL-01", "root.speed"),
        ("implements", "X-1", "root.hp"),
        ("verifies", "Y-2", "root.hp"),
    ];
    assert_eq!(
        got,
        want.map(|(r, i, a)| (r.to_owned(), i.to_owned(), a.to_owned()))
    );
}

// ------------------------------------- the review's exact cases (iteration 2)

/// Case (1). Red under both mutations: drop the keyword stop (one marker)
/// and drop `token_end > cursor` (a backwards slice, a panic). Its
/// binding to the next `fn`: `code_bind.rs`
/// `two_markers_in_one_comment_bind_the_next_fn`.
#[test]
fn case_1_an_empty_id_before_a_keyword_is_two_markers() {
    assert_eq!(
        parts("// @implements @verifies A-1"),
        [
            one(Relation::Implements, "", "", None, None),
            one(Relation::Verifies, "A-1", "A-1", None, None),
        ]
    );
}

/// Case (2): `B-2@` (rev none, note none) and `A-1`. Red under drop the
/// keyword stop (the first token swallows `@verifies`).
#[test]
fn case_2_a_keyword_glued_after_an_at_is_a_second_marker() {
    assert_eq!(
        parts("// @implements B-2@@verifies A-1"),
        [
            one(Relation::Implements, "B-2", "B-2@", None, None),
            one(Relation::Verifies, "A-1", "A-1", None, None),
        ]
    );
}

/// Case (3): `B-2` unversioned (its token is the ID alone) and
/// `verifies A-1`. Red under drop the keyword stop (one marker, `B-2@verifies`
/// a malformed revision) and under drop `token_end > cursor` (the token
/// ends where the ID does: a panic).
#[test]
fn case_3_a_keyword_glued_to_an_id_is_a_second_marker() {
    let comment = "// @implements B-2@verifies A-1";
    assert_eq!(
        parts(comment),
        [
            one(Relation::Implements, "B-2", "B-2", None, None),
            one(Relation::Verifies, "A-1", "A-1", None, None),
        ]
    );
    let first = &markers_in(comment)[0];
    assert_eq!(first.token(comment).len(), first.id.len(), "unversioned");
}

/// Case (4): an empty ID with `@3` keeps rev 3, as at HEAD, and does not
/// panic. No named mutation turns it red; it pins the behaviour kept.
#[test]
fn case_4_an_empty_id_with_a_revision_keeps_it() {
    assert_eq!(
        parts("// @implements @3"),
        [one(Relation::Implements, "", "@3", Some(3), None)]
    );
}
