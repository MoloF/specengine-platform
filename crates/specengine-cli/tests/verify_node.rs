//! docs/features/verify-node.md (6b-2a): `spec verify --node REF
//! [--symbol QPATH] [--json]`, plain verify's rows kept to one node's
//! bindings, judged over the files the code index picks. RD-1..RD-6 (each
//! test names the mutation `Mn` that turns it red), the cut's pin, TE-1,
//! TE-3 (the CLI half) and TE-4.
//!
//! **The oracle is plain verify** (as `verify_task.rs`'s): on one state,
//! plain `spec verify` and `spec verify --node REF` run as the binary
//! (text, then `--json`, stdin null). The test knows REF's holders (their
//! canonical IDs) and keeps plain's rows `docs/canon/spec-verify-scope.md`
//! "`--node`" (step 6) keeps for them: a row whose
//! `node` is a holder's ID, or a look-alike written for one (keyed as
//! written, kept under its Latin fix); never a section's or a parent's.
//! `--symbol QPATH` keeps of those the rows whose `symbol` is QPATH byte
//! for byte (a RON row also as `<file>#<symbol>`), or whose `anchor.from`
//! is. Everything else follows from the kept rows by the canon's rules
//! (`spec-verify.md` "Verdict and exit", `spec-verify-output.md` "Text",
//! "Hints", "JSON": under `--node` `ok` rows printed, class `-`; `moved
//! from` under a moved `ok` row; the cut), each derivation first
//! checked against plain's own document ([`plain_premises`]: a red premise
//! is this file's model, not the developer's code). `notes` are not
//! compared with plain's (they are the refresh's); the spec's own notes
//! are.
//!
//! TE-3 runs around every `--node` run ([`Untouched`]): every byte under
//! the worktree (`.git` too), `git status --porcelain=v1 -z
//! --untracked-files=all`, `.git/index`'s mtime, `spec.lock`'s bytes and
//! the queue's dump (proposals, tasks, runs, events).
//!
//! Fixtures reused, no new one: `code-a` with the format-2 golden lock
//! (`verify_common::golden`; its `expected.json` "generated" files for a
//! second holder), `anchor-states` (`anchor_common`, its 18 scenarios),
//! `verify-tests` (a kiln library, no lock) and `verify-ecs`
//! (`verify_common::ecs`, an ECS game crate with RON data). The code
//! index's level digests are tampered through the system's `sqlite3`
//! (`code_index_common`). Each repository has its own `HOME`. Non-Latin
//! characters in Rust sources are Unicode escapes (ADR-0024).

mod anchor_common;
mod code_index_common;
mod common;
mod lock_common;
mod verify_common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{data_dir, fixture, snapshot};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_cli::{Globals, VerifiedTree, VerifyRequest, verify};
use specengine_store::{ProposalQueue as _, SqliteQueue};
use verify_common::{
    BOUNDS, COUNTER, COUNTER_MD, ECS_LIB, LIB, LIMITS, MOVEMENT, STAMINA, TOP_KEYS, ecs, golden,
    golden_file, new_task, set_mode,
};

/// The slugs of the fixtures: the queue databases' names.
const CODE_A: &str = "code-a";
const ANCHORS: &str = "anchor-states";
const KILN: &str = "kiln-works";
const GAME: &str = "verify-ecs";

/// Plain `enforce-introduced`'s note (`spec-verify.md` "Verdict and
/// exit"): `--node` has no base either.
const NO_BASE: &str =
    "mode `enforce-introduced` has no base without --changed: judged as `enforce`";

/// The states a drift item is raised for (core `drift::raises`, a sweep):
/// the `hint: drift:` line.
const DRIFTING: [&str; 4] = ["code_ahead", "spec_ahead", "conflict", "broken_binding"];

/// Every state of `spec-verify.md` "States".
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

/// The cut ("Text"): `OUTPUT_CAP_CHARS`.
const CAP: usize = 40_000;

/// The look-alike of `GOAL-01` (a Cyrillic O).
const LOOK_ALIKE: &str = "G\u{41E}AL-01";

/// The look-alike of `LAW-BOUND` (a Cyrillic A).
const LAW_LOOK_ALIKE: &str = "L\u{410}W-BOUND";

// ------------------------------------------------------------------- runs

/// One `spec verify <args>` as the binary from one cwd: text, then
/// `--json`.
struct Verify {
    what: String,
    text: common::Run,
    json_run: common::Run,
}

impl Verify {
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

    fn rows(&self) -> Vec<Value> {
        list(&self.json()["rows"])
    }

    fn notes(&self) -> Vec<String> {
        list(&self.json()["notes"])
            .iter()
            .map(|note| note.as_str().expect("a note text").to_owned())
            .collect()
    }

    fn stderr_lines(&self) -> Vec<&str> {
        self.text.stderr.lines().collect()
    }

    /// stdout's last line.
    fn summary(&self) -> String {
        self.text().lines().last().unwrap_or_default().to_owned()
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

    /// Exit `code` with a report on stdout, text and JSON.
    fn answered(&self, code: i32) -> &Self {
        assert!(
            !self.text.stdout.is_empty() && !self.json_run.stdout.is_empty(),
            "{}: a report\n{}",
            self.what,
            self.show()
        );
        assert_eq!(self.code(), code, "{}", self.show());
        self
    }

    /// Exit 2, stdout empty, `--json` too: stderr.
    fn failed(&self) -> &str {
        assert_eq!(self.code(), 2, "{}", self.show());
        assert_eq!(self.text.stdout, "", "{}: stdout empty", self.what);
        assert_eq!(
            self.json_run.stdout, "",
            "{} --json: stdout empty",
            self.what
        );
        assert!(
            !self.text.stderr.is_empty(),
            "{}: the error on stderr",
            self.what
        );
        &self.text.stderr
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

/// `spec verify <extra>` from `cwd`, text and `--json`: both exit alike
/// and print the same stderr (a refusal: its first line; clap's usage
/// lines after it echo the arguments).
fn verify_at(repo: &LockRepo, cwd: &Path, extra: &[&str]) -> Verify {
    let mut args = vec!["verify"];
    args.extend_from_slice(extra);
    let what = format!("spec {}", args.join(" "));
    let text = repo.spec_piped(cwd, &args);
    args.push("--json");
    let json_run = repo.spec_piped(cwd, &args);
    assert_eq!(
        text.code,
        json_run.code,
        "{what}: text and JSON exit alike\n{}\n{}",
        text.show(),
        json_run.show()
    );
    if text.stdout.is_empty() && json_run.stdout.is_empty() {
        assert_eq!(
            text.stderr.lines().next(),
            json_run.stderr.lines().next(),
            "{what}: text and JSON refuse alike"
        );
    } else {
        assert_eq!(
            text.stderr, json_run.stderr,
            "{what}: text and JSON print the same stderr"
        );
    }
    Verify {
        what,
        text,
        json_run,
    }
}

/// Plain verify from the root.
fn plain(repo: &LockRepo) -> Verify {
    verify_at(repo, &repo.root, &[])
}

/// `case`'s `--node` from the root, TE-3 asserted around it.
fn scoped(repo: &LockRepo, slug: &str, case: &Case<'_>) -> Verify {
    let before = Untouched::take(repo, slug);
    let run = verify_at(repo, &repo.root, &case.args());
    before.still(repo, slug, &run.what);
    run
}

/// `spec index` from the root: the code index warm.
fn index(repo: &LockRepo) {
    repo.spec_piped(&repo.root, &["index"]).code(0);
}

// ------------------------------------------------------------------ TE-3

/// What a `--node` run must leave as it was (TE-3, the single door:
/// writes only the data directory).
#[derive(Debug, PartialEq, Eq)]
struct Untouched {
    /// Every byte under the worktree, `.git` too.
    files: BTreeMap<String, Option<Vec<u8>>>,
    /// `git status --porcelain=v1 -z --untracked-files=all`, read with
    /// `--no-optional-locks` (the harness never refreshes the index).
    status: Vec<u8>,
    /// `.git/index`'s mtime (seconds, nanoseconds).
    index_mtime: Option<(i64, i64)>,
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
/// optional index refresh, which would write `.git/index` (a racily
/// clean entry) and so move the mtime TE-3 pins.
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

// ---------------------------------------------------------------- oracle

/// One `--node` call on one state: REF as written, its holders, `--symbol`.
#[derive(Debug, Clone, Copy)]
struct Case<'a> {
    reference: &'a str,
    /// The holders' canonical IDs, in holder order, as `node` lists them.
    holders: &'a [&'a str],
    /// A row `node` naming a holder by another spelling (a look-alike):
    /// (as keyed, the holder's canonical ID).
    spelled: &'a [(&'a str, &'a str)],
    symbol: Option<&'a str>,
}

impl<'a> Case<'a> {
    fn of(reference: &'a str, holders: &'a [&'a str]) -> Self {
        Self {
            reference,
            holders,
            spelled: &[],
            symbol: None,
        }
    }

    fn on(self, symbol: &'a str) -> Self {
        Self {
            symbol: Some(symbol),
            ..self
        }
    }

    fn args(&self) -> Vec<&'a str> {
        let mut args = vec!["--node", self.reference];
        if let Some(symbol) = self.symbol {
            args.extend(["--symbol", symbol]);
        }
        args
    }

    /// The summary's prefix ("Text").
    fn prefix(&self) -> String {
        match self.symbol {
            Some(symbol) => format!("verify --node {} --symbol {symbol}: ", self.reference),
            None => format!("verify --node {}: ", self.reference),
        }
    }

    /// The note of a call keeping no row (`spec-verify-scope.md` "`--node`"
    /// step 7).
    fn no_binding(&self) -> String {
        match self.symbol {
            Some(symbol) => format!("verify: {} has no binding on {symbol}", self.reference),
            None => format!("verify: {} has no binding", self.reference),
        }
    }

    /// The holder `node` (as a row keys it) names, if any.
    fn names(&self, node: &str) -> Option<&'a str> {
        let canonical = self
            .spelled
            .iter()
            .find(|(written, _)| *written == node)
            .map_or(node, |(_, canonical)| canonical);
        self.holders.iter().copied().find(|id| *id == canonical)
    }

    /// `spec-verify-scope.md` "`--node`" step 6: the holders' rows, then
    /// `--symbol`'s.
    fn keeps(&self, row: &Value) -> bool {
        self.names(text(&row["node"])).is_some()
            && self.symbol.is_none_or(|symbol| symbol_is(row, symbol))
    }
}

/// `--symbol`: the place's `symbol` byte for byte, or a RON place's
/// `<file>#<symbol>`; a keyless place never.
fn place_is(place: &Value, qpath: &str) -> bool {
    !place["symbol"].is_null()
        && (place["symbol"] == qpath
            || (place["kind"] == "ron"
                && format!("{}#{}", text(&place["file"]), text(&place["symbol"])) == qpath))
}

/// `--symbol` on a row: its own key, or the key it moved from.
fn symbol_is(row: &Value, qpath: &str) -> bool {
    place_is(row, qpath)
        || (row["anchor"]["state"] == "moved" && place_is(&row["anchor"]["from"], qpath))
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn list(value: &Value) -> Vec<Value> {
    value.as_array().expect("a list").clone()
}

/// Rows naming no node: a dangling or bare marker, an entry naming no
/// node's canonical ID (`node_gone`).
fn names_no_node(row: &Value) -> bool {
    row["category"] == "dangling" || row["category"] == "node_gone" || row["node"] == ""
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

fn count(rows: &[Value], wanted: impl Fn(&Value) -> bool) -> usize {
    rows.iter().filter(|row| wanted(row)).count()
}

/// "JSON" `counts` over `rows`.
fn counts_of(rows: &[Value], skipped: &Value) -> Value {
    json!({
        "rows": rows.len(),
        "ok": count(rows, |row| row["class"].is_null()),
        "fix": count(rows, |row| row["class"] == "fix"),
        "check": count(rows, |row| row["class"] == "check"),
        "cannot": count(rows, |row| row["class"] == "cannot"),
        "pre_existing": count(rows, |row| row["pre_existing"] == true),
        "skipped": skipped,
    })
}

/// "JSON" `cascade` over `rows`.
fn cascade_of(rows: &[Value]) -> Value {
    json!({
        "by_blob": count(rows, |row| row["compared"] == "blob"),
        "by_ast": count(rows, |row| row["compared"] == "ast"),
    })
}

/// "Verdict and exit" over `rows` under `mode` (the summary's: plain
/// `enforce-introduced` reads `enforce`), no run cause.
fn verdict_of(mode: &Value, rows: &[Value]) -> (&'static str, i32) {
    if rows.iter().any(|row| row["class"] == "cannot") {
        return ("cannot-verify", 2);
    }
    let blocking = match mode.as_str() {
        Some("enforce") => rows.iter().any(|row| row["class"] == "fix"),
        Some("enforce-introduced") => rows
            .iter()
            .any(|row| row["class"] == "fix" && row["pre_existing"] == false),
        _ => false,
    };
    if blocking {
        ("blocked", 1)
    } else if rows.iter().all(|row| row["class"].is_null()) {
        ("clean", 0)
    } else {
        ("observed", 0)
    }
}

/// `delta` over `rows`: plain's lists, each kept to the keys of `rows`
/// (`moved` by its `to`), `affected_nodes` to the nodes of `rows` not
/// `ok` and not Pre-existing (a look-alike's under its fix).
fn delta_of(plain: &Value, rows: &[Value], case: Option<&Case<'_>>) -> Value {
    let keys: Vec<String> = rows.iter().map(key_of).collect();
    let mut delta = plain.clone();
    let bindings = &plain["bindings"];
    delta["bindings"]["moved"] = Value::Array(
        list(&bindings["moved"])
            .into_iter()
            .filter(|pair| keys.contains(&key_of(&pair["to"])))
            .collect(),
    );
    for name in ["ambiguous", "lost", "stale"] {
        delta["bindings"][name] = Value::Array(
            list(&bindings[name])
                .into_iter()
                .filter(|key| keys.contains(&key_of(key)))
                .collect(),
        );
    }
    delta["affected_nodes"] = Value::Array(
        list(&plain["affected_nodes"])
            .into_iter()
            .filter(|node| {
                rows.iter().any(|row| {
                    !row["class"].is_null()
                        && row["pre_existing"] == false
                        && (row["node"] == *node
                            || case.and_then(|case| case.names(text(&row["node"])))
                                == node.as_str())
                })
            })
            .collect(),
    );
    // docs/features/show-bindings.md "delta.tests_reaching": plain lists
    // the tests reaching its affected nodes (kept as plain's for its own
    // premise, `case` `None`); under `--node` it stays `null` (Q-E: only
    // the node's own files are judged).
    assert!(
        plain["tests_reaching"].is_array(),
        "premise: plain's tests_reaching is a list: {plain}"
    );
    if case.is_some() {
        delta["tests_reaching"] = Value::Null;
    }
    delta
}

/// The `hint:` line `line` stays over `rows` ("Hints": one per state and
/// category present; `moved` when a row moved, `ok` too; `drift` when a
/// row would raise an item).
fn hint_kept(line: &str, rows: &[Value]) -> bool {
    let label = line
        .strip_prefix("hint: ")
        .and_then(|rest| rest.split_once(": "))
        .map(|(label, _)| label)
        .unwrap_or_else(|| panic!("a hint line: {line:?}"));
    match label {
        "moved" => rows.iter().any(|row| row["anchor"]["state"] == "moved"),
        "drift" => rows
            .iter()
            .any(|row| DRIFTING.contains(&text(&row["state"]))),
        _ => rows
            .iter()
            .filter(|row| !row["class"].is_null())
            .any(|row| {
                let category = if row["detail"] == "key_problem" {
                    "key_problem"
                } else {
                    text(&row["category"])
                };
                let mut own = text(&row["state"]).to_owned();
                if !category.is_empty() {
                    own.push(' ');
                    own.push_str(category);
                }
                own == label
            }),
    }
}

/// The summary after its prefix ("Text"): `document`'s verdict, mode,
/// counts and cascade, `(m moved)` over `rows` (every kept row, printed
/// or not).
fn summary_of(document: &Value, rows: &[Value]) -> String {
    let counts = &document["counts"];
    let mode = document["mode"]
        .as_str()
        .map(|mode| format!(" [{mode}]"))
        .unwrap_or_default();
    let pre_existing = if document["base"].is_null() {
        String::new()
    } else {
        format!(" ({} pre-existing)", counts["pre_existing"])
    };
    let moved = count(rows, |row| {
        row["class"].is_null() && row["anchor"]["state"] == "moved"
    });
    let moved = if moved > 0 {
        format!(" ({moved} moved)")
    } else {
        String::new()
    };
    format!(
        "{}{mode}: {} fix, {} check{pre_existing}, {} cannot, {} ok{moved}; {} by blob, {} by ast; {} skipped",
        text(&document["verdict"]),
        counts["fix"],
        counts["check"],
        counts["cannot"],
        counts["ok"],
        document["cascade"]["by_blob"],
        document["cascade"]["by_ast"],
        counts["skipped"],
    )
}

/// Plain's text split: each non-`ok` row's block (its line and the
/// `moved from` / `candidate` lines under it), the `cannot:` lines, the
/// hints.
fn plain_parts(plain: &Verify) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut blocks, mut causes, mut hints): (Vec<String>, Vec<String>, Vec<String>) =
        (Vec::new(), Vec::new(), Vec::new());
    let mut lines: Vec<&str> = plain.text().lines().collect();
    lines.pop(); // the summary
    for line in lines {
        if line.starts_with("cannot: ") {
            causes.push(line.to_owned());
        } else if line.starts_with("hint: ") {
            hints.push(line.to_owned());
        } else if line.starts_with("  ") {
            let block = blocks
                .last_mut()
                .unwrap_or_else(|| panic!("a row before {line:?}"));
            block.push_str(line);
            block.push('\n');
        } else {
            blocks.push(format!("{line}\n"));
        }
    }
    (blocks, causes, hints)
}

/// A row's binding as a row line ends: `<kind> <symbol>[ <item_kind>]`.
fn binding_text(row: &Value) -> String {
    let mut binding = format!("{} {}", text(&row["kind"]), text(&row["symbol"]));
    if let Some(item_kind) = row["item_kind"].as_str() {
        binding.push(' ');
        binding.push_str(item_kind);
    }
    binding
}

/// An `ok` row as `--node` prints it ("Text": class `-`), a moved one with
/// its entry's place under it.
fn ok_block(row: &Value) -> String {
    let mut block = format!(
        "- | ok | {} | {} | {} | {}\n",
        text(&row["node"]),
        text(&row["relation"]),
        text(&row["file"]),
        binding_text(row)
    );
    if row["anchor"]["state"] == "moved" {
        block.push_str(&format!(
            "  moved from {}\n",
            anchor_common::place_text(&row["anchor"]["from"])
        ));
    }
    block
}

/// Plain's printed text, split: the block of each of `rows` (plain's own
/// for a row not `ok`, [`ok_block`] for an `ok` one), plain's `cannot:`
/// lines and its hints those rows keep.
fn parts_of(plain: &Verify, rows: &[Value]) -> (Vec<String>, Vec<String>) {
    let (blocks, causes, hints) = plain_parts(plain);
    let findings: Vec<Value> = plain
        .rows()
        .into_iter()
        .filter(|row| !row["class"].is_null())
        .collect();
    assert_eq!(
        blocks.len(),
        findings.len(),
        "premise: one block per plain row not ok\n{}",
        plain.show()
    );
    let block_of: BTreeMap<String, &String> = findings
        .iter()
        .map(Value::to_string)
        .zip(blocks.iter())
        .collect();
    let row_blocks = rows
        .iter()
        .map(|row| {
            if row["class"].is_null() {
                ok_block(row)
            } else {
                (*block_of
                    .get(&row.to_string())
                    .unwrap_or_else(|| panic!("plain prints the row {}", key_of(row))))
                .clone()
            }
        })
        .collect();
    let tail = causes
        .into_iter()
        .chain(hints.into_iter().filter(|line| hint_kept(line, rows)))
        .collect();
    (row_blocks, tail)
}

/// How many of `blocks` print before the cut ("Text": a block whole while
/// the blocks printed stay within [`CAP`] characters).
fn printed(blocks: &[String]) -> usize {
    let mut used = 0;
    for (at, block) in blocks.iter().enumerate() {
        used += block.chars().count();
        if used > CAP {
            return at;
        }
    }
    blocks.len()
}

/// The depth-1 keys of the JSON object `raw` opens with, in order.
fn top_keys(raw: &str) -> Vec<String> {
    let bytes = raw.as_bytes();
    let (mut depth, mut in_string, mut escaped, mut start) = (0usize, false, false, 0usize);
    let mut keys = Vec::new();
    for (at, &byte) in bytes.iter().enumerate() {
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
            b'}' | b']' => depth -= 1,
            _ => {}
        }
    }
    keys
}

/// The rows only one side has, or has otherwise, by key (for messages).
fn rows_diff(want: &[Value], got: &[Value]) -> String {
    let index = |rows: &[Value]| -> BTreeMap<String, Vec<String>> {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for row in rows {
            map.entry(key_of(row)).or_default().push(row.to_string());
        }
        map
    };
    let (want, got) = (index(want), index(got));
    let mut out = String::new();
    for key in want.keys().chain(got.keys()).collect::<BTreeSet<_>>() {
        match (want.get(key), got.get(key)) {
            (Some(left), Some(right)) if left == right => {}
            (left, right) => out.push_str(&format!(
                "{key}\n  want {}\n  got  {}\n",
                left.map_or("-".to_owned(), |rows| rows.join(" ; ")),
                right.map_or("-".to_owned(), |rows| rows.join(" ; ")),
            )),
        }
    }
    if out.is_empty() {
        out.push_str("(the same rows, in another order)\n");
    }
    out
}

/// The premises: this file's model reproduces plain's own document and
/// text; plain's `task`, `node`, `symbol` and `omitted` are `null`
/// ("JSON").
fn plain_premises(plain: &Verify) {
    let document = plain.json();
    let rows = plain.rows();
    assert_eq!(top_keys(plain.json_text()), TOP_KEYS, "plain's keys");
    for key in ["task", "node", "symbol", "omitted"] {
        assert_eq!(
            document[key],
            Value::Null,
            "plain's `{key}`: {}",
            plain.what
        );
    }
    assert_eq!(
        document["cannot"],
        json!([]),
        "premise: no run cause\n{}",
        plain.show()
    );
    assert_eq!(
        counts_of(&rows, &document["counts"]["skipped"]),
        document["counts"],
        "premise: counts"
    );
    assert_eq!(cascade_of(&rows), document["cascade"], "premise: cascade");
    let (verdict, code) = verdict_of(&document["mode"], &rows);
    assert_eq!(
        (verdict, code),
        (text(&document["verdict"]), plain.code()),
        "premise: the verdict\n{}",
        plain.show()
    );
    assert_eq!(
        delta_of(&document["delta"], &rows, None),
        document["delta"],
        "premise: delta"
    );
    let (blocks, tail) = parts_of(plain, &rows);
    let mut want: String = rows
        .iter()
        .zip(&blocks)
        .filter(|(row, _)| !row["class"].is_null())
        .map(|(_, block)| block.as_str())
        .collect();
    for line in tail {
        want.push_str(&line);
        want.push('\n');
    }
    want.push_str("verify: ");
    want.push_str(&summary_of(&document, &rows));
    want.push('\n');
    assert_eq!(plain.text(), want, "premise: plain's text");
}

/// TE-1's differential (RD-2): `--node`'s rows are plain's rows the case
/// keeps, each object whole and in order, up to the cut; `node`,
/// `symbol`, `omitted`, the counts, cascade, delta, verdict, exit and text
/// follow from them; `mode`, `base`, `task` and `cannot` are plain's; the
/// spec's own notes and warning. The rows kept, printed or not.
fn differential(plain: &Verify, run: &Verify, case: &Case<'_>) -> Vec<Value> {
    let context = format!("{} ({:?})", run.what, case.reference);
    assert!(
        !run.text().is_empty() && !run.json_text().is_empty(),
        "{context}: a report\n{}",
        run.show()
    );
    let base = plain.json();
    let document = run.json();
    let kept: Vec<Value> = plain
        .rows()
        .into_iter()
        .filter(|row| case.keeps(row))
        .collect();
    let (blocks, tail) = parts_of(plain, &kept);
    let shown = printed(&blocks);
    let rows = run.rows();
    assert!(
        rows == kept[..shown],
        "{context}: the rows are plain's kept to the node, the printed ones\n{}\n{}",
        rows_diff(&kept[..shown], &rows),
        run.show()
    );
    assert_eq!(top_keys(run.json_text()), TOP_KEYS, "{context}: keys");
    assert_eq!(document["node"], json!(case.holders), "{context}: `node`");
    assert_eq!(
        document["symbol"],
        json!(case.symbol),
        "{context}: `symbol`"
    );
    assert_eq!(
        document["omitted"],
        json!({"rows": kept.len() - shown}),
        "{context}: `omitted`"
    );
    for key in ["mode", "base", "task", "cannot"] {
        assert_eq!(document[key], base[key], "{context}: `{key}` is plain's");
    }
    let (verdict, code) = verdict_of(&document["mode"], &kept);
    assert_eq!(document["verdict"], verdict, "{context}: the verdict");
    assert_eq!(run.code(), code, "{context}: the exit\n{}", run.show());
    assert_eq!(
        document["counts"],
        counts_of(&kept, &json!(0)),
        "{context}: counts over the kept rows, `skipped` 0"
    );
    assert_eq!(document["cascade"], cascade_of(&kept), "{context}: cascade");
    assert_eq!(
        document["delta"],
        delta_of(&base["delta"], &kept, Some(case)),
        "{context}: delta over the kept rows"
    );

    // The spec's notes: the empty call's before the mode note (plain's,
    // kept); no other `has no binding`.
    let notes = run.notes();
    let empty = notes.iter().position(|note| *note == case.no_binding());
    assert_eq!(
        empty.is_some(),
        kept.is_empty(),
        "{context}: `{}` exactly when no row is kept: {notes:?}",
        case.no_binding()
    );
    assert!(
        !notes
            .iter()
            .any(|note| note.contains("has no binding") && *note != case.no_binding()),
        "{context}: {notes:?}"
    );
    let mode = notes.iter().position(|note| note == NO_BASE);
    assert_eq!(
        mode.is_some(),
        plain.notes().iter().any(|note| note == NO_BASE),
        "{context}: plain's no-base note: {notes:?}"
    );
    if let (Some(empty), Some(mode)) = (empty, mode) {
        assert!(empty < mode, "{context}: the note before the mode note");
    }
    let stderr = run.stderr_lines();
    for note in &notes {
        assert!(
            stderr.contains(&format!("note: {note}").as_str()),
            "{context}: `note: {note}` on stderr: {stderr:?}"
        );
    }
    let warnings: Vec<&&str> = stderr
        .iter()
        .filter(|line| line.starts_with("warning: "))
        .collect();
    if case.holders.len() > 1 {
        assert_eq!(warnings.len(), 1, "{context}: one warning: {stderr:?}");
        assert!(
            warnings[0].starts_with(&format!(
                "warning: `{}` has {} holders, all verified: ",
                case.reference,
                case.holders.len()
            )),
            "{context}: {warnings:?}"
        );
    } else {
        assert!(warnings.is_empty(), "{context}: no warning: {stderr:?}");
    }

    let mut want: String = blocks[..shown].concat();
    if shown < kept.len() {
        want.push_str(&format!(
            "[truncated: {} rows not shown; narrow with --symbol QPATH]\n",
            kept.len() - shown
        ));
    }
    for line in tail {
        want.push_str(&line);
        want.push('\n');
    }
    want.push_str(&case.prefix());
    want.push_str(&summary_of(&document, &kept));
    want.push('\n');
    assert!(
        run.text() == want,
        "{context}: the text\n--- want\n{}\n--- got\n{}",
        want.chars().take(6_000).collect::<String>(),
        run.text().chars().take(6_000).collect::<String>()
    );
    kept
}

/// Plain, then each case's `--node` on one state, each through
/// [`differential`]: plain, the `--node` runs in case order, and every
/// row they keep.
fn compare(repo: &LockRepo, slug: &str, cases: &[Case<'_>]) -> (Verify, Vec<Verify>, Vec<Value>) {
    let reference = plain(repo);
    plain_premises(&reference);
    let mut kept = Vec::new();
    let runs = cases
        .iter()
        .map(|case| {
            let run = scoped(repo, slug, case);
            kept.extend(differential(&reference, &run, case));
            run
        })
        .collect();
    (reference, runs, kept)
}

/// The cases (without `--symbol`) together keep every plain row naming a
/// node: each state, category and anchor state of the scenario is met.
fn covered(plain: &Verify, cases: &[Case<'_>]) {
    let missing: Vec<String> = plain
        .rows()
        .iter()
        .filter(|row| !names_no_node(row))
        .filter(|row| {
            !cases
                .iter()
                .any(|case| case.symbol.is_none() && case.keeps(row))
        })
        .map(key_of)
        .collect();
    assert!(
        missing.is_empty(),
        "premise: the cases name every bound node; not kept: {missing:#?}"
    );
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

/// A run cause's report under `--node` is plain's: exit 2, `rows` `[]`,
/// the same `cannot` lines and the same text but the summary's prefix,
/// `skipped` 0, `omitted.rows` 0.
fn run_cause_is_plain(plain: &Verify, run: &Verify, case: &Case<'_>, context: &str) {
    plain.answered(2);
    let base = plain.json();
    assert_ne!(
        base["cannot"],
        json!([]),
        "premise ({context}): a run cause\n{}",
        plain.show()
    );
    run.answered(2);
    let document = run.json();
    assert_eq!(
        document["cannot"],
        base["cannot"],
        "{context}\n{}",
        run.show()
    );
    assert_eq!(document["rows"], json!([]), "{context}");
    assert_eq!(document["verdict"], "cannot-verify", "{context}");
    assert_eq!(document["mode"], base["mode"], "{context}: the mode");
    assert_eq!(document["node"], json!(case.holders), "{context}: `node`");
    assert_eq!(document["omitted"], json!({"rows": 0}), "{context}");
    assert_eq!(
        document["counts"],
        counts_of(&[], &json!(0)),
        "{context}: \"Counts\": `skipped` 0 (plain's {})",
        base["counts"]["skipped"]
    );
    let lines = |run: &Verify| -> Vec<String> {
        let mut lines: Vec<String> = run.text().lines().map(str::to_owned).collect();
        lines.pop();
        lines
    };
    assert_eq!(
        lines(run),
        lines(plain),
        "{context}: plain's `cannot:` lines"
    );
    assert!(
        lines(run).iter().any(|line| line.starts_with("cannot: ")),
        "{context}\n{}",
        run.show()
    );
    assert_eq!(
        run.summary(),
        format!("{}{}", case.prefix(), summary_of(&document, &[])),
        "{context}"
    );
}

// ----------------------------------------------------------------- edits

/// `verify_output.rs`'s `drift()` over the golden, repeated: `CAP` and the
/// RON cap edited, LAW-BOUND's text edited, PIECE-COUNTER's `//!` marker
/// in the test file removed, a non-UTF-8 file (a note), plain
/// `enforce-introduced` (a note).
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

/// The two files of a cross-file pair (`symbol-names.md` "Ambiguity":
/// `src/x.rs` beside `src/x/mod.rs`): one qpath, `crates/tally::pair`.
const PAIR_FILES: [&str; 2] = ["crates/tally/src/pair.rs", "crates/tally/src/pair/mod.rs"];

/// Both halves of "A row in a set" and the stored names (after
/// `verify_task.rs`'s RD-3): GOAL-01's marker written as a look-alike (a
/// Cyrillic O: keyed as written, kept under GOAL-01; GOAL-01's entry left
/// lock-only, `marker_gone`), a look-alike of LAW-BOUND on a new item,
/// LAW-BOUND's Display impl removed (a lock-only `symbol_gone` row), an
/// alias marker (`AIM-01`, keyed GOAL-01), an agent's typo (`LAW-BUOND`:
/// dangling, kept by no node), and a cross-file `duplicate` twin whose
/// second file holds no marker: the first file's row is ambiguous only
/// through a file `--node LAW-STEP` never selects.
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

/// The rows a look-alike keys, as each holder's case needs them (the
/// last: the generated `look.rs`' Cyrillic-initial PIECE-COUNTER).
const GOAL_SPELLED: [(&str, &str); 1] = [(LOOK_ALIKE, "GOAL-01")];
const LAW_SPELLED: [(&str, &str); 1] = [(LAW_LOOK_ALIKE, "LAW-BOUND")];
const PIECE_SPELLED: [(&str, &str); 1] = [("\u{420}IECE-COUNTER", "PIECE-COUNTER")];

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

// ------------------------------------------------------------------ RD-1

/// The library's `verify` with `node` and `symbol` on `tree` and `task`:
/// its refusal's whole stderr line.
fn library_refusal(
    repo: &LockRepo,
    tree: VerifiedTree,
    task: Option<&str>,
    node: Option<&str>,
    symbol: Option<&str>,
) -> String {
    let result = verify(
        &repo.env(&repo.root),
        &Globals::default(),
        &VerifyRequest {
            tree,
            task: task.map(str::to_owned),
            node: node.map(str::to_owned),
            symbol: symbol.map(str::to_owned),
            git: repo.git_env(&repo.root),
        },
    );
    match result {
        Ok(outcome) => panic!(
            "{tree:?} task {task:?} node {node:?} symbol {symbol:?}: refused, not exit {}",
            outcome.exit().code()
        ),
        Err(error) => {
            assert_eq!(error.exit.code(), 2, "{}", error.message);
            error.message
        }
    }
}

/// RD-1: `--node` with `--changed`, `--staged`, `--task T` or `--tests`
/// (either order), and `--symbol` without `--node` (alone or with
/// `--changed`): usage, exit 2, stdout empty, `--json` too, stderr naming
/// both flags; nothing written. The library's four refusals, their exact
/// lines, the first match only (before `--staged`'s own refusal of
/// `--task`), the HOME-less env never read. `--node` and `--node
/// --symbol` alone answer with a report. M1: one of those forms accepted.
#[test]
fn verify_node_forms_are_usage() {
    let repo = golden("vn-forms");
    let task = new_task(&repo, &["LAW-BOUND"]);
    let before = Untouched::take(&repo, CODE_A);
    let conflicts: [(&[&str], &str); 8] = [
        (&["--node", "LAW-BOUND", "--changed"], "--changed"),
        (&["--changed", "--node", "LAW-BOUND"], "--changed"),
        (&["--node", "LAW-BOUND", "--staged"], "--staged"),
        (&["--staged", "--node", "LAW-BOUND"], "--staged"),
        (&["--node", "LAW-BOUND", "--task", task.as_str()], "--task"),
        (&["--task", task.as_str(), "--node", "LAW-BOUND"], "--task"),
        (&["--node", "LAW-BOUND", "--tests"], "--tests"),
        (&["--tests", "--node", "LAW-BOUND"], "--tests"),
    ];
    for (args, other) in conflicts {
        let run = verify_at(&repo, &repo.root, args);
        let stderr = run.failed();
        assert!(
            stderr.contains("--node") && stderr.contains(other),
            "{args:?}: the usage error names --node and {other}:\n{stderr}"
        );
        before.still(&repo, CODE_A, &run.what);
    }
    // `--symbol` alone: clap's requirement, naming both flags; beside
    // `--changed`, `--node` named.
    for (args, both) in [
        (&["--symbol", "crates/tally::counter::CAP"][..], true),
        (
            &["--symbol", "crates/tally::counter::CAP", "--changed"][..],
            false,
        ),
        (
            &["--changed", "--symbol", "crates/tally::counter::CAP"][..],
            false,
        ),
    ] {
        let run = verify_at(&repo, &repo.root, args);
        let stderr = run.failed();
        assert!(
            stderr.contains("--node <REF>") && (!both || stderr.contains("--symbol")),
            "{args:?}: --symbol requires --node:\n{stderr}"
        );
        before.still(&repo, CODE_A, &run.what);
    }

    let symbol_alone = "spec: the following required arguments were not provided: --node <REF>";
    let with_changed = "spec: the argument '--node <REF>' cannot be used with '--changed'";
    let with_staged = "spec: the argument '--node <REF>' cannot be used with '--staged'";
    let with_task = "spec: the argument '--node <REF>' cannot be used with '--task <T>'";
    let qpath = Some("crates/tally::counter::CAP");
    let node = Some("LAW-BOUND");
    let t = Some(task.as_str());
    for (tree, task, node, symbol, line) in [
        (VerifiedTree::WorkingTree, None, None, qpath, symbol_alone),
        (VerifiedTree::Changed, None, None, qpath, symbol_alone),
        (VerifiedTree::Staged, t, None, qpath, symbol_alone),
        (VerifiedTree::Changed, None, node, None, with_changed),
        (VerifiedTree::Changed, t, node, qpath, with_changed),
        (VerifiedTree::Staged, None, node, None, with_staged),
        (VerifiedTree::Staged, t, node, None, with_staged),
        (VerifiedTree::WorkingTree, t, node, None, with_task),
        (VerifiedTree::WorkingTree, t, node, qpath, with_task),
    ] {
        assert_eq!(
            library_refusal(&repo, tree, task, node, symbol),
            line,
            "{tree:?} task {task:?} node {node:?} symbol {symbol:?}"
        );
    }
    before.still(&repo, CODE_A, "the library's refusals");

    let reference = plain(&repo);
    plain_premises(&reference);
    for case in [
        Case::of("LAW-BOUND", &["LAW-BOUND"]),
        Case::of("LAW-BOUND", &["LAW-BOUND"]).on("crates/tally::counter::CAP"),
    ] {
        let run = scoped(&repo, CODE_A, &case);
        differential(&reference, &run, &case);
    }
}

// ------------------------------------------------------------------ RD-2

/// The golden's nodes (after [`drift`] and [`halves`]), each by one or
/// more REF forms; the documents' sections are nodes of their own.
fn golden_cases() -> Vec<Case<'static>> {
    vec![
        Case {
            spelled: &PIECE_SPELLED,
            ..Case::of("PIECE-COUNTER", &["PIECE-COUNTER"])
        },
        Case {
            spelled: &PIECE_SPELLED,
            ..Case::of(COUNTER_MD, &["PIECE-COUNTER"])
        },
        Case {
            spelled: &LAW_SPELLED,
            ..Case::of("LAW-BOUND", &["LAW-BOUND"])
        },
        Case {
            spelled: &LAW_SPELLED,
            ..Case::of("PIECE-COUNTER#LAW-BOUND", &["LAW-BOUND"])
        },
        Case::of("LAW-STEP", &["LAW-STEP"]),
        Case::of("LAW-ODD", &["LAW-ODD"]),
        Case {
            spelled: &GOAL_SPELLED,
            ..Case::of("GOAL-01", &["GOAL-01"])
        },
        Case {
            spelled: &GOAL_SPELLED,
            ..Case::of("AIM-01", &["GOAL-01"])
        },
        Case {
            spelled: &GOAL_SPELLED,
            ..Case::of("docs/records/GOAL-01.md", &["GOAL-01"])
        },
        Case::of("PREM-001", &["PREM-001"]),
        Case::of("cap-check/PASS-01", &["cap-check/PASS-01"]),
        Case::of("PIECE-LIMITS", &["PIECE-LIMITS"]),
        Case::of("LAW-SOURCE", &["LAW-SOURCE"]),
        Case::of("LAW-DEFAULT", &["LAW-DEFAULT"]),
        Case::of("docs/spec/limits.md", &["PIECE-LIMITS"]),
    ]
}

/// RD-2 over the golden with [`drift`]'s and [`halves`]' edits (plain
/// `enforce-introduced`; Fix, Check and Cannot rows; rows naming no node),
/// every node of the corpus by each REF form (an ID, an alias,
/// `ID#SECTION`, a document path, `slug/ID`): its rows are plain's kept
/// to its holders, whole and in order, every key (`levels`, `anchor`); the
/// rest follows from them; nothing written. A document keeps its own rows,
/// never its sections' (LAW-BOUND, LAW-STEP); its path the same rows;
/// LAW-BOUND a lock-only row and a look-alike's; GOAL-01 its lock-only,
/// alias and look-alike rows, by its alias as REF too; a node with no
/// binding is `clean` with its note. M2: a section's row kept, or a
/// lock-only, alias or look-alike row dropped.
#[test]
fn verify_node_rows_equal_plain_rows_filtered() {
    let repo = golden("vn-golden");
    drift(&repo);
    halves(&repo);
    let cases = golden_cases();
    let (reference, runs, _) = compare(&repo, CODE_A, &cases);
    covered(&reference, &cases);

    let rows = reference.rows();
    let kept_by = |at: usize| runs[at].rows();
    let lock_only = |row: &Value| row["markers"] == json!([]) && !row["lock"].is_null();
    for section in ["LAW-BOUND", "LAW-STEP"] {
        assert!(
            rows.iter().any(|row| row["node"] == section),
            "premise: PIECE-COUNTER's section {section} has rows"
        );
    }
    assert!(
        !kept_by(0).is_empty() && kept_by(0).iter().all(|row| row["node"] == "PIECE-COUNTER"),
        "the document keeps its own rows only\n{}",
        runs[0].show()
    );
    let bound = kept_by(2);
    assert!(
        bound
            .iter()
            .any(|row| row["category"] == "symbol_gone" && lock_only(row)),
        "LAW-BOUND's lock-only row\n{}",
        runs[2].show()
    );
    assert!(
        bound
            .iter()
            .any(|row| row["node"] == LAW_LOOK_ALIKE && row["category"] == "homoglyph"),
        "LAW-BOUND's look-alike row under its fix\n{}",
        runs[2].show()
    );
    let goal = kept_by(6);
    assert!(
        goal.iter()
            .any(|row| row["node"] == LOOK_ALIKE && row["category"] == "homoglyph"),
        "a look-alike row under its fix\n{}",
        runs[6].show()
    );
    assert!(
        goal.iter()
            .any(|row| row["category"] == "marker_gone" && lock_only(row)),
        "GOAL-01's lock-only row\n{}",
        runs[6].show()
    );
    assert!(
        goal.iter().any(
            |row| row["symbol"] == "crates/tally::counter::spare" && row["state"] == "unlocked"
        ),
        "the alias marker's row\n{}",
        runs[6].show()
    );
    assert!(
        kept_by(4).iter().any(|row| row["file"] == PAIR_FILES[0]
            && row["category"] == "ambiguous_symbol"
            && row["detail"] == "duplicate"),
        "the twin's row is `duplicate` through a file never selected\n{}",
        runs[4].show()
    );
    assert!(
        rows.iter().any(|row| row["node"] == "LAW-BUOND"),
        "premise: the typo's row (kept by no node)"
    );
    for at in [11, 12, 13, 14] {
        let document = runs[at].answered(0).json();
        assert_eq!(document["verdict"], "clean", "{}", runs[at].show());
        assert_eq!(document["rows"], json!([]));
    }
    // One node by several forms: the same rows.
    for (one, other) in [(0, 1), (2, 3), (6, 7), (6, 8), (11, 14)] {
        assert_eq!(runs[one].json()["rows"], runs[other].json()["rows"]);
    }
}

// ------------------------------------------------------------------ RD-3

/// RD-3: over `verify_common::ecs` under `enforce`, Fix rows outside
/// PERK-DRAIN (TUNE-01's markers raised past its record, in the RON file
/// and in PERK-DRAIN's own file), a Check row of it (its body edited) and
/// markers of another project (skipped, one in PERK-DRAIN's file): plain
/// is blocked (exit 1, `skipped` 2, both nodes affected); `--node
/// PERK-DRAIN` is observed (exit 0, `skipped` 0, `affected_nodes` its own,
/// the `verify --node PERK-DRAIN: ` prefix, its `ok` rows printed `- | ok
/// | …`); `--node TUNE-01` is blocked; PERK-STAMINA, whose file holds the skipped
/// marker, `skipped` 0. A run cause stays the whole run's (a bad `[verify]
/// mode`, a lock not UTF-8): exit 2, plain's `cannot:` lines, `rows` `[]`;
/// an unknown `[code] frameworks` name, which plain never reads, is
/// `--node`'s cause (1) (Q-F). Over `anchor-states`, a function renamed:
/// `--node KITE-RENAME` prints its moved `ok` row with its `moved from`
/// line, `(1 moved)`. M3: counts, `delta`, verdict or exit taken over
/// plain's rows.
#[test]
fn verify_node_counts_its_rows_only() {
    let repo = ecs("vn-counts");
    repo.replace(
        MOVEMENT,
        "// @configures TUNE-01@1",
        "// @configures TUNE-01@9",
    );
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(
        ECS_LIB,
        "//! @implements PERK-STAMINA@2\n",
        "//! @implements PERK-STAMINA@2\n// @implements other:PERK-STAMINA\n",
    );
    // In PERK-DRAIN's own file too: another node's Fix row and a skipped
    // marker the files judged hold, never kept.
    repo.replace(
        STAMINA,
        "// @configures TUNE-01@1\n",
        "// @configures TUNE-01@9\n",
    );
    repo.replace(
        STAMINA,
        "// @implements PERK-DRAIN\n",
        "// @implements other:PERK-DRAIN\n// @implements PERK-DRAIN\n",
    );
    set_mode(&repo.root, "enforce");
    let drain = Case::of("PERK-DRAIN", &["PERK-DRAIN"]);
    let tune = Case::of("TUNE-01", &["TUNE-01"]);
    let stamina = Case::of("PERK-STAMINA", &["PERK-STAMINA"]);
    let (whole, runs, _) = compare(&repo, GAME, &[drain, tune, stamina]);

    whole.answered(1);
    let document = whole.json();
    assert_eq!(document["verdict"], "blocked");
    assert_eq!(
        document["counts"]["skipped"], 2,
        "premise: two skipped markers"
    );
    assert_eq!(
        document["delta"]["affected_nodes"],
        json!(["PERK-DRAIN", "TUNE-01"]),
        "premise"
    );

    let own = runs[0].answered(0).json();
    assert_eq!(own["verdict"], "observed", "{}", runs[0].show());
    assert_eq!(own["counts"]["skipped"], 0);
    assert_eq!(own["counts"]["fix"], 0);
    assert_eq!(own["counts"]["check"], 1);
    assert_eq!(own["delta"]["affected_nodes"], json!(["PERK-DRAIN"]));
    let summary = runs[0].summary();
    assert!(
        summary.starts_with(
            "verify --node PERK-DRAIN: observed [enforce]: 0 fix, 1 check, 0 cannot, "
        ) && summary.ends_with("; 0 skipped"),
        "{summary:?}"
    );
    let blocked = runs[1].answered(1).json();
    assert_eq!(blocked["verdict"], "blocked", "{}", runs[1].show());
    assert_eq!(blocked["delta"]["affected_nodes"], json!(["TUNE-01"]));
    let lib = runs[2].answered(0).json();
    assert_eq!(lib["counts"]["skipped"], 0, "{}", runs[2].show());
    let oks = runs[2]
        .text()
        .lines()
        .filter(|line| line.starts_with("- | ok | PERK-STAMINA | "))
        .count();
    assert_eq!(
        oks,
        runs[2].rows().len(),
        "every `ok` row printed with class `-`\n{}",
        runs[2].show()
    );
    assert!(oks > 0, "{}", runs[2].show());

    // A moved `ok` row with its `moved from` line (`anchor-states`: the
    // ECS bodies are under `TRIVIAL_BODY_BYTES`, never followed).
    let kites = anchor_common::base("vn-counts-moved");
    anchor_common::change(&kites, "rename within the file");
    let rename = Case::of("KITE-RENAME", &["KITE-RENAME"]);
    let (_, runs, _) = compare(&kites, ANCHORS, &[rename]);
    let moved = runs[0].answered(0);
    let lines: Vec<&str> = moved.text().lines().collect();
    let row = "- | ok | KITE-RENAME | implements | crates/sky/src/rename.rs | item crates/sky::rename::lift_power function_item";
    let at = lines
        .iter()
        .position(|line| *line == row)
        .unwrap_or_else(|| panic!("the moved ok row printed\n{}", moved.show()));
    assert_eq!(
        lines.get(at + 1).copied(),
        Some(
            "  moved from crates/sky/src/rename.rs | item crates/sky::rename::lift_force function_item"
        ),
        "its `moved from` line\n{}",
        moved.show()
    );
    assert!(
        moved.summary().contains(" ok (1 moved); "),
        "{}",
        moved.summary()
    );

    // Run causes: never filtered.
    repo.replace("specengine.toml", "mode = \"enforce\"", "mode = \"strict\"");
    run_cause_is_plain(
        &plain(&repo),
        &scoped(&repo, GAME, &drain),
        &drain,
        "a bad [verify] mode",
    );
    repo.replace("specengine.toml", "mode = \"strict\"", "mode = \"enforce\"");
    let lock = repo.lock().expect("the lock");
    repo.write("spec.lock", b"\xff\xfe".as_slice());
    run_cause_is_plain(
        &plain(&repo),
        &scoped(&repo, GAME, &drain),
        &drain,
        "a lock not UTF-8",
    );
    repo.write("spec.lock", lock);

    // Q-F: `[code] frameworks` is read by `--node`'s refresh only.
    repo.replace(
        "specengine.toml",
        "roots = [\"crates\", \"assets\"]",
        "roots = [\"crates\", \"assets\"]\nframeworks = [\"no-such-framework\"]",
    );
    let unread = plain(&repo);
    unread.answered(1);
    assert_eq!(
        unread.json()["cannot"],
        json!([]),
        "plain never reads `[code] frameworks`\n{}",
        unread.show()
    );
    let run = scoped(&repo, GAME, &drain);
    run.answered(2);
    let refused = run.json();
    let causes = list(&refused["cannot"]);
    assert_eq!(causes.len(), 1, "{}", run.show());
    let cause = text(&causes[0]);
    assert!(
        cause.starts_with("specengine.toml:")
            && cause.contains("`no-such-framework`")
            && !cause.contains("spec: "),
        "cause (1): known_frameworks' line without `spec: `: {cause:?}"
    );
    assert_eq!(refused["rows"], json!([]));
    assert_eq!(refused["verdict"], "cannot-verify");
    assert_eq!(refused["counts"], counts_of(&[], &json!(0)));
    assert!(
        run.text()
            .lines()
            .any(|line| line == format!("cannot: {cause}")),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------ RD-4

/// `spec show REF`'s `spec: ` line (show fails).
fn show_error(repo: &LockRepo, reference: &str) -> String {
    let show = repo.spec_piped(&repo.root, &["show", reference]);
    assert_ne!(
        show.code,
        0,
        "premise: show {reference:?} fails\n{}",
        show.show()
    );
    show.stderr
        .lines()
        .find(|line| line.starts_with("spec: "))
        .unwrap_or_else(|| panic!("premise: show's `spec: ` line\n{}", show.show()))
        .to_owned()
}

/// RD-4, Q-E: a REF naming nothing (an unknown ID, an unknown prefix, a
/// look-alike, `project:`, an unindexed `.md` path, a section its file
/// lacks, a feature-scoped ID its feature lacks) exits 2 with stdout
/// empty, `--json` and `--symbol` too, stderr holding show's reason (show
/// itself exits 1 for some: ADR-0042 keeps exit 1 for a Fix row), the
/// look-alike's naming its Latin fix; nothing written. Two holders of one
/// ID (the generated `GOAL-03`): one `warning:`, the rows of both, `node`
/// the ID twice; one holder by its path: its rows. A written `@rev`: the
/// note, the rows of the node as it is. M4: a REF naming nothing at exit
/// 1, or run as plain.
#[test]
fn verify_node_naming_nothing_is_exit_2() {
    let repo = golden("vn-nothing");
    generated(&repo);
    repo.commit_all("a second holder");
    for reference in [
        "GOAL-99",
        "NOPE-404",
        LOOK_ALIKE,
        "other:PIECE-COUNTER",
        "docs/spec/nope.md",
        "PIECE-COUNTER#LAW-NOPE",
        "cap-check/PASS-02",
    ] {
        let reason = show_error(&repo, reference);
        for case in [
            Case::of(reference, &[]),
            Case::of(reference, &[]).on("crates/tally::counter::CAP"),
        ] {
            let before = Untouched::take(&repo, CODE_A);
            let run = verify_at(&repo, &repo.root, &case.args());
            let stderr = run.failed();
            assert!(
                stderr.lines().any(|line| line == reason),
                "{reference:?}: show's reason {reason:?} on stderr:\n{stderr}"
            );
            before.still(&repo, CODE_A, &run.what);
        }
    }
    let reason = show_error(&repo, LOOK_ALIKE);
    assert!(reason.contains("GOAL-01"), "the Latin fix named: {reason}");

    let twice = Case::of("GOAL-03", &["GOAL-03", "GOAL-03"]);
    let piece = Case {
        spelled: &PIECE_SPELLED,
        ..Case::of("PIECE-COUNTER", &["PIECE-COUNTER"])
    };
    let one = Case::of("docs/records/GOAL-03-twin.md", &["GOAL-03"]);
    let (_, runs, _) = compare(&repo, CODE_A, &[twice, piece, one]);
    assert!(
        !runs[0].rows().is_empty(),
        "GOAL-03's ambiguous marker\n{}",
        runs[0].show()
    );
    assert_eq!(runs[0].json()["rows"], runs[2].json()["rows"]);
    assert!(
        runs[1]
            .rows()
            .iter()
            .any(|row| row["category"] == "homoglyph"),
        "the look-alike under PIECE-COUNTER\n{}",
        runs[1].show()
    );

    let revised = verify_at(&repo, &repo.root, &["--node", "LAW-BOUND@2"]);
    let current = verify_at(&repo, &repo.root, &["--node", "LAW-BOUND"]);
    assert_eq!(revised.code(), current.code(), "{}", revised.show());
    assert_eq!(revised.json()["rows"], current.json()["rows"]);
    assert_eq!(revised.json()["counts"], current.json()["counts"]);
    assert!(
        revised.stderr_lines().contains(
            &"note: `@2` is ignored: `spec verify --node` judges each binding against the node as it is now"
        ),
        "{}",
        revised.show()
    );
}

// ------------------------------------------------------------------ RD-5

/// RD-5 over the golden with [`drift`] and [`halves`]: `--symbol` keeps
/// the rows whose `symbol` is QPATH byte for byte: an item (its
/// discriminator in), a RON value by `<file>#<path>` and by its path, a
/// gone symbol by its lock row; a suffix, a bare name, a trailing space, a
/// RON prefix or another node's qpath keep nothing: exit 0, `clean`, the
/// note `verify: <REF> has no binding on <QPATH>`. Over `anchor-states`
/// with every scenario: each moved row (`ok` too) is kept by its new key
/// and by its `anchor.from`, its anchors paired before the filter (a RON
/// row is never followed, so `anchor.from`'s `<file>#<symbol>` form has
/// no row).
/// M5: a suffix or tier match, or a moved row matched only on its new key.
#[test]
fn verify_symbol_is_byte_exact() {
    let repo = golden("vn-symbol");
    drift(&repo);
    halves(&repo);
    let bound = Case {
        spelled: &LAW_SPELLED,
        ..Case::of("LAW-BOUND", &["LAW-BOUND"])
    };
    let piece = Case::of("PIECE-COUNTER", &["PIECE-COUNTER"]);
    let cases = [
        bound.on("crates/tally::counter::CAP"),
        bound.on("crates/tally::counter::impl Tally #1"),
        bound.on("data/limits.ron#root.cap"),
        bound.on("root.cap"),
        bound.on("crates/tally::counter::<Tally as fmt::Display>::fmt"),
        piece.on("data/limits.ron#root"),
        piece.on("crates/tally"),
        // Nothing: a suffix, a name, a space, an impl without its
        // discriminator, a RON prefix, a file, another node's qpath.
        bound.on("counter::CAP"),
        bound.on("CAP"),
        bound.on("crates/tally::counter::CAP "),
        bound.on("crates/tally::counter::impl Tally"),
        bound.on("data/limits.ron#root"),
        bound.on("data/limits.ron"),
        piece.on("crates/tally::counter::CAP"),
    ];
    let (_, runs, _) = compare(&repo, CODE_A, &cases);
    for (at, run) in runs.iter().enumerate() {
        assert_eq!(
            run.rows().len(),
            usize::from(at < 7),
            "{:?}: one row, or none\n{}",
            cases[at].symbol,
            run.show()
        );
    }
    assert_eq!(
        runs[4].rows()[0]["category"],
        "symbol_gone",
        "a gone symbol matches its lock row"
    );
    assert_eq!(runs[2].rows(), runs[3].rows(), "a RON row by either form");
    runs[7].answered(0);
    assert_eq!(
        runs[7].summary(),
        "verify --node LAW-BOUND --symbol counter::CAP: clean [enforce]: 0 fix, 0 check, 0 cannot, 0 ok; 0 by blob, 0 by ast; 0 skipped"
    );

    // Moved rows: by either key.
    let kites = anchor_common::base("vn-symbol-moved");
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&kites, name);
    }
    let reference = plain(&kites);
    let moved: Vec<Value> = reference
        .rows()
        .into_iter()
        .filter(|row| {
            row["anchor"]["state"] == "moved" && row["anchor"]["from"]["symbol"] != row["symbol"]
        })
        .collect();
    // A RON row is never followed ("a RON path renamed" reads
    // `marker_gone` plus `unlocked`): `anchor.from`'s `<file>#<symbol>`
    // form has no row to match, so it is not exercised.
    assert!(
        reference
            .rows()
            .iter()
            .any(|row| row["kind"] == "ron" && row["category"] == "marker_gone"),
        "premise: the RON path renamed, not followed\n{}",
        reference.show()
    );
    assert!(
        moved.iter().any(|row| row["class"].is_null()),
        "premise: a moved ok row\n{}",
        reference.show()
    );
    let owned: Vec<(String, String, String, String)> = moved
        .iter()
        .map(|row| {
            let from = &row["anchor"]["from"];
            let old = if from["kind"] == "ron" {
                format!("{}#{}", text(&from["file"]), text(&from["symbol"]))
            } else {
                text(&from["symbol"]).to_owned()
            };
            (
                text(&row["node"]).to_owned(),
                text(&row["symbol"]).to_owned(),
                old,
                key_of(row),
            )
        })
        .collect();
    let holders: Vec<[&str; 1]> = owned.iter().map(|(node, ..)| [node.as_str()]).collect();
    let mut cases = Vec::new();
    for ((node, new, old, _), holder) in owned.iter().zip(&holders) {
        cases.push(Case::of(node, holder).on(new));
        cases.push(Case::of(node, holder).on(old));
    }
    let (_, runs, _) = compare(&kites, ANCHORS, &cases);
    for (pair, (_, _, old, key)) in runs.chunks(2).zip(&owned) {
        for run in pair {
            assert!(
                run.rows().iter().any(|row| key_of(row) == *key),
                "the moved row {key} by {old:?} or its new key\n{}",
                run.show()
            );
        }
    }
}

// ------------------------------------------------------------------ RD-6

/// The digest every tampered level column holds: no recipe makes it.
fn fake_digest() -> String {
    format!("b3:{}", "e".repeat(64))
}

/// RD-6 over the golden with [`drift`]'s edits. (a) The code index warm
/// (`spec index`), then a marker of LAW-BOUND in a new test file alone
/// (`unlocked`; no selected file changed), then one added to an indexed
/// file and one removed from it (its entry left `marker_gone`): `--node
/// LAW-BOUND` is plain's filtered each time, the new rows there, no
/// second `spec index`. (b) Indexed again, every stored
/// level digest tampered in the scratch data directory (`sqlite3`):
/// `--node` still equals plain, which never reads them, while the tampered
/// columns survive its no-op refresh. M6: the code refresh skipped (the
/// new file's `unlocked` row missing); M7: the index's level columns read
/// (a row compared by `ast` changes).
#[test]
fn verify_node_is_fresh() {
    let repo = golden("vn-fresh");
    drift(&repo);
    index(&repo);
    let fresh = "crates/tally/tests/fresh.rs";
    repo.write(
        fresh,
        "// @verifies LAW-BOUND@2\n#[test]\nfn fresh_bound() {}\n",
    );
    let bound = Case::of("LAW-BOUND", &["LAW-BOUND"]);
    let piece = Case::of("PIECE-COUNTER", &["PIECE-COUNTER"]);
    // The new file alone: no selected file changed, so only the refresh
    // brings its marker in.
    let (_, runs, _) = compare(&repo, CODE_A, &[bound]);
    assert!(
        runs[0]
            .rows()
            .iter()
            .any(|row| row["file"] == fresh && row["state"] == "unlocked"),
        "the new file's marker, no second spec index\n{}",
        runs[0].show()
    );
    repo.replace(
        COUNTER,
        "pub fn total(",
        "// @implements LAW-BOUND@2\npub fn total(",
    );
    repo.replace(
        BOUNDS,
        "// @verifies LAW-BOUND@2 [body] mutation: drop the cap\n",
        "",
    );
    let (_, runs, _) = compare(&repo, CODE_A, &[bound]);
    let rows = runs[0].rows();
    assert!(
        rows.iter()
            .any(|row| row["file"] == fresh && row["state"] == "unlocked"),
        "the new file's marker\n{}",
        runs[0].show()
    );
    assert!(
        rows.iter().any(|row| row["file"] == COUNTER
            && row["symbol"] == "crates/tally::counter::total"
            && row["state"] == "unlocked"),
        "the marker added to an indexed file\n{}",
        runs[0].show()
    );
    assert!(
        rows.iter()
            .any(|row| row["file"] == BOUNDS && row["category"] == "marker_gone"),
        "the marker removed from an indexed file\n{}",
        runs[0].show()
    );

    // M7: the stored level digests tampered after a refresh.
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
    let (_, runs, kept) = compare(&repo, CODE_A, &[bound, piece]);
    assert!(
        kept.iter()
            .any(|row| row["compared"] == "ast" && row["state"] == "ok"),
        "premise: an `ok` row compared by `ast`, which a stored digest would turn\n{}",
        runs[0].show()
    );
    assert_eq!(
        tampered(),
        before,
        "premise: the no-op refresh kept the tampered columns"
    );
}

// --------------------------------------------------------- the cut's pin

/// The cut ("Text"), a pin: LAW-BOUND bound by 500 marked tests in one
/// file (one `unlocked` row each, about 120 characters) passes 40 000
/// characters: the blocks printed stay within the cap, then
/// `[truncated: <k> rows not shown; narrow with --symbol QPATH]`; the JSON
/// `rows` are the printed ones, `omitted.rows` = k; counts, hints and
/// `delta` over every kept row. `--symbol` narrows it uncut. M: `--node`
/// left uncut.
#[test]
fn verify_node_cut_names_narrowing() {
    let repo = golden("vn-cut");
    let mut many = String::new();
    for at in 0..500 {
        many.push_str(&format!(
            "// @verifies LAW-BOUND@2\n#[test]\nfn stays_within_the_cap_{at:04}() {{}}\n\n"
        ));
    }
    repo.write("crates/tally/tests/many.rs", many);
    let bound = Case::of("LAW-BOUND", &["LAW-BOUND"]);
    let narrow = bound.on("crates/tally::test:many::stays_within_the_cap_0499");
    let (_, runs, kept) = compare(&repo, CODE_A, &[bound, narrow]);
    let cut = runs[0].json();
    let omitted = cut["omitted"]["rows"].as_u64().expect("omitted.rows");
    assert!(omitted > 0, "the cut falls\n{}", runs[0].show());
    let shown = cut["rows"].as_array().expect("rows").len();
    assert_eq!(
        cut["counts"]["rows"].as_u64(),
        Some(shown as u64 + omitted),
        "counts over every kept row"
    );
    let before_tail: usize = runs[0]
        .text()
        .split("[truncated: ")
        .next()
        .expect("the rows")
        .chars()
        .count();
    assert!(
        before_tail <= CAP,
        "{before_tail} characters before the tail"
    );
    assert_eq!(runs[1].rows().len(), 1, "{}", runs[1].show());
    assert_eq!(runs[1].json()["omitted"], json!({"rows": 0}));
    assert!(kept.len() > shown);
}

// ------------------------------------------------------------------ TE-1

/// TE-1 (a): the golden with [`drift`], [`halves`] and the generated
/// files (a second holder, an ambiguous ID, a RON file past the depth
/// cap): every node by ID. Then a second state: a bound file deleted
/// (`file_gone`), one not UTF-8 (skipped and counted: `not_utf8`), one
/// excluded from `[code]` (`outside_code`), a marker's level list
/// narrowed (`levels_changed`).
fn corpus_golden() -> Vec<Value> {
    let repo = golden("vn-te1-golden");
    generated(&repo);
    repo.commit_all("a second holder");
    drift(&repo);
    halves(&repo);
    let mut cases = golden_cases();
    cases.push(Case::of("GOAL-03", &["GOAL-03", "GOAL-03"]));
    let (reference, _, mut kept) = compare(&repo, CODE_A, &cases);
    covered(&reference, &cases);

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
    let (reference, _, more) = compare(&repo, CODE_A, &cases);
    covered(&reference, &cases);
    kept.extend(more);
    kept
}

/// TE-1 (b): `fixtures/anchor-states`, its baseline, then its 18
/// scenarios applied together: every node (the two documents, every
/// section, the records; `WISP-01` an alias of `WISP-02` after its
/// scenario, by both IDs; a document by path): merged (moved) rows, `ok`
/// too, with their `moved from` line, ambiguous groups, lost and lock-only
/// rows, a `hidden_by_error` Cannot row.
fn corpus_anchor_states() -> Vec<Value> {
    let repo = anchor_common::base("vn-te1-anchors");
    let mut ids: Vec<&str> = anchor_common::NODES.to_vec();
    ids.extend([
        "KITE-DECOY",
        "KITE-FLIGHT",
        "KITE-RIGGING",
        "WISP-03",
        "WISP-04",
    ]);
    let holders: Vec<[&str; 1]> = ids.iter().map(|id| [*id]).collect();
    let cases: Vec<Case<'_>> = ids
        .iter()
        .zip(&holders)
        .map(|(id, holder)| Case::of(id, holder))
        .collect();
    let (reference, _, mut kept) = compare(&repo, ANCHORS, &cases);
    covered(&reference, &cases);

    for name in anchor_common::SCENARIOS {
        anchor_common::change(&repo, name);
    }
    let renamed: Vec<&str> = ids
        .iter()
        .map(|id| if *id == "WISP-01" { "WISP-02" } else { id })
        .collect();
    let holders: Vec<[&str; 1]> = renamed.iter().map(|id| [*id]).collect();
    let mut cases: Vec<Case<'_>> = renamed
        .iter()
        .zip(&holders)
        .map(|(id, holder)| Case::of(id, holder))
        .collect();
    cases.push(Case::of("WISP-01", &["WISP-02"]));
    cases.push(Case::of("docs/spec/rigging.md", &["KITE-RIGGING"]));
    let (reference, runs, more) = compare(&repo, ANCHORS, &cases);
    covered(&reference, &cases);
    let wisp = &runs[runs.len() - 2];
    assert!(
        !wisp.rows().is_empty() && wisp.rows().iter().all(|row| row["node"] == "WISP-02"),
        "the alias REF keeps the new ID's rows, never the old entry\n{}",
        wisp.show()
    );
    assert!(
        more.iter()
            .any(|row| row["class"].is_null() && row["anchor"]["state"] == "moved"),
        "a moved ok row printed with its line"
    );
    kept.extend(more);
    kept
}

/// TE-1 (c): `fixtures/verify-tests` (no lock: marker rows): the document
/// `KILN-FIRING` by ID and path (its own rows: none; its sections' never),
/// each section (`ID#SECTION` too), `CONE-01` and its legacy alias
/// `PYRO-01` as REF (one marker written `PYRO-01`, keyed CONE-01),
/// `glaze-check/GLZ-01` (`slug/ID`); then `KILN-HOLD` renamed: the old ID
/// names nothing, the new one has no binding.
fn corpus_kiln() -> Vec<Value> {
    let repo = LockRepo::of("vn-te1-kiln", "verify-tests", "repo", &[]);
    let cases = [
        Case::of("KILN-FIRING", &["KILN-FIRING"]),
        Case::of("docs/spec/kiln.md", &["KILN-FIRING"]),
        Case::of("KILN-RAMP", &["KILN-RAMP"]),
        Case::of("KILN-FIRING#KILN-HOLD", &["KILN-HOLD"]),
        Case::of("KILN-COOL", &["KILN-COOL"]),
        Case::of("CONE-01", &["CONE-01"]),
        Case::of("PYRO-01", &["CONE-01"]),
        Case::of("CONE-02", &["CONE-02"]),
        Case::of("CONE-03", &["CONE-03"]),
        Case::of("glaze-check/GLZ-01", &["glaze-check/GLZ-01"]),
    ];
    let (reference, runs, mut kept) = compare(&repo, KILN, &cases);
    covered(&reference, &cases);
    assert!(runs[0].rows().is_empty(), "{}", runs[0].show());
    assert_eq!(
        runs[5].rows().len(),
        2,
        "CONE-01's two rows\n{}",
        runs[5].show()
    );
    assert_eq!(runs[5].json()["rows"], runs[6].json()["rows"]);

    repo.replace("docs/spec/kiln.md", "{#KILN-HOLD}", "{#KILN-DWELL}");
    repo.commit_all("the hold is a dwell now");
    let cases = [
        Case::of("KILN-DWELL", &["KILN-DWELL"]),
        Case::of("KILN-RAMP", &["KILN-RAMP"]),
        Case::of("CONE-01", &["CONE-01"]),
    ];
    let (_, _, more) = compare(&repo, KILN, &cases);
    kept.extend(more);
    verify_at(&repo, &repo.root, &["--node", "KILN-HOLD"]).failed();
    kept
}

/// TE-1 (d): `verify_common::ecs` (an ECS game crate, a RON file) with a
/// body edited, a RON value edited and a marker raised past its node:
/// every node by ID, the documents by path too.
fn corpus_ecs() -> Vec<Value> {
    let repo = ecs("vn-te1-ecs");
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
    let cases = [
        Case::of("PERK-STAMINA", &["PERK-STAMINA"]),
        Case::of("docs/spec/stamina.md", &["PERK-STAMINA"]),
        Case::of("PERK-REGEN", &["PERK-REGEN"]),
        Case::of("PERK-DRAIN", &["PERK-DRAIN"]),
        Case::of("TUNE-01", &["TUNE-01"]),
        Case::of("TUNE-02", &["TUNE-02"]),
        Case::of("TUNE-03", &["TUNE-03"]),
        Case::of("PERK-MOVE", &["PERK-MOVE"]),
        Case::of("docs/spec/movement.md", &["PERK-MOVE"]),
        Case::of("PERK-SPRINT", &["PERK-SPRINT"]),
        Case::of("PERK-SPRINT-COST", &["PERK-SPRINT-COST"]),
        Case::of("PERK-WALK", &["PERK-WALK"]),
    ];
    let (reference, _, kept) = compare(&repo, GAME, &cases);
    covered(&reference, &cases);
    kept
}

/// TE-1 (e): a walk gap anywhere is plain's run cause (3) under `--node`,
/// the same `cannot:` line, `rows` `[]`, exit 2, never the refresh's
/// error: over `verify_common::ecs`, a code file no node binds made
/// unreadable (`--node` cold, then warm), then a directory that cannot be
/// listed; readable again, the report.
fn corpus_walk_gap() -> Vec<Value> {
    let repo = ecs("vn-te1-gap");
    let spare = "crates/game/src/spare.rs";
    repo.write(spare, "pub fn spare() {}\n");
    repo.write("crates/game/src/deep/inner.rs", "pub fn inner() {}\n");
    repo.commit_all("unbound code");
    let regen = Case::of("PERK-REGEN", &["PERK-REGEN"]);
    {
        let _mode = Mode::set(repo.root.join(spare), 0o000);
        let whole = plain(&repo);
        for warmth in ["cold", "warm"] {
            let run = verify_at(&repo, &repo.root, &regen.args());
            run_cause_is_plain(
                &whole,
                &run,
                &regen,
                &format!("an unreadable file, {warmth}"),
            );
        }
    }
    {
        let _mode = Mode::set(repo.root.join("crates/game/src/deep"), 0o000);
        let whole = plain(&repo);
        let run = verify_at(&repo, &repo.root, &regen.args());
        run_cause_is_plain(&whole, &run, &regen, "a directory not listed");
    }
    let (_, _, kept) = compare(&repo, GAME, &[regen]);
    assert!(!kept.is_empty(), "readable again: PERK-REGEN's rows");
    kept
}

/// One corpus of TE-1: every row its `--node` runs keep.
type Corpus = fn() -> Vec<Value>;

/// TE-1: every node of each corpus, `--node` against plain filtered
/// ([`differential`], each corpus on a thread of its own); across them
/// every state, every anchor state and every category the corpora give a
/// node's rows is met. M2.
#[test]
fn verify_node_equals_plain_on_every_corpus() {
    let corpora: [(&str, Corpus); 5] = [
        ("golden", corpus_golden),
        ("anchor-states", corpus_anchor_states),
        ("verify-tests", corpus_kiln),
        ("verify-ecs", corpus_ecs),
        ("a walk gap", corpus_walk_gap),
    ];
    let kept: Vec<Value> = std::thread::scope(|scope| {
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
    });
    assert_eq!(
        values_of(&kept, |row| &row["state"]),
        owned(&STATES),
        "every state kept"
    );
    assert_eq!(
        values_of(&kept, |row| &row["anchor"]["state"]),
        owned(&ANCHOR_STATES),
        "every anchor state kept"
    );
    let categories = values_of(&kept, |row| &row["category"]);
    for category in [
        "item_error",
        "ambiguous_symbol",
        "ambiguous_id",
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
            "{category} kept: {categories:?}"
        );
    }
    let details = values_of(&kept, |row| &row["detail"]);
    for detail in ["duplicate", "not_utf8", "outside_code"] {
        assert!(details.contains(detail), "{detail} kept: {details:?}");
    }
}

// ------------------------------------------------------------------ TE-3

/// TE-3 (the CLI half): a git repository of `code-a` with the golden lock
/// and a task in the queue: `--node` valid (an ID, `--symbol`), with zero
/// rows, with two holders, naming nothing (an unknown ID, a look-alike),
/// as usage (with `--changed`, `--symbol` alone): `git status
/// --porcelain=v1 -z --untracked-files=all`, `.git/index`'s mtime,
/// `spec.lock`'s bytes, every byte under the worktree, the queue's rows
/// and events all as before; each byte-identical from a subdirectory and
/// repeated, and no absolute path printed.
#[test]
fn verify_node_writes_nothing_under_the_root() {
    let repo = golden("vn-door");
    generated(&repo);
    repo.commit_all("a second holder");
    new_task(&repo, &["LAW-BOUND"]);
    drift(&repo);
    let before = Untouched::take(&repo, CODE_A);
    assert!(!before.queue.is_empty(), "premise: a task in the queue");
    let calls: [&[&str]; 8] = [
        &["--node", "LAW-BOUND"],
        &[
            "--node",
            "LAW-BOUND",
            "--symbol",
            "crates/tally::counter::CAP",
        ],
        &["--node", "PIECE-LIMITS"],
        &["--node", "GOAL-03"],
        &["--node", "GOAL-99"],
        &["--node", LOOK_ALIKE],
        &["--node", "LAW-BOUND", "--changed"],
        &["--symbol", "crates/tally::counter::CAP"],
    ];
    for args in calls {
        let run = verify_at(&repo, &repo.root, args);
        before.still(&repo, CODE_A, &run.what);
        let deeper = verify_at(&repo, &repo.root.join("crates").join("tally"), args);
        before.still(&repo, CODE_A, &deeper.what);
        assert_eq!(deeper.bytes(), run.bytes(), "{args:?} from a subdirectory");
        let again = verify_at(&repo, &repo.root, args);
        before.still(&repo, CODE_A, &again.what);
        assert_eq!(again.bytes(), run.bytes(), "{args:?} repeated");
        let root = repo.root.display().to_string();
        assert!(
            !run.text().contains(&root)
                && !run.json_text().contains(&root)
                && !run.text.stderr.contains(&root),
            "{args:?}: no absolute path printed"
        );
    }
    assert_eq!(
        repo.lock().as_deref(),
        Some(fs::read(golden_file()).expect("the golden").as_slice()),
        "spec.lock is the golden's bytes"
    );
}

// ------------------------------------------------------------------ TE-4

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// TE-4, manual (`#[ignore]`), release: `code-a` with the golden lock and
/// a crate of 1 000 unrelated `.rs` files, the code index warm: `--node
/// LAW-BOUND` against plain verify, the medians of five warm runs printed
/// (`--no-capture`) for the spec's "Implementation"; `--node` still
/// plain's rows filtered, and below plain.
///
/// `cargo nextest run -p specengine-cli --release --test verify_node
/// verify_node_cost --run-ignored ignored-only --no-capture`
#[test]
#[ignore = "manual (TE-4): release, figures recorded in the spec"]
fn verify_node_cost() {
    let repo = golden("vn-cost");
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
    let case = Case::of("LAW-BOUND", &["LAW-BOUND"]);
    let (reference, runs, _) = compare(&repo, CODE_A, &[case]);
    assert!(!runs[0].rows().is_empty() && reference.rows().len() > runs[0].rows().len());
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
    eprintln!(
        "TE-4: code-a + 1 000 unrelated .rs files, warm index, median of 5: plain verify {} ms, verify --node LAW-BOUND {} ms",
        whole.as_millis(),
        node.as_millis()
    );
    assert!(node < whole, "--node {node:?} below plain {whole:?}");
}
