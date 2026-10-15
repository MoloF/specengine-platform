//! `spec reject PR --reason T --follow-up code|spec` of a `drift` item
//! (task spec `drift-follow-up`; canon `proposal-completion`, "Reject"):
//! the reject and a `draft` task in one transaction, the task's title and
//! goal core's ([`follow_up_title`], [`follow_up_goal`]). What
//! [`crate::apply`]'s reject asks of a follow-up besides its own checks:
//!
//! - after the item is read, before its state: another kind exits 2
//!   `--follow-up: only a drift item has a follow-up`; an orphan, or a
//!   recorded worktree that no longer exists, exits 1 naming it (`reject
//!   without it`); the recorded root's `[ids]` taking `T` (core
//!   [`task_prefix_clash`]) exits 2, as `spec task new` ([`recorded_for`]);
//! - step 1, after reject's checks, before the question: the node resolved
//!   now at the recorded root as `spec task new --nodes` resolves one, its
//!   revision and path read there; gone (no node has its canonical ID, an
//!   alias included, and every file was read and parsed) exits 1
//!   `--follow-up: R-12 names no node now; reject without it`; not one
//!   node (several holders, declared twice, a file that was not read or
//!   does not parse) exits 1 with the resolver's reason, `--follow-up:
//!   <reason>; reject without it`; its revision unreadable exits 1
//!   ([`planned`]);
//! - step 2: the title and the goal shown above reject's question, which
//!   ends ` and make a draft task (code)? [y/N]` ([`shown`]);
//! - step 4: the note naming the task made ([`created_note`]).
//!
//! The rows in the goal are the item's stored ones as `spec review` prints
//! them (the one row formatter): text, never re-read. Nothing here writes.

use specengine_core::check::CheckFile;
use specengine_core::revision;
use specengine_core::task::{
    FollowUp, FollowUpItem, follow_up_goal, follow_up_title, task_prefix_clash,
};
use specengine_store::{GitEnv, Proposal};

use crate::corpus::indexed;
use crate::lock::canonical_ids;
use crate::preflight::{History, history, recorded_project, recorded_root};
use crate::project::ProjectRoot;
use crate::proposals::{QueueContext, drift_row_line};
use crate::propose::resolved_at;
use crate::{CliError, Env, Message};

/// The refusal of `--follow-up` on another kind (exit 2).
pub(crate) const ONLY_DRIFT: &str = "--follow-up: only a drift item has a follow-up";

/// What the follow-up stores beside the reject: its node and texts.
pub(crate) struct Planned {
    /// The node's canonical ID, read now: the task's one target.
    pub node: String,
    pub title: String,
    pub goal: String,
}

/// The recorded root as a project, where the follow-up's node is read:
/// `Ok(Err)` the refusal (exit 1) of an `orphan` (its repository gone or
/// moved) or of a recorded worktree that no longer exists (or is no longer
/// the repository's); exit 2 when its config cannot be read or its `[ids]`
/// takes `T`.
pub(crate) fn recorded_for(
    git_env: &GitEnv,
    context: &QueueContext,
    proposal: &Proposal,
    orphan: bool,
) -> Result<Result<ProjectRoot, String>, CliError> {
    let id = &proposal.id;
    if orphan {
        return Ok(Err(format!(
            "--follow-up: the repository {} of {id} no longer exists; reject without it",
            proposal.place.git_common_dir
        )));
    }
    if !matches!(
        history(git_env, context, proposal),
        Ok(History::Recorded(_))
    ) {
        return Ok(Err(format!(
            "--follow-up: the worktree {} of {id} no longer exists; reject without it",
            proposal.place.worktree
        )));
    }
    let recorded = recorded_project(&recorded_root(proposal), &context.slug)
        .map_err(|error| CliError::spec(format!("--follow-up: {error}")))?;
    if let Some(clash) = task_prefix_clash(&recorded.config.scheme) {
        return Err(CliError::spec(format!(
            "--follow-up: {}: {clash}",
            recorded.config_label
        )));
    }
    Ok(Ok(recorded))
}

/// Step 1 and the texts: the item's node resolved now at `recorded` (its
/// index brought up to date, its warnings into `messages`), its revision
/// and path read there, and core's title and goal over them, `reason` and
/// the stored rows. `Ok(Err)`: the refusal (exit 1), a gone node's
/// `--follow-up: R-12 names no node now; reject without it` (never while
/// a file's nodes are unknown), else the resolver's reason or the
/// unreadable revision, in the same frame.
pub(crate) fn planned(
    env: &Env,
    recorded: &ProjectRoot,
    proposal: &Proposal,
    follow_up: FollowUp,
    reason: &str,
    messages: &mut Vec<Message>,
) -> Result<Result<Planned, String>, CliError> {
    let target = proposal.target_id.as_str();
    let without = |why: String| Ok(Err(format!("--follow-up: {why}; reject without it")));
    let input = indexed(env, recorded, messages, false)?;
    let (node, path, ord) = match resolved_at(recorded, &input, target)? {
        Ok(found) => found,
        Err(reason) => {
            // Gone only when every file's nodes are known: one that was
            // not read or does not parse may hold it.
            let gone = input.files.iter().all(nodes_known)
                && !canonical_ids(&input, &recorded.config.scheme, recorded).contains(target);
            return without(if gone {
                format!("{target} names no node now")
            } else {
                reason
            });
        }
    };
    let rev = input
        .files
        .iter()
        .find(|file| file.path == path)
        .and_then(|file| file.parsed.as_ref())
        .and_then(|parsed| revision(parsed, ord).number());
    let Some(rev) = rev else {
        return without(format!(
            "the revision of {node} in {path} cannot be read now"
        ));
    };
    let (rows, more) = proposal.drift.as_ref().map_or((Vec::new(), 0), |drift| {
        (
            drift.evidence.rows.iter().map(drift_row_line).collect(),
            drift.evidence.more,
        )
    });
    let item = FollowUpItem {
        follow_up,
        node: &node,
        rev,
        path: &path,
        item: &proposal.id,
        reason,
        rows: &rows,
        more,
    };
    let (title, goal) = (follow_up_title(&item), follow_up_goal(&item));
    Ok(Ok(Planned { node, title, goal }))
}

/// The file's nodes are known: it was read and parsed (an unreadable file
/// or a parser panic leaves no parse, a file that is not UTF-8 no node).
fn nodes_known(file: &CheckFile) -> bool {
    file.parsed
        .as_ref()
        .is_some_and(|parsed| parsed.document().is_some())
}

/// The title and the goal as the owner reads them above the question:
/// `draft task: <title>`, `goal:`, then each goal line indented; each line
/// ends in LF (the caller escapes the whole).
pub(crate) fn shown(planned: &Planned) -> String {
    let mut out = format!("draft task: {}\ngoal:\n", planned.title);
    for line in planned.goal.lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// What reject's question gains before its `?`.
pub(crate) fn question_tail(follow_up: FollowUp) -> String {
    format!(" and make a draft task ({})", follow_up.as_str())
}

/// Step 4's note: `created T-0003 (draft) for PR-0012: spec task approve
/// T-0003 makes it ready`.
pub(crate) fn created_note(task: &str, id: &str) -> String {
    format!("created {task} (draft) for {id}: spec task approve {task} makes it ready")
}
