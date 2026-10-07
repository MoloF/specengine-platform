//! AC-06 of docs/features/daemon-read.md: the served projects. `--root A
//! --root B` serves both, in order (`/api/projects` `[{slug, name, root,
//! branch}]`, the start lines); a root without a config, a broken one,
//! one without a slug, a missing one, two of one slug, a taken or invalid
//! port, no `--root`: exit 2 before listening, `specengine-http: <reason>`
//! naming them, the port left free, nothing written in `HOME`; an unknown
//! slug is a 404 error body. M: a duplicate slug deduplicated.

mod common;

use std::ffi::OsStr;
use std::fs;
use std::net::TcpListener;
use std::path::Path;

use common::{Refused, Scratch, Server, root_args, snapshot, write};
use serde_json::json;

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("address")
        .port()
}

#[test]
fn ac06_both_roots_are_served_in_root_order() {
    let scratch = Scratch::new("projects-order");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "trunk");
    let home = scratch.home("h");

    let server = Server::serve(&home, scratch.path(), &[&a, &b]);
    assert_eq!(
        server.lines,
        [
            format!("serving lantern-keep {}", a.display()),
            format!("serving zerkalo {}", b.display()),
            format!("listening http://127.0.0.1:{}", server.port),
        ]
    );
    let reply = server.get("/api/projects");
    reply.status(200);
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8")
    );
    assert_eq!(
        reply.text(),
        expected_projects(&[
            ("lantern-keep", Some("lantern-keep"), &a, Some("main")),
            ("zerkalo", None, &b, Some("trunk")),
        ]),
        "the body is compact, keys slug, name, root, branch, in --root order"
    );
    drop(server);

    // The other order, a relative root, a detached HEAD and a root outside
    // git: B first; root canonical; branch null for both.
    scratch.git().run(&b, &["checkout", "-q", "--detach"]);
    let plain = scratch.copy("spec-a", "plain");
    let args: Vec<&OsStr> = vec![
        OsStr::new("--root"),
        OsStr::new("b/./"),
        OsStr::new("--root"),
        plain.as_os_str(),
        OsStr::new("--port"),
        OsStr::new("0"),
    ];
    let server = Server::start(&home, scratch.path(), &args);
    assert_eq!(server.lines[0], format!("serving zerkalo {}", b.display()));
    assert_eq!(
        server.get("/api/projects").status(200).text(),
        expected_projects(&[
            ("zerkalo", None, &b, None),
            ("lantern-keep", Some("lantern-keep"), &plain, None),
        ])
    );
}

fn expected_projects(entries: &[(&str, Option<&str>, &Path, Option<&str>)]) -> String {
    let items: Vec<String> = entries
        .iter()
        .map(|(slug, name, root, branch)| {
            format!(
                "{{\"slug\":{},\"name\":{},\"root\":{},\"branch\":{}}}",
                json!(slug),
                json!(name),
                json!(root.to_str().unwrap()),
                json!(branch)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// The start fails: exit 2, stdout empty, one `specengine-http: …` line
/// on stderr containing each of `named`; `port` free afterwards.
fn refused(home: &Path, cwd: &Path, args: &[&OsStr], port: u16, named: &[&str]) -> Refused {
    let refused = match Server::try_start(home, cwd, args) {
        Ok(server) => panic!(
            "{args:?} started a server listening on {}: {:?}",
            server.port, server.lines
        ),
        Err(refused) => refused,
    };
    assert_eq!(refused.code, Some(2), "{args:?}: {refused:?}");
    assert!(refused.stdout.is_empty(), "{args:?}: {refused:?}");
    assert!(
        refused.stderr.starts_with("specengine-http: ") && refused.stderr.ends_with('\n'),
        "{args:?}: {refused:?}"
    );
    for name in named {
        assert!(
            refused.stderr.contains(name),
            "{args:?}: stderr names {name:?}: {refused:?}"
        );
    }
    let listener = TcpListener::bind(("127.0.0.1", port));
    assert!(
        listener.is_ok(),
        "{args:?}: port {port} is free after the refusal"
    );
    refused
}

#[test]
fn ac06_a_start_that_cannot_serve_exits_2_before_listening() {
    let scratch = Scratch::new("projects-refused");
    let a = scratch.repo("spec-a", "a", "main");
    let twin = scratch.repo("spec-a", "twin", "main");
    let empty = scratch.dir("empty");
    let broken = scratch.copy("spec-b", "broken");
    write(&broken, "specengine.toml", "[project\nslug = \"zerkalo\"\n");
    let unslugged = scratch.copy("spec-b", "unslugged");
    let config = common::read_text(&unslugged, "specengine.toml");
    write(
        &unslugged,
        "specengine.toml",
        config.replace("slug = \"zerkalo\"\n", ""),
    );
    let home = scratch.home("h");
    let cwd = scratch.path();
    let missing = scratch.join("missing");
    let port = free_port();
    let port_text = port.to_string();
    let with_port = |roots: &[&OsStr]| -> Vec<&'static OsStr> {
        let mut args: Vec<&'static OsStr> = Vec::new();
        for root in roots {
            args.push(OsStr::new("--root"));
            args.push(Box::leak(root.to_os_string().into_boxed_os_str()));
        }
        args.push(OsStr::new("--port"));
        args.push(Box::leak(
            OsStr::new(&port_text).to_os_string().into_boxed_os_str(),
        ));
        args
    };

    // No --root at all.
    refused(
        &home,
        cwd,
        &[OsStr::new("--port"), OsStr::new(&port_text)],
        port,
        &["--root"],
    );
    // A root without a config, a broken config, no slug, a missing root.
    let empty_text = empty.display().to_string();
    refused(
        &home,
        cwd,
        &with_port(&[a.as_os_str(), empty.as_os_str()]),
        port,
        &[&empty_text],
    );
    let broken_text = broken.display().to_string();
    let broken_run = refused(
        &home,
        cwd,
        &with_port(&[broken.as_os_str()]),
        port,
        &[&broken_text],
    );
    assert_eq!(broken_run.stderr.lines().count(), 1, "{broken_run:?}");
    let unslugged_text = unslugged.display().to_string();
    refused(
        &home,
        cwd,
        &with_port(&[unslugged.as_os_str()]),
        port,
        &[&unslugged_text, "slug"],
    );
    let missing_text = missing.display().to_string();
    refused(
        &home,
        cwd,
        &with_port(&[missing.as_os_str()]),
        port,
        &[&missing_text],
    );

    // Two roots of one slug: two copies, and one root twice (relative and
    // absolute).
    let a_text = a.display().to_string();
    let twin_text = twin.display().to_string();
    refused(
        &home,
        cwd,
        &with_port(&[a.as_os_str(), twin.as_os_str()]),
        port,
        &[&a_text, &twin_text, "lantern-keep"],
    );
    refused(
        &home,
        cwd,
        &with_port(&[OsStr::new("a"), a.as_os_str()]),
        port,
        &["--root a ", &a_text, "lantern-keep"],
    );

    // An invalid port; a taken one.
    let mut args = root_args(&[&a]);
    args.pop();
    args.push(OsStr::new("99999"));
    refused(&home, cwd, &args, port, &["99999"]);
    let taken = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let taken_port = taken.local_addr().unwrap().port().to_string();
    let mut args = root_args(&[&a]);
    args.pop();
    args.push(Box::leak(
        OsStr::new(&taken_port).to_os_string().into_boxed_os_str(),
    ));
    let run = Server::try_start(&home, cwd, &args)
        .err()
        .expect("a taken port refuses");
    assert_eq!(run.code, Some(2), "{run:?}");
    assert!(run.stdout.is_empty(), "{run:?}");
    assert!(
        run.stderr.starts_with("specengine-http: ") && run.stderr.contains(&taken_port),
        "{run:?}"
    );
    drop(taken);

    assert!(
        snapshot(&home).is_empty(),
        "a refused start opens nothing in the data directory: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
    assert!(
        fs::read_dir(&empty).unwrap().next().is_none(),
        "the empty root stays empty"
    );
}

#[test]
fn ac06_an_unknown_slug_is_a_404_error_body_listing_the_served() {
    let scratch = Scratch::new("projects-unknown");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a, &b]);
    for (method, path) in [
        ("GET", "/api/projects/nope/tree"),
        ("GET", "/api/projects/nope/nodes/MEC-STAMINA"),
        ("GET", "/api/projects/nope/search?query=stamina"),
        ("GET", "/api/projects/nope/bundle?node_ids=MEC-STAMINA"),
        ("GET", "/api/projects/nope/graph?ref=MEC-STAMINA"),
        ("GET", "/api/projects/nope/check"),
        ("GET", "/api/projects/nope/inbox"),
        ("GET", "/api/projects/nope/proposals/PR-0001"),
        ("GET", "/api/projects/nope/tasks"),
        ("GET", "/api/projects/nope/tasks/T-0001"),
        ("GET", "/api/projects/nope/events"),
        ("POST", "/api/projects/nope/proposals/PR-0001/decision"),
    ] {
        let reply = server.request(method, path, &[]);
        assert_eq!(reply.status, 404, "{method} {path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains("`nope`") && message.contains("lantern-keep, zerkalo"),
            "{method} {path}: {message}"
        );
    }
    // An unknown route: 404, the error body too, listing the routes
    // (`graph` and `check` since docs/features/ui-live.md, `tasks` and
    // `tasks/<id>` since docs/features/ui-live-tasks.md).
    for path in [
        "/",
        "/api",
        "/api/projects/lantern-keep",
        "/api/projects/lantern-keep/tasks/",
        "/api/projects/lantern-keep/tasks/T-0001/transition",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        let message = reply.error_message();
        for route in ["graph", "check", "proposals/<id>", "tasks", "tasks/<id>"] {
            assert!(
                message.contains(&format!(" {route},")),
                "{path}: the routes name `{route}`: {message}"
            );
        }
    }
    assert!(
        common::files_ending(&home, ".db").is_empty(),
        "no unknown-slug answer opened a database"
    );
}
