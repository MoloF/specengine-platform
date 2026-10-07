//! AC-03 of docs/features/daemon-read.md: one door. On a repository with
//! an open `update`, every endpoint, the decision POST too, leaves `git
//! status --porcelain` empty, `HEAD`, the branches, the files, the
//! proposal's state and the queue's `events` unchanged; the POST is a 403
//! naming `spec approve PR-0001`; no call of `approve`, `reject`,
//! `propose`, `import_state`, `export_`, `init`, `index` in
//! `crates/specengine-http/src`. M: the POST approves with an always-yes
//! consent.
//!
//! AC-07 of docs/features/ui-live.md, the door amended ("Data", "The one
//! door, amended"): `graph` and the plain `check` join the endpoints that
//! change nothing; `check` leaves the forbidden names, `Staged`, `Changed`
//! and `baseline` join them (a git mode or a client's file named anywhere
//! in `src`, a field access included), each caught in a probe (positive
//! controls: `CheckedTree::Changed`, `CheckedTree::Staged`, a `baseline`
//! field written or set, and every writer of the old list), the plain
//! `check(…, &CheckRequest::default())` call not. M: a handler calls
//! `index`.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Scratch, Server, Stream, propose_update, repository_root, snapshot, spec_json};
use serde_json::json;
use specengine_cli::{Env, EventLine, Globals, events_after};

/// The project's events as stored, read through the CLI library under
/// `home` (not through the daemon).
fn events(home: &Path, root: &Path) -> Vec<EventLine> {
    let env = Env {
        cwd: root.to_path_buf(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(root.to_path_buf()),
        config: None,
    };
    events_after(&env, &globals, Some(0))
        .expect("the events read")
        .events
}

/// Everything the AC names about the repository and the queue.
#[derive(Debug, PartialEq, Eq)]
struct State {
    porcelain: String,
    head: String,
    refs: String,
    worktrees: String,
    files: Vec<(String, Option<Vec<u8>>)>,
    proposal: serde_json::Value,
    events: Vec<(i64, String, String)>,
}

fn state(scratch: &Scratch, home: &Path, cwd: &Path, root: &Path) -> State {
    let git = scratch.git();
    let mut proposal = spec_json(home, cwd, root, &["review", "PR-0001"]).json();
    // The review's preview is computed now; its stored state is what
    // counts.
    proposal.as_object_mut().unwrap().retain(|key, _| {
        [
            "id",
            "status",
            "decided_by",
            "decided_at",
            "decision_note",
            "applied_commit",
            "updated_at",
            "base_hash",
            "new_text",
        ]
        .contains(&key.as_str())
    });
    State {
        porcelain: git.run(root, &["status", "--porcelain", "--untracked-files=all"]),
        head: git.run(root, &["rev-parse", "HEAD"]),
        refs: git.run(root, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        worktrees: git.run(root, &["worktree", "list", "--porcelain"]),
        files: snapshot(root)
            .into_iter()
            .filter(|(path, _)| path != ".git/index")
            .collect(),
        proposal,
        events: events(home, root)
            .into_iter()
            .map(|event| (event.seq, event.event_type, event.payload))
            .collect(),
    }
}

#[test]
fn ac03_no_endpoint_changes_the_repository_or_the_queue() {
    let scratch = Scratch::new("door-endpoints");
    let a = scratch.repo("spec-a", "a", "main");
    // An identity, as an owner's repository has one: a decision taken
    // through the daemon could commit.
    let git = scratch.git();
    git.run(&a, &["config", "user.name", "Owner"]);
    git.run(&a, &["config", "user.email", "owner@example.invalid"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let id = propose_update(
        &home,
        &cwd,
        &a,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.2 s.\n",
    );
    assert_eq!(id, "PR-0001");
    let before = state(&scratch, &home, &cwd, &a);
    assert_eq!(before.porcelain, "", "the copy is committed");
    assert_eq!(before.proposal["status"], json!("open"));
    assert_eq!(before.events.len(), 1, "{:?}", before.events);
    assert_eq!(before.events[0].1, "proposal.created");

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    for path in [
        "/api/projects".to_owned(),
        format!("{p}/tree"),
        format!("{p}/tree?depth=1&archive=true"),
        format!("{p}/nodes/MEC-STAMINA"),
        format!("{p}/nodes/EDGE-STAM-ZERO?with=links&archive=true"),
        format!("{p}/nodes/docs%2Fspec%2Fmovement%2Fstamina.md"),
        format!("{p}/search?query=stamina"),
        format!("{p}/bundle?node_ids=EDGE-STAM-ZERO"),
        format!("{p}/graph?ref=MEC-STAMINA"),
        format!("{p}/graph?ref=EDGE-STAM-ZERO&impact=true&types=depends_on&depth=1&archive=true"),
        format!("{p}/check"),
        format!("{p}/inbox"),
        format!("{p}/proposals/PR-0001"),
        format!("{p}/proposals/PR-0002"),
    ] {
        let reply = server.get(&path);
        assert!(
            reply.status == 200 || reply.status == 404,
            "{path}: {}",
            reply.text()
        );
    }
    // The tail, from the start: the one stored event, then nothing.
    let mut stream = Stream::open(
        server.port,
        &format!("{p}/events"),
        &[("Last-Event-ID", "0")],
    );
    assert_eq!(stream.status, 200);
    let event = stream
        .next_event(Duration::from_secs(5))
        .expect("the stored event");
    assert_eq!(event.event.as_deref(), Some("proposal.created"));
    drop(stream);

    // The decision POST: refused, its body unread, naming the terminal.
    for body in ["", "{\"decision\":\"approve\",\"consent\":true}"] {
        let length = body.len().to_string();
        let host = format!("127.0.0.1:{}", server.port);
        let mut bytes = common::request_bytes(
            "POST",
            &format!("{p}/proposals/PR-0001/decision"),
            &[
                ("Host", &host),
                ("Content-Type", "application/json"),
                ("Content-Length", &length),
            ],
        );
        bytes.extend_from_slice(body.as_bytes());
        let reply = server.raw(&bytes);
        assert_eq!(reply.status, 403, "{}", reply.text());
        let message = reply.error_message();
        assert_eq!(
            message,
            format!(
                "decisions are made on a terminal: `spec approve PR-0001` or `spec reject PR-0001 \
                 --reason \u{2026}` in {}; nothing changed",
                a.display()
            )
        );
    }
    // Not `PR-` and digits: `PR-…`.
    let reply = server.request("POST", &format!("{p}/proposals/approve-all/decision"), &[]);
    assert_eq!(reply.status, 403);
    assert!(
        reply.error_message().contains("`spec approve PR-\u{2026}`"),
        "{}",
        reply.text()
    );
    // Other methods on the decision: 405; a POST on a read: 405.
    for (method, path) in [
        ("GET", format!("{p}/proposals/PR-0001/decision")),
        ("PUT", format!("{p}/proposals/PR-0001/decision")),
        ("POST", format!("{p}/proposals/PR-0001")),
        ("POST", format!("{p}/inbox")),
    ] {
        let reply = server.request(method, &path, &[]);
        assert_eq!(reply.status, 405, "{method} {path}: {}", reply.text());
        reply.error_message();
    }
    drop(server);

    let after = state(&scratch, &home, &cwd, &a);
    assert_eq!(
        after, before,
        "nothing under the root or in the queue changed"
    );
}

/// `text` with comments, string, char and byte literals blanked out.
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            at += 2;
            while at + 1 < chars.len() && !(chars[at] == '*' && chars[at + 1] == '/') {
                at += 1;
            }
            at += 2;
            out.push(' ');
            continue;
        }
        // Raw strings: r"…", r#"…"#, br#"…"#.
        if c == 'r' && (next == Some('"') || next == Some('#')) {
            let mut hashes = 0;
            let mut probe = at + 1;
            while chars.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if chars.get(probe) == Some(&'"') {
                at = probe + 1;
                loop {
                    if at >= chars.len() {
                        break;
                    }
                    if chars[at] == '"'
                        && (0..hashes).all(|offset| chars.get(at + 1 + offset) == Some(&'#'))
                    {
                        at += 1 + hashes;
                        break;
                    }
                    at += 1;
                }
                out.push_str("\"\"");
                continue;
            }
        }
        if c == '"' {
            at += 1;
            while at < chars.len() && chars[at] != '"' {
                if chars[at] == '\\' {
                    at += 1;
                }
                at += 1;
            }
            at += 1;
            out.push_str("\"\"");
            continue;
        }
        // A char literal ('x', '\n', '\u{…}'), not a lifetime.
        if c == '\'' {
            if next == Some('\\') {
                let mut probe = at + 2;
                while probe < chars.len() && chars[probe] != '\'' {
                    probe += 1;
                }
                at = probe + 1;
                out.push_str("' '");
                continue;
            }
            if chars.get(at + 2) == Some(&'\'') {
                at += 3;
                out.push_str("' '");
                continue;
            }
        }
        out.push(c);
        at += 1;
    }
    out
}

/// Every whole identifier in `code`, with the character before it.
fn identifiers(code: &str) -> Vec<(char, String)> {
    let mut found = Vec::new();
    let mut current = String::new();
    let mut before = ' ';
    let mut previous = ' ';
    for c in code.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' {
            if current.is_empty() {
                before = previous;
            }
            current.push(c);
        } else if !current.is_empty() {
            found.push((before, std::mem::take(&mut current)));
        }
        if !c.is_whitespace() {
            previous = c;
        }
    }
    found
}

fn sources() -> Vec<(PathBuf, String)> {
    let src = repository_root().join("crates/specengine-http/src");
    let mut files = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = fs::read_to_string(&path).expect("source");
                files.push((path, text));
            }
        }
    }
    files.sort();
    assert!(files.len() >= 2, "the daemon's sources: {files:?}");
    files
}

/// The names no handler may call: the CLI's deciding, storing, exporting,
/// importing, creating and indexing calls (`check` left the list with
/// docs/features/ui-live.md: the plain run is a read).
fn forbidden(name: &str) -> bool {
    [
        "approve",
        "approve_with",
        "reject",
        "propose",
        "propose_brief",
        "propose_question",
        "propose_discrepancy",
        "import_state",
        "init",
        "index",
    ]
    .contains(&name)
        || name.starts_with("export_")
}

/// The names `src` may not hold at all, a field access included: a check's
/// git mode (`CheckedTree::Staged`, `CheckedTree::Changed`) or a client's
/// baseline file (docs/features/ui-live.md "The one door, amended").
fn not_plain(name: &str) -> bool {
    ["Staged", "Changed", "baseline"].contains(&name)
}

/// What the scan reports in `code`: a forbidden call not after a `.`, a
/// name of a check that is not the plain run anywhere.
fn caught(code: &str) -> Vec<String> {
    identifiers(&code_only(code))
        .into_iter()
        .filter(|(before, name)| (*before != '.' && forbidden(name)) || not_plain(name))
        .map(|(_, name)| name)
        .collect()
}

#[test]
fn ac03_the_daemon_names_no_deciding_or_writing_call() {
    let mut named = Vec::new();
    for (path, text) in sources() {
        // A method or a field of something else (`.index`) is no CLI call;
        // a path segment or a bare name is. A check's mode or baseline is
        // caught wherever it is named.
        for name in caught(&text) {
            named.push(format!("{}: {name}", path.display()));
        }
        for crate_name in ["rusqlite", "specengine_store"] {
            assert!(
                !code_only(&text).contains(crate_name),
                "{}: names {crate_name}: reads go through the CLI library",
                path.display()
            );
        }
    }
    assert!(
        named.is_empty(),
        "calls of the one door's writers: {named:#?}"
    );

    // The scan sees a call: the 403's text names `spec approve` in a
    // string, which is no call.
    let probe =
        "fn f() { let s = \"spec approve PR-1\"; // approve(x)\n specengine_cli::approve(&e); }";
    assert_eq!(caught(probe), ["approve"]);
}

#[test]
fn ac07_the_scan_catches_a_checks_git_mode_or_baseline_and_every_writer() {
    // Positive controls: a check that is not the plain run.
    for (probe, want) in [
        (
            "async fn check() { let request = CheckRequest { tree: \
             CheckedTree::Changed(process_git(env)), ..CheckRequest::default() }; }",
            vec!["Changed"],
        ),
        (
            "fn f() { specengine_cli::check(env, globals, &CheckRequest { tree: \
             specengine_cli::CheckedTree::Staged(git), ..Default::default() }) }",
            vec!["Staged"],
        ),
        (
            "fn f() { let r = CheckRequest { baseline: Some(path), ..CheckRequest::default() }; }",
            vec!["baseline"],
        ),
        (
            "fn f() { let mut r = CheckRequest::default(); r.baseline = Some(p.into()); }",
            vec!["baseline"],
        ),
    ] {
        assert_eq!(caught(probe), want, "{probe}");
    }
    // The old list still caught, called by path or bare.
    for name in [
        "approve",
        "approve_with",
        "reject",
        "propose",
        "propose_brief",
        "propose_question",
        "propose_discrepancy",
        "import_state",
        "init",
        "index",
        "export_index",
        "export_state",
    ] {
        let by_path = format!("fn f() {{ let _ = specengine_cli::{name}(&env, &globals, &r); }}");
        assert_eq!(caught(&by_path), [name], "{by_path}");
        let bare = format!("fn f() {{ {name}(&env, &globals, &r)?; }}");
        assert_eq!(caught(&bare), [name], "{bare}");
    }
    // Negative controls: the plain run, and the names in a string or a
    // comment, are no finding.
    for probe in [
        "async fn check() { args(\"check\", &[], &uri)?; \
         specengine_cli::check(env, globals, &CheckRequest::default()).map(Outcome::Check) }",
        "fn f() { let s = \"Staged Changed baseline index\"; } // CheckedTree::Changed, baseline",
        "/* CheckRequest { baseline: Some(p) } */ fn f() { let n = list.index; }",
    ] {
        assert_eq!(caught(probe), Vec::<String>::new(), "{probe}");
    }
}
