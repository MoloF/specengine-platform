//! crates/specengine-http/README.md "Endpoints", Query, iteration 2 of the daemon:
//! the raw query string is decoded strictly, as a form. A `%` not followed
//! by two hex digits, or bytes that are not UTF-8 once decoded, in a value
//! or in a name, are a 400 naming the parameter (the raw name when the
//! name is the one that fails), before any call (a fresh `HOME` stays
//! empty); never passed on as written or as U+FFFD. A `+` is a space, a
//! `%2B` a plus: `C%2B%2B` reaches the CLI as `C++`, `stamina+regen` as
//! `stamina regen`. M: lossy UTF-8 decoding; a bad `%` passed on as
//! written; `+` left a plus.
//!
//! docs/features/show-bindings.md RD-7 "HTTP":
//! `nodes_with_bindings_equals_show_bindings` (`with=bindings`, with
//! `with=links` in either order, and the 400 of any other value).
//!
//! docs/features/board-ui.md "Data": `cards` takes `flow`, `column`,
//! `discipline`, `assignee`, `no_target`, `with_cancelled`, decoded as every
//! query (a bad `%`, bytes that are not UTF-8: a 400 naming the parameter; a
//! `+` a space, a `%2B` a plus), a scalar once, a boolean `true` or `false`;
//! `cards/:id` takes no query name. Each refusal runs no call.
//!
//! docs/features/board-handoff.md BH-19: `parent` joins the names, last; a
//! second `parent` is a 400; `?parent=C` lists exactly C's children.

mod common;

use common::{Scratch, Server, beyond_daemon, spec_json};
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
        (format!("{P}/cards?flow=%FF"), "flow", "%FF", utf8),
        (format!("{P}/cards?column=%E2%82"), "column", "%E2%82", utf8),
        (
            format!("{P}/cards?discipline=co%zzpy"),
            "discipline",
            "co%zzpy",
            hex,
        ),
        (format!("{P}/cards?assignee=ana%"), "assignee", "ana%", hex),
        (
            format!("{P}/cards?with_cancelled=%C3%28"),
            "with_cancelled",
            "%C3%28",
            utf8,
        ),
        (format!("{P}/cards?no_target=%4"), "no_target", "%4", hex),
        (
            format!("{P}/cards?%FFflow=copy"),
            "%FFflow",
            "%FFflow",
            utf8,
        ),
        (format!("{P}/cards/C-0001?%FF=1"), "%FF", "%FF", utf8),
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
        beyond_daemon(&home).is_empty(),
        "a refused query ran no call: the fresh HOME holds {:?}",
        beyond_daemon(&home)
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

/// docs/features/board-ui.md "Data": the board's filters as the shared
/// `Args` read them, each refusal a 400 before any call (a fresh `HOME`
/// stays empty).
#[test]
fn a_card_query_outside_its_names_or_values_is_a_400_before_any_call() {
    let scratch = Scratch::new("query-cards-refused");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    // docs/canon/card-flows.md "Children": `parent`, last.
    let names = "flow, column, discipline, assignee, no_target, with_cancelled, marks, parent";
    let cases: Vec<(String, String)> = vec![
        (
            format!("{P}/cards?limit=1"),
            format!("`limit` is no query name of cards: it takes {names}"),
        ),
        (
            format!("{P}/cards?status=draft"),
            format!("`status` is no query name of cards: it takes {names}"),
        ),
        (
            format!("{P}/cards?flow=copy&flow=plates"),
            "`flow` is given 2 times: it takes one value".to_owned(),
        ),
        (
            format!("{P}/cards?assignee=ana&assignee=ben"),
            "`assignee` is given 2 times: it takes one value".to_owned(),
        ),
        (
            format!("{P}/cards?with_cancelled=1"),
            "`with_cancelled=1`: not `true` or `false`".to_owned(),
        ),
        (
            format!("{P}/cards?with_cancelled"),
            "`with_cancelled=`: not `true` or `false`".to_owned(),
        ),
        (
            format!("{P}/cards?no_target=yes"),
            "`no_target=yes`: not `true` or `false`".to_owned(),
        ),
        (
            format!("{P}/cards/C-0001?history=true"),
            "`history` is no query name of cards/:id: it takes no query name".to_owned(),
        ),
    ];
    for (target, message) in &cases {
        let reply = server.get(target);
        assert_eq!(reply.status, 400, "{target}: {}", reply.text());
        assert_eq!(&reply.error_message(), message, "{target}");
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "a refused query ran no call: the fresh HOME holds {:?}",
        beyond_daemon(&home)
    );
}

/// docs/features/board-ui.md "Data": a filter is the CLI's flag, decoded as a
/// form: `+` a space, `%2B` a plus, so `ana%2Bben` finds the name written
/// with a plus and `ana+ben` finds nobody; filters AND; `false` is the
/// flag left out.
#[test]
fn the_card_filters_decode_as_a_form_and_are_the_clis_flags() {
    let scratch = Scratch::new("query-cards");
    let root = scratch.repo("card-press", "press", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let base = "/api/projects/card-press";
    for (title, discipline, assignee) in [
        ("Lead story", "copy", "ana+ben"),
        ("Back page", "copy", "cid"),
        ("Front plate", "plates", "cid"),
    ] {
        let run = spec_json(
            &home,
            &cwd,
            &root,
            &[
                "card",
                "new",
                "--title",
                title,
                "--discipline",
                discipline,
                "--assignee",
                assignee,
            ],
        );
        run.code(0);
    }
    let ids = |target: &str| -> Vec<String> {
        let reply = server.get(&format!("{base}/{target}"));
        assert_eq!(reply.status, 200, "{target}: {}", reply.text());
        reply.json()["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .map(|row| row["id"].as_str().expect("an ID").to_owned())
            .collect()
    };
    assert_eq!(ids("cards?assignee=ana%2Bben"), ["C-0001"]);
    assert_eq!(ids("cards?assignee=ana+ben"), Vec::<String>::new());
    assert_eq!(ids("cards?assignee=cid"), ["C-0002", "C-0003"]);
    assert_eq!(ids("cards?assignee=cid&discipline=copy"), ["C-0002"]);
    assert_eq!(ids("cards?assignee=cid&flow=plates"), ["C-0003"]);
    assert_eq!(
        ids("cards?flow=copy&discipline=plates"),
        Vec::<String>::new()
    );
    assert_eq!(ids("cards?column=draft"), ["C-0001", "C-0002"]);
    assert_eq!(ids("cards?no_target=true"), ["C-0001", "C-0002", "C-0003"]);
    assert_eq!(
        ids("cards?no_target=false&with_cancelled=false"),
        ["C-0001", "C-0002", "C-0003"]
    );
    // Exact, never a part of the word.
    assert_eq!(ids("cards?assignee=ci"), Vec::<String>::new());
}

/// A git repository of `fixtures/code-a` with the golden lock committed
/// (`fixtures/spec-lock/code-a.lock`), then LAW-BOUND's text edited
/// (uncommitted): rows judged, some drifted.
fn code_a_repo(scratch: &Scratch) -> std::path::PathBuf {
    let root = scratch.copy("code-a", "code");
    let _ = std::fs::remove_file(root.join("expected.json"));
    std::fs::copy(
        common::fixture("spec-lock").join("code-a.lock"),
        root.join("spec.lock"),
    )
    .expect("the golden lock");
    let git = scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "code-a and its lock"]);
    common::replace(
        &root,
        "docs/spec/counter.md",
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    root
}

/// docs/features/show-bindings.md RD-7 "HTTP": `GET
/// /api/projects/<slug>/nodes/{*ref}?with=bindings`, repeatable with
/// `with=links` in either order, is the CLI's `spec --root R show REF
/// --bindings [--links] --json` byte for byte (a document holding its
/// sections' rows, a section, a path, a node without bindings, REF naming
/// nothing as a 404), its rows judged; `with=<other>` is a 400 `` `with=<value>`:
/// the values are `links` and `bindings` `` before any call (a fresh `HOME`
/// stays empty); since docs/features/node-history.md NH12 `` the values are
/// `links`, `bindings` and `history` ``. M: `with=bindings` ignored (its
/// `bindings` `null`).
#[test]
fn nodes_with_bindings_equals_show_bindings() {
    let scratch = Scratch::new("query-bindings");
    let root = code_a_repo(&scratch);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let base = "/api/projects/code-a";
    for reference in [
        "PIECE-COUNTER",
        "LAW-BOUND",
        "docs/spec/counter.md",
        "PIECE-COUNTER#LAW-BOUND",
        "PIECE-LIMITS",
        "LAW-NOPE",
    ] {
        let encoded = common::encode_component(reference);
        for (query, flags) in [
            ("with=bindings", &["--bindings"][..]),
            ("with=links&with=bindings", &["--links", "--bindings"]),
            ("with=bindings&with=links", &["--links", "--bindings"]),
        ] {
            let mut args = vec!["show", reference];
            args.extend_from_slice(flags);
            let run = spec_json(&home, &cwd, &root, &args);
            let reply = server.get(&format!("{base}/nodes/{encoded}?{query}"));
            let want = if run.code == 0 { 200 } else { 404 };
            assert_eq!(reply.status, want, "{reference} {query}: {}", reply.text());
            assert_eq!(
                reply.text(),
                run.document(),
                "{reference} {query}: the CLI's document"
            );
            if run.code == 0 {
                let document = reply.json();
                for node in document["nodes"].as_array().expect("nodes") {
                    let rows = node["bindings"]["rows"]
                        .as_array()
                        .unwrap_or_else(|| panic!("{reference} {query}: rows: {node}"));
                    assert_eq!(node["bindings"]["omitted"], 0, "the daemon's view is uncut");
                    assert!(
                        rows.iter().all(|row| row["sync"].is_string()),
                        "{reference} {query}: every row judged"
                    );
                }
            }
        }
    }
    let piece = server
        .get(&format!("{base}/nodes/PIECE-COUNTER?with=bindings"))
        .json();
    assert!(
        piece["nodes"][0]["bindings"]["rows"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["node"] == "LAW-BOUND")),
        "the document holds its sections' rows: {piece}"
    );
    let plain = server.get(&format!("{base}/nodes/PIECE-COUNTER")).json();
    assert_eq!(
        plain["nodes"][0]["bindings"],
        Value::Null,
        "without with=bindings"
    );

    let fresh = scratch.home("fresh");
    let other = Server::serve(&fresh, &cwd, &[&root]);
    for value in ["tasks", "Bindings", "History", ""] {
        let reply = other.get(&format!("{base}/nodes/LAW-BOUND?with={value}"));
        assert_eq!(reply.status, 400, "with={value}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            format!("`with={value}`: the values are `links`, `bindings` and `history`"),
            "with={value}"
        );
    }
    assert!(
        beyond_daemon(&fresh).is_empty(),
        "a refused query ran no call: the fresh HOME holds {:?}",
        beyond_daemon(&fresh)
    );
}

/// docs/features/board-handoff.md BH-19, the daemon's half: `GET
/// .../cards?parent=C-0001` is `spec card list --parent C-0001`: exactly the
/// card's children, every flow, the CLI's bytes; a card with no child lists
/// none, a 200; filters AND; a second `parent` is a 400 before any call; a
/// look-alike or a `C` off the card-ID grammar cannot run, a 503 with the
/// CLI's line. M22: the filter ignored.
#[test]
fn board_handoff_bh19_parent_lists_its_children() {
    let scratch = Scratch::new("query-cards-parent");
    let root = scratch.repo("studio-lamp", "lamp", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let base = "/api/projects/studio-lamp";
    for args in [
        &["--title", "Lamp room scene", "--discipline", "narrative"][..],
        &[
            "--title",
            "Lamp room dialogue",
            "--discipline",
            "development",
            "--parent",
            "C-0001",
        ][..],
        &[
            "--title",
            "Lamp glow",
            "--discipline",
            "art",
            "--parent",
            "C-0001",
        ][..],
        &[
            "--title",
            "Dialogue lines",
            "--discipline",
            "development",
            "--parent",
            "C-0002",
        ][..],
        &["--title", "Cove scene", "--discipline", "narrative"][..],
    ] {
        let mut all = vec!["card", "new"];
        all.extend_from_slice(args);
        spec_json(&home, &cwd, &root, &all).code(0);
    }
    let ids = |target: &str| -> Vec<String> {
        let reply = server.get(&format!("{base}/{target}"));
        assert_eq!(reply.status, 200, "{target}: {}", reply.text());
        let document = reply.json();
        assert_eq!(
            document["flows"].as_array().map(Vec::len),
            // department-split S9: the preset's six flows.
            Some(6),
            "{target}: every flow"
        );
        document["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .map(|row| row["id"].as_str().expect("an ID").to_owned())
            .collect()
    };
    assert_eq!(ids("cards?parent=C-0001"), ["C-0002", "C-0003"]);
    assert_eq!(ids("cards?parent=C-0002"), ["C-0004"]);
    assert_eq!(ids("cards?parent=C-0005"), Vec::<String>::new());
    assert_eq!(ids("cards?parent=C-9999"), Vec::<String>::new());
    assert_eq!(ids("cards?parent=C-0001&flow=art"), ["C-0003"]);
    assert_eq!(ids("cards").len(), 5, "no parent: every card");
    // The CLI's bytes.
    let reply = server.get(&format!("{base}/cards?parent=C-0001"));
    let run = spec_json(&home, &cwd, &root, &["card", "list", "--parent", "C-0001"]);
    run.code(0);
    assert_eq!(reply.text(), run.document(), "the CLI's document");

    let twice = server.get(&format!("{base}/cards?parent=C-0001&parent=C-0002"));
    assert_eq!(twice.status, 400, "{}", twice.text());
    assert_eq!(
        twice.error_message(),
        "`parent` is given 2 times: it takes one value"
    );
    for (written, encoded) in [
        ("\u{0421}-0001", "%D0%A1-0001"),
        ("C-1", "C-1"),
        ("T-0001", "T-0001"),
    ] {
        let reply = server.get(&format!("{base}/cards?parent={encoded}"));
        assert_eq!(reply.status, 503, "{written:?}: {}", reply.text());
        let root_text = root.to_str().expect("UTF-8 root");
        let run = common::spec(
            &home,
            &cwd,
            &[
                "--root", root_text, "card", "list", "--parent", written, "--json",
            ],
        );
        run.code(2);
        assert_eq!(
            reply.error_message(),
            run.stderr.trim_end_matches('\n'),
            "{written:?}: the CLI's line"
        );
    }
}
