//! AC-09 of docs/features/spec-index.md: the format stamp. A rewritten
//! stored stamp (a DB written by another format) makes reads `NotIndexed`
//! and the next update re-parse everything (`reparsed_all`), after which the
//! dump equals a fresh index; `update_paths` escalates to a walk; other
//! worktrees of the DB read `NotIndexed` until they are updated.
//! `tests/format_history.txt` records `<INDEX_FORMAT> <BLAKE3 of the dump of
//! spec-a and spec-b, root masked>`: it ends with the current pair, and no
//! earlier line has the current number with another hash — a change of what
//! a fresh index stores needs a new line and a new number.
//!
//! The stamp is rewritten through a raw connection to the DB file: an older
//! format cannot be produced through the store's own API (the only test that
//! names `rusqlite`, see `api.rs`).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use common::{Corpus, Scratch, assert_equals_fresh, blake3_hex};
use specengine_store::{INDEX_FORMAT, IndexWriter, SpecIndex, SqliteIndex, StoreError};

/// Rewrites the stored stamp as a build of another format would have left it.
fn rewrite_stamp(db: &Path, value: &str) {
    let conn = rusqlite::Connection::open(db).expect("raw connection");
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .expect("busy timeout");
    let changed = conn
        .execute(
            "UPDATE index_meta SET value = ?1 WHERE key = 'format'",
            [value],
        )
        .expect("rewrite the stamp");
    assert_eq!(changed, 1, "one stamp row");
}

fn assert_not_indexed<T: std::fmt::Debug>(result: Result<T, StoreError>, context: &str) {
    match result {
        Err(StoreError::NotIndexed) => {}
        other => panic!("{context}: expected NotIndexed, got {other:?}"),
    }
}

#[test]
fn a_rewritten_stamp_reparses_everything_and_equals_a_fresh_index() {
    let scratch = Scratch::new("format");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let db = scratch.db("index");
    let mut index = corpus.open(&db);
    corpus.update(&mut index);
    let fresh = index.dump().expect("dump");

    rewrite_stamp(&db, "0");
    assert_not_indexed(index.files(), "files() after the stamp changed");
    assert_not_indexed(
        index.file("docs/spec/game.md"),
        "file() after the stamp changed",
    );
    assert_not_indexed(
        index.lookup_id("MEC-STAMINA"),
        "lookup_id() after the stamp changed",
    );
    assert_not_indexed(
        index.search(&specengine_store::SearchQuery::new("stamina")),
        "search() after the stamp changed",
    );

    let report = corpus.update(&mut index);
    assert!(
        report.reparsed_all,
        "the stamp change re-parses all: {report:?}"
    );
    assert_eq!(report.parsed, report.walked, "{report:?}");
    assert_eq!(index.dump().expect("dump"), fresh, "after the recreation");
    assert_equals_fresh(&index, &corpus, &scratch, "the stamp change");

    // `update_paths` after a stamp change escalates to a full walk.
    rewrite_stamp(&db, "0");
    let report = index
        .update_paths(&corpus.tree(), &corpus.scheme, &["docs/spec/game.md"])
        .expect("update_paths");
    assert!(report.reparsed_all, "update_paths escalates: {report:?}");
    assert_eq!(report.walked, corpus.listing().paths.len(), "{report:?}");
    assert_equals_fresh(
        &index,
        &corpus,
        &scratch,
        "update_paths after the stamp change",
    );
}

#[test]
fn other_worktrees_read_not_indexed_until_they_are_updated() {
    let scratch = Scratch::new("format-worktrees");
    let first = Corpus::copy_of("spec-a", &scratch, "first");
    let second = Corpus::copy_of("spec-b", &scratch, "second");
    let db = scratch.db("index");
    let mut one = first.open(&db);
    let mut two = second.open(&db);
    first.update(&mut one);
    second.update(&mut two);
    let two_before = two.dump_worktree().expect("dump");

    rewrite_stamp(&db, "0");
    let report = first.update(&mut one);
    assert!(report.reparsed_all, "{report:?}");
    assert!(!one.files().expect("files").is_empty());
    assert_not_indexed(two.files(), "the other worktree after the recreation");

    let report = second.update(&mut two);
    assert!(
        report.reparsed_all || report.parsed == report.walked,
        "{report:?}"
    );
    assert_eq!(
        two.dump_worktree().expect("dump"),
        two_before,
        "the other worktree after its own update"
    );
}

/// The dump of spec-a and spec-b, each in a fresh DB of its own, with the
/// scratch root masked: what a fresh index of the current format stores.
fn format_dump() -> String {
    let scratch = Scratch::new("format-history");
    let mut out = String::new();
    for name in ["spec-a", "spec-b"] {
        let corpus = Corpus::copy_of(name, &scratch, name);
        let mut index =
            SqliteIndex::open(scratch.db(name), "format-history", &corpus.root).expect("open");
        corpus.update(&mut index);
        let root = index.root().to_str().expect("UTF-8 root").to_owned();
        let dump = index.dump().expect("dump");
        assert!(dump.contains(&root), "the dump names its root");
        out.push_str(&dump.replace(&root, "<root>"));
    }
    out
}

#[test]
fn format_history_ends_with_the_current_format_and_dump_hash() {
    let dump = format_dump();
    assert_eq!(dump, format_dump(), "the dump is deterministic");
    let hash = blake3_hex(dump.as_bytes());
    let history_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/format_history.txt");
    let history = std::fs::read_to_string(&history_path).expect("tests/format_history.txt");
    let mut lines: Vec<(u32, String)> = Vec::new();
    for (number, line) in history.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(format), Some(dump_hash), None) = (fields.next(), fields.next(), fields.next())
        else {
            panic!(
                "format_history.txt:{}: expected `<INDEX_FORMAT> <hash>`: {line}",
                number + 1
            );
        };
        let format: u32 = format.parse().unwrap_or_else(|_| {
            panic!(
                "format_history.txt:{}: {format} is not a number",
                number + 1
            )
        });
        assert!(
            dump_hash.len() == 64 && dump_hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "format_history.txt:{}: {dump_hash} is not a BLAKE3 hex",
            number + 1
        );
        if let Some((previous, _)) = lines.last() {
            assert!(
                format >= *previous,
                "format_history.txt:{}: the numbers never go down",
                number + 1
            );
        }
        lines.push((format, dump_hash.to_owned()));
    }
    let current = format!("{INDEX_FORMAT} {hash}");
    let Some((last_format, last_hash)) = lines.last() else {
        panic!("format_history.txt is empty; its line is:\n{current}");
    };
    for (format, earlier) in &lines[..lines.len() - 1] {
        assert!(
            !(*format == INDEX_FORMAT && *earlier != hash),
            "format_history.txt: INDEX_FORMAT {INDEX_FORMAT} was already used for another dump \
             ({earlier}); a changed dump needs a new INDEX_FORMAT and a new line"
        );
    }
    assert!(
        *last_format == INDEX_FORMAT && *last_hash == hash,
        "format_history.txt must end with the current format and dump hash:\n{current}\n\
         last line: {last_format} {last_hash}\n\
         (a changed dump needs a new INDEX_FORMAT and a new line)"
    );
}

/// Every key path the model serialises into the index (`ParsedFile`: the
/// shell, `node`, `link`, `anchor` and `diagnostic` values), plus both forms
/// of `canon` and every front-matter value kind. A field the fixtures never
/// set is left out of the dump (`skip_serializing_if`), so a projection
/// change touching only it would keep the dump hash above: the fixtures
/// must set each one. A new model field belongs in this list and in a
/// fixture.
const SERIALISED: &[&str] = &[
    "bom",
    "front_matter",
    "body",
    "nodes[].id",
    "nodes[].script",
    "nodes[].kind",
    "nodes[].title",
    "nodes[].summary",
    "nodes[].level",
    "nodes[].heading",
    "nodes[].body",
    "nodes[].attrs",
    "nodes[].classes",
    "nodes[].rev",
    "nodes[].parent.id",
    "nodes[].parent.span",
    "nodes[].span",
    "nodes[].tokens_est",
    "nodes[].extra[].key",
    "nodes[].extra[].value",
    "nodes[].fields.class",
    "nodes[].fields.status",
    "nodes[].fields.owner",
    "nodes[].fields.reviewed",
    "nodes[].fields.date",
    "nodes[].fields.shipped",
    "nodes[].fields.ref",
    "nodes[].fields.to",
    "nodes[].fields.severity",
    "nodes[].fields.generator",
    "nodes[].fields.source",
    "nodes[].fields.acceptance",
    "nodes[].fields.tier",
    "nodes[].fields.scope",
    "nodes[].fields.aliases",
    "nodes[].fields.working_answer.id",
    "nodes[].fields.canon.id",
    "nodes[].fields.canon.path",
    "nodes[].fields.canon.anchor",
    "nodes[].fields.canon.span",
    "nodes[].fields.supersedes[].id",
    "nodes[].fields.adrs[].id",
    "nodes[].fields.refs[].id",
    "nodes[].fields.links.*[].id",
    "nodes[].fields.raised_by.*",
    "links[].src",
    "links[].src_span",
    "links[].type",
    "links[].origin",
    "links[].dst.id",
    "links[].dst.alias_of",
    "links[].dst.script",
    "links[].dst.project",
    "links[].dst.scope",
    "links[].dst.section",
    "links[].dst.rev",
    "links[].dst.form",
    "links[].dst.label",
    "links[].dst.span",
    "links[].dst.path",
    "links[].dst.anchor",
    "links[].dst (path, frontmatter).path",
    "links[].dst (path, frontmatter).anchor",
    "links[].dst (path, frontmatter).span",
    "links[].dst (path, inline).path",
    "links[].dst (path, inline).anchor",
    "links[].dst (path, inline).span",
    "anchors[].name",
    "anchors[].origin",
    "anchors[].origin: slug",
    "anchors[].origin: attr",
    "anchors[].origin: html",
    "anchors[].level",
    "anchors[].span",
    "diagnostics[].code",
    "diagnostics[].severity",
    "diagnostics[].line",
    "diagnostics[].span",
    "diagnostics[].fix",
    "diagnostics[].message",
    "front-matter value: null",
    "front-matter value: bool",
    "front-matter value: i64",
    "front-matter value: u64",
    "front-matter value: f64",
    "front-matter value: string",
    "front-matter value: sequence",
    "front-matter value: mapping",
];

/// Key paths of a serialised `ParsedFile`: array items as `[]`, the
/// dynamic keys of `fields.links`, `fields.raised_by` and front-matter
/// mappings as `*`, the kind of every front-matter value, every anchor
/// origin, and the keys of a path destination by link origin (a `canon:`
/// path and a Markdown file link share `dst.path` / `dst.span` with an ID
/// reference's keys; docs/features/spec-check-links.md).
fn key_paths(value: &serde_json::Value, path: &str, in_value: bool, out: &mut BTreeSet<String>) {
    use serde_json::Value;
    if path == "links[]"
        && let Value::Object(link) = value
        && let (Some(Value::Object(dst)), Some(Value::String(origin))) =
            (link.get("dst"), link.get("origin"))
        && dst.contains_key("path")
    {
        for key in dst.keys() {
            out.insert(format!("links[].dst (path, {origin}).{key}"));
        }
    }
    if in_value {
        out.insert(
            match value {
                Value::Null => "front-matter value: null",
                Value::Bool(_) => "front-matter value: bool",
                Value::Number(n) if n.is_i64() => "front-matter value: i64",
                Value::Number(n) if n.is_u64() => "front-matter value: u64",
                Value::Number(_) => "front-matter value: f64",
                Value::String(_) => "front-matter value: string",
                Value::Array(_) => "front-matter value: sequence",
                Value::Object(_) => "front-matter value: mapping",
            }
            .to_owned(),
        );
    }
    match value {
        Value::Object(map) => {
            let dynamic =
                in_value || path.ends_with("fields.links") || path.ends_with("fields.raised_by");
            for (key, child) in map {
                let key = if dynamic { "*" } else { key.as_str() };
                let child_path = if path.is_empty() {
                    key.to_owned()
                } else {
                    format!("{path}.{key}")
                };
                out.insert(child_path.clone());
                // Each anchor origin is its own shape of the row: slug and
                // attr carry a level, html does not.
                if child_path == "anchors[].origin"
                    && let Value::String(origin) = child
                {
                    out.insert(format!("{child_path}: {origin}"));
                }
                let child_in_value = in_value
                    || child_path.ends_with("extra[].value")
                    || child_path.ends_with("fields.raised_by.*");
                key_paths(child, &child_path, child_in_value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                key_paths(item, &format!("{path}[]"), in_value, out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_fixture_dumps_exercise_every_serialised_field() {
    let scratch = Scratch::new("format-fields");
    let mut seen = BTreeSet::new();
    for name in ["spec-a", "spec-b"] {
        let corpus = Corpus::copy_of(name, &scratch, name);
        let mut index = corpus.open(&scratch.db(name));
        corpus.update(&mut index);
        for path in index.files().expect("files") {
            let parsed = index
                .file(&path)
                .expect("file")
                .and_then(|file| file.parsed)
                .expect("parsed");
            let value = serde_json::to_value(&parsed).expect("JSON");
            key_paths(&value, "", false, &mut seen);
        }
    }
    let missing: Vec<&str> = SERIALISED
        .iter()
        .copied()
        .filter(|path| !seen.contains(*path))
        .collect();
    assert!(
        missing.is_empty(),
        "serialised fields no fixture of spec-a or spec-b sets (the format dump \
         cannot see a change to them):\n{}",
        missing.join("\n")
    );
}

/// AC-19 of docs/features/spec-cli.md: `files.tier3` and `nodes.line`
/// change the schema, so `INDEX_FORMAT` is 6 and the history is the five
/// earlier lines, verbatim, followed by exactly one `6 <hash>` line (no
/// earlier `6`). Replaces pass B's format-5 pin
/// (docs/features/spec-check-links.md AC-03), whose line stays verbatim.
#[test]
fn spec_cli_pins_format_6_with_one_new_history_line() {
    assert_eq!(INDEX_FORMAT, 6);
    let history = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/format_history.txt"),
    )
    .expect("format_history.txt");
    let lines: Vec<&str> = history
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    let earlier = [
        "1 4fd55fa4cf7716f7acc45556ab7c831f1f67d7decdf2f0829ad0a7da5e52a906",
        "2 2cfc299da177807c83d246aea934a73ab8509021386568a0b2011a31de66c9aa",
        "3 d63c06dc03ed97243ff25dfb14a165cfec059582eee05dc72d9fba05b3c5f55f",
        "4 d958fe30af22feb15f1dbd8fafa67ef1113a8c4ba267c4dd71a8968eb30bf5b5",
        "5 df99199e747d5d2f16712fd23737a443457140d4bb6e37d59393c78724809be4",
    ];
    assert_eq!(
        lines.len(),
        earlier.len() + 1,
        "pass B's history plus exactly one new line:\n{history}"
    );
    assert_eq!(
        &lines[..earlier.len()],
        &earlier[..],
        "the earlier lines stay verbatim"
    );
    let last = lines[earlier.len()];
    let (format, hash) = last.split_once(' ').expect("`<format> <hash>`");
    assert_eq!(format, "6", "the new line is format 6: {last}");
    assert!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "a BLAKE3 hex: {last}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.starts_with("6 ")).count(),
        1,
        "no earlier `6` line"
    );
}
