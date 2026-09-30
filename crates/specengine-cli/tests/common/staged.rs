//! Helpers of the pass 2a.2 tests (docs/features/spec-cli-staged.md): a
//! scratch git repository holding a copy of `fixtures/spec-a` or `-b`
//! (every git process isolated by [`super::git::Sandbox`]), `spec` run with
//! the sandbox's variables and a watchdog, "plain" (`spec check` on the
//! same repository) and "the library" (`check_staged`, `today_utc()`).

use std::ffi::OsStr;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use specengine_core::check::Report;
use specengine_store::{GitEnv, check_staged, today_utc};

use super::check::{FAR, baseline_covering, library};
use super::git::Sandbox;
use super::{RUN_TIMEOUT, Run, SPEC, Scratch, copy_dir, fixture, write};

/// A scratch repository: `<scratch>/<dir>` under git, with its sandbox.
pub struct Repo {
    pub scratch: Scratch,
    pub git: Sandbox,
    /// The repository's top level, canonical.
    pub top: PathBuf,
}

impl Repo {
    /// An empty repository at `<scratch>/repo`.
    pub fn empty(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        git.init(&top);
        Self { scratch, git, top }
    }

    /// A repository whose top level is a copy of `fixture`, not yet staged.
    pub fn of(name: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(name);
        let git = Sandbox::new(scratch.path());
        let top = scratch.copy(fixture, "repo");
        git.init(&top);
        Self { scratch, git, top }
    }

    /// A repository whose top level is a copy of `fixture`, `git add -A`.
    pub fn staged(name: &str, fixture: &str) -> Self {
        let repo = Self::of(name, fixture);
        repo.add_all();
        repo
    }

    pub fn add_all(&self) {
        self.git.add_all(&self.top);
    }

    pub fn git(&self, args: &[&str]) -> Vec<u8> {
        self.git.git(&self.top, args)
    }

    /// `spec args` in the top level.
    pub fn spec(&self, args: &[&str]) -> Run {
        spec_in(&self.git, &self.top, args, &[])
    }

    /// `spec check --staged` plus `args` in the top level.
    pub fn staged_check(&self, args: &[&str]) -> Run {
        self.spec(&check_args(true, args))
    }

    /// `spec check` plus `args` in the top level.
    pub fn plain_check(&self, args: &[&str]) -> Run {
        self.spec(&check_args(false, args))
    }

    /// The library's report of the top level's index.
    pub fn library(&self) -> Report {
        library_staged(&self.git, &self.top)
    }
}

/// A superproject at `<scratch>/repo` whose `proj/` is a submodule holding
/// its own project (a copy of `fixture`, a covering baseline: clean) —
/// with (`top_project`) a clean project at the superproject's top as well
/// (`spec-b`, `roots = ["docs"]`, the gitlink `proj` outside its roots, a
/// covering baseline) — `README.txt` at the top, one commit, and a linked
/// worktree at `<scratch>/linked` (canonical) with the submodule checked
/// out (its git dir under the linked worktree's own, `modules/proj`).
/// The submodule is cloned from its local path only
/// (`protocol.file.allow=always`, no network), templates from an empty
/// directory; every git process runs in the sandbox.
pub struct Superproject {
    pub scratch: Scratch,
    pub git: Sandbox,
    /// The superproject's main worktree, canonical.
    pub top: PathBuf,
    /// The superproject's linked worktree, canonical.
    pub linked: PathBuf,
}

impl Superproject {
    pub fn new(label: &str, fixture_name: &str, top_project: bool) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let templates = scratch.dir("templates");
        let local = [
            ("GIT_TEMPLATE_DIR", templates.as_os_str()),
            ("GIT_ALLOW_PROTOCOL", OsStr::new("file")),
        ];

        let sub = scratch.copy(fixture_name, "sub");
        write(
            &sub,
            ".spec-debt.toml",
            baseline_covering(&library(&sub), FAR),
        );
        git.init(&sub);
        git.add_all(&sub);
        git.commit(&sub, "the submodule's project");

        let top = scratch.dir("repo");
        if top_project {
            copy_dir(&fixture("spec-b"), &top);
            write(
                &top,
                ".spec-debt.toml",
                baseline_covering(&library(&top), FAR),
            );
        }
        write(&top, "README.txt", "outside the root\n");
        git.init(&top);
        git.git_env(
            &top,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "--quiet",
                "add",
                sub.to_str().expect("a UTF-8 scratch path"),
                "proj",
            ],
            &local,
        );
        git.add_all(&top);
        git.commit(&top, "base");

        let linked = scratch.path().join("linked");
        git.git(
            &top,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "side",
                linked.to_str().expect("a UTF-8 scratch path"),
            ],
        );
        let linked = std::fs::canonicalize(&linked).expect("the linked worktree");
        git.git_env(
            &linked,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "--quiet",
                "update",
                "--init",
            ],
            &local,
        );
        assert!(
            linked.join("proj/specengine.toml").is_file() && linked.join("proj/.git").is_file(),
            "the submodule is checked out in the linked worktree, a gitfile at its top"
        );
        Self {
            scratch,
            git,
            top,
            linked,
        }
    }

    /// The superproject's linked worktree's git dir (what git exports as
    /// `GIT_DIR` to its hooks there).
    pub fn linked_git_dir(&self) -> PathBuf {
        self.top.join(".git/worktrees/linked")
    }

    /// The submodule's own git dir in the linked worktree, absolute.
    pub fn submodule_git_dir(&self) -> PathBuf {
        PathBuf::from(self.git.git_text(
            &self.linked.join("proj"),
            &["rev-parse", "--absolute-git-dir"],
        ))
    }
}

/// `check [--staged] <args>`, the globals (`--json`, `--root`, `--config`)
/// kept before `check`.
pub fn check_args<'a>(staged: bool, args: &[&'a str]) -> Vec<&'a str> {
    let mut globals = Vec::new();
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(&arg) = iter.next() {
        match arg {
            "--json" => globals.push(arg),
            "--root" | "--config" => {
                globals.push(arg);
                globals.push(iter.next().expect("a value"));
            }
            _ => rest.push(arg),
        }
    }
    globals.push("check");
    if staged {
        globals.push("--staged");
    }
    globals.extend(rest);
    globals
}

/// `check_staged` of `root`: no `--config`, no `--baseline`, git run with
/// the sandbox's variables from `root`, `today_utc()`.
pub fn library_staged(git: &Sandbox, root: &Path) -> Report {
    check_staged(
        root,
        None,
        None,
        &GitEnv::new(root, git.vars()),
        &today_utc(),
    )
}

/// `spec args` in `cwd` with the sandbox's variables plus `extra`, killed
/// after [`RUN_TIMEOUT`].
pub fn spec_in(git: &Sandbox, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Run {
    spec_timed(git, cwd, args, extra, &[], RUN_TIMEOUT)
        .unwrap_or_else(|problem| panic!("{problem}"))
}

/// `spec args` in `cwd` with the sandbox's variables plus `extra`, minus
/// `removed`; `Err` when it runs longer than `limit` (then killed).
pub fn spec_timed(
    git: &Sandbox,
    cwd: &Path,
    args: &[&str],
    extra: &[(&str, &OsStr)],
    removed: &[&str],
    limit: Duration,
) -> Result<Run, String> {
    let mut command = git.command(SPEC, cwd, extra);
    for name in removed {
        command.env_remove(name);
    }
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn spec");
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
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            // A git child left behind holds the pipes open: do not join.
            return Err(format!("spec {args:?} ran longer than {limit:?}; killed"));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = out.join().expect("stdout reader");
    let stderr = err.join().expect("stderr reader");
    Ok(Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    })
}

/// `a` and `b` print the same: exit, stdout, stderr.
pub fn assert_same(a: &Run, b: &Run, context: &str) {
    assert!(
        a.code == b.code && a.stdout == b.stdout && a.stderr == b.stderr,
        "{context}: the runs differ\n=== spec {:?}\n{}\n=== spec {:?}\n{}",
        a.args,
        a.show(),
        b.args,
        b.show()
    );
}

/// `--staged` and plain print the same bytes for each of the output
/// forms: text, `--debt`, `--json`, `--json --debt` (`extra` added).
pub fn assert_parity(repo: &Repo, extra: &[&str], context: &str) {
    for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
        let mut args: Vec<&str> = extra.to_vec();
        args.extend_from_slice(form);
        let staged = repo.staged_check(&args);
        let plain = repo.plain_check(&args);
        assert_same(&staged, &plain, &format!("{context} {args:?}"));
    }
}
