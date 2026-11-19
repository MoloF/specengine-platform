//! docs/features/doc-library.md (RM-0127) "Acceptance criteria", DL3, the
//! core unit test: `decision_group` reads the convention's four words
//! alone. `accepted` and `rejected` exactly, a status starting
//! `superseded-by ` (the space included), and anything else, `draft`, `""`
//! and no status among them, `other`. M: `superseded-by …` read as
//! accepted.
//!
//! The rows, the decisions' order and their placement are the CLI's
//! `tests/library.rs` (through the binary, over the scene).

use specengine_core::library::{DecisionGroup, decision_group};

#[test]
fn dl3_decision_group_reads_the_conventions_four_words() {
    for (status, group) in [
        (Some("accepted"), "accepted"),
        (Some("superseded-by DEC-0023"), "superseded"),
        (Some("superseded-by ADR-0039"), "superseded"),
        (Some("rejected"), "rejected"),
        (Some("draft"), "other"),
        (Some(""), "other"),
        (None, "other"),
        (Some("Accepted"), "other"),
        (Some("accepted "), "other"),
        (Some(" accepted"), "other"),
        (Some("superseded"), "other"),
        (Some("superseded-by"), "other"),
        (Some("rejected-by DEC-0001"), "other"),
        (Some("\u{0430}ccepted"), "other"),
    ] {
        assert_eq!(decision_group(status).as_str(), group, "{status:?}");
    }
}

/// "Data": the groups in a row's order, accepted, superseded, rejected,
/// other.
#[test]
fn dl3_the_groups_order_as_a_row_lists_them() {
    let mut groups = [
        DecisionGroup::Other,
        DecisionGroup::Rejected,
        DecisionGroup::Superseded,
        DecisionGroup::Accepted,
    ];
    groups.sort();
    assert_eq!(
        groups
            .iter()
            .map(|group| group.as_str())
            .collect::<Vec<_>>(),
        ["accepted", "superseded", "rejected", "other"]
    );
}

/// DL7, iteration 3 (item E): `kept_tags`, the library's judgement with no
/// words, keeps exactly what `judge_tags` keeps: on DL7's 38 entries the
/// 32 of entries 5 and 7-37, and on edge lists alike (none, all bad, a tag
/// in another script, byte-equal repeats only, a case pair kept apart).
#[test]
fn dl7_kept_tags_are_judge_tags_kept() {
    use specengine_core::library::{judge_tags, kept_tags};
    let owned = |entries: &[&str]| -> Vec<String> {
        entries.iter().map(|entry| (*entry).to_owned()).collect()
    };
    let valid: Vec<String> = (1..=32).map(|n| format!("t{n:02}")).collect();
    let mut dl7 = owned(&["", "x ", "a\u{7}b", &"a".repeat(65), "ok", "ok"]);
    dl7.extend(valid.iter().cloned());
    assert_eq!(dl7.len(), 38);
    let mut want = vec!["ok".to_owned()];
    want.extend(valid[..31].iter().cloned());
    assert_eq!(kept_tags(&dl7), want);
    assert_eq!(judge_tags(&dl7).kept, want);
    assert_eq!(judge_tags(&dl7).problems.len(), 6);
    for entries in [
        owned(&[]),
        owned(&["", " ", "\t", "y\u{0}"]),
        owned(&["\u{0441}\u{0432}\u{0435}\u{0442}", "light"]),
        owned(&["Tide", "tide", "Tide"]),
        owned(&[&"b".repeat(64), &"b".repeat(65)]),
    ] {
        assert_eq!(
            kept_tags(&entries),
            judge_tags(&entries).kept,
            "{entries:?}"
        );
    }
    assert_eq!(
        kept_tags(&owned(&["Tide", "tide", "Tide"])),
        ["Tide", "tide"]
    );
    assert_eq!(
        kept_tags(&owned(&[&"b".repeat(64), &"b".repeat(65)])).len(),
        1
    );
}
