//! AC-04 of docs/features/spec-cli.md: a bad `specengine.toml` is exit 2
//! with one stderr line `specengine.toml:<line>: message` at the right line,
//! nothing on stdout, and no data directory created. Both fixtures' own
//! `[ids]` follow each case (AC-17).

#![cfg(unix)]

mod common;

use common::{FIXTURES, Scratch, data_dir, read_text, snapshot, spec, write};

/// The fixture's config from its `[ids]` table on (its `[project]` and
/// `[paths]` dropped), so a case can put its own tables first.
fn ids_table(root: &std::path::Path) -> String {
    let text = read_text(root, "specengine.toml");
    let start = text.find("\n[ids]\n").expect("an [ids] table") + 1;
    let rest = &text[start..];
    // spec-b's `[project]` follows its `[ids]`: keep `[ids]` alone.
    match rest.find("\n[project]") {
        Some(end) => rest[..end + 1].to_owned(),
        None => rest.to_owned(),
    }
}

/// `(what, config head, line)`: the head goes first in the file, so the
/// error line is counted in it. A bad `[ids]` prefix is its own case below.
fn cases() -> Vec<(&'static str, String, usize)> {
    let long = format!("a{}", "b".repeat(64));
    assert_eq!(long.len(), 65);
    let mut cases = vec![
        (
            "unknown [project] key",
            "[project]\nslug = \"ok\"\nowner = \"someone\"\n".to_owned(),
            3,
        ),
        (
            "wrong type of name",
            "[project]\nslug = \"ok\"\nname = 7\n".to_owned(),
            3,
        ),
        ("slug 3", "[project]\nslug = 3\n".to_owned(), 2),
        (
            "slug 65 characters",
            format!("# a comment\n[project]\nslug = \"{long}\"\n"),
            3,
        ),
        (
            "absolute [paths] root",
            "[project]\nslug = \"ok\"\n\n[paths]\nroots = [\"/abs\"]\n".to_owned(),
            5,
        ),
        (
            "unknown table",
            "[project]\nslug = \"ok\"\n\n[path]\nroots = [\"docs\"]\n".to_owned(),
            4,
        ),
        (
            "unknown top-level key",
            "slugg = \"ok\"\n[project]\nslug = \"ok\"\n".to_owned(),
            1,
        ),
    ];
    for bad in ["Lantern", "1x", "a/b", "../x", ""] {
        cases.push((
            "bad slug",
            format!("[project]\nname = \"n\"\nslug = \"{bad}\"\n"),
            3,
        ));
    }
    cases
}

fn assert_refused(
    home: &std::path::Path,
    root: &std::path::Path,
    args: &[&str],
    line: usize,
    context: &str,
) {
    let run = spec(home, root, args);
    run.code(2);
    assert_eq!(run.stdout, "", "{context}: {args:?}");
    let lines = run.stderr_lines();
    assert_eq!(
        lines.len(),
        1,
        "{context}: {args:?}: one stderr line\n{}",
        run.show()
    );
    let prefix = format!("specengine.toml:{line}: ");
    assert!(
        lines[0].starts_with(&prefix) && lines[0].len() > prefix.len(),
        "{context}: {args:?}: want {prefix:?}…\n{}",
        run.show()
    );
    assert!(
        !data_dir(home).exists() && snapshot(home).is_empty(),
        "{context}: {args:?}: something was created under HOME: {:?}",
        snapshot(home).keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_bad_config_is_exit_2_at_its_line_with_no_data_directory() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("config");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let ids = ids_table(&root);
        for (what, head, line) in cases() {
            write(&root, "specengine.toml", format!("{head}\n{ids}"));
            for args in [
                &["index"][..],
                &["search", "anything"],
                &["show", "X-1"],
                &["--json", "index"],
            ] {
                assert_refused(
                    &home,
                    &root,
                    args,
                    line,
                    &format!("{fixture}: {what}: {head:?}"),
                );
            }
        }
        // A bad `[ids]` prefix: a lower-case prefix, at its own line.
        let head = "[project]\nslug = \"ok\"\n";
        let ids_line = head.lines().count() + 3;
        let bad_ids = ids.replacen(
            "[ids]\n",
            "[ids]\nlow = { kind = \"requirement\", width = 2 }\n",
            1,
        );
        write(&root, "specengine.toml", format!("{head}\n{bad_ids}"));
        assert_refused(
            &home,
            &root,
            &["index"],
            ids_line,
            &format!("{fixture}: bad [ids] prefix"),
        );
    }
}

/// A missing slug is an error only for a database command, at the
/// `[project]` line, else line 1.
#[test]
fn no_slug_is_exit_2_for_index_search_and_show_at_the_project_line() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("config-noslug");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let ids = ids_table(&root);
        for (head, line) in [
            ("# no slug\n\n[project]\nname = \"n\"\n", 3),
            ("# no project table at all\n", 1),
        ] {
            write(&root, "specengine.toml", format!("{head}\n{ids}"));
            for args in [
                &["index"][..],
                &["index", "--full"],
                &["search", "anything"],
                &["show", "X-1"],
                &["--json", "search", "anything"],
                &["--json", "show", "X-1"],
            ] {
                assert_refused(&home, &root, args, line, &format!("{fixture}: {head:?}"));
            }
        }
    }
}

/// The 64-byte boundary: a 64-byte slug is kept and names the database.
#[test]
fn a_64_byte_slug_is_accepted_and_names_the_database() {
    let scratch = Scratch::new("config-64");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let slug = format!("a{}", "9".repeat(63));
    let ids = ids_table(&root);
    write(
        &root,
        "specengine.toml",
        format!("[project]\nslug = \"{slug}\"\n\n{ids}"),
    );
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    assert!(
        run.stdout.starts_with(&format!("indexed {slug}: ")),
        "{}",
        run.show()
    );
    assert!(data_dir(&home).join(format!("{slug}.db")).is_file());
}

/// The name-checked tables are accepted whatever they hold; the data
/// example of the spec (slug, name, language) loads.
#[test]
fn known_tables_and_the_data_example_are_accepted() {
    let scratch = Scratch::new("config-known");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let ids = ids_table(&root);
    let head = "[project]\nslug = \"lantern-keep\"\nname = \"Lantern Keep\"\nlanguage = \"en\"\n\n\
                [budgets]\nx = 1\n[classes]\ny = \"z\"\n[check]\n[generators]\n[zones]\n[gate]\n[code]\nw = [1]\n";
    write(&root, "specengine.toml", format!("{head}\n{ids}"));
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
}
