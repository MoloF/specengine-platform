//! AC-01 of docs/features/daemon-read.md: the fence. The listening socket
//! is `127.0.0.1` only and no flag names another address; a foreign
//! `Host`, `Origin` or `Sec-Fetch-Site` is a 403 before anything is read
//! (a fresh `HOME` stays empty); no response, `OPTIONS` included, carries
//! an `Access-Control-*` header. M: bind `0.0.0.0`; Host check off;
//! permissive CORS.

mod common;

use std::ffi::OsStr;
use std::process::Command;

use common::{Reply, Scratch, Server, root_args, snapshot};

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
        ("GET", "/no/such/route".to_owned()),
        ("OPTIONS", "/api/projects".to_owned()),
    ]
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
        snapshot(&home).is_empty(),
        "a refused request read nothing: the fresh HOME holds {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
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
            (_, "/no/such/route") | (_, "/api/projects/lantern-keep/proposals/PR-0001") => 404,
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
