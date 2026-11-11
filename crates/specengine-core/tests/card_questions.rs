//! docs/features/card-questions.md S3, the core half (RM-0097, ADR-0077):
//! a decision's `why` judged by `decision_problem` right after
//! `price_of_other` and before the patch; the record's `{{why}}` slot, the
//! sixteenth, free text in the body only, and its place in the known-slots
//! line of an unknown slot.
//!
//! - `why`: blank (whitespace) refused with "Data"'s line, then 2 049 bytes;
//!   1 024 `é` (2 048 bytes) read; absent read; a blank `why` beside a bad
//!   patch is `why`'s refusal; a bad price before a blank `why` is the
//!   price's.
//! - `Slot::ALL` is 16, `why` last (after `decided_by`), free text; the
//!   template's "is no slot" line names `{{why}}` last; `{{why}}` in the
//!   front-matter is a template error, in the body a slot the template
//!   holds. M: `why` accepted in the front-matter.
//!
//! Every expected value below is worked out from the spec, never read back
//! from a run.

use specengine_core::intake::{AuthorInput, DecisionInput, ProposedPatch, decision_problem};
use specengine_core::record::{Slot, Template};

/// An agent's identity, a role alone.
fn agent() -> AuthorInput<'static> {
    AuthorInput {
        role: Some("developer"),
        model: None,
        run: None,
    }
}

/// "Data"'s decision with `why`.
fn input(why: Option<&str>) -> DecisionInput {
    DecisionInput {
        node_ids: vec!["RULE-STAM-REGEN".to_owned()],
        text: "Does regeneration wait for rest?".to_owned(),
        working_answer: "Yes: it starts 1.5 s after the last stamina use.".to_owned(),
        price_of_other: "Without the wait R-28 is rebalanced.".to_owned(),
        why: why.map(str::to_owned),
        severity: None,
        proposed_patch: None,
        distinct_from: None,
    }
}

/// The problem of `input`, as `<field>: <problem>`.
fn problem(input: &DecisionInput) -> Option<String> {
    decision_problem(input, agent()).map(|found| found.to_string())
}

/// S3, core: `why` absent, 2 048 bytes of two-byte characters and "Data"'s
/// text read; whitespace alone, then 2 049 bytes, refused with "Data"'s
/// lines.
#[test]
fn s3_a_decisions_why_is_optional_not_blank_and_at_most_2048_bytes() {
    assert_eq!(problem(&input(None)), None);
    assert_eq!(
        problem(&input(Some(
            "Two playtests read instant regeneration as a bug."
        ))),
        None
    );
    let full = "\u{e9}".repeat(1024);
    assert_eq!(full.len(), 2048);
    assert_eq!(problem(&input(Some(&full))), None, "2 048 bytes read");
    for blank in ["", " ", " \n\t"] {
        assert_eq!(
            problem(&input(Some(blank))).as_deref(),
            Some("why: blank: give a value"),
            "{blank:?}"
        );
    }
    let over = format!("{full}a");
    assert_eq!(over.len(), 2049);
    assert_eq!(
        problem(&input(Some(&over))).as_deref(),
        Some("why: 2049 bytes; at most 2048")
    );
}

/// S3, core: `why` is judged after `price_of_other` and before the patch: a
/// blank `why` beside a bad patch (a blank rationale) is `why`'s refusal; a
/// blank price beside a blank `why` is the price's.
#[test]
fn s3_why_is_judged_after_the_price_and_before_the_patch() {
    let mut both = input(Some("  "));
    both.proposed_patch = Some(ProposedPatch {
        target: "EDGE-STAM-ZERO".to_owned(),
        base: "b3:00".to_owned(),
        text: "## Depletion {#EDGE-STAM-ZERO}\n".to_owned(),
        rationale: " ".to_owned(),
    });
    assert_eq!(problem(&both).as_deref(), Some("why: blank: give a value"));
    both.why = None;
    assert_eq!(
        problem(&both).as_deref(),
        Some("proposed_patch.rationale: blank: give a value")
    );

    let mut price = input(Some(" "));
    price.price_of_other = " ".to_owned();
    assert_eq!(
        problem(&price).as_deref(),
        Some("price_of_other: blank: give a value")
    );
}

/// The fixture template of `fixtures/spec-a`.
fn template() -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/spec-a");
    std::fs::read_to_string(root.join("templates/decision.md")).expect("spec-a's template")
}

/// S3, core: sixteen slots, `why` the last, after `decided_by`, free text;
/// an unknown slot's line lists the sixteen ending `{{why}}`; `{{why}}` in
/// the body is held, in the front-matter a template error at its line.
#[test]
fn s3_why_is_the_sixteenth_slot_free_text_in_the_body_alone() {
    assert_eq!(Slot::ALL.len(), 16);
    assert_eq!(Slot::ALL[15], Slot::Why);
    assert_eq!(Slot::ALL[14], Slot::DecidedBy);
    assert_eq!(Slot::Why.name(), "why");
    assert!(Slot::Why.is_free_text());
    assert_eq!(Slot::parse("why"), Some(Slot::Why));

    let base = template();
    let unknown = base.replacen("Note:\n", "Note: {{colour}}\n", 1);
    let error = Template::parse(&unknown).expect_err("an unknown slot");
    assert!(
        error.message.ends_with("`{{decided_by}}`, `{{why}}`")
            || error.message.contains("`{{decided_by}}`, `{{why}}`"),
        "{error}"
    );

    let body = base.replacen(
        "Cost:\n\n{{cost}}\n",
        "Cost:\n\n{{cost}}\n\nWhy:\n\n{{why}}\n",
        1,
    );
    let parsed = Template::parse(&body).expect("the body takes it");
    assert!(parsed.holds(Slot::Why));

    let front = base.replacen("title: {{title}}\n", "title: {{title}}\nwhy: {{why}}\n", 1);
    let error = Template::parse(&front).expect_err("free text in the front-matter");
    let line = front
        .lines()
        .position(|text| text == "why: {{why}}")
        .expect("the line")
        + 1;
    assert_eq!(error.line, line);
    assert!(error.message.contains("free text"), "{error}");
}
