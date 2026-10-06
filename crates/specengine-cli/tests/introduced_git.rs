//! docs/features/spec-cli-introduced.md against the git side: AC-07 (the
//! base's failures: a written root absent at `HEAD`; a partial base — a
//! missing `HEAD` blob, a `HEAD` entry naming a tree or a commit, alone,
//! shared with the index, merged with the checked run's causes, at a moved
//! path; `HEAD`-only paths sharing one blob; a missing `HEAD` tree),
//! AC-12 (the git commands through a
//! logging wrapper: five subcommands, `ls-tree -r -z <oid>` once in the
//! root iff `HEAD` is born, one `cat-file --batch` session, each OID
//! requested once, none for an unchanged path's `HEAD` blob) and AC-13
//! (2a.2's AC-07, AC-08, AC-12 with errors at `HEAD`: a linked worktree's
//! own `HEAD`, `GIT_DIR` with `GIT_WORK_TREE`, the guard, nothing written,
//! TAB/LF names, SHA-256). Every git process runs in the sandbox of
//! `common::git`, in scratch repositories only.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use common::check::{dangling_document, set_paths_key, undefined_id};
use common::git::Sandbox;
use common::staged::{GitLog, Repo, check_args, spec_in};
use common::{FIXTURES, Run, Scratch, copy_dir, read_text, snapshot, write};

/// The guard's message, at `.` (2a.2 AC-07).
const GUARD_MESSAGE: &str =
    "GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top";

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

/// A copy of `fixture` in `enforce-introduced`, committed: the backlog.
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

/// No git text, no absolute path in either stream (2a.2 AC-10).
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

// ---------------------------------------------------------------------------
// AC-07: the base's failures.
// ---------------------------------------------------------------------------

/// AC-07 (a): a written root absent at `HEAD` gives no cause: its files'
/// findings are introduced (exit 1 on its error only, stderr empty).
#[test]
fn a_root_absent_at_head_is_no_cause_and_its_findings_are_introduced() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-root-absent", fixture);
        let config = read_text(&repo.top, "specengine.toml");
        write(
            &repo.top,
            "specengine.toml",
            set_paths_key(&config, "roots", "[\"docs\", \"more\"]"),
        );
        write(&repo.top, "more/zz-more.md", dangling_document(fixture));
        repo.add_all();
        let run = repo.staged_check(&[]);
        run.code(1);
        assert!(
            labelled(&run.stdout, "cannot").is_empty(),
            "{fixture}:\n{}",
            run.show()
        );
        assert_eq!(
            labelled(&run.stdout, "error"),
            [format!(
                "error  more/zz-more.md:5: ref-dangling: `refs`: `{}` resolves to no ID and no alias",
                undefined_id(fixture)
            )],
            "{fixture}:\n{}",
            run.show()
        );
        assert_eq!(run.stderr, "", "{fixture}");
        assert!(
            summary(&run.stdout).contains(", 1 introduced, "),
            "{}",
            run.show()
        );
    }
}

/// The cause at a document's path when `HEAD` lists it with a blob not in
/// the object database (AC-07 (a'), the fixed message).
const MISSING_HEAD_BLOB: &str = "HEAD's blob is missing from the git object database";

/// The cause at a document's path when `HEAD` lists it as a regular file
/// whose OID names a tree or a commit (the orchestrator's ruling).
const HEAD_NOT_A_BLOB: &str = "HEAD's object is not a blob";

/// The checked run's own cause (2a.2) for a staged blob not in the object
/// database.
const STAGED_MISSING: &str = "cannot read: the staged blob is missing from the git object database";

/// The checked run's own cause (2a.2) for a staged regular entry whose OID
/// names another object type.
const STAGED_NOT_A_BLOB: &str = "cannot read: the staged object is not a blob";

/// A spec document whose one error is a front-matter reference to `id`
/// (at line 5).
fn citing(id: &str) -> String {
    format!(
        "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\nrefs: [{id}]\n---\n\n# Dangling\n\nText.\n"
    )
}

/// A second ID of the fixture's scheme that nothing defines.
fn other_undefined(fixture: &str) -> &'static str {
    if fixture == "spec-a" {
        "R-78"
    } else {
        "REQ-778"
    }
}

/// `spec check --staged` in text and in JSON is cannot-check (exit 2) in
/// `enforce-introduced` with exactly the `causes` (path, message; sorted),
/// no note, no git text, no absolute path. `partial` (the base failed
/// closed): no finding, `stale` or `new` line either; else (only the
/// checked run's causes, 2a.2) a judged report, nothing introduced.
fn assert_causes(repo: &Repo, causes: &[(&str, &str)], partial: bool, context: &str) {
    let text = repo.staged_check(&[]);
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
    if partial {
        assert_eq!(
            text.stdout.lines().count(),
            causes.len() + 1,
            "{context}: only the causes and the summary\n{}",
            text.show()
        );
    }
    assert!(
        summary(&text.stdout).starts_with("spec check [enforce-introduced]: ")
            && summary(&text.stdout).ends_with(" — cannot-check"),
        "{context}:\n{}",
        text.show()
    );
    assert_eq!(text.stderr, "", "{context}: no note");
    assert_no_git_text_or_absolute_path(&text, repo.scratch.path(), context);
    let json = repo.staged_check(&["--json"]);
    json.code(2);
    assert_eq!(json.stderr, "", "{context}: no note");
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
    let findings = value["findings"].as_array().expect("findings");
    if partial {
        assert!(findings.is_empty(), "{context}: {findings:?}");
    } else {
        assert_eq!(value["counts"]["introduced"], 0, "{context}");
        assert!(
            findings
                .iter()
                .all(|finding| finding["introduced"] == false),
            "{context}: {findings:?}"
        );
    }
    assert_eq!(value["mode"], "enforce-introduced", "{context}");
}

/// AC-07 (a'), as the spec now words it (the review of iteration 1): the
/// `HEAD` blob of a listed document missing from the object database is
/// cannot-check at that path — exit 2, exactly one cause, at the
/// document's path, the fixed message, no finding, no note, no git text,
/// no absolute path — never a silent "introduced" (which could pass
/// dependent findings as pre-existing).
#[test]
fn a_missing_head_blob_of_a_document_is_a_cause_at_its_path() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-blob-gone", fixture);
        const PATH: &str = "docs/spec/zz-doc.md";
        write(&repo.top, PATH, dangling_document(fixture));
        repo.add_all();
        repo.git.commit(&repo.top, "a document");
        let head_oid = repo.git.staged_oid(&repo.top, PATH);
        write(
            &repo.top,
            PATH,
            format!("{}\nA changed line.\n", dangling_document(fixture)),
        );
        repo.add_all();
        assert_ne!(repo.git.staged_oid(&repo.top, PATH), head_oid);
        fs::remove_file(repo.git.loose_object(&repo.top, &head_oid)).unwrap();
        assert_causes(&repo, &[(PATH, MISSING_HEAD_BLOB)], true, fixture);
    }
}

/// AC-07 (a'), the spec's own case: `HEAD`'s `a.md` defines an ID that
/// `b.md` cites; staged, `a.md` drops it, so `b.md`'s `ref-dangling` is
/// introduced (the control: exit 1, that one error). With `a.md`'s `HEAD`
/// blob deleted the base is partial: exit 2, one cause at `a.md`, the
/// fixed message — never exit 0 with `b.md`'s error read as pre-existing
/// (the old rule: the missing blob read as an unreadable file).
#[test]
fn a_missing_head_blob_never_passes_a_dependent_finding_as_pre_existing() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-blob-dependent", fixture);
        const DEFINES: &str = "docs/spec/zz-def.md";
        const CITES: &str = "docs/spec/zz-cite.md";
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
        write(&repo.top, CITES, citing(id));
        repo.add_all();
        repo.git.commit(&repo.top, "a definition and a citation");
        let head_oid = repo.git.staged_oid(&repo.top, DEFINES);
        write(&repo.top, DEFINES, defines(""));
        repo.add_all();
        let control = repo.staged_check(&[]);
        control.code(1);
        assert_eq!(
            labelled(&control.stdout, "error"),
            [format!(
                "error  {CITES}:5: ref-dangling: `refs`: `{id}` resolves to no ID and no alias"
            )],
            "{fixture}: the control\n{}",
            control.show()
        );
        fs::remove_file(repo.git.loose_object(&repo.top, &head_oid)).unwrap();
        assert_causes(&repo, &[(DEFINES, MISSING_HEAD_BLOB)], true, fixture);
    }
}

/// The orchestrator's ruling (iteration 2): a document `HEAD` lists as a
/// regular file (mode 100644) whose OID names a tree or a commit, the
/// staged side changing that path: the base is partial — exit 2, exactly
/// one cause at that path, `HEAD's object is not a blob`, no finding, no
/// note, no git text, no absolute path.
#[test]
fn a_head_entry_naming_a_tree_or_a_commit_is_a_cause_at_its_path() {
    const PATH: &str = "docs/spec/zz-foreign.md";
    for (fixture, _) in FIXTURES {
        for kind in ["tree", "commit"] {
            let context = format!("{fixture} {kind}");
            let repo = backlog("intro-foreign", fixture);
            let oid = repo
                .git
                .git_text(&repo.top, &["rev-parse", &format!("HEAD^{{{kind}}}")]);
            assert_eq!(
                repo.git.git_text(&repo.top, &["cat-file", "-t", &oid]),
                kind
            );
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
            write(&repo.top, PATH, dangling_document(fixture));
            repo.git(&["add", "--", PATH]);
            assert_ne!(repo.git.staged_oid(&repo.top, PATH), oid, "{context}");
            assert_causes(&repo, &[(PATH, HEAD_NOT_A_BLOB)], true, &context);
        }
    }
}

/// The shared case: the same OID staged at the same path as `HEAD`'s — a
/// tree or a commit under mode 100644, or a blob missing from the object
/// database — is one cause, the checked run's own (the staged side's
/// message), never a second one from the base; the base is not partial
/// (that path reuses the checked file), so the report is judged (2a.2's
/// cannot-check with its findings, none introduced).
#[test]
fn a_bad_head_object_shared_with_the_index_is_the_checked_run_s_cause_only() {
    const PATH: &str = "docs/spec/zz-shared.md";
    for (fixture, _) in FIXTURES {
        for kind in ["tree", "commit"] {
            let context = format!("{fixture} {kind}");
            let repo = backlog("intro-foreign-shared", fixture);
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
            assert_eq!(repo.git.staged_oid(&repo.top, PATH), oid, "{context}");
            assert_causes(&repo, &[(PATH, STAGED_NOT_A_BLOB)], false, &context);
        }
        let context = format!("{fixture} missing");
        let repo = backlog("intro-missing-shared", fixture);
        write(&repo.top, PATH, dangling_document(fixture));
        repo.add_all();
        repo.git.commit(&repo.top, "a document");
        let oid = repo.git.staged_oid(&repo.top, PATH);
        fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
        assert_causes(&repo, &[(PATH, STAGED_MISSING)], false, &context);
    }
}

/// A partial base merges the checked run's causes: a changed document's
/// `HEAD` blob deleted plus a staged entry whose blob is missing → both
/// causes, each at its path, exit 2, no finding, no note.
#[test]
fn a_partial_base_and_a_missing_staged_blob_give_both_causes() {
    const PATH: &str = "docs/spec/zz-doc.md";
    const GHOST: &str = "docs/spec/zz-ghost.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-merged-causes", fixture);
        write(&repo.top, PATH, dangling_document(fixture));
        repo.add_all();
        repo.git.commit(&repo.top, "a document");
        let head_oid = repo.git.staged_oid(&repo.top, PATH);
        write(
            &repo.top,
            PATH,
            format!("{}\nA changed line.\n", dangling_document(fixture)),
        );
        repo.add_all();
        fs::remove_file(repo.git.loose_object(&repo.top, &head_oid)).unwrap();
        let ghost = "1".repeat(head_oid.len());
        repo.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{ghost},{GHOST}"),
        ]);
        assert_causes(
            &repo,
            &[(PATH, MISSING_HEAD_BLOB), (GHOST, STAGED_MISSING)],
            true,
            fixture,
        );
    }
}

/// A `HEAD`-only path whose OID the index holds at another path (a
/// `git mv`), the object missing: the checked run's cause at the new path
/// and the base's at `HEAD`'s path — never the moved file's checked
/// result reused for `HEAD`'s path by OID alone.
#[test]
fn a_head_only_path_whose_missing_blob_the_index_holds_elsewhere_is_a_cause_there() {
    const OLD: &str = "docs/spec/zz-old.md";
    const NEW: &str = "docs/spec/zz-new.md";
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-moved-missing", fixture);
        write(&repo.top, OLD, dangling_document(fixture));
        repo.add_all();
        repo.git.commit(&repo.top, "a document");
        let oid = repo.git.staged_oid(&repo.top, OLD);
        repo.git(&["mv", OLD, NEW]);
        assert_eq!(repo.git.staged_oid(&repo.top, NEW), oid);
        fs::remove_file(repo.git.loose_object(&repo.top, &oid)).unwrap();
        assert_causes(
            &repo,
            &[(NEW, STAGED_MISSING), (OLD, MISSING_HEAD_BLOB)],
            true,
            fixture,
        );
    }
}

/// `HEAD`-only paths sharing one blob each get that blob's bytes (copied
/// before the OID's last use, moved at it): `HEAD`'s `zz-1.md` and
/// `zz-3.md` hold one blob, `zz-2.md` between them (in path order)
/// another, citing another undefined ID; each staged with an edit keeping
/// its error → every one pre-existing (exit 0, `introduced: false`, listed
/// with `--debt` ending ` (pre-existing)`); a new `zz-4.md` with the same
/// error is the one introduced (exit 1).
#[test]
fn head_only_paths_sharing_one_blob_each_get_its_bytes() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-shared-bytes", fixture);
        let (first, second) = (undefined_id(fixture), other_undefined(fixture));
        let files = [
            ("docs/spec/zz-1.md", first),
            ("docs/spec/zz-2.md", second),
            ("docs/spec/zz-3.md", first),
        ];
        for (path, id) in files {
            write(&repo.top, path, citing(id));
        }
        repo.add_all();
        repo.git.commit(&repo.top, "three documents, two blobs");
        let oids: Vec<String> = files
            .iter()
            .map(|(path, _)| repo.git.staged_oid(&repo.top, path))
            .collect();
        assert_eq!(oids[0], oids[2], "{fixture}: one blob");
        assert_ne!(oids[0], oids[1], "{fixture}: another blob");
        for (n, (path, id)) in files.iter().enumerate() {
            write(
                &repo.top,
                path,
                format!("{}\nEdited {}.\n", citing(id), n + 1),
            );
        }
        repo.add_all();
        let error = |path: &str, id: &str| {
            format!("error  {path}:5: ref-dangling: `refs`: `{id}` resolves to no ID and no alias")
        };
        let run = repo.staged_check(&[]);
        run.code(0);
        assert!(
            summary(&run.stdout).contains(", 0 introduced, "),
            "{fixture}:\n{}",
            run.show()
        );
        let debt = repo.staged_check(&["--debt"]);
        debt.code(0);
        let listed: Vec<&str> = labelled(&debt.stdout, "error")
            .into_iter()
            .filter(|line| line.contains("/zz-"))
            .collect();
        let expected: Vec<String> = files
            .iter()
            .map(|(path, id)| format!("{} (pre-existing)", error(path, id)))
            .collect();
        assert_eq!(listed, expected, "{fixture}:\n{}", debt.show());
        let json = repo.staged_check(&["--json"]).json();
        for (path, id) in files {
            let mine: Vec<&serde_json::Value> = json["findings"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|finding| finding["path"] == path)
                .collect();
            assert_eq!(mine.len(), 1, "{fixture} {path}: {mine:?}");
            assert_eq!(mine[0]["code"], "ref-dangling", "{fixture} {path}");
            assert!(
                mine[0]["message"].as_str().unwrap().contains(id),
                "{fixture} {path}: {}",
                mine[0]["message"]
            );
            assert_eq!(mine[0]["introduced"], false, "{fixture} {path}");
        }
        write(&repo.top, "docs/spec/zz-4.md", citing(first));
        repo.add_all();
        let run = repo.staged_check(&[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error"),
            [error("docs/spec/zz-4.md", first)],
            "{fixture}:\n{}",
            run.show()
        );
    }
}

/// AC-07 (b): `HEAD`'s tree object deleted: exit 2, one fixed cause at `.`
/// naming `git ls-tree`, in the checked config's mode; no git text, no
/// absolute path; never an empty base.
#[test]
fn a_missing_head_tree_is_one_fixed_cause() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-tree-gone", fixture);
        let tree = repo.git.git_text(&repo.top, &["rev-parse", "HEAD^{tree}"]);
        fs::remove_file(repo.git.loose_object(&repo.top, &tree)).unwrap();
        for form in [&[][..], &["--json"]] {
            let run = repo.staged_check(form);
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
                assert!(labelled(&run.stdout, "error").is_empty(), "{}", run.show());
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
// AC-12: the git commands.
// ---------------------------------------------------------------------------

/// The stage-0 entries of the index: path → OID.
fn index_oids(repo: &Repo) -> BTreeMap<String, String> {
    let text = String::from_utf8(repo.git(&["ls-files", "-s"])).unwrap();
    text.lines()
        .map(|line| {
            let (meta, path) = line.split_once('\t').unwrap();
            let oid = meta.split_whitespace().nth(1).unwrap();
            (path.to_owned(), oid.to_owned())
        })
        .collect()
}

/// `HEAD`'s blobs: path → OID.
fn head_oids(repo: &Repo) -> BTreeMap<String, String> {
    let text = String::from_utf8(repo.git(&["ls-tree", "-r", "HEAD"])).unwrap();
    text.lines()
        .map(|line| {
            let (meta, path) = line.split_once('\t').unwrap();
            let oid = meta.split_whitespace().nth(2).unwrap();
            (path.to_owned(), oid.to_owned())
        })
        .collect()
}

/// AC-12: through a logging wrapper, a born `HEAD` with a changed, a
/// renamed and a new document: only `rev-parse`, `ls-files`, `cat-file`,
/// `diff-files`, `ls-tree`; `ls-tree -r -z <HEAD's OID>` once, in the
/// root; one `cat-file --batch`; `-c core.fsmonitor=false` and the four
/// variables on every call; each OID requested once; the changed path's
/// `HEAD` blob requested after every staged one, no other `HEAD` blob
/// (an unchanged path's, a renamed path's, are the staged ones). An
/// unborn `HEAD`: no `ls-tree`.
#[test]
fn the_base_runs_five_subcommands_and_one_session() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-wrapper", fixture);
        let top = &repo.top;
        // Every blob distinct, so an OID names one role.
        let tagged = |tag: &str| format!("{}\n{tag}\n", dangling_document(fixture));
        write(top, "docs/spec/zz-renamed.md", tagged("Renamed."));
        write(top, "docs/spec/zz-changed.md", tagged("Before."));
        write(top, "docs/spec/zz-empty.md", "");
        repo.add_all();
        repo.git.commit(top, "more documents");
        write(top, "docs/spec/zz-changed.md", tagged("After."));
        repo.git(&["mv", "docs/spec/zz-renamed.md", "docs/spec/zz-moved.md"]);
        write(top, "docs/spec/zz-added.md", tagged("Added."));
        repo.add_all();
        let head = repo.git.head(top).expect("a born HEAD");
        let staged = index_oids(&repo);
        let at_head = head_oids(&repo);

        let log = GitLog::new(&repo.scratch, &repo.git);
        for form in [&[][..], &["--json"]] {
            log.clear();
            let run = spec_in(&repo.git, top, &check_args(true, form), &log.env());
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
            let allowed: BTreeSet<String> =
                ["rev-parse", "ls-files", "cat-file", "diff-files", "ls-tree"]
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect();
            assert!(names.is_subset(&allowed), "{fixture}: {names:?}");
            for needed in ["rev-parse", "ls-files", "cat-file", "ls-tree", "diff-files"] {
                assert!(names.contains(needed), "{fixture}: {needed} ran: {names:?}");
            }
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
            let probes = calls
                .iter()
                .filter(|call| call.sub() == ["rev-parse", "--verify", "-q", "HEAD"])
                .count();
            assert_eq!(probes, 1, "{fixture}: {calls:?}");
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
            let changed_head = &at_head["docs/spec/zz-changed.md"];
            assert_ne!(changed_head, &staged["docs/spec/zz-changed.md"]);
            let at = requests
                .iter()
                .position(|oid| oid == changed_head)
                .unwrap_or_else(|| panic!("{fixture}: the changed path's HEAD blob is read"));
            for (path, oid) in &staged {
                if let Some(staged_at) = requests.iter().position(|seen| seen == oid) {
                    assert!(
                        staged_at < at,
                        "{fixture}: {path}'s staged blob after HEAD's"
                    );
                }
            }
            let staged_set: BTreeSet<&String> = staged.values().collect();
            let head_only: Vec<&String> = requests
                .iter()
                .filter(|oid| !staged_set.contains(oid))
                .collect();
            assert_eq!(
                head_only,
                [changed_head],
                "{fixture}: HEAD blobs read: {requests:?}"
            );
            assert_eq!(
                at_head["docs/spec/zz-renamed.md"], staged["docs/spec/zz-moved.md"],
                "the rename keeps the blob"
            );
        }

        // An unborn HEAD: probed once, never listed.
        let unborn = Repo::staged("intro-wrapper-unborn", fixture);
        let log = GitLog::new(&unborn.scratch, &unborn.git);
        let run = spec_in(&unborn.git, &unborn.top, &check_args(true, &[]), &log.env());
        run.code(1);
        assert!(
            log.calls().iter().all(|call| call.name() != "ls-tree"),
            "{fixture}: {:?}",
            log.calls()
        );
        assert_eq!(
            log.calls()
                .iter()
                .filter(|call| call.sub() == ["rev-parse", "--verify", "-q", "HEAD"])
                .count(),
            1
        );
    }
}

// ---------------------------------------------------------------------------
// AC-13: 2a.2's AC-07, AC-08, AC-12 with errors at HEAD.
// ---------------------------------------------------------------------------

/// AC-13: a linked worktree is judged against its own `HEAD` — a
/// document with an error committed only on its branch is pre-existing
/// there (exit 0), with `GIT_DIR` = its git dir as a hook gets it or
/// without; the main worktree, whose `HEAD` lacks it, staging the same
/// document: introduced (exit 1).
#[test]
fn a_linked_worktree_is_judged_against_its_own_head() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-linked", fixture);
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
        repo.git.add_all(&linked);
        let git_dir = repo.top.join(".git/worktrees/linked");
        for extra in [&[][..], &[("GIT_DIR", git_dir.as_os_str())]] {
            let run = spec_in(&repo.git, &linked, &["check", "--staged", "--debt"], extra);
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
        repo.add_all();
        let run = repo.staged_check(&[]);
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

/// AC-13: `GIT_DIR` and `GIT_WORK_TREE` naming a git dir apart from the
/// working tree (no `.git` there, discovery stopped by the ceiling): the
/// base's calls use them too — `HEAD` is read (exit 0 on the backlog, 1
/// with an introduced error), never a git failure.
#[test]
fn git_dir_and_work_tree_reach_the_base_s_calls() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("intro-separate");
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
        let run = spec_in(&git, &tree, &["check", "--staged", "--debt"], &vars);
        run.code(0);
        assert!(
            run.stdout.contains(" (pre-existing)"),
            "{fixture}:\n{}",
            run.show()
        );
        write(&tree, "docs/spec/zz-new.md", dangling_document(fixture));
        git.git_env(&tree, &["add", "-A"], &vars);
        let run = spec_in(&git, &tree, &["check", "--staged"], &vars);
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

/// AC-13: the guard refuses as before with errors at `HEAD` — `GIT_DIR`
/// alone below the top: exit 2, the guard's one cause, no `HEAD` probe,
/// no `ls-tree`.
#[test]
fn the_guard_refuses_before_the_base() {
    for (fixture, _) in FIXTURES {
        let repo = Repo::empty("intro-guard");
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
        repo.add_all();
        let log = GitLog::new(&repo.scratch, &repo.git);
        let git_dir = repo.top.join(".git");
        let mut extra = log.env().to_vec();
        extra.push(("GIT_DIR", git_dir.as_os_str()));
        let run = spec_in(&repo.git, &proj, &["check", "--staged"], &extra);
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
            log.calls()
                .iter()
                .all(|call| call.name() != "ls-tree"
                    && !call.argv.iter().any(|arg| arg == "--verify")),
            "{fixture}: {:?}",
            log.calls()
        );
        // From the top: the base runs (exit 1 on the introduced error).
        let run = spec_in(
            &repo.git,
            &repo.top,
            &["--root", "proj", "check", "--staged"],
            &[],
        );
        run.code(1);
    }
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

/// What AC-08 of 2a.2 compares.
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

/// AC-13: with errors at `HEAD`, a tracked file touched (stat-dirty), the
/// base read: `.git/index` (bytes, mtime), every file under `.git`
/// (objects, refs, `HEAD`), the working tree, the scratch `HOME` and
/// `XDG_CONFIG_HOME` are as they were, no `index.lock` — for a blocked
/// and an observed verdict, text and JSON.
#[test]
fn the_base_writes_nothing() {
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-readonly", fixture);
        let top = &repo.top;
        for (verdict, code) in [("observed", 0), ("blocked", 1)] {
            if verdict == "blocked" {
                write(top, "docs/spec/zz-new.md", dangling_document(fixture));
                repo.add_all();
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
                let run = repo.staged_check(form);
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

/// AC-13: names with TAB and LF (and a non-ASCII one, `core.quotePath`
/// on) committed with errors are pre-existing (exit 0); a new error in the
/// TAB-named one is introduced (exit 1, that path); a SHA-256 repository
/// alike.
#[test]
fn tab_lf_names_and_sha256_are_read_from_head() {
    let names = [
        "docs/spec/tab\there.md",
        "docs/spec/line\nbreak.md",
        "docs/spec/caf\u{e9} \"q\".md",
    ];
    for (fixture, _) in FIXTURES {
        let repo = backlog("intro-names", fixture);
        repo.git(&["config", "core.quotePath", "true"]);
        for path in names {
            write(&repo.top, path, dangling_document(fixture));
        }
        repo.add_all();
        repo.git.commit(&repo.top, "odd names");
        let run = repo.staged_check(&["--json"]);
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
        let mut text = dangling_document(fixture);
        text = text.replace(
            "refs: [",
            &format!(
                "refs: [{}, ",
                if fixture == "spec-a" {
                    "R-78"
                } else {
                    "REQ-778"
                }
            ),
        );
        write(&repo.top, names[0], &text);
        repo.add_all();
        let run = repo.staged_check(&["--json"]);
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
        let scratch = Scratch::new("intro-sha256");
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
        let run = spec_in(&repo.git, &repo.top, &check_args(true, &[]), &log.env());
        run.code(0);
        assert!(
            log.calls()
                .iter()
                .any(|call| call.sub() == ["ls-tree", "-r", "-z", head.as_str()]),
            "{fixture}: {:?}",
            log.calls()
        );
        write(&repo.top, "docs/spec/zz-new.md", dangling_document(fixture));
        repo.add_all();
        let run = repo.staged_check(&[]);
        run.code(1);
        assert_eq!(
            labelled(&run.stdout, "error").len(),
            1,
            "{fixture}:\n{}",
            run.show()
        );
    }
}
