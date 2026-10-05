//! docs/features/agent-intake.md through the CLI library (the MCP tools'
//! twins): `spec propose question`, `spec propose discrepancy`, the queue
//! records that never apply, their dedup against what is decided and asked
//! (Rules 6–7), the refusals (Rules 1–5), a proposed patch as a linked
//! update, escaping, and settling (`approve` refused before any prompt,
//! `reject` with the answer). AC-03 (the twin half), AC-04, AC-06, AC-07,
//! AC-08, AC-09.
//!
//! Scratch git repositories of `fixtures/spec-a` as `common::proposal`
//! makes them (the main worktree on `main`, a linked one on `t1`), a scratch
//! `HOME`, the injected clock, consent through the callback; the binary only
//! where it is the subject (the twin without author flags, `--input`).
//! "Refused" = exit 1 naming the field, nothing stored, no event, the next
//! ID unchanged (the queue's `dump()` byte-identical).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::path::Path;

use common::proposal::{LATER, NOW, Pair, json_of, printed, printed_inbox};
use common::write;
use serde_json::{Value, json};
use specengine_cli::{
    CliError, DiscrepancyInput, DiscrepancyRequest, Evidence, Exit, GapType, Globals, IntakeOption,
    IntakeOutcome, IntakeSeverity, IntakeSource, Message, Outcome, ProposedPatch, QuestionRequest,
    ReviewRequest, propose_discrepancy, propose_question, read_discrepancy_input, render_json,
    render_text, review_brief,
};
use specengine_core::proposal::Author;
use specengine_store::{ProposalKind, ProposalStatus};

const QUESTION: &str = "Does the sprint stop at zero stamina?";

// ---------------------------------------------------------------- helpers

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

/// A question of a `developer` agent about `node_ids`, from `cwd`.
fn question(pair: &Pair, cwd: &Path, node_ids: &[&str], text: &str) -> QuestionRequest {
    QuestionRequest {
        node_ids: ids(node_ids),
        text: text.to_owned(),
        working_answer: "yes, 1.5 s".to_owned(),
        price_of_other: "R-12 rebalanced".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: None,
        run: None,
        now: NOW.to_owned(),
        git: pair.git_env(cwd),
    }
}

fn ask(pair: &Pair, cwd: &Path, request: &QuestionRequest) -> Result<IntakeOutcome, CliError> {
    propose_question(&pair.env(cwd), &Globals::default(), request)
}

/// An ask that answers (exit 0, stored or not).
fn asked(pair: &Pair, cwd: &Path, request: &QuestionRequest) -> IntakeOutcome {
    let outcome = ask(pair, cwd, request).unwrap_or_else(|error| panic!("ask: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome
}

fn option(label: &str) -> IntakeOption {
    IntakeOption {
        label: label.to_owned(),
        effect: format!("{label}: the effect"),
        price: "1 item".to_owned(),
    }
}

fn evidence() -> Evidence {
    Evidence {
        file: "src/stamina.rs".to_owned(),
        qpath: Some("stamina::regen".to_owned()),
        lines: Some("3-9".to_owned()),
        observed: "regenerates while walking".to_owned(),
        documented: "only at rest".to_owned(),
    }
}

fn discrepancy(node_ids: &[&str], summary: &str) -> DiscrepancyInput {
    DiscrepancyInput {
        node_ids: ids(node_ids),
        summary: summary.to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::High,
        evidence: vec![evidence()],
        options: vec![option("code"), option("spec")],
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from: None,
    }
}

fn report(pair: &Pair, cwd: &Path, input: DiscrepancyInput) -> Result<IntakeOutcome, CliError> {
    propose_discrepancy(
        &pair.env(cwd),
        &Globals::default(),
        &DiscrepancyRequest {
            input,
            author_role: Some("developer".to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(cwd),
        },
    )
}

/// The outcome's stdout text and parsed JSON, as `spec` prints them.
fn printed_intake(outcome: &IntakeOutcome) -> (String, Value) {
    let outcome = Outcome::Intake(Box::new(outcome.clone()));
    (render_text(&outcome), json_of(&render_json(&outcome)))
}

/// Everything a refusal must leave: the queue's rows and events.
fn queue_state(pair: &Pair) -> String {
    if pair.db().exists() {
        pair.queue().dump().expect("dump")
    } else {
        String::new()
    }
}

/// The outcome is a refusal (exit 1) that changed nothing: its reason.
fn refused(
    pair: &Pair,
    before: &str,
    outcome: &Result<IntakeOutcome, CliError>,
    context: &str,
) -> String {
    let reason = match outcome {
        Ok(outcome) => {
            assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
            assert!(outcome.document.id.is_none(), "{context}: {outcome:?}");
            assert!(!outcome.document.created, "{context}");
            assert_eq!(
                outcome.document.notes.last(),
                outcome.refusal.as_ref(),
                "{context}: the reason is the last note"
            );
            let (text, _) = printed_intake(outcome);
            assert_eq!(text, "", "{context}: a refusal prints no result");
            outcome.refusal.clone().expect("a refusal")
        }
        Err(error) => panic!("{context}: exit {:?}: {error}", error.exit),
    };
    assert_eq!(queue_state(pair), before, "{context}: the queue changed");
    reason
}

/// The outcome could not run (exit 2): its message; the queue unchanged.
fn cannot(
    pair: &Pair,
    before: &str,
    outcome: &Result<IntakeOutcome, CliError>,
    context: &str,
) -> String {
    let message = match outcome {
        Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
            error.message.clone()
        }
    };
    assert_eq!(queue_state(pair), before, "{context}: the queue changed");
    message
}

/// Library `reject` with `reason`, consent yes; it must reject.
fn reject_ok(pair: &Pair, cwd: &Path, id: &str, reason: &str) {
    let (outcome, questions) = pair.reject_answer(cwd, id, reason, true);
    let outcome = outcome.unwrap_or_else(|error| panic!("reject {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "reject {id}: {outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
}

fn hit_ids(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect()
}

// ------------------------------------------------------------------ AC-03

/// AC-03, the twin half: no author → `human` (the library and the binary
/// without `A`); `--author-role nest-developer` alone →
/// `{"type":"agent","role":"nest-developer","model":null,"run":null}`
/// verbatim (no enum); `"a b"` refused naming `author_role`. M: the role
/// optional and absent passed on (the MCP half), an enum of roles.
#[test]
fn ac03_the_author_is_verbatim_and_without_one_human() {
    let pair = Pair::new("ai-ac03", "spec-a");
    let cwd = pair.linked.clone();

    let before = queue_state(&pair);
    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], "One?");
    request.author_role = Some("a b".to_owned());
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "a b");
    assert!(reason.starts_with("author_role: "), "{reason}");

    request.author_role = Some("nest-developer".to_owned());
    let outcome = asked(&pair, &cwd, &request);
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0001"));
    let stored = pair.proposal("PR-0001");
    assert_eq!(
        serde_json::to_value(&stored.author).unwrap(),
        json!({"type": "agent", "role": "nest-developer", "model": null, "run": null})
    );

    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], "Two?");
    request.author_role = None;
    asked(&pair, &cwd, &request);
    assert_eq!(pair.proposal("PR-0002").author, Author::human());

    // The binary without author flags: `human` too.
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "question",
            "EDGE-STAM-ZERO",
            "--text",
            "Three?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "none",
        ],
        b"",
    );
    assert_eq!(run.code, 0, "{run:?}");
    assert_eq!(
        run.stdout, "PR-0003\nrelated: PR-0001 | open | - | -\nrelated: PR-0002 | open | - | -\n",
        "{run:?}"
    );
    assert_eq!(pair.proposal("PR-0003").author, Author::human());
    assert_eq!(pair.proposal("PR-0003").kind, ProposalKind::Question);
}

// ------------------------------------------------------------------ AC-04

/// AC-04: `EDGE-STAM-ZERO` asked twice, whitespace (U+3000 too) and case
/// apart: not created, the first ID its hit, `counts()` unchanged; after
/// `reject --reason R` the hit reads `rejected`, answer `R`;
/// `RULE-STAM-REGEN` and `Q-031` hit `DEC-0023` with its title, nothing
/// stored (Data's example, exactly); the same text on another node is
/// created; `distinct_from` naming every hit stores it (in review), a
/// subset does not; a temp accepted decision only mentioning the node is
/// `related` and the question created. M: dedup off; targets ignored; any
/// `distinct_from`; mentions as hits.
#[test]
fn ac04_what_is_decided_or_asked_answers_a_question() {
    let pair = Pair::new("ai-ac04", "spec-a");
    let cwd = pair.linked.clone();

    let first = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    assert_eq!(first.document.id.as_deref(), Some("PR-0001"));
    assert!(first.document.created && first.document.hits.is_empty());
    let (text, _) = printed_intake(&first);
    assert_eq!(text, "PR-0001\n");
    let counts = pair.queue().counts().expect("counts");

    let again = "  does THE sprint\u{3000}stop at\tzero\n STAMINA?  ";
    let second = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], again),
    );
    let (text, json) = printed_intake(&second);
    assert_eq!(
        json,
        json!({"id": null, "created": false,
            "hits": [{"id": "PR-0001", "source": "queue", "status": "open", "path": null,
                "answer": null}],
            "related": [], "linked": null, "diagnostics": [], "notes": []})
    );
    assert_eq!(
        text,
        "hit: PR-0001 | open | - | -\n\
         not stored: name every hit in `distinct_from` to store it anyway\n"
    );
    assert_eq!(
        pair.queue().counts().expect("counts"),
        counts,
        "nothing stored"
    );

    reject_ok(
        &pair,
        &pair.main,
        "PR-0001",
        "Yes: it stops at once.\nSee A-101.",
    );
    let third = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], again),
    );
    let (text, json) = printed_intake(&third);
    assert_eq!(
        json["hits"],
        json!([{"id": "PR-0001", "source": "queue", "status": "rejected", "path": null,
            "answer": "Yes: it stops at once.\nSee A-101."}])
    );
    assert!(
        text.starts_with("hit: PR-0001 | rejected | - | Yes: it stops at once.\n"),
        "{text}"
    );
    assert_eq!(
        pair.queue().counts().expect("counts").proposals,
        counts.proposals
    );

    // Data's example, exactly; Q-031 by `answers`, RULE-STAM-REGEN by
    // `canon:`.
    for target in ["Q-031", "RULE-STAM-REGEN"] {
        let request = question(&pair, &cwd, &[target], "Does regeneration wait for rest?");
        let outcome = asked(&pair, &cwd, &request);
        let (text, json) = printed_intake(&outcome);
        assert_eq!(
            json,
            json!({"id": null, "created": false,
                "hits": [{"id": "DEC-0023", "source": "corpus", "status": "accepted",
                    "path": "docs/records/DEC/DEC-0023.md",
                    "answer": "Regeneration waits for rest"}],
                "related": [], "linked": null, "diagnostics": [], "notes": []}),
            "{target}"
        );
        assert_eq!(
            text,
            "hit: DEC-0023 | accepted | docs/records/DEC/DEC-0023.md | Regeneration waits for rest\n\
             not stored: name every hit in `distinct_from` to store it anyway\n",
            "{target}"
        );
    }
    assert_eq!(pair.queue().counts().expect("counts"), {
        let mut after = counts;
        after.events += 1; // the rejection
        after
    });

    // The same text on another node: created.
    let other = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-SPRINT-EMPTY"], QUESTION),
    );
    assert_eq!(other.document.id.as_deref(), Some("PR-0002"), "{other:?}");
    assert!(other.document.hits.is_empty(), "{other:?}");

    // Two nodes: DEC-0023 and PR-0001 hit; a subset named is not enough.
    let mut both = question(
        &pair,
        &cwd,
        &["RULE-STAM-REGEN", "EDGE-STAM-ZERO"],
        QUESTION,
    );
    for subset in [
        vec![],
        vec!["DEC-0023"],
        vec!["PR-0001"],
        vec!["dec-0023", "PR-0001"],
    ] {
        both.distinct_from = ids(&subset);
        let outcome = asked(&pair, &cwd, &both);
        assert_eq!(hit_ids(&outcome), ["DEC-0023", "PR-0001"], "{subset:?}");
        assert!(!outcome.document.created, "{subset:?}: {outcome:?}");
    }
    both.distinct_from = ids(&["PR-0001", "DEC-0023"]);
    let stored = asked(&pair, &cwd, &both);
    assert_eq!(stored.document.id.as_deref(), Some("PR-0003"), "{stored:?}");
    assert_eq!(hit_ids(&stored), ["DEC-0023", "PR-0001"]);
    let (text, _) = printed_intake(&stored);
    assert!(text.starts_with("PR-0003\nhit: DEC-0023 | "), "{text}");
    let review = pair.review_ok(&cwd, "PR-0003");
    let (text, json) = printed(&review);
    let json = json_of(&json);
    assert_eq!(json["distinct_from"], json!(["PR-0001", "DEC-0023"]));
    assert_eq!(
        json["target_ids"],
        json!(["RULE-STAM-REGEN", "EDGE-STAM-ZERO"])
    );
    assert!(
        text.contains("\ndistinct_from: 2\n  PR-0001\n  DEC-0023\n"),
        "{text}"
    );

    // A temp accepted decision that only mentions the node: related.
    write(
        &cwd,
        "docs/records/DEC/DEC-0099.md",
        "---\nid: DEC-0099\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n---\n\n\
         # Sprint feel\n\nThe empty tank (EDGE-SPRINT-EMPTY) keeps its sound.\n",
    );
    let outcome = asked(
        &pair,
        &cwd,
        &question(
            &pair,
            &cwd,
            &["EDGE-SPRINT-EMPTY"],
            "Is the tank sound kept?",
        ),
    );
    let (text, json) = printed_intake(&outcome);
    assert_eq!(json["id"], json!("PR-0004"), "{json}");
    assert_eq!(json["hits"], json!([]), "{json}");
    assert_eq!(
        json["related"],
        json!([
            {"id": "DEC-0099", "source": "corpus", "status": "accepted",
                "path": "docs/records/DEC/DEC-0099.md", "answer": null},
            {"id": "PR-0002", "source": "queue", "status": "open", "path": null,
                "answer": null}
        ])
    );
    assert_eq!(
        text,
        "PR-0004\n\
         related: DEC-0099 | accepted | docs/records/DEC/DEC-0099.md | -\n\
         related: PR-0002 | open | - | -\n"
    );
}

/// A discrepancy is deduplicated as a question is, against its own kind
/// only: the same summary on a shared node hits the stored discrepancy, a
/// question of that text does not; the hit's ID named in `distinct_from`
/// stores it.
#[test]
fn a_discrepancy_hits_only_its_own_kind() {
    let pair = Pair::new("ai-dedup-kind", "spec-a");
    let cwd = pair.linked.clone();
    let summary = "The tank refills while walking.";
    let first = report(&pair, &cwd, discrepancy(&["EDGE-STAM-ZERO"], summary)).unwrap();
    assert_eq!(first.document.id.as_deref(), Some("PR-0001"), "{first:?}");
    let asked = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], summary),
    );
    assert_eq!(asked.document.id.as_deref(), Some("PR-0002"), "{asked:?}");
    let mut input = discrepancy(
        &["EDGE-SPRINT-EMPTY", "EDGE-STAM-ZERO"],
        "the tank REFILLS while walking.",
    );
    let second = report(&pair, &cwd, input.clone()).unwrap();
    assert_eq!(hit_ids(&second), ["PR-0001"], "{second:?}");
    assert!(!second.document.created);
    input.distinct_from = Some(ids(&["PR-0001"]));
    let third = report(&pair, &cwd, input).unwrap();
    assert_eq!(third.document.id.as_deref(), Some("PR-0003"), "{third:?}");
}

// ------------------------------------------------------------------ AC-06

/// AC-06: each refused naming the field, nothing stored, no event, the next
/// ID unchanged: 1 and 7 options, `recommendation` 2 of 2, a blank
/// `working_answer`, a blank `price_of_other`, no `node_ids`, an unknown
/// ID, the alias `QST-031` (exit 1 naming `Q-031`), a U+0420 look-alike
/// (exit 2), an evidence field over its cap, an unknown argument of
/// `--input` (exit 2); then a valid ask stores `PR-0001`. M: `options`
/// optional; a cap dropped.
#[test]
fn ac06_each_refusal_names_its_field_and_stores_nothing() {
    let pair = Pair::new("ai-ac06", "spec-a");
    let cwd = pair.linked.clone();
    // A refusal after the index exists too: one read first.
    pair.node(&cwd, "EDGE-STAM-ZERO");
    let before = queue_state(&pair);

    let with_options = |count: usize| {
        let mut input = discrepancy(&["EDGE-STAM-ZERO"], "It departs.");
        input.options = (0..count)
            .map(|index| option(&format!("o{index}")))
            .collect();
        input
    };
    for (count, want) in [
        (1, "options: 1 option(s); give 2 to 6, each priced"),
        (7, "options: 7 option(s); give 2 to 6, each priced"),
        (0, "options: 0 option(s); give 2 to 6, each priced"),
    ] {
        let reason = refused(
            &pair,
            &before,
            &report(&pair, &cwd, with_options(count)),
            want,
        );
        assert_eq!(reason, want);
    }
    let mut input = with_options(2);
    input.recommendation = 2;
    let reason = refused(
        &pair,
        &before,
        &report(&pair, &cwd, input),
        "recommendation",
    );
    assert_eq!(
        reason,
        "recommendation: 2 is no index into the 2 options (0 to 1)"
    );

    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
    request.working_answer = " \t\u{3000}".to_owned();
    let reason = refused(
        &pair,
        &before,
        &ask(&pair, &cwd, &request),
        "working_answer",
    );
    assert_eq!(reason, "working_answer: blank: give a value");
    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
    request.price_of_other = String::new();
    let reason = refused(
        &pair,
        &before,
        &ask(&pair, &cwd, &request),
        "price_of_other",
    );
    assert_eq!(reason, "price_of_other: blank: give a value");
    let mut input = discrepancy(&["EDGE-STAM-ZERO"], "It departs.");
    input.working_answer = Some("  ".to_owned());
    let reason = refused(&pair, &before, &report(&pair, &cwd, input), "a given blank");
    assert_eq!(reason, "working_answer: blank: give a value");

    let request = question(&pair, &cwd, &[], QUESTION);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "no node_ids");
    assert_eq!(reason, "node_ids: none: name 1 to 16 nodes by ID");
    let seventeen: Vec<&str> = std::iter::repeat_n("EDGE-STAM-ZERO", 17).collect();
    let request = question(&pair, &cwd, &seventeen, QUESTION);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "17 node_ids");
    assert_eq!(reason, "node_ids: 17 IDs; at most 16");

    let request = question(&pair, &cwd, &["EDGE-STAM-ZERO", "MEC-NOPE"], QUESTION);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "unknown ID");
    assert!(
        reason.starts_with("node_ids[1]: ") && reason.contains("MEC-NOPE"),
        "{reason}"
    );
    let request = question(&pair, &cwd, &["QST-031"], QUESTION);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "alias");
    assert!(
        reason.starts_with("node_ids[0]: ") && reason.contains("`Q-031`"),
        "{reason}"
    );
    let request = question(&pair, &cwd, &["Q-031", "QST-031"], QUESTION);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "twice");
    assert!(reason.starts_with("node_ids[1]: "), "{reason}");
    // U+0420 (Cyrillic Er) for the `P` of a Latin-looking ID: exit 2.
    let request = question(&pair, &cwd, &["RULE-S\u{0420}RINT-COST"], QUESTION);
    let message = cannot(&pair, &before, &ask(&pair, &cwd, &request), "look-alike");
    assert!(message.contains("node_ids[0]"), "{message}");
    assert!(message.contains("RULE-SPRINT-COST"), "{message}");

    let mut input = discrepancy(&["EDGE-STAM-ZERO"], "It departs.");
    input.evidence = vec![evidence(), evidence(), evidence()];
    input.evidence[2].observed = "x".repeat(1300);
    let reason = refused(&pair, &before, &report(&pair, &cwd, input), "evidence cap");
    assert_eq!(reason, "evidence[2].observed: 1300 bytes; at most 1024");
    for (field, at, cap) in [
        ("text", 1025, 1024),
        ("working_answer", 2049, 2048),
        ("price_of_other", 2049, 2048),
    ] {
        let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
        let value = "y".repeat(at);
        match field {
            "text" => request.text = value,
            "working_answer" => request.working_answer = value,
            _ => request.price_of_other = value,
        }
        let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), field);
        assert_eq!(reason, format!("{field}: {at} bytes; at most {cap}"));
    }
    for (field, edit) in [
        ("summary", 1025_usize),
        ("evidence[0].file", 513),
        ("evidence[0].qpath", 513),
        ("evidence[0].documented", 1025),
        ("options[1].label", 129),
        ("options[0].effect", 513),
        ("options[0].price", 513),
        ("working_answer", 2049),
    ] {
        let mut input = discrepancy(&["EDGE-STAM-ZERO"], "It departs.");
        let value = "z".repeat(edit);
        match field {
            "summary" => input.summary = value,
            "evidence[0].file" => input.evidence[0].file = value,
            "evidence[0].qpath" => input.evidence[0].qpath = Some(value),
            "evidence[0].documented" => input.evidence[0].documented = value,
            "options[1].label" => input.options[1].label = value,
            "options[0].effect" => input.options[0].effect = value,
            "options[0].price" => input.options[0].price = value,
            _ => input.working_answer = Some(value),
        }
        let reason = refused(&pair, &before, &report(&pair, &cwd, input), field);
        assert!(
            reason.starts_with(&format!("{field}: {edit} bytes; at most ")),
            "{reason}"
        );
    }
    let mut input = discrepancy(&["EDGE-STAM-ZERO"], "It departs.");
    input.evidence = (0..9).map(|_| evidence()).collect();
    let reason = refused(&pair, &before, &report(&pair, &cwd, input), "9 evidence");
    assert_eq!(reason, "evidence: 9 items; at most 8");
    let mut input = discrepancy(&["EDGE-STAM-ZERO"], "It departs.");
    input.evidence[0].lines = Some("9-3".to_owned());
    let reason = refused(&pair, &before, &report(&pair, &cwd, input), "lines");
    assert!(
        reason.starts_with("evidence[0].lines: \"9-3\" is not"),
        "{reason}"
    );
    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
    request.distinct_from = ids(&["PR-0001", "A\nB"]);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "distinct_from");
    assert_eq!(reason, "distinct_from[1]: holds a control character");
    request.distinct_from = (0..65).map(|index| format!("PR-{index:04}")).collect();
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "65 distinct");
    assert_eq!(reason, "distinct_from: 65 entries; at most 64");

    // An unknown argument of `--input`: exit 2 naming it, nothing read on.
    let mut document = serde_json::to_value(evidence()).unwrap();
    document["bogus"] = json!(1);
    let unknown = json!({"node_ids": ["EDGE-STAM-ZERO"], "summary": "S", "gap_type": "partial",
        "severity": "low", "evidence": [document], "options": [], "recommendation": 0});
    match read_discrepancy_input(
        &pair.env(&cwd),
        &IntakeSource::Given(unknown.to_string().into_bytes()),
    ) {
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun, "{error}");
            assert!(error.message.contains("bogus"), "{}", error.message);
        }
        Ok(input) => panic!("an unknown argument read as {input:?}"),
    }
    assert_eq!(queue_state(&pair), before);

    let stored = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    assert_eq!(stored.document.id.as_deref(), Some("PR-0001"), "{stored:?}");
    assert_eq!(pair.events_of("PR-0001").len(), 1);
    assert_eq!(pair.events().len(), 1, "no event of a refusal");
}

/// Rules 1: `--input` not UTF-8, not JSON, not the shape, an unknown
/// severity, or over 8 MiB, from a file or stdin, exits 2 naming
/// `--input`; the binary's `--severity` takes only `high|normal|low`
/// (exit 2). Nothing stored.
#[test]
fn a_bad_input_document_exits_2() {
    let pair = Pair::new("ai-input", "spec-a");
    let cwd = pair.linked.clone();
    let good = json!({"node_ids": ["EDGE-STAM-ZERO"], "summary": "S", "gap_type": "partial",
        "severity": "low", "evidence": [evidence()],
        "options": [option("a"), option("b")], "recommendation": 1});
    let mut bad_severity = good.clone();
    bad_severity["severity"] = json!("urgent");
    let mut no_summary = good.clone();
    no_summary.as_object_mut().unwrap().remove("summary");
    let mut big = good.to_string().into_bytes();
    big.extend(std::iter::repeat_n(b' ', 8 << 20));
    let cases: [(&str, Vec<u8>); 5] = [
        ("not UTF-8", b"{\"node_ids\": [\"\xff\"]}".to_vec()),
        ("not JSON", b"{".to_vec()),
        ("no summary", no_summary.to_string().into_bytes()),
        ("severity", bad_severity.to_string().into_bytes()),
        ("8 MiB", big),
    ];
    for (name, bytes) in cases {
        let run = pair.spec_piped(
            &cwd,
            &[
                "propose",
                "discrepancy",
                "--input",
                "-",
                "--author-role",
                "dev",
            ],
            &bytes,
        );
        assert_eq!(run.code, 2, "{name}: {run:?}");
        assert!(run.stderr.contains("--input -"), "{name}: {}", run.stderr);
        assert_eq!(run.stdout, "", "{name}");
        write(&cwd, "../input.json", &bytes);
        let file = cwd.join("../input.json");
        let run = pair.spec_piped(
            &cwd,
            &["propose", "discrepancy", "--input", file.to_str().unwrap()],
            b"",
        );
        assert_eq!(run.code, 2, "{name}: {run:?}");
        assert!(run.stderr.contains("--input "), "{name}: {}", run.stderr);
    }
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "question",
            "EDGE-STAM-ZERO",
            "--text",
            "T",
            "--working-answer",
            "W",
            "--price-of-other",
            "P",
            "--severity",
            "urgent",
        ],
        b"",
    );
    assert_eq!(run.code, 2, "{run:?}");
    assert!(!pair.db().exists() || pair.proposals().is_empty());
    // The good document stores, from stdin, as the twin of MCP's call.
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "discrepancy",
            "--input",
            "-",
            "--author-role",
            "dev",
        ],
        good.to_string().as_bytes(),
    );
    assert_eq!(run.code, 0, "{run:?}");
    assert_eq!(run.stdout, "PR-0001\n");
    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.kind, ProposalKind::Discrepancy);
    let intake = stored.intake.expect("its fields");
    assert_eq!(intake.recommendation, Some(1));
    assert_eq!(intake.severity, IntakeSeverity::Low);
    assert_eq!(intake.gap_type, Some(GapType::Partial));
    assert_eq!(intake.evidence, [evidence()]);
}

/// The control and bidi characters [`read_discrepancy_input`] must never
/// let through raw (the owner's terminal reads its message).
fn holds_raw_marks(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '\u{1b}' | '\u{202e}' | '\u{2066}'))
}

/// `read_discrepancy_input` that could not run (exit 2): its message.
fn input_cannot(outcome: Result<DiscrepancyInput, CliError>, context: &str) -> String {
    match outcome {
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
            error.message
        }
        Ok(input) => panic!("{context}: read as {input:?}"),
    }
}

/// Rules 1, escaped: what `--input`'s message quotes of the agent — an
/// unknown field's name and a bad enum value (the parser's quote), the
/// file's path (as given, relative to the current directory) — has its
/// control and bidi characters escaped (`\u{1b}`, `\u{202e}`, `\u{2066}`),
/// none raw, from stdin and from a file (missing, not UTF-8, not the
/// shape); exit 2, nothing stored; the binary's stderr alike. M:
/// `escaped_error` dropped from `read_discrepancy_input`.
#[test]
fn a_bad_input_document_is_quoted_escaped() {
    let pair = Pair::new("ai-input-esc", "spec-a");
    let cwd = pair.linked.clone();
    let env = pair.env(&cwd);
    let before = queue_state(&pair);
    let good = json!({"node_ids": ["EDGE-STAM-ZERO"], "summary": "S", "gap_type": "partial",
        "severity": "low", "evidence": [evidence()],
        "options": [option("a"), option("b")], "recommendation": 1});
    let mut unknown = good.clone();
    unknown["\u{1b}[2Jx\u{202e}"] = json!(1);
    let mut bad_enum = good.clone();
    bad_enum["gap_type"] = json!("\u{1b}[31mRED\u{202e}");
    let shape = "spec: --input -: not a discrepancy's arguments (`node_ids`, ";
    for (name, document, quoted) in [
        (
            "unknown field",
            &unknown,
            "unknown field `\\u{1b}[2Jx\\u{202e}`",
        ),
        (
            "bad enum",
            &bad_enum,
            "unknown variant `\\u{1b}[31mRED\\u{202e}`",
        ),
    ] {
        let message = input_cannot(
            read_discrepancy_input(
                &env,
                &IntakeSource::Given(document.to_string().into_bytes()),
            ),
            name,
        );
        assert!(message.starts_with(shape), "{name}: {message}");
        assert!(message.contains(quoted), "{name}: {message}");
        assert!(!holds_raw_marks(&message), "{name}: {message:?}");
    }

    let missing = "../in\u{1b}[2J\u{2066}put.json";
    let message = input_cannot(
        read_discrepancy_input(&env, &IntakeSource::File(missing.into())),
        "missing",
    );
    assert!(
        message.starts_with("spec: --input ../in\\u{1b}[2J\\u{2066}put.json: "),
        "{message}"
    );
    assert!(!holds_raw_marks(&message), "{message:?}");

    let not_utf8 = "../in\u{202e}put.json";
    write(&cwd, not_utf8, b"\xff");
    let message = input_cannot(
        read_discrepancy_input(&env, &IntakeSource::File(not_utf8.into())),
        "not UTF-8",
    );
    assert_eq!(
        message,
        "spec: --input ../in\\u{202e}put.json: not UTF-8 (at byte 0)"
    );

    let shaped = "../enum\u{1b}[31m.json";
    write(&cwd, shaped, bad_enum.to_string());
    let message = input_cannot(
        read_discrepancy_input(&env, &IntakeSource::File(shaped.into())),
        "bad enum in a file",
    );
    assert!(
        message.starts_with("spec: --input ../enum\\u{1b}[31m.json: not a discrepancy's arguments"),
        "{message}"
    );
    assert!(
        message.contains("unknown variant `\\u{1b}[31mRED\\u{202e}`"),
        "{message}"
    );
    assert!(!holds_raw_marks(&message), "{message:?}");
    assert_eq!(queue_state(&pair), before, "nothing stored");

    // The binary: `spec: ` and the same escaped message on stderr, exit 2.
    let run = pair.spec_piped(
        &cwd,
        &["propose", "discrepancy", "--input", "-"],
        unknown.to_string().as_bytes(),
    );
    assert_eq!(run.code, 2, "{run:?}");
    assert!(run.stderr.starts_with(shape), "{run:?}");
    assert!(
        run.stderr.contains("unknown field `\\u{1b}[2Jx\\u{202e}`"),
        "{run:?}"
    );
    assert!(!holds_raw_marks(&run.stderr), "{run:?}");
    let run = pair.spec_piped(&cwd, &["propose", "discrepancy", "--input", missing], b"");
    assert_eq!(run.code, 2, "{run:?}");
    assert!(
        run.stderr
            .starts_with("spec: --input ../in\\u{1b}[2J\\u{2066}put.json: "),
        "{run:?}"
    );
    assert!(!holds_raw_marks(&run.stderr), "{run:?}");
    assert_eq!(run.stdout, "");
    assert_eq!(queue_state(&pair), before, "nothing stored");
}

// ------------------------------------------------------------------ AC-07

/// `RULE-STAM-REGEN`'s span with `extra` appended to its first bullet.
fn regen_text(pair: &Pair, cwd: &Path, extra: &str) -> (String, String) {
    let (hash, text) = pair.span(cwd, "RULE-STAM-REGEN");
    let anchor = "(R-12).";
    assert_eq!(text.matches(anchor).count(), 1, "{text}");
    (hash, text.replacen(anchor, &format!("(R-12){extra}."), 1))
}

/// AC-07: a discrepancy whose patch cites the undeclared `R-99`: both
/// stored (`PR-0001` the discrepancy, `PR-0002` the update), `linked` both
/// ways, the finding in the intake's `diagnostics` and text and in the
/// update's brief review; the update is decided on its own (reviewed as an
/// update with its diff). A stale `base`: refused naming `proposed_patch`.
/// M: refused on findings; a `linked` missing.
#[test]
fn ac07_a_proposed_patch_is_a_linked_update_with_its_findings() {
    let pair = Pair::new("ai-ac07", "spec-a");
    let cwd = pair.linked.clone();
    let (hash, text) = regen_text(&pair, &cwd, ", R-99 sets the delay");
    let patch = |base: &str| ProposedPatch {
        target: "RULE-STAM-REGEN".to_owned(),
        base: base.to_owned(),
        text: text.clone(),
        rationale: "Name the delay's source.".to_owned(),
    };
    let before = queue_state(&pair);
    let mut input = discrepancy(&["EDGE-STAM-ZERO", "RULE-STAM-REGEN"], "The delay is 2 s.");
    input.distinct_from = Some(ids(&["DEC-0023"]));
    input.proposed_patch = Some(patch("b3:00"));
    let reason = refused(&pair, &before, &report(&pair, &cwd, input.clone()), "stale");
    assert!(reason.starts_with("proposed_patch: "), "{reason}");
    assert!(reason.contains(&hash), "names the current hash: {reason}");

    let mut other = patch(&hash);
    other.target = "MEC-STAMINA".to_owned();
    input.proposed_patch = Some(other);
    let reason = refused(
        &pair,
        &before,
        &report(&pair, &cwd, input.clone()),
        "target",
    );
    assert!(
        reason.starts_with("proposed_patch: its target `MEC-STAMINA` is not among node_ids"),
        "{reason}"
    );

    input.proposed_patch = Some(patch(&hash));
    let outcome = report(&pair, &cwd, input).expect("report");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let (printed_text, json) = printed_intake(&outcome);
    assert_eq!(json["id"], json!("PR-0001"), "{json}");
    assert_eq!(json["linked"], json!("PR-0002"), "{json}");
    assert_eq!(json["created"], json!(true));
    let subjects: Vec<&str> = json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["subject"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(subjects, ["R-99"], "{json}");
    assert!(
        printed_text.starts_with("PR-0001\nlinked: PR-0002\nintroduced: 1\n"),
        "{printed_text}"
    );
    assert!(printed_text.contains("R-99"), "{printed_text}");
    assert!(
        printed_text.contains("\nhit: DEC-0023 | accepted | docs/records/DEC/DEC-0023.md | "),
        "{printed_text}"
    );

    let reported = pair.proposal("PR-0001");
    let update = pair.proposal("PR-0002");
    assert_eq!(reported.kind, ProposalKind::Discrepancy);
    assert_eq!(reported.linked.as_deref(), Some("PR-0002"));
    assert_eq!(update.kind, ProposalKind::Update);
    assert_eq!(update.linked.as_deref(), Some("PR-0001"));
    assert_eq!(update.target_id, "RULE-STAM-REGEN");
    assert_eq!(update.new_text, text);
    assert_eq!(update.rationale, "Name the delay's source.");
    assert_eq!(update.author, reported.author);
    assert_eq!(
        update
            .diagnostics
            .iter()
            .map(|finding| finding.subject.clone())
            .collect::<Vec<_>>(),
        ["R-99"]
    );
    assert_eq!(
        pair.events()
            .iter()
            .map(|event| (event.event_type.as_str(), event.payload["id"].clone()))
            .collect::<Vec<_>>(),
        [
            ("proposal.created", json!("PR-0001")),
            ("proposal.created", json!("PR-0002"))
        ]
    );

    let brief = review_brief(
        &pair.env(&cwd),
        &Globals::default(),
        &ReviewRequest {
            id: "PR-0002".to_owned(),
            git: pair.git_env(&cwd),
        },
    )
    .expect("review --brief");
    let (_, brief_json) = printed(&brief);
    let brief_json = json_of(&brief_json);
    assert_eq!(brief_json["linked"], json!("PR-0001"));
    assert_eq!(brief_json["diagnostics"][0]["subject"], json!("R-99"));
    for dropped in ["base_text", "new_text", "diff", "conflict"] {
        assert!(brief_json[dropped].is_null(), "{dropped}: {brief_json}");
    }
    assert_eq!(brief_json["preview"], json!("applies"));
    let (full_text, _) = printed(&pair.review_ok(&cwd, "PR-0002"));
    assert!(full_text.contains("+- Base rate 10 units/s"), "{full_text}");
    let (discrepancy_text, discrepancy_json) = printed(&pair.review_ok(&cwd, "PR-0001"));
    let discrepancy_json = json_of(&discrepancy_json);
    assert_eq!(discrepancy_json["linked"], json!("PR-0002"));
    for absent in [
        "diff",
        "preview",
        "conflict",
        "base_text",
        "new_text",
        "rationale",
    ] {
        assert!(discrepancy_json[absent].is_null(), "{absent}");
    }
    assert!(
        discrepancy_text.contains("\nlinked: PR-0002\n"),
        "{discrepancy_text}"
    );

    // The linked update is decided on its own: applied, the discrepancy
    // still open.
    let applied = pair.approve_ok(&cwd, "PR-0002");
    assert_eq!(applied.document.status.as_deref(), Some("applied"));
    assert_eq!(pair.proposal("PR-0001").status, ProposalStatus::Open);
    assert_eq!(pair.proposal("PR-0002").linked.as_deref(), Some("PR-0001"));
}

// ---------------------------------------------------------------- Rules 5

/// Rules 5: a root in no git worktree cannot be bound to a place: both
/// kinds exit 2, nothing stored (no queue row in the data directory).
#[test]
fn a_root_without_git_exits_2() {
    let pair = Pair::new("ai-place", "spec-a");
    let plain = pair.scratch.copy("spec-a", "plain");
    let before = queue_state(&pair);
    let outcome = ask(
        &pair,
        &plain,
        &question(&pair, &plain, &["EDGE-STAM-ZERO"], QUESTION),
    );
    let message = cannot(&pair, &before, &outcome, "question");
    assert!(message.starts_with("spec: "), "{message}");
    let outcome = report(
        &pair,
        &plain,
        discrepancy(&["EDGE-STAM-ZERO"], "It departs."),
    );
    cannot(&pair, &before, &outcome, "discrepancy");
    assert!(pair.proposals().is_empty());
}

// ---------------------------------------------------------------- Rules 7

/// Rules 7: the queue's write lock held by another process over 5 s
/// (`sqlite3` in `BEGIN IMMEDIATE`): the ask exits 2 after the busy wait,
/// nothing stored; released, the same ask stores `PR-0001`.
#[test]
fn a_queue_busy_over_5_s_exits_2_and_stores_nothing() {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let pair = Pair::new("ai-busy", "spec-a");
    let cwd = pair.linked.clone();
    // The queue's tables exist: one ask that a hit leaves unstored.
    let request = question(&pair, &cwd, &["Q-031"], "Rest?");
    assert!(!asked(&pair, &cwd, &request).document.created);
    let mut holder = Command::new(common::proposal::sqlite3())
        .arg(pair.db())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("sqlite3 runs");
    let mut stdin = holder.stdin.take().expect("stdin");
    stdin
        .write_all(b"BEGIN IMMEDIATE;\nINSERT INTO events (project, type, payload, at) VALUES ('x', 'x', '{}', 'x');\n")
        .expect("lock");
    stdin.flush().expect("flush");
    std::thread::sleep(Duration::from_millis(500));
    let started = Instant::now();
    let outcome = ask(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    let waited = started.elapsed();
    stdin.write_all(b"ROLLBACK;\n.quit\n").expect("release");
    drop(stdin);
    let _ = holder.wait();
    match outcome {
        Err(error) => assert_eq!(error.exit, Exit::CannotRun, "{error}"),
        Ok(outcome) => panic!("asked under a held lock: {outcome:?}"),
    }
    assert!(waited >= Duration::from_secs(5), "gave up after {waited:?}");
    assert!(waited < Duration::from_secs(60), "waited {waited:?}");
    assert!(pair.proposals().is_empty(), "nothing stored");
    let stored = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    assert_eq!(stored.document.id.as_deref(), Some("PR-0001"));
}

/// Data: at most 10 hits and 10 related items are listed in full, the rest
/// named in a note as `distinct_from` names them (`<k> more hit(s) by name
/// only: …`, `<k> more related item(s) by name only: …`), by ID number; the
/// inbox cuts a summary's first line at 80 characters, ending in `…`, the
/// text and JSON alike.
#[test]
fn more_than_ten_matches_are_counted_in_a_note() {
    let pair = Pair::new("ai-many", "spec-a");
    let cwd = pair.linked.clone();
    let mut stored: Vec<String> = Vec::new();
    for _ in 0..12 {
        let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
        request.distinct_from = stored.clone();
        let outcome = asked(&pair, &cwd, &request);
        stored.push(outcome.document.id.clone().expect("stored"));
    }
    for index in 0..11 {
        let text = format!("Other question {index}: {}", "w".repeat(90));
        asked(
            &pair,
            &cwd,
            &question(&pair, &cwd, &["EDGE-STAM-ZERO"], &text),
        );
    }
    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    let (text, json) = printed_intake(&outcome);
    let listed: Vec<String> = (1..=10).map(|number| format!("PR-{number:04}")).collect();
    assert_eq!(hit_ids(&outcome), listed);
    assert_eq!(json["related"].as_array().unwrap().len(), 10);
    assert_eq!(json["related"][0]["id"], json!("PR-0013"));
    assert_eq!(
        json["notes"],
        json!([
            "2 more hit(s) by name only: PR-0011, PR-0012",
            "1 more related item(s) by name only: PR-0023"
        ])
    );
    assert!(!outcome.document.created);
    assert!(!outcome.unnameable, "12 hits can all be named");
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("hit: "))
            .count(),
        10
    );
    assert!(text.ends_with("not stored: name every hit in `distinct_from` to store it anyway\n"));
    assert_eq!(
        outcome.messages.len(),
        2,
        "the notes on stderr too: {:?}",
        outcome.messages
    );

    let (text, json) = printed_inbox(&pair.inbox(&cwd, false).expect("inbox"));
    let line = text.lines().nth(12).expect("PR-0013's line");
    let last = line.rsplit(" | ").next().unwrap();
    assert!(last.starts_with("normal: Other question 0: www"), "{line}");
    assert_eq!(last.chars().count(), "normal: ".len() + 80, "{line}");
    assert!(last.ends_with('\u{2026}'), "{line}");
    let summary = json_of(&json)["proposals"][12]["summary"]
        .as_str()
        .expect("summary")
        .to_owned();
    assert_eq!(summary.chars().count(), 80);
    assert_eq!(format!("normal: {summary}"), last);
}

/// Asks `QUESTION` on `EDGE-STAM-ZERO` `count` times, each naming every
/// earlier one in `distinct_from`, so each is stored: their IDs.
fn ask_repeatedly(pair: &Pair, cwd: &Path, count: usize) -> Vec<String> {
    let mut stored: Vec<String> = Vec::new();
    for _ in 0..count {
        let mut request = question(pair, cwd, &["EDGE-STAM-ZERO"], QUESTION);
        request.distinct_from = stored.clone();
        let outcome = asked(pair, cwd, &request);
        stored.push(outcome.document.id.clone().expect("stored"));
    }
    stored
}

/// `PR-0001`… for the ID numbers `numbers`.
fn pr_ids(numbers: std::ops::RangeInclusive<usize>) -> Vec<String> {
    numbers.map(|number| format!("PR-{number:04}")).collect()
}

/// The names an agent copies from an intake document into `distinct_from`:
/// the listed hits' names, then those the hits' note names.
fn names_to_copy(json: &Value) -> Vec<String> {
    let mut names: Vec<String> = json["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|hit| {
            hit["id"]
                .as_str()
                .or_else(|| hit["path"].as_str())
                .expect("a name")
                .to_owned()
        })
        .collect();
    let note = json["notes"][0].as_str().expect("the hits' note");
    let (_, listed) = note.split_once(" by name only: ").expect("names");
    let listed = listed.split("; ").next().unwrap_or_default();
    names.extend(listed.split(", ").map(str::to_owned));
    names
}

/// Data (iteration 2): past the ten listed in full, every hit is named in
/// the note as `distinct_from` takes it — 13 asks of one question → hits
/// `PR-0001`…`PR-0010`, the note `3 more hit(s) by name only: PR-0011,
/// PR-0012, PR-0013`, on stderr too; the 13 names copied from the document
/// into `distinct_from` store it (`PR-0014`, those names stored as given).
/// M: the names dropped from the note.
#[test]
fn hits_past_ten_are_named_so_distinct_from_takes_them() {
    let pair = Pair::new("ai-named", "spec-a");
    let cwd = pair.linked.clone();
    assert_eq!(ask_repeatedly(&pair, &cwd, 13), pr_ids(1..=13));
    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    let (text, json) = printed_intake(&outcome);
    assert_eq!(hit_ids(&outcome), pr_ids(1..=10));
    let note = "3 more hit(s) by name only: PR-0011, PR-0012, PR-0013";
    assert_eq!(json["notes"], json!([note]));
    assert_eq!(outcome.messages, [Message::Note(note.to_owned())]);
    assert!(
        !outcome.document.created && !outcome.unnameable,
        "{outcome:?}"
    );
    assert!(json["id"].is_null());
    assert!(
        text.ends_with("not stored: name every hit in `distinct_from` to store it anyway\n"),
        "{text}"
    );

    let names = names_to_copy(&json);
    assert_eq!(names, pr_ids(1..=13));
    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
    request.distinct_from = names.clone();
    let outcome = asked(&pair, &cwd, &request);
    assert!(outcome.document.created, "{outcome:?}");
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0014"));
    let stored = pair.proposal("PR-0014");
    assert_eq!(stored.intake.expect("its fields").distinct_from, names);
    assert_eq!(pair.proposals().len(), 14);
}

/// Data (iteration 2), the known limit: 65 stored asks of one question (the
/// 65th naming the 64 before it: `distinct_from`'s most) → the 66th lists 10
/// hits, names 54 (`PR-0011`…`PR-0064`), counts `1 more not listed`, and
/// states the limit in that note (JSON, stderr) and in the text's `not
/// stored:` line; `unnameable`; naming 64 of the hits still stores nothing;
/// 65 names are refused (`distinct_from: 65 entries; at most 64`, exit 1).
/// M: `NAMED_PAST_MAX` ignored; the names dropped; the unnameable check
/// removed.
#[test]
fn more_than_64_hits_are_a_stated_limit() {
    let pair = Pair::new("ai-limit", "spec-a");
    let cwd = pair.linked.clone();
    let stored = ask_repeatedly(&pair, &cwd, 65);
    assert_eq!(stored, pr_ids(1..=65));
    assert_eq!(
        pair.proposal("PR-0065")
            .intake
            .expect("its fields")
            .distinct_from
            .len(),
        64
    );
    let limit = "more than 64 hits: `distinct_from` names at most 64, so this item cannot be \
                 stored (a known limit)";
    let note = format!(
        "54 more hit(s) by name only: {}; 1 more not listed; {limit}",
        pr_ids(11..=64).join(", ")
    );
    let check = |outcome: &IntakeOutcome, context: &str| {
        assert_eq!(outcome.exit(), Exit::Answered, "{context}");
        assert!(outcome.unnameable, "{context}: {outcome:?}");
        assert!(!outcome.document.created, "{context}");
        let (text, json) = printed_intake(outcome);
        assert!(
            json["id"].is_null() && json["created"] == json!(false),
            "{context}"
        );
        assert_eq!(hit_ids(outcome), pr_ids(1..=10), "{context}");
        assert_eq!(json["notes"], json!([note]), "{context}");
        assert_eq!(
            outcome.messages,
            [Message::Note(note.clone())],
            "{context}: stderr"
        );
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("hit: "))
                .count(),
            10,
            "{context}"
        );
        assert!(
            text.ends_with(&format!("not stored: {limit}\n")),
            "{context}: {text}"
        );
        assert!(!text.contains("name every hit"), "{context}: {text}");
    };
    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    check(&outcome, "no distinct_from");
    let (_, json) = printed_intake(&outcome);
    assert_eq!(names_to_copy(&json), pr_ids(1..=64));

    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION);
    request.distinct_from = pr_ids(1..=64);
    check(&asked(&pair, &cwd, &request), "64 named");

    let before = queue_state(&pair);
    request.distinct_from = pr_ids(1..=65);
    let reason = refused(&pair, &before, &ask(&pair, &cwd, &request), "65 named");
    assert_eq!(reason, "distinct_from: 65 entries; at most 64");
    assert_eq!(pair.proposals().len(), 65, "nothing stored past the 65");
}

// ------------------------------------------------------------------ AC-08

/// AC-08: ESC, U+0085, U+202E and U+2066 in a summary, a working answer,
/// `observed` and a label: `inbox` and `review` text escaped (`\u{1b}`,
/// …), JSON raw; the inbox line `… | question | open | EDGE-STAM-ZERO |
/// t1 | <created_at> | normal: <text>`. M: one field raw.
#[test]
fn ac08_agent_text_is_escaped_in_text_and_raw_in_json() {
    let pair = Pair::new("ai-ac08", "spec-a");
    let cwd = pair.linked.clone();
    let marks = "\u{1b}[2J\u{85}\u{202e}\u{2066}";
    let escaped = "\\u{1b}[2J\\u{85}\\u{202e}\\u{2066}";
    let mut request = question(&pair, &cwd, &["EDGE-STAM-ZERO"], &format!("Stop{marks}?"));
    request.working_answer = format!("yes{marks}");
    asked(&pair, &cwd, &request);
    let mut input = discrepancy(&["EDGE-STAM-ZERO"], &format!("Departs{marks}."));
    input.working_answer = Some(format!("wait{marks}"));
    input.evidence[0].observed = format!("walks{marks}");
    input.options[1].label = format!("spec{marks}");
    let outcome = report(&pair, &cwd, input).expect("report");
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0002"));

    let (text, json) = printed_inbox(&pair.inbox(&cwd, false).expect("inbox"));
    assert_eq!(
        text,
        format!(
            "PR-0001 | question | open | EDGE-STAM-ZERO | t1 | {NOW} | normal: Stop{escaped}?\n\
             PR-0002 | discrepancy | open | EDGE-STAM-ZERO | t1 | {NOW} | high: Departs{escaped}.\n"
        )
    );
    assert!(!text.contains('\u{1b}') && !text.contains('\u{202e}'));
    let json = json_of(&json);
    assert_eq!(
        json["proposals"][0]["summary"],
        json!(format!("Stop{marks}?"))
    );
    assert_eq!(json["proposals"][0]["severity"], json!("normal"));
    assert!(json["proposals"][0]["rationale"].is_null());
    assert_eq!(
        json["proposals"][1]["summary"],
        json!(format!("Departs{marks}."))
    );

    let (text, json) = printed(&pair.review_ok(&cwd, "PR-0001"));
    assert!(
        text.contains(&format!("\nsummary: Stop{escaped}?\n")),
        "{text}"
    );
    assert!(
        text.contains(&format!("\nworking_answer: yes{escaped}\n")),
        "{text}"
    );
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{85}'),
        "{text}"
    );
    let json = json_of(&json);
    assert_eq!(json["summary"], json!(format!("Stop{marks}?")));
    assert_eq!(json["working_answer"], json!(format!("yes{marks}")));
    assert_eq!(json["severity"], json!("normal"));
    assert_eq!(json["evidence"], json!([]));
    assert_eq!(json["options"], json!([]));
    assert!(json["recommendation"].is_null() && json["gap_type"].is_null());

    let (text, json) = printed(&pair.review_ok(&cwd, "PR-0002"));
    for line in [
        format!("\nsummary: Departs{escaped}.\n"),
        format!("\nworking_answer: wait{escaped}\n"),
        format!(
            "\nevidence: 1\n  src/stamina.rs:3-9 stamina::regen | walks{escaped} | only at rest\n"
        ),
        format!(
            "\noptions: 2\n  [0] code | code: the effect | 1 item (recommended)\n  \
             [1] spec{escaped} | spec: the effect | 1 item\n"
        ),
        "\nrecommendation: 0\n".to_owned(),
        "\ngap_type: contradicts\n".to_owned(),
        "\nseverity: high\n".to_owned(),
    ] {
        assert!(text.contains(&line), "{line:?} in {text}");
    }
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{2066}'),
        "{text}"
    );
    let json = json_of(&json);
    assert_eq!(json["summary"], json!(format!("Departs{marks}.")));
    assert_eq!(
        json["evidence"][0]["observed"],
        json!(format!("walks{marks}"))
    );
    assert_eq!(json["options"][1]["label"], json!(format!("spec{marks}")));
    assert_eq!(json["working_answer"], json!(format!("wait{marks}")));
    assert_eq!(json["recommendation"], json!(0));
    assert!(json["price_of_other"].is_null());
}

// ------------------------------------------------------------------ AC-09

/// AC-09: library `approve` (consent yes) of a question and a discrepancy:
/// exit 1 naming `spec reject`, no prompt (the consent callback never
/// called), `dump()` and git unchanged; `reject`: `rejected`, the reason in
/// `decision_note`, `proposal.rejected`, git unchanged (the linked update
/// stays `open`). M: approve accepted.
#[test]
fn ac09_approve_refuses_and_reject_settles_a_question() {
    let pair = Pair::new("ai-ac09", "spec-a");
    let cwd = pair.linked.clone();
    asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    let (hash, text) = regen_text(&pair, &cwd, " at rest");
    let mut input = discrepancy(&["RULE-STAM-REGEN"], "The delay is 2 s.");
    input.distinct_from = Some(ids(&["DEC-0023"]));
    input.proposed_patch = Some(ProposedPatch {
        target: "RULE-STAM-REGEN".to_owned(),
        base: hash,
        text,
        rationale: "At rest.".to_owned(),
    });
    let outcome = report(&pair, &cwd, input).expect("report");
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0002"));
    let state = pair.state();
    let dump = queue_state(&pair);
    for id in ["PR-0001", "PR-0002"] {
        for cwd in [&pair.linked, &pair.main] {
            let (outcome, questions) = pair.approve_answer(cwd, id, true, pair.git_env(cwd));
            let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
            assert_eq!(outcome.exit(), Exit::NotFound, "{id}: {outcome:?}");
            assert_eq!(
                outcome.refusal.as_deref(),
                Some(
                    format!(
                        "`{id}` never applies: `spec reject {id} --reason <answer>` settles \
                         it; nothing changed"
                    )
                    .as_str()
                )
            );
            assert!(questions.is_empty(), "{id}: asked {questions:?}");
            assert_eq!(queue_state(&pair), dump, "{id}");
            assert_eq!(pair.state(), state, "{id}");
        }
    }

    reject_ok(&pair, &cwd, "PR-0001", "Yes, it stops.");
    reject_ok(&pair, &pair.main, "PR-0002", "Fix the code.");
    for (id, reason) in [("PR-0001", "Yes, it stops."), ("PR-0002", "Fix the code.")] {
        let stored = pair.proposal(id);
        assert_eq!(stored.status, ProposalStatus::Rejected, "{id}");
        assert_eq!(stored.decision_note.as_deref(), Some(reason), "{id}");
        assert_eq!(stored.decided_at.as_deref(), Some(LATER), "{id}");
        assert_eq!(
            stored.decided_by.as_deref(),
            Some(common::proposal::DECIDER),
            "{id}"
        );
        assert!(stored.applied_commit.is_none(), "{id}");
        assert_eq!(
            pair.events_of(id),
            [
                ("proposal.created".to_owned(), None),
                ("proposal.rejected".to_owned(), None)
            ]
        );
    }
    assert_eq!(pair.proposal("PR-0003").status, ProposalStatus::Open);
    let after = pair.state();
    assert_eq!(after.main_files, state.main_files, "no file written");
    assert_eq!(after.linked_files, state.linked_files, "no file written");
    assert_eq!(after.refs, state.refs, "no commit");
    let (outcome, _) = pair.reject_answer(&cwd, "PR-0001", "Again.", true);
    let outcome = outcome.expect("reject again");
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
}

/// The brief of a new kind (`get_proposal`'s twin): no diff, preview or
/// conflict, no apply step run; an update's brief (`propose update
/// --brief`) drops the texts and keeps the findings' count.
#[test]
fn the_brief_answers_leave_the_texts_out() {
    let pair = Pair::new("ai-brief", "spec-a");
    let cwd = pair.linked.clone();
    asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["EDGE-STAM-ZERO"], QUESTION),
    );
    let brief = review_brief(
        &pair.env(&cwd),
        &Globals::default(),
        &ReviewRequest {
            id: "PR-0001".to_owned(),
            git: pair.git_env(&cwd),
        },
    )
    .expect("review --brief");
    let (text, json) = printed(&brief);
    let json = json_of(&json);
    for key in [
        "diff",
        "preview",
        "conflict",
        "base_text",
        "new_text",
        "base_hash",
        "rationale",
    ] {
        assert!(json[key].is_null(), "{key}: {json}");
    }
    assert_eq!(json["kind"], json!("question"));
    assert_eq!(json["summary"], json!(QUESTION));
    assert_eq!(json["notes"], json!([]));
    assert!(text.contains("\nsummary: Does the sprint stop"), "{text}");

    let (hash, new_text) = regen_text(&pair, &cwd, ", R-99 and A-999 set it");
    let mut request = pair.request(&cwd, "RULE-STAM-REGEN", &hash, &new_text);
    request.rationale = "r".repeat(4096);
    let outcome = specengine_cli::propose_brief(&pair.env(&cwd), &Globals::default(), &request)
        .expect("propose --brief");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let (text, json) = printed(&outcome);
    let json = json_of(&json);
    assert_eq!(json["id"], json!("PR-0002"));
    for key in ["base_text", "new_text", "diff", "conflict"] {
        assert!(json[key].is_null(), "{key}: {json}");
    }
    assert_eq!(json["diagnostics"].as_array().unwrap().len(), 2);
    assert!(!text.contains("Base rate"), "{text}");
    assert_eq!(pair.proposal("PR-0002").new_text, new_text, "stored whole");
    request.rationale = "r".repeat(4097);
    let refusal = specengine_cli::propose(&pair.env(&cwd), &Globals::default(), &request);
    let reason = common::proposal::refused(&refusal, "rationale cap");
    assert!(
        reason.starts_with("rationale: 4097 bytes; at most 4096"),
        "{reason}"
    );
}
