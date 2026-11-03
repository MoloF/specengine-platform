//! A hub's MCP (docs/features/hub-mcp.md, ADR-0065): `/mcp/<slug>`, rmcp's
//! Streamable HTTP service around `specengine-mcp`'s hub server, one per
//! served project, mounted on a hub alone ([`crate::app::router`]). This is
//! the one module of the daemon naming `specengine_mcp` or rmcp; it calls
//! no CLI writer itself: every tool's call is the hub server's, run here
//! inside the project's turn ([`Seam`]).
//!
//! Past the fence (class `Mcp`, [`crate::hub`]), [`serve`] finds the project
//! as [`crate::app::App::project_of`] judges a slug, judges the session
//! binding, and hands the request to that project's service:
//!
//! - **One id.** A request naming more than one `Mcp-Session-Id` value:
//!   rmcp's own 404 bytes, rmcp not called, nothing unbound.
//! - **Judge.** A request naming an id not bound to its own slug and login
//!   (unknown, ended, another login's, another project's): the same 404, on
//!   every method, rmcp not called.
//! - **Unknown tools.** A `tools/call` naming no tool of [`HUB_TOOLS`]:
//!   rmcp's own unknown-tool error (`-32602` `tool not found`, the request's
//!   id), rmcp not called, so its per-name schema cache never takes the name.
//! - **Bind.** rmcp answering a POST that named no `Mcp-Session-Id` with a
//!   2xx carrying one: that id is bound to the request's slug and login. An
//!   `initialize` is answered and bound in a task of its own ([`settled`]), so
//!   a request dropped meanwhile leaves no session unbound: the task closes it.
//! - **Caps.** At most [`MCP_SESSIONS_PER_LOGIN`] per login per project, the
//!   least recently used of that login there closed by a fifth; at most
//!   [`MCP_SESSIONS_MAX`] per hub, past which the new session is closed and
//!   the answer is a 503 with no `Mcp-Session-Id`. Only sessions rmcp still
//!   holds count (`has_session`, asked before each count).
//! - **Life.** Memory only: a restart, rmcp's idle end
//!   (`SessionConfig.keep_alive`), a DELETE by its own login or an eviction
//!   ends one. A session id is never printed, stored or answered but in
//!   rmcp's own header; it only refuses, and is no login, author or actor.
//!
//! The stateless era (2026-07-28) opens no session: nothing is bound.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::{Body, Bytes, HttpBody as _};
use axum::extract::{Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::{StreamExt as _, stream};
use rmcp::ErrorData;
use rmcp::model::{
    ClientJsonRpcMessage, ClientRequest, Extensions, RequestId, ServerJsonRpcMessage,
};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{
    SessionId, SessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde::Deserialize;
use serde_json::Value;
use specengine_cli::Env;
use specengine_mcp::{Command, HUB_TOOLS, HubSeam, SpecEngineServer, TurnFuture};
use tokio::sync::{Mutex, oneshot};

use crate::answer::{self, error};
use crate::app::{Shared, unknown_route_of};
use crate::hub::{Hub, Login};
use crate::start::Project;

/// Sessions one login may hold in one project (S1).
pub(crate) const MCP_SESSIONS_PER_LOGIN: usize = 4;

/// Sessions a hub holds in all (S1).
pub(crate) const MCP_SESSIONS_MAX: usize = 64;

/// rmcp's session header.
const SESSION_ID: HeaderName = HeaderName::from_static("mcp-session-id");

/// rmcp's body for an unknown session (`tower.rs`), sent byte for byte.
const SESSION_NOT_FOUND: &str = "Not Found: Session not found";

/// The fence's line for a request with no login, which the fence makes
/// unreachable here.
const NO_LOGIN: &str = "refused: this hub needs one Remote-User header from its proxy";

/// rmcp's message for a tool its router does not hold
/// (`handler/server/router/tool.rs`).
const TOOL_NOT_FOUND: &str = "tool not found";

/// The answer of an open whose task failed (never expected).
const OPEN_FAILED: &str = "internal error: the MCP request failed";

/// The daemon's half of the hub server for one project: the login a
/// request's fence judged, and the project's turn.
struct Seam {
    project: Arc<Project>,
}

impl HubSeam for Seam {
    /// The fence's [`Login`], from the request's parts rmcp puts in each MCP
    /// message's extensions.
    fn login(&self, extensions: &Extensions) -> Option<String> {
        let parts = extensions.get::<Parts>()?;
        parts
            .extensions
            .get::<Login>()
            .map(|Login(login)| login.clone())
    }

    /// As [`answer::run`]: the project's turn awaited by the request's own
    /// task (dropped while waiting, the command never runs), then the
    /// blocking pool with the turn, the process's `Env`, the project's
    /// globals, the slug found again, the one call.
    fn turn(&self, command: Command) -> TurnFuture {
        let project = Arc::clone(&self.project);
        Box::pin(async move {
            let turn = Arc::clone(&project.turn).lock_owned().await;
            // A panic in the call leaves its answer unset: the hub server
            // answers it as an internal error.
            let _ = tokio::task::spawn_blocking(move || {
                let _turn = turn;
                let ready = Env::from_process().and_then(|env| {
                    let globals = project.globals();
                    answer::found(&env, &globals, &project)?;
                    Ok((env, globals))
                });
                match ready {
                    Ok((env, globals)) => command.run(&env, &globals),
                    Err(cannot) => command.cannot_run(cannot),
                }
            })
            .await;
        })
    }
}

/// One project's service and the session manager the daemon keeps a handle
/// to.
struct Mount {
    project: Arc<Project>,
    manager: Arc<LocalSessionManager>,
    service: StreamableHttpService<SpecEngineServer, LocalSessionManager>,
}

/// A bound session (memory only).
struct Bound {
    slug: String,
    login: String,
    /// The table's clock at its last judged request: the eviction's order.
    last_used: u64,
}

/// The session table: id to its binding, and a clock that only counts.
#[derive(Default)]
struct Table {
    bound: HashMap<String, Bound>,
    clock: u64,
}

impl Table {
    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }
}

/// A hub's MCP: one mount per served project, and the hub's session table,
/// shared with the tasks that open sessions ([`settled`]).
pub(crate) struct Mounts {
    inner: Arc<Inner>,
}

/// What [`Mounts`] holds.
struct Inner {
    mounts: Vec<Mount>,
    table: Mutex<Table>,
    /// Calls of rmcp's `handle` (H19's counting seam): test builds alone.
    #[cfg(test)]
    handled: AtomicUsize,
}

impl fmt::Debug for Mounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slugs: Vec<&str> = self
            .inner
            .mounts
            .iter()
            .map(|mount| mount.project.slug.as_str())
            .collect();
        f.debug_struct("Mounts").field("projects", &slugs).finish()
    }
}

impl Mounts {
    /// The mounts of a hub serving `projects` at `hub`'s origin, rmcp's idle
    /// end its default (300 s).
    pub(crate) fn new(projects: &[Arc<Project>], hub: &Hub) -> Self {
        Self::with_idle(projects, hub, None)
    }

    /// As [`Self::new`], a session ending after `idle` without a request
    /// when given (H17's unit test).
    pub(crate) fn with_idle(projects: &[Arc<Project>], hub: &Hub, idle: Option<Duration>) -> Self {
        // rmcp's defaults but the two allow-lists (docs/features/hub-mcp.md
        // "Data"): legacy sessions on, SSE responses, keep-alive 15 s, retry
        // 3 s, a 4 MiB body, no session store, no stateless metadata
        // required; `Host` the hub's authority alone (the default is
        // loopback), `Origin` its origin alone with its port explicit, since
        // rmcp matches a portless entry to any port: as the fence judges
        // both. One token for every mount: the daemon's shutdown.
        let config = StreamableHttpServerConfig::default()
            .with_allowed_hosts([hub.authority()])
            .with_allowed_origins([hub.origin_with_port()]);
        let mounts = projects
            .iter()
            .map(|project| {
                let mut manager = LocalSessionManager::default();
                if let Some(idle) = idle {
                    manager.session_config.keep_alive = Some(idle);
                }
                let manager = Arc::new(manager);
                let seam: Arc<dyn HubSeam> = Arc::new(Seam {
                    project: Arc::clone(project),
                });
                let service = StreamableHttpService::new(
                    move || Ok(SpecEngineServer::hub(Arc::clone(&seam))),
                    Arc::clone(&manager),
                    config.clone(),
                );
                Mount {
                    project: Arc::clone(project),
                    manager,
                    service,
                }
            })
            .collect();
        Self {
            inner: Arc::new(Inner {
                mounts,
                table: Mutex::new(Table::default()),
                #[cfg(test)]
                handled: AtomicUsize::new(0),
            }),
        }
    }

    /// Ends every session and stream at the daemon's shutdown.
    pub(crate) fn shutdown(&self) {
        for mount in &self.inner.mounts {
            mount.service.config.cancellation_token.cancel();
        }
    }

    /// The answer to `request` on project `slug` as `login`, in order: one
    /// `Mcp-Session-Id` at most, bound to `slug` and `login`; a POST's body
    /// read up to rmcp's cap, an unknown tool answered here; an `initialize`
    /// answered and bound in its own task; else rmcp's answer, a DELETE's
    /// session unbound.
    pub(crate) async fn answer(&self, slug: &str, login: &str, request: Request) -> Response {
        let inner = &self.inner;
        let Some(at) = inner
            .mounts
            .iter()
            .position(|mount| mount.project.slug == slug)
        else {
            return unknown_route_of(request.uri()).into_response();
        };
        let mount = &inner.mounts[at];
        let mut values = request.headers().get_all(&SESSION_ID).iter();
        let named = match (values.next(), values.next()) {
            (None, _) => None,
            (Some(value), None) => match value.to_str() {
                Ok(id) => Some(id.to_owned()),
                Err(_) => return session_not_found(),
            },
            // rmcp reads the first value alone: two would part what it ends
            // from what the table unbinds.
            (Some(_), Some(_)) => return session_not_found(),
        };
        if let Some(id) = &named
            && !inner.judged(slug, login, id).await
        {
            return session_not_found();
        }
        let method = request.method().clone();
        let request = if method == Method::POST {
            let (parts, body) = request.into_parts();
            let cap = mount.service.config.max_request_body_bytes;
            let (body, read) = buffered(body, cap).await;
            match read.as_deref().map(sent).unwrap_or(Sent::Other) {
                Sent::UnknownTool(Some(id)) => return unknown_tool(id, named.is_some()),
                Sent::UnknownTool(None) => return accepted(),
                Sent::Initialize if named.is_none() => {
                    let request = Request::from_parts(parts, body);
                    return settled(inner, at, login, request).await;
                }
                Sent::Initialize | Sent::Other => {}
            }
            Request::from_parts(parts, body)
        } else {
            request
        };
        let response = inner.handle(mount, request).await;
        if !response.status().is_success() {
            return response;
        }
        if method == Method::DELETE
            && let Some(id) = &named
        {
            inner.unbind(id).await;
        }
        // rmcp opens a session for an `initialize` alone, bound above in its
        // own task; any other id it issues is bound here all the same.
        if method == Method::POST && named.is_none() {
            return inner.bound(mount, login, response).await;
        }
        response
    }
}

/// Answers an `initialize` (rmcp's answer, then the bind or the 503) in a
/// task of its own, which the handler awaits: dropped with its request, the
/// handler stops waiting, and the task, finding nobody to answer, unbinds and
/// closes the session it opened, so none outlives its request uncounted.
async fn settled(inner: &Arc<Inner>, at: usize, login: &str, request: Request) -> Response {
    let (sent, answered) = oneshot::channel();
    let inner = Arc::clone(inner);
    let login = login.to_owned();
    tokio::spawn(async move {
        let mount = &inner.mounts[at];
        let response = inner.handle(mount, request).await;
        let response = inner.bound(mount, &login, response).await;
        if let Err(response) = sent.send(response)
            && let Some(id) = issued(&response)
        {
            inner.unbind(&id).await;
            let _ = mount.manager.close_session(&session_id(&id)).await;
        }
    });
    answered
        .await
        .unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, OPEN_FAILED))
}

impl Inner {
    fn mount(&self, slug: &str) -> Option<&Mount> {
        self.mounts.iter().find(|mount| mount.project.slug == slug)
    }

    /// rmcp's answer to `request` on `mount`.
    async fn handle(&self, mount: &Mount, request: Request) -> Response {
        #[cfg(test)]
        self.handled.fetch_add(1, Ordering::SeqCst);
        mount.service.handle(request).await.map(Body::new)
    }

    /// `response`, rmcp's to a POST that named no session: the session it
    /// opens bound to `mount`'s slug and `login`, else the 503 of a full hub.
    async fn bound(&self, mount: &Mount, login: &str, response: Response) -> Response {
        if !response.status().is_success() {
            return response;
        }
        let Some(id) = issued(&response) else {
            return response;
        };
        if self.bind(mount, id, login).await {
            return response;
        }
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            &format!(
                "refused: this hub holds {MCP_SESSIONS_MAX} MCP sessions, its cap: try again \
                 once one ends"
            ),
        )
    }

    /// `id` leaves the table.
    async fn unbind(&self, id: &str) {
        self.table.lock().await.bound.remove(id);
    }

    /// `id` is bound to `slug` and `login`; it is then used now.
    async fn judged(&self, slug: &str, login: &str, id: &str) -> bool {
        let mut table = self.table.lock().await;
        let now = table.tick();
        match table.bound.get_mut(id) {
            Some(bound) if bound.slug == slug && bound.login == login => {
                bound.last_used = now;
                true
            }
            _ => false,
        }
    }

    /// Binds `id`, just opened on `mount` by `login`, under the caps (S1,
    /// S2); `false` when the hub was full, the new session closed.
    async fn bind(&self, mount: &Mount, id: String, login: &str) -> bool {
        let slug = mount.project.slug.as_str();
        let mut table = self.table.lock().await;
        let now = table.tick();
        table.bound.insert(
            id.clone(),
            Bound {
                slug: slug.to_owned(),
                login: login.to_owned(),
                last_used: now,
            },
        );
        self.held(&mut table).await;
        // A login's fifth in a project ends its least recently used there,
        // never the new one, which is the newest.
        loop {
            let mut own: Vec<(u64, &String)> = table
                .bound
                .iter()
                .filter(|(_, bound)| bound.slug == slug && bound.login == login)
                .map(|(id, bound)| (bound.last_used, id))
                .collect();
            if own.len() <= MCP_SESSIONS_PER_LOGIN {
                break;
            }
            own.sort();
            let oldest = own[0].1.clone();
            table.bound.remove(&oldest);
            let _ = mount.manager.close_session(&session_id(&oldest)).await;
        }
        // Past the hub's cap the new session goes; no other login's does.
        if table.bound.len() > MCP_SESSIONS_MAX {
            table.bound.remove(&id);
            let _ = mount.manager.close_session(&session_id(&id)).await;
            return false;
        }
        true
    }

    /// Keeps in `table` only the sessions rmcp still holds.
    async fn held(&self, table: &mut Table) {
        let mut gone = Vec::new();
        for (id, bound) in &table.bound {
            let held = match self.mount(&bound.slug) {
                Some(mount) => mount
                    .manager
                    .has_session(&session_id(id))
                    .await
                    .unwrap_or(false),
                None => false,
            };
            if !held {
                gone.push(id.clone());
            }
        }
        for id in gone {
            table.bound.remove(&id);
        }
    }
}

/// rmcp's id of `id`.
fn session_id(id: &str) -> SessionId {
    Arc::from(id)
}

/// The session id rmcp issued in `response`.
fn issued(response: &Response) -> Option<String> {
    response
        .headers()
        .get(&SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// The fields of a message that rmcp's header check reads (`tower.rs`
/// `validate_standard_headers`, which looks a tool's schema up by name for
/// any request or notification whose `method` is `tools/call`), borrowed
/// from the body; every other field is skipped unread.
#[derive(Deserialize)]
struct Peek<'a> {
    #[serde(default, borrow)]
    method: Option<Cow<'a, str>>,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default, borrow)]
    params: Option<PeekParams<'a>>,
}

/// The one field of `params` the peek reads.
#[derive(Deserialize)]
struct PeekParams<'a> {
    #[serde(default, borrow)]
    name: Option<Cow<'a, str>>,
}

/// What a POST's body is, as the daemon judges it before rmcp.
enum Sent {
    /// `tools/call` naming no tool of [`HUB_TOOLS`]: a request's id, `None`
    /// for a notification (no id, or none rmcp could read as one).
    UnknownTool(Option<RequestId>),
    /// Its `method` is `initialize`.
    Initialize,
    /// Anything else, rmcp's to answer.
    Other,
}

/// `read`, judged on the raw JSON as rmcp's header check reads it, whatever
/// the message's kind: a request rmcp would type as a custom one and a
/// notification included. A body the peek cannot read (`params` or a name
/// of another type, a duplicated key) is read as rmcp reads it, typed and
/// then as JSON; one rmcp cannot read either is rmcp's to refuse.
fn sent(read: &[u8]) -> Sent {
    let (method, name, id) = match serde_json::from_slice::<Peek<'_>>(read) {
        Ok(peek) => (
            peek.method.map(Cow::into_owned),
            peek.params
                .and_then(|params| params.name)
                .map(Cow::into_owned),
            peek.id
                .and_then(|id| serde_json::from_value::<RequestId>(id).ok()),
        ),
        Err(_) => {
            let Ok(message) = serde_json::from_slice::<ClientJsonRpcMessage>(read) else {
                return Sent::Other;
            };
            let id = match &message {
                ClientJsonRpcMessage::Request(request) => Some(request.id.clone()),
                ClientJsonRpcMessage::Notification(_) => None,
                _ => return Sent::Other,
            };
            let Ok(value) = serde_json::to_value(&message) else {
                return Sent::Other;
            };
            let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_owned);
            (
                text(value.get("method")),
                text(value.get("params").and_then(|params| params.get("name"))),
                id,
            )
        }
    };
    match (method.as_deref(), name) {
        (Some("tools/call"), Some(name)) if !HUB_TOOLS.contains(&name.as_str()) => {
            Sent::UnknownTool(id)
        }
        (Some("initialize"), _) if initialize(read) => Sent::Initialize,
        _ => Sent::Other,
    }
}

/// `read` is an `initialize` request as rmcp types it: the one message that
/// opens a session.
fn initialize(read: &[u8]) -> bool {
    matches!(
        serde_json::from_slice::<ClientJsonRpcMessage>(read),
        Ok(ClientJsonRpcMessage::Request(request))
            if matches!(request.request, ClientRequest::InitializeRequest(_))
    )
}

/// rmcp's answer to a notification it accepts: 202, no body.
fn accepted() -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::ACCEPTED;
    response
}

/// A POST's body read up to `cap` bytes, rmcp's own cap, and the body to
/// hand rmcp: all of it, also returned, when it fits; else what was read
/// followed by the rest unread, or by the read's error, so rmcp answers its
/// own 413 or error as if nothing had read it.
async fn buffered(body: Body, cap: usize) -> (Body, Option<Bytes>) {
    let hint = body.size_hint();
    let expected = usize::try_from(hint.exact().unwrap_or(hint.lower())).unwrap_or(cap);
    let mut chunks = body.into_data_stream();
    let mut read: Vec<u8> = Vec::with_capacity(expected.min(cap));
    while let Some(chunk) = chunks.next().await {
        match chunk {
            Ok(chunk) => {
                read.extend_from_slice(&chunk);
                if read.len() > cap {
                    let head = stream::once(async move { Ok(Bytes::from(read)) });
                    return (Body::from_stream(head.chain(chunks)), None);
                }
            }
            Err(failed) => {
                let head = stream::iter([Ok(Bytes::from(read)), Err(failed)]);
                return (Body::from_stream(head), None);
            }
        }
    }
    let read = Bytes::from(read);
    (Body::from(read.clone()), Some(read))
}

/// rmcp's answer to a `tools/call` naming a tool its router does not hold:
/// the JSON-RPC error `-32602` `tool not found` with the request's `id`, as
/// JSON; HTTP 200 in a session (legacy keeps 200 for JSON-RPC errors), 400
/// for a stateless request (rmcp's per-request mapping of invalid params).
fn unknown_tool(id: RequestId, in_session: bool) -> Response {
    let message =
        ServerJsonRpcMessage::error(ErrorData::invalid_params(TOOL_NOT_FOUND, None), Some(id));
    let body = serde_json::to_vec(&message).unwrap_or_default();
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = if in_session {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

/// rmcp's own 404 for an unknown session, byte for byte: its status and
/// body, no content type.
fn session_not_found() -> Response {
    let mut response = Response::new(Body::from(SESSION_NOT_FOUND));
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

/// `/mcp/<slug>` on a hub, every method the fence passes (POST, GET,
/// DELETE): the project as [`crate::app::App::project_of`] judges a slug
/// (unknown: its 404), then [`Mounts::answer`] as the fence's login.
pub(crate) async fn serve(State(app): State<Shared>, request: Request) -> Response {
    let Some(mounts) = app.mcp.as_ref() else {
        return unknown_route_of(request.uri()).into_response();
    };
    let raw = request
        .uri()
        .path()
        .strip_prefix("/mcp/")
        .unwrap_or_default();
    let project = match app.project_named(raw) {
        Ok(project) => project,
        Err(refusal) => return refusal.into_response(),
    };
    let Some(Login(login)) = request.extensions().get::<Login>().cloned() else {
        return error(StatusCode::FORBIDDEN, NO_LOGIN);
    };
    mounts.answer(&project.slug, &login, request).await
}

/// The test-only seams of H18 and H19 (docs/features/hub-mcp.md
/// "Acceptance criteria"): no production build holds them. Allowed unused,
/// since a test build without those unit tests still compiles them.
#[cfg(test)]
#[allow(dead_code)]
impl Mounts {
    /// Calls of rmcp's `handle` so far, on every mount (H19).
    pub(crate) fn handled(&self) -> usize {
        self.inner.handled.load(Ordering::SeqCst)
    }

    /// The session ids rmcp's manager holds for `slug`, then those the
    /// daemon has bound there, each sorted (H18).
    pub(crate) async fn sessions(&self, slug: &str) -> (Vec<String>, Vec<String>) {
        let Some(mount) = self.inner.mount(slug) else {
            return (Vec::new(), Vec::new());
        };
        let mut held: Vec<String> = mount
            .manager
            .sessions
            .read()
            .await
            .keys()
            .map(|id| id.to_string())
            .collect();
        held.sort();
        let mut bound: Vec<String> = self
            .inner
            .table
            .lock()
            .await
            .bound
            .iter()
            .filter(|(_, bound)| bound.slug == slug)
            .map(|(id, _)| id.clone())
            .collect();
        bound.sort();
        (held, bound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLUG: &str = "lantern-keep";
    const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"unit","version":"0"}}}"#;
    const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    const LIST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;

    /// The idle end of this test: short, yet past the time the hub takes to
    /// fill in a debug build.
    const IDLE: Duration = Duration::from_secs(3);

    /// A POST as the fence hands it on: the hub's `Host`, JSON in, JSON or SSE
    /// out, `session` named when given.
    fn post(session: Option<&str>, body: &str) -> Request {
        let mut builder = axum::http::Request::builder()
            .method(Method::POST)
            .uri(format!("/mcp/{SLUG}"))
            .header("host", "hub.example.test")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream");
        if let Some(id) = session {
            builder = builder
                .header("mcp-session-id", id)
                .header("mcp-protocol-version", "2025-11-25");
        }
        builder
            .body(Body::from(body.to_owned()))
            .expect("a request")
    }

    /// A session of `login`, initialized; `None` when the hub answered no
    /// 200.
    async fn open(mounts: &Mounts, login: &str) -> Option<String> {
        let response = mounts.answer(SLUG, login, post(None, INITIALIZE)).await;
        if response.status() != StatusCode::OK {
            return None;
        }
        let id = response
            .headers()
            .get(&SESSION_ID)?
            .to_str()
            .ok()?
            .to_owned();
        let initialized = mounts
            .answer(SLUG, login, post(Some(&id), INITIALIZED))
            .await;
        assert_eq!(initialized.status(), StatusCode::ACCEPTED, "{login} {id}");
        Some(id)
    }

    /// H17 of docs/features/hub-mcp.md, the unit: with sessions ending after
    /// [`IDLE`] without a request, a full hub refuses a 65th; once idle, each
    /// of its 64 answers rmcp's 404 bytes, and their places are free: 64 new
    /// sessions fit. M: counting the sessions rmcp has ended (`held` skipped).
    #[tokio::test]
    async fn an_idle_session_answers_404_and_frees_its_place() {
        let project = Arc::new(Project {
            slug: SLUG.to_owned(),
            root: std::env::temp_dir(),
            turn: Arc::new(Mutex::new(())),
        });
        let hub = Hub::parse("https://hub.example.test").expect("an origin");
        let mounts = Mounts::with_idle(&[project], &hub, Some(IDLE));
        let started = std::time::Instant::now();
        let mut opened = Vec::new();
        for n in 0..MCP_SESSIONS_MAX {
            let login = format!("l{:02}", n / MCP_SESSIONS_PER_LOGIN);
            let id = open(&mounts, &login).await.expect("a place");
            opened.push((login, id));
        }
        let refused = mounts.answer(SLUG, "late", post(None, INITIALIZE)).await;
        assert!(
            started.elapsed() < IDLE,
            "filling took {:?}, past the idle end",
            started.elapsed()
        );
        assert_eq!(
            refused.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "the 65th"
        );
        assert!(refused.headers().get(&SESSION_ID).is_none());

        tokio::time::sleep(IDLE + Duration::from_millis(1500)).await;
        for (login, id) in &opened {
            let response = mounts.answer(SLUG, login, post(Some(id), LIST)).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{login} {id}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("a body");
            assert_eq!(&body[..], SESSION_NOT_FOUND.as_bytes(), "{login} {id}");
        }
        for n in 0..MCP_SESSIONS_MAX {
            let login = format!("m{:02}", n / MCP_SESSIONS_PER_LOGIN);
            assert!(open(&mounts, &login).await.is_some(), "a freed place: {n}");
        }
    }

    /// A hub serving `lantern-keep` alone, rmcp's idle end its default.
    fn hub_mounts() -> Mounts {
        let project = Arc::new(Project {
            slug: SLUG.to_owned(),
            root: std::env::temp_dir(),
            turn: Arc::new(Mutex::new(())),
        });
        let hub = Hub::parse("https://hub.example.test").expect("an origin");
        Mounts::with_idle(&[project], &hub, None)
    }

    /// `method` as the fence hands it on, naming each of `ids`.
    fn naming(method: Method, ids: &[&str], body: &str) -> Request {
        let mut builder = axum::http::Request::builder()
            .method(method)
            .uri(format!("/mcp/{SLUG}"))
            .header("host", "hub.example.test")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25");
        for id in ids {
            builder = builder.header("mcp-session-id", *id);
        }
        builder
            .body(Body::from(body.to_owned()))
            .expect("a request")
    }

    /// A response's body, read whole.
    async fn body_of(response: Response) -> Vec<u8> {
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a body")
            .to_vec()
    }

    /// H18 of docs/features/hub-mcp.md, the unit: two sessions of `mira`; a
    /// POST, a GET and a DELETE, each naming two ids (A and B, then A and A),
    /// are each rmcp's unknown-session 404 with rmcp never called, and after
    /// every step the sessions rmcp holds are the bound ones, A and B: none
    /// lives uncounted. M: unbinding every named value and handing the request
    /// to rmcp.
    #[tokio::test]
    async fn two_ids_in_one_request_leave_rmcps_sessions_the_bound_ones() {
        let mounts = hub_mounts();
        let a = open(&mounts, "mira").await.expect("A");
        let b = open(&mounts, "mira").await.expect("B");
        let mut both = vec![a.clone(), b.clone()];
        both.sort();
        assert_eq!(mounts.sessions(SLUG).await, (both.clone(), both.clone()));
        let handled = mounts.handled();
        for ids in [[a.as_str(), b.as_str()], [a.as_str(), a.as_str()]] {
            for (method, body) in [
                (Method::POST, LIST),
                (Method::GET, ""),
                (Method::DELETE, ""),
            ] {
                let response = mounts
                    .answer(SLUG, "mira", naming(method.clone(), &ids, body))
                    .await;
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {ids:?}");
                assert_eq!(
                    body_of(response).await,
                    SESSION_NOT_FOUND.as_bytes(),
                    "{method} {ids:?}"
                );
                assert_eq!(
                    mounts.sessions(SLUG).await,
                    (both.clone(), both.clone()),
                    "after {method} {ids:?}"
                );
            }
        }
        assert_eq!(mounts.handled(), handled, "rmcp never called");
        for id in [&a, &b] {
            let listed = mounts.answer(SLUG, "mira", post(Some(id), LIST)).await;
            assert_eq!(listed.status(), StatusCode::OK, "{id}");
        }
        assert_eq!(mounts.sessions(SLUG).await, (both.clone(), both));
    }

    /// A stateless (2026-07-28) `tools/call` of `name`, request `id`, with its
    /// `_meta` and SEP-2243's headers.
    fn stateless_call(id: usize, name: &str) -> Request {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{{}},"_meta":{{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{{}}}}}}}}"#
        );
        axum::http::Request::builder()
            .method(Method::POST)
            .uri(format!("/mcp/{SLUG}"))
            .header("host", "hub.example.test")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28")
            .header("mcp-method", "tools/call")
            .header("mcp-name", name)
            .body(Body::from(body))
            .expect("a request")
    }

    /// H19 of docs/features/hub-mcp.md, the unit: `claim_task` and `x-0` to
    /// `x-999`, in a session and statelessly, are each rmcp's unknown-tool error
    /// (`-32602` `tool not found`, the request's `id`; HTTP 200 in a session,
    /// 400 stateless) and the counting seam on rmcp's `handle` counts none of
    /// them, so rmcp's schema cache never sees a name; a known call is counted.
    /// M: handing the request to rmcp.
    #[tokio::test]
    async fn an_unknown_tool_name_never_reaches_rmcp() {
        let mounts = hub_mounts();
        let session = open(&mounts, "mira").await.expect("a session");
        let names: Vec<String> = std::iter::once("claim_task".to_owned())
            .chain((0..1_000).map(|n| format!("x-{n}")))
            .collect();
        let handled = mounts.handled();
        for (n, name) in names.iter().enumerate() {
            let id = n + 10;
            let body = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{{}}}}}}"#
            );
            for (era, request, status) in [
                ("legacy", post(Some(&session), &body), StatusCode::OK),
                (
                    "stateless",
                    stateless_call(id, name),
                    StatusCode::BAD_REQUEST,
                ),
            ] {
                let response = mounts.answer(SLUG, "mira", request).await;
                assert_eq!(response.status(), status, "{era} {name}");
                let answer: serde_json::Value =
                    serde_json::from_slice(&body_of(response).await).expect("JSON");
                assert_eq!(answer["id"], serde_json::json!(id), "{era} {name}");
                assert_eq!(
                    answer["error"]["code"],
                    serde_json::json!(-32602),
                    "{era} {name}: {answer}"
                );
                assert_eq!(
                    answer["error"]["message"],
                    serde_json::json!("tool not found"),
                    "{era} {name}: {answer}"
                );
            }
        }
        assert_eq!(mounts.handled(), handled, "rmcp handled an unknown name");
        let listed = mounts
            .answer(SLUG, "mira", post(Some(&session), LIST))
            .await;
        assert_eq!(listed.status(), StatusCode::OK);
        assert_eq!(
            mounts.handled(),
            handled + 1,
            "the seam counts a known call"
        );
    }

    /// A POST of `body` as a 2026-07-28 client sends it, no session:
    /// `MCP-Protocol-Version`, and SEP-2243's `Mcp-Method` and `Mcp-Name`
    /// when `name` is given.
    fn modern(body: &str, name: Option<&str>) -> Request {
        let mut builder = axum::http::Request::builder()
            .method(Method::POST)
            .uri(format!("/mcp/{SLUG}"))
            .header("host", "hub.example.test")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28");
        if let Some(name) = name {
            builder = builder
                .header("mcp-method", "tools/call")
                .header("mcp-name", name);
        }
        builder
            .body(Body::from(body.to_owned()))
            .expect("a request")
    }

    /// H19 of docs/features/hub-mcp.md, iteration 3: the shapes a typed
    /// filter misses, each with `MCP-Protocol-Version: 2026-07-28`, with and
    /// without SEP-2243's headers, never reach rmcp's `handle` (so its schema
    /// cache never takes the name): (a) a request whose `arguments` is no
    /// object, which parses as a custom request, is the unknown-tool error
    /// with its `id`; (b) the same with no `id`, a notification, and a
    /// well-formed notification naming an unknown tool, are a 202. A known
    /// call is counted. M: filtering only a typed `CallToolRequest`.
    #[tokio::test]
    async fn an_unknown_name_in_an_untyped_shape_never_reaches_rmcp() {
        let mounts = hub_mounts();
        let handled = mounts.handled();
        for name in [None, Some("x-1")] {
            let malformed = mounts
                .answer(
                    SLUG,
                    "mira",
                    modern(
                        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x-1","arguments":5}}"#,
                        name,
                    ),
                )
                .await;
            let status = malformed.status();
            assert!(
                status == StatusCode::BAD_REQUEST || status == StatusCode::OK,
                "(a) {name:?}: {status}"
            );
            let answer: serde_json::Value =
                serde_json::from_slice(&body_of(malformed).await).expect("JSON");
            assert_eq!(answer["id"], serde_json::json!(1), "(a) {name:?}: {answer}");
            assert_eq!(
                answer["error"]["code"],
                serde_json::json!(-32602),
                "(a) {name:?}: {answer}"
            );
            assert_eq!(
                answer["error"]["message"],
                serde_json::json!("tool not found"),
                "(a) {name:?}: {answer}"
            );
            for body in [
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"x-1","arguments":5}}"#,
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"x-1","arguments":{}}}"#,
            ] {
                let notified = mounts.answer(SLUG, "mira", modern(body, name)).await;
                assert_eq!(
                    notified.status(),
                    StatusCode::ACCEPTED,
                    "(b) {name:?} {body}"
                );
            }
            assert_eq!(mounts.handled(), handled, "{name:?}: rmcp handled a shape");
        }
        let session = open(&mounts, "mira").await.expect("a session");
        let counted = mounts.handled();
        let listed = mounts
            .answer(SLUG, "mira", post(Some(&session), LIST))
            .await;
        assert_eq!(listed.status(), StatusCode::OK);
        assert_eq!(
            mounts.handled(),
            counted + 1,
            "the seam counts a known call"
        );
    }
}
