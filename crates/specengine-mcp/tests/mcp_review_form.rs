//! docs/features/review-form.md (RM-0012 slice 1, ADR-0054): the MCP tool
//! `review_proposal {proposal_id}`, the owner's staging form, over the
//! real binary in both protocol eras, the queue as the CLI library reads
//! it:
//!
//! - D1: listed in the default build with exactly the required
//!   `proposal_id` (closed, no choice field), `_meta` the size cap and
//!   `anthropic/requiresUserInteraction: true`, the four hints `false`,
//!   `get_proposal`'s output schema. M1, M2.
//! - D2: a malformed or look-alike ID, an unknown, a rejected, an applied,
//!   another repository's and a drift item: no `elicitation/create`, no
//!   `input_required`, nothing staged, no event, the CLI's line. M6.
//! - D3: a form `accept` stages the owner's answer: one `proposal.staged`,
//!   the row `spec approve PR --option N --note T` would store (the
//!   library's `stage` in a twin `HOME`), shown by a bare `spec approve PR`
//!   as `staged <at>: spec approve PR --option N --note "T"`; content the
//!   outcome line and `spec review PR --brief` read after the stage,
//!   `structuredContent` its `--json`. M7 (with `mcp_door.rs`).
//! - D4: decline, a form cancel, a cancelled call and a client without
//!   form elicitation: `staged` NULL, no event; the cancelled legacy call
//!   withdraws its form with `notifications/cancelled`, its late answer
//!   stages nothing. M3, M5, M8.
//! - D5: a proposal changed between the form and the answer: the CLI's
//!   "changed since it was read" refusal, nothing of the form's staged;
//!   the stateless `requestState` carries the `updated_at` read before the
//!   form (round 2 never re-reads it), unsealed and deterministic. M4.
//! - Agent text in the form is escaped (control characters, bidi marks)
//!   and marked agent-declared; the form's fields follow the kind family,
//!   the same over a second genre (ADR-0008).
//!
//! The setup's git and the library calls run in the CLI tests' sandbox
//! (`common/git.rs`, included by path); every server with a cleared
//! environment and the repository's scratch `HOME`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::read::{
    ERAS, Era, Expected, TOOLS, cli_env, conforms, content_text, expected, output_schemas,
    queue_library, server_git,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CliError, DiscrepancyInput, DiscrepancyRequest, Evidence, Exit,
    GapType, GitEnv, Globals, IntakeOption, IntakeSeverity, Outcome, ProposeRequest, ProposedText,
    QuestionRequest, RejectRequest, ReviewRequest, ShowRequest, StageBody, StageOutcome,
    StageRequest, approve_with, events_after, propose, propose_discrepancy, propose_question,
    reject, show,
};

/// The proposals' injected clock.
const NOW: &str = "2026-10-05T21:14:03Z";
/// A stage or a decision made through the library, after `NOW`.
const LATER: &str = "2026-10-05T21:30:00Z";
const SUMMARY: &str = "Regeneration starts while sprinting in the code";
const META_KEY: &str = "anthropic/requiresUserInteraction";

// ------------------------------------------------------------- the repository

/// A scratch git repository of `fixture` (its main worktree, one commit on
/// `main`, a linked worktree `t1` where proposals are raised) and a data
/// `HOME`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    linked: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        Self::of(label, "spec-a")
    }

    fn of(label: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.copy(fixture, "main");
        let _ = fs::remove_file(main.join("expected.json"));
        Self::committed(scratch, git, main)
    }

    /// `main` committed as it is, `t1` added.
    fn committed(scratch: Scratch, git: Sandbox, main: PathBuf) -> Self {
        git.init(&main);
        git.add_all(&main);
        git.commit(&main, "the fixture");
        let linked = scratch.join("t1");
        git.git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "t1",
                linked.to_str().expect("UTF-8"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            main,
            linked,
            home,
        }
    }

    fn git_env(&self, cwd: &Path) -> GitEnv {
        GitEnv::new(cwd, self.git.vars())
    }

    fn env(&self) -> specengine_cli::Env {
        cli_env(&self.linked, Some(&self.home))
    }

    /// `HOME`'s twin: another data directory over the same repository.
    fn twin_home(&self, name: &str) -> PathBuf {
        self.scratch.home(name)
    }

    fn span(&self, reference: &str) -> (String, String) {
        let outcome = show(
            &self.env(),
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("show {reference}: {error}"));
        let node = outcome.nodes.into_iter().next().expect("one node");
        (node.span_hash, node.text)
    }

    /// An `update` of `reference` raised in `t1` at `NOW` into `home`.
    fn propose_edit_in(&self, home: &Path, reference: &str, from: &str, to: &str) -> String {
        let (base, text) = self.span(reference);
        let outcome = propose(
            &cli_env(&self.linked, Some(home)),
            &Globals::default(),
            &ProposeRequest {
                target: reference.to_owned(),
                base,
                text: ProposedText::Given(text.replacen(from, to, 1).into_bytes()),
                rationale: format!("Edit {reference}."),
                author_role: Some("writer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("propose {reference}: {error}"));
        outcome.document.id.clone().expect("stored")
    }

    fn propose_edit(&self, reference: &str, from: &str, to: &str) -> String {
        self.propose_edit_in(&self.home, reference, from, to)
    }

    /// The discrepancy of `input` raised in `t1` at `NOW` into `home`.
    fn report_in(&self, home: &Path, input: DiscrepancyInput) -> String {
        let outcome = propose_discrepancy(
            &cli_env(&self.linked, Some(home)),
            &Globals::default(),
            &DiscrepancyRequest {
                input,
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("report: {error}"));
        assert!(outcome.document.created, "stored: {outcome:?}");
        outcome.document.id.clone().expect("stored")
    }

    fn report(&self) -> String {
        self.report_in(&self.home, discrepancy(SUMMARY))
    }

    /// A question on `MEC-SPRINT` raised in `t1` at `NOW` into `home`,
    /// declared distinct from its hits when it has any.
    fn ask_in(&self, home: &Path) -> String {
        let ask = |distinct_from: Vec<String>| {
            propose_question(
                &cli_env(&self.linked, Some(home)),
                &Globals::default(),
                &QuestionRequest {
                    node_ids: vec!["MEC-SPRINT".to_owned()],
                    text: "Does a sprint end at zero?".to_owned(),
                    working_answer: "yes".to_owned(),
                    price_of_other: "a rule".to_owned(),
                    severity: None,
                    distinct_from,
                    author_role: Some("developer".to_owned()),
                    author_model: None,
                    run: None,
                    now: NOW.to_owned(),
                    git: self.git_env(&self.linked),
                },
            )
            .expect("asked")
        };
        let first = ask(Vec::new());
        let asked = if first.document.created {
            first
        } else {
            ask(first
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect())
        };
        asked.document.id.clone().expect("stored")
    }

    fn ask(&self) -> String {
        self.ask_in(&self.home)
    }

    /// A question on `node` with `text`, raised in `t1` at `NOW`; it must
    /// be stored (no hit).
    fn ask_on(&self, node: &str, text: &str) -> String {
        let outcome = propose_question(
            &self.env(),
            &Globals::default(),
            &QuestionRequest {
                node_ids: vec![node.to_owned()],
                text: text.to_owned(),
                working_answer: "no".to_owned(),
                price_of_other: "a rule".to_owned(),
                severity: None,
                distinct_from: Vec::new(),
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .expect("asked");
        outcome
            .document
            .id
            .clone()
            .unwrap_or_else(|| panic!("{node}: stored, no hit: {:?}", outcome.document.hits))
    }

    /// `spec review PR --brief`, the parity reference (the server's git:
    /// its working directory and `HOME`).
    fn brief(&self, id: &str) -> Expected {
        self.brief_in(&self.home, id)
    }

    fn brief_in(&self, home: &Path, id: &str) -> Expected {
        let env = cli_env(&self.linked, Some(home));
        queue_library(
            "get_proposal",
            &json!({"proposal_id": id}),
            &env,
            &Globals::default(),
            NOW,
            &server_git(&env),
        )
    }

    /// The `--json` document of `spec review PR --brief`.
    fn document(&self, id: &str) -> Value {
        self.document_in(&self.home, id)
    }

    fn document_in(&self, home: &Path, id: &str) -> Value {
        self.brief_in(home, id)
            .document
            .unwrap_or_else(|| panic!("{id}: no document"))
    }

    /// The library's `stage` of `body` against `updated_at`, which must be
    /// refused (nothing stored): its answer as the form's refusal (canon
    /// `review-form` "Result"), brief.
    fn refused_stage(
        &self,
        id: &str,
        body: impl FnOnce(String) -> StageBody,
        updated_at: String,
    ) -> Expected {
        as_refusal(
            self.full_refused_stage(id, body, updated_at)
                .map(StageOutcome::brief),
        )
    }

    /// The library's refused `stage`, its document full (`spec review`'s).
    fn full_refused_stage(
        &self,
        id: &str,
        body: impl FnOnce(String) -> StageBody,
        updated_at: String,
    ) -> Result<StageOutcome, CliError> {
        let env = self.env();
        let outcome = specengine_cli::stage(
            &env,
            &Globals::default(),
            &StageRequest {
                id: id.to_owned(),
                body: body(updated_at),
                now: LATER.to_owned(),
                git: server_git(&env),
            },
        );
        if let Ok(outcome) = &outcome {
            assert!(
                outcome.cause.is_some(),
                "{id}: the twin stage is refused: {outcome:?}"
            );
        }
        outcome
    }

    /// `staged` and `staged_at` of `id` as `review --brief --json` shows
    /// them.
    fn stage_of(&self, id: &str) -> (Value, Value) {
        let document = self.document(id);
        (document["staged"].clone(), document["staged_at"].clone())
    }

    fn updated_at(&self, id: &str) -> String {
        self.document(id)["updated_at"]
            .as_str()
            .expect("updated_at")
            .to_owned()
    }

    /// The library's `stage` of `id` (the daemon's call) at `now` against
    /// `updated_at`, into `home`.
    fn stage_in(
        &self,
        home: &Path,
        id: &str,
        body: impl FnOnce(String) -> StageBody,
        updated_at: String,
        now: &str,
    ) -> specengine_cli::StageOutcome {
        specengine_cli::stage(
            &cli_env(&self.linked, Some(home)),
            &Globals::default(),
            &StageRequest {
                id: id.to_owned(),
                body: body(updated_at),
                now: now.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("stage {id}: {error}"))
    }

    /// The library's `stage` of `id` against its stored `updated_at`; it
    /// must be stored.
    fn stage(&self, id: &str, body: impl FnOnce(String) -> StageBody, now: &str) {
        let outcome = self.stage_in(&self.home, id, body, self.updated_at(id), now);
        assert_eq!(outcome.cause, None, "stage {id}: {outcome:?}");
    }

    /// The project's events, by `seq`: `(type, payload)`.
    fn events_in(&self, home: &Path) -> Vec<(String, Value)> {
        let page = events_after(
            &cli_env(&self.linked, Some(home)),
            &Globals::default(),
            Some(0),
        )
        .expect("events");
        assert!(!page.full, "fewer events than a page");
        page.events
            .into_iter()
            .map(|event| {
                (
                    event.event_type,
                    serde_json::from_str(&event.payload).expect("a JSON payload"),
                )
            })
            .collect()
    }

    fn events(&self) -> Vec<(String, Value)> {
        self.events_in(&self.home)
    }

    /// The `proposal.staged` events of `id`.
    fn staged_events(&self, id: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|(kind, payload)| kind == "proposal.staged" && payload["id"] == id)
            .map(|(_, payload)| payload)
            .collect()
    }

    /// A bare `spec approve PR` from `main`, declined: its question(s).
    fn bare_approve_questions(&self, id: &str) -> Vec<String> {
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            false
        };
        let outcome = approve_with(
            &cli_env(&self.main, Some(&self.home)),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: LATER.to_owned(),
                git: self.git_env(&self.main),
            },
            &ApproveFlags::default(),
            &mut consent,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::NotFound, "declined: {outcome:?}");
        questions
    }

    /// `spec reject PR --reason R` from `main`, consented.
    fn rejected(&self, id: &str, reason: &str) {
        let mut yes = |_: &str| true;
        let outcome = reject(
            &cli_env(&self.main, Some(&self.home)),
            &Globals::default(),
            &RejectRequest {
                id: id.to_owned(),
                reason: Some(reason.to_owned()),
                now: LATER.to_owned(),
                git: self.git_env(&self.main),
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("reject {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    /// `spec approve PR --option N` from `main`, consented: applied.
    fn applied(&self, id: &str, option: u64) {
        let mut yes = |_: &str| true;
        let outcome = approve_with(
            &cli_env(&self.main, Some(&self.home)),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: LATER.to_owned(),
                git: self.git_env(&self.main),
            },
            &ApproveFlags {
                option: Some(option),
                answer: None,
                canon: None,
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    /// Everything a stage must leave as it was: both worktrees' files,
    /// every ref and `HEAD`, both `git status`.
    fn state(&self) -> impl PartialEq + std::fmt::Debug {
        let status = |dir: &Path| {
            self.git
                .git_text(dir, &["status", "--porcelain=v1", "--untracked-files=all"])
        };
        (
            status(&self.main),
            status(&self.linked),
            self.git.git_text(
                &self.main,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            ),
            self.git.git_text(&self.main, &["rev-parse", "HEAD"]),
            snapshot(&self.main),
            snapshot(&self.linked),
        )
    }
}

/// decision-apply's discrepancy on `RULE-STAM-REGEN` (distinct from its
/// hit `DEC-0023`), two options, `summary` as given.
fn discrepancy(summary: &str) -> DiscrepancyInput {
    let option = |label: &str, effect: &str| IntakeOption {
        label: label.to_owned(),
        effect: effect.to_owned(),
        price: "one item".to_owned(),
    };
    DiscrepancyInput {
        node_ids: vec!["RULE-STAM-REGEN".to_owned()],
        summary: summary.to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::Normal,
        evidence: vec![Evidence {
            file: "src/stamina.rs".to_owned(),
            qpath: None,
            lines: Some("10-20".to_owned()),
            observed: "regenerates while sprinting".to_owned(),
            documented: "regenerates only at rest".to_owned(),
        }],
        options: vec![
            option("Keep the spec", "Fix the code"),
            option("Change the spec", "Regeneration also while sprinting"),
        ],
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from: Some(vec!["DEC-0023".to_owned()]),
    }
}

fn approve_body(option: Option<u64>, note: Option<&str>) -> impl FnOnce(String) -> StageBody {
    let note = note.map(str::to_owned);
    move |updated_at| StageBody::Approve {
        option,
        answer: None,
        canon: None,
        editorial: None,
        note,
        updated_at,
    }
}

fn reject_body(reason: &str) -> impl FnOnce(String) -> StageBody {
    let reason = reason.to_owned();
    move |updated_at| StageBody::Reject { reason, updated_at }
}

// ------------------------------------------------------------ the client

/// One client session in `era` over a server in `t1` with the
/// repository's `HOME`, declaring `capabilities` (legacy: in `initialize`;
/// stateless: in every request's `_meta`).
struct Client {
    server: Server,
    era: Era,
    meta: Value,
    next: u64,
}

impl Client {
    fn open(era: Era, repo: &Repo, capabilities: Value) -> Self {
        Self::open_in(era, &repo.linked, &repo.home, capabilities)
    }

    fn open_in(era: Era, cwd: &Path, home: &Path, capabilities: Value) -> Self {
        let mut server = Server::spawn_with(&[], Some(cwd), Home::At(home));
        if era == Era::Legacy {
            server.initialize(capabilities.clone());
        }
        Self {
            server,
            era,
            meta: stateless_meta(capabilities),
            // Far from the ids a server numbers its own requests with.
            next: 100,
        }
    }

    /// A client declaring form elicitation.
    fn form(era: Era, repo: &Repo) -> Self {
        Self::open(era, repo, form_capabilities())
    }

    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn params(&self, params: Value) -> Value {
        match self.era {
            Era::Legacy => params,
            Era::Stateless => with_meta(params, &self.meta),
        }
    }

    /// The next message, which must answer request `id` (no server request
    /// or notification in between).
    fn answer_to(&mut self, id: u64, context: &str) -> Value {
        let message = self.server.recv();
        assert!(
            message.get("method").is_none(),
            "{context}: the server sent {} before answering",
            clip(&message.to_string())
        );
        assert_eq!(message["id"], json!(id), "{context}: {message}");
        message
    }

    /// One request, its whole reply.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.id();
        let params = self.params(params);
        self.server
            .send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.answer_to(id, method)
    }

    fn tools(&mut self) -> Value {
        result(&self.request("tools/list", json!({}))).clone()
    }

    /// `tools/call`, the whole reply.
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    /// `review_proposal` that must answer without asking: no
    /// `elicitation/create` (legacy), no `input_required` (stateless).
    /// The tool result.
    fn refused(&mut self, proposal_id: &str) -> Value {
        let reply = self.call("review_proposal", json!({"proposal_id": proposal_id}));
        let answer = result(&reply).clone();
        assert_ne!(
            answer["resultType"],
            json!("input_required"),
            "{:?} {proposal_id:?}: a form for a refusal: {}",
            self.era,
            clip(&answer.to_string())
        );
        assert!(
            answer.get("requestState").is_none() && answer.get("inputRequests").is_none(),
            "{:?} {proposal_id:?}: {}",
            self.era,
            clip(&answer.to_string())
        );
        answer
    }

    /// Legacy: sends the call; the next message must be the server's form
    /// request. The call's id and that request.
    fn legacy_ask(&mut self, proposal_id: &str) -> (u64, Value) {
        assert_eq!(self.era, Era::Legacy);
        let id = self.id();
        self.server.send(&json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": "review_proposal", "arguments": {"proposal_id": proposal_id}}
        }));
        let ask = self.server.recv();
        assert_eq!(
            ask["method"],
            json!("elicitation/create"),
            "{proposal_id}: expected the form, got {}",
            clip(&ask.to_string())
        );
        assert!(ask.get("id").is_some(), "a request: {ask}");
        (id, ask)
    }

    /// Legacy: answers the form `ask` with `answer`; the call's result.
    fn legacy_reply(&mut self, call: u64, ask: &Value, answer: Value) -> Value {
        self.server
            .send(&json!({"jsonrpc": "2.0", "id": ask["id"], "result": answer}));
        let reply = self.answer_to(call, "the call after the form");
        result(&reply).clone()
    }

    /// Stateless round 1, which must ask: the result.
    fn round1(&mut self, proposal_id: &str) -> Value {
        let reply = self.call("review_proposal", json!({"proposal_id": proposal_id}));
        let first = result(&reply).clone();
        assert_eq!(
            first["resultType"],
            json!("input_required"),
            "{proposal_id}: round 1: {}",
            clip(&first.to_string())
        );
        first
    }

    /// Stateless round 2: the call again with `state` and `responses`;
    /// the whole reply.
    fn round2(
        &mut self,
        proposal_id: &str,
        state: Option<&str>,
        responses: Option<Value>,
    ) -> Value {
        let mut params =
            json!({"name": "review_proposal", "arguments": {"proposal_id": proposal_id}});
        if let Some(state) = state {
            params["requestState"] = json!(state);
        }
        if let Some(responses) = responses {
            params["inputResponses"] = responses;
        }
        self.request("tools/call", params)
    }

    /// The form of `proposal_id` answered with `answer`, either era: the
    /// form's `elicitation/create` params (no `_meta`: a legacy request's
    /// progress token) and the tool result.
    fn review(&mut self, proposal_id: &str, answer: Value) -> (Value, Value) {
        match self.era {
            Era::Legacy => {
                let (call, ask) = self.legacy_ask(proposal_id);
                let result = self.legacy_reply(call, &ask, answer);
                (shown(&ask["params"]), result)
            }
            Era::Stateless => {
                let first = self.round1(proposal_id);
                let (key, form) = input_request(&first);
                let state = request_state(&first);
                let reply = self.round2(proposal_id, Some(&state), Some(json!({key: answer})));
                (shown(&form), result(&reply).clone())
            }
        }
    }

    /// The form `review_proposal` shows for `proposal_id`, not answered
    /// (legacy: cancelled by the client's `cancel`; stateless: round 1
    /// only).
    fn form_of(&mut self, proposal_id: &str) -> Value {
        match self.era {
            Era::Legacy => {
                let (call, ask) = self.legacy_ask(proposal_id);
                self.legacy_reply(call, &ask, json!({"action": "cancel"}));
                shown(&ask["params"])
            }
            Era::Stateless => shown(&input_request(&self.round1(proposal_id)).1),
        }
    }

    fn finish(self) -> Finished {
        self.server.finish()
    }
}

/// A form's `elicitation/create` params as the owner sees them: without
/// `_meta` (rmcp's progress token on a legacy request).
fn shown(params: &Value) -> Value {
    let mut params = params.clone();
    if let Some(object) = params.as_object_mut() {
        object.remove("_meta");
    }
    params
}

/// Round 1's one input request: its key and its `elicitation/create`
/// params.
fn input_request(first: &Value) -> (String, Value) {
    let requests = first["inputRequests"]
        .as_object()
        .unwrap_or_else(|| panic!("no inputRequests: {}", clip(&first.to_string())));
    assert_eq!(requests.len(), 1, "one form: {first}");
    let (key, request) = requests.iter().next().expect("one");
    assert_eq!(request["method"], json!("elicitation/create"), "{first}");
    (key.clone(), request["params"].clone())
}

fn request_state(first: &Value) -> String {
    first["requestState"]
        .as_str()
        .unwrap_or_else(|| panic!("round 1 without requestState: {first}"))
        .to_owned()
}

/// Every character a terminal shows escaped (`specengine_core`'s
/// `is_escaped`, restated): C0 but LF and TAB, DEL, C1, the bidi marks,
/// embeddings, overrides and isolates.
fn is_escaped(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(
            c,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

/// Every string anywhere in `value`.
fn strings_in(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| strings_in(item, out)),
        Value::Object(object) => {
            for (key, item) in object {
                out.push(key.clone());
                strings_in(item, out);
            }
        }
        _ => {}
    }
}

/// The CLI's answer to the read before any form: `stage_form` (`spec
/// review PR --brief`, refused as `stage` refuses), rendered as `spec`.
fn read_before_form(repo: &Repo, home: &Path, id: &str) -> Expected {
    let env = cli_env(&repo.linked, Some(home));
    as_refusal(
        specengine_cli::stage_form(
            &env,
            &Globals::default(),
            &ReviewRequest {
                id: id.to_owned(),
                git: server_git(&env),
            },
        )
        .map(|answer| answer.outcome),
    )
}

/// A refusal of `stage_form` or `stage` as the read tools answer one
/// (canon `review-form` "Result", `mcp-read.md` **Parity**): exit 1 the
/// CLI's text and document; exit 2 its line(s) only, no document.
fn as_refusal(outcome: Result<StageOutcome, CliError>) -> Expected {
    match outcome {
        Ok(outcome) => {
            let outcome = Outcome::Stage(Box::new(outcome));
            if outcome.exit() == Exit::CannotRun {
                expected(Err(CliError::lines(outcome.stderr_lines())))
            } else {
                expected(Ok(outcome))
            }
        }
        Err(error) => expected(Err(error)),
    }
}

/// A tool result equal to `want` (content, `isError`, `structuredContent`
/// or none), conforming to `schema`.
fn assert_same(result: &Value, want: &Expected, schema: &Value, context: &str) {
    assert_eq!(content_text(result), want.text, "{context}: content");
    assert_eq!(
        result["isError"],
        json!(want.is_error),
        "{context}: isError"
    );
    match &want.document {
        Some(document) => {
            assert_eq!(
                &result["structuredContent"], document,
                "{context}: structuredContent"
            );
            if let Err(problem) = conforms(schema, &result["structuredContent"], "$") {
                panic!("{context}: structuredContent breaks its outputSchema: {problem}");
            }
        }
        None => assert!(
            result.get("structuredContent").is_none(),
            "{context}: structuredContent on exit 2: {result}"
        ),
    }
}

// ------------------------------------------------------------------ D1

/// D1, D6: both eras list the default build's twenty-two tools;
/// `review_proposal` takes exactly the required string `proposal_id`
/// (closed: no field carries a choice), its `_meta` is the size cap and
/// `requiresUserInteraction: true`, its four hints `false`, its output
/// schema `get_proposal`'s; its description at most 2 048 bytes, ASCII,
/// with the determinism sentence and no stack word; `INSTRUCTIONS` name
/// it. M1: `requiresUserInteraction` dropped. M2: an `option` argument.
#[test]
fn d1_review_proposal_is_listed_with_one_input_the_consent_meta_and_get_proposals_schema() {
    for era in ERAS {
        let mut session = common::read::Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        if !cfg!(feature = "probes") {
            assert_eq!(tool_names(&list), TOOLS, "{era:?}: twenty-two tools");
        }
        assert_eq!(TOOLS.len(), 22);
        let review = tool(&list, "review_proposal");
        let input = &review["inputSchema"];
        assert_eq!(input["type"], json!("object"), "{era:?}: {input}");
        let properties: Vec<&String> = input["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        assert_eq!(properties, ["proposal_id"], "{era:?}: {input}");
        assert_eq!(
            input["properties"]["proposal_id"]["type"],
            json!("string"),
            "{era:?}: {input}"
        );
        assert!(
            input["properties"]["proposal_id"].get("enum").is_none(),
            "{era:?}: {input}"
        );
        assert_eq!(input["required"], json!(["proposal_id"]), "{era:?}");
        assert_eq!(input["additionalProperties"], json!(false), "{era:?}");
        assert_eq!(
            review["_meta"],
            json!({
                "anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS,
                META_KEY: true
            }),
            "{era:?}: _meta"
        );
        assert_eq!(
            review["annotations"],
            json!({"readOnlyHint": false, "destructiveHint": false,
                "idempotentHint": false, "openWorldHint": false}),
            "{era:?}: annotations"
        );
        let schemas = output_schemas(&list);
        assert_eq!(
            schemas["review_proposal"], schemas["get_proposal"],
            "{era:?}: get_proposal's output schema"
        );
        // No other tool asks the human on every call.
        for other in list["tools"].as_array().expect("tools") {
            if other["name"] != "review_proposal" {
                assert!(
                    other["_meta"].get(META_KEY).is_none(),
                    "{era:?}: {} asks the human",
                    other["name"]
                );
            }
        }
        let description = review["description"].as_str().expect("description");
        assert!(description.len() <= 2048, "{era:?}: {}", description.len());
        assert!(description.is_ascii(), "{era:?}: {description}");
        assert!(
            description.contains("Deterministic: one state, one result; no LLM inside."),
            "{era:?}: {description}"
        );
        for word in common::read::STACK_WORDS {
            assert!(
                !common::read::has_word(description, word),
                "{era:?}: {word} in {description}"
            );
        }
        // A bad argument: an unknown name (a choice), a wrong type, none.
        for args in [
            json!({"proposal_id": "PR-0001", "option": 1}),
            json!({"proposal_id": "PR-0001", "note": "ok"}),
            json!({"proposal_id": 1}),
            json!({}),
        ] {
            let reply = session.call("review_proposal", args.clone());
            common::read::assert_bad_arguments(&reply, "", &format!("{era:?} {args}"));
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert!(
        common::read::has_word(instructions, "review_proposal"),
        "INSTRUCTIONS name the tool"
    );
    assert!(instructions.len() <= 2048 && instructions.is_ascii());
}

// ------------------------------------------------------------------ D2

/// D2: before any form — no `elicitation/create` (legacy), no
/// `input_required` (stateless) — an ID that is malformed, a look-alike,
/// unknown, of a rejected or an applied proposal, of another
/// repository's, of a drift item: the CLI's read refusing it
/// (`stage_form`: content, `isError`, document), nothing staged, no
/// event. M6: the form sent before the not-open or drift check.
#[test]
fn d2_refusals_answer_before_any_form_and_stage_nothing() {
    let repo = Repo::new("rf-d2");
    let rejected = repo.ask();
    repo.rejected(&rejected, "asked before");
    let applied = repo.report();
    repo.applied(&applied, 1);
    // Another repository of the project: a clone of the same fixture, its
    // own git, the same `HOME` (one queue per slug).
    let other_scratch = Scratch::new("rf-d2-other");
    let other_git = Sandbox::new(other_scratch.path());
    let other_main = other_scratch.copy("spec-a", "main");
    other_git.init(&other_main);
    other_git.add_all(&other_main);
    other_git.commit(&other_main, "the other clone");
    let elsewhere = {
        let outcome = propose_question(
            &cli_env(&other_main, Some(&repo.home)),
            &Globals::default(),
            &QuestionRequest {
                node_ids: vec!["EDGE-STAM-ZERO".to_owned()],
                text: "Is zero stamina a sprint stop elsewhere?".to_owned(),
                working_answer: "yes".to_owned(),
                price_of_other: "a rule".to_owned(),
                severity: None,
                distinct_from: Vec::new(),
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: GitEnv::new(&other_main, other_git.vars()),
            },
        )
        .expect("asked elsewhere");
        outcome
            .document
            .id
            .clone()
            .unwrap_or_else(|| panic!("stored elsewhere: {:?}", outcome.document.hits))
    };
    // An orphan: raised in a third clone, which is then removed.
    let orphan = {
        let gone_scratch = Scratch::new("rf-d2-gone");
        let gone_git = Sandbox::new(gone_scratch.path());
        let gone_main = gone_scratch.copy("spec-a", "main");
        gone_git.init(&gone_main);
        gone_git.add_all(&gone_main);
        gone_git.commit(&gone_main, "the gone clone");
        let outcome = propose_question(
            &cli_env(&gone_main, Some(&repo.home)),
            &Globals::default(),
            &QuestionRequest {
                node_ids: vec!["MEC-STAMINA".to_owned()],
                text: "Does a clone that is gone keep its question?".to_owned(),
                working_answer: "yes".to_owned(),
                price_of_other: "a rule".to_owned(),
                severity: None,
                distinct_from: vec!["DEC-0023".to_owned()],
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: GitEnv::new(&gone_main, gone_git.vars()),
            },
        )
        .expect("asked in the gone clone");
        outcome
            .document
            .id
            .clone()
            .unwrap_or_else(|| panic!("stored: {:?}", outcome.document.hits))
    };
    let open = repo.propose_edit("EDGE-STAM-ZERO", "immediately", "at once");
    let too_long = format!("P{}", "0".repeat(64));
    let ids = [
        "",
        "1-P",
        "PR 0001",
        too_long.as_str(),
        // CYRILLIC CAPITAL LETTER ER in place of the Latin P.
        "\u{0420}R-0001",
        "PR-0099",
        "PR-9999",
        rejected.as_str(),
        applied.as_str(),
        elsewhere.as_str(),
        orphan.as_str(),
    ];
    let events = repo.events();
    let state = repo.state();
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let schemas = output_schemas(&client.tools());
        for id in ids {
            let answer = client.refused(id);
            let want = read_before_form(&repo, &repo.home, id);
            assert!(want.is_error, "{era:?} {id:?}: the twin refuses: {want:?}");
            assert_same(
                &answer,
                &want,
                &schemas["review_proposal"],
                &format!("{era:?} {id:?}"),
            );
        }
        // An open proposal still asks in the same session.
        client.form_of(&open);
        let done = client.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert!(done.messages.is_empty(), "{era:?}: {:?}", done.messages);
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.state(), state, "the repository as it was");
    for id in [&rejected, &applied, &open] {
        assert_eq!(repo.stage_of(id), (Value::Null, Value::Null), "{id}");
    }
    // The CLI's words (canon "Before the form").
    for (id, words) in [
        (
            applied.as_str(),
            "is applied: only an open proposal takes a staged choice (commit ",
        ),
        (
            rejected.as_str(),
            "is rejected: only an open proposal takes a staged choice; nothing changed",
        ),
        (
            elsewhere.as_str(),
            "belongs to another repository of the project",
        ),
        (orphan.as_str(), "which no longer exists"),
        (
            "\u{0420}R-0001",
            "proposal IDs are Latin only: write `PR-0001`",
        ),
    ] {
        let text = read_before_form(&repo, &repo.home, id).text;
        assert!(text.contains(words), "{id}: {words}: {text}");
    }
}

/// D2 for a drift item (`fixtures/code-a`, its golden lock, `CAP` edited:
/// `LAW-BOUND` drifts), raised by `report_drift`: decided on a terminal
/// only, before any form; nothing staged, no event. M6.
#[test]
fn d2_a_drift_item_is_refused_before_any_form() {
    let scratch = Scratch::new("rf-d2-drift");
    let git = Sandbox::new(scratch.path());
    let main = scratch.copy("code-a", "main");
    let _ = fs::remove_file(main.join("expected.json"));
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the fixture");
    copy_file(
        fixture("spec-lock").join("code-a.lock"),
        main.join("spec.lock"),
    );
    git.add_all(&main);
    git.commit(&main, "the lock");
    replace(
        &main,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    git.add_all(&main);
    git.commit(&main, "the cap");
    let home = scratch.home("h");
    let repo = Repo {
        linked: main.clone(),
        main,
        home,
        git,
        scratch,
    };
    let mut client = Client::form(Era::Legacy, &repo);
    let reply = client.call("report_drift", json!({"author_role": "tester"}));
    let drift = result(&reply)["structuredContent"]["created"][0]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("a drift item: {reply}"))
        .to_owned();
    drop(client.finish());
    let events = repo.events();
    let state = repo.state();
    let want = read_before_form(&repo, &repo.home, &drift);
    assert!(want.is_error, "{want:?}");
    assert!(
        want.text
            .contains("decided on a terminal only; nothing staged"),
        "{}",
        want.text
    );
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let schemas = output_schemas(&client.tools());
        let answer = client.refused(&drift);
        assert_same(
            &answer,
            &want,
            &schemas["review_proposal"],
            &format!("{era:?} drift"),
        );
        drop(client.finish());
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.state(), state, "the repository as it was");
}

// ------------------------------------------------------- the owner's form

/// An `accept` of `choice`, with `note` when given (canon "Answer":
/// exactly `{choice, note?}`).
fn choosing(choice: &str, note: Option<&str>) -> Value {
    let mut content = json!({"choice": choice});
    if let Some(note) = note {
        content["note"] = json!(note);
    }
    json!({"action": "accept", "content": content})
}

/// The outcome lines above an accepted form's read: the tool's line and
/// the terminal's staged line (`staged <at>: <command>`).
fn staged_lines(id: &str, staged: &str) -> String {
    format!(
        "review_proposal {id}: the owner staged a choice; it is no answer until the owner \
         confirms it on a terminal.\n{staged}"
    )
}

/// The outcome line above a declined form's read.
fn declined_line(id: &str) -> String {
    format!(
        "review_proposal {id}: the owner declined; nothing staged. Do not call review_proposal \
         for {id} again unless the owner asks."
    )
}

/// The outcome line above a dismissed (form-cancelled) form's read.
fn dismissed_line(id: &str) -> String {
    format!(
        "review_proposal {id}: the owner dismissed the form; nothing staged. Do not call \
         review_proposal for {id} again unless the owner asks."
    )
}

/// The tool's own line for a client without form elicitation.
fn no_form_line(id: &str) -> String {
    format!(
        "review_proposal {id}: this client declares no form elicitation, so the owner cannot be \
         asked here; nothing staged. The owner decides on a terminal: spec approve {id} or spec \
         reject {id}.\n"
    )
}

/// The tool's own line for an accepted answer that is not the form's.
fn bad_answer_line(id: &str) -> String {
    format!(
        "review_proposal {id}: the form's answer is not {{choice, note}} as the form asked; \
         nothing staged.\n"
    )
}

/// `lines` above `spec review PR --brief` read now: content, `isError`,
/// `structuredContent` (conforming to `schema`).
fn assert_read_after(
    repo: &Repo,
    id: &str,
    lines: &str,
    result: &Value,
    schema: &Value,
    context: &str,
) {
    let mut want = repo.brief(id);
    want.text = format!("{lines}\n{}", want.text);
    assert!(!want.is_error, "{context}: the read answers");
    assert_same(result, &want, schema, context);
}

/// The tool's own error: `line`, `isError`, no `structuredContent`.
fn assert_own_error(result: &Value, line: &str, context: &str) {
    assert_eq!(result["isError"], json!(true), "{context}: {result}");
    assert_eq!(content_text(result), line, "{context}");
    assert!(
        result.get("structuredContent").is_none(),
        "{context}: {result}"
    );
}

/// The stored `staged` of an approve: the terminal flags as the queue
/// keeps them.
fn approve_row(
    option: Option<u64>,
    note: Option<&str>,
    editorial: Option<bool>,
    span: Option<&str>,
) -> Value {
    json!({"decision": "approve", "option": option, "answer": null, "canon": null,
        "editorial": editorial, "note": note, "span_hash": span})
}

/// `staged_at` of `id`, which must be staged.
fn staged_at(repo: &Repo, id: &str) -> String {
    repo.stage_of(id)
        .1
        .as_str()
        .unwrap_or_else(|| panic!("{id} is staged"))
        .to_owned()
}

// ------------------------------------------------------------------ D3

/// D3: in each era the owner's `accept` of `option-1` with a note on a
/// discrepancy stages exactly what the library's `stage` stores for
/// `spec approve PR --option 1 --note T` (a twin `HOME`, the same
/// proposal): the same row, one `proposal.staged` with the twin's payload
/// keys; a bare `spec approve PR` shows the result's staged line `staged
/// <at>: spec approve PR --option 1 --note "T"` (declined: kept); the
/// result is the outcome lines above `spec review PR --brief` read after
/// the stage, its document the `structuredContent`; the repository
/// untouched, the item `open`. M7: `approve` in place of `stage`.
#[test]
fn d3_an_accepted_option_and_note_stage_as_the_terminal_flags() {
    let note = "keep the \"cap\" \\ as is";
    let shown_note = "\"keep the \\\"cap\\\" \\\\ as is\"";
    for era in ERAS {
        let repo = Repo::new(&format!("rf-d3-{era:?}"));
        let id = repo.report();
        let twin = repo.twin_home("twin");
        assert_eq!(
            repo.report_in(&twin, discrepancy(SUMMARY)),
            id,
            "the twin's ID"
        );
        let state = repo.state();
        let events = repo.events();
        let mut client = Client::form(era, &repo);
        let schemas = output_schemas(&client.tools());
        let (form, answer) = client.review(&id, choosing("option-1", Some(note)));
        let at = staged_at(&repo, &id);
        let staged_line = format!("staged {at}: spec approve {id} --option 1 --note {shown_note}");
        assert_read_after(
            &repo,
            &id,
            &staged_lines(&id, &staged_line),
            &answer,
            &schemas["review_proposal"],
            &format!("{era:?}"),
        );
        assert!(
            form["message"].as_str().is_some_and(|m| m.contains(&id)),
            "{era:?}: {form}"
        );
        let (staged, at_value) = repo.stage_of(&id);
        assert_eq!(
            staged,
            approve_row(Some(1), Some(note), None, None),
            "{era:?}"
        );
        assert!(at.len() == 20 && at.ends_with('Z'), "{era:?}: {at}");
        // The twin: the library's stage of the same flags.
        let twin_at = repo.document_in(&twin, &id)["updated_at"]
            .as_str()
            .expect("updated_at")
            .to_owned();
        let outcome = repo.stage_in(
            &twin,
            &id,
            approve_body(Some(1), Some(note)),
            twin_at,
            LATER,
        );
        assert_eq!(outcome.cause, None, "{era:?}: {outcome:?}");
        assert_eq!(
            repo.document_in(&twin, &id)["staged"],
            staged,
            "{era:?}: the twin's row"
        );
        // One event, the twin's keys.
        let after = repo.events();
        assert_eq!(after.len(), events.len() + 1, "{era:?}: one event");
        assert_eq!(after[..events.len()], events[..], "{era:?}: appended");
        let (kind, payload) = after.last().expect("the event");
        assert_eq!(kind, "proposal.staged", "{era:?}");
        assert_eq!(repo.staged_events(&id).len(), 1, "{era:?}: one stage");
        assert_eq!(payload["id"], json!(id), "{era:?}");
        assert_eq!(payload["staged"], staged, "{era:?}: {payload}");
        assert_eq!(payload["staged_at"], at_value, "{era:?}: {payload}");
        let twin_event = repo
            .events_in(&twin)
            .into_iter()
            .rev()
            .find(|(kind, _)| kind == "proposal.staged")
            .expect("the twin's event")
            .1;
        let keys = |value: &Value| -> Vec<String> {
            value.as_object().expect("object").keys().cloned().collect()
        };
        assert_eq!(
            keys(payload),
            keys(&twin_event),
            "{era:?}: the payload's keys"
        );
        // A bare `spec approve PR` shows the same line; the owner
        // declines: kept.
        let questions = repo.bare_approve_questions(&id);
        assert_eq!(questions.len(), 1, "{era:?}: {questions:?}");
        assert!(
            questions[0].contains(&format!("{staged_line}\n")),
            "{era:?}: {}",
            questions[0]
        );
        assert_eq!(repo.stage_of(&id), (staged, at_value), "{era:?}: kept");
        assert_eq!(repo.document(&id)["status"], json!("open"), "{era:?}");
        assert_eq!(
            repo.state(),
            state,
            "{era:?}: nothing under the root, no commit"
        );
        let done = client.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert!(done.messages.is_empty(), "{era:?}: {:?}", done.messages);
    }
}

/// D3: a question's `reject` with its note stages `spec reject PR
/// --reason R` (a bare `spec reject PR` shows it); an update's
/// `editorial` with a note stages `--note N --editorial` and the target's
/// span hash, as the library's stage does; a question's `working-answer`
/// stages a bare `spec approve PR`; a second accepted form replaces the
/// stage, one more event; a blank note is none. Both eras.
#[test]
fn d3_a_reject_an_editorial_approve_and_the_working_answer_stage_their_flags() {
    for era in ERAS {
        let repo = Repo::new(&format!("rf-d3b-{era:?}"));
        let question = repo.ask();
        let update =
            repo.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "the sprint stops;");
        let working = repo.ask_on("EDGE-STAM-ZERO", "Does zero stamina stop a walk too?");
        let (span, _) = repo.span("EDGE-SPRINT-EMPTY");
        let state = repo.state();
        let mut client = Client::form(era, &repo);
        let schemas = output_schemas(&client.tools());
        let schema = &schemas["review_proposal"];

        let (_, answer) = client.review(&question, choosing("reject", Some("asked before")));
        let at = staged_at(&repo, &question);
        let line = format!("staged {at}: spec reject {question} --reason \"asked before\"");
        assert_read_after(
            &repo,
            &question,
            &staged_lines(&question, &line),
            &answer,
            schema,
            "reject",
        );
        assert_eq!(
            repo.stage_of(&question).0,
            json!({"decision": "reject", "reason": "asked before"})
        );
        let mut questions = Vec::new();
        let outcome = reject(
            &cli_env(&repo.main, Some(&repo.home)),
            &Globals::default(),
            &RejectRequest {
                id: question.clone(),
                reason: None,
                now: LATER.to_owned(),
                git: repo.git_env(&repo.main),
            },
            &mut |asked: &str| {
                questions.push(asked.to_owned());
                false
            },
        )
        .expect("reject");
        assert_eq!(outcome.exit(), Exit::NotFound, "declined: {outcome:?}");
        assert!(
            questions.len() == 1 && questions[0].contains(&format!("{line}\n")),
            "{era:?}: {questions:?}"
        );

        let (_, answer) = client.review(&update, choosing("editorial", Some("typo")));
        let at = staged_at(&repo, &update);
        let line = format!("staged {at}: spec approve {update} --note \"typo\" --editorial");
        assert_read_after(
            &repo,
            &update,
            &staged_lines(&update, &line),
            &answer,
            schema,
            "editorial",
        );
        assert_eq!(
            repo.stage_of(&update).0,
            approve_row(None, Some("typo"), Some(true), Some(&span)),
            "{era:?}"
        );
        let questions = repo.bare_approve_questions(&update);
        assert!(
            questions[0].contains(&format!("{line}\n")),
            "{era:?}: {questions:?}"
        );
        // A second form replaces the stage: `approve`, a blank note none.
        let before = repo.staged_events(&update).len();
        let (_, answer) = client.review(&update, choosing("approve", Some("  ")));
        let at = staged_at(&repo, &update);
        let line = format!("staged {at}: spec approve {update}");
        assert_read_after(
            &repo,
            &update,
            &staged_lines(&update, &line),
            &answer,
            schema,
            "approve",
        );
        assert_eq!(
            repo.stage_of(&update).0,
            approve_row(None, None, None, Some(&span)),
            "{era:?}: replaced whole"
        );
        assert_eq!(repo.staged_events(&update).len(), before + 1, "{era:?}");

        let (_, answer) = client.review(&working, choosing("working-answer", None));
        let at = staged_at(&repo, &working);
        let line = format!("staged {at}: spec approve {working}");
        assert_read_after(
            &repo,
            &working,
            &staged_lines(&working, &line),
            &answer,
            schema,
            "working",
        );
        assert_eq!(
            repo.stage_of(&working).0,
            approve_row(None, None, None, None),
            "{era:?}"
        );
        let questions = repo.bare_approve_questions(&working);
        assert!(
            questions[0].contains(&format!("{line}\n")),
            "{era:?}: {questions:?}"
        );
        assert_eq!(repo.state(), state, "{era:?}: nothing under the root");
        drop(client.finish());
    }
}

/// D3: what an `accept` gives that the terminal refuses as flags — a
/// reject with a blank or no note (no reason), a note over 4 096 bytes, an
/// option the item lacks, `editorial` or `working-answer` on a
/// discrepancy, an option on a question — is the stage's error result,
/// exactly the library's `stage` of those flags (`StageOutcome` rendered
/// as `spec`); an answer that is not `{choice, note?}` of strings with a
/// known choice is the tool's bad-answer line; nothing staged, no event.
/// Both eras.
#[test]
fn d3_an_answer_the_terminal_would_refuse_stages_nothing() {
    let repo = Repo::new("rf-d3-refused");
    let id = repo.report();
    let question = repo.ask();
    let update = repo.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "the sprint stops;");
    let events = repo.events();
    let long = "n".repeat(4097);
    let five_k = "f".repeat(5000);
    let read_at = |target: &str| repo.updated_at(target);
    type Body<'a> = Box<dyn FnOnce(String) -> StageBody + 'a>;
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let schemas = output_schemas(&client.tools());
        // (target, answer, the twin's flags, a word of its refusal, the
        // exit pinned: 2 the line(s) only, 1 the text and the brief
        // document).
        let cases: [(&str, Value, Body<'_>, &str, Option<u8>); 8] = [
            (
                &id,
                choosing("reject", Some("   ")),
                Box::new(reject_body("")),
                "--reason",
                Some(2),
            ),
            (
                &id,
                choosing("reject", None),
                Box::new(reject_body("")),
                "--reason",
                None,
            ),
            (
                &id,
                choosing("option-1", Some(&long)),
                Box::new(approve_body(Some(1), Some(&long))),
                "--note: 4097 bytes; at most 4096",
                None,
            ),
            (
                &id,
                choosing("option-7", None),
                Box::new(approve_body(Some(7), None)),
                "7",
                None,
            ),
            (
                &id,
                choosing("editorial", None),
                Box::new(move |updated_at| StageBody::Approve {
                    option: None,
                    answer: None,
                    canon: None,
                    editorial: Some(true),
                    note: None,
                    updated_at,
                }),
                "--editorial",
                None,
            ),
            (
                &id,
                choosing("working-answer", None),
                Box::new(approve_body(None, None)),
                "--option N",
                None,
            ),
            (
                &question,
                choosing("option-0", None),
                Box::new(approve_body(Some(0), None)),
                "--option",
                Some(2),
            ),
            // An applying kind over the cap: exit 1, its brief document
            // (the full one carries the diff).
            (
                &update,
                choosing("approve", Some(&five_k)),
                Box::new(approve_body(None, Some(&five_k))),
                "--note: 5000 bytes; at most 4096",
                Some(1),
            ),
        ];
        for (target, answer, body, needle, exit) in cases {
            let context = format!("{era:?} {target} {}", clip(&answer.to_string()));
            let (_, result) = client.review(target, answer.clone());
            let full = repo.full_refused_stage(target, body, read_at(target));
            let want = as_refusal(full.clone().map(StageOutcome::brief));
            assert!(want.text.contains(needle), "{context}: {}", want.text);
            assert_same(&result, &want, &schemas["review_proposal"], &context);
            match exit {
                Some(2) => {
                    assert!(
                        result.get("structuredContent").is_none(),
                        "{context}: exit 2 has no document: {result}"
                    );
                    let text = content_text(&result);
                    assert!(
                        text.starts_with("spec: ")
                            && text.ends_with('\n')
                            && text.lines().count() == 1,
                        "{context}: exit 2 is the line only: {text:?}"
                    );
                }
                Some(1) => {
                    let document = &result["structuredContent"];
                    assert!(document.is_object(), "{context}: exit 1 has its document");
                    assert_eq!(document["id"], json!(target), "{context}");
                    let full = as_refusal(full);
                    assert_ne!(
                        full.document.as_ref(),
                        Some(document),
                        "{context}: the document is the brief one, not the full review's"
                    );
                    assert!(
                        full.document
                            .as_ref()
                            .is_some_and(|full| !full["diff"].is_null()),
                        "{context}: the full review carries the diff"
                    );
                }
                _ => {}
            }
        }
        for content in [
            json!({"choice": "maybe"}),
            json!({"choice": "option-x"}),
            json!({"choice": "option-"}),
            json!({"choice": "option-1", "note": 7}),
            json!({"choice": 1}),
            json!({"note": "no choice"}),
            json!({"choice": "option-1", "option": "1"}),
            json!({"choice": "approve", "verdict": "approve"}),
            json!(null),
        ] {
            let (_, result) = client.review(&id, json!({"action": "accept", "content": content}));
            assert_own_error(
                &result,
                &bad_answer_line(&id),
                &format!("{era:?} {content}"),
            );
        }
        drop(client.finish());
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.stage_of(&id), (Value::Null, Value::Null));
    assert_eq!(repo.stage_of(&question), (Value::Null, Value::Null));
    assert_eq!(repo.stage_of(&update), (Value::Null, Value::Null));
}

/// D3 (the review's last fix): a usage refusal after the form is its
/// line alone, never briefed. An update whose text cites 21 undeclared IDs
/// (`R-70` to `R-90`: 21 introduced findings, one over the brief cap, so a
/// brief answer adds `1 more introduced finding(s): spec review PR`),
/// rejected with a blank note: exactly the CLI's one `spec:` line, no
/// `structuredContent`, nothing staged, no event. Both eras. M: the exit-2
/// refusal briefed (the note line before it).
#[test]
fn d3_a_usage_refusal_of_an_item_with_many_findings_is_its_line_alone() {
    let repo = Repo::new("rf-d3-many");
    let cited: Vec<String> = (70..=90).map(|n| format!("R-{n}")).collect();
    let id = repo.propose_edit(
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        &format!("the sprint ends ({});", cited.join(", ")),
    );
    // More than the brief cap is stored: `review --brief` keeps 20 and
    // names the rest.
    let brief = repo.document(&id);
    assert_eq!(brief["kind"], json!("update"));
    assert_eq!(
        brief["diagnostics"].as_array().map(Vec::len),
        Some(20),
        "{brief}"
    );
    let more = format!("1 more introduced finding(s): spec review {id}");
    assert!(
        brief["notes"]
            .as_array()
            .is_some_and(|notes| notes.iter().any(|note| note == &json!(more))),
        "{brief}"
    );
    let line = "spec: --reason is empty: say why the proposal is rejected\n";
    let full = repo.full_refused_stage(&id, reject_body(""), repo.updated_at(&id));
    let want = as_refusal(full.clone());
    assert_eq!(want.text, line, "the CLI's line");
    assert!(want.document.is_none());
    // The case discriminates: briefed, the answer would gain the note.
    let briefed = as_refusal(full.map(StageOutcome::brief));
    assert!(
        briefed.text.contains(&more) && briefed.text.lines().count() == 2,
        "{briefed:?}"
    );
    let events = repo.events();
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let (_, result) = client.review(&id, choosing("reject", Some("   ")));
        assert_own_error(&result, line, &format!("{era:?}"));
        drop(client.finish());
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.stage_of(&id), (Value::Null, Value::Null));
}

// ------------------------------------------------------------------ D4

/// D4: a declined and a dismissed (form `cancel`) form stage nothing, log
/// no event: the result is the line saying so above `spec review PR
/// --brief` read then. Both eras. M3: staging on decline.
#[test]
fn d4_a_declined_or_dismissed_form_stages_nothing() {
    let repo = Repo::new("rf-d4");
    let id = repo.report();
    let events = repo.events();
    let state = repo.state();
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let schemas = output_schemas(&client.tools());
        for (answer, line) in [
            (json!({"action": "decline"}), declined_line(&id)),
            (json!({"action": "cancel"}), dismissed_line(&id)),
            // A declined form's content, were a client to send one, is
            // never read.
            (
                json!({"action": "decline", "content": {"choice": "option-1"}}),
                declined_line(&id),
            ),
            (
                json!({"action": "cancel", "content": {"choice": "reject", "note": "x"}}),
                dismissed_line(&id),
            ),
        ] {
            let (_, result) = client.review(&id, answer.clone());
            assert_read_after(
                &repo,
                &id,
                &line,
                &result,
                &schemas["review_proposal"],
                &format!("{era:?} {answer}"),
            );
        }
        drop(client.finish());
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.stage_of(&id), (Value::Null, Value::Null));
    assert_eq!(repo.state(), state);
}

/// D4: a client that declares no form elicitation (none at all, or URL
/// mode only) is never sent a form: the tool's no-form line; no event.
/// Both eras. An empty `elicitation` (the shape before 2025-11-25) is
/// form mode. M5: the capability check skipped.
#[test]
fn d4_a_client_without_form_elicitation_gets_no_form() {
    let repo = Repo::new("rf-d4-caps");
    let id = repo.report();
    let events = repo.events();
    for era in ERAS {
        for capabilities in [json!({}), json!({"elicitation": {"url": {}}})] {
            let mut client = Client::open(era, &repo, capabilities.clone());
            let answer = client.refused(&id);
            assert_own_error(
                &answer,
                &no_form_line(&id),
                &format!("{era:?} {capabilities}"),
            );
            let done = client.finish();
            assert!(done.status.success(), "{era:?}: {}", done.stderr);
            assert!(done.messages.is_empty(), "{era:?}: {:?}", done.messages);
        }
        let mut client = Client::open(era, &repo, json!({"elicitation": {}}));
        let form = client.form_of(&id);
        assert!(form["requestedSchema"].is_object(), "{era:?}: {form}");
        drop(client.finish());
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.stage_of(&id), (Value::Null, Value::Null));
}

/// Pings until a message satisfying `found` arrives (at most `wait`),
/// every other message kept: whether it came, and every message read.
fn wait_for(
    client: &mut Client,
    wait: Duration,
    found: impl Fn(&Value) -> bool,
) -> (bool, Vec<Value>) {
    let start = Instant::now();
    let mut seen = Vec::new();
    loop {
        let id = client.id();
        client
            .server
            .send(&json!({"jsonrpc": "2.0", "id": id, "method": "ping"}));
        loop {
            let message = client.server.recv();
            let answered = message["id"] == json!(id) && message.get("method").is_none();
            seen.push(message);
            if answered {
                break;
            }
        }
        if seen.iter().any(&found) {
            return (true, seen);
        }
        if start.elapsed() > wait {
            return (false, seen);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// D4 (R5): a legacy call cancelled while its form is open withdraws the
/// form — `notifications/cancelled` naming the `elicitation/create` —
/// stages nothing and gets no answer; the owner's late `accept` of the
/// withdrawn form stages nothing either, no event. M8: no
/// `notifications/cancelled`.
#[test]
fn d4_a_cancelled_legacy_call_withdraws_its_form_and_stages_nothing() {
    let repo = Repo::new("rf-d4-cancel");
    let id = repo.report();
    let events = repo.events();
    let mut client = Client::form(Era::Legacy, &repo);
    let (call, ask) = client.legacy_ask(&id);
    client.server.notify(
        "notifications/cancelled",
        Some(json!({"requestId": call, "reason": "the user pressed Esc"})),
    );
    let withdrawn = |message: &Value| {
        message["method"] == json!("notifications/cancelled")
            && message["params"]["requestId"] == ask["id"]
    };
    let (found, seen) = wait_for(&mut client, Duration::from_secs(5), withdrawn);
    assert!(found, "no notifications/cancelled for the form: {seen:?}");
    // The owner answers the withdrawn form anyway.
    client.server.send(&json!({
        "jsonrpc": "2.0", "id": ask["id"],
        "result": choosing("option-1", Some("late"))
    }));
    let (_, later) = wait_for(&mut client, Duration::from_millis(500), |_| false);
    // The session goes on: another form opens and is dismissed.
    client.form_of(&id);
    let done = client.finish();
    assert!(done.status.success(), "{}", done.stderr);
    for message in seen.iter().chain(&later).chain(&done.messages) {
        assert!(
            message["id"] != json!(call) || message.get("method").is_some(),
            "the cancelled call was answered: {message}"
        );
        assert_ne!(
            message["method"],
            json!("elicitation/create"),
            "a second form: {message}"
        );
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.stage_of(&id), (Value::Null, Value::Null));
}

/// D4 (iteration 2): a legacy call cancelled before its form goes out —
/// the client's `notifications/cancelled` right behind the call, while the
/// read before the form runs — sends no `elicitation/create` and gets no
/// answer; nothing staged, no event; the session goes on.
#[test]
fn d4_a_call_cancelled_before_its_form_sends_none() {
    let repo = Repo::new("rf-d4-early");
    let id = repo.report();
    let events = repo.events();
    let mut client = Client::form(Era::Legacy, &repo);
    let call = client.id();
    client.server.send(&json!({
        "jsonrpc": "2.0", "id": call, "method": "tools/call",
        "params": {"name": "review_proposal", "arguments": {"proposal_id": id}}
    }));
    client.server.notify(
        "notifications/cancelled",
        Some(json!({"requestId": call, "reason": "the user pressed Esc"})),
    );
    let (_, seen) = wait_for(&mut client, Duration::from_millis(1500), |_| false);
    // The session goes on: a form for the same proposal, dismissed.
    client.form_of(&id);
    let done = client.finish();
    assert!(done.status.success(), "{}", done.stderr);
    for message in seen.iter().chain(&done.messages) {
        assert_ne!(
            message["method"],
            json!("elicitation/create"),
            "a form for a cancelled call: {message}"
        );
        assert!(
            message["id"] != json!(call) || message.get("method").is_some(),
            "the cancelled call was answered: {message}"
        );
    }
    assert_eq!(repo.events(), events, "no event");
    assert_eq!(repo.stage_of(&id), (Value::Null, Value::Null));
}

// ------------------------------------------------------------------ D5

/// D5 (legacy): the proposal changes while its form is open — the
/// library stages a reject, so its `updated_at` moves — and the owner's
/// `accept` then is the CLI's "changed since it was read" refusal (the
/// library's stage against the `updated_at` read before the form):
/// nothing of the form's staged, the library's stage kept, its one event.
#[test]
fn d5_legacy_a_proposal_changed_while_its_form_is_open_is_refused() {
    // An applying kind: its brief document and the full review's differ
    // (the diff), so the refusal's document is pinned brief.
    let repo = Repo::new("rf-d5-legacy");
    let id = repo.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "the sprint stops;");
    let read = repo.updated_at(&id);
    let mut client = Client::form(Era::Legacy, &repo);
    let schemas = output_schemas(&client.tools());
    let (call, ask) = client.legacy_ask(&id);
    repo.stage(&id, reject_body("meanwhile"), LATER);
    let events = repo.events();
    let kept = repo.stage_of(&id);
    assert_eq!(kept.0["reason"], json!("meanwhile"));
    let answer = client.legacy_reply(call, &ask, choosing("approve", Some("now")));
    let full = repo.full_refused_stage(&id, approve_body(None, Some("now")), read);
    let want = as_refusal(full.clone().map(StageOutcome::brief));
    assert!(
        want.text.contains("changed since it was read"),
        "{}",
        want.text
    );
    assert!(want.is_error && want.document.is_some(), "exit 1: {want:?}");
    assert_same(&answer, &want, &schemas["review_proposal"], "legacy");
    let full = as_refusal(full).document.expect("the full review");
    assert!(!full["diff"].is_null(), "the full review carries the diff");
    assert_ne!(
        answer["structuredContent"], full,
        "the document is the brief one"
    );
    assert_eq!(repo.stage_of(&id), kept, "the library's stage kept");
    assert_eq!(repo.events(), events, "no event of the form's");
    drop(client.finish());
}

/// D5 (stateless, A3): round 1's `requestState` is the compact JSON
/// `{"proposal_id", "updated_at"}` of the read before the form, unsealed
/// (no key, no randomness: round 1 again, and in another process, is
/// byte-identical); round 2 stages against it, never a re-read: the
/// proposal changed between the rounds is the CLI's "changed since it was
/// read" refusal, nothing of the form's staged; a fresh round 1 reads the
/// new one and stages. -32602 for a state of another proposal or not those
/// two keys, and for a missing or malformed answer. M4: round 2 re-reads
/// `updated_at`.
#[test]
fn d5_stateless_round_two_stages_against_round_ones_updated_at() {
    let repo = Repo::new("rf-d5-stateless");
    let id = repo.report();
    let other = repo.ask();
    let read = repo.updated_at(&id);
    let mut client = Client::form(Era::Stateless, &repo);
    let schemas = output_schemas(&client.tools());
    let first = client.round1(&id);
    let state = request_state(&first);
    assert_eq!(
        state,
        format!("{{\"proposal_id\":\"{id}\",\"updated_at\":\"{read}\"}}"),
        "requestState"
    );
    let (key, _) = input_request(&first);
    assert_eq!(key, "owner_review");
    // One state, one result: again, and in a new process.
    let line = |client: &mut Client| {
        let id_now = client.id();
        let params = client.params(json!({"name": "review_proposal",
            "arguments": {"proposal_id": id}}));
        client
            .server
            .request_line(id_now, "tools/call", Some(params))
    };
    let once = line(&mut client);
    let again = line(&mut client);
    let mut fresh = Client::form(Era::Stateless, &repo);
    let elsewhere = line(&mut fresh);
    drop(fresh.finish());
    let strip_id = |text: &str| {
        let mut value: Value = serde_json::from_str(text).expect("JSON");
        value["id"] = Value::Null;
        value.to_string()
    };
    assert_eq!(strip_id(&once), strip_id(&again), "round 1 again");
    assert_eq!(
        strip_id(&once),
        strip_id(&elsewhere),
        "round 1 in another process"
    );

    let accept = choosing("option-1", Some("now"));
    let not_this =
        "requestState is not this call's: call review_proposal again without requestState";
    for bad in [
        format!("{{\"proposal_id\":\"{other}\",\"updated_at\":\"{read}\"}}"),
        format!("{{\"proposal_id\":\"{id}\"}}"),
        format!("{{\"proposal_id\":\"{id}\",\"updated_at\":\"{read}\",\"x\":1}}"),
        format!("{{\"proposal_id\":\"{id}\",\"updated_at\":7}}"),
        "rs1.AAAA.BBBB".to_owned(),
    ] {
        let reply = client.round2(&id, Some(&bad), Some(json!({key.clone(): accept.clone()})));
        assert_eq!(error_code(&reply), INVALID_PARAMS, "{bad}: {reply}");
        assert_eq!(reply["error"]["message"], json!(not_this), "{bad}");
    }
    let reply = client.round2(&id, Some(&state), None);
    assert_eq!(
        error_code(&reply),
        INVALID_PARAMS,
        "no inputResponses: {reply}"
    );
    assert_eq!(
        reply["error"]["message"],
        json!("inputResponses.owner_review is missing from the retried call")
    );
    let reply = client.round2(&id, Some(&state), Some(json!({"another": accept.clone()})));
    assert_eq!(
        reply["error"]["message"],
        json!("inputResponses.owner_review is missing from the retried call")
    );
    let reply = client.round2(
        &id,
        Some(&state),
        Some(json!({key.clone(): {"choice": "reject"}})),
    );
    assert_eq!(error_code(&reply), INVALID_PARAMS, "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("inputResponses.owner_review is not an elicitation result")
    );
    assert_eq!(
        repo.stage_of(&id),
        (Value::Null, Value::Null),
        "none staged"
    );

    repo.stage(&id, reject_body("meanwhile"), LATER);
    let events = repo.events();
    let kept = repo.stage_of(&id);
    let reply = client.round2(
        &id,
        Some(&state),
        Some(json!({key.clone(): accept.clone()})),
    );
    let want = repo.refused_stage(&id, approve_body(Some(1), Some("now")), read);
    assert!(
        want.text.contains("changed since it was read"),
        "{}",
        want.text
    );
    assert_same(
        result(&reply),
        &want,
        &schemas["review_proposal"],
        "stateless",
    );
    assert_eq!(repo.stage_of(&id), kept, "the library's stage kept");
    assert_eq!(repo.events(), events, "no event of the form's");

    // A fresh round 1 reads the new `updated_at`, and its answer stages.
    let first = client.round1(&id);
    let fresh_state = request_state(&first);
    assert_eq!(
        fresh_state,
        format!("{{\"proposal_id\":\"{id}\",\"updated_at\":\"{LATER}\"}}")
    );
    let reply = client.round2(&id, Some(&fresh_state), Some(json!({key: accept})));
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    assert_eq!(
        repo.stage_of(&id).0,
        approve_row(Some(1), Some("now"), None, None)
    );
    assert_eq!(repo.events().len(), events.len() + 1, "one event");
    drop(client.finish());
}

// ------------------------------------------------- the form's contents

/// The form's field names, sorted.
fn fields(form: &Value) -> Vec<String> {
    let mut names: Vec<String> = form["requestedSchema"]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("no fields: {form}"))
        .keys()
        .cloned()
        .collect();
    names.sort();
    names
}

/// `choice`'s `(const, title)` pairs.
fn choices(form: &Value) -> Vec<(String, String)> {
    form["requestedSchema"]["properties"]["choice"]["oneOf"]
        .as_array()
        .unwrap_or_else(|| panic!("choice is no titled single-select: {form}"))
        .iter()
        .map(|item| {
            (
                item["const"].as_str().expect("a const").to_owned(),
                item["title"].as_str().expect("a title").to_owned(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(value, title)| ((*value).to_owned(), (*title).to_owned()))
        .collect()
}

/// Every escaped character of `form`'s strings, raw: none may be (the
/// line and paragraph separators too, canon "Form").
fn raw_escaped(form: &Value) -> Vec<String> {
    let mut texts = Vec::new();
    strings_in(form, &mut texts);
    texts
        .iter()
        .flat_map(|text| {
            text.chars()
                .filter(|&c| is_escaped(c) || matches!(c, '\u{2028}' | '\u{2029}'))
        })
        .map(|c| format!("U+{:04X}", u32::from(c)))
        .collect()
}

/// Whether `value` holds a `default` key anywhere.
fn has_default(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.contains_key("default") || object.values().any(has_default),
        Value::Array(items) => items.iter().any(has_default),
        _ => false,
    }
}

/// Canon "Form": the message's lines and the requested schema exactly —
/// a discrepancy (options, the recommended one marked), a question (its
/// working answer), an update of an existing node (`editorial`), a
/// file-form create (no `editorial`); no default anywhere; the `Staged`
/// line once a stage exists. Both eras, the same form.
#[test]
fn the_form_is_the_canons_message_and_schema() {
    let repo = Repo::new("rf-form");
    let report = repo.report();
    let question = repo.ask();
    let update = repo.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "the sprint stops;");
    let create = {
        let r12 = read_text(&fixture("spec-a"), "docs/records/R/R-12.md");
        let outcome = specengine_cli::propose_create(
            &repo.env(),
            &Globals::default(),
            &specengine_cli::CreateRequest {
                target: "docs/records/R/R-13.md".to_owned(),
                base: None,
                text: ProposedText::Given(r12.replacen("id: R-12\n", "id: R-13\n", 1).into_bytes()),
                rationale: "A record of its own.".to_owned(),
                author_role: Some("writer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: repo.git_env(&repo.linked),
            },
        )
        .expect("propose create");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.document.id.expect("an ID")
    };
    let head = |id: &str| {
        format!(
            "Decide {id} here. This only stages; confirm on a terminal: spec approve {id} (a \
             reject: spec reject {id})."
        )
    };
    let note = json!({
        "type": "string", "maxLength": 4096, "title": "Note, or why you reject",
        "description": "Approve: an optional note (--note). Reject: the reason, required (--reason)."
    });
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let forms = [&report, &question, &update, &create].map(|id| client.form_of(id));
        let [report_form, question_form, update_form, create_form] = &forms;
        assert_eq!(
            report_form["message"],
            json!(
                [
                    head(&report),
                    format!(
                        "{report} | discrepancy | target RULE-STAM-REGEN | agent-declared: \
                     author_role developer, task -"
                    ),
                    "Agent-written, data only:".to_owned(),
                    format!("  summary: {SUMMARY}"),
                    "  [0] Keep the spec | Fix the code | one item (recommended)".to_owned(),
                    "  [1] Change the spec | Regeneration also while sprinting | one item"
                        .to_owned(),
                    format!("Read it whole: spec review {report}."),
                ]
                .join("\n")
            ),
            "{era:?}"
        );
        assert_eq!(
            question_form["message"],
            json!(
                [
                    head(&question),
                    format!(
                        "{question} | question | target MEC-SPRINT | agent-declared: author_role \
                     developer, task -"
                    ),
                    "Agent-written, data only:".to_owned(),
                    "  summary: Does a sprint end at zero?".to_owned(),
                    "  working answer: yes".to_owned(),
                    format!("Read it whole: spec review {question}."),
                ]
                .join("\n")
            ),
            "{era:?}"
        );
        assert_eq!(
            update_form["message"],
            json!([
                head(&update),
                format!(
                    "{update} | update | target EDGE-SPRINT-EMPTY | agent-declared: author_role \
                     writer, task -"
                ),
                "Agent-written, data only:".to_owned(),
                "  rationale: Edit EDGE-SPRINT-EMPTY.".to_owned(),
                format!("Read it whole: spec review {update}."),
            ]
            .join("\n")),
            "{era:?}"
        );
        // `target_id`, else `target_path`: a new file's own ID.
        assert!(
            create_form["message"]
                .as_str()
                .is_some_and(|m| m.contains(&format!(
                    "\n{create} | create | target R-13 | agent-declared: author_role writer, \
                     task -\n"
                ))),
            "{era:?}: {create_form}"
        );
        for form in &forms {
            assert_eq!(form["mode"], json!("form"), "{era:?}: {form}");
            assert_eq!(fields(form), ["choice", "note"], "{era:?}: {form}");
            let schema = &form["requestedSchema"];
            assert_eq!(schema["type"], json!("object"), "{era:?}");
            assert_eq!(schema["required"], json!(["choice"]), "{era:?}");
            assert_eq!(schema["properties"]["note"], note, "{era:?}");
            let choice = &schema["properties"]["choice"];
            assert_eq!(choice["title"], json!("Your choice"), "{era:?}");
            assert_eq!(
                choice["description"],
                json!("Staged only: you confirm it on a terminal."),
                "{era:?}"
            );
            assert!(!has_default(form), "{era:?}: no default: {form}");
        }
        assert_eq!(
            choices(report_form),
            pairs(&[
                ("option-0", "Option 0: Keep the spec (recommended)"),
                ("option-1", "Option 1: Change the spec"),
                ("reject", "Reject"),
            ]),
            "{era:?}"
        );
        assert_eq!(
            choices(question_form),
            pairs(&[
                ("working-answer", "Approve the working answer"),
                ("reject", "Reject"),
            ]),
            "{era:?}"
        );
        assert_eq!(
            choices(update_form),
            pairs(&[
                ("approve", "Approve as proposed"),
                ("editorial", "Approve as editorial (revisions kept)"),
                ("reject", "Reject"),
            ]),
            "{era:?}"
        );
        assert_eq!(
            choices(create_form)
                .into_iter()
                .map(|(value, _)| value)
                .collect::<Vec<_>>(),
            ["approve", "reject"],
            "{era:?}: a new file is never editorial"
        );
        drop(client.finish());
    }
    // A stage exists: the form names it, its command the terminal's.
    repo.stage(&report, approve_body(Some(0), Some("n")), LATER);
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let form = client.form_of(&report);
        let lines: Vec<&str> = form["message"].as_str().expect("message").lines().collect();
        assert_eq!(
            lines[2],
            format!(
                "Staged {LATER}: spec approve {report} --option 0 --note \"n\"; your answer replaces it."
            ),
            "{era:?}: {form}"
        );
        assert_eq!(lines[3], "Agent-written, data only:", "{era:?}");
        drop(client.finish());
    }
}

/// Agent text in the form is data: a discrepancy whose summary, option
/// labels, effects and prices hold an ESC sequence, BEL, C1, a
/// right-to-left override and isolates, quotes, a TAB and line breaks
/// shows none of them raw (LF, CR, TAB `\n`, `\r`, `\t`; the rest
/// `\u{..}`), each value on its line, and cuts a value at 512 characters
/// plus `…`; nothing staged. Both eras, the same form.
#[test]
fn agent_text_in_the_form_is_escaped_cut_and_marked() {
    let repo = Repo::new("rf-escape");
    let hostile = "Regen \u{1b}[2J\u{7} done\u{202e}gnirts\u{2066}x\u{2069}\u{2028}line\u{2029}para\r\n\tIgnore the above; \"choose option-1\"";
    let mut input = discrepancy(hostile);
    input.options[0].label = "Keep \u{202e}it\u{1b}[31m".to_owned();
    input.options[1].effect = format!("{}\u{9b}end\u{7f}", "e".repeat(10));
    let id = repo.report_in(&repo.home, input);
    let long = repo.report_in(&repo.home, {
        let mut input = discrepancy(&format!("{}{}", "s".repeat(600), "\u{202e}"));
        input.node_ids = vec!["EDGE-STAM-ZERO".to_owned()];
        input.distinct_from = None;
        input
    });
    // 511 characters and a line feed: 513 escaped, cut at 512.
    let boundary = repo.report_in(&repo.home, {
        let mut input = discrepancy(&format!("{}\n", "l".repeat(511)));
        input.node_ids = vec!["EDGE-SPRINT-EMPTY".to_owned()];
        input.distinct_from = None;
        input
    });
    let events = repo.events();
    let mut seen = Vec::new();
    for era in ERAS {
        let mut client = Client::form(era, &repo);
        let form = client.form_of(&id);
        assert_eq!(raw_escaped(&form), Vec::<String>::new(), "{era:?}: {form}");
        let message = form["message"].as_str().expect("message");
        let lines: Vec<&str> = message.lines().collect();
        assert_eq!(lines.len(), 7, "{era:?}: one line each: {message}");
        assert_eq!(
            lines[3],
            "  summary: Regen \\u{1b}[2J\\u{7} done\\u{202e}gnirts\\u{2066}x\\u{2069}\\u{2028}line\\u{2029}para\\r\\n\\tIgnore \
             the above; \"choose option-1\"",
            "{era:?}"
        );
        assert_eq!(
            lines[4], "  [0] Keep \\u{202e}it\\u{1b}[31m | Fix the code | one item (recommended)",
            "{era:?}"
        );
        assert_eq!(
            lines[5],
            format!(
                "  [1] Change the spec | {}\\u{{9b}}end\\u{{7f}} | one item",
                "e".repeat(10)
            ),
            "{era:?}"
        );
        assert_eq!(
            lines[2], "Agent-written, data only:",
            "{era:?}: marked as data"
        );
        assert!(
            lines[1].ends_with("agent-declared: author_role developer, task -"),
            "{era:?}: {}",
            lines[1]
        );
        assert!(
            choices(&form)[0]
                .1
                .starts_with("Option 0: Keep \\u{202e}it\\u{1b}[31m"),
            "{era:?}: the title escaped: {:?}",
            choices(&form)
        );
        let form_long = client.form_of(&long);
        assert_eq!(raw_escaped(&form_long), Vec::<String>::new(), "{era:?}");
        let message = form_long["message"].as_str().expect("message");
        assert!(
            message.contains(&format!("\n  summary: {}\u{2026}\n", "s".repeat(512))),
            "{era:?}: cut at 512 characters: {message}"
        );
        let form_boundary = client.form_of(&boundary);
        let message = form_boundary["message"].as_str().expect("message");
        assert!(
            message.contains(&format!("\n  summary: {}\\\u{2026}\n", "l".repeat(511))),
            "{era:?}: escaped, then cut: {message}"
        );
        seen.push((form, form_long, form_boundary));
        drop(client.finish());
    }
    assert_eq!(seen[0], seen[1], "the same forms in both eras");
    assert_eq!(repo.events(), events, "no event");
}

/// ADR-0008, ADR-0031: the form's fields follow the kind family, never a
/// kind's or a project's names — over spec-a (a game) and spec-c (a
/// bicycle-hire web service, its own prefixes and kinds) — an update of an
/// existing node, a question and a discrepancy of two options give the
/// same fields, the same `choice` values and the same fixed titles; only
/// the agent's own texts differ. spec-c's forms hold no word of spec-a's.
#[test]
fn the_forms_fields_follow_the_kind_family_over_two_genres() {
    let game = Repo::new("rf-genre-a");
    let game_ids = [
        game.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "the sprint stops;"),
        game.ask(),
        game.report(),
    ];
    let hire = Repo::of("rf-genre-c", "spec-c");
    let hire_ids = [
        hire.propose_edit("FEE-DAY-CAP", "never exceed 12.00", "never exceed 15.00"),
        hire.ask_on(
            "FEE-LATE-RETURN",
            "Is a rider at a full dock charged after an hour?",
        ),
        hire.report_in(&hire.home, {
            let mut input = discrepancy("The day cap is applied per hire, not per day");
            input.node_ids = vec!["FEE-DAY-CAP".to_owned()];
            input.evidence[0].file = "src/fees.py".to_owned();
            input.options[0].label = "Cap per day".to_owned();
            input.options[1].label = "Cap per hire".to_owned();
            input.distinct_from = None;
            input
        }),
    ];
    let forms = |repo: &Repo, ids: &[String; 3]| {
        let mut client = Client::form(Era::Legacy, repo);
        let forms = ids.clone().map(|id| client.form_of(&id));
        drop(client.finish());
        forms
    };
    let game_forms = forms(&game, &game_ids);
    let hire_forms = forms(&hire, &hire_ids);
    let fixed = |form: &Value| -> Vec<(String, String)> {
        choices(form)
            .into_iter()
            .map(|(value, title)| {
                if value.starts_with("option-") {
                    (value, "Option".to_owned())
                } else {
                    (value, title)
                }
            })
            .collect()
    };
    for (game, hire) in game_forms.iter().zip(&hire_forms) {
        assert_eq!(fields(game), fields(hire), "{game}\n{hire}");
        assert_eq!(fixed(game), fixed(hire), "{game}\n{hire}");
        assert_eq!(
            game["requestedSchema"]["properties"]["note"],
            hire["requestedSchema"]["properties"]["note"]
        );
        let mut game_choice = game["requestedSchema"]["properties"]["choice"].clone();
        let mut hire_choice = hire["requestedSchema"]["properties"]["choice"].clone();
        game_choice["oneOf"] = Value::Null;
        hire_choice["oneOf"] = Value::Null;
        assert_eq!(game_choice, hire_choice);
    }
    assert_eq!(
        choices(&hire_forms[2]),
        pairs(&[
            ("option-0", "Option 0: Cap per day (recommended)"),
            ("option-1", "Option 1: Cap per hire"),
            ("reject", "Reject"),
        ])
    );
    let hire_text = hire_forms
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    for word in [
        "mechanic",
        "edge-case",
        "domain",
        "stamina",
        "sprint",
        "lantern",
    ] {
        assert!(
            !common::read::has_word(&hire_text, word),
            "spec-c's forms say {word}: {hire_text}"
        );
    }
}
