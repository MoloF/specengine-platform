//! The router (docs/features/daemon-read.md "Data"): the fence around the
//! whole router (it runs before routing, so a route added later is fenced
//! too), the read endpoints, the stage of a choice, the error bodies of an
//! unknown route (404) and another method on a route (405, its `Allow`
//! what the route serves: `GET` on a read, `POST, DELETE` on the stage;
//! `HEAD` is no read here), `Cache-Control: no-store` on every response
//! and no `Access-Control-*` header on any.
//!
//! One door (docs/features/daemon-read.md "Rules and edge cases", amended
//! by docs/features/ui-live.md "Data" and
//! `docs/canon/decision-staging.md` "Daemon"): no handler here decides,
//! exports, imports, creates or indexes, and it checks only as the plain
//! run (the working tree on disk:
//! no git mode, no client's file); a task is only listed or shown, never
//! moved (docs/features/ui-live-tasks.md "Description and interactions").
//! The one queue write is a proposal's staged choice, through the CLI
//! library's `stage` and `unstage` (POST and DELETE on the stage route,
//! past the fence only from a same-origin page: `Sec-Fetch-Site:
//! same-origin`, which is not authentication, ADR-0034), confirmed only on
//! a terminal (ADR-0035); the other writes are the reads' own, in the data
//! directory.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::handler::Handler;
use axum::http::header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{MethodRouter, get, post};
use futures_util::StreamExt as _;
use specengine_cli::{
    BundleRequest, CheckRequest, GraphRequest, InboxRequest, Outcome, ReviewRequest, SearchRequest,
    ShowRequest, StageBody, StageRequest, TaskListRequest, TaskShowRequest, TreeRequest,
    UnstageRequest, View, process_git, project_entry, utc_now,
};

use crate::answer::{self, NO_STORE, Refusal, error, json, run};
use crate::args::{Args, percent_decode, project_path};
use crate::start::Project;
use crate::tail;

/// What every handler shares.
#[derive(Debug)]
pub(crate) struct App {
    /// In `--root` order.
    pub projects: Vec<Arc<Project>>,
    /// The bound port, which `Host` and `Origin` must name.
    pub port: u16,
}

pub(crate) type Shared = Arc<App>;

impl App {
    pub(crate) fn new(projects: Vec<Project>, port: u16) -> Self {
        Self {
            projects: projects.into_iter().map(Arc::new).collect(),
            port,
        }
    }

    /// The project the path names (`/api/projects/<slug>/…`): an unknown
    /// slug is a 404, a slug that does not decode a 400.
    pub(crate) fn project_of(&self, uri: &Uri) -> Result<(Arc<Project>, String), Refusal> {
        let Some((raw, rest)) = project_path(uri.path()) else {
            return Err(unknown_route_of(uri));
        };
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
        Ok((Arc::clone(project), rest.to_owned()))
    }
}

/// The service served: every route, wrapped whole in the fence. The
/// routes are the one fallback service of an otherwise empty router whose
/// layer is the fence, so the fence sees every request before the routes'
/// router does (a `layer` on the routes' router itself would wrap each
/// route after routing, and miss a route added after it).
pub(crate) fn router(app: Shared) -> Router {
    let routes = Router::new()
        .route("/api/projects", read(projects))
        .route("/api/projects/{p}/tree", read(tree))
        .route("/api/projects/{p}/nodes/{*ref}", read(nodes))
        .route("/api/projects/{p}/search", read(search))
        .route("/api/projects/{p}/bundle", read(bundle))
        .route("/api/projects/{p}/graph", read(graph))
        .route("/api/projects/{p}/check", read(check))
        .route("/api/projects/{p}/inbox", read(inbox))
        .route("/api/projects/{p}/proposals/{id}", read(proposal))
        .route("/api/projects/{p}/tasks", read(tasks))
        .route("/api/projects/{p}/tasks/{id}", read(task))
        .route(
            "/api/projects/{p}/proposals/{id}/decision",
            post(stage).delete(unstage).fallback(only_stage),
        )
        .route("/api/projects/{p}/events", read(tail::events))
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
/// `same-origin` or `none`. Every response gets `Cache-Control: no-store`.
async fn fence(State(app): State<Shared>, request: Request, next: Next) -> Response {
    let mut response = if let Err(reason) = fenced(app.port, request.headers()) {
        error(StatusCode::FORBIDDEN, &reason)
    } else {
        next.run(request).await
    };
    response.headers_mut().insert(CACHE_CONTROL, NO_STORE);
    response
}

/// Why the request is refused, if it is.
fn fenced(port: u16, headers: &HeaderMap) -> Result<(), String> {
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
    let site = HeaderName::from_static("sec-fetch-site");
    if let Some(site) = single(headers, &site, "Sec-Fetch-Site")?
        && site != b"same-origin"
        && site != b"none"
    {
        return Err(
            "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`".to_owned(),
        );
    }
    Ok(())
}

/// The one value of a header, if sent; sent twice is refused.
fn single<'h>(
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

/// 405 on a route without its own.
async fn method_not_allowed(method: Method, uri: Uri) -> Response {
    method_refused(&method, &uri, "another method is served here", None)
}

/// 404: no route.
async fn unknown_route(uri: Uri) -> Refusal {
    unknown_route_of(&uri)
}

fn unknown_route_of(uri: &Uri) -> Refusal {
    Refusal::new(
        StatusCode::NOT_FOUND,
        format!(
            "no route {}: the routes are /api/projects and /api/projects/<slug>/{{tree, \
             nodes/<REF>, search, bundle, graph, check, inbox, proposals/<id>, tasks, \
             tasks/<id>, events}}",
            uri.path()
        ),
    )
}

/// The endpoint's query (the raw query string of `uri`), decoded and its
/// names checked (400).
fn args(endpoint: &str, names: &[&str], uri: &Uri) -> Result<Args, Refusal> {
    Args::new(endpoint, names, uri.query()).map_err(Refusal::bad_request)
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
    Ok(run(project, move |env, globals| {
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
    let request = (|| {
        Ok::<_, String>(ShowRequest {
            reference,
            links: args.with_links()?,
            archive: args.boolean("archive")?.unwrap_or(false),
        })
    })()
    .map_err(Refusal::bad_request)?;
    Ok(run(project, move |env, globals| {
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
    Ok(run(project, move |env, globals| {
        specengine_cli::search_with_view(env, globals, &request, View::Browser).map(Outcome::Search)
    })
    .await)
}

/// `GET …/bundle`: `spec bundle --json`.
async fn bundle(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    let args = args("bundle", &["node_ids", "budget"], &uri)?;
    let request = (|| {
        Ok::<_, String>(BundleRequest {
            references: args.required_texts("node_ids")?,
            budget: args.integer("budget")?,
        })
    })()
    .map_err(Refusal::bad_request)?;
    Ok(run(project, move |env, globals| {
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
    Ok(run(project, move |env, globals| {
        specengine_cli::graph_with_view(env, globals, &request, View::Browser).map(Outcome::Graph)
    })
    .await)
}

/// `GET …/check`: `spec check --json`, the plain run only (the working
/// tree on disk, the root's config and baseline, today's UTC date); no
/// query name. Every report is a 200 document (`answer.rs`).
async fn check(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    args("check", &[], &uri)?;
    Ok(run(project, |env, globals| {
        specengine_cli::check(env, globals, &CheckRequest::default()).map(Outcome::Check)
    })
    .await)
}

/// `GET …/inbox`: `spec inbox --json` (the root's repository, every
/// worktree).
async fn inbox(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    args("inbox", &[], &uri)?;
    Ok(run(project, |env, globals| {
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
    Ok(run(project, move |env, globals| {
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
    Ok(run(project, move |env, globals| {
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
    Ok(run(project, move |env, globals| {
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

/// The most bytes of a stage's body
/// (`docs/features/decision-staging.md` "Data").
const STAGE_BODY_MAX: usize = 16_384;

/// The project and the proposal ID (percent-decoded once, judged by the
/// CLI) of the stage route `uri`, and the page's origin: only a same-origin
/// page stages (`Sec-Fetch-Site: same-origin`; absent or `none`, which the
/// fence lets a read through, is refused: 403). This is no authentication
/// (ADR-0034): any local process can send the header.
fn stage_target(
    app: &App,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<(Arc<Project>, String), Refusal> {
    // The served slug first (an unknown one is a 404 on every route), then
    // the page's origin, before anything of the proposal is read.
    let (project, rest) = app.project_of(uri)?;
    let site = headers.get(HeaderName::from_static("sec-fetch-site"));
    if site.is_none_or(|site| site.as_bytes() != b"same-origin") {
        return Err(Refusal::new(
            StatusCode::FORBIDDEN,
            "staging needs a same-origin page (not authentication: ADR-0034)",
        ));
    }
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
/// cannot run (`answer.rs`).
async fn stage(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = stage_target(&app, request.uri(), request.headers())?;
    json_typed(request.headers())?;
    let bytes = body(request).await?;
    let body = StageBody::from_json(&bytes).map_err(|error| Refusal::bad_request(error.message))?;
    Ok(run(project, move |env, globals| {
        let request = StageRequest {
            id,
            body,
            now: utc_now(),
            git: process_git(env),
        };
        specengine_cli::stage(env, globals, &request)
            .map(|outcome| Outcome::Stage(Box::new(outcome)))
    })
    .await)
}

/// `DELETE …/proposals/<id>/decision`: the staged choice cleared through
/// the CLI library's `unstage` (nothing staged: the document, no event);
/// the body is never read.
async fn unstage(State(app): State<Shared>, request: Request) -> Result<Response, Refusal> {
    let (project, id) = stage_target(&app, request.uri(), request.headers())?;
    Ok(run(project, move |env, globals| {
        let request = UnstageRequest {
            id,
            now: utc_now(),
            git: process_git(env),
        };
        specengine_cli::unstage(env, globals, &request)
            .map(|outcome| Outcome::Stage(Box::new(outcome)))
    })
    .await)
}

/// The body is JSON: one `Content-Type` whose media type is
/// `application/json` (any parameter after it), else 415.
fn json_typed(headers: &HeaderMap) -> Result<(), Refusal> {
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

/// The request's body, at most [`STAGE_BODY_MAX`] bytes (a longer one, or
/// one declared longer, is a 413 and is not read further).
async fn body(request: Request) -> Result<Vec<u8>, Refusal> {
    let too_large = || {
        Refusal::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("the body is over {STAGE_BODY_MAX} bytes"),
        )
    };
    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .and_then(|length| length.trim().parse::<u64>().ok());
    if declared.is_some_and(|length| length > STAGE_BODY_MAX as u64) {
        return Err(too_large());
    }
    let mut stream = request.into_body().into_data_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| Refusal::bad_request("the body cannot be read"))?;
        if bytes.len() + chunk.len() > STAGE_BODY_MAX {
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
        assert!(fenced(7, &headers(&[("host", "127.0.0.1:7")])).is_ok());
        assert!(
            fenced(
                7,
                &headers(&[("host", "localhost:7"), ("origin", "http://localhost:7")])
            )
            .is_ok()
        );
        assert!(
            fenced(
                7,
                &headers(&[("host", "127.0.0.1:7"), ("sec-fetch-site", "none")])
            )
            .is_ok()
        );
        assert!(fenced(7, &headers(&[])).is_err());
        assert!(fenced(7, &headers(&[("host", "evil.example:7")])).is_err());
        assert!(fenced(7, &headers(&[("host", "127.0.0.1:8")])).is_err());
        assert!(
            fenced(
                7,
                &headers(&[("host", "127.0.0.1:7"), ("origin", "http://evil.example")])
            )
            .is_err()
        );
        assert!(
            fenced(
                7,
                &headers(&[("host", "127.0.0.1:7"), ("sec-fetch-site", "cross-site")])
            )
            .is_err()
        );
        assert!(
            fenced(
                7,
                &headers(&[("host", "127.0.0.1:7"), ("sec-fetch-site", "same-site")])
            )
            .is_err()
        );
        assert!(
            fenced(
                7,
                &headers(&[("host", "127.0.0.1:7"), ("host", "127.0.0.1:7")])
            )
            .is_err()
        );
    }
}
