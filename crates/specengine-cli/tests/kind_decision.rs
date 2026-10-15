//! docs/features/kind-decision.md, AC-T1 (the CLI halves of AC-R2 to
//! AC-R9 and AC-R12) over a git copy of `fixtures/spec-a` (`DEC`, the next
//! record `DEC-0024`; `RULE-STAM-REGEN` in `DEC-0023`'s `canon:` section),
//! spec-b (`ADR`, the next `ADR-0003`; `CMD-SYNC`, Russian prose) and the
//! non-Rust spec-c (no `[decision_records]`: W25).
//!
//! The queue's commands run through the library with the sandbox's git
//! environment and an injected clock; the binary where its exit code,
//! stdout or stderr is the subject (`spec propose decision --input F`, a
//! look-alike subcommand, a flag form, `spec propose create`, the
//! terminal's consent question). "Nothing stored": the queue's dump as
//! before (or no database), both worktrees' `git status` empty.
//!
//! - AC-R2: `PR-0001`, `linked: PR-0002`, `introduced: 0`; `--json` the
//!   intake document; exit 2 for an `options` key, no `working_answer`, a
//!   `decision` key, a look-alike field name, `--distinct-from` as a flag,
//!   a look-alike subcommand; exit 1 for a blank `working_answer` (and
//!   `text`, `price_of_other`) and a patch target outside `node_ids`.
//! - AC-R3: a decision equal to an open question on a shared target is a
//!   hit, not stored; named in `distinct_from`, stored; a question after a
//!   decision hits it; a discrepancy meets neither.
//! - AC-R4, AC-R5: `spec approve PR-0001`, `y`: one commit, exactly `A
//!   docs/records/DEC/DEC-0024.md`, `choice` `{"working_answer":true}`,
//!   the consent `(the working answer)`, `PR-0002` named by step 10;
//!   `--answer T`: `{"answer":"T"}`; `--option 0`: exit 2 naming `a
//!   decision`, no event; `spec approve PR-0002`: a second commit, exactly
//!   `M docs/spec/movement/stamina.md`; and in the other order, neither
//!   waiting.
//! - AC-R6: the CLI's stage (the daemon's call): no flag or an `answer`
//!   stored, an `option` exit 2, nothing stored; a staged answer confirmed
//!   by `spec approve`; a stage or a choice holding an option on the row:
//!   corrupt, named by review and inbox.
//! - AC-R7: `round new` lists it as a question (`## PR-0001 - decision,
//!   normal`, `Working answer:`, `Another answer costs:`, JSON `"kind":
//!   "decision"`, `"options": []`); `ok` and own text staged, `1` refused;
//!   the walk's `y` writes the record.
//! - AC-R8: with `--task T`: both rows bound, `open_proposals` lists them,
//!   `assumptions` the decision's working answer (a discrepancy's: its
//!   recommended option).
//! - AC-R9: `propose create` of `DEC-0024` (spec-b: `ADR-0003`): exit 1,
//!   W22's text.
//! - AC-R12: a decision, its update and its record: `export state`,
//!   `import-state` into a fresh `HOME`, `export state` byte-identical.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::path::{Path, PathBuf};

use common::decision::{THREE, discrepancy, flags, option};
use common::proposal::{DECIDER, LATER, NOW, Pair, cannot, edit, json_of, printed, printed_inbox};
use common::read_text;
use serde_json::{Value, json};
use specengine_cli::{
    DecisionInput, DecisionRequest, Exit, ExportStateRequest, Globals, ImportStateRequest,
    IntakeOutcome, IntakeSeverity, Message, Outcome, ProposalOutcome, ProposedPatch,
    RoundAnswerRequest, RoundNewRequest, RoundRun, RoundTerminal, StageBody, StageOutcome,
    StageRequest, export_state, import_state, propose_decision, propose_decision_with_task,
    render_json, render_text, round_answer, round_new, stage,
};
use specengine_store::{
    Choice, KindFamily, ProposalKind, ProposalQueue as _, ProposalStatus, Stage,
};
use task_common::Tasks;

const STAMINA: &str = "docs/spec/movement/stamina.md";
const DEC24: &str = "docs/records/DEC/DEC-0024.md";
/// "Data"'s example.
const TEXT: &str = "Does regeneration wait for rest?";
const WORKING: &str = "Yes: it starts 1.5 s after the last stamina use.";
const PRICE: &str = "Without the wait R-28 is rebalanced.";
/// The rule's new text: rest named in `RULE-STAM-REGEN`'s first line.
const REST: (&str, &str) = ("Base rate 10 units/s,", "Base rate 10 units/s at rest,");
/// W19's refusal of `--option` on a decision.
fn option_refusal(id: &str) -> String {
    format!(
        "spec: `--option` names a discrepancy's option, and `{id}` is a decision: its working \
         answer, or `--answer T`, answers it; nothing changed"
    )
}

// ---------------------------------------------------------------- setup

/// A decision's input: `targets`, `text`, "Data"'s working answer and
/// price, `patch`, `distinct_from`.
fn input(
    targets: &[&str],
    text: &str,
    patch: Option<ProposedPatch>,
    distinct_from: &[&str],
) -> DecisionInput {
    DecisionInput {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        text: text.to_owned(),
        working_answer: WORKING.to_owned(),
        price_of_other: PRICE.to_owned(),
        severity: None,
        proposed_patch: patch,
        distinct_from: (!distinct_from.is_empty())
            .then(|| distinct_from.iter().map(|id| (*id).to_owned()).collect()),
    }
}

/// `target`'s span read in `cwd`, `from` replaced by `to`, as a proposed
/// patch.
fn patch_of(pair: &Pair, cwd: &Path, target: &str, from: &str, to: &str) -> ProposedPatch {
    let (base, text) = pair.span(cwd, target);
    ProposedPatch {
        target: target.to_owned(),
        base,
        text: edit(&text, from, to),
        rationale: "Rest only.".to_owned(),
    }
}

/// spec-a's patch: rest named in `RULE-STAM-REGEN`.
fn rest_patch(pair: &Pair, cwd: &Path) -> ProposedPatch {
    patch_of(pair, cwd, "RULE-STAM-REGEN", REST.0, REST.1)
}

fn request(pair: &Pair, cwd: &Path, input: DecisionInput) -> DecisionRequest {
    DecisionRequest {
        input,
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        now: NOW.to_owned(),
        git: pair.git_env(cwd),
    }
}

/// `spec propose decision` through the library from `cwd`.
fn propose(pair: &Pair, cwd: &Path, input: DecisionInput, task: Option<&str>) -> IntakeOutcome {
    propose_decision_with_task(
        &pair.env(cwd),
        &Globals::default(),
        &request(pair, cwd, input),
        task,
    )
    .unwrap_or_else(|error| panic!("propose decision: {error}"))
}

/// A decision that must be stored: asked again naming every hit when its
/// first ask hits. Its ID.
fn raise(pair: &Pair, cwd: &Path, given: DecisionInput, task: Option<&str>) -> String {
    let first = propose(pair, cwd, given.clone(), task);
    let outcome = if first.document.created {
        first
    } else {
        let mut named = given;
        named.distinct_from = Some(
            first
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect(),
        );
        propose(pair, cwd, named, task)
    };
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// The queue's hits of `outcome` (corpus hits aside).
fn queue_hits(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .filter_map(|hit| hit.id.clone())
        .filter(|id| id.starts_with("PR-"))
        .collect()
}

fn printed_intake(outcome: &IntakeOutcome) -> (String, Value) {
    let outcome = Outcome::Intake(Box::new(outcome.clone()));
    (render_text(&outcome), json_of(&render_json(&outcome)))
}

/// `value` as JSON in `<scratch>/inputs/<name>` (outside both worktrees):
/// its path.
fn input_file(pair: &Pair, name: &str, value: &Value) -> PathBuf {
    let path = pair.scratch.dir("inputs").join(name);
    std::fs::write(&path, value.to_string()).expect("write the input");
    path
}

/// The queue as stored (no database: empty).
fn queue_dump(pair: &Pair) -> String {
    if pair.db().exists() {
        pair.queue().dump().expect("dump")
    } else {
        String::new()
    }
}

/// `git diff-tree --name-status` of `commit`: its lines.
fn name_status(pair: &Pair, commit: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["diff-tree", "-r", "--no-commit-id", "--name-status", commit],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

fn notes_of(outcome: &ProposalOutcome) -> Vec<String> {
    outcome
        .messages
        .iter()
        .map(Message::line)
        .filter(|line| line.starts_with("note: "))
        .collect()
}

/// Both worktrees' `git status --porcelain`, both empty after any intake.
fn assert_clean(pair: &Pair, context: &str) {
    assert_eq!(pair.porcelain(&pair.main), "", "{context}: main");
    assert_eq!(pair.porcelain(&pair.linked), "", "{context}: t1");
}

/// spec-a's record of a decision as the fixture's template renders a
/// question's: no effect, option or evidence.
fn spec_a_record(id: &str, title: &str, choice: &str, cost: &str, note: &str, pr: &str) -> String {
    format!(
        "---\nid: {id}\nclass: decision\nstatus: accepted\ndate: 2026-10-05\ntitle: \"{title}\"\n\
         canon: RULE-STAM-REGEN\nlinks:\n  answers: [RULE-STAM-REGEN]\nref: {pr}\n\
         scope: [decisions]\n---\n\n# {title}\n\nChoice:\n\n{choice}\n\nEffect:\n\n\n\n\
         Cost:\n\n{cost}\n\nSummary:\n\n{TEXT}\n\nOptions:\n\n\n\nEvidence:\n\n\n\n\
         Note:\n\n{note}\n\nDecided by:\n\n{DECIDER}\n"
    )
}

/// The library's stage of `id` from `cwd` at `now`, against its stored
/// `updated_at`.
fn stage_of(
    pair: &Pair,
    cwd: &Path,
    id: &str,
    option: Option<u64>,
    answer: Option<&str>,
    now: &str,
) -> StageOutcome {
    stage(
        &pair.env(cwd),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body: StageBody::Approve {
                option,
                answer: answer.map(str::to_owned),
                canon: None,
                editorial: None,
                note: None,
                updated_at: pair.proposal(id).updated_at,
            },
            now: now.to_owned(),
            git: pair.git_env(cwd),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"))
}

// ------------------------------------------------------------------ AC-R2

/// AC-R2: the binary stores the decision and its patch's linked update
/// (`PR-0001`, `linked: PR-0002`, `introduced: 0`), `--json` is the intake
/// document; the row is a decision with a question's fields, the update
/// the patch's; nothing under either root is written. The same input
/// again is a hit on its own family, not stored.
#[test]
fn ac_r2_the_binary_stores_a_decision_and_its_linked_update() {
    let pair = Pair::new("kd-r2", "spec-a");
    let cwd = pair.linked.clone();
    let head = pair.rev(&cwd, "HEAD");
    let patch = rest_patch(&pair, &cwd);
    let document = json!({
        "node_ids": ["RULE-STAM-REGEN"], "text": TEXT, "working_answer": WORKING,
        "price_of_other": PRICE, "severity": null,
        "proposed_patch": {"target": "RULE-STAM-REGEN", "base": patch.base,
            "text": patch.text, "rationale": "Rest only."},
        "distinct_from": ["DEC-0023"]
    });
    let file = input_file(&pair, "decision.json", &document);
    let file = file.to_str().expect("UTF-8");
    let run = pair.spec_piped(
        &cwd,
        &[
            "--json",
            "propose",
            "decision",
            "--input",
            file,
            "--author-role",
            "developer",
            "--author-model",
            "claude-opus-5-5",
        ],
        b"",
    );
    assert_eq!(run.code, 0, "{run:?}");
    let answer = json_of(&run.stdout);
    assert_eq!(
        answer
            .as_object()
            .expect("an object")
            .keys()
            .collect::<Vec<_>>(),
        [
            "created",
            "diagnostics",
            "hits",
            "id",
            "linked",
            "notes",
            "related"
        ]
    );
    assert_eq!(answer["id"], json!("PR-0001"), "{answer}");
    assert_eq!(answer["created"], json!(true));
    assert_eq!(answer["linked"], json!("PR-0002"));
    assert_eq!(answer["diagnostics"], json!([]));
    assert_clean(&pair, "propose");
    assert_eq!(pair.rev(&cwd, "HEAD"), head);

    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.kind, ProposalKind::Decision);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.target_id, "RULE-STAM-REGEN");
    assert_eq!(stored.target_path, STAMINA);
    assert_eq!(stored.linked.as_deref(), Some("PR-0002"));
    let intake = stored.intake.clone().expect("its fields");
    assert_eq!(intake.target_ids, ["RULE-STAM-REGEN"]);
    assert_eq!(intake.summary, TEXT);
    assert_eq!(intake.working_answer.as_deref(), Some(WORKING));
    assert_eq!(intake.price_of_other.as_deref(), Some(PRICE));
    assert_eq!(intake.severity, IntakeSeverity::Normal);
    assert_eq!(intake.gap_type, None);
    assert!(intake.evidence.is_empty() && intake.options.is_empty());
    assert_eq!(intake.recommendation, None);
    assert_eq!(intake.distinct_from, ["DEC-0023"]);
    let update = pair.proposal("PR-0002");
    assert_eq!(update.kind, ProposalKind::Update);
    assert_eq!(update.linked.as_deref(), Some("PR-0001"));
    assert_eq!(update.target_id, "RULE-STAM-REGEN");
    assert_eq!(update.new_text, patch.text);
    assert_eq!(update.rationale, "Rest only.");
    assert_eq!(update.author, stored.author);
    assert_eq!(stored.author.role.as_deref(), Some("developer"));

    // The text form of the same input: a hit on PR-0001 (its own family),
    // nothing stored.
    let dump = queue_dump(&pair);
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
        ],
        document.to_string().as_bytes(),
    );
    assert_eq!(run.code, 0, "{run:?}");
    assert!(run.stdout.contains("hit: PR-0001 | open"), "{}", run.stdout);
    assert!(!run.stdout.starts_with("PR-"), "{}", run.stdout);
    assert_eq!(queue_dump(&pair), dump, "nothing stored");

    // Another decision's text form: its ID, the linked update, the count.
    let mut other = document.clone();
    other["text"] = json!("How long is the wait after a sprint?");
    other["distinct_from"] = json!(["DEC-0023", "PR-0001"]);
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
        ],
        other.to_string().as_bytes(),
    );
    assert_eq!(run.code, 0, "{run:?}");
    assert!(
        run.stdout
            .starts_with("PR-0003\nlinked: PR-0004\nintroduced: 0\n"),
        "{}",
        run.stdout
    );
    assert_eq!(pair.proposal("PR-0003").kind, ProposalKind::Decision);
    assert_eq!(pair.proposal("PR-0004").kind, ProposalKind::Update);
    assert_clean(&pair, "the second decision");
}

/// AC-R2, the refusals: exit 2 for an `options` key, no `working_answer`,
/// a `decision` key, a look-alike field name, `--distinct-from` as a flag
/// and a look-alike subcommand; exit 1 naming the field for a blank
/// `working_answer`, `text`, `price_of_other`, and a patch target outside
/// `node_ids`. Nothing stored by any, both worktrees clean. M: no
/// `deny_unknown_fields`.
#[test]
fn ac_r2_refusals_store_nothing() {
    let pair = Pair::new("kd-r2-refused", "spec-a");
    let cwd = pair.linked.clone();
    let patch = rest_patch(&pair, &cwd);
    let good = json!({
        "node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero stamina a stop?",
        "working_answer": "Yes.", "price_of_other": "A new rule case."
    });
    let with = |key: &str, value: Value| {
        let mut document = good.clone();
        document[key] = value;
        document
    };
    let without = |key: &str| {
        let mut document = good.clone();
        document.as_object_mut().unwrap().remove(key);
        document
    };
    // "working_answer" with a Cyrillic small ie (U+0435) for its "e".
    let look_alike = "working_answ\u{0435}r";
    let mut lookalike_field = without("working_answer");
    lookalike_field[look_alike] = json!("Yes.");
    let exit_2: Vec<(&str, Value, &str)> = vec![
        (
            "an options key",
            with(
                "options",
                json!([{"label": "a", "effect": "b", "price": "c"}]),
            ),
            "unknown field `options`",
        ),
        (
            "no working_answer",
            without("working_answer"),
            "missing field `working_answer`",
        ),
        (
            "a decision key",
            with("decision", json!("Yes.")),
            "unknown field `decision`",
        ),
        ("a look-alike field", lookalike_field, "unknown field"),
        (
            "a gap type",
            with("gap_type", json!("contradicts")),
            "unknown field `gap_type`",
        ),
        (
            "a severity of no kind",
            with("severity", json!("urgent")),
            "--input -",
        ),
    ];
    for (label, document, named) in exit_2 {
        let run = pair.spec_piped(
            &cwd,
            &[
                "propose",
                "decision",
                "--input",
                "-",
                "--author-role",
                "developer",
            ],
            document.to_string().as_bytes(),
        );
        assert_eq!(run.code, 2, "{label}: {run:?}");
        assert!(
            run.stderr
                .starts_with("spec: --input -: not a decision's arguments"),
            "{label}: {}",
            run.stderr
        );
        assert!(run.stderr.contains(named), "{label}: {}", run.stderr);
        assert_eq!(run.stdout, "", "{label}");
        assert_eq!(queue_dump(&pair), "", "{label}: nothing stored");
    }
    // A flag form: no flag names a field.
    for args in [
        vec![
            "propose",
            "decision",
            "--input",
            "-",
            "--distinct-from",
            "DEC-0023",
        ],
        vec!["propose", "decision", "--text", "T", "--input", "-"],
        // A look-alike subcommand: "decisi\u{043e}n" (a Cyrillic small o).
        vec!["propose", "decisi\u{043e}n", "--input", "-"],
    ] {
        let run = pair.spec_piped(&cwd, &args, good.to_string().as_bytes());
        assert_eq!(run.code, 2, "{args:?}: {run:?}");
        assert_eq!(run.stdout, "", "{args:?}");
        assert_eq!(queue_dump(&pair), "", "{args:?}: nothing stored");
    }
    // Exit 1, naming the field.
    let mut outside = good.clone();
    outside["proposed_patch"] = json!({"target": "RULE-STAM-REGEN", "base": patch.base,
        "text": patch.text, "rationale": "Outside."});
    let exit_1: Vec<(&str, Value, &str)> = vec![
        (
            "a blank working answer",
            with("working_answer", json!(" \n\t")),
            "working_answer: blank: give a value",
        ),
        (
            "a blank text",
            with("text", json!("  ")),
            "text: blank: give a value",
        ),
        (
            "a blank price",
            with("price_of_other", json!("")),
            "price_of_other: blank: give a value",
        ),
        (
            "a working answer over 2 048 bytes",
            with("working_answer", json!("a".repeat(2049))),
            "working_answer: 2049 bytes; at most 2048",
        ),
        (
            "a patch target outside node_ids",
            outside,
            "proposed_patch: its target `RULE-STAM-REGEN` is not among node_ids",
        ),
    ];
    for (label, document, named) in exit_1 {
        let run = pair.spec_piped(
            &cwd,
            &[
                "propose",
                "decision",
                "--input",
                "-",
                "--author-role",
                "developer",
            ],
            document.to_string().as_bytes(),
        );
        assert_eq!(run.code, 1, "{label}: {run:?}");
        assert!(
            run.stderr.contains(named) || run.stdout.contains(named),
            "{label}: {run:?}"
        );
        let stored = pair.proposals();
        assert!(stored.is_empty(), "{label}: nothing stored: {stored:?}");
    }
    // The library refuses the same, naming the field first.
    let mut blank = input(&["EDGE-STAM-ZERO"], "Zero?", None, &[]);
    blank.working_answer = "  ".to_owned();
    let outcome = propose(&pair, &cwd, blank, None);
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    assert_eq!(
        outcome.refusal.as_deref(),
        Some("working_answer: blank: give a value")
    );
    assert!(pair.proposals().is_empty());
    assert_clean(&pair, "the refusals");
}

// ------------------------------------------------------------------ AC-R3

/// AC-R3, W18, through the library: a decision equal to an open question
/// on a shared target is a hit, not stored (no ID taken); named in
/// `distinct_from`, stored; a question after a decision hits it; a
/// discrepancy of the same text meets neither, nor they it. M:
/// `queue_matches` by kind.
#[test]
fn ac_r3_a_question_and_a_decision_hit_each_other_a_discrepancy_neither() {
    let pair = Pair::new("kd-r3", "spec-a");
    let cwd = pair.linked.clone();
    let text = "Is zero stamina a full stop?";
    let asked = pair.raise_question(&cwd, &["EDGE-STAM-ZERO"], text, "Yes.", "A rule case.");
    assert_eq!(asked, "PR-0001");
    let dump = queue_dump(&pair);

    let hit = propose(
        &pair,
        &cwd,
        input(
            &["MEC-STAMINA", "EDGE-STAM-ZERO"],
            "  is ZERO stamina a full   stop? ",
            None,
            &[],
        ),
        None,
    );
    assert_eq!(hit.exit(), Exit::Answered, "{hit:?}");
    assert!(!hit.document.created, "{hit:?}");
    assert_eq!(hit.document.id, None);
    assert_eq!(queue_hits(&hit), ["PR-0001"]);
    assert_eq!(queue_dump(&pair), dump, "nothing stored");
    let (text_out, json_out) = printed_intake(&hit);
    assert!(text_out.contains("hit: PR-0001 | open"), "{text_out}");
    assert_eq!(json_out["created"], json!(false));

    let mut names: Vec<String> = hit
        .document
        .hits
        .iter()
        .map(|found| found.name().to_owned())
        .collect();
    names.sort();
    let named: Vec<&str> = names.iter().map(String::as_str).collect();
    let stored = propose(
        &pair,
        &cwd,
        input(
            &["MEC-STAMINA", "EDGE-STAM-ZERO"],
            "  is ZERO stamina a full   stop? ",
            None,
            &named,
        ),
        None,
    );
    assert!(stored.document.created, "{stored:?}");
    assert_eq!(stored.document.id.as_deref(), Some("PR-0002"));
    assert_eq!(pair.proposal("PR-0002").kind, ProposalKind::Decision);

    // A question after the decision: both hit.
    let again = specengine_cli::propose_question(
        &pair.env(&cwd),
        &Globals::default(),
        &specengine_cli::QuestionRequest {
            node_ids: vec!["MEC-STAMINA".to_owned()],
            text: text.to_owned(),
            working_answer: "Yes.".to_owned(),
            price_of_other: "A rule case.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            author_role: Some("developer".to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&cwd),
        },
    )
    .expect("ask");
    assert!(!again.document.created, "{again:?}");
    assert_eq!(queue_hits(&again), ["PR-0002"]);

    // A discrepancy of the same text on the same targets: no queue hit.
    let report = pair.raise_discrepancy(
        &cwd,
        discrepancy(&["MEC-STAMINA", "EDGE-STAM-ZERO"], text, &THREE),
    );
    assert_eq!(report, "PR-0003");
    // And a decision after it meets only its own family.
    let after = propose(
        &pair,
        &cwd,
        input(&["EDGE-STAM-ZERO"], text, None, &[]),
        None,
    );
    assert_eq!(queue_hits(&after), ["PR-0001", "PR-0002"]);
    assert!(!after.document.created);
    assert_clean(&pair, "the asks");
}

// ------------------------------------------------------------------ AC-R4

/// AC-R4, AC-R5: `spec approve PR-0001` with no flag: one commit, one
/// parent, exactly `A docs/records/DEC/DEC-0024.md`, the record as the
/// template renders a question's (the working answer its choice and
/// title, `canon:` the first ID target), `choice` `{"working_answer":
/// true}`, the consent `(the working answer)`, step 10 naming `PR-0002`;
/// the update still open. Then `spec approve PR-0002`: a second commit,
/// exactly `M docs/spec/movement/stamina.md`. M: `decide.rs:802` left; the
/// patch in the record's commit.
#[test]
fn ac_r4_r5_the_record_then_the_patch_two_commits() {
    let pair = Pair::new("kd-r4", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &cwd)),
            &[],
        ),
        None,
    );
    assert_eq!(id, "PR-0001");
    let linked = pair.proposal(&id).linked.expect("its linked update");
    assert_eq!(linked, "PR-0002");
    let base = pair.rev(&cwd, "HEAD");
    let base_commit = pair.proposal(&id).place.base_commit;

    let (outcome, question) = pair.decide_ok(&pair.main, &id, &flags(None, None, None), None);
    let head = pair.rev(&cwd, "t1");
    assert_eq!(pair.rev(&cwd, &format!("{head}^")), base, "one parent");
    assert_eq!(
        pair.git_text(&pair.main, &["rev-list", "--count", &format!("{base}..t1")]),
        "1"
    );
    assert_eq!(name_status(&pair, &head), [format!("A\t{DEC24}")]);
    let record = read_text(&cwd, DEC24);
    let want = spec_a_record("DEC-0024", WORKING, WORKING, "", "", &id);
    assert_eq!(record, want);
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]);
    assert_eq!(
        message,
        format!(
            "spec: apply PR-0001\n\nDEC-0024: {WORKING}\n\nProposal: PR-0001\n\
             Decided-by: {DECIDER}\nProposed-by: agent role=developer model=claude-opus-5-5 \
             run=unknown\nBase-commit: {base_commit}"
        )
    );
    assert!(
        question.ends_with(&format!(
            "apply {id} as DEC-0024 (the working answer) on t1 in {}? [y/N]",
            cwd.display()
        )),
        "{question}"
    );
    assert!(
        question.starts_with(&format!("introduced: 0\nrecord DEC-0024 at {DEC24}:\n")),
        "{question}"
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    let kept = stored.record.expect("its record");
    assert_eq!(kept.id, "DEC-0024");
    assert_eq!(kept.path, DEC24);
    assert_eq!(kept.choice, Choice::WorkingAnswer);
    assert_eq!(
        json_of(&printed(&outcome).1)["choice"],
        json!({"working_answer": true})
    );
    assert_eq!(
        notes_of(&outcome),
        [format!(
            "note: `{linked}` (linked) is open: `spec approve {linked}` changes RULE-STAM-REGEN"
        )]
    );
    assert_eq!(pair.proposal(&linked).status, ProposalStatus::Open);
    assert_eq!(read_text(&cwd, STAMINA).matches(REST.1).count(), 0);
    let (inbox, _) = printed_inbox(&pair.inbox(&pair.main, true).expect("inbox"));
    assert!(
        inbox.starts_with(&format!(
            "PR-0001 | decision | applied | RULE-STAM-REGEN | t1 | {NOW} | normal: {TEXT} \
             [DEC-0024]\n"
        )),
        "{inbox}"
    );

    // AC-R5: the linked update, a second commit of its own.
    let applied = pair.approve_ok(&pair.main, &linked);
    let second = pair.rev(&cwd, "t1");
    assert_eq!(
        pair.rev(&cwd, &format!("{second}^")),
        head,
        "after the record"
    );
    assert_eq!(name_status(&pair, &second), [format!("M\t{STAMINA}")]);
    assert_eq!(read_text(&cwd, STAMINA).matches(REST.1).count(), 1);
    assert_eq!(pair.proposal(&linked).status, ProposalStatus::Applied);
    assert_eq!(applied.exit(), Exit::Answered);
    assert_eq!(read_text(&cwd, DEC24), want, "the record kept");
    assert_clean(&pair, "both applied");
}

/// AC-R5, ADR-0012: the other order: the update first (`M`), the record
/// after (`A`), neither waiting for the other; with no update left open,
/// step 10 names none. `--answer T` records `{"answer":"T"}`, the consent
/// `(the given answer)`, the price its cost, `--note` its note.
#[test]
fn ac_r4_r5_the_patch_first_then_the_given_answer() {
    let pair = Pair::new("kd-r5-order", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &cwd)),
            &[],
        ),
        None,
    );
    let linked = pair.proposal(&id).linked.expect("its linked update");
    let base = pair.rev(&cwd, "HEAD");
    pair.approve_ok(&pair.main, &linked);
    let first = pair.rev(&cwd, "t1");
    assert_eq!(pair.rev(&cwd, &format!("{first}^")), base);
    assert_eq!(name_status(&pair, &first), [format!("M\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);

    let answer = "No: at once.";
    let (outcome, question) = pair.decide_ok(
        &pair.main,
        &id,
        &flags(None, Some(answer), None),
        Some("Owner note."),
    );
    let second = pair.rev(&cwd, "t1");
    assert_eq!(pair.rev(&cwd, &format!("{second}^")), first);
    assert_eq!(name_status(&pair, &second), [format!("A\t{DEC24}")]);
    assert_eq!(
        read_text(&cwd, DEC24),
        spec_a_record("DEC-0024", answer, answer, PRICE, "Owner note.", &id)
    );
    assert!(question.contains("(the given answer) on t1"), "{question}");
    let stored = pair.proposal(&id);
    assert_eq!(
        stored.record.expect("its record").choice,
        Choice::Answer(answer.to_owned())
    );
    assert_eq!(stored.decision_note.as_deref(), Some("Owner note."));
    assert_eq!(
        json_of(&printed(&outcome).1)["choice"],
        json!({"answer": answer})
    );
    assert!(
        !notes_of(&outcome)
            .iter()
            .any(|note| note.contains(&format!("`{linked}` (linked)"))),
        "{:?}",
        notes_of(&outcome)
    );
    assert_clean(&pair, "both applied");
}

/// AC-R4, W19: `--option` on a decision (alone, or with `--answer`):
/// exit 2, the exact text naming `a decision`, never a question, no event,
/// nothing written; a blank `--answer` exit 2. M: `--option` accepted.
#[test]
fn ac_r4_option_on_a_decision_is_exit_2_nothing_changed() {
    let pair = Pair::new("kd-r4-option", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &cwd)),
            &[],
        ),
        None,
    );
    let before = pair.state();
    let events = pair.events().len();
    for given in [option(0), flags(Some(1), Some("yes"), None)] {
        let (outcome, questions) = pair.decide(&pair.main, &id, &given, None, true);
        assert_eq!(cannot(&outcome, "--option"), option_refusal(&id));
        assert!(questions.is_empty(), "{questions:?}");
        assert_eq!(pair.state(), before, "nothing written");
        assert_eq!(pair.events().len(), events, "no event");
    }
    let (outcome, _) = pair.decide(&pair.main, &id, &flags(None, Some("  "), None), None, true);
    assert!(
        cannot(&outcome, "blank answer").contains("`--answer` is blank"),
        "{outcome:?}"
    );
    assert_eq!(pair.state(), before);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
}

/// AC-R4 on a terminal: `spec approve PR-0001`, `y`: the record shown, the
/// question `(the working answer)`, the linked update named, exit 0; `spec
/// approve PR-0001 --option 0` exits 2 with W19's text.
#[test]
fn ac_r4_the_binary_asks_on_the_terminal() {
    let pair = Pair::new("kd-r4-tty", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &cwd)),
            &[],
        ),
        None,
    );
    let refused = pair.spec_pty(&pair.main, &["approve", &id, "--option", "0"], None);
    assert_eq!(refused.code, 2, "{}", refused.output);
    assert!(!refused.asked, "{}", refused.output);
    assert!(
        refused.output.contains(&option_refusal(&id)),
        "{}",
        refused.output
    );
    let run = pair.spec_pty(&pair.main, &["approve", &id], Some("y"));
    assert_eq!(run.code, 0, "{}", run.output);
    assert!(run.asked);
    assert!(
        run.output.contains(&format!(
            "apply {id} as DEC-0024 (the working answer) on t1 in {}? [y/N]",
            cwd.display()
        )),
        "{}",
        run.output
    );
    assert!(
        run.output.contains(
            "note: `PR-0002` (linked) is open: `spec approve PR-0002` changes RULE-STAM-REGEN"
        ),
        "{}",
        run.output
    );
    let head = pair.rev(&cwd, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{DEC24}")]);
}

/// AC-R4 for spec-b: a decision on `CMD-SYNC` (Russian text) with its
/// patch: `ADR-0003` at `docs/records/ADR/ADR-0003.md`, one commit `A`,
/// `canon: CMD-SYNC` (W20: not the patch's, not `ADR-0001`'s slug), the
/// texts verbatim; then the patch's own commit `M docs/spec/cli.md`.
#[test]
fn ac_r4_spec_b_records_adr_0003_and_the_patch_apart() {
    let pair = Pair::new("kd-r4-b", "spec-b");
    let cwd = pair.linked.clone();
    // Russian: "Does sync wait for the lock?" / "Yes, up to 5 s." / "A new
    // case."
    let text = "\u{0416}\u{0434}\u{0451}\u{0442} \u{043b}\u{0438} sync \u{0431}\u{043b}\u{043e}\
                \u{043a}\u{0438}\u{0440}\u{043e}\u{0432}\u{043a}\u{0443}?";
    let working = "\u{0414}\u{0430}, \u{0434}\u{043e} 5 \u{0441}.";
    let price = "\u{041d}\u{043e}\u{0432}\u{044b}\u{0439} \u{0441}\u{043b}\u{0443}\u{0447}\u{0430}\
                 \u{0439}.";
    let mut given = input(
        &["CMD-SYNC"],
        text,
        Some(patch_of(&pair, &cwd, "CMD-SYNC", "`sync`", "`sync --all`")),
        &[],
    );
    given.working_answer = working.to_owned();
    given.price_of_other = price.to_owned();
    let id = raise(&pair, &cwd, given, None);
    let linked = pair.proposal(&id).linked.expect("its linked update");
    let base = pair.rev(&cwd, "HEAD");
    let (_, question) = pair.decide_ok(&pair.main, &id, &flags(None, None, None), None);
    let head = pair.rev(&cwd, "t1");
    assert_eq!(pair.rev(&cwd, &format!("{head}^")), base);
    let path = "docs/records/ADR/ADR-0003.md";
    assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
    let record = read_text(&cwd, path);
    assert!(record.starts_with("---\nid: ADR-0003\n"), "{record}");
    assert!(record.contains("\ncanon: CMD-SYNC\n"), "{record}");
    assert!(
        record.contains(working) && record.contains(text),
        "{record}"
    );
    assert!(
        question.contains(&format!(
            "apply {id} as ADR-0003 (the working answer) on t1"
        )),
        "{question}"
    );
    pair.approve_ok(&pair.main, &linked);
    let second = pair.rev(&cwd, "t1");
    assert_eq!(
        name_status(&pair, &second),
        ["M\tdocs/spec/cli.md".to_owned()]
    );
    assert_clean(&pair, "spec-b");
}

/// W25 on spec-c (no `[decision_records]`): the decision is stored (its
/// patch linked); approve exits 2 at step 3 naming the table, nothing
/// written; `reject --reason` settles it unrecorded; the linked update is
/// decided on its own.
#[test]
fn spec_c_without_records_approve_exits_2_and_reject_settles() {
    let pair = Pair::new("kd-c", "spec-c");
    let cwd = pair.linked.clone();
    let (base, span) = pair.span(&cwd, "FEE-DAY-CAP");
    let mut lines = span.lines();
    let heading = lines.next().expect("a heading").to_owned();
    let text = format!("{heading}\nThe day cap is charged once per calendar day.\n");
    let id = raise(
        &pair,
        &cwd,
        input(
            &["FEE-DAY-CAP"],
            "Is the cap per calendar day?",
            Some(ProposedPatch {
                target: "FEE-DAY-CAP".to_owned(),
                base,
                text,
                rationale: "Name the day.".to_owned(),
            }),
            &[],
        ),
        None,
    );
    let linked = pair.proposal(&id).linked.expect("its linked update");
    assert_eq!(pair.proposal(&id).kind, ProposalKind::Decision);
    let before = pair.state();
    let (outcome, questions) = pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
    assert_eq!(
        cannot(&outcome, "no table"),
        format!(
            "spec: `{id}` not applied (step 3): {}/specengine.toml has no `[decision_records]` \
             (`prefix`, `dir`, `template`) for `{id}`'s record: add it, or `spec reject {id} \
             --reason <answer>`; nothing changed",
            cwd.display()
        )
    );
    assert!(questions.is_empty());
    assert_eq!(pair.state(), before, "nothing written");
    let (outcome, _) = pair.reject_answer(&pair.main, &id, "Per 24 hours.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Rejected);
    assert!(stored.record.is_none());
    assert_eq!(pair.proposal(&linked).status, ProposalStatus::Open);
    assert_clean(&pair, "spec-c");
}

// ------------------------------------------------------------------ AC-R6

/// AC-R6, the CLI's stage (the daemon's call): `option` exit 2 with W19's
/// text, nothing stored, no event; no flag stored; an `answer` replaces
/// it; `spec approve PR` without flags confirms the staged answer. M:
/// `stage.rs`'s arm `None`.
#[test]
fn ac_r6_a_staged_decision_takes_an_answer_never_an_option() {
    let pair = Pair::new("kd-r6-stage", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(&["RULE-STAM-REGEN"], TEXT, None, &[]),
        None,
    );
    let dump = queue_dump(&pair);
    let refused = stage_of(
        &pair,
        &pair.main,
        &id,
        Some(0),
        None,
        "2026-10-05T21:30:00Z",
    );
    assert_eq!(refused.exit(), Exit::CannotRun, "{refused:?}");
    assert_eq!(
        refused.proposal.refusal.as_deref(),
        option_refusal(&id).strip_prefix("spec: ")
    );
    assert_eq!(queue_dump(&pair), dump, "nothing stored, no event");

    let working = stage_of(&pair, &pair.main, &id, None, None, "2026-10-05T21:30:00Z");
    assert_eq!(working.exit(), Exit::Answered, "{working:?}");
    assert_eq!(
        pair.proposal(&id).staged.map(|staged| staged.stage),
        Some(Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            editorial: false,
            note: None,
            span_hash: None
        })
    );
    let answered = stage_of(
        &pair,
        &pair.main,
        &id,
        None,
        Some("No."),
        "2026-10-05T21:40:00Z",
    );
    assert_eq!(answered.exit(), Exit::Answered, "{answered:?}");
    assert_eq!(
        pair.sql(&format!("SELECT staged FROM proposals WHERE id = '{id}';"))
            .trim(),
        "{\"decision\":\"approve\",\"option\":null,\"answer\":\"No.\",\"canon\":null,\
         \"editorial\":null,\"note\":null,\"span_hash\":null}"
    );
    let review = json_of(&printed(&pair.review_ok(&pair.main, &id)).1);
    assert_eq!(review["kind"], json!("decision"));
    assert_eq!(review["staged"]["answer"], json!("No."));

    let (outcome, question) =
        pair.decide_at(&pair.main, &id, &flags(None, None, None), None, true, LATER);
    let outcome = outcome.expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(question[0].contains("staged"), "{question:?}");
    assert_eq!(
        pair.proposal(&id).record.expect("its record").choice,
        Choice::Answer("No.".to_owned())
    );
    assert_clean(&pair, "staged and applied");
}

/// AC-R6, "Data" Row: a decision row whose stage holds an `option`, or
/// whose `choice` is an option: corrupt, named by `review` (exit 2 naming the row and the column) and
/// by the inbox (a note; the other rows listed).
#[test]
fn ac_r6_a_corrupt_decision_row_is_named_by_review_and_inbox() {
    let pair = Pair::new("kd-r6-corrupt", "spec-a");
    let cwd = pair.linked.clone();
    let open = raise(
        &pair,
        &cwd,
        input(&["EDGE-STAM-ZERO"], "Is zero a stop?", None, &[]),
        None,
    );
    let decided = raise(
        &pair,
        &cwd,
        input(&["RULE-STAM-REGEN"], TEXT, None, &[]),
        None,
    );
    pair.decide_ok(&pair.main, &decided, &flags(None, None, None), None);
    let staged_option = "{\"decision\":\"approve\",\"option\":0,\"answer\":null,\"canon\":null,\
                         \"editorial\":null,\"note\":null,\"span_hash\":null}";
    for (label, sql, broken, column) in [
        (
            "an option staged",
            format!(
                "UPDATE proposals SET staged = '{staged_option}', staged_at = \
                 '2026-10-05T21:30:00Z' WHERE id = '{open}';"
            ),
            open.as_str(),
            "staged",
        ),
        (
            "an option chosen",
            format!("UPDATE proposals SET choice = '{{\"option\":0}}' WHERE id = '{decided}';"),
            decided.as_str(),
            "choice",
        ),
    ] {
        let good = pair.sql(&format!(
            "SELECT quote(staged), quote(staged_at), quote(choice), quote(options) FROM \
             proposals WHERE id = '{broken}';"
        ));
        pair.sql(&sql);
        let message = match pair.review(&pair.main, broken) {
            Err(error) => error.message,
            Ok(outcome) => panic!("{label}: read as {outcome:?}"),
        };
        assert!(
            message.contains(broken) && message.contains(&format!("`{column}`")),
            "{label}: {message}"
        );
        let inbox = pair.inbox(&pair.main, true).expect("inbox lists the rest");
        assert!(
            inbox
                .notes
                .iter()
                .any(|note| note.contains(broken) && note.contains(&format!("`{column}`"))),
            "{label}: {:?}",
            inbox.notes
        );
        assert_eq!(inbox.proposals.len(), 1, "{label}");
        let values: Vec<&str> = good.trim().split('|').collect();
        pair.sql(&format!(
            "UPDATE proposals SET staged = {}, staged_at = {}, choice = {}, options = {} WHERE \
             id = '{broken}';",
            values[0], values[1], values[2], values[3]
        ));
        assert_eq!(
            pair.inbox(&pair.main, true).unwrap().proposals.len(),
            2,
            "{label}: restored"
        );
    }
}

// ------------------------------------------------------------------ AC-R7

/// The owner's terminal of a library round: the keys answer in order.
struct Keys {
    keys: Vec<Option<bool>>,
    asked: Vec<String>,
    reports: Vec<String>,
}

impl RoundTerminal for Keys {
    fn report(&mut self, text: &str) {
        self.reports.push(text.to_owned());
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        let index = self.asked.len();
        self.asked.push(question.to_owned());
        self.keys.get(index).copied().flatten()
    }

    fn tell(&mut self, _line: &str) {}
}

fn round_with(pair: &Pair, text: &str, name: &str, keys: &[Option<bool>]) -> (RoundRun, Keys) {
    let path = pair.scratch.dir("answers").join(name);
    std::fs::write(&path, text).expect("write the answers");
    let mut terminal = Keys {
        keys: keys.to_vec(),
        asked: Vec::new(),
        reports: Vec::new(),
    };
    let run = round_answer(
        &pair.env(&pair.main),
        &Globals::default(),
        &RoundAnswerRequest {
            file: Some(path),
            now: "2026-10-06T10:00:00Z".to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut terminal,
    )
    .unwrap_or_else(|error| panic!("round answer: {error}"));
    (run, terminal)
}

/// AC-R7: the sheet lists a decision as a question (no option list), its
/// linked update noted, not listed; JSON `"kind": "decision"`, `"options":
/// []`; `1` refused (`has no options`), `ok` staged as the working answer,
/// own text as the answer; the walk's `y` writes the record in one commit
/// `A`. M: selection by `Question | Discrepancy`.
#[test]
fn ac_r7_the_round_asks_a_decision_as_a_question() {
    let pair = Pair::new("kd-r7", "spec-a");
    let main = pair.main.clone();
    let first = raise(
        &pair,
        &main,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &main)),
            &[],
        ),
        None,
    );
    let second = raise(
        &pair,
        &main,
        input(&["EDGE-STAM-ZERO"], "Is zero a stop?", None, &[]),
        None,
    );
    let third = raise(
        &pair,
        &main,
        input(&["MEC-SPRINT"], "Can a sprint be cancelled?", None, &[]),
        None,
    );
    assert_eq!(
        [first.as_str(), second.as_str(), third.as_str()],
        ["PR-0001", "PR-0003", "PR-0004"]
    );
    let sheet = round_new(
        &pair.env(&main),
        &Globals::default(),
        &RoundNewRequest {
            limit: None,
            git: pair.git_env(&main),
        },
    )
    .expect("round new");
    let outcome = Outcome::RoundSheet(Box::new(sheet));
    let text = render_text(&outcome);
    let json = json_of(&render_json(&outcome));
    // Deterministic: a second sheet of the same state, byte for byte.
    let again = round_new(
        &pair.env(&main),
        &Globals::default(),
        &RoundNewRequest {
            limit: None,
            git: pair.git_env(&main),
        },
    )
    .expect("round new again");
    let again = Outcome::RoundSheet(Box::new(again));
    assert_eq!(render_text(&again), text, "the same sheet");
    assert_eq!(render_json(&again), render_json(&outcome), "the same JSON");
    let block = format!(
        "## PR-0001 - decision, normal\n\n  {TEXT}\n\nWorking answer:\n  {WORKING}\n\n\
         Another answer costs:\n  {PRICE}\n\nConcerns: Regeneration (RULE-STAM-REGEN)\n\n\
         PR-0001:\n"
    );
    assert!(text.contains(&block), "{text}");
    assert!(
        !text.contains("## PR-0002"),
        "the update is not an item: {text}"
    );
    let items = json["items"].as_array().expect("items");
    let listed: Vec<&str> = items
        .iter()
        .map(|item| item["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(listed.len(), 3, "{listed:?}");
    for item in items {
        assert_eq!(item["kind"], json!("decision"), "{item}");
        assert_eq!(item["options"], json!([]), "{item}");
        assert_eq!(item["gap_type"], Value::Null, "{item}");
        assert_eq!(item["recommendation"], Value::Null, "{item}");
    }
    let item = items
        .iter()
        .find(|item| item["id"] == json!("PR-0001"))
        .expect("PR-0001");
    assert_eq!(item["working_answer"], json!(WORKING));
    assert_eq!(item["price_of_other"], json!(PRICE));

    let file =
        format!("project: lantern-keep\n{first}: ok\n{second}: No: zero only slows.\n{third}: 1\n");
    let (run, terminal) = round_with(&pair, &file, "a.md", &[Some(true), Some(false)]);
    assert_eq!(
        terminal.reports,
        [format!(
            "{first} line 2: staged: approve, the working answer\n{second} line 3: staged: \
             approve, your answer\n{third} line 4: refused: `{third}` has \
             no options: `ok` takes its working answer, other text is your answer; nothing \
             staged\nanswers: 2 staged, 0 left open, 1 refused\n"
        )],
        "{terminal:?}",
        terminal = terminal.reports
    );
    assert_eq!(terminal.asked.len(), 2, "{:?}", terminal.asked);
    assert!(
        terminal.asked[0].contains(&format!(
            "apply {first} as DEC-0024 (the working answer, staged)"
        )),
        "{}",
        terminal.asked[0]
    );
    // The walk's `y`: the record, one commit `A`.
    let applied = pair.proposal(&first);
    assert_eq!(applied.status, ProposalStatus::Applied);
    let commit = applied.applied_commit.expect("a commit");
    assert_eq!(name_status(&pair, &commit), [format!("A\t{DEC24}")]);
    assert_eq!(
        applied.record.expect("its record").choice,
        Choice::WorkingAnswer
    );
    // The `n`: kept staged with the owner's text.
    let kept = pair.proposal(&second);
    assert_eq!(kept.status, ProposalStatus::Open);
    assert_eq!(
        kept.staged.map(|staged| staged.stage),
        Some(Stage::Approve {
            option: None,
            answer: Some("No: zero only slows.".to_owned()),
            canon: None,
            editorial: false,
            note: None,
            span_hash: None
        })
    );
    assert!(pair.proposal(&third).staged.is_none(), "`1` staged nothing");
    assert_eq!(pair.proposal("PR-0002").status, ProposalStatus::Open);
    assert_eq!(run.exit(), Exit::NotFound, "one kept staged");
}

// ------------------------------------------------------------------ AC-R8

/// AC-R8, W21: `--task T-0001` binds the decision and its linked update;
/// the package's `open_proposals` lists both, `assumptions` the
/// decision's working answer, a discrepancy's its recommended option. The
/// binary's `--task` binds too. M: `package.rs:471` left.
#[test]
fn ac_r8_the_task_package_assumes_the_working_answer() {
    let pair = Pair::new("kd-r8", "spec-a");
    let main = pair.main.clone();
    let task = pair.new_ok(&main, &["MEC-STAMINA"]);
    assert_eq!(task, "T-0001");
    let id = raise(
        &pair,
        &main,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &main)),
            &[],
        ),
        Some(&task),
    );
    let linked = pair.proposal(&id).linked.expect("its linked update");
    for proposal in [&id, &linked] {
        assert_eq!(
            pair.proposal(proposal).task_id.as_deref(),
            Some("T-0001"),
            "{proposal}"
        );
    }
    let gap = task_common::report_gap(
        &pair,
        &main,
        discrepancy(&["MEC-STAMINA"], "Walking drains stamina.", &THREE),
        Some(&task),
    );
    let gap = gap.document.id.clone().expect("stored");
    let package = pair.package(&main, &task);
    let listed: Vec<(&str, &str, &str)> = package["open_proposals"]
        .as_array()
        .expect("open proposals")
        .iter()
        .map(|item| {
            (
                item["id"].as_str().unwrap(),
                item["kind"].as_str().unwrap(),
                item["task_id"].as_str().unwrap_or("null"),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (id.as_str(), "decision", "T-0001"),
            (linked.as_str(), "update", "T-0001"),
            (gap.as_str(), "discrepancy", "T-0001"),
        ]
    );
    assert_eq!(package["open_proposals"][0]["summary"], json!(TEXT));
    assert_eq!(
        package["assumptions"],
        json!([
            {"proposal": id, "text": WORKING},
            {"proposal": gap, "text": "Keep the spec"}
        ])
    );

    // The binary's `--task`.
    let document = json!({
        "node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero a stop?", "working_answer": "Yes.",
        "price_of_other": "A rule case."
    });
    let run = pair.spec_piped(
        &main,
        &[
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
            "--task",
            "T-0001",
        ],
        document.to_string().as_bytes(),
    );
    assert_eq!(run.code, 0, "{run:?}");
    let bound = run.stdout.lines().next().expect("an ID").to_owned();
    assert_eq!(pair.proposal(&bound).task_id.as_deref(), Some("T-0001"));
    let run = pair.spec_piped(
        &main,
        &[
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
            "--task",
            "T-0099",
        ],
        json!({"node_ids": ["MEC-SPRINT"], "text": "Other?", "working_answer": "Yes.",
            "price_of_other": "None."})
        .to_string()
        .as_bytes(),
    );
    assert_eq!(run.code, 1, "{run:?}");
    assert!(
        run.stderr
            .contains("--task: no task T-0099 in this repository"),
        "{run:?}"
    );
    assert_clean(&pair, "the task");
}

// ------------------------------------------------------------------ AC-R9

/// AC-R9, W22: `spec propose create` of a record of the
/// `[decision_records]` prefix: exit 1, W22's text (spec-a `DEC-0024`,
/// spec-b `ADR-0003`), nothing stored. M: the old text.
#[test]
fn ac_r9_create_refuses_the_record_prefix_naming_the_three_kinds() {
    for (fixture, path, id) in [
        ("spec-a", DEC24, "DEC-0024"),
        ("spec-b", "docs/records/ADR/ADR-0003.md", "ADR-0003"),
    ] {
        let pair = Pair::new(&format!("kd-r9-{fixture}"), fixture);
        let text = format!(
            "---\nid: {id}\nclass: decision\nstatus: accepted\n---\n\n# A record\n\nA body.\n"
        );
        let run = pair.spec_piped(
            &pair.linked,
            &[
                "propose",
                "create",
                path,
                "--text-file",
                "-",
                "--rationale",
                "A record by hand.",
                "--author-role",
                "writer",
            ],
            text.as_bytes(),
        );
        assert_eq!(run.code, 1, "{fixture}: {run:?}");
        let want = format!(
            "`{id}`: these records are made by `spec approve` of a `question`, `discrepancy` or \
             `decision` (`spec propose decision` raises one); nothing stored"
        );
        assert!(
            run.stderr.contains(&want) || run.stdout.contains(&want),
            "{fixture}: {run:?}"
        );
        assert!(pair.proposals().is_empty(), "{fixture}");
        assert_clean(&pair, fixture);
    }
}

// ----------------------------------------------------------------- AC-R12

/// AC-R12: a decision (applied, its record), its linked update (open) and
/// a staged decision: `export state`, `import-state` into a fresh `HOME`,
/// `export state` byte-identical; the rows read back equal. M: the decode
/// refusing the kind.
#[test]
fn ac_r12_a_backup_with_a_decision_round_trips_byte_identical() {
    let pair = Pair::new("kd-r12", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &cwd)),
            &[],
        ),
        None,
    );
    pair.decide_ok(
        &pair.main,
        &id,
        &flags(None, Some("No: at once."), None),
        None,
    );
    let staged = raise(
        &pair,
        &cwd,
        input(&["EDGE-STAM-ZERO"], "Is zero a stop?", None, &[]),
        None,
    );
    let outcome = stage_of(&pair, &pair.main, &staged, None, Some("Yes."), LATER);
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let dumps = pair.scratch.dir("dumps");
    let env_of = |home: &Path| specengine_cli::Env {
        cwd: pair.main.clone(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let export = |home: &Path, name: &str| -> Vec<u8> {
        let file = dumps.join(name);
        export_state(
            &env_of(home),
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
    let bytes = export(&pair.home, "q.jsonl");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    assert!(
        text.lines()
            .next()
            .is_some_and(|line| line.contains("\"proposals\":3,")),
        "{text}"
    );
    assert!(text.contains("\"kind\":\"decision\""), "{text}");
    let fresh = pair.scratch.home("fresh");
    let imported = import_state(
        &env_of(&fresh),
        &Globals::default(),
        &ImportStateRequest {
            file: dumps.join("q.jsonl"),
        },
        &mut |_: &str| true,
    )
    .expect("import");
    assert_eq!(imported.exit(), Exit::Answered, "{imported:?}");
    assert_eq!(export(&fresh, "again.jsonl"), bytes, "byte-identical");
    let restored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .expect("open");
    for proposal in pair.proposals() {
        assert_eq!(
            restored.get(&proposal.id).unwrap().unwrap(),
            proposal,
            "{}",
            proposal.id
        );
    }
    assert_eq!(
        restored
            .get(&id)
            .unwrap()
            .unwrap()
            .record
            .expect("its record")
            .choice,
        Choice::Answer("No: at once.".to_owned())
    );
}

/// The library's `propose_decision` (no task) is `propose_decision_with_task`
/// with `None`: the same document.
#[test]
fn propose_decision_is_the_unbound_twin() {
    let pair = Pair::new("kd-twin", "spec-a");
    let cwd = pair.linked.clone();
    let outcome = propose_decision(
        &pair.env(&cwd),
        &Globals::default(),
        &request(
            &pair,
            &cwd,
            input(&["EDGE-STAM-ZERO"], "Is zero a stop?", None, &[]),
        ),
    )
    .expect("propose");
    assert!(outcome.document.created, "{outcome:?}");
    assert_eq!(pair.proposal("PR-0001").task_id, None);
    let (text, json) = printed_intake(&outcome);
    assert!(text.starts_with("PR-0001\n"), "{text}");
    assert_eq!(json["linked"], Value::Null);
}

// ------------------------------------------------------------ edge cases

/// "Edge cases": a held decision completes as recorded. A failing
/// `pre-commit` hook stops the record's commit (exit 1, the record held
/// by the reopen); committed by hand from `record_text`, `--answer`
/// (the given answer, not the held working answer) exits 2 naming the held record and its choice, nothing
/// written; no flag completes it by that commit after the owner's
/// consent, no new commit.
#[test]
fn edge_a_held_decision_completes_as_recorded() {
    use std::os::unix::fs::PermissionsExt as _;
    let pair = Pair::new("kd-held", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["RULE-STAM-REGEN"],
            TEXT,
            Some(rest_patch(&pair, &cwd)),
            &[],
        ),
        None,
    );
    let hooks = pair.main.join(".git/hooks");
    std::fs::create_dir_all(&hooks).expect("hooks");
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").expect("hook");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let (outcome, _) = pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
    let outcome = outcome.expect("approve runs");
    assert_eq!(outcome.exit(), Exit::NotFound, "the hook: {outcome:?}");
    std::fs::remove_file(&hook).expect("hook removed");
    let held = pair.proposal(&id);
    assert_eq!(held.status, ProposalStatus::Open);
    let record = held.record.expect("held by the reopen");
    assert_eq!(record.id, "DEC-0024");
    assert_eq!(record.choice, Choice::WorkingAnswer);
    common::write(&cwd, &record.path, &record.text);
    pair.git.git(&cwd, &["add", "--", &record.path]);
    let message = format!(
        "spec: apply {id}\n\nDEC-0024: {}\n\nProposal: {id}\nDecided-by: {DECIDER}\n\
         Proposed-by: agent role=developer model=claude-opus-5-5 run=unknown\nBase-commit: {}\n",
        record.title, held.place.base_commit
    );
    pair.git.git_stdin(
        &cwd,
        &["commit", "-q", "--no-verify", "-F", "-"],
        message.as_bytes(),
    );
    let tip = pair.rev(&cwd, "HEAD");

    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) =
        pair.decide(&pair.main, &id, &flags(None, Some("No"), None), None, true);
    assert_eq!(
        cannot(&outcome, "another answer"),
        format!(
            "spec: `{id}` holds its record `DEC-0024` (the working answer), completed by its \
             commit {tip} on `t1`: `spec approve {id}` completes it as recorded; the flags name \
             the given answer; nothing changed"
        )
    );
    assert!(questions.is_empty());
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");

    let (outcome, questions) = pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
    let outcome = outcome.expect("complete");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "complete {id} by its commit {tip} on t1 in {}? [y/N]",
            cwd.display()
        )]
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(tip.as_str()));
    assert_eq!(pair.rev(&cwd, "t1"), tip, "no new commit");
    assert_eq!(name_status(&pair, &tip), [format!("A\t{DEC24}")]);
    assert_eq!(pair.proposal("PR-0002").status, ProposalStatus::Open);
}

/// "Edge cases": a decision whose targets are paths only: approve with no
/// flag exits 2 asking for `--canon`, nothing written; with `--canon
/// RULE-STAM-REGEN` the record governs it (`canon: RULE-STAM-REGEN`), one
/// commit `A`.
#[test]
fn edge_a_path_only_decision_needs_canon() {
    let pair = Pair::new("kd-path", "spec-a");
    let cwd = pair.linked.clone();
    let id = raise(
        &pair,
        &cwd,
        input(
            &["docs/features/stamina-tuning.md"],
            "Is the tuning final?",
            None,
            &[],
        ),
        None,
    );
    let before = pair.state();
    let (outcome, questions) = pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
    assert_eq!(
        cannot(&outcome, "no ID"),
        format!(
            "spec: `{id}` names no ID: name the section its record governs with `--canon REF`; \
             nothing changed"
        )
    );
    assert!(questions.is_empty());
    assert_eq!(pair.state(), before);
    pair.decide_ok(
        &pair.main,
        &id,
        &flags(None, None, Some("RULE-STAM-REGEN")),
        None,
    );
    let head = pair.rev(&cwd, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{DEC24}")]);
    let record = read_text(&cwd, DEC24);
    assert!(record.contains("\ncanon: RULE-STAM-REGEN\n"), "{record}");
}

// ------------------------------------------------------------ iteration 2

/// The deciding kinds as `family()` names them (`KindFamily::Decides`,
/// docs/features/drift-queue.md "Kind family"), as a refusal lists them:
/// `` a `question`, `discrepancy` or `decision` ``.
fn deciding_kinds() -> String {
    let names: Vec<String> = ProposalKind::ALL
        .into_iter()
        .filter(|kind| kind.family() == KindFamily::Decides)
        .map(|kind| format!("`{}`", kind.as_str()))
        .collect();
    let (last, rest) = names.split_last().expect("a deciding kind");
    // The first kind's article: "a `question`, ...".
    format!("a {} or {last}", rest.join(", "))
}

/// Iteration 2: a decision flag on an update (`--option`, `--answer`,
/// `--canon`, through `approve` and the daemon's `stage`) exits 2 naming
/// the kinds that decide, from `decides()`: `` `question`, `discrepancy`
/// or `decision` ``; nothing written, no event, no prompt. M: the old
/// two-kind text.
#[test]
fn iteration2_a_decision_flag_on_an_update_names_the_deciding_kinds() {
    assert_eq!(
        deciding_kinds(),
        "a `question`, `discrepancy` or `decision`"
    );
    let pair = Pair::new("kd-flag-kinds", "spec-a");
    let update = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
    );
    let before = pair.state();
    let events = pair.events().len();
    let want = |flag: &str| {
        format!(
            "spec: `{flag}` decides {}; `{update}` is an update: `spec approve {update}` applies \
             it as proposed; nothing changed",
            deciding_kinds()
        )
    };
    for (given, flag) in [
        (option(0), "--option"),
        (flags(None, Some("Yes."), None), "--answer"),
        (flags(None, None, Some("RULE-STAM-REGEN")), "--canon"),
    ] {
        let (outcome, questions) = pair.decide(&pair.main, &update, &given, None, true);
        assert_eq!(cannot(&outcome, flag), want(flag));
        assert!(questions.is_empty(), "{flag}: {questions:?}");
        assert_eq!(pair.state(), before, "{flag}: nothing written");
        assert_eq!(pair.events().len(), events, "{flag}: no event");
    }
    // The daemon's stage of the same flag: exit 2, the same line.
    let staged = stage_of(&pair, &pair.main, &update, Some(0), None, LATER);
    assert_eq!(staged.exit(), Exit::CannotRun, "{staged:?}");
    assert_eq!(
        staged.proposal.refusal.as_deref(),
        want("--option").strip_prefix("spec: ")
    );
    assert_eq!(pair.events().len(), events, "no stage event");
}

/// Iteration 2, the stricter row reading through the CLI: a question's or
/// a decision's row holding `options` (`[]` counts), `evidence`,
/// `gap_type` or `recommendation`, and a discrepancy's row holding
/// `price_of_other`: `review` exits 2 naming the row and the column (`it
/// is set on <kind>, which holds none`); the inbox skips the row with a
/// note naming it and lists the rest. M: a column dropped from
/// `null_columns()`.
#[test]
fn iteration2_a_row_holding_a_column_its_shape_lacks_is_named_by_review_and_inbox() {
    let pair = Pair::new("kd-shape-rows", "spec-a");
    let cwd = pair.linked.clone();
    let question = pair.raise_question(&cwd, &["EDGE-STAM-ZERO"], "Stop?", "Yes.", "A case.");
    let decided = raise(
        &pair,
        &cwd,
        input(&["RULE-STAM-REGEN"], TEXT, None, &[]),
        None,
    );
    let report = pair.raise_discrepancy(
        &cwd,
        discrepancy(&["MEC-STAMINA"], "Walking drains stamina.", &THREE),
    );
    let option = "[{\"label\":\"a\",\"effect\":\"b\",\"price\":\"c\"}]";
    let cases: Vec<(&str, &str, &str, &str)> = vec![
        (question.as_str(), "a question", "options", "[]"),
        (question.as_str(), "a question", "gap_type", "contradicts"),
        (decided.as_str(), "a decision", "options", "[]"),
        (decided.as_str(), "a decision", "options", option),
        (decided.as_str(), "a decision", "evidence", "[]"),
        (decided.as_str(), "a decision", "gap_type", "contradicts"),
        (decided.as_str(), "a decision", "recommendation", "0"),
        (
            report.as_str(),
            "a discrepancy",
            "price_of_other",
            "A rebalance.",
        ),
    ];
    for (id, kind, column, value) in cases {
        let label = format!("{kind} {column} {value}");
        pair.sql(&format!(
            "UPDATE proposals SET {column} = '{}' WHERE id = '{id}';",
            value.replace('\'', "''")
        ));
        let error = match pair.review(&pair.main, id) {
            Err(error) => error,
            Ok(outcome) => panic!("{label}: read as {outcome:?}"),
        };
        assert_eq!(error.exit, Exit::CannotRun, "{label}: {error}");
        assert!(
            error.message.contains(&format!(
                "proposal {id}: the stored `{column}` cannot be read: it is set on {kind}, which \
                 holds none"
            )),
            "{label}: {}",
            error.message
        );
        let inbox = pair.inbox(&pair.main, true).expect("inbox lists the rest");
        assert!(
            inbox
                .notes
                .iter()
                .any(|note| note.contains(id) && note.contains(&format!("`{column}`"))),
            "{label}: {:?}",
            inbox.notes
        );
        let listed: Vec<&str> = inbox
            .proposals
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(listed.len(), 2, "{label}: {listed:?}");
        assert!(!listed.contains(&id), "{label}: skipped");
        pair.sql(&format!(
            "UPDATE proposals SET {column} = NULL WHERE id = '{id}';"
        ));
        assert_eq!(
            pair.inbox(&pair.main, true).unwrap().proposals.len(),
            3,
            "{label}: restored"
        );
    }
}
