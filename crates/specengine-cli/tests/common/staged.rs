//! Helpers of the pass 2a.2 tests (docs/features/spec-cli-staged.md): a
//! scratch git repository holding a copy of `fixtures/spec-a` or `-b`
//! (every git process isolated by [`super::git::Sandbox`]), `spec` run with
//! the sandbox's variables and a watchdog, "plain" (`spec check` on the
//! same repository) and "the library" (`check_staged`, `today_utc()`).
//!
//! Since docs/features/spec-cli-introduced.md (2a.2 Q4) every `--staged`
//! run is judged against `HEAD`: a `--staged` run equals plain only with
//! the base's fields set aside ([`base_aside`]), under `enforce`, with no
//! new debt.

use std::ffi::OsStr;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use specengine_core::check::Report;
use specengine_store::{GitEnv, check_staged, check_staged_with_notes, today_utc};

use super::check::{FAR, baseline_covering, library};
use super::git::{Sandbox, quiet_config_text};
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
/// (`protocol.file.allow=always`, no network), templates from a directory
/// holding only a `config` of [`super::git::QUIET`] (each submodule clone
/// starts without automatic maintenance, as `Sandbox::init` leaves every
/// repository it makes); every git process runs in the sandbox.
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
        std::fs::write(templates.join("config"), quiet_config_text()).expect("the template config");
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
/// the sandbox's variables from `root`, `today_utc()`. The report keeps
/// the base's fields (the CLI prints them too); the notes are taken from
/// `check_staged_with_notes`, which must give the same report, and must be
/// none (the CLI's stderr would print them).
pub fn library_staged(git: &Sandbox, root: &Path) -> Report {
    let env = GitEnv::new(root, git.vars());
    let today = today_utc();
    let report = check_staged(root, None, None, &env, &today);
    let with_notes = check_staged_with_notes(root, None, None, &env, &today);
    assert_eq!(
        with_notes.report,
        report,
        "check_staged and check_staged_with_notes of {}",
        root.display()
    );
    assert!(
        with_notes.notes.is_empty(),
        "notes of {}: {:?}",
        root.display(),
        with_notes.notes
    );
    report
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

/// Whether `run` is a `spec check` with a base: `--staged` or `--changed`.
pub fn is_staged(run: &Run) -> bool {
    run.args
        .iter()
        .any(|arg| arg == "--staged" || arg == "--changed")
}

/// `run` with the base's fields set aside (docs/features/spec-cli-introduced.md,
/// Data): what a run without a base prints. Text: the summary's
/// `, <i> introduced` and `, 0 new debt` before `, worst W`, and the
/// ` (pre-existing)` ending of a finding line. JSON: each finding's last
/// field `introduced`, `counts.introduced`, `counts.new_debt` of 0 and an
/// empty `new_debt`. New debt (a `new` line, a non-empty `new_debt`, a
/// count above 0), the mode, the verdict and stderr's notes are kept, so
/// they still show as a difference.
pub fn base_aside(run: &Run) -> Run {
    let stdout = run
        .stdout
        .split_inclusive('\n')
        .map(|line| {
            if line.starts_with('{') {
                json_base_aside(line)
            } else if line.starts_with("spec check [") {
                summary_base_aside(line)
            } else {
                match line.strip_suffix(" (pre-existing)\n") {
                    Some(kept) => format!("{kept}\n"),
                    None => line.to_owned(),
                }
            }
        })
        .collect();
    Run {
        args: run.args.clone(),
        code: run.code,
        stdout,
        stderr: run.stderr.clone(),
    }
}

/// The summary line without `, <i> introduced` and `, 0 new debt`.
fn summary_base_aside(line: &str) -> String {
    let Some(at) = line.find(", worst W ") else {
        return line.to_owned();
    };
    let (mut head, tail) = line.split_at(at);
    if let Some(kept) = head.strip_suffix(", 0 new debt") {
        head = kept;
    }
    if let Some(cut) = head.rfind(", ")
        && let Some(count) = head[cut + 2..].strip_suffix(" introduced")
        && !count.is_empty()
        && count.bytes().all(|byte| byte.is_ascii_digit())
    {
        head = &head[..cut];
    }
    format!("{head}{tail}")
}

/// A JSON report line without the base's fields (encoded strings never
/// hold a bare `"`, so the patterns cannot match inside one).
fn json_base_aside(line: &str) -> String {
    let mut text = line
        .replace(",\"introduced\":true}", "}")
        .replace(",\"introduced\":false}", "}")
        .replace(",\"new_debt\":0,\"worst_w_bytes\":", ",\"worst_w_bytes\":")
        .replace(",\"new_debt\":[],", ",");
    const COUNT: &str = ",\"introduced\":";
    if let Some(at) = text.find(COUNT) {
        let digits = text[at + COUNT.len()..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        let end = at + COUNT.len() + digits;
        if digits > 0 && text[end..].starts_with(',') {
            text.replace_range(at..end, "");
        }
    }
    text
}

/// `a` and `b` print the same: exit, stdout, stderr. When exactly one of
/// them is a `--staged` run, its base's fields are set aside first
/// ([`base_aside`]): only a run without a base prints plain's bytes.
pub fn assert_same(a: &Run, b: &Run, context: &str) {
    let (a, b) = match (is_staged(a), is_staged(b)) {
        (true, false) => (base_aside(a), b.clone()),
        (false, true) => (a.clone(), base_aside(b)),
        _ => (a.clone(), b.clone()),
    };
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
/// forms: text, `--debt`, `--json`, `--json --debt` (`extra` added), the
/// staged run's base fields set aside ([`assert_same`]).
pub fn assert_parity(repo: &Repo, extra: &[&str], context: &str) {
    for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
        let mut args: Vec<&str> = extra.to_vec();
        args.extend_from_slice(form);
        let staged = repo.staged_check(&args);
        let plain = repo.plain_check(&args);
        assert_same(&staged, &plain, &format!("{context} {args:?}"));
    }
}

/// A logging `git` wrapper first on `PATH` (docs/features/spec-cli-introduced.md
/// AC-12): each call appends its argv, the four forced variables,
/// `GIT_DIR` and the directory it runs in (`pwd -P`) to `git.log`; a
/// `cat-file` call's stdin (the OIDs requested) is copied to
/// `cat-file.in` through `tee` as it arrives, so the lockstep session is
/// kept. Then the real git runs.
pub struct GitLog {
    log: PathBuf,
    requests: PathBuf,
    path: std::ffi::OsString,
}

/// One logged git call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCall {
    pub argv: Vec<String>,
    /// `GIT_OPTIONAL_LOCKS`, `GIT_NO_LAZY_FETCH`, `GIT_NO_REPLACE_OBJECTS`,
    /// `GIT_TERMINAL_PROMPT` (`unset` when unset).
    pub forced: Vec<String>,
    /// `GIT_DIR`, `unset` when unset.
    pub git_dir: String,
    /// The directory git ran in, physical.
    pub cwd: PathBuf,
}

impl GitCall {
    /// The argv after `-c core.fsmonitor=false`, else the whole argv.
    pub fn sub(&self) -> Vec<&str> {
        let argv: Vec<&str> = self.argv.iter().map(String::as_str).collect();
        match argv.as_slice() {
            ["-c", "core.fsmonitor=false", rest @ ..] => rest.to_vec(),
            _ => argv,
        }
    }

    /// The subcommand.
    pub fn name(&self) -> &str {
        self.sub().first().copied().unwrap_or_default()
    }
}

impl GitLog {
    pub fn new(scratch: &Scratch, git: &Sandbox) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch.dir("git-log-wrapper");
        let log = scratch.path().join("git.log");
        let requests = scratch.path().join("cat-file.in");
        let script = format!(
            "#!/bin/sh\n\
             {{\n\
             printf 'argv'\n\
             for arg in \"$@\"; do printf '\\037%s' \"$arg\"; done\n\
             printf '\\036%s\\037%s\\037%s\\037%s\\036%s\\036%s\\n' \"${{GIT_OPTIONAL_LOCKS-unset}}\" \"${{GIT_NO_LAZY_FETCH-unset}}\" \"${{GIT_NO_REPLACE_OBJECTS-unset}}\" \"${{GIT_TERMINAL_PROMPT-unset}}\" \"${{GIT_DIR-unset}}\" \"$(pwd -P)\"\n\
             }} >> '{log}'\n\
             if [ \"$3\" = cat-file ]; then\n\
             \x20 tee -a '{requests}' | '{git}' \"$@\"\n\
             \x20 exit $?\n\
             fi\n\
             exec '{git}' \"$@\"\n",
            log = log.display(),
            requests = requests.display(),
            git = git.git_program().display()
        );
        let wrapper = dir.join("git");
        std::fs::write(&wrapper, script).expect("the wrapper");
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
            .expect("the wrapper's mode");
        let mut path = dir.into_os_string();
        path.push(":");
        path.push(git.var("PATH").expect("the sandbox's PATH"));
        Self {
            log,
            requests,
            path,
        }
    }

    /// The `PATH` with the wrapper first, for [`spec_in`].
    pub fn env(&self) -> [(&str, &OsStr); 1] {
        [("PATH", self.path.as_os_str())]
    }

    /// Forgets every logged call and request.
    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.log);
        let _ = std::fs::remove_file(&self.requests);
    }

    /// The calls logged since the last [`GitLog::clear`], in order.
    pub fn calls(&self) -> Vec<GitCall> {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        text.lines()
            .map(|line| {
                let mut parts = line.split('\u{1e}');
                let argv = parts
                    .next()
                    .and_then(|argv| argv.strip_prefix("argv"))
                    .expect("argv")
                    .split('\u{1f}')
                    .skip(1)
                    .map(str::to_owned)
                    .collect();
                let forced = parts
                    .next()
                    .expect("the forced variables")
                    .split('\u{1f}')
                    .map(str::to_owned)
                    .collect();
                let git_dir = parts.next().expect("GIT_DIR").to_owned();
                let cwd = PathBuf::from(parts.next().expect("the directory"));
                GitCall {
                    argv,
                    forced,
                    git_dir,
                    cwd,
                }
            })
            .collect()
    }

    /// The lines `cat-file` read on stdin since the last clear, in order.
    pub fn requests(&self) -> Vec<String> {
        std::fs::read_to_string(&self.requests)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}
