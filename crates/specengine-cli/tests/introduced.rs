//! docs/features/spec-cli-introduced.md through the `spec` binary: AC-03
//! (plain `enforce-introduced` is `enforce`, one note, no git), AC-04 (the
//! key), AC-05 (renames), AC-06 (an unborn `HEAD`, a new root), AC-08
//! (blocking), AC-09 (new debt), AC-10 (the stricter mode), AC-11 (given
//! files outside the root), AC-11b (`HEAD`'s baseline not known), AC-16
//! (determinism, field order) and the line order. Scratch repositories
//! hold copies of spec-a and spec-b, `HEAD` committed with `--no-verify`
//! as a backlog; every git process runs in the sandbox of `common::git`;
//! no git runs in this repository.

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::symlink;

use common::check::{
    FAR, PAST, baseline_covering, dangling_document, library, line_of, quoted, undefined_id,
};
use common::git::Sandbox;
use common::staged::{GitLog, Repo, check_args, spec_in};
use common::{FIXTURES, Run, Scratch, copy_dir, read_text, write};

/// The summary line: the last of stdout.
fn summary(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or_default()
}

/// The lines of stdout starting with `label` and two spaces.
fn labelled<'a>(stdout: &'a str, label: &str) -> Vec<&'a str> {
    let start = format!("{label}  ");
    stdout
        .lines()
        .filter(|line| line.starts_with(&start))
        .collect()
}

/// stderr's `note: ` lines.
fn notes(run: &Run) -> Vec<&str> {
    run.stderr
        .lines()
        .filter(|line| line.starts_with("note: "))
        .collect()
}

/// The fixture's config with `[check] mode = "<mode>"` appended.
fn with_mode(config: &str, mode: &str) -> String {
    format!("{config}\n[check]\nmode = \"{mode}\"\n")
}

/// A second ID of the fixture's scheme that nothing defines.
fn other_undefined(fixture: &str) -> &'static str {
    if fixture == "spec-a" {
        "R-78"
    } else {
        "REQ-778"
    }
}

/// A spec document whose `refs` list (from line 5) holds `ids`, one per
/// line: one `ref-dangling` per undefined item, at its line.
fn refs_document(ids: &[&str]) -> String {
    let mut text = String::from("---\nclass: spec\nstatus: draft\nscope: [docs/spec]\nrefs:\n");
    for id in ids {
        text.push_str(&format!("  - {id}\n"));
    }
    text.push_str("---\n\n# Refs\n\nText.\n");
    text
}

/// A spec document with no finding of its own in either fixture.
fn clean_document(title: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# {title}\n\nText.\n")
}

/// A copy of `fixture` with `[check] mode = mode`, everything committed
/// with `--no-verify`: `HEAD` holds the fixture's backlog.
fn backlog(label: &str, fixture: &str, mode: &str) -> Repo {
    let repo = Repo::of(label, fixture);
    let config = read_text(&repo.top, "specengine.toml");
    write(&repo.top, "specengine.toml", with_mode(&config, mode));
    repo.add_all();
    repo.git.commit(&repo.top, "the backlog");
    repo
}

/// `git commit -q --no-verify -m <message>` of everything staged.
fn commit(repo: &Repo, message: &str) {
    repo.git.commit(&repo.top, message);
}

/// `git add -A` then a commit.
fn commit_all(repo: &Repo, message: &str) {
    repo.add_all();
    commit(repo, message);
}

/// One `[[debt]]` entry.
fn entry(code: &str, path: &str, subject: &str, reason: &str, expires: &str) -> String {
    format!(
        "[[debt]]\ncode    = {}\npath    = {}\nsubject = {}\nreason  = {}\nexpires = \"{expires}\"\n\n",
        quoted(code),
        quoted(path),
        quoted(subject),
        quoted(reason)
    )
}

/// The 1-based line of the `[[debt]]` header of the entry for `path`.
fn entry_line(text: &str, path: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("path") && line.contains(&quoted(path)))
        .unwrap_or_else(|| panic!("no entry for {path}"));
    (0..at)
        .rev()
        .find(|&i| lines[i] == "[[debt]]")
        .expect("a header above")
        + 1
}

/// The `introduced` of every finding of a `--json` run, by (path, line,
/// code, subject).
fn introduced(run: &Run) -> Vec<(String, u64, String, String, Option<bool>)> {
    run.json()["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_owned(),
                f["line"].as_u64().unwrap(),
                f["code"].as_str().unwrap().to_owned(),
                f["subject"].as_str().unwrap().to_owned(),
                f["introduced"].as_bool(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// AC-03: plain `enforce-introduced`.
// ---------------------------------------------------------------------------

/// AC-03 (2a.2 Q8): plain `spec check` under `enforce-introduced` on a
/// blocked fixture prints `enforce`'s bytes under `[enforce]`, exit 1, and
/// exactly one `note:` — also with no `git` on `PATH`, and it runs no git
/// (a logging wrapper first on `PATH` logs nothing). Its JSON has no
/// `introduced` nor `new_debt`.
#[test]
fn plain_enforce_introduced_is_enforce_with_one_note_and_no_git() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-plain", fixture, "enforce-introduced");
        let twin = Scratch::new("intro-plain-twin");
        let enforced = twin.copy(fixture, "copy");
        let config = read_text(&enforced, "specengine.toml");
        write(&enforced, "specengine.toml", with_mode(&config, "enforce"));
        let twin_git = Sandbox::new(twin.path());
        let nowhere = repo.scratch.dir("no-git-here");
        let log = GitLog::new(&repo.scratch, &repo.git);
        for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
            let args = check_args(false, form);
            let expected = spec_in(&twin_git, &enforced, &args, &[]);
            expected.code(1);
            log.clear();
            let logged = spec_in(&repo.git, &repo.top, &args, &log.env());
            let bare = spec_in(
                &repo.git,
                &repo.top,
                &args,
                &[("PATH", nowhere.as_os_str())],
            );
            for run in [&logged, &bare] {
                run.code(1);
                assert_eq!(run.stdout, expected.stdout, "{fixture} {form:?}");
                // The one note of the mode, first; `--json --debt` keeps
                // its own note after it.
                assert_eq!(
                    notes(run)[0],
                    "note: mode `enforce-introduced` has no base without --staged or --changed: judged as `enforce`",
                    "{fixture} {form:?}\n{}",
                    run.show()
                );
                assert_eq!(
                    notes(run).len(),
                    1 + notes(&expected).len(),
                    "{fixture} {form:?}\n{}",
                    run.show()
                );
                let rest: Vec<&str> = run
                    .stderr
                    .lines()
                    .filter(|line| !line.starts_with("note: mode `enforce-introduced`"))
                    .collect();
                let expected_rest: Vec<&str> = expected.stderr.lines().collect();
                assert_eq!(rest, expected_rest, "{fixture} {form:?}: the other notes");
            }
            assert!(
                log.calls().is_empty(),
                "{fixture} {form:?}: plain ran git: {:?}",
                log.calls()
            );
            if form.contains(&"--json") {
                assert!(logged.stdout.starts_with("{\"mode\":\"enforce\","));
                assert!(!logged.stdout.contains("\"introduced\""));
                assert!(!logged.stdout.contains("\"new_debt\""));
            } else {
                assert!(
                    summary(&logged.stdout).starts_with("spec check [enforce]: "),
                    "{}",
                    logged.show()
                );
                assert!(!logged.stdout.contains(" introduced"));
                assert!(!logged.stdout.contains("(pre-existing)"));
            }
        }
    }
}

/// AC-02: without a base (a plain run) no output carries a base's field:
/// in every mode the JSON lacks `introduced` and `new_debt` (not even as
/// `null`), the text lacks ` introduced`, ` new debt`, ` (pre-existing)`
/// and `new` lines.
#[test]
fn plain_output_carries_no_base_field() {
    for (fixture, _) in FIXTURES {
        for mode in ["observe", "enforce-introduced", "enforce"] {
            let repo = backlog("intro-plain-fields", fixture, mode);
            for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
                let run = repo.plain_check(form);
                let context = format!("{fixture} {mode} {form:?}");
                assert!(
                    !run.stdout.contains("introduced"),
                    "{context}:\n{}",
                    run.show()
                );
                assert!(
                    !run.stdout.contains("new_debt"),
                    "{context}:\n{}",
                    run.show()
                );
                assert!(
                    !run.stdout.contains("new debt"),
                    "{context}:\n{}",
                    run.show()
                );
                assert!(
                    !run.stdout.contains("(pre-existing)"),
                    "{context}:\n{}",
                    run.show()
                );
                assert!(labelled(&run.stdout, "new").is_empty(), "{context}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-04: the key.
// ---------------------------------------------------------------------------

/// AC-04: `HEAD` holds E1 in `docs/spec/zz-a.md`; staged, E1 on another
/// line, E1's key again on a third, and a new E2: exit 1, one `error`
/// line (E2); `introduced` true on E2, false on both E1-keyed findings;
/// `counts.introduced` 1.
#[test]
fn the_key_is_code_path_subject_without_line_or_count() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-key", fixture, "enforce-introduced");
        let (e1, e2) = (undefined_id(fixture), other_undefined(fixture));
        const PATH: &str = "docs/spec/zz-a.md";
        write(&repo.top, PATH, refs_document(&[e1]));
        commit_all(&repo, "E1");
        // HEAD: E1 on line 6. Staged: E1 on 7, E2 on 8, E1's key on 9.
        let staged = refs_document(&[e1, e2, e1]).replacen("refs:\n", "refs:\n  # moved\n", 1);
        write(&repo.top, PATH, &staged);
        repo.add_all();

        let run = repo.staged_check(&[]);
        run.code(1);
        let errors = labelled(&run.stdout, "error");
        assert_eq!(errors.len(), 1, "{fixture}:\n{}", run.show());
        assert!(
            errors[0].starts_with(&format!("error  {PATH}:8: ref-dangling: ")),
            "{fixture}: {}",
            errors[0]
        );
        let json = repo.staged_check(&["--json"]);
        let ours: Vec<(u64, String, Option<bool>)> = introduced(&json)
            .into_iter()
            .filter(|(path, _, code, _, _)| path == PATH && code == "ref-dangling")
            .map(|(_, line, _, subject, flag)| (line, subject, flag))
            .collect();
        assert_eq!(
            ours,
            [
                (7, e1.to_owned(), Some(false)),
                (8, e2.to_owned(), Some(true)),
                (9, e1.to_owned(), Some(false)),
            ],
            "{fixture}:\n{}",
            json.stdout
        );
        assert_eq!(json.json()["counts"]["introduced"], 1, "{fixture}");
        assert!(
            introduced(&json).iter().all(|(.., flag)| flag.is_some()),
            "{fixture}: every finding has `introduced`"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-05: renames.
// ---------------------------------------------------------------------------

/// AC-05: no rename detection. `git mv` of a document with a pre-existing
/// error, a feature document moved to another feature's name, and an NFD
/// index entry (`update-index --index-info`) over `HEAD`'s precomposed
/// name: each is a new path, its error introduced (exit 1 at the new
/// path). The blob is the same, so its parse is never shared by OID alone.
#[test]
fn a_moved_document_is_a_new_path() {
    for (fixture, _) in FIXTURES {
        // `git mv` of a plain document.
        let repo = backlog("intro-mv", fixture, "enforce-introduced");
        write(&repo.top, "docs/spec/zz-old.md", dangling_document(fixture));
        commit_all(&repo, "a pre-existing error");
        repo.staged_check(&[]).code(0);
        repo.git(&["mv", "docs/spec/zz-old.md", "docs/spec/zz-new.md"]);
        let run = repo.staged_check(&[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            [format!(
                "error  docs/spec/zz-new.md:5: ref-dangling: `refs`: `{}` resolves to no ID and no alias",
                undefined_id(fixture)
            )],
            "{fixture}:\n{}",
            run.show()
        );

        // A feature document moved to another feature.
        let repo = backlog("intro-feature", fixture, "enforce-introduced");
        let (from, to) = if fixture == "spec-a" {
            (
                "docs/features/stamina-tuning.md",
                "docs/features/sprint-tuning.md",
            )
        } else {
            ("docs/features/dry-run.md", "docs/features/wet-run.md")
        };
        repo.staged_check(&[]).code(0);
        repo.git(&["mv", from, to]);
        let run = repo.staged_check(&[]);
        run.code(1);
        let errors = labelled(&run.stdout, "error");
        assert!(
            errors
                .iter()
                .any(|line| line.starts_with(&format!("error  {to}:6: "))),
            "{fixture}: the moved feature document's error\n{}",
            run.show()
        );
        assert!(
            errors.iter().all(|line| !line.contains(from)),
            "{fixture}\n{}",
            run.show()
        );

        // An NFD name in the index over HEAD's precomposed one.
        let repo = backlog("intro-nfd", fixture, "enforce-introduced");
        let nfc = "docs/spec/caf\u{e9}.md";
        let nfd = "docs/spec/cafe\u{301}.md";
        write(&repo.top, nfc, dangling_document(fixture));
        commit_all(&repo, "a precomposed name");
        repo.staged_check(&[]).code(0);
        repo.git(&["config", "core.precomposeUnicode", "false"]);
        let oid = repo.git.staged_oid(&repo.top, nfc);
        repo.git(&["update-index", "--force-remove", "--", nfc]);
        repo.git.git_stdin(
            &repo.top,
            &["update-index", "--add", "-z", "--index-info"],
            format!("100644 {oid}\t{nfd}\0").as_bytes(),
        );
        let listed = repo.git(&["ls-files", "-z"]);
        assert!(
            listed
                .split(|&byte| byte == 0)
                .any(|path| path == nfd.as_bytes()),
            "{fixture}: the index holds the NFD bytes"
        );
        assert!(
            !listed
                .split(|&byte| byte == 0)
                .any(|path| path == nfc.as_bytes()),
            "{fixture}: and not the precomposed ones"
        );
        let run = repo.staged_check(&["--json"]);
        run.code(1);
        let flags: Vec<(String, Option<bool>)> = introduced(&run)
            .into_iter()
            .filter(|(_, _, code, _, _)| code == "ref-dangling")
            .filter(|(path, ..)| path.starts_with("docs/spec/caf"))
            .map(|(path, .., flag)| (path, flag))
            .collect();
        assert_eq!(
            flags,
            [(nfd.to_owned(), Some(true))],
            "{fixture}:\n{}",
            run.stdout
        );
    }
}

// ---------------------------------------------------------------------------
// AC-06: an unborn `HEAD`, a new root.
// ---------------------------------------------------------------------------

/// AC-06: with no commit, `enforce-introduced` prints `enforce`'s blocking
/// lines, exit 1, no cause, no note; a `--root proj` new in a born
/// `HEAD`'s index likewise; once `proj` is committed with only its
/// pre-existing errors: exit 0 (`ls-tree` lists the root's own subtree,
/// root-relative).
#[test]
fn an_unborn_head_or_a_new_root_introduces_everything() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("intro-unborn", fixture);
        let config = read_text(&repo.top, "specengine.toml");
        write(
            &repo.top,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        repo.add_all();
        assert_eq!(repo.git.head(&repo.top), None);
        let twin = Scratch::new("intro-unborn-twin");
        let enforced = twin.copy(fixture, "copy");
        let enforce = spec_in(&Sandbox::new(twin.path()), &enforced, &["check"], &[]);
        enforce.code(1);
        let run = repo.staged_check(&[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            labelled(&enforce.stdout, "error"),
            "{fixture}:\n{}",
            run.show()
        );
        assert!(labelled(&run.stdout, "cannot").is_empty(), "{}", run.show());
        assert_eq!(run.stderr, "", "{fixture}");
        assert!(
            summary(&run.stdout).starts_with("spec check [enforce-introduced]: "),
            "{}",
            run.show()
        );

        // A new root `proj` in a born HEAD's index.
        let scratch = Scratch::new("intro-new-root");
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        write(&top, "README.txt", "outside the root\n");
        git.init(&top);
        git.add_all(&top);
        git.commit(&top, "no project yet");
        let proj = top.join("proj");
        copy_dir(&common::fixture(fixture), &proj);
        write(
            &proj,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        git.add_all(&top);
        let staged = ["--root", "proj", "check", "--staged"];
        let run = spec_in(&git, &top, &staged, &[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            labelled(&enforce.stdout, "error"),
            "{fixture} --root proj:\n{}",
            run.show()
        );
        assert!(labelled(&run.stdout, "cannot").is_empty(), "{}", run.show());
        assert_eq!(run.stderr, "", "{fixture}");

        // Committed with only its pre-existing errors: exit 0.
        git.commit(&top, "the project");
        let run = spec_in(&git, &top, &staged, &[]);
        run.code(0);
        assert!(
            summary(&run.stdout).ends_with(" — observed")
                && summary(&run.stdout).contains(", 0 introduced, 0 new debt, worst W "),
            "{fixture}:\n{}",
            run.show()
        );
        // The same from inside `proj`.
        let run = spec_in(&git, &proj, &["check", "--staged"], &[]);
        run.code(0);
    }
}

// ---------------------------------------------------------------------------
// AC-08: blocking.
// ---------------------------------------------------------------------------

/// AC-08: a pre-existing error without debt does not block (`observed`),
/// shows only with `--debt`, ending ` (pre-existing)`; in live debt: debt
/// (`clean`); its debt expired: it blocks (2a.2 Q4); an introduced error
/// under an entry `HEAD`'s baseline already holds: no block, no new debt.
#[test]
fn pre_existing_errors_block_only_when_expired() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-block", fixture, "enforce-introduced");
        let blocking = labelled(&repo.plain_check(&[]).stdout, "error")
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(!blocking.is_empty(), "{fixture}: a backlog");

        // Pre-existing, no debt.
        let run = repo.staged_check(&[]);
        run.code(0);
        assert!(labelled(&run.stdout, "error").is_empty(), "{}", run.show());
        assert!(
            summary(&run.stdout).ends_with(" — observed"),
            "{}",
            run.show()
        );
        let detail = repo.staged_check(&["--debt"]);
        detail.code(0);
        let expected: Vec<String> = blocking
            .iter()
            .map(|line| format!("{line} (pre-existing)"))
            .collect();
        assert_eq!(
            labelled(&detail.stdout, "error"),
            expected,
            "{fixture}:\n{}",
            detail.show()
        );

        // Live debt, committed (not new).
        let report = library(&repo.top);
        write(
            &repo.top,
            ".spec-debt.toml",
            baseline_covering(&report, FAR),
        );
        commit_all(&repo, "live debt");
        let run = repo.staged_check(&["--debt"]);
        run.code(0);
        assert!(summary(&run.stdout).ends_with(" — clean"), "{}", run.show());
        assert_eq!(
            labelled(&run.stdout, "debt").len(),
            expected.len(),
            "{}",
            run.show()
        );
        assert!(!run.stdout.contains("(pre-existing)"), "{}", run.show());

        // Expired debt over a pre-existing error, committed: blocks.
        write(
            &repo.top,
            ".spec-debt.toml",
            baseline_covering(&report, PAST),
        );
        commit_all(&repo, "expired debt");
        let run = repo.staged_check(&[]);
        run.code(1);
        let errors = labelled(&run.stdout, "error");
        assert_eq!(errors.len(), 1, "{fixture}:\n{}", run.show());
        assert!(
            errors[0].ends_with(&format!(" (debt expired {PAST}: test debt)")),
            "{}",
            errors[0]
        );
        assert!(summary(&run.stdout).contains(", 0 introduced, 0 new debt, "));

        // An introduced error under HEAD's existing (stale) entry.
        let mut text = baseline_covering(&report, FAR);
        text.push_str(&entry(
            "ref-dangling",
            "docs/spec/zz-new.md",
            undefined_id(fixture),
            "planned",
            FAR,
        ));
        write(&repo.top, ".spec-debt.toml", &text);
        commit_all(&repo, "an entry ahead of its error");
        repo.staged_check(&["--debt"]).code(0);
        write(&repo.top, "docs/spec/zz-new.md", dangling_document(fixture));
        repo.add_all();
        let run = repo.staged_check(&["--debt"]);
        run.code(0);
        assert!(
            run.stdout.lines().any(|line| line
                .starts_with("debt  docs/spec/zz-new.md:5: ref-dangling: ")
                && line.ends_with(&format!(" (debt until {FAR}: planned)"))),
            "{fixture}:\n{}",
            run.show()
        );
        assert!(labelled(&run.stdout, "new").is_empty(), "{}", run.show());
        assert!(summary(&run.stdout).ends_with(" — clean"), "{}", run.show());
        let json = repo.staged_check(&["--json"]);
        let flag = introduced(&json)
            .into_iter()
            .find(|(path, ..)| path == "docs/spec/zz-new.md")
            .map(|(.., flag)| flag);
        assert_eq!(flag, Some(Some(true)), "{fixture}: introduced, in debt");
    }
}

// ---------------------------------------------------------------------------
// AC-09: new debt.
// ---------------------------------------------------------------------------

/// AC-09 (2a.2 Q5, Q7): against `HEAD`'s baseline, an added triple blocks
/// under `enforce-introduced` and `enforce`, one `new` line naming it;
/// `expires` moved later blocks ("extends HEAD's expiry"); moved earlier,
/// a changed `reason`, a removed entry do not; an unborn `HEAD` makes
/// every staged entry new.
#[test]
fn new_debt_blocks_in_both_enforcing_modes() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-newdebt", fixture, "enforce-introduced");
        let covering = baseline_covering(&library(&repo.top), FAR);
        let stale = entry(
            "ref-dangling",
            "docs/spec/none.md",
            "X-1",
            "stale",
            "2998-06-30",
        );
        let head = format!("{covering}{stale}");
        write(&repo.top, ".spec-debt.toml", &head);
        commit_all(&repo, "HEAD's baseline");
        let config = read_text(&repo.top, "specengine.toml");
        let enforce = config.replace("mode = \"enforce-introduced\"", "mode = \"enforce\"");
        assert_ne!(config, enforce);
        repo.staged_check(&[]).code(0);

        let added = entry(
            "ref-dangling",
            "docs/spec/none2.md",
            "X-2",
            "added",
            "2999-01-01",
        );
        let added_text = format!("{head}{added}");
        let line = entry_line(&added_text, "docs/spec/none2.md");
        let expected_new = format!(
            "new  docs/spec/none2.md: debt-new: the baseline entry at line {line} (ref-dangling, subject \"X-2\") is not in HEAD's baseline (added; expires 2999-01-01)"
        );
        for (mode, text) in [("enforce-introduced", &config), ("enforce", &enforce)] {
            write(&repo.top, "specengine.toml", text);
            write(&repo.top, ".spec-debt.toml", &added_text);
            repo.add_all();
            let run = repo.staged_check(&[]);
            run.code(1);
            assert_eq!(
                labelled(&run.stdout, "new"),
                [expected_new.as_str()],
                "{fixture} {mode}:\n{}",
                run.show()
            );
            assert!(labelled(&run.stdout, "error").is_empty(), "{}", run.show());
            assert!(
                summary(&run.stdout).starts_with(&format!("spec check [{mode}]: "))
                    && summary(&run.stdout).contains(", 1 new debt, worst W ")
                    && summary(&run.stdout).ends_with(" — blocked"),
                "{}",
                run.show()
            );
            let json = repo.staged_check(&["--json"]);
            json.code(1);
            assert!(
                json.stdout.contains(&format!(
                    ",\"new_debt\":[{{\"code\":\"ref-dangling\",\"path\":\"docs/spec/none2.md\",\"subject\":\"X-2\",\"reason\":\"added\",\"expires\":\"2999-01-01\",\"line\":{line}}}],"
                )),
                "{fixture} {mode}:\n{}",
                json.stdout
            );
            assert_eq!(json.json()["counts"]["new_debt"], 1);
        }
        write(&repo.top, "specengine.toml", &config);

        let later = entry(
            "ref-dangling",
            "docs/spec/none.md",
            "X-1",
            "stale",
            "2998-07-01",
        );
        let earlier = entry(
            "ref-dangling",
            "docs/spec/none.md",
            "X-1",
            "stale",
            "2998-06-29",
        );
        let reasoned = entry(
            "ref-dangling",
            "docs/spec/none.md",
            "X-1",
            "renewed",
            "2998-06-30",
        );
        for (case, text, code) in [
            ("a later expiry", format!("{covering}{later}"), 1),
            ("an earlier expiry", format!("{covering}{earlier}"), 0),
            ("a changed reason", format!("{covering}{reasoned}"), 0),
            ("a removed entry", covering.clone(), 0),
            ("unchanged", head.clone(), 0),
        ] {
            write(&repo.top, ".spec-debt.toml", &text);
            repo.add_all();
            let run = repo.staged_check(&[]);
            assert_eq!(run.code, code, "{fixture} {case}:\n{}", run.show());
            let new = labelled(&run.stdout, "new");
            if code == 1 {
                let line = entry_line(&text, "docs/spec/none.md");
                assert_eq!(
                    new,
                    [format!(
                        "new  docs/spec/none.md: debt-new: the baseline entry at line {line} (ref-dangling, subject \"X-1\") extends HEAD's expiry 2998-06-30 (stale; expires 2998-07-01)"
                    )],
                    "{fixture} {case}"
                );
            } else {
                assert!(new.is_empty(), "{fixture} {case}:\n{}", run.show());
                assert!(
                    summary(&run.stdout).contains(", 0 new debt, "),
                    "{fixture} {case}:\n{}",
                    run.show()
                );
            }
        }

        // An unborn HEAD: every staged entry is new.
        let unborn = Repo::of("intro-newdebt-unborn", fixture);
        write(
            &unborn.top,
            "specengine.toml",
            with_mode(
                &read_text(&unborn.top, "specengine.toml"),
                "enforce-introduced",
            ),
        );
        write(&unborn.top, ".spec-debt.toml", &head);
        unborn.add_all();
        let run = unborn.staged_check(&["--json"]);
        run.code(1);
        let entries = head.matches("[[debt]]").count();
        assert_eq!(
            run.json()["counts"]["new_debt"],
            entries as u64,
            "{fixture}"
        );
        assert_eq!(
            run.json()["new_debt"].as_array().map(Vec::len),
            Some(entries),
            "{fixture}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-10: the mode.
// ---------------------------------------------------------------------------

/// A copy of `fixture` whose `HEAD` holds `head` (a closure setting up and
/// committing its config), then the config staged with `mode` and an
/// introduced error (`docs/spec/zz-new.md`).
fn mode_case(fixture: &str, staged_mode: &str, head: impl FnOnce(&Repo, &str)) -> Repo {
    let repo = Repo::of("intro-mode", fixture);
    let config = read_text(&repo.top, "specengine.toml");
    head(&repo, &config);
    write(
        &repo.top,
        "specengine.toml",
        with_mode(&config, staged_mode),
    );
    write(&repo.top, "docs/spec/zz-new.md", dangling_document(fixture));
    repo.add_all();
    repo
}

/// AC-10 (2a.2 Q6): the stricter of `HEAD`'s and the staged mode applies.
/// HEAD/staged `enforce`/`observe` → `[enforce]`, exit 1, one note naming
/// `HEAD`'s mode; `enforce-introduced`/`observe` → `[enforce-introduced]`;
/// `observe`/`enforce` → `[enforce]`, no note; `HEAD`'s config invalid
/// TOML, invalid check tables, a symlink, a missing blob → the staged
/// mode, one note (the first two, exactly: `HEAD's specengine.toml is not
/// a valid config (line N): …` at the offending line of `HEAD`'s text);
/// no config at `HEAD` → the staged mode, no note.
#[test]
fn the_stricter_of_head_s_and_the_staged_mode_applies() {
    for (fixture, _) in FIXTURES {
        let committed = |mode: &'static str| {
            move |repo: &Repo, config: &str| {
                write(&repo.top, "specengine.toml", with_mode(config, mode));
                commit_all(repo, mode);
            }
        };
        let stricter = |mode: &str, checked: &str| {
            format!(
                "note: HEAD's specengine.toml sets mode `{mode}`, stricter than `{checked}`: the check runs in `{mode}`"
            )
        };
        // Invalid TOML and invalid check tables at `HEAD` read alike, at the
        // offending line of `HEAD`'s text (iteration 2's wording).
        let fixture_config = read_text(&common::fixture(fixture), "specengine.toml");
        let not_valid = |head_text: &str, start: &str| {
            format!(
                "note: HEAD's specengine.toml is not a valid config (line {}): HEAD's mode is unknown, so the check runs in `observe`",
                line_of(head_text, start)
            )
        };
        let invalid_toml = not_valid(&format!("{fixture_config}\n[check\n"), "[check");
        let bogus_mode = not_valid(&with_mode(&fixture_config, "bogus"), "mode = ");
        let cases: Vec<(&str, Repo, &str, i32, Option<String>)> = vec![
            (
                "enforce/observe",
                mode_case(fixture, "observe", committed("enforce")),
                "enforce",
                1,
                Some(stricter("enforce", "observe")),
            ),
            (
                "enforce-introduced/observe",
                mode_case(fixture, "observe", committed("enforce-introduced")),
                "enforce-introduced",
                1,
                Some(stricter("enforce-introduced", "observe")),
            ),
            (
                "enforce/enforce-introduced",
                mode_case(fixture, "enforce-introduced", committed("enforce")),
                "enforce",
                1,
                Some(stricter("enforce", "enforce-introduced")),
            ),
            (
                "observe/enforce",
                mode_case(fixture, "enforce", committed("observe")),
                "enforce",
                1,
                None,
            ),
            (
                "invalid TOML/observe",
                mode_case(fixture, "observe", |repo, config| {
                    write(&repo.top, "specengine.toml", format!("{config}\n[check\n"));
                    commit_all(repo, "invalid TOML");
                }),
                "observe",
                0,
                Some(invalid_toml),
            ),
            (
                "invalid check tables/observe",
                mode_case(fixture, "observe", |repo, config| {
                    write(&repo.top, "specengine.toml", with_mode(config, "bogus"));
                    commit_all(repo, "a bogus mode");
                }),
                "observe",
                0,
                Some(bogus_mode),
            ),
            (
                "a symlink/observe",
                mode_case(fixture, "observe", |repo, config| {
                    write(&repo.top, "real.toml", with_mode(config, "enforce"));
                    fs::remove_file(repo.top.join("specengine.toml")).unwrap();
                    symlink("real.toml", repo.top.join("specengine.toml")).unwrap();
                    commit_all(repo, "a symlinked config");
                    fs::remove_file(repo.top.join("specengine.toml")).unwrap();
                }),
                "observe",
                0,
                Some(String::new()),
            ),
            (
                "a missing blob/observe",
                mode_case(fixture, "observe", |repo, config| {
                    write(&repo.top, "specengine.toml", with_mode(config, "enforce"));
                    commit_all(repo, "a config whose blob goes");
                    let oid = repo.git.staged_oid(&repo.top, "specengine.toml");
                    fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
                }),
                "observe",
                0,
                Some(String::new()),
            ),
            (
                "none/observe",
                mode_case(fixture, "observe", |repo, _| {
                    write(&repo.top, "README.txt", "no config yet\n");
                    repo.git(&["add", "README.txt"]);
                    commit(repo, "no config");
                }),
                "observe",
                0,
                None,
            ),
        ];
        for (case, repo, mode, code, note) in cases {
            let run = repo.staged_check(&[]);
            assert_eq!(run.code, code, "{fixture} {case}:\n{}", run.show());
            assert!(
                summary(&run.stdout).starts_with(&format!("spec check [{mode}]: ")),
                "{fixture} {case}:\n{}",
                run.show()
            );
            assert!(labelled(&run.stdout, "cannot").is_empty(), "{}", run.show());
            match note {
                None => assert_eq!(run.stderr, "", "{fixture} {case}"),
                Some(expected) => {
                    assert_eq!(notes(&run).len(), 1, "{fixture} {case}:\n{}", run.show());
                    assert_eq!(run.stderr.lines().count(), 1, "{}", run.show());
                    if expected.is_empty() {
                        assert!(
                            notes(&run)[0].starts_with("note: HEAD's specengine.toml ")
                                && notes(&run)[0].ends_with(
                                    "HEAD's mode is unknown, so the check runs in `observe`"
                                ),
                            "{fixture} {case}: {}",
                            notes(&run)[0]
                        );
                    } else {
                        assert_eq!(notes(&run)[0], expected, "{fixture} {case}");
                    }
                }
            }
            // `--json` carries the mode, never a note.
            let json = repo.staged_check(&["--json"]);
            assert_eq!(json.json()["mode"], mode, "{fixture} {case}");
            assert!(!json.stdout.contains("note"), "{fixture} {case}");
        }
    }
}

// ---------------------------------------------------------------------------
// AC-11: given files outside the root.
// ---------------------------------------------------------------------------

/// AC-11 (Q3): `--baseline` outside the repository, or inside it but
/// outside `--root`, holding entries `HEAD` lacks: the rule lifted, no
/// block, one note naming the flag as typed, no `new_debt`; under the root
/// it is compared with `HEAD`'s blob at its root-relative path (not
/// `HEAD`'s root baseline); `--config` outside the root, `HEAD` `enforce`,
/// given `observe` → `[observe]`, one note.
#[test]
fn given_files_outside_the_root_are_not_judged_against_head() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("intro-given");
        let git = Sandbox::new(scratch.path());
        let top = scratch.dir("repo");
        let proj = top.join("proj");
        copy_dir(&common::fixture(fixture), &proj);
        let config = read_text(&proj, "specengine.toml");
        write(&proj, "specengine.toml", with_mode(&config, "enforce"));
        write(&top, "README.txt", "outside the root\n");
        git.init(&top);
        git.add_all(&top);
        git.commit(&top, "the backlog, no baseline");
        let covering = baseline_covering(&library(&proj), FAR);
        let outside = scratch.path().join("outside-debt.toml");
        fs::write(&outside, &covering).unwrap();
        write(&top, "other/debt.toml", &covering);
        let check = |extra: &[&str]| {
            let mut args = vec!["--root", "proj"];
            args.extend_from_slice(extra);
            spec_in(&git, &top, &args, &[])
        };
        let outside_text = outside.to_str().unwrap();
        for typed in [outside_text, "other/debt.toml"] {
            let run = check(&["check", "--staged", "--baseline", typed]);
            run.code(0);
            assert!(summary(&run.stdout).ends_with(" — clean"), "{}", run.show());
            assert!(!summary(&run.stdout).contains("new debt"), "{}", run.show());
            assert_eq!(
                notes(&run),
                [format!(
                    "note: --baseline {typed} is outside the root: new baseline entries are not judged against HEAD"
                )
                .as_str()],
                "{fixture} {typed}:\n{}",
                run.show()
            );
            let json = check(&["--json", "check", "--staged", "--baseline", typed]);
            json.code(0);
            assert!(!json.stdout.contains("new_debt"), "{}", json.stdout);
        }

        // Under the root: compared with HEAD's blob at `given.toml`.
        write(&proj, "given.toml", &covering);
        let run = check(&["check", "--staged", "--baseline", "proj/given.toml"]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "new").len(),
            covering.matches("[[debt]]").count(),
            "{fixture}:\n{}",
            run.show()
        );
        assert_eq!(run.stderr, "", "{fixture}");
        // HEAD's own root baseline holds the entries: still new at
        // `given.toml`.
        write(&proj, ".spec-debt.toml", &covering);
        git.git(&top, &["add", "proj/.spec-debt.toml"]);
        git.commit(&top, "the root's baseline");
        let run = check(&["check", "--staged", "--baseline", "proj/given.toml"]);
        run.code(1);
        // Committed at its own path: not new.
        git.git(&top, &["add", "proj/given.toml"]);
        git.commit(&top, "the given baseline");
        let run = check(&["check", "--staged", "--baseline", "proj/given.toml"]);
        run.code(0);
        assert!(
            summary(&run.stdout).contains(", 0 new debt, "),
            "{}",
            run.show()
        );

        // `--config` outside the root: HEAD's `enforce` is not known.
        let observe = scratch.path().join("observe.toml");
        fs::write(&observe, with_mode(&config, "observe")).unwrap();
        let typed = observe.to_str().unwrap();
        write(&proj, "docs/spec/zz-new.md", dangling_document(fixture));
        git.add_all(&top);
        let run = check(&["--config", typed, "check", "--staged"]);
        run.code(0);
        assert!(
            summary(&run.stdout).starts_with("spec check [observe]: "),
            "{fixture}:\n{}",
            run.show()
        );
        assert_eq!(
            notes(&run),
            [format!(
                "note: --config {typed} is outside the root: HEAD's mode is unknown, so the check runs in `observe`"
            )
            .as_str()],
            "{fixture}:\n{}",
            run.show()
        );
        // Inside the root, no counterpart at HEAD: the given mode, no note.
        write(&proj, "alt.toml", with_mode(&config, "observe"));
        let run = check(&["--config", "proj/alt.toml", "check", "--staged"]);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        // Committed there as `enforce`: the stricter applies, a note.
        write(&proj, "alt.toml", with_mode(&config, "enforce"));
        git.git(&top, &["add", "proj/alt.toml"]);
        git.commit(&top, "alt as enforce");
        write(&proj, "alt.toml", with_mode(&config, "observe"));
        let run = check(&["--config", "proj/alt.toml", "check", "--staged"]);
        run.code(1);
        assert_eq!(notes(&run).len(), 1, "{}", run.show());
    }
}

// ---------------------------------------------------------------------------
// AC-11b: HEAD's baseline not known.
// ---------------------------------------------------------------------------

/// AC-11b (Q4): `HEAD`'s `.spec-debt.toml` a symlink, a missing blob,
/// invalid TOML: the new-debt rule is lifted — one note, no block from the
/// staged entries `HEAD` lacks, no `new_debt` key.
#[test]
fn head_s_unknown_baseline_lifts_the_rule() {
    for (fixture, _) in FIXTURES {
        type Setup = fn(&Repo);
        let cases: [(&str, Setup); 3] = [
            ("a symlink", |repo| {
                write(&repo.top, "real-debt.toml", "");
                symlink("real-debt.toml", repo.top.join(".spec-debt.toml")).unwrap();
                commit_all(repo, "a symlinked baseline");
                fs::remove_file(repo.top.join(".spec-debt.toml")).unwrap();
            }),
            ("a missing blob", |repo| {
                write(
                    &repo.top,
                    ".spec-debt.toml",
                    entry("budget", "docs/none.md", "", "gone", FAR),
                );
                commit_all(repo, "a baseline whose blob goes");
                let oid = repo.git.staged_oid(&repo.top, ".spec-debt.toml");
                fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
            }),
            ("invalid TOML", |repo| {
                write(&repo.top, ".spec-debt.toml", "[[debt]\n");
                commit_all(repo, "a broken baseline");
            }),
        ];
        for (case, setup) in cases {
            let repo = backlog("intro-lifted", fixture, "enforce-introduced");
            let covering = baseline_covering(&library(&repo.top), FAR);
            setup(&repo);
            write(&repo.top, ".spec-debt.toml", &covering);
            repo.add_all();
            let run = repo.staged_check(&[]);
            run.code(0);
            assert!(summary(&run.stdout).ends_with(" — clean"), "{}", run.show());
            assert!(!summary(&run.stdout).contains("new debt"), "{}", run.show());
            assert!(labelled(&run.stdout, "new").is_empty(), "{}", run.show());
            assert_eq!(notes(&run).len(), 1, "{fixture} {case}:\n{}", run.show());
            assert!(
                notes(&run)[0].starts_with("note: HEAD's .spec-debt.toml ")
                    && notes(&run)[0]
                        .ends_with(": new baseline entries are not judged against HEAD"),
                "{fixture} {case}: {}",
                notes(&run)[0]
            );
            let json = repo.staged_check(&["--json"]);
            json.code(0);
            assert!(
                !json.stdout.contains("new_debt"),
                "{fixture} {case}: {}",
                json.stdout
            );
            assert!(json.stdout.contains("\"introduced\":"), "{}", json.stdout);
        }
    }
}

// ---------------------------------------------------------------------------
// AC-16: determinism; the line order.
// ---------------------------------------------------------------------------

/// A repository at `<scratch>/<dir>/repo`: `fixture` in `enforce`, its
/// files added one by one (path order or reversed) and committed, then
/// `enforce-introduced` staged (`HEAD`'s stricter mode: a note), an
/// introduced error, a changed document and a baseline with an entry
/// `HEAD` lacks.
fn twin(
    scratch: &Scratch,
    git: &Sandbox,
    fixture: &str,
    dir: &str,
    reverse: bool,
) -> std::path::PathBuf {
    let top = scratch.copy(fixture, &format!("{dir}/repo"));
    let config = read_text(&top, "specengine.toml");
    write(&top, "specengine.toml", with_mode(&config, "enforce"));
    git.init(&top);
    let mut files: Vec<String> = common::snapshot(&top)
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && !path.starts_with(".git"))
        .map(|(path, _)| path)
        .collect();
    if reverse {
        files.reverse();
    }
    for file in &files {
        git.git(&top, &["add", "--", file]);
    }
    git.commit(&top, "the backlog");
    let mut later = vec![
        ("specengine.toml", with_mode(&config, "enforce-introduced")),
        ("docs/spec/zz-new.md", dangling_document(fixture)),
        (
            ".spec-debt.toml",
            entry("budget", "docs/none.md", "", "new", FAR),
        ),
        ("docs/spec/zz-clean.md", clean_document("Clean")),
    ];
    if reverse {
        later.reverse();
    }
    for (path, text) in later {
        write(&top, path, text);
        git.git(&top, &["add", "--", path]);
    }
    top
}

/// AC-16: with a base, `counts.introduced` (then `new_debt`) precedes
/// `worst_w_bytes`, the summary ends `, worst W <n> B — <verdict>`; two
/// repositories at different absolute paths, their files added in
/// opposite orders (the same tree, `HEAD` tree, config, baseline, date):
/// identical stdout and stderr in every output form.
#[test]
fn the_judged_report_is_the_same_across_repositories() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("intro-determinism");
        let git = Sandbox::new(scratch.path());
        let one = twin(&scratch, &git, fixture, "a", false);
        let two = twin(
            &scratch,
            &git,
            fixture,
            "a-much-longer-directory-name",
            true,
        );
        for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
            let args = check_args(true, form);
            let a = spec_in(&git, &one, &args, &[]);
            let b = spec_in(&git, &two, &args, &[]);
            a.code(1);
            assert!(
                a.code == b.code && a.stdout == b.stdout && a.stderr == b.stderr,
                "{fixture} {form:?}:\n{}\n{}",
                a.show(),
                b.show()
            );
            // HEAD's stricter mode; `--json --debt` adds its own note last.
            let json_debt = form == ["--json", "--debt"];
            assert_eq!(notes(&a).len(), 1 + usize::from(json_debt), "{}", a.show());
            assert_eq!(
                notes(&a)[0],
                "note: HEAD's specengine.toml sets mode `enforce`, stricter than `enforce-introduced`: the check runs in `enforce`",
                "{}",
                a.show()
            );
            if form.contains(&"--json") {
                let counts = a.json()["counts"].clone();
                let w = counts["worst_w_bytes"].as_u64().unwrap();
                assert!(w > 0);
                assert!(
                    a.stdout.contains(&format!(
                        ",\"stale\":{},\"introduced\":{},\"new_debt\":1,\"worst_w_bytes\":{w}}}",
                        counts["stale"], counts["introduced"]
                    )),
                    "{}",
                    a.stdout
                );
                assert_eq!(counts["introduced"], 1, "{}", a.stdout);
            } else {
                let line = summary(&a.stdout);
                let (head, tail) = line.split_once(", worst W ").expect("W in the summary");
                assert!(head.ends_with(", 1 introduced, 1 new debt"), "{line}");
                let (w, verdict) = tail.split_once(" B \u{2014} ").expect("` B — `");
                assert!(w.parse::<u64>().unwrap() > 0, "{line}");
                assert_eq!(verdict, "blocked", "{line}");
            }
        }
    }
}

/// The line order with a base (`--debt`): finding lines (error, warning,
/// debt), then `stale`, then `new`, then `cannot`, then the summary.
#[test]
fn new_lines_follow_stale_and_precede_causes() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-order", fixture, "enforce-introduced");
        let config = read_text(&repo.top, "specengine.toml");
        let covering = baseline_covering(&library(&repo.top), FAR);
        write(
            &repo.top,
            ".spec-debt.toml",
            format!(
                "{covering}{}",
                entry("budget", "docs/zz-none.md", "", "stale and new", FAR)
            ),
        );
        write(&repo.top, "docs/spec/zz-new.md", dangling_document(fixture));
        write(
            &repo.top,
            "specengine.toml",
            common::check::set_paths_key(&config, "roots", "[\"docs\", \"nowhere\"]"),
        );
        repo.add_all();
        let run = repo.staged_check(&["--debt"]);
        run.code(2);
        let rank = |line: &str| match line.split("  ").next().unwrap_or_default() {
            "error" | "warning" | "debt" => 0,
            "stale" => 1,
            "new" => 2,
            "cannot" => 3,
            _ if line.starts_with("spec check [") => 4,
            other => panic!("{fixture}: an unknown line {other:?} in\n{}", run.show()),
        };
        let ranks: Vec<u8> = run.stdout.lines().map(rank).collect();
        for kind in 0..=4 {
            assert!(
                ranks.contains(&kind),
                "{fixture}: no line of rank {kind}\n{}",
                run.show()
            );
        }
        let mut sorted = ranks.clone();
        sorted.sort();
        assert_eq!(ranks, sorted, "{fixture}: the line order\n{}", run.show());
    }
}
