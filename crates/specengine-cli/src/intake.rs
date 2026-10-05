//! `spec propose question ID… --text T --working-answer W --price-of-other
//! P [--severity S] [--distinct-from X]…` and `spec propose discrepancy
//! --input F|-` (canon `agent-intake`, "Tools", "Rules"): an agent's
//! question or reported discrepancy stored as a queue record that never
//! applies, checked first against what is already decided and asked. Only
//! the queue is written: nothing under the root, no commit, no state change
//! of another proposal. In order, the first refusal answering (exit 1
//! `<field>: <problem>`, nothing stored, no ID taken):
//!
//! 1. form: clap, `--input` not UTF-8 JSON of the shape (exit 2);
//! 2. caps, enums, the author's grammar (core [`specengine_core::intake`]);
//! 3. `node_ids` resolved as `propose update` step 1 resolves its target
//!    (a `.md` path: its file's document, by its `id:`, else by its path),
//!    generated and `immutable_text` holders allowed, each node once
//!    canonical; stored canonical, in the order given;
//! 4. a discrepancy's `proposed_patch`: its target among them, then
//!    `propose update` steps 1–4 (refusals prefixed `proposed_patch: `;
//!    the findings it introduces stored with it, never refusing);
//! 5. the place, as `propose update` step 5 (exit 2);
//! 6. corpus hits from the index this call refreshed: per target, each
//!    link `spec show --links` lists (both ways, resolved, any type but
//!    `mentions`) whose other end lies in a live `class: decision` document
//!    with `status: accepted`, and that document when the target lies in
//!    one; such a document only mentioned: related. A document's links
//!    include its sections'. Node kinds are never consulted: the engine
//!    knows no subject domain;
//! 7. the queue's hits and the insert in one `Immediate` transaction
//!    ([`specengine_store::ProposalQueue::create_intake`]): stored only when
//!    every hit is named in `distinct_from`; a patch becomes a linked
//!    `update`, decided on its own.
//!
//! The answer is the intake document (exit 0 whether stored or not):
//! `{id, created, hits, related, linked, diagnostics, notes}`; past 10 hits
//! (related items), a note names the rest, up to 64 in all
//! (`distinct_from`'s cap), and counts those past that: an item with more
//! than 64 hits is never stored (a known limit).

use std::fs;
use std::io::Read as _;
use std::path::PathBuf;

use serde::Serialize;
use specengine_core::check::{DocClass, Endpoint, NodeAt, SpecGraph, Standing};
use specengine_core::intake::{
    AuthorInput, DISTINCT_MAX, DiscrepancyInput, Evidence, FieldProblem, GapType, IntakeOption,
    IntakeSeverity, ProposedPatch, QuestionInput, discrepancy_problem, question_problem,
};
use specengine_core::proposal::Author;
use specengine_model::{Direction, is_weak_link};
use specengine_store::{
    GitEnv, Intake, NewIntake, NewProposal, ProposalFinding, ProposalKind, ProposalQueue as _,
    ProposalStatus, QueueMatch,
};

use crate::cap::SHOW_TAIL_NAMES;
use crate::corpus::{Admission, indexed};
use crate::proposals::{
    checked_now, escaped_error, finding_line, more_findings_note, open_context, queue_cannot,
};
use crate::propose::{checked_update, is_path_target, resolved_node, written_reference};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The most bytes of a discrepancy's `--input` document: 8 MiB.
pub const INTAKE_INPUT_MAX_BYTES: usize = 8 << 20;

/// The most hits, and the most related items, an intake answer lists in
/// full; a note names the rest (up to [`DISTINCT_MAX`] in all, so that
/// `distinct_from` can name every hit of an item that can be stored) and
/// counts those past that.
pub const INTAKE_MATCHES_MAX: usize = 10;

/// The most matches of one list a note names past [`INTAKE_MATCHES_MAX`].
const NAMED_PAST_MAX: usize = DISTINCT_MAX - INTAKE_MATCHES_MAX;

/// The `status:` of a decision the owner accepted (the documentation
/// convention's decision class).
const ACCEPTED: &str = "accepted";

/// `spec propose question` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionRequest {
    /// `ID…` as given.
    pub node_ids: Vec<String>,
    /// `--text T`.
    pub text: String,
    /// `--working-answer W`: what the agent works on meanwhile.
    pub working_answer: String,
    /// `--price-of-other P`: what another answer would cost.
    pub price_of_other: String,
    /// `--severity S`; absent: `normal`.
    pub severity: Option<IntakeSeverity>,
    /// `--distinct-from X…`: the hits this question is declared distinct
    /// from.
    pub distinct_from: Vec<String>,
    /// `--author-role`, `--author-model`, `--run`: any given → an agent.
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// `spec propose discrepancy` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscrepancyRequest {
    /// The `--input` document ([`read_discrepancy_input`]).
    pub input: DiscrepancyInput,
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`; its date is a patch's
    /// validation's today.
    pub now: String,
    pub git: GitEnv,
}

/// Where `--input` comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeSource {
    /// `--input F`: relative to the current directory.
    File(PathBuf),
    /// `--input -`: the bytes `main` read from stdin (at most one more than
    /// [`INTAKE_INPUT_MAX_BYTES`]), or a caller's own.
    Given(Vec<u8>),
}

/// Where a match was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchSource {
    /// An accepted decision document of the corpus.
    Corpus,
    /// An item of the queue.
    Queue,
}

impl MatchSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Corpus => "corpus",
            Self::Queue => "queue",
        }
    }
}

/// One hit or related item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntakeMatch {
    /// A decision's `id:` (`null` without one) or a proposal's ID.
    pub id: Option<String>,
    pub source: MatchSource,
    /// `accepted`, or the proposal's state.
    pub status: String,
    /// A decision's root-relative path; `null` for a proposal.
    pub path: Option<String>,
    /// A hit's answer: the decision's title, a rejected proposal's reason;
    /// `null` for a related item.
    pub answer: Option<String>,
}

impl IntakeMatch {
    /// The name `distinct_from` gives it: its ID, else its path.
    pub fn name(&self) -> &str {
        self.id
            .as_deref()
            .or(self.path.as_deref())
            .unwrap_or_default()
    }
}

/// The intake document: every key present, absent = `null`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntakeDocument {
    /// The stored item's `PR-…`; `null` when nothing was stored.
    pub id: Option<String>,
    pub created: bool,
    /// Corpus hits by path, then queue hits by ID number; at most
    /// [`INTAKE_MATCHES_MAX`], the rest named in a note.
    pub hits: Vec<IntakeMatch>,
    /// As `hits`, their answers `null`.
    pub related: Vec<IntakeMatch>,
    /// The linked update of a discrepancy's proposed patch.
    pub linked: Option<String>,
    /// The findings that update introduces, at most `SHOW_TAIL_NAMES`.
    pub diagnostics: Vec<ProposalFinding>,
    /// Matches past those listed (named, then counted) and findings not
    /// listed; a refusal's reason last.
    pub notes: Vec<String>,
}

/// What `spec propose question` or `spec propose discrepancy` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeOutcome {
    pub document: IntakeDocument,
    /// Why it was refused (exit 1); also the document's last note.
    pub refusal: Option<String>,
    pub messages: Vec<Message>,
    /// The linked update's introduced findings past those listed.
    pub omitted_findings: usize,
    /// A patch was proposed with it (the text then counts its findings).
    pub patched: bool,
    /// More hits than `distinct_from` can name ([`DISTINCT_MAX`]): never
    /// stored, a known limit the text and the hits' note state.
    pub unnameable: bool,
}

impl IntakeOutcome {
    /// Exit 0 whether stored or not; 1 when refused.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    fn refused(reason: &str, messages: Vec<Message>) -> Self {
        let reason = one_line(reason);
        Self {
            document: IntakeDocument {
                notes: vec![reason.clone()],
                ..IntakeDocument::default()
            },
            refusal: Some(reason),
            messages,
            omitted_findings: 0,
            patched: false,
            unnameable: false,
        }
    }
}

impl Serialize for IntakeOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.document.serialize(serializer)
    }
}

/// `spec propose question`: one question, checked and stored unless it is
/// already answered or asked.
pub fn propose_question(
    env: &Env,
    globals: &Globals,
    request: &QuestionRequest,
) -> Result<IntakeOutcome, CliError> {
    run_question(env, globals, request).map_err(escaped_error)
}

/// `spec propose discrepancy`: one discrepancy (and its proposed patch as a
/// linked update), checked and stored unless it is already decided or
/// reported.
pub fn propose_discrepancy(
    env: &Env,
    globals: &Globals,
    request: &DiscrepancyRequest,
) -> Result<IntakeOutcome, CliError> {
    run_discrepancy(env, globals, request).map_err(escaped_error)
}

/// `--input F|-`: UTF-8 JSON of a discrepancy's arguments (the author's
/// aside), at most [`INTAKE_INPUT_MAX_BYTES`]; anything else exits 2, its
/// message's control characters escaped (the path and the parser's quote
/// of the input are agent-written).
pub fn read_discrepancy_input(
    env: &Env,
    source: &IntakeSource,
) -> Result<DiscrepancyInput, CliError> {
    read_input(env, source).map_err(escaped_error)
}

fn read_input(env: &Env, source: &IntakeSource) -> Result<DiscrepancyInput, CliError> {
    let read: Vec<u8>;
    let (label, bytes) = match source {
        IntakeSource::Given(bytes) => ("-".to_owned(), bytes.as_slice()),
        IntakeSource::File(path) => {
            let label = path.display().to_string();
            let cannot =
                |error: std::io::Error| CliError::spec(format!("--input {label}: {error}"));
            let file = fs::File::open(env.cwd.join(path)).map_err(cannot)?;
            let mut bytes = Vec::new();
            file.take(INTAKE_INPUT_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(cannot)?;
            read = bytes;
            (label, read.as_slice())
        }
    };
    if bytes.len() > INTAKE_INPUT_MAX_BYTES {
        return Err(CliError::spec(format!(
            "--input {label}: longer than {INTAKE_INPUT_MAX_BYTES} bytes (8 MiB)"
        )));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| {
        CliError::spec(format!(
            "--input {label}: not UTF-8 (at byte {})",
            error.valid_up_to()
        ))
    })?;
    serde_json::from_str(text).map_err(|error| {
        CliError::spec(format!(
            "--input {label}: not a discrepancy's arguments (`node_ids`, `summary`, \
             `gap_type`, `severity`, `evidence`, `options`, `recommendation`, optional \
             `working_answer`, `proposed_patch`, `distinct_from`): {error}"
        ))
    })
}

fn run_question(
    env: &Env,
    globals: &Globals,
    request: &QuestionRequest,
) -> Result<IntakeOutcome, CliError> {
    let author = AuthorInput {
        role: request.author_role.as_deref(),
        model: request.author_model.as_deref(),
        run: request.run.as_deref(),
    };
    let problem = question_problem(
        &QuestionInput {
            node_ids: &request.node_ids,
            text: &request.text,
            working_answer: &request.working_answer,
            price_of_other: &request.price_of_other,
            distinct_from: &request.distinct_from,
        },
        author,
    );
    run_intake(
        env,
        globals,
        &Item {
            kind: ProposalKind::Question,
            node_ids: &request.node_ids,
            severity: request.severity.unwrap_or(IntakeSeverity::Normal),
            gap_type: None,
            summary: &request.text,
            working_answer: Some(&request.working_answer),
            price_of_other: Some(&request.price_of_other),
            evidence: &[],
            options: &[],
            recommendation: None,
            patch: None,
            distinct_from: &request.distinct_from,
            author: (&request.author_role, &request.author_model, &request.run),
            now: &request.now,
            git: &request.git,
        },
        problem,
    )
}

fn run_discrepancy(
    env: &Env,
    globals: &Globals,
    request: &DiscrepancyRequest,
) -> Result<IntakeOutcome, CliError> {
    let input = &request.input;
    let author = AuthorInput {
        role: request.author_role.as_deref(),
        model: request.author_model.as_deref(),
        run: request.run.as_deref(),
    };
    let problem = discrepancy_problem(input, author);
    run_intake(
        env,
        globals,
        &Item {
            kind: ProposalKind::Discrepancy,
            node_ids: &input.node_ids,
            severity: input.severity,
            gap_type: Some(input.gap_type),
            summary: &input.summary,
            working_answer: input.working_answer.as_deref(),
            price_of_other: None,
            evidence: &input.evidence,
            options: &input.options,
            recommendation: Some(input.recommendation),
            patch: input.proposed_patch.as_ref(),
            distinct_from: input.distinct_from.as_deref().unwrap_or_default(),
            author: (&request.author_role, &request.author_model, &request.run),
            now: &request.now,
            git: &request.git,
        },
        problem,
    )
}

/// One item as given, either kind.
struct Item<'a> {
    kind: ProposalKind,
    node_ids: &'a [String],
    severity: IntakeSeverity,
    gap_type: Option<GapType>,
    summary: &'a str,
    working_answer: Option<&'a str>,
    price_of_other: Option<&'a str>,
    evidence: &'a [Evidence],
    options: &'a [IntakeOption],
    recommendation: Option<u64>,
    patch: Option<&'a ProposedPatch>,
    distinct_from: &'a [String],
    /// Role, model, run.
    author: (&'a Option<String>, &'a Option<String>, &'a Option<String>),
    now: &'a str,
    git: &'a GitEnv,
}

/// A target resolved at step 3.
struct Target {
    /// Canonical: `ID`, `slug/ID`, or an id-less document's path.
    id: String,
    /// Its holder, root-relative.
    path: String,
}

fn run_intake(
    env: &Env,
    globals: &Globals,
    item: &Item<'_>,
    problem: Option<FieldProblem>,
) -> Result<IntakeOutcome, CliError> {
    let now = checked_now(item.now)?;
    // 2. Caps, enums, the author's grammar.
    if let Some(problem) = problem {
        return Ok(IntakeOutcome::refused(&problem.to_string(), Vec::new()));
    }
    let (role, model, run) = item.author;
    let author = Author::new(role.clone(), model.clone(), run.clone()).map_err(CliError::spec)?;
    let mut context = open_context(env, globals, item.git)?;
    let mut messages = Vec::new();
    let input = indexed(env, &context.project, &mut messages, false)?;

    // 3. The nodes, each once.
    let mut targets: Vec<Target> = Vec::with_capacity(item.node_ids.len());
    for (index, written) in item.node_ids.iter().enumerate() {
        let field = format!("node_ids[{index}]");
        let (id, path) = match resolved_node(&context.project, &input, written)
            .map_err(|error| prefixed(error, &field))?
        {
            Ok(found) => found,
            Err(reason) => {
                return Ok(IntakeOutcome::refused(
                    &format!("{field}: {reason}"),
                    messages,
                ));
            }
        };
        if let Some(first) = targets.iter().position(|target| target.id == id) {
            return Ok(IntakeOutcome::refused(
                &format!(
                    "{field}: `{}` names `{id}`, as node_ids[{first}] does: name each node once",
                    written.trim()
                ),
                messages,
            ));
        }
        targets.push(Target { id, path });
    }

    // 4. The proposed patch: one of the targets, then `propose update`
    // steps 1–4.
    let mut checked = None;
    if let Some(patch) = item.patch {
        const FIELD: &str = "proposed_patch";
        let target = match resolved_node(&context.project, &input, &patch.target)
            .map_err(|error| prefixed(error, FIELD))?
        {
            Ok((id, _)) => id,
            Err(reason) => {
                return Ok(IntakeOutcome::refused(
                    &format!("{FIELD}: {reason}"),
                    messages,
                ));
            }
        };
        if !targets.iter().any(|known| known.id == target) {
            return Ok(IntakeOutcome::refused(
                &format!(
                    "{FIELD}: its target `{target}` is not among node_ids: a patch updates one \
                     of the nodes it reports on"
                ),
                messages,
            ));
        }
        let written = patch.target.trim();
        let reference = match written_reference(written, &context.project.config.scheme)
            .map_err(|error| prefixed(error, FIELD))?
        {
            Ok(reference) => reference,
            Err(reason) => {
                return Ok(IntakeOutcome::refused(
                    &format!("{FIELD}: {reason}"),
                    messages,
                ));
            }
        };
        match checked_update(
            &context.project,
            &input,
            written,
            reference,
            &patch.base,
            &patch.text,
            &now[..10],
            &mut messages,
        )
        .map_err(|error| prefixed(error, FIELD))?
        {
            Ok(update) => checked = Some(update),
            Err(reason) => {
                return Ok(IntakeOutcome::refused(
                    &format!("{FIELD}: {reason}"),
                    messages,
                ));
            }
        }
    }

    // 5. The place.
    let place = context.git.place(&context.project.root).map_err(|error| {
        CliError::spec(format!(
            "the project root {} cannot be bound to a proposal: {error}",
            context.project.root.display()
        ))
    })?;

    // 6. The corpus's accepted decisions on the targets.
    let graph = SpecGraph::new(
        &input,
        &context.project.config.scheme,
        &context.project.config.paths,
    );
    let (corpus_hits, corpus_related) = corpus_matches(&graph, &targets);

    // 7. The queue's hits, and the insert, in one transaction.
    let new_intake = NewIntake {
        kind: item.kind,
        target_path: targets[0].path.clone(),
        place: place.clone(),
        author: author.clone(),
        intake: Intake {
            target_ids: targets.iter().map(|target| target.id.clone()).collect(),
            severity: item.severity,
            gap_type: item.gap_type,
            summary: item.summary.to_owned(),
            working_answer: item.working_answer.map(str::to_owned),
            price_of_other: item.price_of_other.map(str::to_owned),
            evidence: item.evidence.to_vec(),
            options: item.options.to_vec(),
            recommendation: item.recommendation,
            distinct_from: item.distinct_from.to_vec(),
        },
    };
    let patch: Option<NewProposal> = match (&checked, item.patch) {
        (Some(update), Some(patch)) => Some(update.new_proposal(place, &patch.rationale, author)),
        _ => None,
    };
    let corpus_names: Vec<String> = corpus_hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect();
    let result = context
        .queue
        .create_intake(&new_intake, &corpus_names, patch.as_ref(), now)
        .map_err(queue_cannot)?;
    Ok(answer(
        corpus_hits,
        corpus_related,
        result,
        item.patch.is_some(),
        messages,
    ))
}

/// The intake document of what step 7 did.
fn answer(
    corpus_hits: Vec<IntakeMatch>,
    corpus_related: Vec<IntakeMatch>,
    result: specengine_store::IntakeResult,
    patched: bool,
    mut messages: Vec<Message>,
) -> IntakeOutcome {
    let queue_match = |found: QueueMatch, hit: bool| IntakeMatch {
        id: Some(found.id),
        source: MatchSource::Queue,
        status: found.status.as_str().to_owned(),
        path: None,
        answer: if hit && found.status == ProposalStatus::Rejected {
            found.reason
        } else {
            None
        },
    };
    let mut hits = corpus_hits;
    hits.extend(
        result
            .hits
            .into_iter()
            .map(|found| queue_match(found, true)),
    );
    let mut related = corpus_related;
    related.extend(
        result
            .related
            .into_iter()
            .map(|found| queue_match(found, false)),
    );
    let unnameable = hits.len() > DISTINCT_MAX;
    let mut notes = Vec::new();
    if let Some(mut note) = named_past(&mut hits, "hit(s)") {
        if unnameable {
            note.push_str("; ");
            note.push_str(&unnameable_reason());
        }
        notes.push(note);
    }
    notes.extend(named_past(&mut related, "related item(s)"));
    let created = result.created.is_some();
    let linked = result.linked;
    let mut diagnostics = linked
        .as_ref()
        .map(|update| update.diagnostics.clone())
        .unwrap_or_default();
    let mut omitted_findings = 0;
    if diagnostics.len() > SHOW_TAIL_NAMES {
        omitted_findings = diagnostics.len() - SHOW_TAIL_NAMES;
        diagnostics.truncate(SHOW_TAIL_NAMES);
        let id = linked.as_ref().map_or("PR", |update| update.id.as_str());
        notes.push(more_findings_note(omitted_findings, id));
    }
    messages.extend(notes.iter().cloned().map(Message::Note));
    IntakeOutcome {
        document: IntakeDocument {
            id: result.created.map(|created| created.id),
            created,
            hits,
            related,
            linked: linked.map(|update| update.id),
            diagnostics,
            notes,
        },
        refusal: None,
        messages,
        omitted_findings,
        patched: patched && created,
        unnameable,
    }
}

/// Past [`INTAKE_MATCHES_MAX`], `list` keeps its first ones and the note
/// names the rest as `distinct_from` names them (an ID, else a path), in
/// the list's order, up to [`DISTINCT_MAX`] in all; those past that are
/// counted. `None`: nothing past.
fn named_past(list: &mut Vec<IntakeMatch>, what: &str) -> Option<String> {
    if list.len() <= INTAKE_MATCHES_MAX {
        return None;
    }
    let past = list.split_off(INTAKE_MATCHES_MAX);
    let named = past.len().min(NAMED_PAST_MAX);
    let names: Vec<&str> = past[..named].iter().map(IntakeMatch::name).collect();
    let mut note = format!("{named} more {what} by name only: {}", names.join(", "));
    if past.len() > named {
        note.push_str(&format!("; {} more not listed", past.len() - named));
    }
    Some(one_line(&note))
}

/// Why an item with more hits than `distinct_from` takes is never stored.
fn unnameable_reason() -> String {
    format!(
        "more than {DISTINCT_MAX} hits: `distinct_from` names at most {DISTINCT_MAX}, so this \
         item cannot be stored (a known limit)"
    )
}

/// Step 6: the accepted decisions linked to the targets (hits: any link
/// type but `mentions`, both ways, resolved; a target inside one: that
/// document) and those only mentioning them (related), each document once,
/// by path; a hit is never related too.
fn corpus_matches(
    graph: &SpecGraph<'_>,
    targets: &[Target],
) -> (Vec<IntakeMatch>, Vec<IntakeMatch>) {
    let mut hits: Vec<usize> = Vec::new();
    let mut related: Vec<usize> = Vec::new();
    let edges = graph.edges();
    for target in targets {
        let Some(at) = node_of(graph, target) else {
            continue;
        };
        if accepted_decision(graph, at.file) {
            hits.push(at.file);
        }
        let admission = Admission::new(graph, false, [at.file]);
        let found = graph.links(at, |file| admission.admits(file));
        let ends = found
            .outgoing
            .iter()
            .map(|&(index, _)| (index, Direction::Out))
            .chain(
                found
                    .incoming
                    .iter()
                    .map(|&(index, _)| (index, Direction::In)),
            );
        for (index, direction) in ends {
            let edge = &edges[index];
            let other = match direction {
                Direction::Out => &edge.target,
                Direction::In => &edge.source,
            };
            let Endpoint::Nodes(nodes) = other else {
                continue;
            };
            for node in nodes {
                if !accepted_decision(graph, node.file) {
                    continue;
                }
                if is_weak_link(&edge.link_type) {
                    related.push(node.file);
                } else {
                    hits.push(node.file);
                }
            }
        }
    }
    let by_path = |files: &mut Vec<usize>| {
        files.sort_by(|a, b| graph.paths()[*a].cmp(graph.paths()[*b]));
        files.dedup();
    };
    by_path(&mut hits);
    by_path(&mut related);
    related.retain(|file| !hits.contains(file));
    let matched = |file: usize, hit: bool| {
        let document = graph.document(file).and_then(|at| graph.node(at));
        IntakeMatch {
            id: document.and_then(|node| node.id.clone()),
            source: MatchSource::Corpus,
            status: ACCEPTED.to_owned(),
            path: Some(graph.paths()[file].to_owned()),
            answer: if hit {
                document.and_then(|node| node.title.clone())
            } else {
                None
            },
        }
    };
    (
        hits.into_iter().map(|file| matched(file, true)).collect(),
        related
            .into_iter()
            .map(|file| matched(file, false))
            .collect(),
    )
}

/// The graph node of a resolved target: the node of its holder declaring
/// its bare ID; a path's, its holder's document.
fn node_of(graph: &SpecGraph<'_>, target: &Target) -> Option<NodeAt> {
    let file = graph.file_of(&target.path)?;
    if is_path_target(&target.id) {
        return graph.document(file);
    }
    let bare = target
        .id
        .rsplit_once('/')
        .map_or(target.id.as_str(), |(_, id)| id);
    let ord = graph
        .nodes(file)
        .iter()
        .position(|node| node.id.as_deref() == Some(bare))?;
    Some(NodeAt { file, ord })
}

/// `file` is a live document of `class: decision` with `status: accepted`.
fn accepted_decision(graph: &SpecGraph<'_>, file: usize) -> bool {
    if graph.standing(file) != Standing::Live {
        return false;
    }
    graph
        .file(file)
        .and_then(|file| file.parsed.as_ref())
        .and_then(|parsed| parsed.document())
        .and_then(|document| document.fields.as_ref())
        .is_some_and(|fields| {
            fields.class.as_deref().and_then(DocClass::parse) == Some(DocClass::Decision)
                && fields.status.as_deref() == Some(ACCEPTED)
        })
}

/// An exit-2 error of a field's resolution, naming the field.
fn prefixed(error: CliError, field: &str) -> CliError {
    let message = match error.message.strip_prefix("spec: ") {
        Some(rest) => format!("spec: {field}: {rest}"),
        None => error.message,
    };
    CliError {
        exit: error.exit,
        message,
    }
}

/// The text: the stored ID, `linked: PR-…`, a patch's `introduced: <n>` and
/// finding lines, a `hit:` or `related:` line per match, `not stored: …`
/// when nothing was stored; nothing when refused. Control characters
/// escaped.
pub(crate) fn render_text(outcome: &IntakeOutcome) -> String {
    escape_controls(&raw_text(outcome))
}

fn raw_text(outcome: &IntakeOutcome) -> String {
    let mut out = String::new();
    if outcome.refusal.is_some() {
        return out;
    }
    let document = &outcome.document;
    if let Some(id) = &document.id {
        out.push_str(&format!("{id}\n"));
    }
    if let Some(linked) = &document.linked {
        out.push_str(&format!("linked: {linked}\n"));
    }
    if outcome.patched {
        let introduced = document.diagnostics.len() + outcome.omitted_findings;
        out.push_str(&format!("introduced: {introduced}\n"));
        for finding in &document.diagnostics {
            out.push_str(&finding_line(finding));
            out.push('\n');
        }
    }
    for (label, list) in [("hit", &document.hits), ("related", &document.related)] {
        for found in list {
            let first = |text: &str| one_line(text.lines().next().unwrap_or_default());
            out.push_str(&one_line(&format!(
                "{label}: {} | {} | {} | {}",
                found.name(),
                found.status,
                found.path.as_deref().unwrap_or("-"),
                found
                    .answer
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), first)
            )));
            out.push('\n');
        }
    }
    if outcome.unnameable {
        out.push_str(&format!("not stored: {}\n", unnameable_reason()));
    } else if !document.created {
        out.push_str("not stored: name every hit in `distinct_from` to store it anyway\n");
    }
    out
}
