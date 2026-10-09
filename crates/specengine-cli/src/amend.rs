//! `spec propose amendment PATH --text-file F|- --rationale T
//! [--author-role R] [--author-model M] [--run ID] [--task T] [--brief]`
//! (task spec `kind-amendment`): a new spec or decision document whose
//! front-matter declares `links: {amends: [ID, ...]}` to live nodes, stored
//! as `open`, no file touched. In order, the first refusal answers, nothing
//! stored, no ID taken:
//!
//! 1. the author grammar, the place bound (exit 2);
//! 2. the target: a create's file-form path rules; an indexed file or an ID
//!    is refused (an amendment is a new file);
//! 3. the text: a create's file-form rules, then [`text_problem`]: it
//!    declares `amends`, is `class: spec` or `class: decision`, not Tier 3
//!    by status;
//! 4. its new IDs, as a create's;
//! 5. the amended nodes ([`amended_nodes`]), over the index this call
//!    refreshed: each canonical, live, not defined by the text, named once;
//! 6. validation (findings never refuse: nothing is blocked); 7. `--task`; 8. the
//!    insert, its own new IDs reserved as a create's (never an amended one).
//!
//! The amended IDs are read from the stored text, their one copy: no column
//! holds them. The apply is a create's new file
//! ([`crate::create::approve_file`]): its step 4 checks the amended nodes
//! again ([`amended_step`]), its step 6 the text, its consent names each
//! amended node and its path; no amended file is ever written. No
//! project's word is written here: the proposer names the path, its IDs
//! and the nodes it amends.

use specengine_core::check::resolve::feature_stem;
use specengine_core::check::{
    CheckInput, DocClass, Resolver, front_matter_failure, is_live, is_tier3_file,
};
use specengine_core::create::amended_refs;
use specengine_core::intake::{
    AMENDMENT_KIND, AuthorInput, CREATE_KIND, author_problem, rationale_problem, with_article,
};
use specengine_core::patch::{HolderError, one_holder};
use specengine_core::proposal::Author;
use specengine_core::record::ACCEPTED;
use specengine_model::{AMENDS, IdScheme, ParsedFile, Reference};
use specengine_store::{
    GitEnv, NewProposal, Proposal, ProposalKind, ProposalQueue as _, QueueError, WorkingTree,
    patch_hash,
};

use crate::corpus::indexed;
use crate::create::{
    Corpus, NewIds, canonical_in, checked_file, file_target, parsed_text, reserved_refusal,
    validated,
};
use crate::intake::prefixed;
use crate::preflight::StepFailure;
use crate::project::ProjectRoot;
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, briefed, checked_now, escaped_error,
    open_context, queue_cannot,
};
use crate::propose::{ProposedText, is_path_target, read_text, resolved_node, written_reference};
use crate::review::previewed;
use crate::show::project_qualified;
use crate::task::bound_task;
use crate::{CliError, Env, Globals, Message, one_line, store_error};

/// `spec propose amendment` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendmentRequest {
    /// `PATH` as given: a root-relative `.md` path naming nothing yet.
    pub target: String,
    /// The whole new file: a file, or `-` for stdin.
    pub text: ProposedText,
    /// `--rationale T`, verbatim (the commit's body).
    pub rationale: String,
    /// `--author-role`, `--author-model`, `--run`: any given → an agent.
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`; its date is the
    /// validation's today.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// `spec propose amendment [--task T]`: checks and stores one amendment,
/// bound to the task `task` (as written) when given.
pub fn propose_amendment(
    env: &Env,
    globals: &Globals,
    request: &AmendmentRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    run_amend(env, globals, request, task).map_err(escaped_error)
}

/// `spec propose amendment --brief` (MCP `propose_change`, kind
/// `amendment`): [`propose_amendment`], its answer brief.
pub fn propose_amendment_brief(
    env: &Env,
    globals: &Globals,
    request: &AmendmentRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    propose_amendment(env, globals, request, task).map(briefed)
}

/// `an amendment`: the writer its refusals name.
fn an_amendment() -> String {
    with_article(AMENDMENT_KIND)
}

fn run_amend(
    env: &Env,
    globals: &Globals,
    request: &AmendmentRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    // 1. The form: the clock, the author, the place bound.
    let now = checked_now(&request.now)?;
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
    let text = match read_text(env, &request.text)? {
        Ok(text) => text,
        Err(reason) => return refuse(reason, messages),
    };
    if let Some(problem) = rationale_problem("rationale", &request.rationale) {
        return refuse(problem.to_string(), messages);
    }

    // 2. The target: a new file's path, over the index refreshed now.
    let amendment = an_amendment();
    let written = request.target.trim();
    if !is_path_target(written) {
        return refuse(
            format!("`{written}`: {amendment} is a new file: name its .md path; nothing stored"),
            messages,
        );
    }
    let input = indexed(env, &context.project, &mut messages, false)?;
    if input.files.iter().any(|file| file.path == written) {
        return refuse(
            format!("`{written}` exists: {amendment} is a new file; nothing stored"),
            messages,
        );
    }
    // 3. The text: a create's file form, then an amendment's own rules.
    let project = &context.project;
    let checked = match checked_file(
        project,
        &context.git,
        written,
        None,
        &text,
        &amendment,
        "nothing stored",
    )? {
        Ok(checked) => checked,
        Err(reason) => return refuse(reason, messages),
    };
    let Some(parsed) = checked.parsed.as_ref() else {
        return refuse(
            format!("the spec parser failed on the text of `{written}`"),
            messages,
        );
    };
    if let Some(problem) = text_problem(&checked.path, parsed) {
        return refuse(format!("{problem}; nothing stored"), messages);
    }

    // 4. Its new IDs, as a create's.
    let scheme = &project.config.scheme;
    let corpus = Corpus::of(&input, scheme, &project.config.paths);
    let reserved = context.queue.reserved().map_err(queue_cannot)?;
    let named = NewIds {
        scheme,
        decision_prefix: project
            .config
            .decision_records
            .as_ref()
            .map(|table| table.prefix.as_str()),
        slug: checked.slug.as_deref(),
    };
    let new_ids = match named.checked(&checked.sites, &corpus, &reserved)? {
        Ok(new_ids) => new_ids,
        Err(reason) => return refuse(reason, messages),
    };

    // 5. The amended nodes.
    if let Err(reason) = amended_nodes(
        project,
        &input,
        &text,
        parsed,
        &new_ids,
        checked.slug.as_deref(),
    )? {
        return refuse(format!("{reason}; nothing stored"), messages);
    }

    // 6. Validation: never a refusal.
    let today = &now[..10];
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let diagnostics = validated(
        project,
        &tree,
        today,
        &checked.path,
        checked.patched.clone(),
        &mut messages,
    );

    // 7. `--task`.
    let task = match task {
        Some(written) => match bound_task(&context, written, &place)? {
            Ok(id) => Some(id),
            Err(reason) => return refuse(reason, messages),
        },
        None => None,
    };

    // 8. Stored, bound to the place found first; its new IDs reserved as a
    // create's, a live proposal taking one meanwhile step 4's refusal.
    let target_id = file_target(&checked, &new_ids);
    let new = NewProposal {
        kind: ProposalKind::Amendment,
        patch_hash: patch_hash(&target_id, "", &checked.new_text),
        target_id,
        target_path: checked.path.clone(),
        place,
        base_hash: None,
        base_text: None,
        new_text: checked.new_text.clone(),
        rationale: request.rationale.clone(),
        author,
        diagnostics,
        new_ids: new_ids.clone(),
        interpretation: None,
    };
    let created = match context.queue.create_with_task(&new, task.as_deref(), now) {
        Ok(created) => created,
        Err(QueueError::TaskRefused { reason, .. }) => {
            return refuse(format!("--task: {reason}"), messages);
        }
        Err(QueueError::Reserved { id, by, kind }) => {
            let reason = reserved_refusal(&context, &named, &corpus, &new_ids, &id, &by, kind)?;
            return refuse(reason, messages);
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

/// Propose step 3's own rules on an amendment's text `parsed` (the new
/// file `path`), after a create's file-form rules (task spec
/// `kind-amendment`, W29): its front-matter reads (else the parser's
/// finding, its file and line named); it declares the nodes it amends; it
/// is a spec or a decision (the classes that reach Tier 3 by status, which
/// applies it); it is not Tier 3 already (a decision is live only while
/// `accepted`). `None`: it passes. Apply step 6 runs it again on the
/// stored text.
pub(crate) fn text_problem(path: &str, parsed: &ParsedFile) -> Option<String> {
    let amendment = an_amendment();
    // The nodes it amends are named only in its front-matter: one that
    // does not read is refused by the parser's finding, as the check
    // reports it.
    if let Some(failure) = front_matter_failure(parsed) {
        return Some(format!(
            "{path}:{}: {}: {}; {amendment} names the nodes it amends in its front-matter, which \
             does not read",
            failure.line,
            failure.code.as_str(),
            one_line(&failure.message)
        ));
    }
    if amended_refs(parsed).is_empty() {
        return Some(format!(
            "`{path}` declares no `{AMENDS}` link: name the node it amends in `links: \
             {{{AMENDS}: [ID]}}`; a new file amending nothing is `spec propose {CREATE_KIND}`"
        ));
    }
    let fields = parsed
        .document()
        .and_then(|document| document.fields.as_ref());
    let applied = format!(
        "{amendment} is applied when its document goes to Tier 3 by status, which only a {} or a \
         {} can",
        DocClass::Spec.as_str(),
        DocClass::Decision.as_str()
    );
    let class = fields.and_then(|fields| fields.class.as_deref());
    let class = match class.and_then(DocClass::parse) {
        Some(class @ (DocClass::Spec | DocClass::Decision)) => class,
        Some(DocClass::Canon | DocClass::Generated) | None => {
            return Some(match class {
                Some(class) => format!("`{path}` is `class: {class}`: {applied}"),
                None => format!("`{path}` declares no `class:`: {applied}"),
            });
        }
    };
    if is_tier3_file(parsed) {
        let status = fields
            .and_then(|fields| fields.status.as_deref())
            .unwrap_or_default();
        return Some(match class {
            DocClass::Decision => format!(
                "`{path}` is `class: {}` with `status: {status}`: a decision document is live \
                 only while `{ACCEPTED}`, and {amendment} is written live",
                class.as_str()
            ),
            DocClass::Spec | DocClass::Canon | DocClass::Generated => {
                format!("`{path}` is archived (`status: {status}`): {amendment} is written live")
            }
        });
    }
    None
}

/// One node an amendment amends, as found over an index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Amended {
    /// Its ID as the text writes it: canonical (`slug/ID` for a
    /// feature-scoped one).
    pub id: String,
    /// Its holder, root-relative.
    pub path: String,
}

/// Propose step 5 (and apply step 4): each node `parsed` (the text `text`
/// of a new file whose feature slug is `slug`, defining `new_ids`) amends,
/// in written order, over `input` (an index refreshed by this call):
/// written as Creation 1 takes a target (an alias, a legacy ID, `#SECTION`,
/// `@rev`, `[[…]]` refused naming the canonical form; a look-alike, mixed
/// script or `project:` exits 2 with the Latin fix), named once, not one of
/// the text's own new IDs, held by a node (a reserved ID is none), written
/// canonical (a bare feature-scoped ID is refused naming `slug/ID`), its
/// document live (neither Tier 3 by status nor generated). An
/// `immutable_text` holder passes: nothing writes it. `Ok(Err)`: the
/// refusal (`amends …`), the caller ending it.
pub(crate) fn amended_nodes(
    project: &ProjectRoot,
    input: &CheckInput,
    text: &str,
    parsed: &ParsedFile,
    new_ids: &[String],
    slug: Option<&str>,
) -> Result<Result<Vec<Amended>, String>, CliError> {
    let scheme = &project.config.scheme;
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    let mut seen: Vec<String> = Vec::new();
    let mut found = Vec::new();
    for reference in amended_refs(parsed) {
        let written = written_of(text, reference);
        let why = |reason: String| Ok(Err(format!("{AMENDS} `{written}`: {reason}")));
        // Creation 1's forms.
        let named =
            match written_reference(&written, scheme).map_err(|error| prefixed(error, AMENDS))? {
                Ok(Some(named)) => named,
                Ok(None) => return why(format!("no node {written}")),
                Err(reason) => return Ok(Err(format!("{AMENDS}: {reason}"))),
            };
        if seen.contains(&written) {
            return why(format!("{written} is named twice"));
        }
        seen.push(written.clone());
        let own = match &named.scope {
            Some(scope) => format!("{scope}/{}", named.id),
            None => canonical_in(scheme, slug, &named.id),
        };
        if new_ids.contains(&own) || new_ids.contains(&written) {
            return why(format!("{written} is defined by this text"));
        }
        match one_holder(&resolver, &named, &written) {
            Err(HolderError::Dangling(_)) => return why(format!("no node {written}")),
            Err(HolderError::Project) => {
                return Err(prefixed(project_qualified(&written), AMENDS));
            }
            Err(HolderError::Several(_)) | Ok(_) => {}
        }
        let (id, path) = match resolved_node(project, input, &written)
            .map_err(|error| prefixed(error, AMENDS))?
        {
            Ok(holder) => holder,
            Err(reason) => return Ok(Err(format!("{AMENDS}: {reason}"))),
        };
        if id != written {
            return why(format!("{} names the canonical ID `{id}`", an_amendment()));
        }
        let holder = input
            .files
            .iter()
            .find(|file| file.path == path)
            .and_then(|file| file.parsed.as_ref());
        if let Some(holder) = holder
            && !is_live(holder)
        {
            let fields = holder
                .document()
                .and_then(|document| document.fields.as_ref());
            let class = fields.and_then(|fields| fields.class.as_deref());
            if class == Some(DocClass::Generated.as_str()) {
                return why(format!("{written} is generated ({path})"));
            }
            let status = fields
                .and_then(|fields| fields.status.as_deref())
                .unwrap_or_default();
            return why(format!("{written} is archived ({path}, status: {status})"));
        }
        found.push(Amended { id, path });
    }
    Ok(Ok(found))
}

/// Apply step 4 of a proposal that [amends](ProposalKind::amends): the
/// nodes its stored text amends checked again over the recorded root's
/// refreshed index (`input`), as at its creation; the nodes found, or the
/// failure (`amends …; nothing changed`).
pub(crate) fn amended_step(
    recorded: &ProjectRoot,
    input: &CheckInput,
    proposal: &Proposal,
    parsed: &ParsedFile,
) -> Result<Vec<Amended>, StepFailure> {
    let slug = feature_stem(&recorded.config.paths.features, &proposal.target_path);
    match amended_nodes(
        recorded,
        input,
        &proposal.new_text,
        parsed,
        &proposal.new_ids,
        slug,
    ) {
        Ok(Ok(found)) => Ok(found),
        Ok(Err(reason)) => Err(StepFailure::refused(
            4,
            format!("{reason}; nothing changed"),
        )),
        Err(error) => {
            let reason = error
                .message
                .strip_prefix("spec: ")
                .unwrap_or(&error.message);
            Err(StepFailure::cannot(4, format!("{reason}; nothing changed")))
        }
    }
}

/// An amended reference as the text writes it: the text under its span
/// when the parser kept one (a verbatim scalar), else its parts joined.
fn written_of(text: &str, reference: &Reference) -> String {
    if let Some(span) = reference.span
        && let Some(written) = text.get(span.start..span.end)
    {
        return written.trim().to_owned();
    }
    let mut written = String::new();
    if let Some(project) = &reference.project {
        written.push_str(project);
        written.push(':');
    }
    if let Some(scope) = &reference.scope {
        written.push_str(scope);
        written.push('/');
    }
    written.push_str(&reference.id);
    if let Some(section) = &reference.section {
        written.push('#');
        written.push_str(section);
    }
    if let Some(rev) = reference.rev {
        written.push_str(&format!("@{rev}"));
    }
    written
}

/// The IDs a stored new file amends, read from its text (their one copy)
/// under `scheme`, as written, in order: an amendment's; none for any
/// other proposal or a text the parser fails on.
pub(crate) fn amended_ids(proposal: &Proposal, scheme: &IdScheme) -> Vec<String> {
    if !proposal.new_file() {
        return Vec::new();
    }
    let Some(parsed) = parsed_text(&proposal.target_path, &proposal.new_text, scheme) else {
        return Vec::new();
    };
    amended_refs(&parsed)
        .iter()
        .map(|reference| written_of(&proposal.new_text, reference))
        .collect()
}
