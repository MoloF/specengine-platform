//! The router (docs/features/daemon-read.md "Data"): the fence around the
//! whole router (it runs before routing, so a route added later is fenced
//! too), the read endpoints (the round's sheet among them), the stage of a
//! choice and of the round's answers, the error bodies of an unknown route
//! (404) and another method on a route (405, its `Allow` what the route
//! serves: `GET` on a read, `POST, DELETE` on the stage, `POST` on the
//! round's answers; `HEAD` is no read here), `Cache-Control: no-store` on
//! every response and no `Access-Control-*` header on any. The web UI's two
//! read routes, `/` and `/assets/<file>`, are fenced like the API's
//! (docs/features/ui-embed.md, [`crate::ui`]). A card's routes
//! (docs/features/board-ui.md "Data"): `…/cards` a read and a write, its 405
//! `Allow: GET, POST`; `…/cards/<id>` a read; its six writes `Allow: POST`.
//! `…/trace` a read, `…/trace/clear` a write (docs/features/card-trace.md).
//! `/api/identity` a read of the request's login, no project, no turn
//! (docs/features/hub-identity.md); a hub has its own fence ([`crate::hub`]),
//! and `…/team`, the asker's own row of a project's `[team]`, and
//! `…/proposals/<id>/confirm` ([`crate::confirm`]) are routes of a hub alone
//! (docs/features/hub-team.md, docs/features/hub-confirm.md): locally they are
//! no routes at all.
//!
//! One door (docs/features/daemon-read.md "Rules and edge cases", amended
//! by docs/features/ui-live.md "Data" and
//! `docs/canon/decision-staging.md` "Daemon"): no handler here decides,
//! exports, imports, creates or indexes, and it checks only as the plain
//! run (the working tree on disk:
//! no git mode, no client's file); a task is only listed or shown, never
//! moved (docs/features/ui-live-tasks.md "Description and interactions").
//! A proposal's queue write is a staged choice, through the CLI library's
//! `stage` and `unstage` (POST and DELETE on the stage route, past the
//! fence only from a same-origin page: `Sec-Fetch-Site: same-origin`, which
//! is not authentication, ADR-0034), and through its `round_stage_answers`
//! (POST on the round's answers, the same door: docs/features/ui-round.md
//! "Data"), confirmed on a terminal (ADR-0035, ADR-0038) or, on a hub alone,
//! by [`crate::confirm`], one item at a time, which takes a stage and never
//! a choice: no handler here walks a round or decides an item. A card's
//! writes are the library's `card_new`, `card_move`, `card_edit`,
//! `card_assign`, `card_cancel`,
//! `card_accept`, `card_clear` and `card_clear_group`, through the same door
//! (docs/features/board-ui.md "Data", docs/features/card-trace.md); they
//! touch the queue alone, a departing accept making its one proposal there
//! inside the library's accept (ADR-0057), and no agent tool reaches them.
//! The other writes are the reads' own, in the data directory.
//!
//! On a hub the stage, the round's answers and the three card acts taking a
//! consent open to the logins a project's `[team]` names, past
//! [`hub::member`], before their body is read and again once it is in, so an
//! outsider costs no body and a slow body outlives no entry; locally that
//! call reads nothing and every handler is as it was
//! (docs/features/hub-team.md).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Request, State};
use axum::handler::Handler;
use axum::http::header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{MethodRouter, any, get, post};
use futures_util::StreamExt as _;
use specengine_cli::{
    BundleRequest, CardAcceptBody, CardAcceptRequest, CardAssignBody, CardCancelBody,
    CardClearBody, CardClearGroupBody, CardClearGroupRequest, CardEditBody, CardListRequest,
    CardMoveBody, CardNewBody, CardOutcome, CardShowRequest, CheckRequest, CliError, FlowRequest,
    GraphRequest, HealthRequest, InboxRequest, IndexTrust, LockDiffRequest, Outcome, ReviewRequest,
    RoadmapRequest, RoundAnswersBody, RoundAnswersRequest, RoundNewRequest, SearchRequest,
    ShowRequest, StageBody, StageRequest, TaskListRequest, TaskShowRequest, TeamRow, TraceRequest,
    TreeRequest, UnstageRequest, View, process_git, project_entry, roadmap_flags, round_limit,
    team_read, utc_now,
};

use crate::answer::{self, Freshness, NO_STORE, Refusal, error, json, run, run_as, run_read};
use crate::args::{Args, percent_decode, project_path};
use crate::hub::{Hub, Login};
use crate::mcp::{self, Mounts};
use crate::start::Project;
use crate::sync::{Push, RepoSync, Syncs};
use crate::{confirm, hub, tail, ui};

/// What every handler shares.
#[derive(Debug)]
pub(crate) struct App {
    /// In `--root` order.
    pub projects: Vec<Arc<Project>>,
    /// The bound port, which `Host` and `Origin` must name.
    pub port: u16,
    /// `--public-origin`: present, a hub, whose fence replaces [`fenced`].
    pub hub: Option<Hub>,
    /// `--push` (a hub's alone): each repository's sync
    /// (docs/features/hub-git.md); absent, nothing fetches or pushes.
    pub syncs: Option<Syncs>,
    /// A hub's `/mcp/<slug>` (docs/features/hub-mcp.md): one MCP service per
    /// project and the session table; absent on a local daemon.
    pub mcp: Option<Mounts>,
    /// A stream's maximum life: `Some(STREAM_LIFE)` on a hub, `None` locally,
    /// where no stream ends for its age ([`hub::lived`]).
    pub life: Option<Duration>,
}

pub(crate) type Shared = Arc<App>;

impl App {
    pub(crate) fn new(
        projects: Vec<Project>,
        port: u16,
        hub: Option<Hub>,
        push: Option<Push>,
    ) -> Self {
        // A hub's reads trust the index its writers keep; a local
        // daemon's walk (the `read-trust` spec, D6): `hub` is the switch.
        let projects: Vec<Arc<Project>> = projects
            .into_iter()
            .map(|mut project| {
                project.trust = hub
                    .as_ref()
                    .map(|_| Arc::new(IndexTrust::new(project.root.clone())));
                Arc::new(project)
            })
            .collect();
        let syncs = push.map(|push| Syncs::new(&projects, push));
        // `--public-origin` is the whole switch (S5): no flag of its own.
        let mcp = hub.as_ref().map(|hub| Mounts::new(&projects, hub));
        // A hub's streams end at their life, a local daemon's never: the
        // switch is `hub` alone, no flag, key or variable
        // (`docs/canon/serve.md` "Live tail").
        let life = hub.as_ref().map(|_| hub::STREAM_LIFE);
        Self {
            projects,
            port,
            hub,
            syncs,
            mcp,
            life,
        }
    }

    /// The sync of `project`'s repository, on a hub with `--push`.
    pub(crate) fn sync_of(&self, project: &Project) -> Option<Arc<RepoSync>> {
        self.syncs
            .as_ref()
            .and_then(|syncs| syncs.of(&project.slug))
            .map(Arc::clone)
    }

    /// The project the path names (`/api/projects/<slug>/…`): an unknown
    /// slug is a 404, a slug that does not decode a 400.
    pub(crate) fn project_of(&self, uri: &Uri) -> Result<(Arc<Project>, String), Refusal> {
        let Some((raw, rest)) = project_path(uri.path()) else {
            return Err(unknown_route_of(uri));
        };
        Ok((self.project_named(raw)?, rest.to_owned()))
    }

    /// The project a raw path segment names, as [`Self::project_of`] judges
    /// it: an unknown slug is a 404, a slug that does not decode a 400.
    pub(crate) fn project_named(&self, raw: &str) -> Result<Arc<Project>, Refusal> {
        let slug = percent_decode(raw).map_err(Refusal::bad_request)?;
        let project = self
            .projects
            .iter()
            .find(|project| project.slug == slug)
            .ok_or_else(|| {
                let served: Vec<&str> = self.projects.iter().map(|p| p.slug.as_str()).collect();
                Refusal::new(
                    StatusCode::NOT_FOUND,
                    format!(
                        "no project `{slug}` is served here: served {}",
                        served.join(", ")
                    ),
                )
            })?;
        Ok(Arc::clone(project))
    }
}

/// The service served: every route, wrapped whole in the fence. The
/// routes are the one fallback service of an otherwise empty router whose
/// layer is the fence, so the fence sees every request before the routes'
/// router does (a `layer` on the routes' router itself would wrap each
/// route after routing, and miss a route added after it).
pub(crate) fn router(app: Shared) -> Router {
    let routes = Router::new()
        .route("/api/identity", read(identity))
        .route("/api/projects", read(projects))
        .route("/api/projects/{p}/tree", read(tree))
        .route("/api/projects/{p}/nodes/{*ref}", read(nodes))
        .route("/api/projects/{p}/search", read(search))
        .route("/api/projects/{p}/bundle", read(bundle))
        .route("/api/projects/{p}/flow", read(flow))
        .route("/api/projects/{p}/trace", read(trace))
        .route(
            "/api/projects/{p}/trace/clear",
            post(clear_group).fallback(only_post),
        )
        .route("/api/projects/{p}/graph", read(graph))
        .route("/api/projects/{p}/check", read(check))
        .route("/api/projects/{p}/health", read(health))
        .route("/api/projects/{p}/roadmap", read(roadmap))
        .route("/api/projects/{p}/lock-diff", read(lock_diff))
        .route("/api/projects/{p}/inbox", read(inbox))
        .route("/api/projects/{p}/proposals/{id}", read(proposal))
        .route("/api/projects/{p}/tasks", read(tasks))
        .route("/api/projects/{p}/tasks/{id}", read(task))
        .route(
            "/api/projects/{p}/cards",
            get(cards)
                .post(new_card)
                .head(only_cards)
                .fallback(only_cards),
        )
        .route("/api/projects/{p}/cards/{id}", read(card))
        .route(
            "/api/projects/{p}/cards/{id}/move",
            post(move_card).fallback(only_post),
        )
        .route(
            "/api/projects/{p}/cards/{id}/edit",
            post(edit_card).fallback(only_post),
        )
        .route(
            "/api/projects/{p}/cards/{id}/assign",
            post(assign_card).fallback(only_post),
        )
        .route(
            "/api/projects/{p}/cards/{id}/cancel",
            post(cancel_card).fallback(only_post),
        )
        .route(
            "/api/projects/{p}/cards/{id}/accept",
            post(accept_card).fallback(only_post),
        )
        .route(
            "/api/projects/{p}/cards/{id}/clear",
            post(clear_card).fallback(only_post),
        )
        .route(
            "/api/projects/{p}/proposals/{id}/decision",
            post(stage).delete(unstage).fallback(only_stage),
        )
        .route("/api/projects/{p}/round", read(round))
        .route(
            "/api/projects/{p}/round/answers",
            post(round_answers).fallback(only_round_answers),
        )
        .route("/api/projects/{p}/events", read(tail::events))
        // The web UI (docs/features/ui-embed.md "Data"): reads like the
        // API's, no config, no turn.
        .route("/", read(ui::page))
        .route("/assets/{file}", read(ui::asset));
    // A hub's alone: a local daemon has no such route, on any method. MCP's
    // every method goes to its handler, past the fence's class `Mcp`
    // (docs/features/hub-mcp.md S3).
    let routes = if app.hub.is_some() {
        routes
            .route("/api/projects/{p}/team", read(team))
            .route(
                "/api/projects/{p}/proposals/{id}/confirm",
                get(confirm::read).post(confirm::write),
            )
            .route("/mcp/{p}", any(mcp::serve))
    } else {
        routes
    };
    // A `--push` hub's alone (docs/features/hub-git.md): elsewhere no route.
    let routes = if app.syncs.is_some() {
        routes.route("/api/projects/{p}/sync", read(sync_status))
    } else {
        routes
    };
    let routes = routes
        .fallback(unknown_route)
        // A route without its own 405 (none today): the error body, axum's
        // `Allow`.
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(Arc::clone(&app));
    Router::new()
        .fallback_service(routes)
        .layer(middleware::from_fn_with_state(app, fence))
}

/// A read route: `handler` on GET; any other method, `HEAD` included, a 405
/// with `Allow: GET`.
fn read<H, T>(handler: H) -> MethodRouter<Shared>
where
    H: Handler<T, Shared>,
    T: 'static,
{
    get(handler).head(only_get).fallback(only_get)
}

/// The fence, before routing (failing: 403, no `Allow`, nothing read):
/// `Host` exactly `127.0.0.1:<port>` or `localhost:<port>`; `Origin`
/// absent or `http://` and one of them; `Sec-Fetch-Site` absent,
/// `same-origin` or `none`, but for the [`landing`]
/// (docs/features/local-landing.md). A hub judges [`Hub::fenced`] instead
/// and hands the handlers its [`Login`]; a local daemon reads no login
/// header. Every response gets `Cache-Control: no-store`.
async fn fence(State(app): State<Shared>, mut request: Request, next: Next) -> Response {
    let passed = match &app.hub {
        Some(hub) => {
            let uri = request.uri();
            hub.fenced(request.method(), uri.path(), uri.query(), request.headers())
                .map(|login| {
                    request.extensions_mut().insert(login);
                })
        }
        None => fenced(
            app.port,
            request.method(),
            request.uri().path(),
            request.headers(),
        ),
    };
    let mut response = if let Err(reason) = passed {
        error(StatusCode::FORBIDDEN, &reason)
    } else {
        next.run(request).await
    };
    response.headers_mut().insert(CACHE_CONTROL, NO_STORE);
    response
}

/// Why the request is refused, if it is: `Host`, then `Origin`, then
/// `Sec-Fetch-Site` with the [`landing`]. `path` is the URI's, never
/// decoded or normalised.
fn fenced(port: u16, method: &Method, path: &str, headers: &HeaderMap) -> Result<(), String> {
    let own = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let host = single(headers, &HOST, "Host")?;
    if !host.is_some_and(|host| own.iter().any(|own| own.as_bytes() == host)) {
        return Err(format!(
            "refused: the Host header must be {} or {}: this server serves its own origin only",
            own[0], own[1]
        ));
    }
    if let Some(origin) = single(headers, &ORIGIN, "Origin")?
        && !own
            .iter()
            .any(|own| origin == format!("http://{own}").as_bytes())
    {
        return Err(format!(
            "refused: the Origin header, when sent, must be http://{} or http://{}",
            own[0], own[1]
        ));
    }
    if let Some(site) = single(headers, &SEC_FETCH_SITE, "Sec-Fetch-Site")?
        && site != b"same-origin"
        && site != b"none"
        && !landing(method, path, headers)?
    {
        return Err(
            "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`".to_owned(),
        );
    }
    Ok(())
}

pub(crate) const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");
const SEC_FETCH_MODE: HeaderName = HeaderName::from_static("sec-fetch-mode");
const SEC_FETCH_DEST: HeaderName = HeaderName::from_static("sec-fetch-dest");

/// The one exception to `Sec-Fetch-Site`, a local daemon's as a hub's: a
/// top-level navigation to `/`, as the login door and a link from a chat or
/// another site arrive. Static bytes, never framed, and `/` still takes the
/// rest of the fence; an asset, a frame, any API route or method never
/// qualifies. Both fences call this one predicate, so they cannot drift
/// (docs/features/local-landing.md D1).
pub(crate) fn landing(method: &Method, path: &str, headers: &HeaderMap) -> Result<bool, String> {
    Ok(method == Method::GET
        && path == "/"
        && single(headers, &SEC_FETCH_MODE, "Sec-Fetch-Mode")? == Some(b"navigate".as_slice())
        && single(headers, &SEC_FETCH_DEST, "Sec-Fetch-Dest")? == Some(b"document".as_slice()))
}

/// The one value of a header, if sent; sent twice is refused.
pub(crate) fn single<'h>(
    headers: &'h HeaderMap,
    name: &HeaderName,
    label: &str,
) -> Result<Option<&'h [u8]>, String> {
    let mut values = headers.get_all(name).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(format!(
            "refused: the {label} header is sent more than once"
        ));
    }
    Ok(first.map(|value| value.as_bytes()))
}

/// A 405 for `method` on `uri`'s route, `why` what the route takes, `allow`
/// its `Allow` (`None`: axum's, from the route's methods).
fn method_refused(method: &Method, uri: &Uri, why: &str, allow: Option<&'static str>) -> Response {
    let mut response = error(
        StatusCode::METHOD_NOT_ALLOWED,
        &format!("method {method} is not served on {}: {why}", uri.path()),
    );
    if let Some(allow) = allow {
        response
            .headers_mut()
            .insert(ALLOW, HeaderValue::from_static(allow));
    }
    response
}

/// 405 on a read route.
async fn only_get(method: Method, uri: Uri) -> Response {
    method_refused(&method, &uri, "only GET is served here", Some("GET"))
}

/// 405 on the stage route.
async fn only_stage(method: Method, uri: Uri) -> Response {
    method_refused(
        &method,
        &uri,
        "this path takes only POST (stage a choice) and DELETE (unstage it); a staged choice is \
         confirmed on a terminal",
        Some("POST, DELETE"),
    )
}

/// 405 on the round's answers.
async fn only_round_answers(method: Method, uri: Uri) -> Response {
    method_refused(
        &method,
        &uri,
        "this path takes only POST (stage each answer of the round); a staged choice is \
         confirmed on a terminal",
        Some("POST"),
    )
}

/// 405 on the board's route, `HEAD` included.
async fn only_cards(method: Method, uri: Uri) -> Response {
    method_refused(
        &method,
        &uri,
        "this path takes only GET (the board) and POST (a new card)",
        Some("GET, POST"),
    )
}

/// 405 on a card's write.
async fn only_post(method: Method, uri: Uri) -> Response {
    method_refused(&method, &uri, "only POST is served here", Some("POST"))
}

/// 405 on a route without its own.
async fn method_not_allowed(method: Method, uri: Uri) -> Response {
    method_refused(&method, &uri, "another method is served here", None)
}

/// 404: no route.
async fn unknown_route(uri: Uri) -> Refusal {
    unknown_route_of(&uri)
}

/// The 404 of `uri`: the API's routes, then the web UI's.
pub(crate) fn unknown_route_of(uri: &Uri) -> Refusal {
    Refusal::new(
        StatusCode::NOT_FOUND,
        format!(
            "no route {}: the routes are /api/projects and /api/projects/<slug>/{{tree, \
             nodes/<REF>, search, bundle, trace, flow, graph, check, health, roadmap, lock-diff, \
             inbox, proposals/<id>, tasks, tasks/<id>, cards, cards/<id>, round, events}}, \
             /api/identity; the web UI: / and /assets/<file>",
            uri.path()
        ),
    )
}

/// The endpoint's query (the raw query string of `uri`), decoded and its
/// names checked (400).
pub(crate) fn args(endpoint: &str, names: &[&str], uri: &Uri) -> Result<Args, Refusal> {
    Args::new(endpoint, names, uri.query()).map_err(Refusal::bad_request)
}

/// `GET /api/identity`: `{"login":"mira"}` on a hub, `{"login":null}` on a
/// local daemon, the UI's one way to learn the mode. No project, no turn.
async fn identity(request: Request) -> Result<Response, Refusal> {
    args("identity", &[], request.uri())?;
    let login = request
        .extensions()
        .get::<Login>()
        .map(|Login(login)| login.as_str());
    let body = serde_json::json!({ "login": login }).to_string();
    Ok(json(StatusCode::OK, body))
}

/// `GET …/team`, a hub's alone: the asker's own row of the `[team]` the root
/// reads now, `{login, name, allowed}`, no turn. Outside `[team]`, or with
/// none: a 200, `name` null, `allowed` empty; a bad `[team]` a 503. It tells a
/// page what to offer; the route's own 403 stays the truth.
async fn team(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let uri = request.uri();
    let (project, _) = app.project_of(uri)?;
    args("team", &[], uri)?;
    let Some(Login(login)) = login_of(&request) else {
        return Err(unknown_route_of(uri));
    };
    let row = answer::read(project, move |env, globals, _| {
        team_read(env, globals).map(|team| TeamRow::of(team.as_ref(), login))
    })
    .await?;
    let body = serde_json::to_string(&row).map_err(|failure| {
        Refusal::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("internal error: cannot encode the row: {failure}"),
        )
    })?;
    Ok(json(StatusCode::OK, body))
}

/// `GET …/sync`, a `--push` hub's alone: `{ahead, behind, state, error, at}`
/// for the project's repository, `ahead` and `behind` from git now, no turn,
/// no query (docs/features/hub-git.md "Data").
async fn sync_status(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    args("sync", &[], &uri)?;
    let (Some(syncs), Some(sync)) = (&app.syncs, app.sync_of(&project)) else {
        return Err(unknown_route_of(&uri));
    };
    let internal = || {
        Refusal::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal error: cannot read the sync",
        )
    };
    let status = sync.status(syncs.deadline()).await.ok_or_else(internal)?;
    let body = serde_json::to_string(&status).map_err(|_| internal())?;
    Ok(json(StatusCode::OK, body))
}

/// `GET /api/projects`: `[{slug, name, root, branch}]`, in `--root` order.
async fn projects(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    args("projects", &[], &uri)?;
    let mut entries = Vec::with_capacity(app.projects.len());
    for project in &app.projects {
        let entry = answer::read(Arc::clone(project), |env, globals, _| {
            project_entry(env, globals, &process_git(env))
        })
        .await?;
        entries.push(entry);
    }
    let body = serde_json::to_string(&entries).map_err(|failure| {
        Refusal::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("internal error: cannot encode the projects: {failure}"),
        )
    })?;
    Ok(json(StatusCode::OK, body))
}

/// `GET …/tree`: `spec tree --json`, the browser view.
async fn tree(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("tree", &["root", "depth", "kinds", "archive"], &uri)?;
    let request = (|| {
        Ok::<_, String>(TreeRequest {
            root: args.text("root")?,
            depth: args.integer("depth")?,
            kinds: args.texts("kinds"),
            archive: args.boolean("archive")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        specengine_cli::tree_with_view(env, globals, &request, View::Browser).map(Outcome::Tree)
    })
    .await)
}

/// `GET …/nodes/<REF>`: `spec show REF --json`, the browser view; `REF`
/// the rest of the path, percent-decoded once.
async fn nodes(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, rest) = app.project_of(&uri)?;
    let raw = rest
        .strip_prefix("nodes/")
        .ok_or_else(|| unknown_route_of(&uri))?;
    let reference = percent_decode(raw).map_err(Refusal::bad_request)?;
    let args = args("nodes", &["with", "archive"], &uri)?;
    let ((links, bindings), archive) =
        (|| Ok::<_, String>((args.with()?, args.boolean("archive")?.unwrap_or(false))))()
            .map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        let request = ShowRequest {
            reference,
            links,
            archive,
            bindings,
            git: process_git(env),
        };
        specengine_cli::show_with_view(env, globals, &request, View::Browser).map(Outcome::Show)
    })
    .await)
}

/// `GET …/search`: `spec search --json`, the browser view.
async fn search(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("search", &["query", "kinds", "limit", "archive"], &uri)?;
    let request = (|| {
        Ok::<_, String>(SearchRequest {
            // One shell word: `spec search "<query>"`.
            terms: vec![args.required_text("query")?],
            kinds: args.texts("kinds"),
            limit: args.integer("limit")?,
            archive: args.boolean("archive")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        specengine_cli::search_with_view(env, globals, &request, View::Browser).map(Outcome::Search)
    })
    .await)
}

/// `GET …/bundle`: `spec bundle --json`.
async fn bundle(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("bundle", &["node_ids", "budget"], &uri)?;
    let (references, budget) =
        (|| Ok::<_, String>((args.required_texts("node_ids")?, args.integer("budget")?)))()
            .map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        let request = BundleRequest {
            references,
            budget,
            git: process_git(env),
        };
        specengine_cli::bundle(env, globals, &request).map(Outcome::Bundle)
    })
    .await)
}

/// `GET …/graph`: `spec graph REF --json`, the browser view (every node
/// and edge); `ref` required, `types` repeated, in order; a link type is
/// never judged here.
async fn graph(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args(
        "graph",
        &["ref", "impact", "types", "depth", "archive"],
        &uri,
    )?;
    let request = (|| {
        Ok::<_, String>(GraphRequest {
            reference: args.required_text("ref")?,
            impact: args.boolean("impact")?.unwrap_or(false),
            types: args.texts("types"),
            depth: args.integer("depth")?,
            archive: args.boolean("archive")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        specengine_cli::graph_with_view(env, globals, &request, View::Browser).map(Outcome::Graph)
    })
    .await)
}

/// `GET …/flow`: `spec flow --json`, the browser view (every node, edge
/// and gap item, nothing cut); `root` naming nothing: the exit-1 document,
/// a 404. A gap row changes no status. `stage` is `--stage` (`-`: the
/// steps with none), once. `trace=true` is `--trace`: a queue that cannot
/// be read is a note, never a failed flow.
async fn flow(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("flow", &["root", "depth", "stage", "trace"], &uri)?;
    let request = (|| {
        Ok::<_, String>(FlowRequest {
            root: args.text("root")?,
            depth: args.integer("depth")?,
            stage: args.text("stage")?,
            trace: args.boolean("trace")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    // `trace` shows the drift a person accepts a card on: it walks.
    let freshness = if request.trace {
        Freshness::Walk
    } else {
        Freshness::Trust
    };
    Ok(run_as(project, freshness, move |env, globals| {
        specengine_cli::flow_with_view(env, globals, &request, View::Browser)
            .map(|outcome| Outcome::Flow(Box::new(outcome)))
    })
    .await)
}

/// `GET …/check`: `spec check --json`, the plain run only (the working
/// tree on disk, the root's config and baseline, today's UTC date); no
/// query name. Every report is a 200 document (`answer.rs`).
async fn check(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    args("check", &[], &uri)?;
    Ok(run_read(project, |env, globals| {
        specengine_cli::check(env, globals, &CheckRequest::default()).map(Outcome::Check)
    })
    .await)
}

/// `GET …/health`: `spec health --json` (task specs `health-left`,
/// `health-code`, `health-drift`), in the project's turn; no query name.
/// Row 3 walks the working tree on disk as the plain check walks it; rows
/// 4-5 read the code index the call refreshes, as `refs` does (the data
/// directory's indexes written, nothing else); row 6 judges over that
/// handle as plain `spec verify` judges, git read-only through the
/// daemon's environment. Any row state is a 200 document; only the CLI's
/// exit 2, row 3's alone (a config that cannot be used, an incomplete
/// walk), is a 503.
async fn health(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    args("health", &[], &uri)?;
    Ok(run_read(project, |env, globals| {
        specengine_cli::health(
            env,
            globals,
            &HealthRequest {
                git: process_git(env),
            },
        )
        .map(Outcome::Health)
    })
    .await)
}

/// `GET …/lock-diff`: `spec lock diff REF --json` (docs/features/lock-diff.md
/// "Route"), in the project's turn; `ref` required, one value. It reads the
/// root's working tree and repository objects, git read-only (`cat-file`,
/// `diff --no-index` over scratch files in the data directory) through the
/// daemon's environment; a REF naming nothing: the exit-1 document, a 404;
/// the CLI's exit 2: a 503. Nothing is cut.
async fn lock_diff(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("lock-diff", &["ref"], &uri)?;
    let reference = args.required_text("ref").map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        let request = LockDiffRequest {
            reference,
            git: process_git(env),
        };
        specengine_cli::lock_diff(env, globals, &request)
            .map(|outcome| Outcome::LockDiff(Box::new(outcome)))
    })
    .await)
}

/// `GET …/roadmap`: `spec roadmap [--release R | --pool] --json`, the
/// browser view (nothing cut: `truncated` false, `omitted` 0), in the
/// project's turn (it refreshes the index; the queue read as it is).
/// `release` with `pool=true`: a 400 with the CLI's line, nothing run; a
/// `release` naming none: the exit-1 document, a 404; no `[roadmap]` or a
/// bad one: the CLI's exit 2, a 503.
async fn roadmap(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("roadmap", &["release", "pool"], &uri)?;
    let release = args.text("release").map_err(Refusal::bad_request)?;
    let pool = args
        .boolean("pool")
        .map_err(Refusal::bad_request)?
        .unwrap_or(false);
    roadmap_flags(release.as_deref(), pool).map_err(|error| Refusal::bad_request(error.message))?;
    Ok(run_read(project, move |env, globals| {
        let request = RoadmapRequest {
            release,
            pool,
            git: process_git(env),
        };
        specengine_cli::roadmap_with_view(env, globals, &request, View::Browser)
            .map(|outcome| Outcome::Roadmap(Box::new(outcome)))
    })
    .await)
}

/// `GET …/inbox`: `spec inbox --json` (the root's repository, every
/// worktree).
async fn inbox(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    args("inbox", &[], &uri)?;
    Ok(run_read(project, |env, globals| {
        let request = InboxRequest {
            all: false,
            git: process_git(env),
        };
        specengine_cli::inbox(env, globals, &request).map(Outcome::Inbox)
    })
    .await)
}

/// `GET …/proposals/<id>`: `spec review PR --json` (not `--brief`), its
/// preview in the proposal's recorded worktree.
async fn proposal(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, rest) = app.project_of(&uri)?;
    let raw = rest
        .strip_prefix("proposals/")
        .ok_or_else(|| unknown_route_of(&uri))?;
    let id = percent_decode(raw).map_err(Refusal::bad_request)?;
    args("proposals", &[], &uri)?;
    Ok(run_read(project, move |env, globals| {
        let request = ReviewRequest {
            id,
            git: process_git(env),
        };
        specengine_cli::review(env, globals, &request)
            .map(|outcome| Outcome::Proposal(Box::new(outcome)))
    })
    .await)
}

/// `GET …/tasks`: `spec task list [--status S]… --json` (the root's
/// repository); `status` repeated, in order, each a task state as the
/// model parses it.
async fn tasks(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("tasks", &["status"], &uri)?;
    let statuses = args.statuses("status").map_err(Refusal::bad_request)?;
    Ok(run_read(project, move |env, globals| {
        let request = TaskListRequest {
            statuses,
            git: process_git(env),
        };
        specengine_cli::task_list(env, globals, &request).map(Outcome::TaskList)
    })
    .await)
}

/// `GET …/tasks/<id>`: `spec task show T --json`, the package uncut (no
/// `--next`, no query name); no such task: the CLI's exit-1 document, a
/// 404.
async fn task(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, rest) = app.project_of(&uri)?;
    let raw = rest
        .strip_prefix("tasks/")
        .ok_or_else(|| unknown_route_of(&uri))?;
    let id = percent_decode(raw).map_err(Refusal::bad_request)?;
    args("tasks/:id", &[], &uri)?;
    Ok(run_read(project, move |env, globals| {
        let request = TaskShowRequest {
            id: Some(id),
            next: false,
            git: process_git(env),
        };
        specengine_cli::task_show(env, globals, &request)
            .map(|outcome| Outcome::TaskShow(Box::new(outcome)))
    })
    .await)
}

/// `GET …/round`: `spec round new [--limit N] --json`, uncut; `limit`, its
/// only query name, judged as the CLI judges `--limit` (1 to 200: a 400
/// with the CLI's line, nothing run), the sheet read in the project's turn
/// (it refreshes the index).
async fn round(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("round", &["limit"], &uri)?;
    let limit = args.integer("limit").map_err(Refusal::bad_request)?;
    round_limit(limit).map_err(|error| Refusal::bad_request(error.message))?;
    Ok(run(project, move |env, globals| {
        let request = RoundNewRequest {
            limit,
            git: process_git(env),
        };
        specengine_cli::round_new(env, globals, &request)
            .map(|sheet| Outcome::RoundSheet(Box::new(sheet)))
    })
    .await)
}

/// The most bytes of a stage's body
/// (`docs/features/decision-staging.md` "Data").
const STAGE_BODY_MAX: usize = 16_384;

/// The most bytes of the round's answers (docs/features/ui-round.md
/// "Data", W-6: 200 answers at their caps, escaped, fit).
const ROUND_BODY_MAX: usize = 4_194_304;

/// The most bytes of a card write's body
/// (docs/features/board-ui.md "Data").
const CARD_BODY_MAX: usize = 1_048_576;

/// Only a same-origin page writes (`Sec-Fetch-Site: same-origin`; absent or
/// `none`, which the fence lets a read through, is refused: 403). This is
/// no authentication (ADR-0034): any local process can send the header.
pub(crate) fn same_origin(headers: &HeaderMap) -> Result<(), Refusal> {
    let site = headers.get(HeaderName::from_static("sec-fetch-site"));
    if site.is_none_or(|site| site.as_bytes() != b"same-origin") {
        return Err(Refusal::new(
            StatusCode::FORBIDDEN,
            "a write needs a same-origin page (not authentication: ADR-0034)",
        ));
    }
    Ok(())
}

/// The project and the proposal ID (percent-decoded once, judged by the
/// CLI) of the stage route `uri`, from a same-origin page only
/// ([`same_origin`]).
fn stage_target(
    app: &App,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<(Arc<Project>, String), Refusal> {
    // The served slug first (an unknown one is a 404 on every route), then
    // the page's origin, before anything of the proposal is read.
    let (project, rest) = app.project_of(uri)?;
    same_origin(headers)?;
    let raw = rest
        .strip_prefix("proposals/")
        .and_then(|rest| rest.strip_suffix("/decision"))
        .ok_or_else(|| unknown_route_of(uri))?;
    let id = percent_decode(raw).map_err(Refusal::bad_request)?;
    Ok((project, id))
}

/// `POST …/proposals/<id>/decision`: the owner's choice staged through the
/// CLI library's `stage`, its body the CLI's `StageBody` (JSON only: 415
/// otherwise; at most [`STAGE_BODY_MAX`] bytes: 413; a body that does not
/// decode: 400 with the CLI's line); 200 the review document, 400 a usage
/// defect, 404 unknown, 409 refused (the refused review document), 503
/// cannot run (`answer.rs`). On a hub a `[team]` login alone, past the
/// page's origin and again past the body ([`hub::member`]), named `as` in the
/// event.
async fn stage(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = stage_target(&app, request.uri(), request.headers())?;
    let login = login_of(&request);
    hub::member(&project, login.as_ref()).await?;
    json_typed(request.headers())?;
    let bytes = body(request, STAGE_BODY_MAX).await?;
    let body = StageBody::from_json(&bytes).map_err(|error| Refusal::bad_request(error.message))?;
    hub::member(&project, login.as_ref()).await?;
    Ok(run(project, move |env, globals| {
        let request = StageRequest {
            id,
            body,
            now: utc_now(),
            git: process_git(env),
            actor: login.map(|Login(login)| login),
        };
        specengine_cli::stage(env, globals, &request)
            .map(|outcome| Outcome::Stage(Box::new(outcome)))
    })
    .await)
}

/// `DELETE …/proposals/<id>/decision`: the staged choice cleared through
/// the CLI library's `unstage` (nothing staged: the document, no event);
/// the body is never read. On a hub as [`stage`].
async fn unstage(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = stage_target(&app, request.uri(), request.headers())?;
    let login = login_of(&request);
    hub::member(&project, login.as_ref()).await?;
    Ok(run(project, move |env, globals| {
        let request = UnstageRequest {
            id,
            now: utc_now(),
            git: process_git(env),
            actor: login.map(|Login(login)| login),
        };
        specengine_cli::unstage(env, globals, &request)
            .map(|outcome| Outcome::Stage(Box::new(outcome)))
    })
    .await)
}

/// `POST …/round/answers`: the Questions screen's answers staged through
/// the CLI library's `round_stage_answers`, past the stage route's door, in
/// order, nothing read or stored on a refusal: the served slug (404), a
/// same-origin page (403), JSON (415), at most [`ROUND_BODY_MAX`] bytes
/// (413), a body the CLI decodes (`RoundAnswersBody`: for this project, at
/// most 200 answers; else a 400 with its line). Then one call in the
/// project's turn, one clock reading: 200 with the answers document
/// whatever became of each answer (`answer.rs`), 503 cannot run. An answer
/// only stages; nothing is decided here. On a hub as [`stage`].
async fn round_answers(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(request.uri())?;
    same_origin(request.headers())?;
    let login = login_of(&request);
    hub::member(&project, login.as_ref()).await?;
    json_typed(request.headers())?;
    let bytes = body(request, ROUND_BODY_MAX).await?;
    let answers = RoundAnswersBody::from_json(&bytes, &project.slug)
        .map_err(|error| Refusal::bad_request(error.message))?;
    hub::member(&project, login.as_ref()).await?;
    Ok(run(project, move |env, globals| {
        let request = RoundAnswersRequest {
            body: answers,
            now: utc_now(),
            git: process_git(env),
            actor: login.map(|Login(login)| login),
        };
        specengine_cli::round_stage_answers(env, globals, &request)
            .map(|outcome| Outcome::RoundAnswers(Box::new(outcome)))
    })
    .await)
}

/// `GET …/trace?ref=&stage=&summary=`: `spec trace [REF] [--stage S]
/// [--summary] --json`, the browser view (every node kept, `truncated`
/// false). A read; it changes nothing but the index.
async fn trace(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("trace", &["ref", "stage", "summary"], &uri)?;
    let request = (|| {
        Ok::<_, String>(TraceRequest {
            reference: args.text("ref")?,
            stage: args.text("stage")?,
            summary: args.boolean("summary")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    Ok(run(project, move |env, globals| {
        specengine_cli::trace_with_view(env, globals, &request, View::Browser)
            .map(|outcome| Outcome::Trace(Box::new(outcome)))
    })
    .await)
}

/// `GET …/cards`: `spec card list --json`, the browser view (every row, in
/// the flows' order, nothing cut). Filters ANDed, exact; `with_cancelled`
/// is the flag `--with-cancelled`, `marks` `--marks`, nothing here reading a
/// card's state. No index to read marks from: a 503, never a clean row.
async fn cards(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args(
        "cards",
        &[
            "flow",
            "column",
            "discipline",
            "assignee",
            "no_target",
            "with_cancelled",
            "marks",
        ],
        &uri,
    )?;
    let request = (|| {
        Ok::<_, String>(CardListRequest {
            flow: args.text("flow")?,
            column: args.text("column")?,
            discipline: args.text("discipline")?,
            assignee: args.text("assignee")?,
            no_target: args.boolean("no_target")?.unwrap_or(false),
            with_cancelled: args.boolean("with_cancelled")?.unwrap_or(false),
            marks: args.boolean("marks")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    // `marks` show the drift a person accepts a card on: it walks.
    let freshness = if request.marks {
        Freshness::Walk
    } else {
        Freshness::Trust
    };
    Ok(run_as(project, freshness, move |env, globals| {
        specengine_cli::card_list(env, globals, &request)
            .map(|outcome| Outcome::CardList(Box::new(outcome.in_view(View::Browser))))
    })
    .await)
}

/// `GET …/cards/<id>`: `spec card show C --history --json`, uncut (no query
/// name); no such card: the CLI's exit-1 document, a 404.
async fn card(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, rest) = app.project_of(&uri)?;
    let raw = rest
        .strip_prefix("cards/")
        .ok_or_else(|| unknown_route_of(&uri))?;
    let id = percent_decode(raw).map_err(Refusal::bad_request)?;
    args("cards/:id", &[], &uri)?;
    Ok(run_read(project, move |env, globals| {
        let request = CardShowRequest {
            id,
            history: true,
            marks: false,
        };
        specengine_cli::card_show(env, globals, &request)
            .map(|outcome| Outcome::CardShow(Box::new(outcome)))
    })
    .await)
}

/// The project and the card ID (percent-decoded once, judged by the CLI) of
/// the card write `verb` at `uri`, from a same-origin page only
/// ([`same_origin`]), as [`stage_target`].
fn written_target(
    app: &App,
    uri: &Uri,
    headers: &HeaderMap,
    verb: &str,
) -> Result<(Arc<Project>, String), Refusal> {
    let (project, rest) = app.project_of(uri)?;
    same_origin(headers)?;
    let raw = rest
        .strip_prefix("cards/")
        .and_then(|rest| rest.strip_suffix(verb))
        .ok_or_else(|| unknown_route_of(uri))?;
    let id = percent_decode(raw).map_err(Refusal::bad_request)?;
    Ok((project, id))
}

/// A card write's body past the door's last three steps, in order: JSON
/// (415), at most [`CARD_BODY_MAX`] bytes (413), what `decode`, the CLI's
/// `Card*Body`, takes (400 with its line: a shape, or a person off the
/// card's grammar, which is someone's typing and no failure to run).
async fn written_body<T>(
    request: Request,
    decode: impl FnOnce(&[u8]) -> Result<T, CliError>,
) -> Result<T, Refusal> {
    json_typed(request.headers())?;
    let bytes = body(request, CARD_BODY_MAX).await?;
    decode(&bytes).map_err(|error| Refusal::bad_request(error.message))
}

/// What a card write answers.
fn written(outcome: CardOutcome) -> Outcome {
    Outcome::Card(Box::new(outcome))
}

/// The login the fence set on `request`: a hub's, never a local daemon's.
pub(crate) fn login_of(request: &Request) -> Option<Login> {
    request.extensions().get::<Login>().cloned()
}

/// `POST …/cards`: `card_new`, past `round_answers`' door in order, nothing
/// read on a refusal: the served slug (404), a same-origin page (403),
/// [`written_body`], on a hub the login as `as` ([`hub::acting`], 403). Then
/// one call in the project's turn, one clock reading: 200 the write
/// document, 400 usage, 404 `{id, reason}`, 409 refused (the write
/// document), 503 cannot run (`answer.rs`).
async fn new_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(request.uri())?;
    same_origin(request.headers())?;
    let login = login_of(&request);
    let mut body = written_body(request, CardNewBody::from_json).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    Ok(run(project, move |env, globals| {
        specengine_cli::card_new(env, globals, &body.request(utc_now())).map(written)
    })
    .await)
}

/// `POST …/cards/<id>/move`: `card_move`, past [`new_card`]'s door; a
/// lost compare-and-set, a move outside the flow, an `as` outside `by`: 409.
async fn move_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = written_target(&app, request.uri(), request.headers(), "/move")?;
    let login = login_of(&request);
    let mut body = written_body(request, CardMoveBody::from_json).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    Ok(run(project, move |env, globals| {
        specengine_cli::card_move(env, globals, &body.request(id, utc_now())).map(written)
    })
    .await)
}

/// `POST …/cards/<id>/edit`: `card_edit`, past [`new_card`]'s door.
async fn edit_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = written_target(&app, request.uri(), request.headers(), "/edit")?;
    let login = login_of(&request);
    let mut body = written_body(request, CardEditBody::from_json).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    Ok(run(project, move |env, globals| {
        specengine_cli::card_edit(env, globals, &body.request(id, utc_now())).map(written)
    })
    .await)
}

/// `POST …/cards/<id>/assign`: `card_assign`, past [`new_card`]'s door.
async fn assign_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = written_target(&app, request.uri(), request.headers(), "/assign")?;
    let login = login_of(&request);
    let mut body = written_body(request, CardAssignBody::from_json).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    Ok(run(project, move |env, globals| {
        specengine_cli::card_assign(env, globals, &body.request(id, utc_now())).map(written)
    })
    .await)
}

/// `POST …/cards/<id>/cancel`: `card_cancel`, past [`new_card`]'s door.
async fn cancel_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = written_target(&app, request.uri(), request.headers(), "/cancel")?;
    let login = login_of(&request);
    let mut body = written_body(request, CardCancelBody::from_json).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    Ok(run(project, move |env, globals| {
        specengine_cli::card_cancel(env, globals, &body.request(id, utc_now())).map(written)
    })
    .await)
}

/// `POST …/cards/<id>/accept`: `card_accept`, past [`new_card`]'s door. Its
/// consent is the page's dialog, so the handler answers yes; the rules are
/// the CLI's (`accept.from`, the card's reviewer, never its assignee). The
/// name is self-declared, and no terminal asks here: a convention until
/// the hub's named login (ADR-0034). A departing accept makes its one
/// proposal inside that call (ADR-0057); no `propose` name is called here.
/// On a hub, past [`hub::member`], judged again once the body is in: the
/// login as `as` before `request()`, which would default it to `local`, and a
/// required `revision`, so a dialog answers for the card it showed; a
/// departing accept's proposal names the login.
async fn accept_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = written_target(&app, request.uri(), request.headers(), "/accept")?;
    let login = login_of(&request);
    hub::member(&project, login.as_ref()).await?;
    let mut body = written_body(request, CardAcceptBody::from_json).await?;
    hub::member(&project, login.as_ref()).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    hub::needs(login.as_ref(), "revision", body.revision.is_some())?;
    Ok(run(project, move |env, globals| {
        let request = CardAcceptRequest {
            login: login.map(|Login(login)| login),
            ..body.request(id, utc_now())
        };
        specengine_cli::card_accept(env, globals, &request, &mut |_| true).map(written)
    })
    .await)
}

/// `POST …/cards/<id>/clear`: `card_clear`, past [`new_card`]'s door. Its
/// consent is the page's dialog, the name typed as an accept's: a convention
/// until the hub's named login (ADR-0034). No agent tool reaches it. On a hub
/// as [`accept_card`].
async fn clear_card(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = written_target(&app, request.uri(), request.headers(), "/clear")?;
    let login = login_of(&request);
    hub::member(&project, login.as_ref()).await?;
    let mut body = written_body(request, CardClearBody::from_json).await?;
    hub::member(&project, login.as_ref()).await?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    hub::needs(login.as_ref(), "revision", body.revision.is_some())?;
    Ok(run(project, move |env, globals| {
        specengine_cli::card_clear(env, globals, &body.request(id, utc_now()), &mut |_| true)
            .map(written)
    })
    .await)
}

/// `POST …/trace/clear`: `card_clear_group`, past [`new_card`]'s door: one
/// transaction, one event per card, no effect only. On a hub as
/// [`accept_card`], a `member` refused before the body and after it
/// ([`hub::clears_groups`]) and a required `count`; the library judges the
/// entry read last, its cover, and the count with the set known, before its
/// consent.
async fn clear_group(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(request.uri())?;
    same_origin(request.headers())?;
    let login = login_of(&request);
    let entry = hub::member(&project, login.as_ref()).await?;
    hub::clears_groups(entry.as_ref())?;
    let mut body = written_body(request, CardClearGroupBody::from_json).await?;
    let entry = hub::member(&project, login.as_ref()).await?;
    hub::clears_groups(entry.as_ref())?;
    hub::acting(login.as_ref(), &mut body.actor)?;
    hub::needs(login.as_ref(), "count", body.count.is_some())?;
    Ok(run(project, move |env, globals| {
        let request = CardClearGroupRequest {
            entry,
            ..body.request(utc_now())
        };
        specengine_cli::card_clear_group(env, globals, &request, &mut |_| true)
            .map(|outcome| Outcome::CardGroup(Box::new(outcome)))
    })
    .await)
}

/// The body is JSON: one `Content-Type` whose media type is
/// `application/json` (any parameter after it), else 415.
pub(crate) fn json_typed(headers: &HeaderMap) -> Result<(), Refusal> {
    let mut types = headers.get_all(CONTENT_TYPE).iter();
    let json = match (types.next(), types.next()) {
        (Some(value), None) => value.to_str().is_ok_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        }),
        _ => false,
    };
    if json {
        return Ok(());
    }
    Err(Refusal::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "the body is JSON: send it with Content-Type: application/json",
    ))
}

/// The request's body, at most `max` bytes (a longer one, or one declared
/// longer, is a 413 and is not read further).
pub(crate) async fn body(request: Request, max: usize) -> Result<Vec<u8>, Refusal> {
    let too_large = || {
        Refusal::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("the body is over {max} bytes"),
        )
    };
    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .and_then(|length| length.trim().parse::<u64>().ok());
    if declared.is_some_and(|length| length > max as u64) {
        return Err(too_large());
    }
    let mut stream = request.into_body().into_data_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| Refusal::bad_request("the body cannot be read"))?;
        if bytes.len() + chunk.len() > max {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                HeaderName::from_static(name),
                HeaderValue::from_static(value),
            );
        }
        map
    }

    #[test]
    fn the_fence_takes_only_its_own_origin() {
        assert!(fenced(7, &Method::GET, "/", &headers(&[("host", "127.0.0.1:7")])).is_ok());
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "localhost:7"), ("origin", "http://localhost:7")])
            )
            .is_ok()
        );
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "127.0.0.1:7"), ("sec-fetch-site", "none")])
            )
            .is_ok()
        );
        assert!(fenced(7, &Method::GET, "/", &headers(&[])).is_err());
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "evil.example:7")])
            )
            .is_err()
        );
        assert!(fenced(7, &Method::GET, "/", &headers(&[("host", "127.0.0.1:8")])).is_err());
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "127.0.0.1:7"), ("origin", "http://evil.example")])
            )
            .is_err()
        );
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "127.0.0.1:7"), ("sec-fetch-site", "cross-site")])
            )
            .is_err()
        );
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "127.0.0.1:7"), ("sec-fetch-site", "same-site")])
            )
            .is_err()
        );
        assert!(
            fenced(
                7,
                &Method::GET,
                "/",
                &headers(&[("host", "127.0.0.1:7"), ("host", "127.0.0.1:7")])
            )
            .is_err()
        );
    }

    /// SL2 of docs/features/stream-life.md: the choice of life. `STREAM_LIFE`
    /// is 300 s; a hub's `App` has it, and so do its mounts' GET streams; a
    /// local daemon's has none and no mounts. M: `None` on a hub; `Some`
    /// locally; 60 s.
    #[tokio::test]
    async fn sl2_a_hub_takes_the_stream_life_and_a_local_daemon_none() {
        assert_eq!(hub::STREAM_LIFE, Duration::from_secs(300));
        let origin = Hub::parse("https://hub.example.test").expect("an origin");
        let on_hub = App::new(vec![], 0, Some(origin), None);
        assert_eq!(on_hub.life, Some(hub::STREAM_LIFE), "a hub's life");
        let mounts = on_hub.mcp.as_ref().expect("a hub's mounts");
        assert_eq!(mounts.life(), hub::STREAM_LIFE, "its mounts' life");
        let local = App::new(vec![], 0, None, None);
        assert_eq!(local.life, None, "a local daemon's life");
        assert!(local.mcp.is_none(), "{:?}", local.mcp);
    }
}
