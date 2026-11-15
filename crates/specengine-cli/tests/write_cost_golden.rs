//! docs/features/write-cost.md (RM-0125) WC9, the terminal's half: on every
//! fixture corpus, `spec propose update|create|amendment|interpretation`,
//! with and without `--brief`, and each deciding kind's approve (a
//! question's, a discrepancy's, a decision's; an update's too) up to its
//! prompt, declined, answer byte for byte as 0.47.0 did
//! (`fixtures/write-cost-golden/<corpus>.json`, section `cli`). Through the
//! library as `spec` runs it, on an injected clock and a fixed git identity,
//! each answer as `spec` prints it (stdout text, `--json`, stderr), a
//! refusal too. The shared half is `write_cost_common/mod.rs`.
//!
//! M: findings sorted by another key (`introduced_findings` sorting its
//! list by message): red, the first case naming two findings.
//!
//! Capture (at `v0.47.0` only): `SPECENGINE_WRITE_COST_CAPTURE=1 cargo
//! nextest run -p specengine-cli --test write_cost_golden`.

#![cfg(unix)]

mod write_cost_common;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use specengine_cli::{
    AmendmentRequest, ApproveFlags, ApproveRequest, CreateRequest, DecisionInput, DecisionRequest,
    DiscrepancyInput, DiscrepancyRequest, Evidence, GapType, Globals, IntakeOption, IntakeOutcome,
    IntakeSeverity, InterpretRequest, Outcome, ProposalOutcome, ProposeRequest, ProposedText,
    QuestionRequest, approve_with, propose, propose_amendment, propose_amendment_brief,
    propose_brief, propose_create, propose_create_brief, propose_decision, propose_discrepancy,
    propose_interpretation, propose_interpretation_brief, propose_question,
};
use write_cost_common::{
    CORPORA, LATER, NOW, Probe, Repo, appended, corpora_on_disk, first_markdown, front_matter,
    new_file, normalized, printed, probe, settle, sibling, without_keys,
};

const ROLE: &str = "spec-writer";
const MODEL: &str = "claude-opus-5-5";

fn proposal(result: Result<ProposalOutcome, specengine_cli::CliError>) -> (Value, Option<String>) {
    let id = result
        .as_ref()
        .ok()
        .filter(|outcome| outcome.refusal.is_none())
        .and_then(|outcome| outcome.document.id.clone());
    (
        printed(result.map(|outcome| Outcome::Proposal(Box::new(outcome)))),
        id,
    )
}

fn intake(result: Result<IntakeOutcome, specengine_cli::CliError>) -> Value {
    printed(result.map(|outcome| Outcome::Intake(Box::new(outcome))))
}

/// The gating similar entries a create met, by name; empty when none gated
/// or it was stored.
fn gates(result: &Result<ProposalOutcome, specengine_cli::CliError>) -> Vec<String> {
    let Ok(outcome) = result else {
        return Vec::new();
    };
    if outcome.document.id.is_some() {
        return Vec::new();
    }
    outcome
        .document
        .similar
        .iter()
        .flatten()
        .filter(|entry| entry.gates)
        .map(|entry| entry.name().to_owned())
        .collect()
}

/// The stored intake's ID, if it was stored.
fn stored(result: &Result<IntakeOutcome, specengine_cli::CliError>) -> Option<String> {
    result
        .as_ref()
        .ok()
        .filter(|outcome| outcome.document.created)
        .and_then(|outcome| outcome.document.id.clone())
}

/// The hits a first ask met, to be named as distinct in a second.
fn hits(result: &Result<IntakeOutcome, specengine_cli::CliError>) -> Option<Vec<String>> {
    let outcome = result.as_ref().ok()?;
    (outcome.refusal.is_none() && !outcome.document.created).then(|| {
        outcome
            .document
            .hits
            .iter()
            .map(|hit| hit.name().to_owned())
            .collect()
    })
}

/// The creates of a new file beside `path`, with and without `--brief`, its
/// front matter `front` without its IDs: a first ask a similar entry gates
/// is asked again naming each.
fn creates(repo: &Repo, path: &str, front: &str) -> Vec<(String, Value)> {
    let env = repo.env();
    let globals = Globals::default();
    let front = without_keys(front, &["id", "aliases"]);
    let create = |name: &str, tag: &str| CreateRequest {
        target: sibling(path, name),
        base: None,
        text: ProposedText::Given(new_file(&front, "Write cost probe", tag).into_bytes()),
        rationale: "A new probe file.".to_owned(),
        author_role: Some(ROLE.to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
        distinct_from: Vec::new(),
        keywords: Vec::new(),
    };
    let mut out = Vec::new();
    for (case, name, tag, brief) in [
        ("propose create", "zz-wc-create.md", "wc-create", false),
        (
            "propose create --brief",
            "zz-wc-create-brief.md",
            "wc-create-brief",
            true,
        ),
    ] {
        let run = |request: &CreateRequest| {
            if brief {
                propose_create_brief(&env, &globals, request)
            } else {
                propose_create(&env, &globals, request)
            }
        };
        let mut request = create(name, tag);
        let created = run(&request);
        let gating = gates(&created);
        out.push((case.to_owned(), proposal(created).0));
        if !gating.is_empty() {
            request.distinct_from = gating;
            out.push((format!("{case}, distinct"), proposal(run(&request)).0));
        }
    }
    out
}

/// Every case of one corpus, in a fresh repository.
fn cases(corpus: &str) -> BTreeMap<String, Value> {
    let repo = Repo::new("cli", corpus);
    let env = repo.env();
    let globals = Globals::default();
    let mut cases = BTreeMap::new();
    let probe = match probe(&env, &repo.git_env()) {
        Ok(Some(probe)) => probe,
        Ok(None) => {
            // No node with an ID: the creates alone, beside the first file.
            cases.insert("probe".to_owned(), Value::Null);
            if let Some(path) = first_markdown(&repo.root) {
                let file = std::fs::read_to_string(repo.root.join(&path)).unwrap_or_default();
                for (case, answer) in creates(&repo, &path, &front_matter(&file)) {
                    cases.insert(case, normalized(answer, &repo.root, &[]));
                }
            }
            return cases;
        }
        Err(error) => {
            let error = normalized(json!({"error": error}), &repo.root, &[]);
            cases.insert("probe".to_owned(), error);
            return cases;
        }
    };
    let Probe {
        target,
        path,
        base,
        text,
        decision,
        front,
    } = probe.clone();
    cases.insert(
        "probe".to_owned(),
        json!({"target": target, "path": path, "base": base, "decision": decision}),
    );
    let mut put = |case: &str, value: Value| {
        cases.insert(case.to_owned(), normalized(value, &repo.root, &[]));
    };

    // Updates.
    let update = |tag: &str| ProposeRequest {
        target: target.clone(),
        base: base.clone(),
        text: ProposedText::Given(appended(&text, tag).into_bytes()),
        rationale: format!("Probe {target}."),
        author_role: Some(ROLE.to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let (answer, update_id) = proposal(propose(&env, &globals, &update("wc-update")));
    put("propose update", answer);
    let (answer, _) = proposal(propose_brief(&env, &globals, &update("wc-update-brief")));
    put("propose update --brief", answer);

    for (case, answer) in creates(&repo, &path, &front) {
        put(&case, answer);
    }
    let front = without_keys(&front, &["id", "aliases"]);

    // Amendments: a new file amending the target, a draft spec (only a
    // spec or a decision goes to Tier 3 by status).
    let amending = format!(
        "{}class: spec\nstatus: draft\nlinks:\n  amends: [{target}]\n",
        without_keys(&front, &["links", "class", "status", "tier"])
    );
    let amend = |name: &str, tag: &str| AmendmentRequest {
        target: sibling(&path, name),
        text: ProposedText::Given(new_file(&amending, "Write cost amendment", tag).into_bytes()),
        rationale: "Amend the probe.".to_owned(),
        author_role: Some(ROLE.to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let (answer, _) = proposal(propose_amendment(
        &env,
        &globals,
        &amend("zz-wc-amends.md", "wc-amends"),
        None,
    ));
    put("propose amendment", answer);
    let (answer, _) = proposal(propose_amendment_brief(
        &env,
        &globals,
        &amend("zz-wc-amends-brief.md", "wc-amends-brief"),
        None,
    ));
    put("propose amendment --brief", answer);

    // Interpretations of the first accepted node.
    let interpret = |tag: &str| InterpretRequest {
        target: target.clone(),
        base: base.clone(),
        decision: decision.clone(),
        how: "Applied as written.".to_owned(),
        why: "The decision left the wording open.".to_owned(),
        text: ProposedText::Given(appended(&text, tag).into_bytes()),
        rationale: "Interpret it.".to_owned(),
        author_role: Some(ROLE.to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let (answer, _) = proposal(propose_interpretation(
        &env,
        &globals,
        &interpret("wc-interpret"),
        None,
    ));
    put("propose interpretation", answer);
    let (answer, _) = proposal(propose_interpretation_brief(
        &env,
        &globals,
        &interpret("wc-interpret-brief"),
        None,
    ));
    put("propose interpretation --brief", answer);

    // Approves up to their prompt, declined.
    let approve_case = |id: Option<String>, flags: ApproveFlags| -> Value {
        let Some(id) = id else {
            return json!("not stored");
        };
        let mut questions: Vec<String> = Vec::new();
        let result = approve_with(
            &env,
            &globals,
            &ApproveRequest {
                id,
                note: None,
                now: LATER.to_owned(),
                git: repo.git_env(),
            },
            &flags,
            &mut |question: &str| {
                questions.push(question.to_owned());
                false
            },
        );
        json!({
            "questions": questions,
            "answer": printed(result.map(|outcome| Outcome::Proposal(Box::new(outcome)))),
        })
    };
    let no_flags = || ApproveFlags {
        option: None,
        answer: None,
        canon: None,
    };
    put("approve update", approve_case(update_id, no_flags()));

    // A question.
    let mut question = QuestionRequest {
        node_ids: vec![target.clone()],
        text: "Does the write cost probe hold as written?".to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        gist: None,
        matters: None,
        options: None,
        recommendation: None,
        severity: None,
        distinct_from: Vec::new(),
        card: None,
        author_role: Some("developer".to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let mut asked = propose_question(&env, &globals, &question);
    put("raise question", intake(asked.clone()));
    if let Some(names) = hits(&asked) {
        question.distinct_from = names;
        asked = propose_question(&env, &globals, &question);
        put("raise question, distinct", intake(asked.clone()));
    }
    put("approve question", approve_case(stored(&asked), no_flags()));

    // A discrepancy, two options.
    let mut input = DiscrepancyInput {
        node_ids: vec![target.clone()],
        summary: "The probe and the code disagree.".to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::Normal,
        evidence: vec![Evidence {
            file: "src/probe.rs".to_owned(),
            qpath: Some("probe".to_owned()),
            lines: Some("1-2".to_owned()),
            observed: "returns early".to_owned(),
            documented: "runs to the end".to_owned(),
        }],
        options: [
            ("Keep the spec", "Fix the code", "one work item"),
            ("Change the spec", "Allow it", "a new rule case"),
        ]
        .iter()
        .map(|(label, effect, price)| IntakeOption {
            label: (*label).to_owned(),
            effect: (*effect).to_owned(),
            price: (*price).to_owned(),
        })
        .collect(),
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from: None,
    };
    let report = |input: DiscrepancyInput| {
        propose_discrepancy(
            &env,
            &globals,
            &DiscrepancyRequest {
                input,
                card: None,
                author_role: Some("developer".to_owned()),
                author_model: Some(MODEL.to_owned()),
                run: None,
                login: None,
                now: NOW.to_owned(),
                git: repo.git_env(),
            },
        )
    };
    let mut reported = report(input.clone());
    put("raise discrepancy", intake(reported.clone()));
    if let Some(names) = hits(&reported) {
        input.distinct_from = Some(names);
        reported = report(input);
        put("raise discrepancy, distinct", intake(reported.clone()));
    }
    put(
        "approve discrepancy --option 1",
        approve_case(
            stored(&reported),
            ApproveFlags {
                option: Some(1),
                answer: None,
                canon: None,
            },
        ),
    );

    // A decision.
    let mut decided = DecisionInput {
        node_ids: vec![target.clone()],
        text: "Does the probe stay?".to_owned(),
        working_answer: "Yes: it stays as written.".to_owned(),
        price_of_other: "Another probe.".to_owned(),
        severity: None,
        proposed_patch: None,
        distinct_from: None,
        why: None,
    };
    let raise = |input: DecisionInput| {
        propose_decision(
            &env,
            &globals,
            &DecisionRequest {
                input,
                card: None,
                author_role: Some("developer".to_owned()),
                author_model: Some(MODEL.to_owned()),
                run: None,
                login: None,
                now: NOW.to_owned(),
                git: repo.git_env(),
            },
        )
    };
    let mut raised = raise(decided.clone());
    put("raise decision", intake(raised.clone()));
    if let Some(names) = hits(&raised) {
        decided.distinct_from = Some(names);
        raised = raise(decided);
        put("raise decision, distinct", intake(raised.clone()));
    }
    put(
        "approve decision",
        approve_case(stored(&raised), no_flags()),
    );
    cases
}

/// docs/features/write-cost.md WC6: every corpus's cases inside a check
/// memo of `budget` bytes (a stdio server's: no turn) equal the 0.47.0
/// goldens, the budget 0 (nothing kept) and the whole budget alike.
fn under_a_memo(budget: usize) {
    assert!(
        !write_cost_common::capturing(),
        "a capture runs with no memo in force"
    );
    for corpus in CORPORA {
        let memo = std::sync::Arc::new(specengine_store::CheckMemo::with_budget(budget));
        let cases = specengine_store::check_memo::scoped(&memo, "-", || cases(corpus));
        settle(corpus, "cli", cases);
    }
}

/// WC6: with the budget forced to 0 every answer is the 0.47.0 golden's.
#[test]
fn every_corpus_under_a_memo_of_budget_0() {
    under_a_memo(0);
}

/// WC6: with the whole budget every answer is the 0.47.0 golden's too.
#[test]
fn every_corpus_under_a_memo_of_the_whole_budget() {
    under_a_memo(specengine_store::check_memo::BUDGET);
}

/// The list of corpora is the disk's: a corpus added after 0.47.0 has no
/// golden and must be named here on purpose.
#[test]
fn every_fixture_corpus_is_listed() {
    assert_eq!(
        corpora_on_disk(),
        CORPORA
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    );
}

macro_rules! golden {
    ($($name:ident => $corpus:literal,)*) => {
        $(
            #[test]
            fn $name() {
                settle($corpus, "cli", cases($corpus));
            }
        )*
        /// Every corpus of [`CORPORA`] has its test above.
        #[test]
        fn every_corpus_has_its_test() {
            let tested: Vec<&str> = vec![$($corpus),*];
            assert_eq!(tested, CORPORA.to_vec());
        }
    };
}

golden! {
    anchor_states => "anchor-states",
    apply_outputs_forge => "apply-outputs/forge",
    apply_outputs_press => "apply-outputs/press",
    auto_mini => "auto-mini",
    card_press => "card-press",
    code_a => "code-a",
    code_bevy => "code-bevy",
    code_index => "code-index",
    corpus_mini => "corpus-mini",
    flow_aside => "flow-aside",
    flow_gaps => "flow-gaps",
    flow_hire => "flow-hire",
    flow_relay => "flow-relay",
    impact => "impact",
    import_layout_four => "import-layout/four",
    import_layout_one => "import-layout/one",
    import_layout_three => "import-layout/three",
    import_layout_two => "import-layout/two",
    parent_ref => "parent-ref",
    pilot_w_one => "pilot-w/one",
    pilot_w_two => "pilot-w/two",
    plant_diary => "plant-diary",
    roadmap_a => "roadmap-a",
    roadmap_b => "roadmap-b",
    spec_a => "spec-a",
    spec_b => "spec-b",
    spec_c => "spec-c",
    studio_lamp => "studio-lamp",
    trace_almanac => "trace-almanac",
    verify_ecs => "verify-ecs",
    verify_tests => "verify-tests",
}
