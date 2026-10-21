//! docs/features/lock-diff.md (RM-0033, 1 of 2; ADR-0053;
//! docs/canon/spec-lock-diff.md): `spec lock diff REF [--json]`, the old
//! sides of each drifted binding of REF's holders against the working tree
//! as git's hunks. "Acceptance criteria" RD-1..RD-6, RD-8 and TE-1; each
//! test names the mutation `Mn` of the spec that turns it red.
//!
//! **Fixtures** are made at test time (TE-1), no new corpus: a scratch git
//! copy of `fixtures/code-a` (`lock_common::LockRepo`: the sandboxed git,
//! fixed identity and dates, a scratch `HOME`) after `spec lock accept
//! LAW-BOUND` through the library at the injected clock; the golden lock
//! (`verify_common::golden`) where a lock whose `commit` names no object is
//! the subject; `fixtures/verify-ecs` (an ECS game crate, its tuning in
//! RON) for a second genre (ADR-0008); `fixtures/spec-c` (no `[code]`).
//!
//! **The oracle** is `spec verify --node REF` on the same state (text and
//! `--json`, stdin null): the kept rows are its rows whose `lock` is not
//! null and whose state raises drift, in its order, each `row` byte-equal
//! to its row ([`Diffed::assert_contract`]); a side's presence follows
//! "Sides" from the row and its `spec.lock` entry (the row's key, a moved
//! row's `anchor.from`); the text is derived from the JSON document by
//! "Text" with verify's own row lines ([`expected_text`]); expected hunks
//! are "Data"'s example or `git diff --no-index` run here over the texts
//! the spec names ([`git_hunks`]). Never the command's own output.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod bindings_common;
mod code_index_common;
mod common;
mod lock_common;
mod verify_common;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bindings_common::{Read, read_with, run_with, text, vars_of, with_var, without_code};
use common::git::Sandbox;
use common::{Scratch, copy_dir, data_dir, fixture, snapshot};
use lock_common::rebase::{edit_block, set_string};
use lock_common::{CLOCK, LockRepo, blocks, fake_oid, value};
use serde_json::Value;
use specengine_code::TRIVIAL_BODY_BYTES;
use specengine_store::{ProposalQueue as _, SqliteQueue};
use verify_common::{BOUNDS, COUNTER, COUNTER_MD, LIB, LIMITS, MOVEMENT, STAMINA, ecs, golden};

/// code-a's slug: its queue database's name.
const SLUG: &str = "code-a";

/// The REF of "Data"'s example.
const LAW: &str = "LAW-BOUND";

/// LAW-BOUND's sentence and "Data"'s edit of it.
const OLD_SENTENCE: &str = "The count never exceeds the cap.";
const NEW_SENTENCE: &str = "The count never exceeds the cap, twelve by default.";

/// `CAP`'s value and "Data"'s edit of it.
const OLD_CAP: &str = "CAP: u32 = 10;";
const NEW_CAP: &str = "CAP: u32 = 12;";

/// The spec side of "Data"'s example: LAW-BOUND's span as `spec show`
/// cuts it ("Spec texts"), carried through its last line's LF (so no `\ No
/// newline at end of file`), hunk lines counted from the span's first
/// line. "Data"'s example also shows the blank line after the span (`-1,4
/// +1,4`): the cut wins over it (the coordinator's ruling, 2026-10-21).
const LAW_HUNKS: &str = "@@ -1,3 +1,3 @@\n ## Bound {#LAW-BOUND rev=2}\n \n-The count never exceeds the cap.\n+The count never exceeds the cap, twelve by default.\n";

/// "Data"'s code side: `CAP`'s whole line, nothing around it.
const CAP_HUNKS: &str = "@@ -1 +1 @@\n-pub const CAP: u32 = 10; // @configures LAW-BOUND@2\n+pub const CAP: u32 = 12; // @configures LAW-BOUND@2\n";

/// `CAP`'s row key as verify's text prints it ("Notes").
const CAP_KEY: &str = "LAW-BOUND | configures | crates/tally/src/counter.rs | item crates/tally::counter::CAP const_item";

/// The note of a moved side git prints no hunk for (its bytes binary to
/// git: a NUL), iteration 2's words ("Rules and edge cases": `null` with
/// one note, never `""`).
const NO_HUNK: &str = "git shows no hunk for bytes that differ";

/// N0 without `[code]` (`code-reads.md` "Degrade", cause (2)).
const NO_CODE: &str =
    "bindings: specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The states core `drift::raises(state, detail, false)` holds.
const DRIFTING: [&str; 4] = ["code_ahead", "spec_ahead", "conflict", "broken_binding"];

/// "JSON"'s key sets, in order.
const TOP_KEYS: [&str; 4] = ["ref", "reason", "notes", "nodes"];
const NODE_KEYS: [&str; 4] = ["id", "path", "line", "rows"];
const ROW_KEYS: [&str; 3] = ["row", "spec", "code"];
const SPEC_KEYS: [&str; 3] = ["commit", "path", "diff"];
const CODE_KEYS: [&str; 3] = ["blob", "file", "diff"];

/// The look-alike of `LAW-BOUND` (a Cyrillic A).
const LAW_LOOK_ALIKE: &str = "L\u{410}W-BOUND";

/// A function bound to LAW-BOUND whose body passes `TRIVIAL_BODY_BYTES`, so
/// a move of it is followed (`spec-verify-anchors.md` "The pairing"):
/// `CAP`'s body is two bytes, and a trivial item is never followed.
const CLAMP: &str = "\n// @implements LAW-BOUND@2\npub fn clamp_all(values: &mut [u32]) -> u32 {\n    let mut clamped = 0;\n    let mut largest = 0;\n    for value in values.iter_mut() {\n        if *value > largest {\n            largest = *value;\n        }\n        if *value > CAP {\n            *value = CAP;\n            clamped += 1;\n        }\n    }\n    assert!(largest >= clamped, \"the largest value bounds the count\");\n    clamped\n}\n";

// -------------------------------------------------------------- raw JSON

fn skip_ws(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    at
}

/// The byte after the JSON value opening at `at`.
fn value_end(bytes: &[u8], at: usize) -> usize {
    match bytes[at] {
        b'"' => {
            let mut index = at + 1;
            loop {
                match bytes[index] {
                    b'\\' => index += 2,
                    b'"' => return index + 1,
                    _ => index += 1,
                }
            }
        }
        b'{' | b'[' => {
            let (mut depth, mut index) = (0usize, at);
            loop {
                match bytes[index] {
                    b'"' => {
                        index = value_end(bytes, index);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return index + 1;
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
        }
        _ => {
            let mut index = at;
            while index < bytes.len()
                && !matches!(bytes[index], b',' | b'}' | b']')
                && !bytes[index].is_ascii_whitespace()
            {
                index += 1;
            }
            index
        }
    }
}

/// The members of the JSON object `raw`, each key with its value's bytes
/// as printed, in the order printed.
fn members(raw: &str) -> Vec<(String, String)> {
    let bytes = raw.as_bytes();
    let mut at = skip_ws(bytes, 0);
    assert_eq!(bytes.get(at), Some(&b'{'), "an object: {raw}");
    at = skip_ws(bytes, at + 1);
    let mut out = Vec::new();
    if bytes[at] == b'}' {
        return out;
    }
    loop {
        let end = value_end(bytes, at);
        let key: String = serde_json::from_str(&raw[at..end]).expect("a key");
        at = skip_ws(bytes, end);
        assert_eq!(bytes[at], b':', "a member: {raw}");
        at = skip_ws(bytes, at + 1);
        let end = value_end(bytes, at);
        out.push((key, raw[at..end].to_owned()));
        at = skip_ws(bytes, end);
        match bytes[at] {
            b',' => at = skip_ws(bytes, at + 1),
            b'}' => return out,
            other => panic!("`{}` after a member: {raw}", other as char),
        }
    }
}

/// The elements of the JSON array `raw`, each as printed.
fn elements(raw: &str) -> Vec<String> {
    let bytes = raw.as_bytes();
    let mut at = skip_ws(bytes, 0);
    assert_eq!(bytes.get(at), Some(&b'['), "an array: {raw}");
    at = skip_ws(bytes, at + 1);
    let mut out = Vec::new();
    if bytes[at] == b']' {
        return out;
    }
    loop {
        let end = value_end(bytes, at);
        out.push(raw[at..end].to_owned());
        at = skip_ws(bytes, end);
        match bytes[at] {
            b',' => at = skip_ws(bytes, at + 1),
            b']' => return out,
            other => panic!("`{}` after an element: {raw}", other as char),
        }
    }
}

/// `key`'s value in the object `raw`, as printed.
fn member(raw: &str, key: &str) -> String {
    members(raw)
        .into_iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("no `{key}` in {raw}"))
}

/// The keys of the object `raw`, in the order printed.
fn keys(raw: &str) -> Vec<String> {
    members(raw).into_iter().map(|(key, _)| key).collect()
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

// ----------------------------------------------------------------- oracle

/// `spec verify --node REF` on the same state: its text and its kept rows
/// (`lock` not null, a drifting state), raw, in order.
struct Oracle {
    text: String,
    kept: Vec<String>,
    all: Vec<Value>,
}

fn oracle_in(cwd: &Path, reference: &str, vars: &[(OsString, OsString)]) -> Oracle {
    let text_run = run_with(cwd, &["verify", "--node", reference], vars);
    let json_run = run_with(cwd, &["verify", "--node", reference, "--json"], vars);
    assert!(
        [0, 1, 2].contains(&json_run.code) && json_run.stdout.ends_with('\n'),
        "premise: verify --node {reference} answers a document\n{}",
        json_run.show()
    );
    let document = json_run.stdout.trim_end_matches('\n');
    let omitted: Value = serde_json::from_str(&member(document, "omitted")).expect("omitted");
    assert_eq!(
        omitted["rows"], 0,
        "premise: verify --node {reference} printed every row"
    );
    let raw_rows = elements(&member(document, "rows"));
    let all: Vec<Value> = raw_rows
        .iter()
        .map(|raw| serde_json::from_str(raw).expect("a row"))
        .collect();
    let kept = raw_rows
        .into_iter()
        .zip(&all)
        .filter(|(_, row)| !row["lock"].is_null() && DRIFTING.contains(&text(&row["state"])))
        .map(|(raw, _)| raw)
        .collect();
    Oracle {
        text: text_run.stdout,
        kept,
        all,
    }
}

// ------------------------------------------------------------------- runs

/// One `spec lock diff REF` (text, then `--json`) and its oracle.
struct Diffed {
    read: Read,
    /// The JSON document without its final LF.
    raw: String,
    doc: Value,
    oracle: Oracle,
}

impl Diffed {
    /// Every kept row of every holder, as parsed.
    fn rows(&self) -> Vec<Value> {
        self.doc["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .flat_map(|node| node["rows"].as_array().cloned().unwrap_or_default())
            .collect()
    }

    /// The one kept row whose `row` has `symbol`.
    fn row(&self, symbol: &str) -> Value {
        let found: Vec<Value> = self
            .rows()
            .into_iter()
            .filter(|row| row["row"]["symbol"] == symbol)
            .collect();
        assert_eq!(
            found.len(),
            1,
            "one kept row on {symbol}\n{}",
            self.read.show()
        );
        found.into_iter().next().expect("one")
    }

    fn notes(&self) -> Vec<String> {
        self.doc["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .map(|note| text(note).to_owned())
            .collect()
    }

    fn show(&self) -> String {
        self.read.show()
    }

    /// The contract every answered read keeps ("JSON", "Sides", "Text",
    /// "Notes"), checked against the oracle and the lock.
    #[track_caller]
    fn assert_contract(&self, repo: &LockRepo, reference: &str) {
        let what = self.show();
        assert_eq!(keys(&self.raw), owned(&TOP_KEYS), "{what}");
        assert_eq!(self.doc["ref"], reference, "`ref` raw: {what}");
        assert!(self.doc["reason"].is_null(), "{what}");
        // stderr: one `note:` line per `notes` entry, in order.
        let printed: Vec<String> = self
            .read
            .text
            .stderr
            .lines()
            .filter(|line| !line.starts_with("warning: "))
            .map(str::to_owned)
            .collect();
        let noted: Vec<String> = self
            .notes()
            .iter()
            .map(|note| format!("note: {note}"))
            .collect();
        assert_eq!(printed, noted, "stderr is the notes: {what}");
        for printed in [&self.read.text.stdout, &self.read.text.stderr, &self.raw] {
            assert_private(printed, repo, "the output");
        }
        // Holders, rows, the oracle's rows byte for byte.
        let mut raw_rows = Vec::new();
        for node in elements(&member(&self.raw, "nodes")) {
            assert_eq!(keys(&node), owned(&NODE_KEYS), "{what}");
            let rows = member(&node, "rows");
            if rows != "null" {
                raw_rows.extend(elements(&rows));
            }
        }
        let rows_of_rows: Vec<String> = raw_rows
            .iter()
            .map(|raw| {
                assert_eq!(keys(raw), owned(&ROW_KEYS), "{what}");
                member(raw, "row")
            })
            .collect();
        if self.rows_judged() {
            assert_eq!(
                rows_of_rows, self.oracle.kept,
                "the kept rows are verify --node's drifting rows with a lock entry, byte for byte, in its order: {what}"
            );
        }
        let lock = repo
            .lock()
            .map(|bytes| String::from_utf8(bytes).expect("UTF-8 lock"));
        let mut norms: BTreeMap<String, String> = BTreeMap::new();
        for (raw, kept) in raw_rows.iter().zip(self.rows()) {
            let row = &kept["row"];
            assert_eq!(row["pre_existing"], false, "{what}");
            let entry = entry_of(lock.as_deref().expect("a lock"), row);
            let spec = member(raw, "spec");
            let code = member(raw, "code");
            if spec != "null" {
                assert_eq!(keys(&spec), owned(&SPEC_KEYS), "{what}");
                assert_eq!(
                    kept["spec"]["commit"],
                    value(&entry, "commit").expect("commit"),
                    "spec.commit is the entry's: {what}"
                );
                assert_eq!(
                    kept["spec"]["path"],
                    node_path(repo, text(&row["node"])),
                    "spec.path holds the row's node now: {what}"
                );
                assert!(
                    kept["spec"]["diff"].is_null() || kept["spec"]["diff"].is_string(),
                    "{what}"
                );
            }
            if code != "null" {
                assert_eq!(keys(&code), owned(&CODE_KEYS), "{what}");
                assert_eq!(
                    kept["code"]["blob"],
                    value(&entry, "file_blob").expect("file_blob"),
                    "code.blob is the entry's file_blob: {what}"
                );
                assert_eq!(
                    kept["code"]["file"],
                    value(&entry, "file").expect("file"),
                    "code.file is the entry's file: {what}"
                );
                assert!(
                    kept["code"]["diff"].is_null() || kept["code"]["diff"].is_string(),
                    "{what}"
                );
            }
            let (want_spec, want_code) = sides_of(row, &entry, || {
                let node = text(&row["node"]).to_owned();
                norms
                    .entry(node.clone())
                    .or_insert_with(|| repo.norm_hash(&node))
                    .clone()
            });
            assert_eq!(
                (spec != "null", code != "null"),
                (want_spec, want_code),
                "\"Sides\" for {} {}: {what}",
                text(&row["state"]),
                row["category"]
            );
            let key = key_text(row);
            if kept["spec"]["diff"].is_null() && spec != "null" {
                assert!(
                    self.notes()
                        .iter()
                        .any(|note| note.starts_with(&format!("lock diff: {key}: no spec diff: "))),
                    "a spec side not shown has its note: {what}"
                );
            }
            if kept["code"]["diff"].is_null() && code != "null" {
                assert!(
                    self.notes()
                        .iter()
                        .any(|note| note.starts_with(&format!("lock diff: {key}: no code diff: "))),
                    "a code side not shown has its note: {what}"
                );
            }
            let whole = match text(&row["kind"]) {
                "module" => Some("a module binding"),
                "ron" => Some("a ron binding"),
                _ => None,
            };
            if let (Some(why), Some(_)) = (whole, kept["code"]["diff"].as_str()) {
                let n6 = format!("lock diff: {key}: the code diff is the whole file: {why}");
                assert!(self.notes().contains(&n6), "N6 {n6:?}: {what}");
            }
            if matches!(text(&row["category"]), "file_gone" | "symbol_gone")
                && let Some(diff) = kept["code"]["diff"].as_str()
            {
                assert!(
                    !diff.is_empty() && hunk_lines(diff, '+').is_empty(),
                    "{}: the new text is empty: {what}",
                    text(&row["category"])
                );
            }
        }
        assert_eq!(
            self.read.text.stdout,
            expected_text(&self.doc, reference.trim(), &self.oracle.text),
            "the text is \"Text\" of the document"
        );
    }

    /// No holder's `rows` is null.
    fn rows_judged(&self) -> bool {
        self.doc["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .all(|node| !node["rows"].is_null())
    }
}

/// `spec lock diff REF` (text and `--json`) from `cwd` with `vars`.
fn lock_diff_in(cwd: &Path, reference: &str, vars: &[(OsString, OsString)]) -> Read {
    read_with(cwd, &["lock", "diff", reference], vars)
}

/// An answered `spec lock diff REF` from `cwd` with `vars`, its oracle,
/// the contract checked.
#[track_caller]
fn diffed_in(
    repo: &LockRepo,
    cwd: &Path,
    reference: &str,
    vars: &[(OsString, OsString)],
) -> Diffed {
    let read = lock_diff_in(cwd, reference, vars);
    read.answered();
    let raw = read
        .json_run
        .stdout
        .strip_suffix('\n')
        .unwrap_or_else(|| panic!("one LF ends the document\n{}", read.show()))
        .to_owned();
    assert!(!raw.contains('\n'), "one compact document\n{}", read.show());
    let doc: Value = serde_json::from_str(&raw).expect("the document");
    let oracle = oracle_in(cwd, reference, vars);
    let diffed = Diffed {
        read,
        raw,
        doc,
        oracle,
    };
    diffed.assert_contract(repo, reference);
    diffed
}

/// [`diffed_in`] from the root with the repository's variables.
#[track_caller]
fn diffed(repo: &LockRepo, reference: &str) -> Diffed {
    diffed_in(repo, &repo.root, reference, &vars_of(repo))
}

/// verify's `<key>` of a row ("Notes"): `<node> | <relation> | <file> |
/// <kind> <symbol>[ <item_kind>]`.
fn key_text(row: &Value) -> String {
    let mut binding = format!("{} {}", text(&row["kind"]), text(&row["symbol"]));
    if let Some(item_kind) = row["item_kind"].as_str() {
        binding.push(' ');
        binding.push_str(item_kind);
    }
    format!(
        "{} | {} | {} | {binding}",
        text(&row["node"]),
        text(&row["relation"]),
        text(&row["file"])
    )
}

/// verify's printed line of a non-`ok` keyed row
/// (`spec-verify-output.md` "Text").
fn verify_row_line(row: &Value) -> String {
    let mut state = text(&row["state"]).to_owned();
    for part in ["category", "detail"] {
        if let Some(word) = row[part].as_str() {
            state.push(' ');
            state.push_str(word);
        }
    }
    let mut binding = format!("{} {}", text(&row["kind"]), text(&row["symbol"]));
    if let Some(item_kind) = row["item_kind"].as_str() {
        binding.push(' ');
        binding.push_str(item_kind);
    }
    format!(
        "{} | {state} | {} | {} | {} | {binding}",
        text(&row["class"]),
        text(&row["node"]),
        text(&row["relation"]),
        text(&row["file"])
    )
}

/// "Text" of `doc`: per holder its line, its rows (verify's row block as
/// verify printed it: the row line, its `  moved from` or `  candidate`
/// lines; then the sides), the summary.
fn expected_text(doc: &Value, reference: &str, verify_text: &str) -> String {
    let verify_lines: Vec<&str> = verify_text.lines().collect();
    let mut out = String::new();
    let (mut rows, mut specs, mut codes, mut hidden) = (0, 0, 0, 0);
    let mut judged = true;
    for node in doc["nodes"].as_array().expect("nodes") {
        out.push_str(&format!(
            "{} | {}:{}\n",
            node["id"].as_str().unwrap_or("-"),
            text(&node["path"]),
            node["line"]
        ));
        let Some(kept) = node["rows"].as_array() else {
            out.push_str("  not judged\n");
            judged = false;
            continue;
        };
        if kept.is_empty() {
            out.push_str("  no drifted row with a lock entry\n");
        }
        for row in kept {
            rows += 1;
            out.push('\n');
            let line = verify_row_line(&row["row"]);
            let at = verify_lines
                .iter()
                .position(|printed| *printed == line)
                .unwrap_or_else(|| panic!("verify --node prints {line:?}:\n{verify_text}"));
            out.push_str(&line);
            out.push('\n');
            for next in verify_lines[at + 1..].iter().take_while(|next| {
                next.starts_with("  moved from ") || next.starts_with("  candidate ")
            }) {
                out.push_str(next);
                out.push('\n');
            }
            let mut side = |head: String, diff: &Value| {
                out.push_str(&head);
                match diff.as_str() {
                    None => {
                        hidden += 1;
                        out.push_str(": not shown\n");
                    }
                    Some("") => out.push_str(": unchanged\n"),
                    Some(hunks) => {
                        out.push('\n');
                        out.push_str(hunks);
                    }
                }
            };
            let spec = &row["spec"];
            if !spec.is_null() {
                specs += 1;
                side(
                    format!(
                        "  spec {} at {}",
                        text(&spec["path"]),
                        text(&spec["commit"])
                    ),
                    &spec["diff"],
                );
            }
            let code = &row["code"];
            if !code.is_null() {
                codes += 1;
                side(
                    format!(
                        "  code {} blob {}",
                        text(&code["file"]),
                        text(&code["blob"])
                    ),
                    &code["diff"],
                );
            }
        }
    }
    out.push('\n');
    if judged {
        out.push_str(&format!(
            "lock diff {reference}: rows {rows}, spec sides {specs}, code sides {codes}"
        ));
        if hidden > 0 {
            out.push_str(&format!(", not shown {hidden}"));
        }
        out.push('\n');
    } else {
        out.push_str(&format!("lock diff {reference}: not judged\n"));
    }
    out
}

/// The `spec.lock` block of `row`'s key; a moved row's by `anchor.from`.
fn entry_of(lock: &str, row: &Value) -> Vec<(String, String)> {
    let at = if row["anchor"]["state"] == "moved" {
        &row["anchor"]["from"]
    } else {
        row
    };
    let found: Vec<Vec<(String, String)>> = blocks(lock)
        .into_iter()
        .filter(|block| {
            value(block, "node") == row["node"].as_str()
                && value(block, "relation") == row["relation"].as_str()
                && value(block, "file") == at["file"].as_str()
                && value(block, "kind") == at["kind"].as_str()
                && value(block, "symbol") == at["symbol"].as_str()
                && value(block, "item_kind") == at["item_kind"].as_str()
        })
        .collect();
    assert_eq!(found.len(), 1, "one entry of {row} in:\n{lock}");
    found.into_iter().next().expect("one")
}

/// "Sides": whether the row has a spec side and a code side. `norm` gives
/// the row's node's `norm_hash` now (fresh, as `spec show` prints it).
fn sides_of(
    row: &Value,
    entry: &[(String, String)],
    norm: impl FnOnce() -> String,
) -> (bool, bool) {
    let mut norm = Some(norm);
    let mut spec_moved = || {
        let rev = value(entry, "rev").expect("rev");
        row["node_rev"].as_u64() != rev.parse::<u64>().ok()
            || (norm.take().expect("asked once"))() != value(entry, "spec_hash").expect("spec_hash")
    };
    let levels = &row["levels"];
    let levels_moved = levels["changed"]
        .as_array()
        .is_some_and(|changed| !changed.is_empty())
        || levels["lock"] != levels["markers"];
    match (text(&row["state"]), text(&row["category"])) {
        ("conflict", _) => (true, true),
        ("code_ahead", "code_changed" | "levels_changed") => (false, true),
        ("code_ahead", "raised") => (true, levels_moved),
        ("spec_ahead", "spec_changed" | "same_revision") => (true, false),
        ("spec_ahead", "marker_behind") => (spec_moved(), false),
        ("broken_binding", "node_gone") => (true, false),
        ("broken_binding", "file_gone" | "symbol_gone" | "marker_gone") => (spec_moved(), true),
        other => panic!("a kept row on no line of \"Sides\": {other:?} {row}"),
    }
}

/// The node's root-relative path now (`spec show <id> --json`).
fn node_path(repo: &LockRepo, id: &str) -> String {
    let run = repo.spec_piped(&repo.root, &["show", id, "--json"]);
    run.code(0);
    text(&run.json()["nodes"][0]["path"]).to_owned()
}

/// The lines of `hunks` starting with `sign`, without it.
fn hunk_lines(hunks: &str, sign: char) -> Vec<String> {
    hunks
        .lines()
        .filter(|line| !line.starts_with("@@") && line.starts_with(sign))
        .map(|line| line[1..].to_owned())
        .collect()
}

/// git's hunks of `old` against `new` as "The git door" asks for them
/// (`diff --no-index --no-color --no-ext-diff --diff-algorithm=myers
/// -U3`), from the first `@@`; `""` equal.
fn git_hunks(repo: &LockRepo, old: &[u8], new: &[u8]) -> String {
    let dir = repo.scratch.dir("oracle-hunks");
    fs::write(dir.join("old"), old).expect("old");
    fs::write(dir.join("new"), new).expect("new");
    let output = repo.git.git_output(
        &dir,
        &[
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "--diff-algorithm=myers",
            "-U3",
            "--",
            "old",
            "new",
        ],
        &[],
    );
    match output.status.code() {
        Some(0) => String::new(),
        Some(1) => {
            let text = String::from_utf8(output.stdout).expect("UTF-8 hunks");
            let at = if text.starts_with("@@") {
                0
            } else {
                text.find("\n@@").expect("a hunk") + 1
            };
            text[at..].to_owned()
        }
        other => panic!(
            "git diff --no-index: {other:?}\n{}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

// ---------------------------------------------------------- repositories

/// `fixtures/code-a` committed, `spec lock accept LAW-BOUND` through the
/// library at [`CLOCK`] (its lock committed).
fn accepted(label: &str) -> LockRepo {
    let repo = LockRepo::code_a(label);
    repo.accept_ok(LAW);
    repo
}

/// "Data"'s example: LAW-BOUND's sentence and `CAP` edited, uncommitted.
fn conflict_edits(repo: &LockRepo) {
    repo.replace(COUNTER_MD, OLD_SENTENCE, NEW_SENTENCE);
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
}

/// The lock's text with `edit` applied to the block of (node, relation,
/// symbol), written (uncommitted).
fn edit_entry(repo: &LockRepo, key: (&str, &str, &str), edit: impl FnOnce(&str) -> String) {
    let text = edit_block(&repo.lock_text(), key, edit);
    repo.write("spec.lock", text);
}

/// `CAP`'s lock key for [`edit_entry`].
const CAP_ENTRY: (&str, &str, &str) = (LAW, "configures", "crates/tally::counter::CAP");

/// Every working-tree file of `repo` restored to `files` (a snapshot of
/// the root, `.git` left alone).
fn restore(repo: &LockRepo, files: &BTreeMap<String, Option<Vec<u8>>>) {
    let now = snapshot(&repo.root);
    for (path, bytes) in &now {
        if path == ".git" || path.starts_with(".git/") || bytes.is_none() {
            continue;
        }
        if !files.contains_key(path) {
            fs::remove_file(repo.root.join(path)).expect("a file the scenario added");
        }
    }
    for (path, bytes) in files {
        if path == ".git" || path.starts_with(".git/") {
            continue;
        }
        if let Some(bytes) = bytes {
            repo.write(path, bytes);
        }
    }
    assert_eq!(
        repo.git(&["--no-optional-locks", "status", "--porcelain=v1", "-z"]),
        Vec::<u8>::new(),
        "premise: the scenario restored"
    );
}

// ------------------------------------------------------------- the shim

/// One logged git process: `GIT_OPTIONAL_LOCKS` and `GIT_NO_LAZY_FETCH`
/// as it ran (`unset` when not set) and its arguments from the subcommand
/// on (git's global options before it dropped).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    locks: String,
    lazy: String,
    args: Vec<String>,
}

/// A `git` first on `PATH` that appends `GIT_OPTIONAL_LOCKS`,
/// `GIT_NO_LAZY_FETCH` and its argv (each followed by U+001F, one line per
/// process) to a log, then runs the real git; `refuse_diff` makes it fail
/// every `diff --no-index`, its stderr naming every argument (the scratch
/// files' absolute paths among them).
struct Shim {
    dir: PathBuf,
    log: PathBuf,
}

impl Shim {
    fn new(repo: &LockRepo, name: &str, refuse_diff: bool) -> Self {
        let dir = repo.scratch.dir(name);
        let log = dir.join("argv.log");
        let real = repo.git.git_program().to_str().expect("a UTF-8 git path");
        let refuse = if refuse_diff {
            "case \" $* \" in *' diff --no-index '*) echo \"fatal: the test refuses git $*\" >&2; exit 128;; esac\n"
        } else {
            ""
        };
        let script = format!(
            "#!/bin/sh\nline=$(printf '%s\\037' \"${{GIT_OPTIONAL_LOCKS-unset}}\" \"${{GIT_NO_LAZY_FETCH-unset}}\" \"$@\")\nprintf '%s\\n' \"$line\" >> '{}'\n{refuse}exec '{real}' \"$@\"\n",
            log.to_str().expect("a UTF-8 scratch path")
        );
        let shim = dir.join("git");
        fs::write(&shim, script).expect("the shim");
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
        Self { dir, log }
    }

    /// `repo`'s variables with the shim first on `PATH`.
    fn vars(&self, repo: &LockRepo) -> Vec<(OsString, OsString)> {
        with_var(&vars_of(repo), "PATH", &repo.git.path_with(&self.dir))
    }

    /// Every logged call, in order.
    fn calls(&self) -> Vec<Call> {
        let log = fs::read_to_string(&self.log).unwrap_or_default();
        log.lines()
            .map(|line| {
                let mut args: Vec<String> = line.split('\u{1f}').map(str::to_owned).collect();
                if args.last().is_some_and(String::is_empty) {
                    args.pop();
                }
                let locks = args.remove(0);
                let lazy = args.remove(0);
                let mut at = 0;
                while let Some(arg) = args.get(at) {
                    match arg.as_str() {
                        "-c" | "-C" => at += 2,
                        global if global.starts_with('-') => at += 1,
                        _ => break,
                    }
                }
                Call {
                    locks,
                    lazy,
                    args: args.split_off(at.min(args.len())),
                }
            })
            .collect()
    }

    fn clear(&self) {
        let _ = fs::remove_file(&self.log);
    }
}

/// `printed` holds no absolute path of the test (its scratch directory,
/// the repository's `HOME`, the data directory) and no scratch file's name
/// (RD-8, "Rules and edge cases").
#[track_caller]
fn assert_private(printed: &str, repo: &LockRepo, what: &str) {
    for path in [
        repo.scratch.path(),
        repo.home.as_path(),
        data_dir(&repo.home).as_path(),
    ] {
        let path = path.to_str().expect("a UTF-8 path");
        assert!(!printed.contains(path), "{what} names {path}: {printed}");
    }
    for word in printed.split(|c: char| c.is_whitespace() || "/'\"`:,".contains(c)) {
        assert!(
            !is_scratch_name(word, "base") && !is_scratch_name(word, "proposed"),
            "{what} names a scratch file ({word}): {printed}"
        );
    }
}

/// A scratch file's name: `specengine-<pid>-<seq>-<suffix>`.
fn is_scratch_name(name: &str, suffix: &str) -> bool {
    let Some(rest) = name
        .strip_prefix("specengine-")
        .and_then(|rest| rest.strip_suffix(&format!("-{suffix}")))
    else {
        return false;
    };
    let parts: Vec<&str> = rest.split('-').collect();
    parts.len() == 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Whether a call is through "The git door".
fn through_the_door(call: &[String], data: &Path) -> bool {
    let args: Vec<&str> = call.iter().map(String::as_str).collect();
    let scratch = |path: &str, suffix: &str| {
        let path = Path::new(path);
        path.starts_with(data)
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| is_scratch_name(name, suffix))
    };
    match args.as_slice() {
        ["rev-parse", rest @ ..] => !rest.is_empty(),
        ["hash-object", rest @ ..] => !rest
            .iter()
            .any(|arg| *arg == "-w" || arg.starts_with("--write")),
        ["cat-file", "--end-of-options", "blob", object] => object.contains(':'),
        ["cat-file", batch] => batch.starts_with("--batch"),
        ["diff", rest @ ..] => {
            // "The git door"'s flags, each once; beyond them only what keeps
            // a driver or an attribute out (`--no-textconv`) or reads every
            // byte as text (`--text`).
            let Some(dash) = rest.iter().position(|arg| *arg == "--") else {
                return false;
            };
            let (flags, paths) = (&rest[..dash], &rest[dash + 1..]);
            let required = [
                "--no-index",
                "--no-color",
                "--no-ext-diff",
                "--diff-algorithm=myers",
                "-U3",
            ];
            let extra = ["--no-textconv", "--text", "-a"];
            paths.len() == 2
                && scratch(paths[0], "base")
                && scratch(paths[1], "proposed")
                && required.iter().all(|flag| flags.contains(flag))
                && flags
                    .iter()
                    .all(|flag| required.contains(flag) || extra.contains(flag))
        }
        _ => false,
    }
}

/// The files under the data directory named as a diff's scratch files.
fn scratch_files(home: &Path) -> Vec<String> {
    snapshot(&data_dir(home))
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some() && {
                let name = path.rsplit('/').next().unwrap_or(path);
                is_scratch_name(name, "base") || is_scratch_name(name, "proposed")
            }
        })
        .map(|(path, _)| path)
        .collect()
}

// ---------------------------------------------------------- what is left

/// What a read must leave as it was (RD-6): every byte under the root,
/// `.git` too (its index, objects, refs), the index's mtime, `git status`,
/// `spec.lock` and the slug's queue.
#[derive(Debug, PartialEq, Eq)]
struct Untouched {
    files: BTreeMap<String, Option<Vec<u8>>>,
    status: Vec<u8>,
    index_mtime: Option<(i64, i64)>,
    refs: String,
    objects: Vec<String>,
    lock: Option<Vec<u8>>,
    queue: Vec<String>,
}

impl Untouched {
    fn take(repo: &LockRepo) -> Self {
        let status = status(repo);
        let git_dir = repo.root.join(".git");
        Self {
            files: snapshot(&repo.root),
            status,
            index_mtime: fs::metadata(git_dir.join("index"))
                .ok()
                .map(|meta| (meta.mtime(), meta.mtime_nsec())),
            refs: repo.git_text(&["for-each-ref", "--format=%(refname) %(objectname)"]),
            objects: snapshot(&git_dir.join("objects")).into_keys().collect(),
            lock: repo.lock(),
            queue: queue_state(repo),
        }
    }

    #[track_caller]
    fn still(&self, repo: &LockRepo, context: &str) {
        let now = Self::take(repo);
        assert_eq!(
            now.index_mtime, self.index_mtime,
            "{context}: .git/index's mtime"
        );
        assert_eq!(now.refs, self.refs, "{context}: the refs");
        assert_eq!(now.objects, self.objects, "{context}: the objects");
        assert_eq!(now.lock, self.lock, "{context}: spec.lock");
        assert_eq!(now.queue, self.queue, "{context}: the queue");
        assert_eq!(now.status, self.status, "{context}: git status");
        let moved: Vec<&String> = self
            .files
            .keys()
            .chain(now.files.keys())
            .filter(|path| self.files.get(*path) != now.files.get(*path))
            .collect();
        assert!(
            moved.is_empty(),
            "{context} wrote under the root: {moved:?}"
        );
    }
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

/// The slug's queue dump and its events' count (no database: nothing).
fn queue_state(repo: &LockRepo) -> Vec<String> {
    let db = data_dir(&repo.home).join(format!("{SLUG}.db"));
    if !db.exists() {
        return Vec::new();
    }
    let queue = SqliteQueue::open(&db, SLUG).expect("the queue opens");
    let mut state: Vec<String> = queue
        .dump()
        .expect("the dump")
        .lines()
        .map(str::to_owned)
        .collect();
    state.push(format!(
        "events {}",
        queue.events().expect("the events").len()
    ));
    state
}

// ------------------------------------------------------------------ RD-1

/// RD-1 (docs/features/lock-diff.md "Acceptance criteria"; "Data" its
/// example): after `spec lock accept LAW-BOUND`, LAW-BOUND's sentence and
/// `CAP` edited, uncommitted: exactly one `conflict` row, CAP's, with both
/// sides, whose hunks are "Data"'s (so its `+` lines are exactly the
/// edited lines, uncommitted bytes); every other kept row `spec_ahead`
/// with the same spec side and no code side; no note; the text "Data"'s.
/// M1 (old texts against `HEAD`, not the working tree): the hunks empty.
#[test]
fn rd1_conflict_has_both_sides_whose_plus_lines_are_the_uncommitted_edits() {
    let repo = accepted("ld-rd1");
    conflict_edits(&repo);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let nodes = run.doc["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 1, "{what}");
    assert_eq!(
        (&nodes[0]["id"], &nodes[0]["path"], &nodes[0]["line"]),
        (
            &Value::from(LAW),
            &Value::from(COUNTER_MD),
            &Value::from(13)
        ),
        "{what}"
    );
    let rows = run.rows();
    assert_eq!(rows.len(), 4, "LAW-BOUND's four entries drift: {what}");
    let conflicts: Vec<&Value> = rows
        .iter()
        .filter(|row| row["row"]["state"] == "conflict")
        .collect();
    assert_eq!(conflicts.len(), 1, "exactly one conflict: {what}");
    let cap = conflicts[0];
    assert_eq!(cap["row"]["symbol"], "crates/tally::counter::CAP", "{what}");
    assert_eq!(cap["spec"]["diff"], LAW_HUNKS, "{what}");
    assert_eq!(cap["code"]["diff"], CAP_HUNKS, "{what}");
    assert_eq!(cap["code"]["file"], COUNTER, "{what}");
    for side in [&cap["spec"]["commit"], &cap["code"]["blob"]] {
        let oid = text(side);
        assert!(
            oid.len() == 40 && oid.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "a full OID: {oid:?}"
        );
    }
    for row in rows.iter().filter(|row| row["row"]["state"] != "conflict") {
        assert_eq!(row["row"]["state"], "spec_ahead", "{what}");
        assert_eq!(row["spec"]["diff"], LAW_HUNKS, "{what}");
        assert!(row["code"].is_null(), "{what}");
    }
    let mut plus: Vec<String> = rows
        .iter()
        .flat_map(|row| {
            [&row["spec"]["diff"], &row["code"]["diff"]]
                .into_iter()
                .filter_map(Value::as_str)
                .flat_map(|diff| hunk_lines(diff, '+'))
                .collect::<Vec<_>>()
        })
        .collect();
    plus.sort();
    plus.dedup();
    assert_eq!(
        plus,
        [
            NEW_SENTENCE,
            "pub const CAP: u32 = 12; // @configures LAW-BOUND@2",
        ],
        "the `+` lines are the uncommitted edits: {what}"
    );
    assert!(run.notes().is_empty(), "{what}");
    assert_eq!(run.read.text.stderr, "", "{what}");
    assert!(
        run.read
            .text
            .stdout
            .ends_with("\nlock diff LAW-BOUND: rows 4, spec sides 4, code sides 1\n"),
        "{what}"
    );
}

// ------------------------------------------------------------------ RD-2

/// RD-2: only `CAP` edited: one `code_ahead` row with a code side ("Data"'s
/// hunks), `spec` null; only the sentence edited: four `spec_ahead` rows,
/// each its spec side, `code` null; an `ok` row and an `unlocked` binding
/// (a new marker) are never listed (verify lists both). M2 (a spec side on
/// `code_ahead`) red.
#[test]
fn rd2_code_ahead_has_a_code_side_only_and_spec_ahead_a_spec_side_only() {
    let repo = accepted("ld-rd2");
    let before = snapshot(&repo.root);
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
    repo.append(
        COUNTER,
        "\n// @implements LAW-BOUND@2\npub fn ceiling() -> u32 {\n    CAP\n}\n",
    );
    let run = diffed(&repo, LAW);
    let what = run.show();
    let states: Vec<&str> = run
        .oracle
        .all
        .iter()
        .map(|row| text(&row["state"]))
        .collect();
    assert!(
        states.contains(&"ok") && states.contains(&"unlocked"),
        "premise: verify --node lists an ok row and an unlocked one: {states:?}"
    );
    assert!(
        run.oracle
            .all
            .iter()
            .any(|row| row["state"] == "unlocked"
                && row["symbol"] == "crates/tally::counter::ceiling"),
        "premise: the new marker's binding is unlocked"
    );
    let rows = run.rows();
    assert_eq!(rows.len(), 1, "CAP's row alone: {what}");
    assert_eq!(
        (&rows[0]["row"]["state"], &rows[0]["row"]["category"]),
        (&Value::from("code_ahead"), &Value::from("code_changed")),
        "{what}"
    );
    assert!(
        rows[0]["spec"].is_null(),
        "no spec side on code_ahead: {what}"
    );
    assert_eq!(rows[0]["code"]["diff"], CAP_HUNKS, "{what}");

    restore(&repo, &before);
    repo.replace(COUNTER_MD, OLD_SENTENCE, NEW_SENTENCE);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 4, "{what}");
    for row in &rows {
        assert_eq!(
            (&row["row"]["state"], &row["row"]["category"]),
            (&Value::from("spec_ahead"), &Value::from("same_revision")),
            "{what}"
        );
        assert_eq!(row["spec"]["diff"], LAW_HUNKS, "{what}");
        assert!(row["code"].is_null(), "no code side on spec_ahead: {what}");
    }
}

/// RD-2, "Sides" per state (each scenario over the accepted repository,
/// restored after): `code_ahead levels_changed` (CAP's marker `[sig]`) a
/// code side alone; `code_ahead raised` (LAW-BOUND at rev 3, CAP's marker
/// `@3`) a spec side, with a code side only when its levels moved (CAP's
/// value too) — the other rows `spec_ahead marker_behind` with a spec
/// side (R moved); `spec_ahead marker_behind` with (R, H) unmoved (CAP's
/// marker lowered to `@1`): no side at all, the row still listed;
/// `spec_ahead spec_changed` (rev 3, CAP's marker without a revision);
/// `broken_binding file_gone` (the test file deleted) and `symbol_gone`
/// (its function deleted): a code side of `-` lines only; `marker_gone`
/// (CAP's marker deleted): a code side. Every row's sides are checked
/// against "Sides" by [`Diffed::assert_contract`]. M2 red.
#[test]
fn rd2_the_sides_follow_the_table_per_state() {
    let repo = accepted("ld-rd2-sides");
    let before = snapshot(&repo.root);
    let cap_line = "pub const CAP: u32 = 10; // @configures LAW-BOUND@2";
    type Scenario<'a> = (
        &'a str,
        &'a dyn Fn(&LockRepo),
        &'a [(&'a str, &'a str, &'a str)],
    );
    let raise = |repo: &LockRepo| repo.replace(COUNTER_MD, "#LAW-BOUND rev=2", "#LAW-BOUND rev=3");
    let scenarios: [Scenario<'_>; 8] = [
        (
            "levels_changed",
            &|repo: &LockRepo| {
                repo.replace(
                    COUNTER,
                    cap_line,
                    "pub const CAP: u32 = 10; // @configures LAW-BOUND@2 [sig]",
                )
            },
            &[("crates/tally::counter::CAP", "code_ahead", "levels_changed")],
        ),
        (
            "raised, the code moved",
            &|repo: &LockRepo| {
                raise(repo);
                repo.replace(COUNTER_MD, OLD_SENTENCE, NEW_SENTENCE);
                repo.replace(
                    COUNTER,
                    cap_line,
                    "pub const CAP: u32 = 12; // @configures LAW-BOUND@3",
                );
            },
            &[
                ("crates/tally::counter::CAP", "code_ahead", "raised"),
                (
                    "crates/tally::counter::<Tally as fmt::Display>::fmt",
                    "spec_ahead",
                    "marker_behind",
                ),
                (
                    "crates/tally::test:bounds::stops_at_cap",
                    "spec_ahead",
                    "marker_behind",
                ),
                ("root.cap", "spec_ahead", "marker_behind"),
            ],
        ),
        (
            "raised, the code as accepted",
            &|repo: &LockRepo| {
                raise(repo);
                repo.replace(
                    COUNTER,
                    cap_line,
                    "pub const CAP: u32 = 10; // @configures LAW-BOUND@3",
                );
            },
            &[
                ("crates/tally::counter::CAP", "code_ahead", "raised"),
                (
                    "crates/tally::counter::<Tally as fmt::Display>::fmt",
                    "spec_ahead",
                    "marker_behind",
                ),
                (
                    "crates/tally::test:bounds::stops_at_cap",
                    "spec_ahead",
                    "marker_behind",
                ),
                ("root.cap", "spec_ahead", "marker_behind"),
            ],
        ),
        (
            "marker_behind, the node unmoved",
            &|repo: &LockRepo| {
                repo.replace(
                    COUNTER,
                    cap_line,
                    "pub const CAP: u32 = 10; // @configures LAW-BOUND@1",
                )
            },
            &[("crates/tally::counter::CAP", "spec_ahead", "marker_behind")],
        ),
        (
            "spec_changed",
            &|repo: &LockRepo| {
                raise(repo);
                repo.replace(
                    COUNTER,
                    cap_line,
                    "pub const CAP: u32 = 10; // @configures LAW-BOUND",
                );
            },
            &[
                ("crates/tally::counter::CAP", "spec_ahead", "spec_changed"),
                (
                    "crates/tally::counter::<Tally as fmt::Display>::fmt",
                    "spec_ahead",
                    "marker_behind",
                ),
                (
                    "crates/tally::test:bounds::stops_at_cap",
                    "spec_ahead",
                    "marker_behind",
                ),
                ("root.cap", "spec_ahead", "marker_behind"),
            ],
        ),
        (
            "file_gone",
            &|repo: &LockRepo| {
                fs::remove_file(repo.root.join(BOUNDS)).expect("the test file deleted")
            },
            &[(
                "crates/tally::test:bounds::stops_at_cap",
                "broken_binding",
                "file_gone",
            )],
        ),
        (
            "symbol_gone",
            &|repo: &LockRepo| {
                repo.replace(
                    BOUNDS,
                    "// @verifies LAW-BOUND@2 [body] mutation: drop the cap\n#[test]\nfn stops_at_cap() {\n    let mut tally = Tally::default();\n    for _ in 0..11 {\n        tally.step();\n    }\n    assert_eq!(tally.value(), 10);\n}\n\n",
                    "",
                );
            },
            &[(
                "crates/tally::test:bounds::stops_at_cap",
                "broken_binding",
                "symbol_gone",
            )],
        ),
        (
            "marker_gone",
            &|repo: &LockRepo| repo.replace(COUNTER, " // @configures LAW-BOUND@2", ""),
            &[(
                "crates/tally::counter::CAP",
                "broken_binding",
                "marker_gone",
            )],
        ),
    ];
    for (name, change, want) in scenarios {
        change(&repo);
        let run = diffed(&repo, LAW);
        let what = format!("{name}: {}", run.show());
        let mut got: Vec<(String, String, String)> = run
            .rows()
            .iter()
            .map(|row| {
                (
                    text(&row["row"]["symbol"]).to_owned(),
                    text(&row["row"]["state"]).to_owned(),
                    text(&row["row"]["category"]).to_owned(),
                )
            })
            .collect();
        let mut want: Vec<(String, String, String)> = want
            .iter()
            .map(|(symbol, state, category)| {
                (
                    (*symbol).to_owned(),
                    (*state).to_owned(),
                    (*category).to_owned(),
                )
            })
            .collect();
        got.sort();
        want.sort();
        assert_eq!(got, want, "{what}");
        match name {
            "levels_changed" => assert_eq!(
                run.row("crates/tally::counter::CAP")["code"]["diff"],
                "@@ -1 +1 @@\n-pub const CAP: u32 = 10; // @configures LAW-BOUND@2\n+pub const CAP: u32 = 10; // @configures LAW-BOUND@2 [sig]\n",
                "{what}"
            ),
            "raised, the code moved" => {
                let cap = run.row("crates/tally::counter::CAP");
                assert!(!cap["code"].is_null(), "levels moved: a code side: {what}");
                assert_eq!(
                    hunk_lines(text(&cap["spec"]["diff"]), '+'),
                    ["## Bound {#LAW-BOUND rev=3}", NEW_SENTENCE],
                    "{what}"
                );
            }
            "raised, the code as accepted" => {
                let cap = run.row("crates/tally::counter::CAP");
                assert_eq!(
                    cap["row"]["levels"]["changed"],
                    serde_json::json!([]),
                    "premise: no level moved: {what}"
                );
                assert!(
                    cap["code"].is_null(),
                    "no level moved: no code side: {what}"
                );
                assert_eq!(
                    hunk_lines(text(&cap["spec"]["diff"]), '+'),
                    ["## Bound {#LAW-BOUND rev=3}"],
                    "{what}"
                );
            }
            "marker_behind, the node unmoved" => {
                let cap = run.row("crates/tally::counter::CAP");
                assert!(
                    cap["spec"].is_null() && cap["code"].is_null(),
                    "listed with no side: {what}"
                );
            }
            "file_gone" | "symbol_gone" => {
                let test = run.row("crates/tally::test:bounds::stops_at_cap");
                let minus = hunk_lines(text(&test["code"]["diff"]), '-');
                assert!(
                    minus.contains(&"fn stops_at_cap() {".to_owned())
                        && minus.contains(&"    assert_eq!(tally.value(), 10);".to_owned()),
                    "the old item, `-` lines: {what}"
                );
                assert!(test["spec"].is_null(), "{what}");
            }
            "marker_gone" => assert_eq!(
                run.row("crates/tally::counter::CAP")["code"]["diff"],
                "@@ -1 +1 @@\n-pub const CAP: u32 = 10; // @configures LAW-BOUND@2\n+pub const CAP: u32 = 10;\n",
                "{what}"
            ),
            _ => {}
        }
        restore(&repo, &before);
    }
}

// ------------------------------------------------------------------ RD-3

/// RD-3: `CAP` and `LABEL` (three lines below it) edited: CAP's hunks are
/// "Data"'s, no `LABEL` line among them (no context from outside the item);
/// `LABEL` binds nothing of LAW-BOUND, so no other code side. M3
/// (whole-file hunks for every item) red.
#[test]
fn rd3_an_unrelated_edit_in_the_same_file_is_not_in_the_hunks() {
    let repo = accepted("ld-rd3-label");
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
    repo.replace(
        COUNTER,
        "pub const LABEL: &str = \"GOAL-01 // @implements LAW-STEP\";",
        "pub const LABEL: &str = \"GOAL-01 // @implements LAW-STEP!\";",
    );
    let run = diffed(&repo, LAW);
    let what = run.show();
    let cap = run.row("crates/tally::counter::CAP");
    assert_eq!(cap["code"]["diff"], CAP_HUNKS, "{what}");
    assert!(
        !text(&cap["code"]["diff"]).contains("LABEL"),
        "no LABEL line: {what}"
    );
}

/// RD-3, a moved row (`anchor.state` `moved`), over a non-trivial function
/// bound to LAW-BOUND and accepted in `counter.rs` (RD-3's own wording
/// moves `CAP`, whose two-byte body is under `TRIVIAL_BODY_BYTES` and is
/// never followed): renamed and changed in the same file (a literal; the
/// skeleton pairs it), then, restored, moved to `lib.rs` with its return
/// type changed (the body pairs it). Each time `code.file` and `code.blob`
/// are the entry's (`anchor.from.file`), the old item located by the
/// entry's key in the blob and the new by the row's in the working tree:
/// the hunks are git's of the two items' lines alone, nothing else of
/// either file. M3 (whole-file hunks for every item) red.
#[test]
fn rd3_a_moved_row_diffs_the_entrys_item_against_the_rows() {
    let repo = LockRepo::code_a("ld-rd3-moved");
    let counter = repo.read(COUNTER);
    repo.append(COUNTER, CLAMP);
    repo.commit_all("clamp_all");
    repo.accept_ok(LAW);
    let accepted = snapshot(&repo.root);
    let lock = repo.lock_text();
    let entry = blocks(&lock)
        .into_iter()
        .find(|block| value(block, "symbol") == Some("crates/tally::counter::clamp_all"))
        .expect("premise: clamp_all's entry");
    let body_bytes: u32 = value(&entry, "body_bytes")
        .expect("body_bytes")
        .parse()
        .expect("a number");
    assert!(
        body_bytes >= TRIVIAL_BODY_BYTES,
        "premise: clamp_all is not trivial ({body_bytes} bytes)"
    );
    let old_item = CLAMP
        .strip_prefix("\n// @implements LAW-BOUND@2\n")
        .expect("the item after its marker");

    let renamed = CLAMP
        .replacen("pub fn clamp_all(", "pub fn clamp_each(", 1)
        .replacen("clamped += 1;", "clamped += 2;", 1);
    repo.replace(COUNTER, CLAMP, &renamed);
    let renamed_item = renamed
        .strip_prefix("\n// @implements LAW-BOUND@2\n")
        .expect("the item after its marker")
        .to_owned();
    let moved = CLAMP.replacen(") -> u32 {", ") -> usize {", 1);
    let moved_item = moved
        .strip_prefix("\n// @implements LAW-BOUND@2\n")
        .expect("the item after its marker")
        .to_owned();
    for (case, symbol, file, new_item) in [
        (
            "renamed and changed in its file",
            "crates/tally::counter::clamp_each",
            COUNTER,
            renamed_item,
        ),
        (
            "moved to lib.rs, its signature changed",
            "crates/tally::clamp_all",
            LIB,
            moved_item,
        ),
    ] {
        if file == LIB {
            restore(&repo, &accepted);
            repo.write(COUNTER, &counter);
            repo.replace(
                LIB,
                "pub mod counter;\n",
                &format!("pub mod counter;\n{moved}"),
            );
        }
        let run = diffed(&repo, LAW);
        let what = format!("{case}: {}", run.show());
        let row = run.row(symbol);
        assert_eq!(
            row["row"]["anchor"]["state"], "moved",
            "premise: followed: {what}"
        );
        assert_eq!(row["row"]["anchor"]["from"]["file"], COUNTER, "{what}");
        assert_eq!(row["row"]["file"], file, "{what}");
        assert_eq!(row["code"]["file"], COUNTER, "the entry's file: {what}");
        assert_eq!(
            row["code"]["blob"],
            value(&entry, "file_blob").expect("file_blob"),
            "{what}"
        );
        assert_eq!(
            row["code"]["diff"],
            git_hunks(&repo, old_item.as_bytes(), new_item.as_bytes()),
            "the two items' lines alone: {what}"
        );
        assert!(
            run.read.text.stdout.contains("\n  moved from "),
            "verify's moved line: {what}"
        );
    }
}

/// RD-3 and "Code texts": whole files with N6. A drifted `module` row
/// (PIECE-COUNTER accepted, `lib.rs` edited): the code side is `lib.rs`'s
/// blob against `lib.rs` now, git's hunks of the two whole files; a `ron`
/// row (`data/limits.ron`'s cap edited): the whole file; an item not
/// located in its blob (CAP's `file_blob` hand-set to `lib.rs`'s blob,
/// `CAP` changed): that blob against `counter.rs` now, N6 naming the
/// symbol and the blob. Each N6 once, in output order.
#[test]
fn rd3_module_ron_and_an_item_not_in_its_blob_diff_whole_files() {
    let repo = accepted("ld-rd3-whole");
    repo.accept_ok("PIECE-COUNTER");
    let before = snapshot(&repo.root);
    let lib_then = repo.read(LIB);
    repo.replace(
        LIB,
        "pub use counter::Tally;\n",
        "pub use counter::Tally;\n\n/// A spare count.\npub fn spare() -> u32 {\n    1\n}\n",
    );
    let run = diffed(&repo, "PIECE-COUNTER");
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 1, "lib.rs's module row alone: {what}");
    let module = &rows[0];
    assert_eq!(
        (
            &module["row"]["kind"],
            &module["row"]["file"],
            &module["row"]["state"]
        ),
        (
            &Value::from("module"),
            &Value::from(LIB),
            &Value::from("code_ahead")
        ),
        "{what}"
    );
    assert_eq!(
        module["code"]["diff"],
        git_hunks(&repo, lib_then.as_bytes(), repo.read(LIB).as_bytes()),
        "{what}"
    );
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: PIECE-COUNTER | implements | {LIB} | module crates/tally: the code diff is the whole file: a module binding"
        )],
        "{what}"
    );
    restore(&repo, &before);

    let ron_then = repo.read(LIMITS);
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    let run = diffed(&repo, LAW);
    let what = run.show();
    let ron = run.row("root.cap");
    assert_eq!(
        ron["code"]["diff"],
        git_hunks(&repo, ron_then.as_bytes(), repo.read(LIMITS).as_bytes()),
        "{what}"
    );
    assert!(text(&ron["code"]["diff"]).starts_with("@@ -1,"), "{what}");
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: LAW-BOUND | configures | {LIMITS} | ron root.cap: the code diff is the whole file: a ron binding"
        )],
        "{what}"
    );
    restore(&repo, &before);

    let lib_blob = repo.blob_at("HEAD", LIB);
    edit_entry(&repo, CAP_ENTRY, |block| {
        set_string(block, "file_blob", &lib_blob)
    });
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let cap = run.row("crates/tally::counter::CAP");
    assert_eq!(cap["code"]["blob"], lib_blob.as_str(), "{what}");
    assert_eq!(
        cap["code"]["diff"],
        git_hunks(&repo, lib_then.as_bytes(), repo.read(COUNTER).as_bytes()),
        "{what}"
    );
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: {CAP_KEY}: the code diff is the whole file: crates/tally::counter::CAP is not found in blob {lib_blob}"
        )],
        "{what}"
    );
}

// ------------------------------------------------------------------ RD-4

/// RD-4: a lock whose `commit` names no object (the golden lock committed
/// into a fresh copy), LAW-BOUND's sentence and `CAP` edited: every spec
/// side's `diff` null with its N1 (in output order), CAP's code side
/// "Data"'s (its blob is an object here), exit 0, the summary counting
/// the four sides not shown.
#[test]
fn rd4_a_commit_naming_no_object_leaves_the_spec_side_null_with_n1() {
    let repo = golden("ld-rd4-golden");
    conflict_edits(&repo);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 4, "{what}");
    let mut want = Vec::new();
    for row in &rows {
        assert!(row["spec"]["diff"].is_null(), "{what}");
        want.push(format!(
            "lock diff: {}: no spec diff: {COUNTER_MD} cannot be read at {}",
            key_text(&row["row"]),
            text(&row["spec"]["commit"])
        ));
    }
    assert_eq!(
        run.notes(),
        want,
        "one N1 per side, in output order: {what}"
    );
    assert_eq!(
        run.row("crates/tally::counter::CAP")["code"]["diff"],
        CAP_HUNKS,
        "{what}"
    );
    assert!(
        run.read
            .text
            .stdout
            .ends_with(": rows 4, spec sides 4, code sides 1, not shown 4\n"),
        "{what}"
    );
}

/// RD-4, "Known limits" (decayed objects): the entries' `commit` amended
/// away (the commit squashed into its parent by `git reset --soft` and
/// `commit --amend`), the reflog expired and `gc --prune=now`: exit 0,
/// every spec side's `diff` null with N1, the code sides intact. M4 (exit
/// 2 on a gone commit) red.
#[test]
fn rd4_an_amended_and_collected_commit_answers_exit_0() {
    let repo = LockRepo::code_a("ld-rd4-amended");
    let first = repo.head();
    repo.write("tools/notes.txt", "A note committed before the accept.\n");
    repo.commit_all("a note");
    let noted = repo.head();
    repo.accept_ok(LAW);
    assert!(
        repo.lock_text().contains(&format!("commit = \"{noted}\"")),
        "premise: the entries name the note's commit"
    );
    repo.git(&["reset", "-q", "--soft", &first]);
    repo.git(&[
        "commit",
        "-q",
        "--amend",
        "-m",
        "the fixture, the note and the lock",
    ]);
    repo.git(&["reflog", "expire", "--expire=now", "--all"]);
    repo.git(&["gc", "-q", "--prune=now"]);
    let probe = repo.git.git_output(
        &repo.root,
        &["cat-file", "-e", &format!("{noted}^{{commit}}")],
        &[],
    );
    assert!(!probe.status.success(), "premise: {noted} is gone");
    conflict_edits(&repo);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 4, "{what}");
    let want: Vec<String> = rows
        .iter()
        .map(|row| {
            assert!(row["spec"]["diff"].is_null(), "{what}");
            assert_eq!(row["spec"]["commit"], noted.as_str(), "{what}");
            format!(
                "lock diff: {}: no spec diff: {COUNTER_MD} cannot be read at {noted}",
                key_text(&row["row"])
            )
        })
        .collect();
    assert_eq!(run.notes(), want, "{what}");
    assert_eq!(
        run.row("crates/tally::counter::CAP")["code"]["diff"],
        CAP_HUNKS,
        "{what}"
    );
}

/// RD-4 and "Notes" N4, N5: CAP's `file_blob` hand-set to an OID naming no
/// object, then to a blob written here whose bytes are not UTF-8, `CAP`
/// changed: `code.diff` null with N4, then N5; the row and the exit stand.
#[test]
fn rd4_a_blob_missing_or_not_utf8_leaves_the_code_side_null() {
    let repo = accepted("ld-rd4-blob");
    let lock = repo.lock_text();
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
    let missing = fake_oid('e');
    edit_entry(&repo, CAP_ENTRY, |block| {
        set_string(block, "file_blob", &missing)
    });
    let run = diffed(&repo, LAW);
    let what = run.show();
    let cap = run.row("crates/tally::counter::CAP");
    assert_eq!(cap["row"]["state"], "code_ahead", "{what}");
    assert!(cap["code"]["diff"].is_null(), "{what}");
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: {CAP_KEY}: no code diff: blob {missing} of {COUNTER} cannot be read"
        )],
        "{what}"
    );
    assert!(
        run.read
            .text
            .stdout
            .contains(&format!("\n  code {COUNTER} blob {missing}: not shown\n"))
    );

    let foreign = String::from_utf8(repo.git.git_stdin(
        &repo.root,
        &["hash-object", "-w", "--stdin"],
        b"pub const CAP: u32 = \xff;\n",
    ))
    .expect("an OID")
    .trim()
    .to_owned();
    repo.write("spec.lock", &lock);
    edit_entry(&repo, CAP_ENTRY, |block| {
        set_string(block, "file_blob", &foreign)
    });
    let run = diffed(&repo, LAW);
    let what = run.show();
    assert!(
        run.row("crates/tally::counter::CAP")["code"]["diff"].is_null(),
        "{what}"
    );
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: {CAP_KEY}: no code diff: blob {foreign} of {COUNTER} is not UTF-8"
        )],
        "{what}"
    );
}

/// "Notes" N2, N3: the entries' `commit` hand-set to commits of a side
/// branch whose `counter.md` holds no LAW-BOUND (the section renamed) or
/// is not UTF-8, LAW-BOUND's sentence edited: each spec side's `diff`
/// null, its note naming the path and the commit.
#[test]
fn rd4_a_node_not_at_the_commit_or_bytes_not_utf8_leave_the_spec_side_null() {
    let repo = accepted("ld-rd4-spec");
    let lock = repo.lock_text();
    repo.git(&["checkout", "-q", "-b", "side"]);
    repo.replace(COUNTER_MD, "#LAW-BOUND rev=2", "#LAW-LIMIT rev=2");
    repo.commit_all("renamed");
    let renamed = repo.head();
    let mut broken = repo.read(COUNTER_MD).into_bytes();
    broken.extend_from_slice(b"\xff\n");
    repo.write(COUNTER_MD, &broken);
    repo.commit_all("not UTF-8");
    let not_utf8 = repo.head();
    repo.git(&["checkout", "-q", "main"]);
    let commits: Vec<String> = blocks(&lock)
        .iter()
        .filter_map(|block| value(block, "commit").map(str::to_owned))
        .collect();
    let accepted_at = commits.first().expect("a commit").clone();
    assert!(
        commits.iter().all(|commit| *commit == accepted_at),
        "premise: one accept"
    );
    let mut text = lock.replace(
        &format!("commit = \"{accepted_at}\""),
        &format!("commit = \"{renamed}\""),
    );
    text = edit_block(&text, CAP_ENTRY, |block| {
        set_string(block, "commit", &not_utf8)
    });
    repo.write("spec.lock", text);
    repo.replace(COUNTER_MD, OLD_SENTENCE, NEW_SENTENCE);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 4, "{what}");
    let want: Vec<String> = rows
        .iter()
        .map(|row| {
            assert!(row["spec"]["diff"].is_null(), "{what}");
            let key = key_text(&row["row"]);
            if row["row"]["symbol"] == "crates/tally::counter::CAP" {
                format!("lock diff: {key}: no spec diff: {COUNTER_MD} at {not_utf8} is not UTF-8")
            } else {
                format!(
                    "lock diff: {key}: no spec diff: {COUNTER_MD} at {renamed} holds no LAW-BOUND"
                )
            }
        })
        .collect();
    assert_eq!(run.notes(), want, "{what}");
}

/// "Notes" N7: every `git diff --no-index` failing (a shim on `PATH` whose
/// stderr names every argument, the scratch files' absolute paths among
/// them): both sides of CAP's conflict `diff` null, each with N7 carrying
/// git's failure on one line, never an absolute path or a scratch file's
/// name (RD-8: no absolute path or scratch name in the output); the exit
/// stays 0.
#[test]
fn rd4_a_failing_diff_leaves_the_side_null_with_n7() {
    let repo = accepted("ld-rd4-n7");
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
    repo.replace(COUNTER_MD, OLD_SENTENCE, NEW_SENTENCE);
    let shim = Shim::new(&repo, "refusing", true);
    let run = diffed_in(&repo, &repo.root, LAW, &shim.vars(&repo));
    let what = run.show();
    let cap = run.row("crates/tally::counter::CAP");
    assert!(
        cap["spec"]["diff"].is_null() && cap["code"]["diff"].is_null(),
        "{what}"
    );
    for side in ["spec", "code"] {
        // "Notes" N7 reads `git diff --no-index failed: <GitError, one
        // line>`; iteration 2 words a failure whose stderr names a scratch
        // file (this shim's) `git diff --no-index could not run`: no path
        // can leak.
        let n7 = format!("lock diff: {CAP_KEY}: no {side} diff: git diff --no-index could not run");
        let found: Vec<String> = run
            .notes()
            .into_iter()
            .filter(|note| note.starts_with(&format!("lock diff: {CAP_KEY}: no {side} diff: ")))
            .collect();
        assert_eq!(found, [n7], "one N7 for the {side} side: {what}");
        assert_private(&found[0], &repo, "N7");
    }
    assert!(
        shim.calls().iter().any(|call| call.args[0] == "diff"),
        "premise: the shim refused the diffs"
    );
}

/// RD-4: no `[code]` (the table removed from the working copy): every
/// holder's `rows` null, one N0 (`code-reads.md` "Degrade"'s words), exit
/// 0; the text "Text"'s `not judged` lines.
#[test]
fn rd4_no_code_table_leaves_rows_null_with_n0() {
    let repo = accepted("ld-rd4-no-code");
    conflict_edits(&repo);
    let vars = vars_of(&repo);
    let read = without_code(&repo.root, || lock_diff_in(&repo.root, LAW, &vars));
    read.answered();
    assert_eq!(
        read.json_run.stdout,
        format!(
            "{{\"ref\":\"LAW-BOUND\",\"reason\":null,\"notes\":[\"{NO_CODE}\"],\"nodes\":[{{\"id\":\"LAW-BOUND\",\"path\":\"{COUNTER_MD}\",\"line\":13,\"rows\":null}}]}}\n"
        ),
        "{}",
        read.show()
    );
    assert_eq!(
        read.text.stdout,
        format!("LAW-BOUND | {COUNTER_MD}:13\n  not judged\n\nlock diff LAW-BOUND: not judged\n")
    );
    assert_eq!(read.text.stderr, format!("note: {NO_CODE}\n"));
}

// ------------------------------------------------------------------ RD-5

/// RD-5 (docs/canon/spec-lock-diff.md "Never"): with the `conflict` row,
/// `[verify] mode = "enforce"` answers exit 0 with the very bytes of no
/// `[verify]` (no verdict, mode or `hint:` line), and so does `mode =
/// "nope"`; stdin is never a terminal here (answered). A REF naming
/// nothing: exit 1, `{ref, reason, notes: [], nodes: []}` with `show`'s
/// reason, `spec: <reason>` on stderr, the text's stdout empty; a
/// look-alike, a missing REF or an unknown flag: exit 2, stdout empty.
/// A REF with blanks echoes raw and is trimmed in the summary; a written
/// `@rev` gives `show`'s note first. M5 (verify's exit: 1 on a Fix row
/// under `enforce`) red.
#[test]
fn rd5_no_verdict_and_the_exits() {
    let repo = accepted("ld-rd5");
    conflict_edits(&repo);
    let plain = diffed(&repo, LAW);
    assert!(
        plain.rows().iter().any(|row| row["row"]["class"] == "fix"),
        "premise: a Fix row"
    );
    let config = repo.read("specengine.toml");
    for mode in ["enforce", "nope"] {
        repo.write(
            "specengine.toml",
            format!("{config}\n[verify]\nmode = \"{mode}\"\n"),
        );
        let read = lock_diff_in(&repo.root, LAW, &vars_of(&repo));
        assert_eq!(
            read.bytes(),
            plain.read.bytes(),
            "mode {mode}: the same bytes, exit 0\n{}",
            read.show()
        );
        for printed in [&read.text.stdout, &read.text.stderr] {
            assert!(
                !printed.contains("hint:")
                    && !printed.contains("[enforce]")
                    && !printed.contains("[nope]")
                    && !printed.contains("verdict"),
                "mode {mode}: no verdict, mode or hint\n{}",
                read.show()
            );
        }
    }
    repo.write("specengine.toml", &config);
    let vars = vars_of(&repo);

    let show = repo.spec_piped(&repo.root, &["show", "NOPE-9", "--json"]);
    show.code(1);
    let reason = text(&show.json()["reason"]).to_owned();
    let nothing = lock_diff_in(&repo.root, "NOPE-9", &vars);
    assert_eq!(nothing.code(), 1, "{}", nothing.show());
    assert_eq!(nothing.text.stdout, "", "{}", nothing.show());
    assert_eq!(nothing.text.stderr, format!("spec: {reason}\n"));
    assert_eq!(
        nothing.json_run.stdout,
        format!(
            "{{\"ref\":\"NOPE-9\",\"reason\":{},\"notes\":[],\"nodes\":[]}}\n",
            Value::from(reason.as_str())
        )
    );

    for args in [
        vec!["lock", "diff", LAW_LOOK_ALIKE],
        vec!["lock", "diff", LAW_LOOK_ALIKE, "--json"],
        vec!["lock", "diff"],
        vec!["lock", "diff", LAW, "--nope"],
    ] {
        let run = run_with(&repo.root, &args, &vars);
        assert_eq!(run.code, 2, "{args:?}\n{}", run.show());
        assert_eq!(run.stdout, "", "{args:?}: stdout empty\n{}", run.show());
    }

    let blanks = format!(" {LAW} ");
    let padded = diffed(&repo, &blanks);
    assert_eq!(
        padded.read.text.stdout, plain.read.text.stdout,
        "the summary names the REF trimmed"
    );
    assert_eq!(
        padded
            .raw
            .replacen(&format!("\"ref\":\"{blanks}\""), "\"ref\":\"LAW-BOUND\"", 1),
        plain.raw,
        "the JSON echoes the REF raw"
    );

    let show = repo.spec_piped(&repo.root, &["show", "LAW-BOUND@2"]);
    show.code(0);
    let show_notes: Vec<String> = show
        .stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        show_notes.len(),
        1,
        "premise: show's @rev note\n{}",
        show.show()
    );
    let written = diffed(&repo, "LAW-BOUND@2");
    assert_eq!(written.notes(), show_notes, "show's note, first and alone");

    // A `.md` REF whose bytes are not UTF-8: no holder to judge, exit 1
    // (the coordinator's ruling: `spec show` answers it, exit 0, with
    // `utf8` false; `verify --node` refuses it, exit 2).
    let odd = "docs/spec/odd.md";
    repo.write(
        odd,
        b"---\nclass: canon\nid: PIECE-ODD\n---\n\n# Odd \xff\n".as_slice(),
    );
    let read = lock_diff_in(&repo.root, odd, &vars);
    assert_eq!(read.code(), 1, "{}", read.show());
    assert_eq!(read.text.stdout, "", "{}", read.show());
    let document = common::one_json_document(&read.json_run.stdout).expect("the exit-1 document");
    let reason = text(&document["reason"]).to_owned();
    assert!(reason.contains(odd), "{}", read.show());
    assert_eq!(
        read.json_run.stdout,
        format!(
            "{{\"ref\":\"{odd}\",\"reason\":{},\"notes\":[],\"nodes\":[]}}\n",
            Value::from(reason.as_str())
        )
    );
    assert_eq!(read.text.stderr, format!("spec: {reason}\n"));
}

// ------------------------------------------------------------------ RD-6

/// RD-6, "The git door": a `git` shim on `PATH` logging argv records, for
/// text and `--json` reads of a conflict, a module and a RON row, only
/// `rev-parse`, `hash-object` without `-w`, `cat-file` (by `<commit>:<path>`
/// and the batch) and `diff --no-index --no-color --no-ext-diff
/// --diff-algorithm=myers -U3 -- <base> <proposed>` (`--no-textconv` or
/// `--text` beside them), every one under `GIT_OPTIONAL_LOCKS=0`, every
/// `cat-file` under `GIT_NO_LAZY_FETCH=1`, the two files in the data
/// directory, named
/// `specengine-<pid>-<seq>-base` and `-proposed`, gone after; every byte
/// under the root (`.git` too: index, objects, refs), the index's mtime,
/// `git status`, `spec.lock` and the queue unchanged; no scratch file left
/// in the data directory. M6 (the spec side as `git diff <commit> --
/// <path>`) red.
#[test]
fn rd6_git_goes_through_the_door_and_nothing_is_written() {
    let repo = accepted("ld-rd6");
    repo.accept_ok("PIECE-COUNTER");
    conflict_edits(&repo);
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(
        LIB,
        "pub use counter::Tally;\n",
        "pub use counter::Tally;\n\npub fn spare() -> u32 {\n    1\n}\n",
    );
    let shim = Shim::new(&repo, "door", false);
    let vars = shim.vars(&repo);
    // The indexes first, so the reads below find them.
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let before = Untouched::take(&repo);
    let data = data_dir(&repo.home);
    for reference in [LAW, "PIECE-COUNTER"] {
        shim.clear();
        let read = lock_diff_in(&repo.root, reference, &vars);
        read.answered();
        before.still(&repo, reference);
        let calls = shim.calls();
        let strays: Vec<&Call> = calls
            .iter()
            .filter(|call| !through_the_door(&call.args, &data))
            .collect();
        assert!(
            strays.is_empty(),
            "{reference}: outside the door: {strays:?}"
        );
        // `WorktreeGit::new`'s variables lookup (`rev-parse
        // --local-env-vars`) runs with no `GIT_*` variable by design,
        // before any repository is read; it lists names and opens nothing.
        let unlocked: Vec<&Call> = calls
            .iter()
            .filter(|call| call.locks != "0" && call.args != ["rev-parse", "--local-env-vars"])
            .collect();
        assert!(
            unlocked.is_empty(),
            "{reference}: every git under GIT_OPTIONAL_LOCKS=0: {unlocked:?}"
        );
        // "The git door": `cat-file` never fetches (`GIT_NO_LAZY_FETCH`),
        // the spec side's `blob <commit>:<path>` too.
        let fetching: Vec<&Call> = calls
            .iter()
            .filter(|call| call.args[0] == "cat-file" && call.lazy != "1")
            .collect();
        assert!(
            fetching.is_empty(),
            "{reference}: every cat-file under GIT_NO_LAZY_FETCH=1: {fetching:?}"
        );
        for first in ["diff", "cat-file"] {
            assert!(
                calls.iter().any(|call| call.args[0] == first),
                "{reference}: {first} ran: {calls:?}"
            );
        }
        if reference == LAW {
            assert!(
                calls.iter().any(|call| call.args.len() == 4
                    && call.args[..3] == ["cat-file", "--end-of-options", "blob"]
                    && call.args[3].ends_with(&format!(":{COUNTER_MD}"))),
                "{reference}: the spec side read by <commit>:<path>: {calls:?}"
            );
        }
        for call in calls.iter().filter(|call| call.args[0] == "diff") {
            for path in &call.args[call.args.len() - 2..] {
                assert!(!Path::new(path).exists(), "{path} removed");
            }
        }
        assert_eq!(
            scratch_files(&repo.home),
            Vec::<String>::new(),
            "{reference}"
        );
    }
}

// ------------------------------------------------------- a nested root

/// "Spec texts": a project in a subdirectory of its repository (`top/` the
/// worktree, `top/project` the root): the spec side is read at
/// `<commit>:project/docs/spec/counter.md` (the root's prefix in its
/// repository), its JSON `path` root-relative; both sides "Data"'s hunks.
#[test]
fn a_root_below_the_repository_top_reads_the_top_relative_path() {
    let scratch = Scratch::new("ld-nested");
    let git = Sandbox::new(scratch.path());
    let top = scratch.dir("top");
    copy_dir(&fixture("code-a"), &top.join("project"));
    fs::remove_file(top.join("project/expected.json")).expect("expected.json");
    git.init(&top);
    git.add_all(&top);
    git.commit(&top, "the fixture in a subdirectory");
    let home = scratch.home("h");
    let repo = LockRepo {
        root: top.join("project"),
        scratch,
        git,
        home,
    };
    repo.attempt_at(&repo.root, LAW, false, CLOCK, true)
        .committed("accept in a subdirectory");
    conflict_edits(&repo);
    let shim = Shim::new(&repo, "nested-shim", false);
    let run = diffed_in(&repo, &repo.root, LAW, &shim.vars(&repo));
    let what = run.show();
    let cap = run.row("crates/tally::counter::CAP");
    assert_eq!(cap["spec"]["path"], COUNTER_MD, "root-relative: {what}");
    assert_eq!(cap["spec"]["diff"], LAW_HUNKS, "{what}");
    assert_eq!(cap["code"]["diff"], CAP_HUNKS, "{what}");
    let commit = text(&cap["spec"]["commit"]).to_owned();
    let wanted = vec![
        "cat-file".to_owned(),
        "--end-of-options".to_owned(),
        "blob".to_owned(),
        format!("{commit}:project/{COUNTER_MD}"),
    ];
    assert!(
        shim.calls().iter().any(|call| call.args == wanted),
        "blob_at by the top-relative path: {:?}",
        shim.calls()
    );
}

// ------------------------------------------------------------------ RD-8

/// A byte copy of `from` at `to`, every directory's entries made in
/// reverse name order.
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

/// code-a at `<scratch>/<dir>`, LAW-BOUND and PIECE-COUNTER accepted,
/// "Data"'s edits, `lib.rs` and the RON cap edited: spec sides, item,
/// module and RON code sides, N6 notes.
fn drifting_at(label: &str, dir: &str) -> LockRepo {
    let repo = LockRepo::of(label, "code-a", dir, &[]);
    repo.accept_ok(LAW);
    repo.accept_ok("PIECE-COUNTER");
    conflict_edits(&repo);
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(
        LIB,
        "pub use counter::Tally;\n",
        "pub use counter::Tally;\n\npub fn spare() -> u32 {\n    1\n}\n",
    );
    repo
}

/// RD-8, "Rules and edge cases" (no absolute path, scratch name or time in
/// the output): text, JSON, stderr and exit are byte-identical repeated,
/// from another root with its own `HOME` (made second), from another cwd
/// below it, and from a copy made in reverse creation order.
#[test]
fn rd8_byte_identical_across_roots_cwd_and_reverse_creation() {
    let first = drifting_at("ld-det-a", "repo");
    let second = drifting_at("ld-det-b", "elsewhere/deeper");
    assert_eq!(second.head(), first.head(), "premise: the same commits");
    let reversed = first.scratch.join("reversed");
    copy_reversed(&first.root, &reversed);
    let reversed = fs::canonicalize(&reversed).expect("the reversed copy");
    let vars = vars_of(&first);
    let other = vars_of(&second);
    let deeper = second.root.join("crates").join("tally");
    for reference in [LAW, "PIECE-COUNTER", COUNTER_MD] {
        let reference_run = diffed(&first, reference);
        assert!(
            !reference_run.rows().is_empty(),
            "premise: drifted rows: {}",
            reference_run.show()
        );
        if reference == LAW {
            assert!(
                reference_run
                    .rows()
                    .iter()
                    .any(|row| row["spec"]["diff"].is_string() && row["code"]["diff"].is_string()),
                "premise: hunks on both sides: {}",
                reference_run.show()
            );
        }
        for (context, read) in [
            ("a repeat run", lock_diff_in(&first.root, reference, &vars)),
            (
                "another root",
                lock_diff_in(&second.root, reference, &other),
            ),
            ("another cwd", lock_diff_in(&deeper, reference, &other)),
            (
                "a reverse creation order",
                lock_diff_in(&reversed, reference, &vars),
            ),
        ] {
            assert_eq!(
                read.bytes(),
                reference_run.read.bytes(),
                "{reference}: {context}\n{}",
                read.show()
            );
        }
    }
}

/// RD-8: `fixtures/spec-c` (a Python service, no `[code]`, not a
/// repository, `HOME` alone set): `rows` null, its N0, exit 0, for a
/// section and its document.
#[test]
fn rd8_spec_c_has_no_code_layer() {
    let scratch = Scratch::new("ld-spec-c");
    let root = scratch.copy("spec-c", "spec-c");
    let home = scratch.home("h");
    let vars: Vec<(OsString, OsString)> = vec![("HOME".into(), home.into_os_string())];
    for reference in ["JRN-UNLOCK", "AREA-HIRE"] {
        let shown = run_with(&root, &["show", reference, "--json"], &vars);
        shown.code(0);
        let node = &shown.json()["nodes"][0];
        let (path, line) = (text(&node["path"]).to_owned(), node["line"].clone());
        let read = lock_diff_in(&root, reference, &vars);
        read.answered();
        assert_eq!(
            read.json_run.stdout,
            format!(
                "{{\"ref\":\"{reference}\",\"reason\":null,\"notes\":[\"{NO_CODE}\"],\"nodes\":[{{\"id\":\"{reference}\",\"path\":\"{path}\",\"line\":{line},\"rows\":null}}]}}\n"
            ),
            "{}",
            read.show()
        );
        assert_eq!(
            read.text.stdout,
            format!(
                "{reference} | {path}:{line}\n  not judged\n\nlock diff {reference}: not judged\n"
            )
        );
        assert_eq!(read.text.stderr, format!("note: {NO_CODE}\n"));
    }
}

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// RD-8, manual (`#[ignore]`), release: code-a with LAW-BOUND accepted,
/// "Data"'s edits and a crate of 1 000 unrelated `.rs` files, the indexes
/// warm: `spec lock diff LAW-BOUND` against `spec verify --node
/// LAW-BOUND`, the medians of five warm runs printed (`--no-capture`) for
/// the spec's "Implementation".
///
/// `cargo nextest run -p specengine-cli --release --test lock_diff
/// lock_diff_cost --run-ignored ignored-only --no-capture`
#[test]
#[ignore = "manual (RD-8): release, figures recorded in the spec"]
fn lock_diff_cost() {
    let repo = accepted("ld-cost");
    repo.write(
        "crates/filler/Cargo.toml",
        "[package]\nname = \"filler\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    let mut lib = String::new();
    for at in 0..1_000 {
        lib.push_str(&format!("pub mod part_{at:04};\n"));
        repo.write(
            &format!("crates/filler/src/part_{at:04}.rs"),
            format!(
                "/// Part {at}.\npub fn value_{at:04}(input: u32) -> u32 {{\n    input.wrapping_mul({at}).wrapping_add(1)\n}}\n"
            ),
        );
    }
    repo.write("crates/filler/src/lib.rs", lib);
    repo.commit_all("1 000 unrelated files");
    conflict_edits(&repo);
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let time = |args: &[&str]| {
        median(|| {
            let started = Instant::now();
            let run = repo.spec_piped(&repo.root, args);
            let took = started.elapsed();
            assert!(!run.stdout.is_empty(), "{}", run.show());
            took
        })
    };
    let node = time(&["verify", "--node", LAW]);
    let diff = time(&["lock", "diff", LAW]);
    let json = time(&["lock", "diff", LAW, "--json"]);
    eprintln!(
        "RD-8: code-a + 1 000 unrelated .rs files, LAW-BOUND accepted, sentence and CAP edited, warm indexes, median of 5: verify --node LAW-BOUND {} ms, lock diff LAW-BOUND {} ms, lock diff LAW-BOUND --json {} ms",
        node.as_millis(),
        diff.as_millis(),
        json.as_millis()
    );
}

// ------------------------------------------------------------- the genre

/// ADR-0008 (the core knows no genre), over `fixtures/verify-ecs` (an ECS
/// game crate: systems, a tuning constant, RON data), its four nodes
/// accepted: PERK-REGEN's sentence and `regen_system`'s body edited give
/// one `conflict` whose hunks are the section's and the system's lines;
/// TUNE-01's RON value edited gives its `ron` row a whole-file code side
/// with N6, the constant's row unlisted (`ok`).
#[test]
fn genre_an_ecs_game_crate_and_its_ron_tuning() {
    let repo = ecs("ld-ecs");
    repo.replace(
        "docs/spec/stamina.md",
        "Stamina regains a fixed amount per second while the runner rests.",
        "Stamina regains a fixed amount per second once the runner rests.",
    );
    repo.replace(STAMINA, "rate * dt)", "rate * dt * 0.5)");
    let run = diffed(&repo, "PERK-REGEN");
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 1, "{what}");
    assert_eq!(rows[0]["row"]["state"], "conflict", "{what}");
    assert_eq!(
        rows[0]["spec"]["diff"],
        "@@ -1,3 +1,3 @@\n ## Regen {#PERK-REGEN rev=1}\n \n-Stamina regains a fixed amount per second while the runner rests.\n+Stamina regains a fixed amount per second once the runner rests.\n",
        "{what}"
    );
    assert_eq!(
        rows[0]["code"]["diff"],
        "@@ -1,4 +1,4 @@\n pub fn regen_system(stamina: &mut Stamina, rate: f32, dt: f32) {\n     // Rest restores a fixed amount per second.\n-    stamina.current = (stamina.current + rate * dt).min(stamina.max);\n+    stamina.current = (stamina.current + rate * dt * 0.5).min(stamina.max);\n }\n",
        "{what}"
    );

    let movement = repo.read(MOVEMENT);
    repo.replace(MOVEMENT, "regen_per_second: 2.5,", "regen_per_second: 3.0,");
    let run = diffed(&repo, "TUNE-01");
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 1, "the RON row alone: {what}");
    let ron = &rows[0];
    assert_eq!(ron["row"]["kind"], "ron", "{what}");
    assert_eq!(
        ron["code"]["diff"],
        git_hunks(&repo, movement.as_bytes(), repo.read(MOVEMENT).as_bytes()),
        "{what}"
    );
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: {}: the code diff is the whole file: a ron binding",
            key_text(&ron["row"])
        )],
        "{what}"
    );
}

// --------------------------------------- binary sides, enclosing repos

/// `repo`'s variables with `HOME` (so the data directory) at `home`.
fn vars_with_home(repo: &LockRepo, home: &Path) -> Vec<(OsString, OsString)> {
    with_var(&vars_of(repo), "HOME", home.as_os_str())
}

/// A scratch git repository at `<scratch>/<name>` with `attributes` in its
/// `.gitattributes` and `config` in its local config, holding a `HOME`
/// (so the data directory lies inside it): that `HOME`.
fn enclosing_home(
    repo: &LockRepo,
    name: &str,
    attributes: &str,
    config: &[(&str, &str)],
) -> PathBuf {
    let top = repo.scratch.dir(name);
    repo.git.init(&top);
    for (key, value) in config {
        repo.git.git(&top, &["config", "--local", key, value]);
    }
    fs::write(top.join(".gitattributes"), attributes).expect(".gitattributes");
    let home = top.join("home");
    fs::create_dir_all(&home).expect("the enclosed HOME");
    home
}

/// git's own `diff --no-index` of two text files in `dir` (the flags "The
/// git door" names, nothing else): its stdout.
fn plain_git_diff(repo: &LockRepo, dir: &Path) -> String {
    fs::write(dir.join("probe-old"), "one\n").expect("probe");
    fs::write(dir.join("probe-new"), "two\n").expect("probe");
    let output = repo.git.git_output(
        dir,
        &[
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "--diff-algorithm=myers",
            "-U3",
            "--",
            "probe-old",
            "probe-new",
        ],
        &[],
    );
    let _ = fs::remove_file(dir.join("probe-old"));
    let _ = fs::remove_file(dir.join("probe-new"));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every shown side of `run` that moved is either git's hunks (non-empty)
/// or `null` with its note: never `""` (`: unchanged`).
#[track_caller]
fn assert_never_unchanged(run: &Diffed, what: &str) {
    for row in run.rows() {
        let key = key_text(&row["row"]);
        for side in ["spec", "code"] {
            if row[side].is_null() {
                continue;
            }
            match row[side]["diff"].as_str() {
                Some(hunks) => assert!(
                    hunks.starts_with("@@"),
                    "{what}: {key}: a moved {side} side shows hunks, not {hunks:?}: {}",
                    run.show()
                ),
                None => assert!(
                    run.notes().iter().any(
                        |note| note.starts_with(&format!("lock diff: {key}: no {side} diff: "))
                    ),
                    "{what}: {key}: a {side} side not shown has its note: {}",
                    run.show()
                ),
            }
        }
    }
    assert!(
        !run.read.text.stdout.contains(": unchanged\n"),
        "{what}: no moved side reads unchanged: {}",
        run.show()
    );
}

/// "Rules and edge cases" (a side that moved but cannot be shown: `null`
/// with one note, never `""`): LAW-BOUND's sentence edited to end in a NUL
/// byte, so git reads the span's two copies as binary and prints no hunk:
/// every spec side's `diff` null with one note naming its key, the text
/// `: not shown`, never `""` or `: unchanged`; CAP's code side, plain
/// text, keeps "Data"'s hunks.
#[test]
fn a_moved_side_holding_a_nul_is_not_shown_never_unchanged() {
    let repo = accepted("ld-nul");
    repo.replace(
        COUNTER_MD,
        OLD_SENTENCE,
        "The count never exceeds the cap.\u{0}",
    );
    repo.replace(COUNTER, OLD_CAP, NEW_CAP);
    let run = diffed(&repo, LAW);
    let what = run.show();
    let rows = run.rows();
    assert_eq!(rows.len(), 4, "{what}");
    for row in &rows {
        assert!(!row["spec"].is_null(), "premise: the node moved: {what}");
        assert!(
            row["spec"]["diff"].is_null(),
            "a binary spec side is not shown: {what}"
        );
    }
    let want: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "lock diff: {}: no spec diff: {NO_HUNK}",
                key_text(&row["row"])
            )
        })
        .collect();
    assert_eq!(
        run.notes(),
        want,
        "one note per side, in output order: {what}"
    );
    assert_eq!(
        run.row("crates/tally::counter::CAP")["code"]["diff"],
        CAP_HUNKS,
        "{what}"
    );
    assert_never_unchanged(&run, "a NUL");
}

/// The same rule for a code side. A NUL in Rust source breaks the item
/// (`cannot_verify item_error`, never kept), so the old side holds it:
/// PIECE-COUNTER accepted, its `lib.rs` module entry's `file_blob`
/// hand-set to a blob written here (`lib.rs` plus a comment holding a
/// NUL), `lib.rs` then edited: the module row's code side (whole files)
/// is binary to git: `diff` null with one note, never `""`.
#[test]
fn a_moved_code_side_holding_a_nul_is_not_shown_never_unchanged() {
    let repo = accepted("ld-nul-code");
    repo.accept_ok("PIECE-COUNTER");
    let mut bytes = repo.read(LIB).into_bytes();
    bytes.extend_from_slice(b"// \0 a NUL\n");
    let oid = String::from_utf8(repo.git.git_stdin(
        &repo.root,
        &["hash-object", "-w", "--stdin"],
        &bytes,
    ))
    .expect("an OID")
    .trim()
    .to_owned();
    edit_entry(
        &repo,
        ("PIECE-COUNTER", "implements", "crates/tally"),
        |block| set_string(block, "file_blob", &oid),
    );
    repo.replace(
        LIB,
        "pub use counter::Tally;\n",
        "pub use counter::Tally;\n\npub fn spare() -> u32 {\n    1\n}\n",
    );
    let run = diffed(&repo, "PIECE-COUNTER");
    let what = run.show();
    let module = run.row("crates/tally");
    assert_eq!(
        (&module["row"]["kind"], &module["row"]["state"]),
        (&Value::from("module"), &Value::from("code_ahead")),
        "premise: the module drifts: {what}"
    );
    assert_eq!(module["code"]["blob"], oid.as_str(), "{what}");
    assert!(
        module["code"]["diff"].is_null(),
        "a binary code side is not shown: {what}"
    );
    assert_eq!(
        run.notes(),
        [format!(
            "lock diff: PIECE-COUNTER | implements | {LIB} | module crates/tally: no code diff: {NO_HUNK}"
        )],
        "its one note, no N6 (a whole file not shown): {what}"
    );
    assert_never_unchanged(&run, "a NUL in the old code");
}

/// "Rules and edge cases" with git's attributes: a `.gitattributes` saying
/// `* -diff` in the project's repository, then in a repository enclosing
/// the data directory (where the scratch files are written), "Data"'s
/// edits: no moved side reads `""` or `: unchanged`, and the read's bytes
/// are the plain run's: the attribute never reaches the scratch files
/// (iteration 2: `-c core.attributesFile=/dev/null`, `GIT_ATTR_NOSYSTEM`,
/// `GIT_CEILING_DIRECTORIES` at the scratch directory's parent).
#[test]
fn a_diff_attribute_never_turns_a_moved_side_unchanged() {
    let repo = accepted("ld-attributes");
    conflict_edits(&repo);
    let plain = diffed(&repo, LAW);
    repo.write(".gitattributes", "* -diff\n");
    let in_project = diffed(&repo, LAW);
    assert_never_unchanged(&in_project, "`* -diff` in the project's repository");
    fs::remove_file(repo.root.join(".gitattributes")).expect(".gitattributes removed");
    let home = enclosing_home(&repo, "binary-top", "* -diff\n", &[]);
    assert!(
        plain_git_diff(&repo, home.parent().expect("the enclosing top")).contains("Binary files"),
        "premise: git reads files there as binary"
    );
    let enclosed = diffed_in(&repo, &repo.root, LAW, &vars_with_home(&repo, &home));
    assert_never_unchanged(&enclosed, "`* -diff` around the data directory");
    for (run, place) in [
        (&in_project, "the project"),
        (&enclosed, "an enclosing repository"),
    ] {
        assert_eq!(
            run.read.bytes(),
            plain.read.bytes(),
            "`* -diff` in {place}: the plain run's bytes\n{}",
            run.show()
        );
    }
}

/// "The git door" (global and system config unread; a diff by git's own
/// rules only): the data directory inside a repository whose local config
/// names a `textconv` driver for every file (`* diff=shout`, a script that
/// records its run and upper-cases its input): the driver never runs and
/// the read's bytes (exit, text, JSON, stderr) are the plain run's (its
/// `HOME` outside any repository).
#[test]
fn an_enclosing_repositorys_textconv_driver_never_runs() {
    let repo = accepted("ld-textconv");
    conflict_edits(&repo);
    let plain = diffed(&repo, LAW);
    let tools = repo.scratch.dir("textconv-tools");
    let marker = tools.join("driver-ran");
    let driver = tools.join("shout");
    fs::write(
        &driver,
        format!(
            "#!/bin/sh\necho ran >> '{}'\ntr 'a-z' 'A-Z' < \"$1\"\n",
            marker.to_str().expect("a UTF-8 path")
        ),
    )
    .expect("the driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("chmod the driver");
    let home = enclosing_home(
        &repo,
        "textconv-top",
        "* diff=shout\n",
        &[(
            "diff.shout.textconv",
            driver.to_str().expect("a UTF-8 path"),
        )],
    );
    let probe = plain_git_diff(&repo, home.parent().expect("the enclosing top"));
    assert!(
        marker.exists() && probe.contains("+TWO"),
        "premise: git runs the driver there: {probe:?}"
    );
    fs::remove_file(&marker).expect("the premise's marker removed");
    let enclosed = diffed_in(&repo, &repo.root, LAW, &vars_with_home(&repo, &home));
    assert!(
        !marker.exists(),
        "the textconv driver ran: {}",
        fs::read_to_string(&marker).unwrap_or_default()
    );
    assert_eq!(
        enclosed.read.bytes(),
        plain.read.bytes(),
        "the plain run's bytes\n{}",
        enclosed.show()
    );
}
