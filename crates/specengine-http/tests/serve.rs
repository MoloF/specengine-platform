//! docs/features/spec-serve.md, the daemon's half: AC-D1, AC-D3..AC-D8,
//! AC-D10 and AC-D11's cases against a daemon `spec serve` started. Git
//! copies of `fixtures/spec-a` (A, `lantern-keep`) and `fixtures/spec-b`
//! (B, `zerkalo`), a scratch `HOME` per daemon, `--port 0` always; `spec`
//! is the build directory's sibling of the daemon binary, so `spec serve`
//! finds `specengine-http` beside it.
//!
//! Every process runs under `perl -e 'alarm N; exec @ARGV'`: `perl` execs
//! `spec`, `spec` execs `specengine-http`, so the child's PID is the
//! daemon's from its `listening` line on, and it is stopped by that PID
//! (the handle's drop: `SIGKILL`, reaped), never by name.
//!
//! - AC-D1: `spec serve` from `<A>/docs` serves A's canonical root, and
//!   `/api/projects` lists exactly it. M: the cwd passed for the root.
//! - AC-D3: a `spec` alone in a directory, no daemon on `PATH`: exit 2,
//!   one line naming both places and `cargo install`, no data directory.
//!   M: the data directory created before the lookup.
//! - AC-D4: one daemon per data directory: a second start under the same
//!   `HOME` (the daemon, `spec serve` in B) exits 2 before listening,
//!   naming the first's PID and URL; under other `HOME`s both start; two
//!   simultaneous starts leave exactly one listening. M: no lock.
//! - AC-D5: `spec serve` for served roots answers the "already running"
//!   lines, exit 0, nothing started, `daemon.json` byte-identical. M:
//!   always exec.
//! - AC-D6: `daemon.json` exactly: five keys in order, a project `slug`,
//!   `root`; the PID, the bound port and URL, the roots in order. M: a
//!   `token` key; the configured port 0 written.
//! - AC-D7: after `kill -9` the stale file is replaced by the next start.
//!   M: refused whenever the file exists.
//! - AC-D8: every served root's `git status` byte-identical through a
//!   start, reads, a stage and `SIGTERM`; a data directory inside a root
//!   (`HOME` the root): exit 2, nothing created. M: `daemon.json` written
//!   into the root.
//! - AC-D10: `--json`, `--config`, `--root` with a DIR: exit 2, the usage
//!   line, nothing created; no project: exit 2 naming `spec init`. M:
//!   `--json` accepted.
//! - AC-D11: the Host fence and the live tail of a daemon `spec serve`
//!   started.
//! - "Rules and edge cases": a held lock without a readable file (the
//!   "unpublished" line, after the 2 s and 500 ms waits), readers taking
//!   the five keys, the restart command's quoting, a renamed slug.

mod common;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use common::{
    HTTP, Refused, Scratch, Server, Stream, ask, beyond_daemon, copy_file, data_dir, product_env,
    product_path, run_product, snapshot, spec_bin,
};
use serde_json::{Value, json};

const A_SLUG: &str = "lantern-keep";
const B_SLUG: &str = "zerkalo";

/// Within this of the storing process's exit (docs/features/daemon-read.md
/// AC-07).
const LIVE: Duration = Duration::from_secs(1);

// ------------------------------------------------------------ helpers

/// `program args` started as a daemon in `cwd` with `env`: listening, or
/// what it printed before it exited.
fn launch_with(
    program: &Path,
    env: &[(OsString, OsString)],
    cwd: &Path,
    args: &[&str],
) -> Result<Server, Refused> {
    let os: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    Server::try_start_program(program, env, cwd, &os)
}

/// `spec args` (a `serve`) in `cwd` under `HOME=home`.
fn launch(home: &Path, cwd: &Path, args: &[&str]) -> Result<Server, Refused> {
    launch_with(&spec_bin(), &product_env(home), cwd, args)
}

/// [`launch`], which must listen with exactly the `serving` lines of
/// `served` (slug, canonical root) and its `listening` line: never the
/// "already running" answer.
fn serving(home: &Path, cwd: &Path, args: &[&str], served: &[(&str, &Path)]) -> Server {
    let server = launch(home, cwd, args)
        .unwrap_or_else(|refused| panic!("spec {args:?} did not start: {refused:?}"));
    let mut lines: Vec<String> = served
        .iter()
        .map(|(slug, root)| format!("serving {slug} {}", root.display()))
        .collect();
    lines.push(format!("listening http://127.0.0.1:{}", server.port));
    assert_eq!(
        server.lines, lines,
        "spec {args:?}: the daemon's start lines, nothing before them"
    );
    server
}

/// [`launch`] (or `program`), which must exit 2 before any `listening`:
/// stdout empty, exactly one stderr line, returned without its LF.
fn refused_with(program: &Path, env: &[(OsString, OsString)], cwd: &Path, args: &[&str]) -> String {
    let refused = match launch_with(program, env, cwd, args) {
        Ok(server) => panic!(
            "{} {args:?} started a daemon on {}: {:?}",
            program.display(),
            server.port,
            server.lines
        ),
        Err(refused) => refused,
    };
    assert_eq!(refused.code, Some(2), "{args:?}: {refused:?}");
    assert!(refused.stdout.is_empty(), "{args:?}: {refused:?}");
    assert!(
        refused.stderr.ends_with('\n') && refused.stderr.lines().count() == 1,
        "{args:?}: one stderr line: {refused:?}"
    );
    refused.stderr.trim_end_matches('\n').to_owned()
}

fn refused(home: &Path, cwd: &Path, args: &[&str]) -> String {
    refused_with(&spec_bin(), &product_env(home), cwd, args)
}

/// `specengine-http --root R... --port 0` under `HOME=home`, which must
/// exit 2 before listening: its one stderr line.
fn daemon_refused(home: &Path, cwd: &Path, roots: &[&Path]) -> String {
    let mut args: Vec<&str> = Vec::new();
    for root in roots {
        args.push("--root");
        args.push(root.to_str().expect("UTF-8 root"));
    }
    args.extend(["--port", "0"]);
    refused_with(Path::new(HTTP), &product_env(home), cwd, &args)
}

fn daemon_file(home: &Path) -> Vec<u8> {
    fs::read(data_dir(home).join("daemon.json")).expect("daemon.json")
}

fn daemon_json(home: &Path) -> Value {
    serde_json::from_slice(&daemon_file(home)).expect("daemon.json is JSON")
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
fn is_utc(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

/// `daemon.json` as the daemon must write it: `to_string_pretty` of the
/// five keys in order, a project exactly `slug`, `root`, then a LF.
fn expected_daemon_file(pid: u32, port: u16, served: &[(&str, &Path)], started: &str) -> String {
    let quoted = |text: &str| serde_json::to_string(text).expect("a JSON string");
    let projects: Vec<String> = served
        .iter()
        .map(|(slug, root)| {
            format!(
                "    {{\n      \"slug\": {},\n      \"root\": {}\n    }}",
                quoted(slug),
                quoted(root.to_str().expect("UTF-8 root"))
            )
        })
        .collect();
    format!(
        "{{\n  \"pid\": {pid},\n  \"port\": {port},\n  \"url\": \"http://127.0.0.1:{port}\",\n  \
         \"projects\": [\n{}\n  ],\n  \"started\": {}\n}}\n",
        projects.join(",\n"),
        quoted(started)
    )
}

/// Every key of `value`, at any depth.
fn keys(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                out.push(key.clone());
                keys(inner, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| keys(item, out)),
        _ => {}
    }
}

/// The restart command's word: as written within `A-Za-z0-9/._-`, else
/// single-quoted (docs/features/spec-serve.md "Rules and edge cases").
fn shell_word(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// `git status --porcelain --untracked-files=all --ignored` of `root`.
fn status(scratch: &Scratch, root: &Path) -> String {
    scratch.git().run(
        root,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--ignored",
        ],
    )
}

/// Every path under `dir` whose file name is one of the daemon's files.
fn daemon_files_under(dir: &Path) -> Vec<String> {
    snapshot(dir)
        .into_keys()
        .filter(|path| {
            let name = Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            name.starts_with("daemon.")
        })
        .collect()
}

// ------------------------------------------------------------ AC-D1

/// AC-D1: `spec serve --port 0` from `<A>/docs` serves A's canonical root
/// (the start lines, `/api/projects` exactly that root). M: the cwd passed
/// for the discovered root.
#[test]
fn d1_spec_serve_from_a_subdirectory_serves_the_found_root() {
    let scratch = Scratch::new("serve-d1");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = serving(
        &home,
        &a.join("docs"),
        &["serve", "--port", "0"],
        &[(A_SLUG, &a)],
    );
    let projects = server.get("/api/projects").status(200).json();
    let listed = projects.as_array().expect("a list of projects");
    assert_eq!(listed.len(), 1, "exactly the found root: {projects}");
    assert_eq!(listed[0]["slug"], json!(A_SLUG), "{projects}");
    assert_eq!(
        listed[0]["root"],
        json!(a.to_str().unwrap()),
        "the canonical root, not the cwd: {projects}"
    );
    // The global `--root` names the project as any command takes it.
    let other_home = scratch.home("h-root");
    let by_root = serving(
        &other_home,
        scratch.path(),
        &["--root", a.to_str().unwrap(), "serve", "--port", "0"],
        &[(A_SLUG, &a)],
    );
    drop(by_root);
    drop(server);
}

// ------------------------------------------------------------ AC-D3

/// AC-D3: `spec` copied alone into an empty directory, `PATH` without the
/// daemon: exit 2, one line naming both places and the install command;
/// no data directory. A `PATH` entry holding a directory or a
/// non-executable file of that name is no daemon either; one holding the
/// daemon starts it. M: the data directory created before the lookup.
#[test]
fn d3_no_daemon_beside_spec_or_on_path_is_exit_2_creating_nothing() {
    let scratch = Scratch::new("serve-d3");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let lonely_dir = scratch.dir("lonely");
    let lonely = lonely_dir.join("spec");
    copy_file(spec_bin(), &lonely);
    fs::set_permissions(&lonely, fs::Permissions::from_mode(0o755)).unwrap();
    let path = product_path();
    for dir in std::env::split_paths(&path) {
        assert!(
            !dir.join("specengine-http").exists(),
            "the test's PATH entry {} holds a specengine-http",
            dir.display()
        );
    }
    let env = product_env(&home);
    let line = refused_with(&lonely, &env, &a.join("docs"), &["serve", "--port", "0"]);
    assert_eq!(
        line,
        format!(
            "spec serve: specengine-http not found beside {} or on PATH; install it: \
             cargo install --locked --path <clone>/crates/specengine-http",
            lonely.display()
        )
    );
    assert!(
        snapshot(&home).is_empty(),
        "no data directory: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );

    // Not a daemon: a directory and a non-executable file of that name on
    // PATH.
    let decoy_dir = scratch.dir("decoy-dir");
    fs::create_dir_all(decoy_dir.join("specengine-http")).unwrap();
    let decoy_file = scratch.dir("decoy-file");
    copy_file(HTTP, decoy_file.join("specengine-http"));
    fs::set_permissions(
        decoy_file.join("specengine-http"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let mut decoys: Vec<PathBuf> = vec![decoy_dir.clone(), decoy_file.clone()];
    decoys.extend(std::env::split_paths(&path));
    let decoy_path = std::env::join_paths(&decoys).unwrap();
    let mut decoy_env: Vec<(OsString, OsString)> = env
        .iter()
        .filter(|(key, _)| key != "PATH")
        .cloned()
        .collect();
    decoy_env.push(("PATH".into(), decoy_path));
    let line = refused_with(
        &lonely,
        &decoy_env,
        &a.join("docs"),
        &["serve", "--port", "0"],
    );
    assert!(line.contains("not found beside"), "{line}");
    assert!(snapshot(&home).is_empty(), "no data directory");

    // The daemon on PATH (after the decoys) is found and started.
    let found = scratch.dir("found");
    symlink(HTTP, found.join("specengine-http")).unwrap();
    let mut on_path: Vec<PathBuf> = vec![decoy_dir, decoy_file, found];
    on_path.extend(std::env::split_paths(&path));
    let mut found_env: Vec<(OsString, OsString)> = env
        .iter()
        .filter(|(key, _)| key != "PATH")
        .cloned()
        .collect();
    found_env.push(("PATH".into(), std::env::join_paths(&on_path).unwrap()));
    let server = launch_with(
        &lonely,
        &found_env,
        &a.join("docs"),
        &["serve", "--port", "0"],
    )
    .unwrap_or_else(|refused| panic!("the daemon on PATH did not start: {refused:?}"));
    assert_eq!(
        server.lines,
        [
            format!("serving {A_SLUG} {}", a.display()),
            format!("listening http://127.0.0.1:{}", server.port)
        ]
    );
}

// ------------------------------------------------------------ AC-D4

/// AC-D4: a daemon up under `HOME` H: a second `specengine-http` and
/// `spec serve` in B exit 2 before any `listening`, naming the first's
/// PID and URL (and the restart command); under other `HOME`s each
/// starts. M: no lock.
#[test]
fn d4_a_second_start_under_the_same_home_exits_2_naming_the_first() {
    let scratch = Scratch::new("serve-d4");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let first = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    let url = format!("http://127.0.0.1:{}", first.port);
    let pid = first.pid();
    let file = daemon_file(&home);

    let line = daemon_refused(&home, scratch.path(), &[&b]);
    assert_eq!(
        line,
        format!(
            "specengine-http: a daemon already runs under {}: pid {pid}, {url}, serving \
             {A_SLUG} {}; stop it first",
            data_dir(&home).display(),
            a.display()
        )
    );
    let line = refused(&home, &b, &["serve", "--port", "0"]);
    assert_eq!(
        line,
        format!(
            "spec serve: a daemon (pid {pid}, {url}) serves {A_SLUG} {}; stop it, then run: \
             spec serve {} {} --port {}",
            a.display(),
            shell_word(a.to_str().unwrap()),
            shell_word(b.to_str().unwrap()),
            first.port
        )
    );
    assert_eq!(daemon_file(&home), file, "the first's file untouched");
    assert_eq!(
        beyond_daemon(&home),
        Vec::<String>::new(),
        "the refused starts wrote nothing"
    );

    // Under other HOMEs each starts, beside the first.
    let h2 = scratch.home("h2");
    let mut args: Vec<&OsStr> = vec![OsStr::new("--root"), b.as_os_str()];
    args.extend([OsStr::new("--port"), OsStr::new("0")]);
    let second = Server::start(&h2, scratch.path(), &args);
    let h3 = scratch.home("h3");
    let third = serving(&h3, &b, &["serve", "--port", "0"], &[(B_SLUG, &b)]);
    let mut first = first;
    assert!(first.alive(), "the first daemon still runs");
    assert_eq!(daemon_json(&h2)["pid"], json!(second.pid()));
    assert_eq!(daemon_json(&h3)["pid"], json!(third.pid()));
    assert_eq!(daemon_file(&home), file, "the first's file untouched");
}

/// AC-D4: two `specengine-http --port 0` started at once under one `HOME`
/// (three rounds, a fresh `HOME` each): exactly one listens; the other
/// exits 2 before listening, naming the winner (or its file unreadable
/// yet). M: no lock.
#[test]
fn d4_two_simultaneous_starts_leave_exactly_one_listening() {
    let scratch = Scratch::new("serve-d4-race");
    let a = scratch.repo("spec-a", "a", "main");
    for round in 0..3 {
        let home = scratch.home(&format!("race-{round}"));
        let barrier = Arc::new(Barrier::new(2));
        let starts: Vec<_> = (0..2)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let home = home.clone();
                let a = a.clone();
                let cwd = scratch.path().to_path_buf();
                thread::spawn(move || {
                    let args: Vec<&OsStr> = vec![
                        OsStr::new("--root"),
                        a.as_os_str(),
                        OsStr::new("--port"),
                        OsStr::new("0"),
                    ];
                    barrier.wait();
                    Server::try_start(&home, &cwd, &args)
                })
            })
            .collect();
        let results: Vec<Result<Server, Refused>> = starts
            .into_iter()
            .map(|start| start.join().expect("a start thread"))
            .collect();
        let (listening, refusals): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
        let listening: Vec<Server> = listening.into_iter().map(Result::unwrap).collect();
        let refusals: Vec<Refused> = refusals.into_iter().map(|r| r.err().unwrap()).collect();
        assert_eq!(
            listening.len(),
            1,
            "round {round}: exactly one listens: {:?} / {refusals:?}",
            listening.iter().map(|s| s.port).collect::<Vec<_>>()
        );
        let winner = &listening[0];
        let loser = &refusals[0];
        assert_eq!(loser.code, Some(2), "round {round}: {loser:?}");
        assert!(loser.stdout.is_empty(), "round {round}: {loser:?}");
        let named = format!(
            "specengine-http: a daemon already runs under {}: pid {}, http://127.0.0.1:{}, \
             serving {A_SLUG} {}; stop it first\n",
            data_dir(&home).display(),
            winner.pid(),
            winner.port,
            a.display()
        );
        let unpublished = format!(
            "specengine-http: a daemon holds {}, but {} is unreadable: ",
            data_dir(&home).join("daemon.lock").display(),
            data_dir(&home).join("daemon.json").display()
        );
        assert!(
            loser.stderr == named
                || (loser.stderr.starts_with(&unpublished) && loser.stderr.lines().count() == 1),
            "round {round}: the loser names the winner: {loser:?}"
        );
        assert_eq!(daemon_json(&home)["pid"], json!(winner.pid()));
    }
}

// ------------------------------------------------------------ AC-D5

/// AC-D5: `spec serve` for roots the daemon serves prints the three
/// "already running" lines (a `serving` line per requested root, in the
/// request's order), exit 0, whatever `--port`; nothing started,
/// `daemon.json` byte-identical. M: always exec.
#[test]
fn d5_a_daemon_serving_the_roots_is_answered_not_started_again() {
    let scratch = Scratch::new("serve-d5");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let a_text = a.to_str().unwrap();
    let b_text = b.to_str().unwrap();
    let mut server = serving(
        &home,
        scratch.path(),
        &["serve", a_text, b_text, "--port", "0"],
        &[(A_SLUG, &a), (B_SLUG, &b)],
    );
    let file = daemon_file(&home);
    let pid = server.pid();
    let url = format!("http://127.0.0.1:{}", server.port);
    let lines = |served: &[(&str, &Path)]| {
        let mut text = format!("already running: pid {pid}\n");
        for (slug, root) in served {
            text.push_str(&format!("serving {slug} {}\n", root.display()));
        }
        text.push_str(&format!("listening {url}\n"));
        text
    };
    let docs = a.join("docs");
    // A repeated canonical root is requested once.
    let again = format!("{a_text}/docs/..");
    for (cwd, args, served) in [
        (&a, vec!["serve"], vec![(A_SLUG, a.as_path())]),
        (
            &b,
            vec!["serve", a_text, again.as_str(), "docs/.."],
            vec![(A_SLUG, a.as_path()), (B_SLUG, b.as_path())],
        ),
        (
            &docs,
            vec!["serve", "--port", "0"],
            vec![(A_SLUG, a.as_path())],
        ),
        (
            &b,
            vec!["serve", "--port", "9"],
            vec![(B_SLUG, b.as_path())],
        ),
        (
            &docs,
            vec!["serve", b_text, a_text],
            vec![(B_SLUG, b.as_path()), (A_SLUG, a.as_path())],
        ),
        (
            &b,
            vec!["--root", a_text, "serve", "--port", "7777"],
            vec![(A_SLUG, a.as_path())],
        ),
    ] {
        let started = Instant::now();
        let run = run_product(&spec_bin(), &home, cwd, &args, b"");
        assert_eq!(
            (run.code, run.stdout.as_str(), run.stderr.as_str()),
            (0, lines(&served).as_str(), ""),
            "{args:?} in {}",
            cwd.display()
        );
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "{args:?}: answered, not served"
        );
        assert_eq!(
            daemon_file(&home),
            file,
            "{args:?}: daemon.json byte-identical"
        );
    }
    assert!(server.alive(), "the daemon still runs");
    assert_eq!(beyond_daemon(&home), Vec::<String>::new());
    // Still the one daemon answering.
    let projects = server.get("/api/projects").status(200).json();
    assert_eq!(projects.as_array().map(Vec::len), Some(2), "{projects}");
}

// ------------------------------------------------------------ AC-D6

/// AC-D6: `daemon.json`: the five keys in order, a project exactly `slug`,
/// `root`; `pid` the child's, `port` and `url` the `listening` line's,
/// `projects` the `serving` lines in order, `started` shaped; no key
/// holding `token`, `key` or `secret`; `daemon.lock` empty, no partial
/// file left. M: a `token` key; the configured port 0 written.
#[test]
fn d6_daemon_json_holds_exactly_the_pid_the_bound_port_and_the_roots() {
    let scratch = Scratch::new("serve-d6");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let before = specengine_cli::utc_now();
    let server = serving(
        &home,
        scratch.path(),
        &[
            "serve",
            b.to_str().unwrap(),
            a.to_str().unwrap(),
            "--port",
            "0",
        ],
        &[(B_SLUG, &b), (A_SLUG, &a)],
    );
    let after = specengine_cli::utc_now();
    assert_ne!(server.port, 0, "the bound port");
    let text = String::from_utf8(daemon_file(&home)).expect("UTF-8 daemon.json");
    let parsed: Value = serde_json::from_str(&text).expect("JSON");
    let started = parsed["started"].as_str().expect("started").to_owned();
    assert!(is_utc(&started), "started {started:?}");
    assert!(
        before <= started && started <= after,
        "started {started} within [{before}, {after}]"
    );
    assert_eq!(
        text,
        expected_daemon_file(
            server.pid(),
            server.port,
            &[(B_SLUG, &b), (A_SLUG, &a)],
            &started
        ),
        "the five keys in order, the PID, the bound port and URL, the roots in --root order"
    );
    let mut all = Vec::new();
    keys(&parsed, &mut all);
    for key in &all {
        let lower = key.to_ascii_lowercase();
        assert!(
            !["token", "key", "secret"]
                .iter()
                .any(|word| lower.contains(word)),
            "no secret-shaped key: {key:?} in {text}"
        );
    }
    let data = data_dir(&home);
    assert_eq!(
        fs::read(data.join("daemon.lock")).unwrap(),
        b"",
        "the lock is empty"
    );
    assert!(
        !data.join("daemon.json.tmp").exists(),
        "no partial file left"
    );
    // Readers take the file as the CLI library does.
    let read = specengine_cli::read_daemon_file(&data).expect("readable");
    assert_eq!(read.pid, server.pid());
    assert_eq!(read.port, server.port);
}

// ------------------------------------------------------------ AC-D7

/// AC-D7: after `kill -9` the file still names the dead daemon;
/// `spec serve` starts (no "already running") and rewrites it with the new
/// PID and port. M: refused whenever the file exists.
#[test]
fn d7_after_kill_9_the_next_start_replaces_the_stale_file() {
    let scratch = Scratch::new("serve-d7");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let first = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    let old_pid = first.pid();
    let stale = daemon_file(&home);
    // The handle's drop: SIGKILL by its PID, reaped.
    drop(first);
    assert_eq!(daemon_file(&home), stale, "the file outlives the daemon");
    assert_eq!(daemon_json(&home)["pid"], json!(old_pid));

    let second = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    let fresh = daemon_json(&home);
    assert_eq!(fresh["pid"], json!(second.pid()), "{fresh}");
    assert_eq!(fresh["port"], json!(second.port), "{fresh}");
    assert_ne!(second.pid(), old_pid);
    assert_ne!(daemon_file(&home), stale, "rewritten");
    // And it is the one daemon now: another start is answered.
    let run = run_product(&spec_bin(), &home, &a, &["serve"], b"");
    run.code(0);
    assert!(
        run.stdout
            .starts_with(&format!("already running: pid {}\n", second.pid())),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------ AC-D8

/// AC-D8: `git status --porcelain` of every served root byte-identical
/// before the start, after reads, after a stage and after `SIGTERM`; the
/// daemon's files only in the data directory. M: `daemon.json` written
/// into a root.
#[test]
fn d8_the_served_roots_stay_byte_identical_through_a_daemons_life() {
    let scratch = Scratch::new("serve-d8");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let before = [status(&scratch, &a), status(&scratch, &b)];
    assert_eq!(before, [String::new(), String::new()], "committed copies");
    let check = |when: &str| {
        assert_eq!(
            [status(&scratch, &a), status(&scratch, &b)],
            before,
            "{when}: git status of the served roots"
        );
        for root in [&a, &b] {
            assert_eq!(
                daemon_files_under(root),
                Vec::<String>::new(),
                "{when}: no daemon file under {}",
                root.display()
            );
        }
    };
    let mut server = serving(
        &home,
        scratch.path(),
        &[
            "serve",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "--port",
            "0",
        ],
        &[(A_SLUG, &a), (B_SLUG, &b)],
    );
    check("started");
    for slug in [A_SLUG, B_SLUG] {
        let p = format!("/api/projects/{slug}");
        for path in [
            "/api/projects".to_owned(),
            format!("{p}/tree"),
            format!("{p}/search?query=stamina"),
            format!("{p}/check"),
            format!("{p}/inbox"),
            format!("{p}/tasks"),
            format!("{p}/round"),
        ] {
            let reply = server.get(&path);
            assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        }
    }
    for path in [
        "/api/projects/lantern-keep/nodes/MEC-STAMINA",
        "/api/projects/lantern-keep/graph?ref=MEC-STAMINA",
        "/api/projects/lantern-keep/bundle?node_ids=MEC-STAMINA",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
    }
    check("after reads");

    // A stage: a question stored by `spec`, a reject staged by the page.
    let id = ask(&home, &a, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let proposal = server
        .get(&format!("/api/projects/{A_SLUG}/proposals/{id}"))
        .status(200)
        .json();
    let updated_at = proposal["updated_at"].as_str().expect("updated_at");
    let staged = server.stage(
        &format!("/api/projects/{A_SLUG}/proposals/{id}/decision"),
        &format!(
            "{{\"decision\":\"reject\",\"reason\":\"a duplicate\",\"updated_at\":\"{updated_at}\"}}"
        ),
    );
    assert_eq!(staged.status, 200, "{}", staged.text());
    check("after a stage");

    // SIGTERM by its PID ends it (reaped within the start timeout).
    server.signal_and_reap("TERM");
    check("after SIGTERM");
    let data = data_dir(&home);
    for name in ["daemon.lock", "daemon.json"] {
        assert!(data.join(name).is_file(), "{name} in the data directory");
    }
    assert!(!data.join("daemon.json.tmp").exists());
}

/// AC-D8: `HOME` set to a copy's root (the data directory inside it):
/// `spec serve` and `specengine-http` exit 2, nothing created, in either
/// root's position; the guard runs before anything.
#[test]
fn d8_a_data_directory_inside_a_served_root_is_refused_creating_nothing() {
    let scratch = Scratch::new("serve-d8-inside");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let tree_a = snapshot(&a);
    let tree_b = snapshot(&b);
    let inside = data_dir(&a);
    let expected_tail = "lies inside the project root; the index must live outside the \
                         repository (a project at the home directory is not supported); \
                         nothing was created";

    let line = refused(&a, &a, &["serve", "--port", "0"]);
    assert!(
        line.contains(&inside.display().to_string()) && line.ends_with(expected_tail),
        "{line}"
    );
    let line = refused(
        &a,
        scratch.path(),
        &[
            "serve",
            b.to_str().unwrap(),
            a.to_str().unwrap(),
            "--port",
            "0",
        ],
    );
    assert!(line.ends_with(expected_tail), "{line}");
    let line = daemon_refused(&a, scratch.path(), &[&a]);
    assert!(
        line.starts_with(&format!("specengine-http: --root {}: ", a.display()))
            && line.ends_with(expected_tail),
        "{line}"
    );
    let line = daemon_refused(&a, scratch.path(), &[&b, &a]);
    assert!(line.ends_with(expected_tail), "{line}");
    assert_eq!(snapshot(&a), tree_a, "nothing created in A");
    assert_eq!(snapshot(&b), tree_b, "nothing created in B");
    assert_eq!(status(&scratch, &a), "");
}

// ------------------------------------------------------------ AC-D10

/// AC-D10: `spec serve --json`, `spec serve --config F`, `spec --root A
/// serve B`: exit 2, the usage line, nothing created; no `specengine.toml`
/// up the tree: exit 2 naming `spec init`. M: `--json` accepted.
#[test]
fn d10_usage_refusals_exit_2_creating_nothing() {
    let scratch = Scratch::new("serve-d10");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("spec-b", "b", "main");
    let home = scratch.home("h");
    let docs = a.join("docs");
    let tree_a = snapshot(&a);
    let a_text = a.to_str().unwrap();
    let b_text = b.to_str().unwrap();
    let json_line = "spec serve: --json is not supported: the daemon prints lines";
    let config_line = "spec serve: --config is not supported: each root's specengine.toml is read";
    let both_line = "spec serve: give the roots as DIR... or as --root, not both";
    for (args, line) in [
        (vec!["serve", "--json", "--port", "0"], json_line),
        (vec!["--json", "serve", "--port", "0"], json_line),
        (vec!["serve", a_text, "--json", "--port", "0"], json_line),
        (
            vec!["serve", "--config", "specengine.toml", "--port", "0"],
            config_line,
        ),
        (vec!["--config", "F", "serve", "--port", "0"], config_line),
        (
            vec!["--root", a_text, "serve", b_text, "--port", "0"],
            both_line,
        ),
    ] {
        assert_eq!(refused(&home, &docs, &args), line, "{args:?}");
    }

    // No project up the tree; a DIR that is none.
    let empty = scratch.dir("empty");
    let line = refused(&home, &empty, &["serve", "--port", "0"]);
    assert!(line.contains("`spec init`"), "{line}");
    let line = refused(
        &home,
        &docs,
        &["serve", empty.to_str().unwrap(), "--port", "0"],
    );
    assert!(line.contains("`spec init`"), "{line}");
    // A port clap refuses.
    let run = run_product(
        &spec_bin(),
        &home,
        &docs,
        &["serve", "--port", "99999"],
        b"",
    );
    assert_eq!(run.code, 2, "{}", run.show());
    assert!(run.stdout.is_empty(), "{}", run.show());

    assert!(
        snapshot(&home).is_empty(),
        "nothing created: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
    assert!(fs::read_dir(&empty).unwrap().next().is_none());
    assert_eq!(snapshot(&a), tree_a, "nothing created in A");
}

// ------------------------------------------------------------ AC-D11

/// AC-D11 against a daemon `spec serve` started: a foreign `Host`,
/// `Origin` or `Sec-Fetch-Site` is a 403 before anything is read, the own
/// origin passes; a question `spec` stores reaches the live tail within
/// 1 s of its exit. M: no Host check.
#[test]
fn d11_the_fence_and_the_live_tail_hold_for_a_daemon_spec_serve_started() {
    let scratch = Scratch::new("serve-d11");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let server = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    let own = format!("127.0.0.1:{}", server.port);
    let evil = format!("evil.example:{}", server.port);
    let foreign: [Vec<(&str, &str)>; 3] = [
        vec![("Host", evil.as_str())],
        vec![("Host", own.as_str()), ("Origin", "http://evil.example")],
        vec![("Host", own.as_str()), ("Sec-Fetch-Site", "cross-site")],
    ];
    for path in ["/api/projects", "/api/projects/lantern-keep/tree"] {
        for headers in &foreign {
            let reply = server.raw(&common::request_bytes("GET", path, headers));
            assert_eq!(reply.status, 403, "{path} {headers:?}: {}", reply.text());
            assert!(
                reply.error_message().starts_with("refused: "),
                "{path} {headers:?}"
            );
        }
    }
    assert_eq!(
        beyond_daemon(&home),
        Vec::<String>::new(),
        "a refused request read nothing"
    );
    server.get("/api/projects").status(200);

    let mut stream = Stream::open(server.port, "/api/projects/lantern-keep/events", &[]);
    assert_eq!(stream.status, 200);
    assert_eq!(stream.header("content-type"), Some("text/event-stream"));
    let opening = stream
        .next_frame(Duration::from_secs(10))
        .expect("the opening frame");
    assert_eq!(opening.raw, "id: 0", "{opening:?}");
    let id = ask(
        &home,
        &a,
        &["EDGE-STAM-ZERO"],
        "Does the sprint end at zero?",
    );
    let exited = Instant::now();
    let frame = stream
        .next_event(Duration::from_secs(5))
        .expect("the stored question's event");
    assert!(
        exited.elapsed() <= LIVE,
        "the event came {:?} after the process's exit",
        exited.elapsed()
    );
    assert_eq!(
        frame.event.as_deref(),
        Some("proposal.created"),
        "{frame:?}"
    );
    let payload: Value = serde_json::from_str(frame.data.as_deref().unwrap()).unwrap();
    assert_eq!(payload["id"], json!(id), "{payload}");
}

// ------------------------------------------------------------ Rules

/// "Rules and edge cases": a lock held with no readable `daemon.json` is
/// the "unpublished" line (`spec serve` after its 2 s wait, the daemon
/// after its 500 ms); readers take the five keys and ignore others (the
/// lock decides, no reader checks the PID); a file missing a key is
/// unreadable; the lock released, the stale file is replaced by a start.
#[test]
fn rules_a_held_lock_decides_and_readers_take_the_five_keys() {
    let scratch = Scratch::new("serve-rules-lock");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let data = data_dir(&home);
    fs::create_dir_all(&data).unwrap();
    let lock_path = data.join("daemon.lock");
    let json_path = data.join("daemon.json");
    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    held.try_lock().expect("the test holds the lock");
    let unpublished = format!(
        "a daemon holds {}, but {} is unreadable: ",
        lock_path.display(),
        json_path.display()
    );

    let started = Instant::now();
    let line = refused(&home, &a, &["serve", "--port", "0"]);
    assert!(
        line.starts_with(&format!("spec serve: {unpublished}")),
        "{line}"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(1900),
        "spec serve waits 2 s for the file: {:?}",
        started.elapsed()
    );
    let started = Instant::now();
    let line = daemon_refused(&home, scratch.path(), &[&a]);
    assert!(
        line.starts_with(&format!("specengine-http: {unpublished}")),
        "{line}"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(450),
        "the daemon retries the lock 500 ms: {:?}",
        started.elapsed()
    );

    // A key missing: unreadable.
    let a_text = a.to_str().unwrap();
    let four = format!(
        "{{\"pid\": 999999, \"port\": 7, \"url\": \"http://127.0.0.1:7\", \"projects\": \
         [{{\"slug\": \"{A_SLUG}\", \"root\": {}}}]}}\n",
        json!(a_text)
    );
    fs::write(&json_path, &four).unwrap();
    let line = refused(&home, &a, &["serve"]);
    assert!(
        line.starts_with(&format!("spec serve: {unpublished}")) && line.contains("started"),
        "{line}"
    );
    // The five keys and others: read, the others ignored; the PID never
    // checked.
    let six = format!(
        "{{\"pid\": 999999, \"port\": 7, \"url\": \"http://127.0.0.1:7\", \"projects\": \
         [{{\"slug\": \"{A_SLUG}\", \"root\": {}, \"extra\": 1}}], \"started\": \
         \"2026-10-08T21:04:05Z\", \"version\": \"9\"}}\n",
        json!(a_text)
    );
    fs::write(&json_path, &six).unwrap();
    let run = run_product(&spec_bin(), &home, &a, &["serve"], b"");
    assert_eq!(
        (run.code, run.stdout.as_str(), run.stderr.as_str()),
        (
            0,
            format!(
                "already running: pid 999999\nserving {A_SLUG} {a_text}\nlistening \
                 http://127.0.0.1:7\n"
            )
            .as_str(),
            ""
        )
    );
    assert_eq!(
        fs::read_to_string(&json_path).unwrap(),
        six,
        "spec never writes it"
    );

    // The lock released: the stale file is replaced by a start.
    drop(held);
    let server = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    assert_eq!(daemon_json(&home)["pid"], json!(server.pid()));
    assert_eq!(fs::read(&lock_path).unwrap(), b"", "the lock never written");
}

/// "Rules and edge cases": the "served elsewhere" restart command names
/// every served root, then the new ones, each once, a root outside
/// `A-Za-z0-9/._-` single-quoted (`'` as `'\''`); a served root now under
/// another slug is not served.
#[test]
fn rules_the_restart_command_quotes_and_a_renamed_slug_is_not_served() {
    let scratch = Scratch::new("serve-rules-quote");
    let a = scratch.repo("spec-a", "a", "main");
    let odd = scratch.repo("spec-b", "b dir's", "main");
    let home = scratch.home("h");
    let server = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    let url = format!("http://127.0.0.1:{}", server.port);
    let odd_text = odd.to_str().unwrap();
    let quoted = format!("'{}'", odd_text.replace('\'', "'\\''"));
    assert!(quoted.contains("b dir'\\''s"), "{quoted}");
    let line = refused(&home, &odd, &["serve", "--port", "0"]);
    assert_eq!(
        line,
        format!(
            "spec serve: a daemon (pid {}, {url}) serves {A_SLUG} {}; stop it, then run: \
             spec serve {} {quoted} --port {}",
            server.pid(),
            a.display(),
            a.display(),
            server.port
        )
    );

    // A's slug renamed in its config: (new slug, A) is not served.
    let config = a.join("specengine.toml");
    let text = fs::read_to_string(&config).unwrap();
    assert_eq!(text.matches("slug = \"lantern-keep\"").count(), 1, "{text}");
    fs::write(
        &config,
        text.replace("slug = \"lantern-keep\"", "slug = \"lantern-two\""),
    )
    .unwrap();
    let line = refused(&home, &a, &["serve"]);
    assert_eq!(
        line,
        format!(
            "spec serve: a daemon (pid {}, {url}) serves {A_SLUG} {}; stop it, then run: \
             spec serve {} --port {}",
            server.pid(),
            a.display(),
            a.display(),
            server.port
        )
    );
}

// ------------------------------------------------- iteration 2: lines

/// `spec args`, which must exit 2 with nothing on stdout and one stderr
/// line: that line (the CLI's own wording of the same failure).
fn cli_line(home: &Path, cwd: &Path, args: &[&str]) -> String {
    let run = run_product(&spec_bin(), home, cwd, args, b"");
    assert_eq!(run.code, 2, "{args:?}: {}", run.show());
    assert!(run.stdout.is_empty(), "{args:?}: {}", run.show());
    assert_eq!(run.stderr.lines().count(), 1, "{args:?}: {}", run.show());
    run.stderr.trim_end_matches('\n').to_owned()
}

/// A DIR's discovery, config, slug and guard errors read `spec serve: DIR
/// <given>: <the CLI's line>` (the DIR as given, the CLI's `spec:` line
/// kept); without a DIR the CLI's line as it is; nothing created. M: the
/// DIR not named.
#[test]
fn a_dirs_error_names_the_dir_as_given_then_the_clis_line() {
    let scratch = Scratch::new("serve-dir-lines");
    let cwd = scratch.path();
    let a = scratch.repo("spec-a", "a", "main");
    scratch.dir("empty");
    let broken = scratch.copy("spec-b", "broken");
    common::write(&broken, "specengine.toml", "[project\nslug = \"zerkalo\"\n");
    let unslugged = scratch.copy("spec-b", "unslugged");
    let config = common::read_text(&unslugged, "specengine.toml");
    common::write(
        &unslugged,
        "specengine.toml",
        config.replace("slug = \"zerkalo\"\n", ""),
    );
    let home = scratch.home("h");
    let cli_home = scratch.home("cli");

    let missing = refused(&home, cwd, &["serve", "a", "nope", "--port", "0"]);
    assert_eq!(
        missing,
        "spec serve: DIR nope: spec: --root nope: No such file or directory (os error 2)"
    );
    assert_eq!(
        missing,
        format!(
            "spec serve: DIR nope: {}",
            cli_line(&cli_home, cwd, &["--root", "nope", "tree"])
        )
    );
    let no_config = refused(&home, cwd, &["serve", "./empty", "--port", "0"]);
    assert_eq!(
        no_config,
        "spec serve: DIR ./empty: spec: no specengine.toml in the --root directory; run \
         `spec init` there"
    );
    for (given, args) in [
        ("broken", ["--root", "broken", "search", "stamina"]),
        ("unslugged", ["--root", "unslugged", "search", "stamina"]),
    ] {
        let line = refused(&home, cwd, &["serve", "a", given, "--port", "0"]);
        assert_eq!(
            line,
            format!(
                "spec serve: DIR {given}: {}",
                cli_line(&cli_home, cwd, &args)
            ),
            "{given}"
        );
    }
    let broken_line = refused(&home, cwd, &["serve", "broken"]);
    assert!(
        broken_line.starts_with("spec serve: DIR broken: specengine.toml:1: "),
        "the config's file and line: {broken_line}"
    );
    let unslugged_line = refused(&home, cwd, &["serve", "unslugged"]);
    assert!(unslugged_line.contains("slug"), "{unslugged_line}");

    // The guard: HOME the copy A, so A's data directory lies inside it.
    let guard = refused(
        &a,
        cwd,
        &["serve", "broken/../unslugged/../a", "--port", "0"],
    );
    let cli_guard = cli_line(&a, cwd, &["--root", "a", "search", "stamina"]);
    assert_eq!(
        guard,
        format!("spec serve: DIR broken/../unslugged/../a: {cli_guard}")
    );
    assert!(
        cli_guard.starts_with("spec: the data directory ")
            && cli_guard.ends_with("nothing was created"),
        "{cli_guard}"
    );
    // Without a DIR (the walk, the global --root): the CLI's line as it is.
    assert_eq!(refused(&a, &a, &["serve", "--port", "0"]), cli_guard);
    assert_eq!(
        refused(&a, cwd, &["--root", "a", "serve", "--port", "0"]),
        cli_guard
    );

    assert!(snapshot(&home).is_empty(), "nothing created in HOME");
    assert_eq!(
        scratch
            .git()
            .run(&a, &["status", "--porcelain", "--ignored"]),
        ""
    );
}

/// Without a DIR, `spec serve` (which refuses `--config`) is not told to
/// pass one; every other command keeps the hint byte-identical; the walk's
/// line is the CLI's. M: the hint given to `serve`; the hint dropped for
/// the other commands.
#[test]
fn serve_is_never_told_to_pass_config_and_the_other_commands_still_are() {
    let scratch = Scratch::new("serve-hint");
    let cwd = scratch.path();
    let empty = scratch.dir("empty");
    let home = scratch.home("h");
    let empty_text = empty.to_str().unwrap();
    let bare = "spec: no specengine.toml in the --root directory; run `spec init` there";
    assert_eq!(
        refused(&home, cwd, &["--root", empty_text, "serve", "--port", "0"]),
        bare
    );
    for args in [
        vec!["--root", empty_text, "tree"],
        vec!["--root", empty_text, "search", "stamina"],
        vec!["--root", empty_text, "show", "MEC-STAMINA"],
    ] {
        assert_eq!(
            cli_line(&home, cwd, &args),
            format!("{bare}, or pass --config"),
            "{args:?}"
        );
    }
    let walk = cli_line(&home, &empty, &["tree"]);
    assert_eq!(
        walk,
        "spec: no specengine.toml in the current directory or any directory above it; run \
         `spec init` at the project root, or pass --root"
    );
    assert_eq!(refused(&home, &empty, &["serve", "--port", "0"]), walk);
    assert!(snapshot(&home).is_empty(), "nothing created");
}

// ------------------------------------------- iteration 2: daemon texts

/// `daemon.lock` of `home`'s data directory (created), locked exclusively
/// by this test until the returned file drops.
fn hold_lock(home: &Path) -> fs::File {
    let data = data_dir(home);
    fs::create_dir_all(&data).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join("daemon.lock"))
        .unwrap();
    held.try_lock().expect("the test holds the lock");
    held
}

/// Whether `text` carries a character a terminal would act on: ESC, BEL,
/// any other C0/C1 control but LF and TAB, a bidirectional override.
fn raw_control(text: &str) -> Option<char> {
    text.chars().find(|&c| {
        (c.is_control() && c != '\n' && c != '\t') || matches!(c, '\u{202a}'..='\u{202e}')
    })
}

/// Every text read from `daemon.json` (URL, slug, root, the restart
/// command's roots) is printed escaped (`\u{1b}`, `\u{7}`, `\u{202e}`) and
/// on one line, by `spec serve` (served elsewhere, already running,
/// unpublished) and by `specengine-http` (already runs). M: the texts
/// printed raw (`daemon_text` the identity); the daemon's URL printed raw.
#[test]
fn texts_from_daemon_json_are_printed_escaped_on_one_line() {
    let scratch = Scratch::new("serve-escape");
    let a = scratch.repo("spec-a", "a", "main");
    let a_text = a.to_str().unwrap();
    let home = scratch.home("h");
    let held = hold_lock(&home);
    let data = data_dir(&home);
    let json_path = data.join("daemon.json");
    let url = "http://127.0.0.1:7\u{1b}[2J\u{1b}]0;owned\u{7}\nX";
    let slug = "evil\u{1b}[31m\u{202e}kls";
    let root = "/tmp/r\u{1b}[0m\nnext";
    let crafted = json!({
        "pid": 4242,
        "port": 7,
        "url": url,
        "projects": [{"slug": slug, "root": root}],
        "started": "2026-10-08T21:04:05Z",
    });
    fs::write(&json_path, serde_json::to_vec_pretty(&crafted).unwrap()).unwrap();
    let shown_url = "http://127.0.0.1:7\\u{1b}[2J\\u{1b}]0;owned\\u{7} X";
    let shown_slug = "evil\\u{1b}[31m\\u{202e}kls";
    let shown_root = "/tmp/r\\u{1b}[0m next";

    // Served elsewhere: the restart command's crafted root escaped, then
    // shell-quoted.
    let line = refused(&home, &a, &["serve", "--port", "0"]);
    assert_eq!(raw_control(&line), None, "{line:?}");
    assert_eq!(
        line,
        format!(
            "spec serve: a daemon (pid 4242, {shown_url}) serves {shown_slug} {shown_root}; \
             stop it, then run: spec serve '{shown_root}' {a_text} --port 7"
        )
    );
    // The daemon's own refusal.
    let line = daemon_refused(&home, scratch.path(), &[&a]);
    assert_eq!(raw_control(&line), None, "{line:?}");
    assert_eq!(
        line,
        format!(
            "specengine-http: a daemon already runs under {}: pid 4242, {shown_url}, serving \
             {shown_slug} {shown_root}; stop it first",
            data.display()
        )
    );

    // Already running: the URL escaped, three lines.
    let serving_a = json!({
        "pid": 4242,
        "port": 7,
        "url": url,
        "projects": [{"slug": A_SLUG, "root": a_text}],
        "started": "2026-10-08T21:04:05Z",
    });
    fs::write(&json_path, serde_json::to_vec_pretty(&serving_a).unwrap()).unwrap();
    let run = run_product(&spec_bin(), &home, &a, &["serve"], b"");
    assert_eq!(raw_control(&run.stdout), None, "{:?}", run.stdout);
    assert_eq!(
        (run.code, run.stdout.as_str(), run.stderr.as_str()),
        (
            0,
            format!(
                "already running: pid 4242\nserving {A_SLUG} {a_text}\nlistening {shown_url}\n"
            )
            .as_str(),
            ""
        )
    );

    // Unpublished: whatever the reason quotes, one escaped line.
    fs::write(
        &json_path,
        "{\"pid\": \"\\u001b[2J\\u0007\\u202e\", \"port\": 7}\n",
    )
    .unwrap();
    let line = refused(&home, &a, &["serve", "--port", "0"]);
    assert_eq!(raw_control(&line), None, "{line:?}");
    assert!(line.starts_with("spec serve: a daemon holds "), "{line}");
    let line = daemon_refused(&home, scratch.path(), &[&a]);
    assert_eq!(raw_control(&line), None, "{line:?}");
    assert!(
        line.starts_with("specengine-http: a daemon holds "),
        "{line}"
    );
    drop(held);
}

// ------------------------------------------- iteration 2: probe retry

/// A holder of the exclusive lock that ends during `spec serve`'s 2 s
/// wait (no `daemon.json` ever written): the shared lock is tried again
/// before each read, so `spec serve` execs the daemon, which starts; no
/// "unpublished" line. The holder is a `perl` child that `flock`s the file
/// and exits by itself (alarm 20). M: the shared lock tried once only.
#[test]
fn a_lock_holder_that_ends_during_the_wait_lets_spec_serve_start() {
    use std::io::{BufRead as _, BufReader};
    use std::process::{Command, Stdio};

    let scratch = Scratch::new("serve-probe-retry");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let data = data_dir(&home);
    fs::create_dir_all(&data).unwrap();
    let lock_path = data.join("daemon.lock");
    fs::write(&lock_path, b"").unwrap();
    // A warm `spec` first: its first exec on macOS may be slow, and the
    // probe must meet the held lock.
    let warm = scratch.home("warm");
    let _ = refused(&warm, &a, &["serve", "--json"]);

    let hold = Duration::from_millis(1200);
    let mut holder = Command::new("/usr/bin/perl")
        .env_clear()
        .args([
            "-e",
            "alarm 20; use Fcntl ':flock'; open(my $f, '+<', $ARGV[0]) or die \"open: $!\"; \
             flock($f, LOCK_EX | LOCK_NB) or die \"flock: $!\"; $| = 1; print \"locked\\n\"; \
             select(undef, undef, undef, $ARGV[1]); exit 0;",
        ])
        .arg(&lock_path)
        .arg(format!("{}", hold.as_secs_f64()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("the holder starts");
    let mut first = String::new();
    BufReader::new(holder.stdout.take().unwrap())
        .read_line(&mut first)
        .expect("the holder's line");
    assert_eq!(first, "locked\n", "the holder took the lock");
    let locked_at = Instant::now();
    let probe = fs::File::open(&lock_path).unwrap();
    assert!(
        matches!(probe.try_lock_shared(), Err(fs::TryLockError::WouldBlock)),
        "the holder's flock is the one spec serve probes"
    );
    drop(probe);
    assert!(
        matches!(holder.try_wait(), Ok(None)),
        "the holder still holds the lock when spec serve starts"
    );

    let mut server = serving(&home, &a, &["serve", "--port", "0"], &[(A_SLUG, &a)]);
    let waited = locked_at.elapsed();
    let ended = holder.wait().expect("the holder ends by itself");
    assert!(ended.success(), "{ended:?}");
    assert!(
        waited >= hold - Duration::from_millis(100),
        "spec serve started only once the holder ended: {waited:?}"
    );
    assert!(
        !server.stderr().contains("unreadable"),
        "{}",
        server.stderr()
    );
    assert!(server.alive());
    assert_eq!(daemon_json(&home)["pid"], json!(server.pid()));
}
