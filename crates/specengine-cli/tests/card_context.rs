//! docs/features/card-context.md (RM-0108) "Acceptance criteria", the CLI
//! half: the `spec` binary over scratch git copies of `fixtures/spec-a` (a
//! game, `[decision_records]`), each with its own `HOME`; the queue made
//! through the library at fixed clocks (cards as `card_new`, items as the
//! intake writers with their card, decided as `card_questions.rs` decides
//! them), so two copies hold one state. The fixture is only read.
//!
//! - CC1: `card show C --context --budget 3000 --json`'s `context.bundle`
//!   is `spec bundle T1 T2 R1 --budget 3000 --json`'s document as a value;
//!   the text ends with `context.bundle: <hash>` and that body; the budget
//!   is 10000 by default, `[budgets] bundle_task`'s when set, never
//!   `bundle_node`'s; a second read and a copy at another root and `HOME`
//!   are byte-equal. M: the first target only; `bundle_node` read; records
//!   before targets.
//! - CC2: plain `show` and `--json` are the build before's ([`GOLDEN`],
//!   recorded from 3892f03's `spec` by `record_the_reads_before_card_context`)
//!   plus `context: -` and `"context": null` (and, since
//!   docs/features/upstream-signal.md US-8, `upstream: -` after
//!   `correction:` and `"upstream": null` after `"correction"`: C-0001 has
//!   no parent; the golden stays as recorded); every `.md` unreadable and
//!   `.git` aside, `show` exits 0 with no stderr and `--context` with one
//!   note. M: the context always built.
//! - CC3: the bundle is fitted into the room the card's lines leave; a card
//!   whose lines are cut asks no bundle. M: fitted against 40 000.
//! - CC4: a bundle that cannot be made is a note on an exit-0 read; the
//!   usage errors exit 2 before anything is read. M: the bundle's exit
//!   passed through; the config error exit 2.
//! - CC5: `context.parent` is the direct parent's `{id, title, goal}`, its
//!   goal cut at 4 096 bytes after a hand edit, a corrupt row a note. M: the
//!   card's own goal.
//! - CC6, the CLI part: the REFs are the targets, then the records of the
//!   card's own applied items by item ID; a subtask's records stay out. M:
//!   open items taken; `subtask_items` taken. The cap of 64 records is the
//!   unit test in `card.rs`.
//!
//! The MCP half (CC2's `get_card`, CC4's `isError`, CC7, CC9) is
//! `specengine-mcp`'s `tests/card_context.rs`; CC10 http `daemon_keys.rs`;
//! CC11 MCP `mcp_size.rs`. Every expected value is worked out from the spec
//! or from `spec bundle` on the same state, never read back from a run of
//! the read under test.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::decision::{discrepancy, flags, hit_names};
use common::proposal::Pair;
use common::{RUN_TIMEOUT, Run, SPEC, md_files, read_text, repository_root, write};
use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use specengine_cli::{
    CardNewRequest, DecisionInput, DecisionRequest, DiscrepancyRequest, Exit, Globals,
    IntakeOutcome, QuestionRequest, card_new, propose_decision, propose_discrepancy,
    propose_question,
};

/// The golden of the reads before the slice, beside this file (CC2).
const GOLDEN: &str = "card_context_before.json";

/// The build the golden was recorded with: the slice's spec commit, its
/// code 3892f03's.
const RECORDED_WITH: &str = "spec at 82a7d39 (card-context's spec commit; its code is 3892f03's, \
                             before any src change of the slice)";

/// The clock of every card write: the scene is one state at every copy.
const CLOCK: &str = "2026-11-12T09:00:00Z";
/// The clock of every raise.
const RAISED: &str = "2026-11-12T08:00:00Z";

/// The ceiling of a read's text (`OUTPUT_CAP_CHARS`).
const CEILING: usize = 40_000;
/// CC3: the `context.bundle:` line at its made length.
const BUNDLE_LINE: usize = 84;
/// D4: `DEFAULT_TASK_BUDGET`.
const DEFAULT_BUDGET: u64 = 10_000;

/// CC1's card: two targets in two files, T2's file its own.
const T1: &str = "MEC-SPRINT";
const T2: &str = "MEC-STAMINA";
const T2_FILE: &str = "docs/spec/movement/stamina.md";
/// spec-a's records end at `DEC-0023`: the first approval writes this.
const R1: &str = "DEC-0024";
const TITLE: &str = "Write the stamina foreword";
const G1: &str = "Readers learn stamina before they sprint.";
const G2: &str = "The keeper's voice reads the foreword aloud.";

/// The notes of CC3 (b) and CC4 (d), as the spec writes them.
const NO_ROOM: &str = "no bundle: the card's lines leave no room under the 40000-character ceiling";
const NOTHING: &str = "no bundle: no targets and no recorded decision";

// ------------------------------------------------------------- helpers

/// `program args` in `cwd` with the sandbox's variables and the pair's
/// `HOME`, an empty pipe on stdin, with a watchdog.
fn run_by(pair: &Pair, program: &OsStr, cwd: &Path, args: &[&str]) -> Run {
    run_home(pair, program, cwd, &pair.home, args)
}

/// [`run_by`] with `home` as `HOME`.
fn run_home(pair: &Pair, program: &OsStr, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    let mut command = Command::new(program);
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", home)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn spec");
    drop(child.stdin.take());
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on spec") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec {args:?} ran longer than {RUN_TIMEOUT:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(out.join().expect("stdout")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr")).expect("UTF-8 stderr"),
    }
}

/// The binary under test in `cwd`.
fn spec(pair: &Pair, cwd: &Path, args: &[&str]) -> Run {
    run_by(pair, OsStr::new(SPEC), cwd, args)
}

/// `spec --json args` in `cwd`, exit `code`: the document.
fn doc_at(pair: &Pair, cwd: &Path, args: &[&str], code: i32) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = spec(pair, cwd, &all);
    done.code(code);
    done.json()
}

/// `spec --json args` in `cwd`, exit 0: stdout as written.
fn doc_text(pair: &Pair, cwd: &Path, args: &[&str]) -> String {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = spec(pair, cwd, &all);
    done.code(0);
    done.stdout
}

/// `card show <id> <flags>` in `cwd`: text, then JSON, each exit 0.
fn shown(pair: &Pair, cwd: &Path, id: &str, flags: &[&str]) -> (Run, Value) {
    let mut args = vec!["card", "show", id];
    args.extend_from_slice(flags);
    let text = spec(pair, cwd, &args);
    text.code(0);
    (text, doc_at(pair, cwd, &args, 0))
}

/// `spec bundle <refs> --budget N` in `cwd`: text, then JSON, each exit 0.
fn bundled(pair: &Pair, cwd: &Path, refs: &[&str], budget: u64) -> (Run, Value) {
    let budget = budget.to_string();
    let mut args = vec!["bundle"];
    args.extend_from_slice(refs);
    args.extend_from_slice(&["--budget", &budget]);
    let text = spec(pair, cwd, &args);
    text.code(0);
    (text, doc_at(pair, cwd, &args, 0))
}

/// A `spec bundle` text without its two closing lines (`bundle_hash …`,
/// `tokens …`): the body a card's text prints.
fn body_of(text: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    assert!(lines.len() > 2, "a bundle text: {text}");
    let closing = &lines[lines.len() - 2..];
    assert!(closing[0].starts_with("bundle_hash b3:"), "{text}");
    assert!(closing[1].starts_with("tokens "), "{text}");
    lines[..lines.len() - 2].concat()
}

/// A card through the library at [`CLOCK`], in `cwd`: its ID.
fn card(
    pair: &Pair,
    cwd: &Path,
    title: &str,
    goal: Option<&str>,
    targets: &[&str],
    criteria: &[String],
    parent: Option<&str>,
) -> String {
    let outcome = card_new(
        &pair.env(cwd),
        &Globals::default(),
        &CardNewRequest {
            title: title.to_owned(),
            goal: goal.map(str::to_owned),
            discipline: None,
            targets: targets.iter().map(|id| (*id).to_owned()).collect(),
            criteria: criteria.to_vec(),
            assignee: None,
            reviewer: parent.map(|_| "Ana Ruiz".to_owned()),
            mandate: None,
            estimate: None,
            due: None,
            parent: parent.map(str::to_owned),
            corrects: None,
            actor: None,
            now: CLOCK.to_owned(),
        },
    )
    .unwrap_or_else(|error| panic!("card new {title}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "card new {title}: {outcome:?}"
    );
    outcome.id.expect("a card's ID")
}

/// The stored item's ID.
fn stored(outcome: &IntakeOutcome) -> String {
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// A decision on `node` raised for `card` in `cwd`; when it hits, raised
/// again naming every hit.
fn decision(pair: &Pair, cwd: &Path, card: &str, node: &str, text: &str) -> String {
    let mut request = DecisionRequest {
        input: DecisionInput {
            node_ids: vec![node.to_owned()],
            text: text.to_owned(),
            working_answer: format!("Yes: {text}"),
            price_of_other: "A page rewritten.".to_owned(),
            why: None,
            severity: None,
            proposed_patch: None,
            distinct_from: None,
        },
        card: Some(card.to_owned()),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: RAISED.to_owned(),
        git: pair.git_env(cwd),
    };
    let raise = |request: &DecisionRequest| {
        propose_decision(&pair.env(cwd), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("decision {text}: {error}"))
    };
    let first = raise(&request);
    if first.document.created {
        return stored(&first);
    }
    request.input.distinct_from = Some(hit_names(&first));
    stored(&raise(&request))
}

/// A question on `node` raised for `card` in `cwd`, as [`decision`].
fn question(pair: &Pair, cwd: &Path, card: &str, node: &str, text: &str) -> String {
    let mut request = QuestionRequest {
        node_ids: vec![node.to_owned()],
        text: text.to_owned(),
        working_answer: "Yes, at once.".to_owned(),
        price_of_other: "A new edge case.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        card: Some(card.to_owned()),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: RAISED.to_owned(),
        git: pair.git_env(cwd),
        gist: None,
        matters: None,
        options: None,
        recommendation: None,
    };
    let ask = |request: &QuestionRequest| {
        propose_question(&pair.env(cwd), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("question {text}: {error}"))
    };
    let first = ask(&request);
    if first.document.created {
        return stored(&first);
    }
    request.distinct_from = hit_names(&first);
    stored(&ask(&request))
}

/// A discrepancy on `node` raised for `card` in `cwd`, as [`decision`].
fn report(pair: &Pair, cwd: &Path, card: &str, node: &str, summary: &str) -> String {
    let mut request = DiscrepancyRequest {
        input: discrepancy(
            &[node],
            summary,
            &[
                ("Keep the spec", "Fix the code", "one work item"),
                ("Change the spec", "Write the rule anew", "a rule"),
            ],
        ),
        card: Some(card.to_owned()),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: RAISED.to_owned(),
        git: pair.git_env(cwd),
    };
    let raise = |request: &DiscrepancyRequest| {
        propose_discrepancy(&pair.env(cwd), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("discrepancy {summary}: {error}"))
    };
    let first = raise(&request);
    if first.document.created {
        return stored(&first);
    }
    request.input.distinct_from = Some(hit_names(&first));
    stored(&raise(&request))
}

/// Approves `id` with no flag (its working answer): one record, written
/// and committed where it was raised; the record's ID.
fn approve(pair: &Pair, id: &str) -> String {
    pair.decide_ok(&pair.main, id, &flags(None, None, None), None);
    pair.proposal(id)
        .record
        .unwrap_or_else(|| panic!("{id} holds a record"))
        .id
}

/// Rejects `id`: settled, no record.
fn reject(pair: &Pair, id: &str) {
    let (outcome, _) = pair.reject_answer(&pair.main, id, "Not for this foreword.", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("reject {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "reject {id}: {outcome:?}");
}

/// CC1's scene on a fresh copy, read in the linked worktree (where the
/// items are raised and their records committed): `C-0001` with T1, T2
/// and G1; `PR-0001` a decision approved as [`R1`], `PR-0002` an open
/// question, `PR-0003` a rejected decision, each naming `C-0001`.
fn foreword(label: &str) -> Pair {
    let pair = Pair::new(label, "spec-a");
    let cwd = pair.linked.clone();
    let id = card(
        &pair,
        &cwd,
        TITLE,
        Some(G1),
        &[T1, T2],
        &["One page, read before the first sprint.".to_owned()],
        None,
    );
    assert_eq!(id, "C-0001");
    let first = decision(
        &pair,
        &cwd,
        "C-0001",
        "RULE-STAM-REGEN",
        "Does the foreword name the regeneration delay?",
    );
    assert_eq!(first, "PR-0001");
    assert_eq!(approve(&pair, &first), R1);
    let open = question(
        &pair,
        &cwd,
        "C-0001",
        "EDGE-STAM-ZERO",
        "Does the foreword show an empty tank?",
    );
    assert_eq!(open, "PR-0002");
    let gone = decision(
        &pair,
        &cwd,
        "C-0001",
        "EDGE-SPRINT-EMPTY",
        "Does the foreword quote the sprint table?",
    );
    assert_eq!(gone, "PR-0003");
    reject(&pair, &gone);
    pair
}

/// `config`'s worktree `specengine.toml` with `extra` appended: the line
/// `extra`'s first line lands on.
fn append_config(root: &Path, extra: &str) -> usize {
    let mut config = read_text(root, "specengine.toml");
    if !config.ends_with('\n') {
        config.push('\n');
    }
    let line = config.lines().count() + 1;
    config.push_str(extra);
    write(root, "specengine.toml", config);
    line
}

/// The lines of `text` that start with `prefix`, each without it.
fn values<'a>(text: &'a str, prefix: &str) -> Vec<&'a str> {
    text.lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .collect()
}

/// A JSON object's keys, in the order written.
struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> Visitor<'de> for Seen {
            type Value = Keys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

/// A JSON object's keys, in the order written.
fn keys(value: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(value)
        .unwrap_or_else(|error| panic!("{error}: {value}"))
        .0
}

/// The keys of the object under the first `"key":{` of a JSON text, in
/// the order written.
fn keys_at(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\":{{");
    let at = text
        .find(&needle)
        .unwrap_or_else(|| panic!("{key} in {text}"))
        + needle.len()
        - 1;
    let mut stream = serde_json::Deserializer::from_str(&text[at..]).into_iter::<Value>();
    stream.next().expect("an object").expect("JSON");
    keys(&text[at..at + stream.byte_offset()])
}

/// The `context: ` notes of a run's stderr, each without its prefix.
fn context_notes(done: &Run) -> Vec<String> {
    done.stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: context: "))
        .map(str::to_owned)
        .collect()
}

/// The text up to and including the `context.bundle:` line, and what
/// follows it.
fn split_at_bundle(text: &str) -> (&str, &str) {
    let at = text
        .find("\ncontext.bundle: ")
        .unwrap_or_else(|| panic!("a context.bundle line: {text}"));
    let end = at + 1 + text[at + 1..].find('\n').expect("the line ends") + 1;
    (&text[..end], &text[end..])
}

// ------------------------------------------------------------------ CC1

/// CC1: `--context --budget 3000 --json`'s bundle equals `spec bundle T1 T2
/// R1 --budget 3000 --json` as a value, `refs` `[T1, T2, R1]` (the open
/// question and the rejected decision hold no record); the text ends with
/// `context.bundle: <hash>` and that body byte for byte; its stderr is the
/// plain read's then the bundle's own. M: the first target only; records
/// before targets.
#[test]
fn cc1_the_bundle_is_spec_bundles_over_targets_then_records() {
    let pair = foreword("cc1-bundle");
    let cwd = pair.linked.clone();
    let (text, document) = shown(&pair, &cwd, "C-0001", &["--context", "--budget", "3000"]);
    let (bundle_text, bundle) = bundled(&pair, &cwd, &[T1, T2, R1], 3000);
    assert_eq!(bundle["refs"], json!([T1, T2, R1]), "{bundle}");
    let context = &document["context"];
    assert_eq!(
        context["bundle"], bundle,
        "M: the first target only, or records before targets"
    );
    assert_eq!(context["parent"], Value::Null, "{context}");
    assert_eq!(context["notes"], json!([]), "{context}");
    let raw = doc_text(
        &pair,
        &cwd,
        &["card", "show", "C-0001", "--context", "--budget", "3000"],
    );
    assert_eq!(keys_at(&raw, "context"), ["parent", "bundle", "notes"]);

    // The text: the card's lines, `history: -`, the three context lines,
    // the body last.
    let hash = bundle["bundle_hash"].as_str().expect("a hash");
    let body = body_of(&bundle_text.stdout);
    assert_eq!(bundle["body"], json!(body), "the body is the text's");
    let tail = format!(
        "history: -\ncontext.parent: -\ncontext.parent.goal: -\ncontext.bundle: {hash}\n{body}"
    );
    assert!(
        text.stdout.ends_with(&tail),
        "the text ends with the context lines and the body:\n{}",
        text.stdout
    );
    assert_eq!(
        format!("context.bundle: {hash}\n").chars().count(),
        BUNDLE_LINE
    );
    let plain = spec(&pair, &cwd, &["card", "show", "C-0001"]);
    plain.code(0);
    let before = plain
        .stdout
        .strip_suffix("context: -\n")
        .unwrap_or_else(|| panic!("the plain read ends with `context: -`: {}", plain.stdout));
    assert_eq!(
        text.stdout,
        format!("{}{}", before, &tail["history: -\n".len()..]),
        "the card's lines are the plain read's"
    );
    assert_eq!(
        text.stderr,
        format!("{}{}", plain.stderr, bundle_text.stderr),
        "the read's messages, then the bundle's own"
    );
    // The document is the plain one but `context`.
    let plain_document = doc_at(&pair, &cwd, &["card", "show", "C-0001"], 0);
    let mut without = document.clone();
    without["context"] = Value::Null;
    assert_eq!(without, plain_document);
}

/// CC1: without `--budget` the budget is 10000; `[budgets] bundle_task =
/// 1500` makes it 1500 (equal to `spec bundle … --budget 1500`);
/// `bundle_node = 900` alone leaves it 10000. M: `bundle_node` read for the
/// budget.
#[test]
fn cc1_the_budget_is_the_flag_then_bundle_task_then_10000() {
    let pair = foreword("cc1-budget");
    let cwd = pair.linked.clone();
    let refs = [T1, T2, R1];
    let at = |extra: &[&str]| {
        let mut flags = vec!["--context"];
        flags.extend_from_slice(extra);
        shown(&pair, &cwd, "C-0001", &flags).1["context"]["bundle"].clone()
    };
    let default = at(&[]);
    assert_eq!(default["budget"], json!(DEFAULT_BUDGET), "{default}");
    assert_eq!(default, bundled(&pair, &cwd, &refs, DEFAULT_BUDGET).1);

    let config = read_text(&cwd, "specengine.toml");
    append_config(&cwd, "\n[budgets]\nbundle_node = 900\n");
    let node_alone = at(&[]);
    assert_eq!(
        node_alone["budget"],
        json!(DEFAULT_BUDGET),
        "M: bundle_node read for the budget: {node_alone}"
    );
    assert_eq!(node_alone, default);

    write(&cwd, "specengine.toml", &config);
    append_config(&cwd, "\n[budgets]\nbundle_task = 1500\nbundle_node = 900\n");
    let task = at(&[]);
    assert_eq!(task["budget"], json!(1500), "{task}");
    assert_eq!(task, bundled(&pair, &cwd, &refs, 1500).1);
    // The flag over the key.
    let flagged = at(&["--budget", "3000"]);
    assert_eq!(flagged["budget"], json!(3000));
    assert_eq!(flagged, bundled(&pair, &cwd, &refs, 3000).1);
}

/// CC1: a second read is byte-equal, and so is a copy at another root and
/// `HOME`, text and JSON, stderr too.
#[test]
fn cc1_one_state_reads_byte_equal_at_any_root_and_home() {
    let first = foreword("cc1-copy-a");
    let second = foreword("cc1-copy-b");
    assert_ne!(first.linked, second.linked);
    assert_ne!(first.home, second.home);
    let reads = |pair: &Pair| -> Vec<(i32, String, String)> {
        [
            vec!["card", "show", "C-0001", "--context"],
            vec!["--json", "card", "show", "C-0001", "--context"],
            vec!["card", "show", "C-0001", "--context", "--budget", "3000"],
            vec![
                "--json",
                "card",
                "show",
                "C-0001",
                "--context",
                "--budget",
                "3000",
            ],
            vec![
                "card",
                "show",
                "C-0001",
                "--context",
                "--history",
                "--marks",
            ],
        ]
        .iter()
        .map(|args| {
            let done = spec(pair, &pair.linked, args);
            (done.code, done.stdout, done.stderr)
        })
        .collect()
    };
    let once = reads(&first);
    assert!(once.iter().all(|(code, _, _)| *code == 0), "{once:?}");
    assert_eq!(reads(&first), once, "a second read");
    assert_eq!(reads(&second), once, "a copy at another root and HOME");
}

// ------------------------------------------------------------------ CC2

/// One recorded read: its arguments, exit, stdout and stderr, the scratch
/// paths as placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Row {
    args: Vec<String>,
    code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Golden {
    recorded_with: String,
    procedure: String,
    rows: Vec<Row>,
}

/// The reads of the golden, on CC1's scene.
const READS: [&[&str]; 10] = [
    &["card", "show", "C-0001"],
    &["--json", "card", "show", "C-0001"],
    &["card", "show", "C-0001", "--marks"],
    &["--json", "card", "show", "C-0001", "--marks"],
    &["card", "show", "C-0001", "--history"],
    &["--json", "card", "show", "C-0001", "--history"],
    &["card", "show", "C-0001", "--history", "--marks"],
    &["--json", "card", "show", "C-0001", "--history", "--marks"],
    &["card", "show", "C-0009"],
    &["--json", "card", "show", "C-0009"],
];

fn golden_path() -> PathBuf {
    repository_root()
        .join("crates/specengine-cli/tests")
        .join(GOLDEN)
}

/// `text` with the pair's scratch paths as placeholders.
fn placed(pair: &Pair, text: &str) -> String {
    let mut out = text.to_owned();
    for (path, name) in [
        (&pair.linked, "<linked>"),
        (&pair.main, "<main>"),
        (&pair.home, "<home>"),
    ] {
        out = out.replace(path.to_str().expect("UTF-8"), name);
    }
    out.replace(pair.scratch.path().to_str().expect("UTF-8"), "<scratch>")
}

/// Every read of [`READS`] by `program` on a fresh CC1 scene.
fn play(program: &OsStr) -> Vec<Row> {
    let pair = foreword("cc2-play");
    READS
        .iter()
        .map(|args| {
            let done = run_by(&pair, program, &pair.linked, args);
            Row {
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                code: done.code,
                stdout: placed(&pair, &done.stdout),
                stderr: placed(&pair, &done.stderr),
            }
        })
        .collect()
}

fn golden() -> Golden {
    let text = fs::read_to_string(golden_path())
        .unwrap_or_else(|error| panic!("{GOLDEN}: {error}; record it once from 3892f03's binary"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{GOLDEN}: {error}"))
}

/// CC2: every read of the golden, played now: a text read is the build
/// before's plus the last line `context: -`, a JSON read its document plus
/// `"context": null` between `history` and `notes` (every other key equal,
/// in order), stderr and exit unchanged; no such card exits 1 as before,
/// with `--context` too. M: the context always built.
#[test]
fn cc2_not_asked_is_the_read_before_plus_context_null() {
    let golden = golden();
    assert_eq!(golden.recorded_with, RECORDED_WITH);
    let now = play(OsStr::new(SPEC));
    assert_eq!(now.len(), golden.rows.len());
    for (before, after) in golden.rows.iter().zip(&now) {
        assert_eq!(after.args, before.args);
        let context = format!("{:?}", before.args);
        assert_eq!(after.code, before.code, "{context}");
        assert_eq!(after.stderr, before.stderr, "{context}: stderr");
        if before.code != 0 {
            assert_eq!(after.stdout, before.stdout, "{context}: no context");
            continue;
        }
        if before.args[0] == "--json" {
            let mut want = keys(&before.stdout);
            // docs/features/upstream-signal.md US-8: `upstream` right after
            // `correction`, `null` on a card with no parent.
            let correction = want
                .iter()
                .position(|key| key == "correction")
                .expect("correction");
            want.insert(correction + 1, "upstream".to_owned());
            let at = want
                .iter()
                .position(|key| key == "history")
                .expect("history")
                + 1;
            want.insert(at, "context".to_owned());
            assert_eq!(
                keys(&after.stdout),
                want,
                "{context}: context after history"
            );
            let mut document: Value = serde_json::from_str(&after.stdout).expect("JSON");
            assert_eq!(document["context"], Value::Null, "{context}");
            assert_eq!(document["upstream"], Value::Null, "{context}");
            for key in ["context", "upstream"] {
                document.as_object_mut().expect("an object").remove(key);
            }
            // docs/features/plain-questions.md PQ10: each item's `gist` and
            // `matters` after `summary`, `null` on the golden's items.
            for list in ["items", "subtask_items"] {
                if let Some(items) = document[list].as_array_mut() {
                    for item in items {
                        let object = item.as_object_mut().expect("an item");
                        for key in ["gist", "matters"] {
                            assert_eq!(
                                object.remove(key),
                                Some(Value::Null),
                                "{context}: {list}'s {key}"
                            );
                        }
                    }
                }
            }
            let was: Value = serde_json::from_str(&before.stdout).expect("JSON");
            assert_eq!(document, was, "{context}: every other key equal");
        } else {
            // docs/features/upstream-signal.md US-8: `upstream: -` right
            // after the `correction:` line.
            let mut lines: Vec<&str> = before.stdout.lines().collect();
            let correction = lines
                .iter()
                .position(|line| line.starts_with("correction: "))
                .expect("a correction line");
            lines.insert(correction + 1, "upstream: -");
            assert_eq!(
                after.stdout,
                format!("{}\ncontext: -\n", lines.join("\n")),
                "{context}: `upstream: -` after `correction:`, the last line `context: -`"
            );
        }
    }
    // No such card, asked: exit 1 as before, no context.
    let pair = foreword("cc2-none");
    for args in [
        &["card", "show", "C-0009", "--context"][..],
        &["--json", "card", "show", "C-0009", "--context"],
    ] {
        let done = spec(&pair, &pair.linked, args);
        let without: Vec<&str> = args.iter().copied().filter(|a| *a != "--context").collect();
        let row = golden
            .rows
            .iter()
            .find(|row| row.args == without)
            .expect("its row");
        assert_eq!(done.code, 1, "{args:?}");
        assert_eq!(placed(&pair, &done.stdout), row.stdout, "{args:?}");
        assert_eq!(placed(&pair, &done.stderr), row.stderr, "{args:?}");
    }
}

/// CC2: after the card is made, every `.md` of the copy unreadable (mode
/// 000) and `.git` moved aside: `show C-0001` exits 0 with no stderr line
/// (it reads no corpus and no git); with `--context` it exits 0 with the
/// card's lines, `context.bundle` `null` and one `context:` note. M: the
/// context always built.
#[test]
fn cc2_a_plain_read_reads_no_corpus_and_no_git() {
    let pair = foreword("cc2-locked");
    let cwd = pair.linked.clone();
    let plain_before = spec(&pair, &cwd, &["card", "show", "C-0001"]);
    plain_before.code(0);
    let files = md_files(&cwd);
    assert!(files.len() > 10, "{files:?}");
    for file in &files {
        fs::set_permissions(cwd.join(file), fs::Permissions::from_mode(0o000)).expect("chmod 000");
    }
    fs::rename(cwd.join(".git"), cwd.join("git-aside")).expect("move .git aside");
    let plain = spec(&pair, &cwd, &["card", "show", "C-0001"]);
    plain.code(0);
    assert_eq!(plain.stderr, "", "M: the context always built");
    assert_eq!(plain.stdout, plain_before.stdout);
    let json = spec(&pair, &cwd, &["--json", "card", "show", "C-0001"]);
    json.code(0);
    assert_eq!(json.stderr, "");
    assert_eq!(json.json()["context"], Value::Null);

    let asked = spec(&pair, &cwd, &["card", "show", "C-0001", "--context"]);
    asked.code(0);
    let lines = plain
        .stdout
        .strip_suffix("context: -\n")
        .expect("the plain read's last line");
    assert!(
        asked.stdout.starts_with(lines),
        "the card's lines:\n{}",
        asked.stdout
    );
    assert!(
        asked
            .stdout
            .ends_with("context.parent: -\ncontext.parent.goal: -\ncontext.bundle: -\n"),
        "{}",
        asked.stdout
    );
    let notes = context_notes(&asked);
    assert_eq!(notes.len(), 1, "one context note: {}", asked.stderr);
    assert!(notes[0].starts_with("no bundle: "), "{notes:?}");
    let document = doc_at(&pair, &cwd, &["card", "show", "C-0001", "--context"], 0);
    assert_eq!(document["context"]["bundle"], Value::Null);
    assert_eq!(document["context"]["notes"], json!(notes));
    // Restored for the scratch's removal.
    fs::rename(cwd.join("git-aside"), cwd.join(".git")).expect("move .git back");
}

/// Writes [`GOLDEN`] from the binary `SPECENGINE_BEFORE_SPEC` names
/// (3892f03's `spec`, built before any `src/` change of card-context). Run
/// once, by hand: `SPECENGINE_BEFORE_SPEC=<binary> cargo nextest run -p
/// specengine-cli --test card_context record_the_reads_before
/// --run-ignored ignored-only`. Every read is played twice and must answer
/// alike. It refuses when the golden exists.
#[test]
#[ignore = "manual: records card-context's golden from 3892f03's binary, once"]
fn record_the_reads_before_card_context() {
    let program = std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC");
    let path = golden_path();
    assert!(
        !path.exists(),
        "{GOLDEN} exists: it is recorded once, never again"
    );
    let rows = play(&program);
    assert_eq!(rows, play(&program), "the build before answers alike twice");
    assert!(
        rows.iter().all(|row| !row.stdout.contains("context")),
        "the build before knows no context: {rows:?}"
    );
    let golden = Golden {
        recorded_with: RECORDED_WITH.to_owned(),
        procedure: "card_context.rs `foreword`: a fresh spec-a copy, its queue made through the \
                    library at fixed clocks; each read in the linked worktree, the pair's HOME; \
                    scratch paths as <linked>, <main>, <home>, <scratch>"
            .to_owned(),
        rows,
    };
    let mut text = serde_json::to_string_pretty(&golden).expect("JSON");
    text.push('\n');
    fs::write(&path, text).expect("write the golden");
}

// ------------------------------------------------------------------ CC3

/// About `chars` characters of plain prose, sentence by sentence, marked
/// with `mark` so no two documents share a sentence.
fn prose(mark: &str, chars: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    while out.len() < chars {
        n += 1;
        out.push_str(&format!(
            "The {mark} keeper trims wick number {n} before the tide turns, and the lamp burns \
             steady through watch {n}.\n"
        ));
        if n % 6 == 0 {
            out.push('\n');
        }
    }
    out
}

/// A canon document `MEC-<name>` under `docs/spec/long/`, its body about
/// `chars` characters.
fn long_doc(root: &Path, name: &str, chars: usize) {
    write(
        root,
        &format!("docs/spec/long/{}.md", name.to_lowercase()),
        format!(
            "---\nid: MEC-{name}\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Long {name}\n\
             parent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
             # Long {name}\n\n{}",
            prose(name, chars)
        ),
    );
}

/// A canon document `MEC-FILL` with 64 sections `RULE-FILL-01` to `-64`.
fn fill_doc(root: &Path) -> Vec<String> {
    let mut text = "---\nid: MEC-FILL\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Fill\n\
                    parent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n\
                    ---\n\n# Fill\n\nThe lamp room's rules.\n"
        .to_owned();
    let mut ids = Vec::new();
    for n in 1..=64 {
        let id = format!("RULE-FILL-{n:02}");
        text.push_str(&format!(
            "\n## Fill {n} {{#{id}}}\n\nWick {n} is trimmed at dusk.\n"
        ));
        ids.push(id);
    }
    write(root, "docs/spec/fill.md", text);
    ids
}

/// `n` ASCII criteria of exactly `bytes` bytes each.
fn criteria(n: usize, bytes: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let mut line = format!("Criterion {i}: the lamp stays lit");
            while line.len() < bytes {
                line.push_str(" and steady");
            }
            line.truncate(bytes);
            line
        })
        .collect()
}

/// CC3 (a): a card whose criteria make its lines about 25 000 characters,
/// three targets whose bundle at 10 000 prints more than the room. The
/// text before the body (`U`, the `context.bundle:` line counted) plus
/// `context.bundle.chars` stays within 40 000, `chars` is the body's, and
/// the body is shorter than `spec bundle`'s at the same budget alone. M:
/// the bundle fitted against 40 000 instead of the room.
#[test]
fn cc3_the_bundle_fits_the_room_the_lines_leave() {
    let pair = Pair::new("cc3-room", "spec-a");
    let cwd = pair.linked.clone();
    for name in ["LONG-A", "LONG-B", "LONG-C"] {
        long_doc(&cwd, name, 16_000);
    }
    pair.commit_all(&cwd, "Three long documents.");
    let refs = ["MEC-LONG-A", "MEC-LONG-B", "MEC-LONG-C"];
    let id = card(
        &pair,
        &cwd,
        "The long watch",
        Some("Every keeper reads the three watches."),
        &refs,
        &criteria(25, 1000),
        None,
    );
    let (text, document) = shown(&pair, &cwd, &id, &["--context"]);
    let bundle = &document["context"]["bundle"];
    assert_eq!(bundle["budget"], json!(DEFAULT_BUDGET), "{bundle}");
    let (head, body) = split_at_bundle(&text.stdout);
    let room = head.chars().count();
    assert!(
        (24_000..30_000).contains(&room),
        "the lines are about 25 000 characters: {room}"
    );
    let chars = usize::try_from(bundle["chars"].as_u64().expect("chars")).expect("usize");
    assert_eq!(chars, body.chars().count(), "chars is the body's");
    assert_eq!(bundle["body"], json!(body));
    assert!(
        room + chars <= CEILING,
        "M: fitted against 40 000: U {room} + chars {chars} > {CEILING}"
    );
    assert!(text.stdout.chars().count() <= CEILING);
    let (alone_text, alone) = bundled(&pair, &cwd, &refs, DEFAULT_BUDGET);
    let alone_chars = body_of(&alone_text.stdout).chars().count();
    assert!(
        alone_chars > CEILING - room,
        "the bundle alone prints more than the room: {alone_chars} vs {}",
        CEILING - room
    );
    assert!(chars < alone_chars, "{chars} < {alone_chars}");
    assert_eq!(bundle["refs"], alone["refs"]);
    assert!(context_notes(&text).is_empty(), "{}", text.stderr);
}

/// CC3 (b): a subtask with a 4 096 B goal, 32 criteria of 1 024 B and 64
/// targets, its parent's goal 4 096 B: exit 0, the text within 40 000
/// characters plus the truncated tail, `context.bundle` `null`, one `no
/// bundle:` note.
#[test]
fn cc3_lines_that_are_cut_ask_no_bundle() {
    let pair = Pair::new("cc3-cut", "spec-a");
    let cwd = pair.linked.clone();
    let targets = fill_doc(&cwd);
    pair.commit_all(&cwd, "A document of 64 rules.");
    let goal = "g".repeat(4096);
    let parent = card(&pair, &cwd, "The lamp room", Some(&goal), &[T1], &[], None);
    let refs: Vec<&str> = targets.iter().map(String::as_str).collect();
    let child = card(
        &pair,
        &cwd,
        "Every wick",
        Some(&"w".repeat(4096)),
        &refs,
        &criteria(32, 1024),
        Some(&parent),
    );
    let (text, document) = shown(&pair, &cwd, &child, &["--context"]);
    let shown_text = match text.stdout.find("\n[truncated: ") {
        Some(at) => &text.stdout[..=at],
        None => text.stdout.as_str(),
    };
    assert!(
        shown_text.chars().count() <= CEILING,
        "{} characters before the tail",
        shown_text.chars().count()
    );
    let context = &document["context"];
    assert_eq!(context["bundle"], Value::Null, "{context}");
    assert_eq!(context["notes"], json!([NO_ROOM]), "{context}");
    assert_eq!(context_notes(&text), [NO_ROOM], "{}", text.stderr);
    assert_eq!(context["parent"]["id"], json!(parent));
    assert_eq!(context["parent"]["goal"], json!(goal));
}

// ------------------------------------------------------------------ CC4

/// CC4 (a): T2's file deleted: exit 0 with the card's document, `bundle`
/// `null`, the one note `no bundle: <spec bundle's reason naming T2>`,
/// printed `note: context: …`. M: the bundle's exit passed through.
#[test]
fn cc4_a_ref_that_does_not_resolve_is_a_note() {
    let pair = foreword("cc4-gone");
    let cwd = pair.linked.clone();
    fs::remove_file(cwd.join(T2_FILE)).expect("remove T2's file");
    let failed = spec(&pair, &cwd, &["--json", "bundle", T1, T2, R1]);
    failed.code(1);
    let reason = failed.json()["reason"]
        .as_str()
        .expect("a reason")
        .to_owned();
    assert!(reason.contains(T2), "{reason}");
    let note = format!("no bundle: {reason}");
    let text = spec(&pair, &cwd, &["card", "show", "C-0001", "--context"]);
    assert_eq!(
        text.code,
        0,
        "M: the bundle's exit passed through: {}",
        text.show()
    );
    assert!(
        text.stdout.ends_with("context.bundle: -\n"),
        "{}",
        text.stdout
    );
    assert_eq!(
        context_notes(&text),
        std::slice::from_ref(&note),
        "{}",
        text.stderr
    );
    assert!(
        text.stderr
            .lines()
            .any(|line| line == format!("note: context: {note}")),
        "{}",
        text.stderr
    );
    let document = doc_at(&pair, &cwd, &["card", "show", "C-0001", "--context"], 0);
    assert_eq!(document["id"], json!("C-0001"));
    assert_eq!(document["context"]["bundle"], Value::Null);
    assert_eq!(document["context"]["notes"], json!([note]));
}

/// CC4 (b): `--budget 1`: exit 0, the note `spec bundle`'s exit-2 line
/// naming the minimum. M: the bundle's exit passed through.
#[test]
fn cc4_a_budget_below_the_minimum_is_a_note() {
    let pair = foreword("cc4-minimum");
    let cwd = pair.linked.clone();
    let failed = spec(&pair, &cwd, &["bundle", T1, T2, R1, "--budget", "1"]);
    failed.code(2);
    let line = failed
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("spec: "))
        .expect("bundle's error line")
        .to_owned();
    assert!(line.contains("minimum"), "{line}");
    let text = spec(
        &pair,
        &cwd,
        &["card", "show", "C-0001", "--context", "--budget", "1"],
    );
    assert_eq!(
        text.code,
        0,
        "M: the bundle's exit passed through: {}",
        text.show()
    );
    let notes = context_notes(&text);
    assert_eq!(notes, [format!("no bundle: {line}")], "{}", text.stderr);
    assert!(notes[0].contains("--budget 1"), "{notes:?}");
    let document = doc_at(
        &pair,
        &cwd,
        &["card", "show", "C-0001", "--context", "--budget", "1"],
        0,
    );
    assert_eq!(document["context"]["bundle"], Value::Null);
    assert_eq!(document["context"]["notes"], json!(notes));
}

/// CC4 (c): `[budgets] bundle_task = 0`: exit 0, a note naming its line;
/// with `--budget 3000` the bundle is made. M: the config error exit 2.
#[test]
fn cc4_a_bad_bundle_task_is_a_note_at_its_line() {
    let pair = foreword("cc4-config");
    let cwd = pair.linked.clone();
    let line = append_config(&cwd, "[budgets]\nbundle_task = 0\n") + 1;
    let text = spec(&pair, &cwd, &["card", "show", "C-0001", "--context"]);
    assert_eq!(text.code, 0, "M: the config error exit 2: {}", text.show());
    let notes = context_notes(&text);
    assert_eq!(
        notes,
        [format!(
            "no bundle: specengine.toml:{line}: `[budgets] bundle_task` must be a whole number of \
             tokens from 1 to 4294967295, not 0"
        )],
        "{}",
        text.stderr
    );
    let document = doc_at(&pair, &cwd, &["card", "show", "C-0001", "--context"], 0);
    assert_eq!(document["context"]["bundle"], Value::Null);
    let (_, made) = shown(&pair, &cwd, "C-0001", &["--context", "--budget", "3000"]);
    assert_eq!(
        made["context"]["bundle"],
        bundled(&pair, &cwd, &[T1, T2, R1], 3000).1
    );
    assert_eq!(made["context"]["notes"], json!([]));
}

/// CC4 (d): a manual card with no records: `no bundle: no targets and no
/// recorded decision` (the spec's "decisions" made singular for the CLI's
/// genre scan, `proposal_genre.rs`).
#[test]
fn cc4_a_manual_card_has_nothing_to_bundle() {
    let pair = foreword("cc4-manual");
    let cwd = pair.linked.clone();
    let manual = card(&pair, &cwd, "Sweep the stairs", None, &[], &[], None);
    let (text, document) = shown(&pair, &cwd, &manual, &["--context"]);
    assert_eq!(context_notes(&text), [NOTHING], "{}", text.stderr);
    assert_eq!(
        document["context"],
        json!({"parent": null, "bundle": null, "notes": [NOTHING]})
    );
    assert!(
        text.stdout.ends_with("context.bundle: -\n"),
        "{}",
        text.stdout
    );
}

/// CC4: `--budget 0`, `--budget 4294967296` and `--budget 5` without
/// `--context` exit 2 with stdout empty, before anything is read: a fresh
/// `HOME` holds no data directory after them.
#[test]
fn cc4_the_usage_errors_exit_2_with_nothing_read() {
    let pair = foreword("cc4-usage");
    let cwd = pair.linked.clone();
    let fresh = pair.scratch.home("fresh");
    for (args, says) in [
        (
            &["card", "show", "C-0001", "--context", "--budget", "0"][..],
            "--budget 0: the budget is a whole number of estimated tokens from 1 to 4294967295",
        ),
        (
            &[
                "card",
                "show",
                "C-0001",
                "--context",
                "--budget",
                "4294967296",
            ],
            "--budget 4294967296: the budget is a whole number of estimated tokens from 1 to \
             4294967295",
        ),
        (
            &["card", "show", "C-0001", "--budget", "5"],
            "`--budget` needs `--context`",
        ),
        (
            &["--json", "card", "show", "C-0001", "--budget", "5"],
            "`--budget` needs `--context`",
        ),
        // The pairing is checked before the range.
        (
            &["card", "show", "C-0001", "--budget", "0"],
            "`--budget` needs `--context`",
        ),
    ] {
        let done = run_home(&pair, OsStr::new(SPEC), &cwd, &fresh, args);
        assert_eq!(done.code, 2, "{args:?}: {}", done.show());
        assert_eq!(done.stdout, "", "{args:?}");
        assert!(done.stderr.contains(says), "{args:?}: {}", done.stderr);
        let entries: Vec<_> = fs::read_dir(&fresh).expect("home").collect();
        assert!(entries.is_empty(), "{args:?}: nothing read or made in HOME");
    }
}

// ------------------------------------------------------------------ CC5

/// CC5: a hand-off `C-0002` (own goal G2) of `C-0001` (goal G1) reads its
/// parent's `{id, title, goal}`; `C-0001` reads `parent: null` and no
/// note; a parent without a goal `goal: null`. M: the card's own goal.
#[test]
fn cc5_the_parent_and_its_goal() {
    let pair = foreword("cc5-parent");
    let cwd = pair.linked.clone();
    let child = card(
        &pair,
        &cwd,
        "Read it aloud",
        Some(G2),
        &[T1],
        &[],
        Some("C-0001"),
    );
    assert_eq!(child, "C-0002");
    let (text, document) = shown(&pair, &cwd, &child, &["--context"]);
    assert_eq!(
        document["context"]["parent"],
        json!({"id": "C-0001", "title": TITLE, "goal": G1}),
        "M: the card's own goal"
    );
    assert_eq!(
        values(&text.stdout, "context.parent: "),
        [format!("C-0001 | {TITLE}")]
    );
    assert_eq!(values(&text.stdout, "context.parent.goal: "), [G1]);
    assert!(context_notes(&text).is_empty(), "{}", text.stderr);

    let (text, document) = shown(&pair, &cwd, "C-0001", &["--context"]);
    assert_eq!(document["context"]["parent"], Value::Null);
    assert_eq!(values(&text.stdout, "context.parent: "), ["-"]);
    assert_eq!(values(&text.stdout, "context.parent.goal: "), ["-"]);
    assert!(context_notes(&text).is_empty(), "no note: {}", text.stderr);

    let bare = card(&pair, &cwd, "No goal", None, &[T1], &[], None);
    let below = card(&pair, &cwd, "Below it", Some(G2), &[T1], &[], Some(&bare));
    let (text, document) = shown(&pair, &cwd, &below, &["--context"]);
    assert_eq!(
        document["context"]["parent"],
        json!({"id": bare, "title": "No goal", "goal": null})
    );
    assert_eq!(values(&text.stdout, "context.parent.goal: "), ["-"]);
}

/// CC5: the parent's row hand-corrupted: `parent: null` and the note
/// `parent C-0001 not read: …`; its goal hand-edited to 5 001 bytes of
/// three-byte characters: cut within 4 096 bytes on a character boundary
/// (4 095), with its note.
#[test]
fn cc5_a_corrupt_parent_and_a_long_goal_are_notes() {
    let pair = foreword("cc5-corrupt");
    let cwd = pair.linked.clone();
    let child = card(
        &pair,
        &cwd,
        "Read it aloud",
        Some(G2),
        &[T1],
        &[],
        Some("C-0001"),
    );

    let euro = "\u{20ac}";
    let long = euro.repeat(1667);
    assert_eq!(long.len(), 5001);
    pair.sql(&format!(
        "UPDATE cards SET goal = '{long}' WHERE id = 'C-0001';"
    ));
    let (text, document) = shown(&pair, &cwd, &child, &["--context"]);
    let cut = euro.repeat(1365);
    assert_eq!(cut.len(), 4095);
    assert_eq!(document["context"]["parent"]["goal"], json!(cut));
    let note = "parent C-0001's goal cut at 4096 bytes";
    assert_eq!(
        document["context"]["notes"]
            .as_array()
            .map(|n| n[0].clone()),
        Some(json!(note))
    );
    assert_eq!(context_notes(&text)[0], note, "{}", text.stderr);
    assert_eq!(
        values(&text.stdout, "context.parent.goal: "),
        [cut.as_str()]
    );

    pair.sql("UPDATE cards SET evidence = 'not json' WHERE id = 'C-0001';");
    let broken = spec(&pair, &cwd, &["card", "show", "C-0001"]);
    broken.code(2);
    let (text, document) = shown(&pair, &cwd, &child, &["--context"]);
    assert_eq!(document["context"]["parent"], Value::Null);
    let notes = context_notes(&text);
    assert!(
        notes[0].starts_with("parent C-0001 not read: ") && notes[0].contains("evidence"),
        "{notes:?}"
    );
    assert_eq!(document["context"]["notes"][0], json!(notes[0]));
    assert_eq!(values(&text.stdout, "context.parent: "), ["-"]);
}

// ------------------------------------------------------------------ CC6

/// CC6, the CLI part: `C-0001`'s items are a decision applied (`PR-0001`),
/// a question applied (`PR-0002`, approved first, so its record is the
/// lower), an open decision, a rejected one, a decision approved whose row
/// was set back to `approved` (a record, not applied), and a discrepancy
/// whose row was set to `applied` with no record (unreadable: a store
/// rule, so it is left out of `items`). Its subtask `C-0002` has a decision
/// applied with R3, listed in `C-0001`'s `subtask_items`. `C-0001`'s refs
/// are its targets, then `PR-0001`'s record and `PR-0002`'s, by item ID;
/// `C-0002`'s its target then R3. M: open items taken; `subtask_items`
/// taken.
#[test]
fn cc6_only_the_cards_own_applied_records() {
    let pair = Pair::new("cc6-records", "spec-a");
    let cwd = pair.linked.clone();
    let parent = card(&pair, &cwd, TITLE, Some(G1), &[T1, T2], &[], None);
    let first = decision(
        &pair,
        &cwd,
        &parent,
        "RULE-STAM-REGEN",
        "Does the foreword name the regeneration delay?",
    );
    let second = question(
        &pair,
        &cwd,
        &parent,
        "EDGE-STAM-ZERO",
        "Does the foreword show an empty tank?",
    );
    let open = decision(
        &pair,
        &cwd,
        &parent,
        "EDGE-SPRINT-EMPTY",
        "Does the foreword quote the sprint table?",
    );
    let rejected = decision(
        &pair,
        &cwd,
        &parent,
        "RULE-SPRINT-COST",
        "Does the foreword price a sprint?",
    );
    let unapplied = decision(
        &pair,
        &cwd,
        &parent,
        "MEC-SPRINT",
        "Does the foreword draw the sprint key?",
    );
    let unrecorded = report(
        &pair,
        &cwd,
        &parent,
        "MEC-STAMINA",
        "The foreword's stamina bar is drawn twice.",
    );
    let child = card(
        &pair,
        &cwd,
        "Read it aloud",
        Some(G2),
        &[T1],
        &[],
        Some(&parent),
    );
    let below = decision(
        &pair,
        &cwd,
        &child,
        "EDGE-STAM-ZERO",
        "Does the reading pause at an empty tank?",
    );
    assert_eq!(
        [
            &first,
            &second,
            &open,
            &rejected,
            &unapplied,
            &unrecorded,
            &below
        ],
        [
            "PR-0001", "PR-0002", "PR-0003", "PR-0004", "PR-0005", "PR-0006", "PR-0007"
        ]
    );
    let r2 = approve(&pair, &second);
    let r1 = approve(&pair, &first);
    assert_eq!((r2.as_str(), r1.as_str()), ("DEC-0024", "DEC-0025"));
    reject(&pair, &rejected);
    let held = approve(&pair, &unapplied);
    let r3 = approve(&pair, &below);
    assert_eq!((held.as_str(), r3.as_str()), ("DEC-0026", "DEC-0027"));
    // A record, not applied: as a killed run leaves one.
    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved' WHERE id = '{unapplied}';"
    ));
    pair.sql(&format!(
        "UPDATE proposals SET status = 'applied' WHERE id = '{unrecorded}';"
    ));

    let (_, document) = shown(&pair, &cwd, &parent, &["--context"]);
    let items: Vec<(&str, &str)> = document["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            (
                item["id"].as_str().expect("an ID"),
                item["status"].as_str().expect("a status"),
            )
        })
        .collect();
    assert!(
        items.contains(&(unapplied.as_str(), "approved")),
        "{items:?}"
    );
    let listed: Vec<&str> = document["subtask_items"]
        .as_array()
        .expect("subtask_items")
        .iter()
        .map(|item| item["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(listed, [below.as_str()]);
    let bundle = &document["context"]["bundle"];
    assert_eq!(
        bundle["refs"],
        json!([T1, T2, r1, r2]),
        "M: open items taken, or subtask_items taken: {bundle}"
    );
    assert_eq!(
        *bundle,
        bundled(&pair, &cwd, &[T1, T2, &r1, &r2], DEFAULT_BUDGET).1
    );

    let (_, document) = shown(&pair, &cwd, &child, &["--context"]);
    assert_eq!(document["context"]["bundle"]["refs"], json!([T1, r3]));
}

// ------------------------------------------------------------- writes

/// The spec's writes rule: a context read writes nothing under the project root, no queue
/// row and no event.
#[test]
fn a_context_read_writes_nothing_but_the_data_directory() {
    let pair = foreword("cc-writes");
    let cwd = pair.linked.clone();
    let status = |dir: &Path| {
        pair.git_text(
            dir,
            &[
                "status",
                "--porcelain",
                "--ignored",
                "--untracked-files=all",
            ],
        )
    };
    let before = (
        status(&cwd),
        status(&pair.main),
        pair.queue().dump().expect("dump"),
    );
    for args in [
        &["card", "show", "C-0001", "--context"][..],
        &[
            "--json",
            "card",
            "show",
            "C-0001",
            "--context",
            "--history",
            "--marks",
        ],
    ] {
        spec(&pair, &cwd, args).code(0);
    }
    let after = (
        status(&cwd),
        status(&pair.main),
        pair.queue().dump().expect("dump"),
    );
    assert_eq!(after, before);
}
