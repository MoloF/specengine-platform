//! A hub for the integration tests: the test proxy (docs/canon/hub.md "Tests")
//! and a daemon started with `--public-origin`, shared by `hub.rs`,
//! `hub_team.rs` and `daemon_keys.rs`. It proves the daemon's half; the real
//! proxy's half is RM-0064's.

use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};

use super::{
    HTTP_TIMEOUT, Reply, Scratch, Server, find, parse_reply, request_bytes, root_args, snapshot,
    spec_json,
};

/// The public origin of every hub here, and its authority.
pub const ORIGIN: &str = "https://hub.example.test";
pub const AUTHORITY: &str = "hub.example.test";

/// The print shop's project.
pub const P: &str = "/api/projects/card-press";

// ---------------------------------------------------------- the test proxy

/// One request as the proxy hands it to the daemon: the test's Traefik
/// (docs/canon/hub.md "Tests"). What the browser sent, the public `Host` kept
/// (`passHostHeader`), the `Sec-Fetch-*` of its kind of request, the one
/// `Remote-User` the login door answered, and the `X-Forwarded-*` Traefik adds
/// to every request. It proves the daemon's half; the real proxy's half is
/// RM-0064's.
#[derive(Debug, Clone)]
pub struct Proxied {
    pub headers: Vec<(String, String)>,
}

impl Proxied {
    pub fn browser(login: &str, fetch: [(&str, &str); 3]) -> Self {
        let mut headers = vec![("Host".to_owned(), AUTHORITY.to_owned())];
        headers.extend(fetch.map(|(name, value)| (name.to_owned(), value.to_owned())));
        headers.push(("Remote-User".to_owned(), login.to_owned()));
        for (name, value) in [
            ("X-Forwarded-For", "203.0.113.7"),
            ("X-Forwarded-Host", AUTHORITY),
            ("X-Forwarded-Port", "443"),
            ("X-Forwarded-Proto", "https"),
            ("X-Forwarded-Server", "edge"),
        ] {
            headers.push((name.to_owned(), value.to_owned()));
        }
        Self { headers }
    }

    /// A read of the page's own: a same-origin `fetch`, which sends no
    /// `Origin` on a GET.
    pub fn fetch(login: &str) -> Self {
        Self::browser(
            login,
            [
                ("Sec-Fetch-Site", "same-origin"),
                ("Sec-Fetch-Mode", "cors"),
                ("Sec-Fetch-Dest", "empty"),
            ],
        )
    }

    /// A write of the page's own: its `Origin` and its JSON.
    pub fn write(login: &str) -> Self {
        Self::fetch(login)
            .plus("Origin", ORIGIN)
            .plus("Content-Type", "application/json")
    }

    /// A top-level navigation arriving from `site`.
    pub fn navigation(site: &str, login: &str) -> Self {
        Self::browser(
            login,
            [
                ("Sec-Fetch-Site", site),
                ("Sec-Fetch-Mode", "navigate"),
                ("Sec-Fetch-Dest", "document"),
            ],
        )
    }

    /// With `name` holding `value` alone, added when absent.
    pub fn set(self, name: &str, value: &str) -> Self {
        self.without(name).plus(name, value)
    }

    /// With one line more, whatever is there.
    pub fn plus(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn without(mut self, name: &str) -> Self {
        self.headers.retain(|(held, _)| held != name);
        self
    }

    pub fn pairs(&self) -> Vec<(&str, &str)> {
        self.headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect()
    }

    /// The request's bytes, a `Content-Length` with a body.
    pub fn bytes(&self, method: &str, path: &str, body: &[u8]) -> Vec<u8> {
        let length = body.len().to_string();
        let mut headers = self.pairs();
        if !body.is_empty() {
            headers.push(("Content-Length", length.as_str()));
        }
        let mut bytes = request_bytes(method, path, &headers);
        bytes.extend_from_slice(body);
        bytes
    }
}

/// `method path` through the proxy.
pub fn through(server: &Server, method: &str, path: &str, proxied: &Proxied, body: &[u8]) -> Reply {
    server.raw(&proxied.bytes(method, path, body))
}

// ----------------------------------------------------------------- a hub

/// `method path` through the proxy in two steps, as a slow client sends it: the
/// head with `Expect: 100-continue`, then, on the `100` hyper sends when a
/// handler first reads the body, so past the handler's first judgement,
/// `between`, then the body. No clock: nothing races. The answer, and whether
/// the `100` came; with none the answer is what came instead, and `between` has
/// not run.
pub fn through_in_two_steps(
    server: &Server,
    method: &str,
    path: &str,
    proxied: &Proxied,
    body: &[u8],
    between: impl FnOnce(),
) -> (Reply, bool) {
    let bytes = proxied
        .clone()
        .plus("Expect", "100-continue")
        .bytes(method, path, body);
    let (head, rest) = bytes.split_at(bytes.len() - body.len());
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect to the daemon");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("timeout");
    let mut writer = stream.try_clone().expect("a second handle");
    writer.write_all(head).expect("write the head");
    let started = Instant::now();
    let mut raw = Vec::new();
    let mut read_more = |raw: &mut Vec<u8>| -> bool {
        let mut buffer = [0u8; 65536];
        assert!(
            started.elapsed() < HTTP_TIMEOUT,
            "no complete answer within {HTTP_TIMEOUT:?}: {:?}",
            String::from_utf8_lossy(&raw[..raw.len().min(600)])
        );
        match stream.read(&mut buffer) {
            Ok(0) => false,
            Ok(n) => {
                raw.extend_from_slice(&buffer[..n]);
                true
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                true
            }
            Err(error) => panic!("read the answer: {error}"),
        }
    };
    let end = loop {
        if let Some(end) = find(&raw, b"\r\n\r\n") {
            break end;
        }
        assert!(read_more(&mut raw), "the daemon closed before answering");
    };
    let continued = raw.starts_with(b"HTTP/1.1 100");
    if continued {
        raw.drain(..end + 4);
        between();
        writer.write_all(rest).expect("write the body");
    }
    while read_more(&mut raw) {}
    (parse_reply(&raw), continued)
}

/// `--root` per root, `--port 0`, `--public-origin origin`.
pub fn hub_args<'a>(roots: &[&Path], origin: &'a str) -> Vec<&'a OsStr> {
    let mut args: Vec<&'a OsStr> = root_args(roots);
    args.push(OsStr::new("--public-origin"));
    args.push(OsStr::new(origin));
    args
}

/// A git repository of a fixture, a `HOME` and a working directory, with no
/// daemon yet. An identity in the repository, as an owner's has one: a write
/// through the daemon could commit.
pub struct Setup {
    pub root: PathBuf,
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub scratch: Scratch,
}

impl Setup {
    pub fn new(name: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo(fixture, "served", "main");
        let git = scratch.git();
        git.run(&root, &["config", "user.name", "Owner"]);
        git.run(&root, &["config", "user.email", "owner@example.invalid"]);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            root,
            home,
            cwd,
            scratch,
        }
    }

    pub fn start(self) -> Hub {
        let server = Server::start(&self.home, &self.cwd, &hub_args(&[&self.root], ORIGIN));
        Hub {
            server,
            setup: self,
        }
    }
}

/// A running hub. `server` is first: it stops before the scratch goes.
pub struct Hub {
    pub server: Server,
    pub setup: Setup,
}

/// What a refusal must leave of the queue, as AC-08 names it.
#[derive(Debug, PartialEq)]
pub struct Stored {
    pub events: Vec<(String, Value)>,
    pub board: String,
    pub inbox: String,
    pub round: String,
    pub proposals: Vec<Value>,
}

impl Hub {
    /// `GET path` by `login`.
    pub fn get(&self, login: &str, path: &str) -> Reply {
        through(&self.server, "GET", path, &Proxied::fetch(login), b"")
    }

    /// `POST <P>/path` with the JSON `body` by `login`.
    pub fn post(&self, login: &str, path: &str, body: &Value) -> Reply {
        through(
            &self.server,
            "POST",
            &format!("{P}/{path}"),
            &Proxied::write(login),
            body.to_string().as_bytes(),
        )
    }

    /// As [`Hub::post`], the answer a 200.
    pub fn done(&self, login: &str, path: &str, body: &Value) -> Value {
        let reply = self.post(login, path, body);
        assert_eq!(reply.status, 200, "{login} {path} {body}: {}", reply.text());
        reply.json()
    }

    /// `show`'s document, as a page reads it.
    pub fn show(&self, id: &str) -> Value {
        self.get("mira", &format!("{P}/cards/{id}"))
            .status(200)
            .json()
    }

    /// Every stored event, `(type, payload)`.
    pub fn events(&self) -> Vec<(String, Value)> {
        events_of(&self.setup.home, &self.setup.root)
    }

    /// The events and the board as a page reads it: every row and event.
    pub fn board(&self) -> (Vec<(String, Value)>, String) {
        let board = self
            .get("mira", &format!("{P}/cards?with_cancelled=true"))
            .status(200)
            .text()
            .to_owned();
        (self.events(), board)
    }

    /// `git status`, `HEAD`, the refs, the worktrees and every file.
    pub fn repository(&self) -> Vec<String> {
        let git = self.setup.scratch.git();
        let root = &self.setup.root;
        let files: Vec<String> = snapshot(root)
            .into_iter()
            .filter(|(path, _)| path != ".git/index")
            .map(|(path, bytes)| format!("{path} {bytes:?}"))
            .collect();
        vec![
            git.run(root, &["status", "--porcelain", "--untracked-files=all"]),
            git.run(root, &["rev-parse", "HEAD"]),
            git.run(root, &["for-each-ref", "--format=%(refname) %(objectname)"]),
            git.run(root, &["worktree", "list", "--porcelain"]),
            files.join("\n"),
        ]
    }

    /// A proposal's stored fields, read through the CLI: the review's preview
    /// is computed when asked.
    pub fn proposal(&self, id: &str) -> Value {
        let setup = &self.setup;
        let mut review = spec_json(&setup.home, &setup.cwd, &setup.root, &["review", id]).json();
        review.as_object_mut().expect("an object").retain(|key, _| {
            [
                "id",
                "status",
                "decided_by",
                "decided_at",
                "decision_note",
                "applied_commit",
                "updated_at",
                "record_id",
                "staged",
                "staged_at",
            ]
            .contains(&key.as_str())
        });
        review
    }

    pub fn stored(&self, ids: &[&str]) -> Stored {
        let (events, board) = self.board();
        Stored {
            events,
            board,
            inbox: self.text("tomas", "inbox"),
            round: self.text("tomas", "round"),
            proposals: ids.iter().map(|id| self.proposal(id)).collect(),
        }
    }

    /// `GET <P>/path`, a 200, its bytes.
    pub fn text(&self, login: &str, path: &str) -> String {
        self.get(login, &format!("{P}/{path}"))
            .status(200)
            .text()
            .to_owned()
    }
}

pub fn events_of(home: &Path, root: &Path) -> Vec<(String, Value)> {
    let env = Env {
        cwd: root.to_path_buf(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(root.to_path_buf()),
        config: None,
    };
    events_after(&env, &globals, Some(0))
        .expect("the events read")
        .events
        .into_iter()
        .map(|event| {
            let payload = serde_json::from_str(&event.payload).expect("a JSON payload");
            (event.event_type, payload)
        })
        .collect()
}

/// The hub's five writes (docs/canon/hub.md "Routes") on a new card `id`, in
/// an order that lands, on the print shop's `plates`, which has no `moves`.
pub fn five_writes(id: &str) -> [(String, Value, &'static str); 5] {
    [
        (
            "cards".to_owned(),
            json!({"title": "Back page", "discipline": "plates"}),
            "card.created",
        ),
        (
            format!("cards/{id}/move"),
            json!({"to": "checked"}),
            "card.moved",
        ),
        (
            format!("cards/{id}/edit"),
            json!({"goal": "Fill the back."}),
            "card.edited",
        ),
        (
            format!("cards/{id}/assign"),
            json!({"assignee": "ana"}),
            "card.assigned",
        ),
        (
            format!("cards/{id}/cancel"),
            json!({"reason": "Pulled for space."}),
            "card.cancelled",
        ),
    ]
}

/// No `Access-Control-*`, `no-store`.
pub fn assert_plain(reply: &Reply, context: &str) {
    assert!(
        reply.cors_headers().is_empty(),
        "{context}: an Access-Control-* header: {:?}",
        reply.headers
    );
    assert_eq!(
        reply.header("cache-control"),
        Some("no-store"),
        "{context}: {:?}",
        reply.headers
    );
}

/// The local fence's way of saying no (docs/canon/hub.md "Fence"): 403, the
/// error body holding `line`, no `Allow`, `no-store`, no `Access-Control-*`.
pub fn assert_refused(reply: &Reply, line: &str, context: &str) {
    assert_eq!(
        reply.status,
        403,
        "{context}: {}",
        String::from_utf8_lossy(&reply.body)
    );
    assert_eq!(reply.error_message(), line, "{context}");
    assert_plain(reply, context);
    assert_eq!(
        reply.header("allow"),
        None,
        "{context}: {:?}",
        reply.headers
    );
}
