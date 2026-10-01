//! docs/features/spec-cli-changed.md through the `spec` binary: AC-01
//! (parity with plain, the base's fields set aside), AC-02 (the git index
//! is never read), AC-03 (introduced, by bytes), AC-04 (paths: a plain
//! move, a deleted cited document, an untracked document, an NFD name),
//! AC-08 (plain's note), AC-10 (flags and failures) and AC-14
//! (determinism). Scratch repositories hold copies of spec-a and spec-b,
//! `HEAD` committed with `--no-verify` as the backlog, the checked changes
//! left unstaged; every git process runs in the sandbox of `common::git`;
//! no git runs in this repository.

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use common::check::{
    FAR, baseline_covering, dangling_document, library, set_paths_key, undefined_id,
};
use common::git::Sandbox;
use common::staged::{GitLog, Repo, assert_same, base_aside, check_args, spec_in};
use common::{FIXTURES, Run, Scratch, read_text, write};

/// `check --changed <args>`, the globals kept before `check`.
fn changed_args<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut all = check_args(false, args);
    let at = all.iter().position(|arg| *arg == "check").expect("check");
    all.insert(at + 1, "--changed");
    all
}

/// `spec check --changed <args>` in the repository's top level.
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

/// A second ID of the fixture's scheme that nothing defines.
fn other_undefined(fixture: &str) -> &'static str {
    if fixture == "spec-a" {
        "R-78"
    } else {
        "REQ-778"
    }
}

/// A spec document whose `refs` list (from line 5) holds `ids`.
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

/// [`dangling_document`] with `tag` appended: distinct bytes, the same one
/// error at line 5.
fn tagged(fixture: &str, tag: &str) -> String {
    format!("{}\n{tag}\n", dangling_document(fixture))
}

/// A copy of `fixture` with `[check] mode = mode`, everything committed:
/// `HEAD` holds the fixture's backlog.
fn backlog(label: &str, fixture: &str, mode: &str) -> Repo {
    let repo = Repo::of(label, fixture);
    let config = read_text(&repo.top, "specengine.toml");
    write(&repo.top, "specengine.toml", with_mode(&config, mode));
    repo.add_all();
    repo.git.commit(&repo.top, "the backlog");
    repo
}

fn commit_all(repo: &Repo, message: &str) {
    repo.add_all();
    repo.git.commit(&repo.top, message);
}

/// `git commit -- <paths>`: the working tree's content of exactly those
/// (known) paths, nothing else staged is recorded.
fn commit_only(repo: &Repo, message: &str, paths: &[&str]) {
    let mut args = vec!["commit", "-q", "--no-verify", "-m", message, "--"];
    args.extend_from_slice(paths);
    repo.git(&args);
}

/// `HEAD`'s blob OID of `path`.
fn head_oid(repo: &Repo, path: &str) -> String {
    repo.git
        .git_text(&repo.top, &["rev-parse", &format!("HEAD:{path}")])
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

/// The `introduced` flags of the findings at `path`.
fn flags_at(run: &Run, path: &str) -> Vec<Option<bool>> {
    introduced(run)
        .into_iter()
        .filter(|(at, ..)| at == path)
        .map(|(.., flag)| flag)
        .collect()
}

/// `--changed` and plain print the same bytes for text, `--debt`,
/// `--json`, `--json --debt` (`extra` added), the `--changed` run's base
/// fields set aside.
fn assert_changed_parity(repo: &Repo, extra: &[&str], context: &str) {
    for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
        let mut args: Vec<&str> = extra.to_vec();
        args.extend_from_slice(form);
        let with_base = changed(repo, &args);
        let plain = repo.plain_check(&args);
        assert_same(&with_base, &plain, &format!("{context} {args:?}"));
    }
}

/// No git text, no absolute path in either stream.
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

/// The `git` paths of `args`' output, NUL-separated, as text.
fn git_paths(repo: &Repo, args: &[&str]) -> Vec<String> {
    repo.git(args)
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// AC-01: parity with plain.
// ---------------------------------------------------------------------------

/// AC-01: under `enforce`, the same mode and baseline on disk and at
/// `HEAD`, the unstaged tree holding a modified document, an untracked
/// one, a `.gitignore`d one, one `git rm --cached` but kept, a
/// dot-directory, an excluded `.md` and a symlinked `.md` under a root:
/// `--changed` prints plain's bytes with the base's fields set aside —
/// text, `--debt`, `--json`, `--json --debt` — blocked, clean (a covering
/// baseline, committed), observed and cannot-check (a written root missing
/// on disk and at `HEAD`). The modified, untracked and ignored documents'
/// errors are introduced; the one removed from the index only, its bytes
/// still `HEAD`'s, is not.
#[test]
fn changed_prints_plain_s_bytes_for_every_verdict() {
    const MODIFIED: &str = "docs/spec/zz-modified.md";
    const UNTRACKED: &str = "docs/spec/zz-untracked.md";
    const IGNORED: &str = "docs/spec/zz-ignored.md";
    const UNCACHED: &str = "docs/spec/zz-uncached.md";
    for (fixture, _) in FIXTURES {
        let repo = Repo::of("chg-parity", fixture);
        let top = &repo.top;
        let config = read_text(top, "specengine.toml");
        let excluding = set_paths_key(&config, "exclude", "[\"docs/spec/excluded-*.md\"]");
        write(top, "specengine.toml", &excluding);
        write(top, ".gitignore", format!("{IGNORED}\n"));
        write(top, MODIFIED, clean_document("Modified"));
        write(top, UNCACHED, tagged(fixture, "Uncached."));
        commit_all(&repo, "the backlog");

        // The unstaged tree.
        write(top, MODIFIED, tagged(fixture, "Modified."));
        write(top, UNTRACKED, tagged(fixture, "Untracked."));
        write(top, IGNORED, tagged(fixture, "Ignored."));
        repo.git(&["rm", "-q", "--cached", "--", UNCACHED]);
        write(top, "docs/spec/.hidden/dot.md", dangling_document(fixture));
        write(top, "docs/spec/excluded-one.md", dangling_document(fixture));
        write(top, "outside/linked-target.md", dangling_document(fixture));
        symlink(
            "../../outside/linked-target.md",
            top.join("docs/spec/link.md"),
        )
        .unwrap();
        // The setup is what it claims.
        assert_eq!(
            git_paths(&repo, &["diff", "--name-only", "-z", "--", MODIFIED]),
            [MODIFIED],
            "{fixture}: modified, unstaged"
        );
        let others = git_paths(&repo, &["ls-files", "-z", "--others", "--exclude-standard"]);
        assert!(others.iter().any(|path| path == UNTRACKED), "{others:?}");
        assert!(others.iter().any(|path| path == UNCACHED), "{others:?}");
        assert!(!others.iter().any(|path| path == IGNORED), "{others:?}");
        let ignored = git_paths(
            &repo,
            &[
                "ls-files",
                "-z",
                "--others",
                "--ignored",
                "--exclude-standard",
            ],
        );
        assert_eq!(ignored, [IGNORED], "{fixture}: ignored");
        assert!(git_paths(&repo, &["ls-files", "-z", "--", UNCACHED]).is_empty());
        head_oid(&repo, UNCACHED);

        // Blocked.
        let blocked = changed(&repo, &[]);
        blocked.code(1);
        assert!(summary(&blocked.stdout).starts_with("spec check [enforce]: "));
        for skipped in ["dot.md", "excluded-one.md", "link.md", "linked-target.md"] {
            assert!(
                !blocked.stdout.contains(skipped),
                "{fixture}: {skipped} is not walked\n{}",
                blocked.stdout
            );
        }
        let json = changed(&repo, &["--json"]);
        for (path, flag) in [
            (MODIFIED, true),
            (UNTRACKED, true),
            (IGNORED, true),
            (UNCACHED, false),
        ] {
            assert_eq!(
                flags_at(&json, path),
                [Some(flag)],
                "{fixture} {path}:\n{}",
                json.stdout
            );
        }
        assert_eq!(json.json()["counts"]["introduced"], 3, "{}", json.stdout);
        assert_changed_parity(&repo, &[], &format!("{fixture} blocked"));

        // Clean: a covering baseline on disk and at HEAD.
        write(
            top,
            ".spec-debt.toml",
            baseline_covering(&library(top), FAR),
        );
        repo.git(&["add", "--", ".spec-debt.toml"]);
        commit_only(&repo, "the baseline", &[".spec-debt.toml"]);
        let clean = changed(&repo, &[]);
        clean.code(0);
        assert!(
            summary(&clean.stdout).ends_with(" — clean"),
            "{}",
            clean.show()
        );
        assert_changed_parity(&repo, &[], &format!("{fixture} clean"));

        // Observed: the baseline gone, `observe`, on disk and at HEAD.
        repo.git(&["rm", "-q", "-f", "--", ".spec-debt.toml"]);
        write(top, "specengine.toml", with_mode(&excluding, "observe"));
        commit_only(&repo, "observe", &["specengine.toml", ".spec-debt.toml"]);
        let observed = changed(&repo, &[]);
        observed.code(0);
        assert!(
            summary(&observed.stdout).ends_with(" — observed"),
            "{}",
            observed.show()
        );
        assert_changed_parity(&repo, &[], &format!("{fixture} observed"));

        // Cannot-check: a written root missing on disk and at HEAD.
        write(
            top,
            "specengine.toml",
            set_paths_key(
                &with_mode(&excluding, "observe"),
                "roots",
                "[\"docs\", \"nowhere\"]",
            ),
        );
        commit_only(&repo, "a missing root", &["specengine.toml"]);
        let cannot = changed(&repo, &[]);
        cannot.code(2);
        assert!(
            summary(&cannot.stdout).ends_with(" — cannot-check"),
            "{}",
            cannot.show()
        );
        assert_changed_parity(&repo, &[], &format!("{fixture} cannot-check"));
        // The index still lacks the uncached document, still holds the
        // modified one's HEAD bytes.
        assert!(git_paths(&repo, &["ls-files", "-z", "--", UNCACHED]).is_empty());
    }
}

// ---------------------------------------------------------------------------
// AC-02: the git index is not read.
// ---------------------------------------------------------------------------

/// AC-02 (a), (b): an error staged then fixed only on disk → `--changed`
/// 0, `--staged` 1; the reverse → `--changed` 1 at the disk line,
/// `--staged` 0.
#[test]
fn the_disk_decides_not_the_index() {
    const PATH: &str = "docs/spec/zz-new.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-index-a", fixture, "enforce-introduced");
        write(&repo.top, PATH, dangling_document(fixture));
        repo.add_all();
        write(&repo.top, PATH, clean_document("Fixed"));
        changed(&repo, &[]).code(0);
        repo.staged_check(&[]).code(1);

        let repo = backlog("chg-index-b", fixture, "enforce-introduced");
        write(&repo.top, PATH, clean_document("Clean"));
        repo.add_all();
        write(&repo.top, PATH, dangling_document(fixture));
        let run = changed(&repo, &[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            [format!(
                "error  {PATH}:5: ref-dangling: `refs`: `{}` resolves to no ID and no alias",
                undefined_id(fixture)
            )],
            "{fixture}:\n{}",
            run.show()
        );
        repo.staged_check(&[]).code(0);
    }
}

/// AC-02 (c): a merge conflict leaves a document unmerged in the index;
/// the disk holds a resolution with an introduced error → `--changed`
/// exit 1 at that file, no unmerged cause (`--staged` refuses, exit 2).
/// The base is `HEAD` (A9).
#[test]
fn an_unmerged_document_is_judged_from_disk() {
    const PATH: &str = "docs/spec/zz-conflict.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-unmerged", fixture, "enforce-introduced");
        write(&repo.top, PATH, clean_document("Base"));
        commit_all(&repo, "a document");
        repo.git(&["checkout", "-q", "-b", "side"]);
        write(&repo.top, PATH, clean_document("Side"));
        commit_all(&repo, "side");
        repo.git(&["checkout", "-q", "main"]);
        write(&repo.top, PATH, clean_document("Main"));
        commit_all(&repo, "main");
        let merge = repo
            .git
            .git_output(&repo.top, &["merge", "--no-edit", "-q", "side"], &[]);
        assert!(!merge.status.success(), "{fixture}: the merge conflicts");
        assert!(
            !repo.git(&["ls-files", "-u", "--", PATH]).is_empty(),
            "{fixture}: unmerged"
        );
        let staged = repo.staged_check(&[]);
        staged.code(2);
        assert!(staged.stdout.contains("unmerged"), "{}", staged.show());

        write(&repo.top, PATH, dangling_document(fixture));
        for form in [&[][..], &["--json"]] {
            let run = changed(&repo, form);
            run.code(1);
            assert!(!run.stdout.contains("unmerged"), "{}", run.show());
            assert_eq!(run.stderr, "", "{fixture}");
            if form.is_empty() {
                assert_eq!(
                    labelled(&run.stdout, "error"),
                    [format!(
                        "error  {PATH}:5: ref-dangling: `refs`: `{}` resolves to no ID and no alias",
                        undefined_id(fixture)
                    )],
                    "{fixture}:\n{}",
                    run.show()
                );
                assert!(labelled(&run.stdout, "cannot").is_empty());
            } else {
                assert_eq!(flags_at(&run, PATH), [Some(true)], "{}", run.stdout);
            }
        }
        // A resolution keeping `HEAD`'s bytes: nothing introduced.
        write(&repo.top, PATH, clean_document("Main"));
        changed(&repo, &[]).code(0);
    }
}

/// AC-02 (d): `HEAD` and the index `observe`, the disk `enforce` →
/// `[enforce]`, exit 1 (`--staged`: `[observe]`, exit 0).
#[test]
fn the_config_is_read_from_disk() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-index-d", fixture, "observe");
        let config = read_text(&common::fixture(fixture), "specengine.toml");
        write(&repo.top, "specengine.toml", with_mode(&config, "enforce"));
        let run = changed(&repo, &[]);
        run.code(1);
        assert!(
            summary(&run.stdout).starts_with("spec check [enforce]: "),
            "{fixture}:\n{}",
            run.show()
        );
        assert_eq!(run.stderr, "", "{fixture}: no note");
        let staged = repo.staged_check(&[]);
        staged.code(0);
        assert!(summary(&staged.stdout).starts_with("spec check [observe]: "));
    }
}

/// AC-02 (e): under `enforce`, a baseline at `HEAD` and in the index
/// covering every error, deleted on disk → the errors block, exit 1
/// (`--staged`: clean, exit 0).
#[test]
fn the_baseline_is_read_from_disk() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-index-e", fixture, "enforce");
        write(
            &repo.top,
            ".spec-debt.toml",
            baseline_covering(&library(&repo.top), FAR),
        );
        commit_all(&repo, "a covering baseline");
        changed(&repo, &[]).code(0);
        fs::remove_file(repo.top.join(".spec-debt.toml")).unwrap();
        let run = changed(&repo, &[]);
        run.code(1);
        assert!(
            summary(&run.stdout).ends_with(" — blocked")
                && summary(&run.stdout).contains(", 0 debt, "),
            "{fixture}:\n{}",
            run.show()
        );
        assert!(!labelled(&run.stdout, "error").is_empty());
        repo.staged_check(&[]).code(0);
    }
}

// ---------------------------------------------------------------------------
// AC-03: introduced, by bytes.
// ---------------------------------------------------------------------------

/// AC-03: `HEAD` holds E1 in `zz-a.md`; on disk E1 on another line, E1's
/// key again and a new E2 → exit 1, one `error` line (E2); `introduced`
/// true on E2 only, `counts.introduced` 1. Disk == `HEAD` with the
/// backlog → exit 0, observed, 0 introduced.
#[test]
fn introduced_is_decided_by_bytes() {
    const PATH: &str = "docs/spec/zz-a.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-key", fixture, "enforce-introduced");
        let (e1, e2) = (undefined_id(fixture), other_undefined(fixture));
        write(&repo.top, PATH, refs_document(&[e1]));
        commit_all(&repo, "E1");

        let run = changed(&repo, &[]);
        run.code(0);
        assert!(
            summary(&run.stdout).ends_with(" — observed")
                && summary(&run.stdout).contains(", 0 introduced, "),
            "{fixture}:\n{}",
            run.show()
        );
        let json = changed(&repo, &["--json"]);
        json.code(0);
        assert_eq!(json.json()["counts"]["introduced"], 0);
        assert!(
            introduced(&json)
                .iter()
                .all(|(.., flag)| *flag == Some(false))
        );

        let disk = refs_document(&[e1, e2, e1]).replacen("refs:\n", "refs:\n  # moved\n", 1);
        write(&repo.top, PATH, &disk);
        let run = changed(&repo, &[]);
        run.code(1);
        let errors = labelled(&run.stdout, "error");
        assert_eq!(errors.len(), 1, "{fixture}:\n{}", run.show());
        assert!(
            errors[0].starts_with(&format!("error  {PATH}:8: ref-dangling: ")),
            "{fixture}: {}",
            errors[0]
        );
        let json = changed(&repo, &["--json"]);
        json.code(1);
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
        // Nothing staged: the index still holds HEAD's bytes.
        assert!(repo.git(&["diff", "--cached", "--name-only"]).is_empty());
    }
}

// ---------------------------------------------------------------------------
// AC-04: paths.
// ---------------------------------------------------------------------------

/// AC-04: a plain `mv` of a document with a pre-existing error → exit 1 at
/// the new path; a document deleted on disk whose ID an unchanged
/// document cites → the dangling citation introduced, exit 1; an
/// untracked document's errors introduced; an NFD-named file replacing
/// `HEAD`'s precomposed one → introduced.
#[test]
fn moved_deleted_untracked_and_nfd_paths_are_introduced() {
    for (fixture, _) in FIXTURES {
        // A plain `mv`, the same bytes at a new path.
        let repo = backlog("chg-mv", fixture, "enforce-introduced");
        write(&repo.top, "docs/spec/zz-old.md", dangling_document(fixture));
        commit_all(&repo, "a pre-existing error");
        changed(&repo, &[]).code(0);
        fs::rename(
            repo.top.join("docs/spec/zz-old.md"),
            repo.top.join("docs/spec/zz-new.md"),
        )
        .unwrap();
        let run = changed(&repo, &[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            [format!(
                "error  docs/spec/zz-new.md:5: ref-dangling: `refs`: `{}` resolves to no ID and no alias",
                undefined_id(fixture)
            )],
            "{fixture}: mv\n{}",
            run.show()
        );

        // A deleted document whose ID an unchanged document cites.
        let repo = backlog("chg-deleted", fixture, "enforce-introduced");
        const DEFINES: &str = "docs/spec/zz-def.md";
        const CITES: &str = "docs/spec/zz-cite.md";
        let id = if fixture == "spec-a" {
            "RULE-ZZ-DEF"
        } else {
            "CMD-ZZ-DEF"
        };
        write(
            &repo.top,
            DEFINES,
            format!(
                "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Defined\n\n## Rule {{#{id}}}\n\nText.\n"
            ),
        );
        write(
            &repo.top,
            CITES,
            format!(
                "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\nrefs: [{id}]\n---\n\n# Cites\n\nText.\n"
            ),
        );
        commit_all(&repo, "a definition and a citation");
        changed(&repo, &[]).code(0);
        fs::remove_file(repo.top.join(DEFINES)).unwrap();
        let run = changed(&repo, &[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            [format!(
                "error  {CITES}:5: ref-dangling: `refs`: `{id}` resolves to no ID and no alias"
            )],
            "{fixture}: deleted\n{}",
            run.show()
        );
        let json = changed(&repo, &["--json"]);
        assert_eq!(flags_at(&json, CITES), [Some(true)], "{}", json.stdout);

        // An untracked document.
        let repo = backlog("chg-untracked", fixture, "enforce-introduced");
        write(
            &repo.top,
            "docs/spec/zz-untracked.md",
            refs_document(&[undefined_id(fixture), other_undefined(fixture)]),
        );
        let json = changed(&repo, &["--json"]);
        json.code(1);
        assert_eq!(
            flags_at(&json, "docs/spec/zz-untracked.md"),
            [Some(true), Some(true)],
            "{fixture}: untracked\n{}",
            json.stdout
        );
        assert_eq!(json.json()["counts"]["introduced"], 2);

        // An NFD name on disk over HEAD's precomposed one.
        let repo = backlog("chg-nfd", fixture, "enforce-introduced");
        let nfc = "docs/spec/caf\u{e9}.md";
        let nfd = "docs/spec/cafe\u{301}.md";
        write(&repo.top, nfc, dangling_document(fixture));
        commit_all(&repo, "a precomposed name");
        changed(&repo, &[]).code(0);
        fs::remove_file(repo.top.join(nfc)).unwrap();
        write(&repo.top, nfd, dangling_document(fixture));
        let names: Vec<Vec<u8>> = fs::read_dir(repo.top.join("docs/spec"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_encoded_bytes())
            .collect();
        assert!(
            names.contains(&"cafe\u{301}.md".as_bytes().to_vec()),
            "{fixture}: the disk holds the NFD bytes"
        );
        let run = changed(&repo, &["--json"]);
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
            "{fixture}: nfd\n{}",
            run.stdout
        );
    }
}

// ---------------------------------------------------------------------------
// AC-08: plain's note.
// ---------------------------------------------------------------------------

/// AC-08: plain under `enforce-introduced` → `[enforce]`, exit 1, one note
/// naming both flags; `--changed` → `[enforce-introduced]`, observed, no
/// note at all.
#[test]
fn plain_s_note_names_both_flags_and_changed_has_none() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-note", fixture, "enforce-introduced");
        let plain = repo.plain_check(&[]);
        plain.code(1);
        assert!(summary(&plain.stdout).starts_with("spec check [enforce]: "));
        assert_eq!(
            notes(&plain),
            [
                "note: mode `enforce-introduced` has no base without --staged or --changed: judged as `enforce`"
            ],
            "{fixture}"
        );
        for form in [&[][..], &["--debt"], &["--json"]] {
            let run = changed(&repo, form);
            run.code(0);
            assert_eq!(run.stderr, "", "{fixture} {form:?}: no note");
            if form.contains(&"--json") {
                assert_eq!(run.json()["mode"], "enforce-introduced");
            } else {
                assert!(
                    summary(&run.stdout).starts_with("spec check [enforce-introduced]: ")
                        && summary(&run.stdout).ends_with(" — observed"),
                    "{}",
                    run.show()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-10: flags and failures.
// ---------------------------------------------------------------------------

/// AC-10: `--changed --staged`, in either order → a usage error, exit 2,
/// empty stdout.
#[test]
fn changed_and_staged_conflict() {
    let repo = backlog("chg-usage", "spec-a", "enforce-introduced");
    for args in [
        &["check", "--changed", "--staged"][..],
        &["check", "--staged", "--changed"],
        &["--json", "check", "--staged", "--changed", "--debt"],
    ] {
        let run = repo.spec(args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        assert!(
            run.stderr.contains("cannot be used with")
                && run.stderr.contains("--changed")
                && run.stderr.contains("--staged"),
            "{args:?}:\n{}",
            run.show()
        );
    }
}

/// One failure case: its name, the sandbox and directory `spec` runs
/// with, the variables added.
type Case<'a> = (
    &'a str,
    &'a Sandbox,
    &'a Path,
    Vec<(&'a str, &'a std::ffi::OsStr)>,
);

/// AC-10: no repository, or no `git` on `PATH` → exit 2, one cause at `.`
/// in the disk config's mode, no git text, no absolute path, no note;
/// never plain's report.
#[test]
fn no_repository_or_no_git_is_one_cause() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("chg-norepo");
        let git = Sandbox::new(scratch.path());
        let top = scratch.copy(fixture, "copy");
        let config = read_text(&top, "specengine.toml");
        write(
            &top,
            "specengine.toml",
            with_mode(&config, "enforce-introduced"),
        );
        let repo = backlog("chg-nogit", fixture, "enforce-introduced");
        let nowhere = repo.scratch.dir("no-git-here");
        let cases: [Case; 2] = [
            ("no repository", &git, &top, Vec::new()),
            (
                "no git",
                &repo.git,
                &repo.top,
                vec![("PATH", nowhere.as_os_str())],
            ),
        ];
        for (case, sandbox, cwd, extra) in cases {
            for form in [&[][..], &["--json"]] {
                let run = spec_in(sandbox, cwd, &changed_args(form), &extra);
                let context = format!("{fixture} {case} {form:?}");
                run.code(2);
                assert_eq!(run.stderr, "", "{context}: no note");
                assert_no_git_text_or_absolute_path(&run, sandbox.parent(), &context);
                if form.is_empty() {
                    let causes = labelled(&run.stdout, "cannot");
                    assert_eq!(causes.len(), 1, "{context}:\n{}", run.show());
                    assert!(causes[0].starts_with("cannot  .: "), "{context}");
                    assert_eq!(run.stdout.lines().count(), 2, "{context}:\n{}", run.show());
                    assert!(
                        summary(&run.stdout).starts_with("spec check [enforce-introduced]: ")
                            && summary(&run.stdout).ends_with(" — cannot-check"),
                        "{context}:\n{}",
                        run.show()
                    );
                } else {
                    let json = run.json();
                    assert_eq!(json["mode"], "enforce-introduced");
                    assert_eq!(json["cannot_check"].as_array().map(Vec::len), Some(1));
                    assert_eq!(json["cannot_check"][0]["path"], ".");
                    assert_eq!(json["findings"].as_array().map(Vec::len), Some(0));
                }
            }
        }
    }
}

/// AC-10 (A5): an invalid config or baseline on disk → plain's causes and
/// output, and no git runs (a logging wrapper first on `PATH` logs
/// nothing).
#[test]
fn an_invalid_disk_config_is_plain_s_causes_without_git() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("chg-invalid", fixture, "enforce-introduced");
        let config = read_text(&repo.top, "specengine.toml");
        let log = GitLog::new(&repo.scratch, &repo.git);
        for (case, file, text) in [
            (
                "invalid TOML",
                "specengine.toml",
                format!("{config}\n[check\n"),
            ),
            (
                "an invalid mode",
                "specengine.toml",
                config.replace("enforce-introduced", "bogus"),
            ),
            (
                "an invalid baseline",
                ".spec-debt.toml",
                "[[debt]\n".to_owned(),
            ),
        ] {
            write(&repo.top, file, &text);
            for form in [&[][..], &["--json"], &["--debt"]] {
                log.clear();
                let run = spec_in(&repo.git, &repo.top, &changed_args(form), &log.env());
                let plain = repo.plain_check(form);
                let context = format!("{fixture} {case} {form:?}");
                run.code(2);
                plain.code(2);
                // Plain's causes; only plain's `enforce-introduced`
                // fallback may rename the mode.
                let aside = base_aside(&run).stdout;
                let lines: Vec<&str> = aside.lines().collect();
                let plain_lines: Vec<&str> = plain.stdout.lines().collect();
                assert_eq!(lines.len(), plain_lines.len(), "{context}\n{}", run.show());
                for (ours, theirs) in lines.iter().zip(&plain_lines) {
                    assert_eq!(
                        ours.replacen("[enforce-introduced]", "[enforce]", 1)
                            .replacen("\"mode\":\"enforce-introduced\"", "\"mode\":\"enforce\"", 1),
                        *theirs,
                        "{context}"
                    );
                }
                assert!(!labelled(&run.stdout, "cannot").is_empty() || form.contains(&"--json"));
                assert!(
                    labelled(&run.stdout, "cannot")
                        .iter()
                        .all(|cause| !cause.starts_with("cannot  .: ")),
                    "{context}: no git cause\n{}",
                    run.show()
                );
                assert_eq!(run.stderr, "", "{context}: no note");
                assert!(
                    log.calls().is_empty(),
                    "{context}: git ran before the config was valid: {:?}",
                    log.calls()
                );
            }
            if file == "specengine.toml" {
                write(&repo.top, file, &config);
            } else {
                fs::remove_file(repo.top.join(file)).unwrap();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-14: determinism.
// ---------------------------------------------------------------------------

/// A repository at `<scratch>/<dir>/repo`: `fixture` in `enforce`, its
/// files added one by one (path order or reversed) and committed, then on
/// disk only: `enforce-introduced` (`HEAD`'s stricter mode: a note), an
/// introduced error, a changed document and a baseline with an entry
/// `HEAD` lacks, written in that order or reversed.
fn twin(scratch: &Scratch, git: &Sandbox, fixture: &str, dir: &str, reverse: bool) -> PathBuf {
    let top = scratch.copy(fixture, &format!("{dir}/repo"));
    let config = read_text(&top, "specengine.toml");
    write(&top, "specengine.toml", with_mode(&config, "enforce"));
    write(&top, "docs/spec/zz-doc.md", tagged(fixture, "Before."));
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
            format!(
                "[[debt]]\ncode    = \"budget\"\npath    = \"docs/none.md\"\nsubject = \"\"\nreason  = \"new\"\nexpires = \"{FAR}\"\n"
            ),
        ),
        ("docs/spec/zz-clean.md", clean_document("Clean")),
    ];
    if reverse {
        later.reverse();
    }
    for (path, text) in later {
        write(&top, path, text);
    }
    top
}

/// AC-14: two repositories at different absolute paths, their files
/// created and added in opposite orders → identical stdout and stderr in
/// every output form: blocked (with `HEAD`'s stricter-mode note); a
/// partial base merged with the checked run's cause (a missing `HEAD`
/// blob, an unreadable file); a git failure (`HEAD`'s tree deleted).
#[test]
fn the_report_is_the_same_across_repositories() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("chg-determinism");
        let git = Sandbox::new(scratch.path());
        let one = twin(&scratch, &git, fixture, "a", false);
        let two = twin(
            &scratch,
            &git,
            fixture,
            "a-much-longer-directory-name",
            true,
        );
        let compare = |stage: &str, code: i32| {
            for form in [&[][..], &["--debt"], &["--json"], &["--json", "--debt"]] {
                let args = changed_args(form);
                let a = spec_in(&git, &one, &args, &[]);
                let b = spec_in(&git, &two, &args, &[]);
                assert_eq!(a.code, code, "{fixture} {stage} {form:?}\n{}", a.show());
                assert!(
                    a.code == b.code && a.stdout == b.stdout && a.stderr == b.stderr,
                    "{fixture} {stage} {form:?}:\n{}\n{}",
                    a.show(),
                    b.show()
                );
                assert_no_git_text_or_absolute_path(&a, scratch.path(), stage);
            }
        };
        // Blocked: one introduced error, one new debt entry, HEAD's note.
        compare("blocked", 1);
        let a = spec_in(&git, &one, &changed_args(&[]), &[]);
        assert_eq!(
            notes(&a),
            [
                "note: HEAD's specengine.toml sets mode `enforce`, stricter than `enforce-introduced`: the check runs in `enforce`"
            ],
            "{}",
            a.show()
        );
        assert!(
            summary(&a.stdout).contains(", 1 introduced, 1 new debt, worst W "),
            "{}",
            a.show()
        );

        // A partial base and the checked run's cause.
        for top in [&one, &two] {
            write(top, "docs/spec/zz-doc.md", tagged(fixture, "After."));
            let oid = git.git_text(top, &["rev-parse", "HEAD:docs/spec/zz-doc.md"]);
            fs::remove_file(git.loose_object(top, &oid)).unwrap();
            let locked = top.join("docs/spec/zz-locked.md");
            fs::write(&locked, dangling_document(fixture)).unwrap();
            fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o000))
                .unwrap();
        }
        compare("partial", 2);
        let a = spec_in(&git, &one, &changed_args(&[]), &[]);
        assert_eq!(
            labelled(&a.stdout, "cannot"),
            [
                "cannot  docs/spec/zz-doc.md: HEAD's blob is missing from the git object database",
                "cannot  docs/spec/zz-locked.md: cannot read: Permission denied (os error 13)",
            ],
            "{}",
            a.show()
        );
        for top in [&one, &two] {
            fs::remove_file(top.join("docs/spec/zz-locked.md")).unwrap();
        }

        // A git failure: HEAD's tree deleted.
        for top in [&one, &two] {
            let tree = git.git_text(top, &["rev-parse", "HEAD^{tree}"]);
            fs::remove_file(git.loose_object(top, &tree)).unwrap();
        }
        compare("ls-tree", 2);
    }
}
