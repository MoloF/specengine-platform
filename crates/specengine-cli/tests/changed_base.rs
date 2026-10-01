//! docs/features/spec-cli-changed.md through the `spec` binary: AC-07
//! (the mode, the notes and new debt — docs/features/spec-cli-introduced.md
//! AC-09, AC-10, AC-11 and AC-11b re-run with `--changed`, the checked side
//! left unstaged, so the index holds `HEAD`'s bytes) and AC-09 (an unborn
//! `HEAD`, a new root: introduced AC-06 re-run with `--changed`, through a
//! logging wrapper). Scratch repositories hold copies of spec-a and
//! spec-b, `HEAD` committed with `--no-verify`; every git process runs in
//! the sandbox of `common::git`; no git runs in this repository.

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::symlink;

use common::check::{
    FAR, baseline_covering, dangling_document, library, line_of, quoted, undefined_id,
};
use common::git::Sandbox;
use common::staged::{GitLog, Repo, check_args, spec_in};
use common::{FIXTURES, Run, Scratch, copy_dir, read_text, write};

/// `check --changed <args>`, the globals kept before `check`.
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

fn notes(run: &Run) -> Vec<&str> {
    run.stderr
        .lines()
        .filter(|line| line.starts_with("note: "))
        .collect()
}

fn with_mode(config: &str, mode: &str) -> String {
    format!("{config}\n[check]\nmode = \"{mode}\"\n")
}

fn backlog(label: &str, fixture: &str, mode: &str) -> Repo {
    let repo = Repo::of(label, fixture);
    let config = read_text(&repo.top, "specengine.toml");
    write(&repo.top, "specengine.toml", with_mode(&config, mode));
    repo.add_all();
    repo.git.commit(&repo.top, "the backlog");
    repo
}

fn commit(repo: &Repo, message: &str) {
    repo.git.commit(&repo.top, message);
}

fn commit_all(repo: &Repo, message: &str) {
    repo.add_all();
    commit(repo, message);
}

/// Nothing staged against `HEAD`: the checked side is on disk only.
fn assert_nothing_staged(repo: &Repo, context: &str) {
    let staged = repo.git(&["diff", "--cached", "--name-only"]);
    assert!(
        staged.is_empty(),
        "{context}: staged {}",
        String::from_utf8_lossy(&staged)
    );
}

fn entry(code: &str, path: &str, subject: &str, reason: &str, expires: &str) -> String {
    format!(
        "[[debt]]\ncode    = {}\npath    = {}\nsubject = {}\nreason  = {}\nexpires = \"{expires}\"\n\n",
        quoted(code),
        quoted(path),
        quoted(subject),
        quoted(reason)
    )
}

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

// ---------------------------------------------------------------------------
// AC-07: introduced AC-09 (new debt) with `--changed`.
// ---------------------------------------------------------------------------

/// Introduced AC-09 with `--changed`: against `HEAD`'s baseline, an entry
/// added on disk only blocks under `enforce-introduced` and `enforce`, one
/// `new` line; a later `expires` blocks; an earlier one, a changed
/// `reason`, a removed entry do not; an unborn `HEAD` makes every disk
/// entry new.
#[test]
fn new_debt_on_disk_blocks_in_both_enforcing_modes() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-newdebt", fixture, "enforce-introduced");
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
        changed(&repo, &[]).code(0);

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
            assert_nothing_staged(&repo, fixture);
            let run = changed(&repo, &[]);
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
            let json = changed(&repo, &["--json"]);
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
            let run = changed(&repo, &[]);
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

        // An unborn HEAD: every disk entry is new.
        let unborn = Repo::of("chg-newdebt-unborn", fixture);
        write(
            &unborn.top,
            "specengine.toml",
            with_mode(
                &read_text(&unborn.top, "specengine.toml"),
                "enforce-introduced",
            ),
        );
        write(&unborn.top, ".spec-debt.toml", &head);
        let run = changed(&unborn, &["--json"]);
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
// AC-07: introduced AC-10 (the stricter mode) with `--changed`.
// ---------------------------------------------------------------------------

/// A copy of `fixture` whose `HEAD` holds what `head` commits, then on
/// disk only the config with `mode` and an introduced error.
fn mode_case(fixture: &str, disk_mode: &str, head: impl FnOnce(&Repo, &str)) -> Repo {
    let repo = Repo::of("chg-mode", fixture);
    let config = read_text(&repo.top, "specengine.toml");
    head(&repo, &config);
    write(&repo.top, "specengine.toml", with_mode(&config, disk_mode));
    write(&repo.top, "docs/spec/zz-new.md", dangling_document(fixture));
    repo
}

/// Introduced AC-10 with `--changed`: the stricter of `HEAD`'s and the
/// disk's mode applies, with the same notes.
#[test]
fn the_stricter_of_head_s_and_the_disk_mode_applies() {
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
            let run = changed(&repo, &[]);
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
            let json = changed(&repo, &["--json"]);
            assert_eq!(json.json()["mode"], mode, "{fixture} {case}");
            assert!(!json.stdout.contains("note"), "{fixture} {case}");
        }
    }
}

// ---------------------------------------------------------------------------
// AC-07: introduced AC-11 (given files) with `--changed`.
// ---------------------------------------------------------------------------

/// Introduced AC-11 with `--changed`: `--baseline` outside the repository,
/// or inside it but outside `--root` → the rule lifted, one note; under
/// the root it is compared with `HEAD`'s blob at its root-relative path,
/// not `HEAD`'s root baseline; `--config` outside the root → `HEAD`'s mode
/// unknown, one note; inside the root, `HEAD`'s file at the same
/// root-relative path decides.
#[test]
fn given_files_are_placed_as_with_staged() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("chg-given");
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
            let run = check(&["check", "--changed", "--baseline", typed]);
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
            let json = check(&["--json", "check", "--changed", "--baseline", typed]);
            json.code(0);
            assert!(!json.stdout.contains("new_debt"), "{}", json.stdout);
        }

        // Under the root: compared with HEAD's blob at `given.toml`.
        write(&proj, "given.toml", &covering);
        let run = check(&["check", "--changed", "--baseline", "proj/given.toml"]);
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
        let run = check(&["check", "--changed", "--baseline", "proj/given.toml"]);
        run.code(1);
        // Committed at its own path: not new.
        git.git(&top, &["add", "proj/given.toml"]);
        git.commit(&top, "the given baseline");
        let run = check(&["check", "--changed", "--baseline", "proj/given.toml"]);
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
        let run = check(&["--config", typed, "check", "--changed"]);
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
        let run = check(&["--config", "proj/alt.toml", "check", "--changed"]);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        // Committed there as `enforce`, `observe` on disk: the stricter
        // applies, a note.
        write(&proj, "alt.toml", with_mode(&config, "enforce"));
        git.git(&top, &["add", "proj/alt.toml"]);
        git.commit(&top, "alt as enforce");
        write(&proj, "alt.toml", with_mode(&config, "observe"));
        let run = check(&["--config", "proj/alt.toml", "check", "--changed"]);
        run.code(1);
        assert_eq!(
            notes(&run),
            [
                "note: HEAD's proj/alt.toml sets mode `enforce`, stricter than `observe`: the check runs in `enforce`"
            ],
            "{fixture}:\n{}",
            run.show()
        );
        // `--staged`'s note, word for word.
        let staged = check(&["--config", "proj/alt.toml", "check", "--staged"]);
        assert_eq!(notes(&staged), notes(&run), "{}", staged.show());
    }
}

// ---------------------------------------------------------------------------
// AC-07: introduced AC-11b (`HEAD`'s baseline not known) with `--changed`.
// ---------------------------------------------------------------------------

/// Introduced AC-11b with `--changed`: `HEAD`'s `.spec-debt.toml` a
/// symlink, a missing blob, invalid TOML → the new-debt rule is lifted:
/// one note, no block from the disk's entries `HEAD` lacks, no `new_debt`.
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
            let repo = backlog("chg-lifted", fixture, "enforce-introduced");
            let covering = baseline_covering(&library(&repo.top), FAR);
            setup(&repo);
            write(&repo.top, ".spec-debt.toml", &covering);
            let run = changed(&repo, &[]);
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
            let json = changed(&repo, &["--json"]);
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
// AC-09: an unborn `HEAD`, a new root.
// ---------------------------------------------------------------------------

/// The subcommands a logged run called.
fn subcommands(log: &GitLog) -> Vec<String> {
    log.calls()
        .iter()
        .map(|call| call.name().to_owned())
        .collect()
}

/// AC-09 (introduced AC-06 with `--changed`): with no commit, `--changed`
/// prints `enforce`'s blocking lines, exit 1, no cause, no note, and runs
/// only `rev-parse` (no `ls-tree`, no `cat-file`); a `--root proj`
/// untracked in a born repository likewise; once `proj` is committed with
/// only its pre-existing errors: exit 0, also from inside `proj`.
#[test]
fn an_unborn_head_or_a_new_root_introduces_everything() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("chg-unborn", fixture);
        let config = read_text(&repo.top, "specengine.toml");
        write(
            &repo.top,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        assert_eq!(repo.git.head(&repo.top), None);
        let twin = Scratch::new("chg-unborn-twin");
        let enforced = twin.copy(fixture, "copy");
        let enforce = spec_in(&Sandbox::new(twin.path()), &enforced, &["check"], &[]);
        enforce.code(1);
        let log = GitLog::new(&repo.scratch, &repo.git);
        for form in [&[][..], &["--json"]] {
            log.clear();
            let run = spec_in(&repo.git, &repo.top, &changed_args(form), &log.env());
            run.code(1);
            assert_eq!(run.stderr, "", "{fixture}");
            assert_eq!(
                subcommands(&log),
                ["rev-parse", "rev-parse"],
                "{fixture}: {:?}",
                log.calls()
            );
            if form.is_empty() {
                assert_eq!(
                    labelled(&run.stdout, "error"),
                    labelled(&enforce.stdout, "error"),
                    "{fixture}:\n{}",
                    run.show()
                );
                assert!(labelled(&run.stdout, "cannot").is_empty(), "{}", run.show());
                assert!(
                    summary(&run.stdout).starts_with("spec check [enforce-introduced]: "),
                    "{}",
                    run.show()
                );
            } else {
                assert!(
                    run.json()["findings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|finding| finding["introduced"] == true),
                    "{}",
                    run.stdout
                );
            }
        }

        // A new root `proj`, untracked, in a born repository.
        let scratch = Scratch::new("chg-new-root");
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
        let head = git.head(&top).expect("born");
        let log = GitLog::new(&scratch, &git);
        let args = ["--root", "proj", "check", "--changed"];
        let run = spec_in(&git, &top, &args, &log.env());
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            labelled(&enforce.stdout, "error"),
            "{fixture} --root proj:\n{}",
            run.show()
        );
        assert!(labelled(&run.stdout, "cannot").is_empty(), "{}", run.show());
        assert_eq!(run.stderr, "", "{fixture}");
        let trees: Vec<Vec<String>> = log
            .calls()
            .iter()
            .filter(|call| call.name() == "ls-tree")
            .map(|call| call.sub().iter().map(|arg| (*arg).to_owned()).collect())
            .collect();
        assert_eq!(
            trees,
            [vec![
                "ls-tree".to_owned(),
                "-r".to_owned(),
                "-z".to_owned(),
                head
            ]],
            "{fixture}"
        );

        // Committed with only its pre-existing errors: exit 0.
        git.add_all(&top);
        git.commit(&top, "the project");
        let run = spec_in(&git, &top, &args, &[]);
        run.code(0);
        assert!(
            summary(&run.stdout).ends_with(" — observed")
                && summary(&run.stdout).contains(", 0 introduced, 0 new debt, worst W "),
            "{fixture}:\n{}",
            run.show()
        );
        // The same from inside `proj`.
        let run = spec_in(&git, &proj, &["check", "--changed"], &[]);
        run.code(0);
        // An error added on disk under `proj` is the one introduced.
        write(&proj, "docs/spec/zz-new.md", dangling_document(fixture));
        let run = spec_in(&git, &top, &args, &[]);
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
    }
}
