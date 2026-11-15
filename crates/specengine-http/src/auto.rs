//! A hub's delegated apply (docs/features/hub-auto.md, ADR-0082;
//! `docs/canon/hub-auto.md`): the one file of the daemon that calls the
//! CLI's `auto_apply`, and it calls nothing else of the CLI's writers. It
//! holds no consent: the library judges the rule, the row and the cap, and
//! answers its own question byte for byte (`tests/door.rs`).
//!
//! - **When.** A hub's `propose_change` that stored an `update` or a
//!   `create` tells its seam once its answer is built ([`crate::mcp`]); the
//!   seam hands the row here ([`queue`]). Nothing else does: no route, no
//!   other tool, no local daemon (which mounts no `/mcp`).
//! - **Where.** One command in that login's FIFO line for the project
//!   ([`crate::turn::Lines`]), joined before the propose's answer leaves, so
//!   it runs after the propose and before that login's next call, then in
//!   the project's turn, as an MCP call of that login (class `mcp`): a page
//!   request waits for at most one call per login, this one counted among
//!   them. No request owns it, so nothing cancels it; a daemon stopping drops
//!   it, the row left open.
//! - **What.** The library's one call inside the turn, in the `walking`
//!   scope, its hold's slow lines `by=<login> id=<proposal> tool=auto_apply
//!   class=mcp`; its outcome on stderr in one line, never to the agent, none
//!   when `HEAD` holds no `[auto]` table (a hub without one logs as 0.46.1
//!   did):
//!
//! ```text
//! specengine-http: auto <slug> <id> by <login>: applied <sha12>
//! specengine-http: auto <slug> <id> by <login>: held (<reason>): <detail>
//! specengine-http: auto <slug> <id> by <login>: off (<reason>): <why>
//! specengine-http: auto <slug> <id> by <login>: <gone|moved>
//! specengine-http: auto <slug> <id> by <login>: cannot run: <the CLI's line>
//! ```
//!
//!   A commit signals a `--push` hub's sync, as a confirm's does. A run
//!   that may have written the tree doubts the project's index, as a refused
//!   confirm does (the `read-trust` spec, D9): an apply refused at a step,
//!   one naming no commit, a command that cannot run or panics; a
//!   predicate's hold, which writes one queue event, does not.

use std::future::{Future, poll_fn};
use std::pin::{Pin, pin};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use specengine_cli::{
    AutoApplyOutcome, AutoApplyRequest, CliError, Env, escape_controls, process_git, utc_now,
};

use crate::answer::{self, DoubtOnPanic, Freshness};
use crate::start::Project;
use crate::sync::RepoSync;
use crate::{slow, turn};

/// The library call a delegated apply's turn holds for: its slow line's
/// `tool` (`by` the login, `id` the proposal, class `mcp`).
const TOOL: &str = "auto_apply";

/// The command's work, its line and turn awaited first.
type Work = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Queues the delegated apply of `id`, which `login`'s `/mcp` call stored in
/// `project`, in that login's line (see the module documentation). Called
/// on the runtime, never awaited: its first step runs here, so the command
/// stands in the line before the propose's answer is sent; the rest is a
/// task of its own. Outside a runtime (never on a hub) nothing is queued.
pub(crate) fn queue(project: &Arc<Project>, sync: Option<Arc<RepoSync>>, login: &str, id: &str) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let project = Arc::clone(project);
    let login = login.to_owned();
    let id = id.to_owned();
    let mut work: Work = Box::pin(async move {
        let place = project.lines.line_up(&login, None).await;
        let holder = turn::Holder::delegated(&project, &login, &id, TOOL);
        let held = turn::take(&project, holder).await;
        // A panic leaves its line on stderr; the trust is doubted.
        let _ = tokio::task::spawn_blocking(move || {
            // Dropped in reverse: the turn, then the line's place.
            let _place = place;
            let _held = held;
            run(&project, sync.as_deref(), &login, &id);
        })
        .await;
    });
    // One poll now joins the login's FIFO (tokio's lock queues its waiter
    // on the first poll), outside the cooperative budget, which would
    // otherwise leave it unqueued when spent; the task polls it again with
    // its own waker.
    let mut context = Context::from_waker(Waker::noop());
    let first = tokio::task::unconstrained(poll_fn(|cx| Poll::Ready(work.as_mut().poll(cx))));
    let polled = pin!(first).poll(&mut context);
    if !matches!(polled, Poll::Ready(Poll::Ready(()))) {
        runtime.spawn(work);
    }
}

/// The command on the blocking pool, the turn and the place held: as a
/// hub's MCP call runs ([`crate::mcp`]), the process's `Env`, the project's
/// globals, the slug found again, then the library's one call in the
/// `walking` scope, its line on stderr.
fn run(project: &Project, sync: Option<&RepoSync>, login: &str, id: &str) {
    let trust = project.trust.clone();
    // Dropped before the turn: a panic doubts while it is held.
    let _doubt = DoubtOnPanic(trust.as_deref());
    let called = Env::from_process().and_then(|env| {
        let globals = project.globals();
        answer::found(&env, &globals, project)?;
        let request = AutoApplyRequest {
            id: id.to_owned(),
            login: login.to_owned(),
            now: utc_now(),
            git: process_git(&env),
        };
        answer::in_scope(trust.as_deref(), Freshness::Walk, || {
            specengine_cli::auto_apply(&env, &globals, &request)
        })
    });
    if let Some(line) = line(&project.slug, id, login, &called) {
        slow::write_line(&line);
    }
    if let (Ok(AutoApplyOutcome::Applied { .. }), Some(sync)) = (&called, sync) {
        sync.signal();
    }
    // Only a run that may have written the tree doubts the index: a
    // predicate's hold wrote one queue event, and the next page read keeps
    // its trust.
    let doubted = called.as_ref().map_or(true, AutoApplyOutcome::doubts);
    if doubted && let Some(trust) = &trust {
        trust.doubt();
    }
}

/// The command's stderr line (see the module documentation); `None` when
/// `HEAD` holds no `[auto]` table.
fn line(
    slug: &str,
    id: &str,
    login: &str,
    called: &Result<AutoApplyOutcome, CliError>,
) -> Option<String> {
    let said = match called {
        Ok(outcome) => outcome.summary()?,
        Err(error) => format!(
            "cannot run: {}",
            escape_controls(&error.message.replace(['\n', '\r'], " "))
        ),
    };
    Some(format!(
        "specengine-http: auto {slug} {id} by {login}: {said}"
    ))
}

/// The door file's units (task spec `hub-auto`): the stderr line's forms and
/// which outcomes doubt the index; the delegated hold's names in the slow
/// lines, the login as `project` and `id` print.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use specengine_cli::{AutoApplyOutcome, AutoReason, CliError};

    use super::{TOOL, line};
    use crate::start::Project;
    use crate::turn::{Class, Holder, Lines, Seats};

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn said(called: &Result<AutoApplyOutcome, CliError>) -> Option<String> {
        line("auto-mini", "PR-100001", "ann", called)
    }

    fn held(reason: AutoReason, detail: &str, ran: bool) -> AutoApplyOutcome {
        AutoApplyOutcome::Held {
            reason,
            detail: detail.to_owned(),
            ran,
        }
    }

    #[test]
    fn each_outcome_has_its_line() {
        let prefix = "specengine-http: auto auto-mini PR-100001 by ann: ";
        let cases = [
            (
                AutoApplyOutcome::Applied {
                    commit: Some(SHA.to_owned()),
                },
                "applied 0123456789ab",
            ),
            (
                held(AutoReason::ExcludedZone, "in `narrative`", false),
                "held (excluded-zone): in `narrative`",
            ),
            (
                AutoApplyOutcome::Off {
                    reason: AutoReason::Team,
                    why: "`[team]` names 2 logins".to_owned(),
                    absent: false,
                },
                "off (team): `[team]` names 2 logins",
            ),
            (
                AutoApplyOutcome::Off {
                    reason: AutoReason::Off,
                    why: "`mode` is local".to_owned(),
                    absent: false,
                },
                "off (off): `mode` is local",
            ),
            (AutoApplyOutcome::Gone, "gone"),
            (AutoApplyOutcome::Moved, "moved"),
            (
                AutoApplyOutcome::Applied { commit: None },
                "cannot run: applied, but its document names no commit",
            ),
        ];
        for (outcome, tail) in cases {
            assert_eq!(said(&Ok(outcome)), Some(format!("{prefix}{tail}")));
        }
    }

    #[test]
    fn no_table_prints_nothing() {
        let absent = AutoApplyOutcome::Off {
            reason: AutoReason::Off,
            why: "no [auto] table in specengine.toml@HEAD".to_owned(),
            absent: true,
        };
        assert_eq!(said(&Ok(absent)), None);
    }

    #[test]
    fn a_detail_and_a_cannot_run_stay_one_escaped_line() {
        let detail = held(AutoReason::Rebases, "step 5\nrefused\u{1b}[2J", true);
        assert_eq!(
            said(&Ok(detail)).as_deref(),
            Some(
                "specengine-http: auto auto-mini PR-100001 by ann: held (rebases): step 5 \
                 refused\\u{1b}[2J"
            )
        );
        let cannot = CliError::cannot("spec: cannot run git: gone");
        assert_eq!(
            said(&Err(cannot)).as_deref(),
            Some(
                "specengine-http: auto auto-mini PR-100001 by ann: cannot run: spec: cannot run git: gone"
            )
        );
    }

    #[test]
    fn only_a_run_that_may_have_written_doubts() {
        let doubting = [
            held(AutoReason::Rebases, "refused at step 7", true),
            AutoApplyOutcome::Applied { commit: None },
        ];
        for outcome in doubting {
            assert!(outcome.doubts(), "{outcome:?}");
        }
        let keeping = [
            held(AutoReason::ExcludedZone, "in `narrative`", false),
            held(AutoReason::Author, "not ann's", false),
            held(AutoReason::Cap, "20 today", false),
            held(AutoReason::Rebases, "its span changed", false),
            AutoApplyOutcome::Applied {
                commit: Some(SHA.to_owned()),
            },
            AutoApplyOutcome::Off {
                reason: AutoReason::Stopped,
                why: "the stop file".to_owned(),
                absent: false,
            },
            AutoApplyOutcome::Gone,
            AutoApplyOutcome::Moved,
        ];
        for outcome in keeping {
            assert!(!outcome.doubts(), "{outcome:?}");
        }
    }

    #[test]
    fn the_delegated_hold_names_its_login_proposal_and_call() {
        let project = Project {
            slug: "auto-mini".to_owned(),
            root: PathBuf::from("/nowhere"),
            turn: Arc::new(tokio::sync::Mutex::new(())),
            trust: None,
            lines: Lines::default(),
            seats: Seats::default(),
        };
        let named = |login: &str| {
            let holder = Holder::delegated(&project, login, "PR-100001", TOOL);
            let (by, id, tool, class) = holder.named();
            (
                by.map(str::to_owned),
                id.map(str::to_owned),
                tool.map(str::to_owned),
                class,
            )
        };
        assert_eq!(
            named("ann"),
            (
                Some("ann".to_owned()),
                Some("PR-100001".to_owned()),
                Some("auto_apply".to_owned()),
                Class::Mcp
            )
        );
        // A login the line's grammar cannot carry prints as `?`.
        for login in ["Ann Lee", "ann\nx", "ann=1", &"a".repeat(65)] {
            assert_eq!(named(login).0.as_deref(), Some("?"), "{login:?}");
        }
        assert_eq!(named("ann.lee_2-b").0.as_deref(), Some("ann.lee_2-b"));
    }
}
