//! Helpers of the docs/features/spec-verify.md tests ("Acceptance
//! criteria", TE-1..TE-9, TE-11 and the named RD tests): scratch git
//! repositories made at test time through slice 3's `lock_common`
//! (`LockRepo`: every git process sandboxed, fixed dates, a scratch `HOME`)
//! from two fixtures of different nature:
//!
//! - `fixtures/code-a` with the golden lock
//!   (`fixtures/spec-lock/code-a.lock`, format 2 since
//!   docs/features/lock-levels.md) committed on top: a library whose
//!   markers carry every binding problem the canon names (TE-1);
//! - `fixtures/verify-ecs`: an ECS-style game crate (systems, a plugin, RON
//!   tuning data) whose every binding is sound, its four nodes accepted
//!   through the library, so it verifies `clean` (TE-4..TE-7).
//!
//! `spec verify` runs as the binary (stdin null), text and `--json`, from
//! the root unless a test says otherwise; the expected rows are built here
//! from the spec's "States" and "Output", never from the command's own
//! output.
//!
//! Only the verify test binaries include this module (`mod
//! verify_common;`, after `mod common;` and `mod lock_common;`).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use specengine_store::{ProposalFilter, ProposalQueue as _, SqliteQueue};

use crate::common::{Run, copy_dir, data_dir, fixture, snapshot};
use crate::lock_common::{LockRepo, blocks, value};

/// The second fixture: sound bindings, a different genre.
pub const ECS: &str = "verify-ecs";

/// verify-ecs's nodes, in the order the tests accept them.
pub const ECS_REFS: [&str; 4] = ["PERK-STAMINA", "PERK-REGEN", "PERK-DRAIN", "TUNE-01"];

/// code-a's paths.
pub const COUNTER: &str = "crates/tally/src/counter.rs";
pub const LIB: &str = "crates/tally/src/lib.rs";
pub const BOUNDS: &str = "crates/tally/tests/bounds.rs";
pub const BROKEN: &str = "crates/tally/src/broken.rs";
pub const LIMITS: &str = "data/limits.ron";
pub const COUNTER_MD: &str = "docs/spec/counter.md";
pub const GOAL_MD: &str = "docs/records/GOAL-01.md";

/// verify-ecs's paths.
pub const ECS_LIB: &str = "crates/game/src/lib.rs";
pub const STAMINA: &str = "crates/game/src/stamina.rs";
pub const MOVEMENT: &str = "assets/data/movement.ron";
pub const STAMINA_MD: &str = "docs/spec/stamina.md";

/// The three modes of `[verify]`, loosest first.
pub const MODES: [&str; 3] = ["observe", "enforce-introduced", "enforce"];

/// The keys of the JSON document, in order ("Output").
pub const TOP_KEYS: [&str; 9] = [
    "mode", "base", "verdict", "counts", "cascade", "rows", "delta", "cannot", "notes",
];

/// The keys of a row, in order ("Output"; `levels` after `compared` since
/// docs/features/lock-levels-verify.md "Level-aware verify", **JSON**;
/// `anchor` after `levels` since docs/features/anchor-states.md "Output").
pub const ROW_KEYS: [&str; 17] = [
    "node",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "state",
    "category",
    "detail",
    "class",
    "pre_existing",
    "markers",
    "node_rev",
    "lock",
    "compared",
    "levels",
    "anchor",
];

/// The keys of a row's `levels` object, in order
/// (docs/features/lock-levels-verify.md "Level-aware verify", **JSON**).
pub const LEVELS_KEYS: [&str; 3] = ["lock", "markers", "changed"];

/// The golden lock's path.
pub fn golden_file() -> std::path::PathBuf {
    fixture("spec-lock").join("code-a.lock")
}

/// `fixtures/code-a` committed as `the fixture`, the golden lock committed
/// on top as `the lock`.
pub fn golden(label: &str) -> LockRepo {
    let repo = LockRepo::code_a(label);
    repo.write(
        "spec.lock",
        fs::read(golden_file()).expect("fixtures/spec-lock/code-a.lock"),
    );
    repo.commit_all("the lock");
    repo
}

/// `fixtures/code-a` committed as `the fixture`, the format-1 golden
/// (`fixtures/spec-lock/code-a.format-1.lock`: the golden's bytes before
/// docs/features/lock-levels.md) committed on top as `the lock`: what a
/// project not yet rebased holds (Q10).
pub fn golden_format_1(label: &str) -> LockRepo {
    let repo = LockRepo::code_a(label);
    repo.write(
        "spec.lock",
        fs::read(fixture("spec-lock").join("code-a.format-1.lock"))
            .expect("fixtures/spec-lock/code-a.format-1.lock"),
    );
    repo.commit_all("the lock");
    repo
}

/// `fixtures/verify-ecs` committed once, its four nodes accepted (one lock
/// commit each).
pub fn ecs(label: &str) -> LockRepo {
    let repo = LockRepo::of(label, ECS, "repo", &[]);
    for reference in ECS_REFS {
        repo.accept_ok(reference);
    }
    repo
}

/// `fixtures/verify-ecs` committed once, nothing accepted.
pub fn ecs_unlocked(label: &str) -> LockRepo {
    LockRepo::of(label, ECS, "repo", &[])
}

/// A copy of `repo`'s worktree and `.git` at `<scratch>/<dir>`.
pub fn clone_dir(repo: &LockRepo, dir: &str) -> std::path::PathBuf {
    let to = repo.scratch.join(dir);
    copy_dir(&repo.root, &to);
    fs::canonicalize(&to).expect("the copy")
}

/// One `spec verify` from `cwd`, text and `--json`.
pub struct Verify {
    pub text: Run,
    pub json_run: Run,
    pub json: Value,
}

impl Verify {
    /// The exit code, the same for both forms.
    pub fn code(&self) -> i32 {
        self.text.code
    }

    /// The row matching every given key field (`None`: the field is
    /// JSON `null`), exactly one.
    pub fn row(&self, node: &str, relation: &str, file: &str, symbol: Option<&str>) -> &Value {
        let found: Vec<&Value> = self
            .rows()
            .into_iter()
            .filter(|row| {
                row["node"] == node
                    && row["relation"] == relation
                    && row["file"] == file
                    && row["symbol"] == json!(symbol)
            })
            .collect();
        assert_eq!(
            found.len(),
            1,
            "one row {node} {relation} {file} {symbol:?}: {found:#?}\n{}",
            self.text.show()
        );
        found[0]
    }

    /// The rows matching `wanted`.
    pub fn rows_where(&self, wanted: impl Fn(&Value) -> bool) -> Vec<&Value> {
        self.rows().into_iter().filter(|row| wanted(row)).collect()
    }

    pub fn rows(&self) -> Vec<&Value> {
        self.json["rows"]
            .as_array()
            .expect("`rows` is an array")
            .iter()
            .collect()
    }

    /// The rows with a lock entry.
    pub fn lock_rows(&self) -> Vec<&Value> {
        self.rows_where(|row| !row["lock"].is_null())
    }

    /// stdout's lines.
    pub fn lines(&self) -> Vec<&str> {
        self.text.stdout.lines().collect()
    }

    /// stdout holds `line` as a whole line.
    pub fn has_line(&self, line: &str) {
        assert!(
            self.lines().contains(&line),
            "no line {line:?} in:\n{}",
            self.text.show()
        );
    }

    /// The summary line (stdout's last).
    pub fn summary(&self) -> &str {
        self.lines().last().copied().unwrap_or_default()
    }

    pub fn verdict(&self) -> &str {
        self.json["verdict"].as_str().expect("a verdict")
    }
}

/// The worktree holding `cwd`: the nearest directory with a `.git`.
fn worktree_of(cwd: &Path) -> std::path::PathBuf {
    cwd.ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(cwd)
        .to_path_buf()
}

/// TE-7 after every run: every byte under the worktree (`.git` with its
/// HEAD, refs, index and objects; `spec.lock`; the files `git status`
/// reads) as before, and every queue of the data directory without a
/// proposal or an event.
fn assert_read_only(
    repo: &LockRepo,
    top: &Path,
    before: &BTreeMap<String, Option<Vec<u8>>>,
    args: &[&str],
) {
    let after = snapshot(top);
    let changed: Vec<&String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .collect();
    assert!(
        changed.is_empty(),
        "spec {args:?} wrote under the worktree: {changed:?}"
    );
    let Ok(entries) = fs::read_dir(data_dir(&repo.home)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "db") {
            continue;
        }
        let slug = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("a UTF-8 slug")
            .to_owned();
        let queue = SqliteQueue::open(&path, &slug).expect("the queue opens");
        assert!(
            queue
                .list(&ProposalFilter::default())
                .expect("list")
                .is_empty(),
            "spec {args:?}: a proposal in {slug}"
        );
        assert!(
            queue.events().expect("events").is_empty(),
            "spec {args:?}: an event in {slug}"
        );
    }
}

/// `spec verify <extra>` and `spec verify <extra> --json` from `cwd`:
/// both exit alike, the JSON is one document; each run read-only
/// ([`assert_read_only`]).
pub fn verify_at(repo: &LockRepo, cwd: &Path, extra: &[&str]) -> Verify {
    let top = worktree_of(cwd);
    let mut args = vec!["verify"];
    args.extend_from_slice(extra);
    let before = snapshot(&top);
    let text = repo.spec_piped(cwd, &args);
    assert_read_only(repo, &top, &before, &args);
    args.push("--json");
    let json_run = repo.spec_piped(cwd, &args);
    assert_read_only(repo, &top, &before, &args);
    assert_eq!(
        text.code,
        json_run.code,
        "text and JSON exit alike:\n{}\n{}",
        text.show(),
        json_run.show()
    );
    let json = json_run.json();
    Verify {
        text,
        json_run,
        json,
    }
}

/// [`verify_at`] from the root.
pub fn verify(repo: &LockRepo, extra: &[&str]) -> Verify {
    verify_at(repo, &repo.root, extra)
}

/// `[verify] mode = "<mode>"` appended to the root's config (written, not
/// committed).
pub fn set_mode(root: &Path, mode: &str) {
    let path = root.join("specengine.toml");
    let mut text = fs::read_to_string(&path).expect("specengine.toml");
    text.push_str(&format!("\n[verify]\nmode = \"{mode}\"\n"));
    fs::write(path, text).expect("specengine.toml");
}

/// The class "States" gives a state; `null` for `ok`.
pub fn class_of(state: &str) -> Value {
    match state {
        "cannot_verify" => json!("cannot"),
        "predated" | "bad_marker" | "broken_binding" | "conflict" => json!("fix"),
        "unlocked" | "spec_ahead" | "code_ahead" => json!("check"),
        "ok" => Value::Null,
        other => panic!("no state {other}"),
    }
}

/// A row's key fields.
#[derive(Debug, Clone, Copy)]
pub struct Key<'a> {
    pub node: &'a str,
    pub relation: &'a str,
    pub file: &'a str,
    pub kind: Option<&'a str>,
    pub symbol: Option<&'a str>,
    pub item_kind: Option<&'a str>,
}

/// An item's key.
pub fn item<'a>(
    node: &'a str,
    relation: &'a str,
    file: &'a str,
    symbol: &'a str,
    item_kind: &'a str,
) -> Key<'a> {
    Key {
        node,
        relation,
        file,
        kind: Some("item"),
        symbol: Some(symbol),
        item_kind: Some(item_kind),
    }
}

/// A module's key.
pub fn module<'a>(node: &'a str, relation: &'a str, file: &'a str, symbol: &'a str) -> Key<'a> {
    Key {
        node,
        relation,
        file,
        kind: Some("module"),
        symbol: Some(symbol),
        item_kind: None,
    }
}

/// A RON value's key.
pub fn ron<'a>(node: &'a str, relation: &'a str, file: &'a str, symbol: &'a str) -> Key<'a> {
    Key {
        node,
        relation,
        file,
        kind: Some("ron"),
        symbol: Some(symbol),
        item_kind: None,
    }
}

/// A keyless binding's key (`kind`, `symbol`, `item_kind` null).
pub fn keyless<'a>(node: &'a str, relation: &'a str, file: &'a str) -> Key<'a> {
    Key {
        node,
        relation,
        file,
        kind: None,
        symbol: None,
        item_kind: None,
    }
}

/// What a row says beyond its key.
#[derive(Debug, Clone)]
pub struct Says<'a> {
    pub state: &'a str,
    pub category: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub pre_existing: bool,
    /// `(line, rev)` of each marker.
    pub markers: &'a [(usize, Option<u32>)],
    pub node_rev: Option<u32>,
    pub lock: Value,
    pub compared: Option<&'a str>,
    /// The row's `levels` object ([`levels`]), `null` without an entry or a
    /// bound marker, or with an `Invalid` list.
    pub levels: Value,
}

/// The full JSON row of `key` saying `says`.
pub fn row_json(key: Key<'_>, says: &Says<'_>) -> Value {
    json!({
        "node": key.node,
        "relation": key.relation,
        "file": key.file,
        "kind": key.kind,
        "symbol": key.symbol,
        "item_kind": key.item_kind,
        "state": says.state,
        "category": says.category,
        "detail": says.detail,
        "class": class_of(says.state),
        "pre_existing": says.pre_existing,
        "markers": says
            .markers
            .iter()
            .map(|(line, rev)| json!({"line": line, "rev": rev}))
            .collect::<Vec<_>>(),
        "node_rev": says.node_rev,
        "lock": says.lock,
        "compared": says.compared,
        "levels": says.levels,
        "anchor": default_anchor(says),
    })
}

/// The `anchor` docs/features/anchor-states.md "Anchors per row" gives a
/// row nothing renamed or moved: `null` without an entry or on
/// `cannot_verify`; `exact` with an entry and a marker at its key; else
/// (an entry, no binding at its key, not followed) `lost`. A moved or
/// ambiguous row's anchor is the anchor tests' own.
pub fn default_anchor(says: &Says<'_>) -> Value {
    if says.lock.is_null() || says.state == "cannot_verify" {
        return Value::Null;
    }
    let state = if says.markers.is_empty() {
        "lost"
    } else {
        "exact"
    };
    json!({"state": state, "from": null, "candidates": null})
}

/// A row's `levels` object (docs/features/lock-levels-verify.md
/// "Level-aware verify", **JSON**): `lock` the entry's `levels`, `markers`
/// the union of the bound markers' effective levels (no list: `sig`,
/// `body`), `changed` the compared levels whose digests differ, in
/// canonical order (`None`: no digest compared, so `null`).
pub fn levels(lock: &[&str], markers: &[&str], changed: Option<&[&str]>) -> Value {
    json!({"lock": lock, "markers": markers, "changed": changed})
}

/// [`levels`] of a row whose entry and markers name the same levels and
/// whose digests were not compared (`ok` by `blob`, or a state decided
/// before the comparison).
pub fn same_levels(both: &[&str]) -> Value {
    levels(both, both, None)
}

/// The `levels` line of `text`'s entry for `key`, its names in order.
pub fn entry_levels(text: &str, key: Key<'_>) -> Vec<String> {
    let found: Vec<_> = blocks(text)
        .into_iter()
        .filter(|block| {
            value(block, "node") == Some(key.node)
                && value(block, "relation") == Some(key.relation)
                && value(block, "file") == Some(key.file)
                && value(block, "kind") == key.kind
                && value(block, "symbol") == key.symbol
                && value(block, "item_kind") == key.item_kind
        })
        .collect();
    assert_eq!(found.len(), 1, "one entry for {key:?}");
    let line = found[0]
        .iter()
        .find(|(name, _)| name == "levels")
        .map(|(_, list)| list.clone())
        .unwrap_or_else(|| panic!("no `levels` in the entry for {key:?}"));
    let inner = line
        .trim()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or_else(|| panic!("`levels = {line}` is no list"));
    inner
        .split(',')
        .map(|name| name.trim().trim_matches('"').to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// The text line "Output" gives the row (`class | state[ category][
/// detail] | node | relation | file | kind symbol[ item_kind]`); a keyless
/// binding prints `binding` in place of the kind and symbol.
pub fn row_line(key: Key<'_>, says: &Says<'_>, binding: Option<&str>) -> String {
    let class = class_of(says.state);
    let class = class.as_str().unwrap_or("ok");
    let mut state = says.state.to_owned();
    for part in [says.category, says.detail].into_iter().flatten() {
        state.push(' ');
        state.push_str(part);
    }
    let binding = match (binding, key.kind, key.symbol) {
        (Some(binding), ..) => binding.to_owned(),
        (None, Some(kind), Some(symbol)) => match key.item_kind {
            Some(item_kind) => format!("{kind} {symbol} {item_kind}"),
            None => format!("{kind} {symbol}"),
        },
        _ => panic!("a keyless row names its binding"),
    };
    format!(
        "{}{class} | {state} | {} | {} | {} | {binding}",
        if says.pre_existing {
            "pre-existing "
        } else {
            ""
        },
        key.node,
        key.relation,
        key.file,
    )
}

/// The `lock` object of the golden's entry for the key: its `rev`,
/// `commit` and `verified_at`, read from the fixture.
pub fn golden_lock(key: Key<'_>) -> Value {
    lock_entry(
        &fs::read_to_string(golden_file()).expect("the golden lock"),
        key,
    )
}

/// The `lock` object of `text`'s entry for `key`.
pub fn lock_entry(text: &str, key: Key<'_>) -> Value {
    let found: Vec<_> = blocks(text)
        .into_iter()
        .filter(|block| {
            value(block, "node") == Some(key.node)
                && value(block, "relation") == Some(key.relation)
                && value(block, "file") == Some(key.file)
                && value(block, "kind") == key.kind
                && value(block, "symbol") == key.symbol
                && value(block, "item_kind") == key.item_kind
        })
        .collect();
    assert_eq!(found.len(), 1, "one entry for {key:?}");
    let block = &found[0];
    json!({
        "rev": value(block, "rev").expect("rev").parse::<u32>().expect("a number"),
        "commit": value(block, "commit").expect("commit"),
        "verified_at": value(block, "verified_at").expect("verified_at"),
    })
}

/// Asserts the row of `key` is exactly `says` in the JSON and prints as
/// [`row_line`] in the text (an `ok` row prints nothing).
pub fn assert_row(run: &Verify, key: Key<'_>, says: &Says<'_>, binding: Option<&str>) {
    let expected = row_json(key, says);
    let found: Vec<&Value> = run.rows_where(|row| {
        ["node", "relation", "file", "kind", "symbol", "item_kind"]
            .iter()
            .all(|field| row[*field] == expected[*field])
            && (key.kind.is_some()
                || (row["state"] == expected["state"]
                    && row["category"] == expected["category"]
                    && row["detail"] == expected["detail"]))
    });
    assert_eq!(
        found,
        vec![&expected],
        "the row of {key:?}\n{}",
        run.text.show()
    );
    let line = row_line(key, says, binding);
    if says.state == "ok" {
        // An `ok` row is never printed: no line ends with its key.
        let tail = &line["ok | ok".len()..];
        assert!(
            !run.lines().iter().any(|printed| printed.ends_with(tail)),
            "an ok row is printed ({tail:?}):\n{}",
            run.text.show()
        );
    } else {
        run.has_line(&line);
    }
}
