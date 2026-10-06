//! Edge cases of docs/features/daemon-read.md beyond the numbered ACs:
//! "Rules and edge cases" R4 (a queue of a newer build is a 503 per call,
//! the CLI's line verbatim), "Description" (the config re-read per call:
//! a slug changed since the start is a 503 naming both, served again once
//! reverted), the fence's malformed `Host` cases, and determinism (a
//! repeated request, and a second daemon on a second copy under a fresh
//! `HOME`, answer byte for byte the same).

mod common;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use common::{Scratch, Server, Stream, ask, data_dir, read_text, spec, spec_json, write};

fn sqlite3(db: &Path, sql: &str) -> String {
    let output = Command::new("/usr/bin/sqlite3")
        .env_clear()
        .args(["-cmd", ".timeout 2000"])
        .arg(db)
        .arg(sql)
        .output()
        .expect("run sqlite3");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("sqlite3 output")
}

#[test]
fn a_queue_of_a_newer_build_is_a_503_per_call_with_the_clis_line() {
    let scratch = Scratch::new("edges-newer");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let id = ask(&home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
    let db = data_dir(&home).join("lantern-keep.db");
    let version: i64 = sqlite3(&db, "PRAGMA user_version;").trim().parse().unwrap();
    sqlite3(&db, &format!("PRAGMA user_version = {};", version + 7));
    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    for (path, args) in [
        (format!("{p}/inbox"), vec!["inbox"]),
        (format!("{p}/proposals/{id}"), vec!["review", id.as_str()]),
    ] {
        let root = a.to_str().unwrap();
        let mut all = vec!["--root", root];
        all.extend_from_slice(&args);
        all.push("--json");
        let run = spec(&home, &cwd, &all);
        run.code(2);
        let reply = server.get(&path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            run.stderr.trim_end_matches('\n'),
            "{path}: the CLI's line"
        );
    }
    // The tail cannot start either: a 503, not a stream.
    let reply = server.request_within("GET", &format!("{p}/events"), &[], Duration::from_secs(20));
    assert_eq!(reply.status, 503, "{}", reply.text());
    reply.error_message();
    // The reads of the index answer as the CLI does (exit 0 or 2 alike).
    for (path, args) in [
        (format!("{p}/tree"), vec!["tree"]),
        (
            format!("{p}/nodes/MEC-STAMINA"),
            vec!["show", "MEC-STAMINA"],
        ),
    ] {
        let root = a.to_str().unwrap();
        let mut all = vec!["--root", root];
        all.extend_from_slice(&args);
        all.push("--json");
        let run = spec(&home, &cwd, &all);
        let reply = server.get(&path);
        match run.code {
            0 => assert_eq!(reply.text(), run.document(), "{path}"),
            2 => assert_eq!(reply.status, 503, "{path}"),
            other => panic!("{path}: exit {other}"),
        }
    }
    assert_eq!(
        sqlite3(&db, "PRAGMA user_version;").trim(),
        (version + 7).to_string(),
        "the newer queue is left as it is"
    );
}

#[test]
fn a_slug_changed_since_the_start_is_a_503_naming_both_until_reverted() {
    let scratch = Scratch::new("edges-slug");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a, &b]);
    let config = read_text(&a, "specengine.toml");
    write(
        &a,
        "specengine.toml",
        config.replace("slug = \"lantern-keep\"", "slug = \"lantern-two\""),
    );
    for path in [
        "/api/projects/lantern-keep/tree",
        "/api/projects/lantern-keep/nodes/MEC-STAMINA",
        "/api/projects/lantern-keep/inbox",
        "/api/projects",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains("lantern-keep") && message.contains("lantern-two"),
            "{path}: names both slugs: {message}"
        );
    }
    let reply = server.request_within(
        "GET",
        "/api/projects/lantern-keep/events",
        &[],
        Duration::from_secs(20),
    );
    assert_eq!(reply.status, 503, "{}", reply.text());
    // B is untouched.
    server.get("/api/projects/zerkalo/tree").status(200);
    // The new slug is not served.
    assert_eq!(server.get("/api/projects/lantern-two/tree").status, 404);
    write(&a, "specengine.toml", &config);
    server.get("/api/projects/lantern-keep/tree").status(200);
    server.get("/api/projects").status(200);
    assert!(
        common::files_ending(&home, "lantern-two.db").is_empty(),
        "no database of the new slug"
    );
}

#[test]
fn a_malformed_or_doubled_host_is_refused() {
    let scratch = Scratch::new("edges-host");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let own = format!("127.0.0.1:{}", server.port);
    let other_port = format!("127.0.0.1:{}", server.port.wrapping_add(1));
    let upper = format!("LOCALHOST:{}", server.port);
    let cases: Vec<Vec<u8>> = vec![
        format!("GET /api/projects HTTP/1.1\r\nHost: {own}\r\nHost: {own}\r\nConnection: close\r\n\r\n").into_bytes(),
        b"GET /api/projects HTTP/1.0\r\n\r\n".to_vec(),
        "GET /api/projects HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n".into(),
        format!("GET /api/projects HTTP/1.1\r\nHost: {other_port}\r\nConnection: close\r\n\r\n").into_bytes(),
        format!("GET /api/projects HTTP/1.1\r\nHost: {upper}\r\nConnection: close\r\n\r\n").into_bytes(),
        b"GET http://evil.example/api/projects HTTP/1.1\r\nHost: evil.example\r\nConnection: close\r\n\r\n"
            .to_vec(),
        format!(
            "GET /api/projects HTTP/1.1\r\nHost: {own}\r\nOrigin: http://{own}\r\nOrigin: http://{own}\r\nConnection: close\r\n\r\n"
        )
        .into_bytes(),
    ];
    for bytes in &cases {
        let reply = server.raw(bytes);
        assert_eq!(
            reply.status,
            403,
            "{:?}: {}",
            String::from_utf8_lossy(bytes),
            String::from_utf8_lossy(&reply.body)
        );
        reply.error_message();
    }
    assert!(common::snapshot(&home).is_empty(), "nothing read");
}

#[test]
fn every_document_is_deterministic_across_requests_and_daemons() {
    let scratch = Scratch::new("edges-determinism");
    let first = scratch.repo("spec-a", "first", "main");
    let second = scratch.repo("spec-a", "second", "main");
    let cwd = scratch.dir("cwd");
    let paths = [
        "/api/projects/lantern-keep/tree",
        "/api/projects/lantern-keep/tree?depth=1",
        "/api/projects/lantern-keep/nodes/MEC-STAMINA?with=links",
        "/api/projects/lantern-keep/nodes/MEC-STAMINA%23RULE-STAM-REGEN",
        "/api/projects/lantern-keep/search?query=stamina",
        "/api/projects/lantern-keep/bundle?node_ids=MEC-STAMINA",
        "/api/projects/lantern-keep/bundle?node_ids=MEC-SPRINT&node_ids=R-12&budget=900",
        "/api/projects/lantern-keep/inbox",
    ];
    let answers = |root: &Path, home: &Path| -> Vec<Vec<u8>> {
        let server = Server::serve(home, &cwd, &[root]);
        let once: Vec<Vec<u8>> = paths
            .iter()
            .map(|path| server.get(path).status(200).body.clone())
            .collect();
        let twice: Vec<Vec<u8>> = paths
            .iter()
            .map(|path| server.get(path).body.clone())
            .collect();
        assert_eq!(once, twice, "a repeated request answers the same bytes");
        let mut stream = Stream::open(server.port, "/api/projects/lantern-keep/events", &[]);
        let opening = stream.next_frame(Duration::from_secs(5)).expect("opening");
        assert_eq!(opening.raw, "id: 0");
        once
    };
    let a = answers(&first, &scratch.home("one"));
    let b = answers(&second, &scratch.home("two"));
    for ((path, left), right) in paths.iter().zip(&a).zip(&b) {
        assert_eq!(
            String::from_utf8_lossy(left),
            String::from_utf8_lossy(right),
            "{path}: two daemons on two copies"
        );
    }
    // And the CLI's, for one of them.
    let run = spec_json(&scratch.home("three"), &cwd, &first, &["tree"]);
    assert_eq!(String::from_utf8_lossy(&a[0]), run.document());
}
