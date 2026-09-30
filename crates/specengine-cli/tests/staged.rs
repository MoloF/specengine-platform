//! AC-02, AC-03, AC-04, AC-14 and AC-17 of docs/features/spec-cli-staged.md:
//! `spec check --staged` on scratch git repositories holding copies of
//! spec-a and spec-b. "Plain" is `spec check` on the same repository; a
//! fully staged tree prints the same bytes (stdout, stderr, exit) in every
//! output form, while what is staged, not what is on disk, decides the
//! verdict, the config and the baseline. Every git process runs in the
//! sandbox of `common::git` (no test runs git in this repository).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};

use common::check::{FAR, baseline_covering, dangling_document, library, line_of, set_paths_key};
use common::staged::{Repo, assert_parity, assert_same};
use common::{FIXTURES, read_text, write};
use serde_json::Value;
use specengine_core::check::Verdict;

/// The summary line: the last of stdout.
fn summary(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or_default()
}

/// The `error  ` lines of a text report.
fn errors(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("error  "))
        .collect()
}

/// The `cannot  ` lines of a text report.
fn causes(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("cannot  "))
        .collect()
}

/// `counts.documents` of a `--json` report.
fn documents(stdout: &str) -> u64 {
    let json: Value = serde_json::from_str(stdout).expect("a JSON report");
    json["counts"]["documents"].as_u64().expect("documents")
}

/// A spec document with no finding of its own in either fixture.
fn clean_document(title: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# {title}\n\nText.\n")
}

/// The fixture's config with `[check] mode = "<mode>"` appended.
fn with_mode(config: &str, mode: &str) -> String {
    format!("{config}\n[check]\nmode = \"{mode}\"\n")
}

/// A staged copy of `fixture` whose baseline (staged) covers every
/// blocking finding: `--staged` and plain are clean.
fn clean_repo(name: &str, fixture: &str) -> Repo {
    let repo = Repo::of(name, fixture);
    let blocked = library(&repo.top);
    assert_eq!(blocked.verdict, Verdict::Blocked, "{fixture} as copied");
    write(
        &repo.top,
        ".spec-debt.toml",
        baseline_covering(&blocked, FAR),
    );
    repo.add_all();
    let run = repo.staged_check(&[]);
    run.code(0);
    assert!(summary(&run.stdout).ends_with(" — clean"), "{}", run.show());
    repo
}

/// AC-02 (and AC-17: both fixtures): a dot-directory, an excluded, a
/// `100755` and a symlinked `.md` under a root, a document outside every
/// root, `git add -A`: `--staged` prints plain's bytes — text, `--debt`,
/// `--json`, `--json --debt` — when blocked, clean (a covering baseline),
/// observed and cannot-check (a written root in neither).
#[test]
fn a_fully_staged_tree_prints_plain_s_bytes_for_every_verdict() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("staged-parity", fixture);
        let top = &repo.top;
        let config = read_text(top, "specengine.toml");
        write(
            top,
            "specengine.toml",
            set_paths_key(&config, "exclude", "[\"docs/spec/excluded-*.md\"]"),
        );
        write(top, "docs/spec/.hidden/dot.md", dangling_document(fixture));
        write(top, "docs/spec/excluded-one.md", dangling_document(fixture));
        write(top, "docs/spec/exec.md", dangling_document(fixture));
        fs::set_permissions(
            top.join("docs/spec/exec.md"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        write(top, "outside/linked-target.md", dangling_document(fixture));
        symlink(
            "../../outside/linked-target.md",
            top.join("docs/spec/link.md"),
        )
        .unwrap();
        // Outside the default roots of spec-a (`[paths]` writes no roots
        // there), inside spec-b's `docs`: a `docs/` fallback shows.
        write(top, "docs/zz-outside.md", dangling_document(fixture));
        repo.add_all();

        let modes = String::from_utf8(repo.git(&[
            "ls-files",
            "-s",
            "--",
            "docs/spec/exec.md",
            "docs/spec/link.md",
        ]))
        .unwrap();
        assert!(
            modes.contains("100755 ") && modes.contains("120000 "),
            "{fixture}: the setup stages a 100755 and a symlink:\n{modes}"
        );

        // Blocked.
        let blocked = repo.staged_check(&[]);
        blocked.code(1);
        assert!(summary(&blocked.stdout).ends_with(" — blocked"));
        assert!(
            blocked.stdout.contains("docs/spec/exec.md:"),
            "{fixture}: the 100755 document is walked\n{}",
            blocked.stdout
        );
        for skipped in ["dot.md", "excluded-one.md", "link.md", "linked-target.md"] {
            assert!(
                !blocked.stdout.contains(skipped),
                "{fixture}: {skipped} is not walked\n{}",
                blocked.stdout
            );
        }
        assert_parity(&repo, &[], &format!("{fixture} blocked"));

        // Clean: a staged baseline covering every blocking finding.
        let report = library(top);
        write(top, ".spec-debt.toml", baseline_covering(&report, FAR));
        repo.add_all();
        let clean = repo.staged_check(&[]);
        clean.code(0);
        assert!(
            summary(&clean.stdout).ends_with(" — clean"),
            "{}",
            clean.show()
        );
        assert_parity(&repo, &[], &format!("{fixture} clean"));

        // Observed: the baseline gone, `mode = "observe"`.
        repo.git(&["rm", "-q", "-f", ".spec-debt.toml"]);
        let excluding = read_text(top, "specengine.toml");
        write(top, "specengine.toml", with_mode(&excluding, "observe"));
        repo.add_all();
        let observed = repo.staged_check(&[]);
        observed.code(0);
        assert!(
            summary(&observed.stdout).ends_with(" — observed"),
            "{}",
            observed.show()
        );
        assert_parity(&repo, &[], &format!("{fixture} observed"));

        // Cannot-check: a written root in neither the index nor the disk.
        write(
            top,
            "specengine.toml",
            set_paths_key(&excluding, "roots", "[\"docs\", \"nowhere\"]"),
        );
        repo.add_all();
        let cannot = repo.staged_check(&[]);
        cannot.code(2);
        assert!(
            summary(&cannot.stdout).ends_with(" — cannot-check"),
            "{}",
            cannot.show()
        );
        assert_parity(&repo, &[], &format!("{fixture} cannot-check"));
    }
}

/// AC-03 (a)–(d): what is staged is judged, not the working tree.
#[test]
fn the_staged_bytes_decide_not_the_working_tree() {
    for (fixture, _) in FIXTURES {
        let repo = clean_repo("staged-bytes", fixture);
        let top = &repo.top;
        let tracked = documents(&repo.staged_check(&["--json"]).stdout);

        // (a) An error staged, fixed only on disk: exit 1, one error at
        // the staged line; plain passes.
        write(top, "docs/spec/zz-err.md", dangling_document(fixture));
        repo.add_all();
        write(top, "docs/spec/zz-err.md", clean_document("Fixed on disk"));
        let staged = repo.staged_check(&[]);
        staged.code(1);
        let found = errors(&staged.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", staged.stdout);
        let line = line_of(&dangling_document(fixture), "refs:");
        assert!(
            found[0].starts_with(&format!("error  docs/spec/zz-err.md:{line}: ")),
            "{fixture}: {}",
            found[0]
        );
        repo.plain_check(&[]).code(0);

        // (b) The reverse: staged clean, the error only on disk.
        repo.add_all();
        repo.staged_check(&[]).code(0);
        write(top, "docs/spec/zz-err.md", dangling_document(fixture));
        repo.staged_check(&[]).code(0);
        repo.plain_check(&[]).code(1);
        write(top, "docs/spec/zz-err.md", clean_document("Fixed on disk"));

        // (c) An erroneous untracked document under a root: not walked,
        // `documents` is the tracked count.
        let with_new = documents(&repo.staged_check(&["--json"]).stdout);
        assert_eq!(with_new, tracked + 1, "{fixture}: zz-err.md is tracked");
        write(top, "docs/spec/zz-untracked.md", dangling_document(fixture));
        let run = repo.staged_check(&["--json"]);
        run.code(0);
        assert_eq!(documents(&run.stdout), with_new, "{fixture} untracked");
        let plain = repo.plain_check(&["--json"]);
        plain.code(1);
        assert_eq!(documents(&plain.stdout), with_new + 1, "{fixture} plain");
        fs::remove_file(top.join("docs/spec/zz-untracked.md")).unwrap();

        // (d) `git rm --cached`, the erroneous file kept: not walked.
        write(top, "docs/spec/zz-err.md", dangling_document(fixture));
        repo.add_all();
        repo.staged_check(&[]).code(1);
        repo.git(&["rm", "-q", "--cached", "docs/spec/zz-err.md"]);
        assert!(top.join("docs/spec/zz-err.md").is_file());
        let run = repo.staged_check(&["--json"]);
        run.code(0);
        assert_eq!(documents(&run.stdout), tracked, "{fixture} rm --cached");
        repo.plain_check(&[]).code(1);
    }
}

/// AC-03 (e): an edit staged and the registered index regenerated by
/// `spec export index` but not staged: `index-drift`, exit 1; staged:
/// clean.
#[test]
fn an_unstaged_regenerated_index_is_drift() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("staged-drift", fixture);
        let top = &repo.top;
        let index = common::check::index_path(fixture);
        let base = read_text(top, "specengine.toml");
        write(
            top,
            "specengine.toml",
            common::check::registered(&base, index, "gen-index", None),
        );
        repo.spec(&["export", "index"]).code(0);
        let blocked = library(top);
        write(top, ".spec-debt.toml", baseline_covering(&blocked, FAR));
        repo.add_all();
        repo.staged_check(&[]).code(0);
        repo.plain_check(&[]).code(0);

        // A new document staged, the index regenerated on disk only.
        write(top, "docs/spec/zz-new.md", clean_document("A new document"));
        repo.add_all();
        repo.spec(&["export", "index"]).code(0);
        repo.plain_check(&[]).code(0);
        let run = repo.staged_check(&["--json"]);
        run.code(1);
        let codes = common::check::codes(&run.stdout);
        assert!(
            codes.iter().any(|code| code == "index-drift"),
            "{fixture}: {codes:?}"
        );

        // The regenerated index staged: clean.
        repo.add_all();
        repo.staged_check(&[]).code(0);
        assert_parity(&repo, &[], &format!("{fixture} regenerated"));
    }
}

/// AC-04 (a), (b): the staged config's mode and the staged baseline decide;
/// the working tree's never do.
#[test]
fn the_staged_config_and_baseline_decide() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("staged-config", fixture);
        let top = &repo.top;
        let config = read_text(top, "specengine.toml");

        // (a) `observe` staged, `enforce` on disk.
        write(top, "specengine.toml", with_mode(&config, "observe"));
        repo.add_all();
        write(top, "specengine.toml", &config);
        let run = repo.staged_check(&[]);
        run.code(0);
        assert!(
            summary(&run.stdout).starts_with("spec check [observe]: ")
                && summary(&run.stdout).ends_with(" — observed"),
            "{fixture}: {}",
            run.stdout
        );
        repo.plain_check(&[]).code(1);

        // The reverse: `enforce` staged, `observe` on disk (Q3).
        repo.add_all();
        write(top, "specengine.toml", with_mode(&config, "observe"));
        let run = repo.staged_check(&[]);
        run.code(1);
        assert!(
            summary(&run.stdout).starts_with("spec check [enforce]: "),
            "{fixture}: {}",
            run.stdout
        );
        repo.plain_check(&[]).code(0);
        write(top, "specengine.toml", &config);

        // (b) A covering baseline only on disk: blocked; staged: debt.
        let blocked = library(top);
        let baseline = baseline_covering(&blocked, FAR);
        write(top, ".spec-debt.toml", &baseline);
        repo.plain_check(&[]).code(0);
        repo.staged_check(&[]).code(1);
        repo.add_all();
        let run = repo.staged_check(&["--debt"]);
        run.code(0);
        let debt = run
            .stdout
            .lines()
            .filter(|line| line.starts_with("debt  "))
            .count();
        assert!(
            debt > 0,
            "{fixture}: the staged baseline is debt\n{}",
            run.stdout
        );
        assert_parity(&repo, &[], &format!("{fixture} staged baseline"));

        // The staged baseline removed from disk only: still debt.
        fs::remove_file(top.join(".spec-debt.toml")).unwrap();
        repo.staged_check(&[]).code(0);
        repo.plain_check(&[]).code(1);
    }
}

/// AC-04 (c): `--config F` and `--baseline F` are read from disk and named
/// as typed, as plain reads them (2a.1's AC-03, AC-05).
#[test]
fn given_config_and_baseline_are_read_from_disk_as_typed() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::staged("staged-given", fixture);
        let top = &repo.top;
        let config = read_text(top, "specengine.toml");
        let blocked = library(top);

        // `--config` on disk, untracked: its `observe` applies.
        write(top, "alt.toml", with_mode(&config, "observe"));
        let run = repo.staged_check(&["--config", "alt.toml"]);
        run.code(0);
        assert!(
            summary(&run.stdout).starts_with("spec check [observe]: "),
            "{fixture}: {}",
            run.stdout
        );
        assert_parity(
            &repo,
            &["--config", "alt.toml"],
            &format!("{fixture} --config"),
        );

        // An error in it: a cause named as typed.
        let broken = with_mode(&config, "bogus");
        write(top, "alt.toml", &broken);
        let line = line_of(&broken, "mode = ");
        for typed in ["alt.toml", "./sub/../alt.toml"] {
            fs::create_dir_all(top.join("sub")).unwrap();
            let run = repo.staged_check(&["--config", typed]);
            run.code(2);
            let found = causes(&run.stdout);
            assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
            assert!(
                found[0].starts_with(&format!("cannot  {typed}:{line}: ")),
                "{fixture}: {}",
                found[0]
            );
            assert_same(
                &run,
                &repo.plain_check(&["--config", typed]),
                &format!("{fixture} --config {typed}"),
            );
        }

        // `--baseline` on disk, untracked: covering, so clean with debt.
        write(top, "debt.toml", baseline_covering(&blocked, FAR));
        let run = repo.staged_check(&["--baseline", "debt.toml"]);
        run.code(0);
        assert_parity(
            &repo,
            &["--baseline", "debt.toml"],
            &format!("{fixture} --baseline"),
        );
        // It replaces a staged `.spec-debt.toml`.
        write(
            top,
            ".spec-debt.toml",
            "[[debt]]\ncode    = \"ref-dangling\"\npath    = \"docs/none.md\"\nreason  = \"elsewhere\"\nexpires = \"2999-12-31\"\n",
        );
        repo.add_all();
        repo.staged_check(&[]).code(1);
        repo.staged_check(&["--baseline", "debt.toml"]).code(0);

        // A missing or broken `--baseline`: one cause named as typed.
        write(top, "broken.toml", "[[debt]\n");
        for typed in ["nope.toml", "./sub/../broken.toml"] {
            let run = repo.staged_check(&["--baseline", typed]);
            run.code(2);
            let found = causes(&run.stdout);
            assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
            assert!(
                found[0].starts_with(&format!("cannot  {typed}")),
                "{fixture}: {}",
                found[0]
            );
            assert_same(
                &run,
                &repo.plain_check(&["--baseline", typed]),
                &format!("{fixture} --baseline {typed}"),
            );
        }
    }
}

/// AC-04 (d), (e), (f): the config not staged, the config or the baseline
/// staged as a symlink, an error only in the staged config.
#[test]
fn the_staged_config_must_be_a_regular_staged_blob() {
    for (fixture, _) in FIXTURES {
        // (d) Never staged.
        let repo = Repo::of("staged-noconfig", fixture);
        repo.git(&["add", "docs"]);
        let run = repo.staged_check(&[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  specengine.toml: "),
            "{fixture}: {}",
            found[0]
        );
        assert_eq!(run.stderr, "", "{fixture}");

        // (d) `git rm --cached`.
        let repo = Repo::staged("staged-rmconfig", fixture);
        repo.git(&["rm", "-q", "--cached", "specengine.toml"]);
        let run = repo.staged_check(&[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  specengine.toml: "),
            "{fixture}: {}",
            found[0]
        );

        // (e) The config staged as a symlink (discovery follows it on disk).
        let repo = Repo::of("staged-linkconfig", fixture);
        let top = &repo.top;
        fs::rename(top.join("specengine.toml"), top.join("real.toml")).unwrap();
        symlink("real.toml", top.join("specengine.toml")).unwrap();
        repo.add_all();
        repo.plain_check(&[]).code(1);
        let run = repo.staged_check(&[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  specengine.toml: "),
            "{fixture}: {}",
            found[0]
        );

        // (e) The baseline staged as a symlink.
        let repo = Repo::of("staged-linkbaseline", fixture);
        let top = &repo.top;
        let blocked = library(top);
        write(top, "real-debt.toml", baseline_covering(&blocked, FAR));
        symlink("real-debt.toml", top.join(".spec-debt.toml")).unwrap();
        repo.add_all();
        let run = repo.staged_check(&[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
        assert!(
            found[0].starts_with("cannot  .spec-debt.toml: "),
            "{fixture}: {}",
            found[0]
        );

        // (f) An error only in the staged config: a cause at its staged
        // line.
        let repo = Repo::of("staged-badconfig", fixture);
        let top = &repo.top;
        let config = read_text(top, "specengine.toml");
        let broken = with_mode(&config, "bogus");
        write(top, "specengine.toml", &broken);
        repo.add_all();
        write(top, "specengine.toml", &config);
        repo.plain_check(&[]).code(1);
        let run = repo.staged_check(&[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
        let line = line_of(&broken, "mode = ");
        assert!(
            found[0].starts_with(&format!("cannot  specengine.toml:{line}: ")),
            "{fixture}: {}",
            found[0]
        );
    }
}

/// AC-14: documents staged, no commit (`HEAD` unborn): `--staged` = plain.
#[test]
fn an_unborn_head_is_never_read() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::staged("staged-unborn", fixture);
        assert_eq!(repo.git.head(&repo.top), None, "{fixture}: HEAD is unborn");
        repo.staged_check(&[]).code(1);
        assert_parity(&repo, &[], &format!("{fixture} unborn"));
    }
}

/// AC-17: `mode = "enforce-introduced"` staged: exit 2 at its line, as
/// plain reports it.
#[test]
fn enforce_introduced_is_a_cause_at_its_line() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("staged-introduced", fixture);
        let top = &repo.top;
        let text = with_mode(&read_text(top, "specengine.toml"), "enforce-introduced");
        write(top, "specengine.toml", &text);
        repo.add_all();
        let run = repo.staged_check(&[]);
        run.code(2);
        let found = causes(&run.stdout);
        assert_eq!(found.len(), 1, "{fixture}: {}", run.stdout);
        let line = line_of(&text, "mode = ");
        assert!(
            found[0].starts_with(&format!("cannot  specengine.toml:{line}: ")),
            "{fixture}: {}",
            found[0]
        );
        assert_parity(&repo, &[], &format!("{fixture} enforce-introduced"));
    }
}
