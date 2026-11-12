//! docs/features/node-history.md (RM-0035) "Acceptance criteria", the
//! daemon's half: `GET /api/projects/<slug>/nodes/{*ref}?with=history` over
//! scratch git repositories built at run time (an invented rules book:
//! `docs/rules.md` holding `{#R-12}` and `{#R-13}`), every commit with an
//! explicit author and committer, the apply ones as `commit_message_raising`
//! writes them.
//!
//! - NH12: `?with=history` (alone, with `links` and `bindings`, either
//!   order) is the CLI's `spec --root R show REF --history [...] --json`
//!   byte for byte: R-12's entries D, B, A newest first, the document's
//!   every commit; `with=<other>` is a 400 naming the three values, before
//!   any call; not asked, `history` is `null`. M: `with=history` refused.
//! - NH10, the daemon's half: a history past the CLI's cut (64 entries
//!   under a 470-character `Decided-by:`) is served whole, 64 entries,
//!   `omitted` 0, where the CLI's JSON holds fewer. M: the daemon cut as
//!   the terminal is.
//!
//! The key sets (`ShownNode` 21 keys, `ShownHistory`, `HistoryEntry`) are
//! `daemon_keys.rs`'s.

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::{Scratch, Server, beyond_daemon, spec_json};
use git::Sandbox;
use serde_json::{Value, json};

const BASE: &str = "/api/projects/quill-rules";

const CONFIG: &str = "[project]\nslug = \"quill-rules\"\n\n[paths]\nroots = [\"docs\"]\n\n\
                      [ids]\nR = { kind = \"requirement\", width = 2 }\n";

const RULES: &str = "docs/rules.md";

fn rules(window: &str) -> String {
    format!(
        "# Rules\n\nThe house rules.\n\n{window}\n\n## Door {{#R-13}}\n\nThe door shuts at dusk.\n"
    )
}

/// One commit of `root` as `name <email>` (author and committer) at
/// `date`, `message` verbatim: its OID.
fn commit(
    git: &Sandbox,
    scratch: &Scratch,
    root: &Path,
    who: (&str, &str),
    date: &str,
    message: &str,
) -> String {
    let file = scratch.join("message.txt");
    fs::write(&file, message).expect("message");
    git.add_all(root);
    let env: Vec<(&str, OsString)> = vec![
        ("GIT_AUTHOR_NAME", who.0.into()),
        ("GIT_AUTHOR_EMAIL", who.1.into()),
        ("GIT_AUTHOR_DATE", date.into()),
        ("GIT_COMMITTER_NAME", who.0.into()),
        ("GIT_COMMITTER_EMAIL", who.1.into()),
        ("GIT_COMMITTER_DATE", date.into()),
    ];
    let extra: Vec<(&str, &OsStr)> = env.iter().map(|(k, v)| (*k, v.as_os_str())).collect();
    git.git_env(
        root,
        &[
            "commit",
            "-q",
            "--no-verify",
            "--cleanup=verbatim",
            "-F",
            file.to_str().expect("UTF-8"),
        ],
        &extra,
    );
    git.git_text(root, &["rev-parse", "HEAD"])
}

/// A hand commit creating both sections (2026-01-01), an apply of PR-0001
/// raising R-12 1 -> 2 decided by Ann Owner (2026-01-02), a hand edit of
/// R-12 (2026-01-04): the root and the OIDs, oldest first.
fn book(scratch: &Scratch) -> (PathBuf, Vec<String>) {
    let git = Sandbox::new(scratch.path());
    let root = scratch.dir("repo");
    git.init(&root);
    common::write(&root, "specengine.toml", CONFIG);
    common::write(
        &root,
        RULES,
        rules("## Window {#R-12}\n\nThe window opens at noon."),
    );
    let bob = ("Bob Hand", "bob@example.org");
    let a = commit(
        &git,
        scratch,
        &root,
        bob,
        "2026-01-01T10:00:00+0000",
        "Add the rules\n",
    );
    common::write(
        &root,
        RULES,
        rules("## Window {#R-12 rev=2}\n\nThe window opens at noon for ten minutes."),
    );
    let b = commit(
        &git,
        scratch,
        &root,
        ("Cleo Commit", "cleo@example.org"),
        "2026-01-02T10:00:00+0000",
        &format!(
            "spec: apply PR-0001\n\nTighten the window.\n\nProposal: PR-0001\nDecided-by: Ann \
             Owner <ann@example.org>\nProposed-by: agent role=spec-writer model=m run=r\n\
             Base-commit: {a}\nRevision: R-12 1 -> 2\n"
        ),
    );
    common::write(
        &root,
        RULES,
        rules("## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes."),
    );
    let d = commit(
        &git,
        scratch,
        &root,
        bob,
        "2026-01-04T10:00:00+0000",
        "Fix a typo in the window rule\n",
    );
    (root, vec![a, b, d])
}

/// Each entry's `id` of the first node.
fn ids(document: &Value) -> Vec<String> {
    document["nodes"][0]["history"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("entries: {document}"))
        .iter()
        .map(|entry| entry["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// NH12: the daemon's node route with `with=history` is the CLI's
/// document byte for byte; `with=bogus` a 400 naming the three values
/// before any call. M: `with=history` refused.
#[test]
fn nh12_the_daemon_serves_the_history() {
    let scratch = Scratch::new("nh12");
    let (root, oids) = book(&scratch);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    for reference in ["R-12", RULES, "R-13"] {
        let encoded = common::encode_component(reference);
        for (query, flags) in [
            ("with=history", &["--history"][..]),
            (
                "with=links&with=bindings&with=history",
                &["--links", "--bindings", "--history"],
            ),
            ("with=history&with=links", &["--links", "--history"]),
            ("", &[]),
        ] {
            let mut args = vec!["show", reference];
            args.extend_from_slice(flags);
            let run = spec_json(&home, &cwd, &root, &args);
            run.code(0);
            let reply = server.get(&format!("{BASE}/nodes/{encoded}?{query}"));
            assert_eq!(reply.status, 200, "{reference} {query}: {}", reply.text());
            assert_eq!(
                reply.text(),
                run.document(),
                "{reference} {query}: the CLI's document"
            );
            let history = &reply.json()["nodes"][0]["history"];
            if query.is_empty() {
                assert_eq!(*history, Value::Null, "{reference}: not asked");
            } else {
                assert_eq!(history["omitted"], json!(0), "{reference} {query}");
            }
        }
    }
    let r12 = server
        .get(&format!("{BASE}/nodes/R-12?with=history"))
        .json();
    let newest_first: Vec<String> = oids.iter().rev().cloned().collect();
    assert_eq!(ids(&r12), newest_first, "D, B, A: {r12}");
    let changes: Vec<&str> = r12["nodes"][0]["history"]["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| entry["change"].as_str().expect("change"))
        .collect();
    assert_eq!(changes, ["hand", "raised", "hand"]);
    assert_eq!(
        r12["nodes"][0]["history"]["entries"][1]["by_name"],
        json!("Ann Owner")
    );

    let fresh = scratch.home("fresh");
    let other = Server::serve(&fresh, &cwd, &[&root]);
    for value in ["bogus", "History", "tasks", ""] {
        let reply = other.get(&format!("{BASE}/nodes/R-12?with={value}"));
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

/// NH10, the daemon's half: 64 apply commits under a 470-character
/// `Decided-by:` pass the terminal's cut; the daemon serves all 64 entries,
/// `omitted` 0, while `spec show --history --json` holds fewer and counts
/// the rest. M: the daemon cut as the terminal is.
#[test]
fn nh10_the_daemon_serves_every_entry() {
    let scratch = Scratch::new("nh10-daemon");
    let git = Sandbox::new(scratch.path());
    let root = scratch.dir("repo");
    git.init(&root);
    let decider = format!(
        "{} <ann@example.org>",
        "Ann Owner of the Long Name".repeat(18)
    );
    let mut stream = Vec::new();
    for n in 0..64_usize {
        let message = if n == 0 {
            "Add the rules\n".to_owned()
        } else {
            let mut rationale = format!("Step {n:02}:");
            while rationale.len() < 300 {
                rationale.push_str(" tighten");
            }
            rationale.truncate(300);
            format!(
                "spec: apply PR-{n:04}\n\n{rationale}\n\nProposal: PR-{n:04}\nDecided-by: \
                 {decider}\nProposed-by: agent role=spec-writer model=m run=r\nBase-commit: {}\n",
                "0".repeat(40)
            )
        };
        let time = 1_767_225_600 + 60 * i64::try_from(n).expect("small");
        stream.extend_from_slice(
            format!(
                "commit refs/heads/main\nmark :{}\nauthor Bob Hand <bob@example.org> {time} \
                 +0000\ncommitter Dana Desk <dana@example.org> {time} +0000\ndata {}\n{message}\n",
                n + 1,
                message.len()
            )
            .as_bytes(),
        );
        if n > 0 {
            stream.extend_from_slice(format!("from :{n}\n").as_bytes());
        }
        let mut files = vec![(
            RULES.to_owned(),
            rules(&format!(
                "## Window {{#R-12}}\n\nThe window opens at minute {n}."
            )),
        )];
        if n == 0 {
            files.push(("specengine.toml".to_owned(), CONFIG.to_owned()));
        }
        for (path, content) in files {
            stream.extend_from_slice(
                format!(
                    "M 100644 inline {path}\ndata {}\n{content}\n",
                    content.len()
                )
                .as_bytes(),
            );
        }
        stream.push(b'\n');
    }
    git.git_stdin(&root, &["fast-import", "--quiet"], &stream);
    git.git(&root, &["reset", "-q", "--hard", "main"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let cli = spec_json(&home, &cwd, &root, &["show", "R-12", "--history"]);
    cli.code(0);
    let cut = cli.json();
    let printed = cut["nodes"][0]["history"]["entries"]
        .as_array()
        .expect("entries")
        .len();
    assert!(printed < 64, "the terminal cuts: {printed}");
    assert_eq!(
        cut["nodes"][0]["history"]["omitted"],
        json!(64 - printed),
        "the CLI counts the rest"
    );
    let server = Server::serve(&home, &cwd, &[&root]);
    let served = server.get(&format!("{BASE}/nodes/R-12?with=history"));
    assert_eq!(served.status, 200, "{}", served.text());
    let served = served.json();
    let history = &served["nodes"][0]["history"];
    assert_eq!(
        history["entries"].as_array().expect("entries").len(),
        64,
        "M: the daemon cut"
    );
    assert_eq!(history["omitted"], json!(0));
    assert_eq!(history["truncated"], json!(false));
    let all = history["entries"].as_array().expect("entries");
    let shown = cut["nodes"][0]["history"]["entries"]
        .as_array()
        .expect("entries");
    assert_eq!(
        &all[..printed],
        &shown[..],
        "the CLI's entries are the newest"
    );
}
