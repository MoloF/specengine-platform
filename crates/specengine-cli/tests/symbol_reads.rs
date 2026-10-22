//! docs/features/symbol-reads.md (slice 6a-3): `spec symbols` and
//! `spec unmapped` over 6a-2's code index. TE-1 (every form, tier and row
//! kind over `fixtures/code-index` as it stands, against its
//! `expected.json` `reads`; the same commands over `fixtures/verify-ecs`,
//! an ECS-style game crate, against goldens here), TE-3 (`fixtures/spec-c`
//! has no `[code]`), the CLI half of RD-1 (the lookup's tiers and order),
//! RD-2 (one form, `--at`'s order, no `[code]`, every JSON key), the exits
//! and the cap.
//!
//! Over scratch copies; the fixtures are only read. Every text golden is
//! byte-exact; the JSON is derived from it (`symbol_reads_common`), the
//! shapes the spec leaves open pinned as the Phase A readings (Q35).
//!
//! docs/features/bevy-roles-reads.md RD-6 (10a-2): every target holds
//! `roles` after `bindings`, `null` without `[code] frameworks`, and with
//! them every role object holds every key in order
//! (`symbols_json_every_key_present`, over a scratch `fixtures/code-bevy`
//! too).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod symbol_reads_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use serde_json::{Value, json};

use common::{Scratch, spec, write};
use symbol_reads_common::{
    BINDING_KEYS, ECS, FIXTURE, FORMS, ITEM_KEYS, NO_TABLE, ROLE_KEYS, ROW_KEYS, SYMBOL_KEYS,
    SYMBOLS_KEYS, TARGETS, UNMAPPED_KEYS, args_of, as_strs, assert_key_order, both, copy,
    exit_2_quiet, ledger, lines_of, percents_as_f64, reads, stdout_of, stored_symbol, symbols_json,
    unmapped_json, without_code_table,
};

/// The golden of `reads.<list>` named `case`.
fn case(all: &Value, list: &str, case: &str) -> Value {
    all["reads"][list]
        .as_array()
        .expect("a golden list")
        .iter()
        .find(|entry| entry["case"] == case)
        .cloned()
        .unwrap_or_else(|| panic!("no reads.{list} case {case}"))
}

/// docs/features/layer-c-reads.md "JSON": a target read while no layer C
/// snapshot is stored holds its three layer C keys, each `null`.
fn assert_no_layer_c(target: &Value, context: &str) {
    for key in ["c_state", "moniker", "agrees"] {
        assert_eq!(
            target.get(key),
            Some(&Value::Null),
            "{context}: `{key}` null without a snapshot: {target}"
        );
    }
}

/// The key order of a `spec --json symbols` answer: the document, every
/// target, every binding, `omitted`, `code`.
fn assert_symbols_keys(stdout: &str, targets: usize, bindings: usize, context: &str) {
    assert_key_order(stdout, "$", &SYMBOLS_KEYS, 1, context);
    assert_key_order(stdout, "$.symbols[]", &SYMBOL_KEYS, targets, context);
    assert_key_order(
        stdout,
        "$.symbols[].bindings[]",
        &BINDING_KEYS,
        bindings,
        context,
    );
    assert_key_order(stdout, "$.omitted", &["symbols"], 1, context);
    assert_key_order(
        stdout,
        "$.code",
        &["files", "rs", "ron", "skipped"],
        1,
        context,
    );
}

/// The key order of a `spec --json unmapped` answer.
fn assert_unmapped_keys(stdout: &str, rows: usize, items: usize, context: &str) {
    assert_key_order(stdout, "$", &UNMAPPED_KEYS, 1, context);
    assert_key_order(stdout, "$.rows[]", &ROW_KEYS, rows, context);
    assert_key_order(stdout, "$.total", &ROW_KEYS, 1, context);
    assert_key_order(stdout, "$.unmapped[]", &ITEM_KEYS, items, context);
    assert_key_order(stdout, "$.omitted", &["rows", "unmapped"], 1, context);
    assert_key_order(
        stdout,
        "$.code",
        &["files", "rs", "ron", "skipped"],
        1,
        context,
    );
}

/// Every `symbols` golden of `entries` in `root`: text byte for byte,
/// stderr empty, exit 0; the JSON derived from the text (`stored` gives an
/// item's `name` and `discriminator`), its keys in order; each `also`
/// argument vector prints the same text. Failures are collected.
fn check_symbols(
    root: &Path,
    home: &Path,
    entries: &[Value],
    stored: &dyn Fn(&str, &str) -> Value,
) -> Vec<String> {
    let mut failures = Vec::new();
    for entry in entries {
        let name = entry["case"].as_str().expect("a case");
        let args = args_of(&entry["args"]);
        let (text, json_run) = both(home, root, &as_strs(&args));
        let want = stdout_of(entry);
        if (text.code, text.stdout.as_str(), text.stderr.as_str()) != (0, want.as_str(), "") {
            failures.push(format!(
                "{name}: spec {args:?}\n{}--- want\n{want}",
                text.show()
            ));
            continue;
        }
        let lines = lines_of(entry);
        let wanted = symbols_json(&lines, &entry["json"], stored);
        json_run.code(0);
        let got = json_run.json();
        if percents_as_f64(&got) != percents_as_f64(&wanted) {
            failures.push(format!(
                "{name}: spec --json {args:?}\n--- got\n{}\n--- want\n{}",
                serde_json::to_string_pretty(&got).expect("JSON"),
                serde_json::to_string_pretty(&wanted).expect("JSON")
            ));
        }
        let targets = wanted["symbols"].as_array().expect("symbols").len();
        let bindings = lines
            .iter()
            .filter(|line| line.starts_with("  ") && !line.starts_with("  sig "))
            .count();
        assert_symbols_keys(&json_run.stdout, targets, bindings, name);
        for target in got["symbols"].as_array().into_iter().flatten() {
            assert!(
                TARGETS.contains(&target["target"].as_str().unwrap_or("")),
                "{name}: target {}",
                target["target"]
            );
        }
        assert!(
            FORMS.contains(&got["form"].as_str().unwrap_or("")),
            "{name}"
        );
        for also in entry["also"].as_array().into_iter().flatten() {
            let other = args_of(also);
            let run = spec(home, root, &as_strs(&other));
            if run.code != 0 || run.stdout != want {
                failures.push(format!(
                    "{name}: spec {other:?} prints otherwise than {args:?}\n{}",
                    run.show()
                ));
            }
        }
    }
    failures
}

/// Every `unmapped` golden of `entries` in `root`, as [`check_symbols`].
fn check_unmapped(root: &Path, home: &Path, entries: &[Value]) -> Vec<String> {
    let mut failures = Vec::new();
    for entry in entries {
        let name = entry["case"].as_str().expect("a case");
        let args = args_of(&entry["args"]);
        let (text, json_run) = both(home, root, &as_strs(&args));
        let want = stdout_of(entry);
        if (text.code, text.stdout.as_str(), text.stderr.as_str()) != (0, want.as_str(), "") {
            failures.push(format!(
                "{name}: spec {args:?}\n{}--- want\n{want}",
                text.show()
            ));
            continue;
        }
        let lines = lines_of(entry);
        let wanted = unmapped_json(&lines, &entry["path"]);
        json_run.code(0);
        let got = json_run.json();
        if percents_as_f64(&got) != percents_as_f64(&wanted) {
            failures.push(format!(
                "{name}: spec --json {args:?}\n--- got\n{}\n--- want\n{}",
                serde_json::to_string_pretty(&got).expect("JSON"),
                serde_json::to_string_pretty(&wanted).expect("JSON")
            ));
        }
        let rows = wanted["rows"].as_array().expect("rows").len();
        let items = wanted["unmapped"].as_array().expect("unmapped").len();
        assert_unmapped_keys(&json_run.stdout, rows, items, name);
        for row in got["rows"].as_array().into_iter().flatten() {
            assert!(
                row["percent"].is_number() || row["percent"].is_null(),
                "{name}: percent is a number or null: {row}"
            );
        }
    }
    failures
}

// ------------------------------------------------------------------ TE-1

/// TE-1, `spec symbols`: each form (QUERY, `--node`, `--file`, `--at`),
/// each tier (`exact`, `suffix`, `name`; `impl Sheet #2` as one quoted
/// word and as three), `--limit`, zero rows, a NULL signature, a cut one,
/// an ambiguous item, a module and a RON target, over code-index as it
/// stands: `expected.json` `reads.symbols`.
#[test]
fn symbols_match_expected_json() {
    let scratch = Scratch::new("sr-symbols");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let all = reads();
    let stored = |path: &str, qpath: &str| stored_symbol(&all, path, qpath);
    let entries = all["reads"]["symbols"].as_array().expect("symbols").clone();
    assert!(entries.len() >= 18, "every form and tier is pinned");
    let failures = check_symbols(&root, &home, &entries, &stored);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// TE-1, `spec unmapped`: no PATH (a row per root), a package, a directory
/// (its child directories aggregated with `/`, its files), a file PATH (its
/// row, its unmapped items), a RON root, then `(total)`; and, with one
/// item-less file added to a scratch copy only (the fixture never
/// changes), `-` for no unit: `expected.json` `reads.unmapped`,
/// `reads.scratch`.
#[test]
fn unmapped_matches_expected_json() {
    let scratch = Scratch::new("sr-unmapped");
    let all = reads();
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let entries = all["reads"]["unmapped"]
        .as_array()
        .expect("unmapped")
        .clone();
    let mut failures = check_unmapped(&root, &home, &entries);

    let extended = copy(&scratch, FIXTURE, "extended");
    for file in all["reads"]["scratch"]["files"].as_array().expect("files") {
        write(
            &extended,
            file["path"].as_str().expect("path"),
            file["text"].as_str().expect("text"),
        );
    }
    let home = scratch.home("extended");
    let entries = all["reads"]["scratch"]["unmapped"]
        .as_array()
        .expect("scratch unmapped")
        .clone();
    failures.extend(check_unmapped(&extended, &home, &entries));
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The ECS goldens: verify-ecs's code (`fixtures/verify-ecs`), derived by
/// hand. Its lib's inner-doc marker maps every lib item; its stamina file
/// leaves the component and one constant unmapped.
const ECS_SYMBOLS: [(&[&str], &str, &[&str]); 3] = [
    (
        &["symbols", "regen_system"],
        "query",
        &[
            "crates/game::stamina::regen_system | function_item | crates/game/src/stamina.rs:10-13 | layer a | mentions 0 | suffix",
            "  sig pub fn regen_system(stamina: &mut Stamina, rate: f32, dt: f32)",
            "  implements | 9:4 | PERK-REGEN@1 | resolved | current | default | PERK-REGEN",
            "symbols: 1 of 1; code files 3 (.rs 2, .ron 1), skipped 0",
        ],
    ),
    (
        &["symbols", "--node", "PERK-STAMINA"],
        "node",
        &[
            "crates/game | module | crates/game/src/lib.rs",
            "  implements | 2:5 | PERK-STAMINA@2 | resolved | current | default | PERK-STAMINA",
            "crates/game::StaminaPlugin::build | function_item | crates/game/src/lib.rs:17-20 | layer a | mentions 0",
            "  sig pub fn build(&self, app: &mut App)",
            "  implements | 16:8 | PERK-STAMINA@2 | resolved | current | default | PERK-STAMINA",
            "symbols: 2 of 2; code files 3 (.rs 2, .ron 1), skipped 0",
        ],
    ),
    (
        &["symbols", "--at", "crates/game/src/lib.rs:18"],
        "at",
        &[
            "crates/game::StaminaPlugin::build | function_item | crates/game/src/lib.rs:17-20 | layer a | mentions 0",
            "  sig pub fn build(&self, app: &mut App)",
            "  implements | 16:8 | PERK-STAMINA@2 | resolved | current | default | PERK-STAMINA",
            "crates/game::impl StaminaPlugin | impl_item | crates/game/src/lib.rs:15-21 | layer a | mentions 0",
            "  sig impl StaminaPlugin",
            "symbols: 2 of 2; code files 3 (.rs 2, .ron 1), skipped 0",
        ],
    ),
];

/// verify-ecs's `unmapped` goldens: the root rows by path bytes (`assets/`
/// before `crates/`, the config's order reversed), a file PATH.
const ECS_UNMAPPED: [(&[&str], Option<&str>, &[&str]); 2] = [
    (
        &["unmapped"],
        None,
        &[
            "assets/ | units 1 | mapped 1 | 100.0 % | mentioned 0",
            "crates/ | units 10 | mapped 8 | 80.0 % | mentioned 0",
            "(total) | units 11 | mapped 9 | 81.8 % | mentioned 0",
            "unmapped: code files 3 (.rs 2, .ron 1), skipped 0",
        ],
    ),
    (
        &["unmapped", "crates/game/src/stamina.rs"],
        Some("crates/game/src/stamina.rs"),
        &[
            "crates/game/src/stamina.rs | units 5 | mapped 3 | 60.0 % | mentioned 0",
            "  crates/game::stamina::Stamina | struct_item | 4",
            "  crates/game::stamina::REST_DELAY | const_item | 24",
            "unmapped: code files 3 (.rs 2, .ron 1), skipped 0",
        ],
    ),
];

/// TE-1 on a project of another nature (ADR-0008): the same commands over
/// verify-ecs (systems, a plugin, RON tuning) print their goldens, text and
/// JSON, keys in order.
#[test]
fn ecs_symbols_and_unmapped_match_their_goldens() {
    let scratch = Scratch::new("sr-ecs");
    let root = copy(&scratch, ECS, "verify-ecs");
    let home = scratch.home("h");
    let entries: Vec<Value> = ECS_SYMBOLS
        .iter()
        .map(|(args, form, lines)| {
            let request = match *form {
                "query" => json!({"form": "query", "query": args[1], "node": null, "path": null, "line": null}),
                "node" => json!({"form": "node", "query": null, "node": args[2], "path": null, "line": null}),
                _ => {
                    let (path, line) = args[2].rsplit_once(':').expect("PATH:LINE");
                    json!({"form": "at", "query": null, "node": null, "path": path,
                           "line": line.parse::<u64>().expect("LINE")})
                }
            };
            json!({"case": args.join(" "), "args": args, "also": [], "json": request, "lines": lines})
        })
        .collect();
    // No twins in verify-ecs: an item's name is its qpath's last segment.
    let stored =
        |_: &str, qpath: &str| json!({"name": qpath.rsplit("::").next(), "discriminator": null});
    let mut failures = check_symbols(&root, &home, &entries, &stored);
    let entries: Vec<Value> = ECS_UNMAPPED
        .iter()
        .map(|(args, path, lines)| {
            json!({"case": args.join(" "), "args": args, "path": path, "lines": lines})
        })
        .collect();
    failures.extend(check_unmapped(&root, &home, &entries));
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

// ------------------------------------------------------------------ RD-1

/// RD-1 (Q27). M: a string split of the query. `query_name` reads a
/// segment `<T as a::Tr>` and a `cfg(…)` value whole: a name-tier query
/// whose last `::` sits inside `<>` or `()` finds its items; a suffix
/// with the trait owner is one segment.
#[test]
fn trait_owner_with_a_path_is_one_segment() {
    let scratch = Scratch::new("sr-segment");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let all = reads();
    let stored = |path: &str, qpath: &str| stored_symbol(&all, path, qpath);
    let mut entries: Vec<Value> = [
        "name_with_a_trait_owner",
        "suffix_with_a_trait_owner",
        "name_past_a_path_in_a_cfg",
    ]
    .iter()
    .map(|name| case(&all, "symbols", name))
    .collect();
    entries.push(json!({
        "case": "suffix_of_depth_two",
        "args": ["symbols", "<Page as std::fmt::Display>::fmt"],
        "also": [],
        "json": {"form": "query", "query": "<Page as std::fmt::Display>::fmt", "node": null, "path": null, "line": null},
        "lines": [
            "crates/inkwell::page::<Page as std::fmt::Display>::fmt | function_item | crates/inkwell/src/page.rs:14-16 | layer a | mentions 0 | suffix",
            "  sig fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result",
            "  implements | 13:8 | TENET-WIDTH@2 | resolved | current | default | TENET-WIDTH",
            "symbols: 1 of 1; code files 10 (.rs 9, .ron 1), skipped 0",
        ],
    }));
    let failures = check_symbols(&root, &home, &entries, &stored);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// RD-1 (Q27), the oracle of the `name` tier: for every stored symbol of
/// code-index (`expected.json` `symbols`, 6a-2's TE-1), `query_name` of its
/// qpath and of each of its suffixes (`expected.json` `suffixes`) is its
/// stored `name`, so a query of any of them meets the symbol at the `name`
/// tier at worst. M: a string split.
#[test]
fn query_name_of_every_stored_qpath_and_suffix_is_its_name() {
    let all = reads();
    let symbols = all["symbols"].as_array().expect("symbols");
    assert_eq!(symbols.len(), 31);
    let mut failures = Vec::new();
    for symbol in symbols {
        let qpath = symbol["qpath"].as_str().expect("qpath");
        let name = symbol["name"].as_str().expect("name");
        let mut queries = vec![qpath.to_owned()];
        if let Some(suffixes) = all["suffixes"][qpath].as_array() {
            queries.extend(
                suffixes
                    .iter()
                    .map(|suffix| suffix.as_str().expect("a suffix").to_owned()),
            );
        }
        for query in queries {
            let got = specengine_code::naming::query_name(&query);
            if got != name {
                failures.push(format!("{query:?}: {got:?}, want {name:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `text` appended to `root/relative`.
fn append(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    let mut bytes = fs::read(&path).expect("read");
    bytes.extend_from_slice(text.as_bytes());
    fs::write(&path, bytes).expect("write");
}

/// RD-1. M: rows in rowid order. After an index, two files are re-derived
/// (a comment line appended: no item, line or mention moves), so their
/// rows come back after the others'; the same tree indexed afresh in
/// another `HOME` stores them in walk order. Both answer every
/// multi-row golden byte for byte.
#[test]
fn lookup_order_is_independent_of_insertion_order() {
    let scratch = Scratch::new("sr-order");
    let root = copy(&scratch, FIXTURE, "code-index");
    let (old, fresh) = (scratch.home("old"), scratch.home("fresh"));
    spec(&old, &root, &["index"]).code(0);
    spec(&old, &root, &["symbols", "main"]).code(0);
    append(&root, "crates/inkwell/src/bin/stamp.rs", "// appended\n");
    append(&root, "crates/inkwell/src/twins.rs", "// appended\n");
    let all = reads();
    for name in [
        "exact",
        "suffix_then_name",
        "name_of_cfg_twins",
        "suffix_of_a_declaration_and_a_broken_item",
        "suffix_across_units",
        "name_of_twins_in_twin_mods",
        "node_items_and_a_ron_path",
        "node_modules",
    ] {
        let entry = case(&all, "symbols", name);
        let args = args_of(&entry["args"]);
        let want = stdout_of(&entry);
        for (label, home) in [("re-derived", &old), ("fresh", &fresh)] {
            let run = spec(home, &root, &as_strs(&args));
            run.code(0);
            assert_eq!(run.stdout, want, "{name}, {label} index");
        }
    }
    let entry = case(&all, "unmapped", "directory");
    let args = args_of(&entry["args"]);
    for home in [&old, &fresh] {
        assert_eq!(spec(home, &root, &as_strs(&args)).stdout, stdout_of(&entry));
    }
}

// ------------------------------------------------------------------ RD-2

/// RD-2. M: two forms accepted. Exactly one of QUERY, `--node`, `--file`,
/// `--at`; `--limit` 1..=200 and only with QUERY: anything else is exit 2,
/// stdout empty, text and JSON. `--limit 200` and `--limit 1` answer.
#[test]
fn symbols_takes_exactly_one_form() {
    let scratch = Scratch::new("sr-forms");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let file = "crates/inkwell/src/main.rs";
    let at = "crates/inkwell/src/main.rs:2";
    let refused: [&[&str]; 14] = [
        &["symbols"],
        &["symbols", "main", "--node", "SLAB-SHEET"],
        &["symbols", "main", "--file", file],
        &["symbols", "main", "--at", at],
        &["symbols", "--node", "SLAB-SHEET", "--file", file],
        &["symbols", "--file", file, "--at", at],
        &["symbols", "--node", "SLAB-SHEET", "--at", at],
        &["symbols", "--node", "SLAB-SHEET", "--limit", "5"],
        &["symbols", "--file", file, "--limit", "5"],
        &["symbols", "--at", at, "--limit", "5"],
        &["symbols", "main", "--limit", "0"],
        &["symbols", "main", "--limit", "201"],
        &["symbols", "main", "--limit", "x"],
        &["symbols", "--limit", "5"],
    ];
    for args in refused {
        let (text, json_run) = both(&home, &root, args);
        exit_2_quiet(&text);
        exit_2_quiet(&json_run);
        assert!(!text.stderr.is_empty(), "{args:?}: a reason");
    }
    let run = spec(&home, &root, &["symbols", "main", "--limit", "200"]);
    run.code(0);
    assert!(
        run.stdout
            .ends_with("symbols: 2 of 2; code files 10 (.rs 9, .ron 1), skipped 0\n")
    );
    let run = spec(&home, &root, &["symbols", "main", "--limit", "1"]);
    run.code(0);
    assert!(
        run.stdout
            .ends_with("symbols: 1 of 2; code files 10 (.rs 9, .ron 1), skipped 0\n"),
        "{}",
        run.show()
    );
}

/// RD-2. M: outermost first. `--at` lists the items covering LINE,
/// shortest span first: in nested inline mods, in a function body (a
/// nested `fn` is no item), on an ECS plugin's method; a line no item
/// covers answers zero rows.
#[test]
fn at_line_lists_innermost_first() {
    let scratch = Scratch::new("sr-at");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let all = reads();
    for name in ["at_nested_mods", "at_a_body", "at_no_item"] {
        let entry = case(&all, "symbols", name);
        let run = spec(&home, &root, &as_strs(&args_of(&entry["args"])));
        run.code(0);
        assert_eq!(run.stdout, stdout_of(&entry), "{name}");
    }
    let ecs = copy(&scratch, ECS, "verify-ecs");
    let (args, _, lines) = ECS_SYMBOLS[2];
    let run = spec(&home, &ecs, args);
    run.code(0);
    let want: String = lines.iter().map(|line| format!("{line}\n")).collect();
    assert_eq!(run.stdout, want);
}

/// RD-2. M: exit 0 without `[code]`. Every form of `symbols` and
/// `unmapped` (a PATH too) prints exactly the one line, stdout empty, text
/// and JSON; a `--node` naming nothing too (the config comes first).
#[test]
fn symbols_without_code_table_is_exit_2() {
    let scratch = Scratch::new("sr-no-table");
    let root = copy(&scratch, FIXTURE, "code-index");
    without_code_table(&root);
    let home = scratch.home("h");
    let cases: [&[&str]; 8] = [
        &["symbols", "main"],
        &["symbols", "--node", "SLAB-SHEET"],
        &["symbols", "--node", "QUILL-99"],
        &["symbols", "--file", "crates/inkwell/src/main.rs"],
        &["symbols", "--at", "crates/inkwell/src/main.rs:2"],
        &["unmapped"],
        &["unmapped", "crates/inkwell"],
        &["unmapped", "crates/inkwell/src/main.rs"],
    ];
    for args in cases {
        let (text, json_run) = both(&home, &root, args);
        for run in [&text, &json_run] {
            exit_2_quiet(run);
            assert_eq!(run.stderr, NO_TABLE, "{}", run.show());
        }
    }
}

/// A code file whose markers carry a note, declared levels, invalid
/// levels, a look-alike ID and a dangling one: the binding keys that are
/// `null` elsewhere hold values here.
const NOTED: &str = "// @implements TENET-FLOW@1 [body] keeps one mark\n\
pub fn noted() {}\n\
\n\
// @implements TENET-FLOW@1 [sig, nope]\n\
pub fn badly() {}\n\
\n\
// @implements \u{0422}ENET-FLOW\n\
pub fn looked() {}\n\
\n\
// @implements ZZZ-1\n\
pub fn dangled() {}\n";

/// RD-2. M: a key dropped (`levels`, `note` too). Every JSON object of
/// every form holds every key in the spec's order, absent ones `null`;
/// `tier` is `null` outside the `query` form (Q35). On a scratch file:
/// a note, declared and invalid levels, a look-alike's `fix`, a dangling
/// marker's `node` `null`.
///
/// docs/features/bevy-roles-reads.md RD-6 (`roles` added). M: a key
/// dropped (`roles` on a target; `form`, `group`, `detail`, `candidates`
/// on a role, `null` on most rows). Without `[code] frameworks`
/// (code-index) every target holds `roles: null`; over a scratch
/// `fixtures/code-bevy` (the opt-in on) every form's targets hold `roles`
/// as a list and every role object exactly [`ROLE_KEYS`] in order, a
/// `null` kept as a key (a derive row: `form`, `group`, `detail`,
/// `candidates`; the `cfg_attr` row: `detail` set; the ambiguous row:
/// `candidates` set).
///
/// docs/features/layer-c-reads.md TE-2 (the keys 18 and 12). M: a layer
/// C key dropped or not `null` while absent. No layer C snapshot is stored
/// in either corpus: every target holds `c_state`, `moniker`, `agrees`
/// `null`, every document `layer_c` `null` (the keys' order:
/// [`SYMBOL_KEYS`], [`SYMBOLS_KEYS`]).
#[test]
fn symbols_json_every_key_present() {
    let scratch = Scratch::new("sr-keys");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(&root, "crates/inkwell/src/noted.rs", NOTED);
    let home = scratch.home("h");
    let forms: [(&[&str], &str); 5] = [
        (&["symbols", "main"], "query"),
        (&["symbols", "--node", "TENET-WIDTH"], "node"),
        (
            &["symbols", "--file", "crates/inkwell/src/twins.rs"],
            "file",
        ),
        (&["symbols", "--at", "crates/inkwell/src/lib.rs:19"], "at"),
        (
            &["symbols", "--file", "crates/inkwell/src/noted.rs"],
            "file",
        ),
    ];
    for (args, form) in forms {
        let (_, json_run) = both(&home, &root, args);
        json_run.code(0);
        let value = json_run.json();
        let context = format!("{args:?}");
        let targets = value["symbols"].as_array().expect("symbols");
        let bindings: usize = targets
            .iter()
            .map(|target| target["bindings"].as_array().expect("bindings").len())
            .sum();
        assert!(!targets.is_empty() && bindings > 0, "{context}");
        assert_symbols_keys(&json_run.stdout, targets.len(), bindings, &context);
        assert_eq!(
            value.get("layer_c"),
            Some(&Value::Null),
            "{context}: `layer_c` null without a snapshot"
        );
        assert_eq!(value["form"], form, "{context}");
        for target in targets {
            assert!(
                TARGETS.contains(&target["target"].as_str().unwrap_or("")),
                "{context}: {target}"
            );
            assert_eq!(
                target.get("roles"),
                Some(&Value::Null),
                "{context}: `roles` null without `[code] frameworks`: {target}"
            );
            assert_no_layer_c(target, &context);
            if form == "query" {
                assert!(target["tier"].is_string(), "{context}: {target}");
            } else {
                assert!(target["tier"].is_null(), "{context}: tier {target}");
            }
        }
    }

    let (_, json_run) = both(
        &home,
        &root,
        &["symbols", "--file", "crates/inkwell/src/noted.rs"],
    );
    let value = json_run.json();
    let binding = |qpath: &str| -> Value {
        let target = value["symbols"]
            .as_array()
            .expect("symbols")
            .iter()
            .find(|target| target["qpath"] == qpath)
            .unwrap_or_else(|| panic!("no {qpath}: {value}"));
        target["bindings"][0].clone()
    };
    let noted = binding("crates/inkwell::noted::noted");
    assert_eq!(
        (
            &noted["levels"],
            &noted["levels_state"],
            &noted["note"],
            &noted["rev"],
            &noted["node"]
        ),
        (
            &json!(["body"]),
            &json!("declared"),
            &json!("keeps one mark"),
            &json!(1),
            &json!("TENET-FLOW")
        )
    );
    let badly = binding("crates/inkwell::noted::badly");
    assert_eq!(
        (&badly["levels"], &badly["levels_state"], &badly["note"]),
        (&Value::Null, &json!("unknown"), &Value::Null)
    );
    let looked = binding("crates/inkwell::noted::looked");
    assert_eq!(
        (
            &looked["token"],
            &looked["id_state"],
            &looked["fix"],
            &looked["rev"],
            &looked["rev_state"]
        ),
        (
            &json!("\u{0422}ENET-FLOW"),
            &json!("homoglyph"),
            &json!("TENET-FLOW"),
            &Value::Null,
            &json!("unversioned")
        )
    );
    let dangled = binding("crates/inkwell::noted::dangled");
    assert_eq!(
        (&dangled["id_state"], &dangled["fix"], &dangled["node"]),
        (&json!("dangling"), &Value::Null, &Value::Null)
    );

    // docs/features/bevy-roles-reads.md RD-6: code-bevy as it stands, its
    // `[code] frameworks = ["bevy"]` on.
    let bevy = copy(&scratch, "code-bevy", "code-bevy");
    let home = scratch.home("bevy");
    let bevy_forms: [(&[&str], &str); 5] = [
        (&["symbols", "run"], "query"),
        (&["symbols", "--node", "ALM-GROWTH"], "node"),
        (&["symbols", "--file", "crates/farm/src/herd.rs"], "file"),
        (&["symbols", "--file", "crates/farm/src/clock.rs"], "file"),
        (&["symbols", "--at", "crates/farm/src/lib.rs:33"], "at"),
    ];
    let nullable = ["form", "group", "detail", "candidates"];
    let mut nulls = [0_usize; 4];
    let mut values = [0_usize; 4];
    for (args, form) in bevy_forms {
        let (_, json_run) = both(&home, &bevy, args);
        json_run.code(0);
        let value = json_run.json();
        let context = format!("code-bevy {args:?}");
        assert_eq!(value["form"], form, "{context}");
        assert_eq!(
            value.get("layer_c"),
            Some(&Value::Null),
            "{context}: `layer_c` null without a snapshot"
        );
        let targets = value["symbols"].as_array().expect("symbols");
        let bindings: usize = targets
            .iter()
            .map(|target| target["bindings"].as_array().expect("bindings").len())
            .sum();
        let mut roles = 0;
        for target in targets {
            assert_no_layer_c(target, &context);
            let list = target["roles"]
                .as_array()
                .unwrap_or_else(|| panic!("{context}: `roles` a list with the opt-in: {target}"));
            roles += list.len();
            for role in list {
                for (at, key) in nullable.iter().enumerate() {
                    if role[*key].is_null() {
                        nulls[at] += 1;
                    } else {
                        values[at] += 1;
                    }
                }
            }
        }
        assert!(!targets.is_empty() && roles > 0, "{context}: {value}");
        assert_symbols_keys(&json_run.stdout, targets.len(), bindings, &context);
        assert_key_order(
            &json_run.stdout,
            "$.symbols[].roles[]",
            &ROLE_KEYS,
            roles,
            &context,
        );
    }
    for (at, key) in nullable.iter().enumerate() {
        assert!(
            nulls[at] > 0 && values[at] > 0,
            "`{key}`: a value and a kept `null` both exercised ({} null, {} set)",
            nulls[at],
            values[at]
        );
    }
}

/// The closed `ambiguity` words (docs/canon/code-bindings.md "Rust binding").
const AMBIGUITIES: [&str; 3] = ["path_attribute", "unrooted", "duplicate"];

/// `value` is a JSON integer.
fn integer(value: &Value) -> bool {
    value.is_u64()
}

/// RD-2 (Q35). M: a type changed (`percent` a string, `tier` set outside
/// QUERY). Every `symbols` document: `reason` `null`; `total` an integer;
/// `omitted` `{symbols: <integer>}`; `line` an integer for `--at`, else
/// `null`; each target's `target` one of `item`, `module`, `ron`, `tier`
/// one of `exact`, `suffix`, `name` in the QUERY form and `null` in the
/// others, `ambiguity` `null` or a closed word, an item's lines and
/// `mentions` integers; each binding's `line`, `column` integers, `rev` an
/// integer or `null`, `levels` a list or `null`. Every `unmapped`
/// document: `total` a row object; each row's `dir` a boolean, its counts
/// integers, `percent` a number, `null` exactly where the text shows `-`;
/// `omitted` `{rows, unmapped}` integers.
#[test]
fn symbol_reads_json_types_are_pinned() {
    let scratch = Scratch::new("sr-types");
    let root = copy(&scratch, FIXTURE, "code-index");
    let all = reads();
    for file in all["reads"]["scratch"]["files"].as_array().expect("files") {
        write(
            &root,
            file["path"].as_str().expect("path"),
            file["text"].as_str().expect("text"),
        );
    }
    let home = scratch.home("h");
    let symbol_reads: [(&[&str], &str); 6] = [
        (&["symbols", "impl Sheet #2"], "query"),
        (&["symbols", "broken"], "query"),
        (&["symbols", "--node", "TENET-WIDTH"], "node"),
        (&["symbols", "--node", "SLAB-SHEET"], "node"),
        (
            &["symbols", "--file", "crates/inkwell/src/twins.rs"],
            "file",
        ),
        (&["symbols", "--at", "crates/inkwell/src/lib.rs:19"], "at"),
    ];
    for (args, form) in symbol_reads {
        let (_, json_run) = both(&home, &root, args);
        json_run.code(0);
        let value = json_run.json();
        let context = format!("{args:?}: {value}");
        assert!(value["reason"].is_null(), "{context}");
        assert!(integer(&value["total"]), "{context}");
        assert!(
            integer(&value["omitted"]["symbols"])
                && value["omitted"].as_object().expect("omitted").len() == 1,
            "{context}"
        );
        assert_eq!(value["form"], form, "{context}");
        assert_eq!(integer(&value["line"]), form == "at", "{context}");
        if form != "at" {
            assert!(value["line"].is_null(), "{context}");
        }
        for target in value["symbols"].as_array().expect("symbols") {
            let kind = target["target"].as_str().unwrap_or("");
            assert!(TARGETS.contains(&kind), "{context}");
            match target["tier"].as_str() {
                Some(tier) => assert!(
                    form == "query" && ["exact", "suffix", "name"].contains(&tier),
                    "{context}"
                ),
                None => assert!(target["tier"].is_null() && form != "query", "{context}"),
            }
            assert!(
                target["ambiguity"].is_null()
                    || AMBIGUITIES.contains(&target["ambiguity"].as_str().unwrap_or("")),
                "{context}"
            );
            if kind == "item" {
                for key in ["start_line", "end_line", "mentions"] {
                    assert!(integer(&target[key]), "{key}: {context}");
                }
            }
            for binding in target["bindings"].as_array().expect("bindings") {
                assert!(
                    integer(&binding["line"]) && integer(&binding["column"]),
                    "{context}"
                );
                assert!(
                    binding["rev"].is_null() || integer(&binding["rev"]),
                    "{context}"
                );
                assert!(
                    binding["levels"].is_null() || binding["levels"].is_array(),
                    "{context}"
                );
            }
        }
    }
    let unmapped_reads: [&[&str]; 4] = [
        &["unmapped"],
        &["unmapped", "crates/inkwell/src"],
        &["unmapped", "crates/inkwell/src/blank"],
        &["unmapped", "crates/inkwell/src/twins.rs"],
    ];
    let mut dashes = 0;
    for args in unmapped_reads {
        let (text, json_run) = both(&home, &root, args);
        text.code(0);
        json_run.code(0);
        let value = json_run.json();
        let context = format!("{args:?}: {value}");
        assert!(value["total"].is_object(), "{context}");
        assert!(
            integer(&value["omitted"]["rows"])
                && integer(&value["omitted"]["unmapped"])
                && value["omitted"].as_object().expect("omitted").len() == 2,
            "{context}"
        );
        let rows: Vec<&Value> = value["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .chain([&value["total"]])
            .collect();
        let dash_lines = text
            .stdout
            .lines()
            .filter(|line| line.contains(" | - | "))
            .count();
        let mut null_percents = 0;
        for row in rows {
            assert!(row["dir"].is_boolean(), "{context}");
            for key in ["units", "mapped", "mentioned"] {
                assert!(integer(&row[key]), "{key}: {context}");
            }
            if row["units"] == 0 {
                assert!(row["percent"].is_null(), "no unit, no percent: {context}");
                null_percents += 1;
            } else {
                assert!(row["percent"].is_number(), "a number: {context}");
            }
        }
        let file_path = args.len() == 2 && args[1].ends_with(".rs");
        if !file_path {
            assert_eq!(null_percents, dash_lines, "`-` rows: {context}");
        }
        dashes += dash_lines;
    }
    assert!(dashes >= 3, "the `-` rows were exercised");
}

/// `spec symbols --node BOOK-SHELF` over the scratch ledger: its three
/// targets by path, each with every direct marker by line.
const LEDGER_NODE: [&str; 11] = [
    "crates/ledger::book | module | crates/ledger/src/book.rs",
    "  implements | 2:5 | BOOK-SHELF@1 | resolved | current | default | BOOK-SHELF",
    "crates/ledger::odd | module | crates/ledger/src/odd.rs",
    "  implements | 1:5 | ZZZ-1@1 | dangling | rev_unknown | default | -",
    "  implements | 2:5 | PR-12 | queue | unversioned | default | -",
    "  implements | 3:5 | other:BOOK-SHELF@1 | other_project | rev_unknown | default | -",
    "  implements | 4:5 | \u{0412}OOK-SHELF@1 | homoglyph BOOK-SHELF | rev_unknown | default | BOOK-SHELF",
    "crates/ledger::shelf::impl Shelf | impl_item | crates/ledger/src/shelf.rs:4-8 | layer a | mentions 0",
    "  sig impl Shelf",
    "  implements | 3:4 | BOOK-SHELF@1 | resolved | current | default | BOOK-SHELF",
    "symbols: 3 of 3; code files 15 (.rs 11, .ron 4), skipped 0",
];

/// Review 1's owner-approved readings X1, X2 (the developer's R2, R5):
/// `--node` follows `refs`. A target whose only marker naming the node is a
/// look-alike is listed (the look-alike's row shows `homoglyph <fix>` and
/// the fix's node), every direct marker of a target is listed whatever it
/// names (dangling, queue, other project), and `unmapped` still counts the
/// look-alike as binding nothing. M: `--node` keeps only resolved markers;
/// only the markers naming the node.
#[test]
fn node_lists_a_look_alike_target_and_every_direct_marker() {
    let scratch = Scratch::new("sr-node-markers");
    let root = ledger(&scratch, "ledger");
    let home = scratch.home("h");
    let run = spec(&home, &root, &["symbols", "--node", "BOOK-SHELF"]);
    run.code(0);
    let want: String = LEDGER_NODE.iter().map(|line| format!("{line}\n")).collect();
    assert_eq!(run.stdout, want, "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    let (_, json_run) = both(&home, &root, &["symbols", "--node", "BOOK-SHELF"]);
    let value = json_run.json();
    let odd = &value["symbols"][1];
    assert_eq!(odd["target"], "module");
    let states: Vec<&str> = odd["bindings"]
        .as_array()
        .expect("bindings")
        .iter()
        .map(|binding| binding["id_state"].as_str().expect("an id_state"))
        .collect();
    assert_eq!(states, ["dangling", "queue", "other_project", "homoglyph"]);
    assert_eq!(odd["bindings"][3]["fix"], "BOOK-SHELF");
    assert_eq!(odd["bindings"][3]["node"], "BOOK-SHELF");
    assert_eq!(odd["bindings"][0]["node"], Value::Null);
    let unmapped = spec(&home, &root, &["unmapped", "crates/ledger/src/odd.rs"]);
    unmapped.code(0);
    assert!(
        unmapped
            .stdout
            .starts_with("crates/ledger/src/odd.rs | units 6 | mapped 0 | 0.0 % | mentioned 0\n"),
        "{}",
        unmapped.show()
    );
}

// ------------------------------------------------------------------ TE-3

/// TE-3: `fixtures/spec-c`, a project of another genre with no `[code]`:
/// `symbols` (every form) and `unmapped` exit 2 naming `[code]`, stdout
/// empty, text and JSON.
#[test]
fn symbols_and_unmapped_on_spec_c_exit_2_naming_the_code_table() {
    let scratch = Scratch::new("sr-spec-c");
    let root = scratch.copy("spec-c", "spec-c");
    let home = scratch.home("h");
    let cases: [&[&str]; 6] = [
        &["symbols", "anything"],
        &["symbols", "--node", "WISH-001"],
        &["symbols", "--file", "a.rs"],
        &["symbols", "--at", "a.rs:1"],
        &["unmapped"],
        &["unmapped", "docs"],
    ];
    for args in cases {
        let (text, json_run) = both(&home, &root, args);
        for run in [&text, &json_run] {
            exit_2_quiet(run);
            assert_eq!(run.stderr, NO_TABLE, "{}", run.show());
        }
    }
}

// ----------------------------------------------------------------- exits

/// RD-2 (Q35). M: a REF naming nothing as a `CliError`. A `--node` REF
/// naming nothing (an ID, a `.md` path with no node) is an outcome: exit
/// 1, stdout empty and one `spec: ` line in text; in JSON the document
/// kept, as `refs`: `reason` a string, `symbols` `[]`, `code` and
/// `omitted` `null`, keys in order (a `CliError` prints no document).
#[test]
fn symbols_node_naming_nothing_keeps_its_document() {
    let scratch = Scratch::new("sr-nothing");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    for reference in ["QUILL-99", "docs/nope.md"] {
        let (text, json_run) = both(&home, &root, &["symbols", "--node", reference]);
        text.code(1);
        assert_eq!(text.stdout, "", "{}", text.show());
        assert!(
            text.stderr.starts_with("spec: ") && text.stderr_lines().len() == 1,
            "{}",
            text.show()
        );
        json_run.code(1);
        let value = json_run.json();
        assert!(value["reason"].is_string(), "{value}");
        assert_eq!(value["symbols"], json!([]));
        assert_eq!(value["total"], 0, "the developer's R1: an integer");
        assert_eq!(value["code"], Value::Null);
        assert_eq!(value["omitted"], Value::Null);
        assert_eq!(
            value.get("layer_c"),
            Some(&Value::Null),
            "docs/features/layer-c-reads.md \"JSON\": exit 1, `layer_c` null as `code`"
        );
        assert_eq!(value["node"], reference);
        assert_eq!(value["form"], "node");
        assert_key_order(&json_run.stdout, "$", &SYMBOLS_KEYS, 1, reference);
    }
}

/// "spec symbols", "spec unmapped" exit 2: a PATH that is not a walked
/// code file (another file, a directory, a missing one, one outside the
/// root), a bad LINE, a PATH outside every root — stdout empty, a reason,
/// text and JSON. A walked RON file and a root answer.
#[test]
fn paths_and_lines_outside_the_walk_are_exit_2() {
    let scratch = Scratch::new("sr-paths");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let refused: [&[&str]; 17] = [
        &["symbols", "--file", "docs/spec/ink.md"],
        &["symbols", "--file", "crates/inkwell/Cargo.toml"],
        &["symbols", "--file", "crates/inkwell/src/nope.rs"],
        &["symbols", "--file", "crates/inkwell/src"],
        &["symbols", "--file", "../outside.rs"],
        &["symbols", "--at", "crates/inkwell/src/lib.rs:0"],
        &["symbols", "--at", "crates/inkwell/src/lib.rs:x"],
        &["symbols", "--at", "crates/inkwell/src/lib.rs"],
        &["symbols", "--at", "docs/spec/ink.md:1"],
        &["symbols", "--at", "crates/inkwell/src/nope.rs:1"],
        &["unmapped", "docs"],
        &["unmapped", "docs/spec/ink.md"],
        &["unmapped", "crates/nope"],
        &["unmapped", "crates/inkwell/Cargo.toml"],
        &["unmapped", "specengine.toml"],
        &["unmapped", "../outside"],
        &["unmapped", "crates/inkwell/src/nope.rs"],
    ];
    for args in refused {
        let (text, json_run) = both(&home, &root, args);
        exit_2_quiet(&text);
        exit_2_quiet(&json_run);
        assert!(text.stderr.starts_with("spec: "), "{}", text.show());
    }
    spec(&home, &root, &["symbols", "--file", "data/inks.ron"]).code(0);
    spec(&home, &root, &["unmapped", "crates"]).code(0);
}

/// "spec symbols", "spec unmapped": a walk gap (an unlistable directory)
/// is exit 2, stdout empty, `spec: the code walk is incomplete: <gap>`,
/// text and JSON (6a-2's reads rule).
#[test]
fn a_walk_gap_is_exit_2() {
    let scratch = Scratch::new("sr-gap");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    spec(&home, &root, &["index"]).code(0);
    let tests_dir = root.join("crates/inkwell/tests");
    fs::set_permissions(&tests_dir, fs::Permissions::from_mode(0o000)).expect("chmod");
    let want =
        "spec: the code walk is incomplete: the directory `crates/inkwell/tests` cannot be listed";
    for args in [
        &["symbols", "main"][..],
        &["symbols", "--file", "crates/inkwell/src/main.rs"][..],
        &["unmapped"][..],
        &["unmapped", "crates/inkwell/src"][..],
    ] {
        let (text, json_run) = both(&home, &root, args);
        for run in [&text, &json_run] {
            exit_2_quiet(run);
            assert!(
                run.stderr.starts_with(want) && run.stderr_lines().len() == 1,
                "want {want:?}\n{}",
                run.show()
            );
        }
    }
    fs::set_permissions(&tests_dir, fs::Permissions::from_mode(0o755)).expect("chmod back");
}

/// 800 one-line functions with long names in one scratch file.
fn many() -> String {
    (0..800)
        .map(|index| format!("pub fn function_with_a_long_name_number_{index:04}() {{}}\n"))
        .collect()
}

/// `[truncated: <k> …]`'s `k`.
fn truncated(line: &str) -> usize {
    line.strip_prefix("[truncated: ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|k| k.parse().ok())
        .unwrap_or_else(|| panic!("a tail: {line:?}"))
}

/// "spec symbols", "spec unmapped": the cap. Rows while the text stays
/// within `OUTPUT_CAP_CHARS` (40 000), then `[truncated: <k> symbols not
/// shown]` (`unmapped` of a file PATH: `[truncated: <k> unmapped items not
/// shown]`, the developer's R11), the summary last; the JSON holds the
/// printed ones and `omitted` the rest.
#[test]
fn symbols_and_unmapped_are_capped_with_a_tail() {
    let scratch = Scratch::new("sr-cap");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(&root, "crates/inkwell/src/many.rs", many());
    let home = scratch.home("h");
    let code = "code files 11 (.rs 10, .ron 1), skipped 0";

    let (text, json_run) = both(
        &home,
        &root,
        &["symbols", "--file", "crates/inkwell/src/many.rs"],
    );
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let tail = lines[lines.len() - 2];
    assert!(
        tail.starts_with("[truncated: ") && tail.ends_with(" symbols not shown]"),
        "{tail}"
    );
    let summary = lines[lines.len() - 1];
    assert!(
        summary.starts_with("symbols: ") && summary.ends_with(&format!(" of 800; {code}")),
        "{summary}"
    );
    let before: usize = lines[..lines.len() - 2]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(before <= 40_000, "{before} characters before the tail");
    let shown = lines[..lines.len() - 2]
        .iter()
        .filter(|line| !line.starts_with("  "))
        .count();
    assert_eq!(shown + truncated(tail), 800);
    json_run.code(0);
    let value = json_run.json();
    let listed = value["symbols"].as_array().expect("symbols").len();
    assert_eq!(listed, shown, "text and JSON show the same targets");
    assert_eq!(value["omitted"], json!({"symbols": 800 - shown}));
    assert_eq!(value["total"], 800);

    let (text, json_run) = both(&home, &root, &["unmapped", "crates/inkwell/src/many.rs"]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let tail = lines[lines.len() - 2];
    assert!(
        tail.starts_with("[truncated: ") && tail.ends_with(" unmapped items not shown]"),
        "{tail}"
    );
    assert_eq!(lines[lines.len() - 1], format!("unmapped: {code}"));
    assert_eq!(
        lines[0],
        "crates/inkwell/src/many.rs | units 800 | mapped 0 | 0.0 % | mentioned 0"
    );
    let before: usize = lines[..lines.len() - 2]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(before <= 40_000, "{before} characters before the tail");
    let items = lines[..lines.len() - 2]
        .iter()
        .filter(|line| line.starts_with("  "))
        .count();
    assert_eq!(items + truncated(tail), 800);
    json_run.code(0);
    let value = json_run.json();
    assert_eq!(value["unmapped"].as_array().expect("unmapped").len(), items);
    assert_eq!(
        value["omitted"],
        json!({"rows": 0, "unmapped": 800 - items})
    );
    assert_eq!(value["total"]["units"], 800);
}
