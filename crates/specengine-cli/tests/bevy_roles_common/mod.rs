//! Helpers of the docs/features/bevy-roles.md tests (slice 10a-1, framework
//! roles in the code index, opt-in `[code] frameworks`): scratch copies of
//! `fixtures/code-bevy` (TE-1) and small probe projects written at test
//! time, the `spec` binary with a scratch `HOME` per index, the stored
//! `code_roles` rows read by column name through the system's `sqlite3`
//! (`code_index_common::query`), and the `roles <slug>:` line of `spec
//! index` read back into the shape of its JSON twin `code.roles`.
//!
//! The column names, the line and the JSON keys are the spec's ("Table",
//! "spec index"). Only the bevy-roles test binaries include this module
//! (`mod bevy_roles_common;`, after `mod common;` and
//! `mod code_index_common;`).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::code_index_common::{FILE_JOIN, query, with_code_table};
use crate::common::{Run, Scratch, fixture, read_text, spec, write};

/// The fixture of this slice (TE-1) and its `[project] slug`.
pub const FIXTURE: &str = "code-bevy";
pub const SLUG: &str = "code-bevy";

/// Every word of `bevy` in the spec's "Role facts", byte order: the words
/// `spec index` lists, zeros too. A test literal (the genre scan reads the
/// words from the code crate's data, never from here).
pub const WORDS: [&str; 9] = [
    "component",
    "entity_event",
    "event",
    "message",
    "observer",
    "plugin",
    "reflect",
    "resource",
    "system",
];

/// The `code_roles` columns of the spec's "Table", after the file's path.
pub const ROLE_COLUMNS: [&str; 12] = [
    "ord",
    "word",
    "source",
    "form",
    "origin",
    "group_text",
    "symbol_ord",
    "subject",
    "subject_name",
    "certainty",
    "category",
    "line",
];

/// code-bevy's `[code]` tables: the opt-in on (the fixture's own), off,
/// and listed empty (off as well).
pub const CODE_ON: &str =
    "[code]\nroots      = [\"crates\", \"assets\"]\nframeworks = [\"bevy\"]\n";
pub const CODE_OFF: &str = "[code]\nroots = [\"crates\", \"assets\"]\n";
pub const CODE_EMPTY: &str = "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = []\n";

/// The probe projects' slug and config: one prefix of their own, the
/// opt-in on.
pub const PROBE_SLUG: &str = "role-probe";
pub const PROBE_CONFIG: &str = "[project]\nslug = \"role-probe\"\n\n[ids]\nPLOT = { kind = \"plot\", width = 2 }\n\n[code]\nroots = [\"crates\"]\nframeworks = [\"bevy\"]\n";

/// The probe's package manifest (`crates/<dir>/Cargo.toml`).
pub fn manifest(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n"
    )
}

/// `fixtures/code-bevy/expected.json`.
pub fn expected() -> Value {
    serde_json::from_str(&read_text(&fixture(FIXTURE), "expected.json"))
        .expect("fixtures/code-bevy/expected.json is JSON")
}

/// A probe project at `<scratch>/<dir>`: [`PROBE_CONFIG`], one record, the
/// package `crates/probe` (its manifest unless `files` writes one), and
/// `files` (root-relative path, text). Canonical.
pub fn probe(scratch: &Scratch, dir: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = scratch.dir(dir);
    write(&root, "specengine.toml", PROBE_CONFIG);
    write(
        &root,
        "docs/records/PLOT-01.md",
        "---\nclass: canon\nid: PLOT-01\nowner: owner\nreviewed: 2026-10-16\n---\n\n# A plot is sown\n\nA plot is sown once a season.\n",
    );
    write(&root, "crates/probe/Cargo.toml", manifest("probe"));
    for (path, text) in files {
        write(&root, path, text);
    }
    root
}

/// `spec index` of `root` in `home`, exit 0.
pub fn index(home: &Path, root: &Path) -> Run {
    let run = spec(home, root, &["index"]);
    run.code(0);
    run
}

/// The `roles <slug>: …` line of a `spec index` run, if any.
pub fn roles_line(run: &Run) -> Option<String> {
    run.stdout
        .lines()
        .find(|line| line.starts_with("roles "))
        .map(str::to_owned)
}

/// The roles line a run must print.
pub fn roles_line_of_run(run: &Run) -> String {
    roles_line(run).unwrap_or_else(|| panic!("no roles line:\n{}", run.show()))
}

/// The line "spec index" gives for `counts` (each word absent from it is
/// 0), `subjects` `[linked, ambiguous, unlinked]` and `uncertain`.
pub fn line_of(slug: &str, counts: &[(&str, u64)], subjects: [u64; 3], uncertain: u64) -> String {
    for (word, _) in counts {
        assert!(WORDS.contains(word), "{word} is no word of the spec");
    }
    let words: Vec<String> = WORDS
        .iter()
        .map(|word| {
            let n = counts
                .iter()
                .find(|(w, _)| w == word)
                .map_or(0, |(_, n)| *n);
            format!("{word} {n}")
        })
        .collect();
    let [linked, ambiguous, unlinked] = subjects;
    format!(
        "roles {slug}: {}; subjects {} (linked {linked}, ambiguous {ambiguous}, unlinked {unlinked}), uncertain {uncertain}",
        words.join(", "),
        linked + ambiguous + unlinked
    )
}

/// A `roles <slug>: …` line read into the shape of JSON `code.roles`
/// ("spec index": `{words, subjects: {total, linked, ambiguous,
/// unlinked}, uncertain}`).
pub fn parse_roles_line(line: &str) -> Value {
    let (_, body) = line.split_once(": ").expect("`roles <slug>: …`");
    let (words_part, rest) = body
        .split_once("; subjects ")
        .unwrap_or_else(|| panic!("`; subjects ` in {line:?}"));
    let mut words = serde_json::Map::new();
    for part in words_part.split(", ") {
        let (word, n) = part
            .rsplit_once(' ')
            .unwrap_or_else(|| panic!("`word n`: {part:?} in {line:?}"));
        let n: u64 = n.parse().unwrap_or_else(|_| panic!("{n:?} in {line:?}"));
        words.insert(word.to_owned(), json!(n));
    }
    let (total, rest) = rest
        .split_once(" (")
        .unwrap_or_else(|| panic!("`N (` in {line:?}"));
    let (states, rest) = rest
        .split_once("), uncertain ")
        .unwrap_or_else(|| panic!("`), uncertain ` in {line:?}"));
    let mut subjects = serde_json::Map::new();
    subjects.insert(
        "total".to_owned(),
        json!(total.parse::<u64>().expect("a total")),
    );
    for part in states.split(", ") {
        let (state, n) = part.rsplit_once(' ').expect("`state n`");
        subjects.insert(state.to_owned(), json!(n.parse::<u64>().expect("a count")));
    }
    json!({
        "words": Value::Object(words),
        "subjects": Value::Object(subjects),
        "uncertain": rest.parse::<u64>().expect("an uncertain count"),
    })
}

/// `spec --json index` of `root` in `home` (exit 0): the document.
pub fn index_json(home: &Path, root: &Path) -> Value {
    let run = spec(home, root, &["--json", "index"]);
    run.code(0);
    run.json()
}

/// Every `code_roles` row by (path, ord): the file's `path`, then
/// [`ROLE_COLUMNS`].
pub fn role_rows(db: &Path) -> Vec<BTreeMap<String, Value>> {
    let columns: Vec<String> = ROLE_COLUMNS.iter().map(|c| format!("x.{c}")).collect();
    query(
        db,
        &format!(
            "SELECT f.path AS path, {} FROM code_roles x {FILE_JOIN} ORDER BY f.path, x.ord",
            columns.join(", ")
        ),
    )
}

/// The rows of one file, without the `path` column.
pub fn rows_of(db: &Path, path: &str) -> Vec<BTreeMap<String, Value>> {
    role_rows(db)
        .into_iter()
        .filter(|row| row["path"] == path)
        .map(|mut row| {
            row.remove("path");
            row
        })
        .collect()
}

/// A row as the tests write it: (word, source, form, origin, group,
/// symbol_ord, subject, subject_name, certainty, category, line); `ord`
/// is its position.
pub type Row<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    &'a str,
    Option<&'a str>,
    Option<u64>,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
    Option<&'a str>,
    u64,
);

/// A `derive` row on the item `ord`, `certain`.
pub fn derived(word: &'static str, ord: u64, line: u64) -> Row<'static> {
    (
        word,
        "derive",
        None,
        "code",
        None,
        Some(ord),
        None,
        None,
        "certain",
        None,
        line,
    )
}

/// A `derive` row inside `cfg_attr` on the item `ord`: `uncertain`.
pub fn derived_cfg_attr(word: &'static str, ord: u64, line: u64) -> Row<'static> {
    (
        word,
        "derive",
        None,
        "code",
        None,
        Some(ord),
        None,
        None,
        "uncertain",
        Some("cfg_attr"),
        line,
    )
}

/// An `fn` row (`&mut App`, word `plugin`) on the fn `ord`.
pub fn app_fn(ord: u64, line: u64) -> Row<'static> {
    (
        "plugin",
        "fn",
        None,
        "code",
        None,
        Some(ord),
        None,
        None,
        "certain",
        None,
        line,
    )
}

/// An `impl` row with its subject.
pub fn implemented(word: &'static str, subject: &'static str, line: u64) -> Row<'static> {
    (
        word,
        "impl",
        None,
        "code",
        None,
        None,
        Some(subject),
        Some(subject),
        "certain",
        None,
        line,
    )
}

/// A `call` row with a subject: `form`, `origin`, the group (systems
/// only), the subject and its last segment.
pub fn called(
    word: &'static str,
    form: &'static str,
    origin: &'static str,
    group: Option<&'static str>,
    subject: &'static str,
    name: &'static str,
    line: u64,
) -> Row<'static> {
    (
        word,
        "call",
        Some(form),
        origin,
        group,
        None,
        Some(subject),
        Some(name),
        "certain",
        None,
        line,
    )
}

/// A closure registration: on its enclosing fn `ord`, `certain`.
pub fn closure(
    word: &'static str,
    group: Option<&'static str>,
    ord: u64,
    line: u64,
) -> Row<'static> {
    (
        word,
        "call",
        Some("closure"),
        "code",
        group,
        Some(ord),
        None,
        None,
        "certain",
        None,
        line,
    )
}

/// A call the detector cannot read: `uncertain`, its category, no
/// subject, on nothing.
pub fn unreadable(
    word: &'static str,
    origin: &'static str,
    category: &'static str,
    line: u64,
) -> Row<'static> {
    (
        word,
        "call",
        None,
        origin,
        None,
        None,
        None,
        None,
        "uncertain",
        Some(category),
        line,
    )
}

/// `rows` as the stored maps, `ord` 0.. in order.
pub fn stored(rows: &[Row<'_>]) -> Vec<BTreeMap<String, Value>> {
    let text = |value: Option<&str>| value.map_or(Value::Null, |text| json!(text));
    rows.iter()
        .enumerate()
        .map(
            |(
                ord,
                (
                    word,
                    source,
                    form,
                    origin,
                    group,
                    symbol_ord,
                    subject,
                    subject_name,
                    certainty,
                    category,
                    line,
                ),
            )| {
                BTreeMap::from([
                    ("ord".to_owned(), json!(ord)),
                    ("word".to_owned(), json!(word)),
                    ("source".to_owned(), json!(source)),
                    ("form".to_owned(), text(*form)),
                    ("origin".to_owned(), json!(origin)),
                    ("group_text".to_owned(), text(*group)),
                    (
                        "symbol_ord".to_owned(),
                        symbol_ord.map_or(Value::Null, |ord| json!(ord)),
                    ),
                    ("subject".to_owned(), text(*subject)),
                    ("subject_name".to_owned(), text(*subject_name)),
                    ("certainty".to_owned(), json!(certainty)),
                    ("category".to_owned(), text(*category)),
                    ("line".to_owned(), json!(line)),
                ])
            },
        )
        .collect()
}

/// A scratch copy of code-bevy at `<scratch>/<dir>` without its
/// `expected.json`.
pub fn copy(scratch: &Scratch, dir: &str) -> PathBuf {
    crate::code_index_common::copy(scratch, FIXTURE, dir)
}

/// code-bevy's `[code]` replaced by `table`.
pub fn set_code(root: &Path, table: &str) {
    with_code_table(root, table);
}

/// Copies `from` to `to` creating every entry in reverse name order,
/// `expected.json` left out.
pub fn copy_reversed(from: &Path, to: &Path) {
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
        } else if entry.file_name() != "expected.json" {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}

/// Every `.rs` and `.ron` file under `root` (not `.git`), root-relative.
pub fn code_paths_under(root: &Path) -> Vec<String> {
    crate::common::snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some()
                && !path.starts_with(".git")
                && (path.ends_with(".rs") || path.ends_with(".ron"))
        })
        .map(|(path, _)| path)
        .collect()
}

/// A `spec index` text with its `db <path>` line (the data directory of
/// one `HOME`) masked.
pub fn mask_db(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("db ") {
                "db <db>".to_owned()
            } else {
                line.to_owned()
            }
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

/// A `spec --json index` document without its `db` key.
pub fn without_db(mut document: Value) -> Value {
    document
        .as_object_mut()
        .expect("an index object")
        .remove("db");
    document
}

/// One stderr line, `spec: ` dropped if the binary prefixes it.
pub fn cause(line: &str) -> &str {
    line.strip_prefix("spec: ").unwrap_or(line)
}
