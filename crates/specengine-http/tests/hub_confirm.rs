//! docs/features/hub-confirm.md (RM-0050, step 9b), the daemon's half: a login
//! `[team]` entitles confirms one staged choice, as one commit in the hub's own
//! checkout, nothing pushed. Through the real daemon over git repositories of
//! `fixtures/card-press` (a print shop), `fixtures/spec-a` (a game's brief) and
//! `fixtures/code-a` (a library with its `spec.lock`), so the mode is tried on
//! projects of different nature (ADR-0008). Every expected value is worked out
//! from the spec and the canon, or asked of the library's own `approve` and
//! `reject` as a terminal asks them, never read back from a hub's run.
//!
//! - B1: `mira` and `ann` confirm two proposals, the checkout's `user.name` being
//!   `Owner`: two commits, `Decided-by`, queue, author and committer equal; a
//!   record and a new file alike. M: a default committer.
//! - B2: one commit, one path, `Proposal:`, status clean, a bare `origin`
//!   unchanged; a rejected one adds none and its cards still move. M: a push; a
//!   reject touching a card.
//! - B3: a lead outside its zone, an unzoned path, a discrepancy over two zones:
//!   403 for the lead, 200 for `owner`; a `member`, a login outside `[team]`:
//!   403, nothing stored. M: judging `target_path` alone.
//! - B4: a `question` other than the library's, or a target moved after the read:
//!   409, nothing changed. M: an always-yes closure.
//! - B5: a staged reject rejects and never applies; nothing staged, or a stage
//!   replaced after the read: 409. M: always calling approve.
//! - B6: `editorial: true`, drift, task-bound, an `approved` row: 409 naming the
//!   terminal, `spec.lock` byte for byte. M: dropping the refusal.
//! - B8: on a local daemon both verbs are the unknown-route 404, `Remote-User`
//!   too. M: mounting it unconditionally.
//! - B9: no MCP path: `tools/list`, the MCP crate's manifest, `Remote-User:
//!   agent` a 403 on all four. M: a confirm tool.
//! - B10, the daemon's half: three round answers stage three and apply none, each
//!   takes its own POST; the read adds no event on a failed place. M:
//!   `round_answers` confirming; rehearsing through `failed()`.
//! - B11, the trailers: `login=tomas` in `Proposed-by` on a hub, a local trailer
//!   byte for byte. M: `login` written on a local row.
//! - docs/features/apply-outputs.md (RM-0072) O2 and O8's hub half, over
//!   `fixtures/apply-outputs/forge`: a confirm's commit carries the index's
//!   outputs as a terminal's does, and a gap commits the target alone with the
//!   warning in the POST's `notes`. M: render only on the terminal path; the
//!   partial walk rendered.
//! - docs/features/hub-git.md (RM-0051) "The confirm dialog after a 200", the
//!   daemon's half: a clean confirm's 200 document holds `notes: []`, with and
//!   without the index's outputs in its commit. M: the outputs' note in `notes`.
//!
//! Elsewhere: B7 is `door.rs`, with its `git diff` a command; B10's dialog is
//! `ui/src/inbox/ConfirmDialog.test.tsx`; B11's step-up regex is the unit
//! `the_step_up_regex_is_the_confirm_class` in `src/hub.rs`; the read's key set
//! `daemon_keys.rs` (`ConfirmRead`); the route's GET pin `tests/hub.rs`.
//!
//! Beyond the criteria, from "Data" and `docs/canon/hub-team.md`: the route is
//! the confirm class's fourth (no query, a required `Origin`, exact raw segments,
//! `[team]` judged again past the body), its body is closed and capped at 4 MiB,
//! a read repeats byte for byte, a second click and racing clicks make one
//! commit, a question naming its decider is no other login's, the refusals come
//! in their order, and a POST, a real attempt, logs what a read never does.
//! `HubRun.judged`, the row a run reads being the row judged, cannot be raced from
//! here: it is a unit in `specengine-cli`'s `src/confirm.rs`. A five-key author
//! against `get_proposal`'s schema is `specengine-mcp`'s `tests/mcp_trace.rs`.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::thread;

use common::hub::{Hub, ORIGIN, P, Proxied, Setup, assert_refused, through, through_in_two_steps};
use common::{
    HTTP_TIMEOUT, Mcp, Reply, Scratch, Server, copy_file, fixture, http_exchange, product_env,
    propose_update, read_text, replace, repository_root, run_product, snapshot, spec, spec_bin,
    spec_json, write,
};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, DiscrepancyInput, DiscrepancyRequest, Env, Evidence, Exit,
    GapType, GitEnv, Globals, IntakeOption, IntakeSeverity, QuestionRequest, RejectRequest,
    StageBody, StageRequest, approve, approve_with, propose_discrepancy, propose_question, reject,
    stage, utc_now,
};

/// The game's project (`fixtures/spec-a`).
const KEEP: &str = "/api/projects/lantern-keep";
/// The library's project (`fixtures/code-a`).
const TALLY: &str = "/api/projects/code-a";

/// The lines a confirm answers with: its own two moved texts, the question's
/// and `updated_at`'s, tell which compare caught a change.
const MOVED_LINE: &str = "moved since its question was read";
const CHANGED_LINE: &str = "changed since it was read";

/// The most bytes of a confirm's body.
const BODY_MAX: usize = 4_194_304;

/// The lines of docs/canon/hub-team.md "Answers".
const OFF_LIST_LINE: &str = "refused: this hub does not serve this route";
const NO_QUERY_LINE: &str = "refused: this route takes no query";
const NO_ORIGIN_LINE: &str = "refused: this route needs the Origin header";
const WRONG_ORIGIN_LINE: &str =
    "refused: the Origin header, when sent, must be https://hub.example.test";
const NO_TEAM_LINE: &str = "refused: this project has no `[team]`";

/// Each login's commit identity (docs/features/hub-confirm.md "Data"): the
/// name part is the login, never `name`.
const ANN: &str = "ann <ann@example.org>";
const MIRA: &str = "mira <mira@example.org>";
const LENA: &str = "lena <lena@example.org>";

/// What the checkout's own config says: a commit under it is a hub ignoring the
/// login.
const OWNER_NAME: &str = "Owner";
const OWNER: &str = "Owner <owner@example.invalid>";

const FRONT_MD: &str = "docs/spec/sht-front.md";
const BACK_MD: &str = "docs/spec/sht-back.md";
const INNER_MD: &str = "docs/spec/sht-inner.md";
const SPRINT_MD: &str = "docs/spec/movement/sprint.md";
const STAMINA_MD: &str = "docs/spec/movement/stamina.md";
const LOCK: &str = "spec.lock";

/// The print shop's team. `ann` the owner; `mira` leads the front, `lena` the
/// front and the back; `tomas` a member. `sht-inner.md` lies in no zone; `eve`, in
/// no table, is the login outside it. `mira`'s `name` is shown only.
const PRESS_TEAM: &str = r#"
[zones]
front = ["docs/spec/sht-front.md"]
back = ["docs/spec/sht-back.md"]

[team.ann]
role = "owner"
email = "ann@example.org"

[team.mira]
name = "Mira K"
role = "lead"
zones = ["front"]
email = "mira@example.org"

[team.lena]
role = "lead"
zones = ["front", "back"]
email = "lena@example.org"

[team.tomas]
role = "member"
email = "tomas@example.org"
"#;

/// `lena`'s entry of [`PRESS_TEAM`], as `[team]` holds it.
const LENA_ENTRY: &str =
    "[team.lena]\nrole = \"lead\"\nzones = [\"front\", \"back\"]\nemail = \"lena@example.org\"\n";

/// The game's team: `ann` the owner; `mira` leads the sprint, `kit` the stamina,
/// `lena` both, `ren` the feature documents alone and `wil` all three.
const KEEP_TEAM: &str = r#"
[zones]
sprint = ["docs/spec/movement/sprint.md"]
stamina = ["docs/spec/movement/stamina.md"]
features = ["docs/features/**"]

[team.ren]
role = "lead"
zones = ["features"]
email = "ren@example.org"

[team.wil]
role = "lead"
zones = ["features", "sprint", "stamina"]
email = "wil@example.org"

[team.ann]
role = "owner"
email = "ann@example.org"

[team.mira]
role = "lead"
zones = ["sprint"]
email = "mira@example.org"

[team.kit]
role = "lead"
zones = ["stamina"]
email = "kit@example.org"

[team.lena]
role = "lead"
zones = ["sprint", "stamina"]
email = "lena@example.org"
"#;

/// The library's team: `ann` the owner, `mira` leading a file none of its tests
/// here touches.
const TALLY_TEAM: &str = r#"
[zones]
limits = ["docs/spec/limits.md"]

[team.ann]
role = "owner"
email = "ann@example.org"

[team.mira]
role = "lead"
zones = ["limits"]
email = "mira@example.org"
"#;

/// Where the print shop's records go, for what decides there.
const PRESS_RECORDS: &str = r#"
[decision_records]
prefix = "STY"
dir = "docs/records/STY"
template = "templates/ruling.md"
"#;

// ------------------------------------------------------------------ set up

/// `table` at the end of `root`'s config.
fn append_config(root: &Path, table: &str) {
    let mut text = read_text(root, "specengine.toml");
    text.push_str(table);
    write(root, "specengine.toml", text);
}

/// Everything under `setup`'s root committed as `message`.
fn committed(setup: &Setup, message: &str) {
    let git = setup.scratch.git();
    git.run(&setup.root, &["add", "-A"]);
    git.run(
        &setup.root,
        &["commit", "-q", "--allow-empty", "-m", message],
    );
}

/// The print shop, opted in by a commit.
fn press(name: &str) -> Setup {
    let setup = Setup::new(name, "card-press");
    append_config(&setup.root, PRESS_TEAM);
    committed(&setup, "the team");
    setup
}

/// [`press`], with `[decision_records]` and a record template, so a discrepancy
/// can be approved there; the shape is the project's own table and `spec-a`'s
/// template.
fn press_with_records(name: &str) -> Setup {
    let setup = Setup::new(name, "card-press");
    append_config(&setup.root, PRESS_TEAM);
    append_config(&setup.root, PRESS_RECORDS);
    fs::create_dir_all(setup.root.join("templates")).expect("templates");
    copy_file(
        fixture("spec-a").join("templates/decision.md"),
        setup.root.join("templates/ruling.md"),
    );
    committed(&setup, "the team and its records");
    setup
}

/// The game, opted in by a commit.
fn keep(name: &str) -> Setup {
    let setup = Setup::new(name, "spec-a");
    append_config(&setup.root, KEEP_TEAM);
    committed(&setup, "the team");
    setup
}

/// The library with its golden `spec.lock`, opted in by a commit.
fn tally(name: &str) -> Setup {
    let setup = Setup::new(name, "code-a");
    copy_file(
        fixture("spec-lock").join("code-a.lock"),
        setup.root.join(LOCK),
    );
    append_config(&setup.root, TALLY_TEAM);
    committed(&setup, "the lock and the team");
    setup
}

/// `path` under `root` as bytes.
fn bytes(root: &Path, path: &str) -> Vec<u8> {
    fs::read(root.join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// An `update` of `target`, its text with `from` become `to`: the ID.
fn reworded(setup: &Setup, target: &str, from: &str, to: &str) -> String {
    let text = shown_text(setup, target);
    assert!(text.contains(from), "{target} holds no {from:?}: {text}");
    propose_update(
        &setup.home,
        &setup.cwd,
        &setup.root,
        target,
        &text.replacen(from, to, 1),
    )
}

/// `target`'s text as `spec show` reads it, holding a word the test will change.
fn shown_text(setup: &Setup, target: &str) -> String {
    let shown = spec_json(&setup.home, &setup.cwd, &setup.root, &["show", target]).json();
    shown["nodes"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{target}: {shown}"))
        .to_owned()
}

/// A feature document at `path` amending `amended`, raised by `spec propose
/// amendment`: its ID.
fn amendment(setup: &Setup, path: &str, amended: &str) -> String {
    let text = format!(
        "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: [{amended}]\n---\n\n\
         # An amendment\n\n## Delay {{#AC-01}}\n\nRegeneration starts 1.5 s after the last sprint.\n"
    );
    let root = setup.root.to_str().expect("UTF-8 root");
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.cwd,
        &[
            "--root",
            root,
            "propose",
            "amendment",
            path,
            "--text-file",
            "-",
            "--rationale",
            "Regeneration waits after a sprint.",
            "--author-role",
            "spec-author",
        ],
        text.as_bytes(),
    );
    run.code(0);
    run.stdout.lines().next().unwrap_or_default().to_owned()
}

/// The section `{#id}` of `path` loses its ID by a commit: what names it now
/// names nothing.
fn id_removed(desk: &Desk, path: &str, heading: &str, id: &str) {
    replace(
        desk.root(),
        path,
        &format!("{heading} {{#{id} .playtest}}\n"),
        &format!("{heading}\n"),
    );
    desk.git(&["commit", "-qam", "the section loses its ID"]);
}

/// A lead's line when what it would govern resolves to nothing.
#[track_caller]
fn assert_owner_only(reply: &Reply, login: &str, zones: &str, context: &str) {
    assert_eq!(reply.status, 403, "{context}: {}", reply.text());
    let line = reply.error_message();
    assert!(
        line.starts_with(&format!("refused: {login} leads {zones}; "))
            && line.ends_with(": this act needs an `owner`"),
        "{context}: {line}"
    );
}

/// A new file at `path` holding the record `id`, raised by `spec propose create`.
fn new_record(setup: &Setup, id: &str, path: &str) -> String {
    let text = read_text(&fixture("spec-a"), "docs/records/R/R-12.md").replacen(
        "id: R-12\n",
        &format!("id: {id}\n"),
        1,
    );
    let root = setup.root.to_str().expect("UTF-8 root");
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.cwd,
        &[
            "--root",
            root,
            "propose",
            "create",
            path,
            "--text-file",
            "-",
            "--rationale",
            "A new record.",
        ],
        text.as_bytes(),
    );
    run.code(0);
    run.stdout.lines().next().unwrap_or_default().to_owned()
}

/// The library's calls of `setup`'s root, as a terminal there makes them.
fn library(setup: &Setup) -> (Env, Globals) {
    (
        Env {
            cwd: setup.root.clone(),
            home: Some(setup.home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        Globals {
            root: Some(setup.root.clone()),
            config: None,
        },
    )
}

/// A terminal's git: the repository's own identity.
fn terminal_git(setup: &Setup) -> GitEnv {
    GitEnv::new(setup.root.as_path(), product_env(&setup.home))
}

/// A terminal's git, the identity `login@example.org`: what a question quoting
/// the decider reads for that login.
fn git_as(setup: &Setup, login: &str) -> GitEnv {
    let email = format!("{login}@example.org");
    terminal_git(setup)
        .with_var("GIT_COMMITTER_NAME", login)
        .with_var("GIT_COMMITTER_EMAIL", email.as_str())
        .with_var("GIT_AUTHOR_NAME", login)
        .with_var("GIT_AUTHOR_EMAIL", email.as_str())
}

/// A question on `targets` by a developer, stored: when its first ask hits, asked
/// again naming every hit (`docs/canon/agent-intake.md` "Dedup").
fn asked(setup: &Setup, targets: &[&str], text: &str) -> String {
    let (env, globals) = library(setup);
    let mut request = QuestionRequest {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        text: text.to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: utc_now(),
        git: terminal_git(setup),
        card: None,
    };
    let ask = |request: &QuestionRequest| {
        propose_question(&env, &globals, request).unwrap_or_else(|error| panic!("{text}: {error}"))
    };
    let mut outcome = ask(&request);
    if outcome.refusal.is_none() && !outcome.document.created {
        request.distinct_from = outcome
            .document
            .hits
            .iter()
            .map(|hit| hit.name().to_owned())
            .collect();
        outcome = ask(&request);
    }
    assert_eq!(outcome.exit(), Exit::Answered, "{text}: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("the question's ID")
}

/// A discrepancy on `targets` by a developer, two options, stored as [`asked`]
/// stores one, `evidence` its evidence.
fn reported_with(
    setup: &Setup,
    targets: &[&str],
    summary: &str,
    evidence: Vec<Evidence>,
) -> String {
    let (env, globals) = library(setup);
    let input = DiscrepancyInput {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        summary: summary.to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::High,
        evidence,
        options: [
            ("Keep the spec", "Fix the code", "one work item"),
            ("Change the spec", "Allow it", "a new rule case"),
        ]
        .iter()
        .map(|(label, effect, price)| IntakeOption {
            label: (*label).to_owned(),
            effect: (*effect).to_owned(),
            price: (*price).to_owned(),
        })
        .collect(),
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from: None,
    };
    let report = |input: DiscrepancyInput| {
        propose_discrepancy(
            &env,
            &globals,
            &DiscrepancyRequest {
                input,
                author_role: Some("developer".to_owned()),
                author_model: Some("claude-opus-5-5".to_owned()),
                run: None,
                login: None,
                now: utc_now(),
                git: terminal_git(setup),
                card: None,
            },
        )
        .unwrap_or_else(|error| panic!("{summary}: {error}"))
    };
    let mut outcome = report(input.clone());
    if outcome.refusal.is_none() && !outcome.document.created {
        let mut named = input;
        named.distinct_from = Some(
            outcome
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect(),
        );
        outcome = report(named);
    }
    assert_eq!(outcome.exit(), Exit::Answered, "{summary}: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("the discrepancy's ID")
}

/// [`reported_with`], one line of evidence.
fn reported(setup: &Setup, targets: &[&str], summary: &str) -> String {
    reported_with(
        setup,
        targets,
        summary,
        vec![Evidence {
            file: "src/stamina.rs".to_owned(),
            qpath: Some("regen_system".to_owned()),
            lines: Some("10-20".to_owned()),
            observed: "regenerates while sprinting".to_owned(),
            documented: "regenerates only at rest".to_owned(),
        }],
    )
}

/// What `spec approve id` asks a terminal now: the library's own question,
/// declined, so nothing is written (`docs/canon/proposal-apply.md` "Consent").
fn approve_asks(setup: &Setup, id: &str, git: GitEnv) -> String {
    let (env, globals) = library(setup);
    let mut questions = Vec::new();
    let outcome = approve(
        &env,
        &globals,
        &ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: utc_now(),
            git,
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            false
        },
    )
    .unwrap_or_else(|error| panic!("{id}: {error}"));
    assert_eq!(questions.len(), 1, "{id}: one question: {outcome:?}");
    questions.remove(0)
}

/// What `spec reject id` asks a terminal now, declined.
fn reject_asks(setup: &Setup, id: &str, git: GitEnv) -> String {
    let (env, globals) = library(setup);
    let mut questions = Vec::new();
    let outcome = reject(
        &env,
        &globals,
        &RejectRequest {
            id: id.to_owned(),
            reason: None,
            now: utc_now(),
            git,
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            false
        },
    )
    .unwrap_or_else(|error| panic!("{id}: {error}"));
    assert_eq!(questions.len(), 1, "{id}: one question: {outcome:?}");
    questions.remove(0)
}

/// `body` as the hook `name` of the repository at `root`.
fn hook(root: &Path, name: &str, body: &str) {
    let dir = root.join(".git").join("hooks");
    fs::create_dir_all(&dir).expect("hooks directory");
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("hook");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable hook");
}

/// An approve staged.
fn approved_choice() -> Value {
    json!({"decision": "approve"})
}

/// A reject staged, for `reason`.
fn rejected_choice(reason: &str) -> Value {
    json!({"decision": "reject", "reason": reason})
}

/// A refused login's line outside `[team]`.
fn outside_line(login: &str) -> String {
    format!("refused: {login} is not in this project's `[team]`: reads and plain writes")
}

/// A `member`'s line on an act needing a lead.
fn member_line(login: &str) -> String {
    format!("refused: {login} is a `member`: this act needs a `lead`")
}

/// A lead's line outside its zones.
fn zone_line(login: &str, zones: &str, path: &str) -> String {
    format!("refused: {login} leads {zones}; {path} lies outside them")
}

/// `status`, the answer holding `line`, as an error body or as a document's note.
#[track_caller]
fn assert_says(reply: &Reply, status: u16, line: &str, context: &str) {
    assert_eq!(reply.status, status, "{context}: {}", reply.text());
    assert!(
        reply.text().contains(line),
        "{context}: {line:?} not in {}",
        reply.text()
    );
}

/// "Terminal only": a 409 that says where the choice goes.
#[track_caller]
fn assert_terminal(reply: &Reply, context: &str) {
    assert_eq!(reply.status, 409, "{context}: {}", reply.text());
    assert!(
        reply.text().to_lowercase().contains("terminal"),
        "{context}: names no terminal: {}",
        reply.text()
    );
}

// ------------------------------------------------------------------- a desk

/// What a refusal must leave: the repository byte for byte (`git status`, `HEAD`,
/// the refs, the worktrees, every file), every event, each proposal's stored
/// fields, the inbox and the round as a page reads them.
#[derive(Debug, PartialEq)]
struct Left {
    repository: Vec<String>,
    events: Vec<(String, Value)>,
    proposals: Vec<Value>,
    inbox: String,
    round: String,
}

/// A running hub and the project every call here names.
struct Desk {
    hub: Hub,
    base: &'static str,
}

impl Desk {
    fn new(setup: Setup, base: &'static str) -> Self {
        Self {
            hub: setup.start(),
            base,
        }
    }

    fn setup(&self) -> &Setup {
        &self.hub.setup
    }

    fn root(&self) -> &Path {
        &self.hub.setup.root
    }

    /// `.../proposals/<id>/<route>`.
    fn route(&self, id: &str, route: &str) -> String {
        format!("{}/proposals/{id}/{route}", self.base)
    }

    /// The stage of `id` by `login`, against the row as it stands.
    fn stage(&self, login: &str, id: &str, choice: &Value) -> Reply {
        let mut body = choice.clone();
        body["updated_at"] = self.hub.proposal(id)["updated_at"].clone();
        through(
            &self.hub.server,
            "POST",
            &self.route(id, "decision"),
            &Proxied::write(login),
            body.to_string().as_bytes(),
        )
    }

    /// [`Desk::stage`], the answer a 200.
    fn staged(&self, login: &str, id: &str, choice: &Value) {
        let reply = self.stage(login, id, choice);
        assert_eq!(reply.status, 200, "{login} stages {id}: {}", reply.text());
    }

    /// The stage of `id` removed.
    fn unstaged(&self, login: &str, id: &str) {
        through(
            &self.hub.server,
            "DELETE",
            &self.route(id, "decision"),
            &Proxied::write(login),
            b"",
        )
        .status(200);
    }

    /// `GET .../confirm` by `login`.
    fn read(&self, login: &str, id: &str) -> Reply {
        self.hub.get(login, &self.route(id, "confirm"))
    }

    /// [`Desk::read`], a 200, its document.
    fn read_ok(&self, login: &str, id: &str) -> Value {
        let reply = self.read(login, id);
        assert_eq!(reply.status, 200, "{login} reads {id}: {}", reply.text());
        reply.json()
    }

    /// `POST .../confirm` by `login` with the bytes of `body`.
    fn post_bytes(&self, login: &str, id: &str, body: &[u8]) -> Reply {
        through(
            &self.hub.server,
            "POST",
            &self.route(id, "confirm"),
            &Proxied::write(login),
            body,
        )
    }

    /// `POST .../confirm` by `login` of `question`, as of `updated_at`.
    fn answer(&self, login: &str, id: &str, question: &str, updated_at: &Value) -> Reply {
        let body = json!({"question": question, "updated_at": updated_at});
        self.post_bytes(login, id, body.to_string().as_bytes())
    }

    /// `POST .../confirm` by `login`, of what the read showed.
    fn confirm(&self, login: &str, id: &str, read: &Value) -> Reply {
        let question = read["question"].as_str().expect("a question");
        self.answer(login, id, question, &read["updated_at"])
    }

    /// One click: the read and its POST, each a 200: the review document.
    fn confirmed(&self, login: &str, id: &str) -> Value {
        let read = self.read_ok(login, id);
        let reply = self.confirm(login, id, &read);
        assert_eq!(reply.status, 200, "{login} confirms {id}: {}", reply.text());
        reply.json()
    }

    /// See [`Left`].
    fn left(&self, ids: &[&str]) -> Left {
        let hub = &self.hub;
        Left {
            repository: hub.repository(),
            events: hub.events(),
            proposals: ids.iter().map(|id| hub.proposal(id)).collect(),
            inbox: self.page("inbox"),
            round: self.page("round"),
        }
    }

    /// `GET <base>/path` as a page reads it, a 200: its bytes.
    fn page(&self, path: &str) -> String {
        self.hub
            .get("tomas", &format!("{}/{path}", self.base))
            .status(200)
            .text()
            .to_owned()
    }

    /// The event types stored after the first `seen`.
    fn added(&self, seen: usize) -> Vec<String> {
        self.hub.events()[seen..]
            .iter()
            .map(|(kind, _)| kind.clone())
            .collect()
    }

    fn git(&self, args: &[&str]) -> String {
        self.setup().scratch.git().run(self.root(), args)
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// `commit`'s parents.
    fn parents(&self, commit: &str) -> Vec<String> {
        self.git(&["rev-list", "--parents", "-n", "1", commit])
            .split_whitespace()
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    /// The paths `commit` changes.
    fn paths(&self, commit: &str) -> Vec<String> {
        self.git(&[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "--root",
            commit,
        ])
        .lines()
        .map(str::to_owned)
        .collect()
    }

    /// The values of `key` in `commit`'s message.
    fn trailers(&self, commit: &str, key: &str) -> Vec<String> {
        let prefix = format!("{key}: ");
        self.git(&["log", "-1", "--format=%B", commit])
            .lines()
            .filter_map(|line| line.strip_prefix(prefix.as_str()))
            .map(str::to_owned)
            .collect()
    }

    /// `commit` under `format`, for `git log`.
    fn shown(&self, commit: &str, format: &str) -> String {
        self.git(&["log", "-1", &format!("--format={format}"), commit])
            .trim()
            .to_owned()
    }

    /// B1's four places: `person` in the commit's `Decided-by`, author and
    /// committer, and in the queue's `decided_by`.
    #[track_caller]
    fn assert_decided(&self, id: &str, commit: &str, person: &str) {
        assert_eq!(
            self.trailers(commit, "Decided-by"),
            [person],
            "{id}: trailer"
        );
        assert_eq!(self.shown(commit, "%an <%ae>"), person, "{id}: author");
        assert_eq!(self.shown(commit, "%cn <%ce>"), person, "{id}: committer");
        assert_eq!(
            self.hub.proposal(id)["decided_by"],
            json!(person),
            "{id}: the queue"
        );
    }
}

/// A desk over the print shop with an `update` of SHT-BACK staged by `tomas`,
/// which `lena` covers and `mira` does not: the desk and its ID.
fn back_update_staged(name: &str) -> (Desk, String) {
    let setup = press(name);
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());
    (desk, id)
}

// ------------------------------------------------------------------- the read

/// "Data": the read is `{id, question, updated_at}` in that order, and `question`
/// is the text `spec approve` or `spec reject` asks, byte for byte: worked out
/// here from `decision-staging.md` "Terminal" and asked of the library. It adds no
/// event, changes no file, and repeats byte for byte. M: `confirm_question`
/// writing its own text.
#[test]
fn the_read_is_the_terminals_question_and_changes_nothing() {
    let setup = press("confirm-read");
    let update = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let turned = reworded(&setup, "SHT-INNER-FOLD", "No headline", "No caption");
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &update, &approved_choice());
    desk.staged("tomas", &turned, &rejected_choice("Asked \"before\"."));
    let root = desk.root().display().to_string();
    let ids = [update.as_str(), turned.as_str()];
    let before = desk.left(&ids);

    let at = desk.hub.proposal(&update)["staged_at"].clone();
    let at = at.as_str().expect("staged_at");
    let want = format!(
        "staged {at}: spec approve {update}\napply {update} to {BACK_MD} on main in {root} \
         (applies; raises SHT-BACK 1 -> 2, staged)? [y/N]"
    );
    let reply = desk.read("ann", &update);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.text(),
        format!(
            "{{\"id\":\"{update}\",\"question\":{},\"updated_at\":\"{at}\"}}",
            Value::from(want.as_str())
        ),
        "the read's bytes"
    );
    assert_eq!(
        want,
        approve_asks(desk.setup(), &update, terminal_git(desk.setup())),
        "the library's question"
    );

    let at = desk.hub.proposal(&turned)["staged_at"].clone();
    let at = at.as_str().expect("staged_at");
    let want = format!(
        "staged {at}: spec reject {turned} --reason \"Asked \\\"before\\\".\"\nreject {turned} \
         (SHT-INNER-FOLD in {INNER_MD} on main in {root}, staged)? [y/N]"
    );
    let read = desk.read_ok("ann", &turned);
    assert_eq!(read["question"], json!(want));
    assert_eq!(
        want,
        reject_asks(desk.setup(), &turned, terminal_git(desk.setup())),
        "the library's question"
    );

    let first = desk.read("ann", &update);
    let second = desk.read("ann", &update);
    assert_eq!(first.body, second.body, "a read repeats byte for byte");
    assert_eq!(desk.left(&ids), before, "a read writes nothing");
}

/// The read of a kind that decides and of a new file on the game: the record's
/// whole text, the choice, under the asking login's identity, is what a terminal
/// of that identity is asked. M: `confirm_question` skipping the record's text.
#[test]
fn the_read_is_the_terminals_question_for_a_record_and_a_new_file() {
    let setup = keep("confirm-read-record");
    let question = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let gap = reported(
        &setup,
        &["RULE-SPRINT-COST"],
        "The cost differs in the build.",
    );
    let created = new_record(&setup, "R-13", "docs/records/R/R-13.md");
    let desk = Desk::new(setup, KEEP);
    desk.staged("mira", &question, &approved_choice());
    desk.staged("mira", &gap, &json!({"decision": "approve", "option": 1}));
    desk.staged("ann", &created, &approved_choice());
    let ids = [question.as_str(), gap.as_str(), created.as_str()];
    let before = desk.left(&ids);
    for id in [&question, &gap] {
        let read = desk.read_ok("mira", id);
        assert_eq!(
            read["question"],
            json!(approve_asks(desk.setup(), id, git_as(desk.setup(), "mira"))),
            "{id}"
        );
        let text = read["question"].as_str().expect("a question");
        assert!(text.contains(MIRA), "{id}: the record's decider: {text}");
        // docs/features/spec-ids.md "Data", "Records": a hub's records from
        // 100001.
        assert!(text.contains(" as DEC-100001 ("), "{id}: its ID: {text}");
    }
    let read = desk.read_ok("ann", &created);
    assert_eq!(
        read["question"],
        json!(approve_asks(
            desk.setup(),
            &created,
            git_as(desk.setup(), "ann")
        ))
    );
    assert_eq!(desk.left(&ids), before, "a read writes nothing");
}

// ------------------------------------------------------------------------ B1

/// B1: `mira` (lead) and `ann` (owner) confirm two proposals, the checkout's
/// `user.name` being `Owner`: two commits, each with its login's
/// `Decided-by: <login> <email>`, its author and committer, and the queue's
/// `decided_by`, all one string; `name` is display, and the checkout's config is
/// as it was. M: a default committer.
#[test]
fn b1_each_login_decides_and_commits_under_its_own_name() {
    let setup = press("confirm-names");
    let front = reworded(
        &setup,
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    let back = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    assert_eq!(desk.git(&["config", "user.name"]).trim(), OWNER_NAME);
    let config = read_text(desk.root(), ".git/config");
    desk.staged("tomas", &front, &approved_choice());
    desk.staged("tomas", &back, &approved_choice());

    for (login, id, person, path) in [
        ("mira", &front, MIRA, FRONT_MD),
        ("ann", &back, ANN, BACK_MD),
    ] {
        let parent = desk.head();
        let document = desk.confirmed(login, id);
        let commit = desk.head();
        assert_eq!(document["status"], json!("applied"), "{document}");
        assert_eq!(document["applied_commit"], json!(commit), "{document}");
        assert_eq!(desk.parents(&commit), [parent], "{id}: one commit");
        assert_eq!(desk.paths(&commit), [path], "{id}");
        desk.assert_decided(id, &commit, person);
    }
    assert_eq!(
        read_text(desk.root(), ".git/config"),
        config,
        "the login is the commit's, never the checkout's"
    );
}

/// B1 on the game, the other two places that commit: a question's record by a
/// lead, a new file by the owner, a discrepancy's record by a lead, each commit
/// and queue under the login, the record's `Decided by:` too. M: a default
/// committer in `decide.rs` or `create.rs`.
#[test]
fn b1_a_record_and_a_new_file_are_committed_under_the_login() {
    let setup = keep("confirm-names-record");
    let question = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let created = new_record(&setup, "R-13", "docs/records/R/R-13.md");
    let gap = reported(&setup, &["EDGE-STAM-ZERO"], "The bar empties twice.");
    let desk = Desk::new(setup, KEEP);
    desk.staged("mira", &question, &approved_choice());
    desk.staged("ann", &created, &approved_choice());
    desk.staged("kit", &gap, &json!({"decision": "approve", "option": 0}));

    let document = desk.confirmed("mira", &question);
    let commit = desk.head();
    desk.assert_decided(&question, &commit, MIRA);
    // docs/features/spec-ids.md "Data", "Records": a hub's records from
    // 100001.
    assert_eq!(document["record_id"], json!("DEC-100001"), "{document}");
    assert_eq!(desk.paths(&commit), ["docs/records/DEC/DEC-100001.md"]);
    assert!(
        read_text(desk.root(), "docs/records/DEC/DEC-100001.md").contains(MIRA),
        "the record's decider"
    );

    desk.confirmed("ann", &created);
    let commit = desk.head();
    desk.assert_decided(&created, &commit, ANN);
    assert_eq!(desk.paths(&commit), ["docs/records/R/R-13.md"]);

    desk.confirmed("kit", &gap);
    let commit = desk.head();
    desk.assert_decided(&gap, &commit, "kit <kit@example.org>");
    assert_eq!(desk.paths(&commit), ["docs/records/DEC/DEC-100002.md"]);
}

/// B1, "The terminal keeps its identity, on the host too": a confirm leaves the
/// checkout and the process as they were, so a `spec approve` beside the running
/// hub commits under the repository's identity. M: the login written into the
/// checkout's config.
#[test]
fn b1_the_terminal_keeps_its_identity_on_the_host() {
    let setup = press("confirm-terminal");
    let front = reworded(
        &setup,
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    let back = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &front, &approved_choice());
    desk.confirmed("mira", &front);

    let (env, globals) = library(desk.setup());
    let outcome = approve(
        &env,
        &globals,
        &ApproveRequest {
            id: back.clone(),
            note: None,
            now: utc_now(),
            git: terminal_git(desk.setup()),
        },
        &mut |_: &str| true,
    )
    .expect("a terminal's approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    desk.assert_decided(&back, &desk.head(), OWNER);
}

// ------------------------------------------------------------------------ B2

/// A bare `origin` of `setup`'s `main`, in step with the checkout, which tracks
/// it: a push from here lands.
fn origin_of(setup: &Setup) -> PathBuf {
    let git = setup.scratch.git();
    let bare = setup.scratch.dir("origin.git");
    git.run(
        &bare,
        &[
            "init",
            "-q",
            "--bare",
            "--template=",
            "--initial-branch=main",
        ],
    );
    let path = bare.to_str().expect("UTF-8 path");
    git.run(&setup.root, &["remote", "add", "origin", path]);
    git.run(&setup.root, &["push", "-q", "-u", "origin", "main"]);
    bare
}

/// A commit someone else pushed to `bare`: a fetch or a merge from the checkout
/// would show, a plain push would be refused.
fn pushed_from_elsewhere(setup: &Setup, bare: &Path) {
    let git = setup.scratch.git();
    let elsewhere = setup.scratch.join("elsewhere");
    let from = bare.to_str().expect("UTF-8 path");
    let to = elsewhere.to_str().expect("UTF-8 path");
    git.run(setup.scratch.path(), &["clone", "-q", from, to]);
    write(&elsewhere, "notes.md", "A note from elsewhere.\n");
    git.run(&elsewhere, &["add", "-A"]);
    git.run(&elsewhere, &["commit", "-q", "-m", "elsewhere"]);
    git.run(&elsewhere, &["push", "-q", "origin", "main"]);
}

/// B2: past a confirm the checkout has one commit more, on the recorded branch,
/// whose one parent is the old `HEAD`: one path, `Proposal:`, `git status` clean.
/// The bare `origin` stands in step, so a push from here would land: its refs and
/// its every byte are as they were. M: a push.
#[test]
fn b2_a_confirm_is_one_commit_of_one_path_and_pushes_nothing() {
    let setup = press("confirm-one-commit");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let bare = origin_of(&setup);
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());

    let git = desk.setup().scratch.git();
    let refs = ["for-each-ref", "--format=%(refname) %(objectname)"];
    let base = desk.head();
    let branch = desk.git(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let remote_refs = git.run(&bare, &refs);
    let remote_bytes = snapshot(&bare);
    assert!(remote_refs.contains(&base), "in step: {remote_refs}");

    let document = desk.confirmed("lena", &id);

    let head = desk.head();
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(document["applied_commit"], json!(head), "{document}");
    assert_eq!(desk.parents(&head), [base], "one commit, on the old `HEAD`");
    assert_eq!(desk.paths(&head), [BACK_MD], "one path");
    assert_eq!(desk.trailers(&head, "Proposal"), [id.as_str()]);
    assert_eq!(
        desk.git(&["status", "--porcelain", "--untracked-files=all"]),
        "",
        "the checkout is clean"
    );
    assert_eq!(
        desk.git(&["rev-parse", "--abbrev-ref", "HEAD"]),
        branch,
        "on the recorded branch"
    );
    assert!(
        read_text(desk.root(), BACK_MD).contains("the moon table"),
        "the proposal's text"
    );
    assert_eq!(git.run(&bare, &refs), remote_refs, "the origin's refs");
    assert_eq!(snapshot(&bare), remote_bytes, "the origin's every byte");
}

/// B2, "9 ends": another clone moved `origin` on first, and a confirm neither
/// fetches nor merges: the checkout's view of the remote stands and its one new
/// commit sits on the old `HEAD`, the operator's to reconcile. M: a fetch; a merge.
#[test]
fn b2_a_confirm_fetches_and_merges_nothing() {
    let setup = press("confirm-no-fetch");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let bare = origin_of(&setup);
    pushed_from_elsewhere(&setup, &bare);
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());

    let base = desk.head();
    let tracked = desk.git(&["for-each-ref", "refs/remotes"]);
    let remote = snapshot(&bare);
    assert!(
        tracked.contains(&base),
        "the checkout saw no more: {tracked}"
    );

    desk.confirmed("lena", &id);

    let head = desk.head();
    assert_eq!(desk.parents(&head), [base], "no merge");
    assert_eq!(desk.paths(&head), [BACK_MD], "no other file came in");
    assert_eq!(
        desk.git(&["for-each-ref", "refs/remotes"]),
        tracked,
        "no fetch"
    );
    assert_eq!(snapshot(&bare), remote, "the origin's every byte");
}

/// A card as stored, what a computed mark leaves out: a stored change shows
/// here. Its history's own `card.*` events alone: since
/// docs/features/card-questions.md K1 the read merges its items' events in,
/// and a departing accept's discrepancy is one of its items.
fn card_stored(hub: &Hub, id: &str) -> Value {
    let mut shown = hub.show(id);
    if let Some(history) = shown["history"].as_array_mut() {
        history.retain(|entry| {
            entry["type"]
                .as_str()
                .is_some_and(|kind| kind.starts_with("card."))
        });
    }
    shown.as_object_mut().expect("a card").retain(|key, _| {
        [
            "id",
            "column",
            "revision",
            "history",
            "acceptance",
            "cancellation",
            "assignee",
            "reviewer",
            "targets",
            "updated_at",
        ]
        .contains(&key.as_str())
    });
    shown
}

/// A `plates` card on `target`, `ana` its assignee and `tomas` its reviewer.
fn card_on(hub: &Hub, title: &str, target: &str) -> String {
    let made = hub.done(
        "ana",
        "cards",
        &json!({"title": title, "discipline": "plates", "target": [target], "assignee": "ana"}),
    );
    let id = made["id"].as_str().expect("a card's ID").to_owned();
    hub.done(
        "ana",
        &format!("cards/{id}/assign"),
        &json!({"reviewer": "tomas"}),
    );
    id
}

/// [`card_on`], moved to `accept.from`.
fn ready_card(hub: &Hub, title: &str, target: &str) -> String {
    let id = card_on(hub, title, target);
    hub.done(
        "ana",
        &format!("cards/{id}/move"),
        &json!({"to": "checked"}),
    );
    id
}

/// The card's `revision`, a string on the wire.
fn revision(hub: &Hub, id: &str) -> String {
    hub.show(id)["revision"]
        .as_str()
        .expect("a revision")
        .to_owned()
}

/// A departing accept by `tomas` of `card`, on `node`: the discrepancy's ID.
fn departed(hub: &Hub, card: &str, node: &str) -> String {
    let seen = hub.events().len();
    hub.done(
        "tomas",
        &format!("cards/{card}/accept"),
        &json!({
            "departs": [node],
            "summary": "The result left the brief.",
            "observed": "The lead runs below the fold.",
            "documented": "The lead runs above the fold.",
            "revision": revision(hub, card),
        }),
    );
    let events = hub.events();
    let created: Vec<&Value> = events[seen..]
        .iter()
        .filter(|(kind, _)| kind == "proposal.created")
        .map(|(_, payload)| payload)
        .collect();
    assert_eq!(created.len(), 1, "one item: {created:?}");
    created[0]["id"].as_str().expect("the item's ID").to_owned()
}

/// B2: a rejected item adds no commit, and it touches no card: the departing
/// discrepancy a card is linked to and a plain `update` on the same node, each
/// staged as a reject and confirmed, add `proposal.rejected` alone, the two cards
/// stored as they were, git byte for byte, and the open card still moves
/// (`cards.md`: a rejected brief stops no card). M: a reject touching a card.
#[test]
fn b2_a_rejected_one_adds_no_commit_and_its_cards_still_move() {
    let desk = Desk::new(press("confirm-reject-cards"), P);
    let hub = &desk.hub;
    let accepted = ready_card(hub, "Lead story", "SHT-FRONT");
    let open = card_on(hub, "Weather box", "SHT-FRONT");
    let item = departed(hub, &accepted, "SHT-FRONT");
    let update = reworded(
        desk.setup(),
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    for (id, reason, to) in [
        (&item, "The brief stands.", "blocked"),
        (&update, "Not this week.", "checked"),
    ] {
        desk.staged("tomas", id, &rejected_choice(reason));
        let repository = hub.repository();
        let cards = [card_stored(hub, &accepted), card_stored(hub, &open)];
        let seen = hub.events().len();

        let document = desk.confirmed("mira", id);

        assert_eq!(document["status"], json!("rejected"), "{document}");
        assert_eq!(document["decision_note"], json!(reason), "{document}");
        assert_eq!(hub.repository(), repository, "{id}: no commit, no file");
        assert_eq!(desk.added(seen), ["proposal.rejected"], "{id}: one event");
        assert_eq!(
            [card_stored(hub, &accepted), card_stored(hub, &open)],
            cards,
            "{id}: no card stored anew"
        );
        hub.done("ana", &format!("cards/{open}/move"), &json!({"to": to}));
        assert_eq!(hub.show(&open)["column"], json!(to), "{id}: the card moved");
    }
}

// ------------------------------------------------------------------------ B3

/// B3 on the print shop's `update`s, judged by `target_path`: a lead outside its
/// zone and a lead facing a path in no zone are refused the read and the POST,
/// each with its line in the error body, nothing stored; the owner's own read
/// lands. The refused POST carries the owner's question, so nothing but the cover
/// stops it. A lead inside its zone lands too (B1). M: an unzoned path read as
/// covered; the cover answered as a document.
#[test]
fn b3_a_lead_outside_its_zone_or_on_an_unzoned_path_is_refused_and_the_owner_is_not() {
    let setup = press("confirm-zones");
    let back = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let inner = reworded(
        &setup,
        "SHT-INNER",
        "carries the long reads",
        "carries the short reads",
    );
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &back, &approved_choice());
    desk.staged("tomas", &inner, &approved_choice());
    let ids = [back.as_str(), inner.as_str()];
    let before = desk.left(&ids);

    for (login, id, lead_line) in [
        ("mira", &back, zone_line("mira", "front", BACK_MD)),
        ("mira", &inner, zone_line("mira", "front", INNER_MD)),
        ("lena", &inner, zone_line("lena", "front, back", INNER_MD)),
    ] {
        let context = format!("{login} on {id}");
        assert_refused(&desk.read(login, id), &lead_line, &context);
        let read = desk.read_ok("ann", id);
        assert_refused(&desk.confirm(login, id, &read), &lead_line, &context);
    }
    assert_eq!(desk.left(&ids), before, "a refused lead stored nothing");

    desk.confirmed("ann", &back);
    desk.confirmed("ann", &inner);
    assert_eq!(desk.hub.proposal(&back)["status"], json!("applied"));
    assert_eq!(desk.hub.proposal(&inner)["status"], json!("applied"));
    desk.assert_decided(&inner, &desk.head(), ANN);
}

/// B3, "Cover": a kind that decides is covered by every target's holder, never by
/// `target_path`, which is the first target alone. A discrepancy over two zones
/// is refused a lead of the first, whom `target_path` would pass, and a lead of
/// the second; a lead of both reads it and the owner lands it. M: judging
/// `target_path` alone.
#[test]
fn b3_a_discrepancy_spanning_two_zones_is_refused_each_lead_of_one() {
    let setup = keep("confirm-two-zones");
    let gap = reported(
        &setup,
        &["MEC-SPRINT", "MEC-STAMINA"],
        "The drain and the sprint disagree.",
    );
    let question = asked(
        &setup,
        &["RULE-SPRINT-COST", "EDGE-STAM-ZERO"],
        "Does a sprint end at zero?",
    );
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &gap, &json!({"decision": "approve", "option": 0}));
    desk.staged("ann", &question, &approved_choice());
    let ids = [gap.as_str(), question.as_str()];
    let before = desk.left(&ids);

    for id in [&gap, &question] {
        for (login, line) in [
            ("mira", zone_line("mira", "sprint", STAMINA_MD)),
            ("kit", zone_line("kit", "stamina", SPRINT_MD)),
        ] {
            let context = format!("{login} on {id}");
            assert_refused(&desk.read(login, id), &line, &context);
            let read = desk.read_ok("ann", id);
            assert_refused(&desk.confirm(login, id, &read), &line, &context);
        }
    }
    assert_eq!(desk.left(&ids), before, "each lead stored nothing");

    desk.read_ok("lena", &gap);
    let document = desk.confirmed("lena", &question);
    assert_eq!(document["status"], json!("applied"), "{document}");
    let document = desk.confirmed("ann", &gap);
    assert_eq!(document["status"], json!("applied"), "{document}");
    desk.assert_decided(&gap, &desk.head(), ANN);
}

/// B3, "Cover": a gone target is the owner's alone, a lead's zones covering
/// everything left: its holder is resolved now, at the read. M: a gone target
/// skipped by the cover.
#[test]
fn b3_a_gone_target_is_refused_every_lead() {
    let setup = keep("confirm-gone");
    let question = asked(
        &setup,
        &["RULE-SPRINT-COST", "EDGE-SPRINT-EMPTY"],
        "Does an empty tank end a sprint?",
    );
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &question, &approved_choice());
    desk.read_ok("mira", &question);
    let root = desk.root();
    replace(
        root,
        SPRINT_MD,
        "### Empty tank {#EDGE-SPRINT-EMPTY .playtest}\n",
        "### Empty tank\n",
    );
    desk.git(&["commit", "-qam", "the section loses its ID"]);
    let before = desk.left(&[question.as_str()]);
    let reply = desk.read("mira", &question);
    assert_eq!(reply.status, 403, "{}", reply.text());
    let line = reply.error_message();
    assert!(
        line.starts_with("refused: mira leads sprint; ")
            && line.ends_with(": this act needs an `owner`"),
        "{line}"
    );
    assert_eq!(desk.left(&[question.as_str()]), before, "nothing stored");
    let reply = desk.read("ann", &question);
    assert_ne!(reply.status, 403, "the owner's: {}", reply.text());
}

/// B3: a `member` and a login outside `[team]` are refused the read and the POST
/// with their lines, the owner's own question in hand, nothing stored; a project
/// with no `[team]` and a bad one answer as the confirm class does. M: an absent
/// entry read as a role; a member taken as a lead.
#[test]
fn b3_a_member_and_a_login_outside_team_are_refused_and_nothing_is_stored() {
    let (desk, id) = back_update_staged("confirm-outsiders");
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let read = desk.read_ok("ann", &id);
    for (login, line) in [
        ("tomas", member_line("tomas")),
        ("eve", outside_line("eve")),
    ] {
        assert_refused(&desk.read(login, &id), &line, login);
        assert_refused(&desk.confirm(login, &id, &read), &line, login);
    }
    assert_eq!(desk.left(&ids), before, "nothing stored");

    let root = desk.root();
    let good = read_text(root, "specengine.toml");
    let bad = good.replace("role = \"member\"", "role = \"editor\"");
    assert_ne!(bad, good, "a role off the three");
    write(root, "specengine.toml", bad);
    let reply = desk.read("ann", &id);
    assert_eq!(reply.status, 503, "{}", reply.text());
    let reply = desk.confirm("ann", &id, &read);
    assert_eq!(reply.status, 503, "{}", reply.text());
    write(root, "specengine.toml", &good);
    assert_eq!(desk.left(&ids), before, "a bad `[team]` confirmed nothing");
}

/// B3, the line of a project with no `[team]`: nobody confirms, the proposal
/// stands. M: no `[team]` read as an open door.
#[test]
fn b3_a_project_with_no_team_confirms_nothing() {
    let setup = Setup::new("confirm-no-team", "card-press");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    assert_refused(&desk.read("ann", &id), NO_TEAM_LINE, "the read");
    let reply = desk.answer("ann", &id, "any", &json!("2026-11-02T09:00:00Z"));
    assert_refused(&reply, NO_TEAM_LINE, "the POST");
    assert_eq!(desk.left(&ids), before, "nothing stored");
}

// ------------------------------------------------------------------------ B4

/// B4: only the library's question, byte for byte, consents: any other text, the
/// question of another proposal, or a read of another `updated_at` is a 409 with
/// nothing changed, and past them the read's own question lands. M: an always-yes
/// closure.
#[test]
fn b4_a_question_other_than_the_librarys_is_a_409_and_nothing_changes() {
    let setup = press("confirm-question");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let other = reworded(
        &setup,
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());
    desk.staged("tomas", &other, &approved_choice());
    let ids = [id.as_str(), other.as_str()];
    let before = desk.left(&ids);
    let read = desk.read_ok("lena", &id);
    let question = read["question"].as_str().expect("a question");
    let (staged_line, asked_line) = question.split_once('\n').expect("two lines");

    for text in [
        format!("{question} "),
        format!(" {question}"),
        question.replace('\n', "\r\n"),
        question.replace("[y/N]", "[Y/n]"),
        question.to_uppercase(),
        asked_line.to_owned(),
        staged_line.to_owned(),
        "y".to_owned(),
        "yes".to_owned(),
    ] {
        let reply = desk.answer("lena", &id, &text, &read["updated_at"]);
        assert_says(&reply, 409, MOVED_LINE, &text);
        assert_eq!(desk.left(&ids), before, "{text:?}: nothing changed");
    }
    let foreign = desk.read_ok("lena", &other);
    let reply = desk.confirm("lena", &id, &foreign);
    assert_eq!(reply.status, 409, "another's: {}", reply.text());
    assert_eq!(desk.hub.proposal(&id)["status"], json!("open"));
    let reply = desk.answer("lena", &id, question, &json!("2026-01-01T00:00:00Z"));
    assert_says(&reply, 409, CHANGED_LINE, "another version");
    assert_eq!(desk.left(&ids), before, "nothing changed");

    desk.confirm("lena", &id, &read).status(200);
}

/// The back sheet in five paragraphs, so a change of its first and a change of
/// its last merge clean.
const BACK_PARAGRAPHS: &str = "\n\nThe notices run in two columns.\n\nThe small ads close the \
                               page.\n\nThe weather runs along the foot.\n";

/// B4: the target moves after the read: a commit beside the proposal turns
/// `applies` into `rebases`, a merge a person never saw. The old read is a 409,
/// the branch where the commit left it, the row as it was, no event: the closure
/// said no. A new read names the rebase, and that click lands with both edits.
/// M: an always-yes closure.
#[test]
fn b4_a_target_moved_after_the_read_is_a_409_and_a_new_read_lands() {
    let setup = press("confirm-moved");
    let long = read_text(&setup.root, BACK_MD).trim_end().to_owned() + BACK_PARAGRAPHS;
    write(&setup.root, BACK_MD, long);
    committed(&setup, "a longer sheet");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());
    let read = desk.read_ok("lena", &id);
    let question = read["question"].as_str().expect("a question");
    assert!(question.contains("(applies; raises"), "{question}");

    replace(
        desk.root(),
        BACK_MD,
        "The weather runs along the foot.",
        "The weather runs along the head.",
    );
    desk.git(&["commit", "-qam", "by hand"]);
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    let reply = desk.confirm("lena", &id, &read);
    assert_says(&reply, 409, MOVED_LINE, "a rebase");
    assert_eq!(desk.left(&ids), before, "nothing changed");
    assert_eq!(desk.hub.proposal(&id)["status"], json!("open"));

    let fresh = desk.read_ok("lena", &id);
    let text = fresh["question"].as_str().expect("a question");
    assert!(text.contains("(rebases; raises"), "{text}");
    assert_ne!(fresh["question"], read["question"]);
    desk.confirm("lena", &id, &fresh).status(200);
    let written = read_text(desk.root(), BACK_MD);
    assert!(written.contains("the moon table"), "{written}");
    assert!(written.contains("along the head"), "{written}");
}

/// B4: a target's file renamed after the read refuses at step 3, a 409, no
/// commit, the row and its stage kept. A POST is a real attempt, so that refusal
/// logs its one `proposal.apply_failed`, as a terminal's would. M: an always-yes
/// closure; a POST rehearsing.
#[test]
fn b4_a_target_file_renamed_after_the_read_is_a_409() {
    let (desk, id) = back_update_staged("confirm-renamed");
    let read = desk.read_ok("lena", &id);
    desk.git(&["mv", BACK_MD, "docs/spec/sht-rear.md"]);
    desk.git(&["commit", "-qm", "by hand"]);
    let repository = desk.hub.repository();
    let row = desk.hub.proposal(&id);
    let seen = desk.hub.events().len();

    let reply = desk.confirm("lena", &id, &read);
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(desk.hub.repository(), repository, "no commit");
    assert_eq!(desk.hub.proposal(&id), row, "the row and its stage kept");
    assert_eq!(desk.added(seen), ["proposal.apply_failed"], "one attempt");
}

/// B4, "another record ID": two records wait; the read of one names DEC-100001
/// (a hub's records from 100001 since docs/features/spec-ids.md "Data",
/// "Records"), the other takes that ID first. The old read is a 409, nothing
/// written; a new read names DEC-100002 and lands. M: an always-yes closure.
#[test]
fn b4_another_record_id_after_the_read_is_a_409() {
    let setup = keep("confirm-record-id");
    let first = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let second = asked(&setup, &["MEC-STAMINA"], "Does stamina drain at rest?");
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &first, &approved_choice());
    desk.staged("ann", &second, &approved_choice());
    let read = desk.read_ok("ann", &first);
    let question = read["question"].as_str().expect("a question");
    assert!(question.contains(" as DEC-100001 ("), "{question}");

    desk.confirmed("ann", &second);
    let ids = [first.as_str(), second.as_str()];
    let before = desk.left(&ids);
    let reply = desk.confirm("ann", &first, &read);
    assert_says(&reply, 409, MOVED_LINE, "another record ID");
    assert_eq!(desk.left(&ids), before, "nothing changed");

    let fresh = desk.read_ok("ann", &first);
    let text = fresh["question"].as_str().expect("a question");
    assert!(text.contains(" as DEC-100002 ("), "{text}");
    let document = desk.confirm("ann", &first, &fresh).status(200).json();
    assert_eq!(document["record_id"], json!("DEC-100002"), "{document}");
}

// ------------------------------------------------------------------------ B5

/// B5: a reject staged, on an `update` and on a discrepancy, is rejected: its
/// reason the note, the row `rejected`, `decided_by` the login, its stage's time
/// on the event; nothing is applied, `HEAD` and every file as they were, and the
/// read asked `reject`, never `apply`. M: always calling approve.
#[test]
fn b5_a_staged_reject_rejects_and_never_applies() {
    let (desk, id) = back_update_staged("confirm-reject");
    let reason = "Not this week.";
    desk.staged("lena", &id, &rejected_choice(reason));
    let read = desk.read_ok("lena", &id);
    let question = read["question"].as_str().expect("a question");
    assert!(question.contains(&format!("\nreject {id} (")), "{question}");
    assert!(!question.contains("apply"), "{question}");
    let repository = desk.hub.repository();
    let at = desk.hub.proposal(&id)["staged_at"].clone();
    let seen = desk.hub.events().len();

    let document = desk.confirm("lena", &id, &read).status(200).json();

    assert_eq!(document["status"], json!("rejected"), "{document}");
    assert_eq!(document["decision_note"], json!(reason), "{document}");
    assert_eq!(desk.hub.repository(), repository, "nothing applied");
    assert_eq!(desk.added(seen), ["proposal.rejected"]);
    let stored = desk.hub.proposal(&id);
    assert_eq!(stored["decided_by"], json!(LENA), "{stored}");
    let (_, payload) = desk.hub.events().pop().expect("an event");
    assert_eq!(payload["staged_at"], at, "{payload}");

    let setup = keep("confirm-reject-record");
    let gap = reported(&setup, &["MEC-SPRINT"], "The sprint drains twice.");
    let desk = Desk::new(setup, KEEP);
    desk.staged("mira", &gap, &rejected_choice("Not a gap."));
    let repository = desk.hub.repository();
    let document = desk.confirmed("mira", &gap);
    assert_eq!(document["status"], json!("rejected"), "{document}");
    assert_eq!(document["decided_by"], json!(MIRA), "{document}");
    assert_eq!(desk.hub.repository(), repository, "no record");
}

/// B5, "A confirm takes a stage, never a choice": with nothing staged the read is
/// a 409 and the POST of the text a terminal would ask, the true question of the
/// proposal, is a 409 too, nothing applied. M: confirming with no stage.
#[test]
fn b5_nothing_staged_is_a_409_even_for_the_terminals_own_question() {
    let setup = press("confirm-nothing-staged");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let reply = desk.read("lena", &id);
    assert_eq!(reply.status, 409, "{}", reply.text());
    let question = approve_asks(desk.setup(), &id, terminal_git(desk.setup()));
    let stored = desk.hub.proposal(&id);
    let reply = desk.answer("lena", &id, &question, &stored["updated_at"]);
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(desk.left(&ids), before, "nothing applied");
}

/// `choice` staged on `id` on the clock reading of the stage it replaces, so
/// `staged_at` and `updated_at` do not move: the second a stage's compare cannot
/// see into (`decision-staging.md` "Gap"). The library's own call, whose clock is
/// injected: a test cannot steer the daemon's.
fn replaced_in_the_same_second(desk: &Desk, id: &str, choice: &Value) {
    let stored = desk.hub.proposal(id);
    let at = stored["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let mut body = choice.clone();
    body["updated_at"] = json!(at);
    let (env, globals) = library(desk.setup());
    let outcome = stage(
        &env,
        &globals,
        &StageRequest {
            id: id.to_owned(),
            body: StageBody::from_json(body.to_string().as_bytes()).expect("a stage"),
            now: at,
            git: terminal_git(desk.setup()),
            actor: None,
        },
    )
    .expect("a stage");
    assert_eq!(outcome.cause, None, "{outcome:?}");
}

/// B5: the read's stage replaced in the same second, by another approve and by a
/// reject: `updated_at` and `staged_at` are what the read saw, so the compare of
/// `updated_at` passes and the question alone, whose staged line shows the
/// choice, answers the 409, nothing applied, the row as the replacing act left
/// it. Taken away, nothing is staged. A new read of what stands then lands. M: an
/// always-yes closure; always calling approve.
#[test]
fn b5_a_stage_replaced_or_removed_after_the_read_is_a_409() {
    let (desk, id) = back_update_staged("confirm-replaced");
    let ids = [id.as_str()];
    let read = desk.read_ok("lena", &id);
    let replacing = [
        json!({"decision": "approve", "note": "Only the wording."}),
        rejected_choice("Not this week."),
    ];
    for choice in &replacing {
        replaced_in_the_same_second(&desk, &id, choice);
        let stored = desk.hub.proposal(&id);
        assert_eq!(
            stored["updated_at"], read["updated_at"],
            "one clock reading"
        );
        assert_eq!(stored["staged_at"], read["updated_at"], "one clock reading");
        let before = desk.left(&ids);
        let reply = desk.confirm("lena", &id, &read);
        assert_says(&reply, 409, MOVED_LINE, &choice.to_string());
        assert_eq!(desk.left(&ids), before, "{choice}: nothing changed");
    }
    desk.unstaged("tomas", &id);
    let before = desk.left(&ids);
    let reply = desk.confirm("lena", &id, &read);
    assert_eq!(reply.status, 409, "removed: {}", reply.text());
    assert_eq!(desk.left(&ids), before, "removed: nothing changed");

    desk.staged("tomas", &id, &rejected_choice("Not this week."));
    let document = desk.confirmed("lena", &id);
    assert_eq!(document["status"], json!("rejected"), "{document}");
}

// ------------------------------------------------------------------------ B6

/// B6: a staged `editorial: true` stays a terminal's: the read and the POST of
/// the text a terminal would ask are 409s naming it, `spec.lock` byte for byte.
/// The refusal is the flag's: the same text staged with no flag lands, the lock
/// still as it was. M: dropping the refusal.
#[test]
fn b6_a_staged_editorial_is_the_terminals_and_the_lock_stays() {
    let setup = tally("confirm-editorial");
    let id = reworded(
        &setup,
        "PIECE-COUNTER",
        "grows one step at a time",
        "grows a single step at a time",
    );
    let lock = bytes(&setup.root, LOCK);
    let desk = Desk::new(setup, TALLY);
    desk.staged(
        "ann",
        &id,
        &json!({"decision": "approve", "editorial": true}),
    );
    let question = approve_asks(desk.setup(), &id, git_as(desk.setup(), "ann"));
    assert!(question.contains("editorial: "), "{question}");
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    assert_terminal(&desk.read("ann", &id), "the read");
    let stored = desk.hub.proposal(&id);
    let reply = desk.answer("ann", &id, &question, &stored["updated_at"]);
    assert_terminal(&reply, "the POST");
    assert_eq!(desk.left(&ids), before, "nothing changed");
    assert_eq!(bytes(desk.root(), LOCK), lock, "`spec.lock` byte for byte");

    desk.staged("ann", &id, &approved_choice());
    let document = desk.confirmed("ann", &id);
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(
        bytes(desk.root(), LOCK),
        lock,
        "a meaning approve writes no lock"
    );
}

/// B6: a drift item stays a terminal's: `#apply`, only a terminal `[y/N]` writes
/// `spec.lock`. The read and the POST of the very question a terminal is asked
/// are 409s naming it, the lock byte for byte. M: dropping the refusal.
#[test]
fn b6_a_drift_item_is_the_terminals_and_the_lock_stays() {
    let setup = tally("confirm-drift");
    replace(
        &setup.root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    committed(&setup, "the cap");
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.root,
        &["propose", "drift", "LAW-BOUND", "--json"],
        b"",
    );
    run.code(0);
    let id = run.json()["created"][0]["id"]
        .as_str()
        .expect("the item's ID")
        .to_owned();
    let lock = bytes(&setup.root, LOCK);
    let desk = Desk::new(setup, TALLY);
    let question = approve_asks(desk.setup(), &id, git_as(desk.setup(), "ann"));
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    assert_terminal(&desk.read("ann", &id), "the read");
    let stored = desk.hub.proposal(&id);
    let reply = desk.answer("ann", &id, &question, &stored["updated_at"]);
    assert_terminal(&reply, "the POST");
    assert_says(&reply, 409, "is a drift item", "the row's own refusal");
    assert_eq!(desk.left(&ids), before, "nothing changed");
    assert_eq!(bytes(desk.root(), LOCK), lock, "`spec.lock` byte for byte");
}

/// B6: a proposal bound to a task stays a terminal's, staged or not: a stage
/// leaves its task alone, and only a terminal completes that flow. The read and
/// the POST of the very question are 409s naming it, nothing changed. The same
/// text with no task lands on `b1_each_login_decides_and_commits_under_its_own_name`.
/// M: dropping the refusal.
#[test]
fn b6_a_task_bound_proposal_is_the_terminals() {
    let setup = press("confirm-task");
    let root = setup.root.to_str().expect("UTF-8 root");
    let run = spec(
        &setup.home,
        &setup.cwd,
        &["--root", root, "task", "new", "--nodes", "SHT-BACK"],
    );
    run.code(0);
    assert_eq!(run.stdout, "created T-0001\n", "{}", run.show());
    let base = spec_json(&setup.home, &setup.cwd, &setup.root, &["show", "SHT-BACK"]).json();
    let base = base["nodes"][0]["span_hash"].as_str().expect("span_hash");
    let text = shown_text(&setup, "SHT-BACK").replacen("the tide table", "the moon table", 1);
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.cwd,
        &[
            "--root",
            root,
            "propose",
            "update",
            "SHT-BACK",
            "--base",
            base,
            "--text-file",
            "-",
            "--rationale",
            "A test edit.",
            "--task",
            "T-0001",
        ],
        text.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());
    let question = approve_asks(desk.setup(), &id, git_as(desk.setup(), "ann"));
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    assert_terminal(&desk.read("ann", &id), "the read");
    let stored = desk.hub.proposal(&id);
    let reply = desk.answer("ann", &id, &question, &stored["updated_at"]);
    assert_terminal(&reply, "the POST");
    assert_eq!(desk.left(&ids), before, "nothing changed");
}

/// B6: an `approved` row, an apply stopped at step 10 (a `post-commit` hook undid
/// its commit), is a terminal's to complete: the read and the POST of the
/// question the terminal was asked are 409s naming it, the row and the files as
/// the stopped run left them. M: dropping the refusal.
#[test]
fn b6_an_approved_row_is_the_terminals() {
    let setup = press("confirm-approved");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    hook(desk.root(), "post-commit", "git reset -q --soft HEAD~1");
    let (env, globals) = library(desk.setup());
    let mut questions = Vec::new();
    let outcome = approve(
        &env,
        &globals,
        &ApproveRequest {
            id: id.clone(),
            note: None,
            now: utc_now(),
            git: terminal_git(desk.setup()),
        },
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    );
    fs::remove_file(desk.root().join(".git/hooks/post-commit")).expect("the hook");
    let stored = desk.hub.proposal(&id);
    assert_eq!(stored["status"], json!("approved"), "{outcome:?}");
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    assert_terminal(&desk.read("ann", &id), "the read");
    let reply = desk.answer("ann", &id, &questions[0], &stored["updated_at"]);
    assert_terminal(&reply, "the POST");
    assert_eq!(desk.left(&ids), before, "nothing changed");
}

// ------------------------------------------------------------------------ B8

/// B8: on a local daemon `GET` and `POST .../proposals/PR-0001/confirm` are the
/// unknown-route 404, its line with its own path, on every method, `Remote-User`
/// sent or not, `[team]` in the file or none, over a real proposal with a stage
/// that a mounted route would confirm. The local fence and router are as they
/// were: nothing stored. M: mounting it unconditionally.
#[test]
fn b8_on_a_local_daemon_the_route_is_the_unknown_route_404() {
    for table in ["", PRESS_TEAM] {
        let setup = Setup::new("confirm-local", "card-press");
        append_config(&setup.root, table);
        committed(&setup, "the config");
        let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
        assert_eq!(id, "PR-0001", "the criterion's ID");
        let server = Server::serve(&setup.home, &setup.cwd, &[&setup.root]);
        let review =
            |id: &str| spec_json(&setup.home, &setup.cwd, &setup.root, &["review", id]).json();
        let stage = json!({"decision": "approve", "updated_at": review(&id)["updated_at"]});
        let staged = server.stage(&format!("{P}/proposals/{id}/decision"), &stage.to_string());
        assert_eq!(staged.status, 200, "{}", staged.text());
        let before = review(&id);
        let head = setup.scratch.git().run(&setup.root, &["rev-parse", "HEAD"]);

        let path = format!("{P}/proposals/{id}/confirm");
        let body = json!({"question": "any", "updated_at": before["updated_at"]}).to_string();
        let page = [
            ("Sec-Fetch-Site", "same-origin"),
            ("Content-Type", "application/json"),
        ];
        let named = [
            ("Sec-Fetch-Site", "same-origin"),
            ("Content-Type", "application/json"),
            ("Remote-User", "mira"),
        ];
        let line = |from: &Reply| from.error_message().replace("/no/such/route", &path);
        let get = server.get("/no/such/route");
        assert_eq!(get.status, 404, "{}", get.text());
        let post = server.send("POST", "/no/such/route", &page, body.as_bytes());
        assert_eq!(post.status, 404, "{}", post.text());

        for (context, reply) in [
            ("GET", server.get(&path)),
            (
                "GET, a login",
                server.request("GET", &path, &[("Remote-User", "mira")]),
            ),
        ] {
            assert_eq!(reply.status, 404, "{context}: {}", reply.text());
            assert_eq!(reply.error_message(), line(&get), "{context}");
        }
        for (context, headers) in [("POST", &page[..]), ("POST, a login", &named[..])] {
            let reply = server.send("POST", &path, headers, body.as_bytes());
            assert_eq!(reply.status, 404, "{context}: {}", reply.text());
            assert_eq!(reply.error_message(), line(&post), "{context}");
        }
        for method in ["PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            let reply = server.send(method, &path, &page, b"");
            assert_eq!(reply.status, 404, "{method}");
        }
        assert_eq!(review(&id), before, "nothing confirmed");
        assert_eq!(
            setup.scratch.git().run(&setup.root, &["rev-parse", "HEAD"]),
            head,
            "no commit"
        );
    }
}

// ------------------------------------------------------------------------ B9

/// A tool's name that would be an agent's path to a decision.
fn decides(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["approve", "reject", "accept", "clear", "confirm"]
        .iter()
        .any(|word| name.contains(word))
}

/// B9: no MCP tool approves, rejects, accepts, clears or confirms: `tools/list`
/// over a real server, the read and the queue's tools there. M: a confirm tool.
#[test]
fn b9_tools_list_names_no_tool_that_decides() {
    for probe in ["confirm_staged", "approve_proposal", "Reject", "clear_card"] {
        assert!(decides(probe), "{probe}");
    }
    let scratch = Scratch::new("confirm-mcp");
    let root = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let mut mcp = Mcp::start(&home, &root);
    let reply = mcp.request(1, "tools/list", json!({}));
    mcp.finish();
    let names: Vec<&str> = reply["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("no tools: {reply}"))
        .iter()
        .map(|tool| tool["name"].as_str().expect("a tool's name"))
        .collect();
    assert!(names.contains(&"get_proposal"), "{names:?}");
    assert!(names.contains(&"propose_change"), "{names:?}");
    let found: Vec<&&str> = names.iter().filter(|name| decides(name)).collect();
    assert!(found.is_empty(), "a tool that decides: {found:?}");
}

/// The dependency names of a `Cargo.toml`, every `[*dependencies*]` table.
fn dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line.contains("dependencies");
            continue;
        }
        if inside && !line.is_empty() && !line.starts_with('#') {
            let name = line.split(['=', '.']).next().unwrap_or_default().trim();
            names.push(name.to_owned());
        }
    }
    names
}

/// B9: `crates/specengine-mcp/Cargo.toml` names no HTTP crate, so no tool can
/// reach a hub's route by a client of its own. M: a confirm tool calling the
/// daemon.
#[test]
fn b9_the_mcp_crate_names_no_http_crate() {
    let probe = "[dependencies]\n# axum\nserde.workspace = true\nreqwest = \"0.12\"\n\
                 [features]\naxum = []\n";
    assert_eq!(dependencies(probe), ["serde", "reqwest"]);
    let manifest = read_text(&repository_root(), "crates/specengine-mcp/Cargo.toml");
    let names = dependencies(&manifest);
    assert!(
        names.iter().any(|name| name == "specengine-cli"),
        "{names:?}"
    );
    for name in names {
        assert!(
            ![
                "specengine-http",
                "axum",
                "hyper",
                "hyper-util",
                "http",
                "http-body",
                "reqwest",
                "ureq",
                "isahc",
                "curl",
                "attohttpc",
                "surf",
            ]
            .contains(&name.as_str()),
            "the MCP crate names {name}"
        );
    }
}

/// A hand's change of meaning in a file, committed: a card on it turns suspect.
fn hand_edit(desk: &Desk, path: &str, from: &str, to: &str) {
    replace(desk.root(), path, from, to);
    desk.git(&["commit", "-qam", "by hand"]);
}

/// B9: the record's words for no person are 403 in the fence, on all four confirm
/// routes and on the read, in any ASCII case, with requests that land for a real
/// login, which they do last. M: a route reachable as `agent`.
#[test]
fn b9_remote_user_agent_is_a_403_on_all_four() {
    let setup = press("confirm-agent");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());
    let hub = &desk.hub;
    let ready = ready_card(hub, "Back page", "SHT-BACK");
    let suspect = card_on(hub, "Lead story", "SHT-FRONT");
    card_on(hub, "Long read", "SHT-INNER");
    hand_edit(
        &desk,
        FRONT_MD,
        "the lead story.",
        "the lead story and the weather.",
    );
    hand_edit(
        &desk,
        INNER_MD,
        "carries the long reads",
        "carries the short reads",
    );
    let read = desk.read_ok("ann", &id);
    let routes: [(String, Value, &str); 4] = [
        (
            format!("cards/{ready}/accept"),
            json!({"briefed": true, "revision": revision(hub, &ready)}),
            "tomas",
        ),
        (
            format!("cards/{suspect}/clear"),
            json!({"outcome": "no_effect", "reason": "A reprint.",
                "revision": revision(hub, &suspect)}),
            "tomas",
        ),
        (
            "trace/clear".to_owned(),
            json!({"group": "SHT-INNER", "reason": "A reprint.", "count": 1}),
            "ann",
        ),
        (
            format!("proposals/{id}/confirm"),
            json!({"question": read["question"], "updated_at": read["updated_at"]}),
            "ann",
        ),
    ];
    let ids = [id.as_str()];
    let before = hub.stored(&ids);
    for login in ["agent", "AGENT", "Agent", "local", "Local", "LOCAL"] {
        let line = format!("refused: Remote-User: `{login}` is the record's word for no person");
        for (route, body, _) in &routes {
            let reply = hub.post(login, route, body);
            assert_refused(&reply, &line, &format!("{login} on {route}"));
        }
        assert_refused(&desk.read(login, &id), &line, &format!("{login} reads"));
    }
    assert_eq!(hub.stored(&ids), before, "nothing stored");
    for (route, body, login) in &routes {
        let reply = hub.post(login, route, body);
        assert_eq!(reply.status, 200, "{login} on {route}: {}", reply.text());
    }
}

// ----------------------------------------------------------------------- B10

/// B10: three round answers on a hub stage three and apply none: the repository
/// byte for byte, three `proposal.staged` and no other event, each row `open`.
/// No route takes a set: a body naming others is a 400, a path over a set off the
/// list or no proposal. Each then takes its own POST: one commit each, the others
/// still open, staged. M: `round_answers` confirming.
#[test]
fn b10_three_round_answers_stage_three_apply_none_and_each_takes_its_own_post() {
    let setup = keep("confirm-round");
    let items = [
        asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?"),
        asked(&setup, &["MEC-STAMINA"], "Does stamina drain at rest?"),
        asked(&setup, &["RULE-SPRINT-COST"], "Is the cost a flat rate?"),
    ];
    let desk = Desk::new(setup, KEEP);
    let hub = &desk.hub;
    let answers: Vec<Value> = items
        .iter()
        .map(|id| {
            json!({"id": id, "stage": {"decision": "approve",
                "updated_at": hub.proposal(id)["updated_at"]}})
        })
        .collect();
    let repository = hub.repository();
    let seen = hub.events().len();
    let reply = through(
        &hub.server,
        "POST",
        &format!("{KEEP}/round/answers"),
        &Proxied::write("lena"),
        json!({"project": "lantern-keep", "answers": answers})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    for entry in reply.json()["answers"].as_array().expect("answers") {
        assert_eq!(entry["outcome"], json!("staged"), "{entry}");
    }
    assert_eq!(hub.repository(), repository, "nothing applied");
    assert_eq!(desk.added(seen), ["proposal.staged"; 3], "three stages");
    for id in &items {
        let stored = hub.proposal(id);
        assert_eq!(stored["status"], json!("open"), "{stored}");
        assert!(stored["staged"].is_object(), "{stored}");
    }

    let first = desk.read_ok("lena", &items[0]);
    let ids: Vec<&str> = items.iter().map(String::as_str).collect();
    let before = desk.left(&ids);
    let set = json!({"question": first["question"], "updated_at": first["updated_at"],
        "ids": items});
    let reply = desk.post_bytes("lena", &items[0], set.to_string().as_bytes());
    assert_eq!(reply.status, 400, "{}", reply.text());
    for route in ["proposals/confirm", "round/confirm", "confirm"] {
        let path = format!("{KEEP}/{route}");
        let page = Proxied::write("lena");
        let reply = through(
            &hub.server,
            "POST",
            &path,
            &page,
            set.to_string().as_bytes(),
        );
        assert_refused(&reply, OFF_LIST_LINE, &path);
    }
    // The set in the path, the first item's own closed body with it: a route
    // walking the set would confirm that first item.
    let one = json!({"question": first["question"], "updated_at": first["updated_at"]});
    let reply = desk.post_bytes("lena", &items.join(","), one.to_string().as_bytes());
    assert_ne!(reply.status, 200, "a set in the path: {}", reply.text());
    assert_eq!(desk.left(&ids), before, "no route took the set");

    for (at, id) in items.iter().enumerate() {
        let base = desk.head();
        desk.confirmed("lena", id);
        let head = desk.head();
        assert_eq!(desk.parents(&head), [base], "{id}: one commit");
        assert_eq!(desk.trailers(&head, "Proposal"), [id.as_str()]);
        for later in &items[at + 1..] {
            let stored = hub.proposal(later);
            assert_eq!(stored["status"], json!("open"), "{later}: {stored}");
            assert!(stored["staged"].is_object(), "{later}: {stored}");
        }
    }
}

/// The reads of `ids` by the owner on a place that fails: each `status`, no
/// event, no row changed.
#[track_caller]
fn assert_quiet_refusals(desk: &Desk, ids: &[&str], status: u16, context: &str) {
    let events = desk.hub.events();
    let rows: Vec<Value> = ids.iter().map(|id| desk.hub.proposal(id)).collect();
    for id in ids {
        let reply = desk.read("ann", id);
        assert_eq!(reply.status, status, "{context}: {id}: {}", reply.text());
    }
    assert_eq!(desk.hub.events(), events, "{context}: no event");
    let now: Vec<Value> = ids.iter().map(|id| desk.hub.proposal(id)).collect();
    assert_eq!(now, rows, "{context}: no row changed");
}

/// B10: the read rehearses, and a rehearsal logs nothing. On a place that fails
/// for an `update` (`apply.rs`), a record (`decide.rs`) and a new file
/// (`create.rs`), on another branch and with each one's own file in the way, the
/// read refuses with no `proposal.apply_failed`, where a terminal's `spec approve`
/// would log one. Put right, each reads again, with no event added. M: rehearsing
/// through `failed()`.
#[test]
fn b10_the_confirm_read_adds_no_event_on_a_failed_place() {
    let setup = keep("confirm-failed-place");
    let update = reworded(&setup, "EDGE-STAM-ZERO", "immediately", "after 0.2 s");
    let question = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let created = new_record(&setup, "R-13", "docs/records/R/R-13.md");
    let desk = Desk::new(setup, KEEP);
    for id in [&update, &question, &created] {
        desk.staged("ann", id, &approved_choice());
    }
    let ids = [update.as_str(), question.as_str(), created.as_str()];
    let events = desk.hub.events();

    desk.git(&["checkout", "-q", "-b", "elsewhere"]);
    assert_quiet_refusals(&desk, &ids, 503, "another branch");
    desk.git(&["checkout", "-q", "main"]);

    write(desk.root(), STAMINA_MD, "A hand was here.\n");
    assert_quiet_refusals(&desk, &ids[..1], 409, "a dirty target");
    desk.git(&["checkout", "-q", "--", STAMINA_MD]);

    write(desk.root(), "templates/decision.md", "A hand was here.\n");
    assert_quiet_refusals(&desk, &ids[1..2], 503, "a dirty template");
    desk.git(&["checkout", "-q", "--", "templates/decision.md"]);

    write(desk.root(), "docs/records/R/R-13.md", "In the way.\n");
    assert_quiet_refusals(&desk, &ids[2..], 409, "the path taken");
    fs::remove_file(desk.root().join("docs/records/R/R-13.md")).expect("the file");

    for id in ids {
        desk.read_ok("ann", id);
    }
    assert_eq!(desk.hub.events(), events, "no read logged anything");
}

/// "Data": a POST is a real attempt, where a read rehearses: on a place that
/// fails the read is a 503 with no event, and the POST a 503 with its one
/// `proposal.apply_failed`, the row and its stage kept, as a terminal's run would
/// leave them. Put right, a new read lands. M: a POST rehearsing; rehearsing
/// through `failed()`.
#[test]
fn a_post_on_a_failed_place_is_a_503_and_logs_its_one_failure() {
    let (desk, id) = back_update_staged("confirm-post-place");
    let read = desk.read_ok("lena", &id);
    let row = desk.hub.proposal(&id);
    desk.git(&["checkout", "-q", "-b", "elsewhere"]);
    let seen = desk.hub.events().len();

    let reply = desk.read("lena", &id);
    assert_eq!(reply.status, 503, "the read: {}", reply.text());
    assert_eq!(
        desk.added(seen),
        Vec::<String>::new(),
        "a read logs nothing"
    );

    let reply = desk.confirm("lena", &id, &read);
    assert_eq!(reply.status, 503, "the POST: {}", reply.text());
    assert_eq!(desk.added(seen), ["proposal.apply_failed"], "one attempt");
    assert_eq!(desk.hub.proposal(&id), row, "the row and its stage kept");

    desk.git(&["checkout", "-q", "main"]);
    let document = desk.confirmed("lena", &id);
    assert_eq!(document["status"], json!("applied"), "{document}");
}

// ----------------------------------------------------------------------- B11

/// `Author::human()`'s provenance: a `Proposed-by` with no login.
const HUMAN: &str = "human role=unknown model=unknown run=unknown";

/// B11: a departing accept by `tomas` on a hub carries his login in the item's
/// author, last, and in the commit's `Proposed-by`; the review, the page's read
/// and the MCP mirror all hold it. `Decided-by` stays whoever confirmed. M:
/// `login` dropped from `provenance()` or from `ProposalAuthor`.
#[test]
fn b11_a_departing_accept_on_a_hub_reads_the_login_in_proposed_by() {
    let desk = Desk::new(press_with_records("confirm-proposed-by"), P);
    let hub = &desk.hub;
    let card = ready_card(hub, "Lead story", "SHT-FRONT");
    let item = departed(hub, &card, "SHT-FRONT");
    let want = json!({"type": "human", "role": null, "model": null, "run": null,
        "login": "tomas"});
    let served = hub
        .get("mira", &format!("{P}/proposals/{item}"))
        .status(200)
        .json();
    assert_eq!(served["author"], want, "{served}");
    let setup = desk.setup();
    let review = spec_json(&setup.home, &setup.cwd, &setup.root, &["review", &item]).json();
    assert_eq!(review["author"], want, "{review}");
    let mut mcp = Mcp::start(&setup.home, &setup.root);
    let reply = mcp.request(
        1,
        "tools/call",
        json!({"name": "get_proposal", "arguments": {"proposal_id": item}}),
    );
    mcp.finish();
    assert_eq!(
        reply["result"]["structuredContent"]["author"]["login"],
        json!("tomas"),
        "{reply}"
    );

    desk.staged("tomas", &item, &json!({"decision": "approve", "option": 1}));
    let document = desk.confirmed("mira", &item);
    assert_eq!(document["status"], json!("applied"), "{document}");
    let commit = desk.head();
    assert_eq!(
        desk.trailers(&commit, "Proposed-by"),
        [format!("{HUMAN} login=tomas")]
    );
    assert_eq!(desk.trailers(&commit, "Decided-by"), [MIRA]);
}

/// B11, a hub's other authors: an item raised at a terminal beside a running hub
/// holds no login: only a departing accept sets it, so its trailer stays as it
/// was. M: the login set by `confirm_staged`.
#[test]
fn b11_only_a_departing_accept_sets_the_login() {
    let setup = press("confirm-no-login");
    let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &id, &approved_choice());
    desk.confirmed("lena", &id);
    let commit = desk.head();
    assert_eq!(desk.trailers(&commit, "Proposed-by"), [HUMAN]);
    let stored = desk.hub.get("mira", &format!("{P}/proposals/{id}"));
    assert_eq!(
        stored.status(200).json()["author"],
        json!({"type": "human", "role": null, "model": null, "run": null}),
    );
}

/// B11: a local daemon's departing accept keeps its author at four keys and its
/// commit's `Proposed-by` byte for byte as it was, a typed `as` no login. M:
/// `login` written on a local row.
#[test]
fn b11_a_local_departing_accept_keeps_its_trailer_byte_for_byte() {
    let setup = press_with_records("confirm-local-trailer");
    let server = Server::serve(&setup.home, &setup.cwd, &[&setup.root]);
    let post = |path: &str, body: &Value| {
        let reply = server.stage(&format!("{P}/{path}"), &body.to_string());
        assert_eq!(reply.status, 200, "{path} {body}: {}", reply.text());
        reply.json()
    };
    post(
        "cards",
        &json!({"title": "Lead story", "discipline": "plates", "target": ["SHT-FRONT"],
            "assignee": "ana", "as": "desk"}),
    );
    post(
        "cards/C-0001/assign",
        &json!({"reviewer": "tomas", "as": "desk"}),
    );
    post("cards/C-0001/move", &json!({"to": "checked", "as": "desk"}));
    post(
        "cards/C-0001/accept",
        &json!({"as": "tomas", "departs": ["SHT-FRONT"],
            "summary": "The result left the brief.",
            "observed": "The lead runs below the fold.",
            "documented": "The lead runs above the fold."}),
    );
    let id = "PR-0001";
    let review = spec_json(&setup.home, &setup.cwd, &setup.root, &["review", id]).json();
    assert_eq!(
        review["author"],
        json!({"type": "human", "role": null, "model": null, "run": null}),
        "{review}"
    );

    let (env, globals) = library(&setup);
    let outcome = approve_with(
        &env,
        &globals,
        &ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: utc_now(),
            git: terminal_git(&setup),
        },
        &ApproveFlags {
            option: Some(0),
            ..ApproveFlags::default()
        },
        &mut |_: &str| true,
    )
    .expect("a terminal's approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let git = setup.scratch.git();
    let message = git.run(&setup.root, &["log", "-1", "--format=%B"]);
    let lines: Vec<&str> = message
        .lines()
        .filter(|line| line.starts_with("Proposed-by: "))
        .collect();
    assert_eq!(lines, [format!("Proposed-by: {HUMAN}")], "{message}");
}

// ----------------------------------------------------- the confirm class's fourth

/// "Data": the route is the confirm class's fourth, `docs/canon/hub-team.md`
/// "Classes": in the fence no query and a required `Origin`, every raw form and
/// any other verb off the list, a same-origin page and JSON as ever. Each with a
/// login and a body that land, which they do last. M: `class` missing the route;
/// a listed route matched after decoding.
#[test]
fn the_route_is_the_fourth_of_the_confirm_class() {
    let (desk, id) = back_update_staged("confirm-class");
    let hub = &desk.hub;
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let read = desk.read_ok("lena", &id);
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]}).to_string();
    let body = body.as_bytes();
    let path = desk.route(&id, "confirm");
    let page = Proxied::write("lena");

    for query in ["?x=/move", "?x=1", "?", "?question=y", "?&", "?/confirm"] {
        let form = format!("{path}{query}");
        let reply = through(&hub.server, "POST", &form, &page, body);
        assert_refused(&reply, NO_QUERY_LINE, &form);
    }
    let form = format!("{ORIGIN}{path}?x=1");
    let reply = through(&hub.server, "POST", &form, &page, body);
    assert_refused(&reply, NO_QUERY_LINE, &form);

    let reply = through(
        &hub.server,
        "POST",
        &path,
        &page.clone().without("Origin"),
        body,
    );
    assert_refused(&reply, NO_ORIGIN_LINE, "no Origin");
    for origin in [
        "https://hub.example.test/",
        "https://HUB.example.test",
        "http://hub.example.test",
        "https://evil.example.test",
        "null",
    ] {
        let reply = through(
            &hub.server,
            "POST",
            &path,
            &page.clone().set("Origin", origin),
            body,
        );
        assert_refused(&reply, WRONG_ORIGIN_LINE, origin);
    }
    let reply = through(
        &hub.server,
        "POST",
        &path,
        &page.clone().without("Sec-Fetch-Site"),
        body,
    );
    assert_eq!(reply.status, 403, "no page: {}", reply.text());
    let reply = through(
        &hub.server,
        "POST",
        &path,
        &page.clone().set("Content-Type", "text/plain"),
        body,
    );
    assert_eq!(reply.status, 415, "{}", reply.text());

    for form in [
        format!("{path}/"),
        format!("{P}/proposals/{id}//confirm"),
        format!("{P}/proposals/{id}/%63onfirm"),
        format!("{P}/proposals/{id}/CONFIRM"),
        format!("{P}/proposals/{id}%2Fconfirm"),
        format!("{P}/proposals/{id}/confirm;x"),
        format!("{P}/proposals/{id}/confirm%00"),
        format!("{P}/proposals/{id}/x/../confirm"),
        format!("{P}/proposals//confirm"),
        format!("/api/projects//proposals/{id}/confirm"),
    ] {
        let reply = through(&hub.server, "POST", &form, &page, body);
        assert_refused(&reply, OFF_LIST_LINE, &form);
    }
    for method in ["PUT", "PATCH", "DELETE"] {
        let reply = through(&hub.server, method, &path, &page, body);
        assert_refused(&reply, OFF_LIST_LINE, method);
    }
    assert_eq!(desk.left(&ids), before, "no form reached a handler");

    desk.confirm("lena", &id, &read).status(200);
}

/// "Data": the body is `{question, updated_at}`, closed: a choice, a name, any
/// other key is a 400, and so is a key missing or no JSON at all: a confirm takes
/// a stage, never a choice, and `as` has no place in it. Nothing stored. An ID
/// that is none is a 404. M: `deny_unknown_fields` left off the body.
#[test]
fn the_body_is_closed_so_a_choice_is_a_400() {
    let (desk, id) = back_update_staged("confirm-body");
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let read = desk.read_ok("lena", &id);
    let good = json!({"question": read["question"], "updated_at": read["updated_at"]});
    for (key, value) in [
        ("choice", json!("reject")),
        ("decision", json!("approve")),
        ("option", json!(1)),
        ("answer", json!("Yes.")),
        ("canon", json!("SHT-FRONT")),
        ("editorial", json!(true)),
        ("note", json!("A note.")),
        ("reason", json!("A reason.")),
        ("as", json!("lena")),
        ("id", json!(id)),
        ("ids", json!([id])),
    ] {
        let mut body = good.clone();
        body[key] = value;
        let reply = desk.post_bytes("lena", &id, body.to_string().as_bytes());
        assert_eq!(reply.status, 400, "{key}: {}", reply.text());
    }
    for body in [
        json!({"question": read["question"]}).to_string(),
        json!({"updated_at": read["updated_at"]}).to_string(),
        json!({"question": 1, "updated_at": read["updated_at"]}).to_string(),
        "{".to_owned(),
        String::new(),
    ] {
        let reply = desk.post_bytes("lena", &id, body.as_bytes());
        assert_eq!(reply.status, 400, "{body}: {}", reply.text());
    }
    assert_eq!(desk.left(&ids), before, "nothing stored");

    for unknown in ["PR-9999", "approve-all"] {
        let reply = desk.read("ann", unknown);
        assert_eq!(reply.status, 404, "{unknown}: {}", reply.text());
        let reply = desk.post_bytes("ann", unknown, good.to_string().as_bytes());
        assert_eq!(reply.status, 404, "{unknown}: {}", reply.text());
    }
    assert_eq!(desk.left(&ids), before, "nothing stored");
}

/// "One click per item": the same read posted again changes nothing: the item is
/// no longer `open`, a 409, one commit. Status, stage and the run each refuse it,
/// so no single mutation turns this red; it pins the three together.
#[test]
fn a_second_click_changes_nothing() {
    let (desk, id) = back_update_staged("confirm-twice");
    let read = desk.read_ok("lena", &id);
    desk.confirm("lena", &id, &read).status(200);
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let reply = desk.confirm("lena", &id, &read);
    assert_eq!(reply.status, 409, "{}", reply.text());
    assert_eq!(desk.left(&ids), before, "one commit, nothing more");
}

/// Four logins' browsers on one read: one commit, three 409s. The project's turn
/// serialises them, and the queue's step-7 compare-and-set holds one winner even
/// without it, so no single mutation turns this red: a confirm run outside the
/// turn survives here, and the test pins the two together.
#[test]
fn four_racing_clicks_make_one_commit() {
    let (desk, id) = back_update_staged("confirm-race");
    let base = desk.head();
    let read = desk.read_ok("lena", &id);
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]}).to_string();
    let path = desk.route(&id, "confirm");
    let bytes = Proxied::write("lena").bytes("POST", &path, body.as_bytes());
    let port = desk.hub.server.port;
    let start = Barrier::new(4);
    let replies: Vec<Reply> = thread::scope(|scope| {
        let racers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    http_exchange(port, &bytes, HTTP_TIMEOUT)
                })
            })
            .collect();
        racers
            .into_iter()
            .map(|racer| racer.join().expect("a racer"))
            .collect()
    });
    let mut statuses: Vec<u16> = replies.iter().map(|reply| reply.status).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, [200, 409, 409, 409], "{replies:?}");
    let head = desk.head();
    assert_eq!(desk.parents(&head), [base], "one commit");
}

/// `docs/canon/hub-team.md` "Classes": `[team]` is read before the body and again
/// once it is in, so a login taken off `[team]`, made a `member` or moved out of
/// the zone while its body was on the way confirms nothing, each the refusal of
/// what `[team]` says now. Left as it was, the same two steps land. The `100` is
/// the signal, no clock. M: dropping the second judgement.
#[test]
fn a_login_changed_while_its_body_is_on_the_way_confirms_nothing() {
    let (desk, id) = back_update_staged("confirm-slow-body");
    let hub = &desk.hub;
    let root = desk.root();
    let good = read_text(root, "specengine.toml");
    assert_eq!(good.matches(LENA_ENTRY).count(), 1, "lena's entry");
    let read = desk.read_ok("lena", &id);
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]}).to_string();
    let path = desk.route(&id, "confirm");
    let page = Proxied::write("lena");
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    let member = "[team.lena]\nrole = \"member\"\nemail = \"lena@example.org\"\n";
    let front = "[team.lena]\nrole = \"lead\"\nzones = [\"front\"]\nemail = \"lena@example.org\"\n";
    for (entry, line) in [
        ("", outside_line("lena")),
        (member, member_line("lena")),
        (front, zone_line("lena", "front", BACK_MD)),
    ] {
        let (reply, continued) =
            through_in_two_steps(&hub.server, "POST", &path, &page, body.as_bytes(), || {
                write(root, "specengine.toml", good.replace(LENA_ENTRY, entry))
            });
        assert!(continued, "past the head: {}", reply.text());
        write(root, "specengine.toml", &good);
        assert_says(&reply, 403, &line, entry);
        assert_eq!(desk.left(&ids), before, "{entry}: nothing changed");
    }

    let (reply, continued) =
        through_in_two_steps(&hub.server, "POST", &path, &page, body.as_bytes(), || {});
    assert!(continued, "past the head");
    assert_eq!(reply.status, 200, "{}", reply.text());
}

/// `docs/canon/hub-team.md` "Answers": a lead and an owner read `confirm` among
/// their acts, the page's way to know; a member and a login outside do not. M:
/// `confirm` offered to a `member`.
#[test]
fn the_team_row_allows_a_confirm_to_a_lead_and_an_owner_alone() {
    let desk = Desk::new(press("confirm-allowed"), P);
    let allowed = |login: &str| {
        desk.hub.get(login, &format!("{P}/team")).status(200).json()["allowed"]
            .as_array()
            .expect("allowed")
            .contains(&json!("confirm"))
    };
    assert!(allowed("ann"), "an owner");
    assert!(allowed("mira"), "a lead");
    assert!(!allowed("tomas"), "a member");
    assert!(!allowed("eve"), "outside `[team]`");
}

/// The read's question goes back in the POST, whatever its length: eight lines of
/// evidence make a record's question longer than a stage's 16 KiB body, and
/// nothing a person can read may be beyond confirming. M: the stage's body cap on
/// this route.
#[test]
fn a_long_records_question_still_lands() {
    let setup = keep("confirm-long");
    let line = "regenerates while sprinting and ".repeat(31);
    let evidence = (0..8)
        .map(|n| Evidence {
            file: format!("src/stamina_{n}.rs"),
            qpath: Some("regen_system".to_owned()),
            lines: Some("10-20".to_owned()),
            observed: line.trim_end().to_owned(),
            documented: line.trim_end().to_owned(),
        })
        .collect();
    let gap = reported_with(&setup, &["MEC-SPRINT"], "The drain disagrees.", evidence);
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &gap, &json!({"decision": "approve", "option": 0}));
    let read = desk.read_ok("ann", &gap);
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]}).to_string();
    assert!(body.len() > 16_384, "{} bytes", body.len());
    let document = desk.confirm("ann", &gap, &read).status(200).json();
    assert_eq!(document["status"], json!("applied"), "{document}");
}

/// A deciding kind's question names its decider, in the record it shows, so it is
/// one login's: the owner replaying a lead's read is a 409, nothing written, and
/// the lead's own click lands under the lead's name. M: an always-yes closure; the
/// question read under no login.
#[test]
fn a_question_naming_its_decider_is_no_other_logins() {
    let setup = keep("confirm-replay");
    let id = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let desk = Desk::new(setup, KEEP);
    desk.staged("mira", &id, &approved_choice());
    let read = desk.read_ok("mira", &id);
    let text = read["question"].as_str().expect("a question");
    assert!(text.contains(MIRA), "its decider: {text}");
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    let reply = desk.confirm("ann", &id, &read);
    assert_says(&reply, 409, MOVED_LINE, "the owner replays a lead's read");
    assert_eq!(desk.left(&ids), before, "nothing written");

    desk.confirm("mira", &id, &read).status(200);
    desk.assert_decided(&id, &desk.head(), MIRA);
}

/// The refusals' order on one repository: status, task-bound, nothing staged,
/// cover, `updated_at`, question. Each row carries the next fault too, and the
/// first answers: `mira`, outside the back sheet, reads a decided row's line, a
/// task's and a bare row's, never the cover's; past the cover `lena` meets
/// `updated_at` before the question. M: the cover judged first; the question
/// compared before `updated_at`.
#[test]
fn the_refusals_come_in_their_order() {
    let setup = press("confirm-order");
    let closed = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
    let bare = reworded(&setup, "SHT-BACK", "the tide table", "the sun table");
    let staged = reworded(&setup, "SHT-BACK", "the tide table", "the star table");
    let root = setup.root.to_str().expect("UTF-8 root").to_owned();
    let run = spec(
        &setup.home,
        &setup.cwd,
        &["--root", &root, "task", "new", "--nodes", "SHT-INNER-FOLD"],
    );
    run.code(0);
    let shown = spec_json(&setup.home, &setup.cwd, &setup.root, &["show", "SHT-BACK"]).json();
    let base = shown["nodes"][0]["span_hash"].as_str().expect("span_hash");
    let text = shown_text(&setup, "SHT-BACK").replacen("the tide table", "the sea table", 1);
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.cwd,
        &[
            "--root",
            &root,
            "propose",
            "update",
            "SHT-BACK",
            "--base",
            base,
            "--text-file",
            "-",
            "--rationale",
            "A test edit.",
            "--task",
            "T-0001",
        ],
        text.as_bytes(),
    );
    run.code(0);
    let bound = run.stdout.lines().next().unwrap_or_default().to_owned();
    let desk = Desk::new(setup, P);
    desk.staged("tomas", &closed, &rejected_choice("Not this week."));
    desk.confirmed("ann", &closed);
    desk.staged("tomas", &staged, &approved_choice());
    let read = desk.read_ok("lena", &staged);
    let ids = [
        closed.as_str(),
        bare.as_str(),
        staged.as_str(),
        bound.as_str(),
    ];
    let before = desk.left(&ids);
    let now = &read["updated_at"];
    let old = json!("2026-01-01T00:00:00Z");

    assert_says(&desk.read("mira", &closed), 409, "is rejected", "status");
    assert_says(
        &desk.read("mira", &bound),
        409,
        "is bound to T-0001",
        "task",
    );
    assert_says(
        &desk.read("mira", &bare),
        409,
        "has nothing staged",
        "stage",
    );
    assert_refused(
        &desk.answer("mira", &staged, "any", &old),
        &zone_line("mira", "front", BACK_MD),
        "the cover before `updated_at`",
    );
    assert_says(
        &desk.answer("lena", &staged, "any", &old),
        409,
        CHANGED_LINE,
        "`updated_at` before the question",
    );
    assert_says(
        &desk.answer("lena", &staged, "any", now),
        409,
        MOVED_LINE,
        "the question last",
    );
    assert_eq!(desk.left(&ids), before, "nothing changed");
}

/// The refusals' order past a row's kind: an editorial stage before the cover, a
/// drift item before its task and before its stage. M: the cover judged first.
#[test]
fn a_terminals_row_is_refused_before_the_cover_and_a_drift_before_its_task() {
    let setup = tally("confirm-order-lock");
    let edited = reworded(
        &setup,
        "PIECE-COUNTER",
        "grows one step at a time",
        "grows a single step at a time",
    );
    replace(
        &setup.root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    committed(&setup, "the cap");
    let root = setup.root.to_str().expect("UTF-8 root").to_owned();
    spec(
        &setup.home,
        &setup.cwd,
        &["--root", &root, "task", "new", "--nodes", "LAW-BOUND"],
    )
    .code(0);
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.root,
        &[
            "propose",
            "drift",
            "LAW-BOUND",
            "--task",
            "T-0001",
            "--json",
        ],
        b"",
    );
    run.code(0);
    let drift = run.json()["created"][0]["id"]
        .as_str()
        .expect("the item's ID")
        .to_owned();
    let desk = Desk::new(setup, TALLY);
    desk.staged(
        "ann",
        &edited,
        &json!({"decision": "approve", "editorial": true}),
    );
    let reply = desk.read("mira", &edited);
    assert_terminal(&reply, "an editorial stage, a lead outside");
    assert_says(&reply, 409, "editorial", "the stage's own refusal");
    let reply = desk.read("mira", &drift);
    assert_terminal(&reply, "a drift item, a lead outside");
    assert_says(&reply, 409, "is a drift item", "its kind, not its task");
}

/// The repository comes first: a row another repository of the slug decided is
/// that repository's, whatever its status, and the hub names where it belongs. M:
/// the status read first.
#[test]
fn another_repositorys_row_is_refused_before_its_status() {
    let setup = press("confirm-order-repository");
    let other = setup.scratch.repo("card-press", "other", "main");
    let git = setup.scratch.git();
    git.run(&other, &["config", "user.name", "Owner"]);
    git.run(&other, &["config", "user.email", "owner@example.invalid"]);
    let text = shown_text(&setup, "SHT-BACK").replacen("the tide table", "the moon table", 1);
    let id = propose_update(&setup.home, &setup.cwd, &other, "SHT-BACK", &text);
    let env = Env {
        cwd: other.clone(),
        home: Some(setup.home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(other.clone()),
        config: None,
    };
    let outcome = reject(
        &env,
        &globals,
        &RejectRequest {
            id: id.clone(),
            reason: Some("Not this week.".to_owned()),
            now: utc_now(),
            git: GitEnv::new(other.as_path(), product_env(&setup.home)),
        },
        &mut |_: &str| true,
    )
    .expect("a terminal's reject");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let desk = Desk::new(setup, P);
    let reply = desk.read("ann", &id);
    assert_says(&reply, 409, "belongs to another repository", "the read");
    let reply = desk.answer("ann", &id, "any", &json!("2026-01-01T00:00:00Z"));
    assert_says(&reply, 409, "belongs to another repository", "the POST");
}

/// The body's cap: 4 MiB, a whole record's question with each line end doubled
/// by JSON. A body of exactly that is read and judged, a 409 for its text; one
/// declared a byte over it is a 413, unread. M: the stage's body cap on this
/// route; no cap.
#[test]
fn the_body_holds_up_to_4_mib() {
    let (desk, id) = back_update_staged("confirm-cap");
    let read = desk.read_ok("lena", &id);
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let frame = json!({"question": "", "updated_at": read["updated_at"]}).to_string();
    let question = "x".repeat(BODY_MAX - frame.len());
    let body = json!({"question": question, "updated_at": read["updated_at"]}).to_string();
    assert_eq!(body.len(), BODY_MAX);
    let reply = desk.post_bytes("lena", &id, body.as_bytes());
    assert_says(&reply, 409, MOVED_LINE, "at the cap");

    let over = BODY_MAX + 1;
    let page = Proxied::write("lena").plus("Content-Length", &over.to_string());
    let bytes = page.bytes("POST", &desk.route(&id, "confirm"), b"");
    let reply = desk.hub.server.raw(&bytes);
    assert_eq!(reply.status, 413, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        format!("the body is over {BODY_MAX} bytes")
    );
    assert_eq!(desk.left(&ids), before, "nothing changed");
}

// ------------------------------------------------------------- iteration 2

/// B3, "Cover": an `amendment` is a new file, and it governs the nodes its
/// `amends` links name, so each one's holder is judged too, resolved now.
/// `ren`, who leads the feature documents alone, may read and confirm no new
/// feature document amending a stamina rule; `wil`, leading both, reads it; the
/// owner lands it, one new file. M: judging without the amends holders.
#[test]
fn b3_an_amendment_is_judged_by_the_holders_of_what_it_amends() {
    let setup = keep("confirm-amends");
    let id = amendment(&setup, "docs/features/regen-delay.md", "RULE-STAM-REGEN");
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &id, &approved_choice());
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let line = zone_line("ren", "features", STAMINA_MD);
    assert_refused(&desk.read("ren", &id), &line, "the read");
    let read = desk.read_ok("ann", &id);
    assert_refused(&desk.confirm("ren", &id, &read), &line, "the POST");
    assert_eq!(desk.left(&ids), before, "a refused lead stored nothing");

    desk.read_ok("wil", &id);
    let document = desk.confirm("ann", &id, &read).status(200).json();
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(desk.paths(&desk.head()), ["docs/features/regen-delay.md"]);
}

/// B3, "Cover": an amended node resolving to nothing now is the owner's alone:
/// `wil` leads every zone the amendment touched, and its node lost its ID after
/// the stage; nothing is stored. The owner passes the cover, and apply step 4,
/// which re-reads the amended nodes, then refuses the apply (409): B3's "200 for
/// `owner`" cannot hold for a gone amended node. M: an
/// amended node that resolves to nothing skipped by the cover.
#[test]
fn b3_an_amendment_of_a_node_gone_since_is_the_owners_alone() {
    let setup = keep("confirm-amends-gone");
    let id = amendment(&setup, "docs/features/empty-tank.md", "EDGE-SPRINT-EMPTY");
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &id, &approved_choice());
    desk.read_ok("wil", &id);
    id_removed(&desk, SPRINT_MD, "### Empty tank", "EDGE-SPRINT-EMPTY");
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let reply = desk.read("wil", &id);
    assert_owner_only(&reply, "wil", "features, sprint, stamina", "the read");
    let reply = desk.answer("wil", &id, "any", &json!("2026-01-01T00:00:00Z"));
    assert_owner_only(&reply, "wil", "features, sprint, stamina", "the POST");
    assert_eq!(desk.left(&ids), before, "nothing stored");
    let reply = desk.read("ann", &id);
    assert_ne!(
        reply.status,
        403,
        "the owner passes the cover: {}",
        reply.text()
    );
}

/// B3, "Cover": a stage's `canon` REF names the section its record governs, so
/// its holder is judged too: without it a member's stage widens what a lead's
/// click governs. `mira`, leading the sprint, may not confirm a sprint question
/// staged to govern a stamina rule; `lena`, leading both, reads it; the owner
/// lands it, the record's `canon:` the REF as staged. M: judging without the
/// staged `canon` holder.
#[test]
fn b3_a_staged_canon_is_judged_by_its_holder() {
    let setup = keep("confirm-canon");
    let id = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let desk = Desk::new(setup, KEEP);
    desk.staged(
        "ann",
        &id,
        &json!({"decision": "approve", "canon": "RULE-STAM-REGEN"}),
    );
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let line = zone_line("mira", "sprint", STAMINA_MD);
    assert_refused(&desk.read("mira", &id), &line, "the read");
    let read = desk.read_ok("ann", &id);
    assert_refused(&desk.confirm("mira", &id, &read), &line, "the POST");
    assert_eq!(desk.left(&ids), before, "a refused lead stored nothing");

    desk.read_ok("lena", &id);
    let document = desk.confirm("ann", &id, &read).status(200).json();
    assert_eq!(document["status"], json!("applied"), "{document}");
    let record = read_text(desk.root(), "docs/records/DEC/DEC-100001.md");
    assert!(record.contains("canon: RULE-STAM-REGEN\n"), "{record}");
}

/// B3, "Cover": a staged `canon` resolving to nothing now is the owner's alone:
/// `lena` leads the sprint and the stamina, and the section the stage named lost
/// its ID since. The owner reads the record's dangling `canon:` among its
/// introduced findings, and lands it. M: a `canon` that resolves to nothing
/// skipped by the cover.
#[test]
fn b3_a_staged_canon_naming_nothing_is_the_owners_alone() {
    let setup = keep("confirm-canon-gone");
    let id = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let desk = Desk::new(setup, KEEP);
    desk.staged(
        "ann",
        &id,
        &json!({"decision": "approve", "canon": "EDGE-SPRINT-EMPTY"}),
    );
    desk.read_ok("lena", &id);
    id_removed(&desk, SPRINT_MD, "### Empty tank", "EDGE-SPRINT-EMPTY");
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    assert_owner_only(
        &desk.read("lena", &id),
        "lena",
        "sprint, stamina",
        "the read",
    );
    let reply = desk.answer("lena", &id, "any", &json!("2026-01-01T00:00:00Z"));
    assert_owner_only(&reply, "lena", "sprint, stamina", "the POST");
    assert_eq!(desk.left(&ids), before, "nothing stored");
    let read = desk.read_ok("ann", &id);
    let question = read["question"].as_str().expect("a question");
    assert!(
        question.contains("ref-dangling"),
        "the finding shown: {question}"
    );
    let document = desk.confirm("ann", &id, &read).status(200).json();
    assert_eq!(document["status"], json!("applied"), "{document}");
}

/// B4, "Terminal only": an `update` staged while its preview failed holds no
/// `span_hash`, so a span change between the read and the POST would leave the
/// question byte for byte. Staged while its target was dirty, then put right:
/// the read and the POST of the very question a terminal asks are 409s naming
/// the terminal, the target and the queue as they were. Staged again on a clean
/// target, it holds its hash and lands. M: accepting a `None` span hash.
#[test]
fn b4_an_update_staged_with_no_span_hash_is_the_terminals() {
    let (desk, id) = back_update_staged("confirm-no-span");
    let root = desk.root().to_path_buf();
    let clean = bytes(&root, BACK_MD);
    write(&root, BACK_MD, "A hand was here.\n");
    desk.staged("tomas", &id, &approved_choice());
    desk.git(&["checkout", "-q", "--", BACK_MD]);
    let staged = desk.hub.proposal(&id)["staged"].clone();
    assert_eq!(staged["span_hash"], Value::Null, "{staged}");
    let question = approve_asks(desk.setup(), &id, git_as(desk.setup(), "lena"));
    let ids = [id.as_str()];
    let before = desk.left(&ids);

    assert_terminal(&desk.read("lena", &id), "the read");
    let stored = desk.hub.proposal(&id);
    let reply = desk.answer("lena", &id, &question, &stored["updated_at"]);
    assert_terminal(&reply, "the POST");
    assert_eq!(desk.left(&ids), before, "nothing changed");
    assert_eq!(bytes(&root, BACK_MD), clean, "the target byte for byte");

    desk.staged("tomas", &id, &approved_choice());
    let staged = desk.hub.proposal(&id)["staged"].clone();
    assert!(staged["span_hash"].is_string(), "{staged}");
    let document = desk.confirmed("lena", &id);
    assert_eq!(document["status"], json!("applied"), "{document}");
}

/// B4: a new file and a kind that decides always stage `span_hash` null, and
/// they are not refused for it: each confirms. M: refusing every null span hash.
#[test]
fn b4_a_new_file_and_a_question_confirm_with_no_span_hash() {
    let setup = keep("confirm-null-span");
    let question = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let created = new_record(&setup, "R-13", "docs/records/R/R-13.md");
    let desk = Desk::new(setup, KEEP);
    for id in [&question, &created] {
        desk.staged("ann", id, &approved_choice());
        let staged = desk.hub.proposal(id)["staged"].clone();
        assert_eq!(staged["span_hash"], Value::Null, "{id}: {staged}");
        let document = desk.confirmed("ann", id);
        assert_eq!(document["status"], json!("applied"), "{id}: {document}");
    }
}

/// The four format characters B12 names, each with its escape.
const FORMAT_FOUR: [(char, &str); 4] = [
    ('\u{200b}', "\\u{200b}"),
    ('\u{2060}', "\\u{2060}"),
    ('\u{feff}', "\\u{feff}"),
    ('\u{ad}', "\\u{ad}"),
];

/// The ranges of `ui/src/ui/text.ts`'s `FORMAT`, the UI's table of Unicode Cf,
/// read when the test runs.
fn ui_format_table() -> Vec<(u32, u32)> {
    let text = read_text(&repository_root(), "ui/src/ui/text.ts");
    let at = text.find("const FORMAT =").expect("the UI's FORMAT table");
    let rest = &text[at..];
    let open = rest.find("/[").expect("a class") + 2;
    let close = rest.find("]/u").expect("its end");
    let class: Vec<char> = rest[open..close].chars().collect();
    let mut points = Vec::new();
    let mut i = 0;
    while i < class.len() {
        assert_eq!(&class[i..i + 2], ['\\', 'u'], "an escape at {i}: {class:?}");
        let (hex, next) = if class[i + 2] == '{' {
            let end = (i + 3..class.len())
                .find(|&j| class[j] == '}')
                .expect("a brace");
            (class[i + 3..end].iter().collect::<String>(), end + 1)
        } else {
            (class[i + 2..i + 6].iter().collect::<String>(), i + 6)
        };
        points.push(u32::from_str_radix(&hex, 16).expect("hex"));
        i = next;
        if class.get(i) == Some(&'-') {
            points.push(u32::MAX);
            i += 1;
        }
    }
    let mut ranges = Vec::new();
    let mut at = 0;
    while at < points.len() {
        if points.get(at + 1) == Some(&u32::MAX) {
            ranges.push((points[at], points[at + 2]));
            at += 3;
        } else {
            ranges.push((points[at], points[at]));
            at += 1;
        }
    }
    ranges
}

/// `c` in the UI's table of format characters.
fn in_table(table: &[(u32, u32)], c: char) -> bool {
    let code = u32::from(c);
    table
        .iter()
        .any(|(low, high)| (*low..=*high).contains(&code))
}

/// `text` with each format character of `table` written `\u{..}`, lower-case
/// hex: what docs/features/hub-confirm.md "Data", Escaping, makes of a
/// terminal's question.
fn escaped_format(table: &[(u32, u32)], text: &str) -> String {
    text.chars()
        .map(|c| {
            if in_table(table, c) {
                format!("\\u{{{:x}}}", u32::from(c))
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// A character the queue refuses in any text it stores, core
/// `record::is_escaped`: such a text never reaches a question.
fn queue_refuses(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(
            c,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

/// Escaping: the UI's table is Unicode's general category Cf at every code
/// point, as the `regex` crate's Unicode tables read it, so the dialog and the
/// daemon can share one set. M: a range dropped from or added to the table.
#[test]
fn b12_the_uis_format_table_is_unicode_cf_at_every_code_point() {
    let table = ui_format_table();
    for (c, _) in FORMAT_FOUR {
        assert!(in_table(&table, c), "{c:?}");
    }
    let cf = regex::Regex::new(r"^\p{Cf}$").expect("a pattern");
    let mut apart = Vec::new();
    for code in 0..=0x10_ffff_u32 {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let mut buffer = [0u8; 4];
        if cf.is_match(c.encode_utf8(&mut buffer)) != in_table(&table, c) {
            apart.push(format!("U+{code:04X}"));
        }
    }
    assert!(apart.is_empty(), "the table and Cf part on {apart:?}");
}

/// B12: a question's text and its staged note holding U+200B, U+2060, U+FEFF and
/// U+00AD. The terminal's `spec approve` question keeps them raw; the read's
/// question is that question with each written `\u{..}`, lower-case hex, and
/// holds none raw; its POST confirms, the record on disk holding them raw. The
/// raw question, posted, is a 409. M: an unescaped Cf.
#[test]
fn b12_format_characters_read_escaped_and_the_escaped_question_confirms() {
    let table = ui_format_table();
    let setup = keep("confirm-format");
    let raw: String = FORMAT_FOUR.iter().map(|(c, _)| *c).collect();
    let id = asked(
        &setup,
        &["MEC-SPRINT"],
        &format!("Does a sprint end{raw} at zero?"),
    );
    let desk = Desk::new(setup, KEEP);
    desk.staged(
        "mira",
        &id,
        &json!({"decision": "approve", "note": format!("Keep{raw} it.")}),
    );
    let terminal = approve_asks(desk.setup(), &id, git_as(desk.setup(), "mira"));
    for (c, _) in FORMAT_FOUR {
        assert!(
            terminal.contains(c),
            "the terminal keeps {c:?} raw: {terminal}"
        );
    }
    let read = desk.read_ok("mira", &id);
    let question = read["question"].as_str().expect("a question");
    for (c, escape) in FORMAT_FOUR {
        assert!(!question.contains(c), "{c:?} raw in {question}");
        assert!(question.contains(escape), "{escape} not in {question}");
    }
    assert_eq!(question, escaped_format(&table, &terminal));
    let ids = [id.as_str()];
    let before = desk.left(&ids);
    let reply = desk.answer("mira", &id, &terminal, &read["updated_at"]);
    assert_eq!(reply.status, 409, "the raw question: {}", reply.text());
    assert_eq!(desk.left(&ids), before, "nothing changed");

    let document = desk.confirm("mira", &id, &read).status(200).json();
    assert_eq!(document["status"], json!("applied"), "{document}");
    let record = read_text(desk.root(), "docs/records/DEC/DEC-100001.md");
    for (c, _) in FORMAT_FOUR {
        assert!(record.contains(c), "the record holds {c:?} raw");
    }
}

/// B12 over every format character the queue takes: one staged note holding
/// each Cf code point of the UI's table but those the queue refuses (the
/// bidirectional ones, core `record::is_escaped`), and characters next to them
/// that are no Cf (an invisible mark outside Cf, U+034F, among them). The read's
/// question writes each Cf `\u{..}` and none of the others, so the daemon's set
/// is the UI's on every code point a question can hold. M: an unescaped Cf; a
/// non-Cf escaped.
#[test]
fn b12_the_daemons_escaped_set_is_the_uis_table() {
    let table = ui_format_table();
    let format: Vec<char> = table
        .iter()
        .flat_map(|(low, high)| *low..=*high)
        .filter_map(char::from_u32)
        .filter(|c| !queue_refuses(*c))
        .collect();
    let others: Vec<char> = [
        0xac, 0xae, 0x34f, 0x606, 0x200a, 0x2010, 0x2065, 0x2070, 0xfe0f, 0xfefe, 0xfff8, 0xfffc,
        0x3164, 0x110bc, 0x110be, 0x1d172, 0x1d17b, 0xe0000, 0xe0080,
    ]
    .into_iter()
    .filter_map(char::from_u32)
    .collect();
    for c in &others {
        assert!(!in_table(&table, *c), "{c:?} is no Cf");
    }
    let note: String = format.iter().chain(&others).collect();
    let setup = keep("confirm-format-set");
    let id = asked(&setup, &["MEC-SPRINT"], "Does a sprint end at zero?");
    let desk = Desk::new(setup, KEEP);
    desk.staged("ann", &id, &json!({"decision": "approve", "note": note}));
    let read = desk.read_ok("ann", &id);
    let question = read["question"].as_str().expect("a question");
    let mut unescaped = Vec::new();
    for c in &format {
        if question.contains(*c) || !question.contains(&format!("\\u{{{:x}}}", u32::from(*c))) {
            unescaped.push(format!("U+{:04X}", u32::from(*c)));
        }
    }
    assert!(unescaped.is_empty(), "Cf not escaped: {unescaped:?}");
    let mut escaped = Vec::new();
    for c in &others {
        if !question.contains(*c) {
            escaped.push(format!("U+{:04X}", u32::from(*c)));
        }
    }
    assert!(escaped.is_empty(), "no Cf, yet not shown raw: {escaped:?}");
    desk.confirm("ann", &id, &read).status(200);
}

/// B13, "A stale `[team]` entry": the entry is read again inside the project's
/// turn, and that read is the one judged. `ann`'s confirm holds the turn, its
/// commit stopped in a `pre-commit` hook; `lena`'s POST passes the reads before
/// the turn and waits behind it; `[team]` then demotes her to `member`, or
/// narrows her zones off the target; the hook lets go. `ann`'s lands, `lena`'s is
/// the refusal of her new entry, her row and target as they were. The wait for
/// her to reach the turn is a clock, 1.5 s, and only a mutation can lose that
/// race: with her entry judged before the turn she would land. M: the entry read
/// before the turn.
#[test]
fn b13_an_entry_changed_while_its_post_waits_for_the_turn_is_judged_anew() {
    let member = "[team.lena]\nrole = \"member\"\nemail = \"lena@example.org\"\n";
    let front = "[team.lena]\nrole = \"lead\"\nzones = [\"front\"]\nemail = \"lena@example.org\"\n";
    for (label, entry, line) in [
        ("demoted", member, member_line("lena")),
        ("narrowed", front, zone_line("lena", "front", BACK_MD)),
    ] {
        let setup = press(&format!("confirm-turn-{label}"));
        let held = reworded(
            &setup,
            "SHT-FRONT-HEAD",
            "stays the same",
            "changes with the season",
        );
        let id = reworded(&setup, "SHT-BACK", "the tide table", "the moon table");
        let desk = Desk::new(setup, P);
        desk.staged("tomas", &held, &approved_choice());
        desk.staged("tomas", &id, &approved_choice());
        let root = desk.root().to_path_buf();
        let good = read_text(&root, "specengine.toml");
        assert_eq!(good.matches(LENA_ENTRY).count(), 1, "lena's entry");
        let gate = desk.setup().scratch.dir("gate");
        let (entered, go) = (gate.join("entered"), gate.join("go"));
        hook(
            &root,
            "pre-commit",
            &format!(
                "touch '{}'\ni=0\nwhile [ ! -f '{}' ] && [ $i -lt 1200 ]; do sleep 0.05; \
                 i=$((i+1)); done",
                entered.display(),
                go.display()
            ),
        );
        let request = |login: &str, id: &str| {
            let read = desk.read_ok(login, id);
            let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
            Proxied::write(login).bytes(
                "POST",
                &desk.route(id, "confirm"),
                body.to_string().as_bytes(),
            )
        };
        let (first, second) = (request("ann", &held), request("lena", &id));
        let row = desk.hub.proposal(&id);
        let target = bytes(&root, BACK_MD);
        let port = desk.hub.server.port;
        let (holder, waiter) = thread::scope(|scope| {
            let holder = scope.spawn(|| http_exchange(port, &first, HTTP_TIMEOUT));
            let started = std::time::Instant::now();
            while !entered.exists() {
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(60),
                    "{label}: the hook never ran"
                );
                thread::sleep(std::time::Duration::from_millis(20));
            }
            let waiter = scope.spawn(|| http_exchange(port, &second, HTTP_TIMEOUT));
            thread::sleep(std::time::Duration::from_millis(1500));
            write(&root, "specengine.toml", good.replace(LENA_ENTRY, entry));
            fs::write(&go, b"").expect("the gate");
            (
                holder.join().expect("the holder"),
                waiter.join().expect("the waiter"),
            )
        });
        write(&root, "specengine.toml", &good);
        fs::remove_file(root.join(".git/hooks/pre-commit")).expect("the hook");
        assert_eq!(holder.status, 200, "{label}: {}", holder.text());
        assert_refused(&waiter, &line, label);
        assert_eq!(desk.hub.proposal(&id), row, "{label}: her row as it was");
        assert_eq!(
            bytes(&root, BACK_MD),
            target,
            "{label}: her target as it was"
        );
    }
}

// ------------------------------------------- apply-outputs (RM-0072): O2, O8

/// The game of docs/features/apply-outputs.md (`fixtures/apply-outputs/forge`).
const FORGE: &str = "/api/projects/ember-forge";

/// "Data"'s new file and its text, as the terminal's `tests/apply_outputs.rs`
/// raises it.
const STAMINA_PATH: &str = "docs/features/stamina.md";
const STAMINA_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
     # Smith stamina {#MEC-STAMINA}\n\nEach strike costs the smith one stamina; at zero the \
     hammer stops until the smith rests.\n";

/// The game's team: `ann` its owner.
const FORGE_TEAM: &str = "\n[team.ann]\nrole = \"owner\"\nemail = \"ann@example.org\"\n";

/// The game with its index's outputs rendered by `spec export index` and its
/// team, committed; `roots` edits the `[paths] roots` line first (the fixture's
/// own when `None`).
fn forge(name: &str, roots: Option<&str>) -> Setup {
    let setup = Setup::new(name, "apply-outputs/forge");
    spec(&setup.home, &setup.root, &["export", "index"]).code(0);
    if let Some(roots) = roots {
        replace(
            &setup.root,
            "specengine.toml",
            "roots = [\"README.md\", \"docs\", \"design\"]",
            roots,
        );
    }
    append_config(&setup.root, FORGE_TEAM);
    committed(&setup, "the index and the team");
    setup
}

/// `spec propose create` of [`STAMINA_TEXT`] on the hub's host, its data
/// directory: the ID.
fn stamina_raised(setup: &Setup) -> String {
    let root = setup.root.to_str().expect("UTF-8 root");
    let run = run_product(
        &spec_bin(),
        &setup.home,
        &setup.cwd,
        &[
            "--root",
            root,
            "propose",
            "create",
            STAMINA_PATH,
            "--text-file",
            "-",
            "--rationale",
            "The smith tires.",
        ],
        STAMINA_TEXT.as_bytes(),
    );
    run.code(0);
    run.stdout.lines().next().unwrap_or_default().to_owned()
}

/// `git diff-tree --name-status` of `commit` in the hub's checkout.
fn name_status_of(desk: &Desk, commit: &str) -> Vec<String> {
    desk.git(&["diff-tree", "-r", "--no-commit-id", "--name-status", commit])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The `index-*` findings of `spec check` over the hub's checkout.
fn index_findings_of(desk: &Desk) -> Vec<Value> {
    let setup = desk.setup();
    let report = spec_json(&setup.home, &setup.cwd, &setup.root, &["check"]).json();
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| {
            finding["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("index-"))
        })
        .cloned()
        .collect()
}

/// O2: O1's create raised on the hub's host, its data directory (so
/// `PR-100001`, the hub's floor), staged and confirmed by `POST
/// .../proposals/PR-100001/confirm` as the `[team]` `owner`: 200, `applied`,
/// one commit whose name-status equals O1's, `A` of the new file beside `M
/// docs/index.md`, no warning, and no index finding after. M: render
/// only on the terminal path.
#[test]
fn o2_a_hubs_confirm_commits_the_index_as_a_terminals_apply() {
    let desk = Desk::new(forge("confirm-outputs", None), FORGE);
    let id = stamina_raised(desk.setup());
    assert_eq!(id, "PR-100001", "the hub's floor");
    desk.staged("ann", &id, &approved_choice());
    let parent = desk.head();
    let document = desk.confirmed("ann", &id);
    let commit = desk.head();
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(document["applied_commit"], json!(commit), "{document}");
    assert_eq!(desk.parents(&commit), [parent], "one commit");
    assert_eq!(
        name_status_of(&desk, &commit),
        [format!("A\t{STAMINA_PATH}"), "M\tdocs/index.md".to_owned()]
    );
    assert_eq!(
        desk.trailers(&commit, "Proposal"),
        std::slice::from_ref(&id)
    );
    assert!(
        !document.to_string().contains("were not rendered"),
        "no warning: {document}"
    );
    assert_eq!(index_findings_of(&desk), Vec::<Value>::new());
    assert_eq!(
        desk.git(&["status", "--porcelain", "--untracked-files=all"]),
        "",
        "the checkout clean"
    );
}

/// O8's hub half: `HEAD`'s config lists the root `notes`, present in the
/// checkout and absent from `HEAD`'s tree: the confirm answers 200, `applied`,
/// the commit holds the target alone, and the POST's review document holds the
/// warning's text in `notes`, the `MissingRoot` gap and `spec export index`
/// named. M: render the partial walk; refuse the apply; the warning kept from
/// the page.
#[test]
fn o8_a_hubs_confirm_with_a_gap_commits_the_target_alone_and_notes_why() {
    let desk = Desk::new(
        forge(
            "confirm-outputs-gap",
            Some("roots = [\"README.md\", \"docs\", \"design\", \"notes\"]"),
        ),
        FORGE,
    );
    write(
        desk.root(),
        "notes/plan.md",
        "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n# A plan\n\nTo sort.\n",
    );
    let id = stamina_raised(desk.setup());
    desk.staged("ann", &id, &approved_choice());
    let document = desk.confirmed("ann", &id);
    let commit = desk.head();
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(
        name_status_of(&desk, &commit),
        [format!("A\t{STAMINA_PATH}")]
    );
    let warning = format!(
        "the commit carries `{STAMINA_PATH}` alone: the index's outputs were not rendered: the \
         `[paths] roots` entry `notes` names no directory and no `.md` file; `spec export index` \
         writes them"
    );
    let notes = document["notes"].as_array().expect("notes");
    assert!(
        notes
            .iter()
            .any(|note| note.as_str().is_some_and(|note| note.contains(&warning))),
        "the warning in notes: {document}"
    );
}

/// docs/features/hub-git.md "Status and the screen", **The confirm dialog after
/// a 200**, the daemon's half: the dialog stays open on any note, so a clean
/// confirm's 200 document holds `notes: []`. Two clean confirms: an update of
/// the print shop, which renders no index, and O2's create on the game, whose
/// one commit carries the index's outputs, rendered with no gap: the outputs'
/// `note:` is the terminal's stderr alone, and only a gap's warning reaches the
/// POST's `notes` (docs/canon/proposal-outputs.md "Messages"). M: the outputs'
/// note appended to `notes`; a note on every confirm.
#[test]
fn a_clean_confirms_document_holds_no_notes() {
    let (desk, id) = back_update_staged("confirm-clean-notes");
    let document = desk.confirmed("lena", &id);
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(
        document["notes"],
        json!([]),
        "an update, no index: {document}"
    );

    let desk = Desk::new(forge("confirm-clean-notes-outputs", None), FORGE);
    let id = stamina_raised(desk.setup());
    desk.staged("ann", &id, &approved_choice());
    let document = desk.confirmed("ann", &id);
    assert_eq!(document["status"], json!("applied"), "{document}");
    assert_eq!(
        name_status_of(&desk, &desk.head()),
        [format!("A\t{STAMINA_PATH}"), "M\tdocs/index.md".to_owned()],
        "the premise: the outputs rendered and committed"
    );
    assert_eq!(
        document["notes"],
        json!([]),
        "a create with its outputs, no gap: {document}"
    );
}

// ---------------------------------------------- card-questions (RM-0097)

/// docs/features/card-questions.md H4: the game's hub, its `[decision_records]`
/// template gaining "Data"'s `Why:` lines and a `member`, `tomas`, on its team.
/// `tomas` raises a decision with a `why` on `C-0001` through the card's route,
/// stages it; `kit`, the lead covering `stamina.md`, confirms it: one commit
/// adding its record, the body holding the why. `GET .../cards/C-0001` then
/// lists the item `applied` with `record_id`, and its `history` ends with
/// `proposal.approved` then `proposal.applied`. M: the confirm's record
/// rendered without the slot.
#[test]
fn card_questions_h4_a_carded_decision_confirmed_on_a_hub_writes_its_why() {
    let setup = keep("confirm-cq-why");
    let template = read_text(&setup.root, "templates/decision.md");
    let block = "Cost:\n\n{{cost}}\n";
    assert!(template.contains(block), "{template}");
    write(
        &setup.root,
        "templates/decision.md",
        template.replacen(block, "Cost:\n\n{{cost}}\n\nWhy:\n\n{{why}}\n", 1),
    );
    append_config(
        &setup.root,
        "\n[team.tomas]\nrole = \"member\"\nemail = \"tomas@example.org\"\n",
    );
    committed(&setup, "the why slot and a member");
    let made = spec_json(
        &setup.home,
        &setup.cwd,
        &setup.root,
        &[
            "card",
            "new",
            "--title",
            "Rest regeneration",
            "--target",
            "RULE-STAM-REGEN",
        ],
    );
    made.code(0);
    assert_eq!(made.json()["id"], json!("C-0001"));
    let desk = Desk::new(setup, KEEP);

    let why = "Two playtests read instant regeneration as a bug.";
    let body = json!({"kind": "decision", "node_ids": ["RULE-STAM-REGEN"],
        "text": "Does regeneration wait for rest?",
        "working_answer": "Yes: it starts 1.5 s after the last stamina use.",
        "price_of_other": "Without the wait R-28 is rebalanced.",
        "why": why, "severity": null, "distinct_from": ["DEC-0023"], "as": null});
    let reply = through(
        &desk.hub.server,
        "POST",
        &format!("{KEEP}/cards/C-0001/items"),
        &Proxied::write("tomas"),
        body.to_string().as_bytes(),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    let raised = reply.json();
    assert_eq!(raised["created"], json!(true), "{raised}");
    let id = raised["id"].as_str().expect("an ID").to_owned();

    desk.staged("tomas", &id, &approved_choice());
    let before = desk.head();
    let review = desk.confirmed("kit", &id);
    assert_eq!(review["status"], json!("applied"), "{review}");
    // A hub issues from its floor (docs/canon/spec-ids.md): the record's ID
    // and path are the review's.
    let record_id = review["record_id"].as_str().expect("a record").to_owned();
    let record_path = format!("docs/records/DEC/{record_id}.md");
    assert_eq!(review["record_path"], json!(record_path), "{review}");
    let head = desk.head();
    assert_eq!(desk.parents(&head), [before]);
    assert_eq!(desk.paths(&head), std::slice::from_ref(&record_path));
    let record = read_text(desk.root(), &record_path);
    assert!(
        record.contains(&format!("\n\nWhy:\n\n{why}\n\nSummary:\n\n")),
        "{record}"
    );

    let card = desk
        .hub
        .get("kit", &format!("{KEEP}/cards/C-0001"))
        .status(200)
        .json();
    let item = &card["items"][0];
    assert_eq!(item["id"], json!(id), "{card}");
    assert_eq!(item["status"], json!("applied"), "{item}");
    assert_eq!(item["record_id"], json!(record_id), "{item}");
    assert_eq!(item["why"], json!(why), "{item}");
    let history: Vec<&str> = card["history"]
        .as_array()
        .expect("history")
        .iter()
        .map(|entry| entry["type"].as_str().expect("a type"))
        .collect();
    assert_eq!(
        history[history.len() - 2..],
        ["proposal.approved", "proposal.applied"],
        "{history:?}"
    );
}
