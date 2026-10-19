//! docs/features/verify-tests.md (8a-1) RD-6 and TE-3: the runner
//! ("Runner"): one process group per invocation, a time-out killing the
//! whole group (a grandchild that ignores `SIGTERM` too), stdin null, the
//! environment the owner's minus git's repository variables, stdout and
//! stderr one pipe copied live to `spec`'s stderr and hashed whole, never
//! on stdout. Scratch repositories of `fixtures/verify-tests` (P10) with
//! the fake runner (TE-1). Where a file descriptor is the subject the test
//! re-runs its own binary as the `spec` stand-in (`rerun`): stdin a pipe
//! holding bytes, stdout and stderr read here. Every test that can leave a
//! process holds a `Reaper`, which kills what is left even when an
//! assertion fails; every fake runner ends itself (`alarm`). Each test
//! names the mutation that turns it red (`M:`).

mod common;
mod lock_common;
mod verify_tests_common;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specengine_store::b3_hash;
use verify_tests_common::{
    HOLD, HOLD_TEST, Kiln, LOCAL_GIT_VARS, RAMP_STOPS, RAMP_STOPS_TEST, TEMPLATE, child_role,
    rerun, survivors,
};

/// A re-run's limit: a run here takes seconds.
const CHILD_LIMIT: Duration = Duration::from_secs(90);

/// The flood's bytes (the runner's `flood`): 1 KiB of `a`..`z` cycling,
/// 1024 times.
fn flood() -> Vec<u8> {
    let block: Vec<u8> = (0..1024u32)
        .map(|i| b'a' + u8::try_from(i % 26).expect("below 26"))
        .collect();
    block.repeat(1024)
}

/// `needle` occurs in `haystack` as one contiguous run.
fn contains_run(haystack: &[u8], needle: &[u8]) -> bool {
    let head = &needle[..64.min(needle.len())];
    haystack
        .windows(head.len())
        .enumerate()
        .filter(|(_, window)| *window == head)
        .any(|(at, _)| haystack.get(at..at + needle.len()) == Some(needle))
}

/// The event row of `symbol`.
fn event_test(event: &Value, symbol: &str) -> Value {
    let found: Vec<&Value> = event["tests"]
        .as_array()
        .expect("tests")
        .iter()
        .filter(|test| test["symbol"] == symbol)
        .collect();
    assert_eq!(found.len(), 1, "{symbol}: {event}");
    found[0].clone()
}

/// RD-6, TE-3 ("Runner": overrun, every exit): a test whose runner forks a
/// grandchild that ignores `SIGTERM`, then hangs past a 2 s timeout, is
/// `timed out`, and neither the runner nor the grandchild outlives `spec`
/// (the group gets `SIGKILL` once the leader's exit is seen). M: only the
/// child killed.
#[test]
fn no_grandchild_outlives_a_timeout() {
    let kiln = Kiln::new("vt-grandchild");
    let _reaper = kiln.reaper();
    kiln.set_verify(&TEMPLATE, Some(2));
    kiln.plan(&[(HOLD_TEST, "grandchild")]);
    let started = Instant::now();
    let run = kiln.run();
    assert!(
        started.elapsed() < Duration::from_secs(25),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(run.row(HOLD)["outcome"], "timed_out", "{}", run.show());
    let noted = kiln.noted_pids();
    let roles: Vec<&str> = noted.iter().map(|(role, _)| role.as_str()).collect();
    assert!(
        roles.contains(&"leader") && roles.contains(&"grandchild"),
        "{noted:?}"
    );
    let pids: Vec<u32> = noted.iter().map(|(_, pid)| *pid).collect();
    let left = survivors(&pids, Duration::from_secs(3));
    assert!(left.is_empty(), "the whole group is gone: {left:?}");
    let leader = kiln
        .log()
        .into_iter()
        .find(|call| call.args.last().is_some_and(|last| last == HOLD_TEST))
        .expect("the hanging test ran");
    assert_eq!(leader.pgid, leader.pid, "the runner leads its own group");
}

/// RD-6 ("Runner": stdin null): the runner reads end-of-file at once from
/// `/dev/null` even when `spec`'s own stdin is a pipe holding bytes. M:
/// stdin inherited.
#[test]
fn runner_stdin_is_null() {
    if child_role() {
        return;
    }
    let kiln = Kiln::new("vt-stdin");
    let _reaper = kiln.reaper();
    kiln.plan(&[(RAMP_STOPS_TEST, "stdin")]);
    let child = rerun(
        &kiln,
        "runner_stdin_is_null",
        None,
        b"VT-STDIN-LEAK\n",
        CHILD_LIMIT,
    );
    assert!(child.status.success(), "{}", child.show());
    assert_eq!(child.code, 1, "{}", child.show());
    let seen = String::from_utf8(kiln.runner_file("stdin")).expect("UTF-8");
    assert_eq!(seen, "len=0 null=1\n", "the runner's stdin");
}

/// RD-6 ("Runner": environment, the owner's decision in "Risks"): the
/// runner gets the owner's variables (`HOME`, `PATH`, a token, an agent
/// socket) minus every variable `git rev-parse --local-env-vars` lists,
/// and `GIT_TERMINAL_PROMPT=0`, even when the caller's environment sets
/// `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and their kin. M:
/// `GIT_DIR` kept.
#[test]
fn runner_env_has_no_git_repository_variables() {
    let kiln = Kiln::new("vt-env");
    kiln.plan(&[(RAMP_STOPS_TEST, "env")]);
    let root = kiln.root().to_path_buf();
    let git_dir = root.join(".git");
    let path = |dir: &Path| dir.display().to_string();
    let extra: Vec<(String, String)> = vec![
        ("GIT_DIR".into(), path(&git_dir)),
        ("GIT_WORK_TREE".into(), path(&root)),
        ("GIT_INDEX_FILE".into(), path(&git_dir.join("index"))),
        (
            "GIT_OBJECT_DIRECTORY".into(),
            path(&git_dir.join("objects")),
        ),
        ("GIT_COMMON_DIR".into(), path(&git_dir)),
        ("GIT_CONFIG_PARAMETERS".into(), "'vt.leak=1'".into()),
        ("GIT_PREFIX".into(), "crates/".into()),
        ("VT_OWNER_TOKEN".into(), "kept".into()),
        ("SSH_AUTH_SOCK".into(), "/nonexistent/vt-agent.sock".into()),
    ];
    let extra_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let run = kiln.run_with(&root, None, true, &extra_refs);
    run.code(1);
    assert_eq!(run.row(RAMP_STOPS)["outcome"], "passed", "{}", run.show());
    let env: BTreeMap<String, String> = kiln
        .runner_file("env")
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let text = String::from_utf8(entry.to_vec()).expect("UTF-8");
            let (name, value) = text.split_once('=').expect("NAME=VALUE");
            (name.to_owned(), value.to_owned())
        })
        .collect();
    for name in LOCAL_GIT_VARS {
        assert!(
            !env.contains_key(name),
            "{name} reached the runner: {env:#?}"
        );
    }
    assert_eq!(
        env.get("GIT_TERMINAL_PROMPT").map(String::as_str),
        Some("0")
    );
    assert_eq!(env.get("VT_OWNER_TOKEN").map(String::as_str), Some("kept"));
    assert_eq!(
        env.get("SSH_AUTH_SOCK").map(String::as_str),
        Some("/nonexistent/vt-agent.sock")
    );
    assert_eq!(
        env.get("HOME").map(String::as_str),
        Some(kiln.repo.git.home().display().to_string().as_str())
    );
    assert!(env.contains_key("PATH"), "{env:#?}");
}

/// RD-6 ("Runner": nothing reaches stdout, no log file): what the runner
/// writes on its stdout and stderr is copied live to `spec`'s stderr and
/// never reaches `spec`'s stdout, text or JSON. M: output on stdout
/// (the child's stdout inherited).
#[test]
fn runner_output_never_reaches_stdout() {
    if child_role() {
        return;
    }
    let kiln = Kiln::new("vt-stdout");
    let _reaper = kiln.reaper();
    kiln.plan(&[(RAMP_STOPS_TEST, "sentinel")]);
    let child = rerun(
        &kiln,
        "runner_output_never_reaches_stdout",
        None,
        b"",
        CHILD_LIMIT,
    );
    assert!(child.status.success(), "{}", child.show());
    for sentinel in ["VT-RUNNER-STDOUT-SENTINEL", "VT-RUNNER-STDERR-SENTINEL"] {
        let stdout = String::from_utf8_lossy(&child.stdout);
        assert!(
            !stdout.contains(sentinel),
            "{sentinel} on spec's stdout:\n{stdout}"
        );
        assert!(
            !child.text.contains(sentinel) && !child.json_text.contains(sentinel),
            "{sentinel} in the printed outcome"
        );
        assert!(
            String::from_utf8_lossy(&child.stderr).contains(sentinel),
            "{sentinel} copied live to spec's stderr:\n{}",
            child.show()
        );
    }
    let entries: Vec<String> = std::fs::read_dir(kiln.root())
        .expect("the root")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !entries.iter().any(|name| name.ends_with(".log")),
        "no log file: {entries:?}"
    );
}

/// RD-6, TE-3 ("Runner", P9): a runner that floods 1 MiB on its stdout and
/// fails is read whole while it writes (no dead-lock on a full pipe): the
/// event's `output_hash` is the BLAKE3 of the whole stream (`b3_hash`'s
/// form), `tail` its last 4 096 bytes; the flood is copied to `spec`'s
/// stderr in one piece and never reaches its stdout. M: only the tail
/// hashed.
#[test]
fn output_hash_covers_the_whole_stream() {
    if child_role() {
        return;
    }
    let kiln = Kiln::new("vt-flood");
    let _reaper = kiln.reaper();
    kiln.plan(&[(RAMP_STOPS_TEST, "flood")]);
    let child = rerun(
        &kiln,
        "output_hash_covers_the_whole_stream",
        None,
        b"",
        CHILD_LIMIT,
    );
    assert!(child.status.success(), "{}", child.show());
    let bytes = flood();
    let event = kiln.one_event();
    let row = event_test(&event, RAMP_STOPS);
    assert_eq!(
        (&row["outcome"], &row["exit"], &row["signal"]),
        (&json!("failed"), &json!(1), &Value::Null),
        "{row}"
    );
    assert_eq!(
        row["output_hash"],
        json!(b3_hash(&bytes)),
        "the whole stream"
    );
    let tail = String::from_utf8(bytes[bytes.len() - 4096..].to_vec()).expect("ASCII");
    assert_eq!(row["tail"], json!(tail), "the last 4 096 bytes");
    assert!(
        child.stdout.len() < 64 * 1024,
        "spec's stdout holds the flood: {} bytes",
        child.stdout.len()
    );
    assert!(
        contains_run(&child.stderr, &bytes),
        "the flood copied whole to spec's stderr ({} bytes)",
        child.stderr.len()
    );
    let doc = child.json();
    assert_eq!(doc["tests"].as_array().map(Vec::len), Some(7));
    assert!(
        !child.json_text.contains("abcdefghijklmnopqrstuvwxyzabcdef"),
        "no output in the JSON"
    );
}
