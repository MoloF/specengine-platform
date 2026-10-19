//! Helpers of the docs/features/layer-c-resolve.md (11a-2) tests
//! ("Acceptance criteria" R1-R15, TE-1, TE-2): `spec resolve` against fake
//! `specengine-ra` executables, never the real one (A6: the CLI tests never
//! build it; TE-2 alone, ignored, copies a built one).
//!
//! - **The lab** ([`Lab`]): a scratch copy of `fixtures/ra-mini` (the
//!   fixture is only read) plus [`CONFIG`], whose `[code]` walk lists
//!   exactly its six `.rs` files ([`FILES`]); a scratch `HOME`; and a
//!   directory holding a **copy** of the `spec` binary under test (never a
//!   hard link: the scratch resets modes on drop) with a fake `specengine-ra`
//!   beside it (R1: the binary beside `spec` wins over `PATH`). Every run is
//!   that copy, its environment cleared: `HOME`, the lab's own `PATH`
//!   (never the test process's), [`SENTINEL`], and the variables a caller
//!   passes (a git sandbox's). `spec` leads a process group of its own, so
//!   a signal a test sends reaches it and nothing else.
//! - **The fake** ([`FAKE_SCRIPT`], [`Fake`]): a Perl script written at
//!   test time. It logs each start (pid, process group, parent, cwd, argv),
//!   its environment and the request line it read, then acts by its plan
//!   (`key=value` lines in its directory): it reads the request line or
//!   never reads stdin; watches stdin for EOF as the binary does (exit 3, no
//!   line; canon `docs/canon/layer-c-protocol.md` "Watchdogs") or
//!   deliberately not; forks a grandchild that ignores `SIGTERM`; waits on
//!   a gate file; replays a reply file line by line (the golden, or one
//!   built by [`Reply`]); appends to a file of the tree before its last
//!   line; floods stdout; then exits with its code, kills itself, or hangs.
//!   Its own `alarm` ends it, and its grandchild's ends the grandchild,
//!   whatever `spec` does.
//! - **Cleanup**: a [`Fake`]'s drop kills, by pid, each process its logs
//!   name (the fake, its grandchild) that still runs its script (checked
//!   with `ps`); never by name, never a group.
//! - **The layer C dump** ([`Lab::layer_c`]): the store's canonical dump
//!   (`SqliteIndex::dump`) restricted to the three snapshot tables
//!   (`code_index_common::layer_c_lines`), the root masked.
//!
//! Readings (texts the spec gives in parts; each lives here once), fixed
//! against the developer's iteration 1 notes and review:
//! - every exit 2 of `spec resolve` past step 1 ends stderr with `spec
//!   resolve: <reason>; the previous snapshot is kept`, stdout empty
//!   ([`assert_refused`]), the refusals before the spawn too;
//! - `(signal <s>)` names the signal by its number (or its name)
//!   ([`assert_signal_ending`]);
//! - the exit-0 text is the spec's four lines, its timings masked
//!   ([`mask_timing`], [`GOLDEN_TEXT`]); JSON `ms`, `peak_rss`,
//!   `discover_ms`, `database_ms` are checked for their type only;
//! - the binary's stderr is inherited: its lines show on `spec`'s.
//!
//! Only the layer C test binaries include this module (`mod
//! layer_c_common;`, after `mod common;` and `mod code_index_common;`).
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use specengine_code::KIND_MAP;

use crate::code_index_common::{db, dump, layer_c_lines, query};
use crate::common::{Run, SPEC, Scratch, copy_file, data_dir, read_text, repository_root, write};

/// The executable `spec resolve` spawns.
pub const RA_BIN: &str = "specengine-ra";

/// The lab's `[project] slug`: its database `<slug>.db`, its layer C
/// directory `<slug>.ra`.
pub const SLUG: &str = "ra-mini";

/// The masked golden reply of 11a-1 to the whole of `fixtures/ra-mini`.
pub const GOLDEN: &str = "fixtures/layer-c/ra-mini.jsonl";

/// ra-mini's `.rs` files in the walk's byte order: the request's `files`
/// (`arena-derive/…` before `arena/…`: `-` sorts before `/`).
pub const FILES: [&str; 6] = [
    "arena-derive/src/lib.rs",
    "arena/build.rs",
    "arena/src/lib.rs",
    "arena/src/stray.rs",
    "arena/src/systems.rs",
    "tools/probe.rs",
];

/// The lab's `specengine.toml`: a slug, one prefix of its own, and a
/// `[code]` walk of exactly ra-mini's six `.rs` files.
pub const CONFIG: &str = "[project]\nslug = \"ra-mini\"\n\n[ids]\nVIT = { kind = \"vital\", width = 2 }\n\n[code]\nroots = [\"arena\", \"arena-derive\", \"tools\"]\n";

/// A variable every run carries; the fake must see it unchanged (step 4:
/// `spec`'s environment is passed on as it is).
pub const SENTINEL: (&str, &str) = ("LCR_SENTINEL", "kept as it is");

/// The tail of every exit-2 line of a run ("Output and exits").
pub const KEPT: &str = "; the previous snapshot is kept";

/// The install line of a missing binary, after `install it: `.
pub const INSTALL: &str =
    "cargo install --locked --force --path <clone>/crates/specengine-ra (rustc >= 1.98, rust-src)";

/// The crate word of every moniker [`Reply::from_index`] makes: no output
/// of a read may ever show it (R12).
pub const GENERATED: &str = "zzmoniker";

/// The escalation's grace between the group's `SIGTERM` and `SIGKILL`.
pub const GRACE: Duration = Duration::from_secs(5);

/// Slack on every time bound: a debug `spec` refreshing its index first.
pub const SLACK: Duration = Duration::from_secs(8);

/// A `spec` run longer than this is killed and the test fails.
pub const RUN_LIMIT: Duration = Duration::from_secs(90);

/// How long a pipe of `spec` may stay open after its exit.
const PIPE_BOUND: Duration = Duration::from_secs(10);

const POLL: Duration = Duration::from_millis(20);

/// The exit-0 text of the golden replay, timings masked ([`mask_timing`]):
/// the spec's four lines ("Output and exits") with the golden's stamps
/// (`host`, `toolchain` "masked", `sysroot_packages` 0).
pub const GOLDEN_TEXT: &str = "layer c: specengine-ra protocol 1, ra_ap 0.0.352, host masked\n\
workspace 1 .: discover <n> ms (toolchain masked, sysroot 0, degraded no), database <n> ms\n\
files 6 (in crate 4), items 29: moniker 26, local 0, none 0, unresolved 2, not_loaded 1\n\
joined 29, unmatched 0, c_only 0; agrees 26, disagrees 0; collisions 1; stored fresh, <n> s, peak <n> GiB\n";

/// The top-level keys of `spec --json resolve`, in the spec's order.
pub const JSON_KEYS: [&str; 16] = [
    "protocol",
    "ra_ap",
    "host",
    "workspaces",
    "files",
    "in_crate",
    "items",
    "by_state",
    "joined",
    "unmatched",
    "c_only",
    "agrees",
    "disagrees",
    "collisions",
    "ms",
    "peak_rss",
];

/// The keys of one `workspaces` entry.
pub const WORKSPACE_KEYS: [&str; 6] = [
    "dir",
    "toolchain",
    "sysroot_packages",
    "metadata_degraded",
    "discover_ms",
    "database_ms",
];

/// The five states, `by_state`'s keys in order.
pub const STATES: [&str; 5] = ["moniker", "local", "none", "unresolved", "not_loaded"];

/// The request's six keys.
pub const REQUEST_KEYS: [&str; 6] = [
    "budget_ms",
    "files",
    "protocol",
    "root",
    "target_dir",
    "tmp_dir",
];

static UNIQUE: AtomicUsize = AtomicUsize::new(0);

fn unique() -> usize {
    UNIQUE.fetch_add(1, Ordering::SeqCst)
}

// ------------------------------------------------------------- the fake

/// The fake `specengine-ra` (TE-1); `@DIR@` is its state directory.
pub const FAKE_SCRIPT: &str = r##"#!/usr/bin/perl
# A fake `specengine-ra` of the layer-c-resolve tests
# (docs/features/layer-c-resolve.md TE-1), written at test time: never
# rust-analyzer, never cargo. Its plan is `@DIR@/plan` (key=value lines);
# its logs go to `@DIR@/log`. Its own alarm ends it, and its grandchild's
# ends the grandchild, whatever `spec` does.
use strict;
use warnings;
use Cwd ();
use Time::HiRes ();

my $dir = '@DIR@';
my %plan = (
    alarm => 60,
    read => 'line',
    eof => 'watch',
    hold_ms => 0,
    after => 'exit',
    exit => 0,
    reply => "$dir/reply",
    cut => 0,
    line_delay_ms => 0,
    flood => 0,
    grandchild => 0,
    term => 'default',
    gate => '',
    touch => '',
    stderr => '',
);
if (open(my $in, '<', "$dir/plan")) {
    while (my $line = <$in>) {
        chomp $line;
        my ($key, $value) = split /=/, $line, 2;
        $plan{$key} = $value if defined $value && length $key;
    }
    close $in;
}
alarm($plan{alarm});
$SIG{TERM} = 'IGNORE' if $plan{term} eq 'ignore';
mkdir "$dir/log";

sub note {
    my ($file, $text) = @_;
    open(my $out, '>>', "$dir/log/$file") or return;
    binmode $out;
    print {$out} $text;
    close $out;
}

note('invocations', join("\x1f", $$, getpgrp(), getppid(), Cwd::getcwd(), @ARGV) . "\n");
note('pids', "fake $$\n");
note("env.$$", join('', map { "$_=$ENV{$_}\0" } sort keys %ENV));
print STDERR "$plan{stderr}\n" if length $plan{stderr};

binmode STDIN;
binmode STDOUT;

sub now_ms { return Time::HiRes::time() * 1000; }

# Whether stdin reaches EOF (or fails) within $ms milliseconds; bytes that
# come are read and dropped.
sub stdin_closed_within {
    my ($ms) = @_;
    my $deadline = now_ms() + $ms;
    while (1) {
        my $left = ($deadline - now_ms()) / 1000;
        $left = 0 if $left < 0;
        my $rin = '';
        vec($rin, fileno(STDIN), 1) = 1;
        my $ready = select(my $rout = $rin, undef, undef, $left);
        if (defined $ready && $ready > 0) {
            my $got = sysread(STDIN, my $chunk, 65536);
            return 1 if !defined $got || $got == 0;
            next;
        }
        return 0 if $left <= 0;
    }
}

# What the binary does when its caller is gone: no line, exit 3.
sub gone {
    note('ended', "stdin-eof $$\n");
    exit 3;
}

sub grandchild_noted {
    open(my $in, '<', "$dir/log/pids") or return 0;
    my $found = grep { /^grandchild / } <$in>;
    close $in;
    return $found;
}

my $reads = $plan{read} eq 'line';
my $watch = $reads && $plan{eof} eq 'watch';

if ($reads) {
    my $request = '';
    while (index($request, "\n") < 0) {
        my $got = sysread(STDIN, my $chunk, 65536);
        last if !defined $got || $got == 0;
        $request .= $chunk;
    }
    note("request.$$", $request);
}

if ($plan{grandchild}) {
    my $pid = fork();
    if (defined $pid && $pid == 0) {
        $SIG{TERM} = 'IGNORE';
        alarm(60);
        note('pids', "grandchild $$\n");
        sleep 1 while 1;
    }
    for (1 .. 250) {
        last if grandchild_noted();
        Time::HiRes::sleep(0.02);
    }
}

if ($reads && $plan{hold_ms} > 0) {
    gone() if stdin_closed_within($plan{hold_ms});
}

if (length $plan{gate}) {
    note('started', "$$\n");
    until (-e $plan{gate}) {
        if ($watch) {
            gone() if stdin_closed_within(20);
        } else {
            Time::HiRes::sleep(0.02);
        }
    }
}

my @lines;
if (open(my $in, '<', $plan{reply})) {
    binmode $in;
    @lines = <$in>;
    close $in;
}
@lines = @lines[0 .. $plan{cut} - 1] if $plan{cut} > 0 && $plan{cut} < @lines;

for my $at (0 .. $#lines) {
    gone() if $watch && stdin_closed_within(0);
    if (length $plan{touch} && $at == $#lines) {
        open(my $out, '>>', $plan{touch}) or exit 97;
        print {$out} "// touched during the run\n";
        close $out;
        note('touched', "$$\n");
    }
    my $line = $lines[$at];
    my $off = 0;
    while ($off < length $line) {
        my $wrote = syswrite(STDOUT, $line, length($line) - $off, $off);
        exit 4 unless defined $wrote;
        $off += $wrote;
    }
    Time::HiRes::sleep($plan{line_delay_ms} / 1000) if $plan{line_delay_ms} > 0;
}

if ($plan{flood} > 0) {
    my $block = 'x' x 65536;
    my $left = $plan{flood};
    while ($left > 0) {
        my $wrote = syswrite(STDOUT, $block, $left < 65536 ? $left : 65536);
        exit 4 unless defined $wrote;
        $left -= $wrote;
    }
}

note('wrote', scalar(@lines) . " $$\n");
if ($plan{after} eq 'eof') {
    # The binary's guard after its last line: it waits for its caller to
    # close stdin (at most 30 s here), then exits with that line's code.
    note('waited', (stdin_closed_within(30000) ? 'eof' : 'timeout') . " $$\n");
}
if ($plan{after} eq 'hang') {
    sleep 1 while 1;
}
if ($plan{after} eq 'kill') {
    kill 'KILL', $$;
    sleep 1 while 1;
}
exit $plan{exit};
"##;

/// One start of a fake, as it logged it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub pid: i32,
    pub pgid: i32,
    pub ppid: i32,
    pub cwd: String,
    pub args: Vec<String>,
}

/// A fake `specengine-ra`: the executable and its state directory (plan,
/// reply, logs). Its drop kills what it left running.
pub struct Fake {
    /// The plan, the reply and `log/`.
    pub dir: PathBuf,
    /// The executable, named `specengine-ra`.
    pub program: PathBuf,
}

/// `bytes` at `path`, executable.
pub fn write_executable(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the executable's directory");
    }
    fs::write(path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod {}: {error}", path.display()));
}

impl Fake {
    /// The fake at `program`, its state under `dir`; an empty plan (read
    /// the request, watch stdin, exit 0) and the golden as its reply.
    pub fn write(program: &Path, dir: &Path) -> Fake {
        fs::create_dir_all(dir).expect("the fake's directory");
        let dir_text = dir.to_str().expect("a UTF-8 scratch path");
        assert!(
            !dir_text.contains('\''),
            "{dir_text}: a quote in a scratch path"
        );
        write_executable(program, FAKE_SCRIPT.replace("@DIR@", dir_text).as_bytes());
        let fake = Fake {
            dir: dir.to_path_buf(),
            program: program.to_path_buf(),
        };
        fake.plan(&[]);
        fake.reply(&golden_text());
        fake
    }

    /// Rewrites the script itself (after a test replaced it).
    pub fn restore(&self) {
        let dir_text = self.dir.to_str().expect("a UTF-8 scratch path");
        write_executable(
            &self.program,
            FAKE_SCRIPT.replace("@DIR@", dir_text).as_bytes(),
        );
    }

    /// The plan, replacing the last one; values as the script reads them.
    pub fn plan(&self, entries: &[(&str, &str)]) {
        let mut text = String::new();
        for (key, value) in entries {
            assert!(
                !key.contains('=') && !value.contains('\n'),
                "a plan entry: {key}={value}"
            );
            text.push_str(key);
            text.push('=');
            text.push_str(value);
            text.push('\n');
        }
        fs::write(self.dir.join("plan"), text).expect("the plan");
    }

    /// The reply the fake writes, line by line.
    pub fn reply(&self, text: &str) {
        fs::write(self.dir.join("reply"), text).expect("the reply");
    }

    /// The plan and the reply at once.
    pub fn script(&self, entries: &[(&str, &str)], reply: &str) {
        self.plan(entries);
        self.reply(reply);
    }

    fn log(&self, name: &str) -> Option<Vec<u8>> {
        fs::read(self.dir.join("log").join(name)).ok()
    }

    /// Every start, in order.
    pub fn invocations(&self) -> Vec<Invocation> {
        let Some(bytes) = self.log("invocations") else {
            return Vec::new();
        };
        String::from_utf8_lossy(&bytes)
            .lines()
            .map(|line| {
                let fields: Vec<&str> = line.split('\u{1f}').collect();
                assert!(fields.len() >= 4, "an invocation record: {line:?}");
                let number = |at: usize| -> i32 {
                    fields[at]
                        .parse()
                        .unwrap_or_else(|_| panic!("a number in {line:?}"))
                };
                Invocation {
                    pid: number(0),
                    pgid: number(1),
                    ppid: number(2),
                    cwd: fields[3].to_owned(),
                    args: fields[4..].iter().map(|arg| (*arg).to_owned()).collect(),
                }
            })
            .collect()
    }

    /// The one start of this run: exactly one invocation logged.
    pub fn only(&self) -> Invocation {
        let all = self.invocations();
        assert_eq!(all.len(), 1, "one start of the fake: {all:#?}");
        all[0].clone()
    }

    /// The request bytes `pid` read (the line with its end), if it read.
    pub fn request_of(&self, pid: i32) -> Option<String> {
        self.log(&format!("request.{pid}"))
            .map(|bytes| String::from_utf8(bytes).expect("a UTF-8 request"))
    }

    /// The environment `pid` started with.
    pub fn env_of(&self, pid: i32) -> BTreeMap<String, String> {
        let bytes = self
            .log(&format!("env.{pid}"))
            .unwrap_or_else(|| panic!("no environment logged by {pid}"));
        String::from_utf8_lossy(&bytes)
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let (key, value) = entry.split_once('=').expect("KEY=VALUE");
                (key.to_owned(), value.to_owned())
            })
            .collect()
    }

    /// `(role, pid)` of every process the fake noted: `fake`, `grandchild`.
    pub fn pids(&self) -> Vec<(String, i32)> {
        let bytes = self.log("pids").unwrap_or_default();
        String::from_utf8_lossy(&bytes)
            .lines()
            .filter_map(|line| {
                let (role, pid) = line.split_once(' ')?;
                Some((role.to_owned(), pid.trim().parse().ok()?))
            })
            .collect()
    }

    pub fn grandchildren(&self) -> Vec<i32> {
        self.pids()
            .into_iter()
            .filter(|(role, _)| role == "grandchild")
            .map(|(_, pid)| pid)
            .collect()
    }

    /// Whether a start saw stdin's EOF and ended as the binary does.
    pub fn ended_on_eof(&self) -> bool {
        self.log("ended").is_some()
    }

    /// Whether a start reached its gate.
    pub fn started(&self) -> bool {
        self.log("started").is_some()
    }

    /// Whether a start edited the tree before its last line.
    pub fn touched(&self) -> bool {
        self.log("touched").is_some()
    }

    /// Whether a start wrote its reply lines (and went on to its ending).
    pub fn wrote(&self) -> bool {
        self.log("wrote").is_some()
    }

    /// What a start with `after=eof` saw after its last line: `eof` (its
    /// caller closed stdin) or `timeout` (30 s, stdin still open).
    pub fn waited(&self) -> Option<String> {
        self.log("waited").map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .split(' ')
                .next()
                .unwrap_or_default()
                .to_owned()
        })
    }

    /// Kills what is left, then forgets every log.
    pub fn clear(&self) {
        self.reap();
        let _ = fs::remove_dir_all(self.dir.join("log"));
    }

    /// Every noted process still running this fake's script.
    pub fn running(&self) -> Vec<(String, i32)> {
        self.pids()
            .into_iter()
            .filter(|(_, pid)| runs(*pid, &self.program))
            .collect()
    }

    /// Polls until no noted process runs the script, or `within`: what
    /// still runs then.
    pub fn survivors(&self, within: Duration) -> Vec<(String, i32)> {
        let deadline = Instant::now() + within;
        loop {
            let left = self.running();
            if left.is_empty() || Instant::now() >= deadline {
                return left;
            }
            thread::sleep(POLL);
        }
    }

    /// SIGKILL, by pid, to every noted process still running the script.
    pub fn reap(&self) {
        for (_, pid) in self.running() {
            // SAFETY: no pointers; `pid` was just seen running this fake's
            // script (`runs`), never the test's own process or group.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.reap();
    }
}

/// Whether `pid` is a live process whose command line names `program` (a
/// zombie's does not; a reused pid's does not).
pub fn runs(pid: i32, program: &Path) -> bool {
    if pid <= 1 {
        return false;
    }
    let Ok(output) = Command::new("/bin/ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return false;
    };
    let needle = program.to_string_lossy();
    String::from_utf8_lossy(&output.stdout).contains(needle.as_ref())
}

// --------------------------------------------------------- running spec

/// A running copy of `spec`; killed on drop if still running.
pub struct Started {
    child: Child,
    pub pid: i32,
    out: mpsc::Receiver<Vec<u8>>,
    err: mpsc::Receiver<Vec<u8>>,
    pub at: Instant,
    args: Vec<String>,
}

/// A finished copy of `spec`.
#[derive(Debug, Clone)]
pub struct Ended {
    pub args: Vec<String>,
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
}

fn drain(mut pipe: impl std::io::Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    receiver
}

fn take(pipe: &mpsc::Receiver<Vec<u8>>, name: &str) -> Vec<u8> {
    pipe.recv_timeout(PIPE_BOUND).unwrap_or_else(|_| {
        format!("<{name} still open {PIPE_BOUND:?} after spec exited: a descendant holds it>")
            .into_bytes()
    })
}

impl Started {
    /// `signal` to `spec` alone.
    pub fn signal(&self, signal: libc::c_int) {
        // SAFETY: no pointers; the pid is this test's own child, not yet
        // reaped (only `wait` reaps it).
        unsafe { libc::kill(self.pid, signal) };
    }

    /// Waits at most `limit` after the start; past it `spec` is killed and
    /// the test fails.
    pub fn wait(mut self, limit: Duration) -> Ended {
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("try_wait on spec") {
                break status;
            }
            if self.at.elapsed() > limit {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("spec {:?} ran longer than {limit:?}; killed", self.args);
            }
            thread::sleep(POLL);
        };
        let elapsed = self.at.elapsed();
        let stdout = take(&self.out, "stdout");
        let stderr = take(&self.err, "stderr");
        Ended {
            args: self.args.clone(),
            status,
            stdout: String::from_utf8(stdout).expect("UTF-8 stdout"),
            stderr: String::from_utf8(stderr).expect("UTF-8 stderr"),
            elapsed,
        }
    }
}

impl Drop for Started {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl Ended {
    /// The run as `common::Run`; `spec` must have exited, not died.
    pub fn run(&self) -> Run {
        let code = self.status.code().unwrap_or_else(|| {
            panic!(
                "spec {:?} died of signal {:?}, it did not exit:\n{}",
                self.args,
                self.status.signal(),
                self.show()
            )
        });
        Run {
            args: self.args.clone(),
            code,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        }
    }

    pub fn show(&self) -> String {
        format!(
            "{:?} after {:?}\n--- stdout\n{}--- stderr\n{}",
            self.status, self.elapsed, self.stdout, self.stderr
        )
    }
}

// ------------------------------------------------------------------ lab

/// A project, a `HOME`, and a copy of `spec` with a fake beside it.
pub struct Lab {
    /// The fake beside the copy (dropped first: it reaps).
    pub fake: Fake,
    /// The project root, canonical.
    pub root: PathBuf,
    /// The data directory's `HOME`.
    pub home: PathBuf,
    /// Where the lab's own files live (outside the root).
    pub base: PathBuf,
    /// The directory of the copy and the fake.
    pub bin: PathBuf,
    /// The copy of `spec`.
    pub spec: PathBuf,
    pub slug: String,
    /// `PATH`, entry by entry.
    pub path: Vec<PathBuf>,
    /// Further variables of every run (no `HOME`, no `PATH`).
    pub vars: Vec<(OsString, OsString)>,
    _scratch: Option<Scratch>,
}

impl Lab {
    /// A scratch ra-mini with [`CONFIG`], a scratch `HOME`, the fake
    /// replaying the golden.
    pub fn ra_mini(label: &str) -> Lab {
        let scratch = Scratch::new(label);
        let root = scratch.copy("ra-mini", "ra-mini");
        write(&root, "specengine.toml", CONFIG);
        let home = scratch.home("h");
        let base = scratch.path().to_path_buf();
        Lab::assemble(
            Some(scratch),
            base,
            root,
            home,
            SLUG,
            Vec::new(),
            Vec::new(),
        )
    }

    /// As [`Lab::ra_mini`], every entry of the copy created in reverse name
    /// order (R9, TE-4).
    pub fn ra_mini_reversed(label: &str) -> Lab {
        let scratch = Scratch::new(label);
        let root = scratch.path().join("ra-mini");
        copy_reversed(&repository_root().join("fixtures").join("ra-mini"), &root);
        let root = fs::canonicalize(root).expect("the reversed copy");
        write(&root, "specengine.toml", CONFIG);
        let home = scratch.home("h");
        let base = scratch.path().to_path_buf();
        Lab::assemble(
            Some(scratch),
            base,
            root,
            home,
            SLUG,
            Vec::new(),
            Vec::new(),
        )
    }

    /// A scratch copy of `fixtures/<fixture>` (its `expected.json` left
    /// out) with its own config and slug.
    pub fn fixture(label: &str, fixture: &str, slug: &str) -> Lab {
        let scratch = Scratch::new(label);
        let root = scratch.copy(fixture, fixture);
        let _ = fs::remove_file(root.join("expected.json"));
        let home = scratch.home("h");
        let base = scratch.path().to_path_buf();
        Lab::assemble(
            Some(scratch),
            base,
            root,
            home,
            slug,
            Vec::new(),
            Vec::new(),
        )
    }

    /// Over a project the caller made (a `LockRepo`'s): the lab's files
    /// under `base`, the caller's root, `HOME` and variables (any `HOME` or
    /// `PATH` among them dropped), `path` the `PATH` entries after the
    /// lab's own empty one.
    pub fn over(
        base: &Path,
        root: &Path,
        home: &Path,
        slug: &str,
        vars: Vec<(OsString, OsString)>,
        path: Vec<PathBuf>,
    ) -> Lab {
        fs::create_dir_all(base).expect("the lab's directory");
        let vars = vars
            .into_iter()
            .filter(|(name, _)| name != "HOME" && name != "PATH")
            .collect();
        Lab::assemble(
            None,
            base.to_path_buf(),
            root.to_path_buf(),
            home.to_path_buf(),
            slug,
            vars,
            path,
        )
    }

    fn assemble(
        scratch: Option<Scratch>,
        base: PathBuf,
        root: PathBuf,
        home: PathBuf,
        slug: &str,
        vars: Vec<(OsString, OsString)>,
        extra_path: Vec<PathBuf>,
    ) -> Lab {
        let bin = base.join("bin");
        fs::create_dir_all(&bin).expect("the lab's bin");
        let spec = bin.join("spec");
        // A copy, never a hard link: the scratch's drop resets every file's
        // mode to 0644, which a link would carry over to the build's binary.
        copy_file(SPEC, &spec);
        let fake = Fake::write(&bin.join(RA_BIN), &base.join("fake"));
        let empty = base.join("path-empty");
        fs::create_dir_all(&empty).expect("an empty PATH entry");
        let mut path = vec![empty];
        path.extend(extra_path);
        for dir in ["/usr/bin", "/bin"] {
            let dir = PathBuf::from(dir);
            if !path.contains(&dir) {
                path.push(dir);
            }
        }
        Lab {
            fake,
            root,
            home,
            base,
            bin,
            spec,
            slug: slug.to_owned(),
            path,
            vars,
            _scratch: scratch,
        }
    }

    /// A second fake at `<base>/<name>/specengine-ra` (its state in
    /// `<base>/<name>-state`), for a `PATH` entry.
    pub fn extra_fake(&self, name: &str) -> Fake {
        Fake::write(
            &self.base.join(name).join(RA_BIN),
            &self.base.join(format!("{name}-state")),
        )
    }

    /// Every variable of a run with `HOME=home`.
    pub fn env_for(&self, home: &Path) -> Vec<(OsString, OsString)> {
        let mut vars: Vec<(OsString, OsString)> = self
            .vars
            .iter()
            .filter(|(name, _)| name != "HOME" && name != "PATH" && name != SENTINEL.0)
            .cloned()
            .collect();
        vars.push(("HOME".into(), home.as_os_str().to_owned()));
        vars.push((
            "PATH".into(),
            std::env::join_paths(&self.path).expect("a PATH value"),
        ));
        vars.push((SENTINEL.0.into(), SENTINEL.1.into()));
        vars
    }

    pub fn env(&self) -> Vec<(OsString, OsString)> {
        self.env_for(&self.home)
    }

    /// The copy of `spec args` in `cwd` with `HOME=home`, started.
    pub fn start_in(&self, cwd: &Path, home: &Path, args: &[&str]) -> Started {
        let mut command = Command::new(&self.spec);
        command
            .env_clear()
            .envs(self.env_for(home))
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let at = Instant::now();
        let mut child = command.spawn().expect("spawn the copy of spec");
        let out = drain(child.stdout.take().expect("stdout"));
        let err = drain(child.stderr.take().expect("stderr"));
        let pid = i32::try_from(child.id()).expect("a pid fits pid_t");
        Started {
            child,
            pid,
            out,
            err,
            at,
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        }
    }

    /// The copy of `spec args` in the root, started.
    pub fn start(&self, args: &[&str]) -> Started {
        self.start_in(&self.root, &self.home, args)
    }

    /// The copy of `spec args` in the root, finished.
    pub fn ended(&self, args: &[&str]) -> Ended {
        self.start(args).wait(RUN_LIMIT)
    }

    /// The copy of `spec args` in the root; it must exit.
    pub fn run(&self, args: &[&str]) -> Run {
        self.ended(args).run()
    }

    pub fn resolve(&self) -> Run {
        self.run(&["resolve"])
    }

    pub fn resolve_json(&self) -> Run {
        self.run(&["--json", "resolve"])
    }

    /// `spec index`, which must answer.
    pub fn index(&self) {
        self.run(&["index"]).code(0);
    }

    pub fn db(&self) -> PathBuf {
        db(&self.home, &self.slug)
    }

    /// `<data dir>/<slug>.ra`, as `HOME` spells it.
    pub fn ra_dir(&self) -> PathBuf {
        data_dir(&self.home).join(format!("{}.ra", self.slug))
    }

    /// The layer C dump: the snapshot tables' lines of the store's dump.
    pub fn layer_c(&self) -> String {
        layer_c_lines(&dump(&self.home, &self.root, &self.slug))
    }

    /// The whole dump, the root masked.
    pub fn dump(&self) -> String {
        dump(&self.home, &self.root, &self.slug)
    }

    /// The golden replayed (plan reset): exit 0, `stored fresh`, a stored
    /// snapshot; its layer C dump.
    pub fn store_golden(&self) -> String {
        self.store_reply(&golden_text())
    }

    /// `reply` replayed (plan reset): exit 0, `stored fresh`, a stored
    /// snapshot; its layer C dump.
    pub fn store_reply(&self, reply: &str) -> String {
        self.fake.clear();
        self.fake.script(&[], reply);
        let run = self.resolve();
        run.code(0);
        assert!(
            run.stdout.contains("stored fresh"),
            "the snapshot is stored:\n{}",
            run.show()
        );
        let stored = self.layer_c();
        assert!(
            stored
                .lines()
                .any(|line| line.starts_with("code_resolve\t"))
                && stored
                    .lines()
                    .any(|line| line.starts_with("code_monikers\t")),
            "the layer C dump holds the snapshot:\n{stored}"
        );
        stored
    }

    /// The `.rs` files a request names: the walked ones whose content is
    /// UTF-8, byte-sorted, as the code index stored them (after an index).
    pub fn requested_files(&self) -> Vec<String> {
        query(
            &self.db(),
            "SELECT path FROM code_files WHERE path LIKE '%.rs' AND utf8 = 1 \
             AND read_error IS NULL ORDER BY path",
        )
        .into_iter()
        .map(|row| row["path"].as_str().expect("a path").to_owned())
        .collect()
    }

    /// The copy of `spec args` on a pseudo-terminal (`script`), stdout and
    /// stderr into files of their own; at the first `[y/N]` the terminal
    /// answers `answer`. Bounded by [`RUN_LIMIT`].
    pub fn pty(&self, cwd: &Path, args: &[&str], answer: &str) -> Run {
        let n = unique();
        let out = self.base.join(format!("pty-{n}.out"));
        let err = self.base.join(format!("pty-{n}.err"));
        let script = ["/usr/bin/script", "/bin/script"]
            .into_iter()
            .find(|path| Path::new(path).exists())
            .expect("a `script` program gives `spec` a terminal");
        let program = self.spec.to_str().expect("a UTF-8 path");
        let inner: Vec<&str> = [
            "/bin/sh",
            "-c",
            "exec \"$0\" \"$@\" >\"$LCR_PTY_OUT\" 2>\"$LCR_PTY_ERR\"",
            program,
        ]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
        let mut command = Command::new(script);
        if cfg!(target_os = "macos") {
            command.arg("-q").arg("/dev/null").args(&inner);
        } else {
            let quoted: Vec<String> = inner
                .iter()
                .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
                .collect();
            command.arg("-qec").arg(quoted.join(" ")).arg("/dev/null");
        }
        command
            .env_clear()
            .envs(self.env())
            .env("LCR_PTY_OUT", &out)
            .env("LCR_PTY_ERR", &err)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("spawn script");
        let mut stdin = child.stdin.take().expect("stdin");
        let echo = drain(child.stdout.take().expect("stdout"));
        let mut asked = false;
        let started = Instant::now();
        let status = loop {
            if !asked
                && fs::read(&err)
                    .map(|bytes| {
                        String::from_utf8_lossy(&bytes)
                            .trim_end()
                            .ends_with("? [y/N]")
                    })
                    .unwrap_or(false)
            {
                asked = true;
                let _ = stdin.write_all(format!("{answer}\n").as_bytes());
                let _ = stdin.flush();
            }
            if let Some(status) = child.try_wait().expect("wait on script") {
                break status;
            }
            if started.elapsed() > RUN_LIMIT {
                let _ = child.kill();
                let _ = child.wait();
                panic!("spec {args:?} on a terminal ran longer than {RUN_LIMIT:?}; killed");
            }
            thread::sleep(POLL);
        };
        drop(stdin);
        let _ = echo.recv_timeout(PIPE_BOUND);
        let stdout = fs::read(&out).unwrap_or_default();
        let stderr = fs::read(&err).unwrap_or_default();
        let _ = fs::remove_file(out);
        let _ = fs::remove_file(err);
        Run {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status
                .code()
                .expect("script exited, not killed by a signal"),
            stdout: String::from_utf8(stdout).expect("UTF-8 stdout"),
            stderr: String::from_utf8(stderr).expect("UTF-8 stderr"),
        }
    }
}

/// Copies `from` to `to` creating every entry in reverse name order.
pub fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("type");
        if kind.is_dir() {
            copy_reversed(&entry.path(), &target);
        } else if kind.is_file() {
            fs::write(target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}

// ---------------------------------------------------------------- texts

/// The golden's bytes.
pub fn golden_text() -> String {
    read_text(&repository_root(), GOLDEN)
}

/// The golden's lines, without their ends.
pub fn golden_lines() -> Vec<String> {
    golden_text().lines().map(str::to_owned).collect()
}

/// `lines` as a reply: each ended by `\n`.
pub fn reply_of(lines: &[String]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// `line` with its one `from` replaced by `to`.
pub fn swap_in(line: &str, from: &str, to: &str) -> String {
    assert_eq!(
        line.matches(from).count(),
        1,
        "{from:?} occurs once in {line:?}"
    );
    line.replacen(from, to, 1)
}

/// Every number right before ` ms`, ` s` or ` GiB` replaced by `<n>`: the
/// timings of `resolve`'s text (L3), never compared.
pub fn mask_timing(text: &str) -> String {
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let (body, end) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let words: Vec<&str> = body.split(' ').collect();
        let masked: Vec<&str> = words
            .iter()
            .enumerate()
            .map(|(at, word)| {
                let numeric =
                    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit() || c == '.');
                let unit = words
                    .get(at + 1)
                    .map(|next| next.trim_end_matches([',', ';', ')']));
                if numeric && matches!(unit, Some("ms" | "s" | "GiB")) {
                    "<n>"
                } else {
                    *word
                }
            })
            .collect();
        out.push_str(&masked.join(" "));
        out.push_str(end);
    }
    out
}

/// The whole exit-2 line of a run: `spec resolve: <reason>; the previous
/// snapshot is kept`.
pub fn refusal(reason: &str) -> String {
    format!("spec resolve: {reason}{KEPT}")
}

/// The last line of `run`'s stderr.
pub fn last_line(run: &Run) -> &str {
    run.stderr.lines().last().unwrap_or_default()
}

/// Exit 2, stdout empty, stderr ending with [`refusal`] of `reason`.
pub fn assert_refused(run: &Run, reason: &str) {
    refused_as("spec resolve", run, reason);
}

/// [`assert_refused`], the failure named `name`.
pub fn refused_as(name: &str, run: &Run, reason: &str) {
    assert_eq!(run.code, 2, "{name}: exit 2:\n{}", run.show());
    assert_eq!(
        run.stdout,
        "",
        "{name}: stdout stays empty on exit 2:\n{}",
        run.show()
    );
    assert_eq!(last_line(run), refusal(reason), "{name}:\n{}", run.show());
}

/// Polls `done` until it holds; past `within` the test fails naming `what`.
pub fn wait_until(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + within;
    while !done() {
        assert!(Instant::now() < deadline, "{what}: not within {within:?}");
        thread::sleep(POLL);
    }
}

/// Exit 2 before the spawn (`no Rust file to resolve`, the lock, the
/// install line): the same frame as every other reason ([`assert_refused`];
/// the developer's iteration 1 notes, "Reason texts").
pub fn assert_refused_before_spawn(run: &Run, reason: &str) {
    refused_as("before the spawn", run, reason);
}

/// Exit 2 for a binary killed by SIGKILL: `specengine-ra ended (signal
/// <s>) before done` (or `after done`), `<s>` its number or its name.
pub fn assert_signal_ending(run: &Run, after_done: bool) {
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    let when = if after_done {
        "after done"
    } else {
        "before done"
    };
    let accepted: Vec<String> = ["9", "SIGKILL", "KILL"]
        .iter()
        .map(|signal| refusal(&format!("specengine-ra ended (signal {signal}) {when}")))
        .collect();
    let last = last_line(run).to_owned();
    assert!(
        accepted.contains(&last),
        "want one of {accepted:#?}:\n{}",
        run.show()
    );
}

/// `run`'s JSON document's counts: `joined`, `unmatched`, `c_only`,
/// `agrees`, `disagrees`, `collisions`.
pub fn counts(run: &Run) -> BTreeMap<String, u64> {
    let json = run.json();
    [
        "joined",
        "unmatched",
        "c_only",
        "agrees",
        "disagrees",
        "collisions",
        "files",
        "in_crate",
        "items",
    ]
    .iter()
    .map(|key| {
        (
            (*key).to_owned(),
            json[*key]
                .as_u64()
                .unwrap_or_else(|| panic!("{key} is a count: {json}")),
        )
    })
    .collect()
}

/// The top-level keys of the JSON object `text`, in the order written.
pub fn top_keys(text: &str) -> Vec<String> {
    use serde::de::{Deserializer as _, IgnoredAny, MapAccess, Visitor};
    struct Keys;
    impl<'de> Visitor<'de> for Keys {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a JSON object")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Vec<String>, M::Error> {
            let mut keys = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                let _: IgnoredAny = map.next_value()?;
                keys.push(key);
            }
            Ok(keys)
        }
    }
    let mut deserializer = serde_json::Deserializer::from_str(text.trim_end());
    (&mut deserializer)
        .deserialize_map(Keys)
        .unwrap_or_else(|error| panic!("a JSON object: {error}\n{text}"))
}

/// The keys of a JSON object value, sorted.
pub fn keys_of(value: &Value) -> Vec<String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("an object: {value}"))
        .keys()
        .cloned()
        .collect()
}

/// `names`, sorted, as owned strings.
pub fn sorted(names: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    out.sort();
    out
}

/// The byte offset in a file's CRLF form of `offset` in its LF form `lf`:
/// one more byte per line end before it.
pub fn crlf_offset(lf: &str, offset: u64) -> u64 {
    let at = usize::try_from(offset).expect("an offset");
    let ends = lf.as_bytes()[..at]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    offset + u64::try_from(ends).expect("a count")
}

// ---------------------------------------------------------------- reply

/// One item of a `file` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub start: u64,
    pub end: u64,
    pub kind: String,
    pub state: String,
    pub moniker: Option<String>,
}

/// One `file` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLine {
    pub path: String,
    pub in_crate: bool,
    pub items: Vec<Item>,
}

/// One workspace: its `discover` and `database` steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub workspace: u64,
    pub dir: String,
    pub toolchain: Option<String>,
    pub sysroot_packages: u64,
    pub degraded: bool,
}

/// How a reply ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// The `file` lines, then `done` with the tallies.
    Done,
    /// `error` after the steps, no `file` line.
    Error { workspace: u64, category: String },
}

/// A protocol v1 reply, rendered with `t` first and every key in the
/// canon's order, the `done` tallies computed from the `file` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub protocol: u64,
    pub ra_ap: String,
    pub host: String,
    pub steps: Vec<Step>,
    pub files: Vec<FileLine>,
    pub peak_rss: Option<u64>,
    pub ms: u64,
    pub end: End,
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("a JSON string")
}

fn json_option(text: Option<&str>) -> String {
    text.map_or_else(|| "null".to_owned(), json_string)
}

impl Step {
    /// One workspace at `dir`, stamps masked as the golden's.
    pub fn at(workspace: u64, dir: &str) -> Step {
        Step {
            workspace,
            dir: dir.to_owned(),
            toolchain: Some("masked".to_owned()),
            sysroot_packages: 0,
            degraded: false,
        }
    }
}

impl Reply {
    /// The golden, read into its parts.
    pub fn golden() -> Reply {
        let lines: Vec<Value> = golden_lines()
            .iter()
            .map(|line| serde_json::from_str(line).expect("a golden line is JSON"))
            .collect();
        let hello = &lines[0];
        assert_eq!(hello["t"], "hello", "the golden starts with hello");
        let mut steps = Vec::new();
        let mut files = Vec::new();
        let mut peak_rss = None;
        let mut ms = 0;
        for line in &lines[1..] {
            match line["t"].as_str() {
                Some("step") if line["step"] == "discover" => steps.push(Step {
                    workspace: line["workspace"].as_u64().expect("workspace"),
                    dir: line["dir"].as_str().expect("dir").to_owned(),
                    toolchain: line["toolchain"].as_str().map(str::to_owned),
                    sysroot_packages: line["sysroot_packages"].as_u64().expect("sysroot"),
                    degraded: line["metadata_degraded"].as_bool().expect("degraded"),
                }),
                Some("step") => {}
                Some("file") => files.push(FileLine {
                    path: line["path"].as_str().expect("path").to_owned(),
                    in_crate: line["in_crate"].as_bool().expect("in_crate"),
                    items: line["items"]
                        .as_array()
                        .expect("items")
                        .iter()
                        .map(|item| Item {
                            start: item["start"].as_u64().expect("start"),
                            end: item["end"].as_u64().expect("end"),
                            kind: item["kind"].as_str().expect("kind").to_owned(),
                            state: item["state"].as_str().expect("state").to_owned(),
                            moniker: item["moniker"].as_str().map(str::to_owned),
                        })
                        .collect(),
                }),
                Some("done") => {
                    peak_rss = line["peak_rss"].as_u64();
                    ms = line["ms"].as_u64().expect("ms");
                }
                other => panic!("a golden line of type {other:?}"),
            }
        }
        Reply {
            protocol: hello["protocol"].as_u64().expect("protocol"),
            ra_ap: hello["ra_ap"].as_str().expect("ra_ap").to_owned(),
            host: hello["host"].as_str().expect("host").to_owned(),
            steps,
            files,
            peak_rss,
            ms,
            end: End::Done,
        }
    }

    /// A reply to `files` whose items are the code index's own rows of
    /// each (`symbols`: their bytes, their kind mapped through the kind
    /// map), each a `moniker` row with a moniker of its own: every row of
    /// layer A joins one row of C. Read after an index of the same tree.
    pub fn from_index(db: &Path, files: &[String]) -> Reply {
        let mut lines = Vec::new();
        for path in files {
            assert!(!path.contains('\''), "{path}: a quote in a path");
            let rows = query(
                db,
                &format!(
                    "SELECT s.start_byte AS start, s.end_byte AS \"end\", s.item_kind AS kind, \
                     s.owner AS owner FROM symbols s JOIN code_files f ON f.file_id = s.file \
                     WHERE f.path = '{path}' ORDER BY s.start_byte, s.end_byte"
                ),
            );
            let mut items: Vec<Item> = Vec::new();
            for row in rows {
                let start = row["start"].as_u64().expect("start_byte");
                if items.last().is_some_and(|last| last.start >= start) {
                    continue;
                }
                let kind = row["kind"].as_str().expect("item_kind");
                let Some(layer_c) = layer_c_kind(kind, !row["owner"].is_null()) else {
                    continue;
                };
                items.push(Item {
                    start,
                    end: row["end"].as_u64().expect("end_byte"),
                    kind: layer_c.to_owned(),
                    state: "moniker".to_owned(),
                    moniker: Some(format!("{GENERATED} m{start}.")),
                });
            }
            lines.push(FileLine {
                path: path.clone(),
                in_crate: true,
                items,
            });
        }
        Reply {
            protocol: 1,
            ra_ap: "0.0.352".to_owned(),
            host: "masked".to_owned(),
            steps: vec![Step::at(1, ".")],
            files: lines,
            peak_rss: Some(0),
            ms: 0,
            end: End::Done,
        }
    }

    /// The `file` line of `path`.
    pub fn file_mut(&mut self, path: &str) -> &mut FileLine {
        self.files
            .iter_mut()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("no file line of {path}"))
    }

    /// The item of `path` starting at `start`.
    pub fn item_mut(&mut self, path: &str, start: u64) -> &mut Item {
        self.file_mut(path)
            .items
            .iter_mut()
            .find(|item| item.start == start)
            .unwrap_or_else(|| panic!("no item of {path} at {start}"))
    }

    /// Items in all `file` lines.
    pub fn item_count(&self) -> usize {
        self.files.iter().map(|file| file.items.len()).sum()
    }

    pub fn hello_line(&self) -> String {
        format!(
            "{{\"t\":\"hello\",\"protocol\":{},\"ra_ap\":{},\"host\":{}}}",
            self.protocol,
            json_string(&self.ra_ap),
            json_string(&self.host)
        )
    }

    fn step_lines(step: &Step) -> [String; 2] {
        [
            format!(
                "{{\"t\":\"step\",\"workspace\":{},\"dir\":{},\"step\":\"discover\",\"ms\":0,\"toolchain\":{},\"sysroot_packages\":{},\"metadata_degraded\":{}}}",
                step.workspace,
                json_string(&step.dir),
                json_option(step.toolchain.as_deref()),
                step.sysroot_packages,
                step.degraded
            ),
            format!(
                "{{\"t\":\"step\",\"workspace\":{},\"dir\":{},\"step\":\"database\",\"ms\":0}}",
                step.workspace,
                json_string(&step.dir)
            ),
        ]
    }

    pub fn file_line(file: &FileLine) -> String {
        let items: Vec<String> = file
            .items
            .iter()
            .map(|item| {
                format!(
                    "{{\"start\":{},\"end\":{},\"kind\":{},\"state\":{},\"moniker\":{}}}",
                    item.start,
                    item.end,
                    json_string(&item.kind),
                    json_string(&item.state),
                    json_option(item.moniker.as_deref())
                )
            })
            .collect();
        format!(
            "{{\"t\":\"file\",\"path\":{},\"in_crate\":{},\"items\":[{}]}}",
            json_string(&file.path),
            file.in_crate,
            items.join(",")
        )
    }

    pub fn done_line(&self) -> String {
        let mut by_state = Vec::new();
        for state in STATES {
            let count = self
                .files
                .iter()
                .flat_map(|file| &file.items)
                .filter(|item| item.state == state)
                .count();
            by_state.push(format!("\"{state}\":{count}"));
        }
        format!(
            "{{\"t\":\"done\",\"files\":{},\"items\":{},\"by_state\":{{{}}},\"peak_rss\":{},\"ms\":{}}}",
            self.files.len(),
            self.item_count(),
            by_state.join(","),
            self.peak_rss
                .map_or_else(|| "null".to_owned(), |peak| peak.to_string()),
            self.ms
        )
    }

    /// Every line, without its end.
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![self.hello_line()];
        for step in &self.steps {
            out.extend(Reply::step_lines(step));
        }
        match &self.end {
            End::Done => {
                out.extend(self.files.iter().map(Reply::file_line));
                out.push(self.done_line());
            }
            End::Error {
                workspace,
                category,
            } => out.push(format!(
                "{{\"t\":\"error\",\"workspace\":{workspace},\"category\":{}}}",
                json_string(category)
            )),
        }
        out
    }

    /// The reply's bytes.
    pub fn render(&self) -> String {
        reply_of(&self.lines())
    }
}

/// The layer C kind of a layer A row: `function_item` is `method` inside
/// an `impl` or `trait` (an owner), `function` outside; every other kind
/// by the kind map; `None` for a kind the map does not hold.
pub fn layer_c_kind(item_kind: &str, member: bool) -> Option<&'static str> {
    if item_kind == "function_item" {
        return Some(if member { "method" } else { "function" });
    }
    KIND_MAP
        .iter()
        .find(|(_, kinds)| kinds.contains(&item_kind))
        .map(|(kind, _)| *kind)
}

/// `{"t":"error",…}` of `workspace` and `category`.
pub fn error_line(workspace: u64, category: &str) -> String {
    format!(
        "{{\"t\":\"error\",\"workspace\":{workspace},\"category\":{}}}",
        json_string(category)
    )
}

/// `{"t":"refused",…}` of `protocol` and `reason`.
pub fn refused_line(protocol: u64, reason: &str) -> String {
    format!(
        "{{\"t\":\"refused\",\"protocol\":{protocol},\"reason\":{}}}",
        json_string(reason)
    )
}

/// The first `n` lines of the golden, as a reply.
pub fn golden_head(n: usize) -> String {
    reply_of(&golden_lines()[..n])
}
