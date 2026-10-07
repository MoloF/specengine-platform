//! Shared harness of the `specengine-http` integration tests
//! (docs/features/daemon-read.md "Acceptance criteria", setup): git copies
//! of `fixtures/spec-a` (A) and `fixtures/spec-b` (B) made by byte copies
//! (never `fs::copy`, which clones on macOS), a scratch `HOME` per test,
//! the daemon binary from the build directory started on `--port 0` with
//! its port read from stdout, and a raw HTTP/1.1 client over
//! `std::net::TcpStream` (no client crate).
//!
//! No server outlives a test: every daemon, `spec` and `specengine-mcp`
//! process runs under `perl -e 'alarm N; exec @ARGV'` (the process dies
//! by SIGALRM even when the test process is killed) and is killed and
//! reaped when its handle drops. Every product process runs with a cleared
//! environment: `HOME` (the test's scratch), `PATH` (the real git's
//! directory, `/usr/bin`, `/bin`), git's global and system configs off.
//!
//! `spec` and `specengine-mcp` are the build directory's siblings of the
//! daemon binary; a test refuses one older than its crates' sources (a
//! stale binary would make parity compare two builds).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

/// The daemon under test.
pub const HTTP: &str = env!("CARGO_BIN_EXE_specengine-http");

/// The self-kill alarm of every spawned product process, seconds.
pub const ALARM_SECS: u32 = 300;

/// The longest wait for the daemon's `listening` line or its exit.
pub const START_TIMEOUT: Duration = Duration::from_secs(60);

/// The longest wait for one HTTP answer.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(120);

/// A `spec` run longer than this is killed and the test fails.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(120);

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

pub fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

// ------------------------------------------------------------ binaries

/// The build directory's `name` next to the daemon binary, refused when
/// missing or older than any source file of `crates` (relative to
/// `crates/`).
pub fn sibling(name: &str, crates: &[&str]) -> PathBuf {
    let path = Path::new(HTTP)
        .parent()
        .expect("the daemon binary has a directory")
        .join(name);
    let built = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|error| {
            panic!(
                "{}: {error}; build it first (`cargo build --workspace --bins`; \
                 `cargo nextest run --workspace` builds it)",
                path.display()
            )
        });
    for krate in crates {
        let src = repository_root().join("crates").join(krate).join("src");
        if let Some((newest, at)) = newest_file(&src)
            && at > built
        {
            panic!(
                "{} is older than {}: rebuild it (`cargo build --workspace --bins`) so the \
                 test does not compare two builds",
                path.display(),
                newest.display()
            );
        }
    }
    path
}

fn newest_file(dir: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let candidate = if kind.is_dir() {
            newest_file(&path)
        } else {
            entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .map(|at| (path, at))
        };
        if let Some((path, at)) = candidate
            && newest.as_ref().is_none_or(|(_, best)| at > *best)
        {
            newest = Some((path, at));
        }
    }
    newest
}

/// The `spec` binary of this build.
pub fn spec_bin() -> PathBuf {
    sibling(
        "spec",
        &[
            "specengine-cli",
            "specengine-store",
            "specengine-core",
            "specengine-model",
            "specengine-code",
        ],
    )
}

/// The `specengine-mcp` binary of this build.
pub fn mcp_bin() -> PathBuf {
    sibling(
        "specengine-mcp",
        &[
            "specengine-mcp",
            "specengine-cli",
            "specengine-store",
            "specengine-core",
            "specengine-model",
            "specengine-code",
        ],
    )
}

// ------------------------------------------------------------ scratch

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A scratch directory under the system temp dir, canonical, removed on
/// drop (permissions restored first).
pub struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-http-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self {
            root: fs::canonicalize(&path).expect("canonical scratch directory"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// A new, empty `HOME` at `<scratch>/homes/<name>`.
    pub fn home(&self, name: &str) -> PathBuf {
        let home = self.root.join("homes").join(name);
        fs::create_dir_all(&home).expect("home directory");
        fs::canonicalize(&home).expect("canonical home")
    }

    /// An empty directory `<scratch>/<dir>`, canonical.
    pub fn dir(&self, dir: &str) -> PathBuf {
        let path = self.root.join(dir);
        fs::create_dir_all(&path).expect("directory");
        fs::canonicalize(&path).expect("canonical directory")
    }

    /// A byte copy of `fixtures/<name>` at `<scratch>/<dir>`, canonical,
    /// not a repository.
    pub fn copy(&self, name: &str, dir: &str) -> PathBuf {
        let root = self.root.join(dir);
        copy_dir(&fixture(name), &root);
        fs::canonicalize(&root).expect("copy exists")
    }

    /// A git repository at `<scratch>/<dir>`: a byte copy of
    /// `fixtures/<name>`, [`QUIET`] in its own config, every file committed
    /// once on `branch`.
    pub fn repo(&self, name: &str, dir: &str, branch: &str) -> PathBuf {
        let root = self.copy(name, dir);
        let git = self.git();
        git.run(
            &root,
            &[
                "init",
                "-q",
                "--template=",
                &format!("--initial-branch={branch}"),
            ],
        );
        git.quiet(&root);
        git.run(&root, &["add", "-A"]);
        git.run(&root, &["commit", "-q", "-m", "fixture"]);
        root
    }

    /// The git sandbox of this scratch.
    pub fn git(&self) -> Git {
        Git::new(&self.root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        restore_permissions(&self.root);
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn restore_permissions(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            restore_permissions(&path);
        } else {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
    }
}

/// Copies every directory and regular file under `from` to `to`.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("copy target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("readable fixture directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type().expect("file type");
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if kind.is_file() {
            copy_file(entry.path(), &target);
        }
    }
}

/// The bytes and permission bits of `from` at `to`. Not `fs::copy`: on
/// macOS it clones first (`fclonefileat`), which a sandboxed run refuses.
pub fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let (from, to) = (from.as_ref(), to.as_ref());
    let bytes = fs::read(from).unwrap_or_else(|error| panic!("{}: {error}", from.display()));
    fs::write(to, bytes).unwrap_or_else(|error| panic!("{}: {error}", to.display()));
    let permissions = fs::metadata(from)
        .unwrap_or_else(|error| panic!("{}: {error}", from.display()))
        .permissions();
    fs::set_permissions(to, permissions)
        .unwrap_or_else(|error| panic!("{}: {error}", to.display()));
}

/// Writes `bytes` to `root/relative`, creating its directories.
pub fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directory");
    fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

pub fn read_text(root: &Path, relative: &str) -> String {
    let path = root.join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Replaces the one occurrence of `from` in `root/relative` by `to`.
pub fn replace(root: &Path, relative: &str, from: &str, to: &str) {
    let text = read_text(root, relative);
    assert_eq!(
        text.matches(from).count(),
        1,
        "{relative}: {from:?} must occur exactly once"
    );
    write(root, relative, text.replacen(from, to, 1));
}

/// Every entry under `dir`, relative, `/`-joined: directories as `None`,
/// files as their bytes; `.git` included.
pub fn snapshot(dir: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                out.insert(relative, None);
                walk(root, &path, out);
            } else if kind.is_symlink() {
                let target = fs::read_link(&path).expect("link");
                out.insert(
                    relative,
                    Some(format!("link -> {}", target.display()).into_bytes()),
                );
            } else {
                out.insert(relative, Some(fs::read(&path).expect("file")));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The directory the host rule puts the index databases in, under `home`.
pub fn data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    }
}

/// Every file under `dir` whose name ends `suffix`, relative.
pub fn files_ending(dir: &Path, suffix: &str) -> Vec<String> {
    snapshot(dir)
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && path.ends_with(suffix))
        .map(|(path, _)| path)
        .collect()
}

// ------------------------------------------------------------ git

/// Git's automatic maintenance off: a git that repacks in the background
/// after a commit (git 2.54's detached geometric repack: loose objects into
/// a pack and a multi-pack-index within seconds) would otherwise race the
/// test's next git command and every snapshot of `.git`. Written into each
/// repository's own config right after `init` ([`Git::quiet`]), which every
/// git process opening the repository reads — the setup's, a product's
/// (whose environment is [`product_env`], untouched; the product's
/// worktree git drops `GIT_CONFIG_COUNT` and `GIT_CONFIG_PARAMETERS` in any
/// case), `/usr/bin/git` called directly — and also given to the setup git
/// as `GIT_CONFIG_*` variables.
pub const QUIET: [(&str, &str); 2] = [("maintenance.auto", "false"), ("gc.auto", "0")];

/// The isolated environment of every git process of a test: no inherited
/// variable, git's global and system configs off, a fixed identity, a
/// scratch `HOME`, `GIT_CEILING_DIRECTORIES` the scratch, and no automatic
/// maintenance ([`QUIET`] as `GIT_CONFIG_*`). Product processes get
/// [`product_env`], untouched.
#[derive(Debug, Clone)]
pub struct Git {
    vars: Vec<(OsString, OsString)>,
}

/// The directory of the real `git`, then `/usr/bin:/bin`.
pub fn product_path() -> OsString {
    let git = real_git();
    let dir = git.parent().expect("git's directory").to_path_buf();
    let mut dirs = vec![dir];
    for extra in ["/usr/bin", "/bin"] {
        let extra = PathBuf::from(extra);
        if !dirs.contains(&extra) {
            dirs.push(extra);
        }
    }
    std::env::join_paths(dirs).expect("PATH")
}

fn real_git() -> PathBuf {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("git");
            let executable = fs::metadata(&candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
            if executable && dir.is_absolute() {
                return candidate;
            }
        }
    }
    PathBuf::from("/usr/bin/git")
}

impl Git {
    fn new(scratch: &Path) -> Self {
        let home = scratch.join("git-home");
        fs::create_dir_all(&home).expect("git home");
        let mut vars: Vec<(OsString, OsString)> = vec![
            ("HOME".into(), home.into_os_string()),
            ("PATH".into(), product_path()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            (
                "GIT_CEILING_DIRECTORIES".into(),
                scratch.parent().expect("scratch parent").as_os_str().into(),
            ),
            ("GIT_CONFIG_COUNT".into(), QUIET.len().to_string().into()),
        ];
        for (index, (key, value)) in QUIET.iter().enumerate() {
            vars.push((format!("GIT_CONFIG_KEY_{index}").into(), (*key).into()));
            vars.push((format!("GIT_CONFIG_VALUE_{index}").into(), (*value).into()));
        }
        for (key, value) in [
            ("GIT_AUTHOR_NAME", "Scratch Author"),
            ("GIT_AUTHOR_EMAIL", "author@example.invalid"),
            ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+0000"),
            ("GIT_COMMITTER_NAME", "Scratch Committer"),
            ("GIT_COMMITTER_EMAIL", "committer@example.invalid"),
            ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+0000"),
        ] {
            vars.push((key.into(), value.into()));
        }
        Self { vars }
    }

    /// The sandbox's variables: the caller's git environment of a CLI
    /// library call (`GitEnv::new(cwd, git.vars())`).
    pub fn vars(&self) -> Vec<(OsString, OsString)> {
        self.vars.clone()
    }

    /// [`QUIET`] into the config of the repository git finds from `cwd`
    /// (right after its `init`).
    pub fn quiet(&self, cwd: &Path) {
        for (key, value) in QUIET {
            self.run(cwd, &["config", "--local", key, value]);
        }
    }

    /// `git args` in `cwd`, which must succeed; its stdout.
    pub fn run(&self, cwd: &Path, args: &[&str]) -> String {
        let output = Command::new(real_git())
            .env_clear()
            .envs(self.vars.iter().map(|(k, v)| (k, v)))
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git {args:?} in {}: {}\n{}",
            cwd.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git stdout is UTF-8")
    }
}

// ------------------------------------------------------------ products

/// The environment of a product process (daemon, `spec`,
/// `specengine-mcp`): `HOME`, `PATH`, git's configs off.
pub fn product_env(home: &Path) -> Vec<(OsString, OsString)> {
    vec![
        ("HOME".into(), home.as_os_str().into()),
        ("PATH".into(), product_path()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
    ]
}

/// `program args` under the self-kill alarm, with a cleared environment
/// plus `env`.
pub fn alarmed(program: &Path, args: &[&OsStr], env: &[(OsString, OsString)]) -> Command {
    let mut command = Command::new("/usr/bin/perl");
    command
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .arg("-e")
        .arg(format!(
            "alarm {ALARM_SECS}; exec @ARGV or die \"exec: $!\""
        ))
        .arg(program)
        .args(args);
    command
}

/// One finished process.
#[derive(Debug, Clone)]
pub struct Run {
    pub args: Vec<String>,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn code(&self, code: i32) -> &Self {
        assert_eq!(self.code, code, "{:?}:\n{}", self.args, self.show());
        self
    }

    pub fn show(&self) -> String {
        format!(
            "exit {}\n--- stdout\n{}--- stderr\n{}",
            self.code, self.stdout, self.stderr
        )
    }

    /// stdout without its one final LF (the daemon's body of the same
    /// document).
    pub fn document(&self) -> &str {
        let body = self
            .stdout
            .strip_suffix('\n')
            .unwrap_or_else(|| panic!("{:?}: stdout ends with no LF\n{}", self.args, self.show()));
        assert!(
            !body.contains('\n'),
            "{:?}: more than one line\n{}",
            self.args,
            self.show()
        );
        body
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(self.document())
            .unwrap_or_else(|error| panic!("{:?}: {error}\n{}", self.args, self.show()))
    }
}

/// Runs `program args` in `cwd` with `product_env(home)` and `stdin`
/// bytes; waits at most [`RUN_TIMEOUT`].
pub fn run_product(program: &Path, home: &Path, cwd: &Path, args: &[&str], stdin: &[u8]) -> Run {
    let os_args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let mut child = alarmed(program, &os_args, &product_env(home))
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn product");
    let mut input = child.stdin.take().expect("stdin");
    let bytes = stdin.to_vec();
    let feeder = thread::spawn(move || {
        let _ = input.write_all(&bytes);
    });
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{} {args:?} ran longer than {RUN_TIMEOUT:?}; killed",
                program.display()
            );
        }
        thread::sleep(Duration::from_millis(5));
    };
    let _ = feeder.join();
    let stdout = out.join().expect("stdout reader");
    let stderr = err.join().expect("stderr reader");
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("exited, not killed by a signal"),
        stdout: String::from_utf8(stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    }
}

/// `spec args` in `cwd` under `HOME=home`.
pub fn spec(home: &Path, cwd: &Path, args: &[&str]) -> Run {
    run_product(&spec_bin(), home, cwd, args, b"")
}

/// `spec --root <root> <args> --json`, exit 0 or 1 (a document).
pub fn spec_json(home: &Path, cwd: &Path, root: &Path, args: &[&str]) -> Run {
    let root = root.to_str().expect("UTF-8 root");
    let mut all = vec!["--root", root];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(home, cwd, &all);
    assert!(
        run.code == 0 || run.code == 1,
        "{all:?}: no document\n{}",
        run.show()
    );
    run
}

/// `spec --root <root> propose question <targets> …`, stored (exit 0);
/// its ID.
pub fn ask(home: &Path, root: &Path, targets: &[&str], text: &str) -> String {
    let root = root.to_str().expect("UTF-8 root");
    let mut args = vec!["--root", root, "propose", "question"];
    args.extend_from_slice(targets);
    args.extend_from_slice(&[
        "--text",
        text,
        "--working-answer",
        "keep 1.5 s",
        "--price-of-other",
        "R-12 rebalanced",
    ]);
    let run = spec(home, Path::new(root), &args);
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

/// `spec --root <root> propose update TARGET` with `text`, against the
/// node's current span hash; its ID.
pub fn propose_update(home: &Path, cwd: &Path, root: &Path, target: &str, text: &str) -> String {
    let shown = spec_json(home, cwd, root, &["show", target]).json();
    let base = shown["nodes"][0]["span_hash"]
        .as_str()
        .expect("span_hash")
        .to_owned();
    let root = root.to_str().expect("UTF-8 root");
    let run = run_product(
        &spec_bin(),
        home,
        cwd,
        &[
            "--root",
            root,
            "propose",
            "update",
            target,
            "--base",
            &base,
            "--text-file",
            "-",
            "--rationale",
            "A test edit.",
        ],
        text.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

// ------------------------------------------------------------ the daemon

/// A running daemon; killed and reaped on drop.
pub struct Server {
    child: Child,
    pub port: u16,
    /// Its stdout lines up to `listening`.
    pub lines: Vec<String>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

/// A daemon that exited before listening.
#[derive(Debug)]
pub struct Refused {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Server {
    /// `specengine-http args` in `cwd` under `HOME=home`, until its
    /// `listening` line; `Err` when it exits first.
    pub fn try_start(home: &Path, cwd: &Path, args: &[&OsStr]) -> Result<Self, Refused> {
        Self::try_start_env(&product_env(home), cwd, args)
    }

    /// As [`Server::try_start`] with the environment `env` instead of
    /// [`product_env`] (a `PATH` without git, say).
    pub fn try_start_env(
        env: &[(OsString, OsString)],
        cwd: &Path,
        args: &[&OsStr],
    ) -> Result<Self, Refused> {
        let mut child = alarmed(Path::new(HTTP), args, env)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn specengine-http");
        let stdout = child.stdout.take().expect("stdout");
        let mut stderr_pipe = child.stderr.take().expect("stderr");
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&stderr);
        thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match stderr_pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink
                        .lock()
                        .expect("stderr sink")
                        .extend_from_slice(&buffer[..n]),
                }
            }
        });
        let (sender, lines_in): (_, Receiver<String>) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let started = Instant::now();
        let mut lines = Vec::new();
        loop {
            let left = START_TIMEOUT.saturating_sub(started.elapsed());
            match lines_in.recv_timeout(left) {
                Ok(line) => {
                    let port = line
                        .strip_prefix("listening http://127.0.0.1:")
                        .map(|port| port.parse::<u16>().expect("a port"));
                    lines.push(line);
                    if let Some(port) = port {
                        return Ok(Self {
                            child,
                            port,
                            lines,
                            stderr,
                        });
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let status = wait_for_exit(&mut child);
                    thread::sleep(Duration::from_millis(50));
                    let stderr =
                        String::from_utf8_lossy(&stderr.lock().expect("stderr")).into_owned();
                    return Err(Refused {
                        code: status,
                        stdout: lines.iter().map(|line| format!("{line}\n")).collect(),
                        stderr,
                    });
                }
                Err(RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "specengine-http {args:?}: no `listening` line within {START_TIMEOUT:?}; killed"
                    );
                }
            }
        }
    }

    /// As [`Server::try_start`], which must listen.
    pub fn start(home: &Path, cwd: &Path, args: &[&OsStr]) -> Self {
        Self::try_start(home, cwd, args)
            .unwrap_or_else(|refused| panic!("specengine-http {args:?} did not start: {refused:?}"))
    }

    /// `--root` for each of `roots`, then `--port 0`.
    pub fn serve(home: &Path, cwd: &Path, roots: &[&Path]) -> Self {
        Self::start(home, cwd, &root_args(roots))
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().expect("stderr")).into_owned()
    }

    /// `GET path` with this server's own `Host`.
    pub fn get(&self, path: &str) -> Reply {
        self.request("GET", path, &[])
    }

    /// `method path` with this server's own `Host` and `headers`.
    pub fn request(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> Reply {
        let host = format!("127.0.0.1:{}", self.port);
        let mut all = vec![("Host", host.as_str())];
        all.extend_from_slice(headers);
        self.raw(&request_bytes(method, path, &all))
    }

    /// The bytes as written, answered.
    pub fn raw(&self, bytes: &[u8]) -> Reply {
        http_exchange(self.port, bytes, HTTP_TIMEOUT)
    }

    /// As [`Server::request`], the answer complete within `timeout` (an
    /// answer that may be an endless stream).
    pub fn request_within(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Reply {
        let host = format!("127.0.0.1:{}", self.port);
        let mut all = vec![("Host", host.as_str())];
        all.extend_from_slice(headers);
        http_exchange(self.port, &request_bytes(method, path, &all), timeout)
    }

    /// Whether the process is still running.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for_exit(child: &mut Child) -> Option<i32> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status.code();
        }
        if started.elapsed() > START_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("specengine-http closed stdout but did not exit; killed");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// `--root R` per root, then `--port 0`.
pub fn root_args(roots: &[&Path]) -> Vec<&'static OsStr> {
    let mut args: Vec<&'static OsStr> = Vec::new();
    for root in roots {
        args.push(OsStr::new("--root"));
        let leaked: &'static OsStr = Box::leak(root.as_os_str().to_os_string().into_boxed_os_str());
        args.push(leaked);
    }
    args.push(OsStr::new("--port"));
    args.push(OsStr::new("0"));
    args
}

// ------------------------------------------------------------ HTTP

/// One HTTP answer.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    /// Names lowercased, in order.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.body).expect("UTF-8 body")
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|error| panic!("not JSON ({error}): {self:?}"))
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Asserts the status, showing the answer when it differs.
    pub fn status(&self, status: u16) -> &Self {
        assert_eq!(
            self.status,
            status,
            "status {} body {}",
            self.status,
            String::from_utf8_lossy(&self.body)
        );
        self
    }

    /// The error body `{"status":<code>,"message":"…"}`, exactly two keys,
    /// its status the answer's; the message.
    pub fn error_message(&self) -> String {
        let value = self.json();
        let object = value
            .as_object()
            .unwrap_or_else(|| panic!("not an object: {self:?}"));
        assert_eq!(
            object.len(),
            2,
            "the error body has exactly two keys: {}",
            self.text()
        );
        assert!(
            self.text()
                .starts_with(&format!("{{\"status\":{},\"message\":\"", self.status)),
            "the error body is compact `{{\"status\":<code>,\"message\":\"…\"}}`: {}",
            self.text()
        );
        assert_eq!(
            value["status"],
            serde_json::json!(self.status),
            "{}",
            self.text()
        );
        assert_eq!(
            self.header("content-type"),
            Some("application/json; charset=utf-8")
        );
        value["message"].as_str().expect("message").to_owned()
    }

    /// Any `Access-Control-*` header.
    pub fn cors_headers(&self) -> Vec<&(String, String)> {
        self.headers
            .iter()
            .filter(|(name, _)| name.starts_with("access-control-"))
            .collect()
    }
}

/// A request's bytes, `Connection: close` added.
pub fn request_bytes(method: &str, path: &str, headers: &[(&str, &str)]) -> Vec<u8> {
    let mut text = format!("{method} {path} HTTP/1.1\r\n");
    for (name, value) in headers {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("Connection: close\r\n\r\n");
    text.into_bytes()
}

/// Writes `bytes` to `127.0.0.1:port` and reads the answer to EOF, all
/// of it within `timeout`.
pub fn http_exchange(port: u16, bytes: &[u8], timeout: Duration) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the daemon");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("timeout");
    stream.write_all(bytes).expect("write the request");
    let started = Instant::now();
    let mut raw = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&buffer[..n]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("read the answer: {error}"),
        }
        assert!(
            started.elapsed() < timeout,
            "no complete answer within {timeout:?}: {:?}",
            String::from_utf8_lossy(&raw[..raw.len().min(600)])
        );
    }
    parse_reply(&raw)
}

fn parse_reply(raw: &[u8]) -> Reply {
    let split = find(raw, b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no header end: {:?}", String::from_utf8_lossy(raw)));
    let head = std::str::from_utf8(&raw[..split]).expect("ASCII head");
    let mut lines = head.split("\r\n");
    let status_line = lines.next().expect("status line");
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("status line {status_line:?}"));
    let headers: Vec<(String, String)> = lines
        .map(|line| {
            let (name, value) = line.split_once(':').expect("header line");
            (name.trim().to_ascii_lowercase(), value.trim().to_owned())
        })
        .collect();
    let rest = &raw[split + 4..];
    let chunked = headers
        .iter()
        .any(|(name, value)| name == "transfer-encoding" && value.contains("chunked"));
    let body = if chunked {
        dechunk(rest)
    } else {
        rest.to_vec()
    };
    Reply {
        status,
        headers,
        body,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    while let Some(end) = find(rest, b"\r\n") {
        let size_text = std::str::from_utf8(&rest[..end]).expect("chunk size");
        let size = usize::from_str_radix(size_text.split(';').next().unwrap().trim(), 16)
            .expect("hex chunk size");
        rest = &rest[end + 2..];
        if size == 0 {
            break;
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
    body
}

/// `encodeURIComponent`: every byte but `A-Z a-z 0-9 - _ . ! ~ * ' ( )`
/// as `%XX`.
pub fn encode_component(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

// ------------------------------------------------------------ SSE

/// One server-sent event block as received.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: Option<String>,
    /// `:` lines.
    pub comments: Vec<String>,
    /// The block's bytes as sent, without its blank line.
    pub raw: String,
}

/// An open `events` stream.
pub struct Stream {
    stream: TcpStream,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Decoded body bytes not yet split into blocks.
    pending: Vec<u8>,
    /// Raw socket bytes not yet de-chunked.
    raw: Vec<u8>,
    chunked: bool,
    ended: bool,
}

impl Stream {
    /// `GET path` with this server's `Host` and `headers`; the head read.
    pub fn open(port: u16, path: &str, headers: &[(&str, &str)]) -> Self {
        let host = format!("127.0.0.1:{port}");
        let mut all = vec![("Host", host.as_str()), ("Accept", "text/event-stream")];
        all.extend_from_slice(headers);
        let mut text = format!("GET {path} HTTP/1.1\r\n");
        for (name, value) in &all {
            text.push_str(&format!("{name}: {value}\r\n"));
        }
        text.push_str("\r\n");
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the daemon");
        stream
            .write_all(text.as_bytes())
            .expect("write the request");
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        let mut raw = Vec::new();
        let started = Instant::now();
        let split = loop {
            if let Some(split) = find(&raw, b"\r\n\r\n") {
                break split;
            }
            assert!(started.elapsed() < HTTP_TIMEOUT, "no answer head to {path}");
            let mut buffer = [0u8; 8192];
            match stream.read(&mut buffer) {
                Ok(0) => panic!(
                    "closed before the head: {:?}",
                    String::from_utf8_lossy(&raw)
                ),
                Ok(n) => raw.extend_from_slice(&buffer[..n]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => panic!("read: {error}"),
            }
        };
        let reply = parse_reply(&raw[..split + 4]);
        let chunked = reply
            .headers
            .iter()
            .any(|(name, value)| name == "transfer-encoding" && value.contains("chunked"));
        Self {
            stream,
            status: reply.status,
            headers: reply.headers,
            pending: Vec::new(),
            raw: raw[split + 4..].to_vec(),
            chunked,
            ended: false,
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The rest of a non-stream answer (an error body), read to EOF.
    pub fn rest_of_body(mut self) -> Vec<u8> {
        self.stream
            .set_read_timeout(Some(HTTP_TIMEOUT))
            .expect("timeout");
        let mut more = Vec::new();
        let _ = self.stream.read_to_end(&mut more);
        let mut all = std::mem::take(&mut self.raw);
        all.extend_from_slice(&more);
        if self.chunked { dechunk(&all) } else { all }
    }

    fn decode(&mut self) {
        if !self.chunked {
            self.pending.append(&mut self.raw);
            return;
        }
        loop {
            let Some(end) = find(&self.raw, b"\r\n") else {
                return;
            };
            let size_text = std::str::from_utf8(&self.raw[..end]).expect("chunk size");
            let size = usize::from_str_radix(size_text.split(';').next().unwrap().trim(), 16)
                .expect("hex chunk size");
            if size == 0 {
                self.ended = true;
                self.raw.clear();
                return;
            }
            if self.raw.len() < end + 2 + size + 2 {
                return;
            }
            let start = end + 2;
            self.pending
                .extend_from_slice(&self.raw[start..start + size]);
            self.raw.drain(..start + size + 2);
        }
    }

    /// The next block within `timeout`; `None` when none came (or the
    /// stream ended: see [`Stream::ended`]).
    pub fn next_frame(&mut self, timeout: Duration) -> Option<Frame> {
        let started = Instant::now();
        loop {
            self.decode();
            if let Some(end) = find(&self.pending, b"\n\n") {
                let block: Vec<u8> = self.pending.drain(..end + 2).collect();
                let text = String::from_utf8(block[..end].to_vec()).expect("UTF-8 event");
                return Some(parse_frame(&text));
            }
            if self.ended || started.elapsed() >= timeout {
                return None;
            }
            let mut buffer = [0u8; 8192];
            match self.stream.read(&mut buffer) {
                Ok(0) => {
                    self.ended = true;
                    return None;
                }
                Ok(n) => self.raw.extend_from_slice(&buffer[..n]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => {
                    self.ended = true;
                    return None;
                }
            }
        }
    }

    /// The next block that carries an `event:` within `timeout` (skipping
    /// comments).
    pub fn next_event(&mut self, timeout: Duration) -> Option<Frame> {
        let started = Instant::now();
        loop {
            let left = timeout.saturating_sub(started.elapsed());
            let frame = self.next_frame(left)?;
            if frame.event.is_some() || frame.data.is_some() {
                return Some(frame);
            }
            if started.elapsed() >= timeout {
                return None;
            }
        }
    }

    /// Every block that comes within `window`.
    pub fn frames_within(&mut self, window: Duration) -> Vec<Frame> {
        let started = Instant::now();
        let mut frames = Vec::new();
        loop {
            let left = window.saturating_sub(started.elapsed());
            if left.is_zero() {
                return frames;
            }
            match self.next_frame(left) {
                Some(frame) => frames.push(frame),
                None if self.ended => return frames,
                None => {}
            }
        }
    }

    pub fn ended(&self) -> bool {
        self.ended
    }
}

fn parse_frame(text: &str) -> Frame {
    let mut frame = Frame {
        raw: text.to_owned(),
        ..Frame::default()
    };
    for line in text.split('\n') {
        if let Some(comment) = line.strip_prefix(':') {
            frame.comments.push(comment.to_owned());
        } else if let Some(id) = line.strip_prefix("id: ") {
            frame.id = Some(id.to_owned());
        } else if let Some(event) = line.strip_prefix("event: ") {
            frame.event = Some(event.to_owned());
        } else if let Some(data) = line.strip_prefix("data: ") {
            frame.data = Some(match frame.data.take() {
                Some(before) => format!("{before}\n{data}"),
                None => data.to_owned(),
            });
        }
    }
    frame
}

// ------------------------------------------------------------ MCP

/// A `specengine-mcp` process over stdio; killed on drop.
pub struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
}

impl Mcp {
    /// Started in `cwd` under `HOME=home`, initialized.
    pub fn start(home: &Path, cwd: &Path) -> Self {
        let mut child = alarmed(&mcp_bin(), &[], &product_env(home))
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn specengine-mcp");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout");
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let mut mcp = Self {
            child,
            stdin,
            lines,
        };
        let reply = mcp.request(
            0,
            "initialize",
            serde_json::json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "specengine-http-tests", "version": "0.0.0"}
            }),
        );
        assert!(reply.get("result").is_some(), "initialize: {reply}");
        mcp.send(&serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        let mut line = serde_json::to_string(message).expect("JSON");
        line.push('\n');
        stdin.write_all(line.as_bytes()).expect("write");
        stdin.flush().expect("flush");
    }

    /// The reply to request `id`.
    pub fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        );
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(60))
                .expect("an answer from specengine-mcp within 60 s");
            let message: Value = serde_json::from_str(&line).expect("JSON-RPC line");
            if message.get("id") == Some(&serde_json::json!(id)) {
                return message;
            }
        }
    }

    /// Closes stdin and waits for the exit (the process's end).
    pub fn finish(mut self) {
        self.stdin.take();
        let started = Instant::now();
        loop {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            if started.elapsed() > Duration::from_secs(20) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("specengine-mcp did not exit after stdin closed");
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
