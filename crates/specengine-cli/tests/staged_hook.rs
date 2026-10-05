//! AC-07 of docs/features/spec-cli-staged.md: `spec check --staged` as a
//! real `pre-commit` hook. The config lives in `proj/`; `core.hooksPath`
//! of the scratch repository points at a hook running
//! `"$SPEC" check --staged --root proj` (`$SPEC` = the built binary). A
//! staged error, `commit -a` of an error only on disk and `commit -o` of a
//! document staged clean but erroneous on disk are refused; a clean commit
//! and `commit -o` of a clean document beside a staged erroneous one are
//! recorded; the same in a `git worktree add` worktree; `--no-verify`
//! commits. At the top level `GIT_DIR=.git`, `GIT_INDEX_FILE=.git/index`
//! change nothing, and a `GIT_INDEX_FILE` naming a missing file is exit 2.
//! A hook running `cd proj` first: recorded in the main worktree; in a
//! linked worktree git exports `GIT_DIR` alone, `proj` lies below the top
//! git finds without it, and the guard refuses every commit (exit 2, one
//! cause at `.`), with or without a project at the top, also with
//! `GIT_CEILING_DIRECTORIES` = the linked top and with `proj` a submodule
//! holding its own project. A hook from the top with `--root proj` passes
//! the guard in a linked worktree.

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use common::check::{FAR, baseline_covering, dangling_document, library};
use common::git::Sandbox;
use common::staged::{Superproject, assert_same, spec_in};
use common::{FIXTURES, SPEC, Scratch, copy_dir, copy_file, fixture, write};

fn clean_document(title: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# {title}\n\nText.\n")
}

const TRACKED: &str = "proj/docs/spec/zz-tracked.md";
const OTHER: &str = "proj/docs/spec/zz-other.md";

/// The hook's outcome.
#[derive(Debug, PartialEq, Eq)]
enum Commit {
    Recorded,
    Refused,
}

struct Hooked {
    scratch: Scratch,
    git: Sandbox,
    top: PathBuf,
}

impl Hooked {
    /// A repository with a copy of `name` in `proj/` (its baseline covering
    /// every blocking finding, `zz-tracked.md` clean), one commit, and
    /// `core.hooksPath` at the `pre-commit` hook.
    fn new(name: &str) -> Self {
        Self::with_hook(
            name,
            "#!/bin/sh\nexec \"$SPEC\" check --staged --root proj\n",
        )
    }

    /// As [`Hooked::new`], the `pre-commit` hook being `script`.
    fn with_hook(name: &str, script: &str) -> Self {
        let scratch = Scratch::new("staged-hook");
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        let proj = top.join("proj");
        copy_dir(&fixture(name), &proj);
        write(&top, TRACKED, clean_document("Tracked"));
        let blocked = library(&proj);
        write(&proj, ".spec-debt.toml", baseline_covering(&blocked, FAR));
        write(&top, "README.txt", "outside the root\n");
        git.init(&top);
        git.add_all(&top);
        git.commit(&top, "base");

        let hooks = scratch.dir("hooks");
        let hook = hooks.join("pre-commit");
        fs::write(&hook, script).unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        git.git(&top, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        Self { scratch, git, top }
    }

    /// `git commit -m <message> <args>` in `dir`, the hook finding `spec`
    /// through `$SPEC`.
    fn commit(&self, dir: &Path, message: &str, args: &[&str]) -> (Commit, String) {
        self.commit_with(dir, message, args, &[])
    }

    /// As [`Hooked::commit`], `git commit` (and so its hook) given the
    /// variables `extra` as well.
    fn commit_with(
        &self,
        dir: &Path,
        message: &str,
        args: &[&str],
        extra: &[(&str, &OsStr)],
    ) -> (Commit, String) {
        let before = self.git.head(dir);
        let mut full = vec!["commit", "-q", "-m", message];
        full.extend_from_slice(args);
        let mut vars = vec![("SPEC", OsStr::new(SPEC))];
        vars.extend_from_slice(extra);
        let output = self.git.git_output(dir, &full, &vars);
        let after = self.git.head(dir);
        let shown = format!(
            "git {full:?}: {}\n--- stdout\n{}--- stderr\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let outcome = if output.status.success() {
            assert_ne!(before, after, "{shown}");
            Commit::Recorded
        } else {
            assert_eq!(before, after, "{shown}");
            Commit::Refused
        };
        (outcome, shown)
    }

    fn git(&self, dir: &Path, args: &[&str]) {
        self.git.git(dir, args);
    }

    /// The five hook scenarios in the working tree `dir`.
    fn scenarios(&self, name: &str, dir: &Path, place: &str) {
        let context = |what: &str| format!("{name} {place}: {what}");

        // Refused: a staged error.
        write(dir, OTHER, dangling_document(name));
        self.git(dir, &["add", OTHER]);
        let (outcome, shown) = self.commit(dir, "a staged error", &[]);
        assert_eq!(
            outcome,
            Commit::Refused,
            "{}\n{shown}",
            context("staged error")
        );
        assert!(
            shown.contains(" — blocked"),
            "{}\n{shown}",
            context("the report")
        );
        self.git(dir, &["rm", "-q", "--cached", OTHER]);
        fs::remove_file(dir.join(OTHER)).unwrap();

        // Refused: `commit -a`, the error only on disk.
        write(dir, TRACKED, dangling_document(name));
        let (outcome, shown) = self.commit(dir, "commit -a", &["-a"]);
        assert_eq!(
            outcome,
            Commit::Refused,
            "{}\n{shown}",
            context("commit -a")
        );
        self.git(dir, &["checkout", "--", TRACKED]);

        // Refused: `commit -o` of a document staged clean, erroneous on disk.
        write(dir, TRACKED, clean_document("Staged clean"));
        self.git(dir, &["add", TRACKED]);
        write(dir, TRACKED, dangling_document(name));
        let (outcome, shown) = self.commit(dir, "commit -o", &["-o", TRACKED]);
        assert_eq!(
            outcome,
            Commit::Refused,
            "{}\n{shown}",
            context("commit -o erroneous")
        );
        self.git(dir, &["reset", "-q", "--", TRACKED]);
        self.git(dir, &["checkout", "--", TRACKED]);

        // Recorded: a clean commit.
        write(dir, TRACKED, clean_document(&format!("Clean in {place}")));
        self.git(dir, &["add", TRACKED]);
        let (outcome, shown) = self.commit(dir, "clean", &[]);
        assert_eq!(
            outcome,
            Commit::Recorded,
            "{}\n{shown}",
            context("clean commit")
        );

        // Recorded: `commit -o` of a clean document, another staged one
        // erroneous.
        write(dir, OTHER, dangling_document(name));
        self.git(dir, &["add", OTHER]);
        write(dir, TRACKED, clean_document(&format!("Only in {place}")));
        let (outcome, shown) = self.commit(dir, "commit -o clean", &["-o", TRACKED]);
        assert_eq!(
            outcome,
            Commit::Recorded,
            "{}\n{shown}",
            context("commit -o clean")
        );
        let committed = String::from_utf8(
            self.git
                .git(dir, &["show", "--name-only", "--format=", "HEAD"]),
        )
        .unwrap();
        assert_eq!(
            committed.trim(),
            TRACKED,
            "{}",
            context("only the named path")
        );
        self.git(dir, &["rm", "-q", "--cached", OTHER]);
        fs::remove_file(dir.join(OTHER)).unwrap();
    }

    /// `--no-verify` records a staged error.
    fn no_verify(&self, name: &str, dir: &Path, place: &str) {
        write(dir, OTHER, dangling_document(name));
        self.git(dir, &["add", OTHER]);
        let (outcome, shown) = self.commit(dir, "no verify", &["--no-verify"]);
        assert_eq!(
            outcome,
            Commit::Recorded,
            "{name} {place} --no-verify\n{shown}"
        );
    }
}

#[test]
fn the_pre_commit_hook_vouches_for_the_recorded_tree() {
    for (name, _) in FIXTURES {
        let hooked = Hooked::new(name);
        let top = hooked.top.clone();
        hooked.scenarios(name, &top, "main worktree");

        let linked = hooked.scratch.path().join("linked");
        hooked.git(
            &top,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "side",
                linked.to_str().unwrap(),
            ],
        );
        let linked = fs::canonicalize(&linked).unwrap();
        hooked.scenarios(name, &linked, "linked worktree");

        hooked.no_verify(name, &linked, "linked worktree");
        hooked.no_verify(name, &top, "main worktree");
    }
}

/// At the top level, `GIT_DIR=.git` and `GIT_INDEX_FILE=.git/index` (as a
/// hook exports them, relative) change nothing for `--root proj`; in a
/// linked worktree a relative `GIT_DIR` alone changes nothing; a
/// `GIT_INDEX_FILE` naming a missing file is exit 2.
#[test]
fn relative_git_variables_resolve_against_the_current_directory() {
    for (name, _) in FIXTURES {
        let hooked = Hooked::new(name);
        let git = &hooked.git;
        let top = &hooked.top;
        // A staged error, fixed on disk: only the index blocks.
        write(top, OTHER, dangling_document(name));
        git.git(top, &["add", OTHER]);
        write(top, OTHER, clean_document("Fixed on disk"));

        let args = ["--root", "proj", "check", "--staged"];
        let bare = spec_in(git, top, &args, &[]);
        bare.code(1);
        for extra in [
            &[("GIT_DIR", ".git")][..],
            &[("GIT_INDEX_FILE", ".git/index")],
            &[("GIT_DIR", ".git"), ("GIT_INDEX_FILE", ".git/index")],
        ] {
            let vars: Vec<(&str, &OsStr)> =
                extra.iter().map(|(k, v)| (*k, OsStr::new(*v))).collect();
            let run = spec_in(git, top, &args, &vars);
            assert_same(&run, &bare, &format!("{name}: {extra:?}"));
        }

        let run = spec_in(
            git,
            top,
            &args,
            &[("GIT_INDEX_FILE", OsStr::new(".git/no-such-index"))],
        );
        run.code(2);
        assert!(
            run.stdout.starts_with("cannot  .: "),
            "{name}: {}",
            run.show()
        );

        // A linked worktree, `GIT_DIR` alone and relative.
        git.git(top, &["rm", "-q", "-f", "--cached", OTHER]);
        let linked = hooked.scratch.path().join("linked");
        git.git(
            top,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "side",
                linked.to_str().unwrap(),
            ],
        );
        let linked = fs::canonicalize(&linked).unwrap();
        write(&linked, OTHER, dangling_document(name));
        git.git(&linked, &["add", OTHER]);
        write(&linked, OTHER, clean_document("Fixed on disk"));
        let bare = spec_in(git, &linked, &args, &[]);
        bare.code(1);
        let run = spec_in(
            git,
            &linked,
            &args,
            &[("GIT_DIR", OsStr::new("../repo/.git/worktrees/linked"))],
        );
        assert_same(
            &run,
            &bare,
            &format!("{name}: a relative GIT_DIR in a linked worktree"),
        );
    }
}

/// The guard's one cause (AC-07): `GIT_DIR` without `GIT_WORK_TREE` in a
/// directory below the top git finds without `GIT_DIR`.
const GUARD_CAUSE: &str = "cannot  .: GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top";

/// The `cannot  ` lines of a commit's shown output (the hook's report).
fn causes(shown: &str) -> Vec<&str> {
    shown
        .lines()
        .filter(|line| line.starts_with("cannot  "))
        .collect()
}

/// A `pre-commit` hook changing into `proj` first, printing the top git
/// sees there and spec's exit status.
const CD_PROJ_HOOK: &str = "#!/bin/sh\n\
    cd proj || exit 97\n\
    echo \"git top: $(git rev-parse --show-toplevel)\"\n\
    \"$SPEC\" check --staged\n\
    status=$?\n\
    echo \"spec exit status $status\"\n\
    exit $status\n";

/// The code review of iteration 1, and the orchestrator's decision on
/// iteration 2 (option A: follow git): a hook that changes into the root
/// first (`cd proj`, then `"$SPEC" check --staged`; the hook also prints
/// the top git itself sees there and spec's exit status).
///
/// Main worktree: git exports `GIT_INDEX_FILE=.git/index` for a plain
/// `git commit`, relative to the working tree's top (git reads a relative
/// `GIT_INDEX_FILE` there, not in the invoker's directory): a clean commit
/// is recorded, a staged error refused.
///
/// Linked worktree: git exports `GIT_DIR` alone (absolute). After
/// `cd proj` git itself takes `linked/proj` as the top (`GIT_WORK_TREE` =
/// the cwd), while the top git finds there without `GIT_DIR` is `linked`:
/// iteration 3's guard (AC-07) refuses. The clean commit is refused with
/// exactly one cause, the guard's at `.`, the verdict cannot-check, spec
/// exits 2; `HEAD` and `ls-files -s` are unchanged. (The recommended form,
/// `--root proj` from the top, is covered in both worktrees by
/// `the_pre_commit_hook_vouches_for_the_recorded_tree` and
/// `a_hook_from_the_top_with_root_proj_passes_the_guard_in_a_linked_worktree`.)
#[test]
fn a_hook_that_changes_into_the_root_reads_the_same_index() {
    for (name, _) in FIXTURES {
        let hooked = Hooked::with_hook(name, CD_PROJ_HOOK);
        let top = hooked.top.clone();
        let linked = hooked.scratch.path().join("linked");
        hooked.git(
            &top,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "side",
                linked.to_str().unwrap(),
            ],
        );
        let linked = fs::canonicalize(&linked).unwrap();

        // The main worktree.
        let place = "main worktree";
        let seen_top = format!("git top: {}\n", top.display());
        write(&top, TRACKED, clean_document(&format!("Clean in {place}")));
        hooked.git(&top, &["add", TRACKED]);
        let (outcome, shown) = hooked.commit(&top, "clean", &[]);
        assert_eq!(
            outcome,
            Commit::Recorded,
            "{name} {place}: a clean commit\n{shown}"
        );
        assert!(
            shown.contains(&seen_top) && shown.contains("spec exit status 0\n"),
            "{name} {place}: git's top and spec's exit\n{shown}"
        );

        write(&top, OTHER, dangling_document(name));
        hooked.git(&top, &["add", OTHER]);
        let (outcome, shown) = hooked.commit(&top, "a staged error", &[]);
        assert_eq!(
            outcome,
            Commit::Refused,
            "{name} {place}: a staged error\n{shown}"
        );
        assert!(shown.contains(" — blocked"), "{name} {place}\n{shown}");
        assert!(
            shown.contains("spec exit status 1\n"),
            "{name} {place}\n{shown}"
        );
        hooked.git(&top, &["rm", "-q", "-f", "--cached", OTHER]);
        fs::remove_file(top.join(OTHER)).unwrap();

        // The linked worktree: `proj` is below the top; the guard refuses.
        let place = "linked worktree";
        let staged_before = hooked.git.git(&linked, &["ls-files", "-s"]);
        write(
            &linked,
            TRACKED,
            clean_document(&format!("Clean in {place}")),
        );
        hooked.git(&linked, &["add", TRACKED]);
        let staged = hooked.git.git(&linked, &["ls-files", "-s"]);
        assert_ne!(staged, staged_before, "{name} {place}: the edit is staged");
        let head = hooked.git.head(&linked);
        let (outcome, shown) = hooked.commit(&linked, "clean", &[]);
        assert_eq!(
            outcome,
            Commit::Refused,
            "{name} {place}: `cd proj` below the top; the guard refuses\n{shown}"
        );
        let seen_top = format!("git top: {}\n", linked.join("proj").display());
        assert!(
            shown.contains(&seen_top),
            "{name} {place}: git itself takes `linked/proj` as the top\n{shown}"
        );
        assert_eq!(
            causes(&shown),
            [GUARD_CAUSE],
            "{name} {place}: exactly one cause, the guard's\n{shown}"
        );
        assert!(
            shown.contains(" — cannot-check\n") && !shown.contains(" — blocked"),
            "{name} {place}: the verdict\n{shown}"
        );
        assert!(
            shown.contains("spec exit status 2\n"),
            "{name} {place}: spec's exit\n{shown}"
        );
        // Nothing committed, the index untouched.
        assert_eq!(hooked.git.head(&linked), head, "{name} {place}: HEAD");
        assert_eq!(
            hooked.git.git(&linked, &["ls-files", "-s"]),
            staged,
            "{name} {place}: the index"
        );
    }
}

/// The two layouts of the iteration-3 guard tests: `proj/` alone, and a
/// clean project at the top as well.
const LAYOUTS: [(bool, &str); 2] = [(false, "proj alone"), (true, "a top project too")];

/// A [`Hooked`] repository whose hook is `script`, with (`top_project`) a
/// project at its top beside `proj/` (`spec-b`, `roots = ["docs"]`, `proj/`
/// outside its roots, a covering baseline: clean) committed with
/// `--no-verify`, and a linked worktree at `<scratch>/linked`, canonical.
fn layout(name: &str, script: &str, top_project: bool) -> (Hooked, PathBuf) {
    let hooked = Hooked::with_hook(name, script);
    let top = hooked.top.clone();
    if top_project {
        copy_dir(&fixture("spec-b"), &top);
        let covering = baseline_covering(&library(&top), FAR);
        write(&top, ".spec-debt.toml", covering);
        hooked.git(&top, &["add", "-A"]);
        hooked.git(
            &top,
            &["commit", "-q", "--no-verify", "-m", "a top project"],
        );
        spec_in(&hooked.git, &top, &["check", "--staged"], &[]).code(0);
    }
    let linked = hooked.scratch.path().join("linked");
    hooked.git(
        &top,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().unwrap(),
        ],
    );
    let linked = fs::canonicalize(&linked).unwrap();
    (hooked, linked)
}

/// The code review of iteration 2 (a false pass under option A) and
/// AC-07's guard (iteration 3): the `cd proj` hook, with `proj/` alone and
/// with a clean project at the top as well (`spec-b`, `roots = ["docs"]`,
/// `proj/` outside its roots). Main worktree: a clean commit is recorded
/// (spec exits 0), an error staged only under `proj/` is refused (blocked,
/// exit 1). Linked worktree: every commit is refused, the error staged
/// only under `proj/` and a clean one alike (both judged as expected from
/// the top with `--root proj`), each with exactly one cause, the guard's
/// at `.`, verdict cannot-check, spec exit 2, `HEAD` and `ls-files -s`
/// unchanged. No commit of either worktree holds the error.
#[test]
fn a_cd_proj_hook_in_a_linked_worktree_never_passes_the_top_project_for_proj() {
    for (name, _) in FIXTURES {
        for (top_project, layout_name) in LAYOUTS {
            let (hooked, linked) = layout(name, CD_PROJ_HOOK, top_project);
            let top = hooked.top.clone();
            let context =
                |place: &str, what: &str| format!("{name}, {layout_name}, {place}: {what}");

            // The main worktree.
            let place = "main worktree";
            write(&top, TRACKED, clean_document("Clean in the main worktree"));
            hooked.git(&top, &["add", TRACKED]);
            let (outcome, shown) = hooked.commit(&top, "clean", &[]);
            assert_eq!(
                outcome,
                Commit::Recorded,
                "{}\n{shown}",
                context(place, "a clean commit")
            );
            assert!(
                shown.contains("spec exit status 0\n"),
                "{}\n{shown}",
                context(place, "spec's exit")
            );
            write(&top, OTHER, dangling_document(name));
            hooked.git(&top, &["add", OTHER]);
            let (outcome, shown) = hooked.commit(&top, "an error under proj", &[]);
            assert_eq!(
                outcome,
                Commit::Refused,
                "{}\n{shown}",
                context(place, "an error staged under proj/")
            );
            assert!(
                shown.contains(" — blocked") && shown.contains("spec exit status 1\n"),
                "{}\n{shown}",
                context(place, "blocked, exit 1")
            );
            assert_eq!(
                causes(&shown),
                Vec::<&str>::new(),
                "{}\n{shown}",
                context(place, "no cause")
            );
            hooked.git(&top, &["rm", "-q", "-f", "--cached", OTHER]);
            fs::remove_file(top.join(OTHER)).unwrap();

            // The linked worktree: an error staged only under `proj/`, then a
            // clean edit; every commit refused by the guard.
            let place = "linked worktree";
            let from_top = ["--root", "proj", "check", "--staged"];
            write(&linked, OTHER, dangling_document(name));
            hooked.git(&linked, &["add", OTHER]);
            spec_in(&hooked.git, &linked, &from_top, &[]).code(1);
            let refused = |what: &str| {
                let head = hooked.git.head(&linked);
                let staged = hooked.git.git(&linked, &["ls-files", "-s"]);
                let (outcome, shown) = hooked.commit(&linked, what, &[]);
                assert_eq!(
                    outcome,
                    Commit::Refused,
                    "{}: committed by the cd-proj hook in a linked worktree\n{shown}",
                    context(place, what)
                );
                assert_eq!(
                    causes(&shown),
                    [GUARD_CAUSE],
                    "{}: exactly one cause, the guard's\n{shown}",
                    context(place, what)
                );
                assert!(
                    shown.contains(" — cannot-check\n")
                        && !shown.contains(" — blocked")
                        && shown.contains("spec exit status 2\n"),
                    "{}: cannot-check, exit 2\n{shown}",
                    context(place, what)
                );
                assert_eq!(
                    hooked.git.head(&linked),
                    head,
                    "{}: HEAD",
                    context(place, what)
                );
                assert_eq!(
                    hooked.git.git(&linked, &["ls-files", "-s"]),
                    staged,
                    "{}: the index",
                    context(place, what)
                );
            };
            refused("an error staged under proj/");
            hooked.git(&linked, &["rm", "-q", "-f", "--cached", OTHER]);
            fs::remove_file(linked.join(OTHER)).unwrap();
            write(
                &linked,
                TRACKED,
                clean_document("Clean in the linked worktree"),
            );
            hooked.git(&linked, &["add", TRACKED]);
            spec_in(&hooked.git, &linked, &from_top, &[]).code(0);
            refused("a clean commit");

            // No commit of either worktree records the error.
            let history = hooked
                .git
                .git_text(&top, &["log", "--all", "--name-only", "--format="]);
            assert!(
                !history.lines().any(|line| line == OTHER),
                "{}\n{history}",
                context("history", "the error under proj/ was committed")
            );
        }
    }
}

/// A `pre-commit` hook from the top printing whether git exported
/// `GIT_DIR` (non-empty) and `GIT_WORK_TREE`, then running
/// `"$SPEC" check --staged --root proj`.
const ROOT_PROJ_ENV_HOOK: &str = "#!/bin/sh\n\
    echo \"hook GIT_DIR=${GIT_DIR:+set} GIT_WORK_TREE=${GIT_WORK_TREE-unset}\"\n\
    exec \"$SPEC\" check --staged --root proj\n";

/// AC-07's guard (iteration 3) never refuses the recommended hook: in a
/// linked worktree git exports `GIT_DIR` alone (the guard's condition, the
/// hook shows it), the cwd is the top, and with `proj/` alone and with a
/// project at the top as well a clean commit is recorded and a staged
/// error refused as blocked, neither with the guard's cause. With a top
/// project the five hook scenarios pass in both worktrees.
#[test]
fn a_hook_from_the_top_with_root_proj_passes_the_guard_in_a_linked_worktree() {
    for (name, _) in FIXTURES {
        for (top_project, layout_name) in LAYOUTS {
            let (hooked, linked) = layout(name, ROOT_PROJ_ENV_HOOK, top_project);
            let top = hooked.top.clone();
            let place = format!("linked worktree, {layout_name}");
            let exported = "hook GIT_DIR=set GIT_WORK_TREE=unset\n";

            write(&linked, TRACKED, clean_document("Clean from the top"));
            hooked.git(&linked, &["add", TRACKED]);
            let (outcome, shown) = hooked.commit(&linked, "clean", &[]);
            assert_eq!(
                outcome,
                Commit::Recorded,
                "{name} {place}: a clean commit\n{shown}"
            );
            assert!(
                shown.contains(exported),
                "{name} {place}: git exports GIT_DIR alone to the hook\n{shown}"
            );

            write(&linked, OTHER, dangling_document(name));
            hooked.git(&linked, &["add", OTHER]);
            let (outcome, shown) = hooked.commit(&linked, "a staged error", &[]);
            assert_eq!(
                outcome,
                Commit::Refused,
                "{name} {place}: a staged error\n{shown}"
            );
            assert!(
                shown.contains(exported) && shown.contains(" — blocked"),
                "{name} {place}: blocked, not refused by the guard\n{shown}"
            );
            assert_eq!(
                causes(&shown),
                Vec::<&str>::new(),
                "{name} {place}: no cause\n{shown}"
            );
            hooked.git(&linked, &["rm", "-q", "-f", "--cached", OTHER]);
            fs::remove_file(linked.join(OTHER)).unwrap();

            if top_project {
                hooked.scenarios(name, &linked, &place);
                hooked.scenarios(name, &top, &format!("main worktree, {layout_name}"));
            }
        }
    }
}

/// Git 2.50.1 exports the temporary index of `commit -a` and `commit -o`
/// absolute (only a plain commit's `.git/index` is relative), so the hook
/// scenarios never resolve a relative temporary index. Simulated: an
/// alternate index `.git/next-index-test.lock` holding a staged error (the
/// default index clean, the file clean on disk), `GIT_INDEX_FILE` set to
/// it relative to the top. From the top with `--root proj` and from `proj`
/// without it, the alternate index is judged, as with its absolute path
/// (exit 1), not the default index nor a missing file.
#[test]
fn a_relative_alternate_index_resolves_against_the_top() {
    for (name, _) in FIXTURES {
        let hooked = Hooked::new(name);
        let git = &hooked.git;
        let top = &hooked.top;
        let proj = top.join("proj");
        let alternate = ".git/next-index-test.lock";
        copy_file(top.join(".git/index"), top.join(alternate));
        write(top, OTHER, dangling_document(name));
        git.git_env(
            top,
            &["add", OTHER],
            &[("GIT_INDEX_FILE", top.join(alternate).as_os_str())],
        );
        write(top, OTHER, clean_document("Clean on disk"));
        let absolute = top.join(alternate);

        for (dir, args, place) in [
            (top, &["--root", "proj", "check", "--staged"][..], "the top"),
            (&proj, &["check", "--staged"], "proj"),
        ] {
            let default = spec_in(git, dir, args, &[]);
            default.code(0);
            let reference = spec_in(git, dir, args, &[("GIT_INDEX_FILE", absolute.as_os_str())]);
            reference.code(1);
            let run = spec_in(git, dir, args, &[("GIT_INDEX_FILE", OsStr::new(alternate))]);
            assert_same(
                &run,
                &reference,
                &format!("{name}: a relative alternate index from {place}"),
            );
        }
    }
}

/// The `cd proj` hook's commit in the linked worktree `dir` is refused by
/// AC-07's guard: git itself takes `dir/proj` as the top, exactly one
/// cause (the guard's at `.`), cannot-check, spec exit 2, `HEAD` and
/// `ls-files -s` unchanged. `extra`: variables `git commit` is given.
fn assert_refused_by_the_guard(
    hooked: &Hooked,
    dir: &Path,
    what: &str,
    extra: &[(&str, &OsStr)],
    context: &str,
) {
    let head = hooked.git.head(dir);
    let staged = hooked.git.git(dir, &["ls-files", "-s"]);
    let (outcome, shown) = hooked.commit_with(dir, what, &[], extra);
    assert_eq!(
        outcome,
        Commit::Refused,
        "{context}: committed by the cd-proj hook in a linked worktree\n{shown}"
    );
    let seen_top = format!("git top: {}\n", dir.join("proj").display());
    assert!(
        shown.contains(&seen_top),
        "{context}: git itself takes `proj` as the top\n{shown}"
    );
    assert_eq!(
        causes(&shown),
        [GUARD_CAUSE],
        "{context}: exactly one cause, the guard's\n{shown}"
    );
    assert!(
        shown.contains(" — cannot-check\n")
            && !shown.contains(" — blocked")
            && shown.contains("spec exit status 2\n"),
        "{context}: cannot-check, exit 2\n{shown}"
    );
    assert_eq!(hooked.git.head(dir), head, "{context}: HEAD");
    assert_eq!(
        hooked.git.git(dir, &["ls-files", "-s"]),
        staged,
        "{context}: the index"
    );
}

/// AC-07's ceiling case (iteration 4): the `cd proj` hook in a linked
/// worktree, `git commit` run with `GIT_CEILING_DIRECTORIES` = the linked
/// top (exported to the hook, so git finds no repository from `proj`
/// without `GIT_DIR`), with `proj/` alone and with a clean project at the
/// top as well: an error staged only under `proj/`, then a clean edit,
/// each refused by the guard; no commit records the error.
#[test]
fn a_cd_proj_hook_is_refused_in_a_linked_worktree_under_a_ceiling_at_its_top() {
    for (name, _) in FIXTURES {
        for (top_project, layout_name) in LAYOUTS {
            let (hooked, linked) = layout(name, CD_PROJ_HOOK, top_project);
            let extra = [("GIT_CEILING_DIRECTORIES", linked.as_os_str())];
            let context = |what: &str| {
                format!("{name}, {layout_name}, GIT_CEILING_DIRECTORIES = the linked top: {what}")
            };

            write(&linked, OTHER, dangling_document(name));
            hooked.git(&linked, &["add", OTHER]);
            let what = "an error staged under proj/";
            assert_refused_by_the_guard(&hooked, &linked, what, &extra, &context(what));
            hooked.git(&linked, &["rm", "-q", "-f", "--cached", OTHER]);
            fs::remove_file(linked.join(OTHER)).unwrap();

            write(&linked, TRACKED, clean_document("Clean under a ceiling"));
            hooked.git(&linked, &["add", TRACKED]);
            let what = "a clean commit";
            assert_refused_by_the_guard(&hooked, &linked, what, &extra, &context(what));

            let history = hooked
                .git
                .git_text(&hooked.top, &["log", "--all", "--name-only", "--format="]);
            assert!(
                !history.lines().any(|line| line == OTHER),
                "{}\n{history}",
                context("the error under proj/ was committed")
            );
        }
    }
}

/// AC-07's submodule case (iteration 4): a superproject (no project at its
/// top, and a clean one) whose `proj` is a submodule with its own
/// `specengine.toml`, the `cd proj` hook as its `pre-commit`. In the
/// superproject's linked worktree git exports its own `GIT_DIR`; after
/// `cd proj` the top git finds is the submodule's, the cwd, but its git
/// dir is not `GIT_DIR`'s: a clean superproject commit is refused by the
/// guard.
#[test]
fn a_cd_proj_hook_into_a_submodule_of_a_linked_worktree_is_refused() {
    for (name, _) in FIXTURES {
        for (top_project, layout_name) in LAYOUTS {
            let sp = Superproject::new("staged-hook-sub", name, top_project);
            let hooks = sp.scratch.dir("hooks");
            let hook = hooks.join("pre-commit");
            fs::write(&hook, CD_PROJ_HOOK).unwrap();
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
            sp.git.git(
                &sp.top,
                &["config", "core.hooksPath", hooks.to_str().unwrap()],
            );
            let linked = sp.linked.clone();
            let context = format!("{name}, {layout_name}, the submodule proj");
            // The submodule's own project is clean.
            spec_in(&sp.git, &linked.join("proj"), &["check", "--staged"], &[]).code(0);
            let hooked = Hooked {
                scratch: sp.scratch,
                git: sp.git,
                top: sp.top,
            };

            write(&linked, "README.txt", "edited in the linked worktree\n");
            hooked.git(&linked, &["add", "README.txt"]);
            assert_refused_by_the_guard(
                &hooked,
                &linked,
                "a clean superproject commit",
                &[],
                &context,
            );
        }
    }
}
