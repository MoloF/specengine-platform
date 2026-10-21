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
//!
//! docs/features/health-code.md RD-10 (WA-9): the route is unchanged for
//! rows 4-5. None of the fixtures above has `[code]`: their documents
//! carry `code` null and both rows `off`, nothing refreshed, as before. On
//! a copy of `fixtures/code-a` the body is the CLI's bytes with rows 4-5
//! measured, and a row that cannot be measured (a code walk gap, a bad
//! `[code]`) stays a 200 holding `cannot`, never a 503; the call writes
//! the slug's DB in the data directory alone. M: `cannot` as a 503.
//!
//! docs/features/health-drift.md RD-7 (9b-1, "Exit and route"): the body
//! grows by row 6, `drift`, the route unchanged. On the example tree
//! (code-a, the golden lock, `CAP` edited) the body is the CLI's bytes
//! with row 6 measured and its rows listed; a row 6 that cannot be
//! measured (an invalid `spec.lock`, no `git` on the daemon's `PATH`) and
//! one that shares rows 4-5's cause stay a 200 holding `cannot`, never a
//! 503. M: row 6 `cannot` as a 503, or the daemon's own git ignored.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use common::{
    Reply, Run, Scratch, Server, beyond_daemon, product_env, read_text, replace, root_args, spec,
    write,
};

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
    drop(server);

    // docs/features/health-drift.md RD-7: the example tree, a daemon of
    // its own (it refreshes the code index).
    let example = example_tree(&scratch, "example");
    let home = scratch.home("example");
    let server = Server::serve(&home, &cwd, &[&example]);
    let path = "/api/projects/code-a/health";
    let run = cli_health(&cli_home, &cwd, &example);
    let reply = server.get(path);
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
    assert_plain(&reply, path);
    let again = server.get(path);
    assert_eq!((again.status, again.text()), (200, reply.text()), "{path}");
    let document = reply.json();
    let drift = &document["drift"];
    assert_eq!(drift["state"], "measured", "{drift}");
    assert_eq!(drift["counts"]["rows"], 39, "{drift}");
    assert!(
        drift["rows"].as_array().is_some_and(|rows| rows
            .iter()
            .any(|row| row["state"] == "code_ahead" && row["lock"].is_object())),
        "the CAP row listed: {drift}"
    );
}

/// "Data"'s example tree (docs/features/health-drift.md): a git copy of
/// `fixtures/code-a` with the golden lock committed, `CAP: u32 = 10;` ->
/// `12;` in `counter.rs` left uncommitted.
fn example_tree(scratch: &Scratch, dir: &str) -> PathBuf {
    let lock = read_text(&common::fixture("spec-lock"), "code-a.lock");
    let root = variant(
        scratch,
        "code-a",
        dir,
        None,
        &[("spec.lock", lock.as_str())],
    );
    replace(
        &root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    root
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
            // docs/features/roadmap-surfaces.md: `roadmap` after `health`;
            // docs/features/lock-diff.md "Route": `lock-diff` after it.
            message.contains("graph, check, health, roadmap, lock-diff, inbox,"),
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

/// code-a's `[code] roots` line, as the fixture writes it.
const CODE_ROOTS: &str = "roots   = [\"crates\", \"data\", \"tools/loose.rs\"]";

/// docs/features/health-code.md RD-10 (WA-9, "Route and HTTP"): on a git
/// copy of `fixtures/code-a` with a rule on its laws, the route's body is
/// `spec --root R health --json`'s bytes (another `HOME`) with rows 4-5
/// measured. A code walk gap (`lib.rs` mode 000) and a bad `[code]`
/// (`roots` a string), each made after the start, keep it a 200, the
/// CLI's bytes, rows 4-5 `cannot` with no count, `code` null, row 3
/// measured; restored, measured again. Beyond the daemon's own files the
/// data directory holds the slug's DB alone. M: `cannot` as a 503.
#[test]
fn a_row_that_cannot_be_measured_is_still_a_200() {
    let scratch = Scratch::new("health-code-route");
    let config = format!(
        "{}\n[[health.bound]]\nkinds = [\"law\"]\n",
        read_text(&common::fixture("code-a"), "specengine.toml")
    );
    assert!(config.contains(CODE_ROOTS), "{config}");
    let code = variant(
        &scratch,
        "code-a",
        "code",
        None,
        &[("specengine.toml", config.as_str())],
    );
    let home = scratch.home("fresh");
    let cli_home = scratch.home("cli");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&code]);
    let path = "/api/projects/code-a/health";
    let measured = |context: &str| {
        let reply = server.get(path);
        assert_eq!(reply.status, 200, "{context}: {}", reply.text());
        assert_eq!(
            reply.text(),
            cli_health(&cli_home, &cwd, &code).document(),
            "{context}: the CLI's bytes"
        );
        let document = reply.json();
        for row in ["assumes", "unbound"] {
            assert_eq!(document[row]["state"], "measured", "{context}: {row}");
        }
        // docs/features/health-drift.md: no lock, every binding `unlocked`.
        assert_eq!(document["drift"]["state"], "measured", "{context}");
        assert!(document["code"].is_object(), "{context}");
        assert_plain(&reply, context);
    };
    measured("measured");

    let cannot = |context: &str| {
        for _ in 0..2 {
            let reply = server.get(path);
            assert_eq!(reply.status, 200, "{context}: no 503: {}", reply.text());
            assert_eq!(
                reply.text(),
                cli_health(&cli_home, &cwd, &code).document(),
                "{context}: the CLI's bytes"
            );
            let document = reply.json();
            for row in ["assumes", "unbound"] {
                assert_eq!(document[row]["state"], "cannot", "{context}: {row}");
                assert!(document[row]["counts"].is_null(), "{context}: {row}");
                assert!(document[row]["rows"].is_null(), "{context}: {row}");
                assert_eq!(
                    document[row]["causes"].as_array().map(Vec::len),
                    Some(1),
                    "{context}: {row}"
                );
            }
            assert!(document["code"].is_null(), "{context}");
            assert!(
                document["documents"]
                    .as_u64()
                    .is_some_and(|count| count > 0),
                "{context}: row 3 measured"
            );
            // docs/features/health-drift.md "Degrade matrix": a shared
            // step's cause is row 6's too.
            let drift = &document["drift"];
            assert_eq!(drift["state"], "cannot", "{context}: {drift}");
            for key in ["counts", "rows", "tested"] {
                assert!(drift[key].is_null(), "{context}: {drift}");
            }
            assert!(
                drift["causes"]
                    .as_array()
                    .is_some_and(|causes| causes.contains(&document["assumes"]["causes"][0])),
                "{context}: {drift}"
            );
            assert_plain(&reply, context);
        }
    };

    // A code walk gap after the start.
    let lib = code.join("crates/tally/src/lib.rs");
    fs::set_permissions(&lib, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    cannot("a code walk gap");
    fs::set_permissions(&lib, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    measured("readable again");

    // A bad `[code]` after the start.
    write(
        &code,
        "specengine.toml",
        config.replace(CODE_ROOTS, "roots   = \"crates\""),
    );
    cannot("a bad [code]");
    write(&code, "specengine.toml", &config);
    measured("restored");

    // docs/features/health-drift.md RD-7: an invalid `spec.lock` after the
    // start: row 6 alone `cannot` (one cause, `spec.lock:<line>: …`), rows
    // 4-5 measured, a 200 with the CLI's bytes; removed, measured again.
    write(&code, "spec.lock", "format = 2\n[[binding]\n");
    for _ in 0..2 {
        let reply = server.get(path);
        assert_eq!(
            reply.status,
            200,
            "an invalid lock: no 503: {}",
            reply.text()
        );
        assert_eq!(
            reply.text(),
            cli_health(&cli_home, &cwd, &code).document(),
            "an invalid lock: the CLI's bytes"
        );
        let document = reply.json();
        for row in ["assumes", "unbound"] {
            assert_eq!(document[row]["state"], "measured", "an invalid lock: {row}");
        }
        let drift = &document["drift"];
        assert_eq!(drift["state"], "cannot", "{drift}");
        for key in ["counts", "rows", "tested"] {
            assert!(drift[key].is_null(), "never 0: {drift}");
        }
        assert_eq!(drift["notes"], serde_json::json!([]), "{drift}");
        let causes = drift["causes"].as_array().expect("causes");
        assert!(
            causes.len() == 1
                && causes[0]
                    .as_str()
                    .is_some_and(|cause| cause.starts_with("spec.lock:")),
            "{drift}"
        );
        assert_plain(&reply, "an invalid lock");
    }
    fs::remove_file(code.join("spec.lock")).expect("the lock removed");
    measured("the lock removed");

    let written = beyond_daemon(&home);
    assert!(!written.is_empty(), "the route refreshed the code index");
    assert!(
        written.iter().all(|entry| entry.contains("code-a.db")),
        "the slug's DB alone: {written:?}"
    );
}

/// docs/features/health-drift.md RD-7, RD-8 ("Exit and route": the
/// handler passes the daemon's own git): a daemon whose `PATH` holds no
/// `git` serves the example tree with row 6 `cannot`, its one cause ``
/// `git` could not be run: no `git` program was found on PATH ``, never
/// a count; rows 3-5 as a daemon with git serves them; a 200 both times.
/// M: the handler reads git from somewhere but its own environment.
#[test]
fn a_daemon_without_git_serves_row_6_cannot() {
    let scratch = Scratch::new("health-drift-no-git");
    let example = example_tree(&scratch, "example");
    let cwd = scratch.dir("cwd");
    let no_git = scratch.dir("no-git-bin");
    let path = "/api/projects/code-a/health";

    let home = scratch.home("with-git");
    let server = Server::serve(&home, &cwd, &[&example]);
    let with_git = server.get(path);
    assert_eq!(with_git.status, 200, "{}", with_git.text());
    drop(server);

    let home = scratch.home("without-git");
    let env: Vec<(std::ffi::OsString, std::ffi::OsString)> = product_env(&home)
        .into_iter()
        .map(|(key, value)| {
            if key == "PATH" {
                (key, no_git.clone().into_os_string())
            } else {
                (key, value)
            }
        })
        .collect();
    let server = Server::try_start_env(&env, &cwd, &root_args(&[&example]))
        .unwrap_or_else(|refused| panic!("the daemon without git did not start: {refused:?}"));
    let reply = server.get(path);
    assert_eq!(reply.status, 200, "no 503: {}", reply.text());
    assert_plain(&reply, "no git");
    let document = reply.json();
    assert_eq!(
        document["drift"],
        serde_json::json!({"state": "cannot",
            "causes": ["`git` could not be run: no `git` program was found on PATH"],
            "counts": null, "rows": null, "tested": null, "notes": []}),
    );
    let measured = with_git.json();
    assert_eq!(measured["drift"]["state"], "measured");
    for key in [
        "documents",
        "amendments",
        "notes",
        "code",
        "assumes",
        "unbound",
    ] {
        assert_eq!(document[key], measured[key], "rows 3-5's `{key}`");
    }
}
