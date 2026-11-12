//! docs/features/spec-ids.md (RM-0073), the hub's half: a hub's data
//! directory (`proposal-floor`, which its start writes) mints every
//! number-shape spec ID from 100001, a developer's terminal below it, so the
//! two meet on one branch with no add/add conflict and no `id-taken`.
//! Through the real daemon (`--public-origin --push --push-deadline 3`) over
//! a clone of a bare repository in the test's scratch, which a developer's
//! clone pushes to as well: the world of `tests/hub_git.rs`, over
//! `fixtures/apply-outputs/forge` (a smithing game, `DEC` width 4, its index
//! generated) and `fixtures/card-press` (a print shop, `STY` width 4,
//! `STY-0001` in the tree), so the range is tried on projects of different
//! nature (ADR-0008).
//!
//! - AC-1 `ac1_*`: a developer's `DEC-0001` pushed, the hub's confirm of a
//!   question raised on it `DEC-100001`; the sync ends `ok` with a merge
//!   commit holding both, `spec check --json` reports no `id-taken` and no
//!   `id-width`; the developer pulls and approves `DEC-0002`. M1, M2.
//! - AC-2 `ac2_*`: the hub's `/mcp` `propose_change` create of `STY-0002` is
//!   refused naming `STY-100001`; resent with it, stored and confirmed; a
//!   developer's `STY-0002` pushed; the sync merges cleanly, no `id-taken`.
//!   M3.
//! - AC-5 (a) `ac5a_*`: a terminal's dump holding an open create of
//!   `STY-0002` and an open amendment defining `STY-0003`, restored by `spec
//!   import-state` into an empty queue whose data directory holds the floor:
//!   on the hub each confirm answers 200 and is `applied`. M9.
//! - AC-8 (a), the wire `ac8a_*`: `tools/list` over stdio and over a hub's
//!   `/mcp` shows "The MCP texts", ASCII, at 2044 and 2040 bytes. (d)
//!   `ac8d_*`: `tag-check.sh v0.29.1` exits 0 printing `0.29.1`, and
//!   `.env.example` names the version; since docs/features/flow-scope.md
//!   (FS-6, the plugin's MINOR to 0.30.0) the tag is `v0.30.0`.
//!
//! Every expected value is worked out from docs/features/spec-ids.md, never
//! read back from a run.

#![cfg(unix)]

mod common;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use common::hub::{Hub, ORIGIN, P, Proxied, Setup, hub_args, through};
use common::mcp_http::{Session, content_text, is_error, result_of, tool};
use common::{
    FLOOR_FILE, Git, HUB_FLOOR, Mcp, Reply, Server, alarmed, data_dir, product_env, read_text,
    repository_root, run_product, spec, spec_bin, spec_json, write,
};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveRequest, Env, Exit, ExportStateRequest, GitEnv, Globals, ImportStateRequest,
    QuestionRequest, approve, export_state, import_state, propose_question, utc_now,
};

/// The print shop's project and the game's.
const PRESS: &str = "/api/projects/card-press";
const FORGE: &str = "/api/projects/ember-forge";

/// The one login of each project's `[team]`: its owner stages and confirms.
const TEAM: &str = "\n[team.ann]\nrole = \"owner\"\nemail = \"ann@example.org\"\n";

/// The flags of every pushing hub here.
const PUSH: [&str; 3] = ["--push", "--push-deadline", "3"];

/// A sync a signal starts ends well within this.
const SYNC_WAIT: Duration = Duration::from_secs(60);
/// More than `at`'s resolution, a second.
const NEXT_SECOND: Duration = Duration::from_millis(1100);
const POLL: Duration = Duration::from_millis(100);

/// docs/features/spec-ids.md "Data", "The MCP texts": the two sentences.
///
/// docs/features/intake-similar.md "Acceptance criteria" IS8: each sentence
/// ends `; a similar entry with gates true stores nothing (id null) until
/// distinct_from names it.` (2 041 and 2 037 bytes since).
const HUB_SENTENCE: &str = "you name each new ID, from 100001 on this hub; a taken or lower one is \
                            refused naming the next free ID; a similar entry with gates true \
                            stores nothing (id null) until distinct_from names it.";
const STDIO_SENTENCE: &str = "you name each new ID, below 100001 unless on a hub; a taken one is \
                              refused naming the next free ID; a similar entry with gates true \
                              stores nothing (id null) until distinct_from names it.";
const OLD_SENTENCE: &str = "you name each new ID; a taken one is refused naming the next free one.";

// ---------------------------------------------------------------- helpers

/// docs/features/spec-ids.md "Data", "Refusal texts": a hub's.
fn off_hub(id: &str, line: usize, next: &str) -> String {
    format!(
        "`{id}` (line {line}): a hub numbers from 100001; the next free here is `{next}`; nothing \
         stored"
    )
}

/// A card-press style ruling whose `id:` (line 2) is `id`.
fn ruling(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         canon: docs/spec/sht-front.md#the-front-sheet\nlinks:\n  constrains: [SHT-FRONT]\n\
         scope: [front-sheet]\n---\n\n# The masthead keeps one face\n\n\
         The masthead runs in one typeface every week.\n"
    )
}

/// A new spec file amending `SHT-FRONT`, defining the section `{#id}`
/// (line 11).
fn amending(id: &str) -> String {
    format!(
        "---\nclass: spec\nstatus: draft\nscope: [front-sheet]\nlinks:\n  amends: [SHT-FRONT]\n\
         ---\n\n# A special edition\n\n## Its masthead {{#{id}}}\n\n\
         A special edition may run its own masthead.\n"
    )
}

fn ruling_path(id: &str) -> String {
    format!("docs/records/STY/{id}.md")
}

fn dec_path(id: &str) -> String {
    format!("docs/records/DEC/{id}.md")
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// `table` appended to `root`'s `specengine.toml`.
fn append(root: &Path, table: &str) {
    let mut config = read_text(root, "specengine.toml");
    config.push_str(table);
    write(root, "specengine.toml", config);
}

fn commit_all(git: &Git, dir: &Path, message: &str) {
    git.run(dir, &["add", "-A"]);
    git.run(dir, &["commit", "-q", "-m", message]);
}

/// The library's calls in `root` under `home`, as a terminal there makes
/// them.
fn library(home: &Path, root: &Path) -> (Env, Globals, GitEnv) {
    (
        Env {
            cwd: root.to_path_buf(),
            home: Some(home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        Globals {
            root: Some(root.to_path_buf()),
            config: None,
        },
        GitEnv::new(root, product_env(home)),
    )
}

/// A question on `targets` asked in `root` under `home`, stored: when its
/// first ask hits, asked again naming every hit (`docs/canon/agent-intake.md`
/// "Dedup"). Its ID.
fn asked(home: &Path, root: &Path, targets: &[&str], question: &str) -> String {
    let (env, globals, git) = library(home, root);
    let mut request = QuestionRequest {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        text: question.to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: utc_now(),
        git,
        card: None,
    };
    let ask = |request: &QuestionRequest| {
        propose_question(&env, &globals, request)
            .unwrap_or_else(|error| panic!("{question}: {error}"))
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
    assert_eq!(outcome.exit(), Exit::Answered, "{question}: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("the question's ID")
}

/// `spec approve id` in `root` under `home`, consent yes: it must apply.
fn approved_at(home: &Path, root: &Path, id: &str) {
    let (env, globals, git) = library(home, root);
    let outcome = approve(
        &env,
        &globals,
        &ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: utc_now(),
            git,
        },
        &mut |_: &str| true,
    )
    .unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
}

/// `spec --root root propose create path` of `body` under `home`: its ID.
fn raised_create(home: &Path, root: &Path, path: &str, body: &str) -> String {
    let run = run_product(
        &spec_bin(),
        home,
        root,
        &[
            "--root",
            text(root),
            "propose",
            "create",
            path,
            "--text-file",
            "-",
            "--rationale",
            "A second ruling.",
            "--author-role",
            "writer",
        ],
        body.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

/// `spec --root root propose amendment path` of `body` under `home`: its ID.
fn raised_amendment(home: &Path, root: &Path, path: &str, body: &str) -> String {
    let run = run_product(
        &spec_bin(),
        home,
        root,
        &[
            "--root",
            text(root),
            "propose",
            "amendment",
            path,
            "--text-file",
            "-",
            "--rationale",
            "A special edition.",
            "--author-role",
            "writer",
        ],
        body.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

/// The codes of `spec check --json` in `root`, read by a scratch `HOME`.
fn check_codes(home: &Path, cwd: &Path, root: &Path) -> Vec<(String, String)> {
    let report = spec_json(home, cwd, root, &["check"]).json();
    report["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("findings: {report}"))
        .iter()
        .map(|finding| {
            (
                finding["code"].as_str().unwrap_or_default().to_owned(),
                finding["subject"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// AC-1, AC-2: neither `id-taken` nor `id-width` among `codes`.
#[track_caller]
fn assert_no_id_findings(codes: &[(String, String)], context: &str) {
    let ids: Vec<&(String, String)> = codes
        .iter()
        .filter(|(code, _)| code == "id-taken" || code == "id-width")
        .collect();
    assert!(ids.is_empty(), "{context}: {ids:?}");
}

#[track_caller]
fn assert_state(status: &Value, state: &str, context: &str) {
    assert_eq!(status["state"], json!(state), "{context}: {status}");
}

// ------------------------------------------------------------ the world

/// A project's git world, as `tests/hub_git.rs` builds it: a bare
/// `origin.git`, the developer's clone (where the fixture was first
/// committed, its own `HOME`, a terminal's data directory) and the hub's
/// checkout, cloned from the bare repository, `main` tracking `origin/main`.
/// `server` is first: it stops before the scratch goes.
struct World {
    server: Option<Server>,
    setup: Setup,
    base: &'static str,
    bare: PathBuf,
    dev: PathBuf,
    dev_home: PathBuf,
}

impl World {
    /// The print shop with its `[team]`.
    fn press(name: &str) -> Self {
        let seed = Setup::new(name, "card-press");
        append(&seed.root, TEAM);
        commit_all(&seed.scratch.git(), &seed.root, "the team");
        Self::over(seed, PRESS)
    }

    /// The game, its index's outputs rendered by `spec export index` and its
    /// `[team]`, committed.
    fn forge(name: &str) -> Self {
        let seed = Setup::new(name, "apply-outputs/forge");
        let renderer = seed.scratch.home("renderer");
        spec(&renderer, &seed.root, &["export", "index"]).code(0);
        append(&seed.root, TEAM);
        commit_all(&seed.scratch.git(), &seed.root, "the index and the team");
        Self::over(seed, FORGE)
    }

    fn over(seed: Setup, base: &'static str) -> Self {
        let Setup {
            root: dev,
            home,
            cwd,
            scratch,
        } = seed;
        let git = scratch.git();
        let bare = scratch.dir("origin.git");
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
        git.quiet(&bare);
        git.run(&bare, &["config", "receive.autogc", "false"]);
        git.run(&dev, &["remote", "add", "origin", text(&bare)]);
        git.run(&dev, &["push", "-q", "-u", "origin", "main"]);
        let hub = scratch.join("hub");
        git.run(
            scratch.path(),
            &["clone", "-q", "--template=", text(&bare), text(&hub)],
        );
        let hub = fs::canonicalize(hub).expect("the hub's checkout");
        git.quiet(&hub);
        git.run(&hub, &["config", "user.name", "Owner"]);
        git.run(&hub, &["config", "user.email", "owner@example.invalid"]);
        let dev_home = scratch.home("developer");
        Self {
            server: None,
            setup: Setup {
                root: hub,
                home,
                cwd,
                scratch,
            },
            base,
            bare,
            dev,
            dev_home,
        }
    }

    fn root(&self) -> &Path {
        &self.setup.root
    }

    fn git(&self) -> Git {
        self.setup.scratch.git()
    }

    /// `git args` in `dir`, which must succeed: its stdout, trimmed.
    fn run(&self, dir: &Path, args: &[&str]) -> String {
        self.git().run(dir, args).trim_end().to_owned()
    }

    fn rev(&self, dir: &Path, revision: &str) -> String {
        self.run(dir, &["rev-parse", "--verify", "-q", revision])
    }

    fn head(&self) -> String {
        self.rev(self.root(), "HEAD")
    }

    fn remote_main(&self) -> String {
        self.rev(&self.bare, "refs/heads/main")
    }

    fn parents(&self, commit: &str) -> Vec<String> {
        self.run(&self.bare, &["rev-list", "--parents", "-n", "1", commit])
            .split_whitespace()
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    /// Whether `path` is in `commit`'s tree of the bare repository.
    fn holds(&self, commit: &str, path: &str) -> bool {
        self.git()
            .output(&self.bare, &["cat-file", "-e", &format!("{commit}:{path}")])
            .status
            .success()
    }

    /// The hub over the checkout with [`PUSH`], listening, then the start's
    /// sync ended: its status. The start writes the floor.
    fn start(&mut self) -> Value {
        let root = self.root().to_path_buf();
        let mut args = hub_args(&[root.as_path()], ORIGIN);
        args.extend(PUSH.iter().map(OsStr::new));
        self.server = Some(Server::start(&self.setup.home, &self.setup.cwd, &args));
        assert_eq!(
            fs::read(data_dir(&self.setup.home).join(FLOOR_FILE)).expect("the floor"),
            HUB_FLOOR,
            "the hub's start wrote its floor"
        );
        self.after(&json!({"at": null}), SYNC_WAIT)
    }

    fn server(&self) -> &Server {
        self.server.as_ref().expect("a running daemon")
    }

    fn port(&self) -> u16 {
        self.server().port
    }

    fn get(&self, path: &str) -> Reply {
        through(self.server(), "GET", path, &Proxied::fetch("ann"), b"")
    }

    fn post(&self, path: &str, body: &Value) -> Reply {
        through(
            self.server(),
            "POST",
            &format!("{}/{path}", self.base),
            &Proxied::write("ann"),
            body.to_string().as_bytes(),
        )
    }

    /// `GET .../sync`: a 200, its document.
    fn status(&self) -> Value {
        let reply = self.get(&format!("{}/sync", self.base));
        assert_eq!(reply.status, 200, "GET sync: {}", reply.text());
        reply.json()
    }

    /// The first status after `before`'s whose `at` differs and is set.
    fn after(&self, before: &Value, within: Duration) -> Value {
        let started = Instant::now();
        loop {
            let status = self.status();
            if !status["at"].is_null() && status["at"] != before["at"] {
                return status;
            }
            assert!(
                started.elapsed() < within,
                "no sync ended within {within:?} after {before}: {status}\n--- the daemon's \
                 stderr\n{}",
                self.server().stderr()
            );
            thread::sleep(POLL);
        }
    }

    /// `act` a second after the last sync's end, then the next sync's status.
    fn synced<T>(&self, act: impl FnOnce() -> T) -> (T, Value) {
        let before = self.status();
        thread::sleep(NEXT_SECOND);
        let out = act();
        (out, self.after(&before, SYNC_WAIT))
    }

    fn review(&self, id: &str) -> Value {
        let setup = &self.setup;
        spec_json(&setup.home, &setup.cwd, &setup.root, &["review", id]).json()
    }

    /// `id` staged by `ann` and confirmed in one click: the POST's document,
    /// `applied`.
    fn confirm(&self, id: &str) -> Value {
        let stage = self.post(
            &format!("proposals/{id}/decision"),
            &json!({"decision": "approve", "updated_at": self.review(id)["updated_at"]}),
        );
        assert_eq!(stage.status, 200, "ann stages {id}: {}", stage.text());
        let route = format!("{}/proposals/{id}/confirm", self.base);
        let read = self.get(&route);
        assert_eq!(read.status, 200, "ann reads {id}: {}", read.text());
        let read = read.json();
        let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
        let reply = through(
            self.server(),
            "POST",
            &route,
            &Proxied::write("ann"),
            body.to_string().as_bytes(),
        );
        assert_eq!(reply.status, 200, "ann confirms {id}: {}", reply.text());
        let document = reply.json();
        assert_eq!(document["status"], json!("applied"), "{document}");
        document
    }

    /// The developer's clone brought to the remote's `main`.
    fn developer_pulls(&self) {
        self.run(&self.dev, &["pull", "-q", "--ff-only", "origin", "main"]);
    }

    /// The developer's `main` pushed: its SHA.
    fn developer_pushes(&self) -> String {
        self.run(&self.dev, &["push", "-q", "origin", "main"]);
        self.rev(&self.dev, "HEAD")
    }

    /// After a confirm's sync: `ok`, the remote tip `HEAD`, a merge commit
    /// whose parents are `applied` and `theirs`, holding every one of
    /// `paths`.
    #[track_caller]
    fn assert_merged(&self, status: &Value, applied: &str, theirs: &str, paths: &[String]) {
        assert_state(status, "ok", "the merge pushed");
        let tip = self.remote_main();
        assert_eq!(tip, self.head(), "the remote holds HEAD");
        assert_eq!(
            self.parents(&tip),
            [applied, theirs],
            "a merge commit: applied_commit first, the developer's second"
        );
        for path in paths {
            assert!(self.holds(&tip, path), "{path} at the tip");
        }
        assert_eq!(
            self.run(
                self.root(),
                &["status", "--porcelain=v1", "--untracked-files=all"]
            ),
            "",
            "the checkout clean"
        );
    }
}

// ------------------------------------------------------------------ AC-1

/// docs/features/spec-ids.md "Acceptance criteria" AC-1 (forge, `DEC` width
/// 4): a developer's clone approves a question and pushes `DEC-0001`; before
/// the next sync the hub confirms a question raised on it and commits
/// `DEC-100001`. The sync ends `ok` with a merge commit holding both files,
/// and `spec check --json` on the hub's checkout reports no `id-taken` and no
/// `id-width`. The developer pulls and approves another question:
/// `DEC-0002`. M1: the hub's `next_number` without the range (both
/// `DEC-0001`, `held`). M2: the terminal's corpus maximum counts numbers from
/// 100001 (`DEC-100002`).
#[test]
fn ac1_a_hubs_and_a_developers_records_merge_with_no_id_taken() {
    let mut world = World::forge("si-records");
    assert_state(&world.start(), "ok", "the start");

    let theirs = asked(
        &world.dev_home,
        &world.dev,
        &["RULE-FORGE-DECAY"],
        "Does the heat decay faster in the rain?",
    );
    assert_eq!(theirs, "PR-0001", "a terminal's PR- range");
    approved_at(&world.dev_home, &world.dev, &theirs);
    assert!(
        read_text(&world.dev, &dec_path("DEC-0001")).starts_with("---\nid: DEC-0001\n"),
        "the developer's record"
    );
    let theirs_commit = world.developer_pushes();

    let ours = asked(
        &world.setup.home,
        world.root(),
        &["RULE-QUENCH-MEDIUM"],
        "Does oil suit every blade?",
    );
    assert_eq!(ours, "PR-100001", "the hub's PR- range");
    let (document, status) = world.synced(|| world.confirm(&ours));
    assert_eq!(document["record_id"], json!("DEC-100001"), "{document}");
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    world.assert_merged(
        &status,
        &applied,
        &theirs_commit,
        &[dec_path("DEC-0001"), dec_path("DEC-100001")],
    );
    let reader = world.setup.scratch.home("reader");
    let codes = check_codes(&reader, &world.setup.cwd, world.root());
    assert_no_id_findings(&codes, "the hub's checkout after the merge");

    world.developer_pulls();
    assert!(world.dev.join(dec_path("DEC-100001")).is_file(), "pulled");
    let again = asked(
        &world.dev_home,
        &world.dev,
        &["MEC-BELLOWS"],
        "Do the bellows tire the smith?",
    );
    approved_at(&world.dev_home, &world.dev, &again);
    assert!(
        read_text(&world.dev, &dec_path("DEC-0002")).starts_with("---\nid: DEC-0002\n"),
        "the developer's next record"
    );
    assert!(!world.dev.join(dec_path("DEC-100002")).exists());
    let codes = check_codes(&reader, &world.setup.cwd, &world.dev);
    assert_no_id_findings(&codes, "the developer's clone");
}

// ------------------------------------------------------------------ AC-2

/// `propose_change`'s arguments for a new ruling `id` at its path.
fn create_args(id: &str) -> Value {
    json!({"kind": "create", "target": ruling_path(id), "base": null, "text": ruling(id),
        "rationale": "A second ruling.", "author_role": "writer"})
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-2 (card-press, `STY`
/// width 4, `STY-0001` in the tree): the hub's `/mcp` `propose_change`
/// create names `STY-0002` and is refused, an error result naming
/// `STY-100001`, nothing stored; resent with `STY-100001` it is stored and
/// confirmed. A developer creates and approves `STY-0002` and pushes; the
/// sync merges cleanly and `spec check --json` reports no `id-taken`. M3:
/// the propose step removed (the hub stores `STY-0002`).
#[test]
fn ac2_a_hubs_and_a_developers_creates_merge_with_no_id_taken() {
    let mut world = World::press("si-creates");
    assert_state(&world.start(), "ok", "the start");
    let mut session = Session::open(world.port(), "card-press", "ann");

    let refused = session.call("propose_change", create_args("STY-0002"));
    let result = result_of(&refused, "STY-0002");
    assert!(is_error(result), "refused: {result}");
    assert!(
        content_text(result).contains(&off_hub("STY-0002", 2, "STY-100001")),
        "{result}"
    );
    assert_eq!(result["structuredContent"]["id"], Value::Null, "{result}");
    // docs/features/spec-ids.md "Data", "Created IDs: the propose step":
    // `propose_change` of kind `amendment` alike, at its site's line.
    let refused = session.call(
        "propose_change",
        json!({"kind": "amendment", "target": "docs/spec/special-edition.md", "base": null,
            "text": amending("STY-0003"), "rationale": "A special edition.",
            "author_role": "writer"}),
    );
    let result = result_of(&refused, "the amendment");
    assert!(is_error(result), "refused: {result}");
    assert!(
        content_text(result).contains(&off_hub("STY-0003", 11, "STY-100001")),
        "{result}"
    );
    let inbox = spec_json(
        &world.setup.home,
        &world.setup.cwd,
        world.root(),
        &["inbox"],
    )
    .json();
    assert_eq!(inbox["proposals"], json!([]), "nothing stored: {inbox}");

    let stored = session.call("propose_change", create_args("STY-100001"));
    let result = result_of(&stored, "STY-100001");
    assert!(!is_error(result), "stored: {result}");
    let ours = result["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an ID: {result}"))
        .to_owned();
    assert_eq!(ours, "PR-100001");

    let theirs = raised_create(
        &world.dev_home,
        &world.dev,
        &ruling_path("STY-0002"),
        &ruling("STY-0002"),
    );
    assert_eq!(theirs, "PR-0001", "a terminal's PR- range");
    approved_at(&world.dev_home, &world.dev, &theirs);
    let theirs_commit = world.developer_pushes();

    let (document, status) = world.synced(|| world.confirm(&ours));
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    world.assert_merged(
        &status,
        &applied,
        &theirs_commit,
        &[ruling_path("STY-0002"), ruling_path("STY-100001")],
    );
    let reader = world.setup.scratch.home("reader");
    let codes = check_codes(&reader, &world.setup.cwd, world.root());
    assert_no_id_findings(&codes, "the hub's checkout after the merge");
}

// ------------------------------------------------------------------ AC-5 (a)

/// `id` staged by `ann` and confirmed in one click on `hub`: the POST's
/// reply, which must be a 200.
fn confirmed(hub: &Hub, id: &str) -> Value {
    let updated = hub.proposal(id)["updated_at"].clone();
    let stage = hub.post(
        "ann",
        &format!("proposals/{id}/decision"),
        &json!({"decision": "approve", "updated_at": updated}),
    );
    assert_eq!(stage.status, 200, "ann stages {id}: {}", stage.text());
    let route = format!("{P}/proposals/{id}/confirm");
    let read = hub.get("ann", &route).status(200).json();
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
    let reply = through(
        &hub.server,
        "POST",
        &route,
        &Proxied::write("ann"),
        body.to_string().as_bytes(),
    );
    assert_eq!(reply.status, 200, "ann confirms {id}: {}", reply.text());
    reply.json()
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-5 (a) (ADR-0012): a
/// terminal's dump holding an open create of `STY-0002` and an open
/// amendment whose new file defines `STY-0003`, restored with `spec
/// import-state` into an empty queue whose data directory holds the floor
/// (the hub's first start wrote it): on the hub each confirm answers 200 and
/// is `applied`, its file in the checkout. M9: the side judged inside
/// `NewIds::checked` (apply step 6 refuses: 409).
#[test]
fn ac5a_a_terminals_dump_restored_on_a_hub_confirms_and_applies() {
    let setup = Setup::new("si-restore", "card-press");
    append(&setup.root, TEAM);
    commit_all(&setup.scratch.git(), &setup.root, "the team");
    let terminal = setup.scratch.home("terminal");
    let created = raised_create(
        &terminal,
        &setup.root,
        &ruling_path("STY-0002"),
        &ruling("STY-0002"),
    );
    let special = "docs/spec/special-edition.md";
    let amendment = amending("STY-0003");
    let amended = raised_amendment(&terminal, &setup.root, special, &amendment);
    assert_eq!((created.as_str(), amended.as_str()), ("PR-0001", "PR-0002"));
    let (env, globals, git) = library(&terminal, &setup.root);
    let dump = setup.scratch.dir("dumps").join("terminal.jsonl");
    export_state(
        &env,
        &globals,
        &ExportStateRequest {
            out: Some(dump.clone()),
            now: utc_now(),
            git,
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));

    let home = setup.home.clone();
    drop(Server::start(
        &home,
        &setup.cwd,
        &hub_args(&[&setup.root], ORIGIN),
    ));
    assert_eq!(
        fs::read(data_dir(&home).join(FLOOR_FILE)).expect("the floor"),
        HUB_FLOOR
    );
    let (env, globals, _) = library(&home, &setup.root);
    let outcome = import_state(
        &env,
        &globals,
        &ImportStateRequest { file: dump },
        &mut |_: &str| true,
    )
    .unwrap_or_else(|error| panic!("import-state: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let server = Server::start(&home, &setup.cwd, &hub_args(&[&setup.root], ORIGIN));
    let hub = Hub { server, setup };

    for (id, path, bytes) in [
        (&created, ruling_path("STY-0002"), ruling("STY-0002")),
        (&amended, special.to_owned(), amendment.clone()),
    ] {
        let document = confirmed(&hub, id);
        assert_eq!(document["status"], json!("applied"), "{id}: {document}");
        assert_eq!(read_text(&hub.setup.root, &path), bytes, "{id}");
    }
}

// ------------------------------------------------------------------ AC-8

/// docs/features/spec-ids.md "Acceptance criteria" AC-8 (a), the wire:
/// `tools/list` over stdio shows `CHANGE_DESCRIPTION` with its sentence,
/// 2044 bytes, and over a hub's `/mcp` `HUB_CHANGE_DESCRIPTION` with its own,
/// 2040 bytes; each ASCII, the `target` line `(the whole file)`, the old
/// sentence gone. M13: a sentence dropped. Since
/// docs/features/intake-similar.md "Acceptance criteria" IS8 (its seven
/// edits): 2041 and 2037 bytes, the `target` line ending `.md path.`.
#[test]
fn ac8a_both_descriptions_name_the_range_on_the_wire() {
    let setup = Setup::new("si-texts", "card-press");
    append(&setup.root, TEAM);
    commit_all(&setup.scratch.git(), &setup.root, "the team");

    let root = setup.root.clone();
    let mut stdio = Mcp::start_with(
        &setup.scratch.home("stdio"),
        &setup.cwd,
        &[OsStr::new("--root"), root.as_os_str()],
    );
    let listed = stdio.request(1, "tools/list", json!({}));
    let list = listed.get("result").unwrap_or_else(|| panic!("{listed}"));
    let terminal = tool(list, "propose_change")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    stdio.finish();

    let hub = setup.start();
    let mut session = Session::open(hub.server.port, "card-press", "ann");
    let listed = session.tools();
    let on_hub = tool(&listed, "propose_change")["description"]
        .as_str()
        .expect("a description")
        .to_owned();

    for (label, description, sentence, bytes) in [
        ("stdio", &terminal, STDIO_SENTENCE, 2041),
        ("hub", &on_hub, HUB_SENTENCE, 2037),
    ] {
        assert!(description.is_ascii(), "{label}");
        assert_eq!(
            description.matches(sentence).count(),
            1,
            "{label}: {description}"
        );
        assert!(!description.contains(OLD_SENTENCE), "{label}");
        assert!(
            description
                .contains("(no alias, ID#SECTION or ID@rev), or a root-relative .md path.\n"),
            "{label}: {description}"
        );
        assert!(!description.contains("(the whole file)"), "{label}");
        assert!(
            !description.contains("(its document, the whole file)"),
            "{label}"
        );
        assert_eq!(description.len(), bytes, "{label}: {description}");
    }
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-8 (d) and
/// "Data", "The plugin": `sh scripts/deploy/tag-check.sh v0.29.1` exits 0 and prints
/// `0.29.1`; `.env.example` sets `SPECENGINE_VERSION=0.29.1`. Since
/// docs/features/flow-scope.md FS-6 (`get_flow`'s `stage`, the
/// orchestrator's MINOR of 2026-11-08) the plugin, the tag and the example
/// are 0.30.0; since docs/features/board-handoff.md (keys the card tools
/// take) 0.31.0; since card-tasks (RM-0092, `edit_card`'s `task`) 0.32.0;
/// since flow-scenario (RM-0094, `get_flow`'s `in_flow`) 0.33.0; since
/// doc-modal (RM-0095, DM-2: `get_node`'s link `title` and `end_kind`,
/// `get_flow`'s gap `titles`) 0.34.0; since departments (RM-0096, DP-4:
/// `list_cards`' flow `title`) 0.35.0; since card-questions (RM-0097, P1:
/// `card_id`, `why`, `items`) 0.36.0; since department-split (RM-0100, P1:
/// the tool `split_card`) 0.37.0; since task-mandate (RM-0104, P1: the
/// mandate's keys) 0.38.0; since card-context (RM-0108, CC8: `get_card`'s
/// `context`) 0.39.0; since node-history (RM-0035, NH11: `get_node`'s
/// `history`) 0.40.0; since upstream-signal (RM-0105, US-11: `get_card`'s
/// and `list_cards`' `upstream`) 0.41.0; since intake-similar (RM-0059,
/// IS11: `propose_change`'s `distinct_from`, `keywords` and `similar`)
/// 0.42.0.
#[test]
fn ac8d_the_release_tag_is_the_plugin_version() {
    let scratch = common::Scratch::new("si-tag");
    let elsewhere = scratch.dir("elsewhere");
    let script = repository_root().join("scripts/deploy/tag-check.sh");
    let env: Vec<(OsString, OsString)> = vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("HOME".into(), elsewhere.as_os_str().to_os_string()),
    ];
    let output = alarmed(
        Path::new("/bin/sh"),
        &[script.as_os_str(), OsStr::new("v0.42.0")],
        &env,
    )
    .current_dir(&elsewhere)
    .output()
    .expect("tag-check.sh runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0.42.0\n");
    let example = read_text(&repository_root(), "scripts/deploy/.env.example");
    assert_eq!(
        example
            .lines()
            .filter(|line| line.starts_with("SPECENGINE_VERSION="))
            .collect::<Vec<_>>(),
        ["SPECENGINE_VERSION=0.42.0"]
    );
}
