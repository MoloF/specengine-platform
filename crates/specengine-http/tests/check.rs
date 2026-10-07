//! AC-04, AC-05 of docs/features/ui-live.md and its "Rules and edge
//! cases": `GET /api/projects/:p/check`, the plain `spec check --json`.
//!
//! AC-04, the verdicts: git copies of A clean (the one error's file gone),
//! observed (`[check] mode = "observe"`, the error kept), blocked (A:
//! enforce and an error), cannot-check (an invalid `.spec-debt.toml`), one
//! daemon serving the four: each a 200 byte-equal to `spec --root R check
//! --json` stdout less its final LF (exit 0, 0, 1, 2), the JSON headers,
//! the same bytes again. A plain run never sends `new_debt` or
//! `introduced`. M: the generic exit map.
//!
//! AC-05, the reach: any query name (`staged`, `changed`, `baseline`,
//! `debt`, `json`, `config`, `x`) is a 400 (`… is no query name of check:
//! it takes no query name`), a bare `?` none; a daemon started with no
//! `git` on `PATH` answers `/check` 200, the CLI's bytes; its fresh `HOME`
//! holds nothing after; `git status --porcelain` stays empty and `HEAD`
//! unchanged. M: `staged` wired to `CheckedTree::Staged`.
//!
//! Rules: the working tree on disk is read, uncommitted and untracked
//! files included, never `HEAD` (an edit shows on the next read, no
//! event); a config that no longer parses makes `/check` and `/graph` a
//! 503 with the discovery's line, where `spec check` prints a
//! cannot-check report (the parity gap Q6), and the routes answer again
//! once it is restored.

mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use common::{
    Run, Scratch, Server, product_env, read_text, replace, root_args, snapshot, spec, write,
};
use serde_json::Value;

/// `spec --root root check --json`: exit 0, 1 or 2, one document.
fn cli_check(home: &Path, cwd: &Path, root: &Path) -> Run {
    let run = spec(
        home,
        cwd,
        &[
            "--root",
            root.to_str().expect("UTF-8 root"),
            "check",
            "--json",
        ],
    );
    assert!([0, 1, 2].contains(&run.code), "spec check: {}", run.show());
    run
}

/// The daemon's `/check` of `slug` against the CLI's on `root`: a 200
/// with the same bytes, the JSON headers, the same bytes again; the
/// CLI's run.
fn same_check(server: &Server, home: &Path, cwd: &Path, root: &Path, slug: &str) -> Run {
    let run = cli_check(home, cwd, root);
    let path = format!("/api/projects/{slug}/check");
    let reply = server.get(&path);
    assert_eq!(
        reply.status,
        200,
        "{path} vs spec check (exit {}): {}",
        run.code,
        reply.text()
    );
    assert_eq!(
        reply.text(),
        run.document(),
        "{path} is byte-equal to `spec --root {} check --json`",
        root.display()
    );
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8"),
        "{path}"
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
    let again = server.get(&path);
    assert_eq!(
        (again.status, again.text()),
        (200, reply.text()),
        "{path}: a repeat answers the same bytes"
    );
    run
}

/// A git copy of A at `dir` served as `slug`, `change` applied before
/// the commit.
fn variant(scratch: &Scratch, dir: &str, slug: &str, change: impl FnOnce(&Path)) -> PathBuf {
    let root = scratch.copy("spec-a", dir);
    replace(
        &root,
        "specengine.toml",
        "slug = \"lantern-keep\"",
        &format!("slug = \"{slug}\""),
    );
    change(&root);
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

/// The one error of A: its file.
const ERROR_FILE: &str = "docs/features/stamina-tuning.md";

#[test]
fn ac04_every_verdict_is_a_200_document_byte_equal_to_the_cli() {
    let scratch = Scratch::new("check-verdicts");
    let clean = variant(&scratch, "clean", "lk-clean", |root| {
        std::fs::remove_file(root.join(ERROR_FILE)).expect("remove the error's file");
    });
    let observed = variant(&scratch, "observed", "lk-observed", |root| {
        let config = read_text(root, "specengine.toml");
        write(
            root,
            "specengine.toml",
            format!("{config}\n[check]\nmode = \"observe\"\n"),
        );
    });
    let blocked = variant(&scratch, "blocked", "lk-blocked", |_| {});
    let cannot = variant(&scratch, "cannot", "lk-cannot", |root| {
        write(root, ".spec-debt.toml", "this is [ not toml\n");
    });
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&clean, &observed, &blocked, &cannot]);

    for (root, slug, verdict, mode, exit) in [
        (&clean, "lk-clean", "clean", "enforce", 0),
        (&observed, "lk-observed", "observed", "observe", 0),
        (&blocked, "lk-blocked", "blocked", "enforce", 1),
        (&cannot, "lk-cannot", "cannot-check", "enforce", 2),
    ] {
        let run = same_check(&server, &home, &cwd, root, slug);
        assert_eq!(run.code, exit, "{slug}: {}", run.show());
        let report = run.json();
        assert_eq!(report["verdict"], Value::from(verdict), "{slug}");
        assert_eq!(report["mode"], Value::from(mode), "{slug}");
        // The plain run: six keys, never a base's.
        let keys: Vec<&String> = report.as_object().expect("an object").keys().collect();
        assert_eq!(
            keys.len(),
            6,
            "{slug}: mode, verdict, counts, findings, stale, cannot_check: {keys:?}"
        );
        assert!(!run.stdout.contains("\"new_debt\""), "{slug}");
        assert!(!run.stdout.contains("\"introduced\""), "{slug}");
        let causes = report["cannot_check"].as_array().expect("cannot_check");
        assert_eq!(causes.is_empty(), verdict != "cannot-check", "{slug}");
        let errors = report["counts"]["errors"].as_u64().expect("errors");
        assert_eq!(
            errors > 0,
            verdict == "observed" || verdict == "blocked",
            "{slug}"
        );
    }
    // The causes reach the client verbatim.
    let reply = server.get("/api/projects/lk-cannot/check");
    let cause = &reply.json()["cannot_check"][0];
    assert_eq!(
        cause["path"],
        Value::from(".spec-debt.toml:1"),
        "{}",
        reply.text()
    );
    assert!(
        cause["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{}",
        reply.text()
    );
}

#[test]
fn ac05_check_takes_no_query_needs_no_git_and_writes_nothing() {
    let scratch = Scratch::new("check-reach");
    let a = scratch.repo("spec-a", "a", "main");
    let git = scratch.git();
    let head = git.run(&a, &["rev-parse", "HEAD"]);
    let files = snapshot(&a);
    let fresh = scratch.home("fresh");
    let cwd = scratch.dir("cwd");
    // No git on PATH: an empty directory is the whole PATH.
    let no_git = scratch.dir("no-git-bin");
    let env: Vec<(OsString, OsString)> = product_env(&fresh)
        .into_iter()
        .map(|(key, value)| {
            if key == "PATH" {
                (key, no_git.clone().into_os_string())
            } else {
                (key, value)
            }
        })
        .collect();
    let args = root_args(&[&a]);
    let server = Server::try_start_env(&env, &cwd, &args)
        .unwrap_or_else(|refused| panic!("the daemon without git did not start: {refused:?}"));
    let p = "/api/projects/lantern-keep";

    for name in [
        "staged=true",
        "changed=true",
        "baseline=.spec-debt.toml",
        "debt=true",
        "json=true",
        "config=specengine.toml",
        "x=1",
        "staged",
    ] {
        let path = format!("{p}/check?{name}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let key = name.split_once('=').map_or(name, |(key, _)| key);
        assert_eq!(
            reply.error_message(),
            format!("`{key}` is no query name of check: it takes no query name"),
            "{path}"
        );
    }

    // The CLI with git on PATH and its own HOME; the daemon without.
    let cli_home = scratch.home("cli");
    let run = cli_check(&cli_home, &cwd, &a);
    for path in [format!("{p}/check"), format!("{p}/check?")] {
        for _ in 0..3 {
            let reply = server.get(&path);
            assert_eq!(reply.status, 200, "{path}: {}", reply.text());
            assert_eq!(
                reply.text(),
                run.document(),
                "{path}: the CLI's bytes, no git"
            );
        }
    }
    assert_eq!(run.json()["verdict"], Value::from("blocked"));
    assert!(
        snapshot(&fresh).is_empty(),
        "a check opens no data directory: the fresh HOME holds {:?}",
        snapshot(&fresh).keys().collect::<Vec<_>>()
    );
    // The control: this daemon has no git indeed (the inbox lists the
    // repository's worktrees with it).
    let inbox = server.get(&format!("{p}/inbox"));
    assert_eq!(inbox.status, 503, "{}", inbox.text());
    drop(server);
    assert_eq!(
        git.run(
            &a,
            &[
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--ignored"
            ]
        ),
        "",
        "nothing written under the root"
    );
    assert_eq!(git.run(&a, &["rev-parse", "HEAD"]), head, "HEAD unchanged");
    let after: Vec<_> = snapshot(&a)
        .into_iter()
        .filter(|(path, _)| path != ".git/index")
        .collect();
    let before: Vec<_> = files
        .into_iter()
        .filter(|(path, _)| path != ".git/index")
        .collect();
    assert!(after == before, "no file under the root changed");
}

#[test]
fn check_and_graph_read_the_working_tree_on_disk_not_head() {
    let scratch = Scratch::new("check-disk");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a]);
    let slug = "lantern-keep";
    assert_eq!(
        same_check(&server, &home, &cwd, &a, slug).json()["verdict"],
        Value::from("blocked")
    );
    // The error fixed on disk, not committed: the next read shows it.
    replace(&a, ERROR_FILE, "tier: two\n", "tier: 2\n");
    let fixed = same_check(&server, &home, &cwd, &a, slug);
    assert_ne!(
        fixed.json()["verdict"],
        Value::from("blocked"),
        "{}",
        fixed.stdout
    );
    // An untracked file with an error is read too.
    write(
        &a,
        "docs/spec/untracked.md",
        "---\nid: MEC-UNTRACKED\nclass: canon\ntier: two\nowner: owner\n\
         reviewed: 2026-09-20\n---\n\n# Untracked\n\nNot committed.\n",
    );
    let untracked = same_check(&server, &home, &cwd, &a, slug);
    assert!(
        untracked.stdout.contains("docs/spec/untracked.md"),
        "{}",
        untracked.stdout
    );
    // The graph reads the same tree: the new node is there.
    let reply = server.get("/api/projects/lantern-keep/graph?ref=MEC-UNTRACKED");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.text(),
        common::spec_json(&home, &cwd, &a, &["graph", "MEC-UNTRACKED"]).document()
    );
}

#[test]
fn a_config_that_no_longer_parses_is_a_503_on_check_and_graph() {
    let scratch = Scratch::new("check-broken");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    server.get(&format!("{p}/check")).status(200);
    server.get(&format!("{p}/graph?ref=MEC-SPRINT")).status(200);

    let config = read_text(&a, "specengine.toml");
    write(&a, "specengine.toml", format!("{config}\nbroken = [\n"));
    let root = a.to_str().unwrap();
    let graph_run = spec(
        &home,
        &cwd,
        &["--root", root, "graph", "MEC-SPRINT", "--json"],
    );
    graph_run.code(2);
    let line = graph_run.stderr.trim_end_matches('\n').to_owned();
    assert!(line.contains("specengine.toml"), "{line}");
    for path in [format!("{p}/check"), format!("{p}/graph?ref=MEC-SPRINT")] {
        let reply = server.get(&path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(reply.error_message(), line, "{path}: the discovery's line");
    }
    // The parity gap Q6: the CLI's check prints a cannot-check report.
    let check_run = cli_check(&home, &cwd, &a);
    assert_eq!(check_run.code, 2, "{}", check_run.show());
    assert_eq!(
        check_run.json()["verdict"],
        Value::from("cannot-check"),
        "{}",
        check_run.show()
    );

    write(&a, "specengine.toml", config);
    same_check(&server, &home, &cwd, &a, "lantern-keep");
    server.get(&format!("{p}/graph?ref=MEC-SPRINT")).status(200);
}
