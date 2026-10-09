//! docs/features/health-left.md AC-R6 (AC-T1's HTTP half): `GET
//! /api/projects/:p/health`, `spec health --json` behind the fence.
//!
//! One daemon serving git copies of `fixtures/spec-a` (no amendment), A
//! with amendments written in (a spec draft, an accepted decision whose
//! links take every state, a decision without status), `spec-b` (its
//! broken `QN-08` a note) and the non-Rust `spec-c`: each a 200 byte-equal
//! to `spec --root R health --json` stdout less its final LF, the JSON
//! headers, `no-store`, no `Access-Control-*`, the same bytes again; a
//! fresh `HOME` holds nothing beyond the daemon's own files after (no
//! index refreshed). Any query name, and a bad `%`, is a 400 naming it;
//! every method but GET (HEAD too) a 405 `Allow: GET`; an unknown slug a
//! 404; an unknown route's 404 lists `health`; the fence's 403s. A walk
//! gap (a mode-000 file, a missing written root) is a 503 whose two-key
//! body's `message` is the CLI's stderr line; a config the discovery
//! refuses, and a slug changed since the start, a 503 too, the route
//! answering again once each is restored.
//!
//! M: `limit` accepted; the route at `/left`; 200 `{"amendments":[]}` on a
//! gap.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use common::{Reply, Run, Scratch, Server, beyond_daemon, read_text, replace, spec, write};

/// A's amendments (docs/features/health-left.md AC-R1, AC-R2).
const AMENDMENTS: [(&str, &str); 3] = [
    (
        "docs/features/regen-delay.md",
        "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
         [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
         {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint.\n",
    ),
    (
        "docs/records/DEC/DEC-0031.md",
        "---\nid: DEC-0031\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         links:\n  amends: [EDGE-STAM-ZERO, QST-031, RULE-NOPE, DEC-0007]\n---\n\n\
         # Amber light at zero stamina\n",
    ),
    (
        "docs/records/DEC/DEC-0032.md",
        "---\nid: DEC-0032\nclass: decision\ndate: 2026-10-02\nlinks:\n  amends: \
         [RULE-SPRINT-COST]\n---\n\n# Cellar walls slow the sprint\n",
    ),
];

/// A git copy of `fixture` at `dir`, served as `slug` (when given), with
/// `files` written before the commit.
fn variant(
    scratch: &Scratch,
    fixture: &str,
    dir: &str,
    slug: Option<(&str, &str)>,
    files: &[(&str, &str)],
) -> PathBuf {
    let root = scratch.copy(fixture, dir);
    if let Some((from, to)) = slug {
        replace(
            &root,
            "specengine.toml",
            &format!("slug = \"{from}\""),
            &format!("slug = \"{to}\""),
        );
    }
    for (path, text) in files {
        write(&root, path, text);
    }
    let git = scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "fixture"]);
    root
}

/// `spec --root root health --json`: exit 0, one document.
fn cli_health(home: &Path, cwd: &Path, root: &Path) -> Run {
    let run = spec(
        home,
        cwd,
        &[
            "--root",
            root.to_str().expect("UTF-8 root"),
            "health",
            "--json",
        ],
    );
    run.code(0);
    run
}

/// `spec --root root health --json` that cannot run: exit 2, stdout
/// empty; its stderr less the final LF.
fn cli_cannot(home: &Path, cwd: &Path, root: &Path) -> String {
    let run = spec(
        home,
        cwd,
        &[
            "--root",
            root.to_str().expect("UTF-8 root"),
            "health",
            "--json",
        ],
    );
    run.code(2);
    assert_eq!(run.stdout, "", "exit 2 prints no document: {}", run.show());
    run.stderr.trim_end_matches('\n').to_owned()
}

/// `no-store`, no `Access-Control-*`.
fn assert_plain(reply: &Reply, context: &str) {
    assert_eq!(
        reply.header("cache-control"),
        Some("no-store"),
        "{context}: {:?}",
        reply.headers
    );
    assert!(
        reply.cors_headers().is_empty(),
        "{context}: {:?}",
        reply.headers
    );
}

#[test]
fn ac_r6_every_health_is_the_clis_document_byte_for_byte() {
    let scratch = Scratch::new("health-parity");
    let a = variant(&scratch, "spec-a", "a", None, &[]);
    let amended = variant(
        &scratch,
        "spec-a",
        "amended",
        Some(("lantern-keep", "lk-amended")),
        &AMENDMENTS,
    );
    let b = variant(&scratch, "spec-b", "b", None, &[]);
    let c = variant(&scratch, "spec-c", "c", None, &[]);
    let home = scratch.home("fresh");
    let cli_home = scratch.home("cli");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a, &amended, &b, &c]);

    for (root, slug, rows, notes) in [
        (&a, "lantern-keep", 0, 0),
        (&amended, "lk-amended", 3, 0),
        (&b, "zerkalo", 0, 1),
        (&c, "dock-hire", 0, 0),
    ] {
        let run = cli_health(&cli_home, &cwd, root);
        let document = run.json();
        assert_eq!(
            document["amendments"].as_array().map(Vec::len),
            Some(rows),
            "{slug}: {}",
            run.stdout
        );
        assert_eq!(
            document["notes"].as_array().map(Vec::len),
            Some(notes),
            "{slug}"
        );
        for path in [
            format!("/api/projects/{slug}/health"),
            format!("/api/projects/{slug}/health?"),
        ] {
            let reply = server.get(&path);
            assert_eq!(reply.status, 200, "{path}: {}", reply.text());
            assert_eq!(
                reply.text(),
                run.document(),
                "{path}: `spec --root R health --json` less its final LF"
            );
            assert_eq!(
                reply.header("content-type"),
                Some("application/json; charset=utf-8"),
                "{path}"
            );
            assert_plain(&reply, &path);
            let again = server.get(&path);
            assert_eq!((again.status, again.text()), (200, reply.text()), "{path}");
        }
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "no index refreshed, nothing written: {:?}",
        beyond_daemon(&home)
    );
}

#[test]
fn ac_r6_a_query_another_method_an_unknown_slug_or_route_and_the_fence() {
    let scratch = Scratch::new("health-refusals");
    let a = variant(&scratch, "spec-a", "a", None, &AMENDMENTS);
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    let path = "/api/projects/lantern-keep/health";

    // Any query name: 400 naming it.
    for query in ["limit=5", "x=1", "json", "depth=2", "archive=true", "limit"] {
        let target = format!("{path}?{query}");
        let reply = server.get(&target);
        assert_eq!(reply.status, 400, "{target}: {}", reply.text());
        let key = query.split_once('=').map_or(query, |(key, _)| key);
        assert_eq!(
            reply.error_message(),
            format!("`{key}` is no query name of health: it takes no query name"),
            "{target}"
        );
        assert_plain(&reply, &target);
    }
    // A bad `%`: 400 naming it.
    let target = format!("{path}?li%zzmit=5");
    let reply = server.get(&target);
    assert_eq!(reply.status, 400, "{target}: {}", reply.text());
    let message = reply.error_message();
    assert!(message.contains("li%zzmit"), "{target}: {message}");

    // Every method but GET, HEAD too: 405 `Allow: GET`.
    for method in ["HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"] {
        let reply = server.request(method, path, &[]);
        let context = format!("{method} {path}");
        assert_eq!(reply.status, 405, "{context}: {reply:?}");
        let allows: Vec<&str> = reply
            .headers
            .iter()
            .filter(|(name, _)| name == "allow")
            .map(|(_, value)| value.as_str())
            .collect();
        assert_eq!(allows, ["GET"], "{context}");
        assert_plain(&reply, &context);
        if method == "HEAD" {
            assert!(reply.body.is_empty(), "{context}");
        } else {
            assert_eq!(
                reply.error_message(),
                format!("method {method} is not served on {path}: only GET is served here"),
                "{context}"
            );
        }
    }

    // An unknown slug: 404.
    let reply = server.get("/api/projects/no-such-project/health");
    assert_eq!(reply.status, 404, "{}", reply.text());
    let message = reply.error_message();
    assert!(message.contains("no-such-project"), "{message}");
    assert_plain(&reply, "unknown slug");

    // An unknown route: 404 listing `health`, after `check`.
    for unknown in [
        "/api/projects/lantern-keep/left",
        "/api/projects/lantern-keep/health/more",
        "/api/projects/lantern-keep/healths",
    ] {
        let reply = server.get(unknown);
        assert_eq!(reply.status, 404, "{unknown}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.starts_with(&format!(
                "no route {unknown}: the routes are /api/projects "
            )),
            "{unknown}: {message}"
        );
        assert!(
            message.contains("graph, check, health, inbox,"),
            "{unknown}: the list names health: {message}"
        );
        assert_plain(&reply, unknown);
    }

    // The fence: a foreign Host, Origin or Sec-Fetch-Site is a 403.
    let own = format!("127.0.0.1:{}", server.port);
    let evil_host = format!("evil.example:{}", server.port);
    let foreign: Vec<Vec<(&str, &str)>> = vec![
        vec![("Host", evil_host.as_str())],
        vec![("Host", own.as_str()), ("Origin", "http://evil.example")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "cross-site")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "same-site")],
    ];
    for headers in &foreign {
        for method in ["GET", "HEAD", "POST"] {
            let reply = server.raw(&common::request_bytes(method, path, headers));
            let context = format!("{method} {headers:?}");
            assert_eq!(reply.status, 403, "{context}: {}", reply.text());
            assert_eq!(reply.header("allow"), None, "{context}");
            if method != "HEAD" {
                let message = reply.error_message();
                assert!(message.starts_with("refused: "), "{context}: {message}");
            }
            assert_plain(&reply, &context);
        }
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "no refusal ran a call: {:?}",
        beyond_daemon(&home)
    );
    // The control: the route answers.
    let reply = server.get(path);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["amendments"].as_array().map(Vec::len), Some(3));
}

#[test]
fn ac_r6_a_walk_gap_or_a_refused_config_is_a_503_never_a_document() {
    let scratch = Scratch::new("health-503");
    let unreadable = variant(
        &scratch,
        "spec-a",
        "unreadable",
        Some(("lantern-keep", "lk-unreadable")),
        &AMENDMENTS,
    );
    let missing = variant(
        &scratch,
        "spec-a",
        "missing",
        Some(("lantern-keep", "lk-missing")),
        &AMENDMENTS,
    );
    let config = variant(
        &scratch,
        "spec-a",
        "config",
        Some(("lantern-keep", "lk-config")),
        &AMENDMENTS,
    );
    let home = scratch.home("fresh");
    let cli_home = scratch.home("cli");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&unreadable, &missing, &config]);

    // Both gaps arise after the start (the daemon reads the config again
    // per call).
    let file = unreadable.join("docs/spec/game.md");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let text = read_text(&missing, "specengine.toml");
    write(
        &missing,
        "specengine.toml",
        format!("{text}\n[paths]\nroots = [\"docs\", \"notes\"]\n"),
    );
    for (root, slug, names) in [
        (&unreadable, "lk-unreadable", "`docs/spec/game.md`"),
        (&missing, "lk-missing", "`notes`"),
    ] {
        let line = cli_cannot(&cli_home, &cwd, root);
        assert!(
            line.starts_with("spec: the walk is incomplete: ") && line.contains(names),
            "{line}"
        );
        assert!(
            line.ends_with(": unapplied amendments not measured"),
            "{line}"
        );
        let path = format!("/api/projects/{slug}/health");
        for _ in 0..2 {
            let reply = server.get(&path);
            assert_eq!(reply.status, 503, "{path}: {}", reply.text());
            assert_eq!(
                reply.error_message(),
                line,
                "{path}: the CLI's line verbatim"
            );
            assert_plain(&reply, &path);
            assert!(!reply.text().contains("amendments\""), "{}", reply.text());
        }
    }
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    let reply = server.get("/api/projects/lk-unreadable/health");
    assert_eq!(reply.status, 200, "readable again: {}", reply.text());
    assert_eq!(
        reply.text(),
        cli_health(&cli_home, &cwd, &unreadable).document()
    );

    // A config the discovery refuses: 503, then 200 once restored.
    let text = read_text(&config, "specengine.toml");
    write(&config, "specengine.toml", "this is [ not toml\n");
    let reply = server.get("/api/projects/lk-config/health");
    assert_eq!(reply.status, 503, "{}", reply.text());
    let message = reply.error_message();
    assert!(message.contains("specengine.toml"), "{message}");
    assert_plain(&reply, "a refused config");
    write(&config, "specengine.toml", &text);
    let reply = server.get("/api/projects/lk-config/health");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.text(),
        cli_health(&cli_home, &cwd, &config).document()
    );
    // A slug changed since the start: 503, then 200 once restored.
    write(
        &config,
        "specengine.toml",
        text.replace("slug = \"lk-config\"", "slug = \"lk-renamed\""),
    );
    let reply = server.get("/api/projects/lk-config/health");
    assert_eq!(reply.status, 503, "{}", reply.text());
    let message = reply.error_message();
    assert!(message.contains("lk-renamed"), "{message}");
    assert_plain(&reply, "a changed slug");
    write(&config, "specengine.toml", &text);
    server.get("/api/projects/lk-config/health").status(200);
    assert!(
        beyond_daemon(&home).is_empty(),
        "{:?}",
        beyond_daemon(&home)
    );
}
