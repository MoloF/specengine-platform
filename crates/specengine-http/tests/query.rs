//! docs/features/daemon-read.md "Data" "Query", iteration 2 of the daemon:
//! the raw query string is decoded strictly, as a form. A `%` not followed
//! by two hex digits, or bytes that are not UTF-8 once decoded, in a value
//! or in a name, are a 400 naming the parameter (the raw name when the
//! name is the one that fails), before any call (a fresh `HOME` stays
//! empty); never passed on as written or as U+FFFD. A `+` is a space, a
//! `%2B` a plus: `C%2B%2B` reaches the CLI as `C++`, `stamina+regen` as
//! `stamina regen`. M: lossy UTF-8 decoding; a bad `%` passed on as
//! written; `+` left a plus.

mod common;

use common::{Scratch, Server, snapshot, spec_json};
use serde_json::Value;

const P: &str = "/api/projects/lantern-keep";

#[test]
fn a_bad_escape_or_non_utf8_in_a_query_value_or_name_is_a_400_naming_it() {
    let scratch = Scratch::new("query-strict");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    // (target, the name the message gives in backticks, the raw piece it
    // quotes, why).
    let hex = "is not followed by two hex digits";
    let utf8 = "it is not UTF-8";
    let cases: Vec<(String, &str, &str, &str)> = vec![
        (format!("{P}/search?query=%FF"), "query", "%FF", utf8),
        (
            format!("{P}/search?query=st%zzamina"),
            "query",
            "st%zzamina",
            hex,
        ),
        (
            format!("{P}/search?query=stamina%"),
            "query",
            "stamina%",
            hex,
        ),
        (
            format!("{P}/search?query=stamina%4"),
            "query",
            "stamina%4",
            hex,
        ),
        (format!("{P}/search?query=%E2%82"), "query", "%E2%82", utf8),
        (format!("{P}/tree?kinds=%C3%28"), "kinds", "%C3%28", utf8),
        (format!("{P}/tree?depth=%31%"), "depth", "%31%", hex),
        (
            format!("{P}/bundle?node_ids=MEC-STAMINA&node_ids=%80"),
            "node_ids",
            "%80",
            utf8,
        ),
        (
            format!("{P}/nodes/MEC-STAMINA?with=%FF"),
            "with",
            "%FF",
            utf8,
        ),
        (
            format!("{P}/inbox?archive=%ED%A0%80"),
            "archive",
            "%ED%A0%80",
            utf8,
        ),
        (
            format!("{P}/search?%FFquery=stamina"),
            "%FFquery",
            "%FFquery",
            utf8,
        ),
        (
            format!("{P}/search?qu%zzery=stamina"),
            "qu%zzery",
            "qu%zzery",
            hex,
        ),
        (
            format!("{P}/search?query%=stamina"),
            "query%",
            "query%",
            hex,
        ),
        ("/api/projects?%FF=1".to_owned(), "%FF", "%FF", utf8),
        (
            format!("{P}/proposals/PR-0001?%C0%AF"),
            "%C0%AF",
            "%C0%AF",
            utf8,
        ),
    ];
    for (target, name, raw, why) in &cases {
        let reply = server.get(target);
        assert_eq!(reply.status, 400, "{target}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(&format!("`{name}`")) && message.contains(raw),
            "{target}: the message names `{name}` and quotes {raw:?}: {message:?}"
        );
        assert!(message.contains(why), "{target}: {message:?} says {why:?}");
        assert!(
            !message.contains('\u{FFFD}'),
            "{target}: no replacement character: {message:?}"
        );
    }
    // The tail's query too: a 400, not a stream.
    let reply = server.request_within(
        "GET",
        &format!("{P}/events?%FF=1"),
        &[],
        std::time::Duration::from_secs(20),
    );
    assert_eq!(reply.status, 400, "events?%FF=1: {}", reply.text());
    let message = reply.error_message();
    assert!(message.contains("`%FF`"), "{message:?}");
    assert!(
        snapshot(&home).is_empty(),
        "a refused query ran no call: the fresh HOME holds {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
}

/// `query` of the daemon's search and of the CLI's, and their hits' IDs.
fn searched(server: &Server, target: &str) -> (Value, Vec<Value>) {
    let reply = server.get(target);
    assert_eq!(reply.status, 200, "{target}: {}", reply.text());
    let document = reply.json();
    let ids = document["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|hit| hit["id"].clone())
        .collect();
    (document["query"].clone(), ids)
}

#[test]
fn a_plus_is_a_space_and_an_encoded_plus_a_plus() {
    let scratch = Scratch::new("query-plus");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a]);
    for (target, text) in [
        (format!("{P}/search?query=C%2B%2B"), "C++"),
        (format!("{P}/search?query=stamina+regen"), "stamina regen"),
        (format!("{P}/search?query=stamina%20regen"), "stamina regen"),
        (format!("{P}/search?query=stamina%2Bregen"), "stamina+regen"),
        (
            format!("{P}/search?query=lantern+%2B+keep"),
            "lantern + keep",
        ),
        (
            format!("{P}/search?query=na%C3%AFve+stamina"),
            "na\u{ef}ve stamina",
        ),
        (format!("{P}/search?&query=stamina&"), "stamina"),
    ] {
        let run = spec_json(&home, &cwd, &a, &["search", text]);
        run.code(0);
        let cli = run.json();
        let ids: Vec<Value> = cli["hits"]
            .as_array()
            .expect("hits")
            .iter()
            .map(|hit| hit["id"].clone())
            .collect();
        let (query, hits) = searched(&server, &target);
        assert_eq!(
            query,
            Value::from(text),
            "{target} reaches the CLI as {text:?}"
        );
        assert_eq!(query, cli["query"], "{target}");
        assert_eq!(hits, ids, "{target}: the CLI's hits");
    }
    // Two terms, not one: `stamina+regen` finds what `stamina regen` does,
    // which is not what the one term `stamina+regen` finds.
    let (_, spaced) = searched(&server, &format!("{P}/search?query=stamina+regen"));
    let (_, plus) = searched(&server, &format!("{P}/search?query=stamina%2Bregen"));
    assert!(!spaced.is_empty(), "`stamina regen` has hits in A");
    assert_ne!(spaced, plus, "a space and a plus search differently in A");
}
