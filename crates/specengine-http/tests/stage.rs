//! docs/features/decision-staging.md, the daemon's half ("Daemon": `POST`
//! and `DELETE` on `/api/projects/:p/proposals/:id/decision` through the
//! CLI library's `stage` and `unstage`; `docs/canon/decision-staging.md`
//! "Daemon"). Every request a real one over TCP to the real binary, the
//! queue read back with `sqlite3`, the events through the CLI library.
//!
//! - AC-03: staging an open `update`, `question`, `discrepancy` answers 200
//!   with the review document holding `staged`; `git status --porcelain`
//!   empty, `HEAD`, the branches, the worktrees and the files unchanged,
//!   each still `open`; one `proposal.staged` each, its payload the stage's
//!   bytes, seen by a subscriber within 1 s of the answer. M: the handler
//!   approves with an always-yes consent.
//! - AC-04: each refusal of `docs/canon/decision-record.md` "Flags" and a
//!   decision flag on an `update` or a `create`, via POST: the CLI's
//!   message, 400
//!   (the error body) or 409 (the document, its last note), nothing
//!   stored, no event; a path-only question's approve 400. M: the daemon's
//!   own option-range check.
//! - AC-05: `approved`, `applied`, `rejected` -> 409 (an applied one naming
//!   its commit); an orphan's approve 409, its reject stored; another
//!   existing repository's 409; an unknown ID 404 (the document); a
//!   look-alike 400; not `PR-` and digits 404 (the ID judged by the CLI,
//!   as the removed `a_proposal_id_is_pr_and_digits` judged it); a stale
//!   `updated_at` 409 with the current document. M: an applied row staged.
//! - AC-06: a second POST replaces the stage, its event the new one;
//!   DELETE clears both, one `proposal.unstaged`; DELETE with nothing
//!   staged 200, no event; DELETE on a rejected one 409. M: no unstage
//!   event.
//! - AC-15, the transport: a foreign `Origin`, `text/plain`, an oversize
//!   body (sent or declared), `Sec-Fetch-Site` absent, `none` or
//!   `cross-site`, a body that is no stage (bad JSON, a key unknown or
//!   missing, another decision): refused, nothing stored, no event, no
//!   `Access-Control-*`; `application/json` with a parameter, in any case,
//!   taken. M: the content-type check removed.
//! - AC-02, the daemon's half: a queue at schema 6 -> POST and DELETE 503.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::{
    Reply, Scratch, Server, Stream, ask, data_dir, encode_component, propose_update, run_product,
    snapshot, spec_bin, spec_json,
};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveRequest, Env, EventLine, GitEnv, Globals, RejectRequest, events_after,
};

const P: &str = "/api/projects/lantern-keep";
const DEPLETION: &str =
    "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.2 s.\n";

/// A repository of spec-a with an identity in its config (a decision
/// taken through the daemon could commit), a data `HOME`, and three open
/// proposals raised there: an update, a question and a discrepancy.
struct Setup {
    scratch: Scratch,
    a: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    update: String,
    question: String,
    report: String,
}

fn setup(label: &str) -> Setup {
    let scratch = Scratch::new(label);
    let a = scratch.repo("spec-a", "a", "main");
    let git = scratch.git();
    git.run(&a, &["config", "user.name", "Owner"]);
    git.run(&a, &["config", "user.email", "owner@example.invalid"]);
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let update = propose_update(&home, &cwd, &a, "EDGE-STAM-ZERO", DEPLETION);
    let question = ask(
        &home,
        &a,
        &["MEC-SPRINT", "EDGE-STAM-ZERO"],
        "Does a sprint end at zero?",
    );
    let report = report(&home, &a);
    assert_eq!(
        [update.as_str(), question.as_str(), report.as_str()],
        ["PR-0001", "PR-0002", "PR-0003"]
    );
    Setup {
        scratch,
        a,
        home,
        cwd,
        update,
        question,
        report,
    }
}

/// `spec propose discrepancy` on MEC-SPRINT, two options; its ID.
fn report(home: &Path, root: &Path) -> String {
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
    let root_text = root.to_str().expect("UTF-8 root");
    let run = run_product(
        &spec_bin(),
        home,
        root,
        &[
            "--root",
            root_text,
            "propose",
            "discrepancy",
            "--input",
            "-",
        ],
        input.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

fn decision(id: &str) -> String {
    format!("{P}/proposals/{id}/decision")
}

fn db(home: &Path) -> PathBuf {
    data_dir(home).join("lantern-keep.db")
}

/// `sql` on `db` through the system's `sqlite3` (a 2 s busy timeout).
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
    String::from_utf8(output.stdout)
        .expect("sqlite3 output")
        .trim()
        .to_owned()
}

/// `status|staged|staged_at|updated_at` of `id` as stored.
fn row(home: &Path, id: &str) -> String {
    sqlite3(
        &db(home),
        &format!(
            "SELECT status, coalesce(staged, '-'), coalesce(staged_at, '-'), updated_at \
             FROM proposals WHERE id = '{id}';"
        ),
    )
}

/// The project's events as stored, through the CLI library.
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

fn seqs(home: &Path, root: &Path) -> Vec<(i64, String, String)> {
    events(home, root)
        .into_iter()
        .map(|event| (event.seq, event.event_type, event.payload))
        .collect()
}

/// Everything AC-03 names about the repository.
fn repository(setup: &Setup, root: &Path) -> Vec<String> {
    let git = setup.scratch.git();
    let files: Vec<String> = snapshot(root)
        .into_iter()
        .filter(|(path, _)| path != ".git/index")
        .map(|(path, bytes)| format!("{path} {:?}", bytes.map(|bytes| bytes.len())))
        .collect();
    vec![
        git.run(root, &["status", "--porcelain", "--untracked-files=all"]),
        git.run(root, &["rev-parse", "HEAD"]),
        git.run(root, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        git.run(root, &["worktree", "list", "--porcelain"]),
        files.join("\n"),
    ]
}

/// The review document of `id` as the daemon serves it.
fn read(server: &Server, id: &str) -> Value {
    server
        .get(&format!("{P}/proposals/{id}"))
        .status(200)
        .json()
}

/// A stage of `fields` on `id` against its stored `updated_at` (read from
/// the queue: another repository's proposal is no read of this root's).
fn stage_stored(server: &Server, home: &Path, id: &str, fields: &str) -> Reply {
    let updated_at = sqlite3(
        &db(home),
        &format!("SELECT updated_at FROM proposals WHERE id = '{id}';"),
    );
    server.stage(&decision(id), &body(fields, &updated_at))
}

/// `{<fields>,"updated_at":"<at>"}`.
fn body(fields: &str, updated_at: &str) -> String {
    format!("{{{fields},\"updated_at\":\"{updated_at}\"}}")
}

/// A same-origin page's stage of `fields` on `id`, against the
/// `updated_at` it reads first.
fn stage_now(server: &Server, id: &str, fields: &str) -> Reply {
    let current = read(server, id);
    let updated_at = current["updated_at"].as_str().expect("updated_at");
    server.stage(&decision(id), &body(fields, updated_at))
}

fn is_utc(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[10] == b'T'
        && bytes[19] == b'Z'
        && text
            .chars()
            .enumerate()
            .all(|(at, c)| matches!(at, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit())
}

/// The library's git environment for `cwd` (the scratch's sandbox).
fn git_env(setup: &Setup, cwd: &Path) -> GitEnv {
    GitEnv::new(cwd, setup.scratch.git().vars())
}

fn library_env(setup: &Setup, cwd: &Path) -> (Env, Globals) {
    (
        Env {
            cwd: cwd.to_path_buf(),
            home: Some(setup.home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        Globals {
            root: Some(cwd.to_path_buf()),
            config: None,
        },
    )
}

// ------------------------------------------------------------------ AC-03

#[test]
fn ac03_staging_an_update_a_question_and_a_discrepancy_writes_only_the_queue() {
    let s = setup("stage-ac03");
    let before = repository(&s, &s.a);
    assert_eq!(before[0], "", "the copy is committed");
    let stored = seqs(&s.home, &s.a);
    let highest = stored.last().expect("the created events").0;
    let span = spec_json(&s.home, &s.cwd, &s.a, &["show", "EDGE-STAM-ZERO"]).json()["nodes"][0]
        ["span_hash"]
        .as_str()
        .expect("span_hash")
        .to_owned();
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let mut stream = Stream::open(
        server.port,
        &format!("{P}/events"),
        &[("Last-Event-ID", &highest.to_string())],
    );
    assert_eq!(stream.status, 200);
    let opening = stream
        .next_frame(Duration::from_secs(10))
        .expect("the opening frame");
    assert_eq!(opening.id.as_deref(), Some(highest.to_string().as_str()));

    let cases = [
        (
            &s.update,
            "\"decision\":\"approve\"".to_owned(),
            format!(
                "{{\"decision\":\"approve\",\"option\":null,\"answer\":null,\"canon\":null,\
                 \"note\":null,\"span_hash\":\"{span}\"}}"
            ),
        ),
        (
            &s.question,
            "\"decision\":\"approve\",\"answer\":\"Yes, at zero.\",\"canon\":\"EDGE-STAM-ZERO\",\
             \"note\":\"asked twice\""
                .to_owned(),
            "{\"decision\":\"approve\",\"option\":null,\"answer\":\"Yes, at zero.\",\
             \"canon\":\"EDGE-STAM-ZERO\",\"note\":\"asked twice\",\"span_hash\":null}"
                .to_owned(),
        ),
        (
            &s.report,
            "\"decision\":\"approve\",\"option\":1,\"answer\":null,\"canon\":null,\
             \"note\":\"keep the cap\""
                .to_owned(),
            "{\"decision\":\"approve\",\"option\":1,\"answer\":null,\"canon\":null,\
             \"note\":\"keep the cap\",\"span_hash\":null}"
                .to_owned(),
        ),
    ];
    for (id, fields, staged) in &cases {
        let reply = stage_now(&server, id, fields);
        let answered = Instant::now();
        assert_eq!(reply.status, 200, "{id}: {}", reply.text());
        assert_eq!(
            reply.header("content-type"),
            Some("application/json; charset=utf-8")
        );
        assert!(reply.cors_headers().is_empty(), "{reply:?}");
        let document = reply.json();
        assert_eq!(document.as_object().unwrap().len(), 45, "{document}");
        assert_eq!(document["id"], json!(id));
        assert_eq!(document["status"], json!("open"), "an attribute");
        assert!(document["decided_by"].is_null() && document["applied_commit"].is_null());
        let at = document["staged_at"]
            .as_str()
            .expect("staged_at")
            .to_owned();
        assert!(is_utc(&at), "{at}");
        assert_eq!(document["updated_at"], json!(at), "updated_at = now");
        assert!(
            reply.text().contains(&format!(
                "\"staged\":{staged},\"staged_at\":\"{at}\",\"notes\":"
            )),
            "the stage's bytes, keys in the canon's order: {}",
            reply.text()
        );
        assert_eq!(
            row(&s.home, id),
            format!("open|{staged}|{at}|{at}"),
            "{id} as stored"
        );
        // Its event, to a subscriber, within 1 s of the answer.
        let frame = stream
            .next_event(Duration::from_secs(5))
            .unwrap_or_else(|| panic!("{id}: no event within 5 s"));
        let late = answered.elapsed();
        assert!(
            late <= Duration::from_secs(1),
            "{id}: the event came {late:?} after"
        );
        assert_eq!(frame.event.as_deref(), Some("proposal.staged"), "{frame:?}");
        assert_eq!(
            frame.data.as_deref(),
            Some(
                format!("{{\"id\":\"{id}\",\"staged\":{staged},\"staged_at\":\"{at}\"}}").as_str()
            ),
            "{frame:?}"
        );
    }
    // One event each, nothing else.
    let frames = stream.frames_within(Duration::from_millis(400));
    assert!(
        frames
            .iter()
            .all(|frame| frame.event.is_none() && frame.data.is_none()),
        "{frames:?}"
    );
    drop(stream);
    drop(server);
    let after = seqs(&s.home, &s.a);
    assert_eq!(after[..stored.len()], stored[..], "the earlier events kept");
    let added: Vec<(&str, &str)> = after[stored.len()..]
        .iter()
        .map(|(_, kind, payload)| (kind.as_str(), payload.as_str()))
        .collect();
    assert_eq!(added.len(), 3, "{added:?}");
    assert!(added.iter().all(|(kind, _)| *kind == "proposal.staged"));
    assert_eq!(
        repository(&s, &s.a),
        before,
        "git and the files as they were"
    );
    for id in [&s.update, &s.question, &s.report] {
        let review = spec_json(&s.home, &s.cwd, &s.a, &["review", id]).json();
        assert_eq!(review["status"], json!("open"), "{id}");
        assert!(review["staged"].is_object(), "{id}: {review}");
    }
}

// ------------------------------------------------------------------ AC-04

#[test]
fn ac04_a_flag_the_terminal_refuses_is_refused_with_its_message_nothing_stored() {
    let s = setup("stage-ac04");
    // A path-only question (no ID among its targets) and a create.
    let path_only = ask(
        &s.home,
        &s.a,
        &["docs/features/stamina-tuning.md"],
        "Is the tuning final?",
    );
    let record = common::read_text(&common::fixture("spec-a"), "docs/records/R/R-12.md").replacen(
        "id: R-12\n",
        "id: R-13\n",
        1,
    );
    let a_text = s.a.to_str().unwrap();
    let run = run_product(
        &spec_bin(),
        &s.home,
        &s.a,
        &[
            "--root",
            a_text,
            "propose",
            "create",
            "docs/records/R/R-13.md",
            "--text-file",
            "-",
            "--rationale",
            "A new record.",
        ],
        record.as_bytes(),
    );
    run.code(0);
    let create = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert_eq!(
        [path_only.as_str(), create.as_str()],
        ["PR-0004", "PR-0005"]
    );
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let stored = seqs(&s.home, &s.a);
    let rows: Vec<String> = (1..=5)
        .map(|n| row(&s.home, &format!("PR-{n:04}")))
        .collect();
    let (update, question, report) = (s.update.as_str(), s.question.as_str(), s.report.as_str());
    let (path_only, create) = (path_only.as_str(), create.as_str());
    let long = |n: usize| "x".repeat(n);
    // (id, fields, status, the CLI's reason)
    let cases: Vec<(&str, String, u16, String)> =
        vec![
        (
            question,
            "\"decision\":\"approve\",\"option\":0".to_owned(),
            400,
            format!(
                "`--option` names a discrepancy's option, and `{question}` is a question: its \
                 working answer, or `--answer T`, answers it; nothing changed"
            ),
        ),
        (
            report,
            "\"decision\":\"approve\",\"option\":1,\"answer\":\"yes\"".to_owned(),
            400,
            format!(
                "`--answer` answers a question, and `{report}` is a discrepancy: name the owner's \
                 choice with `--option N` (0-1); nothing changed"
            ),
        ),
        (
            report,
            "\"decision\":\"approve\",\"note\":\"n\"".to_owned(),
            400,
            format!(
                "`{report}` is a discrepancy: name the owner's choice with `--option N` (0-1); \
                 nothing changed"
            ),
        ),
        (
            path_only,
            "\"decision\":\"approve\"".to_owned(),
            400,
            "`PR-0004` names no ID: name the section its record governs with `--canon REF`; \
             nothing changed"
                .to_owned(),
        ),
        (
            question,
            "\"decision\":\"approve\",\"canon\":\"not a ref\"".to_owned(),
            400,
            "`--canon not a ref` is no `ID`, `ID#SECTION` or `path#anchor`; nothing changed"
                .to_owned(),
        ),
        (
            question,
            "\"decision\":\"approve\",\"answer\":\"  \"".to_owned(),
            400,
            "`--answer` is blank: give the owner's answer, or leave it out to take the working \
             answer; nothing changed"
                .to_owned(),
        ),
        (
            question,
            "\"decision\":\"reject\",\"reason\":\" \"".to_owned(),
            400,
            "--reason is empty: say why the proposal is rejected".to_owned(),
        ),
        (
            report,
            "\"decision\":\"approve\",\"option\":5".to_owned(),
            409,
            format!("--option 5: `{report}` has options 0-1; nothing changed"),
        ),
        (
            question,
            format!("\"decision\":\"approve\",\"answer\":\"{}\"", long(2049)),
            409,
            "--answer: 2049 bytes; at most 2048; nothing changed".to_owned(),
        ),
        (
            question,
            format!(
                "\"decision\":\"approve\",\"canon\":\"EDGE-STAM-ZERO#{}\"",
                long(520)
            ),
            409,
            "--canon: 535 bytes; at most 512; nothing changed".to_owned(),
        ),
        (
            report,
            format!("\"decision\":\"approve\",\"option\":0,\"note\":\"{}\"", long(4097)),
            409,
            "--note: 4097 bytes; at most 4096; nothing changed".to_owned(),
        ),
        (
            question,
            format!("\"decision\":\"reject\",\"reason\":\"{}\"", long(4097)),
            409,
            "--reason: 4097 bytes; at most 4096; nothing changed".to_owned(),
        ),
        (
            report,
            "\"decision\":\"approve\",\"option\":0,\"note\":\"a\\u202eb\"".to_owned(),
            409,
            "--note: holds U+202E: a staged choice never carries it; nothing changed".to_owned(),
        ),
        (
            question,
            "\"decision\":\"reject\",\"reason\":\"bell\\u0007\"".to_owned(),
            409,
            "--reason: holds U+0007: a staged choice never carries it; nothing changed".to_owned(),
        ),
    ];
    let flagged = [
        (update, "option", "1"),
        (create, "option", "1"),
        (update, "answer", "\"yes\""),
        (create, "answer", "\"yes\""),
        (update, "canon", "\"EDGE-STAM-ZERO\""),
        (create, "canon", "\"EDGE-STAM-ZERO\""),
    ];
    for (id, fields, status, reason) in &cases {
        let reply = stage_now(&server, id, fields);
        let context = format!("{id} {}", &fields[..fields.len().min(80)]);
        assert_eq!(reply.status, *status, "{context}: {}", reply.text());
        assert!(reply.cors_headers().is_empty(), "{context}");
        if *status == 400 {
            assert_eq!(
                reply.error_message(),
                format!("spec: {reason}"),
                "{context}"
            );
        } else {
            let document = reply.json();
            assert_eq!(document["id"], json!(id), "{context}: the current document");
            assert_eq!(
                document["notes"].as_array().and_then(|notes| notes.last()),
                Some(&json!(reason)),
                "{context}"
            );
            assert!(document["staged"].is_null(), "{context}");
        }
    }
    // A decision flag on an update or a create: 400, the CLI's line naming
    // the flag.
    for (id, flag, value) in flagged {
        let reply = stage_now(
            &server,
            id,
            &format!("\"decision\":\"approve\",\"{flag}\":{value}"),
        );
        assert_eq!(reply.status, 400, "{id} --{flag}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.starts_with("spec: ") && message.contains(&format!("--{flag}")),
            "{id} --{flag}: {message}"
        );
    }
    drop(server);
    let after: Vec<String> = (1..=5)
        .map(|n| row(&s.home, &format!("PR-{n:04}")))
        .collect();
    assert_eq!(after, rows, "nothing stored");
    assert_eq!(seqs(&s.home, &s.a), stored, "no event");
}

// ------------------------------------------------------------------ AC-05

#[test]
fn ac05_a_proposal_that_is_not_open_here_is_refused_and_an_orphans_reject_stored() {
    let s = setup("stage-ac05");
    // Another repository of the project, existing; one that is gone.
    let other = s.scratch.repo("spec-a", "other", "main");
    let elsewhere = propose_update(&s.home, &s.cwd, &other, "EDGE-STAM-ZERO", DEPLETION);
    let gone_root = s.scratch.repo("spec-a", "gone", "main");
    let orphan = propose_update(&s.home, &s.cwd, &gone_root, "EDGE-STAM-ZERO", DEPLETION);
    std::fs::remove_dir_all(&gone_root).expect("the repository is gone");
    let held = propose_update(
        &s.home,
        &s.cwd,
        &s.a,
        "EDGE-STAM-ZERO",
        &DEPLETION.replace("0.2 s", "0.3 s"),
    );
    assert_eq!(
        [elsewhere.as_str(), orphan.as_str(), held.as_str()],
        ["PR-0004", "PR-0005", "PR-0006"]
    );
    // PR-0001 applied (the library's approve, consented), PR-0002 rejected.
    let (env, globals) = library_env(&s, &s.a);
    let applied = specengine_cli::approve(
        &env,
        &globals,
        &ApproveRequest {
            id: s.update.clone(),
            note: None,
            now: "2026-10-07T10:00:00Z".to_owned(),
            git: git_env(&s, &s.a),
        },
        &mut |_: &str| true,
    )
    .expect("approve");
    let commit = applied.document.applied_commit.clone().expect("applied");
    specengine_cli::reject(
        &env,
        &globals,
        &RejectRequest {
            id: s.question.clone(),
            reason: Some("Asked before.".to_owned()),
            now: "2026-10-07T10:00:00Z".to_owned(),
            git: git_env(&s, &s.a),
        },
        &mut |_: &str| true,
    )
    .expect("reject");
    // PR-0006 `approved` (as a run left it after its step 7).
    sqlite3(
        &db(&s.home),
        "UPDATE proposals SET status = 'approved', decided_by = 'Owner <owner@example.invalid>', \
         decided_at = '2026-10-07T10:00:00Z', updated_at = '2026-10-07T10:00:00Z' \
         WHERE id = 'PR-0006';",
    );
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let stored = seqs(&s.home, &s.a);
    let rows = |ids: &[&str]| -> Vec<String> { ids.iter().map(|id| row(&s.home, id)).collect() };
    let all = [
        "PR-0001", "PR-0002", "PR-0003", "PR-0004", "PR-0005", "PR-0006",
    ];
    let before = rows(&all);
    let approve = "\"decision\":\"approve\"";
    let reject = "\"decision\":\"reject\",\"reason\":\"not wanted\"";
    for (id, fields, reason) in [
        (
            "PR-0001",
            approve,
            format!(
                "`PR-0001` is applied: only an open proposal takes a staged choice (commit \
                 {commit}); nothing changed"
            ),
        ),
        (
            "PR-0002",
            reject,
            "`PR-0002` is rejected: only an open proposal takes a staged choice; nothing changed"
                .to_owned(),
        ),
        (
            "PR-0006",
            approve,
            "`PR-0006` is approved: only an open proposal takes a staged choice; nothing changed"
                .to_owned(),
        ),
    ] {
        let reply = stage_now(&server, id, fields);
        assert_eq!(reply.status, 409, "{id}: {}", reply.text());
        let document = reply.json();
        assert_eq!(document["id"], json!(id));
        assert_eq!(
            document["notes"].as_array().and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{id}: {document}"
        );
    }
    // Another existing repository's: refused, approve and reject alike.
    for fields in [approve, reject] {
        let reply = stage_stored(&server, &s.home, &elsewhere, fields);
        assert_eq!(reply.status, 409, "{fields}: {}", reply.text());
        let last = reply.json()["notes"].as_array().unwrap().last().cloned();
        let last = last
            .and_then(|note| note.as_str().map(str::to_owned))
            .unwrap_or_default();
        assert!(last.contains(other.to_str().unwrap()), "{last}");
    }
    // The orphan: its approve refused; its reject stored.
    let reply = stage_stored(&server, &s.home, &orphan, approve);
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(rows(&all), before, "nothing stored by the refusals");
    assert_eq!(seqs(&s.home, &s.a), stored, "no event by the refusals");
    let reply = stage_stored(&server, &s.home, &orphan, reject);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let at = reply.json()["staged_at"].as_str().unwrap().to_owned();
    assert_eq!(
        row(&s.home, &orphan),
        format!("open|{{\"decision\":\"reject\",\"reason\":\"not wanted\"}}|{at}|{at}")
    );
    // Unknown, not an ID, a look-alike.
    for (written, reason) in [
        (
            "PR-0099".to_owned(),
            "no proposal `PR-0099` in this project's queue".to_owned(),
        ),
        (
            "PR-1".to_owned(),
            "no proposal `PR-1`: a proposal ID is `PR-` and 4 or more digits, as `spec inbox` \
             lists it"
                .to_owned(),
        ),
        (
            "PR-".to_owned(),
            "no proposal `PR-`: a proposal ID is `PR-` and 4 or more digits, as `spec inbox` \
             lists it"
                .to_owned(),
        ),
        (
            "PR-4a".to_owned(),
            "no proposal `PR-4a`: a proposal ID is `PR-` and 4 or more digits, as `spec inbox` \
             lists it"
                .to_owned(),
        ),
        (
            "approve-all".to_owned(),
            "no proposal `approve-all`: a proposal ID is `PR-` and 4 or more digits, as `spec \
             inbox` lists it"
                .to_owned(),
        ),
    ] {
        let reply = server.stage(
            &decision(&encode_component(&written)),
            &body(approve, "2026-10-07T10:00:00Z"),
        );
        assert_eq!(reply.status, 404, "{written}: {}", reply.text());
        assert_eq!(
            reply.json()["notes"]
                .as_array()
                .and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{written}"
        );
    }
    // A lower-case or look-alike ID: the CLI's judgement, never staged.
    for written in ["pr-0004", "\u{0420}R-0004"] {
        let reply = server.stage(
            &decision(&encode_component(written)),
            &body(approve, "2026-10-07T10:00:00Z"),
        );
        assert!(
            reply.status == 400 || reply.status == 404,
            "{written}: {}",
            reply.text()
        );
        if reply.status == 400 {
            assert!(reply.error_message().starts_with("spec: "), "{written}");
        }
    }
    let reply = server.stage(
        &decision(&encode_component("\u{0420}R-0004")),
        &body(approve, "2026-10-07T10:00:00Z"),
    );
    assert_eq!(reply.status, 400, "a look-alike: {}", reply.text());
    assert!(
        reply.error_message().contains("PR-0004"),
        "the look-alike's Latin fix named: {}",
        reply.text()
    );
    // A stale `updated_at`: 409 with the current document, both times
    // named.
    let open = &s.report;
    let current = read(&server, open);
    let now_at = current["updated_at"].as_str().unwrap().to_owned();
    let stale = "2026-01-01T00:00:00Z";
    let reply = server.stage(
        &decision(open),
        &body("\"decision\":\"approve\",\"option\":1", stale),
    );
    assert_eq!(reply.status, 409, "{}", reply.text());
    let document = reply.json();
    assert_eq!(
        document["updated_at"],
        json!(now_at),
        "the current document"
    );
    assert_eq!(document["status"], json!("open"));
    assert!(document["staged"].is_null());
    assert_eq!(
        document["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!(format!(
            "`{open}` changed since it was read: updated at {now_at}, the choice was made on the \
             one of {stale}; read it again; nothing staged"
        ))),
        "{document}"
    );
    assert_eq!(row(&s.home, open).split('|').nth(1), Some("-"));
}

// ------------------------------------------------------------------ AC-06

#[test]
fn ac06_a_second_stage_replaces_and_delete_clears_with_one_event() {
    let s = setup("stage-ac06");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let id = &s.report;
    let stored = seqs(&s.home, &s.a).len();
    let first = stage_now(&server, id, "\"decision\":\"approve\",\"option\":0");
    assert_eq!(first.status, 200, "{}", first.text());
    let second = stage_now(
        &server,
        id,
        "\"decision\":\"reject\",\"reason\":\"a duplicate\"",
    );
    assert_eq!(second.status, 200, "{}", second.text());
    let second_at = second.json()["staged_at"].as_str().unwrap().to_owned();
    let reject = "{\"decision\":\"reject\",\"reason\":\"a duplicate\"}";
    assert_eq!(
        second.json()["staged"],
        json!({"decision": "reject", "reason": "a duplicate"})
    );
    assert_eq!(
        row(&s.home, id),
        format!("open|{reject}|{second_at}|{second_at}"),
        "replaced"
    );
    let added = seqs(&s.home, &s.a);
    assert_eq!(added.len(), stored + 2, "{added:?}");
    assert_eq!(added[stored + 1].1, "proposal.staged");
    assert_eq!(
        added[stored + 1].2,
        format!("{{\"id\":\"{id}\",\"staged\":{reject},\"staged_at\":\"{second_at}\"}}"),
        "the event the new stage"
    );

    // DELETE: both NULL, one `proposal.unstaged`, the document.
    let reply = server.unstage(&decision(id));
    assert_eq!(reply.status, 200, "{}", reply.text());
    let document = reply.json();
    assert!(
        document["staged"].is_null() && document["staged_at"].is_null(),
        "{document}"
    );
    assert_eq!(document["status"], json!("open"));
    let row_after = row(&s.home, id);
    assert!(row_after.starts_with("open|-|-|"), "{row_after}");
    let added = seqs(&s.home, &s.a);
    assert_eq!(added.len(), stored + 3, "{added:?}");
    assert_eq!(
        (added[stored + 2].1.as_str(), added[stored + 2].2.as_str()),
        ("proposal.unstaged", format!("{{\"id\":\"{id}\"}}").as_str())
    );
    // Nothing staged: 200, the document, no event, nothing written.
    let reply = server.unstage(&decision(id));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["id"], json!(id));
    assert_eq!(row(&s.home, id), row_after, "nothing written");
    assert_eq!(seqs(&s.home, &s.a).len(), stored + 3, "no event");
    // Not open: 409; unknown: 404.
    let (env, globals) = library_env(&s, &s.a);
    specengine_cli::reject(
        &env,
        &globals,
        &RejectRequest {
            id: s.question.clone(),
            reason: Some("Asked before.".to_owned()),
            now: "2026-10-07T10:00:00Z".to_owned(),
            git: git_env(&s, &s.a),
        },
        &mut |_: &str| true,
    )
    .expect("reject");
    let reply = server.unstage(&decision(&s.question));
    assert_eq!(reply.status, 409, "{}", reply.text());
    let reply = server.unstage(&decision("PR-0099"));
    assert_eq!(reply.status, 404, "{}", reply.text());
}

// ------------------------------------------------------------------ AC-15

/// A transport case: its name, the headers, the body, the status, the
/// error body's message (`None`: the CLI's decode line).
type TransportCase<'a> = (
    &'a str,
    Vec<(&'a str, &'a str)>,
    &'a str,
    u16,
    Option<&'a str>,
);

#[test]
fn ac15_the_transport_refuses_a_foreign_page_another_type_an_oversize_or_bad_body() {
    let s = setup("stage-ac15");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let id = &s.report;
    let path = decision(id);
    let updated_at = read(&server, id)["updated_at"].as_str().unwrap().to_owned();
    let good = body("\"decision\":\"approve\",\"option\":1", &updated_at);
    let own = format!("http://127.0.0.1:{}", server.port);
    let stored = seqs(&s.home, &s.a);
    let before = row(&s.home, id);
    let big = format!(
        "{{\"decision\":\"approve\",\"option\":1,\"note\":\"{}\",\"updated_at\":\"{updated_at}\"}}",
        "x".repeat(17 * 1024)
    );
    let json_type = ("Content-Type", "application/json");
    let same = ("Sec-Fetch-Site", "same-origin");
    let cases: Vec<TransportCase<'_>> = vec![
        (
            "a foreign Origin",
            vec![("Origin", "http://evil.example"), same, json_type],
            &good,
            403,
            Some(
                "refused: the Origin header, when sent, must be http://127.0.0.1:PORT or \
                 http://localhost:PORT",
            ),
        ),
        (
            "text/plain",
            vec![
                ("Origin", own.as_str()),
                same,
                ("Content-Type", "text/plain"),
            ],
            &good,
            415,
            Some("the body is JSON: send it with Content-Type: application/json"),
        ),
        (
            "no Content-Type",
            vec![("Origin", own.as_str()), same],
            &good,
            415,
            Some("the body is JSON: send it with Content-Type: application/json"),
        ),
        (
            "an oversize body",
            vec![("Origin", own.as_str()), same, json_type],
            &big,
            413,
            Some("the body is over 16384 bytes"),
        ),
        (
            "no Sec-Fetch-Site",
            vec![("Origin", own.as_str()), json_type],
            &good,
            403,
            Some("staging needs a same-origin page (not authentication: ADR-0034)"),
        ),
        (
            "Sec-Fetch-Site: none",
            vec![("Sec-Fetch-Site", "none"), json_type],
            &good,
            403,
            Some("staging needs a same-origin page (not authentication: ADR-0034)"),
        ),
        (
            "Sec-Fetch-Site: cross-site",
            vec![("Sec-Fetch-Site", "cross-site"), json_type],
            &good,
            403,
            Some("refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`"),
        ),
        (
            "bad JSON",
            vec![same, json_type],
            "{\"decision\":",
            400,
            None,
        ),
        (
            "an unknown key",
            vec![same, json_type],
            "{\"decision\":\"approve\",\"option\":1,\"consent\":true,\"updated_at\":\"x\"}",
            400,
            None,
        ),
        (
            "updated_at missing",
            vec![same, json_type],
            "{\"decision\":\"approve\",\"option\":1}",
            400,
            None,
        ),
        (
            "a reject's reason missing",
            vec![same, json_type],
            "{\"decision\":\"reject\",\"updated_at\":\"x\"}",
            400,
            None,
        ),
        (
            "another decision",
            vec![same, json_type],
            "{\"decision\":\"defer\",\"updated_at\":\"x\"}",
            400,
            None,
        ),
        (
            "span_hash sent",
            vec![same, json_type],
            "{\"decision\":\"approve\",\"option\":1,\"span_hash\":\"b3:00\",\"updated_at\":\"x\"}",
            400,
            None,
        ),
    ];
    for (name, headers, sent, status, message) in &cases {
        let reply = server.send("POST", &path, headers, sent.as_bytes());
        assert_eq!(reply.status, *status, "{name}: {}", reply.text());
        assert!(reply.cors_headers().is_empty(), "{name}: {reply:?}");
        assert_eq!(reply.header("allow"), None, "{name}");
        let got = reply.error_message();
        match message {
            Some(want) => assert_eq!(
                got,
                want.replace("PORT", &server.port.to_string()),
                "{name}"
            ),
            None => assert!(
                got.starts_with("spec: the stage is not `{\"decision\":\"approve\"")
                    && got.ends_with("; nothing changed"),
                "{name}: {got}"
            ),
        }
        assert_eq!(row(&s.home, id), before, "{name}: nothing stored");
    }
    // Declared over the cap: 413 before a byte of it is read.
    let host = format!("127.0.0.1:{}", server.port);
    let mut bytes = common::request_bytes(
        "POST",
        &path,
        &[
            ("Host", &host),
            same,
            json_type,
            ("Content-Length", "16385"),
        ],
    );
    bytes.extend_from_slice(b"{}");
    let reply = server.raw(&bytes);
    assert_eq!(reply.status, 413, "{}", reply.text());
    // OPTIONS, a preflight: 405, no `Access-Control-*`.
    let reply = server.request(
        "OPTIONS",
        &path,
        &[
            ("Origin", own.as_str()),
            ("Access-Control-Request-Method", "POST"),
        ],
    );
    assert_eq!(reply.status, 405, "{}", reply.text());
    assert!(reply.cors_headers().is_empty(), "{reply:?}");
    assert_eq!(seqs(&s.home, &s.a), stored, "no event");
    assert_eq!(row(&s.home, id), before, "nothing stored");

    // `application/json` with a parameter, in another case: taken.
    let reply = server.send(
        "POST",
        &path,
        &[same, ("Content-Type", "Application/JSON; charset=utf-8")],
        good.as_bytes(),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(reply.cors_headers().is_empty(), "{reply:?}");
    assert_eq!(reply.json()["staged"]["option"], json!(1));
}

// ------------------------------------------------------------------ AC-02

#[test]
fn ac02_a_queue_of_a_newer_schema_answers_503_to_post_and_delete() {
    let s = setup("stage-ac02");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let updated_at = read(&server, &s.report)["updated_at"]
        .as_str()
        .unwrap()
        .to_owned();
    sqlite3(&db(&s.home), "PRAGMA user_version = 6;");
    let reply = server.stage(
        &decision(&s.report),
        &body("\"decision\":\"approve\",\"option\":1", &updated_at),
    );
    assert_eq!(reply.status, 503, "{}", reply.text());
    let message = reply.error_message();
    assert!(message.contains('6') && message.contains('5'), "{message}");
    let reply = server.unstage(&decision(&s.report));
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(
        sqlite3(&db(&s.home), "PRAGMA user_version;"),
        "6",
        "left alone"
    );
    assert_eq!(
        sqlite3(
            &db(&s.home),
            "SELECT count(*) FROM proposals WHERE staged IS NOT NULL;"
        ),
        "0"
    );
}

// ------------------------------------------- the page before the path

/// `Sec-Fetch-Site` is judged before the proposal's path is decoded: POST
/// and DELETE on `/proposals/%FF/decision` (no UTF-8) without the header →
/// 403 with ADR-0034's message, with it → 400; an unknown slug without the
/// header stays 404 (the served slug first). Nothing stored. M: the path
/// decoded first.
#[test]
fn the_page_is_judged_before_the_path_is_decoded() {
    let s = setup("stage-site-first");
    let server = Server::serve(&s.home, &s.cwd, &[&s.a]);
    let stored = seqs(&s.home, &s.a);
    let path = format!("{P}/proposals/%FF/decision");
    let stage_body = body("\"decision\":\"approve\"", "2026-10-07T10:00:00Z");
    let json_type = ("Content-Type", "application/json");
    for (method, sent) in [("POST", stage_body.as_bytes()), ("DELETE", &b""[..])] {
        let reply = server.send(method, &path, &[json_type], sent);
        assert_eq!(reply.status, 403, "{method} without: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            "staging needs a same-origin page (not authentication: ADR-0034)",
            "{method}"
        );
        let reply = server.send(
            method,
            &path,
            &[("Sec-Fetch-Site", "same-origin"), json_type],
            sent,
        );
        assert_eq!(reply.status, 400, "{method} with: {}", reply.text());
        reply.error_message();
        // An unknown slug: 404 whatever the page.
        let reply = server.send(
            method,
            "/api/projects/no-such-slug/proposals/PR-0001/decision",
            &[json_type],
            sent,
        );
        assert_eq!(reply.status, 404, "{method} unknown slug: {}", reply.text());
        assert!(reply.cors_headers().is_empty(), "{reply:?}");
    }
    assert_eq!(seqs(&s.home, &s.a), stored, "nothing stored");
}
