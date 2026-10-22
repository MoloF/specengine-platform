//! A layer C snapshot written into a project's index for the MCP tests of
//! docs/features/layer-c-reads.md (11a-3): `spec` is not built for this
//! crate's tests and `spec resolve` needs the binary it spawns, so the rows
//! go in through the system's `sqlite3` in the canon's columns
//! (`docs/canon/layer-c.md` "Tables": `code_resolve`, `code_resolve_files`,
//! `code_monikers`; a column the store names otherwise fails the insert,
//! naming it), with the `tree_fp` canon "Freshness" defines computed here
//! from the index's rows and the tree's bytes (the hand-written BLAKE3 of
//! [`super::blake3`]), so the reads find the snapshot `fresh`; any edit of
//! the tree then makes it `stale`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use super::blake3::blake3_hex;

/// The crate word of every generated moniker.
pub const MONIKER_WORD: &str = "zzmoniker";

/// The names canon "Freshness" fingerprints in the root and each walked
/// file's directory up to it (`specengine_code::layer_c::TREE_FILES`).
const TREE_FILES: [&str; 6] = [
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    ".cargo/config",
    ".cargo/config.toml",
];

/// The stamps of the written run.
pub const RA_AP: &str = "0.0.352";
pub const HOST: &str = "masked";

/// The system's `sqlite3`.
fn sqlite3() -> &'static str {
    ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .expect("a sqlite3 client")
}

/// `sql` on `db` through `sqlite3 -json`: every row.
pub fn sqlite(db: &Path, sql: &str) -> Vec<BTreeMap<String, Value>> {
    let output = Command::new(sqlite3())
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
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    if text.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("sqlite3 -json: {error}\n{text}"))
}

/// `text` as an SQL string literal.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The layer C kind of a layer A item kind (`owner`: inside an `impl` or
/// a `trait`).
fn layer_c_kind(item_kind: &str, member: bool) -> &'static str {
    match item_kind {
        "function_item" if member => "method",
        "function_signature_item" => "method",
        "type_item" => "type_alias",
        "mod_item" => "mod",
        "macro_definition" => "macro_rules",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "union_item" => "union",
        "trait_item" => "trait",
        "impl_item" => "impl",
        "const_item" => "const",
        "static_item" => "static",
        _ => "function",
    }
}

/// canon `layer-c.md` "Freshness" over the index at `db` and the tree at
/// `root`: `b3:` of the entries and the stamps, each a JSON array,
/// byte-sorted, unique, joined by `\n`.
pub fn tree_fp(root: &Path, db: &Path) -> String {
    let files = sqlite(
        db,
        "SELECT path, lang, blake3, utf8, read_error FROM code_files ORDER BY path",
    );
    let mut entries: Vec<String> = Vec::new();
    let mut dirs: BTreeSet<String> = BTreeSet::from([String::new()]);
    for file in &files {
        let path = file["path"].as_str().expect("a path");
        let requested = file["lang"] == json!("rust")
            && file["utf8"] == json!(1)
            && file["read_error"].is_null()
            && file["blake3"].is_string();
        if requested {
            entries.push(json!(["file", path, file["blake3"]]).to_string());
        }
        let parts: Vec<&str> = path.split('/').collect();
        if parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
        {
            continue;
        }
        for depth in 1..parts.len() {
            dirs.insert(parts[..depth].join("/"));
        }
    }
    for package in sqlite(
        db,
        "SELECT dir, manifest_b3 FROM code_packages ORDER BY dir",
    ) {
        entries.push(json!(["package", package["dir"], package["manifest_b3"]]).to_string());
    }
    for dir in &dirs {
        for name in TREE_FILES {
            let relative = if dir.is_empty() {
                name.to_owned()
            } else {
                format!("{dir}/{name}")
            };
            let path = root.join(&relative);
            if path.is_file() {
                let bytes = fs::read(&path).expect("a tree file");
                entries.push(
                    json!(["bytes", relative, format!("b3:{}", blake3_hex(&bytes))]).to_string(),
                );
            }
        }
    }
    entries.push(json!(["protocol", 1]).to_string());
    entries.push(json!(["ra_ap", RA_AP]).to_string());
    entries.push(json!(["host", HOST]).to_string());
    entries.sort();
    entries.dedup();
    format!("b3:{}", blake3_hex(entries.join("\n").as_bytes()))
}

/// One layer A row of the index, as the writer reads it.
#[derive(Debug, Clone)]
pub struct Row {
    pub path: String,
    pub qpath: String,
    pub item_kind: String,
    pub start: u64,
    pub end: u64,
    pub member: bool,
    /// Its place among its file's rows, from 1, by start.
    pub nth: usize,
}

/// What [`write_snapshot`] wrote.
#[derive(Debug, Clone, Default)]
pub struct Written {
    /// `code_monikers` rows.
    pub items: u64,
    /// `code_resolve_files` rows.
    pub files: u64,
    /// The layer A rows by `(path, start)`, with their written moniker.
    pub monikers: BTreeMap<(String, u64), (String, Option<String>)>,
}

/// The plan of one row: its state and raw moniker, its `end` moved.
pub type Plan<'a> = &'a dyn Fn(&Row) -> (String, Option<String>, i64);

/// A fresh snapshot of the worktree in the index at `db` (the tree at
/// `root`): one `code_resolve` row (stamps [`RA_AP`], [`HOST`], one
/// workspace, toolchain `masked`), a `code_resolve_files` row per
/// requested `.rs` file (its stored BLAKE3), and a `code_monikers` row at
/// every layer A item's bytes (one per start) as `plan` gives it: its
/// state, its moniker and a shift of its `end`.
pub fn write_snapshot(root: &Path, db: &Path, plan: Plan<'_>) -> Written {
    let worktrees = sqlite(db, "SELECT wt FROM code_worktrees");
    assert_eq!(worktrees.len(), 1, "one code worktree: {worktrees:?}");
    let wt = worktrees[0]["wt"].as_i64().expect("wt");
    let files = sqlite(
        db,
        "SELECT path, blake3 FROM code_files WHERE lang = 'rust' AND utf8 = 1 \
         AND read_error IS NULL AND blake3 IS NOT NULL ORDER BY path",
    );
    let mut sql = vec!["BEGIN IMMEDIATE;".to_owned()];
    let mut written = Written::default();
    for file in &files {
        let path = file["path"].as_str().expect("a path");
        let blake3 = file["blake3"].as_str().expect("a blake3");
        sql.push(format!(
            "INSERT INTO code_resolve_files (wt, path, blake3, in_crate) VALUES ({wt}, {}, {}, 1);",
            quoted(path),
            quoted(blake3)
        ));
        written.files += 1;
        let rows = sqlite(
            db,
            &format!(
                "SELECT s.qpath AS qpath, s.start_byte AS start, s.end_byte AS \"end\", \
                 s.item_kind AS kind, s.owner AS owner \
                 FROM symbols s JOIN code_files f ON f.file_id = s.file \
                 WHERE f.path = {} ORDER BY s.start_byte, s.end_byte",
                quoted(path)
            ),
        );
        let mut last = None;
        let mut nth = 0;
        for row in rows {
            let start = row["start"].as_u64().expect("start_byte");
            if last == Some(start) {
                continue;
            }
            last = Some(start);
            nth += 1;
            let row = Row {
                path: path.to_owned(),
                qpath: row["qpath"].as_str().expect("qpath").to_owned(),
                item_kind: row["kind"].as_str().expect("item_kind").to_owned(),
                start,
                end: row["end"].as_u64().expect("end_byte"),
                member: !row["owner"].is_null(),
                nth,
            };
            let (state, moniker, shift) = plan(&row);
            let end = row.end.checked_add_signed(shift).expect("an end");
            sql.push(format!(
                "INSERT INTO code_monikers (wt, path, start, \"end\", kind, state, moniker) VALUES ({wt}, {}, {start}, {end}, '{}', {}, {});",
                quoted(path),
                layer_c_kind(&row.item_kind, row.member),
                quoted(&state),
                moniker.as_deref().map_or_else(|| "NULL".to_owned(), quoted)
            ));
            written
                .monikers
                .insert((path.to_owned(), start), (row.qpath.clone(), moniker));
            written.items += 1;
        }
    }
    sql.insert(
        1,
        format!(
            "INSERT INTO code_resolve (wt, tree_fp, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items) VALUES ({wt}, {}, 1, '{RA_AP}', '{HOST}', '[\"masked\"]', 0, 1, {}, {});",
            quoted(&tree_fp(root, db)),
            written.files,
            written.items
        ),
    );
    sql.push("COMMIT;".to_owned());
    let output = Command::new(sqlite3())
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(sql.join("\n"))
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "the snapshot's rows (the canon's columns): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    written
}

/// Every row a `moniker` of its own, `<MONIKER_WORD> m<start>.`.
pub fn plain(row: &Row) -> (String, Option<String>, i64) {
    (
        "moniker".to_owned(),
        Some(format!("{MONIKER_WORD} m{}.", row.start)),
        0,
    )
}
