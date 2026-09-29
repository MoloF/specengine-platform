//! AC-19 of docs/features/spec-index.md (`docs/canon/architecture.md#distribution`):
//! no `pub` item of the store's `src` names `rusqlite` — neither the crate
//! path nor a name imported from it — and the tests need nothing but the
//! public API: they write through `IndexWriter` and read through
//! `SpecIndex`. The only test naming `rusqlite` is `format.rs`, which
//! rewrites the stored stamp as a build of another format would have left
//! it (not expressible through the store's API).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Corpus, Scratch};
use specengine_model::IdScheme;
use specengine_store::{IndexWriter, SearchQuery, Source, SpecIndex, SqliteIndex, StoreError};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    assert!(!out.is_empty(), "no sources under {}", dir.display());
    out
}

/// The line without a `//` comment (string contents with `//` are rare
/// enough in item signatures to ignore).
fn code_of(line: &str) -> &str {
    line.split("//").next().unwrap_or("")
}

/// The names a file imports from `rusqlite` (`use rusqlite::…;`, with
/// braces, nested paths and `as` renames).
fn rusqlite_imports(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find("use rusqlite::") {
        let tail = &rest[start + "use rusqlite::".len()..];
        let end = tail.find(';').expect("a use statement ends with ;");
        let body = &tail[..end];
        for item in body.split([',', '{', '}']) {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let name = match item.split_once(" as ") {
                Some((_, alias)) => alias.trim(),
                None => item.rsplit("::").next().unwrap_or(item).trim(),
            };
            if !name.is_empty() && name != "self" && name != "*" {
                names.insert(name.to_owned());
            }
        }
        rest = &tail[end..];
    }
    names
}

fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let boundary = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        boundary(before) && boundary(after)
    })
}

/// Every `pub` item (not `pub(crate)`, `pub(super)`, `pub(in …)`) of the
/// store's `src` whose signature names `rusqlite` or a name imported from
/// it, as `file:line: signature`.
fn public_rusqlite_items(src: &Path) -> Vec<String> {
    let mut offenders = Vec::new();
    for file in rust_files(src) {
        let text = fs::read_to_string(&file).expect("UTF-8 source");
        let imported = rusqlite_imports(&text);
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("pub ") || trimmed == "pub") {
                continue;
            }
            // The signature: up to the line that opens a body or ends the
            // item; a field (or a variant list entry) also ends at `,`.
            let is_fn = trimmed.contains("fn ");
            let mut signature = String::new();
            for next in &lines[index..] {
                let code = code_of(next);
                signature.push_str(code.trim());
                signature.push(' ');
                let end = code.trim_end();
                if code.contains('{') || end.ends_with(';') || (!is_fn && end.ends_with(',')) {
                    break;
                }
            }
            let names_rusqlite = signature.contains("rusqlite")
                || imported.iter().any(|name| has_word(&signature, name));
            if names_rusqlite {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(crate_dir()).unwrap_or(&file).display(),
                    index + 1,
                    signature.trim()
                ));
            }
        }
    }
    offenders
}

#[test]
fn no_public_item_of_the_store_names_rusqlite() {
    let src = crate_dir().join("src");
    let offenders = public_rusqlite_items(&src);
    assert!(
        offenders.is_empty(),
        "public items naming rusqlite:\n{}",
        offenders.join("\n")
    );
}

/// The scan itself finds what it is meant to find.
#[test]
fn the_scan_sees_a_public_rusqlite_accessor() {
    let scratch = Scratch::new("api-scan");
    let dir = scratch.join("src");
    fs::create_dir_all(&dir).unwrap();
    for (name, body, expected) in [
        (
            "direct.rs",
            "impl X {\n    pub fn connection(&self) -> &rusqlite::Connection {\n        &self.conn\n    }\n}\n",
            1,
        ),
        (
            "imported.rs",
            "use rusqlite::{Connection, Transaction as Tx};\npub struct H {\n    pub conn: Connection,\n    pub(crate) tx: Tx,\n}\npub fn begin(\n    h: &H,\n) -> Tx {\n    todo!()\n}\n",
            2,
        ),
        (
            "private.rs",
            "use rusqlite::Connection;\npub(crate) fn raw(c: &Connection) {}\nfn other(c: &Connection) {}\n/// pub fn x() -> rusqlite::Connection\npub fn fine() -> u8 { 0 }\n",
            0,
        ),
    ] {
        fs::write(dir.join(name), body).unwrap();
        let found = public_rusqlite_items(&dir);
        let in_file = found.iter().filter(|line| line.contains(name)).count();
        assert_eq!(in_file, expected, "{name}: {found:?}");
    }
}

#[test]
fn the_tests_use_only_the_public_traits() {
    let tests = crate_dir().join("tests");
    let mut offenders = Vec::new();
    for file in rust_files(&tests) {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        // `format.rs`: the stamp rewrite (see the module docs); this file:
        // the scanner, whose strings name the crate.
        if name == "format.rs" || name == "api.rs" {
            continue;
        }
        let text = fs::read_to_string(&file).expect("UTF-8 test source");
        for (index, line) in text.lines().enumerate() {
            if code_of(line).contains("rusqlite") {
                offenders.push(format!("{}:{}: {}", file.display(), index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "tests reaching around the public API:\n{}",
        offenders.join("\n")
    );
}

/// Written and read only through trait objects: the traits suffice.
fn index_through_traits(
    writer: &mut dyn IndexWriter,
    source: &dyn Source,
    scheme: &IdScheme,
) -> Result<(), StoreError> {
    writer.update(source, scheme)?;
    writer.update_paths(source, scheme, &["docs/spec/game.md"])?;
    writer.rebuild(source, scheme)?;
    Ok(())
}

fn read_through_traits(reader: &dyn SpecIndex) -> Result<usize, StoreError> {
    let files = reader.files()?;
    let mut nodes = 0;
    for path in &files {
        nodes += reader
            .file(path)?
            .and_then(|file| file.parsed)
            .map_or(0, |parsed| parsed.nodes.len());
    }
    assert!(!reader.lookup_id("MEC-STAMINA")?.is_empty());
    assert!(!reader.search(&SearchQuery::new("stamina"))?.hits.is_empty());
    Ok(nodes)
}

#[test]
fn the_public_traits_suffice_and_the_handle_is_send() {
    fn send<T: Send>() {}
    fn error<E: std::error::Error + Send + Sync + 'static>() {}
    send::<SqliteIndex>();
    error::<StoreError>();

    let scratch = Scratch::new("api-traits");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    index_through_traits(&mut index, &corpus.tree(), &corpus.scheme).expect("writes");
    let nodes = read_through_traits(&index).expect("reads");
    assert!(nodes > 0);

    // A handle moves to another thread.
    let handle = std::thread::spawn(move || read_through_traits(&index).expect("reads"));
    assert_eq!(handle.join().expect("thread"), nodes);

    // A source walking another root is refused.
    let other = Corpus::copy_of("spec-b", &scratch, "other");
    let mut index = corpus.open(&scratch.db("index"));
    match index.update(&other.tree(), &other.scheme) {
        Err(StoreError::RootMismatch { .. }) => {}
        result => panic!("expected RootMismatch, got {result:?}"),
    }
}
