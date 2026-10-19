//! docs/features/bevy-roles-reads.md (slice 10a-2): framework roles in
//! `spec symbols`, one line per role under each item target (after `sig`,
//! before its bindings) and `roles` in its JSON (`null` without
//! `[code] frameworks`).
//!
//! - RD-6: `roles_are_null_without_frameworks`,
//!   `role_rows_sort_by_content`, `non_item_targets_hold_empty_roles`,
//!   `role_lines_count_against_the_cap`, `role_fields_are_one_line`,
//!   `unlinked_rows_show_on_no_target`; every key present is 6a-3's
//!   `symbols_json_every_key_present` (`symbol_reads.rs`, `roles` added).
//! - RD-7, the `symbols` half (the index half: `bevy_roles_outputs.rs`):
//!   `symbols_equal_with_and_without_frameworks_but_roles`.
//! - TE-5: `symbols_roles_match_expected_json_in_every_form` (code-bevy
//!   per form against its `expected.json`),
//!   `code_index_and_code_a_hold_null_roles_and_spec_c_is_unchanged`,
//!   `code_a_with_frameworks_holds_empty_roles_everywhere`.
//! - The developer's iteration 1 suggestions (phase B):
//!   `role_reads_search_their_indexes` (the store's two new statements,
//!   `FILE_ROLES_SQL` and `NAMED_ROLES_SQL`, planned as index searches),
//!   `a_spaced_qualified_subject_is_stored_compact_and_links_by_suffix`
//!   (B5, `CODE_INDEX_FORMAT` 4), `path_form_and_code_origin_are_the_stored_words`
//!   (D1).
//!
//! Real data: the `spec` binary over scratch copies of
//! `fixtures/code-bevy` (10a-1's farm game, the opt-in on; a module and a
//! RON marker of `CHORE-02` written in: `bevy_roles_reads_common`),
//! `fixtures/code-index`, `fixtures/code-a` (a counter library, no Bevy)
//! and `fixtures/spec-c` (no `[code]`, not Rust), and probe projects
//! written here (`bevy_roles_common::probe`: one package `crates/probe`,
//! the opt-in on), each index in a scratch `HOME`. What a target shows is
//! derived from `expected.json`'s rows and the link states it records
//! (docs/canon/code-roles.md "Links"); the readings R1-R4 the spec leaves
//! open are `bevy_roles_reads_common`'s, pinned literally only in
//! `role_rows_sort_by_content` and `unlinked_rows_show_on_no_target`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod bevy_roles_reads_common;
mod code_index_common;
mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use specengine_cli::OUTPUT_CAP_CHARS;
use specengine_code::frameworks::{CODE_ORIGIN, PATH_FORM};

use bevy_roles_common::{CODE_EMPTY, CODE_OFF, PROBE_SLUG, probe, role_rows, set_code};
use bevy_roles_reads_common::{
    FORMS, bevy_copy, both, expected_roles, item_qpath, json_without_roles, role_lines_of,
    roles_of, shows_on, text_without_roles, with_role_lines, with_roles,
};
use code_index_common::{db, query};
use common::{Run, Scratch, read_text, replace, repository_root, spec};

/// `spec refs`' line for a config without `[code]`
/// (docs/canon/code-bindings.md "The `[code]` table"); `symbols` prints it
/// alike.
const NO_TABLE: &str =
    "spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)\n";

/// `fixtures/code-index` forms: items, a RON path (`TENET-WIDTH`), modules
/// (`SLAB-SHEET`).
const CODE_INDEX_FORMS: [&[&str]; 5] = [
    &["symbols", "main"],
    &["symbols", "--node", "TENET-WIDTH"],
    &["symbols", "--node", "SLAB-SHEET"],
    &["symbols", "--file", "crates/inkwell/src/twins.rs"],
    &["symbols", "--at", "crates/inkwell/src/lib.rs:19"],
];

/// `fixtures/code-a` forms: items, modules and RON paths
/// (`PIECE-COUNTER`, `LAW-BOUND`), the file with a parse error.
const CODE_A_FORMS: [&[&str]; 6] = [
    &["symbols", "Tally"],
    &["symbols", "--node", "PIECE-COUNTER"],
    &["symbols", "--node", "LAW-BOUND"],
    &["symbols", "--file", "crates/tally/src/counter.rs"],
    &["symbols", "--file", "crates/tally/src/broken.rs"],
    &["symbols", "--at", "crates/tally/src/counter.rs:22"],
];

/// code-a's `[code]` line the opt-in follows.
const CODE_A_EXCLUDE: &str = "exclude = [\"crates/*/tests/fixtures/**\"]\n";

/// `run`'s document, or `None` for an exit-2 answer (stdout empty).
fn document(run: &Run) -> Option<Value> {
    if run.code == 2 {
        assert_eq!(run.stdout, "", "exit 2 prints nothing:\n{}", run.show());
        return None;
    }
    Some(run.json())
}

/// Every target of a document.
fn targets(document: &Value) -> Vec<Value> {
    document["symbols"].as_array().cloned().unwrap_or_default()
}

fn kind_set(kinds: &[&str]) -> BTreeSet<String> {
    kinds.iter().map(|kind| (*kind).to_owned()).collect()
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("JSON")
}

/// The form of a `spec symbols` argument list.
fn form_of(args: &[&str]) -> &'static str {
    match args.get(1).copied() {
        Some("--node") => "node",
        Some("--file") => "file",
        Some("--at") => "at",
        _ => "query",
    }
}

// ------------------------------------------------------------------ RD-6

/// RD-6. M: `[]` without the opt-in. code-bevy with a `[code]` without
/// `frameworks` and with `frameworks = []`: no form prints a role line,
/// and every target of every document (items, the module and the RON path
/// of `CHORE-02`) holds `roles: null`, the key kept.
#[test]
fn roles_are_null_without_frameworks() {
    let scratch = Scratch::new("brr-null");
    for (label, table) in [("off", CODE_OFF), ("empty", CODE_EMPTY)] {
        let root = bevy_copy(&scratch, label);
        set_code(&root, table);
        let home = scratch.home(label);
        let mut kinds = BTreeSet::new();
        for args in FORMS {
            let (text, json_run) = both(&home, &root, args);
            assert_eq!(text.code, json_run.code, "{label} {args:?}");
            assert!(
                !text.stdout.lines().any(|line| line.starts_with("  role ")),
                "{label} {args:?}: a role line without the opt-in\n{}",
                text.show()
            );
            let Some(document) = document(&json_run) else {
                continue;
            };
            for target in targets(&document) {
                kinds.insert(target["target"].as_str().unwrap_or_default().to_owned());
                assert_eq!(
                    target.get("roles"),
                    Some(&Value::Null),
                    "{label} {args:?}: {target}"
                );
            }
        }
        assert_eq!(
            kinds,
            kind_set(&["item", "module", "ron"]),
            "{label}: every kind of target read"
        );
    }
}

/// One role line's (word, path, line), as its JSON object names them.
fn word_path_line(line: &str) -> (Value, Value, Value) {
    let fields: Vec<&str> = line.trim_start().split(" | ").collect();
    let word = fields[0].strip_prefix("role ").expect("`role <word>`");
    let (path, at) = fields[2].rsplit_once(':').expect("<path>:<line>");
    (
        json!(word),
        json!(path),
        json!(at.parse::<u64>().expect("a line")),
    )
}

/// RD-6, the order. M: the rows in file order (`(path, ord)`). On a
/// target the rows sort by layer, word, path bytes, line, source, `ord`,
/// whatever order the files hold them in: `Weather`'s one derive
/// (`Resource`, then `Reflect`) prints `reflect` before `resource`;
/// `clock::run`'s ambiguous row of `fields.rs` before its linked row of
/// `lib.rs`; `Harvest`'s `add_message` at `lib.rs:32` before its derive at
/// `news.rs:6` (path before line); `Quake`'s derive at line 12 before its
/// `macro_rules` call at 28. A probe's `#[derive(Resource, Reflect,
/// Component)]` prints `component`, `reflect`, `resource`. Each JSON
/// `roles` lists the same rows in the same order; three are pinned whole
/// (readings R1-R3: `form` `path` and `origin` `code` kept, `candidates`
/// the ambiguous count, `detail` the category).
#[test]
fn role_rows_sort_by_content() {
    let scratch = Scratch::new("brr-order");
    let root = bevy_copy(&scratch, "bevy");
    let home = scratch.home("h");
    let cases: [(&[&str], &str, &[&str]); 4] = [
        (
            &["symbols", "--file", "crates/farm/src/weather.rs"],
            "crates/farm::weather::Weather",
            &[
                "  role reflect | a | crates/farm/src/weather.rs:6 | derive | - | certain",
                "  role resource | a | crates/farm/src/weather.rs:6 | derive | - | certain",
            ],
        ),
        (
            &["symbols", "crates/farm::clock::run"],
            "crates/farm::clock::run",
            &[
                "  role system | a | crates/farm/src/fields.rs:26 | call | Update | ambiguous 3",
                "  role system | a | crates/farm/src/lib.rs:30 | call | Update | linked",
            ],
        ),
        (
            &["symbols", "Harvest"],
            "crates/farm::news::Harvest",
            &[
                "  role message | a | crates/farm/src/lib.rs:32 | call | - | linked",
                "  role message | a | crates/farm/src/news.rs:6 | derive | - | certain",
            ],
        ),
        (
            &["symbols", "--file", "crates/farm/src/news.rs"],
            "crates/farm::news::Quake",
            &[
                "  role message | a | crates/farm/src/news.rs:12 | derive | - | certain",
                "  role message | a | crates/farm/src/news.rs:28 | call macro_rules | - | linked",
            ],
        ),
    ];
    for (args, qpath, want) in cases {
        let (text, json_run) = both(&home, &root, args);
        text.code(0);
        assert_eq!(
            role_lines_of(&text.stdout, qpath),
            want,
            "{args:?}\n{}",
            text.stdout
        );
        let roles = roles_of(&json_run.json(), qpath);
        let got: Vec<(Value, Value, Value)> = roles
            .as_array()
            .expect("roles")
            .iter()
            .map(|role| {
                (
                    role["role"].clone(),
                    role["path"].clone(),
                    role["line"].clone(),
                )
            })
            .collect();
        let want: Vec<(Value, Value, Value)> =
            want.iter().map(|line| word_path_line(line)).collect();
        assert_eq!(got, want, "{args:?}: the JSON's order");
    }

    let run = json!([
        {"role": "system", "layer": "a", "path": "crates/farm/src/fields.rs", "line": 26,
         "source": "call", "form": "path", "origin": "code", "group": "Update",
         "state": "ambiguous", "detail": null, "candidates": 3},
        {"role": "system", "layer": "a", "path": "crates/farm/src/lib.rs", "line": 30,
         "source": "call", "form": "path", "origin": "code", "group": "Update",
         "state": "linked", "detail": null, "candidates": null},
    ]);
    let clock = both(
        &home,
        &root,
        &["symbols", "--file", "crates/farm/src/clock.rs"],
    )
    .1;
    assert_eq!(roles_of(&clock.json(), "crates/farm::clock::run"), run);
    let herd = both(
        &home,
        &root,
        &["symbols", "--file", "crates/farm/src/herd.rs"],
    )
    .1;
    assert_eq!(
        roles_of(&herd.json(), "crates/farm::herd::Ghost"),
        json!([{"role": "component", "layer": "a", "path": "crates/farm/src/herd.rs",
                "line": 22, "source": "derive", "form": null, "origin": "code",
                "group": null, "state": "uncertain", "detail": "cfg_attr",
                "candidates": null}])
    );
    let news = both(&home, &root, &["symbols", "Quake"]).1;
    assert_eq!(
        roles_of(&news.json(), "crates/farm::news::Quake"),
        json!([
            {"role": "message", "layer": "a", "path": "crates/farm/src/news.rs", "line": 12,
             "source": "derive", "form": null, "origin": "code", "group": null,
             "state": "certain", "detail": null, "candidates": null},
            {"role": "message", "layer": "a", "path": "crates/farm/src/news.rs", "line": 28,
             "source": "call", "form": "path", "origin": "macro_rules", "group": null,
             "state": "linked", "detail": null, "candidates": null},
        ])
    );

    let pantry = probe(
        &scratch,
        "pantry",
        &[(
            "crates/probe/src/lib.rs",
            "use bevy::prelude::*;\n\n/// What the plot stores.\n#[derive(Resource, Reflect, Component)]\npub struct Pantry;\n",
        )],
    );
    let home = scratch.home("pantry");
    let (text, json_run) = both(
        &home,
        &pantry,
        &["symbols", "--file", "crates/probe/src/lib.rs"],
    );
    text.code(0);
    let want: Vec<String> = ["component", "reflect", "resource"]
        .iter()
        .map(|word| format!("  role {word} | a | crates/probe/src/lib.rs:4 | derive | - | certain"))
        .collect();
    assert_eq!(role_lines_of(&text.stdout, "crates/probe::Pantry"), want);
    let words: Vec<Value> = roles_of(&json_run.json(), "crates/probe::Pantry")
        .as_array()
        .expect("roles")
        .iter()
        .map(|role| role["role"].clone())
        .collect();
    assert_eq!(
        words,
        [json!("component"), json!("reflect"), json!("resource")]
    );
}

/// RD-6. M: a module or RON target `null` with the opt-in (or an item
/// without a row). code-bevy, the opt-in on: `--node CHORE-02` lists the
/// module of `clock.rs` and a RON path of `assets/farm.ron`, each
/// `roles: []`, no role line; `clock.rs`' items without a row
/// (`TICKS_PER_DAY`, `clock::tick`, the latter only a name-tier namesake
/// of `crops::tick`'s rows) `[]` beside `clock::run`'s two rows; `lib.rs`'
/// `mod` items `[]`; `--file assets/farm.ron` lists nothing. Off, the
/// module and the RON path hold `null`.
#[test]
fn non_item_targets_hold_empty_roles() {
    let scratch = Scratch::new("brr-empty");
    let on = bevy_copy(&scratch, "on");
    let off = bevy_copy(&scratch, "off");
    set_code(&off, CODE_OFF);
    let (home_on, home_off) = (scratch.home("on"), scratch.home("off"));

    let (text, json_run) = both(&home_on, &on, &["symbols", "--node", "CHORE-02"]);
    text.code(0);
    assert!(
        !text.stdout.lines().any(|line| line.starts_with("  role ")),
        "{}",
        text.stdout
    );
    let document = json_run.json();
    let listed = targets(&document);
    let kinds: BTreeSet<String> = listed
        .iter()
        .map(|target| target["target"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(kinds, kind_set(&["module", "ron"]), "{}", pretty(&document));
    for target in &listed {
        assert_eq!(target["roles"], json!([]), "with the opt-in: {target}");
    }
    let off_document = both(&home_off, &off, &["symbols", "--node", "CHORE-02"])
        .1
        .json();
    assert_eq!(targets(&off_document).len(), listed.len());
    for target in targets(&off_document) {
        assert_eq!(target.get("roles"), Some(&Value::Null), "off: {target}");
    }

    let clock = both(
        &home_on,
        &on,
        &["symbols", "--file", "crates/farm/src/clock.rs"],
    )
    .1
    .json();
    for qpath in [
        "crates/farm::clock::TICKS_PER_DAY",
        "crates/farm::clock::tick",
    ] {
        assert_eq!(roles_of(&clock, qpath), json!([]), "{qpath}");
    }
    assert_eq!(
        roles_of(&clock, "crates/farm::clock::run")
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    let lib = both(
        &home_on,
        &on,
        &["symbols", "--file", "crates/farm/src/lib.rs"],
    )
    .1
    .json();
    for module in ["clock", "crops", "fields", "herd", "news", "weather"] {
        assert_eq!(
            roles_of(&lib, &format!("crates/farm::{module}")),
            json!([]),
            "the `mod {module};` item"
        );
    }
    let ron = both(&home_on, &on, &["symbols", "--file", "assets/farm.ron"]).1;
    ron.code(0);
    let ron = ron.json();
    assert_eq!(
        (&ron["symbols"], &ron["total"]),
        (&json!([]), &json!(0)),
        "{ron}"
    );
}

/// `[truncated: <k> …]`'s `k`.
fn truncated(line: &str) -> usize {
    line.strip_prefix("[truncated: ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|k| k.parse().ok())
        .unwrap_or_else(|| panic!("a tail: {line:?}"))
}

/// RD-6, the cap. M: role lines outside the cap. A probe of 250 structs,
/// each `#[derive(Component, Resource, Reflect, Message, Event)]` (five
/// rows): their target and `sig` lines alone fit in `OUTPUT_CAP_CHARS`,
/// with their role lines they do not. `spec symbols --file` prints whole
/// targets, each with its five role lines in word order, while the text
/// before the tail stays within the cap, then `[truncated: <k> symbols not
/// shown]` and `symbols: <n> of 250; …`; the JSON holds the printed
/// targets, each with five roles, `omitted` the rest.
#[test]
fn role_lines_count_against_the_cap() {
    const STALLS: usize = 250;
    let mut source = String::from("use bevy::prelude::*;\n\n");
    for index in 0..STALLS {
        source.push_str(&format!(
            "#[derive(Component, Resource, Reflect, Message, Event)]\npub struct Stall{index:04};\n"
        ));
    }
    let scratch = Scratch::new("brr-cap");
    let root = probe(
        &scratch,
        "probe",
        &[("crates/probe/src/lib.rs", source.as_str())],
    );
    let home = scratch.home("h");
    let (text, json_run) = both(
        &home,
        &root,
        &["symbols", "--file", "crates/probe/src/lib.rs"],
    );
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    assert!(lines.len() > 2, "{}", text.show());
    let tail = lines[lines.len() - 2];
    assert!(
        tail.starts_with("[truncated: ") && tail.ends_with(" symbols not shown]"),
        "the role lines count against the cap: {tail}"
    );
    let summary = lines[lines.len() - 1];
    assert!(
        summary.starts_with("symbols: ") && summary.contains(&format!(" of {STALLS}; ")),
        "{summary}"
    );
    let body = &lines[..lines.len() - 2];
    let size = |keep: &dyn Fn(&str) -> bool| -> usize {
        body.iter()
            .filter(|line| keep(line))
            .map(|line| line.chars().count() + 1)
            .sum()
    };
    let before = size(&|_: &str| true);
    assert!(
        before <= OUTPUT_CAP_CHARS,
        "{before} characters before the tail"
    );
    let shown = body
        .iter()
        .filter(|line| item_qpath(line).is_some())
        .count();
    assert!(shown > 0 && shown < STALLS, "{shown} of {STALLS} shown");
    let bare = size(&|line: &str| !line.starts_with("  role "));
    assert!(
        bare * STALLS / shown < OUTPUT_CAP_CHARS,
        "the probe's targets without their role lines fit: about {} characters",
        bare * STALLS / shown
    );
    let roles = body
        .iter()
        .filter(|line| line.starts_with("  role "))
        .count();
    assert_eq!(roles, shown * 5, "whole targets, every role line kept");
    let first: Vec<String> = ["component", "event", "message", "reflect", "resource"]
        .iter()
        .map(|word| format!("  role {word} | a | crates/probe/src/lib.rs:3 | derive | - | certain"))
        .collect();
    assert_eq!(
        role_lines_of(&text.stdout, "crates/probe::Stall0000"),
        first
    );
    assert_eq!(truncated(tail) + shown, STALLS);

    json_run.code(0);
    let value = json_run.json();
    let listed = targets(&value);
    assert_eq!(listed.len(), shown, "text and JSON show the same targets");
    for target in &listed {
        assert_eq!(
            target["roles"].as_array().map(Vec::len),
            Some(5),
            "{target}"
        );
    }
    assert_eq!(value["omitted"], json!({"symbols": STALLS - shown}));
    assert_eq!(value["total"], json!(STALLS));
}

/// The kind of one line of a `spec symbols` answer, or `None` for a line
/// no answer holds (a field's line break).
fn line_kind(line: &str) -> Option<&'static str> {
    if line.starts_with("  sig ") {
        Some("sig")
    } else if let Some(role) = line.strip_prefix("  role ") {
        let fields: Vec<&str> = role.split(" | ").collect();
        let state = fields.last().copied().unwrap_or_default();
        (fields.len() == 6
            && ["certain", "uncertain", "linked", "ambiguous"]
                .iter()
                .any(|word| state == *word || state.starts_with(&format!("{word} "))))
        .then_some("role")
    } else if line.starts_with("symbols: ") {
        Some("summary")
    } else if item_qpath(line).is_some() {
        Some("item")
    } else {
        None
    }
}

/// RD-6, "Fields through `one_line`". M: a field not through `one_line`
/// (the row's path, the only free text of a role line: words, sources,
/// forms, origins, categories are data, a group has its whitespace
/// dropped). A probe whose registrations sit in files whose names hold a
/// line feed and a carriage return: `tend`'s row prints
/// `crates/probe/src/wi re.rs:5`, `water`'s `crates/probe/src/ho ok.rs:5`,
/// `wire`'s own `fn` row `crates/probe/src/wi re.rs:4`, each on one line;
/// every line of the answers a target, `sig`, role line or the summary.
/// The JSON keeps the stored path (R4).
#[test]
fn role_fields_are_one_line() {
    let scratch = Scratch::new("brr-one-line");
    let root = probe(
        &scratch,
        "probe",
        &[
            (
                "crates/probe/src/lib.rs",
                "use bevy::prelude::*;\n\n/// Tends the plot.\npub fn tend() {}\n\n/// Waters the plot.\npub fn water() {}\n",
            ),
            (
                "crates/probe/src/wi\nre.rs",
                "use bevy::prelude::*;\n\n/// Wires the plot.\npub fn wire(app: &mut App) {\n    app.add_systems(Update, tend);\n}\n",
            ),
            (
                "crates/probe/src/ho\rok.rs",
                "use bevy::prelude::*;\n\n/// Hooks the plot.\npub fn hook(app: &mut App) {\n    app.add_systems(Startup, water);\n}\n",
            ),
        ],
    );
    let home = scratch.home("h");
    let (text, json_run) = both(
        &home,
        &root,
        &["symbols", "--file", "crates/probe/src/lib.rs"],
    );
    text.code(0);
    for line in text.stdout.lines() {
        assert!(
            line_kind(line).is_some(),
            "a broken line {line:?}\n{}",
            text.stdout
        );
    }
    assert_eq!(
        role_lines_of(&text.stdout, "crates/probe::tend"),
        ["  role system | a | crates/probe/src/wi re.rs:5 | call | Update | linked"]
    );
    assert_eq!(
        role_lines_of(&text.stdout, "crates/probe::water"),
        ["  role system | a | crates/probe/src/ho ok.rs:5 | call | Startup | linked"]
    );
    let document = json_run.json();
    assert_eq!(
        roles_of(&document, "crates/probe::tend")[0]["path"],
        json!("crates/probe/src/wi\nre.rs"),
        "the JSON keeps the stored path"
    );
    assert_eq!(
        roles_of(&document, "crates/probe::water")[0]["path"],
        json!("crates/probe/src/ho\rok.rs")
    );

    let (text, json_run) = both(
        &home,
        &root,
        &["symbols", "--file", "crates/probe/src/wi\nre.rs"],
    );
    text.code(0);
    for line in text.stdout.lines() {
        assert!(
            line_kind(line).is_some(),
            "a broken line {line:?}\n{}",
            text.stdout
        );
    }
    let roles: Vec<&str> = text
        .stdout
        .lines()
        .filter(|line| line.starts_with("  role "))
        .collect();
    assert_eq!(
        roles,
        ["  role plugin | a | crates/probe/src/wi re.rs:4 | fn | - | certain"]
    );
    let document = json_run.json();
    let wire = targets(&document)
        .into_iter()
        .find(|target| target["name"] == "wire")
        .unwrap_or_else(|| panic!("no wire: {}", pretty(&document)));
    assert_eq!(
        wire["roles"][0]["path"],
        json!("crates/probe/src/wi\nre.rs")
    );
}

/// RD-6, "On a target". M: an `unlinked` row shown on a same-named item
/// (or a row on nothing shown on its enclosing fn). A probe: `Lamp`, a
/// struct, registered as a system (`system` takes `fn` items only:
/// unlinked), `lantern::glow_more` naming nothing (unlinked), an
/// `add_message` of two types (`uncertain` `arguments`, on nothing), all
/// inside the `&mut App` fn `yard`: `spec index` counts two unlinked
/// subjects and one uncertain row, yet `Lamp` and `glow` show no role and
/// `yard` only its own `plugin` row, in every form. Over code-bevy
/// (`DefaultPlugins`, the nested `blink`: unlinked) no form shows an
/// unlinked row.
#[test]
fn unlinked_rows_show_on_no_target() {
    const YARD: &str = "use bevy::prelude::*;\n\n/// A lamp: a thing, never a system.\npub struct Lamp;\n\n/// A glow of its own.\npub fn glow() {}\n\n/// Wires the yard.\npub fn yard(app: &mut App) {\n    app.add_systems(Update, Lamp);\n    app.add_systems(Update, lantern::glow_more);\n    app.add_message::<Lamp, Lamp>();\n}\n";
    let scratch = Scratch::new("brr-unlinked");
    let root = probe(&scratch, "probe", &[("crates/probe/src/lib.rs", YARD)]);
    let home = scratch.home("h");
    let index = spec(&home, &root, &["--json", "index"]);
    index.code(0);
    let counts = index.json()["code"]["roles"].clone();
    assert_eq!(
        (
            &counts["subjects"]["unlinked"],
            &counts["subjects"]["total"],
            &counts["uncertain"]
        ),
        (&json!(2), &json!(2), &json!(1)),
        "the probe's rows: {counts}"
    );
    let yard_role = json!([{"role": "plugin", "layer": "a", "path": "crates/probe/src/lib.rs",
        "line": 10, "source": "fn", "form": null, "origin": "code", "group": null,
        "state": "certain", "detail": null, "candidates": null}]);
    let yard_line = "  role plugin | a | crates/probe/src/lib.rs:10 | fn | - | certain";
    let forms: [&[&str]; 4] = [
        &["symbols", "--file", "crates/probe/src/lib.rs"],
        &["symbols", "Lamp"],
        &["symbols", "glow"],
        &["symbols", "--at", "crates/probe/src/lib.rs:11"],
    ];
    for args in forms {
        let (text, json_run) = both(&home, &root, args);
        text.code(0);
        let document = json_run.json();
        let listed = targets(&document);
        assert!(!listed.is_empty(), "{args:?}: {document}");
        for target in &listed {
            let want = if target["qpath"] == "crates/probe::yard" {
                yard_role.clone()
            } else {
                json!([])
            };
            assert_eq!(target["roles"], want, "{args:?}: {target}");
        }
        let lines: Vec<&str> = text
            .stdout
            .lines()
            .filter(|line| line.starts_with("  role "))
            .collect();
        let want: Vec<&str> = if text.stdout.contains("crates/probe::yard | ") {
            vec![yard_line]
        } else {
            Vec::new()
        };
        assert_eq!(lines, want, "{args:?}\n{}", text.stdout);
    }

    let bevy = bevy_copy(&scratch, "bevy");
    let home = scratch.home("bevy");
    for args in FORMS {
        let (text, json_run) = both(&home, &bevy, args);
        assert!(
            !text.stdout.contains("| unlinked"),
            "{args:?}\n{}",
            text.stdout
        );
        if let Some(document) = document(&json_run) {
            for target in targets(&document) {
                for role in target["roles"].as_array().into_iter().flatten() {
                    assert_ne!(role["state"], "unlinked", "{args:?}: {target}");
                }
            }
        }
    }
}

// ------------------------------------------------------------------ RD-7

/// RD-7, the `symbols` half: annotation only. M: a role alters a target's
/// bindings, `mentions`, tier or `total`. code-bevy with the opt-in on and
/// off, every form of [`FORMS`] (each code file, every exit), text and
/// JSON: the same exit and stderr; the on text without its `  role `
/// lines is the off text byte for byte; the on document without each
/// target's `roles` is the off document. Role lines show in every form.
#[test]
fn symbols_equal_with_and_without_frameworks_but_roles() {
    let scratch = Scratch::new("brr-equal");
    let on = bevy_copy(&scratch, "on");
    let off = bevy_copy(&scratch, "off");
    set_code(&off, CODE_OFF);
    let (home_on, home_off) = (scratch.home("on"), scratch.home("off"));
    let mut forms_with_roles = BTreeSet::new();
    for args in FORMS {
        let (text_on, json_on) = both(&home_on, &on, args);
        let (text_off, json_off) = both(&home_off, &off, args);
        for (left, right) in [(&text_on, &text_off), (&json_on, &json_off)] {
            assert_eq!(
                (left.code, &left.stderr),
                (right.code, &right.stderr),
                "{args:?}\n--- on\n{}--- off\n{}",
                left.show(),
                right.show()
            );
        }
        assert_eq!(
            text_without_roles(&text_on.stdout),
            text_off.stdout,
            "{args:?}: the text but its role lines"
        );
        if let (Some(on_document), Some(off_document)) = (document(&json_on), document(&json_off)) {
            assert_eq!(
                json_without_roles(&on_document),
                json_without_roles(&off_document),
                "{args:?}: the document but `roles`"
            );
        }
        if text_on
            .stdout
            .lines()
            .any(|line| line.starts_with("  role "))
        {
            forms_with_roles.insert(form_of(args));
        }
    }
    assert_eq!(
        forms_with_roles,
        BTreeSet::from(["at", "file", "node", "query"]),
        "role lines in every form"
    );
}

// ------------------------------------------------------------------ TE-5

/// TE-5, code-bevy per form against `expected.json`. M: a row shown on a
/// target it neither sits on nor links to; a candidate's row missing; a
/// field other than the row's. Every form of [`FORMS`]: the opt-in text is
/// the same form's text without the opt-in with, after each item target's
/// `sig` line, the lines of the rows `expected.json` shows on it, in the
/// spec's order; the opt-in document is the same with each item's `roles`
/// those rows' objects and each module or RON target's `[]`. Every row
/// that shows anywhere (25 of the 27: the two `unlinked` show nowhere) is
/// printed under some form.
#[test]
fn symbols_roles_match_expected_json_in_every_form() {
    let rows = expected_roles();
    assert_eq!(rows.len(), 27, "10a-1's 27 rows");
    let scratch = Scratch::new("brr-expected");
    let on = bevy_copy(&scratch, "on");
    let off = bevy_copy(&scratch, "off");
    set_code(&off, CODE_OFF);
    let (home_on, home_off) = (scratch.home("on"), scratch.home("off"));
    let mut printed = BTreeSet::new();
    let mut failures = Vec::new();
    for args in FORMS {
        let (text_on, json_on) = both(&home_on, &on, args);
        let (text_off, json_off) = both(&home_off, &off, args);
        let want = with_role_lines(&text_off.stdout, &rows);
        if text_on.stdout != want {
            failures.push(format!(
                "{args:?}: text\n--- got\n{}--- want\n{want}",
                text_on.stdout
            ));
        }
        if let (Some(got), Some(off_document)) = (document(&json_on), document(&json_off)) {
            let want = with_roles(&off_document, &rows);
            if got != want {
                failures.push(format!(
                    "{args:?}: JSON\n--- got\n{}\n--- want\n{}",
                    pretty(&got),
                    pretty(&want)
                ));
            }
        }
        for line in text_on.stdout.lines() {
            if let Some(qpath) = item_qpath(line) {
                for row in rows.iter().filter(|row| shows_on(row, qpath)) {
                    printed.insert((
                        row["path"].as_str().expect("a path").to_owned(),
                        row["ord"].as_u64().expect("an ord"),
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    let shown_somewhere = rows
        .iter()
        .filter(|row| {
            !row["sits_on"].is_null()
                || matches!(row["state"].as_str(), Some("linked" | "ambiguous"))
        })
        .count();
    assert_eq!(shown_somewhere, 25, "every row but the two unlinked");
    assert_eq!(printed.len(), shown_somewhere, "{printed:?}");
}

/// TE-5, genre ("Rules and edge cases"): without `[code] frameworks` the
/// reads are 6b-1's but `roles: null` (the text goldens:
/// `symbol_reads.rs` `symbols_match_expected_json`, the derived JSON
/// `roles: null`). `fixtures/code-index` and `fixtures/code-a`, as they
/// stand: no form prints a role line, every target (items, modules, RON
/// paths) holds `roles: null`; `fixtures/spec-c` (no `[code]`, not Rust)
/// answers `symbols` with the code-table line, exit 2, as before. M: `[]`
/// without the opt-in.
#[test]
fn code_index_and_code_a_hold_null_roles_and_spec_c_is_unchanged() {
    let scratch = Scratch::new("brr-genre");
    for (fixture, forms) in [
        ("code-index", &CODE_INDEX_FORMS[..]),
        ("code-a", &CODE_A_FORMS[..]),
    ] {
        let root = code_index_common::copy(&scratch, fixture, fixture);
        let home = scratch.home(fixture);
        let mut kinds = BTreeSet::new();
        for args in forms {
            let (text, json_run) = both(&home, &root, args);
            assert!(
                !text.stdout.lines().any(|line| line.starts_with("  role ")),
                "{fixture} {args:?}\n{}",
                text.stdout
            );
            let Some(document) = document(&json_run) else {
                continue;
            };
            for target in targets(&document) {
                kinds.insert(target["target"].as_str().unwrap_or_default().to_owned());
                assert_eq!(
                    target.get("roles"),
                    Some(&Value::Null),
                    "{fixture} {args:?}: {target}"
                );
            }
        }
        assert_eq!(
            kinds,
            kind_set(&["item", "module", "ron"]),
            "{fixture}: every kind of target read"
        );
    }
    let root = scratch.copy("spec-c", "spec-c");
    let home = scratch.home("spec-c");
    for args in [
        &["symbols", "anything"][..],
        &["--json", "symbols", "anything"],
    ] {
        let run = spec(&home, &root, args);
        run.code(2);
        assert_eq!(
            (run.stdout.as_str(), run.stderr.as_str()),
            ("", NO_TABLE),
            "{args:?}"
        );
    }
}

/// TE-5: a scratch `fixtures/code-a` with `frameworks = ["bevy"]` (a
/// counter library: nothing of Bevy's; a file with a parse error): every
/// form's text, stderr and exit are those without the opt-in, every
/// target (items, modules, RON paths) holds `roles: []`, the documents
/// otherwise alike. M: `null` with the opt-in; a row on a non-Bevy item.
#[test]
fn code_a_with_frameworks_holds_empty_roles_everywhere() {
    let scratch = Scratch::new("brr-code-a");
    let on = code_index_common::copy(&scratch, "code-a", "on");
    replace(
        &on,
        "specengine.toml",
        CODE_A_EXCLUDE,
        &format!("{CODE_A_EXCLUDE}frameworks = [\"bevy\"]\n"),
    );
    let off = code_index_common::copy(&scratch, "code-a", "off");
    let (home_on, home_off) = (scratch.home("on"), scratch.home("off"));
    let mut kinds = BTreeSet::new();
    for args in CODE_A_FORMS {
        let (text_on, json_on) = both(&home_on, &on, args);
        let (text_off, json_off) = both(&home_off, &off, args);
        assert_eq!(
            (text_on.code, &text_on.stdout, &text_on.stderr),
            (text_off.code, &text_off.stdout, &text_off.stderr),
            "{args:?}"
        );
        assert_eq!(json_on.code, json_off.code, "{args:?}");
        let (Some(on_document), Some(off_document)) = (document(&json_on), document(&json_off))
        else {
            continue;
        };
        for target in targets(&on_document) {
            kinds.insert(target["target"].as_str().unwrap_or_default().to_owned());
            assert_eq!(target["roles"], json!([]), "{args:?}: {target}");
        }
        assert_eq!(
            json_without_roles(&on_document),
            json_without_roles(&off_document),
            "{args:?}"
        );
    }
    assert_eq!(kinds, kind_set(&["item", "module", "ron"]));
}

// ------------------------------------------------- phase B: the developer's

/// The SQL of the store's `const <name>: &str = concat!(…)` as written in
/// `crates/specengine-store/src/roles.rs`: its string literals joined,
/// `role_columns!()` replaced by that macro's literal (the statements are
/// the crate's own; the source is read, never copied here).
fn store_role_sql(name: &str) -> String {
    let source = read_text(&repository_root(), "crates/specengine-store/src/roles.rs");
    let at = source
        .find("macro_rules! role_columns")
        .expect("the role_columns! macro");
    let columns = source[at..].split('"').nth(1).expect("the macro's literal");
    let start = source
        .find(&format!("const {name}: &str = concat!("))
        .unwrap_or_else(|| panic!("no {name} in the store's roles.rs"));
    let body = &source[start..];
    let body = &body[..body.find(");").expect("the concat! closes")];
    body.replace("role_columns!()", &format!("\"{columns}\""))
        .split('"')
        .skip(1)
        .step_by(2)
        .collect()
}

/// The lines of `EXPLAIN QUERY PLAN <sql>` over the index at `db`, by the
/// system's `sqlite3` (a plan only, nothing written; parameters unbound;
/// not `-readonly`: the index is in WAL mode, whose `-shm` a read-only
/// open cannot make).
fn plan(db: &Path, sql: &str) -> Vec<String> {
    let output = Command::new(common::proposal::sqlite3())
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(format!("EXPLAIN QUERY PLAN {sql};"))
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "{sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("sqlite3 prints UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The developer's iteration 1 suggestion (docs/canon/code-roles.md
/// "Known limits": `code_roles_by_subject_name` gains its reader). M: a
/// statement that scans `code_roles` or `code_files`. The store's two
/// targeted role statements, as its source writes them, planned by SQLite over a
/// code-bevy index (no `ANALYZE`: the plan a large table gets):
/// `NAMED_ROLES_SQL` searches `code_roles_by_subject_name`, then the file
/// by its key; `FILE_ROLES_SQL` searches the file by its `(wt, path)` key,
/// then its rows by `(file, ord)`; neither scans a table.
#[test]
fn role_reads_search_their_indexes() {
    let scratch = Scratch::new("brr-plan");
    let root = bevy_copy(&scratch, "bevy");
    let home = scratch.home("h");
    spec(&home, &root, &["index"]).code(0);
    let db = db(&home, "code-bevy");
    for (name, searches) in [
        ("NAMED_ROLES_SQL", ["code_roles_by_subject_name", "rowid=?"]),
        ("FILE_ROLES_SQL", ["wt=? AND path=?", "file=?"]),
    ] {
        let sql = store_role_sql(name);
        assert!(
            sql.trim_start().starts_with("SELECT ") && sql.contains(" FROM "),
            "{name}: {sql}"
        );
        let lines = plan(&db, &sql);
        for search in searches {
            assert!(
                lines
                    .iter()
                    .any(|line| line.contains("SEARCH") && line.contains(search)),
                "{name}: no search on {search:?}: {lines:#?}\n{sql}"
            );
        }
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("SCAN") && !line.contains("CONSTANT ROW")),
            "{name}: a scan: {lines:#?}\n{sql}"
        );
    }
}

/// B5 (the developer's iteration 1; `CODE_INDEX_FORMAT` 4 by the owner's
/// K2; the review's pin). M: the qualifier kept as written (`<Foo<A, B> as
/// Tr>::f`, which meets no suffix: ambiguous at the name tier). A probe
/// registers `<Foo<A, B> as Tr>::f` beside a free `fn f` and the trait's
/// own `fn f();`: the stored subject is `<Foo<A,B> as Tr>::f` (each side
/// compacted, as the impl's qpath names it), its name `f`; `spec index`
/// counts one subject, linked; the impl's method shows the row (`linked`,
/// its line the leaf's), the free `f` and the trait's declaration none.
#[test]
fn a_spaced_qualified_subject_is_stored_compact_and_links_by_suffix() {
    const PAIR: &str = "use bevy::prelude::*;\n\n/// A seam.\npub trait Tr {\n    fn f();\n}\n\n/// A pair.\npub struct Foo<A, B>(A, B);\n\nimpl<A, B> Tr for Foo<A, B> {\n    fn f() {}\n}\n\n/// A namesake.\npub fn f() {}\n\n/// Wires the pair.\npub fn wire(app: &mut App) {\n    app.add_systems(Update, <Foo<A, B> as Tr>::f);\n}\n";
    let scratch = Scratch::new("brr-b5");
    let root = probe(&scratch, "probe", &[("crates/probe/src/lib.rs", PAIR)]);
    let home = scratch.home("h");
    let index = spec(&home, &root, &["--json", "index"]);
    index.code(0);
    let counts = index.json()["code"]["roles"].clone();
    assert_eq!(
        (
            &counts["subjects"]["total"],
            &counts["subjects"]["linked"],
            &counts["subjects"]["ambiguous"]
        ),
        (&json!(1), &json!(1), &json!(0)),
        "{counts}"
    );
    let stored = query(
        &db(&home, PROBE_SLUG),
        "SELECT subject, subject_name FROM code_roles WHERE subject IS NOT NULL",
    );
    assert_eq!(stored.len(), 1, "{stored:?}");
    assert_eq!(
        (&stored[0]["subject"], &stored[0]["subject_name"]),
        (&json!("<Foo<A,B> as Tr>::f"), &json!("f")),
        "the stored subject"
    );

    let (text, json_run) = both(
        &home,
        &root,
        &["symbols", "--file", "crates/probe/src/lib.rs"],
    );
    text.code(0);
    let document = json_run.json();
    let methods: Vec<Value> = targets(&document)
        .into_iter()
        .filter(|target| {
            target["item_kind"] == "function_item"
                && target["qpath"]
                    .as_str()
                    .is_some_and(|qpath| qpath.ends_with(" as Tr>::f"))
        })
        .collect();
    assert_eq!(methods.len(), 1, "the impl's method: {}", pretty(&document));
    let method = methods[0]["qpath"].as_str().expect("a qpath").to_owned();
    assert_eq!(
        role_lines_of(&text.stdout, &method),
        ["  role system | a | crates/probe/src/lib.rs:20 | call | Update | linked"],
        "{}",
        text.stdout
    );
    for target in targets(&document) {
        let qpath = target["qpath"].as_str().unwrap_or_default();
        if qpath == method || target["name"] == "wire" {
            continue;
        }
        assert_eq!(target["roles"], json!([]), "{qpath}: {target}");
    }
}

/// D1 (the developer's iteration 1: `frameworks::PATH_FORM`,
/// `frameworks::CODE_ORIGIN`, which the CLI compares to drop the defaults
/// from a role line). M: either constant a word the data never stores.
/// code-bevy's stored rows: the path leaf `crops::tick::<Wheat>` holds form
/// `PATH_FORM` and origin `CODE_ORIGIN`; the stored forms are `PATH_FORM`,
/// `closure`, `factory`, the origins `CODE_ORIGIN`, `macro_call`,
/// `macro_rules`; the text of `crops::tick`'s rows prints the source
/// alone.
#[test]
fn path_form_and_code_origin_are_the_stored_words() {
    let scratch = Scratch::new("brr-words");
    let root = bevy_copy(&scratch, "bevy");
    let home = scratch.home("h");
    spec(&home, &root, &["index"]).code(0);
    let rows = role_rows(&db(&home, "code-bevy"));
    let tick = rows
        .iter()
        .find(|row| row["path"] == "crates/farm/src/lib.rs" && row["ord"] == 1)
        .unwrap_or_else(|| panic!("no lib.rs row 1: {rows:?}"));
    assert_eq!(
        (tick["form"].as_str(), tick["origin"].as_str()),
        (Some(PATH_FORM), Some(CODE_ORIGIN)),
        "{tick:?}"
    );
    let forms: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["form"].as_str())
        .map(str::to_owned)
        .collect();
    assert_eq!(forms, kind_set(&[PATH_FORM, "closure", "factory"]));
    let origins: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["origin"].as_str())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        origins,
        kind_set(&[CODE_ORIGIN, "macro_call", "macro_rules"])
    );
    let text = spec(
        &home,
        &root,
        &["symbols", "--file", "crates/farm/src/crops.rs"],
    );
    text.code(0);
    let lines = role_lines_of(&text.stdout, "crates/farm::crops::tick");
    assert_eq!(lines.len(), 3, "{}", text.stdout);
    for line in &lines {
        let fields: Vec<&str> = line.split(" | ").collect();
        assert_eq!(fields.get(3), Some(&"call"), "the source alone: {line}");
    }
}
