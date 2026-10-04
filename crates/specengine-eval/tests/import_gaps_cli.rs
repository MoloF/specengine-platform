//! docs/features/import-gaps.md end to end on the real `specengine-eval`
//! binary: AC-08 (`[documents]` accepted by `census` and `import`, every
//! bad value refused at its line), AC-07 (`census` unchanged: corpus-mini
//! and its `expected.json` file untouched, each fixture's census equal with
//! and without `[documents]`), AC-09 (stdout counts of the new forms equal
//! the detail files). The whitelist, `expected.json` and the genre test are
//! `import_cli.rs` and `import_genre.rs`; the engine rules are
//! `crates/specengine-import/tests/import_gaps.rs`.
//!
//! The fixtures are only read; every run writes under a scratch `--out`.

mod import_support;

use std::fs;

use import_support::*;
use serde_json::Value;

const FIXTURES: [&str; 2] = ["import-one", "import-two"];

/// A valid census part every case starts from (4 lines).
const BASE: &str = "[corpus]\nroots = [\"spec\"]\n[ids]\nregex = '^[A-Z]{2,3}-[0-9]{3}$'\n";

/// AC-08: both fixture configs (each with `[documents]`) and a config with
/// both keys are accepted by `census` and `import`.
#[test]
fn ac08_documents_keys_are_accepted_by_census_and_import() {
    let scratch = Scratch::new("documents-accepted");
    for name in FIXTURES {
        let config = fs::read_to_string(fixture_config(name)).unwrap();
        assert!(config.contains("[documents]\nid_key = "), "{name}");
        for measurement in ["census", "import"] {
            let envelope = measure(
                measurement,
                &fixture_dir(name),
                &scratch.join(&format!("{name}-{measurement}")),
                &[],
            );
            assert_eq!(envelope["measurement"], measurement);
        }
    }
    let config = scratch.join("both.toml");
    fs::write(
        &config,
        format!(
            "{BASE}[documents]\nid_key = \"ident\"\nid_path = '^spec/(?P<id>[A-Z]+-[0-9]+)\\.md$'\n"
        ),
    )
    .unwrap();
    for measurement in ["census", "import"] {
        measure(
            measurement,
            &fixture_dir("import-one"),
            &scratch.join(&format!("both-{measurement}")),
            &["--config", config.to_str().unwrap()],
        );
    }
}

/// AC-08: a bad `[documents]` value refuses `census` and `import` alike:
/// exit 2 at `<config>:<line>`, nothing on stdout, nothing under `--out`;
/// an `id_key` with blanks around it (it would match no header key) alike.
/// M: `deny_unknown_fields` dropped from `[documents]`.
#[test]
fn ac08_bad_documents_keys_are_refused_at_their_line() {
    // (case, appended text, 1-based line of the error within it)
    let cases: [(&str, &str, usize); 11] = [
        (
            "unknown key",
            "[documents]\nid_key = \"ident\"\nid_column = 1\n",
            3,
        ),
        ("id_key not a string", "[documents]\nid_key = 1\n", 2),
        ("id_path not a string", "[documents]\nid_path = ['x']\n", 2),
        ("empty id_key", "[documents]\nid_key = \"\"\n", 2),
        (
            "id_key with a blank before",
            "[documents]\nid_key = \" docid\"\n",
            2,
        ),
        (
            "id_key with a tab after",
            "[documents]\nid_key = \"ident\\t\"\n",
            2,
        ),
        (
            "id_key with an NBSP before",
            "[documents]\nid_key = \"\\u00A0ident\"\n",
            2,
        ),
        (
            "bad id_path regex",
            "[documents]\nid_path = '^(?P<id>[A-Z]+'\n",
            2,
        ),
        (
            "id_path matching the empty path",
            "[documents]\nid_path = '(?P<id>[A-Z]*)'\n",
            2,
        ),
        (
            "id_path without a group",
            "[documents]\nid_path = '^spec/[A-Z]+\\.md$'\n",
            2,
        ),
        (
            "id_path with another group",
            "[documents]\nid_path = '^spec/(?P<name>[A-Z]+)\\.md$'\n",
            2,
        ),
    ];
    let scratch = Scratch::new("documents-refused");
    let corpus = fixture_dir("import-one");
    let base_lines = BASE.lines().count();
    let mut failures = Vec::new();
    for (index, (case, appended, line)) in cases.into_iter().enumerate() {
        let config = scratch.join(&format!("bad-{index}.toml"));
        fs::write(&config, format!("{BASE}{appended}")).unwrap();
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

/// AC-07: `census` on corpus-mini prints its `expected.json`, a file this
/// task leaves untouched.
#[test]
fn ac07_corpus_mini_census_and_its_expected_json_are_unchanged() {
    assert_eq!(
        git_status(&repository_root(), "fixtures/corpus-mini"),
        "",
        "fixtures/corpus-mini is unchanged"
    );
    let scratch = Scratch::new("gaps-corpus-mini");
    let out = scratch.join("out");
    let envelope = envelope(&run(&["census", "--out", out.to_str().unwrap()]));
    let expected = read_json(&repository_root().join("fixtures/corpus-mini/expected.json"));
    for (key, want) in expected.as_object().unwrap() {
        assert_eq!(&envelope["result"][key.as_str()], want, "result.{key}");
    }
}

/// The fixture config without its `[documents]` table.
fn without_documents(name: &str) -> String {
    let text = fs::read_to_string(fixture_config(name)).unwrap();
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        if line.starts_with('[') {
            skipping = line.trim() == "[documents]";
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    assert_ne!(out, text, "{name}: the config had [documents]");
    out
}

/// AC-07: each fixture (titled lead-ins, document IDs) gives the same
/// census `result` and byte-identical detail with and without
/// `[documents]`; `import` without it finds no document record and demotes
/// nothing.
#[test]
fn ac07_the_census_ignores_documents_and_import_has_no_default() {
    let scratch = Scratch::new("documents-ignored");
    for name in FIXTURES {
        let config = scratch.join(&format!("{name}-no-documents.toml"));
        fs::write(&config, without_documents(name)).unwrap();
        let with_out = scratch.join(&format!("{name}-with"));
        let with = measure("census", &fixture_dir(name), &with_out, &[]);
        let without_out = scratch.join(&format!("{name}-without"));
        let without = measure(
            "census",
            &fixture_dir(name),
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
        let import = ImportRun::new(
            &fixture_dir(name),
            &scratch,
            &format!("{name}-import-without"),
            &["--config", config.to_str().unwrap()],
        );
        assert_eq!(import.count("records/per_form/document"), 0, "{name}");
        assert_eq!(import.count("references/by_document"), 0, "{name}");
    }
}

/// AC-09: the three new stdout counts equal what the detail files hold.
#[test]
fn ac09_new_stdout_counts_match_the_detail_files() {
    let scratch = Scratch::new("new-counts");
    for name in FIXTURES {
        let run = ImportRun::new(&fixture_dir(name), &scratch, name, &[]);
        let records = run.records();
        let titled = records
            .iter()
            .filter(|record| record.get("title").is_some())
            .count() as u64;
        let documents = records
            .iter()
            .filter(|record| record["form"] == "document")
            .count() as u64;
        assert_eq!(run.count("records/titled"), titled, "{name}");
        assert_eq!(run.count("records/per_form/document"), documents, "{name}");
        assert!(
            records
                .iter()
                .filter(|record| record.get("title").is_some())
                .all(|record| record["form"] == "list-item" && record["title"].is_string()),
            "{name}: only list items carry a title"
        );
        assert!(
            records
                .iter()
                .filter(|record| record["form"] != "list-item")
                .all(|record| record.get("title").is_none()),
            "{name}: `title` omitted elsewhere"
        );
        // Demoted: references whose ID a document of no reference path defines.
        let defined_by_documents: Vec<&Value> = records
            .iter()
            .filter(|record| record["form"] == "document" && record["role"] == "definition")
            .map(|record| &record["id"])
            .collect();
        let demoted = records
            .iter()
            .filter(|record| {
                record["form"] != "document"
                    && record["role"] == "reference"
                    && defined_by_documents.contains(&&record["id"])
            })
            .count() as u64;
        assert_eq!(run.count("references/by_document"), demoted, "{name}");
        assert!(
            run.count("references/by_document") <= run.count("references/total"),
            "{name}: by_document within references.total"
        );
    }
}
