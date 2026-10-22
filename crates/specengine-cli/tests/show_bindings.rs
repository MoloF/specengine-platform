//! docs/features/show-bindings.md (7a-1): `spec show REF --bindings`,
//! each shown node's code binding rows (its own and its contained
//! sections'), each judged as `spec verify --node` judges it. RD-1, RD-2,
//! RD-3, RD-5, RD-8 (each test names the mutation that turns it red),
//! TE-1 (the differential on every corpus), TE-2 (determinism), TE-3 (the
//! cost, manual) and TE-4 (`fixtures/spec-c` unchanged).
//!
//! **The oracle is `spec verify --node`** (Q-B): on one state, `spec show
//! REF --bindings` (text, then `--json`, stdin null) and, for every node of
//! a shown node's set (its ID, then its `sections`, as the same JSON lists
//! them), `spec verify --node <ID> --json`. A shown row's key (`node`,
//! `relation`, `file`, `kind`, `symbol`, `item_kind`), `sync`, `category`,
//! `detail` and `anchor` are a kept verify row's (`state` for `sync`),
//! row for row; its `line`, `end_line`, `signature` and `layer` are the
//! code index's stored symbol of that key (read through the system's
//! `sqlite3` after the read refreshed it: [`stored_symbols`]); a RON row's
//! `line` its first marker's, the rest `null`; a key with no stored symbol
//! all four `null`. The set itself is checked against the parser
//! ([`parsed_set`]): a node's ID and every ID section within its span.
//! The spec part of a read is `spec show REF`'s, byte for byte (text with
//! the block taken out; JSON with the `bindings` keys set aside).
//!
//! Fixtures reused, no new one: `code-a` with the format-2 golden lock
//! (`verify_common::golden`), `anchor-states` (`anchor_common`, its 18
//! scenarios), `verify-tests` (a kiln library, no lock), `verify-ecs`
//! (`verify_common::ecs`, an ECS game crate with RON data) and `spec-c`
//! (another genre, no `[code]`). Each repository has its own `HOME`.
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).
//!
//! docs/features/tested-reads.md (8b-2): a node's `bindings` is `{rows,
//! omitted, tested}` (`tested` `[]` over every corpus above: none holds a
//! block); RD-5 `show_cut_keeps_tested_with_closing` over the evidence
//! kiln (`tested_kiln::evidence_kiln`).

#![cfg(unix)]

mod anchor_common;
mod code_index_common;
mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Run, copy_dir, data_dir, fixture, repository_root, snapshot, spec_with};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_core::ProjectConfig;
use specengine_store::{ProposalQueue as _, SqliteQueue};
use verify_common::{
    BOUNDS, COUNTER, COUNTER_MD, LIB, LIMITS, MOVEMENT, STAMINA, clone_dir, ecs, golden,
    golden_file, golden_format_1, new_task, set_mode,
};

/// The slugs of the fixtures: the index and queue databases' names.
const CODE_A: &str = "code-a";
const ANCHORS: &str = "anchor-states";
const KILN: &str = "kiln-works";
const GAME: &str = "verify-ecs";

/// The cut (`spec-cli-read.md` "The cap"): `OUTPUT_CAP_CHARS`.
const CAP: usize = 40_000;

/// The look-alike of `GOAL-01` (a Cyrillic O).
const LOOK_ALIKE: &str = "G\u{41E}AL-01";

/// The look-alike of `LAW-BOUND` (a Cyrillic A).
const LAW_LOOK_ALIKE: &str = "L\u{410}W-BOUND";

/// Every state of `spec-verify.md` "States": the words `sync` takes.
const STATES: [&str; 9] = [
    "bad_marker",
    "broken_binding",
    "cannot_verify",
    "code_ahead",
    "conflict",
    "ok",
    "predated",
    "spec_ahead",
    "unlocked",
];

/// Every anchor state (`spec-verify-anchors.md` "Anchors per row").
const ANCHOR_STATES: [&str; 4] = ["ambiguous", "exact", "lost", "moved"];

/// The 14 keys of a binding row, in order (docs/features/show-bindings.md
/// "Data"; model `BindingRow`).
const ROW_KEYS: [&str; 14] = [
    "node",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "line",
    "end_line",
    "signature",
    "layer",
    "sync",
    "category",
    "detail",
    "anchor",
];

/// `code-bindings.md` "The `[code]` table": the sentence of a config
/// without the table (`spec refs`' exit-2 line, without `spec: `).
const NO_CODE: &str =
    "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The note of a read asked for bindings without `[code]`
/// (docs/features/show-bindings.md "Degrade").
fn no_code_note() -> String {
    format!("note: bindings: {NO_CODE}")
}

// ------------------------------------------------------------------- runs

/// One `spec show <args>` as the binary from one cwd: text, then
/// `--json`.
struct Show {
    what: String,
    text: Run,
    json_run: Run,
}

impl Show {
    fn code(&self) -> i32 {
        self.text.code
    }

    /// stdout without `--json`.
    fn text(&self) -> &str {
        &self.text.stdout
    }

    /// stdout with `--json`.
    fn json_text(&self) -> &str {
        &self.json_run.stdout
    }

    fn json(&self) -> Value {
        common::one_json_document(&self.json_run.stdout)
            .unwrap_or_else(|problem| panic!("{}: {problem}\n{}", self.what, self.show()))
    }

    fn nodes(&self) -> Vec<Value> {
        list(&self.json()["nodes"])
    }

    /// The `at`th node's `bindings`: `{rows, omitted, tested}` or `null`.
    fn bindings(&self, at: usize) -> Value {
        self.nodes()[at]["bindings"].clone()
    }

    /// The `at`th node's `bindings.rows`, a list.
    fn rows(&self, at: usize) -> Vec<Value> {
        let bindings = self.bindings(at);
        let keys: Vec<&str> = bindings
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(
            keys,
            ["omitted", "rows", "tested"],
            "{}: node {at}'s bindings are {{rows, omitted, tested}} (docs/features/tested-reads.md), not {bindings}\n{}",
            self.what,
            self.show()
        );
        list(&bindings["rows"])
    }

    /// The `at`th node's `bindings.omitted`.
    fn omitted(&self, at: usize) -> u64 {
        self.bindings(at)["omitted"]
            .as_u64()
            .expect("bindings.omitted, a count")
    }

    /// Every node's rows, in node order.
    fn all_rows(&self) -> Vec<Value> {
        (0..self.nodes().len())
            .flat_map(|at| self.rows(at))
            .collect()
    }

    /// The `note: ` lines of stderr (text run).
    fn notes(&self) -> Vec<String> {
        self.text
            .stderr
            .lines()
            .filter(|line| line.starts_with("note: "))
            .map(str::to_owned)
            .collect()
    }

    fn show(&self) -> String {
        let cut = |text: &str| -> String { text.chars().take(6_000).collect() };
        format!(
            "{}: exit {}\n--- stdout\n{}\n--- stdout --json\n{}\n--- stderr\n{}",
            self.what,
            self.code(),
            cut(&self.text.stdout),
            cut(&self.json_run.stdout),
            self.text.stderr
        )
    }

    /// Exit 0 with the node(s) on stdout, text and JSON.
    fn answered(&self) -> &Self {
        assert_eq!(self.code(), 0, "{}", self.show());
        assert!(
            !self.text.stdout.is_empty() && !self.json_run.stdout.is_empty(),
            "{}: an answer\n{}",
            self.what,
            self.show()
        );
        self
    }

    /// Every printed byte: the exit, stdout (text, JSON), stderr.
    fn bytes(&self) -> (i32, &str, &str, &str) {
        (
            self.code(),
            self.text(),
            self.json_text(),
            &self.text.stderr,
        )
    }
}

/// Every variable a `spec` process of `repo` gets: the sandbox's (its
/// `PATH` with git), `repo`'s `HOME` (`LockRepo::spec_piped`'s).
fn vars_of(repo: &LockRepo) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = repo
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), repo.home.clone().into_os_string()));
    vars
}

/// `vars` with `name` set to `value`.
fn with_var(vars: &[(OsString, OsString)], name: &str, value: &OsStr) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = vars
        .iter()
        .filter(|(known, _)| known != name)
        .cloned()
        .collect();
    vars.push((name.into(), value.to_owned()));
    vars
}

/// `spec args` in `cwd` with exactly `vars`, stdin null.
fn run_with(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Run {
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.to_str().expect("a UTF-8 name"), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

/// `spec show <args>` from `cwd` with `vars`, text and `--json`: both exit
/// alike and print the same stderr.
fn show_with(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Show {
    let mut args: Vec<&str> = [&["show"][..], args].concat();
    let what = format!("spec {}", args.join(" "));
    let text = run_with(cwd, &args, vars);
    args.push("--json");
    let json_run = run_with(cwd, &args, vars);
    assert_eq!(
        text.code,
        json_run.code,
        "{what}: text and JSON exit alike\n{}\n{}",
        text.show(),
        json_run.show()
    );
    assert_eq!(
        text.stderr, json_run.stderr,
        "{what}: text and JSON print the same stderr"
    );
    Show {
        what,
        text,
        json_run,
    }
}

/// `spec show <args>` from `cwd`, `repo`'s environment.
fn show_at(repo: &LockRepo, cwd: &Path, args: &[&str]) -> Show {
    show_with(cwd, args, &vars_of(repo))
}

/// `spec show REF --bindings` from the root.
fn bound(repo: &LockRepo, reference: &str) -> Show {
    show_at(repo, &repo.root, &[reference, "--bindings"])
}

/// `spec show REF` from the root: the spec part every read keeps.
fn unbound(repo: &LockRepo, reference: &str) -> Show {
    show_at(repo, &repo.root, &[reference])
}

/// `spec index` from the root: the indexes warm.
fn index(repo: &LockRepo) {
    repo.spec_piped(&repo.root, &["index"]).code(0);
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn list(value: &Value) -> Vec<Value> {
    value.as_array().expect("a list").clone()
}

/// A row's six key fields.
fn key_of(row: &Value) -> String {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
    })
    .to_string()
}

/// The distinct strings of `field` over `rows` (`null` left out).
fn values_of(rows: &[Value], field: impl Fn(&Value) -> &Value) -> BTreeSet<String> {
    rows.iter()
        .map(field)
        .filter(|value| !value.is_null())
        .map(|value| text(value).to_owned())
        .collect()
}

fn owned(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

// ---------------------------------------------------------------- oracle

/// `verify --node`'s answer for one REF: its `--json` document and, per
/// row in the same order, the text's `<binding>` field and the lines under
/// the row (`moved from`, `candidate`), indent dropped.
struct Judged {
    document: Value,
    texts: Vec<(String, Vec<String>)>,
}

impl Judged {
    fn rows(&self) -> Vec<Value> {
        list(&self.document["rows"])
    }
}

/// The text rows of a `verify` report: per row line (`<class> | <state> |
/// <node> | <relation> | <file> | <binding>`) its `<binding>` and the
/// `moved from` and `candidate` lines under it.
fn verify_text_rows(text: &str) -> Vec<(String, Vec<String>)> {
    let classes = [
        "-",
        "fix",
        "check",
        "cannot",
        "pre-existing fix",
        "pre-existing check",
        "pre-existing cannot",
    ];
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if line.starts_with("  moved from ") || line.starts_with("  candidate ") {
            rows.last_mut()
                .expect("a row above its anchor line")
                .1
                .push(line.trim_start().to_owned());
            continue;
        }
        let fields: Vec<&str> = line.splitn(6, " | ").collect();
        if fields.len() == 6 && classes.contains(&fields[0]) {
            rows.push((fields[5].to_owned(), Vec::new()));
        }
    }
    rows
}

/// `spec verify --node REF` from `cwd` with `vars`, text and `--json`: its
/// document and text rows, whatever its causes.
fn judged_with(cwd: &Path, reference: &str, vars: &[(OsString, OsString)]) -> Judged {
    let text = run_with(cwd, &["verify", "--node", reference], vars);
    let json_run = run_with(cwd, &["verify", "--node", reference, "--json"], vars);
    let document = common::one_json_document(&json_run.stdout).unwrap_or_else(|problem| {
        panic!(
            "spec verify --node {reference}: {problem}\n{}",
            json_run.show()
        )
    });
    let texts = verify_text_rows(&text.stdout);
    assert_eq!(
        texts.len(),
        list(&document["rows"]).len(),
        "premise: verify --node {reference}'s text prints its JSON rows\n{}",
        text.show()
    );
    Judged { document, texts }
}

/// `spec verify --node REF` from the root, judging (no run cause, uncut).
fn judged(repo: &LockRepo, reference: &str) -> Judged {
    let judged = judged_with(&repo.root, reference, &vars_of(repo));
    assert_eq!(
        judged.document["cannot"],
        json!([]),
        "premise: verify --node {reference} judges (no run cause): {}",
        judged.document
    );
    assert_eq!(
        judged.document["omitted"],
        json!({"rows": 0}),
        "premise: verify --node {reference} uncut"
    );
    judged
}

/// `spec verify --node ID --json`'s document from the root (judging).
fn verify_node(repo: &LockRepo, id: &str) -> Value {
    judged(repo, id).document
}

/// One verify row with its text: the oracle of one binding row.
#[derive(Debug, Clone)]
struct Expected {
    row: Value,
    binding: String,
    under: Vec<String>,
}

/// The verify rows of each set node and plain verify's row order, asked
/// once per state.
struct Oracle {
    by_ref: BTreeMap<String, Vec<Expected>>,
    /// Each row's place in plain `spec verify --json`'s rows (key order),
    /// by the whole row (keyless rows share a key).
    order: BTreeMap<String, usize>,
}

impl Oracle {
    fn new(repo: &LockRepo) -> Self {
        let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
        let plain = common::one_json_document(&run.stdout)
            .unwrap_or_else(|problem| panic!("spec verify: {problem}\n{}", run.show()));
        let order = list(&plain["rows"])
            .iter()
            .enumerate()
            .map(|(at, row)| (row.to_string(), at))
            .collect();
        Self {
            by_ref: BTreeMap::new(),
            order,
        }
    }

    /// `verify --node REF`'s rows with their texts.
    fn rows(&mut self, repo: &LockRepo, reference: &str) -> Vec<Expected> {
        self.by_ref
            .entry(reference.to_owned())
            .or_insert_with(|| {
                let judged = judged(repo, reference);
                judged
                    .rows()
                    .into_iter()
                    .zip(judged.texts)
                    .map(|(row, (binding, under))| Expected {
                        row,
                        binding,
                        under,
                    })
                    .collect()
            })
            .clone()
    }

    /// The rows of every REF of `set`, each row once (a row two set
    /// nodes keep is one), in verify's row order (plain's, which `--node`
    /// keeps whole: `verify_node.rs`).
    fn set_rows(&mut self, repo: &LockRepo, set: &[String]) -> Vec<Expected> {
        let mut by_row: BTreeMap<String, Expected> = BTreeMap::new();
        for reference in set {
            for expected in self.rows(repo, reference) {
                by_row.entry(expected.row.to_string()).or_insert(expected);
            }
        }
        let mut rows: Vec<Expected> = by_row.into_values().collect();
        rows.sort_by_key(|expected| {
            *self
                .order
                .get(&expected.row.to_string())
                .unwrap_or_else(|| panic!("premise: plain verify has {}", expected.row))
        });
        rows
    }
}

/// A shown node's set as its JSON names it: its ID (none for a document
/// without one), then its `sections` in source order.
fn node_set(node: &Value) -> Vec<String> {
    let mut set: Vec<String> = node["id"].as_str().map(str::to_owned).into_iter().collect();
    set.extend(
        list(&node["sections"])
            .iter()
            .map(|section| text(section).to_owned()),
    );
    set
}

/// The REF `verify --node` takes for `id` of a node in `path`: a
/// feature-scoped ID (`scope = "feature"`) as `<feature>/<ID>`, the
/// feature its document's file stem; any other ID as is.
fn reference_of(root: &Path, path: &str, id: &str) -> String {
    let config = ProjectConfig::from_toml(
        &fs::read_to_string(root.join("specengine.toml")).expect("the config"),
    )
    .expect("the config parses");
    let prefix = id.split('-').next().unwrap_or(id);
    let scoped = config
        .scheme
        .prefix(prefix)
        .is_some_and(|spec| spec.scope == specengine_model::IdScope::Feature);
    if scoped {
        let stem = Path::new(path)
            .file_stem()
            .and_then(OsStr::to_str)
            .expect("a feature document's stem");
        format!("{stem}/{id}")
    } else {
        id.to_owned()
    }
}

/// The parser's set of a shown node: the ID nodes of its file whose span
/// starts within the node's lines (a document: every ID node of the file).
fn parsed_set(root: &Path, node: &Value) -> Vec<String> {
    let path = text(&node["path"]);
    let config = ProjectConfig::from_toml(
        &fs::read_to_string(root.join("specengine.toml")).expect("the config"),
    )
    .expect("the config parses");
    let bytes = fs::read(root.join(path)).expect("the holder's file");
    let parsed = specengine_core::parse(path, &bytes, &config.scheme);
    let first = node["line"].as_u64().expect("line");
    let last = node["end_line"].as_u64().expect("end_line");
    let line_of = |offset: usize| -> u64 {
        1 + bytes[..offset]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count() as u64
    };
    parsed
        .nodes
        .iter()
        .filter(|parsed| {
            let start = line_of(parsed.span.range().start);
            parsed.id.is_some() && start >= first && start <= last
        })
        .filter_map(|parsed| parsed.id.clone())
        .collect()
}

/// A verify row as the binding row it must be: its key, `sync` its
/// `state`, `category`, `detail`, `anchor`.
fn judged_view(row: &Value) -> Value {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
        "sync": row["state"],
        "category": row["category"],
        "detail": row["detail"],
        "anchor": row["anchor"],
    })
}

/// A shown row's judged part (what [`judged_view`] makes of verify's).
fn shown_view(row: &Value) -> Value {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
        "sync": row["sync"],
        "category": row["category"],
        "detail": row["detail"],
        "anchor": row["anchor"],
    })
}

/// Every stored symbol of `slug`'s code index under `home`, by (path,
/// qpath): `(item_kind, start_line, end_line, signature, layer)`. Read
/// after a read refreshed it, never refreshing it.
fn stored_symbols(home: &Path, slug: &str) -> BTreeMap<(String, String), Vec<[Value; 5]>> {
    let db = code_index_common::db(home, slug);
    let mut out: BTreeMap<(String, String), Vec<[Value; 5]>> = BTreeMap::new();
    for row in code_index_common::symbols(&db) {
        out.entry((
            text(&row["path"]).to_owned(),
            text(&row["qpath"]).to_owned(),
        ))
        .or_default()
        .push([
            row["item_kind"].clone(),
            row["start_line"].clone(),
            row["end_line"].clone(),
            row["signature"].clone(),
            row["layer"].clone(),
        ]);
    }
    out
}

/// The four located fields a row must carry ("The binding row"): an
/// `item` key's stored symbol's (`line`, `end_line`, `signature`,
/// `layer`), all `null` with none; a `ron` key its first marker's line
/// (`markers` as verify gives them), the rest `null`; any other key (a
/// module, a keyless row) all `null`.
fn located_view(
    row: &Value,
    markers: &Value,
    stored: &BTreeMap<(String, String), Vec<[Value; 5]>>,
) -> Vec<Value> {
    match text(&row["kind"]) {
        "item" => {
            let symbols = stored
                .get(&(
                    text(&row["file"]).to_owned(),
                    text(&row["symbol"]).to_owned(),
                ))
                .map(|found| {
                    found
                        .iter()
                        .filter(|symbol| symbol[0] == row["item_kind"])
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            match symbols.as_slice() {
                [] => vec![Value::Null; 4],
                [one] => one[1..].to_vec(),
                several => {
                    // The same file in two worktrees of one data
                    // directory, or twins under one qpath: one of them.
                    let mine = shown_located(row);
                    assert!(
                        several.iter().any(|symbol| symbol[1..] == mine[..]),
                        "{row}: one of the stored symbols {several:?}"
                    );
                    mine
                }
            }
        }
        "ron" => vec![
            markers
                .as_array()
                .and_then(|markers| markers.first())
                .map_or(Value::Null, |marker| marker["line"].clone()),
            Value::Null,
            Value::Null,
            Value::Null,
        ],
        _ => vec![Value::Null; 4],
    }
}

/// A row's four located fields as shown.
fn shown_located(row: &Value) -> Vec<Value> {
    vec![
        row["line"].clone(),
        row["end_line"].clone(),
        row["signature"].clone(),
        row["layer"].clone(),
    ]
}

/// The class "States" gives a state (`verify_common::class_of`'s words);
/// `None` for `ok`.
fn class_of(state: &str) -> Option<&'static str> {
    match verify_common::class_of(state) {
        Value::String(class) if class == "fix" => Some("fix"),
        Value::String(class) if class == "check" => Some("check"),
        Value::String(class) if class == "cannot" => Some("cannot"),
        _ => None,
    }
}

/// A row's lines as "Text" prints them: the row line, `    sig …` when
/// it has a signature, then `under` (verify's `moved from` and `candidate`
/// lines) four spaces in.
fn row_lines(row: &Value, binding: &str, under: &[String]) -> Vec<String> {
    let file = text(&row["file"]);
    let place = match (row["line"].as_u64(), row["end_line"].as_u64()) {
        (Some(line), Some(end)) => format!("{file}:{line}-{end}"),
        (Some(line), None) => format!("{file}:{line}"),
        _ => file.to_owned(),
    };
    let layer = row["layer"].as_str().unwrap_or("-");
    let mut sync = row["sync"].as_str().unwrap_or("-").to_owned();
    for part in ["category", "detail"] {
        if let Some(word) = row[part].as_str() {
            sync.push(' ');
            sync.push_str(word);
        }
    }
    let mut lines = vec![format!(
        "  {} | {} | {place} | {binding} | layer {layer} | sync {sync}\n",
        text(&row["node"]),
        text(&row["relation"])
    )];
    if let Some(signature) = row["signature"].as_str() {
        lines.push(format!("    sig {signature}\n"));
    }
    lines.extend(under.iter().map(|line| format!("    {line}\n")));
    lines
}

/// A block's lines in groups, one per row (its line and the lines under
/// it, four spaces in); the closing line, if printed, left out.
fn row_groups(block: &[String]) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    for line in block {
        if line.starts_with("    ") {
            groups
                .last_mut()
                .expect("a row above a line under it")
                .push(line.clone());
        } else if !line.starts_with("  bindings ") {
            groups.push(vec![line.clone()]);
        }
    }
    groups
}

/// A row's group as "Text" prints it: a keyed row's `<binding>`
/// [`binding_of`]; a keyless one's as verify words it (the marker's
/// words), read from the printed line and checked not empty.
fn expected_group(row: &Value, printed: &[String], under: &[String]) -> Vec<String> {
    if row["kind"].is_string() {
        return row_lines(row, &binding_of(row), under);
    }
    let head = format!(
        "  {} | {} | {} | ",
        text(&row["node"]),
        text(&row["relation"]),
        text(&row["file"])
    );
    let line = printed.first().map(String::as_str).unwrap_or_default();
    let binding = line
        .strip_prefix(&head)
        .and_then(|rest| rest.split(" | layer ").next())
        .unwrap_or_default();
    assert!(
        !binding.trim().is_empty(),
        "a keyless row's binding words: {line:?} for {row}"
    );
    row_lines(row, binding, under)
}

/// verify's `<binding>` text of a keyed row: `<kind> <symbol>[ <item_kind>]`.
fn binding_of(row: &Value) -> String {
    let mut binding = format!("{} {}", text(&row["kind"]), text(&row["symbol"]));
    if let Some(item_kind) = row["item_kind"].as_str() {
        binding.push(' ');
        binding.push_str(item_kind);
    }
    binding
}

/// The closing line of a block: `  bindings <n>[, <f> fix][, <c> check][,
/// <k> cannot]` over the rows' classes; a listed block's `  bindings <n>,
/// sync -`.
fn closing_line(rows: &[Value], listed: bool) -> String {
    if listed {
        return format!("  bindings {}, sync -\n", rows.len());
    }
    let mut line = format!("  bindings {}", rows.len());
    for class in ["fix", "check", "cannot"] {
        let n = rows
            .iter()
            .filter(|row| class_of(text(&row["sync"])) == Some(class))
            .count();
        if n > 0 {
            line.push_str(&format!(", {n} {class}"));
        }
    }
    line.push('\n');
    line
}

/// The spec part of a `--json` document: each node's `bindings` and the
/// read's `bindings: ` note set aside ("Degrade": "The spec part").
fn spec_part(document: &Value) -> Value {
    let mut document = document.clone();
    if let Some(nodes) = document["nodes"].as_array_mut() {
        for node in nodes {
            if let Some(object) = node.as_object_mut() {
                object.remove("bindings");
            }
        }
    }
    if let Some(notes) = document["notes"].as_array_mut() {
        notes.retain(|note| !text(note).starts_with("bindings: "));
    }
    document
}

/// `spec show REF` without `--bindings`: each node's `bindings` `null`,
/// its last key, after `links` (19 keys): docs/features/show-bindings.md
/// "spec show --bindings".
fn assert_unbound_document(run: &Show) {
    for node in run.nodes() {
        assert!(
            node.as_object()
                .is_some_and(|object| object.contains_key("bindings") && object.len() == 19),
            "{}: `bindings` present, 19 keys\n{}",
            run.what,
            run.show()
        );
        assert_eq!(
            node["bindings"],
            Value::Null,
            "{}: `bindings` null",
            run.what
        );
    }
    let raw = run.json_text();
    if let Some(at) = raw.find("\"nodes\":[{") {
        let keys = raw_order(raw, at + "\"nodes\":[".len());
        assert_eq!(
            &keys[keys.len() - 2..],
            ["links", "bindings"],
            "{}: `bindings` after `links`",
            run.what
        );
    }
}

/// The lines `read` holds beyond `plain`'s, in runs: `read`'s text is
/// `plain`'s with runs of lines put in and nothing else changed (each
/// line of `plain` met in order). Each run with the index of the `plain`
/// line it comes before.
fn inserted_runs(read: &str, plain: &str) -> Vec<(usize, Vec<String>)> {
    let read: Vec<&str> = read.split_inclusive('\n').collect();
    let plain: Vec<&str> = plain.split_inclusive('\n').collect();
    let (mut at, mut runs) = (0usize, Vec::<(usize, Vec<String>)>::new());
    let mut open: Option<(usize, Vec<String>)> = None;
    for line in read {
        if at < plain.len()
            && line == plain[at]
            && open.as_ref().is_none_or(|(_, lines)| {
                // A block ends with its closing line.
                lines
                    .last()
                    .is_some_and(|last| last.starts_with("  bindings "))
            })
        {
            if let Some(run) = open.take() {
                runs.push(run);
            }
            at += 1;
        } else {
            open.get_or_insert_with(|| (at, Vec::new()))
                .1
                .push(line.to_owned());
        }
    }
    if let Some(run) = open.take() {
        runs.push(run);
    }
    assert_eq!(
        at,
        plain.len(),
        "every line of `spec show REF` is kept, in order"
    );
    runs
}

/// One read's spec part is `spec show REF`'s: the text with its bindings
/// blocks taken out ([`inserted_runs`]: each block a run of lines put in
/// right after a header line or its links block, none changed), the JSON
/// with the keys and the note set aside, the exit; stderr but for the
/// read's own `note: bindings:` line. The blocks, in node order.
fn assert_spec_part(run: &Show, plain: &Show) -> Vec<Vec<String>> {
    assert_eq!(run.code(), plain.code(), "{}: the exit", run.what);
    let runs = inserted_runs(run.text(), plain.text());
    let plain_lines: Vec<&str> = plain.text().split_inclusive('\n').collect();
    for (at, lines) in &runs {
        assert!(
            lines.iter().all(|line| line.starts_with("  ")),
            "{}: a block's lines are indented: {lines:#?}",
            run.what
        );
        let above = plain_lines[at - 1];
        assert!(
            above.contains(" | span b3:") || above.starts_with("  "),
            "{}: a block right after its header line or links block, not after {above:?}",
            run.what
        );
    }
    assert_eq!(
        spec_part(&run.json()),
        spec_part(&plain.json()),
        "{}: the JSON but `bindings` and its note is show's",
        run.what
    );
    let stderr = |show: &Show| -> Vec<String> {
        show.text
            .stderr
            .lines()
            .filter(|line| !line.starts_with("note: bindings: "))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(stderr(run), stderr(plain), "{}: stderr", run.what);
    runs.into_iter().map(|(_, lines)| lines).collect()
}

/// Every row's keys, in "Data"'s order (the raw document's first row too).
fn assert_row_keys(run: &Show) {
    for row in run.all_rows() {
        let mut keys: Vec<&str> = row
            .as_object()
            .expect("a row object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut wanted = ROW_KEYS.to_vec();
        wanted.sort_unstable();
        assert_eq!(keys, wanted, "{}: a row's 14 keys: {row}", run.what);
    }
    let raw = run.json_text();
    if let Some(at) = raw.find("\"rows\":[{") {
        let open = at + "\"rows\":[".len();
        assert_eq!(raw_order(raw, open), ROW_KEYS, "{}: key order", run.what);
    }
}

/// The keys of the JSON object opening at byte `open` of `raw`, in the
/// order printed (depth 1 of that object only; `verify_output.rs`').
fn raw_order(raw: &str, open: usize) -> Vec<String> {
    let bytes = raw.as_bytes();
    assert_eq!(bytes[open], b'{', "an object opens at {open}");
    let (mut depth, mut in_string, mut escaped, mut start) = (0usize, false, false, 0);
    let mut keys = Vec::new();
    for (at, &byte) in bytes.iter().enumerate().skip(open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                if depth == 1 && bytes.get(at + 1) == Some(&b':') {
                    keys.push(raw[start..at].to_owned());
                }
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                start = at + 1;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
    keys
}

/// RD-1 and TE-1 over one state: each REF's read against `verify --node`
/// of every node of each shown node's set (its ID and its sections'),
/// row for row in verify's order ([`judged_view`], [`located_view`]); the
/// block's text the rows' ([`row_lines`], [`closing_line`]); each set the
/// parser's; the spec part `spec show REF`'s; `omitted` 0. Every shown
/// row, for the corpora's coverage.
fn differential(repo: &LockRepo, slug: &str, references: &[&str]) -> Vec<Value> {
    let mut oracle = Oracle::new(repo);
    let mut kept = Vec::new();
    let mut reads = Vec::new();
    for reference in references {
        let run = bound(repo, reference);
        run.answered();
        assert_eq!(
            run.notes()
                .iter()
                .filter(|note| note.starts_with("note: bindings: "))
                .count(),
            0,
            "{}: a sound state, no bindings note\n{}",
            run.what,
            run.show()
        );
        assert_row_keys(&run);
        let plain = unbound(repo, reference);
        assert_unbound_document(&plain);
        let blocks = assert_spec_part(&run, &plain);
        assert_eq!(
            blocks.len(),
            run.nodes().len(),
            "{}: a block per node\n{}",
            run.what,
            run.show()
        );
        reads.push((run, blocks));
    }
    let stored = stored_symbols(&repo.home, slug);
    for (run, blocks) in &reads {
        for (at, node) in run.nodes().iter().enumerate() {
            let set = node_set(node);
            assert_eq!(
                set,
                parsed_set(&repo.root, node),
                "{}: node {at}'s set is its ID and its sections",
                run.what
            );
            let references: Vec<String> = set
                .iter()
                .map(|id| reference_of(&repo.root, text(&node["path"]), id))
                .collect();
            let want = oracle.set_rows(repo, &references);
            let got = run.rows(at);
            assert_eq!(run.omitted(at), 0, "{}: uncut", run.what);
            let shown: Vec<Value> = got.iter().map(shown_view).collect();
            let judged: Vec<Value> = want
                .iter()
                .map(|expected| judged_view(&expected.row))
                .collect();
            assert_eq!(
                shown,
                judged,
                "{}: node {at} ({set:?}): rows as verify --node judges them, in its order\n{}",
                run.what,
                rows_diff(&judged, &shown)
            );
            let mut lines = Vec::new();
            for (row, expected) in got.iter().zip(&want) {
                assert_eq!(
                    shown_located(row),
                    located_view(row, &expected.row["markers"], &stored),
                    "{}: {row}: located as the code index stores it",
                    run.what
                );
                if row["kind"].is_string() {
                    assert_eq!(
                        expected.binding,
                        binding_of(row),
                        "premise: verify's binding text"
                    );
                }
                lines.extend(row_lines(row, &expected.binding, &expected.under));
            }
            lines.push(closing_line(&got, false));
            assert_eq!(
                blocks[at], lines,
                "{}: node {at}'s block as \"Text\" prints its rows",
                run.what
            );
            kept.extend(got);
        }
    }
    kept
}

fn sorted(rows: &[Value]) -> Vec<String> {
    let mut texts: Vec<String> = rows.iter().map(Value::to_string).collect();
    texts.sort();
    texts
}

/// What `got` lacks of `want` and holds beyond it.
fn rows_diff(want: &[Value], got: &[Value]) -> String {
    let want = sorted(want);
    let got = sorted(got);
    let missing: Vec<&String> = want.iter().filter(|row| !got.contains(row)).collect();
    let extra: Vec<&String> = got.iter().filter(|row| !want.contains(row)).collect();
    format!("missing: {missing:#?}\nextra: {extra:#?}")
}

// ----------------------------------------------------------------- edits

/// `verify_node.rs`' `drift()` over the golden: `CAP` and the RON cap
/// edited, LAW-BOUND's text edited, PIECE-COUNTER's `//!` marker in the
/// test file removed, a non-UTF-8 file, plain `enforce-introduced`.
fn drift(repo: &LockRepo) {
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    repo.replace(BOUNDS, "//! @verifies PIECE-COUNTER\n", "");
    repo.write("crates/tally/src/odd.rs", b"// \xff\n".as_slice());
    set_mode(&repo.root, "enforce-introduced");
}

/// The two files of a cross-file pair (`symbol-names.md` "Ambiguity").
const PAIR_FILES: [&str; 2] = ["crates/tally/src/pair.rs", "crates/tally/src/pair/mod.rs"];

/// `verify_node.rs`' `halves()`: GOAL-01's marker written as a look-alike
/// (its entry left lock-only, `marker_gone`), a look-alike of LAW-BOUND
/// on a new item, LAW-BOUND's Display impl removed (a lock-only
/// `symbol_gone` row), an alias marker (`AIM-01`, keyed GOAL-01), a typo
/// (`LAW-BUOND`: dangling, no node's), a cross-file `duplicate` twin.
fn halves(repo: &LockRepo) {
    repo.replace(
        COUNTER,
        "@implements AIM-01@1",
        &format!("@implements {LOOK_ALIKE}@1"),
    );
    repo.replace(
        COUNTER,
        "impl fmt::Display for Tally {\n    // @implements PIECE-COUNTER#LAW-BOUND@2\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        write!(f, \"{}\", self.count)\n    }\n}\n",
        "",
    );
    repo.replace(
        COUNTER,
        "pub fn total(",
        "// @implements LAW-BUOND\npub fn total(",
    );
    repo.append(
        COUNTER,
        &format!(
            "\n// @implements AIM-01\npub fn spare() -> u32 {{\n    1\n}}\n\n// @implements {LAW_LOOK_ALIKE}@2\npub fn ceiling() -> u32 {{\n    CAP\n}}\n"
        ),
    );
    repo.write(PAIR_FILES[0], "// @implements LAW-STEP\npub fn twin() {}\n");
    repo.write(PAIR_FILES[1], "pub fn twin() {}\n");
}

/// Markers on PIECE-LIMITS' nested sections (LAW-SOURCE holds
/// LAW-DEFAULT) and on the document, in a new file: one row each, so a
/// section's read holds its section's row and never its parent's.
const NESTED: &str = "crates/tally/src/nested.rs";

fn nested(repo: &LockRepo) {
    repo.write(
        NESTED,
        "// @implements PIECE-LIMITS\npub fn limits() -> u32 {\n    10\n}\n\n// @implements LAW-SOURCE\npub fn source() -> u32 {\n    10\n}\n\n// @implements LAW-DEFAULT\npub fn fallback() -> u32 {\n    10\n}\n\n// LAW-BOUND is cited here only: a mention, never a row.\npub fn cited() -> u32 {\n    10\n}\n",
    );
}

/// Restores a path's mode when dropped (a failing test leaves no
/// unreadable path in its scratch).
struct Mode {
    path: PathBuf,
    mode: u32,
}

impl Mode {
    /// `path` set to `mode` until dropped.
    fn set(path: PathBuf, mode: u32) -> Self {
        let old = fs::metadata(&path).expect("the path").permissions().mode() & 0o7777;
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("chmod");
        Self { path, mode: old }
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(self.mode));
    }
}

// ------------------------------------------------------------- the corpora

/// code-a's REFs: every node by one or more forms (an ID, an alias,
/// `ID#SECTION`, a document path, `slug/ID`, a document without an ID).
const GOLDEN_REFS: [&str; 17] = [
    "PIECE-COUNTER",
    COUNTER_MD,
    "LAW-BOUND",
    "PIECE-COUNTER#LAW-BOUND",
    "LAW-STEP",
    "LAW-ODD",
    "GOAL-01",
    "AIM-01",
    "docs/records/GOAL-01.md",
    "PREM-001",
    "cap-check/PASS-01",
    "docs/features/cap-check.md",
    "PIECE-LIMITS",
    "docs/spec/limits.md",
    "LAW-SOURCE",
    "LAW-DEFAULT",
    "PIECE-LIMITS#LAW-SOURCE",
];

/// TE-1 (a): the golden with [`drift`], [`halves`] and [`nested`]; then a
/// second state: a bound file deleted (`file_gone`), one not UTF-8
/// (`not_utf8`), one excluded from `[code]` (`outside_code`), a marker's
/// level list narrowed (`levels_changed`).
fn corpus_golden() -> Vec<Value> {
    let repo = golden("sb-te1-golden");
    drift(&repo);
    halves(&repo);
    nested(&repo);
    let mut kept = differential(&repo, CODE_A, &GOLDEN_REFS);

    fs::remove_file(repo.root.join(BOUNDS)).expect("the test file deleted");
    let mut bytes = fs::read(repo.root.join(LIB)).expect("lib.rs");
    bytes.extend_from_slice(b"// \xff\n");
    repo.write(LIB, bytes);
    repo.replace(
        "specengine.toml",
        "exclude = [\"crates/*/tests/fixtures/**\"]",
        "exclude = [\"crates/*/tests/fixtures/**\", \"data/limits.ron\"]",
    );
    repo.replace(
        COUNTER,
        "@implements PIECE-COUNTER@3 [sig, body]",
        "@implements PIECE-COUNTER@3 [body]",
    );
    kept.extend(differential(&repo, CODE_A, &GOLDEN_REFS));
    kept
}

/// TE-1 (b): `fixtures/anchor-states`, its baseline, then its 18
/// scenarios applied together: both documents (by ID and path), every
/// section, the records; `WISP-01` an alias of `WISP-02` after its
/// scenario, by both IDs.
fn corpus_anchor_states() -> Vec<Value> {
    let repo = anchor_common::base("sb-te1-anchors");
    let mut refs: Vec<&str> = anchor_common::NODES.to_vec();
    refs.extend([
        "KITE-DECOY",
        "KITE-FLIGHT",
        "KITE-RIGGING",
        "WISP-03",
        "WISP-04",
        "docs/spec/kites.md",
    ]);
    let mut kept = differential(&repo, ANCHORS, &refs);
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&repo, name);
    }
    let renamed: Vec<&str> = refs
        .iter()
        .map(|id| if *id == "WISP-01" { "WISP-02" } else { id })
        .chain(["WISP-01", "docs/spec/rigging.md"])
        .collect();
    let more = differential(&repo, ANCHORS, &renamed);
    assert!(
        more.iter()
            .any(|row| row["sync"] == "ok" && row["anchor"]["state"] == "moved"),
        "premise: a moved `ok` row"
    );
    kept.extend(more);
    kept
}

/// TE-1 (c): `fixtures/verify-tests` (no lock: marker rows): the document
/// by ID and path, each section, `CONE-01` and its legacy alias
/// `PYRO-01`, `glaze-check/GLZ-01`.
fn corpus_kiln() -> Vec<Value> {
    let repo = LockRepo::of("sb-te1-kiln", "verify-tests", "repo", &[]);
    differential(
        &repo,
        KILN,
        &[
            "KILN-FIRING",
            "docs/spec/kiln.md",
            "KILN-RAMP",
            "KILN-FIRING#KILN-HOLD",
            "KILN-COOL",
            "CONE-01",
            "PYRO-01",
            "CONE-02",
            "CONE-03",
            "glaze-check/GLZ-01",
            "docs/features/glaze-check.md",
        ],
    )
}

/// TE-1 (d): `verify_common::ecs` (an ECS game crate, a RON file) with a
/// body edited, a RON value edited and a marker raised past its node:
/// every node by ID, the documents by path too.
fn corpus_ecs() -> Vec<Value> {
    let repo = ecs("sb-te1-ecs");
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(MOVEMENT, "regen_per_second: 2.5,", "regen_per_second: 3.0,");
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@2\n",
    );
    differential(
        &repo,
        GAME,
        &[
            "PERK-STAMINA",
            "docs/spec/stamina.md",
            "PERK-REGEN",
            "PERK-DRAIN",
            "TUNE-01",
            "TUNE-02",
            "TUNE-03",
            "PERK-MOVE",
            "docs/spec/movement.md",
            "PERK-SPRINT",
            "PERK-SPRINT-COST",
            "PERK-WALK",
        ],
    )
}

/// One corpus: every row its reads show.
type Corpus = fn() -> Vec<Value>;

/// Runs each corpus on a thread of its own; every row they show.
fn every_corpus(corpora: &[(&str, Corpus)]) -> Vec<Value> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = corpora
            .iter()
            .map(|(name, corpus)| (*name, scope.spawn(corpus)))
            .collect();
        let mut kept = Vec::new();
        let mut failed = Vec::new();
        for (name, handle) in handles {
            match handle.join() {
                Ok(rows) => kept.extend(rows),
                Err(_) => failed.push(name),
            }
        }
        assert!(failed.is_empty(), "corpora failed (above): {failed:?}");
        kept
    })
}

/// TE-1 (e): a walk gap anywhere (an unread file no node binds, then a
/// directory not listed) gives each read no row: `bindings` `null`, no
/// block, `verify --node`'s cause as the note; readable again, the rows.
fn corpus_walk_gap() -> Vec<Value> {
    let repo = ecs("sb-te1-gap");
    let spare = "crates/game/src/spare.rs";
    repo.write(spare, "pub fn spare() {}\n");
    repo.write("crates/game/src/deep/inner.rs", "pub fn inner() {}\n");
    repo.commit_all("unbound code");
    let vars = vars_of(&repo);
    {
        let _mode = Mode::set(repo.root.join(spare), 0o000);
        for warmth in ["cold", "warm"] {
            assert_degraded(
                &repo,
                &repo.root,
                "PERK-REGEN",
                &vars,
                Degraded::NoRows,
                &format!("an unreadable file, {warmth}"),
            );
        }
    }
    {
        let _mode = Mode::set(repo.root.join("crates/game/src/deep"), 0o000);
        assert_degraded(
            &repo,
            &repo.root,
            "PERK-STAMINA",
            &vars,
            Degraded::NoRows,
            "a directory not listed",
        );
    }
    let kept = differential(&repo, GAME, &["PERK-REGEN", "PERK-STAMINA"]);
    assert!(!kept.is_empty(), "readable again: the rows");
    kept
}

/// TE-1: every ID'd node of each corpus (code-a's golden twice over with
/// a `duplicate` twin, the 18 anchor scenarios, the kiln library without a
/// lock, the ECS game with RON data, a walk gap), by every REF form, each
/// corpus on a thread of its own: each shown node's rows equal the oracle
/// of RD-1 ([`differential`]). Across them every state, anchor state and
/// the categories and details the corpora give are met, an item row with
/// its signature and a RON row with its marker's line only.
#[test]
fn show_bindings_equals_verify_node_on_every_corpus() {
    let kept = every_corpus(&[
        ("golden", corpus_golden),
        ("anchor-states", corpus_anchor_states),
        ("verify-tests", corpus_kiln),
        ("verify-ecs", corpus_ecs),
        ("a walk gap", corpus_walk_gap),
    ]);
    assert_eq!(
        values_of(&kept, |row| &row["sync"]),
        owned(&STATES),
        "every state shown"
    );
    assert_eq!(
        values_of(&kept, |row| &row["anchor"]["state"]),
        owned(&ANCHOR_STATES),
        "every anchor state shown"
    );
    let categories = values_of(&kept, |row| &row["category"]);
    for category in [
        "item_error",
        "ambiguous_symbol",
        "rev_unknown",
        "unreadable",
        "hidden_by_error",
        "homoglyph",
        "bad_rev",
        "unanchored",
        "bad_levels",
        "file_gone",
        "symbol_gone",
        "marker_gone",
        "marker_behind",
        "code_changed",
        "levels_changed",
    ] {
        assert!(
            categories.contains(category),
            "{category} shown: {categories:?}"
        );
    }
    let details = values_of(&kept, |row| &row["detail"]);
    for detail in ["duplicate", "not_utf8", "outside_code"] {
        assert!(details.contains(detail), "{detail} shown: {details:?}");
    }
    for kind in ["item", "module", "ron"] {
        assert!(
            kept.iter().any(|row| row["kind"] == kind),
            "a {kind} row shown"
        );
    }
    assert!(
        kept.iter().any(|row| row["kind"].is_null()),
        "a keyless row shown"
    );
    assert!(
        kept.iter().any(|row| row["kind"] == "item"
            && row["line"].is_u64()
            && row["end_line"].is_u64()
            && row["signature"].is_string()
            && row["layer"].is_string()),
        "an item row located with its signature"
    );
    assert!(
        kept.iter().any(|row| row["kind"] == "ron"
            && row["line"].is_u64()
            && row["end_line"].is_null()
            && row["signature"].is_null()),
        "a RON row with its marker's line only"
    );
}

// ------------------------------------------------------------------ RD-1

/// The digest every tampered level column holds: no recipe makes it.
fn fake_digest() -> String {
    format!("b3:{}", "e".repeat(64))
}

/// RD-1: on `verify_common::golden` with [`drift`], `LAW-BOUND`,
/// `LAW-STEP`, `PIECE-COUNTER` and `docs/spec/counter.md`: each shown
/// node's rows are, key for key and in verify's order, the union of
/// `verify --node` over the node and its sections, `sync` its `state`,
/// with `category`, `detail` and `anchor` ([`differential`]: the block's
/// text, the located fields, the spec part too). Then indexed, every
/// stored level digest tampered in the scratch data directory: the reads
/// still equal `verify --node`, an `ok` row compared by `ast` among them,
/// and the tampered columns survive the reads' no-op refresh. M1: the
/// index's level digest columns read. Variant: the state taken from the
/// marker's `@rev` (LAW-BOUND's `spec_ahead` rows turn).
#[test]
fn show_bindings_sync_equals_verify_node() {
    let repo = golden("sb-parity");
    drift(&repo);
    let refs = ["LAW-BOUND", "LAW-STEP", "PIECE-COUNTER", COUNTER_MD];
    let kept = differential(&repo, CODE_A, &refs);
    assert!(
        kept.iter()
            .any(|row| row["node"] == "LAW-BOUND" && row["sync"] == "spec_ahead"),
        "premise: a row whose state differs from its marker's revision's\n{kept:#?}"
    );

    index(&repo);
    let db = code_index_common::db(&repo.home, CODE_A);
    let fake = fake_digest();
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
    let before = tampered();
    assert!(before > 0, "premise: digests to tamper");
    let kept = differential(&repo, CODE_A, &refs);
    let by_ast = ["PIECE-COUNTER", "LAW-BOUND", "LAW-STEP"]
        .iter()
        .flat_map(|id| list(&verify_node(&repo, id)["rows"]))
        .any(|row| row["compared"] == "ast" && row["state"] == "ok");
    assert!(
        by_ast,
        "premise: an `ok` row compared by `ast`, which a stored digest would turn"
    );
    assert!(kept.iter().any(|row| row["sync"] == "ok"), "{kept:#?}");
    assert_eq!(
        tampered(),
        before,
        "premise: the reads' no-op refresh kept the tampered columns"
    );
}

// ------------------------------------------------------------------ RD-2

/// The nodes a read's rows name (`node` as keyed).
fn nodes_of(rows: &[Value]) -> BTreeSet<String> {
    rows.iter()
        .map(|row| text(&row["node"]).to_owned())
        .collect()
}

/// RD-2 over the golden with [`drift`], [`halves`] and [`nested`]: a
/// read holds its holders' rows and its contained sections', nothing
/// else. GOAL-01 (by ID, alias and path) keeps its lock-only
/// `marker_gone` row, its alias marker's row and its look-alike's;
/// LAW-BOUND its lock-only `symbol_gone` row and its look-alike's, never
/// PIECE-COUNTER's (its parent) or LAW-STEP's (a sibling), never the
/// comment that only cites it (a mention `spec refs` lists); PIECE-COUNTER
/// holds its sections' rows; LAW-SOURCE holds LAW-DEFAULT's, LAW-DEFAULT
/// never LAW-SOURCE's or PIECE-LIMITS'; two holders of `PASS-01` (a
/// second feature defines it) list their own rows each. Every read
/// equals `verify --node` of its set ([`differential`]). M2: a lock-only
/// row dropped. Variant: a parent's row kept.
#[test]
fn show_bindings_rows_are_own_and_sections() {
    let repo = golden("sb-own");
    drift(&repo);
    halves(&repo);
    nested(&repo);
    differential(&repo, CODE_A, &GOLDEN_REFS);

    // The premise: the comment in `nested.rs` is a mention of LAW-BOUND.
    let refs = repo.spec_piped(&repo.root, &["refs", "LAW-BOUND", "--json"]);
    let refs = refs.json();
    assert!(
        refs.to_string().contains(NESTED),
        "premise: refs lists the mention in {NESTED}: {refs}"
    );
    let lock_only = |id: &str, category: &str| -> Value {
        let rows = list(&verify_node(&repo, id)["rows"]);
        let row = rows
            .iter()
            .find(|row| {
                row["markers"] == json!([]) && !row["lock"].is_null() && row["category"] == category
            })
            .unwrap_or_else(|| panic!("premise: {id}'s lock-only {category} row: {rows:#?}"));
        judged_view(row)
    };

    let goal = bound(&repo, "GOAL-01");
    let rows = goal.rows(0);
    let shown: Vec<Value> = rows.iter().map(shown_view).collect();
    assert!(
        shown.contains(&lock_only("GOAL-01", "marker_gone")),
        "GOAL-01's lock-only row\n{}",
        goal.show()
    );
    assert!(
        rows.iter()
            .any(|row| row["symbol"] == "crates/tally::counter::spare"
                && row["node"] == "GOAL-01"
                && row["sync"] == "unlocked"),
        "the alias marker's row, keyed GOAL-01\n{}",
        goal.show()
    );
    assert!(
        rows.iter()
            .any(|row| row["node"] == LOOK_ALIKE && row["category"] == "homoglyph"),
        "the look-alike's row\n{}",
        goal.show()
    );
    for other in ["AIM-01", "docs/records/GOAL-01.md"] {
        assert_eq!(
            bound(&repo, other).json()["nodes"][0]["bindings"],
            goal.json()["nodes"][0]["bindings"],
            "{other}: GOAL-01's rows"
        );
    }

    let law = bound(&repo, "LAW-BOUND");
    let rows = law.rows(0);
    let shown: Vec<Value> = rows.iter().map(shown_view).collect();
    assert!(
        shown.contains(&lock_only("LAW-BOUND", "symbol_gone")),
        "LAW-BOUND's lock-only row\n{}",
        law.show()
    );
    assert!(
        rows.iter()
            .any(|row| row["node"] == LAW_LOOK_ALIKE && row["category"] == "homoglyph"),
        "LAW-BOUND's look-alike row\n{}",
        law.show()
    );
    assert_eq!(
        nodes_of(&rows),
        owned(&["LAW-BOUND", LAW_LOOK_ALIKE]),
        "never the parent's or a sibling's\n{}",
        law.show()
    );
    assert!(
        rows.iter().all(|row| row["file"] != NESTED),
        "never the mention\n{}",
        law.show()
    );

    let piece = bound(&repo, "PIECE-COUNTER");
    let nodes = nodes_of(&piece.rows(0));
    for id in [
        "PIECE-COUNTER",
        "LAW-BOUND",
        "LAW-STEP",
        "LAW-ODD",
        LAW_LOOK_ALIKE,
    ] {
        assert!(
            nodes.contains(id),
            "the document holds {id}'s rows: {nodes:?}\n{}",
            piece.show()
        );
    }
    // Two holders: a second feature defines `PASS-01`; the bare REF shows
    // both, each listing its own rows only.
    repo.write(
        "docs/features/cap-again.md",
        "---\nclass: spec\nstatus: draft\nscope: [crates/tally]\n---\n\n# Cap again\n\n## Acceptance criteria\n\n### The cap holds twice over {#PASS-01}\n\nStep the count twenty times from zero: it reads 10.\n",
    );
    repo.append(
        BOUNDS,
        "\n// @verifies cap-again/PASS-01\n#[test]\nfn holds_twice_over() {}\n",
    );
    differential(
        &repo,
        CODE_A,
        &["PASS-01", "cap-again/PASS-01", "cap-check/PASS-01"],
    );
    let two = bound(&repo, "PASS-01");
    assert_eq!(two.nodes().len(), 2, "two holders\n{}", two.show());
    for (at, path, own) in [
        (0, "docs/features/cap-again.md", "cap-again/PASS-01"),
        (1, "docs/features/cap-check.md", "cap-check/PASS-01"),
    ] {
        assert_eq!(two.nodes()[at]["path"], path, "holders by path");
        assert_eq!(
            nodes_of(&two.rows(at)),
            owned(&[own]),
            "{path} lists its own rows\n{}",
            two.show()
        );
    }

    for (reference, wanted) in [
        (
            "PIECE-LIMITS",
            &["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"][..],
        ),
        (
            "docs/spec/limits.md",
            &["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"],
        ),
        ("LAW-SOURCE", &["LAW-SOURCE", "LAW-DEFAULT"]),
        ("PIECE-LIMITS#LAW-SOURCE", &["LAW-SOURCE", "LAW-DEFAULT"]),
        ("LAW-DEFAULT", &["LAW-DEFAULT"]),
    ] {
        let read = bound(&repo, reference);
        assert_eq!(
            nodes_of(&read.rows(0)),
            owned(wanted),
            "{reference}: its own and its sections' rows only\n{}",
            read.show()
        );
    }
}

// ------------------------------------------------------------------ RD-3

/// The keys of `rows`, sorted.
fn keys_of(rows: &[Value]) -> Vec<String> {
    let mut keys: Vec<String> = rows.iter().map(key_of).collect();
    keys.sort();
    keys
}

/// The keys a listing gives for judged `rows` ("Degrade": no pairing): each
/// row's key, and a moved row's entry's key (`anchor.from`) beside it.
fn listed_keys(rows: &[Value]) -> Vec<Value> {
    let mut keys = Vec::new();
    for row in rows {
        keys.push(row.clone());
        if row["anchor"]["state"] == "moved" {
            let from = &row["anchor"]["from"];
            keys.push(json!({
                "node": row["node"],
                "relation": row["relation"],
                "file": from["file"],
                "kind": from["kind"],
                "symbol": from["symbol"],
                "item_kind": from["item_kind"],
            }));
        }
    }
    keys
}

/// The first run cause `verify --node ID` names in `cwd` with `vars`.
fn first_cause(cwd: &Path, id: &str, vars: &[(OsString, OsString)]) -> String {
    let document = judged_with(cwd, id, vars).document;
    let causes = list(&document["cannot"]);
    assert!(
        !causes.is_empty(),
        "premise: verify --node {id} has a run cause here: {document}"
    );
    text(&causes[0]).to_owned()
}

/// What a degraded read shows.
enum Degraded<'a> {
    /// No row: `bindings` `null`, no block (a `[code]` error, a walk gap).
    NoRows,
    /// These keys listed, never judged (a lock or git cause).
    Listed(&'a [Value]),
}

/// RD-3's one read: `spec show ID --bindings` from `cwd` with `vars`:
/// exit 0, the spec part `spec show ID`'s in the same place, one note
/// `bindings: <cause>` (`verify --node`'s first `cannot:` line there)
/// after show's own on stderr and in the JSON's `notes`; then
/// [`Degraded`]'s `bindings`: `null` with no block, or the keys listed
/// with `sync`, `category`, `detail` and `anchor` `null` (never `ok`),
/// located as the index stores them, the block closed `bindings <n>, sync
/// -`.
fn assert_degraded(
    repo: &LockRepo,
    cwd: &Path,
    id: &str,
    vars: &[(OsString, OsString)],
    shows: Degraded<'_>,
    context: &str,
) {
    let cause = first_cause(cwd, id, vars);
    let read = show_with(cwd, &[id, "--bindings"], vars);
    let plain = show_with(cwd, &[id], vars);
    assert_eq!(read.code(), 0, "{context}: exit 0\n{}", read.show());
    let blocks = assert_spec_part(&read, &plain);
    let note = format!("note: bindings: {cause}");
    assert_eq!(
        read.text.stderr.lines().last(),
        Some(note.as_str()),
        "{context}: the note, after show's own\n{}",
        read.show()
    );
    assert_eq!(
        read.notes()
            .iter()
            .filter(|line| line.starts_with("note: bindings: "))
            .count(),
        1,
        "{context}: one note\n{}",
        read.show()
    );
    let notes = list(&read.json()["notes"]);
    assert_eq!(
        notes.last(),
        Some(&json!(format!("bindings: {cause}"))),
        "{context}: the JSON's last note"
    );
    match shows {
        Degraded::NoRows => {
            for at in 0..read.nodes().len() {
                assert_eq!(
                    read.bindings(at),
                    Value::Null,
                    "{context}: no row, `bindings` null\n{}",
                    read.show()
                );
            }
            assert_eq!(blocks, Vec::<Vec<String>>::new(), "{context}: no block");
        }
        Degraded::Listed(keys) => {
            let rows = read.all_rows();
            assert_eq!(
                keys_of(&rows),
                keys_of(keys),
                "{context}: the keys listed\n{}",
                read.show()
            );
            for row in &rows {
                for unjudged in ["sync", "category", "detail", "anchor"] {
                    assert_eq!(
                        row[unjudged],
                        Value::Null,
                        "{context}: {unjudged} null, never judged: {row}"
                    );
                }
            }
            let stored = stored_symbols(&repo.home, &slug_of(cwd));
            for row in &rows {
                if row["kind"] == "item" {
                    assert_eq!(
                        shown_located(row),
                        located_view(row, &json!([]), &stored),
                        "{context}: {row} located"
                    );
                }
            }
            assert_eq!(
                blocks.len(),
                read.nodes().len(),
                "{context}: a block per node"
            );
            for (at, block) in blocks.iter().enumerate() {
                let rows = read.rows(at);
                let groups = row_groups(block);
                assert_eq!(groups.len(), rows.len(), "{context}: a group per row");
                let mut lines: Vec<String> = Vec::new();
                for (row, printed) in rows.iter().zip(&groups) {
                    lines.extend(expected_group(row, printed, &[]));
                }
                lines.push(closing_line(&rows, true));
                assert_eq!(block, &lines, "{context}: node {at}'s listed block");
            }
        }
    }
}

/// `[project] slug` of the config under `root`.
fn slug_of(root: &Path) -> String {
    let config = fs::read_to_string(root.join("specengine.toml")).expect("the config");
    config
        .lines()
        .find_map(|line| line.trim().strip_prefix("slug"))
        .and_then(|rest| rest.split('"').nth(1))
        .expect("a slug")
        .to_owned()
}

/// A `git` that fails every call, first on `PATH`.
fn failing_git(dir: &Path) -> OsString {
    fs::create_dir_all(dir).expect("the shim's directory");
    let shim = dir.join("git");
    fs::write(
        &shim,
        "#!/bin/sh\necho 'fatal: refused by the test' >&2\nexit 128\n",
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
    dir.as_os_str().to_owned()
}

/// A copy of `base` in a merge whose `spec.lock` conflicts
/// (`verify_modes.rs`'): both branches changed one entry's `verified_at`.
fn unmerged_copy(base: &LockRepo, dir: &str) -> PathBuf {
    let root = clone_dir(base, dir);
    let git = |args: &[&str]| base.git.git(&root, args);
    git(&["checkout", "-q", "-b", "side"]);
    let entry = "verified_at = \"2026-10-08\"\n\n[[binding]]\nnode = \"PERK-REGEN\"";
    let lock = fs::read_to_string(root.join("spec.lock")).expect("the lock");
    assert!(lock.contains(entry), "premise: the entry");
    fs::write(
        root.join("spec.lock"),
        lock.replace(entry, &entry.replace("2026-10-08", "2026-10-09")),
    )
    .expect("side's lock");
    base.git.add_all(&root);
    base.git.commit(&root, "side lock");
    git(&["checkout", "-q", "main"]);
    fs::write(
        root.join("spec.lock"),
        lock.replace(entry, &entry.replace("2026-10-08", "2026-10-07")),
    )
    .expect("main's lock");
    base.git.add_all(&root);
    base.git.commit(&root, "main lock");
    let merge = base
        .git
        .git_output(&root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: spec.lock conflicts");
    root
}

/// RD-3: a read never fails on the code layer. Over the golden with
/// [`drift`]: a refused `[code]` key, an unknown `frameworks` name and a
/// walk gap (an unread file no node binds) show no row; no git on `PATH`,
/// a git that fails, a root in no worktree, a lock not UTF-8 and a
/// format-1 lock list the keys, never judged; over the ECS game, an
/// unmerged `spec.lock` lists the markers' keys. Each read exit 0, its
/// spec part `spec show`'s, one `note: bindings: <cause>`, the cause
/// `verify --node`'s first `cannot:` line in that place. A bad `[verify]
/// mode`, which `verify --node` refuses, changes nothing: the rows judged
/// as with a good one, no note. M: exit 2 propagated; a `[verify]` error
/// degrading; an unjudged row shown `ok`.
#[test]
fn show_bindings_survive_a_broken_code_layer() {
    let repo = golden("sb-degrade");
    drift(&repo);
    let vars = vars_of(&repo);
    let root = repo.root.clone();
    let config = repo.read("specengine.toml");
    let ids = ["LAW-BOUND", "PIECE-COUNTER", "GOAL-01"];
    let healthy: BTreeMap<&str, Vec<Value>> = ids
        .into_iter()
        .map(|id| (id, list(&verify_node(&repo, id)["rows"])))
        .collect();
    assert!(
        healthy.values().all(|rows| !rows.is_empty()),
        "premise: rows to list"
    );
    // A read lists a node's rows and its sections': PIECE-COUNTER's set.
    let set_rows = |judged: &BTreeMap<&str, Vec<Value>>, id: &str, extra: &[Value]| -> Vec<Value> {
        let mut rows = judged[id].clone();
        rows.extend(extra.iter().cloned());
        rows
    };
    let sections: Vec<Value> = ["LAW-BOUND", "LAW-STEP", "LAW-ODD"]
        .iter()
        .flat_map(|id| list(&verify_node(&repo, id)["rows"]))
        .collect();
    let listed: BTreeMap<&str, Vec<Value>> = ids
        .into_iter()
        .map(|id| {
            let extra = if id == "PIECE-COUNTER" {
                sections.clone()
            } else {
                Vec::new()
            };
            (id, listed_keys(&set_rows(&healthy, id, &extra)))
        })
        .collect();

    // A bad `[verify] mode`: never read by a read.
    repo.replace(
        "specengine.toml",
        "mode = \"enforce-introduced\"",
        "mode = \"strict\"",
    );
    assert_ne!(
        repo.read("specengine.toml"),
        config,
        "premise: the mode edited"
    );
    for id in ["LAW-BOUND", "GOAL-01"] {
        let read = bound(&repo, id);
        read.answered();
        assert_eq!(
            read.notes(),
            Vec::<String>::new(),
            "a bad [verify]: no note\n{}",
            read.show()
        );
        let shown: Vec<Value> = read.rows(0).iter().map(shown_view).collect();
        let judged: Vec<Value> = healthy[id].iter().map(judged_view).collect();
        assert_eq!(shown, judged, "a bad [verify]: {id}'s rows judged");
    }
    repo.write("specengine.toml", &config);

    // No row: a refused `[code]` key, an unknown framework, a walk gap.
    let roots = "roots   = [\"crates\", \"data\", \"tools/loose.rs\"]";
    for (edit, context) in [
        (
            format!("{roots}\nlanguages = [\"rust\"]"),
            "a refused [code] key",
        ),
        (
            format!("{roots}\nframeworks = [\"no-such-framework\"]"),
            "an unknown framework",
        ),
    ] {
        repo.replace("specengine.toml", roots, &edit);
        for id in ids {
            assert_degraded(&repo, &root, id, &vars, Degraded::NoRows, context);
        }
        repo.write("specengine.toml", &config);
    }
    let spare = "crates/tally/src/spare.rs";
    repo.write(spare, "pub fn spare_part() {}\n");
    {
        let _mode = Mode::set(repo.root.join(spare), 0o000);
        for id in ids {
            assert_degraded(&repo, &root, id, &vars, Degraded::NoRows, "a walk gap");
        }
    }
    fs::remove_file(repo.root.join(spare)).expect("the spare file");

    // Listed, never judged: git missing or failing, no worktree.
    let nowhere = repo.scratch.dir("no-git-here");
    let no_git = with_var(&vars, "PATH", nowhere.as_os_str());
    let bad_git = with_var(&vars, "PATH", &failing_git(&repo.scratch.join("bad-git")));
    for (vars, context) in [(&no_git, "no git"), (&bad_git, "a git failure")] {
        for (id, keys) in &listed {
            assert_degraded(&repo, &root, id, vars, Degraded::Listed(keys), context);
        }
    }
    let loose = repo.scratch.join("loose");
    copy_dir(&repo.root, &loose);
    fs::remove_dir_all(loose.join(".git")).expect("no .git");
    let loose = fs::canonicalize(&loose).expect("the loose copy");
    for (id, keys) in &listed {
        assert_degraded(
            &repo,
            &loose,
            id,
            &vars,
            Degraded::Listed(keys),
            "no worktree",
        );
    }

    // A lock not UTF-8: it does not parse, so the markers' keys alone.
    let lock = repo.lock().expect("the lock");
    let bare = clone_dir(&repo, "bare");
    fs::remove_file(bare.join("spec.lock")).expect("no lock");
    let markers_only: BTreeMap<&str, Vec<Value>> = ids
        .into_iter()
        .map(|id| {
            let mut rows = list(&judged_with(&bare, id, &vars).document["rows"]);
            if id == "PIECE-COUNTER" {
                for section in ["LAW-BOUND", "LAW-STEP", "LAW-ODD"] {
                    rows.extend(list(&judged_with(&bare, section, &vars).document["rows"]));
                }
            }
            (id, rows)
        })
        .collect();
    repo.write("spec.lock", b"\xff\xfe".as_slice());
    for (id, keys) in &markers_only {
        assert_degraded(
            &repo,
            &root,
            id,
            &vars,
            Degraded::Listed(keys),
            "a lock not UTF-8",
        );
    }
    repo.write("spec.lock", lock);

    // A format-1 lock (the golden before lock-levels): its bytes parse, so
    // its entries' keys too, as the format-2 golden's rows key them.
    let old = golden_format_1("sb-degrade-format-1");
    drift(&old);
    let old_vars = vars_of(&old);
    for (id, keys) in &listed {
        assert_degraded(
            &old,
            &old.root,
            id,
            &old_vars,
            Degraded::Listed(keys),
            "a format-1 lock",
        );
    }

    // An unmerged `spec.lock` over the ECS game: the markers' keys.
    let game = ecs("sb-degrade-unmerged");
    let game_vars = vars_of(&game);
    let bare = clone_dir(&game, "bare");
    fs::remove_file(bare.join("spec.lock")).expect("no lock");
    let merged = unmerged_copy(&game, "unmerged");
    for id in ["PERK-REGEN", "TUNE-01"] {
        let rows = list(&judged_with(&bare, id, &game_vars).document["rows"]);
        assert!(!rows.is_empty(), "premise: {id}'s markers");
        assert_eq!(
            first_cause(&merged, id, &game_vars),
            "spec.lock is unmerged: run spec lock regen",
            "premise: cause 5"
        );
        assert_degraded(
            &game,
            &merged,
            id,
            &game_vars,
            Degraded::Listed(&rows),
            "an unmerged lock",
        );
    }
}

/// A `git` first on `PATH` that answers every call as the real one does,
/// but ends each `cat-file` session (verify's object session) with exit
/// 1: the session's end fails, and only that.
fn session_end_failing_git(repo: &LockRepo, dir: &Path) -> OsString {
    fs::create_dir_all(dir).expect("the shim's directory");
    let real = repo
        .git
        .git_program()
        .to_str()
        .expect("a UTF-8 git path")
        .to_owned();
    let shim = dir.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\ncase \" $* \" in *\" cat-file \"*) '{real}' \"$@\"; exit 1;; esac\nexec '{real}' \"$@\"\n"
        ),
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
    repo.git.path_with(dir)
}

/// docs/features/show-bindings.md "Degrade", one note, the first cause
/// (iteration 2's (a)): git's session ending with a failure is a cause of
/// its own only when none is listed yet, as `verify --node` counts it.
/// Over the golden with [`drift`] and a `git` whose object session exits
/// 1 at its end ([`session_end_failing_git`]): with a sound lock the
/// session's end is the run's only cause, `verify --node`'s first `cannot:`
/// line and the read's note, the keys listed; with a lock that is not
/// UTF-8 the lock's cause is both, never the session's end. M: the
/// session's end counted after a listed cause (the note names git).
#[test]
fn show_bindings_note_the_first_cause_not_the_session_end() {
    let repo = golden("sb-session-end");
    drift(&repo);
    let ids = ["LAW-BOUND", "GOAL-01"];
    let healthy: BTreeMap<&str, Vec<Value>> = ids
        .into_iter()
        .map(|id| (id, list(&verify_node(&repo, id)["rows"])))
        .collect();
    let bare = clone_dir(&repo, "bare");
    fs::remove_file(bare.join("spec.lock")).expect("no lock");
    let vars = vars_of(&repo);
    let markers_only: BTreeMap<&str, Vec<Value>> = ids
        .into_iter()
        .map(|id| (id, list(&judged_with(&bare, id, &vars).document["rows"])))
        .collect();
    let path = session_end_failing_git(&repo, &repo.scratch.join("session-end-git"));
    let shim = with_var(&vars, "PATH", &path);

    // A sound lock: the session's end is the cause.
    for id in ids {
        let cause = first_cause(&repo.root, id, &shim);
        assert!(
            !cause.starts_with("spec.lock"),
            "premise: the session's end fails, a git cause: {cause}"
        );
        assert_degraded(
            &repo,
            &repo.root,
            id,
            &shim,
            Degraded::Listed(&listed_keys(&healthy[id])),
            "the session's end alone",
        );
    }
    // A lock not UTF-8 listed first: its cause, never the session's end.
    repo.write("spec.lock", b"\xff\xfe".as_slice());
    for id in ids {
        assert_eq!(
            first_cause(&repo.root, id, &shim),
            "spec.lock: not UTF-8: SpecEngine writes it",
            "premise: verify --node names the lock's cause first"
        );
        assert_degraded(
            &repo,
            &repo.root,
            id,
            &shim,
            Degraded::Listed(&markers_only[id]),
            "a lock not UTF-8, then the session's end",
        );
    }
}

// ------------------------------------------------------------------ RD-5

/// RD-5 (the cap, `spec-cli-read.md` "The cap"): LAW-BOUND bound by 500
/// marked tests in one file (an `unlocked` row and its `sig` line each,
/// about 190 characters) passes 40 000 characters: the header, then whole
/// rows (each with the line under it) within the cap, no closing line,
/// then the tail ending `; bindings not shown: <k>]`; the JSON's `rows`
/// are the printed rows, the first of plain verify's LAW-BOUND rows in its
/// order, and `omitted` is k, the rest. M5: the JSON holds every row.
#[test]
fn show_bindings_cut_keeps_json_to_printed_rows() {
    let repo = golden("sb-cut");
    let mut many = String::new();
    for at in 0..500 {
        many.push_str(&format!(
            "// @verifies LAW-BOUND@2\n#[test]\nfn stays_within_the_cap_{at:04}() {{}}\n\n"
        ));
    }
    repo.write("crates/tally/tests/many.rs", many);
    let plain = repo.spec_piped(&repo.root, &["verify", "--json"]).json();
    let all: Vec<Value> = list(&plain["rows"])
        .into_iter()
        .filter(|row| row["node"] == "LAW-BOUND")
        .collect();
    assert!(all.len() > 500, "premise: LAW-BOUND's rows, uncut in plain");
    assert!(
        all.iter()
            .all(|row| !["moved", "ambiguous"].contains(&text(&row["anchor"]["state"]))),
        "premise: no line under a row but its `sig`"
    );

    let read = bound(&repo, "LAW-BOUND");
    read.answered();
    let lines: Vec<&str> = read.text().lines().collect();
    let tail = *lines.last().expect("a tail");
    let rows = read.rows(0);
    let shown = rows.len();
    let omitted = read.omitted(0);
    assert!(
        omitted > 0 && shown > 0,
        "the cut falls in the rows\n{}",
        read.show()
    );
    assert_eq!(
        shown as u64 + omitted,
        all.len() as u64,
        "printed and omitted are every row"
    );
    assert!(
        tail.starts_with("[truncated: ")
            && tail.ends_with(&format!("; bindings not shown: {omitted}]")),
        "the tail counts the rows not shown: {tail}"
    );
    let before_tail: usize = read.text()[..read.text().len() - tail.len() - 1]
        .chars()
        .count();
    assert!(
        before_tail <= CAP,
        "{before_tail} characters before the tail"
    );
    let judged: Vec<Value> = all[..shown].iter().map(judged_view).collect();
    let got: Vec<Value> = rows.iter().map(shown_view).collect();
    assert_eq!(got, judged, "the printed rows, in verify's order");
    let block: Vec<String> = read
        .text()
        .split_inclusive('\n')
        .skip(1)
        .take(lines.len() - 2)
        .map(str::to_owned)
        .collect();
    assert!(
        lines[0].contains(" | span b3:"),
        "the header first: {}",
        lines[0]
    );
    let groups = row_groups(&block);
    assert_eq!(groups.len(), shown, "one group per printed row");
    let mut printed: Vec<String> = Vec::new();
    for (row, group) in rows.iter().zip(&groups) {
        printed.extend(expected_group(row, group, &[]));
    }
    assert_eq!(block, printed, "whole rows only, no closing line");
}

// ------------------------------------------------------------------ RD-8

/// What a read must leave as it was (RD-8, the single door: it writes
/// only the data directory).
#[derive(Debug, PartialEq, Eq)]
struct Untouched {
    /// Every byte under the worktree, `.git` too.
    files: BTreeMap<String, Option<Vec<u8>>>,
    /// `git status --porcelain=v1 -z --untracked-files=all`, read with
    /// `--no-optional-locks` (the harness never refreshes the index).
    status: Vec<u8>,
    /// `.git/index`'s mtime (seconds, nanoseconds).
    index_mtime: Option<(i64, i64)>,
    /// The number of objects in `.git/objects`.
    objects: usize,
    lock: Option<Vec<u8>>,
    /// The slug's queue dump (proposals, tasks, runs, events) and its
    /// events' count.
    queue: Vec<String>,
}

impl Untouched {
    fn take(repo: &LockRepo, slug: &str) -> Self {
        let status = status(repo);
        Self {
            index_mtime: index_mtime(&repo.root),
            objects: objects(&repo.root),
            files: snapshot(&repo.root),
            status,
            lock: repo.lock(),
            queue: queue_state(repo, slug),
        }
    }

    fn still(&self, repo: &LockRepo, slug: &str, context: &str) {
        assert_eq!(
            index_mtime(&repo.root),
            self.index_mtime,
            "{context}: .git/index's mtime"
        );
        assert_eq!(objects(&repo.root), self.objects, "{context}: git objects");
        let files = snapshot(&repo.root);
        let changed: Vec<&String> = self
            .files
            .keys()
            .chain(files.keys())
            .filter(|path| self.files.get(*path) != files.get(*path))
            .collect();
        assert!(
            changed.is_empty(),
            "{context} wrote under the worktree: {changed:?}"
        );
        assert_eq!(repo.lock(), self.lock, "{context}: spec.lock's bytes");
        assert_eq!(queue_state(repo, slug), self.queue, "{context}: the queue");
        assert_eq!(status(repo), self.status, "{context}: git status");
    }
}

/// `git status --porcelain=v1 -z --untracked-files=all` without the
/// optional index refresh.
fn status(repo: &LockRepo) -> Vec<u8> {
    repo.git(&[
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ])
}

/// `.git/index`'s mtime, `None` when there is none.
fn index_mtime(root: &Path) -> Option<(i64, i64)> {
    fs::metadata(root.join(".git").join("index"))
        .ok()
        .map(|meta| (meta.mtime(), meta.mtime_nsec()))
}

/// The files under `.git/objects` (loose objects and packs).
fn objects(root: &Path) -> usize {
    snapshot(&root.join(".git").join("objects"))
        .values()
        .filter(|bytes| bytes.is_some())
        .count()
}

/// The slug's queue dump, line by line, and its events' count (no
/// database: nothing).
fn queue_state(repo: &LockRepo, slug: &str) -> Vec<String> {
    let db = data_dir(&repo.home).join(format!("{slug}.db"));
    if !db.exists() {
        return Vec::new();
    }
    let queue = SqliteQueue::open(&db, slug).expect("the queue opens");
    let mut state: Vec<String> = queue
        .dump()
        .expect("the dump")
        .lines()
        .map(str::to_owned)
        .collect();
    let events = queue.events().expect("the events").len();
    if !state.is_empty() || events > 0 {
        state.push(format!("events {events}"));
    }
    state
}

/// `fixtures/code-a/expected.json`'s "generated" files written under the
/// root: a second holder of `GOAL-03`, its ambiguous marker, a
/// look-alike of PIECE-COUNTER, a RON file past the depth cap.
fn generated(repo: &LockRepo) {
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture("code-a").join("expected.json")).expect("code-a's expected"),
    )
    .expect("code-a's expected.json is JSON");
    for file in list(&expected["generated"]["files"]) {
        let mut body = String::new();
        for part in list(&file["parts"]) {
            let times = usize::try_from(part["times"].as_u64().expect("times")).expect("usize");
            body.push_str(&text(&part["text"]).repeat(times));
        }
        repo.write(text(&file["path"]), body);
    }
}

/// RD-8: a git repository of `code-a` with the golden lock and a task in
/// the queue, after [`drift`]: `show --bindings` with rows, with none,
/// with two holders, `--links` too, naming nothing (an unknown ID, a
/// look-alike), and plain `show`: `.git/index`'s mtime, the objects'
/// count, `git status`, `spec.lock`'s bytes, every byte under the
/// worktree, the queue's rows and events all as before; each
/// byte-identical from a subdirectory and repeated; no absolute path
/// printed; then a degraded read (no git), the same. M8: `git status` or
/// `git diff` run by the read (they refresh `.git/index`).
#[test]
fn show_bindings_writes_nothing_under_the_root() {
    let repo = golden("sb-door");
    generated(&repo);
    repo.commit_all("a second holder");
    new_task(&repo, &["LAW-BOUND"]);
    drift(&repo);
    let before = Untouched::take(&repo, CODE_A);
    assert!(!before.queue.is_empty(), "premise: a task in the queue");
    let calls: [&[&str]; 7] = [
        &["LAW-BOUND", "--bindings"],
        &["PIECE-COUNTER", "--bindings", "--links"],
        &["PIECE-LIMITS", "--bindings"],
        &["GOAL-03", "--bindings"],
        &["GOAL-99", "--bindings"],
        &[LOOK_ALIKE, "--bindings"],
        &["LAW-BOUND"],
    ];
    for args in calls {
        let run = show_at(&repo, &repo.root, args);
        before.still(&repo, CODE_A, &run.what);
        let deeper = show_at(&repo, &repo.root.join("crates").join("tally"), args);
        before.still(&repo, CODE_A, &deeper.what);
        assert_eq!(deeper.bytes(), run.bytes(), "{args:?} from a subdirectory");
        let again = show_at(&repo, &repo.root, args);
        before.still(&repo, CODE_A, &again.what);
        assert_eq!(again.bytes(), run.bytes(), "{args:?} repeated");
        let root = repo.root.display().to_string();
        let scratch = repo.scratch.path().display().to_string();
        for printed in [run.text(), run.json_text(), &run.text.stderr] {
            assert!(
                !printed.contains(&root) && !printed.contains(&scratch),
                "{args:?}: no absolute path printed"
            );
        }
    }
    // A degraded read (no git on `PATH`): the keys listed, nothing written.
    let no_git = with_var(
        &vars_of(&repo),
        "PATH",
        repo.scratch.dir("no-git-here").as_os_str(),
    );
    let degraded = show_with(&repo.root, &["LAW-BOUND", "--bindings"], &no_git);
    assert_eq!(degraded.code(), 0, "{}", degraded.show());
    assert!(
        degraded.rows(0).iter().all(|row| row["sync"].is_null()),
        "premise: a listed read\n{}",
        degraded.show()
    );
    before.still(&repo, CODE_A, "a degraded read");
    assert_eq!(
        repo.lock().as_deref(),
        Some(fs::read(golden_file()).expect("the golden").as_slice()),
        "spec.lock is the golden's bytes"
    );
}

/// The direct dependency names of a manifest's `[dependencies]`,
/// `[dev-dependencies]`, `[build-dependencies]` and
/// `[workspace.dependencies]` tables.
fn dependencies(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = matches!(
                line,
                "[dependencies]"
                    | "[dev-dependencies]"
                    | "[build-dependencies]"
                    | "[workspace.dependencies]"
            );
            continue;
        }
        if inside
            && !line.starts_with('#')
            && let Some((name, _)) = line.split_once('=')
        {
            let name = name.trim();
            out.insert(name.strip_suffix(".workspace").unwrap_or(name).to_owned());
        }
    }
    out
}

/// The `name = "..."` of every `[[package]]` of a `Cargo.lock`.
fn locked_packages(lock: &str) -> BTreeSet<String> {
    lock.lines()
        .filter_map(|line| line.strip_prefix("name = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .collect()
}

/// `git <args>` in this repository, read-only, its stdout; `None` when it
/// fails.
fn repository_git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .current_dir(repository_root())
        .args(["--no-optional-locks"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).expect("git prints UTF-8"))
}

/// RD-8: the slice adds no dependency. Against `main` (else `HEAD`): every
/// workspace manifest (the root's and each crate's) adds no direct
/// dependency, no crate is added, and `Cargo.lock` names no new package
/// (a later slice may remove one: docs/features/review-form.md drops the
/// MCP server's `getrandom`). M: a crate or a dependency added.
#[test]
fn no_workspace_dependency_is_added_against_main() {
    let base = ["main", "HEAD"]
        .into_iter()
        .find(|name| {
            repository_git(&[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{name}^{{commit}}"),
            ])
            .is_some()
        })
        .expect("a main branch or a HEAD to compare against");
    let root = repository_root();
    let crates: BTreeSet<String> = fs::read_dir(root.join("crates"))
        .expect("crates/")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| root.join("crates").join(name).join("Cargo.toml").is_file())
        .collect();
    let at_base: BTreeSet<String> = repository_git(&["ls-tree", "--name-only", base, "crates/"])
        .expect("ls-tree")
        .lines()
        .filter_map(|path| path.strip_prefix("crates/"))
        .map(str::to_owned)
        .collect();
    assert_eq!(crates, at_base, "no crate added or removed against {base}");
    let mut manifests = vec!["Cargo.toml".to_owned()];
    manifests.extend(
        crates
            .iter()
            .map(|name| format!("crates/{name}/Cargo.toml")),
    );
    for manifest in manifests {
        let now = fs::read_to_string(root.join(&manifest)).expect("the manifest");
        let then = repository_git(&["show", &format!("{base}:{manifest}")])
            .unwrap_or_else(|| panic!("{manifest} at {base}"));
        let then = dependencies(&then);
        let added: Vec<String> = dependencies(&now).difference(&then).cloned().collect();
        assert!(
            added.is_empty(),
            "{manifest}: dependencies added against {base}: {added:?}"
        );
    }
    let lock_now = fs::read_to_string(root.join("Cargo.lock")).expect("Cargo.lock");
    let lock_then = repository_git(&["show", &format!("{base}:Cargo.lock")]).expect("the lock");
    let then = locked_packages(&lock_then);
    let added: Vec<String> = locked_packages(&lock_now)
        .difference(&then)
        .cloned()
        .collect();
    assert!(
        added.is_empty(),
        "Cargo.lock's packages added against {base}: {added:?}"
    );
}

// ------------------------------------------------------------------ TE-2

/// Copies `from` (the worktree and `.git`) to `to`, every entry created in
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

/// The golden at `<scratch>/<dir>` (`verify_common::golden`'s commits),
/// with [`drift`] and [`halves`].
fn golden_at(label: &str, dir: &str) -> LockRepo {
    let repo = LockRepo::of(label, "code-a", dir, &[]);
    repo.write(
        "spec.lock",
        fs::read(golden_file()).expect("fixtures/spec-lock/code-a.lock"),
    );
    repo.commit_all("the lock");
    drift(&repo);
    halves(&repo);
    repo
}

/// The code files of a worktree (`.rs`, `.ron`), `.git` aside.
fn code_paths(root: &Path) -> Vec<String> {
    snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some()
                && !path.starts_with(".git")
                && (path.ends_with(".rs") || path.ends_with(".ron"))
        })
        .map(|(path, _)| path)
        .collect()
}

/// TE-2: over the golden with [`drift`] and [`halves`], each REF's read
/// (text and JSON, stderr, exit) is byte-identical repeated, from another
/// root (`elsewhere/deeper`, its own `HOME`), from another cwd, from a
/// copy made in reverse creation order and from a checkout of every
/// committed code file CRLF under `core.autocrlf=true`. M: rows in walk or
/// rowid order; a stored absolute path; a signature or a line read with
/// its CR.
#[test]
fn show_bindings_are_byte_identical_across_roots_order_and_crlf() {
    let first = golden_at("sb-det-a", "repo");
    let second = golden_at("sb-det-b", "elsewhere/deeper");
    assert_eq!(second.head(), first.head(), "the same commits");
    let reversed = first.scratch.join("reversed");
    copy_reversed(&first.root, &reversed);
    let reversed = fs::canonicalize(&reversed).expect("the reversed copy");

    let crlf = clone_dir(&first, "crlf");
    first.git.git(&crlf, &["config", "core.autocrlf", "true"]);
    let committed: BTreeSet<String> = first
        .git_text(&["ls-files"])
        .lines()
        .map(str::to_owned)
        .collect();
    let dirty: BTreeSet<String> = status(&first)
        .split(|byte| *byte == 0)
        .filter(|entry| entry.len() > 3)
        .map(|entry| String::from_utf8_lossy(&entry[3..]).into_owned())
        .collect();
    let mut crlf_files = 0;
    for path in code_paths(&crlf) {
        if !committed.contains(&path) || dirty.contains(&path) {
            continue;
        }
        fs::remove_file(crlf.join(&path)).expect("remove");
        first.git.git(&crlf, &["checkout", "--", &path]);
        if fs::read(crlf.join(&path))
            .expect("checked out")
            .windows(2)
            .any(|pair| pair == b"\r\n")
        {
            crlf_files += 1;
        }
    }
    assert!(crlf_files > 0, "premise: CRLF code files in the worktree");

    let vars = vars_of(&first);
    for reference in [
        "PIECE-COUNTER",
        "LAW-BOUND",
        "GOAL-01",
        "PIECE-LIMITS",
        "cap-check/PASS-01",
    ] {
        let args = [reference, "--bindings"];
        let reference_run = show_at(&first, &first.root, &args);
        reference_run.answered();
        assert!(
            !reference_run.all_rows().is_empty() || reference == "PIECE-LIMITS",
            "premise: {reference}'s rows"
        );
        let runs = [
            ("a repeat run", show_at(&first, &first.root, &args)),
            ("another root", show_at(&second, &second.root, &args)),
            (
                "another cwd",
                show_at(&second, &second.root.join("crates").join("tally"), &args),
            ),
            (
                "a reverse creation order",
                show_with(&reversed, &args, &vars),
            ),
            ("a CRLF checkout", show_with(&crlf, &args, &vars)),
        ];
        for (context, run) in runs {
            assert_eq!(
                run.bytes(),
                reference_run.bytes(),
                "{reference}: {context}\n{}\n{}",
                run.show(),
                reference_run.show()
            );
        }
    }
}

// ------------------------------------------------------------------ TE-3

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// TE-3, manual (`#[ignore]`), release: `code-a` with the golden lock and
/// a crate of 1 000 unrelated `.rs` files, the indexes warm: `show
/// LAW-BOUND --bindings` against plain `spec verify`, the medians of five
/// warm runs printed (`--no-capture`) for the spec's "Implementation";
/// the read still `verify --node`'s, and below plain verify.
///
/// `cargo nextest run -p specengine-cli --release --test show_bindings
/// show_bindings_cost --run-ignored ignored-only --no-capture`
#[test]
#[ignore = "manual (TE-3): release, figures recorded in the spec"]
fn show_bindings_cost() {
    let repo = golden("sb-cost");
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
    index(&repo);
    let kept = differential(&repo, CODE_A, &["LAW-BOUND"]);
    assert!(!kept.is_empty(), "premise: LAW-BOUND's rows");
    let time = |args: &[&str]| {
        median(|| {
            let started = Instant::now();
            let run = repo.spec_piped(&repo.root, args);
            let took = started.elapsed();
            assert!(!run.stdout.is_empty(), "{}", run.show());
            took
        })
    };
    let whole = time(&["verify"]);
    let node = time(&["verify", "--node", "LAW-BOUND"]);
    let read = time(&["show", "LAW-BOUND", "--bindings"]);
    let plain = time(&["show", "LAW-BOUND"]);
    eprintln!(
        "TE-3: code-a + 1 000 unrelated .rs files, warm indexes, median of 5: plain verify {} ms, verify --node LAW-BOUND {} ms, show LAW-BOUND --bindings {} ms, show LAW-BOUND {} ms",
        whole.as_millis(),
        node.as_millis(),
        read.as_millis(),
        plain.as_millis()
    );
    assert!(
        read < whole,
        "show --bindings {read:?} below plain verify {whole:?}"
    );
}

// ------------------------------------------------------------------ TE-4

/// A copy of `fixture` without `[code]`: for every ID of its
/// `expected.json`, `show ID --bindings` prints `show ID`'s stdout byte
/// for byte, exit 0, every node's `bindings` `null` (19 keys), and one note
/// naming the `[code]` sentence, on stderr after show's own and last in
/// the JSON's `notes`; `show ID` keeps `bindings` `null`.
fn assert_no_code_is_show(fixture_name: &str) {
    let scratch = common::Scratch::new(&format!("sb-no-code-{fixture_name}"));
    let root = scratch.copy(fixture_name, fixture_name);
    let home = scratch.home("h");
    assert!(
        !fs::read_to_string(root.join("specengine.toml"))
            .expect("the config")
            .contains("[code]"),
        "premise: {fixture_name} has no [code]"
    );
    let vars: Vec<(OsString, OsString)> = vec![("HOME".into(), home.into_os_string())];
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture(fixture_name).join("expected.json")).expect("expected.json"),
    )
    .expect("JSON");
    let mut ids: Vec<String> = Vec::new();
    for file in expected["files"].as_object().expect("files").values() {
        ids.extend(file["id"].as_str().map(str::to_owned));
        for section in list(&file["sections"]) {
            ids.extend(section["id"].as_str().map(str::to_owned));
        }
    }
    assert!(ids.len() > 3, "premise: {fixture_name}'s IDs: {ids:?}");
    for id in &ids {
        let read = show_with(&root, &[id, "--bindings"], &vars);
        let plain = show_with(&root, &[id], &vars);
        let context = format!("{fixture_name} {id}");
        assert_eq!(read.code(), plain.code(), "{context}: the exit");
        if plain.code() != 0 {
            // An ID several nodes hold or the parser refuses is show's own
            // answer, the code layer never read.
            assert_eq!(read.bytes(), plain.bytes(), "{context}: show's refusal");
            continue;
        }
        assert_eq!(read.text(), plain.text(), "{context}: the text unchanged");
        assert_unbound_document(&read);
        assert_unbound_document(&plain);
        assert_eq!(
            spec_part(&read.json()),
            spec_part(&plain.json()),
            "{context}: the JSON but the note"
        );
        assert_eq!(
            read.text.stderr.lines().last(),
            Some(no_code_note().as_str()),
            "{context}: the [code] note, after show's own\n{}",
            read.show()
        );
        assert_eq!(
            read.notes()
                .iter()
                .filter(|note| note.starts_with("note: bindings: "))
                .count(),
            1,
            "{context}: one note"
        );
        assert_eq!(
            list(&read.json()["notes"]).last(),
            Some(&json!(no_code_note().trim_start_matches("note: "))),
            "{context}: the JSON's last note"
        );
        assert!(
            !plain.text.stderr.contains("bindings"),
            "{context}: show without the flag never names bindings"
        );
    }
}

/// RD-4's last leg: `spec-a` (a game design) and `spec-b` (another
/// project's conventions, a Cyrillic legacy alias), no `[code]`: `show
/// --bindings` is `show` with the `[code]` note ([`assert_no_code_is_show`]).
/// M: an empty block or a heading printed; the read refused.
#[test]
fn show_bindings_without_code_is_show() {
    for fixture_name in ["spec-a", "spec-b"] {
        assert_no_code_is_show(fixture_name);
    }
}

/// docs/features/show-bindings.md "Degrade", cause (2) (iteration 2's
/// (c)): without `[code]` a `--bindings` read is `spec show REF`'s read and
/// one note. Over spec-a with a second holder of `AC-07` (show's
/// warning), a REF written with `@rev` (show's note), a document, `--links`
/// too: stdout byte for byte `spec show`'s, stderr `spec show`'s with the
/// one note line after it, the JSON `spec show`'s with the note last in
/// `notes`; a REF naming nothing and a look-alike: `spec show`'s answer
/// exactly, no note (show's own exits come first). The data directory,
/// warm from `spec show`, is byte-identical after the read: the code layer
/// writes nothing (opening the shared database is not observable apart
/// from `spec show`'s own). M: the note missing; the corpus read with
/// bytes for `--bindings` (no difference outside a race: see the report).
#[test]
fn show_bindings_without_code_read_as_show_and_the_note() {
    let scratch = common::Scratch::new("sb-no-code-read");
    let root = scratch.copy("spec-a", "a");
    write_feature(&root);
    let home = scratch.home("h");
    let vars: Vec<(OsString, OsString)> = vec![("HOME".into(), home.clone().into_os_string())];
    let note = no_code_note();
    let cases: [&[&str]; 9] = [
        &["MEC-STAMINA"],
        &["MEC-STAMINA", "--links"],
        &["MEC-STAMINA#RULE-STAM-REGEN@3"],
        &["AC-07"],
        &["AC-07", "--links"],
        &["docs/spec/movement/stamina.md"],
        &["MEC-NOPE"],
        &["M\u{0415}C-STAMINA"],
        &["other:MEC-STAMINA"],
    ];
    for args in cases {
        let plain = show_with(&root, args, &vars);
        let data = snapshot(&data_dir(&home));
        let read = show_with(&root, &[args, &["--bindings"]].concat(), &vars);
        assert_eq!(
            snapshot(&data_dir(&home)),
            data,
            "{args:?}: the read wrote nothing to the data directory"
        );
        if plain.code() != 0 {
            assert_eq!(
                read.bytes(),
                plain.bytes(),
                "{args:?}: show's own answer, no note"
            );
            continue;
        }
        assert_eq!(read.code(), 0, "{}", read.show());
        assert_eq!(read.text(), plain.text(), "{args:?}: stdout");
        assert_eq!(
            read.text.stderr,
            format!("{}{note}\n", plain.text.stderr),
            "{args:?}: stderr is show's and the one note"
        );
        let mut want = plain.json();
        want["notes"]
            .as_array_mut()
            .expect("notes")
            .push(json!(note.trim_start_matches("note: ")));
        assert_eq!(
            read.json(),
            want,
            "{args:?}: the JSON is show's and the note"
        );
        if args.contains(&"AC-07") || args[0].contains('@') {
            assert!(
                !plain.text.stderr.is_empty(),
                "premise: {args:?} has show's own lines on stderr"
            );
        }
    }
}

/// A second holder of spec-a's `AC-07` (`mcp_read.rs`' feature).
fn write_feature(root: &Path) {
    common::write(
        root,
        "docs/features/lantern-fuel.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Lantern fuel\n\n## Acceptance criteria\n\n\
         ### Fuel lasts the night {#AC-07}\n\nMeasured.\n",
    );
}

/// TE-4: `fixtures/spec-c` (a non-Rust project, no `[code]`) is unchanged
/// the same way ([`assert_no_code_is_show`]).
#[test]
fn show_bindings_leave_spec_c_unchanged() {
    assert_no_code_is_show("spec-c");
}

// ------------------------------------------------- docs/features/tested-reads.md

/// The characters of `text` before its line starting `start`, if any.
fn chars_before_line(text: &str, start: &str) -> Option<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with(start) {
            return Some(at);
        }
        at += line.chars().count();
    }
    None
}

/// docs/features/tested-reads.md RD-5 **M5** (the closing line printed
/// without its tested lines, or they without it): over the evidence kiln,
/// KILN-COOL bound by many marked tests in one file (KILN-HOLD's and
/// KILN-RAMP's blocks `verified`), `show KILN-FIRING --bindings` with the
/// last test's name grown one character at a time so the end of the rows
/// walks across 40 000: the text before the tail never passes 40 000; the
/// closing line and its two `  tested | …` lines print together or not at
/// all; the JSON's `tested` holds the printed entries (plain verify's, or
/// `[]` once they are cut); at the first cut the closing line alone would
/// still have fitted (the rows' count found from row 2's cost, fewer until
/// all print with the unit at pad 0). (`View::Browser`, the daemon's uncut
/// view: the HTTP half.)
#[test]
fn show_cut_keeps_tested_with_closing() {
    let kiln = tested_kiln::evidence_kiln("sb-tested-cut");
    let repo = &kiln.repo;
    let many = "crates/kiln/tests/many.rs";
    let write = |count: usize, pad: usize| {
        let mut text = String::new();
        for at in 0..count {
            let extra = if at + 1 == count {
                "x".repeat(pad)
            } else {
                String::new()
            };
            text.push_str(&format!(
                "// @verifies KILN-COOL@2\n#[test]\nfn cools_in_steps_{at:04}{extra}() {{}}\n\n"
            ));
        }
        repo.write(many, text);
    };
    let by_node = tested_common::entries_by_node(&tested_common::plain(repo, &[]));
    let want: Vec<Value> = ["KILN-HOLD", "KILN-RAMP"]
        .iter()
        .map(|node| by_node[*node].clone())
        .collect();
    assert!(
        want.iter().all(|entry| entry["state"] == "verified"),
        "premise: the evidence holds: {want:?}"
    );
    let unit: Vec<String> = want
        .iter()
        .map(|entry| format!("  {}", tested_common::entry_line(entry)))
        .collect();
    let closing_of =
        |read: &Show| -> Option<usize> { chars_before_line(read.text(), "  bindings ") };

    // The cost of one row (its line and its `sig` line), uncut.
    write(1, 0);
    let one = bound(repo, "KILN-FIRING");
    write(2, 0);
    let two = bound(repo, "KILN-FIRING");
    let (one_at, two_at) = (
        closing_of(&one).expect("uncut: the closing line"),
        closing_of(&two).expect("uncut: the closing line"),
    );
    let row = two_at - one_at;
    let unit_chars: usize = unit
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum::<usize>()
        + "  bindings 1000, 1000 check\n".len();
    // A row's cost grows with its line numbers' digits: an estimate from
    // row 2's, then fewer rows until every row and the closing unit print
    // at pad 0; the pad that brings the rows' end near the cut follows from
    // the room left (each pad character costs two: the row's symbol and
    // its `sig` line).
    let mut count = (CAP - (one_at - row) - unit_chars - 16) / row;
    assert!(count > 100, "premise: many rows: {count}");
    let room = loop {
        write(count, 0);
        let read = bound(repo, "KILN-FIRING");
        match closing_of(&read) {
            Some(at) if read.rows(0).len() == count + 7 => break CAP - at,
            _ => count -= 1,
        }
    };
    assert!(
        room >= unit_chars,
        "premise: the unit fits at pad 0: {room}"
    );
    let first_pad = (room - unit_chars).saturating_sub(16) / 2;

    let mut seen: Vec<(usize, bool)> = Vec::new();
    // From about 16 characters of room past the unit down to about 4: the
    // last row always fits.
    for pad in first_pad..=first_pad + (unit_chars + 12) / 2 {
        write(count, pad);
        let read = bound(repo, "KILN-FIRING");
        read.answered();
        let context = format!("{count} rows, pad {pad}");
        let lines: Vec<&str> = read.text().lines().collect();
        if let Some(tail) = lines.last().filter(|line| line.starts_with("[truncated: ")) {
            let before: usize = read.text()[..read.text().len() - tail.len() - 1]
                .chars()
                .count();
            assert!(
                before <= CAP,
                "{context}: {before} characters before the tail"
            );
        }
        let rows = read.rows(0);
        assert_eq!(rows.len(), count + 7, "{context}: every row printed");
        let tested_at: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.starts_with("  tested | "))
            .map(|(at, _)| at)
            .collect();
        let tested = read.bindings(0)["tested"].clone();
        let end_of_rows = match closing_of(&read) {
            Some(at) => {
                let closing = lines
                    .iter()
                    .position(|line| line.starts_with("  bindings "))
                    .expect("the closing line");
                assert_eq!(
                    lines[closing + 1..closing + 1 + unit.len()].to_vec(),
                    unit,
                    "{context}: the closing line with its tested lines\n{}",
                    read.show()
                );
                assert_eq!(
                    tested_at.len(),
                    unit.len(),
                    "{context}: no other tested line"
                );
                assert_eq!(list(&tested), want, "{context}: the printed entries");
                seen.push((at, true));
                at
            }
            None => {
                assert!(
                    tested_at.is_empty(),
                    "{context}: no tested line without its closing line\n{}",
                    read.show()
                );
                assert_eq!(
                    tested,
                    json!([]),
                    "{context}: none printed, none in the JSON"
                );
                let tail = lines.last().expect("a tail");
                let at = read.text()[..read.text().len() - tail.len() - 1]
                    .chars()
                    .count();
                seen.push((at, false));
                at
            }
        };
        assert!(end_of_rows <= CAP, "{context}");
    }
    let first_cut = seen
        .iter()
        .find(|(_, printed)| !printed)
        .unwrap_or_else(|| panic!("the sweep reaches the cut: {seen:?}"));
    assert!(
        seen.iter().any(|(_, printed)| *printed),
        "the sweep starts uncut: {seen:?}"
    );
    assert!(
        seen.iter()
            .skip_while(|(_, printed)| *printed)
            .all(|(_, printed)| !printed),
        "printed, then cut, never back: {seen:?}"
    );
    let closing_alone = format!("  bindings {}, {} check\n", count + 7, count + 7).len();
    assert!(
        CAP - first_cut.0 >= closing_alone,
        "at the first cut the closing line alone would fit ({} characters left, the line {closing_alone}): {seen:?}",
        CAP - first_cut.0
    );
}
