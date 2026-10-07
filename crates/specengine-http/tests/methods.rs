//! docs/features/daemon-read.md "Data" ("Fence": another method → 405;
//! "Error body"), iteration 2 of the daemon: a read route answers any
//! method but GET, `HEAD` included, with a 405 whose `Allow` is `GET` and
//! whose message is `method <M> is not served on <path>: only GET is
//! served here`; the decision route answers any method but POST with a 405
//! whose `Allow` is `POST` and whose message says decisions are made on a
//! terminal; an unknown path is a 404 for every method, no `Allow`; the
//! fence's 403 carries no `Allow` whatever the method. None of them runs a
//! call: a fresh `HOME` stays empty. M: `.head(only_get)` dropped (HEAD
//! served by GET); the two 405 texts swapped; the fence back as a `layer`
//! on the routes' router (axum adds its `Allow` to the fence's 403); the
//! routes' fallback dropped (axum's bare 404).

mod common;

use common::{Reply, Scratch, Server, request_bytes, snapshot};

/// Every read route of A, with a valid query (`graph`, `check`:
/// docs/features/ui-live.md "Description and interactions"; `tasks`,
/// `tasks/:id`: docs/features/ui-live-tasks.md "Rules and edge cases").
const READS: [&str; 12] = [
    "/api/projects",
    "/api/projects/lantern-keep/tree",
    "/api/projects/lantern-keep/nodes/MEC-STAMINA",
    "/api/projects/lantern-keep/search?query=stamina",
    "/api/projects/lantern-keep/bundle?node_ids=MEC-STAMINA",
    "/api/projects/lantern-keep/graph?ref=MEC-STAMINA",
    "/api/projects/lantern-keep/check",
    "/api/projects/lantern-keep/inbox",
    "/api/projects/lantern-keep/proposals/PR-0001",
    "/api/projects/lantern-keep/tasks",
    "/api/projects/lantern-keep/tasks/T-0001",
    "/api/projects/lantern-keep/events",
];

const DECISION: &str = "/api/projects/lantern-keep/proposals/PR-0001/decision";

/// Paths no route serves (`tasks/:id/transition`, 07 s3, stays one:
/// docs/features/ui-live-tasks.md "Rules and edge cases").
const UNKNOWN: [&str; 12] = [
    "/",
    "/api",
    "/no/such/route",
    "/api/projects/",
    "/api/projects/lantern-keep",
    "/api/projects/lantern-keep/nope",
    "/api/projects/lantern-keep/proposals/PR-0001/decision/more",
    "/api/projects/lantern-keep/tree/more",
    "/api/projects/lantern-keep/check/more",
    "/api/projects/lantern-keep/graph/MEC-STAMINA",
    "/api/projects/lantern-keep/tasks/",
    "/api/projects/lantern-keep/tasks/T-0001/transition",
];

/// Every method but GET and POST.
const OTHERS: [&str; 6] = ["HEAD", "PUT", "DELETE", "PATCH", "OPTIONS", "TRACE"];

/// The path part of `target` (no query): what the message names.
fn path_of(target: &str) -> &str {
    target.split_once('?').map_or(target, |(path, _)| path)
}

/// A 405 with `Allow: allow`, `no-store`, no `Access-Control-*`, and (but
/// for HEAD, whose answer has no body) the error body with `message`.
fn assert_405(reply: &Reply, method: &str, allow: &str, message: &str, context: &str) {
    assert_eq!(reply.status, 405, "{context}: {:?}", reply);
    let allows: Vec<&str> = reply
        .headers
        .iter()
        .filter(|(name, _)| name == "allow")
        .map(|(_, value)| value.as_str())
        .collect();
    assert_eq!(allows, [allow], "{context}: the one `Allow`");
    assert_eq!(reply.header("cache-control"), Some("no-store"), "{context}");
    assert!(reply.cors_headers().is_empty(), "{context}: {:?}", reply);
    if method == "HEAD" {
        assert!(
            reply.body.is_empty(),
            "{context}: a HEAD answer has no body"
        );
    } else {
        assert_eq!(reply.error_message(), message, "{context}");
    }
}

#[test]
fn a_read_route_answers_every_other_method_405_allow_get_head_included() {
    let scratch = Scratch::new("methods-read");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    for target in READS {
        for method in OTHERS {
            let reply = server.request(method, target, &[]);
            let context = format!("{method} {target}");
            let message = format!(
                "method {method} is not served on {}: only GET is served here",
                path_of(target)
            );
            assert_405(&reply, method, "GET", &message, &context);
        }
    }
    assert!(
        snapshot(&home).is_empty(),
        "a 405 runs no call: the fresh HOME holds {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
    // GET still answers (the control): the route is served.
    let reply = server.get("/api/projects/lantern-keep/tree");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.header("allow"), None, "a 200 carries no `Allow`");
}

#[test]
fn the_decision_route_answers_every_other_method_405_allow_post() {
    let scratch = Scratch::new("methods-decision");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    for method in ["GET", "HEAD", "PUT", "DELETE", "PATCH", "OPTIONS", "TRACE"] {
        let reply = server.request(method, DECISION, &[]);
        let message = format!(
            "method {method} is not served on {DECISION}: this path takes only POST \
             (refused): decisions are made on a terminal"
        );
        assert_405(
            &reply,
            method,
            "POST",
            &message,
            &format!("{method} {DECISION}"),
        );
    }
    // POST: the 403 naming the terminal command, no `Allow`.
    let reply = server.request("POST", DECISION, &[]);
    assert_eq!(reply.status, 403, "{}", reply.text());
    assert_eq!(reply.header("allow"), None, "the POST's 403: {:?}", reply);
    assert!(
        reply.error_message().contains("`spec approve PR-0001`"),
        "{}",
        reply.text()
    );
    assert!(snapshot(&home).is_empty(), "a 405 or the POST runs no call");
}

#[test]
fn an_unknown_path_is_a_404_for_every_method() {
    let scratch = Scratch::new("methods-unknown");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let mut methods = vec!["GET", "POST"];
    methods.extend_from_slice(&OTHERS);
    for path in UNKNOWN {
        for method in &methods {
            let reply = server.request(method, path, &[]);
            let context = format!("{method} {path}");
            assert_eq!(reply.status, 404, "{context}: {:?}", reply);
            assert_eq!(
                reply.header("allow"),
                None,
                "{context}: no `Allow` on a 404"
            );
            assert_eq!(reply.header("cache-control"), Some("no-store"), "{context}");
            if *method == "HEAD" {
                assert!(reply.body.is_empty(), "{context}");
                continue;
            }
            let message = reply.error_message();
            assert!(
                message.starts_with(&format!("no route {path}: the routes are /api/projects ")),
                "{context}: {message}"
            );
        }
    }
    // The asterisk form: no route either.
    let host = format!("127.0.0.1:{}", server.port);
    let reply = server.raw(&request_bytes("OPTIONS", "*", &[("Host", &host)]));
    assert_eq!(reply.status, 404, "OPTIONS *: {:?}", reply);
    assert_eq!(reply.header("allow"), None, "OPTIONS *");
    reply.error_message();
    assert!(snapshot(&home).is_empty(), "a 404 of no route runs no call");
}

#[test]
fn the_fences_403_carries_no_allow_whatever_the_method() {
    let scratch = Scratch::new("methods-fence");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let evil = format!("evil.example:{}", server.port);
    let own = format!("127.0.0.1:{}", server.port);
    let foreign: [Vec<(&str, &str)>; 3] = [
        vec![("Host", evil.as_str())],
        vec![("Host", own.as_str()), ("Origin", "http://evil.example")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "cross-site")],
    ];
    let mut targets: Vec<&str> = READS.to_vec();
    targets.push(DECISION);
    targets.push("/no/such/route");
    for headers in &foreign {
        for target in &targets {
            for method in ["GET", "POST", "HEAD", "PUT", "DELETE", "PATCH", "OPTIONS"] {
                let reply = server.raw(&request_bytes(method, target, headers));
                let context = format!("{method} {target} {headers:?}");
                assert_eq!(reply.status, 403, "{context}: {:?}", reply);
                assert_eq!(
                    reply.header("allow"),
                    None,
                    "{context}: the fence's 403 carries no `Allow`: {:?}",
                    reply.headers
                );
                assert_eq!(reply.header("cache-control"), Some("no-store"), "{context}");
                assert!(reply.cors_headers().is_empty(), "{context}");
                if method != "HEAD" {
                    let message = reply.error_message();
                    assert!(message.starts_with("refused: "), "{context}: {message}");
                }
            }
        }
    }
    assert!(
        snapshot(&home).is_empty(),
        "a refused request read nothing: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
}
