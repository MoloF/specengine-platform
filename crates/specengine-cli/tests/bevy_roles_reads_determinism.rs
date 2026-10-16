//! docs/features/bevy-roles-reads.md TE-3, the `symbols` half (the
//! `find_symbols` half: `crates/specengine-mcp/tests/mcp_code.rs`
//! `te3_find_symbols_roles_are_byte_identical_across_roots_order_cwd_and_crlf`;
//! the index half: `bevy_roles_determinism.rs`), and "Rules and edge
//! cases", Determinism: two roots, cwd, reverse creation, CRLF give
//! byte-identical `spec symbols` in every form, text and JSON, role lines
//! and `roles` included; nothing follows a rowid (a copy whose role-holding
//! files are re-derived after every other file answers alike).
//!
//! Over scratch copies of `fixtures/code-bevy` with the module and RON
//! markers of `bevy_roles_reads_common`, each read in a `HOME` of its own.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod bevy_roles_reads_common;
mod code_index_common;
mod common;

use std::fs;
use std::path::Path;

use bevy_roles_common::{code_paths_under, copy_reversed};
use bevy_roles_reads_common::{FORMS, bevy_copy, mark_non_items};
use code_index_common::to_crlf;
use common::{Scratch, fixture, md_files, spec};

/// Every form of [`FORMS`], text then JSON, run in `cwd` with `extra`
/// before the command: (arguments, exit, stdout, stderr).
fn answers(home: &Path, cwd: &Path, extra: &[&str]) -> Vec<(String, i32, String, String)> {
    let mut out = Vec::new();
    for form in FORMS {
        for json in [false, true] {
            let mut args: Vec<&str> = extra.to_vec();
            if json {
                args.push("--json");
            }
            args.extend_from_slice(form);
            let run = spec(home, cwd, &args);
            out.push((
                format!("{form:?} json {json}"),
                run.code,
                run.stdout,
                run.stderr,
            ));
        }
    }
    out
}

/// `answers` equal, entry by entry, to `base`.
fn assert_same(
    label: &str,
    base: &[(String, i32, String, String)],
    got: &[(String, i32, String, String)],
) {
    assert_eq!(base.len(), got.len(), "{label}");
    for (want, got) in base.iter().zip(got) {
        assert_eq!(want, got, "{label}: {}", want.0);
    }
}

/// TE-3. M: a rowid order (rows or candidates as inserted); the absolute
/// root in a field; CR kept in a field. The same copy twice; a copy made
/// in reverse creation order at another root, read with `--root` from
/// another cwd; the copy read from a subdirectory; a copy whose `.rs`,
/// `.ron` and `.md` files are all CRLF: every form, text and JSON, byte
/// for byte alike, role lines in the answers. A copy read, then `lib.rs`
/// and `herd.rs` (the files holding most rows and the candidates of
/// `lib.rs`' rows) re-derived after every other file: alike again.
#[test]
fn symbols_roles_are_byte_identical_across_roots_order_cwd_and_crlf() {
    let scratch = Scratch::new("brr-determinism");
    let first = bevy_copy(&scratch, "one/copy");
    let second = scratch.dir("two/elsewhere-else");
    copy_reversed(&fixture("code-bevy"), &second);
    mark_non_items(&second);
    let crlf = bevy_copy(&scratch, "three/crlf");
    for path in code_paths_under(&crlf).into_iter().chain(md_files(&crlf)) {
        to_crlf(&crlf, &path);
    }
    let cwd = scratch.dir("somewhere");
    let second_text = second.to_str().expect("a UTF-8 scratch").to_owned();

    let base = answers(&scratch.home("a"), &first, &[]);
    assert!(
        base.iter()
            .any(|(_, _, stdout, _)| stdout.lines().any(|line| line.starts_with("  role "))),
        "the answers hold role lines"
    );
    assert!(
        base.iter()
            .any(|(_, _, stdout, _)| stdout.contains("\"roles\":[{")),
        "the documents hold roles"
    );
    assert_same(
        "the same copy again",
        &base,
        &answers(&scratch.home("b"), &first, &[]),
    );
    assert_same(
        "reverse creation, --root from another cwd",
        &base,
        &answers(&scratch.home("c"), &cwd, &["--root", second_text.as_str()]),
    );
    assert_same(
        "from a subdirectory",
        &base,
        &answers(&scratch.home("d"), &first.join("crates/farm/src"), &[]),
    );
    assert_same("CRLF", &base, &answers(&scratch.home("e"), &crlf, &[]));

    let later = bevy_copy(&scratch, "four/later");
    let home = scratch.home("later");
    spec(&home, &later, &["index"]).code(0);
    for path in ["crates/farm/src/lib.rs", "crates/farm/src/herd.rs"] {
        let mut bytes = fs::read(later.join(path)).expect("read");
        bytes.extend_from_slice(b"// appended\n");
        fs::write(later.join(path), bytes).expect("write");
    }
    assert_same(
        "lib.rs and herd.rs re-derived last",
        &base,
        &answers(&home, &later, &[]),
    );
}
