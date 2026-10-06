//! The router (docs/features/daemon-read.md "Data"): the fence around the
//! whole router (it runs before routing, so a route added later is fenced
//! too), the read endpoints, the refused decision, the error bodies of an
//! unknown route (404) and another method on a route (405, its `Allow`
//! what the route serves: `GET` on a read, `POST` on the decision; `HEAD`
//! is no read here), `Cache-Control: no-store` on every response and no
//! `Access-Control-*` header on any.
//!
//! One door (docs/features/daemon-read.md "Rules and edge cases"): no
//! handler here decides, stores, exports, imports, creates or checks; the
//! only writes are the reads' own, in the data directory.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::handler::Handler;
use axum::http::header::{ALLOW, CACHE_CONTROL, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{MethodRouter, get, post};
use specengine_cli::{
    BundleRequest, InboxRequest, Outcome, ReviewRequest, SearchRequest, ShowRequest, TreeRequest,
    View, process_git, project_entry,
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
        .route("/api/projects/{p}/inbox", read(inbox))
        .route("/api/projects/{p}/proposals/{id}", read(proposal))
        .route(
            "/api/projects/{p}/proposals/{id}/decision",
            post(decision).fallback(only_post),
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
            "refused: the Host header must be {} or {}: this server answers its own origin only",
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

/// 405 on the decision route.
async fn only_post(method: Method, uri: Uri) -> Response {
    method_refused(
        &method,
        &uri,
        "this path takes only POST (refused): decisions are made on a terminal",
        Some("POST"),
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
             nodes/<REF>, search, bundle, inbox, proposals/<id>, events}}",
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

/// `POST …/proposals/<id>/decision`: always 403 naming the terminal
/// command; the body is never read, nothing is called.
async fn decision(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {
    let (project, rest) = app.project_of(&uri)?;
    let written = rest
        .strip_prefix("proposals/")
        .and_then(|rest| rest.strip_suffix("/decision"))
        .and_then(|raw| percent_decode(raw).ok())
        .filter(|id| is_proposal_id(id));
    let id = written.as_deref().unwrap_or("PR-…");
    Err(Refusal::new(
        StatusCode::FORBIDDEN,
        format!(
            "decisions are made on a terminal: `spec approve {id}` or `spec reject {id} --reason …` \
             in {}; nothing changed",
            project.root.display()
        ),
    ))
}

/// `PR-` and digits.
fn is_proposal_id(id: &str) -> bool {
    id.strip_prefix("PR-").is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
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

    #[test]
    fn a_proposal_id_is_pr_and_digits() {
        assert!(is_proposal_id("PR-0004"));
        assert!(!is_proposal_id("PR-"));
        assert!(!is_proposal_id("pr-0004"));
        assert!(!is_proposal_id("PR-4a"));
    }
}
