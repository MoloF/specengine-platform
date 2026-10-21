//! docs/features/norm-hash.md through the `spec` binary ("spec show and
//! mirrors", "Index"):
//!
//! - TE-1: the five golden `b3:` hashes (MEC-STAMINA, RULE-STAM-REGEN,
//!   RULE-SPRINT-COST, MEC-SPRINT of spec-a, FLAG-DRY-RUN of spec-b), each
//!   the BLAKE3 (the tests' own implementation) of core's `norm_input`,
//!   the store's `b3_hash` of it, and what `spec show` prints. The input
//!   bytes are pinned by hand in core's `norm_recipe.rs`.
//! - TE-7: exact headers, ` | norm` right before ` | span`, omitted when
//!   null; the JSON keys in order; show = store = core over every ID of
//!   spec-a and spec-b; `bundle` text and `bundle_hash` byte-identical to
//!   before the change (pinned from the pre-change binary). M: `norm`
//!   added to `header()`.
//! - TE-4 (CLI half): a null document and a file that is not UTF-8 have
//!   no ` | norm` and a JSON `null`.
//! - TE-9: two roots, another cwd, reverse creation order: identical
//!   hashes, in `show` and in the index.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::bundle::{blake3_hex, split_text};
use common::{
    NORM_SUFFIX, SPAN_SUFFIX, Scratch, data_dir, fixture, index, read, read_text, snapshot, spec,
    write,
};
use serde_json::Value;
use specengine_core::ProjectConfig;
use specengine_model::ParsedFile;

const STAMINA: &str = "docs/spec/movement/stamina.md";
const SPRINT: &str = "docs/spec/movement/sprint.md";
const CLI: &str = "docs/spec/cli.md";

/// The golden hashes of TE-1 (computed from the hand-built inputs of
/// `norm_recipe.rs` by three BLAKE3 implementations: this crate's tests',
/// the `blake3` crate's, and one outside the repository).
const GOLDEN: [(&str, &str, &str, &str); 5] = [
    (
        "spec-a",
        STAMINA,
        "MEC-STAMINA",
        "b3:d28068144ffb79d558aa83cb5da6756bf2e5284b685d7df96671f2091218ce06",
    ),
    (
        "spec-a",
        STAMINA,
        "RULE-STAM-REGEN",
        "b3:613edf30c8b48316017d34ed06c3f83fe2a7ba27c294f09afe48467771e7c014",
    ),
    (
        "spec-a",
        SPRINT,
        "RULE-SPRINT-COST",
        "b3:bb97b1d5b795143baa4c6e9b3cfc3d7f7c798239339680cf8ed0f3af1623ca4d",
    ),
    (
        "spec-a",
        SPRINT,
        "MEC-SPRINT",
        "b3:1a1fd1318af16cc454a8630747054d51d8a72ba11b33873c0a02f2896f5cdcae",
    ),
    (
        "spec-b",
        CLI,
        "FLAG-DRY-RUN",
        "b3:f7564300dc534d7507947e7935b594a9a24ce15120da98dede9239d0e9f503eb",
    ),
];

fn parse(root: &Path, path: &str) -> ParsedFile {
    let config = ProjectConfig::from_toml(&read_text(root, "specengine.toml")).expect("config");
    specengine_core::parse(path, &read(root, path), &config.scheme)
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3_hex(bytes))
}

/// Core's hash of node `id` of `path`: the tests' BLAKE3 of `norm_input`.
fn core_norm(root: &Path, path: &str, id: &str) -> Option<String> {
    let bytes = read(root, path);
    let parsed = parse(root, path);
    let ord = parsed
        .nodes
        .iter()
        .position(|node| node.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} in {path}"));
    specengine_core::norm_input(&parsed, &bytes, ord).map(|input| b3(&input))
}

/// `spec show REF`: the header line and the JSON's first node.
fn show(home: &Path, root: &Path, reference: &str) -> (String, Value) {
    let text = spec(home, root, &["show", reference]);
    text.code(0);
    let header = text.stdout.lines().next().expect("a header").to_owned();
    let json = spec(home, root, &["show", reference, "--json"]);
    json.code(0);
    (header, json.json()["nodes"][0].clone())
}

/// The `| norm` field of a header, checked to sit right before the span
/// suffix, which ends the line.
fn header_norm(header: &str) -> Option<String> {
    let (before, span) = header
        .rsplit_once(SPAN_SUFFIX)
        .unwrap_or_else(|| panic!("no span suffix: {header}"));
    assert_eq!(span.len(), 64, "{header}");
    assert_eq!(
        header.matches(" | norm ").count(),
        usize::from(before.contains(NORM_SUFFIX)),
        "at most one norm field, only right before the span: {header}"
    );
    before
        .rsplit_once(NORM_SUFFIX)
        .map(|(_, hex)| format!("b3:{hex}"))
        .inspect(|norm| assert_eq!(norm.len(), 67, "{header}"))
}

/// The keys of every object of a compact JSON text, in written order,
/// with the object's depth (1 the top level), in the order objects open.
fn object_keys(text: &str) -> Vec<(usize, Vec<String>)> {
    let bytes = text.as_bytes();
    let mut out: Vec<(usize, Vec<String>)> = Vec::new();
    let mut stack: Vec<Option<usize>> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'{' => {
                out.push((stack.len() + 1, Vec::new()));
                stack.push(Some(out.len() - 1));
            }
            b'[' => stack.push(None),
            b'}' | b']' => {
                stack.pop();
            }
            b'"' => {
                let start = at + 1;
                at += 1;
                while bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
                let string = &text[start..at];
                if bytes.get(at + 1) == Some(&b':')
                    && let Some(Some(object)) = stack.last()
                {
                    out[*object].1.push(string.to_owned());
                }
            }
            _ => {}
        }
        at += 1;
    }
    out
}

/// 1-based line of byte `offset` of `bytes`.
fn line_of(bytes: &[u8], offset: usize) -> usize {
    bytes[..offset]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

// ------------------------------------------------------------------ TE-1

/// TE-1: literal = the tests' BLAKE3 of core's input = the store's
/// `b3_hash` = `spec show`'s header and JSON.
#[test]
fn te1_golden_hashes_are_cores_stores_and_shows() {
    let scratch = Scratch::new("norm-golden");
    let home = scratch.home("h");
    let mut roots = BTreeMap::new();
    for (fixture_name, _, _, _) in GOLDEN {
        roots
            .entry(fixture_name)
            .or_insert_with(|| scratch.copy(fixture_name, fixture_name));
    }
    for (fixture_name, path, id, golden) in GOLDEN {
        let root = &roots[fixture_name];
        assert_eq!(
            core_norm(root, path, id).as_deref(),
            Some(golden),
            "{id}: core"
        );
        let bytes = read(root, path);
        let parsed = parse(root, path);
        let ord = parsed
            .nodes
            .iter()
            .position(|node| node.id.as_deref() == Some(id))
            .unwrap();
        let input = specengine_core::norm_input(&parsed, &bytes, ord).unwrap();
        assert_eq!(
            specengine_store::b3_hash(&input),
            golden,
            "{id}: the store's b3_hash"
        );
        let (header, node) = show(&home, root, id);
        assert_eq!(
            header_norm(&header).as_deref(),
            Some(golden),
            "{id}: {header}"
        );
        assert_eq!(node["norm_hash"], golden, "{id}: JSON");
    }
}

// ------------------------------------------------------------------ TE-7

/// TE-7: the spec's header of RULE-SPRINT-COST exactly, a document's with
/// its status, spec-b's nested FLAG-DRY-RUN with its `rev 2`.
#[test]
fn te7_exact_headers_norm_right_before_span() {
    let scratch = Scratch::new("norm-headers");
    let home = scratch.home("h");
    let a = scratch.copy("spec-a", "a");
    let b = scratch.copy("spec-b", "b");
    let cases = [
        (
            &a,
            SPRINT,
            "RULE-SPRINT-COST",
            "RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:18 | {tokens} tokens | rev 3 | norm b3:bb97b1d5b795143baa4c6e9b3cfc3d7f7c798239339680cf8ed0f3af1623ca4d | span {span}",
        ),
        (
            &a,
            STAMINA,
            "MEC-STAMINA",
            "MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | {tokens} tokens | status accepted | norm b3:d28068144ffb79d558aa83cb5da6756bf2e5284b685d7df96671f2091218ce06 | span {span}",
        ),
        (
            &b,
            CLI,
            "FLAG-DRY-RUN",
            "FLAG-DRY-RUN | flag | \u{0424}\u{043b}\u{0430}\u{0433} --dry-run | docs/spec/cli.md:{line} | {tokens} tokens | rev 2 | norm b3:f7564300dc534d7507947e7935b594a9a24ce15120da98dede9239d0e9f503eb | span {span}",
        ),
    ];
    for (root, path, id, template) in cases {
        let bytes = read(root, path);
        let parsed = parse(root, path);
        let node = parsed
            .nodes
            .iter()
            .find(|node| node.id.as_deref() == Some(id))
            .unwrap();
        let span = b3(&bytes[node.span.range()]);
        let want = template
            .replace("{tokens}", &node.tokens_est.to_string())
            .replace("{span}", &span)
            .replace("{line}", &line_of(&bytes, node.span.start).to_string());
        let (header, _) = show(&home, root, id);
        assert_eq!(header, want, "{id}");
    }
}

/// TE-7: the JSON's top-level keys and its node's keys, in written order:
/// `norm_hash` between `sections` and `span_hash`, a string or `null`.
#[test]
fn te7_json_keys_in_order() {
    let scratch = Scratch::new("norm-keys");
    let home = scratch.home("h");
    let a = scratch.copy("spec-a", "a");
    let node_keys = [
        "id",
        "kind",
        "title",
        "path",
        "line",
        "end_line",
        "status",
        "rev",
        "tokens_est",
        "archived",
        "utf8",
        "sections",
        "norm_hash",
        "span_hash",
        "text",
        "truncated",
        "omitted",
        "links",
        // docs/features/show-bindings.md "spec show --bindings": last.
        "bindings",
    ];
    for reference in [
        "MEC-STAMINA",
        "RULE-SPRINT-COST",
        // A null document: the key is there, `null`.
        "docs/features/stamina-tuning.md",
    ] {
        let run = spec(&home, &a, &["show", reference, "--json"]);
        run.code(0);
        let objects = object_keys(run.stdout.trim_end());
        assert_eq!(
            objects[0],
            (
                1,
                ["ref", "reason", "notes", "nodes"]
                    .map(str::to_owned)
                    .to_vec()
            ),
            "{reference}: the top level (no norm key there)"
        );
        let node = objects
            .iter()
            .find(|(depth, _)| *depth == 3)
            .unwrap_or_else(|| panic!("{reference}: a node object"));
        assert_eq!(node.1, node_keys.map(str::to_owned), "{reference}");
        let json = run.json();
        let norm = &json["nodes"][0]["norm_hash"];
        if reference.ends_with(".md") {
            assert!(norm.is_null(), "{reference}: {norm}");
        } else {
            assert!(norm.as_str().is_some_and(|hash| hash.starts_with("b3:")));
        }
    }
}

/// `(path, ord)` -> `(id, norm_hash)` of an index dump.
type StoredRows = BTreeMap<(String, u64), (Option<String>, Value)>;

/// `(path, ord)` -> `(id, norm_hash)` of the CLI's own index of `root`,
/// read through the store's canonical dump.
fn stored(home: &Path, root: &Path, slug: &str) -> StoredRows {
    let db = data_dir(home).join(format!("{slug}.db"));
    let store = specengine_store::SqliteIndex::open(&db, slug, root).expect("open the index");
    let dump = store.dump().expect("dump");
    let mut rows = BTreeMap::new();
    for line in dump.lines() {
        if let Some(json) = line.strip_prefix("nodes\t") {
            let row: Vec<Value> = serde_json::from_str(json).expect("a row");
            assert_eq!(row.len(), 12, "{line}");
            rows.insert(
                (
                    row[2].as_str().unwrap().to_owned(),
                    row[3].as_u64().unwrap(),
                ),
                (row[5].as_str().map(str::to_owned), row[10].clone()),
            );
        }
    }
    rows
}

/// TE-7: over every ID of spec-a and spec-b, what `spec show` prints (text
/// and JSON), what the index stores and what core computes are one hash
/// (or one null); the paths of the ID-less and null documents too.
#[test]
fn te7_show_store_and_core_agree_over_every_id() {
    for (fixture_name, slug, null_docs) in [
        (
            "spec-a",
            "lantern-keep",
            ["docs/features/stamina-tuning.md"],
        ),
        ("spec-b", "zerkalo", ["docs/records/QN/QN-08.md"]),
    ] {
        let scratch = Scratch::new("norm-every");
        let home = scratch.home("h");
        let root = scratch.copy(fixture_name, "copy");
        let before = snapshot(&root);
        index(&home, &root);
        let rows = stored(&home, &root, slug);
        let mut ids = 0;
        let mut hashed = 0;
        for ((path, ord), (id, stored_norm)) in &rows {
            let reference = match (id, ord) {
                (Some(id), _) => id.clone(),
                (None, 0) => path.clone(),
                (None, _) => continue,
            };
            let bytes = read(&root, path);
            let parsed = parse(&root, path);
            let core =
                specengine_core::norm_input(&parsed, &bytes, *ord as usize).map(|input| b3(&input));
            let (header, node) = show(&home, &root, &reference);
            assert_eq!(node["path"], path.as_str(), "{reference}");
            let json_norm = node["norm_hash"].as_str().map(str::to_owned);
            assert_eq!(json_norm, core, "{fixture_name} {reference}: JSON = core");
            assert_eq!(
                stored_norm.as_str().map(str::to_owned),
                core,
                "{fixture_name} {reference}: store = core"
            );
            assert_eq!(
                header_norm(&header),
                core,
                "{fixture_name} {reference}: text = core"
            );
            ids += usize::from(id.is_some());
            hashed += usize::from(core.is_some());
        }
        for path in null_docs {
            let (header, node) = show(&home, &root, path);
            assert!(!header.contains(" | norm "), "{path}: {header}");
            assert!(node["norm_hash"].is_null(), "{path}");
            assert!(
                rows[&(path.to_owned(), 0)].1.is_null(),
                "{path}: NULL stored"
            );
        }
        assert!(ids >= 12, "{fixture_name}: {ids} IDs");
        // RD-4: `index` and `show` wrote nothing under the root.
        assert!(
            snapshot(&root) == before,
            "{fixture_name}: the root is unchanged"
        );
        eprintln!(
            "{fixture_name}: {ids} IDs, {hashed} hashed of {} rows",
            rows.len()
        );
    }
}

/// TE-7: `spec bundle` is unchanged: its text and `bundle_hash` equal the
/// pre-change binary's byte for byte (the five hashes were taken from the
/// build before norm-hash and compared over 44 bundles of spec-a, -b, -c);
/// no ` | norm` anywhere in a bundle, and a target's header is `spec
/// show`'s without its norm and span fields. M: `norm` added to `header()`.
#[test]
fn te7_bundle_text_and_hash_are_unchanged() {
    let scratch = Scratch::new("norm-bundle");
    let home = scratch.home("h");
    let a = scratch.copy("spec-a", "a");
    let b = scratch.copy("spec-b", "b");
    for (root, id, want) in [
        (
            &a,
            "MEC-STAMINA",
            "b3:2f99a3acfdf948274ecca4d4d6376e8ae9d7939a85625fcb0539b87ee5304fc4",
        ),
        (
            &a,
            "MEC-SPRINT",
            "b3:2b3c1cadfc6f99a24c3856e4b9cf82ecf1b609958c67fc114fd2e530527f6ef7",
        ),
        (
            &a,
            "RULE-SPRINT-COST",
            "b3:154cc08a39407ebd277c373bc27e0c7330558af6ff86bf11e24f5e32b2e59440",
        ),
        (
            &b,
            "MOD-CLI",
            "b3:099eb625f1b806edd5c2254dfa64b4003b6cdaa94bc478434f774ae7c2ab2edb",
        ),
        (
            &b,
            "FLAG-DRY-RUN",
            "b3:ac7bb7ba41f9e76e0f5f2f0d3521ddad7d462510c9bc12138327b451fa17866b",
        ),
    ] {
        let run = spec(&home, root, &["bundle", id]);
        run.code(0);
        let text = split_text(&run.stdout);
        assert_eq!(text.hash, want, "{id}: bundle_hash");
        assert_eq!(b3(text.body.as_bytes()), want, "{id}: the body's BLAKE3");
        assert!(!run.stdout.contains(" | norm "), "{id}: {}", run.stdout);
        let json = spec(&home, root, &["bundle", id, "--json"]);
        json.code(0);
        assert_eq!(json.json()["bundle_hash"], want, "{id}: JSON");
        assert!(
            !json.stdout.contains("norm"),
            "{id}: no norm key in a bundle"
        );
        // The target's header line: show's, its norm and span fields cut.
        let (header, _) = show(&home, root, id);
        let (before_span, _) = header.rsplit_once(SPAN_SUFFIX).unwrap();
        let (bare, _) = before_span.rsplit_once(NORM_SUFFIX).unwrap();
        let target = text
            .body
            .lines()
            .skip_while(|line| *line != "## Targets")
            .nth(1)
            .unwrap_or_else(|| panic!("{id}: a Targets line: {}", text.body));
        assert_eq!(target, bare, "{id}: the bundle's target header");
    }
}

// ------------------------------------------------------------------ TE-4

/// TE-4, CLI half: a file that is not UTF-8 is shown whole with no
/// ` | norm` and a JSON `null` (and `utf8` false); a null document (a
/// broken front-matter written into a copy) likewise, its section hashed.
#[test]
fn te4_not_utf8_and_a_broken_front_matter_print_no_norm() {
    let scratch = Scratch::new("norm-null");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "a");
    write(
        &root,
        "docs/spec/broken.md",
        b"---\nid: MEC-BROKEN\n---\n\n# Broken\n\nbad \xff byte\n",
    );
    let section_before = core_norm(&root, SPRINT, "RULE-SPRINT-COST");
    common::replace(&root, SPRINT, "tier: 2\n", "tier: high\n");
    index(&home, &root);
    let (header, node) = show(&home, &root, "docs/spec/broken.md");
    assert!(!header.contains(" | norm "), "{header}");
    assert!(
        header.ends_with(&format!(
            "{SPAN_SUFFIX}{}",
            &b3(&read(&root, "docs/spec/broken.md"))[3..]
        )),
        "{header}"
    );
    assert!(node["norm_hash"].is_null(), "{node}");
    assert_eq!(node["utf8"], false);
    for reference in ["MEC-SPRINT", SPRINT] {
        let (header, node) = show(&home, &root, reference);
        assert!(!header.contains(" | norm "), "{reference}: {header}");
        assert!(node["norm_hash"].is_null(), "{reference}");
    }
    let (header, node) = show(&home, &root, "RULE-SPRINT-COST");
    assert_eq!(
        header_norm(&header),
        section_before,
        "the section keeps its hash"
    );
    assert_eq!(
        node["norm_hash"].as_str().map(str::to_owned),
        section_before
    );
}

// ------------------------------------------------------------------ TE-9

/// Copies `fixtures/<name>` to `to`, creating the files in reverse path
/// order (directories as they are first needed).
fn copy_reversed(name: &str, to: &Path) {
    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let from = fixture(name);
    let mut all = Vec::new();
    files(&from, &mut all);
    all.sort();
    all.reverse();
    for path in all {
        let relative = path.strip_prefix(&from).unwrap();
        let target = to.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, fs::read(&path).unwrap()).unwrap();
    }
}

/// Every ID's `norm_hash` as `spec show --json` gives it, run with `args`
/// before the command and in `cwd`.
fn shown_hashes(home: &Path, cwd: &Path, root: &Path, ids: &[String]) -> BTreeMap<String, Value> {
    let root_text = root.to_str().unwrap();
    ids.iter()
        .map(|id| {
            let run = spec(home, cwd, &["--root", root_text, "show", id, "--json"]);
            run.code(0);
            (id.clone(), run.json()["nodes"][0]["norm_hash"].clone())
        })
        .collect()
}

/// TE-9: the same corpus at two roots (one deeper, its files created in
/// reverse order), each with its own `HOME`, shown from inside its root and
/// from a cwd outside both: one hash per ID; the two indexes store the same
/// column. M: the root, a path or a time in the input.
#[test]
fn te9_two_roots_another_cwd_reverse_order_one_hash() {
    for (fixture_name, slug) in [("spec-a", "lantern-keep"), ("spec-b", "zerkalo")] {
        let scratch = Scratch::new("norm-determinism");
        let first = scratch.copy(fixture_name, "one");
        let second = scratch.join("deep/er/two");
        copy_reversed(fixture_name, &second);
        let second = fs::canonicalize(&second).unwrap();
        let elsewhere = scratch.dir("elsewhere");
        let (home_one, home_two) = (scratch.home("one"), scratch.home("two"));
        index(&home_one, &first);
        index(&home_two, &second);
        let rows_one = stored(&home_one, &first, slug);
        let rows_two = stored(&home_two, &second, slug);
        assert_eq!(
            rows_one, rows_two,
            "{fixture_name}: the stored ids and hashes"
        );
        let ids: Vec<String> = rows_one.values().filter_map(|(id, _)| id.clone()).collect();
        assert!(ids.len() >= 12, "{ids:?}");
        let in_root = shown_hashes(&home_one, &first, &first, &ids);
        assert_eq!(
            shown_hashes(&home_one, &elsewhere, &first, &ids),
            in_root,
            "{fixture_name}: another cwd"
        );
        assert_eq!(
            shown_hashes(&home_two, &second.join("docs"), &second, &ids),
            in_root,
            "{fixture_name}: another root, reverse creation order, a subdirectory cwd"
        );
        // And the store's own column agrees with show's.
        for (id, (_, norm)) in rows_one.iter().filter_map(|(_, (id, norm))| {
            id.as_ref()
                .map(|id| (id.clone(), (id.clone(), norm.clone())))
        }) {
            assert_eq!(in_root[&id], norm, "{fixture_name} {id}");
        }
    }
}
