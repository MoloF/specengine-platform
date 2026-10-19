//! docs/features/symbol-index.md RD-2 (the cache key), RD-3 (the
//! cross-file pass), RD-4 (the dump shows no storage order) and TE-3's CLI
//! half (`code_format_history.txt`, Q16, Q24): after every change the
//! index of a scratch `HOME` equals a fresh index of the same tree in a new
//! `HOME`, byte for byte (the store's canonical dump, every table), and
//! `spec refs` shows the names the change gives.
//!
//! Each RD-2 test makes one key part change while the file's bytes stay:
//! the manifest (`targets_fp`), a package directory (a new `Cargo.toml`
//! between a file and its package), `[ids]` (mentions), the stored
//! `code_fp` (the analyzer stamp: rewritten through the system's `sqlite3`,
//! as a build of another `CODE_INDEX_FORMAT` left it), the bytes under an
//! unchanged size and mtime, `[code] exclude`. The RD-3 tests change one
//! file and read the effect on another: a cross-file twin removed, a
//! `#[path]` dropped from the declaring file, and a ` #` in a file name, an
//! unrooted twin and a `#[path]` target twin (the 6a-1 review's S4: the
//! pass reads discriminators, never splits a name, and counts every file).
//!
//! `docs/canon/code-roles.md` "Table" (bevy-roles F1, RD-3): from
//! `CODE_INDEX_FORMAT` 3 the history hashes code-index's code-table dump
//! followed by code-bevy's (its `code_roles` rows in), lines 1 and 2
//! verbatim.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;

use std::fs;
use std::path::Path;

use code_index_common::{
    FIXTURE, SLUG, assert_equals_fresh, code_line, code_lines, code_paths, copy, counts_of, db,
    dump, execute, fresh_dump, index, query, refs_rows, same_size_same_mtime_edit, symbols,
    with_code_table, with_ids_line,
};
use common::bundle::blake3_hex;
use common::{Scratch, read_text, repository_root, spec, write};

/// The marker rows of `spec refs reference` (the summary dropped).
fn marker_rows(home: &Path, root: &Path, reference: &str) -> Vec<String> {
    let run = spec(home, root, &["refs", reference]);
    run.code(0);
    let mut rows = refs_rows(&run.stdout);
    rows.pop();
    rows
}

fn replace(root: &Path, relative: &str, from: &str, to: &str) {
    let text = read_text(root, relative);
    assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?} once");
    write(root, relative, text.replacen(from, to, 1));
}

// ------------------------------------------------------------------ RD-2

/// RD-2. M: the manifest ignored. A `[package] name` edit renames the
/// `src/main.rs` bin unit; a new `src/bin/` file changes the target table:
/// every file of the package is re-derived (`targets_fp`), equal to a
/// fresh index.
#[test]
fn manifest_edit_renames_qpaths_as_a_rebuild() {
    let scratch = Scratch::new("ci-manifest");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let main_row = "implements | crates/inkwell/src/main.rs:1:4 | SLAB-SHEET@1 | resolved | current | default | item crates/inkwell::bin:inkwell-core::main function_item";
    assert!(
        marker_rows(&home, &root, "SLAB-SHEET").contains(&main_row.to_owned()),
        "before"
    );

    replace(
        &root,
        "crates/inkwell/Cargo.toml",
        "name = \"inkwell-core\"",
        "name = \"inkwell-press\"",
    );
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        counts["parsed"],
        9,
        "the package's Rust files: {}",
        run.show()
    );
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the package renamed");
    assert!(
        marker_rows(&home, &root, "SLAB-SHEET")
            .contains(&main_row.replace("bin:inkwell-core", "bin:inkwell-press")),
        "the bin unit follows the manifest"
    );

    write(&root, "crates/inkwell/src/bin/press.rs", "fn main() {}\n");
    index(&home, &root);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a bin added");
    fs::remove_file(root.join("crates/inkwell/src/bin/press.rs")).unwrap();
    index(&home, &root);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the bin removed");
}

/// RD-2. M: the package directory ignored. A `Cargo.toml` written between
/// a file and its package re-parents the file (its bytes unchanged): its
/// qpath moves from the outer package, unrooted, to the new package's
/// library; removing it moves it back. Each step equals a fresh index.
#[test]
fn new_manifest_between_reparents_files() {
    let scratch = Scratch::new("ci-reparent");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(
        &root,
        "crates/inkwell/vendor/side/src/lib.rs",
        "// @implements TENET-FLOW@1\npub fn side() {}\n",
    );
    index(&home, &root);
    let row = |binding: &str| {
        format!(
            "implements | crates/inkwell/vendor/side/src/lib.rs:1:4 | TENET-FLOW@1 | resolved | current | default | {binding}"
        )
    };
    let unrooted = row("item crates/inkwell::side function_item ambiguous unrooted");
    assert!(
        marker_rows(&home, &root, "TENET-FLOW").contains(&unrooted),
        "{:?}",
        marker_rows(&home, &root, "TENET-FLOW")
    );

    write(
        &root,
        "crates/inkwell/vendor/side/Cargo.toml",
        "[package]\nname = \"side\"\n",
    );
    let run = index(&home, &root);
    assert!(counts_of(&run)["parsed"] >= 1, "{}", run.show());
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a manifest between");
    assert!(
        marker_rows(&home, &root, "TENET-FLOW")
            .contains(&row("item crates/inkwell/vendor/side::side function_item")),
        "{:?}",
        marker_rows(&home, &root, "TENET-FLOW")
    );

    fs::remove_file(root.join("crates/inkwell/vendor/side/Cargo.toml")).unwrap();
    index(&home, &root);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the manifest removed");
    assert!(marker_rows(&home, &root, "TENET-FLOW").contains(&unrooted));
}

/// RD-2. M: `[ids]` ignored. A prefix added to `[ids]` makes a comment's
/// `NIB-01` a mention: every code file is re-derived (the `[ids]`
/// fingerprint is in `code_fp`), the stored mentions gain it; removed, they
/// lose it. Each step equals a fresh index.
#[test]
fn ids_change_rescans_mentions() {
    let scratch = Scratch::new("ci-ids");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let config = read_text(&root, "specengine.toml");
    let nib = |home: &Path| {
        query(
            &db(home, SLUG),
            "SELECT written FROM code_mentions WHERE written = 'NIB-01'",
        )
        .len()
    };
    assert_eq!(nib(&home), 0);

    with_ids_line(&root, "NIB   = { kind = \"nib\", width = 2 }\n");
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(counts["mentions"], 12, "{}", run.show());
    assert_eq!(counts.get("rederived_all"), Some(&1), "{}", run.show());
    assert_eq!(nib(&home), 1);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a prefix added");

    write(&root, "specengine.toml", &config);
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["mentions"], 11, "{}", run.show());
    assert_eq!(nib(&home), 0);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the prefix removed");
}

/// RD-2. M: the analyzer stamp ignored. A stored `code_fp` of another
/// analyzer (rewritten as a build of another `CODE_INDEX_FORMAT` left it):
/// the next index re-derives every code file (`, rederived all`), equal to
/// a fresh index; the one after re-derives none. That the stamp is part of
/// `code_fp` at all is pinned by `code_format_history_pins_the_code_index_dump`
/// (the dump holds `code_fp`).
#[test]
fn analyzer_format_change_rederives_every_code_file() {
    let scratch = Scratch::new("ci-stamp");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    execute(
        &db(&home, SLUG),
        "UPDATE code_worktrees SET code_fp = 'b3:written-by-another-analyzer'",
    );
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(counts["parsed"], counts["walked"], "{}", run.show());
    assert_eq!(counts["walked"], 10, "{}", run.show());
    assert_eq!(counts.get("rederived_all"), Some(&1), "{}", run.show());
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the stamp changed");
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (counts["parsed"], counts["unchanged"]),
        (0, 10),
        "{}",
        run.show()
    );
    assert_eq!(counts.get("rederived_all"), None, "{}", run.show());
}

/// RD-2. M: the mtime as the key. A same-size edit with the mtime put back
/// is re-derived (the key is `(path, BLAKE3)`); a touch alone re-derives
/// nothing.
#[test]
fn same_size_same_mtime_edit_is_reindexed() {
    let scratch = Scratch::new("ci-mtime");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    same_size_same_mtime_edit(&root, "crates/inkwell/src/twins.rs", "'/'", "'|'");
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (counts["parsed"], counts["unchanged"]),
        (1, 9),
        "{}",
        run.show()
    );
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a same-size edit");

    let path = root.join("crates/inkwell/src/page.rs");
    let file = fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(std::time::SystemTime::now()).unwrap();
    drop(file);
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["parsed"], 0, "a touch: {}", run.show());
}

/// RD-2. M: `exclude` ignored. `[code] exclude` gains a directory: its
/// file's rows go (its cross-file twin stops being `duplicate`); the
/// pattern removed, they come back. Each step equals a fresh index.
#[test]
fn code_exclude_change_drops_rows() {
    let scratch = Scratch::new("ci-exclude");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    with_code_table(
        &root,
        "[code]\nroots = [\"crates\", \"data\"]\nexclude = [\"crates/inkwell/src/split/**\"]\n",
    );
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["removed"], 1, "{}", run.show());
    let excluded = "crates/inkwell/src/split/mod.rs".to_owned();
    assert!(!code_paths(&db(&home, SLUG)).contains(&excluded));
    assert!(
        marker_rows(&home, &root, "TENET-WIDTH").contains(
            &"implements | crates/inkwell/src/split.rs:3:4 | TENET-WIDTH@2 | resolved | current | default | item crates/inkwell::split::joined function_item".to_owned()
        ),
        "{:?}",
        marker_rows(&home, &root, "TENET-WIDTH")
    );
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a directory excluded");

    with_code_table(&root, "[code]\nroots = [\"crates\", \"data\"]\n");
    index(&home, &root);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the exclude removed");
}

/// `--full` re-derives every code file (`, rederived all`) and keeps the
/// rows equal to the incremental ones.
#[test]
fn full_rederives_every_code_file() {
    let scratch = Scratch::new("ci-full");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    let first = index(&home, &root);
    let before = dump(&home, &root, SLUG);
    assert_eq!(
        counts_of(&first).get("rederived_all"),
        None,
        "{}",
        first.show()
    );
    let run = spec(&home, &root, &["index", "--full"]);
    run.code(0);
    let counts = counts_of(&run);
    assert_eq!(counts["parsed"], 10, "{}", run.show());
    assert_eq!(counts.get("rederived_all"), Some(&1), "{}", run.show());
    assert!(
        code_line(&run).unwrap().ends_with(", rederived all"),
        "{}",
        run.show()
    );
    assert_eq!(dump(&home, &root, SLUG), before);
}

// ------------------------------------------------------- RD-2, TE-3, Q16

/// The code tables of a fresh index of `fixtures/code-index` (a scratch
/// copy), its root masked.
fn code_index_dump() -> String {
    let scratch = Scratch::new("ci-history");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    code_lines(&dump(&home, &root, SLUG))
}

/// The code tables of a fresh index of `fixtures/code-bevy` (a scratch
/// copy, `frameworks = ["bevy"]`), its root masked
/// (`docs/canon/code-roles.md` "Table"; bevy-roles F1).
fn code_bevy_dump() -> String {
    let scratch = Scratch::new("ci-history-bevy");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy", "code-bevy");
    index(&home, &root);
    code_lines(&dump(&home, &root, "code-bevy"))
}

/// What the current line of `code_format_history.txt` hashes: from
/// `CODE_INDEX_FORMAT` 3 on, code-index's code dump followed by
/// code-bevy's (the header says so).
fn code_history_dump() -> String {
    format!("{}{}", code_index_dump(), code_bevy_dump())
}

/// RD-2, TE-3 (Q16, Q24); bevy-roles RD-3 (F1). M: a signature or a role
/// rule changed, the stamp kept. `tests/code_format_history.txt` holds
/// `<CODE_INDEX_FORMAT> <BLAKE3 of the code tables' dump, root masked>`
/// lines (from 3: code-index then code-bevy): numbers never go down, the
/// first is 1, it ends with the current pair, and no earlier line has the
/// current number with another hash.
#[test]
fn code_format_history_pins_the_code_index_dump() {
    let text = code_history_dump();
    assert_eq!(text, code_history_dump(), "the code dump is deterministic");
    assert!(text.lines().count() > 100, "{text}");
    assert!(
        text.lines()
            .filter(|line| line.starts_with("code_roles\t"))
            .count()
            >= 27,
        "code-bevy's role rows are hashed:\n{text}"
    );
    let hash = blake3_hex(text.as_bytes());
    // The stamp of "Analyzer stamp", in `specengine-code`.
    let format = specengine_code::CODE_INDEX_FORMAT;
    let current = format!("{format} {hash}");
    let history = read_text(
        &repository_root(),
        "crates/specengine-cli/tests/code_format_history.txt",
    );
    let mut lines: Vec<(u32, String)> = Vec::new();
    for (number, line) in history.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(stamp), Some(dump_hash), None) = (fields.next(), fields.next(), fields.next())
        else {
            panic!(
                "code_format_history.txt:{}: expected `<CODE_INDEX_FORMAT> <hash>`: {line}",
                number + 1
            );
        };
        let stamp: u32 = stamp.parse().unwrap_or_else(|_| {
            panic!(
                "code_format_history.txt:{}: {stamp} is not a number",
                number + 1
            )
        });
        assert!(
            dump_hash.len() == 64 && dump_hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "code_format_history.txt:{}: {dump_hash} is not a BLAKE3 hex",
            number + 1
        );
        if let Some((previous, _)) = lines.last() {
            assert!(
                stamp >= *previous,
                "code_format_history.txt:{}: the numbers never go down",
                number + 1
            );
        }
        lines.push((stamp, dump_hash.to_owned()));
    }
    let Some((last_format, last_hash)) = lines.last() else {
        panic!("code_format_history.txt is empty; its line is:\n{current}");
    };
    assert_eq!(lines[0].0, 1, "CODE_INDEX_FORMAT starts at 1 (Q16)");
    for (stamp, earlier) in &lines[..lines.len() - 1] {
        assert!(
            !(*stamp == format && *earlier != hash),
            "code_format_history.txt: CODE_INDEX_FORMAT {format} was already used for another \
             dump ({earlier}); a changed code dump needs a new CODE_INDEX_FORMAT and a new line"
        );
    }
    assert!(
        *last_format == format && *last_hash == hash,
        "code_format_history.txt must end with the current stamp and code dump hash:\n{current}\n\
         last line: {last_format} {last_hash}\n\
         (a changed code dump needs a new CODE_INDEX_FORMAT and a new line)"
    );
}

/// bevy-roles "Formats" (F1); bevy-roles-reads B5 (K2, the owner's
/// pre-approval: a role subject's qualified type is stored compacted, so
/// the stamp moved to 4); docs/features/hash-levels-use-lists.md TE-9
/// (the level recipe amended in place under the same `level_header()`, so
/// the stamp moves to 5). M: `CODE_INDEX_FORMAT` kept
/// at 4; an earlier line edited. The stamp is 5; the history is 6b-1's two
/// lines, 10a-1's third and bevy-roles-reads' fourth verbatim and exactly
/// one `5 <hash>`; its header names code-bevy as hashed after code-index.
#[test]
fn code_format_history_keeps_lines_1_to_4_and_adds_5() {
    assert_eq!(specengine_code::CODE_INDEX_FORMAT, 5);
    let history = read_text(
        &repository_root(),
        "crates/specengine-cli/tests/code_format_history.txt",
    );
    let header: String = history
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        header.contains("fixtures/code-index") && header.contains("fixtures/code-bevy"),
        "the header says what is hashed:\n{header}"
    );
    let lines: Vec<&str> = history
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(lines.len(), 5, "{history}");
    assert_eq!(
        lines[..4],
        [
            "1 3eac6715cece86e4385c82367d489ccc24d4a7896044accb18a713ec344a3a0e",
            "2 206f941120e31b317e822f026607d6bbccd5efdf77a30f338446578d7fdc275a",
            "3 49cfadfed7e012013a20e19453f7d70a6a4b50530c9febad2f602e2b86f4889a",
            "4 8a7cd52d4c4e94d09697c39535e786037a36a60595a5ebf22f6a0a35cffef276",
        ],
        "lines 1 to 4 verbatim"
    );
    let (format, hash) = lines[4].split_once(' ').expect("`<format> <hash>`");
    assert_eq!(format, "5", "{history}");
    assert!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "a BLAKE3 hex: {}",
        lines[4]
    );
}

// ------------------------------------------------------------------ RD-3

/// RD-3. M: per-file ambiguity. `split.rs`'s `joined` is `duplicate` while
/// `split/mod.rs` renders the same qpath; renaming the other file's item
/// (only that file re-derived) clears it in `split.rs` (its `twin_files`
/// 0), and renaming it back restores it. Each step equals a fresh index.
#[test]
fn removing_a_twin_in_another_file_clears_duplicate() {
    let scratch = Scratch::new("ci-twin-gone");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let row = "implements | crates/inkwell/src/split.rs:3:4 | TENET-WIDTH@2 | resolved | current | default | item crates/inkwell::split::joined function_item";
    let duplicate = format!("{row} ambiguous duplicate");
    assert!(marker_rows(&home, &root, "TENET-WIDTH").contains(&duplicate));

    replace(
        &root,
        "crates/inkwell/src/split/mod.rs",
        "pub fn joined()",
        "pub fn apart()",
    );
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["parsed"], 1, "{}", run.show());
    assert!(
        marker_rows(&home, &root, "TENET-WIDTH").contains(&row.to_owned()),
        "{:?}",
        marker_rows(&home, &root, "TENET-WIDTH")
    );
    let twin_files: Vec<_> = symbols(&db(&home, SLUG))
        .into_iter()
        .filter(|symbol| symbol["path"] == "crates/inkwell/src/split.rs")
        .map(|symbol| symbol["twin_files"].clone())
        .collect();
    assert_eq!(twin_files, [serde_json::json!(0)]);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the twin renamed away");

    replace(
        &root,
        "crates/inkwell/src/split/mod.rs",
        "pub fn apart()",
        "pub fn joined()",
    );
    index(&home, &root);
    assert!(marker_rows(&home, &root, "TENET-WIDTH").contains(&duplicate));
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the twin back");
}

/// RD-3. M: `#[path]` ignored. The `#[path]` target's items are
/// `path_attribute` while the declaring file names it; dropping the
/// attribute from the declaring file (the target's bytes unchanged)
/// clears it, and writing it back restores it. Each step equals a fresh
/// index.
#[test]
fn path_attribute_follows_the_declaring_file() {
    let scratch = Scratch::new("ci-path-attr");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(
        &root,
        "crates/inkwell/src/extra/moved.rs",
        "// @implements TENET-FLOW@1\npub fn shift() {}\n",
    );
    let declared = "pub mod twins;\n#[path = \"extra/moved.rs\"]\nmod moved;\n";
    replace(
        &root,
        "crates/inkwell/src/lib.rs",
        "pub mod twins;\n",
        declared,
    );
    index(&home, &root);
    let row = "implements | crates/inkwell/src/extra/moved.rs:1:4 | TENET-FLOW@1 | resolved | current | default | item crates/inkwell::extra::moved::shift function_item";
    let attributed = format!("{row} ambiguous path_attribute");
    assert!(
        marker_rows(&home, &root, "TENET-FLOW").contains(&attributed),
        "{:?}",
        marker_rows(&home, &root, "TENET-FLOW")
    );

    replace(
        &root,
        "crates/inkwell/src/lib.rs",
        "#[path = \"extra/moved.rs\"]\n",
        "",
    );
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["parsed"], 1, "only lib.rs: {}", run.show());
    assert!(
        marker_rows(&home, &root, "TENET-FLOW").contains(&row.to_owned()),
        "{:?}",
        marker_rows(&home, &root, "TENET-FLOW")
    );
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the attribute dropped");

    replace(
        &root,
        "crates/inkwell/src/lib.rs",
        "pub mod twins;\nmod moved;\n",
        declared,
    );
    index(&home, &root);
    assert!(marker_rows(&home, &root, "TENET-FLOW").contains(&attributed));
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the attribute back");
}

/// RD-3, the 6a-1 review's S4. M: twins read as `qpath` in [`u #`,
/// `u $`) alone; unrooted or `#[path]` files left out of `twin_files`. A
/// file named `src/note #2.rs` renders `crates/inkwell::note #2::kept`,
/// inside that range of `crates/inkwell::note` (lib.rs's `mod note;`) but
/// no twin of it: `note` stays unambiguous, `twin_files` 0. An unrooted
/// file's `fn twins` twins lib.rs's `mod twins;`, and a `#[path]` target's
/// `shift` twins `src/extra.rs`'s inline `moved::shift`: both other sides
/// read `duplicate`, `twin_files` 1. Equal to a fresh index.
#[test]
fn twin_pass_reads_discriminators_not_names() {
    let scratch = Scratch::new("ci-twin-names");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    replace(
        &root,
        "crates/inkwell/src/lib.rs",
        "pub mod split;\npub mod twins;\n",
        "pub mod split;\n// @implements TENET-FLOW@1\npub mod twins;\n// @implements TENET-FLOW@1\npub mod note;\n#[path = \"extra/moved.rs\"]\nmod moved;\n",
    );
    write(&root, "crates/inkwell/src/note #2.rs", "pub fn kept() {}\n");
    write(
        &root,
        "crates/inkwell/vendor/loose.rs",
        "pub fn twins() {}\n",
    );
    write(
        &root,
        "crates/inkwell/src/extra/moved.rs",
        "pub fn shift() {}\n",
    );
    write(
        &root,
        "crates/inkwell/src/extra.rs",
        "pub mod moved {\n    // @implements TENET-FLOW@1\n    pub fn shift() {}\n}\n",
    );
    index(&home, &root);
    let rows = marker_rows(&home, &root, "TENET-FLOW");
    for want in [
        "implements | crates/inkwell/src/extra.rs:2:8 | TENET-FLOW@1 | resolved | current | default | item crates/inkwell::extra::moved::shift function_item ambiguous duplicate",
        "implements | crates/inkwell/src/lib.rs:7:4 | TENET-FLOW@1 | resolved | current | default | item crates/inkwell::twins mod_item ambiguous duplicate",
        "implements | crates/inkwell/src/lib.rs:9:4 | TENET-FLOW@1 | resolved | current | default | item crates/inkwell::note mod_item",
    ] {
        assert!(rows.contains(&want.to_owned()), "{want}\n{rows:#?}");
    }
    let stored = symbols(&db(&home, SLUG));
    let twin_files = |path: &str, qpath: &str| {
        stored
            .iter()
            .find(|row| row["path"] == path && row["qpath"] == qpath)
            .unwrap_or_else(|| panic!("{path} {qpath}"))["twin_files"]
            .clone()
    };
    for (path, qpath, count) in [
        ("crates/inkwell/src/lib.rs", "crates/inkwell::note", 0),
        (
            "crates/inkwell/src/note #2.rs",
            "crates/inkwell::note #2::kept",
            0,
        ),
        ("crates/inkwell/src/lib.rs", "crates/inkwell::twins", 1),
        ("crates/inkwell/vendor/loose.rs", "crates/inkwell::twins", 1),
        (
            "crates/inkwell/src/extra.rs",
            "crates/inkwell::extra::moved::shift",
            1,
        ),
    ] {
        assert_eq!(twin_files(path, qpath), count, "{path} {qpath}");
    }
    assert_eq!(
        twin_files(
            "crates/inkwell/src/extra/moved.rs",
            "crates/inkwell::extra::moved::shift"
        ),
        1
    );
    assert_equals_fresh(&scratch, &home, &root, SLUG, "names with ` #`");

    // The twin of `note` across files appears only with a real one.
    write(&root, "crates/inkwell/vendor/note.rs", "pub fn note() {}\n");
    index(&home, &root);
    assert!(
        marker_rows(&home, &root, "TENET-FLOW").contains(
            &"implements | crates/inkwell/src/lib.rs:9:4 | TENET-FLOW@1 | resolved | current | default | item crates/inkwell::note mod_item ambiguous duplicate".to_owned()
        ),
        "{:?}",
        marker_rows(&home, &root, "TENET-FLOW")
    );
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a real twin of note");
}

// ------------------------------------------------------------------ RD-4

/// RD-4. M: a rowid dumped. Rows deleted and inserted again (a file
/// removed, indexed, written back, indexed) land after every other row,
/// yet the dump equals a fresh index; so does a copy of the fixture made
/// in reverse order under another root, its root masked.
#[test]
fn code_dump_is_independent_of_insertion_order() {
    let scratch = Scratch::new("ci-order");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    for relative in [
        "crates/inkwell/src/bin/stamp.rs",
        "crates/inkwell/src/broken.rs",
    ] {
        let bytes = fs::read(root.join(relative)).unwrap();
        fs::remove_file(root.join(relative)).unwrap();
        index(&home, &root);
        fs::write(root.join(relative), bytes).unwrap();
        index(&home, &root);
    }
    let have = dump(&home, &root, SLUG);
    assert_eq!(have, fresh_dump(&scratch, &root, SLUG), "re-inserted rows");

    let reversed = scratch.dir("other/place");
    copy_reversed(&common::fixture(FIXTURE), &reversed);
    let _ = fs::remove_file(reversed.join("expected.json"));
    let other_home = scratch.home("reversed");
    index(&other_home, &reversed);
    assert_eq!(
        code_lines(&dump(&other_home, &reversed, SLUG)),
        code_lines(&have),
        "another root, reverse creation order"
    );
}

/// Copies `from` to `to` creating every entry in reverse name order.
fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_reversed(&entry.path(), &target);
        } else {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}
