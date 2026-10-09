//! docs/features/kind-interpretation.md, AC-T1 (the CLI half of AC-R2,
//! AC-R3, AC-R4, AC-R5, AC-R6, AC-R7, AC-R9, AC-R10, AC-R13) over a git copy
//! of `fixtures/spec-a` (`RULE-STAM-REGEN` in `DEC-0023`'s `canon:`
//! section, `DEC-0007` superseded by it), spec-b (`CMD-SYNC` under
//! `ADR-0001`'s non-Latin `canon:` slug, `ADR-0002` superseded) and the
//! non-Rust spec-c (no decision until one is added by its `class:`).
//!
//! The queue's commands run through the library with the sandbox's git
//! environment and an injected clock; the binary where its exit code,
//! stdout or stderr is the subject (`spec propose interpretation`, the
//! terminal's `interprets` line). Every refusal leaves the worktrees, the
//! refs and the queue as they were.
//!
//! Mutations (docs/features/kind-interpretation.md "Acceptance criteria"):
//! the status check dropped (AC-R3), the cap dropped (AC-R4), the trailer
//! dropped or required for completion (AC-R6), the key left out of
//! `--brief` (AC-R7), the refresh limited to `Update | Create` (AC-R9), the
//! W7 note absent or a refusal (AC-R10), Creation 3 skipped (AC-R2).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;
mod task_common;

use std::path::Path;

use common::decision::flags;
use common::proposal::{
    DECIDER, LATER, NOW, Pair, cannot, edit, json_of, printed, printed_inbox, refused,
};
use common::{read_text, replace, write};
use serde_json::{Value, json};
use specengine_cli::{
    CliError, Exit, ExportStateRequest, Globals, ImportStateRequest, InterpretRequest,
    ProposalOutcome, ProposeRequest, ProposedText, ReviewRequest, export_state, import_state,
    propose_interpretation, review_brief,
};
use specengine_store::{Interpretation, ProposalKind, ProposalStatus};
use task_common::Tasks;

const STAMINA: &str = "docs/spec/movement/stamina.md";
const DEC23: &str = "docs/records/DEC/DEC-0023.md";
const HOW: &str = "Rest is any speed under 40 % of walking.";
const WHY: &str = "DEC-0023 says rest; the rule names no speed.";
const RATIONALE: &str = "Carry DEC-0023 into the rule.\n\nRest gets a speed.";
/// The edit of `RULE-STAM-REGEN`'s first line.
const RATE: (&str, &str) = (
    "Base rate 10 units/s,",
    "Base rate 10 units/s at rest (under 40 % of walking),",
);
/// An edit of `EDGE-STAM-ZERO` (another section of the same file).
const ZERO: (&str, &str) = ("is applied immediately.", "is applied at once.");

fn example() -> Interpretation {
    Interpretation {
        decision: "DEC-0023".to_owned(),
        how: HOW.to_owned(),
        why: WHY.to_owned(),
    }
}

/// An agent's `spec propose interpretation` request of `text` against
/// `base`, from `cwd`, at [`NOW`].
fn request(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    base: &str,
    decision: &str,
    text: &str,
) -> InterpretRequest {
    InterpretRequest {
        target: target.to_owned(),
        base: base.to_owned(),
        decision: decision.to_owned(),
        how: HOW.to_owned(),
        why: WHY.to_owned(),
        text: ProposedText::Given(text.as_bytes().to_vec()),
        rationale: RATIONALE.to_owned(),
        author_role: Some("spec-author".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        now: NOW.to_owned(),
        git: pair.git_env(cwd),
    }
}

fn interpret_with(
    pair: &Pair,
    cwd: &Path,
    request: &InterpretRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    propose_interpretation(&pair.env(cwd), &Globals::default(), request, task)
}

/// `target`'s span read in `cwd` with `from` replaced by `to`, proposed
/// as an interpretation of `decision` (bound to `task`); it must be
/// stored: its ID.
fn interpret_edit(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    (from, to): (&str, &str),
    decision: &str,
    task: Option<&str>,
) -> String {
    let (hash, text) = pair.span(cwd, target);
    let outcome = interpret_with(
        pair,
        cwd,
        &request(pair, cwd, target, &hash, decision, &edit(&text, from, to)),
        task,
    )
    .unwrap_or_else(|error| panic!("interpret {target}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// [`interpret_edit`] of `RULE-STAM-REGEN` by [`RATE`] under `DEC-0023`.
fn interpret_ok(pair: &Pair, cwd: &Path) -> String {
    interpret_edit(pair, cwd, "RULE-STAM-REGEN", RATE, "DEC-0023", None)
}

/// The arguments of `spec propose interpretation TARGET --base B
/// --decision D --how H --why W --text-file - --rationale R
/// --author-role spec-author`.
fn cli_args<'a>(
    target: &'a str,
    base: &'a str,
    decision: &'a str,
    how: &'a str,
    why: &'a str,
) -> Vec<&'a str> {
    vec![
        "propose",
        "interpretation",
        target,
        "--base",
        base,
        "--decision",
        decision,
        "--how",
        how,
        "--why",
        why,
        "--text-file",
        "-",
        "--rationale",
        RATIONALE,
        "--author-role",
        "spec-author",
    ]
}

/// The review document's JSON text of `id`, read in `cwd`.
fn review_json(pair: &Pair, cwd: &Path, id: &str) -> String {
    printed(&pair.review_ok(cwd, id)).1
}

/// The update `spec propose update` stores of `target`'s span in `cwd`
/// with `from` replaced by `to`: the outcome (it must be stored).
fn update_ok(pair: &Pair, cwd: &Path, target: &str, from: &str, to: &str) -> ProposalOutcome {
    let (hash, text) = pair.span(cwd, target);
    let outcome = pair
        .propose(cwd, target, &hash, &edit(&text, from, to))
        .unwrap_or_else(|error| panic!("propose {target}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome
}

/// The W7 note naming `target` and `decisions` (the first carried).
fn w7(target: &str, decisions: &str, first: &str) -> String {
    format!(
        "{target} is in the `canon:` section of {decisions}: a change carrying one is `spec \
         propose interpretation {target} --decision {first}`"
    )
}

/// The `note:` lines of an outcome's messages.
fn notes_of(outcome: &ProposalOutcome) -> Vec<String> {
    outcome
        .messages
        .iter()
        .map(specengine_cli::Message::line)
        .filter(|line| line.starts_with("note: "))
        .collect()
}

// ------------------------------------------------------------------ AC-R2

/// AC-R2: the binary stores `PR-0001` (`introduced: 0`), nothing under
/// the root written, the row an interpretation with "Data"'s three
/// fields; a `{#ID}` added or dropped, or a stale base: exit 1, nothing
/// stored. M: skip Creation 3.
#[test]
fn ac_r2_propose_interpretation_stores_pr_0001_and_refuses_an_id_change() {
    let pair = Pair::new("ki-r2", "spec-a");
    let cwd = pair.linked.clone();
    let (hash, text) = pair.span(&cwd, "RULE-STAM-REGEN");
    let new_text = edit(&text, RATE.0, RATE.1);
    let before = pair.state();
    let run = pair.spec_piped(
        &cwd,
        &cli_args("RULE-STAM-REGEN", &hash, "DEC-0023", HOW, WHY),
        new_text.as_bytes(),
    );
    run.code(0);
    assert_eq!(run.stdout, "PR-0001\nintroduced: 0\n", "{}", run.show());
    assert_eq!(
        run.stderr,
        "",
        "no W7 note on an interpretation: {}",
        run.show()
    );
    let after = pair.state();
    assert_eq!(after.main_files, before.main_files);
    assert_eq!(after.linked_files, before.linked_files);
    assert_eq!(
        (after.main_status, after.linked_status, after.refs),
        (before.main_status, before.linked_status, before.refs),
        "nothing under the root"
    );
    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.kind, ProposalKind::Interpretation);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.target_id, "RULE-STAM-REGEN");
    assert_eq!(stored.target_path, STAMINA);
    assert_eq!(stored.base_hash, hash);
    assert_eq!(stored.new_text, new_text);
    assert_eq!(stored.rationale, RATIONALE);
    assert_eq!(stored.interpretation, Some(example()));
    assert_eq!(stored.place.branch, "t1");

    // Creation 3: a `{#ID}` added, one dropped; Creation 2: a stale base.
    let added = format!("{text}\n### Rest {{#RULE-STAM-REST}}\n- Under 40 %.\n");
    let dropped = text.replacen(" {#RULE-STAM-REGEN}", "", 1);
    assert_ne!(dropped, text);
    let stale = format!("b3:{}", "0".repeat(64));
    for (label, base, body) in [
        ("an ID added", hash.as_str(), added.as_str()),
        ("its ID dropped", hash.as_str(), dropped.as_str()),
        ("a stale base", stale.as_str(), new_text.as_str()),
    ] {
        let before = pair.state();
        let run = pair.spec_piped(
            &cwd,
            &cli_args("RULE-STAM-REGEN", base, "DEC-0023", HOW, WHY),
            body.as_bytes(),
        );
        run.code(1);
        assert!(!run.stderr.is_empty(), "{label}: {}", run.show());
        assert_eq!(pair.state(), before, "{label}: nothing stored or written");
        assert_eq!(pair.proposals().len(), 1, "{label}");
    }
}

// ------------------------------------------------------------------ AC-R3

/// AC-R3: each `--decision` that names no live accepted decision is
/// refused in its own words, the inbox unchanged: a superseded one (exit
/// 1 naming its successor), a rule (its class and path), an alias (naming
/// `DEC-0023`), a Cyrillic look-alike (exit 2, the Latin fix), `#SECTION`,
/// an unknown ID, an open question's and an update's `PR-`, a `DEC-`
/// document of another class, one without `class:`, a rejected and a
/// proposed decision, the target inside the decision. M: drop the status
/// check.
#[test]
fn ac_r3_a_decision_that_is_not_live_and_accepted_is_refused() {
    let pair = Pair::new("ki-r3", "spec-a");
    let main = pair.main.clone();
    // An alias of DEC-0023, and four DEC- documents that are not live
    // accepted decisions.
    replace(
        &main,
        DEC23,
        "id: DEC-0023\n",
        "id: DEC-0023\naliases: [DEC-0019]\n",
    );
    let record = |id: &str, head: &str| {
        format!("---\nid: {id}\n{head}date: 2026-09-02\n---\n\n# Record {id}\n\nA body.\n")
    };
    let documents = [
        ("DEC-0090", "class: canon\nstatus: accepted\n"),
        ("DEC-0091", "status: accepted\n"),
        ("DEC-0092", "class: decision\nstatus: rejected\n"),
        ("DEC-0093", "class: decision\nstatus: proposed\n"),
    ];
    for (id, head) in documents {
        write(
            &main,
            &format!("docs/records/DEC/{id}.md"),
            record(id, head),
        );
    }
    pair.commit_all(&main, "Records.");
    let question = pair.raise_question(
        &main,
        &["RULE-STAM-REGEN"],
        "Is rest a speed?",
        "Under 40 % of walking",
        "a rebalance",
    );
    let update = pair.propose_edit(&main, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    assert_eq!([question.as_str(), update.as_str()], ["PR-0001", "PR-0002"]);

    let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
    let new_text = edit(&text, RATE.0, RATE.1);
    let look_alike = "D\u{0415}C-0023";
    let cases: [(&str, i32, String); 16] = [
        (
            "DEC-0007",
            1,
            "--decision: DEC-0007 is superseded by DEC-0023: interpret DEC-0023; nothing stored"
                .to_owned(),
        ),
        (
            "RULE-STAM-REGEN",
            1,
            format!(
                "--decision: RULE-STAM-REGEN is no accepted decision: canon document {STAMINA}; \
                 nothing stored"
            ),
        ),
        ("DEC-0019", 1, "DEC-0023".to_owned()),
        (look_alike, 2, "DEC-0023".to_owned()),
        ("DEC-0023#regeneration", 1, "--decision".to_owned()),
        ("DEC-9999", 1, "--decision".to_owned()),
        (
            "PR-0001",
            1,
            "--decision: `PR-0001` is not recorded yet: `spec approve PR-0001` records it \
             first; nothing stored"
                .to_owned(),
        ),
        (
            "PR-0002",
            1,
            "--decision: `PR-0002` is an update: it records no decision; nothing stored".to_owned(),
        ),
        (
            "DEC-0090",
            1,
            "--decision: DEC-0090 is no accepted decision: canon document \
             docs/records/DEC/DEC-0090.md; nothing stored"
                .to_owned(),
        ),
        (
            "DEC-0091",
            1,
            "--decision: DEC-0091 is no accepted decision: document docs/records/DEC/DEC-0091.md \
             declares no `class:`; nothing stored"
                .to_owned(),
        ),
        (
            "DEC-0092",
            1,
            "--decision: DEC-0092 is rejected; nothing stored".to_owned(),
        ),
        (
            "DEC-0093",
            1,
            "--decision: DEC-0093 is no accepted decision: decision document \
             docs/records/DEC/DEC-0093.md; nothing stored"
                .to_owned(),
        ),
        (
            "docs/records/DEC/DEC-0007.md",
            1,
            "DEC-0007 is superseded by DEC-0023".to_owned(),
        ),
        // `@rev` and a wiki form: exit 1 naming the bare ID; `project:`
        // exit 2 (docs/features/kind-interpretation.md "The decision").
        (
            "DEC-0023@2",
            1,
            "--decision: `DEC-0023@2` carries a revision".to_owned(),
        ),
        ("[[DEC-0023]]", 1, "--decision: `[[DEC-0023]]`".to_owned()),
        ("shared:DEC-0023", 2, "shared:DEC-0023".to_owned()),
    ];
    for (decision, code, needle) in &cases {
        let before = pair.state();
        let run = pair.spec_piped(
            &main,
            &cli_args("RULE-STAM-REGEN", &hash, decision, HOW, WHY),
            new_text.as_bytes(),
        );
        assert_eq!(run.code, *code, "{decision}: {}", run.show());
        assert!(
            run.stderr.contains(needle.as_str()),
            "{decision}: want {needle:?}: {}",
            run.show()
        );
        assert_eq!(run.stdout, "", "{decision}: {}", run.show());
        assert_eq!(pair.state(), before, "{decision}: nothing stored");
    }
    // The look-alike's fix is Latin, `DEC-0023` named as written.
    let run = pair.spec_piped(
        &main,
        &cli_args("RULE-STAM-REGEN", &hash, look_alike, HOW, WHY),
        new_text.as_bytes(),
    );
    assert!(
        run.stderr.contains("`DEC-0023`") || run.stderr.contains(" DEC-0023"),
        "{}",
        run.show()
    );

    // The target inside the decision itself.
    let (own_hash, own_text) = pair.span(&main, "DEC-0023");
    let run = pair.spec_piped(
        &main,
        &cli_args("DEC-0023", &own_hash, "DEC-0023", HOW, WHY),
        edit(&own_text, "only at rest", "only at rest, under 40 %").as_bytes(),
    );
    run.code(1);
    assert!(
        run.stderr
            .contains("--decision: the target lies in DEC-0023 itself; nothing stored"),
        "{}",
        run.show()
    );
    // The inbox as before: the question and the update.
    let inbox = pair.inbox(&main, true).expect("inbox");
    let ids: Vec<&str> = inbox
        .proposals
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, ["PR-0001", "PR-0002"]);
}

/// AC-R3 and AC-R10's step-10 note: a question approved into `DEC-0024`
/// (`--canon RULE-STAM-REGEN`) notes `spec propose interpretation
/// RULE-STAM-REGEN --decision DEC-0024`; its `PR-` is then refused naming
/// the record, the record itself interpreted; a later update of the rule
/// is noted under both decisions, by path. M: the note's old text.
#[test]
fn ac_r10_the_step_10_note_names_interpretation_and_a_recorded_pr_is_refused() {
    let pair = Pair::new("ki-r10-step10", "spec-a");
    let main = pair.main.clone();
    let question = pair.raise_question(
        &main,
        &["RULE-STAM-REGEN"],
        "Is rest a speed?",
        "Under 40 % of walking",
        "a rebalance",
    );
    let (outcome, _) = pair.decide_ok(
        &main,
        &question,
        &flags(None, None, Some("RULE-STAM-REGEN")),
        None,
    );
    let record = pair.proposal(&question).record.expect("a record");
    assert_eq!(record.id, "DEC-0024");
    assert_eq!(
        notes_of(&outcome),
        [
            "note: DEC-0024 names RULE-STAM-REGEN in `canon:`, its text unchanged: `spec propose \
             interpretation RULE-STAM-REGEN --decision DEC-0024` carries it there"
        ]
    );

    let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
    let new_text = edit(&text, RATE.0, RATE.1);
    let before = pair.state();
    let outcome = interpret_with(
        &pair,
        &main,
        &request(&pair, &main, "RULE-STAM-REGEN", &hash, &question, &new_text),
        None,
    );
    assert_eq!(
        refused(&outcome, "a recorded PR"),
        format!("--decision: `{question}` is recorded as DEC-0024: name it; nothing stored")
    );
    assert_eq!(pair.state(), before);
    let id = interpret_edit(&pair, &main, "RULE-STAM-REGEN", RATE, "DEC-0024", None);
    assert_eq!(
        pair.proposal(&id)
            .interpretation
            .expect("its fields")
            .decision,
        "DEC-0024"
    );
    // An update of the rule now lies in two decisions' `canon:`.
    let outcome = update_ok(&pair, &main, "RULE-STAM-REGEN", ZERO_REGEN.0, ZERO_REGEN.1);
    assert_eq!(
        notes_of(&outcome),
        [format!(
            "note: {}",
            w7("RULE-STAM-REGEN", "DEC-0023, DEC-0024", "DEC-0023")
        )]
    );
}

/// An edit of `RULE-STAM-REGEN`'s heading, its ID kept.
const HEADING: (&str, &str) = ("## Regeneration {#", "## Regeneration at rest {#");

/// An edit of `RULE-STAM-REGEN`'s second line.
const ZERO_REGEN: (&str, &str) = ("rate \u{d7} 0.5.", "rate \u{d7} 0.25.");

/// AC-R3, step 4: `DEC-0023` superseded (committed in the proposal's
/// worktree) after the proposal: `spec approve` exits 1, one
/// `apply_failed` at step 4 naming the successor, nothing written, the
/// proposal open; the review's preview `unavailable` with the step's note.
/// M: drop the status check.
#[test]
fn ac_r3_a_decision_superseded_after_propose_fails_step_4() {
    let pair = Pair::new("ki-r3-step4", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    replace(
        &pair.linked,
        DEC23,
        "status: accepted\n",
        "status: superseded-by DEC-0099\n",
    );
    write(
        &pair.linked,
        "docs/records/DEC/DEC-0099.md",
        "---\nid: DEC-0099\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         canon: docs/spec/movement/stamina.md#regeneration\nsupersedes: [DEC-0023]\n---\n\n\
         # Rest is under 40 %\n\nRest is any speed under 40 % of walking.\n",
    );
    pair.commit_all(&pair.linked, "Supersede DEC-0023.");
    let why = "--decision: DEC-0023 is superseded by DEC-0099: interpret DEC-0099";
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.preview.map(|preview| preview.as_str()),
        Some("unavailable")
    );
    assert!(
        review
            .document
            .notes
            .contains(&format!("not applicable now (step 4): {why}")),
        "{:?}",
        review.document.notes
    );

    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "step 4");
    assert!(
        reason.contains("(step 4)") && reason.contains(why),
        "{reason}"
    );
    assert!(questions.is_empty(), "never asked: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    let failed: Vec<_> = pair
        .events_of(&id)
        .into_iter()
        .filter(|(kind, _)| kind == "proposal.apply_failed")
        .collect();
    assert_eq!(failed, [("proposal.apply_failed".to_owned(), Some(4))]);
    // A new proposal names the successor.
    interpret_edit(
        &pair,
        &pair.linked,
        "RULE-STAM-REGEN",
        ZERO_REGEN,
        "DEC-0099",
        None,
    );
}

// ------------------------------------------------------------------ AC-R4

/// AC-R4: `--how` or `--why` absent: exit 2 (the form); blank or 2 049
/// bytes: exit 1 `--how: 2049 bytes; at most 2048`; 2 048 bytes each
/// stored. M: drop the cap.
#[test]
fn ac_r4_how_and_why_are_required_not_blank_and_capped() {
    let pair = Pair::new("ki-r4", "spec-a");
    let main = pair.main.clone();
    let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
    let new_text = edit(&text, RATE.0, RATE.1);
    let full = cli_args("RULE-STAM-REGEN", &hash, "DEC-0023", HOW, WHY);
    for flag in ["--how", "--why", "--decision"] {
        let at = full.iter().position(|arg| *arg == flag).unwrap();
        let mut args = full.clone();
        args.drain(at..at + 2);
        let run = pair.spec_piped(&main, &args, new_text.as_bytes());
        run.code(2);
        assert!(run.stderr.contains(flag), "{flag}: {}", run.show());
        assert!(pair.proposals().is_empty(), "{flag}");
    }
    let over = "h".repeat(2049);
    let cap = "w".repeat(2048);
    for (label, how, why, needle) in [
        ("--how empty", "", WHY, "--how"),
        ("--how blank", " \t ", WHY, "--how"),
        ("--why blank", HOW, "\n", "--why"),
        (
            "--how over",
            over.as_str(),
            WHY,
            "--how: 2049 bytes; at most 2048",
        ),
        (
            "--why over",
            HOW,
            over.as_str(),
            "--why: 2049 bytes; at most 2048",
        ),
    ] {
        let before = pair.state();
        let run = pair.spec_piped(
            &main,
            &cli_args("RULE-STAM-REGEN", &hash, "DEC-0023", how, why),
            new_text.as_bytes(),
        );
        run.code(1);
        assert!(run.stderr.contains(needle), "{label}: {}", run.show());
        assert_eq!(pair.state(), before, "{label}: nothing stored");
    }
    let run = pair.spec_piped(
        &main,
        &cli_args("RULE-STAM-REGEN", &hash, "DEC-0023", &cap, &cap),
        new_text.as_bytes(),
    );
    run.code(0);
    let carried = pair.proposal("PR-0001").interpretation.expect("stored");
    assert_eq!((carried.how.len(), carried.why.len()), (2048, 2048));
}

/// AC-R4: a U+202E and a control in `how` are stored and printed raw by
/// `--json`, escaped in the review's text. M: the text unescaped.
#[test]
fn ac_r4_bidi_controls_are_escaped_in_text_and_raw_in_json() {
    let pair = Pair::new("ki-r4-bidi", "spec-a");
    let main = pair.main.clone();
    let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
    let how = "Rest \u{202e}tluaf\u{202c} under 40 %\u{7}";
    let mut req = request(
        &pair,
        &main,
        "RULE-STAM-REGEN",
        &hash,
        "DEC-0023",
        &edit(&text, RATE.0, RATE.1),
    );
    req.how = how.to_owned();
    let outcome = interpret_with(&pair, &main, &req, None).expect("propose");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let id = outcome.document.id.clone().expect("an ID");
    let (text_out, json_out) = printed(&pair.review_ok(&main, &id));
    assert!(
        !text_out.contains('\u{202e}') && !text_out.contains('\u{7}'),
        "escaped in text:\n{text_out}"
    );
    assert!(
        text_out.contains("  how the decision was applied:\n    Rest "),
        "{text_out}"
    );
    assert_eq!(
        json_of(&json_out)["interpretation"]["how"],
        json!(how),
        "raw in JSON"
    );
}

// ------------------------------------------------------------------ AC-R6

/// AC-R6: approved on a terminal (the binary on a pseudo-terminal),
/// `interprets DEC-0023 (<path>)` before the prompt; one commit, exactly
/// `M docs/spec/movement/stamina.md`, its message "Data"'s: the
/// rationale, the two labelled texts, the update's four trailers and
/// `Interprets: DEC-0023` last. M: drop the trailer.
#[test]
fn ac_r6_approve_commits_the_body_and_interprets_last() {
    let pair = Pair::new("ki-r6", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let base = pair.rev(&pair.linked, "HEAD");
    let run = pair.spec_pty(&pair.main, &["approve", &id], Some("y"));
    assert_eq!(run.code, 0, "{}", run.output);
    assert!(run.asked, "{}", run.output);
    let line = format!("interprets DEC-0023 ({DEC23})\n");
    let at_line = run
        .output
        .find(&line)
        .unwrap_or_else(|| panic!("{}", run.output));
    let at_prompt = run.output.find("[y/N]").expect("the prompt");
    assert!(at_line < at_prompt, "{}", run.output);
    assert!(
        run.output[at_line + line.len()..].starts_with(&format!("apply {id} to {STAMINA} on t1 ")),
        "the question follows: {}",
        run.output
    );

    let head = pair.rev(&pair.linked, "t1");
    assert_ne!(head, base);
    assert_eq!(pair.rev(&pair.linked, &format!("{head}^")), base);
    assert_eq!(
        pair.git_text(
            &pair.main,
            &["diff-tree", "--no-commit-id", "--name-status", "-r", &head]
        ),
        format!("M\t{STAMINA}")
    );
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    let decided_by = stored.decided_by.clone().expect("decided_by");
    assert_eq!(
        message,
        format!(
            "spec: apply {id}\n\n{RATIONALE}\n\nHow the decision was applied:\n{HOW}\n\n\
             Why interpretation was needed:\n{WHY}\n\nProposal: {id}\nDecided-by: {decided_by}\n\
             Proposed-by: agent role=spec-author model=claude-opus-5-5 run=unknown\n\
             Base-commit: {base}\n\
             Interprets: DEC-0023"
        )
    );
    let trailers = pair.git_text(
        &pair.main,
        &["log", "-1", "--format=%(trailers:only,unfold)", &head],
    );
    let keys: Vec<&str> = trailers
        .lines()
        .filter_map(|line| line.split_once(':').map(|(key, _)| key))
        .collect();
    assert_eq!(
        keys,
        [
            "Proposal",
            "Decided-by",
            "Proposed-by",
            "Base-commit",
            "Interprets"
        ]
    );
    let (_, now_text) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    assert!(now_text.contains(RATE.1), "{now_text}");
}

/// AC-R6 through the library: the consent question opens with the
/// `interprets` line; the decider as the sandbox's committer.
#[test]
fn ac_r6_the_consent_question_names_the_decision_first() {
    let pair = Pair::new("ki-r6-lib", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1);
    assert!(
        questions[0].starts_with(&format!(
            "interprets DEC-0023 ({DEC23})\napply {id} to {STAMINA} on t1 in {} (applies)? [y/N]",
            pair.linked.display()
        )),
        "{questions:?}"
    );
    assert_eq!(pair.proposal(&id).decided_by.as_deref(), Some(DECIDER));
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", "t1"]);
    assert!(message.ends_with("\nInterprets: DEC-0023"), "{message}");
}

/// AC-R6: a commit of another section after the proposal: the span is
/// unchanged, the apply `applies` on the new `HEAD` (the branch moved on),
/// both edits land, one commit `M <path>` with `Interprets:`; a commit of
/// another line of the same span: `rebases` (the merge); an overlapping
/// commit: exit 1 printing the conflict, nothing written.
#[test]
fn ac_r6_an_interpretation_rebases_or_is_refused_with_the_conflict() {
    let pair = Pair::new("ki-r6-rebase", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    replace(&pair.linked, STAMINA, ZERO.0, ZERO.1);
    pair.commit_all(&pair.linked, "Depletion.");
    assert_eq!(
        pair.review_ok(&pair.main, &id)
            .document
            .preview
            .map(|preview| preview.as_str()),
        Some("applies"),
        "another section: the span as proposed against"
    );
    let base = pair.rev(&pair.linked, "HEAD");
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    assert_eq!(outcome.expect("approve").exit(), Exit::Answered);
    assert!(questions[0].ends_with(" (applies)? [y/N]"), "{questions:?}");
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(pair.rev(&pair.linked, &format!("{head}^")), base);
    let file = read_text(&pair.linked, STAMINA);
    assert!(file.contains(RATE.1) && file.contains(ZERO.1), "{file}");
    assert_eq!(
        pair.git_text(
            &pair.main,
            &["diff-tree", "--no-commit-id", "--name-status", "-r", &head]
        ),
        format!("M\t{STAMINA}")
    );
    assert!(
        pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head])
            .ends_with("\nInterprets: DEC-0023")
    );

    // Another line of the same span: the merge.
    let merged = Pair::new("ki-r6-merge", "spec-a");
    // The proposal edits the span's last line, the commit its heading
    // (git's merge takes changes on lines that do not touch).
    let id = interpret_edit(
        &merged,
        &merged.linked,
        "RULE-STAM-REGEN",
        ZERO_REGEN,
        "DEC-0023",
        None,
    );
    replace(&merged.linked, STAMINA, HEADING.0, HEADING.1);
    merged.commit_all(&merged.linked, "The heading.");
    let base = merged.rev(&merged.linked, "HEAD");
    let (outcome, questions) =
        merged.approve_answer(&merged.main, &id, true, merged.git_env(&merged.main));
    assert_eq!(outcome.expect("approve").exit(), Exit::Answered);
    assert!(questions[0].ends_with(" (rebases)? [y/N]"), "{questions:?}");
    let head = merged.rev(&merged.linked, "t1");
    assert_eq!(merged.rev(&merged.linked, &format!("{head}^")), base);
    let (_, span) = merged.span(&merged.linked, "RULE-STAM-REGEN");
    assert!(
        span.contains(HEADING.1) && span.contains(ZERO_REGEN.1),
        "{span}"
    );
    assert!(
        merged
            .git_text(&merged.main, &["log", "-1", "--format=%B", &head])
            .ends_with("\nInterprets: DEC-0023")
    );

    // A conflict: the same line changed and committed after the proposal.
    let other = Pair::new("ki-r6-conflict", "spec-a");
    let id = interpret_ok(&other, &other.linked);
    replace(&other.linked, STAMINA, RATE.0, "Base rate 12 units/s,");
    other.commit_all(&other.linked, "Rate.");
    let before = other.state();
    let outcome = other.approve(&other.main, &id);
    let reason = refused(&outcome, "conflict");
    assert!(reason.contains("(step 5)"), "{reason}");
    let (stdout, json_text) = printed(outcome.as_ref().unwrap());
    assert!(
        stdout.contains("<<<<<<< current")
            && stdout.contains(">>>>>>> proposed")
            && stdout.contains(RATE.1),
        "{stdout}"
    );
    assert_eq!(json_of(&json_text)["preview"], "conflicts");
    assert_eq!(other.state(), before, "nothing written");
    assert_eq!(other.proposal(&id).status, ProposalStatus::Open);
}

/// The proposal's text committed by hand in its worktree, the message
/// `By hand.` and `Proposal: <id>` alone (no `Interprets:`): that commit.
fn hand_commit(pair: &Pair, id: &str) -> String {
    replace(&pair.linked, STAMINA, RATE.0, RATE.1);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            "By hand.",
            "-m",
            &format!("Proposal: {id}"),
            "--",
            STAMINA,
        ],
    );
    pair.rev(&pair.linked, "HEAD")
}

/// AC-R6, completion: the edit committed by hand with `Proposal: PR-…`
/// alone (no `Interprets:`): `spec approve` completes it with that commit,
/// no second commit, no failure. M: require the trailer for completion.
#[test]
fn ac_r6_a_hand_commit_with_proposal_alone_completes_it() {
    let pair = Pair::new("ki-r6-complete", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let hand = hand_commit(&pair, &id);
    assert!(
        !pair
            .git_text(&pair.main, &["log", "-1", "--format=%B", &hand])
            .contains("Interprets:")
    );
    let outcome = pair.approve_ok(&pair.main, &id);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied, "{outcome:?}");
    assert_eq!(stored.applied_commit.as_deref(), Some(hand.as_str()));
    assert_eq!(pair.rev(&pair.linked, "t1"), hand, "no second commit");
    assert!(
        !pair
            .events_of(&id)
            .iter()
            .any(|(kind, _)| kind == "proposal.apply_failed"),
        "{:?}",
        pair.events_of(&id)
    );
}

/// AC-R6 / "Description and interactions", Approve: "Completion matches
/// `Proposal:` only: a commit without `Interprets:` completes it, the
/// decision not re-checked". `DEC-0023` superseded (its own commit), then
/// the proposal's text committed by hand with `Proposal:` alone:
/// `spec approve` completes it with the hand commit.
#[test]
fn ac_r6_a_completion_does_not_check_the_decision_again() {
    let pair = Pair::new("ki-r6-complete-superseded", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    replace(
        &pair.linked,
        DEC23,
        "status: accepted\n",
        "status: superseded-by DEC-0099\n",
    );
    pair.commit_all(&pair.linked, "Supersede DEC-0023.");
    let hand = hand_commit(&pair, &id);
    let outcome = pair.approve(&pair.main, &id);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "the completion re-checked the decision: {:?}",
        outcome.refusal
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(hand.as_str()));
}

/// AC-R6: `--option 0`, `--answer A`, `--canon X` on an interpretation:
/// exit 2 naming the kind, nothing changed.
#[test]
fn ac_r6_a_decision_flag_on_an_interpretation_is_exit_2() {
    let pair = Pair::new("ki-r6-flags", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let before = pair.state();
    for (flag, given) in [
        ("--option", flags(Some(0), None, None)),
        ("--answer", flags(None, Some("A"), None)),
        ("--canon", flags(None, None, Some("MEC-STAMINA"))),
    ] {
        let (outcome, questions) = pair.decide(&pair.main, &id, &given, None, true);
        let message = cannot(&outcome, flag);
        assert_eq!(
            message,
            format!(
                // The deciding kinds by `decides()` (docs/features/kind-decision.md).
                "spec: `{flag}` decides a `question`, `discrepancy` or `decision`; `{id}` is an \
                 interpretation: `spec approve {id}` applies it as proposed; nothing changed"
            )
        );
        assert!(questions.is_empty());
        assert_eq!(pair.state(), before, "{flag}");
    }
    for args in [
        vec!["approve", id.as_str(), "--option", "0"],
        vec!["approve", id.as_str(), "--answer", "A"],
        vec!["approve", id.as_str(), "--canon", "MEC-STAMINA"],
    ] {
        // On a terminal (a pipe is refused before the flags are read).
        let run = pair.spec_pty(&pair.main, &args, None);
        assert_eq!(run.code, 2, "{args:?}: {}", run.output);
        assert!(!run.asked, "{args:?}: never asked: {}", run.output);
        assert!(
            run.output.contains("is an interpretation: `spec approve"),
            "{args:?}: {}",
            run.output
        );
    }
    assert_eq!(pair.state(), before);
}

// ------------------------------------------------------------------ AC-R7

/// AC-R7: the review document's 46 keys, `interpretation` after
/// `staged_at` ("Data"'s object; `null` on an update); its text block;
/// kept by `--brief` (the texts dropped); the inbox line `PR-0001 |
/// interpretation | open | RULE-STAM-REGEN | t1 | <created_at> | <the
/// rationale's first line>`, its JSON entry an update's keys. M: dropped
/// by `--brief`.
#[test]
fn ac_r7_review_brief_and_inbox_carry_the_interpretation() {
    let pair = Pair::new("ki-r7", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let update = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    let raw = review_json(&pair, &pair.main, &id);
    let keys = task_common::keys(&raw);
    assert_eq!(keys.len(), 46, "{keys:?}");
    assert_eq!(
        keys[42..],
        ["staged", "staged_at", "interpretation", "notes"],
        "{keys:?}"
    );
    assert!(
        raw.contains(&format!(
            "\"staged_at\":null,\"interpretation\":{{\"decision\":\"DEC-0023\",\"how\":\"{HOW}\",\
             \"why\":\"{WHY}\"}},\"notes\":"
        )),
        "{raw}"
    );
    let value = json_of(&raw);
    assert_eq!(value["kind"], json!("interpretation"));
    assert_eq!(
        value["interpretation"],
        json!({"decision": "DEC-0023", "how": HOW, "why": WHY})
    );
    // An update's review: the key `null`, the text `-`.
    let update_raw = review_json(&pair, &pair.main, &update);
    assert_eq!(task_common::keys(&update_raw), keys);
    assert_eq!(json_of(&update_raw)["interpretation"], Value::Null);
    let (update_text, _) = printed(&pair.review_ok(&pair.main, &update));
    assert!(
        update_text.contains("\ninterpretation: -\nnotes:"),
        "{update_text}"
    );

    // The text block, after `staged_at`, before `notes`.
    let (text, _) = printed(&pair.review_ok(&pair.main, &id));
    assert!(
        text.contains(&format!(
            "\nstaged_at: -\ninterpretation:\n  decision: DEC-0023\n  how the decision was \
             applied:\n    {HOW}\n  why interpretation was needed:\n    {WHY}\nnotes:"
        )),
        "{text}"
    );

    // `--brief` keeps it, drops the texts.
    let brief = review_brief(
        &pair.env(&pair.main),
        &Globals::default(),
        &ReviewRequest {
            id: id.clone(),
            git: pair.git_env(&pair.main),
        },
    )
    .expect("review --brief");
    let (_, brief_json) = printed(&brief);
    let brief_value = json_of(&brief_json);
    assert_eq!(brief_value["interpretation"], value["interpretation"]);
    assert_eq!(brief_value["new_text"], Value::Null);
    assert_eq!(task_common::keys(&brief_json).len(), 46);
    // The binary's `--brief --json` too.
    let run = pair.spec_piped(&pair.main, &["--json", "review", &id, "--brief"], b"");
    run.code(0);
    assert_eq!(run.json()["interpretation"], value["interpretation"]);

    // The inbox line and its JSON entry.
    let inbox = pair.inbox(&pair.main, false).expect("inbox");
    let (inbox_text, inbox_json) = printed_inbox(&inbox);
    let created = pair.proposal(&id).created_at;
    assert!(
        inbox_text.contains(&format!(
            "{id} | interpretation | open | RULE-STAM-REGEN | t1 | {created} | Carry DEC-0023 \
             into the rule.\n"
        )),
        "{inbox_text}"
    );
    let entries = json_of(&inbox_json)["proposals"].clone();
    let entries = entries.as_array().expect("entries");
    let keys_of = |entry: &Value| {
        entry
            .as_object()
            .expect("an entry")
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keys_of(&entries[0]),
        keys_of(&entries[1]),
        "an update's keys"
    );
    assert!(entries[0].get("interpretation").is_none());
}

/// AC-R5, the CLI half: the column set on an update, `NULL` on an
/// interpretation, keys reordered: `spec review` fails naming the row and
/// the column, `spec inbox` notes the row and lists the rest.
#[test]
fn ac_r5_a_corrupt_interpretation_row_is_named_by_review_and_inbox() {
    let pair = Pair::new("ki-r5", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let update = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    let good = pair.sql(&format!(
        "SELECT interpretation FROM proposals WHERE id = '{id}';"
    ));
    for (label, sql, broken) in [
        (
            "the column on an update",
            format!(
                "UPDATE proposals SET interpretation = '{{\"decision\":\"DEC-0023\",\"how\":\"h\",\
                 \"why\":\"w\"}}' WHERE id = '{update}';"
            ),
            update.as_str(),
        ),
        (
            "NULL on an interpretation",
            format!("UPDATE proposals SET interpretation = NULL WHERE id = '{id}';"),
            id.as_str(),
        ),
        (
            "keys reordered",
            format!(
                "UPDATE proposals SET interpretation = '{{\"how\":\"h\",\"decision\":\"DEC-0023\",\
                 \"why\":\"w\"}}' WHERE id = '{id}';"
            ),
            id.as_str(),
        ),
    ] {
        pair.sql(&sql);
        let message = match pair.review(&pair.main, broken) {
            Err(error) => error.message,
            Ok(outcome) => panic!("{label}: read as {outcome:?}"),
        };
        assert!(
            message.contains(broken) && message.contains("`interpretation`"),
            "{label}: {message}"
        );
        let inbox = pair.inbox(&pair.main, true).expect("inbox lists the rest");
        assert!(
            inbox
                .notes
                .iter()
                .any(|note| note.contains(broken) && note.contains("`interpretation`")),
            "{label}: the row noted: {:?}",
            inbox.notes
        );
        let (_, inbox_json) = printed_inbox(&inbox);
        assert!(
            json_of(&inbox_json)["notes"]
                .as_array()
                .is_some_and(|notes| !notes.is_empty()),
            "{label}: {inbox_json}"
        );
        assert_eq!(inbox.proposals.len(), 1, "{label}: {inbox:?}");
        // Restored for the next case.
        pair.sql(&format!(
            "UPDATE proposals SET interpretation = NULL WHERE id = '{update}'; UPDATE proposals \
             SET interpretation = '{}' WHERE id = '{id}';",
            good.trim().replace('\'', "''")
        ));
        assert_eq!(pair.inbox(&pair.main, true).unwrap().proposals.len(), 2);
    }
}

// ------------------------------------------------------------------ AC-R9

/// AC-R9: bound to a `ready` task whose snapshot holds `RULE-STAM-REGEN`:
/// the package's `open_proposals` lists it; applied in the task's place:
/// one `task.refreshed`, the snapshot hash the applied one. M: the refresh
/// limited to `Update | Create`.
#[test]
fn ac_r9_a_bound_interpretation_is_listed_and_refreshes_the_snapshot() {
    let pair = Pair::new("ki-r9", "spec-a");
    let main = pair.main.clone();
    let task = pair.new_ok(&main, &["RULE-STAM-REGEN"]);
    pair.plan_ok(&main, &task, &[], &[]);
    pair.approve_task(&main, &task);
    let frozen = |pair: &Pair| -> Vec<(String, String)> {
        pair.package(&pair.main, &task)["spec_snapshot"]["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| {
                (
                    node["id"].as_str().unwrap().to_owned(),
                    node["span_hash"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let before = frozen(&pair);
    assert_eq!(
        before.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["RULE-STAM-REGEN"]
    );
    let id = interpret_edit(
        &pair,
        &main,
        "RULE-STAM-REGEN",
        RATE,
        "DEC-0023",
        Some(&task),
    );
    assert_eq!(pair.proposal(&id).task_id.as_deref(), Some(task.as_str()));
    let package = pair.package(&main, &task);
    let listed: Vec<(&str, &str)> = package["open_proposals"]
        .as_array()
        .expect("open_proposals")
        .iter()
        .map(|entry| {
            (
                entry["id"].as_str().unwrap_or_default(),
                entry["kind"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(listed, [(id.as_str(), "interpretation")], "{package}");
    assert!(
        package["assumptions"]
            .as_array()
            .is_none_or(|assumptions| assumptions.is_empty()),
        "no assumption: {package}"
    );

    pair.approve_ok(&main, &id);
    let refreshed: Vec<(String, String)> = pair
        .task_events()
        .into_iter()
        .filter(|(kind, _)| kind == "task.refreshed")
        .map(|(_, payload)| {
            (
                payload["proposal"].as_str().unwrap().to_owned(),
                payload["node"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(refreshed, [(id.clone(), "RULE-STAM-REGEN".to_owned())]);
    let after = frozen(&pair);
    assert_eq!(after[0].1, pair.span(&main, "RULE-STAM-REGEN").0);
    assert_ne!(after, before);
    assert_eq!(pair.package(&main, &task)["stale"], json!(false));
}

// ----------------------------------------------------------------- AC-R10

/// AC-R10 (W7) on spec-a: an update of `RULE-STAM-REGEN` is stored (exit
/// 0) with the note naming `DEC-0023`, its review notes it too (open), an
/// applied one's does not; the file's path target (its document encloses
/// the section) is noted, `EDGE-STAM-ZERO` not; an interpretation is not
/// noted. The binary prints the note on stderr. M: no note, or a refusal.
#[test]
fn ac_r10_an_update_in_a_decisions_canon_section_is_noted_never_refused() {
    let pair = Pair::new("ki-r10", "spec-a");
    let main = pair.main.clone();
    let note = w7("RULE-STAM-REGEN", "DEC-0023", "DEC-0023");
    let outcome = update_ok(&pair, &main, "RULE-STAM-REGEN", RATE.0, RATE.1);
    let id = outcome.document.id.clone().expect("stored");
    assert_eq!(notes_of(&outcome), [format!("note: {note}")]);
    assert!(outcome.document.notes.contains(&note), "{outcome:?}");
    let review = pair.review_ok(&main, &id);
    assert!(review.document.notes.contains(&note), "{review:?}");

    // EDGE-STAM-ZERO lies outside the `canon:` section.
    let zero = update_ok(&pair, &main, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    assert_eq!(notes_of(&zero), Vec::<String>::new());
    let zero_id = zero.document.id.clone().expect("stored");
    assert!(
        !pair
            .review_ok(&main, &zero_id)
            .document
            .notes
            .iter()
            .any(|line| line.contains("`canon:` section")),
        "no note"
    );

    // The file's path: its document encloses the section.
    let (hash, text) = pair.span(&main, STAMINA);
    let outcome = pair
        .propose(
            &main,
            STAMINA,
            &hash,
            &edit(
                &text,
                "Stamina limits sprinting.",
                "Stamina limits running.",
            ),
        )
        .expect("propose the file");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let target = outcome.document.target_id.clone().expect("a target");
    assert_eq!(
        notes_of(&outcome),
        [format!("note: {}", w7(&target, "DEC-0023", "DEC-0023"))]
    );

    // An interpretation is not noted.
    let interpreted = {
        let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
        interpret_with(
            &pair,
            &main,
            &request(
                &pair,
                &main,
                "RULE-STAM-REGEN",
                &hash,
                "DEC-0023",
                &edit(&text, RATE.0, RATE.1),
            ),
            None,
        )
        .expect("interpret")
    };
    assert_eq!(notes_of(&interpreted), Vec::<String>::new());
    let interpreted_id = interpreted.document.id.clone().expect("stored");
    assert!(
        !pair
            .review_ok(&main, &interpreted_id)
            .document
            .notes
            .iter()
            .any(|line| line.contains("`canon:` section"))
    );

    // The binary: exit 0, the note on stderr.
    let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
    let run = pair.spec_piped(
        &main,
        &[
            "propose",
            "update",
            "RULE-STAM-REGEN",
            "--base",
            &hash,
            "--text-file",
            "-",
            "--rationale",
            "Rest.",
        ],
        edit(&text, ZERO_REGEN.0, ZERO_REGEN.1).as_bytes(),
    );
    run.code(0);
    assert!(
        run.stderr.contains(&format!("note: {note}\n")),
        "{}",
        run.show()
    );

    // Applied: its review notes nothing more.
    pair.approve_ok(&main, &id);
    assert!(
        !pair
            .review_ok(&main, &id)
            .document
            .notes
            .iter()
            .any(|line| line.contains("`canon:` section"))
    );
}

/// AC-R10 on spec-b: `CMD-SYNC` lies in `ADR-0001`'s non-Latin `canon:`
/// slug: an update is noted naming `ADR-0001` alone (the superseded
/// `ADR-0002`, whose reference-form `canon:` names it too, is not live);
/// an interpretation of `ADR-0001` is stored, of `ADR-0002` refused naming
/// its successor.
#[test]
fn ac_r10_spec_b_notes_adr_0001_and_interprets_it() {
    let pair = Pair::new("ki-r10-b", "spec-b");
    let main = pair.main.clone();
    let outcome = update_ok(&pair, &main, "CMD-SYNC", "`sync`", "`sync --all`");
    assert_eq!(
        notes_of(&outcome),
        [format!("note: {}", w7("CMD-SYNC", "ADR-0001", "ADR-0001"))]
    );
    let (hash, text) = pair.span(&main, "CMD-SYNC");
    let new_text = edit(&text, "`sync`", "`sync --each`");
    let outcome = interpret_with(
        &pair,
        &main,
        &request(&pair, &main, "CMD-SYNC", &hash, "ADR-0002", &new_text),
        None,
    );
    assert_eq!(
        refused(&outcome, "ADR-0002"),
        "--decision: ADR-0002 is superseded by ADR-0001: interpret ADR-0001; nothing stored"
    );
    let outcome = interpret_with(
        &pair,
        &main,
        &request(&pair, &main, "CMD-SYNC", &hash, "ADR-0001", &new_text),
        None,
    )
    .expect("interpret");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let id = outcome.document.id.clone().expect("stored");
    assert_eq!(
        pair.proposal(&id).interpretation.expect("fields").decision,
        "ADR-0001"
    );

    // A live decision whose `canon:` is reference-form (`MOD-CLI#CMD-SYNC`,
    // as `spec check` resolves it): named beside ADR-0001, by path; a
    // section inside `CMD-SYNC` (`FLAG-DRY-RUN`) lies in both.
    write(
        &main,
        "docs/records/ADR/ADR-0003.md",
        "---\nid: ADR-0003\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         scope: [sync]\ncanon: MOD-CLI#CMD-SYNC\n---\n\n# Sync keeps its worktree\n\nA body.\n",
    );
    pair.commit_all(&main, "ADR-0003.");
    let outcome = update_ok(&pair, &main, "CMD-SYNC", "`sync`", "`sync --all`");
    assert_eq!(
        notes_of(&outcome),
        [format!(
            "note: {}",
            w7("CMD-SYNC", "ADR-0001, ADR-0003", "ADR-0001")
        )]
    );
    let (hash, text) = pair.span(&main, "FLAG-DRY-RUN");
    let outcome = pair
        .propose(
            &main,
            "FLAG-DRY-RUN",
            &hash,
            &edit(&text, "dry-run/CRIT-01", "dry-run/CRIT-01 (dry run)"),
        )
        .expect("propose");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        notes_of(&outcome),
        [format!(
            "note: {}",
            w7("FLAG-DRY-RUN", "ADR-0001, ADR-0003", "ADR-0001")
        )]
    );
}

/// The non-Rust spec-c (ADR-0008): no document of class `decision`, so
/// `OPEN-01` is refused by its class; a `class: decision` document added
/// under the project's own `OPEN` prefix (no `[ids]` kind `decision`) is
/// told by its class: an update of `FEE-DAY-CAP` is noted under it and an
/// interpretation of it stored.
#[test]
fn spec_c_a_decision_is_told_by_its_class_not_an_ids_kind() {
    let pair = Pair::new("ki-spec-c", "spec-c");
    let main = pair.main.clone();
    let (hash, text) = pair.span(&main, "FEE-DAY-CAP");
    let lines: Vec<&str> = text.lines().collect();
    let last = lines.last().expect("a line").to_owned();
    let new_text = edit(&text, last, &format!("{last} (per calendar day)"));
    let outcome = interpret_with(
        &pair,
        &main,
        &request(&pair, &main, "FEE-DAY-CAP", &hash, "OPEN-01", &new_text),
        None,
    );
    assert_eq!(
        refused(&outcome, "OPEN-01"),
        "--decision: OPEN-01 is no accepted decision: canon document \
         docs/records/OPEN/OPEN-01.md; nothing stored"
    );
    write(
        &main,
        "docs/records/OPEN/OPEN-02.md",
        "---\nid: OPEN-02\nclass: decision\nstatus: accepted\ndate: 2026-10-02\n\
         canon: FEE-DAY-CAP\n---\n\n# The cap counts calendar days\n\nA day is a calendar day.\n",
    );
    pair.commit_all(&main, "A decision.");
    let (hash, text) = pair.span(&main, "FEE-DAY-CAP");
    let new_text = edit(&text, last, &format!("{last} (per calendar day)"));
    let outcome = interpret_with(
        &pair,
        &main,
        &request(&pair, &main, "FEE-DAY-CAP", &hash, "OPEN-02", &new_text),
        None,
    )
    .expect("interpret");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let update = pair
        .propose(&main, "FEE-DAY-CAP", &hash, &new_text)
        .expect("update");
    assert_eq!(
        notes_of(&update),
        [format!("note: {}", w7("FEE-DAY-CAP", "OPEN-02", "OPEN-02"))]
    );
}

// ----------------------------------------------------------------- AC-R13

/// AC-R13: `export state` with an interpretation: header `{"format":2,
/// "queue_schema":6,…}`, its row's `interpretation` the stored JSON text;
/// imported into a fresh `HOME`, the export byte-identical, the row read
/// equal. A schema-5 dump whose row carries `interpretation` is refused
/// naming its line. M: import refuses schema 5 (`stage_confirm.rs` holds
/// the schema-5 restore).
#[test]
fn ac_r13_a_backup_with_an_interpretation_round_trips_byte_identical() {
    let pair = Pair::new("ki-r13", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    let dumps = pair.scratch.dir("dumps");
    let export = |home: &Path, name: &str| -> Vec<u8> {
        let file = dumps.join(name);
        export_state(
            &specengine_cli::Env {
                cwd: pair.main.clone(),
                home: Some(home.as_os_str().to_os_string()),
                xdg_data_home: None,
            },
            &Globals::default(),
            &ExportStateRequest {
                out: Some(file.clone()),
                now: LATER.to_owned(),
                git: pair.git_env(&pair.main),
            },
        )
        .unwrap_or_else(|error| panic!("export: {error}"));
        std::fs::read(file).expect("the dump")
    };
    let import = |home: &Path, file: &Path| {
        import_state(
            &specengine_cli::Env {
                cwd: pair.main.clone(),
                home: Some(home.as_os_str().to_os_string()),
                xdg_data_home: None,
            },
            &Globals::default(),
            &ImportStateRequest {
                file: file.to_path_buf(),
            },
            &mut |_: &str| true,
        )
    };
    let bytes = export(&pair.home, "q.jsonl");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with(
            "{\"format\":2,\"queue_schema\":6,\"project\":\"lantern-keep\",\"proposals\":2,"
        ),
        "{}",
        lines[0]
    );
    let row: Value = serde_json::from_str(lines[1]).expect("a row");
    let row = row["proposals"].as_object().expect("a proposals row");
    assert_eq!(row.len(), 44);
    assert_eq!(row["kind"], json!("interpretation"));
    assert_eq!(
        row["interpretation"],
        json!(serde_json::to_string(&example()).unwrap())
    );
    let second: Value = serde_json::from_str(lines[2]).expect("a row");
    assert_eq!(second["proposals"]["interpretation"], Value::Null);

    let fresh = pair.scratch.home("fresh");
    let outcome = import(&fresh, &dumps.join("q.jsonl")).expect("import");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(export(&fresh, "again.jsonl"), bytes, "byte-identical");
    let restored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .expect("open");
    use specengine_store::ProposalQueue as _;
    assert_eq!(
        restored.get(&id).unwrap().unwrap().interpretation,
        Some(example())
    );

    // A schema-5 dump cannot carry the column.
    let mut v5: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    v5[0] = v5[0].replacen("\"queue_schema\":6,", "\"queue_schema\":5,", 1);
    let file = dumps.join("v5-with-44.jsonl");
    std::fs::write(&file, format!("{}\n", v5.join("\n"))).unwrap();
    let home = pair.scratch.home("v5");
    match import(&home, &file) {
        Err(error) => assert!(
            error.message.contains(":2: ") || error.message.contains("line 2"),
            "{}",
            error.message
        ),
        Ok(outcome) => panic!("a schema-5 row with 44 columns imported: {outcome:?}"),
    }
}

/// The review request of the binary and the library agree for an update
/// raised through `ProposeRequest` (kept to name the import).
#[allow(dead_code)]
fn _uses(_: ProposeRequest) {}

/// Edge data: `DEC-0023`'s front-matter broken (an unclosed list) after
/// an interpretation of it was proposed: a new `--decision DEC-0023` is
/// refused (exit 1, naming the decision or its file, nothing stored), the
/// stored one's approve fails at step 4 the same way, nothing written; no
/// panic, never a "green".
#[test]
fn a_decision_with_broken_front_matter_is_refused_not_a_panic() {
    let pair = Pair::new("ki-broken", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    replace(
        &pair.linked,
        DEC23,
        "supersedes: [DEC-0007]\n",
        "supersedes: [DEC-0007\n",
    );
    pair.commit_all(&pair.linked, "Broken front-matter.");
    let (hash, text) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    let before = pair.state();
    let run = pair.spec_piped(
        &pair.linked,
        &cli_args("RULE-STAM-REGEN", &hash, "DEC-0023", HOW, WHY),
        edit(&text, RATE.0, RATE.1).as_bytes(),
    );
    assert_eq!(run.code, 1, "{}", run.show());
    assert!(
        run.stderr.contains("--decision") && (run.stderr.contains("DEC-0023")),
        "{}",
        run.show()
    );
    assert!(!run.stderr.contains("panicked"), "{}", run.show());
    assert_eq!(pair.state(), before, "nothing stored");

    let outcome = pair.approve(&pair.main, &id);
    let reason = refused(&outcome, "step 4 on a broken decision");
    assert!(
        reason.contains("(step 4)") && reason.contains("--decision"),
        "{reason}"
    );
    assert_eq!(pair.state(), before, "nothing written");
}

// ------------------------------------------ iteration 2: W7 in review

/// Whether `notes` hold a W7 note or a note that cannot tell one.
fn has_w7(notes: &[String]) -> bool {
    notes.iter().any(|note| {
        note.contains("`canon:` section")
            || note.starts_with("cannot tell whether")
            || note.contains("in an accepted decision's `canon:`")
    })
}

/// W7 in `spec review` is judged in the recorded root (the proposal's own
/// worktree) as the preview's step 4 refreshed it: a decision whose
/// `canon:` names `EDGE-STAM-ZERO` exists only on `t1`; an update of it
/// proposed from `t1` is noted there and, reviewed from the main
/// worktree (where no decision names it), still noted. M: judge against
/// the current checkout.
#[test]
fn w7_in_review_is_judged_in_the_recorded_root() {
    let pair = Pair::new("ki-w7-recorded", "spec-a");
    write(
        &pair.linked,
        "docs/records/DEC/DEC-0050.md",
        "---\nid: DEC-0050\nclass: decision\nstatus: accepted\ndate: 2026-10-02\n\
         canon: EDGE-STAM-ZERO\n---\n\n# Zero stamina exhausts at once\n\nA body.\n",
    );
    pair.commit_all(&pair.linked, "DEC-0050 on t1 only.");
    assert!(!pair.main.join("docs/records/DEC/DEC-0050.md").exists());
    let note = w7("EDGE-STAM-ZERO", "DEC-0050", "DEC-0050");
    let outcome = update_ok(&pair, &pair.linked, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    assert_eq!(notes_of(&outcome), [format!("note: {note}")]);
    let id = outcome.document.id.clone().expect("stored");
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.preview.map(|preview| preview.as_str()),
        Some("applies")
    );
    assert!(
        review.document.notes.contains(&note),
        "{:?}",
        review.document.notes
    );
    // From the main worktree, an update there is not noted: its own
    // corpus holds no DEC-0050.
    let here = update_ok(&pair, &pair.main, "EDGE-STAM-ZERO", ZERO.0, ZERO.1);
    assert_eq!(notes_of(&here), Vec::<String>::new());
}

/// The preview stops at step 3 (the target's file dirty in the recorded
/// worktree): no step-4 corpus, the note still given by the review's own
/// refresh of the recorded root. M: no note without the step-4 corpus.
#[test]
fn w7_in_review_survives_a_preview_stopped_at_step_3() {
    let pair = Pair::new("ki-w7-step3", "spec-a");
    let outcome = update_ok(&pair, &pair.linked, "RULE-STAM-REGEN", RATE.0, RATE.1);
    let id = outcome.document.id.clone().expect("stored");
    // Dirty: another line of the same file, uncommitted.
    replace(&pair.linked, STAMINA, ZERO.0, ZERO.1);
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.preview.map(|preview| preview.as_str()),
        Some("unavailable")
    );
    assert!(
        review
            .document
            .notes
            .iter()
            .any(|note| note.starts_with("not applicable now (step 3)")),
        "{:?}",
        review.document.notes
    );
    assert!(
        review
            .document
            .notes
            .contains(&w7("RULE-STAM-REGEN", "DEC-0023", "DEC-0023")),
        "{:?}",
        review.document.notes
    );
}

/// The recorded worktree removed: the review gives no W7 note (nor one
/// that cannot tell); the preview's own notes name the place. M: a note
/// (or a "cannot tell") from a gone root.
#[test]
fn w7_in_review_is_absent_when_the_recorded_worktree_is_gone() {
    let pair = Pair::new("ki-w7-gone", "spec-a");
    let outcome = update_ok(&pair, &pair.linked, "RULE-STAM-REGEN", RATE.0, RATE.1);
    let id = outcome.document.id.clone().expect("stored");
    let linked = pair.linked.to_str().expect("UTF-8").to_owned();
    pair.git
        .git(&pair.main, &["worktree", "remove", "--force", &linked]);
    assert!(!pair.linked.exists());
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.preview.map(|preview| preview.as_str()),
        Some("unavailable")
    );
    assert!(
        !has_w7(&review.document.notes),
        "{:?}",
        review.document.notes
    );
    assert!(!review.document.notes.is_empty(), "the place is named");
}

/// W2: a section-form create (new ID sections inside `RULE-STAM-REGEN`,
/// which lies in `DEC-0023`'s `canon:` section) is never flagged, at
/// propose or in review; nor is an interpretation. M: the review's test
/// `applies()` and not an interpretation.
#[test]
fn w7_never_flags_a_section_form_create() {
    let pair = Pair::new("ki-w7-create", "spec-a");
    let main = pair.main.clone();
    let (hash, span) = pair.span(&main, "RULE-STAM-REGEN");
    let outcome = specengine_cli::propose_create(
        &pair.env(&main),
        &Globals::default(),
        &specengine_cli::CreateRequest {
            target: "RULE-STAM-REGEN".to_owned(),
            base: Some(hash),
            text: ProposedText::Given(
                format!("{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Waits 1.5 s.")
                    .into_bytes(),
            ),
            rationale: "A rest case.".to_owned(),
            author_role: Some("spec-author".to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&main),
        },
    )
    .expect("propose create");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(!has_w7(&notes_of(&outcome)), "{:?}", outcome.messages);
    let id = outcome.document.id.clone().expect("stored");
    assert_eq!(pair.proposal(&id).kind, ProposalKind::Create);
    let review = pair.review_ok(&main, &id);
    assert!(
        !has_w7(&review.document.notes),
        "{:?}",
        review.document.notes
    );
    let interpreted = interpret_ok(&pair, &main);
    assert!(!has_w7(&pair.review_ok(&main, &interpreted).document.notes));
}

// ------------------------------------- iteration 2: step 10 and W14

/// Step 10 with a linked update (a discrepancy's proposed patch): the
/// note is the linked one alone, naming no interpretation: the target is
/// not computed. M: the linked update ignored at step 10.
#[test]
fn step_10_with_a_linked_update_names_it_not_an_interpretation() {
    let pair = Pair::new("ki-step10-linked", "spec-a");
    let main = pair.main.clone();
    let (hash, text) = pair.span(&main, "RULE-STAM-REGEN");
    let mut input = common::decision::discrepancy(
        &["RULE-STAM-REGEN"],
        "Regeneration starts while walking.",
        &common::decision::THREE,
    );
    input.proposed_patch = Some(specengine_cli::ProposedPatch {
        target: "RULE-STAM-REGEN".to_owned(),
        base: hash,
        text: edit(&text, RATE.0, RATE.1),
        rationale: "Rest only.".to_owned(),
    });
    let report = pair.raise_discrepancy(&main, input);
    let linked = pair.proposal(&report).linked.expect("its linked update");
    let (outcome, _) = pair.decide_ok(&main, &report, &common::decision::option(0), None);
    let notes = notes_of(&outcome);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with(&format!(
            "note: `{linked}` (linked) is open: `spec approve {linked}` changes "
        )),
        "{notes:?}"
    );
    assert!(
        !notes
            .iter()
            .any(|note| note.contains("propose interpretation")),
        "{notes:?}"
    );
}

/// The step-10 note's target (`canon_target`) for each `canon:` form, a
/// question approved with `--canon` each: a plain ID; a reference with an
/// ID section (that section's ID); `ID#nonexistent` (an ID-shaped section
/// that is none, and a non-ID anchor: the ID part); a path
/// with an anchor on a heading without an ID inside `RULE-STAM-REGEN`
/// (the nearest enclosing ID node); a path to an id-less document (its
/// path). M: each rule's branch changed.
#[test]
fn step_10_canon_target_follows_each_canon_form() {
    let pair = Pair::new("ki-step10-forms", "spec-a");
    let main = pair.main.clone();
    replace(
        &main,
        STAMINA,
        "rate \u{d7} 0.5.\n",
        "rate \u{d7} 0.5.\n\n### Rest speed\n- Under 40 % of walking.\n",
    );
    write(
        &main,
        "docs/spec/movement/notes.md",
        "---\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # Movement notes\n\nPlain notes.\n",
    );
    pair.commit_all(&main, "A heading without an ID; an id-less document.");
    let cases = [
        ("RULE-STAM-REGEN", "RULE-STAM-REGEN"),
        ("MEC-STAMINA#RULE-STAM-REGEN", "RULE-STAM-REGEN"),
        ("MEC-STAMINA#RULE-STAM-NOPE", "MEC-STAMINA"),
        ("MEC-STAMINA#nope", "MEC-STAMINA"),
        (
            "docs/spec/movement/stamina.md#rest-speed",
            "RULE-STAM-REGEN",
        ),
        (
            "docs/spec/movement/notes.md#movement-notes",
            "docs/spec/movement/notes.md",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (index, (canon, target)) in cases.iter().enumerate() {
        let question = pair.raise_question(
            &main,
            &["RULE-STAM-REGEN"],
            &format!("Form {index}: where does the rule {index} live?"),
            &format!("Under the canon form {index}"),
            "a rebalance",
        );
        let (outcome, _) = pair.decide_ok(&main, &question, &flags(None, None, Some(canon)), None);
        let record = pair.proposal(&question).record.expect("a record").id;
        want.push(format!(
            "note: {record} names {canon} in `canon:`, its text unchanged: `spec propose \
             interpretation {target} --decision {record}` carries it there"
        ));
        got.push(notes_of(&outcome).join("\n"));
    }
    assert_eq!(got, want);
}

/// W14: the consent line names the stored decision, the one the trailer
/// carries, at the path step 4 found its document: `DEC-0023` turned into
/// a section of a document renamed `DEC-0123` (moved too) after the
/// proposal: `interprets DEC-0023 (docs/records/DEC/DEC-0123.md)`, the
/// commit's `Interprets: DEC-0023`. M: the consent line takes step 4's
/// document ID.
#[test]
fn the_consent_line_names_the_stored_decision_at_step_4s_path() {
    let pair = Pair::new("ki-w14", "spec-a");
    let id = interpret_ok(&pair, &pair.linked);
    let moved = "docs/records/DEC/DEC-0123.md";
    pair.git.git(&pair.linked, &["mv", DEC23, moved]);
    replace(&pair.linked, moved, "id: DEC-0023\n", "id: DEC-0123\n");
    let text = read_text(&pair.linked, moved);
    write(
        &pair.linked,
        moved,
        format!("{text}\n## Rest {{#DEC-0023}}\n\nRest is any speed under 40 % of walking.\n"),
    );
    pair.commit_all(&pair.linked, "DEC-0023 is a section of DEC-0123.");
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    let first = questions[0].lines().next().expect("a line");
    assert_eq!(first, format!("interprets DEC-0023 ({moved})"));
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", "t1"]);
    let trailer = message
        .lines()
        .find_map(|line| line.strip_prefix("Interprets: "))
        .expect("the trailer");
    assert_eq!(trailer, "DEC-0023");
    assert!(first.starts_with(&format!("interprets {trailer} (")));
}
