//! The write side of one recorded worktree (task spec `proposal-apply`,
//! creation step 5, apply steps 2, 3, 5, 7–10): its git facts, the dirty
//! check of one path, `git merge-file`, `git diff --no-index`, `git commit
//! --only` with the caller's message, the trailer lookup on a branch, a
//! file's blob at a commit, and the atomic replace of one file. A decision
//! record (`docs/canon/decision-record.md` "Steps") or a create's new file:
//! a new file created without ever replacing one ([`create_file`]), its
//! intent-to-add entry
//! and its removal ([`WorktreeGit::intent_to_add`],
//! [`WorktreeGit::remove_cached`]), the name-status of a commit
//! ([`WorktreeGit::name_status`]).
//! `crate::git` stays the read side of `spec check`, resolving `GIT_*`
//! against the caller; here nothing of the caller's repository is used.
//!
//! **Environment**: git runs as `git -C <dir>` with the caller's variables
//! ([`GitEnv`]) minus every variable `git rev-parse --local-env-vars` lists
//! (and the names git 2.50 lists, should an older git list fewer):
//! `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`,
//! `GIT_COMMON_DIR`, `GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_COUNT`, …; with
//! `GIT_TERMINAL_PROMPT=0` and stdin null. Every command but `commit` also
//! gets `GIT_OPTIONAL_LOCKS=0`, so a read (`review`) writes nothing in the
//! worktree's git dir. `commit` runs the hooks (never `--no-verify`).
//!
//! **Paths** given to a command are relative to the handle's directory
//! (the worktree's top for an apply), passed as `:(literal)` pathspecs;
//! paths git prints are relative to the top. Temporary files (the merge's
//! three sides, the diff's two, the commit message) go to a caller's
//! scratch directory (the data directory), each created new and removed
//! after the command.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::git::{GitEnv, same_dir};
use crate::queue::Place;

/// The variables `git rev-parse --local-env-vars` lists in git 2.50,
/// dropped even when the running git lists fewer.
const LOCAL_ENV_VARS: [&str; 15] = [
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

/// The most bytes of git's stderr an error keeps.
const STDERR_MAX: usize = 4096;

/// The start of git's one message for "no repository found from here"
/// (`setup.c`: up to the file system root or a ceiling directory, or up to
/// a mount point), untranslated (`LC_ALL=C`). A broken `.git` file, a
/// dubious owner, a refused bare repository die with other words.
const NOT_A_REPOSITORY: &str = "fatal: not a git repository (or any ";

/// The state files of an operation in progress, by `git rev-parse
/// --git-path`.
const OPERATION_PATHS: [(&str, Operation); 7] = [
    ("MERGE_HEAD", Operation::Merge),
    ("rebase-merge", Operation::Rebase),
    ("rebase-apply", Operation::Rebase),
    ("CHERRY_PICK_HEAD", Operation::CherryPick),
    ("REVERT_HEAD", Operation::Revert),
    ("BISECT_LOG", Operation::Bisect),
    ("sequencer", Operation::Sequencer),
];

/// Names of scratch files, unique within the process.
static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);

/// Why a git command gave nothing usable.
#[derive(Debug)]
pub enum GitError {
    /// `git` could not be started.
    NotRunnable(io::Error),
    /// The command exited unsuccessfully (`code` `None`: killed by a
    /// signal); `stderr` is its text (its stdout when stderr is empty),
    /// lossy, trimmed, at most 4 KiB.
    Failed {
        command: &'static str,
        code: Option<i32>,
        stderr: String,
    },
    /// The output broke the expected form.
    Unreadable { command: &'static str },
    /// A path could not be resolved, or a scratch file written or removed.
    Io { path: PathBuf, source: io::Error },
}

impl GitError {
    fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunnable(error) if error.kind() == io::ErrorKind::NotFound => {
                f.write_str("`git` could not be run: no `git` program was found on PATH")
            }
            Self::NotRunnable(error) => write!(f, "`git` could not be run: {error}"),
            Self::Failed {
                command,
                code,
                stderr,
            } => {
                match code {
                    Some(code) => write!(f, "`git {command}` failed (exit status {code})")?,
                    None => write!(f, "`git {command}` was stopped by a signal")?,
                }
                if stderr.is_empty() {
                    Ok(())
                } else {
                    write!(f, ": {stderr}")
                }
            }
            Self::Unreadable { command } => {
                write!(f, "`git {command}` gave output that could not be read")
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotRunnable(error) | Self::Io { source: error, .. } => Some(error),
            _ => None,
        }
    }
}

/// An operation git has in progress in a worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
    /// A multi-commit cherry-pick or revert (`sequencer/`).
    Sequencer,
}

impl Operation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::CherryPick => "cherry-pick",
            Self::Revert => "revert",
            Self::Bisect => "bisect",
            Self::Sequencer => "sequencer",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a directory cannot be bound to a proposal.
#[derive(Debug)]
pub enum PlaceError {
    /// Git failed (no repository found from the directory, among others).
    Git(GitError),
    /// `HEAD` names no branch.
    Detached,
    /// `HEAD`'s branch has no commit yet.
    Unborn,
    /// A path is not UTF-8 and cannot be stored.
    NotUtf8 { path: PathBuf },
    /// The directory does not lie in the worktree git names.
    Outside { dir: PathBuf, top: PathBuf },
}

impl From<GitError> for PlaceError {
    fn from(error: GitError) -> Self {
        Self::Git(error)
    }
}

impl fmt::Display for PlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git(error) => error.fmt(f),
            Self::Detached => f.write_str("HEAD is detached: check out a branch first"),
            Self::Unborn => f.write_str("HEAD's branch has no commit yet: commit first"),
            Self::NotUtf8 { path } => write!(f, "{}: the path is not UTF-8", path.display()),
            Self::Outside { dir, top } => write!(
                f,
                "{} does not lie in the worktree {} git names for it",
                dir.display(),
                top.display()
            ),
        }
    }
}

impl std::error::Error for PlaceError {}

/// One worktree `git worktree list` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedWorktree {
    /// Its directory: canonical when it exists, else as git prints it
    /// (absolute; a pruned worktree's directory may be gone).
    pub path: PathBuf,
    /// The main entry of a bare repository: the repository itself, no
    /// files checked out.
    pub bare: bool,
}

/// A three-way merge's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge {
    /// No conflict: the merged bytes.
    Clean(Vec<u8>),
    /// `conflicts` conflicts (at most 127, git's cap): the text with
    /// conflict markers labelled `current`, `base`, `proposed`.
    Conflict { text: Vec<u8>, conflicts: i32 },
}

/// Git in one directory with the environment of this module.
#[derive(Debug, Clone)]
pub struct WorktreeGit {
    dir: PathBuf,
    vars: BTreeMap<OsString, OsString>,
}

impl WorktreeGit {
    /// Git in `dir` with `env`'s variables minus the local ones (`git
    /// rev-parse --local-env-vars`, run with no `GIT_*` variable, and
    /// [`LOCAL_ENV_VARS`]), `GIT_TERMINAL_PROMPT=0` set. `env`'s directory
    /// is not used.
    pub fn new(dir: impl Into<PathBuf>, env: &GitEnv) -> Result<Self, GitError> {
        let mut vars: BTreeMap<OsString, OsString> = env
            .vars()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        for name in local_env_vars(&vars)? {
            vars.remove(&name);
        }
        for name in LOCAL_ENV_VARS {
            vars.remove(OsStr::new(name));
        }
        vars.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        Ok(Self {
            dir: dir.into(),
            vars,
        })
    }

    /// The directory git runs in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The variables every child gets (`GIT_OPTIONAL_LOCKS` aside).
    pub fn vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(name, value)| (name.as_os_str(), value.as_os_str()))
    }

    fn command(&self, args: &[&OsStr], read_only: bool) -> Command {
        let mut command = Command::new("git");
        command
            .env_clear()
            .envs(&self.vars)
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if read_only {
            command.env("GIT_OPTIONAL_LOCKS", "0");
        }
        command
    }

    fn output(&self, args: &[&OsStr], read_only: bool) -> Result<Output, GitError> {
        self.command(args, read_only)
            .output()
            .map_err(GitError::NotRunnable)
    }

    /// A read-only command whose output must not depend on the user's or
    /// the system's git config (`diff`, `merge-file`):
    /// `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`.
    fn output_isolated(&self, args: &[&OsStr]) -> Result<Output, GitError> {
        self.command(args, true)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .map_err(GitError::NotRunnable)
    }

    /// Runs a read-only command to success; its stdout.
    fn read(&self, command: &'static str, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let output = self.output(&args, true)?;
        success(command, output)
    }

    /// One path git printed, relative to the handle's directory or
    /// absolute, made absolute and canonical.
    fn path_of(&self, command: &'static str, output: &[u8]) -> Result<PathBuf, GitError> {
        let printed = one_line(output).ok_or(GitError::Unreadable { command })?;
        let path = self
            .dir
            .join(os_path(printed).ok_or(GitError::Unreadable { command })?);
        fs::canonicalize(&path).map_err(|error| GitError::io(path, error))
    }

    /// The canonical top of the worktree holding the directory (`rev-parse
    /// --show-toplevel`).
    pub fn top(&self) -> Result<PathBuf, GitError> {
        let output = self.read("rev-parse", &["rev-parse", "--show-toplevel"])?;
        self.path_of("rev-parse", &output)
    }

    /// The repository's canonical git common dir (`rev-parse
    /// --git-common-dir`): one for every linked worktree.
    pub fn common_dir(&self) -> Result<PathBuf, GitError> {
        let output = self.read("rev-parse", &["rev-parse", "--git-common-dir"])?;
        self.path_of("rev-parse", &output)
    }

    /// [`Self::top`], but `None` when git finds no repository from the
    /// directory: the one failure read so, by git's own message under
    /// `LC_ALL=C` ([`NOT_A_REPOSITORY`], exit 128). Any other failure — a
    /// dubious owner, a broken `.git` file, the directory inside a git dir,
    /// no `git` to run — is an error.
    pub fn top_if_repository(&self) -> Result<Option<PathBuf>, GitError> {
        const COMMAND: &str = "rev-parse";
        let args = [OsStr::new("rev-parse"), OsStr::new("--show-toplevel")];
        let output = self
            .command(&args, true)
            .env("LC_ALL", "C")
            .env_remove("LANGUAGE")
            .output()
            .map_err(GitError::NotRunnable)?;
        if output.status.success() {
            return self.path_of(COMMAND, &output.stdout).map(Some);
        }
        let no_repository = output.status.code() == Some(128)
            && String::from_utf8_lossy(&output.stderr)
                .lines()
                .any(|line| line.starts_with(NOT_A_REPOSITORY));
        if no_repository {
            Ok(None)
        } else {
            Err(failed(COMMAND, &output))
        }
    }

    /// Every worktree of the repository (`worktree list --porcelain -z`,
    /// git 2.36 or later): the main one first (for a bare repository, the
    /// repository itself, `bare`), then the linked ones, a pruned one
    /// included.
    pub fn worktrees(&self) -> Result<Vec<ListedWorktree>, GitError> {
        const COMMAND: &str = "worktree";
        let unreadable = || GitError::Unreadable { command: COMMAND };
        let output = self.read(COMMAND, &["worktree", "list", "--porcelain", "-z"])?;
        let mut listed: Vec<ListedWorktree> = Vec::new();
        for field in output.split(|&byte| byte == 0) {
            if let Some(path) = field.strip_prefix(b"worktree ") {
                let path = self.dir.join(os_path(path).ok_or_else(unreadable)?);
                listed.push(ListedWorktree {
                    path: fs::canonicalize(&path).unwrap_or(path),
                    bare: false,
                });
            } else if field == b"bare" {
                listed.last_mut().ok_or_else(unreadable)?.bare = true;
            } else if !field.is_empty() && listed.is_empty() {
                return Err(unreadable());
            }
        }
        if listed.is_empty() {
            return Err(unreadable());
        }
        Ok(listed)
    }

    /// The branch `HEAD` names (`symbolic-ref -q HEAD`, `refs/heads/`
    /// dropped; another ref whole); `None` when detached.
    pub fn branch(&self) -> Result<Option<String>, GitError> {
        const COMMAND: &str = "symbolic-ref";
        let output = self.output(
            &[
                OsStr::new("symbolic-ref"),
                OsStr::new("-q"),
                OsStr::new("HEAD"),
            ],
            true,
        )?;
        match output.status.code() {
            Some(0) => {
                let name = one_line(&output.stdout)
                    .and_then(|line| std::str::from_utf8(line).ok())
                    .ok_or(GitError::Unreadable { command: COMMAND })?;
                Ok(Some(
                    name.strip_prefix("refs/heads/").unwrap_or(name).to_owned(),
                ))
            }
            Some(1) => Ok(None),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// `HEAD`'s commit, hex (`rev-parse --verify -q HEAD^{commit}`);
    /// `None` when unborn.
    pub fn head(&self) -> Result<Option<String>, GitError> {
        self.commit_of("HEAD")
    }

    /// The commit `revision` names, hex; `None` when it names none
    /// (`rev-parse --verify -q --end-of-options <revision>^{commit}` exits
    /// 1). Every revision this module passes follows `--end-of-options`: a
    /// stored value starting with `-` is never an option.
    pub fn commit_of(&self, revision: &str) -> Result<Option<String>, GitError> {
        const COMMAND: &str = "rev-parse";
        let spec = format!("{revision}^{{commit}}");
        let output = self.output(
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("-q"),
                OsStr::new("--end-of-options"),
                OsStr::new(&spec),
            ],
            true,
        )?;
        match output.status.code() {
            Some(0) => one_line(&output.stdout)
                .filter(|oid| is_oid(oid))
                .and_then(|oid| String::from_utf8(oid.to_vec()).ok())
                .map(Some)
                .ok_or(GitError::Unreadable { command: COMMAND }),
            Some(1) => Ok(None),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// The first operation in progress in the worktree (merge, rebase,
    /// cherry-pick, revert, bisect, sequencer), by its state file under
    /// `rev-parse --git-path`; `None` when there is none.
    pub fn operation_in_progress(&self) -> Result<Option<Operation>, GitError> {
        for (name, operation) in OPERATION_PATHS {
            let output = self.read("rev-parse", &["rev-parse", "--git-path", name])?;
            let printed = one_line(&output).ok_or(GitError::Unreadable {
                command: "rev-parse",
            })?;
            let path = self.dir.join(os_path(printed).ok_or(GitError::Unreadable {
                command: "rev-parse",
            })?);
            match fs::symlink_metadata(&path) {
                Ok(_) => return Ok(Some(operation)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(GitError::io(path, error)),
            }
        }
        Ok(None)
    }

    /// The binding of a proposal raised in `root` (the project root, in
    /// this handle's directory or below it): the worktree's canonical top,
    /// `root`'s place in it, the common dir, the branch and `HEAD`.
    pub fn place(&self, root: &Path) -> Result<Place, PlaceError> {
        let root = fs::canonicalize(root).map_err(|error| GitError::io(root, error))?;
        let top = self.top()?;
        let Ok(inner) = root.strip_prefix(&top) else {
            return Err(PlaceError::Outside { dir: root, top });
        };
        let mut parts = Vec::new();
        for component in inner.components() {
            match component {
                Component::Normal(part) => parts.push(
                    part.to_str()
                        .ok_or_else(|| PlaceError::NotUtf8 { path: root.clone() })?
                        .to_owned(),
                ),
                _ => return Err(PlaceError::Outside { dir: root, top }),
            }
        }
        let common = self.common_dir()?;
        let branch = self.branch()?.ok_or(PlaceError::Detached)?;
        let head = self.head()?.ok_or(PlaceError::Unborn)?;
        let utf8 = |path: &Path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| PlaceError::NotUtf8 {
                    path: path.to_path_buf(),
                })
        };
        Ok(Place {
            git_common_dir: utf8(&common)?,
            worktree: utf8(&top)?,
            root_rel: parts.join("/"),
            branch,
            base_commit: head,
        })
    }

    /// `path` is in the index (`ls-files -z -- :(literal)<path>` lists
    /// exactly it).
    pub fn is_tracked(&self, path: &str) -> Result<bool, GitError> {
        let spec = literal(path);
        let output = self.read("ls-files", &["ls-files", "-z", "--", &spec])?;
        Ok(output
            .split(|&byte| byte == 0)
            .any(|entry| entry == path.as_bytes()))
    }

    /// `path`'s index entry is an intent-to-add one (`git add
    /// --intent-to-add`): `status --porcelain=v2 -z --untracked-files=no --
    /// :(literal)<path>` gives it the `.A` status. No entry: `false`.
    pub fn is_intent_to_add(&self, path: &str) -> Result<bool, GitError> {
        let spec = literal(path);
        let output = self.read(
            "status",
            &[
                "status",
                "--porcelain=v2",
                "-z",
                "--untracked-files=no",
                "--",
                &spec,
            ],
        )?;
        let mut records = output.split(|&byte| byte == 0);
        while let Some(record) = records.next() {
            // `2 …` (a rename or copy) is followed by its original path.
            if record.starts_with(b"2 ") {
                records.next();
                continue;
            }
            // `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`.
            let fields: Vec<&[u8]> = record.splitn(9, |&byte| byte == b' ').collect();
            if let [b"1", b".A", .., named] = fields.as_slice()
                && fields.len() == 9
                && *named == path.as_bytes()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `path` differs from `HEAD` in the index or the working tree, or is
    /// untracked (`status --porcelain=v1 -z --untracked-files=all --
    /// :(literal)<path>` prints anything).
    pub fn is_dirty(&self, path: &str) -> Result<bool, GitError> {
        let spec = literal(path);
        let output = self.read(
            "status",
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--",
                &spec,
            ],
        )?;
        Ok(!output.is_empty())
    }

    /// The committer identity without its date (`var
    /// GIT_COMMITTER_IDENT`): `Name <email>`. No identity: git fails.
    pub fn committer_ident(&self) -> Result<String, GitError> {
        const COMMAND: &str = "var";
        let output = self.read(COMMAND, &["var", "GIT_COMMITTER_IDENT"])?;
        let line = one_line(&output)
            .and_then(|line| std::str::from_utf8(line).ok())
            .ok_or(GitError::Unreadable { command: COMMAND })?;
        // `Name <email> <seconds> <zone>`: the two last fields dropped.
        let mut fields = line.rsplitn(3, ' ');
        let (_zone, _seconds, ident) = (fields.next(), fields.next(), fields.next());
        ident
            .filter(|ident| ident.ends_with('>'))
            .map(str::to_owned)
            .ok_or(GitError::Unreadable { command: COMMAND })
    }

    /// `git merge-file -p -L current -L base -L proposed` over three
    /// scratch files in `scratch`, neither the global nor the system git
    /// config read.
    pub fn merge_file(
        &self,
        scratch: &Path,
        current: &[u8],
        base: &[u8],
        proposed: &[u8],
    ) -> Result<Merge, GitError> {
        const COMMAND: &str = "merge-file";
        let mut files = Scratch::default();
        let current = files.write(scratch, "current", current)?;
        let base = files.write(scratch, "base", base)?;
        let proposed = files.write(scratch, "proposed", proposed)?;
        let args: [&OsStr; 11] = [
            OsStr::new("merge-file"),
            OsStr::new("-p"),
            OsStr::new("-L"),
            OsStr::new("current"),
            OsStr::new("-L"),
            OsStr::new("base"),
            OsStr::new("-L"),
            OsStr::new("proposed"),
            current.as_os_str(),
            base.as_os_str(),
            proposed.as_os_str(),
        ];
        let output = self.output_isolated(&args)?;
        drop(files);
        match output.status.code() {
            Some(0) => Ok(Merge::Clean(output.stdout)),
            Some(conflicts @ 1..=127) => Ok(Merge::Conflict {
                text: output.stdout,
                conflicts,
            }),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// The hunks of `git diff --no-index --no-color --no-ext-diff
    /// --diff-algorithm=myers -U3` from `old` to `new` (scratch files in
    /// `scratch`, neither the global nor the system git config read): every
    /// line from the first `@@` on, the headers dropped; empty when they are
    /// equal.
    pub fn diff_hunks(&self, scratch: &Path, old: &[u8], new: &[u8]) -> Result<Vec<u8>, GitError> {
        const COMMAND: &str = "diff";
        let mut files = Scratch::default();
        let old = files.write(scratch, "base", old)?;
        let new = files.write(scratch, "proposed", new)?;
        let args: [&OsStr; 9] = [
            OsStr::new("diff"),
            OsStr::new("--no-index"),
            OsStr::new("--no-color"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--diff-algorithm=myers"),
            OsStr::new("-U3"),
            OsStr::new("--"),
            old.as_os_str(),
            new.as_os_str(),
        ];
        let output = self.output_isolated(&args)?;
        drop(files);
        match output.status.code() {
            Some(0) => Ok(Vec::new()),
            Some(1) => {
                let text = output.stdout;
                let mut at = 0;
                while at < text.len() && !text[at..].starts_with(b"@@") {
                    match text[at..].iter().position(|&byte| byte == b'\n') {
                        Some(end) => at += end + 1,
                        None => at = text.len(),
                    }
                }
                Ok(text[at..].to_vec())
            }
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// `git commit --only --cleanup=verbatim -F <scratch file> --
    /// :(literal)<path>`: commits exactly `path`'s working-tree bytes, other
    /// staged paths left staged; the hooks run. A failure carries git's
    /// stderr (a hook's output, `index.lock`).
    pub fn commit_only(&self, scratch: &Path, message: &str, path: &str) -> Result<(), GitError> {
        const COMMAND: &str = "commit";
        let mut files = Scratch::default();
        let message_file = files.write(scratch, "message", message.as_bytes())?;
        let spec = literal(path);
        let args: [&OsStr; 7] = [
            OsStr::new("commit"),
            OsStr::new("--only"),
            OsStr::new("--cleanup=verbatim"),
            OsStr::new("-F"),
            message_file.as_os_str(),
            OsStr::new("--"),
            OsStr::new(&spec),
        ];
        let output = self.output(&args, false);
        // Removed whatever happened: a leftover scratch file never turns a
        // made commit into a failure.
        drop(files);
        success(COMMAND, output?).map(|_| ())
    }

    /// `git add --intent-to-add -- :(literal)<path>`: a new file's entry in
    /// the index, so `git commit --only` takes it. A failure carries git's
    /// stderr (an ignored path, `index.lock`).
    pub fn intent_to_add(&self, path: &str) -> Result<(), GitError> {
        const COMMAND: &str = "add";
        let spec = literal(path);
        let args = [
            OsStr::new("add"),
            OsStr::new("--intent-to-add"),
            OsStr::new("--"),
            OsStr::new(&spec),
        ];
        success(COMMAND, self.output(&args, false)?).map(|_| ())
    }

    /// `git rm --cached --quiet --ignore-unmatch -- :(literal)<path>`: the
    /// path's index entry gone, its file untouched; none is no failure.
    pub fn remove_cached(&self, path: &str) -> Result<(), GitError> {
        const COMMAND: &str = "rm";
        let spec = literal(path);
        let args = [
            OsStr::new("rm"),
            OsStr::new("--cached"),
            OsStr::new("--quiet"),
            OsStr::new("--ignore-unmatch"),
            OsStr::new("--"),
            OsStr::new(&spec),
        ];
        success(COMMAND, self.output(&args, false)?).map(|_| ())
    }

    /// The paths that differ between the commits `from` and `to` with
    /// their status letter (`diff-tree -r -z --name-status --no-renames
    /// --no-commit-id --end-of-options`), top-relative, in git's order; a
    /// name that is not UTF-8 lossily.
    pub fn name_status(&self, from: &str, to: &str) -> Result<Vec<ChangedPath>, GitError> {
        const COMMAND: &str = "diff-tree";
        let output = self.read(
            COMMAND,
            &[
                "diff-tree",
                "-r",
                "-z",
                "--name-status",
                "--no-renames",
                "--no-commit-id",
                "--end-of-options",
                from,
                to,
            ],
        )?;
        let mut fields = output
            .split(|&byte| byte == 0)
            .filter(|entry| !entry.is_empty());
        let mut changed = Vec::new();
        while let Some(status) = fields.next() {
            let path = fields
                .next()
                .ok_or(GitError::Unreadable { command: COMMAND })?;
            changed.push(ChangedPath {
                status: String::from_utf8_lossy(status).into_owned(),
                path: String::from_utf8_lossy(path).into_owned(),
            });
        }
        Ok(changed)
    }

    /// The parents of `commit`, hex, in order (`rev-list --parents -n 1
    /// --end-of-options <commit>`).
    pub fn parents(&self, commit: &str) -> Result<Vec<String>, GitError> {
        const COMMAND: &str = "rev-list";
        let output = self.read(
            COMMAND,
            &[
                "rev-list",
                "--parents",
                "-n",
                "1",
                "--end-of-options",
                commit,
            ],
        )?;
        let line = one_line(&output)
            .and_then(|line| std::str::from_utf8(line).ok())
            .ok_or(GitError::Unreadable { command: COMMAND })?;
        let mut oids = line.split(' ');
        if oids.next().is_none_or(|first| !is_oid(first.as_bytes())) {
            return Err(GitError::Unreadable { command: COMMAND });
        }
        oids.map(|oid| {
            is_oid(oid.as_bytes())
                .then(|| oid.to_owned())
                .ok_or(GitError::Unreadable { command: COMMAND })
        })
        .collect()
    }

    /// The paths that differ between the commits `from` and `to`
    /// (`diff-tree -r -z --name-only --no-renames --no-commit-id
    /// --end-of-options`), top-relative, in git's order; a name that is not
    /// UTF-8 lossily.
    pub fn changed_paths(&self, from: &str, to: &str) -> Result<Vec<String>, GitError> {
        let output = self.read(
            "diff-tree",
            &[
                "diff-tree",
                "-r",
                "-z",
                "--name-only",
                "--no-renames",
                "--no-commit-id",
                "--end-of-options",
                from,
                to,
            ],
        )?;
        Ok(output
            .split(|&byte| byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| String::from_utf8_lossy(entry).into_owned())
            .collect())
    }

    /// The bytes of the top-relative `path` in `commit`'s tree as stored
    /// (`cat-file --end-of-options blob <commit>:<path>`): no filter, no
    /// end-of-line conversion. A path git would resolve against the
    /// directory (empty, or starting with `./` or `../`) is refused unread
    /// ([`GitError::Unreadable`]).
    pub fn blob_at(&self, commit: &str, path: &str) -> Result<Vec<u8>, GitError> {
        const COMMAND: &str = "cat-file";
        if path.is_empty() || path.starts_with("./") || path.starts_with("../") {
            return Err(GitError::Unreadable { command: COMMAND });
        }
        let object = format!("{commit}:{path}");
        self.read(COMMAND, &["cat-file", "--end-of-options", "blob", &object])
    }

    /// Whether `commit`'s tree holds the top-relative `path`, a file or a
    /// directory (`rev-parse --verify -q --end-of-options <commit>:<path>`
    /// exits 0; 1: it does not, or `commit` names nothing). A path
    /// [`Self::blob_at`] refuses is refused alike, unread.
    pub fn has_path(&self, commit: &str, path: &str) -> Result<bool, GitError> {
        const COMMAND: &str = "rev-parse";
        if path.is_empty() || path.starts_with("./") || path.starts_with("../") {
            return Err(GitError::Unreadable { command: COMMAND });
        }
        let object = format!("{commit}:{path}");
        let output = self.output(
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("-q"),
                OsStr::new("--end-of-options"),
                OsStr::new(&object),
            ],
            true,
        )?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// The values of `key`'s trailers in `commit`'s message, in order
    /// (`log -1 --format=%(trailers:key=<key>,valueonly) --end-of-options
    /// <commit> --`).
    pub fn trailer_values(&self, commit: &str, key: &str) -> Result<Vec<String>, GitError> {
        let format = format!("--format=%(trailers:key={key},valueonly,separator=%x1e)");
        let output = self.read(
            "log",
            &[
                "log",
                "-1",
                "--no-show-signature",
                &format,
                "--end-of-options",
                commit,
                "--",
            ],
        )?;
        let text = String::from_utf8_lossy(&output);
        Ok(values(text.trim_end_matches('\n')))
    }

    /// The commits in `from..refs/heads/<branch>`, newest first, whose
    /// message has a `key` trailer equal to `value`: an interrupted
    /// apply's commit, found by its `Proposal:` trailer.
    pub fn commits_with_trailer(
        &self,
        from: &str,
        branch: &str,
        key: &str,
        value: &str,
    ) -> Result<Vec<String>, GitError> {
        self.trailer_log(&format!("{from}..refs/heads/{branch}"), key, value)
    }

    /// The commits of `refs/heads/<branch>`'s whole history, newest first,
    /// whose message has a `key` trailer equal to `value`: the read of
    /// [`Self::commits_with_trailer`] when its `from` is not in the
    /// repository (pruned).
    pub fn branch_commits_with_trailer(
        &self,
        branch: &str,
        key: &str,
        value: &str,
    ) -> Result<Vec<String>, GitError> {
        self.trailer_log(&format!("refs/heads/{branch}"), key, value)
    }

    /// `log -z` of `range`: the commits whose `key` trailer is `value`.
    fn trailer_log(&self, range: &str, key: &str, value: &str) -> Result<Vec<String>, GitError> {
        let format = format!("--format=%H%x1f%(trailers:key={key},valueonly,separator=%x1e)");
        let output = self.read(
            "log",
            &[
                "log",
                "-z",
                "--no-show-signature",
                &format,
                "--end-of-options",
                range,
                "--",
            ],
        )?;
        let mut found = Vec::new();
        for record in output.split(|&byte| byte == 0) {
            let record = String::from_utf8_lossy(record);
            let record = record.trim_start_matches('\n');
            if record.is_empty() {
                continue;
            }
            let Some((commit, trailers)) = record.split_once('\u{1f}') else {
                return Err(GitError::Unreadable { command: "log" });
            };
            if !is_oid(commit.as_bytes()) {
                return Err(GitError::Unreadable { command: "log" });
            }
            if values(trailers.trim_end_matches('\n'))
                .iter()
                .any(|found| found == value)
            {
                found.push(commit.to_owned());
            }
        }
        Ok(found)
    }
}

/// `recorded` (a stored canonical common dir) and `current` name one
/// repository: equal once canonical, or one inode of one device.
pub fn same_repository(recorded: &str, current: &Path) -> bool {
    same_dir(Path::new(recorded), current)
}

/// Replaces the regular file `path` with `bytes` atomically: a new sibling
/// (`.<name>.specengine-<pid>-<n>.tmp`, a dot-name the walk skips) created
/// with `path`'s mode, written and synced, given `path`'s permissions,
/// renamed over it. `path` itself
/// must be a regular file, not a symlink; on any failure the sibling is
/// removed and `path` is unchanged.
pub fn replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the path names no file",
        ));
    };
    let (temp, mut file) = loop {
        let mut candidate = OsString::from(".");
        candidate.push(name);
        candidate.push(format!(
            ".specengine-{}-{}.tmp",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let temp = dir.join(candidate);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
            options.mode(meta.permissions().mode() & 0o7777);
        }
        match options.open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    };
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::set_permissions(&temp, meta.permissions()))
        .and_then(|()| fs::rename(&temp, path));
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    // The rename made durable where the platform allows; best effort.
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// One path a commit changed ([`WorktreeGit::name_status`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedPath {
    /// Git's status letter: `A` added, `M` modified, `D` deleted, `T` its
    /// type changed…
    pub status: String,
    /// Top-relative.
    pub path: String,
}

impl ChangedPath {
    /// Git's status letter of a path a commit adds.
    const ADDED: char = 'A';

    /// The commit adds `path`.
    pub fn adds(&self, path: &str) -> bool {
        self.path == path && self.status.chars().eq([Self::ADDED])
    }
}

impl fmt::Display for ChangedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status, self.path)
    }
}

/// Why [`create_file`] wrote nothing.
#[derive(Debug)]
pub enum CreateFileError {
    /// Something is at the path already (a dangling symlink too), or
    /// appeared there while the file was written: left untouched.
    Exists,
    /// An existing component of the path is a symlink (named, relative).
    Symlink(String),
    /// An existing component of the directory is not a directory (named).
    NotDirectory(String),
    /// The path names no file under the root.
    BadPath,
    /// The file system refused (what was being done, the error).
    Io(String, io::Error),
}

impl fmt::Display for CreateFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exists => f.write_str("something is at the path"),
            Self::Symlink(component) => write!(f, "`{component}` is a symlink"),
            Self::NotDirectory(component) => write!(f, "`{component}` is not a directory"),
            Self::BadPath => f.write_str("the path names no file under the root"),
            Self::Io(what, error) => write!(f, "{what}: {error}"),
        }
    }
}

impl std::error::Error for CreateFileError {}

/// A file [`create_file`] made, and the directories it made for it,
/// outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedFile {
    pub path: PathBuf,
    pub made_dirs: Vec<PathBuf>,
}

impl CreatedFile {
    /// The file and the directories made for it removed (innermost first,
    /// only while empty); what could not be removed, one line each.
    pub fn remove(&self) -> Vec<String> {
        let mut left = Vec::new();
        match fs::remove_file(&self.path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                left.push(format!("cannot remove {}: {error}", self.path.display()));
            }
            _ => {}
        }
        left.extend(remove_dirs(&self.made_dirs));
        left
    }
}

/// `dirs` (outermost first) removed innermost first, only while empty;
/// what stays, one line each.
fn remove_dirs(dirs: &[PathBuf]) -> Vec<String> {
    let mut left = Vec::new();
    for dir in dirs.iter().rev() {
        if let Err(error) = fs::remove_dir(dir) {
            left.push(format!(
                "the directory {} made for it stays: {error}",
                dir.display()
            ));
        }
    }
    left
}

/// Creates the new file `relative` (`/`-separated, clean) under the
/// directory `root` holding `bytes`, never replacing anything: every
/// existing component of its directory a directory and no symlink, missing
/// ones made one by one (each checked again once made); nothing at the
/// path, a dangling symlink included. The bytes go to a new sibling
/// (`.<name>.specengine-<pid>-<n>.tmp`, a dot-name the walk skips),
/// synced, hard-linked to the path (`link` never replaces a name: one
/// appearing meanwhile is [`CreateFileError::Exists`], untouched), the
/// sibling removed and the directory synced (best effort). On any failure
/// the sibling and the directories made are removed (innermost first, only
/// while empty) and nothing at the path is touched.
pub fn create_file(
    root: &Path,
    relative: &str,
    bytes: &[u8],
) -> Result<CreatedFile, CreateFileError> {
    let components: Vec<&str> = relative.split('/').collect();
    let Some((name, parents)) = components.split_last() else {
        return Err(CreateFileError::BadPath);
    };
    if components
        .iter()
        .any(|component| matches!(*component, "" | "." | ".."))
    {
        return Err(CreateFileError::BadPath);
    }
    let mut made = Vec::new();
    let failed = |error: CreateFileError, made: &[PathBuf]| {
        remove_dirs(made);
        error
    };
    let mut dir = root.to_path_buf();
    for (index, component) in parents.iter().enumerate() {
        dir.push(component);
        let shown = || components[..=index].join("/");
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(failed(CreateFileError::Symlink(shown()), &made));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(failed(CreateFileError::NotDirectory(shown()), &made)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&dir) {
                    Ok(()) => made.push(dir.clone()),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        let what = format!("cannot create the directory {}", dir.display());
                        return Err(failed(CreateFileError::Io(what, error), &made));
                    }
                }
                // Made here or meanwhile: a directory, never a symlink.
                match fs::symlink_metadata(&dir) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        return Err(failed(CreateFileError::Symlink(shown()), &made));
                    }
                    Ok(meta) if meta.is_dir() => {}
                    _ => return Err(failed(CreateFileError::NotDirectory(shown()), &made)),
                }
            }
            Err(error) => {
                let what = format!("cannot read {}", dir.display());
                return Err(failed(CreateFileError::Io(what, error), &made));
            }
        }
    }
    let path = dir.join(name);
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err(failed(CreateFileError::Exists, &made)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            let what = format!("cannot read {}", path.display());
            return Err(failed(CreateFileError::Io(what, error), &made));
        }
    }
    let (temp, mut file) = loop {
        let candidate = dir.join(format!(
            ".{name}.specengine-{}-{}.tmp",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                let what = format!("cannot create {}", candidate.display());
                return Err(failed(CreateFileError::Io(what, error), &made));
            }
        }
    };
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let placed = match written {
        Err(error) => Err(CreateFileError::Io(
            format!("cannot write {}", temp.display()),
            error,
        )),
        Ok(()) => match fs::hard_link(&temp, &path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                Err(CreateFileError::Exists)
            }
            Err(_) if fs::symlink_metadata(&path).is_ok() => Err(CreateFileError::Exists),
            Err(error) => Err(CreateFileError::Io(
                format!("cannot link {} to {}", temp.display(), path.display()),
                error,
            )),
        },
    };
    let _ = fs::remove_file(&temp);
    if let Err(error) = placed {
        return Err(failed(error, &made));
    }
    // The new name made durable where the platform allows; best effort.
    if let Ok(dir) = fs::File::open(&dir) {
        let _ = dir.sync_all();
    }
    Ok(CreatedFile {
        path,
        made_dirs: made,
    })
}

/// Scratch files of one command, removed when it is dropped (best effort:
/// the command's own result decides).
#[derive(Default)]
struct Scratch {
    paths: Vec<PathBuf>,
}

impl Scratch {
    /// A new file `specengine-<pid>-<n>-<label>` in `dir` holding `bytes`;
    /// its absolute path.
    fn write(&mut self, dir: &Path, label: &str, bytes: &[u8]) -> Result<PathBuf, GitError> {
        let dir = std::path::absolute(dir).map_err(|error| GitError::io(dir, error))?;
        loop {
            let path = dir.join(format!(
                "specengine-{}-{}-{label}",
                std::process::id(),
                SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    self.paths.push(path.clone());
                    file.write_all(bytes)
                        .map_err(|error| GitError::io(&path, error))?;
                    return Ok(path);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(GitError::io(path, error)),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

/// The names `git rev-parse --local-env-vars` prints, run with `vars` minus
/// every `GIT_*` variable (the list is git's own, never the caller's).
fn local_env_vars(vars: &BTreeMap<OsString, OsString>) -> Result<BTreeSet<OsString>, GitError> {
    let output = Command::new("git")
        .env_clear()
        .envs(
            vars.iter()
                .filter(|(name, _)| !name.as_encoded_bytes().starts_with(b"GIT_")),
        )
        .args(["rev-parse", "--local-env-vars"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(GitError::NotRunnable)?;
    let stdout = success("rev-parse", output)?;
    let text = std::str::from_utf8(&stdout).map_err(|_| GitError::Unreadable {
        command: "rev-parse",
    })?;
    Ok(text
        .lines()
        .filter(|name| !name.is_empty())
        .map(OsString::from)
        .collect())
}

/// `output`'s stdout when it succeeded, else [`GitError::Failed`].
fn success(command: &'static str, output: Output) -> Result<Vec<u8>, GitError> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(failed(command, &output))
    }
}

/// [`GitError::Failed`] of `output`: its stderr, or its stdout when stderr
/// is empty (`git commit` says "nothing to commit" on stdout).
fn failed(command: &'static str, output: &Output) -> GitError {
    let mut stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        stderr = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    }
    if stderr.len() > STDERR_MAX {
        let mut end = STDERR_MAX;
        while !stderr.is_char_boundary(end) {
            end -= 1;
        }
        stderr.truncate(end);
        stderr.push('…');
    }
    GitError::Failed {
        command,
        code: output.status.code(),
        stderr,
    }
}

/// The one line of `output`: its trailing LF dropped; `None` when empty.
fn one_line(output: &[u8]) -> Option<&[u8]> {
    let line = output.strip_suffix(b"\n").unwrap_or(output);
    (!line.is_empty()).then_some(line)
}

/// `bytes` as a path: any bytes on Unix, UTF-8 elsewhere.
#[cfg(unix)]
fn os_path(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    Some(PathBuf::from(OsStr::from_bytes(bytes)))
}

/// `bytes` as a path: any bytes on Unix, UTF-8 elsewhere.
#[cfg(not(unix))]
fn os_path(bytes: &[u8]) -> Option<PathBuf> {
    std::str::from_utf8(bytes).ok().map(PathBuf::from)
}

/// A SHA-1 or SHA-256 object name in lower-case hex.
pub(crate) fn is_oid(oid: &[u8]) -> bool {
    matches!(oid.len(), 40 | 64)
        && oid
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

/// `path` as a literal pathspec (no glob, no magic).
fn literal(path: &str) -> String {
    format!(":(literal){path}")
}

/// Trailer values separated by U+001E, each trimmed, empty ones dropped.
fn values(text: &str) -> Vec<String> {
    text.split('\u{1e}')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
