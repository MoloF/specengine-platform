//! docs/features/daemon-read.md "Data" ("Fence": another method → 405;
//! "Error body"), iteration 2 of the daemon: a read route answers any
//! method but GET, `HEAD` included, with a 405 whose `Allow` is `GET` and
//! whose message is `method <M> is not served on <path>: only GET is
//! served here`; the decision route (the stage route since
//! docs/features/decision-staging.md "Daemon") answers any method but POST
//! and DELETE with a 405 whose `Allow` is `POST, DELETE` and whose message
//! says a staged choice is confirmed on a terminal; its POST and DELETE
//! without `Sec-Fetch-Site: same-origin` are a 403, no `Allow`; an unknown
//! path is a 404 for every method, no `Allow`; the fence's 403 carries no
//! `Allow` whatever the method. None of them runs a call: a fresh `HOME`
//! stays empty. M: `.head(only_get)` dropped (HEAD served by GET); the two
//! 405 texts swapped; the fence back as a `layer` on the routes' router
//! (axum adds its `Allow` to the fence's 403); the routes' fallback dropped
//! (axum's bare 404).
//!
//! R-03 of docs/features/ui-round.md: `round` is a read (HEAD, POST, PUT
//! and every other method: 405 `Allow: GET`); `round/answers` answers any
//! method but POST, GET, HEAD, PUT and DELETE among them, with a 405 whose
//! `Allow` is `POST` and whose message ends `confirmed on a terminal`; its
//! POST without `Sec-Fetch-Site: same-origin` is the stage route's 403.

mod common;

use common::{Reply, Scratch, Server, beyond_daemon, request_bytes};

/// Every read route of A, with a valid query (`graph`, `check`:
/// docs/features/ui-live.md "Description and interactions"; `tasks`,
/// `tasks/:id`: docs/features/ui-live-tasks.md "Rules and edge cases";
/// `round`: docs/features/ui-round.md "Data").
const READS: [&str; 13] = [
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
    "/api/projects/lantern-keep/round",
    "/api/projects/lantern-keep/events",
];

const DECISION: &str = "/api/projects/lantern-keep/proposals/PR-0001/decision";

/// The round's answers (docs/features/ui-round.md "Data").
const ROUND_ANSWERS: &str = "/api/projects/lantern-keep/round/answers";

/// Paths no route serves (`tasks/:id/transition`, 07 s3, stays one:
/// docs/features/ui-live-tasks.md "Rules and edge cases").
const UNKNOWN: [&str; 15] = [
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
    "/api/projects/lantern-keep/round/",
    "/api/projects/lantern-keep/round/answers/more",
    "/api/projects/lantern-keep/round/answers/PR-0001",
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
        beyond_daemon(&home).is_empty(),
        "a 405 runs no call: the fresh HOME holds {:?}",
        beyond_daemon(&home)
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
    // A same-origin page's header too: the method decides, not the page.
    for headers in [&[][..], &[("Sec-Fetch-Site", "same-origin")][..]] {
        for method in ["GET", "HEAD", "PUT", "PATCH", "OPTIONS", "TRACE"] {
            let reply = server.request(method, DECISION, headers);
            let message = format!(
                "method {method} is not served on {DECISION}: this path takes only POST \
                 (stage a choice) and DELETE (unstage it); a staged choice is confirmed on a \
                 terminal"
            );
            assert_405(
                &reply,
                method,
                "POST, DELETE",
                &message,
                &format!("{method} {DECISION} {headers:?}"),
            );
        }
    }
    // POST and DELETE from no page (or the address bar's `none`): the 403
    // that says it is no authentication, no `Allow`.
    for method in ["POST", "DELETE"] {
        for headers in [&[][..], &[("Sec-Fetch-Site", "none")][..]] {
            let reply = server.request(method, DECISION, headers);
            assert_eq!(reply.status, 403, "{method} {headers:?}: {}", reply.text());
            assert_eq!(reply.header("allow"), None, "the {method}'s 403: {reply:?}");
            assert!(reply.cors_headers().is_empty(), "{reply:?}");
            assert_eq!(
                reply.error_message(),
                "staging needs a same-origin page (not authentication: ADR-0034)",
                "{method} {headers:?}"
            );
        }
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "a 405 or a refused POST, DELETE runs no call"
    );
}

#[test]
fn the_round_routes_answer_another_method_405_allow_get_on_the_sheet_post_on_the_answers() {
    let scratch = Scratch::new("methods-round");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    // The sheet is a read: HEAD, PUT and POST (a same-origin page's too,
    // with a JSON body) are 405 `Allow: GET`.
    let sheet = "/api/projects/lantern-keep/round";
    let page = [
        ("Sec-Fetch-Site", "same-origin"),
        ("Content-Type", "application/json"),
    ];
    for headers in [&[][..], &page[..]] {
        for method in ["HEAD", "POST", "PUT"] {
            let body: &[u8] = if method == "HEAD" { b"" } else { b"{}" };
            let reply = server.send(method, sheet, headers, body);
            let message =
                format!("method {method} is not served on {sheet}: only GET is served here");
            assert_405(
                &reply,
                method,
                "GET",
                &message,
                &format!("{method} {sheet} {headers:?}"),
            );
        }
    }
    for headers in [&[][..], &[("Sec-Fetch-Site", "same-origin")][..]] {
        for method in ["GET", "HEAD", "PUT", "DELETE", "PATCH", "OPTIONS", "TRACE"] {
            let reply = server.request(method, ROUND_ANSWERS, headers);
            let context = format!("{method} {ROUND_ANSWERS} {headers:?}");
            assert_eq!(reply.status, 405, "{context}: {reply:?}");
            let allows: Vec<&str> = reply
                .headers
                .iter()
                .filter(|(name, _)| name == "allow")
                .map(|(_, value)| value.as_str())
                .collect();
            assert_eq!(allows, ["POST"], "{context}: the one `Allow`");
            assert_eq!(reply.header("cache-control"), Some("no-store"), "{context}");
            assert!(reply.cors_headers().is_empty(), "{context}: {reply:?}");
            if method == "HEAD" {
                assert!(reply.body.is_empty(), "{context}");
                continue;
            }
            let message = reply.error_message();
            assert!(
                message.starts_with(&format!(
                    "method {method} is not served on {ROUND_ANSWERS}: this path takes only POST ("
                )),
                "{context}: {message}"
            );
            assert!(
                message.ends_with("; a staged choice is confirmed on a terminal"),
                "{context}: {message}"
            );
        }
    }
    // POST from no page (or the address bar's `none`): the stage route's
    // 403, no `Allow`.
    for headers in [&[][..], &[("Sec-Fetch-Site", "none")][..]] {
        let reply = server.request("POST", ROUND_ANSWERS, headers);
        assert_eq!(reply.status, 403, "POST {headers:?}: {}", reply.text());
        assert_eq!(reply.header("allow"), None, "{reply:?}");
        assert_eq!(
            reply.error_message(),
            "staging needs a same-origin page (not authentication: ADR-0034)"
        );
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "a 405 or a refused POST runs no call: {:?}",
        beyond_daemon(&home)
    );
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
    assert!(
        beyond_daemon(&home).is_empty(),
        "a 404 of no route runs no call"
    );
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
    targets.push(ROUND_ANSWERS);
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
        beyond_daemon(&home).is_empty(),
        "a refused request read nothing: {:?}",
        beyond_daemon(&home)
    );
}
