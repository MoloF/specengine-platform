//! docs/features/apply-outputs.md (RM-0072), a hub's own `PR-` range: O9 and
//! O10. `specengine-http --public-origin` stores `proposal-floor` (`100001`
//! and LF) once in its data directory at its first start, and every queue of
//! that data directory mints `PR-` no lower; a data directory where no hub
//! started holds none and mints as before. Through the real daemon over git
//! repositories of `fixtures/card-press` (an invented print shop with a board
//! and `[decision_records]`), a dump of 40 proposals made at a terminal and
//! restored by `spec import-state` (the library, consent yes), a bare
//! repository on disk where two clones meet.
//!
//! - O9: the floor's bytes after a hub's first start; the dump imported there;
//!   a second start leaves its bytes and mtime; a departing accept through the
//!   hub stores PR-100001, then `spec propose` on the host PR-100002; the dump
//!   in a fresh data directory with no hub mints PR-0041; a local start writes
//!   no floor. M: `next_id` without the floor; the floor on every queue.
//! - O10: both queues from that dump, the hub's confirm of PR-100001 in clone
//!   A, the terminal's PR-0041 in clone B, A's branch merged into B's through
//!   the bare repository: `spec reject PR-0041` rejects. M: no floor (both
//!   mint PR-0041 and the hub's commit blocks the reject).
//! - O15 (iteration 2), the daemon's half: a hub's start beside leftover
//!   siblings of the floor listens and mints from the floor. The write's
//!   steps are store `floor.rs`'s unit tests against `store_floor_with`.
//!
//! Beyond the criteria, from "Rules and edge cases": an invalid floor stops a
//! hub before it listens (exit 2, the line naming the file) and a mint there
//! stores nothing (`spec propose` exit 2, the MCP tool's error, the same
//! text); a stored floor is read as written and never rewritten.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::hub::{Hub, ORIGIN, P, Proxied, Setup, hub_args, through};
use common::{
    Mcp, Server, copy_file, data_dir, fixture, product_env, propose_update, read_text, run_product,
    spec_bin, spec_json, write,
};
use serde_json::{Value, json};
use specengine_cli::{
    Env, Exit, ExportStateRequest, GitEnv, Globals, ImportStateRequest, ProposeRequest,
    ProposedText, RejectRequest, export_state, import_state, propose, reject, utc_now,
};

/// The floor a hub's first start writes ("Data").
const FLOOR: &[u8] = b"100001\n";

/// The line of an invalid floor, after `<data dir>/proposal-floor: `.
const NOT_A_FLOOR: &str = "not a proposal floor (a decimal number from 1, then LF)";

/// The proposals of the dump.
const DUMPED: u64 = 40;

/// The print shop's team (hub-confirm's): `ann` the owner, `mira` leading the
/// front, `tomas` a member.
const TEAM: &str = r#"
[zones]
front = ["docs/spec/sht-front.md"]

[team.ann]
role = "owner"
email = "ann@example.org"

[team.mira]
role = "lead"
zones = ["front"]
email = "mira@example.org"

[team.tomas]
role = "member"
email = "tomas@example.org"
"#;

/// Where the print shop's records go.
const RECORDS: &str = r#"
[decision_records]
prefix = "STY"
dir = "docs/records/STY"
template = "templates/ruling.md"
"#;

// ---------------------------------------------------------------- helpers

/// The print shop with its team, `[decision_records]` and a template
/// (`spec-a`'s), committed, so a departing accept's item can be confirmed.
fn press(name: &str) -> Setup {
    let setup = Setup::new(name, "card-press");
    let mut config = read_text(&setup.root, "specengine.toml");
    config.push_str(TEAM);
    config.push_str(RECORDS);
    write(&setup.root, "specengine.toml", config);
    fs::create_dir_all(setup.root.join("templates")).expect("templates");
    copy_file(
        fixture("spec-a").join("templates/decision.md"),
        setup.root.join("templates/ruling.md"),
    );
    let git = setup.scratch.git();
    git.run(&setup.root, &["add", "-A"]);
    git.run(
        &setup.root,
        &["commit", "-q", "-m", "the team and its records"],
    );
    setup
}

/// The library's calls in `root` under `home`, as a terminal there makes them.
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

/// `<data dir>/proposal-floor` under `home`.
fn floor_of(home: &Path) -> PathBuf {
    data_dir(home).join("proposal-floor")
}

/// The floor's bytes and mtime.
fn floor_stamp(home: &Path) -> (Vec<u8>, SystemTime) {
    let path = floor_of(home);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let meta = fs::symlink_metadata(&path).expect("the floor's metadata");
    assert!(meta.file_type().is_file(), "a regular file");
    (bytes, meta.modified().expect("mtime"))
}

/// The floor's mtime set to a time long past, so a rewrite shows.
fn aged(home: &Path) {
    let file = fs::File::options()
        .write(true)
        .open(floor_of(home))
        .expect("the floor opens");
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000))
        .expect("an old mtime");
}

/// A hub on `setup`'s root under `home`, listening.
fn hub_on(home: &Path, setup: &Setup) -> Server {
    Server::start(home, &setup.cwd, &hub_args(&[&setup.root], ORIGIN))
}

/// `target`'s text as `spec show` reads it in `root` under `home`, `from`
/// become `to`, raised by `spec propose update` there: the ID.
fn reworded(home: &Path, cwd: &Path, root: &Path, target: &str, from: &str, to: &str) -> String {
    let shown = spec_json(home, cwd, root, &["show", target]).json();
    let text = shown["nodes"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{target}: {shown}"));
    assert!(text.contains(from), "{target} holds no {from:?}: {text}");
    propose_update(home, cwd, root, target, &text.replacen(from, to, 1))
}

/// A dump of PR-0001 to PR-0040, 40 updates of `SHT-BACK` raised at a
/// terminal (a `HOME` of its own) on `setup`'s root, exported by `spec export
/// state`: the file, outside every worktree.
fn forty(setup: &Setup) -> PathBuf {
    let home = setup.scratch.home("terminal-source");
    let shown = spec_json(&home, &setup.cwd, &setup.root, &["show", "SHT-BACK"]).json();
    let base = shown["nodes"][0]["span_hash"]
        .as_str()
        .expect("span_hash")
        .to_owned();
    let text = shown["nodes"][0]["text"].as_str().expect("text").to_owned();
    assert!(text.contains("the tide table"), "{text}");
    let (env, globals, git) = library(&home, &setup.root);
    for n in 1..=DUMPED {
        let outcome = propose(
            &env,
            &globals,
            &ProposeRequest {
                target: "SHT-BACK".to_owned(),
                base: base.clone(),
                text: ProposedText::Given(
                    text.replacen("the tide table", &format!("tide table {n}"), 1)
                        .into_bytes(),
                ),
                rationale: format!("Edition {n}."),
                author_role: Some("spec-writer".to_owned()),
                author_model: None,
                run: None,
                now: utc_now(),
                git: git.clone(),
            },
        )
        .unwrap_or_else(|error| panic!("propose {n}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "propose {n}: {outcome:?}");
        assert_eq!(
            outcome.document.id.as_deref(),
            Some(format!("PR-{n:04}").as_str())
        );
    }
    let file = setup.scratch.dir("dumps").join("forty.jsonl");
    let outcome = export_state(
        &env,
        &globals,
        &ExportStateRequest {
            out: Some(file.clone()),
            now: utc_now(),
            git,
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
    assert_eq!(outcome.proposals, DUMPED, "{outcome:?}");
    file
}

/// `spec import-state file` into the data directory of `home` from `root`,
/// consent yes: it must restore the dump's 40 proposals.
fn imported(home: &Path, root: &Path, file: &Path) {
    let (env, globals, _) = library(home, root);
    let mut asked = 0;
    let outcome = import_state(
        &env,
        &globals,
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut |_: &str| {
            asked += 1;
            true
        },
    )
    .unwrap_or_else(|error| panic!("import-state into {}: {error}", home.display()));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(outcome.proposals, DUMPED, "{outcome:?}");
    assert_eq!(asked, 1, "one question");
}

/// A `plates` card on `target`, `ana` its assignee and `tomas` its reviewer,
/// moved to `accept.from`: its ID.
fn ready_card(hub: &Hub, title: &str, target: &str) -> String {
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
    hub.done(
        "ana",
        &format!("cards/{id}/move"),
        &json!({"to": "checked"}),
    );
    id
}

/// A departing accept by `tomas` of `card` on `node`: the ID of the one item
/// it stores.
fn departed(hub: &Hub, card: &str, node: &str) -> String {
    let revision = hub.show(card)["revision"]
        .as_str()
        .expect("a revision")
        .to_owned();
    let seen = hub.events().len();
    hub.done(
        "tomas",
        &format!("cards/{card}/accept"),
        &json!({
            "departs": [node],
            "summary": "The result left the brief.",
            "observed": "The lead runs below the fold.",
            "documented": "The lead runs above the fold.",
            "revision": revision,
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

/// `id`'s choice staged by `login`, against the row as it stands.
fn staged(hub: &Hub, login: &str, id: &str, choice: &Value) {
    let mut body = choice.clone();
    body["updated_at"] = hub.proposal(id)["updated_at"].clone();
    let reply = through(
        &hub.server,
        "POST",
        &format!("{P}/proposals/{id}/decision"),
        &Proxied::write(login),
        body.to_string().as_bytes(),
    );
    assert_eq!(reply.status, 200, "{login} stages {id}: {}", reply.text());
}

/// One click by `login`: the confirm's read and its POST, each a 200: the
/// review document.
fn confirmed(hub: &Hub, login: &str, id: &str) -> Value {
    let route = format!("{P}/proposals/{id}/confirm");
    let read = hub.get(login, &route).status(200).json();
    let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
    let reply = through(
        &hub.server,
        "POST",
        &route,
        &Proxied::write(login),
        body.to_string().as_bytes(),
    );
    assert_eq!(reply.status, 200, "{login} confirms {id}: {}", reply.text());
    reply.json()
}

/// The values of `key` in `commit`'s message in `root`.
fn trailers(setup: &Setup, root: &Path, commit: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key}: ");
    setup
        .scratch
        .git()
        .run(root, &["log", "-1", "--format=%B", commit])
        .lines()
        .filter_map(|line| line.strip_prefix(prefix.as_str()))
        .map(str::to_owned)
        .collect()
}

fn head_of(setup: &Setup, root: &Path) -> String {
    setup
        .scratch
        .git()
        .run(root, &["rev-parse", "HEAD"])
        .trim()
        .to_owned()
}

// ------------------------------------------------------------------- O9

/// O9: after `specengine-http --public-origin` has started on a data
/// directory, `proposal-floor` reads `100001` and LF, a regular file; the dump
/// of PR-0001..PR-0040 imports there (step 3 judges tables alone); a second
/// hub start leaves the file's bytes and mtime; a departing accept through
/// that hub stores PR-100001, then `spec propose` on the host, the same data
/// directory, PR-100002. The dump in a fresh data directory where no hub
/// started: the next `spec propose` is PR-0041 and no floor exists. A local
/// daemon's start writes none. M: `next_id` without the floor; the floor on
/// every queue.
#[test]
fn o9_a_hub_stores_its_floor_once_and_its_data_directory_mints_from_it() {
    let setup = press("ids-floor");
    let dump = forty(&setup);
    let home = setup.home.clone();
    let root = setup.root.clone();
    let cwd = setup.cwd.clone();

    let first = hub_on(&home, &setup);
    assert_eq!(floor_stamp(&home).0, FLOOR, "the first start's floor");
    drop(first);
    imported(&home, &root, &dump);
    aged(&home);
    let before = floor_stamp(&home);
    let hub = Hub {
        server: hub_on(&home, &setup),
        setup,
    };
    assert_eq!(
        floor_stamp(&home),
        before,
        "a second start rewrites nothing"
    );

    let card = ready_card(&hub, "Lead story", "SHT-FRONT");
    let item = departed(&hub, &card, "SHT-FRONT");
    let host = reworded(
        &home,
        &cwd,
        &root,
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    assert_eq!(
        (item.as_str(), host.as_str()),
        ("PR-100001", "PR-100002"),
        "the hub's accept, then the host's propose"
    );
    assert_eq!(floor_stamp(&home), before, "minting rewrites nothing");

    let fresh = hub.setup.scratch.home("fresh");
    imported(&fresh, &root, &dump);
    let next = reworded(
        &fresh,
        &cwd,
        &root,
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the tide",
    );
    assert_eq!(next, "PR-0041", "no hub: the highest restored + 1");
    assert!(!floor_of(&fresh).exists(), "no floor without a hub");

    let local = hub.setup.scratch.home("local");
    let server = Server::serve(&local, &cwd, &[root.as_path()]);
    assert!(
        data_dir(&local).is_dir(),
        "the local start made its directory"
    );
    assert!(!floor_of(&local).exists(), "a local start writes no floor");
    drop(server);
    assert!(!floor_of(&local).exists());
}

// ------------------------------------------------------------------ O10

/// O10: both queues restored from that dump, the hub's (`specengine-http
/// --public-origin`, started once before) over clone A and a terminal's over
/// clone B of one bare repository. The hub's departing accept is staged and
/// confirmed by its `owner` in A, A's branch pushed; the terminal raises its
/// next proposal in B; A's branch is merged into B's from the bare repository.
/// `spec reject` of the terminal's proposal: exit 0, `rejected`; the two IDs
/// are PR-100001 and PR-0041. M: no floor (both mint PR-0041 and the hub's
/// commit blocks the reject).
#[test]
fn o10_a_hubs_commit_on_a_shared_branch_never_blocks_a_terminals_reject() {
    let setup = press("ids-two-queues");
    let dump = forty(&setup);
    let git = setup.scratch.git();
    let a = setup.root.clone();
    let cwd = setup.cwd.clone();
    let bare = setup.scratch.dir("origin.git");
    let bare_path = bare.to_str().expect("UTF-8 path").to_owned();
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
    git.run(&a, &["remote", "add", "origin", &bare_path]);
    git.run(&a, &["push", "-q", "-u", "origin", "main"]);
    let b = setup.scratch.join("b");
    git.run(
        setup.scratch.path(),
        &["clone", "-q", &bare_path, b.to_str().expect("UTF-8 path")],
    );
    let b = fs::canonicalize(b).expect("clone B");
    git.quiet(&b);
    git.run(&b, &["config", "user.name", "Terminal"]);
    git.run(&b, &["config", "user.email", "terminal@example.invalid"]);

    let terminal = setup.scratch.home("terminal");
    imported(&terminal, &b, &dump);
    let home = setup.home.clone();
    drop(hub_on(&home, &setup));
    imported(&home, &a, &dump);
    let hub = Hub {
        server: hub_on(&home, &setup),
        setup,
    };

    let card = ready_card(&hub, "Lead story", "SHT-FRONT");
    let item = departed(&hub, &card, "SHT-FRONT");
    staged(
        &hub,
        "tomas",
        &item,
        &json!({"decision": "approve", "option": 1}),
    );
    let document = confirmed(&hub, "ann", &item);
    assert_eq!(document["status"], json!("applied"), "{document}");
    let commit = head_of(&hub.setup, &a);
    assert_eq!(
        trailers(&hub.setup, &a, &commit, "Proposal"),
        std::slice::from_ref(&item)
    );
    git.run(&a, &["push", "-q", "origin", "main"]);

    let id = reworded(
        &terminal,
        &cwd,
        &b,
        "SHT-BACK",
        "the tide table",
        "the moon table",
    );
    git.run(&b, &["pull", "-q", "--no-rebase", "origin", "main"]);
    assert_eq!(
        head_of(&hub.setup, &b),
        commit,
        "A's commit is on B's branch"
    );

    let (env, globals, terminal_git) = library(&terminal, &b);
    let outcome = reject(
        &env,
        &globals,
        &RejectRequest {
            id: id.clone(),
            reason: Some("Not this week.".to_owned()),
            now: utc_now(),
            git: terminal_git,
        },
        &mut |_: &str| true,
    )
    .unwrap_or_else(|error| panic!("reject {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "reject {id}: {outcome:?}");
    let review = spec_json(&terminal, &cwd, &b, &["review", &id]).json();
    assert_eq!(review["status"], json!("rejected"), "{review}");
    assert_eq!(
        (item.as_str(), id.as_str()),
        ("PR-100001", "PR-0041"),
        "the hub's ID and the terminal's"
    );
}

// --------------------------------------------------- beyond the criteria

/// "Rules and edge cases": a floor that is no decimal number from 1 then LF
/// stops a hub before it listens, exit 2, `specengine-http: <data
/// dir>/proposal-floor: not a proposal floor (a decimal number from 1, then
/// LF)`, the file as it was; `spec propose` there exits 2 with the same text
/// and the MCP tool answers an error with it, storing nothing: once the file
/// holds `100001` the next mint is PR-100001. M: a guessed floor; the file
/// rewritten.
#[test]
fn an_invalid_floor_stops_a_hub_before_it_listens_and_a_mint_stores_nothing() {
    let setup = press("ids-bad-floor");
    let bad: [&[u8]; 10] = [
        b"",
        b"0\n",
        b"0100001\n",
        b"100001",
        b"+100001\n",
        b" 100001\n",
        b"100001\n\n",
        b"100001\r\n",
        b"1e5\n",
        b"18446744073709551616\n",
    ];
    let mut checked = 0;
    for (n, bytes) in bad.iter().enumerate() {
        let home = setup.scratch.home(&format!("bad-{n}"));
        let data = data_dir(&home);
        fs::create_dir_all(&data).expect("the data directory");
        fs::write(floor_of(&home), bytes).expect("the floor");
        let line = format!("{}/proposal-floor: {NOT_A_FLOOR}", data.display());
        let context = format!("{:?}", String::from_utf8_lossy(bytes));

        let refused = Server::try_start(&home, &setup.cwd, &hub_args(&[&setup.root], ORIGIN))
            .err()
            .unwrap_or_else(|| panic!("{context}: the hub listened"));
        assert_eq!(refused.code, Some(2), "{context}: {refused:?}");
        assert!(
            refused.stderr.contains(&format!("specengine-http: {line}")),
            "{context}: {}",
            refused.stderr
        );
        assert!(
            !refused.stdout.contains("listening"),
            "{context}: {}",
            refused.stdout
        );
        assert_eq!(
            fs::read(floor_of(&home)).expect("floor"),
            *bytes,
            "{context}"
        );

        let root = setup.root.to_str().expect("UTF-8 root");
        let run = run_product(
            &spec_bin(),
            &home,
            &setup.cwd,
            &[
                "--root",
                root,
                "propose",
                "question",
                "SHT-BACK",
                "--text",
                "Does the tide table keep its rule?",
                "--working-answer",
                "keep it",
                "--price-of-other",
                "a new rule",
            ],
            b"",
        );
        assert_eq!(run.code, 2, "{context}: {}", run.show());
        assert!(run.stderr.contains(&line), "{context}: {}", run.show());
        assert!(!run.stdout.contains("PR-"), "{context}: {}", run.show());
        checked += 1;

        if n == 1 {
            let mut mcp = Mcp::start(&home, &setup.root);
            let reply = mcp.request(
                1,
                "tools/call",
                json!({"name": "ask_question", "arguments": {
                    "node_ids": ["SHT-BACK"], "text": "Does the tide table keep its rule?",
                    "working_answer": "keep it", "price_of_other": "a new rule",
                    "author_role": "writer"}}),
            );
            mcp.finish();
            assert_eq!(reply["result"]["isError"], json!(true), "{reply}");
            assert!(reply.to_string().contains(NOT_A_FLOOR), "{reply}");

            fs::write(floor_of(&home), FLOOR).expect("a valid floor");
            let id = reworded(
                &home,
                &setup.cwd,
                &setup.root,
                "SHT-BACK",
                "the tide table",
                "the moon table",
            );
            assert_eq!(id, "PR-100001", "nothing was stored before");
        }
    }
    assert_eq!(checked, bad.len());
}

/// "Data": readers use the stored number. A floor written `500` in a data
/// directory where no hub started: `spec propose` mints PR-0500; a hub's start
/// there keeps the file byte for byte and its mtime, and its departing accept
/// mints PR-0501. M: the constant in place of the stored number; the floor
/// rewritten at start.
#[test]
fn a_stored_floor_is_read_as_written_and_never_rewritten() {
    let setup = press("ids-stored-floor");
    let home = setup.home.clone();
    fs::create_dir_all(data_dir(&home)).expect("the data directory");
    fs::write(floor_of(&home), b"500\n").expect("the floor");
    let first = reworded(
        &home,
        &setup.cwd,
        &setup.root,
        "SHT-BACK",
        "the tide table",
        "the moon table",
    );
    assert_eq!(first, "PR-0500");
    aged(&home);
    let before = floor_stamp(&home);
    let hub = Hub {
        server: hub_on(&home, &setup),
        setup,
    };
    assert_eq!(floor_stamp(&home), before, "kept byte for byte, mtime too");
    let card = ready_card(&hub, "Lead story", "SHT-FRONT");
    assert_eq!(departed(&hub, &card, "SHT-FRONT"), "PR-0501");
}

// ------------------------------------------------------------------ O15

/// The floor's sibling name prefix ("Data": `.proposal-floor.specengine-<pid>-<n>.tmp`).
const SIBLING: &str = ".proposal-floor.specengine-";

/// O15's daemon half (the write's steps are store `floor.rs`'s unit tests, its
/// seam `store_floor_with`): leftover siblings a crash would leave, one partial
/// and one empty, are never read as the floor and do not stop a start: the hub
/// listens with the floor `100001` and LF, and the host's next mint is
/// PR-100001. M: a sibling taken for the floor.
#[test]
fn o15_a_hub_start_beside_leftover_siblings_listens_and_mints_from_the_floor() {
    let setup = Setup::new("ids-floor-leftover", "card-press");
    let home = setup.home.clone();
    let data = data_dir(&home);
    fs::create_dir_all(&data).expect("the data directory");
    fs::write(data.join(format!("{SIBLING}1-0.tmp")), b"42\n").expect("a sibling");
    fs::write(data.join(format!("{SIBLING}2-0.tmp")), b"").expect("an empty sibling");
    let server = hub_on(&home, &setup);
    assert_eq!(fs::read(floor_of(&home)).expect("the floor"), FLOOR);
    drop(server);
    let id = reworded(
        &home,
        &setup.cwd,
        &setup.root,
        "SHT-BACK",
        "the tide table",
        "the moon table",
    );
    assert_eq!(id, "PR-100001");
}
