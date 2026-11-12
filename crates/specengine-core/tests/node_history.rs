//! docs/features/node-history.md (RM-0035) "Acceptance criteria" NH2, the
//! unit half: core's pure `history` over messages built here, the apply
//! ones by `commit_message_raising` and `commit_message_interpreting` as an
//! apply writes them.
//!
//! - `applied_trailers`: the subject must be `spec: apply <ID>` and the
//!   message's last paragraph must hold `Proposal: <the same ID>`; only
//!   that paragraph is read, so a `Proposal:` line in the rationale never
//!   counts, a `Co-Authored-By:` paragraph last makes a hand commit, and
//!   one naming another ID too. M: every body line scanned.
//! - `identity_name`: `"Ann <a@b>"` gives `"Ann"`, `"mira"` and `"x <y"`
//!   the whole.
//! - `proposer`: `role=unknown` gives `"agent"`.
//! - The three caps are the spec's numbers.
//!
//! The CLI half (the same messages in real commits) is `specengine-cli`'s
//! `tests/node_history.rs`.

use specengine_core::history::{
    AppliedCommit, HISTORY_MAX, HISTORY_SCAN_MAX, HISTORY_TEXT_MAX, Revised, applied_trailers,
    identity_name, proposer,
};
use specengine_core::proposal::{
    ApplyTrailers, Author, AuthorType, CommitFacts, Interprets, Revised as Raise,
    commit_message_interpreting, commit_message_raising,
};

const DECIDER: &str = "Ann Owner <ann@example.org>";
const PROPOSED_BY: &str = "agent role=spec-writer model=m run=r";
const BASE: &str = "0123456789abcdef0123456789abcdef01234567";

fn writer() -> Author {
    Author {
        author_type: AuthorType::Agent,
        role: Some("spec-writer".to_owned()),
        model: Some("m".to_owned()),
        run: Some("r".to_owned()),
        login: None,
    }
}

fn facts<'a>(id: &'a str, rationale: &'a str, author: &'a Author) -> CommitFacts<'a> {
    CommitFacts {
        id,
        rationale,
        decided_by: DECIDER,
        author,
        base_commit: BASE,
    }
}

/// An apply's message with `revised` and `locked` trailers.
fn applied(id: &str, rationale: &str, revised: &[Raise<'_>], locked: &[&str]) -> String {
    let author = writer();
    commit_message_raising(
        &facts(id, rationale, &author),
        None,
        &ApplyTrailers { revised, locked },
    )
}

#[test]
fn the_caps_are_the_specs() {
    assert_eq!(HISTORY_MAX, 64);
    assert_eq!(HISTORY_SCAN_MAX, 128);
    assert_eq!(HISTORY_TEXT_MAX, 300);
}

/// A raise: every trailer as written, the rationale between the subject's
/// blank line and the trailer block.
#[test]
fn an_apply_reads_its_trailers() {
    let message = applied(
        "PR-0001",
        "Tighten the window.",
        &[Raise {
            id: "R-12",
            rev: 1,
            to: Some(2),
        }],
        &[],
    );
    assert_eq!(
        applied_trailers(&message),
        Some(AppliedCommit {
            proposal: "PR-0001".to_owned(),
            decided_by: Some(DECIDER.to_owned()),
            proposed_by: Some(PROPOSED_BY.to_owned()),
            revisions: vec![("R-12".to_owned(), Revised::Raised { from: 1, to: 2 })],
            rationale: Some("Tighten the window.".to_owned()),
        }),
        "{message}"
    );
}

/// Editorial and raised `Revision:` lines in order, beside `Lock:` and
/// `Lock-mode:`, which are not revisions.
#[test]
fn every_revision_in_order() {
    let message = applied(
        "PR-0002",
        "Wording only.",
        &[
            Raise {
                id: "R-12",
                rev: 2,
                to: None,
            },
            Raise {
                id: "R-13",
                rev: 4,
                to: Some(5),
            },
        ],
        &["R-12"],
    );
    let read = applied_trailers(&message).expect("an apply");
    assert_eq!(
        read.revisions,
        [
            ("R-12".to_owned(), Revised::Editorial { rev: 2 }),
            ("R-13".to_owned(), Revised::Raised { from: 4, to: 5 }),
        ],
        "{message}"
    );
    assert_eq!(read.rationale.as_deref(), Some("Wording only."));
}

/// An interpretation: its labelled texts are the rationale's, its
/// `Interprets:` trailer read past.
#[test]
fn an_interpretation_is_an_apply() {
    let author = writer();
    let message = commit_message_interpreting(
        &facts("PR-0007", "Read the rule so.", &author),
        Some(&Interprets {
            decision: "DEC-0031",
            how: "As written.",
            why: "It was unclear.",
        }),
    );
    let read = applied_trailers(&message).expect("an apply");
    assert_eq!(read.proposal, "PR-0007");
    assert_eq!(read.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(read.revisions, []);
    let rationale = read.rationale.expect("a rationale");
    assert!(
        rationale.starts_with("Read the rule so.") && rationale.contains("It was unclear."),
        "{rationale}"
    );
}

/// NH2: a `Proposal:` line in the rationale never counts.
#[test]
fn a_proposal_line_in_the_rationale_never_counts() {
    let message = applied(
        "PR-0001",
        "Tighten the window.\n\nProposal: PR-9999",
        &[],
        &[],
    );
    let read = applied_trailers(&message).expect("an apply");
    assert_eq!(read.proposal, "PR-0001", "{message}");
    assert_eq!(
        read.rationale.as_deref(),
        Some("Tighten the window.\n\nProposal: PR-9999")
    );
    // The rationale's line alone, before a trailer block of others.
    let lone = "spec: apply PR-0001\n\nProposal: PR-0001 is what this says.\n\nDecided-by: Ann \
                Owner <ann@example.org>\n";
    assert_eq!(
        applied_trailers(lone),
        None,
        "no `Proposal:` in the last paragraph"
    );
}

/// NH2: hand commits. M: every body line scanned (the middle paragraph's
/// `Proposal:` would read as an apply).
#[test]
fn hand_commits_have_no_apply_trailers() {
    for message in [
        "Tidy the window rule\n\nCo-Authored-By: Dee Pair <dee@example.org>\n",
        "spec: apply PR-0003\n\nProposal: PR-0003\n\nCo-Authored-By: Dee Pair <dee@example.org>\n",
        "spec: apply PR-0003\n\nNo trailer here.\n",
        "spec: apply PR-0003\n\nWhy.\n\nProposal: PR-0004\nDecided-by: Ann <a@b>\n",
        "spec: apply PR-0003\n",
        "spec: apply\n\nProposal: \n",
        "Fix a typo\n\nProposal: PR-0003\n",
        "",
    ] {
        assert_eq!(applied_trailers(message), None, "{message:?}");
    }
}

/// The smallest apply: a subject and the trailer paragraph, no rationale.
#[test]
fn an_apply_without_a_rationale() {
    let read = applied_trailers("spec: apply PR-0003\n\nProposal: PR-0003\n").expect("an apply");
    assert_eq!(read.proposal, "PR-0003");
    assert_eq!(read.rationale, None);
    assert_eq!(read.decided_by, None);
    assert_eq!(read.proposed_by, None);
}

#[test]
fn identity_name_is_the_part_before_the_address() {
    assert_eq!(identity_name("Ann <a@b>"), "Ann");
    assert_eq!(identity_name("Ann Owner <ann@example.org>"), "Ann Owner");
    assert_eq!(identity_name("mira"), "mira");
    assert_eq!(identity_name("x <y"), "x <y");
    assert_eq!(identity_name(""), "");
}

#[test]
fn proposer_is_the_type_and_the_role() {
    assert_eq!(proposer(PROPOSED_BY).as_deref(), Some("agent spec-writer"));
    assert_eq!(
        proposer("agent role=unknown model=m run=r").as_deref(),
        Some("agent")
    );
    assert_eq!(
        proposer("human role=unknown model=unknown run=unknown").as_deref(),
        Some("human")
    );
    assert_eq!(
        proposer("agent role=developer model=m run=r login=ann").as_deref(),
        Some("agent developer")
    );
    // The provenance `Author::provenance` writes reads back.
    assert_eq!(
        proposer(&writer().provenance()).as_deref(),
        Some("agent spec-writer")
    );
}
