//! docs/features/queue-path-targets.md through the CLI library: a queue
//! target written as the root-relative `.md` path of an indexed document
//! (`propose update`, `propose question`, `propose discrepancy` and its
//! `proposed_patch`), resolved as `spec show` resolves a path, stored under
//! the document's `id:` else its path; approve, rebase and conflict on the
//! whole file; the refusals; dedup on the document node; the backup round
//! trip; escaping. AC-01 to AC-08, AC-10, AC-11 and the developer's
//! deviation 3 (a path row read only at its recorded path), plus the
//! completion of a path row by its own commit.
//!
//! Scratch git repositories of `fixtures/spec-a` (`spec-b` where named) as
//! `common::proposal` makes them (the main worktree on `main`, a linked one
//! on `t1`), a scratch `HOME`, the injected clock, consent through the
//! callback; the binary only where stderr is the subject. "Refused": exit 1,
//! nothing stored (the queue's `dump()` byte-identical, git and the files as
//! they were), so the next ID is unchanged.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::bundle::blake3_hex;
use common::proposal::{
    DECIDER, LATER, NOW, Pair, cannot, edit, json_of, printed, printed_inbox, refused,
};
use common::{read, read_text, replace, write};
use serde_json::{Value, json};
use specengine_cli::{
    CliError, DiscrepancyInput, DiscrepancyRequest, Env, Evidence, Exit, ExportStateRequest,
    GapType, Globals, ImportStateRequest, IntakeOption, IntakeOutcome, IntakeSeverity, Outcome,
    ProposalOutcome, ProposedPatch, QuestionRequest, ShowRequest, export_state, import_state,
    propose_discrepancy, propose_question, render_json, render_text, show,
};
use specengine_store::SqliteQueue;

/// spec-a's feature document without an `id:`.
const TUNING: &str = "docs/features/stamina-tuning.md";
/// spec-a's document declaring `id: MEC-STAMINA`.
const STAMINA: &str = "docs/spec/movement/stamina.md";
/// The current text of every refusal of a path that is not clean.
const UNCLEAN: &str = "is no clean root-relative path: no leading `/`, no `.`, `..` or empty \
                       component";
/// The reason of a path that is no file of the walk.
const NOT_INDEXED: &str =
    "is no indexed document: not under the `[paths]` roots, excluded, or missing";

// ---------------------------------------------------------------- helpers

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

/// AC-01's text: `priority:` and AC-07's prose changed.
fn tuned(text: &str) -> String {
    edit(
        &edit(text, "priority: high", "priority: low"),
        "first regeneration tick 1.5 s later.",
        "first regeneration tick 1.2 s later.",
    )
}

/// A question of a `developer` agent about `node_ids`, from `cwd`.
fn question(pair: &Pair, cwd: &Path, node_ids: &[&str], text: &str) -> QuestionRequest {
    QuestionRequest {
        node_ids: ids(node_ids),
        text: text.to_owned(),
        working_answer: "yes, 1.2 s".to_owned(),
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

fn discrepancy(node_ids: &[&str], summary: &str) -> DiscrepancyInput {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label}: the effect"),
        price: "1 item".to_owned(),
    };
    DiscrepancyInput {
        node_ids: ids(node_ids),
        summary: summary.to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::High,
        evidence: vec![Evidence {
            file: "src/stamina.rs".to_owned(),
            qpath: Some("stamina::regen".to_owned()),
            lines: Some("3-9".to_owned()),
            observed: "regenerates after 1.2 s".to_owned(),
            documented: "after 1.5 s".to_owned(),
        }],
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

/// The queue's rows and events, as `dump()` gives them.
fn queue_state(pair: &Pair) -> String {
    if pair.db().exists() {
        pair.queue().dump().expect("dump")
    } else {
        String::new()
    }
}

/// An intake refused (exit 1) that changed nothing: its reason.
fn intake_refused(
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
            outcome.refusal.clone().expect("a refusal")
        }
        Err(error) => panic!("{context}: exit {:?}: {error}", error.exit),
    };
    assert_eq!(queue_state(pair), before, "{context}: the queue changed");
    reason
}

/// An intake that could not run (exit 2): its message; nothing changed.
fn intake_cannot(
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

/// A `propose update` that stored: its ID.
fn stored(outcome: Result<ProposalOutcome, CliError>, context: &str) -> String {
    let outcome = outcome.unwrap_or_else(|error| panic!("{context}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "{context}: {:?}",
        outcome.refusal
    );
    outcome.document.id.clone().expect("an ID")
}

/// The canonical targets an intake row stores.
fn target_ids(pair: &Pair, id: &str) -> Vec<String> {
    pair.proposal(id)
        .intake
        .unwrap_or_else(|| panic!("{id} is an intake row"))
        .target_ids
}

fn hit_ids(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect()
}

/// `spec show <reference>` through the library: exit 0 with nodes, or the
/// reason of exit 1.
fn shown(pair: &Pair, cwd: &Path, reference: &str) -> Result<usize, String> {
    let outcome = show(
        &pair.env(cwd),
        &Globals::default(),
        &ShowRequest {
            reference: reference.to_owned(),
            links: false,
            archive: false,
        },
    )
    .unwrap_or_else(|error| panic!("show {reference}: {error}"));
    match outcome.reason {
        Some(reason) => Err(reason),
        None => Ok(outcome.nodes.len()),
    }
}

// ------------------------------------------------------------------ AC-01

/// AC-01: `propose update docs/features/stamina-tuning.md` with `spec
/// show`'s `span_hash` (the whole file's) and a text changing `priority:`
/// and AC-07's prose: `PR-0001`; review's `target_id`, `target_path`,
/// `target_ids` and inbox's target column are the path; `ask_question` on
/// it: created, `DEC-0023` (its `adrs:`, a mention) in `related`. M: the `is
/// a path` refusal restored.
#[test]
fn ac01_an_id_less_document_is_proposed_and_asked_by_its_path() {
    let pair = Pair::new("qpt-ac01", "spec-a");
    let cwd = pair.linked.clone();
    let (base, text) = pair.span(&cwd, TUNING);
    let file = read(&cwd, TUNING);
    assert_eq!(
        text.as_bytes(),
        file,
        "the document's span is the whole file"
    );
    assert_eq!(base, format!("b3:{}", blake3_hex(&file)));

    let new_text = tuned(&text);
    let outcome = pair
        .propose(&cwd, TUNING, &base, &new_text)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{:?}", outcome.refusal);
    assert_eq!(printed(&outcome).0, "PR-0001\nintroduced: 0\n");
    let row = pair.proposal("PR-0001");
    assert_eq!(
        (row.target_id.as_str(), row.target_path.as_str()),
        (TUNING, TUNING)
    );
    assert_eq!(row.base_hash, base);
    assert_eq!(row.base_text, text);
    assert_eq!(row.new_text, new_text);

    let (review_text, review_json) = printed(&pair.review_ok(&cwd, "PR-0001"));
    let review_json = json_of(&review_json);
    assert_eq!(review_json["target_id"], json!(TUNING));
    assert_eq!(review_json["target_path"], json!(TUNING));
    assert_eq!(review_json["target_ids"], json!([TUNING]));
    assert_eq!(review_json["preview"], json!("applies"));
    for line in [
        format!("\ntarget_id: {TUNING}\n"),
        format!("\ntarget_path: {TUNING}\n"),
        format!("\ntarget_ids: 1\n  {TUNING}\n"),
    ] {
        assert!(review_text.contains(&line), "{line:?} in {review_text}");
    }

    let (inbox_text, inbox_json) = printed_inbox(&pair.inbox(&cwd, false).expect("inbox"));
    assert_eq!(
        inbox_text,
        format!("PR-0001 | update | open | {TUNING} | t1 | {NOW} | Update {TUNING}.\n")
    );
    assert_eq!(
        json_of(&inbox_json)["proposals"][0]["target_id"],
        json!(TUNING)
    );

    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &[TUNING], "Is 1.2 s the delay to tune to?"),
    );
    let (text, json) = printed_intake(&outcome);
    assert_eq!(
        json,
        json!({"id": "PR-0002", "created": true, "hits": [],
            "related": [{"id": "DEC-0023", "source": "corpus", "status": "accepted",
                "path": "docs/records/DEC/DEC-0023.md", "answer": null, "record": null}],
            "linked": null, "diagnostics": [], "notes": []})
    );
    assert_eq!(
        text,
        "PR-0002\nrelated: DEC-0023 | accepted | docs/records/DEC/DEC-0023.md | -\n"
    );
    assert_eq!(target_ids(&pair, "PR-0002"), [TUNING]);
    let row = pair.proposal("PR-0002");
    assert_eq!(
        (row.target_id.as_str(), row.target_path.as_str()),
        (TUNING, TUNING)
    );
    let review = json_of(&printed(&pair.review_ok(&cwd, "PR-0002")).1);
    assert_eq!(review["target_ids"], json!([TUNING]));
}

// ------------------------------------------------------------------ AC-02

/// AC-02: `docs/spec/movement/stamina.md` names `MEC-STAMINA`: `spec show`
/// by path and by ID give one span; `ask_question` by path answers exactly
/// as by ID (hit `DEC-0023`, nothing stored); with `distinct_from:
/// ["DEC-0023"]` the row's `target_ids` is `["MEC-STAMINA"]`; `propose
/// update` by path stores `target_id` `MEC-STAMINA` with the `patch_hash`
/// of the same update by ID. M: the path stored despite an `id:`.
#[test]
fn ac02_a_document_with_an_id_is_named_by_it() {
    let pair = Pair::new("qpt-ac02", "spec-a");
    let cwd = pair.linked.clone();
    let by_path = pair.span(&cwd, STAMINA);
    assert_eq!(by_path, pair.span(&cwd, "MEC-STAMINA"));
    assert_eq!(by_path.1, read_text(&cwd, STAMINA), "the whole file");

    let before = queue_state(&pair);
    let text = "Does regeneration wait for rest?";
    let path_answer = printed_intake(&asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &[STAMINA], text),
    ));
    let id_answer = printed_intake(&asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &["MEC-STAMINA"], text),
    ));
    assert_eq!(path_answer, id_answer, "a path answers as its ID");
    assert_eq!(
        path_answer.1,
        json!({"id": null, "created": false,
            "hits": [{"id": "DEC-0023", "source": "corpus", "status": "accepted",
                "path": "docs/records/DEC/DEC-0023.md",
                "answer": "Regeneration waits for rest", "record": "DEC-0023"}],
            "related": [], "linked": null, "diagnostics": [], "notes": []})
    );
    assert_eq!(queue_state(&pair), before, "nothing stored");

    let mut request = question(&pair, &cwd, &[STAMINA], text);
    request.distinct_from = ids(&["DEC-0023"]);
    let outcome = asked(&pair, &cwd, &request);
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0001"), ["MEC-STAMINA"]);
    let row = pair.proposal("PR-0001");
    assert_eq!(
        (row.target_id.as_str(), row.target_path.as_str()),
        ("MEC-STAMINA", STAMINA)
    );

    let (base, current) = by_path;
    let new_text = edit(
        &current,
        "delay after sprinting 1.5 s",
        "delay after sprinting 1.2 s",
    );
    let first = stored(pair.propose(&cwd, STAMINA, &base, &new_text), "by path");
    let second = stored(pair.propose(&cwd, "MEC-STAMINA", &base, &new_text), "by ID");
    let (by_path, by_id) = (pair.proposal(&first), pair.proposal(&second));
    assert_eq!(by_path.target_id, "MEC-STAMINA");
    assert_eq!(by_path.target_path, STAMINA);
    for (field, a, b) in [
        ("target_id", &by_path.target_id, &by_id.target_id),
        ("target_path", &by_path.target_path, &by_id.target_path),
        ("base_hash", &by_path.base_hash, &by_id.base_hash),
        ("base_text", &by_path.base_text, &by_id.base_text),
        ("new_text", &by_path.new_text, &by_id.new_text),
        ("patch_hash", &by_path.patch_hash, &by_id.patch_hash),
    ] {
        assert_eq!(a, b, "{field}: by path as by ID");
    }
    let review = json_of(&printed(&pair.review_ok(&cwd, &first)).1);
    assert_eq!(review["target_ids"], json!(["MEC-STAMINA"]));
    let (inbox, _) = printed_inbox(&pair.inbox(&cwd, false).expect("inbox"));
    assert!(
        inbox.contains(&format!("{first} | update | open | MEC-STAMINA | t1 | ")),
        "{inbox}"
    );
}

// ------------------------------------------------------------------ AC-03

/// AC-03: approve of AC-01's proposal with another tracked file modified
/// in the worktree: `applied`; the new `HEAD` has one parent, the old one;
/// `git diff-tree` lists exactly the path; the four trailers; the file is
/// byte-equal to the text; the other file is still modified, its bytes
/// kept. M: a first section as the span (the `priority:` edit lost).
#[test]
fn ac03_approve_by_path_commits_the_whole_file_alone() {
    let pair = Pair::new("qpt-ac03", "spec-a");
    let (base, text) = pair.span(&pair.linked, TUNING);
    let new_text = tuned(&text);
    let id = stored(
        pair.propose(&pair.linked, TUNING, &base, &new_text),
        "AC-01's update",
    );
    let other = "docs/spec/game.md";
    let mut dirty = read(&pair.linked, other);
    dirty.extend_from_slice(b"\nA line not yet committed.\n");
    write(&pair.linked, other, &dirty);
    let old = pair.rev(&pair.linked, "HEAD");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("approve: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "apply {id} to {TUNING} on t1 in {} (applies)? [y/N]",
            pair.linked.display()
        )]
    );
    let head = pair.rev(&pair.linked, "HEAD");
    assert_eq!(
        printed(&outcome).0,
        format!("applied {id} as {head} on t1\n")
    );
    assert_eq!(
        pair.git_text(&pair.linked, &["rev-list", "--parents", "-n", "1", "HEAD"]),
        format!("{head} {old}"),
        "one parent, the old HEAD"
    );
    assert_eq!(
        pair.git_text(
            &pair.linked,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        TUNING
    );
    let provenance = pair.proposal(&id).author.provenance();
    assert_eq!(
        pair.git_text(&pair.linked, &["log", "-1", "--format=%B", "HEAD"]),
        format!(
            "spec: apply {id}\n\nUpdate {TUNING}.\n\n\
             Proposal: {id}\nDecided-by: {DECIDER}\nProposed-by: {provenance}\n\
             Base-commit: {old}"
        )
    );
    assert_eq!(read(&pair.linked, TUNING), new_text.as_bytes());
    assert_eq!(
        pair.git
            .git(&pair.linked, &["show", &format!("HEAD:{TUNING}")]),
        new_text.as_bytes()
    );
    assert_eq!(
        String::from_utf8(pair.git.git(&pair.linked, &["status", "--porcelain=v1"])).unwrap(),
        format!(" M {other}\n"),
        "the other file is still modified, nothing else"
    );
    assert_eq!(read(&pair.linked, other), dirty);
    let row = pair.proposal(&id);
    assert_eq!(row.status.as_str(), "applied");
    assert_eq!(row.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(row.decided_at.as_deref(), Some(LATER));
}

// ------------------------------------------------------------------ AC-04

/// AC-04: after proposing, a commit changing another line of the file:
/// approve `rebases`, both edits kept; a later proposal and a commit
/// changing its line: exit 1 at step 5 with the conflict on stdout, the
/// file, the branch and the queue as before, still `open`. M: step 5 over a
/// section's span.
#[test]
fn ac04_a_path_update_rebases_or_stops_on_the_conflict() {
    let pair = Pair::new("qpt-ac04", "spec-a");
    let (base, text) = pair.span(&pair.linked, TUNING);
    let first = stored(
        pair.propose(&pair.linked, TUNING, &base, &tuned(&text)),
        "first",
    );
    replace(
        &pair.linked,
        TUNING,
        "ref: owner request, stamina tuning\n",
        "ref: owner request, stamina tuning, round 2\n",
    );
    pair.commit_all(&pair.linked, "Round 2.");
    let review = pair.review_ok(&pair.main, &first);
    assert_eq!(review.document.preview.map(|p| p.as_str()), Some("rebases"));
    let (outcome, questions) =
        pair.approve_answer(&pair.main, &first, true, pair.git_env(&pair.main));
    assert_eq!(
        outcome.expect("approve").exit(),
        Exit::Answered,
        "{questions:?}"
    );
    assert!(questions[0].ends_with(" (rebases)? [y/N]"), "{questions:?}");
    let now = read_text(&pair.linked, TUNING);
    for kept in [
        "priority: low\n",
        "first regeneration tick 1.2 s later.",
        "stamina tuning, round 2\n",
    ] {
        assert!(now.contains(kept), "{kept:?} kept: {now}");
    }
    assert_eq!(
        pair.git_text(
            &pair.linked,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        TUNING
    );

    let (base, text) = pair.span(&pair.linked, TUNING);
    let second = stored(
        pair.propose(
            &pair.linked,
            TUNING,
            &base,
            &edit(&text, "priority: low", "priority: critical"),
        ),
        "second",
    );
    replace(
        &pair.linked,
        TUNING,
        "priority: low\n",
        "priority: medium\n",
    );
    pair.commit_all(&pair.linked, "Medium.");
    let before = pair.state();
    let file = read(&pair.linked, TUNING);
    let outcome = pair.approve(&pair.main, &second);
    let reason = refused(&outcome, "the same line");
    assert!(reason.contains("(step 5)"), "{reason}");
    let (stdout, json) = printed(outcome.as_ref().expect("refused"));
    for part in [
        "<<<<<<< current",
        "priority: medium",
        "=======",
        "priority: critical",
        ">>>>>>> proposed",
    ] {
        assert!(stdout.contains(part), "{part:?} in the conflict:\n{stdout}");
    }
    assert_eq!(json_of(&json)["preview"], "conflicts");
    assert_eq!(pair.state(), before, "the conflict changed nothing");
    assert_eq!(read(&pair.linked, TUNING), file);
    assert_eq!(pair.proposal(&second).status.as_str(), "open");
    assert_eq!(
        pair.events_of(&second).last().expect("events"),
        &("proposal.apply_failed".to_owned(), Some(5))
    );
}

// ------------------------------------------------------------------ AC-05

/// AC-05, the CLI half: a path not clean exits 2 (`../`, absolute, `.`
/// and empty components) for `update`, `node_ids` and `proposed_patch`
/// alike; a missing file, `NOTES.md` at the root (outside the roots) and
/// another case are refused naming the field, as `spec show` refuses them;
/// `update` on `R-12.md` (immutable) and spec-b's `GLS-task-branch.md`
/// (generated), and on an id-less `class: generated` document, is refused
/// while a question on them stores `R-12`, `GLS-task-branch` and the path;
/// a non-UTF-8 document is refused (unlike `show`); a
/// document whose front-matter fails is stored by its path. Every refusal
/// stores nothing: the next ID is `PR-0001`. M: the immutability check
/// skipped for a path.
#[test]
fn ac05_a_path_target_resolves_as_show_resolves_it() {
    let pair = Pair::new("qpt-ac05", "spec-a");
    let cwd = pair.linked.clone();
    write(&cwd, "NOTES.md", "# Notes\n\nNot under the roots.\n");
    write(
        &cwd,
        "docs/features/latin1.md",
        b"---\nclass: spec\n---\n\n# Caf\xe9 notes\n",
    );
    let generated = "docs/features/glossary.md";
    write(
        &cwd,
        generated,
        "---\nclass: generated\ngenerator: glossary tool\n---\n\n# Glossary\n\nTerms.\n",
    );
    let (base, text) = pair.span(&cwd, TUNING);
    let new_text = tuned(&text);
    let state = pair.state();
    let before = queue_state(&pair);

    let absolute = cwd.join("docs/spec/game.md");
    let absolute = absolute.to_str().expect("a UTF-8 scratch path");
    for unclean in [
        "../spec-a/docs/spec/game.md",
        absolute,
        "docs/./spec/game.md",
        "docs//spec/game.md",
    ] {
        let message = cannot(&pair.propose(&cwd, unclean, &base, &new_text), unclean);
        assert_eq!(message, format!("spec: `{unclean}` {UNCLEAN}"));
        let outcome = ask(&pair, &cwd, &question(&pair, &cwd, &[unclean], "Clean?"));
        let message = intake_cannot(&pair, &before, &outcome, unclean);
        assert_eq!(message, format!("spec: node_ids[0]: `{unclean}` {UNCLEAN}"));
        let outcome = report(&pair, &cwd, discrepancy(&[TUNING, unclean], "Departs."));
        let message = intake_cannot(&pair, &before, &outcome, unclean);
        assert_eq!(message, format!("spec: node_ids[1]: `{unclean}` {UNCLEAN}"));
        let mut input = discrepancy(&[TUNING], "Departs.");
        input.proposed_patch = Some(ProposedPatch {
            target: unclean.to_owned(),
            base: base.clone(),
            text: new_text.clone(),
            rationale: "Tune.".to_owned(),
        });
        let message = intake_cannot(&pair, &before, &report(&pair, &cwd, input), unclean);
        assert_eq!(
            message,
            format!("spec: proposed_patch: `{unclean}` {UNCLEAN}")
        );
    }

    for missing in ["docs/spec/missing.md", "NOTES.md", "docs/SPEC/game.md"] {
        let reason = format!("`{missing}` {NOT_INDEXED}");
        assert_eq!(shown(&pair, &cwd, missing), Err(reason.clone()), "show");
        let outcome = pair.propose(&cwd, missing, &base, &new_text);
        assert_eq!(refused(&outcome, missing), reason);
        let outcome = ask(&pair, &cwd, &question(&pair, &cwd, &[missing], "Here?"));
        assert_eq!(
            intake_refused(&pair, &before, &outcome, missing),
            format!("node_ids[0]: {reason}")
        );
        let mut input = discrepancy(&[TUNING], "Departs.");
        input.proposed_patch = Some(ProposedPatch {
            target: missing.to_owned(),
            base: base.clone(),
            text: new_text.clone(),
            rationale: "Tune.".to_owned(),
        });
        assert_eq!(
            intake_refused(&pair, &before, &report(&pair, &cwd, input), missing),
            format!("proposed_patch: {reason}")
        );
    }

    // Immutable by its `id:`'s prefix: no update, a question allowed.
    let r12 = "docs/records/R/R-12.md";
    let (r12_base, r12_text) = pair.span(&cwd, r12);
    let outcome = pair.propose(
        &cwd,
        r12,
        &r12_base,
        &edit(&r12_text, "a short delay", "a 1.2 s delay"),
    );
    assert_eq!(
        refused(&outcome, "immutable"),
        "`R-12`: `R-12` has the prefix `R`, whose text is immutable (`immutable_text` in \
         `[ids]`): it is never updated"
    );

    // `class: generated` without an `id:`: refused by its path.
    let (generated_base, generated_text) = pair.span(&cwd, generated);
    let outcome = pair.propose(
        &cwd,
        generated,
        &generated_base,
        &edit(&generated_text, "Terms.", "Terms and names."),
    );
    assert_eq!(
        refused(&outcome, "generated, id-less"),
        format!(
            "`{generated}`: its file is a `class: generated` document: only its registered \
             generator writes it, never a proposal"
        )
    );

    // Not UTF-8: `show` gives the whole file, a proposal has no node.
    let latin1 = "docs/features/latin1.md";
    assert_eq!(shown(&pair, &cwd, latin1), Ok(1), "show reads it by path");
    let reason = format!("`{latin1}` is not UTF-8: it has no node to name");
    let (latin1_base, _) = pair.span(&cwd, latin1);
    let outcome = pair.propose(&cwd, latin1, &latin1_base, "# Cafe notes\n");
    assert_eq!(refused(&outcome, "not UTF-8"), reason);
    let outcome = ask(&pair, &cwd, &question(&pair, &cwd, &[latin1], "Cafe?"));
    assert_eq!(
        intake_refused(&pair, &before, &outcome, "not UTF-8"),
        format!("node_ids[0]: {reason}")
    );

    assert_eq!(pair.state(), state, "every refusal left git and the files");
    assert_eq!(queue_state(&pair), before);
    assert!(pair.events().is_empty(), "no event");

    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &[r12], "Is 1.5 s binding?"),
    );
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0001"), ["R-12"]);
    assert_eq!(pair.proposal("PR-0001").target_path, r12);

    // A front-matter that fails (its `id:` line included): the path.
    let broken = "docs/features/broken.md";
    write(
        &cwd,
        broken,
        "---\nid: MEC-BROKEN\nclass: [spec\n---\n\n# Broken\n\nThe tank refills.\n",
    );
    let (broken_base, broken_text) = pair.span(&cwd, broken);
    let id = stored(
        pair.propose(
            &cwd,
            broken,
            &broken_base,
            &edit(
                &broken_text,
                "The tank refills.",
                "The tank refills slowly.",
            ),
        ),
        "broken front-matter",
    );
    assert_eq!(id, "PR-0002");
    let row = pair.proposal(&id);
    assert_eq!(
        (row.target_id.as_str(), row.target_path.as_str()),
        (broken, broken)
    );
    let outcome = asked(&pair, &cwd, &question(&pair, &cwd, &[broken], "Refills?"));
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0003"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0003"), [broken]);
    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &[generated], "Who writes it?"),
    );
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0004"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0004"), [generated]);

    // spec-b: generated, refused for an update, allowed for a question.
    let pair = Pair::new("qpt-ac05-b", "spec-b");
    let cwd = pair.linked.clone();
    let gls = "docs/records/GLS/GLS-task-branch.md";
    let (gls_base, gls_text) = pair.span(&cwd, gls);
    let state = pair.state();
    let outcome = pair.propose(
        &cwd,
        gls,
        &gls_base,
        &edit(&gls_text, "GLS-worktree.", "GLS-worktree (see)."),
    );
    assert_eq!(
        refused(&outcome, "generated"),
        "`GLS-task-branch`: its file is a `class: generated` document: only its registered \
         generator writes it, never a proposal"
    );
    assert_eq!(pair.state(), state, "nothing stored or written");
    let outcome = asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &[gls], "Who writes it?"),
    );
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0001"), ["GLS-task-branch"]);
}

// ------------------------------------------------------------------ AC-06

/// AC-06: on a path, check 3 covers the whole file: a text adding `id:
/// MEC-TUNING` to the front-matter, dropping `{#AC-07}` or adding an `{#ID}`
/// section is refused; so is a stale base, naming the current hash; then
/// the valid update is `PR-0001`. M: check 3 skipped for a path.
#[test]
fn ac06_a_path_update_keeps_the_structure_and_its_base() {
    let pair = Pair::new("qpt-ac06", "spec-a");
    let cwd = pair.linked.clone();
    let (base, text) = pair.span(&cwd, TUNING);
    let state = pair.state();
    let structure = |change: &str| {
        format!(
            "`{TUNING}`: the edit changes the file's structure: {change}; an update keeps every \
             `{{#ID}}` heading and its level"
        )
    };
    for (label, bad, reason) in [
        (
            "an id: added",
            edit(
                &text,
                "---\nclass: spec\n",
                "---\nid: MEC-TUNING\nclass: spec\n",
            ),
            structure("node 1 was (no ID) (the document), would be MEC-TUNING (the document)"),
        ),
        (
            "{#AC-07} dropped",
            edit(
                &text,
                "after the last sprint {#AC-07}\n",
                "after the last sprint\n",
            ),
            structure("node 2 was AC-07 (level 3), would be nothing"),
        ),
        (
            "an {#ID} section added",
            format!("{text}\n### Regeneration stops at once {{#AC-08}}\n\nVerifies R-12.\n"),
            structure("node 3 was nothing, would be AC-08 (level 3)"),
        ),
    ] {
        assert_eq!(
            refused(&pair.propose(&cwd, TUNING, &base, &bad), label),
            reason,
            "{label}"
        );
        assert_eq!(pair.state(), state, "{label}: nothing stored");
    }
    let stale = format!("b3:{}", "0".repeat(64));
    let reason = refused(
        &pair.propose(&cwd, TUNING, &stale, &tuned(&text)),
        "a stale base",
    );
    assert!(reason.contains(&base), "names the current hash: {reason}");
    assert_eq!(pair.state(), state, "nothing stored");
    assert!(pair.events().is_empty());
    let id = stored(pair.propose(&cwd, TUNING, &base, &tuned(&text)), "valid");
    assert_eq!(id, "PR-0001", "no ID used by a refusal");
}

// ------------------------------------------------------------------ AC-07

/// AC-07: `climb.md` (`class: canon`, no `id:`) and `DEC-0099` (accepted,
/// `canon: docs/spec/movement/climb.md#climb`) committed in the copy: a
/// question on the path is answered by `DEC-0099` (its title), not stored;
/// with `distinct_from: ["DEC-0099"]` stored under the path; asked again,
/// the queue's row hits too; an accepted decision that only links to the
/// file (a Markdown link) is related, never a hit. spec-b `docs/spec/cli.md`
/// is hit by `ADR-0001` (its `canon:` lands on a section) and stored as
/// `MOD-CLI`. M: no corpus dedup for a path; mentions as hits.
#[test]
fn ac07_a_decision_whose_canon_lands_in_the_file_answers_it() {
    let pair = Pair::new("qpt-ac07", "spec-a");
    let cwd = pair.linked.clone();
    let climb = "docs/spec/movement/climb.md";
    write(
        &cwd,
        climb,
        "---\nclass: canon\n---\n\n# Climb\n\nClimbing a wall drains stamina.\n",
    );
    write(
        &cwd,
        "docs/records/DEC/DEC-0099.md",
        "---\nid: DEC-0099\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         canon: docs/spec/movement/climb.md#climb\n---\n\n# Climbing costs stamina\n\n\
         Every wall climbed drains the tank.\n",
    );
    write(
        &cwd,
        "docs/records/DEC/DEC-0098.md",
        "---\nid: DEC-0098\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n---\n\n\
         # Walls stay dark\n\nSee [the climb](../../spec/movement/climb.md) for the cost.\n",
    );
    pair.commit_all(&cwd, "Climb and its decisions.");
    let before = queue_state(&pair);
    let text = "Does climbing cost stamina?";
    let outcome = asked(&pair, &cwd, &question(&pair, &cwd, &[climb], text));
    let (printed_text, json) = printed_intake(&outcome);
    let dec_0099 = json!({"id": "DEC-0099", "source": "corpus", "status": "accepted",
        "path": "docs/records/DEC/DEC-0099.md", "answer": "Climbing costs stamina",
        "record": "DEC-0099"});
    let dec_0098 = json!({"id": "DEC-0098", "source": "corpus", "status": "accepted",
        "path": "docs/records/DEC/DEC-0098.md", "answer": null, "record": null});
    assert_eq!(
        json,
        json!({"id": null, "created": false, "hits": [dec_0099], "related": [dec_0098],
            "linked": null, "diagnostics": [], "notes": []})
    );
    assert_eq!(
        printed_text,
        "hit: DEC-0099 | accepted | docs/records/DEC/DEC-0099.md | Climbing costs stamina\n\
         related: DEC-0098 | accepted | docs/records/DEC/DEC-0098.md | -\n\
         not stored: name every hit in `distinct_from` to store it anyway\n"
    );
    assert_eq!(queue_state(&pair), before, "nothing stored");

    let mut request = question(&pair, &cwd, &[climb], text);
    request.distinct_from = ids(&["DEC-0099"]);
    let outcome = asked(&pair, &cwd, &request);
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0001"), [climb]);
    assert_eq!(pair.proposal("PR-0001").target_id, climb);

    let again = asked(&pair, &cwd, &request);
    assert!(!again.document.created, "{again:?}");
    assert_eq!(hit_ids(&again), ["DEC-0099", "PR-0001"]);
    assert_eq!(pair.proposals().len(), 1);

    // spec-b: the document's links include its sections'.
    let pair = Pair::new("qpt-ac07-b", "spec-b");
    let cwd = pair.linked.clone();
    let cli = "docs/spec/cli.md";
    let text = "Does sync use clones?";
    let outcome = asked(&pair, &cwd, &question(&pair, &cwd, &[cli], text));
    assert_eq!(hit_ids(&outcome), ["ADR-0001"], "{outcome:?}");
    assert!(!outcome.document.created);
    assert_eq!(
        printed_intake(&outcome),
        printed_intake(&asked(
            &pair,
            &cwd,
            &question(&pair, &cwd, &["MOD-CLI"], text)
        )),
        "the path answers as its ID"
    );
    let mut request = question(&pair, &cwd, &[cli], text);
    request.distinct_from = ids(&["ADR-0001"]);
    let outcome = asked(&pair, &cwd, &request);
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0001"), ["MOD-CLI"]);
}

// ------------------------------------------------------------------ AC-08

/// AC-08: a node is named once, canonical: a path and its document's ID
/// (either order), one path twice, are refused naming the later field; a
/// discrepancy on `MEC-STAMINA` with `proposed_patch.target` its path is
/// stored with its linked update on `MEC-STAMINA`; one on the path patched
/// by ID, and one on an id-less path patched by that path, too. M: the
/// written forms compared.
#[test]
fn ac08_a_node_is_named_once_canonical() {
    let pair = Pair::new("qpt-ac08", "spec-a");
    let cwd = pair.linked.clone();
    let before = queue_state(&pair);
    for (node_ids, reason) in [
        (
            [STAMINA, "MEC-STAMINA"],
            "node_ids[1]: `MEC-STAMINA` names `MEC-STAMINA`, as node_ids[0] does: name each \
             node once"
                .to_owned(),
        ),
        (
            ["MEC-STAMINA", STAMINA],
            format!(
                "node_ids[1]: `{STAMINA}` names `MEC-STAMINA`, as node_ids[0] does: name each \
                 node once"
            ),
        ),
        (
            [TUNING, TUNING],
            format!(
                "node_ids[1]: `{TUNING}` names `{TUNING}`, as node_ids[0] does: name each node \
                 once"
            ),
        ),
    ] {
        let outcome = ask(&pair, &cwd, &question(&pair, &cwd, &node_ids, "Once?"));
        assert_eq!(
            intake_refused(&pair, &before, &outcome, &format!("{node_ids:?}")),
            reason
        );
        let outcome = report(&pair, &cwd, discrepancy(&node_ids, "Departs once."));
        assert_eq!(
            intake_refused(&pair, &before, &outcome, &format!("{node_ids:?}")),
            reason
        );
    }

    let (base, text) = pair.span(&cwd, STAMINA);
    let regen = edit(
        &text,
        "delay after sprinting 1.5 s",
        "delay after sprinting 1.2 s",
    );
    let patched = |node_ids: &[&str], summary: &str, target: &str, base: &str, text: &str| {
        let mut input = discrepancy(node_ids, summary);
        input.distinct_from = Some(ids(&["DEC-0023"]));
        input.proposed_patch = Some(ProposedPatch {
            target: target.to_owned(),
            base: base.to_owned(),
            text: text.to_owned(),
            rationale: "Match the code.".to_owned(),
        });
        input
    };
    let outcome = report(
        &pair,
        &cwd,
        patched(
            &["MEC-STAMINA"],
            "The delay is 1.2 s.",
            STAMINA,
            &base,
            &regen,
        ),
    )
    .expect("report");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0001"));
    assert_eq!(outcome.document.linked.as_deref(), Some("PR-0002"));
    assert_eq!(target_ids(&pair, "PR-0001"), ["MEC-STAMINA"]);
    let update = pair.proposal("PR-0002");
    assert_eq!(
        (update.target_id.as_str(), update.target_path.as_str()),
        ("MEC-STAMINA", STAMINA)
    );
    assert_eq!(update.new_text, regen);

    // The path among node_ids, the patch by ID.
    let outcome = report(
        &pair,
        &cwd,
        patched(
            &[STAMINA],
            "Regeneration starts early.",
            "MEC-STAMINA",
            &base,
            &regen,
        ),
    )
    .expect("report");
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0003"),
        "{outcome:?}"
    );
    assert_eq!(pair.proposal("PR-0004").target_id, "MEC-STAMINA");

    // An id-less document: its path on both sides.
    let (tuning_base, tuning_text) = pair.span(&cwd, TUNING);
    let mut input = patched(
        &[TUNING],
        "The tuned delay is 1.2 s.",
        TUNING,
        &tuning_base,
        &tuned(&tuning_text),
    );
    input.distinct_from = None;
    let outcome = report(&pair, &cwd, input).expect("report");
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0005"),
        "{outcome:?}"
    );
    assert_eq!(outcome.document.linked.as_deref(), Some("PR-0006"));
    assert_eq!(target_ids(&pair, "PR-0005"), [TUNING]);
    assert_eq!(pair.proposal("PR-0006").target_id, TUNING);
}

// ------------------------------------------------------------------ AC-10

fn env_at(home: &Path, cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

/// Library `export state` of `home`'s queue into `out`: its bytes.
fn export_to(pair: &Pair, home: &Path, out: &Path, now: &str) -> Vec<u8> {
    export_state(
        &env_at(home, &pair.main),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: now.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
    fs::read(out).expect("the dump")
}

/// AC-10: AC-01's update and AC-07's question (both path targets)
/// exported and imported into a fresh `HOME`: `dump()` equal, the re-export
/// byte-identical, format 2, `queue_schema` 4 (docs/features/task-package.md
/// "Backup"); the restored rows read back by
/// `review`. M: a read-time ID check on `target_id`.
#[test]
fn ac10_path_targets_round_trip_through_a_backup() {
    let pair = Pair::new("qpt-ac10", "spec-a");
    let cwd = pair.linked.clone();
    let (base, text) = pair.span(&cwd, TUNING);
    stored(pair.propose(&cwd, TUNING, &base, &tuned(&text)), "AC-01");
    let climb = "docs/spec/movement/climb.md";
    write(
        &cwd,
        climb,
        "---\nclass: canon\n---\n\n# Climb\n\nIt drains.\n",
    );
    pair.commit_all(&cwd, "Climb.");
    asked(
        &pair,
        &cwd,
        &question(&pair, &cwd, &[climb], "Does it drain?"),
    );
    let before = queue_state(&pair);

    let dumps = pair.scratch.dir("dumps");
    let bytes = export_to(&pair, &pair.home, &dumps.join("first.jsonl"), NOW);
    let text = String::from_utf8(bytes.clone()).expect("a UTF-8 dump");
    assert!(
        text.starts_with(
            "{\"format\":2,\"queue_schema\":4,\"project\":\"lantern-keep\",\"proposals\":2,\
             \"tasks\":0,\"runs\":0,"
        ),
        "{text}"
    );
    let rows: Vec<Value> = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            serde_json::from_str::<Value>(line)
                .ok()?
                .get("proposals")
                .cloned()
        })
        .collect();
    assert_eq!(rows.len(), 2, "{text}");
    assert_eq!(
        (&rows[0]["target_id"], &rows[0]["target_path"]),
        (&json!(TUNING), &json!(TUNING))
    );
    assert_eq!(
        (&rows[1]["target_id"], &rows[1]["target_path"]),
        (&json!(climb), &json!(climb))
    );
    let dumped_ids = match &rows[1]["target_ids"] {
        Value::String(stored) => serde_json::from_str::<Value>(stored).expect("JSON text"),
        other => other.clone(),
    };
    assert_eq!(dumped_ids, json!([climb]), "{text}");

    let fresh = pair.scratch.home("fresh");
    let mut consent = |_: &str| true;
    let restored = import_state(
        &env_at(&fresh, &pair.main),
        &Globals::default(),
        &ImportStateRequest {
            file: dumps.join("first.jsonl"),
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("import-state: {error}"));
    assert_eq!(restored.exit(), Exit::Answered, "{restored:?}");
    let db: PathBuf = common::data_dir(&fresh).join("lantern-keep.db");
    let after = SqliteQueue::open(&db, "lantern-keep")
        .expect("the restored queue")
        .dump()
        .expect("dump");
    assert_eq!(after, before, "the queue as it was");
    let again = export_to(&pair, &fresh, &dumps.join("again.jsonl"), LATER);
    assert_eq!(again, bytes, "the re-export is byte-identical");

    let review = specengine_cli::review(
        &env_at(&fresh, &pair.main),
        &Globals::default(),
        &specengine_cli::ReviewRequest {
            id: "PR-0002".to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .expect("review of a restored path row");
    let json = json_of(&printed(&review).1);
    assert_eq!(json["target_id"], json!(climb));
    assert_eq!(json["target_ids"], json!([climb]));
}

// ------------------------------------------------------------------ AC-11

/// AC-11 (Rules 8): a walked name with U+202E: a question and an update on
/// it are stored under the path raw; inbox, review, the approve prompt and
/// the binary's stderr show `\u{202e}`; JSON keeps it raw. M: the target
/// printed raw.
#[test]
fn ac11_a_path_with_a_bidi_mark_is_escaped_in_text_raw_in_json() {
    let pair = Pair::new("qpt-ac11", "spec-a");
    let cwd = pair.linked.clone();
    let raw = "docs/features/a\u{202e}b.md";
    let escaped = "docs/features/a\\u{202e}b.md";
    write(
        &cwd,
        raw,
        "---\nclass: spec\n---\n\n# Mirrored\n\nThe tank refills.\n",
    );
    pair.commit_all(&cwd, "A mirrored name.");
    let outcome = asked(&pair, &cwd, &question(&pair, &cwd, &[raw], "Mirrored?"));
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "{outcome:?}"
    );
    assert_eq!(target_ids(&pair, "PR-0001"), [raw]);
    let (base, text) = pair.span(&cwd, raw);
    let id = stored(
        pair.propose(
            &cwd,
            raw,
            &base,
            &edit(&text, "The tank refills.", "The tank refills at rest."),
        ),
        "update",
    );
    assert_eq!(id, "PR-0002");

    let (text, json) = printed_inbox(&pair.inbox(&cwd, false).expect("inbox"));
    assert_eq!(
        text,
        format!(
            "PR-0001 | question | open | {escaped} | t1 | {NOW} | normal: Mirrored?\n\
             PR-0002 | update | open | {escaped} | t1 | {NOW} | Update {escaped}.\n"
        )
    );
    let json = json_of(&json);
    assert_eq!(json["proposals"][0]["target_id"], json!(raw));
    assert_eq!(json["proposals"][1]["target_id"], json!(raw));

    for id in ["PR-0001", "PR-0002"] {
        let (text, json) = printed(&pair.review_ok(&cwd, id));
        for line in [
            format!("\ntarget_id: {escaped}\n"),
            format!("\ntarget_path: {escaped}\n"),
            format!("\ntarget_ids: 1\n  {escaped}\n"),
        ] {
            assert!(text.contains(&line), "{id}: {line:?} in {text}");
        }
        assert!(!text.contains('\u{202e}'), "{id}: raw in {text}");
        let json = json_of(&json);
        assert_eq!(json["target_id"], json!(raw), "{id}");
        assert_eq!(json["target_path"], json!(raw), "{id}");
        assert_eq!(json["target_ids"], json!([raw]), "{id}");
    }

    let (outcome, questions) =
        pair.approve_answer(&pair.main, "PR-0002", false, pair.git_env(&pair.main));
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert!(
        questions[0].contains(escaped) && !questions[0].contains('\u{202e}'),
        "the prompt escapes the path: {:?}",
        questions[0]
    );

    // stderr of the binary: exit 1 (no such file) and exit 2 (not clean).
    let missing = "docs/features/c\u{202e}d.md";
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "question",
            missing,
            "--text",
            "Here?",
            "--working-answer",
            "no",
            "--price-of-other",
            "none",
        ],
        b"",
    );
    assert_eq!(run.code, 1, "{run:?}");
    assert_eq!(
        run.stderr,
        format!("spec: node_ids[0]: `docs/features/c\\u{{202e}}d.md` {NOT_INDEXED}\n"),
        "{run:?}"
    );
    let unclean = "../c\u{202e}d.md";
    let texts = pair.scratch.dir("texts");
    write(&texts, "new.md", "# Mirrored\n");
    let text_file = texts.join("new.md");
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "update",
            unclean,
            "--base",
            &base,
            "--text-file",
            text_file.to_str().expect("a UTF-8 scratch path"),
            "--rationale",
            "r",
        ],
        b"",
    );
    assert_eq!(run.code, 2, "{run:?}");
    assert_eq!(
        run.stderr,
        format!("spec: `../c\\u{{202e}}d.md` {UNCLEAN}\n"),
        "{run:?}"
    );
}

// ------------------------------------------------------- deviation 3, completion

/// The developer's deviation 3: a path row whose `target_path` is not its
/// `target_id` (set in the database) is refused at step 4 naming both,
/// nothing applied. M: `document_of` without the comparison.
#[test]
fn a_path_row_applies_only_at_its_recorded_path() {
    let pair = Pair::new("qpt-dev3", "spec-a");
    let (base, text) = pair.span(&pair.linked, TUNING);
    let id = stored(
        pair.propose(&pair.linked, TUNING, &base, &tuned(&text)),
        "update",
    );
    pair.sql(&format!(
        "UPDATE proposals SET target_path = 'docs/spec/game.md' WHERE id = '{id}';"
    ));
    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "another recorded path");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 4): `{TUNING}` names a document by its path, not the \
             recorded `docs/spec/game.md`"
        )
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "nothing applied");
}

/// A path row committed by hand with its `Proposal:` trailer completes by
/// that commit (the trailer lookup reads the document of the commit's
/// blob): `applied`, no new commit.
#[test]
fn a_path_update_committed_by_hand_completes() {
    let pair = Pair::new("qpt-complete", "spec-a");
    let (base, text) = pair.span(&pair.linked, TUNING);
    let new_text = tuned(&text);
    let id = stored(
        pair.propose(&pair.linked, TUNING, &base, &new_text),
        "update",
    );
    write(&pair.linked, TUNING, &new_text);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            "Tune by hand.",
            "-m",
            &format!("Proposal: {id}"),
            "--",
            TUNING,
        ],
    );
    let commit = pair.rev(&pair.linked, "t1");
    let review = pair.review_ok(&pair.main, &id);
    let note = format!(
        "its commit {commit} is on `t1`: `spec approve {id}` completes it; no new apply is needed"
    );
    assert!(
        review.document.notes.contains(&note),
        "{:?}",
        review.document.notes
    );
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert!(questions[0].starts_with(&format!("complete {id} by its commit {commit}")));
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    let row = pair.proposal(&id);
    assert_eq!(row.status.as_str(), "applied");
    assert_eq!(row.applied_commit.as_deref(), Some(commit.as_str()));
}

/// "Known limits": a document given an `id:` after a path row was stored
/// keeps that row; approve still finds the file (the whole-file merge keeps
/// the new `id:` and the edit, `rebases`); a question by path now names
/// the ID and is not joined to the path row of the same text (no queue
/// hit, created).
#[test]
fn a_document_given_an_id_later_keeps_its_path_rows() {
    let pair = Pair::new("qpt-limit", "spec-a");
    let cwd = pair.linked.clone();
    let (base, text) = pair.span(&cwd, TUNING);
    let id = stored(pair.propose(&cwd, TUNING, &base, &tuned(&text)), "update");
    let text = "Is 1.2 s the delay to tune to?";
    let first = asked(&pair, &cwd, &question(&pair, &cwd, &[TUNING], text));
    assert_eq!(first.document.id.as_deref(), Some("PR-0002"), "{first:?}");
    replace(
        &cwd,
        TUNING,
        "---\nclass: spec\n",
        "---\nid: MEC-TUNING\nclass: spec\n",
    );
    pair.commit_all(&cwd, "Give the tuning an ID.");

    let again = asked(&pair, &cwd, &question(&pair, &cwd, &[TUNING], text));
    assert_eq!(again.document.id.as_deref(), Some("PR-0003"), "{again:?}");
    assert!(hit_ids(&again).is_empty(), "{again:?}");
    assert_eq!(target_ids(&pair, "PR-0003"), ["MEC-TUNING"]);

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("approve: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions[0].ends_with(" (rebases)? [y/N]"), "{questions:?}");
    let now = read_text(&cwd, TUNING);
    for kept in [
        "---\nid: MEC-TUNING\nclass: spec\n",
        "priority: low\n",
        "first regeneration tick 1.2 s later.",
    ] {
        assert!(now.contains(kept), "{kept:?} kept: {now}");
    }
    assert_eq!(pair.proposal(&id).status.as_str(), "applied");
    assert_eq!(
        pair.proposal(&id).target_id,
        TUNING,
        "the row keeps its path"
    );
}

// ------------------------------------------------------------- determinism

/// The same path proposal in two fresh repositories: equal rows (`dump()`
/// minus the places' paths) and equal printed answers.
#[test]
fn a_path_proposal_is_deterministic() {
    let answers: Vec<(String, String, String)> = (0..2)
        .map(|run| {
            let pair = Pair::new(&format!("qpt-det-{run}"), "spec-a");
            let cwd = pair.linked.clone();
            let (base, text) = pair.span(&cwd, TUNING);
            let outcome = pair
                .propose(&cwd, TUNING, &base, &tuned(&text))
                .expect("propose");
            let row = pair.proposal("PR-0001");
            let review = json_of(&printed(&pair.review_ok(&cwd, "PR-0001")).1);
            (
                printed(&outcome).0,
                format!("{} {} {}", row.target_id, row.patch_hash, row.base_hash),
                review["diff"].to_string(),
            )
        })
        .collect();
    assert_eq!(answers[0], answers[1]);
}
