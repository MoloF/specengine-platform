//! docs/features/spec-cli-changed.md against the git side: AC-06 (a
//! partial base fails closed: a missing `HEAD` blob of a changed or an
//! unchanged document, merged with the checked run's causes; a `HEAD`
//! entry naming a tree or a commit; `HEAD`'s tree deleted), AC-11 (the git
//! commands through a logging wrapper: `rev-parse`, `ls-tree`, `cat-file`
//! only, one session, every `HEAD` OID requested once, equal contents
//! once) and AC-12 (read-only; a linked worktree's own `HEAD`, `GIT_DIR`
//! with `GIT_WORK_TREE`, the guard, TAB/LF names, SHA-256). Every git
//! process runs in the sandbox of `common::git`, in scratch repositories
//! only.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::time::{Duration, SystemTime};

use common::check::dangling_document;
use common::git::Sandbox;
use common::staged::{GitLog, Repo, check_args, spec_in};
use common::{FIXTURES, Run, Scratch, copy_dir, read_text, snapshot, write};

const GUARD_MESSAGE: &str =
    "GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top";
const MISSING_HEAD_BLOB: &str = "HEAD's blob is missing from the git object database";
const HEAD_NOT_A_BLOB: &str = "HEAD's object is not a blob";
const LOCKED: &str = "cannot read: Permission denied (os error 13)";

fn changed_args<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut all = check_args(false, args);
    let at = all.iter().position(|arg| *arg == "check").expect("check");
    all.insert(at + 1, "--changed");
    all
}

fn changed(repo: &Repo, args: &[&str]) -> Run {
    repo.spec(&changed_args(args))
}

fn summary(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or_default()
}

fn labelled<'a>(stdout: &'a str, label: &str) -> Vec<&'a str> {
    let start = format!("{label}  ");
    stdout
        .lines()
        .filter(|line| line.starts_with(&start))
        .collect()
}

fn with_mode(config: &str, mode: &str) -> String {
    format!("{config}\n[check]\nmode = \"{mode}\"\n")
}

fn backlog(label: &str, fixture: &str) -> Repo {
    let repo = Repo::of(label, fixture);
    let config = read_text(&repo.top, "specengine.toml");
    write(
        &repo.top,
        "specengine.toml",
        with_mode(&config, "enforce-introduced"),
    );
    repo.add_all();
    repo.git.commit(&repo.top, "the backlog");
    repo
}

fn commit_all(repo: &Repo, message: &str) {
    repo.add_all();
    repo.git.commit(&repo.top, message);
}

fn head_oid(repo: &Repo, path: &str) -> String {
    repo.git
        .git_text(&repo.top, &["rev-parse", &format!("HEAD:{path}")])
}

fn tagged(fixture: &str, tag: &str) -> String {
    format!("{}\n{tag}\n", dangling_document(fixture))
}

fn assert_no_git_text_or_absolute_path(run: &Run, scratch: &Path, context: &str) {
    for stream in [&run.stdout, &run.stderr] {
        assert!(
            !stream.contains("fatal:") && !stream.contains("error:") && !stream.contains("hint:"),
            "{context}: git's text relayed\n{}",
            run.show()
        );
        let home = std::env::var("HOME").unwrap_or_else(|_| "/nonexistent-home".to_owned());
        for absolute in [
            scratch.to_str().unwrap(),
            "/private/",
            "/var/",
            "/tmp/",
            home.as_str(),
        ] {
            assert!(
                !stream.contains(absolute),
                "{context}: an absolute path ({absolute}) in the output\n{}",
                run.show()
            );
        }
    }
}

/// `--changed` in text and JSON is cannot-check (exit 2) in
/// `enforce-introduced` with exactly `causes` (path, message; sorted),
/// nothing else on stdout but the summary, no finding, no note, no git
/// text, no absolute path.
fn assert_partial(repo: &Repo, causes: &[(&str, &str)], context: &str) {
    let text = changed(repo, &[]);
    text.code(2);
    let expected: Vec<String> = causes
        .iter()
        .map(|(path, message)| format!("cannot  {path}: {message}"))
        .collect();
    assert_eq!(
        labelled(&text.stdout, "cannot"),
        expected,
        "{context}:\n{}",
        text.show()
    );
    assert_eq!(
        text.stdout.lines().count(),
        causes.len() + 1,
        "{context}: only the causes and the summary\n{}",
        text.show()
    );
    assert!(
        summary(&text.stdout).starts_with("spec check [enforce-introduced]: ")
            && summary(&text.stdout).ends_with(" — cannot-check"),
        "{context}:\n{}",
        text.show()
    );
    assert_eq!(text.stderr, "", "{context}: no note");
    assert_no_git_text_or_absolute_path(&text, repo.scratch.path(), context);
    let json = changed(repo, &["--json", "--debt"]);
    json.code(2);
    assert_eq!(
        json.stderr,
        "note: --debt changes only the text report: the JSON lists every finding and stale entry\n",
        "{context}: no other note"
    );
    assert_no_git_text_or_absolute_path(&json, repo.scratch.path(), context);
    let value = json.json();
    let got: Vec<(String, String)> = value["cannot_check"]
        .as_array()
        .unwrap_or_else(|| panic!("{context}: no cannot_check\n{}", json.show()))
        .iter()
        .map(|cause| {
            (
                cause["path"].as_str().unwrap_or_default().to_owned(),
                cause["message"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let want: Vec<(String, String)> = causes
        .iter()
        .map(|(path, message)| ((*path).to_owned(), (*message).to_owned()))
        .collect();
    assert_eq!(got, want, "{context}");
    assert_eq!(
        value["findings"].as_array().map(Vec::len),
        Some(0),
        "{context}"
    );
    assert_eq!(value["mode"], "enforce-introduced", "{context}");
}

// ---------------------------------------------------------------------------
// AC-06: a partial base fails closed.
// ---------------------------------------------------------------------------

/// AC-06: `HEAD`'s `a.md` defines an ID `b.md` cites; on disk `a.md` drops
/// it (the control: exit 1, `b.md`'s one error). `HEAD`'s blob of `a.md`
/// deleted → exit 2, one cause at `a.md`, no finding, no note — never exit
/// 0 with `b.md`'s error read as pre-existing. With an unreadable file on
/// disk too, the checked run's cause is merged.
#[test]
fn a_missing_head_blob_of_a_changed_document_fails_closed() {
    const DEFINES: &str = "docs/spec/zz-def.md";
    const CITES: &str = "docs/spec/zz-cite.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-blob-gone", fixture);
        let id = if fixture == "spec-a" {
            "RULE-ZZ-DEF"
        } else {
            "CMD-ZZ-DEF"
        };
        let defines = |anchor: &str| {
            format!(
                "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Defined\n\n## Rule{anchor}\n\nText.\n"
            )
        };
        write(&repo.top, DEFINES, defines(&format!(" {{#{id}}}")));
        write(
            &repo.top,
            CITES,
            format!(
                "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\nrefs: [{id}]\n---\n\n# Cites\n\nText.\n"
            ),
        );
        commit_all(&repo, "a definition and a citation");
        let oid = head_oid(&repo, DEFINES);
        write(&repo.top, DEFINES, defines(""));
        let control = changed(&repo, &[]);
        control.code(1);
        assert_eq!(
            labelled(&control.stdout, "error"),
            [format!(
                "error  {CITES}:5: ref-dangling: `refs`: `{id}` resolves to no ID and no alias"
            )],
            "{fixture}: the control\n{}",
            control.show()
        );
        fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
        assert_partial(&repo, &[(DEFINES, MISSING_HEAD_BLOB)], fixture);

        // The checked run's cause merged.
        let locked = repo.top.join("docs/spec/zz-locked.md");
        fs::write(&locked, dangling_document(fixture)).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        assert_partial(
            &repo,
            &[
                (DEFINES, MISSING_HEAD_BLOB),
                ("docs/spec/zz-locked.md", LOCKED),
            ],
            &format!("{fixture} merged"),
        );
        fs::remove_file(&locked).unwrap();
    }
}

/// AC-06: the `HEAD` blob of a document unchanged on disk deleted → the
/// same: exit 2, one cause at its path (under `--changed` no object is the
/// checked run's own); likewise for a document deleted on disk.
#[test]
fn a_missing_head_blob_of_an_unchanged_or_deleted_document_fails_closed() {
    const PATH: &str = "docs/spec/zz-same.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-blob-same", fixture);
        write(&repo.top, PATH, tagged(fixture, "Same."));
        commit_all(&repo, "a document");
        let oid = head_oid(&repo, PATH);
        changed(&repo, &[]).code(0);
        fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
        assert_partial(&repo, &[(PATH, MISSING_HEAD_BLOB)], fixture);
        fs::remove_file(repo.top.join(PATH)).unwrap();
        assert_partial(
            &repo,
            &[(PATH, MISSING_HEAD_BLOB)],
            &format!("{fixture} deleted"),
        );
    }
}

/// AC-06: a document `HEAD` lists as a regular file whose OID names a tree
/// or a commit → exit 2, one cause at that path, `HEAD's object is not a
/// blob` — with no file there on disk and with one.
#[test]
fn a_head_entry_naming_a_tree_or_a_commit_fails_closed() {
    const PATH: &str = "docs/spec/zz-foreign.md";
    for (fixture, _) in FIXTURES {
        for kind in ["tree", "commit"] {
            let context = format!("{fixture} {kind}");
            let repo = backlog("chg-foreign", fixture);
            let oid = repo
                .git
                .git_text(&repo.top, &["rev-parse", &format!("HEAD^{{{kind}}}")]);
            repo.git.git_stdin(
                &repo.top,
                &["update-index", "--index-info"],
                format!("100644 {oid}\t{PATH}\n").as_bytes(),
            );
            repo.git
                .commit(&repo.top, "a regular entry naming a non-blob");
            assert!(
                repo.git
                    .git_text(&repo.top, &["ls-tree", "HEAD", "--", PATH])
                    .starts_with(&format!("100644 blob {oid}\t")),
                "{context}"
            );
            assert!(!repo.top.join(PATH).exists());
            assert_partial(&repo, &[(PATH, HEAD_NOT_A_BLOB)], &context);
            write(&repo.top, PATH, dangling_document(fixture));
            assert_partial(&repo, &[(PATH, HEAD_NOT_A_BLOB)], &context);
        }
    }
}

/// AC-06: `HEAD`'s tree object deleted → exit 2, one fixed cause at `.`
/// naming `git ls-tree`, in the disk config's mode; no git text, no
/// absolute path; never an empty base.
#[test]
fn a_missing_head_tree_is_one_fixed_cause() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-tree-gone", fixture);
        let tree = repo.git.git_text(&repo.top, &["rev-parse", "HEAD^{tree}"]);
        fs::remove_file(repo.git.loose_object(&repo.top, &tree)).unwrap();
        for form in [&[][..], &["--json"]] {
            let run = changed(&repo, form);
            run.code(2);
            assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), fixture);
            assert_eq!(run.stderr, "", "{fixture}");
            if form.is_empty() {
                let causes = labelled(&run.stdout, "cannot");
                assert_eq!(causes.len(), 1, "{fixture}:\n{}", run.show());
                assert!(
                    causes[0].starts_with("cannot  .: ") && causes[0].contains("`git ls-tree`"),
                    "{fixture}: {}",
                    causes[0]
                );
                assert_eq!(run.stdout.lines().count(), 2, "{}", run.show());
                assert!(
                    summary(&run.stdout).starts_with("spec check [enforce-introduced]: ")
                        && summary(&run.stdout).ends_with(" — cannot-check"),
                    "{}",
                    run.show()
                );
            } else {
                let json = run.json();
                assert_eq!(json["cannot_check"].as_array().map(Vec::len), Some(1));
                assert_eq!(json["cannot_check"][0]["path"], ".");
                assert_eq!(json["findings"].as_array().map(Vec::len), Some(0));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-11: git use.
// ---------------------------------------------------------------------------

/// `HEAD`'s entries: path → (mode, OID).
fn head_entries(repo: &Repo) -> BTreeMap<String, (String, String)> {
    let text = String::from_utf8(repo.git(&["ls-tree", "-r", "HEAD"])).unwrap();
    text.lines()
        .map(|line| {
            let (meta, path) = line.split_once('\t').unwrap();
            let fields: Vec<&str> = meta.split_whitespace().collect();
            (
                path.to_owned(),
                (fields[0].to_owned(), fields[2].to_owned()),
            )
        })
        .collect()
}

/// AC-11: through a logging wrapper, a born `HEAD` and a disk holding a
/// changed, a moved, a deleted and an untracked document, two `HEAD`
/// paths of equal content: only `rev-parse`, `ls-tree`, `cat-file`;
/// `ls-tree -r -z <HEAD's OID>` once, in the root; one `cat-file --batch`;
/// `-c core.fsmonitor=false` and the four variables on every call; the
/// requests are `HEAD`'s config, `HEAD`'s baseline, then every `HEAD` blob
/// under the roots, each OID exactly once (the equal contents once). An
/// unborn `HEAD`: `rev-parse` only.
#[test]
fn changed_runs_three_subcommands_and_one_session() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-wrapper", fixture);
        let top = &repo.top;
        write(top, ".spec-debt.toml", "");
        write(top, "docs/spec/zz-changed.md", tagged(fixture, "Before."));
        write(top, "docs/spec/zz-moved.md", tagged(fixture, "Moved."));
        write(top, "docs/spec/zz-deleted.md", tagged(fixture, "Deleted."));
        write(top, "docs/spec/zz-twin-1.md", tagged(fixture, "Twin."));
        write(top, "docs/spec/zz-twin-2.md", tagged(fixture, "Twin."));
        write(top, "outside/zz-outside.md", tagged(fixture, "Outside."));
        commit_all(&repo, "more documents");
        write(top, "docs/spec/zz-changed.md", tagged(fixture, "After."));
        fs::rename(
            top.join("docs/spec/zz-moved.md"),
            top.join("docs/spec/zz-moved-here.md"),
        )
        .unwrap();
        fs::remove_file(top.join("docs/spec/zz-deleted.md")).unwrap();
        write(top, "docs/spec/zz-added.md", tagged(fixture, "Added."));
        let head = repo.git.head(top).expect("a born HEAD");
        let at_head = head_entries(&repo);
        let config_oid = at_head["specengine.toml"].1.clone();
        let baseline_oid = at_head[".spec-debt.toml"].1.clone();
        assert_eq!(
            at_head["docs/spec/zz-twin-1.md"].1,
            at_head["docs/spec/zz-twin-2.md"].1
        );
        let listed: BTreeSet<String> = at_head
            .iter()
            .filter(|(path, (mode, _))| {
                path.starts_with("docs/") && path.ends_with(".md") && mode == "100644"
            })
            .map(|(_, (_, oid))| oid.clone())
            .collect();
        assert!(listed.len() > 5, "{fixture}: {listed:?}");

        let log = GitLog::new(&repo.scratch, &repo.git);
        for form in [&[][..], &["--json"]] {
            log.clear();
            let run = spec_in(&repo.git, top, &changed_args(form), &log.env());
            run.code(1);
            let calls = log.calls();
            let mut names = BTreeSet::new();
            for call in &calls {
                assert!(
                    call.argv.len() >= 3
                        && call.argv[0] == "-c"
                        && call.argv[1] == "core.fsmonitor=false",
                    "{fixture}: {:?}",
                    call.argv
                );
                assert_eq!(
                    call.forced,
                    ["0", "1", "1", "0"],
                    "{fixture}: {:?}",
                    call.argv
                );
                names.insert(call.name().to_owned());
            }
            let expected: BTreeSet<String> = ["rev-parse", "ls-tree", "cat-file"]
                .iter()
                .map(|name| (*name).to_owned())
                .collect();
            assert_eq!(names, expected, "{fixture}: {calls:?}");
            let trees: Vec<_> = calls
                .iter()
                .filter(|call| call.name() == "ls-tree")
                .collect();
            assert_eq!(trees.len(), 1, "{fixture}: {calls:?}");
            assert_eq!(
                trees[0].sub(),
                ["ls-tree", "-r", "-z", head.as_str()],
                "{fixture}"
            );
            assert_eq!(trees[0].cwd, *top, "{fixture}: ls-tree runs in the root");
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call.sub() == ["rev-parse", "--verify", "-q", "HEAD"])
                    .count(),
                1,
                "{fixture}: {calls:?}"
            );
            let sessions: Vec<_> = calls
                .iter()
                .filter(|call| call.name() == "cat-file")
                .collect();
            assert_eq!(sessions.len(), 1, "{fixture}: {calls:?}");
            assert_eq!(sessions[0].sub(), ["cat-file", "--batch"]);

            let requests = log.requests();
            let unique: BTreeSet<&String> = requests.iter().collect();
            assert_eq!(
                unique.len(),
                requests.len(),
                "{fixture}: an OID requested twice: {requests:?}"
            );
            assert_eq!(
                requests[..2],
                [config_oid.clone(), baseline_oid.clone()],
                "{fixture}: HEAD's config, then its baseline, first"
            );
            let blobs: BTreeSet<String> = requests[2..].iter().cloned().collect();
            assert_eq!(blobs, listed, "{fixture}: every HEAD blob under the roots");
            assert!(
                !requests.contains(&at_head["outside/zz-outside.md"].1),
                "{fixture}: nothing outside the roots"
            );
        }

        // An unborn HEAD: probed, never listed or read.
        let unborn = Repo::of("chg-wrapper-unborn", fixture);
        let log = GitLog::new(&unborn.scratch, &unborn.git);
        let run = spec_in(&unborn.git, &unborn.top, &changed_args(&[]), &log.env());
        run.code(1);
        let subs: Vec<Vec<String>> = log
            .calls()
            .iter()
            .map(|call| call.sub().iter().map(|arg| (*arg).to_owned()).collect())
            .collect();
        assert_eq!(
            subs,
            [
                vec!["rev-parse".to_owned(), "--is-inside-work-tree".to_owned()],
                ["rev-parse", "--verify", "-q", "HEAD"]
                    .iter()
                    .map(|arg| (*arg).to_owned())
                    .collect(),
            ],
            "{fixture}"
        );
        assert!(log.requests().is_empty(), "{fixture}");
    }
}

// ---------------------------------------------------------------------------
// AC-12: read-only, the environment.
// ---------------------------------------------------------------------------

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[derive(Debug, PartialEq)]
struct State {
    index: Vec<u8>,
    index_mtime: SystemTime,
    lock: bool,
    git_dir: BTreeMap<String, Option<Vec<u8>>>,
    tree: BTreeMap<String, Option<Vec<u8>>>,
    home: Vec<String>,
    xdg: Vec<String>,
}

fn state(git: &Sandbox, top: &Path) -> State {
    let dot_git = top.join(".git");
    State {
        index: fs::read(dot_git.join("index")).unwrap(),
        index_mtime: mtime(&dot_git.join("index")),
        lock: dot_git.join("index.lock").exists(),
        git_dir: snapshot(&dot_git),
        tree: snapshot(top)
            .into_iter()
            .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
            .collect(),
        home: snapshot(git.home()).into_keys().collect(),
        xdg: snapshot(git.xdg()).into_keys().collect(),
    }
}

/// AC-12: a tracked file touched (stat-dirty), a changed and an untracked
/// document: `.git/index` (bytes, mtime), everything under `.git`
/// (objects, refs, `HEAD`), the working tree, the scratch `HOME` and
/// `XDG_CONFIG_HOME` are as they were, no `index.lock` — observed and
/// blocked, text and JSON.
#[test]
fn changed_writes_nothing() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-readonly", fixture);
        let top = &repo.top;
        for (verdict, code) in [("observed", 0), ("blocked", 1)] {
            if verdict == "blocked" {
                write(top, "docs/spec/zz-new.md", dangling_document(fixture));
                write(top, "README.md", "changed outside the roots\n");
            }
            let touched = top.join("specengine.toml");
            let later = mtime(&touched) + Duration::from_secs(7);
            fs::File::options()
                .write(true)
                .open(&touched)
                .unwrap()
                .set_modified(later)
                .unwrap();
            std::thread::sleep(Duration::from_millis(20));
            let before = state(&repo.git, top);
            for form in [&[][..], &["--json"]] {
                let run = changed(&repo, form);
                assert_eq!(run.code, code, "{fixture} {verdict}\n{}", run.show());
                let after = state(&repo.git, top);
                assert!(
                    after.index == before.index && after.index_mtime == before.index_mtime,
                    "{fixture} {verdict}: .git/index rewritten"
                );
                assert!(!after.lock, "{fixture} {verdict}: index.lock left");
                assert!(
                    after.git_dir == before.git_dir,
                    "{fixture} {verdict}: .git changed"
                );
                assert!(
                    after.tree == before.tree,
                    "{fixture} {verdict}: the working tree"
                );
                assert_eq!(after.home, Vec::<String>::new(), "{fixture}: HOME");
                assert_eq!(after.xdg, Vec::<String>::new(), "{fixture}: XDG");
            }
        }
    }
}

/// AC-12: a linked worktree is judged against its own `HEAD` — a document
/// with an error committed only on its branch is pre-existing there (exit
/// 0), with `GIT_DIR` = its git dir as a hook gets it or without; the main
/// worktree, whose `HEAD` lacks it, holding the same document on disk:
/// introduced (exit 1).
#[test]
fn a_linked_worktree_is_judged_against_its_own_head() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-linked", fixture);
        let linked = repo.scratch.path().join("linked");
        repo.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().unwrap(),
        ]);
        let linked = fs::canonicalize(&linked).unwrap();
        write(&linked, "docs/spec/zz-side.md", dangling_document(fixture));
        repo.git.add_all(&linked);
        repo.git.commit(&linked, "an error on the side branch");
        write(
            &linked,
            "docs/spec/zz-clean.md",
            "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Clean\n\nText.\n",
        );
        let git_dir = repo.top.join(".git/worktrees/linked");
        for extra in [&[][..], &[("GIT_DIR", git_dir.as_os_str())]] {
            let run = spec_in(&repo.git, &linked, &["check", "--changed", "--debt"], extra);
            run.code(0);
            assert!(
                run.stdout
                    .lines()
                    .any(|line| line.starts_with("error  docs/spec/zz-side.md:5: ")
                        && line.ends_with(" (pre-existing)")),
                "{fixture} {extra:?}:\n{}",
                run.show()
            );
        }
        write(
            &repo.top,
            "docs/spec/zz-side.md",
            dangling_document(fixture),
        );
        let run = changed(&repo, &[]);
        run.code(1);
        assert!(
            labelled(&run.stdout, "error")
                .iter()
                .any(|line| line.starts_with("error  docs/spec/zz-side.md:5: ")),
            "{fixture}:\n{}",
            run.show()
        );
    }
}

/// AC-12: `GIT_DIR` and `GIT_WORK_TREE` naming a git dir apart from the
/// working tree (no `.git` there, discovery stopped by the ceiling): every
/// call uses them — `HEAD` is read (exit 0 on the backlog, 1 with an
/// introduced error on disk), never a git failure.
#[test]
fn git_dir_and_work_tree_reach_every_call() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("chg-separate");
        let git = Sandbox::new(scratch.path());
        let tree = scratch.copy(fixture, "tree");
        let config = read_text(&tree, "specengine.toml");
        write(
            &tree,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        let git_dir = scratch.path().join("separate.git");
        let vars = [
            ("GIT_DIR", git_dir.as_os_str()),
            ("GIT_WORK_TREE", tree.as_os_str()),
        ];
        git.git_env(
            scratch.path(),
            &[
                "init",
                "--template=",
                "-q",
                "-b",
                "main",
                git_dir.to_str().unwrap(),
            ],
            &[("GIT_DIR", git_dir.as_os_str())],
        );
        git.quiet_env(scratch.path(), &[("GIT_DIR", git_dir.as_os_str())]);
        git.git_env(&tree, &["add", "-A"], &vars);
        git.git_env(
            &tree,
            &["commit", "-q", "--no-verify", "-m", "the backlog"],
            &vars,
        );
        assert!(!tree.join(".git").exists(), "no .git in the working tree");
        let run = spec_in(&git, &tree, &["check", "--changed", "--debt"], &vars);
        run.code(0);
        assert!(
            run.stdout.contains(" (pre-existing)"),
            "{fixture}:\n{}",
            run.show()
        );
        write(&tree, "docs/spec/zz-new.md", dangling_document(fixture));
        let run = spec_in(&git, &tree, &["check", "--changed"], &vars);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error").len(),
            1,
            "{fixture}:\n{}",
            run.show()
        );
        assert!(labelled(&run.stdout, "cannot").is_empty(), "{}", run.show());
    }
}

/// AC-12: the guard refuses as for `--staged` — `GIT_DIR` alone below the
/// top: exit 2, the guard's one cause, no `HEAD` probe, no `ls-tree`, no
/// `cat-file`; from the top with `--root`: the base runs.
#[test]
fn the_guard_refuses_as_for_staged() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::empty("chg-guard");
        let proj = repo.top.join("proj");
        copy_dir(&common::fixture(fixture), &proj);
        let config = read_text(&proj, "specengine.toml");
        write(
            &proj,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        repo.add_all();
        repo.git.commit(&repo.top, "the backlog");
        write(&proj, "docs/spec/zz-new.md", dangling_document(fixture));
        let log = GitLog::new(&repo.scratch, &repo.git);
        let git_dir = repo.top.join(".git");
        let mut extra = log.env().to_vec();
        extra.push(("GIT_DIR", git_dir.as_os_str()));
        let run = spec_in(&repo.git, &proj, &["check", "--changed"], &extra);
        run.code(2);
        assert_eq!(
            labelled(&run.stdout, "cannot"),
            [format!("cannot  .: {GUARD_MESSAGE}").as_str()],
            "{fixture}:\n{}",
            run.show()
        );
        assert_eq!(run.stderr, "", "{fixture}");
        assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), fixture);
        assert!(
            log.calls().iter().all(|call| call.name() != "ls-tree"
                && call.name() != "cat-file"
                && !call.argv.iter().any(|arg| arg == "--verify")),
            "{fixture}: {:?}",
            log.calls()
        );
        // `--staged` refuses alike.
        let staged = spec_in(&repo.git, &proj, &["check", "--staged"], &extra);
        staged.code(2);
        assert_eq!(
            labelled(&staged.stdout, "cannot"),
            labelled(&run.stdout, "cannot")
        );
        // From the top: the base runs (exit 1 on the introduced error).
        let run = spec_in(
            &repo.git,
            &repo.top,
            &["--root", "proj", "check", "--changed"],
            &[],
        );
        run.code(1);
    }
}

/// AC-12: names with TAB and LF (and a non-ASCII one, `core.quotePath`
/// on) committed with errors are pre-existing (exit 0); a new error in the
/// TAB-named one on disk is introduced (exit 1, that path); a SHA-256
/// repository alike, `ls-tree -r -z` given its 64-digit OID.
#[test]
fn tab_lf_names_and_sha256_are_read_from_head() {
    let names = [
        "docs/spec/tab\there.md",
        "docs/spec/line\nbreak.md",
        "docs/spec/caf\u{e9} \"q\".md",
    ];
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-names", fixture);
        repo.git(&["config", "core.quotePath", "true"]);
        for path in names {
            write(&repo.top, path, dangling_document(fixture));
        }
        commit_all(&repo, "odd names");
        let run = changed(&repo, &["--json"]);
        run.code(0);
        for path in names {
            let escaped = serde_json::to_string(path).unwrap();
            assert!(
                run.stdout.contains(&format!("\"path\":{escaped}")),
                "{fixture}: {path:?} walked\n{}",
                run.stdout
            );
        }
        assert!(
            !run.stdout.contains("\"introduced\":true"),
            "{fixture}: {}",
            run.stdout
        );
        let other = if fixture == "spec-a" {
            "R-78"
        } else {
            "REQ-778"
        };
        let text = dangling_document(fixture).replace("refs: [", &format!("refs: [{other}, "));
        write(&repo.top, names[0], &text);
        let run = changed(&repo, &["--json"]);
        run.code(1);
        let json = run.json();
        let introduced: Vec<&str> = json["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["introduced"] == true)
            .map(|f| f["path"].as_str().unwrap())
            .collect();
        assert_eq!(introduced, [names[0]], "{fixture}:\n{}", run.stdout);
    }

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("chg-sha256");
        let git = Sandbox::new(scratch.path());
        let top = scratch.copy(fixture, "repo");
        let config = read_text(&top, "specengine.toml");
        write(
            &top,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        git.init_with(&top, &["--object-format=sha256"]);
        git.add_all(&top);
        git.commit(&top, "the backlog");
        let head = git.head(&top).unwrap();
        assert_eq!(head.len(), 64, "{fixture}: {head}");
        let repo = Repo { scratch, git, top };
        let log = GitLog::new(&repo.scratch, &repo.git);
        let run = spec_in(&repo.git, &repo.top, &changed_args(&[]), &log.env());
        run.code(0);
        assert!(
            log.calls()
                .iter()
                .any(|call| call.sub() == ["ls-tree", "-r", "-z", head.as_str()]),
            "{fixture}: {:?}",
            log.calls()
        );
        assert!(
            log.requests().iter().all(|oid| oid.len() == 64),
            "{fixture}: {:?}",
            log.requests()
        );
        write(&repo.top, "docs/spec/zz-new.md", dangling_document(fixture));
        let run = changed(&repo, &[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error").len(),
            1,
            "{fixture}:\n{}",
            run.show()
        );
    }
}
