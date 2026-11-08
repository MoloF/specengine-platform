//! AC-01 of docs/features/daemon-read.md: the fence. The listening socket
//! is `127.0.0.1` only and no flag names another address; a foreign
//! `Host`, `Origin` or `Sec-Fetch-Site` is a 403 before anything is read
//! (a fresh `HOME` stays empty); no response, `OPTIONS` included, carries
//! an `Access-Control-*` header. M: bind `0.0.0.0`; Host check off;
//! permissive CORS.
//!
//! docs/features/ui-embed.md T4: the web UI's `GET /` and
//! `GET /assets/x.js` stand behind the same fence (403 for a foreign
//! `Host`, `Origin` or `Sec-Fetch-Site`; `no-store`, no CORS header). M: the
//! UI routes outside the fence's layer.
//!
//! docs/features/board-ui.md "The fence stays": `fence()` does not change;
//! it runs before routing, so the two card reads and the six writes are
//! fenced like every route: a foreign `Host`, `Origin` or `Sec-Fetch-Site`
//! is a 403 before anything is read, no response carries an
//! `Access-Control-*` header (a `same-origin` claim beside a foreign
//! `Origin` or `Host` included: `cards.rs`, AC-03).
//!
//! docs/features/card-trace.md: the trace read and the two clears stand
//! behind the same fence (`trace.rs`, AC-10).
//!
//! docs/features/local-landing.md "Acceptance criteria" (RM-0080): the
//! landing, the hub's exception given to the local fence through one
//! predicate. `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest:
//! document` passes the `Sec-Fetch-Site` step whatever it says (LL-1, M1);
//! one header changed is the 403 (LL-2, M2, M3); any other method or path
//! is the 403, a path a decoder or a trimmer would read as `/` included
//! (LL-3, M4, M5); `Host` and `Origin` are judged first, and a header sent
//! twice is refused (LL-4, M6, M7). LL-5 is this file's tests above the
//! `ll` ones, `hub.rs`, `serve.rs` and `ui.rs`, unchanged; M8 turns either
//! `hub.rs`'s AC-03 or the `ll` tests red.

mod common;

use std::ffi::OsStr;
use std::process::Command;

use common::{Reply, Scratch, Server, beyond_daemon, root_args};

/// The daemon's listening sockets as `lsof` names them (`127.0.0.1:N`,
/// `*:N`, `[::1]:N`).
fn listening(server: &Server) -> Vec<String> {
    let output = Command::new("/usr/sbin/lsof")
        .args([
            "-nP",
            "-a",
            "-p",
            &server.pid().to_string(),
            "-iTCP",
            "-sTCP:LISTEN",
            "-Fn",
        ])
        .output()
        .expect("run lsof");
    String::from_utf8(output.stdout)
        .expect("lsof output")
        .lines()
        .filter_map(|line| line.strip_prefix('n').map(str::to_owned))
        .collect()
}

#[test]
fn ac01_the_socket_is_loopback_only_and_no_flag_names_another_address() {
    let scratch = Scratch::new("fence-bind");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    assert_eq!(
        server.lines.last().map(String::as_str),
        Some(format!("listening http://127.0.0.1:{}", server.port).as_str())
    );
    let sockets = listening(&server);
    assert_eq!(
        sockets,
        [format!("127.0.0.1:{}", server.port)],
        "the one listening socket is 127.0.0.1:<port>"
    );

    for flag in ["--host", "--bind"] {
        let mut args = root_args(&[&a]);
        args.push(OsStr::new(flag));
        args.push(OsStr::new("0.0.0.0"));
        let refused = Server::try_start(&home, scratch.path(), &args)
            .err()
            .unwrap_or_else(|| panic!("{flag} 0.0.0.0 started a server"));
        assert_eq!(refused.code, Some(2), "{flag}: {refused:?}");
        assert!(refused.stdout.is_empty(), "{flag}: {refused:?}");
        assert!(
            refused.stderr.starts_with("specengine-http: ") && refused.stderr.contains(flag),
            "{flag}: {refused:?}"
        );
    }
}

/// Every endpoint the fence stands before, with a valid query.
fn endpoints(port: u16) -> Vec<(&'static str, String)> {
    let _ = port;
    vec![
        ("GET", "/api/projects".to_owned()),
        ("GET", "/api/projects/lantern-keep/tree".to_owned()),
        (
            "GET",
            "/api/projects/lantern-keep/nodes/MEC-STAMINA".to_owned(),
        ),
        (
            "GET",
            "/api/projects/lantern-keep/search?query=stamina".to_owned(),
        ),
        (
            "GET",
            "/api/projects/lantern-keep/bundle?node_ids=MEC-STAMINA".to_owned(),
        ),
        ("GET", "/api/projects/lantern-keep/inbox".to_owned()),
        (
            "GET",
            "/api/projects/lantern-keep/proposals/PR-0001".to_owned(),
        ),
        ("GET", "/api/projects/lantern-keep/events".to_owned()),
        (
            "POST",
            "/api/projects/lantern-keep/proposals/PR-0001/decision".to_owned(),
        ),
        ("GET", "/api/projects/lantern-keep/cards".to_owned()),
        ("GET", "/api/projects/lantern-keep/cards/C-0001".to_owned()),
        ("POST", "/api/projects/lantern-keep/cards".to_owned()),
        (
            "POST",
            "/api/projects/lantern-keep/cards/C-0001/move".to_owned(),
        ),
        (
            "POST",
            "/api/projects/lantern-keep/cards/C-0001/edit".to_owned(),
        ),
        (
            "POST",
            "/api/projects/lantern-keep/cards/C-0001/assign".to_owned(),
        ),
        (
            "POST",
            "/api/projects/lantern-keep/cards/C-0001/cancel".to_owned(),
        ),
        (
            "POST",
            "/api/projects/lantern-keep/cards/C-0001/accept".to_owned(),
        ),
        (
            "POST",
            "/api/projects/lantern-keep/cards/C-0001/clear".to_owned(),
        ),
        ("GET", "/api/projects/lantern-keep/trace".to_owned()),
        ("POST", "/api/projects/lantern-keep/trace/clear".to_owned()),
        ("GET", "/no/such/route".to_owned()),
        ("OPTIONS", "/api/projects".to_owned()),
        // The web UI (docs/features/ui-embed.md T4).
        ("GET", "/".to_owned()),
        ("GET", "/assets/x.js".to_owned()),
    ]
}

/// What `GET /` answers on this tree's daemon: the page, or the 503 of a
/// daemon compiled without `ui/dist` (docs/canon/serve-ui.md "Routes").
fn page_status() -> u16 {
    if common::repository_root()
        .join("ui/dist/index.html")
        .is_file()
    {
        200
    } else {
        503
    }
}

fn assert_plain(reply: &Reply, context: &str) {
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

#[test]
fn ac01_a_foreign_host_origin_or_fetch_site_is_refused_before_anything_is_read() {
    let scratch = Scratch::new("fence-refused");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let own = format!("127.0.0.1:{}", server.port);
    let evil_host = format!("evil.example:{}", server.port);
    let foreign: Vec<Vec<(&str, &str)>> = vec![
        vec![("Host", evil_host.as_str())],
        vec![("Host", own.as_str()), ("Origin", "http://evil.example")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "cross-site")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "same-site")],
    ];
    for headers in &foreign {
        for (method, path) in endpoints(server.port) {
            let reply = server.raw(&common::request_bytes(method, &path, headers));
            let context = format!("{method} {path} {headers:?}");
            assert_eq!(reply.status, 403, "{context}: {}", reply.text());
            let message = reply.error_message();
            assert!(message.starts_with("refused: "), "{context}: {message}");
            assert_plain(&reply, &context);
        }
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "a refused request read nothing: the fresh HOME holds {:?}",
        beyond_daemon(&home)
    );

    // The own origin passes: either host name, its Origin, same-origin
    // or none.
    let localhost = format!("localhost:{}", server.port);
    let own_origin = format!("http://{own}");
    let local_origin = format!("http://{localhost}");
    let passing: Vec<Vec<(&str, &str)>> = vec![
        vec![("Host", own.as_str())],
        vec![("Host", localhost.as_str())],
        vec![("Host", own.as_str()), ("Origin", own_origin.as_str())],
        vec![
            ("Host", localhost.as_str()),
            ("Origin", local_origin.as_str()),
        ],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "same-origin")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "none")],
    ];
    for headers in &passing {
        let reply = server.raw(&common::request_bytes("GET", "/api/projects", headers));
        assert_eq!(reply.status, 200, "{headers:?}: {}", reply.text());
        assert_plain(&reply, &format!("{headers:?}"));
    }
    // An Origin of the other host name, or another port, or https: refused.
    let wrong_port = format!("http://127.0.0.1:{}", server.port.wrapping_add(1));
    let https = format!("https://{own}");
    for origin in [wrong_port.as_str(), https.as_str(), "null"] {
        let reply = server.request("GET", "/api/projects", &[("Origin", origin)]);
        assert_eq!(reply.status, 403, "Origin {origin}: {}", reply.text());
        reply.error_message();
    }
}

#[test]
fn ac01_no_response_carries_an_access_control_header() {
    let scratch = Scratch::new("fence-cors");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let own_origin = format!("http://127.0.0.1:{}", server.port);
    // A preflight, from the own origin and from a foreign one.
    for origin in [own_origin.as_str(), "http://evil.example"] {
        let reply = server.request(
            "OPTIONS",
            "/api/projects/lantern-keep/tree",
            &[
                ("Origin", origin),
                ("Access-Control-Request-Method", "GET"),
                ("Access-Control-Request-Headers", "last-event-id"),
            ],
        );
        let want = if origin == own_origin { 405 } else { 403 };
        assert_eq!(
            reply.status,
            want,
            "OPTIONS from {origin}: {}",
            reply.text()
        );
        reply.error_message();
        assert_plain(&reply, &format!("OPTIONS from {origin}"));
    }
    // Every endpoint's answer, with the own Origin: 200, 404, 405, 403.
    for (method, path) in endpoints(server.port) {
        if path.ends_with("/events") {
            let stream = common::Stream::open(server.port, &path, &[("Origin", &own_origin)]);
            assert_eq!(stream.status, 200);
            assert!(
                stream
                    .headers
                    .iter()
                    .all(|(name, _)| !name.starts_with("access-control-")),
                "events: {:?}",
                stream.headers
            );
            assert_eq!(stream.header("cache-control"), Some("no-store"));
            continue;
        }
        let reply = server.request(method, &path, &[("Origin", &own_origin)]);
        assert_plain(&reply, &format!("{method} {path}"));
        let want = match (method, path.as_str()) {
            ("OPTIONS", _) => 405,
            ("POST", _) => 403,
            (_, "/no/such/route")
            | (_, "/api/projects/lantern-keep/proposals/PR-0001")
            | (_, "/api/projects/lantern-keep/cards/C-0001")
            | (_, "/assets/x.js") => 404,
            (_, "/") => page_status(),
            _ => 200,
        };
        assert_eq!(reply.status, want, "{method} {path}: {}", reply.text());
    }
    // HEAD, DELETE, PUT: 405 with the error body.
    for method in ["DELETE", "PUT", "PATCH"] {
        let reply = server.request(method, "/api/projects", &[]);
        assert_eq!(reply.status, 405, "{method}: {}", reply.text());
        reply.error_message();
        assert_plain(&reply, method);
    }
}

// ------------------------------------------------------------ local-landing

/// The `Sec-Fetch-Site` step's refusal, byte for byte ("Data").
const SITE_LINE: &str = "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`";

/// The page's policy ("Data"; `ui.rs`' `PAGE_POLICY`).
const PAGE_POLICY: &str = "default-src 'self'; img-src 'self' data:; object-src 'none'; \
                           base-uri 'none'; frame-ancestors 'none'";

/// The landing's headers from another site: `Host` the daemon's own, then
/// `site`, `navigate` and `document`, in that order.
fn landing_from(port: u16, site: &'static str) -> Vec<(String, String)> {
    vec![
        ("Host".to_owned(), format!("127.0.0.1:{port}")),
        ("Sec-Fetch-Site".to_owned(), site.to_owned()),
        ("Sec-Fetch-Mode".to_owned(), "navigate".to_owned()),
        ("Sec-Fetch-Dest".to_owned(), "document".to_owned()),
    ]
}

/// `headers` with `name` set to `value` (the first of that name replaced).
fn set(headers: &[(String, String)], name: &str, value: &str) -> Vec<(String, String)> {
    let mut out = headers.to_vec();
    let at = out
        .iter()
        .position(|(key, _)| key == name)
        .unwrap_or_else(|| panic!("no {name} in {headers:?}"));
    out[at].1 = value.to_owned();
    out
}

/// `headers` without `name`.
fn without(headers: &[(String, String)], name: &str) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(key, _)| key != name)
        .cloned()
        .collect()
}

/// `headers` with `name: value` added once more, at the end.
fn plus(headers: &[(String, String)], name: &str, value: &str) -> Vec<(String, String)> {
    let mut out = headers.to_vec();
    out.push((name.to_owned(), value.to_owned()));
    out
}

/// `method path` with exactly `headers`, as written.
fn send(server: &Server, method: &str, path: &str, headers: &[(String, String)]) -> Reply {
    let borrowed: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    server.raw(&common::request_bytes(method, path, &borrowed))
}

/// The fence's 403 with exactly `line` (a `HEAD` carries no body to read a
/// line from), `no-store`, no `Allow`, no `Access-Control-*`.
fn assert_refused_with(reply: &Reply, method: &str, line: &str, context: &str) {
    assert_eq!(
        reply.status,
        403,
        "{context}: {}",
        String::from_utf8_lossy(&reply.body)
    );
    assert_plain(reply, context);
    assert_eq!(
        reply.header("allow"),
        None,
        "{context}: {:?}",
        reply.headers
    );
    if method == "HEAD" {
        assert!(
            reply.body.is_empty(),
            "{context}: a HEAD answer has no body"
        );
    } else {
        assert_eq!(reply.error_message(), line, "{context}");
    }
}

/// LL-1: a top-level navigation to `/` from another site, `cross-site` or
/// `same-site`, a query included, answers what a same-origin `GET /`
/// answers: the page with its headers at 200, the not-built body at 503.
/// M1: the exception removed (403).
#[test]
fn ll1_a_top_level_navigation_from_any_site_opens_the_page() {
    let scratch = Scratch::new("fence-landing");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let own = server.request("GET", "/", &[("Sec-Fetch-Site", "same-origin")]);
    assert_eq!(own.status, page_status(), "the same-origin page");
    for site in ["cross-site", "same-site"] {
        for path in ["/", "/?scenario=normal"] {
            let context = format!("GET {path} from {site}");
            let reply = send(&server, "GET", path, &landing_from(server.port, site));
            assert_eq!(
                reply.status,
                page_status(),
                "{context}: {}",
                String::from_utf8_lossy(&reply.body)
            );
            assert_plain(&reply, &context);
            assert_eq!(
                reply.body, own.body,
                "{context}: the same-origin page's bytes"
            );
            if reply.status == 200 {
                assert_eq!(
                    reply.header("x-content-type-options"),
                    Some("nosniff"),
                    "{context}"
                );
                assert_eq!(
                    reply.header("content-security-policy"),
                    Some(PAGE_POLICY),
                    "{context}: the page's policy, `frame-ancestors 'none'` in it"
                );
            } else {
                let message = reply.error_message();
                assert!(
                    message.starts_with("this specengine-http holds no web UI"),
                    "{context}: the not-built body: {message}"
                );
            }
        }
    }
}

/// LL-2: the LL-1 request with one header changed is no top-level document
/// any more: a frame, a fetch, or either header missing is the 403 of the
/// `Sec-Fetch-Site` step. M2: `Sec-Fetch-Mode` judged alone (`iframe`
/// passes). M3: `Sec-Fetch-Dest` judged alone (`no-cors` passes).
#[test]
fn ll2_only_a_top_level_document_is_the_landing() {
    let scratch = Scratch::new("fence-landing-document");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    for site in ["cross-site", "same-site"] {
        let landing = landing_from(server.port, site);
        let changed = [
            (
                "Sec-Fetch-Dest iframe",
                set(&landing, "Sec-Fetch-Dest", "iframe"),
            ),
            (
                "Sec-Fetch-Dest frame",
                set(&landing, "Sec-Fetch-Dest", "frame"),
            ),
            (
                "Sec-Fetch-Mode no-cors",
                set(&landing, "Sec-Fetch-Mode", "no-cors"),
            ),
            (
                "Sec-Fetch-Mode cors",
                set(&landing, "Sec-Fetch-Mode", "cors"),
            ),
            ("no Sec-Fetch-Mode", without(&landing, "Sec-Fetch-Mode")),
            ("no Sec-Fetch-Dest", without(&landing, "Sec-Fetch-Dest")),
        ];
        for (change, headers) in &changed {
            let reply = send(&server, "GET", "/", headers);
            assert_refused_with(
                &reply,
                "GET",
                SITE_LINE,
                &format!("GET / from {site}, {change}"),
            );
        }
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "a refused request read nothing: {:?}",
        beyond_daemon(&home)
    );
}

/// LL-3: the LL-1 headers on any other path or method are the 403 of the
/// `Sec-Fetch-Site` step. M4: the exception keyed on the headers and not on
/// `GET /` (an asset, a route, `HEAD` or `POST` pass). M5: the path compared
/// after decoding or trimming (`//`, `/%2F`, `/%20`, `/.` and `/./` pass,
/// then answer the router's 404).
#[test]
fn ll3_only_get_slash_is_the_landing() {
    let scratch = Scratch::new("fence-landing-route");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let landing = landing_from(server.port, "cross-site");
    let others = [
        ("GET", "/assets/x.js"),
        ("GET", "/index.html"),
        ("GET", "/api/projects"),
        ("GET", "/api/projects/lantern-keep/tree"),
        ("HEAD", "/"),
        ("POST", "/"),
        // M5: what a decoder or a trimmer reads as `/`.
        ("GET", "//"),
        ("GET", "/%2F"),
        ("GET", "/%20"),
        ("GET", "/."),
        ("GET", "/./"),
    ];
    for (method, path) in others {
        let reply = send(&server, method, path, &landing);
        assert_refused_with(
            &reply,
            method,
            SITE_LINE,
            &format!("{method} {path} cross-site"),
        );
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "a refused request read nothing: {:?}",
        beyond_daemon(&home)
    );
}

/// LL-4: `Host`, then `Origin`, then the `Sec-Fetch-Site` step: the landing
/// passes neither of the first two, and a `Sec-Fetch-*` header sent twice is
/// refused by its step. M6: the exception judged before step 1. M7: before
/// step 2.
#[test]
fn ll4_host_and_origin_are_judged_before_the_landing() {
    let scratch = Scratch::new("fence-landing-order");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let port = server.port;
    let host_line = format!(
        "refused: the Host header must be 127.0.0.1:{port} or localhost:{port}: this server \
         serves its own origin only"
    );
    let origin_line = format!(
        "refused: the Origin header, when sent, must be http://127.0.0.1:{port} or \
         http://localhost:{port}"
    );
    for site in ["cross-site", "same-site"] {
        let landing = landing_from(port, site);
        let cases = [
            (
                "a foreign Host",
                set(&landing, "Host", &format!("evil.example:{port}")),
                host_line.clone(),
            ),
            (
                "a foreign Origin",
                plus(&landing, "Origin", "http://evil.example"),
                origin_line.clone(),
            ),
            (
                "Sec-Fetch-Mode twice",
                plus(&landing, "Sec-Fetch-Mode", "navigate"),
                "refused: the Sec-Fetch-Mode header is sent more than once".to_owned(),
            ),
            // L3: the predicate's `single`, as the hub's.
            (
                "Sec-Fetch-Dest twice",
                plus(&landing, "Sec-Fetch-Dest", "document"),
                "refused: the Sec-Fetch-Dest header is sent more than once".to_owned(),
            ),
            // "Edge cases": the duplicate is refused before the predicate.
            (
                "Sec-Fetch-Site twice",
                plus(&landing, "Sec-Fetch-Site", site),
                "refused: the Sec-Fetch-Site header is sent more than once".to_owned(),
            ),
        ];
        for (case, headers, line) in &cases {
            let reply = send(&server, "GET", "/", headers);
            assert_refused_with(
                &reply,
                "GET",
                line,
                &format!("the landing from {site}, {case}"),
            );
        }
    }
    // The other host name is the own origin: the landing passes there too.
    let localhost = set(
        &landing_from(port, "cross-site"),
        "Host",
        &format!("localhost:{port}"),
    );
    let reply = send(&server, "GET", "/", &localhost);
    assert_eq!(
        reply.status,
        page_status(),
        "the landing on localhost:{port}"
    );
    assert_plain(&reply, "the landing on localhost");
}
