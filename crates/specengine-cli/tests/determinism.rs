//! AC-16 of docs/features/spec-cli.md (07 §1.1): one database state gives
//! one stdout, byte for byte; nothing depends on rowid, insertion order,
//! time or the absolute root. Two copies of each fixture are written file
//! by file in opposite orders, each indexed after every file under its own
//! `HOME`, so their databases hold the rows in opposite insertion orders;
//! `show` and `search` then print the same bytes.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use common::{FIXTURES, Scratch, copy_dir, fixture, index, md_files, read, spec, write};

/// Extra files: a second holder of a feature-scoped ID (several holders)
/// and two identical records (a bm25 tie broken by path).
fn extras(name: &str) -> Vec<(String, String)> {
    let (feature_id, prefix) = if name == "spec-a" {
        ("AC-07", "docs/features")
    } else {
        ("CRIT-01", "docs/features")
    };
    vec![
        (
            format!("{prefix}/aa-second.md"),
            format!(
                "---\nclass: spec\nstatus: draft\n---\n\n# Second\n\n### Also {{#{feature_id}}}\n\nlumenstone twice.\n"
            ),
        ),
        (
            "docs/records/tie/t-1.md".to_owned(),
            "# Tiebreak\n\nlumenstone\n".to_owned(),
        ),
        (
            "docs/records/tie/t-2.md".to_owned(),
            "# Tiebreak\n\nlumenstone\n".to_owned(),
        ),
    ]
}

/// A copy of `name` at `<scratch>/<dir>` whose documents are written (and
/// indexed) one at a time in `order`.
fn written_in_order(
    scratch: &Scratch,
    name: &str,
    dir: &str,
    home: &Path,
    reverse: bool,
) -> std::path::PathBuf {
    let source = scratch.dir(&format!("source-{dir}"));
    copy_dir(&fixture(name), &source);
    for (path, text) in extras(name) {
        write(&source, &path, text);
    }
    let root = scratch.dir(dir);
    // The config (and anything but documents) first.
    write(&root, "specengine.toml", read(&source, "specengine.toml"));
    let mut files: Vec<String> = md_files(&source.join("docs"))
        .into_iter()
        .map(|file| format!("docs/{file}"))
        .collect();
    if reverse {
        files.reverse();
    }
    index(home, &root);
    for file in files {
        write(&root, &file, read(&source, &file));
        index(home, &root);
    }
    root
}

#[test]
fn opposite_insertion_orders_print_the_same_bytes() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new("determinism");
        let home_forward = scratch.home("forward");
        let home_reverse = scratch.home("reverse");
        let forward = written_in_order(&scratch, name, "forward", &home_forward, false);
        let reverse = written_in_order(&scratch, name, "reverse", &home_reverse, true);

        // Every ID of the corpus (the several-holders one included), by the
        // parser.
        let config = specengine_core::ProjectConfig::from_toml(
            &String::from_utf8(read(&forward, "specengine.toml")).unwrap(),
        )
        .unwrap();
        let mut references: BTreeSet<String> = BTreeSet::new();
        for file in md_files(&forward.join("docs")) {
            let path = format!("docs/{file}");
            let parsed = specengine_core::parse(&path, &read(&forward, &path), &config.scheme);
            references.extend(parsed.nodes.iter().filter_map(|node| node.id.clone()));
            references.insert(path);
        }
        let mut queries: Vec<Vec<&str>> = vec![
            vec!["search", "lumenstone"],
            vec!["search", "lumenstone", "--limit", "1"],
            vec!["search", "lumenstone", "--archive"],
        ];
        if name == "spec-a" {
            queries.push(vec!["search", "stamina", "sprint", "--limit", "200"]);
            queries.push(vec!["search", "regeneration", "--archive"]);
        } else {
            queries.push(vec!["search", "sync", "--limit", "200"]);
            queries.push(vec!["search", "dry-run", "--archive"]);
        }
        let mut compared = 0;
        let several = if name == "spec-a" { "AC-07" } else { "CRIT-01" };
        for reference in &references {
            for json in [false, true] {
                let mut args = vec!["show", reference.as_str()];
                if json {
                    args.insert(0, "--json");
                }
                let one = spec(&home_forward, &forward, &args);
                let two = spec(&home_reverse, &reverse, &args);
                assert_eq!(one.code, two.code, "{name}: {args:?}");
                assert_eq!(one.stdout, two.stdout, "{name}: {args:?}");
                assert_eq!(one.stderr, two.stderr, "{name}: {args:?}");
                if reference == several && !json {
                    assert_eq!(
                        one.stderr_lines().len(),
                        1,
                        "{name}: several holders\n{}",
                        one.show()
                    );
                }
                compared += 1;
            }
        }
        for query in &queries {
            for json in [false, true] {
                let mut args = query.clone();
                if json {
                    args.insert(0, "--json");
                }
                let one = spec(&home_forward, &forward, &args);
                let two = spec(&home_reverse, &reverse, &args);
                one.code(0);
                assert_eq!(one.stdout, two.stdout, "{name}: {args:?}");
                compared += 1;
            }
        }
        // The tie is broken by path.
        let tie = spec(&home_reverse, &reverse, &["--json", "search", "Tiebreak"]).json();
        let paths: Vec<&str> = tie["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths[..2],
            ["docs/records/tie/t-1.md", "docs/records/tie/t-2.md"],
            "{name}"
        );
        assert!(compared > 20, "{name}: {compared}");
        // A repeat of a whole index run is byte-identical too (no timing).
        let one = spec(&home_forward, &forward, &["index"]);
        let two = spec(&home_forward, &forward, &["index"]);
        assert_eq!(one.stdout, two.stdout, "{name}: index repeat");
    }
}

/// AC-07 of docs/features/spec-cli-check.md: `spec check` over two copies
/// at different absolute paths, their documents written in opposite
/// orders, gives byte-identical stdout and stderr, text and JSON, blocked
/// and cannot-check (a broken default baseline), from the root, from
/// below it and with `--root` from elsewhere; no absolute root is printed.
#[test]
fn check_prints_the_same_bytes_wherever_the_root_lies() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new("determinism-check");
        let home = scratch.home("h");
        let source = fixture(name);
        let mut files: Vec<String> = md_files(&source.join("docs"))
            .into_iter()
            .map(|file| format!("docs/{file}"))
            .collect();
        let one = scratch.dir("one");
        let two = scratch.dir("second/copy");
        for (root, reverse) in [(&one, false), (&two, true)] {
            write(root, "specengine.toml", read(&source, "specengine.toml"));
            if reverse {
                files.reverse();
            }
            for file in &files {
                write(root, file, read(&source, file));
            }
        }
        let elsewhere = scratch.dir("elsewhere");
        let variants: [&[&str]; 4] = [
            &["check"],
            &["check", "--debt"],
            &["--json", "check"],
            &["--json", "check", "--debt"],
        ];
        for broken in [false, true] {
            if broken {
                write(&one, ".spec-debt.toml", "[[debt]\n");
                write(&two, ".spec-debt.toml", "[[debt]\n");
            }
            let exit = if broken { 2 } else { 1 };
            for args in variants {
                let mut runs = Vec::new();
                for root in [&one, &two] {
                    runs.push(spec(&home, root, args));
                    runs.push(spec(&home, &root.join("docs/spec"), args));
                    let mut rooted = vec!["--root", root.to_str().unwrap()];
                    rooted.extend(args.iter().copied());
                    runs.push(spec(&home, &elsewhere, &rooted));
                }
                let first = &runs[0];
                assert_eq!(first.code, exit, "{name} {args:?}\n{}", first.show());
                for run in &runs[1..] {
                    assert_eq!(run.code, first.code, "{name} {args:?}");
                    assert_eq!(
                        run.stdout, first.stdout,
                        "{name} {args:?} (broken {broken})"
                    );
                    assert_eq!(
                        run.stderr, first.stderr,
                        "{name} {args:?} (broken {broken})"
                    );
                }
                for root in [&one, &two, &scratch.path().to_path_buf()] {
                    let shown = root.to_str().unwrap();
                    assert!(
                        !first.stdout.contains(shown) && !first.stderr.contains(shown),
                        "{name} {args:?}: {shown} printed\n{}",
                        first.show()
                    );
                }
                if broken {
                    let cause = first
                        .stdout
                        .lines()
                        .any(|line| line.starts_with("cannot  .spec-debt.toml"))
                        || first.stdout.contains("\"path\":\".spec-debt.toml");
                    assert!(cause, "{name} {args:?}: {}", first.stdout);
                }
            }
        }
    }
}

/// docs/features/spec-cli-graph.md AC-16: two copies of each fixture,
/// their documents written (and indexed) one at a time in opposite orders
/// at different roots under their own `HOME`, with a `parent:` cycle, a
/// dangling `parent:` and several holders: `tree`, `graph`, `graph
/// --impact` and `show --links` of every ID and document path, text and
/// JSON, print the same bytes (stdout, stderr, exit); no absolute root is
/// printed. M: `HashMap` or rowid order.
#[test]
fn graph_reads_print_the_same_bytes_in_opposite_orders() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new("determinism-graph");
        let home_forward = scratch.home("forward");
        let home_reverse = scratch.home("reverse");
        let forward = written_in_order(&scratch, name, "forward", &home_forward, false);
        let reverse = written_in_order(&scratch, name, "second/reverse", &home_reverse, true);
        for root in [&forward, &reverse] {
            if name == "spec-a" {
                common::replace(
                    root,
                    "docs/spec/movement/sprint.md",
                    "parent: DOM-MOVEMENT",
                    "parent: MEC-STAMINA",
                );
                common::replace(
                    root,
                    "docs/spec/movement/stamina.md",
                    "parent: DOM-MOVEMENT",
                    "parent: MEC-SPRINT",
                );
            } else {
                common::replace(
                    root,
                    "docs/spec/cli.md",
                    "tier: 1\n",
                    "tier: 1\nparent: MOD-CLI\n",
                );
            }
            write(
                root,
                "docs/spec/zz-dangling.md",
                "---\nid: DOM-ZZ\nclass: canon\nparent: DOM-NOWHERE\nlinks:\n  depends_on: [DOM-NOWHERE, other:R-1]\n---\n\n# Dangling\n",
            );
        }
        let config = specengine_core::ProjectConfig::from_toml(
            &String::from_utf8(read(&forward, "specengine.toml")).unwrap(),
        )
        .unwrap();
        let mut references: BTreeSet<String> = BTreeSet::new();
        for file in md_files(&forward.join("docs")) {
            let path = format!("docs/{file}");
            let parsed = specengine_core::parse(&path, &read(&forward, &path), &config.scheme);
            references.extend(parsed.nodes.iter().filter_map(|node| node.id.clone()));
            references.insert(path);
        }
        let mut commands: Vec<Vec<String>> = vec![
            vec!["tree".into()],
            vec!["tree".into(), "--archive".into()],
            vec![
                "tree".into(),
                "--kind".into(),
                "rule".into(),
                "--kind".into(),
                "command".into(),
            ],
        ];
        for reference in &references {
            for command in [
                &["tree"][..],
                &["graph"],
                &["graph", "--impact"],
                &["graph", "--type", "mentions", "--archive"],
                &["show", "--links"],
            ] {
                let mut args: Vec<String> = vec![command[0].into(), reference.clone()];
                args.extend(command[1..].iter().map(|arg| (*arg).to_owned()));
                commands.push(args);
            }
        }
        let mut compared = 0;
        for command in &commands {
            for json in [false, true] {
                let mut args: Vec<&str> = command.iter().map(String::as_str).collect();
                if json {
                    args.insert(0, "--json");
                }
                let one = spec(&home_forward, &forward, &args);
                let two = spec(&home_reverse, &reverse, &args);
                assert!(one.code <= 1, "{name}: {args:?}\n{}", one.show());
                assert_eq!(one.code, two.code, "{name}: {args:?}");
                assert_eq!(one.stdout, two.stdout, "{name}: {args:?}");
                assert_eq!(one.stderr, two.stderr, "{name}: {args:?}");
                for root in [&forward, &reverse] {
                    let shown = root.to_str().unwrap();
                    assert!(
                        !one.stdout.contains(shown) && !one.stderr.contains(shown),
                        "{name}: {args:?} printed {shown}"
                    );
                }
                compared += 1;
            }
        }
        assert!(compared > 200, "{name}: {compared}");
        // A repeat is byte-identical, too.
        let one = spec(&home_forward, &forward, &["--json", "graph", "DOM-ZZ"]);
        let two = spec(&home_forward, &forward, &["--json", "graph", "DOM-ZZ"]);
        assert_eq!(one.stdout, two.stdout, "{name}");
    }
}
