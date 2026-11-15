//! docs/features/auto-rule.md RL11 (the seam): `delegated_approve` on
//! `fixtures/auto-mini` through the library at an injected clock, the
//! update raised in the linked worktree `t1`, approved from the main one.
//!
//! - `Delegation {rule, pack: Some(812)}` on an open `update`: one commit,
//!   `proposal-apply.md` "Commit"'s message with `Delegated-by: auto
//!   <hash>` last (after `Base-commit:` and `Revision:`), the worktree's
//!   identity;
//!   `proposal.approved` and `.applied` with `"by":"rule"`, `rule`,
//!   `"pack":812` after today's keys.
//! - `identity` set (a hub): author and committer are the login and email.
//! - A row whose text no longer applies: refused, nothing applied; a row
//!   not open: refused, nothing written.
//! - A plain `spec approve` stays as RL1 (`auto_manual.rs`).
//!
//! - `expected` (the reviewer's row): a row changed since the caller's
//!   predicate read it (a stage) gives `Moved`, nothing written.
//!
//! Mutations: the trailer before `Base-commit:`; `by` always written.

#![cfg(unix)]

mod auto_common;
mod common;

use auto_common::{Auto, COMMITTER};
use common::proposal::LATER;
use serde_json::json;
use specengine_cli::{
    DelegatedApproveRequest, DelegatedOutcome, Delegation, Globals, delegated_approve,
};

const RULE: &str = "b3:7777777777777777777777777777777777777777777777777777777777777777";

fn delegated(
    auto: &Auto,
    id: &str,
    pack: Option<i64>,
    identity: Option<(&str, &str)>,
) -> DelegatedOutcome {
    delegated_approve(
        &auto.env(&auto.main),
        &Globals::default(),
        &DelegatedApproveRequest {
            id: id.to_owned(),
            now: LATER.to_owned(),
            git: auto.git_env(&auto.main),
            delegation: Delegation {
                rule: RULE.to_owned(),
                pack,
            },
            identity: identity.map(|(l, e)| (l.to_owned(), e.to_owned())),
            expected: None,
        },
    )
    .unwrap_or_else(|error| panic!("delegated_approve {id}: {error}"))
}

/// RL11. M: the trailer before `Base-commit:`; the keys without `pack`.
#[test]
fn rl11_a_delegated_apply_is_one_commit_with_its_trailer_last() {
    let auto = Auto::with_rule("rl11", "on.toml");
    let id = auto.propose_edit(&auto.linked, "R-01", "three save slots", "four save slots");
    let before = auto.rev(&auto.linked, "HEAD");
    let outcome = delegated(&auto, &id, Some(812), None);
    let DelegatedOutcome::Applied(applied) = &outcome else {
        panic!("not applied: {outcome:?}");
    };
    let commit = outcome.commit().expect("a commit").to_owned();
    assert_eq!(applied.document.status.as_deref(), Some("applied"));
    // One commit on `t1`.
    assert_eq!(
        auto.git_text(&auto.linked, &["rev-list", &format!("{before}..HEAD")]),
        commit
    );
    let message = auto.message(&auto.linked, &commit);
    let lines: Vec<&str> = message.trim_end().lines().collect();
    assert_eq!(
        lines.last(),
        Some(&format!("Delegated-by: auto {RULE}").as_str()),
        "{message}"
    );
    let base = lines
        .iter()
        .position(|line| line.starts_with("Base-commit: "))
        .expect("Base-commit:");
    assert!(
        base + 1 < lines.len(),
        "Delegated-by after Base-commit: {message}"
    );
    assert!(
        lines[0].starts_with(&format!("spec: apply {id}")),
        "{message}"
    );
    for trailer in ["Proposal: ", "Decided-by: ", "Proposed-by: "] {
        assert!(
            lines.iter().any(|line| line.starts_with(trailer)),
            "{trailer}: {message}"
        );
    }
    assert!(
        message.contains(&format!("Decided-by: {COMMITTER}")),
        "{message}"
    );
    // The worktree's identity.
    assert_eq!(
        auto.git_text(&auto.linked, &["log", "-1", "--format=%cn <%ce>", &commit]),
        COMMITTER
    );
    // The events: today's keys, then by, rule, pack.
    let applied_event = auto.events_of_kind("proposal.applied").pop().unwrap();
    assert_eq!(
        applied_event.json(),
        json!({"id": id, "commit": commit, "by": "rule", "rule": RULE, "pack": 812})
    );
    assert!(
        applied_event.payload.ends_with(&format!(
            ",\"by\":\"rule\",\"rule\":\"{RULE}\",\"pack\":812}}"
        )),
        "{}",
        applied_event.payload
    );
    let approved = auto.events_of_kind("proposal.approved").pop().unwrap();
    assert!(
        approved.payload.ends_with(&format!(
            ",\"by\":\"rule\",\"rule\":\"{RULE}\",\"pack\":812}}"
        )),
        "{}",
        approved.payload
    );
    assert_eq!(approved.json()["id"], id.as_str());
    // The rehearsal logged nothing of its own: one approved, one applied.
    assert_eq!(auto.events_of_kind("proposal.approved").len(), 1);
    assert!(auto.events_of_kind("proposal.apply_failed").is_empty());

    // Not open any more: refused, nothing written.
    let events = auto.raw_events();
    let head = auto.rev(&auto.linked, "HEAD");
    let again = delegated(&auto, &id, Some(812), None);
    assert!(matches!(again, DelegatedOutcome::Refused(_)), "{again:?}");
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.rev(&auto.linked, "HEAD"), head);
}

/// A hub's identity and no pack: author and committer the login, the
/// events without `pack`.
#[test]
fn rl11_a_hubs_identity_and_no_pack() {
    let auto = Auto::with_rule("rl11-hub", "hub.toml");
    let id = auto.propose_edit(&auto.linked, "R-02", "five seconds", "six seconds");
    let outcome = delegated(&auto, &id, None, Some(("ann", "ann@example.org")));
    let commit = outcome.commit().expect("applied").to_owned();
    assert_eq!(
        auto.git_text(
            &auto.linked,
            &["log", "-1", "--format=%an <%ae>|%cn <%ce>", &commit]
        ),
        "ann <ann@example.org>|ann <ann@example.org>"
    );
    let message = auto.message(&auto.linked, &commit);
    assert!(
        message.contains("Decided-by: ann <ann@example.org>"),
        "{message}"
    );
    assert!(
        message
            .trim_end()
            .ends_with(&format!("Delegated-by: auto {RULE}")),
        "{message}"
    );
    let applied = auto.events_of_kind("proposal.applied").pop().unwrap();
    assert_eq!(
        applied.json(),
        json!({"id": id, "commit": commit, "by": "rule", "rule": RULE})
    );
}

/// A stale base: the run refuses at its step, nothing applied, the row
/// open; no `Delegated-by:` commit.
#[test]
fn rl11_a_row_that_no_longer_applies_is_refused() {
    let auto = Auto::with_rule("rl11-stale", "on.toml");
    let id = auto.propose_edit(&auto.linked, "R-02", "five seconds", "six seconds");
    common::replace(
        &auto.linked,
        "docs/spec/core.md",
        "A door closes five seconds",
        "A door closes ten seconds",
    );
    auto.commit_all(&auto.linked, "R-02 moved");
    let head = auto.rev(&auto.linked, "HEAD");
    let outcome = delegated(&auto, &id, Some(812), None);
    assert!(
        matches!(outcome, DelegatedOutcome::Refused(_)),
        "{outcome:?}"
    );
    assert!(outcome.reason().is_some());
    assert_eq!(auto.rev(&auto.linked, "HEAD"), head);
    assert_eq!(auto.proposal(&id).status.as_str(), "open");
    assert!(auto.events_of_kind("proposal.applied").is_empty());
}

/// RL11's `Moved` through `expected`: the row's `Seen` read, then the row
/// staged by a person, then the delegated apply with the old `Seen`:
/// `Moved`, nothing written, the row open with its stage. M: the consent
/// answering yes without the compare.
#[test]
fn rl11_a_row_changed_since_it_was_judged_is_moved() {
    use specengine_store::{ProposalQueue as _, Stage};
    let auto = Auto::with_rule("rl11-moved", "on.toml");
    let id = auto.propose_edit(&auto.linked, "R-01", "three save slots", "four save slots");
    let judged = auto.proposal(&id).seen();
    {
        let mut queue = auto.store();
        queue
            .stage_from(
                &id,
                &judged,
                &Stage::Reject {
                    reason: "Not now.".to_owned(),
                },
                LATER,
            )
            .expect("staged");
    }
    let events = auto.raw_events();
    let head = auto.rev(&auto.linked, "HEAD");
    let outcome = delegated_approve(
        &auto.env(&auto.main),
        &Globals::default(),
        &DelegatedApproveRequest {
            id: id.clone(),
            now: LATER.to_owned(),
            git: auto.git_env(&auto.main),
            delegation: Delegation {
                rule: RULE.to_owned(),
                pack: Some(812),
            },
            identity: None,
            expected: Some(judged),
        },
    )
    .expect("delegated_approve");
    assert!(
        matches!(outcome, DelegatedOutcome::Moved { .. }),
        "{outcome:?}"
    );
    assert_eq!(auto.raw_events(), events, "nothing written");
    assert_eq!(auto.rev(&auto.linked, "HEAD"), head);
    assert_eq!(auto.proposal(&id).status.as_str(), "open");
    // Another row, its `Seen` as judged: it applies.
    let other = auto.propose_edit(&auto.linked, "R-04", "save altar", "save shrine");
    let seen = auto.proposal(&other).seen();
    let outcome = delegated_approve(
        &auto.env(&auto.main),
        &Globals::default(),
        &DelegatedApproveRequest {
            id: other.clone(),
            now: LATER.to_owned(),
            git: auto.git_env(&auto.main),
            delegation: Delegation {
                rule: RULE.to_owned(),
                pack: Some(812),
            },
            identity: None,
            expected: Some(seen),
        },
    )
    .expect("delegated_approve");
    assert!(outcome.commit().is_some(), "{outcome:?}");
}
