//! docs/features/workspaces.md (RM-0081, v4 step 1a) "Acceptance criteria",
//! the daemon's half on a local daemon: `GET /api/projects/:p/workspaces`, the
//! plain `spec workspace --json`, over git copies of `fixtures/spec-a` (a
//! game's brief, no table) and `fixtures/card-press` (a print shop, the
//! example tables), so one route serves projects of different nature
//! (ADR-0008). Every expected value is worked out from the spec's "Data" and
//! "The daemon", never read back from a run.
//!
//! - WS-1: spec-a's route answers the implicit document, byte-equal to the
//!   CLI's; card-press's the document of "Data". The route reads the config
//!   alone: a fresh `HOME` holds nothing past the daemon's own files.
//! - WS-4: each of (a) to (g), written under a running daemon, is a 200
//!   byte-equal to the CLI's JSON, and `GET /api/projects` is byte-equal to the
//!   same copy's with both tables removed. M7: a malformed table refused by
//!   `ProjectConfig` (`/api/projects` 503).
//! - WS-8, a daemon's start: card-press with its tables starts; `[[workspaces]]`
//!   refuses the start, exit 2, at its line with the TOML reader's message.
//!   M18.
//! - WS-12: a query name is a 400 naming it, a bare `?` none; HEAD, POST and
//!   every other method a 405 `Allow: GET`; a slug `ProjectConfig` refuses a
//!   503 with the CLI's line, answered again once mended.
//!
//! The turn is `tests/turn.rs`'s, the hub's `tests/hub.rs`'s (WS-10), the key
//! sets `tests/daemon_keys.rs`'s.

mod common;

#[path = "../../specengine-cli/tests/workspaces_common/mod.rs"]
mod workspaces_common;

use std::path::Path;

use common::{
    Reply, Run, Scratch, Server, beyond_daemon, read_text, root_args, spec, spec_json, write,
};
use workspaces_common::{
    CONFIG, IMPLICIT_JSON, PRESS_JSON, line_holding, misspelt, variants, without_tables,
};

/// card-press's route.
const PRESS: &str = "/api/projects/card-press/workspaces";

/// `spec --root <root> workspace --json` under its own `HOME`: exit 0.
fn cli(home: &Path, cwd: &Path, root: &Path) -> Run {
    let run = spec_json(home, cwd, root, &["workspace"]);
    run.code(0);
    run
}

/// A 200 of the route: the JSON headers, `no-store`, the same bytes again.
fn answered(server: &Server, path: &str) -> Reply {
    let reply = server.get(path);
    assert_eq!(reply.status, 200, "{path}: {}", reply.text());
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8"),
        "{path}"
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
    let again = server.get(path);
    assert_eq!(
        (again.status, again.text()),
        (200, reply.text()),
        "{path}: a repeat answers the same bytes"
    );
    reply
}

/// WS-1: spec-a's route is the implicit document of "Data", byte-equal to the
/// CLI's; card-press's the document of "Data". A fresh `HOME` holds nothing
/// but the daemon's own files after: the route opens no index (W3).
#[test]
fn ws1_the_route_is_the_cli_s_document() {
    let scratch = Scratch::new("workspaces-route");
    let a = scratch.repo("spec-a", "a", "main");
    let press = scratch.repo("card-press", "press", "main");
    let home = scratch.home("fresh");
    let cli_home = scratch.home("cli");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a, &press]);

    let reply = answered(&server, "/api/projects/lantern-keep/workspaces");
    assert_eq!(reply.text(), IMPLICIT_JSON);
    assert_eq!(reply.text(), cli(&cli_home, &cwd, &a).document());

    let reply = answered(&server, PRESS);
    assert_eq!(reply.text(), PRESS_JSON);
    assert_eq!(reply.text(), cli(&cli_home, &cwd, &press).document());

    assert!(
        beyond_daemon(&home).is_empty(),
        "the route reads the config alone: the fresh HOME holds {:?}",
        beyond_daemon(&home)
    );
}

/// WS-4: each variant written under a running daemon over a git copy of
/// card-press: the route is a 200 byte-equal to `spec workspace --json` of the
/// same copy; `GET /api/projects` is a 200 byte-equal to the copy's with both
/// tables removed. M7.
#[test]
fn ws4_every_variant_is_the_cli_s_bytes_and_projects_stand() {
    let scratch = Scratch::new("workspaces-variants");
    let press = scratch.repo("card-press", "press", "main");
    let home = scratch.home("h");
    let cli_home = scratch.home("cli");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&press]);
    let base = read_text(&press, CONFIG);
    for (label, changed) in
        std::iter::once(("the tables as committed", base.clone())).chain(variants(&base))
    {
        write(&press, CONFIG, &changed);
        let reply = answered(&server, PRESS);
        assert_eq!(
            reply.text(),
            cli(&cli_home, &cwd, &press).document(),
            "{label}: the CLI's bytes"
        );
        let projects = server.get("/api/projects");
        assert_eq!(projects.status, 200, "{label}: {}", projects.text());
        write(&press, CONFIG, without_tables(&changed));
        let bare = server.get("/api/projects");
        assert_eq!(bare.status, 200, "{label}: {}", bare.text());
        assert_eq!(
            projects.text(),
            bare.text(),
            "{label}: /api/projects as without the tables"
        );
    }
}

/// WS-8, a daemon's start: card-press with its tables starts and lists it;
/// the first `[[workspace]]` misspelt refuses the start: exit 2 before
/// listening, stdout empty, one `specengine-http: ` line naming
/// `specengine.toml:<line>: ` and the TOML reader's unknown-field message. M18.
#[test]
fn ws8_a_daemon_starts_with_the_tables_and_refuses_a_misspelt_one() {
    let scratch = Scratch::new("workspaces-start");
    let press = scratch.repo("card-press", "press", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&press]);
    assert_eq!(
        server.lines[0],
        format!("serving card-press {}", press.display())
    );
    drop(server);

    let text = misspelt(&read_text(&press, CONFIG));
    write(&press, CONFIG, &text);
    let line = line_holding(&text, "[[workspaces]]");
    let refused = match Server::try_start(&home, &cwd, &root_args(&[&press])) {
        Ok(server) => panic!("a misspelt table started a daemon: {:?}", server.lines),
        Err(refused) => refused,
    };
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stdout.is_empty(), "{refused:?}");
    assert!(
        refused.stderr.starts_with("specengine-http: ")
            && refused.stderr.contains(&format!(
                "{CONFIG}:{line}: unknown field `workspaces`, expected one of "
            )),
        "{refused:?}"
    );
}

/// Every method but GET.
const OTHERS: [&str; 7] = ["HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "TRACE"];

/// WS-12: any query name is a 400 naming it (a bare `?` is none); every other
/// method, HEAD and POST among them, a 405 whose one `Allow` is `GET`. M26's
/// sibling: a query handed to the CLI.
#[test]
fn ws12_the_route_takes_no_query_and_get_alone() {
    let scratch = Scratch::new("workspaces-reach");
    let press = scratch.repo("card-press", "press", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&press]);

    for (query, name) in [
        ("x=1", "x"),
        ("json=true", "json"),
        ("config=other.toml", "config"),
        ("project=card-press", "project"),
        ("x", "x"),
    ] {
        let path = format!("{PRESS}?{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(&format!("`{name}`")) && message.contains("no query name"),
            "{path}: names `{name}`: {message}"
        );
    }
    let bare = answered(&server, &format!("{PRESS}?"));
    assert_eq!(bare.text(), PRESS_JSON, "a bare `?` is no query");

    for method in OTHERS {
        let reply = server.request(method, PRESS, &[]);
        let context = format!("{method} {PRESS}");
        assert_eq!(reply.status, 405, "{context}: {reply:?}");
        let allows: Vec<&str> = reply
            .headers
            .iter()
            .filter(|(name, _)| name == "allow")
            .map(|(_, value)| value.as_str())
            .collect();
        assert_eq!(allows, ["GET"], "{context}: the one `Allow`");
        if method != "HEAD" {
            assert_eq!(
                reply.error_message(),
                format!("method {method} is not served on {PRESS}: only GET is served here"),
                "{context}"
            );
        }
    }
    assert_eq!(answered(&server, PRESS).header("allow"), None);
}

/// WS-12: a `[project]` slug `ProjectConfig` refuses, written under a running
/// daemon, makes the route a 503 whose message is the CLI's line (exit 2);
/// the route answers again once the config is mended.
#[test]
fn ws12_a_refused_config_is_a_503_with_the_cli_s_line() {
    let scratch = Scratch::new("workspaces-refused");
    let press = scratch.repo("card-press", "press", "main");
    let home = scratch.home("h");
    let cli_home = scratch.home("cli");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&press]);
    answered(&server, PRESS);

    let good = read_text(&press, CONFIG);
    write(
        &press,
        CONFIG,
        good.replacen("slug = \"card-press\"", "slug = \"Card Press\"", 1),
    );
    let root = press.to_str().expect("UTF-8 root");
    let run = spec(&cli_home, &cwd, &["--root", root, "workspace", "--json"]);
    run.code(2);
    let line = run.stderr.trim_end_matches('\n').to_owned();
    assert!(line.starts_with(&format!("{CONFIG}:")), "{}", run.show());
    let reply = server.get(PRESS);
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line, "the CLI's line");

    write(&press, CONFIG, &good);
    assert_eq!(answered(&server, PRESS).text(), PRESS_JSON);
}
