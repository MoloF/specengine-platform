//! The auto runner's test bench (docs/features/auto-runner.md "Data", the
//! shim): `plugin/specengine/scripts/autopilot.sh` run under a shell with a
//! `claude` shim first on `PATH`, `spec` the binary under test beside it,
//! `git` and `jq` the system's, over an [`Auto`] repository.
//!
//! The shim appends its argv (one `arg` line each), the two variables and
//! its working directory to `$SHIM_LOG/<n>.call`, logs `start`/`end` lines
//! to `$SHIM_LOG/events` (appends: one line each, atomic), sleeps a second
//! (so a runner that forgot `--busy` would start a second leg), then does a
//! leg's queue work through `spec` (a plan: `spec task plan`; an implement:
//! `spec task claim`, a new file `src/<T>.rs`, `report`, `complete`; a line:
//! `spec task new` under the runner's `SPECENGINE_AUTO_LINE`, then `plan`),
//! prints the scenario's stream and exits with its code. `$SHIM_SCENARIO`
//! holds tab-separated rows `subject attempt stream queue exit hold`
//! (`attempt` a number or `*`; `queue` `yes`, `no` or `claim`; `hold` `yes`
//! keeps the leg running until `$SHIM_LOG/release` exists, at most 60 s).

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::auto_common::Auto;
use crate::common::{Run, SPEC, repository_root};

/// The script under test.
pub fn script() -> PathBuf {
    repository_root().join("plugin/specengine/scripts/autopilot.sh")
}

/// The shells the script runs under: `sh`, and `dash` and `bash` when
/// present.
pub fn shells() -> Vec<String> {
    let mut found = vec!["/bin/sh".to_owned()];
    for shell in ["/bin/dash", "/usr/bin/dash", "/bin/bash", "/usr/bin/bash"] {
        let name = Path::new(shell).file_name().unwrap().to_owned();
        if Path::new(shell).exists()
            && !found
                .iter()
                .any(|known| Path::new(known).file_name() == Some(&name))
        {
            found.push(shell.to_owned());
        }
    }
    found
}

/// `jq`'s directory: the runner needs it, so a missing one fails the test,
/// naming it.
pub fn jq_dir() -> PathBuf {
    for dir in ["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin", "/bin"] {
        if Path::new(dir).join("jq").exists() {
            return PathBuf::from(dir);
        }
    }
    panic!("jq is required: the runner needs it on PATH and none was found");
}

const SHIM: &str = r#"#!/bin/sh
# The tests' `claude`: see crates/specengine-cli/tests/auto_common/runner.rs.
log=$SHIM_LOG
k=1
while ! mkdir "$log/n.$k" 2>/dev/null; do k=$((k + 1)); done
prompt=
want=0
for a in "$@"; do
  if [ "$want" = 1 ]; then prompt=$a; want=0; fi
  [ "$a" = -p ] && want=1
done
subject=${prompt#/specengine:feature }
{
  for a in "$@"; do printf 'arg %s\n' "$a"; done
  printf 'env CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=%s\n' "${CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS-<unset>}"
  printf 'env SPECENGINE_AUTO_LINE=%s\n' "${SPECENGINE_AUTO_LINE-<unset>}"
  printf 'cwd %s\n' "$(pwd -P)"
  printf 'at %s\n' "$(date -u +%s)"
} >"$log/$k.call"
hex=$(printf '%s' "$subject" | od -An -tx1 | tr -d ' \n')
attempt=1
while ! mkdir "$log/a.$hex.$attempt" 2>/dev/null; do attempt=$((attempt + 1)); done
row=$(awk -F '\t' -v s="$subject" -v a="$attempt" '$1 == s && ($2 == a || $2 == "*") { print; exit }' "$SHIM_SCENARIO" 2>/dev/null)
stream=ok queue=yes code=0 hold=no
if [ -n "$row" ]; then
  stream=$(printf '%s' "$row" | cut -f 3)
  queue=$(printf '%s' "$row" | cut -f 4)
  code=$(printf '%s' "$row" | cut -f 5)
  hold=$(printf '%s' "$row" | cut -f 6)
fi
leg=line
case $subject in
  T-[0-9]*)
    status=$(spec --json task show "$subject" 2>/dev/null | jq -r '.status // "none"')
    case $status in
      draft | changes_requested) leg=plan ;;
      *) leg=implement ;;
    esac
    ;;
esac
printf 'start %s %s %s %s\n' "$k" "$leg" "$attempt" "$subject" >>"$log/events"
sleep 1
if [ "$hold" = yes ]; then
  printf 'held %s\n' "$k" >>"$log/events"
  i=0
  while [ ! -e "$log/release" ] && [ "$i" -lt 600 ]; do sleep 0.1; i=$((i + 1)); done
fi
if [ "$queue" != no ]; then
  case $leg in
    plan)
      printf '1. Do it.\n' | spec task plan "$subject" --plan-file - >>"$log/$k.spec" 2>&1
      ;;
    implement)
      if [ "$status" = ready ]; then
        spec task claim "$subject" --role developer --worktree "$(pwd -P)" >>"$log/$k.spec" 2>&1
      fi
      if [ "$queue" = yes ]; then
        printf 'pub const SHIM: u8 = 1;\n' >"src/$subject.rs"
        spec task report "$subject" --outcome completed --summary "Done by the shim." \
          --changed "src/$subject.rs" >>"$log/$k.spec" 2>&1
        spec task complete "$subject" >>"$log/$k.spec" 2>&1
      fi
      ;;
    line)
      made=$(spec --json task new --nodes R-01 --title "$subject" 2>>"$log/$k.spec" | jq -r '.id // empty')
      printf 'made %s %s\n' "$k" "$made" >>"$log/events"
      if [ -n "$made" ]; then
        printf '1. Do it.\n' | spec task plan "$made" --plan-file - >>"$log/$k.spec" 2>&1
      fi
      ;;
  esac
fi
init_ok='{"type":"system","subtype":"init","session_id":"s1","mcp_servers":[{"name":"plugin:specengine:specengine","status":"connected"}],"plugins":[{"name":"specengine"}]}'
init_none='{"type":"system","subtype":"init","session_id":"s1","mcp_servers":[],"plugins":[]}'
success='{"type":"result","subtype":"success","is_error":false,"num_turns":12,"total_cost_usd":1.2,"terminal_reason":"completed","result":"done","permission_denials":[]}'
case $stream in
  ok) printf '%s\n%s\n' "$init_ok" "$success" ;;
  nomcp) printf '%s\n%s\n' "$init_none" "$success" ;;
  noinit) printf '%s\n' "$success" ;;
  denied) printf '%s\n%s\n' "$init_ok" '{"type":"result","subtype":"success","is_error":false,"permission_denials":[{"tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"cargo test"}},{"tool_name":"Bash","tool_use_id":"t2","tool_input":{"command":"ls"}}]}' ;;
  maxturns) printf '%s\n%s\n' "$init_ok" '{"type":"result","subtype":"error_max_turns","is_error":true,"terminal_reason":"max_turns"}' ;;
  maxbudget) printf '%s\n%s\n' "$init_ok" '{"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted"}' ;;
  execution) printf '%s\n%s\n' "$init_ok" '{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"api_error","result":"the API failed"}' ;;
  noresult) printf '%s\n' "$init_ok" ;;
  dasherror) printf '%s\n%s\n' "$init_ok" '{"type":"result","is_error":true,"result":"boom"}' ;;
  limited)
    resets=$(( $(date -u +%s) - 58 ))
    printf '%s\n{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":%s}}\n' "$init_ok" "$resets"
    ;;
esac
printf 'end %s %s %s %s\n' "$k" "$leg" "$attempt" "$subject" >>"$log/events"
exit "$code"
"#;

/// One leg the shim saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub n: usize,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
    /// The epoch second it started.
    pub at: i64,
}

/// One `start` or `end` line of the shim's events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub start: bool,
    pub n: usize,
    pub leg: String,
    pub attempt: usize,
    pub subject: String,
}

/// The bench of one runner test.
pub struct Bench {
    pub auto: Auto,
    pub bin: PathBuf,
    pub log: PathBuf,
    pub scenario: PathBuf,
}

impl std::ops::Deref for Bench {
    type Target = Auto;
    fn deref(&self) -> &Auto {
        &self.auto
    }
}

impl Bench {
    /// `auto` with the shim and `spec` in `<scratch>/bin`.
    pub fn new(auto: Auto) -> Self {
        let bin = auto.scratch.dir("bin");
        let shim = bin.join("claude");
        fs::write(&shim, SHIM).expect("the shim");
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(SPEC, bin.join("spec")).expect("spec on PATH");
        let log = auto.scratch.dir("shim-log");
        let scenario = auto.scratch.join("scenario.tsv");
        fs::write(&scenario, "").unwrap();
        Self {
            auto,
            bin,
            log,
            scenario,
        }
    }

    /// The scenario's rows: `(subject, attempt, stream, queue, exit, hold)`.
    pub fn scenario(&self, rows: &[(&str, &str, &str, &str, i32, bool)]) {
        let text: String = rows
            .iter()
            .map(|(subject, attempt, stream, queue, exit, hold)| {
                format!(
                    "{subject}\t{attempt}\t{stream}\t{queue}\t{exit}\t{}\n",
                    if *hold { "yes" } else { "no" }
                )
            })
            .collect();
        fs::write(&self.scenario, text).unwrap();
    }

    /// `PATH`: the shim's directory, `git`'s, `jq`'s, the system's.
    pub fn path(&self) -> String {
        let git = self
            .git
            .git_program()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        format!(
            "{}:{git}:{}:/usr/bin:/bin",
            self.bin.display(),
            jq_dir().display()
        )
    }

    /// The runner under `shell` with `args`, started in the main worktree.
    pub fn spawn(&self, shell: &str, args: &[&str], path: Option<&str>) -> Child {
        let mut command = Command::new(shell);
        command
            .arg(script())
            .args(args)
            .env_clear()
            .envs(self.git.vars())
            .env("HOME", &self.home)
            .env("PATH", path.map_or_else(|| self.path(), str::to_owned))
            .env("SHIM_LOG", &self.log)
            .env("SHIM_SCENARIO", &self.scenario)
            .current_dir(&self.main)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.spawn().expect("the runner starts")
    }

    /// The runner under `shell` with `args`, to its end (a watchdog sends
    /// TERM past `limit`, then KILL 40 s on, and fails the test).
    pub fn run(&self, shell: &str, args: &[&str], limit: Duration) -> Run {
        let child = self.spawn(shell, args, None);
        finish(child, shell, args, limit)
    }

    /// The legs the shim saw, by number.
    pub fn calls(&self) -> Vec<Call> {
        let mut calls = Vec::new();
        for entry in fs::read_dir(&self.log).unwrap().flatten() {
            let name = entry.file_name().into_string().unwrap();
            let Some(n) = name.strip_suffix(".call") else {
                continue;
            };
            let text = fs::read_to_string(entry.path()).unwrap();
            let mut call = Call {
                n: n.parse().unwrap(),
                argv: Vec::new(),
                env: BTreeMap::new(),
                cwd: String::new(),
                at: 0,
            };
            for line in text.lines() {
                if let Some(arg) = line.strip_prefix("arg ") {
                    call.argv.push(arg.to_owned());
                } else if let Some(var) = line.strip_prefix("env ") {
                    let (name, value) = var.split_once('=').unwrap();
                    call.env.insert(name.to_owned(), value.to_owned());
                } else if let Some(cwd) = line.strip_prefix("cwd ") {
                    call.cwd = cwd.to_owned();
                } else if let Some(at) = line.strip_prefix("at ") {
                    call.at = at.parse().unwrap_or_default();
                } else if line == "arg" {
                    call.argv.push(String::new());
                }
            }
            calls.push(call);
        }
        calls.sort_by_key(|call| call.n);
        calls
    }

    /// The shim's `start` and `end` lines, in order.
    pub fn marks(&self) -> Vec<Mark> {
        let Ok(text) = fs::read_to_string(self.log.join("events")) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| {
                let (kind, rest) = line.split_once(' ')?;
                if kind != "start" && kind != "end" {
                    return None;
                }
                let mut parts = rest.splitn(4, ' ');
                Some(Mark {
                    start: kind == "start",
                    n: parts.next()?.parse().ok()?,
                    leg: parts.next()?.to_owned(),
                    attempt: parts.next()?.parse().ok()?,
                    subject: parts.next().unwrap_or_default().to_owned(),
                })
            })
            .collect()
    }

    /// The most legs alive at once, and every pair of subjects alive
    /// together (by their `start`/`end` order), filtered by `keep`.
    pub fn concurrency(&self, keep: impl Fn(&Mark) -> bool) -> (usize, BTreeSet<(String, String)>) {
        let mut alive: BTreeMap<usize, String> = BTreeMap::new();
        let mut most = 0;
        let mut pairs = BTreeSet::new();
        for mark in self.marks().into_iter().filter(|mark| keep(mark)) {
            if mark.start {
                for other in alive.values() {
                    let (a, b) = if *other < mark.subject {
                        (other.clone(), mark.subject.clone())
                    } else {
                        (mark.subject.clone(), other.clone())
                    };
                    pairs.insert((a, b));
                }
                alive.insert(mark.n, mark.subject.clone());
                most = most.max(alive.len());
            } else {
                alive.remove(&mark.n);
            }
        }
        (most, pairs)
    }

    /// The `<worktrees>` folder the runner defaults to.
    pub fn worktrees(&self) -> PathBuf {
        let main = &self.main;
        main.parent().unwrap().join(format!(
            "{}.auto",
            main.file_name().unwrap().to_str().unwrap()
        ))
    }

    /// Lets every held leg end.
    pub fn release(&self) {
        fs::write(self.log.join("release"), "").unwrap();
    }

    /// Every recorded argv holds `--permission-prompts none` and none of
    /// the three forbidden words (RN5).
    pub fn assert_permissions(&self) {
        for call in self.calls() {
            let joined = call.argv.join(" ");
            assert!(
                call.argv
                    .windows(2)
                    .any(|w| w[0] == "--permission-prompts" && w[1] == "none"),
                "leg {}: {joined}",
                call.n
            );
            for forbidden in [
                "--dangerously-skip-permissions",
                "--allow-dangerously-skip-permissions",
                "bypassPermissions",
            ] {
                assert!(
                    !call.argv.iter().any(|arg| arg.contains(forbidden)),
                    "leg {}: {forbidden} in {joined}",
                    call.n
                );
            }
            if let Some(at) = call.argv.iter().position(|arg| arg == "--permission-mode") {
                assert_eq!(
                    call.argv.get(at + 1).map(String::as_str),
                    Some("acceptEdits")
                );
            }
        }
    }
}

/// `child` to its end within `limit` (TERM, then KILL 40 s on: a failure).
pub fn finish(mut child: Child, shell: &str, args: &[&str], limit: Duration) -> Run {
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
    let mut termed: Option<Instant> = None;
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on the runner") {
            break status;
        }
        match termed {
            None if started.elapsed() > limit => {
                // SAFETY-free: `kill` the runner by its pid with TERM.
                let _ = Command::new("/bin/kill")
                    .args(["-TERM", &child.id().to_string()])
                    .status();
                termed = Some(Instant::now());
            }
            Some(at) if at.elapsed() > Duration::from_secs(40) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{shell} autopilot.sh {args:?} outlived TERM; killed");
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = String::from_utf8_lossy(&out.join().unwrap()).into_owned();
    let stderr = String::from_utf8_lossy(&err.join().unwrap()).into_owned();
    assert!(
        termed.is_none(),
        "{shell} autopilot.sh {args:?} ran past {limit:?}; sent TERM:\n{stderr}"
    );
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().unwrap_or(-1),
        stdout,
        stderr,
    }
}
