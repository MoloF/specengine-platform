//! AC-16 of docs/features/spec-index.md: the walk. spec-a is walked by the
//! `[paths]` defaults, spec-b by its `roots`; files outside the roots,
//! excluded, under a dot-directory, dot-files, symlinks and non-`.md` files
//! are not indexed; a single-file root is; `probe` agrees with `list`; an
//! unknown `[paths]` key, a `..` or an absolute root is an error naming its
//! line.

#![cfg(unix)]

mod common;

use std::os::unix::fs::symlink;

use common::{Corpus, Scratch, md_files_under};
use specengine_core::Paths;
use specengine_store::{Source, SpecIndex, WorkingTree};

#[test]
fn spec_a_is_walked_by_the_defaults_and_spec_b_by_its_roots() {
    let scratch = Scratch::new("walk-fixtures");
    let a = Corpus::copy_of("spec-a", &scratch, "spec-a");
    assert_eq!(a.paths, Paths::default(), "spec-a has no [paths]");
    assert_eq!(
        a.paths.roots,
        ["docs/spec", "docs/records", "docs/features", "docs/archive"]
    );
    let listing = a.listing();
    let mut expected: Vec<String> = ["docs/spec", "docs/records", "docs/features"]
        .iter()
        .flat_map(|dir| md_files_under(&a.root, dir))
        .collect();
    expected.sort();
    assert_eq!(listing.paths, expected, "spec-a by the defaults");
    assert_eq!(
        listing.missing_roots,
        ["docs/archive"],
        "spec-a has no archive"
    );

    let b = Corpus::copy_of("spec-b", &scratch, "spec-b");
    assert_eq!(b.paths.roots, ["docs"], "spec-b's roots");
    let listing = b.listing();
    assert_eq!(
        listing.paths,
        md_files_under(&b.root, "docs"),
        "spec-b by roots"
    );
    assert!(listing.missing_roots.is_empty());
    for corpus in [&a, &b] {
        let paths = corpus.listing().paths;
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted, "byte-sorted");
        assert!(
            paths
                .iter()
                .all(|path| !path.starts_with('/') && !path.contains('\\'))
        );
    }
}

#[test]
fn only_md_files_under_the_roots_are_walked() {
    let scratch = Scratch::new("walk-rules");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write(
        "specengine.toml",
        format!(
            "{}\n[paths]\nroots = [\"docs/spec\", \"docs/records/\", \"CLAUDE.md\", \"docs/gone\"]\n\
             exclude = [\"docs/spec/drafts/**\", \"**/*.draft.md\"]\n",
            corpus.read_text("specengine.toml")
        ),
    );
    let corpus = Corpus::load(&corpus.root);
    assert_eq!(
        corpus.paths.roots,
        ["docs/spec", "docs/records", "CLAUDE.md", "docs/gone"],
        "one trailing / dropped"
    );
    let indexed_extra = ["CLAUDE.md", "docs/spec/deep/er/nested.md"];
    let not_indexed = [
        "README.md",                       // outside the roots
        "notes/outside.md",                // outside the roots
        "docs/features/outside-now.md",    // a default root, not configured
        "docs/spec/drafts/excluded.md",    // excluded by `**`
        "docs/spec/drafts/deeper/also.md", // excluded by `**`
        "docs/records/R/R-99.draft.md",    // excluded by `**/*.draft.md`
        "docs/spec/.hidden/in-dot-dir.md", // under a dot-directory
        "docs/spec/.dot-file.md",          // a dot-file
        "docs/spec/notes.txt",             // not .md
        "docs/spec/rule.md.bak",           // not ending in .md
        "docs/spec/UPPER.MD",              // not exactly .md
        "docs/spec/mdfile",                // no extension
    ];
    for path in indexed_extra.iter().chain(not_indexed.iter()) {
        corpus.write(path, format!("# {path}\n\nText.\n"));
    }
    // Symlinks: to a file inside a root, to a file outside, to a directory.
    symlink(
        corpus.root.join("docs/spec/game.md"),
        corpus.root.join("docs/spec/link-to-game.md"),
    )
    .unwrap();
    symlink(
        corpus.root.join("notes/outside.md"),
        corpus.root.join("docs/spec/link-outside.md"),
    )
    .unwrap();
    symlink(
        corpus.root.join("notes"),
        corpus.root.join("docs/spec/linked-dir"),
    )
    .unwrap();
    let symlinks = [
        "docs/spec/link-to-game.md",
        "docs/spec/link-outside.md",
        "docs/spec/linked-dir/outside.md",
    ];
    // A directory whose name ends in .md is not a file.
    std::fs::create_dir_all(corpus.root.join("docs/spec/folder.md")).unwrap();

    let listing = corpus.listing();
    let mut expected: Vec<String> = md_files_under(&corpus.root, "docs/spec")
        .into_iter()
        .chain(md_files_under(&corpus.root, "docs/records"))
        .filter(|path| !not_indexed.contains(&path.as_str()))
        .chain(["CLAUDE.md".to_owned()])
        .collect();
    expected.sort();
    assert_eq!(listing.paths, expected, "the walk");
    for path in indexed_extra {
        assert!(listing.paths.contains(&path.to_owned()), "{path} is walked");
    }
    for path in not_indexed
        .iter()
        .chain(symlinks.iter())
        .chain(["docs/spec/folder.md"].iter())
    {
        assert!(
            !listing.paths.contains(&(*path).to_owned()),
            "{path} is not walked"
        );
    }
    assert_eq!(
        listing.missing_roots,
        ["docs/gone"],
        "a missing root is reported"
    );

    // `probe` agrees with `list` for every candidate.
    let tree = corpus.tree();
    for path in listing
        .paths
        .iter()
        .map(String::as_str)
        .chain(not_indexed)
        .chain(symlinks)
        .chain([
            "docs/spec/folder.md",
            "docs/spec/../spec/game.md",
            "/docs/spec/game.md",
            "",
        ])
    {
        assert_eq!(
            tree.probe(path),
            listing.paths.iter().any(|listed| listed == path),
            "probe({path:?}) disagrees with list"
        );
    }

    // The index stores exactly the walk.
    let mut index = corpus.open(&scratch.db("index"));
    let report = corpus.update(&mut index);
    assert_eq!(index.files().expect("files"), listing.paths);
    assert_eq!(report.missing_roots, ["docs/gone"]);
}

#[test]
fn a_root_that_is_a_symlink_is_reported_missing() {
    let scratch = Scratch::new("walk-symlinked-root");
    let corpus = Corpus::copy_of("spec-b", &scratch, "wt");
    std::fs::create_dir_all(scratch.join("elsewhere")).unwrap();
    std::fs::write(scratch.join("elsewhere/x.md"), "# X\n").unwrap();
    symlink(scratch.join("elsewhere"), corpus.root.join("linked")).unwrap();
    let paths = Paths::from_toml("[paths]\nroots = [\"docs\", \"linked\"]\n").expect("paths");
    let tree = WorkingTree::new(&corpus.root, &paths).expect("tree");
    let listing = tree.list().expect("list");
    assert_eq!(listing.missing_roots, ["linked"]);
    assert!(listing.paths.iter().all(|path| path.starts_with("docs/")));
}

#[test]
fn bad_paths_tables_are_errors_naming_their_line() {
    let cases = [
        (
            "[paths]\nroots = [\"docs\"]\nsurprise = 1\n",
            3,
            "unknown key",
        ),
        (
            "[project]\nname = \"x\"\n\n[paths]\nroots = [\"docs\", \"../other\"]\n",
            5,
            "a .. root",
        ),
        (
            "[paths]\nroots = [\n  \"docs\",\n  \"/etc\",\n]\n",
            4,
            "an absolute root",
        ),
        ("[paths]\nspec = \"docs/../spec\"\n", 2, "a .. role key"),
        ("[paths]\nexclude = [\"../x/**\"]\n", 2, "a .. exclude glob"),
        ("[paths]\n\nroots = \"docs\"\n", 3, "a wrong type"),
    ];
    for (text, line, context) in cases {
        let error = Paths::from_toml(text).expect_err(context);
        assert_eq!(error.line, Some(line), "{context}: {error:?}");
        let located = error.at("specengine.toml");
        assert!(
            located.starts_with(&format!("specengine.toml:{line}: ")),
            "{context}: {located}"
        );
    }
    // Other tables are ignored; no [paths] gives the defaults.
    assert_eq!(
        Paths::from_toml("[ids]\nR = { kind = \"r\", width = 2 }\n[budgets]\nx = 1\n").unwrap(),
        Paths::default()
    );
}

/// A directory below a root that cannot be listed is reported; the rest of
/// the walk goes on and the update never fails (ADR-0012). Non-UTF-8 names
/// (`skipped_names`) cannot be created on APFS (EILSEQ), so that case is
/// not exercised here.
#[test]
fn an_unreadable_directory_is_reported_and_the_rest_is_walked() {
    let scratch = Scratch::new("walk-unreadable-dir");
    let corpus = Corpus::copy_of("spec-b", &scratch, "wt");
    corpus.write("docs/spec/locked/inside.md", "# Inside\n");
    common::chmod(&corpus.root, "docs/spec/locked", 0o000);
    if std::fs::read_dir(corpus.root.join("docs/spec/locked")).is_ok() {
        eprintln!("mode-000 directories are listable here (root?): case skipped");
        return;
    }
    let listing = corpus.listing();
    assert_eq!(listing.unreadable_dirs, ["docs/spec/locked"]);
    assert!(listing.paths.contains(&"docs/spec/cli.md".to_owned()));
    assert!(
        !listing
            .paths
            .iter()
            .any(|path| path.starts_with("docs/spec/locked/"))
    );
    let mut index = corpus.open(&scratch.db("index"));
    let report = corpus.update(&mut index);
    assert_eq!(report.unreadable_dirs, ["docs/spec/locked"]);
    assert_eq!(index.files().expect("files"), listing.paths);
    common::chmod(&corpus.root, "docs/spec/locked", 0o755);
}
