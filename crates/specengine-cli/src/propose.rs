//! `spec propose update ID --base HASH --text-file F|- --rationale T
//! [--author-role R] [--author-model M] [--run ID]` (task spec
//! `proposal-apply`, "Creation"): one proposal of kind `update` stored as
//! `open`, no file touched. Nothing is stored and no ID taken unless:
//!
//! 1. `ID` (an ID or `slug/ID`) resolves as `spec show` resolves it to one
//!    holder, not a `class: generated` file, its prefix (and for a section
//!    its document's) without `immutable_text`; an alias, a path, `#SECTION`,
//!    `@rev` or `[[…]]` is refused naming the canonical ID when known; a
//!    look-alike exits 2 naming the Latin form;
//! 2. `--base` is the span's current hash (`spec show`'s `span_hash`); a
//!    stale one is refused printing the current;
//! 3. the text (UTF-8, at most 1 MiB; verbatim, a section's trailing
//!    whitespace dropped), spliced in and parsed afresh, keeps the file's
//!    ordered (ID, heading level) list, the target spanning exactly it, and
//!    changes the file (else "no change");
//! 4. validation: the findings the edit introduces in the project's check
//!    are stored with it, never refusing;
//! 5. binding: the canonical worktree top, the root in it, the common dir,
//!    the branch and `HEAD` (a root in no worktree, a detached or unborn
//!    `HEAD` exit 2).
//!
//! A bare ID of a `scope = "feature"` prefix found in a feature document is
//! stored as `slug/ID`, the form a later apply resolves alike. The answer
//! is the proposal's review document.

use std::fs;
use std::io::Read as _;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use specengine_core::DOCUMENT_EXTENSION;
use specengine_core::check::{CheckInput, Resolver};
use specengine_core::intake::{AuthorInput, author_problem, rationale_problem};
use specengine_core::patch::{
    HolderError, LocateError, TargetForm, locate, one_holder, span_bytes, target_form,
    update_refusal,
};
use specengine_core::proposal::Author;
use specengine_model::{IdScheme, IdScope, IdScript, ParsedFile, Reference, grammar};
use specengine_store::{
    GitEnv, NamedBytes, NewProposal, ProposalFinding, ProposalKind, ProposalQueue as _,
    Source as _, WorkingTree, default_baseline, introduced_findings, load_check, patch_hash,
    span_hash, update_file,
};

use crate::corpus::indexed;
use crate::project::{CONFIG_FILE, ProjectRoot};
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, briefed, checked_now, escaped_error,
    open_context, queue_cannot,
};
use crate::review::previewed;
use crate::show::{latin_fix, no_reference, project_qualified};
use crate::{CliError, Env, Globals, Message, store_error};

/// The most bytes of a proposed text (core's).
pub use specengine_core::proposal::TEXT_MAX_BYTES;

/// Where the proposed text comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposedText {
    /// `--text-file F`: relative to the current directory.
    File(PathBuf),
    /// `--text-file -`: the bytes `main` read from stdin (at most one more
    /// than [`TEXT_MAX_BYTES`]), or a caller's own.
    Given(Vec<u8>),
}

/// `spec propose update` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposeRequest {
    /// `ID` as given.
    pub target: String,
    /// `--base HASH`: the span hash the text was written against.
    pub base: String,
    pub text: ProposedText,
    /// `--rationale T`, verbatim (the commit's body); at most
    /// [`specengine_core::intake::RATIONALE_MAX`] bytes.
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

/// `spec propose update`: checks and stores one proposal.
pub fn propose(
    env: &Env,
    globals: &Globals,
    request: &ProposeRequest,
) -> Result<ProposalOutcome, CliError> {
    run_propose(env, globals, request).map_err(escaped_error)
}

/// `spec propose update --brief` (MCP `propose_change`): [`propose`], its
/// answer brief (`proposals::briefed`: the texts and diff dropped, at most
/// 20 findings, the text cut at the output cap).
pub fn propose_brief(
    env: &Env,
    globals: &Globals,
    request: &ProposeRequest,
) -> Result<ProposalOutcome, CliError> {
    propose(env, globals, request).map(briefed)
}

fn run_propose(
    env: &Env,
    globals: &Globals,
    request: &ProposeRequest,
) -> Result<ProposalOutcome, CliError> {
    let now = checked_now(&request.now)?;
    // An author field outside its grammar exits 2, named as the intake
    // commands and the MCP tools name it (`author_role`).
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

    // 1. The target as written, then (steps 1–4) resolved, based, spliced
    // and validated over the index refreshed now.
    let written = request.target.trim();
    let reference = match written_reference(written, &context.project.config.scheme)? {
        Ok(reference) => reference,
        Err(reason) => return refuse(reason, messages),
    };
    let input = indexed(env, &context.project, &mut messages, false)?;
    let checked = match checked_update(
        &context.project,
        &input,
        written,
        reference,
        &request.base,
        &text,
        &now[..10],
        &mut messages,
    )? {
        Ok(checked) => checked,
        Err(reason) => return refuse(reason, messages),
    };

    // 5. Stored, bound to the place found first.
    let created = context
        .queue
        .create(
            &checked.new_proposal(place, &request.rationale, author),
            now,
        )
        .map_err(queue_cannot)?;
    let document = previewed(env, &request.git, &context, &created, &mut messages);
    Ok(ProposalOutcome::done(
        QueueCommand::Propose,
        document,
        messages,
    ))
}

/// What steps 1–4 make of a target, a base and a text: everything an
/// `update` stores but its place, rationale and author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedUpdate {
    /// Canonical: `ID`, or `slug/ID` for a feature-scoped one.
    pub target_id: String,
    /// Root-relative.
    pub path: String,
    pub base_hash: String,
    pub base_text: String,
    /// As spliced.
    pub new_text: String,
    pub diagnostics: Vec<ProposalFinding>,
}

impl CheckedUpdate {
    /// The `update` to store.
    pub(crate) fn new_proposal(
        &self,
        place: specengine_store::Place,
        rationale: &str,
        author: Author,
    ) -> NewProposal {
        NewProposal {
            kind: ProposalKind::Update,
            patch_hash: patch_hash(&self.target_id, &self.base_hash, &self.new_text),
            target_id: self.target_id.clone(),
            target_path: self.path.clone(),
            place,
            base_hash: self.base_hash.clone(),
            base_text: self.base_text.clone(),
            new_text: self.new_text.clone(),
            rationale: rationale.to_owned(),
            author,
            diagnostics: self.diagnostics.clone(),
        }
    }
}

/// Step 1 before the index is read: `written` (trimmed) parsed as a
/// reference in its canonical form; `Ok(Ok(None))` for a `.md` path (refused
/// once resolved, naming its ID). An alias, `#SECTION`, `@rev`, `[[…]]` or
/// no reference at all is a refusal (`Ok(Err)`); a look-alike, mixed script
/// or `project:` exits 2.
pub(crate) fn written_reference(
    written: &str,
    scheme: &IdScheme,
) -> Result<Result<Option<Reference>, String>, CliError> {
    if written.ends_with(DOCUMENT_EXTENSION) {
        return Ok(Ok(None));
    }
    let Some(found) = grammar::parse_reference(written, 0, scheme) else {
        return Ok(Err(no_reference(written, scheme)));
    };
    if !found.homoglyphs.is_empty() || found.reference.script == IdScript::Mixed {
        return Err(CliError::spec(latin_fix(written, &found.homoglyphs)));
    }
    let scope = found.reference.scope.as_deref();
    let reason = match target_form(&found.reference) {
        TargetForm::Canonical => None,
        TargetForm::Project => return Err(project_qualified(written)),
        TargetForm::Alias { canonical } => Some(format!(
            "`{written}` is a legacy alias: a proposal names the canonical ID `{canonical}`"
        )),
        TargetForm::Section { section } => Some(format!(
            "`{written}` names a section through its document: a proposal names the \
             section's own ID `{}`",
            qualified(scheme, scope, &section)
        )),
        TargetForm::Revision { canonical } => Some(format!(
            "`{written}` carries a revision: a proposal names the bare ID `{canonical}` \
             (`--base` pins the text)"
        )),
        TargetForm::Wiki { canonical } => Some(format!(
            "`{written}` is no bare ID: a proposal names `{canonical}`"
        )),
    };
    Ok(match reason {
        Some(reason) => Err(reason),
        None => Ok(Some(found.reference)),
    })
}

/// Step 1 over the index: the one holder of `reference` (`None`: the path
/// `written`, refused naming its ID), as `spec show` resolves it.
fn holder_path(
    input: &CheckInput,
    resolver: &Resolver<'_>,
    scheme: &IdScheme,
    written: &str,
    reference: Option<&Reference>,
) -> Result<Result<String, String>, CliError> {
    let Some(reference) = reference else {
        let known = input
            .files
            .iter()
            .find(|file| file.path == written)
            .and_then(|file| file.parsed.as_ref())
            .and_then(ParsedFile::document)
            .and_then(|document| document.id.clone());
        return Ok(Err(match known {
            Some(id) => format!(
                "`{written}` is a path: a proposal names its node by ID, `{}`",
                canonical_id(resolver, scheme, written, &id, None)
            ),
            None => format!(
                "`{written}` is a path: a proposal names its node by ID, and no indexed \
                 document with an ID lies there"
            ),
        }));
    };
    match one_holder(resolver, reference, written) {
        Ok(file) => Ok(Ok(resolver.paths()[file].to_owned())),
        Err(HolderError::Dangling(reason)) => Ok(Err(format!("`{written}` {reason}"))),
        Err(HolderError::Several(paths)) => Ok(Err(format!(
            "`{written}` is held by {} files: {}; a proposal names one node",
            paths.len(),
            paths.join(", ")
        ))),
        Err(HolderError::Project) => Err(project_qualified(written)),
    }
}

/// The node `reference` names in `parsed` (of `path`): its position, else
/// why not (declared several times; absent, an alias of the document
/// named).
fn node_position(
    resolver: &Resolver<'_>,
    scheme: &IdScheme,
    parsed: &ParsedFile,
    path: &str,
    written: &str,
    reference: &Reference,
) -> Result<usize, String> {
    match locate(parsed, &reference.id) {
        Ok(ord) => Ok(ord),
        Err(LocateError::Repeated(count)) => Err(format!(
            "`{written}` is declared {count} times in `{path}`; a proposal names one node"
        )),
        Err(LocateError::Absent) => {
            let document = parsed.document();
            let aliased = document
                .and_then(|document| document.fields.as_ref())
                .and_then(|fields| fields.aliases.as_ref())
                .is_some_and(|aliases| {
                    aliases
                        .iter()
                        .any(|alias| *alias == reference.id || alias == written)
                });
            Err(match document.and_then(|document| document.id.as_deref()) {
                Some(id) if aliased => format!(
                    "`{written}` is an alias of `{}`: a proposal names the canonical ID",
                    canonical_id(resolver, scheme, path, id, None)
                ),
                _ => format!("`{written}` is not in `{path}` as read now; run it again"),
            })
        }
    }
}

/// The intake's step 3 (canon `agent-intake`, "Rules"): `written`
/// resolved as `propose update` step 1 resolves its target, over the index
/// this call refreshed, generated and `immutable_text` holders allowed: its
/// canonical ID and its holder's path, or the refusal.
pub(crate) fn resolved_node(
    project: &ProjectRoot,
    input: &CheckInput,
    written: &str,
) -> Result<Result<(String, String), String>, CliError> {
    let written = written.trim();
    let scheme = &project.config.scheme;
    let reference = match written_reference(written, scheme)? {
        Ok(reference) => reference,
        Err(reason) => return Ok(Err(reason)),
    };
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    let path = match holder_path(input, &resolver, scheme, written, reference.as_ref())? {
        Ok(path) => path,
        Err(reason) => return Ok(Err(reason)),
    };
    // `holder_path` refuses a path: a reference is left.
    let Some(reference) = reference else {
        return Ok(Err(format!("`{written}` names no node by ID")));
    };
    let Some(parsed) = input
        .files
        .iter()
        .find(|file| file.path == path)
        .and_then(|file| file.parsed.as_ref())
    else {
        return Ok(Err(format!(
            "`{path}` could not be read: its nodes cannot be named"
        )));
    };
    if let Err(reason) = node_position(&resolver, scheme, parsed, &path, written, &reference) {
        return Ok(Err(reason));
    }
    let id = canonical_id(
        &resolver,
        scheme,
        &path,
        &reference.id,
        reference.scope.as_deref(),
    );
    Ok(Ok((id, path)))
}

/// Steps 1–4 of `propose update` over the index this call refreshed
/// (`input`), `reference` from [`written_reference`]: the target resolved
/// to one updatable node, `base` its span's hash now, `text` spliced and
/// parsed afresh, the introduced findings (never a refusal). `Ok(Err)`: the
/// refusal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn checked_update(
    project: &ProjectRoot,
    input: &CheckInput,
    written: &str,
    reference: Option<Reference>,
    base: &str,
    text: &str,
    today: &str,
    messages: &mut Vec<Message>,
) -> Result<Result<CheckedUpdate, String>, CliError> {
    let scheme = &project.config.scheme;
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    let path = match holder_path(input, &resolver, scheme, written, reference.as_ref())? {
        Ok(path) => path,
        Err(reason) => return Ok(Err(reason)),
    };
    let Some(reference) = reference else {
        return Ok(Err(format!("`{written}` names no node by ID")));
    };
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let bytes = match tree.read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return Ok(Err(format!("cannot read `{path}`: {error}"))),
    };
    if std::str::from_utf8(&bytes).is_err() {
        return Ok(Err(format!(
            "`{path}` is not UTF-8: its nodes cannot be updated"
        )));
    }
    let parsed = match panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(&path, &bytes, scheme)
    })) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(Err(format!("the spec parser failed on `{path}`"))),
    };
    let ord = match node_position(&resolver, scheme, &parsed, &path, written, &reference) {
        Ok(ord) => ord,
        Err(reason) => return Ok(Err(reason)),
    };
    let target_id = canonical_id(
        &resolver,
        scheme,
        &path,
        &reference.id,
        reference.scope.as_deref(),
    );
    if let Some(refusal) = update_refusal(&parsed, ord, scheme) {
        return Ok(Err(format!("`{target_id}`: {refusal}")));
    }

    // 2. The base.
    let node = &parsed.nodes[ord];
    let base_hash = span_hash(&bytes, node);
    let given = base.trim();
    if given != base_hash {
        return Ok(Err(format!(
            "`--base {given}` is stale: the span of `{target_id}` hashes to {base_hash} now; \
             read it again (`spec show {target_id}`) and propose against that"
        )));
    }
    let Ok(base_text) = String::from_utf8(span_bytes(&bytes, node).to_vec()) else {
        return Ok(Err(format!("the span of `{target_id}` is not UTF-8")));
    };

    // 3. The structure.
    let update = match update_file(&path, &bytes, &parsed, ord, text, scheme) {
        Ok(update) => update,
        Err(error) => return Ok(Err(format!("`{target_id}`: {error}"))),
    };
    if update.bytes == bytes {
        return Ok(Err(format!(
            "no change: `{path}` with the new text of `{target_id}` is byte for byte the \
             file as read; nothing to propose"
        )));
    }

    // 4. Validation: never a refusal.
    let root = &project.root;
    let config = NamedBytes::read(CONFIG_FILE, &root.join(CONFIG_FILE));
    let baseline = default_baseline(root);
    let diagnostics = match load_check(&config, baseline.as_ref()) {
        Ok(setup) => introduced_findings(&tree, &setup, today, &path, update.bytes.clone()),
        Err(report) => {
            messages.push(Message::Warning(format!(
                "the edit was not validated (no introduced findings stored): {}",
                report.lines(false).join("; ")
            )));
            Vec::new()
        }
    };
    Ok(Ok(CheckedUpdate {
        target_id,
        path,
        base_hash,
        base_text,
        new_text: update.text,
        diagnostics,
    }))
}

/// The proposed text: UTF-8, at most [`TEXT_MAX_BYTES`] (else a refusal,
/// `Ok(Err)`); a file that cannot be read exits 2.
fn read_text(env: &Env, text: &ProposedText) -> Result<Result<String, String>, CliError> {
    let bytes = match text {
        ProposedText::Given(bytes) => bytes.clone(),
        ProposedText::File(path) => {
            let cannot = |error: std::io::Error| {
                CliError::spec(format!("--text-file {}: {error}", path.display()))
            };
            let file = fs::File::open(env.cwd.join(path)).map_err(cannot)?;
            let mut bytes = Vec::new();
            file.take(TEXT_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(cannot)?;
            bytes
        }
    };
    if bytes.len() > TEXT_MAX_BYTES {
        return Ok(Err(format!(
            "the text is longer than {TEXT_MAX_BYTES} bytes (1 MiB)"
        )));
    }
    Ok(String::from_utf8(bytes).map_err(|error| {
        format!(
            "the text is not UTF-8 (at byte {})",
            error.utf8_error().valid_up_to()
        )
    }))
}

/// `id` qualified by `scope` when its prefix is feature-scoped.
fn qualified(scheme: &IdScheme, scope: Option<&str>, id: &str) -> String {
    match scope {
        Some(slug) if feature_scoped(scheme, id) => format!("{slug}/{id}"),
        _ => id.to_owned(),
    }
}

/// The stored form of the target `id` held in `path`: `scope/ID` as
/// written; a bare feature-scoped ID in a feature document `slug/ID`; else
/// the ID.
fn canonical_id(
    resolver: &Resolver<'_>,
    scheme: &IdScheme,
    path: &str,
    id: &str,
    scope: Option<&str>,
) -> String {
    if let Some(slug) = scope {
        return format!("{slug}/{id}");
    }
    match resolver.feature_slug(path) {
        Some(slug) if feature_scoped(scheme, id) => format!("{slug}/{id}"),
        _ => id.to_owned(),
    }
}

/// `id`'s prefix has `scope = "feature"`.
fn feature_scoped(scheme: &IdScheme, id: &str) -> bool {
    id.split_once('-')
        .and_then(|(prefix, _)| scheme.prefix(prefix))
        .is_some_and(|spec| spec.scope == IdScope::Feature)
}
