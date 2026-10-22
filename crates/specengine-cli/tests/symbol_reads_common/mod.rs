//! Helpers of the docs/features/symbol-reads.md tests (slice 6a-3,
//! `spec symbols` and `spec unmapped` over 6a-2's code index): scratch
//! copies of `fixtures/code-index` (TE-1's corpus as it stands) and
//! `fixtures/verify-ecs` (an ECS-style game crate, the second project of
//! another nature: ADR-0008), the `spec` binary with a scratch `HOME`
//! (`common::spec`), the JSON documents derived from the text goldens, the
//! key order of every JSON object as written, and a scratch ledger project
//! (another genre again) for RD-3's containment cases.
//!
//! The JSON shapes the spec leaves open are pinned here as the Phase A
//! report's readings (Q35, the owner's pre-approved answer from 6b's
//! analysis): `percent` a number or `null` where the text shows `-`;
//! `total` a row object; `omitted` `{symbols}` and `{rows, unmapped}`;
//! `target` one of [`TARGETS`]; `tier` `null` outside the `query` form; a
//! `--node` naming nothing an outcome with a `reason`, exit 1.
//!
//! docs/features/bevy-roles-reads.md (10a-2) "spec symbols": each target
//! gains `roles` after `bindings` ([`SYMBOL_KEYS`]), `null` without `[code]
//! frameworks` — every corpus these goldens read lists none, so the
//! derived JSON holds `roles: null` and a golden never holds a `  role `
//! line; a role object's keys are [`ROLE_KEYS`].
//!
//! docs/features/layer-c-reads.md (11a-3) "JSON": each target gains
//! `c_state`, `moniker`, `agrees` after `roles`, the document `layer_c`
//! after `code` ([`SYMBOL_KEYS`] 18, [`SYMBOLS_KEYS`] 12), all `null` while
//! no layer C snapshot is stored — no golden here has one, so the derived
//! JSON holds them `null` and a golden never holds a `  c ` or `layer c:`
//! line (the snapshot's reads: `tests/layer_c_reads.rs`).
//!
//! Only the symbol-reads test binaries include this module
//! (`mod symbol_reads_common;`, after `mod common;`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::common::{Run, Scratch, fixture, read_text, spec, write};

/// TE-1's corpus (6a-2's `fixtures/code-index`, Q24) and its slug.
pub const FIXTURE: &str = "code-index";
pub const SLUG: &str = "inkwell";

/// The ECS-style game crate of `fixtures/verify-ecs`.
pub const ECS: &str = "verify-ecs";

/// `spec refs`' line for a config without `[code]`
/// (docs/canon/code-bindings.md "The `[code]` table"); `symbols` and
/// `unmapped` print it alike.
pub const NO_TABLE: &str =
    "spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)\n";

/// `spec --json symbols`: the document's keys in order. The spec's list
/// plus `reason` (Q35: a `--node` naming nothing is an outcome with a
/// reason, as `refs`), placed before `notes` as `refs` places it — L3;
/// `layer_c` after `code` since docs/features/layer-c-reads.md "JSON"
/// (`null` while no layer C snapshot is stored, and at exit 1).
pub const SYMBOLS_KEYS: [&str; 12] = [
    "form", "query", "node", "path", "line", "reason", "notes", "symbols", "total", "omitted",
    "code", "layer_c",
];

/// One target of `symbols[]`, in order; `roles` after `bindings` since
/// docs/features/bevy-roles-reads.md "spec symbols"; `c_state`, `moniker`,
/// `agrees` after `roles` since docs/features/layer-c-reads.md "JSON"
/// (`null` while no snapshot is stored, and on a module or a RON path).
pub const SYMBOL_KEYS: [&str; 18] = [
    "target",
    "qpath",
    "name",
    "discriminator",
    "item_kind",
    "path",
    "start_line",
    "end_line",
    "signature",
    "ambiguity",
    "layer",
    "tier",
    "mentions",
    "bindings",
    "roles",
    "c_state",
    "moniker",
    "agrees",
];

/// One role of `symbols[].roles[]`, in order
/// (docs/features/bevy-roles-reads.md "spec symbols", JSON).
pub const ROLE_KEYS: [&str; 11] = [
    "role",
    "layer",
    "path",
    "line",
    "source",
    "form",
    "origin",
    "group",
    "state",
    "detail",
    "candidates",
];

/// One binding of `symbols[].bindings[]`, in order.
pub const BINDING_KEYS: [&str; 13] = [
    "relation",
    "line",
    "column",
    "token",
    "id",
    "rev",
    "id_state",
    "fix",
    "rev_state",
    "levels",
    "levels_state",
    "note",
    "node",
];

/// `spec --json unmapped`: the document's keys in order.
pub const UNMAPPED_KEYS: [&str; 7] = [
    "path", "rows", "total", "unmapped", "notes", "omitted", "code",
];

/// A row (`rows[]` and `total`), in order.
pub const ROW_KEYS: [&str; 6] = ["path", "dir", "units", "mapped", "percent", "mentioned"];

/// An unmapped item (`unmapped[]`), in order.
pub const ITEM_KEYS: [&str; 4] = ["qpath", "item_kind", "path", "line"];

/// L4 (Q35): the values of `target`, the binding words of `spec refs`.
pub const TARGETS: [&str; 3] = ["item", "module", "ron"];

/// L5: the values of `form`, one per form of the command line.
pub const FORMS: [&str; 4] = ["query", "node", "file", "at"];

// -------------------------------------------------------------- corpora

/// `fixtures/code-index/expected.json`'s `reads`.
pub fn reads() -> Value {
    let all: Value = serde_json::from_str(&read_text(&fixture(FIXTURE), "expected.json"))
        .expect("fixtures/code-index/expected.json is JSON");
    assert!(all["reads"].is_object(), "expected.json has its `reads`");
    all
}

/// The stored-symbol rows of `expected.json` (6a-2's TE-1), by (path,
/// qpath): `name` and `discriminator` of the JSON targets.
pub fn stored_symbol(all: &Value, path: &str, qpath: &str) -> Value {
    all["symbols"]
        .as_array()
        .expect("symbols")
        .iter()
        .find(|row| row["path"] == path && row["qpath"] == qpath)
        .cloned()
        .unwrap_or_else(|| panic!("expected.json stores no {qpath} in {path}"))
}

/// A copy of `fixtures/<name>` at `<scratch>/<dir>` without its
/// `expected.json` (the fixture is only read).
pub fn copy(scratch: &Scratch, name: &str, dir: &str) -> PathBuf {
    let root = scratch.copy(name, dir);
    let _ = fs::remove_file(root.join("expected.json"));
    root
}

/// `root`'s config with its `[code]` table (the config's last) removed.
pub fn without_code_table(root: &Path) {
    let text = read_text(root, "specengine.toml");
    let at = text.find("\n[code]\n").expect("a [code] table") + 1;
    write(root, "specengine.toml", &text[..at]);
}

/// Every `.rs` and `.ron` file under `root` (`.git` aside), root-relative.
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

/// `relative` under `root` with CRLF line ends.
pub fn to_crlf(root: &Path, relative: &str) {
    let text = read_text(root, relative);
    write(
        root,
        relative,
        text.replace("\r\n", "\n").replace('\n', "\r\n"),
    );
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

// ------------------------------------------------------------- running

/// `spec <args>` and `spec --json <args>` in `cwd`.
pub fn both(home: &Path, cwd: &Path, args: &[&str]) -> (Run, Run) {
    let mut json_args = vec!["--json"];
    json_args.extend(args);
    (spec(home, cwd, args), spec(home, cwd, &json_args))
}

/// `args` of a golden entry as `&str`s.
pub fn args_of(entry: &Value) -> Vec<String> {
    entry
        .as_array()
        .unwrap_or_else(|| panic!("an argument list: {entry}"))
        .iter()
        .map(|arg| arg.as_str().expect("an argument").to_owned())
        .collect()
}

pub fn as_strs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

/// A golden entry's `lines` as stdout (each line ended).
pub fn stdout_of(entry: &Value) -> String {
    entry["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|line| format!("{}\n", line.as_str().expect("a line")))
        .collect()
}

/// Exit 2 with stdout empty.
pub fn exit_2_quiet(run: &Run) {
    run.code(2);
    assert_eq!(
        run.stdout,
        "",
        "stdout stays empty on exit 2:\n{}",
        run.show()
    );
}

// ------------------------------------------------------- JSON key order

/// Every object of the JSON `text` as (its path, its keys in the order
/// written): `$` the document, `$.symbols[]` each target,
/// `$.symbols[].bindings[]` each binding, and so on.
pub fn key_orders(text: &str) -> Vec<(String, Vec<String>)> {
    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
        out: Vec<(String, Vec<String>)>,
    }
    impl Reader<'_> {
        fn blank(&mut self) {
            while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
        }
        fn string(&mut self) -> String {
            let start = self.at;
            assert_eq!(self.bytes[self.at], b'"', "a string at {start}");
            self.at += 1;
            while self.bytes[self.at] != b'"' {
                if self.bytes[self.at] == b'\\' {
                    self.at += 1;
                }
                self.at += 1;
            }
            self.at += 1;
            serde_json::from_slice(&self.bytes[start..self.at]).expect("a JSON string")
        }
        fn value(&mut self, path: &str) {
            self.blank();
            match self.bytes[self.at] {
                b'{' => {
                    self.at += 1;
                    let slot = self.out.len();
                    self.out.push((path.to_owned(), Vec::new()));
                    let mut keys = Vec::new();
                    loop {
                        self.blank();
                        if self.bytes[self.at] == b'}' {
                            self.at += 1;
                            break;
                        }
                        if self.bytes[self.at] == b',' {
                            self.at += 1;
                            continue;
                        }
                        let key = self.string();
                        self.blank();
                        assert_eq!(self.bytes[self.at], b':', "a colon after {key:?}");
                        self.at += 1;
                        self.value(&format!("{path}.{key}"));
                        keys.push(key);
                    }
                    self.out[slot].1 = keys;
                }
                b'[' => {
                    self.at += 1;
                    loop {
                        self.blank();
                        if self.bytes[self.at] == b']' {
                            self.at += 1;
                            break;
                        }
                        if self.bytes[self.at] == b',' {
                            self.at += 1;
                            continue;
                        }
                        self.value(&format!("{path}[]"));
                    }
                }
                b'"' => {
                    self.string();
                }
                _ => {
                    while self.at < self.bytes.len()
                        && !matches!(self.bytes[self.at], b',' | b'}' | b']')
                        && !self.bytes[self.at].is_ascii_whitespace()
                    {
                        self.at += 1;
                    }
                }
            }
        }
    }
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
        out: Vec::new(),
    };
    reader.value("$");
    reader.out
}

/// Every object at `path` of `text` has exactly `keys`, in that order; at
/// least `at_least` such objects exist.
pub fn assert_key_order(text: &str, path: &str, keys: &[&str], at_least: usize, context: &str) {
    let orders = key_orders(text);
    let found: Vec<&Vec<String>> = orders
        .iter()
        .filter(|(at, _)| at == path)
        .map(|(_, keys)| keys)
        .collect();
    assert!(
        found.len() >= at_least,
        "{context}: {} objects at {path}, want at least {at_least}\n{text}",
        found.len()
    );
    for got in found {
        assert_eq!(got, keys, "{context}: the keys at {path}");
    }
}

// ------------------------------------------- JSON derived from the text

/// `id` and `rev` of a marker token ("spec refs": the ID up to the first
/// `@`, `rev` its 1-9 digits ending the token).
fn id_and_rev(token: &str) -> (Value, Value) {
    match token.split_once('@') {
        None => (json!(token), Value::Null),
        Some((id, digits)) => {
            let ok = (1..=9).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit());
            (
                json!(id),
                if ok {
                    json!(digits.parse::<u64>().expect("digits"))
                } else {
                    Value::Null
                },
            )
        }
    }
}

/// `levels`, `levels_state` from the text's `<levels>` ("spec refs").
fn levels_json(field: &str) -> (Value, Value) {
    match field {
        "default" => (json!(["sig", "body"]), json!("default")),
        invalid if invalid.starts_with("invalid:") => {
            (Value::Null, json!(&invalid["invalid:".len()..]))
        }
        declared => (
            json!(
                declared
                    .trim_matches(['[', ']'])
                    .split(", ")
                    .collect::<Vec<_>>()
            ),
            json!("declared"),
        ),
    }
}

/// A binding's JSON from its text row (two leading spaces trimmed):
/// `<relation> | <line>:<column> | <token> | <id_state>[ <fix>] |
/// <rev_state> | <levels> | <node or ->[ | <note>]`.
pub fn binding_json(row: &str) -> Value {
    let fields: Vec<&str> = row.trim_start().split(" | ").collect();
    assert!(fields.len() >= 7, "a binding row: {row:?}");
    let (line, column) = fields[1].split_once(':').expect("line:column");
    let (id, rev) = id_and_rev(fields[2]);
    let (id_state, fix) = match fields[3].split_once(' ') {
        Some((state, fix)) => (state, json!(fix)),
        None => (fields[3], Value::Null),
    };
    let (levels, levels_state) = levels_json(fields[5]);
    let node = if fields[6] == "-" {
        Value::Null
    } else {
        json!(fields[6])
    };
    json!({
        "relation": fields[0],
        "line": line.parse::<u64>().expect("a line"),
        "column": column.parse::<u64>().expect("a column"),
        "token": fields[2],
        "id": id,
        "rev": rev,
        "id_state": id_state,
        "fix": fix,
        "rev_state": fields[4],
        "levels": levels,
        "levels_state": levels_state,
        "note": fields.get(7).map_or(Value::Null, |note| json!(note)),
        "node": node,
    })
}

/// `code files <f> (.rs <r>, .ron <o>), skipped <s>` as `refs`' JSON
/// `code`.
pub fn code_json(text: &str) -> Value {
    let numbers: Vec<u64> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().expect("a count"))
        .collect();
    assert_eq!(numbers.len(), 4, "code files … in {text:?}");
    json!({"files": numbers[0], "rs": numbers[1], "ron": numbers[2], "skipped": numbers[3]})
}

/// One target's JSON from its line (L4, L6: a module's and a RON path's
/// fields the text does not print are `null`; a RON path's `start_line`
/// is its printed line); `stored` gives an item's `name`, `discriminator`.
/// `roles` is `null`: no golden's corpus lists `[code] frameworks`
/// (docs/features/bevy-roles-reads.md "spec symbols").
fn target_json(line: &str, stored: &dyn Fn(&str, &str) -> Value) -> Value {
    let fields: Vec<&str> = line.split(" | ").collect();
    let mut value = json!({
        "target": null, "qpath": fields[0], "name": null, "discriminator": null,
        "item_kind": null, "path": null, "start_line": null, "end_line": null,
        "signature": null, "ambiguity": null, "layer": null, "tier": null,
        "mentions": null, "bindings": [], "roles": null,
        "c_state": null, "moniker": null, "agrees": null,
    });
    match fields[1] {
        "module" => {
            assert_eq!(fields.len(), 3, "a module target: {line:?}");
            value["target"] = json!("module");
            value["path"] = json!(fields[2]);
        }
        "ron" => {
            assert_eq!(fields.len(), 3, "a RON target: {line:?}");
            let (path, at) = fields[2].rsplit_once(':').expect("path:line");
            value["target"] = json!("ron");
            value["path"] = json!(path);
            value["start_line"] = json!(at.parse::<u64>().expect("a line"));
        }
        item_kind => {
            let (path, span) = fields[2].rsplit_once(':').expect("path:start-end");
            let (start, end) = span.split_once('-').expect("start-end");
            let row = stored(path, fields[0]);
            value["target"] = json!("item");
            value["name"] = row["name"].clone();
            value["discriminator"] = row["discriminator"].clone();
            value["item_kind"] = json!(item_kind);
            value["path"] = json!(path);
            value["start_line"] = json!(start.parse::<u64>().expect("start"));
            value["end_line"] = json!(end.parse::<u64>().expect("end"));
            value["layer"] = json!(fields[3].strip_prefix("layer ").expect("layer <l>"));
            value["mentions"] = json!(
                fields[4]
                    .strip_prefix("mentions ")
                    .expect("mentions <n>")
                    .parse::<u64>()
                    .expect("a count")
            );
            for extra in &fields[5..] {
                if let Some(reason) = extra.strip_prefix("ambiguous ") {
                    value["ambiguity"] = json!(reason);
                } else {
                    assert!(
                        ["exact", "suffix", "name"].contains(extra),
                        "a tier: {extra:?} in {line:?}"
                    );
                    value["tier"] = json!(extra);
                }
            }
        }
    }
    value
}

/// The JSON `spec --json symbols` must print for a golden's `lines` and
/// its request `echo` (`form`, `query`, `node`, `path`, `line`); `stored`
/// as [`target_json`].
pub fn symbols_json(lines: &[String], echo: &Value, stored: &dyn Fn(&str, &str) -> Value) -> Value {
    let mut targets: Vec<Value> = Vec::new();
    let mut summary = None;
    for line in lines {
        if let Some(rest) = line.strip_prefix("symbols: ") {
            summary = Some(rest.to_owned());
        } else if let Some(signature) = line.strip_prefix("  sig ") {
            let target = targets.last_mut().expect("a target before its sig");
            target["signature"] = if signature == "-" {
                Value::Null
            } else {
                json!(signature)
            };
        } else if line.starts_with("  ") {
            assert!(
                !line.starts_with("  role "),
                "a role line in a golden without `[code] frameworks`: {line:?}"
            );
            let target = targets.last_mut().expect("a target before its bindings");
            target["bindings"]
                .as_array_mut()
                .expect("bindings")
                .push(binding_json(line));
        } else {
            targets.push(target_json(line, stored));
        }
    }
    let summary = summary.expect("a summary line");
    let (counts, code) = summary.split_once("; ").expect("`N of M; code files …`");
    let (shown, total) = counts.split_once(" of ").expect("N of M");
    assert_eq!(
        shown.parse::<usize>().expect("N"),
        targets.len(),
        "the golden's own count"
    );
    json!({
        "form": echo["form"], "query": echo["query"], "node": echo["node"],
        "path": echo["path"], "line": echo["line"], "reason": null, "notes": [],
        "symbols": targets, "total": total.parse::<u64>().expect("M"),
        "omitted": {"symbols": 0}, "code": code_json(code), "layer_c": null,
    })
}

/// A row object from a row line: `<label> | units U | mapped M | <P %|-> |
/// mentioned N`; `(path, dir)` given for the total row (L7, L8).
fn row_json(line: &str, total_of: Option<(&Value, bool)>) -> Value {
    let fields: Vec<&str> = line.split(" | ").collect();
    assert_eq!(fields.len(), 5, "a row: {line:?}");
    let number = |field: &str, word: &str| -> u64 {
        field
            .strip_prefix(word)
            .unwrap_or_else(|| panic!("{word}… in {line:?}"))
            .parse()
            .expect("a count")
    };
    let percent = match fields[3] {
        "-" => Value::Null,
        text => json!(
            text.strip_suffix(" %")
                .expect("<P> %")
                .parse::<f64>()
                .expect("a percent")
        ),
    };
    let (path, dir) = match total_of {
        Some((path, dir)) => (path.clone(), dir),
        None => match fields[0].strip_suffix('/') {
            Some(dir) => (json!(dir), true),
            None => (json!(fields[0]), false),
        },
    };
    json!({
        "path": path, "dir": dir,
        "units": number(fields[1], "units "), "mapped": number(fields[2], "mapped "),
        "percent": percent, "mentioned": number(fields[4], "mentioned "),
    })
}

/// The JSON `spec --json unmapped [PATH]` must print for a golden's
/// `lines`: rows, the `(total)` row as `total` (a file PATH: its own row,
/// L8), the item lines as `unmapped`.
pub fn unmapped_json(lines: &[String], path: &Value) -> Value {
    let mut rows = Vec::new();
    let mut total = None;
    let mut items = Vec::new();
    let mut code = None;
    let file = path.as_str().is_some_and(|path| {
        lines
            .first()
            .is_some_and(|line| line.starts_with(&format!("{path} | ")))
    });
    for line in lines {
        if let Some(rest) = line.strip_prefix("unmapped: ") {
            code = Some(code_json(rest));
        } else if let Some(item) = line.strip_prefix("  ") {
            let fields: Vec<&str> = item.split(" | ").collect();
            assert_eq!(fields.len(), 3, "an item: {line:?}");
            items.push(json!({
                "qpath": fields[0], "item_kind": fields[1], "path": path,
                "line": fields[2].parse::<u64>().expect("a line"),
            }));
        } else if line
            .split(" | ")
            .next()
            .is_some_and(|label| label.ends_with("(total)"))
        {
            total = Some(row_json(line, Some((path, true))));
        } else {
            rows.push(row_json(line, None));
        }
    }
    let total = match total {
        Some(total) => total,
        None => {
            assert!(file && rows.len() == 1, "a file PATH's one row: {lines:?}");
            rows[0].clone()
        }
    };
    json!({
        "path": path, "rows": rows, "total": total, "unmapped": items, "notes": [],
        "omitted": {"rows": 0, "unmapped": 0}, "code": code.expect("a summary line"),
    })
}

/// `value` with every `percent` number read as `f64` (`100` and `100.0`
/// alike: the type is pinned, not the digits), for comparison.
pub fn percents_as_f64(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| {
                    let item = if key == "percent" && item.is_number() {
                        json!(item.as_f64().expect("a number"))
                    } else {
                        percents_as_f64(item)
                    };
                    (key.clone(), item)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(percents_as_f64).collect()),
        other => other.clone(),
    }
}

/// The lines of a golden entry.
pub fn lines_of(entry: &Value) -> Vec<String> {
    entry["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|line| line.as_str().expect("a line").to_owned())
        .collect()
}

// ------------------------------------------------- the ledger (RD-3)

/// A scratch project of another genre (a ledger of books and terms) for
/// the containment cases of RD-3, written at test time under `root`:
/// a package `crates/ledger` (its lib holds no item), the files of
/// [`LEDGER_FILES`], RON data, and a spec where `TERM-02` has two holders
/// and `OLDTERM-01` is an alias of `TERM-01`.
pub fn ledger(scratch: &Scratch, dir: &str) -> PathBuf {
    let root = scratch.dir(dir);
    write(
        &root,
        "specengine.toml",
        "# A scratch ledger (docs/features/symbol-reads.md RD-3).\n\n\
         [project]\nslug = \"ledger\"\n\n\
         [ids]\n\
         BOOK = { kind = \"volume\", shape = \"name\" }\n\
         TERM = { kind = \"entry\", width = 2, aliases_from = [\"OLDTERM\"] }\n\n\
         [code]\nroots = [\"crates\", \"data\"]\n",
    );
    let doc = |id: &str, title: &str, body: &str| {
        format!(
            "---\nclass: canon\nid: {id}\nowner: owner\nreviewed: 2026-10-16\n---\n\n# {title}\n\n{body}\n"
        )
    };
    write(
        &root,
        "docs/spec/book.md",
        doc(
            "BOOK-SHELF",
            "Shelf",
            "A shelf holds the books.\n\n## Rows {#BOOK-ROWS}\n\nBooks stand in rows.",
        ),
    );
    write(
        &root,
        "docs/records/TERM-01.md",
        doc("TERM-01", "One term", "A term is kept once."),
    );
    write(
        &root,
        "docs/records/TERM-02.md",
        doc("TERM-02", "Two holders", "Written twice."),
    );
    write(
        &root,
        "docs/records/TERM-02-twin.md",
        doc("TERM-02", "Two holders again", "Written twice."),
    );
    write(
        &root,
        "crates/ledger/Cargo.toml",
        "[package]\nname = \"ledger\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n",
    );
    for (path, text) in LEDGER_FILES {
        write(&root, path, text);
    }
    for (path, text) in [
        ("crates/ledger/src/count/few.rs", counted("c", 1, 1)),
        ("crates/ledger/src/count/five.rs", counted("b", 16, 5)),
        ("crates/ledger/src/count/many.rs", counted("a", 16, 1)),
    ] {
        write(&root, path, text);
    }
    root
}

/// `items` one-line functions `<stem>01…`, the first `mapped` of them under
/// a resolved marker.
fn counted(stem: &str, items: usize, mapped: usize) -> String {
    let mut text = String::new();
    for index in 1..=items {
        if index <= mapped {
            text.push_str("// @implements TERM-01@1\n");
        }
        text.push_str(&format!("pub fn {stem}{index:02}() {{}}\n"));
    }
    text
}

/// The ledger's code files besides `count/` (line numbers are the
/// goldens' in `symbol_unmapped.rs`).
pub const LEDGER_FILES: [(&str, &str); 12] = [
    ("crates/ledger/src/lib.rs", "//! The ledger.\n"),
    (
        "crates/ledger/src/book.rs",
        "//! The book.\n\
         //! @implements BOOK-SHELF@1\n\
         \n\
         pub struct Entry;\n\
         \n\
         impl Entry {\n    pub fn amount(&self) -> u32 {\n        1\n    }\n}\n\
         \n\
         pub mod pages {\n    pub fn first() -> u32 {\n        1\n    }\n}\n",
    ),
    (
        "crates/ledger/src/shelf.rs",
        "//! The shelf.\n\
         \n\
         // @implements BOOK-SHELF@1\n\
         impl Shelf {\n    pub fn count(&self) -> u32 {\n        0\n    }\n}\n\
         \n\
         pub struct Shelf;\n\
         \n\
         pub mod drawer {\n    //! @implements BOOK-ROWS@1\n\n    pub mod inner {\n        pub fn deep() -> u32 {\n            2\n        }\n    }\n}\n\
         \n\
         pub mod loose {\n    pub fn alone() -> u32 {\n        3\n    }\n}\n",
    ),
    (
        "crates/ledger/src/notes.rs",
        "//! Notes on BOOK-SHELF.\n\
         \n\
         // See TERM-01 for the terms.\n\
         use std::fmt as _;\n\
         \n\
         /// Kept per BOOK-ROWS.\n\
         pub fn kept() -> u32 {\n    // counts toward TERM-01\n    1\n}\n\
         \n\
         pub fn plain() -> u32 {\n    // @implements ZZZ-1 see BOOK-SHELF\n    2\n}\n",
    ),
    (
        "crates/ledger/src/quiet.rs",
        "// About TERM-01, before a use.\n\
         use std::fmt as _;\n\
         \n\
         pub fn one() -> u32 {\n    1\n}\n\
         \n\
         pub fn two() -> u32 {\n    2\n}\n",
    ),
    (
        "crates/ledger/src/single.rs",
        "/// Per TERM-01.\n\
         pub fn noted() -> u32 {\n    1\n}\n\
         \n\
         pub fn silent() -> u32 {\n    2\n}\n",
    ),
    (
        "crates/ledger/src/odd.rs",
        "//! @implements ZZZ-1@1\n\
         //! @implements PR-12\n\
         //! @implements other:BOOK-SHELF@1\n\
         //! @implements \u{0412}OOK-SHELF@1\n\
         \n\
         // @implements ZZZ-2\n\
         impl Odd {\n    pub fn even(&self) -> u32 {\n        0\n    }\n}\n\
         \n\
         pub struct Odd;\n\
         \n\
         pub mod deep {\n    //! @implements PR-3\n\n    pub mod deeper {\n        //! @verifies \u{0412}OOK-ROWS\n\n        // @implements other:TERM-01\n        pub fn bottom() -> u32 {\n            1\n        }\n    }\n}\n",
    ),
    (
        "crates/ledger/src/even.rs",
        "//! The even half.\n\
         \n\
         // @implements TERM-02\n\
         impl Even {\n    pub fn both(&self) -> u32 {\n        2\n    }\n}\n\
         \n\
         pub struct Even;\n\
         \n\
         pub mod kept {\n    //! @implements OLDTERM-01@1\n\n    pub fn by_alias() -> u32 {\n        1\n    }\n}\n\
         \n\
         // @implements BOOK-ROWS@1\n\
         pub fn section() -> u32 {\n    3\n}\n",
    ),
    ("data/odd.ron", "// @configures ZZZ-9\n(rate: 1)\n"),
    (
        "data/tail.ron",
        "(\n    rate: 1,\n    // @configures TERM-01@1\n)\n",
    ),
    (
        "data/even.ron",
        "(\n    rate: 1, // @configures OLDTERM-01@1\n)\n",
    ),
    ("data/notes.ron", "// see BOOK-SHELF\n(rate: 1)\n"),
];
