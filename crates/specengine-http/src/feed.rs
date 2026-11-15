//! `GET /api/projects/<p>/feed` (task spec `auto-feed`, "The route"): `spec
//! feed --json`'s body for the same state, locally and on a hub, any login
//! on a hub, in the project's turn as every read. Its query names are the
//! flags': `since` (a seq or a time), `needs_me` (`1` alone), `task`,
//! `department`, each at most once; another name, a second value or a bad
//! one is a 400 with the error body, nothing run. On a hub the `auto` head
//! is judged for the hub's surface, locally for a terminal's.

use axum::extract::State;
use axum::http::Uri;
use axum::response::Response;
use specengine_cli::{FeedRequest, Outcome, feed_since, feed_task, process_git};

use crate::answer::{Refusal, run_read};
use crate::app::{Shared, args};

/// The route's handler (see the module documentation).
pub(crate) async fn feed(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("feed", &["since", "needs_me", "task", "department"], &uri)?;
    let since = args.text("since").map_err(Refusal::bad_request)?;
    let needs_me = match args
        .text("needs_me")
        .map_err(Refusal::bad_request)?
        .as_deref()
    {
        None => false,
        Some("1") => true,
        Some(value) => {
            return Err(Refusal::bad_request(format!(
                "`needs_me={value}`: it takes `1` alone"
            )));
        }
    };
    let task = args.text("task").map_err(Refusal::bad_request)?;
    let department = args.text("department").map_err(Refusal::bad_request)?;
    // Judged as the CLI judges the flags, before anything runs.
    if let Some(since) = &since {
        feed_since(since).map_err(|error| Refusal::bad_request(error.message))?;
    }
    if let Some(task) = &task {
        feed_task(task).map_err(|error| Refusal::bad_request(error.message))?;
    }
    let hub = app.hub.is_some();
    Ok(run_read(project, move |env, globals| {
        let request = FeedRequest {
            since,
            needs_me,
            task,
            department,
            hub,
            git: process_git(env),
        };
        specengine_cli::feed(env, globals, &request).map(|outcome| Outcome::Feed(Box::new(outcome)))
    })
    .await)
}
