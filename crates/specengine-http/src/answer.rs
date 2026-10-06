//! One CLI library call per request, and its response
//! (docs/features/daemon-read.md "Description and interactions", "Data").
//!
//! A call runs on the blocking pool, one at a time per project (each read
//! refreshes the project's index): the request waits for the project's
//! turn in its own task, so a request whose client is gone before its
//! turn is dropped unrun; the turn then goes with the call to the pool.
//! The call: the process's `Env`, the project's root as the globals, the
//! config read again (a slug changed since the start is a 503 naming
//! both), one library function, its `--json` document. Exit 0 → 200 with the document; exit 1 → 404 with the exit-1
//! document (its `reason` set: data to the client); exit 2 → 503 with the
//! error body, `message` the CLI's line(s) verbatim. A body is
//! `application/json; charset=utf-8`, compact: the CLI's bytes without the
//! final line end. The error body is `{"status":<code>,"message":"…"}`,
//! exactly two keys. The store and `rusqlite` are never touched here.

use std::sync::Arc;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use specengine_cli::{CliError, Env, Exit, Globals, Outcome, ProjectRoot, discover, render_json};

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
fn document(outcome: &Outcome) -> String {
    let mut text = render_json(outcome);
    if text.ends_with('\n') {
        text.pop();
    }
    text
}

/// What a call gave on the blocking pool.
enum Answered {
    Document { exit: Exit, body: String },
    CannotRun(String),
}

/// Runs `command` once for `project` (see the module documentation) and
/// answers with its document.
pub(crate) async fn run<F>(project: Arc<Project>, command: F) -> Response
where
    F: FnOnce(&Env, &Globals) -> Result<Outcome, CliError> + Send + 'static,
{
    // Dropped here, with the request, while waiting: the call never runs.
    let turn = Arc::clone(&project.turn).lock_owned().await;
    let joined = tokio::task::spawn_blocking(move || {
        let _turn = turn;
        let answered = Env::from_process().and_then(|env| {
            let globals = project.globals();
            found(&env, &globals, &project)?;
            command(&env, &globals)
        });
        match answered {
            Ok(outcome) => match outcome.exit() {
                Exit::CannotRun => Answered::CannotRun(outcome.stderr_lines().join("\n")),
                exit => Answered::Document {
                    exit,
                    body: document(&outcome),
                },
            },
            Err(error) => Answered::CannotRun(error.message),
        }
    })
    .await;
    match joined {
        Ok(Answered::Document { exit, body }) => {
            let status = if exit == Exit::Answered {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            };
            json(status, body)
        }
        Ok(Answered::CannotRun(message)) => error(StatusCode::SERVICE_UNAVAILABLE, &message),
        Err(_) => internal(),
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
/// served under, else exit 2 naming both.
fn found(env: &Env, globals: &Globals, project: &Project) -> Result<ProjectRoot, CliError> {
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
