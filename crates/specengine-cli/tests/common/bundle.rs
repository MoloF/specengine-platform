//! Helpers of the `spec bundle` tests (docs/features/spec-cli-bundle.md):
//! the parts of a bundle's stdout, the names per layer of its JSON, the
//! layer keys in print order, and a BLAKE3 written here from the
//! specification (no crate of the code under test), so `bundle_hash` is
//! checked against a hash it did not compute.

use std::path::Path;

use serde_json::Value;

use super::graph::spec30;
use super::{Run, spec};

/// The JSON keys of the layers, in print order (spec, Data).
pub const LAYERS: [&str; 9] = [
    "targets",
    "open_questions",
    "ancestors",
    "criteria",
    "bindings",
    "decisions",
    "neighbours",
    "terms",
    "tests",
];

/// The top-level keys of `spec bundle --json` (spec, Data).
pub const TOP_KEYS: [&str; 13] = [
    "refs",
    "reason",
    "notes",
    "task",
    "budget",
    "tokens",
    "chars",
    "bytes",
    "bundle_hash",
    "body",
    "layers",
    "tail",
    "more",
];

/// The keys of an item (spec, Data).
pub const ITEM_KEYS: [&str; 11] = [
    "name",
    "kind",
    "title",
    "path",
    "line",
    "form",
    "status",
    "via",
    "working_answer",
    "tokens_est",
    "archived",
];

/// The keys of a tail entry (spec, Data).
pub const TAIL_KEYS: [&str; 6] = ["name", "title", "path", "line", "tokens_est", "layer"];

/// The keys of a working answer (spec, Data).
pub const WORKING_ANSWER_KEYS: [&str; 5] = ["name", "written", "path", "line", "state"];

/// The words of P2-3 (07 §1.2): stack words and this repository's role
/// names; none may reach a bundle of a project that never wrote them.
pub const STACK_WORDS: [&str; 15] = [
    "cargo",
    "nextest",
    "clippy",
    "bevy",
    "pnpm",
    "npm",
    "nest",
    "react",
    "jira",
    "requirement-analyst",
    "spec-writer",
    "rust-developer",
    "ui-developer",
    "test-engineer",
    "code-reviewer",
];

/// One answered text bundle split into its body and its two closing lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBundle {
    pub body: String,
    /// `b3:<64 hex>` of the `bundle_hash` line.
    pub hash: String,
    /// The `tokens … of …, chars …, bytes …, not included …` line.
    pub totals: String,
}

/// The body and the two lines outside it of a text bundle.
pub fn split_text(stdout: &str) -> TextBundle {
    let without_end = stdout
        .strip_suffix('\n')
        .unwrap_or_else(|| panic!("a bundle ends with a line end: {stdout:?}"));
    let (rest, totals) = without_end
        .rsplit_once('\n')
        .unwrap_or_else(|| panic!("no totals line: {stdout:?}"));
    let (body, hash_line) = rest
        .rsplit_once('\n')
        .map(|(body, line)| (format!("{body}\n"), line))
        .unwrap_or_else(|| panic!("no bundle_hash line: {stdout:?}"));
    let hash = hash_line
        .strip_prefix("bundle_hash ")
        .unwrap_or_else(|| panic!("not a bundle_hash line: {hash_line:?}"))
        .to_owned();
    assert!(
        totals.starts_with("tokens "),
        "not a totals line: {totals:?}"
    );
    TextBundle {
        body,
        hash,
        totals: totals.to_owned(),
    }
}

/// `spec bundle args…` in `root` with `HOME=home`, within the graph
/// deadline.
pub fn bundle(home: &Path, root: &Path, args: &[&str]) -> Run {
    let mut all = vec!["bundle"];
    all.extend_from_slice(args);
    spec30(home, root, &all)
}

/// `spec --json bundle args…`, which must answer (exit 0).
pub fn bundle_json(home: &Path, root: &Path, args: &[&str]) -> Value {
    let mut all = vec!["--json", "bundle"];
    all.extend_from_slice(args);
    let run = spec30(home, root, &all);
    run.code(0);
    run.json()
}

/// `spec --json bundle args…` with the long deadline of [`spec`] (large
/// scratch corpora).
pub fn bundle_json_slow(home: &Path, root: &Path, args: &[&str]) -> Value {
    let mut all = vec!["--json", "bundle"];
    all.extend_from_slice(args);
    let run = spec(home, root, &all);
    run.code(0);
    run.json()
}

/// The item names of one layer.
pub fn names(json: &Value, layer: &str) -> Vec<String> {
    json["layers"][layer]
        .as_array()
        .unwrap_or_else(|| panic!("layer {layer} is no array: {json}"))
        .iter()
        .map(|item| item["name"].as_str().expect("name").to_owned())
        .collect()
}

/// The item names of every layer, in print order.
pub fn all_layers(json: &Value) -> Vec<(&'static str, Vec<String>)> {
    LAYERS
        .iter()
        .map(|&layer| (layer, names(json, layer)))
        .collect()
}

/// The item of `layer` named `name`.
pub fn item<'a>(json: &'a Value, layer: &str, name: &str) -> &'a Value {
    json["layers"][layer]
        .as_array()
        .expect("layer array")
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("no {name} in {layer}: {json}"))
}

/// `(type, direction)` of an item's `via`.
pub fn via(item: &Value) -> Vec<(String, String)> {
    item["via"]
        .as_array()
        .unwrap_or_else(|| panic!("no via array: {item}"))
        .iter()
        .map(|pair| {
            (
                pair["type"].as_str().expect("type").to_owned(),
                pair["direction"].as_str().expect("direction").to_owned(),
            )
        })
        .collect()
}

/// The names of the tail entries.
pub fn tail_names(json: &Value) -> Vec<String> {
    json["tail"]
        .as_array()
        .unwrap_or_else(|| panic!("tail is no array: {json}"))
        .iter()
        .map(|entry| entry["name"].as_str().expect("name").to_owned())
        .collect()
}

/// The items of layers 2 to 9 (every candidate placed in the body).
pub fn placed_candidates(json: &Value) -> Vec<String> {
    LAYERS[1..]
        .iter()
        .flat_map(|layer| names(json, layer))
        .collect()
}

/// The minimum a `--budget 1` run names (`… minimum of <n> tokens …`).
pub fn minimum(home: &Path, root: &Path, refs: &[&str]) -> u32 {
    let mut args: Vec<&str> = refs.to_vec();
    args.extend_from_slice(&["--budget", "1"]);
    let run = bundle(home, root, &args);
    run.code(2);
    assert!(run.stdout.is_empty(), "{}", run.show());
    let tail = run
        .stderr
        .split("minimum of ")
        .nth(1)
        .unwrap_or_else(|| panic!("no minimum named: {}", run.show()));
    tail.split(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no number after `minimum of`: {}", run.show()))
}

/// `text` has `word` as a whole word (ASCII-case-insensitive; a word
/// character is an ASCII letter, digit, `_` or `-`).
pub fn has_word(text: &str, word: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let word = word.to_ascii_lowercase();
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    lower.match_indices(&word).any(|(at, _)| {
        let before = lower[..at].chars().next_back();
        let after = lower[at + word.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

/// Every string literal of a Rust source's lines outside `//` comments
/// (naive: the text between pairs of `"`, as core `check_genre.rs`).
pub fn literals(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for literal in line.split('"').skip(1).step_by(2) {
            found.push((number + 1, literal.to_owned()));
        }
    }
    found
}

// BLAKE3, hash mode, written from the specification (one chunk = 1 024
// bytes, blocks of 64, the left subtree the largest power of two of
// chunks, the root flag on the last compression).

const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];
const PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const CHUNK_START: u32 = 1;
const CHUNK_END: u32 = 2;
const PARENT: u32 = 4;
const ROOT: u32 = 8;
const BLOCK_LEN: usize = 64;
const CHUNK_LEN: usize = 1024;

#[allow(clippy::too_many_arguments)]
fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(x);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(y);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn compress(cv: &[u32; 8], block: &[u32; 16], counter: u64, len: u32, flags: u32) -> [u32; 16] {
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        len,
        flags,
    ];
    let mut m = *block;
    for index in 0..7 {
        round(&mut state, &m);
        if index < 6 {
            let mut permuted = [0u32; 16];
            for (slot, &from) in permuted.iter_mut().zip(PERMUTATION.iter()) {
                *slot = m[from];
            }
            m = permuted;
        }
    }
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= cv[i];
    }
    state
}

fn words(block: &[u8]) -> [u32; 16] {
    let mut padded = [0u8; BLOCK_LEN];
    padded[..block.len()].copy_from_slice(block);
    let mut out = [0u32; 16];
    for (i, word) in out.iter_mut().enumerate() {
        *word = u32::from_le_bytes(padded[i * 4..i * 4 + 4].try_into().unwrap());
    }
    out
}

fn first8(state: [u32; 16]) -> [u32; 8] {
    state[..8].try_into().unwrap()
}

fn chunk_cv(chunk: &[u8], counter: u64, root: bool) -> [u32; 8] {
    let blocks = chunk.len().div_ceil(BLOCK_LEN).max(1);
    let mut cv = IV;
    for index in 0..blocks {
        let block = &chunk
            [(index * BLOCK_LEN).min(chunk.len())..((index + 1) * BLOCK_LEN).min(chunk.len())];
        let mut flags = 0;
        if index == 0 {
            flags |= CHUNK_START;
        }
        if index + 1 == blocks {
            flags |= CHUNK_END;
            if root {
                flags |= ROOT;
            }
        }
        cv = first8(compress(
            &cv,
            &words(block),
            counter,
            block.len() as u32,
            flags,
        ));
    }
    cv
}

fn subtree_cv(input: &[u8], counter: u64, root: bool) -> [u32; 8] {
    if input.len() <= CHUNK_LEN {
        return chunk_cv(input, counter, root);
    }
    let chunks = input.len().div_ceil(CHUNK_LEN);
    let left_chunks = 1usize << (usize::BITS - 1 - (chunks - 1).leading_zeros());
    let split = left_chunks * CHUNK_LEN;
    let left = subtree_cv(&input[..split], counter, false);
    let right = subtree_cv(&input[split..], counter + left_chunks as u64, false);
    let mut block = [0u32; 16];
    block[..8].copy_from_slice(&left);
    block[8..].copy_from_slice(&right);
    let flags = PARENT | if root { ROOT } else { 0 };
    first8(compress(&IV, &block, 0, BLOCK_LEN as u32, flags))
}

/// The lower-case hex BLAKE3 (32 bytes) of `input`.
pub fn blake3_hex(input: &[u8]) -> String {
    subtree_cv(input, 0, true)
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
