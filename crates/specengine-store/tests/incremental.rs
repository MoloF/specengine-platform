//! AC-07 and AC-08 of docs/features/spec-index.md: after every step of an
//! edit script over a scratch copy of spec-a (edit a section, add, delete,
//! rename keeping the bytes, touch an mtime, add then remove an `[ids]`
//! prefix, drop a root) the incrementally updated index dumps exactly as a
//! fresh index of the same tree; the cache key is `(path, BLAKE3)` plus the
//! `[ids]` fingerprint: nothing is re-parsed after no change, an mtime
//! touch, a `[budgets]` edit or an `[ids]` comment, one file after one edit,
//! and `update_paths` of that file gives the walk's dump.

#![cfg(unix)]

mod common;

use common::{ADDED_FILE, Corpus, Scratch, ac07_script, assert_equals_fresh, touch};
use specengine_store::{IndexWriter, SpecIndex};

#[test]
fn after_every_step_of_the_edit_script_the_index_equals_a_fresh_one() {
    let scratch = Scratch::new("incremental");
    let mut corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    assert_equals_fresh(&index, &corpus, &scratch, "the first index");
    for (step, edit) in ac07_script() {
        edit(&mut corpus);
        let report = corpus.update(&mut index);
        assert_equals_fresh(&index, &corpus, &scratch, step);
        match step {
            "delete a file" => assert_eq!(report.removed, 1, "{step}: {report:?}"),
            "rename keeping the bytes" => {
                assert_eq!(report.removed, 1, "{step}: {report:?}");
                assert_eq!(report.parsed, 1, "{step}: {report:?}");
            }
            "touch an mtime" => assert_eq!(report.parsed, 0, "{step}: {report:?}"),
            "add an [ids] prefix" | "remove the [ids] prefix" => {
                assert!(report.reparsed_all, "{step}: {report:?}");
                assert_eq!(report.parsed, report.walked, "{step}: {report:?}");
            }
            "drop a root" => {
                assert!(report.removed >= 1, "{step}: {report:?}");
                assert!(
                    !index
                        .files()
                        .expect("files")
                        .iter()
                        .any(|path| path.starts_with("docs/features/")),
                    "{step}: the dropped root's files are gone"
                );
            }
            _ => {}
        }
    }
    // The prefix step really changed what the unchanged added file parses to.
    let extended = specengine_model::IdScheme::new(
        corpus
            .scheme
            .prefixes()
            .iter()
            .cloned()
            .chain([specengine_model::PrefixSpec::number("EXTRA", "extra", 2)])
            .collect(),
    )
    .expect("scheme with EXTRA");
    let with_prefix = specengine_core::parse(ADDED_FILE, &corpus.bytes(ADDED_FILE), &extended);
    let without = specengine_core::parse(ADDED_FILE, &corpus.bytes(ADDED_FILE), &corpus.scheme);
    assert_ne!(
        with_prefix.links, without.links,
        "the [ids] step must change the parse of an unchanged file"
    );
}

#[test]
fn only_changed_bytes_are_parsed_and_update_paths_gives_the_walks_dump() {
    let scratch = Scratch::new("incremental-key");
    let mut corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut walked = corpus.open(&scratch.db("walked"));
    let mut probed = corpus.open(&scratch.db("probed"));
    let first = corpus.update(&mut walked);
    assert_eq!(first.parsed, first.walked);
    corpus.update(&mut probed);

    let unchanged = corpus.update(&mut walked);
    assert_eq!(unchanged.parsed, 0, "no change: {unchanged:?}");
    assert_eq!(unchanged.unchanged, unchanged.walked);
    assert!(!unchanged.reparsed_all);

    touch(&corpus.root, "docs/spec/movement/sprint.md", 1_200_000_000);
    let report = corpus.update(&mut walked);
    assert_eq!(report.parsed, 0, "an mtime touch: {report:?}");

    // Another table of specengine.toml; the caller re-reads the file.
    let toml = corpus.read_text("specengine.toml");
    corpus.write(
        "specengine.toml",
        format!("{toml}\n[budgets]\nnode_tokens = 900\n"),
    );
    corpus.reload();
    let report = corpus.update(&mut walked);
    assert_eq!(report.parsed, 0, "a [budgets] edit: {report:?}");
    assert!(!report.reparsed_all, "a [budgets] edit: {report:?}");

    // A comment inside [ids].
    let toml = corpus.read_text("specengine.toml");
    corpus.write(
        "specengine.toml",
        toml.replacen("[ids]\n", "[ids]\n# prefixes of the game design\n", 1),
    );
    assert!(
        corpus
            .read_text("specengine.toml")
            .contains("# prefixes of the game design")
    );
    corpus.reload();
    let report = corpus.update(&mut walked);
    assert_eq!(report.parsed, 0, "an [ids] comment: {report:?}");
    assert!(!report.reparsed_all, "an [ids] comment: {report:?}");

    // One edit: one file parsed, by the walk and by the path.
    let path = "docs/records/DEC/DEC-0007.md";
    let text = corpus.read_text(path);
    corpus.write(path, format!("{text}\nAmended after the playtest.\n"));
    let report = corpus.update(&mut walked);
    assert_eq!(report.parsed, 1, "one edit, walked: {report:?}");
    assert_eq!(report.unchanged, report.walked - 1);
    let report = probed
        .update_paths(&corpus.tree(), &corpus.scheme, &[path])
        .expect("update_paths");
    assert_eq!(report.parsed, 1, "one edit, by path: {report:?}");
    assert!(!report.reparsed_all, "one edit, by path: {report:?}");
    let walked_dump = walked.dump().expect("dump");
    assert!(
        probed.dump().expect("dump") == walked_dump,
        "update_paths differs from the walk\n{}",
        common::dump_diff(&probed.dump().unwrap(), &walked_dump)
    );
    assert_equals_fresh(&walked, &corpus, &scratch, "one edit");

    // A deleted file named by path goes; a named directory re-probes the
    // stored files under it.
    corpus.remove(path);
    let report = probed
        .update_paths(&corpus.tree(), &corpus.scheme, &[path])
        .expect("update_paths of a deleted file");
    assert_eq!(report.removed, 1, "{report:?}");
    corpus.remove("docs/records/Q/Q-031.md");
    corpus.remove("docs/records/Q/Q-032.md");
    let report = probed
        .update_paths(&corpus.tree(), &corpus.scheme, &["docs/records/Q"])
        .expect("update_paths of a directory");
    assert_eq!(report.removed, 2, "{report:?}");
    assert_equals_fresh(&probed, &corpus, &scratch, "update_paths of deletions");
}

#[test]
fn update_paths_of_a_never_indexed_worktree_walks_everything() {
    let scratch = Scratch::new("incremental-first");
    let corpus = Corpus::copy_of("spec-b", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    let report = index
        .update_paths(&corpus.tree(), &corpus.scheme, &["docs/spec/cli.md"])
        .expect("update_paths");
    assert_eq!(report.walked, corpus.listing().paths.len(), "{report:?}");
    assert_equals_fresh(&index, &corpus, &scratch, "the first update_paths");
}

/// AC-11 of docs/features/phase1-cleanup.md (S1): `update_paths` walks
/// everything when a named path is a directory the walk reaches (new or
/// stored: a rename), a directory holding a root, or no clean root-relative
/// path (`./x`, absolute, `""`, a trailing `/`); the index then equals a
/// fresh one. A clean `.md` path is probed alone.
#[test]
fn update_paths_escalates_on_directories_and_unclean_paths() {
    let scratch = Scratch::new("incremental-escalate");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    let everything = |corpus: &Corpus| corpus.listing().paths.len();
    let update_paths = |index: &mut specengine_store::SqliteIndex, path: &str| {
        index
            .update_paths(&corpus.tree(), &corpus.scheme, &[path])
            .unwrap_or_else(|error| panic!("update_paths({path:?}): {error}"))
    };

    // A clean `.md` path: that one file.
    let edited = "docs/records/DEC/DEC-0007.md";
    let text = corpus.read_text(edited);
    corpus.write(edited, format!("{text}\nAmended once.\n"));
    let report = update_paths(&mut index, edited);
    assert_eq!((report.walked, report.parsed), (1, 1), "{report:?}");
    assert!(!report.reparsed_all);
    assert_equals_fresh(&index, &corpus, &scratch, "a clean .md path");

    // A new directory under a root, named alone.
    corpus.write(
        "docs/spec/newdir/a.md",
        "---\nclass: spec\n---\n# New\n\nSee RULE-CORE-LOOP.\n",
    );
    let report = update_paths(&mut index, "docs/spec/newdir");
    assert_equals_fresh(&index, &corpus, &scratch, "a new directory");
    assert_eq!(report.walked, everything(&corpus), "new dir: {report:?}");

    // A stored directory renamed: the new name alone.
    std::fs::rename(
        corpus.root.join("docs/spec/newdir"),
        corpus.root.join("docs/spec/renamed"),
    )
    .expect("rename");
    let report = update_paths(&mut index, "docs/spec/renamed");
    assert_equals_fresh(&index, &corpus, &scratch, "a renamed directory");
    assert_eq!(report.walked, everything(&corpus), "renamed: {report:?}");

    // A directory holding roots.
    corpus.write("docs/records/R/R-77.md", "---\nid: R-77\n---\n# R-77\n");
    let report = update_paths(&mut index, "docs");
    assert_equals_fresh(&index, &corpus, &scratch, "a directory holding roots");
    assert_eq!(report.walked, everything(&corpus), "docs: {report:?}");

    // An edited file named by no clean root-relative path.
    let absolute = corpus.root.join(edited).to_str().unwrap().to_owned();
    for (step, named) in [
        ("./path", format!("./{edited}")),
        ("absolute", absolute),
        ("empty", String::new()),
        ("parent/", "docs/records/DEC/".to_owned()),
    ] {
        let text = corpus.read_text(edited);
        corpus.write(edited, format!("{text}\nAmended by {step}.\n"));
        let report = update_paths(&mut index, &named);
        assert_equals_fresh(&index, &corpus, &scratch, step);
        assert_eq!(report.walked, everything(&corpus), "{step}: {report:?}");
        assert_eq!(report.parsed, 1, "{step}: {report:?}");
    }

    // A dot-directory under a root is none the walk reaches: no walk, and
    // the index still equals a fresh one (the walk skips it too).
    corpus.write("docs/spec/.hidden/a.md", "# Hidden\n");
    let report = update_paths(&mut index, "docs/spec/.hidden");
    assert_eq!(report.walked, 0, ".hidden: {report:?}");
    assert_equals_fresh(&index, &corpus, &scratch, "a dot-directory");
}
