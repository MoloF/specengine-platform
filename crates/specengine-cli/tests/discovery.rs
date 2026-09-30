//! AC-02 and AC-03 of docs/features/spec-cli.md: the project is found by
//! walking up from the canonical current directory to the first
//! `specengine.toml`; `--root` does not walk; `--config` replaces
//! `<root>/specengine.toml` and leaves the copy untouched.

#![cfg(unix)]

mod common;

use common::{Scratch, index, snapshot, spec, write};

/// AC-02: from the root, from a subdirectory and with `--root` from
/// elsewhere, `spec show MEC-STAMINA` prints the same bytes; for both
/// fixtures (AC-17), `show` of the first document ID found in each.
#[test]
fn show_from_the_root_a_subdirectory_and_root_flag_is_identical() {
    for (fixture, reference, subdirectory) in [
        ("spec-a", "MEC-STAMINA", "docs/spec/movement"),
        ("spec-b", "REQ-001", "docs/records/REQ"),
    ] {
        let scratch = Scratch::new("discovery");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let elsewhere = scratch.dir("elsewhere");
        index(&home, &root);

        let from_root = spec(&home, &root, &["show", reference]);
        from_root.code(0);
        assert!(
            from_root.stdout.starts_with(&format!("{reference} | ")),
            "{fixture}: {}",
            from_root.show()
        );
        let from_below = spec(&home, &root.join(subdirectory), &["show", reference]);
        from_below.code(0);
        let root_flag = spec(
            &home,
            &elsewhere,
            &["--root", root.to_str().unwrap(), "show", reference],
        );
        root_flag.code(0);
        assert_eq!(
            from_below.stdout, from_root.stdout,
            "{fixture}: from {subdirectory}"
        );
        assert_eq!(
            root_flag.stdout, from_root.stdout,
            "{fixture}: --root from elsewhere"
        );
        // A relative --root is taken from the current directory.
        let relative = spec(
            &home,
            scratch.path(),
            &["--root", "copy", "show", reference],
        );
        relative.code(0);
        assert_eq!(relative.stdout, from_root.stdout, "{fixture}: --root copy");
    }
}

/// AC-02: no `specengine.toml` up the tree → exit 2, empty stdout, stderr
/// names `spec init`; `--json` prints nothing either.
#[test]
fn no_config_up_the_tree_is_exit_2_naming_spec_init() {
    let scratch = Scratch::new("discovery-none");
    let home = scratch.home("h");
    let empty = scratch.dir("nothing/below");
    for args in [
        &["show", "MEC-STAMINA"][..],
        &["search", "stamina"],
        &["index"],
        &["--json", "show", "MEC-STAMINA"],
        &["--json", "index"],
    ] {
        let run = spec(&home, &empty, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        assert_eq!(run.stderr_lines().len(), 1, "{args:?}: {}", run.show());
        assert!(
            run.stderr.starts_with("spec: ") && run.stderr.contains("spec init"),
            "{args:?}: {}",
            run.show()
        );
    }
    assert!(
        snapshot(&home).is_empty(),
        "nothing created under HOME: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
}

/// `--root DIR` does not walk: a directory without its own
/// `specengine.toml` is refused even when an ancestor has one.
#[test]
fn root_flag_does_not_walk_up() {
    let scratch = Scratch::new("discovery-root");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec(
        &home,
        scratch.path(),
        &["--root", root.join("docs").to_str().unwrap(), "index"],
    );
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.starts_with("spec: "), "{}", run.show());
    assert!(snapshot(&home).is_empty(), "nothing created under HOME");
}

/// AC-03: the copy's own config deleted, `--root <copy> --config
/// <outside>` serves `index`, `search` and `show`; the copy is unchanged.
/// `--config` without `--root`: the root is the current directory.
#[test]
fn config_flag_replaces_the_root_config_and_leaves_the_copy_untouched() {
    for (fixture, reference, term) in [
        ("spec-a", "RULE-STAM-REGEN", "regeneration"),
        ("spec-b", "CRIT-01", "dry-run"),
    ] {
        let scratch = Scratch::new("discovery-config");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let outside = scratch.dir("outside").join("pilot.toml");
        std::fs::rename(root.join("specengine.toml"), &outside).expect("move the config out");
        let before = snapshot(&root);
        let elsewhere = scratch.dir("elsewhere");
        let root_arg = root.to_str().unwrap();
        let config_arg = outside.to_str().unwrap();

        let run = spec(
            &home,
            &elsewhere,
            &["--root", root_arg, "--config", config_arg, "index"],
        );
        run.code(0);
        assert!(
            run.stdout.starts_with("indexed "),
            "{fixture}: {}",
            run.show()
        );
        let found = spec(
            &home,
            &elsewhere,
            &["--root", root_arg, "--config", config_arg, "search", term],
        );
        found.code(0);
        assert!(
            found.stdout.lines().count() > 1,
            "{fixture}: search {term} found nothing:\n{}",
            found.show()
        );
        let shown = spec(
            &home,
            &elsewhere,
            &[
                "--root", root_arg, "--config", config_arg, "show", reference,
            ],
        );
        shown.code(0);
        assert!(
            shown.stdout.starts_with(&format!("{reference} | ")),
            "{fixture}: {}",
            shown.show()
        );
        // Without --root: the current directory is the root.
        let from_cwd = spec(&home, &root, &["--config", config_arg, "show", reference]);
        from_cwd.code(0);
        assert_eq!(
            from_cwd.stdout, shown.stdout,
            "{fixture}: --config from the root"
        );
        assert_eq!(snapshot(&root), before, "{fixture}: the copy is unchanged");

        // A config error names the config as given.
        write(
            scratch.path(),
            "outside/bad.toml",
            "[project]\nslug = \"Bad\"\n",
        );
        let bad = spec(
            &home,
            &elsewhere,
            &[
                "--root",
                root_arg,
                "--config",
                "../outside/bad.toml",
                "index",
            ],
        );
        bad.code(2);
        assert_eq!(bad.stdout, "");
        assert!(
            bad.stderr.starts_with("../outside/bad.toml:2: "),
            "{fixture}: {}",
            bad.show()
        );
        assert_eq!(snapshot(&root), before, "{fixture}: the copy is unchanged");
    }
}
