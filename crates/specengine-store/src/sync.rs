//! A hub's sync of its checkout with the upstream (task spec `hub-git`): the
//! only module of any crate that spawns git's `fetch` and `push`
//! (`tests/hub_git.rs` pins it). Its pieces, each a git child under
//! [`WorktreeGit`]'s environment (cleared, the caller's variables,
//! `GIT_TERMINAL_PROMPT=0`, stdin null) and its own deadline, local ones
//! too: SIGTERM, then SIGKILL 2 s later, so git removes its own locks:
//!
//! - **The look** ([`SyncGit::look`]), reads only: `HEAD` on a branch with a
//!   commit, an upstream on a named remote, a clone neither shallow nor
//!   partial, the remote's URLs (after `insteadOf`, fetch and push) free of
//!   a password and of a user name on `http`, `https`, `ftp`, `ftps`, every
//!   sparse-set path `HEAD` holds present ([`crate::sparse`]) for the synced
//!   worktree's roots; a status that is not clean, or an operation in
//!   progress, only provisionally ([`SyncGit::still_ready`] decides inside
//!   the turns). A URL is never shown: a token holding one is replaced
//!   whole ([`Upstream::redacted`]).
//! - **The fetch** ([`SyncGit::fetch`]): the upstream branch alone into its
//!   tracking ref, a forced update allowed.
//! - **The merge's parts**: the tips ([`SyncGit::tips`]), how they relate
//!   ([`SyncGit::relation`]), `merge-tree --write-tree` in memory
//!   ([`SyncGit::merge_tree`]), a tree with some paths replaced
//!   ([`SyncGit::replaced_tree`], `hash-object -w` and `mktree`), the merge
//!   commit ([`SyncGit::commit_merge`], `commit-tree` under the checkout's
//!   identity, `user.useConfigOnly`), the fast-forward ([`SyncGit::fast_forward`],
//!   `merge --ff-only --no-autostash`). No rebase, reset, stash, clean,
//!   checkout or `MERGE_HEAD`, ever.
//! - **The push** ([`SyncGit::push`]): `refs/heads/<b>:refs/heads/<m>`,
//!   `--porcelain`, never forced, `pre-push` run; read from its ref line.
//! - **Ahead and behind** ([`ahead_behind`]), per status read.
//!
//! Every child but `fetch` and `push` has `GIT_NO_LAZY_FETCH=1`.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use crate::git::GitEnv;
use crate::sparse::{SparseError, sparse_set};
use crate::worktree::{GitError, WorktreeGit, cut_stderr, failed, is_oid, one_line, success};

/// Git's verb that reads the remote; spawned here alone.
const FETCH: &str = "fetch";

/// Git's verb that writes the remote; spawned here alone.
const PUSH: &str = "push";

/// The most paths a look's or a merge's message names.
const NAMED_MAX: usize = 10;

/// Git's mode of a directory in a tree.
const TREE_MODE: &str = "040000";

/// Git's mode of a regular file.
const FILE_MODE: &str = "100644";

/// What a token holding a URL is shown as.
const URL_SHOWN: &str = "<url>";

/// The schemes whose URL may hold no user name at all.
const NAMELESS_SCHEMES: [&str; 4] = ["http", "https", "ftp", "ftps"];

/// The checkout's branch and where it goes: `branch.<b>.remote` and
/// `branch.<b>.merge`.
#[derive(Clone, PartialEq, Eq)]
pub struct Upstream {
    /// `<b>`, `HEAD`'s branch.
    pub branch: String,
    /// `<r>`, a named remote.
    pub remote: String,
    /// `<m>`, `refs/heads/` dropped.
    pub merge: String,
    /// The remote's URLs, fetch and push, after `insteadOf`: never shown,
    /// only taken out of git's words.
    urls: Vec<String>,
}

impl fmt::Debug for Upstream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upstream")
            .field("branch", &self.branch)
            .field("remote", &self.remote)
            .field("merge", &self.merge)
            .finish_non_exhaustive()
    }
}

impl Upstream {
    /// `refs/remotes/<r>/<m>`.
    pub fn tracking(&self) -> String {
        format!("refs/remotes/{}/{}", self.remote, self.merge)
    }

    /// `<r>/<m>`, as a message names it.
    pub fn shown(&self) -> String {
        format!("{}/{}", self.remote, self.merge)
    }

    /// `text` with every whitespace-delimited token that holds a URL
    /// replaced whole, never cut inside whatever characters precede it: one
    /// holding the remote's own URL by the remote's name, any other by
    /// `<url>` ([`Self::url_in`]). Git's words keep no URL; the whitespace
    /// between tokens is kept.
    pub fn redacted(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for piece in text.split_inclusive(char::is_whitespace) {
            let token = piece.trim_end_matches(char::is_whitespace);
            match self.url_in(token) {
                Some(UrlToken::Own) => out.push_str(&self.remote),
                Some(UrlToken::Other) => out.push_str(URL_SHOWN),
                None => out.push_str(token),
            }
            out.push_str(&piece[token.len()..]);
        }
        out
    }

    /// The URL a token holds, if any. The remote's own: its quotes,
    /// brackets and trailing punctuation aside, one of the remote's URLs (a
    /// `file://` one's path too) exactly, or an absolute or `host:path` one
    /// with a path below it; so a short URL (`.`) takes only a token that is
    /// exactly it. Another: `://` anywhere in the token.
    fn url_in(&self, token: &str) -> Option<UrlToken> {
        if token.is_empty() {
            return None;
        }
        let core = token
            .trim_matches(|c: char| {
                matches!(c, '\'' | '"' | '`' | '(' | ')' | '[' | ']' | '<' | '>')
            })
            .trim_end_matches([',', ';', ':'])
            .trim_matches(|c: char| matches!(c, '\'' | '"' | '`'));
        let core = core.trim_end_matches('/');
        let own = self
            .urls
            .iter()
            .flat_map(|url| [Some(url.as_str()), url.strip_prefix("file://")])
            .flatten()
            .map(|url| url.trim_end_matches('/'))
            .filter(|url| !url.is_empty())
            .any(|url| {
                let deep = url.starts_with('/') || (url.contains(':') && !url.starts_with('.'));
                core == url
                    || (deep
                        && core
                            .strip_prefix(url)
                            .is_some_and(|below| below.starts_with('/')))
            });
        if own {
            Some(UrlToken::Own)
        } else if token.contains("://") {
            Some(UrlToken::Other)
        } else {
            None
        }
    }
}

/// What URL a token of git's words holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UrlToken {
    /// One of the remote's own: shown as its name.
    Own,
    /// Any other: shown as `<url>`.
    Other,
}

/// What a look found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Looked {
    /// Ready to fetch: the branch and its upstream. `unclean`: the status
    /// was not clean or an operation was in progress, which is provisional
    /// (a confirm in mid-apply): [`SyncGit::still_ready`], inside the turns,
    /// decides.
    Ready { upstream: Upstream, unclean: bool },
    /// Nothing touched: why, in the daemon's words (no URL).
    Stopped(String),
}

/// How a network step failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetFailure {
    /// Git spoke: its stderr, URLs taken out, cut at 4 KiB.
    Failed(String),
    /// The deadline passed: the child was killed.
    Deadline,
}

/// How `R`, the tracking ref, stands to `L`, `HEAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// `R` is `L` or an ancestor of it: nothing to merge.
    Contained,
    /// `L` is an ancestor of `R`: a fast-forward.
    FastForward,
    /// No merge base.
    Unrelated,
    /// Both moved.
    Diverged,
}

/// What [`SyncGit::changed`] found between two commits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangedPaths {
    /// Top-relative, in git's order.
    pub paths: Vec<String>,
    /// Names left out because they are not UTF-8.
    pub skipped_names: usize,
}

/// `merge-tree --write-tree`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedTree {
    /// The merged tree, conflicted files holding git's markers.
    pub tree: String,
    /// The conflicted paths from the top, each once, in git's order.
    pub conflicts: Vec<String>,
}

/// One path's new entry in [`SyncGit::replaced_tree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replacement {
    /// The blob `oid` under `mode` (`100644`, `100755`, `120000`).
    Blob { mode: String, oid: String },
    /// No entry.
    Remove,
}

/// One entry of a tree, as `ls-tree` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    /// `blob`, `tree` or `commit`.
    pub kind: String,
    pub oid: String,
}

/// Why no merge commit was made.
#[derive(Debug)]
pub enum CommitFailure {
    /// Git has no configured identity: its words.
    Identity(String),
    /// Anything else.
    Git(GitError),
}

/// What a push answered, read from its `--porcelain` ref line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// The remote holds the branch, and git ended cleanly.
    Accepted,
    /// Not a fast-forward of the remote's ref: fetch and merge again.
    NotFastForward,
    /// The remote refused the ref (a hook, a protection): its reason and
    /// git's stderr.
    Refused { reason: String, stderr: String },
    /// Anything else: network, authentication, `pre-push`, an answer
    /// lost.
    Failed(String),
    /// The deadline passed: the child was killed.
    Deadline,
}

/// Git in a checkout's top for a sync: each child with its own deadline.
#[derive(Debug, Clone)]
pub struct SyncGit {
    git: WorktreeGit,
    top: PathBuf,
    deadline: Duration,
}

impl SyncGit {
    /// Git in the worktree's top `top` with `env`'s variables, each child
    /// stopped past `deadline` (SIGTERM, then SIGKILL 2 s later).
    pub fn open(top: &Path, env: &GitEnv, deadline: Duration) -> Result<Self, GitError> {
        let git = WorktreeGit::new_until(top, env, Instant::now() + deadline)?;
        Ok(Self {
            git,
            top: top.to_path_buf(),
            deadline,
        })
    }

    /// The deadline of one child, in whole seconds.
    pub fn deadline_seconds(&self) -> u64 {
        self.deadline.as_secs()
    }

    /// The variables of this sync's children (the local `GIT_*` ones
    /// dropped), in `dir`: the render's.
    pub fn env_in(&self, dir: &Path) -> GitEnv {
        GitEnv::new(dir.to_path_buf(), self.git.vars())
    }

    /// The handle of the next child.
    fn child(&self) -> WorktreeGit {
        self.git.until(Instant::now() + self.deadline)
    }

    fn output(&self, args: &[&str], read_only: bool) -> Result<Output, GitError> {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        self.child().output_of(&args, read_only)
    }

    fn read(&self, command: &'static str, args: &[&str]) -> Result<Vec<u8>, GitError> {
        success(command, self.output(args, true)?)
    }

    /// `git config --get <key>`: `None` when unset.
    fn config(&self, key: &str) -> Result<Option<String>, GitError> {
        const COMMAND: &str = "config";
        let output = self.output(&["config", "--get", key], true)?;
        match output.status.code() {
            Some(0) => {
                Ok(one_line(&output.stdout).map(|line| String::from_utf8_lossy(line).into_owned()))
            }
            Some(1) => Ok(None),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// The look (see the module documentation); `roots` the served roots in
    /// this worktree, canonical, whose sparse sets must be present.
    pub fn look(&self, roots: &[PathBuf]) -> Result<Looked, GitError> {
        let shown = self.top.display();
        let stopped = |why: String| Ok(Looked::Stopped(why));
        let git = self.child();
        let Some(branch) = git.branch()?.filter(|branch| !branch.starts_with("refs/")) else {
            return stopped(format!(
                "HEAD is on no branch in {shown}: check out the branch a sync carries"
            ));
        };
        if self.child().head()?.is_none() {
            return stopped(format!(
                "`{branch}` has no commit yet in {shown}: a sync carries commits"
            ));
        }
        let no_upstream = || {
            Ok(Looked::Stopped(format!(
                "`{branch}` has no upstream on a remote: set one with git branch \
                 --set-upstream-to"
            )))
        };
        let remote = self.config(&format!("branch.{branch}.remote"))?;
        let merge = self.config(&format!("branch.{branch}.merge"))?;
        let (Some(remote), Some(merge)) = (remote, merge) else {
            return no_upstream();
        };
        let Some(merge) = merge
            .strip_prefix("refs/heads/")
            .filter(|merge| !merge.is_empty())
            .map(str::to_owned)
        else {
            return no_upstream();
        };
        if remote.is_empty()
            || remote == "."
            || remote.starts_with('-')
            || remote.contains([':', ' ', '\t'])
        {
            return no_upstream();
        }
        let Some(fetch_urls) = self.urls(&remote, false)? else {
            return no_upstream();
        };
        let push_urls = self.urls(&remote, true)?.unwrap_or_default();
        if self.read("rev-parse", &["rev-parse", "--is-shallow-repository"])? != b"false\n" {
            return stopped("a shallow clone: a sync needs the whole history".to_owned());
        }
        if self.partial()? {
            return stopped("a partial clone: a sync needs every blob".to_owned());
        }
        let urls: Vec<String> = fetch_urls.into_iter().chain(push_urls).collect();
        if urls.iter().any(|url| holds_secret(url)) {
            return stopped(format!(
                "the remote `{remote}` has a URL holding a user name or password: refused \
                 before any connection"
            ));
        }
        // Provisional: a confirm in mid-apply has its target written and
        // not yet committed; the turns decide.
        let unclean = self.child().operation_in_progress()?.is_some() || self.unclean()?.is_some();
        if let Some(why) = self.outside(roots)? {
            return stopped(why);
        }
        Ok(Looked::Ready {
            upstream: Upstream {
                branch,
                remote,
                merge,
                urls,
            },
            unclean,
        })
    }

    /// Inside the turns, before anything is written: `HEAD` still on
    /// `branch`, no operation in progress, a clean status; else why not.
    pub fn still_ready(&self, upstream: &Upstream) -> Result<Option<String>, GitError> {
        let shown = self.top.display();
        if self.child().branch()?.as_deref() != Some(upstream.branch.as_str()) {
            return Ok(Some(format!(
                "HEAD left `{}` in {shown} during the sync: nothing merged",
                upstream.branch
            )));
        }
        if let Some(operation) = self.child().operation_in_progress()? {
            return Ok(Some(format!(
                "a {operation} is in progress in {shown}: finish or abort it first"
            )));
        }
        self.unclean()
    }

    /// The remote's URLs (`remote get-url [--push] --all`), after
    /// `insteadOf`; `None`: no such remote.
    fn urls(&self, remote: &str, push: bool) -> Result<Option<Vec<String>>, GitError> {
        let mut args = vec!["remote", "get-url"];
        if push {
            args.push("--push");
        }
        args.extend(["--all", remote]);
        let output = self.output(&args, true)?;
        if !output.status.success() {
            // No such remote, or one without a URL: git's words are not
            // shown, they could hold the URL.
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(Some(
            text.lines()
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        ))
    }

    /// A partial clone: `extensions.partialClone` set, or a remote with
    /// `promisor` true or a `partialclonefilter`.
    fn partial(&self) -> Result<bool, GitError> {
        const COMMAND: &str = "config";
        let output = self.output(
            &[
                "config",
                "--get-regexp",
                r"^(extensions\.partialclone|remote\..*\.(promisor|partialclonefilter))$",
            ],
            true,
        )?;
        match output.status.code() {
            Some(1) => Ok(false),
            Some(0) => {
                let text = String::from_utf8_lossy(&output.stdout).into_owned();
                Ok(text.lines().any(|line| {
                    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
                    if key.ends_with(".promisor") {
                        matches!(
                            value.to_ascii_lowercase().as_str(),
                            "true" | "yes" | "on" | "1" | ""
                        )
                    } else {
                        true
                    }
                }))
            }
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// Why the status is not clean (`status --porcelain=v1 -z
    /// --untracked-files=all`), naming at most ten paths; `None` when clean.
    fn unclean(&self) -> Result<Option<String>, GitError> {
        let entries = self.child().status_all()?;
        if entries.is_empty() {
            return Ok(None);
        }
        let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
        let (count, verb) = match paths.len() {
            1 => ("1 path".to_owned(), "is"),
            n => (format!("{n} paths"), "are"),
        };
        Ok(Some(format!(
            "{count} {verb} not clean in {}: {}",
            self.top.display(),
            named(&paths)
        )))
    }

    /// Why a sparse-set path `HEAD` holds is missing from the worktree, for
    /// any root of `roots`; `None` when none is. A root whose `HEAD` config
    /// cannot be used checks nothing: the sync never judges a config.
    fn outside(&self, roots: &[PathBuf]) -> Result<Option<String>, GitError> {
        let mut missing: Vec<String> = Vec::new();
        for root in roots {
            let git =
                WorktreeGit::new_until(root, &self.env_in(root), Instant::now() + self.deadline)?;
            match sparse_set(root, &git) {
                Ok(set) => {
                    for path in set.missing {
                        if !missing.contains(&path) {
                            missing.push(path);
                        }
                    }
                }
                Err(SparseError::Git(error)) => return Err(error),
                Err(SparseError::Config { .. } | SparseError::NoCommit) => {}
                Err(SparseError::NoRepository) => {
                    return Ok(Some(format!(
                        "{} lies in no git repository",
                        root.display()
                    )));
                }
            }
        }
        if missing.is_empty() {
            return Ok(None);
        }
        let paths: Vec<&str> = missing.iter().map(String::as_str).collect();
        Ok(Some(format!(
            "outside this checkout: {}: widen the sparse checkout (spec sparse-set)",
            named(&paths)
        )))
    }

    /// `fetch --no-tags --no-recurse-submodules --no-auto-maintenance <r>
    /// +refs/heads/<m>:refs/remotes/<r>/<m>`.
    pub fn fetch(&self, upstream: &Upstream) -> Result<(), NetFailure> {
        let refspec = format!("+refs/heads/{}:{}", upstream.merge, upstream.tracking());
        let args = [
            FETCH,
            "--no-tags",
            "--no-recurse-submodules",
            "--no-auto-maintenance",
            upstream.remote.as_str(),
            refspec.as_str(),
        ];
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        match self.child().network_output(&args) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(NetFailure::Failed(spoken(upstream, &output))),
            Err(GitError::Deadline) => Err(NetFailure::Deadline),
            Err(error) => Err(NetFailure::Failed(upstream.redacted(&error.to_string()))),
        }
    }

    /// `L`, `HEAD`'s commit, and `R`, the tracking ref's (`None`: absent).
    pub fn tips(&self, upstream: &Upstream) -> Result<(Option<String>, Option<String>), GitError> {
        let left = self.child().head()?;
        let right = self.child().commit_of(&upstream.tracking())?;
        Ok((left, right))
    }

    /// How `right` stands to `left` (`merge-base [--is-ancestor]`).
    pub fn relation(&self, left: &str, right: &str) -> Result<Relation, GitError> {
        if self.is_ancestor(right, left)? {
            return Ok(Relation::Contained);
        }
        if self.is_ancestor(left, right)? {
            return Ok(Relation::FastForward);
        }
        const COMMAND: &str = "merge-base";
        let output = self.output(&["merge-base", left, right], true)?;
        match output.status.code() {
            Some(0) => Ok(Relation::Diverged),
            Some(1) => Ok(Relation::Unrelated),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    fn is_ancestor(&self, ancestor: &str, of: &str) -> Result<bool, GitError> {
        const COMMAND: &str = "merge-base";
        let output = self.output(&["merge-base", "--is-ancestor", ancestor, of], true)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(failed(COMMAND, &output)),
        }
    }

    /// `merge-tree --write-tree -z --name-only L R`: the merged tree and the
    /// conflicted paths, nothing touched but new objects.
    pub fn merge_tree(&self, left: &str, right: &str) -> Result<MergedTree, GitError> {
        const COMMAND: &str = "merge-tree";
        let output = self.output(
            &[
                "merge-tree",
                "--write-tree",
                "-z",
                "--name-only",
                left,
                right,
            ],
            false,
        )?;
        let clean = match output.status.code() {
            Some(0) => true,
            Some(1) => false,
            _ => return Err(failed(COMMAND, &output)),
        };
        let mut fields = output.stdout.split(|&byte| byte == 0);
        let tree = fields
            .next()
            .filter(|oid| is_oid(oid))
            .map(|oid| String::from_utf8_lossy(oid).into_owned())
            .ok_or(GitError::Unreadable { command: COMMAND })?;
        let mut conflicts = Vec::new();
        if !clean {
            for field in fields {
                if field.is_empty() {
                    break;
                }
                let path = String::from_utf8_lossy(field).into_owned();
                if !conflicts.contains(&path) {
                    conflicts.push(path);
                }
            }
            if conflicts.is_empty() {
                return Err(GitError::Unreadable { command: COMMAND });
            }
        }
        Ok(MergedTree { tree, conflicts })
    }

    /// The bytes of the blob at the top-relative `path` of `tree_ish`;
    /// `None` when it holds no blob there.
    pub fn blob(&self, tree_ish: &str, path: &str) -> Result<Option<Vec<u8>>, GitError> {
        match self.entry(tree_ish, path)? {
            Some(entry) if entry.kind == "blob" => self
                .read("cat-file", &["cat-file", "blob", &entry.oid])
                .map(Some),
            _ => Ok(None),
        }
    }

    /// The entry at the top-relative `path` of `tree_ish`, walked one
    /// directory at a time (`ls-tree -z`, exact names); `None`: nothing.
    pub fn entry(&self, tree_ish: &str, path: &str) -> Result<Option<TreeEntry>, GitError> {
        let mut tree = format!("{tree_ish}^{{tree}}");
        let mut names = path.split('/').peekable();
        while let Some(name) = names.next() {
            let entries = self.level(&tree)?;
            let Some(entry) = entries.get(name.as_bytes()) else {
                return Ok(None);
            };
            if names.peek().is_none() {
                return Ok(Some(entry.clone()));
            }
            if entry.kind != "tree" {
                return Ok(None);
            }
            tree.clone_from(&entry.oid);
        }
        Ok(None)
    }

    /// One tree's entries by name (`ls-tree -z <tree>`).
    fn level(&self, tree: &str) -> Result<BTreeMap<Vec<u8>, TreeEntry>, GitError> {
        const COMMAND: &str = "ls-tree";
        let output = self.read(COMMAND, &["ls-tree", "-z", "--end-of-options", tree])?;
        let mut entries = BTreeMap::new();
        for record in output.split(|&byte| byte == 0) {
            if record.is_empty() {
                continue;
            }
            let unreadable = || GitError::Unreadable { command: COMMAND };
            let tab = record
                .iter()
                .position(|&byte| byte == b'\t')
                .ok_or_else(unreadable)?;
            let (head, name) = (&record[..tab], &record[tab + 1..]);
            let head = std::str::from_utf8(head).map_err(|_| unreadable())?;
            let mut parts = head.split(' ');
            let (Some(mode), Some(kind), Some(oid), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(unreadable());
            };
            entries.insert(
                name.to_vec(),
                TreeEntry {
                    mode: mode.to_owned(),
                    kind: kind.to_owned(),
                    oid: oid.to_owned(),
                },
            );
        }
        Ok(entries)
    }

    /// `bytes` written as a blob (`hash-object -w --no-filters --stdin`):
    /// its OID.
    pub fn write_blob(&self, bytes: &[u8]) -> Result<String, GitError> {
        const COMMAND: &str = "hash-object";
        let args = ["hash-object", "-w", "--no-filters", "--stdin"].map(OsStr::new);
        let output = success(
            COMMAND,
            self.child().output_with_input(&args, false, bytes)?,
        )?;
        oid_of(COMMAND, &output)
    }

    /// `tree` with each top-relative path of `changes` replaced, its
    /// directories rebuilt by `mktree` (an emptied one dropped): the new
    /// tree's OID.
    pub fn replaced_tree(
        &self,
        tree: &str,
        changes: &BTreeMap<String, Replacement>,
    ) -> Result<String, GitError> {
        if changes.is_empty() {
            return Ok(tree.to_owned());
        }
        let changes: BTreeMap<&str, &Replacement> = changes
            .iter()
            .map(|(path, change)| (path.as_str(), change))
            .collect();
        match self.rebuild(Some(tree), &changes, true)? {
            Some(oid) => Ok(oid),
            None => Err(GitError::Unreadable { command: "mktree" }),
        }
    }

    /// The tree `tree` (`None`: an empty one) with `changes` (paths below
    /// it) applied; `None` when nothing is left in it, unless `top`.
    fn rebuild(
        &self,
        tree: Option<&str>,
        changes: &BTreeMap<&str, &Replacement>,
        top: bool,
    ) -> Result<Option<String>, GitError> {
        let mut entries = match tree {
            Some(tree) => self.level(tree)?,
            None => BTreeMap::new(),
        };
        let mut nested: BTreeMap<&str, BTreeMap<&str, &Replacement>> = BTreeMap::new();
        for (path, change) in changes {
            match path.split_once('/') {
                None => match change {
                    Replacement::Blob { mode, oid } => {
                        entries.insert(
                            path.as_bytes().to_vec(),
                            TreeEntry {
                                mode: mode.clone(),
                                kind: "blob".to_owned(),
                                oid: oid.clone(),
                            },
                        );
                    }
                    Replacement::Remove => {
                        entries.remove(path.as_bytes());
                    }
                },
                Some((dir, rest)) => {
                    nested.entry(dir).or_default().insert(rest, *change);
                }
            }
        }
        for (dir, below) in nested {
            let child = entries
                .get(dir.as_bytes())
                .filter(|entry| entry.kind == "tree")
                .map(|entry| entry.oid.clone());
            match self.rebuild(child.as_deref(), &below, false)? {
                Some(oid) => {
                    entries.insert(
                        dir.as_bytes().to_vec(),
                        TreeEntry {
                            mode: TREE_MODE.to_owned(),
                            kind: "tree".to_owned(),
                            oid,
                        },
                    );
                }
                None => {
                    entries.remove(dir.as_bytes());
                }
            }
        }
        if entries.is_empty() && !top {
            return Ok(None);
        }
        let mut input = Vec::new();
        for (name, entry) in &entries {
            input.extend_from_slice(
                format!("{} {} {}\t", entry.mode, entry.kind, entry.oid).as_bytes(),
            );
            input.extend_from_slice(name);
            input.push(0);
        }
        const COMMAND: &str = "mktree";
        let args = ["mktree", "-z"].map(OsStr::new);
        let output = success(
            COMMAND,
            self.child().output_with_input(&args, false, &input)?,
        )?;
        oid_of(COMMAND, &output).map(Some)
    }

    /// The merge commit of `tree` with parents `left` then `right` and
    /// `message`, under the checkout's identity or the process's
    /// `GIT_AUTHOR_*`/`GIT_COMMITTER_*`, never guessed (`-c
    /// user.useConfigOnly=true`; `var` asked first, `commit-tree
    /// --no-gpg-sign … -F -`, so it signs nothing whatever the config): its
    /// OID. No ref moves.
    pub fn commit_merge(
        &self,
        tree: &str,
        left: &str,
        right: &str,
        message: &str,
    ) -> Result<String, CommitFailure> {
        for ident in ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"] {
            let output = self
                .output(&["-c", "user.useConfigOnly=true", "var", ident], true)
                .map_err(CommitFailure::Git)?;
            if !output.status.success() {
                return Err(CommitFailure::Identity(cut_stderr(
                    String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                )));
            }
        }
        const COMMAND: &str = "commit-tree";
        let args = [
            "-c",
            "user.useConfigOnly=true",
            "commit-tree",
            "--no-gpg-sign",
            tree,
            "-p",
            left,
            "-p",
            right,
            "-F",
            "-",
        ]
        .map(OsStr::new);
        let output = self
            .child()
            .output_with_input(&args, false, message.as_bytes())
            .map_err(CommitFailure::Git)?;
        let output = success(COMMAND, output).map_err(CommitFailure::Git)?;
        oid_of(COMMAND, &output).map_err(CommitFailure::Git)
    }

    /// `merge --ff-only --no-autostash <commit>`: `HEAD`'s branch, index
    /// and worktree moved to `commit`, a descendant of `HEAD`; `post-merge`
    /// runs.
    pub fn fast_forward(&self, commit: &str) -> Result<(), GitError> {
        const COMMAND: &str = "merge";
        let output = self.output(&["merge", "--ff-only", "--no-autostash", commit], false)?;
        success(COMMAND, output).map(|_| ())
    }

    /// `diff --name-only -z --no-renames <left> <right> --` (no colour, no
    /// external diff), under the deadline: the top-relative paths that
    /// differ between two commits, in git's order, and how many names it
    /// left out because they are not UTF-8 (no walk lists one, but the
    /// caller cannot tell what they moved). The `read-trust` spec, "Rules
    /// and edge cases" R11: the merge's update of each root's index.
    pub fn changed(&self, left: &str, right: &str) -> Result<ChangedPaths, GitError> {
        const COMMAND: &str = "diff";
        let output = self.read(
            COMMAND,
            &[
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--name-only",
                "-z",
                "--no-renames",
                left,
                right,
                "--",
            ],
        )?;
        let mut changed = ChangedPaths::default();
        for name in output.split(|&byte| byte == 0) {
            if name.is_empty() {
                continue;
            }
            match std::str::from_utf8(name) {
                Ok(path) => changed.paths.push(path.to_owned()),
                Err(_) => changed.skipped_names += 1,
            }
        }
        Ok(changed)
    }

    /// `push --porcelain --no-follow-tags --recurse-submodules=no <r>
    /// refs/heads/<b>:refs/heads/<m>`, read from its ref line: accepted
    /// only when git also ended cleanly.
    pub fn push(&self, upstream: &Upstream) -> Pushed {
        let from = format!("refs/heads/{}", upstream.branch);
        let to = format!("refs/heads/{}", upstream.merge);
        let refspec = format!("{from}:{to}");
        let args = [
            PUSH,
            "--porcelain",
            "--no-follow-tags",
            "--recurse-submodules=no",
            upstream.remote.as_str(),
            refspec.as_str(),
        ];
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let output = match self.child().network_output(&args) {
            Ok(output) => output,
            Err(GitError::Deadline) => return Pushed::Deadline,
            Err(error) => return Pushed::Failed(upstream.redacted(&error.to_string())),
        };
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let line = stdout.lines().find_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let (Some(flag), Some(names), summary) = (fields.next(), fields.next(), fields.next())
            else {
                return None;
            };
            (names == refspec).then(|| (flag.to_owned(), summary.unwrap_or("").to_owned()))
        });
        let stderr = spoken(upstream, &output);
        match line {
            Some((flag, _)) if matches!(flag.as_str(), " " | "*" | "=" | "+") => {
                if output.status.success() {
                    Pushed::Accepted
                } else {
                    Pushed::Failed(stderr)
                }
            }
            Some((flag, summary)) if flag == "!" => {
                if summary.starts_with("[rejected]") {
                    Pushed::NotFastForward
                } else if let Some(rest) = summary.strip_prefix("[remote rejected]") {
                    let reason = rest
                        .trim()
                        .strip_prefix('(')
                        .and_then(|rest| rest.strip_suffix(')'))
                        .unwrap_or(rest.trim())
                        .to_owned();
                    Pushed::Refused {
                        reason: cut_stderr(upstream.redacted(&reason)),
                        stderr,
                    }
                } else {
                    Pushed::Failed(stderr)
                }
            }
            _ => Pushed::Failed(stderr),
        }
    }
}

/// `ahead` and `behind` of the checkout at `top` against its tracking ref
/// (`rev-list --left-right --count refs/remotes/<r>/<m>...HEAD`, read-only,
/// each child killed past `deadline`); `None` with no branch, no upstream,
/// no tracking ref, or a git failure.
pub fn ahead_behind(top: &Path, env: &GitEnv, deadline: Duration) -> Option<(u64, u64)> {
    let sync = SyncGit::open(top, env, deadline).ok()?;
    let branch = sync.child().branch().ok()??;
    let remote = sync.config(&format!("branch.{branch}.remote")).ok()??;
    let merge = sync.config(&format!("branch.{branch}.merge")).ok()??;
    let merge = merge.strip_prefix("refs/heads/")?;
    if remote.is_empty() || remote == "." || merge.is_empty() {
        return None;
    }
    let tracking = format!("refs/remotes/{remote}/{merge}");
    sync.child().commit_of(&tracking).ok()??;
    let range = format!("{tracking}...HEAD");
    let output = sync
        .read("rev-list", &["rev-list", "--left-right", "--count", &range])
        .ok()?;
    let text = String::from_utf8_lossy(one_line(&output)?).into_owned();
    let (behind, ahead) = text.split_once('\t')?;
    Some((ahead.trim().parse().ok()?, behind.trim().parse().ok()?))
}

/// The URL `url` holds a credential: a password in any scheme
/// (`user:pass@`), or any user name on `http`, `https`, `ftp`, `ftps`
/// (`user@`). A `<transport>::<url>` form is judged by its URL; an ssh user
/// name (`ssh://git@host/x`, `git@host:x`) passes.
pub fn holds_secret(url: &str) -> bool {
    let mut rest = url;
    loop {
        let Some(at) = rest.find("://") else {
            return false;
        };
        let before = &rest[..at];
        // `<transport>::<scheme>://…` or `<scheme>://…`.
        let scheme = before[scheme_start(before)..].to_ascii_lowercase();
        let after = &rest[at + 3..];
        let authority_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..authority_end];
        if let Some(at_sign) = authority.rfind('@') {
            let user_info = &authority[..at_sign];
            if user_info.contains(':') {
                return true;
            }
            let base = scheme.rsplit('+').next().unwrap_or(&scheme);
            if NAMELESS_SCHEMES.contains(&base) || NAMELESS_SCHEMES.contains(&scheme.as_str()) {
                return true;
            }
        }
        rest = after;
    }
}

/// Where the scheme that ends `before` starts: just after its last
/// character that can be no part of a scheme, stepped by characters, so a
/// multi-byte one before it is never cut.
fn scheme_start(before: &str) -> usize {
    before
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')))
        .map_or(0, |(at, c)| at + c.len_utf8())
}

/// Git's words of a failed network child: its stderr (else its stdout),
/// URLs taken out, cut at 4 KiB.
fn spoken(upstream: &Upstream, output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if text.is_empty() && !output.status.success() {
        text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    }
    cut_stderr(upstream.redacted(&text))
}

/// The one OID `output` holds.
fn oid_of(command: &'static str, output: &[u8]) -> Result<String, GitError> {
    one_line(output)
        .filter(|oid| is_oid(oid))
        .map(|oid| String::from_utf8_lossy(oid).into_owned())
        .ok_or(GitError::Unreadable { command })
}

/// At most ten of `paths`, comma-separated, then `and <k> more`.
pub fn named(paths: &[&str]) -> String {
    let shown = paths
        .iter()
        .take(NAMED_MAX)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > NAMED_MAX {
        format!("{shown} and {} more", paths.len() - NAMED_MAX)
    } else {
        shown
    }
}

/// The mode a rendered blob takes at a path whose entry is `entry`: its
/// own for a regular file, else a plain file's.
pub fn file_mode(entry: Option<&TreeEntry>) -> String {
    match entry {
        Some(entry)
            if entry.kind == "blob" && (entry.mode == "100755" || entry.mode == FILE_MODE) =>
        {
            entry.mode.clone()
        }
        _ => FILE_MODE.to_owned(),
    }
}

/// The top of the worktree holding `dir` and its repository's common
/// directory, both canonical: the key of one sync.
pub fn sync_place(dir: &Path, env: &GitEnv) -> Result<(PathBuf, PathBuf), GitError> {
    WorktreeGit::new(dir, env)?.top_and_common_dir()
}

#[cfg(test)]
mod tests {
    //! docs/features/hub-git.md (RM-0051) P24, at the unit: the URL rule
    //! and the redaction of git's words over non-ASCII text never slice
    //! inside a character, and replace each whitespace-delimited token
    //! holding a URL whole: the remote's own by its name, any other by
    //! `<url>`, a relative one (`.`) only as an exact token. The daemon's
    //! half is `specengine-http`'s `tests/hub_git.rs`, the public half
    //! `tests/sync_urls.rs`. M: byte-index slicing.
    //!
    //! Non-Latin characters in Rust sources are Unicode escapes.

    use super::{Upstream, holds_secret, scheme_start};

    fn upstream(urls: &[&str]) -> Upstream {
        Upstream {
            branch: "main".to_owned(),
            remote: "origin".to_owned(),
            merge: "main".to_owned(),
            urls: urls.iter().map(|url| (*url).to_owned()).collect(),
        }
    }

    #[test]
    fn a_url_token_after_non_ascii_text_goes_whole() {
        let shown = upstream(&["/srv/origin.git"]).redacted(
            "remote: refus\u{e9} https://u:p@example.invalid/r.git \
             x\u{e9}://u:p@example.invalid/q.git \
             na\u{ef}vehttps://u:p@example.invalid/s.git fin\n\
             error: failed to push some refs to '/srv/origin.git'",
        );
        let (first, second) = shown.split_once('\n').expect("two lines");
        assert_eq!(first, "remote: refus\u{e9} <url> <url> <url> fin");
        assert!(
            !second.contains("/srv/origin.git") && second.ends_with("origin"),
            "{second:?}"
        );
        assert_eq!(
            upstream(&["/srv/origin.git"]).redacted("caf\u{e9}  cr\u{e8}me\tbr\u{fb}l\u{e9}e\n"),
            "caf\u{e9}  cr\u{e8}me\tbr\u{fb}l\u{e9}e\n",
            "words and the whitespace between them kept"
        );
    }

    #[test]
    fn a_relative_url_takes_only_an_exact_token() {
        assert_eq!(
            upstream(&["."]).redacted("pushed to . and ./x and a.b"),
            "pushed to origin and ./x and a.b"
        );
    }

    #[test]
    fn a_credential_is_judged_whatever_precedes_the_scheme() {
        assert!(holds_secret("x\u{e9}://u:p@example.invalid/q.git"));
        assert!(holds_secret("\u{e9}https://tok@example.invalid/r.git"));
        assert!(holds_secret("na\u{ef}vehttps://u:p@example.invalid/s.git"));
        assert!(!holds_secret("\u{e9}://example.invalid/x"));
        assert!(!holds_secret("ssh://git@example.invalid/r.git"));
        assert_eq!(scheme_start("x\u{e9}"), "x\u{e9}".len());
        assert_eq!(scheme_start("\u{e9}https"), "\u{e9}".len());
        assert_eq!(scheme_start("https"), 0);
    }
}
