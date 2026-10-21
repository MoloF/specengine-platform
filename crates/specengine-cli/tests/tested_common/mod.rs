//! Helpers of the docs/features/tested-evidence.md (8b-1) tests
//! ("Acceptance criteria" RD-1..RD-8, TE-1..TE-7): the `[[tested]]` block
//! and the bound hash built here from "Data" ("Format 3 and the block",
//! "Bound hash"), never from the command's output; the code digests come
//! from `specengine-code`'s own level functions (`item_levels`,
//! `module_levels`, `ron::value_hashes`), as `lock_common`'s `item_code`
//! makes an accept's, and the hash is the store's `b3_hash`.
//!
//! Plain `spec verify` runs as the binary through `verify_common::verify`
//! (read-only checked). 8a's repositories (`verify_tests_common::Kiln`)
//! and what a run over them writes: `tested_kiln`.
//!
//! PHASE-B (names the spec does not give, chosen here; the developer's own
//! replace them in one place, this module): none on the CLI side beyond
//! 8a's; the JSON `tested` and `lock` objects are read by the spec's keys.
//!
//! Only the 8b-1 test binaries include this module (`mod tested_common;`,
//! after `mod common;`, `mod lock_common;` and `mod verify_common;`). Non-Latin characters in Rust sources are
//! Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde_json::Value;
use specengine_code::{
    LevelState, RonValues, RustParser, analyze_tree, item_levels, module_levels, value_hashes,
};

use crate::common::fixture;
use crate::lock_common::{CODE_RECIPE, HEADER, LockRepo, NORM_RECIPE, b3, blocks, value};
use crate::verify_common::Verify;

/// The golden (Data "Fixture"): the first lock a passing run writes in a
/// scratch copy of `fixtures/verify-tests`.
pub const GOLDEN: &str = "verify-tests.tested.lock";

/// The bound hash's tag line (Data "Bound hash").
pub const TAG: &str = "specengine-tested/v1";

/// The date of 8a's clock (`verify_tests_common::CLOCK`).
pub const DAY: &str = "2026-10-16";

/// The question's new tail (Data "The run's write", step 7).
pub const QUESTION_TAIL: &str = ", then commit their evidence to spec.lock? [y/N]";

/// The ignored-lock text (step 6).
pub const IGNORED: &str =
    "spec.lock is ignored by git: the lock is committed, so un-ignore it first";

/// The JSON `tested` entry's keys, in order (Data "Plain verify").
pub const TESTED_KEYS: [&str; 7] = [
    "node",
    "state",
    "category",
    "rev",
    "node_rev",
    "commit",
    "verified_at",
];

/// The run's JSON `lock` keys, in order (Data "The run's write", Output).
pub const LOCK_KEYS: [&str; 4] = ["rows", "commit", "unchanged", "refusal"];

/// The block's keys, in render order.
pub const BLOCK_KEYS: [&str; 7] = [
    "node",
    "rev",
    "spec_hash",
    "bound_hash",
    "tests",
    "commit",
    "verified_at",
];

/// The nodes a passing full run gives a block (Data "Fixture"), by bytes.
pub const BLOCK_NODES: [&str; 4] = ["CONE-01", "KILN-HOLD", "KILN-RAMP", "glaze-check/GLZ-01"];

/// The golden's text.
pub fn golden_text() -> String {
    fs::read_to_string(fixture("spec-lock").join(GOLDEN))
        .expect("fixtures/spec-lock/verify-tests.tested.lock")
}

// --------------------------------------------------------- bound bytes

/// One binding naming a node, as the bound bytes list it: its key and the
/// three level digests (Data "Bound hash").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    pub node: String,
    pub relation: &'static str,
    pub file: String,
    pub kind: &'static str,
    pub symbol: String,
    pub item_kind: Option<String>,
    pub sig: String,
    pub body: String,
    pub deps: String,
}

/// The relation's place in the key order (`implements`, `verifies`,
/// `configures`, `assumes`).
fn relation_rank(relation: &str) -> usize {
    ["implements", "verifies", "configures", "assumes"]
        .iter()
        .position(|known| *known == relation)
        .unwrap_or_else(|| panic!("a relation: {relation}"))
}

impl Bound {
    fn order(&self) -> (String, usize, String, String, String, Option<String>) {
        (
            self.node.clone(),
            relation_rank(self.relation),
            self.file.clone(),
            self.kind.to_owned(),
            self.symbol.clone(),
            self.item_kind.clone(),
        )
    }
}

/// The bound bytes of `bindings` (any order; sorted here by the entry key:
/// `node` bytes, `relation` as listed, `file`, `kind`, `symbol`,
/// `item_kind` bytes): the tag line, then per binding its key lines and
/// three digest lines as `render_block` writes them (`item_kind` when
/// present), then an empty line; every line LF-ended.
pub fn bound_bytes(bindings: &[Bound]) -> String {
    let mut sorted = bindings.to_vec();
    sorted.sort_by_key(Bound::order);
    sorted.dedup();
    let mut out = format!("{TAG}\n");
    for bound in &sorted {
        out.push_str(&format!(
            "node = \"{}\"\nrelation = \"{}\"\nfile = \"{}\"\nkind = \"{}\"\nsymbol = \"{}\"\n",
            bound.node, bound.relation, bound.file, bound.kind, bound.symbol
        ));
        if let Some(item_kind) = &bound.item_kind {
            out.push_str(&format!("item_kind = \"{item_kind}\"\n"));
        }
        out.push_str(&format!(
            "sig_hash = \"{}\"\nbody_hash = \"{}\"\ndeps_hash = \"{}\"\n\n",
            bound.sig, bound.body, bound.deps
        ));
    }
    out
}

/// The store's `b3_hash` of [`bound_bytes`].
pub fn bound_hash(bindings: &[Bound]) -> String {
    specengine_store::b3_hash(bound_bytes(bindings).as_bytes())
}

/// The digests of `levels`' hashed state.
fn digests(state: &LevelState, what: &str) -> (String, String, String) {
    match state {
        LevelState::Hashed(hashes) => (b3(&hashes.sig), b3(&hashes.body), b3(&hashes.deps)),
        LevelState::CannotVerify { categories } => {
            panic!("{what} cannot be hashed: {categories:?}")
        }
    }
}

/// The item `name` of `source` whose `#[cfg]` chain joined by `,` is
/// `cfg` (`None`: any): its kind and its three digests, through
/// `item_levels` over `analyze_tree`'s items.
pub fn item_digests(
    source: &str,
    name: &str,
    cfg: Option<&str>,
) -> (String, String, String, String) {
    let mut parser = RustParser::new().expect("the Rust grammar");
    let tree = parser.parse(source).expect("a tree");
    let analysis = analyze_tree(&tree, source);
    let indexes: Vec<usize> = analysis
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.name.as_deref() == Some(name) && cfg.is_none_or(|cfg| item.cfg.join(",") == cfg)
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        indexes.len(),
        1,
        "one item {name} (cfg {cfg:?}): {indexes:?}"
    );
    let kind = analysis.items[indexes[0]].kind.clone();
    let states = item_levels(&tree, source, &indexes);
    let state = states[0]
        .as_ref()
        .unwrap_or_else(|| panic!("no levels for {name}"));
    let (sig, body, deps) = digests(state, name);
    (kind, sig, body, deps)
}

/// `source`'s file-module digests, through `module_levels`.
pub fn module_digests(source: &str) -> (String, String, String) {
    let mut parser = RustParser::new().expect("the Rust grammar");
    let tree = parser.parse(source).expect("a tree");
    digests(&module_levels(&tree, source), "the module")
}

/// The digest of the RON value the comment on 1-based `line` anchors to:
/// `sig` = `body` = the value hash, `deps` the empty set's.
pub fn ron_digests(source: &str, line: usize) -> (String, String, String) {
    let RonValues::Hashed(values) = value_hashes(source) else {
        panic!("the RON file is rejected")
    };
    let found: Vec<_> = values
        .iter()
        .filter(|(range, _)| source[..range.start].matches('\n').count() + 1 == line)
        .collect();
    assert_eq!(found.len(), 1, "one anchored comment on line {line}");
    let value = b3(&found[0].1);
    (
        value.clone(),
        value,
        b3(&specengine_code::empty_deps_digest()),
    )
}

/// An item binding of `node` read from `root`'s `file`.
pub fn item(
    root: &Path,
    node: &str,
    relation: &'static str,
    file: &str,
    symbol: &str,
    name: &str,
    cfg: Option<&str>,
) -> Bound {
    let source = fs::read_to_string(root.join(file)).expect("the source");
    let (kind, sig, body, deps) = item_digests(&source, name, cfg);
    Bound {
        node: node.to_owned(),
        relation,
        file: file.to_owned(),
        kind: "item",
        symbol: symbol.to_owned(),
        item_kind: Some(kind),
        sig,
        body,
        deps,
    }
}

/// A module binding of `node` read from `root`'s `file`.
pub fn module(root: &Path, node: &str, relation: &'static str, file: &str, symbol: &str) -> Bound {
    let source = fs::read_to_string(root.join(file)).expect("the source");
    let (sig, body, deps) = module_digests(&source);
    Bound {
        node: node.to_owned(),
        relation,
        file: file.to_owned(),
        kind: "module",
        symbol: symbol.to_owned(),
        item_kind: None,
        sig,
        body,
        deps,
    }
}

/// A RON binding of `node`: the value the comment on `line` of `root`'s
/// `file` anchors to, at field path `symbol`.
pub fn ron(
    root: &Path,
    node: &str,
    relation: &'static str,
    file: &str,
    symbol: &str,
    line: usize,
) -> Bound {
    let source = fs::read_to_string(root.join(file)).expect("the RON source");
    let (sig, body, deps) = ron_digests(&source, line);
    Bound {
        node: node.to_owned(),
        relation,
        file: file.to_owned(),
        kind: "ron",
        symbol: symbol.to_owned(),
        item_kind: None,
        sig,
        body,
        deps,
    }
}

// --------------------------------------------------------------- blocks

/// One block's values (Data "Format 3 and the block").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub node: String,
    pub rev: u32,
    pub spec_hash: String,
    pub bound_hash: String,
    /// (file, symbol), in the order written.
    pub tests: Vec<(String, String)>,
    pub commit: String,
    pub verified_at: String,
}

/// A TOML basic string of `text` as "The file" escapes it (only `"`, `\`
/// and the controls).
pub fn basic(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            ch if (ch as u32) < 0x20 || ch as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04X}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

impl Block {
    /// The block's bytes: `[[tested]]`, its keys as shown, `tests = [`,
    /// one `{ file = <s>, symbol = <s> },` per line two spaces in, `]`
    /// alone (no leading empty line).
    pub fn text(&self) -> String {
        let mut out = format!(
            "[[tested]]\nnode = {}\nrev = {}\nspec_hash = {}\nbound_hash = {}\ntests = [\n",
            basic(&self.node),
            self.rev,
            basic(&self.spec_hash),
            basic(&self.bound_hash)
        );
        for (file, symbol) in &self.tests {
            out.push_str(&format!(
                "  {{ file = {}, symbol = {} }},\n",
                basic(file),
                basic(symbol)
            ));
        }
        out.push_str(&format!(
            "]\ncommit = {}\nverified_at = {}\n",
            basic(&self.commit),
            basic(&self.verified_at)
        ));
        out
    }
}

/// A lock of this build's recipes: the header, `format = <2 or 3>` (3 iff
/// `blocks` is not empty), both recipes, each binding entry (a block's
/// `key = value` lines, no `[[binding]]` line), then each block, an empty
/// line before each.
pub fn lock_with(entries: &[String], blocks: &[Block]) -> String {
    let format = if blocks.is_empty() { 2 } else { 3 };
    let mut out = format!(
        "{HEADER}\nformat = {format}\nnorm_recipe = \"{NORM_RECIPE}\"\ncode_recipe = \"{CODE_RECIPE}\"\n"
    );
    for entry in entries {
        out.push_str("\n[[binding]]\n");
        out.push_str(entry);
    }
    let mut sorted = blocks.to_vec();
    sorted.sort_by(|a, b| a.node.as_bytes().cmp(b.node.as_bytes()));
    for block in &sorted {
        out.push('\n');
        out.push_str(&block.text());
    }
    out
}

/// The `[[tested]]` blocks of a canonical lock's text: `(node, the block's
/// bytes from `[[tested]]` to its last line, LF-ended)`, in file order.
pub fn tested_blocks(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for chunk in text.split("\n[[tested]]\n").skip(1) {
        let body = chunk.split("\n[[").next().unwrap_or_default();
        let body = body.trim_end_matches('\n');
        let node = body
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("node = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| panic!("a block's first line is its node:\n{chunk}"))
            .to_owned();
        out.push((node, format!("[[tested]]\n{body}\n")));
    }
    out
}

/// The block of `node` in `text`, when there is one.
pub fn block_of(text: &str, node: &str) -> Option<String> {
    tested_blocks(text)
        .into_iter()
        .find(|(id, _)| id == node)
        .map(|(_, block)| block)
}

/// The value of top-level `key` (`key = <value>`) in a block's bytes, the
/// quotes of a string dropped.
pub fn block_value(block: &str, key: &str) -> String {
    block
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key} = ")))
        .map(|value| value.trim_matches('"').to_owned())
        .unwrap_or_else(|| panic!("no {key} in\n{block}"))
}

/// The nodes holding a block in `text`, in file order.
pub fn block_nodes(text: &str) -> Vec<String> {
    tested_blocks(text)
        .into_iter()
        .map(|(node, _)| node)
        .collect()
}

/// `text` without its blocks, `format = 3` back to 2: what the entries'
/// side of the lock reads (Data: "format 3 is format 2 plus blocks").
pub fn without_blocks(text: &str) -> String {
    let head = text.split("\n[[tested]]\n").next().unwrap_or_default();
    let mut out = head.trim_end_matches('\n').to_owned();
    out.push('\n');
    out.replacen("\nformat = 3\n", "\nformat = 2\n", 1)
}

/// The relation named `name`, as a `&'static str`.
fn relation_named(name: &str) -> &'static str {
    ["implements", "verifies", "configures", "assumes"]
        .into_iter()
        .find(|known| *known == name)
        .unwrap_or_else(|| panic!("a relation: {name}"))
}

/// The kind named `name`, as a `&'static str`.
fn kind_named(name: &str) -> &'static str {
    ["item", "module", "ron"]
        .into_iter()
        .find(|known| *known == name)
        .unwrap_or_else(|| panic!("a kind: {name}"))
}

/// The entries of `node` in a canonical lock's `text` as the bound bytes
/// list bindings: each entry's key and its three level digests.
pub fn entry_bounds(text: &str, node: &str) -> Vec<Bound> {
    blocks(text)
        .into_iter()
        .filter(|block| value(block, "node") == Some(node))
        .map(|block| {
            let get = |key: &str| {
                value(&block, key)
                    .unwrap_or_else(|| panic!("an entry without {key}"))
                    .to_owned()
            };
            Bound {
                node: node.to_owned(),
                relation: relation_named(&get("relation")),
                file: get("file"),
                kind: kind_named(&get("kind")),
                symbol: get("symbol"),
                item_kind: value(&block, "item_kind").map(str::to_owned),
                sig: get("sig_hash"),
                body: get("body_hash"),
                deps: get("deps_hash"),
            }
        })
        .collect()
}

/// A block of `node` that reads `verified` over a lock whose entries are
/// every binding of `node`, each at its blob (an accept just made): `rev`
/// and `spec_hash` the entries', `bound_hash` over them, `tests` its
/// `verifies` entries' (file, symbol) or one made-up test, `commit`
/// `commit`, `verified_at` [`DAY`].
pub fn vouching_block(text: &str, node: &str, commit: &str) -> Block {
    let entries: Vec<Vec<(String, String)>> = blocks(text)
        .into_iter()
        .filter(|block| value(block, "node") == Some(node))
        .collect();
    let first = entries
        .first()
        .unwrap_or_else(|| panic!("{node} holds no entry"));
    let rev: u32 = value(first, "rev")
        .and_then(|rev| rev.parse().ok())
        .expect("a revision");
    let spec_hash = value(first, "spec_hash").expect("a spec_hash").to_owned();
    let mut tests: Vec<(String, String)> = entries
        .iter()
        .filter(|block| value(block, "relation") == Some("verifies"))
        .map(|block| {
            (
                value(block, "file").expect("a file").to_owned(),
                value(block, "symbol").expect("a symbol").to_owned(),
            )
        })
        .collect();
    if tests.is_empty() {
        tests.push(("tests/made_up.rs".to_owned(), "made_up::a_test".to_owned()));
    }
    tests.sort_by(|a, b| (a.0.as_bytes(), a.1.as_bytes()).cmp(&(b.0.as_bytes(), b.1.as_bytes())));
    tests.dedup();
    Block {
        node: node.to_owned(),
        rev,
        spec_hash,
        bound_hash: bound_hash(&entry_bounds(text, node)),
        tests,
        commit: commit.to_owned(),
        verified_at: DAY.to_owned(),
    }
}

/// A block of `node` at `rev` with made-up hashes of `digit`, one test.
pub fn made_up_block(node: &str, rev: u32, digit: char, commit: &str) -> Block {
    Block {
        node: node.to_owned(),
        rev,
        spec_hash: format!("b3:{}", digit.to_string().repeat(64)),
        bound_hash: format!("b3:{}", digit.to_string().repeat(64)),
        tests: vec![("tests/made_up.rs".to_owned(), "made_up::a_test".to_owned())],
        commit: commit.to_owned(),
        verified_at: DAY.to_owned(),
    }
}

/// A canonical format-2 lock's `text` with `blocks` after its entries (by
/// `node` bytes), `format = 3`.
pub fn with_blocks_added(text: &str, blocks: &[Block]) -> String {
    assert!(text.contains("\nformat = 2\n"), "a format-2 lock:\n{text}");
    let mut out = text.replacen("\nformat = 2\n", "\nformat = 3\n", 1);
    let mut sorted = blocks.to_vec();
    sorted.sort_by(|a, b| a.node.as_bytes().cmp(b.node.as_bytes()));
    for block in &sorted {
        out.push('\n');
        out.push_str(&block.text());
    }
    out
}

/// The evidence commit's message for `nodes` (the changed ones, any
/// order), its `Decided-by:` `committer`, its `Test-command:` `template`
/// (the question's template text): subject, empty line, the trailers.
pub fn evidence_message(nodes: &[&str], committer: &str, template: &str) -> String {
    let mut sorted: Vec<&str> = nodes.to_vec();
    sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let named = if sorted.len() > 3 {
        format!("{} and {} more", sorted[..3].join(", "), sorted.len() - 3)
    } else {
        sorted.join(", ")
    };
    let mut out = format!("spec: verify tests {named}\n\n");
    for node in &sorted {
        out.push_str(&format!("Lock: {node}\n"));
    }
    out.push_str(&format!(
        "Lock-mode: tests\nDecided-by: {committer}\nTest-command: {template}\n"
    ));
    out
}

/// The question's template text: between ` as ` and ` (timeout `.
pub fn template_of(question: &str) -> String {
    let (_, rest) = question.split_once(" as ").expect("` as ` in the question");
    let (template, _) = rest
        .split_once(" (timeout ")
        .expect("` (timeout ` in the question");
    template.to_owned()
}

// --------------------------------------------------------- plain verify

/// Plain `spec verify` (text and `--json`) from the root of `repo`,
/// `extra` its arguments: both exit alike, every byte under the worktree
/// (`.git` too) as before each. The queue is not checked (a test run's
/// `verify.tests` event is in it); `verify_common::verify` checks it where
/// no run happened.
pub fn plain(repo: &LockRepo, extra: &[&str]) -> Verify {
    let mut args = vec!["verify"];
    args.extend_from_slice(extra);
    let before = crate::common::snapshot(&repo.root);
    let text = repo.spec_piped(&repo.root, &args);
    assert_eq!(
        crate::common::snapshot(&repo.root),
        before,
        "spec {args:?} wrote"
    );
    args.push("--json");
    let json_run = repo.spec_piped(&repo.root, &args);
    assert_eq!(
        crate::common::snapshot(&repo.root),
        before,
        "spec {args:?} wrote"
    );
    assert_eq!(text.code, json_run.code, "text and JSON exit alike");
    let json = json_run.json();
    Verify {
        text,
        json_run,
        json,
    }
}

/// The document's `tested` entries.
pub fn tested(run: &Verify) -> Vec<Value> {
    run.json["tested"]
        .as_array()
        .unwrap_or_else(|| panic!("`tested` is an array:\n{}", run.json_run.show()))
        .clone()
}

/// The one `tested` entry of `node`.
pub fn tested_of(run: &Verify, node: &str) -> Value {
    let found: Vec<Value> = tested(run)
        .into_iter()
        .filter(|entry| entry["node"] == node)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one tested {node}: {found:#?}\n{}",
        run.text.show()
    );
    found.into_iter().next().expect("one")
}

/// `(state, category)` of `node`'s entry (`category` `""` when null).
pub fn state_of(run: &Verify, node: &str) -> (String, String) {
    let entry = tested_of(run, node);
    (
        entry["state"].as_str().expect("a state").to_owned(),
        entry["category"].as_str().unwrap_or_default().to_owned(),
    )
}

/// stdout's `tested | …` lines.
pub fn tested_lines(run: &Verify) -> Vec<String> {
    run.lines()
        .into_iter()
        .filter(|line| line.starts_with("tested | "))
        .map(str::to_owned)
        .collect()
}

/// The text line plain verify prints for an entry not `verified`.
pub fn tested_line(state: &str, category: Option<&str>, node: &str) -> String {
    match category {
        Some(category) => format!("tested | {state} {category} | {node}"),
        None => format!("tested | {state} | {node}"),
    }
}

/// `text`'s lines without its `tested | ` ones: what an evidence-free run
/// prints.
pub fn without_tested_lines(text: &str) -> String {
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        if !line.starts_with("tested | ") {
            out.push_str(line);
        }
    }
    out
}

/// The document without its `tested` key.
pub fn without_tested_key(document: &Value) -> Value {
    let mut copy = document.clone();
    copy.as_object_mut().expect("an object").remove("tested");
    copy
}

/// `program` (a `spec` binary) with `args` in `cwd`, stdin null, the
/// variables a test's `spec` gets (the sandbox's, `repo`'s `HOME`): its
/// exit and stdout.
pub fn spec_program(program: &Path, repo: &LockRepo, cwd: &Path, args: &[&str]) -> (i32, String) {
    let mut command = std::process::Command::new(program);
    command
        .env_clear()
        .current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null());
    for (name, value) in repo.git.vars() {
        if name != "HOME" {
            command.env(name, value);
        }
    }
    command.env("HOME", &repo.home);
    let output = command.output().expect("spawn the spec binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("UTF-8 stdout"),
    )
}

/// `json` (a compact verify document) without its `tested` key: what a
/// build before docs/features/tested-evidence.md printed.
pub fn json_without_tested(json: &str) -> String {
    for value in ["[]", "null"] {
        let key = format!(",\"tested\":{value}");
        if json.matches(&key).count() == 1 {
            return json.replacen(&key, "", 1);
        }
    }
    panic!("no empty or null `tested` once in {json}")
}

/// The ECS corpus (`verify_common::ecs`: every node accepted), then a
/// format-3 lock committed over its entries: PERK-STAMINA's block vouching
/// for its entries (`verified`), PERK-REGEN's at another `spec_hash`
/// (`stale spec_changed`), PERK-DRAIN's at another `bound_hash` (`stale
/// code_changed`), GONE-01's naming no node (`node_gone`). The repository,
/// the lock's text before the blocks, the blocks' commit.
pub fn ecs_with_blocks(label: &str) -> (LockRepo, String, String) {
    let repo = crate::verify_common::ecs(label);
    let text = repo.lock_text();
    let commit = repo.head();
    let stamina = vouching_block(&text, "PERK-STAMINA", &commit);
    let mut regen = vouching_block(&text, "PERK-REGEN", &commit);
    regen.spec_hash = format!("b3:{}", "7".repeat(64));
    let mut drain = vouching_block(&text, "PERK-DRAIN", &commit);
    drain.bound_hash = format!("b3:{}", "8".repeat(64));
    let gone = made_up_block("GONE-01", 1, '9', &commit);
    repo.write(
        "spec.lock",
        with_blocks_added(&text, &[stamina, regen, drain, gone]),
    );
    repo.commit_all("evidence");
    (repo, text, commit)
}

/// A block of `node` at `rev` vouching for `spec_hash` (made-up bound
/// hash of `digit`, one test).
pub fn block_at(node: &str, rev: u32, spec_hash: &str, digit: char, commit: &str) -> Block {
    let mut block = made_up_block(node, rev, digit, commit);
    block.spec_hash = spec_hash.to_owned();
    block
}

/// A canonical lock's `text` without the `[[binding]]` entries of `node`.
pub fn without_entries(text: &str, node: &str) -> String {
    let mut parts = text.split("\n[[binding]]\n");
    let mut out = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        if part.starts_with(&format!("node = \"{node}\"\n")) {
            // A block's text ends where the next one begins; keep a
            // trailing `[[tested]]` section that followed the last entry.
            if let Some(at) = part.find("\n[[tested]]\n") {
                out.push_str(&part[at..]);
            }
            continue;
        }
        out.push_str("\n[[binding]]\n");
        out.push_str(part);
    }
    out
}

// ------------------------------------------------- 8b-2: the reads

/// The keys of a read's `tested` entry, in order: 8b-1's plain-verify
/// entry (docs/features/tested-reads.md "Data": the model's `TestedRow`).
pub const READ_TESTED_KEYS: [&str; 7] = TESTED_KEYS;

/// The one line of an entry, verify's and every read's
/// (docs/features/tested-reads.md "Data"): `tested | <state>[ <category>]
/// | <node>`, no indent, no line end.
pub fn entry_line(entry: &Value) -> String {
    tested_line(
        entry["state"]
            .as_str()
            .unwrap_or_else(|| panic!("a state: {entry}")),
        entry["category"].as_str(),
        entry["node"]
            .as_str()
            .unwrap_or_else(|| panic!("a node: {entry}")),
    )
}

/// Plain verify's entries by node.
pub fn entries_by_node(run: &Verify) -> std::collections::BTreeMap<String, Value> {
    tested(run)
        .into_iter()
        .map(|entry| (entry["node"].as_str().expect("a node").to_owned(), entry))
        .collect()
}

/// `json` (one compact document of a read) without the `tested` keys
/// docs/features/tested-reads.md adds: each `,"tested":[]` and
/// `,"tested":null`, exactly `count` of them: what a build before 8b-2
/// printed for an evidence-free read.
pub fn without_read_tested(json: &str, count: usize) -> String {
    let mut out = json.to_owned();
    let mut found = 0;
    for key in [",\"tested\":[]", ",\"tested\":null"] {
        found += out.matches(key).count();
        out = out.replace(key, "");
    }
    assert_eq!(
        found, count,
        "{count} empty or null `tested` keys, not {found}, in {json}"
    );
    out
}

/// The text's `tested | ` lines (any indent), each without its indent.
pub fn read_tested_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("tested | "))
        .map(str::to_owned)
        .collect()
}
