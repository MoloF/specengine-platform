//! AC-15 of docs/features/spec-cli-check.md: the store's one loader of the
//! config and the baseline (`NamedBytes` in, `CheckSetup` or a
//! `cannot-check` report out), shared by `spec check`, `check_worktree`
//! and eval's `prepare`. Causes carry only the names passed in: config
//! bytes named `cfg.toml` with an unknown `[project]` key and
//! `mode = "strict"` give the two causes `cfg.toml:<line>` and nothing
//! else; `check_worktree` is the loader's report; a broken default
//! baseline is named `.spec-debt.toml`, the root `.`; no cause holds an
//! absolute path. (Eval refusing `[bogus]` at its line is in the eval
//! crate's `check_cli.rs`: its binary is not reachable from this crate.)

#![cfg(unix)]

mod common;

use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;

use common::{Scratch, copy_dir, fixture, read_text, write};
use specengine_core::check::{Mode, Report, Verdict};
use specengine_store::{
    BASELINE_FILE, NamedBytes, check_tree, check_worktree, default_baseline, load_check,
    load_config,
};

const TODAY: &str = "2026-09-30";

fn named(name: &str, text: &str) -> NamedBytes {
    NamedBytes {
        name: name.to_owned(),
        bytes: Ok(text.as_bytes().to_vec()),
    }
}

fn cause_paths(report: &Report) -> Vec<&str> {
    report
        .cannot_check
        .iter()
        .map(|cause| cause.path.as_str())
        .collect()
}

/// No cause names `absolute` (the scratch) or any absolute path.
fn assert_no_absolute(report: &Report, absolute: &Path, context: &str) {
    let shown = absolute.display().to_string();
    for cause in &report.cannot_check {
        assert!(
            !cause.path.starts_with('/') && !cause.path.contains(&shown),
            "{context}: {cause:?}"
        );
        assert!(!cause.message.contains(&shown), "{context}: {cause:?}");
    }
}

fn copy(scratch: &Scratch, name: &str) -> std::path::PathBuf {
    let root = scratch.join(name);
    copy_dir(&fixture(name), &root);
    root
}

#[test]
fn config_causes_carry_only_the_name_passed_in() {
    let text = "[project]\nslug = \"x\"\nflavour = 1\n\n[check]\nmode = \"strict\"\n";
    let report = *load_config(&named("cfg.toml", text)).unwrap_err();
    assert_eq!(report.verdict, Verdict::CannotCheck);
    assert_eq!(report.mode, Mode::Enforce, "an unreadable mode is enforce");
    let mut paths = cause_paths(&report);
    paths.sort_unstable();
    assert_eq!(paths, ["cfg.toml:3", "cfg.toml:6"], "{report:?}");
    assert!(
        report
            .cannot_check
            .iter()
            .any(|cause| cause.path == "cfg.toml:3" && cause.message.contains("flavour")),
        "{report:?}"
    );

    // A config error stops before the baseline.
    let broken = named(BASELINE_FILE, "[[debt]\n");
    let setup = load_check(&named("cfg.toml", text), Some(&broken)).unwrap_err();
    assert_eq!(
        *setup, report,
        "the baseline is not read after a config error"
    );

    // The mode is the config's when it can be read.
    let observe = "[project]\nflavour = 1\n\n[check]\nmode = \"observe\"\n";
    let report = *load_config(&named("cfg.toml", observe)).unwrap_err();
    assert_eq!(report.mode, Mode::Observe);
    assert_eq!(cause_paths(&report), ["cfg.toml:2"]);

    // Unreadable, not UTF-8: the name alone.
    let unreadable = NamedBytes {
        name: "cfg.toml".to_owned(),
        bytes: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
    };
    let report = *load_config(&unreadable).unwrap_err();
    assert_eq!(cause_paths(&report), ["cfg.toml"]);
    let latin1 = NamedBytes {
        name: "cfg.toml".to_owned(),
        bytes: Ok(b"[project]\nname = \"caf\xe9\"\n".to_vec()),
    };
    let report = *load_config(&latin1).unwrap_err();
    assert_eq!(cause_paths(&report), ["cfg.toml"]);

    // A valid config and baseline load.
    let setup = load_check(
        &named("cfg.toml", "[check]\nmode = \"observe\"\n"),
        Some(&named("debt.toml", "")),
    )
    .expect("a valid setup");
    assert_eq!(setup.config.mode, Mode::Observe);
    assert!(setup.baseline.entries.is_empty());
}

/// docs/canon/spec-check-cli.md "Cannot check" (task spec-check-process,
/// iteration 3): a config error is reported in mode `enforce` — the mode
/// is read only when all of `CheckConfig` is valid, so a `[budgets]`,
/// `[classes]`, `[check]` or `[[check.rules]]` error beside a valid
/// `[check] mode = "observe"` (or `enforce-introduced`, before or after
/// the bad key) is `enforce`, through `load_config`, `load_check` and
/// `check_worktree` alike. An error of `ProjectConfig` alone keeps the
/// written mode (the contrast, as in the test above). M: the mode read from
/// `[check] mode` alone (iteration 2's `CheckConfig::mode_from_toml`).
#[test]
fn a_check_config_error_is_judged_in_mode_enforce_whatever_mode_is_written() {
    let ids = "[ids]\nQ = { kind = \"question\", width = 3 }\n\n";
    // (the bad part, the 1-based line of its error within the part)
    let bad: [(&str, usize); 8] = [
        ("[budgets]\ntier0_bytes = 0\n", 2),
        ("[budgets]\ntier1_bytes = -4\n", 2),
        ("[classes.canon]\nflavour = 1\n", 2),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nflavour = 1\n",
            4,
        ),
        ("[[check.rules]]\nkeys = [\"to\"]\n", 1),
        (
            "[[check.rules]]\nkinds = [\"question\"]\nkeys = [\"to\"]\nseverity = \"high\"\n",
            4,
        ),
        (
            "[[check.rules]]\nkinds = [\"ticket\"]\nkeys = [\"to\"]\n",
            2,
        ),
        ("[[check.rules]]\nkinds = [\"question\"]\nparts = []\n", 3),
    ];
    let scratch = Scratch::new("check-loader-mode");
    let root = copy(&scratch, "spec-a");
    let fixture_config = read_text(&root, "specengine.toml");
    for written in ["observe", "enforce-introduced"] {
        for (part, at) in bad {
            let check = format!("[check]\nmode = \"{written}\"\n\n");
            // `[check]` before the bad part, and after it.
            for (text, line) in [
                (
                    format!("{ids}{check}{part}"),
                    ids.lines().count() + check.lines().count() + at,
                ),
                (format!("{ids}{part}\n{check}"), ids.lines().count() + at),
            ] {
                let context = format!("{written}:\n{text}");
                let report = *load_config(&named("cfg.toml", &text)).unwrap_err();
                assert_eq!(report.verdict, Verdict::CannotCheck, "{context}");
                assert_eq!(report.mode, Mode::Enforce, "{context}\n{report:?}");
                assert_eq!(
                    cause_paths(&report),
                    [format!("cfg.toml:{line}").as_str()],
                    "{context}"
                );
                let setup = load_check(&named("cfg.toml", &text), None).unwrap_err();
                assert_eq!(*setup, report, "{context}");
                // A valid baseline is not read either: the same report.
                let debt = named(BASELINE_FILE, "");
                let setup = load_check(&named("cfg.toml", &text), Some(&debt)).unwrap_err();
                assert_eq!(*setup, report, "{context}");

                // On disk, beside spec-a's own config.
                write(
                    &root,
                    "specengine.toml",
                    format!("{fixture_config}\n{check}{part}"),
                );
                let report = check_worktree(&root, &root.join("specengine.toml"), None, TODAY);
                assert_eq!(report.verdict, Verdict::CannotCheck, "{context}");
                assert_eq!(report.mode, Mode::Enforce, "{context}\n{report:?}");
                assert_eq!(report.cannot_check.len(), 1, "{context}\n{report:?}");
            }
        }
    }
    // The bad part alone is valid with the mode: the written mode stands.
    let fixed = format!("{ids}[check]\nmode = \"observe\"\n\n[budgets]\ntier0_bytes = 1\n");
    let (_, config) = load_config(&named("cfg.toml", &fixed)).expect("a valid config");
    assert_eq!(config.mode, Mode::Observe);
    // A `ProjectConfig` error with a valid `CheckConfig`: still observe.
    let project = format!("{ids}[project]\nflavour = 1\n\n[check]\nmode = \"observe\"\n");
    let report = *load_config(&named("cfg.toml", &project)).unwrap_err();
    assert_eq!(report.mode, Mode::Observe, "{report:?}");
    // Both broken: enforce, both causes.
    let both = format!("{project}\n[budgets]\ntier0_bytes = 0\n");
    let report = *load_config(&named("cfg.toml", &both)).unwrap_err();
    assert_eq!(report.mode, Mode::Enforce, "{report:?}");
    let mut paths = cause_paths(&report);
    paths.sort_unstable();
    assert_eq!(paths, ["cfg.toml:11", "cfg.toml:5"], "{report:?}");
}

#[test]
fn check_worktree_is_the_loader_s_report() {
    for name in ["spec-a", "spec-b"] {
        let scratch = Scratch::new("check-loader");
        let root = copy(&scratch, name);
        let config_path = root.join("specengine.toml");
        let through_loader = |root: &Path| {
            let config = NamedBytes::read("specengine.toml", &config_path);
            match load_check(&config, default_baseline(root).as_ref()) {
                Ok(setup) => check_tree(root, &setup, TODAY),
                Err(report) => *report,
            }
        };
        let report = check_worktree(&root, &config_path, None, TODAY);
        assert_eq!(report.verdict, Verdict::Blocked, "{name}");
        assert_eq!(report, through_loader(&root), "{name}");

        // With a default baseline, valid and broken.
        write(
            &root,
            BASELINE_FILE,
            "[[debt]]\ncode = \"ref-dangling\"\npath = \"docs/none.md\"\nreason = \"r\"\nexpires = \"2999-12-31\"\n",
        );
        let report = check_worktree(&root, &config_path, None, TODAY);
        assert_eq!(report.counts.stale, 1, "{name}");
        assert_eq!(report, through_loader(&root), "{name}");

        write(&root, BASELINE_FILE, "[[debt]\n");
        let report = check_worktree(&root, &config_path, None, TODAY);
        assert_eq!(report.verdict, Verdict::CannotCheck, "{name}");
        assert_eq!(report, through_loader(&root), "{name}");
        let paths = cause_paths(&report);
        assert_eq!(paths.len(), 1, "{report:?}");
        assert!(
            paths[0] == BASELINE_FILE || paths[0].starts_with(&format!("{BASELINE_FILE}:")),
            "{name}: {paths:?}"
        );
        assert_no_absolute(&report, scratch.path(), name);
        assert_eq!(read_text(&root, BASELINE_FILE), "[[debt]\n");
    }
}

#[test]
fn the_default_baseline_the_root_and_the_config_are_named_without_their_path() {
    let scratch = Scratch::new("check-loader-names");
    let root = copy(&scratch, "spec-a");
    let config_path = root.join("specengine.toml");

    assert!(default_baseline(&root).is_none(), "no entry, no baseline");
    // A dangling symlink is an entry: unreadable.
    symlink(scratch.join("gone.toml"), root.join(BASELINE_FILE)).unwrap();
    let baseline = default_baseline(&root).expect("an entry of that name");
    assert_eq!(baseline.name, BASELINE_FILE);
    assert!(baseline.bytes.is_err());
    let report = check_worktree(&root, &config_path, None, TODAY);
    assert_eq!(cause_paths(&report), [BASELINE_FILE], "{report:?}");
    assert!(
        report.cannot_check[0]
            .message
            .starts_with("cannot read the baseline: ")
    );
    assert_no_absolute(&report, scratch.path(), "dangling baseline");
    std::fs::remove_file(root.join(BASELINE_FILE)).unwrap();

    // A root that does not exist: `.`.
    let config = NamedBytes::read("specengine.toml", &config_path);
    let setup = load_check(&config, None).expect("spec-a's config loads");
    let missing = scratch.join("no-such-root");
    let report = check_tree(&missing, &setup, TODAY);
    assert_eq!(report.verdict, Verdict::CannotCheck);
    assert_eq!(cause_paths(&report), ["."], "{report:?}");
    assert_no_absolute(&report, scratch.path(), "missing root");
    let report = check_worktree(&missing, &config_path, None, TODAY);
    assert_eq!(cause_paths(&report), ["."], "{report:?}");
    assert_no_absolute(&report, scratch.path(), "missing root (worktree)");

    // A config that cannot be read: its file name only.
    let report = check_worktree(&root, &scratch.join("cfg/none.toml"), None, TODAY);
    assert_eq!(cause_paths(&report), ["none.toml"], "{report:?}");
    assert_no_absolute(&report, scratch.path(), "missing config");

    // An invalid config at an absolute path: `<file name>:<line>`.
    let text = read_text(&root, "specengine.toml");
    write(
        scratch.path(),
        "cfg/bad.toml",
        format!("{text}\n[bogus]\nx = 1\n"),
    );
    let report = check_worktree(&root, &scratch.join("cfg/bad.toml"), None, TODAY);
    let line = text.lines().count() + 2;
    assert_eq!(cause_paths(&report), [format!("bad.toml:{line}").as_str()]);
    assert_no_absolute(&report, scratch.path(), "invalid config");
}
