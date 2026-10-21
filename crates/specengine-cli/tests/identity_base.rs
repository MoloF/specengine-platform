//! docs/features/hygiene-pass.md HY-1 (byte identity): a behaviour-free
//! refactor prints and writes what the base build does. Each case runs the
//! base `spec` (a build of the base commit, its path in
//! `SPECENGINE_BASE_SPEC`) and the workspace's `spec` over twin scratch
//! git copies made the same way (`lock_common::LockRepo`: every git
//! process sandboxed, fixed `GIT_{AUTHOR,COMMITTER}_{NAME,EMAIL,DATE}`, a
//! `HOME` of each twin's own) and compares, step by step, the exit code,
//! stdout, stderr, `spec.lock`'s bytes and `HEAD`. No golden: the base
//! binary is the oracle.
//!
//! The twins take turns at one path (`Scratch::at`): the base's twin is
//! made, run and removed, then the new one's at the same path, so every
//! path either prints (the terminal question's worktree, a queue item's
//! `worktree`) is the same by construction. The only clock either reads
//! is today's UTC date (`verified_at` of an accept on the terminal): a
//! case whose UTC date changes between its first and its last step fails
//! as such, to be run again. The one masked field: `spec review <PR>
//! --json`'s top-level `created_at` and `updated_at` values (the queue's
//! clock, seconds, no override) are replaced by `"<run time>"` once each,
//! after their shape is checked; the item's `drift` value is compared
//! as its raw bytes besides.
//!
//! Cases: (a) `fixtures/anchor-states` (`anchor_common::based`, the 18
//! `SCENARIOS` one by one and all at once): `spec verify`, `--json`,
//! `--staged`, `--node <node>`; (b) `fixtures/code-a`: `spec lock accept`
//! per bound node on a terminal, then a RON edit, a body edit and a node
//! removal committed: `spec verify`, `spec lock diff`, `spec propose
//! drift`, `spec review <PR> --json`, the RON entries accepted again; (c)
//! code-a under the golden lock with six tasks (one without a binding, Q3)
//! and dangling, look-alike and alias markers, a node removed (Q2):
//! `spec verify --task T` plain, `--json`, `--changed`; (d) `spec lock
//! rebase` on a terminal over the format-1 golden (RON entries, `AIM-01`'s
//! alias entry), the alias gone, and another `code_recipe`, as
//! `lock_rebase.rs` builds them; (e) `fixtures/verify-tests`' evidence
//! kiln (`tested_kiln::with_evidence`, `tested_common::lock_with` blocks)
//! in each `EvidenceState`: `spec verify`, `spec show <ID> --bindings`.
//!
//! Every test is `#[ignore]` (it needs the base binary):
//! `SPECENGINE_BASE_SPEC=<base spec> cargo nextest run -p specengine-cli
//! --test identity_base --run-ignored only`. `SPECENGINE_NEW_SPEC`, when
//! set, names the binary to compare instead of the workspace's (a copy of
//! the base: the harness's own determinism). Non-Latin characters in Rust
//! sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod anchor_common;
mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use common::{Run, SPEC, Scratch, one_json_document};
use lock_common::rebase::golden_text;
use lock_common::{LockRepo, PtyRun, Variant, golden_format_1_text};
use serde_json::{Value, json};
use verify_common::{
    BOUNDS, COUNTER, COUNTER_MD, LIMITS, golden_file, new_task, plan_task, set_mode,
};
use verify_tests_common::Kiln;

/// The variable naming the base `spec` binary.
const BASE_VAR: &str = "SPECENGINE_BASE_SPEC";

/// The variable naming a binary to stand for the workspace's (optional: a
/// copy of the base, the harness's own determinism).
const NEW_VAR: &str = "SPECENGINE_NEW_SPEC";

/// The binary compared with the base: the workspace's `spec`, unless
/// `SPECENGINE_NEW_SPEC` names another.
fn new_spec() -> PathBuf {
    std::env::var_os(NEW_VAR).map_or_else(|| PathBuf::from(SPEC), PathBuf::from)
}

/// The base `spec`: `SPECENGINE_BASE_SPEC`, an existing file that is not
/// the binary it is compared with.
fn base_spec() -> PathBuf {
    let Some(path) = std::env::var_os(BASE_VAR) else {
        panic!(
            "{BASE_VAR} is unset: it names the base `spec` binary (a build of the base \
             commit) the workspace's `spec` is compared with; run \
             `{BASE_VAR}=<base spec> cargo nextest run -p specengine-cli --test identity_base \
             --run-ignored only`"
        );
    };
    let path = PathBuf::from(path);
    assert!(
        path.is_file(),
        "{BASE_VAR}={}: no such file (the base `spec` binary)",
        path.display()
    );
    let canonical = |path: &Path| fs::canonicalize(path).expect("a canonical path");
    assert_ne!(
        canonical(&path),
        canonical(&new_spec()),
        "{BASE_VAR} names the binary it is compared with: nothing would be compared"
    );
    path
}

/// Today's UTC day number (days since the epoch).
fn utc_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after the epoch")
        .as_secs()
        / 86_400
}

// ------------------------------------------------------------- the twins

/// What one step showed: a `spec` run's exit, stdout and stderr (or a
/// value read off one), and the repository's lock and `HEAD` after it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    step: String,
    exit: i32,
    stdout: String,
    stderr: String,
    lock: Option<Vec<u8>>,
    head: Option<String>,
}

/// One side of a case: the binary it runs, what each step showed and the
/// case's premises that failed (what the case claims to exercise).
struct Twin {
    program: PathBuf,
    seen: RefCell<Vec<Seen>>,
    failed: RefCell<Vec<String>>,
}

impl Twin {
    fn program(&self) -> &OsStr {
        self.program.as_os_str()
    }

    fn record(&self, repo: &LockRepo, step: String, exit: i32, stdout: &str, stderr: &str) {
        self.seen.borrow_mut().push(Seen {
            step,
            exit,
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
            lock: repo.lock(),
            head: repo.git.head(&repo.root),
        });
    }

    /// A premise of the case: judged on the base's side (where it fails
    /// the case exercises less than it claims); the new side's are its
    /// steps' bytes.
    fn premise(&self, holds: bool, what: &str) {
        if !holds {
            self.failed.borrow_mut().push(what.to_owned());
        }
    }

    /// The repository as made (no run): its lock and `HEAD`.
    fn setup(&self, repo: &LockRepo, what: &str) {
        self.record(repo, format!("setup: {what}"), 0, "", "");
    }

    /// `spec args` from the root, stdin null, not recorded.
    fn quiet(&self, repo: &LockRepo, args: &[&str]) -> Run {
        repo.spec_piped_with(self.program(), &repo.root, args)
    }

    /// `spec args` from the root, stdin null, recorded.
    fn run(&self, repo: &LockRepo, args: &[&str]) -> Run {
        let run = self.quiet(repo, args);
        self.record(
            repo,
            format!("spec {}", args.join(" ")),
            run.code,
            &run.stdout,
            &run.stderr,
        );
        run
    }

    /// `spec args` from the root on a terminal, `answer` typed at its
    /// question, recorded.
    fn terminal(&self, repo: &LockRepo, args: &[&str], answer: &str) -> PtyRun {
        let run = repo.spec_pty_with(self.program(), &repo.root, args, Some(answer));
        self.record(
            repo,
            format!("spec {} (terminal, {answer})", args.join(" ")),
            run.code,
            &run.stdout,
            &run.stderr,
        );
        run
    }

    /// [`Twin::terminal`] answering `y`, which must commit: exit 0 after
    /// the question, `HEAD` moved (a premise).
    fn committed(&self, repo: &LockRepo, args: &[&str]) {
        let before = repo.git.head(&repo.root);
        let run = self.terminal(repo, args, "y");
        self.premise(
            run.code == 0 && run.asked && repo.git.head(&repo.root) != before,
            &format!("spec {} commits: {}", args.join(" "), run.show()),
        );
    }

    /// `spec args` (ending `--json`) recorded; its stdout one JSON
    /// document (a premise).
    fn json(&self, repo: &LockRepo, args: &[&str]) -> Value {
        let run = self.run(repo, args);
        match one_json_document(&run.stdout) {
            Ok(document) => document,
            Err(problem) => {
                self.premise(false, &format!("spec {args:?}: {problem}\n{}", run.show()));
                Value::Null
            }
        }
    }

    /// `run` recorded with `stdout` in place of its own (a value read off
    /// it).
    fn derived(&self, repo: &LockRepo, step: String, run: &Run, stdout: &str) {
        self.record(repo, step, run.code, stdout, &run.stderr);
    }
}

/// Runs `case` for the base binary and then for the workspace's, each over
/// its own twin made at one path, and compares every step.
fn identical(name: &str, case: impl Fn(&Twin, Scratch)) {
    let base = base_spec();
    let at = std::env::temp_dir().join(format!(
        "specengine-cli-identity-{name}-{}",
        std::process::id()
    ));
    let day = utc_day();
    let side = |program: &Path| {
        let twin = Twin {
            program: program.to_path_buf(),
            seen: RefCell::new(Vec::new()),
            failed: RefCell::new(Vec::new()),
        };
        case(&twin, Scratch::at(&at));
        assert!(
            !at.exists(),
            "{name}: the twin's scratch is removed before the other twin is made"
        );
        (twin.seen.into_inner(), twin.failed.into_inner())
    };
    let (base_seen, failed) = side(&base);
    assert!(
        failed.is_empty(),
        "{name}: premises failed on the base's side (the case exercises less than it \
         claims):\n{}",
        failed.join("\n")
    );
    let (new_seen, _) = side(&new_spec());
    assert_eq!(
        utc_day(),
        day,
        "{name}: the UTC date changed during the case (an accept's `verified_at`); run it again"
    );
    compare(name, &base_seen, &new_seen);
    eprintln!("{name}: {} steps byte-identical", base_seen.len());
}

/// The first byte where `base` and `new` part, with both sides around it.
fn first_difference(base: &[u8], new: &[u8]) -> String {
    let at = base
        .iter()
        .zip(new)
        .position(|(b, n)| b != n)
        .unwrap_or(base.len().min(new.len()));
    let line = base[..at].iter().filter(|&&byte| byte == b'\n').count() + 1;
    let window = |bytes: &[u8]| {
        let from = at.saturating_sub(120);
        let to = (at + 120).min(bytes.len());
        String::from_utf8_lossy(&bytes[from.min(to)..to]).into_owned()
    };
    format!(
        "byte {at} (line {line}; {} vs {} bytes)\n      base: {:?}\n      new:  {:?}",
        base.len(),
        new.len(),
        window(base),
        window(new)
    )
}

/// Every step of `base` and `new` alike, else a failure naming each part.
fn compare(name: &str, base: &[Seen], new: &[Seen]) {
    let mut problems = Vec::new();
    if base.len() != new.len() {
        problems.push(format!(
            "the step counts differ: base {}, new {}",
            base.len(),
            new.len()
        ));
    }
    for (index, (b, n)) in base.iter().zip(new).enumerate() {
        if b.step != n.step {
            problems.push(format!(
                "step {index}: base {:?}, new {:?}; the cases parted",
                b.step, n.step
            ));
            break;
        }
        let step = &b.step;
        if b.exit != n.exit {
            problems.push(format!(
                "step {index} `{step}`: exit base {}, new {}",
                b.exit, n.exit
            ));
        }
        for (what, bb, nb) in [
            ("stdout", b.stdout.as_bytes(), n.stdout.as_bytes()),
            ("stderr", b.stderr.as_bytes(), n.stderr.as_bytes()),
        ] {
            if bb != nb {
                problems.push(format!(
                    "step {index} `{step}`: {what} at {}",
                    first_difference(bb, nb)
                ));
            }
        }
        match (&b.lock, &n.lock) {
            (Some(bl), Some(nl)) if bl != nl => problems.push(format!(
                "step {index} `{step}`: spec.lock at {}",
                first_difference(bl, nl)
            )),
            (bl, nl) if bl.is_some() != nl.is_some() => problems.push(format!(
                "step {index} `{step}`: spec.lock exists: base {}, new {}",
                bl.is_some(),
                nl.is_some()
            )),
            _ => {}
        }
        if b.head != n.head {
            problems.push(format!(
                "step {index} `{step}`: HEAD base {:?}, new {:?}",
                b.head, n.head
            ));
        }
    }
    let shown: Vec<&String> = problems.iter().take(12).collect();
    assert!(
        problems.is_empty(),
        "{name}: the base and the workspace's `spec` differ in {} place(s) over {} step(s):\n{}{}",
        problems.len(),
        base.len().min(new.len()),
        shown
            .iter()
            .map(|problem| format!("  - {problem}\n"))
            .collect::<String>(),
        if problems.len() > shown.len() {
            format!("  ... and {} more\n", problems.len() - shown.len())
        } else {
            String::new()
        }
    );
}

// ------------------------------------------------------------ raw JSON

/// The index just past the JSON string opening at `at`.
fn string_end(bytes: &[u8], at: usize) -> usize {
    let mut index = at + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    panic!("an unterminated JSON string at byte {at}");
}

/// The index just past the JSON value starting at `at`.
fn value_end(bytes: &[u8], at: usize) -> usize {
    match bytes[at] {
        b'"' => string_end(bytes, at),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut index = at;
            while index < bytes.len() {
                match bytes[index] {
                    b'"' => {
                        index = string_end(bytes, index);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return index + 1;
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
            panic!("an unclosed JSON value at byte {at}");
        }
        _ => {
            let mut index = at;
            while index < bytes.len() && !b",}] \n".contains(&bytes[index]) {
                index += 1;
            }
            index
        }
    }
}

/// The byte range of the value of `key` in the top-level object of the
/// JSON `document`, as written.
fn top_field(document: &str, key: &str) -> Option<Range<usize>> {
    let bytes = document.as_bytes();
    let wanted = format!("\"{key}\"");
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let end = string_end(bytes, index);
                if depth == 1 && document[index..end] == wanted && bytes.get(end) == Some(&b':') {
                    let start = end + 1;
                    return Some(start..value_end(bytes, start));
                }
                index = end;
                continue;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    None
}

/// `document` with the top-level `created_at` and `updated_at` values,
/// each a `"YYYY-MM-DDTHH:MM:SSZ"` string, replaced by `"<run time>"`.
fn queue_clock_masked(document: &str) -> String {
    let mut text = document.to_owned();
    for key in ["created_at", "updated_at"] {
        let range = top_field(&text, key).unwrap_or_else(|| panic!("no {key}: {document}"));
        let value = &text[range.clone()];
        let shape: Vec<bool> = value.bytes().map(|byte| byte.is_ascii_digit()).collect();
        let digits = [1, 2, 3, 4, 6, 7, 9, 10, 12, 13, 15, 16, 18, 19];
        assert!(
            value.len() == 22
                && value.starts_with('"')
                && value.ends_with("Z\"")
                && digits.iter().all(|&at| shape[at]),
            "{key} is no queue timestamp: {value}"
        );
        text.replace_range(range, "\"<run time>\"");
    }
    // Only those two values changed: the masked text is the document with
    // the two keys' values replaced, key for key.
    let mut want: Value = serde_json::from_str(document).expect("a JSON document");
    want["created_at"] = json!("<run time>");
    want["updated_at"] = json!("<run time>");
    assert_eq!(
        serde_json::from_str::<Value>(&text).expect("the masked document"),
        want
    );
    text
}

/// The raw bytes of the top-level `key`'s value, checked against the
/// parsed document's.
fn raw_field<'a>(document: &'a str, key: &str) -> &'a str {
    let range = top_field(document, key).unwrap_or_else(|| panic!("no {key}: {document}"));
    let raw = &document[range];
    let parsed: Value = serde_json::from_str(document).expect("a JSON document");
    assert_eq!(
        serde_json::from_str::<Value>(raw).expect("a JSON value"),
        parsed[key],
        "{key} read as written"
    );
    raw
}

// ------------------------------------------------------- (a) anchor-states

/// `spec verify`, `--json`, and `--node <node>` (text and JSON) of each of
/// `nodes`; then everything staged, `--staged` and `--staged --json`. The
/// anchor states of plain `--json`'s rows.
fn anchor_runs(twin: &Twin, repo: &LockRepo, nodes: &[String]) -> Vec<String> {
    twin.run(repo, &["verify"]);
    let plain = twin.json(repo, &["verify", "--json"]);
    for node in nodes {
        twin.run(repo, &["verify", "--node", node]);
        twin.json(repo, &["verify", "--node", node, "--json"]);
    }
    repo.git(&["add", "-A"]);
    twin.run(repo, &["verify", "--staged"]);
    twin.json(repo, &["verify", "--staged", "--json"]);
    plain["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["anchor"]["state"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The worktree and the index back at `HEAD`.
fn restore(repo: &LockRepo) {
    repo.git(&["reset", "-q", "--hard", "HEAD"]);
    repo.git(&["clean", "-fdq"]);
    assert!(repo.porcelain().is_empty(), "restored");
}

/// The nodes `expected.json` gives scenario `name`.
fn scenario_nodes(name: &str) -> Vec<String> {
    anchor_common::scenario(name)["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node.as_str().expect("a node").to_owned())
        .collect()
}

/// The anchor base (every node but `KITE-DECOY` accepted through the
/// library at its fixed clock) at the twin's path.
fn anchor_base(twin: &Twin, scratch: Scratch) -> LockRepo {
    let repo = anchor_common::based(
        LockRepo::of_in(scratch, anchor_common::FIXTURE, "repo", &[]),
        |_| {},
    );
    twin.setup(&repo, "the anchor base");
    repo
}

/// HY-1 (a), the baseline and all 18 scenarios at once: every node's
/// `--node` run (`KITE-DECOY`'s too).
#[test]
#[ignore = "needs the base `spec` binary in SPECENGINE_BASE_SPEC"]
fn anchor_states_baseline_and_all_scenarios_at_once() {
    identical("anchors-all", |twin, scratch| {
        let repo = anchor_base(twin, scratch);
        let mut every: Vec<String> = anchor_common::NODES
            .iter()
            .map(|node| (*node).to_owned())
            .collect();
        every.push("KITE-DECOY".to_owned());
        anchor_runs(twin, &repo, &every);
        restore(&repo);
        for name in anchor_common::SCENARIOS {
            anchor_common::change(&repo, name);
        }
        let anchors = anchor_runs(twin, &repo, &every);
        for state in ["moved", "ambiguous", "lost"] {
            twin.premise(
                anchors.iter().any(|anchor| anchor == state),
                &format!("all scenarios at once: a `{state}` row"),
            );
        }
        restore(&repo);
    });
}

/// HY-1 (a), each of the 18 scenarios alone (R1: a pairing order slip
/// moves `moved` or `ambiguous`): its nodes' `--node` runs.
#[test]
#[ignore = "needs the base `spec` binary in SPECENGINE_BASE_SPEC"]
fn anchor_states_each_scenario() {
    identical("anchors-each", |twin, scratch| {
        let repo = anchor_base(twin, scratch);
        let mut seen = Vec::new();
        for name in anchor_common::SCENARIOS {
            anchor_common::change(&repo, name);
            seen.extend(anchor_runs(twin, &repo, &scenario_nodes(name)));
            restore(&repo);
        }
        for state in ["moved", "ambiguous", "lost"] {
            twin.premise(
                seen.iter().any(|anchor| anchor == state),
                &format!("the scenarios alone: a `{state}` row"),
            );
        }
    });
}

// ------------------------------------------------- (b) code-a, accept, drift

/// Every node and alias of code-a a marker names, and two no marker binds
/// lockably (each accept's refusal compared too).
const ACCEPTED: [&str; 8] = [
    "GOAL-01",
    "LAW-BOUND",
    "LAW-STEP",
    "PIECE-COUNTER",
    "PREM-001",
    "AIM-01",
    "cap-check/PASS-01",
    "LAW-ODD",
];

/// HY-1 (b): code-a's nodes accepted on the terminal (RON entries among
/// them, `verified_at` today), then a RON value edit, a body edit and a
/// node's file removed, committed: verify, each node's lock diff, drift
/// raised twice (created, then hits), each item's review, and the RON
/// entries accepted again.
#[test]
#[ignore = "needs the base `spec` binary in SPECENGINE_BASE_SPEC"]
fn code_a_accept_and_drift() {
    identical("code-a-drift", |twin, scratch| {
        let repo = LockRepo::of_in(scratch, "code-a", "repo", &[]);
        twin.setup(&repo, "code-a");
        for node in &ACCEPTED[..5] {
            twin.committed(&repo, &["lock", "accept", node]);
        }
        for node in &ACCEPTED[5..] {
            twin.terminal(&repo, &["lock", "accept", node], "y");
        }
        twin.premise(
            repo.lock_text().contains("kind = \"ron\""),
            "RON entries accepted",
        );
        twin.run(&repo, &["verify"]);
        twin.run(&repo, &["verify", "--json"]);
        repo.replace(LIMITS, "cap: 10,", "cap: 12,");
        repo.replace(
            COUNTER,
            "        self.count = next.min(CAP);\n",
            "        self.count = next.min(CAP).max(1);\n",
        );
        fs::remove_file(repo.root.join("docs/records/PREM-001.md")).expect("PREM-001 removed");
        repo.commit_all("a RON value, a body, a node gone");
        twin.setup(&repo, "the edits committed");
        twin.run(&repo, &["verify"]);
        twin.run(&repo, &["verify", "--json"]);
        for node in &ACCEPTED[..6] {
            let diff = twin.run(&repo, &["lock", "diff", node]);
            if *node == "LAW-BOUND" {
                twin.premise(
                    diff.code == 0 && diff.stdout.contains("| ron root.cap\n"),
                    &format!("LAW-BOUND's RON entry moved: {}", diff.show()),
                );
            }
            twin.run(&repo, &["lock", "diff", node, "--json"]);
        }
        let raised = twin.run(&repo, &["propose", "drift"]);
        let items: Vec<String> = raised
            .stdout
            .lines()
            .filter_map(|line| line.strip_prefix("created "))
            .map(|rest| rest.split(' ').next().unwrap_or_default().to_owned())
            .collect();
        twin.premise(
            !items.is_empty(),
            &format!("drift raised: {}", raised.show()),
        );
        for item in &items {
            let review = twin.quiet(&repo, &["review", item, "--json"]);
            let document = one_json_document(&review.stdout).unwrap_or(Value::Null);
            twin.premise(
                review.code == 0 && document["kind"] == "drift",
                &format!("review {item}: {}", review.show()),
            );
            if document.is_null() {
                let step = format!("spec review {item} --json");
                twin.derived(&repo, step, &review, &review.stdout);
                continue;
            }
            let drift = raw_field(&review.stdout, "drift");
            twin.derived(
                &repo,
                format!("spec review {item} --json: drift"),
                &review,
                drift,
            );
            twin.derived(
                &repo,
                format!("spec review {item} --json: the document, its clock masked"),
                &review,
                &queue_clock_masked(&review.stdout),
            );
        }
        twin.run(&repo, &["propose", "drift"]);
        twin.committed(&repo, &["lock", "accept", "LAW-BOUND"]);
        twin.committed(&repo, &["lock", "accept", "PIECE-COUNTER"]);
        twin.json(&repo, &["verify", "--json"]);
    });
}

// ------------------------------------------------------ (c) code-a, --task

/// `verify_task.rs`' RD-3 edits: GOAL-01's alias marker turned a
/// look-alike (a Cyrillic O), LAW-BOUND's Display impl removed (a
/// lock-only row), PIECE-COUNTER's `//!` marker in the test file removed,
/// a typo (`LAW-BUOND`, dangling) and an alias marker added; its drift
/// edits besides (the cap, its RON value, LAW-BOUND's text, a file not
/// UTF-8, `enforce-introduced`).
fn task_edits(repo: &LockRepo) {
    repo.replace(
        COUNTER,
        "@implements AIM-01@1",
        "@implements G\u{41E}AL-01@1",
    );
    repo.replace(
        COUNTER,
        "impl fmt::Display for Tally {\n    // @implements PIECE-COUNTER#LAW-BOUND@2\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        write!(f, \"{}\", self.count)\n    }\n}\n",
        "",
    );
    repo.replace(BOUNDS, "//! @verifies PIECE-COUNTER\n", "");
    repo.replace(
        COUNTER,
        "pub fn total(",
        "// @implements LAW-BUOND\npub fn total(",
    );
    repo.append(
        COUNTER,
        "\n// @implements AIM-01\npub fn spare() -> u32 {\n    1\n}\n",
    );
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    repo.write("crates/tally/src/odd.rs", b"// \xff\n".as_slice());
    set_mode(&repo.root, "enforce-introduced");
}

/// Plain verify, then each task's `--task` run: text, `--json`,
/// `--changed`, `--changed --json`; an unknown task. The `--task` runs'
/// stdout and stderr, joined.
fn task_runs(twin: &Twin, repo: &LockRepo, tasks: &[String]) -> String {
    twin.run(repo, &["verify"]);
    twin.json(repo, &["verify", "--json"]);
    let mut said = String::new();
    for task in tasks {
        for extra in [&[][..], &["--changed"]] {
            let mut args = vec!["verify", "--task", task];
            args.extend_from_slice(extra);
            let run = twin.run(repo, &args);
            said.push_str(&run.stdout);
            said.push_str(&run.stderr);
            args.push("--json");
            twin.json(repo, &args);
        }
    }
    twin.run(repo, &["verify", "--task", "T-0099"]);
    said
}

/// HY-1 (c): code-a under the golden lock, six tasks made through the
/// library at its fixed clock (PIECE-COUNTER's document, a feature-scoped
/// criterion, `limits.md` with no binding (Q3), PREM-001 planned with
/// GOAL-01, GOAL-01, LAW-BOUND), judged as committed, with
/// [`task_edits`], and with GOAL-01's record removed (its entries
/// `node_gone`, its markers dangling: Q2).
#[test]
#[ignore = "needs the base `spec` binary in SPECENGINE_BASE_SPEC"]
fn code_a_verify_task() {
    identical("code-a-task", |twin, scratch| {
        let repo = LockRepo::of_in(scratch, "code-a", "repo", &[]);
        repo.write(
            "spec.lock",
            fs::read(golden_file()).expect("fixtures/spec-lock/code-a.lock"),
        );
        repo.commit_all("the lock");
        let premise = new_task(&repo, &["PREM-001"]);
        plan_task(&repo, &premise, &["GOAL-01"], &["docs/records/PREM-001.md"]);
        let tasks = vec![
            new_task(&repo, &["PIECE-COUNTER"]),
            new_task(&repo, &["cap-check/PASS-01"]),
            new_task(&repo, &["docs/spec/limits.md"]),
            premise,
            new_task(&repo, &["GOAL-01"]),
            new_task(&repo, &["LAW-BOUND"]),
        ];
        twin.setup(&repo, &format!("the golden lock, tasks {tasks:?}"));
        let mut said = task_runs(twin, &repo, &tasks);
        task_edits(&repo);
        said.push_str(&task_runs(twin, &repo, &tasks));
        fs::remove_file(repo.root.join("docs/records/GOAL-01.md")).expect("GOAL-01 removed");
        said.push_str(&task_runs(twin, &repo, &tasks));
        for note in [
            "rows name no node now",
            "have no binding",
            "names nothing now",
        ] {
            twin.premise(said.contains(note), &format!("a `--task` note: {note:?}"));
        }
    });
}

// ----------------------------------------------------- (d) lock rebase

/// code-a with `lock` committed as `the lock`, then `edit` (file, from,
/// to) committed when given: verify, `spec lock rebase` (`extra`) on the
/// terminal, verify again, a second rebase (nothing to change), LAW-BOUND's
/// and GOAL-01's lock diff.
fn rebase_case(
    name: &str,
    lock: &str,
    edit: Option<(&'static str, &'static str, &'static str)>,
    extra: &'static [&'static str],
) {
    identical(name, |twin, scratch| {
        let repo = LockRepo::of_in(scratch, "code-a", "repo", &[]);
        repo.write("spec.lock", lock);
        repo.commit_all("the lock");
        if let Some((file, from, to)) = edit {
            repo.replace(file, from, to);
            repo.commit_all("the edit");
        }
        twin.setup(&repo, "the lock committed");
        twin.json(&repo, &["verify", "--json"]);
        let mut args = vec!["lock", "rebase"];
        args.extend_from_slice(extra);
        twin.committed(&repo, &args);
        twin.run(&repo, &["verify"]);
        twin.json(&repo, &["verify", "--json"]);
        let again = twin.terminal(&repo, &["lock", "rebase"], "y");
        twin.premise(
            again.code == 0 && !again.asked,
            &format!("a second rebase changes nothing: {}", again.show()),
        );
        twin.run(&repo, &["lock", "diff", "LAW-BOUND"]);
        twin.run(&repo, &["lock", "diff", "GOAL-01"]);
    });
}

/// HY-1 (d): the format-1 golden (its RON entries `root` and `root.cap`,
/// GOAL-01's entry bound through the alias marker `AIM-01`) rebased, text
/// and `--json`; the same with `AIM` no longer GOAL's alias (the entry no
/// blob marker gives); accept's format-2 golden under another grammar's
/// `code_recipe`.
#[test]
#[ignore = "needs the base `spec` binary in SPECENGINE_BASE_SPEC"]
fn lock_rebase_over_ron_and_alias_entries() {
    let format_1 = golden_format_1_text();
    rebase_case("rebase-format-1", &format_1, None, &[]);
    rebase_case("rebase-format-1-json", &format_1, None, &["--json"]);
    rebase_case(
        "rebase-alias-gone",
        &format_1,
        Some(("specengine.toml", ", aliases_from = [\"AIM\"] }", " }")),
        &[],
    );
    rebase_case(
        "rebase-other-code",
        &Variant::OtherCode.text(&golden_text()),
        None,
        &[],
    );
}

// ------------------------------------------------------ (e) the kiln

/// The evidence kiln's nodes and sections a read shows.
const KILN_REFS: [&str; 5] = [
    "KILN-FIRING",
    "KILN-RAMP",
    "KILN-HOLD",
    "CONE-01",
    "glaze-check/GLZ-01",
];

/// HY-1 (e): `fixtures/verify-tests`' evidence kiln (its four blocks and
/// a gone node's, `tested_common::lock_with`) in each `EvidenceState`:
/// `spec verify`, `--json`, `spec show <ID> --bindings` (text and JSON),
/// `spec verify --node KILN-RAMP --json`.
#[test]
#[ignore = "needs the base `spec` binary in SPECENGINE_BASE_SPEC"]
fn kiln_evidence_reads() {
    identical("kiln", |twin, scratch| {
        let kiln = tested_kiln::with_evidence(Kiln::with_runner(LockRepo::of_in(
            scratch,
            verify_tests_common::FIXTURE,
            "repo",
            &[],
        )));
        twin.setup(&kiln.repo, "the evidence kiln");
        for state in tested_kiln::EVIDENCE_STATES {
            state.write(&kiln);
            twin.setup(&kiln.repo, &format!("{state:?}"));
            twin.run(&kiln.repo, &["verify"]);
            let plain = twin.json(&kiln.repo, &["verify", "--json"]);
            twin.premise(
                plain["tested"]
                    .as_array()
                    .is_some_and(|tested| tested.len() == 5),
                &format!("{state:?}: five `tested` entries"),
            );
            for reference in KILN_REFS {
                twin.run(&kiln.repo, &["show", reference, "--bindings"]);
                twin.json(&kiln.repo, &["show", reference, "--bindings", "--json"]);
            }
            twin.json(&kiln.repo, &["verify", "--node", "KILN-RAMP", "--json"]);
            state.undo(&kiln);
        }
    });
}
