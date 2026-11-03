//! An MCP client over Streamable HTTP for a hub's `/mcp/<slug>`
//! (docs/features/hub-mcp.md "Acceptance criteria"): JSON-RPC over POST
//! through the test proxy ([`proxied`]: the public `Host`, one `Remote-User`,
//! Traefik's `X-Forwarded-*`, no `Sec-Fetch-*` and no `Origin`, which no MCP
//! client sends), the answer read as SSE frames or as JSON, in both protocol
//! eras:
//!
//! - **legacy** ([`Session`]): `initialize` (2025-11-25) opens a session, its
//!   `Mcp-Session-Id` named on every later request with
//!   `MCP-Protocol-Version: 2025-11-25`; a GET is its SSE stream, a DELETE its
//!   end;
//! - **stateless** ([`Stateless`], 2026-07-28): every request alone, `_meta`
//!   with the protocol version and the client's capabilities, the
//!   `MCP-Protocol-Version` header and SEP-2243's `Mcp-Method` and `Mcp-Name`,
//!   no session.
//!
//! No client crate: the raw HTTP/1.1 client of `common` over
//! `std::net::TcpStream`, every exchange on loopback.

use std::time::Duration;

use serde_json::{Value, json};

use super::hub::{AUTHORITY, Proxied};
use super::{HTTP_TIMEOUT, Reply, Stream, http_exchange};

/// The legacy era's protocol version: Claude Code's handshake.
pub const LEGACY: &str = "2025-11-25";

/// The stateless era's protocol version.
pub const STATELESS: &str = "2026-07-28";

/// rmcp's session header.
pub const SESSION_HEADER: &str = "Mcp-Session-Id";

/// rmcp's own bytes for an unknown session (docs/features/hub-mcp.md "Data"),
/// sent the same on every method.
pub const NO_SESSION: &str = "Not Found: Session not found";

/// A well-formed UUID v4 no hub issued.
pub const NEVER_ISSUED: &str = "00000000-0000-4000-8000-000000000000";

/// rmcp's `Parameters` message for arguments that break a tool's input schema.
pub const BAD_ARGUMENTS: &str = "failed to deserialize parameters";

/// The longest wait for an answer that must come at once: a refusal, a list.
pub const QUICK: Duration = Duration::from_secs(20);

/// `/mcp/<slug>`.
pub fn mcp_path(slug: &str) -> String {
    format!("/mcp/{slug}")
}

/// An MCP client's request of `login` as the proxy hands it to the daemon: the
/// public `Host` kept, the one `Remote-User` the door answered and the
/// `X-Forwarded-*` Traefik adds. No `Sec-Fetch-*` and no `Origin`: an MCP
/// client is no browser.
pub fn proxied(login: &str) -> Proxied {
    let mut headers = vec![
        ("Host".to_owned(), AUTHORITY.to_owned()),
        ("Remote-User".to_owned(), login.to_owned()),
    ];
    for (name, value) in [
        ("X-Forwarded-For", "203.0.113.7"),
        ("X-Forwarded-Host", AUTHORITY),
        ("X-Forwarded-Port", "443"),
        ("X-Forwarded-Proto", "https"),
        ("X-Forwarded-Server", "edge"),
    ] {
        headers.push((name.to_owned(), value.to_owned()));
    }
    Proxied { headers }
}

/// `proxied` with a POST's two: JSON in, JSON or SSE out.
pub fn posting(proxied: Proxied) -> Proxied {
    proxied
        .plus("Content-Type", "application/json")
        .plus("Accept", "application/json, text/event-stream")
}

/// A JSON-RPC request.
pub fn message(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// A JSON-RPC notification.
pub fn notification(method: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": method})
}

/// A 2025-11-25 `initialize`'s params.
pub fn initialize_params() -> Value {
    json!({
        "protocolVersion": LEGACY,
        "capabilities": {},
        "clientInfo": {"name": "specengine-http-tests", "version": "0.0.0"}
    })
}

/// A 2026-07-28 request's `_meta`.
pub fn stateless_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": STATELESS,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "specengine-http-tests", "version": "0.0.0"}
    })
}

/// One HTTP answer to an MCP request, and the JSON-RPC messages in it: SSE
/// `data:` frames, or a JSON body.
#[derive(Debug, Clone)]
pub struct Answer {
    pub reply: Reply,
    pub messages: Vec<Value>,
}

impl Answer {
    pub fn of(reply: Reply) -> Self {
        let messages = messages_of(&reply);
        Self { reply, messages }
    }

    pub fn status(&self) -> u16 {
        self.reply.status
    }

    /// The body, lossily.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.reply.body).into_owned()
    }

    /// Every `Mcp-Session-Id` the answer carries.
    pub fn sessions(&self) -> Vec<String> {
        self.reply
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(SESSION_HEADER))
            .map(|(_, value)| value.clone())
            .collect()
    }

    /// The one `Mcp-Session-Id`, if any; two fail the test.
    pub fn session(&self) -> Option<String> {
        let sessions = self.sessions();
        assert!(
            sessions.len() <= 1,
            "more than one Mcp-Session-Id: {:?}",
            self.reply.headers
        );
        sessions.into_iter().next()
    }

    /// The response to request `id`, if the answer holds it.
    pub fn response(&self, id: i64) -> Option<&Value> {
        self.messages.iter().find(|message| {
            message.get("id") == Some(&json!(id)) && message.get("method").is_none()
        })
    }

    /// [`Answer::response`], which must be there.
    pub fn expect_response(&self, id: i64, context: &str) -> Value {
        self.response(id).cloned().unwrap_or_else(|| {
            panic!(
                "{context}: no response to {id}: status {} headers {:?} body {}",
                self.status(),
                self.reply.headers,
                self.text()
            )
        })
    }
}

fn messages_of(reply: &Reply) -> Vec<Value> {
    let kind = reply.header("content-type").unwrap_or_default().to_owned();
    if kind.starts_with("text/event-stream") {
        sse_messages(&reply.body)
    } else if kind.starts_with("application/json") {
        match serde_json::from_slice::<Value>(&reply.body) {
            Ok(Value::Array(items)) => items,
            Ok(value) => vec![value],
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    }
}

/// The JSON-RPC messages of an SSE body: each block's `data:` lines joined,
/// empty data (rmcp's priming event) and comments (keep-alives) skipped.
pub fn sse_messages(body: &[u8]) -> Vec<Value> {
    let text = String::from_utf8_lossy(body).replace("\r\n", "\n");
    let mut messages = Vec::new();
    for block in text.split("\n\n") {
        let data: Vec<&str> = block
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(|rest| rest.strip_prefix(' ').unwrap_or(rest))
            .collect();
        let joined = data.join("\n");
        if joined.trim().is_empty() {
            continue;
        }
        messages
            .push(serde_json::from_str(&joined).unwrap_or_else(|error| {
                panic!("an SSE frame that is no JSON ({error}): {joined}")
            }));
    }
    messages
}

/// `method path` through the proxy with `body`, answered within `timeout`.
pub fn exchange(
    port: u16,
    method: &str,
    path: &str,
    proxied: &Proxied,
    body: &[u8],
    timeout: Duration,
) -> Answer {
    Answer::of(http_exchange(
        port,
        &proxied.bytes(method, path, body),
        timeout,
    ))
}

/// A POST of `message` by `proxied` (its JSON headers added).
pub fn post(port: u16, path: &str, proxied: &Proxied, message: &Value) -> Answer {
    exchange(
        port,
        "POST",
        path,
        &posting(proxied.clone()),
        message.to_string().as_bytes(),
        HTTP_TIMEOUT,
    )
}

/// The result of a JSON-RPC response; a JSON-RPC error fails the test.
pub fn result_of<'a>(response: &'a Value, context: &str) -> &'a Value {
    assert!(
        response.get("error").is_none(),
        "{context}: a JSON-RPC error: {response}"
    );
    response
        .get("result")
        .unwrap_or_else(|| panic!("{context}: no result: {response}"))
}

/// A tool result's text blocks, joined.
pub fn content_text(result: &Value) -> String {
    result["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Whether a tool result is an error result.
pub fn is_error(result: &Value) -> bool {
    result["isError"] == json!(true)
}

/// A `tools/list` result's names, sorted.
pub fn tool_names(list: &Value) -> Vec<String> {
    let mut names: Vec<String> = list["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("no tools: {list}"))
        .iter()
        .map(|tool| tool["name"].as_str().expect("a tool's name").to_owned())
        .collect();
    names.sort();
    names
}

/// One tool of a `tools/list` result.
pub fn tool<'a>(list: &'a Value, name: &str) -> &'a Value {
    list["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .find(|tool| tool["name"] == name)
        .unwrap_or_else(|| panic!("{name} is not listed"))
}

// ------------------------------------------------------------- the legacy era

/// A legacy session: `initialize` answered with its id, which every later
/// request names.
#[derive(Debug, Clone)]
pub struct Session {
    pub port: u16,
    pub path: String,
    pub login: String,
    pub id: String,
    next: i64,
}

impl Session {
    /// `initialize` (2025-11-25) as `login` at `path`, the raw answer.
    pub fn initialize(port: u16, path: &str, login: &str) -> Answer {
        Self::initialize_by(port, path, &proxied(login))
    }

    /// As [`Session::initialize`] with the request's headers `proxied`.
    pub fn initialize_by(port: u16, path: &str, proxied: &Proxied) -> Answer {
        post(
            port,
            path,
            proxied,
            &message(0, "initialize", initialize_params()),
        )
    }

    /// A session of `login` on project `slug`: `initialize` a 200 with one
    /// `Mcp-Session-Id` and a result, `notifications/initialized` a 202.
    pub fn open(port: u16, slug: &str, login: &str) -> Self {
        let path = mcp_path(slug);
        let answer = Self::initialize(port, &path, login);
        Self::opened(port, &path, login, &answer)
    }

    /// The session an `initialize` `answer` opened, initialized.
    pub fn opened(port: u16, path: &str, login: &str, answer: &Answer) -> Self {
        assert_eq!(
            answer.status(),
            200,
            "initialize as {login} at {path}: {}",
            answer.text()
        );
        let id = answer
            .session()
            .unwrap_or_else(|| panic!("initialize as {login}: no Mcp-Session-Id"));
        let response = answer.expect_response(0, "initialize");
        result_of(&response, "initialize");
        let session = Self {
            port,
            path: path.to_owned(),
            login: login.to_owned(),
            id,
            next: 1,
        };
        let initialized = session.notify_as(login, "notifications/initialized");
        assert_eq!(
            initialized.status(),
            202,
            "notifications/initialized: {}",
            initialized.text()
        );
        session
    }

    /// This client under another session id: one never issued, say.
    pub fn named(&self, id: &str) -> Self {
        Self {
            id: id.to_owned(),
            ..self.clone()
        }
    }

    /// This session's headers as `login` sends them: its id and its version.
    pub fn headers_as(&self, login: &str) -> Proxied {
        proxied(login)
            .plus(SESSION_HEADER, &self.id)
            .plus("MCP-Protocol-Version", LEGACY)
    }

    /// This session's headers as its own login sends them.
    pub fn headers(&self) -> Proxied {
        self.headers_as(&self.login)
    }

    /// A notification on this session by `login`.
    pub fn notify_as(&self, login: &str, method: &str) -> Answer {
        post(
            self.port,
            &self.path,
            &self.headers_as(login),
            &notification(method),
        )
    }

    /// The next request ID.
    pub fn next_id(&mut self) -> i64 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// `method` on this session by `login` at `path`: the request's ID and the
    /// raw answer.
    pub fn send_at(
        &mut self,
        path: &str,
        login: &str,
        method: &str,
        params: Value,
    ) -> (i64, Answer) {
        let id = self.next_id();
        let answer = post(
            self.port,
            path,
            &self.headers_as(login),
            &message(id, method, params),
        );
        (id, answer)
    }

    /// `method` on this session by `login`: the request's ID and the answer.
    pub fn send_as(&mut self, login: &str, method: &str, params: Value) -> (i64, Answer) {
        let path = self.path.clone();
        self.send_at(&path, login, method, params)
    }

    /// `method` on this session by its own login.
    pub fn send(&mut self, method: &str, params: Value) -> (i64, Answer) {
        let login = self.login.clone();
        self.send_as(&login, method, params)
    }

    /// `method`'s response, a 200.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let (id, answer) = self.send(method, params);
        assert_eq!(
            answer.status(),
            200,
            "{method} on {}: {}",
            self.id,
            answer.text()
        );
        answer.expect_response(id, method)
    }

    /// `tools/call name arguments`: the response.
    pub fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    /// `tools/list`'s result.
    pub fn tools(&mut self) -> Value {
        let response = self.request("tools/list", json!({}));
        result_of(&response, "tools/list").clone()
    }

    /// `tools/list` on this session by `login`, the raw answer, within
    /// [`QUICK`].
    pub fn list_as(&mut self, login: &str) -> Answer {
        let id = self.next_id();
        exchange(
            self.port,
            "POST",
            &self.path,
            &posting(self.headers_as(login)),
            message(id, "tools/list", json!({})).to_string().as_bytes(),
            QUICK,
        )
    }

    /// Whether this session still answers its own login: `tools/list` a 200
    /// with the list, or rmcp's 404.
    pub fn answers(&mut self) -> bool {
        let login = self.login.clone();
        let answer = self.list_as(&login);
        match answer.status() {
            200 => {
                assert!(
                    answer.messages.iter().any(|m| m.get("result").is_some()),
                    "a 200 with no list: {}",
                    answer.text()
                );
                true
            }
            404 => {
                assert_eq!(answer.text(), NO_SESSION, "the 404's bytes");
                false
            }
            other => panic!("session {}: status {other}: {}", self.id, answer.text()),
        }
    }

    /// `DELETE` of this session by `login`, within [`QUICK`].
    pub fn delete_as(&self, login: &str) -> Answer {
        exchange(
            self.port,
            "DELETE",
            &self.path,
            &self.headers_as(login),
            b"",
            QUICK,
        )
    }

    /// `GET` of this session by `login` answered whole within [`QUICK`]: for
    /// a refusal; a stream that opens fails the test by its timeout.
    pub fn get_as(&self, login: &str) -> Answer {
        exchange(
            self.port,
            "GET",
            &self.path,
            &self.headers_as(login).plus("Accept", "text/event-stream"),
            b"",
            QUICK,
        )
    }

    /// The session's SSE stream, opened by `login`: its head read.
    pub fn stream_as(&self, login: &str) -> Stream {
        Stream::open_raw(self.port, &self.path, &self.headers_as(login).pairs())
    }
}

// ---------------------------------------------------------- the stateless era

/// A 2026-07-28 client: every request alone.
#[derive(Debug, Clone)]
pub struct Stateless {
    pub port: u16,
    pub path: String,
    pub login: String,
    next: i64,
}

impl Stateless {
    pub fn new(port: u16, slug: &str, login: &str) -> Self {
        Self {
            port,
            path: mcp_path(slug),
            login: login.to_owned(),
            next: 1,
        }
    }

    /// The headers of a `method` request by `login`: SEP-2243's `Mcp-Method`
    /// and, for a call or a read, `Mcp-Name` (ASCII here, so plain).
    pub fn headers_as(login: &str, method: &str, params: &Value) -> Proxied {
        let mut headers = proxied(login)
            .plus("MCP-Protocol-Version", STATELESS)
            .plus("Mcp-Method", method);
        let name = match method {
            "tools/call" | "prompts/get" => params["name"].as_str(),
            "resources/read" | "resources/subscribe" | "resources/unsubscribe" => {
                params["uri"].as_str()
            }
            _ => None,
        };
        if let Some(name) = name {
            assert!(
                name.is_ascii() && name.trim() == name && !name.starts_with("=?"),
                "a name the header carries plain: {name:?}"
            );
            headers = headers.plus("Mcp-Name", name);
        }
        headers
    }

    /// `method` with `params` and `_meta`, by `login`: the request's ID and
    /// the raw answer.
    pub fn send_as(&mut self, login: &str, method: &str, params: Value) -> (i64, Answer) {
        let id = self.next;
        self.next += 1;
        let mut params = params;
        params["_meta"] = stateless_meta();
        let headers = Self::headers_as(login, method, &params);
        let answer = post(
            self.port,
            &self.path,
            &headers,
            &message(id, method, params),
        );
        (id, answer)
    }

    pub fn send(&mut self, method: &str, params: Value) -> (i64, Answer) {
        let login = self.login.clone();
        self.send_as(&login, method, params)
    }

    /// `method`'s response: a 200, or rmcp's 4xx carrying a JSON-RPC error;
    /// never a session.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let (id, answer) = self.send(method, params);
        assert!(
            answer.status() == 200 || (400..500).contains(&answer.status()),
            "{method}: status {}: {}",
            answer.status(),
            answer.text()
        );
        assert_eq!(
            answer.sessions(),
            Vec::<String>::new(),
            "{method}: a stateless request got a session"
        );
        answer.expect_response(id, method)
    }

    pub fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    pub fn tools(&mut self) -> Value {
        let response = self.request("tools/list", json!({}));
        result_of(&response, "tools/list").clone()
    }
}

// ------------------------------------------------------------- either era

/// The two eras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Legacy,
    Stateless,
}

pub const ERAS: [Era; 2] = [Era::Legacy, Era::Stateless];

/// A client of either era.
#[derive(Debug, Clone)]
pub enum Client {
    Legacy(Session),
    Stateless(Stateless),
}

impl Client {
    pub fn open(era: Era, port: u16, slug: &str, login: &str) -> Self {
        match era {
            Era::Legacy => Self::Legacy(Session::open(port, slug, login)),
            Era::Stateless => Self::Stateless(Stateless::new(port, slug, login)),
        }
    }

    pub fn era(&self) -> Era {
        match self {
            Self::Legacy(_) => Era::Legacy,
            Self::Stateless(_) => Era::Stateless,
        }
    }

    pub fn request(&mut self, method: &str, params: Value) -> Value {
        match self {
            Self::Legacy(session) => session.request(method, params),
            Self::Stateless(client) => client.request(method, params),
        }
    }

    /// `tools/call`'s result; a JSON-RPC error fails the test.
    pub fn call(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        result_of(&response, name).clone()
    }

    pub fn tools(&mut self) -> Value {
        match self {
            Self::Legacy(session) => session.tools(),
            Self::Stateless(client) => client.tools(),
        }
    }
}
