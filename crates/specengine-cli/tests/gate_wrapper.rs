//! docs/features/plugin-gate.md AC-07 and the wrapper's iteration-2 cases
//! (canon `docs/canon/gate.md` "Hook"): the committed
//! `plugin/specengine/hooks/gate.sh` run as the hook runs it (`sh
//! gate.sh`, the hook JSON on stdin) with a scratch `PATH` holding a
//! stand-in `spec` (or none), a `sleep` that records its PID before it
//! runs `/bin/sleep`, and a scratch `TMPDIR`.
//!
//! - AC-07: a `spec` exiting 1, 101 or killed by `SIGKILL` is 2 naming the
//!   status; 2 keeps `spec`'s stderr; one printing on stdout (an `allow`)
//!   leaves stdout empty; no `spec`: 2 with the `PATH` hint under a
//!   `[gate]` line at or above `CLAUDE_PROJECT_DIR` (else `$PWD`), an
//!   unreadable config 2, else 0. M: the code passed through. In `sh`,
//!   `dash`, `bash` (and `zsh` where present).
//! - The widened `[gate]` line: the wrapper's `grep -E` agrees with core
//!   `mentions_gate` on every form and near miss; `[gateway]` passes.
//! - Version skew: a `spec` without `gate` (`unrecognized subcommand`) is
//!   decided like a missing one: gated 2 with the reinstall hint, else 0.
//! - The limit: a `spec` that ignores `TERM` and never answers is 2 within
//!   the limit (7 s) + 0.5 s, every pipe closed, no process left.
//! - `HUP`, `INT`, `TERM` to the wrapper: 2 `(the hook was stopped)`, no
//!   temp file, no process of the run left after 1.1 s.
//! - A `SIGKILL`ed wrapper: its watchdog gone within 1.2 s (so no `USR1`
//!   is ever sent), only the documented leftovers (the temp file; the
//!   stand-in, killed here).
//! - 60 fast runs: no `sleep` of this test left after 1.2 s.
//!
//! Processes are identified only by PIDs this test started or recorded
//! (the stand-ins write their own PID; the watchdog by its parent, the
//! wrapper this test spawned); nothing is signalled by name. Every
//! wrapper runs under `perl -e 'alarm 20'`.

#![cfg(unix)]

mod common;

use std::ffi::OsString;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{Scratch, repository_root};
use specengine_core::gate::mentions_gate;

/// The wrapper's limit (`limit=7` in `gate.sh`).
const LIMIT: Duration = Duration::from_secs(7);

const PATH_HINT: &str = "specengine gate: no verdict (spec is not on PATH): the write is refused. Put the directory of spec on the PATH Claude Code starts with.";

fn script() -> PathBuf {
    repository_root().join("plugin/specengine/hooks/gate.sh")
}

/// The shells to run the wrapper in: `sh` always, the others present.
fn shells() -> Vec<&'static str> {
    ["/bin/sh", "/bin/dash", "/bin/bash", "/bin/zsh"]
        .into_iter()
        .filter(|shell| Path::new(shell).exists())
        .collect()
}

/// The status of `pid` as `ps` shows it: `None` when it is gone (a zombie
/// counts as gone: it runs nothing).
fn running(pid: u32) -> bool {
    let output = Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("ps runs");
    let stat = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    !stat.is_empty() && !stat.starts_with('Z')
}

/// Signals `pid`, a process this test started or recorded.
fn signal(pid: u32, name: &str) {
    let _ = Command::new("/bin/kill")
        .args([format!("-{name}"), pid.to_string()])
        .stderr(Stdio::null())
        .status();
}

/// `(pid, command)` of every process whose parent is `parent`.
fn children(parent: u32) -> Vec<(u32, String)> {
    let output = Command::new("/bin/ps")
        .args(["-ax", "-o", "pid=,ppid=,command="])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid: u32 = parts.next()?.parse().ok()?;
            let ppid: u32 = parts.next()?.parse().ok()?;
            (ppid == parent).then(|| (pid, parts.collect::<Vec<_>>().join(" ")))
        })
        .collect()
}

/// Polls `done` every 10 ms until it holds or `limit` passes: whether it
/// held.
fn within(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    loop {
        if done() {
            return true;
        }
        if started.elapsed() >= limit {
            return done();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// One test's scratch: `bin/` (the stand-ins, first on `PATH`), `tmp/`
/// (`TMPDIR`), the PID files.
struct Bench {
    scratch: Scratch,
    bin: PathBuf,
    tmp: PathBuf,
    /// Every PID this bench must not leave running (checked and killed on
    /// drop).
    extra: Vec<u32>,
}

impl Bench {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let bin = scratch.dir("bin");
        let tmp = scratch.dir("tmp");
        let sleeps = scratch.join("sleeps");
        Self::executable(
            &bin.join("sleep"),
            &format!(
                "#!/bin/sh\necho $$ >> '{}'\nexec /bin/sleep \"$@\"\n",
                sleeps.display()
            ),
        );
        Self {
            scratch,
            bin,
            tmp,
            extra: Vec::new(),
        }
    }

    fn executable(path: &Path, text: &str) {
        fs::write(path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// `bin/spec`: records its PID, then runs `body`.
    fn spec(&self, body: &str) {
        let pids = self.scratch.join("spec-pids");
        Self::executable(
            &self.bin.join("spec"),
            &format!("#!/bin/sh\necho $$ >> '{}'\n{body}\n", pids.display()),
        );
    }

    fn no_spec(&self) {
        let _ = fs::remove_file(self.bin.join("spec"));
    }

    fn pids(&self, file: &str) -> Vec<u32> {
        fs::read_to_string(self.scratch.join(file))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect()
    }

    fn sleeps(&self) -> Vec<u32> {
        self.pids("sleeps")
    }

    fn spec_pids(&self) -> Vec<u32> {
        self.pids("spec-pids")
    }

    fn temp_files(&self) -> Vec<String> {
        fs::read_dir(&self.tmp)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    /// A project directory holding `config` as its `specengine.toml`.
    fn project(&self, name: &str, config: &str) -> PathBuf {
        let dir = self.scratch.dir(name);
        fs::write(dir.join("specengine.toml"), config).unwrap();
        dir
    }

    fn vars(&self, project: Option<&Path>) -> Vec<(OsString, OsString)> {
        let mut path = OsString::from(&self.bin);
        path.push(":/usr/bin:/bin");
        let mut vars = vec![
            ("PATH".into(), path),
            ("TMPDIR".into(), self.tmp.clone().into()),
            ("HOME".into(), self.scratch.dir("home").into()),
        ];
        if let Some(project) = project {
            vars.push(("CLAUDE_PROJECT_DIR".into(), project.into()));
        }
        vars
    }

    /// The wrapper in `shell`, started (under a 20 s alarm), `input` on
    /// its stdin, then closed.
    fn start(&self, shell: &str, cwd: &Path, project: Option<&Path>, input: &[u8]) -> Child {
        let mut child = Command::new("/usr/bin/perl")
            .args(["-e", "alarm 20; exec @ARGV", shell])
            .arg(script())
            .env_clear()
            .envs(self.vars(project))
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the wrapper starts");
        let mut stdin = child.stdin.take().unwrap();
        let _ = stdin.write_all(input);
        drop(stdin);
        child
    }

    /// The wrapper run to its end: code, stdout, stderr, the time until
    /// its pipes closed.
    fn run(&self, shell: &str, cwd: &Path, project: Option<&Path>) -> Wrapped {
        let started = Instant::now();
        let child = self.start(shell, cwd, project, HOOK);
        finish(child, started)
    }
}

impl Drop for Bench {
    /// Whatever a failed assertion left: only recorded PIDs, only while
    /// they run.
    fn drop(&mut self) {
        let mut pids = self.spec_pids();
        pids.extend(self.sleeps());
        pids.extend(&self.extra);
        for pid in pids {
            if running(pid) {
                signal(pid, "KILL");
            }
        }
    }
}

const HOOK: &[u8] = br#"{"session_id":"s","transcript_path":"/tmp/t","cwd":"/","permission_mode":"default","hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"/x/src/a.rs","old_string":"a","new_string":"b"}}"#;

#[derive(Debug)]
struct Wrapped {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
    took: Duration,
}

/// Reads `child`'s pipes to their end and reaps it (bounded: 25 s, then
/// killed); `took` counts from `started` to the pipes' close.
fn finish(mut child: Child, started: Instant) -> Wrapped {
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
    let ended = within(Duration::from_secs(25), || {
        matches!(child.try_wait(), Ok(Some(_)))
    });
    if !ended {
        let _ = child.kill();
    }
    let status = child.wait().expect("reaped");
    let stdout = out.join().unwrap();
    let stderr = String::from_utf8(err.join().unwrap()).unwrap();
    assert!(ended, "the wrapper ran past 25 s");
    Wrapped {
        code: status.code(),
        stdout,
        stderr,
        took: started.elapsed(),
    }
}

// ------------------------------------------------------------------ AC-07

/// AC-07: every code but 0 becomes 2; stdout never passes. M: the code
/// passed through.
#[test]
fn ac07_every_failure_of_spec_is_2_and_its_stdout_never_passes() {
    let bench = Bench::new("pg-w-codes");
    let cwd = bench.scratch.dir("cwd");
    for shell in shells() {
        for (body, code, needle) in [
            (
                "cat >/dev/null\nexit 1",
                2,
                "specengine gate: no verdict (spec ended with status 1): the write is refused.",
            ),
            (
                "exit 101",
                2,
                "specengine gate: no verdict (spec ended with status 101): the write is refused.",
            ),
            ("kill -KILL $$", 2, "(spec ended with status 137)"),
            ("echo 'refused by spec' >&2\nexit 2", 2, "refused by spec"),
            ("exit 0", 0, ""),
        ] {
            bench.spec(body);
            let ran = bench.run(shell, &cwd, Some(&cwd));
            let context = format!("{shell}: {body:?}");
            assert_eq!(ran.code, Some(code), "{context}: {ran:?}");
            assert!(ran.stdout.is_empty(), "{context}");
            if needle.is_empty() {
                assert_eq!(ran.stderr, "", "{context}");
            } else {
                assert!(ran.stderr.contains(needle), "{context}: {}", ran.stderr);
            }
            assert_eq!(bench.temp_files(), Vec::<String>::new(), "{context}");
        }
        // A spec printing an allow on stdout: nothing reaches Claude Code.
        bench.spec("cat >/dev/null\necho '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\"}}'\nexit 0");
        let ran = bench.run(shell, &cwd, Some(&cwd));
        assert_eq!(
            (ran.code, ran.stdout.len(), ran.stderr.as_str()),
            (Some(0), 0, ""),
            "{shell}"
        );
        bench.spec("echo allow\nexit 2");
        let ran = bench.run(shell, &cwd, Some(&cwd));
        assert_eq!((ran.code, ran.stdout.len()), (Some(2), 0), "{shell}");
        // The stdin reaches spec unchanged.
        let copy = bench.scratch.join("stdin-copy");
        bench.spec(&format!("cat > '{}'\nexit 0", copy.display()));
        let ran = bench.run(shell, &cwd, Some(&cwd));
        assert_eq!(ran.code, Some(0), "{shell}");
        assert_eq!(
            fs::read(&copy).unwrap(),
            HOOK,
            "{shell}: the hook input passed on"
        );
    }
}

/// AC-07: no `spec` on `PATH`: 2 with the hint under a `[gate]` line at or
/// above `CLAUDE_PROJECT_DIR` (else `$PWD`); an unreadable config 2; else
/// (no `[gate]`, no project) 0.
#[test]
fn ac07_no_spec_on_path_is_decided_by_the_session_project() {
    let bench = Bench::new("pg-w-nospec");
    bench.no_spec();
    let gated = bench.project(
        "gated",
        "[project]\nslug = \"g\"\n\n[gate]\nmode = \"selective\"\nzones = [\"code\"]\n",
    );
    let deep = gated.join("a/b");
    fs::create_dir_all(&deep).unwrap();
    let plain = bench.project("plain", "[project]\nslug = \"p\"\n");
    let nothing = bench.scratch.dir("nothing");
    for shell in shells() {
        for (cwd, project, code, context) in [
            (&nothing, Some(&gated), 2, "CLAUDE_PROJECT_DIR gated"),
            (&nothing, Some(&deep), 2, "below a gated config"),
            (&deep, None, 2, "$PWD below a gated config"),
            (&nothing, Some(&plain), 0, "no [gate]"),
            (&nothing, Some(&nothing), 0, "no project"),
            (&nothing, None, 0, "no project, no CLAUDE_PROJECT_DIR"),
            (&gated, Some(&plain), 0, "CLAUDE_PROJECT_DIR wins over $PWD"),
        ] {
            let ran = bench.run(shell, cwd, project.map(PathBuf::as_path));
            let context = format!("{shell}: {context}");
            assert_eq!(ran.code, Some(code), "{context}: {ran:?}");
            assert!(ran.stdout.is_empty(), "{context}");
            if code == 2 {
                assert_eq!(ran.stderr.trim_end(), PATH_HINT, "{context}");
            } else {
                assert_eq!(ran.stderr, "", "{context}");
            }
        }
        // An unreadable config refuses.
        fs::set_permissions(
            plain.join("specengine.toml"),
            fs::Permissions::from_mode(0o000),
        )
        .unwrap();
        let ran = bench.run(shell, &nothing, Some(&plain));
        fs::set_permissions(
            plain.join("specengine.toml"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(ran.code, Some(2), "{shell}: unreadable: {ran:?}");
        assert!(
            ran.stderr.contains("spec is not on PATH"),
            "{shell}: {}",
            ran.stderr
        );
    }
}

/// The wrapper's widened `[gate]` test agrees with core `mentions_gate`;
/// `[gateway]` passes.
#[test]
fn the_wrappers_gate_line_is_mentions_gate() {
    let bench = Bench::new("pg-w-lines");
    bench.no_spec();
    let nothing = bench.scratch.dir("nothing");
    let lines = [
        ("[gate]", true),
        ("  [gate]", true),
        ("[ gate ]", true),
        ("[\"gate\"]", true),
        ("['gate']", true),
        ("[gate.x]", true),
        ("gate.mode = \"selective\"", true),
        ("gate = { mode = \"off\" }", true),
        ("\"gate\".mode = 1", true),
        ("[gateway]", false),
        ("[gates]", false),
        ("gateway = 1", false),
        ("# [gate]", false),
        ("x = \"[gate]\"", false),
        ("agate = 1", false),
    ];
    for (index, (line, gated)) in lines.iter().enumerate() {
        assert_eq!(mentions_gate(line), *gated, "core: {line:?}");
        let project = bench.project(
            &format!("p{index}"),
            &format!("[project]\nslug = \"p\"\n{line}\n"),
        );
        let ran = bench.run("/bin/sh", &nothing, Some(&project));
        assert_eq!(
            ran.code,
            Some(if *gated { 2 } else { 0 }),
            "wrapper: {line:?}: {ran:?}"
        );
    }
}

/// A `spec` without `gate` is decided like a missing one, with the
/// reinstall hint.
#[test]
fn an_old_spec_is_decided_by_the_session_project() {
    let bench = Bench::new("pg-w-old");
    bench.spec("cat >/dev/null\necho \"error: unrecognized subcommand 'gate'\" >&2\necho \"Usage: spec [OPTIONS] <COMMAND>\" >&2\nexit 2");
    let gated = bench.project(
        "gated",
        "[project]\nslug = \"g\"\n[ gate ]\nmode = \"observe\"\n",
    );
    let plain = bench.project("plain", "[project]\nslug = \"p\"\n");
    let nothing = bench.scratch.dir("nothing");
    for shell in shells() {
        let ran = bench.run(shell, &nothing, Some(&gated));
        assert_eq!(ran.code, Some(2), "{shell}: {ran:?}");
        assert_eq!(
            ran.stderr.trim_end(),
            "specengine gate: no verdict (this spec has no gate): the write is refused. Reinstall spec and the plugin from one commit.",
            "{shell}"
        );
        for project in [&plain, &nothing] {
            let ran = bench.run(shell, &nothing, Some(project));
            assert_eq!(
                (ran.code, ran.stderr.as_str()),
                (Some(0), ""),
                "{shell}: {ran:?}"
            );
        }
        assert_eq!(bench.temp_files(), Vec::<String>::new(), "{shell}");
    }
}

// ------------------------------------------------------------ the limit

/// A `spec` that ignores `TERM` and never answers: 2 within the limit +
/// 0.5 s, every pipe closed, nothing of the run left.
#[test]
fn a_stalled_spec_is_2_within_the_limit() {
    let bench = Bench::new("pg-w-slow");
    bench.spec("trap '' TERM\nexec /bin/sleep 30");
    let cwd = bench.scratch.dir("cwd");
    let ran = bench.run("/bin/sh", &cwd, Some(&cwd));
    assert_eq!(ran.code, Some(2), "{ran:?}");
    assert!(ran.stdout.is_empty());
    assert_eq!(
        ran.stderr.trim_end(),
        "specengine gate: no verdict (spec gave no answer within 7 s): the write is refused."
    );
    assert!(ran.took >= LIMIT, "{:?}", ran.took);
    assert!(
        ran.took < LIMIT + Duration::from_millis(500),
        "pipes closed late: {:?}",
        ran.took
    );
    assert_eq!(bench.temp_files(), Vec::<String>::new());
    let spec = bench.spec_pids();
    assert_eq!(spec.len(), 1, "{spec:?}");
    assert!(
        within(Duration::from_millis(200), || !running(spec[0])),
        "spec {} killed",
        spec[0]
    );
    assert!(
        within(Duration::from_millis(1100), || bench
            .sleeps()
            .iter()
            .all(|pid| !running(*pid))),
        "a watchdog sleep left: {:?}",
        bench.sleeps()
    );
}

// ------------------------------------------------------------ signals

/// The wrapper's watchdog: its child running `specengine-gate-watchdog`.
fn watchdog_of(wrapper: u32) -> Option<u32> {
    children(wrapper)
        .into_iter()
        .find(|(_, command)| command.contains("specengine-gate-watchdog"))
        .map(|(pid, _)| pid)
}

/// `HUP`, `INT`, `TERM` to the wrapper (Claude Code stopping the hook): 2,
/// no temp file, no process of the run after 1.1 s.
#[test]
fn a_stopped_hook_leaves_nothing_behind() {
    for name in ["HUP", "INT", "TERM"] {
        for shell in shells() {
            let mut bench = Bench::new("pg-w-stop");
            bench.spec("trap '' TERM\nexec /bin/sleep 30");
            let cwd = bench.scratch.dir("cwd");
            let started = Instant::now();
            let child = bench.start(shell, &cwd, Some(&cwd), HOOK);
            let wrapper = child.id();
            assert!(
                within(Duration::from_secs(5), || bench.spec_pids().len() == 1),
                "{shell}: spec started"
            );
            let mut dog = None;
            assert!(
                within(Duration::from_secs(5), || {
                    dog = watchdog_of(wrapper);
                    dog.is_some()
                }),
                "{shell}: the watchdog of {wrapper}"
            );
            let dog = dog.unwrap();
            bench.extra.push(dog);
            let spec = bench.spec_pids()[0];
            assert!(running(spec) && running(dog));
            let stopped = Instant::now();
            signal(wrapper, name);
            let ran = finish(child, started);
            let context = format!("{shell} {name}");
            assert_eq!(ran.code, Some(2), "{context}: {ran:?}");
            assert!(ran.stdout.is_empty(), "{context}");
            assert_eq!(
                ran.stderr.trim_end(),
                "specengine gate: no verdict (the hook was stopped): the write is refused.",
                "{context}"
            );
            assert_eq!(
                bench.temp_files(),
                Vec::<String>::new(),
                "{context}: the temp file"
            );
            let all_gone = || {
                !running(spec) && !running(dog) && bench.sleeps().iter().all(|pid| !running(*pid))
            };
            let gone = within(
                Duration::from_millis(1100).saturating_sub(stopped.elapsed()),
                all_gone,
            );
            assert!(
                gone,
                "{context}: left after 1.1 s: spec {spec} {}, watchdog {dog} {}, sleeps {:?}",
                running(spec),
                running(dog),
                bench
                    .sleeps()
                    .iter()
                    .filter(|pid| running(**pid))
                    .collect::<Vec<_>>()
            );
        }
    }
}

/// A `SIGKILL`ed wrapper: its watchdog sees the parent gone and ends
/// within 1.2 s, before it could send `USR1` to anyone; what is left is
/// the documented temp file and the stand-in (killed here by its PID).
#[test]
fn a_killed_wrappers_watchdog_ends_and_signals_nobody() {
    for shell in shells() {
        let mut bench = Bench::new("pg-w-kill");
        bench.spec("trap '' TERM\nexec /bin/sleep 30");
        let cwd = bench.scratch.dir("cwd");
        let mut child = bench.start(shell, &cwd, Some(&cwd), HOOK);
        let wrapper = child.id();
        assert!(
            within(Duration::from_secs(5), || bench.spec_pids().len() == 1),
            "{shell}"
        );
        let mut dog = None;
        assert!(
            within(Duration::from_secs(5), || {
                dog = watchdog_of(wrapper);
                dog.is_some()
            }),
            "{shell}"
        );
        let dog = dog.unwrap();
        bench.extra.push(dog);
        let killed = Instant::now();
        child.kill().unwrap();
        child.wait().unwrap();
        let gone = within(Duration::from_millis(1200), || !running(dog));
        let after = killed.elapsed();
        assert!(
            gone,
            "{shell}: watchdog {dog} still runs {after:?} after the wrapper's SIGKILL"
        );
        // Its sleeps end too; the run is far from the limit, so the
        // watchdog never reached its `kill -USR1`.
        assert!(
            within(Duration::from_millis(1200), || bench
                .sleeps()
                .iter()
                .all(|pid| !running(*pid))),
            "{shell}: {:?}",
            bench.sleeps()
        );
        assert!(killed.elapsed() < LIMIT - Duration::from_secs(3), "{shell}");
        // The documented leftovers: the temp file, the stalled spec.
        assert_eq!(
            bench.temp_files().len(),
            1,
            "{shell}: {:?}",
            bench.temp_files()
        );
        let spec = bench.spec_pids()[0];
        assert!(
            running(spec),
            "{shell}: the stand-in outlives a SIGKILLed wrapper"
        );
        signal(spec, "KILL");
        assert!(within(Duration::from_secs(1), || !running(spec)), "{shell}");
    }
}

/// 60 fast runs: every watchdog `sleep` of this test ends within 1.2 s of
/// the last run.
#[test]
fn sixty_fast_runs_leave_no_sleep() {
    let bench = Bench::new("pg-w-fast");
    bench.spec("cat >/dev/null\nexit 0");
    let cwd = bench.scratch.dir("cwd");
    let mut runs = 0;
    for shell in shells() {
        let count = if shell == "/bin/sh" { 60 } else { 20 };
        for _ in 0..count {
            let ran = bench.run(shell, &cwd, Some(&cwd));
            assert_eq!(
                (ran.code, ran.stderr.as_str()),
                (Some(0), ""),
                "{shell}: {ran:?}"
            );
            runs += 1;
        }
    }
    assert_eq!(bench.spec_pids().len(), runs);
    let last = Instant::now();
    let gone = within(Duration::from_millis(1200), || {
        bench.sleeps().iter().all(|pid| !running(*pid))
    });
    let left: Vec<u32> = bench
        .sleeps()
        .into_iter()
        .filter(|pid| running(*pid))
        .collect();
    assert!(
        gone,
        "{} sleeps of {} still run {:?} after the last run: {left:?}",
        left.len(),
        bench.sleeps().len(),
        last.elapsed()
    );
    assert_eq!(bench.temp_files(), Vec::<String>::new());
    eprintln!(
        "{runs} fast runs, {} watchdog sleeps started, none left",
        bench.sleeps().len()
    );
}

// ------------------------------------------------- spec-serve AC-D9

/// The hook's input for an `Edit` of `file` from `cwd`.
fn edit_of(cwd: &Path, file: &Path) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "session_id": "s",
        "transcript_path": "/tmp/t",
        "cwd": cwd,
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": "Edit",
        "tool_input": {"file_path": file, "old_string": "a", "new_string": "b"},
    }))
    .unwrap()
}

/// spec-serve AC-D9 through the wrapper: with no `spec` on `PATH` (a
/// gated session refused with the hint, an ungated one passed) and with
/// the built `spec` on it (an ungated write passed, a gated one closed),
/// every shell's exit code, stdout and stderr are identical with no
/// daemon ever started, with a daemon up under the same `HOME` serving
/// both projects, and after it is gone. M: refuse when there is no
/// `daemon.json`.
#[test]
fn serve_d9_the_wrapper_decides_alike_with_a_daemon_up_or_none() {
    let bench = Bench::new("pg-w-serve-d9");
    let gated = bench.scratch.copy("spec-a", "gated");
    let config = gated.join("specengine.toml");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str(
        "\n[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"code\"]\n",
    );
    fs::write(&config, text).unwrap();
    let plain = bench.scratch.copy("spec-b", "plain");
    for root in [&gated, &plain] {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.rs"), "fn a() {}\n").unwrap();
    }
    let home = bench.scratch.dir("home");
    let discovery = common::data_dir(&home).join("daemon.json");

    let observe = || {
        let mut out = Vec::new();
        for with_spec in [false, true] {
            if with_spec {
                std::os::unix::fs::symlink(common::SPEC, bench.bin.join("spec")).unwrap();
            } else {
                bench.no_spec();
            }
            for shell in shells() {
                for (label, root) in [("gated", &gated), ("plain", &plain)] {
                    let input = edit_of(root, &root.join("src/a.rs"));
                    let started = Instant::now();
                    let child = bench.start(shell, root, Some(root), &input);
                    let ran = finish(child, started);
                    out.push((
                        format!("{shell} {label} spec={with_spec}"),
                        ran.code,
                        String::from_utf8_lossy(&ran.stdout).into_owned(),
                        ran.stderr,
                    ));
                }
            }
            bench.no_spec();
        }
        assert_eq!(bench.temp_files(), Vec::<String>::new());
        out
    };

    assert!(!discovery.exists(), "no daemon ever ran under this HOME");
    let none = observe();
    for (label, code, stdout, stderr) in &none {
        let expected = if label.contains(" gated ") { 2 } else { 0 };
        assert_eq!(*code, Some(expected), "{label}: {stderr}");
        assert!(stdout.is_empty(), "{label}: {stdout}");
        if label.contains(" gated spec=false") {
            assert_eq!(stderr.trim_end(), PATH_HINT, "{label}");
        }
        if label.contains(" plain ") {
            assert_eq!(stderr, "", "{label}");
        }
    }

    let daemon =
        common::serve::Daemon::start(&bench.vars(None), bench.scratch.path(), &[&gated, &plain]);
    let published: serde_json::Value =
        serde_json::from_slice(&fs::read(&discovery).expect("the daemon's daemon.json"))
            .expect("JSON");
    assert_eq!(published["pid"], serde_json::json!(daemon.pid()));
    let up = observe();
    assert_eq!(up, none, "a daemon up under the same HOME changes nothing");
    drop(daemon);
    let down = observe();
    assert_eq!(down, none, "a stale daemon.json changes nothing");
}
