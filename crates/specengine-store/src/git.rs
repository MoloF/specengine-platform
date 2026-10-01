//! Git plumbing for `spec check --staged` (docs/features/spec-cli-staged.md,
//! docs/features/spec-cli-introduced.md): the index git would commit and
//! `HEAD`'s tree, its base, read through `std::process` and never written.
//! Git runs in the root, so git computes the prefix and every path comes
//! root-relative; nothing strips the root against `--show-toplevel`.
//!
//! Commands, and nothing else: `rev-parse --git-path index`,
//! `--git-path objects`, `--git-common-dir` in the caller's directory, one
//! per relative `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR`
//! set; there too `rev-parse --show-toplevel`, then, the top being the
//! caller's directory, `rev-parse --absolute-git-dir`, both without
//! `GIT_DIR` and `GIT_CEILING_DIRECTORIES` and across file systems, only
//! when `GIT_DIR` is set and `GIT_WORK_TREE` is not (a top other than the
//! caller's directory, or a git dir other than `GIT_DIR`'s, is refused);
//! in the root
//! `rev-parse --is-inside-work-tree`, `ls-files -s -z`, `rev-parse --verify
//! -q HEAD` (exit 0 and one OID: born; exit 1 and nothing: unborn), when
//! born `ls-tree -r -z <oid>` (no `--full-tree`, no `--full-name`: the
//! root's subtree, paths root-relative as `ls-files` gives them, nothing
//! for a root `HEAD` lacks; the OID pins both calls to one commit),
//! one `cat-file --batch` session per check, and the intent-to-add detector
//! `diff-files -z --name-only --no-renames --ignore-submodules=all
//! --diff-filter=A --ita-invisible-in-index --relative` (plumbing: no
//! index refresh, no rename pairing, no submodule probed; paths
//! root-relative and under the root; run only when a stage-0 entry holds
//! the empty blob, the one an intent-to-add entry holds, at a `.md` path or
//! at the root's config or baseline: it runs clean filters on racily-clean
//! entries). An intent-to-add entry whose file is gone from the working
//! tree is not reported by it and is walked as an empty file (the spec's
//! fallback). Any other intent-to-add entry, neither `.md` nor TOML
//! (`git add -N docs/.gitkeep`), stays an empty regular file only when the
//! detector does not run; when an empty `.md`, config or baseline starts
//! it, the entry is dropped.
//!
//! Every child: `-c core.fsmonitor=false`, the variables of [`GitEnv`]
//! with the path variables made absolute and `GIT_OPTIONAL_LOCKS=0`,
//! `GIT_NO_LAZY_FETCH=1`, `GIT_NO_REPLACE_OBJECTS=1`,
//! `GIT_TERMINAL_PROMPT=0`; stdin null but `cat-file`'s request pipe;
//! stderr captured (`cat-file`: discarded) and never relayed. A failure is
//! a cause with a fixed one-line message at `.`: never git's text, never an
//! absolute path.
//!
//! `cat-file --batch` runs in strict lockstep, without `--buffer`: one
//! request is written and its whole reply read before the next, so neither
//! side ever waits on a full pipe (no deadlock whatever the blob sizes).
//! Each OID is requested once per check: [`Staged::blob`] keeps what it
//! read, for the index and `HEAD` alike.

use std::collections::{BTreeMap, BTreeSet, btree_map};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use specengine_core::DOCUMENT_EXTENSION;
use specengine_core::check::Cause;

use crate::check::{BASELINE_FILE, CONFIG_FILE};

/// The path variables git reads against the caller's directory: a relative
/// value is made absolute against [`GitEnv::cwd`] before git runs in the
/// root (linked worktrees may export `GIT_DIR`).
const CWD_RELATIVE: [&str; 2] = ["GIT_DIR", "GIT_WORK_TREE"];

/// The path variables git reads against the working tree's top, after its
/// own chdir (hooks export `GIT_INDEX_FILE=.git/index`, `.git/index.lock`,
/// relative to the top), each with the `rev-parse` arguments printing the
/// path git reads, relative to the directory it runs in.
const TOP_RELATIVE: [(&str, &[&str]); 3] = [
    ("GIT_INDEX_FILE", &["rev-parse", "--git-path", "index"]),
    (
        "GIT_OBJECT_DIRECTORY",
        &["rev-parse", "--git-path", "objects"],
    ),
    ("GIT_COMMON_DIR", &["rev-parse", "--git-common-dir"]),
];

/// A set `GIT_INDEX_FILE` that is empty or names no file.
const NO_INDEX_FILE: &str = "GIT_INDEX_FILE names no file: git would read it as an empty index";

/// `GIT_DIR` set alone makes the caller's directory the top (git's rule),
/// so below the top of `GIT_DIR`'s working tree (a hook `cd`-ing below a
/// linked worktree's top, or into a submodule) another project's config
/// could be checked: refused.
const GIT_DIR_BELOW_TOP: &str =
    "GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top";

/// Set on every child, whatever [`GitEnv`] holds: no index refresh, no
/// lazy fetch from a promisor remote, no replace refs, no prompt.
const CHILD_VARIABLES: [(&str, &str); 4] = [
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("GIT_NO_LAZY_FETCH", "1"),
    ("GIT_NO_REPLACE_OBJECTS", "1"),
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// Before every subcommand: no fsmonitor daemon is asked or started.
const CONFIG_OVERRIDES: [&str; 2] = ["-c", "core.fsmonitor=false"];

/// The blob an intent-to-add entry holds, per object format (SHA-1,
/// SHA-256): the detector runs only when a stage-0 entry the check may
/// read holds one ([`may_be_intent_to_add`]).
const EMPTY_BLOBS: [&str; 2] = [
    "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
    "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813",
];

/// The read error of a document, config or baseline whose staged blob is
/// not in the object database.
pub(crate) const MISSING_BLOB: &str = "the staged blob is missing from the git object database";
/// The read error of a staged object that is not a blob.
pub(crate) const NOT_A_BLOB: &str = "the staged object is not a blob";

/// The environment of git's children: the directory relative path
/// variables are resolved against, and every variable they get. Explicit,
/// so a library caller or test passes its own and never sets the process's.
/// `PATH` is searched for `git` as the children see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitEnv {
    cwd: PathBuf,
    vars: BTreeMap<OsString, OsString>,
}

impl GitEnv {
    /// `cwd` and exactly `vars` (nothing inherited).
    pub fn new<K, V>(cwd: impl Into<PathBuf>, vars: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        Self {
            cwd: cwd.into(),
            vars: vars
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    /// The running process's current directory and whole environment.
    pub fn from_process() -> io::Result<Self> {
        Ok(Self::new(std::env::current_dir()?, std::env::vars_os()))
    }

    /// The directory relative `GIT_*` paths are resolved against (the
    /// caller's current directory; a hook's is the working tree's top).
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The value of `name`, if set.
    pub fn var(&self, name: impl AsRef<OsStr>) -> Option<&OsStr> {
        self.vars.get(name.as_ref()).map(OsString::as_os_str)
    }

    /// Every variable, by name.
    pub fn vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(name, value)| (name.as_os_str(), value.as_os_str()))
    }

    /// With `name` set to `value`.
    #[must_use]
    pub fn with_var(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.vars.insert(name.into(), value.into());
        self
    }

    /// With `name` unset.
    #[must_use]
    pub fn without_var(mut self, name: impl AsRef<OsStr>) -> Self {
        self.vars.remove(name.as_ref());
        self
    }

    /// The variables of children run in the root, [`CHILD_VARIABLES`] set.
    /// A relative [`TOP_RELATIVE`] variable becomes what its `rev-parse`
    /// prints in the cwd with the inherited variables, made absolute
    /// against the cwd (git resolves it against the top it finds from
    /// there); then a relative [`CWD_RELATIVE`] one is made absolute
    /// against the cwd, and `GIT_DIR` alone is given `GIT_WORK_TREE` = the
    /// cwd (git's rule for an exported `GIT_DIR`). A non-empty `GIT_DIR`
    /// without `GIT_WORK_TREE` in a cwd that is not the top of its working
    /// tree ([`Git::is_not_top_of`]) is refused before the relative
    /// variables are resolved ([`GIT_DIR_BELOW_TOP`]). A set
    /// `GIT_INDEX_FILE` that is empty or names no file is refused: git
    /// would read it as an empty index. An absolute or empty value is kept:
    /// `rev-parse` would print an absolute one unchanged.
    fn child_vars(&self) -> Result<BTreeMap<OsString, OsString>, Cause> {
        if self.var("GIT_INDEX_FILE").is_some_and(OsStr::is_empty) {
            return Err(root_cause(NO_INDEX_FILE));
        }
        let cwd = std::path::absolute(&self.cwd).unwrap_or_else(|_| self.cwd.clone());
        let mut inherited = self.vars.clone();
        for (name, value) in CHILD_VARIABLES {
            inherited.insert(name.into(), value.into());
        }
        let caller = Git {
            dir: cwd.clone(),
            vars: inherited,
        };
        if let Some(git_dir) = self.var("GIT_DIR").filter(|dir| !dir.is_empty())
            && self.var("GIT_WORK_TREE").is_none()
            && caller.is_not_top_of(&cwd.join(git_dir))?
        {
            return Err(root_cause(GIT_DIR_BELOW_TOP));
        }
        let mut vars = caller.vars.clone();
        for (name, args) in TOP_RELATIVE {
            if vars
                .get(OsStr::new(name))
                .is_some_and(|value| !value.is_empty() && Path::new(value).is_relative())
            {
                let resolved = caller.resolve(name, args)?;
                vars.insert(name.into(), cwd.join(resolved).into_os_string());
            }
        }
        for name in CWD_RELATIVE {
            if let Some(value) = vars.get_mut(OsStr::new(name))
                && !value.is_empty()
                && Path::new(value).is_relative()
            {
                *value = cwd.join(&*value).into_os_string();
            }
        }
        if vars.contains_key(OsStr::new("GIT_DIR"))
            && !vars.contains_key(OsStr::new("GIT_WORK_TREE"))
        {
            vars.insert("GIT_WORK_TREE".into(), cwd.into_os_string());
        }
        if let Some(index) = vars.get(OsStr::new("GIT_INDEX_FILE"))
            && matches!(fs::metadata(index), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            return Err(root_cause(NO_INDEX_FILE));
        }
        Ok(vars)
    }
}

/// `bytes` as an OS string: any bytes on Unix, UTF-8 elsewhere.
#[cfg(unix)]
fn os_string(bytes: Vec<u8>) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt as _;
    Some(OsString::from_vec(bytes))
}

/// `bytes` as an OS string: any bytes on Unix, UTF-8 elsewhere.
#[cfg(not(unix))]
fn os_string(bytes: Vec<u8>) -> Option<OsString> {
    String::from_utf8(bytes).ok().map(OsString::from)
}

/// The one path `git rev-parse` prints: one trailing LF stripped, the
/// bytes as an OS string. Anything else is unreadable output.
fn rev_parse_path(output: &[u8]) -> Result<PathBuf, Cause> {
    output
        .strip_suffix(b"\n")
        .filter(|path| !path.is_empty())
        .and_then(|path| os_string(path.to_vec()))
        .map(PathBuf::from)
        .ok_or_else(|| {
            GitFailure::Unreadable {
                command: "rev-parse",
            }
            .cause()
        })
}

/// `a` and `b` name one directory (or file): equal once canonical
/// (symbolic links, `/tmp` and `/private/tmp`) or, on Unix, one inode of
/// one device (case and Unicode forms on a case-insensitive or normalising
/// file system). A side that cannot be resolved compares as given.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b || same_inode(&a, &b),
        _ => a == b,
    }
}

#[cfg(unix)]
fn same_inode(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_inode(_: &Path, _: &Path) -> bool {
    false
}

/// A cause at the root, `.`.
pub(crate) fn root_cause(message: &str) -> Cause {
    Cause {
        path: ".".to_owned(),
        message: message.to_owned(),
    }
}

/// Why a git command gave nothing usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitFailure {
    /// `git` could not be started.
    NotRunnable(io::ErrorKind),
    /// The root vanished before git could run in it.
    RootGone,
    /// The command exited unsuccessfully (`None`: killed by a signal).
    Failed {
        command: &'static str,
        code: Option<i32>,
    },
    /// The output broke the format, or the pipe broke.
    Unreadable { command: &'static str },
}

impl GitFailure {
    /// The fixed cause at `.`.
    pub(crate) fn cause(&self) -> Cause {
        let message = match self {
            Self::NotRunnable(io::ErrorKind::NotFound) => {
                "`git` could not be run: no `git` program was found on PATH".to_owned()
            }
            Self::NotRunnable(_) => "`git` could not be run".to_owned(),
            Self::RootGone => "the root cannot be read".to_owned(),
            Self::Failed {
                command,
                code: Some(code),
            } => format!("`git {command}` failed (exit status {code})"),
            Self::Failed {
                command,
                code: None,
            } => {
                format!("`git {command}` was stopped by a signal")
            }
            Self::Unreadable { command } => {
                format!("`git {command}` gave output that could not be read")
            }
        };
        Cause {
            path: ".".to_owned(),
            message,
        }
    }
}

/// What a stage-0 entry is, by its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    /// `100644`, `100755`.
    Regular,
    /// `120000`.
    Symlink,
    /// `160000`.
    Gitlink,
    /// Anything else (never listed).
    Other,
}

impl EntryKind {
    fn of(mode: &[u8]) -> Self {
        match mode {
            b"100644" | b"100755" => Self::Regular,
            b"120000" => Self::Symlink,
            b"160000" => Self::Gitlink,
            _ => Self::Other,
        }
    }

    /// Why a config or baseline entry of this kind cannot be read; `None`
    /// for a regular file.
    pub(crate) fn not_regular(self) -> Option<&'static str> {
        match self {
            Self::Regular => None,
            Self::Symlink => Some("staged as a symbolic link, not a regular file"),
            Self::Gitlink => Some("staged as a submodule, not a regular file"),
            Self::Other => Some("staged with a mode that is not a regular file's"),
        }
    }
}

/// One stage-0 entry of the index, root-relative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The path as git stores it, root-relative, `/`-separated.
    pub(crate) path: Vec<u8>,
    pub(crate) kind: EntryKind,
    /// Lower-case hex, 40 or 64 characters.
    pub(crate) oid: String,
}

/// An OID as git prints it: 40 or 64 lower-case hex digits.
fn is_oid(oid: &[u8]) -> bool {
    matches!(oid.len(), 40 | 64)
        && oid
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

/// One record of `ls-files -s -z`.
struct Record {
    entry: Entry,
    stage: u8,
}

/// `<mode> SP <oid> SP <stage> TAB <path> NUL`, parsed as bytes: split at
/// the first TAB (a path may hold TAB and LF), an OID of 40 or 64 hex.
/// `None`: a record breaks the format.
fn parse_ls_files(output: &[u8]) -> Option<Vec<Record>> {
    let Some(body) = output.strip_suffix(b"\0") else {
        return output.is_empty().then(Vec::new);
    };
    let mut records = Vec::new();
    for record in body.split(|&byte| byte == 0) {
        let tab = record.iter().position(|&byte| byte == b'\t')?;
        let (head, path) = (&record[..tab], &record[tab + 1..]);
        let mut fields = head.split(|&byte| byte == b' ');
        let (Some(mode), Some(oid), Some(stage), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return None;
        };
        let mode_ok = is_mode(mode);
        let oid_ok = is_oid(oid);
        let stage = match stage {
            b"0" => 0,
            b"1" => 1,
            b"2" => 2,
            b"3" => 3,
            _ => return None,
        };
        if !mode_ok || !oid_ok || path.is_empty() {
            return None;
        }
        records.push(Record {
            entry: Entry {
                path: path.to_vec(),
                kind: EntryKind::of(mode),
                oid: String::from_utf8(oid.to_vec()).ok()?,
            },
            stage,
        });
    }
    Some(records)
}

/// Six octal digits.
fn is_mode(mode: &[u8]) -> bool {
    mode.len() == 6 && mode.iter().all(|byte| (b'0'..=b'7').contains(byte))
}

/// `<mode> SP <type> SP <oid> TAB <path> NUL`, `ls-tree -r -z`'s records,
/// parsed as [`parse_ls_files`] parses its own: split at the first TAB, an
/// OID of 40 or 64 hex. `None`: a record breaks the format.
fn parse_ls_tree(output: &[u8]) -> Option<Vec<Entry>> {
    let Some(body) = output.strip_suffix(b"\0") else {
        return output.is_empty().then(Vec::new);
    };
    let mut entries = Vec::new();
    for record in body.split(|&byte| byte == 0) {
        let tab = record.iter().position(|&byte| byte == b'\t')?;
        let (head, path) = (&record[..tab], &record[tab + 1..]);
        let mut fields = head.split(|&byte| byte == b' ');
        let (Some(mode), Some(kind), Some(oid), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return None;
        };
        let kind_ok = !kind.is_empty() && kind.iter().all(u8::is_ascii_lowercase);
        if !is_mode(mode) || !kind_ok || !is_oid(oid) || path.is_empty() {
            return None;
        }
        entries.push(Entry {
            path: path.to_vec(),
            kind: EntryKind::of(mode),
            oid: String::from_utf8(oid.to_vec()).ok()?,
        });
    }
    Some(entries)
}

/// Git run in one directory with one resolved environment.
#[derive(Debug)]
struct Git {
    dir: PathBuf,
    vars: BTreeMap<OsString, OsString>,
}

impl Git {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .current_dir(&self.dir)
            .env_clear()
            .envs(&self.vars)
            .args(CONFIG_OVERRIDES)
            .args(args)
            .stdin(Stdio::null());
        command
    }

    fn spawn_failure(&self, error: &io::Error) -> GitFailure {
        if error.kind() == io::ErrorKind::NotFound && !self.dir.is_dir() {
            GitFailure::RootGone
        } else {
            GitFailure::NotRunnable(error.kind())
        }
    }

    /// Runs `git <args>` to completion; its stdout on success.
    fn run(&self, command: &'static str, args: &[&str]) -> Result<Vec<u8>, GitFailure> {
        let output = self
            .command(args)
            .output()
            .map_err(|error| self.spawn_failure(&error))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(GitFailure::Failed {
                command,
                code: output.status.code(),
            })
        }
    }

    /// The path git reads the relative variable `name` as, relative to this
    /// directory: the one line `git <args>` prints here. A failure is a
    /// fixed cause at `.` naming the variable (git failing here: no
    /// repository found from this directory, or the directory gone).
    fn resolve(&self, name: &str, args: &[&str]) -> Result<PathBuf, Cause> {
        let output = match self.run("rev-parse", args) {
            Ok(output) => output,
            Err(GitFailure::Failed { code: Some(_), .. } | GitFailure::RootGone) => {
                return Err(root_cause(&format!(
                    "{name} is relative and git could not resolve it from the current directory"
                )));
            }
            Err(failure) => return Err(failure.cause()),
        };
        rev_parse_path(&output)
    }

    /// Whether this directory is not the top of the working tree of
    /// `git_dir` (absolute), the `GIT_DIR` set alone: git discovering from
    /// here without `GIT_DIR` and `GIT_CEILING_DIRECTORIES`, with
    /// `GIT_DISCOVERY_ACROSS_FILESYSTEM=1` (the caller's limits hide no
    /// top), finds a top other than this directory, or a git dir other
    /// than `git_dir` or than what the top's `.git` names when `git_dir` is
    /// that gitfile. Two `rev-parse` calls, `--show-toplevel` then (a top
    /// here) `--absolute-git-dir`: each prints one path, which may hold an
    /// LF, so no two-path output is split. `false` when git finds no
    /// working tree (it fails, prints nothing, or this directory is gone):
    /// git's rule, this directory is the top.
    fn is_not_top_of(&self, git_dir: &Path) -> Result<bool, Cause> {
        let mut vars = self.vars.clone();
        vars.remove(OsStr::new("GIT_DIR"));
        vars.remove(OsStr::new("GIT_CEILING_DIRECTORIES"));
        vars.insert("GIT_DISCOVERY_ACROSS_FILESYSTEM".into(), "1".into());
        let discovery = Git {
            dir: self.dir.clone(),
            vars,
        };
        let output = match discovery.run("rev-parse", &["rev-parse", "--show-toplevel"]) {
            Ok(output) if output.is_empty() => return Ok(false),
            Ok(output) => output,
            Err(GitFailure::Failed { code: Some(_), .. } | GitFailure::RootGone) => {
                return Ok(false);
            }
            Err(failure) => return Err(failure.cause()),
        };
        let top = self.dir.join(rev_parse_path(&output)?);
        if !same_dir(&top, &self.dir) {
            return Ok(true);
        }
        let output = discovery
            .run("rev-parse", &["rev-parse", "--absolute-git-dir"])
            .map_err(|failure| failure.cause())?;
        let found = rev_parse_path(&output)?;
        let gitfile = git_dir.is_file() && same_dir(git_dir, &self.dir.join(".git"));
        Ok(!gitfile && !same_dir(&found, git_dir))
    }

    /// The root lies in a git working tree.
    fn check_work_tree(&self) -> Result<(), Cause> {
        match self.run("rev-parse", &["rev-parse", "--is-inside-work-tree"]) {
            Ok(output) if output.trim_ascii() == b"true" => Ok(()),
            Ok(_) => Err(root_cause("the root is not inside a git working tree")),
            Err(GitFailure::Failed { .. }) => Err(root_cause(
                "no git repository was found for the root, or git could not read it",
            )),
            Err(failure) => Err(failure.cause()),
        }
    }

    /// `HEAD`'s commit: `Some(oid)` when born, `None` when unborn (exit 1,
    /// nothing printed); anything else is a failure.
    fn head(&self) -> Result<Option<String>, GitFailure> {
        const COMMAND: &str = "rev-parse";
        let output = self
            .command(&["rev-parse", "--verify", "-q", "HEAD"])
            .output()
            .map_err(|error| self.spawn_failure(&error))?;
        let unreadable = GitFailure::Unreadable { command: COMMAND };
        match output.status.code() {
            Some(0) => {
                let oid = output
                    .stdout
                    .strip_suffix(b"\n")
                    .ok_or(unreadable.clone())?;
                if !is_oid(oid) {
                    return Err(unreadable);
                }
                String::from_utf8(oid.to_vec())
                    .map(Some)
                    .map_err(|_| unreadable)
            }
            Some(1) if output.stdout.is_empty() => Ok(None),
            Some(1) => Err(unreadable),
            code => Err(GitFailure::Failed {
                command: COMMAND,
                code,
            }),
        }
    }

    /// The tree of commit `oid` under the root, root-relative.
    fn ls_tree(&self, oid: &str) -> Result<Vec<Entry>, GitFailure> {
        let output = self.run("ls-tree", &["ls-tree", "-r", "-z", oid])?;
        parse_ls_tree(&output).ok_or(GitFailure::Unreadable { command: "ls-tree" })
    }

    fn ls_files(&self) -> Result<Vec<Record>, GitFailure> {
        let output = self.run("ls-files", &["ls-files", "-s", "-z"])?;
        parse_ls_files(&output).ok_or(GitFailure::Unreadable {
            command: "ls-files",
        })
    }

    /// The intent-to-add paths under the root, root-relative.
    fn intent_to_add(&self) -> Result<BTreeSet<Vec<u8>>, GitFailure> {
        let output = self.run(
            "diff-files",
            &[
                "diff-files",
                "-z",
                "--name-only",
                "--no-renames",
                // No `status` run inside each submodule.
                "--ignore-submodules=all",
                "--diff-filter=A",
                "--ita-invisible-in-index",
                "--relative",
            ],
        )?;
        Ok(output
            .split(|&byte| byte == 0)
            .filter(|path| !path.is_empty())
            .map(<[u8]>::to_vec)
            .collect())
    }

    fn cat_file(&self) -> Result<CatFile, GitFailure> {
        let unreadable = GitFailure::Unreadable {
            command: "cat-file",
        };
        let mut child = self
            .command(&["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Nobody reads it: a pipe could fill and stall the session.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| self.spawn_failure(&error))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(unreadable);
        };
        Ok(CatFile {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            done: false,
        })
    }
}

/// A staged object as `cat-file --batch` gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Blob {
    Bytes(Vec<u8>),
    /// `<oid> missing`: not in the object database (no fetch is tried).
    Missing,
    /// Another type (a tree, a commit, a tag) under a regular entry's OID.
    Foreign,
}

impl Blob {
    /// The bytes, or the read error of a missing or foreign object.
    pub(crate) fn bytes(&self) -> io::Result<Vec<u8>> {
        match self {
            Self::Bytes(bytes) => Ok(bytes.clone()),
            Self::Missing => Err(io::Error::other(MISSING_BLOB)),
            Self::Foreign => Err(io::Error::other(NOT_A_BLOB)),
        }
    }
}

/// One `cat-file --batch` session in strict lockstep.
struct CatFile {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    done: bool,
}

impl CatFile {
    const COMMAND: &'static str = "cat-file";

    /// Writes one request and reads its whole reply:
    /// `<oid> SP <type> SP <size> LF <bytes> LF` or `<oid> SP missing LF`.
    fn get(&mut self, oid: &str) -> Result<Blob, GitFailure> {
        let unreadable = || GitFailure::Unreadable {
            command: Self::COMMAND,
        };
        let stdin = self.stdin.as_mut().ok_or_else(unreadable)?;
        stdin
            .write_all(format!("{oid}\n").as_bytes())
            .and_then(|()| stdin.flush())
            .map_err(|_| unreadable())?;
        let mut header = Vec::new();
        self.stdout
            .read_until(b'\n', &mut header)
            .map_err(|_| unreadable())?;
        let header = header.strip_suffix(b"\n").ok_or_else(unreadable)?;
        let fields: Vec<&[u8]> = header.split(|&byte| byte == b' ').collect();
        match fields.as_slice() {
            [name, status] if *name == oid.as_bytes() && *status == b"missing" => Ok(Blob::Missing),
            [name, kind, size] if *name == oid.as_bytes() => {
                let size: u64 = std::str::from_utf8(size)
                    .ok()
                    .and_then(|size| size.parse().ok())
                    .ok_or_else(unreadable)?;
                let mut bytes = Vec::new();
                (&mut self.stdout)
                    .take(size)
                    .read_to_end(&mut bytes)
                    .map_err(|_| unreadable())?;
                let mut end = [0_u8; 1];
                self.stdout.read_exact(&mut end).map_err(|_| unreadable())?;
                if u64::try_from(bytes.len()).ok() != Some(size) || end != *b"\n" {
                    return Err(unreadable());
                }
                Ok(if *kind == b"blob" {
                    Blob::Bytes(bytes)
                } else {
                    Blob::Foreign
                })
            }
            _ => Err(unreadable()),
        }
    }

    /// Closes the requests and waits: the session must end cleanly, with
    /// nothing left unread.
    fn finish(mut self) -> Result<(), GitFailure> {
        drop(self.stdin.take());
        let mut rest = Vec::new();
        let drained = self.stdout.read_to_end(&mut rest);
        let status = self.child.wait();
        self.done = true;
        let status = status.map_err(|_| GitFailure::Unreadable {
            command: Self::COMMAND,
        })?;
        if !status.success() {
            return Err(GitFailure::Failed {
                command: Self::COMMAND,
                code: status.code(),
            });
        }
        if drained.is_err() || !rest.is_empty() {
            return Err(GitFailure::Unreadable {
                command: Self::COMMAND,
            });
        }
        Ok(())
    }
}

impl std::fmt::Debug for CatFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CatFile")
    }
}

impl Drop for CatFile {
    /// A session left mid-way is killed, never waited on while it may be
    /// blocked writing a reply nobody reads.
    fn drop(&mut self) {
        drop(self.stdin.take());
        if !self.done {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// A stage-0 entry the intent-to-add detector must rule on: the empty blob
/// at a path the check may read, a `.md` (a superset of the walk, known
/// before the config) or the root's config or baseline. Any other empty
/// blob (`.gitkeep`, an empty `__init__.py`) never starts the detector,
/// which runs clean filters on racily-clean entries.
fn may_be_intent_to_add(entry: &Entry) -> bool {
    EMPTY_BLOBS.contains(&entry.oid.as_str())
        && (entry.path.ends_with(DOCUMENT_EXTENSION.as_bytes())
            || entry.path == CONFIG_FILE.as_bytes()
            || entry.path == BASELINE_FILE.as_bytes())
}

/// The index git would commit, under the root: its stage-0 entries minus
/// intent-to-add ones, byte-sorted, and the `cat-file` session reading
/// their blobs and `HEAD`'s (started at the first read, one per check),
/// with every object it read.
#[derive(Debug)]
pub(crate) struct Staged {
    /// Canonical.
    pub(crate) root: PathBuf,
    pub(crate) entries: Vec<Entry>,
    git: Git,
    session: Option<CatFile>,
    /// Every object read, by OID: none is requested twice.
    pub(crate) objects: BTreeMap<String, Blob>,
}

impl Staged {
    /// Lists the index of the repository the root lies in. Causes: the
    /// root unreadable, `GIT_DIR` set alone off its working tree's top, a
    /// relative path variable git cannot resolve from the caller's
    /// directory, `GIT_INDEX_FILE` naming no file, no repository, `git` not
    /// runnable or failing (each at `.`), a path under the root with
    /// unmerged stages (one cause per path).
    pub(crate) fn read(root: &Path, env: &GitEnv) -> Result<Self, Vec<Cause>> {
        let dir = fs::canonicalize(root).map_err(|_| vec![GitFailure::RootGone.cause()])?;
        let git = Git {
            dir,
            vars: env.child_vars().map_err(|cause| vec![cause])?,
        };
        git.check_work_tree().map_err(|cause| vec![cause])?;
        let records = git.ls_files().map_err(|failure| vec![failure.cause()])?;
        let mut unmerged = BTreeSet::new();
        let mut entries = Vec::new();
        for record in records {
            if record.stage == 0 {
                entries.push(record.entry);
            } else {
                unmerged.insert(record.entry.path);
            }
        }
        if !unmerged.is_empty() {
            return Err(unmerged
                .into_iter()
                .map(|path| Cause {
                    path: String::from_utf8_lossy(&path).into_owned(),
                    message: "unmerged in the git index: the conflict is not resolved".to_owned(),
                })
                .collect());
        }
        if entries.iter().any(may_be_intent_to_add) {
            let intent = git
                .intent_to_add()
                .map_err(|failure| vec![failure.cause()])?;
            entries.retain(|entry| !intent.contains(&entry.path));
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries.dedup_by(|a, b| a.path == b.path);
        Ok(Self {
            root: git.dir.clone(),
            entries,
            git,
            session: None,
            objects: BTreeMap::new(),
        })
    }

    /// The stage-0 entry at exactly `path`.
    pub(crate) fn entry(&self, path: &[u8]) -> Option<&Entry> {
        self.entries
            .binary_search_by(|entry| entry.path.as_slice().cmp(path))
            .ok()
            .map(|at| &self.entries[at])
    }

    /// The object `oid`, read through the session (started on first use)
    /// unless already read: each OID is requested once.
    pub(crate) fn blob(&mut self, oid: &str) -> Result<&Blob, GitFailure> {
        match self.objects.entry(oid.to_owned()) {
            btree_map::Entry::Occupied(known) => Ok(known.into_mut()),
            btree_map::Entry::Vacant(slot) => {
                let session = match &mut self.session {
                    Some(session) => session,
                    None => self.session.insert(self.git.cat_file()?),
                };
                Ok(slot.insert(session.get(oid)?))
            }
        }
    }

    /// `HEAD`'s commit in the root: `None` when unborn.
    pub(crate) fn head(&self) -> Result<Option<String>, GitFailure> {
        self.git.head()
    }

    /// The tree of commit `oid` under the root (`ls-tree -r -z`),
    /// byte-sorted, one entry per path: nothing for a root it lacks.
    pub(crate) fn head_entries(&self, oid: &str) -> Result<Vec<Entry>, GitFailure> {
        let mut entries = self.git.ls_tree(oid)?;
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries.dedup_by(|a, b| a.path == b.path);
        Ok(entries)
    }

    /// Ends the session, if one was started.
    pub(crate) fn finish(&mut self) -> Result<(), GitFailure> {
        match self.session.take() {
            Some(session) => session.finish(),
            None => Ok(()),
        }
    }
}
