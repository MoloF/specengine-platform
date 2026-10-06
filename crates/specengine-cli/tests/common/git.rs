//! The scratch-git helper of docs/features/spec-cli-staged.md (its
//! acceptance-criteria rules, tested by AC-15): every git process a
//! `--staged` test starts — setup and product (the `spec` binary and the
//! git it runs) alike — gets exactly [`Sandbox::vars`]: the test process's
//! environment with every `GIT_*` variable removed (so `GIT_DIR`,
//! `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`,
//! `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR` of a hook or a
//! shell never reach a scratch repository), then `GIT_CONFIG_GLOBAL=/dev/null`,
//! `GIT_CONFIG_NOSYSTEM=1`, a scratch `HOME` and `XDG_CONFIG_HOME`,
//! `GIT_CEILING_DIRECTORIES` = the scratch parent, a fixed author and
//! committer, and `PATH` = the directory of the real `git` plus
//! `/usr/bin:/bin`. Repositories are made by `git init --template=`, and
//! each new repository's own config at once turns git's automatic
//! maintenance off ([`QUIET`]); no remote is ever added. A test sets a
//! variable only through [`Sandbox::git_env`] or its own `spec` call.
//!
//! Why the repository's own config: git 2.54 (Apple Git-157) follows a plain
//! `git commit` (and a push into a repository) with a detached
//! `git maintenance run --auto` that within seconds turns loose objects
//! into a pack and a multi-pack-index — a change to `.git` racing every
//! "no repository file changed" snapshot and every test that removes a
//! loose object. `GIT_CONFIG_*` variables or `-c` reach only the setup git:
//! the product drops `GIT_CONFIG_COUNT` and `GIT_CONFIG_PARAMETERS` before
//! it runs git in a worktree, a local push drops them for the receiving
//! side, and `/usr/bin/git` called directly never sees a `PATH` wrapper.
//! The repository's config reaches every git process that opens the
//! repository.
//!
//! Self-contained (no `super::`): the store's tests include it by path.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The fixed author and committer of every scratch commit.
pub const IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "Scratch Author"),
    ("GIT_AUTHOR_EMAIL", "author@example.invalid"),
    ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+0000"),
    ("GIT_COMMITTER_NAME", "Scratch Committer"),
    ("GIT_COMMITTER_EMAIL", "committer@example.invalid"),
    ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+0000"),
];

/// The repository-local config every scratch repository gets right after
/// `git init` (see the module docs): no automatic maintenance
/// (`maintenance.auto`, the switch git 2.54's detached repack honours),
/// no automatic `gc --auto` (`gc.auto`, an older git's path).
pub const QUIET: [(&str, &str); 2] = [("maintenance.auto", "false"), ("gc.auto", "0")];

/// [`QUIET`] as a git config file, for a template directory
/// (`GIT_TEMPLATE_DIR`): a repository git makes from it — a clone, a
/// submodule — starts with it.
pub fn quiet_config_text() -> String {
    let mut text = String::new();
    for (key, value) in QUIET {
        let (section, name) = key.split_once('.').expect("a section.name key");
        for part in ["[", section, "]\n\t", name, " = ", value, "\n"] {
            text.push_str(part);
        }
    }
    text
}

/// Replaced whatever the test process holds.
const OVERRIDDEN: [&str; 3] = ["HOME", "XDG_CONFIG_HOME", "PATH"];

/// Whether an inherited variable reaches a git process: no `GIT_*`
/// variable does, nor one the sandbox sets itself.
pub fn inherited(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    !name.starts_with("GIT_") && !OVERRIDDEN.contains(&name.as_ref())
}

/// The first executable `git` on `path` (a `PATH` value), else
/// `/usr/bin/git`.
pub fn real_git(path: Option<&OsStr>) -> PathBuf {
    if let Some(path) = path {
        for dir in std::env::split_paths(path) {
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

/// The isolated environment of every git process of one test.
#[derive(Debug, Clone)]
pub struct Sandbox {
    parent: PathBuf,
    home: PathBuf,
    xdg: PathBuf,
    git: PathBuf,
    vars: BTreeMap<OsString, OsString>,
}

impl Sandbox {
    /// Over the test process's own environment, below `parent` (the scratch
    /// directory every repository of the test lives in).
    pub fn new(parent: &Path) -> Self {
        Self::over(parent, std::env::vars_os())
    }

    /// Over `base`, the environment a git process would otherwise inherit.
    pub fn over(parent: &Path, base: impl IntoIterator<Item = (OsString, OsString)>) -> Self {
        let parent = fs::canonicalize(parent).expect("the scratch parent exists");
        let home = parent.join("git-home");
        let xdg = parent.join("git-xdg");
        fs::create_dir_all(&home).expect("scratch HOME");
        fs::create_dir_all(&xdg).expect("scratch XDG_CONFIG_HOME");
        let base: Vec<(OsString, OsString)> = base.into_iter().collect();
        let git = real_git(
            base.iter()
                .find(|(name, _)| name == "PATH")
                .map(|(_, value)| value.as_os_str()),
        );
        let mut path = vec![git.parent().expect("git's directory").to_path_buf()];
        for dir in ["/usr/bin", "/bin"] {
            if !path.iter().any(|known| known == Path::new(dir)) {
                path.push(PathBuf::from(dir));
            }
        }
        let mut vars: BTreeMap<OsString, OsString> = base
            .into_iter()
            .filter(|(name, _)| inherited(name))
            .collect();
        vars.insert("HOME".into(), home.clone().into());
        vars.insert("XDG_CONFIG_HOME".into(), xdg.clone().into());
        vars.insert(
            "PATH".into(),
            std::env::join_paths(path).expect("a PATH value"),
        );
        vars.insert("GIT_CONFIG_GLOBAL".into(), "/dev/null".into());
        vars.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
        vars.insert("GIT_CEILING_DIRECTORIES".into(), parent.clone().into());
        for (name, value) in IDENTITY {
            vars.insert(name.into(), value.into());
        }
        Self {
            parent,
            home,
            xdg,
            git,
            vars,
        }
    }

    /// The scratch parent (`GIT_CEILING_DIRECTORIES`).
    pub fn parent(&self) -> &Path {
        &self.parent
    }

    /// The scratch `HOME`.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The scratch `XDG_CONFIG_HOME`.
    pub fn xdg(&self) -> &Path {
        &self.xdg
    }

    /// The real `git` the setup runs.
    pub fn git_program(&self) -> &Path {
        &self.git
    }

    /// Every variable a git process of the test gets.
    pub fn vars(&self) -> Vec<(OsString, OsString)> {
        self.vars
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    }

    /// The value of `name` in [`Sandbox::vars`].
    pub fn var(&self, name: &str) -> Option<&OsStr> {
        self.vars.get(OsStr::new(name)).map(OsString::as_os_str)
    }

    /// `program` in `cwd` with exactly [`Sandbox::vars`] plus `extra`,
    /// stdin null.
    pub fn command(
        &self,
        program: impl AsRef<OsStr>,
        cwd: &Path,
        extra: &[(&str, &OsStr)],
    ) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(cwd)
            .env_clear()
            .envs(&self.vars)
            .stdin(Stdio::null());
        for (name, value) in extra {
            command.env(name, value);
        }
        command
    }

    /// `git args` in `cwd`, whatever its outcome.
    pub fn git_output(&self, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Output {
        self.command(&self.git, cwd, extra)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("git {args:?} in {}: {error}", cwd.display()))
    }

    /// `git args` in `cwd` with `extra` variables; it must succeed.
    pub fn git_env(&self, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Vec<u8> {
        let output = self.git_output(cwd, args, extra);
        assert!(
            output.status.success(),
            "git {args:?} in {} failed ({}):\n{}{}",
            cwd.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    /// `git args` in `cwd`; it must succeed. Its stdout.
    pub fn git(&self, cwd: &Path, args: &[&str]) -> Vec<u8> {
        self.git_env(cwd, args, &[])
    }

    /// `git args` in `cwd`; its stdout as text, trimmed.
    pub fn git_text(&self, cwd: &Path, args: &[&str]) -> String {
        String::from_utf8(self.git(cwd, args))
            .expect("UTF-8 git output")
            .trim()
            .to_owned()
    }

    /// `git init --template= -q -b main` of `dir` (created when absent),
    /// with `extra` arguments before the directory, then [`QUIET`] in the
    /// new repository's config (its git directory's, wherever
    /// `--separate-git-dir` puts it; a `--bare` one's own).
    pub fn init_with(&self, dir: &Path, extra: &[&str]) {
        fs::create_dir_all(dir).expect("repository directory");
        let dir_text = dir.to_str().expect("a UTF-8 scratch path");
        let mut args = vec!["init", "--template=", "-q", "-b", "main"];
        args.extend_from_slice(extra);
        args.push(dir_text);
        self.git(&self.parent, &args);
        self.quiet(dir);
    }

    /// [`QUIET`] into the config of the repository git finds from `cwd`.
    pub fn quiet(&self, cwd: &Path) {
        self.quiet_env(cwd, &[]);
    }

    /// [`QUIET`] into the config of the repository git finds from `cwd`
    /// with `extra` variables (a `GIT_DIR` of a separate git directory).
    pub fn quiet_env(&self, cwd: &Path, extra: &[(&str, &OsStr)]) {
        for (key, value) in QUIET {
            self.git_env(cwd, &["config", "--local", key, value], extra);
        }
    }

    /// `git init --template=` of `dir`.
    pub fn init(&self, dir: &Path) {
        self.init_with(dir, &[]);
    }

    /// `git add -A` in `dir`.
    pub fn add_all(&self, dir: &Path) {
        self.git(dir, &["add", "-A"]);
    }

    /// `git commit -q --no-verify -m message` in `dir`.
    pub fn commit(&self, dir: &Path, message: &str) {
        self.git(dir, &["commit", "-q", "--no-verify", "-m", message]);
    }

    /// `git rev-parse HEAD` in `dir`, or `None` when it is unborn.
    pub fn head(&self, dir: &Path) -> Option<String> {
        let output = self.git_output(dir, &["rev-parse", "--verify", "-q", "HEAD"], &[]);
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// The stage-0 blob OID of `path` (relative to `dir`).
    pub fn staged_oid(&self, dir: &Path, path: &str) -> String {
        let line = self.git_text(dir, &["ls-files", "-s", "--", path]);
        let mut fields = line.split_whitespace();
        let (_, oid) = (fields.next(), fields.next());
        oid.unwrap_or_else(|| panic!("{path} is not staged"))
            .to_owned()
    }

    /// The loose object file of `oid` in the repository at `dir`.
    pub fn loose_object(&self, dir: &Path, oid: &str) -> PathBuf {
        dir.join(".git")
            .join("objects")
            .join(&oid[..2])
            .join(&oid[2..])
    }
}

impl Sandbox {
    /// `git args` in `cwd` with `input` on stdin; it must succeed.
    pub fn git_stdin(&self, cwd: &Path, args: &[&str], input: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        let mut child = self
            .command(&self.git, cwd, &[])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input).expect("git's stdin");
        drop(stdin);
        let output = child.wait_with_output().expect("git ran");
        assert!(
            output.status.success(),
            "git {args:?} in {} failed ({}):\n{}",
            cwd.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
}

impl Sandbox {
    /// Writes `<dir>/git` (created when absent), a stand-in for a git
    /// built with translations, as gettext picks them: when the message
    /// locale (`LANGUAGE`, else `LC_ALL`, `LC_MESSAGES`, `LANG`) is German
    /// and `LC_ALL` is not `C` or `POSIX`, git's "not a git repository (or
    /// any ...)" line comes out in German; everything else, and every
    /// other run, is the real git's, exit status included. `dir`, put
    /// first on `PATH`, makes every `git` a process starts this one.
    /// macOS's git ships no translations; this shows what a translated
    /// one does.
    pub fn translating_git(&self, dir: &Path) -> PathBuf {
        fs::create_dir_all(dir).expect("the shim's directory");
        let real = self.git.to_str().expect("a UTF-8 git path");
        let stash = dir.to_str().expect("a UTF-8 scratch path");
        let script = format!(
            "#!/bin/sh\n\
             locale=\"${{LC_ALL:-${{LC_MESSAGES:-${{LANG-}}}}}}\"\n\
             case \"${{LC_ALL-}}\" in C|POSIX|C.*) exec '{real}' \"$@\";; esac\n\
             case \"${{LANGUAGE:-$locale}}\" in de*) ;; *) exec '{real}' \"$@\";; esac\n\
             err='{stash}'/stderr.$$\n\
             '{real}' \"$@\" 2>\"$err\"\n\
             code=$?\n\
             sed 's/^fatal: not a git repository (or any .*$/fatal: Kein \
             Git-Repository (oder irgendeines der Elternverzeichnisse): .git/' \"$err\" >&2\n\
             rm -f \"$err\"\n\
             exit $code\n"
        );
        let shim = dir.join("git");
        fs::write(&shim, script).expect("the shim");
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
        dir.to_path_buf()
    }

    /// `PATH` with `first` before the sandbox's own entries.
    pub fn path_with(&self, first: &Path) -> OsString {
        let mut dirs = vec![first.to_path_buf()];
        dirs.extend(std::env::split_paths(
            self.var("PATH").expect("the sandbox's PATH"),
        ));
        std::env::join_paths(dirs).expect("a PATH value")
    }
}
