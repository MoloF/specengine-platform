//! `spec auto approve T` (task spec `auto-land` "`spec auto approve`"):
//! the runner's approve of a pack's task, between its plan and implement
//! legs, with no terminal. Run in the worktree the task will be claimed in
//! (`--root`): the snapshot is frozen there, as `spec task approve`
//! freezes it, `by` that worktree's git identity.
//!
//! The predicate ([`judge_approve`]): the switches (exit 1 `auto is off:
//! <why>`, nothing written), membership (`not-in-pack`, no event), the
//! pack's `until`, `status` (it needs `review`; no event: a caller's
//! error), then the paths and kinds of its targets and affected nodes.
//! Apply: `review` → `ready` with `task.approved {id, by: "rule", pack,
//! rule}`; any other hold logs one `auto.held` (`act` `approve`).

use serde::Serialize;
use specengine_core::auto::{Act, Reason, Surface, Switches, Verdict, judge_approve};
use specengine_model::{SnapshotPlace, TaskStatus};
use specengine_store::{AutoEvent, AutoHeld, GitEnv, QueueError, TaskSnapshot};

use crate::auto::{AutoOutcome, frozen_off, json_text};
use crate::auto_pack::{auto_state, pack_of};
use crate::auto_read::task_facts;
use crate::corpus::indexed;
use crate::proposals::{checked_now, escaped_error, queue_cannot};
use crate::task::{find_task, frozen_nodes, not_a_task, written_task};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls};

/// `spec auto approve T` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoApproveRequest {
    /// `T` as given.
    pub id: String,
    pub now: String,
    pub git: GitEnv,
}

#[derive(Serialize)]
struct ApprovedJson<'a> {
    id: &'a str,
    status: &'a str,
    by: Option<&'a str>,
    pack: Option<i64>,
    rule: Option<&'a str>,
    reason: Option<&'a str>,
    detail: Option<&'a str>,
    notes: &'a [String],
}

/// `spec auto approve` (see the module documentation): exit 0 approved, 1
/// held or refused, 2 when it cannot run.
pub fn auto_approve(
    env: &Env,
    globals: &Globals,
    request: &AutoApproveRequest,
) -> Result<AutoOutcome, CliError> {
    run(env, globals, request).map_err(escaped_error)
}

fn run(
    env: &Env,
    globals: &Globals,
    request: &AutoApproveRequest,
) -> Result<AutoOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let written = written_task(&request.id)?;
    let state = auto_state(env, globals, &request.git, Surface::Local)?;
    let Some(id) = written else {
        return Ok(AutoOutcome::refused(not_a_task(&request.id)));
    };
    if let Some(line) = state.rule.off_line() {
        return Ok(AutoOutcome::refused(line));
    }
    let Some(rule) = state.rule.rule.as_ref() else {
        return Ok(AutoOutcome::refused("auto is off".to_owned()));
    };
    let task = match find_task(&state.context, &id)? {
        Ok(task) => task,
        Err(reason) => return Ok(AutoOutcome::refused(format!("{reason}; nothing approved"))),
    };
    let held_document = |reason: Reason, detail: &str, pack: Option<i64>| {
        json_text(&ApprovedJson {
            id: &task.id,
            status: task.status.as_str(),
            by: None,
            pack,
            rule: Some(&rule.hash),
            reason: Some(reason.as_str()),
            detail: Some(detail),
            notes: &[],
        })
    };
    let held_line = |reason: Reason, detail: &str| {
        format!("{} held ({reason}): {detail}; nothing approved", task.id)
    };
    let Some((pack, _)) = pack_of(&state.packs, &id, &state.tasks, false) else {
        let detail = format!("`{id}` is in no live pack");
        return Ok(AutoOutcome::refused_with(
            &held_line(Reason::NotInPack, &detail),
            held_document(Reason::NotInPack, &detail, None),
        ));
    };
    let mut messages: Vec<Message> = Vec::new();
    let input = indexed(env, &state.context.project, &mut messages, false)?;
    let switches = Switches {
        surface: Surface::Local,
        ..state.rule.switches()
    };
    let verdict = match pack.facts() {
        Ok(facts_of_pack) => {
            let facts = task_facts(&state.context.project, &input, &task, Some(facts_of_pack))?;
            judge_approve(&switches, &facts, now)
        }
        Err(why) => frozen_off(why),
    };
    if let Verdict::Hold { reason, detail } = verdict {
        if reason.is_switch() {
            return Ok(AutoOutcome::refused(format!("auto is off: {detail}")));
        }
        let outcome = AutoOutcome::refused_with(
            &held_line(reason, &detail),
            held_document(reason, &detail, Some(pack.seq)),
        );
        // `status` is a caller's error, not the task's: no event.
        if reason != Reason::Status {
            let mut context = state.context;
            context
                .queue
                .record_auto(
                    &AutoEvent::Held(AutoHeld {
                        pack: Some(pack.seq),
                        task: Some(task.id.clone()),
                        line: None,
                        proposal: None,
                        act: Act::Approve.as_str().to_owned(),
                        reason: reason.as_str().to_owned(),
                        detail,
                        rule: rule.hash.clone(),
                    }),
                    now,
                )
                .map_err(queue_cannot)?;
        }
        return Ok(outcome);
    }
    // The snapshot, as `spec task approve` freezes it, in this worktree.
    let place = state
        .context
        .git
        .place(&state.context.project.root)
        .map_err(|error| {
            CliError::spec(format!(
                "the project root {} cannot be the snapshot's place: {error}",
                state.context.project.root.display()
            ))
        })?;
    let by = state.context.git.committer_ident().map_err(|error| {
        CliError::spec(format!(
            "no git identity in {}: {error}; set user.name and user.email",
            place.worktree
        ))
    })?;
    let nodes = match frozen_nodes(&state.context.project, &input, &task)? {
        Ok(nodes) => nodes,
        Err(reason) => return Ok(AutoOutcome::refused(format!("{reason}; nothing approved"))),
    };
    let snapshot = TaskSnapshot {
        at: now.to_owned(),
        by,
        place: SnapshotPlace {
            worktree: place.worktree.clone(),
            root_rel: place.root_rel.clone(),
            branch: place.branch.clone(),
            commit: place.base_commit.clone(),
        },
        nodes,
    };
    let pack_seq = pack.seq;
    let hash = rule.hash.clone();
    let mut context = state.context;
    let stored = match context.queue.approve_task_by_rule(
        &task.id,
        &task.seen(),
        &snapshot,
        pack_seq,
        &hash,
        now,
    ) {
        Ok(stored) => stored,
        Err(QueueError::TaskRefused { reason, .. }) => return Ok(AutoOutcome::refused(reason)),
        Err(error) => return Err(queue_cannot(error)),
    };
    let status = stored.status;
    debug_assert_eq!(status, TaskStatus::Ready);
    let notes: Vec<String> = Vec::new();
    Ok(AutoOutcome {
        exit: Exit::Answered,
        text: escape_controls(&format!(
            "approved {}: {status} (by rule, pack {pack_seq})\n",
            stored.id
        )),
        json: json_text(&ApprovedJson {
            id: &stored.id,
            status: status.as_str(),
            by: Some("rule"),
            pack: Some(pack_seq),
            rule: Some(&hash),
            reason: None,
            detail: None,
            notes: &notes,
        }),
        stderr: messages.iter().map(Message::line).collect(),
    })
}
