//! AC-05 of docs/features/spec-cli.md: the index lives at the host rule's
//! `<data dir>/<slug>.db` (macOS: `~/Library/Application Support/specengine`;
//! elsewhere `$XDG_DATA_HOME/specengine` when absolute, else
//! `~/.local/share/specengine`), one file per slug, shared by copies of one
//! project; `HOME` unset, empty, relative or placing the data directory
//! inside the project root → exit 2 with nothing created.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::ffi::OsStr;

use common::{FIXTURES, Scratch, data_dir, index, paths_under, replace, snapshot, spec, spec_with};

/// Every path `index` may leave under `HOME`: the data directory, its
/// ancestors below `HOME`, the databases of `slugs` and at most their
/// `-wal` and `-shm` files.
fn allowed_under_home(slugs: &[&str]) -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();
    let data = if cfg!(target_os = "macos") {
        "Library/Application Support/specengine"
    } else {
        ".local/share/specengine"
    };
    let mut prefix = String::new();
    for part in data.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        allowed.insert(prefix.clone());
    }
    for slug in slugs {
        for suffix in ["", "-wal", "-shm"] {
            allowed.insert(format!("{data}/{slug}.db{suffix}"));
        }
    }
    allowed
}

fn assert_only(home: &std::path::Path, slugs: &[&str]) {
    let allowed = allowed_under_home(slugs);
    let present = paths_under(home);
    let extra: Vec<&String> = present.iter().filter(|p| !allowed.contains(*p)).collect();
    assert!(extra.is_empty(), "unexpected entries under HOME: {extra:?}");
    for slug in slugs {
        assert!(
            data_dir(home).join(format!("{slug}.db")).is_file(),
            "{slug}.db exists: {present:?}"
        );
    }
}

#[test]
fn index_creates_only_the_slug_database_under_home() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("location");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let run = index(&home, &root);
        let db = data_dir(&home).join(format!("{slug}.db"));
        assert_eq!(
            run.stdout.lines().nth(1),
            Some(format!("db {}", db.display()).as_str()),
            "{fixture}: the db line\n{}",
            run.show()
        );
        assert_only(&home, &[slug]);
    }
}

#[test]
fn two_slugs_make_two_databases_in_one_home() {
    let scratch = Scratch::new("location-two");
    let home = scratch.home("h");
    for (fixture, _) in FIXTURES {
        let root = scratch.copy(fixture, fixture);
        index(&home, &root);
    }
    assert_only(&home, &["lantern-keep", "zerkalo"]);
    let dbs: Vec<String> = paths_under(&data_dir(&home))
        .into_iter()
        .filter(|name| name.ends_with(".db"))
        .collect();
    assert_eq!(dbs, ["lantern-keep.db", "zerkalo.db"]);
}

/// Two copies of one project share its one database; each copy's `show`
/// prints its own bytes.
#[test]
fn a_second_copy_of_one_slug_shares_the_database() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("location-shared");
        let home = scratch.home("h");
        let first = scratch.copy(fixture, "first");
        let second = scratch.copy(fixture, "second");
        let (path, reference) = if fixture == "spec-a" {
            ("docs/records/R/R-12.md", "R-12")
        } else {
            ("docs/records/REQ/REQ-001.md", "REQ-001")
        };
        replace(&second, path, "owner: owner\n", "owner: second-copy\n");
        index(&home, &first);
        index(&home, &second);
        assert_only(&home, &[slug]);
        let one = spec(&home, &first, &["show", reference]);
        one.code(0);
        let two = spec(&home, &second, &["show", reference]);
        two.code(0);
        assert!(
            !one.stdout.contains("second-copy"),
            "{fixture}: {}",
            one.show()
        );
        assert!(
            two.stdout.contains("second-copy"),
            "{fixture}: {}",
            two.show()
        );
        assert_only(&home, &[slug]);
    }
}

/// `HOME` unset, empty or relative → exit 2, nothing created anywhere in
/// the scratch (the copy included).
#[test]
fn home_unset_empty_or_relative_is_exit_2_with_nothing_created() {
    let scratch = Scratch::new("location-home");
    let root = scratch.copy("spec-a", "copy");
    let before = snapshot(scratch.path());
    let relative = OsStr::new("homes/relative");
    let empty = OsStr::new("");
    let cases: [(&str, Vec<(&str, &OsStr)>); 4] = [
        ("unset", vec![]),
        ("empty", vec![("HOME", empty)]),
        ("relative", vec![("HOME", relative)]),
        (
            "unset, XDG_DATA_HOME absolute",
            vec![("XDG_DATA_HOME", scratch.path().as_os_str())],
        ),
    ];
    for (what, env) in cases {
        for args in [&["index"][..], &["search", "stamina"], &["show", "R-12"]] {
            let run = spec_with(&root, args, &env);
            run.code(2);
            assert_eq!(run.stdout, "", "{what}: {args:?}");
            assert_eq!(
                run.stderr_lines().len(),
                1,
                "{what}: {args:?}\n{}",
                run.show()
            );
            assert!(
                run.stderr.starts_with("spec: ") && run.stderr.contains("HOME"),
                "{what}: {args:?}\n{}",
                run.show()
            );
            assert_eq!(
                snapshot(scratch.path()),
                before,
                "{what}: {args:?}: nothing created"
            );
        }
    }
}

/// A `HOME` inside the project root puts the data directory under it:
/// exit 2, nothing created (for both fixtures, also through a symlinked
/// `HOME` that resolves into the root).
#[test]
fn home_inside_the_root_is_exit_2_with_nothing_created() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("location-inside");
        let root = scratch.copy(fixture, "copy");
        let link = scratch.path().join("link-into-copy");
        std::os::unix::fs::symlink(root.join("docs"), &link).expect("symlink");
        let before = snapshot(scratch.path());
        for home in [
            root.clone(),
            root.join("docs"),
            root.join("not-yet"),
            link.clone(),
        ] {
            for args in [
                &["index"][..],
                &["search", "anything"],
                &["show", "docs/x.md"],
            ] {
                let run = spec(&home, &root, args);
                run.code(2);
                assert_eq!(
                    run.stdout,
                    "",
                    "{fixture}: HOME {}: {args:?}",
                    home.display()
                );
                assert!(
                    run.stderr.starts_with("spec: ")
                        && run.stderr.contains("inside the project root"),
                    "{fixture}: HOME {}: {args:?}\n{}",
                    home.display(),
                    run.show()
                );
                assert_eq!(
                    snapshot(scratch.path()),
                    before,
                    "{fixture}: HOME {}: {args:?}: nothing created",
                    home.display()
                );
            }
        }
    }
}

/// The host rule: on macOS `XDG_DATA_HOME` is not read; elsewhere an
/// absolute one wins and a relative one is ignored.
#[test]
fn the_host_rule_places_the_database() {
    let scratch = Scratch::new("location-host");
    let home = scratch.home("h");
    let xdg = scratch.dir("xdg");
    let root = scratch.copy("spec-a", "copy");
    let run = spec_with(
        &root,
        &["index"],
        &[
            ("HOME", home.as_os_str()),
            ("XDG_DATA_HOME", xdg.as_os_str()),
        ],
    );
    run.code(0);
    if cfg!(target_os = "macos") {
        assert!(
            snapshot(&xdg).is_empty(),
            "XDG_DATA_HOME is not read on macOS"
        );
        assert_only(&home, &["lantern-keep"]);
    } else {
        assert!(snapshot(&home).is_empty(), "an absolute XDG_DATA_HOME wins");
        assert!(xdg.join("specengine").join("lantern-keep.db").is_file());
    }
    let relative = spec_with(
        &root,
        &["index"],
        &[
            ("HOME", home.as_os_str()),
            ("XDG_DATA_HOME", OsStr::new("rel")),
        ],
    );
    relative.code(0);
    assert!(
        !root.join("rel").exists(),
        "a relative XDG_DATA_HOME is never used"
    );
    assert_only(&home, &["lantern-keep"]);
}
