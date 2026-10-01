//! AC-15 of docs/features/spec-cli.md (`docs/canon/architecture.md#apply`,
//! `#storage`): `index`, `index --full`, `search` and every `show` form
//! leave the spec-a and spec-b copies byte- and path-identical; new files
//! appear only in the data directory.

#![cfg(unix)]

mod common;

use common::{FIXTURES, Scratch, data_dir, snapshot, spec};

/// The commands of the read loop over `fixture`: every AC-10 form of it,
/// the dangling and refused ones included.
fn commands(fixture: &str) -> Vec<Vec<&'static str>> {
    let mut commands: Vec<Vec<&'static str>> = vec![
        vec!["index"],
        vec!["index", "--full"],
        vec!["--json", "index"],
        vec!["search", "stamina"],
        vec!["search", "sync", "--archive", "--limit", "200"],
        vec!["--json", "search", "stamina", "--kind", "rule"],
        vec!["search", "ab"],
        vec!["show", "docs/nope.md"],
        vec!["show", "../x.md"],
        vec!["show", "other:R-12"],
    ];
    let forms: &[&'static str] = if fixture == "spec-a" {
        &[
            "TERM-tired",
            "QST-031",
            "stamina-tuning/AC-07",
            "MEC-STAMINA#RULE-STAM-REGEN",
            "R-12@3",
            "AC-07",
            "R-99",
            "FOO-1",
            "\u{0410}-101",
            "DEC-0007",
            "docs/spec/game.md",
        ]
    } else {
        &[
            "\u{0422}\u{0420}\u{0411}-001",
            "\u{0412}\u{041e}\u{041f}-07",
            "dry-run/CRIT-01",
            "CRIT-01",
            "MOD-CLI#CMD-SYNC",
            "REQ-001@2",
            "REQ-999",
            "ADR-0002",
            "docs/spec/cli.md",
        ]
    };
    for form in forms {
        commands.push(vec!["show", form]);
        commands.push(vec!["--json", "show", form]);
    }
    commands
}

#[test]
fn the_read_loop_leaves_the_copies_untouched() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("read-only");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let below = root.join("docs");
        let outside = scratch.dir("outside");
        let copy_before = snapshot(&root);
        let outside_before = snapshot(&outside);
        for command in commands(fixture) {
            for cwd in [&root, &below] {
                let run = spec(&home, cwd, &command);
                assert!(run.code <= 2, "{fixture}: {command:?}: {}", run.show());
                assert_eq!(
                    snapshot(&root),
                    copy_before,
                    "{fixture}: {command:?} from {} changed the copy",
                    cwd.display()
                );
            }
            // With --root from elsewhere, too.
            let mut rooted = vec!["--root", root.to_str().unwrap()];
            rooted.extend(&command);
            spec(&home, &outside, &rooted);
            assert_eq!(
                snapshot(&root),
                copy_before,
                "{fixture}: --root {command:?}"
            );
            assert_eq!(
                snapshot(&outside),
                outside_before,
                "{fixture}: --root {command:?}"
            );
        }
        // Under HOME: only the data directory and the slug's database files.
        let data = data_dir(&home);
        let data_relative = data
            .strip_prefix(&home)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for path in snapshot(&home).into_keys() {
            let inside = path == data_relative
                || data_relative.starts_with(&format!("{path}/"))
                || [".db", ".db-wal", ".db-shm"]
                    .iter()
                    .any(|suffix| path == format!("{data_relative}/{slug}{suffix}"));
            assert!(
                inside,
                "{fixture}: {path} under HOME is not the slug's database"
            );
        }
        // Nothing else appeared in the scratch but the copy, HOME and outside.
        let top: Vec<String> = snapshot(scratch.path())
            .into_keys()
            .filter(|path| !path.contains('/'))
            .collect();
        assert_eq!(top, ["copy", "homes", "outside"], "{fixture}");
    }
}

/// AC-06 of docs/features/spec-cli-check.md: `spec check` (each verdict)
/// and `spec export index` (written, `--stdout`) need no `HOME` and exit as
/// their verdicts say; with `HOME` set nothing is created under it; after
/// `check` the copy is byte- and path-identical, after `export index` only
/// `[paths] index` differs.
#[test]
fn check_and_export_need_no_home_and_write_only_the_index() {
    use common::check::{baseline_covering, index_path, library, registered};
    use common::{spec_with, write};

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("read-only-check");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let outside = scratch.dir("outside");
        let base = std::fs::read_to_string(root.join("specengine.toml")).unwrap();
        let index = index_path(fixture);
        write(
            &root,
            "specengine.toml",
            registered(&base, index, "gen-index", None),
        );
        let config = std::fs::read_to_string(root.join("specengine.toml")).unwrap();
        write(
            &outside,
            "cover.toml",
            baseline_covering(&library(&root), "2999-12-31"),
        );
        write(
            &outside,
            "observe.toml",
            format!("{config}\n[check]\nmode = \"observe\"\n"),
        );
        let before = snapshot(&root);
        let outside_before = snapshot(&outside);

        let checks: [(&[&str], i32); 6] = [
            (&["check"], 1),
            (&["--json", "check", "--debt"], 1),
            (&["check", "--baseline", "../outside/cover.toml"], 0),
            (&["--config", "../outside/observe.toml", "check"], 0),
            (&["check", "--baseline", "nope.toml"], 2),
            (&["--json", "check", "--baseline", "nope.toml"], 2),
        ];
        for (args, exit) in checks {
            for with_home in [false, true] {
                let run = if with_home {
                    spec(&home, &root, args)
                } else {
                    spec_with(&root, args, &[])
                };
                assert_eq!(
                    run.code,
                    exit,
                    "{fixture} {args:?} (HOME {with_home})\n{}",
                    run.show()
                );
                assert!(!run.stdout.is_empty(), "{fixture} {args:?}: a report");
                assert_eq!(
                    snapshot(&root),
                    before,
                    "{fixture} {args:?} changed the copy"
                );
            }
        }

        for with_home in [false, true] {
            let run = if with_home {
                spec(&home, &root, &["export", "index", "--stdout"])
            } else {
                spec_with(&root, &["export", "index", "--stdout"], &[])
            };
            run.code(0);
            assert_eq!(snapshot(&root), before, "{fixture}: --stdout wrote");
        }
        let run = spec_with(&root, &["export", "index"], &[]);
        run.code(0);
        let mut after = snapshot(&root);
        assert!(
            after.remove(index).is_some_and(|bytes| bytes.is_some()),
            "{fixture}: the index written"
        );
        assert_eq!(after, before, "{fixture}: only [paths] index differs");
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert!(run.stdout.starts_with("unchanged "), "{}", run.stdout);

        assert_eq!(snapshot(&outside), outside_before, "{fixture}: outside");
        assert!(
            snapshot(&home).is_empty(),
            "{fixture}: something under HOME"
        );
        let top: Vec<String> = snapshot(scratch.path())
            .into_keys()
            .filter(|path| !path.contains('/'))
            .collect();
        assert_eq!(top, ["copy", "homes", "outside"], "{fixture}");
    }
}

/// docs/features/spec-cli-graph.md AC-17: `tree`, `graph` and `show
/// --links` in every form (found, not found, refused, dangling, cycles)
/// leave the copies byte- and path-identical, from the root, from below
/// it and with `--root` from elsewhere; new files appear only as the
/// slug's database under `HOME`. M: a write under the root.
#[test]
fn graph_reads_leave_the_copies_untouched() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("read-only-graph");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let below = root.join("docs");
        let outside = scratch.dir("outside");
        let copy_before = snapshot(&root);
        let outside_before = snapshot(&outside);
        let refs: &[&str] = if fixture == "spec-a" {
            &[
                "MEC-STAMINA",
                "RULE-STAM-REGEN",
                "R-12",
                "docs/spec/game.md",
                "R-99",
                "\u{041c}\u{0415}\u{0421}-STAMINA",
                "other:R-12",
            ]
        } else {
            &[
                "MOD-CLI#CMD-SYNC",
                "REQ-002",
                "\u{0422}\u{0420}\u{0411}-001",
                "docs/spec/cli.md",
                "REQ-999",
                "R\u{0415}Q-002",
                "other:REQ-001",
            ]
        };
        let mut commands: Vec<Vec<&str>> = vec![
            vec!["tree"],
            vec!["--json", "tree", "--archive"],
            vec!["tree", "--depth", "1", "--kind", "rule"],
            vec!["tree", "--depth", "-1"],
        ];
        for reference in refs {
            commands.push(vec!["tree", reference]);
            commands.push(vec!["--json", "graph", reference, "--impact"]);
            commands.push(vec!["graph", reference, "--type", "mentions", "--archive"]);
            commands.push(vec!["show", reference, "--links"]);
            commands.push(vec!["--json", "show", reference, "--links", "--archive"]);
        }
        for command in &commands {
            for cwd in [&root, &below] {
                let run = spec(&home, cwd, command);
                assert!(run.code <= 2, "{fixture}: {command:?}: {}", run.show());
                assert_eq!(
                    snapshot(&root),
                    copy_before,
                    "{fixture}: {command:?} from {} changed the copy",
                    cwd.display()
                );
            }
            let mut rooted = vec!["--root", root.to_str().unwrap()];
            rooted.extend(command);
            spec(&home, &outside, &rooted);
            assert_eq!(
                snapshot(&root),
                copy_before,
                "{fixture}: --root {command:?}"
            );
            assert_eq!(
                snapshot(&outside),
                outside_before,
                "{fixture}: --root {command:?}"
            );
        }
        let data = data_dir(&home);
        let data_relative = data
            .strip_prefix(&home)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let under_home = snapshot(&home);
        assert!(
            under_home.contains_key(&format!("{data_relative}/{slug}.db")),
            "{fixture}: the slug's database: {:?}",
            under_home.keys().collect::<Vec<_>>()
        );
        for path in under_home.into_keys() {
            let inside = path == data_relative
                || data_relative.starts_with(&format!("{path}/"))
                || [".db", ".db-wal", ".db-shm"]
                    .iter()
                    .any(|suffix| path == format!("{data_relative}/{slug}{suffix}"));
            assert!(
                inside,
                "{fixture}: {path} under HOME is not the slug's database"
            );
        }
        let top: Vec<String> = snapshot(scratch.path())
            .into_keys()
            .filter(|path| !path.contains('/'))
            .collect();
        assert_eq!(top, ["copy", "homes", "outside"], "{fixture}");
    }
}
