//! docs/features/direction-catalog.md (RM-0106) "Acceptance criteria",
//! the daemon's half: `GET /api/projects/studio-lamp/catalog` over the copy
//! (the shared scene of `specengine-cli`'s `tests/catalog_scene`), a daemon
//! of its own under the copy's `HOME`.
//!
//! - DC-9: `?kinds=scene` is byte-equal to `spec catalog --kind scene
//!   --json` less its LF, the spec's document; repeated `kinds` and
//!   `archive=true` are the CLI's `--kind …` and `--archive`; no `kinds` and
//!   an unknown parameter are 400s naming `kinds` (and `archive`); an
//!   unknown route's 404 lists `catalog`; the daemon never cuts. M:
//!   `archive` not passed through.
//! - DC-8, the daemon's half: a `git` shim first on the daemon's `PATH`
//!   logs every git run. The second request at the same `HEAD` runs
//!   `rev-parse` alone; after a new commit the walk runs again; a request
//!   with one more kind walks once more, then is held; every answer
//!   byte-equals the CLI's at that state; a shallow answer is not kept past
//!   `fetch --unshallow`. M: the cache keyed without `HEAD`.
//!
//! The key sets are `daemon_keys.rs`'s.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/catalog_scene/mod.rs"]
mod catalog_scene;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use catalog_scene::{BOB, Lamp, STORM, pinned_json};
use common::{Server, product_env, root_args, spec};
use serde_json::{Value, json};

const ROUTE: &str = "/api/projects/studio-lamp/catalog";

fn serve(copy: &Lamp) -> Server {
    Server::serve(&copy.home, &copy.root, &[&copy.root])
}

/// `spec --root R catalog args… --json`'s document, less its LF.
fn cli(copy: &Lamp, args: &[&str]) -> String {
    let root = copy.root.to_str().expect("UTF-8 root");
    let mut all = vec!["--root", root, "catalog"];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(&copy.home, &copy.root, &all);
    run.code(0);
    run.document().to_owned()
}

/// An archived scene document (a shipped spec) committed by Bob.
fn add_archived(copy: &Lamp) {
    copy.put(
        "docs/spec/scn-wreck.md",
        "---\nid: SCN-WRECK\nclass: spec\nstatus: shipped\ntitle: The wreck\nscope: \
         [scenes]\n---\n\n# The wreck\n\nA hull on the reef.\n",
    );
    copy.commit(
        BOB,
        BOB,
        "2026-01-05T10:00:00Z",
        "2026-01-05T10:00:00Z",
        "The wreck\n",
    );
}

/// DC-9: `?kinds=scene` is the CLI's JSON and the spec's document;
/// `kinds=scene&kinds=island` is `--kind scene --kind island`;
/// `archive=true` is `--archive`, which lists the archived scene the plain
/// read leaves out; `archive=false` is the plain read. M: `archive` not
/// passed through.
#[test]
fn dc9_the_route_is_the_clis_json() {
    let pinned = Lamp::pinned("http-dc9");
    let copy = &pinned.copy;
    add_archived(copy);
    let server = serve(copy);
    let plain = server.get(&format!("{ROUTE}?kinds=scene"));
    plain.status(200);
    assert_eq!(plain.text(), cli(copy, &["--kind", "scene"]));
    for (query, args) in [
        (
            "kinds=scene&archive=true",
            &["--kind", "scene", "--archive"][..],
        ),
        ("kinds=scene&archive=false", &["--kind", "scene"]),
        (
            "kinds=scene&kinds=island",
            &["--kind", "scene", "--kind", "island"],
        ),
        (
            "archive=true&kinds=island&kinds=scene",
            &["--kind", "island", "--kind", "scene", "--archive"],
        ),
    ] {
        let reply = server.get(&format!("{ROUTE}?{query}"));
        reply.status(200);
        assert_eq!(reply.text(), cli(copy, args), "{query}");
    }
    let archived = server
        .get(&format!("{ROUTE}?kinds=scene&archive=true"))
        .json();
    assert_eq!(archived["archive"], true);
    let listed: Vec<&str> = archived["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["id"].as_str().expect("an id"))
        .collect();
    assert!(listed.contains(&"SCN-WRECK"), "{listed:?}");
    assert!(
        !plain.text().contains("SCN-WRECK"),
        "the plain read leaves the archive out"
    );
}

/// DC-9: on the copy as pinned, the route's body is the spec's document.
#[test]
fn dc9_the_route_serves_the_pinned_document() {
    let pinned = Lamp::pinned("http-dc9-pinned");
    let copy = &pinned.copy;
    let server = serve(copy);
    let reply = server.get(&format!("{ROUTE}?kinds=scene"));
    reply.status(200);
    assert_eq!(
        format!("{}\n", reply.text()),
        pinned_json(&pinned.applied, &pinned.hand)
    );
}

/// DC-9: no `kinds` is a 400 naming it; an unknown parameter a 400 listing
/// `kinds` and `archive`; the daemon serves the next good read; an empty
/// `kinds=` is a 400 naming `kinds` or the CLI's `--kind ""`.
#[test]
fn dc9_a_bad_query_is_a_400() {
    let pinned = Lamp::pinned("http-dc9-400");
    let copy = &pinned.copy;
    let server = serve(copy);
    for query in ["", "?archive=true", "?archive=false"] {
        let reply = server.get(&format!("{ROUTE}{query}"));
        assert_eq!(reply.status, 400, "{query:?}: {}", reply.text());
        assert!(
            reply.error_message().contains("kinds"),
            "{query:?}: {}",
            reply.error_message()
        );
    }
    for query in ["kinds=scene&x=1", "kind=scene", "kinds=scene&depth=2"] {
        let reply = server.get(&format!("{ROUTE}?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains("kinds") && message.contains("archive"),
            "{query}: {message}"
        );
    }
    server.get(&format!("{ROUTE}?kinds=scene")).status(200);
    // An empty value (the spec names no rule): a 400 naming `kinds`, or the
    // CLI's `--kind ""` (`rows 0`), never anything else.
    let empty = server.get(&format!("{ROUTE}?kinds="));
    if empty.status == 400 {
        assert!(
            empty.error_message().contains("kinds"),
            "{}",
            empty.error_message()
        );
    } else {
        empty.status(200);
        assert_eq!(empty.text(), cli(copy, &["--kind", ""]));
    }
}

/// DC-4, the daemon's half: the copy's commits carry past committer dates;
/// after them every scene file's mtime is set a day past now. Each row's
/// `last_change.at` from `GET …/catalog?kinds=scene` equals `spec catalog
/// --json`'s and the commit's `%ct` in UTC (git's own reading, `TZ=UTC`),
/// never the file's time. M: the file's mtime used as `at`.
#[test]
fn dc4_the_routes_at_is_the_commits_time_never_the_files() {
    let pinned = Lamp::pinned("http-dc4-mtime");
    let copy = &pinned.copy;
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(86_400);
    for relative in [catalog_scene::ARRIVAL, STORM] {
        fs::File::options()
            .write(true)
            .open(copy.path(relative))
            .expect("open")
            .set_modified(later)
            .expect("an mtime");
    }
    let server = serve(copy);
    let served: Value = server
        .get(&format!("{ROUTE}?kinds=scene"))
        .status(200)
        .json();
    let cli: Value = serde_json::from_str(&cli(copy, &["--kind", "scene"])).expect("JSON");
    let rows = served["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2);
    for (row, cli_row) in rows.iter().zip(cli["rows"].as_array().expect("rows")) {
        let change = &row["last_change"];
        let commit = change["commit"].as_str().expect("a commit");
        let utc = String::from_utf8(copy.git.git_env(
            &copy.root,
            &[
                "log",
                "-1",
                "--format=%cd",
                "--date=format-local:%Y-%m-%dT%H:%M:%SZ",
                commit,
            ],
            &[("TZ", std::ffi::OsStr::new("UTC"))],
        ))
        .expect("UTF-8")
        .trim()
        .to_owned();
        assert_eq!(change["at"], json!(utc), "{row}");
        assert_eq!(change["at"], cli_row["last_change"]["at"], "{row}");
    }
    assert_eq!(
        rows.iter()
            .map(|row| row["last_change"]["at"].clone())
            .collect::<Vec<_>>(),
        [json!("2026-01-02T10:00:00Z"), json!("2026-01-03T10:00:00Z")]
    );
}

/// DC-9 (exits): a database a newer build wrote cannot be read: the CLI's
/// exit 2, the daemon's 503, never a clean row.
#[test]
fn dc9_an_unreadable_queue_is_a_503() {
    let pinned = Lamp::pinned("http-dc9-503");
    let copy = &pinned.copy;
    copy.sql("PRAGMA user_version = 99;");
    let server = serve(copy);
    let reply = server.get(&format!("{ROUTE}?kinds=scene"));
    assert_eq!(reply.status, 503, "{}", reply.text());
}

/// DC-9 (rollout): an unknown route under the project lists `catalog`.
#[test]
fn dc9_an_unknown_route_lists_catalog() {
    let copy = Lamp::new("http-dc9-404");
    let server = serve(&copy);
    for path in [
        "/api/projects/studio-lamp/catalogs",
        "/api/projects/studio-lamp/catalog/scene",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        let message = reply.error_message();
        let listed = message
            .split_once('{')
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(list, _)| list.split(", ").collect::<Vec<_>>())
            .unwrap_or_default();
        assert!(listed.contains(&"catalog"), "{path}: {message}");
    }
}

/// DC-9 (the browser view): 420 scenes with long titles, past the CLI's
/// cut, are all served, `truncated` false, `omitted` `null`; the CLI's
/// rows are its first ones.
#[test]
fn dc9_the_route_never_cuts() {
    let copy = Lamp::new("http-dc9-uncut");
    let long = "of the long stair below the lamp room ".repeat(3);
    for i in 0..420 {
        copy.put(
            &format!("docs/spec/many/scn-many-{i:03}.md"),
            &format!(
                "---\nid: SCN-MANY-{i:03}\nclass: canon\ntitle: Step {i:03} {}\nowner: \
                 owner\nreviewed: 2026-11-08\n---\n\n# Step {i:03}\n\nA step.\n",
                long.trim_end()
            ),
        );
    }
    copy.commit(
        BOB,
        BOB,
        "2026-02-01T10:00:00Z",
        "2026-02-01T10:00:00Z",
        "Many steps\n",
    );
    let server = serve(&copy);
    let served = server
        .get(&format!("{ROUTE}?kinds=scene"))
        .status(200)
        .json();
    assert_eq!(served["truncated"], false);
    assert_eq!(served["omitted"], Value::Null);
    let rows = served["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 422);
    let cut: Value = serde_json::from_str(&cli(&copy, &["--kind", "scene"])).expect("JSON");
    assert_eq!(cut["truncated"], true);
    let shown = cut["rows"].as_array().expect("rows");
    assert!(shown.len() < 422);
    assert_eq!(shown[..], rows[..shown.len()]);
}

// ------------------------------------------------------------------ DC-8

/// `spec --root <root> catalog args… --json` under the copy's `HOME`, in
/// `root`: the document, less its LF.
fn cli_at(copy: &Lamp, root: &Path, args: &[&str]) -> String {
    let root_text = root.to_str().expect("UTF-8 root");
    let mut all = vec!["--root", root_text, "catalog"];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(&copy.home, root, &all);
    run.code(0);
    run.document().to_owned()
}

/// DC-8 (shallow answers are not kept, iteration 2): a daemon over a
/// `--depth 1` clone of the copy's three commits answers both scenes
/// unreached (the shallow note, 2 files); after `git fetch --deepen=1` at
/// the same `HEAD` the storm's change is read and the arrival's still
/// beyond the boundary (1 file), as the CLI answers there.
#[test]
fn dc8_a_deepened_clone_answers_as_the_cli() {
    let pinned = Lamp::committed("http-dc8-deepen");
    let copy = &pinned.copy;
    let clone = copy.scratch().join("shallow");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let clone = fs::canonicalize(&clone).expect("canonical");
    let server = Server::serve(&copy.home, &clone, &[&clone]);
    let scene = format!("{ROUTE}?kinds=scene");
    let before = server.get(&scene).status(200).text().to_owned();
    assert_eq!(before, cli_at(copy, &clone, &["--kind", "scene"]));
    assert!(before.contains("a shallow clone: 2 files"), "{before}");
    let head = copy.git.git_text(&clone, &["rev-parse", "HEAD"]);
    copy.git.git(&clone, &["fetch", "-q", "--deepen=1"]);
    assert_eq!(copy.git.git_text(&clone, &["rev-parse", "HEAD"]), head);
    let after = server.get(&scene).status(200).text().to_owned();
    assert_eq!(after, cli_at(copy, &clone, &["--kind", "scene"]));
    assert!(after.contains("a shallow clone: 1 files"), "{after}");
    let document: Value = serde_json::from_str(&after).expect("JSON");
    assert_eq!(
        document["rows"][1]["last_change"]["commit"],
        json!(pinned.hand)
    );
}

/// DC-8 (shallow answers are not kept): a daemon over a `--depth 1` clone
/// answers with the shallow note; after `git fetch --unshallow` at the same
/// `HEAD` it answers as the CLI does there, every last change read.
#[test]
fn dc8_an_unshallowed_clone_answers_as_the_cli() {
    let pinned = Lamp::committed("http-dc8-unshallow");
    let copy = &pinned.copy;
    let clone = copy.scratch().join("shallow");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let clone = fs::canonicalize(&clone).expect("canonical");
    let server = Server::serve(&copy.home, &clone, &[&clone]);
    let scene = format!("{ROUTE}?kinds=scene");
    let before = server.get(&scene).status(200).text().to_owned();
    assert_eq!(before, cli_at(copy, &clone, &["--kind", "scene"]));
    assert!(before.contains("a shallow clone"), "{before}");
    let head = copy.git.git_text(&clone, &["rev-parse", "HEAD"]);
    copy.git.git(&clone, &["fetch", "-q", "--unshallow"]);
    assert_eq!(copy.git.git_text(&clone, &["rev-parse", "HEAD"]), head);
    let after = server.get(&scene).status(200).text().to_owned();
    assert_eq!(after, cli_at(copy, &clone, &["--kind", "scene"]));
    assert!(!after.contains("a shallow clone"), "{after}");
    let document: Value = serde_json::from_str(&after).expect("JSON");
    assert_eq!(
        document["rows"][0]["last_change"]["commit"],
        json!(pinned.applied)
    );
}

/// A `git` shim (in `<scratch>/<dir>`) logging its arguments to `args.log`
/// and a `cat-file`'s stdin to `stdin.log`, then running the real git.
fn shim(copy: &Lamp, dir: &str) -> (PathBuf, PathBuf, PathBuf) {
    let bin = copy.scratch().join(dir);
    fs::create_dir_all(&bin).expect("the shim's directory");
    let args = bin.join("args.log");
    let stdin = bin.join("stdin.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{args}'\ncase \" $* \" in\n  *\" cat-file \"*) \
         /usr/bin/tee -a '{stdin}' | '{git}' \"$@\" ;;\n  *) exec '{git}' \"$@\" ;;\nesac\n",
        args = args.display(),
        stdin = stdin.display(),
        git = common::real_git().display(),
    );
    let path = bin.join("git");
    fs::write(&path, script).expect("the shim");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    (bin, args, stdin)
}

/// The logged lines since the last call, the logs emptied.
fn drain(log: &Path) -> Vec<String> {
    let lines = fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    let _ = fs::remove_file(log);
    lines
}

/// A logged line's git subcommand (its first word that is no option and
/// no `-C` or `-c` value).
fn subcommand(line: &str) -> String {
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        if word == "-C" || word == "-c" {
            words.next();
            continue;
        }
        if !word.starts_with('-') {
            return word.to_owned();
        }
    }
    String::new()
}

/// The daemon over the copy with the shim first on its `PATH`.
fn shimmed(copy: &Lamp, bin: &Path) -> Server {
    let mut env = product_env(&copy.home);
    let path = env
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .expect("a PATH");
    let mut dirs = vec![bin.to_path_buf()];
    dirs.extend(std::env::split_paths(&path));
    let joined: OsString = std::env::join_paths(dirs).expect("a PATH");
    env.retain(|(name, _)| name != "PATH");
    env.push(("PATH".into(), joined));
    Server::try_start_env(&env, &copy.root, &root_args(&[&copy.root]))
        .unwrap_or_else(|refused| panic!("the daemon did not start: {refused:?}"))
}

/// DC-8: the first request walks; the second at the same `HEAD` runs
/// `rev-parse` alone; a new commit walks again; one more kind walks once,
/// then is held; each answer the CLI's. M: the cache keyed without `HEAD`.
#[test]
fn dc8_the_daemon_walks_once_per_head() {
    let pinned = Lamp::pinned("http-dc8");
    let copy = &pinned.copy;
    let (bin, args_log, stdin_log) = shim(copy, "shim");
    let server = shimmed(copy, &bin);
    let scene = format!("{ROUTE}?kinds=scene");

    let first = server.get(&scene).status(200).text().to_owned();
    let lines = drain(&args_log);
    let _ = drain(&stdin_log);
    assert!(
        lines.iter().any(|line| subcommand(line) == "log"),
        "the first request walks: {lines:#?}"
    );
    assert_eq!(first, cli(copy, &["--kind", "scene"]));

    let second = server.get(&scene).status(200).text().to_owned();
    let lines = drain(&args_log);
    let others: Vec<&String> = lines
        .iter()
        .filter(|line| subcommand(line) != "rev-parse")
        .collect();
    assert_eq!(
        others,
        Vec::<&String>::new(),
        "the same HEAD: rev-parse alone: {lines:#?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("rev-parse")),
        "HEAD is read: {lines:#?}"
    );
    assert_eq!(second, first);
    let _ = drain(&args_log);

    copy.edit(STORM, "lantern at once", "lantern at once and for good");
    let newer = copy.commit(
        BOB,
        BOB,
        "2026-01-08T10:00:00Z",
        "2026-01-08T10:00:00Z",
        "For good\n",
    );
    let _ = drain(&args_log);
    let third = server.get(&scene).status(200).text().to_owned();
    let lines = drain(&args_log);
    let _ = drain(&stdin_log);
    let walks = lines
        .iter()
        .filter(|line| subcommand(line) == "log" && line.contains("--first-parent"))
        .count();
    assert_eq!(walks, 1, "a new HEAD walks again: {lines:#?}");
    assert_eq!(third, cli(copy, &["--kind", "scene"]));
    let document: Value = serde_json::from_str(&third).expect("JSON");
    assert_eq!(
        document["rows"][1]["last_change"]["commit"],
        json!(newer),
        "the new commit is the storm's change"
    );
    let _ = drain(&args_log);

    let both = server
        .get(&format!("{ROUTE}?kinds=scene&kinds=chapter"))
        .status(200)
        .text()
        .to_owned();
    let lines = drain(&args_log);
    // One walk for the new kind's file (which paths it walks for is the CLI
    // unit seam's, `catalog_tests.rs`: the presence read lists the whole
    // tree since iteration 2, so no stdin names them here).
    let walks = lines
        .iter()
        .filter(|line| subcommand(line) == "log" && line.contains("--first-parent"))
        .count();
    assert_eq!(walks, 1, "one more kind walks once: {lines:#?}");
    assert_eq!(both, cli(copy, &["--kind", "scene", "--kind", "chapter"]));
    let again = server
        .get(&format!("{ROUTE}?kinds=scene&kinds=chapter"))
        .status(200)
        .text()
        .to_owned();
    let lines = drain(&args_log);
    assert!(
        lines.iter().all(|line| subcommand(line) == "rev-parse"),
        "both kinds held: rev-parse alone: {lines:#?}"
    );
    assert_eq!(again, both);
}

/// DC-8: two requests for the same kinds at two `HEAD`s give each its own
/// answer: going back to the first `HEAD` (a checkout of its commit)
/// answers as the CLI does there, never the newer `HEAD`'s rows.
#[test]
fn dc8_a_checkout_back_answers_its_own_head() {
    let pinned = Lamp::pinned("http-dc8-back");
    let copy = &pinned.copy;
    let server = serve(copy);
    let scene = format!("{ROUTE}?kinds=scene");
    let at_first = server.get(&scene).status(200).text().to_owned();
    copy.edit(STORM, "lantern at once", "lantern at last");
    copy.commit(
        BOB,
        BOB,
        "2026-01-08T10:00:00Z",
        "2026-01-08T10:00:00Z",
        "At last\n",
    );
    let at_second = server.get(&scene).status(200).text().to_owned();
    assert_ne!(at_first, at_second);
    assert_eq!(at_second, cli(copy, &["--kind", "scene"]));
    copy.git
        .git(&copy.root, &["checkout", "-q", "--detach", &pinned.hand]);
    let back = server.get(&scene).status(200).text().to_owned();
    assert_eq!(back, cli(copy, &["--kind", "scene"]));
    assert_eq!(back, at_first);
}
