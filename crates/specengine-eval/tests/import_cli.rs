//! `specengine-eval import` end to end on the real binary
//! (docs/features/import-records.md, "Acceptance criteria" AC-02 to AC-06,
//! AC-08 and AC-09; AC-01 is `import_genre.rs`, AC-07 is
//! `crates/specengine-import/tests/import_hash.rs`).
//!
//! The corpora are the two committed invented conventions
//! `fixtures/import-one` (the default fixture, config `census.toml` at its
//! root) and `fixtures/import-two`, scratch copies of them, and the AC-05
//! copy generated at test time (non-Latin text from `\u{...}` escapes,
//! `docs/canon/architecture.md` "Repository language").
//! The fixtures are only read; every run writes under a scratch `--out`.

mod import_support;
#[cfg(unix)]
mod pilot;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use import_support::*;
use serde_json::Value;

/// What each convention holds for the definition / reference rule, the
/// unclaimed counter, the walk and the code scan.
struct Convention {
    name: &'static str,
    /// Defined once, cited in a reference table and in a reference path.
    cited: &'static str,
    reference_table: &'static str,
    reference_path: &'static str,
    /// A feature-scoped ID defined in two documents.
    feature: &'static str,
    feature_documents: [&'static str; 2],
    /// The one reference no definition answers.
    unresolved: &'static str,
    /// The project ID defined twice, and the file whose removal leaves it once.
    duplicate: &'static str,
    duplicate_file: &'static str,
    /// A document defining `cited` a second time (path, text).
    second_definition: (&'static str, &'static str),
    /// A document a planted token is appended to, and the token.
    planted: (&'static str, &'static str),
    /// The one feature-scoped ID cited from a document that does not define
    /// it: (path, line, token).
    feature_outside: (&'static str, u64, &'static str),
    /// The ID scheme `parse` needs (written to scratch).
    scheme: &'static str,
    /// An excluded file, a file of an unlisted extension, a file under a
    /// dot-directory: none is a document.
    not_documents: [&'static str; 3],
    /// Every expected code citation: (document, code file, line).
    citations: [(&'static str, &'static str, u64); 3],
    /// Files under the code roots that must not be scanned or cited.
    not_cited: &'static [&'static str],
}

const ONE: Convention = Convention {
    name: "import-one",
    cited: "REQ-001",
    reference_table: "spec/index.md",
    reference_path: "spec/map.md",
    feature: "AC-001",
    feature_documents: ["spec/feature-export.md", "spec/feature-login.md"],
    unresolved: "REQ-099",
    duplicate: "REQ-006",
    duplicate_file: "spec/overlap.md",
    second_definition: (
        "spec/second.md",
        "# Second\n\n| ID | Statement |\n|---|---|\n| REQ-001 | Defined a second time. |\n",
    ),
    planted: ("spec/notes.md", "ZQ-777"),
    feature_outside: ("spec/notes.md", 11, "AC-002"),
    scheme: "[ids]\nREQ = { kind = \"requirement\", width = 3 }\nAC = { kind = \"criterion\", width = 3 }\n",
    not_documents: [
        "spec/archive/old.md",
        "spec/readme.txt",
        "spec/.drafts/draft.md",
    ],
    citations: [
        ("spec/requirements.md", "src/lib.rs", 3),
        ("spec/requirements.md", "src/lib.rs", 4),
        ("spec/design.md", "tests/design.rs", 3),
    ],
    not_cited: &["src/notes.txt"],
};

const TWO: Convention = Convention {
    name: "import-two",
    cited: "DEC-0001",
    reference_table: "pages/catalog.mdown",
    reference_path: "pages/refs/sources.mdown",
    feature: "CRT-0001",
    feature_documents: ["log/criteria-b.markdown", "log/criteria.markdown"],
    unresolved: "DEC-0099",
    duplicate: "DEC-0008",
    duplicate_file: "log/duplicate.markdown",
    second_definition: (
        "log/second.markdown",
        "# Second\n\n| # | Code | Decision |\n|---|---|---|\n| 1 | DEC-0001 | Defined a second time. |\n",
    ),
    planted: ("pages/wiki.mdown", "XQ-7777"),
    feature_outside: ("pages/wiki.mdown", 9, "CRT-0003"),
    scheme: "[ids]\nDEC = { kind = \"decision\", width = 4 }\nCRT = { kind = \"criterion\", width = 4 }\n",
    not_documents: [
        "log/draft-1.markdown",
        "log/notes.md",
        "pages/.cache/x.mdown",
    ],
    citations: [
        ("log/decisions.markdown", "tools/check.py", 3),
        ("log/decisions.markdown", "tools/check.py", 4),
        ("pages/spec-x.mdown", "tools/check.py", 5),
    ],
    not_cited: &["tools/readme.txt", "tools/generated/out.py"],
};

const CONVENTIONS: [&Convention; 2] = [&ONE, &TWO];

fn expected(name: &str) -> Value {
    read_json(&fixture_dir(name).join("expected.json"))
}

/// A scratch copy of a fixture.
fn copy_of(convention: &Convention, scratch: &Scratch, name: &str) -> std::path::PathBuf {
    let corpus = scratch.join(name);
    copy_dir(&fixture_dir(convention.name), &corpus);
    corpus
}

fn records_of<'a>(records: &'a [Value], id: &str) -> Vec<&'a Value> {
    records.iter().filter(|record| record["id"] == id).collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

// ----------------------------------------------------------------- AC-03

/// AC-03: `import` on each fixture prints `result` = its `expected.json`
/// (timings aside); `import-one` is what runs without `--pilot`.
#[test]
fn import_on_each_fixture_matches_expected_json() {
    let scratch = Scratch::new("expected");
    let out = scratch.join("default");
    let envelope = envelope(&run(&["import", "--out", out.to_str().unwrap()]));
    assert_eq!(envelope["measurement"], "import");
    assert_eq!(envelope["label"], "fixtures", "the default label");
    assert!(
        out.join("import/fixtures/records.json").is_file(),
        "detail under --out/import/fixtures/"
    );
    assert_eq!(
        without_ms(&envelope["result"]),
        expected("import-one"),
        "import without --pilot runs on fixtures/import-one"
    );
    for convention in CONVENTIONS {
        let run = ImportRun::new(
            &fixture_dir(convention.name),
            &scratch,
            convention.name,
            &[],
        );
        assert_eq!(
            without_ms(&run.result),
            expected(convention.name),
            "{}: result minus *_ms differs from expected.json",
            convention.name
        );
        assert!(
            run.result["detail"]["import_ms"].is_u64(),
            "{}: detail.import_ms is the only timing",
            convention.name
        );
    }
}

/// AC-03: what `expected.json` pins in each convention, and no record sits
/// at a row counted without an ID.
#[test]
fn expected_json_pins_every_form_header_and_row_count() {
    let scratch = Scratch::new("pins");
    for convention in CONVENTIONS {
        let expected = expected(convention.name);
        let name = convention.name;
        let count = |path: &str| {
            let mut value = &expected;
            for part in path.split('/') {
                value = &value[part];
            }
            value
                .as_u64()
                .unwrap_or_else(|| panic!("{name}: {path} is not a count"))
        };
        for form in ["table_row", "headerless_row", "list_item", "section"] {
            assert!(
                count(&format!("records/per_form/{form}")) >= 2,
                "{name}: per_form.{form} >= 2"
            );
        }
        for (path, at_least) in [
            ("front_matter/field_table", 1),
            ("front_matter/keys/mapped", 1),
            ("front_matter/keys/unmapped", 1),
            ("front_matter/values/mapped", 1),
            ("rows_without_id/local_number", 2),
            ("rows_without_id/none", 1),
            // docs/features/import-gaps.md AC-09.
            ("records/titled", 1),
            ("records/per_form/document", 1),
            ("references/by_document", 1),
        ] {
            assert!(count(path) >= at_least, "{name}: {path} >= {at_least}");
        }
        let per_prefix = expected["records"]["per_prefix"]
            .as_object()
            .expect("per_prefix");
        assert!(per_prefix.len() >= 2, "{name}: two prefixes pinned");
        assert_eq!(
            per_prefix.values().filter_map(Value::as_u64).sum::<u64>(),
            count("records/total"),
            "{name}: per_prefix partitions the records"
        );

        let run = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let records = run.records();
        let rows = run.file("rows_without_id.json");
        let rows = rows.as_array().expect("rows_without_id.json is an array");
        assert_eq!(
            rows.len() as u64,
            count("rows_without_id/local_number") + count("rows_without_id/none")
        );
        for row in rows {
            assert!(
                !records
                    .iter()
                    .any(|record| record["path"] == row["path"] && record["line"] == row["line"]),
                "{name}: a record at a row without ID: {row}"
            );
        }
    }
}

/// AC-03 "two conventions": every recognizer key both configs set holds a
/// different value in each.
#[test]
fn the_two_fixtures_are_two_conventions() {
    let one: toml::Table =
        toml::from_str(&fs::read_to_string(fixture_config("import-one")).unwrap()).unwrap();
    let two: toml::Table =
        toml::from_str(&fs::read_to_string(fixture_config("import-two")).unwrap()).unwrap();
    let mut compared = 0;
    for (table, key) in [
        ("corpus", "roots"),
        ("front_matter", "class_key"),
        ("front_matter", "header_table"),
        ("front_matter", "key_map"),
        ("front_matter", "value_map"),
        ("ids", "regex"),
        ("ids", "feature_prefixes"),
        ("ids", "hyphenless"),
        ("ids", "legacy"),
        ("tables", "local_number"),
        ("lists", "separators"),
        ("definitions", "reference_paths"),
        ("definitions", "reference_headers"),
        ("links", "base"),
        ("code", "roots"),
        ("code", "extensions"),
        ("code", "strip"),
        ("documents", "id_key"),
    ] {
        let a = &one[table][key];
        let b = &two[table][key];
        assert_ne!(a, b, "{table}.{key} is the same in both fixtures");
        compared += 1;
    }
    assert_eq!(compared, 18);
}

// ----------------------------------------------------------------- AC-04

/// AC-04 per fixture: the cited ID is one definition and two references
/// (a reference table, a reference path); a feature-scoped ID defined in two
/// documents is no duplicate; the one reference without definition is
/// `unresolved`; a second definition outside the reference places is one
/// duplicate.
#[test]
fn definitions_references_and_duplicates() {
    let scratch = Scratch::new("references");
    for convention in CONVENTIONS {
        let name = convention.name;
        let run = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let records = run.records();

        let cited = records_of(&records, convention.cited);
        let definitions: Vec<_> = cited
            .iter()
            .filter(|record| record["role"] == "definition")
            .collect();
        let references: BTreeSet<&str> = cited
            .iter()
            .filter(|record| record["role"] == "reference")
            .map(|record| record["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            definitions.len(),
            1,
            "{name}: {} defined once",
            convention.cited
        );
        assert_eq!(
            references,
            BTreeSet::from([convention.reference_table, convention.reference_path]),
            "{name}: {} referenced from the reference table and the reference path",
            convention.cited
        );
        assert_eq!(cited.len(), 3, "{name}: {cited:?}");

        let feature = records_of(&records, convention.feature);
        let feature_documents: BTreeSet<&str> = feature
            .iter()
            .map(|record| record["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            feature_documents,
            BTreeSet::from(convention.feature_documents),
            "{name}: {} defined in two documents",
            convention.feature
        );
        assert!(
            feature
                .iter()
                .all(|record| record["role"] == "definition" && record["scope"] == "feature"),
            "{name}: {feature:?}"
        );

        let duplicates = run.file("duplicates.json");
        let duplicate_ids: Vec<&str> = duplicates["duplicate_definitions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            duplicate_ids,
            [convention.duplicate],
            "{name}: only the planted project duplicate"
        );
        assert_eq!(run.count("duplicate_definitions"), 1);
        let unresolved: Vec<&str> = duplicates["unresolved_references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["token"].as_str().unwrap())
            .collect();
        assert_eq!(unresolved, [convention.unresolved], "{name}");
        assert_eq!(run.count("references/unresolved"), 1);

        // Without the planted duplicate: no duplicate at all.
        let corpus = copy_of(convention, &scratch, &format!("{name}-single"));
        fs::remove_file(corpus.join(convention.duplicate_file)).unwrap();
        let config = fixture_config(name);
        let single = ImportRun::new(
            &corpus,
            &scratch,
            &format!("{name}-single-out"),
            &["--config", config.to_str().unwrap()],
        );
        assert_eq!(
            single.count("duplicate_definitions"),
            0,
            "{name}: one definition, two references, a feature ID in two documents"
        );
        assert_eq!(
            single.count("references/total"),
            run.count("references/total")
        );

        // A second definition of the cited ID outside the reference places.
        let (path, text) = convention.second_definition;
        fs::write(corpus.join(path), text).unwrap();
        let twice = ImportRun::new(
            &corpus,
            &scratch,
            &format!("{name}-twice-out"),
            &["--config", config.to_str().unwrap()],
        );
        assert_eq!(twice.count("duplicate_definitions"), 1, "{name}");
        let entry = &twice.file("duplicates.json")["duplicate_definitions"][0];
        assert_eq!(entry["id"], convention.cited, "{name}: {entry}");
        assert_eq!(entry["scope"], "project");
    }
}

// ----------------------------------------------------------------- AC-05

/// AC-05 on the test-generated copy of `import-one`: a Cyrillic legacy prefix
/// becomes the Latin one with the written ID as alias; unmapped non-Latin
/// prefixes — one of Latin look-alikes only — are counted and never guessed;
/// every hyphenless match is a definition or a mention, never a record, an
/// alias or an unclaimed token; a Cyrillic key maps like a Latin one.
#[test]
fn legacy_prefixes_and_hyphenless_codes_on_a_generated_copy() {
    let scratch = Scratch::new("generated");
    let base = ImportRun::new(&fixture_dir("import-one"), &scratch, "base", &[]);
    let (corpus, config) = generated_copy(&scratch);
    let generated = ImportRun::new(
        &corpus,
        &scratch,
        "generated-out",
        &["--config", config.to_str().unwrap()],
    );
    let delta = |path: &str| generated.count(path) as i64 - base.count(path) as i64;

    // The legacy prefix.
    let records = generated.records();
    let legacy_written = format!("{}-031", legacy_prefix());
    let mapped = records_of(&records, "REQ-031");
    assert_eq!(mapped.len(), 1, "{mapped:?}");
    assert_eq!(mapped[0]["prefix"], "REQ");
    assert_eq!(mapped[0]["path"], "spec/legacy.md");
    assert_eq!(
        strings(&mapped[0]["aliases"]),
        std::slice::from_ref(&legacy_written)
    );
    assert_eq!(mapped[0]["script"], "non-latin");
    assert_eq!(delta("legacy/mapped"), 1);
    let legacy = generated.file("legacy.json");
    assert!(
        legacy["mapped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["written"] == legacy_written.as_str() && change["id"] == "REQ-031"),
        "{legacy}"
    );

    // Unmapped prefixes: counted, no record, no guessed ID.
    assert_eq!(delta("legacy/unmapped"), 2);
    let unmapped: BTreeSet<String> = legacy["unmapped"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|token| token["path"] == "spec/legacy.md")
        .map(|token| token["token"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        unmapped,
        BTreeSet::from([
            format!("{}-032", look_alike_prefix()),
            format!("{}-033", foreign_prefix())
        ])
    );
    let in_document: Vec<&str> = records
        .iter()
        .filter(|record| record["path"] == "spec/legacy.md")
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        in_document,
        ["REQ-031"],
        "only the mapped legacy ID is a record"
    );
    for guessed in ["TEC-032", "PRB-033", "PPB-033"] {
        assert!(
            records_of(&records, guessed).is_empty(),
            "{guessed} was guessed from an unmapped prefix"
        );
    }
    assert_eq!(delta("records/total"), 1);

    // Hyphenless codes.
    assert_eq!(delta("legacy/hyphenless/definitions"), 2);
    assert_eq!(delta("legacy/hyphenless/mentions"), 3);
    let mut found: Vec<(String, String)> = legacy["hyphenless"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|found| found["path"] == "spec/legacy.md")
        .map(|found| {
            (
                found["token"].as_str().unwrap().to_owned(),
                found["role"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            ("QP201".to_owned(), "definition".to_owned()),
            ("QP201".to_owned(), "mention".to_owned()),
            ("QP202".to_owned(), "mention".to_owned()),
            ("QP203".to_owned(), "mention".to_owned()),
            ("QP204".to_owned(), "definition".to_owned()),
        ]
    );
    let unclaimed = generated.file("unclaimed.json");
    for record in &records {
        let id = record["id"].as_str().unwrap();
        assert!(
            !id.starts_with("QP"),
            "a hyphenless code became a record: {record}"
        );
        assert!(
            strings(&record["aliases"])
                .iter()
                .all(|alias| !alias.contains("QP")),
            "a hyphenless code became an alias: {record}"
        );
    }
    assert!(
        unclaimed
            .as_array()
            .unwrap()
            .iter()
            .all(|token| !token["token"].as_str().unwrap().contains("QP")),
        "a hyphenless code is an unclaimed token: {unclaimed}"
    );
    // The only new unclaimed tokens are the two unmapped legacy IDs.
    assert_eq!(delta("id_like/unclaimed"), 2, "{unclaimed}");

    // The Cyrillic key and its value.
    assert_eq!(delta("front_matter/non_latin_keys"), 1);
    assert_eq!(delta("front_matter/keys/mapped"), 1);
    assert_eq!(delta("front_matter/values/mapped"), 1);
}

/// AC-05 under an ASCII-only `ids.like`: a token of letters, `-` and digits
/// with a non-ASCII letter is a candidate whatever `like` says, so the
/// unmapped non-Latin prefix and the all-look-alike one still count
/// `legacy.unmapped` and give no record, and the legacy prefix still maps.
#[test]
fn unmapped_prefixes_are_counted_under_an_ascii_only_like() {
    let scratch = Scratch::new("ascii-like");
    let (corpus, _) = generated_copy(&scratch);
    let anchor = "feature_prefixes = [\"AC\"]";
    let text = generated_config_text();
    assert_eq!(text.matches(anchor).count(), 1, "{text}");
    let config = scratch.join("ascii-like.toml");
    fs::write(
        &config,
        text.replace(
            anchor,
            &format!("like = '[A-Z]{{2,3}}-[0-9]{{3}}'\n{anchor}"),
        ),
    )
    .unwrap();
    let run = ImportRun::new(
        &corpus,
        &scratch,
        "ascii-like-out",
        &["--config", config.to_str().unwrap()],
    );
    let legacy = run.file("legacy.json");
    let unmapped: BTreeSet<String> = legacy["unmapped"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|token| token["path"] == "spec/legacy.md")
        .map(|token| token["token"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        unmapped,
        BTreeSet::from([
            format!("{}-032", look_alike_prefix()),
            format!("{}-033", foreign_prefix())
        ]),
        "{legacy}"
    );
    // The fixture's own prefix without an ASCII letter (requirements.md) and
    // legacy IDs (a table row, a titled lead-in) too.
    let fixture = expected("import-one");
    let fixture_count = |key: &str| fixture["legacy"][key].as_u64().unwrap();
    assert_eq!(fixture_count("unmapped"), 1);
    assert_eq!(
        run.count("legacy/unmapped"),
        fixture_count("unmapped") + 2,
        "{legacy}"
    );
    assert_eq!(
        run.count("legacy/mapped"),
        fixture_count("mapped") + 1,
        "{legacy}"
    );
    let records = run.records();
    let in_document: Vec<&str> = records
        .iter()
        .filter(|record| record["path"] == "spec/legacy.md")
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert_eq!(in_document, ["REQ-031"], "only the mapped legacy ID");
    assert_eq!(
        strings(&records_of(&records, "REQ-031")[0]["aliases"]),
        [format!("{}-031", legacy_prefix())]
    );
}

// ----------------------------------------------------------------- AC-06

fn lines_of(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}

/// AC-06: a planted ID-like token defined nowhere is one more unclaimed
/// token and one more `unclaimed.json` entry, at its line.
#[test]
fn a_planted_token_is_one_more_unclaimed_entry() {
    let scratch = Scratch::new("planted");
    for convention in CONVENTIONS {
        let name = convention.name;
        let base = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let corpus = copy_of(convention, &scratch, &format!("{name}-copy"));
        let (path, token) = convention.planted;
        let file = corpus.join(path);
        let mut text = fs::read_to_string(&file).unwrap();
        text.push_str(&format!("\nPlanted {token} here.\n"));
        fs::write(&file, text).unwrap();
        let line = lines_of(&file);
        let config = fixture_config(name);
        let planted = ImportRun::new(
            &corpus,
            &scratch,
            &format!("{name}-out"),
            &["--config", config.to_str().unwrap()],
        );
        assert_eq!(
            planted.count("id_like/unclaimed"),
            base.count("id_like/unclaimed") + 1,
            "{name}"
        );
        let before = base.file("unclaimed.json");
        let after = planted.file("unclaimed.json");
        let before = before.as_array().unwrap();
        let after = after.as_array().unwrap();
        assert_eq!(after.len(), before.len() + 1, "{name}");
        let new: Vec<&Value> = after
            .iter()
            .filter(|entry| !before.contains(entry))
            .collect();
        assert_eq!(new.len(), 1, "{name}: {new:?}");
        assert_eq!(new[0]["path"], path);
        assert_eq!(new[0]["line"], line as u64);
        assert_eq!(new[0]["token"], token);
        assert_eq!(new[0]["cause"], "unclaimed", "{name}");
        assert_eq!(
            planted.count("id_like/feature_outside"),
            base.count("id_like/feature_outside"),
            "{name}"
        );
    }
}

/// AC-06: a feature-scoped ID cited from a document that does not define it
/// (defined in another) is `feature_outside`, cause `feature-outside` in
/// `unclaimed.json`, and neither claimed nor unclaimed; a document holding
/// only such a token is no file with unclaimed tokens.
#[test]
fn a_feature_scoped_id_cited_outside_its_documents_is_feature_outside() {
    let scratch = Scratch::new("feature-outside");
    for convention in CONVENTIONS {
        let name = convention.name;
        let run = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let (path, line, token) = convention.feature_outside;
        let unclaimed = run.file("unclaimed.json");
        let outside: Vec<(&str, u64, &str)> = unclaimed
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["cause"] == "feature-outside")
            .map(|entry| {
                (
                    entry["path"].as_str().unwrap(),
                    entry["line"].as_u64().unwrap(),
                    entry["token"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(outside, [(path, line, token)], "{name}");
        let causes: BTreeSet<&str> = unclaimed
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["cause"].as_str().unwrap())
            .collect();
        assert_eq!(
            causes,
            BTreeSet::from(["feature-outside", "unclaimed"]),
            "{name}: the two causes"
        );
        assert_eq!(run.count("id_like/feature_outside"), 1, "{name}");
        assert_eq!(
            unclaimed.as_array().unwrap().len() as u64,
            run.count("id_like/unclaimed") + run.count("id_like/feature_outside"),
            "{name}: one unclaimed.json entry per unclaimed or feature-outside token"
        );
        assert!(
            records_of(&run.records(), token)
                .iter()
                .all(|record| record["path"] != path && record["scope"] == "feature"),
            "{name}: {token} is a feature-scoped ID defined elsewhere only"
        );
        assert!(!records_of(&run.records(), token).is_empty(), "{name}");
    }
}

/// The fixture config without its `[lists]` table.
fn config_without_lists(name: &str) -> String {
    let text = fs::read_to_string(fixture_config(name)).unwrap();
    let mut out = String::new();
    let mut in_lists = false;
    for line in text.lines() {
        if line.starts_with('[') {
            in_lists = line.trim() == "[lists]";
        }
        if !in_lists {
            out.push_str(line);
            out.push('\n');
        }
    }
    assert_ne!(out, text, "{name}: the config had a [lists] table");
    out
}

/// AC-06: without `[lists]` no list item is a record, and their IDs fall
/// into the unclaimed counter: it grows by at least the former `list_item`,
/// in `import-one` (lead-ins `**ID**`) and in `import-two` (lead-ins
/// `__ID__`: emphasis delimiter runs read as blanks, so the `_` bounds the ID).
#[test]
fn without_lists_the_list_items_become_unclaimed_tokens() {
    let scratch = Scratch::new("no-lists");
    for convention in CONVENTIONS {
        let name = convention.name;
        let base = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let config = scratch.join(&format!("{name}-no-lists.toml"));
        fs::write(&config, config_without_lists(name)).unwrap();
        let without = ImportRun::new(
            &fixture_dir(name),
            &scratch,
            &format!("{name}-out"),
            &["--config", config.to_str().unwrap()],
        );
        let former = base.count("records/per_form/list_item");
        assert!(former >= 2, "{name}");
        assert_eq!(without.count("records/per_form/list_item"), 0, "{name}");
        assert_eq!(
            without.count("records/total"),
            base.count("records/total") - former,
            "{name}: only the list items are gone"
        );
        assert!(
            without.count("id_like/unclaimed") >= base.count("id_like/unclaimed") + former,
            "{name}: unclaimed {} -> {}, former list_item {former}",
            base.count("id_like/unclaimed"),
            without.count("id_like/unclaimed")
        );
    }
}

// ----------------------------------------------------------------- AC-08

/// AC-08: `parse`, `import` and `census` walk the same documents of each
/// fixture (which holds an excluded file, an unlisted extension and a
/// dot-directory); none of those is a document.
#[test]
fn parse_import_and_census_count_the_same_documents() {
    let scratch = Scratch::new("walk");
    for convention in CONVENTIONS {
        let name = convention.name;
        let corpus = fixture_dir(name);
        for path in convention.not_documents {
            assert!(corpus.join(path).is_file(), "{name}: {path} exists");
        }
        let scheme = scratch.join(&format!("{name}-scheme.toml"));
        fs::write(&scheme, convention.scheme).unwrap();
        let parse = measure(
            "parse",
            &corpus,
            &scratch.join(&format!("{name}-parse")),
            &["--scheme", scheme.to_str().unwrap()],
        );
        let import = ImportRun::new(&corpus, &scratch, &format!("{name}-import"), &[]);
        let census = measure(
            "census",
            &corpus,
            &scratch.join(&format!("{name}-census")),
            &[],
        );
        let parse_files = parse["result"]["files"].as_u64().unwrap();
        let import_total = import.count("documents/total");
        let census_documents = census["result"]["documents"].as_u64().unwrap();
        assert_eq!(
            (parse_files, import_total),
            (census_documents, census_documents),
            "{name}: parse.files, import documents.total, census documents"
        );
        let documents: BTreeSet<String> = import
            .file("documents.json")
            .as_array()
            .unwrap()
            .iter()
            .map(|document| document["path"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(documents.len() as u64, import_total);
        for path in convention.not_documents {
            assert!(!documents.contains(path), "{name}: {path} is a document");
        }
    }
}

/// AC-08: eval `parse` takes its files from the one walk of
/// `specengine-import`, never from its own directory listing.
#[test]
fn eval_parse_has_no_directory_walk_of_its_own() {
    let source = fs::read_to_string(repository_root().join("crates/specengine-eval/src/parse.rs"))
        .expect("parse.rs readable");
    assert!(
        !source.contains("read_dir"),
        "crates/specengine-eval/src/parse.rs walks a directory itself"
    );
    assert!(
        source.contains("walk::documents"),
        "parse.rs uses the import walk"
    );
}

// ----------------------------------------------------------------- AC-09

fn assert_refused(output: &std::process::Output, out: &Path, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: expected exit 2, stderr:\n{}",
        stderr(output)
    );
    assert!(output.stdout.is_empty(), "{context}: stdout must be empty");
    assert!(!out.exists(), "{context}: {} was created", out.display());
}

/// AC-09: `--out` inside the corpus is refused before anything is written.
#[test]
fn out_inside_the_corpus_is_refused_and_nothing_is_created() {
    let scratch = Scratch::new("guard");
    for convention in CONVENTIONS {
        let corpus = copy_of(convention, &scratch, convention.name);
        let before = snapshot(&corpus);
        for (out, context) in [
            (corpus.join("out"), "directly under the corpus"),
            (corpus.join("spec").join("out"), "nested under the corpus"),
            (
                corpus.join("absent").join("..").join("out"),
                "under the corpus through ..",
            ),
        ] {
            let output = run(&[
                "import",
                "--pilot",
                corpus.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
            ]);
            assert_refused(&output, &out, &format!("{}: {context}", convention.name));
        }
        let output = run(&[
            "import",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            corpus.to_str().unwrap(),
        ]);
        assert_eq!(output.status.code(), Some(2), "--out equal to --pilot");
        assert!(output.stdout.is_empty());
        assert_eq!(
            snapshot(&corpus),
            before,
            "{}: the corpus is byte-identical",
            convention.name
        );
    }
}

/// The stdout `result` keys of the "before" report: the whitelist of
/// docs/features/import-records.md AC-09 plus `records.titled`,
/// `records.per_form.document` and `references.by_document`
/// (docs/features/import-gaps.md AC-09); `*` is a map of anonymous labels.
const WHITELIST: [&str; 50] = [
    "documents/total",
    "documents/per_class/*",
    "front_matter/yaml",
    "front_matter/field_table",
    "front_matter/none",
    "front_matter/unclosed",
    "front_matter/non_latin_keys",
    "front_matter/keys/mapped",
    "front_matter/keys/kept",
    "front_matter/keys/unmapped",
    "front_matter/values/mapped",
    "front_matter/values/kept",
    "front_matter/values/unmapped",
    "records/total",
    "records/empty_text",
    "records/titled",
    "records/per_form/table_row",
    "records/per_form/headerless_row",
    "records/per_form/list_item",
    "records/per_form/section",
    "records/per_form/document",
    "records/per_prefix/*",
    "definitions",
    "references/total",
    "references/unresolved",
    "references/by_document",
    "duplicate_definitions",
    "rows_without_id/local_number",
    "rows_without_id/none",
    "legacy/mapped",
    "legacy/unmapped",
    "legacy/homoglyph_fixes",
    "legacy/hyphenless/definitions",
    "legacy/hyphenless/mentions",
    "legacy/hyphenless/per_pattern/*",
    "id_like/claimed",
    "id_like/unclaimed",
    "id_like/feature_outside",
    "id_like/files_with_unclaimed",
    "broken_links/file",
    "broken_links/wiki",
    "broken_links/resolved_by_base",
    "code/files",
    "code/documents_cited",
    "code/citations",
    "code/roots_missing",
    "detail/files_skipped",
    "detail/roots_missing",
    "detail/diagnostics",
    "detail/import_ms",
];

/// Every leaf of `value` as a `/`-path, with label maps folded into `*`.
fn leaves(value: &Value, prefix: &str, out: &mut BTreeSet<String>, labels: &mut Vec<String>) {
    let Value::Object(map) = value else {
        assert!(value.is_u64(), "{prefix} is a count: {value}");
        out.insert(prefix.to_owned());
        return;
    };
    let label_map = ["per_class", "per_prefix", "per_pattern"]
        .iter()
        .any(|name| prefix.ends_with(name));
    if label_map {
        // A label map is one whitelisted key even when empty (a corpus
        // without, say, a hyphenless pattern prints `"per_pattern":{}`).
        out.insert(format!("{prefix}/*"));
    }
    for (key, child) in map {
        if label_map {
            labels.push(format!("{prefix}/{key}"));
            assert!(child.is_u64(), "{prefix}/{key} is a count: {child}");
        } else {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}/{key}")
            };
            leaves(child, &path, out, labels);
        }
    }
}

fn is_label(path: &str) -> bool {
    let (map, label) = path.rsplit_once('/').unwrap();
    let numbered = |stem: &str| {
        label
            .strip_prefix(stem)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|n| {
                !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit())
            })
    };
    if map.ends_with("per_class") {
        numbered("class") || label == "unclassified"
    } else if map.ends_with("per_prefix") {
        numbered("prefix")
    } else {
        numbered("pattern")
    }
}

/// Every string of the detail files that names something in the corpus.
fn corpus_strings(run: &ImportRun) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    fn collect(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::String(text) => {
                if text.chars().filter(|c| c.is_alphabetic()).count() >= 2 && text.len() >= 3 {
                    out.insert(text.clone());
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect(item, out)),
            Value::Object(map) => map.values().for_each(|item| collect(item, out)),
            _ => {}
        }
    }
    for file in [
        "records.json",
        "unclaimed.json",
        "legacy.json",
        "documents.json",
    ] {
        collect(&run.file(file), &mut found);
    }
    // labels.json: the values behind the labels, not the labels.
    let labels = run.file("labels.json");
    for group in ["classes", "prefixes", "patterns"] {
        for row in labels[group].as_array().unwrap() {
            found.insert(row["value"].as_str().unwrap().to_owned());
        }
    }
    // Enum words of the detail files that are also stdout keys (or part of
    // one) are not corpus strings.
    for word in [
        "definition",
        "reference",
        "mention",
        "latin",
        "section",
        "yaml",
        "none",
        "mapped",
        "kept",
        "unmapped",
    ] {
        found.remove(word);
    }
    found
}

/// AC-09: stdout carries exactly the whitelisted keys, counts and anonymous
/// labels only, and no string of the corpus.
#[test]
fn stdout_keys_are_the_whitelist_and_no_corpus_string_leaks() {
    let scratch = Scratch::new("whitelist");
    for convention in CONVENTIONS {
        let name = convention.name;
        let out = scratch.join(name);
        let output = run(&[
            "import",
            "--pilot",
            fixture_dir(name).to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        let envelope = envelope(&output);
        let top: BTreeSet<&str> = envelope
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            top,
            BTreeSet::from(["label", "measurement", "result", "versions", "wall_ms"]),
            "{name}: envelope keys"
        );
        let mut found = BTreeSet::new();
        let mut labels = Vec::new();
        leaves(&envelope["result"], "", &mut found, &mut labels);
        let whitelist: BTreeSet<String> = WHITELIST.iter().map(|path| (*path).to_owned()).collect();
        assert_eq!(found, whitelist, "{name}: stdout result keys");
        for label in &labels {
            assert!(is_label(label), "{name}: {label} is not an anonymous label");
        }
        assert!(labels.len() >= 5, "{name}: {labels:?}");

        // The whitelisted key names are the format, not corpus text: a
        // detail-file enum word (`feature`, `unclaimed`) inside one is no leak.
        let mut stdout = String::from_utf8(output.stdout.clone()).unwrap();
        for key in WHITELIST
            .iter()
            .flat_map(|path| path.split('/'))
            .filter(|segment| *segment != "*")
        {
            stdout = stdout.replace(&format!("\"{key}\":"), "\"\":");
        }
        let run = ImportRun {
            result: envelope["result"].clone(),
            detail: out.join("import").join("pilot"),
        };
        let leaked: Vec<String> = corpus_strings(&run)
            .into_iter()
            .filter(|text| stdout.contains(text.as_str()))
            .collect();
        assert!(
            leaked.is_empty(),
            "{name}: corpus strings on stdout: {leaked:?}"
        );
    }
}

/// AC-09: the code scan counts each bounded occurrence of a document path or
/// its stripped form once, in listed files of the code roots only; an
/// unbounded `x`-prefixed form is not one.
#[test]
fn code_scan_counts_bounded_citations_only() {
    let scratch = Scratch::new("code");
    for convention in CONVENTIONS {
        let name = convention.name;
        let run = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let citations: Vec<(String, String, u64)> = run
            .file("code_citations.json")
            .as_array()
            .unwrap()
            .iter()
            .map(|citation| {
                (
                    citation["document"].as_str().unwrap().to_owned(),
                    citation["file"].as_str().unwrap().to_owned(),
                    citation["line"].as_u64().unwrap(),
                )
            })
            .collect();
        let want: Vec<(String, String, u64)> = convention
            .citations
            .iter()
            .map(|(document, file, line)| ((*document).to_owned(), (*file).to_owned(), *line))
            .collect();
        assert_eq!(citations, want, "{name}");
        assert_eq!(run.count("code/documents_cited"), 2, "{name}");
        assert_eq!(run.count("code/citations"), 3, "{name}");
        for file in convention.not_cited {
            assert!(
                fixture_dir(name).join(file).is_file(),
                "{name}: {file} exists"
            );
            assert!(
                citations.iter().all(|(_, cited_in, _)| cited_in != file),
                "{name}: {file} was scanned"
            );
        }
        // The unbounded form names a real document's stripped form.
        let unbounded = fs::read_to_string(fixture_dir(name).join(convention.citations[0].1))
            .unwrap()
            .lines()
            .find_map(|line| {
                line.split('"')
                    .nth(1)
                    .filter(|literal| literal.starts_with("xa."))
                    .map(str::to_owned)
            });
        let unbounded = unbounded.expect("an x-prefixed literal in the code file");
        let document = &unbounded[1..];
        assert!(
            run.file("documents.json")
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["path"]
                    .as_str()
                    .unwrap()
                    .ends_with(&format!("/{document}"))),
            "{name}: {document} is a document"
        );
        assert!(
            citations
                .iter()
                .all(|(cited, _, _)| !cited.ends_with(&format!("/{document}"))),
            "{name}: {unbounded} was counted"
        );
    }
}

/// AC-09: below a code root, `target*` build directories (`target`,
/// `target-x`) and dot-directories (`.cache`) are not entered: a file there
/// citing a document adds no code file and no citation.
#[test]
fn build_and_dot_directories_under_a_code_root_are_not_scanned() {
    let scratch = Scratch::new("code-skipped");
    for convention in CONVENTIONS {
        let name = convention.name;
        let base = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let corpus = copy_of(convention, &scratch, &format!("{name}-copy"));
        let (document, code_file, _) = convention.citations[0];
        let (root, extension) = code_file
            .split_once('/')
            .map(|(root, file)| (root, file.rsplit_once('.').unwrap().1))
            .unwrap();
        let citing = format!("// see \"{document}\"\n");
        for directory in ["target", "target-x", ".cache"] {
            let dir = corpus.join(root).join(directory);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(format!("cite.{extension}")), &citing).unwrap();
        }
        // The same file under a plain directory is scanned: the control
        // that the written citation is one.
        let control = scratch.join(&format!("{name}-control"));
        copy_dir(&corpus, &control);
        fs::create_dir_all(control.join(root).join("plain")).unwrap();
        fs::write(
            control.join(root).join(format!("plain/cite.{extension}")),
            &citing,
        )
        .unwrap();
        let config = fixture_config(name);
        let skipped = ImportRun::new(
            &corpus,
            &scratch,
            &format!("{name}-out"),
            &["--config", config.to_str().unwrap()],
        );
        let scanned = ImportRun::new(
            &control,
            &scratch,
            &format!("{name}-control-out"),
            &["--config", config.to_str().unwrap()],
        );
        for count in ["code/files", "code/documents_cited", "code/citations"] {
            assert_eq!(skipped.count(count), base.count(count), "{name}: {count}");
        }
        assert_eq!(
            scanned.count("code/files"),
            base.count("code/files") + 1,
            "{name}"
        );
        assert_eq!(
            scanned.count("code/citations"),
            base.count("code/citations") + 1,
            "{name}"
        );
        let cited_in: Vec<String> = skipped
            .file("code_citations.json")
            .as_array()
            .unwrap()
            .iter()
            .map(|citation| citation["file"].as_str().unwrap().to_owned())
            .collect();
        assert!(
            cited_in
                .iter()
                .all(|file| !file.contains("target") && !file.contains(".cache")),
            "{name}: {cited_in:?}"
        );
    }
}

/// AC-09 and determinism: runs leave the fixtures byte-identical and
/// `git status -- fixtures/` shows nothing but the import fixtures; a second
/// run gives the same result and byte-identical detail files.
#[test]
fn runs_are_deterministic_and_leave_the_fixtures_untouched() {
    let scratch = Scratch::new("determinism");
    assert_fixtures_status_clean();
    for convention in CONVENTIONS {
        let name = convention.name;
        let fixture = fixture_dir(name);
        let before = snapshot(&fixture);
        let first = ImportRun::new(&fixture, &scratch, &format!("{name}-1"), &[]);
        let second = ImportRun::new(&fixture, &scratch, &format!("{name}-2"), &[]);
        assert_eq!(
            without_ms(&first.result),
            without_ms(&second.result),
            "{name}"
        );
        let first_files = snapshot(&first.detail);
        assert_eq!(first_files.len(), 10, "{name}: the ten detail files");
        assert_eq!(
            first_files,
            snapshot(&second.detail),
            "{name}: detail files"
        );
        assert_eq!(
            snapshot(&fixture),
            before,
            "{name}: the fixture is untouched"
        );
    }
    assert_fixtures_status_clean();
}

// ----------------------------------------------------------------- AC-02

/// A config with every key of `crates/specengine-import/README.md` "Config",
/// over import-one.
const EVERY_KEY: &str = r#"[corpus]
roots = ["spec"]
[front_matter]
class_key = "kind"
header_table = '^Field$'
header_row_field = true
[front_matter.key_map]
"Phase" = "status"
[front_matter.value_map.status]
"Done" = "shipped"
[ids]
regex = '^[A-Z]{2,3}-[0-9]{3}$'
like = '[A-Z]{2,3}-[0-9]+'
feature_prefixes = ["AC"]
hyphenless = ['\b[A-Z]{2}[0-9]{3}\b']
[ids.legacy]
"SR" = "REQ"
[tables]
text_column = 1
text_header = '^Statement$'
local_number = '^[0-9]+$'
[lists]
lead_in = true
separators = [":"]
[definitions]
reference_paths = ["spec/map.md"]
reference_headers = ['^Source$']
[links]
base = "spec"
[code]
roots = ["src"]
extensions = ["rs"]
exclude = ["src/generated/**"]
strip = ["spec/"]
[documents]
id_key = "ident"
id_path = '^spec/(?P<id>[A-Z]{2,3}-[0-9]{3})\.md$'
"#;

/// AC-02: `census` and `import` accept every new key.
#[test]
fn census_and_import_accept_every_new_key() {
    let scratch = Scratch::new("every-key");
    let config = scratch.join("every.toml");
    fs::write(&config, EVERY_KEY).unwrap();
    let corpus = fixture_dir("import-one");
    for measurement in ["census", "import"] {
        let envelope = measure(
            measurement,
            &corpus,
            &scratch.join(measurement),
            &["--config", config.to_str().unwrap()],
        );
        assert_eq!(envelope["measurement"], measurement);
    }
    for convention in CONVENTIONS {
        measure(
            "census",
            &fixture_dir(convention.name),
            &scratch.join(&format!("{}-census", convention.name)),
            &[],
        );
    }
}

/// The census part every bad config starts from (4 lines).
const CENSUS_BASE: &str = "[corpus]\nroots = [\"spec\"]\n[ids]\nregex = '^[A-Z]{2,3}-[0-9]{3}$'\n";

/// Bad import keys: (case, text appended to [`CENSUS_BASE`], 1-based line of
/// the error within the appended text).
fn bad_configs() -> Vec<(&'static str, String, usize)> {
    let cyrillic_target = "[ids.legacy]\n\"SR\" = \"R\u{0415}Q\"\n".to_owned();
    vec![
        (
            "unknown key in [lists]",
            "[lists]\nlead_in = true\ncolour = \"blue\"\n".into(),
            3,
        ),
        (
            "unknown key in [definitions]",
            "[definitions]\nreference_paths = [\"spec/map.md\"]\nreference_tables = ['^S$']\n"
                .into(),
            3,
        ),
        (
            "unknown key in [code]",
            "[code]\nroots = [\"src\"]\nroot = \"src\"\n".into(),
            3,
        ),
        (
            "unknown key in [front_matter]",
            "[front_matter]\nheader_tables = '^F$'\n".into(),
            2,
        ),
        (
            "unknown key in [ids]",
            "feature_prefix = [\"AC\"]\n".into(),
            1,
        ),
        (
            "unknown key in [tables]",
            "[tables]\ntext_col = 1\n".into(),
            2,
        ),
        (
            "unknown key in [links]",
            "[links]\nbases = \"spec\"\n".into(),
            2,
        ),
        (
            "wrong type: lists.lead_in",
            "[lists]\nlead_in = \"yes\"\n".into(),
            2,
        ),
        (
            "wrong type: lists.separators",
            "[lists]\nseparators = \":\"\n".into(),
            2,
        ),
        (
            "wrong type: tables.text_column",
            "[tables]\ntext_column = \"1\"\n".into(),
            2,
        ),
        (
            "wrong type: header_row_field",
            "[front_matter]\nheader_row_field = 1\n".into(),
            2,
        ),
        (
            "wrong type: feature_prefixes",
            "feature_prefixes = \"AC\"\n".into(),
            1,
        ),
        (
            "wrong type: code.roots",
            "[code]\nroots = \"src\"\n".into(),
            2,
        ),
        (
            "wrong type: reference_paths",
            "[definitions]\nreference_paths = 1\n".into(),
            2,
        ),
        (
            "wrong type: key_map target",
            "[front_matter.key_map]\n\"Phase\" = 1\n".into(),
            2,
        ),
        (
            "bad regex: header_table",
            "[front_matter]\nheader_table = '^[F'\n".into(),
            2,
        ),
        ("bad regex: ids.like", "like = '('\n".into(), 1),
        (
            "bad regex: ids.hyphenless",
            "hyphenless = ['[A-']\n".into(),
            1,
        ),
        (
            "bad regex: tables.text_header",
            "[tables]\ntext_header = '^(S'\n".into(),
            2,
        ),
        (
            "bad regex: tables.local_number",
            "[tables]\nlocal_number = '[0-'\n".into(),
            2,
        ),
        (
            "bad regex: reference_headers",
            "[definitions]\nreference_headers = ['^(S']\n".into(),
            2,
        ),
        ("code root ..", "[code]\nroots = [\"..\"]\n".into(), 2),
        ("code root .", "[code]\nroots = [\".\"]\n".into(), 2),
        ("code root ./", "[code]\nroots = [\"./\"]\n".into(), 2),
        (
            "code root a/.. after a valid one",
            "[code]\nroots = [\"src\",\n  \"a/..\"]\n".into(),
            3,
        ),
        (
            "empty regex: header_table",
            "[front_matter]\nheader_table = ''\n".into(),
            2,
        ),
        (
            "empty-matching regex: header_table",
            "[front_matter]\nheader_table = '^(Field)?$'\n".into(),
            2,
        ),
        ("empty regex: ids.like", "like = ''\n".into(), 1),
        (
            "empty regex: ids.hyphenless entry",
            "hyphenless = ['\\b[A-Z]{2}[0-9]{3}\\b',\n  '']\n".into(),
            2,
        ),
        (
            "empty regex: tables.text_header",
            "[tables]\ntext_header = ''\n".into(),
            2,
        ),
        (
            "empty-matching regex: tables.text_header",
            "[tables]\ntext_header = 'S*'\n".into(),
            2,
        ),
        (
            "empty regex: tables.local_number",
            "[tables]\nlocal_number = ''\n".into(),
            2,
        ),
        (
            "empty regex: reference_headers entry",
            "[definitions]\nreference_headers = ['']\n".into(),
            2,
        ),
        (
            "empty-matching regex: reference_headers entry",
            "[definitions]\nreference_headers = ['^Source$',\n  '|Cited']\n".into(),
            3,
        ),
        (
            "code root escaping",
            "[code]\nroots = [\"src\", \"../outside\"]\n".into(),
            2,
        ),
        (
            "code root absolute",
            "[code]\nroots = [\"/abs\"]\n".into(),
            2,
        ),
        (
            "links.base escaping",
            "[links]\nbase = \"../outside\"\n".into(),
            2,
        ),
        (
            "links.base absolute",
            "[links]\nbase = \"/abs\"\n".into(),
            2,
        ),
        (
            "legacy target lower case",
            "[ids.legacy]\n\"SR\" = \"req\"\n".into(),
            2,
        ),
        (
            "legacy target with a hyphen",
            "[ids.legacy]\n\"SR\" = \"R-Q\"\n".into(),
            2,
        ),
        (
            "legacy target opening with a digit",
            "[ids.legacy]\n\"SR\" = \"1RQ\"\n".into(),
            2,
        ),
        (
            "legacy target empty",
            "[ids.legacy]\n\"SR\" = \"\"\n".into(),
            2,
        ),
        ("legacy target not Latin", cyrillic_target, 2),
    ]
}

/// AC-02: a bad import key refuses `census` and `import` alike: exit 2 at
/// `<config>:<line>`, nothing on stdout, nothing under `--out`.
#[test]
fn bad_import_keys_are_refused_at_their_line_by_census_and_import() {
    let scratch = Scratch::new("bad-keys");
    let corpus = fixture_dir("import-one");
    let base_lines = CENSUS_BASE.lines().count();
    // The base alone is valid.
    let valid = scratch.join("valid.toml");
    fs::write(&valid, CENSUS_BASE).unwrap();
    measure(
        "import",
        &corpus,
        &scratch.join("valid-out"),
        &["--config", valid.to_str().unwrap()],
    );
    let mut failures = Vec::new();
    for (index, (case, appended, line)) in bad_configs().into_iter().enumerate() {
        let config = scratch.join(&format!("bad-{index}.toml"));
        fs::write(&config, format!("{CENSUS_BASE}{appended}")).unwrap();
        let location = format!("{}:{}:", config.display(), base_lines + line);
        for measurement in ["census", "import"] {
            let out = scratch.join(&format!("out-{index}-{measurement}"));
            let output = run(&[
                measurement,
                "--pilot",
                corpus.to_str().unwrap(),
                "--config",
                config.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
            ]);
            let stderr = stderr(&output);
            if output.status.code() != Some(2)
                || !output.stdout.is_empty()
                || out.exists()
                || !stderr.contains(&location)
            {
                failures.push(format!(
                    "{measurement} / {case}: exit {:?}, out created {}, stderr: {}",
                    output.status.code(),
                    out.exists(),
                    stderr.trim()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "expected exit 2 at <config>:<line>:\n{}",
        failures.join("\n")
    );
}

/// AC-02: `census` on `fixtures/corpus-mini` still prints its `expected.json`.
#[test]
fn census_on_corpus_mini_still_matches_expected_json() {
    let scratch = Scratch::new("corpus-mini");
    let out = scratch.join("out");
    let envelope = envelope(&run(&["census", "--out", out.to_str().unwrap()]));
    let expected = read_json(&repository_root().join("fixtures/corpus-mini/expected.json"));
    let expected = expected.as_object().unwrap();
    for (key, want) in expected {
        assert_eq!(&envelope["result"][key.as_str()], want, "result.{key}");
    }
}

/// Import keys of the config: whole tables and keys of shared tables.
const IMPORT_TABLES: [&str; 4] = ["[lists]", "[definitions]", "[code]", "[documents]"];
const IMPORT_TABLE_PREFIXES: [&str; 3] = [
    "[front_matter.key_map]",
    "[front_matter.value_map",
    "[ids.legacy]",
];
const IMPORT_KEYS: [&str; 9] = [
    "header_table",
    "header_row_field",
    "like",
    "feature_prefixes",
    "hyphenless",
    "text_column",
    "text_header",
    "local_number",
    "base",
];

/// The census part of a fixture config: every import key and table removed.
fn census_only(name: &str) -> String {
    let text = fs::read_to_string(fixture_config(name)).unwrap();
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = IMPORT_TABLES.contains(&trimmed)
                || IMPORT_TABLE_PREFIXES
                    .iter()
                    .any(|prefix| trimmed.starts_with(prefix));
        }
        let key = trimmed.split('=').next().unwrap_or("").trim();
        if skipping || IMPORT_KEYS.contains(&key) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    // What is left is census schema only.
    let table: toml::Table = toml::from_str(&out).unwrap();
    let census_keys: [(&str, &[&str]); 6] = [
        ("corpus", &["roots", "extensions", "exclude"]),
        ("front_matter", &["class_key"]),
        ("ids", &["regex"]),
        ("tables", &["id_column", "id_header", "headerless"]),
        ("sections", &["id_attr"]),
        ("links", &["wiki", "wiki_root"]),
    ];
    for (section, value) in &table {
        let allowed = census_keys
            .iter()
            .find(|(name, _)| name == section)
            .unwrap_or_else(|| panic!("{name}: [{section}] left in the census part"))
            .1;
        for key in value.as_table().unwrap().keys() {
            assert!(
                allowed.contains(&key.as_str()),
                "{name}: {section}.{key} left"
            );
        }
    }
    out
}

/// AC-02: with import keys `census` prints what it prints without them —
/// the same `result` and byte-identical detail files.
#[test]
fn census_output_is_unchanged_by_the_import_keys() {
    let scratch = Scratch::new("census-same");
    for convention in CONVENTIONS {
        let name = convention.name;
        let corpus = fixture_dir(name);
        let with_out = scratch.join(&format!("{name}-with"));
        let with = measure("census", &corpus, &with_out, &[]);
        let config = scratch.join(&format!("{name}-census-only.toml"));
        fs::write(&config, census_only(name)).unwrap();
        let without_out = scratch.join(&format!("{name}-without"));
        let without = measure(
            "census",
            &corpus,
            &without_out,
            &["--config", config.to_str().unwrap()],
        );
        assert_eq!(
            without_ms(&with["result"]),
            without_ms(&without["result"]),
            "{name}: census result"
        );
        assert_eq!(
            snapshot(&with_out.join("census/pilot")),
            snapshot(&without_out.join("census/pilot")),
            "{name}: census detail files"
        );
    }
}

// ---------------------------------------------------------------- AC-10

/// AC-10: the pilot runs of `import` and the shared `tests/pilot` helper on
/// an invented setup. The `#[ignore]` pilot tests are owner-run: the corpus
/// comes from `SPECENGINE_PILOT_A` / `_B`, the census config from
/// `SPECENGINE_CENSUS_CONFIG_A` / `_B`, both outside the repository and
/// read-only; no scheme. They fail, never skip, on a missing variable.
#[cfg(unix)]
mod pilots {
    use super::*;

    /// One `import --label <label>` run through
    /// `pilot::run_census_read_only` (the proof over the census config's
    /// corpus and code roots equal, the child with only `PATH`, the empty
    /// scratch `HOME` and the corpus and census-config variables, exit 0,
    /// `HOME` empty), then the anonymous envelope: the five envelope keys,
    /// `result` an object (finished within `--timeout`) whose leaves are the
    /// whitelist with anonymous labels, no path, document name or label
    /// value on stdout, the detail only under `--out/import/<label>`.
    fn import_run(label: &str, scratch: &Path, corpus: &Path, config: &Path) -> Value {
        let output = pilot::run_census_read_only("import", label, scratch, corpus, config);
        let envelope = envelope(&output);
        let object = envelope.as_object().expect("envelope object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["label", "measurement", "result", "versions", "wall_ms"]),
            "envelope keys"
        );
        assert_eq!(envelope["measurement"], "import");
        assert_eq!(envelope["label"], label);
        assert!(envelope["wall_ms"].is_u64());
        let result = &envelope["result"];
        assert!(
            result.is_object(),
            "the run must finish within --timeout: {result}"
        );
        let mut found = BTreeSet::new();
        let mut labels = Vec::new();
        leaves(result, "", &mut found, &mut labels);
        let whitelist: BTreeSet<String> = WHITELIST.iter().map(|path| (*path).to_owned()).collect();
        assert_eq!(found, whitelist, "{label}: stdout result keys");
        for path in &labels {
            assert!(is_label(path), "{label}: {path} is not an anonymous label");
        }
        assert!(
            result["documents"]["total"]
                .as_u64()
                .is_some_and(|total| total > 0),
            "{label}: the corpus roots hold documents: {result}"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        for leak in [".md", "/", "\\"] {
            assert!(!stdout.contains(leak), "{label}: {leak:?} leaked to stdout");
        }
        assert!(
            !stdout.contains(corpus.to_str().unwrap()),
            "{label}: the corpus path leaked to stdout"
        );
        let name = corpus.file_name().unwrap().to_string_lossy();
        assert!(
            !stdout.contains(name.as_ref()),
            "{label}: the corpus name leaked to stdout"
        );
        let out = scratch.join("out");
        let detail = out.join("import").join(label);
        let label_map = read_json(&detail.join("labels.json"));
        for group in ["classes", "prefixes", "patterns"] {
            for row in label_map[group].as_array().expect("a label group") {
                let value = row["value"].as_str().expect("a label value");
                assert!(
                    !stdout.contains(&format!("\"{value}\"")),
                    "{label}: a {group} value leaked to stdout as a JSON string"
                );
            }
        }
        assert!(
            snapshot(&out)
                .into_keys()
                .all(|path| path.starts_with(Path::new("import").join(label))),
            "{label}: the detail lands only under --out/import/{label}"
        );
        eprintln!("{label} result: {result}");
        envelope
    }

    /// The `#[ignore]` pilot test: the corpus and the census config from the
    /// label's variables, each required; the config outside the repository.
    fn pilot_from_environment(label: &str) {
        let variables = pilot::variables(label);
        let corpus = pilot::required(variables.corpus, "the pilot corpus");
        let corpus = fs::canonicalize(corpus).unwrap_or_else(|error| {
            panic!(
                "{}: the pilot corpus is unreadable: {error}",
                variables.corpus
            )
        });
        let config = pilot::required(variables.config, "the pilot's census config");
        let config = fs::canonicalize(config).unwrap_or_else(|error| {
            panic!(
                "{}: the census config is unreadable: {error}",
                variables.config
            )
        });
        assert!(
            !config.starts_with(repository_root()),
            "{}: a pilot census config lives outside the repository",
            variables.config
        );
        let scratch = Scratch::new(label);
        assert!(
            !scratch.0.starts_with(&corpus),
            "the scratch directory must not lie under the pilot"
        );
        import_run(label, &scratch.0, &corpus, &config);
    }

    #[test]
    #[ignore = "needs SPECENGINE_PILOT_A and SPECENGINE_CENSUS_CONFIG_A; read-only, owner-run"]
    fn pilot_a_import_prints_anonymous_counts() {
        pilot_from_environment("pilot-a");
    }

    #[test]
    #[ignore = "needs SPECENGINE_PILOT_B and SPECENGINE_CENSUS_CONFIG_B; read-only, owner-run"]
    fn pilot_b_import_prints_anonymous_counts() {
        pilot_from_environment("pilot-b");
    }

    /// A scratch copy of `fixtures/import-one` under `scratch/corpus` with
    /// `git init -q` and nothing committed; its `census.toml` moved beside
    /// the copy, outside it, so only the variable can name it.
    fn invented_setup(scratch: &Scratch) -> (std::path::PathBuf, std::path::PathBuf) {
        let corpus = scratch.join("corpus");
        copy_dir(&fixture_dir("import-one"), &corpus);
        let config = scratch.join("census-pilot.toml");
        fs::rename(corpus.join("census.toml"), &config).expect("import-one has a census.toml");
        pilot::git_init(&corpus);
        (corpus, config)
    }

    /// AC-10 on an invented setup: the helper runs `import --label pilot-a`
    /// and `pilot-b` on an `import-one` copy with only the corpus and
    /// census-config variables (no `census.toml` at the copy's root, no
    /// scheme): green, and `result` minus `*_ms` = `import-one`'s
    /// `expected.json`, as without a label.
    #[test]
    fn the_pilot_helper_imports_the_invented_setup_read_only() {
        for label in ["pilot-a", "pilot-b"] {
            let scratch = Scratch::new("pilot-setup");
            let (corpus, config) = invented_setup(&scratch);
            let envelope = import_run(label, &scratch.0, &corpus, &config);
            assert_eq!(
                without_ms(&envelope["result"]),
                expected("import-one"),
                "{label}: the config came from the variable"
            );
        }
    }

    /// AC-10 "the read-only proof over corpus and code roots": the helper's
    /// roots are the census config's `[corpus]` then `[code]` roots, and an
    /// edit under a code root alone changes the proof (git status unchanged:
    /// nothing is committed, the file stays untracked).
    #[test]
    fn the_import_proof_covers_the_corpus_and_the_code_roots() {
        let scratch = Scratch::new("pilot-proof");
        let (corpus, config) = invented_setup(&scratch);
        let roots = pilot::census_roots(&config, "SPECENGINE_CENSUS_CONFIG_A");
        assert_eq!(roots, ["spec", "src", "tests"]);
        let before = pilot::proof(&corpus, &roots);
        let code = corpus.join("src/lib.rs");
        let mut text = fs::read(&code).unwrap();
        text.extend_from_slice(b"\n// one more line\n");
        fs::write(&code, text).unwrap();
        let after = pilot::proof(&corpus, &roots);
        assert_eq!(before.status, after.status, "the file stays untracked");
        assert_ne!(before, after, "an edit under a code root changes the proof");
    }

    /// The helper fails, naming the variable, on a census config the engine
    /// refuses; it does not skip.
    #[test]
    #[should_panic(expected = "SPECENGINE_CENSUS_CONFIG_B: ")]
    fn the_import_helper_fails_on_a_refused_config() {
        let scratch = Scratch::new("pilot-bad-config");
        let (corpus, config) = invented_setup(&scratch);
        fs::write(&config, "[corpus]\nroots = []\n").unwrap();
        pilot::run_census_read_only("import", "pilot-b", &scratch.0, &corpus, &config);
    }
}
