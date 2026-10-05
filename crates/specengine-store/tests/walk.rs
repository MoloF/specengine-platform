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
use specengine_core::{Paths, WalkScope, is_clean_relative, is_under};
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

/// AC-14 of docs/features/phase1-cleanup.md (S4): `read` refuses a symlink
/// in any component below the root, as `probe` does: a directory listed
/// once and then swapped for a symlink to a copy of itself is no way in.
#[test]
fn a_listed_directory_swapped_for_a_symlink_is_neither_read_nor_probed() {
    let scratch = Scratch::new("walk-swapped-dir");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let tree = corpus.tree();
    let listing = tree.list().expect("list");
    let under: Vec<String> = listing
        .paths
        .iter()
        .filter(|path| path.starts_with("docs/records/Q/"))
        .cloned()
        .collect();
    assert!(!under.is_empty(), "spec-a lists files under docs/records/Q");
    for path in &under {
        assert!(tree.read(path).is_ok(), "{path}: readable before the swap");
        assert!(tree.probe(path), "{path}: probed before the swap");
    }
    // The same bytes, one symlink away: a copy outside the worktree.
    let outside = scratch.join("outside-q");
    common::copy_dir(&corpus.root.join("docs/records/Q"), &outside);
    std::fs::remove_dir_all(corpus.root.join("docs/records/Q")).expect("remove Q");
    symlink(&outside, corpus.root.join("docs/records/Q")).expect("symlink Q");
    for path in &under {
        let target = outside.join(path.strip_prefix("docs/records/Q/").unwrap());
        assert!(target.is_file(), "{}: the copy exists", target.display());
        assert!(
            tree.read(path).is_err(),
            "{path}: read through a symlinked directory"
        );
        assert!(!tree.probe(path), "{path}: probed through a symlink");
    }
    // A symlinked file (the last component) is refused as before.
    let file = listing
        .paths
        .iter()
        .find(|path| path.starts_with("docs/records/R/"))
        .expect("a file under docs/records/R")
        .as_str();
    let copy = scratch.join("outside-r.md");
    common::copy_file(corpus.root.join(file), &copy);
    std::fs::remove_file(corpus.root.join(file)).expect("remove");
    symlink(&copy, corpus.root.join(file)).expect("symlink");
    assert!(tree.read(file).is_err(), "a symlinked file is not read");
    assert!(!tree.probe(file));
}

/// AC-06 of docs/features/spec-check-links.md: the walk and the check's
/// walk scope share one matcher. For every candidate of the walk rules,
/// `Paths::in_walk_scope` agrees with the listing, but for what the pure
/// predicate cannot see (a symlink, a missing root: accepted, Rule 9); the
/// exclude globs live in core only, the store keeps no second matcher.
#[test]
fn the_walk_and_the_link_scope_share_one_matcher() {
    let scratch = Scratch::new("walk-scope");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write(
        "specengine.toml",
        format!(
            "{}\n[paths]\nroots = [\"docs/spec\", \"docs/records/\", \"CLAUDE.md\", \"docs/gone\"]\n\
             exclude = [\"docs/spec/drafts/**\", \"**/*.draft.md\", \"docs/spec/?.md\"]\n",
            corpus.read_text("specengine.toml")
        ),
    );
    let corpus = Corpus::load(&corpus.root);
    let written = [
        "CLAUDE.md",
        "docs/spec/deep/er/nested.md",
        "docs/spec/x.md",
        "docs/spec/xy.md",
        "README.md",
        "notes/outside.md",
        "docs/features/outside-now.md",
        "docs/spec/drafts/excluded.md",
        "docs/spec/drafts/deeper/also.md",
        "docs/records/R/R-99.draft.md",
        "docs/spec/.hidden/in-dot-dir.md",
        "docs/spec/.dot-file.md",
        "docs/spec/notes.txt",
        "docs/spec/rule.md.bak",
        "docs/spec/UPPER.MD",
    ];
    for path in written {
        corpus.write(path, format!("# {path}\n\nText.\n"));
    }
    symlink(
        corpus.root.join("docs/spec/game.md"),
        corpus.root.join("docs/spec/link-to-game.md"),
    )
    .unwrap();
    let listing = corpus.listing();
    let tree = corpus.tree();
    let mut candidates: Vec<String> = listing.paths.clone();
    candidates.extend(written.iter().map(|path| (*path).to_owned()));
    candidates.extend(
        [
            "docs/gone/x.md",
            "docs/spec/../spec/game.md",
            "/docs/spec/game.md",
            "docs//spec/game.md",
            "docs/spec",
            "",
        ]
        .map(str::to_owned),
    );
    // The compiled scope: once per `[paths]`, equal however it is built,
    // its roots the configured ones in order.
    let scope = corpus.paths.walk_scope();
    assert_eq!(scope, WalkScope::new(&corpus.paths));
    assert_eq!(scope.roots(), corpus.paths.roots.as_slice());
    for path in &candidates {
        let listed = listing.paths.contains(path);
        let invisible = path.starts_with("docs/gone/");
        assert_eq!(
            scope.in_walk_scope(path),
            listed || invisible,
            "WalkScope::in_walk_scope({path:?}) disagrees with the walk"
        );
        assert_eq!(
            corpus.paths.in_walk_scope(path),
            scope.in_walk_scope(path),
            "Paths::in_walk_scope({path:?}) disagrees with WalkScope"
        );
        assert_eq!(
            corpus.paths.is_excluded(path),
            scope.is_excluded(path),
            "Paths::is_excluded({path:?}) disagrees with WalkScope"
        );
        assert_eq!(
            tree.probe(path),
            listed,
            "probe({path:?}) disagrees with the walk"
        );
        if scope.in_walk_scope(path) {
            assert!(is_clean_relative(path), "{path:?}");
            assert!(
                scope
                    .roots()
                    .iter()
                    .any(|root| path == root || is_under(path, root)),
                "{path:?} lies in a root"
            );
        }
    }
    for (path, dir, under) in [
        ("docs/spec/game.md", "docs/spec", true),
        ("docs/spec", "docs/spec", false),
        ("docs/specs/x.md", "docs/spec", false),
        ("docs/spec/deep/x.md", "docs", true),
    ] {
        assert_eq!(is_under(path, dir), under, "is_under({path:?}, {dir:?})");
    }
    for (path, clean) in [
        ("docs/spec/game.md", true),
        ("", false),
        ("/docs/x.md", false),
        ("docs//x.md", false),
        ("docs/./x.md", false),
        ("docs/../x.md", false),
        ("docs/x.md/", false),
    ] {
        assert_eq!(
            is_clean_relative(path),
            clean,
            "is_clean_relative({path:?})"
        );
    }
    // What the predicate cannot see: a symlinked `.md` in scope.
    assert!(
        !listing
            .paths
            .iter()
            .any(|p| p == "docs/spec/link-to-game.md")
    );
    assert!(corpus.paths.in_walk_scope("docs/spec/link-to-game.md"));
    // The exclude globs are core's matcher.
    for (path, excluded) in [
        ("docs/spec/drafts/excluded.md", true),
        ("docs/spec/drafts/deeper/also.md", true),
        ("docs/records/R/R-99.draft.md", true),
        ("docs/spec/x.md", true),
        ("docs/spec/xy.md", false),
        ("docs/spec/game.md", false),
    ] {
        assert_eq!(scope.is_excluded(path), excluded, "{path}");
        assert_eq!(corpus.paths.is_excluded(path), excluded, "{path}");
        assert_eq!(listing.paths.iter().any(|p| p == path), !excluded, "{path}");
    }
    // No second matcher in the store.
    let src = common::repository_root().join("crates/specengine-store/src");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&src).expect("store src") {
        let path = entry.unwrap().path();
        if path.file_name().is_some_and(|name| name == "glob.rs") {
            offenders.push(path.display().to_string());
        }
        if path.extension().is_some_and(|ext| ext == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            for needle in ["struct Glob", "Glob::new", "mod glob", "enum Token"] {
                if text.contains(needle) {
                    offenders.push(format!("{}: {needle}", path.display()));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

/// docs/features/spec-check-links.md (iteration 3): the walker's `.md` rule
/// is core's `DOCUMENT_EXTENSION`. No store source outside its
/// `#[cfg(test)]` module has a `.md` string literal of its own (prose
/// quoting `` `.md` `` aside), and the walker names the constant.
#[test]
fn the_walker_uses_the_core_document_extension() {
    assert_eq!(specengine_core::DOCUMENT_EXTENSION, ".md");
    let src = common::repository_root().join("crates/specengine-store/src");
    let mut offenders = Vec::new();
    let mut sources = 0;
    for entry in std::fs::read_dir(&src).expect("store src") {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        sources += 1;
        let text = std::fs::read_to_string(&path).unwrap();
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            if line.trim_start().starts_with("//") {
                continue;
            }
            for literal in line.split('"').skip(1).step_by(2) {
                if literal.replace("`.md`", "").contains(".md") {
                    offenders.push(format!("{}:{}: {literal}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(sources >= 5, "{sources} store sources");
    assert!(offenders.is_empty(), "{offenders:#?}");
    let walker = std::fs::read_to_string(src.join("source.rs")).expect("source.rs");
    assert!(
        walker.matches("DOCUMENT_EXTENSION").count() >= 3,
        "the walker's file, root and probe rules name the constant"
    );
}

/// Iteration 3 of docs/features/spec-cli-check.md: a directory on the way
/// to a root that cannot be listed — `docs` at mode 000, or the root itself
/// listable but not searchable (mode 0444) — lands in `unreadable_dirs`
/// (root-relative), never in `missing_roots`, for default role roots
/// (spec-a) and written ones (spec-b, a root below `docs`); the readable
/// tree lists as before. Permissions are restored before the scratch goes.
#[test]
fn a_directory_on_the_way_that_cannot_be_listed_is_unreadable_not_missing() {
    for (name, written) in [
        ("spec-a", None),
        ("spec-b", Some("[\"docs/spec\", \"docs/records\"]")),
    ] {
        let scratch = Scratch::new("walk-locked-way");
        let mut corpus = Corpus::copy_of(name, &scratch, "wt");
        if let Some(roots) = written {
            let text = corpus.read_text("specengine.toml");
            corpus.write(
                "specengine.toml",
                text.replacen("roots = [\"docs\"]", &format!("roots = {roots}"), 1),
            );
            corpus.reload();
            assert!(corpus.paths.roots_written, "{name}");
        }
        let readable = corpus.listing();
        assert!(readable.unreadable_dirs.is_empty(), "{name}");
        assert!(!readable.paths.is_empty(), "{name}");

        common::chmod(&corpus.root, "docs", 0o000);
        if std::fs::read_dir(corpus.root.join("docs")).is_ok() {
            common::chmod(&corpus.root, "docs", 0o755);
            eprintln!("mode-000 directories are listable here (root?): case skipped");
            return;
        }
        let locked = corpus.listing();
        common::chmod(&corpus.root, "docs", 0o755);
        assert_eq!(locked.unreadable_dirs, ["docs"], "{name}: docs mode 000");
        assert!(
            locked.missing_roots.is_empty(),
            "{name}: {:?}",
            locked.missing_roots
        );
        assert!(locked.paths.is_empty(), "{name}");

        let tree = corpus.tree();
        common::chmod(&corpus.root, "", 0o444);
        let searched = tree.list();
        common::chmod(&corpus.root, "", 0o755);
        let searched = searched.expect("a listable root lists");
        assert_eq!(searched.unreadable_dirs, ["docs"], "{name}: root mode 0444");
        assert!(searched.missing_roots.is_empty(), "{name}");
        assert!(searched.paths.is_empty(), "{name}");

        assert_eq!(corpus.listing(), readable, "{name}: readable again");
    }
}

/// Iteration 3: a default role root that is genuinely absent stays ignored
/// — `missing_roots`, never `unreadable_dirs`, and no cause of the check:
/// spec-a has no `docs/archive`; with `docs/features` removed too, the
/// check still runs without a `cannot` cause.
#[test]
fn an_absent_default_root_stays_ignored() {
    let scratch = Scratch::new("walk-absent-default");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    std::fs::remove_dir_all(corpus.root.join("docs/features")).unwrap();
    let listing = corpus.listing();
    let mut missing = listing.missing_roots.clone();
    missing.sort();
    assert_eq!(missing, ["docs/archive", "docs/features"]);
    assert!(listing.unreadable_dirs.is_empty());
    let report = specengine_store::check_worktree(
        &corpus.root,
        &corpus.root.join("specengine.toml"),
        None,
        "2026-09-30",
    );
    assert!(report.cannot_check.is_empty(), "{:?}", report.cannot_check);
    assert_eq!(report.counts.documents, listing.paths.len());
}

/// Iteration 3: after `chmod 000 docs`, the incremental update removes
/// every row under it and reports `docs` unreadable; the database then
/// equals a rebuild (a fresh index) in the same state.
#[test]
fn an_update_after_a_directory_turns_unlistable_equals_a_rebuild() {
    let scratch = Scratch::new("walk-locked-update");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    let first = corpus.update(&mut index);
    assert!(first.walked > 0);
    common::chmod(&corpus.root, "docs", 0o000);
    if std::fs::read_dir(corpus.root.join("docs")).is_ok() {
        common::chmod(&corpus.root, "docs", 0o755);
        eprintln!("mode-000 directories are listable here (root?): case skipped");
        return;
    }
    let report = corpus.update(&mut index);
    let files = index.files().expect("files");
    let dump = index.dump().expect("dump");
    let fresh = corpus.fresh_dump(&scratch);
    common::chmod(&corpus.root, "docs", 0o755);
    assert_eq!(report.unreadable_dirs, ["docs"]);
    assert!(
        report.missing_roots.is_empty(),
        "{:?}",
        report.missing_roots
    );
    assert_eq!(report.removed, first.walked);
    assert!(files.is_empty(), "{files:?}");
    assert!(
        dump == fresh,
        "the update differs from a rebuild in the same state"
    );
}
