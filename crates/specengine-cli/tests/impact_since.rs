//! docs/features/impact-since.md (7b-2): `spec impact --since REV [--depth
//! N] [--json]`, the radius of the code changed since a revision. REV's
//! tree under today's `[code]` is compared with the files on disk,
//! untracked ones included, item by item on `sig` and `body` (a RON file
//! one record over its bytes); the changed and removed items
//! reach the nodes REV's markers bind, the changed and added ones the
//! disk's, then 7b-1's walk, rows and tasks. RD-1 to RD-4 and RD-6 to RD-8
//! (each test names the mutation that turns it red), TE-1 (the scripted
//! history), TE-2 (determinism), TE-4 (`fixtures/spec-c`, outside git) and
//! TE-6 (a `git` shim on `PATH` logging argv). RD-5: `verify_delta.rs`;
//! TE-3: `impact_cost.rs`; the pure comparer: core `tests/symbol_delta.rs`.
//!
//! **The history** ([`history`]): a scratch copy of `fixtures/impact/`
//! committed as "the fixture", then
//! - `older` adds `harbour.rs` (a `NOTE-001` marker on `berths`),
//!   `tolls.rs`, `data/tides.ron` and a second package `crates/quay`, and
//!   is tagged `cut` (annotated);
//! - the branch `side`, from `older`: `dock_time`'s body changed and
//!   `tide_count` added;
//! - `HEAD` on `main`, from `older`: `late_fare`'s body changed,
//!   `fares::rows` removed, `board::width` added;
//! - the disk, over `HEAD`: a comment inside `closes`, a new marker above
//!   `dock_time`, `Crossing::opens`' body edited (a method: it and its
//!   `impl`), `board::reach` reformatted, an untracked `pier.rs` (a
//!   `NOTE-002` marker), `harbour.rs` deleted, `tolls.rs` moved to
//!   `fees/tolls.rs` (a module move), `crates/quay` moved to
//!   `crates/wharf` with its manifest renamed (its files one-sided), a RON
//!   value edit under `@configures FARE-01` (`data/fares.ron`) and a RON
//!   comment edit (`data/tides.ron`).
//!
//! Its delta, file count and seeds per REV are written here by hand from
//! docs/features/impact-since.md "Compared", "Core comparer", "Seeds" and
//! "Output" ([`at_head`], [`at_older`], [`at_side`]).

#![cfg(unix)]

mod bindings_common;
mod code_index_common;
mod common;
mod lock_common;
mod verify_common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use bindings_common::*;
use common::snapshot;
use lock_common::LockRepo;
use serde_json::{Value, json};
use verify_common::clone_dir;

/// The fixture and its slug.
const FIXTURE: &str = "impact";
const SLUG: &str = "ferry-lines";

/// docs/features/impact-since.md "Command and guard": not exactly one
/// input, exit 2, stdout empty, before discovery.
const ONE_INPUT: &str = "spec: impact takes exactly one of REF, --symbol and --since";

/// `code-bindings.md` "The `[code]` table": a config without the table.
const NO_CODE: &str =
    "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The tail's narrowing under `--since` (docs/features/impact-since.md
/// "Cut").
const SINCE_TAIL: &str = " not shown; lower --depth N or name a later --since REV]";

/// The top-level keys of the document (docs/canon/spec-impact.md "Output";
/// `tested` after `nodes` since docs/features/tested-reads.md "Per read").
const TOP_KEYS: [&str; 15] = [
    "input",
    "reason",
    "revision",
    "depth",
    "seeds",
    "delta",
    "left_out",
    "nodes",
    "tested",
    "symbols",
    "tests",
    "tasks",
    "notes",
    "truncated",
    "code",
];

/// `delta`'s keys, in order (docs/features/impact-since.md "Output").
const DELTA_KEYS: [&str; 4] = ["added", "removed", "changed", "renamed"];

/// A delta item's keys, in order.
const ITEM_KEYS: [&str; 4] = ["file", "symbol", "item_kind", "line"];

// ------------------------------------------------------------- the files

const LIB: &str = "crates/ferry/src/lib.rs";
const BOARD: &str = "crates/ferry/src/board.rs";
const CROSSING: &str = "crates/ferry/src/crossing.rs";
const FARES: &str = "crates/ferry/src/fares.rs";
const TESTS: &str = "crates/ferry/tests/crossing.rs";
const HARBOUR: &str = "crates/ferry/src/harbour.rs";
const TOLLS: &str = "crates/ferry/src/tolls.rs";
const MOVED_TOLLS: &str = "crates/ferry/src/fees/tolls.rs";
const PIER: &str = "crates/ferry/src/pier.rs";
const QUAY: &str = "crates/quay/src/lib.rs";
const WHARF: &str = "crates/wharf/src/lib.rs";
const FARES_RON: &str = "data/fares.ron";
const TIDES_RON: &str = "data/tides.ron";

/// `older`'s harbour: `berths` bound to `NOTE-001`.
const HARBOUR_RS: &str = "//! The harbour.\n\n// @implements NOTE-001\npub fn berths() -> u32 {\n    6\n}\n\npub fn cranes() -> u32 {\n    2\n}\n";

/// `older`'s tolls, moved on disk unchanged.
const TOLLS_RS: &str = "//! Tolls at the pier.\n\npub fn toll() -> u32 {\n    1\n}\n\npub struct Gate {\n    pub open: bool,\n}\n";

/// `older`'s tides, a RON file whose comment the disk edits.
const TIDES: &str = "// Tides of the strait.\nTides(\n    spring: 4,\n)\n";

/// The disk's untracked pier: `berth` bound to `NOTE-002`.
const PIER_RS: &str =
    "//! The pier.\n\n// @implements NOTE-002\npub fn berth() -> u32 {\n    1\n}\n";

/// The second package's library, renamed with its package on disk.
const QUAY_RS: &str = "//! Where the ferry moors.\n\npub fn moor() -> u32 {\n    1\n}\n";

/// A package manifest named `name`.
fn manifest(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n"
    )
}

// ------------------------------------------------------------ the oracle

/// One expected delta item: a Rust item (file, qpath, item_kind, line), or
/// a RON file's one record (`symbol` its path, `item_kind` `file`, `line`
/// 1).
#[derive(Debug, Clone, Copy)]
enum Want {
    Item(&'static str, &'static str, &'static str, u64),
    Whole(&'static str),
}

impl Want {
    fn json(self) -> Value {
        match self {
            Want::Item(file, symbol, kind, line) => {
                json!({"file": file, "symbol": symbol, "item_kind": kind, "line": line})
            }
            Want::Whole(file) => {
                json!({"file": file, "symbol": file, "item_kind": "file", "line": 1})
            }
        }
    }
}

/// One REV's expected delta, compared-file count and seeds.
#[derive(Debug, Clone)]
struct Expected {
    added: Vec<Want>,
    removed: Vec<Want>,
    changed: Vec<Want>,
    files: u64,
    seeds: Vec<&'static str>,
}

impl Expected {
    fn delta(&self) -> Value {
        let all = |items: &[Want]| Value::Array(items.iter().map(|item| item.json()).collect());
        json!({
            "added": all(&self.added),
            "removed": all(&self.removed),
            "changed": all(&self.changed),
            "renamed": null,
        })
    }
}

/// The delta against `HEAD`: the disk's edits only. Compared files:
/// crossing.rs, board.rs, pier.rs, harbour.rs, tolls.rs, fees/tolls.rs,
/// quay's and wharf's lib.rs, the two RON files.
fn at_head() -> Expected {
    Expected {
        added: vec![
            Want::Item(
                MOVED_TOLLS,
                "crates/ferry::fees::tolls::toll",
                "function_item",
                3,
            ),
            Want::Item(
                MOVED_TOLLS,
                "crates/ferry::fees::tolls::Gate",
                "struct_item",
                7,
            ),
            Want::Item(PIER, "crates/ferry::pier::berth", "function_item", 4),
            Want::Item(WHARF, "crates/wharf::moor", "function_item", 3),
        ],
        removed: vec![
            Want::Item(HARBOUR, "crates/ferry::harbour::berths", "function_item", 4),
            Want::Item(HARBOUR, "crates/ferry::harbour::cranes", "function_item", 8),
            Want::Item(TOLLS, "crates/ferry::tolls::toll", "function_item", 3),
            Want::Item(TOLLS, "crates/ferry::tolls::Gate", "struct_item", 7),
            Want::Item(QUAY, "crates/quay::moor", "function_item", 3),
        ],
        changed: vec![
            Want::Item(
                CROSSING,
                "crates/ferry::crossing::impl Crossing",
                "impl_item",
                10,
            ),
            Want::Item(
                CROSSING,
                "crates/ferry::crossing::Crossing::opens",
                "function_item",
                11,
            ),
            Want::Whole(FARES_RON),
            Want::Whole(TIDES_RON),
        ],
        files: 10,
        // `berths` removed: REV's marker; `berth` added: the disk's; the
        // method's `impl` marker; `data/fares.ron`'s `@configures`.
        seeds: vec!["FARE-01", "NOTE-001", "NOTE-002", "TIDE-WINDOW"],
    }
}

/// The delta against `older` (and its tag `cut`): `HEAD`'s commit too;
/// fares.rs compared as well.
fn at_older() -> Expected {
    let head = at_head();
    let mut added = vec![Want::Item(
        BOARD,
        "crates/ferry::board::width",
        "function_item",
        10,
    )];
    added.extend(head.added);
    let mut removed = vec![Want::Item(
        FARES,
        "crates/ferry::fares::rows",
        "function_item",
        13,
    )];
    removed.extend(head.removed);
    let mut changed = head.changed[..2].to_vec();
    changed.push(Want::Item(
        FARES,
        "crates/ferry::fares::late_fare",
        "function_item",
        9,
    ));
    changed.extend_from_slice(&head.changed[2..]);
    Expected {
        added,
        removed,
        changed,
        files: 11,
        // `late_fare` changed (its marker on both sides); `width` added
        // inside board.rs's file module (the disk's `//!` marker).
        seeds: vec![
            "FARE-01",
            "FARE-02",
            "NOTE-001",
            "NOTE-002",
            "TIDE-BOARD",
            "TIDE-WINDOW",
        ],
    }
}

/// The delta against `side`: `older`'s, plus `side`'s own commit undone.
fn at_side() -> Expected {
    let older = at_older();
    let mut removed = vec![Want::Item(
        CROSSING,
        "crates/ferry::crossing::tide_count",
        "function_item",
        44,
    )];
    removed.extend(older.removed);
    let mut changed = older.changed[..2].to_vec();
    changed.push(Want::Item(
        CROSSING,
        "crates/ferry::crossing::dock_time",
        "function_item",
        37,
    ));
    changed.extend_from_slice(&older.changed[2..]);
    Expected {
        added: older.added,
        removed,
        changed,
        files: 11,
        // `dock_time` changed: the disk's new marker; REV's comment there
        // only mentions ROUTE-DOCKS (a mention never seeds).
        seeds: vec![
            "FARE-01",
            "FARE-02",
            "NOTE-001",
            "NOTE-002",
            "ROUTE-DOCKS",
            "TIDE-BOARD",
            "TIDE-WINDOW",
        ],
    }
}

/// An empty delta over `files` compared files.
fn nothing(files: u64) -> Expected {
    Expected {
        added: vec![],
        removed: vec![],
        changed: vec![],
        files,
        seeds: vec![],
    }
}

// --------------------------------------------------------------- runs

/// `spec impact <args>` (text, then `--json`) from `cwd` with `vars`.
fn impact_in(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Read {
    let mut all = vec!["impact"];
    all.extend_from_slice(args);
    read_with(cwd, &all, vars)
}

/// `spec impact --since=<rev>` from the root (`=`: a REV may start with
/// `-`).
fn since(repo: &LockRepo, rev: &str) -> Read {
    since_in(&repo.root, rev, &vars_of(repo))
}

fn since_in(cwd: &Path, rev: &str, vars: &[(OsString, OsString)]) -> Read {
    let arg = format!("--since={rev}");
    impact_in(cwd, &[&arg], vars)
}

/// `spec verify --changed --json`'s document from the root (any verdict:
/// exit 2 for `cannot_verify` rows prints it too).
fn verify_changed(repo: &LockRepo, vars: &[(OsString, OsString)]) -> Value {
    let run = run_with(&repo.root, &["verify", "--changed", "--json"], vars);
    assert!(
        [0, 1, 2].contains(&run.code) && !run.stdout.is_empty(),
        "premise: verify --changed answers\n{}",
        run.show()
    );
    common::one_json_document(&run.stdout).expect("verify's JSON")
}

/// The names of `seeds.nodes`.
fn seed_names(document: &Value) -> BTreeSet<String> {
    list(&document["seeds"]["nodes"])
        .iter()
        .map(|seed| text(seed).to_owned())
        .collect()
}

/// A graph node's name: its ID, else its path.
fn name_of(node: &Value) -> String {
    node["id"]
        .as_str()
        .unwrap_or_else(|| text(&node["path"]))
        .to_owned()
}

/// The keys of the object at `path` (each key holding an object), in the
/// order printed.
fn keys_in_order(raw: &str, path: &[&str]) -> Vec<String> {
    let mut at = 0;
    for key in path {
        let needle = format!("\"{key}\":{{");
        at += raw[at..]
            .find(&needle)
            .unwrap_or_else(|| panic!("`{key}` holds an object in {raw}"))
            + needle.len()
            - 1;
    }
    raw_order(raw, at)
}

/// A process's variables.
type Vars = Vec<(OsString, OsString)>;

/// A named place to run from: its label, its cwd, its variables.
type Place = (&'static str, PathBuf, Vars);

/// The full OID `rev` names in `repo`.
fn oid(repo: &LockRepo, rev: &str) -> String {
    repo.git_text(&["rev-parse", "--verify", &format!("{rev}^{{commit}}")])
}

/// The disk OID of `relative` (raw bytes, nothing written).
fn disk_oid(repo: &LockRepo, relative: &str) -> String {
    repo.git_text(&["hash-object", "--no-filters", "--", relative])
}

// ------------------------------------------------------------ the history

/// The scripted history (module docs): its repository and REVs.
struct History {
    repo: LockRepo,
    older: String,
    side: String,
    head: String,
}

/// `older` over the committed fixture, tagged `cut`.
fn commit_older(repo: &LockRepo) -> String {
    repo.write(HARBOUR, HARBOUR_RS);
    repo.write(TOLLS, TOLLS_RS);
    repo.write(TIDES_RON, TIDES);
    repo.write("crates/quay/Cargo.toml", manifest("quay"));
    repo.write(QUAY, QUAY_RS);
    repo.commit_all("older");
    repo.git(&["tag", "-a", "-m", "the cut", "cut"]);
    repo.head()
}

/// `side`'s edits over `older`.
fn side_edits(repo: &LockRepo) {
    repo.replace(
        CROSSING,
        "pub fn dock_time() -> u32 {\n    3\n}",
        "pub fn dock_time() -> u32 {\n    4\n}",
    );
    repo.append(CROSSING, "\npub fn tide_count() -> u32 {\n    2\n}\n");
}

/// `HEAD`'s edits over `older`.
fn head_edits(repo: &LockRepo) {
    repo.replace(FARES, "    crossings * 2\n", "    crossings * 3\n");
    repo.replace(FARES, "\npub fn rows() -> u32 {\n    2\n}\n", "");
    repo.append(BOARD, "\npub fn width() -> u32 {\n    7\n}\n");
}

/// The disk's edits over `HEAD`.
fn disk_edits(root: &Path) {
    let replace = |relative: &str, from: &str, to: &str| common::replace(root, relative, from, to);
    // A comment inside `closes`: no change (A3), the lines below move.
    replace(
        CROSSING,
        "        self.close\n",
        "        // the close bell\n        self.close\n",
    );
    // A marker only: no change, seeds nothing (A3).
    replace(
        CROSSING,
        "// Waits at ROUTE-DOCKS between crossings.\n",
        "// Waits at ROUTE-DOCKS between crossings.\n// @implements ROUTE-DOCKS\n",
    );
    // A method's body: it and its `impl`.
    replace(CROSSING, "        self.open\n", "        self.open + 1\n");
    // Formatting only.
    replace(
        BOARD,
        "pub fn reach() -> u32 {\n    4\n}",
        "pub fn reach() -> u32 { 4 }",
    );
    // Untracked, deleted, a module moved, a package renamed.
    common::write(root, PIER, PIER_RS);
    fs::remove_file(root.join(HARBOUR)).expect("harbour.rs deleted");
    common::write(root, MOVED_TOLLS, TOLLS_RS);
    fs::remove_file(root.join(TOLLS)).expect("tolls.rs moved");
    fs::remove_dir_all(root.join("crates/quay")).expect("quay renamed");
    common::write(root, "crates/wharf/Cargo.toml", manifest("wharf"));
    common::write(root, WHARF, QUAY_RS);
    // A RON value, a RON comment.
    replace(FARES_RON, "    late: 2,\n", "    late: 3,\n");
    replace(
        TIDES_RON,
        "// Tides of the strait.\n",
        "// Tides of the narrow strait.\n",
    );
}

/// The history at `<scratch>/<dir>`.
fn history_at(label: &str, dir: &str) -> History {
    let repo = LockRepo::of(label, FIXTURE, dir, &[]);
    let older = commit_older(&repo);
    repo.git(&["checkout", "-q", "-b", "side"]);
    side_edits(&repo);
    repo.commit_all("side");
    let side = repo.head();
    repo.git(&["checkout", "-q", "main"]);
    head_edits(&repo);
    repo.commit_all("head");
    let head = repo.head();
    disk_edits(&repo.root);
    History {
        repo,
        older,
        side,
        head,
    }
}

fn history(label: &str) -> History {
    history_at(label, "repo")
}

// ------------------------------------------------------------ assertions

/// `item` for messages.
fn shown_item(item: &Value) -> String {
    format!(
        "{} | {} | {} | {}",
        text(&item["file"]),
        item["symbol"],
        item["item_kind"],
        item["line"]
    )
}

/// The comparer's sort key: file bytes, line, qpath, item_kind
/// (docs/features/impact-since.md "Core comparer").
fn order_key(item: &Value) -> (Vec<u8>, u64, Vec<u8>, Vec<u8>) {
    (
        text(&item["file"]).as_bytes().to_vec(),
        item["line"].as_u64().expect("a line"),
        text(&item["symbol"]).as_bytes().to_vec(),
        text(&item["item_kind"]).as_bytes().to_vec(),
    )
}

/// Every item of a delta list keyed as "Output" says, the list in the
/// comparer's order.
#[track_caller]
fn assert_well_formed(items: &[Value], what: &str) {
    for item in items {
        let mut keys: Vec<String> = item
            .as_object()
            .unwrap_or_else(|| panic!("{what}: an item object, not {item}"))
            .keys()
            .cloned()
            .collect();
        keys.sort();
        let mut want: Vec<String> = ITEM_KEYS.iter().map(|key| (*key).to_owned()).collect();
        want.sort();
        assert_eq!(keys, want, "{what}: an item's keys: {item}");
    }
    let keys: Vec<_> = items.iter().map(order_key).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "{what}: by file bytes, line, symbol, item_kind"
    );
}

/// The delta of a document against `want`: its keys in order, `renamed`
/// `null`, each list value for value and in order.
#[track_caller]
fn assert_delta(raw: &str, delta: &Value, want: &Expected, what: &str) {
    assert_eq!(
        keys_in_order(raw, &["delta"]),
        DELTA_KEYS,
        "{what}: delta's keys in order"
    );
    for key in ["added", "removed", "changed"] {
        assert_well_formed(&list(&delta[key]), &format!("{what}: {key}"));
    }
    let shown = |value: &Value| -> Vec<String> { list(value).iter().map(shown_item).collect() };
    let expected = want.delta();
    for key in ["added", "removed", "changed"] {
        assert_eq!(
            delta[key],
            expected[key],
            "{what}: {key}\nhave {:#?}\nwant {:#?}",
            shown(&delta[key]),
            shown(&expected[key])
        );
    }
    assert_eq!(delta["renamed"], Value::Null, "{what}: renamed null");
}

/// A delta item's text line (docs/features/impact-since.md "Output"):
/// `<state> | <file>:<line> | <symbol> <item_kind>`.
fn delta_line(state: &str, item: &Value) -> String {
    format!(
        "{state} | {}:{} | {} {}",
        text(&item["file"]),
        item["line"],
        text(&item["symbol"]),
        text(&item["item_kind"])
    )
}

/// The delta's lines of a document: `changed`, then `added`, then
/// `removed`, each in the JSON's order.
fn delta_lines(document: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for state in ["changed", "added", "removed"] {
        for item in list(&document["delta"][state]) {
            lines.push(delta_line(state, &item));
        }
    }
    lines
}

/// The header (docs/features/impact-since.md "Output"): `since <7 hex>:
/// <f> files, <c> changed, <a> added, <r> removed`, the totals.
fn header(full: &str, files: u64, delta: &Value) -> String {
    let count = |key: &str| list(&delta[key]).len();
    format!(
        "since {}: {files} files, {} changed, {} added, {} removed",
        &full[..7],
        count("changed"),
        count("added"),
        count("removed")
    )
}

/// An answered, uncut `--since` read against `want`: exit 0; every top key
/// in order; `input`, `reason`, `revision` (the full OID), `seeds.symbols`
/// `null`, `truncated` `null`; the delta; the header first and the
/// summary next; the delta's lines last, exactly the JSON's items in
/// "Output"'s order; `seeds.nodes` the distance-0 nodes, `want`'s seeds.
#[track_caller]
fn assert_since(read: &Read, rev: &str, full: &str, want: &Expected, what: &str) -> Value {
    read.answered();
    let document = read.json();
    let raw = &read.json_run.stdout;
    assert_eq!(raw_order(raw, 0), TOP_KEYS, "{what}: the top keys");
    assert_eq!(
        document["input"],
        json!({"ref": null, "symbol": null, "since": rev}),
        "{what}: input, REV as given"
    );
    assert_eq!(document["reason"], Value::Null, "{what}: reason");
    assert_eq!(document["revision"], json!(full), "{what}: the full OID");
    assert_eq!(
        document["seeds"]["symbols"],
        Value::Null,
        "{what}: seeds.symbols null under --since"
    );
    assert_eq!(document["truncated"], Value::Null, "{what}: uncut");
    assert_delta(raw, &document["delta"], want, what);

    let lines: Vec<&str> = read.text.stdout.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(header(full, want.files, &document["delta"]).as_str()),
        "{what}: the header\n{}",
        read.show()
    );
    assert!(
        lines
            .get(1)
            .is_some_and(|line| line.starts_with("impact: nodes ")),
        "{what}: the summary second\n{}",
        read.show()
    );
    let wanted = delta_lines(&document);
    let tail: Vec<&str> = lines[lines.len() - wanted.len()..].to_vec();
    assert_eq!(
        tail,
        wanted,
        "{what}: the delta's lines last\n{}",
        read.show()
    );
    assert_eq!(
        seed_names(&document),
        owned(&want.seeds),
        "{what}: seeds.nodes"
    );
    let distance_0: BTreeSet<String> = list(&document["nodes"])
        .iter()
        .filter(|node| node["distance"] == 0)
        .map(name_of)
        .collect();
    assert_eq!(
        seed_names(&document),
        distance_0,
        "{what}: the seeds are the distance-0 nodes"
    );
    document
}

/// Exit 2, stdout empty (text and JSON), no panic.
#[track_caller]
fn assert_cannot_run(read: &Read, what: &str) {
    assert_eq!(read.code(), 2, "{what}\n{}", read.show());
    assert_eq!(read.text.stdout, "", "{what}: stdout empty");
    assert_eq!(read.json_run.stdout, "", "{what}: stdout empty with --json");
    assert!(
        !read.text.stderr.contains("panicked"),
        "{what}: no panic\n{}",
        read.show()
    );
}

/// [`assert_cannot_run`] with `line` on stderr.
#[track_caller]
fn assert_usage(read: &Read, line: &str, what: &str) {
    assert_cannot_run(read, what);
    assert!(
        read.text.stderr.lines().any(|printed| printed == line),
        "{what}: want {line:?} on stderr\n{}",
        read.show()
    );
}

// ------------------------------------------------------------- the shim

/// A `git` first on `PATH` that appends its argv (each argument followed by
/// U+001F, one line per process, one write) to a log, then runs the real
/// git (TE-6).
struct Shim {
    dir: PathBuf,
    log: PathBuf,
}

impl Shim {
    fn new(repo: &LockRepo, name: &str) -> Self {
        let dir = repo.scratch.dir(name);
        let log = dir.join("argv.log");
        let real = repo.git.git_program().to_str().expect("a UTF-8 git path");
        let script = format!(
            "#!/bin/sh\nline=$(printf '%s\\037' \"$@\")\nprintf '%s\\n' \"$line\" >> '{}'\nexec '{real}' \"$@\"\n",
            log.to_str().expect("a UTF-8 scratch path")
        );
        let shim = dir.join("git");
        fs::write(&shim, script).expect("the shim");
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
        Self { dir, log }
    }

    /// `vars` with the shim first on `PATH`.
    fn over(&self, repo: &LockRepo, vars: &[(OsString, OsString)]) -> Vec<(OsString, OsString)> {
        with_var(vars, "PATH", &repo.git.path_with(&self.dir))
    }

    /// `repo`'s variables with the shim first on `PATH`.
    fn vars(&self, repo: &LockRepo) -> Vec<(OsString, OsString)> {
        self.over(repo, &vars_of(repo))
    }

    /// Every logged call, its arguments from the subcommand on: git's
    /// global options before it (`-c <name=value>`, `-C <dir>`, `--...`)
    /// dropped.
    fn calls(&self) -> Vec<Vec<String>> {
        let log = fs::read_to_string(&self.log).unwrap_or_default();
        log.lines()
            .map(|line| {
                let mut args: Vec<String> = line.split('\u{1f}').map(str::to_owned).collect();
                if args.last().is_some_and(String::is_empty) {
                    args.pop();
                }
                let mut at = 0;
                while let Some(arg) = args.get(at) {
                    match arg.as_str() {
                        "-c" | "-C" => at += 2,
                        global if global.starts_with('-') => at += 1,
                        _ => break,
                    }
                }
                args.split_off(at.min(args.len()))
            })
            .collect()
    }

    fn clear(&self) {
        let _ = fs::remove_file(&self.log);
    }
}

/// A lower-case hex OID.
fn is_oid(text: &str) -> bool {
    (text.len() == 40 || text.len() == 64)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether a call is through the door (docs/features/impact-since.md
/// "Rules and edge cases"): `rev-parse` (7b-1's tasks probe among them),
/// `ls-tree -r -z <oid>`, `cat-file --batch`, `hash-object --no-filters
/// --stdin-paths`; nothing else.
fn through_the_door(call: &[String]) -> bool {
    let args: Vec<&str> = call.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["rev-parse", rest @ ..] => !rest.is_empty(),
        ["ls-tree", "-r", "-z", oid] => is_oid(oid),
        ["cat-file", "--batch"] => true,
        ["hash-object", "--no-filters", "--stdin-paths"] => true,
        _ => false,
    }
}

// ----------------------------------------------------- what reads leave

fn index_mtime(root: &Path) -> Option<(i64, i64)> {
    fs::metadata(root.join(".git").join("index"))
        .ok()
        .map(|meta| (meta.mtime(), meta.mtime_nsec()))
}

/// `git status` without the optional index refresh.
fn status(repo: &LockRepo) -> Vec<u8> {
    repo.git(&[
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ])
}

fn objects(root: &Path) -> usize {
    snapshot(&root.join(".git").join("objects"))
        .values()
        .filter(|bytes| bytes.is_some())
        .count()
}

/// Every byte under the root, `.git` included, the index's mtime, the
/// object count and `git status`.
struct Untouched {
    files: BTreeMap<String, Option<Vec<u8>>>,
    index_mtime: Option<(i64, i64)>,
    objects: usize,
    status: Vec<u8>,
}

impl Untouched {
    fn take(repo: &LockRepo) -> Self {
        let status = status(repo);
        Self {
            files: snapshot(&repo.root),
            index_mtime: index_mtime(&repo.root),
            objects: objects(&repo.root),
            status,
        }
    }

    #[track_caller]
    fn still(&self, repo: &LockRepo, what: &str) {
        assert_eq!(
            index_mtime(&repo.root),
            self.index_mtime,
            "{what}: .git/index's mtime"
        );
        assert_eq!(objects(&repo.root), self.objects, "{what}: git objects");
        let files = snapshot(&repo.root);
        let moved: Vec<&String> = self
            .files
            .keys()
            .chain(files.keys())
            .filter(|path| self.files.get(*path) != files.get(*path))
            .collect();
        assert!(
            moved.is_empty(),
            "{what} wrote under the root (.git included): {moved:?}"
        );
        assert_eq!(status(repo), self.status, "{what}: git status");
    }
}

// ------------------------------------------------------------------ RD-1

/// The guard's line for a REV printed as `shown`.
fn refused(shown: &str) -> String {
    format!(
        "spec: --since `{shown}` is refused: a revision never starts with `-` or holds a control character"
    )
}

/// RD-1, TE-6 (docs/features/impact-since.md "Command and guard"): a REV
/// with a leading `-` or any control character (U+0000-U+001F, U+007F),
/// given as `--since=<REV>` and as `--since <REV>` (`allow_hyphen_values`),
/// exits 2 with stdout empty (text and JSON) and the guard's line, every
/// control character of REV (LF, CR and TAB too) written `\u{XX}`, and
/// the logging `git` shim records no call at all, in the project and
/// outside any project (the guard comes before discovery). A REV the
/// guard lets through (`HEAD~0`, a non-ASCII letter) reaches git. M1: the
/// guard removed (the store refusing after `open`).
#[test]
fn option_like_since_is_refused_before_git() {
    let repo = LockRepo::of("is-guard", FIXTURE, "repo", &[]);
    let shim = Shim::new(&repo, "shim");
    let vars = shim.vars(&repo);
    let nowhere = repo.scratch.dir("no-project");
    let escaped: [(&str, &str); 15] = [
        ("-x", "-x"),
        ("-", "-"),
        ("--all", "--all"),
        ("--output=/dev/null", "--output=/dev/null"),
        ("-HEAD", "-HEAD"),
        ("a\u{1}b", "a\\u{1}b"),
        ("a\u{1f}b", "a\\u{1f}b"),
        ("a\u{7f}b", "a\\u{7f}b"),
        ("\u{1b}[31mHEAD", "\\u{1b}[31mHEAD"),
        ("HEAD\r", "HEAD\\u{d}"),
        ("HEAD\u{b}", "HEAD\\u{b}"),
        ("main\u{c}", "main\\u{c}"),
        ("a\tb", "a\\u{9}b"),
        ("HEAD\nHEAD", "HEAD\\u{a}HEAD"),
        ("\nHEAD", "\\u{a}HEAD"),
    ];
    for (rev, shown) in escaped {
        for cwd in [&repo.root, &nowhere] {
            shim.clear();
            let read = since_in(cwd, rev, &vars);
            assert_usage(&read, &refused(shown), &format!("--since={rev:?}"));
            assert_eq!(
                shim.calls(),
                Vec::<Vec<String>>::new(),
                "--since={rev:?}: no git child"
            );
            shim.clear();
            let read = impact_in(cwd, &["--since", rev], &vars);
            assert_usage(&read, &refused(shown), &format!("--since {rev:?}"));
            assert_eq!(
                shim.calls(),
                Vec::<Vec<String>>::new(),
                "--since {rev:?}: no git child"
            );
        }
    }
    // Let through: git answers.
    shim.clear();
    since_in(&repo.root, "HEAD~0", &vars).answered();
    assert!(!shim.calls().is_empty(), "premise: the shim logs git calls");
    let other = since_in(&repo.root, "ma\u{ef}n", &vars);
    assert_eq!(other.code(), 1, "{}", other.show());
}

// ------------------------------------------------------------------ RD-2

/// The exit-1 answer of a REV naming no commit
/// (docs/features/impact-since.md "Command and guard"): stdout empty, ``spec: --since `<REV>` is not a
/// commit`` the last stderr line, the exit-1 JSON (that `reason`,
/// `input.since` REV as given, `revision` and `delta` `null`, `left_out`
/// zeros, `nodes`, `tasks` and `seeds.nodes` `[]`, the rest `null`).
#[track_caller]
fn assert_no_commit(repo: &LockRepo, rev: &str) {
    let read = since(repo, rev);
    let reason = format!("--since `{rev}` is not a commit");
    assert_eq!(read.code(), 1, "{rev}\n{}", read.show());
    assert_eq!(read.text.stdout, "", "{rev}: stdout empty");
    assert_eq!(
        read.text.stderr.lines().last(),
        Some(format!("spec: {reason}").as_str()),
        "{rev}: the reason after the notes\n{}",
        read.show()
    );
    let document = read.json();
    assert_eq!(raw_order(&read.json_run.stdout, 0), TOP_KEYS, "{rev}");
    assert_eq!(document["reason"], json!(reason), "{rev}");
    assert_eq!(
        document["input"],
        json!({"ref": null, "symbol": null, "since": rev}),
        "{rev}"
    );
    assert!(document["notes"].is_array(), "{rev}: notes");
    assert_eq!(
        document["left_out"],
        json!({"generated": 0, "tier3": 0}),
        "{rev}"
    );
    for key in ["nodes", "tasks"] {
        assert_eq!(document[key], json!([]), "{rev}: {key}");
    }
    assert_eq!(document["seeds"]["nodes"], json!([]), "{rev}");
    assert_eq!(document["seeds"]["symbols"], Value::Null, "{rev}");
    for key in ["revision", "delta", "symbols", "tests", "truncated", "code"] {
        assert_eq!(document[key], Value::Null, "{rev}: {key} null");
    }
}

/// RD-2 (docs/features/impact-since.md "Command and guard"): a REV naming
/// no commit (no such name, an unknown OID, a blob by path and by OID, a
/// tree, past the root commit, a range whose end is past it, a tag of a
/// blob) exits 1 ([`assert_no_commit`]); `HEAD` before the first commit
/// too. A tag of a commit (annotated: peeled, `revision` the commit's OID),
/// a 7-hex prefix and a branch answer. Ranges and negations whose ends
/// exist: [`since_names_no_commit_for_a_range_or_a_negation`]. M2: exit 2.
#[test]
fn since_names_no_commit() {
    let repo = LockRepo::of("is-no-commit", FIXTURE, "repo", &[]);
    let blob = repo.blob_at("HEAD", LIB);
    repo.git(&["tag", "blob-tag", &blob]);
    repo.git(&["tag", "light"]);
    repo.git(&["tag", "-a", "-m", "an annotated tag", "annotated"]);
    let head = repo.head();
    let tag_object = repo.git_text(&["rev-parse", "annotated"]);
    assert_ne!(tag_object, head, "premise: an annotated tag is an object");
    for rev in [
        "nope".to_owned(),
        "0123456789abcdef0123456789abcdef01234567".to_owned(),
        format!("HEAD:{LIB}"),
        blob.clone(),
        "HEAD^{tree}".to_owned(),
        "HEAD~3".to_owned(),
        "HEAD~1..HEAD".to_owned(),
        "blob-tag".to_owned(),
    ] {
        assert_no_commit(&repo, &rev);
    }
    for rev in ["light", "annotated", &head[..7], "main", "HEAD"] {
        let read = since(&repo, rev);
        assert_since(&read, rev, &head, &nothing(0), rev);
    }

    let scratch = common::Scratch::new("is-unborn");
    let git = common::git::Sandbox::new(scratch.path());
    let root = scratch.copy(FIXTURE, "repo");
    fs::remove_file(root.join("expected.json")).expect("expected.json");
    git.init(&root);
    let home = scratch.home("h");
    let unborn = LockRepo {
        scratch,
        git,
        root,
        home,
    };
    assert_no_commit(&unborn, "HEAD");
}

/// docs/features/impact-since.md "Command and guard" (iteration 2, the
/// store's `verify_rev`): in a repository of two commits, so both ends of
/// each range exist, `--since HEAD~1..HEAD`, `--since HEAD...HEAD~1` (git
/// prints the ends and exits 1), `--since ^HEAD` and `--since ^HEAD~1`
/// (git prints `^<oid>` and exits 0) each exit 1 with ``spec: --since
/// `<REV>` is not a commit`` and the exit-1 document; `--since HEAD~1`
/// answers, exit 0, the second commit's edit its delta. M8: a range's exit
/// 1 with output read as unreadable again (exit 2).
#[test]
fn since_names_no_commit_for_a_range_or_a_negation() {
    let repo = LockRepo::of("is-ranges", FIXTURE, "repo", &[]);
    repo.replace(FARES, "    crossings * 2\n", "    crossings * 7\n");
    repo.commit_all("the second commit");
    let parent = oid(&repo, "HEAD~1");
    for (spec, code) in [
        ("HEAD~1..HEAD^{commit}", 1),
        ("HEAD...HEAD~1^{commit}", 1),
        ("^HEAD^{commit}", 0),
    ] {
        let output = repo
            .git
            .git_output(&repo.root, &["rev-parse", "--verify", "-q", spec], &[]);
        assert_eq!(output.status.code(), Some(code), "premise: {spec}'s exit");
        assert!(!output.stdout.is_empty(), "premise: git prints for {spec}");
    }
    for rev in ["HEAD~1..HEAD", "HEAD...HEAD~1", "^HEAD", "^HEAD~1"] {
        assert_no_commit(&repo, rev);
    }
    let want = Expected {
        added: vec![],
        removed: vec![],
        changed: vec![Want::Item(
            FARES,
            "crates/ferry::fares::late_fare",
            "function_item",
            9,
        )],
        files: 1,
        seeds: vec!["FARE-02"],
    };
    let read = since(&repo, "HEAD~1");
    assert_since(&read, "HEAD~1", &parent, &want, "HEAD~1");
}

/// docs/features/impact-since.md "Output": in a SHA-256 repository
/// `revision` is the commit's 64-hex OID, the header its first seven.
#[test]
fn since_names_a_sha256_commit_in_full() {
    let repo = LockRepo::of("is-sha256", FIXTURE, "repo", &["--object-format=sha256"]);
    let head = repo.head();
    assert_eq!(head.len(), 64, "premise: a SHA-256 repository");
    repo.replace(FARES, "    crossings * 2\n", "    crossings * 5\n");
    let want = Expected {
        added: vec![],
        removed: vec![],
        changed: vec![Want::Item(
            FARES,
            "crates/ferry::fares::late_fare",
            "function_item",
            9,
        )],
        files: 1,
        seeds: vec!["FARE-02"],
    };
    let read = since(&repo, "HEAD");
    assert_since(&read, "HEAD", &head, &want, "SHA-256");
}

// ------------------------------------------------------------------ RD-3

/// RD-3, TE-1 (docs/features/impact-since.md "Compared"): over the
/// scripted history, `--since` `older`, `HEAD` and `side` each give their
/// own delta, file count and seeds ([`at_older`], [`at_head`],
/// [`at_side`]), `revision` the full OID REV names and the header its
/// first seven; the annotated tag `cut`, a branch name, `HEAD~1`, `side~1`
/// and a 7-hex prefix give their commit's answer byte for byte (but
/// `input.since`). `--depth 0` holds the seeds alone. M3: `HEAD` read for
/// REV.
#[test]
fn since_compares_the_given_revision() {
    let history = history("is-given");
    let repo = &history.repo;
    let cases: [(&str, &str, Expected); 3] = [
        (&history.older, "older", at_older()),
        (&history.head, "HEAD", at_head()),
        (&history.side, "side", at_side()),
    ];
    let mut answers = Vec::new();
    for (full, name, want) in &cases {
        let rev = if *name == "older" { "cut" } else { name };
        let by_name = since(repo, rev);
        assert_since(&by_name, rev, full, want, name);
        let by_oid = since(repo, full);
        assert_since(&by_oid, full, full, want, &format!("{name} by OID"));
        answers.push(by_name.text.stdout.clone());
    }
    assert_ne!(answers[0], answers[1], "older and HEAD differ");
    assert_ne!(answers[2], answers[0], "side and older differ");
    for (alias, of) in [
        ("main", &history.head),
        ("HEAD~1", &history.older),
        (&history.older[..7], &history.older),
        ("side~1", &history.older),
    ] {
        let read = since(repo, alias);
        let named = since(repo, of);
        assert_eq!(
            read.text.stdout, named.text.stdout,
            "{alias}: the commit's text"
        );
        let (mut one, mut two) = (read.json(), named.json());
        assert_eq!(one["input"]["since"], json!(alias));
        one["input"] = Value::Null;
        two["input"] = Value::Null;
        assert_eq!(one, two, "{alias}: the commit's document");
    }
    let shallow = impact_in(
        &repo.root,
        &["--since", &history.older, "--depth", "0"],
        &vars_of(repo),
    );
    shallow.answered();
    let document = shallow.json();
    assert_eq!(document["depth"], json!(0));
    let names: BTreeSet<String> = list(&document["nodes"]).iter().map(name_of).collect();
    assert_eq!(names, seed_names(&document), "--depth 0: the seeds alone");
    assert_eq!(names, owned(&at_older().seeds), "--depth 0: older's seeds");
}

/// RD-3 (docs/features/impact-since.md "Compared", A1): over the committed
/// fixture, untracked files under the roots are `added`: a `.rs` file (its
/// marker seeds `NOTE-002`), one `.gitignore` names (never read), a RON
/// file (a whole-file record); an untracked file outside the roots and a
/// `.`-named one are not compared. `verify --changed --json`'s
/// `delta.symbols` is the same. M4: the disk side from git's index.
#[test]
fn since_counts_untracked_code() {
    let repo = LockRepo::of("is-untracked", FIXTURE, "repo", &[]);
    let head = repo.head();
    repo.write(PIER, PIER_RS);
    repo.write(".gitignore", "crates/ferry/src/ignored.rs\n");
    repo.write(
        "crates/ferry/src/ignored.rs",
        "pub fn hidden() -> u32 {\n    1\n}\n",
    );
    repo.write("data/extra.ron", "Extra(\n    spare: 1,\n)\n");
    repo.write("tools/outside.rs", "pub fn outside() -> u32 {\n    1\n}\n");
    repo.write(
        "crates/ferry/src/.dotted.rs",
        "pub fn dotted() -> u32 {\n    1\n}\n",
    );
    let ignored = repo.git_text(&["status", "--porcelain", "--ignored", "--", "crates"]);
    assert!(
        ignored.contains("!! crates/ferry/src/ignored.rs"),
        "premise: git ignores it: {ignored}"
    );
    let want = Expected {
        added: vec![
            Want::Item(
                "crates/ferry/src/ignored.rs",
                "crates/ferry::ignored::hidden",
                "function_item",
                1,
            ),
            Want::Item(PIER, "crates/ferry::pier::berth", "function_item", 4),
            Want::Whole("data/extra.ron"),
        ],
        removed: vec![],
        changed: vec![],
        files: 3,
        seeds: vec!["NOTE-002"],
    };
    let read = since(&repo, "HEAD");
    let document = assert_since(&read, "HEAD", &head, &want, "untracked");
    let verify = verify_changed(&repo, &vars_of(&repo));
    assert_eq!(
        verify["delta"]["symbols"], document["delta"],
        "verify --changed's delta.symbols"
    );
}

/// RD-3, A3 (docs/features/impact-since.md "Rules and edge cases"): over
/// the committed fixture, edits whose items' `sig` and `body` stay equal
/// are no change and seed nothing, though each file's disk OID differs
/// and each counts in `<f>`: a comment inside a body and one between
/// items (fares.rs), a reformat (board.rs), a marker retargeted and one
/// added (crossing.rs), a CRLF copy of the test file; a file rewritten
/// with its own bytes is no compared file. Zero `changed`, `added`,
/// `removed`; no seed, no node, empty rows; `verify --changed`'s
/// `delta.symbols` the same. Uncounted M: an edit with equal digests
/// counted.
#[test]
fn since_ignores_unchanged_hashes() {
    let repo = LockRepo::of("is-unchanged", FIXTURE, "repo", &[]);
    let head = repo.head();
    repo.replace(
        FARES,
        "    crossings\n}",
        "    // one per crossing\n    crossings\n}",
    );
    repo.replace(
        FARES,
        "\n// @implements FARE-02\n",
        "\n// The late fare follows.\n// @implements FARE-02\n",
    );
    repo.replace(
        BOARD,
        "pub fn rows() -> u32 {\n    3\n}",
        "pub fn rows()->u32{3}",
    );
    repo.replace(
        BOARD,
        "pub fn reach() -> u32 {\n    4\n}",
        "pub fn reach() -> u32 {\n\n        4\n\n}",
    );
    repo.replace(
        CROSSING,
        "// @implements TIDE-NOPE\n",
        "// @implements TIDE-WINDOW\n",
    );
    repo.replace(
        CROSSING,
        "pub fn dock_time() -> u32 {",
        "// @implements ROUTE-DOCKS\npub fn dock_time() -> u32 {",
    );
    code_index_common::to_crlf(&repo.root, TESTS);
    let lib = fs::read(repo.root.join(LIB)).expect("lib.rs");
    fs::write(repo.root.join(LIB), lib).expect("lib.rs rewritten");
    for file in [FARES, BOARD, CROSSING, TESTS] {
        assert_ne!(
            disk_oid(&repo, file),
            repo.blob_at("HEAD", file),
            "premise: {file}'s bytes differ from its blob"
        );
    }
    let read = since(&repo, "HEAD");
    let document = assert_since(&read, "HEAD", &head, &nothing(4), "equal digests");
    assert_eq!(document["nodes"], json!([]), "nothing reached");
    for key in ["symbols", "tests"] {
        assert_eq!(document[key], json!([]), "{key}: no row");
    }
    let verify = verify_changed(&repo, &vars_of(&repo));
    assert_eq!(
        verify["delta"]["symbols"], document["delta"],
        "verify --changed's delta.symbols"
    );
}

/// RD-3, A1 (docs/features/impact-since.md "Compared"): today's `[code]`
/// decides what is compared, never REV's config: with `tools` added to
/// the roots on disk and `crates/ferry/tests/**` excluded, a body edit in
/// a `tools` package (in REV's tree, outside REV's roots) is `changed`; a
/// body edit in the now excluded test file is nothing, and REV's test
/// items are not `removed`.
#[test]
fn since_reads_todays_roots_and_excludes() {
    let repo = LockRepo::of("is-roots", FIXTURE, "repo", &[]);
    repo.write("tools/helper/Cargo.toml", manifest("helper"));
    repo.write(
        "tools/helper/src/lib.rs",
        "pub fn assist() -> u32 {\n    1\n}\n",
    );
    repo.commit_all("a tools package outside the roots");
    let head = repo.head();
    repo.replace(
        "specengine.toml",
        "roots = [\"crates\", \"data\"]\n",
        "roots = [\"crates\", \"data\", \"tools\"]\nexclude = [\"crates/ferry/tests/**\"]\n",
    );
    repo.replace("tools/helper/src/lib.rs", "    1\n", "    2\n");
    repo.replace(
        TESTS,
        "assert_eq!(crossing.closes(), 9);",
        "assert_eq!(crossing.closes(), 8);",
    );
    let want = Expected {
        added: vec![],
        removed: vec![],
        changed: vec![Want::Item(
            "tools/helper/src/lib.rs",
            "tools/helper::assist",
            "function_item",
            1,
        )],
        files: 1,
        seeds: vec![],
    };
    let read = since(&repo, "HEAD");
    assert_since(&read, "HEAD", &head, &want, "today's roots");
}

/// docs/features/impact-since.md "Rules and edge cases": a committed `.rs`
/// file turned non-UTF-8 on disk is skipped by the walk: its items are
/// `removed` (the file still compared), no panic; the walk's note names
/// it.
#[test]
fn since_reads_a_non_utf8_file_as_removed() {
    let repo = LockRepo::of("is-latin1", FIXTURE, "repo", &[]);
    let head = repo.head();
    let mut bytes = fs::read(repo.root.join(BOARD)).expect("board.rs");
    bytes.extend_from_slice(b"// caf\xe9\n");
    fs::write(repo.root.join(BOARD), bytes).expect("board.rs, Latin-1");
    let want = Expected {
        added: vec![],
        removed: vec![
            Want::Item(BOARD, "crates/ferry::board::rows", "function_item", 4),
            Want::Item(BOARD, "crates/ferry::board::reach", "function_item", 8),
        ],
        changed: vec![],
        files: 1,
        // board.rs's `//!` marker at REV encloses both.
        seeds: vec!["TIDE-BOARD"],
    };
    let read = since(&repo, "HEAD");
    assert_since(&read, "HEAD", &head, &want, "a non-UTF-8 file");
    assert!(
        read.text.stderr.contains(BOARD),
        "the walk's note names the skipped file\n{}",
        read.show()
    );
}

// ------------------------------------------------------------------ RD-4

/// RD-4, A2 (docs/features/impact-since.md "Seeds"): over the committed
/// fixture plus a commit where `NOTE-003` exists and binds `board::rows`
/// and `fares::rows` carries a marker naming `NOTE-004` (no node then), the
/// disk:
/// - deletes `late_fare` with its marker: removed, reaches `FARE-02`
///   through REV's marker alone;
/// - edits `fare`'s body and drops its marker: changed, `FARE-01` through
///   REV's marker;
/// - adds `night_fare` under a `NOTE-001` marker: the disk's marker;
/// - edits `board::reach` under a new `NOTE-002` marker: the disk's marker,
///   and `TIDE-BOARD` (board.rs's file module, both sides);
/// - deletes `board::rows` and the `NOTE-003` record: `NOTE-003` names
///   nothing today and seeds nothing;
/// - edits `fares::rows` and drops its marker, the `NOTE-004` record
///   written: REV's marker resolved against today's corpus seeds it;
/// - edits the method `Crossing::closes`: it and its `impl`, whose marker
///   seeds `TIDE-WINDOW`;
/// - deletes `dock_time` with its comment that only mentions
///   `ROUTE-DOCKS`: a mention never seeds.
///
/// The radius is 7b-1's from those seeds ([`assert_radius_of_seeds`]).
/// M5: REV's markers ignored.
#[test]
fn removed_symbol_reaches_its_old_binding() {
    let repo = LockRepo::of("is-reach", FIXTURE, "repo", &[]);
    let record = |id: &str, title: &str| {
        format!(
            "---\nclass: canon\nid: {id}\nowner: owner\nreviewed: 2026-10-21\n---\n\n# {title}\n\nA record of the reach test.\n"
        )
    };
    repo.write(
        "docs/records/NOTE-003.md",
        record("NOTE-003", "Rows on the board"),
    );
    repo.replace(
        BOARD,
        "pub fn rows() -> u32 {",
        "// @implements NOTE-003\npub fn rows() -> u32 {",
    );
    repo.replace(
        FARES,
        "pub fn rows() -> u32 {",
        "// @implements NOTE-004\npub fn rows() -> u32 {",
    );
    repo.commit_all("markers of then");
    let head = repo.head();

    repo.replace(
        FARES,
        "// @implements FARE-02\npub fn late_fare(crossings: u32) -> u32 {\n    crossings * 2\n}\n\n",
        "",
    );
    repo.replace(
        FARES,
        "// @implements FARE-01\npub fn fare(crossings: u32) -> u32 {\n    crossings\n}",
        "pub fn fare(crossings: u32) -> u32 {\n    crossings + 1\n}",
    );
    repo.append(
        FARES,
        "\n// @implements NOTE-001\npub fn night_fare() -> u32 {\n    4\n}\n",
    );
    repo.replace(
        BOARD,
        "pub fn reach() -> u32 {\n    4\n}",
        "// @implements NOTE-002\npub fn reach() -> u32 {\n    5\n}",
    );
    repo.replace(
        BOARD,
        "// @implements NOTE-003\npub fn rows() -> u32 {\n    3\n}\n\n",
        "",
    );
    fs::remove_file(repo.root.join("docs/records/NOTE-003.md")).expect("NOTE-003 gone");
    repo.replace(
        FARES,
        "// @implements NOTE-004\npub fn rows() -> u32 {\n    2\n}",
        "pub fn rows() -> u32 {\n    3\n}",
    );
    repo.write(
        "docs/records/NOTE-004.md",
        record("NOTE-004", "Rows of fares"),
    );
    repo.replace(CROSSING, "        self.close\n", "        self.close + 1\n");
    repo.replace(
        CROSSING,
        "// Waits at ROUTE-DOCKS between crossings.\npub fn dock_time() -> u32 {\n    3\n}\n\n",
        "",
    );

    let read = since(&repo, "HEAD");
    read.answered();
    let document = read.json();
    assert_eq!(document["revision"], json!(head));
    let delta = &document["delta"];
    let symbols = |key: &str| -> BTreeSet<String> {
        list(&delta[key])
            .iter()
            .map(|item| text(&item["symbol"]).to_owned())
            .collect()
    };
    assert_eq!(
        symbols("removed"),
        owned(&[
            "crates/ferry::board::rows",
            "crates/ferry::crossing::dock_time",
            "crates/ferry::fares::late_fare",
        ]),
        "premise: removed"
    );
    assert_eq!(
        symbols("changed"),
        owned(&[
            "crates/ferry::board::reach",
            "crates/ferry::crossing::Crossing::closes",
            "crates/ferry::crossing::impl Crossing",
            "crates/ferry::fares::fare",
            "crates/ferry::fares::rows",
        ]),
        "premise: changed"
    );
    assert_eq!(
        symbols("added"),
        owned(&["crates/ferry::fares::night_fare"]),
        "premise: added"
    );
    assert_eq!(
        seed_names(&document),
        owned(&[
            "FARE-01",
            "FARE-02",
            "NOTE-001",
            "NOTE-002",
            "NOTE-004",
            "TIDE-BOARD",
            "TIDE-WINDOW",
        ]),
        "the seeds of REV's markers (resolved now) and the disk's\n{}",
        read.show()
    );
    assert_radius_of_seeds(&repo, &document, "reach");
}

/// The radius of a `--since` answer equals its seeds' 7b-1 radii merged:
/// each node at its least distance over the seeds' `spec impact <seed>`
/// walks, the row keys of `symbols` and `tests` their union.
#[track_caller]
fn assert_radius_of_seeds(repo: &LockRepo, document: &Value, what: &str) {
    let mut least: BTreeMap<String, u64> = BTreeMap::new();
    let mut symbols = BTreeSet::new();
    let mut tests = BTreeSet::new();
    for seed in seed_names(document) {
        let one = impact_in(&repo.root, &[&seed], &vars_of(repo));
        one.answered();
        let walk = one.json();
        for node in list(&walk["nodes"]) {
            let distance = node["distance"].as_u64().expect("a distance");
            least
                .entry(name_of(&node))
                .and_modify(|known| *known = (*known).min(distance))
                .or_insert(distance);
        }
        symbols.extend(list(&walk["symbols"]).iter().map(key_of));
        tests.extend(list(&walk["tests"]).iter().map(key_of));
    }
    let walked: BTreeMap<String, u64> = list(&document["nodes"])
        .iter()
        .map(|node| {
            (
                name_of(node),
                node["distance"].as_u64().expect("a distance"),
            )
        })
        .collect();
    assert_eq!(walked, least, "{what}: the seeds' walks merged");
    let have =
        |key: &str| -> BTreeSet<String> { list(&document[key]).iter().map(key_of).collect() };
    assert_eq!(have("symbols"), symbols, "{what}: the seeds' symbol rows");
    assert_eq!(have("tests"), tests, "{what}: the seeds' test rows");
}

/// docs/features/impact-since.md "Seeds" over the history: each REV's
/// radius is its seeds' radii merged ([`assert_radius_of_seeds`]).
#[test]
fn since_radius_is_its_seeds_radius() {
    let history = history("is-radius");
    for rev in [&history.older, &history.head, &history.side] {
        let read = since(&history.repo, rev);
        read.answered();
        assert_radius_of_seeds(&history.repo, &read.json(), rev);
    }
}

/// docs/features/impact-since.md "Seeds": a RON value edit (`data/fares.ron`,
/// `@configures FARE-01`) is its whole-file record `changed`, which reaches
/// every marker of the file, RON ones included: `FARE-01`. A RON file with
/// no marker seeds nothing.
#[test]
fn ron_whole_file_record_reaches_its_markers() {
    let repo = LockRepo::of("is-ron", FIXTURE, "repo", &[]);
    repo.write(TIDES_RON, TIDES);
    repo.commit_all("tides");
    let head = repo.head();
    repo.replace(FARES_RON, "    base: 1,", "    base: 2,");
    repo.replace(TIDES_RON, "    spring: 4,", "    spring: 5,");
    let want = Expected {
        added: vec![],
        removed: vec![],
        changed: vec![Want::Whole(FARES_RON), Want::Whole(TIDES_RON)],
        files: 2,
        seeds: vec!["FARE-01"],
    };
    let read = since(&repo, "HEAD");
    assert_since(&read, "HEAD", &head, &want, "RON values");
}

// ------------------------------------------------------------------ TE-1

/// TE-1 (docs/features/impact-since.md "Acceptance criteria"): the
/// scripted history's delta, header counts and seeds per REV (`older` by
/// its tag, `HEAD`, the branch `side`), as [`at_older`], [`at_head`] and
/// [`at_side`] write them by hand: an item changed, added, removed; a
/// comment-only and a formatting-only edit (no item); an untracked and a
/// deleted file; a RON value and a RON comment edit (each `changed |
/// <file>:1 | <file> file`, its OID differing); a module move and a
/// package moved with its manifest renamed (removed plus added); a method
/// edit (it and its `impl`).
#[test]
fn since_history_gives_each_revisions_delta() {
    let history = history("is-history");
    let repo = &history.repo;
    for (rev, full, want) in [
        ("cut", &history.older, at_older()),
        ("HEAD", &history.head, at_head()),
        ("side", &history.side, at_side()),
    ] {
        let read = since(repo, rev);
        assert_since(&read, rev, full, &want, rev);
        for file in [FARES_RON, TIDES_RON] {
            assert_ne!(
                disk_oid(repo, file),
                repo.blob_at(full, file),
                "premise: {file} differs by its OID"
            );
        }
        for line in [
            format!("changed | {FARES_RON}:1 | {FARES_RON} file"),
            format!("changed | {TIDES_RON}:1 | {TIDES_RON} file"),
            format!("changed | {CROSSING}:10 | crates/ferry::crossing::impl Crossing impl_item"),
            format!("added | {WHARF}:3 | crates/wharf::moor function_item"),
            format!("removed | {QUAY}:3 | crates/quay::moor function_item"),
        ] {
            assert!(
                read.text.stdout.lines().any(|printed| printed == line),
                "{rev}: the line {line:?}\n{}",
                read.show()
            );
        }
    }
    assert_eq!(repo.parents(&history.head), vec![history.older.clone()]);
    assert_eq!(repo.parents(&history.side), vec![history.older.clone()]);
    assert_eq!(oid(repo, "cut"), history.older, "the tag peels to older");
}

// ------------------------------------------------------------------ RD-6

/// RD-6 (docs/features/impact-since.md "Rules and edge cases"): uncut
/// `impact --since HEAD --json`'s `delta` equals `verify --changed
/// --json`'s `delta.symbols`, value and order: over the history, over it
/// with `--task` (all code, bound or not), and over a `code-a` copy with a
/// method and a parse-error file edited and an untracked file. M7: a
/// second order (impact by symbol first).
#[test]
fn since_head_delta_equals_verify_changed() {
    let history = history("is-one");
    let repo = &history.repo;
    let vars = vars_of(repo);
    let read = since(repo, "HEAD");
    read.answered();
    let delta = read.json()["delta"].clone();
    let raw = run_with(&repo.root, &["verify", "--changed", "--json"], &vars).stdout;
    let verify = common::one_json_document(&raw).expect("verify's JSON");
    assert_eq!(verify["delta"]["symbols"], delta, "one comparer");
    assert_eq!(
        keys_in_order(&raw, &["delta", "symbols"]),
        DELTA_KEYS,
        "delta.symbols' keys in order"
    );
    let task = verify_common::new_task(repo, &["FARE-02"]);
    let with_task = run_with(
        &repo.root,
        &["verify", "--changed", "--task", &task, "--json"],
        &vars,
    );
    assert!([0, 1].contains(&with_task.code), "{}", with_task.show());
    let with_task = common::one_json_document(&with_task.stdout).expect("JSON");
    assert_eq!(
        with_task["delta"]["symbols"], delta,
        "--task: all code, bound or not"
    );

    let code_a = LockRepo::code_a("is-one-code-a");
    code_a.replace(
        "crates/tally/src/counter.rs",
        "    pub fn value(&self) -> u32 {\n        self.count\n",
        "    pub fn value(&self) -> u32 {\n        self.count + 0\n",
    );
    code_a.replace(
        "crates/tally/src/broken.rs",
        "// GOAL-77 names no node.\n",
        "// GOAL-77 names no node, still.\n",
    );
    code_a.write(
        "crates/tally/src/later.rs",
        "pub fn later() -> u32 {\n    9\n}\n",
    );
    let read = since(&code_a, "HEAD");
    read.answered();
    let delta = read.json()["delta"].clone();
    for key in ["added", "changed"] {
        assert!(
            !list(&delta[key]).is_empty(),
            "premise: code-a's {key}: {delta}"
        );
    }
    let verify = verify_changed(&code_a, &vars_of(&code_a));
    assert_eq!(verify["delta"]["symbols"], delta, "code-a: one comparer");
}

// ------------------------------------------------------------------ RD-7

/// RD-7, TE-6 (docs/features/impact-since.md "Rules and edge cases"): with
/// a `git` shim on `PATH` logging argv, every `--since` read of the
/// history (each REV, text and JSON; `--depth 1` from a subdirectory) and
/// `verify --changed --json` call git only as `rev-parse`, `ls-tree -r -z
/// <oid>`, `cat-file --batch` and `hash-object --no-filters --stdin-paths`
/// (never `-w`, `status`, `diff`, `ls-files`), 7b-1's tasks probe aside
/// (`rev-parse` too); `--since` runs `rev-parse --verify -q <REV>^{commit}`
/// with the REV given, `ls-tree` and `cat-file`; every byte under the root, `.git` included (index,
/// objects, refs), the index's mtime, the object count and `git status`
/// stay as they were. Uncounted M: a `git status` call.
#[test]
fn reads_leave_git_index_and_objects_untouched() {
    let history = history("is-door");
    let repo = &history.repo;
    let shim = Shim::new(repo, "shim");
    let vars = shim.vars(repo);
    verify_common::new_task(repo, &["FARE-02"]);
    let before = Untouched::take(repo);
    for rev in [history.older.as_str(), "HEAD", "side", &history.side[..7]] {
        shim.clear();
        let read = since_in(&repo.root, rev, &vars);
        read.answered();
        before.still(repo, rev);
        let calls = shim.calls();
        let strays: Vec<&Vec<String>> = calls
            .iter()
            .filter(|call| !through_the_door(call))
            .collect();
        assert!(strays.is_empty(), "{rev}: outside the door: {strays:?}");
        let named = format!("{rev}^{{commit}}");
        let verify_rev: Vec<String> = ["rev-parse", "--verify", "-q", named.as_str()]
            .map(str::to_owned)
            .to_vec();
        assert!(
            calls.contains(&verify_rev),
            "{rev}: rev-parse --verify -q <REV>^{{commit}}: {calls:?}"
        );
        for first in ["ls-tree", "cat-file"] {
            assert!(
                calls
                    .iter()
                    .any(|call| call.first().is_some_and(|word| word == first)),
                "{rev}: {first} ran: {calls:?}"
            );
        }
        let deeper = impact_in(
            &repo.root.join("crates"),
            &["--since", rev, "--depth", "1"],
            &vars,
        );
        deeper.answered();
        before.still(repo, &format!("{rev} --depth 1 from a subdirectory"));
    }
    shim.clear();
    let verify = run_with(&repo.root, &["verify", "--changed", "--json"], &vars);
    assert!([0, 1].contains(&verify.code), "{}", verify.show());
    before.still(repo, "verify --changed --json");
    let strays: Vec<Vec<String>> = shim
        .calls()
        .into_iter()
        .filter(|call| !through_the_door(call))
        .collect();
    assert!(
        strays.is_empty(),
        "verify --changed: outside the door: {strays:?}"
    );
}

// ------------------------------------------------------------------ RD-8

/// RD-8, Q-G (docs/features/impact-since.md "Compared"): with the code
/// index built (`spec index`) and every stored level digest tampered
/// (`b3:` and 64 `e`, as `show_bindings.rs`' `fake_digest`), `--since` each
/// REV of the history and `verify --changed --json` give the delta they
/// gave before: both sides are hashed afresh. The history's comment-only
/// edit keeps a file whose OID differs and whose items are equal
/// (`closes`, `Crossing`, `slack`), which a stored digest would turn
/// `changed`. The tampered columns survive the reads. M8: digests read
/// from the index.
#[test]
fn since_hashes_both_sides_afresh() {
    let history = history("is-afresh");
    let repo = &history.repo;
    let vars = vars_of(repo);
    let revs = [history.older.as_str(), "HEAD", "side"];
    let before: Vec<Value> = revs
        .iter()
        .map(|rev| {
            let read = since(repo, rev);
            read.answered();
            read.json()["delta"].clone()
        })
        .collect();
    let verify_before = verify_changed(repo, &vars)["delta"]["symbols"].clone();

    repo.spec_piped(&repo.root, &["index"]).code(0);
    let db = code_index_common::db(&repo.home, SLUG);
    let fake = format!("b3:{}", "e".repeat(64));
    let sets: Vec<String> = ["sig", "body", "deps", "skeleton", "quote"]
        .iter()
        .map(|level| {
            format!("{level}_hash = CASE WHEN {level}_hash IS NULL THEN NULL ELSE '{fake}' END")
        })
        .collect();
    code_index_common::execute(&db, &format!("UPDATE symbols SET {}", sets.join(", ")));
    let tampered = || -> u64 {
        code_index_common::query(
            &db,
            &format!("SELECT count(*) AS n FROM symbols WHERE body_hash = '{fake}'"),
        )[0]["n"]
            .as_u64()
            .expect("a count")
    };
    let count = tampered();
    assert!(count > 0, "premise: digests to tamper");
    for (rev, delta) in revs.iter().zip(&before) {
        let read = since(repo, rev);
        read.answered();
        assert_eq!(&read.json()["delta"], delta, "{rev}: hashed afresh");
    }
    assert_eq!(
        verify_changed(repo, &vars)["delta"]["symbols"],
        verify_before,
        "verify --changed: hashed afresh"
    );
    assert_eq!(tampered(), count, "premise: the tampered columns stayed");
}

// ------------------------------------------------------------------ TE-2

/// The reversed-order copy of a worktree (`.git` included).
fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("type");
        if kind.is_dir() {
            copy_reversed(&entry.path(), &target);
        } else if kind.is_file() {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            let mode = fs::metadata(entry.path()).expect("meta").permissions();
            fs::set_permissions(&target, mode).expect("mode");
        }
    }
}

/// The `.rs` files of a worktree, `.git` aside.
fn rust_files(root: &Path) -> Vec<String> {
    snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some() && !path.starts_with(".git") && path.ends_with(".rs")
        })
        .map(|(path, _)| path)
        .collect()
}

/// The header's file count of a `--since` text.
fn files_of(stdout: &str) -> u64 {
    let first = stdout.lines().next().unwrap_or_default();
    let after = first.split_once(": ").map_or("", |(_, rest)| rest);
    after
        .split_once(" files, ")
        .and_then(|(count, _)| count.parse().ok())
        .unwrap_or_else(|| panic!("a file count in {first:?}"))
}

/// TE-2 (docs/features/impact-since.md "Rules and edge cases"): the
/// history's `--since` answers (REVs by name and OID, `--depth 1`) are
/// byte-identical (exit, text, JSON, stderr) repeated, from another cwd,
/// from a second history built at another root, from a copy of the first
/// in reverse creation order, each its own `HOME`; from a copy whose every
/// `.rs` file on disk is CRLF over the LF blobs the JSON and stderr are
/// identical and the text too but the header's `<f>`, which counts the
/// CRLF files as compared. No answer prints a root. M: items in creation
/// order; a CR read as a change.
#[test]
fn since_is_byte_identical_across_clones_order_and_crlf() {
    let first = history_at("is-det-a", "repo");
    let second = history_at("is-det-b", "elsewhere/deeper");
    assert_eq!(
        (&first.older, &first.head, &first.side),
        (&second.older, &second.head, &second.side),
        "premise: one history, one OID per commit"
    );
    let reversed = first.repo.scratch.join("reversed");
    copy_reversed(&first.repo.root, &reversed);
    let reversed = fs::canonicalize(&reversed).expect("the reversed copy");
    let crlf = clone_dir(&first.repo, "crlf");
    let mut converted = 0;
    for path in rust_files(&crlf) {
        code_index_common::to_crlf(&crlf, &path);
        converted += 1;
    }
    assert!(converted > 5, "premise: .rs files made CRLF");
    let crlf_oid = first
        .repo
        .git
        .git_text(&crlf, &["hash-object", "--no-filters", "--", FARES]);
    assert_ne!(
        crlf_oid,
        first.repo.blob_at("HEAD", FARES),
        "premise: a CRLF file's OID differs from its LF blob's"
    );
    let first_vars = vars_of(&first.repo);
    let home = |name: &str| first.repo.scratch.home(name).into_os_string();
    let places: Vec<Place> = vec![
        ("second", second.repo.root.clone(), vars_of(&second.repo)),
        (
            "reversed",
            reversed,
            with_var(&first_vars, "HOME", &home("reversed")),
        ),
    ];
    let forms: Vec<Vec<String>> = vec![
        vec!["--since".into(), "HEAD".into()],
        vec!["--since".into(), "side".into()],
        vec!["--since".into(), "cut".into()],
        vec!["--since".into(), first.older.clone()],
        vec![
            "--since".into(),
            "HEAD~1".into(),
            "--depth".into(),
            "1".into(),
        ],
    ];
    let crlf_vars = with_var(&first_vars, "HOME", &home("crlf"));
    for form in &forms {
        let args: Vec<&str> = form.iter().map(String::as_str).collect();
        let base = impact_in(&first.repo.root, &args, &first_vars);
        base.answered();
        let again = impact_in(&first.repo.root, &args, &first_vars);
        assert_eq!(again.bytes(), base.bytes(), "{args:?} repeated");
        let below = impact_in(&first.repo.root.join("data"), &args, &first_vars);
        assert_eq!(below.bytes(), base.bytes(), "{args:?} from another cwd");
        for (name, root, vars) in &places {
            let other = impact_in(root, &args, vars);
            assert_eq!(
                other.bytes(),
                base.bytes(),
                "{args:?} from {name}\n{}\n{}",
                base.show(),
                other.show()
            );
        }
        let crlf_read = impact_in(&crlf, &args, &crlf_vars);
        crlf_read.answered();
        assert_eq!(
            crlf_read.json_run.stdout, base.json_run.stdout,
            "{args:?}: CRLF, the JSON"
        );
        assert_eq!(
            crlf_read.text.stderr, base.text.stderr,
            "{args:?}: CRLF, stderr"
        );
        let tail = |stdout: &str| stdout.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert_eq!(
            tail(&crlf_read.text.stdout),
            tail(&base.text.stdout),
            "{args:?}: CRLF, the text after the header"
        );
        assert!(
            files_of(&crlf_read.text.stdout) > files_of(&base.text.stdout),
            "{args:?}: the CRLF files count as compared"
        );
        for scratch in [first.repo.scratch.path(), second.repo.scratch.path()] {
            let root = scratch.display().to_string();
            for printed in [&base.text.stdout, &base.json_run.stdout, &base.text.stderr] {
                assert!(!printed.contains(&root), "{args:?}: no root printed");
            }
        }
    }
}

/// TE-2 (docs/features/impact-since.md "Rules and edge cases"): over the
/// committed fixture, every `.rs` file on disk CRLF over its LF blob: zero
/// `changed`, `added`, `removed`, no seed, each file counted in `<f>`;
/// `verify --changed --json`'s `delta.symbols` the same. A RON file's CRLF
/// copy is its whole-file record `changed` (its OID differs).
#[test]
fn crlf_copy_over_lf_blobs_changes_no_rust_item() {
    let repo = LockRepo::of("is-crlf", FIXTURE, "repo", &[]);
    let head = repo.head();
    let files = rust_files(&repo.root);
    for path in &files {
        code_index_common::to_crlf(&repo.root, path);
        assert_ne!(
            disk_oid(&repo, path),
            repo.blob_at("HEAD", path),
            "premise: {path}'s OID differs"
        );
    }
    let read = since(&repo, "HEAD");
    let document = assert_since(
        &read,
        "HEAD",
        &head,
        &nothing(files.len() as u64),
        "CRLF Rust",
    );
    let verify = verify_changed(&repo, &vars_of(&repo));
    assert_eq!(verify["delta"]["symbols"], document["delta"]);

    code_index_common::to_crlf(&repo.root, FARES_RON);
    let want = Expected {
        added: vec![],
        removed: vec![],
        changed: vec![Want::Whole(FARES_RON)],
        files: files.len() as u64 + 1,
        seeds: vec!["FARE-01"],
    };
    let read = since(&repo, "HEAD");
    assert_since(&read, "HEAD", &head, &want, "a CRLF RON file");
}

// ------------------------------------------------------------------ TE-4

/// TE-4 (docs/features/impact-since.md "Command and guard"): `--since` on
/// `fixtures/spec-c` (no `[code]`) exits 2 naming `[code]`; outside a
/// repository, with no `git` on `PATH` and with a `git` that fails every
/// call: exit 2, stdout empty, one `spec: ` line naming neither a root nor
/// git's own text, no panic.
#[test]
fn since_needs_code_and_git() {
    let genre = LockRepo::of("is-genre", "spec-c", "repo", &[]);
    let read = since(&genre, "HEAD");
    assert_usage(&read, &format!("spec: {NO_CODE}"), "spec-c --since");

    let committed = LockRepo::of("is-nogit", FIXTURE, "repo", &[]);
    let plain = committed.scratch.copy(FIXTURE, "plain");
    let _ = fs::remove_file(plain.join("expected.json"));
    let vars = vars_of(&committed);
    let failing = committed.scratch.dir("failing");
    let script = failing.join("git");
    fs::write(
        &script,
        "#!/bin/sh\necho 'fatal: refused by the test' >&2\nexit 128\n",
    )
    .expect("the failing git");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");
    let cases: Vec<Place> = vec![
        ("outside a repository", plain, vars.clone()),
        (
            "no git on PATH",
            committed.root.clone(),
            with_var(
                &vars,
                "PATH",
                committed.scratch.dir("empty-path").as_os_str(),
            ),
        ),
        (
            "a failing git",
            committed.root.clone(),
            with_var(&vars, "PATH", &committed.git.path_with(&failing)),
        ),
    ];
    for (what, root, vars) in cases {
        let read = since_in(&root, "HEAD", &vars);
        assert_cannot_run(&read, what);
        let reasons: Vec<&str> = read
            .text
            .stderr
            .lines()
            .filter(|line| line.starts_with("spec: "))
            .collect();
        assert_eq!(reasons.len(), 1, "{what}: one reason\n{}", read.show());
        let scratch = committed.scratch.path().display().to_string();
        assert!(
            !read.text.stderr.contains(&scratch)
                && !read.text.stderr.contains("refused by the test"),
            "{what}: neither a root nor git's text\n{}",
            read.show()
        );
    }
}

// ----------------------------------------------------------- the form

/// TE-4, docs/features/impact-since.md "Command and guard": REF with
/// `--since`, `--symbol` with `--since`, all three, a guarded REV beside a
/// REF, and no input each exit 2 with the form's line, stdout empty, no
/// git child, outside any project too (the form comes before discovery).
#[test]
fn since_takes_exactly_one_input() {
    let repo = LockRepo::of("is-form", FIXTURE, "repo", &[]);
    let shim = Shim::new(&repo, "shim");
    let vars = shim.vars(&repo);
    let nowhere = repo.scratch.dir("no-project");
    for args in [
        &["TIDE-WINDOW", "--since", "HEAD"][..],
        &["--symbol", "opens", "--since", "HEAD"],
        &["TIDE-WINDOW", "--symbol", "opens", "--since", "HEAD"],
        &["--since=-x", "TIDE-WINDOW"],
        &[],
    ] {
        for cwd in [&repo.root, &nowhere] {
            shim.clear();
            let read = impact_in(cwd, args, &vars);
            assert_usage(&read, ONE_INPUT, &format!("{args:?}"));
            assert_eq!(shim.calls(), Vec::<Vec<String>>::new(), "{args:?}: no git");
        }
    }
}

// ------------------------------------------------------------- the cut

/// docs/features/impact-since.md "Cut" (Q-F): 900 untracked items (one
/// file) put the delta's lines past 40 000 characters: the header (the
/// totals) and the summary print; the delta lines come last and are cut
/// whole; the tail ``[truncated: <d> delta items not shown; lower --depth
/// N or name a later --since REV]`` closes the answer; `truncated.delta`
/// counts the items not shown and `truncated.seeds` is `null`; the JSON's
/// `added` holds exactly the printed items, in order. M: the JSON holding
/// an unprinted item; delta lines first.
#[test]
fn since_cut_keeps_the_header_and_cuts_the_delta() {
    let repo = LockRepo::of("is-cut", FIXTURE, "repo", &[]);
    let head = repo.head();
    let mut many = String::new();
    for n in 0..900 {
        many.push_str(&format!(
            "pub fn an_untracked_function_with_a_long_name_{n:04}() -> u32 {{\n    {n}\n}}\n"
        ));
    }
    repo.write("crates/ferry/src/many.rs", many);
    let read = since(&repo, "HEAD");
    read.answered();
    let document = read.json();
    let lines: Vec<&str> = read.text.stdout.lines().collect();
    assert_eq!(
        lines[0],
        format!(
            "since {}: 1 files, 0 changed, 900 added, 0 removed",
            &head[..7]
        ),
        "the header counts the totals"
    );
    assert!(lines[1].starts_with("impact: nodes "), "the summary second");
    let shown: usize = read
        .text
        .stdout
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("[truncated: "))
        .map(|line| line.chars().count())
        .sum();
    assert!(shown <= CAP, "{shown} characters before the tail");
    let printed: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with("added | "))
        .collect();
    assert!(
        !printed.is_empty() && printed.len() < 900,
        "some delta lines print, not all: {}",
        printed.len()
    );
    let added = list(&document["delta"]["added"]);
    let from_json: Vec<String> = added.iter().map(|item| delta_line("added", item)).collect();
    assert_eq!(
        printed, from_json,
        "the printed items are the JSON's, in order"
    );
    let hidden = 900 - printed.len();
    let tail = lines.last().copied().unwrap_or_default();
    assert_eq!(
        tail,
        format!("[truncated: {hidden} delta items{SINCE_TAIL}"),
        "the tail names the narrowing"
    );
    let truncated = &document["truncated"];
    assert_eq!(
        truncated["delta"],
        json!(hidden),
        "truncated.delta: {truncated}"
    );
    assert_eq!(
        truncated["seeds"],
        Value::Null,
        "truncated.seeds: {truncated}"
    );
    for key in ["nodes", "symbols", "tests", "tasks"] {
        assert_eq!(truncated[key], json!(0), "truncated.{key}: {truncated}");
    }
    assert_eq!(
        added.first().map(|item| text(&item["symbol"]).to_owned()),
        Some("crates/ferry::many::an_untracked_function_with_a_long_name_0000".to_owned()),
        "the first items print"
    );
}

// --------------------------------------------------------- edge data

/// docs/features/impact-since.md "Command and guard": a disk file and a
/// REV blob that tree-sitter cannot parse whole give an answer (their
/// items `changed` when compared: `Unhashed` is never equal) or a clear
/// exit 2 naming the file, never a panic; repeated, the same bytes.
#[test]
fn since_survives_unparsable_code() {
    let repo = LockRepo::of("is-edge", FIXTURE, "repo", &[]);
    repo.write(
        "crates/ferry/src/broken_then.rs",
        "pub fn then() -> u32 {\n    let x = 1 2;\n    x\n}\n\npub fn fine() -> u32 {\n    1\n}\n",
    );
    repo.commit_all("a broken file then");
    repo.replace(
        "crates/ferry/src/broken_then.rs",
        "pub fn fine() -> u32 {\n    1\n}",
        "pub fn fine() -> u32 {\n    1\n}\n// edited",
    );
    repo.replace(
        BOARD,
        "pub fn rows() -> u32 {\n    3\n}",
        "pub fn rows() -> u32 {\n    let y = 3 4;\n    y\n}",
    );
    let read = since(&repo, "HEAD");
    assert!(
        [0, 2].contains(&read.code()),
        "an answer or exit 2\n{}",
        read.show()
    );
    assert!(!read.text.stderr.contains("panicked"), "{}", read.show());
    if read.code() == 2 {
        assert_eq!(read.text.stdout, "", "stdout empty");
        assert!(
            read.text.stderr.contains(BOARD) || read.text.stderr.contains("broken_then.rs"),
            "exit 2 names the file\n{}",
            read.show()
        );
    } else {
        let changed: BTreeSet<String> = list(&read.json()["delta"]["changed"])
            .iter()
            .map(|item| text(&item["symbol"]).to_owned())
            .collect();
        assert!(
            changed.contains("crates/ferry::board::rows"),
            "a broken item is never proven equal: {changed:?}"
        );
    }
    let again = since(&repo, "HEAD");
    assert_eq!(again.bytes(), read.bytes(), "repeated");
}

// ------------------------------------------------------------- tasks

/// docs/features/impact-since.md "Seeds" (then 7b-1's tasks): over the
/// history, `--since HEAD` lists the open task naming a seed (`NOTE-001`),
/// not the one naming a node outside the radius (`ROUTE-TIMETABLE`); its
/// `nodes` the reached node; the same task row `spec impact NOTE-001`
/// lists.
#[test]
fn since_lists_the_radius_open_tasks() {
    let history = history("is-tasks");
    let repo = &history.repo;
    let inside = verify_common::new_task(repo, &["NOTE-001"]);
    let outside = verify_common::new_task(repo, &["ROUTE-TIMETABLE"]);
    let read = since(repo, "HEAD");
    read.answered();
    let document = read.json();
    let tasks = list(&document["tasks"]);
    let ids: Vec<&str> = tasks.iter().map(|task| text(&task["id"])).collect();
    assert_eq!(
        ids,
        [inside.as_str()],
        "the radius' task alone, not {outside}"
    );
    assert_eq!(tasks[0]["nodes"], json!(["NOTE-001"]));
    let one = impact_in(&repo.root, &["NOTE-001"], &vars_of(repo));
    one.answered();
    assert_eq!(one.json()["tasks"], document["tasks"], "7b-1's task rows");
    assert!(
        read.text
            .stdout
            .lines()
            .any(|line| line.starts_with(&format!("task {inside} | "))),
        "the task line\n{}",
        read.show()
    );
}

// ------------------------------------------------------------- notes

/// docs/features/impact-since.md "Output" (notes: the md refresh's, the
/// disk walk's, the queue's, `bindings: <cause>` last): with a committed
/// `.rs` file turned non-UTF-8 on disk (the walk's note naming it) and an
/// unreadable `spec.lock` (a binding cause), `--since HEAD` still answers
/// (exit 0, the delta the lock never moves), the walk's note before the
/// one `note: bindings: ` line, which is the last note.
#[test]
fn since_notes_keep_their_order() {
    let repo = LockRepo::of("is-notes", FIXTURE, "repo", &[]);
    let mut bytes = fs::read(repo.root.join(BOARD)).expect("board.rs");
    bytes.extend_from_slice(b"// caf\xe9\n");
    fs::write(repo.root.join(BOARD), bytes).expect("board.rs, Latin-1");
    let sound = since(&repo, "HEAD");
    sound.answered();
    repo.write("spec.lock", "garbage = [\n");
    let read = since(&repo, "HEAD");
    read.answered();
    assert_eq!(
        read.json()["delta"],
        sound.json()["delta"],
        "the lock never moves the delta"
    );
    let notes: Vec<&str> = read
        .text
        .stderr
        .lines()
        .filter(|line| line.starts_with("note: "))
        .collect();
    let walk = notes
        .iter()
        .position(|line| line.contains(BOARD))
        .unwrap_or_else(|| panic!("the walk's note names {BOARD}\n{}", read.show()));
    let bindings: Vec<usize> = notes
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("note: bindings: "))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(bindings.len(), 1, "one bindings note\n{}", read.show());
    assert!(walk < bindings[0], "the walk's note first\n{}", read.show());
    assert_eq!(
        bindings[0],
        notes.len() - 1,
        "bindings last\n{}",
        read.show()
    );
}
