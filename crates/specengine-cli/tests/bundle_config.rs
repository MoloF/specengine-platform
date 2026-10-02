//! docs/features/spec-cli-bundle.md, config, writes and genre: AC-13 (the
//! budget's sources; `[budgets] bundle_node` read alone), AC-15 (read only
//! but the data directory; no table added; an edit shows without `spec
//! index`), AC-16 (`INDEX_FORMAT` 6, the format history unchanged), AC-18
//! (a synthetic non-Rust corpus: no P2-3 word in a bundle or the bundle
//! sources). Scratch copies of `fixtures/spec-a`, each run with its own
//! `HOME`; the fixtures are only read.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::bundle::{
    STACK_WORDS, all_layers, bundle, bundle_json, has_word, literals, names, split_text,
};
use common::graph::spec30;
use common::{
    Scratch, data_dir, index, paths_under, read_text, replace, repository_root, snapshot, write,
};

/// spec-a's config with `tail` appended; the line of the first appended
/// line is returned.
fn append_config(root: &Path, tail: &str) -> usize {
    let text = read_text(root, "specengine.toml");
    let first = text.lines().count() + 1;
    write(root, "specengine.toml", format!("{text}{tail}"));
    first
}

/// AC-13: `[budgets] bundle_node = 300` → `budget` 300; absent → 2 000;
/// `--budget 500` over it; `bundle_node` 0, `-1`, 2^32, a string or a
/// float → exit 2 at its line, nothing on stdout; a broken `[classes]`,
/// `[check]` or another `[budgets]` key → exit 0, as `spec show`; below
/// the minimum, the error names the config line. M: a hard-coded default;
/// the budget read through the whole check config.
#[test]
fn ac13_the_budget_comes_from_the_flag_the_config_or_the_default() {
    let scratch = Scratch::new("bundle-ac13");
    let home = scratch.home("h");

    let plain = scratch.copy("spec-a", "plain");
    assert_eq!(bundle_json(&home, &plain, &["MEC-STAMINA"])["budget"], 2000);
    let run = bundle(&home, &plain, &["MEC-STAMINA"]);
    assert!(
        split_text(&run.stdout).totals.contains(" of 2000, "),
        "{}",
        run.show()
    );

    let set = scratch.copy("spec-a", "set");
    append_config(&set, "\n[budgets]\nbundle_node = 300\n");
    assert_eq!(bundle_json(&home, &set, &["MEC-STAMINA"])["budget"], 300);
    let json = bundle_json(&home, &set, &["MEC-STAMINA"]);
    assert!(json["tokens"].as_u64().unwrap() <= 300, "{json}");
    assert_eq!(
        bundle_json(&home, &set, &["MEC-STAMINA", "--budget", "500"])["budget"],
        500
    );

    // Other `[budgets]` keys only: the default.
    let others = scratch.copy("spec-a", "others");
    append_config(&others, "\n[budgets]\ntier1_bytes = 10240\n");
    assert_eq!(
        bundle_json(&home, &others, &["MEC-STAMINA"])["budget"],
        2000
    );

    for (case, value) in [
        ("zero", "0"),
        ("negative", "-1"),
        ("over u32", "4294967296"),
        ("a string", "\"300\""),
        ("a float", "300.5"),
    ] {
        let root = scratch.copy("spec-a", &format!("bad-{}", case.replace(' ', "-")));
        let line = append_config(&root, &format!("\n[budgets]\nbundle_node = {value}\n")) + 2;
        for args in [
            &["bundle", "MEC-STAMINA"][..],
            &["--json", "bundle", "MEC-STAMINA"][..],
        ] {
            let run = spec30(&home, &root, args);
            run.code(2);
            assert!(run.stdout.is_empty(), "{case}: {}", run.show());
            assert!(
                run.stderr.starts_with(&format!("specengine.toml:{line}: "))
                    && run.stderr.lines().count() == 1,
                "{case}: line {line}: {}",
                run.show()
            );
            assert!(run.stderr.contains("bundle_node"), "{case}: {}", run.show());
        }
    }

    // Broken tables `spec check` judges never stop a bundle.
    for (case, tail, budget) in [
        ("classes", "\n[classes]\ncanon = 5\n", 2000),
        (
            "check",
            "\n[check]\nmode = \"sometimes\"\nflavour = 1\n",
            2000,
        ),
        (
            "budgets",
            "\n[budgets]\ntier1_bytes = \"big\"\nmystery = true\nbundle_node = 400\n",
            400,
        ),
    ] {
        let root = scratch.copy("spec-a", &format!("broken-{case}"));
        append_config(&root, tail);
        let show = spec30(&home, &root, &["show", "MEC-STAMINA"]);
        show.code(0);
        let json = bundle_json(&home, &root, &["MEC-STAMINA"]);
        assert_eq!(json["budget"], budget, "{case}: {json}");
    }

    // Below the minimum, the error names the config line.
    let low = scratch.copy("spec-a", "low");
    let line = append_config(&low, "\n[budgets]\nbundle_node = 5\n") + 2;
    let run = bundle(&home, &low, &["MEC-STAMINA"]);
    run.code(2);
    assert!(run.stdout.is_empty(), "{}", run.show());
    assert!(
        run.stderr.contains(&format!(
            "`[budgets] bundle_node = 5` (specengine.toml:{line})"
        )) && run.stderr.contains("minimum of "),
        "{}",
        run.show()
    );
}

/// R9 / AC-13, the gate's side: `spec check` and `spec bundle` agree on
/// `bundle_node`'s range. `u32::MAX` passes both (the bundle's `budget`
/// is it); `u32::MAX + 1`, 0 and -1 are refused by both at the same line,
/// `spec check` as one cannot-check cause in the range's words, exit 2.
#[test]
fn the_gate_and_the_bundle_agree_on_the_bundle_node_range() {
    let scratch = Scratch::new("bundle-node-range");
    let home = scratch.home("h");

    let top = scratch.copy("spec-a", "top");
    append_config(&top, "\n[budgets]\nbundle_node = 4294967295\n");
    let check = spec30(&home, &top, &["check"]);
    assert_ne!(check.code, 2, "{}", check.show());
    assert!(
        !check
            .stdout
            .lines()
            .any(|line| line.starts_with("cannot  ")),
        "{}",
        check.show()
    );
    assert_eq!(
        bundle_json(&home, &top, &["MEC-STAMINA"])["budget"],
        4_294_967_295_u64
    );

    for value in ["4294967296", "0", "-1"] {
        let root = scratch.copy("spec-a", &format!("refused{value}"));
        let line = append_config(&root, &format!("\n[budgets]\nbundle_node = {value}\n")) + 2;
        let check = spec30(&home, &root, &["check"]);
        check.code(2);
        let causes: Vec<&str> = check
            .stdout
            .lines()
            .filter(|line| line.starts_with("cannot  "))
            .collect();
        assert_eq!(
            causes,
            [format!(
                "cannot  specengine.toml:{line}: `bundle_node` must be from 1 to 4294967295, not {value}"
            )],
            "{value}: {}",
            check.show()
        );
        assert!(
            check.stdout.trim_end().ends_with(" — cannot-check"),
            "{value}: {}",
            check.show()
        );
        let run = bundle(&home, &root, &["MEC-STAMINA"]);
        run.code(2);
        assert!(
            run.stderr.starts_with(&format!("specengine.toml:{line}: "))
                && run.stderr.contains("bundle_node"),
            "{value}: {}",
            run.show()
        );
    }
}

/// The `CREATE …` statements in the bytes of the index database files
/// under `home` (the schema text SQLite stores verbatim).
fn schema_statements(home: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let dir = data_dir(home);
    for entry in fs::read_dir(&dir).expect("the data directory") {
        let bytes = fs::read(entry.unwrap().path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        for (at, _) in text.match_indices("CREATE ") {
            let statement: String = text[at..]
                .chars()
                .take_while(|c| *c != '(' && *c != '\0' && !c.is_control())
                .take(120)
                .collect();
            found.insert(statement.trim().to_owned());
        }
    }
    found
}

/// AC-15: after any call the copy is byte- and path-identical, new files
/// only under `HOME`; the database's tables are unchanged; an edit (a new
/// record, a changed summary) shows in the next bundle without `spec
/// index`. M: `update` skipped; a log table.
#[test]
fn ac15_bundle_writes_only_the_data_directory_and_reads_fresh_files() {
    let scratch = Scratch::new("bundle-ac15");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let tree_before = snapshot(&root);
    let paths_before: BTreeSet<String> = paths_under(scratch.path()).into_iter().collect();
    index(&home, &root);
    let schema = schema_statements(&home);
    assert!(
        schema
            .iter()
            .any(|statement| statement.starts_with("CREATE TABLE")),
        "{schema:?}"
    );
    for args in [
        &["bundle", "MEC-STAMINA"][..],
        &["--json", "bundle", "MEC-STAMINA", "--budget", "100"][..],
        &["bundle", "MEC-STAMINA", "MEC-SPRINT", "--budget", "10000"][..],
        &["bundle", "MEC-NOPE"][..],
        &["bundle", "MEC-STAMINA", "--budget", "0"][..],
        &["bundle", "DEC-0007"][..],
    ] {
        spec30(&home, &root, args);
    }
    assert_eq!(snapshot(&root), tree_before, "the copy changed");
    let home_prefix = "homes/h";
    for path in paths_under(scratch.path()) {
        if !paths_before.contains(&path) {
            assert!(
                path.starts_with(home_prefix),
                "a new path outside HOME: {path}"
            );
        }
    }
    assert_eq!(
        schema_statements(&home),
        schema,
        "the database's tables changed"
    );

    // A new record and an edited summary, no `spec index`.
    let before = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    write(
        &root,
        "docs/records/A/A-103.md",
        "---\nid: A-103\nclass: canon\nstatus: open\nowner: owner\nreviewed: 2026-09-20\nlinks:\n  \
         constrains: [MEC-STAMINA]\n---\n\n# Freshly written\n\nWritten after the last bundle.\n",
    );
    replace(
        &root,
        "docs/records/DEC/DEC-0023.md",
        "(A-101 becomes the rule)",
        "(A-101 is now the rule)",
    );
    let after = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(
        names(&after, "neighbours"),
        ["A-101", "A-103", "R-12", "MEC-SPRINT"],
        "{after}"
    );
    assert!(
        after["body"]
            .as_str()
            .unwrap()
            .contains("(A-101 is now the rule)"),
        "{after}"
    );
    assert_ne!(after["bundle_hash"], before["bundle_hash"]);
    assert_eq!(
        schema_statements(&home),
        schema,
        "the database's tables changed"
    );
}

/// AC-16: `INDEX_FORMAT` stays 6 and the store's format history is the
/// one at HEAD (its six lines). M: estimator weights changed, the stamp
/// kept (the store's `format.rs` turns red on the dump; this pins the
/// stamp and the file).
#[test]
fn ac16_index_format_stays_six() {
    assert_eq!(specengine_store::INDEX_FORMAT, 6);
    let history = read_text(
        &repository_root(),
        "crates/specengine-store/tests/format_history.txt",
    );
    assert_eq!(
        history,
        "\
1 4fd55fa4cf7716f7acc45556ab7c831f1f67d7decdf2f0829ad0a7da5e52a906
2 2cfc299da177807c83d246aea934a73ab8509021386568a0b2011a31de66c9aa
3 d63c06dc03ed97243ff25dfb14a165cfec059582eee05dc72d9fba05b3c5f55f
4 d958fe30af22feb15f1dbd8fafa67ef1113a8c4ba267c4dd71a8968eb30bf5b5
5 df99199e747d5d2f16712fd23737a443457140d4bb6e37d59393c78724809be4
6 f014f154e83805144ef39e54ee05e18a5d110ec0d93f6005481dd73f597a70a9
"
    );
}

/// A garden-planning corpus: no programming stack, its own prefixes and
/// kinds, every layer's link type present.
fn garden(root: &Path) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"garden-plan\"\n\n[ids]\n\
         PLOT   = { kind = \"area\",     shape = \"name\" }\n\
         CROP   = { kind = \"plant\",    shape = \"name\" }\n\
         CARE   = { kind = \"routine\",  shape = \"name\" }\n\
         ASK    = { kind = \"doubt\",    width = 2 }\n\
         GUESS  = { kind = \"hunch\",    width = 2 }\n\
         CHOICE = { kind = \"ruling\",   width = 3 }\n\
         CHK    = { kind = \"proof\",    width = 2, scope = \"feature\" }\n\
         WORD   = { kind = \"glossary\", shape = \"name\" }\n",
    );
    let canon = |id: &str, extra: &str, title: &str, body: &str| {
        format!(
            "---\nid: {id}\nclass: canon\n{extra}owner: gardener\nreviewed: 2026-09-20\n---\n\n# {title}\n\n{body}"
        )
    };
    write(
        root,
        "docs/spec/garden.md",
        canon(
            "PLOT-GARDEN",
            "status: accepted\n",
            "The garden",
            "Raised beds behind the house.\n",
        ),
    );
    write(
        root,
        "docs/spec/tomato.md",
        canon(
            "CROP-TOMATO",
            "parent: PLOT-GARDEN\nstatus: accepted\nlinks:\n  depends_on: [CROP-BASIL]\n  \
             derived_from: [GUESS-01]\n  uses_term: [WORD-mulch]\n",
            "Tomatoes",
            "Tomatoes grow in the sunny bed.\n\n## Watering {#CARE-WATER}\n\nWater at dawn.\n",
        ),
    );
    write(
        root,
        "docs/spec/basil.md",
        canon(
            "CROP-BASIL",
            "parent: PLOT-GARDEN\nlinks:\n  constrains: [CROP-TOMATO]\n",
            "Basil",
            "Basil shades the tomato roots.\n",
        ),
    );
    write(
        root,
        "docs/records/ASK-01.md",
        "---\nid: ASK-01\nclass: canon\nstatus: open\nworking_answer: GUESS-01\nrefs: [CROP-TOMATO]\n\
         owner: gardener\nreviewed: 2026-09-20\n---\n\n# Water twice in a heat wave?\n\nOnly if the soil is dry.\n",
    );
    write(
        root,
        "docs/records/GUESS-01.md",
        canon(
            "GUESS-01",
            "status: open\n",
            "Dawn watering is enough",
            "Assumed for a mild summer.\n",
        ),
    );
    write(
        root,
        "docs/records/CHOICE-001.md",
        "---\nid: CHOICE-001\nclass: decision\nstatus: accepted\ndate: 2026-09-01\n\
         canon: docs/spec/tomato.md#watering\nscope: [watering]\n---\n\n# Drip lines for the beds\n\n\
         The beds get drip lines.\n",
    );
    write(
        root,
        "docs/records/WORD-mulch.md",
        canon(
            "WORD-mulch",
            "",
            "Mulch",
            "A layer of straw over the soil.\n",
        ),
    );
    write(
        root,
        "docs/features/watering.md",
        "---\nclass: spec\nstatus: draft\nscope: [watering]\n---\n\n# Watering schedule\n\n\
         Sets the schedule.\n\n## Criteria\n\n### Soil stays moist {#CHK-01}\n\n\
         Checks CROP-TOMATO after a dry week.\n",
    );
}

/// AC-18: a synthetic non-Rust corpus yields a bundle (text and JSON,
/// every layer but bindings and tests filled) with no P2-3 word as a
/// whole word; no literal of the bundle sources holds one. M: a "run
/// `cargo nextest`" hint in the bundle.
#[test]
fn ac18_a_non_rust_corpus_gets_no_stack_word() {
    let scratch = Scratch::new("bundle-ac18");
    let home = scratch.home("h");
    let root = scratch.dir("garden");
    garden(&root);
    let mut texts = Vec::new();
    for budget in ["10000", "150"] {
        let json = bundle_json(&home, &root, &["CROP-TOMATO", "--budget", budget]);
        if budget == "10000" {
            for (layer, items) in all_layers(&json) {
                assert_eq!(
                    items.is_empty(),
                    ["bindings", "tests"].contains(&layer),
                    "{layer}: {json}"
                );
            }
        }
        let run = bundle(&home, &root, &["CROP-TOMATO", "--budget", budget]);
        run.code(0);
        texts.push(run.stdout + &run.stderr);
        texts.push(json.to_string());
    }
    let run = spec30(&home, &root, &["bundle", "CROP-NOPE"]);
    run.code(1);
    texts.push(run.stdout + &run.stderr);
    let run = bundle(&home, &root, &["CROP-TOMATO", "--budget", "1"]);
    run.code(2);
    texts.push(run.stderr);
    for text in &texts {
        for word in STACK_WORDS {
            assert!(!has_word(text, word), "{word:?} in a bundle:\n{text}");
        }
    }

    let repository = repository_root();
    let mut offenders = Vec::new();
    for source in [
        "crates/specengine-core/src/check/bundle.rs",
        "crates/specengine-cli/src/bundle.rs",
    ] {
        for (line, literal) in literals(&read_text(&repository, source)) {
            for word in STACK_WORDS {
                if has_word(&literal, word) {
                    offenders.push(format!("{source}:{line}: {literal:?}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "stack words:\n{}",
        offenders.join("\n")
    );
}
