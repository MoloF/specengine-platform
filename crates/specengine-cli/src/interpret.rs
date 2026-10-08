//! `spec propose interpretation TARGET --base HASH --decision REF --how T
//! --why T --text-file F|- --rationale T [--author-role R] [--author-model
//! M] [--run ID] [--task T] [--brief]` (task spec `kind-interpretation`):
//! an `update` carrying an accepted decision into a rule, stored as `open`
//! with the decision's `id:` and the two labelled texts, no file touched.
//! In order, the first refusal answers and nothing is stored:
//!
//! 1. caps: the text and the rationale as an update's; `--how`, `--why` not
//!    blank, at most 2 048 bytes each (core `INTERPRETATION_TEXT_MAX`); the
//!    author (exit 2);
//! 2. creation step 1: the target resolved as `propose update` resolves it
//!    (a path too);
//! 3. the decision ([`decision_named`]): `--decision REF` resolved as a
//!    target is, its node a live `class: decision` document's with `status:
//!    accepted`, or inside one, never told by an `[ids]` kind (a domain-free
//!    core); the target not inside that document; a queue ID named by its
//!    record;
//! 4. creation steps 2–5 (one span, no `{#ID}` added or dropped; the
//!    findings never refuse);
//! 5. `--task`.
//!
//! The answer is the stored proposal's review document, as `propose
//! update`'s. Apply step 4 checks the decision again ([`decision_step`]).
//!
//! Beside it, the note on an `update` touching an accepted decision's
//! `canon:` section ([`canon_note`], never a refusal: nothing is blocked),
//! and the target a record's `canon:` names ([`canon_target`]).

use std::collections::BTreeMap;

use specengine_core::check::{
    CANON_LINK, CheckInput, DocClass, Endpoint, NodeAt, Resolver, SpecGraph,
};
use specengine_core::intake::{
    AuthorInput, Interpretation, author_problem, interpretation_text_problem, rationale_problem,
};
use specengine_core::proposal::{Author, ProposalIdError, look_alike_message, parse_proposal_id};
use specengine_core::record::{ACCEPTED, REJECTED};
use specengine_model::{CanonTarget, grammar};
use specengine_store::{
    GitEnv, Proposal, ProposalKind, ProposalQueue as _, ProposalStatus, QueueError, SqliteQueue,
};

use crate::corpus::indexed;
use crate::intake::{accepted_decision, prefixed};
use crate::preflight::{recorded_project, recorded_root};
use crate::project::ProjectRoot;
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, briefed, checked_now,
    escaped_error, open_context, queue_cannot,
};
use crate::propose::{
    ProposedText, SpanRule, canonical_id, checked_located, is_path_target, located, read_text,
    resolved_node, written_reference,
};
use crate::review::previewed;
use crate::task::bound_task;
use crate::{CliError, Env, Globals, Message};

/// The most decisions [`canon_note`] names; the rest are counted.
const NOTE_IDS_MAX: usize = 10;

/// `spec propose interpretation` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretRequest {
    /// `TARGET` as given.
    pub target: String,
    /// `--base HASH`: the span hash the text was written against.
    pub base: String,
    /// `--decision REF`: the accepted decision it interprets, as given.
    pub decision: String,
    /// `--how T`: how the decision was applied, verbatim.
    pub how: String,
    /// `--why T`: why interpretation was needed, verbatim.
    pub why: String,
    pub text: ProposedText,
    /// `--rationale T`, verbatim (the commit's body).
    pub rationale: String,
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

/// `spec propose interpretation [--task T]`: checks and stores one
/// interpretation, bound to the task `task` (as written) when given.
pub fn propose_interpretation(
    env: &Env,
    globals: &Globals,
    request: &InterpretRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    run_interpret(env, globals, request, task).map_err(escaped_error)
}

/// `spec propose interpretation --brief` (MCP `propose_change`, kind
/// `interpretation`): [`propose_interpretation`], its answer brief.
pub fn propose_interpretation_brief(
    env: &Env,
    globals: &Globals,
    request: &InterpretRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    propose_interpretation(env, globals, request, task).map(briefed)
}

fn run_interpret(
    env: &Env,
    globals: &Globals,
    request: &InterpretRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let mut context = open_context(env, globals, &request.git)?;
    let place = context.git.place(&context.project.root).map_err(|error| {
        CliError::spec(format!(
            "the project root {} cannot be bound to a proposal: {error}",
            context.project.root.display()
        ))
    })?;
    let mut messages = Vec::new();
    let refuse = |reason: String, messages: Vec<Message>| {
        Ok(ProposalOutcome::refused(
            QueueCommand::Propose,
            ProposalDocument::default(),
            &reason,
            messages,
        ))
    };

    // The caps: the text, the rationale, how and why, then the author.
    let text = match read_text(env, &request.text)? {
        Ok(text) => text,
        Err(reason) => return refuse(reason, messages),
    };
    if let Some(problem) = rationale_problem("rationale", &request.rationale)
        .or_else(|| interpretation_text_problem("--how", &request.how))
        .or_else(|| interpretation_text_problem("--why", &request.why))
    {
        return refuse(problem.to_string(), messages);
    }
    if let Some(problem) = author_problem(AuthorInput {
        role: request.author_role.as_deref(),
        model: request.author_model.as_deref(),
        run: request.run.as_deref(),
    }) {
        return Err(CliError::spec(problem));
    }
    let author = Author::new(
        request.author_role.clone(),
        request.author_model.clone(),
        request.run.clone(),
    )
    .map_err(CliError::spec)?;

    // Creation 1: the target as written, then over the index refreshed now.
    let written = request.target.trim();
    let reference = match written_reference(written, &context.project.config.scheme)? {
        Ok(reference) => reference,
        Err(reason) => return refuse(reason, messages),
    };
    let input = indexed(env, &context.project, &mut messages, false)?;
    let target = match located(&context.project, &input, written, reference, true)? {
        Ok(target) => target,
        Err(reason) => return refuse(reason, messages),
    };

    // The decision.
    let decided = match decision_named(
        &context.project,
        &input,
        Some(&context.queue),
        &request.decision,
        &target.path,
    )
    .map_err(|error| prefixed(error, "--decision"))?
    {
        Ok(decided) => decided,
        Err(why) => return refuse(format!("--decision: {why}; nothing stored"), messages),
    };

    // Creation 2–4.
    let checked = match checked_located(
        &context.project,
        target,
        &request.base,
        &text,
        &now[..10],
        &mut messages,
        SpanRule::Update,
    )? {
        Ok(checked) => checked,
        Err(reason) => return refuse(reason, messages),
    };

    // `--task`, then stored, bound to the place found first (and to its
    // task, checked again under the write lock).
    let task = match task {
        Some(written) => match bound_task(&context, written, &place)? {
            Ok(id) => Some(id),
            Err(reason) => return refuse(reason, messages),
        },
        None => None,
    };
    let mut new = checked.new_proposal(place, &request.rationale, author);
    new.kind = ProposalKind::Interpretation;
    new.interpretation = Some(Interpretation {
        decision: decided.id,
        how: request.how.clone(),
        why: request.why.clone(),
    });
    let created = match context.queue.create_with_task(&new, task.as_deref(), now) {
        Ok(created) => created,
        Err(QueueError::TaskRefused { reason, .. }) => {
            return refuse(format!("--task: {reason}"), messages);
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    let document = previewed(env, &request.git, &context, &created, &mut messages);
    Ok(ProposalOutcome::done(
        QueueCommand::Propose,
        document,
        messages,
    ))
}

/// The accepted decision `--decision` names ([`decision_named`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decided {
    /// Its document's `id:`, canonical; an id-less document's path.
    pub id: String,
    /// Its document's root-relative path.
    pub path: String,
}

/// The decision `written` names over `input` (an index refreshed by this
/// call), for a target held in `target_path`. A queue ID (`queue` given)
/// is named by the record its approval wrote. Else it resolves as a
/// target does (an alias, a legacy ID, `#SECTION`, `@rev` refused naming
/// the canonical form; a look-alike, mixed script or `project:` exit 2 with
/// the Latin fix), its node a live `class: decision` document's with
/// `status: accepted` or inside one; the target outside that document.
/// `Ok(Err)`: why not (the caller frames it).
pub(crate) fn decision_named(
    project: &ProjectRoot,
    input: &CheckInput,
    queue: Option<&SqliteQueue>,
    written: &str,
    target_path: &str,
) -> Result<Result<Decided, String>, CliError> {
    let written = written.trim();
    match parse_proposal_id(written) {
        Ok(id) => return Ok(Err(queue_named(queue, &id)?)),
        Err(ProposalIdError::LookAlike { fix }) => {
            return Err(CliError::spec(look_alike_message(written, &fix)));
        }
        Err(ProposalIdError::NotAnId) => {}
    }
    let (_, path) = match resolved_node(project, input, written)? {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    let Some(parsed) = input
        .files
        .iter()
        .find(|file| file.path == path)
        .and_then(|file| file.parsed.as_ref())
    else {
        return Ok(Err(format!(
            "`{path}` could not be read: its decision cannot be told"
        )));
    };
    let document = parsed.document();
    let fields = document.and_then(|document| document.fields.as_ref());
    let resolver = Resolver::new(input, &project.config.scheme, &project.config.paths);
    let id = document
        .and_then(|document| document.id.as_deref())
        .map_or_else(
            || path.clone(),
            |id| canonical_id(&resolver, &project.config.scheme, &path, id, None),
        );
    let class = fields.and_then(|fields| fields.class.as_deref());
    let status = fields.and_then(|fields| fields.status.as_deref());
    if class.and_then(DocClass::parse) != Some(DocClass::Decision) {
        return Ok(Err(match class {
            Some(class) => format!("{written} is no accepted decision: {class} document {path}"),
            None => {
                format!("{written} is no accepted decision: document {path} declares no `class:`")
            }
        }));
    }
    if let Some((_, successor)) = status.and_then(grammar::split_superseded_by) {
        return Ok(Err(format!(
            "{id} is superseded by {successor}: interpret {successor}"
        )));
    }
    if status == Some(REJECTED) {
        return Ok(Err(format!("{id} is rejected")));
    }
    let live = specengine_core::check::is_live(parsed);
    if status != Some(ACCEPTED) || !live {
        return Ok(Err(format!(
            "{written} is no accepted decision: {} document {path}",
            DocClass::Decision.as_str()
        )));
    }
    if path == target_path {
        return Ok(Err(format!("the target lies in {id} itself")));
    }
    Ok(Ok(Decided { id, path }))
}

/// Why a queue ID is no decision to name: the record its approval wrote,
/// else that none is written yet.
fn queue_named(queue: Option<&SqliteQueue>, id: &str) -> Result<String, CliError> {
    let Some(queue) = queue else {
        return Ok(format!(
            "`{id}` is a queue ID: name the decision it recorded"
        ));
    };
    let Some(proposal) = queue.get(id).map_err(queue_cannot)? else {
        return Ok(format!("no proposal `{id}` in this project's queue"));
    };
    Ok(match (&proposal.record, proposal.status) {
        (Some(record), ProposalStatus::Applied) => {
            format!("`{id}` is recorded as {}: name it", record.id)
        }
        _ if !proposal.kind.decides() => format!(
            "`{id}` is {}: it records no decision",
            proposal.kind.with_article()
        ),
        (_, ProposalStatus::Rejected) => format!("`{id}` is rejected: it records no decision"),
        _ => format!("`{id}` is not recorded yet: `spec approve {id}` records it first"),
    })
}

/// Apply step 4 of an interpretation: its stored decision checked again
/// over the recorded root's refreshed index, as at its creation; the
/// decision found, or the reason (`--decision: <why>`).
pub(crate) fn decision_step(
    project: &ProjectRoot,
    input: &CheckInput,
    interpretation: &Interpretation,
    target_path: &str,
) -> Result<Decided, String> {
    match decision_named(project, input, None, &interpretation.decision, target_path) {
        Ok(Ok(decided)) => Ok(decided),
        Ok(Err(why)) => Err(format!("--decision: {why}")),
        Err(error) => Err(format!(
            "--decision: {}",
            error
                .message
                .strip_prefix("spec: ")
                .unwrap_or(&error.message)
        )),
    }
}

/// The note on an `update` (task spec `kind-interpretation`, W7): when its
/// target's span (`target_id` held in `target_path`, over `input`) is,
/// encloses or lies in the node a live accepted decision's `canon:` names
/// (resolved as `spec check` resolves it), ``<target> is in the `canon:`
/// section of <IDs>: a change carrying one is `spec propose interpretation
/// <target> --decision <ID>` ``, the decisions by path, at most
/// [`NOTE_IDS_MAX`] named. `None` when none; never a refusal.
pub(crate) fn canon_note(
    project: &ProjectRoot,
    input: &CheckInput,
    target_id: &str,
    target_path: &str,
) -> Option<String> {
    let scheme = &project.config.scheme;
    let graph = SpecGraph::new(input, scheme, &project.config.paths);
    let file = graph.file_of(target_path)?;
    let nodes = graph.nodes(file);
    let ord = if is_path_target(target_id) {
        0
    } else {
        let bare = target_id.rsplit_once('/').map_or(target_id, |(_, id)| id);
        nodes
            .iter()
            .position(|node| node.id.as_deref() == Some(bare))?
    };
    let span = nodes.get(ord)?.span;
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    // Path → the decision's name, each decision once.
    let mut found: BTreeMap<&str, String> = BTreeMap::new();
    for edge in graph.edges() {
        if edge.link_type != CANON_LINK || !accepted_decision(&graph, edge.file) {
            continue;
        }
        let Endpoint::Nodes(targets) = &edge.target else {
            continue;
        };
        let touches = targets.iter().any(|at| {
            at.file == file
                && graph.node(*at).is_some_and(|node| {
                    let canon = node.span;
                    (canon.start <= span.start && span.end <= canon.end)
                        || (span.start <= canon.start && canon.end <= span.end)
                })
        });
        if touches {
            let path = graph.paths()[edge.file];
            let name = match graph.node(edge.holder).and_then(|node| node.id.as_deref()) {
                Some(id) => canonical_id(&resolver, scheme, path, id, None),
                None => path.to_owned(),
            };
            found.insert(path, name);
        }
    }
    let names: Vec<&String> = found.values().collect();
    let first = names.first()?;
    let mut listed = names
        .iter()
        .take(NOTE_IDS_MAX)
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > NOTE_IDS_MAX {
        listed.push_str(&format!(" and {} more", names.len() - NOTE_IDS_MAX));
    }
    Some(format!(
        "{target_id} is in the `canon:` section of {listed}: a change carrying one is `spec \
         propose interpretation {target_id} --decision {first}`"
    ))
}

/// The target the `canon:` of the document at `path` names, over `input`,
/// as `spec propose interpretation` takes it: a reference (`ID`,
/// `ID#SECTION`) its section's ID when the section resolves to its own
/// node, else its ID part; a path (`path#anchor`) the ID node it lands on
/// (the nearest enclosing the anchor), an id-less document by its path, the
/// path when it lands nowhere. `None`: no `canon:`, or another project's.
pub(crate) fn canon_target(
    project: &ProjectRoot,
    input: &CheckInput,
    path: &str,
) -> Option<String> {
    let scheme = &project.config.scheme;
    let graph = SpecGraph::new(input, scheme, &project.config.paths);
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    let file = graph.file_of(path)?;
    let canon = graph
        .file(file)?
        .parsed
        .as_ref()?
        .document()?
        .fields
        .as_ref()?
        .canon
        .as_ref()?;
    // The one node the `canon:` edge lands on, when it lands on one.
    let landed = graph
        .edges()
        .iter()
        .find(|edge| edge.file == file && edge.link_type == CANON_LINK)
        .and_then(|edge| match &edge.target {
            Endpoint::Nodes(targets) => match targets.as_slice() {
                [at] => Some(*at),
                _ => None,
            },
            _ => None,
        });
    let named =
        |at: NodeAt, id: &str| canonical_id(&resolver, scheme, graph.paths()[at.file], id, None);
    match canon {
        CanonTarget::Reference(reference) if reference.project.is_some() => None,
        CanonTarget::Reference(reference) => {
            let section = reference.section.as_deref().and_then(|section| {
                let at = landed?;
                (graph.node(at)?.id.as_deref() == Some(section)).then(|| named(at, section))
            });
            Some(section.unwrap_or_else(|| match (&reference.scope, landed) {
                (Some(scope), _) => format!("{scope}/{}", reference.id),
                (None, Some(at)) => named(at, &reference.id),
                (None, None) => reference.id.clone(),
            }))
        }
        CanonTarget::Path(target) => Some(match landed {
            Some(at) => match graph.node(at).and_then(|node| node.id.as_deref()) {
                Some(id) => named(at, id),
                None => graph.paths()[at.file].to_owned(),
            },
            None => target.path.clone(),
        }),
    }
}

/// [`canon_note`] for `spec review` when its preview stopped before step 4
/// (or was not run): over the proposal's recorded root, its index
/// refreshed now; a note that cannot be told says so. A recorded root that
/// is gone gives none: the review's other notes name the place, and the
/// change applies nowhere until it is back.
pub(crate) fn recorded_canon_note(
    env: &Env,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> Option<String> {
    let target_id = proposal.target_id.as_str();
    let cannot = |reason: &str| {
        let reason = reason.strip_prefix("spec: ").unwrap_or(reason);
        format!(
            "cannot tell whether {target_id} is in an accepted decision's `canon:` section: \
             {reason}"
        )
    };
    let root = recorded_root(proposal);
    if !root.is_dir() {
        return None;
    }
    let recorded = match recorded_project(&root, &context.slug) {
        Ok(recorded) => recorded,
        Err(reason) => return Some(cannot(&reason)),
    };
    match indexed(env, &recorded, messages, false) {
        Ok(input) => canon_note(&recorded, &input, target_id, &proposal.target_path),
        Err(error) => Some(cannot(&error.message)),
    }
}
