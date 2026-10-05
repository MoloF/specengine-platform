//! docs/features/proposal-apply.md, the store's git write side
//! (`WorktreeGit`, `replace_file`, `same_repository`) as apply steps 2–10
//! and creation 5 use it, on scratch repositories with a linked worktree:
//! the binding (`place`) and its refusals, the caller's local `GIT_*`
//! variables dropped, `is_tracked` / `is_dirty` with literal pathspecs,
//! operations in progress, the committer identity, `merge-file`, the diff
//! hunks, `commit --only` with a verbatim message and hooks, parents,
//! changed paths, trailers, and the atomic replace keeping the mode. Every
//! git process runs in the sandbox of the CLI tests' `common::git`.
//! Iteration 2 (review of iteration 1): a failure that git explains on
//! stdout only carries that text; `merge-file` and `diff` ignore a global
//! and a system git config; a revision starting with `-` is never an
//! option (`--end-of-options`); the replace's temp sibling is created with
//! the target's mode. Iteration 4: `blob_at` reads a path's bytes in a
//! commit's tree as stored and refuses a directory-relative path.
//! Iteration 5: `has_path` says whether a commit's tree holds a path and
//! refuses the same paths. Iteration 6: `branch_commits_with_trailer`
//! reads a branch's whole history (the proposal's base commit pruned).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};

use common::{Scratch, write};
use git::Sandbox;
use specengine_store::{
    GitEnv, GitError, Merge, Operation, PlaceError, WorktreeGit, replace_file, same_repository,
};

/// A repository at `<scratch>/main` (two files, one commit on `main`)
/// and a linked worktree at `<scratch>/t1` on `t1`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    linked: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.join("main");
        write(&main, "docs/a.md", "# A\n\nOne.\n");
        write(&main, "docs/b.md", "# B\n\nTwo.\n");
        write(&main, "proj/specengine.toml", "[project]\nslug = \"p\"\n");
        git.init(&main);
        let main = fs::canonicalize(&main).unwrap();
        git.add_all(&main);
        git.commit(&main, "first");
        let linked = scratch.join("t1");
        git.git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "t1",
                linked.to_str().unwrap(),
            ],
        );
        let linked = fs::canonicalize(linked).unwrap();
        Self {
            scratch,
            git,
            main,
            linked,
        }
    }

    fn env(&self) -> GitEnv {
        GitEnv::new(&self.main, self.git.vars())
    }

    fn handle(&self, dir: &Path) -> WorktreeGit {
        WorktreeGit::new(dir, &self.env()).expect("git runs")
    }

    /// A scratch directory for git's temp files (the data directory's role).
    fn data(&self) -> PathBuf {
        let dir = self.scratch.join("data");
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}

#[test]
fn the_place_binds_the_linked_worktree_its_branch_and_head() {
    let repo = Repo::new("wt-place");
    let git = repo.handle(&repo.linked);
    assert_eq!(git.top().unwrap(), repo.linked);
    let common = git.common_dir().unwrap();
    assert_eq!(common, fs::canonicalize(repo.main.join(".git")).unwrap());
    assert_eq!(repo.handle(&repo.main).common_dir().unwrap(), common);
    assert_eq!(git.branch().unwrap().as_deref(), Some("t1"));
    let head = repo.git.git_text(&repo.linked, &["rev-parse", "HEAD"]);
    assert_eq!(git.head().unwrap().as_deref(), Some(head.as_str()));
    assert_eq!(
        git.commit_of("refs/heads/t1").unwrap().as_deref(),
        Some(head.as_str())
    );
    assert_eq!(git.commit_of("refs/heads/nope").unwrap(), None);

    let place = git.place(&repo.linked).unwrap();
    assert_eq!(place.worktree, repo.linked.to_str().unwrap());
    assert_eq!(place.root_rel, "");
    assert_eq!(place.branch, "t1");
    assert_eq!(place.base_commit, head);
    assert_eq!(Path::new(&place.git_common_dir), common);
    let place = repo
        .handle(&repo.linked.join("proj"))
        .place(&repo.linked.join("proj"))
        .unwrap();
    assert_eq!(place.root_rel, "proj");
    assert_eq!(place.worktree, repo.linked.to_str().unwrap());

    // The caller's local variables never reach git: a handle with GIT_DIR,
    // GIT_WORK_TREE and GIT_INDEX_FILE of another repository still sees
    // its own directory's.
    let other = repo.scratch.join("other");
    repo.git.init(&other);
    let other = fs::canonicalize(other).unwrap();
    let env = repo
        .env()
        .with_var("GIT_DIR", other.join(".git"))
        .with_var("GIT_WORK_TREE", &other)
        .with_var("GIT_INDEX_FILE", other.join(".git/index"))
        .with_var("GIT_COMMON_DIR", other.join(".git"));
    let git = WorktreeGit::new(&repo.linked, &env).unwrap();
    assert!(
        git.vars().all(|(name, _)| {
            ![
                "GIT_DIR",
                "GIT_WORK_TREE",
                "GIT_INDEX_FILE",
                "GIT_COMMON_DIR",
            ]
            .contains(&name.to_str().unwrap())
        }),
        "the local variables are dropped"
    );
    assert!(
        git.vars()
            .any(|(name, value)| name == "GIT_TERMINAL_PROMPT" && value == "0")
    );
    assert_eq!(git.top().unwrap(), repo.linked);
    assert_eq!(git.common_dir().unwrap(), common);

    // Detached, unborn, outside any repository.
    repo.git.git(&repo.linked, &["checkout", "-q", "--detach"]);
    assert_eq!(repo.handle(&repo.linked).branch().unwrap(), None);
    assert!(matches!(
        repo.handle(&repo.linked).place(&repo.linked),
        Err(PlaceError::Detached)
    ));
    assert!(matches!(
        repo.handle(&other).place(&other),
        Err(PlaceError::Unborn)
    ));
    let nowhere = repo.scratch.join("nowhere");
    fs::create_dir_all(&nowhere).unwrap();
    assert!(repo.handle(&nowhere).top().is_err());
    assert!(repo.handle(&nowhere).place(&nowhere).is_err());
}

#[test]
fn tracked_and_dirty_take_paths_literally() {
    let repo = Repo::new("wt-dirty");
    let git = repo.handle(&repo.linked);
    assert!(git.is_tracked("docs/a.md").unwrap());
    assert!(!git.is_dirty("docs/a.md").unwrap());
    // A glob-shaped name is no pattern.
    assert!(!git.is_tracked("docs/*.md").unwrap());
    assert!(!git.is_dirty("docs/?.md").unwrap());
    write(&repo.linked, "docs/a.md", "# A\n\nChanged.\n");
    assert!(git.is_dirty("docs/a.md").unwrap());
    assert!(!git.is_dirty("docs/b.md").unwrap());
    repo.git.git(&repo.linked, &["add", "docs/a.md"]);
    assert!(git.is_dirty("docs/a.md").unwrap(), "staged");
    write(&repo.linked, "docs/new.md", "# New\n");
    assert!(!git.is_tracked("docs/new.md").unwrap());
    assert!(git.is_dirty("docs/new.md").unwrap(), "untracked");
    // The main worktree knows nothing of it.
    assert!(!repo.handle(&repo.main).is_dirty("docs/a.md").unwrap());
}

#[test]
fn an_operation_in_progress_is_named() {
    let repo = Repo::new("wt-operation");
    let git = repo.handle(&repo.linked);
    assert_eq!(git.operation_in_progress().unwrap(), None);
    repo.git
        .git(&repo.linked, &["checkout", "-q", "-b", "side"]);
    write(&repo.linked, "docs/b.md", "# B\n\nSide.\n");
    repo.git.add_all(&repo.linked);
    repo.git.commit(&repo.linked, "side");
    repo.git.git(&repo.linked, &["checkout", "-q", "t1"]);
    write(&repo.linked, "docs/b.md", "# B\n\nOurs.\n");
    repo.git.add_all(&repo.linked);
    repo.git.commit(&repo.linked, "ours");
    let merge = repo
        .git
        .git_output(&repo.linked, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success());
    assert_eq!(git.operation_in_progress().unwrap(), Some(Operation::Merge));
    assert_eq!(
        repo.handle(&repo.main).operation_in_progress().unwrap(),
        None,
        "per worktree"
    );
    repo.git.git(&repo.linked, &["merge", "--abort"]);
    let pick = repo
        .git
        .git_output(&repo.linked, &["cherry-pick", "side"], &[]);
    assert!(!pick.status.success());
    assert_eq!(
        git.operation_in_progress().unwrap(),
        Some(Operation::CherryPick)
    );
    repo.git.git(&repo.linked, &["cherry-pick", "--abort"]);
    assert_eq!(git.operation_in_progress().unwrap(), None);
}

#[test]
fn the_committer_identity_and_its_absence() {
    let repo = Repo::new("wt-ident");
    assert_eq!(
        repo.handle(&repo.linked).committer_ident().unwrap(),
        "Scratch Committer <committer@example.invalid>"
    );
    repo.git
        .git(&repo.main, &["config", "user.useConfigOnly", "true"]);
    let env = repo
        .env()
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let git = WorktreeGit::new(&repo.linked, &env).unwrap();
    assert!(
        git.committer_ident().is_err(),
        "{:?}",
        git.committer_ident()
    );
}

#[test]
fn merge_file_and_diff_hunks_over_scratch_files() {
    let repo = Repo::new("wt-merge");
    let git = repo.handle(&repo.linked);
    let data = repo.data();
    let base = b"one\ntwo\nthree\nfour\nfive\n";
    let current = b"ONE\ntwo\nthree\nfour\nfive\n";
    let proposed = b"one\ntwo\nthree\nfour\nFIVE\n";
    assert_eq!(
        git.merge_file(&data, current, base, proposed).unwrap(),
        Merge::Clean(b"ONE\ntwo\nthree\nfour\nFIVE\n".to_vec())
    );
    let overlapping = b"uno\ntwo\nthree\nfour\nfive\n";
    match git.merge_file(&data, current, base, overlapping).unwrap() {
        Merge::Conflict { text, conflicts } => {
            assert_eq!(conflicts, 1);
            let text = String::from_utf8(text).unwrap();
            assert!(text.contains("<<<<<<< current\nONE\n"), "{text}");
            assert!(text.contains(">>>>>>> proposed\n"), "{text}");
        }
        other => panic!("{other:?}"),
    }
    // Spans end without a line end: the merge keeps that.
    assert_eq!(
        git.merge_file(&data, b"A\nb\nc", b"a\nb\nc", b"a\nb\nC")
            .unwrap(),
        Merge::Clean(b"A\nb\nC".to_vec())
    );
    assert!(git.diff_hunks(&data, base, base).unwrap().is_empty());
    let hunks = String::from_utf8(git.diff_hunks(&data, base, current).unwrap()).unwrap();
    assert!(hunks.starts_with("@@ -1"), "{hunks}");
    assert!(hunks.contains("\n-one\n+ONE\n"), "{hunks}");
    assert!(
        !hunks.contains("---") && !hunks.contains("+++"),
        "no headers: {hunks}"
    );
    let left: Vec<_> = fs::read_dir(&data).unwrap().collect();
    assert!(left.is_empty(), "scratch files removed: {left:?}");
}

#[test]
fn commit_only_takes_one_path_a_verbatim_message_and_runs_hooks() {
    let repo = Repo::new("wt-commit");
    let git = repo.handle(&repo.linked);
    let head = git.head().unwrap().unwrap();
    write(&repo.linked, "docs/b.md", "# B\n\nStaged.\n");
    repo.git.git(&repo.linked, &["add", "docs/b.md"]);
    write(&repo.linked, "docs/a.md", "# A\n\nApplied.\n");
    let message = "spec: apply PR-0007\n\n# not a comment\n  indented  \n\nProposal: PR-0007\nProposal: PR-0008\n";
    git.commit_only(&repo.data(), message, "docs/a.md").unwrap();
    let new = git.head().unwrap().unwrap();
    assert_eq!(git.parents(&new).unwrap(), std::slice::from_ref(&head));
    assert_eq!(git.changed_paths(&head, &new).unwrap(), ["docs/a.md"]);
    let body = repo.git.git(&repo.linked, &["log", "-1", "--format=%B"]);
    assert_eq!(String::from_utf8(body).unwrap(), format!("{message}\n"));
    assert_eq!(
        git.trailer_values(&new, "Proposal").unwrap(),
        ["PR-0007", "PR-0008"]
    );
    assert_eq!(
        git.commits_with_trailer(&head, "t1", "Proposal", "PR-0007")
            .unwrap(),
        std::slice::from_ref(&new)
    );
    assert!(
        git.commits_with_trailer(&new, "t1", "Proposal", "PR-0007")
            .unwrap()
            .is_empty()
    );
    assert!(
        git.commits_with_trailer(&head, "t1", "Proposal", "PR-0009")
            .unwrap()
            .is_empty()
    );
    let staged = repo
        .git
        .git_text(&repo.linked, &["diff", "--cached", "--name-only"]);
    assert_eq!(staged, "docs/b.md", "the other staged path stays staged");
    assert_eq!(
        repo.git.git_text(&repo.main, &["rev-parse", "HEAD"]),
        head,
        "main not moved"
    );

    // A failing hook: the commit fails with its stderr, nothing moves.
    let hooks = repo.main.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(
        hooks.join("pre-commit"),
        "#!/bin/sh\necho hook says no >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).unwrap();
    write(&repo.linked, "docs/a.md", "# A\n\nAgain.\n");
    let error = git
        .commit_only(&repo.data(), "spec: apply PR-0009\n", "docs/a.md")
        .unwrap_err();
    assert!(error.to_string().contains("hook says no"), "{error}");
    assert_eq!(git.head().unwrap().unwrap(), new);
}

#[test]
fn replace_file_is_atomic_keeps_the_mode_and_refuses_a_symlink() {
    let scratch = Scratch::new("wt-replace");
    let dir = scratch.join("d");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("x.md");
    fs::write(&file, "old\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    replace_file(&file, b"new\n").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"new\n");
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["x.md"], "no temp sibling left");

    let outside = scratch.join("outside.md");
    fs::write(&outside, "outside\n").unwrap();
    let link = dir.join("link.md");
    symlink(&outside, &link).unwrap();
    assert!(replace_file(&link, b"through the link\n").is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"outside\n");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(replace_file(&dir.join("missing.md"), b"x").is_err());
    assert!(!dir.join("missing.md").exists());

    assert!(same_repository(dir.to_str().unwrap(), &dir));
    let alias = scratch.join("alias");
    symlink(&dir, &alias).unwrap();
    assert!(same_repository(dir.to_str().unwrap(), &alias));
    assert!(!same_repository(
        dir.to_str().unwrap(),
        &scratch.join("outside.md")
    ));
}

/// `git commit --only` of an unchanged path fails, and git says why on
/// stdout only ("nothing to commit"): the error carries that text (review
/// of iteration 1: "git commit failed (exit status 1)" with no reason).
/// M: `failed` reads stderr only.
#[test]
fn a_failure_explained_on_stdout_only_carries_that_text() {
    let repo = Repo::new("wt-stdout");
    let git = repo.handle(&repo.linked);
    let head = git.head().unwrap().unwrap();
    let error = git
        .commit_only(&repo.data(), "spec: apply PR-0001\n", "docs/a.md")
        .unwrap_err();
    let text = error.to_string();
    assert!(
        text.starts_with("`git commit` failed (exit status 1): "),
        "{text}"
    );
    assert!(text.contains("to commit"), "git's stdout reason: {text}");
    assert!(text.contains("On branch t1"), "{text}");
    match error {
        specengine_store::GitError::Failed { stderr, .. } => {
            assert!(!stderr.is_empty() && !stderr.ends_with('\n'), "{stderr:?}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(git.head().unwrap().unwrap(), head, "nothing committed");
    let left: Vec<_> = fs::read_dir(repo.data()).unwrap().collect();
    assert!(left.is_empty(), "the message file removed: {left:?}");
}

/// `merge-file` and `diff` read neither a global nor a system git config
/// (review of iteration 1: the preview and the conflict text depended on
/// the user's config): with `merge.conflictStyle = diff3` and
/// `diff.suppressBlankEmpty = true` in a `GIT_CONFIG_GLOBAL` file, and
/// again in a `GIT_CONFIG_SYSTEM` file (`GIT_CONFIG_NOSYSTEM` unset), the
/// conflict has no `|||||||` base section and a blank context line keeps
/// its leading space — the same bytes as with no config. The same git with
/// the same variables shows both settings take effect when not isolated.
/// M: `GIT_CONFIG_GLOBAL=/dev/null` or `GIT_CONFIG_NOSYSTEM=1` dropped.
#[test]
fn merge_file_and_diff_ignore_a_global_and_a_system_config() {
    let repo = Repo::new("wt-config");
    let data = repo.data();
    let config = repo.scratch.join("user.gitconfig");
    fs::write(
        &config,
        "[merge]\n\tconflictStyle = diff3\n[diff]\n\tsuppressBlankEmpty = true\n",
    )
    .unwrap();
    let base = b"one\ntwo\nthree\n";
    let current = b"ONE\ntwo\nthree\n";
    let proposed = b"uno\ntwo\nthree\n";
    let old = b"a\n\nb\nc\nd\n";
    let new = b"a\n\nb\nC\nd\n";

    let plain = repo.handle(&repo.linked);
    let want_merge = plain.merge_file(&data, current, base, proposed).unwrap();
    let want_hunks = plain.diff_hunks(&data, old, new).unwrap();
    let Merge::Conflict { text, .. } = &want_merge else {
        panic!("{want_merge:?}")
    };
    assert!(!String::from_utf8_lossy(text).contains("|||||||"));
    assert!(
        want_hunks.windows(8).any(|w| w == b" a\n \n b\n"),
        "{want_hunks:?}"
    );

    let sandbox: Vec<(std::ffi::OsString, std::ffi::OsString)> = repo.git.vars();
    let with =
        |extra: &[(&str, &Path)], drop: &[&str]| -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
            let mut vars: Vec<_> = sandbox
                .iter()
                .filter(|(name, _)| !drop.iter().any(|gone| name == gone))
                .cloned()
                .collect();
            for (name, value) in extra {
                vars.push(((*name).into(), value.as_os_str().to_owned()));
            }
            vars
        };
    let global = with(&[("GIT_CONFIG_GLOBAL", &config)], &["GIT_CONFIG_GLOBAL"]);
    let system = with(&[("GIT_CONFIG_SYSTEM", &config)], &["GIT_CONFIG_NOSYSTEM"]);
    for (label, vars) in [("global", &global), ("system", &system)] {
        // The control: this git, these variables, not isolated, in the
        // worktree: both settings take effect.
        let write = |name: &str, bytes: &[u8]| {
            let path = repo.scratch.join(&format!("{label}-{name}"));
            fs::write(&path, bytes).unwrap();
            path
        };
        let (c, b, p) = (write("c", current), write("b", base), write("p", proposed));
        let raw = std::process::Command::new(repo.git.git_program())
            .env_clear()
            .envs(vars.iter().cloned())
            .arg("-C")
            .arg(&repo.linked)
            .args([
                "merge-file",
                "-p",
                "-L",
                "current",
                "-L",
                "base",
                "-L",
                "proposed",
            ])
            .args([&c, &b, &p])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&raw.stdout).contains("||||||| base"),
            "{label}: the control merge reads the config: {raw:?}"
        );
        let (o, n) = (write("o", old), write("n", new));
        let raw = std::process::Command::new(repo.git.git_program())
            .env_clear()
            .envs(vars.iter().cloned())
            .arg("-C")
            .arg(&repo.linked)
            .args(["diff", "--no-index", "--no-color", "-U3", "--"])
            .args([&o, &n])
            .output()
            .unwrap();
        assert!(
            raw.stdout.windows(7).any(|w| w == b" a\n\n b\n"),
            "{label}: the control diff reads the config: {raw:?}"
        );

        let git = WorktreeGit::new(&repo.linked, &GitEnv::new(&repo.main, vars.clone()))
            .expect("git runs");
        assert_eq!(
            git.merge_file(&data, current, base, proposed).unwrap(),
            want_merge,
            "{label}: merge-file"
        );
        assert_eq!(
            git.diff_hunks(&data, old, new).unwrap(),
            want_hunks,
            "{label}: diff"
        );
    }
    let left: Vec<_> = fs::read_dir(&data).unwrap().collect();
    assert!(left.is_empty(), "scratch files removed: {left:?}");
}

/// A revision a caller passes (a stored `base_commit`, a branch) is never
/// an option: `--output=<file>` as the commit of `parents`,
/// `changed_paths`, `trailer_values`, `commits_with_trailer` and
/// `commit_of` writes no file (review of iteration 1: `spec review` ran
/// `git log --output=…`). M: `--end-of-options` dropped.
#[test]
fn a_revision_starting_with_a_dash_is_never_an_option() {
    let repo = Repo::new("wt-dash");
    let git = repo.handle(&repo.linked);
    let head = git.head().unwrap().unwrap();
    let out = repo.scratch.join("out");
    fs::create_dir_all(&out).unwrap();
    let file = |name: &str| out.join(name);
    let flag = |name: &str| format!("--output={}", file(name).display());

    assert!(git.parents(&flag("parents")).is_err());
    assert!(git.changed_paths(&flag("changed-from"), &head).is_err());
    assert!(git.changed_paths(&head, &flag("changed-to")).is_err());
    assert!(git.trailer_values(&flag("trailers"), "Proposal").is_err());
    // `<from>..refs/heads/t1` as an option would write `<from>..refs/heads/t1`.
    fs::create_dir_all(out.join("range..refs/heads")).unwrap();
    assert!(
        git.commits_with_trailer(&flag("range"), "t1", "Proposal", "PR-0001")
            .map_or(true, |found| found.is_empty())
    );
    assert!(matches!(git.commit_of(&flag("commit")), Ok(None) | Err(_)));
    let written: Vec<String> = walk(&out)
        .into_iter()
        .filter(|path| !path.is_dir())
        .map(|path| path.display().to_string())
        .collect();
    assert!(written.is_empty(), "no file written: {written:?}");
    assert_eq!(git.head().unwrap().unwrap(), head);
}

/// `branch_commits_with_trailer` (iteration 6, the read when a proposal's
/// base commit is not in the repository): the commits of
/// `refs/heads/<branch>`'s whole history whose `key` trailer is `value`,
/// newest first — the ones before any base commit too, which
/// `commits_with_trailer` from that base does not see; another value:
/// none; a branch that is not there: an error. M: the read limited to a
/// range.
#[test]
fn branch_commits_with_trailer_reads_the_whole_branch_newest_first() {
    let repo = Repo::new("wt-branch-trailer");
    let git = repo.handle(&repo.linked);
    let commit = |file: &str, message: &str| {
        write(&repo.linked, file, "x\n");
        repo.git.git(&repo.linked, &["add", file]);
        repo.git.git(&repo.linked, &["commit", "-q", "-m", message]);
        git.head().unwrap().unwrap()
    };
    let oldest = commit("c1.txt", "one\n\nProposal: PR-0001");
    let middle = commit("c2.txt", "two\n\nProposal: PR-0002");
    let newest = commit("c3.txt", "three\n\nProposal: PR-0001");

    assert_eq!(
        git.branch_commits_with_trailer("t1", "Proposal", "PR-0001")
            .unwrap(),
        [newest.clone(), oldest.clone()],
        "the whole history, newest first"
    );
    assert_eq!(
        git.commits_with_trailer(&middle, "t1", "Proposal", "PR-0001")
            .unwrap(),
        std::slice::from_ref(&newest),
        "from a base: only the commits after it"
    );
    assert_eq!(
        git.branch_commits_with_trailer("t1", "Proposal", "PR-0002")
            .unwrap(),
        std::slice::from_ref(&middle)
    );
    assert!(
        git.branch_commits_with_trailer("t1", "Proposal", "PR-0003")
            .unwrap()
            .is_empty()
    );
    assert!(
        git.branch_commits_with_trailer("gone", "Proposal", "PR-0001")
            .is_err()
    );
    assert!(
        git.branch_commits_with_trailer("main", "Proposal", "PR-0001")
            .unwrap()
            .is_empty(),
        "main holds none"
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(walk(&path));
        }
        found.push(path);
    }
    found
}

/// The replace's temp sibling exists with the target's mode from its
/// creation (review of iteration 1: created with the default mode, a
/// private file was readable by others until the rename): a watcher sees
/// `.<name>.specengine-*.tmp` while a large write syncs, and every mode it
/// sees is the target's `0600` (under the process umask), never the
/// default. M: `OpenOptionsExt::mode` dropped.
#[test]
fn replace_file_creates_its_temp_sibling_with_the_targets_mode() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    let scratch = Scratch::new("wt-replace-temp");
    let dir = scratch.join("d");
    fs::create_dir_all(&dir).unwrap();
    // The process umask, read off a file created with the default mode.
    let probe = dir.join("probe");
    fs::File::create(&probe).unwrap();
    let default_mode = fs::metadata(&probe).unwrap().permissions().mode() & 0o777;
    fs::remove_file(&probe).unwrap();
    let want = 0o600 & default_mode;
    assert_ne!(want, default_mode, "a umask that tells the modes apart");

    let file = dir.join("secret.md");
    fs::write(&file, "old\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let seen = Arc::new(Mutex::new(Vec::<u32>::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (dir, seen, stop) = (dir.clone(), Arc::clone(&seen), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let Ok(entries) = fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with(".secret.md.specengine-")
                        && name.ends_with(".tmp")
                        && let Ok(meta) = fs::symlink_metadata(entry.path())
                    {
                        seen.lock().unwrap().push(meta.permissions().mode() & 0o777);
                    }
                }
            }
        })
    };
    let bytes = vec![b'x'; 16 << 20];
    for _ in 0..10 {
        replace_file(&file, &bytes).unwrap();
        if seen.lock().unwrap().len() >= 3 {
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    watcher.join().unwrap();
    let seen = seen.lock().unwrap().clone();
    assert!(!seen.is_empty(), "the watcher saw the temp sibling");
    assert!(
        seen.iter().all(|mode| *mode == want),
        "every mode seen is {want:o}: {:?}",
        seen.iter()
            .map(|mode| format!("{mode:o}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::metadata(&file).unwrap().len(), bytes.len() as u64);
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["secret.md"], "no temp sibling left");
}

/// Iteration 4 (the content check of a completing commit): `blob_at`
/// gives the bytes of a top-relative path in a commit's tree as stored —
/// no end-of-line conversion (an `eol=crlf` file checked out with CRLF is
/// read with LF), no smudge filter (checked out upper-cased, read as
/// committed), other bytes as they are — from the top and from a
/// subdirectory alike. A path missing from the tree is a failure of `git
/// cat-file` naming it. A path git would resolve against the handle's
/// directory — `./…`, `../…`, empty — is refused unread
/// (`GitError::Unreadable`), though git itself finds the file there. M:
/// the `blob_at` path check removed.
#[test]
fn blob_at_reads_stored_bytes_and_refuses_directory_relative_paths() {
    let repo = Repo::new("wt-blob");
    let dir = &repo.linked;
    write(
        dir,
        ".gitattributes",
        "docs/crlf.md text eol=crlf\ndocs/up.md filter=up\n",
    );
    repo.git
        .git(dir, &["config", "filter.up.smudge", "tr a-z A-Z"]);
    repo.git.git(dir, &["config", "filter.up.clean", "cat"]);
    write(dir, "docs/crlf.md", "one\r\ntwo\r\n");
    write(dir, "docs/up.md", "lower\n");
    let raw: &[u8] = &[0xff, 0xfe, b'\r', b'\n', 0x00, b'x', b'\r'];
    write(dir, "docs/raw.bin", raw);
    repo.git.add_all(dir);
    repo.git.commit(dir, "stored bytes");
    // Checked out afresh: the conversions apply to the files.
    for path in ["docs/crlf.md", "docs/up.md", "docs/raw.bin"] {
        fs::remove_file(dir.join(path)).unwrap();
    }
    repo.git.git(dir, &["checkout", "--", "."]);
    assert_eq!(
        fs::read(dir.join("docs/crlf.md")).unwrap(),
        b"one\r\ntwo\r\n"
    );
    assert_eq!(fs::read(dir.join("docs/up.md")).unwrap(), b"LOWER\n");

    let top = repo.handle(dir);
    let sub = repo.handle(&dir.join("docs"));
    let head = top.head().unwrap().unwrap();
    for git in [&top, &sub] {
        assert_eq!(git.blob_at(&head, "docs/crlf.md").unwrap(), b"one\ntwo\n");
        assert_eq!(git.blob_at(&head, "docs/up.md").unwrap(), b"lower\n");
        assert_eq!(git.blob_at(&head, "docs/raw.bin").unwrap(), raw);
        assert_eq!(git.blob_at(&head, "docs/a.md").unwrap(), b"# A\n\nOne.\n");
    }
    // An earlier commit's bytes, not the worktree's.
    let first = top.parents(&head).unwrap().pop().expect("one parent");
    write(dir, "docs/a.md", "# A\n\nChanged.\n");
    assert_eq!(top.blob_at(&first, "docs/a.md").unwrap(), b"# A\n\nOne.\n");

    let error = top.blob_at(&head, "docs/none.md").unwrap_err();
    assert!(
        matches!(
            error,
            GitError::Failed {
                command: "cat-file",
                ..
            }
        ),
        "{error:?}"
    );
    assert!(error.to_string().contains("docs/none.md"), "{error}");

    // git resolves `./` and `../` against the directory: the file is there.
    assert_eq!(
        repo.git.git(
            &dir.join("docs"),
            &["cat-file", "blob", &format!("{head}:./a.md")]
        ),
        b"# A\n\nOne.\n"
    );
    assert_eq!(
        repo.git.git(
            &dir.join("proj"),
            &["cat-file", "blob", &format!("{head}:../docs/a.md")]
        ),
        b"# A\n\nOne.\n"
    );
    let proj = repo.handle(&dir.join("proj"));
    for (git, path) in [
        (&sub, "./a.md"),
        (&top, "./docs/a.md"),
        (&proj, "../docs/a.md"),
        (&sub, "../docs/a.md"),
        (&top, ""),
        (&sub, ""),
    ] {
        let refused = git.blob_at(&head, path);
        assert!(
            matches!(
                refused,
                Err(GitError::Unreadable {
                    command: "cat-file"
                })
            ),
            "{path:?}: {refused:?}"
        );
    }
}

/// Iteration 5 (a commit tree without its `specengine.toml`): `has_path`
/// says whether a commit's tree holds a top-relative path — a file or a
/// directory: true; a path not in the tree, or a commit git does not
/// have: false (not an error) — from the top and from a subdirectory
/// alike, and for an earlier commit as it was then. A path git would
/// resolve against the handle's directory — `./…`, `../…`, empty — is
/// refused unread (`GitError::Unreadable`, `rev-parse`), though git itself
/// finds it there. M: the `has_path` path check removed.
#[test]
fn has_path_answers_for_files_and_directories_and_refuses_directory_relative_paths() {
    let repo = Repo::new("wt-has-path");
    let dir = &repo.linked;
    let top = repo.handle(dir);
    let sub = repo.handle(&dir.join("docs"));
    let proj = repo.handle(&dir.join("proj"));
    let first = top.head().unwrap().unwrap();
    repo.git.git(dir, &["rm", "-q", "docs/b.md"]);
    write(dir, "docs/c.md", "# C\n");
    repo.git.add_all(dir);
    repo.git.commit(dir, "b out, c in");
    let head = top.head().unwrap().unwrap();
    let unknown = "0123456789abcdef0123456789abcdef01234567";

    for git in [&top, &sub, &proj] {
        assert!(git.has_path(&head, "docs/a.md").unwrap(), "a file");
        assert!(git.has_path(&head, "docs/c.md").unwrap(), "a new file");
        assert!(git.has_path(&head, "docs").unwrap(), "a directory");
        assert!(
            git.has_path(&head, "proj/specengine.toml").unwrap(),
            "a file below the top"
        );
        assert!(!git.has_path(&head, "docs/b.md").unwrap(), "removed");
        assert!(!git.has_path(&head, "specengine.toml").unwrap(), "missing");
        assert!(!git.has_path(&head, "docs/a.md/x").unwrap(), "below a file");
        assert!(git.has_path(&first, "docs/b.md").unwrap(), "then there");
        assert!(!git.has_path(&first, "docs/c.md").unwrap(), "not yet");
        assert!(
            !git.has_path(unknown, "docs/a.md").unwrap(),
            "a commit git does not have"
        );
    }

    // git resolves `./`, `../` and an empty path against the directory.
    let resolves = |cwd: &Path, object: &str| {
        repo.git
            .git_output(cwd, &["rev-parse", "--verify", "-q", object], &[])
            .status
            .success()
    };
    assert!(resolves(&dir.join("docs"), &format!("{head}:./a.md")));
    assert!(resolves(&dir.join("proj"), &format!("{head}:../docs/a.md")));
    assert!(resolves(dir, &format!("{head}:")));
    for (git, path) in [
        (&sub, "./a.md"),
        (&top, "./docs/a.md"),
        (&proj, "../docs/a.md"),
        (&sub, "../docs/a.md"),
        (&top, ""),
        (&sub, ""),
        (&top, "./missing.md"),
    ] {
        let refused = git.has_path(&head, path);
        assert!(
            matches!(
                refused,
                Err(GitError::Unreadable {
                    command: "rev-parse"
                })
            ),
            "{path:?}: {refused:?}"
        );
    }
}
