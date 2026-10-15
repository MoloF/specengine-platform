//! docs/features/plugin-gate.md, the CLI half (`spec gate`; canon
//! `docs/canon/gate.md` "Controlled paths", "Verdict", "CLI", "Event"):
//! AC-01..AC-12 and AC-14 and the iteration-2 cases, on scratch git
//! repositories of `fixtures/spec-a` (`common::proposal::Pair`: the main
//! worktree on `main`, a linked one on `t1`, a scratch `HOME`, every git
//! process in the sandbox of `common::git`, automatic maintenance off) and,
//! where the stack must not matter, `fixtures/spec-c` (a web service in
//! Python). The verdict through the library (`gate`, `gate_session`,
//! `read_hook_input`), and through the binary where the process is the
//! subject (exit codes, stdout, stderr, the deadline, the hook's stdin);
//! every spawned `spec` under `perl -e 'alarm N'` so none outlives the test.
//! The owner's task commands run through the library, consent yes.
//!
//! The wrapper `hooks/gate.sh` with stand-in `spec` binaries is
//! `gate_wrapper.rs`; AC-14 here runs it with the real binary.

#![cfg(unix)]

mod common;
mod task_common;

use std::ffi::OsString;
use std::fs;
use std::io::{Cursor, Read as _, Write as _};
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::proposal::{Pair, sqlite3};
use common::serve::Daemon;
use common::{SPEC, data_dir, repository_root};
use serde_json::{Value, json};
use specengine_cli::{
    Exit, GATE_DEADLINE, GATE_TASKS_MAX, GateInput, GateOutcome, GateProgress, GateReason,
    GateRequest, GateStage, GateVerdict, HookInput, LINK_HOPS, gate, gate_session, read_hook_input,
};
use task_common::{CLOCK, Tasks};

/// The JSON document's keys, in order (plugin-gate "Data", CLI).
const JSON_KEYS: [&str; 11] = [
    "verdict", "reason", "mode", "path", "root", "zone", "worktree", "branch", "task", "tasks",
    "cause",
];

const NO_VERDICT: &str = "specengine gate: no verdict (";
const REFUSAL_TEXT: &str = "no in_progress task";
const TITLE: &str = "Zanzibar title 41q";
const GOAL: &str = "Quokka goal 77z";

// ------------------------------------------------------------------ setup

/// `[zones] code = ["src/**"]` and `[gate]` in `mode` listing `code`.
fn tables(mode: &str) -> String {
    format!("\n[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"{mode}\"\nzones = [\"code\"]\n")
}

/// Appends `extra` to the config of `dir`.
fn append_config(dir: &Path, extra: &str) {
    let path = dir.join("specengine.toml");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(extra);
    fs::write(&path, text).unwrap();
}

/// A pair of `fixture` whose two worktrees hold `src/a.rs` and, unless
/// `mode` is `None`, the gate tables, committed in each.
fn pair_with(label: &str, fixture: &str, mode: Option<&str>) -> Pair {
    let pair = Pair::new(label, fixture);
    for dir in [pair.main.clone(), pair.linked.clone()] {
        if let Some(mode) = mode {
            append_config(&dir, &tables(mode));
        }
        common::write(&dir, "src/a.rs", "fn a() {}\n");
        pair.commit_all(&dir, "the gate's scratch files");
    }
    pair
}

fn gated(label: &str, mode: &str) -> Pair {
    pair_with(label, "spec-a", Some(mode))
}

fn request(pair: &Pair, cwd: &Path, input: GateInput, session: Option<&Path>) -> GateRequest {
    GateRequest {
        input,
        session: session.map(Path::to_path_buf),
        git: pair.git_env(cwd),
        now: CLOCK.to_owned(),
        deadline: Instant::now() + GATE_DEADLINE,
        progress: GateProgress::new(),
    }
}

/// The library's verdict on `--worktree dir --path path`.
fn verdict(pair: &Pair, dir: &Path, path: &str) -> GateOutcome {
    let input = GateInput::Path {
        dir: dir.to_path_buf(),
        path: PathBuf::from(path),
    };
    gate(&pair.env(dir), &request(pair, dir, input, Some(dir)))
}

/// The verdict's JSON document, parsed (its keys checked).
fn doc(outcome: &GateOutcome) -> Value {
    let text = outcome.to_json();
    assert_eq!(task_common::keys(&text), JSON_KEYS, "{text}");
    assert!(text.ends_with('\n') && text.lines().count() == 1, "{text}");
    serde_json::from_str(&text).unwrap()
}

fn assert_refused(outcome: &GateOutcome, context: &str) {
    assert_eq!(
        outcome.verdict,
        GateVerdict::Refuse,
        "{context}: {outcome:#?}"
    );
    assert_eq!(outcome.reason, GateReason::NoClaim, "{context}");
    assert_eq!(outcome.exit_code(), 2, "{context}");
}

fn assert_passed(outcome: &GateOutcome, reason: GateReason, context: &str) {
    assert_eq!(
        outcome.verdict,
        GateVerdict::Pass,
        "{context}: {outcome:#?}"
    );
    assert_eq!(outcome.reason, reason, "{context}: {outcome:#?}");
    assert_eq!(outcome.exit_code(), 0, "{context}");
}

/// No verdict: exit 2, the one stderr line naming the cause, never the
/// refusal's text.
fn assert_no_verdict(outcome: &GateOutcome, context: &str) -> String {
    assert_eq!(
        outcome.verdict,
        GateVerdict::Unavailable,
        "{context}: {outcome:#?}"
    );
    assert_eq!(outcome.exit_code(), 2, "{context}");
    let lines = outcome.stderr_lines();
    assert_eq!(lines.len(), 1, "{context}: {lines:?}");
    assert!(lines[0].starts_with(NO_VERDICT), "{context}: {}", lines[0]);
    assert!(!lines[0].contains(REFUSAL_TEXT), "{context}: {}", lines[0]);
    lines[0].clone()
}

/// A task of `pair` moved to `ready` (planned, approved).
fn ready_task(pair: &Pair, cwd: &Path) -> String {
    let id = pair.new_ok(cwd, &["MEC-STAMINA"]);
    pair.plan_ok(cwd, &id, &["Holds."], &[]);
    pair.approve_task(cwd, &id);
    id
}

fn git(pair: &Pair, dir: &Path, args: &[&str]) {
    pair.git.git(dir, args);
}

fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

/// Whether the volume holding `dir` ignores case.
fn case_insensitive(dir: &Path) -> bool {
    let probe = dir.join(".CaseProbe");
    fs::write(&probe, b"").unwrap();
    let insensitive = dir.join(".caseprobe").exists();
    fs::remove_file(&probe).unwrap();
    insensitive
}

// ------------------------------------------------------------- the binary

/// What a spawned `spec` gave.
#[derive(Debug)]
struct Ran {
    code: i32,
    stdout: Vec<u8>,
    stderr: String,
    took: Duration,
    /// The whole stdin was taken (no closed pipe met).
    wrote: bool,
}

impl Ran {
    fn stdout_text(&self) -> String {
        String::from_utf8(self.stdout.clone()).expect("UTF-8 stdout")
    }

    fn json(&self) -> Value {
        let text = self.stdout_text();
        assert_eq!(task_common::keys(&text), JSON_KEYS, "{text}");
        serde_json::from_str(&text).unwrap()
    }
}

/// The hook's stdin.
enum Input {
    Null,
    Bytes(Vec<u8>),
    /// Written, then held open until the process ends.
    HeldOpen(Vec<u8>),
}

/// `sh`-free spawn of `program args` under a 20 s alarm (so nothing
/// outlives the test), with exactly `vars`, in `cwd`.
fn spawn(
    program: &str,
    args: &[&str],
    vars: &[(OsString, OsString)],
    cwd: &Path,
    input: Input,
) -> Ran {
    let mut command = Command::new("/usr/bin/perl");
    command
        .args(["-e", "alarm 20; exec @ARGV", program])
        .args(args)
        .env_clear()
        .envs(vars.iter().map(|(name, value)| (name, value)))
        .current_dir(cwd)
        .stdin(if matches!(input, Input::Null) {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let mut child = command.spawn().expect("spawn");
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let writer = child.stdin.take().map(|mut stdin| {
        let (bytes, hold) = match input {
            Input::Bytes(bytes) => (bytes, false),
            Input::HeldOpen(bytes) => (bytes, true),
            Input::Null => (Vec::new(), false),
        };
        std::thread::spawn(move || {
            let wrote = stdin.write_all(&bytes).and_then(|()| stdin.flush()).is_ok();
            if hold {
                // Held until the process ended (or 25 s).
                let _ = done_rx.recv_timeout(Duration::from_secs(25));
            }
            drop(stdin);
            wrote
        })
    });
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
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
    let status = wait_bounded(&mut child, Duration::from_secs(25));
    let took = started.elapsed();
    let _ = done_tx.send(());
    let wrote = writer.is_none_or(|writer| writer.join().unwrap());
    Ran {
        code: status.unwrap_or_else(|| panic!("{program} {args:?} was killed by a signal")),
        stdout: out.join().unwrap(),
        stderr: String::from_utf8(err.join().unwrap()).expect("UTF-8 stderr"),
        took,
        wrote,
    }
}

/// Waits for `child` at most `limit`, then kills it (it is ours): its exit
/// code, `None` when a signal ended it.
fn wait_bounded(child: &mut Child, limit: Duration) -> Option<i32> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status.code();
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("a child ran past {limit:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The sandbox's variables, `HOME` the pair's, plus `extra`, minus
/// `unset`.
fn vars(pair: &Pair, extra: &[(&str, &Path)], unset: &[&str]) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = pair
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), pair.home.clone().into()));
    for (name, value) in extra {
        vars.retain(|(known, _)| known != name);
        vars.push(((*name).into(), value.as_os_str().to_owned()));
    }
    vars.retain(|(name, _)| !unset.iter().any(|gone| name == gone));
    vars
}

/// `spec gate --worktree dir --path path [--json]` with the pair's
/// variables.
fn spec_path(pair: &Pair, dir: &Path, path: &str, json: bool) -> Ran {
    let dir_text = dir.to_str().unwrap();
    let mut args = vec!["gate", "--worktree", dir_text, "--path", path];
    if json {
        args.push("--json");
    }
    spawn(SPEC, &args, &vars(pair, &[], &[]), dir, Input::Null)
}

/// Claude Code's `PreToolUse` input for `tool` writing `file` from `cwd`.
fn hook_json(tool: &str, cwd: &Path, file: &str, content_bytes: usize) -> Vec<u8> {
    let tool_input = match tool {
        "NotebookEdit" => json!({"notebook_path": file, "new_source": "x = 1", "cell_id": "c1"}),
        "Write" => json!({"file_path": file, "content": "x".repeat(content_bytes)}),
        "MultiEdit" => {
            json!({"file_path": file, "edits": [{"old_string": "a", "new_string": "b"}]})
        }
        _ => json!({"file_path": file, "old_string": "fn a", "new_string": "fn b"}),
    };
    serde_json::to_vec(&json!({
        "session_id": "s-1",
        "transcript_path": "/tmp/t.jsonl",
        "cwd": cwd,
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": tool_input,
    }))
    .unwrap()
}

/// `spec gate --stdin` with `input`, `CLAUDE_PROJECT_DIR` `session`.
fn spec_stdin(pair: &Pair, cwd: &Path, session: Option<&Path>, input: Input) -> Ran {
    let extra: Vec<(&str, &Path)> = session
        .map(|dir| vec![("CLAUDE_PROJECT_DIR", dir)])
        .unwrap_or_default();
    spawn(
        SPEC,
        &["gate", "--stdin"],
        &vars(pair, &extra, &[]),
        cwd,
        input,
    )
}

// ------------------------------------------------------------------ AC-01

/// AC-01: no `[gate]` is off; each bad `[gate]` is no verdict naming
/// `<root>/specengine.toml:<line>` while `spec show` answers. M: absent
/// read as `selective`; a bad `[gate]` fails `spec show`.
#[test]
fn ac01_no_gate_is_off_and_a_bad_gate_is_no_verdict_at_its_line() {
    let pair = pair_with("pg-ac01", "spec-a", None);
    let main = pair.main.clone();
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_eq!(
        (ran.code, ran.stdout_text().as_str()),
        (0, "pass: off\n"),
        "{ran:?}"
    );
    assert_eq!(ran.stderr, "");
    let document = spec_path(&pair, &main, "src/a.rs", true).json();
    assert_eq!(document["verdict"], "pass");
    assert_eq!(document["reason"], "off");
    assert_eq!(document["mode"], "off");
    assert_eq!(document["path"], "src/a.rs");
    assert_eq!(document["root"], main.to_str().unwrap());
    // No config at all.
    let loose = pair.scratch.dir("loose");
    let ran = spec_path(&pair, &loose, "x.rs", false);
    assert_eq!(
        (ran.code, ran.stdout_text().as_str()),
        (0, "pass: no-config\n")
    );

    let original = fs::read_to_string(main.join("specengine.toml")).unwrap();
    let selective = "mode = \"selective\"\nzones = [\"code\"]\n";
    let code = "code = [\"src/**\"]\n";
    for (label, zones, gate_table, needle) in [
        (
            "an unknown key",
            code,
            &*format!("{selective}colour = \"red\"\n"),
            "colour",
        ),
        (
            "mode strict",
            code,
            "mode = \"strict\"\nzones = [\"code\"]\n",
            "\"strict\"",
        ),
        (
            "mode on",
            code,
            "mode = \"on\"\nzones = [\"code\"]\n",
            "mode = \"on\"",
        ),
        (
            "min_files",
            code,
            &*format!("{selective}min_files = 3\n"),
            "min_files",
        ),
        (
            "an undeclared zone",
            code,
            "mode = \"selective\"\nzones = [\"code\", \"docs\"]\n",
            "\"docs\"]",
        ),
        ("a glob src/[", "code = [\"src/[\"]\n", selective, "src/["),
        (
            "a zone Code",
            "Code = [\"src/**\"]\n",
            "mode = \"selective\"\nzones = [\"Code\"]\n",
            "Code =",
        ),
    ] {
        let text = format!("{original}\n[zones]\n{zones}\n[gate]\n{gate_table}");
        fs::write(main.join("specengine.toml"), &text).unwrap();
        let line = text
            .lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("{label}: {needle}"))
            + 1;
        let at = format!("{}/specengine.toml:{line}", main.display());
        let ran = spec_path(&pair, &main, "src/a.rs", false);
        assert_eq!(ran.code, 2, "{label}: {ran:?}");
        assert!(ran.stdout.is_empty(), "{label}");
        assert_eq!(ran.stderr.lines().count(), 1, "{label}: {}", ran.stderr);
        assert!(
            ran.stderr.starts_with(NO_VERDICT),
            "{label}: {}",
            ran.stderr
        );
        assert!(
            ran.stderr.contains(&at),
            "{label}: {at} not in {}",
            ran.stderr
        );
        assert!(
            ran.stderr.contains(&format!("Fix {at}.")),
            "{label}: {}",
            ran.stderr
        );
        assert!(!ran.stderr.contains(REFUSAL_TEXT), "{label}");
        assert!(ran.took < Duration::from_secs(4), "{label}: {:?}", ran.took);
        let document = spec_path(&pair, &main, "src/a.rs", true).json();
        assert_eq!(
            (document["verdict"].as_str(), document["reason"].as_str()),
            (Some("unavailable"), Some("config")),
            "{label}"
        );
        // The reads keep answering.
        let shown = spawn(
            SPEC,
            &["show", "MEC-STAMINA"],
            &vars(&pair, &[], &[]),
            &main,
            Input::Null,
        );
        assert_eq!(shown.code, 0, "{label}: spec show: {}", shown.stderr);
        assert!(shown.stdout_text().contains("Stamina"), "{label}");
    }
    fs::write(main.join("specengine.toml"), original).unwrap();
}

// ------------------------------------------------------------------ AC-02

/// AC-02: under `selective` only an `in_progress` task claimed here opens a
/// controlled path, a reported run keeps it open. M: `ready` opens; an
/// open run required.
#[test]
fn ac02_only_an_in_progress_claimed_task_opens() {
    let pair = gated("pg-ac02", "selective");
    let main = pair.main.clone();
    let check = |context: &str| verdict(&pair, &main, "src/a.rs").tap_refused(context);
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    check("draft");
    pair.plan_ok(&main, &id, &["Holds."], &[]);
    check("review");
    let (changes, _) = pair.owner(&main, "changes", &id, Some("More."), true, CLOCK);
    changes.expect("changes requested");
    check("changes_requested");
    pair.plan_ok(&main, &id, &["Holds."], &[]);
    pair.approve_task(&main, &id);
    check("ready");
    pair.claim_ok(&main, &id, &main);
    let claimed = verdict(&pair, &main, "src/a.rs");
    assert_passed(&claimed, GateReason::Claimed, "claimed, run open");
    assert_eq!(claimed.task.as_deref(), Some(id.as_str()));
    assert!(claimed.tasks.is_empty(), "tasks only on a refusal");
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_eq!(
        (ran.code, ran.stdout_text().as_str()),
        (0, "pass: claimed\n")
    );
    pair.report_ok(&main, &id);
    assert_passed(
        &verdict(&pair, &main, "src/a.rs"),
        GateReason::Claimed,
        "after report",
    );
    pair.complete(&main, &id).expect("complete");
    check("done");
    let second = pair.new_ok(&main, &["MEC-STAMINA"]);
    let (cancelled, _) = pair.owner(&main, "cancel", &second, None, true, CLOCK);
    cancelled.expect("cancelled");
    check("cancelled");
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_eq!(ran.code, 2, "{ran:?}");
    assert!(ran.stdout.is_empty());
    assert!(
        ran.stderr.starts_with("specengine gate (selective): "),
        "{}",
        ran.stderr
    );
}

trait Tap {
    fn tap_refused(self, context: &str) -> GateOutcome;
}

impl Tap for GateOutcome {
    fn tap_refused(self, context: &str) -> GateOutcome {
        assert_refused(&self, context);
        self
    }
}

// ------------------------------------------------------------------ AC-03

/// AC-03: a claim opens only its worktree on its branch of its repository.
/// M: the repository alone compared; the branch ignored; matched by slug.
#[test]
fn ac03_a_claim_opens_only_its_worktree_on_its_branch() {
    let pair = gated("pg-ac03", "selective");
    let (main, linked) = (pair.main.clone(), pair.linked.clone());
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    assert_passed(
        &verdict(&pair, &main, "src/a.rs"),
        GateReason::Claimed,
        "W1",
    );

    // W2 of the same repository.
    let w2 = verdict(&pair, &linked, "src/a.rs").tap_refused("W2");
    let document = doc(&w2);
    assert_eq!(document["worktree"], linked.to_str().unwrap());
    assert_eq!(document["branch"], "t1");
    assert_eq!(
        document["tasks"],
        json!([{"id": id, "status": "in_progress", "worktree": main.to_str().unwrap(), "branch": "main"}])
    );

    // W2 on the claim's own branch (git allows it when told): the
    // worktree is compared, not only the repository and the branch.
    git(
        &pair,
        &linked,
        &["switch", "-q", "--ignore-other-worktrees", "main"],
    );
    let same_branch = verdict(&pair, &linked, "src/a.rs").tap_refused("W2 on main");
    assert_eq!(same_branch.branch.as_deref(), Some("main"));
    assert_eq!(same_branch.worktree.as_deref(), linked.to_str());
    git(&pair, &linked, &["switch", "-q", "t1"]);

    // W1 on another branch, then back.
    git(&pair, &main, &["switch", "-q", "-c", "other"]);
    let other = verdict(&pair, &main, "src/a.rs").tap_refused("W1 on other");
    assert_eq!(other.branch.as_deref(), Some("other"));
    git(&pair, &main, &["switch", "-q", "main"]);
    assert_passed(
        &verdict(&pair, &main, "src/a.rs"),
        GateReason::Claimed,
        "back on main",
    );

    // Detached: nothing opens, branch null.
    git(&pair, &main, &["checkout", "-q", "--detach"]);
    let detached = verdict(&pair, &main, "src/a.rs").tap_refused("detached");
    assert_eq!(doc(&detached)["branch"], Value::Null);
    let message = &detached.stderr_lines()[0];
    assert!(message.contains("detached HEAD"), "{message}");
    git(&pair, &main, &["checkout", "-q", "main"]);

    // A second clone: the same slug and branch, another repository.
    let clone = pair.scratch.join("clone");
    git(
        &pair,
        pair.scratch.path(),
        &[
            "clone",
            "-q",
            main.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let clone = fs::canonicalize(clone).unwrap();
    pair.git.quiet(&clone);
    let cloned = verdict(&pair, &clone, "src/a.rs").tap_refused("a second clone");
    assert_eq!(cloned.branch.as_deref(), Some("main"));
    assert!(
        cloned.tasks.is_empty(),
        "another repository's tasks: {:?}",
        cloned.tasks
    );
    assert!(
        cloned.stderr_lines()[0].contains("Tasks here: none ready or in_progress."),
        "{:?}",
        cloned.stderr_lines()
    );
}

// ------------------------------------------------------------------ AC-04

/// AC-04: outside every zone, under no config, or in a project without
/// `[gate]`: exit 0 with the data directory unreadable. M: the queue
/// opened before the zone match.
#[test]
fn ac04_an_uncontrolled_write_never_opens_the_queue() {
    let pair = gated("pg-ac04", "selective");
    let plain = pair_with("pg-ac04-plain", "spec-a", None);
    let data = data_dir(&pair.home);
    fs::create_dir_all(&data).unwrap();
    let plain_data = data_dir(&plain.home);
    fs::create_dir_all(&plain_data).unwrap();
    chmod(&data, 0o000);
    chmod(&plain_data, 0o000);
    let main = pair.main.clone();
    let loose = pair.scratch.dir("loose");
    let results = [
        (
            verdict(&pair, &main, "docs/notes.md"),
            GateReason::NotControlled,
            "outside every zone",
        ),
        (
            verdict(&pair, &loose, "x.rs"),
            GateReason::NoConfig,
            "under no config",
        ),
        (
            verdict(&plain, &plain.main, "src/a.rs"),
            GateReason::Off,
            "no [gate]",
        ),
    ];
    // The control: a controlled path does reach the queue.
    let controlled = verdict(&pair, &main, "src/a.rs");
    chmod(&data, 0o755);
    chmod(&plain_data, 0o755);
    for (outcome, reason, context) in results {
        assert_passed(&outcome, reason, context);
        assert!(outcome.stderr_lines().is_empty(), "{context}");
    }
    let cause = assert_no_verdict(&controlled, "a controlled path, the queue unreadable");
    assert!(cause.contains("queue"), "{cause}");
}

// ------------------------------------------------------------------ AC-05

/// AC-05: the path is joined, folded and canonicalised before the zone
/// match. M: the raw `file_path` matched.
#[test]
fn ac05_the_path_is_normalised_before_the_zone_match() {
    let pair = gated("pg-ac05", "selective");
    let main = pair.main.clone();
    symlink("src", main.join("link")).unwrap();
    let absolute = main.join("src/a.rs");
    let mut cases = vec![
        ("src/../src/a.rs".to_owned(), "src/a.rs"),
        (absolute.to_str().unwrap().to_owned(), "src/a.rs"),
        ("link/a.rs".to_owned(), "src/a.rs"),
        ("src/new/dir/b.rs".to_owned(), "src/new/dir/b.rs"),
        ("./src/./a.rs".to_owned(), "src/a.rs"),
        ("docs/../src/x/../y.rs".to_owned(), "src/y.rs"),
    ];
    if case_insensitive(&main) {
        cases.push(("SRC/a.rs".to_owned(), "src/a.rs"));
        cases.push(("SRC/New/c.rs".to_owned(), "src/New/c.rs"));
    } else {
        eprintln!("note: the scratch volume is case-sensitive: SRC/a.rs skipped");
    }
    for (path, folded) in &cases {
        let outcome = verdict(&pair, &main, path).tap_refused(path);
        let document = doc(&outcome);
        assert_eq!(document["path"], *folded, "{path}");
        assert_eq!(document["zone"], "code", "{path}");
        assert_eq!(document["mode"], "selective", "{path}");
        // The same through the hook's JSON (relative to its `cwd`).
        let input = read_hook_input(
            Cursor::new(hook_json("Edit", &main, path, 0)),
            Instant::now() + GATE_DEADLINE,
        );
        let hooked = gate(&pair.env(&main), &request(&pair, &main, input, Some(&main)));
        assert_refused(&hooked, &format!("hook {path}"));
        assert_eq!(hooked.path.as_deref(), Some(*folded), "hook {path}");
    }
    // The config file itself, whatever the zones.
    let config = verdict(&pair, &main, "specengine.toml").tap_refused("the config");
    assert_eq!(doc(&config)["zone"], Value::Null);
    let message = &config.stderr_lines()[0];
    assert!(
        message.contains("specengine.toml would be a project config"),
        "{message}"
    );
}

// ------------------------------------------------------------------ AC-06

/// The queue held by another `sqlite3` with `locking_mode=EXCLUSIVE` and a
/// write (readers wait too), released and reaped on drop.
struct Holder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Holder {
    fn hold(db: &Path) -> Self {
        let mut child = Command::new(sqlite3())
            .arg(db)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sqlite3 runs");
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(b".timeout 5000\nPRAGMA locking_mode=EXCLUSIVE;\nBEGIN EXCLUSIVE;\nINSERT INTO events (project, type, payload, at) VALUES ('x', 'x', '{}', 'x');\n")
            .unwrap();
        stdin.flush().unwrap();
        let holder = Self {
            child,
            stdin: Some(stdin),
        };
        let started = Instant::now();
        loop {
            let probe = Command::new(sqlite3())
                .args(["-readonly", "-cmd", ".timeout 0"])
                .arg(db)
                .arg("SELECT count(*) FROM tasks;")
                .stdin(Stdio::null())
                .output()
                .unwrap();
            if !probe.status.success() {
                return holder;
            }
            assert!(started.elapsed() < Duration::from_secs(5), "never locked");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"ROLLBACK;\n.quit\n");
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Asserts the binary's no verdict: exit 2 within 4 s, nothing on stdout,
/// one stderr line naming the cause (`needle`), never the refusal's text.
fn assert_ran_no_verdict(ran: &Ran, needle: &str, context: &str) {
    assert_eq!(ran.code, 2, "{context}: {ran:?}");
    assert!(ran.stdout.is_empty(), "{context}");
    assert_eq!(ran.stderr.lines().count(), 1, "{context}: {}", ran.stderr);
    assert!(
        ran.stderr.starts_with(NO_VERDICT),
        "{context}: {}",
        ran.stderr
    );
    assert!(
        ran.stderr.contains(needle),
        "{context}: {needle:?} not in {}",
        ran.stderr
    );
    assert!(!ran.stderr.contains(REFUSAL_TEXT), "{context}");
    assert!(
        ran.took < Duration::from_secs(4),
        "{context}: {:?}",
        ran.took
    );
}

/// AC-06: each cause of no verdict is exit 2 in under 4 s naming it. M:
/// one exits 0; an unbounded busy wait.
#[test]
fn ac06_each_no_verdict_is_exit_2_naming_its_cause() {
    let pair = gated("pg-ac06", "selective");
    let main = pair.main.clone();
    let id = ready_task(&pair, &main);
    let main_text = main.to_str().unwrap();

    // HOME unset.
    let ran = spawn(
        SPEC,
        &["gate", "--worktree", main_text, "--path", "src/a.rs"],
        &vars(&pair, &[], &["HOME"]),
        &main,
        Input::Null,
    );
    assert_ran_no_verdict(&ran, "HOME", "HOME unset");
    // HOME relative.
    let ran = spawn(
        SPEC,
        &["gate", "--worktree", main_text, "--path", "src/a.rs"],
        &vars(&pair, &[("HOME", Path::new("rel/home"))], &[]),
        &main,
        Input::Null,
    );
    assert_ran_no_verdict(&ran, "HOME", "HOME relative");

    // A corrupt claim.
    pair.sql(&format!("UPDATE tasks SET claim = '{{' WHERE id = '{id}';"));
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_ran_no_verdict(
        &ran,
        &format!("Run `spec task show {id}` on a terminal."),
        "a corrupt claim",
    );
    assert!(ran.stderr.contains("`claim`"), "{}", ran.stderr);
    pair.sql(&format!("UPDATE tasks SET claim = NULL WHERE id = '{id}';"));

    // The queue under an exclusive lock: the deadline, not a hang.
    {
        let _held = Holder::hold(&pair.db());
        let ran = spec_path(&pair, &main, "src/a.rs", false);
        assert_ran_no_verdict(&ran, "locked past the deadline", "an exclusive lock");
        assert!(
            ran.took >= Duration::from_millis(2500),
            "gave up early: {:?}",
            ran.took
        );
        let document = spec_path(&pair, &main, "src/a.rs", true).json();
        assert_eq!(document["reason"], "deadline");
    }
    // Released: a verdict again.
    assert_refused(&verdict(&pair, &main, "src/a.rs"), "released");

    // A queue of a newer schema (this build's is 8 since
    // docs/features/drift-queue.md "Queue schema 8").
    pair.sql("PRAGMA user_version = 9;");
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_ran_no_verdict(
        &ran,
        "Reinstall spec and the plugin from one commit.",
        "schema 9",
    );
    pair.sql("PRAGMA user_version = 8;");

    // The root outside git.
    let loose = pair.scratch.copy("spec-a", "no-git");
    append_config(&loose, &tables("selective"));
    common::write(&loose, "src/a.rs", "fn a() {}\n");
    let ran = spec_path(&pair, &loose, "src/a.rs", false);
    assert_ran_no_verdict(&ran, "git", "outside git");
    assert_eq!(
        spec_path(&pair, &loose, "src/a.rs", true).json()["reason"],
        "git"
    );

    // Bad stdin, the session gated.
    let ran = spec_stdin(
        &pair,
        &main,
        Some(&main),
        Input::Bytes(b"not json".to_vec()),
    );
    assert_ran_no_verdict(&ran, "not the expected JSON", "bad stdin");

    // A bad [gate] (AC-01 has every form).
    append_config(&main, "min_files = 3\n");
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_ran_no_verdict(&ran, "min_files", "a bad [gate]");
}

// ------------------------------------------------------------------ AC-08

/// AC-08: `--stdin` prints nothing on stdout on a pass, a refusal and a no
/// verdict; no output of `spec gate` names `permissionDecision`. M: the
/// JSON printed in hook mode; `allow` on a pass.
#[test]
fn ac08_the_hook_mode_prints_nothing_on_stdout() {
    let pair = gated("pg-ac08", "selective");
    let (main, linked) = (pair.main.clone(), pair.linked.clone());
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    let loose = pair.scratch.dir("loose");
    let mut outputs = Vec::new();
    for (cwd, file, code, context) in [
        (&main, "src/a.rs", 0, "claimed"),
        (&main, "docs/notes.md", 0, "not controlled"),
        (&loose, "x.rs", 0, "no config"),
        (&linked, "src/a.rs", 2, "refused"),
    ] {
        let ran = spec_stdin(
            &pair,
            cwd,
            Some(cwd),
            Input::Bytes(hook_json("Edit", cwd, file, 0)),
        );
        assert_eq!(ran.code, code, "{context}: {ran:?}");
        assert!(ran.stdout.is_empty(), "{context}: {:?}", ran.stdout_text());
        if code == 0 {
            assert_eq!(ran.stderr, "", "{context}");
        }
        outputs.push(ran.stderr);
    }
    let ran = spec_stdin(&pair, &main, Some(&main), Input::Bytes(b"{".to_vec()));
    assert_eq!(ran.code, 2);
    assert!(ran.stdout.is_empty());
    outputs.push(ran.stderr);
    for args in [
        vec!["gate", "--help"],
        vec!["--help"],
        vec![
            "gate",
            "--worktree",
            main.to_str().unwrap(),
            "--path",
            "src/a.rs",
            "--json",
        ],
        vec![
            "gate",
            "--worktree",
            linked.to_str().unwrap(),
            "--path",
            "src/a.rs",
            "--json",
        ],
        vec![
            "gate",
            "--worktree",
            main.to_str().unwrap(),
            "--path",
            "src/a.rs",
        ],
    ] {
        let ran = spawn(SPEC, &args, &vars(&pair, &[], &[]), &main, Input::Null);
        outputs.push(ran.stdout_text());
        outputs.push(ran.stderr);
    }
    for output in &outputs {
        assert!(!output.contains("permissionDecision"), "{output}");
        assert!(!output.contains("\"allow\""), "{output}");
        assert!(!output.contains("hookSpecificOutput"), "{output}");
    }
    // Usage: one no-verdict line, exit 2, never 1.
    for (args, needle) in [
        (
            vec!["--root", ".", "gate", "--stdin"],
            "--root and --config",
        ),
        (
            vec!["--json", "gate", "--stdin"],
            "--json is not taken with --stdin",
        ),
        (
            vec!["gate", "--stdin", "--json"],
            "--json is not taken with --stdin",
        ),
        (vec!["gate"], "usage"),
        (vec!["gate", "--path", "x"], "usage"),
        (
            vec!["gate", "--frobnicate"],
            "Reinstall spec and the plugin from one commit.",
        ),
    ] {
        let ran = spawn(SPEC, &args, &vars(&pair, &[], &[]), &main, Input::Null);
        assert_eq!(ran.code, 2, "{args:?}: {ran:?}");
        assert!(ran.stdout.is_empty(), "{args:?}");
        assert_eq!(ran.stderr.lines().count(), 1, "{args:?}: {}", ran.stderr);
        assert!(
            ran.stderr.starts_with(NO_VERDICT),
            "{args:?}: {}",
            ran.stderr
        );
        assert!(ran.stderr.contains(needle), "{args:?}: {}", ran.stderr);
    }
}

// ------------------------------------------------------------------ AC-09

/// AC-09: a claimed task passes with an open question, a discrepancy, a
/// staged decision, `stale` true and a red `spec check`. M: `stale` read.
#[test]
fn ac09_a_claimed_task_passes_whatever_else_is_open() {
    let pair = gated("pg-ac09", "selective");
    let main = pair.main.clone();
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    let question = task_common::ask_ok(
        &pair,
        &main,
        &["EDGE-STAM-ZERO"],
        "Zero?",
        "Yes.",
        Some(&id),
    );
    let gap = task_common::report_gap(
        &pair,
        &main,
        common::decision::discrepancy(
            &["RULE-STAM-REGEN"],
            "The delay is 2 s in the code.",
            &common::decision::THREE,
        ),
        Some(&id),
    );
    assert!(gap.document.created, "{gap:?}");
    let updated_at = pair.proposal(&question).updated_at;
    let staged = specengine_cli::stage(
        &pair.env(&main),
        &specengine_cli::Globals::default(),
        &specengine_cli::StageRequest {
            id: question.clone(),
            body: specengine_cli::StageBody::Approve {
                option: None,
                answer: Some("Yes, at zero.".to_owned()),
                canon: None,
                editorial: None,
                note: None,
                updated_at,
            },
            now: "2026-10-07T09:30:00Z".to_owned(),
            git: pair.git_env(&main),
        },
    )
    .expect("stage");
    assert_eq!(staged.cause, None, "{staged:?}");
    // Stale: the target's text changed since approval.
    common::replace(
        &main,
        "docs/spec/movement/stamina.md",
        "Stamina limits sprinting.",
        "Stamina caps sprinting.",
    );
    assert_eq!(pair.package(&main, &id)["stale"], json!(true));
    // A red check: a second node with the same ID.
    common::write(
        &main,
        "docs/spec/movement/again.md",
        "---\nid: MEC-STAMINA\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Again\nparent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n# Again\n\nTwice.\n",
    );
    let check = spawn(SPEC, &["check"], &vars(&pair, &[], &[]), &main, Input::Null);
    assert_ne!(check.code, 0, "spec check is red: {check:?}");
    let outcome = verdict(&pair, &main, "src/a.rs");
    assert_passed(&outcome, GateReason::Claimed, "everything else open");
    assert_eq!(outcome.task.as_deref(), Some(id.as_str()));
}

// ------------------------------------------------------------------ AC-10

/// `type|payload` of every `gate.observed`, the payload as stored.
fn observed_events(pair: &Pair) -> Vec<String> {
    if !pair.db().exists() {
        return Vec::new();
    }
    pair.sql(
        "SELECT project || '|' || payload FROM events WHERE type = 'gate.observed' ORDER BY seq;",
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// AC-10: `observe` passes an unclaimed controlled write and records one
/// `gate.observed` with the four keys; claimed: none; `observe` and `off`
/// pass with the data directory unreadable. M: `observe` exits 2; no event.
#[test]
fn ac10_observe_passes_and_records_an_uncovered_write() {
    let pair = gated("pg-ac10", "observe");
    let main = pair.main.clone();
    let top = main.to_str().unwrap();
    fs::create_dir_all(data_dir(&pair.home)).unwrap();
    drop(pair.queue());
    let ran = spec_path(&pair, &main, "src/a.rs", false);
    assert_eq!(
        (ran.code, ran.stdout_text().as_str(), ran.stderr.as_str()),
        (0, "pass: observed\n", "")
    );
    assert_eq!(
        observed_events(&pair),
        [format!(
            "lantern-keep|{{\"path\":\"src/a.rs\",\"zone\":\"code\",\"worktree\":\"{top}\",\"branch\":\"main\"}}"
        )]
    );
    let config = verdict(&pair, &main, "specengine.toml");
    assert_passed(&config, GateReason::Observed, "the config");
    git(&pair, &main, &["checkout", "-q", "--detach"]);
    assert_passed(
        &verdict(&pair, &main, "src/b.rs"),
        GateReason::Observed,
        "detached",
    );
    git(&pair, &main, &["checkout", "-q", "main"]);
    let events = observed_events(&pair);
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(
        events[1],
        format!(
            "lantern-keep|{{\"path\":\"specengine.toml\",\"zone\":null,\"worktree\":\"{top}\",\"branch\":\"main\"}}"
        )
    );
    assert_eq!(
        events[2],
        format!(
            "lantern-keep|{{\"path\":\"src/b.rs\",\"zone\":\"code\",\"worktree\":\"{top}\",\"branch\":null}}"
        )
    );
    // Claimed: a pass, nothing recorded.
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    assert_passed(
        &verdict(&pair, &main, "src/a.rs"),
        GateReason::Claimed,
        "claimed",
    );
    assert_eq!(observed_events(&pair).len(), 3);
    // Not controlled: nothing recorded either.
    assert_passed(
        &verdict(&pair, &main, "docs/n.md"),
        GateReason::NotControlled,
        "docs",
    );
    assert_eq!(observed_events(&pair).len(), 3);

    // The data directory unreadable: observe passes with one note.
    let data = data_dir(&pair.home);
    chmod(&data, 0o000);
    let linked = pair.linked.clone();
    let observe = spec_path(&pair, &linked, "src/a.rs", false);
    chmod(&data, 0o755);
    assert_eq!(observe.code, 0, "{observe:?}");
    assert_eq!(observe.stdout_text(), "pass: observed\n");
    assert_eq!(observe.stderr.lines().count(), 1, "{}", observe.stderr);
    assert!(
        observe
            .stderr
            .starts_with("note: specengine gate (observe): "),
        "{}",
        observe.stderr
    );

    // No database at all (a fresh HOME): a pass, one note, nothing made.
    let fresh = gated("pg-ac10-fresh", "observe");
    let ran = spec_path(&fresh, &fresh.main, "src/a.rs", false);
    assert_eq!(ran.code, 0, "{ran:?}");
    assert!(
        ran.stderr.starts_with(
            "note: specengine gate (observe): src/a.rs in the zone `code` is not covered"
        ),
        "{}",
        ran.stderr
    );
    assert!(!fresh.db().exists(), "no database created");

    // Off with the data directory unreadable.
    let off = gated("pg-ac10-off", "off");
    let off_data = data_dir(&off.home);
    fs::create_dir_all(&off_data).unwrap();
    chmod(&off_data, 0o000);
    let ran = spec_path(&off, &off.main, "src/a.rs", false);
    chmod(&off_data, 0o755);
    assert_eq!(
        (ran.code, ran.stdout_text().as_str(), ran.stderr.as_str()),
        (0, "pass: off\n", "")
    );
}

// ------------------------------------------------------------------ AC-11

fn md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            md_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

/// AC-11: verdicts identical with every `.md` unreadable; the index
/// database untouched (absent stays absent, else its bytes and mtime
/// kept). M: the reads' `update` called. The pilot's median is the owner's
/// at shipping; this fixture's medians are printed.
#[test]
fn ac11_no_spec_file_is_read_and_the_index_is_untouched() {
    // A fresh slug: no database, and none after the verdicts.
    let fresh = gated("pg-ac11-fresh", "selective");
    for _ in 0..3 {
        assert_refused(&verdict(&fresh, &fresh.main, "src/a.rs"), "fresh");
        let ran = spec_path(&fresh, &fresh.main, "src/a.rs", false);
        assert_eq!(ran.code, 2);
    }
    assert!(!fresh.db().exists(), "absent stays absent");

    let pair = gated("pg-ac11", "selective");
    let (main, linked) = (pair.main.clone(), pair.linked.clone());
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    // Settled: the WAL folded in, the queue closed.
    pair.sql("PRAGMA wal_checkpoint(TRUNCATE);");
    let cases: [(&Path, &str); 5] = [
        (&main, "src/a.rs"),
        (&linked, "src/a.rs"),
        (&main, "docs/notes.md"),
        (&linked, "specengine.toml"),
        (&main, "src/new/b.rs"),
    ];
    let before: Vec<String> = cases
        .iter()
        .map(|(dir, path)| verdict(&pair, dir, path).to_json())
        .collect();
    let db_bytes = fs::read(pair.db()).unwrap();
    let db_mtime = fs::metadata(pair.db()).unwrap().modified().unwrap();
    let mut mds = Vec::new();
    md_files(&main, &mut mds);
    md_files(&linked, &mut mds);
    assert!(mds.len() > 10, "{mds:?}");
    for md in &mds {
        chmod(md, 0o000);
    }
    let after: Vec<String> = cases
        .iter()
        .map(|(dir, path)| verdict(&pair, dir, path).to_json())
        .collect();
    let mut binary = Vec::new();
    for _ in 0..2 {
        for (dir, path) in &cases {
            binary.push(spec_path(&pair, dir, path, true).stdout_text());
        }
    }
    for md in &mds {
        chmod(md, 0o644);
    }
    assert_eq!(after, before, "verdicts with every .md unreadable");
    assert_eq!(binary[..5], before[..], "the binary's documents");
    assert_eq!(
        fs::read(pair.db()).unwrap(),
        db_bytes,
        "the database's bytes kept"
    );
    assert_eq!(
        fs::metadata(pair.db()).unwrap().modified().unwrap(),
        db_mtime,
        "its mtime kept"
    );

    // Medians of 50 calls per kind (debug build, this machine).
    let loose = pair.scratch.dir("loose");
    let mut report = Vec::new();
    for (label, dir, path) in [
        ("claimed", &main, "src/a.rs"),
        ("refused", &linked, "src/a.rs"),
        ("not-controlled", &main, "docs/notes.md"),
        ("no-config", &loose, "x.rs"),
    ] {
        let samples: Vec<Duration> = (0..50)
            .map(|_| spec_path(&pair, dir, path, false).took)
            .collect();
        let median = median(samples);
        assert!(median < Duration::from_millis(1500), "{label}: {median:?}");
        report.push(format!("{label} {:.1} ms", median.as_secs_f64() * 1000.0));
    }
    eprintln!(
        "ac11 medians over 50 binary calls (debug): {}",
        report.join(", ")
    );
    assert_eq!(
        fs::read(pair.db()).unwrap(),
        db_bytes,
        "200 more calls: bytes kept"
    );
}

// ------------------------------------------------------------------ AC-12

/// AC-12: a refusal's stderr and JSON name the root-relative path, zone,
/// mode, `claim_task`, `spec task approve`, the task IDs with status and
/// claim place; never a title or goal. M: a title echoed.
#[test]
fn ac12_a_refusal_names_the_next_step_and_never_a_title() {
    let pair = gated("pg-ac12", "selective");
    let (main, linked) = (pair.main.clone(), pair.linked.clone());
    let first = pair
        .new_task(&main, &["MEC-STAMINA"], Some(TITLE), Some(GOAL))
        .unwrap()
        .id
        .unwrap();
    pair.plan_ok(&main, &first, &["Holds."], &[]);
    pair.approve_task(&main, &first);
    let second = ready_task(&pair, &main);
    pair.claim_ok(&linked, &second, &linked);
    let done = ready_task(&pair, &main);
    pair.claim_ok(&main, &done, &main);
    pair.report_ok(&main, &done);
    pair.complete(&main, &done).unwrap();
    let outcome = verdict(&pair, &main, "src/a.rs").tap_refused("main");
    let message = outcome.stderr_lines().join("\n");
    let top = main.display();
    let want = format!(
        "specengine gate (selective): src/a.rs is in the zone `code` and no in_progress task is claimed in {top} on `main`. Tasks here: {first} ready; {second} in_progress, claimed in {} on `t1`. Claim a ready task in this worktree with claim_task, or ask the owner to approve one (`spec task approve T` on a terminal). Nothing was written.",
        linked.display()
    );
    assert_eq!(message, want);
    let ran = spec_path(&pair, &main, "src/a.rs", true);
    assert_eq!(ran.code, 2);
    let text = ran.stdout_text();
    assert_eq!(ran.stderr.trim_end(), want);
    let document = ran.json();
    assert_eq!(document["verdict"], "refuse");
    assert_eq!(document["reason"], "no-claim");
    assert_eq!(document["mode"], "selective");
    assert_eq!(document["path"], "src/a.rs");
    assert_eq!(document["zone"], "code");
    assert_eq!(document["root"], main.to_str().unwrap());
    assert_eq!(document["task"], Value::Null);
    assert_eq!(document["cause"], Value::Null);
    assert_eq!(
        document["tasks"],
        json!([
            {"id": first, "status": "ready", "worktree": null, "branch": null},
            {"id": second, "status": "in_progress", "worktree": linked.to_str().unwrap(), "branch": "t1"},
        ])
    );
    for output in [&message, &text, &ran.stderr] {
        assert!(
            !output.contains(TITLE) && !output.contains("Zanzibar"),
            "{output}"
        );
        assert!(
            !output.contains(GOAL) && !output.contains("Quokka"),
            "{output}"
        );
        assert!(!output.contains("Tune the regeneration"), "{output}");
    }

    // Nine ready: eight listed, then the rest counted.
    let many = gated("pg-ac12-many", "selective");
    let ids: Vec<String> = (0..9).map(|_| ready_task(&many, &many.main)).collect();
    let outcome = verdict(&many, &many.main, "src/a.rs").tap_refused("nine");
    assert_eq!(outcome.tasks.len(), GATE_TASKS_MAX);
    assert_eq!(outcome.tasks.len(), 8);
    let listed: Vec<&str> = outcome.tasks.iter().map(|task| task.id.as_str()).collect();
    assert_eq!(
        listed,
        ids[..8].iter().map(String::as_str).collect::<Vec<_>>()
    );
    let message = &outcome.stderr_lines()[0];
    assert!(
        message.contains(&format!("{} ready; and 1 more. Claim", ids[7])),
        "{message}"
    );
    assert!(!message.contains(&ids[8]), "{message}");
}

// ------------------------------------------------------------------ AC-14

/// The wrapper as the hook runs it: `sh hooks/gate.sh` with `spec` (the
/// built binary) on a scratch `PATH`, the hook JSON on stdin.
fn wrapper(pair: &Pair, cwd: &Path, input: Vec<u8>) -> Ran {
    let bin = pair.scratch.join("bin");
    if !bin.exists() {
        fs::create_dir_all(&bin).unwrap();
        symlink(SPEC, bin.join("spec")).unwrap();
    }
    let tmp = pair.scratch.dir("wrapper-tmp");
    let path = pair.git.path_with(&bin);
    let mut env = vars(pair, &[("CLAUDE_PROJECT_DIR", cwd), ("TMPDIR", &tmp)], &[]);
    env.retain(|(name, _)| name != "PATH");
    env.push(("PATH".into(), path));
    let script = repository_root().join("plugin/specengine/hooks/gate.sh");
    let ran = spawn(
        "/bin/sh",
        &[script.to_str().unwrap()],
        &env,
        cwd,
        Input::Bytes(input),
    );
    let left: Vec<_> = fs::read_dir(&tmp).unwrap().collect();
    assert!(left.is_empty(), "the wrapper's temp file removed: {left:?}");
    ran
}

/// AC-14: through the wrapper on spec-a under `selective`: closed in
/// `draft`, `ready`; open after `claim` and `report`; closed after
/// `complete`. M: open after `complete`.
#[test]
fn ac14_the_hook_through_the_wrapper_follows_the_task() {
    let pair = gated("pg-ac14", "selective");
    let main = pair.main.clone();
    let file = main.join("src/a.rs");
    let file = file.to_str().unwrap();
    let through = |tool: &str| wrapper(&pair, &main, hook_json(tool, &main, file, 0));
    let closed = |context: &str| {
        for tool in ["Edit", "Write", "MultiEdit", "NotebookEdit"] {
            let ran = through(tool);
            assert_eq!(ran.code, 2, "{context} {tool}: {ran:?}");
            assert!(ran.stdout.is_empty());
            assert!(
                ran.stderr
                    .starts_with("specengine gate (selective): src/a.rs is in the zone `code`"),
                "{context}: {}",
                ran.stderr
            );
        }
    };
    let open = |context: &str| {
        for tool in ["Edit", "Write", "MultiEdit", "NotebookEdit"] {
            let ran = through(tool);
            assert_eq!(
                (ran.code, ran.stdout.len(), ran.stderr.as_str()),
                (0, 0, ""),
                "{context} {tool}: {ran:?}"
            );
        }
    };
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    closed("draft");
    pair.plan_ok(&main, &id, &["Holds."], &[]);
    pair.approve_task(&main, &id);
    closed("ready");
    pair.claim_ok(&main, &id, &main);
    open("claimed");
    pair.report_ok(&main, &id);
    open("reported");
    pair.complete(&main, &id).unwrap();
    closed("done");
    // An unrelated project with the plugin: passes.
    let loose = pair.scratch.dir("unrelated");
    let ran = wrapper(
        &pair,
        &loose,
        hook_json("Write", &loose, loose.join("x.py").to_str().unwrap(), 10),
    );
    assert_eq!((ran.code, ran.stderr.as_str()), (0, ""), "{ran:?}");
}

// ------------------------------------------------------- iteration 2: input

/// A hook input over a mebibyte is read and decided: ungated it passes
/// (library, binary, wrapper); gated and unclaimed it is a real refusal,
/// not an input failure; 30 MB gated and claimed passes within the
/// deadline. M: a capped reader.
#[test]
fn a_large_write_is_read_to_its_end_and_decided() {
    let plain = pair_with("pg-big-plain", "spec-a", None);
    let main = plain.main.clone();
    let file = main.join("src/big.rs");
    let big = hook_json("Write", &main, file.to_str().unwrap(), 1_100_000);
    assert!(big.len() > 1_100_000);
    let input = read_hook_input(Cursor::new(big.clone()), Instant::now() + GATE_DEADLINE);
    assert_eq!(
        input,
        GateInput::Hook(HookInput {
            cwd: Some(main.to_str().unwrap().to_owned()),
            file: Some(file.to_str().unwrap().to_owned()),
        })
    );
    assert_passed(
        &gate(
            &plain.env(&main),
            &request(&plain, &main, input, Some(&main)),
        ),
        GateReason::Off,
        "library",
    );
    let ran = spec_stdin(&plain, &main, Some(&main), Input::Bytes(big.clone()));
    assert_eq!((ran.code, ran.stderr.as_str()), (0, ""), "binary");
    let ran = spec_stdin(&plain, &main, None, Input::Bytes(big.clone()));
    assert_eq!(ran.code, 0, "no CLAUDE_PROJECT_DIR: {ran:?}");
    let ran = wrapper(&plain, &main, big);
    assert_eq!((ran.code, ran.stderr.as_str()), (0, ""), "wrapper");

    let pair = gated("pg-big", "selective");
    let main = pair.main.clone();
    let file = main.join("src/big.rs");
    let big = hook_json("Write", &main, file.to_str().unwrap(), 1_100_000);
    let ran = spec_stdin(&pair, &main, Some(&main), Input::Bytes(big.clone()));
    assert_eq!(ran.code, 2, "{ran:?}");
    assert!(
        ran.stderr.contains("src/big.rs is in the zone `code`"),
        "a real refusal: {}",
        ran.stderr
    );
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    let huge = hook_json("Write", &main, file.to_str().unwrap(), 30_000_000);
    let ran = spec_stdin(&pair, &main, Some(&main), Input::Bytes(huge));
    assert_eq!(
        (ran.code, ran.stderr.as_str()),
        (0, ""),
        "30 MB claimed: {ran:?}"
    );
    assert!(
        ran.took < GATE_DEADLINE + Duration::from_millis(500),
        "{:?}",
        ran.took
    );
    eprintln!("30 MB Write, claimed: {:?}", ran.took);
}

/// A hook input that fails to parse early is still read to its end: the
/// writer (Claude Code) never meets a closed pipe, gated or not.
#[test]
fn an_input_that_fails_to_parse_is_still_drained() {
    let pair = gated("pg-drain", "selective");
    let plain = pair_with("pg-drain-plain", "spec-a", None);
    let mut input = b"not json ".to_vec();
    input.resize(4_000_000, b'x');
    for (session, code) in [(&plain.main, 0), (&pair.main, 2)] {
        let ran = spec_stdin(&pair, session, Some(session), Input::Bytes(input.clone()));
        assert_eq!(ran.code, code, "{ran:?}");
        assert!(
            ran.wrote,
            "{}: the writer met a closed pipe",
            session.display()
        );
    }
    // A valid document followed by trailing bytes: no verdict, drained.
    let mut trailing = hook_json("Edit", &pair.main, "src/a.rs", 0);
    trailing.extend(std::iter::repeat_n(b' ', 2_000_000));
    trailing.extend(b"}");
    let ran = spec_stdin(&pair, &pair.main, Some(&pair.main), Input::Bytes(trailing));
    assert_eq!(ran.code, 2, "{ran:?}");
    assert!(ran.wrote);
}

/// The six kinds of session project.
fn sessions(pair: &Pair) -> Vec<(&'static str, PathBuf, i32, bool)> {
    let make = |name: &str, extra: Option<&str>| {
        let dir = pair.scratch.copy("spec-a", name);
        if let Some(extra) = extra {
            append_config(&dir, extra);
        }
        dir
    };
    vec![
        ("no project", pair.scratch.dir("no-project"), 0, false),
        ("no [gate]", make("s-plain", None), 0, false),
        ("off", make("s-off", Some(&tables("off"))), 0, false),
        (
            "observe",
            make("s-observe", Some(&tables("observe"))),
            0,
            true,
        ),
        (
            "selective",
            make("s-selective", Some(&tables("selective"))),
            2,
            false,
        ),
        (
            "broken",
            make("s-broken", Some("\n[gate]\nmode = \"strict\"\n")),
            2,
            false,
        ),
    ]
}

/// Every input that names no file is decided by the session's project:
/// no project, no `[gate]`, `off` pass; `observe` passes with a note;
/// `selective` and a broken `[gate]` are no verdict naming both causes;
/// a stdin left open is decided at the deadline.
#[test]
fn an_input_naming_no_file_is_decided_by_the_session() {
    let pair = pair_with("pg-session", "spec-a", None);
    let neutral = pair.scratch.dir("neutral");
    let inputs: [(&str, &[u8]); 7] = [
        ("not JSON", b"not json"),
        ("empty", b""),
        (
            "no path",
            b"{\"tool_name\":\"Edit\",\"tool_input\":{\"old_string\":\"a\"}}",
        ),
        (
            "a relative cwd",
            b"{\"cwd\":\"rel/dir\",\"tool_input\":{\"file_path\":\"src/a.rs\"}}",
        ),
        (
            "no cwd, a relative path",
            b"{\"tool_input\":{\"file_path\":\"src/a.rs\"}}",
        ),
        (
            "a path that is no string",
            b"{\"cwd\":\"/\",\"tool_input\":{\"file_path\":7}}",
        ),
        (
            "a NUL in the path",
            b"{\"cwd\":\"/\",\"tool_input\":{\"file_path\":\"/a\\u0000b\"}}",
        ),
    ];
    for (session, dir, code, note) in sessions(&pair) {
        for (label, bytes) in inputs {
            let context = format!("{session} × {label}");
            // `CLAUDE_PROJECT_DIR` decides, not the process's directory.
            let ran = if session == "no project" {
                spec_stdin(&pair, &dir, None, Input::Bytes(bytes.to_vec()))
            } else {
                spec_stdin(&pair, &neutral, Some(&dir), Input::Bytes(bytes.to_vec()))
            };
            assert_eq!(ran.code, code, "{context}: {ran:?}");
            assert!(ran.stdout.is_empty(), "{context}");
            match (code, note) {
                (0, false) => assert_eq!(ran.stderr, "", "{context}"),
                (0, true) => {
                    assert_eq!(ran.stderr.lines().count(), 1, "{context}: {}", ran.stderr);
                    assert!(
                        ran.stderr
                            .starts_with("note: specengine gate (observe): no verdict ("),
                        "{context}: {}",
                        ran.stderr
                    );
                }
                _ => {
                    assert!(
                        ran.stderr.starts_with(NO_VERDICT),
                        "{context}: {}",
                        ran.stderr
                    );
                    assert!(!ran.stderr.contains(REFUSAL_TEXT), "{context}");
                    let both = if session == "broken" {
                        "a gate config that cannot be read"
                    } else {
                        "is gated"
                    };
                    assert!(ran.stderr.contains(both), "{context}: {}", ran.stderr);
                }
            }
        }
        // Without CLAUDE_PROJECT_DIR the current directory is the session.
        if session != "no project" {
            let ran = spec_stdin(&pair, &dir, None, Input::Bytes(b"not json".to_vec()));
            assert_eq!(ran.code, code, "{session} by cwd: {ran:?}");
        }
    }

    // Stdin left open: decided by the session at the deadline, all six at
    // once.
    let handles: Vec<_> = sessions(&pair)
        .into_iter()
        .map(|(session, dir, code, _)| {
            let vars = if session == "no project" {
                vars(&pair, &[], &[])
            } else {
                vars(&pair, &[("CLAUDE_PROJECT_DIR", &dir)], &[])
            };
            let cwd = if session == "no project" {
                dir.clone()
            } else {
                neutral.clone()
            };
            std::thread::spawn(move || {
                let ran = spawn(
                    SPEC,
                    &["gate", "--stdin"],
                    &vars,
                    &cwd,
                    Input::HeldOpen(b"{\"cwd\":".to_vec()),
                );
                (session, code, ran)
            })
        })
        .collect();
    for handle in handles {
        let (session, code, ran) = handle.join().unwrap();
        assert_eq!(ran.code, code, "{session}, stdin open: {ran:?}");
        assert!(
            ran.took >= Duration::from_millis(2900),
            "{session}: {:?}",
            ran.took
        );
        assert!(
            ran.took < GATE_DEADLINE + Duration::from_millis(500),
            "{session}: {:?}",
            ran.took
        );
        if code == 2 {
            assert!(
                ran.stderr.contains("not read to its end within 3 s"),
                "{session}: {}",
                ran.stderr
            );
        }
    }

    // The library: no session at all passes; an input naming no file is
    // never an input failure outside a gated project.
    let outcome = gate(
        &pair.env(&neutral),
        &request(&pair, &neutral, GateInput::Hook(HookInput::default()), None),
    );
    assert_passed(&outcome, GateReason::NoConfig, "no session dir");
    for (session, dir, code, _) in sessions(&pair) {
        let unread = GateInput::Unread {
            cause: "the hook input is not the expected JSON".to_owned(),
            deadline: false,
        };
        let outcome = gate(
            &pair.env(&neutral),
            &request(&pair, &neutral, unread, Some(&dir)),
        );
        assert_eq!(
            i32::from(outcome.exit_code()),
            code,
            "{session}: {outcome:#?}"
        );
        if code == 2 {
            assert_eq!(outcome.reason, GateReason::Input, "{session}");
        }
    }
}

/// The stage a verdict reached, for the backstop; `gate_session` decides
/// by the session's project alone.
#[test]
fn the_progress_and_the_session_verdict_follow_the_config() {
    let pair = pair_with("pg-progress", "spec-a", None);
    let all = sessions(&pair);
    let dir_of = |name: &str| {
        all.iter()
            .find(|(session, ..)| *session == name)
            .unwrap()
            .1
            .clone()
    };
    for (session, path, stage) in [
        ("no project", "x.rs", GateStage::Open),
        ("no [gate]", "src/a.rs", GateStage::Open),
        ("off", "src/a.rs", GateStage::Open),
        ("observe", "docs/x.md", GateStage::Observe),
        ("selective", "docs/x.md", GateStage::Closed),
        ("broken", "src/a.rs", GateStage::Closed),
    ] {
        let dir = dir_of(session);
        let input = GateInput::Path {
            dir: dir.clone(),
            path: PathBuf::from(path),
        };
        let request = request(&pair, &dir, input, Some(&dir));
        assert_eq!(request.progress.stage(), GateStage::Unknown);
        let progress = request.progress.clone();
        gate(&pair.env(&dir), &request);
        assert_eq!(progress.stage(), stage, "{session}");
    }
    let cause = "no answer within 3 s".to_owned();
    let progress = GateProgress::new();
    assert_passed(
        &gate_session(None, cause.clone(), &progress),
        GateReason::NoConfig,
        "none",
    );
    for (session, reason) in [
        ("no project", Some(GateReason::NoConfig)),
        ("no [gate]", Some(GateReason::Off)),
        ("off", Some(GateReason::Off)),
        ("observe", Some(GateReason::Observed)),
        ("selective", None),
        ("broken", None),
    ] {
        let dir = dir_of(session);
        let outcome = gate_session(Some(&dir), cause.clone(), &GateProgress::new());
        match reason {
            Some(reason) => {
                assert_passed(&outcome, reason, session);
                if reason == GateReason::Observed {
                    assert_eq!(outcome.stderr_lines().len(), 1, "{session}");
                    assert!(outcome.stderr_lines()[0].starts_with("note: "), "{session}");
                }
            }
            None => {
                let line = assert_no_verdict(&outcome, session);
                assert!(line.contains(&cause), "{session}: {line}");
                assert_eq!(outcome.reason, GateReason::Deadline, "{session}");
            }
        }
    }
}

// ------------------------------------------------- iteration 2: the config

/// Under a gated root every `specengine.toml`, in any ASCII case, at any
/// depth, is controlled (zone null, "would be a project config"); an
/// existing nested config still governs its subtree. M: the raw `rel ==
/// "specengine.toml"`.
#[test]
fn every_project_config_under_a_gated_root_is_controlled() {
    let pair = gated("pg-configs", "selective");
    let main = pair.main.clone();
    for path in [
        "specengine.toml",
        "sub/specengine.toml",
        "src/x/SpecEngine.toml",
        "SPECENGINE.TOML",
        "docs/spec/specengine.toml",
    ] {
        let outcome = verdict(&pair, &main, path).tap_refused(path);
        assert_eq!(outcome.zone, None, "{path}");
        let message = &outcome.stderr_lines()[0];
        assert!(
            message.contains(&format!("{path} would be a project config"))
                || case_insensitive(&main),
            "{path}: {message}"
        );
        assert!(
            message.contains("would be a project config"),
            "{path}: {message}"
        );
    }
    // A zone whose parent is not in it: the config below is still gated.
    let crates = gated("pg-configs-crates", "selective");
    let root = crates.main.clone();
    let text = fs::read_to_string(root.join("specengine.toml"))
        .unwrap()
        .replace("code = [\"src/**\"]", "code = [\"crates/*/src/**\"]");
    fs::write(root.join("specengine.toml"), text).unwrap();
    verdict(&crates, &root, "crates/foo/specengine.toml").tap_refused("crates/foo/specengine.toml");
    let lib = verdict(&crates, &root, "crates/foo/src/lib.rs").tap_refused("crates/foo/src/lib.rs");
    assert_eq!(lib.zone.as_deref(), Some("code"));
    assert_passed(
        &verdict(&crates, &root, "crates/foo/README.md"),
        GateReason::NotControlled,
        "README",
    );
    // Off: no config is controlled.
    let off = gated("pg-configs-off", "off");
    for path in ["specengine.toml", "sub/specengine.toml", "src/a.rs"] {
        assert_passed(&verdict(&off, &off.main, path), GateReason::Off, path);
    }
    // An existing nested config governs its own subtree.
    common::write(
        &main,
        "sub/specengine.toml",
        "[project]\nslug = \"sub-project\"\n",
    );
    common::write(&main, "sub/src/a.rs", "fn a() {}\n");
    let nested = verdict(&pair, &main, "sub/src/a.rs");
    assert_passed(&nested, GateReason::Off, "under an existing nested config");
    assert_eq!(nested.root.as_deref(), main.join("sub").to_str());
    // The root config named in another case on disk (a case-insensitive
    // volume finds it).
    if case_insensitive(&main) {
        let renamed = gated("pg-configs-case", "selective");
        let root = renamed.main.clone();
        fs::rename(root.join("specengine.toml"), root.join("SpecEngine.toml")).unwrap();
        let outcome =
            verdict(&renamed, &root, "SpecEngine.toml").tap_refused("SpecEngine.toml on disk");
        assert_eq!(outcome.zone, None);
        assert_eq!(outcome.root.as_deref(), root.to_str());
    } else {
        eprintln!("note: case-sensitive volume: the on-disk SpecEngine.toml case skipped");
    }
}

// ------------------------------------------------ iteration 2: symlinks, ~

/// Symbolic links: a dangling final link is followed (bounded); a zone
/// file linking outside stays controlled as written; a loop is no verdict
/// under a gated root. M: only the canonical candidate judged.
#[test]
fn symbolic_links_are_judged_as_written_and_followed() {
    let pair = gated("pg-links", "selective");
    let main = pair.main.clone();
    fs::create_dir_all(main.join("notes")).unwrap();
    symlink("../src/new.rs", main.join("notes/x.rs")).unwrap();
    let outside = pair.scratch.dir("outside");
    fs::write(outside.join("target.rs"), "x\n").unwrap();
    symlink(outside.join("target.rs"), main.join("src/out.rs")).unwrap();
    symlink("c2", main.join("docs/c1")).unwrap();
    symlink("../src/c.rs", main.join("docs/c2")).unwrap();
    symlink("l2", main.join("docs/l1")).unwrap();
    symlink("l1", main.join("docs/l2")).unwrap();

    let dangling = verdict(&pair, &main, "notes/x.rs").tap_refused("dangling");
    assert_eq!(
        (dangling.path.as_deref(), dangling.zone.as_deref()),
        (Some("src/new.rs"), Some("code"))
    );
    let out = verdict(&pair, &main, "src/out.rs").tap_refused("out of the root");
    assert_eq!(out.path.as_deref(), Some("src/out.rs"));
    let chain = verdict(&pair, &main, "docs/c1").tap_refused("a chain");
    assert_eq!(chain.path.as_deref(), Some("src/c.rs"));
    let looped = verdict(&pair, &main, "docs/l1");
    let line = assert_no_verdict(&looped, "a loop, gated");
    assert!(
        line.contains(&format!(
            "a chain of more than {LINK_HOPS} dangling symbolic links"
        )),
        "{line}"
    );
    assert_eq!(LINK_HOPS, 16);

    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    for path in [
        "notes/x.rs",
        "src/out.rs",
        "docs/c1",
        "link-free/../src/a.rs",
    ] {
        assert_passed(&verdict(&pair, &main, path), GateReason::Claimed, path);
    }
    // Under observe a loop passes with a note; ungated it passes.
    let observe = gated("pg-links-observe", "observe");
    symlink("l2", observe.main.join("docs/l1")).unwrap();
    symlink("l1", observe.main.join("docs/l2")).unwrap();
    let outcome = verdict(&observe, &observe.main, "docs/l1");
    assert_passed(&outcome, GateReason::Observed, "a loop, observe");
    assert_eq!(outcome.stderr_lines().len(), 1);
    let plain = pair_with("pg-links-plain", "spec-a", None);
    symlink("l2", plain.main.join("docs/l1")).unwrap();
    symlink("l1", plain.main.join("docs/l2")).unwrap();
    assert_passed(
        &verdict(&plain, &plain.main, "docs/l1"),
        GateReason::Off,
        "a loop, ungated",
    );
}

/// A leading `~` is expanded against `HOME`; without `HOME` the session
/// decides; `~user` stays as written.
#[test]
fn a_leading_tilde_is_home() {
    let pair = gated("pg-tilde", "selective");
    let plain = pair_with("pg-tilde-plain", "spec-a", None);
    symlink(&pair.main, pair.home.join("proj")).unwrap();
    symlink(&plain.main, pair.home.join("plain")).unwrap();
    let neutral = pair.scratch.dir("neutral");
    let hooked = |file: &str, home: bool, session: &Path| {
        let input = read_hook_input(
            Cursor::new(hook_json("Edit", &neutral, file, 0)),
            Instant::now() + GATE_DEADLINE,
        );
        let mut env = pair.env(&neutral);
        if !home {
            env.home = None;
        }
        gate(&env, &request(&pair, &neutral, input, Some(session)))
    };
    let refused = hooked("~/proj/src/a.rs", true, &neutral).tap_refused("~/proj");
    assert_eq!(refused.path.as_deref(), Some("src/a.rs"));
    assert_passed(
        &hooked("~/plain/src/a.rs", true, &neutral),
        GateReason::Off,
        "~/plain",
    );
    let by_flag = verdict(&pair, &neutral, "~/proj/src/a.rs").tap_refused("--path ~/proj");
    assert_eq!(by_flag.path.as_deref(), Some("src/a.rs"));
    // HOME unset: the session's project decides.
    let gated_session = hooked("~/proj/src/a.rs", false, &pair.main);
    let line = assert_no_verdict(&gated_session, "no HOME, a gated session");
    assert!(line.contains("HOME"), "{line}");
    assert_passed(
        &hooked("~/proj/src/a.rs", false, &plain.main),
        GateReason::Off,
        "no HOME, ungated session",
    );
    // `~user` is a name: joined to the session's directory.
    let main = pair.main.clone();
    let input = read_hook_input(
        Cursor::new(hook_json("Edit", &main, "~user/x.rs", 0)),
        Instant::now() + GATE_DEADLINE,
    );
    let outcome = gate(&pair.env(&main), &request(&pair, &main, input, Some(&main)));
    assert_passed(&outcome, GateReason::NotControlled, "~user");
    assert_eq!(outcome.path.as_deref(), Some("~user/x.rs"));
}

// --------------------------------------------------------- genre neutrality

/// The same rules on a project of another nature (`fixtures/spec-c`, a web
/// service in Python): its own zone name, its own slug.
#[test]
fn a_non_rust_project_takes_the_same_gate() {
    let pair = Pair::new("pg-spec-c", "spec-c");
    let main = pair.main.clone();
    append_config(
        &main,
        "\n[zones]\napp = [\"src/**\", \"pyproject.toml\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"app\"]\n",
    );
    pair.commit_all(&main, "gate");
    let src = fs::read_dir(main.join("src"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let rel = src
        .strip_prefix(&main)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let outcome = verdict(&pair, &main, &rel);
    let outcome = if src.is_dir() {
        verdict(&pair, &main, &format!("{rel}/new.py"))
    } else {
        outcome
    };
    assert_refused(&outcome, "spec-c src");
    assert_eq!(outcome.zone.as_deref(), Some("app"));
    assert_refused(&verdict(&pair, &main, "pyproject.toml"), "pyproject.toml");
    assert_passed(
        &verdict(&pair, &main, "tests/test_x.py"),
        GateReason::NotControlled,
        "tests",
    );
    let message = &outcome.stderr_lines()[0];
    for word in ["rust", "cargo", "crate", ".rs"] {
        assert!(
            !message.to_lowercase().contains(word),
            "{word} in {message}"
        );
    }
    let _ = Exit::Answered;
}

// ------------------------------------------------- spec-serve AC-D9

/// Every verdict of the AC-D9 cases: label, exit code, stdout, stderr —
/// through the library, the binary (text and `--json`) and the wrapper,
/// a pass (`main`, claimed) and a refusal (`t1`, unclaimed) each.
fn verdicts(pair: &Pair) -> Vec<(String, i32, String, String)> {
    let mut out = Vec::new();
    for (label, dir) in [("pass", &pair.main), ("refusal", &pair.linked)] {
        let outcome = verdict(pair, dir, "src/a.rs");
        out.push((
            format!("{label} library"),
            i32::from(outcome.exit_code()),
            outcome.to_json(),
            outcome.stderr_lines().join("\n"),
        ));
        for json in [false, true] {
            let ran = spec_path(pair, dir, "src/a.rs", json);
            out.push((
                format!("{label} spec gate json={json}"),
                ran.code,
                ran.stdout_text(),
                ran.stderr,
            ));
        }
        let file = dir.join("src/a.rs");
        let ran = wrapper(pair, dir, hook_json("Edit", dir, file.to_str().unwrap(), 0));
        out.push((
            format!("{label} wrapper"),
            ran.code,
            ran.stdout_text(),
            ran.stderr,
        ));
    }
    out
}

/// spec-serve AC-D9: a pass and a refusal give identical exit codes,
/// stdout and stderr with no daemon ever started, with a daemon up under
/// the same `HOME` (serving this repository) and after it is gone (its
/// `daemon.json` left). M: refuse when there is no `daemon.json`.
#[test]
fn serve_d9_the_gate_decides_alike_with_a_daemon_up_or_none() {
    let pair = gated("pg-serve-d9", "selective");
    let main = pair.main.clone();
    let id = ready_task(&pair, &main);
    pair.claim_ok(&main, &id, &main);
    let discovery = data_dir(&pair.home).join("daemon.json");

    assert!(!discovery.exists(), "no daemon ever ran under this HOME");
    let none = verdicts(&pair);
    let codes: Vec<(&str, i32)> = none
        .iter()
        .map(|(label, code, _, _)| (label.as_str(), *code))
        .collect();
    assert_eq!(
        codes,
        [
            ("pass library", 0),
            ("pass spec gate json=false", 0),
            ("pass spec gate json=true", 0),
            ("pass wrapper", 0),
            ("refusal library", 2),
            ("refusal spec gate json=false", 2),
            ("refusal spec gate json=true", 2),
            ("refusal wrapper", 2),
        ],
        "{none:#?}"
    );

    let daemon = Daemon::start(&vars(&pair, &[], &[]), &main, &[&main]);
    let published: Value =
        serde_json::from_slice(&fs::read(&discovery).expect("the daemon's daemon.json"))
            .expect("JSON");
    assert_eq!(
        published["pid"],
        json!(daemon.pid()),
        "the daemon up is this one"
    );
    let up = verdicts(&pair);
    assert_eq!(
        up, none,
        "a daemon up under the same HOME changes no verdict"
    );

    drop(daemon);
    assert!(discovery.exists(), "the stale daemon.json stays");
    let down = verdicts(&pair);
    assert_eq!(down, none, "a stale daemon.json changes no verdict");
}
