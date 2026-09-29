//! AC-14 and AC-21 of docs/features/spec-check.md, through the store's
//! fresh-parse loader `check_worktree` on scratch copies of the fixtures.
//!
//! AC-14: a blocking finding exits 0 under `observe`, 1 under `enforce`; a
//! missing root, a missing written `[paths] roots` entry, a mode-000 file or
//! directory, an invalid or unreadable config or baseline exit 2 in both
//! modes; spec-a, whose default `docs/archive` role root is missing, is not
//! 2.
//!
//! AC-21: the check writes nothing: `git status --porcelain -- fixtures/`
//! is the same before and after a check of the fixtures in place, and every
//! scratch file keeps its bytes and mtime.
//!
//! docs/features/spec-check-graph.md AC-04/AC-05 through the loader: an
//! invalid `[[generators]]` table is `specengine.toml:<line>: message`,
//! exit 2; the `[paths] index` file absent, outside the roots or excluded is
//! `index-missing`, the walked render is clean, and nothing is written.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use common::{Scratch, chmod, copy_dir, fixture, fixtures_git_status, write};
use specengine_core::check::{Report, Verdict};
use specengine_store::{BASELINE_FILE, check_worktree, today_utc};

const TODAY: &str = "2026-09-29";

/// A scratch copy of `fixtures/<name>` with `extra` appended to its
/// `specengine.toml`; returns the root.
fn copy(scratch: &Scratch, name: &str, dir: &str, extra: &str) -> PathBuf {
    let root = scratch.join(dir);
    copy_dir(&fixture(name), &root);
    if !extra.is_empty() {
        let config = root.join("specengine.toml");
        let text = fs::read_to_string(&config).unwrap();
        fs::write(&config, format!("{text}\n{extra}")).unwrap();
    }
    root
}

fn check(root: &Path) -> Report {
    check_worktree(root, &root.join("specengine.toml"), None, TODAY)
}

fn exit(report: &Report) -> u8 {
    report.exit_code()
}

const OBSERVE: &str = "[check]\nmode = \"observe\"\n";

#[test]
fn a_blocking_finding_exits_0_under_observe_and_1_under_enforce() {
    let scratch = Scratch::new("check-verdict-modes");
    let enforce = copy(&scratch, "spec-a", "enforce", "");
    let observe = copy(&scratch, "spec-a", "observe", OBSERVE);
    let report = check(&enforce);
    assert_eq!(
        report.verdict,
        Verdict::Blocked,
        "{:#?}",
        report.lines(true)
    );
    assert_eq!(exit(&report), 1);
    let report = check(&observe);
    assert_eq!(
        report.verdict,
        Verdict::Observed,
        "{:#?}",
        report.lines(true)
    );
    assert_eq!(exit(&report), 0);
    assert_eq!(report.counts.documents, 14);
}

#[test]
fn spec_a_without_its_default_archive_root_is_not_cannot_check() {
    let scratch = Scratch::new("check-verdict-archive");
    let root = copy(&scratch, "spec-a", "wt", "");
    assert!(
        !root.join("docs/archive").exists(),
        "the default role root is missing"
    );
    let report = check(&root);
    assert_ne!(
        report.verdict,
        Verdict::CannotCheck,
        "{:#?}",
        report.lines(true)
    );
    assert!(report.cannot_check.is_empty());
}

/// Each case must exit 2 under both modes.
fn assert_cannot(case: &str, reports: [Report; 2]) {
    for report in reports {
        assert_eq!(
            report.verdict,
            Verdict::CannotCheck,
            "{case} [{}]: {:#?}",
            report.mode,
            report.lines(true)
        );
        assert_eq!(exit(&report), 2, "{case}");
        assert!(!report.cannot_check.is_empty(), "{case}: a cause is named");
    }
}

#[test]
fn a_missing_root_exits_2_in_both_modes() {
    let scratch = Scratch::new("check-verdict-root");
    let root = copy(&scratch, "spec-b", "wt", "");
    let observed = copy(&scratch, "spec-b", "observed", OBSERVE);
    let missing = scratch.join("nowhere");
    assert_cannot(
        "missing root",
        [
            check_worktree(&missing, &root.join("specengine.toml"), None, TODAY),
            check_worktree(&missing, &observed.join("specengine.toml"), None, TODAY),
        ],
    );
}

#[test]
fn a_missing_written_root_exits_2_in_both_modes() {
    let scratch = Scratch::new("check-verdict-written-root");
    let mut roots = Vec::new();
    for (dir, extra) in [("enforce", ""), ("observe", OBSERVE)] {
        // spec-b writes `roots = ["docs"]`; add one that does not exist.
        let root = copy(&scratch, "spec-b", dir, extra);
        let config = root.join("specengine.toml");
        let text = fs::read_to_string(&config).unwrap();
        assert_eq!(text.matches("roots = [\"docs\"]").count(), 1);
        fs::write(
            &config,
            text.replace("roots = [\"docs\"]", "roots = [\"docs\", \"notes\"]"),
        )
        .unwrap();
        roots.push(root);
    }
    let reports = [check(&roots[0]), check(&roots[1])];
    assert!(
        reports[0]
            .cannot_check
            .iter()
            .any(|cause| cause.path == "notes"),
        "{:#?}",
        reports[0].cannot_check
    );
    assert_cannot("missing written root", reports);
}

#[test]
fn an_unreadable_file_or_directory_exits_2_in_both_modes() {
    let scratch = Scratch::new("check-verdict-mode-000");
    for (case, target) in [
        ("mode-000 file", "docs/spec/cli.md"),
        ("mode-000 directory", "docs/records/QN"),
    ] {
        let mut reports = Vec::new();
        for (dir, extra) in [("enforce", ""), ("observe", OBSERVE)] {
            let root = copy(&scratch, "spec-b", &format!("{case}-{dir}"), extra);
            chmod(&root, target, 0o000);
            reports.push(check(&root));
            chmod(
                &root,
                target,
                if case.ends_with("directory") {
                    0o755
                } else {
                    0o644
                },
            );
        }
        let reports: [Report; 2] = reports.try_into().unwrap();
        assert!(
            reports[0]
                .cannot_check
                .iter()
                .any(|cause| cause.path == target),
            "{case}: {:#?}",
            reports[0].cannot_check
        );
        assert_cannot(case, reports);
    }
}

#[test]
fn an_invalid_or_missing_config_exits_2() {
    let scratch = Scratch::new("check-verdict-config");
    for (case, extra) in [
        ("unknown mode", "[check]\nmode = \"strict\"\n"),
        ("cap 0", "[budgets]\ntier0_bytes = 0\n"),
        ("unknown class", "[classes]\nmemo = {}\n"),
        ("bad TOML", "[check\n"),
        (
            "cap 0 under observe",
            "[check]\nmode = \"observe\"\n\n[budgets]\ntier0_bytes = 0\n",
        ),
        (
            "a second [ids] table under observe",
            "[check]\nmode = \"observe\"\n\n[ids]\nZZ = { kind = \"z\", width = \"two\" }\n",
        ),
    ] {
        let root = copy(&scratch, "spec-a", case, extra);
        let report = check(&root);
        assert_eq!(report.verdict, Verdict::CannotCheck, "{case}");
        assert_eq!(exit(&report), 2, "{case}");
        assert_eq!(report.counts.documents, 0, "{case}: nothing walked");
        let cause = &report.cannot_check[0];
        assert!(
            cause.path.contains("specengine.toml:"),
            "{case}: file:line: {cause:?}"
        );
    }
    let root = copy(&scratch, "spec-a", "no-config", "");
    let report = check_worktree(&root, &root.join("missing.toml"), None, TODAY);
    assert_eq!(exit(&report), 2);
}

#[test]
fn an_invalid_or_missing_baseline_exits_2() {
    let scratch = Scratch::new("check-verdict-baseline");
    // Invalid at the root: read by default.
    for (case, text) in [
        (
            "no reason",
            "[[debt]]\ncode = \"budget\"\npath = \"x.md\"\nexpires = \"2026-12-31\"\n",
        ),
        (
            "no expires",
            "[[debt]]\ncode = \"budget\"\npath = \"x.md\"\nreason = \"r\"\n",
        ),
        ("bad TOML", "[[debt]\n"),
    ] {
        let root = copy(&scratch, "spec-a", case, "");
        write(&root, BASELINE_FILE, text);
        let report = check(&root);
        assert_eq!(report.verdict, Verdict::CannotCheck, "{case}");
        assert!(
            report.cannot_check[0].path.contains(BASELINE_FILE),
            "{case}: {:?}",
            report.cannot_check
        );
    }
    // Passed but missing.
    let root = copy(&scratch, "spec-a", "passed", "");
    let report = check_worktree(
        &root,
        &root.join("specengine.toml"),
        Some(&root.join("elsewhere.toml")),
        TODAY,
    );
    assert_eq!(report.verdict, Verdict::CannotCheck);
}

#[test]
fn a_valid_baseline_at_the_root_is_read_by_default() {
    let scratch = Scratch::new("check-verdict-default-baseline");
    let root = copy(&scratch, "spec-a", "wt", "");
    write(
        &root,
        BASELINE_FILE,
        // `tier: two`: a spanless `frontmatter-type` has its written key as
        // the subject (K1 of docs/features/phase1-cleanup.md).
        "[[debt]]\ncode = \"frontmatter-type\"\npath = \"docs/features/stamina-tuning.md\"\nsubject = \"tier\"\nreason = \"fixture\"\nexpires = \"2026-12-31\"\n",
    );
    let report = check(&root);
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert_eq!(report.counts.debt, 1);
    let later = check_worktree(&root, &root.join("specengine.toml"), None, "2027-01-01");
    assert_eq!(later.verdict, Verdict::Blocked);
    assert_eq!(later.counts.expired, 1);
}

#[test]
fn today_utc_is_a_calendar_date() {
    let today = today_utc();
    assert!(specengine_core::check::is_calendar_date(&today), "{today}");
    assert!(today.as_str() >= "2026-01-01", "{today}");
}

// ------------------------------------------------------------------ AC-21

/// Bytes and mtime of every file under `root`.
fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, (Vec<u8>, SystemTime)>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let modified = fs::metadata(&path).unwrap().modified().unwrap();
                out.insert(
                    path.strip_prefix(root).unwrap().display().to_string(),
                    (fs::read(&path).unwrap(), modified),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn the_check_writes_nothing() {
    // In place on the fixtures: git sees no change.
    let before = fixtures_git_status();
    for name in ["spec-a", "spec-b"] {
        let root = fixture(name);
        let report = check(&root);
        assert_eq!(report.verdict, Verdict::Blocked, "{name}");
    }
    assert_eq!(fixtures_git_status(), before, "fixtures/ changed");

    // A scratch copy with homoglyph fixes on offer: bytes and mtimes kept.
    let scratch = Scratch::new("check-read-only");
    let root = copy(&scratch, "spec-b", "wt", "");
    let snapshot_before = snapshot(&root);
    let report = check(&root);
    assert!(
        report.findings.iter().any(|f| f.fix.is_some()),
        "a fix is offered as data"
    );
    let snapshot_after = snapshot(&root);
    assert_eq!(
        snapshot_before.keys().collect::<Vec<_>>(),
        snapshot_after.keys().collect::<Vec<_>>(),
        "no file created or removed"
    );
    for (path, (bytes, modified)) in &snapshot_before {
        let (after_bytes, after_modified) = &snapshot_after[path];
        assert!(bytes == after_bytes, "{path}: bytes changed");
        assert_eq!(modified, after_modified, "{path}: mtime changed");
    }
}

// ------------------------------------------------------------------ spec-check-graph

#[test]
fn an_invalid_generator_registry_exits_2_with_file_line() {
    let scratch = Scratch::new("check-verdict-generators");
    for (case, extra, line) in [
        (
            "command missing",
            "[[generators]]\nwrites = [\"docs/a.md\"]\n",
            1,
        ),
        (
            "gate without index",
            "[[generators]]\ncommand = \"a\"\nwrites = [\"docs/a.md\"]\ngate = \"g\"\n",
            4,
        ),
        (
            "index = true without [paths] index",
            "[[generators]]\ncommand = \"a\"\nwrites = [\"docs/a.md\"]\nindex = true\n",
            4,
        ),
        (
            "unknown key, observe",
            "[check]\nmode = \"observe\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"docs/a.md\"]\nrun = 1\n",
            7,
        ),
    ] {
        let root = copy(&scratch, "spec-b", case, extra);
        let base = fs::read_to_string(fixture("spec-b").join("specengine.toml"))
            .unwrap()
            .lines()
            .count()
            + 1;
        let report = check(&root);
        assert_eq!(report.verdict, Verdict::CannotCheck, "{case}");
        assert_eq!(exit(&report), 2, "{case}");
        let cause = &report.cannot_check[0];
        assert!(
            cause
                .path
                .ends_with(&format!("specengine.toml:{}", base + line)),
            "{case}: file:line: {cause:?}"
        );
    }
}

/// spec-b with `[paths] index` = `index` and `exclude` added, and an index
/// registry entry.
fn with_index(scratch: &Scratch, dir: &str, index: &str, exclude: &str) -> PathBuf {
    let root = copy(
        scratch,
        "spec-b",
        dir,
        &format!(
            "[[generators]]\ncommand = \"make index\"\nwrites = [\"{index}\"]\nindex = true\n"
        ),
    );
    let config = root.join("specengine.toml");
    let text = fs::read_to_string(&config).unwrap();
    assert_eq!(text.matches("[paths]\n").count(), 1);
    let text = text.replacen(
        "[paths]\n",
        &format!("[paths]\nindex = \"{index}\"\nexclude = [{exclude}]\n"),
        1,
    );
    fs::write(&config, text).unwrap();
    root
}

fn index_codes(report: &Report) -> Vec<(String, String)> {
    report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("index-"))
        .map(|f| (f.code.clone(), f.path.clone()))
        .collect()
}

/// The render of the walked tree, as the check makes it.
fn render_of(root: &Path) -> String {
    use specengine_core::{IdSchemeToml, Paths};
    let text = fs::read_to_string(root.join("specengine.toml")).unwrap();
    let scheme = specengine_model::IdScheme::from_toml(&text).unwrap();
    let paths = Paths::from_toml(&text).unwrap();
    let config = specengine_core::check::CheckConfig::from_toml(&text).unwrap();
    let tree = specengine_store::WorkingTree::new(root, &paths).unwrap();
    let input = specengine_store::check_input(&tree, &scheme);
    specengine_core::check::render_index(
        &input,
        paths.index.as_deref().unwrap(),
        config.index_generator().unwrap(),
    )
}

#[test]
fn an_index_not_walked_is_index_missing_and_the_walked_render_is_clean() {
    let scratch = Scratch::new("check-verdict-index");
    let missing = |code: &str, path: &str| vec![(code.to_owned(), path.to_owned())];

    // Absent.
    let root = with_index(&scratch, "absent", "docs/index.md", "");
    assert_eq!(
        index_codes(&check(&root)),
        missing("index-missing", "docs/index.md")
    );

    // Walked, the render: no index finding, and the render itself is stable.
    let root = with_index(&scratch, "walked", "docs/index.md", "");
    let render = render_of(&root);
    write(&root, "docs/index.md", &render);
    assert_eq!(
        render_of(&root),
        render,
        "the index is not listed in itself"
    );
    let before = fs::read(root.join("docs/index.md")).unwrap();
    let report = check(&root);
    assert!(index_codes(&report).is_empty(), "{:?}", report.lines(true));
    assert_eq!(fs::read(root.join("docs/index.md")).unwrap(), before);

    // Walked, hand-edited: drift, and the file keeps its bytes.
    write(&root, "docs/index.md", format!("{render}x\n"));
    let report = check(&root);
    assert_eq!(
        index_codes(&report),
        missing("index-drift", "docs/index.md")
    );
    assert_eq!(
        fs::read_to_string(root.join("docs/index.md")).unwrap(),
        format!("{render}x\n"),
        "nothing is written"
    );

    // Outside the roots (`roots = ["docs"]`): present but not walked.
    let root = with_index(&scratch, "outside", "site/index.md", "");
    write(&root, "site/index.md", "whatever\n");
    assert_eq!(
        index_codes(&check(&root)),
        missing("index-missing", "site/index.md")
    );

    // Excluded.
    let root = with_index(&scratch, "excluded", "docs/index.md", "\"docs/index.md\"");
    write(&root, "docs/index.md", "whatever\n");
    assert_eq!(
        index_codes(&check(&root)),
        missing("index-missing", "docs/index.md")
    );
}

/// Iteration 2: an incomplete walk (a mode-000 file or directory, a written
/// root missing) is "cannot check" and the index is neither compared nor
/// reported missing: the render would not list every document.
#[test]
fn an_incomplete_walk_gives_no_index_finding() {
    let scratch = Scratch::new("check-verdict-index-walk");
    type Break = fn(&Path);
    let cases: [(&str, Break, Break); 3] = [
        (
            "mode-000 file",
            |root| chmod(root, "docs/spec/cli.md", 0o000),
            |root| chmod(root, "docs/spec/cli.md", 0o644),
        ),
        (
            "mode-000 directory",
            |root| chmod(root, "docs/records/QN", 0o000),
            |root| chmod(root, "docs/records/QN", 0o755),
        ),
        (
            "a written root missing",
            |root| {
                let config = root.join("specengine.toml");
                let text = fs::read_to_string(&config).unwrap();
                assert_eq!(text.matches("roots = [\"docs\"]").count(), 1);
                fs::write(
                    &config,
                    text.replace("roots = [\"docs\"]", "roots = [\"docs\", \"notes\"]"),
                )
                .unwrap();
            },
            |_| {},
        ),
    ];
    for (case, break_walk, repair) in cases {
        for (state, index) in [("drifted", Some("stale\n")), ("absent", None)] {
            let root = with_index(&scratch, &format!("{case}-{state}"), "docs/index.md", "");
            if let Some(bytes) = index {
                write(&root, "docs/index.md", bytes);
            }
            // Complete: the rule speaks.
            assert_eq!(index_codes(&check(&root)).len(), 1, "{case}, {state}");
            break_walk(&root);
            let report = check(&root);
            repair(&root);
            assert_eq!(report.verdict, Verdict::CannotCheck, "{case}, {state}");
            assert!(
                index_codes(&report).is_empty(),
                "{case}, {state}: {:#?}",
                report.lines(true)
            );
        }
    }
}
