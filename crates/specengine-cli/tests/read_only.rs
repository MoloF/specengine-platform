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
