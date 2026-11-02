//! `…/proposals/<id>/confirm`, a hub's alone (docs/features/hub-confirm.md,
//! `docs/canon/hub-team.md`): a login confirms one staged choice, one route for
//! one proposal, no route for a set. The fence lists the POST in the confirm
//! class ([`crate::hub`]); a local daemon does not mount it, so a local daemon
//! answers its unknown-route 404 whatever is sent.
//!
//! Nothing here says yes. The read asks the library for the question the
//! terminal would ask; the write hands back what the page showed, and the
//! library answers its own question by comparing the two, byte for byte
//! (`specengine_cli`'s `confirm` module). This file and no other names those two
//! calls, and it holds no consent of any kind (`tests/door.rs`).
//!
//! Past the fence, in order, nothing read or stored on a refusal: the served
//! slug (404), for the write a same-origin page (403), the path's ID (400),
//! [`hub::member`] and [`hub::confirms`] (403; a bad `[team]` 503), then for the
//! write JSON (415), at most [`CONFIRM_BODY_MAX`] bytes (413), the closed body
//! (400), and those two again, so a login taken out of `[team]` during a slow
//! body keeps no power. Those reads cost an outsider no body; none of them is
//! the one judged. Inside the project's turn the `[team]` entry is read once
//! more ([`hub::confirming`], 403), and that entry goes to the library, so a
//! login demoted or moved while its request waited for the turn is judged by
//! its new role and zones. Then one call: 200, 403 outside the cover, 404
//! unknown, 409 refused, 503 cannot run (`answer.rs`).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::Uri;
use axum::response::Response;
use specengine_cli::{
    CliError, ConfirmBody, ConfirmOutcome, ConfirmRequest, Env, Globals, Outcome, TeamEntry,
    process_git, team_read, utc_now,
};

use crate::answer::{Refusal, run};
use crate::app::{App, Shared, args, body, json_typed, login_of, same_origin, unknown_route_of};
use crate::args::percent_decode;
use crate::hub::{self, Login};
use crate::start::Project;

/// The most bytes of a confirm's body: it carries the question back, and a
/// record's question shows the whole record, at most 1 MiB, each line end
/// doubled by JSON (as `ROUND_BODY_MAX`).
const CONFIRM_BODY_MAX: usize = 4_194_304;

/// The project and the proposal ID (percent-decoded once, judged by the CLI)
/// of the route at `uri`.
fn target(app: &App, uri: &Uri) -> Result<(Arc<Project>, String), Refusal> {
    let (project, rest) = app.project_of(uri)?;
    Ok((project, proposal_id(&rest, uri)?))
}

/// The ID in `rest`, the route after its slug.
fn proposal_id(rest: &str, uri: &Uri) -> Result<String, Refusal> {
    let raw = rest
        .strip_prefix("proposals/")
        .and_then(|rest| rest.strip_suffix("/confirm"))
        .ok_or_else(|| unknown_route_of(uri))?;
    percent_decode(raw).map_err(Refusal::bad_request)
}

/// The login of a confirm route whose `[team]` entry, read now before the
/// turn, is of a role that confirms. A daemon with no login, which mounts no
/// such route, has none to give.
async fn entered(
    project: &Arc<Project>,
    login: Option<&Login>,
    uri: &Uri,
) -> Result<String, Refusal> {
    let entry = hub::member(project, login).await?;
    hub::confirms(entry.as_ref())?;
    match login {
        Some(Login(login)) if entry.is_some() => Ok(login.clone()),
        _ => Err(unknown_route_of(uri)),
    }
}

/// Inside the project's turn: `login`'s `[team]` entry read again, the one
/// judged, then `call` with the library's request for `id` under it, on the
/// clock read now. A login no longer in `[team]` or no longer of a role that
/// confirms is the 403 ([`ConfirmOutcome::outside`]); a bad `[team]` exit 2,
/// the 503.
fn judged<F>(
    env: &Env,
    globals: &Globals,
    login: &str,
    id: String,
    call: F,
) -> Result<Outcome, CliError>
where
    F: FnOnce(&ConfirmRequest) -> Result<ConfirmOutcome, CliError>,
{
    let team = team_read(env, globals)?;
    let entry: TeamEntry = match hub::confirming(team.as_ref(), login) {
        Ok(entry) => entry,
        Err(line) => return Ok(answered(ConfirmOutcome::outside(&line))),
    };
    let request = ConfirmRequest {
        id,
        entry,
        now: utc_now(),
        git: process_git(env),
    };
    call(&request).map(answered)
}

/// What a confirm answers.
fn answered(outcome: ConfirmOutcome) -> Outcome {
    Outcome::Confirm(Box::new(outcome))
}

/// `GET …/proposals/<id>/confirm`: `{id, question, updated_at}`, judged as the
/// write is. No event, whatever it finds.
pub(crate) async fn read(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let uri = request.uri();
    let (project, id) = target(&app, uri)?;
    args("confirm", &[], uri)?;
    let login = entered(&project, login_of(&request).as_ref(), uri).await?;
    Ok(run(project, move |env, globals| {
        judged(env, globals, &login, id, |request| {
            specengine_cli::confirm_question(env, globals, request)
        })
    })
    .await)
}

/// `POST …/proposals/<id>/confirm`, its body what the read gave: the stage
/// confirmed by the login, one commit or one rejection; the review document.
pub(crate) async fn write(
    State(app): State<Shared>,
    request: Request,
) -> Result<Response, Refusal> {
    let uri = request.uri().clone();
    let (project, rest) = app.project_of(&uri)?;
    same_origin(request.headers())?;
    let id = proposal_id(&rest, &uri)?;
    let login = login_of(&request);
    entered(&project, login.as_ref(), &uri).await?;
    json_typed(request.headers())?;
    let bytes = body(request, CONFIRM_BODY_MAX).await?;
    let sent =
        ConfirmBody::from_json(&bytes).map_err(|error| Refusal::bad_request(error.message))?;
    let login = entered(&project, login.as_ref(), &uri).await?;
    Ok(run(project, move |env, globals| {
        judged(env, globals, &login, id, |request| {
            specengine_cli::confirm_staged(env, globals, request, &sent)
        })
    })
    .await)
}
