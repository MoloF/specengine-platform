//! AC-03 of docs/features/daemon-read.md: one door. On a repository with
//! an open `update`, every endpoint, the decision POST too, leaves `git
//! status --porcelain` empty, `HEAD`, the branches, the files, the
//! proposal's state and the queue's `events` unchanged; the POST (the
//! stage route since docs/features/decision-staging.md: its decision-POST
//! block rewritten) refused from no page (403), a body that is no stage
//! (400), an ID that is none (404), a DELETE with nothing staged 200 and
//! no event; no call of `approve`, `reject`,
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
//!
//! AC-05 of docs/features/ui-live-tasks.md, the door's half: `tasks` and
//! `tasks/:id` join the endpoints that change nothing; every `task_` name
//! of the CLI library but `task_list` and `task_show` joins the forbidden
//! calls (a task is listed or shown, never moved), each caught in a probe
//! by path and bare (`task_claim`, `task_approve` among them), the two
//! reads not. M: a handler calls `task_claim`.
//!
//! T-02 of docs/features/ui-round.md, the door's half: `GET round` and a
//! refused `POST round/answers` (no page, another type, a body that is no
//! answers, another project, 201 answers, every answer refused) change
//! nothing; a valid POST changes only the item's stage and adds its one
//! `proposal.staged`. `round_answer` joins the forbidden calls and
//! `RoundTerminal`, `WalkOutcome`, `AnswerOutcome` the names `src` may
//! not hold at all (no handler walks a round, owns a terminal or matches
//! a round outcome variant: "Rules and edge cases"), each caught in a
//! probe by path and bare; `round_new`, the staging call
//! `round_stage_answers` and the handler `round_answers` not. M: the
//! handler calls `round_answer` with an always-yes terminal.
//!
//! AC-T3 of docs/features/kind-interpretation.md, the daemon's half: an
//! interpretation raised over MCP (`propose_change`, kind
//! `interpretation`, a separate `specengine-mcp` process) before the
//! endpoints are called leaves `git status` empty and the files as
//! committed; it is served (`proposals/PR-0002`, its `interpretation`) and
//! no endpoint changes it. M: a file write in propose.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{
    Mcp, Scratch, Server, Stream, ask, propose_update, repository_root, run_product, snapshot,
    spec_bin, spec_json,
};
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
    let committed = snapshot(&a);
    // kind-interpretation AC-T3: an interpretation over MCP.
    let mut mcp = Mcp::start(&home, &a);
    let shown = mcp.request(
        1,
        "tools/call",
        json!({"name": "get_node", "arguments": {"id": "RULE-STAM-REGEN"}}),
    );
    let node = &shown["result"]["structuredContent"]["nodes"][0];
    let base = node["span_hash"].as_str().expect("a span_hash").to_owned();
    let text = node["text"].as_str().expect("a text").replacen(
        "Base rate 10 units/s,",
        "Base rate 10 units/s at rest,",
        1,
    );
    let reply = mcp.request(
        2,
        "tools/call",
        json!({"name": "propose_change", "arguments": {"kind": "interpretation",
            "target": "RULE-STAM-REGEN", "base": base, "decision": "DEC-0023",
            "how": "Rest is any speed under 40 % of walking.",
            "why": "DEC-0023 says rest; the rule names no speed.",
            "text": text, "rationale": "Carry DEC-0023 into the rule.",
            "author_role": "spec-author"}}),
    );
    mcp.finish();
    assert_eq!(
        reply["result"]["structuredContent"]["id"],
        json!("PR-0002"),
        "{reply}"
    );
    assert_eq!(
        reply["result"]["structuredContent"]["interpretation"]["decision"],
        json!("DEC-0023"),
        "{reply}"
    );
    assert_eq!(snapshot(&a), committed, "nothing under the root written");
    let before = state(&scratch, &home, &cwd, &a);
    assert_eq!(before.porcelain, "", "the copy is committed");
    assert_eq!(before.proposal["status"], json!("open"));
    assert_eq!(before.events.len(), 2, "{:?}", before.events);
    assert_eq!(before.events[0].1, "proposal.created");
    assert_eq!(before.events[1].1, "proposal.created");

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
        format!("{p}/tasks"),
        format!("{p}/tasks?status=draft&status=ready"),
        format!("{p}/tasks/T-0001"),
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

    // The decision POST (the stage route, docs/features/decision-staging.md
    // "Daemon"): from no page refused before its body is read (403); from a
    // same-origin page a body that is no stage refused with the CLI's line
    // (400); an ID that is no proposal's 404; DELETE with nothing staged
    // 200, no event. Nothing stored by any of them.
    let host = format!("127.0.0.1:{}", server.port);
    let post = |path: &str, site: Option<&str>, body: &str| {
        let length = body.len().to_string();
        let mut headers = vec![
            ("Host", host.as_str()),
            ("Content-Type", "application/json"),
            ("Content-Length", length.as_str()),
        ];
        if let Some(site) = site {
            headers.push(("Sec-Fetch-Site", site));
        }
        let mut bytes = common::request_bytes("POST", path, &headers);
        bytes.extend_from_slice(body.as_bytes());
        server.raw(&bytes)
    };
    let decision = format!("{p}/proposals/PR-0001/decision");
    for body in ["", "{\"decision\":\"approve\",\"consent\":true}"] {
        let reply = post(&decision, None, body);
        assert_eq!(reply.status, 403, "{}", reply.text());
        assert_eq!(
            reply.error_message(),
            "staging needs a same-origin page (not authentication: ADR-0034)"
        );
        let reply = post(&decision, Some("same-origin"), body);
        assert_eq!(reply.status, 400, "{body:?}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.starts_with("spec: the stage is not `{\"decision\":\"approve\"")
                && message.ends_with("; nothing changed"),
            "{body:?}: {message}"
        );
    }
    // Not `PR-` and digits: no proposal, as the CLI judges it.
    let updated_at = before.proposal["updated_at"].as_str().unwrap().to_owned();
    let stage = format!("{{\"decision\":\"approve\",\"updated_at\":\"{updated_at}\"}}");
    let reply = post(
        &format!("{p}/proposals/approve-all/decision"),
        Some("same-origin"),
        &stage,
    );
    assert_eq!(reply.status, 404, "{}", reply.text());
    assert_eq!(
        reply.json()["notes"]
            .as_array()
            .and_then(|notes| notes.last()),
        Some(&json!(
            "no proposal `approve-all`: a proposal ID is `PR-` and 4 or more digits, as `spec \
             inbox` lists it"
        )),
        "{}",
        reply.text()
    );
    // DELETE: from no page 403; from the page, nothing staged: 200, the
    // document, no event.
    let reply = server.request("DELETE", &decision, &[]);
    assert_eq!(reply.status, 403, "{}", reply.text());
    let reply = server.request("DELETE", &decision, &[("Sec-Fetch-Site", "same-origin")]);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["id"], json!("PR-0001"));
    assert_eq!(reply.json()["staged"], json!(null));
    // Other methods on the decision: 405; a POST on a read: 405.
    for (method, path) in [
        ("GET", format!("{p}/proposals/PR-0001/decision")),
        ("PUT", format!("{p}/proposals/PR-0001/decision")),
        ("POST", format!("{p}/proposals/PR-0001")),
        ("POST", format!("{p}/inbox")),
        ("POST", format!("{p}/tasks")),
        ("POST", format!("{p}/tasks/T-0001")),
        ("PUT", format!("{p}/tasks/T-0001")),
    ] {
        let reply = server.request(method, &path, &[]);
        assert_eq!(reply.status, 405, "{method} {path}: {}", reply.text());
        reply.error_message();
    }
    // The interpretation, served as raised.
    let served = server
        .get(&format!("{p}/proposals/PR-0002"))
        .status(200)
        .json();
    assert_eq!(served["kind"], json!("interpretation"));
    assert_eq!(served["status"], json!("open"));
    assert_eq!(served["interpretation"]["decision"], json!("DEC-0023"));
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

/// The CLI library's task calls a handler may make: the two reads
/// (docs/features/ui-live-tasks.md "Description and interactions").
const TASK_READS: [&str; 2] = ["task_list", "task_show"];

/// The names no handler may call: the CLI's deciding, storing, exporting,
/// importing, creating and indexing calls (`check` left the list with
/// docs/features/ui-live.md: the plain run is a read), and every `task_`
/// call but the two reads (docs/features/ui-live-tasks.md AC-05).
fn forbidden(name: &str) -> bool {
    if name.starts_with("task_") {
        return !TASK_READS.contains(&name);
    }
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
        "round_answer",
    ]
    .contains(&name)
        || name.starts_with("export_")
}

/// The names `src` may not hold at all, a field access included: a check's
/// git mode (`CheckedTree::Staged`, `CheckedTree::Changed`) or a client's
/// baseline file (docs/features/ui-live.md "The one door, amended"); the
/// owner's terminal of a round and a round's outcome variants
/// (docs/features/ui-round.md "Rules and edge cases").
fn not_plain(name: &str) -> bool {
    [
        "Staged",
        "Changed",
        "baseline",
        "RoundTerminal",
        "WalkOutcome",
        "AnswerOutcome",
    ]
    .contains(&name)
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

#[test]
fn ac05_the_scan_catches_every_task_call_but_list_and_show() {
    // Positive controls: every task call of the CLI library that moves a
    // task, by path and bare; a name the library may add later too.
    for name in [
        "task_claim",
        "task_approve",
        "task_new",
        "task_plan",
        "task_changes",
        "task_cancel",
        "task_report",
        "task_complete",
        "task_refresh",
    ] {
        let by_path = format!(
            "fn f() {{ let _ = specengine_cli::{name}(&env, &globals, &request, &mut yes); }}"
        );
        assert_eq!(caught(&by_path), [name], "{by_path}");
        let bare = format!("fn f() {{ {name}(&env, &globals, &request)?; }}");
        assert_eq!(caught(&bare), [name], "{bare}");
    }
    // The handler of a task route that claims: caught.
    let handler = "async fn task(State(app): State<Shared>, uri: Uri) -> Result<Response, Refusal> {\n\
                   Ok(run(project, move |env, globals| {\n\
                   specengine_cli::task_claim(env, globals, &TaskClaimRequest { id, role, \
                   worktree, now, git: process_git(env) }).map(|o| Outcome::Task(Box::new(o)))\n\
                   }).await) }";
    assert_eq!(caught(handler), ["task_claim"], "{handler}");
    // Negative controls: the two reads, a request type, a name in a
    // string or a comment, a method of something else.
    for probe in [
        "fn f() { specengine_cli::task_list(env, globals, &TaskListRequest { statuses, git }) }",
        "fn f() { specengine_cli::task_show(env, globals, &TaskShowRequest { id, next: false, git }) }",
        "fn f() { let r: TaskClaimRequest = todo!(); let s = \"task_claim\"; } // task_approve(x)",
        "fn f() { queue.task_claim(); }",
    ] {
        assert_eq!(caught(probe), Vec::<String>::new(), "{probe}");
    }
}

// ------------------------------------------------------ ui-round, T-02

/// The repository (`git status`, `HEAD`, the refs, the worktrees, every
/// file but the index), the stored state of each of `ids` (the review's
/// own fields and its stage) and the events.
type RoundState = (
    Vec<String>,
    Vec<serde_json::Value>,
    Vec<(i64, String, String)>,
);

/// The [`RoundState`] of `root`.
fn round_state(
    scratch: &Scratch,
    home: &Path,
    cwd: &Path,
    root: &Path,
    ids: &[&str],
) -> RoundState {
    let git = scratch.git();
    let files: Vec<String> = snapshot(root)
        .into_iter()
        .filter(|(path, _)| path != ".git/index")
        .map(|(path, bytes)| format!("{path} {:?}", bytes))
        .collect();
    let repository = vec![
        git.run(root, &["status", "--porcelain", "--untracked-files=all"]),
        git.run(root, &["rev-parse", "HEAD"]),
        git.run(root, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        git.run(root, &["worktree", "list", "--porcelain"]),
        files.join("\n"),
    ];
    let proposals = ids
        .iter()
        .map(|id| {
            let mut review = spec_json(home, cwd, root, &["review", id]).json();
            review.as_object_mut().unwrap().retain(|key, _| {
                [
                    "id",
                    "status",
                    "decided_by",
                    "decided_at",
                    "decision_note",
                    "applied_commit",
                    "updated_at",
                    "record_id",
                    "staged",
                    "staged_at",
                ]
                .contains(&key.as_str())
            });
            review
        })
        .collect();
    let events = events(home, root)
        .into_iter()
        .map(|event| (event.seq, event.event_type, event.payload))
        .collect();
    (repository, proposals, events)
}

#[test]
fn ui_round_t02_the_round_routes_change_nothing_and_a_valid_post_only_stages() {
    let scratch = Scratch::new("door-round");
    let a = scratch.repo("spec-a", "a", "main");
    // An identity: a walk through the daemon could commit a record.
    let git = scratch.git();
    git.run(&a, &["config", "user.name", "Owner"]);
    git.run(&a, &["config", "user.email", "owner@example.invalid"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let q = ask(
        &home,
        &a,
        &["MEC-SPRINT", "EDGE-STAM-ZERO"],
        "Does a sprint end at zero?",
    );
    let input = json!({
        "node_ids": ["MEC-SPRINT"], "summary": "Walking drains stamina in the build.",
        "gap_type": "contradicts", "severity": "high",
        "evidence": [{"file": "src/stamina.rs", "qpath": "stamina::drain", "lines": "3-9",
            "observed": "drains while walking", "documented": "only while sprinting"}],
        "options": [{"label": "code", "effect": "fix the code", "price": "1 item"},
            {"label": "spec", "effect": "allow walking", "price": "a rebalance"}],
        "recommendation": 0
    })
    .to_string();
    let run = run_product(
        &spec_bin(),
        &home,
        &a,
        &[
            "--root",
            a.to_str().unwrap(),
            "propose",
            "discrepancy",
            "--input",
            "-",
        ],
        input.as_bytes(),
    );
    run.code(0);
    let d = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert_eq!([q.as_str(), d.as_str()], ["PR-0001", "PR-0002"]);
    let ids = [q.as_str(), d.as_str()];
    let before = round_state(&scratch, &home, &cwd, &a, &ids);
    assert_eq!(before.0[0], "", "the copy is committed");

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    let answers = format!("{p}/round/answers");
    // The sheet, cut or not; a bad query.
    for (path, status) in [
        (format!("{p}/round"), 200),
        (format!("{p}/round?limit=1"), 200),
        (format!("{p}/round?limit=0"), 400),
        (format!("{p}/round?depth=1"), 400),
    ] {
        let reply = server.get(&path);
        assert_eq!(reply.status, status, "{path}: {}", reply.text());
    }
    let sheet = server.get(&format!("{p}/round")).status(200).json();
    let shown = |id: &str| -> String {
        sheet["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == json!(id))
            .unwrap()["updated_at"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let approve = json!({"project": "lantern-keep", "answers": [{"id": q, "stage": {
        "decision": "approve", "option": null, "answer": null, "canon": null, "note": null,
        "updated_at": shown(&q)}}]})
    .to_string();
    // Refused before anything runs: from no page, another type, a body
    // that is no answers, another project, 201 answers.
    let host = format!("127.0.0.1:{}", server.port);
    let send = |headers: &[(&str, &str)], body: &str| {
        let length = body.len().to_string();
        let mut all = vec![("Host", host.as_str()), ("Content-Length", length.as_str())];
        all.extend_from_slice(headers);
        let mut bytes = common::request_bytes("POST", &answers, &all);
        bytes.extend_from_slice(body.as_bytes());
        server.raw(&bytes)
    };
    let same = ("Sec-Fetch-Site", "same-origin");
    let json_type = ("Content-Type", "application/json");
    let many: Vec<serde_json::Value> = (1..=201)
        .map(|n| {
            json!({"id": format!("PR-{n:04}"), "stage": {"decision": "reject",
            "reason": "r", "updated_at": "2026-10-07T09:00:00Z"}})
        })
        .collect();
    for (name, headers, body, status) in [
        ("no page", vec![json_type], approve.clone(), 403),
        (
            "text/plain",
            vec![same, ("Content-Type", "text/plain")],
            approve.clone(),
            415,
        ),
        (
            "no answers",
            vec![same, json_type],
            "{\"apply\":true}".to_owned(),
            400,
        ),
        (
            "another project",
            vec![same, json_type],
            approve.replace("lantern-keep", "zerkalo"),
            400,
        ),
        (
            "201 answers",
            vec![same, json_type],
            json!({"project": "lantern-keep", "answers": many}).to_string(),
            400,
        ),
    ] {
        let reply = send(&headers, &body);
        assert_eq!(reply.status, status, "{name}: {}", reply.text());
        reply.error_message();
    }
    // Run, every answer refused alone: 200, nothing stored.
    let refused = json!({"project": "lantern-keep", "answers": [
        {"id": "PR-9999", "stage": {"decision": "approve", "updated_at": shown(&q)}},
        {"id": q, "stage": {"decision": "approve", "updated_at": "2026-01-01T00:00:00Z"}},
        {"id": d, "stage": {"decision": "approve", "option": 7, "updated_at": shown(&d)}},
        {"id": d, "stage": {"decision": "reject", "reason": "dup", "updated_at": shown(&d)}}
    ]})
    .to_string();
    let reply = send(&[same, json_type], &refused);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let outcomes: Vec<String> = reply.json()["answers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["outcome"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(outcomes, ["refused"; 4], "{}", reply.text());
    // Another method on either route: 405.
    for (method, path) in [
        ("POST", format!("{p}/round")),
        ("PUT", format!("{p}/round")),
        ("GET", answers.clone()),
        ("DELETE", answers.clone()),
    ] {
        let reply = server.request(method, &path, &[same]);
        assert_eq!(reply.status, 405, "{method} {path}: {}", reply.text());
    }
    assert_eq!(
        round_state(&scratch, &home, &cwd, &a, &ids),
        before,
        "a sheet or a refused POST changes nothing"
    );

    // A valid POST: only the item's stage and its one event.
    let reply = send(&[same, json_type], &approve);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let entry = &reply.json()["answers"][0];
    assert_eq!(entry["outcome"], json!("staged"), "{}", reply.text());
    let at = entry["staged_at"].as_str().unwrap().to_owned();
    drop(server);
    let after = round_state(&scratch, &home, &cwd, &a, &ids);
    assert_eq!(after.0, before.0, "git and the files as they were");
    assert_eq!(after.1[1], before.1[1], "the other item untouched");
    let mut staged = before.1[0].clone();
    let object = staged.as_object_mut().unwrap();
    let stage = json!({"decision": "approve", "option": null, "answer": null, "canon": null,
        "note": null, "span_hash": null});
    object.insert("staged".to_owned(), stage.clone());
    object.insert("staged_at".to_owned(), json!(at));
    object.insert("updated_at".to_owned(), json!(at));
    assert_eq!(
        after.1[0], staged,
        "only its stage (and `updated_at`) changed"
    );
    assert_eq!(after.1[0]["status"], json!("open"));
    assert_eq!(
        after.2[..before.2.len()],
        before.2[..],
        "the earlier events kept"
    );
    let added: Vec<(&str, serde_json::Value)> = after.2[before.2.len()..]
        .iter()
        .map(|(_, kind, payload)| (kind.as_str(), serde_json::from_str(payload).unwrap()))
        .collect();
    assert_eq!(
        added,
        [(
            "proposal.staged",
            json!({"id": q, "staged": stage, "staged_at": at})
        )]
    );
}

#[test]
fn ui_round_t02_the_scan_catches_a_round_walk_and_its_terminal_but_not_the_staging_call() {
    // Positive controls: the walk called by path and bare; its terminal and
    // the round's outcome variants named anywhere.
    let by_path = "fn f() { let _ = specengine_cli::round_answer(&env, &globals, &request, &mut \
                   yes); }";
    assert_eq!(caught(by_path), ["round_answer"], "{by_path}");
    let bare = "fn f() { round_answer(env, globals, &request, &mut yes)?; }";
    assert_eq!(caught(bare), ["round_answer"], "{bare}");
    for (probe, want) in [
        (
            "struct Yes; impl specengine_cli::RoundTerminal for Yes { fn ask(&mut self, _: &str) \
             -> Option<bool> { Some(true) } }",
            vec!["RoundTerminal"],
        ),
        (
            "use specengine_cli::RoundTerminal; impl RoundTerminal for Yes {}",
            vec!["RoundTerminal", "RoundTerminal"],
        ),
        (
            "fn f(o: &Entry) -> u16 { match o.outcome { AnswerOutcome::Refused => 409, _ => 200 } }",
            vec!["AnswerOutcome"],
        ),
        (
            "fn f(s: &Step) -> bool { s.outcome == specengine_cli::WalkOutcome::Applied }",
            vec!["WalkOutcome"],
        ),
    ] {
        assert_eq!(caught(probe), want, "{probe}");
    }
    // The handler that stages and then walks with an always-yes terminal:
    // caught twice over.
    let handler = "async fn round_answers(State(app): State<Shared>, request: Request) -> \
                   Result<Response, Refusal> {\n\
                   Ok(run(project, move |env, globals| {\n\
                   let staged = specengine_cli::round_stage_answers(env, globals, &request)?;\n\
                   specengine_cli::round_answer(env, globals, &RoundAnswerRequest { file: None, \
                   now: utc_now(), git: process_git(env) }, &mut Yes)?;\n\
                   Ok(Outcome::RoundAnswers(Box::new(staged)))\n\
                   }).await) }\n\
                   struct Yes; impl RoundTerminal for Yes {}";
    assert_eq!(
        caught(handler),
        ["round_answer", "RoundTerminal"],
        "{handler}"
    );
    // Negative controls: the sheet, the staging call, the handler's own
    // name, the request types, a method of something else, a name in a
    // string or a comment.
    for probe in [
        "fn f() { specengine_cli::round_new(env, globals, &RoundNewRequest { limit, git }) }",
        "fn f() { specengine_cli::round_stage_answers(env, globals, &request).map(|o| \
         Outcome::RoundAnswers(Box::new(o))) }",
        "async fn round_answers(State(app): State<Shared>, request: Request) {}",
        "fn f() { let r: RoundAnswersRequest = todo!(); let b: RoundAnswersBody = todo!(); }",
        "fn f() { round_limit(limit)?; queue.round_answer(); }",
        "fn f() { let s = \"round_answer RoundTerminal\"; } // round_answer(x), RoundTerminal",
    ] {
        assert_eq!(caught(probe), Vec::<String>::new(), "{probe}");
    }
}
