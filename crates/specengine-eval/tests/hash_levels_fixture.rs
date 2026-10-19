//! docs/features/hash-levels.md (5a-1) over `fixtures/hash-levels/` and its
//! `expected.json`:
//!
//! - TE-1: the unit inventory of every fixture file, every pair (an equal
//!   pair and a different pair per level and per kind, each naming its
//!   mutation) on LF and on an in-memory CRLF copy, every unit equal under
//!   CRLF, the call targets and `body_bytes` counted by hand;
//! - TE-3: every broken item and every broken file's module `CannotVerify`
//!   with v2's categories, no digest shared by two broken units;
//! - TE-6 (over TE-1's edits and the CRLF copies): `hash_item` changes
//!   exactly when `sig` or `body` changes, N5 (`v2: n5` pairs), N6
//!   (`v2: n6` pairs, docs/features/hash-levels-use-lists.md, 5a-1c: the
//!   members of a `use` list sorted) and the CR rule (`cr_exceptions`) the
//!   only exceptions.
//!
//! JSON is read here, in the eval crate, because `specengine-code` has no
//! JSON dependency (as RD-1's grammar pin). TE-2, TE-4, TE-5 and the recipe
//! oracles live in `crates/specengine-code/tests/hash_levels_*.rs`.
//!
//! PHASE-B: the 5a-1 entry points are used by the spec's names ("Entry
//! points"): `item_levels`, `module_levels`, `LevelState`, `LevelHashes`
//! (public fields `sig`, `body`, `deps`, `skeleton`, `quote`, `body_bytes`,
//! method `is_trivial`), `call_targets`, `LEVEL_RECIPE`,
//! `TRIVIAL_BODY_BYTES`, all at the crate root. `unit_of` is the only
//! function reading their shape.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::thread;

use serde_json::Value;
use specengine_code::hash::{self, Digest, ErrorCategory, HashState};
use specengine_code::items::{ITEM_KINDS, MEMBER_KINDS};
use specengine_code::{
    LEVEL_RECIPE, LevelState, RustParser, TRIVIAL_BODY_BYTES, analyze_tree, call_targets,
    item_levels, module_levels,
};

/// The walks of `broken/nesting.rs` recurse once per nested use run; the
/// depth itself is RD-7's subject, not this file's.
const STACK: usize = 16 * 1024 * 1024;

const LEVELS: [&str; 3] = ["sig", "body", "deps"];
const FIELDS: [&str; 6] = ["sig", "body", "deps", "skeleton", "quote", "body_bytes"];

// ---------------------------------------------------------------- the fixture

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/hash-levels")
}

fn expected() -> Value {
    let path = fixture_root().join("expected.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn read(relative: &str) -> String {
    let path = fixture_root().join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        !text.contains('\r'),
        "{relative}: fixture sources are LF only; the CRLF copy is made in memory"
    );
    text
}

/// Every `.rs` file of the fixture outside `variants/`, relative, sorted.
fn fixture_rs_files() -> Vec<String> {
    let root = fixture_root();
    let mut out = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            let relative = path
                .strip_prefix(&root)
                .expect("below the fixture root")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if relative != "variants" {
                    pending.push(path);
                }
            } else if relative.ends_with(".rs") {
                out.push(relative);
            }
        }
    }
    out.sort();
    out
}

fn crlf(source: &str) -> String {
    source.replace('\n', "\r\n")
}

/// `source` with `edit.from` (present exactly once) replaced by `edit.to`.
fn apply(source: &str, edit: &Value, what: &str) -> String {
    let from = edit["from"].as_str().expect("edit.from");
    let to = edit["to"].as_str().expect("edit.to");
    let count = source.matches(from).count();
    assert_eq!(
        count, 1,
        "{what}: edit.from {from:?} must occur exactly once, found {count}"
    );
    source.replacen(from, to, 1)
}

// ---------------------------------------------------------------- the analysis

/// One unit's levels and fingerprints, copied out of `LevelHashes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Levels {
    sig: Digest,
    body: Digest,
    deps: Digest,
    skeleton: Digest,
    quote: Digest,
    body_bytes: u32,
    trivial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Unit {
    Hashed(Levels),
    Refused(Vec<ErrorCategory>),
}

/// PHASE-B: the one place that reads the shape of `LevelState` and `LevelHashes`.
fn unit_of(state: &LevelState) -> Unit {
    match state {
        LevelState::Hashed(levels) => Unit::Hashed(Levels {
            sig: levels.sig,
            body: levels.body,
            deps: levels.deps,
            skeleton: levels.skeleton,
            quote: levels.quote,
            body_bytes: levels.body_bytes,
            trivial: levels.is_trivial(),
        }),
        LevelState::CannotVerify { categories } => Unit::Refused(categories.clone()),
    }
}

impl Unit {
    fn levels(&self, what: &str) -> Levels {
        match self {
            Self::Hashed(levels) => *levels,
            Self::Refused(categories) => {
                panic!("{what}: expected levels, got CannotVerify {categories:?}")
            }
        }
    }
}

/// A field of `Levels` as bytes, so digests and counts compare alike.
fn field(levels: &Levels, name: &str) -> Vec<u8> {
    match name {
        "sig" => levels.sig.as_bytes().to_vec(),
        "body" => levels.body.as_bytes().to_vec(),
        "deps" => levels.deps.as_bytes().to_vec(),
        "skeleton" => levels.skeleton.as_bytes().to_vec(),
        "quote" => levels.quote.as_bytes().to_vec(),
        "body_bytes" => levels.body_bytes.to_le_bytes().to_vec(),
        other => panic!("unknown level or fingerprint {other:?} in expected.json"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    kind: String,
    label: String,
    owner: Option<String>,
    mod_path: Vec<String>,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.kind, self.label)?;
        if let Some(owner) = &self.owner {
            write!(f, " (in {owner})")?;
        }
        if !self.mod_path.is_empty() {
            write!(f, " (mod {})", self.mod_path.join("::"))?;
        }
        Ok(())
    }
}

/// Everything the tests read about one source text.
struct Analyzed {
    keys: Vec<Key>,
    /// `analyze_tree`'s v2 states (`hash_item`), index for index.
    v2: Vec<HashState>,
    units: Vec<Unit>,
    module: Unit,
    targets: Vec<Vec<String>>,
    root_has_error: bool,
    root_categories: Vec<ErrorCategory>,
}

impl Analyzed {
    fn new(source: String) -> Self {
        thread::Builder::new()
            .stack_size(STACK)
            .spawn(move || Self::on_this_thread(&source))
            .expect("analysis thread spawns")
            .join()
            .expect("the analysis must not panic")
    }

    fn on_this_thread(source: &str) -> Self {
        let tree = RustParser::new()
            .expect("grammar loads")
            .parse(source)
            .expect("parse completes");
        let analysis = analyze_tree(&tree, source);
        let indexes: Vec<usize> = (0..analysis.items.len()).collect();
        let states = item_levels(&tree, source, &indexes);
        assert_eq!(states.len(), indexes.len(), "one entry per index asked");
        let units: Vec<Unit> = states
            .iter()
            .enumerate()
            .map(|(index, state)| {
                unit_of(
                    state
                        .as_ref()
                        .unwrap_or_else(|| panic!("item {index} of the analysis has no entry")),
                )
            })
            .collect();
        // The twin follows the indexes asked: reversed, then one past the end.
        let mut reversed = indexes.clone();
        reversed.reverse();
        reversed.push(indexes.len());
        let back = item_levels(&tree, source, &reversed);
        assert_eq!(back.len(), reversed.len(), "one entry per index asked");
        assert!(
            back.last().is_some_and(Option::is_none),
            "None past the analysis' items"
        );
        for (position, &index) in reversed.iter().enumerate().take(indexes.len()) {
            let again = back[position].as_ref().map(unit_of);
            assert_eq!(
                again.as_ref(),
                Some(&units[index]),
                "item {index} asked in reverse order"
            );
        }
        let root = tree.root_node();
        let targets = analysis
            .items
            .iter()
            .map(|item| {
                let node = root
                    .descendant_for_byte_range(item.byte_range.start, item.byte_range.end)
                    .expect("the item's node");
                assert_eq!(node.kind(), item.kind, "the node of {}", item.label);
                assert_eq!(
                    node.byte_range(),
                    item.byte_range,
                    "the node of {}",
                    item.label
                );
                call_targets(node, source)
            })
            .collect();
        Self {
            keys: analysis
                .items
                .iter()
                .map(|item| Key {
                    kind: item.kind.clone(),
                    label: item.label.clone(),
                    owner: item.owner.clone(),
                    mod_path: item.mod_path.clone(),
                })
                .collect(),
            v2: analysis
                .items
                .iter()
                .map(|item| item.state.clone())
                .collect(),
            units,
            module: unit_of(&module_levels(&tree, source)),
            targets,
            root_has_error: root.has_error(),
            root_categories: hash::error_categories(root, source.as_bytes()),
        }
    }
}

/// A unit of a file: one of its items, or its module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum At {
    Item(usize),
    Module,
}

fn locator(value: &Value) -> Option<Key> {
    if value["module"].as_bool() == Some(true) {
        return None;
    }
    Some(Key {
        kind: value["kind"].as_str().expect("locator.kind").to_owned(),
        label: value["label"].as_str().expect("locator.label").to_owned(),
        owner: value["owner"].as_str().map(str::to_owned),
        mod_path: value["mod_path"]
            .as_array()
            .map(|path| {
                path.iter()
                    .map(|s| s.as_str().expect("mod_path segment").to_owned())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn find(analyzed: &Analyzed, value: &Value, what: &str) -> At {
    let Some(key) = locator(value) else {
        return At::Module;
    };
    let hits: Vec<usize> = analyzed
        .keys
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == key)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{what}: locator {key} must name exactly one item, names {hits:?} among {:?}",
        analyzed
            .keys
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    At::Item(hits[0])
}

fn unit(analyzed: &Analyzed, at: At) -> &Unit {
    match at {
        At::Item(index) => &analyzed.units[index],
        At::Module => &analyzed.module,
    }
}

fn v2_digest(analyzed: &Analyzed, index: usize, what: &str) -> Digest {
    match &analyzed.v2[index] {
        HashState::Hashed(digest) => *digest,
        other => panic!("{what}: expected a v2 digest, got {other:?}"),
    }
}

fn relation(equal: bool) -> &'static str {
    if equal { "equal" } else { "differs" }
}

// ---------------------------------------------------------------- TE-1

#[test]
fn te1_level_constants_are_the_spec_values() {
    let expected = expected();
    assert_eq!(LEVEL_RECIPE, expected["level_recipe"].as_str().unwrap());
    assert_eq!(LEVEL_RECIPE, "specengine-hash/v3");
    assert_ne!(LEVEL_RECIPE, hash::RECIPE, "v3 beside v2, never instead");
    assert_eq!(
        u64::from(TRIVIAL_BODY_BYTES),
        expected["trivial_body_bytes"].as_u64().unwrap()
    );
    assert_eq!(TRIVIAL_BODY_BYTES, 124);
}

#[test]
fn te1_inventory_lists_every_unit_of_every_fixture_file() {
    let expected = expected();
    let files = expected["files"].as_object().expect("files");
    let listed: Vec<String> = files.keys().cloned().collect();
    assert_eq!(
        listed,
        fixture_rs_files(),
        "expected.json `files` must list every .rs file outside variants/"
    );
    let mut failures = Vec::new();
    for (file, entry) in files {
        let analyzed = Analyzed::new(read(file));
        let items = entry["items"].as_array().expect("files.*.items");
        let wanted: Vec<Key> = items
            .iter()
            .map(|item| locator(item).expect("an item locator"))
            .collect();
        if wanted != analyzed.keys {
            failures.push(format!(
                "{file}: items {:?}, expected {:?}",
                analyzed
                    .keys
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                wanted.iter().map(ToString::to_string).collect::<Vec<_>>()
            ));
            continue;
        }
        for (index, item) in items.iter().enumerate() {
            let refused = item["state"].as_str() == Some("cannot_verify");
            let got = &analyzed.units[index];
            if refused != matches!(got, Unit::Refused(_)) {
                failures.push(format!(
                    "{file}: {} expected {}, got {got:?}",
                    analyzed.keys[index],
                    if refused { "cannot_verify" } else { "hashed" }
                ));
            }
            // The v2 state of the same analysis agrees on refusal.
            if refused != matches!(analyzed.v2[index], HashState::CannotVerify { .. }) {
                failures.push(format!(
                    "{file}: {} levels and v2 disagree on refusal: v2 {:?}",
                    analyzed.keys[index], analyzed.v2[index]
                ));
            }
        }
        let module_refused = entry["module"].as_str() == Some("cannot_verify");
        if module_refused != matches!(analyzed.module, Unit::Refused(_)) {
            failures.push(format!(
                "{file}: module expected {}, got {:?}",
                entry["module"], analyzed.module
            ));
        }
    }
    assert!(failures.is_empty(), "inventory:\n{}", failures.join("\n"));
}

/// Every pair holds on LF and on the CRLF copy of both sides.
#[test]
fn te1_pairs_hold_on_lf_and_on_crlf() {
    let expected = expected();
    let mut failures = Vec::new();
    let mut ids = BTreeSet::new();
    for pair in expected["pairs"].as_array().expect("pairs") {
        let id = pair["id"].as_str().expect("pair.id");
        assert!(ids.insert(id.to_owned()), "pair id {id} is not unique");
        assert!(
            pair["mutation"]
                .as_str()
                .is_some_and(|m| !m.trim().is_empty()),
            "pair {id} names no mutation"
        );
        let file = pair["file"].as_str().expect("pair.file");
        let source_a = read(file);
        let source_b = match pair.get("edit") {
            Some(edit) => apply(&source_a, edit, id),
            None => source_a.clone(),
        };
        let b_locator = pair.get("b").unwrap_or(&pair["a"]);
        for (mode, a_text, b_text) in [
            ("LF", source_a.clone(), source_b.clone()),
            ("CRLF", crlf(&source_a), crlf(&source_b)),
        ] {
            let analyzed_a = Analyzed::new(a_text);
            let analyzed_b = Analyzed::new(b_text);
            let at_a = find(&analyzed_a, &pair["a"], id);
            let at_b = find(&analyzed_b, b_locator, id);
            if pair.get("edit").is_none() {
                assert_ne!(at_a, at_b, "{id}: a pair without an edit names two units");
            }
            let a = unit(&analyzed_a, at_a).levels(&format!("{id} ({mode}) a"));
            let b = unit(&analyzed_b, at_b).levels(&format!("{id} ({mode}) b"));
            for (name, want) in pair["expect"].as_object().expect("pair.expect") {
                let want = want.as_str().expect("equal or differs");
                assert!(matches!(want, "equal" | "differs"), "{id}: {want:?}");
                let got = relation(field(&a, name) == field(&b, name));
                if got != want {
                    failures.push(format!(
                        "{id} ({mode}): {name} {got}, expected {want} -- mutation: {}",
                        pair["mutation"].as_str().unwrap_or_default()
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "pairs:\n{}", failures.join("\n"));
}

/// Per level and per kind (every `ITEM_KINDS` and `MEMBER_KINDS` kind, and
/// the file module): an equal pair and a different pair, unless the level
/// cannot differ for the kind (`no_differs_pair`).
#[test]
fn te1_pairs_cover_every_level_of_every_kind() {
    let expected = expected();
    let mut kinds: BTreeSet<&str> = ITEM_KINDS.iter().chain(MEMBER_KINDS).copied().collect();
    kinds.insert("source_file");
    let mut constant: BTreeSet<(String, String)> = BTreeSet::new();
    for entry in expected["no_differs_pair"]
        .as_array()
        .expect("no_differs_pair")
    {
        let kind = entry["kind"].as_str().expect("kind");
        for level in entry["levels"].as_array().expect("levels") {
            constant.insert((kind.to_owned(), level.as_str().unwrap().to_owned()));
        }
    }
    let mut seen: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
    for pair in expected["pairs"].as_array().expect("pairs") {
        let id = pair["id"].as_str().unwrap();
        let kind = pair["kind"].as_str().expect("pair.kind");
        assert!(kinds.contains(kind), "{id}: kind {kind} is not collected");
        match locator(&pair["a"]) {
            Some(key) => assert_eq!(key.kind, kind, "{id}: pair.kind is a's kind"),
            None => assert_eq!(kind, "source_file", "{id}: a module pair"),
        }
        for (name, want) in pair["expect"].as_object().unwrap() {
            seen.entry((
                kind.to_owned(),
                name.clone(),
                want.as_str().unwrap().to_owned(),
            ))
            .or_default()
            .push(id.to_owned());
        }
    }
    let mut missing = Vec::new();
    for kind in &kinds {
        for level in LEVELS {
            for want in ["equal", "differs"] {
                let key = (kind.to_string(), level.to_owned(), want.to_owned());
                let impossible =
                    want == "differs" && constant.contains(&(kind.to_string(), level.to_owned()));
                match (seen.contains_key(&key), impossible) {
                    (false, false) => missing.push(format!("{kind} {level} {want}")),
                    (true, true) => missing.push(format!(
                        "{kind} {level} is listed in no_differs_pair yet pairs {:?} differ",
                        seen[&key]
                    )),
                    _ => {}
                }
            }
        }
    }
    for fingerprint in ["skeleton", "quote", "body_bytes"] {
        for want in ["equal", "differs"] {
            if !seen
                .keys()
                .any(|(_, name, rel)| name == fingerprint && rel == want)
            {
                missing.push(format!("no pair with {fingerprint} {want}"));
            }
        }
    }
    assert!(missing.is_empty(), "coverage:\n{}", missing.join("\n"));
}

/// The CR rule: every unit of every fixture file has the same levels and
/// fingerprints, refusals the same categories, on the CRLF copy.
#[test]
fn te1_every_unit_is_equal_on_the_crlf_copy() {
    let mut failures = Vec::new();
    for file in fixture_rs_files() {
        let source = read(&file);
        let lf = Analyzed::new(source.clone());
        let cr = Analyzed::new(crlf(&source));
        assert_eq!(lf.keys, cr.keys, "{file}: the same items on CRLF");
        for (index, key) in lf.keys.iter().enumerate() {
            if lf.units[index] != cr.units[index] {
                failures.push(format!(
                    "{file}: {key}: LF {:?} vs CRLF {:?}",
                    lf.units[index], cr.units[index]
                ));
            }
        }
        if lf.module != cr.module {
            failures.push(format!(
                "{file}: module LF {:?} vs CRLF {:?}",
                lf.module, cr.module
            ));
        }
    }
    assert!(failures.is_empty(), "CRLF:\n{}", failures.join("\n"));
}

#[test]
fn te1_call_targets_and_body_bytes_are_the_hand_counted_ones() {
    let expected = expected();
    let mut failures = Vec::new();
    let mut cache: BTreeMap<String, Analyzed> = BTreeMap::new();
    for entry in expected["call_targets"].as_array().expect("call_targets") {
        let file = entry["file"].as_str().unwrap();
        let analyzed = cache
            .entry(file.to_owned())
            .or_insert_with(|| Analyzed::new(read(file)));
        let At::Item(index) = find(analyzed, &entry["unit"], file) else {
            panic!("call_targets names items only");
        };
        let want: Vec<String> = entry["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_owned())
            .collect();
        let mut sorted = want.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            want, sorted,
            "{file}: expected.json lists targets byte-sorted, deduplicated"
        );
        if analyzed.targets[index] != want {
            failures.push(format!(
                "{file}: {}: call_targets {:?}, expected {want:?}",
                analyzed.keys[index], analyzed.targets[index]
            ));
        }
    }
    let threshold = u32::try_from(expected["trivial_body_bytes"].as_u64().unwrap()).unwrap();
    for entry in expected["body_bytes"].as_array().expect("body_bytes") {
        let file = entry["file"].as_str().unwrap();
        let want = u32::try_from(entry["body_bytes"].as_u64().unwrap()).unwrap();
        for (mode, text) in [("LF", read(file)), ("CRLF", crlf(&read(file)))] {
            let analyzed = Analyzed::new(text);
            let at = find(&analyzed, &entry["unit"], file);
            let levels = unit(&analyzed, at).levels(file);
            if levels.body_bytes != want {
                failures.push(format!(
                    "{file} ({mode}): {:?}: body_bytes {}, expected {want}",
                    entry["unit"], levels.body_bytes
                ));
            }
            if levels.trivial != (want < threshold) {
                failures.push(format!(
                    "{file} ({mode}): {:?}: is_trivial {} at {want} bytes",
                    entry["unit"], levels.trivial
                ));
            }
        }
    }
    assert!(failures.is_empty(), "hand counts:\n{}", failures.join("\n"));
}

// ---------------------------------------------------------------- TE-3

fn category_names(categories: &[ErrorCategory]) -> Vec<&'static str> {
    categories.iter().map(|c| c.as_str()).collect()
}

#[test]
fn te3_broken_units_cannot_verify_with_v2_categories_and_share_no_digest() {
    let expected = expected();
    let mut failures = Vec::new();
    let mut leaked: Vec<(String, Levels)> = Vec::new();
    let mut listed: BTreeSet<(String, String)> = BTreeSet::new();
    for entry in expected["broken"].as_array().expect("broken") {
        let file = entry["file"].as_str().unwrap();
        let analyzed = Analyzed::new(read(file));
        let at = find(&analyzed, &entry["unit"], file);
        let name = match at {
            At::Item(index) => analyzed.keys[index].to_string(),
            At::Module => "module".to_owned(),
        };
        listed.insert((file.to_owned(), name.clone()));
        let categories = match unit(&analyzed, at) {
            Unit::Refused(categories) => categories.clone(),
            Unit::Hashed(levels) => {
                failures.push(format!("{file}: {name} is broken but hashed"));
                leaked.push((format!("{file}: {name}"), *levels));
                continue;
            }
        };
        if categories.is_empty() {
            failures.push(format!("{file}: {name}: CannotVerify names no category"));
        }
        if !categories.is_sorted() {
            failures.push(format!(
                "{file}: {name}: categories unsorted {categories:?}"
            ));
        }
        // v2's categories: `hash_item`'s for an item; for a module,
        // `error_categories(root)`, or `[NestingTooDeep]` without a parse error.
        let v2: Vec<ErrorCategory> = match at {
            At::Item(index) => match &analyzed.v2[index] {
                HashState::CannotVerify { categories } => categories.clone(),
                other => panic!("{file}: {name}: v2 state {other:?}"),
            },
            At::Module if analyzed.root_has_error => analyzed.root_categories.clone(),
            At::Module => vec![ErrorCategory::NestingTooDeep],
        };
        if categories != v2 {
            failures.push(format!(
                "{file}: {name}: categories {:?}, v2's {:?}",
                category_names(&categories),
                category_names(&v2)
            ));
        }
        let names = category_names(&categories);
        if let Some(exact) = entry["categories"].as_array() {
            let want: Vec<&str> = exact.iter().map(|c| c.as_str().unwrap()).collect();
            if names != want {
                failures.push(format!("{file}: {name}: {names:?}, expected {want:?}"));
            }
        }
        if let Some(include) = entry["categories_include"].as_array() {
            for category in include {
                let category = category.as_str().unwrap();
                if !names.contains(&category) {
                    failures.push(format!("{file}: {name}: {names:?} lacks {category}"));
                }
            }
        }
    }
    // Every refused unit of the inventory is listed here, and nothing else.
    for (file, entry) in expected["files"].as_object().unwrap() {
        for item in entry["items"].as_array().unwrap() {
            if item["state"].as_str() == Some("cannot_verify") {
                let key = locator(item).unwrap().to_string();
                assert!(
                    listed.contains(&(file.clone(), key.clone())),
                    "{file}: {key} is cannot_verify in `files` but not in `broken`"
                );
            }
        }
        if entry["module"].as_str() == Some("cannot_verify") {
            assert!(
                listed.contains(&(file.clone(), "module".to_owned())),
                "{file}: its module is cannot_verify in `files` but not in `broken`"
            );
        }
    }
    // The trap (an `is_extra()` filter): broken units that leaked a digest
    // must at least not share it.
    for (i, (a, la)) in leaked.iter().enumerate() {
        for (b, lb) in &leaked[i + 1..] {
            for name in FIELDS {
                if field(la, name) == field(lb, name) {
                    failures.push(format!("{a} and {b} share {name}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "TE-3:\n{}", failures.join("\n"));
}

// ---------------------------------------------------------------- TE-6

/// Over every TE-1 edit of an item: v2 changes exactly when `sig` or `body`
/// changes, the `v2: n5` and `v2: n6` pairs the only exceptions (v2
/// changes, levels hold); each kind of exception occurs.
#[test]
fn te6_v2_changes_exactly_with_sig_or_body_over_the_pairs() {
    let expected = expected();
    let mut failures = Vec::new();
    let mut exceptions: BTreeMap<String, usize> = BTreeMap::new();
    for pair in expected["pairs"].as_array().unwrap() {
        let id = pair["id"].as_str().unwrap();
        if locator(&pair["a"]).is_none() {
            continue; // a file module has no v2 hash
        }
        let file = pair["file"].as_str().unwrap();
        let source_a = read(file);
        let source_b = match pair.get("edit") {
            Some(edit) => apply(&source_a, edit, id),
            None => source_a.clone(),
        };
        let analyzed_a = Analyzed::new(source_a);
        let analyzed_b = Analyzed::new(source_b);
        let At::Item(ia) = find(&analyzed_a, &pair["a"], id) else {
            unreachable!()
        };
        let At::Item(ib) = find(&analyzed_b, pair.get("b").unwrap_or(&pair["a"]), id) else {
            unreachable!()
        };
        let a = analyzed_a.units[ia].levels(id);
        let b = analyzed_b.units[ib].levels(id);
        let v2_changed = v2_digest(&analyzed_a, ia, id) != v2_digest(&analyzed_b, ib, id);
        let levels_changed = a.sig != b.sig || a.body != b.body;
        if let Some(rule) = pair["v2"].as_str() {
            assert!(
                matches!(rule, "n5" | "n6"),
                "{id}: `v2` names n5 or n6, got {rule:?}"
            );
            *exceptions.entry(rule.to_owned()).or_default() += 1;
            if !v2_changed || levels_changed {
                failures.push(format!(
                    "{id}: an {} exception must change v2 ({v2_changed}) and keep sig and body ({})",
                    rule.to_uppercase(),
                    !levels_changed
                ));
            }
        } else if v2_changed != levels_changed {
            failures.push(format!(
                "{id}: v2 changed {v2_changed}, sig or body changed {levels_changed} -- a leaf in exactly one of the two"
            ));
        }
    }
    for rule in ["n5", "n6"] {
        assert!(
            exceptions.get(rule).is_some_and(|n| *n > 0),
            "the fixture names its {rule} exceptions on items: {exceptions:?}"
        );
    }
    assert!(failures.is_empty(), "TE-6:\n{}", failures.join("\n"));
}

/// Over every item on its CRLF copy: v2 holds except for the named
/// multi-line literals, whose v2 changes while every level holds.
#[test]
fn te6_crlf_changes_v2_only_for_the_named_multiline_literals() {
    let expected = expected();
    let mut named: BTreeSet<(String, String)> = BTreeSet::new();
    for entry in expected["cr_exceptions"].as_array().expect("cr_exceptions") {
        let key = locator(&entry["unit"]).expect("an item").to_string();
        named.insert((entry["file"].as_str().unwrap().to_owned(), key));
    }
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();
    for file in fixture_rs_files() {
        let source = read(&file);
        let lf = Analyzed::new(source.clone());
        let cr = Analyzed::new(crlf(&source));
        for (index, key) in lf.keys.iter().enumerate() {
            let (HashState::Hashed(v2_lf), HashState::Hashed(v2_cr)) =
                (&lf.v2[index], &cr.v2[index])
            else {
                continue; // refused on both: TE-3's subject
            };
            let exception = named.contains(&(file.clone(), key.to_string()));
            if exception {
                seen.insert((file.clone(), key.to_string()));
            }
            match (v2_lf == v2_cr, exception) {
                (true, false) => {}
                (false, true) if lf.units[index] == cr.units[index] => {}
                (false, true) => failures.push(format!(
                    "{file}: {key}: a CR exception whose levels changed too"
                )),
                (true, true) => failures.push(format!(
                    "{file}: {key}: named a CR exception, but v2 holds on CRLF (no multi-line leaf?)"
                )),
                (false, false) => failures.push(format!(
                    "{file}: {key}: v2 changed on CRLF and is no named exception"
                )),
            }
        }
    }
    assert_eq!(
        seen, named,
        "every named CR exception exists in the fixture"
    );
    assert!(failures.is_empty(), "TE-6 CRLF:\n{}", failures.join("\n"));
}
