//! `spec propose create TARGET [--base HASH] --text-file F|- --rationale T
//! [--author-role R] [--author-model M] [--run ID] [--brief]` and the apply
//! of a create's new file (task spec `proposal-kinds`): a node comes into
//! existence through the queue, no file touched until the owner approves.
//!
//! - **File form**: `TARGET` a `.md` path naming nothing, no `--base`, the
//!   text the whole new file; the path clean, in the walk, not under
//!   `[paths] generated`, not `[paths] index` nor one of the index's shards
//!   (live or archive, as the root's `[[generators]]` entry with `index =
//!   true` names them), no symlink or non-directory on its way, nothing
//!   there (a dangling symlink too) nor in git's index; the text not
//!   `class: generated`. Stored with no base; `target_id` its `id:`,
//!   canonical, else its path.
//! - **Section form**: `TARGET` resolving as an update's (no `--base`:
//!   `` `<target>` exists ``), `--base` its span's hash, the span's text
//!   adding `{#ID}` sections below the target, every (ID, level) pair of
//!   the file kept in order ([`crate::propose::SpanRule::Sections`]).
//! - **New IDs**, each in text order, canonical forms compared: an
//!   `aliases_from` prefix (exit 1 naming the canonical ID), a look-alike
//!   or mixed script (exit 2 naming the Latin form), a number not as core
//!   `record_id` writes it (exit 1 naming that form), the `[decision_records]`
//!   prefix (exit 1: approving a question or a discrepancy makes those), an
//!   ID twice in the text, an ID defined or aliased in the index this call
//!   refreshed or held by a live create (exit 1 naming the holder and, for
//!   a number, the next free ID: one more than the highest of its prefix,
//!   or its `aliases_from`, in the index, the live creates and the text, in
//!   the same scope). The queue's insert checks the live creates again
//!   under its write lock ([`QueueError::Reserved`]).
//! - What `spec check` judges is a finding, stored, never a refusal:
//!   nothing is blocked by a discrepancy.
//!
//! **Apply of a new file** ([`approve_file`]), in the recorded worktree:
//! its own commit completes it; another commit with its `Proposal:` trailer
//! refuses (step 4); 2 the place; 3 the path in the walk, not generated,
//! no file of the index;
//! 4 the index refreshed, nothing at the path nor in git's index (what a
//! run killed between its steps 8 and 9 left: the two ways out named), the
//! new IDs free; 5–6 the text and its new IDs checked again; the committer
//! identity; 1 the owner's consent; 7 `approved` as a compare-and-set; 8 the
//! file created by a hard link, never replacing one; 9 its intent-to-add
//! entry and `git commit --only` of that path (a failure undoes the entry,
//! the file and the directories made); 10 the commit verified: one parent,
//! the old `HEAD`, exactly `A <path>`, its blob the proposal's text. A
//! create's sections apply as an update ([`crate::preflight`]).
//!
//! The proposer names the path and the IDs (the core knows no subject
//! domain): no prefix, kind or directory of a project is written here.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use specengine_core::check::CheckInput;
use specengine_core::check::resolve::feature_stem;
use specengine_core::create::{IdSite, WrittenId, id_sites, written_id};
use specengine_core::intake::{
    AuthorInput, DISCREPANCY_KIND, QUESTION_KIND, author_problem, rationale_problem,
};
use specengine_core::proposal::{
    Author, COMMIT_SUBJECT, CommitFacts, PROPOSAL_TRAILER, commit_message,
};
use specengine_core::record::{record_form, record_id, record_number};
use specengine_core::{Paths, is_clean_relative, is_under};
use specengine_model::{IdScheme, IdScope, ParsedFile, Shape};
use specengine_store::{
    CreateFileError, Decision, GitEnv, IndexWriter as _, NamedBytes, NewProposal, Proposal,
    ProposalFinding, ProposalKind, ProposalQueue as _, QueueError, Reservation, SpecIndex as _,
    WorkingTree, WorktreeGit, create_file, default_baseline, introduced_findings, load_check,
    load_config, patch_hash,
};

use crate::apply::{
    ApproveRequest, Consent, Made, complete, failed, made_anyway, noted, recorded_already, verify,
};
use crate::corpus::indexed;
use crate::decide::{exists, path_free};
use crate::location::{OpenIndex, open_index};
use crate::preflight::{
    Placed, StepFailure, adds_file, completes_when, completing, place_step, place_unchanged,
    recorded_project, trailer_lookup,
};
use crate::project::{CONFIG_FILE, ProjectRoot};
use crate::proposals::{
    Preview, ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, briefed, checked_now,
    escaped_error, open_context, queue_cannot, queue_refusal, top_of, with_diff,
};
use crate::propose::{
    CheckedUpdate, ProposedText, SectionsFound, SpanRule, checked_span, exists_refusal,
    feature_scoped, is_path_target, read_text, written_reference,
};
use crate::refresh::refresh;
use crate::review::previewed;
use crate::show::latin_fix;
use crate::stage::Staging;
use crate::task::bound_task;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line, store_error};

/// What a create refuses as its writer.
const CREATE: &str = "a create";

/// `spec propose create` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRequest {
    /// `TARGET` as given: a `.md` path naming nothing (a new file), or a
    /// node as `propose update` names it (new sections in its span).
    pub target: String,
    /// `--base HASH`: the target's span hash (sections only).
    pub base: Option<String>,
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

/// `spec propose create`: checks and stores one proposal of kind `create`.
pub fn propose_create(
    env: &Env,
    globals: &Globals,
    request: &CreateRequest,
) -> Result<ProposalOutcome, CliError> {
    propose_create_with_task(env, globals, request, None)
}

/// `spec propose create --brief` (MCP `propose_change` of kind `create`):
/// [`propose_create`], its answer brief.
pub fn propose_create_brief(
    env: &Env,
    globals: &Globals,
    request: &CreateRequest,
) -> Result<ProposalOutcome, CliError> {
    propose_create(env, globals, request).map(briefed)
}

/// [`propose_create`] with `--task T` (canon `tasks`, "Task-bound
/// proposals"): bound to that task of this repository.
pub fn propose_create_with_task(
    env: &Env,
    globals: &Globals,
    request: &CreateRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    run_create(env, globals, request, task).map_err(escaped_error)
}

/// [`propose_create_brief`] with `--task T`.
pub fn propose_create_brief_with_task(
    env: &Env,
    globals: &Globals,
    request: &CreateRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    propose_create_with_task(env, globals, request, task).map(briefed)
}

/// What steps 2–3 make of a target and a text.
struct Checked {
    /// The section form's checked span; `None` for a new file.
    span: Option<CheckedUpdate>,
    /// Root-relative.
    path: String,
    /// The stored text: the new file byte for byte, or the span as spliced.
    new_text: String,
    /// The file as it would be: what is validated.
    patched: Vec<u8>,
    /// The new ID sites, in text order, each with what it is.
    sites: Vec<(IdSite, WrittenId)>,
    /// The file's feature slug (its feature-scoped IDs are `slug/ID`).
    slug: Option<String>,
}

fn run_create(
    env: &Env,
    globals: &Globals,
    request: &CreateRequest,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
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
    let task = match task {
        Some(written) => match bound_task(&context, written, &place)? {
            Ok(id) => Some(id),
            Err(reason) => return refuse(reason, messages),
        },
        None => None,
    };
    let text = match read_text(env, &request.text)? {
        Ok(text) => text,
        Err(reason) => return refuse(reason, messages),
    };
    if let Some(problem) = rationale_problem("rationale", &request.rationale) {
        return refuse(problem.to_string(), messages);
    }

    // 2–3. The form, the target and the text, over the index refreshed now.
    let written = request.target.trim();
    let input = indexed(env, &context.project, &mut messages, false)?;
    let project = &context.project;
    let today = &now[..10];
    let new_file = is_path_target(written) && !input.files.iter().any(|file| file.path == written);
    let checked = if new_file {
        checked_file(
            project,
            &context.git,
            written,
            request.base.as_deref(),
            &text,
        )?
    } else {
        checked_sections(
            project,
            &input,
            written,
            request.base.as_deref(),
            &text,
            today,
            &mut messages,
        )?
    };
    let checked = match checked {
        Ok(checked) => checked,
        Err(reason) => return refuse(reason, messages),
    };

    // 4. The new IDs.
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

    // 5. Validation: never a refusal.
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let diagnostics = validated(
        project,
        &tree,
        today,
        &checked.path,
        checked.patched.clone(),
        &mut messages,
    );

    // 7. Stored, bound to the place found first; a live create taking an ID
    // meanwhile is step 4's refusal.
    let new = match &checked.span {
        Some(span) => NewProposal {
            kind: ProposalKind::Create,
            patch_hash: patch_hash(&span.target_id, &span.base_hash, &checked.new_text),
            target_id: span.target_id.clone(),
            target_path: checked.path.clone(),
            place,
            base_hash: Some(span.base_hash.clone()),
            base_text: Some(span.base_text.clone()),
            new_text: checked.new_text.clone(),
            rationale: request.rationale.clone(),
            author,
            diagnostics,
            new_ids: new_ids.clone(),
        },
        None => {
            // A new file is named by its `id:`, else by its path.
            let declares = checked
                .sites
                .first()
                .is_some_and(|(site, form)| site.level.is_none() && *form != WrittenId::NoId);
            let target_id = match (declares, new_ids.first()) {
                (true, Some(id)) => id.clone(),
                _ => checked.path.clone(),
            };
            NewProposal {
                kind: ProposalKind::Create,
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
            }
        }
    };
    let created = match context.queue.create_with_task(&new, task.as_deref(), now) {
        Ok(created) => created,
        Err(QueueError::TaskRefused { reason, .. }) => {
            return refuse(format!("--task: {reason}"), messages);
        }
        Err(QueueError::Reserved { id, by }) => {
            // Step 4's refusal, its holder's state read again.
            let reserved = context.queue.reserved().map_err(queue_cannot)?;
            let mut reason = match reserved.iter().find(|held| held.id == id && held.by == by) {
                Some(held) => reserved_by(held),
                None => format!("`{id}` is reserved by `{by}`, a live create"),
            };
            if let Some(next) = named.next_free(&id, &corpus, &reserved, &new_ids) {
                reason.push_str(&format!("; the next free is `{next}`"));
            }
            reason.push_str("; nothing stored");
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

/// Steps 2–3 of the file form: `path` (a `.md` path no indexed file has),
/// no `base`, the path free for a new file of the walk, the text not
/// generated; `Ok(Err)`: the refusal.
fn checked_file(
    project: &ProjectRoot,
    git: &WorktreeGit,
    path: &str,
    base: Option<&str>,
    text: &str,
) -> Result<Result<Checked, String>, CliError> {
    if base.is_some() {
        return Ok(Err(format!(
            "nothing at `{path}`: a new file is written against no base; drop --base"
        )));
    }
    let paths = &project.config.paths;
    if let Err(reason) = new_file_path(paths, &index_files(&project.root, paths), path) {
        return Ok(Err(reason));
    }
    if let Err(reason) = path_free(
        git,
        &project.root,
        path,
        path,
        CREATE,
        "to add ID sections to it, name its span_hash with --base",
        "nothing stored",
    ) {
        return Ok(Err(reason));
    }
    let scheme = &project.config.scheme;
    let Some(parsed) = parsed_text(path, text, scheme) else {
        return Ok(Err(format!(
            "the spec parser failed on the text of `{path}`"
        )));
    };
    if is_generated(&parsed) {
        return Ok(Err(generated_refusal(path)));
    }
    Ok(Ok(Checked {
        span: None,
        path: path.to_owned(),
        new_text: text.to_owned(),
        patched: text.as_bytes().to_vec(),
        sites: classified(id_sites(text, scheme), scheme),
        slug: feature_stem(&paths.features, path).map(str::to_owned),
    }))
}

/// Step 2's path rules of a new file: clean, `.md`, in the walk, not under
/// `[paths] generated`, none of the index's files (`index`: [`index_files`]).
fn new_file_path(paths: &Paths, index: &[(String, &str)], path: &str) -> Result<(), String> {
    if !is_clean_relative(path) {
        return Err(format!(
            "`{path}` is no clean root-relative path (no leading `/`, no `.`, `..` or empty \
             component): a new file is named by its path from the project root; nothing stored"
        ));
    }
    if !is_path_target(path) || !paths.in_walk_scope(path) {
        return Err(format!(
            "`{path}` is no file the walk would list (outside the `[paths]` roots, excluded, \
             or a `.`-named component): a new spec file is a document of the corpus; nothing \
             stored"
        ));
    }
    if is_under(path, &paths.generated) {
        return Err(format!(
            "`{path}` lies under `[paths] generated` (`{}`): only a registered generator writes \
             there, never a proposal; nothing stored",
            paths.generated
        ));
    }
    if let Some((_, what)) = index.iter().find(|(file, _)| file == path) {
        return Err(format!(
            "`{path}` is {what}: only `spec export index` writes it, never a proposal; nothing \
             stored"
        ));
    }
    Ok(())
}

/// Steps 2–3 of the section form: the target resolved as an update's, its
/// span's hash `base`, the text adding sections; `Ok(Err)`: the refusal.
fn checked_sections(
    project: &ProjectRoot,
    input: &CheckInput,
    written: &str,
    base: Option<&str>,
    text: &str,
    today: &str,
    messages: &mut Vec<Message>,
) -> Result<Result<Checked, String>, CliError> {
    let scheme = &project.config.scheme;
    let reference = match written_reference(written, scheme)? {
        Ok(reference) => reference,
        Err(reason) => return Ok(Err(reason)),
    };
    let span = match checked_span(
        project,
        input,
        written,
        reference,
        base,
        text,
        today,
        messages,
        SpanRule::Sections,
    )? {
        Ok(span) => span,
        Err(reason) => return Ok(Err(reason)),
    };
    let Some(found) = span.sections.clone() else {
        return Ok(Err(exists_refusal(written)));
    };
    let sites = section_sites(&found, scheme);
    let slug = feature_stem(&project.config.paths.features, &span.path).map(str::to_owned);
    Ok(Ok(Checked {
        path: span.path.clone(),
        new_text: span.new_text.clone(),
        patched: found.after.clone(),
        sites,
        slug,
        span: Some(span),
    }))
}

/// The new ID sites of a section form, in text order: each added section's
/// heading, and each ID-shaped `{#…}` attribute the parser did not take as
/// a definition (an alias prefix, a look-alike of one) that the span did
/// not hold before.
fn section_sites(found: &SectionsFound, scheme: &IdScheme) -> Vec<(IdSite, WrittenId)> {
    let after_text = String::from_utf8_lossy(&found.after);
    let before_text = String::from_utf8_lossy(&found.before);
    let inserted_end = found.after.len() - (found.before.len() - found.base_span.end);
    let inside =
        |site: &IdSite, start: usize, end: usize| site.offset >= start && site.offset < end;
    let headings: BTreeSet<usize> = found
        .parsed
        .nodes
        .iter()
        .filter_map(|node| node.heading.map(|span| span.start))
        .collect();
    let added: BTreeSet<usize> = found
        .added
        .iter()
        .filter_map(|&at| found.parsed.nodes.get(at)?.heading.map(|span| span.start))
        .collect();
    // The attributes no definition took, in the span before: each may stay.
    let mut held: Vec<String> = id_sites(&before_text, scheme)
        .into_iter()
        .filter(|site| site.level.is_some())
        .filter(|site| inside(site, found.base_span.start, found.base_span.end))
        .map(|site| site.written)
        .collect();
    let mut sites = Vec::new();
    for site in id_sites(&after_text, scheme) {
        if site.level.is_none() || !inside(&site, found.base_span.start, inserted_end) {
            continue;
        }
        if added.contains(&site.offset) {
            let form = written_id(&site.written, scheme);
            sites.push((site, form));
            continue;
        }
        if headings.contains(&site.offset) {
            // A section the span held before.
            continue;
        }
        if let Some(at) = held.iter().position(|written| *written == site.written) {
            held.remove(at);
            continue;
        }
        let form = written_id(&site.written, scheme);
        if form != WrittenId::NoId {
            sites.push((site, form));
        }
    }
    sites
}

/// `sites` with what each is, those that are no ID left out.
fn classified(sites: Vec<IdSite>, scheme: &IdScheme) -> Vec<(IdSite, WrittenId)> {
    sites
        .into_iter()
        .map(|site| {
            let form = written_id(&site.written, scheme);
            (site, form)
        })
        .filter(|(_, form)| *form != WrittenId::NoId)
        .collect()
}

/// `text` parsed as the file `path`; `None` when the parser panics.
fn parsed_text(path: &str, text: &str, scheme: &IdScheme) -> Option<ParsedFile> {
    panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, text.as_bytes(), scheme)
    }))
    .ok()
}

/// The document declares `class: generated`.
fn is_generated(parsed: &ParsedFile) -> bool {
    let generated = specengine_core::check::DocClass::Generated.as_str();
    parsed
        .document()
        .and_then(|document| document.fields.as_ref())
        .and_then(|fields| fields.class.as_deref())
        == Some(generated)
}

fn generated_refusal(path: &str) -> String {
    format!(
        "the text of `{path}` is a `class: generated` document: only its registered generator \
         writes one, never a proposal; nothing stored"
    )
}

/// The findings the new file or span introduces in the project's check,
/// the tree checked as is and with it; never a refusal.
fn validated(
    project: &ProjectRoot,
    tree: &WorkingTree,
    today: &str,
    path: &str,
    bytes: Vec<u8>,
    messages: &mut Vec<Message>,
) -> Vec<ProposalFinding> {
    let config = NamedBytes::read(CONFIG_FILE, &project.root.join(CONFIG_FILE));
    let baseline = default_baseline(&project.root);
    match load_check(&config, baseline.as_ref()) {
        Ok(setup) => introduced_findings(tree, &setup, today, path, bytes),
        Err(report) => {
            messages.push(Message::Warning(format!(
                "the edit was not validated (no introduced findings stored): {}",
                report.lines(false).join("; ")
            )));
            Vec::new()
        }
    }
}

/// `id` (a Latin ID) in its canonical form for a file whose feature slug is
/// `slug`: `slug/ID` for a feature-scoped prefix in a feature document,
/// else the ID.
pub(crate) fn canonical_in(scheme: &IdScheme, slug: Option<&str>, id: &str) -> String {
    match slug {
        Some(slug) if feature_scoped(scheme, id) => format!("{slug}/{id}"),
        _ => id.to_owned(),
    }
}

/// Who holds an ID in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Holder {
    /// Root-relative.
    pub path: String,
    /// The holding document's `id:`.
    pub document: Option<String>,
    /// Held as an `aliases:` entry, not a definition.
    pub alias: bool,
}

impl Holder {
    /// `` `<id>` is defined in `<path>` (`id: X`) `` and the like.
    pub(crate) fn taken(&self, id: &str) -> String {
        let document = match &self.document {
            Some(document) => format!("`id: {document}`"),
            None => "no `id:`".to_owned(),
        };
        let how = if self.alias {
            "is an alias in"
        } else {
            "is defined in"
        };
        format!("`{id}` {how} `{}` ({document})", self.path)
    }
}

/// The IDs the index defines or lists in `aliases:`, canonical, with
/// their holders (the first, by path).
pub(crate) struct Corpus {
    held: BTreeMap<String, Holder>,
}

impl Corpus {
    /// The canonical IDs of `input` under `scheme`: every document's `id:`
    /// and section's `{#ID}` (`slug/ID` for a feature-scoped prefix in a
    /// feature document), and every `aliases:` entry (an `aliases_from`
    /// prefix read as its canonical prefix).
    pub(crate) fn of(input: &CheckInput, scheme: &IdScheme, paths: &Paths) -> Self {
        let mut files: Vec<_> = input.files.iter().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut held = BTreeMap::new();
        for file in files {
            let Some(parsed) = &file.parsed else {
                continue;
            };
            let slug = feature_stem(&paths.features, &file.path);
            let document = parsed.document().and_then(|node| node.id.clone());
            let holder = |alias: bool| Holder {
                path: file.path.clone(),
                document: document.clone(),
                alias,
            };
            for node in &parsed.nodes {
                if let Some(id) = &node.id {
                    held.entry(canonical_in(scheme, slug, id))
                        .or_insert_with(|| holder(false));
                }
            }
            let aliases = parsed
                .document()
                .and_then(|node| node.fields.as_ref())
                .and_then(|fields| fields.aliases.as_ref());
            for alias in aliases.into_iter().flatten() {
                let id = match written_id(alias.trim(), scheme) {
                    WrittenId::Alias { canonical } => canonical,
                    WrittenId::Id(id) => id,
                    WrittenId::NoId | WrittenId::LookAlike { .. } => alias.trim().to_owned(),
                };
                held.entry(canonical_in(scheme, slug, &id))
                    .or_insert_with(|| holder(true));
            }
        }
        Self { held }
    }

    /// Who holds `id` (canonical), if anyone.
    pub(crate) fn holder(&self, id: &str) -> Option<&Holder> {
        self.held.get(id)
    }

    fn ids(&self) -> impl Iterator<Item = &str> {
        self.held.keys().map(String::as_str)
    }
}

/// Step 4's rules: the scheme, the `[decision_records]` prefix and the
/// file's feature slug.
struct NewIds<'a> {
    scheme: &'a IdScheme,
    decision_prefix: Option<&'a str>,
    slug: Option<&'a str>,
}

impl NewIds<'_> {
    /// The canonical new IDs of `sites`, in text order, or the first
    /// refusal (`Ok(Err)`); a look-alike exits 2.
    fn checked(
        &self,
        sites: &[(IdSite, WrittenId)],
        corpus: &Corpus,
        reserved: &[Reservation],
    ) -> Result<Result<Vec<String>, String>, CliError> {
        let all: Vec<String> = sites
            .iter()
            .filter_map(|(_, form)| match form {
                WrittenId::Id(id) => Some(self.canonical(id)),
                _ => None,
            })
            .collect();
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut ids = Vec::new();
        for (site, form) in sites {
            let IdSite { written, line, .. } = site;
            let id = match form {
                WrittenId::NoId => continue,
                WrittenId::Alias { canonical } => {
                    return Ok(Err(format!(
                        "`{written}` (line {line}) is written with a legacy `aliases_from` \
                         prefix: a new ID is written with its canonical prefix, `{canonical}`; \
                         nothing stored"
                    )));
                }
                WrittenId::LookAlike { homoglyphs } => {
                    return Err(CliError::spec(format!(
                        "line {line}: {}; nothing stored",
                        latin_fix(written, homoglyphs)
                    )));
                }
                WrittenId::Id(id) => id,
            };
            let spec = id
                .split_once('-')
                .and_then(|(prefix, _)| self.scheme.prefix(prefix));
            if let Some(form) = spec.and_then(|spec| record_form(id, spec)) {
                return Ok(Err(format!(
                    "`{id}` (line {line}): its prefix's numbers are written as `{form}`; nothing \
                     stored"
                )));
            }
            if let (Some(prefix), Some(spec)) = (self.decision_prefix, spec)
                && spec.prefix == prefix
            {
                return Ok(Err(format!(
                    "`{id}`: these records are made by `spec approve` of a `{QUESTION_KIND}` or \
                     `{DISCREPANCY_KIND}`; nothing stored"
                )));
            }
            let canonical = self.canonical(id);
            if let Some(first) = seen.get(&canonical) {
                return Ok(Err(format!(
                    "`{canonical}` is defined twice in the text (lines {first} and {line}); \
                     nothing stored"
                )));
            }
            seen.insert(canonical.clone(), *line);
            let taken = if let Some(holder) = corpus.holder(&canonical) {
                Some(holder.taken(&canonical))
            } else {
                reserved
                    .iter()
                    .find(|held| held.id == canonical)
                    .map(reserved_by)
            };
            if let Some(mut reason) = taken {
                if let Some(next) = self.next_free(&canonical, corpus, reserved, &all) {
                    reason.push_str(&format!("; the next free is `{next}`"));
                }
                reason.push_str("; nothing stored");
                return Ok(Err(reason));
            }
            ids.push(canonical);
        }
        Ok(Ok(ids))
    }

    fn canonical(&self, id: &str) -> String {
        canonical_in(self.scheme, self.slug, id)
    }

    /// For a number-shape `id` (canonical): one more than the highest
    /// number of its prefix, or its `aliases_from`, in the same scope among
    /// the index's IDs, the live creates' and `text`'s, as core
    /// `record_id` writes it.
    fn next_free(
        &self,
        id: &str,
        corpus: &Corpus,
        reserved: &[Reservation],
        text: &[String],
    ) -> Option<String> {
        let (scope, bare) = split_scope(id);
        let spec = self.scheme.prefix(bare.split_once('-')?.0)?;
        if spec.shape != Shape::Number {
            return None;
        }
        let candidates = corpus
            .ids()
            .chain(reserved.iter().map(|held| held.id.as_str()))
            .chain(text.iter().map(String::as_str));
        let mut highest = 0;
        for candidate in candidates {
            let (their_scope, their_bare) = split_scope(candidate);
            if their_scope != scope {
                continue;
            }
            if let Some(number) = record_number(their_bare, spec) {
                highest = highest.max(number);
            }
        }
        let next = highest.checked_add(1)?;
        let free = record_id(&spec.prefix, spec.width.unwrap_or(1), next);
        Some(match (scope, spec.scope) {
            (Some(slug), IdScope::Feature) => format!("{slug}/{free}"),
            _ => free,
        })
    }
}

/// `` `<id>` is reserved by `PR-…` (<status>), a live create ``: step 4's
/// refusal of an ID a live create holds, its holder and state named.
fn reserved_by(held: &Reservation) -> String {
    format!(
        "`{}` is reserved by `{}` ({}), a live create",
        held.id, held.by, held.status
    )
}

/// The files `spec export index` writes, with what each is: `[paths]
/// index`, and the shards (live and archive) of the `index = true`
/// generator when the root's check tables read.
fn index_files(root: &Path, paths: &Paths) -> Vec<(String, &'static str)> {
    let mut files = Vec::new();
    if let Some(index) = &paths.index {
        files.push((index.clone(), "the project's index (`[paths] index`)"));
    }
    let config = NamedBytes::read(CONFIG_FILE, &root.join(CONFIG_FILE));
    if let Ok((_, check)) = load_config(&config)
        && let Some(generator) = check.index_generator()
    {
        for shard in &generator.shards {
            let what = if shard.is_archive() {
                "the index's archive shard"
            } else {
                "a shard of the index"
            };
            files.push((shard.path.clone(), what));
        }
    }
    files
}

/// `slug/ID` as its slug and ID; a bare ID has none.
fn split_scope(id: &str) -> (Option<&str>, &str) {
    match id.split_once('/') {
        Some((slug, bare)) => (Some(slug), bare),
        None => (None, id),
    }
}

/// A create's new file passed apply steps 2–6: what steps 7–10 act on.
pub(crate) struct PreparedFile {
    pub git: WorktreeGit,
    /// The branch's commit when checked.
    pub head: String,
    /// The path from the worktree's top.
    pub top_path: String,
    /// The recorded root's project and its index.
    pub recorded: ProjectRoot,
    pub index: OpenIndex,
    pub tree: WorkingTree,
}

/// Apply steps 2–6 of a create's new file, writing nothing but the data
/// directory: `spec approve`'s checks and `spec review`'s preview
/// (`applies`).
pub(crate) fn prepare_file(
    env: &Env,
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> Result<PreparedFile, StepFailure> {
    let path = proposal.target_path.as_str();
    let Placed { git, top, head } = place_step(git_env, proposal)?;

    // Step 3: the recorded root, the path.
    let place = &proposal.place;
    let root = if place.root_rel.is_empty() {
        top.clone()
    } else {
        top.join(&place.root_rel)
    };
    let recorded =
        recorded_project(&root, &context.slug).map_err(|reason| StepFailure::cannot(3, reason))?;
    let paths = &recorded.config.paths;
    new_file_path(paths, &index_files(&recorded.root, paths), path).map_err(|reason| {
        StepFailure::refused(3, reason.replace("nothing stored", "nothing changed"))
    })?;
    let tree = WorkingTree::new(&recorded.root, paths)
        .map_err(|error| StepFailure::cannot(3, error.to_string()))?;

    // Step 4: the index refreshed, the path free, the new IDs free.
    let mut index =
        open_index(env, &recorded).map_err(|error| StepFailure::cannot(4, error.message))?;
    let (_, warnings) = refresh(&mut index.index, &recorded, false)
        .map_err(|error| StepFailure::cannot(4, error.message))?;
    messages.extend(warnings);
    let top_path = top_of(proposal, path);
    path_free(
        &git,
        &recorded.root,
        path,
        &top_path,
        CREATE,
        "nothing changed",
        "nothing changed",
    )
    .map_err(|reason| {
        let reason = left_behind(&git, &recorded.root, &top_path, proposal).unwrap_or(reason);
        StepFailure::refused(4, reason)
    })?;
    let input = index
        .index
        .indexed_input()
        .map_err(|error| StepFailure::cannot(4, error.to_string()))?;
    let scheme = &recorded.config.scheme;
    let corpus = Corpus::of(&input, scheme, paths);
    for id in &proposal.new_ids {
        if let Some(holder) = corpus.holder(id) {
            return Err(StepFailure::refused(
                4,
                format!("{}; nothing changed", holder.taken(id)),
            ));
        }
    }

    // Steps 5–6: the text, its new IDs as proposed (no live create's).
    let Some(parsed) = parsed_text(path, &proposal.new_text, scheme) else {
        return Err(StepFailure::refused(
            5,
            format!("the spec parser failed on the text of `{path}`"),
        ));
    };
    if is_generated(&parsed) {
        return Err(StepFailure::refused(
            5,
            generated_refusal(path).replace("nothing stored", "nothing changed"),
        ));
    }
    let named = NewIds {
        scheme,
        decision_prefix: recorded
            .config
            .decision_records
            .as_ref()
            .map(|table| table.prefix.as_str()),
        slug: feature_stem(&paths.features, path),
    };
    let sites = classified(id_sites(&proposal.new_text, scheme), scheme);
    let ids = match named.checked(&sites, &corpus, &[]) {
        Ok(Ok(ids)) => ids,
        Ok(Err(reason)) => {
            return Err(StepFailure::refused(
                6,
                reason.replace("nothing stored", "nothing changed"),
            ));
        }
        Err(error) => {
            let reason = error
                .message
                .strip_prefix("spec: ")
                .unwrap_or(&error.message);
            return Err(StepFailure::cannot(
                6,
                reason.replace("nothing stored", "nothing changed"),
            ));
        }
    };
    if ids != proposal.new_ids {
        return Err(StepFailure::refused(
            6,
            format!(
                "the text of `{path}` defines {} under the recorded root's `[ids]` now, the \
                 proposal {}; nothing changed",
                listed(&ids),
                listed(&proposal.new_ids)
            ),
        ));
    }
    Ok(PreparedFile {
        git,
        head,
        top_path,
        recorded,
        index,
        tree,
    })
}

/// IDs as a refusal lists them: `` `A`, `B` ``, or `none`.
fn listed(ids: &[String]) -> String {
    if ids.is_empty() {
        return "none".to_owned();
    }
    ids.iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Step 4's `exists` naming the two ways out when the path holds what a
/// run killed between its steps 8 and 9 left: a regular file whose bytes
/// are the proposal's text, its index entry intent-to-add. `None`
/// otherwise (the plain refusal stands).
fn left_behind(
    git: &WorktreeGit,
    root: &Path,
    top_path: &str,
    proposal: &Proposal,
) -> Option<String> {
    let path = proposal.target_path.as_str();
    let at = root.join(path);
    let regular = fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_file());
    if !regular {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(&at)
        .ok()?
        .take(proposal.new_text.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes != proposal.new_text.as_bytes() || !git.is_intent_to_add(top_path).ok()? {
        return None;
    }
    let id = &proposal.id;
    Some(exists(
        path,
        CREATE,
        &format!(
            "it holds `{id}`'s text as an interrupted apply left it, with an intent-to-add entry \
             in git's index; two ways out in {}: commit it by hand (`git commit --only --trailer \
             '{PROPOSAL_TRAILER}: {id}' -m '{COMMIT_SUBJECT} {id}' -- {top_path}`), then `spec \
             approve {id}` completes it; or `git rm --cached -- {top_path}`, delete the file, \
             then `spec approve {id}` writes it again; nothing changed",
            proposal.place.worktree
        ),
    ))
}

/// `spec approve` of a create's new file, `open` or `approved`
/// (`run_approve` has refused an applied or rejected one and a decision
/// flag): see the module documentation.
pub(crate) fn approve_file(
    env: &Env,
    request: &ApproveRequest,
    context: &mut QueueContext,
    proposal: &Proposal,
    staging: &Staging,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    const COMMAND: QueueCommand = QueueCommand::Approve;
    let id = proposal.id.as_str();
    let read = proposal.seen();
    let mut messages = staging.notes();

    // Its own commit completes it; another one with its trailer refuses a
    // new file. A lookup git cannot make is a note.
    let mut unknown = None;
    match trailer_lookup(&request.git, context, proposal) {
        Ok(found) => {
            if let Some(own) = completing(&found) {
                let commit = own.commit.clone();
                return complete(
                    env, request, context, proposal, &commit, staging, consent, messages,
                );
            }
            if let Some(first) = found.first() {
                let failure = StepFailure::refused(
                    4,
                    format!(
                        "the commit {} on `{}` carries `{PROPOSAL_TRAILER}: {id}` but {}: a new \
                         apply would write the file again; {}; no new apply",
                        first.commit,
                        proposal.place.branch,
                        first.not_completing.as_deref().unwrap_or_default(),
                        completes_when(proposal)
                    ),
                );
                return failed(
                    context,
                    request,
                    proposal,
                    None,
                    failure,
                    Some(Preview::Unavailable),
                    messages,
                );
            }
        }
        Err(error) => {
            let note = one_line(&format!(
                "cannot tell whether `{id}` has its commit on `{}`: {error}",
                proposal.place.branch
            ));
            messages.push(Message::Note(note.clone()));
            unknown = Some(note);
        }
    }

    // Until step 7 the run holds nothing: a refusal only logs.
    let prepared = match prepare_file(env, &request.git, context, proposal, &mut messages) {
        Ok(prepared) => prepared,
        Err(mut failure) => {
            if failure.exit == Exit::CannotRun
                && let Some(unknown) = &unknown
            {
                failure.reason.push_str(&format!("; {unknown}"));
            }
            return failed(
                context,
                request,
                proposal,
                None,
                failure,
                Some(Preview::Unavailable),
                messages,
            );
        }
    };
    let PreparedFile {
        git,
        head,
        top_path,
        recorded,
        mut index,
        tree,
    } = prepared;
    let place = &proposal.place;
    let decided_by = match git.committer_ident() {
        Ok(ident) => ident,
        Err(error) => {
            let failure = StepFailure::cannot(
                7,
                format!(
                    "no git identity in {}: {error}; set user.name and user.email",
                    place.worktree
                ),
            );
            return failed(
                context,
                request,
                proposal,
                None,
                failure,
                Some(Preview::Applies),
                messages,
            );
        }
    };

    // Step 1: the owner's consent.
    let question = format!(
        "{}apply {id} to {top_path} on {} in {} (new file{})? [y/N]",
        staging.preface(None),
        place.branch,
        place.worktree,
        staging.mark()
    );
    if !consent(&noted(unknown.as_deref(), &escape_controls(&question))) {
        let mut document = with_diff(proposal, &request.git, &context.data_dir);
        document.preview = Some(Preview::Applies);
        return Ok(ProposalOutcome::refused(
            COMMAND,
            document,
            &format!("`{id}` not applied: the answer was not `y`; nothing changed"),
            messages,
        ));
    }

    // Step 7: a compare-and-set on the state read (the stage shown too).
    let decision = Decision {
        decided_by: decided_by.clone(),
        note: request.note.clone(),
        staged_at: staging.staged_at(),
    };
    let approved = match context
        .queue
        .approve_from(id, &read, &decision, &request.now)
    {
        Ok(approved) => approved,
        Err(
            error @ (QueueError::Changed { .. }
            | QueueError::Status { .. }
            | QueueError::Invalid(_)),
        ) => {
            let replaced = match error {
                QueueError::Changed { .. } => staging.replaced(context, id),
                _ => None,
            };
            let reason = replaced.unwrap_or_else(|| format!("{error}; nothing written"));
            let failure = StepFailure::refused(7, reason);
            return failed(
                context,
                request,
                proposal,
                None,
                failure,
                Some(Preview::Applies),
                messages,
            );
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    let held = approved.seen();
    let after = |context: &mut QueueContext, failure: StepFailure, messages: Vec<Message>| {
        failed(
            context,
            request,
            proposal,
            Some(&held),
            failure,
            Some(Preview::Applies),
            messages,
        )
    };

    // Step 8: the place and the path as checked, then the new file.
    let path = proposal.target_path.as_str();
    if let Err(failure) = place_unchanged(&git, &head, place) {
        return after(context, failure, messages);
    }
    if let Err(reason) = path_free(
        &git,
        &recorded.root,
        path,
        &top_path,
        CREATE,
        "nothing written",
        "nothing written",
    ) {
        return after(context, StepFailure::refused(8, reason), messages);
    }
    let created = match create_file(&recorded.root, path, proposal.new_text.as_bytes()) {
        Ok(created) => created,
        Err(CreateFileError::Exists) => {
            return after(
                context,
                StepFailure::refused(8, exists(path, CREATE, "nothing written")),
                messages,
            );
        }
        Err(error) => {
            let reason = format!("cannot create `{path}`: {error}; nothing written");
            return after(context, StepFailure::refused(8, reason), messages);
        }
    };

    // Step 9: its index entry, then the commit of that one path.
    let message = commit_message(&CommitFacts {
        id,
        rationale: &proposal.rationale,
        decided_by: &decided_by,
        author: &proposal.author,
        base_commit: &place.base_commit,
    });
    let committed = git
        .intent_to_add(&top_path)
        .and_then(|()| git.commit_only(&context.data_dir, &message, &top_path));
    if let Err(error) = committed {
        let branch = &place.branch;
        match made_anyway(&git, &head, proposal) {
            Made::Commit { tip, on_branch } => {
                let moved = if on_branch {
                    format!("`{branch}` moved to {tip}")
                } else {
                    format!("HEAD moved to {tip}")
                };
                messages.push(Message::Warning(format!(
                    "the commit reported a failure, yet {moved}: {error}"
                )));
            }
            Made::Nothing { foreign } => {
                if let Err(cause) = git.remove_cached(&top_path) {
                    messages.push(Message::Warning(format!(
                        "cannot drop the index entry of `{path}` in {}: {cause}; `git rm \
                         --cached -- {top_path}` drops it",
                        place.worktree
                    )));
                }
                for left in created.remove() {
                    messages.push(Message::Warning(left));
                }
                let foreign = foreign.map_or_else(String::new, |tip| {
                    format!(
                        "; `{branch}` moved to {tip}, which has no `{PROPOSAL_TRAILER}: {id}` \
                         trailer (not this apply's commit)"
                    )
                });
                let failure = StepFailure::refused(
                    9,
                    format!("the commit of `{path}` failed, the file removed: {error}{foreign}"),
                );
                return after(context, failure, messages);
            }
        }
    }

    // Step 10: this run's commit adds the file and nothing else.
    let adds_the_file = |commit: &str| -> Result<(), String> {
        adds_file(
            &git,
            &head,
            commit,
            &top_path,
            &proposal.new_text,
            "the proposal's text",
        )
        .map_err(|why| format!("it {why}"))
    };
    let commit = match verify(&git, &head, &approved, &adds_the_file) {
        Ok(commit) => commit,
        Err(reason) => return after(context, StepFailure::refused(10, reason), messages),
    };
    let applied = match context
        .queue
        .applied_with(id, &commit, &decision, &request.now)
    {
        Ok(applied) => applied,
        Err(error) => match recorded_already(context, &error, &commit, &mut messages) {
            Some(stored) => stored,
            None => {
                let reason = queue_refusal(error)?;
                let document = with_diff(&approved, &request.git, &context.data_dir);
                return Ok(ProposalOutcome::refused(
                    COMMAND, document, &reason, messages,
                ));
            }
        },
    };
    if let Err(error) = index
        .index
        .update_paths(&tree, &recorded.config.scheme, &[path])
    {
        messages.push(Message::Warning(format!(
            "the index was not updated after the commit: {error}; the next command updates it"
        )));
    }
    let document = with_diff(&applied, &request.git, &context.data_dir);
    Ok(ProposalOutcome::done(COMMAND, document, messages))
}
