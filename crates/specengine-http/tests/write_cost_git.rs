//! docs/features/write-cost.md (RM-0125) WC7: the git processes a confirm
//! spawns, counted through the shim ("Goldens and the git shim": an
//! executable `git` first on the daemon's `PATH` that logs one line per call
//! and execs the real git). Over `fixtures/spec-a` opted in to a hub by a
//! committed `[team]`, in a fixed order: an update's GET and POST, then a
//! question's GET and POST, each proposal raised and staged through the
//! library (whose own git is not shimmed), so a request's lines are the
//! daemon's alone.
//!
//! - `confirm_spawns_no_more_than_at_0_47_0`: no kind spawns more than
//!   0.47.0 did (`fixtures/write-cost-golden/git-counts.json`, written by
//!   this test under `SPECENGINE_WRITE_COST_CAPTURE=1` at `v0.47.0`).
//! - `confirm_spawns_within_the_ceilings`: with G1-G5 ("Git cuts"), a
//!   question's GET spawns at most 12, its POST at most 25, and an update's
//!   GET and POST strictly fewer than at 0.47.0. M: one `--git-path` per
//!   process: red.
//! - `terminal_approve_spawns`: a measurement, ignored: the library's approve
//!   of an update and of a question, declined and confirmed, printed.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/write_cost_common/mod.rs"]
mod write_cost_common;

mod write_cost_hub;

use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use specengine_cli::{ApproveFlags, ApproveRequest, GitEnv, Globals, approve_with};
use write_cost_common::{LATER, Repo, capturing, golden_dir, probe};
use write_cost_hub::{
    confirm_get, confirm_post, git_shim, hub_env, logged, opted_in, propose_update, raise_question,
    shimmed, slug, stage_approve, start,
};

/// The corpus counted.
const CORPUS: &str = "spec-a";
/// A question's confirm, its most git spawns once G1-G5 land (the analyst's
/// supplement, adopted 2026-11-15).
const QUESTION_GET_MAX: usize = 12;
const QUESTION_POST_MAX: usize = 25;
/// Before each counted request: time for whatever the daemon does on the
/// library's own writes to end, so the request's lines are its own.
const SETTLE: Duration = Duration::from_millis(500);

/// The shim's lines of one request: the call's arguments.
type Lines = Vec<String>;

/// Each request's lines: `(kind, verb) -> lines`.
fn counted() -> BTreeMap<(String, String), Lines> {
    let repo = opted_in("wc-git", CORPUS);
    let found = probe(&repo.env(), &repo.git_env())
        .expect("spec-a opens")
        .expect("spec-a has a node");
    let slug = slug(&repo).expect("a slug");
    let shim_dir = repo.scratch.dir("shim");
    let log = repo.scratch.path().join("git.log");
    git_shim(&shim_dir, &log);
    let server = start(&repo, &shimmed(&hub_env(&repo), &shim_dir));
    let mut out = BTreeMap::new();
    let mut measure = |kind: &str, id: &str| {
        thread::sleep(SETTLE);
        let before = logged(&log).len();
        let read = confirm_get(&server, &slug, id);
        assert_eq!(read.status, 200, "{kind} GET: {}", read.text());
        let after_read = logged(&log);
        thread::sleep(SETTLE);
        let before_write = logged(&log).len();
        assert_eq!(
            before_write,
            after_read.len(),
            "{kind}: git ran between the GET and the POST: {:?}",
            &logged(&log)[after_read.len()..]
        );
        let written = confirm_post(&server, &slug, id, &read);
        assert_eq!(written.status, 200, "{kind} POST: {}", written.text());
        let after_write = logged(&log);
        out.insert(
            (kind.to_owned(), "get".to_owned()),
            after_read[before..].to_vec(),
        );
        out.insert(
            (kind.to_owned(), "post".to_owned()),
            after_write[before_write..].to_vec(),
        );
    };
    let (_, update) = propose_update(&repo, &found, "wc-git-update");
    let update = update.expect("the update is stored");
    stage_approve(&repo, &update);
    measure("update", &update);
    let (_, question) = raise_question(&repo, &found);
    let question = question.expect("the question is stored");
    stage_approve(&repo, &question);
    measure("question", &question);
    drop(server);
    out
}

/// The subcommand of one logged call (its arguments, each ending in a tab):
/// its first argument past git's own options (`-C dir`, `-c k=v`, `--flag`).
fn subcommand(line: &str) -> String {
    let mut words = line.split('\t');
    while let Some(word) = words.next() {
        match word {
            "-C" | "-c" => {
                words.next();
            }
            _ if word.starts_with('-') => {}
            _ => return word.to_owned(),
        }
    }
    String::new()
}

/// `lines` per subcommand.
fn breakdown(lines: &[String]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for line in lines {
        *out.entry(subcommand(line)).or_insert(0) += 1;
    }
    out
}

/// `{"question":{"get":n,"post":n},"update":{"get":n,"post":n}}`.
fn counts_of(lines: &BTreeMap<(String, String), Lines>) -> Value {
    let mut out = json!({});
    for ((kind, verb), calls) in lines {
        out[kind][verb] = json!(calls.len());
    }
    out
}

fn print(lines: &BTreeMap<(String, String), Lines>) {
    for ((kind, verb), calls) in lines {
        eprintln!(
            "{kind} {verb}: {} git processes {:?}",
            calls.len(),
            breakdown(calls)
        );
        for call in calls {
            eprintln!("    git {}", call.trim_end().replace('\t', " "));
        }
    }
}

fn recorded() -> Value {
    let path = golden_dir().join("git-counts.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}; capture it at v0.47.0", path.display()));
    serde_json::from_str(&text).expect("git-counts.json is JSON")
}

fn count(counts: &Value, kind: &str, verb: &str) -> usize {
    let n = counts[kind][verb]
        .as_u64()
        .unwrap_or_else(|| panic!("no {kind}.{verb} in {counts}"));
    usize::try_from(n).expect("a count")
}

#[test]
fn confirm_spawns_no_more_than_at_0_47_0() {
    let lines = counted();
    print(&lines);
    let now = counts_of(&lines);
    if capturing() {
        let mut text = serde_json::to_string(&now).expect("JSON");
        text.push('\n');
        std::fs::create_dir_all(golden_dir()).expect("the golden directory");
        std::fs::write(golden_dir().join("git-counts.json"), text).expect("write git-counts.json");
        return;
    }
    let then = recorded();
    for kind in ["question", "update"] {
        for verb in ["get", "post"] {
            assert!(
                count(&now, kind, verb) <= count(&then, kind, verb),
                "{kind} {verb}: {} git processes, 0.47.0 spawned {}",
                count(&now, kind, verb),
                count(&then, kind, verb)
            );
        }
    }
}

#[test]
fn confirm_spawns_within_the_ceilings() {
    let lines = counted();
    print(&lines);
    let now = counts_of(&lines);
    let then = recorded();
    assert!(
        count(&now, "question", "get") <= QUESTION_GET_MAX,
        "a question's GET: {} git processes, at most {QUESTION_GET_MAX}",
        count(&now, "question", "get")
    );
    assert!(
        count(&now, "question", "post") <= QUESTION_POST_MAX,
        "a question's POST: {} git processes, at most {QUESTION_POST_MAX}",
        count(&now, "question", "post")
    );
    for verb in ["get", "post"] {
        assert!(
            count(&now, "update", verb) < count(&then, "update", verb),
            "an update's {verb}: {} git processes, 0.47.0 spawned {}",
            count(&now, "update", verb),
            count(&then, "update", verb)
        );
    }
}

/// The library's approve of `id` under `git`, consent `answer`: the shim's
/// lines it added.
fn approve_lines(repo: &Repo, git: GitEnv, log: &Path, id: &str, answer: bool) -> Vec<String> {
    let before = logged(log).len();
    let result = approve_with(
        &repo.env(),
        &Globals::default(),
        &ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: LATER.to_owned(),
            git,
        },
        &ApproveFlags {
            option: None,
            answer: None,
            canon: None,
        },
        &mut |_: &str| answer,
    );
    let outcome = result.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(
        outcome.refusal.is_none(),
        answer,
        "approve {id}: {:?}",
        outcome.refusal
    );
    logged(log)[before..].to_vec()
}

#[test]
#[ignore = "a measurement for the spec's record: the terminal approve's git spawns, printed"]
fn terminal_approve_spawns() {
    let repo = Repo::new("wc-git-terminal", CORPUS);
    let found = probe(&repo.env(), &repo.git_env())
        .expect("spec-a opens")
        .expect("spec-a has a node");
    let shim_dir = repo.scratch.dir("shim");
    let log = repo.scratch.path().join("git.log");
    git_shim(&shim_dir, &log);
    let git = GitEnv::new(repo.root.as_path(), shimmed(&repo.vars, &shim_dir));
    let (_, update) = propose_update(&repo, &found, "wc-git-terminal");
    let update = update.expect("stored");
    let (_, question) = raise_question(&repo, &found);
    let question = question.expect("stored");
    for (what, id) in [("update", &update), ("question", &question)] {
        let declined = approve_lines(&repo, git.clone(), &log, id, false);
        let applied = approve_lines(&repo, git.clone(), &log, id, true);
        eprintln!(
            "terminal approve {what}: up to the prompt {} {:?}; with its commit {} {:?}",
            declined.len(),
            breakdown(&declined),
            applied.len(),
            breakdown(&applied)
        );
    }
}
