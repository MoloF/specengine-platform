//! A stub `CodeAnalyzer` (docs/features/symbol-index.md "Crates": the
//! store owns compare, apply and the cross-file pass behind its trait
//! `CodeAnalyzer` — list, packages, fingerprint, analyze; plain rows): the
//! store has no dependency on `specengine-code`, so its own tests give it
//! rows a stub makes from fixed text, never from a parse. `format.rs`
//! indexes them beside spec-a and spec-b, so `format_history.txt`'s lines
//! from `9` on pin the code tables' projection too ("Tables": `9 <hash>`
//! over spec-a, spec-b plus a stub analyzer's rows; docs/features/
//! bevy-roles.md "Formats": `10 <hash>`, the stub emitting role rows).
//!
//! The stub's tree: two Rust files of one package and a RON file, holding
//! one row of every kind the store keeps — a symbol with suffixes, an
//! in-file twin pair (`#1`, `#2`), an item another file renders too (the
//! pass's `twin_files`), a `#[path]` declaration (the pass's
//! `declared_by`), a cannot-verify symbol with no digest, a marker on an
//! item, a marker on a RON path, a mention, and role rows
//! (`framework_roles`, bevy-roles "Table"): one on an item, one with a
//! subject and a group, one `uncertain` with a category, one in the other
//! file. Their words and sources are the stub's own: the store keeps
//! strings, it knows no framework (RD-8).
//!
//! Every store type it names (the trait, its row types,
//! `SqliteIndex::update_code`, `CodeReport`) is the store's public API; a
//! change of those types is adapted here, and nowhere else in the tests.
//! `CodeRoleRow` and `CodeFileRows.framework_roles` are the spec's
//! names (bevy-roles "Table", SR6); as built: strings,
//! `Option` where the column takes NULL, `u32` ordinals and lines.

#![allow(dead_code)]

use std::path::Path;

use specengine_model::IdScheme;
use specengine_store::{
    CodeAnalyzer, CodeDeclRow, CodeFileRows, CodeListing, CodeMarkerRow, CodeMentionRow,
    CodePackageRow, CodeReport, CodeRoleRow, CodeSymbolRow, ListedCodeFile, SqliteIndex,
    StoreError,
};

/// `b3:` and 64 hex digits of `seed` repeated: a stand-in digest.
fn digest(seed: char) -> String {
    format!("b3:{}", seed.to_string().repeat(64))
}

/// The stub's files: (path, bytes). The bytes only key the cache; the
/// rows below never read them.
pub const FILES: [(&str, &str); 3] = [
    ("crates/stub/src/lib.rs", "stub lib\n"),
    ("crates/stub/src/other.rs", "stub other\n"),
    ("data/stub.ron", "stub ron\n"),
];

/// The analyzer stamp of the stub (its `fingerprint`).
pub const FINGERPRINT: &[u8] = b"stub-analyzer 1";

pub struct Stub {
    pub fingerprint: Vec<u8>,
}

impl Default for Stub {
    fn default() -> Self {
        Self {
            fingerprint: FINGERPRINT.to_vec(),
        }
    }
}

fn symbol(
    ord: u32,
    qpath: &str,
    discriminator: Option<&str>,
    lines: (u32, u32),
    hashed: bool,
) -> CodeSymbolRow {
    let name = qpath
        .rsplit_once(" #")
        .map_or(qpath, |(name, _)| name)
        .rsplit("::")
        .next()
        .unwrap_or(qpath)
        .to_owned();
    // The suffixes are written out whole: the stub, like the analyzer,
    // hands over segments, and the store never splits a qpath.
    let segments: Vec<String> = qpath.split("::").map(str::to_owned).collect();
    let suffixes = (1..=segments.len())
        .map(|depth| segments[segments.len() - depth..].join("::"))
        .collect();
    CodeSymbolRow {
        ord,
        item_kind: "stub_item".to_owned(),
        name,
        owner: None,
        qpath: qpath.to_owned(),
        discriminator: discriminator.map(str::to_owned),
        parent_ord: None,
        start_line: lines.0,
        end_line: lines.1,
        start_byte: lines.0 * 10,
        end_byte: lines.1 * 10 + 9,
        signature: hashed.then(|| format!("stub {qpath}")),
        level_state: if hashed { "hashed" } else { "cannot_verify" }.to_owned(),
        level_categories: if hashed { "[]" } else { "[\"other\"]" }.to_owned(),
        sig_hash: hashed.then(|| digest('1')),
        body_hash: hashed.then(|| digest('2')),
        deps_hash: hashed.then(|| digest('3')),
        skeleton_hash: hashed.then(|| digest('4')),
        quote_hash: hashed.then(|| digest('5')),
        body_bytes: hashed.then_some(42),
        layer: "a".to_owned(),
        suffixes,
    }
}

/// A role row on the item `symbol_ord` (no subject), `certain`.
fn role_on_item(ord: u32, word: &str, symbol_ord: u32, line: u32) -> CodeRoleRow {
    CodeRoleRow {
        ord,
        word: word.to_owned(),
        source: "stub_source_item".to_owned(),
        form: None,
        origin: "code".to_owned(),
        group_text: None,
        symbol_ord: Some(symbol_ord),
        subject: None,
        subject_name: None,
        certainty: "certain".to_owned(),
        category: None,
        line,
    }
}

impl CodeAnalyzer for Stub {
    fn list(&self) -> Result<CodeListing, StoreError> {
        Ok(CodeListing {
            files: FILES
                .iter()
                .map(|(path, bytes)| ListedCodeFile {
                    path: (*path).to_owned(),
                    bytes: Some(bytes.as_bytes().to_vec()),
                    read_error: None,
                    // The package directory is the listing's (part of the
                    // cache key); the RON file is in none.
                    package: path
                        .starts_with("crates/stub/")
                        .then(|| "crates/stub".to_owned()),
                })
                .collect(),
            gaps: Vec::new(),
            skipped: 0,
        })
    }

    fn packages(&self, _listing: &CodeListing) -> Result<Vec<CodePackageRow>, StoreError> {
        Ok(vec![CodePackageRow {
            dir: "crates/stub".to_owned(),
            manifest_b3: Some(digest('6')),
            source: "layout".to_owned(),
            targets: "[{\"kind\":\"lib\",\"name\":\"stub\",\"root\":\"src/lib.rs\"}]".to_owned(),
        }])
    }

    fn fingerprint(&self) -> Vec<u8> {
        self.fingerprint.clone()
    }

    fn analyze(&self, file: &ListedCodeFile, _package: Option<&CodePackageRow>) -> CodeFileRows {
        match file.path.as_str() {
            "crates/stub/src/lib.rs" => CodeFileRows {
                lang: "rust".to_owned(),
                utf8: true,
                read_error: None,
                role: Some("crate_root".to_owned()),
                module: Some("crates/stub".to_owned()),
                orphan_errors: 0,
                symbols: vec![
                    symbol(0, "crates/stub::alpha", None, (1, 3), true),
                    symbol(1, "crates/stub::impl Pair #1", Some("1"), (4, 6), true),
                    symbol(2, "crates/stub::impl Pair #2", Some("2"), (7, 9), true),
                    symbol(3, "crates/stub::broken", None, (10, 12), false),
                    symbol(4, "crates/stub::shared", None, (13, 14), true),
                ],
                decls: vec![CodeDeclRow {
                    ord: 0,
                    target: "crates/stub/src/other.rs".to_owned(),
                    line: 15,
                }],
                markers: vec![CodeMarkerRow {
                    ord: 0,
                    relation: "implements".to_owned(),
                    token: "STUB-ONE@2".to_owned(),
                    id: "STUB-ONE".to_owned(),
                    rev: Some(2),
                    levels: Some("[\"body\"]".to_owned()),
                    levels_state: "declared".to_owned(),
                    note: Some("a note".to_owned()),
                    line: 3,
                    column: 4,
                    binding: "item".to_owned(),
                    symbol_ord: Some(0),
                    anchor: None,
                }],
                mentions: vec![CodeMentionRow {
                    ord: 0,
                    written: "STUB-TWO".to_owned(),
                    line: 5,
                    column: 9,
                    binding: "item".to_owned(),
                    symbol_ord: Some(1),
                    anchor: None,
                }],
                framework_roles: vec![
                    role_on_item(0, "stub_word_a", 0, 1),
                    CodeRoleRow {
                        ord: 1,
                        word: "stub_word_b".to_owned(),
                        source: "stub_source_call".to_owned(),
                        form: Some("path".to_owned()),
                        origin: "code".to_owned(),
                        group_text: Some("StubGroup".to_owned()),
                        symbol_ord: None,
                        subject: Some("other::shared".to_owned()),
                        subject_name: Some("shared".to_owned()),
                        certainty: "certain".to_owned(),
                        category: None,
                        line: 5,
                    },
                    CodeRoleRow {
                        ord: 2,
                        word: "stub_word_b".to_owned(),
                        source: "stub_source_call".to_owned(),
                        form: None,
                        origin: "macro_rules".to_owned(),
                        group_text: None,
                        symbol_ord: None,
                        subject: None,
                        subject_name: None,
                        certainty: "uncertain".to_owned(),
                        category: Some("stub_category".to_owned()),
                        line: 8,
                    },
                ],
            },
            "crates/stub/src/other.rs" => CodeFileRows {
                lang: "rust".to_owned(),
                utf8: true,
                read_error: None,
                role: Some("module".to_owned()),
                module: Some("crates/stub::other".to_owned()),
                orphan_errors: 1,
                symbols: vec![symbol(0, "crates/stub::shared", None, (2, 4), true)],
                decls: Vec::new(),
                markers: Vec::new(),
                mentions: Vec::new(),
                framework_roles: vec![role_on_item(0, "stub_word_a", 0, 2)],
            },
            _ => CodeFileRows {
                lang: "ron".to_owned(),
                utf8: true,
                read_error: None,
                role: None,
                module: None,
                orphan_errors: 0,
                symbols: Vec::new(),
                decls: Vec::new(),
                markers: vec![CodeMarkerRow {
                    ord: 0,
                    relation: "configures".to_owned(),
                    token: "STUB-ONE".to_owned(),
                    id: "STUB-ONE".to_owned(),
                    rev: None,
                    levels: Some("[]".to_owned()),
                    levels_state: "default".to_owned(),
                    note: None,
                    line: 1,
                    column: 4,
                    binding: "ron".to_owned(),
                    symbol_ord: None,
                    anchor: Some("root.cap".to_owned()),
                }],
                mentions: Vec::new(),
                framework_roles: Vec::new(),
            },
        }
    }
}

/// The stub's rows indexed into `index`'s worktree (the store's
/// code refresh).
pub fn index_stub(index: &mut SqliteIndex, scheme: &IdScheme) -> CodeReport {
    index
        .update_code(&Stub::default(), scheme, false)
        .expect("the stub's rows are indexed")
}

/// The stub's files exist nowhere: `root` is only the worktree they are
/// keyed by.
pub fn root_is_untouched(root: &Path) -> bool {
    FILES.iter().all(|(path, _)| !root.join(path).exists())
}

/// The role rows the stub gives, `(path, ord, word, certainty)`, by path
/// and ord: what `code_roles` must hold after [`index_stub`].
pub const ROLE_ROWS: [(&str, u32, &str, &str); 4] = [
    ("crates/stub/src/lib.rs", 0, "stub_word_a", "certain"),
    ("crates/stub/src/lib.rs", 1, "stub_word_b", "certain"),
    ("crates/stub/src/lib.rs", 2, "stub_word_b", "uncertain"),
    ("crates/stub/src/other.rs", 0, "stub_word_a", "certain"),
];
