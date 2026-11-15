//! docs/features/auto-feed.md FD3, the daemon's half: `GET
//! /api/projects/:p/feed` over a git copy of `fixtures/auto-mini` with
//! `rules/on.toml` committed and a queue scripted through the binary and the
//! library (tasks, a pack, a hold, a delegated approve, a run reported, an
//! open proposal).
//!
//! - Locally the body is `spec feed --json`'s stdout (its one LF aside) for
//!   the same flags: `since=0&needs_me=1`, `task=T-0001`,
//!   `department=general`, no query.
//! - A second `since`, an unknown key, `needs_me=2`, a bad `since`: 400 with
//!   the error body, nothing changed.
//! - On a hub any login reads it (one outside `[team]` too); its `auto`
//!   head is the hub's surface: a `[team]` of two gives `team`, a rule of
//!   `mode = "local"` `off` with the hub's why while the terminal's is on.
//!
//! M: the hub's head judged as a terminal's.

mod common;

use std::path::{Path, PathBuf};

use common::hub::{ORIGIN, Proxied, events_of, hub_args, through};
use common::{Scratch, Server, read_text, repository_root, run_product, spec, spec_bin, write};
use serde_json::Value;

const SLUG: &str = "auto-mini";

fn rule(name: &str) -> String {
    read_text(&repository_root().join("fixtures/auto-mini/rules"), name)
}

/// The scripted repository: `tables` committed after the fixture's config.
fn scripted(scratch: &Scratch, tables: &[&str]) -> (PathBuf, PathBuf, PathBuf) {
    let root = scratch.repo("auto-mini", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let git = scratch.git();
    git.run(&root, &["config", "user.name", "Owner"]);
    git.run(&root, &["config", "user.email", "owner@example.org"]);
    let mut config = read_text(&root, "specengine.toml").trim_end().to_owned();
    for table in tables {
        config.push_str("\n\n");
        config.push_str(table.trim_end());
    }
    config.push('\n');
    write(&root, "specengine.toml", config);
    git.run(&root, &["commit", "-qam", "the rule"]);
    let plan = scratch.join("plan.md");
    std::fs::write(&plan, "1. Do it.\n").unwrap();
    let plan = plan.to_str().unwrap().to_owned();
    let ok = |dir: &Path, args: &[&str]| {
        let run = spec(&home, dir, args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.show());
    };
    for (task, title) in [("T-0001", "Fix the door timer"), ("T-0002", "Sort")] {
        ok(&root, &["task", "new", "--nodes", "R-02", "--title", title]);
        ok(&root, &["task", "plan", task, "--plan-file", &plan]);
    }
    let shown = spec(&home, &root, &["--json", "show", "R-01"]).json();
    let base = shown["nodes"][0]["span_hash"].as_str().unwrap().to_owned();
    let text = spec(&home, &root, &["show", "R-01"]).stdout;
    let text = text
        .split_once('\n')
        .unwrap()
        .1
        .replacen("three", "four", 1);
    let run = run_product(
        &spec_bin(),
        &home,
        &root,
        &[
            "propose",
            "update",
            "R-01",
            "--base",
            &base,
            "--text-file",
            "-",
            "--rationale",
            "Four.",
            "--task",
            "T-0001",
        ],
        text.as_bytes(),
    );
    assert_eq!(run.code, 0, "{}", run.show());
    (root, home, cwd)
}

/// A pack of both tasks through the library, T-0002 held.
fn with_a_pack(root: &Path, home: &Path) {
    let env = specengine_cli::Env {
        cwd: root.to_path_buf(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let started = specengine_cli::auto_start(
        &env,
        &specengine_cli::Globals::default(),
        &specengine_cli::AutoStartRequest {
            tasks: vec!["T-0001".into(), "T-0002".into()],
            lines: None,
            until: None,
            now: "2026-11-15T00:00:00Z".to_owned(),
            git: specengine_cli::GitEnv::new(root, common::product_env(home)),
        },
        &mut |_, _| true,
    )
    .expect("the pack starts");
    assert_eq!(started.exit, specengine_cli::Exit::Answered, "{started:?}");
    let run = spec(
        home,
        root,
        &["auto", "hold", "T-0002", "--reason", "agent-error"],
    );
    assert_eq!(run.code, 0, "{}", run.show());
}

/// `spec feed --json` with `flags`: stdout without its LF.
fn cli(home: &Path, root: &Path, flags: &[&str]) -> String {
    let mut args = vec!["--json", "feed"];
    args.extend_from_slice(flags);
    let run = spec(home, root, &args);
    assert_eq!(run.code, 0, "{}", run.show());
    run.stdout.strip_suffix('\n').expect("one LF").to_owned()
}

/// FD3 locally: the body is the CLI's document; the 400s.
#[test]
fn fd3_the_local_route_is_spec_feed_json() {
    let scratch = Scratch::new("feed-route");
    let (root, home, cwd) = scripted(&scratch, &[&rule("on.toml")]);
    with_a_pack(&root, &home);
    let server = Server::serve(&home, &cwd, &[&root]);
    let base = format!("/api/projects/{SLUG}/feed");
    for (query, flags) in [
        ("?since=0&needs_me=1", vec!["--since", "0", "--needs-me"]),
        ("?task=T-0001", vec!["--task", "T-0001"]),
        (
            "?department=general&since=0",
            vec!["--department", "general", "--since", "0"],
        ),
        (
            "?since=2026-11-15T00:00:00Z",
            vec!["--since", "2026-11-15T00:00:00Z"],
        ),
        ("", vec![]),
    ] {
        let reply = server.get(&format!("{base}{query}"));
        assert_eq!(reply.status, 200, "{query}: {}", reply.text());
        assert_eq!(reply.text(), cli(&home, &root, &flags), "{query}");
    }
    let body: Value = server.get(&format!("{base}?since=0&needs_me=1")).json();
    assert_eq!(body["auto"]["state"], "on");
    assert!(
        body["needs_me"]["held"]
            .as_array()
            .is_some_and(|held| held.len() == 1)
    );
    assert!(
        body["needs_me"]["open"]
            .as_array()
            .is_some_and(|open| open.len() == 1)
    );

    // The 400s: nothing changed.
    let events = events_of(&home, &root);
    for query in [
        "?since=0&since=1",
        "?needs_me=1&needs_me=1",
        "?mode=all",
        "?needs_me=2",
        "?needs_me=true",
        "?since=-1",
        "?since=yesterday",
        "?task=T-0001&task=T-0002",
    ] {
        let reply = server.get(&format!("{base}{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        assert!(!reply.error_message().is_empty(), "{query}: the error body");
    }
    assert_eq!(events_of(&home, &root), events, "a read writes nothing");
}

const TEAM_OF_TWO: &str = "[team.ann]\nrole  = \"owner\"\nemail = \"ann@example.org\"\n\n[team.mira]\nrole  = \"member\"\nemail = \"mira@example.org\"\n";
const ANN: &str = "[team.ann]\nrole  = \"owner\"\nemail = \"ann@example.org\"\n";

/// FD3 on a hub: any login reads it; the head is the hub's. M: the hub's
/// head judged as a terminal's.
#[test]
fn fd3_on_a_hub_any_login_reads_it_and_the_head_is_the_hubs() {
    // A [team] of two: `team`.
    let scratch = Scratch::new("feed-route-hub");
    let (root, home, cwd) = scripted(&scratch, &[&rule("on.toml"), TEAM_OF_TWO]);
    let server = Server::start(&home, &cwd, &hub_args(&[&root], ORIGIN));
    let path = format!("/api/projects/{SLUG}/feed?since=0&needs_me=1");
    for login in ["ann", "zoe"] {
        let reply = through(&server, "GET", &path, &Proxied::fetch(login), b"");
        assert_eq!(reply.status, 200, "{login}: {}", reply.text());
        let body = reply.json();
        assert_eq!(body["auto"]["state"], "off", "{login}: {body}");
        assert_eq!(body["auto"]["reason"], "team");
        assert_eq!(
            body["auto"]["why"],
            "[team] names 2 logins: auto stops at two"
        );
        // Past the head, the terminal's document.
        let terminal: Value =
            serde_json::from_str(&cli(&home, &root, &["--since", "0", "--needs-me"])).unwrap();
        for key in ["after", "last_seq", "more", "entries", "needs_me"] {
            assert_eq!(body[key], terminal[key], "{login}: {key}");
        }
    }
    drop(server);

    // A rule of mode `local`: the hub's head off, the terminal's on.
    let scratch = Scratch::new("feed-route-local");
    let (root, home, cwd) = scripted(&scratch, &[&rule("local.toml"), ANN]);
    let server = Server::start(&home, &cwd, &hub_args(&[&root], ORIGIN));
    let reply = through(&server, "GET", &path, &Proxied::fetch("ann"), b"");
    assert_eq!(reply.status, 200, "{}", reply.text());
    let body = reply.json();
    assert_eq!(body["auto"]["state"], "off", "{body}");
    assert_eq!(body["auto"]["reason"], "off");
    assert_eq!(
        body["auto"]["why"],
        "[auto] mode is local: a hub acts only under hub or both"
    );
    let terminal: Value = serde_json::from_str(&cli(&home, &root, &["--since", "0"])).unwrap();
    assert_eq!(terminal["auto"]["state"], "on", "{terminal}");
}
