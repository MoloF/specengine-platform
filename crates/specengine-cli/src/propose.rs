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
use specengine_core::check::Resolver;
use specengine_core::patch::{
    HolderError, LocateError, TargetForm, locate, one_holder, span_bytes, target_form,
    update_refusal,
};
use specengine_core::proposal::Author;
use specengine_model::{IdScheme, IdScope, IdScript, ParsedFile, grammar};
use specengine_store::{
    GitEnv, NamedBytes, NewProposal, ProposalKind, ProposalQueue as _, Source as _, WorkingTree,
    default_baseline, introduced_findings, load_check, patch_hash, span_hash, update_file,
};

use crate::corpus::indexed;
use crate::project::CONFIG_FILE;
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, checked_now, escaped_error, open_context,
    queue_cannot,
};
use crate::review::previewed;
use crate::show::{latin_fix, no_reference, project_qualified};
use crate::{CliError, Env, Globals, Message, store_error};

/// The most bytes of a proposed text.
pub const TEXT_MAX_BYTES: usize = 1 << 20;

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

/// `spec propose update`: checks and stores one proposal.
pub fn propose(
    env: &Env,
    globals: &Globals,
    request: &ProposeRequest,
) -> Result<ProposalOutcome, CliError> {
    run_propose(env, globals, request).map_err(escaped_error)
}

fn run_propose(
    env: &Env,
    globals: &Globals,
    request: &ProposeRequest,
) -> Result<ProposalOutcome, CliError> {
    let now = checked_now(&request.now)?;
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

    // 1. The target as written, then resolved.
    let written = request.target.trim();
    let scheme = &context.project.config.scheme;
    let reference = if written.ends_with(DOCUMENT_EXTENSION) {
        None
    } else {
        let Some(found) = grammar::parse_reference(written, 0, scheme) else {
            return refuse(no_reference(written, scheme), messages);
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
        if let Some(reason) = reason {
            return refuse(reason, messages);
        }
        Some(found.reference)
    };
    let input = indexed(env, &context.project, &mut messages, false)?;
    let resolver = Resolver::new(&input, scheme, &context.project.config.paths);
    let Some(reference) = reference else {
        let known = input
            .files
            .iter()
            .find(|file| file.path == written)
            .and_then(|file| file.parsed.as_ref())
            .and_then(ParsedFile::document)
            .and_then(|document| document.id.clone());
        let reason = match known {
            Some(id) => format!(
                "`{written}` is a path: a proposal names its node by ID, `{}`",
                canonical_id(&resolver, scheme, written, &id, None)
            ),
            None => format!(
                "`{written}` is a path: a proposal names its node by ID, and no indexed \
                 document with an ID lies there"
            ),
        };
        return refuse(reason, messages);
    };
    let path = match one_holder(&resolver, &reference, written) {
        Ok(file) => resolver.paths()[file].to_owned(),
        Err(HolderError::Dangling(reason)) => {
            return refuse(format!("`{written}` {reason}"), messages);
        }
        Err(HolderError::Several(paths)) => {
            return refuse(
                format!(
                    "`{written}` is held by {} files: {}; a proposal updates one node",
                    paths.len(),
                    paths.join(", ")
                ),
                messages,
            );
        }
        Err(HolderError::Project) => return Err(project_qualified(written)),
    };
    let tree = WorkingTree::new(&context.project.root, &context.project.config.paths)
        .map_err(store_error)?;
    let bytes = match tree.read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return refuse(format!("cannot read `{path}`: {error}"), messages),
    };
    if std::str::from_utf8(&bytes).is_err() {
        return refuse(
            format!("`{path}` is not UTF-8: its nodes cannot be updated"),
            messages,
        );
    }
    let parsed = match panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(&path, &bytes, scheme)
    })) {
        Ok(parsed) => parsed,
        Err(_) => return refuse(format!("the spec parser failed on `{path}`"), messages),
    };
    let ord = match locate(&parsed, &reference.id) {
        Ok(ord) => ord,
        Err(LocateError::Repeated(count)) => {
            return refuse(
                format!(
                    "`{written}` is declared {count} times in `{path}`; a proposal updates one \
                     node"
                ),
                messages,
            );
        }
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
            let reason = match document.and_then(|document| document.id.as_deref()) {
                Some(id) if aliased => format!(
                    "`{written}` is an alias of `{}`: a proposal names the canonical ID",
                    canonical_id(&resolver, scheme, &path, id, None)
                ),
                _ => format!("`{written}` is not in `{path}` as read now; run it again"),
            };
            return refuse(reason, messages);
        }
    };
    let target_id = canonical_id(
        &resolver,
        scheme,
        &path,
        &reference.id,
        reference.scope.as_deref(),
    );
    if let Some(refusal) = update_refusal(&parsed, ord, scheme) {
        return refuse(format!("`{target_id}`: {refusal}"), messages);
    }

    // 2. The base.
    let node = &parsed.nodes[ord];
    let base_hash = span_hash(&bytes, node);
    let given = request.base.trim();
    if given != base_hash {
        return refuse(
            format!(
                "`--base {given}` is stale: the span of `{target_id}` hashes to {base_hash} now; \
                 read it again (`spec show {target_id}`) and propose against that"
            ),
            messages,
        );
    }
    let Ok(base_text) = String::from_utf8(span_bytes(&bytes, node).to_vec()) else {
        return refuse(format!("the span of `{target_id}` is not UTF-8"), messages);
    };

    // 3. The structure.
    let update = match update_file(&path, &bytes, &parsed, ord, &text, scheme) {
        Ok(update) => update,
        Err(error) => return refuse(format!("`{target_id}`: {error}"), messages),
    };
    if update.bytes == bytes {
        return refuse(
            format!(
                "no change: `{path}` with the new text of `{target_id}` is byte for byte the \
                 file as read; nothing to propose"
            ),
            messages,
        );
    }

    // 4. Validation: never a refusal.
    let root = &context.project.root;
    let config = NamedBytes::read(CONFIG_FILE, &root.join(CONFIG_FILE));
    let baseline = default_baseline(root);
    let diagnostics = match load_check(&config, baseline.as_ref()) {
        Ok(setup) => introduced_findings(&tree, &setup, &now[..10], &path, update.bytes.clone()),
        Err(report) => {
            messages.push(Message::Warning(format!(
                "the edit was not validated (no introduced findings stored): {}",
                report.lines(false).join("; ")
            )));
            Vec::new()
        }
    };

    // 5. Stored, bound to the place found first.
    let new_text = update.text;
    let created = context
        .queue
        .create(
            &NewProposal {
                kind: ProposalKind::Update,
                patch_hash: patch_hash(&target_id, &base_hash, &new_text),
                target_id,
                target_path: path,
                place,
                base_hash,
                base_text,
                new_text,
                rationale: request.rationale.clone(),
                author,
                diagnostics,
            },
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
