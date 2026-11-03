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
//!
//! AC-R6 and AC-R7 of docs/features/health-left.md, the door's half: `GET
//! health` joins the endpoints that change nothing (no event, no index,
//! no file).
//!
//! AC-02 of docs/features/board-ui.md ("The fence stays"): the door changes
//! for cards alone. `CARD_CALLS`, beside `TASK_READS`, lists the ten
//! `card_` calls (eight, and docs/features/card-trace.md's two clears) and any other `card_` name is forbidden, each caught in a
//! probe by path and bare; `approve`, `reject`, every `propose*`,
//! `round_answer`, `import_state`, `export_*`, `init`, `index` and every
//! `task_` writer stay forbidden. The two card GETs join the endpoints that
//! change nothing. `card_writes_touch_only_the_queue`: each POST leaves
//! `git status`, `HEAD`, the refs, the files and the proposals alone and
//! adds one `card.*` event; a refused one adds none. M: a handler calling
//! `task_claim` or an unlisted `card_` name.
//!
//! B7 of docs/features/hub-confirm.md: the door changes for a hub's confirm
//! alone, by addition. `forbidden()` still holds `approve`, `approve_with` and
//! `reject` for every `src` file, hub code included; [`HUB_CONFIRM_CALLS`] are
//! caught in any file but `src/confirm.rs`, and `src/confirm.rs` holds no `|_|`
//! closure, by the file-aware [`caught_in`] and its positive controls. The
//! `git diff` half of B7, that this file loses no line, is a command. M:
//! `app.rs` calling `confirm_staged`; `confirm.rs` calling `approve_with` with
//! `|_| true`.
//!
//! B7 as amended (iteration 2): the always-yes scan also reads
//! `crates/specengine-cli/src/confirm.rs`, which holds the real consent
//! closures, and the exempt `src/confirm.rs` holds no `use ... as`, which would
//! rename a call past the scan. M: the CLI's `confirm.rs` answering `|_| true`;
//! `use specengine_cli::confirm_staged as go;` in `src/confirm.rs`.

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
        format!("{p}/health"),
        format!("{p}/inbox"),
        format!("{p}/proposals/PR-0001"),
        format!("{p}/proposals/PR-0002"),
        format!("{p}/tasks"),
        format!("{p}/tasks?status=draft&status=ready"),
        format!("{p}/tasks/T-0001"),
        format!("{p}/cards"),
        format!("{p}/cards?with_cancelled=true&flow=plain&no_target=true"),
        format!("{p}/cards/C-0001"),
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

    // The decision POST (the stage route, docs/canon/decision-staging.md
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
            "a write needs a same-origin page (not authentication: ADR-0034)"
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
        ("POST", format!("{p}/cards/C-0001")),
        ("PUT", format!("{p}/cards/C-0001")),
        ("PUT", format!("{p}/cards")),
        ("DELETE", format!("{p}/cards")),
        ("GET", format!("{p}/cards/C-0001/accept")),
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

/// The CLI library's card calls a handler may make, reads and writes: a
/// card is the one record the daemon writes beyond a stage (ADR-0056,
/// docs/features/board-ui.md "The fence stays").
const CARD_CALLS: [&str; 10] = [
    "card_list",
    "card_show",
    "card_new",
    "card_move",
    "card_edit",
    "card_assign",
    "card_cancel",
    "card_accept",
    "card_clear",
    "card_clear_group",
];

/// The names no handler may call: the CLI's deciding, storing, exporting,
/// importing, creating and indexing calls (`check` left the list with
/// docs/features/ui-live.md: the plain run is a read), every `task_` call
/// but the two reads (docs/features/ui-live-tasks.md AC-05) and every
/// `card_` name but [`CARD_CALLS`] (docs/features/board-ui.md AC-02).
fn forbidden(name: &str) -> bool {
    if name.starts_with("task_") {
        return !TASK_READS.contains(&name);
    }
    if name.starts_with("card_") {
        return !CARD_CALLS.contains(&name);
    }
    [
        "approve",
        "approve_with",
        "reject",
        "import_state",
        "init",
        "index",
        "round_answer",
    ]
    .contains(&name)
        || name.starts_with("export_")
        || name.starts_with("propose")
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
        "editorial": null, "note": null, "span_hash": null});
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

// ------------------------------------------------------- board-ui, AC-02

#[test]
fn board_ui_ac02_the_scan_admits_the_ten_card_calls_and_no_other_card_name() {
    // Positive controls: a `card_` name outside the list, by path and bare;
    // a name the library may add later too.
    for name in [
        "card_delete",
        "card_import",
        "card_restore",
        "card_approve",
        "card_history",
        "card_write",
    ] {
        let by_path = format!("fn f() {{ let _ = specengine_cli::{name}(&env, &globals, &r); }}");
        assert_eq!(caught(&by_path), [name], "{by_path}");
        let bare = format!("fn f() {{ {name}(&env, &globals, &r)?; }}");
        assert_eq!(caught(&bare), [name], "{bare}");
    }
    // What stays forbidden, by path and bare: a card POST answering with an
    // always-yes `Consent` could otherwise decide, stage a proposal or walk
    // a round; every `propose*`, the ones named nowhere yet too.
    for name in [
        "approve",
        "approve_with",
        "reject",
        "propose",
        "propose_brief",
        "propose_update",
        "propose_question",
        "propose_question_with_task",
        "propose_discrepancy",
        "propose_decision",
        "propose_interpretation",
        "propose_amendment",
        "propose_drift",
        "round_answer",
        "import_state",
        "export_state",
        "export_index",
        "init",
        "index",
        "task_claim",
        "task_approve",
        "task_new",
        "task_cancel",
    ] {
        let by_path = format!("fn f() {{ let _ = specengine_cli::{name}(&env, &globals, &r); }}");
        assert_eq!(caught(&by_path), [name], "{by_path}");
        let bare = format!("fn f() {{ {name}(&env, &globals, &r)?; }}");
        assert_eq!(caught(&bare), [name], "{bare}");
    }
    // M: the accept handler claiming a task, then calling an unlisted name.
    let handler = "async fn accept(State(app): State<Shared>, request: Request) {\n\
                   run(project, move |env, globals| {\n\
                   specengine_cli::task_claim(env, globals, &claim, &mut |_| true)?;\n\
                   specengine_cli::card_reopen(env, globals, &reopen)\n\
                   }).await }";
    assert_eq!(caught(handler), ["task_claim", "card_reopen"], "{handler}");
    // Negative controls: the ten, by path and bare; the handler of the
    // slice with its yes; the types, the outcome's variant, the body's cap;
    // a method of something else; a name in a string or a comment.
    for name in CARD_CALLS {
        let by_path = format!("fn f() {{ let _ = specengine_cli::{name}(&env, &globals, &r); }}");
        assert_eq!(caught(&by_path), Vec::<String>::new(), "{by_path}");
        let bare = format!("fn f() {{ {name}(&env, &globals, &r)?; }}");
        assert_eq!(caught(&bare), Vec::<String>::new(), "{bare}");
    }
    for probe in [
        "async fn cards_accept(State(app): State<Shared>, request: Request) {\n\
         run(project, move |env, globals| {\n\
         specengine_cli::card_accept(env, globals, &CardAcceptRequest { id, actor, note, \
         revision, now: utc_now() }, &mut |_| true).map(|o| Outcome::Card(Box::new(o)))\n\
         }).await }",
        "fn f() { let b: CardNewBody = todo!(); let r: CardMoveRequest = todo!(); }",
        "const CARD_BODY_MAX: usize = 1_048_576;",
        "fn f() { board.card_delete(); }",
        "fn f() { let s = \"card_delete approve propose_update\"; } // card_delete(x), approve(y)",
    ] {
        assert_eq!(caught(probe), Vec::<String>::new(), "{probe}");
    }
}

/// docs/features/board-ui.md "The fence stays", 3, as
/// docs/features/card-trace.md widens it (ADR-0057): a card's write is a row
/// and an event in the queue, nothing else, a departing accept's one
/// discrepancy apart. An open question stands for the
/// proposals, an identity makes a commit possible, and the whole repository
/// is compared byte for byte after every POST: each of the seven writes
/// adds exactly its one `card.*` event; each refused one, past the door or
/// at it, adds none. `spec task list` answers as it did: a card is no task.
#[test]
fn card_writes_touch_only_the_queue() {
    let scratch = Scratch::new("door-cards");
    let a = scratch.repo("card-press", "press", "main");
    let git = scratch.git();
    git.run(&a, &["config", "user.name", "Owner"]);
    git.run(&a, &["config", "user.email", "owner@example.invalid"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let q = ask(
        &home,
        &a,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let ids = [q.as_str()];
    let before = round_state(&scratch, &home, &cwd, &a, &ids);
    assert_eq!(before.0[0], "", "the copy is committed");
    let tasks = spec_json(&home, &cwd, &a, &["task", "list"])
        .document()
        .to_owned();

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/card-press";
    let post = |path: &str, body: &serde_json::Value| {
        server.stage(&format!("{p}/{path}"), &body.to_string())
    };
    let mut stored = before.2.clone();
    for (path, body, kind) in [
        (
            "cards",
            json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"],
                "assignee": "ana", "as": "desk"}),
            "card.created",
        ),
        (
            "cards/C-0001/assign",
            json!({"reviewer": "ben", "as": "desk"}),
            "card.assigned",
        ),
        (
            "cards/C-0001/edit",
            json!({"goal": "Fill the front.", "evidence": ["note=first pass"], "as": "ana"}),
            "card.edited",
        ),
        (
            "cards/C-0001/move",
            json!({"to": "edit", "as": "desk"}),
            "card.moved",
        ),
        (
            "cards/C-0001/move",
            json!({"to": "proof", "as": "ana"}),
            "card.moved",
        ),
        (
            "cards/C-0001/accept",
            json!({"briefed": true, "as": "ben", "note": "Read twice."}),
            "card.accepted",
        ),
        (
            "cards",
            json!({"title": "Back page", "discipline": "plates"}),
            "card.created",
        ),
        (
            "cards/C-0002/cancel",
            json!({"reason": "Pulled for space."}),
            "card.cancelled",
        ),
    ] {
        let reply = post(path, &body);
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        let after = round_state(&scratch, &home, &cwd, &a, &ids);
        assert_eq!(after.0, before.0, "{path}: git, the refs, every file");
        assert_eq!(after.1, before.1, "{path}: the proposal as it was");
        assert_eq!(
            after.2[..stored.len()],
            stored[..],
            "{path}: the earlier events kept"
        );
        let added: Vec<&str> = after.2[stored.len()..]
            .iter()
            .map(|(_, kind, _)| kind.as_str())
            .collect();
        assert_eq!(added, [kind], "{path}: one event");
        stored = after.2;
    }
    assert_eq!(
        spec_json(&home, &cwd, &a, &["task", "list"]).document(),
        tasks,
        "no task made or moved"
    );

    // Refused past the door: an open card to refuse on.
    let reply = post(
        "cards",
        &json!({"title": "Inner page", "discipline": "copy", "assignee": "ana"}),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    let stored = round_state(&scratch, &home, &cwd, &a, &ids);
    assert_eq!(stored.0, before.0, "git and the files as they were");
    for (status, path, body) in [
        (
            409,
            "cards/C-0003/move",
            json!({"to": "edit", "revision": "9", "as": "desk"}),
        ),
        (409, "cards/C-0003/move", json!({"to": "filed"})),
        (409, "cards/C-0003/move", json!({"to": "edit", "as": "ana"})),
        (
            409,
            "cards/C-0003/accept",
            json!({"briefed": true, "as": "ana"}),
        ),
        (409, "cards/C-0001/edit", json!({"goal": "Again."})),
        (404, "cards/C-0099/cancel", json!({"reason": "Late."})),
        (400, "cards/C-0003/move", json!({})),
        (400, "cards/C-0003/edit", json!({"owner": "ana"})),
        (400, "cards", json!({"goal": "No title."})),
    ] {
        let reply = post(path, &body);
        assert_eq!(reply.status, status, "{path} {body}: {}", reply.text());
        assert_eq!(
            round_state(&scratch, &home, &cwd, &a, &ids),
            stored,
            "{path} {body}: nothing changed"
        );
    }
    // docs/features/card-trace.md: the one proposal a card's write makes: a
    // departing accept stores its discrepancy, `proposal.created` then
    // `card.accepted`, and git, the files and the open question stay. Past it
    // no other write stores one.
    for (path, body) in [
        (
            "cards",
            json!({"title": "Long read", "discipline": "copy", "target": ["SHT-INNER"],
                "as": "desk"}),
        ),
        (
            "cards/C-0004/assign",
            json!({"reviewer": "ben", "as": "desk"}),
        ),
        ("cards/C-0004/move", json!({"to": "edit", "as": "desk"})),
        ("cards/C-0004/move", json!({"to": "proof", "as": "ana"})),
    ] {
        let reply = post(path, &body);
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
    }
    let seen = round_state(&scratch, &home, &cwd, &a, &ids);
    let reply = post(
        "cards/C-0004/accept",
        &json!({"as": "ben", "departs": ["SHT-INNER"], "summary": "It left the brief.",
            "observed": "Short.", "documented": "Long."}),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    let after = round_state(&scratch, &home, &cwd, &a, &ids);
    assert_eq!(after.0, before.0, "git, the refs, every file");
    assert_eq!(after.1, before.1, "the question as it was");
    let added: Vec<&str> = after.2[seen.2.len()..]
        .iter()
        .map(|(_, kind, _)| kind.as_str())
        .collect();
    assert_eq!(added, ["proposal.created", "card.accepted"]);
    let stored = after;

    // Refused at the door.
    let path = format!("{p}/cards/C-0003/edit");
    let body = b"{\"goal\":\"Again.\"}";
    let same = ("Sec-Fetch-Site", "same-origin");
    let json_type = ("Content-Type", "application/json");
    for (status, headers) in [
        (403, &[json_type][..]),
        (403, &[json_type, ("Sec-Fetch-Site", "none")][..]),
        (403, &[json_type, ("Sec-Fetch-Site", "cross-site")][..]),
        (
            403,
            &[same, json_type, ("Origin", "http://evil.example")][..],
        ),
        (415, &[same, ("Content-Type", "text/plain")][..]),
    ] {
        let reply = server.send("POST", &path, headers, body);
        assert_eq!(reply.status, status, "{headers:?}: {}", reply.text());
        assert_eq!(
            round_state(&scratch, &home, &cwd, &a, &ids),
            stored,
            "{headers:?}: nothing changed"
        );
    }
}

// ----------------------------------------------------- hub-confirm, B7

/// The CLI library's confirm calls (docs/features/hub-confirm.md
/// "Rules and edge cases"): the read of the question and the one call that may commit
/// under a login. Only `src/confirm.rs` names them: a handler anywhere else
/// could confirm past that file's judges.
const HUB_CONFIRM_CALLS: [&str; 2] = ["confirm_question", "confirm_staged"];

/// The one file allowed [`HUB_CONFIRM_CALLS`].
const HUB_CONFIRM_FILE: &str = "src/confirm.rs";

/// Each closure in `code` whose one argument is `_` or `_name`: a consent that
/// reads no question, the shape of an always-yes (`code` already blanked by
/// [`code_only`]). Spaces do not hide one.
fn ignoring_closures(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().filter(|c| !c.is_whitespace()).collect();
    let mut found = Vec::new();
    for (at, c) in chars.iter().enumerate() {
        if *c != '|' || chars.get(at + 1) != Some(&'_') {
            continue;
        }
        let mut end = at + 2;
        while chars
            .get(end)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
        {
            end += 1;
        }
        if matches!(chars.get(end), Some('|' | ':')) {
            found.push("|_|".to_owned());
        }
    }
    found
}

/// What the scan reports in `code`, the text of the `src` file `path`:
/// [`caught`] in every file, so the door's writers stay forbidden in
/// [`HUB_CONFIRM_FILE`] too; a call of [`HUB_CONFIRM_CALLS`] not after a `.` in
/// any other file; in that one file, a closure ignoring its argument.
fn caught_in(path: &Path, code: &str) -> Vec<String> {
    let mut found = caught(code);
    let blanked = code_only(code);
    if path.ends_with(HUB_CONFIRM_FILE) {
        found.extend(ignoring_closures(&blanked));
        found.extend(renaming_uses(&blanked));
    } else {
        found.extend(
            identifiers(&blanked)
                .into_iter()
                .filter(|(before, name)| {
                    *before != '.' && HUB_CONFIRM_CALLS.contains(&name.as_str())
                })
                .map(|(_, name)| name),
        );
    }
    found
}

/// B7, the file-aware scan's controls. `approve`, `approve_with` and `reject`
/// are caught by path and bare in every file, the confirm file first among
/// them; [`HUB_CONFIRM_CALLS`] everywhere but there; a `|_|` closure there,
/// however it is spelled; each `M:` of the criterion gives a finding.
/// Negative controls: the confirm file's own calls, a method of something
/// else, a closure reading its argument, a name in a string or a comment. M:
/// `app.rs` calling it; `confirm.rs` calling `approve_with` with `|_| true`.
#[test]
fn hub_confirm_b7_the_scan_knows_which_file_it_reads() {
    let confirm = Path::new("crates/specengine-http/src/confirm.rs");
    let others = [
        "app.rs",
        "hub.rs",
        "answer.rs",
        "main.rs",
        "extra/confirm.rs",
    ]
    .map(|name| PathBuf::from("crates/specengine-http/src").join(name));
    let probes = |name: &str| {
        [
            format!("fn f() {{ let _ = specengine_cli::{name}(&env, &globals, &r); }}"),
            format!("fn f() {{ {name}(&env, &globals, &r)?; }}"),
        ]
    };
    // The door's writers: every file, the confirm file first.
    for path in std::iter::once(confirm).chain(others.iter().map(PathBuf::as_path)) {
        for name in ["approve", "approve_with", "reject"] {
            for probe in probes(name) {
                assert_eq!(
                    caught_in(path, &probe),
                    [name],
                    "{}: {probe}",
                    path.display()
                );
            }
        }
    }
    // The confirm calls: any file but one.
    for name in HUB_CONFIRM_CALLS {
        for probe in probes(name) {
            for path in &others {
                assert_eq!(
                    caught_in(path, &probe),
                    [name],
                    "{}: {probe}",
                    path.display()
                );
            }
            assert_eq!(caught_in(confirm, &probe), Vec::<String>::new(), "{probe}");
        }
        let imported = format!("use specengine_cli::{{Env, {name}}};");
        assert_eq!(caught_in(&others[0], &imported), [name], "{imported}");
    }
    // A file named like it is not it.
    assert_eq!(
        caught_in(&others[4], "fn f() { confirm_staged(e, g, &r); }"),
        ["confirm_staged"]
    );
    assert!(!Path::new("src/confirm.rs.orig").ends_with(HUB_CONFIRM_FILE));

    // A closure ignoring its argument, in the confirm file.
    for closure in [
        "|_| true",
        "| _ | true",
        "|_: &str| true",
        "|_asked| true",
        "|_asked: &str| true",
        "move |_| true",
    ] {
        let probe = format!("fn f() {{ let mut yes = {closure}; run(&mut yes); }}");
        assert_eq!(caught_in(confirm, &probe), ["|_|"], "{probe}");
    }
    // The criterion's two mutations.
    let handler = "async fn accept(request: Request) {\n\
                   run(project, move |env, globals| {\n\
                   specengine_cli::confirm_staged(env, globals, &request)\n\
                   }).await }";
    assert_eq!(
        caught_in(&others[0], handler),
        ["confirm_staged"],
        "{handler}"
    );
    let always_yes = "async fn confirm(request: Request) {\n\
                      run(project, move |env, globals| {\n\
                      specengine_cli::approve_with(env, globals, &request, &flags, &mut |_| true)\n\
                      }).await }";
    assert_eq!(
        caught_in(confirm, always_yes),
        ["approve_with", "|_|"],
        "{always_yes}"
    );

    // Negative controls.
    for probe in [
        "fn f() { specengine_cli::confirm_staged(env, globals, &request) }",
        "fn f() { specengine_cli::confirm_question(env, globals, &request) }",
        "fn f() { let mut asked = |question: &str| question == read; run(&mut asked); }",
        "fn f() { let on = a || b; let mask = a | b; let g = || true; }",
        "fn f() { let s = \"|_| true\"; } // run(&mut |_| true), approve_with(x)",
    ] {
        assert_eq!(caught_in(confirm, probe), Vec::<String>::new(), "{probe}");
    }
    for probe in [
        "fn f() { queue.confirm_staged(); }",
        "fn f() { let r: ConfirmStagedRequest = todo!(); let s = \"confirm_staged\"; }",
        "fn f() {} // confirm_staged(x), confirm_question(y)",
        // The closure is the confirm file's duty: `.map_err(|_| ..)` is plain elsewhere.
        "fn f() { let n = text.parse::<u16>().map_err(|_| refused()); }",
    ] {
        assert_eq!(
            caught_in(&others[0], probe),
            Vec::<String>::new(),
            "{probe}"
        );
    }
}

/// B7, over the daemon as it stands: the scan reads `src/confirm.rs`, which
/// holds the one call of `confirm_staged`, so the exemption is not vacuous; no
/// file holds a finding of [`caught_in`]; and every file read, whatever its
/// name, still catches the door's writers. M: `app.rs` calling
/// `confirm_staged`; `confirm.rs` calling `approve_with` with `|_| true`.
#[test]
fn hub_confirm_b7_the_daemon_confirms_from_one_file_with_no_closure() {
    let files = sources();
    let confirm = files
        .iter()
        .find(|(path, _)| path.ends_with(HUB_CONFIRM_FILE))
        .unwrap_or_else(|| panic!("no {HUB_CONFIRM_FILE} among the daemon's sources"));
    let names: Vec<String> = identifiers(&code_only(&confirm.1))
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    assert!(
        names.iter().any(|name| name == "confirm_staged"),
        "{HUB_CONFIRM_FILE} names no `confirm_staged`: the exemption guards nothing"
    );
    let mut named = Vec::new();
    for (path, text) in &files {
        for name in caught_in(path, text) {
            named.push(format!("{}: {name}", path.display()));
        }
        for writer in ["approve", "approve_with", "reject"] {
            let probe = format!("fn f() {{ specengine_cli::{writer}(env, globals, &r); }}");
            assert_eq!(
                caught_in(path, &probe),
                [writer],
                "{}: the door's writers stay caught here",
                path.display()
            );
        }
    }
    assert!(named.is_empty(), "past the confirm door: {named:#?}");
}

// --------------------------------------------- hub-confirm, B7 as amended

/// The library's file holding the confirm's consent closures, the ones that
/// answer the question.
const CLI_CONFIRM_FILE: &str = "crates/specengine-cli/src/confirm.rs";

/// Each `use` item in `code` (blanked by [`code_only`]) renaming what it brings
/// in: a re-export under another name passes the scan of names, so the exempt
/// file holds none. `as _`, a trait brought into scope unnamed, renames nothing.
fn renaming_uses(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    for item in code.split(';') {
        let names: Vec<String> = identifiers(item)
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        let Some(start) = names.iter().position(|name| name == "use") else {
            continue;
        };
        let renames = names[start..]
            .windows(2)
            .any(|pair| pair[0] == "as" && pair[1] != "_");
        if renames {
            found.push("use as".to_owned());
        }
    }
    found
}

/// B7, the amended scan's controls: a closure ignoring its argument in the
/// library's confirm file, however spelled, and a `use ... as` in the exempt
/// file, each caught; a closure reading its question, a cast, a trait brought in
/// as `_`, a name in a string or a comment, not. M: the CLI's `confirm.rs`
/// answering `|_| true`; `use specengine_cli::confirm_staged as go;` in
/// `src/confirm.rs`.
#[test]
fn hub_confirm_b7_the_amended_scan_knows_the_library_file_and_a_rename() {
    for closure in ["|_| true", "| _ | true", "|_: &str| true", "|_asked| true"] {
        let probe = format!("fn f() {{ let mut same = {closure}; run(&mut same); }}");
        assert_eq!(ignoring_closures(&code_only(&probe)), ["|_|"], "{probe}");
    }
    let reading = "fn f() { let mut same = |asked: &str| { moved = asked != sent; !moved }; }";
    assert_eq!(ignoring_closures(&code_only(reading)), Vec::<String>::new());

    let confirm = Path::new("crates/specengine-http/src/confirm.rs");
    for probe in [
        "use specengine_cli::confirm_staged as go;",
        "use specengine_cli::{Env, confirm_question as ask};",
        "pub use specengine_cli::confirm_staged as go;",
        "pub(crate) use specengine_cli::{\n    ConfirmBody,\n    confirm_staged as go,\n};",
    ] {
        assert_eq!(caught_in(confirm, probe), ["use as"], "{probe}");
    }
    for probe in [
        "use specengine_cli::{ConfirmBody, ConfirmOutcome, Env, confirm_staged};",
        "use std::io::Write as _;",
        "fn f() -> u16 { let n = x as u16; n }",
        "fn f() { let s = \"use a as b\"; } // use specengine_cli::confirm_staged as go;",
    ] {
        assert_eq!(caught_in(confirm, probe), Vec::<String>::new(), "{probe}");
    }
}

/// B7 over the sources as they stand: the library's confirm file holds no
/// closure ignoring its argument, and the exempt daemon file no `use ... as`.
/// M: the CLI's `confirm.rs` answering `|_| true`; `use
/// specengine_cli::confirm_staged as go;` in `src/confirm.rs`.
#[test]
fn hub_confirm_b7_no_always_yes_in_the_library_and_no_rename_in_the_daemon() {
    let library = fs::read_to_string(repository_root().join(CLI_CONFIRM_FILE))
        .unwrap_or_else(|error| panic!("{CLI_CONFIRM_FILE}: {error}"));
    assert!(
        library.contains("confirm_staged"),
        "{CLI_CONFIRM_FILE} is the library's confirm"
    );
    assert_eq!(
        ignoring_closures(&code_only(&library)),
        Vec::<String>::new(),
        "{CLI_CONFIRM_FILE}: a consent that reads no question"
    );
    let files = sources();
    let (path, text) = files
        .iter()
        .find(|(path, _)| path.ends_with(HUB_CONFIRM_FILE))
        .expect("the daemon's confirm file");
    assert_eq!(
        renaming_uses(&code_only(text)),
        Vec::<String>::new(),
        "{}: a call renamed past the scan",
        path.display()
    );
}

/// The daemon's one file that may name the MCP crate (**The door** of
/// docs/features/hub-mcp.md "Rules and edge cases").
const HUB_MCP_FILE: &str = "mcp.rs";

/// Whether `code` names `specengine_mcp` as code: a path segment or a `use`,
/// never a comment or a literal.
fn names_the_mcp_crate(code: &str) -> bool {
    identifiers(&code_only(code))
        .iter()
        .any(|(_, name)| name == "specengine_mcp")
}

/// H9 of docs/features/hub-mcp.md: the daemon raises proposals for a hub's
/// `/mcp` alone, through the MCP crate's hub server, on ADR-0057's precedent:
/// `src/mcp.rs` is the one file of `crates/specengine-http/src` naming
/// `specengine_mcp` (`main.rs` names no item of it), and no file calls a
/// `propose*` name ([`ac03_the_daemon_names_no_deciding_or_writing_call`],
/// unchanged). The reader's controls first: a `use` and a path are seen, a
/// comment and a literal are not. M: `main.rs` or `app.rs` naming
/// `specengine_mcp`.
#[test]
fn hub_mcp_h9_the_mcp_crate_is_named_in_src_mcp_rs_alone() {
    assert!(names_the_mcp_crate("use specengine_mcp::HubSeam;"));
    assert!(names_the_mcp_crate(
        "fn f() { let s = specengine_mcp::SpecEngineServer::hub(seam); }"
    ));
    assert!(!names_the_mcp_crate(
        "// specengine_mcp::HubSeam\nfn f() { let s = \"specengine_mcp\"; }"
    ));
    let src = repository_root().join("crates/specengine-http/src");
    let naming: Vec<String> = sources()
        .into_iter()
        .filter(|(_, text)| names_the_mcp_crate(text))
        .map(|(path, _)| {
            path.strip_prefix(&src)
                .expect("under src")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        naming,
        [HUB_MCP_FILE],
        "the files of crates/specengine-http/src naming specengine_mcp"
    );
}
