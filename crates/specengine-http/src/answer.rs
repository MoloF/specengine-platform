//! One CLI library call per request, and its response
//! (docs/features/daemon-read.md "Description and interactions", "Data";
//! docs/features/ui-live.md "Data").
//!
//! A call runs on the blocking pool, one at a time per project (each read
//! refreshes the project's index; a check and the unapplied amendments,
//! which refresh nothing, take the turn too): the request waits for the
//! project's turn in its own task, so a request whose client is gone
//! before its turn is dropped unrun; the turn then goes with the call to
//! the pool.
//! The call: the process's `Env`, the project's root as the globals, the
//! config read again (a slug changed since the start, or a config the
//! discovery refuses, is a 503), one library function, its `--json`
//! document. Exit 0 → 200 with the document; exit 1 → 404 with the exit-1
//! document (its `reason` set: data to the client); exit 2 → 503 with the
//! error body, `message` the CLI's line(s) verbatim. The check exception:
//! a check's report is a 200 document whatever its verdict's exit, keyed
//! on the outcome being a check (a `CliError` is still a 503); the round's
//! answers alike, a 200 document whatever became of each answer, keyed on
//! the outcome's type (docs/features/ui-round.md "Data"). A stage's
//! answer (`docs/canon/decision-staging.md` "Daemon") is keyed on its
//! cause: none 200, a usage defect 400, unknown 404, refused 409 (a
//! `CliError` is still a 503); a card write's alike
//! (docs/features/board-ui.md "Data"), and a hub's confirm: none 200, unknown
//! 404, refused 409, outside the login's cover 403, the error body
//! (docs/features/hub-confirm.md "Data"). A body is
//! `application/json; charset=utf-8`, compact: the CLI's bytes without the
//! final line end. The error body is `{"status":<code>,"message":"…"}`,
//! exactly two keys. The store and `rusqlite` are never touched here.
//!
//! On a hub the call runs in a CLI scope of the project's `IndexTrust`,
//! holding the turn (the `read-trust` spec, "The daemon"): [`run_read`],
//! a read route's, trusts the index; [`run`], every other route's (a write,
//! an authorization, or a route not classified), walks, so nothing is
//! judged from an index that has not seen a hand edit. A call that panics
//! or cannot run (exit 2) doubts the trust, before the turn is released.
//!
//! The read routes, the one list (`crate::app`): `tree`, `nodes`, `search`,
//! `bundle`, `graph`, `flow` without `trace`, `check`, `health`,
//! `lock-diff`, `roadmap`, `inbox`, `proposals/<id>`, `tasks`, `tasks/<id>`,
//! `cards` (GET) without `marks`, and `cards/<id>` (no marks). Not on it,
//! so walking: trace data wherever it is shown (`trace`, `flow?trace=true`,
//! `cards?marks=true`: the rollups and marks a person accepts a card on,
//! judged on the files as they are now); the round's sheet (it writes a
//! round), the stage, the round's answers, every card write, the trace's
//! clear and a confirm's read and write.

use std::sync::Arc;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use specengine_cli::{
    CardCause, CliError, ConfirmCause, Env, Exit, Globals, IndexTrust, Outcome, ProjectRoot,
    StageCause, discover, render_json, trusted, walking,
};

use crate::start::Project;

/// The content type of every JSON body.
const JSON: &str = "application/json; charset=utf-8";

/// `Cache-Control` of every response.
pub(crate) const NO_STORE: HeaderValue = HeaderValue::from_static("no-store");

/// The error body.
#[derive(Serialize)]
struct ErrorBody<'a> {
    status: u16,
    message: &'a str,
}

/// A JSON body as given.
pub(crate) fn json(status: StatusCode, body: String) -> Response {
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(JSON));
    headers.insert(CACHE_CONTROL, NO_STORE);
    response
}

/// The error body of `status`.
pub(crate) fn error(status: StatusCode, message: &str) -> Response {
    let body = serde_json::to_string(&ErrorBody {
        status: status.as_u16(),
        message,
    })
    .unwrap_or_else(|_| format!("{{\"status\":{},\"message\":\"\"}}", status.as_u16()));
    json(status, body)
}

/// An answer that is no document: its status and message, sent as the
/// error body.
#[derive(Debug)]
pub(crate) struct Refusal {
    status: StatusCode,
    message: String,
}

impl Refusal {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// A 400.
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        error(self.status, &self.message)
    }
}

/// The CLI's `--json` line without its final line end.
pub(crate) fn document(outcome: &Outcome) -> String {
    let mut text = render_json(outcome);
    if text.ends_with('\n') {
        text.pop();
    }
    text
}

/// What a call gave on the blocking pool.
enum Answered {
    Document {
        status: StatusCode,
        body: String,
    },
    /// The error body of a request the CLI refused as a usage defect.
    Refused {
        status: StatusCode,
        message: String,
    },
    CannotRun(String),
}

/// The answer of `outcome`: the exit map (0 → 200, 1 → 404, 2 → the CLI's
/// lines), but a check's report is a 200 document whatever its exit (the
/// check exception, docs/features/ui-live.md "Data": keyed on the outcome
/// being a check, never on its verdict), the round's answers too (keyed on
/// the outcome's type, never on an answer's), and a stage's answer is keyed on
/// its cause (`docs/canon/decision-staging.md` "Daemon"): none 200, a
/// usage defect 400 (the error body, the CLI's line), unknown 404, refused
/// 409 (the refused review document); a card write's alike
/// (docs/features/board-ui.md "Data"): 404 `{id, reason}`, 409 the write
/// document, a lost compare-and-set too, 403 a hub's group clear outside the
/// login's `[team]` cover (docs/features/hub-team.md); a split 200 written or
/// planned, 400 usage, 409 refused; a card's raise 404 no
/// such card, 409 refused, 200 the intake document stored or not; a
/// confirm's by its cause too (docs/features/hub-confirm.md "Data").
fn answered(outcome: &Outcome) -> Answered {
    match outcome {
        Outcome::Stage(stage) => {
            return caused(
                outcome,
                match stage.cause {
                    None => Some(StatusCode::OK),
                    Some(StageCause::Usage) => None,
                    Some(StageCause::Unknown) => Some(StatusCode::NOT_FOUND),
                    Some(StageCause::Refused) => Some(StatusCode::CONFLICT),
                },
            );
        }
        Outcome::Card(card) => return caused(outcome, write_status(card.cause)),
        // A card's raise (`docs/canon/card-questions.md` "The rule"): no such
        // card 404 `{id, reason}`, an intake rule's refusal 409 the intake
        // document, else 200 whether stored or not.
        Outcome::CardRaise(raise) => {
            let status = match (raise.no_card(), raise.exit()) {
                (true, _) => StatusCode::NOT_FOUND,
                (false, Exit::NotFound) => StatusCode::CONFLICT,
                (false, _) => StatusCode::OK,
            };
            return caused(outcome, Some(status));
        }
        Outcome::CardGroup(group) => return caused(outcome, write_status(group.cause)),
        // A split: 200 written or planned, 400 a usage rule, 409 refused
        // with the document; never a 404, the path naming no card
        // (`docs/features/department-split.md`
        // "Acceptance criteria", S7).
        Outcome::CardSplit(split) => return caused(outcome, write_status(split.cause)),
        Outcome::Confirm(confirm) => {
            return match confirm.cause {
                None => caused(outcome, Some(StatusCode::OK)),
                Some(ConfirmCause::Unknown) => caused(outcome, Some(StatusCode::NOT_FOUND)),
                Some(ConfirmCause::Refused) => caused(outcome, Some(StatusCode::CONFLICT)),
                Some(ConfirmCause::Outside) => Answered::Refused {
                    status: StatusCode::FORBIDDEN,
                    message: confirm.proposal.refusal.clone().unwrap_or_default(),
                },
            };
        }
        _ => {}
    }
    let status = match (outcome, outcome.exit()) {
        (Outcome::Check(_) | Outcome::RoundAnswers(_), _) | (_, Exit::Answered) => StatusCode::OK,
        (_, Exit::NotFound) => StatusCode::NOT_FOUND,
        (_, Exit::CannotRun) => return Answered::CannotRun(outcome.stderr_lines().join("\n")),
    };
    Answered::Document {
        status,
        body: document(outcome),
    }
}

/// A card write's status by its cause; `None` a usage defect.
fn write_status(cause: Option<CardCause>) -> Option<StatusCode> {
    match cause {
        None => Some(StatusCode::OK),
        Some(CardCause::Usage) => None,
        Some(CardCause::Unknown) => Some(StatusCode::NOT_FOUND),
        Some(CardCause::Refused) => Some(StatusCode::CONFLICT),
        Some(CardCause::Outside) => Some(StatusCode::FORBIDDEN),
    }
}

/// The answer of a write keyed on its cause: `status` with the document,
/// `None` a usage defect, a 400 with the CLI's line.
fn caused(outcome: &Outcome, status: Option<StatusCode>) -> Answered {
    match status {
        Some(status) => Answered::Document {
            status,
            body: document(outcome),
        },
        None => Answered::Refused {
            status: StatusCode::BAD_REQUEST,
            message: outcome.stderr_lines().join("\n"),
        },
    }
}

/// Runs `command` once for `project` (see the module documentation) and
/// answers with its document; on a hub its freshness steps walk.
pub(crate) async fn run<F>(project: Arc<Project>, command: F) -> Response
where
    F: FnOnce(&Env, &Globals) -> Result<Outcome, CliError> + Send + 'static,
{
    turned(project, Freshness::Walk, command).await
}

/// [`run`] of a read route: on a hub its freshness step may trust the
/// index.
pub(crate) async fn run_read<F>(project: Arc<Project>, command: F) -> Response
where
    F: FnOnce(&Env, &Globals) -> Result<Outcome, CliError> + Send + 'static,
{
    turned(project, Freshness::Trust, command).await
}

/// [`run`] or [`run_read`] as `freshness` says: a read route whose request
/// decides it (trace data walks).
pub(crate) async fn run_as<F>(project: Arc<Project>, freshness: Freshness, command: F) -> Response
where
    F: FnOnce(&Env, &Globals) -> Result<Outcome, CliError> + Send + 'static,
{
    turned(project, freshness, command).await
}

/// What a hub's call does in its freshness steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Freshness {
    /// A read: the CLI's `trusted` scope.
    Trust,
    /// Anything else: the CLI's `walking` scope.
    Walk,
}

async fn turned<F>(project: Arc<Project>, freshness: Freshness, command: F) -> Response
where
    F: FnOnce(&Env, &Globals) -> Result<Outcome, CliError> + Send + 'static,
{
    // Dropped here, with the request, while waiting: the call never runs.
    let turn = Arc::clone(&project.turn).lock_owned().await;
    let joined = tokio::task::spawn_blocking(move || {
        let _turn = turn;
        let trust = project.trust.clone();
        // Dropped before the turn: a panic doubts while it is held.
        let _doubt = DoubtOnPanic(trust.as_deref());
        let called = Env::from_process().and_then(|env| {
            let globals = project.globals();
            found(&env, &globals, &project)?;
            in_scope(trust.as_deref(), freshness, || command(&env, &globals))
        });
        let answer = match called {
            Ok(outcome) => answered(&outcome),
            Err(error) => Answered::CannotRun(error.message),
        };
        if let (Answered::CannotRun(_), Some(trust)) = (&answer, &trust) {
            trust.doubt();
        }
        answer
    })
    .await;
    match joined {
        Ok(Answered::Document { status, body }) => json(status, body),
        Ok(Answered::Refused { status, message }) => error(status, &message),
        Ok(Answered::CannotRun(message)) => error(StatusCode::SERVICE_UNAVAILABLE, &message),
        Err(_) => internal(),
    }
}

/// `call` in the CLI's scope of `trust` as `freshness` asks, when there is
/// one.
pub(crate) fn in_scope<T>(
    trust: Option<&IndexTrust>,
    freshness: Freshness,
    call: impl FnOnce() -> T,
) -> T {
    match (trust, freshness) {
        (Some(trust), Freshness::Trust) => trusted(&[trust], call),
        (Some(trust), Freshness::Walk) => walking(&[trust], call),
        (None, _) => call(),
    }
}

/// Doubts its trust when dropped by a panic's unwind.
pub(crate) struct DoubtOnPanic<'t>(pub Option<&'t IndexTrust>);

impl Drop for DoubtOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(trust) = self.0
        {
            trust.doubt();
        }
    }
}

/// Runs `read` once on the blocking pool for `project`, outside its turn
/// (it refreshes no index), after the slug check, which reads the config
/// once and hands the found project on; exit 2 as its message.
pub(crate) async fn read<T, F>(project: Arc<Project>, read: F) -> Result<T, Refusal>
where
    T: Send + 'static,
    F: FnOnce(&Env, &Globals, &ProjectRoot) -> Result<T, CliError> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(move || {
        Env::from_process().and_then(|env| {
            let globals = project.globals();
            let found = found(&env, &globals, &project)?;
            read(&env, &globals, &found)
        })
    })
    .await;
    match joined {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(cli)) => Err(Refusal::new(StatusCode::SERVICE_UNAVAILABLE, cli.message)),
        Err(_) => Err(Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)),
    }
}

/// The project as its config reads now, which must name the slug it is
/// served under, else exit 2 naming both. A hub's MCP turn judges it too
/// ([`crate::mcp`]).
pub(crate) fn found(
    env: &Env,
    globals: &Globals,
    project: &Project,
) -> Result<ProjectRoot, CliError> {
    let found = discover(env, globals)?;
    let slug = found.slug()?;
    if slug != project.slug {
        return Err(CliError::cannot(format!(
            "specengine-http: {}: the `[project] slug` is now `{slug}`, served as `{}`; \
             restart specengine-http to serve it under the new slug",
            project.root.display(),
            project.slug
        )));
    }
    Ok(found)
}

/// The message of a call that panicked.
const INTERNAL: &str = "internal error: the call failed";

fn internal() -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)
}
