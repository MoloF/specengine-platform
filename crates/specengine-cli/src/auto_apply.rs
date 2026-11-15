//! The apply a delegated act runs once its caller has judged it (task spec
//! `auto-rule` "Events and the apply seam"; canon `auto-mode`):
//! [`delegated_approve`] rehearses `spec approve <id>` with a consent
//! answering no, capturing its question and logging nothing (as a hub's
//! confirm reads its question), then runs it again with the one consent
//! `asked == question`, byte for byte. A row changed between the two, or a
//! question that is not the rehearsed one, is [`DelegatedOutcome::Moved`]:
//! nothing written. The commit ends `Delegated-by: auto <rule>`; the
//! events carry `by: "rule"` ([`crate::apply::Delegation`]).
//!
//! It judges nothing: its callers (`spec auto land`, a hub's `auto_apply`)
//! judge first. No http `src` file may call it.

use specengine_store::{GitEnv, Proposal, Seen};

use crate::apply::{ApproveFlags, ApproveRequest, Delegation, HubRun, run_approve_as};
use crate::proposals::{
    Find, ProposalDocument, ProposalOutcome, QueueCommand, escaped_error, find, no_proposal,
    open_context, written_id,
};
use crate::{CliError, Env, Globals};

// A hub's `auto_apply` (task spec `hub-auto`), below.
use specengine_core::auto::{Act, Reason, Surface, Verdict, judge_proposal};
use specengine_store::{AutoEvent, AutoHeld, ProposalStatus};

use crate::auto_read::{load_auto, proposal_facts};
use crate::corpus::indexed;
use crate::proposals::{checked_now, queue_cannot};
use crate::{Message, escape_controls, one_line};

/// [`delegated_approve`]'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedApproveRequest {
    /// `PR` as given.
    pub id: String,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
    /// The rule's hash and, locally, the pack.
    pub delegation: Delegation,
    /// A hub's login and its `[team]` `email`, set as `GIT_AUTHOR_*` and
    /// `GIT_COMMITTER_*` (`hub-confirm.md` "Identity"); `None` keeps the
    /// worktree's git identity.
    pub identity: Option<(String, String)>,
    /// The row's state the caller's predicate judged
    /// ([`specengine_store::Proposal::seen`]): when set, the row is compared
    /// with it before the rehearsal and again before the run, a mismatch
    /// [`DelegatedOutcome::Moved`], nothing written. `None`: the row as
    /// first read here is the one judged.
    pub expected: Option<Seen>,
}

/// What [`delegated_approve`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegatedOutcome {
    /// Applied (or completed by its own commit): the review document.
    Applied(ProposalOutcome),
    /// The rehearsal or the run refused it, before or at a step: the
    /// refused document, its last note the reason.
    Refused(ProposalOutcome),
    /// The row or its question changed between the rehearsal and the run:
    /// nothing written.
    Moved { reason: String },
}

impl DelegatedOutcome {
    /// The applied commit, when applied.
    pub fn commit(&self) -> Option<&str> {
        match self {
            Self::Applied(outcome) => outcome.document.applied_commit.as_deref(),
            _ => None,
        }
    }

    /// Why nothing was applied; `None` when applied.
    pub fn reason(&self) -> Option<String> {
        match self {
            Self::Applied(_) => None,
            Self::Refused(outcome) => Some(
                outcome
                    .refusal
                    .clone()
                    .unwrap_or_else(|| "refused".to_owned()),
            ),
            Self::Moved { reason } => Some(reason.clone()),
        }
    }
}

/// Rehearses, then runs, `spec approve <id>` marked by the request's
/// delegation (see the module documentation). Exit 2 ([`CliError`]) only
/// when it cannot run here.
pub fn delegated_approve(
    env: &Env,
    globals: &Globals,
    request: &DelegatedApproveRequest,
) -> Result<DelegatedOutcome, CliError> {
    run(env, globals, request).map_err(escaped_error)
}

fn run(
    env: &Env,
    globals: &Globals,
    request: &DelegatedApproveRequest,
) -> Result<DelegatedOutcome, CliError> {
    let git = match &request.identity {
        Some((login, email)) => request
            .git
            .clone()
            .with_var("GIT_COMMITTER_NAME", login)
            .with_var("GIT_COMMITTER_EMAIL", email)
            .with_var("GIT_AUTHOR_NAME", login)
            .with_var("GIT_AUTHOR_EMAIL", email),
        None => request.git.clone(),
    };
    let judged = match read_row(env, globals, &git, &request.id)? {
        Ok(proposal) => {
            if let Some(moved) = moved_from(&proposal, request.expected.as_ref()) {
                return Ok(moved);
            }
            proposal.seen()
        }
        Err(refused) => return Ok(DelegatedOutcome::Refused(refused)),
    };
    let approve = ApproveRequest {
        id: request.id.clone(),
        note: None,
        now: request.now.clone(),
        git: git.clone(),
    };
    let flags = ApproveFlags::default();
    let delegation = Some(&request.delegation);
    // The rehearsal: its question captured, answered no; it logs nothing.
    let mut question: Option<String> = None;
    let mut heard = |asked: &str| {
        question = Some(asked.to_owned());
        false
    };
    let rehearsal = run_approve_as(
        env,
        globals,
        &approve,
        &flags,
        false,
        Some(HubRun {
            judged: &judged,
            rehearsal: true,
        }),
        delegation,
        &mut heard,
    )?;
    let Some(question) = question else {
        return moved_or(env, globals, &git, &request.id, &judged, rehearsal);
    };
    // Before the run: the row still the one judged.
    if request.expected.is_some() {
        match read_row(env, globals, &git, &request.id)? {
            Ok(proposal) => {
                if let Some(moved) = moved_from(&proposal, Some(&judged)) {
                    return Ok(moved);
                }
            }
            Err(refused) => return Ok(DelegatedOutcome::Refused(refused)),
        }
    }
    // The run: the one consent, the rehearsed question byte for byte.
    let mut asked_any = false;
    let mut moved = false;
    let mut same = |asked: &str| {
        asked_any = true;
        moved = asked != question;
        !moved
    };
    let ran = run_approve_as(
        env,
        globals,
        &approve,
        &flags,
        false,
        Some(HubRun {
            judged: &judged,
            rehearsal: false,
        }),
        delegation,
        &mut same,
    )?;
    if moved {
        return Ok(DelegatedOutcome::Moved {
            reason: format!(
                "`{}` moved since its rehearsal: its question changed; nothing changed",
                request.id.trim()
            ),
        });
    }
    if ran.refusal.is_none() {
        return Ok(DelegatedOutcome::Applied(ran));
    }
    if asked_any {
        return Ok(DelegatedOutcome::Refused(ran));
    }
    moved_or(env, globals, &git, &request.id, &judged, ran)
}

/// [`DelegatedOutcome::Moved`] when `proposal` is not the state `judged`
/// (none given: never).
fn moved_from(proposal: &Proposal, judged: Option<&Seen>) -> Option<DelegatedOutcome> {
    let judged = judged?;
    (proposal.seen() != *judged).then(|| DelegatedOutcome::Moved {
        reason: format!(
            "`{}` changed since it was judged: it is {} since {}; nothing changed",
            proposal.id, proposal.status, proposal.updated_at
        ),
    })
}

/// A refusal before the question: [`DelegatedOutcome::Moved`] when the row
/// is no longer the one judged, else the refusal.
fn moved_or(
    env: &Env,
    globals: &Globals,
    git: &GitEnv,
    written: &str,
    judged: &Seen,
    refused: ProposalOutcome,
) -> Result<DelegatedOutcome, CliError> {
    match read_row(env, globals, git, written)? {
        Ok(proposal) if proposal.seen() != *judged => Ok(DelegatedOutcome::Moved {
            reason: format!(
                "`{}` changed since it was judged: it is {} since {}; nothing changed",
                proposal.id, proposal.status, proposal.updated_at
            ),
        }),
        _ => Ok(DelegatedOutcome::Refused(refused)),
    }
}

/// The row `written` names, of this repository; else the refused document.
fn read_row(
    env: &Env,
    globals: &Globals,
    git: &GitEnv,
    written: &str,
) -> Result<Result<Proposal, ProposalOutcome>, CliError> {
    let refused = |reason: &str| {
        ProposalOutcome::refused(
            QueueCommand::Approve,
            ProposalDocument::default(),
            reason,
            Vec::new(),
        )
    };
    let Some(id) = written_id(written)? else {
        return Ok(Err(refused(&no_proposal(written))));
    };
    let context = open_context(env, globals, git)?;
    Ok(find(&context, &id, Find::SameRepository)?.map_err(|reason| refused(&reason)))
}

// ------------------------------------------------------------ a hub's apply

/// [`auto_apply`]'s options: one proposal a hub's `/mcp` call stored
/// (canon `hub-auto` "Rules", its one call).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoApplyRequest {
    /// `PR` as the propose's answer gave it.
    pub id: String,
    /// The login of the `/mcp` call that stored it, as the hub's fence
    /// judged it.
    pub login: String,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// What [`auto_apply`] did, a closed set (task spec `hub-auto` "Data").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoApplyOutcome {
    /// A switch holds (`off`, `team`, `stopped`): nothing judged, no event.
    /// `absent`: `HEAD` holds no `[auto]` table
    /// ([`crate::EffectiveRule::absent`]).
    Off {
        reason: Reason,
        why: String,
        absent: bool,
    },
    /// The row is not open here (decided, or no such row): no event.
    Gone,
    /// Held: one `auto.held` logged (`act` `apply`), the row left open.
    /// `ran`: the apply ran and refused at a step (held `rebases`), so it
    /// may have written the tree; else a predicate's hold, which wrote only
    /// its event.
    Held {
        reason: Reason,
        detail: String,
        ran: bool,
    },
    /// Applied: its commit; `None` when the applied document names none
    /// (never expected).
    Applied { commit: Option<String> },
    /// The row or its question changed between the rehearsal and the run:
    /// nothing written, no event.
    Moved,
}

impl AutoApplyOutcome {
    /// What a hub's daemon prints after `auto <slug> <id> by <login>: `, one
    /// line, controls escaped: `applied <sha12>`, `held (<reason>):
    /// <detail>`, `off (<reason>): <why>`, `gone` or `moved`; an apply
    /// naming no commit `cannot run: applied, but its document names no
    /// commit`. `None` when no `[auto]` table is at `HEAD`: a hub without one
    /// prints nothing, as 0.46.1 did (every other `off` is printed).
    pub fn summary(&self) -> Option<String> {
        let text = match self {
            Self::Off { absent: true, .. } => return None,
            Self::Off { reason, why, .. } => format!("off ({reason}): {why}"),
            Self::Gone => "gone".to_owned(),
            Self::Held { reason, detail, .. } => format!("held ({reason}): {detail}"),
            Self::Applied {
                commit: Some(commit),
            } => format!("applied {}", commit.get(..12).unwrap_or(commit)),
            Self::Applied { commit: None } => {
                "cannot run: applied, but its document names no commit".to_owned()
            }
            Self::Moved => "moved".to_owned(),
        };
        Some(escape_controls(&one_line(&text)))
    }

    /// Whether the project's index is doubted after it: an apply that ran
    /// and refused (it may have written the tree), or one naming no commit.
    /// A predicate's hold, a switch, a gone or moved row wrote no file.
    pub fn doubts(&self) -> bool {
        matches!(
            self,
            Self::Held { ran: true, .. } | Self::Applied { commit: None }
        )
    }
}

/// A hub's delegated apply of one proposal (canon `hub-auto`
/// "Rules"), in one call inside the project's turn: the rule read from the
/// checkout's `HEAD` ([`load_auto`], the hub surface), its switches (a
/// switch: [`AutoApplyOutcome::Off`], no event), the row read once (not
/// open: [`AutoApplyOutcome::Gone`]), its facts, today's delegated applies
/// (UTC), then [`judge_proposal`], `cap` last. A hold logs one `auto.held`;
/// an apply is [`delegated_approve`] under the login and its `[team]`
/// `email`, a refusal at its steps held `rebases` with the refusal as its
/// detail. Exit 2 ([`CliError`]) only when it cannot run.
pub fn auto_apply(
    env: &Env,
    globals: &Globals,
    request: &AutoApplyRequest,
) -> Result<AutoApplyOutcome, CliError> {
    run_auto_apply(env, globals, request).map_err(escaped_error)
}

fn run_auto_apply(
    env: &Env,
    globals: &Globals,
    request: &AutoApplyRequest,
) -> Result<AutoApplyOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let mut context = open_context(env, globals, &request.git)?;
    let effective = load_auto(env, &context.project, &request.git, Surface::Hub)?;
    let absent = effective.absent;
    if let Some((reason, why)) = effective.switch() {
        return Ok(AutoApplyOutcome::Off {
            reason,
            why,
            absent,
        });
    }
    let Some(rule) = effective.rule.as_ref() else {
        return Ok(AutoApplyOutcome::Off {
            reason: Reason::Off,
            why: "[auto] does not read".to_owned(),
            absent,
        });
    };
    // The row, read once: judged as it is now, or not at all.
    let Some(id) = written_id(&request.id)? else {
        return Ok(AutoApplyOutcome::Gone);
    };
    let proposal = match find(&context, &id, Find::SameRepository)? {
        Ok(proposal) if proposal.status == ProposalStatus::Open => proposal,
        _ => return Ok(AutoApplyOutcome::Gone),
    };
    let mut messages: Vec<Message> = Vec::new();
    let input = indexed(env, &context.project, &mut messages, false)?;
    let facts = proposal_facts(
        env,
        &request.git,
        &context,
        &input,
        &proposal,
        None,
        &mut messages,
    );
    let applies_today = context
        .queue
        .rule_applies_on(&now[..10])
        .map_err(queue_cannot)?;
    let switches = effective.switches();
    let verdict = judge_proposal(&switches, None, &facts, Some(applies_today), now);
    // The login whose call stored it, its `[team]` entry the identity.
    let entry = effective
        .team
        .as_ref()
        .and_then(|team| team.entry(&request.login));
    let by_login = facts.author_login.as_deref() == Some(request.login.as_str());
    // A hold: its reason, its detail, and whether the apply ran first.
    let (reason, detail, ran) = match (verdict, entry) {
        (Verdict::Hold { reason, detail }, _) if reason.is_switch() => {
            return Ok(AutoApplyOutcome::Off {
                reason,
                why: detail,
                absent,
            });
        }
        (Verdict::Hold { reason, detail }, _) => (reason, detail, false),
        (Verdict::Apply, Some(entry)) if by_login => {
            let ran = delegated_approve(
                env,
                globals,
                &DelegatedApproveRequest {
                    id: proposal.id.clone(),
                    now: now.to_owned(),
                    git: request.git.clone(),
                    delegation: Delegation {
                        rule: rule.hash.clone(),
                        pack: None,
                    },
                    identity: Some((entry.login.clone(), entry.email.clone())),
                    // The row judged above: one changed since is `Moved`.
                    expected: Some(proposal.seen()),
                },
            )?;
            match &ran {
                DelegatedOutcome::Applied(_) => {
                    return Ok(AutoApplyOutcome::Applied {
                        commit: ran.commit().map(str::to_owned),
                    });
                }
                DelegatedOutcome::Moved { .. } => return Ok(AutoApplyOutcome::Moved),
                DelegatedOutcome::Refused(_) => (
                    Reason::Rebases,
                    ran.reason().unwrap_or_else(|| "refused".to_owned()),
                    true,
                ),
            }
        }
        (Verdict::Apply, _) => (
            Reason::Author,
            format!(
                "`{}` was not raised over /mcp by `{}`, the sole owner login of [team]",
                proposal.id, request.login
            ),
            false,
        ),
    };
    context
        .queue
        .record_auto(
            &AutoEvent::Held(AutoHeld {
                pack: None,
                task: None,
                line: None,
                proposal: Some(proposal.id.clone()),
                act: Act::Apply.as_str().to_owned(),
                reason: reason.as_str().to_owned(),
                detail: detail.clone(),
                rule: rule.hash.clone(),
            }),
            now,
        )
        .map_err(queue_cannot)?;
    Ok(AutoApplyOutcome::Held {
        reason,
        detail,
        ran,
    })
}
