//! AC-06, AC-08, AC-09, AC-10, AC-12, AC-13 and AC-16 of
//! docs/features/spec-cli-staged.md: `spec check --staged` against the git
//! side — unmerged and intent-to-add entries, nothing written, the git
//! commands and variables used (a logging `git` wrapper), failures as
//! fixed causes, names and object formats, clean/smudge filters, and
//! determinism against the library's JSON; and AC-07's guard on `GIT_DIR`
//! set without `GIT_WORK_TREE` (a ceiling at the top, the top's gitfile, a
//! submodule, the guard's two calls). Every git process runs in the
//! sandbox of `common::git`, in scratch repositories only.

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::check::{FAR, baseline_covering, dangling_document, json, library, text};
use common::git::Sandbox;
use common::staged::{
    Repo, Superproject, assert_parity, assert_same, check_args, library_staged, spec_in, spec_timed,
};
use common::{FIXTURES, Run, Scratch, copy_dir, fixture, read_text, snapshot, write};
use serde_json::Value;
use specengine_store::{GitEnv, check_staged, today_utc};

fn summary(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or_default()
}

fn causes(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("cannot  "))
        .collect()
}

fn documents(stdout: &str) -> u64 {
    let json: Value = serde_json::from_str(stdout).expect("a JSON report");
    json["counts"]["documents"].as_u64().expect("documents")
}

fn clean_document(title: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# {title}\n\nText.\n")
}

fn with_mode(config: &str, mode: &str) -> String {
    format!("{config}\n[check]\nmode = \"{mode}\"\n")
}

/// No absolute path and no git text in a run's output.
fn assert_no_git_text_or_absolute_path(run: &Run, scratch: &Path, context: &str) {
    for stream in [&run.stdout, &run.stderr] {
        assert!(
            !stream.contains("fatal:") && !stream.contains("error:") && !stream.contains("hint:"),
            "{context}: git's text relayed\n{}",
            run.show()
        );
        // The scratch, the temp and system prefixes, and the test process's
        // own home directory.
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

// ---------------------------------------------------------------------------
// AC-06: unmerged stages and intent-to-add entries.
// ---------------------------------------------------------------------------

/// `git commit -- <path>` in `top`: `HEAD` gets the staged `path` (and
/// nothing else when it was unborn), so a staged baseline there is not new
/// debt, a staged mode not weaker than `HEAD`'s
/// (docs/features/spec-cli-introduced.md, 2a.2 Q6, Q7).
fn commit_path(git: &Sandbox, top: &Path, path: &str) {
    git.git(
        top,
        &["commit", "-q", "--no-verify", "-m", path, "--", path],
    );
}

/// AC-06 (a): a conflict on a document under the root is a cause naming
/// it (exit 2); a conflict outside a subdirectory root changes nothing.
#[test]
fn a_conflict_under_the_root_is_a_cause_and_outside_it_is_ignored() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new("staged-conflict");
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        copy_dir(&fixture(name), &top.join("proj"));
        let proj = top.join("proj");
        write(&proj, "docs/spec/zz-conflict.md", clean_document("Base"));
        write(&top, "notes/outside.md", "base\n");
        git.init(&top);
        git.add_all(&top);
        git.commit(&top, "base");

        git.git(&top, &["checkout", "-q", "-b", "other"]);
        write(&proj, "docs/spec/zz-conflict.md", clean_document("Other"));
        write(&top, "notes/outside.md", "other\n");
        git.add_all(&top);
        git.commit(&top, "other");
        git.git(&top, &["checkout", "-q", "main"]);
        write(&proj, "docs/spec/zz-conflict.md", clean_document("Main"));
        write(&top, "notes/outside.md", "main\n");
        git.add_all(&top);
        git.commit(&top, "main");

        // Only the file outside `proj` conflicts first.
        let mine = git.git_text(&top, &["rev-parse", "HEAD"]);
        git.git(&top, &["checkout", "-q", "-b", "outside-only", "other"]);
        write(&proj, "docs/spec/zz-conflict.md", clean_document("Main"));
        git.add_all(&top);
        git.commit(&top, "align the document");
        git.git(&top, &["checkout", "-q", &mine]);
        let merge = git.git_output(&top, &["merge", "-q", "outside-only"], &[]);
        assert!(!merge.status.success(), "{name}: the merge conflicts");
        let unmerged = git.git_text(&top, &["ls-files", "-u"]);
        assert!(
            unmerged.contains("notes/outside.md") && !unmerged.contains("proj/"),
            "{name}: {unmerged}"
        );
        let staged = spec_in(&git, &top, &["--root", "proj", "check", "--staged"], &[]);
        let plain = spec_in(&git, &top, &["--root", "proj", "check"], &[]);
        assert_same(
            &staged,
            &plain,
            &format!("{name}: a conflict outside the root"),
        );
        staged.code(1);
        git.git(&top, &["merge", "--abort"]);

        // Then the document under the root conflicts.
        let merge = git.git_output(&top, &["merge", "-q", "other"], &[]);
        assert!(!merge.status.success(), "{name}: the merge conflicts");
        let unmerged = git.git_text(&top, &["ls-files", "-u"]);
        assert!(
            unmerged.contains("proj/docs/spec/zz-conflict.md"),
            "{name}: {unmerged}"
        );
        let run = spec_in(&git, &top, &["--root", "proj", "check", "--staged"], &[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{name}: {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  docs/spec/zz-conflict.md: "),
            "{name}: {}",
            found[0]
        );
        assert!(summary(&run.stdout).ends_with(" — cannot-check"));
        assert_eq!(run.stderr, "", "{name}");
    }
}

/// AC-06 (b): `git add -N` of an erroneous document leaves the report as
/// it was before; a staged, genuinely empty `.md` is walked (as plain
/// walks it).
#[test]
fn an_intent_to_add_entry_is_not_staged_but_an_empty_blob_is() {
    for (name, _) in FIXTURES {
        let repo = Repo::staged("staged-ita", name);
        let top = &repo.top;
        let before: Vec<Run> = [&[][..], &["--json"]]
            .iter()
            .map(|args| repo.staged_check(args))
            .collect();
        before[0].code(1);

        write(top, "docs/spec/zz-ita.md", dangling_document(name));
        repo.git(&["add", "-N", "docs/spec/zz-ita.md"]);
        let listed =
            String::from_utf8(repo.git(&["ls-files", "-s", "docs/spec/zz-ita.md"])).unwrap();
        assert!(
            listed.contains("e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"),
            "{name}: an intent-to-add entry holds the empty blob: {listed}"
        );
        for (args, was) in [&[][..], &["--json"]].iter().zip(&before) {
            let now = repo.staged_check(args);
            assert_same(&now, was, &format!("{name}: git add -N {args:?}"));
        }

        // A genuinely empty document, staged: walked, as by plain on a twin
        // without the intent-to-add entry.
        write(top, "docs/spec/zz-empty.md", "");
        repo.git(&["add", "docs/spec/zz-empty.md"]);
        let with_empty = repo.staged_check(&["--json"]);
        assert_eq!(
            documents(&with_empty.stdout),
            documents(&before[1].stdout) + 1,
            "{name}: the empty document is walked"
        );
        let twin = Repo::of("staged-ita-twin", name);
        write(&twin.top, "docs/spec/zz-empty.md", "");
        twin.add_all();
        for args in [&[][..], &["--json"], &["--debt"]] {
            assert_same(
                &repo.staged_check(args),
                &twin.plain_check(args),
                &format!("{name}: an empty staged document {args:?}"),
            );
        }
    }
}

/// AC-06 (c), the residue: an intent-to-add document deleted on disk is
/// walked as an empty `.md` — exit 1 with its own `class-missing` (a false
/// block, never a false pass), on a tree otherwise clean (its baseline
/// committed: not new debt, docs/features/spec-cli-introduced.md 2a.2 Q7).
#[test]
fn an_intent_to_add_document_deleted_on_disk_blocks_with_class_missing() {
    for (name, _) in FIXTURES {
        let repo = Repo::of("staged-ita-gone", name);
        let top = &repo.top;
        let blocked = library(top);
        write(top, ".spec-debt.toml", baseline_covering(&blocked, FAR));
        repo.add_all();
        commit_path(&repo.git, top, ".spec-debt.toml");
        let committed = repo.staged_check(&["--json"]);
        committed.code(0);

        write(top, "docs/spec/zz-gone.md", dangling_document(name));
        repo.git(&["add", "-N", "docs/spec/zz-gone.md"]);
        fs::remove_file(top.join("docs/spec/zz-gone.md")).unwrap();
        let run = repo.staged_check(&["--json"]);
        run.code(1);
        assert_eq!(
            documents(&run.stdout),
            documents(&committed.stdout) + 1,
            "{name}: walked as an empty document\n{}",
            run.show()
        );
        let json: Value = serde_json::from_str(&run.stdout).unwrap();
        let own: Vec<&Value> = json["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["path"] == "docs/spec/zz-gone.md")
            .collect();
        assert!(
            own.iter()
                .any(|finding| finding["code"] == "class-missing" && finding["severity"] == "error"),
            "{name}: its own class-missing\n{}",
            run.show()
        );
        let blocking: Vec<&Value> = json["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| {
                finding["severity"] == "error"
                    && finding["debt"].is_null()
                    && finding["path"] != "docs/spec/zz-gone.md"
            })
            .collect();
        assert!(
            blocking.is_empty(),
            "{name}: other blocking errors: {blocking:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-08: nothing is written.
// ---------------------------------------------------------------------------

/// Every file under `dir` but `.git`, with its bytes.
fn working_tree(top: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    snapshot(top)
        .into_iter()
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .collect()
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

/// One verdict's setup step.
type Setup<'a> = Box<dyn Fn() + 'a>;

/// Everything AC-08 compares.
#[derive(Debug, PartialEq)]
struct State {
    index: Vec<u8>,
    index_mtime: SystemTime,
    lock: bool,
    objects: Vec<String>,
    refs: Vec<String>,
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
        objects: snapshot(&dot_git.join("objects")).into_keys().collect(),
        refs: snapshot(&dot_git.join("refs")).into_keys().collect(),
        git_dir: snapshot(&dot_git),
        tree: working_tree(top),
        home: snapshot(git.home()).into_keys().collect(),
        xdg: snapshot(git.xdg()).into_keys().collect(),
    }
}

/// AC-08: for each verdict, after touching a tracked file (stat-dirty in
/// the index), `--staged` leaves `.git/index` (bytes, mtime), `.git`
/// (objects, refs, everything), the working tree and the scratch `HOME`
/// as they were; `HOME` unset gives the same verdicts. The clean case's
/// baseline and the observed case's config are committed in their setup
/// (docs/features/spec-cli-introduced.md: else new debt, `HEAD`'s
/// stricter mode block).
#[test]
fn nothing_is_written_for_any_verdict() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new("staged-readonly");
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        copy_dir(&fixture(name), &top.join("proj"));
        let proj = top.join("proj");
        // An empty `.md` blob under the root, outside the `[paths]` roots:
        // the intent-to-add detector runs, the walk is unchanged.
        write(&proj, "notes/empty.md", "");
        git.init(&top);
        git.add_all(&top);
        git.commit(&top, "base");
        let config = read_text(&proj, "specengine.toml");
        let blocked = library(&proj);
        let covering = baseline_covering(&blocked, FAR);

        let verdicts: [(&str, u8, Setup<'_>); 4] = [
            ("blocked", 1, Box::new(|| {})),
            (
                "clean",
                0,
                Box::new(|| {
                    write(&proj, ".spec-debt.toml", &covering);
                    git.add_all(&top);
                    commit_path(&git, &top, "proj/.spec-debt.toml");
                }),
            ),
            (
                "observed",
                0,
                Box::new(|| {
                    fs::remove_file(proj.join(".spec-debt.toml")).unwrap();
                    write(&proj, "specengine.toml", with_mode(&config, "observe"));
                    git.add_all(&top);
                    commit_path(&git, &top, "proj/specengine.toml");
                }),
            ),
            (
                "cannot-check",
                2,
                Box::new(|| {
                    write(
                        &proj,
                        "specengine.toml",
                        common::check::set_paths_key(&config, "roots", "[\"docs\", \"nowhere\"]"),
                    );
                }),
            ),
        ];
        for (verdict, code, setup) in verdicts {
            setup();
            git.add_all(&top);
            // Touch a tracked file: same bytes, a new mtime.
            let touched = proj.join("specengine.toml");
            let later = mtime(&touched) + Duration::from_secs(7);
            fs::File::options()
                .write(true)
                .open(&touched)
                .unwrap()
                .set_modified(later)
                .unwrap();
            // The index is older than the touch, so the entry is stat-dirty.
            std::thread::sleep(Duration::from_millis(20));
            let before = state(&git, &top);
            for args in [&["check", "--staged"][..], &["--json", "check", "--staged"]] {
                let mut full = vec!["--root", "proj"];
                full.extend_from_slice(args);
                let run = spec_in(&git, &top, &full, &[]);
                assert_eq!(
                    run.code,
                    i32::from(code),
                    "{name} {verdict}\n{}",
                    run.show()
                );
                assert!(
                    summary(&run.stdout).contains(verdict),
                    "{name} {verdict}: {}",
                    run.stdout
                );
                let after = state(&git, &top);
                assert!(
                    after.index == before.index && after.index_mtime == before.index_mtime,
                    "{name} {verdict}: .git/index rewritten"
                );
                assert!(!after.lock, "{name} {verdict}: index.lock left");
                assert_eq!(after.objects, before.objects, "{name} {verdict}: objects");
                assert_eq!(after.refs, before.refs, "{name} {verdict}: refs");
                assert!(
                    after.git_dir == before.git_dir,
                    "{name} {verdict}: .git changed"
                );
                assert!(
                    after.tree == before.tree,
                    "{name} {verdict}: the working tree"
                );
                assert_eq!(after.home, Vec::<String>::new(), "{name} {verdict}: HOME");
                assert_eq!(after.xdg, Vec::<String>::new(), "{name} {verdict}: XDG");

                // `HOME` unset: the same bytes.
                let homeless =
                    spec_timed(&git, &top, &full, &[], &["HOME"], common::RUN_TIMEOUT).unwrap();
                assert_same(&homeless, &run, &format!("{name} {verdict}: HOME unset"));
                assert_eq!(
                    state(&git, &top),
                    before,
                    "{name} {verdict}: HOME unset wrote"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-09: the git commands and variables, through a logging wrapper.
// ---------------------------------------------------------------------------

/// A `git` wrapper in `<scratch>/wrapper` logging each call's argv and the
/// four forced variables to `<scratch>/git.log`, then running the real git.
fn wrapper(scratch: &Scratch, git: &Sandbox) -> (PathBuf, PathBuf) {
    let dir = scratch.dir("wrapper");
    let log = scratch.path().join("git.log");
    let script = format!(
        "#!/bin/sh\n\
         {{\n\
         printf 'argv'\n\
         for arg in \"$@\"; do printf '\\037%s' \"$arg\"; done\n\
         printf '\\036%s\\037%s\\037%s\\037%s\\n' \"${{GIT_OPTIONAL_LOCKS-unset}}\" \"${{GIT_NO_LAZY_FETCH-unset}}\" \"${{GIT_NO_REPLACE_OBJECTS-unset}}\" \"${{GIT_TERMINAL_PROMPT-unset}}\"\n\
         }} >> '{}'\n\
         exec '{}' \"$@\"\n",
        log.display(),
        git.git_program().display()
    );
    let path = dir.join("git");
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    (dir, log)
}

/// One logged call: its argv, then the four variables.
fn calls(log: &Path) -> Vec<(Vec<String>, Vec<String>)> {
    let text = fs::read_to_string(log).unwrap_or_default();
    text.lines()
        .map(|line| {
            let (argv, vars) = line.split_once('\u{1e}').expect("argv and variables");
            let argv = argv
                .strip_prefix("argv")
                .expect("argv")
                .split('\u{1f}')
                .skip(1)
                .map(str::to_owned)
                .collect();
            let vars = vars.split('\u{1f}').map(str::to_owned).collect();
            (argv, vars)
        })
        .collect()
}

/// AC-09: only `rev-parse`, `ls-files`, `cat-file`, the detector
/// (`diff-files`) and, for a born `HEAD`, `ls-tree`
/// (docs/features/spec-cli-introduced.md; `HEAD` is unborn here); one
/// `cat-file --batch` per check; every call with `-c core.fsmonitor=false`
/// and the four variables.
#[test]
fn only_plumbing_runs_with_the_forced_variables() {
    for (name, _) in FIXTURES {
        let repo = Repo::of("staged-wrapper", name);
        let top = &repo.top;
        // An empty blob: the detector runs too.
        write(top, "docs/spec/zz-empty.md", "");
        write(top, "alt.toml", read_text(top, "specengine.toml"));
        // Nothing to read: `--config` from disk, no staged baseline, a
        // written root in neither.
        write(
            top,
            "none.toml",
            common::check::set_paths_key(
                &read_text(top, "specengine.toml"),
                "roots",
                "[\"nowhere\"]",
            ),
        );
        repo.add_all();
        let (dir, log) = wrapper(&repo.scratch, &repo.git);
        let mut path = dir.into_os_string();
        path.push(":");
        path.push(repo.git.var("PATH").unwrap());

        let mut seen = std::collections::BTreeSet::new();
        for args in [
            &[][..],
            &["--json"],
            &["--debt"],
            &["--config", "alt.toml"],
            &["--baseline", "alt.toml"],
            &["--config", "none.toml"],
        ] {
            let _ = fs::remove_file(&log);
            let full = check_args(true, args);
            let run = spec_in(&repo.git, top, &full, &[("PATH", path.as_os_str())]);
            let plain = repo.plain_check(args);
            if args != ["--baseline", "alt.toml"] {
                assert_same(
                    &run,
                    &plain,
                    &format!("{name} {args:?} through the wrapper"),
                );
            }
            let calls = calls(&log);
            assert!(!calls.is_empty(), "{name} {args:?}: no git call logged");
            let mut batches = 0;
            for (argv, vars) in &calls {
                assert!(
                    argv.len() >= 3 && argv[0] == "-c" && argv[1] == "core.fsmonitor=false",
                    "{name} {args:?}: {argv:?}"
                );
                let sub = argv[2].as_str();
                assert!(
                    ["rev-parse", "ls-files", "cat-file", "diff-files", "ls-tree"].contains(&sub),
                    "{name} {args:?}: git {sub} ran: {argv:?}"
                );
                seen.insert(sub.to_owned());
                if sub == "cat-file" {
                    assert_eq!(argv[2..], ["cat-file", "--batch"], "{name} {args:?}");
                    batches += 1;
                }
                assert_eq!(
                    vars,
                    &["0", "1", "1", "0"],
                    "{name} {args:?}: GIT_OPTIONAL_LOCKS, GIT_NO_LAZY_FETCH, GIT_NO_REPLACE_OBJECTS, GIT_TERMINAL_PROMPT of {argv:?}"
                );
            }
            // At most one session per check; exactly one when blobs are read.
            let reads = args != ["--config", "none.toml"];
            assert!(
                batches <= 1 && (batches == 1) == reads,
                "{name} {args:?}: {batches} cat-file --batch sessions: {calls:?}"
            );
        }
        assert!(
            ["rev-parse", "ls-files", "cat-file", "diff-files"]
                .iter()
                .all(|sub| seen.contains(*sub)),
            "{name}: every command was exercised: {seen:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-10: failures are fixed causes on stdout, exit 2.
// ---------------------------------------------------------------------------

/// AC-10 (a), (b), (c): the root in no repository, no `git` on `PATH`, a
/// staged blob absent from the object database: exit 2, the report on
/// stdout with one cause (at `.`, `.`, the document), nothing but it on
/// stderr, no git text, no absolute path.
#[test]
fn git_failures_are_fixed_causes() {
    for (name, _) in FIXTURES {
        // (a) No repository.
        let scratch = Scratch::new("staged-norepo");
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy(name, "norepo");
        for args in [&["check", "--staged"][..], &["--json", "check", "--staged"]] {
            let run = spec_in(&git, &root, args, &[]);
            run.code(2);
            assert_eq!(run.stderr, "", "{name} {args:?}");
            assert_no_git_text_or_absolute_path(&run, scratch.path(), &format!("{name} (a)"));
        }
        let run = spec_in(&git, &root, &["check", "--staged"], &[]);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{name} (a): {}", run.stdout);
        assert!(found[0].starts_with("cannot  .: "), "{name}: {}", found[0]);
        assert!(summary(&run.stdout).ends_with(" — cannot-check"));

        // (b) `PATH` without `git`.
        let repo = Repo::staged("staged-nogit", name);
        let empty = repo.scratch.dir("empty-bin");
        let run = spec_in(
            &repo.git,
            &repo.top,
            &["check", "--staged"],
            &[("PATH", empty.as_os_str())],
        );
        run.code(2);
        assert_eq!(run.stderr, "", "{name} (b)");
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{name} (b): {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  .: ") && found[0].contains("`git` could not be run"),
            "{name} (b): {}",
            found[0]
        );
        assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), &format!("{name} (b)"));

        // (c) A staged blob absent.
        let repo = Repo::of("staged-noblob", name);
        write(
            &repo.top,
            "docs/spec/zz-lost.md",
            clean_document("Lost blob"),
        );
        repo.add_all();
        let oid = repo.git.staged_oid(&repo.top, "docs/spec/zz-lost.md");
        fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
        for args in [&[][..], &["--json"]] {
            let run = repo.staged_check(args);
            run.code(2);
            assert_eq!(run.stderr, "", "{name} (c) {args:?}");
            assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), &format!("{name} (c)"));
        }
        let run = repo.staged_check(&[]);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{name} (c): {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  docs/spec/zz-lost.md: "),
            "{name} (c): {}",
            found[0]
        );
    }
}

// ---------------------------------------------------------------------------
// AC-12: names and object formats.
// ---------------------------------------------------------------------------

/// AC-12: escaped non-ASCII names, names with TAB, LF, a quote and a
/// backslash under `core.quotePath=true`; a SHA-256 repository: `--staged`
/// prints plain's bytes.
#[test]
fn names_and_sha256_match_plain() {
    let names = [
        "docs/spec/caf\u{e9}.md",
        "docs/spec/\u{0434}\u{043e}\u{043a}.md",
        "docs/spec/\u{00fc}ber/inner.md",
        "docs/spec/tab\there.md",
        "docs/spec/line\nbreak.md",
        "docs/spec/quote\"d.md",
        "docs/spec/back\\slash.md",
    ];
    for (name, _) in FIXTURES {
        let repo = Repo::of("staged-names", name);
        for path in names {
            write(&repo.top, path, dangling_document(name));
        }
        repo.git(&["config", "core.quotePath", "true"]);
        repo.add_all();
        let quoted = String::from_utf8(repo.git(&["ls-files"])).unwrap();
        assert!(
            quoted.contains("\"docs/spec/tab\\there.md\""),
            "{name}: git quotes the names: {quoted}"
        );
        let run = repo.staged_check(&["--json"]);
        run.code(1);
        for path in names {
            let escaped = serde_json::to_string(path).unwrap();
            assert!(
                run.stdout.contains(&format!("\"path\":{escaped}")),
                "{name}: {path:?} walked\n{}",
                run.stdout
            );
        }
        assert_parity(&repo, &[], &format!("{name} names"));
    }

    for (name, _) in FIXTURES {
        let scratch = Scratch::new("staged-sha256");
        let git = Sandbox::new(scratch.path());
        let top = scratch.copy(name, "repo");
        git.init_with(&top, &["--object-format=sha256"]);
        git.add_all(&top);
        assert_eq!(
            git.git_text(&top, &["rev-parse", "--show-object-format"]),
            "sha256"
        );
        let oid = git.staged_oid(&top, "specengine.toml");
        assert_eq!(oid.len(), 64, "{name}: {oid}");
        let repo = Repo { scratch, git, top };
        repo.staged_check(&[]).code(1);
        assert_parity(&repo, &[], &format!("{name} sha256"));
    }
}

// ---------------------------------------------------------------------------
// AC-13: filters.
// ---------------------------------------------------------------------------

/// AC-13: `*.md filter=probe`, `clean = cat`, a smudge upper-casing and
/// touching a marker: after a checkout (the marker then removed),
/// `--staged` judges the clean blobs — plain's bytes on an unfiltered
/// copy — and runs no filter.
#[test]
fn filters_never_run_and_the_clean_blobs_are_judged() {
    for (name, _) in FIXTURES {
        let repo = Repo::of("staged-filter", name);
        let top = &repo.top;
        write(top, ".gitattributes", "*.md filter=probe\n");
        repo.git(&["config", "filter.probe.clean", "cat"]);
        repo.add_all();
        repo.git.commit(top, "base");

        let marker = repo.scratch.path().join("smudged");
        let smudge = repo.scratch.path().join("smudge.sh");
        fs::write(
            &smudge,
            format!("#!/bin/sh\ntouch '{}'\ntr a-z A-Z\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(&smudge, fs::Permissions::from_mode(0o755)).unwrap();
        repo.git(&["config", "filter.probe.smudge", smudge.to_str().unwrap()]);
        fs::remove_dir_all(top.join("docs")).unwrap();
        repo.git(&["checkout", "--", "docs"]);
        assert!(marker.exists(), "{name}: the smudge ran on checkout");
        let smudged = common::md_files(top);
        assert!(
            smudged
                .iter()
                .any(|path| read_text(top, path).contains("CLASS:")),
            "{name}: the working tree is smudged"
        );
        fs::remove_file(&marker).unwrap();

        let twin = Scratch::new("staged-filter-twin");
        let unfiltered = twin.copy(name, "copy");
        write(&unfiltered, ".gitattributes", "*.md filter=probe\n");
        let twin_git = Sandbox::new(twin.path());
        for args in [&[][..], &["--json"], &["--debt"]] {
            let staged = repo.staged_check(args);
            let plain = spec_in(&twin_git, &unfiltered, &check_args(false, args), &[]);
            assert_same(
                &staged,
                &plain,
                &format!("{name}: the clean blobs {args:?}"),
            );
            assert!(!marker.exists(), "{name}: a filter ran");
        }
        assert_ne!(
            repo.plain_check(&["--json"]).stdout,
            repo.staged_check(&["--json"]).stdout,
            "{name}: the smudged working tree is judged otherwise"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-16: the library's JSON, byte-identical across repositories.
// ---------------------------------------------------------------------------

/// A repository at `<scratch>/<dir>/repo` holding `fixture`, its files
/// added one by one, in path order or reversed.
fn added_one_by_one(
    scratch: &Scratch,
    git: &Sandbox,
    name: &str,
    dir: &str,
    reverse: bool,
) -> PathBuf {
    let top = scratch.copy(name, &format!("{dir}/repo"));
    git.init(&top);
    let mut files: Vec<String> = snapshot(&top)
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && !path.starts_with(".git"))
        .map(|(path, _)| path)
        .collect();
    if reverse {
        files.reverse();
    }
    for file in files {
        git.git(&top, &["add", "--", &file]);
    }
    top
}

/// AC-16: `--staged --json` is the library's `to_json()` and a line end
/// for clean, blocked, observed and cannot-check (a git cause), and two
/// repositories at different absolute paths, their files added in
/// opposite orders, print the same bytes.
#[test]
fn json_is_the_library_s_and_the_same_across_repositories() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new("staged-json");
        let git = Sandbox::new(scratch.path());
        let one = added_one_by_one(&scratch, &git, name, "a", false);
        let two = added_one_by_one(&scratch, &git, name, "a-much-longer-directory-name", true);
        let config = read_text(&one, "specengine.toml");
        let covering = baseline_covering(&library(&one), FAR);

        let compare = |verdict: &str, code: i32| {
            let mut outputs = Vec::new();
            for top in [&one, &two] {
                let report = library_staged(&git, top);
                let run = spec_in(&git, top, &["--json", "check", "--staged"], &[]);
                assert_eq!(run.code, code, "{name} {verdict}\n{}", run.show());
                assert!(
                    run.stdout == json(&report),
                    "{name} {verdict}: not the library's JSON\n{}",
                    run.show()
                );
                assert_eq!(run.stderr, "", "{name} {verdict}");
                let text_run = spec_in(&git, top, &["check", "--staged"], &[]);
                assert_eq!(
                    text_run.stdout,
                    text(&report, false),
                    "{name} {verdict}: text"
                );
                assert_no_git_text_or_absolute_path(
                    &run,
                    scratch.path(),
                    &format!("{name} {verdict}"),
                );
                assert_no_git_text_or_absolute_path(
                    &text_run,
                    scratch.path(),
                    &format!("{name} {verdict}"),
                );
                outputs.push((run, text_run));
            }
            assert_same(
                &outputs[0].0,
                &outputs[1].0,
                &format!("{name} {verdict} --json"),
            );
            assert_same(
                &outputs[0].1,
                &outputs[1].1,
                &format!("{name} {verdict} text"),
            );
        };

        compare("blocked", 1);
        for top in [&one, &two] {
            write(top, ".spec-debt.toml", &covering);
            git.add_all(top);
            // In `HEAD` too: not new debt (spec-cli-introduced, 2a.2 Q7).
            commit_path(&git, top, ".spec-debt.toml");
        }
        compare("clean", 0);
        for top in [&one, &two] {
            git.git(top, &["rm", "-q", "-f", ".spec-debt.toml"]);
            write(top, "specengine.toml", with_mode(&config, "observe"));
            git.add_all(top);
        }
        compare("observed", 0);
        // Cannot-check: a staged blob absent (a cause at its path).
        for top in [&one, &two] {
            write(top, "docs/spec/zz-lost.md", clean_document("Lost blob"));
            git.add_all(top);
            let oid = git.staged_oid(top, "docs/spec/zz-lost.md");
            let _ = fs::remove_file(git.loose_object(top, &oid));
        }
        compare("cannot-check", 2);
        // Cannot-check at `.`: `GIT_INDEX_FILE` naming no file, and no
        // repository.
        let mut outputs = Vec::new();
        for top in [&one, &two] {
            let run = spec_in(
                &git,
                top,
                &["--json", "check", "--staged"],
                &[("GIT_INDEX_FILE", OsStr::new(".git/no-such-index"))],
            );
            run.code(2);
            assert_no_git_text_or_absolute_path(
                &run,
                scratch.path(),
                &format!("{name} missing index"),
            );
            outputs.push(run);
        }
        assert_same(&outputs[0], &outputs[1], &format!("{name} missing index"));
        let mut outputs = Vec::new();
        for dir in ["x", "a-much-longer-directory-name-without-git"] {
            let root = scratch.copy(name, &format!("{dir}/norepo"));
            let report = library_staged(&git, &root);
            let run = spec_in(&git, &root, &["--json", "check", "--staged"], &[]);
            run.code(2);
            assert!(
                run.stdout == json(&report),
                "{name} no repository: not the library's\n{}",
                run.show()
            );
            assert_no_git_text_or_absolute_path(
                &run,
                scratch.path(),
                &format!("{name} no repository"),
            );
            outputs.push(run);
        }
        assert_same(&outputs[0], &outputs[1], &format!("{name} no repository"));
    }
}

/// AC-13 extended (the code review of iteration 1): an empty blob that is
/// neither a `.md` document nor the config or the baseline (an empty
/// `.txt`, a `.gitkeep`) does not start the intent-to-add detector, and so
/// never runs the clean filter of a stat-dirty `.md` (`*.md filter=probe`,
/// a clean filter touching a marker).
#[test]
fn an_empty_non_document_blob_runs_no_detector_and_no_filter() {
    for (name, _) in FIXTURES {
        let repo = Repo::of("staged-clean-filter", name);
        let top = &repo.top;
        let marker = repo.scratch.path().join("cleaned");
        let clean = repo.scratch.path().join("clean.sh");
        fs::write(
            &clean,
            format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(&clean, fs::Permissions::from_mode(0o755)).unwrap();
        write(top, ".gitattributes", "*.md filter=probe\n");
        repo.git(&["config", "filter.probe.clean", clean.to_str().unwrap()]);
        write(top, "docs/spec/empty.txt", "");
        write(top, "docs/spec/.gitkeep", "");
        repo.add_all();
        let _ = fs::remove_file(&marker);

        // A stat-dirty (racily clean) document: same bytes, a new mtime.
        let documents = common::md_files(top);
        let touched = top.join(&documents[0]);
        let later = fs::metadata(&touched).unwrap().modified().unwrap() + Duration::from_secs(3);
        fs::File::options()
            .write(true)
            .open(&touched)
            .unwrap()
            .set_modified(later)
            .unwrap();
        // Every other entry racily clean: `.git/index` no newer than the
        // files (git must compare their contents, through the filter).
        let index = top.join(".git/index");
        // (Copies keep the fixture's mtimes: go back before any of them.)
        let earlier = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(&index)
            .unwrap()
            .set_modified(earlier)
            .unwrap();

        let (dir, log) = wrapper(&repo.scratch, &repo.git);
        let mut path = dir.into_os_string();
        path.push(":");
        path.push(repo.git.var("PATH").unwrap());
        let run = spec_in(
            &repo.git,
            top,
            &["check", "--staged"],
            &[("PATH", path.as_os_str())],
        );
        run.code(1);
        let ran: Vec<String> = calls(&log)
            .into_iter()
            .map(|(argv, _)| argv.get(2).cloned().unwrap_or_default())
            .collect();
        let detector = ran.iter().any(|sub| sub == "diff-files");
        let filtered = marker.exists();
        assert!(
            !detector && !filtered,
            "{name}: the detector ran for an empty non-document blob: {detector} \
             (git calls {ran:?}); the clean filter ran: {filtered}"
        );
    }
}

// ---------------------------------------------------------------------------
// Iteration 2: the environment's resolution, the detector's trigger, a
// root that cannot be resolved.
// ---------------------------------------------------------------------------

/// The logging wrapper's directory first on the sandbox's `PATH`.
fn wrapped_path(dir: PathBuf, git: &Sandbox) -> std::ffi::OsString {
    let mut path = dir.into_os_string();
    path.push(":");
    path.push(git.var("PATH").unwrap());
    path
}

/// The logged calls resolving a variable: `rev-parse --git-path <name>` or
/// `rev-parse --git-common-dir`, without the leading `-c` pair.
fn resolving(log: &Path) -> Vec<Vec<String>> {
    let mut found: Vec<Vec<String>> = calls(log)
        .into_iter()
        .map(|(argv, _)| argv.into_iter().skip(2).collect::<Vec<_>>())
        .filter(|argv| {
            argv.iter()
                .any(|arg| arg == "--git-path" || arg == "--git-common-dir")
        })
        .collect();
    found.sort();
    found
}

/// The subcommands the logged calls ran, in order.
fn subcommands(log: &Path) -> Vec<String> {
    calls(log)
        .into_iter()
        .map(|(argv, _)| argv.get(2).cloned().unwrap_or_default())
        .collect()
}

/// AC-07, AC-10 (iteration 2, minor 1): a relative `GIT_INDEX_FILE`,
/// `GIT_OBJECT_DIRECTORY` or `GIT_COMMON_DIR` set in a directory outside
/// any repository (`--root ../repo`): its `rev-parse` fails there, so exit
/// 2, the report on stdout with one cause at `.` naming the variable,
/// stderr empty, no git text, no absolute path, nothing run after the
/// failing `rev-parse`, and `--json` the library's. An empty
/// `GIT_INDEX_FILE`: exit 2, one cause at `.`, and git is never run.
#[test]
fn a_relative_variable_git_cannot_resolve_and_an_empty_index_file_are_causes() {
    for (name, _) in FIXTURES {
        let repo = Repo::staged("staged-unresolved", name);
        let outside = repo.scratch.dir("outside");
        let (dir, log) = wrapper(&repo.scratch, &repo.git);
        let path = wrapped_path(dir, &repo.git);
        for (var, value, resolve) in [
            ("GIT_INDEX_FILE", ".git/index", &["--git-path", "index"][..]),
            (
                "GIT_OBJECT_DIRECTORY",
                ".git/objects",
                &["--git-path", "objects"],
            ),
            ("GIT_COMMON_DIR", ".git", &["--git-common-dir"]),
        ] {
            let context = format!("{name}: {var}={value} outside any repository");
            let expected = format!(
                "cannot  .: {var} is relative and git could not resolve it from the current directory"
            );
            let extra = [(var, OsStr::new(value)), ("PATH", path.as_os_str())];
            for globals in [&[][..], &["--json"]] {
                let _ = fs::remove_file(&log);
                let mut args = globals.to_vec();
                args.extend(["--root", "../repo", "check", "--staged"]);
                let run = spec_in(&repo.git, &outside, &args, &extra);
                run.code(2);
                assert_eq!(run.stderr, "", "{context} {args:?}");
                assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), &context);
                let mut rev_parse = vec!["rev-parse".to_owned()];
                rev_parse.extend(resolve.iter().map(|arg| (*arg).to_owned()));
                assert_eq!(
                    calls(&log)
                        .into_iter()
                        .map(|(argv, _)| argv.into_iter().skip(2).collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                    vec![rev_parse],
                    "{context} {args:?}: only the failing rev-parse ran"
                );
                if globals.is_empty() {
                    assert_eq!(causes(&run.stdout), [expected.as_str()], "{context}");
                    assert!(
                        summary(&run.stdout).ends_with(" — cannot-check"),
                        "{context}\n{}",
                        run.show()
                    );
                } else {
                    let env = GitEnv::new(&outside, repo.git.vars()).with_var(var, value);
                    let report = check_staged(&repo.top, None, None, &env, &today_utc());
                    assert!(
                        run.stdout == json(&report),
                        "{context}: not the library's JSON\n{}",
                        run.show()
                    );
                }
            }
        }

        // An empty `GIT_INDEX_FILE`: git never runs.
        for globals in [&[][..], &["--json"]] {
            let _ = fs::remove_file(&log);
            let mut args = globals.to_vec();
            args.extend(["check", "--staged"]);
            let run = spec_in(
                &repo.git,
                &repo.top,
                &args,
                &[
                    ("GIT_INDEX_FILE", OsStr::new("")),
                    ("PATH", path.as_os_str()),
                ],
            );
            run.code(2);
            assert_eq!(run.stderr, "", "{name}: an empty GIT_INDEX_FILE");
            assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), name);
            assert_eq!(
                calls(&log),
                Vec::new(),
                "{name} {args:?}: git ran for an empty GIT_INDEX_FILE"
            );
            if globals.is_empty() {
                assert_eq!(
                    causes(&run.stdout),
                    [
                        "cannot  .: GIT_INDEX_FILE names no file: git would read it as an empty index"
                    ],
                    "{name}: an empty GIT_INDEX_FILE\n{}",
                    run.show()
                );
            }
        }
    }
}

/// AC-07, AC-09 (iteration 2, minor 1): `rev-parse --git-path index`,
/// `--git-path objects`, `--git-common-dir` run once for each of
/// `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` set relative,
/// never for one set absolute; either way the report is the one without
/// them. A staged error fixed on disk (plain is clean): the index is what
/// is read. Once the fix is staged, the relative four
/// (`GIT_DIR=.git`, `GIT_OBJECT_DIRECTORY=.git/objects`,
/// `GIT_COMMON_DIR=.git`, `GIT_INDEX_FILE=.git/index`) print plain's bytes.
#[test]
fn rev_parse_resolves_only_the_relative_top_variables() {
    for (name, _) in FIXTURES {
        let repo = Repo::of("staged-top-relative", name);
        let top = &repo.top;
        write(
            top,
            ".spec-debt.toml",
            baseline_covering(&library(top), FAR),
        );
        repo.add_all();
        // In `HEAD` too: not new debt (spec-cli-introduced, 2a.2 Q7).
        commit_path(&repo.git, top, ".spec-debt.toml");
        const OTHER: &str = "docs/spec/zz-other.md";
        write(top, OTHER, dangling_document(name));
        repo.git(&["add", OTHER]);
        write(top, OTHER, clean_document("Fixed on disk"));
        let bare = repo.staged_check(&[]);
        bare.code(1);
        repo.plain_check(&[]).code(0);

        let (dir, log) = wrapper(&repo.scratch, &repo.git);
        let path = wrapped_path(dir, &repo.git);
        let absolute = |relative: &str| top.join(relative).into_os_string();
        let index = absolute(".git/index");
        let objects = absolute(".git/objects");
        let common = absolute(".git");
        let git_dir = absolute(".git");
        for set in [
            &[("GIT_INDEX_FILE", &index)][..],
            &[("GIT_OBJECT_DIRECTORY", &objects)],
            &[("GIT_COMMON_DIR", &common)],
            &[
                ("GIT_DIR", &git_dir),
                ("GIT_INDEX_FILE", &index),
                ("GIT_OBJECT_DIRECTORY", &objects),
                ("GIT_COMMON_DIR", &common),
            ],
        ] {
            let _ = fs::remove_file(&log);
            let mut extra: Vec<(&str, &OsStr)> =
                set.iter().map(|(k, v)| (*k, v.as_os_str())).collect();
            extra.push(("PATH", path.as_os_str()));
            let run = spec_in(&repo.git, top, &["check", "--staged"], &extra);
            let names: Vec<&str> = set.iter().map(|(k, _)| *k).collect();
            assert_same(&run, &bare, &format!("{name}: absolute {names:?}"));
            assert_eq!(
                resolving(&log),
                Vec::<Vec<String>>::new(),
                "{name}: absolute {names:?} resolved through rev-parse"
            );
        }

        let rev_parse = |args: &[&str]| -> Vec<String> {
            std::iter::once("rev-parse")
                .chain(args.iter().copied())
                .map(str::to_owned)
                .collect()
        };
        let index_call = rev_parse(&["--git-path", "index"]);
        let objects_call = rev_parse(&["--git-path", "objects"]);
        let common_call = rev_parse(&["--git-common-dir"]);
        let four = [
            ("GIT_DIR", ".git"),
            ("GIT_OBJECT_DIRECTORY", ".git/objects"),
            ("GIT_COMMON_DIR", ".git"),
            ("GIT_INDEX_FILE", ".git/index"),
        ];
        let mut all = vec![
            index_call.clone(),
            objects_call.clone(),
            common_call.clone(),
        ];
        all.sort();
        for (set, expected) in [
            (&[("GIT_INDEX_FILE", ".git/index")][..], vec![index_call]),
            (
                &[("GIT_OBJECT_DIRECTORY", ".git/objects")],
                vec![objects_call],
            ),
            (&[("GIT_COMMON_DIR", ".git")], vec![common_call]),
            (&[("GIT_DIR", ".git")], Vec::new()),
            (&four, all.clone()),
        ] {
            let _ = fs::remove_file(&log);
            let mut extra: Vec<(&str, &OsStr)> =
                set.iter().map(|(k, v)| (*k, OsStr::new(*v))).collect();
            extra.push(("PATH", path.as_os_str()));
            let run = spec_in(&repo.git, top, &["check", "--staged"], &extra);
            assert_same(&run, &bare, &format!("{name}: relative {set:?}"));
            assert_eq!(
                resolving(&log),
                expected,
                "{name}: relative {set:?}: the resolving rev-parse calls"
            );
            for (argv, vars) in calls(&log) {
                assert_eq!(vars, ["0", "1", "1", "0"], "{name} {set:?}: {argv:?}");
            }
        }

        // The fix staged: the relative four print plain's bytes.
        repo.git(&["add", OTHER]);
        for form in [&[][..], &["--json"], &["--debt"], &["--json", "--debt"]] {
            let _ = fs::remove_file(&log);
            let mut extra: Vec<(&str, &OsStr)> =
                four.iter().map(|(k, v)| (*k, OsStr::new(*v))).collect();
            extra.push(("PATH", path.as_os_str()));
            let run = spec_in(&repo.git, top, &check_args(true, form), &extra);
            let plain = repo.plain_check(form);
            plain.code(0);
            assert_same(&run, &plain, &format!("{name}: the relative four {form:?}"));
            assert_eq!(resolving(&log), all, "{name}: the relative four {form:?}");
        }
    }
}

/// AC-13 (iteration 2, minor 2): the detector's trigger, in a SHA-1 and a
/// SHA-256 repository. `diff-files` runs (once) for a stage-0 empty blob at
/// a `.md` path (a genuinely empty document: plain's bytes), at the root's
/// `specengine.toml` (intent-to-add: not staged, so exit 2 with its cause)
/// and at the root's `.spec-debt.toml` (an empty baseline: plain's bytes);
/// never for an empty `.MD`, nor a `specengine.toml` or `.spec-debt.toml`
/// below the root (the `.gitkeep`:
/// `an_empty_non_document_blob_runs_no_detector_and_no_filter`).
#[test]
fn the_detector_runs_for_an_empty_document_config_or_baseline() {
    for (name, _) in FIXTURES {
        for format in ["sha1", "sha256"] {
            let scratch = Scratch::new("staged-detector");
            let git = Sandbox::new(scratch.path());
            let top = scratch.copy(name, "repo");
            git.init_with(&top, &[&format!("--object-format={format}")]);
            git.add_all(&top);
            assert_eq!(
                git.git_text(&top, &["rev-parse", "--show-object-format"]),
                format
            );
            let repo = Repo { scratch, git, top };
            let top = &repo.top;
            let (dir, log) = wrapper(&repo.scratch, &repo.git);
            let path = wrapped_path(dir, &repo.git);
            let context = |what: &str| format!("{name} {format}: {what}");
            let run_logged = |args: &[&str]| {
                let _ = fs::remove_file(&log);
                let run = spec_in(
                    &repo.git,
                    top,
                    &check_args(true, args),
                    &[("PATH", path.as_os_str())],
                );
                let detector = subcommands(&log)
                    .iter()
                    .filter(|sub| *sub == "diff-files")
                    .count();
                (run, detector)
            };

            // No trigger: an empty `.MD`, a `specengine.toml` and a
            // `.spec-debt.toml` below the root.
            write(top, "docs/spec/upper.MD", "");
            write(top, "docs/specengine.toml", "");
            write(top, "docs/spec/.spec-debt.toml", "");
            repo.add_all();
            let (run, detector) = run_logged(&[]);
            assert_eq!(detector, 0, "{}", context("no trigger"));
            assert_same(&run, &repo.plain_check(&[]), &context("no trigger"));

            // An empty document.
            write(top, "docs/spec/zz-empty.md", "");
            repo.add_all();
            let (run, detector) = run_logged(&[]);
            assert_eq!(detector, 1, "{}", context("an empty .md"));
            assert_same(&run, &repo.plain_check(&[]), &context("an empty .md"));
            let (run, _) = run_logged(&["--json"]);
            let plain = repo.plain_check(&["--json"]);
            assert_same(&run, &plain, &context("an empty .md --json"));
            repo.git(&["rm", "-q", "-f", "docs/spec/zz-empty.md"]);

            // The root's config, intent-to-add.
            repo.git(&["rm", "-q", "--cached", "specengine.toml"]);
            repo.git(&["add", "-N", "specengine.toml"]);
            let (run, detector) = run_logged(&[]);
            assert_eq!(detector, 1, "{}", context("an intent-to-add config"));
            run.code(2);
            assert_eq!(
                causes(&run.stdout),
                ["cannot  specengine.toml: cannot read the config: not staged in the git index"],
                "{}\n{}",
                context("an intent-to-add config"),
                run.show()
            );
            repo.git(&["add", "specengine.toml"]);

            // The root's baseline, empty.
            write(top, ".spec-debt.toml", "");
            repo.add_all();
            let (run, detector) = run_logged(&[]);
            assert_eq!(detector, 1, "{}", context("an empty baseline"));
            assert_same(&run, &repo.plain_check(&[]), &context("an empty baseline"));
        }
    }
}

/// Restores a directory's permissions when dropped (before its scratch).
struct Unlock(PathBuf);

impl Drop for Unlock {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// Nit 3 of the review of iteration 1: a root that cannot be
/// canonicalised (missing; below a directory without search permission)
/// is one cause at `.`, "the root cannot be read", with no OS text, in the
/// mode `enforce` (exit 2 through the CLI). Only the library reaches it:
/// the CLI canonicalises `--root` itself first and reports its own error
/// (2a.1's line), so `check_staged` is called directly.
#[test]
fn a_root_that_cannot_be_resolved_is_a_fixed_cause() {
    let scratch = Scratch::new("staged-rootgone");
    let git = Sandbox::new(scratch.path());
    let locked = scratch.dir("locked");
    let behind = locked.join("proj");
    fs::create_dir_all(&behind).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let _unlock = Unlock(locked.clone());
    assert!(
        fs::canonicalize(&behind).is_err(),
        "the locked root still resolves (running as root?)"
    );
    let env = GitEnv::new(scratch.path(), git.vars());
    for (what, root) in [
        ("a missing root", scratch.path().join("missing/proj")),
        ("a root below a locked directory", behind),
    ] {
        let report = check_staged(&root, None, None, &env, &today_utc());
        assert_eq!(
            report
                .cannot_check
                .iter()
                .map(|cause| (cause.path.as_str(), cause.message.as_str()))
                .collect::<Vec<_>>(),
            [(".", "the root cannot be read")],
            "{what}: {}",
            text(&report, false)
        );
        let rendered = format!("{}{}", text(&report, false), json(&report));
        assert!(
            summary(&text(&report, false)).ends_with(" — cannot-check"),
            "{what}: {rendered}"
        );
        for os_text in [
            "No such file",
            "Permission denied",
            "os error",
            "denied",
            scratch.path().to_str().unwrap(),
        ] {
            assert!(
                !rendered.contains(os_text),
                "{what}: `{os_text}` in the report\n{rendered}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Iteration 3: the guard on `GIT_DIR` set without `GIT_WORK_TREE` (AC-07).
// ---------------------------------------------------------------------------

/// The guard's message, at `.`.
const GUARD_MESSAGE: &str =
    "GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top";

/// A logging wrapper as [`wrapper`], five more fields logged after the
/// four forced variables: `GIT_DIR`, `GIT_WORK_TREE`,
/// `GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM` (`unset`
/// when unset) and the directory git runs in (`pwd -P`).
fn guard_wrapper(scratch: &Scratch, git: &Sandbox) -> (PathBuf, PathBuf) {
    let dir = scratch.dir("guard-wrapper");
    let log = scratch.path().join("guard-git.log");
    let script = format!(
        "#!/bin/sh\n\
         {{\n\
         printf 'argv'\n\
         for arg in \"$@\"; do printf '\\037%s' \"$arg\"; done\n\
         printf '\\036%s\\037%s\\037%s\\037%s\\037%s\\037%s\\037%s\\037%s\\037%s\\n' \"${{GIT_OPTIONAL_LOCKS-unset}}\" \"${{GIT_NO_LAZY_FETCH-unset}}\" \"${{GIT_NO_REPLACE_OBJECTS-unset}}\" \"${{GIT_TERMINAL_PROMPT-unset}}\" \"${{GIT_DIR-unset}}\" \"${{GIT_WORK_TREE-unset}}\" \"${{GIT_CEILING_DIRECTORIES-unset}}\" \"${{GIT_DISCOVERY_ACROSS_FILESYSTEM-unset}}\" \"$(pwd -P)\"\n\
         }} >> '{}'\n\
         exec '{}' \"$@\"\n",
        log.display(),
        git.git_program().display()
    );
    let path = dir.join("git");
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    (dir, log)
}

/// Whether a logged argv is one of the guard's two discovery calls.
fn is_guard_call(argv: &[String]) -> bool {
    argv.iter()
        .any(|arg| arg == "--show-toplevel" || arg == "--absolute-git-dir")
}

/// The logged guard calls: `--show-toplevel` and `--absolute-git-dir`.
fn guard_calls(log: &Path) -> Vec<(Vec<String>, Vec<String>)> {
    calls(log)
        .into_iter()
        .filter(|(argv, _)| is_guard_call(argv))
        .collect()
}

/// Every logged call not the guard's carries `GIT_CEILING_DIRECTORIES` as
/// the caller gave it (`ceiling`) and no `GIT_DISCOVERY_ACROSS_FILESYSTEM`
/// (the caller set none): the guard's removal and addition are its own.
fn assert_main_calls_keep_the_ceiling(log: &Path, ceiling: &OsStr, context: &str) {
    let ceiling = ceiling.to_str().expect("a UTF-8 ceiling");
    for (argv, vars) in calls(log)
        .into_iter()
        .filter(|(argv, _)| !is_guard_call(argv))
    {
        assert_eq!(
            (vars[6].as_str(), vars[7].as_str()),
            (ceiling, "unset"),
            "{context}: a main call's GIT_CEILING_DIRECTORIES, GIT_DISCOVERY_ACROSS_FILESYSTEM: {argv:?}"
        );
    }
}

/// Exactly one guard (iteration 4), its calls the first git runs, in
/// `cwd`: `-c core.fsmonitor=false rev-parse --show-toplevel`, then, only
/// when the top git finds is `cwd` (`top_is_cwd`), `-c core.fsmonitor=false
/// rev-parse --absolute-git-dir`; each with the four forced variables,
/// `GIT_DIR`, `GIT_WORK_TREE` and `GIT_CEILING_DIRECTORIES` unset and
/// `GIT_DISCOVERY_ACROSS_FILESYSTEM=1`. Every other call keeps the
/// caller's `ceiling` ([`assert_main_calls_keep_the_ceiling`]).
fn assert_one_guard_call(log: &Path, cwd: &Path, top_is_cwd: bool, ceiling: &OsStr, context: &str) {
    let all = calls(log);
    let found = guard_calls(log);
    let mut expected = vec![vec![
        "-c",
        "core.fsmonitor=false",
        "rev-parse",
        "--show-toplevel",
    ]];
    if top_is_cwd {
        expected.push(vec![
            "-c",
            "core.fsmonitor=false",
            "rev-parse",
            "--absolute-git-dir",
        ]);
    }
    let argvs: Vec<Vec<&str>> = found
        .iter()
        .map(|(argv, _)| argv.iter().map(String::as_str).collect())
        .collect();
    assert_eq!(
        argvs, expected,
        "{context}: the guard's calls (top is the cwd: {top_is_cwd})"
    );
    assert_eq!(
        &all[..found.len()],
        &found[..],
        "{context}: the guard's calls come first"
    );
    let cwd = cwd.to_str().expect("a UTF-8 scratch path");
    for (argv, vars) in &found {
        assert_eq!(
            vars,
            &["0", "1", "1", "0", "unset", "unset", "unset", "1", cwd],
            "{context}: {argv:?}'s variables (the forced four; no GIT_DIR, GIT_WORK_TREE, \
             GIT_CEILING_DIRECTORIES; GIT_DISCOVERY_ACROSS_FILESYSTEM=1) and directory"
        );
    }
    assert_main_calls_keep_the_ceiling(log, ceiling, context);
}

/// A repository whose `proj/` holds a copy of `name` (a covering baseline:
/// clean) beside `README.txt` at the top, one commit, then an error staged
/// under `proj/` and fixed on disk (only the index blocks), and a linked
/// worktree at `<scratch>/linked` (canonical) in the same state.
fn nested_with_linked(name: &str) -> (Repo, PathBuf) {
    const OTHER: &str = "proj/docs/spec/zz-other.md";
    let repo = Repo::empty("staged-guard");
    let top = repo.top.clone();
    let proj = top.join("proj");
    copy_dir(&fixture(name), &proj);
    write(
        &proj,
        ".spec-debt.toml",
        baseline_covering(&library(&proj), FAR),
    );
    write(&top, "README.txt", "outside the root\n");
    repo.add_all();
    repo.git.commit(&top, "base");
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
    for dir in [&top, &linked] {
        write(dir, OTHER, dangling_document(name));
        repo.git.git(dir, &["add", OTHER]);
        write(dir, OTHER, clean_document("Fixed on disk"));
    }
    (repo, linked)
}

/// AC-07's guard (iteration 3), through a wrapper logging `GIT_DIR` and
/// `GIT_WORK_TREE`, in a repository with the project in `proj/`, an error
/// staged there and fixed on disk, and a linked worktree alike:
/// - `GIT_DIR` and `GIT_WORK_TREE` set (relative, absolute) from `proj/`,
///   main and linked worktree: no `rev-parse --show-toplevel` runs, the
///   report is the one without them (exit 1);
/// - `GIT_DIR` alone (relative `../.git`, absolute) from `proj/` in the
///   main worktree, and absolute from `linked/proj`: exit 2, stdout's one
///   cause the guard's at `.`, cannot-check, stderr empty, no git text nor
///   absolute path, one guard call without `GIT_DIR`; `--json` the
///   library's;
/// - `GIT_DIR` alone and absolute from the linked worktree's top with
///   `--root proj` (the hook's form): one guard call, the report the one
///   without it.
#[test]
fn git_dir_alone_below_the_top_is_refused_and_git_work_tree_skips_the_guard() {
    for (name, _) in FIXTURES {
        let (repo, linked) = nested_with_linked(name);
        let top = repo.top.clone();
        let ceiling = repo
            .git
            .var("GIT_CEILING_DIRECTORIES")
            .expect("the sandbox's ceiling");
        let (dir, log) = guard_wrapper(&repo.scratch, &repo.git);
        let path = wrapped_path(dir, &repo.git);
        let check = ["check", "--staged"];
        let from_top = ["--root", "proj", "check", "--staged"];
        let main_git_dir = top.join(".git").into_os_string();
        let linked_git_dir = top.join(".git/worktrees/linked").into_os_string();

        for (worktree, git_dir, work_tree_abs, relative) in [
            ("main", &main_git_dir, &top, ("../.git", "..")),
            (
                "linked",
                &linked_git_dir,
                &linked,
                ("../../repo/.git/worktrees/linked", ".."),
            ),
        ] {
            let proj = work_tree_abs.join("proj");
            let reference = spec_in(&repo.git, &proj, &check, &[]);
            reference.code(1);
            assert_same(
                &spec_in(&repo.git, work_tree_abs, &from_top, &[]),
                &reference,
                &format!("{name} {worktree}: from proj/ and from the top"),
            );

            // `GIT_DIR` and `GIT_WORK_TREE`: the guard is skipped.
            for (dir_value, tree_value) in [
                (git_dir.as_os_str(), work_tree_abs.as_os_str()),
                (OsStr::new(relative.0), OsStr::new(relative.1)),
            ] {
                let context = format!(
                    "{name} {worktree}: GIT_DIR={dir_value:?} GIT_WORK_TREE={tree_value:?} from proj/"
                );
                let _ = fs::remove_file(&log);
                let run = spec_in(
                    &repo.git,
                    &proj,
                    &check,
                    &[
                        ("GIT_DIR", dir_value),
                        ("GIT_WORK_TREE", tree_value),
                        ("PATH", path.as_os_str()),
                    ],
                );
                assert_same(&run, &reference, &context);
                assert!(!calls(&log).is_empty(), "{context}: git ran unlogged");
                assert_eq!(guard_calls(&log), Vec::new(), "{context}: the guard ran");
                assert_main_calls_keep_the_ceiling(&log, ceiling, &context);
            }

            // `GIT_DIR` alone below the top: refused.
            let mut alone = vec![git_dir.as_os_str()];
            if worktree == "main" {
                alone.push(OsStr::new(relative.0));
            }
            for dir_value in alone {
                let context = format!("{name} {worktree}: GIT_DIR={dir_value:?} alone from proj/");
                let extra = [("GIT_DIR", dir_value), ("PATH", path.as_os_str())];
                let _ = fs::remove_file(&log);
                let run = spec_in(&repo.git, &proj, &check, &extra);
                run.code(2);
                assert_eq!(
                    causes(&run.stdout),
                    [format!("cannot  .: {GUARD_MESSAGE}").as_str()],
                    "{context}\n{}",
                    run.show()
                );
                assert!(
                    summary(&run.stdout).ends_with(" — cannot-check"),
                    "{context}\n{}",
                    run.show()
                );
                assert_eq!(run.stderr, "", "{context}");
                assert_no_git_text_or_absolute_path(&run, repo.scratch.path(), &context);
                assert_one_guard_call(&log, &proj, false, ceiling, &context);

                let run = spec_in(&repo.git, &proj, &["--json", "check", "--staged"], &extra);
                run.code(2);
                let env = GitEnv::new(&proj, repo.git.vars()).with_var("GIT_DIR", dir_value);
                let report = check_staged(&proj, None, None, &env, &today_utc());
                assert!(
                    run.stdout == json(&report),
                    "{context}: not the library's JSON\n{}",
                    run.show()
                );
            }
        }

        // The hook's form: `GIT_DIR` alone, absolute, from the linked
        // worktree's top with `--root proj`.
        let context = format!("{name}: GIT_DIR alone from the linked top, --root proj");
        let bare = spec_in(&repo.git, &linked, &from_top, &[]);
        bare.code(1);
        let _ = fs::remove_file(&log);
        let run = spec_in(
            &repo.git,
            &linked,
            &from_top,
            &[
                ("GIT_DIR", linked_git_dir.as_os_str()),
                ("PATH", path.as_os_str()),
            ],
        );
        assert_same(&run, &bare, &context);
        assert_one_guard_call(&log, &linked, true, ceiling, &context);
    }
}

/// AC-07's guard from the top (iteration 3): `GIT_DIR` alone, `.git` and
/// absolute, the root the top. The guard runs (one `rev-parse
/// --show-toplevel`, without `GIT_DIR`) and changes nothing: fully staged,
/// the report is plain's for text, `--json`, `--debt`; with a covering
/// baseline and an error staged and fixed on disk (plain clean), the report
/// is the one without `GIT_DIR` (exit 1).
#[test]
fn git_dir_alone_from_the_top_runs_the_guard_and_changes_nothing() {
    for (name, _) in FIXTURES {
        let repo = Repo::staged("staged-guard-top", name);
        let top = repo.top.clone();
        let ceiling = repo
            .git
            .var("GIT_CEILING_DIRECTORIES")
            .expect("the sandbox's ceiling");
        let (dir, log) = guard_wrapper(&repo.scratch, &repo.git);
        let path = wrapped_path(dir, &repo.git);
        let absolute = top.join(".git").into_os_string();
        let values = [OsStr::new(".git"), absolute.as_os_str()];

        for value in values {
            for form in [&[][..], &["--json"], &["--debt"]] {
                let context = format!("{name}: GIT_DIR={value:?} from the top {form:?}");
                let _ = fs::remove_file(&log);
                let run = spec_in(
                    &repo.git,
                    &top,
                    &check_args(true, form),
                    &[("GIT_DIR", value), ("PATH", path.as_os_str())],
                );
                assert_same(&run, &repo.plain_check(form), &context);
                assert_one_guard_call(&log, &top, true, ceiling, &context);
            }
        }

        // A covering baseline staged (and committed: not new debt,
        // spec-cli-introduced 2a.2 Q7), then an error staged and fixed on
        // disk: only the index blocks.
        write(
            &top,
            ".spec-debt.toml",
            baseline_covering(&library(&top), FAR),
        );
        repo.add_all();
        commit_path(&repo.git, &top, ".spec-debt.toml");
        const OTHER: &str = "docs/spec/zz-other.md";
        write(&top, OTHER, dangling_document(name));
        repo.git(&["add", OTHER]);
        write(&top, OTHER, clean_document("Fixed on disk"));
        repo.plain_check(&[]).code(0);
        let bare = repo.staged_check(&[]);
        bare.code(1);
        for value in values {
            let context = format!("{name}: GIT_DIR={value:?} from the top, a staged error");
            let _ = fs::remove_file(&log);
            let run = spec_in(
                &repo.git,
                &top,
                &["check", "--staged"],
                &[("GIT_DIR", value), ("PATH", path.as_os_str())],
            );
            assert_same(&run, &bare, &context);
            assert_one_guard_call(&log, &top, true, ceiling, &context);
        }
    }
}

/// AC-07's guard compares directories, not spellings (iteration 3): the
/// library called with the cwd and the root spelled through a symbolic
/// link to the repository's top and `GIT_DIR` alone (`.git`, and absolute
/// through the link) is not refused; its JSON is the one of the top
/// spelled canonically without `GIT_DIR`. (The CLI cannot reach this: its
/// cwd is the process's, already resolved.)
#[test]
fn a_symlinked_spelling_of_the_top_is_the_top() {
    for (name, _) in FIXTURES {
        let repo = Repo::staged("staged-guard-link", name);
        let link = repo.scratch.path().join("link");
        std::os::unix::fs::symlink(&repo.top, &link).unwrap();
        assert_ne!(fs::canonicalize(&link).unwrap(), link, "{name}: a symlink");
        let expected = json(&library_staged(&repo.git, &repo.top));
        let through_link = link.join(".git").into_os_string();
        for value in [OsStr::new(".git"), through_link.as_os_str()] {
            let env = GitEnv::new(&link, repo.git.vars()).with_var("GIT_DIR", value);
            let report = check_staged(&link, None, None, &env, &today_utc());
            let rendered = json(&report);
            assert!(
                !report
                    .cannot_check
                    .iter()
                    .any(|cause| cause.message == GUARD_MESSAGE),
                "{name}: GIT_DIR={value:?}, the cwd through a symlink to the top: refused\n{rendered}"
            );
            assert_eq!(
                rendered, expected,
                "{name}: GIT_DIR={value:?}, the cwd through a symlink to the top"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Iteration 4: the guard discovers past the caller's ceiling and compares
// git dirs (AC-07).
// ---------------------------------------------------------------------------

/// `run` is the guard's refusal: exit 2, stdout's one cause the guard's at
/// `.`, cannot-check, stderr empty, no git text nor absolute path.
fn assert_guard_refusal(run: &Run, scratch: &Path, context: &str) {
    run.code(2);
    assert_eq!(
        causes(&run.stdout),
        [format!("cannot  .: {GUARD_MESSAGE}").as_str()],
        "{context}: exactly one cause, the guard's\n{}",
        run.show()
    );
    assert!(
        summary(&run.stdout).ends_with(" — cannot-check"),
        "{context}\n{}",
        run.show()
    );
    assert_eq!(run.stderr, "", "{context}");
    assert_no_git_text_or_absolute_path(run, scratch, context);
}

/// AC-07's ceiling case (iteration 4), through the guard wrapper, in the
/// main and the linked worktree of a repository with the project in
/// `proj/` (an error staged there, fixed on disk): `GIT_DIR` alone and
/// absolute (the hook's) from `proj/` with `GIT_CEILING_DIRECTORIES` = the
/// worktree's top, alone and after the scratch parent in a list, is
/// refused by the guard (git itself finds no repository from `proj/`
/// there); one `--show-toplevel` call, no `--absolute-git-dir`, both
/// without the ceiling. From the top with `--root proj`, the same
/// variables pass the guard (one call each) and the report is the one
/// without `GIT_DIR` (exit 1); every main call keeps the ceiling as given.
#[test]
fn a_ceiling_at_the_top_hides_it_from_no_guard() {
    for (name, _) in FIXTURES {
        let (repo, linked) = nested_with_linked(name);
        let top = repo.top.clone();
        let parent = repo
            .git
            .var("GIT_CEILING_DIRECTORIES")
            .expect("the sandbox's ceiling")
            .to_os_string();
        let (dir, log) = guard_wrapper(&repo.scratch, &repo.git);
        let path = wrapped_path(dir, &repo.git);
        let check = ["check", "--staged"];
        let from_top = ["--root", "proj", "check", "--staged"];
        let main_git_dir = top.join(".git");
        let linked_git_dir = top.join(".git/worktrees/linked");

        for (worktree, git_dir, work_tree) in [
            ("main", &main_git_dir, &top),
            ("linked", &linked_git_dir, &linked),
        ] {
            let proj = work_tree.join("proj");
            let bare = spec_in(&repo.git, work_tree, &from_top, &[]);
            bare.code(1);
            let mut listed = parent.clone();
            listed.push(":");
            listed.push(work_tree.as_os_str());
            for ceiling in [work_tree.as_os_str(), listed.as_os_str()] {
                let extra = [
                    ("GIT_DIR", git_dir.as_os_str()),
                    ("GIT_CEILING_DIRECTORIES", ceiling),
                    ("PATH", path.as_os_str()),
                ];
                let ceiling_shown = ceiling.to_string_lossy();

                let context = format!(
                    "{name} {worktree}: GIT_DIR alone from proj/, GIT_CEILING_DIRECTORIES={ceiling_shown}"
                );
                let found = repo.git.git_output(
                    &proj,
                    &["rev-parse", "--show-toplevel"],
                    &[("GIT_CEILING_DIRECTORIES", ceiling)],
                );
                assert!(
                    !found.status.success(),
                    "{context}: git itself finds a repository from proj/ past the ceiling"
                );
                let _ = fs::remove_file(&log);
                let run = spec_in(&repo.git, &proj, &check, &extra);
                assert_guard_refusal(&run, repo.scratch.path(), &context);
                assert_one_guard_call(&log, &proj, false, ceiling, &context);

                let context = format!(
                    "{name} {worktree}: GIT_DIR alone from the top, --root proj, GIT_CEILING_DIRECTORIES={ceiling_shown}"
                );
                let _ = fs::remove_file(&log);
                let run = spec_in(&repo.git, work_tree, &from_top, &extra);
                assert_same(&run, &bare, &context);
                assert_one_guard_call(&log, work_tree, true, ceiling, &context);
            }
        }
    }
}

/// AC-07's gitfile case (iteration 4): at a linked worktree's top,
/// `GIT_DIR` naming the top's own `.git` file (`.git`, and absolute), with
/// `--root proj`, passes the guard (one call each: the top is the cwd) and
/// the report is the one without `GIT_DIR` (exit 1: the error staged under
/// `proj/`, fixed on disk), for text, `--json` and `--debt`.
#[test]
fn the_top_s_own_gitfile_as_git_dir_passes_the_guard() {
    for (name, _) in FIXTURES {
        let (repo, linked) = nested_with_linked(name);
        let ceiling = repo
            .git
            .var("GIT_CEILING_DIRECTORIES")
            .expect("the sandbox's ceiling");
        let (dir, log) = guard_wrapper(&repo.scratch, &repo.git);
        let path = wrapped_path(dir, &repo.git);
        let gitfile = linked.join(".git");
        assert!(
            gitfile.is_file(),
            "{name}: a linked worktree's .git is a file"
        );
        for form in [&[][..], &["--json"], &["--debt"]] {
            let mut args = vec!["--root", "proj"];
            args.extend_from_slice(form);
            let args = check_args(true, &args);
            let bare = spec_in(&repo.git, &linked, &args, &[]);
            bare.code(1);
            for value in [OsStr::new(".git"), gitfile.as_os_str()] {
                let context =
                    format!("{name}: GIT_DIR={value:?} (a gitfile) at the linked top {args:?}");
                let _ = fs::remove_file(&log);
                let run = spec_in(
                    &repo.git,
                    &linked,
                    &args,
                    &[("GIT_DIR", value), ("PATH", path.as_os_str())],
                );
                assert_same(&run, &bare, &context);
                assert_one_guard_call(&log, &linked, true, ceiling, &context);
            }
        }
    }
}

/// AC-07's submodule case (iteration 4), through the guard wrapper: a
/// superproject (with no project at its top, and with a clean one) whose
/// linked worktree holds the submodule `proj` with its own project. From
/// `linked/proj` the top git finds is the cwd (one call each):
/// - `GIT_DIR` = the superproject's linked git dir (what its hooks get) is
///   refused, the guard's cause, `--json` the library's;
/// - `GIT_DIR` = the submodule's own git dir, or `.git` (its gitfile),
///   passes: clean and fully staged, the report is plain `spec check`'s
///   there (exit 0); an error staged in the submodule and fixed on disk,
///   the report is `--staged`'s without `GIT_DIR` (exit 1; plain 0).
///
/// From the linked top with `--root proj`, `GIT_DIR=proj/.git` (the
/// submodule's gitfile, not the top's) is refused.
#[test]
fn a_submodule_is_its_own_top_only_with_its_own_git_dir() {
    const OTHER: &str = "docs/spec/zz-other.md";
    for (name, _) in FIXTURES {
        for (top_project, layout_name) in [(false, "no top project"), (true, "a top project")] {
            let sp = Superproject::new("staged-guard-sub", name, top_project);
            let ceiling = sp
                .git
                .var("GIT_CEILING_DIRECTORIES")
                .expect("the sandbox's ceiling");
            let (dir, log) = guard_wrapper(&sp.scratch, &sp.git);
            let path = wrapped_path(dir, &sp.git);
            let proj = sp.linked.join("proj");
            let superproject = sp.linked_git_dir();
            let own = sp.submodule_git_dir();
            assert!(
                own.starts_with(&superproject) && own != superproject,
                "{name}, {layout_name}: the submodule's git dir {own:?} under the linked one"
            );
            let check = ["check", "--staged"];

            let refused = |state: &str| {
                let context = format!(
                    "{name}, {layout_name}, {state}: GIT_DIR = the superproject's linked git dir from linked/proj"
                );
                let extra = [
                    ("GIT_DIR", superproject.as_os_str()),
                    ("PATH", path.as_os_str()),
                ];
                let _ = fs::remove_file(&log);
                let run = spec_in(&sp.git, &proj, &check, &extra);
                assert_guard_refusal(&run, sp.scratch.path(), &context);
                assert_one_guard_call(&log, &proj, true, ceiling, &context);
                let run = spec_in(&sp.git, &proj, &["--json", "check", "--staged"], &extra);
                run.code(2);
                let env =
                    GitEnv::new(&proj, sp.git.vars()).with_var("GIT_DIR", superproject.as_os_str());
                let report = check_staged(&proj, None, None, &env, &today_utc());
                assert!(
                    run.stdout == json(&report),
                    "{context}: not the library's JSON\n{}",
                    run.show()
                );

                let context = format!(
                    "{name}, {layout_name}, {state}: GIT_DIR=proj/.git (the submodule's gitfile) from the linked top, --root proj"
                );
                let _ = fs::remove_file(&log);
                let run = spec_in(
                    &sp.git,
                    &sp.linked,
                    &["--root", "proj", "check", "--staged"],
                    &[
                        ("GIT_DIR", OsStr::new("proj/.git")),
                        ("PATH", path.as_os_str()),
                    ],
                );
                assert_guard_refusal(&run, sp.scratch.path(), &context);
                assert_one_guard_call(&log, &sp.linked, true, ceiling, &context);
            };
            let passes = |state: &str, expected: &Run| {
                for value in [own.as_os_str(), OsStr::new(".git")] {
                    let context = format!(
                        "{name}, {layout_name}, {state}: GIT_DIR={value:?} (the submodule's own) from linked/proj"
                    );
                    let _ = fs::remove_file(&log);
                    let run = spec_in(
                        &sp.git,
                        &proj,
                        &check,
                        &[("GIT_DIR", value), ("PATH", path.as_os_str())],
                    );
                    assert_same(&run, expected, &context);
                    assert_one_guard_call(&log, &proj, true, ceiling, &context);
                }
            };

            // Clean and fully staged: `--staged` is plain's.
            let plain = spec_in(&sp.git, &proj, &["check"], &[]);
            plain.code(0);
            assert_same(
                &spec_in(&sp.git, &proj, &check, &[]),
                &plain,
                &format!("{name}, {layout_name}: the submodule, --staged and plain"),
            );
            refused("clean");
            passes("clean", &plain);

            // An error staged in the submodule, fixed on disk.
            write(&proj, OTHER, dangling_document(name));
            sp.git.git(&proj, &["add", OTHER]);
            write(&proj, OTHER, clean_document("Fixed on disk"));
            spec_in(&sp.git, &proj, &["check"], &[]).code(0);
            let bare = spec_in(&sp.git, &proj, &check, &[]);
            bare.code(1);
            refused("a staged error");
            passes("a staged error", &bare);
        }
    }
}
