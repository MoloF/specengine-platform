//! Helpers of the docs/features/verify-tests.md (8a-1) tests ("Acceptance
//! criteria" RD-1..RD-11, TE-1..TE-9): a scratch git repository made at
//! test time from `fixtures/verify-tests` (P10; its `expected.json` left
//! out, the fixture only read) through slice 3's `lock_common` (`LockRepo`:
//! every git process in the sandbox of `common::git`, a scratch `HOME` and
//! `GIT_CONFIG_GLOBAL=/dev/null`, fixed dates), with a fake runner written
//! at `scripts/run-test` and committed as `the runner` (TE-1).
//!
//! **The fake runner** is a Perl script that stops itself (`alarm`, the
//! grandchild it may fork too); it never runs `cargo` and nothing builds.
//! It appends one record per invocation to `<vt>/log` (pid, process group,
//! parent pid, cwd, the arguments; `0x1f` apart) and acts by its last
//! argument through `<vt>/plan` (`<last argument>\t<action>` lines, `*` the
//! default, `*probe` the probe's): `pass`, `fail` (exit 101), `nomatch`
//! (exit 1: a runner that fails when no test matches, the probe's default),
//! `killed` (SIGKILL to itself), `hang`, `slow` (1.5 s, then exit 0),
//! `grandchild` (forks a TERM-ignoring grandchild, then hangs), `interrupt`
//! (forks one, then SIGINT to its parent), `flood` (1 MiB on stdout, exit
//! 1), `sentinel` (a line on stdout and one on stderr), `stdin`, `env`,
//! `write-tree`, `commit`. `<vt>` is a directory of the scratch outside the
//! worktree, passed to the runner as `VT_DIR` through the request's git
//! environment (the runner's environment, `WorktreeGit::vars()`).
//!
//! **The run** goes through the library with a consent closure, never a
//! pseudo-terminal: `verify_tests` answers what `spec verify --tests` would
//! after `require_terminal`; the binary runs only where the terminal check
//! or the usage error is the subject. Where a file descriptor is the
//! subject (stdin, stdout, the live copy, a signal), the test re-runs its
//! own test binary as a child (`rerun`): the child stands in for `spec`,
//! its stdin a pipe holding bytes, its stdout and stderr read here.
//! [`Reaper`] kills every runner process still alive when a test ends,
//! failed or not, after checking that the pid is still a runner's.
//!
//! PHASE-B (names the spec does not give, chosen here; the developer's own
//! names replace them in one place, this module):
//! - `specengine_cli::verify_tests(&Env, &Globals, &VerifyTestsRequest,
//!   Consent<'_>) -> Result<VerifyTestsOutcome, CliError>`, apply.rs's
//!   `Consent` (`&mut dyn FnMut(&str) -> bool`) asked once with the step-7
//!   listing lines and the question as its last line, as `main`'s `ask`
//!   prints them;
//! - `VerifyTestsRequest { task: Option<String>, now: String, git: GitEnv }`;
//! - `VerifyTestsOutcome::exit() -> Exit`; `Outcome::VerifyTests(Box<_>)`,
//!   printed by `render_text`, `render_json`, `Outcome::stderr_lines`;
//! - the event type `verify.tests` (the spec's), read from the queue.
//!
//! Only the 8a-1 test binaries include this module (`mod
//! verify_tests_common;`, after `mod common;` and `mod lock_common;`).
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use specengine_cli::{
    CliError, Env, GitEnv, Globals, Outcome, ProposedText, TaskClaimRequest, TaskDecisionRequest,
    TaskNewRequest, TaskOutcome, TaskPlanRequest, TaskReportRequest, render_json, render_text,
    task_approve, task_claim, task_new, task_plan, task_report,
};
// PHASE-B: the library entry of `spec verify --tests` (see the module docs).
use specengine_cli::{VerifyTestsRequest, verify_tests};
use specengine_store::{ProposalQueue as _, SqliteQueue};

use crate::common::{RUN_TIMEOUT, Run, SPEC, data_dir, fixture, snapshot};
use crate::lock_common::LockRepo;

/// The fixture (P10).
pub const FIXTURE: &str = "verify-tests";

/// Its project slug: the queue's database name.
pub const SLUG: &str = "kiln-works";

/// The runner's path, as the template writes it.
pub const RUNNER: &str = "scripts/run-test";

/// The fixture's `[verify] tests` (the spec's example template).
pub const TEMPLATE: [&str; 5] = [
    "scripts/run-test",
    "{package}",
    "{target_kind}",
    "{target}",
    "{test}",
];

/// `{test}` of every probe ("Order" step 8).
pub const PROBE: &str = "specengine_verify_probe_no_such_test";

/// The event's type ("Event").
pub const EVENT: &str = "verify.tests";

/// The clock of every library run.
pub const CLOCK: &str = "2026-10-16T10:00:00Z";

/// The committer of every scratch commit (the event's `by`).
pub const COMMITTER: &str = "Scratch Committer <committer@example.invalid>";

/// The probe's refusal ("Order" step 8).
pub const PROBE_PASSED: &str = "the [verify] tests command passed a test that does not exist; make it fail when no test matches";

/// The fixture's paths.
pub const RAMP: &str = "crates/kiln/src/ramp.rs";
pub const COOL: &str = "crates/kiln/src/cool.rs";
pub const SCHEDULE: &str = "crates/kiln/tests/schedule.rs";
pub const KILN_MD: &str = "docs/spec/kiln.md";

/// The fixture's tests (`symbol`) and their `{test}` values.
pub const RAMP_STOPS: &str = "crates/kiln::ramp::tests::ramp_stops_at_peak";
pub const RAMP_STOPS_TEST: &str = "ramp::tests::ramp_stops_at_peak";
pub const TWIN_UNIX: &str = "crates/kiln::ramp::tests::peak_is_capped #cfg(test,unix)";
pub const TWIN_NOT_UNIX: &str = "crates/kiln::ramp::tests::peak_is_capped #cfg(test,not(unix))";
pub const TWIN_TEST: &str = "ramp::tests::peak_is_capped";
pub const COOLING: &str = "crates/kiln::cool::tests::cooling_stops_at_zero";
pub const COOLING_TEST: &str = "cool::tests::cooling_stops_at_zero";
pub const HOLD: &str = "crates/kiln::test:schedule::hold_follows_the_ramp";
pub const HOLD_TEST: &str = "hold_follows_the_ramp";
pub const GLAZE: &str = "crates/kiln::test:schedule::glaze_survives_the_hold";
pub const GLAZE_TEST: &str = "glaze_survives_the_hold";
pub const SCHEDULE_MODULE: &str = "crates/kiln::test:schedule";

/// The JSON document's keys, in order ("Output").
pub const TOP_KEYS: [&str; 13] = [
    "task", "branch", "commit", "command", "timeout", "verdict", "reason", "counts", "tests",
    "nodes", "lock", "cannot", "notes",
];
pub const COUNTS_KEYS: [&str; 3] = ["nodes", "tests", "probes"];
pub const NODE_COUNT_KEYS: [&str; 4] = ["verified", "failed", "not_verifiable", "no_runnable_test"];
pub const TEST_COUNT_KEYS: [&str; 4] = ["passed", "failed", "timed_out", "not_run"];
pub const TEST_KEYS: [&str; 9] = [
    "file", "kind", "symbol", "nodes", "argv", "outcome", "exit", "signal", "reason",
];
pub const NODE_KEYS: [&str; 3] = ["id", "verdict", "state"];

/// The event payload's keys, in order ("Event").
pub const EVENT_KEYS: [&str; 12] = [
    "task",
    "worktree",
    "branch",
    "commit",
    "command",
    "timeout",
    "verdict",
    "probes",
    "tests",
    "nodes",
    "lock_commit",
    "by",
];
pub const EVENT_PROBE_KEYS: [&str; 5] = ["argv", "exit", "signal", "outcome", "output_hash"];
pub const EVENT_TEST_KEYS: [&str; 9] = [
    "file",
    "symbol",
    "nodes",
    "argv",
    "exit",
    "signal",
    "outcome",
    "output_hash",
    "tail",
];

/// `git rev-parse --local-env-vars` of git 2.50 (the store's
/// `LOCAL_ENV_VARS`): none reaches the runner (RD-6).
pub const LOCAL_GIT_VARS: [&str; 15] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// The fake runner (TE-1). Its own alarm ends it, and its grandchild's
/// ends the grandchild, whatever `spec` does.
pub const RUNNER_SCRIPT: &str = r#"#!/usr/bin/perl
# A fake test runner of the verify-tests tests (docs/features/verify-tests.md
# TE-1), written into a scratch repository at test time: never cargo, never a
# build. It stops itself (SIGALRM after VT_ALARM seconds, else 30), and so
# does any grandchild it forks (60 s).
use strict;
use warnings;
use Cwd ();

alarm($ENV{VT_ALARM} || 30);
my $dir = $ENV{VT_DIR} or exit 97;
my $test = @ARGV ? $ARGV[-1] : '';
my %plan;
if (open(my $in, '<', "$dir/plan")) {
    while (my $line = <$in>) {
        chomp $line;
        my ($key, $action) = split /\t/, $line, 2;
        $plan{$key} = $action if defined $action;
    }
    close $in;
}
my $probe = $test =~ /specengine_verify_probe_no_such_test\z/;
my $action = $probe ? ($plan{'*probe'} // 'nomatch') : ($plan{$test} // $plan{'*'} // 'pass');
open(my $log, '>>', "$dir/log") or exit 98;
print {$log} join("\x1f", $$, getpgrp(), getppid(), Cwd::getcwd(), @ARGV), "\n";
close $log;

sub note_pid {
    my ($role) = @_;
    open(my $out, '>>', "$dir/pids") or return;
    print {$out} "$role $$\n";
    close $out;
}

sub grandchild_noted {
    open(my $in, '<', "$dir/pids") or return 0;
    my $found = grep { /^grandchild / } <$in>;
    close $in;
    return $found;
}

sub spawn_grandchild {
    my $pid = fork();
    if (defined $pid && $pid == 0) {
        alarm(60);
        $SIG{TERM} = 'IGNORE';
        note_pid('grandchild');
        sleep 1 while 1;
    }
    for (1 .. 100) {
        last if grandchild_noted();
        select(undef, undef, undef, 0.02);
    }
}

if ($action eq 'pass') { exit 0; }
if ($action eq 'fail') { print STDERR "assertion failed: $test\n"; exit 101; }
if ($action eq 'nomatch') { print STDERR "no test matches $test\n"; exit 1; }
if ($action eq 'killed') { kill 'KILL', $$; sleep 1 while 1; }
if ($action eq 'hang') { note_pid('leader'); sleep 1 while 1; }
if ($action eq 'slow') { select(undef, undef, undef, 1.5); exit 0; }
if ($action eq 'grandchild') { note_pid('leader'); spawn_grandchild(); sleep 1 while 1; }
if ($action eq 'interrupt') {
    note_pid('leader');
    spawn_grandchild();
    kill 'INT', getppid();
    sleep 1 while 1;
}
if ($action eq 'flood') {
    my $block = join('', map { chr(97 + $_ % 26) } 0 .. 1023);
    binmode STDOUT;
    print STDOUT $block for 1 .. 1024;
    exit 1;
}
if ($action eq 'sentinel') {
    print STDOUT "VT-RUNNER-STDOUT-SENTINEL\n";
    print STDERR "VT-RUNNER-STDERR-SENTINEL\n";
    exit 0;
}
if ($action eq 'stdin') {
    binmode STDIN;
    my $read = do { local $/; <STDIN> };
    my @fd = stat(*STDIN);
    my @null = stat('/dev/null');
    my $is_null = (@fd && @null && $fd[0] == $null[0] && $fd[1] == $null[1]) ? 1 : 0;
    open(my $out, '>', "$dir/stdin") or exit 3;
    print {$out} 'len=', length($read // ''), " null=$is_null\n";
    close $out;
    exit 0;
}
if ($action eq 'env') {
    open(my $out, '>', "$dir/env") or exit 3;
    print {$out} "$_=$ENV{$_}\0" for sort keys %ENV;
    close $out;
    exit 0;
}
if ($action eq 'write-tree') {
    open(my $out, '>', 'firing-notes.txt') or exit 3;
    print {$out} "written by a test\n";
    close $out;
    exit 0;
}
if ($action eq 'commit') {
    system('git', 'commit', '-q', '--allow-empty', '--no-verify', '-m', 'moved by a test') == 0 or exit 3;
    exit 0;
}
print STDERR "unknown action $action\n";
exit 99;
"#;

// ------------------------------------------------------------------ repo

/// A scratch repository of the fixture with the runner committed, and the
/// runner's directory `<scratch>/vt` (outside the worktree).
pub struct Kiln {
    pub repo: LockRepo,
    pub vt: PathBuf,
    /// The project slug: its queue's database name.
    pub slug: &'static str,
}

impl Kiln {
    /// `fixtures/verify-tests` at `<scratch>/repo`, committed as `the
    /// fixture`, the runner committed as `the runner`.
    pub fn new(label: &str) -> Self {
        Self::at(label, "repo")
    }

    /// [`Kiln::new`] at `<scratch>/<dir>`.
    pub fn at(label: &str, dir: &str) -> Self {
        Self::with_runner(LockRepo::of(label, FIXTURE, dir, &[]))
    }

    /// The fixture's files written in reverse creation order, then
    /// committed as [`Kiln::new`] commits them (TE-7).
    pub fn reversed(label: &str) -> Self {
        let repo = LockRepo::uncommitted(label);
        fs::remove_dir_all(&repo.root).expect("clear the copy");
        let mut files: Vec<_> = snapshot(&fixture(FIXTURE))
            .into_iter()
            .filter(|(path, bytes)| bytes.is_some() && path != "expected.json")
            .collect();
        files.reverse();
        for (path, bytes) in files {
            repo.write(&path, bytes.expect("a file"));
        }
        repo.git.init(&repo.root);
        repo.commit_all("the fixture");
        Self::with_runner(repo)
    }

    /// `repo` (the fixture's copy, committed) with the runner committed as
    /// `the runner`.
    pub fn with_runner(repo: LockRepo) -> Self {
        let kiln = Self::bare(repo, SLUG);
        kiln.write_runner(RUNNER, 0o755);
        kiln.repo.commit_all("the runner");
        kiln
    }

    /// Another fixture's repository (slug `slug`), no runner written.
    pub fn bare(repo: LockRepo, slug: &'static str) -> Self {
        let vt = repo.scratch.dir("vt");
        Self { repo, vt, slug }
    }

    /// `[verify] tests` = the fixture's template appended to the config,
    /// the runner written; both committed (another fixture's repository).
    pub fn add_template_and_runner(&self) {
        let config = self.repo.read("specengine.toml");
        self.repo.write(
            "specengine.toml",
            format!("{config}\n[verify]\ntests = {}\n", toml_list(&TEMPLATE)),
        );
        self.write_runner(RUNNER, 0o755);
        self.repo.commit_all("a verify table and a runner");
    }

    /// The runner script at `relative`, mode `mode` (not committed).
    pub fn write_runner(&self, relative: &str, mode: u32) {
        self.repo.write(relative, RUNNER_SCRIPT);
        fs::set_permissions(
            self.repo.root.join(relative),
            fs::Permissions::from_mode(mode),
        )
        .expect("the runner's mode");
    }

    pub fn root(&self) -> &Path {
        &self.repo.root
    }

    /// The config with its `[verify]` table replaced by `table` (the
    /// fixture's is last), committed as `message`.
    pub fn set_verify_table(&self, table: &str, message: &str) {
        let config = self.repo.read("specengine.toml");
        let at = config
            .find("\n[verify]\n")
            .expect("the fixture's [verify] table is its last");
        self.repo
            .write("specengine.toml", format!("{}{table}", &config[..=at]));
        self.repo.commit_all(message);
    }

    /// The 1-based line of the config's first line starting `start`.
    pub fn config_line(&self, start: &str) -> usize {
        self.repo
            .read("specengine.toml")
            .lines()
            .position(|line| line.starts_with(start))
            .unwrap_or_else(|| panic!("no config line starting {start:?}"))
            + 1
    }

    /// `[verify] tests = template` (TOML strings), `timeout` when given,
    /// committed.
    pub fn set_verify(&self, template: &[&str], timeout: Option<u32>) {
        let mut table = format!("[verify]\ntests = {}\n", toml_list(template));
        if let Some(timeout) = timeout {
            table.push_str(&format!("timeout = {timeout}\n"));
        }
        self.set_verify_table(&table, "the verify table");
    }

    /// The runner's plan: `(last argument, action)`; `*` the default,
    /// `*probe` the probe's. Replaces the plan before.
    pub fn plan(&self, entries: &[(&str, &str)]) {
        let mut text = String::new();
        for (key, action) in entries {
            text.push_str(&format!("{key}\t{action}\n"));
        }
        fs::write(self.vt.join("plan"), text).expect("the plan");
    }

    /// The runner's log, the pid list and the files the runner wrote,
    /// removed (between two runs of one test).
    pub fn clear_runner_files(&self) {
        for name in ["log", "pids", "stdin", "env"] {
            let _ = fs::remove_file(self.vt.join(name));
        }
    }

    /// Every variable the request's git environment carries: the
    /// sandbox's, `VT_DIR`, then `extra` (last wins).
    pub fn vars(&self, extra: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        let mut vars = self.repo.git.vars();
        vars.push(("VT_DIR".into(), self.vt.clone().into_os_string()));
        for (name, value) in extra {
            vars.retain(|(known, _)| known != OsStr::new(name));
            vars.push(((*name).into(), (*value).into()));
        }
        vars
    }

    pub fn git_env(&self, cwd: &Path, extra: &[(&str, &str)]) -> GitEnv {
        GitEnv::new(cwd, self.vars(extra))
    }

    // ------------------------------------------------------------- runs

    /// `spec verify --tests` from the root, consent `y`.
    pub fn run(&self) -> TestsRun {
        self.run_with(&self.repo.root, None, true, &[])
    }

    /// `spec verify --tests --task <task>` from the root, consent `y`.
    pub fn run_task(&self, task: &str) -> TestsRun {
        self.run_with(&self.repo.root, Some(task), true, &[])
    }

    /// `spec verify --tests [--task T]` through the library from `cwd`,
    /// consent `answer`, the request's git environment [`Kiln::vars`]
    /// with `extra`. The runner's log and pid list are cleared first.
    pub fn run_with(
        &self,
        cwd: &Path,
        task: Option<&str>,
        answer: bool,
        extra: &[(&str, &str)],
    ) -> TestsRun {
        self.run_at(cwd, task, answer, extra, CLOCK)
    }

    /// [`Kiln::run_with`] at the clock `now` (docs/features/tested-evidence.md
    /// TE-2: another day, another time of day).
    pub fn run_at(
        &self,
        cwd: &Path,
        task: Option<&str>,
        answer: bool,
        extra: &[(&str, &str)],
        now: &str,
    ) -> TestsRun {
        self.clear_runner_files();
        let mut prompts: Vec<String> = Vec::new();
        let mut consent = |text: &str| {
            prompts.push(text.to_owned());
            answer
        };
        let result = verify_tests(
            &self.repo.env(cwd),
            &Globals::default(),
            &VerifyTestsRequest {
                task: task.map(str::to_owned),
                now: now.to_owned(),
                git: self.git_env(cwd, extra),
            },
            &mut consent,
        );
        TestsRun::of(result, prompts)
    }

    // ----------------------------------------------------------- runner

    /// The runner's invocations, in order.
    pub fn log(&self) -> Vec<Invocation> {
        let Ok(bytes) = fs::read(self.vt.join("log")) else {
            return Vec::new();
        };
        bytes
            .split(|byte| *byte == b'\n')
            .filter(|record| !record.is_empty())
            .map(|record| {
                let fields: Vec<String> = record
                    .split(|byte| *byte == 0x1f)
                    .map(|field| String::from_utf8(field.to_vec()).expect("a UTF-8 field"))
                    .collect();
                assert!(fields.len() >= 4, "a log record: {fields:?}");
                Invocation {
                    pid: fields[0].parse().expect("a pid"),
                    pgid: fields[1].parse().expect("a process group"),
                    ppid: fields[2].parse().expect("a parent pid"),
                    cwd: fields[3].clone(),
                    args: fields[4..].to_vec(),
                }
            })
            .collect()
    }

    /// The arguments of every invocation that is no probe, in order.
    pub fn test_invocations(&self) -> Vec<Vec<String>> {
        self.log()
            .into_iter()
            .filter(|call| !call.is_probe())
            .map(|call| call.args)
            .collect()
    }

    /// The arguments of every probe, in order.
    pub fn probe_invocations(&self) -> Vec<Vec<String>> {
        self.log()
            .into_iter()
            .filter(Invocation::is_probe)
            .map(|call| call.args)
            .collect()
    }

    /// No runner process started.
    pub fn assert_nothing_started(&self, context: &str) {
        assert!(
            !self.vt.join("log").exists(),
            "{context}: the runner started: {:?}",
            self.log()
        );
    }

    /// `(role, pid)` of every process the runner noted (`leader`,
    /// `grandchild`).
    pub fn noted_pids(&self) -> Vec<(String, u32)> {
        fs::read_to_string(self.vt.join("pids"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let (role, pid) = line.split_once(' ')?;
                Some((role.to_owned(), pid.parse().ok()?))
            })
            .collect()
    }

    /// A file the runner wrote under `<vt>`.
    pub fn runner_file(&self, name: &str) -> Vec<u8> {
        fs::read(self.vt.join(name)).unwrap_or_else(|error| panic!("<vt>/{name}: {error}"))
    }

    /// Kills, at drop, every runner process of this test still alive.
    pub fn reaper(&self) -> Reaper {
        Reaper {
            vt: self.vt.clone(),
        }
    }

    // ------------------------------------------------------------ queue

    /// The slug's queue database.
    pub fn db(&self) -> PathBuf {
        data_dir(&self.repo.home).join(format!("{}.db", self.slug))
    }

    /// The payload texts of every `verify.tests` event, by `seq` (none
    /// when the database is absent).
    pub fn events(&self) -> Vec<String> {
        let db = self.db();
        if !db.exists() {
            return Vec::new();
        }
        let queue = SqliteQueue::open(&db, self.slug).expect("the queue opens");
        // `None` gives only the tail's position, no row: read from seq 0.
        queue
            .events_after(Some(0), 100_000)
            .expect("the events")
            .events
            .into_iter()
            .filter(|event| event.event_type == EVENT)
            .map(|event| event.payload)
            .collect()
    }

    /// The one `verify.tests` event's payload, parsed.
    pub fn one_event(&self) -> Value {
        let events = self.events();
        assert_eq!(events.len(), 1, "one verify.tests event: {events:#?}");
        serde_json::from_str(&events[0]).expect("a JSON payload")
    }

    /// The types of every event of the slug, by `seq`.
    pub fn event_types(&self) -> Vec<String> {
        let db = self.db();
        if !db.exists() {
            return Vec::new();
        }
        let queue = SqliteQueue::open(&db, self.slug).expect("the queue opens");
        queue
            .events()
            .expect("the events")
            .into_iter()
            .map(|event| event.event_type)
            .collect()
    }

    /// The queue's dump (`SqliteQueue::dump`), empty without a database.
    pub fn dump(&self) -> String {
        let db = self.db();
        if !db.exists() {
            return String::new();
        }
        SqliteQueue::open(&db, self.slug)
            .expect("the queue opens")
            .dump()
            .expect("the dump")
    }

    // ------------------------------------------------------------ tasks

    /// `spec task new --nodes <nodes>` through the library: the new ID.
    pub fn new_task(&self, nodes: &[&str]) -> String {
        self.new_task_in(&self.repo.root, nodes)
    }

    /// [`Kiln::new_task`] from the worktree at `cwd`.
    pub fn new_task_in(&self, cwd: &Path, nodes: &[&str]) -> String {
        let outcome = task_new(
            &self.repo.env(cwd),
            &Globals::default(),
            &TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: Some("Fire the glaze".to_owned()),
                goal: Some("A glaze that survives the hold.".to_owned()),
                author_role: None,
                author_model: None,
                run: None,
                now: CLOCK.to_owned(),
                git: GitEnv::new(cwd, self.repo.git.vars()),
            },
        );
        task_ok(outcome, "task new").id.expect("a task ID")
    }

    /// `spec task plan <id>` with `criteria` and `affected`.
    pub fn plan_task(&self, id: &str, criteria: &[&str], affected: &[&str]) {
        let outcome = task_plan(
            &self.repo.env(&self.repo.root),
            &Globals::default(),
            &TaskPlanRequest {
                id: id.to_owned(),
                plan: ProposedText::Given(b"1. Fire.\n2. Check the glaze.\n".to_vec()),
                criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
                affected: affected.iter().map(|a| (*a).to_owned()).collect(),
                now: CLOCK.to_owned(),
                git: GitEnv::new(&self.repo.root, self.repo.git.vars()),
            },
        );
        task_ok(outcome, "task plan");
    }

    /// The owner's `spec task approve <id>`, consent `y`.
    pub fn approve_task(&self, id: &str) {
        let mut asked = 0;
        let mut consent = |_: &str| {
            asked += 1;
            true
        };
        let outcome = task_approve(
            &self.repo.env(&self.repo.root),
            &Globals::default(),
            &TaskDecisionRequest {
                id: id.to_owned(),
                note: None,
                now: CLOCK.to_owned(),
                git: GitEnv::new(&self.repo.root, self.repo.git.vars()),
            },
            &mut consent,
        );
        task_ok(outcome, "task approve");
        assert_eq!(asked, 1, "approve asks once");
    }

    /// `spec task claim <id>` for this worktree.
    pub fn claim_task(&self, id: &str) {
        let outcome = task_claim(
            &self.repo.env(&self.repo.root),
            &Globals::default(),
            &TaskClaimRequest {
                id: id.to_owned(),
                role: "developer".to_owned(),
                worktree: self.repo.root.clone(),
                now: CLOCK.to_owned(),
                git: GitEnv::new(&self.repo.root, self.repo.git.vars()),
            },
        );
        task_ok(outcome, "task claim");
    }

    /// `spec task report <id>` (MCP `report_run`).
    pub fn report_task(&self, id: &str) {
        let outcome = task_report(
            &self.repo.env(&self.repo.root),
            &Globals::default(),
            &TaskReportRequest {
                id: id.to_owned(),
                outcome: "completed".to_owned(),
                summary: "The tests pass on my machine.".to_owned(),
                changed_files: vec![RAMP.to_owned()],
                now: CLOCK.to_owned(),
                git: GitEnv::new(&self.repo.root, self.repo.git.vars()),
            },
        );
        task_ok(outcome, "task report");
    }

    // ----------------------------------------------------------- binary

    /// `spec args` from the root, the sandbox's variables, this `HOME`
    /// and `VT_DIR`, stdin a pipe holding `input` (closed after it).
    pub fn spec_piped_stdin(&self, args: &[&str], input: &[u8]) -> Run {
        let mut vars: Vec<(OsString, OsString)> = self
            .vars(&[])
            .into_iter()
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".into(), self.repo.home.clone().into_os_string()));
        let mut child = Command::new(SPEC)
            .env_clear()
            .envs(vars)
            .current_dir(&self.repo.root)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn spec");
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(input);
        drop(stdin);
        let (status, stdout, stderr) = wait_with_watchdog(child, RUN_TIMEOUT, "spec");
        Run {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status.code().expect("spec exited, not killed by a signal"),
            stdout: String::from_utf8(stdout).expect("UTF-8 stdout"),
            stderr: String::from_utf8(stderr).expect("UTF-8 stderr"),
        }
    }
}

/// A task command that must answer: its outcome.
fn task_ok(outcome: Result<TaskOutcome, CliError>, context: &str) -> TaskOutcome {
    let outcome = outcome.unwrap_or_else(|error| panic!("{context}: {error}"));
    assert_eq!(outcome.exit().code(), 0, "{context}: {outcome:?}");
    outcome
}

/// `["a", "b"]` as a TOML array of basic strings.
pub fn toml_list(items: &[&str]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|item| format!("\"{}\"", item.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", quoted.join(", "))
}

/// One runner invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub pid: u32,
    pub pgid: u32,
    pub ppid: u32,
    pub cwd: String,
    /// `argv[1..]`: the arguments after the program.
    pub args: Vec<String>,
}

impl Invocation {
    pub fn is_probe(&self) -> bool {
        self.args.last().is_some_and(|last| last.ends_with(PROBE))
    }
}

/// The expanded argv a JSON test row carries, without element 0.
pub fn args_of(row: &Value) -> Vec<String> {
    let argv = row["argv"].as_array().expect("an argv");
    assert_eq!(argv[0], RUNNER, "element 0 as written: {row}");
    argv[1..]
        .iter()
        .map(|arg| arg.as_str().expect("a string").to_owned())
        .collect()
}

/// `strings` as owned strings.
pub fn owned(strings: &[&str]) -> Vec<String> {
    strings.iter().map(|text| (*text).to_owned()).collect()
}

// ------------------------------------------------------------------- run

/// One `spec verify --tests`: its exit, the outcome as printed, the
/// consent texts.
pub struct TestsRun {
    pub code: i32,
    /// `Some` when the call answered (exit 0, 1 or 2 after discovery).
    pub outcome: Option<Outcome>,
    /// `Some` when the call failed: stdout empty.
    pub error: Option<CliError>,
    /// Each consent text: the listing lines, the question last.
    pub prompts: Vec<String>,
}

impl TestsRun {
    fn of(
        result: Result<specengine_cli::VerifyTestsOutcome, CliError>,
        prompts: Vec<String>,
    ) -> Self {
        match result {
            Ok(outcome) => Self {
                code: i32::from(outcome.exit().code()),
                outcome: Some(Outcome::VerifyTests(Box::new(outcome))),
                error: None,
                prompts,
            },
            Err(error) => Self {
                code: i32::from(error.exit.code()),
                outcome: None,
                error: Some(error),
                prompts,
            },
        }
    }

    /// stdout's text (empty when the call failed).
    pub fn text(&self) -> String {
        self.outcome.as_ref().map(render_text).unwrap_or_default()
    }

    /// `--json`'s stdout (empty when the call failed).
    pub fn json_text(&self) -> String {
        self.outcome.as_ref().map(render_json).unwrap_or_default()
    }

    /// `--json`'s document.
    pub fn json(&self) -> Value {
        let text = self.json_text();
        assert!(!text.is_empty(), "no document:\n{}", self.show());
        crate::common::one_json_document(&text)
            .unwrap_or_else(|problem| panic!("{problem}\n{}", self.show()))
    }

    /// The stderr lines after the run (notes, warnings; the failure).
    pub fn stderr_lines(&self) -> Vec<String> {
        match (&self.outcome, &self.error) {
            (Some(outcome), _) => outcome.stderr_lines(),
            (None, Some(error)) => error.message.lines().map(str::to_owned).collect(),
            (None, None) => Vec::new(),
        }
    }

    pub fn show(&self) -> String {
        format!(
            "exit {}\n--- stdout\n{}--- stderr\n{}\n--- prompts\n{}",
            self.code,
            self.text(),
            self.stderr_lines().join("\n"),
            self.prompts.join("\n---\n")
        )
    }

    /// Asserts the exit code.
    pub fn code(&self, code: i32) -> &Self {
        assert_eq!(self.code, code, "{}", self.show());
        self
    }

    /// The call failed with exit 2, stdout empty: its message.
    pub fn failed(&self, context: &str) -> String {
        let error = self
            .error
            .as_ref()
            .unwrap_or_else(|| panic!("{context}: stdout not empty:\n{}", self.show()));
        assert_eq!(error.exit.code(), 2, "{context}: {}", error.message);
        error.message.clone()
    }

    /// Exit 2 with verdict `cannot` or a failed call: every text it gives
    /// (the message, or the document's `cannot` and `reason`).
    pub fn cannot_text(&self, context: &str) -> String {
        assert_eq!(self.code, 2, "{context}: {}", self.show());
        if let Some(error) = &self.error {
            return error.message.clone();
        }
        let json = self.json();
        assert_eq!(json["verdict"], "cannot", "{context}: {json}");
        let mut texts: Vec<String> = json["cannot"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_owned))
            .collect();
        if let Some(reason) = json["reason"].as_str() {
            texts.push(reason.to_owned());
        }
        texts.join("\n")
    }

    pub fn verdict(&self) -> String {
        self.json()["verdict"]
            .as_str()
            .expect("a verdict")
            .to_owned()
    }

    /// stdout's lines.
    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(str::to_owned).collect()
    }

    /// stdout's summary line: the last starting `verify --tests: ` or
    /// `verify --tests --task ` (the
    /// last line before docs/features/tested-evidence.md, whose `lock | …`
    /// rows and `committed <oid>` or `lock: <refusal>` follow it).
    pub fn summary(&self) -> String {
        self.lines()
            .into_iter()
            .rev()
            .find(|line| {
                line.starts_with("verify --tests: ") || line.starts_with("verify --tests --task ")
            })
            .unwrap_or_default()
    }

    /// stdout holds `line` as a whole line.
    pub fn has_line(&self, line: &str) {
        assert!(
            self.lines().contains(&line.to_owned()),
            "no line {line:?} in:\n{}",
            self.show()
        );
    }

    /// The JSON test rows.
    pub fn rows(&self) -> Vec<Value> {
        self.json()["tests"]
            .as_array()
            .expect("`tests` is an array")
            .clone()
    }

    /// The one JSON test row of `symbol`.
    pub fn row(&self, symbol: &str) -> Value {
        let found: Vec<Value> = self
            .rows()
            .into_iter()
            .filter(|row| row["symbol"] == symbol)
            .collect();
        assert_eq!(
            found.len(),
            1,
            "one row {symbol}: {found:#?}\n{}",
            self.show()
        );
        found.into_iter().next().expect("one row")
    }

    /// The JSON node rows.
    pub fn nodes(&self) -> Vec<Value> {
        self.json()["nodes"]
            .as_array()
            .expect("`nodes` is an array")
            .clone()
    }

    /// The one node row of `id`.
    pub fn node(&self, id: &str) -> Value {
        let found: Vec<Value> = self
            .nodes()
            .into_iter()
            .filter(|node| node["id"] == id)
            .collect();
        assert_eq!(found.len(), 1, "one node {id}: {found:#?}\n{}", self.show());
        found.into_iter().next().expect("one node")
    }

    /// The JSON `notes`.
    pub fn notes(&self) -> Vec<String> {
        self.json()["notes"]
            .as_array()
            .expect("`notes` is an array")
            .iter()
            .map(|note| note.as_str().expect("a note").to_owned())
            .collect()
    }

    /// The one consent text's lines: the listing, then the question.
    pub fn asked(&self) -> Vec<String> {
        assert_eq!(self.prompts.len(), 1, "one question:\n{}", self.show());
        self.prompts[0].lines().map(str::to_owned).collect()
    }

    /// The question (the consent text's last line).
    pub fn question(&self) -> String {
        self.asked().last().cloned().expect("a question")
    }

    /// The listing (the consent text's lines before the question).
    pub fn listing(&self) -> Vec<String> {
        let mut lines = self.asked();
        lines.pop();
        lines
    }
}

// ----------------------------------------------------------- processes

/// `ps`' view of `pid`: `None` gone or a zombie, or no longer a runner
/// (its command line names no `scripts/run-test`: the pid was reused).
pub fn live_runner(pid: u32) -> Option<String> {
    let output = Command::new("/bin/ps")
        .args(["-o", "stat=", "-o", "command=", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .output()
        .expect("run /bin/ps");
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() || text.starts_with('Z') || !text.contains(RUNNER) {
        return None;
    }
    Some(text)
}

/// Waits up to `within` for every pid in `pids` to be gone; the ones left.
pub fn survivors(pids: &[u32], within: Duration) -> Vec<(u32, String)> {
    let started = Instant::now();
    loop {
        let left: Vec<(u32, String)> = pids
            .iter()
            .filter_map(|pid| live_runner(*pid).map(|text| (*pid, text)))
            .collect();
        if left.is_empty() || started.elapsed() > within {
            return left;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// At drop, `SIGKILL` to every runner process of `<vt>` still alive (each
/// pid checked to be a runner's first), so a failed assertion leaves no
/// process behind.
pub struct Reaper {
    vt: PathBuf,
}

impl Drop for Reaper {
    fn drop(&mut self) {
        let mut pids: BTreeSet<u32> = BTreeSet::new();
        if let Ok(log) = fs::read(self.vt.join("log")) {
            for record in log.split(|byte| *byte == b'\n') {
                if let Some(pid) = record
                    .split(|byte| *byte == 0x1f)
                    .next()
                    .and_then(|field| std::str::from_utf8(field).ok())
                    .and_then(|field| field.parse().ok())
                {
                    pids.insert(pid);
                }
            }
        }
        for line in fs::read_to_string(self.vt.join("pids"))
            .unwrap_or_default()
            .lines()
        {
            if let Some(pid) = line.split_once(' ').and_then(|(_, pid)| pid.parse().ok()) {
                pids.insert(pid);
            }
        }
        for pid in pids {
            if live_runner(pid).is_some() {
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", &pid.to_string()])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }
}

/// Waits for `child` with its stdout and stderr read in threads; killed
/// and a panic past `limit`.
pub fn wait_with_watchdog(
    mut child: std::process::Child,
    limit: Duration,
    what: &str,
) -> (ExitStatus, Vec<u8>, Vec<u8>) {
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
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what} ran longer than {limit:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    (
        status,
        out.join().expect("the stdout reader"),
        err.join().expect("the stderr reader"),
    )
}

// -------------------------------------------------------------- re-runs

/// The variable a re-run child finds its directory in.
const CHILD_OUT: &str = "VT_CHILD_OUT";

/// A re-run of one test of this binary as the `spec` stand-in.
pub struct ChildRun {
    pub status: ExitStatus,
    /// The child's whole stdout: the harness' lines and the text it printed.
    pub stdout: Vec<u8>,
    /// The child's whole stderr: the live copy of the runner's output.
    pub stderr: Vec<u8>,
    pub elapsed: Duration,
    /// The exit `spec verify --tests` gave in the child.
    pub code: i32,
    /// Its `--json` document (empty when the call failed).
    pub json_text: String,
    /// Its stdout text, as printed.
    pub text: String,
}

impl ChildRun {
    pub fn json(&self) -> Value {
        crate::common::one_json_document(&self.json_text)
            .unwrap_or_else(|problem| panic!("the child's JSON: {problem}: {}", self.json_text))
    }

    pub fn show(&self) -> String {
        format!(
            "{:?} after {:?}; spec exit {}\n--- stdout ({} bytes)\n{}\n--- stderr ({} bytes, first 4 KiB)\n{}",
            self.status,
            self.elapsed,
            self.code,
            self.stdout.len(),
            String::from_utf8_lossy(&self.stdout),
            self.stderr.len(),
            String::from_utf8_lossy(&self.stderr[..self.stderr.len().min(4096)])
        )
    }
}

/// Runs this binary's test `name` again as a child process with exactly
/// [`Kiln::vars`] plus the child's own variables, stdin a pipe holding
/// `input` (then closed), stdout and stderr read here; killed past
/// `limit`. The child (see [`child_role`]) runs `spec verify --tests
/// [--task T]` from the root through the library, consent `y`, prints the
/// text on its stdout as `main` would and leaves the exit, the text and
/// the JSON in `<vt>/child.*`.
pub fn rerun(
    kiln: &Kiln,
    name: &str,
    task: Option<&str>,
    input: &[u8],
    limit: Duration,
) -> ChildRun {
    for file in ["child.code", "child.json", "child.txt"] {
        let _ = fs::remove_file(kiln.vt.join(file));
    }
    kiln.clear_runner_files();
    let mut vars = kiln.vars(&[]);
    vars.push((CHILD_OUT.into(), kiln.vt.clone().into_os_string()));
    vars.push((
        "VT_CHILD_ROOT".into(),
        kiln.repo.root.clone().into_os_string(),
    ));
    vars.push((
        "VT_CHILD_HOME".into(),
        kiln.repo.home.clone().into_os_string(),
    ));
    if let Some(task) = task {
        vars.push(("VT_CHILD_TASK".into(), task.into()));
    }
    let exe = std::env::current_exe().expect("this test binary");
    let mut child = Command::new(exe)
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env_clear()
        .envs(vars)
        .current_dir(&kiln.vt)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("re-run this test binary");
    let mut stdin = child.stdin.take().expect("stdin");
    let _ = stdin.write_all(input);
    drop(stdin);
    let started = Instant::now();
    let (status, stdout, stderr) = wait_with_watchdog(child, limit, name);
    let elapsed = started.elapsed();
    let code = fs::read_to_string(kiln.vt.join("child.code"))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(-1);
    ChildRun {
        status,
        stdout,
        stderr,
        elapsed,
        code,
        json_text: fs::read_to_string(kiln.vt.join("child.json")).unwrap_or_default(),
        text: fs::read_to_string(kiln.vt.join("child.txt")).unwrap_or_default(),
    }
}

/// In a child [`rerun`] started: runs `spec verify --tests` as described
/// there and returns `true` (the test returns at once); else `false`.
pub fn child_role() -> bool {
    let Some(out) = std::env::var_os(CHILD_OUT) else {
        return false;
    };
    let out = PathBuf::from(out);
    let root = PathBuf::from(std::env::var_os("VT_CHILD_ROOT").expect("VT_CHILD_ROOT"));
    let home = std::env::var_os("VT_CHILD_HOME").expect("VT_CHILD_HOME");
    let task = std::env::var("VT_CHILD_TASK").ok();
    let env = Env {
        cwd: root.clone(),
        home: Some(home),
        xdg_data_home: None,
    };
    let mut consent = |_: &str| true;
    let result = verify_tests(
        &env,
        &Globals::default(),
        &VerifyTestsRequest {
            task,
            now: CLOCK.to_owned(),
            git: GitEnv::new(&root, std::env::vars_os()),
        },
        &mut consent,
    );
    let run = TestsRun::of(result, Vec::new());
    let text = run.text();
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
    drop(stdout);
    fs::write(out.join("child.txt"), &text).expect("child.txt");
    fs::write(out.join("child.json"), run.json_text()).expect("child.json");
    fs::write(out.join("child.code"), run.code.to_string()).expect("child.code");
    true
}

// ------------------------------------------------------------ ordered JSON

/// A JSON value with every object's keys in the order written (this
/// crate's `serde_json` sorts a `Value`'s keys).
#[derive(Debug, Clone, PartialEq)]
pub enum Ordered {
    Object(Vec<(String, Ordered)>),
    Array(Vec<Ordered>),
    Scalar,
}

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> serde::de::Visitor<'de> for Seen {
            type Value = Ordered;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("JSON")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Ordered, M::Error> {
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    entries.push((key, value));
                }
                Ok(Ordered::Object(entries))
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> Result<Ordered, S::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Ordered>()? {
                    items.push(item);
                }
                Ok(Ordered::Array(items))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_i64<E>(self, _: i64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_u64<E>(self, _: u64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_f64<E>(self, _: f64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_str<E>(self, _: &str) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
        }
        deserializer.deserialize_any(Seen)
    }
}

impl Ordered {
    pub fn of(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
    }

    /// Its keys, in the order written (an object only).
    pub fn keys(&self) -> Vec<String> {
        match self {
            Self::Object(entries) => entries.iter().map(|(key, _)| key.clone()).collect(),
            other => panic!("not an object: {other:?}"),
        }
    }

    pub fn get(&self, key: &str) -> &Self {
        match self {
            Self::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("no key {key}")),
            other => panic!("not an object: {other:?}"),
        }
    }

    /// The items (an array only).
    pub fn items(&self) -> &[Ordered] {
        match self {
            Self::Array(items) => items,
            other => panic!("not an array: {other:?}"),
        }
    }
}

/// `expected.json` of the fixture.
pub fn expected() -> Value {
    let path = fixture(FIXTURE).join("expected.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).expect("expected.json is JSON")
}

/// A JSON array of strings as strings.
pub fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("an array: {value}"))
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}
