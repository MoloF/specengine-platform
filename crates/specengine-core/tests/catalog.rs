//! docs/features/direction-catalog.md (RM-0106) "Acceptance criteria", the
//! pure half of DC-3 and DC-4 in `specengine_core::catalog`:
//!
//! - `last_change` reads one commit as node-history's commit entry: `at`
//!   the committer time in UTC; `by` the `Decided-by:` trailer as written
//!   when the apply trailers read, else `<author_name> <<author_email>>`
//!   (an apply with no `Decided-by:` too); `by_name` its name part;
//!   `commit` the OID; `proposal` the `Proposal:` trailer, `None` on a hand
//!   commit (a `Proposal:` line outside the last paragraph is none);
//!   `of_file` as passed. Each case is checked against core's own
//!   `history::applied_trailers` and `identity_name`, the entry's sources.
//! - `open_items_by_node`: an item counts once per node it names, as
//!   target or in `node_ids`, both or repeated; a node no item names is no
//!   key. M: an item naming a node twice counted twice.
//! - `LAST_CHANGE_SCAN_MAX` is 20 000.

use std::collections::BTreeMap;

use specengine_core::catalog::{
    CommitFacts, LAST_CHANGE_SCAN_MAX, LastChange, OpenItem, last_change, open_items_by_node,
};
use specengine_core::history::{applied_trailers, identity_name};
use specengine_core::proposal::{
    self, ApplyTrailers, Author, AuthorType, commit_message_raising, utc_timestamp,
};

const OID: &str = "0123456789abcdef0123456789abcdef01234567";
const OID_256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// 2026-01-02T10:00:00Z.
const AT: i64 = 1_767_348_000;

fn writer() -> Author {
    Author {
        author_type: AuthorType::Agent,
        role: Some("spec-writer".to_owned()),
        model: Some("m".to_owned()),
        run: Some("r".to_owned()),
        login: None,
    }
}

/// An apply's message, as an apply writes it.
fn apply(id: &str, rationale: &str, decided_by: &str) -> String {
    commit_message_raising(
        &proposal::CommitFacts {
            id,
            rationale,
            decided_by,
            author: &writer(),
            base_commit: OID,
        },
        None,
        &ApplyTrailers::default(),
    )
}

fn facts<'a>(oid: &'a str, message: &'a str) -> CommitFacts<'a> {
    CommitFacts {
        oid,
        author_name: "Bob Hand",
        author_email: "bob@example.org",
        committed_at: AT,
        message,
    }
}

/// What node-history's entry holds for this commit, built from core's own
/// trailer reading.
fn entry_of(commit: &CommitFacts<'_>, of_file: bool) -> LastChange {
    let applied = applied_trailers(commit.message);
    let by = applied
        .as_ref()
        .and_then(|applied| applied.decided_by.clone())
        .unwrap_or_else(|| format!("{} <{}>", commit.author_name, commit.author_email));
    LastChange {
        at: utc_timestamp(commit.committed_at),
        by_name: identity_name(&by).to_owned(),
        by,
        commit: commit.oid.to_owned(),
        proposal: applied.map(|applied| applied.proposal),
        of_file,
    }
}

#[test]
fn dc4_a_hand_commit_is_its_author() {
    let message = "Reword the storm\n";
    let commit = facts(OID, message);
    let change = last_change(&commit, false);
    assert_eq!(
        change,
        LastChange {
            at: "2026-01-02T10:00:00Z".to_owned(),
            by: "Bob Hand <bob@example.org>".to_owned(),
            by_name: "Bob Hand".to_owned(),
            commit: OID.to_owned(),
            proposal: None,
            of_file: false,
        }
    );
    assert_eq!(change, entry_of(&commit, false));
}

#[test]
fn dc4_an_apply_is_its_decider_and_its_proposal() {
    let message = apply(
        "PR-0001",
        "Name the shingle wet.",
        "Ann Owner <ann@example.org>",
    );
    let commit = facts(OID_256, &message);
    for of_file in [false, true] {
        let change = last_change(&commit, of_file);
        assert_eq!(
            change,
            LastChange {
                at: "2026-01-02T10:00:00Z".to_owned(),
                by: "Ann Owner <ann@example.org>".to_owned(),
                by_name: "Ann Owner".to_owned(),
                commit: OID_256.to_owned(),
                proposal: Some("PR-0001".to_owned()),
                of_file,
            }
        );
        assert_eq!(change, entry_of(&commit, of_file));
    }
}

#[test]
fn dc4_an_apply_with_no_decider_is_its_author() {
    let message = "spec: apply PR-0008\n\nAdd the signal.\n\nProposal: PR-0008\nBase-commit: 0\n";
    let commit = facts(OID, message);
    let change = last_change(&commit, false);
    assert_eq!(change.by, "Bob Hand <bob@example.org>");
    assert_eq!(change.by_name, "Bob Hand");
    assert_eq!(change.proposal.as_deref(), Some("PR-0008"));
    assert_eq!(change, entry_of(&commit, false));
}

#[test]
fn dc4_a_proposal_line_outside_the_trailers_is_a_hand_commit() {
    for message in [
        // In the rationale, not the last paragraph.
        "spec: apply PR-0002\n\nProposal: PR-0002\n\nCo-Authored-By: Eve <eve@example.org>\n",
        // No apply subject.
        "Reword\n\nProposal: PR-0002\nDecided-by: Ann Owner <ann@example.org>\n",
        // Another ID than the subject's.
        "spec: apply PR-0002\n\nWhy.\n\nProposal: PR-0003\nDecided-by: Ann Owner \
         <ann@example.org>\n",
    ] {
        let commit = facts(OID, message);
        let change = last_change(&commit, false);
        assert_eq!(change.proposal, None, "{message:?}");
        assert_eq!(change.by, "Bob Hand <bob@example.org>", "{message:?}");
        assert_eq!(change, entry_of(&commit, false), "{message:?}");
    }
}

#[test]
fn dc4_the_time_is_utc_and_a_bare_decider_is_its_own_name() {
    let message = apply("PR-0004", "Why.", "the owner");
    let commit = CommitFacts {
        committed_at: 0,
        ..facts(OID, &message)
    };
    let change = last_change(&commit, false);
    assert_eq!(change.at, "1970-01-01T00:00:00Z");
    assert_eq!(change.by, "the owner");
    assert_eq!(change.by_name, "the owner");
    assert_eq!(change, entry_of(&commit, false));
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn dc3_an_item_counts_once_per_node_it_names() {
    let both = ids(&["SCN-ARRIVAL", "SCN-STORM"]);
    let arrival = ids(&["SCN-ARRIVAL"]);
    let twice = ids(&["SCN-STORM", "SCN-STORM"]);
    let none: Vec<String> = Vec::new();
    let items = [
        // A question naming the arrival.
        OpenItem {
            target_id: None,
            node_ids: &arrival,
        },
        // An update targeting the arrival, its `node_ids` naming it too.
        OpenItem {
            target_id: Some("SCN-ARRIVAL"),
            node_ids: &arrival,
        },
        // An update with no `node_ids`.
        OpenItem {
            target_id: Some("SCN-ARRIVAL"),
            node_ids: &none,
        },
        // An item naming both scenes.
        OpenItem {
            target_id: None,
            node_ids: &both,
        },
        // A node named twice.
        OpenItem {
            target_id: Some("SCN-STORM"),
            node_ids: &twice,
        },
        // An item naming none.
        OpenItem {
            target_id: None,
            node_ids: &none,
        },
        // A target outside its `node_ids`.
        OpenItem {
            target_id: Some("CHP-SHORE"),
            node_ids: &arrival,
        },
    ];
    let expected: BTreeMap<String, u64> = [
        ("CHP-SHORE".to_owned(), 1),
        ("SCN-ARRIVAL".to_owned(), 5),
        ("SCN-STORM".to_owned(), 2),
    ]
    .into();
    assert_eq!(open_items_by_node(&items), expected);
    assert_eq!(open_items_by_node(&[]), BTreeMap::new());
}

#[test]
fn dc11_the_scan_cap_is_20000() {
    assert_eq!(LAST_CHANGE_SCAN_MAX, 20_000);
}
