//! docs/features/write-cost.md (RM-0125) WC28: ten logins at once on one hub,
//! the owner's constraint. Over `fixtures/spec-a` (a game with
//! `[decision_records]`) opted in by a committed `[team]`: `ann` its owner and
//! nine leads whose zone covers `docs/**`. The real daemon runs with
//! `--push` (each confirm's commit starts a sync, which holds the same turns:
//! the "sync ticks"), a bare `origin`, and `--slow-ms 0`, so every turn's
//! `held` line is on its stderr in the order the turns were taken.
//!
//! The scenario, drawn from a fixed seed: each login raises one of ten items
//! over `/mcp` (five updates by `propose_change`, five questions by
//! `ask_question`), all at once; each item is staged by a drawn login (its row
//! read, then the stage); each item's confirm is read by a drawn login, two
//! items (an update and a question) by two logins, all at once; then every
//! read is posted, all at once. Every question's read previews the same next
//! record ID, so all but the first question's POST answer 0.47.0's 409
//! "moved since its question was read": those logins read again and post
//! again, round after round, until every item is decided.
//!
//! What it shows:
//! - the recorded turn order (the `held` lines, mapped to the requests),
//!   replayed one call at a time on a copy (a second hub over the same
//!   commit), gives the same results: every status and body, refusal texts
//!   included, the applied set, the commits' trees and messages (trailers),
//!   the record IDs; times, dates, commit IDs and the scratch roots masked;
//! - each committing POST's commit has, as its parent, the branch tip at its
//!   turn's start: the commits on `main` are the committing POSTs in turn
//!   order, a straight line;
//! - a read after another login's commit shows it: each question's read
//!   previews the record ID that follows the records committed before its turn;
//! - every committed record equals the record its POST's question showed;
//! - after the last sync, `origin`'s `main` is the hub's;
//! - a 120 s watchdog over the concurrent part fails a deadlock instead of
//!   hanging the run.
//!
//! It passes on 0.47.0, which is correct and only slow. Its mutations ("a key
//! without the login", "the memo outside the turn", "the POST using the GET's
//! question") are the developer's memo's and are tried once it lands.

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use common::hub::{ORIGIN, Proxied, Setup, hub_args};
use common::mcp_http::Stateless;
use common::{HTTP_TIMEOUT, Server, http_exchange, product_env, read_text, spec_json, write};
use regex::Regex;
use serde_json::{Value, json};

/// The game's slug, and its page routes' base.
const SLUG: &str = "lantern-keep";
const BASE: &str = "/api/projects/lantern-keep";

const OWNER: &str = "ann";
const LEADS: [&str; 9] = [
    "bea", "cal", "dan", "eva", "fin", "gil", "hana", "ivo", "jae",
];

/// The concurrent part's bound: a deadlock fails the test here.
const WATCHDOG: Duration = Duration::from_secs(120);
/// How long a turn's `held` line may take to reach the test.
const LINE_WAIT: Duration = Duration::from_secs(20);
/// Read-and-post rounds after the first, at most.
const ROUNDS: usize = 8;

/// 0.47.0's 409 line when a question moved after its read.
const MOVED: &str = "moved since its question was read";

/// The five updates: a section each, two of them in one file.
const UPDATES: [&str; 5] = [
    "RULE-CORE-LOOP",
    "RULE-MOVE-SPEEDS",
    "EDGE-SPRINT-EMPTY",
    "RULE-STAM-REGEN",
    "EDGE-STAM-ZERO",
];

/// The five questions: a target and a text each, distinct enough that none
/// meets another as similar.
const QUESTIONS: [(&str, &str); 5] = [
    ("DOM-GAME", "Does the lantern burn through a whole night?"),
    (
        "MEC-SPRINT",
        "May a sprint start while carrying the lantern?",
    ),
    ("RULE-MOVE-SPEEDS", "Is walking slower on the outer walls?"),
    ("DOM-MOVEMENT", "Can the keeper climb the outer walls?"),
    (
        "RULE-CORE-LOOP",
        "Does a failed night restart the whole loop?",
    ),
];

fn logins() -> Vec<&'static str> {
    let mut all = vec![OWNER];
    all.extend(LEADS);
    all
}

/// The team: `ann` owns, every lead's zone covers `docs/**`.
fn team() -> String {
    let mut text = format!(
        "\n[zones]\nall = [\"docs/**\"]\n\n[team.{OWNER}]\nrole = \"owner\"\nemail = \"{OWNER}@example.org\"\n"
    );
    for lead in LEADS {
        text.push_str(&format!(
            "\n[team.{lead}]\nrole = \"lead\"\nzones = [\"all\"]\nemail = \"{lead}@example.org\"\n"
        ));
    }
    text
}

// ---------------------------------------------------------------- a world

/// A hub over spec-a with the team committed and a bare `origin` its branch
/// tracks.
struct World {
    server: Server,
    setup: Setup,
    bare: PathBuf,
    /// `HEAD` once the team is in: every world's is the same commit.
    base: String,
}

impl World {
    fn new(label: &str) -> Self {
        let setup = Setup::new(label, "spec-a");
        let git = setup.scratch.git();
        let mut config = read_text(&setup.root, "specengine.toml");
        config.push_str(&team());
        write(&setup.root, "specengine.toml", config);
        git.run(&setup.root, &["add", "-A"]);
        git.run(&setup.root, &["commit", "-q", "-m", "the team"]);
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
        git.quiet(&bare);
        let url = bare.to_str().expect("a UTF-8 path").to_owned();
        git.run(&setup.root, &["remote", "add", "origin", &url]);
        git.run(&setup.root, &["push", "-q", "-u", "origin", "main"]);
        let base = git
            .run(&setup.root, &["rev-parse", "HEAD"])
            .trim()
            .to_owned();
        let mut args: Vec<&OsStr> = hub_args(&[setup.root.as_path()], ORIGIN);
        for flag in ["--push", "--push-deadline", "20", "--slow-ms", "0"] {
            args.push(OsStr::new(flag));
        }
        let server = Server::try_start_env(&product_env(&setup.home), &setup.cwd, &args)
            .unwrap_or_else(|refused| panic!("the hub did not start: {refused:?}"));
        Self {
            server,
            setup,
            bare,
            base,
        }
    }

    fn port(&self) -> u16 {
        self.server.port
    }

    fn git(&self, dir: &Path, args: &[&str]) -> String {
        self.setup.scratch.git().run(dir, args)
    }

    /// The commits on `main` after the base, oldest first.
    fn commits(&self) -> Vec<String> {
        self.git(
            &self.setup.root,
            &["rev-list", "--reverse", &format!("{}..main", self.base)],
        )
        .lines()
        .map(str::to_owned)
        .collect()
    }

    /// Each commit after the base, oldest first: its tree and message.
    fn trees_and_messages(&self) -> Vec<(String, String)> {
        self.commits()
            .iter()
            .map(|commit| {
                let tree = self.git(
                    &self.setup.root,
                    &["rev-parse", &format!("{commit}^{{tree}}")],
                );
                let message = self.git(&self.setup.root, &["log", "-1", "--format=%B", commit]);
                (tree.trim().to_owned(), message)
            })
            .collect()
    }

    /// `commit`'s parents.
    fn parents(&self, commit: &str) -> Vec<String> {
        self.git(
            &self.setup.root,
            &["rev-list", "--parents", "-n", "1", commit],
        )
        .split_whitespace()
        .skip(1)
        .map(str::to_owned)
        .collect()
    }

    /// The `held` turn lines so far, in the order they were written.
    fn held(&self) -> Vec<Held> {
        held_lines(&self.server.stderr())
    }

    /// The masks of this world's paths.
    fn paths(&self) -> Vec<(String, &'static str)> {
        let mut out = Vec::new();
        for (path, name) in [
            (&self.setup.root, "<root>"),
            (&self.setup.home, "<home>"),
            (&self.bare, "<origin>"),
        ] {
            out.push((path.display().to_string(), name));
            if let Ok(canonical) = fs::canonicalize(path) {
                out.push((canonical.display().to_string(), name));
            }
        }
        out.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
        out
    }

    /// Waits until the last sync pushed everything: `ahead` 0, state `ok`.
    fn synced(&self) -> Value {
        let started = Instant::now();
        loop {
            let reply = http_exchange(
                self.port(),
                &Proxied::fetch(OWNER).bytes("GET", &format!("{BASE}/sync"), b""),
                HTTP_TIMEOUT,
            );
            let status: Value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
            if reply.status == 200 && status["ahead"] == json!(0) && status["state"] == json!("ok")
            {
                return status;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "no sync pushed everything within 60 s: {} {}",
                reply.status,
                reply.text()
            );
            thread::sleep(Duration::from_millis(200));
        }
    }
}

// ------------------------------------------------------------ the items

/// An item: the `/mcp` tool and its arguments.
#[derive(Debug, Clone)]
struct Item {
    tool: &'static str,
    arguments: Value,
    question: bool,
}

/// The ten items over `world`'s spans (every world's are the same bytes).
fn items(world: &World) -> Vec<Item> {
    let setup = &world.setup;
    let mut out = Vec::new();
    for (index, target) in UPDATES.iter().enumerate() {
        let shown = spec_json(&setup.home, &setup.cwd, &setup.root, &["show", target]).json();
        let node = &shown["nodes"][0];
        let text = node["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{target}: {shown}"));
        let mut text = text.to_owned();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!(
            "\nThe write-cost probe {index} holds here; see [its note](zz-wc28-{index}.md).\n"
        ));
        out.push(Item {
            tool: "propose_change",
            arguments: json!({
                "kind": "update",
                "target": target,
                "base": node["span_hash"],
                "text": text,
                "rationale": format!("Probe {target}."),
                "author_role": "spec-writer",
                "author_model": "claude-opus-5-5",
            }),
            question: false,
        });
    }
    for (target, text) in QUESTIONS {
        out.push(Item {
            tool: "ask_question",
            arguments: json!({
                "node_ids": [target],
                "text": text,
                "working_answer": "Keep it as written.",
                "price_of_other": "A new rule case.",
                "author_role": "developer",
                "author_model": "claude-opus-5-5",
            }),
            question: true,
        });
    }
    out
}

// ------------------------------------------------------------ the calls

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    /// The item raised over `/mcp`.
    Raise(usize),
    /// `GET …/proposals/<id>`: the row, for its `updated_at`.
    Row(usize),
    /// `POST …/proposals/<id>/decision`: an approve staged.
    Stage(usize),
    /// `GET …/proposals/<id>/confirm`.
    Read(usize),
    /// `POST …/proposals/<id>/confirm` of the login's last read.
    Write(usize),
}

impl Call {
    fn item(&self) -> usize {
        match self {
            Self::Raise(item)
            | Self::Row(item)
            | Self::Stage(item)
            | Self::Read(item)
            | Self::Write(item) => *item,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    login: &'static str,
    call: Call,
}

/// One answered call.
#[derive(Debug, Clone)]
struct Done {
    step: Step,
    status: u16,
    body: String,
    json: Value,
}

/// What a login's later calls need of the answers so far.
#[derive(Debug, Clone, Default)]
struct Seen {
    ids: BTreeMap<usize, String>,
    rows: BTreeMap<(&'static str, usize), Value>,
    reads: BTreeMap<(&'static str, usize), Value>,
}

impl Seen {
    fn note(&mut self, done: &Done) {
        let key = (done.step.login, done.step.call.item());
        match done.step.call {
            Call::Raise(item) => {
                if let Some(id) = done.json["result"]["structuredContent"]["id"].as_str() {
                    self.ids.insert(item, id.to_owned());
                }
            }
            Call::Row(_) if done.status == 200 => {
                self.rows.insert(key, done.json["updated_at"].clone());
            }
            Call::Read(_) if done.status == 200 => {
                self.reads.insert(key, done.json.clone());
            }
            _ => {}
        }
    }

    fn id(&self, item: usize) -> &str {
        self.ids
            .get(&item)
            .unwrap_or_else(|| panic!("item {item} was not stored"))
    }
}

/// `step` sent to the daemon on `port`.
fn send(port: u16, step: &Step, seen: &Seen, items: &[Item]) -> Done {
    let login = step.login;
    let page = |method: &str, path: String, body: Option<Value>| {
        let bytes = match body {
            Some(body) => Proxied::write(login).bytes(method, &path, body.to_string().as_bytes()),
            None => Proxied::fetch(login).bytes(method, &path, b""),
        };
        let reply = http_exchange(port, &bytes, HTTP_TIMEOUT);
        Done {
            step: step.clone(),
            status: reply.status,
            body: reply.text().to_owned(),
            json: serde_json::from_slice(&reply.body).unwrap_or(Value::Null),
        }
    };
    match &step.call {
        Call::Raise(item) => {
            let item = &items[*item];
            let mut client = Stateless::new(port, SLUG, login);
            let (id, answer) = client.send(
                "tools/call",
                json!({"name": item.tool, "arguments": item.arguments}),
            );
            let response = answer.expect_response(id, item.tool);
            Done {
                step: step.clone(),
                status: answer.status(),
                body: response.to_string(),
                json: response,
            }
        }
        Call::Row(item) => page("GET", format!("{BASE}/proposals/{}", seen.id(*item)), None),
        Call::Stage(item) => {
            let updated_at = seen.rows[&(login, *item)].clone();
            page(
                "POST",
                format!("{BASE}/proposals/{}/decision", seen.id(*item)),
                Some(json!({"decision": "approve", "updated_at": updated_at})),
            )
        }
        Call::Read(item) => page(
            "GET",
            format!("{BASE}/proposals/{}/confirm", seen.id(*item)),
            None,
        ),
        Call::Write(item) => {
            let read = &seen.reads[&(login, *item)];
            page(
                "POST",
                format!("{BASE}/proposals/{}/confirm", seen.id(*item)),
                Some(json!({"question": read["question"], "updated_at": read["updated_at"]})),
            )
        }
    }
}

// ------------------------------------------------------- the turn lines

/// A turn's `held` line: who held it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    by: String,
    id: String,
    tool: String,
}

fn held_lines(stderr: &str) -> Vec<Held> {
    let line = Regex::new(
        r"^specengine-http: slow turn at=\S+ project=(\S+) by=(\S+) id=(\S+) tool=(\S+) state=held ",
    )
    .expect("a regex");
    stderr
        .lines()
        .filter_map(|text| line.captures(text))
        .filter(|captures| &captures[1] == SLUG)
        .map(|captures| Held {
            by: captures[2].to_owned(),
            id: captures[3].to_owned(),
            tool: captures[4].to_owned(),
        })
        .collect()
}

/// The `held` line `done` takes: route template, ID, tool.
fn key_of(done: &Done, seen: &Seen, items: &[Item]) -> Held {
    let item = done.step.call.item();
    let page = |route: &str| Held {
        by: format!("/api/projects/{{p}}/proposals/{{id}}{route}"),
        id: seen.id(item).to_owned(),
        tool: "-".to_owned(),
    };
    match done.step.call {
        Call::Raise(_) => Held {
            by: "/mcp/{p}".to_owned(),
            id: "-".to_owned(),
            tool: items[item].tool.to_owned(),
        },
        Call::Row(_) => page(""),
        Call::Stage(_) => page("/decision"),
        Call::Read(_) | Call::Write(_) => page("/confirm"),
    }
}

/// `dones`, one round's answers, in the order their turns were held: each
/// `held` line from `lines` taken by one answer of its key. Raises take their
/// IDs in turn order, so the n-th raise line is the n-th smallest ID; of two
/// POSTs of one item, the one that committed held first. `None` while a line
/// has not come yet.
fn in_turn_order(dones: &[Done], lines: &[Held], seen: &Seen, items: &[Item]) -> Option<Vec<Done>> {
    let mut left: Vec<Option<&Done>> = dones.iter().map(Some).collect();
    let keys: Vec<Held> = dones.iter().map(|done| key_of(done, seen, items)).collect();
    let mut order = Vec::new();
    for line in lines {
        let candidates: Vec<usize> = (0..left.len())
            .filter(|&at| left[at].is_some() && keys[at] == *line)
            .collect();
        let chosen = if line.by == "/mcp/{p}" {
            candidates.iter().copied().min_by_key(|&at| {
                left[at].and_then(|done| {
                    done.json["result"]["structuredContent"]["id"]
                        .as_str()
                        .map(str::to_owned)
                })
            })
        } else {
            candidates
                .iter()
                .copied()
                .find(|&at| {
                    left[at].is_some_and(|done| {
                        matches!(done.step.call, Call::Write(_)) && done.status == 200
                    })
                })
                .or_else(|| candidates.first().copied())
        };
        if let Some(at) = chosen {
            order.push(left[at].take().expect("unmatched").clone());
        }
    }
    left.iter().all(Option::is_none).then_some(order)
}

// ------------------------------------------------------------- masks

/// `text` with the roots masked, every time, date and full commit ID too.
fn masked(text: &str, paths: &[(String, &'static str)]) -> String {
    let mut out = text.to_owned();
    for (path, name) in paths {
        out = out.replace(path.as_str(), name);
    }
    let time = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?Z").expect("a regex");
    let date = Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").expect("a regex");
    let sha = Regex::new(r"\b[0-9a-f]{40}\b").expect("a regex");
    let out = time.replace_all(&out, "<time>");
    let out = date.replace_all(&out, "<date>");
    sha.replace_all(&out, "<sha>").into_owned()
}

// ------------------------------------------------------------ the seed

/// xorshift64*: the scenario's draws.
struct Draw(u64);

impl Draw {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let value = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        usize::try_from(value % n as u64).expect("small")
    }

    fn shuffled<T>(&mut self, mut list: Vec<T>) -> Vec<T> {
        for at in (1..list.len()).rev() {
            list.swap(at, self.below(at + 1));
        }
        list
    }
}

// -------------------------------------------------------- the rounds

/// One round: each script on its own thread, all released at once (each
/// after a drawn stagger of 0-9 ms), its steps in order; every answer, or a
/// failure once the watchdog's deadline passes.
fn round(
    port: u16,
    scripts: Vec<Vec<Step>>,
    seen: &Seen,
    items: &Arc<Vec<Item>>,
    draw: &mut Draw,
    deadline: Instant,
    stderr: &dyn Fn() -> String,
) -> Vec<Done> {
    let expected: usize = scripts.iter().map(Vec::len).sum();
    let start = Arc::new(Barrier::new(scripts.len()));
    let (sender, answers) = mpsc::channel();
    for script in scripts {
        let stagger = Duration::from_millis(draw.below(10) as u64);
        let (start, sender, items) = (Arc::clone(&start), sender.clone(), Arc::clone(items));
        let mut seen = seen.clone();
        thread::spawn(move || {
            start.wait();
            thread::sleep(stagger);
            for step in script {
                let done = send(port, &step, &seen, &items);
                seen.note(&done);
                if sender.send(done).is_err() {
                    return;
                }
            }
        });
    }
    drop(sender);
    let mut out = Vec::new();
    while out.len() < expected {
        let left = deadline.saturating_duration_since(Instant::now());
        match answers.recv_timeout(left) {
            Ok(done) => out.push(done),
            Err(error) => panic!(
                "watchdog: {} of {expected} calls answered within {WATCHDOG:?} ({error}): a \
                 deadlock?\n--- the daemon's stderr\n{}",
                out.len(),
                stderr()
            ),
        }
    }
    out
}

/// A run's record: every round's answers in turn order.
struct Run {
    order: Vec<Done>,
}

/// The concurrent run over `world`, seed `seed`.
fn concurrent(world: &World, items: &Arc<Vec<Item>>, seed: u64) -> Run {
    let mut draw = Draw(seed);
    let port = world.port();
    let deadline = Instant::now() + WATCHDOG;
    let stderr = || world.server.stderr();
    let mut seen = Seen::default();
    let mut order: Vec<Done> = Vec::new();
    let take = |dones: Vec<Done>, seen: &mut Seen, order: &mut Vec<Done>, mark: usize| {
        let started = Instant::now();
        let ordered = loop {
            let lines = world.held();
            if let Some(ordered) = in_turn_order(&dones, &lines[mark..], seen, items) {
                break ordered;
            }
            assert!(
                started.elapsed() < LINE_WAIT,
                "the turn lines of {} calls never came\n--- the daemon's stderr\n{}",
                dones.len(),
                world.server.stderr()
            );
            thread::sleep(Duration::from_millis(50));
        };
        for done in &ordered {
            seen.note(done);
        }
        order.extend(ordered);
    };

    // Each login raises one item.
    let raisers = draw.shuffled(logins());
    let scripts = raisers
        .iter()
        .enumerate()
        .map(|(item, login)| {
            vec![Step {
                login,
                call: Call::Raise(item),
            }]
        })
        .collect();
    let mark = world.held().len();
    // Raises must be keyed before their IDs are known: note them first.
    let dones = round(port, scripts, &seen, items, &mut draw, deadline, &stderr);
    for done in &dones {
        assert!(
            done.json["result"]["structuredContent"]["id"].is_string(),
            "{} raised item {:?}, not stored: {}",
            done.step.login,
            done.step.call,
            done.body
        );
        seen.note(done);
    }
    take(dones, &mut seen, &mut order, mark);

    // Each item staged by a drawn login: its row read, then the stage.
    let all = logins();
    let scripts = (0..items.len())
        .map(|item| {
            let login = all[draw.below(all.len())];
            vec![
                Step {
                    login,
                    call: Call::Row(item),
                },
                Step {
                    login,
                    call: Call::Stage(item),
                },
            ]
        })
        .collect();
    let mark = world.held().len();
    let dones = round(port, scripts, &seen, items, &mut draw, deadline, &stderr);
    take(dones, &mut seen, &mut order, mark);

    // Each item read by a drawn login; one update and one question by two.
    let shared = [
        draw.below(UPDATES.len()),
        UPDATES.len() + draw.below(QUESTIONS.len()),
    ];
    let mut pairs: Vec<(&'static str, usize)> = Vec::new();
    for item in 0..items.len() {
        let first = all[draw.below(all.len())];
        pairs.push((first, item));
        if shared.contains(&item) {
            let mut second = all[draw.below(all.len())];
            while second == first {
                second = all[draw.below(all.len())];
            }
            pairs.push((second, item));
        }
    }
    let mut pending = draw.shuffled(pairs);
    for _ in 0..ROUNDS {
        if pending.is_empty() {
            break;
        }
        let reads = pending
            .iter()
            .map(|&(login, item)| {
                vec![Step {
                    login,
                    call: Call::Read(item),
                }]
            })
            .collect();
        let mark = world.held().len();
        let dones = round(port, reads, &seen, items, &mut draw, deadline, &stderr);
        let readable: Vec<(&'static str, usize)> = dones
            .iter()
            .filter(|done| done.status == 200)
            .map(|done| (done.step.login, done.step.call.item()))
            .collect();
        take(dones, &mut seen, &mut order, mark);
        let writes = draw
            .shuffled(readable)
            .into_iter()
            .map(|(login, item)| {
                vec![Step {
                    login,
                    call: Call::Write(item),
                }]
            })
            .collect();
        let mark = world.held().len();
        let dones = round(port, writes, &seen, items, &mut draw, deadline, &stderr);
        pending = dones
            .iter()
            .filter(|done| done.status == 409 && done.body.contains(MOVED))
            .map(|done| (done.step.login, done.step.call.item()))
            .collect();
        take(dones, &mut seen, &mut order, mark);
    }
    assert!(
        pending.is_empty(),
        "still moving after {ROUNDS} rounds: {pending:?}"
    );
    Run { order }
}

/// The recorded order replayed one call at a time over `world`.
fn replayed(world: &World, items: &[Item], order: &[Done]) -> Vec<Done> {
    let mut seen = Seen::default();
    order
        .iter()
        .map(|recorded| {
            let done = send(world.port(), &recorded.step, &seen, items);
            seen.note(&done);
            done
        })
        .collect()
}

// --------------------------------------------------------- the checks

/// The record a confirm's question shows: the lines after `record <ID> at
/// <path>:`, each indented two spaces, up to the first line that is not.
fn record_in(question: &str) -> Option<(String, String)> {
    let head = Regex::new(r"(?m)^record (\S+) at (\S+):$").expect("a regex");
    let captures = head.captures(question)?;
    let path = captures[2].to_owned();
    let after = &question[captures.get(0).expect("the match").end() + 1..];
    let mut lines = Vec::new();
    for line in after.split('\n') {
        if let Some(text) = line.strip_prefix("  ") {
            lines.push(text);
        } else if line.is_empty() {
            lines.push("");
        } else {
            break;
        }
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Some((path, text))
}

/// The record ID a question's read previews.
fn previewed(question: &str) -> Option<String> {
    let head = Regex::new(r"(?m)^record (\S+) at ").expect("a regex");
    head.captures(question)
        .map(|captures| captures[1].to_owned())
}

fn ten_logins(label: &str, seed: u64) {
    let world = World::new(&format!("{label}-a"));
    let items = Arc::new(items(&world));
    let run = concurrent(&world, &items, seed);
    let order = &run.order;
    for done in order {
        eprintln!(
            "{:>4} {:<5} {:?} {}",
            done.status,
            done.step.login,
            done.step.call,
            done.body.chars().take(100).collect::<String>()
        );
    }

    // Every item stored, every item decided.
    let raises: Vec<&Done> = order
        .iter()
        .filter(|done| matches!(done.step.call, Call::Raise(_)))
        .collect();
    assert_eq!(raises.len(), items.len());
    for done in &raises {
        assert!(
            done.json["result"]["structuredContent"]["id"].is_string(),
            "stored: {}",
            done.body
        );
    }
    let committed: Vec<&Done> = order
        .iter()
        .filter(|done| matches!(done.step.call, Call::Write(_)) && done.status == 200)
        .collect();
    let mut applied: Vec<usize> = committed.iter().map(|done| done.step.call.item()).collect();
    applied.sort_unstable();
    assert_eq!(
        applied,
        (0..items.len()).collect::<Vec<_>>(),
        "every item applied once"
    );
    assert!(
        order
            .iter()
            .any(|done| done.status == 409 && done.body.contains(MOVED)),
        "a question's record ID moved under another login's commit"
    );
    assert!(
        order.iter().any(|done| {
            matches!(done.step.call, Call::Write(_))
                && done.status == 409
                && done.body.contains("is already applied: commit ")
        }),
        "a shared item's second confirm got 0.47.0's refusal"
    );

    // Each committing POST's parent is the tip at its turn's start: the
    // commits on main are those POSTs in turn order, in a straight line.
    let commits = world.commits();
    let made: Vec<String> = committed
        .iter()
        .map(|done| {
            done.json["applied_commit"]
                .as_str()
                .unwrap_or_else(|| panic!("an applied commit: {}", done.body))
                .to_owned()
        })
        .collect();
    assert_eq!(
        made, commits,
        "the commits on main are the POSTs', in turn order"
    );
    let mut tip = world.base.clone();
    for commit in &commits {
        assert_eq!(world.parents(commit), [tip.clone()], "{commit}'s parent");
        tip.clone_from(commit);
    }

    // A read after another login's commit shows it: the record ID a
    // question's read previews follows the records committed before it.
    let mut records = 0;
    for done in order {
        let item = done.step.call.item();
        match done.step.call {
            Call::Read(_) if items[item].question && done.status == 200 => {
                let question = done.json["question"].as_str().expect("a question");
                assert_eq!(
                    previewed(question),
                    Some(format!("DEC-{}", 100_001 + records)),
                    "{} reads {item} after {records} records: {question}",
                    done.step.login
                );
            }
            Call::Write(_) if items[item].question && done.status == 200 => records += 1,
            _ => {}
        }
    }

    // Every committed record is the one its POST's question showed.
    let mut reads: BTreeMap<(&str, usize), &Done> = BTreeMap::new();
    for done in order {
        match done.step.call {
            Call::Read(item) if done.status == 200 => {
                reads.insert((done.step.login, item), done);
            }
            Call::Write(item) if done.status == 200 && items[item].question => {
                let question = reads[&(done.step.login, item)].json["question"]
                    .as_str()
                    .expect("a question");
                let (path, shown) = record_in(question)
                    .unwrap_or_else(|| panic!("no record in the question: {question}"));
                let commit = done.json["applied_commit"].as_str().expect("a commit");
                let stored = world.git(&world.setup.root, &["show", &format!("{commit}:{path}")]);
                assert_eq!(stored, shown, "{path} as committed and as asked");
            }
            _ => {}
        }
    }

    // The syncs pushed everything.
    world.synced();
    let pushed = world.git(&world.bare, &["rev-parse", "main"]);
    assert_eq!(pushed.trim(), tip, "origin's main is the hub's");

    // The recorded turn order, replayed one call at a time on a copy.
    let copy = World::new(&format!("{label}-b"));
    assert_eq!(copy.base, world.base, "the copy starts on the same commit");
    let again = replayed(&copy, &items, order);
    let (ours, theirs) = (world.paths(), copy.paths());
    for (first, second) in order.iter().zip(&again) {
        assert_eq!(
            (first.status, masked(&first.body, &ours)),
            (second.status, masked(&second.body, &theirs)),
            "{} {:?}: the concurrent answer and the replay's",
            first.step.login,
            first.step.call
        );
    }
    let mask_messages = |list: Vec<(String, String)>, paths: &[(String, &'static str)]| {
        list.into_iter()
            .map(|(tree, message)| (tree, masked(&message, paths)))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        mask_messages(world.trees_and_messages(), &ours),
        mask_messages(copy.trees_and_messages(), &theirs),
        "the commits' trees and trailers"
    );
}

#[test]
fn ten_logins_at_once_seed_11() {
    ten_logins("wc28-11", 11);
}

#[test]
fn ten_logins_at_once_seed_29() {
    ten_logins("wc28-29", 29);
}
