//! docs/features/decision-apply.md through the CLI library (and the binary
//! on a terminal where the terminal is the subject): `spec approve` of a
//! question or a discrepancy writes the owner's choice as an accepted
//! decision record in the project's own shape, one new file and one `spec:
//! apply PR-…` commit in the recorded worktree. AC-01 to AC-15 (AC-12's
//! MCP half: `specengine-mcp/tests/mcp_decision.rs`; AC-16:
//! `plugin_skills.rs`, `plugin_files.rs`; the store's and core's halves in
//! their crates), and the "Known limits" case of a queue restored from
//! before step 7; then the rules of the review's iteration 2: a template's
//! `{{canon}}` and refused characters, `spec reject` without history, a
//! record an interrupted apply left, a held record completed by hand, the
//! title from the answer's first line of text.
//!
//! Setup (the AC's): scratch git repositories of `fixtures/spec-a`
//! (`[decision_records]` `DEC`, `docs/records/DEC`) and `fixtures/spec-b`
//! (`ADR`, `docs/records/ADR`), each with its committed
//! `templates/decision.md` outside the walked roots; a scratch `HOME`; the
//! clock `2026-10-05T12:00:00Z`; the sandbox's git identity; items raised
//! in the linked worktree on `t1` with `distinct_from` naming their hits;
//! library approve from the main worktree, consent yes unless said.
//! "Refused": nothing written, the row as before.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use common::check::library;
use common::decision::{CLOCK, DAY, THREE, discrepancy, flags, option, with_records};
use common::proposal::{DECIDER, Pair, cannot, json_of, printed, printed_inbox, refused};
use common::{read_text, write};
use serde_json::{Value, json};
use specengine_cli::{ApproveFlags, Exit, Globals, Message, ReviewRequest, review_brief};
use specengine_store::{Choice, ProposalQueue as _, ProposalStatus, RecordSeries, create_file};

/// AC-01's discrepancy summary.
const SUMMARY: &str = "Regeneration starts while sprinting in the code";

/// The record path of `id` under spec-a's `dir`.
fn dec_path(id: &str) -> String {
    format!("docs/records/DEC/{id}.md")
}

/// The record spec-a's template renders (the fixture's
/// `templates/decision.md`): every value as the engine places it.
#[allow(clippy::too_many_arguments)]
fn spec_a_record(
    id: &str,
    title_quoted: &str,
    canon: &str,
    targets: &str,
    proposal: &str,
    body: &RecordBody<'_>,
) -> String {
    format!(
        "---\nid: {id}\nclass: decision\nstatus: accepted\ndate: {DAY}\ntitle: {title_quoted}\n\
         canon: {canon}\nlinks:\n  answers: {targets}\nref: {proposal}\nscope: [decisions]\n---\n\n\
         # {title}\n\nChoice:\n\n{choice}\n\nEffect:\n\n{effect}\n\nCost:\n\n{cost}\n\n\
         Summary:\n\n{summary}\n\nOptions:\n\n{options}\n\nEvidence:\n\n{evidence}\n\n\
         Note:\n\n{note}\n\nDecided by:\n\n{DECIDER}\n",
        title = body.title,
        choice = body.choice,
        effect = body.effect,
        cost = body.cost,
        summary = body.summary,
        options = body.options,
        evidence = body.evidence,
        note = body.note,
    )
}

/// The free-text slots of a record body.
struct RecordBody<'a> {
    title: &'a str,
    choice: &'a str,
    effect: &'a str,
    cost: &'a str,
    summary: &'a str,
    options: &'a str,
    evidence: &'a str,
    note: &'a str,
}

/// AC-01's options, as the `options` slot lists them.
const THREE_LISTED: &str = "- Keep the spec | Fix the code to regenerate at rest | one work item\n\
- Change the spec | Regeneration also runs while sprinting | a new rule case and a test\n\
- Defer | Nothing changes now | the gap stays open";

/// The fixture discrepancy's one piece of evidence, as listed.
const EVIDENCE_LISTED: &str =
    "- src/stamina.rs:10-20 regen_system | regenerates while sprinting | regenerates only at rest";

/// The consent question of a record: `introduced: <n>` and its lines, the
/// record indented two spaces (empty lines empty), the closing question.
fn record_question(
    findings: &[&str],
    id: &str,
    path: &str,
    record: &str,
    pr: &str,
    choice: &str,
    pair: &Pair,
) -> String {
    let mut question = format!("introduced: {}\n", findings.len());
    for finding in findings {
        question.push_str(finding);
        question.push('\n');
    }
    question.push_str(&format!("record {id} at {path}:\n"));
    for line in record.split_terminator('\n') {
        if !line.is_empty() {
            question.push_str("  ");
            question.push_str(line);
        }
        question.push('\n');
    }
    question.push_str(&format!(
        "apply {pr} as {id} ({choice}) on t1 in {}? [y/N]",
        pair.linked.display()
    ));
    question
}

/// `git diff-tree --name-status` of `commit` (no merges): its lines.
fn name_status(pair: &Pair, commit: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["diff-tree", "-r", "--no-commit-id", "--name-status", commit],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `(code, path, line, subject)` of every finding of `root`'s check.
fn findings(root: &Path) -> Vec<(String, String, usize, String)> {
    library(root)
        .findings
        .iter()
        .map(|finding| {
            (
                finding.code.clone(),
                finding.path.clone(),
                finding.line,
                finding.subject.clone(),
            )
        })
        .collect()
}

/// The queue's next record ID of spec-a's `DEC` (width 4) over the
/// fixture's corpus (`DEC-0023` its highest).
fn next_dec(pair: &Pair) -> String {
    pair.queue()
        .next_record(&RecordSeries {
            prefix: "DEC".to_owned(),
            width: 4,
            corpus_max: 23,
        })
        .expect("next_record")
}

/// AC-01's item: the discrepancy on `RULE-STAM-REGEN` with three options,
/// raised in the linked worktree; its ID.
fn ac01_item(pair: &Pair) -> String {
    pair.raise_discrepancy(
        &pair.linked,
        discrepancy(&["RULE-STAM-REGEN"], SUMMARY, &THREE),
    )
}

// ------------------------------------------------------------------ AC-01

/// AC-01: spec-a, a discrepancy on `RULE-STAM-REGEN` with three options,
/// a tracked file modified, `--option 1`: one commit, one parent (the old
/// `HEAD`), name-status exactly `A docs/records/DEC/DEC-0024.md`, its
/// message `spec: apply`, the body `DEC-0024: <label 1>` and the four
/// trailers; the record exactly as the template renders it (`status:
/// accepted`, `date: 2026-10-05`, `canon: RULE-STAM-REGEN`, option 1's
/// label, effect and price); the other file still modified, unstaged; the
/// check's findings as before; `applied`, the record stored; the consent
/// question shows the record; stdout `applied … : DEC-0024 <path>`; the
/// inbox ends ` [DEC-0024]`; review's five keys. M: staging the whole tree.
#[test]
fn ac01_an_option_becomes_one_record_in_one_commit() {
    let pair = Pair::new("da-ac01", "spec-a");
    let id = ac01_item(&pair);
    assert_eq!(id, "PR-0001");
    let before_findings = findings(&pair.linked);
    write(
        &pair.linked,
        "docs/spec/game.md",
        format!(
            "{}\nA local edit.\n",
            read_text(&pair.linked, "docs/spec/game.md")
        ),
    );
    let base = pair.rev(&pair.linked, "HEAD");
    let base_commit = pair.proposal(&id).place.base_commit;
    assert_eq!(next_dec(&pair), "DEC-0024");

    let (outcome, question) = pair.decide_ok(&pair.main, &id, &option(1), None);
    let path = dec_path("DEC-0024");
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(
        pair.rev(&pair.linked, &format!("{head}^")),
        base,
        "one parent"
    );
    assert_eq!(
        pair.git_text(&pair.main, &["rev-list", "--count", &format!("{base}..t1")]),
        "1",
        "one commit"
    );
    assert_eq!(
        pair.git_text(&pair.main, &["rev-list", "--parents", "-n", "1", &head])
            .split(' ')
            .count(),
        2,
        "exactly one parent"
    );
    assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]);
    assert_eq!(
        message,
        format!(
            "spec: apply PR-0001\n\nDEC-0024: Change the spec\n\nProposal: PR-0001\n\
             Decided-by: {DECIDER}\nProposed-by: agent role=developer model=claude-opus-5-5 run=unknown\n\
             Base-commit: {base_commit}"
        )
    );
    let record = read_text(&pair.linked, &path);
    let want = spec_a_record(
        "DEC-0024",
        "\"Change the spec\"",
        "RULE-STAM-REGEN",
        "[RULE-STAM-REGEN]",
        "PR-0001",
        &RecordBody {
            title: "Change the spec",
            choice: "Change the spec",
            effect: "Regeneration also runs while sprinting",
            cost: "a new rule case and a test",
            summary: SUMMARY,
            options: THREE_LISTED,
            evidence: EVIDENCE_LISTED,
            note: "",
        },
    );
    assert_eq!(record, want);
    assert_eq!(
        pair.git_text(&pair.main, &["show", &format!("{head}:{path}")]),
        want.trim_end_matches('\n'),
        "the committed blob is the record"
    );
    assert_eq!(
        question,
        record_question(&[], "DEC-0024", &path, &want, &id, "option 1", &pair)
    );
    // The other file: still modified, never staged or committed.
    assert_eq!(pair.porcelain(&pair.linked), " M docs/spec/game.md\n");
    assert_eq!(
        findings(&pair.linked),
        before_findings,
        "the check as before"
    );

    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(stored.decided_at.as_deref(), Some(CLOCK));
    let kept = stored.record.expect("the record stored");
    assert_eq!(
        (kept.id.as_str(), kept.path.as_str(), kept.title.as_str()),
        ("DEC-0024", path.as_str(), "Change the spec")
    );
    assert_eq!(kept.text, want);
    assert_eq!(kept.choice, Choice::Option(1));
    assert_eq!(
        pair.events_of(&id),
        [
            ("proposal.created".to_owned(), None),
            ("proposal.approved".to_owned(), None),
            ("proposal.applied".to_owned(), None),
        ]
    );
    for event in pair
        .events()
        .iter()
        .filter(|event| event.payload["id"] == id)
    {
        if event.event_type != "proposal.created" {
            assert_eq!(event.payload["record"], json!("DEC-0024"), "{event:?}");
        }
    }
    assert_eq!(next_dec(&pair), "DEC-0025", "the queue issued DEC-0024");

    let (text, json) = printed(&outcome);
    assert_eq!(
        text,
        format!("applied PR-0001 as {head} on t1: DEC-0024 {path}\n")
    );
    let json = json_of(&json);
    assert_eq!(json["record_id"], json!("DEC-0024"));
    assert_eq!(json["record_path"], json!(path));
    assert_eq!(json["record_title"], json!("Change the spec"));
    assert_eq!(json["record_text"], json!(want));
    assert_eq!(json["choice"], json!({"option": 1}));

    let (inbox, inbox_json) = printed_inbox(&pair.inbox(&pair.main, true).expect("inbox"));
    assert_eq!(
        inbox,
        format!(
            "PR-0001 | discrepancy | applied | RULE-STAM-REGEN | t1 | {} | normal: {SUMMARY} \
             [DEC-0024]\n",
            common::proposal::NOW
        )
    );
    assert_eq!(
        json_of(&inbox_json)["proposals"][0]["record_id"],
        json!("DEC-0024")
    );
    let (review, review_json) = printed(&pair.review_ok(&pair.main, &id));
    let review_json = json_of(&review_json);
    assert_eq!(review_json["choice"], json!({"option": 1}));
    assert_eq!(review_json["record_text"], json!(want));
    assert!(review.contains("\nrecord_id: DEC-0024\n"), "{review}");
    assert!(
        review.contains(&format!("\nrecord_path: {path}\n")),
        "{review}"
    );
    assert!(review.contains("\nchoice: {\"option\":1}\n"), "{review}");
}

/// AC-01 through the binary on a terminal: `spec approve PR-0001 --option
/// 1` shows the record (stderr: `record DEC-0024 at <path>:`, the record
/// indented two spaces, the question) and prints `applied … : DEC-0024
/// <path>`; one commit adding the record.
#[test]
fn ac01_the_binary_asks_on_the_terminal_and_names_the_record() {
    let pair = Pair::new("da-ac01-tty", "spec-a");
    let id = ac01_item(&pair);
    let base = pair.rev(&pair.linked, "HEAD");
    let run = pair.spec_pty(&pair.main, &["approve", &id, "--option", "1"], Some("y"));
    assert_eq!(run.code, 0, "{}", run.output);
    assert!(run.asked, "{}", run.output);
    let path = dec_path("DEC-0024");
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(pair.rev(&pair.linked, &format!("{head}^")), base);
    assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
    let record = read_text(&pair.linked, &path);
    assert!(record.starts_with("---\nid: DEC-0024\nclass: decision\nstatus: accepted\ndate: "));
    let mut shown = format!("record DEC-0024 at {path}:\n");
    for line in record.split_terminator('\n') {
        if !line.is_empty() {
            shown.push_str("  ");
            shown.push_str(line);
        }
        shown.push('\n');
    }
    shown.push_str(&format!(
        "apply {id} as DEC-0024 (option 1) on t1 in {}? [y/N]",
        pair.linked.display()
    ));
    assert!(run.output.contains(&shown), "{}", run.output);
    assert!(
        run.output
            .contains(&format!("applied {id} as {head} on t1: DEC-0024 {path}\n")),
        "{}",
        run.output
    );
}

// ------------------------------------------------------------------ AC-02

/// AC-02: a question on `Q-031`, no flag: the working answer is the
/// choice, its first line the title, `cost` empty (and no effect, option
/// or evidence); `--answer T` with `--note`: `T` the choice and title,
/// `cost` the question's `price_of_other`, the note in its slot. M: cost
/// always `price_of_other`.
#[test]
fn ac02_a_question_takes_its_working_answer_or_the_given_one() {
    let pair = Pair::new("da-ac02", "spec-a");
    let text = "Does regeneration wait for full rest?";
    let first = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        text,
        "Yes:   it waits\nfor a full second.",
        "a new case in the rule",
    );
    let (outcome, question) = pair.decide_ok(&pair.main, &first, &flags(None, None, None), None);
    let path = dec_path("DEC-0024");
    let want = spec_a_record(
        "DEC-0024",
        "\"Yes: it waits\"",
        "Q-031",
        "[Q-031]",
        &first,
        &RecordBody {
            title: "Yes: it waits",
            choice: "Yes:   it waits\nfor a full second.",
            effect: "",
            cost: "",
            summary: text,
            options: "",
            evidence: "",
            note: "",
        },
    );
    assert_eq!(read_text(&pair.linked, &path), want);
    assert!(
        question.ends_with(&format!(
            "apply {first} as DEC-0024 (the working answer) on t1 in {}? [y/N]",
            pair.linked.display()
        )),
        "{question}"
    );
    assert_eq!(
        pair.proposal(&first).record.expect("stored").choice,
        Choice::WorkingAnswer
    );
    assert_eq!(
        json_of(&printed(&outcome).1)["choice"],
        json!({"working_answer": true})
    );

    let text = "Is the delay configurable per level?";
    let second = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        text,
        "No",
        "a settings key and a test",
    );
    let (outcome, question) = pair.decide_ok(
        &pair.main,
        &second,
        &flags(None, Some("Yes, per level\nwith a default."), None),
        Some("Owner note here."),
    );
    let path = dec_path("DEC-0025");
    let want = spec_a_record(
        "DEC-0025",
        "\"Yes, per level\"",
        "Q-031",
        "[Q-031]",
        &second,
        &RecordBody {
            title: "Yes, per level",
            choice: "Yes, per level\nwith a default.",
            effect: "",
            cost: "a settings key and a test",
            summary: text,
            options: "",
            evidence: "",
            note: "Owner note here.",
        },
    );
    assert_eq!(read_text(&pair.linked, &path), want);
    assert!(question.contains("(the given answer) on t1"), "{question}");
    let stored = pair.proposal(&second);
    assert_eq!(stored.decision_note.as_deref(), Some("Owner note here."));
    assert_eq!(
        stored.record.expect("stored").choice,
        Choice::Answer("Yes, per level\nwith a default.".to_owned())
    );
    assert_eq!(
        json_of(&printed(&outcome).1)["choice"],
        json!({"answer": "Yes, per level\nwith a default."})
    );
}

// ------------------------------------------------------------------ AC-03

/// AC-03, the run half (the scan half: `proposal_genre.rs`): spec-b, a
/// discrepancy on `CMD-SYNC` with Cyrillic labels, `--option 0` →
/// `docs/records/ADR/ADR-0003.md` (`ADR-0001`, `ADR-0002` in the corpus),
/// spec-b's own template, the labels verbatim, `canon: CMD-SYNC`. M:
/// `"DEC"`.
#[test]
fn ac03_spec_b_writes_its_own_record_with_its_labels_verbatim() {
    let pair = Pair::new("da-ac03", "spec-b");
    // Russian: "Fix the code" | "The code follows the spec" | "one task";
    // "Change the spec" | "The spec follows the code" | "a rule and a test".
    let keep = "\u{0418}\u{0441}\u{043f}\u{0440}\u{0430}\u{0432}\u{0438}\u{0442}\u{044c} \u{043a}\u{043e}\u{0434}";
    let keep_effect = "\u{041a}\u{043e}\u{0434} \u{0441}\u{043b}\u{0435}\u{0434}\u{0443}\u{0435}\u{0442} \u{0441}\u{043f}\u{0435}\u{0446}\u{0438}\u{0444}\u{0438}\u{043a}\u{0430}\u{0446}\u{0438}\u{0438}";
    let keep_price =
        "\u{043e}\u{0434}\u{043d}\u{0430} \u{0437}\u{0430}\u{0434}\u{0430}\u{0447}\u{0430}";
    let change = "\u{0418}\u{0437}\u{043c}\u{0435}\u{043d}\u{0438}\u{0442}\u{044c} \u{0441}\u{043f}\u{0435}\u{0446}\u{0438}\u{0444}\u{0438}\u{043a}\u{0430}\u{0446}\u{0438}\u{044e}";
    let change_effect = "\u{0421}\u{043f}\u{0435}\u{0446}\u{0438}\u{0444}\u{0438}\u{043a}\u{0430}\u{0446}\u{0438}\u{044f} \u{0441}\u{043b}\u{0435}\u{0434}\u{0443}\u{0435}\u{0442} \u{043a}\u{043e}\u{0434}\u{0443}";
    let change_price = "\u{043f}\u{0440}\u{0430}\u{0432}\u{0438}\u{043b}\u{043e} \u{0438} \u{0442}\u{0435}\u{0441}\u{0442}";
    // Russian: "sync writes to another's branch".
    let summary = "sync \u{043f}\u{0438}\u{0448}\u{0435}\u{0442} \u{0432} \u{0447}\u{0443}\u{0436}\u{0443}\u{044e} \u{0432}\u{0435}\u{0442}\u{043a}\u{0443}";
    let id = pair.raise_discrepancy(
        &pair.linked,
        discrepancy(
            &["CMD-SYNC"],
            summary,
            &[
                (keep, keep_effect, keep_price),
                (change, change_effect, change_price),
            ],
        ),
    );
    let base = pair.rev(&pair.linked, "HEAD");
    let (outcome, question) = pair.decide_ok(&pair.main, &id, &option(0), None);
    let path = "docs/records/ADR/ADR-0003.md";
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(pair.rev(&pair.linked, &format!("{head}^")), base);
    assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
    let record = read_text(&pair.linked, path);
    // spec-b's template: its own body labels.
    let want = format!(
        "---\nid: ADR-0003\nclass: decision\nstatus: accepted\ndate: {DAY}\ntitle: \"{keep}\"\n\
         canon: CMD-SYNC\nlinks:\n  answers: [CMD-SYNC]\nref: {id}\nscope: [decisions]\n---\n\n\
         # {keep}\n\n\u{0412}\u{044b}\u{0431}\u{043e}\u{0440}:\n\n{keep}\n\n\
         \u{041f}\u{043e}\u{0441}\u{043b}\u{0435}\u{0434}\u{0441}\u{0442}\u{0432}\u{0438}\u{044f}:\n\n{keep_effect}\n\n\
         \u{0426}\u{0435}\u{043d}\u{0430}:\n\n{keep_price}\n\n\
         \u{0421}\u{0443}\u{0442}\u{044c}:\n\n{summary}\n\n\
         \u{0412}\u{0430}\u{0440}\u{0438}\u{0430}\u{043d}\u{0442}\u{044b}:\n\n\
         - {keep} | {keep_effect} | {keep_price}\n- {change} | {change_effect} | {change_price}\n\n\
         \u{0421}\u{0432}\u{0438}\u{0434}\u{0435}\u{0442}\u{0435}\u{043b}\u{044c}\u{0441}\u{0442}\u{0432}\u{0430}:\n\n{EVIDENCE_LISTED}\n\n\
         \u{041f}\u{0440}\u{0438}\u{043c}\u{0435}\u{0447}\u{0430}\u{043d}\u{0438}\u{0435}:\n\n\n\n\
         \u{0420}\u{0435}\u{0448}\u{0438}\u{043b}:\n\n{DECIDER}\n"
    );
    assert_eq!(record, want);
    assert!(
        question.contains(&format!("record ADR-0003 at {path}:\n")),
        "{question}"
    );
    assert_eq!(
        printed(&outcome).0,
        format!("applied {id} as {head} on t1: ADR-0003 {path}\n")
    );
    let stored = pair.proposal(&id).record.expect("stored");
    assert_eq!(
        (stored.id.as_str(), stored.title.as_str()),
        ("ADR-0003", keep)
    );
    // The record is a document of spec-b's corpus.
    assert_eq!(pair.node(&pair.linked, "ADR-0003").path, path);
}

// ------------------------------------------------------------------ AC-04

/// AC-04: a recorded root without `[decision_records]`: exit 2 naming the
/// table and `spec reject`; worktree, git index and `HEAD` unchanged;
/// `open`, one `apply_failed` (step 3), `next_record` unchanged; `reject
/// --reason` settles it. M: a built-in template.
#[test]
fn ac04_no_table_refuses_at_step_3_and_reject_still_settles() {
    let pair = Pair::new("da-ac04", "spec-a");
    pair.drop_records_table(&pair.linked);
    let id = ac01_item(&pair);
    let before = pair.state();
    let next = next_dec(&pair);
    let (outcome, questions) = pair.decide(&pair.main, &id, &option(1), None, true);
    let message = cannot(&outcome, "no table");
    assert_eq!(
        message,
        format!(
            "spec: `{id}` not applied (step 3): {}/specengine.toml has no `[decision_records]` \
             (`prefix`, `dir`, `template`) for `{id}`'s record: add it, or `spec reject {id} \
             --reason <answer>`; nothing changed",
            pair.linked.display()
        )
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "nothing written, the row as before");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    assert!(pair.proposal(&id).record.is_none());
    assert_eq!(
        pair.events_of(&id),
        [
            ("proposal.created".to_owned(), None),
            ("proposal.apply_failed".to_owned(), Some(3)),
        ]
    );
    assert_eq!(next_dec(&pair), next, "no ID issued");

    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Keep the spec.", true);
    let outcome = outcome.expect("reject");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Rejected);
    assert_eq!(stored.decision_note.as_deref(), Some("Keep the spec."));
    assert!(stored.record.is_none());
}

// ------------------------------------------------------------------ AC-05

/// AC-05: `DEC-0007` and `DEC-0023` in the corpus → `DEC-0024`; an item of
/// the main worktree, whose branch lacks `DEC-0024`, → `DEC-0025` (the
/// queue issued `DEC-0024`); a declined prompt issues nothing:
/// `next_record` unchanged, `record_id` NULL. M: issuing from the worktree
/// only.
#[test]
fn ac05_the_id_is_the_corpus_or_the_queue_whichever_is_higher() {
    let pair = Pair::new("da-ac05", "spec-a");
    let first = ac01_item(&pair);
    pair.decide_ok(&pair.main, &first, &option(1), None);
    assert!(pair.linked.join(dec_path("DEC-0024")).is_file());
    assert!(
        !pair.main.join(dec_path("DEC-0024")).exists(),
        "main lacks it"
    );

    let second = pair.raise_question(
        &pair.main,
        &["EDGE-STAM-ZERO"],
        "Is Exhausted applied at once?",
        "yes",
        "a delay rule",
    );
    let main_base = pair.rev(&pair.main, "HEAD");
    let (_, question) = pair.decide_ok(&pair.main, &second, &flags(None, None, None), None);
    assert!(
        question.contains(&format!("record DEC-0025 at {}:\n", dec_path("DEC-0025"))),
        "{question}"
    );
    let head = pair.rev(&pair.main, "main");
    assert_eq!(pair.rev(&pair.main, &format!("{head}^")), main_base);
    assert_eq!(
        name_status(&pair, &head),
        [format!("A\t{}", dec_path("DEC-0025"))]
    );
    assert_eq!(
        pair.proposal(&second).record.expect("stored").id,
        "DEC-0025"
    );

    let third = pair.raise_question(
        &pair.main,
        &["EDGE-STAM-ZERO"],
        "Does Exhausted end at full stamina?",
        "yes",
        "a recovery rule",
    );
    let next = next_dec(&pair);
    assert_eq!(next, "DEC-0026");
    let (outcome, questions) =
        pair.decide(&pair.main, &third, &flags(None, None, None), None, false);
    refused(&outcome, "declined");
    assert_eq!(questions.len(), 1);
    assert!(
        questions[0].contains("record DEC-0026 at "),
        "{questions:?}"
    );
    assert_eq!(next_dec(&pair), next, "nothing issued");
    assert!(pair.proposal(&third).record.is_none());
}

// ------------------------------------------------------------------ AC-06

/// The front-matter lines of `record` (between its two `---`).
fn front_matter(record: &str) -> Vec<&str> {
    let mut lines = record.lines();
    assert_eq!(lines.next(), Some("---"), "{record}");
    lines.take_while(|line| *line != "---").collect()
}

/// AC-06: free text never makes structure: a label `x\nstatus: rejected`
/// leaves `status: accepted` and one `title:` (`"x status: rejected"`);
/// an effect `## X {#RULE-STAM-REGEN}` (a second definition) exits 1
/// naming `options[1].effect`, no prompt, nothing written; a label
/// `{{id}}` stays that text; U+202E in the summary exits 1 naming
/// `summary`, no prompt, no event. M: raw substitution; a re-scan.
#[test]
fn ac06_free_text_is_one_pass_and_never_structure() {
    let pair = Pair::new("da-ac06", "spec-a");
    let injected = pair.raise_discrepancy(
        &pair.linked,
        discrepancy(
            &["RULE-STAM-REGEN"],
            "A label tries a key",
            &[("Keep", "e0", "p0"), ("x\nstatus: rejected", "e1", "p1")],
        ),
    );
    pair.decide_ok(&pair.main, &injected, &option(1), None);
    let record = read_text(&pair.linked, &dec_path("DEC-0024"));
    let front = front_matter(&record);
    assert_eq!(
        front
            .iter()
            .filter(|line| line.starts_with("status:"))
            .collect::<Vec<_>>(),
        [&"status: accepted"],
        "{record}"
    );
    assert_eq!(
        front
            .iter()
            .filter(|line| line.starts_with("title:"))
            .collect::<Vec<_>>(),
        [&"title: \"x status: rejected\""],
        "{record}"
    );
    assert!(record.contains("\n# x status: rejected\n"), "{record}");
    assert!(
        record.contains("\nChoice:\n\nx\nstatus: rejected\n"),
        "{record}"
    );

    let defining = pair.raise_discrepancy(
        &pair.linked,
        discrepancy(
            &["RULE-STAM-REGEN"],
            "An effect tries a heading",
            &[
                ("Keep", "e0", "p0"),
                ("Change", "## X {#RULE-STAM-REGEN}", "p1"),
            ],
        ),
    );
    let before = pair.state();
    let next = next_dec(&pair);
    let (outcome, questions) = pair.decide(&pair.main, &defining, &option(1), None, true);
    let reason = refused(&outcome, "a defining effect");
    assert!(
        reason.starts_with(&format!(
            "`{defining}` not applied (step 6): options[1].effect: "
        )),
        "{reason}"
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.state(), before);
    assert_eq!(next_dec(&pair), next);
    assert_eq!(
        pair.events_of(&defining).last(),
        Some(&("proposal.apply_failed".to_owned(), Some(6)))
    );

    let literal = pair.raise_discrepancy(
        &pair.linked,
        discrepancy(
            &["RULE-STAM-REGEN"],
            "A label holds a slot",
            &[
                ("{{id}}", "{{date}} {{canon}}", "{{proposal}}"),
                ("Other", "e1", "p1"),
            ],
        ),
    );
    pair.decide_ok(&pair.main, &literal, &option(0), None);
    let record = read_text(&pair.linked, &dec_path("DEC-0025"));
    assert!(
        front_matter(&record).contains(&"title: \"{{id}}\""),
        "{record}"
    );
    assert!(record.contains("\n# {{id}}\n"), "{record}");
    assert!(
        record.contains("\nEffect:\n\n{{date}} {{canon}}\n\nCost:\n\n{{proposal}}\n"),
        "{record}"
    );
    assert_eq!(
        pair.proposal(&literal).record.expect("stored").title,
        "{{id}}"
    );

    let marked = pair.raise_discrepancy(
        &pair.linked,
        discrepancy(
            &["RULE-STAM-REGEN"],
            "A summary \u{202e}reversed",
            &[("Keep", "e0", "p0"), ("Change", "e1", "p1")],
        ),
    );
    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.decide(&pair.main, &marked, &option(1), None, true);
    let reason = refused(&outcome, "a refused character");
    assert_eq!(
        reason,
        "summary: holds U+202E: a decision record never carries it; nothing changed"
    );
    assert!(questions.is_empty());
    assert_eq!(pair.state(), before);
    assert_eq!(pair.events().len(), events, "no event");
}

// ------------------------------------------------------------------ AC-07

/// AC-07: the binary with stdin a pipe exits 2 before reading anything;
/// consent `n` (the library, and the binary on a terminal): exit 1, the
/// path absent, `git ls-files --stage` and `git status --porcelain` as
/// before, `record_id` NULL, no event. M: the ID before the prompt.
#[test]
fn ac07_no_consent_writes_and_issues_nothing() {
    let pair = Pair::new("da-ac07", "spec-a");
    let id = ac01_item(&pair);
    let staged = pair.git_text(&pair.linked, &["ls-files", "--stage"]);
    let status = pair.porcelain(&pair.linked);
    let before = pair.state();
    let events = pair.events().len();

    let run = pair.spec_piped(&pair.main, &["approve", &id, "--option", "1"], b"y\n");
    assert_eq!(run.code, 2, "{}", run.show());
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .contains("asks the owner for consent on a terminal, and stdin is not one"),
        "{}",
        run.show()
    );

    let (outcome, questions) = pair.decide(&pair.main, &id, &option(1), None, false);
    let reason = refused(&outcome, "declined");
    assert_eq!(
        reason,
        format!("`{id}` not applied: the answer was not `y`; nothing changed")
    );
    assert_eq!(questions.len(), 1);

    let run = pair.spec_pty(&pair.main, &["approve", &id, "--option", "1"], Some("n"));
    assert_eq!(run.code, 1, "{}", run.output);
    assert!(run.asked, "{}", run.output);

    assert!(!pair.linked.join(dec_path("DEC-0024")).exists());
    assert_eq!(
        pair.git_text(&pair.linked, &["ls-files", "--stage"]),
        staged
    );
    assert_eq!(pair.porcelain(&pair.linked), status);
    assert_eq!(pair.state(), before);
    assert!(pair.proposal(&id).record.is_none(), "record_id NULL");
    assert_eq!(
        pair.sql("select count(*) from proposals where record_id is not null")
            .trim(),
        "0"
    );
    assert_eq!(pair.events().len(), events, "no event");
    assert_eq!(next_dec(&pair), "DEC-0024");
}

// ------------------------------------------------------------------ AC-08

/// Installs `body` as the repository's `pre-commit` hook (the common
/// dir's, shared by the linked worktree); `None` removes it.
fn hook(pair: &Pair, body: Option<&str>) {
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).expect("hooks");
    let path = hooks.join("pre-commit");
    match body {
        Some(body) => {
            fs::write(&path, body).expect("hook");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
        }
        None => {
            let _ = fs::remove_file(&path);
        }
    }
}

/// AC-08: `dir` a missing `docs/records/DEC/new`, a `pre-commit` hook
/// exiting 1: exit 1 (step 9); the file and `new/` gone, `git ls-files
/// --stage -- <path>` empty, `git status` as before; `open`, `record_id`
/// kept, one `apply_failed` (step 9); without the hook the same ID
/// applies. M: the intent-to-add entry left.
#[test]
fn ac08_a_failed_commit_removes_the_record_its_entry_and_its_directory() {
    let pair = Pair::new("da-ac08", "spec-a");
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_records(
            &config,
            "DEC",
            "docs/records/DEC/new",
            "templates/decision.md",
        ),
    );
    pair.commit_all(&pair.linked, "Records in new/.");
    let id = ac01_item(&pair);
    hook(
        &pair,
        Some("#!/bin/sh\necho refused by the hook >&2\nexit 1\n"),
    );
    let path = "docs/records/DEC/new/DEC-0024.md";
    let staged = pair.git_text(&pair.linked, &["ls-files", "--stage"]);
    let status = pair.porcelain(&pair.linked);
    let head = pair.rev(&pair.linked, "HEAD");

    let (outcome, questions) = pair.decide(&pair.main, &id, &option(1), None, true);
    let reason = refused(&outcome, "a failing hook");
    assert!(
        reason.starts_with(&format!(
            "`{id}` not applied (step 9): the commit of `{path}` failed, the record removed: "
        )),
        "{reason}"
    );
    assert!(
        reason.contains("refused by the hook") || reason.contains("pre-commit"),
        "{reason}"
    );
    assert_eq!(questions.len(), 1);
    assert!(!pair.linked.join(path).exists(), "the file removed");
    assert!(
        !pair.linked.join("docs/records/DEC/new").exists(),
        "new/ removed"
    );
    assert_eq!(
        pair.git_text(&pair.linked, &["ls-files", "--stage", "--", path]),
        "",
        "no index entry left"
    );
    assert_eq!(
        pair.git_text(&pair.linked, &["ls-files", "--stage"]),
        staged
    );
    assert_eq!(pair.porcelain(&pair.linked), status);
    assert_eq!(pair.rev(&pair.linked, "HEAD"), head, "no commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.record.expect("record_id kept").id, "DEC-0024");
    let failed: Vec<_> = pair
        .events_of(&id)
        .into_iter()
        .filter(|(kind, _)| kind == "proposal.apply_failed")
        .collect();
    assert_eq!(failed, [("proposal.apply_failed".to_owned(), Some(9))]);

    hook(&pair, None);
    let (_, question) = pair.decide_ok(&pair.main, &id, &option(1), None);
    assert!(
        question.contains(&format!("record DEC-0024 at {path}:\n")),
        "{question}"
    );
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
}

// ------------------------------------------------------------------ AC-09

/// AC-09: the consent callback writes X at the record's path, then says
/// yes: exit 1 (step 8), X intact, no commit, no temporary file left; the
/// store's create-new on an existing path fails, its bytes intact. M:
/// rename over the path.
#[test]
fn ac09_a_file_appearing_meanwhile_is_never_replaced() {
    let pair = Pair::new("da-ac09", "spec-a");
    let id = ac01_item(&pair);
    let path = dec_path("DEC-0024");
    let dir = pair.linked.join("docs/records/DEC");
    let listed = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("the records directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    };
    let mut names = listed(&dir);
    let head = pair.rev(&pair.linked, "HEAD");
    let target = pair.linked.join(&path);
    let mut consent = |_: &str| {
        fs::write(&target, b"X").expect("X written meanwhile");
        true
    };
    let outcome = pair.decide_with(&pair.main, &id, &option(1), None, CLOCK, &mut consent);
    let reason = refused(&outcome, "a file meanwhile");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 8): `{path}` exists: a decision record never replaces a \
             file; nothing written"
        )
    );
    assert_eq!(fs::read(&target).expect("X"), b"X", "X intact");
    names.push("DEC-0024.md".to_owned());
    names.sort();
    assert_eq!(listed(&dir), names, "no temporary left");
    assert_eq!(pair.rev(&pair.linked, "HEAD"), head, "no commit");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);

    // The store's create-new.
    let created =
        create_file(&pair.linked, "docs/records/DEC/fresh.md", b"one").expect("a new file");
    drop(created);
    assert!(create_file(&pair.linked, "docs/records/DEC/fresh.md", b"two").is_err());
    assert_eq!(
        fs::read(pair.linked.join("docs/records/DEC/fresh.md")).unwrap(),
        b"one"
    );
    assert!(create_file(&pair.linked, &path, b"two").is_err());
    assert_eq!(fs::read(&target).unwrap(), b"X");
}

// ------------------------------------------------------------------ AC-10

/// The message `spec approve` commits `id`'s record with.
fn record_message(pair: &Pair, id: &str, record_id: &str, title: &str) -> String {
    format!(
        "spec: apply {id}\n\n{record_id}: {title}\n\nProposal: {id}\nDecided-by: {DECIDER}\n\
         Proposed-by: agent role=developer model=claude-opus-5-5 run=unknown\nBase-commit: {}\n",
        pair.proposal(id).place.base_commit
    )
}

/// `id` stopped after step 7: a failed commit (a `pre-commit` hook
/// exiting 1) reopens it with its record, then the row is set back to
/// `approved` with its decider, as a run killed between steps 7 and 9
/// leaves it; its stored record.
fn stopped_after_step_7(pair: &Pair, id: &str) -> specengine_store::DecisionRecord {
    hook(pair, Some("#!/bin/sh\nexit 1\n"));
    let (outcome, _) = pair.decide(&pair.main, id, &option(1), None, true);
    refused(&outcome, "the hook");
    hook(pair, None);
    pair.sql(&format!(
        "update proposals set status = 'approved', decided_by = '{DECIDER}', decided_at = \
         '{CLOCK}' where id = '{id}'"
    ));
    let stored = pair.proposal(id);
    assert_eq!(stored.status, ProposalStatus::Approved);
    stored.record.expect("its record")
}

/// AC-10: `approved` at step 7, its commit made by hand from
/// `record_text`: approve (a later clock) → `applied` by that commit, no
/// new commit, no prompt; any decision flag on it exits 2; a hand commit
/// with another `date:` does not carry the record: approve refused (step
/// 4) naming it. M: re-rendering.
#[test]
fn ac10_an_approved_record_is_completed_by_its_own_commit_only() {
    let pair = Pair::new("da-ac10", "spec-a");
    let id = ac01_item(&pair);
    let record = stopped_after_step_7(&pair, &id);
    assert_eq!(record.id, "DEC-0024");

    let (outcome, questions) = pair.decide(&pair.main, &id, &option(1), None, true);
    let message = cannot(&outcome, "a flag on an approved record");
    assert_eq!(
        message,
        format!(
            "spec: `{id}` was approved with its record `DEC-0024` (an apply stopped after step \
             7): `spec approve {id}` writes that record and takes only `--note`; nothing changed"
        )
    );
    assert!(questions.is_empty());

    write(&pair.linked, &record.path, &record.text);
    pair.git.git(&pair.linked, &["add", "--", &record.path]);
    pair.git.git_stdin(
        &pair.linked,
        &["commit", "-q", "--no-verify", "-F", "-"],
        record_message(&pair, &id, &record.id, &record.title).as_bytes(),
    );
    let by_hand = pair.rev(&pair.linked, "HEAD");
    let events = pair.events().len();
    let (outcome, questions) = pair.decide_at(
        &pair.main,
        &id,
        &flags(None, None, None),
        None,
        true,
        "2026-10-09T08:00:00Z",
    );
    let outcome = outcome.expect("completed");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(pair.rev(&pair.linked, "t1"), by_hand, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(by_hand.as_str()));
    assert_eq!(pair.events().len(), events + 1);

    // Another date: not the record.
    let other = Pair::new("da-ac10-date", "spec-a");
    let id = ac01_item(&other);
    let record = stopped_after_step_7(&other, &id);
    let changed = record
        .text
        .replacen(&format!("date: {DAY}"), "date: 2026-10-09", 1);
    assert_ne!(changed, record.text);
    write(&other.linked, &record.path, &changed);
    other.git.git(&other.linked, &["add", "--", &record.path]);
    other.git.git_stdin(
        &other.linked,
        &["commit", "-q", "--no-verify", "-F", "-"],
        record_message(&other, &id, &record.id, &record.title).as_bytes(),
    );
    let tip = other.rev(&other.linked, "HEAD");
    let before = other.state();
    let (outcome, questions) = other.decide_at(
        &other.main,
        &id,
        &flags(None, None, None),
        None,
        true,
        "2026-10-09T08:00:00Z",
    );
    let reason = refused(&outcome, "another date");
    assert!(
        reason.starts_with(&format!("`{id}` not applied (step 4): the commit {tip} ")),
        "{reason}"
    );
    assert!(reason.contains("does not carry the record"), "{reason}");
    assert!(questions.is_empty());
    assert_eq!(other.state(), before, "nothing written, the row as before");
    assert_eq!(other.proposal(&id).status, ProposalStatus::Approved);
    // Its `Proposal:` commit is on the branch: never rejected either.
    let (outcome, questions) = other.reject_answer(&other.main, &id, "No.", true);
    let reason = refused(&outcome, "reject over its commit");
    assert!(reason.contains(&tip), "{reason}");
    assert!(questions.is_empty());
    assert_eq!(other.state(), before);
}

/// "An `approved` deciding row (stopped after step 7) … writes its
/// `record_text`": its commit not made, approve (no flag, a later clock)
/// writes the stored bytes, the date it was rendered with. M:
/// re-rendering.
#[test]
fn an_approved_record_is_written_as_stored_never_rendered_again() {
    let pair = Pair::new("da-kept", "spec-a");
    let id = ac01_item(&pair);
    let record = stopped_after_step_7(&pair, &id);
    let (_, question) = {
        let (outcome, questions) = pair.decide_at(
            &pair.main,
            &id,
            &flags(None, None, None),
            None,
            true,
            "2026-10-09T08:00:00Z",
        );
        let outcome = outcome.expect("applied");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        (outcome, questions.into_iter().next().expect("one question"))
    };
    assert!(question.contains("record DEC-0024 at "), "{question}");
    assert_eq!(read_text(&pair.linked, &record.path), record.text);
    assert!(record.text.contains(&format!("\ndate: {DAY}\n")));
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{}", record.path)]);
}

// ------------------------------------------------------------------ AC-11

/// AC-11: after AC-01, the same summary on `RULE-STAM-REGEN` from the
/// recorded root: `created: false`, the hits the corpus record `DEC-0024`
/// and the queue item `PR-0001` (`applied`, `path` the record, `answer`
/// the title, `record` `DEC-0024`); `counts()` unchanged; the brief
/// (`get_proposal`'s twin) carries the record's keys, `record_text` null.
/// M: the answer from `decision_note`.
#[test]
fn ac11_a_decided_item_answers_the_same_question_again() {
    let pair = Pair::new("da-ac11", "spec-a");
    let id = ac01_item(&pair);
    pair.decide_ok(
        &pair.main,
        &id,
        &option(1),
        Some("Sprinting regenerates now."),
    );
    let path = dec_path("DEC-0024");
    let counts = pair.queue().counts().expect("counts");
    let outcome = specengine_cli::propose_discrepancy(
        &pair.env(&pair.linked),
        &Globals::default(),
        &specengine_cli::DiscrepancyRequest {
            input: discrepancy(&["RULE-STAM-REGEN"], SUMMARY, &THREE),
            author_role: Some("developer".to_owned()),
            author_model: None,
            run: None,
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.linked),
        },
    )
    .expect("asked again");
    let json = serde_json::to_value(&outcome.document).expect("JSON");
    assert_eq!(json["created"], json!(false), "{json}");
    assert!(json["id"].is_null());
    let hits = json["hits"].as_array().expect("hits");
    let corpus = hits
        .iter()
        .find(|hit| hit["id"] == json!("DEC-0024"))
        .unwrap_or_else(|| panic!("the record hits: {json}"));
    assert_eq!(
        corpus,
        &json!({"id": "DEC-0024", "source": "corpus", "status": "accepted", "path": path,
            "answer": "Change the spec", "record": "DEC-0024"})
    );
    let queued = hits
        .iter()
        .find(|hit| hit["id"] == json!(id))
        .unwrap_or_else(|| panic!("the item hits: {json}"));
    assert_eq!(
        queued,
        &json!({"id": id, "source": "queue", "status": "applied", "path": path,
            "answer": "Change the spec", "record": "DEC-0024"})
    );
    assert_eq!(
        pair.queue().counts().expect("counts"),
        counts,
        "nothing stored"
    );

    let brief = review_brief(
        &pair.env(&pair.linked),
        &Globals::default(),
        &ReviewRequest {
            id: id.clone(),
            git: pair.git_env(&pair.linked),
        },
    )
    .expect("review --brief");
    let brief: Value = json_of(&printed(&brief).1);
    assert_eq!(brief["record_id"], json!("DEC-0024"));
    assert_eq!(brief["record_path"], json!(path));
    assert_eq!(brief["record_title"], json!("Change the spec"));
    assert!(brief["record_text"].is_null(), "{brief}");
    assert_eq!(brief["choice"], json!({"option": 1}));
    assert_eq!(brief["decision_note"], json!("Sprinting regenerates now."));
}

// ------------------------------------------------------------------ AC-12

/// The `Env` of `cwd` with `home` as `HOME`.
fn env_at(home: &Path, cwd: &Path) -> specengine_cli::Env {
    specengine_cli::Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

/// `spec export state --out <out>` of the queue under `home`, from the
/// main worktree; the dump's bytes.
fn export(pair: &Pair, home: &Path, out: &Path) -> Vec<u8> {
    specengine_cli::export_state(
        &env_at(home, &pair.main),
        &Globals::default(),
        &specengine_cli::ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
    fs::read(out).expect("the dump")
}

/// `spec import-state <file>` into the queue under `home`, consent yes.
fn import(
    pair: &Pair,
    home: &Path,
    file: &Path,
) -> Result<specengine_cli::ImportStateOutcome, specengine_cli::CliError> {
    let mut consent = |_: &str| true;
    specengine_cli::import_state(
        &env_at(home, &pair.main),
        &Globals::default(),
        &specengine_cli::ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    )
}

/// The `dump()` of the queue under `home`.
fn dump_at(pair: &Pair, home: &Path) -> String {
    specengine_store::SqliteQueue::open(
        common::data_dir(home).join(format!("{}.db", pair.slug)),
        &pair.slug,
    )
    .expect("open")
    .dump()
    .expect("dump")
}

/// The dump's lines.
fn dump_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8(bytes.to_vec())
        .expect("UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The `proposals` object of a dump line holds exactly `columns`, each
/// key's text after the one before it (the dump's order).
fn assert_columns(line: &str, columns: &[&str]) {
    let value: Value = serde_json::from_str(line).expect("a JSON row");
    let row = value["proposals"].as_object().expect("a proposals row");
    assert_eq!(row.len(), columns.len(), "{line}");
    let mut last = 0;
    for column in columns {
        let key = format!("\"{column}\":");
        let at = line[last..]
            .find(&key)
            .unwrap_or_else(|| panic!("{key} after byte {last} in {line}"));
        last += at + key.len();
    }
}

/// AC-12, the backup half (the tools' half: `specengine-mcp`'s
/// `mcp_decision.rs`): a decided item exported (`queue_schema` 3, each row
/// the 40 columns in table order, the record's five as stored) and
/// imported fresh: `dump()` equal, the re-export byte-identical. M: a
/// column missing.
#[test]
fn ac12_a_decided_item_round_trips_through_a_backup() {
    let pair = Pair::new("da-ac12", "spec-a");
    let id = ac01_item(&pair);
    pair.decide_ok(&pair.main, &id, &option(1), Some("A note."));
    let rejected = pair.raise_question(&pair.linked, &["EDGE-STAM-ZERO"], "Stop?", "yes", "a rule");
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
    );
    let dumps = pair.scratch.dir("dumps");
    let bytes = export(&pair, &pair.home, &dumps.join("q.jsonl"));
    let lines = dump_lines(&bytes);
    assert!(
        lines[0].starts_with(
            "{\"format\":1,\"queue_schema\":3,\"project\":\"lantern-keep\",\"proposals\":3,"
        ),
        "{}",
        lines[0]
    );
    let columns = specengine_store::PROPOSAL_COLUMNS;
    assert_eq!(columns.len(), 40);
    for line in &lines[1..4] {
        assert_columns(line, &columns);
    }
    let decided: Value = serde_json::from_str(&lines[1]).unwrap();
    let row = &decided["proposals"];
    assert_eq!(row["record_id"], json!("DEC-0024"));
    assert_eq!(row["record_path"], json!(dec_path("DEC-0024")));
    assert_eq!(row["record_title"], json!("Change the spec"));
    assert_eq!(
        row["record_text"],
        json!(read_text(&pair.linked, &dec_path("DEC-0024")))
    );
    assert_eq!(row["choice"], json!("{\"option\":1}"));
    for line in &lines[2..4] {
        let row: Value = serde_json::from_str(line).unwrap();
        for column in &columns[35..] {
            assert!(row["proposals"][column].is_null(), "{column}: {line}");
        }
    }

    let fresh = pair.scratch.home("fresh");
    let outcome = import(&pair, &fresh, &dumps.join("q.jsonl")).expect("import");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        dump_at(&pair, &fresh),
        dump_at(&pair, &pair.home),
        "dump() equal"
    );
    assert_eq!(export(&pair, &fresh, &dumps.join("again.jsonl")), bytes);
    let restored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .unwrap()
    .get(&id)
    .unwrap()
    .unwrap();
    assert_eq!(restored, pair.proposal(&id));
}

/// AC-12: a schema-2 dump (its 35 columns) restores, the five record
/// columns `NULL`, and re-exports byte-identical as schema 3 (a schema-1
/// dump: `intake_state.rs`, re-exported as 3 too); a schema-2 row carrying
/// the record columns is refused naming its line; a database still at
/// schema 2 exports as 3 unmigrated, and the first queue command steps it
/// to `user_version` 3, its rows kept. M: a column missing.
#[test]
fn ac12_a_schema_2_dump_restores_and_a_version_2_database_steps_to_3() {
    let pair = Pair::new("da-ac12-old", "spec-a");
    pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
    );
    let rejected = pair.raise_question(&pair.linked, &["EDGE-STAM-ZERO"], "Stop?", "yes", "a rule");
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    let dumps = pair.scratch.dir("dumps");
    let v3 = export(&pair, &pair.home, &dumps.join("v3.jsonl"));
    let v3_lines = dump_lines(&v3);
    let before = dump_at(&pair, &pair.home);
    let record_nulls = ",\"record_id\":null,\"record_path\":null,\"record_title\":null,\
\"record_text\":null,\"choice\":null}}";
    let mut v2_lines = v3_lines.clone();
    v2_lines[0] = v2_lines[0].replacen("\"queue_schema\":3,", "\"queue_schema\":2,", 1);
    for line in &mut v2_lines[1..3] {
        assert!(line.ends_with(record_nulls), "{line}");
        *line = format!("{}}}}}", &line[..line.len() - record_nulls.len()]);
        assert_columns(line, &specengine_store::PROPOSAL_COLUMNS[..35]);
    }
    let file = dumps.join("v2.jsonl");
    fs::write(&file, format!("{}\n", v2_lines.join("\n"))).unwrap();
    let home = pair.scratch.home("v2");
    let outcome = import(&pair, &home, &file).expect("import");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(dump_at(&pair, &home), before, "the five NULL");
    assert_eq!(
        export(&pair, &home, &dumps.join("v2-again.jsonl")),
        v3,
        "re-exported as queue_schema 3"
    );
    // A schema-2 row with the record columns.
    let mut mixed = v2_lines.clone();
    mixed[1] = v3_lines[1].clone();
    let file = dumps.join("v2-with-40.jsonl");
    fs::write(&file, format!("{}\n", mixed.join("\n"))).unwrap();
    let home = pair.scratch.home("mixed");
    let message = cannot(&import(&pair, &home, &file), "v2 with 40");
    assert!(
        message.contains(&format!("{}:2: ", file.display())),
        "{message}"
    );

    // A version-2 database.
    let drops: Vec<String> = specengine_store::PROPOSAL_COLUMNS[35..]
        .iter()
        .map(|column| format!("ALTER TABLE proposals DROP COLUMN {column};"))
        .collect();
    pair.sql(&format!("{} PRAGMA user_version = 2;", drops.join(" ")));
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "2");
    assert_eq!(
        export(&pair, &pair.home, &dumps.join("from-v2.jsonl")),
        v3,
        "a version-2 DB exports as queue_schema 3"
    );
    assert_eq!(
        pair.sql("PRAGMA user_version;").trim(),
        "2",
        "export steps nothing"
    );
    pair.inbox(&pair.main, true).expect("inbox");
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "3");
    assert_eq!(
        pair.sql("SELECT name FROM pragma_table_info('proposals');")
            .lines()
            .collect::<Vec<_>>(),
        specengine_store::PROPOSAL_COLUMNS,
        "step 3 appends the five in order"
    );
    assert_eq!(dump_at(&pair, &pair.home), before, "rows kept");
}

// ------------------------------------------------------------------ AC-13

/// AC-13: the same item, choice, note, clock and identity in two fresh
/// repositories: byte-identical records, commit messages, commits and
/// stored texts; another clock changes only the record's `date:` (the run's
/// clock, never the wall's). M: the wall clock.
#[test]
fn ac13_the_same_decision_renders_the_same_bytes() {
    let run = |label: &str, now: &str| {
        let pair = Pair::new(label, "spec-a");
        let id = ac01_item(&pair);
        let (outcome, _) = pair.decide_at(
            &pair.main,
            &id,
            &option(1),
            Some("The owner's note."),
            true,
            now,
        );
        assert_eq!(outcome.expect("applied").exit(), Exit::Answered);
        let head = pair.rev(&pair.linked, "t1");
        (
            read(&pair.linked, &dec_path("DEC-0024")),
            pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]),
            head,
            pair.proposal(&id).record.expect("stored").text,
        )
    };
    let first = run("da-ac13-a", CLOCK);
    let second = run("da-ac13-b", CLOCK);
    assert_eq!(first, second);
    assert!(
        String::from_utf8_lossy(&first.0).contains(&format!("\ndate: {DAY}\n")),
        "the run's date"
    );
    let later = run("da-ac13-c", "2001-02-03T04:05:06Z");
    assert_eq!(
        String::from_utf8(later.0).unwrap(),
        String::from_utf8(first.0.clone()).unwrap().replacen(
            &format!("\ndate: {DAY}\n"),
            "\ndate: 2001-02-03\n",
            1
        )
    );
    assert_eq!(later.1, first.1, "the message names no date");
}

/// The bytes of `root/relative`.
fn read(root: &Path, relative: &str) -> Vec<u8> {
    common::read(root, relative)
}

// ------------------------------------------------------------------ AC-14

/// AC-14: exit 2 for `--option` on a question or an update, `--answer` on
/// a discrepancy, both flags, a discrepancy without `--option`, a question
/// on `docs/features/stamina-tuning.md` without `--canon`, `--canon "a
/// b"`, `--canon` on an update, a blank `--answer`; exit 1 for `--option 7`
/// of 3 (naming `0-2`), an `--answer` over 2 048 bytes, a `--canon` over
/// 512; never a prompt, never an event, nothing written. M: an out-of-range
/// option accepted.
#[test]
fn ac14_the_flags_are_checked_before_anything() {
    let pair = Pair::new("da-ac14", "spec-a");
    let update = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
    );
    let question = pair.raise_question(&pair.linked, &["EDGE-STAM-ZERO"], "Stop?", "yes", "a rule");
    let reported = ac01_item(&pair);
    let by_path = pair.raise_question(
        &pair.linked,
        &["docs/features/stamina-tuning.md"],
        "Which tuning applies to regeneration?",
        "The rest tuning",
        "a table",
    );
    let before = pair.state();
    let events = pair.events().len();
    let long_answer = "a".repeat(2049);
    let long_canon = format!("MEC-STAMINA#{}", "a".repeat(501));
    assert_eq!(long_canon.len(), 513);
    let cannot_cases = [
        (
            &question,
            flags(Some(0), None, None),
            format!(
                "spec: `--option` names a discrepancy's option, and `{question}` is a question: its \
                 working answer, or `--answer T`, answers it; nothing changed"
            ),
        ),
        (
            &question,
            flags(Some(0), Some("yes"), None),
            format!(
                "spec: `--option` names a discrepancy's option, and `{question}` is a question: its \
                 working answer, or `--answer T`, answers it; nothing changed"
            ),
        ),
        (
            &update,
            flags(Some(0), None, None),
            format!(
                "spec: `--option` decides a question or a discrepancy; `{update}` is an update: \
                 `spec approve {update}` applies it as proposed; nothing changed"
            ),
        ),
        (
            &update,
            flags(None, None, Some("RULE-STAM-REGEN")),
            format!(
                "spec: `--canon` decides a question or a discrepancy; `{update}` is an update: \
                 `spec approve {update}` applies it as proposed; nothing changed"
            ),
        ),
        (
            &reported,
            flags(None, Some("yes"), None),
            format!(
                "spec: `--answer` answers a question, and `{reported}` is a discrepancy: name the \
                 owner's choice with `--option N` (0-2); nothing changed"
            ),
        ),
        (
            &reported,
            flags(Some(1), Some("yes"), None),
            format!(
                "spec: `--answer` answers a question, and `{reported}` is a discrepancy: name the \
                 owner's choice with `--option N` (0-2); nothing changed"
            ),
        ),
        (
            &reported,
            flags(None, None, None),
            format!(
                "spec: `{reported}` is a discrepancy: name the owner's choice with `--option N` \
                 (0-2); nothing changed"
            ),
        ),
        (
            &by_path,
            flags(None, None, None),
            format!(
                "spec: `{by_path}` names no ID: name the section its record governs with \
                 `--canon REF`; nothing changed"
            ),
        ),
        (
            &question,
            flags(None, None, Some("a b")),
            "spec: `--canon a b` is no `ID`, `ID#SECTION` or `path#anchor`; nothing changed"
                .to_owned(),
        ),
        (
            &question,
            flags(None, Some("  \n "), None),
            "spec: `--answer` is blank: give the owner's answer, or leave it out to take the \
             working answer; nothing changed"
                .to_owned(),
        ),
    ];
    for (id, given, want) in cannot_cases {
        let (outcome, questions) = pair.decide(&pair.main, id, &given, None, true);
        assert_eq!(
            cannot(&outcome, &format!("{id} {given:?}")),
            want,
            "{given:?}"
        );
        assert!(questions.is_empty(), "{given:?}");
    }
    let refused_cases = [
        (
            &reported,
            flags(Some(7), None, None),
            format!("--option 7: `{reported}` has options 0-2; nothing changed"),
        ),
        (
            &reported,
            flags(Some(3), None, None),
            format!("--option 3: `{reported}` has options 0-2; nothing changed"),
        ),
        (
            &question,
            flags(None, Some(long_answer.as_str()), None),
            "--answer: 2049 bytes; at most 2048; nothing changed".to_owned(),
        ),
        (
            &question,
            flags(None, None, Some(long_canon.as_str())),
            "--canon: 513 bytes; at most 512; nothing changed".to_owned(),
        ),
    ];
    for (id, given, want) in refused_cases {
        let (outcome, questions) = pair.decide(&pair.main, id, &given, None, true);
        assert_eq!(refused(&outcome, id), want);
        assert!(questions.is_empty());
    }
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");

    // The binary's flags reach the same checks (a terminal: the check
    // that comes first in `main`).
    let run = pair.spec_pty(&pair.main, &["approve", &reported, "--option", "7"], None);
    assert_eq!(run.code, 1, "{}", run.output);
    assert!(!run.asked, "{}", run.output);
    assert!(run.output.contains("has options 0-2"), "{}", run.output);
    let run = pair.spec_pty(&pair.main, &["approve", &reported, "--answer", "yes"], None);
    assert_eq!(run.code, 2, "{}", run.output);
    let run = pair.spec_pty(&pair.main, &["approve", &question, "--canon", "a b"], None);
    assert_eq!(run.code, 2, "{}", run.output);
    assert_eq!(pair.state(), before);
    assert_eq!(pair.events().len(), events);
}

// ------------------------------------------------------------------ AC-15

/// AC-15: the question on `docs/features/stamina-tuning.md` (a path, no
/// ID), `--canon docs/spec/movement/stamina.md#regeneration`: that
/// `canon:`, `targets` `[]`, no `canon-*` finding introduced; the bare
/// path in a fresh copy: `canon-form` shown above the prompt, still
/// written; a question on `Q-031`, `--canon MEC-STAMINA#RULE-STAM-REGEN`:
/// that `canon:`, its targets `[Q-031]`. M: a path by default.
#[test]
fn ac15_canon_names_the_section_the_record_governs() {
    let tuning = "docs/features/stamina-tuning.md";
    let ask = |pair: &Pair| {
        pair.raise_question(
            &pair.linked,
            &[tuning],
            "Which tuning applies to regeneration?",
            "The rest tuning",
            "a table",
        )
    };
    let pair = Pair::new("da-ac15", "spec-a");
    let id = ask(&pair);
    let anchored = "docs/spec/movement/stamina.md#regeneration";
    let (_, question) = pair.decide_ok(&pair.main, &id, &flags(None, None, Some(anchored)), None);
    assert!(
        question.starts_with("introduced: 0\nrecord DEC-0024 at "),
        "{question}"
    );
    let record = read_text(&pair.linked, &dec_path("DEC-0024"));
    let front = front_matter(&record);
    assert!(
        front.contains(&format!("canon: {anchored}").as_str()),
        "{record}"
    );
    assert!(front.contains(&"  answers: []"), "{record}");
    assert!(!question.contains("canon-"), "{question}");

    let id = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Does the rule govern the delay?",
        "yes",
        "a rule",
    );
    pair.decide_ok(
        &pair.main,
        &id,
        &flags(None, None, Some("MEC-STAMINA#RULE-STAM-REGEN")),
        None,
    );
    let record = read_text(&pair.linked, &dec_path("DEC-0025"));
    let front = front_matter(&record);
    assert!(
        front.contains(&"canon: MEC-STAMINA#RULE-STAM-REGEN"),
        "{record}"
    );
    assert!(front.contains(&"  answers: [Q-031]"), "{record}");

    let fresh = Pair::new("da-ac15-bare", "spec-a");
    let id = ask(&fresh);
    let bare = "docs/spec/movement/stamina.md";
    let (_, question) = fresh.decide_ok(&fresh.main, &id, &flags(None, None, Some(bare)), None);
    let introduced: Vec<&str> = question
        .lines()
        .take_while(|line| !line.starts_with("record "))
        .collect();
    assert!(
        introduced.iter().any(|line| line.contains("canon-form")),
        "canon-form shown: {question}"
    );
    let record = read_text(&fresh.linked, &dec_path("DEC-0024"));
    assert!(
        front_matter(&record).contains(&format!("canon: {bare}").as_str()),
        "{record}"
    );
    assert_eq!(
        fresh.proposal(&id).status,
        ProposalStatus::Applied,
        "still written"
    );
}

// ------------------------------------------------------------ Known limits

/// "Known limits": a queue restored from before step 7 while the record's
/// commit is on its branch: the item `open` without a record; neither
/// approve (step 4: its `Proposal:` commit does not complete it) nor
/// reject takes it; nothing written.
#[test]
fn a_queue_restored_from_before_step_7_takes_neither_approve_nor_reject() {
    let pair = Pair::new("da-limits", "spec-a");
    let id = ac01_item(&pair);
    let dumps = pair.scratch.dir("dumps");
    export(&pair, &pair.home, &dumps.join("before.jsonl"));
    pair.decide_ok(&pair.main, &id, &option(1), None);
    let head = pair.rev(&pair.linked, "t1");

    let restored = pair.scratch.home("restored");
    let outcome = import(&pair, &restored, &dumps.join("before.jsonl")).expect("import");
    assert_eq!(outcome.exit(), Exit::Answered);
    let queue_before = dump_at(&pair, &restored);
    let files = pair.state();
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::approve_with(
        &env_at(&restored, &pair.main),
        &Globals::default(),
        &specengine_cli::ApproveRequest {
            id: id.clone(),
            note: None,
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &option(1),
        &mut consent,
    );
    let reason = refused(&outcome, "approve over the record's commit");
    assert!(
        reason.starts_with(&format!("`{id}` not applied (step 4): the commit {head} ")),
        "{reason}"
    );
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::reject(
        &env_at(&restored, &pair.main),
        &Globals::default(),
        &specengine_cli::RejectRequest {
            id: id.clone(),
            reason: "No.".to_owned(),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut consent,
    );
    let reason = refused(&outcome, "reject over the record's commit");
    assert!(reason.contains(&head), "{reason}");
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state().refs, files.refs, "no commit");
    assert_eq!(pair.state().linked_files, files.linked_files);
    let after = specengine_store::SqliteQueue::open(
        common::data_dir(&restored).join("lantern-keep.db"),
        "lantern-keep",
    )
    .unwrap()
    .get(&id)
    .unwrap()
    .unwrap();
    assert_eq!(after.status, ProposalStatus::Open);
    assert!(after.record.is_none());
    assert!(
        dump_at(&pair, &restored).starts_with(queue_before.trim_end()),
        "no row changed"
    );
}

// ------------------------------------------------------------ config, template

/// "Data", Config: the table is checked on load by every command: a legacy
/// alias as `prefix` makes `spec tree` exit 2 with
/// `specengine.toml:<line>:` naming the canonical prefix; a second key too
/// many, likewise at its line.
#[test]
fn a_bad_table_stops_every_command_at_its_line() {
    let scratch = common::Scratch::new("da-config");
    let root = scratch.copy("spec-a", "root");
    let home = scratch.home("h");
    let config = read_text(&root, "specengine.toml");
    for (label, text, at, want) in [
        (
            "an alias",
            with_records(&config, "QST", "docs/records/Q", "templates/decision.md"),
            "prefix ",
            "`decision_records.prefix`: `QST` is a legacy alias of `Q`: name the canonical prefix",
        ),
        (
            "an extra key",
            format!("{}extra    = \"x\"\n", config),
            "extra ",
            "unknown field `extra`",
        ),
    ] {
        write(&root, "specengine.toml", &text);
        let line = text
            .lines()
            .position(|line| line.starts_with(at))
            .expect("the key's line")
            + 1;
        let run = common::spec(&home, &root, &["tree"]);
        assert_eq!(run.code, 2, "{label}: {}", run.show());
        assert!(
            run.stderr.starts_with(&format!("specengine.toml:{line}: ")),
            "{label}: {}",
            run.show()
        );
        assert!(run.stderr.contains(want), "{label}: {}", run.show());
    }
}

/// Step 3: the template only as committed (tracked, clean, no symlink, its
/// slots known), else exit 2 `<template>:<line>: <problem>`; step 4: `dir`
/// in the walk (else exit 2 naming it), nothing at the path (a dangling
/// symlink, an index entry: exit 1 `exists`); step 6: a template that
/// renders no decision record is the template's error (exit 2). Each
/// refusal logged at its step, nothing written, no prompt.
#[test]
fn the_template_and_the_path_are_checked_before_the_prompt() {
    let pair = Pair::new("da-steps", "spec-a");
    let id = ac01_item(&pair);
    let template = read_text(&pair.linked, "templates/decision.md");
    let config = read_text(&pair.linked, "specengine.toml");
    let attempt = |context: &str| {
        let before = pair.state();
        let (outcome, questions) = pair.decide(&pair.main, &id, &option(1), None, true);
        assert!(questions.is_empty(), "{context}: {questions:?}");
        assert_eq!(pair.state(), before, "{context}");
        outcome
    };
    let last_failure = || pair.events_of(&id).last().cloned().expect("an event");

    // A dirty template (uncommitted).
    write(
        &pair.linked,
        "templates/decision.md",
        format!("{template}\n"),
    );
    let message = cannot(&attempt("dirty"), "dirty");
    assert!(
        message.ends_with(&format!(
            "templates/decision.md:1: has uncommitted changes in {} (staged or not): commit, \
             stash or restore them first",
            pair.linked.display()
        )),
        "{message}"
    );
    assert_eq!(
        last_failure(),
        ("proposal.apply_failed".to_owned(), Some(3))
    );

    // An unknown slot, committed: named at its line.
    let unknown = template.replacen("Note:\n", "Note: {{nope}}\n", 1);
    write(&pair.linked, "templates/decision.md", &unknown);
    pair.commit_all(&pair.linked, "An unknown slot.");
    let line = unknown
        .lines()
        .position(|line| line == "Note: {{nope}}")
        .unwrap()
        + 1;
    let message = cannot(&attempt("unknown slot"), "unknown slot");
    assert!(
        message.contains(&format!(
            "(step 3): templates/decision.md:{line}: `{{{{nope}}}}` is no slot"
        )),
        "{message}"
    );

    // A symlink to a good template.
    write(&pair.linked, "templates/real.md", &template);
    fs::remove_file(pair.linked.join("templates/decision.md")).unwrap();
    std::os::unix::fs::symlink("real.md", pair.linked.join("templates/decision.md")).unwrap();
    pair.commit_all(&pair.linked, "A symlinked template.");
    let message = cannot(&attempt("symlink"), "symlink");
    assert!(
        message.contains("templates/decision.md:1: `templates/decision.md` is a symlink"),
        "{message}"
    );

    // A template that renders no decision record (class canon).
    fs::remove_file(pair.linked.join("templates/decision.md")).unwrap();
    write(
        &pair.linked,
        "templates/decision.md",
        template.replacen("class: decision", "class: canon", 1),
    );
    pair.commit_all(&pair.linked, "A canon template.");
    let message = cannot(&attempt("class canon"), "class canon");
    assert!(
        message.contains(
            "(step 6): templates/decision.md: the record it renders is no decision record"
        ),
        "{message}"
    );
    assert_eq!(
        last_failure(),
        ("proposal.apply_failed".to_owned(), Some(6))
    );
    write(&pair.linked, "templates/decision.md", &template);
    pair.commit_all(&pair.linked, "The template back.");

    // `dir` outside the walk.
    write(
        &pair.linked,
        "specengine.toml",
        with_records(&config, "DEC", "notes/DEC", "templates/decision.md"),
    );
    pair.commit_all(&pair.linked, "Records outside the walk.");
    let message = cannot(&attempt("outside the walk"), "outside");
    assert!(
        message.contains("(step 4): `notes/DEC/DEC-0024.md` lies outside the walk of"),
        "{message}"
    );
    assert!(
        message.contains("`[decision_records] dir` = `notes/DEC`"),
        "{message}"
    );
    write(&pair.linked, "specengine.toml", &config);
    pair.commit_all(&pair.linked, "Records inside again.");

    // A dangling symlink at the path; then an index entry there.
    let path = dec_path("DEC-0024");
    std::os::unix::fs::symlink("nowhere.md", pair.linked.join(&path)).unwrap();
    let reason = refused(&attempt("a dangling symlink"), "dangling");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 4): `{path}` exists: a decision record never replaces a \
             file; nothing changed"
        )
    );
    fs::remove_file(pair.linked.join(&path)).unwrap();
    write(&pair.linked, &path, "left behind\n");
    pair.git
        .git(&pair.linked, &["add", "--intent-to-add", "--", &path]);
    fs::remove_file(pair.linked.join(&path)).unwrap();
    let reason = refused(&attempt("an index entry"), "index entry");
    assert_eq!(
        reason,
        format!(
            "`{id}` not applied (step 4): `{path}` exists: a decision record never replaces a \
             file; nothing changed"
        )
    );
    pair.git
        .git(&pair.linked, &["rm", "--cached", "--quiet", "--", &path]);

    // All clear: it applies.
    pair.decide_ok(&pair.main, &id, &option(1), None);
    assert_eq!(pair.proposal(&id).record.expect("stored").id, "DEC-0024");
}

/// Step 3: a template whose front-matter holds no `{{canon}}` (no `canon:`
/// line; a literal `canon:`; `{{canon}}` only in the body) is the
/// template's error at its line 1, `--canon` given or not; a character a
/// record never carries (ESC in the body, U+202E in the front-matter, CR
/// on line 1) is named at its line. Step 6: `canon: [{{canon}}]`, which
/// reads back no canon whatever it is given (the probe with the record's
/// own ID), is the template's error naming it, not `--canon`'s. Each exit
/// 2, logged at its step, no prompt, nothing written; the fixture's
/// template then applies with the same flags. M: each step-3 check
/// removed; the step-6 probe removed.
#[test]
fn a_template_that_cannot_carry_a_record_is_its_own_error() {
    let pair = Pair::new("da-template-canon", "spec-a");
    let id = ac01_item(&pair);
    let template = read_text(&pair.linked, "templates/decision.md");
    let given = flags(Some(1), None, Some("RULE-STAM-REGEN"));
    let attempt = |label: &str, text: &str| -> String {
        write(&pair.linked, "templates/decision.md", text);
        pair.commit_all(&pair.linked, label);
        let before = pair.state();
        let (outcome, questions) = pair.decide(&pair.main, &id, &given, None, false);
        let message = cannot(&outcome, label);
        assert!(questions.is_empty(), "{label}: {questions:?}");
        assert_eq!(pair.state(), before, "{label}: nothing written");
        message
    };
    let failed_at = |step: u64| {
        assert_eq!(
            pair.events_of(&id).last().cloned(),
            Some(("proposal.apply_failed".to_owned(), Some(step)))
        );
    };
    let step_3 = |line: usize, problem: &str| {
        format!("spec: `{id}` not applied (step 3): templates/decision.md:{line}: {problem}")
    };
    let line_of = |text: &str, prefix: &str| {
        text.lines()
            .position(|line| line.starts_with(prefix))
            .expect("the line")
            + 1
    };
    let no_canon = "its front-matter holds no `{{canon}}`: a record names the section it governs \
                    as `canon: {{canon}}`";

    let absent = template.replacen("canon: {{canon}}\n", "", 1);
    assert_ne!(absent, template);
    assert_eq!(attempt("no canon line", &absent), step_3(1, no_canon));
    failed_at(3);
    let literal = template.replacen("canon: {{canon}}\n", "canon: RULE-STAM-REGEN\n", 1);
    assert_eq!(attempt("a literal canon", &literal), step_3(1, no_canon));
    let body_only = absent.replacen("Note:\n", "Note: it governs {{canon}}\n", 1);
    assert_ne!(body_only, absent);
    assert_eq!(
        attempt("canon in the body", &body_only),
        step_3(1, no_canon)
    );

    let esc = template.replacen("Note:\n", "Note:\u{1b}[31m\n", 1);
    assert_eq!(
        attempt("ESC", &esc),
        step_3(
            line_of(&esc, "Note:"),
            "holds U+001B: a decision record never carries it"
        )
    );
    let rlo = template.replacen("scope: [decisions]\n", "scope: [decisions]\u{202e}\n", 1);
    assert_eq!(line_of(&rlo, "scope:"), 11);
    assert_eq!(
        attempt("U+202E", &rlo),
        step_3(11, "holds U+202E: a decision record never carries it")
    );
    let cr = format!(
        "---\r\n{}",
        template.strip_prefix("---\n").expect("front-matter")
    );
    assert_eq!(
        attempt("CR", &cr),
        step_3(1, "holds U+000D: a decision record never carries it")
    );
    failed_at(3);

    let listed = template.replacen("canon: {{canon}}\n", "canon: [{{canon}}]\n", 1);
    assert_eq!(
        attempt("canon in a list", &listed),
        format!(
            "spec: `{id}` not applied (step 6): templates/decision.md: the record it renders is \
             no decision record: the record's `canon:` does not read back as its slot"
        )
    );
    failed_at(6);

    write(&pair.linked, "templates/decision.md", &template);
    pair.commit_all(&pair.linked, "The template back.");
    pair.decide_ok(&pair.main, &id, &given, None);
    let record = read_text(&pair.linked, &dec_path("DEC-0024"));
    assert!(record.contains("\ncanon: RULE-STAM-REGEN\n"), "{record}");
}

// ------------------------------------------------------------ reject

/// `spec reject` of a question or a discrepancy: one holding no record is
/// refused only by a commit carrying its trailer. Its worktree removed (the
/// branch kept): the history is read, no commit, rejected with no note;
/// its branch deleted too (as after a merge): rejected, the prompt opening
/// with the note that its history cannot be read, the outcome carrying it.
/// One holding a record issued at an earlier step 7 (reopened after a
/// failed step 9) is refused as an update is when git cannot tell: no
/// prompt, nothing written, `open` with its record. M: a lookup error
/// refuses; the record ignored.
#[test]
fn reject_reads_history_only_for_an_item_that_may_have_a_commit() {
    let pair = Pair::new("da-reject-gone", "spec-a");
    let kept_branch = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Does regeneration wait for full rest?",
        "Yes",
        "a new case in the rule",
    );
    let no_record =
        pair.raise_question(&pair.linked, &["EDGE-STAM-ZERO"], "Stop?", "yes", "a rule");
    let with_record = ac01_item(&pair);
    hook(&pair, Some("#!/bin/sh\nexit 1\n"));
    let (outcome, _) = pair.decide(&pair.main, &with_record, &option(1), None, true);
    refused(&outcome, "the hook");
    hook(&pair, None);
    let held = pair.proposal(&with_record);
    assert_eq!(held.status, ProposalStatus::Open);
    assert_eq!(held.record.as_ref().expect("kept").id, "DEC-0024");
    let linked = pair.linked.display().to_string();
    let prompt = |id: &str| {
        let proposal = pair.proposal(id);
        format!(
            "reject {id} ({} in {} on t1 in {linked})? [y/N]",
            proposal.target_id, proposal.target_path
        )
    };
    let rejected = |id: &str, label: &str| {
        let (outcome, questions) = pair.reject_answer(&pair.main, id, "Not now.", true);
        let outcome = outcome.unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{label}: {outcome:?}");
        let stored = pair.proposal(id);
        assert_eq!(stored.status, ProposalStatus::Rejected, "{label}");
        assert_eq!(stored.decision_note.as_deref(), Some("Not now."), "{label}");
        assert!(stored.record.is_none(), "{label}");
        (outcome, questions)
    };

    pair.git
        .git(&pair.main, &["worktree", "remove", "--force", &linked]);
    assert!(!pair.linked.exists());
    let (outcome, questions) = rejected(&kept_branch, "the branch kept");
    assert_eq!(outcome.messages, []);
    assert_eq!(questions, [prompt(&kept_branch)]);

    pair.git.git(&pair.main, &["branch", "-D", "t1"]);
    let note = format!(
        "`{no_record}` holds no record, and its history cannot be read (the branch `t1` no \
         longer exists): rejected without looking for a commit of it"
    );
    let expected_prompt = format!("note: {note}\n{}", prompt(&no_record));
    let (outcome, questions) = rejected(&no_record, "the branch gone");
    assert_eq!(outcome.messages, [Message::Note(note.clone())]);
    assert_eq!(questions, [expected_prompt]);

    let before = pair.state();
    let (outcome, questions) = pair.reject_answer(&pair.main, &with_record, "Not now.", true);
    let reason = refused(&outcome, "a record issued");
    assert!(
        reason.starts_with(&format!(
            "cannot tell whether `{with_record}` has its commit in history: the branch `t1` no \
             longer exists; recreate it"
        )),
        "{reason}"
    );
    assert!(
        reason.ends_with(&format!(
            "then `spec approve {with_record}` or `spec reject {with_record}`"
        )),
        "{reason}"
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    let held = pair.proposal(&with_record);
    assert_eq!(held.status, ProposalStatus::Open);
    assert_eq!(held.record.expect("kept").id, "DEC-0024");
}

// ------------------------------------------------------------ interrupted

/// Step 4 after a run killed between its steps 8 and 9 (`approved`, the
/// file holding the stored `record_text`, its index entry intent-to-add):
/// the `exists` refusal names the two ways out in the worktree; the same
/// file with other bytes, or its bytes staged by a real `git add`, keeps
/// the plain refusal (an index entry with no file:
/// `the_template_and_the_path_are_checked_before_the_prompt`). Way one,
/// the hand commit it names: approve completes it, no prompt, no new
/// commit; way two, `git rm --cached` and the file deleted: approve writes
/// it again, `A <path>`. M: the hint over other bytes; over a real add.
#[test]
fn a_record_left_by_a_killed_run_names_its_two_ways_out() {
    for way in ["commit", "remove"] {
        let pair = Pair::new(&format!("da-killed-{way}"), "spec-a");
        let id = ac01_item(&pair);
        let record = stopped_after_step_7(&pair, &id);
        let path = record.path.clone();
        assert_eq!(path, dec_path("DEC-0024"));
        let attempt = |label: &str| -> String {
            let before = pair.state();
            let (outcome, questions) =
                pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
            let reason = refused(&outcome, label);
            assert!(questions.is_empty(), "{label}: {questions:?}");
            assert_eq!(pair.state(), before, "{label}: nothing written");
            assert_eq!(pair.proposal(&id).status, ProposalStatus::Approved);
            reason
        };
        let take_back = || {
            pair.git
                .git(&pair.linked, &["rm", "--cached", "-q", "--", &path]);
            fs::remove_file(pair.linked.join(&path)).expect("the file");
        };
        let plain = format!(
            "`{id}` not applied (step 4): `{path}` exists: a decision record never replaces a \
             file; nothing changed"
        );

        let other = record
            .text
            .replacen(&format!("date: {DAY}"), "date: 2026-10-09", 1);
        assert_ne!(other, record.text);
        write(&pair.linked, &path, &other);
        pair.git
            .git(&pair.linked, &["add", "--intent-to-add", "--", &path]);
        assert_eq!(attempt("other bytes"), plain);
        take_back();
        write(&pair.linked, &path, &record.text);
        pair.git.git(&pair.linked, &["add", "--", &path]);
        assert_eq!(attempt("a real add"), plain);
        take_back();

        write(&pair.linked, &path, &record.text);
        pair.git
            .git(&pair.linked, &["add", "--intent-to-add", "--", &path]);
        let commit_by_hand =
            format!("git commit --only --trailer 'Proposal: {id}' -m 'DEC-0024' -- {path}");
        assert_eq!(
            attempt("left behind"),
            format!(
                "`{id}` not applied (step 4): `{path}` exists: a decision record never replaces \
                 a file; it holds `DEC-0024`'s record as an interrupted apply left it, with an \
                 intent-to-add entry in git's index; two ways out in {}: commit it by hand \
                 (`{commit_by_hand}`), then `spec approve {id}` completes it; or `git rm \
                 --cached -- {path}`, delete the file, then `spec approve {id}` writes it again; \
                 nothing changed",
                pair.linked.display()
            )
        );

        let tip = pair.rev(&pair.linked, "HEAD");
        let (outcome, _) = if way == "commit" {
            let trailer = format!("Proposal: {id}");
            pair.git.git(
                &pair.linked,
                &[
                    "commit",
                    "-q",
                    "--only",
                    "--trailer",
                    &trailer,
                    "-m",
                    "DEC-0024",
                    "--",
                    &path,
                ],
            );
            let by_hand = pair.rev(&pair.linked, "HEAD");
            assert_eq!(name_status(&pair, &by_hand), [format!("A\t{path}")]);
            let done = pair.decide_at(
                &pair.main,
                &id,
                &flags(None, None, None),
                None,
                true,
                "2026-10-09T08:00:00Z",
            );
            assert!(done.1.is_empty(), "no prompt: {:?}", done.1);
            assert_eq!(pair.rev(&pair.linked, "t1"), by_hand, "no new commit");
            assert_eq!(
                pair.proposal(&id).applied_commit.as_deref(),
                Some(by_hand.as_str())
            );
            done
        } else {
            take_back();
            let done = pair.decide_at(
                &pair.main,
                &id,
                &flags(None, None, None),
                None,
                true,
                "2026-10-09T08:00:00Z",
            );
            assert_eq!(done.1.len(), 1, "{:?}", done.1);
            assert!(
                matches!(&done.0, Ok(outcome) if outcome.exit() == Exit::Answered),
                "{:?}",
                done.0
            );
            let head = pair.rev(&pair.linked, "t1");
            assert_eq!(
                pair.git_text(&pair.main, &["rev-list", "--parents", "-n", "1", &head]),
                format!("{head} {tip}"),
                "one new commit on the old tip"
            );
            assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
            assert_eq!(read_text(&pair.linked, &path), record.text);
            done
        };
        let outcome = outcome.unwrap_or_else(|error| panic!("{way}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{way}: {outcome:?}");
        assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied, "{way}");
        assert_eq!(pair.porcelain(&pair.linked), "", "{way}: a clean worktree");
    }
}

/// "First": an `open` item holding a record (reopened after a failed step
/// 9) whose commit, made by hand from `record_text`, completes it: flags
/// naming another choice (`--option`, `--answer`) or another canon
/// (`--canon`, compared with what the stored record reads back) exit 2
/// naming the stored choice, the commit and what the flags name, before
/// any prompt and before anything is written; the same flags (a question:
/// none) complete it by that commit after the owner's consent, no new
/// commit. M: the flags ignored; the canon ignored.
#[test]
fn a_held_record_completed_by_hand_takes_no_other_choice() {
    let pair = Pair::new("da-held", "spec-a");
    let discrepancy_id = ac01_item(&pair);
    let question_id = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Is the delay configurable per level?",
        "No",
        "a settings key and a test",
    );
    hook(&pair, Some("#!/bin/sh\nexit 1\n"));
    for (id, given) in [
        (&discrepancy_id, option(1)),
        (&question_id, flags(None, Some("Yes, a key"), None)),
    ] {
        let (outcome, _) = pair.decide(&pair.main, id, &given, None, true);
        refused(&outcome, "the hook");
    }
    hook(&pair, None);
    let mut tips = Vec::new();
    for id in [&discrepancy_id, &question_id] {
        let held = pair.proposal(id);
        assert_eq!(held.status, ProposalStatus::Open);
        let record = held.record.expect("kept by the reopen");
        write(&pair.linked, &record.path, &record.text);
        pair.git.git(&pair.linked, &["add", "--", &record.path]);
        pair.git.git_stdin(
            &pair.linked,
            &["commit", "-q", "--no-verify", "-F", "-"],
            record_message(&pair, id, &record.id, &record.title).as_bytes(),
        );
        tips.push(pair.rev(&pair.linked, "HEAD"));
    }
    let tip = pair.rev(&pair.linked, "t1");
    let held = |id: &str, given: &ApproveFlags, label: &str| -> String {
        let before = pair.state();
        let events = pair.events().len();
        let (outcome, questions) = pair.decide(&pair.main, id, given, None, true);
        let message = cannot(&outcome, label);
        assert!(questions.is_empty(), "{label}: {questions:?}");
        assert_eq!(pair.state(), before, "{label}: nothing written");
        assert_eq!(pair.events().len(), events, "{label}: no event");
        message
    };
    let named = |what: &str| {
        format!(
            "spec: `{discrepancy_id}` holds its record `DEC-0024` (option 1), completed by its \
             commit {} on `t1`: `spec approve {discrepancy_id} --option 1` completes it as \
             recorded; the flags name {what}; nothing changed",
            tips[0]
        )
    };
    assert_eq!(
        held(&discrepancy_id, &option(2), "another option"),
        named("option 2")
    );
    assert_eq!(
        held(
            &discrepancy_id,
            &flags(Some(1), None, Some("MEC-STAMINA#RULE-STAM-REGEN")),
            "another canon"
        ),
        named("the canon `MEC-STAMINA#RULE-STAM-REGEN` (the record's: `RULE-STAM-REGEN`)")
    );
    assert_eq!(
        held(
            &discrepancy_id,
            &flags(Some(2), None, Some("EDGE-STAM-ZERO")),
            "both"
        ),
        named("option 2 and the canon `EDGE-STAM-ZERO` (the record's: `RULE-STAM-REGEN`)")
    );
    assert_eq!(
        held(
            &question_id,
            &flags(None, Some("No"), None),
            "another answer"
        ),
        format!(
            "spec: `{question_id}` holds its record `DEC-0025` (the given answer), completed by \
             its commit {} on `t1`: `spec approve {question_id}` completes it as recorded; the \
             flags name another answer; nothing changed",
            tips[1]
        )
    );

    for (index, id, given) in [
        (
            0,
            &discrepancy_id,
            flags(Some(1), None, Some("RULE-STAM-REGEN")),
        ),
        (1, &question_id, flags(None, None, None)),
    ] {
        let (outcome, questions) = pair.decide(&pair.main, id, &given, None, true);
        let outcome = outcome.unwrap_or_else(|error| panic!("complete {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        assert_eq!(
            questions,
            [format!(
                "complete {id} by its commit {} on t1 in {}? [y/N]",
                tips[index],
                pair.linked.display()
            )]
        );
        let stored = pair.proposal(id);
        assert_eq!(stored.status, ProposalStatus::Applied);
        assert_eq!(stored.applied_commit.as_deref(), Some(tips[index].as_str()));
    }
    assert_eq!(pair.rev(&pair.linked, "t1"), tip, "no new commit");
}

// ------------------------------------------------------------ title

/// "Data", `title`: the answer's first line that is not blank, whitespace
/// runs one space: an `--answer` opening with blank lines titles the
/// record (front-matter and H1) and its commit body by its first line of
/// text; `choice` keeps the answer whole. M: the first line taken.
#[test]
fn a_title_is_the_first_line_of_text_of_the_answer() {
    let pair = Pair::new("da-title", "spec-a");
    let text = "Is the delay configurable per level?";
    let id = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        text,
        "No",
        "a settings key and a test",
    );
    let answer = "\n \t\nThe   real answer\nwith a default.";
    pair.decide_ok(&pair.main, &id, &flags(None, Some(answer), None), None);
    let want = spec_a_record(
        "DEC-0024",
        "\"The real answer\"",
        "Q-031",
        "[Q-031]",
        &id,
        &RecordBody {
            title: "The real answer",
            choice: answer,
            effect: "",
            cost: "a settings key and a test",
            summary: text,
            options: "",
            evidence: "",
            note: "",
        },
    );
    assert_eq!(read_text(&pair.linked, &dec_path("DEC-0024")), want);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(
        pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]),
        record_message(&pair, &id, "DEC-0024", "The real answer").trim_end()
    );
    assert_eq!(
        pair.proposal(&id).record.expect("stored").title,
        "The real answer"
    );
}
