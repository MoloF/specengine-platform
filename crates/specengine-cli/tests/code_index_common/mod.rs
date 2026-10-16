//! Helpers of the docs/features/symbol-index.md tests (slice 6a-2, the code
//! index): scratch copies of `fixtures/code-index` (Q24) and
//! `fixtures/code-a`, the `spec` binary with a scratch `HOME` per index
//! (`common::spec`), the store's canonical dump (`SqliteIndex::dump`, every
//! table, surrogate keys replaced: `dump.rs`), a fresh index in a new
//! `HOME` as the rebuild every incremental state must equal, and the code
//! tables read by column name through the system's `sqlite3` (`-json`), as
//! `drift_common::sql` reads the queue: this crate has no SQLite library
//! of its own (`build_graph.rs` `cli_reaches_rusqlite_only_through_the_store`).
//!
//! The column names are the spec's ("Data", "Tables"); a per-file row
//! reaches its `code_files` row through [`FILE_JOIN`].
//!
//! Only the code-index test binaries include this module
//! (`mod code_index_common;`, after `mod common;`).

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

use serde_json::Value;

use crate::common::{Run, Scratch, data_dir, fixture, read_text, spec};

/// The fixture of this slice (Q24) and its `[project] slug`.
pub const FIXTURE: &str = "code-index";
pub const SLUG: &str = "inkwell";

/// code-a and its slug: RD-8 reads it too (TE-1).
pub const CODE_A: &str = "code-a";
pub const CODE_A_SLUG: &str = "code-a";

/// verify-ecs, an ECS-style game crate: the second project of another
/// nature (ADR-0008), and its slug.
pub const ECS: &str = "verify-ecs";

/// Every code table of the spec's "Tables", as the dump and `sqlite_master`
/// name them.
pub const CODE_TABLES: [&str; 8] = [
    "code_worktrees",
    "code_files",
    "code_packages",
    "symbols",
    "symbol_suffixes",
    "code_decls",
    "code_markers",
    "code_mentions",
];

/// A per-file row's `file` is its `code_files` row's rowid (`file_id`)
/// (the spec keys `symbols`, `symbol_suffixes`, `code_decls`,
/// `code_markers`, `code_mentions` by `(file, ord)`).
pub const FILE_JOIN: &str = "JOIN code_files f ON f.file_id = x.file";

static FRESH: AtomicUsize = AtomicUsize::new(0);

/// The project's database under `home`.
pub fn db(home: &Path, slug: &str) -> PathBuf {
    data_dir(home).join(format!("{slug}.db"))
}

/// `fixtures/code-index/expected.json`.
pub fn expected() -> Value {
    serde_json::from_str(&read_text(&fixture(FIXTURE), "expected.json"))
        .expect("fixtures/code-index/expected.json is JSON")
}

/// A copy of `fixtures/<name>` at `<scratch>/<dir>` without its
/// `expected.json` (the fixture is only read).
pub fn copy(scratch: &Scratch, name: &str, dir: &str) -> PathBuf {
    let root = scratch.copy(name, dir);
    let _ = fs::remove_file(root.join("expected.json"));
    root
}

/// `spec index` in `root`, which must answer.
pub fn index(home: &Path, root: &Path) -> Run {
    let run = spec(home, root, &["index"]);
    run.code(0);
    run
}

/// The store's canonical dump of every worktree of `home`'s database of
/// `slug`, `root` (canonical) masked as `<root>`.
pub fn dump(home: &Path, root: &Path, slug: &str) -> String {
    let store = specengine_store::SqliteIndex::open(db(home, slug), slug, root)
        .unwrap_or_else(|error| panic!("open the index of {slug}: {error}"));
    let text = store.dump().expect("dump");
    let root_text = store.root().to_str().expect("a UTF-8 root").to_owned();
    text.replace(&root_text, "<root>")
}

/// The table a dump line belongs to.
fn table_of(line: &str) -> &str {
    line.split('\t').next().unwrap_or_default()
}

/// The dump lines of the code tables only.
pub fn code_lines(dump: &str) -> String {
    dump.lines()
        .filter(|line| CODE_TABLES.contains(&table_of(line)))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The dump lines of every other table (the md index).
pub fn md_lines(dump: &str) -> String {
    dump.lines()
        .filter(|line| !CODE_TABLES.contains(&table_of(line)))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The dump of a fresh index of `root` in a new `HOME` (`spec index`, exit
/// `code`): the rebuild an incremental state is compared with.
pub fn fresh_dump_with_exit(scratch: &Scratch, root: &Path, slug: &str, code: i32) -> String {
    let home = scratch.home(&format!("fresh-{}", FRESH.fetch_add(1, Ordering::SeqCst)));
    spec(&home, root, &["index"]).code(code);
    dump(&home, root, slug)
}

pub fn fresh_dump(scratch: &Scratch, root: &Path, slug: &str) -> String {
    fresh_dump_with_exit(scratch, root, slug, 0)
}

/// The lines only one side has, at most 40 of each.
pub fn dump_diff(have: &str, want: &str) -> String {
    let left: BTreeSet<&str> = have.lines().collect();
    let right: BTreeSet<&str> = want.lines().collect();
    let mut out = String::new();
    for line in left.difference(&right).take(40) {
        out.push_str(&format!("- incremental {line}\n"));
    }
    for line in right.difference(&left).take(40) {
        out.push_str(&format!("+ fresh       {line}\n"));
    }
    if out.is_empty() {
        out.push_str("(the same lines, another order or count)\n");
    }
    out
}

/// `home`'s index of `root` equals a fresh one, byte for byte.
pub fn assert_equals_fresh(scratch: &Scratch, home: &Path, root: &Path, slug: &str, step: &str) {
    let have = dump(home, root, slug);
    let want = fresh_dump(scratch, root, slug);
    assert!(
        have == want,
        "{step}: the index differs from a fresh rebuild:\n{}",
        dump_diff(&have, &want)
    );
}

// ------------------------------------------------------------ sqlite3

/// `sql` on `db` through the system's `sqlite3 -json`: every row as
/// column → value (none: empty).
pub fn query(db: &Path, sql: &str) -> Vec<BTreeMap<String, Value>> {
    let output = Command::new(crate::common::proposal::sqlite3())
        .env_clear()
        .args(["-json", "-cmd", ".timeout 5000"])
        .arg(db)
        .arg(sql)
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("sqlite3 prints UTF-8");
    if text.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("sqlite3 -json: {error}\n{text}"))
}

/// `sql` (a write) on `db` through the system's `sqlite3`.
pub fn execute(db: &Path, sql: &str) {
    let output = Command::new(crate::common::proposal::sqlite3())
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(sql)
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Every table of the database.
pub fn table_names(db: &Path) -> BTreeSet<String> {
    query(db, "SELECT name FROM sqlite_master WHERE type = 'table'")
        .into_iter()
        .map(|row| row["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// Rows of `table`.
pub fn count(db: &Path, table: &str) -> u64 {
    query(db, &format!("SELECT count(*) AS n FROM {table}"))[0]["n"]
        .as_u64()
        .expect("a count")
}

/// Every `symbols` row with its file's `path`, by (path, ord).
pub fn symbols(db: &Path) -> Vec<BTreeMap<String, Value>> {
    query(
        db,
        &format!("SELECT f.path AS path, x.* FROM symbols x {FILE_JOIN} ORDER BY f.path, x.ord"),
    )
}

/// Every `symbol_suffixes` row: (path, ord, depth, suffix), sorted.
pub fn suffixes(db: &Path) -> Vec<(String, u64, u64, String)> {
    query(
        db,
        &format!(
            "SELECT f.path AS path, x.ord AS ord, x.depth AS depth, x.suffix AS suffix \
             FROM symbol_suffixes x {FILE_JOIN} ORDER BY f.path, x.ord, x.depth"
        ),
    )
    .into_iter()
    .map(|row| {
        (
            row["path"].as_str().expect("path").to_owned(),
            row["ord"].as_u64().expect("ord"),
            row["depth"].as_u64().expect("depth"),
            row["suffix"].as_str().expect("suffix").to_owned(),
        )
    })
    .collect()
}

/// Every `code_files` row, by path.
pub fn code_files(db: &Path) -> Vec<BTreeMap<String, Value>> {
    query(db, "SELECT * FROM code_files ORDER BY path")
}

/// The paths of `code_files`.
pub fn code_paths(db: &Path) -> Vec<String> {
    code_files(db)
        .into_iter()
        .map(|row| row["path"].as_str().expect("path").to_owned())
        .collect()
}

// ------------------------------------------------- the `spec index` line

/// The `code <slug>: …` line of a `spec index` run, if any.
pub fn code_line(run: &Run) -> Option<String> {
    run.stdout
        .lines()
        .find(|line| line.starts_with("code "))
        .map(str::to_owned)
}

/// The counts of a `code <slug>: walked 41, parsed 41, …` line by word;
/// `, rederived all` reads as `rederived_all` 1.
pub fn code_counts(line: &str) -> BTreeMap<String, u64> {
    let (_, counts) = line.split_once(": ").expect("`code <slug>: …`");
    let mut out = BTreeMap::new();
    for part in counts.split(", ") {
        if part == "rederived all" {
            out.insert("rederived_all".to_owned(), 1);
            continue;
        }
        let (word, number) = part
            .rsplit_once(' ')
            .unwrap_or_else(|| panic!("`word N`: {part:?} in {line:?}"));
        out.insert(
            word.to_owned(),
            number
                .parse()
                .unwrap_or_else(|_| panic!("{number:?} in {line:?}")),
        );
    }
    out
}

/// The code line's counts of a `spec index` run that must print one.
pub fn counts_of(run: &Run) -> BTreeMap<String, u64> {
    let line = code_line(run).unwrap_or_else(|| panic!("no code line:\n{}", run.show()));
    code_counts(&line)
}

// --------------------------------------------------------------- edits

/// `relative` under `root` rewritten with CRLF line ends (LF ones first
/// folded, so a CRLF file stays as it is).
pub fn to_crlf(root: &Path, relative: &str) {
    let path = root.join(relative);
    let text = fs::read_to_string(&path).expect("a UTF-8 file");
    fs::write(&path, text.replace("\r\n", "\n").replace('\n', "\r\n")).expect("write");
}

/// `relative` under `root` rewritten with LF line ends.
pub fn to_lf(root: &Path, relative: &str) {
    let path = root.join(relative);
    let text = fs::read_to_string(&path).expect("a UTF-8 file");
    fs::write(&path, text.replace("\r\n", "\n")).expect("write");
}

/// `from` replaced by `to` (of the same length) in `relative`, the file's
/// size and modification time kept: only the bytes tell.
pub fn same_size_same_mtime_edit(root: &Path, relative: &str, from: &str, to: &str) {
    assert_eq!(from.len(), to.len(), "a same-size edit");
    let path = root.join(relative);
    let before = fs::metadata(&path).expect("metadata");
    let mtime: SystemTime = before.modified().expect("mtime");
    let text = fs::read_to_string(&path).expect("read");
    assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?} once");
    fs::write(&path, text.replacen(from, to, 1)).expect("write");
    let file = fs::File::options()
        .write(true)
        .open(&path)
        .expect("open for set_modified");
    file.set_modified(mtime).expect("set_modified");
    drop(file);
    let after = fs::metadata(&path).expect("metadata");
    assert_eq!(after.len(), before.len(), "{relative}: the size is kept");
    assert_eq!(
        after.modified().expect("mtime"),
        mtime,
        "{relative}: the mtime is kept"
    );
}

/// `root`'s config with its `[code]` table (the config's last) replaced by
/// `code` (`""`: no `[code]` table).
pub fn with_code_table(root: &Path, code: &str) {
    let text = read_text(root, "specengine.toml");
    let at = text.find("\n[code]\n").expect("a [code] table") + 1;
    fs::write(
        root.join("specengine.toml"),
        format!("{}{code}", &text[..at]),
    )
    .expect("write the config");
}

/// `line` (one `PREFIX = { … }` entry) added as the last line of `[ids]`.
pub fn with_ids_line(root: &Path, line: &str) {
    let text = read_text(root, "specengine.toml");
    fs::write(
        root.join("specengine.toml"),
        crate::common::with_ids_line(&text, line),
    )
    .expect("write the config");
}

/// The `spec refs REF` rows of a text answer: every line after the header
/// (the node's own line), the summary included.
pub fn refs_rows(stdout: &str) -> Vec<String> {
    stdout.lines().skip(1).map(str::to_owned).collect()
}
