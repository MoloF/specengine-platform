//! The read-only apply steps 2–6 (task spec `proposal-apply`, "Apply"),
//! shared by `spec approve` (its checks before the prompt) and `spec
//! review` (`preview`), and the lookup of an interrupted apply's commit
//! ("Idempotence"):
//!
//! 2. place: the worktree exists, its top and common dir as recorded,
//!    `HEAD` on the recorded branch with a commit, no merge, rebase,
//!    cherry-pick, revert, bisect or sequencer state (exit 2);
//! 3. file: the recorded root's own `specengine.toml` (its slug the
//!    queue's; else exit 2), the target listed by its walk, read without a
//!    symlink on its path, UTF-8, not generated nor immutable, tracked and
//!    clean (`git status` of the path empty) (exit 1);
//! 4. resolve: the target held by one file, at its recorded path, in the
//!    recorded root's refreshed index, located in a fresh parse of the
//!    bytes just read, never by stored offsets (exit 1); a path target
//!    (task spec `queue-path-targets`): the document of that parse, no
//!    holder lookup;
//! 5. text: the span hashes to `base_hash` → the new text (`applies`);
//!    else `git merge-file -p -L current -L base -L proposed` over scratch
//!    files in the data directory: clean → the merge (`rebases`), conflict
//!    → exit 1 with its text; the file patched with it equal to the file
//!    as read → exit 1, already in place (nothing to write or commit): by
//!    the proposal's own commit on the branch ([`completing`]) →
//!    that commit, which `spec approve` completes; a `Proposal:` commit
//!    that does not complete it is named with why; a text to write → exit
//!    1 when its own commit is on the branch, or a `Proposal:` commit that
//!    applied it on top of its parent but does not complete it
//!    ([`TrailerCommit::carries`]): no new apply, never a merge on top of
//!    the proposal's own commit;
//! 6. structure: creation's check on this file (exit 1).
//!
//! A create's new sections (task spec `proposal-kinds`) take the same
//! steps: 4 also finds its new IDs free in the refreshed index, checked
//! once step 5's text is known not to be in place (a text in place is step
//! 5's refusal, its own commit a completion), 6 is the section rule on the
//! merged span, adding exactly its stored new IDs. A create's new file has
//! its own steps ([`crate::create`]).
//!
//! [`place_unchanged`] repeats step 2's branch, `HEAD` and operation
//! checks right before the write (step 8).
//!
//! The proposal's `Proposal:` commits on its branch ([`trailer_lookup`]) are
//! read in its [`History`]: the recorded worktree, or the current
//! repository (the recorded one) when the worktree is not there; an
//! orphan's in the current repository ([`trailer_lookup_here`]). One
//! completes the proposal with one parent, changing only the target's
//! path, its text there step 5's on its parent's blob (what the apply
//! wrote on top of it) or on its own blob (the new text; a split or
//! cherry-picked apply's merge), each blob read under the `specengine.toml`
//! of its commit's tree, the one read now when the tree has none. A base
//! commit not there (pruned after a rebase): the branch's whole history is
//! read; a branch not there is named with the way out. A question's or a
//! discrepancy's commit (canon `decision-record`, "Completion")
//! completes it with one parent, adding exactly its record's path, the blob
//! there its stored `record_text` byte for byte, never rendered again
//! ([`record_commit`]); a create's new file (task spec `proposal-kinds`)
//! alike, its blob the stored `new_text` ([`file_commit`]).
//!
//! Git runs `-C <worktree>` (a [`History::Current`] lookup: `-C` the
//! current project root), stdin null, without the caller's local `GIT_*`
//! variables and with optional locks off: nothing is written in the
//! worktree or its git dir.

use std::fmt;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use specengine_core::ProjectConfig;
use specengine_core::check::Resolver;
use specengine_core::create::add_sections;
use specengine_core::patch::{
    HolderError, is_section, locate, one_holder, span_bytes, splice, update_refusal, update_text,
};
use specengine_core::proposal::PROPOSAL_TRAILER;
use specengine_model::node::Node;
use specengine_model::{ParsedFile, grammar};
use specengine_store::{
    GitEnv, GitError, Merge, Place, Proposal, ProposalKind, Source as _, SpecIndex as _,
    WorkingTree, WorktreeGit, same_repository, span_hash, update_file,
};

use crate::create::{Corpus, canonical_in};
use crate::location::{OpenIndex, open_index};
use crate::project::{CONFIG_FILE, ProjectRoot, config_error};
use crate::proposals::{Preview, QueueContext, top_of, top_path};
use crate::propose::is_path_target;
use crate::refresh::refresh;
use crate::{Env, Exit, Message, one_line};

/// Apply steps 2–6 passed: what steps 7–10 act on.
pub(crate) struct Prepared {
    pub git: WorktreeGit,
    /// The branch's commit when checked.
    pub head: String,
    /// The target's path from the worktree's top.
    pub top_path: String,
    /// The recorded root's project and its index.
    pub recorded: ProjectRoot,
    pub index: OpenIndex,
    pub tree: WorkingTree,
    /// The file as read at step 3.
    pub bytes: Vec<u8>,
    /// The file to write (step 6).
    pub patched: Vec<u8>,
    /// [`Preview::Applies`] or [`Preview::Rebases`].
    pub preview: Preview,
}

/// A refusal at an apply step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepFailure {
    pub step: u8,
    /// [`Exit::NotFound`] (refused by the proposal or the target) or
    /// [`Exit::CannotRun`].
    pub exit: Exit,
    /// One line.
    pub reason: String,
    /// `git merge-file`'s text of a conflict (step 5).
    pub conflict: Option<String>,
    /// Step 5's text already in place by the proposal's own commit on the
    /// branch ([`completing`]): that commit, which completes it.
    pub completing: Option<String>,
}

impl StepFailure {
    pub(crate) fn refused(step: u8, reason: impl Into<String>) -> Self {
        Self {
            step,
            exit: Exit::NotFound,
            reason: one_line(&reason.into()),
            conflict: None,
            completing: None,
        }
    }

    pub(crate) fn cannot(step: u8, reason: impl Into<String>) -> Self {
        Self {
            step,
            exit: Exit::CannotRun,
            reason: one_line(&reason.into()),
            conflict: None,
            completing: None,
        }
    }
}

/// Apply steps 2–6 for `proposal`, writing nothing but the data directory
/// (the recorded root's index, git's scratch files): `spec approve`'s
/// checks and `spec review`'s preview.
pub(crate) fn prepare(
    env: &Env,
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> Result<Prepared, StepFailure> {
    let place = &proposal.place;
    let path = proposal.target_path.as_str();
    let Placed { git, top, head } = place_step(git_env, proposal)?;

    // Step 3: the file.
    let root = if place.root_rel.is_empty() {
        top.clone()
    } else {
        top.join(&place.root_rel)
    };
    let recorded =
        recorded_project(&root, &context.slug).map_err(|reason| StepFailure::cannot(3, reason))?;
    let tree = WorkingTree::new(&recorded.root, &recorded.config.paths)
        .map_err(|error| StepFailure::cannot(3, error.to_string()))?;
    if !tree.probe(path) {
        return Err(StepFailure::refused(
            3,
            format!(
                "`{path}` is no file the walk of {} lists: missing, excluded, not a regular \
                 file, or a symlink on its path",
                recorded.root.display()
            ),
        ));
    }
    let bytes = tree
        .read(path)
        .map_err(|error| StepFailure::refused(3, format!("cannot read `{path}`: {error}")))?;
    if std::str::from_utf8(&bytes).is_err() {
        return Err(StepFailure::refused(3, format!("`{path}` is not UTF-8")));
    }
    let scheme = &recorded.config.scheme;
    let parsed: ParsedFile = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, &bytes, scheme)
    }))
    .map_err(|_| StepFailure::refused(3, format!("the spec parser failed on `{path}`")))?;
    let top_path = top_path(proposal);
    let tracked = git
        .is_tracked(&top_path)
        .map_err(|error| StepFailure::cannot(3, error.to_string()))?;
    if !tracked {
        return Err(StepFailure::refused(
            3,
            format!(
                "`{path}` is not tracked by git in {}: commit it first",
                place.worktree
            ),
        ));
    }
    let dirty = git
        .is_dirty(&top_path)
        .map_err(|error| StepFailure::cannot(3, error.to_string()))?;
    if dirty {
        return Err(StepFailure::refused(
            3,
            format!(
                "`{path}` has uncommitted changes in {} (staged or not): commit, stash or \
                 restore them first",
                place.worktree
            ),
        ));
    }

    // Step 4: resolve.
    let mut index =
        open_index(env, &recorded).map_err(|error| StepFailure::cannot(4, error.message))?;
    let (_, warnings) = refresh(&mut index.index, &recorded, false)
        .map_err(|error| StepFailure::cannot(4, error.message))?;
    messages.extend(warnings);
    let input = index
        .index
        .indexed_input()
        .map_err(|error| StepFailure::cannot(4, error.to_string()))?;
    let resolver = Resolver::new(&input, scheme, &recorded.config.paths);
    let target = proposal.target_id.as_str();
    let ord = if is_path_target(target) {
        document_of(&parsed, target, path).map_err(|reason| StepFailure::refused(4, reason))?
    } else {
        held_by_id(&resolver, &recorded, &parsed, path, target)?
    };
    if let Some(refusal) = update_refusal(&parsed, ord, scheme) {
        return Err(StepFailure::refused(3, format!("`{target}`: {refusal}")));
    }
    // A create's new IDs are checked free at step 4 once step 5's text is
    // known not to be in place already (below): a text in place defines
    // them in this very file, which step 5 explains.
    let create = proposal.kind == ProposalKind::Create;

    // Step 5: the text.
    let node = &parsed.nodes[ord];
    let step = step_text(&git, &context.data_dir, &bytes, node, proposal)
        .map_err(|error| StepFailure::cannot(5, error.to_string()))?;
    let (text, preview) = match step {
        StepText::Text(text, preview) => (text, preview),
        StepText::NotUtf8 => {
            return Err(StepFailure::refused(
                5,
                format!("the merge of `{target}` is not UTF-8"),
            ));
        }
        StepText::Conflict { text, conflicts } => {
            return Err(StepFailure {
                step: 5,
                exit: Exit::NotFound,
                reason: format!(
                    "`{target}` changed since the proposal and the edits overlap: {conflicts} \
                     conflict(s) in the merge of current, base and proposed"
                ),
                conflict: Some(String::from_utf8_lossy(&text).into_owned()),
                completing: None,
            });
        }
    };

    // Step 6: the structure: an update's rule; a create's new sections,
    // exactly its stored new IDs (a text already in place: below). A
    // create's text to write: first step 4's new IDs, free in the index
    // refreshed now.
    let patched = if create {
        let spliced = splice(&bytes, node.span, update_text(&text, is_section(node)));
        if spliced == bytes {
            spliced
        } else {
            let corpus = Corpus::of(&input, scheme, &recorded.config.paths);
            if let Some((id, holder)) = proposal
                .new_ids
                .iter()
                .find_map(|id| Some((id, corpus.holder(id)?)))
            {
                return Err(StepFailure::refused(
                    4,
                    format!("{}; nothing changed", holder.taken(id)),
                ));
            }
            let sections = panic::catch_unwind(AssertUnwindSafe(|| {
                add_sections(path, &bytes, &parsed, ord, &text, scheme)
            }))
            .map_err(|_| StepFailure::refused(6, format!("the spec parser failed on `{path}`")))?
            .map_err(|error| StepFailure::refused(6, format!("`{target}`: {error}")))?;
            let slug = resolver.feature_slug(path);
            let added: Vec<String> = sections
                .added
                .iter()
                .filter_map(|&at| sections.parsed.nodes.get(at)?.id.as_deref())
                .map(|id| canonical_in(scheme, slug, id))
                .collect();
            if added != proposal.new_ids {
                return Err(StepFailure::refused(
                    6,
                    format!(
                        "`{target}`: the merged text adds {}, the proposal {}",
                        listed(&added),
                        listed(&proposal.new_ids)
                    ),
                ));
            }
            sections.bytes
        }
    } else {
        update_file(path, &bytes, &parsed, ord, &text, scheme)
            .map_err(|error| StepFailure::refused(6, format!("`{target}`: {error}")))?
            .bytes
    };
    // The proposal's `Proposal:` commits on the branch. Step 5's text
    // already there (the patched file is the file as read; equal bytes
    // pass step 6 always): completed by its own commit, or one that does
    // not complete it named. A text to write: refused when one of them
    // applied the proposal already; a lookup git cannot make is
    // `approve`'s and `review`'s note, not a refusal.
    let config_now = || Ok(recorded.config.clone());
    let then = approve_or_reject(proposal);
    let found = trailer_commits(&git, &context.data_dir, proposal, &config_now, &then);
    if patched == bytes {
        return Err(already_in_place(proposal, found.as_deref()));
    }
    if let Ok(found) = &found
        && let Some(failure) = applied_before(proposal, found)
    {
        return Err(failure);
    }
    Ok(Prepared {
        git,
        head,
        top_path,
        recorded,
        index,
        tree,
        bytes,
        patched,
        preview,
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

/// Step 2 passed: git in the recorded worktree, its top, and the branch's
/// commit.
pub(crate) struct Placed {
    pub git: WorktreeGit,
    /// Canonical.
    pub top: PathBuf,
    pub head: String,
}

/// Apply step 2: the recorded worktree exists, its top and common dir as
/// recorded, `HEAD` on the recorded branch with a commit, no operation in
/// progress; else exit 2.
pub(crate) fn place_step(git_env: &GitEnv, proposal: &Proposal) -> Result<Placed, StepFailure> {
    let place = &proposal.place;
    let worktree = Path::new(&place.worktree);
    let at = |error: &dyn std::fmt::Display| {
        StepFailure::cannot(2, format!("git in {}: {error}", place.worktree))
    };
    if !worktree.is_dir() {
        return Err(StepFailure::cannot(
            2,
            format!(
                "the proposal's worktree {} no longer exists",
                place.worktree
            ),
        ));
    }
    let git = WorktreeGit::new(worktree, git_env).map_err(|error| at(&error))?;
    let top = git.top().map_err(|error| at(&error))?;
    if !same_repository(&place.worktree, &top) {
        return Err(StepFailure::cannot(
            2,
            format!(
                "{} is no longer the top of a git worktree (git names {})",
                place.worktree,
                top.display()
            ),
        ));
    }
    let common = git.common_dir().map_err(|error| at(&error))?;
    if !same_repository(&place.git_common_dir, &common) {
        return Err(StepFailure::cannot(
            2,
            format!(
                "the worktree {} belongs to the repository {} now, not {}",
                place.worktree,
                common.display(),
                place.git_common_dir
            ),
        ));
    }
    match git.branch().map_err(|error| at(&error))? {
        Some(branch) if branch == place.branch => {}
        Some(branch) => {
            return Err(StepFailure::cannot(
                2,
                format!(
                    "the worktree {} is on `{branch}`; the proposal applies on `{}`: check it \
                     out there",
                    place.worktree, place.branch
                ),
            ));
        }
        None => {
            return Err(StepFailure::cannot(
                2,
                format!(
                    "HEAD is detached in {}; the proposal applies on `{}`: check it out there",
                    place.worktree, place.branch
                ),
            ));
        }
    }
    if let Some(operation) = git.operation_in_progress().map_err(|error| at(&error))? {
        return Err(StepFailure::cannot(
            2,
            format!(
                "a {operation} is in progress in {}: finish or abort it first",
                place.worktree
            ),
        ));
    }
    let head = git
        .head()
        .map_err(|error| at(&error))?
        .ok_or_else(|| StepFailure::cannot(2, format!("`{}` has no commit", place.branch)))?;
    Ok(Placed { git, top, head })
}

/// Step 4 for a target by ID: held by one file in the refreshed index,
/// `path`, and declared once in `parsed` (its fresh parse): its position.
fn held_by_id(
    resolver: &Resolver<'_>,
    recorded: &ProjectRoot,
    parsed: &ParsedFile,
    path: &str,
    target: &str,
) -> Result<usize, StepFailure> {
    let scheme = &recorded.config.scheme;
    let Some(found) = grammar::parse_reference(target, 0, scheme) else {
        return Err(StepFailure::refused(
            4,
            format!(
                "`{target}` is no reference under the `[ids]` of {}",
                recorded.root.display()
            ),
        ));
    };
    let reference = found.reference;
    match one_holder(resolver, &reference, target) {
        Ok(file) if resolver.paths()[file] == path => {}
        Ok(file) => {
            return Err(StepFailure::refused(
                4,
                format!(
                    "`{target}` is held by `{}` now, not by `{path}`",
                    resolver.paths()[file]
                ),
            ));
        }
        Err(HolderError::Dangling(reason)) => {
            return Err(StepFailure::refused(4, format!("`{target}` {reason}")));
        }
        Err(HolderError::Several(paths)) => {
            return Err(StepFailure::refused(
                4,
                format!(
                    "`{target}` is held by {} files now: {}",
                    paths.len(),
                    paths.join(", ")
                ),
            ));
        }
        Err(HolderError::Project) => {
            return Err(StepFailure::refused(
                4,
                format!("`{target}` names another project"),
            ));
        }
    }
    locate(parsed, &reference.id).map_err(|_| {
        StepFailure::refused(
            4,
            format!(
                "`{path}` as read does not hold `{}` exactly once",
                reference.id
            ),
        )
    })
}

/// Step 4 for a path target: the document of `parsed` (the fresh parse of
/// `path`, the stored target itself), no holder lookup.
fn document_of(parsed: &ParsedFile, target: &str, path: &str) -> Result<usize, String> {
    if target != path {
        return Err(format!(
            "`{target}` names a document by its path, not the recorded `{path}`"
        ));
    }
    match parsed.document() {
        Some(_) => Ok(0),
        None => Err(format!("`{path}` as read has no node")),
    }
}

/// Right before step 8: the place still as step 2 found it, `HEAD` on the
/// recorded branch at the commit checked (`head`, [`Prepared::head`]), no
/// operation in progress; else a refusal at step 8 (exit 1), nothing
/// written. The owner may switch branches while asked.
pub(crate) fn place_unchanged(
    git: &WorktreeGit,
    head: &str,
    place: &Place,
) -> Result<(), StepFailure> {
    let refused = |what: String| {
        StepFailure::refused(
            8,
            format!("{what} since the checks before the prompt; nothing written"),
        )
    };
    let at = |error: &dyn std::fmt::Display| {
        StepFailure::refused(
            8,
            format!("git in {}: {error}; nothing written", place.worktree),
        )
    };
    match git.branch() {
        Ok(Some(branch)) if branch == place.branch => {}
        Ok(Some(branch)) => {
            return Err(refused(format!(
                "the worktree {} moved to `{branch}` from `{}`",
                place.worktree, place.branch
            )));
        }
        Ok(None) => {
            return Err(refused(format!(
                "HEAD was detached in {} (it was on `{}`)",
                place.worktree, place.branch
            )));
        }
        Err(error) => return Err(at(&error)),
    }
    match git.head() {
        Ok(Some(now)) if now == head => {}
        Ok(Some(now)) => {
            return Err(refused(format!(
                "`{}` moved from {head} to {now}",
                place.branch
            )));
        }
        Ok(None) => return Err(refused(format!("`{}` lost its commit", place.branch))),
        Err(error) => return Err(at(&error)),
    }
    match git.operation_in_progress() {
        Ok(None) => Ok(()),
        Ok(Some(operation)) => Err(refused(format!(
            "a {operation} started in {}",
            place.worktree
        ))),
        Err(error) => Err(at(&error)),
    }
}

/// The recorded root as a project: its own `specengine.toml`, whose slug
/// must be the queue's.
pub(crate) fn recorded_project(root: &Path, slug: &str) -> Result<ProjectRoot, String> {
    let root = fs::canonicalize(root)
        .map_err(|error| format!("the recorded root {}: {error}", root.display()))?;
    let file = root.join(CONFIG_FILE);
    let label = file.display().to_string();
    let bytes = fs::read(&file).map_err(|error| format!("cannot read {label}: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{label}: the file is not UTF-8"))?;
    let config =
        ProjectConfig::from_toml(&text).map_err(|error| config_error(&label, &error).message)?;
    let project = ProjectRoot {
        root,
        config_label: label,
        config,
    };
    let found = project.slug().map_err(|error| error.message)?;
    if found != slug {
        return Err(format!(
            "{}: the slug is `{found}`, the proposal's project is `{slug}`",
            project.config_label
        ));
    }
    Ok(project)
}

/// Step 5's text for a node's span.
enum StepText {
    /// The new text (`applies`), or the clean merge (`rebases`).
    Text(String, Preview),
    /// The clean merge is not UTF-8.
    NotUtf8,
    /// `git merge-file`'s text and its count of conflicts.
    Conflict { text: Vec<u8>, conflicts: i32 },
}

/// Step 5 on `bytes`, a file holding the target at `node`: the span hashes
/// to `base_hash` → the new text; else `git merge-file -p` of current, base
/// and proposed (scratch files in `scratch`). A section's three sides are
/// each given a final line ending, dropped from a clean merge: its span
/// never ends in one (the parser's trimming), so text added after its last
/// line would otherwise change that line and conflict with an edit of the
/// line before it (task spec `proposal-kinds` AC-07).
fn step_text(
    git: &WorktreeGit,
    scratch: &Path,
    bytes: &[u8],
    node: &Node,
    proposal: &Proposal,
) -> Result<StepText, GitError> {
    if span_hash(bytes, node) == proposal.base_hash {
        return Ok(StepText::Text(proposal.new_text.clone(), Preview::Applies));
    }
    let section = is_section(node);
    let side = |text: &[u8]| {
        let mut side = text.to_vec();
        if section {
            side.push(b'\n');
        }
        side
    };
    let merged = git.merge_file(
        scratch,
        &side(span_bytes(bytes, node)),
        &side(proposal.base_text.as_bytes()),
        &side(proposal.new_text.as_bytes()),
    )?;
    let merged = match merged {
        Merge::Clean(mut merged) => {
            if section && merged.last() == Some(&b'\n') {
                merged.pop();
            }
            Merge::Clean(merged)
        }
        conflict => conflict,
    };
    Ok(match merged {
        Merge::Clean(merged) => match String::from_utf8(merged) {
            Ok(merged) => StepText::Text(merged, Preview::Rebases),
            Err(_) => StepText::NotUtf8,
        },
        Merge::Conflict { text, conflicts } => StepText::Conflict { text, conflicts },
    })
}

/// Step 5's refusal of a text already in place: completed by the
/// proposal's own commit on its branch ([`StepFailure::completing`]); else
/// naming the newest `Proposal:` commit that does not complete it, with
/// why, or saying that git cannot tell.
fn already_in_place(
    proposal: &Proposal,
    found: Result<&[TrailerCommit], &LookupError>,
) -> StepFailure {
    let branch = &proposal.place.branch;
    let in_place = format!(
        "`{}`: the proposal's text is already in place in `{}`",
        proposal.target_id, proposal.target_path
    );
    let found = match found {
        Ok(found) => found,
        Err(error) => {
            return StepFailure::refused(
                5,
                format!(
                    "{in_place}; cannot tell whether a commit on `{branch}` carries \
                     `{PROPOSAL_TRAILER}: {}`: {error}; nothing to write or commit",
                    proposal.id
                ),
            );
        }
    };
    if let Some(own) = completing(found) {
        let mut failure = StepFailure::refused(
            5,
            format!(
                "{in_place} by its commit {} on `{branch}`: `spec approve` completes it",
                own.commit
            ),
        );
        failure.completing = Some(own.commit.clone());
        return failure;
    }
    let reason = match found.first() {
        Some(TrailerCommit {
            commit,
            not_completing: Some(why),
            ..
        }) => format!(
            "{in_place}; the commit {commit} on `{branch}` carries `{PROPOSAL_TRAILER}: {}` but \
             {why}; {}; nothing to write or commit",
            proposal.id,
            completes_when(proposal)
        ),
        _ => format!("{in_place}; nothing to write or commit"),
    };
    StepFailure::refused(5, reason)
}

/// Step 5's refusal of a text to write when a `Proposal:` commit on the
/// branch applied the proposal already: its own commit, which `spec
/// approve` completes ([`StepFailure::completing`]); else the newest whose
/// text is step 5's on its first parent ([`TrailerCommit::carries`]) but
/// that does not complete it (two paths, a merge), named with what
/// completes it: a new apply would merge the proposal again on top of it.
fn applied_before(proposal: &Proposal, found: &[TrailerCommit]) -> Option<StepFailure> {
    let branch = &proposal.place.branch;
    let id = &proposal.id;
    if let Some(own) = completing(found) {
        let mut failure = StepFailure::refused(
            5,
            format!(
                "`{id}` has its commit {} on `{branch}`: `spec approve` completes it; no new \
                 apply",
                own.commit
            ),
        );
        failure.completing = Some(own.commit.clone());
        return Some(failure);
    }
    let applied = found.iter().find(|found| found.carries)?;
    let why = applied.not_completing.as_deref().unwrap_or_default();
    Some(StepFailure::refused(
        5,
        format!(
            "the commit {} on `{branch}` carries `{PROPOSAL_TRAILER}: {id}` and applied the \
             proposal on top of its parent, but {why}: a new apply would apply it again; {}; no \
             new apply",
            applied.commit,
            completes_when(proposal)
        ),
    ))
}

/// What completes a proposal by its commit, for a refusal naming one that
/// does not (steps 5 and 10, `spec reject`).
pub(crate) fn completes_when(proposal: &Proposal) -> String {
    if proposal.new_file() {
        return format!(
            "a commit on `{}` with the trailer `{PROPOSAL_TRAILER}: {id}`, one parent, adding \
             only `{}` with the proposal's text completes it (`spec approve {id}`)",
            proposal.place.branch,
            top_path(proposal),
            id = proposal.id
        );
    }
    if let Some(record) = &proposal.record {
        return format!(
            "a commit on `{}` with the trailer `{PROPOSAL_TRAILER}: {id}`, one parent, adding \
             only `{}` with the record's text completes it (`spec approve {id}`)",
            proposal.place.branch,
            top_of(proposal, &record.path),
            id = proposal.id
        );
    }
    format!(
        "a commit on `{}` with the trailer `{PROPOSAL_TRAILER}: {id}`, one parent, changing only \
         `{}` to the proposal's text completes it (`spec approve {id}`)",
        proposal.place.branch,
        top_path(proposal),
        id = proposal.id
    )
}

/// A commit in `base_commit..<branch>` whose `Proposal:` trailer is the
/// proposal's ID.
pub(crate) struct TrailerCommit {
    pub commit: String,
    /// Why it does not complete the proposal (its parents, its paths, its
    /// text); `None`: it does.
    pub not_completing: Option<String>,
    /// Its text at the path is step 5's on its first parent's: what an
    /// apply on top of that parent writes ([`written_on`]).
    pub carries: bool,
}

/// The first of `found` that completes the proposal: its own commit (an
/// interrupted apply's, a cherry-picked or split one).
pub(crate) fn completing(found: &[TrailerCommit]) -> Option<&TrailerCommit> {
    found.iter().find(|found| found.not_completing.is_none())
}

/// What a lookup's repository does not hold. A base commit missing on its
/// own is none: the branch's whole history is read instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Missing {
    /// The branch; the base commit is there.
    Branch,
    /// The branch and the base commit.
    Both,
}

/// Why a proposal's trailer commits cannot be listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LookupError {
    /// What is not in the repository read; `None`: git cannot tell.
    pub missing: Option<Missing>,
    /// One line; naming the way out when something is missing.
    pub reason: String,
}

impl LookupError {
    fn failed(reason: impl Into<String>) -> Self {
        Self {
            missing: None,
            reason: one_line(&reason.into()),
        }
    }
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

/// Where a proposal's branch is read outside the apply steps (the
/// completion's lookup and identity, `review`'s note, `reject`'s guard and
/// identity).
pub(crate) enum History<'c> {
    /// The recorded worktree, a directory of the recorded repository.
    Recorded(WorktreeGit),
    /// The current repository (`QueueContext::git`), which is the recorded
    /// one, when the recorded worktree is not there (removed, moved): its
    /// branches are the same refs.
    Current(&'c WorktreeGit),
}

impl History<'_> {
    pub(crate) fn git(&self) -> &WorktreeGit {
        match self {
            Self::Recorded(git) => git,
            Self::Current(git) => git,
        }
    }

    /// The config a commit is read under when its tree has no
    /// `<root_rel>/specengine.toml` (not committed): the recorded root's
    /// own now; the current project's for a [`History::Current`] lookup.
    fn config_now(
        &self,
        context: &QueueContext,
        proposal: &Proposal,
    ) -> Result<ProjectConfig, String> {
        match self {
            Self::Recorded(_) => {
                recorded_project(&recorded_root(proposal), &context.slug).map(|found| found.config)
            }
            Self::Current(_) => Ok(context.project.config.clone()),
        }
    }
}

/// The recorded root: the recorded worktree joined with `root_rel`.
pub(crate) fn recorded_root(proposal: &Proposal) -> PathBuf {
    let worktree = Path::new(&proposal.place.worktree);
    if proposal.place.root_rel.is_empty() {
        worktree.to_path_buf()
    } else {
        worktree.join(&proposal.place.root_rel)
    }
}

/// The proposal's [`History`]: the recorded worktree when it is a
/// directory whose git common dir is the recorded one, else the current
/// repository when it is the recorded one (`find` gives `approve` and
/// `review` only those); else why not.
pub(crate) fn history<'c>(
    git_env: &GitEnv,
    context: &'c QueueContext,
    proposal: &Proposal,
) -> Result<History<'c>, String> {
    let place = &proposal.place;
    let worktree = Path::new(&place.worktree);
    // The repository the recorded path belongs to now, when another one.
    let mut other = None;
    if worktree.is_dir()
        && let Ok(git) = WorktreeGit::new(worktree, git_env)
        && let Ok(common) = git.common_dir()
    {
        if same_repository(&place.git_common_dir, &common) {
            return Ok(History::Recorded(git));
        }
        other = Some(common);
    }
    if same_repository(&place.git_common_dir, &context.common_dir) {
        return Ok(History::Current(&context.git));
    }
    Err(match other {
        Some(common) => format!(
            "the worktree {} belongs to another repository ({}), and its repository {} is not \
             the current one",
            place.worktree,
            common.display(),
            place.git_common_dir
        ),
        None if worktree.is_dir() => format!(
            "{} is no longer a git worktree, and its repository {} is not the current one",
            place.worktree, place.git_common_dir
        ),
        None => format!(
            "the worktree {} is not there and its repository {} is not the current one",
            place.worktree, place.git_common_dir
        ),
    })
}

/// The proposal's trailer commits on its branch ([`trailer_commits`]),
/// read in its [`History`]; a missing branch's way out ends "then `spec
/// approve PR` or `spec reject PR`".
pub(crate) fn trailer_lookup(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
) -> Result<Vec<TrailerCommit>, LookupError> {
    let history = history(git_env, context, proposal).map_err(LookupError::failed)?;
    let config_now = || history.config_now(context, proposal);
    let then = approve_or_reject(proposal);
    trailer_commits(
        history.git(),
        &context.data_dir,
        proposal,
        &config_now,
        &then,
    )
}

/// The proposal's trailer commits on the branch of its name in the current
/// repository, whatever repository it was recorded in: an orphan's, whose
/// recorded repository is gone or moved here (`spec reject`, which a
/// missing branch's way out names).
pub(crate) fn trailer_lookup_here(
    context: &QueueContext,
    proposal: &Proposal,
) -> Result<Vec<TrailerCommit>, LookupError> {
    let config_now = || Ok(context.project.config.clone());
    let then = format!("`spec reject {}`", proposal.id);
    trailer_commits(
        &context.git,
        &context.data_dir,
        proposal,
        &config_now,
        &then,
    )
}

/// What follows a missing branch recreated: `spec approve` (it completes a
/// commit found) or `spec reject` (none found).
fn approve_or_reject(proposal: &Proposal) -> String {
    format!(
        "`spec approve {id}` or `spec reject {id}`",
        id = proposal.id
    )
}

/// The commits in `base_commit..refs/heads/<branch>` whose `Proposal:`
/// trailer is the proposal's ID, newest first, each with why it does not
/// complete the proposal ([`trailer_commit`]); the base commit not in the
/// repository (pruned after a rebase): the branch's whole history. `Err`:
/// the branch is not there ([`LookupError::missing`], naming the way out:
/// recreate it, then `then`), or git cannot tell. `config_now`: the config
/// of a commit whose tree has none ([`config_at`]).
fn trailer_commits(
    git: &WorktreeGit,
    scratch: &Path,
    proposal: &Proposal,
    config_now: &dyn Fn() -> Result<ProjectConfig, String>,
    then: &str,
) -> Result<Vec<TrailerCommit>, LookupError> {
    let place = &proposal.place;
    let (branch, id, base) = (&place.branch, &proposal.id, &place.base_commit);
    let failed = |error: GitError| LookupError::failed(error.to_string());
    let branch_there = git
        .commit_of(&format!("refs/heads/{branch}"))
        .map_err(failed)?
        .is_some();
    let base_there = git.commit_of(base).map_err(failed)?.is_some();
    let found = match (branch_there, base_there) {
        (true, true) => git.commits_with_trailer(base, branch, PROPOSAL_TRAILER, id),
        (true, false) => git.branch_commits_with_trailer(branch, PROPOSAL_TRAILER, id),
        (false, true) => {
            return Err(LookupError {
                missing: Some(Missing::Branch),
                reason: format!(
                    "the branch `{branch}` no longer exists; recreate it at its last commit \
                     (`git branch {branch} <commit>`), then {then}"
                ),
            });
        }
        (false, false) => {
            return Err(LookupError {
                missing: Some(Missing::Both),
                reason: format!(
                    "the branch `{branch}` no longer exists, and the proposal's base commit \
                     {base} is not in the repository; recreate the branch at its last commit \
                     (`git branch {branch} <commit>`), then {then}"
                ),
            });
        }
    }
    .map_err(failed)?;
    Ok(found
        .into_iter()
        .map(|commit| {
            if proposal.kind.decides() {
                record_commit(git, proposal, commit)
            } else if proposal.new_file() {
                file_commit(git, proposal, commit)
            } else {
                trailer_commit(git, scratch, proposal, config_now, commit)
            }
        })
        .collect())
}

/// One trailer commit of a question or a discrepancy judged: it completes
/// it with one parent, adding exactly the record's path, its blob there the
/// stored `record_text` byte for byte (never rendered again). A proposal
/// with no record issued has nothing to compare: none completes it.
fn record_commit(git: &WorktreeGit, proposal: &Proposal, commit: String) -> TrailerCommit {
    let not_completing = match &proposal.record {
        None => Some("the proposal holds no issued record to compare it with".to_owned()),
        Some(record) => {
            let path = top_of(proposal, &record.path);
            match git.parents(&commit) {
                Ok(parents) => match parents.as_slice() {
                    [parent] => {
                        adds_file(git, parent, &commit, &path, &record.text, "the record").err()
                    }
                    parents => Some(format!("has {} parents", parents.len())),
                },
                Err(error) => Some(one_line(&format!("cannot be read: {error}"))),
            }
        }
    };
    TrailerCommit {
        commit,
        not_completing,
        carries: false,
    }
}

/// One trailer commit of a create's new file judged: it completes it with
/// one parent, adding exactly the target's path, its blob there the stored
/// `new_text` byte for byte.
fn file_commit(git: &WorktreeGit, proposal: &Proposal, commit: String) -> TrailerCommit {
    let path = top_path(proposal);
    let not_completing = match git.parents(&commit) {
        Ok(parents) => match parents.as_slice() {
            [parent] => adds_file(
                git,
                parent,
                &commit,
                &path,
                &proposal.new_text,
                "the proposal's text",
            )
            .err(),
            parents => Some(format!("has {} parents", parents.len())),
        },
        Err(error) => Some(one_line(&format!("cannot be read: {error}"))),
    };
    TrailerCommit {
        commit,
        not_completing,
        carries: false,
    }
}

/// `commit` against `parent` adds exactly `path` (top-relative) and
/// nothing else, its blob there `text` byte for byte; else why not:
/// `changes <status> <path>, …`, `does not carry <what>` (`the record`,
/// `the proposal's text`), `cannot be read: <git's error>`.
pub(crate) fn adds_file(
    git: &WorktreeGit,
    parent: &str,
    commit: &str,
    path: &str,
    text: &str,
    what: &str,
) -> Result<(), String> {
    let unreadable = |error: &dyn std::fmt::Display| one_line(&format!("cannot be read: {error}"));
    match git.name_status(parent, commit) {
        Ok(changed) if changed.len() == 1 && changed[0].adds(path) => {}
        Ok(changed) => {
            let listed: Vec<String> = changed.iter().map(ToString::to_string).collect();
            return Err(format!("changes {}", listed.join(", ")));
        }
        Err(error) => return Err(unreadable(&error)),
    }
    match git.blob_at(commit, path) {
        Ok(bytes) if bytes == text.as_bytes() => Ok(()),
        Ok(_) => Err(format!("does not carry {what}")),
        Err(error) => Err(unreadable(&error)),
    }
}

/// One trailer commit judged: it completes the proposal with one parent,
/// changing only the target's path, its text there step 5's on its parent
/// ([`written_on`]: what the apply wrote on top of it) or on itself
/// ([`settled`]: the new text, a split or cherry-picked apply's merge).
fn trailer_commit(
    git: &WorktreeGit,
    scratch: &Path,
    proposal: &Proposal,
    config_now: &dyn Fn() -> Result<ProjectConfig, String>,
    commit: String,
) -> TrailerCommit {
    let path = top_path(proposal);
    let held = held_at(git, &commit, proposal, config_now, "its");
    let parents = git.parents(&commit);
    let carries = match (&held, &parents) {
        (Ok(held), Ok(parents)) => parents
            .first()
            .is_some_and(|parent| written_on(git, scratch, held, parent, proposal, config_now)),
        _ => false,
    };
    let not_completing = match &parents {
        Ok(parents) => match parents.as_slice() {
            [parent] => match git.changed_paths(parent, &commit) {
                Ok(changed) if changed == [path.as_str()] => match &held {
                    Err(why) => Some(why.clone()),
                    Ok(_) if carries => None,
                    Ok(held) => settled(git, scratch, held, proposal),
                },
                Ok(changed) => Some(format!("changes {}", changed.join(", "))),
                Err(error) => Some(one_line(&format!("cannot be read: {error}"))),
            },
            parents => Some(format!("has {} parents", parents.len())),
        },
        Err(error) => Some(one_line(&format!("cannot be read: {error}"))),
    };
    TrailerCommit {
        commit,
        not_completing,
        carries,
    }
}

/// The target in one commit's blob of its path (raw bytes).
struct Held {
    bytes: Vec<u8>,
    node: Node,
}

impl Held {
    /// The span is `text` as an apply splices it.
    fn holds(&self, text: &str) -> bool {
        span_bytes(&self.bytes, &self.node) == update_text(text, is_section(&self.node)).as_bytes()
    }
}

/// `commit`'s span is step 5's text on `parent`'s blob of the path: the
/// new text where the parent's span is the base, else the merge an apply
/// on top of `parent` wrote (one run of `git merge-file`, which is not
/// idempotent: the merge run again on its own result may differ). Anything
/// unreadable in the parent: no.
fn written_on(
    git: &WorktreeGit,
    scratch: &Path,
    held: &Held,
    parent: &str,
    proposal: &Proposal,
    config_now: &dyn Fn() -> Result<ProjectConfig, String>,
) -> bool {
    let Ok(before) = held_at(git, parent, proposal, config_now, "its parent's") else {
        return false;
    };
    matches!(
        step_text(git, scratch, &before.bytes, &before.node, proposal),
        Ok(StepText::Text(text, _)) if held.holds(&text)
    )
}

/// Why the span is not step 5's text on its own blob (the new text, or a
/// merge the proposal's edit leaves as it is); `None`: it is.
fn settled(git: &WorktreeGit, scratch: &Path, held: &Held, proposal: &Proposal) -> Option<String> {
    match step_text(git, scratch, &held.bytes, &held.node, proposal) {
        Ok(StepText::Text(text, _)) if held.holds(&text) => None,
        Ok(_) => Some("does not carry the proposal's text".to_owned()),
        Err(error) => Some(one_line(&format!("its text cannot be compared: {error}"))),
    }
}

/// The target located in `rev`'s blob of the path, parsed under the
/// `[ids]` of [`config_at`]; else why not, `whose` naming the blob ("its",
/// "its parent's").
fn held_at(
    git: &WorktreeGit,
    rev: &str,
    proposal: &Proposal,
    config_now: &dyn Fn() -> Result<ProjectConfig, String>,
    whose: &str,
) -> Result<Held, String> {
    let (config, config_label) = config_at(git, rev, proposal, config_now, whose)?;
    let path = top_path(proposal);
    let bytes = git
        .blob_at(rev, &path)
        .map_err(|error| one_line(&format!("{whose} `{path}` cannot be read: {error}")))?;
    if std::str::from_utf8(&bytes).is_err() {
        return Err(format!("{whose} `{path}` is not UTF-8"));
    }
    let scheme = &config.scheme;
    let target_path = proposal.target_path.as_str();
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(target_path, &bytes, scheme)
    }))
    .map_err(|_| format!("the spec parser failed on {whose} `{path}`"))?;
    let target = proposal.target_id.as_str();
    let ord = if is_path_target(target) {
        // A path target: the document.
        document_of(&parsed, target, target_path)
            .map_err(|reason| format!("{whose} blob: {reason}"))?
    } else {
        let Some(found) = grammar::parse_reference(target, 0, scheme) else {
            return Err(format!(
                "`{target}` is no reference under the `[ids]` of {config_label}"
            ));
        };
        let id = &found.reference.id;
        locate(&parsed, id)
            .map_err(|_| format!("{whose} `{path}` does not hold `{id}` exactly once"))?
    };
    let node = parsed.nodes[ord].clone();
    Ok(Held { bytes, node })
}

/// The config `rev`'s blob is parsed under, with its label: the
/// `<root_rel>/specengine.toml` in `rev`'s tree; when the tree has none
/// (the config not committed), `config_now`.
fn config_at(
    git: &WorktreeGit,
    rev: &str,
    proposal: &Proposal,
    config_now: &dyn Fn() -> Result<ProjectConfig, String>,
    whose: &str,
) -> Result<(ProjectConfig, String), String> {
    let root_rel = &proposal.place.root_rel;
    let config_path = if root_rel.is_empty() {
        CONFIG_FILE.to_owned()
    } else {
        format!("{root_rel}/{CONFIG_FILE}")
    };
    let bytes = match git.blob_at(rev, &config_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return match git.has_path(rev, &config_path) {
                Ok(false) => config_now()
                    .map(|config| (config, format!("the `{CONFIG_FILE}` read now")))
                    .map_err(|now| {
                        one_line(&format!(
                            "{whose} tree has no `{config_path}`, and the one read now: {now}"
                        ))
                    }),
                _ => Err(one_line(&format!(
                    "{whose} `{config_path}` cannot be read: {error}"
                ))),
            };
        }
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Err(format!("{whose} `{config_path}` is not UTF-8"));
    };
    let label = format!("{rev}:{config_path}");
    match ProjectConfig::from_toml(&text) {
        Ok(config) => Ok((config, format!("{whose} `{config_path}`"))),
        Err(error) => Err(one_line(&config_error(&label, &error).message)),
    }
}
