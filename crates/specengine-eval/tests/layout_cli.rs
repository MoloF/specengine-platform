//! docs/features/import-layout.md AC-01…AC-10 through the real
//! `specengine-eval layout` binary, on the two invented conventions
//! `fixtures/import-layout/one` (a requirements notebook) and
//! `fixtures/import-layout/two` (a decision log), and on test-time copies
//! of them (non-Latin text built from `\u{...}` escapes here, never stored
//! in a fixture). The genre test of AC-10 is `import_genre.rs`; the tamper
//! tests are the `#[cfg(test)]` module of `crates/specengine-eval/src/layout.rs`.
//!
//! Nothing here names a pilot; every run writes only under a scratch `--out`.

mod import_support;
#[cfg(unix)]
mod pilot;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use import_support::*;
use serde_json::Value;

/// The two layout conventions, relative to `fixtures/`.
const ONE: &str = "import-layout/one";
const TWO: &str = "import-layout/two";
const LAYOUT_FIXTURES: [&str; 2] = [ONE, TWO];

/// The run date of every test run (both fixtures' `debt_expires` lie far
/// after it).
const TODAY: &str = "2026-10-04";

/// The detail files beside the tree (docs/features/import-layout.md
/// "stdout and detail").
const DETAIL_FILES: [&str; 7] = [
    "emission.json",
    "records.json",
    "prose.json",
    "extents.json",
    "headers.json",
    "findings.json",
    "diagnostics.json",
];

/// The seven `reasons` keys, always printed.
const REASONS: [&str; 7] = [
    "prefix_unknown",
    "slug",
    "path_taken",
    "section_fields",
    "header_unparseable",
    "reader_boundary",
    "unexplained",
];

// ------------------------------------------------------------------ plumbing

/// One `layout` run: its stdout envelope and its detail directory.
struct LayoutRun {
    output: Output,
    envelope: Value,
    detail: PathBuf,
}

impl LayoutRun {
    /// `layout --pilot <corpus> --out <scratch>/<name> --today TODAY`.
    fn new(corpus: &Path, scratch: &Scratch, name: &str, extra: &[&str]) -> Self {
        let out = scratch.join(name);
        let mut args = vec![
            "layout",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ];
        args.extend_from_slice(extra);
        let output = run(&args);
        let envelope = envelope(&output);
        assert_eq!(envelope["measurement"], "layout");
        Self {
            output,
            envelope,
            detail: out.join("layout").join("pilot"),
        }
    }

    fn of(name: &str, scratch: &Scratch) -> Self {
        Self::new(&fixture_dir(name), scratch, &name.replace('/', "-"), &[])
    }

    fn result(&self) -> &Value {
        &self.envelope["result"]
    }

    fn tree(&self) -> PathBuf {
        self.detail.join("tree")
    }

    fn file(&self, name: &str) -> Value {
        read_json(&self.detail.join(name))
    }

    fn tree_text(&self, path: &str) -> String {
        fs::read_to_string(self.tree().join(path))
            .unwrap_or_else(|error| panic!("tree file {path}: {error}"))
    }

    /// The count at a `/`-separated path of `result`.
    fn count(&self, path: &str) -> u64 {
        let mut value = self.result();
        for part in path.split('/') {
            value = &value[part];
        }
        value
            .as_u64()
            .unwrap_or_else(|| panic!("result/{path} is not a count: {value}"))
    }

    fn definitions(&self) -> Vec<Value> {
        self.file("emission.json")["definitions"]
            .as_array()
            .expect("emission definitions")
            .clone()
    }

    fn documents(&self) -> Vec<Value> {
        self.file("emission.json")["documents"]
            .as_array()
            .expect("emission documents")
            .clone()
    }

    /// The emission entry of the definition `id` at source `line`.
    fn definition(&self, id: &str, line: u64) -> Value {
        self.definitions()
            .into_iter()
            .find(|entry| entry["id"] == id && entry["line"] == line)
            .unwrap_or_else(|| panic!("no definition {id} at line {line}"))
    }

    fn records(&self) -> Vec<Value> {
        self.file("records.json")
            .as_array()
            .expect("records.json is an array")
            .clone()
    }

    fn tree_findings(&self) -> Vec<Value> {
        self.file("findings.json")["tree"]
            .as_array()
            .expect("findings.json tree")
            .clone()
    }

    fn stdout(&self) -> String {
        String::from_utf8(self.output.stdout.clone()).expect("stdout is UTF-8")
    }
}

fn expected(name: &str) -> Value {
    read_json(&fixture_dir(name).join("expected.json"))
}

/// A scratch copy of a layout fixture.
fn copy_of(name: &str, scratch: &Scratch, as_name: &str) -> PathBuf {
    let corpus = scratch.join(as_name);
    copy_dir(&fixture_dir(name), &corpus);
    corpus
}

/// `text` with `from` replaced once; `from` must be there.
fn replaced(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?} not in:\n{text}");
    text.replacen(from, to, 1)
}

fn edit(path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(path).expect("readable");
    fs::write(path, replaced(&text, from, to)).expect("writable");
}

/// Every file under `dir`, relative, `/`-separated, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    snapshot(dir)
        .into_keys()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect()
}

/// The `[layout]` table of a census config.
fn layout_table(name: &str) -> toml::Table {
    let text = fs::read_to_string(fixture_config(name)).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    table["layout"].as_table().expect("[layout]").clone()
}

fn layout_string(name: &str, key: &str) -> String {
    layout_table(name)[key].as_str().unwrap().to_owned()
}

/// Definitions of the source with a task box, by the lead-in shape
/// `- [x] **ID…`, `- [ ] **ID…` of both fixtures.
fn boxed_definitions(name: &str) -> usize {
    snapshot(&fixture_dir(name))
        .into_iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "md"))
        .map(|(_, bytes)| {
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .filter(|line| {
                    ["- [x] **", "- [X] **", "- [ ] **"]
                        .iter()
                        .any(|start| line.starts_with(start))
                })
                .count()
        })
        .sum()
}

// --------------------------------------------------------------- AC-01, base

/// AC-01: `layout` prints each fixture's `expected.json` (timings aside);
/// without `--pilot` it runs on `fixtures/import-layout/one`; the tree, the
/// index and the seven detail files sit under `<out>/layout/<label>/`.
#[test]
fn layout_on_each_fixture_matches_expected_json() {
    let scratch = Scratch::new("layout-expected");
    let out = scratch.join("default");
    let envelope = envelope(&run(&[
        "layout",
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]));
    assert_eq!(envelope["label"], "fixtures", "the default label");
    assert_eq!(
        without_ms(&envelope["result"]),
        expected(ONE),
        "layout without --pilot runs on fixtures/import-layout/one"
    );
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        assert_eq!(
            without_ms(run.result()),
            expected(name),
            "{name}: result minus *_ms differs from expected.json"
        );
        assert!(run.tree().join("specengine.toml").is_file(), "{name}");
        assert!(run.tree().join(".spec-debt.toml").is_file(), "{name}");
        assert!(run.detail.join("index.db").is_file(), "{name}");
        for detail in DETAIL_FILES {
            assert!(run.detail.join(detail).is_file(), "{name}: {detail}");
        }
    }
}

/// AC-01: every `file` definition is one record file at
/// `<records>/<P>/<ID>.md` (a later duplicate `-2`), every feature-scoped
/// one a `{#ID}` section of a document under `<features>`, a document
/// record stays in place with `id:`, one residue per walked document; the
/// forms of AC-01 each land as a file across the two conventions.
#[test]
fn every_definition_lands_at_its_place() {
    let scratch = Scratch::new("layout-places");
    let mut file_forms = BTreeSet::new();
    let mut titled_items = 0;
    let mut untitled_items = 0;
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        let records = layout_string(name, "records");
        let features = layout_string(name, "features");
        let tree_files = files_under(&run.tree());
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut duplicates = 0;
        for entry in run.definitions() {
            let id = entry["id"].as_str().unwrap();
            let prefix = id.split('-').next().unwrap();
            match entry["place"].as_str() {
                Some("file") => {
                    let count = seen.entry(id.to_owned()).or_default();
                    *count += 1;
                    let stem = if *count == 1 {
                        id.to_owned()
                    } else {
                        duplicates += 1;
                        format!("{id}-{count}")
                    };
                    let path = format!("{records}/{prefix}/{stem}.md");
                    assert_eq!(entry["after"], path.as_str(), "{name}: {entry}");
                    let text = run.tree_text(&path);
                    assert!(
                        text.starts_with(&format!("---\nid: {id}\nclass: ")),
                        "{name}: {path}:\n{text}"
                    );
                    let form = entry["form"].as_str().unwrap().to_owned();
                    if form == "list-item" {
                        if text.contains("\ntitle: ") {
                            titled_items += 1;
                        } else {
                            untitled_items += 1;
                        }
                    }
                    file_forms.insert(form);
                }
                Some("section") => {
                    let after = entry["after"].as_str().unwrap();
                    let text = run.tree_text(after);
                    assert!(text.contains(&format!("{{#{id}")), "{name}: {after}");
                    if prefix == "CK" || prefix == "CRT" {
                        assert!(
                            after.starts_with(&format!("{features}/")),
                            "{name}: feature-scoped {id} under {features}: {after}"
                        );
                    }
                }
                Some("in-place") => {
                    assert_eq!(entry["after"], entry["path"], "{name}: {entry}");
                    if entry["form"] == "document" {
                        let text = run.tree_text(entry["after"].as_str().unwrap());
                        assert!(
                            text.starts_with("---\n") && text.contains(&format!("\nid: {id}\n")),
                            "{name}: document record {id}:\n{text}"
                        );
                    }
                }
                None => assert!(entry["reason"].is_string(), "{name}: {entry}"),
                Some(other) => panic!("{name}: place {other}"),
            }
        }
        assert_eq!(duplicates, 1, "{name}: one later duplicate, `-2`");

        // One residue per walked document; the tree is residues, record
        // files and the rendered index outputs.
        let documents = run.documents();
        assert_eq!(
            documents.len() as u64,
            run.count("before/documents"),
            "{name}"
        );
        for document in &documents {
            let after = document["after"].as_str().unwrap();
            assert!(
                tree_files.iter().any(|path| path == after),
                "{name}: {after}"
            );
        }
        let markdown = tree_files
            .iter()
            .filter(|path| path.ends_with(".md"))
            .count() as u64;
        assert_eq!(
            markdown,
            run.count("tree/documents"),
            "{name}: {tree_files:?}"
        );
        assert_eq!(
            run.count("tree/documents"),
            documents.len() as u64 + run.count("tree/record_files") + u64::from(name == TWO), // two's `index = true` output
            "{name}"
        );

        // The emitted configs load: `check` reads the tree with them.
        let check_out = scratch.join(&format!("{}-check", name.replace('/', "-")));
        let check = envelope(&run_check(&run.tree(), &check_out));
        assert_eq!(
            check["result"]["verdicts"]["enforce"], "clean",
            "{name}: {check}"
        );
        assert_eq!(check["result"]["stale"], 0, "{name}");
        assert_eq!(
            check["result"]["files"].as_u64(),
            Some(run.count("tree/documents")),
            "{name}"
        );
    }
    for form in ["table-row", "headerless-row", "list-item", "section"] {
        assert!(
            file_forms.contains(form),
            "{form} → a record file: {file_forms:?}"
        );
    }
    assert!(
        titled_items > 0 && untitled_items > 0,
        "{titled_items} {untitled_items}"
    );
}

/// `check` on a tree with its own `specengine.toml` and `.spec-debt.toml`.
fn run_check(tree: &Path, out: &Path) -> Output {
    run(&[
        "check",
        "--pilot",
        tree.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ])
}

/// AC-01: a feature document outside `<features>` moves to
/// `<features>/<slug>.md` (one by its stem, two by the `slug` group); one
/// directly under `<features>` stays; a non-slug stays with its
/// feature-scoped definitions missing (`slug`); a record path a walked
/// document holds is not written (`path_taken`, AC-08); a prefix outside
/// the scheme is missing (`prefix_unknown`).
#[test]
fn feature_documents_move_and_the_emitter_reasons_name_each_miss() {
    let scratch = Scratch::new("layout-moves");
    let one = LayoutRun::of(ONE, &scratch);
    let moved: Vec<(String, String)> = one
        .documents()
        .iter()
        .filter(|document| document["moved"] == true)
        .map(|document| {
            (
                document["source"].as_str().unwrap().to_owned(),
                document["after"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        moved,
        [(
            "book/checkout.md".to_owned(),
            "book/flows/checkout.md".to_owned()
        )]
    );
    assert!(!one.tree().join("book/checkout.md").exists());
    let stays = one
        .documents()
        .into_iter()
        .find(|document| document["source"] == "book/flows/login.md")
        .unwrap();
    assert_eq!(stays["moved"], false);
    assert_eq!(stays["feature"], true);

    let two = LayoutRun::of(TWO, &scratch);
    let moved: Vec<(String, String)> = two
        .documents()
        .iter()
        .filter(|document| document["moved"] == true)
        .map(|document| {
            (
                document["source"].as_str().unwrap().to_owned(),
                document["after"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        moved,
        [(
            "pages/billing/index.md".to_owned(),
            "topics/billing.md".to_owned()
        )]
    );
    let unslugged = two.definition("CRT-0003", 3);
    assert_eq!(unslugged["reason"], "slug");
    assert!(unslugged["after"].is_null());
    assert!(two.tree().join("pages/Bulk_Export/index.md").is_file());
    assert!(
        two.tree_text("pages/Bulk_Export/index.md")
            .contains("- **CRT-0003** - every row is exported\n"),
        "a missing definition keeps its source lines"
    );

    let taken = two.definition("DEC-0005", 25);
    assert_eq!(taken["reason"], "path_taken");
    assert!(taken["after"].is_null());
    let walked = two.tree_text("ledger/DEC/DEC-0005.md");
    assert!(
        !walked.contains("\nid:"),
        "the walked page keeps its own text:\n{walked}"
    );
    assert!(walked.contains("# A walked page at a record path\n"));

    let unknown = two.definition("ZZQ-0001", 13);
    assert_eq!(unknown["reason"], "prefix_unknown");
    assert!(unknown["after"].is_null());

    let diagnostics = two.file("diagnostics.json");
    let reasons: BTreeSet<&str> = diagnostics["layout"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["reason"].as_str())
        .collect();
    assert_eq!(
        reasons,
        BTreeSet::from(["path_taken", "prefix_unknown", "slug"])
    );
    for reason in ["path_taken", "prefix_unknown", "slug"] {
        assert_eq!(two.count(&format!("reasons/{reason}")), 1, "{reason}");
    }
}

/// "Feature documents": a `<features>/<slug>.md` target that a walked
/// document holds, or that two feature documents share, is a diagnostic;
/// each such document stays and its feature-scoped definitions are missing
/// (`slug`), never written over another document.
#[test]
fn a_walked_or_shared_feature_target_keeps_the_documents_in_place() {
    let scratch = Scratch::new("layout-slug-targets");
    let corpus = copy_of(ONE, &scratch, "walked");
    fs::write(
        corpus.join("book/flows/checkout.md"),
        "# Checkout notes\n\nA walked page where the flow would move.\n",
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "walked-out", &[]);
    assert_eq!(
        run.count("reasons/slug"),
        2,
        "CK-01 and CK-02 of book/checkout.md"
    );
    assert_eq!(run.count("tree/moved"), 0);
    assert_eq!(
        run.tree_text("book/flows/checkout.md"),
        "---\nclass: spec\n---\n# Checkout notes\n\nA walked page where the flow would move.\n"
    );
    assert!(run.tree().join("book/checkout.md").is_file());
    assert_eq!(run.count("hashes/mismatched"), 0);
    assert_eq!(run.count("prose/mismatched"), 0);

    let corpus = copy_of(TWO, &scratch, "shared");
    fs::write(
        corpus.join("log/billing.md"),
        "# Billing log\n\n- **CRT-0009** - a second document whose stem is `billing`\n",
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "shared-out", &[]);
    assert_eq!(
        run.count("reasons/slug"),
        4,
        "CRT-0001, CRT-0002, CRT-0009 and Bulk_Export's CRT-0003"
    );
    assert_eq!(run.count("tree/moved"), 0);
    assert!(!run.tree().join("topics/billing.md").exists());
    assert!(run.tree().join("pages/billing/index.md").is_file());
    assert!(run.tree().join("log/billing.md").is_file());
    assert_eq!(run.count("hashes/mismatched"), 0);
}

/// AC-01: the record file, reshaped heading, Q4 placement and rule S bytes
/// of the spec ("Rules and edge cases").
#[test]
fn record_files_reshaped_sections_and_rule_s_are_the_spec_s_bytes() {
    let scratch = Scratch::new("layout-bytes");
    let one = LayoutRun::of(ONE, &scratch);
    // The spec's example shape: a boxed titled item → id, class, title,
    // the task key, then the body.
    assert_eq!(
        one.tree_text("book/atoms/ASK/ASK-012.md"),
        "---\nid: ASK-012\nclass: canon\ntitle: \"Short `name`\"\nticked: true\n---\n\nWhich name does the notebook print?\n"
    );
    // A row: fields in column order, the key-map target else as written.
    assert_eq!(
        one.tree_text("book/atoms/NEED/NEED-01.md"),
        "---\nid: NEED-01\nclass: canon\nweight: \"high\"\nArea: \"input\"\n---\n\nThe engine reads a pipe \\| inside a cell.\n"
    );
    // A legacy row: `aliases` with the written ID.
    assert!(
        one.tree_text("book/atoms/NEED/NEED-05.md")
            .starts_with("---\nid: NEED-05\nclass: canon\naliases: [\"OLDR-05\"]\n")
    );
    // A section: its heading text the title, its other attribute a key,
    // the body without the heading line.
    assert_eq!(
        one.tree_text("book/atoms/NEED/NEED-40.md"),
        "---\nid: NEED-40\nclass: canon\ntitle: \"Gate\"\nlevel: \"two\"\n---\n\nThe gate text moves to a record file.\n\nIt has two paragraphs.\n"
    );
    // Reshaped under `## Rules` (n = 2): level 3, after the section's last
    // non-blank residue line, in source order; rule S on an in-place
    // legacy section.
    let needs = one.tree_text("book/needs.md");
    let lines: Vec<&str> = needs.lines().collect();
    let at = |wanted: &str| {
        lines
            .iter()
            .position(|line| *line == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} not in:\n{needs}"))
    };
    let paragraph = at("A paragraph between the rules and the subsection.");
    let first = at("### First rule {#RULE-01 ticked=false}");
    let second = at("### RULE-02 {#RULE-02}");
    let old_law = at("## Old law {#RULE-09}");
    assert!(
        paragraph < first && first < second && second < old_law,
        "{needs}"
    );
    assert_eq!(lines[first - 1], "");
    assert_eq!(lines[first + 1], "");
    assert_eq!(
        lines[first + 2],
        "The engine reads the notebook before the flows."
    );
    assert!(!needs.contains("LAW-09"), "rule S: {needs}");
    assert!(!needs.contains("RULE-01: First rule"), "{needs}");
    // Feature criteria under `# Checkout` (n = 1): level 2, the box as the
    // task key.
    let checkout = one.tree_text("book/flows/checkout.md");
    assert!(
        checkout.ends_with(
            "Closing prose of the flow.\n\n## CK-01 {#CK-01 ticked=false}\n\nThe cart total is shown before payment.\n\n## Receipt {#CK-02 ticked=true}\n\nA receipt is written after payment.\n"
        ),
        "{checkout}"
    );

    let two = LayoutRun::of(TWO, &scratch);
    // n = 0: no heading above the item → level 1 after the document's last
    // non-blank residue line.
    assert!(
        two.tree_text("log/problems.md").ends_with(
            "Problems become sections of their document.\n\n# Slow reads {#PRB-0001}\n\nthe log is read twice\n"
        ),
        "{}",
        two.tree_text("log/problems.md")
    );
    // A legacy section to its file, the reshaped block nested in it landing
    // there; the map points there.
    assert_eq!(
        two.tree_text("ledger/DEC/DEC-0003.md"),
        "---\nid: DEC-0003\nclass: decision\ntitle: \"Superseded\"\naliases: [\"OLD-0003\"]\n---\n\nA legacy section moves to its record file.\n\n### PRB-0002 {#PRB-0002}\n\na problem nested in the moved section\n"
    );
    assert_eq!(
        two.definition("PRB-0002", 21)["after"],
        "ledger/DEC/DEC-0003.md"
    );
    // Rule S on a document record's `id:`; the alias added after `---`.
    let legacy = two.tree_text("log/OLD-0041.md");
    assert!(
        legacy.starts_with("---\nclass: decision\naliases: [\"OLD-0041\"]\nid: DEC-0041\n"),
        "{legacy}"
    );
    // No task_box_key: the box is dropped, the text kept.
    assert_eq!(
        two.tree_text("ledger/DEC/DEC-0007.md"),
        "---\nid: DEC-0007\nclass: decision\n---\n\na boxed item loses its box\n"
    );
    // A headerless row's other cells are `col-N` fields.
    assert!(
        two.tree_text("ledger/DEC/DEC-0001-2.md")
            .starts_with("---\nid: DEC-0001\nclass: decision\ncol-0: \"3\"\n---\n")
    );
}

// --------------------------------------------------------------------- AC-02

/// AC-02: every emitted definition matches by identity; each miss is one
/// the emitter named; two empty-text titled records keep their own titles
/// (a match by hash would swap them: M2).
#[test]
fn hashes_match_by_identity_and_every_miss_is_named() {
    let scratch = Scratch::new("layout-hashes");
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        let definitions = run.count("before/definitions");
        let named = run.count("reasons/prefix_unknown")
            + run.count("reasons/slug")
            + run.count("reasons/path_taken");
        assert_eq!(run.count("hashes/mismatched"), 0, "{name}");
        assert_eq!(run.count("hashes/extra"), 0, "{name}");
        assert_eq!(run.count("hashes/missing"), named, "{name}");
        assert_eq!(run.count("hashes/matched"), definitions - named, "{name}");
        assert_eq!(run.count("reasons/unexplained"), 0, "{name}");
        for record in run.records() {
            match record["hash"].as_str() {
                Some("matched") => assert_eq!(record["before_hash"], record["after_hash"]),
                Some("missing") => assert!(record["reason"].is_string(), "{name}: {record}"),
                other => panic!("{name}: {other:?} {record}"),
            }
        }
    }
    let one = LayoutRun::of(ONE, &scratch);
    assert_eq!(
        (
            one.count("hashes/matched"),
            one.count("hashes/mismatched"),
            one.count("hashes/missing"),
            one.count("hashes/extra"),
        ),
        (one.count("before/definitions"), 0, 0, 0),
        "AC-02 literally on the first convention"
    );
    let empty: Vec<Value> = one
        .records()
        .into_iter()
        .filter(|record| record["id"] == "ASK-020" || record["id"] == "ASK-021")
        .collect();
    assert_eq!(empty.len(), 2);
    assert_eq!(
        empty[0]["before_hash"], empty[1]["before_hash"],
        "two titled records with the same (empty) text"
    );
    for record in &empty {
        assert_eq!(record["title_matched"], true, "{record}");
    }
}

// --------------------------------------------------------------------- AC-03

/// AC-03: titles (one with inline code), fields and task boxes match; with
/// no `task_box_key` every box is dropped.
#[test]
fn titles_fields_and_task_boxes_match() {
    let scratch = Scratch::new("layout-titles");
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        assert_eq!(run.count("titles/mismatched"), 0, "{name}");
        assert_eq!(run.count("fields/mismatched"), 0, "{name}");
        assert!(run.count("titles/matched") > 0, "{name}");
        assert!(run.count("fields/matched") > 0, "{name}");
        let boxes = boxed_definitions(name) as u64;
        assert!(boxes > 0, "{name}");
        let has_key = layout_table(name).contains_key("task_box_key");
        let (carried, dropped) = (run.count("task_box/carried"), run.count("task_box/dropped"));
        if has_key {
            assert_eq!((carried, dropped), (boxes, 0), "{name}");
        } else {
            assert_eq!((carried, dropped), (0, boxes), "{name}");
        }
    }
    let one = LayoutRun::of(ONE, &scratch);
    let code_title = one
        .records()
        .into_iter()
        .find(|record| record["id"] == "ASK-012")
        .unwrap();
    assert_eq!(code_title["title_matched"], true);
    let two = LayoutRun::of(TWO, &scratch);
    let code_title = two
        .records()
        .into_iter()
        .find(|record| record["id"] == "CRT-0002")
        .unwrap();
    assert_eq!(
        code_title["title_matched"], true,
        "a reshaped `Refund \\`now\\``"
    );

    // The first convention without its task_box_key: every box dropped,
    // the key nowhere in the tree.
    let corpus = copy_of(ONE, &scratch, "no-box-key");
    edit(
        &corpus.join("census.toml"),
        "task_box_key = \"ticked\"\n",
        "",
    );
    let run = LayoutRun::new(&corpus, &scratch, "no-box-key-out", &[]);
    assert_eq!(run.count("task_box/carried"), 0);
    assert_eq!(run.count("task_box/dropped"), boxed_definitions(ONE) as u64);
    for (path, bytes) in snapshot(&run.tree()) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains("ticked"),
            "{}",
            path.display()
        );
    }
}

// --------------------------------------------------------------------- AC-04

/// AC-04: prose between criteria, around a record table, an ID-less
/// sibling item and an emptied table survive; no extent leaves a residue.
#[test]
fn prose_around_moved_records_survives_and_extents_leave_no_residue() {
    let scratch = Scratch::new("layout-prose");
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        assert_eq!(run.count("prose/mismatched"), 0, "{name}");
        assert_eq!(run.count("extents/residue"), 0, "{name}");
        assert_eq!(
            run.count("prose/documents"),
            run.count("before/documents"),
            "{name}"
        );
        let moved_out = run
            .definitions()
            .iter()
            .filter(|entry| matches!(entry["place"].as_str(), Some("file" | "section")))
            .count() as u64;
        // Interpretation 1 (iteration 2): each field table is one extent.
        let field_tables = run.file("headers.json")["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|outcome| outcome["source"] == "field-table")
            .count() as u64;
        assert_eq!(field_tables, 1, "{name}: one field-table document");
        assert_eq!(
            run.count("extents/total"),
            moved_out + field_tables,
            "{name}"
        );
        let prose = run.file("prose.json");
        for document in prose.as_array().unwrap() {
            assert_eq!(document["matched"], true, "{name}: {document}");
        }
    }
    let one = LayoutRun::of(ONE, &scratch);
    let needs = one.tree_text("book/needs.md");
    for kept in [
        "The notebook opens with the needs the engine must meet.\n\n| Code | Wording | Weight | Area |\n|------|---------|--------|------|\n\nProse after the table names no record.\n",
        "\n- an ID-less sibling item stays where it is\n",
        "## Duplicates\n\n| Code | Wording |\n|------|---------|\n\nSee the [checkout](checkout.md) flow.\n",
    ] {
        assert!(needs.contains(kept), "{kept:?} not in:\n{needs}");
    }
    let checkout = one.tree_text("book/flows/checkout.md");
    assert!(
        checkout.contains("Prose between the criteria explains the order.\n"),
        "{checkout}"
    );
    let two = LayoutRun::of(TWO, &scratch);
    let log = two.tree_text("log/decisions.md");
    for kept in [
        "The log keeps one row per decision.\n\n| 4 | ZZQ-0001 | A row under a prefix the scheme lacks. |\n\nProse after the rows names no decision.\n",
        "- a sibling item without an ID\n",
    ] {
        assert!(log.contains(kept), "{kept:?} not in:\n{log}");
    }
    assert!(
        two.tree_text("log/problems.md")
            .contains("- an ID-less item before any heading\n")
    );
}

// --------------------------------------------------------------------- AC-05

/// AC-05: every source header key reaches the after header (renamed,
/// value mapped), a field table becomes the YAML block, nothing dropped.
#[test]
fn header_keys_are_renamed_mapped_and_none_dropped() {
    let scratch = Scratch::new("layout-headers");
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        assert_eq!(run.count("header/keys_dropped"), 0, "{name}");
        assert_eq!(run.count("header/unparseable"), 0, "{name}");
        assert_eq!(
            run.count("header/documents"),
            run.count("before/documents"),
            "{name}"
        );
        let headers = run.file("headers.json");
        assert!(headers["dropped"].as_array().unwrap().is_empty(), "{name}");
        let outcomes = headers["outcomes"].as_array().unwrap();
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome["source"] == "field-table"),
            "{name}: a field-table header"
        );
        assert!(
            outcomes.iter().any(|o| o["renamed"].as_u64() > Some(0)),
            "{name}"
        );
        assert!(
            outcomes.iter().any(|o| o["mapped"].as_u64() > Some(0)),
            "{name}"
        );
    }
    let one = LayoutRun::of(ONE, &scratch);
    assert_eq!(
        one.tree_text("book/plan.md"),
        "---\nid: NEED-30\nclass: canon\nserial: \"NEED-30\"\nstatus: \"accepted\"\nowner: \"team\"\n---\n# Plan\n\nThe plan is a document record: its field table names its serial.\n\nSteps come in order; the [gate](needs.md#NEED-40) decides.\n\n1. Read the notebook.\n2. Write the tree.\n\nThe OLDR-05 need is cited under its legacy prefix.\n"
    );
    assert!(
        one.tree_text("book/needs.md")
            .starts_with("---\nclass: canon\nsort: register\nstatus: \"accepted\"\n---\n"),
        "the source class mapped through value_map.class, `Stage` renamed and mapped"
    );
    let two = LayoutRun::of(TWO, &scratch);
    assert!(
        two.tree_text("log/decisions.md").starts_with(
            "---\nclass: decision\nstate: \"accepted\"\nsteward: \"ops\"\n---\n# Decisions\n"
        ),
        "{}",
        two.tree_text("log/decisions.md")
    );
    // A key whose key-map target the header already holds is kept as
    // written (a conflict), never dropped.
    let legacy = two.tree_text("log/OLD-0041.md");
    assert!(
        legacy.contains("\nstate: open\n") && legacy.contains("\nOutcome: "),
        "{legacy}"
    );
    assert_eq!(two.count("header/conflicts"), 1);
}

/// `text` with CRLF line ends and a BOM.
fn crlf_bom(text: &str) -> String {
    format!("\u{feff}{}", text.replace('\n', "\r\n"))
}

/// AC-05: CRLF and BOM copies give the same counts as the fixtures, and
/// the residue has no BOM and no CR before LF.
#[test]
fn crlf_and_bom_copies_give_equal_hashes() {
    let scratch = Scratch::new("layout-crlf");
    for name in LAYOUT_FIXTURES {
        let corpus = copy_of(name, &scratch, &format!("{}-crlf", name.replace('/', "-")));
        let mut converted = 0;
        for (path, bytes) in snapshot(&corpus) {
            if path.extension().is_some_and(|ext| ext == "md") {
                let file = corpus.join(&path);
                fs::write(&file, crlf_bom(&String::from_utf8(bytes).unwrap())).unwrap();
                converted += 1;
            }
        }
        assert!(converted >= 5, "{name}");
        let run = LayoutRun::new(
            &corpus,
            &scratch,
            &format!("{}-crlf-out", name.replace('/', "-")),
            &[],
        );
        let want = expected(name);
        for group in [
            "hashes", "titles", "fields", "task_box", "prose", "extents", "tree",
        ] {
            assert_eq!(run.result()[group], want[group], "{name}: {group}");
        }
        for (path, bytes) in snapshot(&run.tree()) {
            let text = String::from_utf8(bytes).unwrap();
            assert!(
                !text.starts_with('\u{feff}'),
                "{name}: BOM in {}",
                path.display()
            );
            assert!(!text.contains('\r'), "{name}: CR in {}", path.display());
        }
    }
}

/// AC-05: the CRLF/BOM copy's tree is byte-for-byte the plain fixture's
/// ("lossless = bytes", the residue LF-joined without BOM or CR).
#[test]
fn a_crlf_bom_copy_writes_the_same_tree_bytes() {
    let scratch = Scratch::new("layout-crlf-tree");
    for name in LAYOUT_FIXTURES {
        let plain = LayoutRun::of(name, &scratch);
        let corpus = copy_of(name, &scratch, &format!("{}-crlf", name.replace('/', "-")));
        for (path, bytes) in snapshot(&corpus) {
            if path.extension().is_some_and(|ext| ext == "md") {
                fs::write(
                    corpus.join(&path),
                    crlf_bom(&String::from_utf8(bytes).unwrap()),
                )
                .unwrap();
            }
        }
        let converted = LayoutRun::new(
            &corpus,
            &scratch,
            &format!("{}-crlf-out", name.replace('/', "-")),
            &[],
        );
        let differing: Vec<String> = snapshot(&plain.tree())
            .into_iter()
            .zip(snapshot(&converted.tree()))
            .filter(|((_, a), (_, b))| a != b)
            .map(|((path, _), _)| path.display().to_string())
            .collect();
        assert_eq!(
            files_under(&plain.tree()),
            files_under(&converted.tree()),
            "{name}"
        );
        assert!(
            differing.is_empty(),
            "{name}: differing tree files {differing:?}"
        );
    }
}

// --------------------------------------------------------------------- AC-06

/// The codes the attribution may call source-caused (the baseline's, Q7):
/// any but `file-name`, `id-scope` and `index-*`.
fn source_code(code: &str) -> bool {
    code != "file-name" && code != "id-scope" && !code.starts_with("index-")
}

/// AC-06: observe `clean`/`observed`, enforce with the baseline `clean`,
/// `stale` 0, no emitter finding; the baseline holds exactly the
/// source-caused errors; `index.files` = `tree.documents`.
#[test]
fn the_tree_checks_clean_with_the_source_debt_baseline() {
    let scratch = Scratch::new("layout-check");
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        let check = &run.result()["check"];
        assert_eq!(check["emitter_findings"], 0, "{name}: {check}");
        assert!(
            check["observe"] == "clean" || check["observe"] == "observed",
            "{name}: {check}"
        );
        assert_eq!(check["enforce"], "clean", "{name}");
        assert_eq!(check["stale"], 0, "{name}");
        assert_eq!(
            run.count("index/files"),
            run.count("tree/documents"),
            "{name}"
        );
        for (code, counts) in check["findings"].as_object().unwrap() {
            assert_eq!(counts["emitter"], 0, "{name}: {code}");
        }

        // The baseline: the distinct (code, path, subject) of the
        // source-caused errors, each with the spec's reason and expiry.
        let mut debt: BTreeSet<(String, String, String)> = BTreeSet::new();
        for finding in run.tree_findings() {
            if finding["cause"] == "source" && finding["severity"] == "error" {
                debt.insert((
                    finding["code"].as_str().unwrap().to_owned(),
                    finding["path"].as_str().unwrap().to_owned(),
                    finding["subject"].as_str().unwrap().to_owned(),
                ));
            }
        }
        let baseline: toml::Table =
            toml::from_str(&run.tree_text(".spec-debt.toml")).expect("baseline is TOML");
        let entries = baseline["debt"].as_array().expect("[[debt]]");
        let written: BTreeSet<(String, String, String)> = entries
            .iter()
            .map(|entry| {
                assert_eq!(entry["reason"].as_str(), Some("import-layout source debt"));
                assert_eq!(
                    entry["expires"].as_str(),
                    Some(layout_string(name, "debt_expires").as_str())
                );
                (
                    entry["code"].as_str().unwrap().to_owned(),
                    entry["path"].as_str().unwrap().to_owned(),
                    entry["subject"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(written, debt, "{name}");
        assert_eq!(
            entries.len() as u64,
            run.count("check/baseline/entries"),
            "{name}"
        );
        let sorted: Vec<_> = written.iter().cloned().collect();
        let in_file: Vec<_> = entries
            .iter()
            .map(|entry| {
                (
                    entry["code"].as_str().unwrap().to_owned(),
                    entry["path"].as_str().unwrap().to_owned(),
                    entry["subject"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(in_file, sorted, "{name}: one sorted [[debt]] per entry");
        for code in check["baseline"]["per_code"].as_object().unwrap().keys() {
            assert!(source_code(code), "{name}: {code} in the baseline");
        }
    }
}

/// AC-06: a legacy citation resolves through `aliases_from` (M2: without
/// it, red); a bare feature-scoped citation outside its document is a
/// source `mention-dangling`; links into or out of moved documents are the
/// layout's.
#[test]
fn legacy_citations_resolve_and_feature_mentions_outside_dangle_from_the_source() {
    let scratch = Scratch::new("layout-mentions");
    type Citation = (&'static str, &'static str, &'static str);
    let cases: [(&str, &[Citation], (&str, &str)); 2] = [
        (
            ONE,
            &[
                ("book/glossary.md", "OLDR-05", "NEED"),
                ("book/glossary.md", "LAW-09", "RULE"),
                ("book/plan.md", "OLDR-05", "NEED"),
            ],
            ("book/glossary.md", "CK-01"),
        ),
        (
            TWO,
            &[
                ("pages/overview.md", "OLD-0003", "DEC"),
                ("topics/billing.md", "OLD-0003", "DEC"),
            ],
            ("pages/overview.md", "CRT-0001"),
        ),
    ];
    for (name, legacy, feature) in cases {
        let run = LayoutRun::of(name, &scratch);
        let findings = run.tree_findings();
        // Core, reading the tree with the emitted scheme, resolves each
        // legacy citation as an alias of its Latin prefix.
        let scheme_file = run.tree().join("specengine.toml");
        let emitted = specengine_store::load_check(
            &specengine_store::NamedBytes::read("specengine.toml", &scheme_file),
            None,
        )
        .unwrap_or_else(|_| panic!("{name}: the emitted scheme loads"));
        for (path, citation, prefix) in legacy {
            let bytes = fs::read(run.tree().join(path)).unwrap();
            let parsed = specengine_core::parse(path, &bytes, &emitted.project.scheme);
            let resolved = parsed.links.iter().any(|link| {
                let json = serde_json::to_value(link).unwrap();
                json["dst"]["id"] == *citation && json["dst"]["alias_of"] == *prefix
            });
            assert!(
                resolved,
                "{name}: {path}: {citation} is no alias of {prefix}: {:?}",
                parsed.links
            );
        }
        for (path, citation, _) in legacy {
            assert!(run.tree_text(path).contains(citation), "{name}: {path}");
            let dangling: Vec<&Value> = findings
                .iter()
                .filter(|finding| {
                    finding["subject"] == *citation
                        || finding["message"]
                            .as_str()
                            .is_some_and(|message| message.contains(citation))
                })
                .collect();
            assert!(dangling.is_empty(), "{name}: {citation}: {dangling:?}");
        }
        let (path, id) = feature;
        let hit: Vec<&Value> = findings
            .iter()
            .filter(|finding| finding["path"] == path && finding["subject"] == id)
            .collect();
        assert_eq!(hit.len(), 1, "{name}: {findings:?}");
        assert_eq!(hit[0]["code"], "mention-dangling");
        assert_eq!(hit[0]["cause"], "source");
        let layout_links: Vec<&Value> = findings
            .iter()
            .filter(|finding| finding["cause"] == "layout")
            .collect();
        assert!(!layout_links.is_empty(), "{name}: a moved link");
        for finding in layout_links {
            let code = finding["code"].as_str().unwrap();
            assert!(
                code == "link-dangling" || code == "link-anchor",
                "{finding}"
            );
        }
    }
}

// --------------------------------------------------------------------- AC-07

/// AC-07: two runs write identical tree and detail bytes (`index.db`
/// aside); the corpus is untouched and nothing is written under `HOME`.
#[test]
fn two_runs_write_identical_bytes_and_touch_nothing_else() {
    let scratch = Scratch::new("layout-determinism");
    let home = scratch.join("home");
    fs::create_dir_all(&home).unwrap();
    for name in LAYOUT_FIXTURES {
        let corpus_before = snapshot(&fixture_dir(name));
        let mut runs = Vec::new();
        for round in ["a", "b"] {
            let out = scratch.join(&format!("{}-{round}", name.replace('/', "-")));
            let output = eval()
                .env("HOME", &home)
                .args([
                    "layout",
                    "--pilot",
                    fixture_dir(name).to_str().unwrap(),
                    "--out",
                    out.to_str().unwrap(),
                    "--today",
                    TODAY,
                ])
                .output()
                .expect("runs");
            let envelope = envelope(&output);
            let mut bytes = snapshot(&out.join("layout/pilot"));
            bytes.retain(|path, _| !path.to_string_lossy().starts_with("index.db"));
            runs.push((without_ms(&envelope["result"]), bytes));
            let top: Vec<String> = fs::read_dir(&out)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            assert_eq!(top, ["layout"], "{name}: writes only under <out>/layout/");
        }
        assert_eq!(runs[0].0, runs[1].0, "{name}: stdout");
        assert!(runs[0].1.len() > 10, "{name}");
        let keys: Vec<_> = runs[0].1.keys().collect();
        assert_eq!(keys, runs[1].1.keys().collect::<Vec<_>>(), "{name}");
        for (path, bytes) in &runs[0].1 {
            assert!(
                runs[1].1.get(path) == Some(bytes),
                "{name}: {} differs between runs",
                path.display()
            );
        }
        assert_eq!(
            snapshot(&fixture_dir(name)),
            corpus_before,
            "{name}: corpus"
        );
    }
    assert!(
        fs::read_dir(&home).unwrap().next().is_none(),
        "nothing under HOME"
    );
    assert_fixtures_status_clean();
}

// --------------------------------------------------------------------- AC-08

/// A layout fixture's census config with its `[layout]` replaced by
/// `body`; returns the text and the 1-based line of `body`'s first line.
fn with_layout(name: &str, body: &str) -> (String, usize) {
    let text = fs::read_to_string(fixture_config(name)).unwrap();
    let head = &text[..text.find("[layout]\n").expect("[layout]")];
    let first = head.lines().count() + 2;
    (format!("{head}[layout]\n{body}"), first)
}

/// An invalid `[layout]` body, and the offset of the offending line in it.
fn bad_layouts() -> Vec<(&'static str, String, usize)> {
    vec![
        ("unknown key", "colour = \"red\"\n".to_owned(), 0),
        ("wrong type", "records = 3\n".to_owned(), 0),
        ("records absolute", "records = \"/abs\"\n".to_owned(), 0),
        (
            "records leaves the root",
            "records = \"../out\"\n".to_owned(),
            0,
        ),
        ("records with a dot", "records = \"a/./b\"\n".to_owned(), 0),
        ("records empty", "records = \"\"\n".to_owned(), 0),
        (
            "records equal to features",
            "records = \"same\"\nfeatures = \"same\"\n".to_owned(),
            1,
        ),
        (
            "features nested in records",
            "records = \"rec\"\nfeatures = \"rec/feat\"\n".to_owned(),
            1,
        ),
        ("slug bad", "slug = '('\n".to_owned(), 0),
        (
            "slug matches empty",
            "slug = '^(?P<slug>.*)$'\n".to_owned(),
            0,
        ),
        (
            "slug without the group",
            "slug = '^p/([a-z]+)\\.md$'\n".to_owned(),
            0,
        ),
        (
            "record_class outside four",
            "record_class = \"memo\"\n".to_owned(),
            0,
        ),
        (
            "classes class outside four",
            "classes = [\n  { glob = \"x/**\", class = \"memo\" },\n]\n".to_owned(),
            1,
        ),
        (
            "classes empty glob",
            "classes = [\n  { glob = \"\", class = \"canon\" },\n]\n".to_owned(),
            1,
        ),
        (
            "classes unknown key",
            "classes = [\n  { glob = \"x/**\", class = \"canon\", colour = 1 },\n]\n".to_owned(),
            1,
        ),
        ("task_box_key empty", "task_box_key = \"\"\n".to_owned(), 0),
        (
            "task_box_key padded",
            "task_box_key = \" done\"\n".to_owned(),
            0,
        ),
        ("task_box_key id", "task_box_key = \"id\"\n".to_owned(), 0),
        (
            "task_box_key class",
            "task_box_key = \"class\"\n".to_owned(),
            0,
        ),
        (
            "task_box_key title",
            "task_box_key = \"title\"\n".to_owned(),
            0,
        ),
        (
            "task_box_key aliases",
            "task_box_key = \"aliases\"\n".to_owned(),
            0,
        ),
        (
            "debt_expires bad",
            "debt_expires = \"2026-02-30\"\n".to_owned(),
            0,
        ),
        (
            "debt_expires words",
            "debt_expires = \"tomorrow\"\n".to_owned(),
            0,
        ),
        (
            "targets key lower case",
            "[layout.targets]\nrule = \"section\"\n".to_owned(),
            1,
        ),
        (
            "targets key not Latin",
            "[layout.targets]\n\"\u{0420}\u{0423}\" = \"section\"\n".to_owned(),
            1,
        ),
        (
            "targets value",
            "[layout.targets]\nRULE = \"page\"\n".to_owned(),
            1,
        ),
        (
            "targets feature prefix file",
            "[layout.targets]\nCK = \"file\"\n".to_owned(),
            1,
        ),
    ]
}

fn assert_refused(output: &Output, out: &Path, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: {}",
        stderr(output)
    );
    assert!(output.stdout.is_empty(), "{context}: stdout");
    assert!(!out.exists(), "{context}: --out created");
    assert!(!stderr(output).contains("panicked"), "{context}");
}

/// AC-08: each bad `[layout]` is refused by `census`, `import` and
/// `layout` at `<config>:<line>`, nothing written (M:
/// `deny_unknown_fields` off `[layout]` → the unknown key passes: red).
#[test]
fn bad_layout_keys_are_refused_at_their_line_by_census_import_and_layout() {
    let scratch = Scratch::new("layout-bad");
    let pilot = fixture_dir(ONE);
    let mut failures = Vec::new();
    for (index, (what, body, offset)) in bad_layouts().into_iter().enumerate() {
        let (text, first) = with_layout(ONE, &body);
        let config = scratch.join(&format!("bad-{index}.toml"));
        fs::write(&config, &text).unwrap();
        let config_arg = config.to_str().unwrap();
        let line = first + offset;
        for measurement in ["census", "import", "layout"] {
            let out = scratch.join(&format!("out-{index}-{measurement}"));
            let mut args = vec![
                measurement,
                "--pilot",
                pilot.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--config",
                config_arg,
            ];
            if measurement == "layout" {
                args.extend(["--today", TODAY]);
            }
            let output = run(&args);
            let wanted = format!("{config_arg}:{line}:");
            if output.status.code() != Some(2)
                || !stderr(&output).contains(&wanted)
                || out.exists()
                || !output.stdout.is_empty()
            {
                failures.push(format!(
                    "{measurement} / {what}: exit {:?}, want {wanted}; stderr: {}",
                    output.status.code(),
                    stderr(&output).trim()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "expected exit 2 at <config>:<line>, nothing written:\n{}",
        failures.join("\n")
    );
}

/// AC-08: `census` and `import` accept a valid `[layout]` and ignore it:
/// `import-one` with one added prints its `expected.json`.
#[test]
fn census_and_import_accept_and_ignore_a_valid_layout() {
    let scratch = Scratch::new("layout-accepted");
    let base = fs::read_to_string(fixture_config("import-one")).unwrap();
    let config = scratch.join("with-layout.toml");
    fs::write(
        &config,
        format!(
            "{base}[layout]\nrecords = \"rec\"\nfeatures = \"feat\"\nrecord_class = \"decision\"\nclasses = [{{ glob = \"spec/**\", class = \"canon\" }}]\ntask_box_key = \"done\"\ndebt_expires = \"2999-01-01\"\ntargets = {{ REQ = \"section\" }}\n"
        ),
    )
    .unwrap();
    let import = ImportRun::new(
        &fixture_dir("import-one"),
        &scratch,
        "import-with-layout",
        &["--config", config.to_str().unwrap()],
    );
    assert_eq!(
        without_ms(&import.result),
        read_json(&fixture_dir("import-one").join("expected.json"))
    );
    let plain = measure(
        "census",
        &fixture_dir("import-one"),
        &scratch.join("census-plain"),
        &[],
    );
    let with = measure(
        "census",
        &fixture_dir("import-one"),
        &scratch.join("census-with-layout"),
        &["--config", config.to_str().unwrap()],
    );
    assert_eq!(without_ms(&with["result"]), without_ms(&plain["result"]));
    for name in LAYOUT_FIXTURES {
        for measurement in ["census", "import"] {
            let envelope = measure(
                measurement,
                &fixture_dir(name),
                &scratch.join(&format!("{}-{measurement}", name.replace('/', "-"))),
                &[],
            );
            assert_eq!(envelope["measurement"], measurement, "{name}");
        }
    }
}

/// AC-08: the start refusals of `layout` (a `targets` key the scheme
/// lacks, a prefix the scheme scopes to a feature set to `file`,
/// `debt_expires` before `--today`, a bad `--today`, `--out` under the
/// corpus): exit 2, `--out` not created.
#[test]
fn layout_start_refusals_write_nothing() {
    let scratch = Scratch::new("layout-start");
    let corpus = copy_of(ONE, &scratch, "corpus");
    let scheme = corpus.join("specengine.toml");
    edit(
        &scheme,
        "CK   = { kind = \"criterion\",   width = 2, scope = \"feature\" }\n",
        "CK   = { kind = \"criterion\",   width = 2, scope = \"feature\" }\nXF   = { kind = \"criterion\",   width = 2, scope = \"feature\" }\n",
    );
    let cases: Vec<(&str, &str, usize)> = vec![
        (
            "targets key the scheme lacks",
            "[layout.targets]\nNOPE = \"section\"\n",
            1,
        ),
        (
            "scheme feature-scoped file",
            "[layout.targets]\nXF = \"file\"\n",
            1,
        ),
        (
            "debt_expires before today",
            "debt_expires = \"2026-10-03\"\n",
            0,
        ),
    ];
    for (index, (what, body, offset)) in cases.into_iter().enumerate() {
        let (text, first) = with_layout(ONE, body);
        let config = scratch.join(&format!("start-{index}.toml"));
        fs::write(&config, text).unwrap();
        let out = scratch.join(&format!("start-out-{index}"));
        let output = run(&[
            "layout",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--today",
            TODAY,
        ]);
        assert_refused(&output, &out, what);
        let wanted = format!("{}:{}:", config.display(), first + offset);
        assert!(
            stderr(&output).contains(&wanted),
            "{what}: want {wanted}: {}",
            stderr(&output)
        );
    }
    // The same debt date is accepted on its own day.
    let (text, _) = with_layout(ONE, "debt_expires = \"2026-10-04\"\n");
    let config = scratch.join("start-today.toml");
    fs::write(&config, text).unwrap();
    LayoutRun::new(
        &corpus,
        &scratch,
        "start-today-out",
        &["--config", config.to_str().unwrap()],
    );

    for bad in ["2026-13-01", "2026-02-29", "04.10.2026", ""] {
        let out = scratch.join("bad-today");
        let output = run(&[
            "layout",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            bad,
        ]);
        assert_refused(&output, &out, &format!("--today {bad:?}"));
    }

    let inside = corpus.join("scratch-out");
    let output = run(&[
        "layout",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        inside.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &inside, "--out under the corpus");
}

/// The second convention's scheme with its index output moved onto a
/// walked document of the tree.
fn index_on_a_tree_document(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let corpus = copy_of(TWO, scratch, "index-taken");
    let scheme = corpus.join("specengine.toml");
    let text = fs::read_to_string(&scheme).unwrap();
    let text = replaced(
        &text,
        "index = \"pages/index.md\"",
        "index = \"pages/overview.md\"",
    );
    let text = replaced(
        &text,
        "writes  = [\"pages/index.md\"]",
        "writes  = [\"pages/overview.md\"]",
    );
    fs::write(&scheme, text).unwrap();
    (corpus, scheme)
}

/// AC-08: an index output path a tree document takes refuses the run,
/// naming the scheme; nothing written.
#[test]
fn an_index_output_held_by_the_tree_is_refused() {
    let scratch = Scratch::new("layout-index-taken");
    let (corpus, scheme) = index_on_a_tree_document(&scratch);
    let out = scratch.join("out");
    let output = run(&[
        "layout",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &out, "index output on a tree document");
    let scheme_name = scheme.canonicalize().unwrap().display().to_string();
    assert!(
        stderr(&output).contains(&scheme_name) || stderr(&output).contains("specengine.toml"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("pages/overview.md"),
        "{}",
        stderr(&output)
    );
}

/// AC-08 ("each refusal → exit 2 at `<config>:<line>`"): the index-output
/// refusal names the line of `[paths] index` in the scheme.
#[test]
fn the_index_output_refusal_names_its_line() {
    let scratch = Scratch::new("layout-index-line");
    let (corpus, scheme) = index_on_a_tree_document(&scratch);
    let line = fs::read_to_string(&scheme)
        .unwrap()
        .lines()
        .position(|line| line.starts_with("index = "))
        .unwrap()
        + 1;
    let out = scratch.join("out");
    let output = run(&[
        "layout",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &out, "index output on a tree document");
    assert!(
        stderr(&output).contains(&format!("specengine.toml:{line}:")),
        "want specengine.toml:{line}: in {}",
        stderr(&output)
    );
}

// --------------------------------------------------------------------- AC-09

/// AC-09: `census` on `corpus-mini`, `import` on `import-one` and
/// `import-two` print their unchanged `expected.json`; `import`'s stdout
/// has no `extent`, its `records.json` an `extent` per record; the
/// fixtures are untouched (M: `extent` on `import` stdout → red).
#[test]
fn census_and_import_outputs_are_unchanged() {
    let scratch = Scratch::new("layout-unchanged");
    let census = measure(
        "census",
        &repository_root().join("fixtures/corpus-mini"),
        &scratch.join("census"),
        &[],
    );
    let want = read_json(&repository_root().join("fixtures/corpus-mini/expected.json"));
    for (key, value) in want.as_object().unwrap() {
        assert_eq!(
            &census["result"][key.as_str()],
            value,
            "corpus-mini result.{key}"
        );
    }
    for name in FIXTURES {
        let out = scratch.join(name);
        let output = run(&[
            "import",
            "--pilot",
            fixture_dir(name).to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        let envelope = envelope(&output);
        assert_eq!(
            without_ms(&envelope["result"]),
            read_json(&fixture_dir(name).join("expected.json")),
            "{name}"
        );
        let stdout = String::from_utf8(output.stdout.clone()).unwrap();
        assert!(!stdout.contains("extent"), "{name}: extent on stdout");
        let records = read_json(&out.join("import/pilot/records.json"));
        let records = records.as_array().unwrap();
        assert!(!records.is_empty());
        for record in records {
            let extent = record["extent"]
                .as_array()
                .unwrap_or_else(|| panic!("{record}"));
            assert_eq!(extent.len(), 2, "{record}");
            let (first, last) = (extent[0].as_u64().unwrap(), extent[1].as_u64().unwrap());
            assert!(first >= 1 && first <= last, "{record}");
            assert!(
                first <= record["line"].as_u64().unwrap() || record["form"] == "document",
                "{record}"
            );
        }
    }
    for fixture in [
        "fixtures/corpus-mini",
        "fixtures/import-one",
        "fixtures/import-two",
    ] {
        assert_eq!(
            git_status(&repository_root(), fixture),
            "",
            "{fixture} changed"
        );
    }
}

// --------------------------------------------------------------------- AC-10

/// Whether a stdout key path is in the whitelist of "stdout and detail";
/// `*` stands for a check code.
fn whitelisted(path: &str) -> bool {
    const FIXED: [&str; 51] = [
        "before/documents",
        "before/definitions",
        "before/duplicates",
        "before/hyphenless/definitions",
        "before/hyphenless/mentions",
        "tree/documents",
        "tree/record_files",
        "tree/feature_documents",
        "tree/moved",
        "tree/reshaped",
        "hashes/matched",
        "hashes/mismatched",
        "hashes/missing",
        "hashes/extra",
        "titles/matched",
        "titles/mismatched",
        "fields/matched",
        "fields/mismatched",
        "task_box/carried",
        "task_box/dropped",
        "prose/documents",
        "prose/mismatched",
        "extents/total",
        "extents/residue",
        "header/documents",
        "header/unparseable",
        "header/keys_dropped",
        "header/conflicts",
        "header/last_row_comment",
        "reasons/prefix_unknown",
        "reasons/slug",
        "reasons/path_taken",
        "reasons/section_fields",
        "reasons/header_unparseable",
        "reasons/reader_boundary",
        "reasons/unexplained",
        "check/observe",
        "check/enforce",
        "check/stale",
        "check/emitter_findings",
        "check/baseline/entries",
        "dangling/mentions/before",
        "dangling/mentions/after",
        "dangling/links/before",
        "dangling/links/after",
        "index/files",
        "index/nodes",
        "index/links",
        "index/full_ms",
        "code/moved_cited",
        "code/citations_to_moved",
    ];
    if FIXED.contains(&path) || path == "detail/diagnostics" || path == "detail/layout_ms" {
        return true;
    }
    let is_code = |code: &str| {
        !code.is_empty()
            && code
                .split('-')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_lowercase()))
    };
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["check", "findings", code, cause] => {
            is_code(code) && ["before", "source", "layout", "emitter"].contains(cause)
        }
        ["check", "baseline", "per_code", code] => is_code(code),
        _ => false,
    }
}

fn leaves(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}/{key}")
                };
                leaves(value, &path, out);
            }
        }
        other => out.push((prefix.to_owned(), other.clone())),
    }
}

/// AC-10: stdout is the whitelist (counts and verdicts), the seven
/// reasons always present, per-code keys only for non-zero codes, and no
/// ID, path or file name of the corpus or the tree (M2: an ID or path on
/// stdout → red).
#[test]
fn stdout_is_the_whitelist_and_names_no_id_or_path() {
    let scratch = Scratch::new("layout-whitelist");
    for name in LAYOUT_FIXTURES {
        let run = LayoutRun::of(name, &scratch);
        let top: BTreeSet<&str> = run
            .envelope
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            top,
            BTreeSet::from(["label", "measurement", "result", "versions", "wall_ms"]),
            "{name}"
        );
        let mut found = Vec::new();
        leaves(run.result(), "", &mut found);
        for (path, value) in &found {
            assert!(whitelisted(path), "{name}: {path} is not whitelisted");
            if path == "check/observe" || path == "check/enforce" {
                assert!(value.is_string(), "{name}: {path}");
            } else {
                assert!(value.is_u64(), "{name}: {path} = {value}");
            }
        }
        for reason in REASONS {
            assert!(
                found
                    .iter()
                    .any(|(path, _)| *path == format!("reasons/{reason}")),
                "{name}: reasons/{reason} missing"
            );
        }
        for (code, counts) in run.result()["check"]["findings"].as_object().unwrap() {
            let total: u64 = counts
                .as_object()
                .unwrap()
                .values()
                .filter_map(Value::as_u64)
                .sum();
            assert!(total > 0, "{name}: {code} printed with zero counts");
        }

        // No corpus string: IDs, paths and file names of the map.
        let mut strings: BTreeSet<String> = BTreeSet::new();
        for entry in run.definitions() {
            for key in ["id", "path", "after"] {
                if let Some(text) = entry[key].as_str() {
                    strings.insert(text.to_owned());
                }
            }
        }
        for document in run.documents() {
            for key in ["source", "after"] {
                strings.insert(document[key].as_str().unwrap().to_owned());
            }
        }
        for path in files_under(&run.tree()) {
            strings.insert(path.rsplit('/').next().unwrap().to_owned());
            strings.insert(path);
        }
        let stdout = run.stdout();
        let leaked: Vec<&String> = strings
            .iter()
            .filter(|text| {
                let shaped = text.contains(['/', '.']) || text.chars().any(|c| c.is_ascii_digit());
                stdout.contains(&format!("\"{text}\""))
                    || (shaped && stdout.contains(text.as_str()))
            })
            .collect();
        assert!(
            leaked.is_empty(),
            "{name}: corpus strings on stdout: {leaked:?}"
        );
        assert!(strings.len() > 20, "{name}");
    }
}

// -------------------------------------------------------- edges of the rules

/// A copy of the first convention whose `NEED` rows are reshaped into
/// sections of their document (`targets.NEED = "section"`).
fn rows_as_sections(scratch: &Scratch, as_name: &str) -> PathBuf {
    let corpus = copy_of(ONE, scratch, as_name);
    edit(
        &corpus.join("census.toml"),
        "targets = { RULE = \"section\", ASK = \"file\" }",
        "targets = { RULE = \"section\", ASK = \"file\", NEED = \"section\" }",
    );
    corpus
}

/// "Reshaped": a field a heading attribute cannot carry (a blank) is a
/// `section_fields` miss, a field mismatch; nothing else changes. (The
/// `## Duplicates` table is cut from the copy so that only the `# Needs`
/// rows are reshaped; the nesting is the next test's.)
#[test]
fn a_field_with_a_blank_cannot_be_a_heading_attribute() {
    let scratch = Scratch::new("layout-section-fields");
    let corpus = rows_as_sections(&scratch, "corpus");
    edit(
        &corpus.join("book/needs.md"),
        "| NEED-02 | The engine keeps every byte it reads. | low | store |",
        "| NEED-02 | The engine keeps every byte it reads. | very low | store |",
    );
    edit(
        &corpus.join("book/needs.md"),
        "## Duplicates\n\n| Code | Wording |\n|------|---------|\n| NEED-02 | A second definition of an existing need. |\n\n",
        "",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert_eq!(run.count("reasons/section_fields"), 1, "{}", run.result());
    assert_eq!(run.count("reasons/unexplained"), 0, "{}", run.result());
    assert_eq!(run.count("fields/mismatched"), 1);
    assert_eq!(run.count("hashes/mismatched"), 0);
    assert_eq!(run.count("prose/mismatched"), 0);
    let needs = run.tree_text("book/needs.md");
    assert!(
        needs.contains("{#NEED-01 weight=high Area=input}"),
        "fields free of blanks are heading attributes:\n{needs}"
    );
    let entry = run.definition("NEED-02", 13);
    assert_eq!(entry["reason"], "section_fields");
}

/// "Reshaped" (Q4 placement, T1): rows of `# Needs` (n = 1, level 2) and a
/// row of its subsection `## Duplicates` (n = 2, level 3) follow the same
/// last residue line (the document's end). Written in source order, the
/// level-3 block would land inside the preceding level-2 block's section
/// (iteration 1: a hash mismatch, `unexplained`); blocks of the deeper
/// section come first, so every reshaped block stays its own section.
#[test]
fn reshaped_blocks_of_nested_sections_ending_together_stay_apart() {
    let scratch = Scratch::new("layout-nested-blocks");
    let corpus = rows_as_sections(&scratch, "corpus");
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let missed: Vec<Value> = run
        .records()
        .into_iter()
        .filter(|record| record["hash"] != "matched")
        .collect();
    assert!(
        missed.is_empty(),
        "{missed:?}\n{}",
        run.tree_text("book/needs.md")
    );
    assert_eq!(run.count("hashes/mismatched"), 0);
    assert_eq!(run.count("reasons/unexplained"), 0);
    assert_eq!(run.count("tree/reshaped"), 10, "RULE 2, CK 4, NEED 4");
}

/// "Verifier": a heading only the import scanner reads (inside an
/// unclosed `<pre>` block, which core keeps as HTML to the end) places the
/// reshaped block where core reads no heading; the miss is
/// `reader_boundary`, not `unexplained`.
#[test]
fn a_heading_only_the_scanner_reads_is_a_reader_boundary() {
    let scratch = Scratch::new("layout-reader-boundary");
    let corpus = copy_of(ONE, &scratch, "corpus");
    fs::write(
        corpus.join("book/notes.md"),
        "# Notes\n\n- **RULE-03**: a rule before a pre block\n\n<pre>\n## Inside pre\nmore text\n",
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let record = run
        .records()
        .into_iter()
        .find(|record| record["id"] == "RULE-03")
        .expect("the rule");
    assert_eq!(record["hash"], "missing", "{record}");
    assert_eq!(record["reason"], "reader_boundary", "{record}");
    assert_eq!(run.count("reasons/reader_boundary"), 1);
    assert_eq!(run.count("reasons/unexplained"), 0);
}

/// "Headers": a field table whose last row opens a comment the next line
/// continues is counted `last_row_comment`; the header still carries
/// every key.
#[test]
fn a_last_row_comment_of_the_field_table_is_counted() {
    let scratch = Scratch::new("layout-last-row");
    let corpus = copy_of(TWO, &scratch, "corpus");
    edit(
        &corpus.join("log/decisions.md"),
        "| Owner | ops |\n",
        "| Owner | ops <!-- the owner |\nchanges each year -->\n",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert_eq!(run.count("header/last_row_comment"), 1);
    assert_eq!(run.count("header/keys_dropped"), 0);
    assert_eq!(run.count("prose/mismatched"), 0);
}

/// "Rules and edge cases" A miss on a record whose after front-matter
/// fails is `header_unparseable`, counted in `header.unparseable`.
#[test]
fn a_document_record_with_bad_yaml_is_header_unparseable() {
    let scratch = Scratch::new("layout-unparseable");
    let corpus = copy_of(ONE, &scratch, "corpus");
    fs::write(
        corpus.join("book/plan.md"),
        "---\nSerial: NEED-30\nbad: [unclosed\n---\n# Plan\n\nA document record whose header is not YAML.\n",
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let record = run
        .records()
        .into_iter()
        .find(|record| record["id"] == "NEED-30")
        .expect("the document record");
    assert_ne!(record["hash"], "matched", "{record}");
    assert_eq!(record["reason"], "header_unparseable", "{record}");
    assert_eq!(run.count("reasons/header_unparseable"), 1);
    assert!(run.count("header/unparseable") >= 1);
    assert_eq!(run.count("reasons/unexplained"), 0);
}

/// Rule S on a look-alike: a `{#ID}` section written with a Cyrillic
/// look-alike prefix stays in place with its Latin ID, the written form
/// becoming no alias of the file (ADR-0009); the generated text never sits
/// in a fixture.
#[test]
fn rule_s_fixes_a_look_alike_heading_in_place() {
    let scratch = Scratch::new("layout-look-alike");
    let corpus = copy_of(ONE, &scratch, "corpus");
    // Latin `RUL` and a Cyrillic Ie (U+0415) for `E`: mixed script, the
    // look-alike fixed by the import.
    let written = "RUL\u{0415}-11";
    fs::write(
        corpus.join("book/look.md"),
        format!(
            "# Look-alikes\n\n## A fixed heading {{#{written}}}\n\nThe section stays where it is.\n"
        ),
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let look = run.tree_text("book/look.md");
    assert!(look.contains("## A fixed heading {#RULE-11}\n"), "{look}");
    assert!(!look.contains('\u{0415}'), "{look}");
    let entry = run
        .definitions()
        .into_iter()
        .find(|entry| entry["id"] == "RULE-11")
        .expect("the look-alike definition");
    assert_eq!(entry["place"], "in-place");
    assert_eq!(run.count("hashes/mismatched"), 0);
    assert_eq!(run.count("prose/mismatched"), 0);
    assert_eq!(run.count("check/emitter_findings"), 0);
}

/// An empty corpus (configs, no document) is no panic: a run with zeros.
#[test]
fn an_empty_corpus_is_counted_not_a_panic() {
    let scratch = Scratch::new("layout-empty");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(corpus.join("book")).unwrap();
    for file in ["census.toml", "specengine.toml"] {
        fs::copy(fixture_dir(ONE).join(file), corpus.join(file)).unwrap();
    }
    let out = scratch.join("out");
    let output = run(&[
        "layout",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert!(!stderr(&output).contains("panicked"), "{}", stderr(&output));
    let envelope = envelope(&output);
    assert_eq!(envelope["result"]["before"]["definitions"], 0);
    assert_eq!(envelope["result"]["tree"]["documents"], 0);
    assert_eq!(envelope["result"]["hashes"]["matched"], 0);
}

// ------------------------------------------------- accepted for iteration 2

/// "Residue" (D8): a residue document with a non-`.md` extension is renamed
/// to `.md` and counted `moved`, so core's walk reads it and `index.files`
/// = `tree.documents`.
#[test]
fn a_non_md_residue_document_is_renamed_to_md_and_counted_moved() {
    let scratch = Scratch::new("layout-markdown");
    let corpus = copy_of(ONE, &scratch, "corpus");
    edit(
        &corpus.join("census.toml"),
        "roots = [\"book\"]\n",
        "roots = [\"book\"]\nextensions = [\"md\", \"markdown\"]\n",
    );
    fs::rename(
        corpus.join("book/glossary.md"),
        corpus.join("book/glossary.markdown"),
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let document = run
        .documents()
        .into_iter()
        .find(|document| document["source"] == "book/glossary.markdown")
        .expect("the .markdown document is walked");
    assert_eq!(document["after"], "book/glossary.md", "{document}");
    assert_eq!(document["moved"], true, "{document}");
    assert!(run.tree().join("book/glossary.md").is_file());
    assert!(!run.tree().join("book/glossary.markdown").exists());
    assert_eq!(run.count("tree/moved"), 2, "checkout and the glossary");
    assert_eq!(run.count("index/files"), run.count("tree/documents"));
    assert_eq!(run.count("prose/mismatched"), 0);
}

// ------------------------------------------- iteration 2 (D1-D8, T2, T4, T5)

/// A copy of a layout fixture with `files` (path, text) written into it.
fn copy_with(name: &str, scratch: &Scratch, as_name: &str, files: &[(&str, &str)]) -> PathBuf {
    let corpus = copy_of(name, scratch, as_name);
    for (path, text) in files {
        fs::write(corpus.join(path), text).unwrap();
    }
    corpus
}

/// The attributed tree findings with `code` on `path`.
fn findings_on(run: &LayoutRun, code: &str, path: &str) -> Vec<Value> {
    run.tree_findings()
        .into_iter()
        .filter(|finding| finding["code"] == code && finding["path"] == path)
        .collect()
}

/// D1 ("Extent residue"): each field table is one extent, the header's;
/// a row less its carried key and value cells leaves no letter or digit —
/// an empty-key continuation row and a third cell do, named in
/// `extents.json` (`id` null), while the prose check (which cuts the whole
/// table) and the header keys see nothing.
#[test]
fn a_field_table_row_s_words_outside_its_key_and_value_are_extent_residue() {
    let scratch = Scratch::new("layout-field-residue");
    let cases = [
        (
            "an empty-key continuation row",
            "| Owner | ops |\n|  | orphan value with words |\n",
            "orphan value with words",
        ),
        (
            "a third cell",
            "| Owner | ops | third cell words |\n",
            "third cell words",
        ),
    ];
    for (index, (what, row, residue)) in cases.into_iter().enumerate() {
        let corpus = copy_of(TWO, &scratch, &format!("corpus-{index}"));
        edit(&corpus.join("log/decisions.md"), "| Owner | ops |\n", row);
        let run = LayoutRun::new(&corpus, &scratch, &format!("out-{index}"), &[]);
        assert_eq!(run.count("extents/residue"), 1, "{what}: {}", run.result());
        assert_eq!(
            run.count("extents/total"),
            expected(TWO)["extents"]["total"].as_u64().unwrap(),
            "{what}: the field table stays one extent"
        );
        let detail = run.file("extents.json");
        let detail = detail.as_array().unwrap();
        assert_eq!(detail.len(), 1, "{what}: {detail:?}");
        assert_eq!(detail[0]["path"], "log/decisions.md", "{what}");
        assert_eq!(detail[0]["id"], Value::Null, "{what}: the header's extent");
        assert_eq!(detail[0]["residue"], residue, "{what}");
        assert_eq!(run.count("prose/mismatched"), 0, "{what}");
        assert_eq!(run.count("fields/mismatched"), 0, "{what}");
        assert_eq!(run.count("header/keys_dropped"), 0, "{what}");
        assert!(
            run.tree_text("log/decisions.md")
                .starts_with("---\nclass: decision\nstate: \"accepted\"\nsteward: \"ops\"\n---\n"),
            "{what}: {}",
            run.tree_text("log/decisions.md")
        );
    }
}

/// D1 ("Fields"): every value carried from a field table is read back by
/// core and compared with its cell or its `value_map` mapping
/// (`headers.json` `field_values`); `fields.matched` = the definitions'
/// matched fields + the matched field-table values. (A value changed on
/// disk is the tamper test `a_changed_field_table_value_is_one_mismatched_field`.)
#[test]
fn field_table_values_are_compared_with_their_cells_or_mappings() {
    let scratch = Scratch::new("layout-field-values");
    // Each source YAML-block value as core reads it under the before
    // scheme, its `key_map`/`value_map`/rule S form: checked by hand
    // against the fixtures (iteration 3, R1).
    type Values<'a> = &'a [(&'a str, &'a str)];
    let cases: [(&str, Values, Values); 2] = [
        (
            ONE,
            &[
                ("serial", "NEED-30"),
                ("status", "accepted"),
                ("owner", "team"),
            ],
            &[
                ("sort", "flow"),
                ("sort", "register"),
                ("status", "accepted"),
            ],
        ),
        (
            TWO,
            &[("state", "accepted"), ("steward", "ops")],
            &[
                ("type", "record"),
                ("state", "accepted"),
                ("id", "DEC-0041"),
                ("state", "open"),
                ("Outcome", "Agreed"),
                ("type", "page"),
            ],
        ),
    ];
    for (name, want, want_yaml) in cases {
        let run = LayoutRun::of(name, &scratch);
        let values = run.file("headers.json")["field_values"]
            .as_array()
            .expect("headers.json field_values")
            .clone();
        let of_form = |form: &str| -> Vec<(String, String, String)> {
            values
                .iter()
                .filter(|value| value["form"] == form)
                .map(|value| {
                    assert_eq!(value["matched"], true, "{name}: {value}");
                    let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
                    (text("key"), text("expected"), text("read"))
                })
                .collect()
        };
        let pairs = |want: Values| -> Vec<(String, String, String)> {
            want.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned(), (*value).to_owned()))
                .collect()
        };
        assert_eq!(of_form("field-table"), pairs(want), "{name}");
        assert_eq!(
            of_form("yaml"),
            pairs(want_yaml),
            "{name}: YAML-block values"
        );
        assert_eq!(
            values.len(),
            want.len() + want_yaml.len(),
            "{name}: every field value has a form"
        );
        let per_record: u64 = run
            .records()
            .iter()
            .map(|record| record["fields_matched"].as_u64().unwrap_or(0))
            .sum();
        assert_eq!(
            run.count("fields/matched"),
            per_record + values.len() as u64,
            "{name}: definitions' fields + field-table values"
        );
        assert_eq!(run.count("fields/mismatched"), 0, "{name}");
    }
}

/// Three moved `{#ID}` sections: the anchor block first, words after it,
/// an ATX closing sequence.
const TITLED_SECTIONS: &str = "# Titles\n\n## {#NEED-41} Anchor first title\n\nThe anchor comes first.\n\n## Big {#NEED-42 level=three} trailing words\n\nWords follow the block.\n\n## Closed {#NEED-43} ##\n\nA closing sequence ends the heading.\n";

/// D2 (AC-03, "Record file"): a moved section's title is its heading minus
/// the ATX marker (closing sequence included) and the `{…}` block, both
/// sides trimmed and joined by one space; it reads back equal to the title
/// the verifier derives from the source bytes; the heading line leaves no
/// residue; the block's other attribute is a field. (A title cut on disk is
/// the tamper test `a_title_cut_at_the_anchor_block_is_one_mismatched_title`,
/// a dropped attribute `a_dropped_heading_attribute_is_one_mismatched_field`.)
#[test]
fn a_moved_section_s_title_is_its_heading_less_the_marker_and_the_block() {
    let scratch = Scratch::new("layout-section-titles");
    let corpus = copy_with(
        ONE,
        &scratch,
        "corpus",
        &[("book/titles.md", TITLED_SECTIONS)],
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    for (id, title, attributes, body) in [
        (
            "NEED-41",
            "Anchor first title",
            "",
            "The anchor comes first.",
        ),
        (
            "NEED-42",
            "Big trailing words",
            "level: \"three\"\n",
            "Words follow the block.",
        ),
        (
            "NEED-43",
            "Closed",
            "",
            "A closing sequence ends the heading.",
        ),
    ] {
        assert_eq!(
            run.tree_text(&format!("book/atoms/NEED/{id}.md")),
            format!("---\nid: {id}\nclass: canon\ntitle: \"{title}\"\n{attributes}---\n\n{body}\n"),
            "{id}"
        );
        let record = run
            .records()
            .into_iter()
            .find(|record| record["id"] == id)
            .unwrap_or_else(|| panic!("{id} in records.json"));
        assert_eq!(record["title_matched"], true, "{record}");
        assert_eq!(record["hash"], "matched", "{record}");
        assert_eq!(record["fields_mismatched"], 0, "{record}");
    }
    let base = expected(ONE);
    assert_eq!(run.count("titles/mismatched"), 0);
    assert_eq!(
        run.count("titles/matched"),
        base["titles"]["matched"].as_u64().unwrap() + 3
    );
    assert_eq!(
        run.count("fields/matched"),
        base["fields"]["matched"].as_u64().unwrap() + 1,
        "level=three"
    );
    assert_eq!(run.count("fields/mismatched"), 0);
    assert_eq!(
        run.count("extents/residue"),
        0,
        "{:?}",
        run.file("extents.json")
    );
    assert_eq!(run.count("reasons/unexplained"), 0);
}

/// D3 (AC-05, "YAML"; "Attribution" `frontmatter-type`, Q1): a value carried
/// under a typed core key — renamed by `key_map` from a header or a field
/// table, a field-table column, a heading attribute — is written in core's
/// type where its text parses as it (`tier: 2`, `rev: 2`), else quoted
/// (`tier: "007"`, `rev: "high"`); core's `frontmatter-type` on a quoted
/// one and `tier-invalid` on a canonical but invalid one read back as the
/// corpus wrote it are the source's (baseline), never the emitter's.
#[test]
fn a_typed_core_key_is_written_in_core_s_type_and_its_findings_are_the_source_s() {
    let scratch = Scratch::new("layout-typed");
    let corpus = copy_with(
        ONE,
        &scratch,
        "corpus",
        &[
            (
                "book/tier-plain.md",
                "---\nTier: 2\n---\n# Plain tier\n\nA canonical tier.\n",
            ),
            (
                "book/tier-high.md",
                "---\nTier: 4\n---\n# High tier\n\nA tier canon lacks.\n",
            ),
            (
                "book/tier-odd.md",
                "| Attribute | Value |\n|-----------|-------|\n| Tier | 007 |\n\n# Odd tier\n\nA tier with leading zeros.\n",
            ),
            (
                "book/tier-table.md",
                "| Attribute | Value |\n|-----------|-------|\n| Tier | 1 |\n\n# Table tier\n\nA tier from a field table.\n",
            ),
            (
                "book/revised.md",
                "# Revised\n\n## Revised once {#NEED-44 rev=2}\n\nA section with a revision.\n",
            ),
        ],
    );
    edit(
        &corpus.join("census.toml"),
        "\"Weight\" = \"weight\"\n",
        "\"Weight\" = \"rev\"\n\"Tier\" = \"tier\"\n",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| NEED-02 | The engine keeps every byte it reads. | low |",
        "| NEED-02 | The engine keeps every byte it reads. | 3 |",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    for (path, line) in [
        ("book/tier-plain.md", "\ntier: 2\n"),
        ("book/tier-high.md", "\ntier: 4\n"),
        ("book/tier-odd.md", "\ntier: \"007\"\n"),
        ("book/tier-table.md", "\ntier: 1\n"),
        ("book/atoms/NEED/NEED-44.md", "\nrev: 2\n"),
        ("book/atoms/NEED/NEED-02.md", "\nrev: 3\n"),
        ("book/atoms/NEED/NEED-01.md", "\nrev: \"high\"\n"),
    ] {
        let text = run.tree_text(path);
        assert!(text.contains(line), "{path}: {line:?} not in:\n{text}");
    }
    let mistyped: Vec<(String, String)> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"] == "frontmatter-type")
        .map(|finding| {
            assert_eq!(finding["cause"], "source", "{finding}");
            (
                finding["path"].as_str().unwrap().to_owned(),
                finding["subject"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let want: Vec<(String, String)> = [
        ("book/atoms/NEED/NEED-01.md", "rev"),
        ("book/atoms/NEED/NEED-05.md", "rev"),
        ("book/tier-odd.md", "tier"),
    ]
    .iter()
    .map(|(path, key)| ((*path).to_owned(), (*key).to_owned()))
    .collect();
    assert_eq!(mistyped, want);
    let invalid = findings_on(&run, "tier-invalid", "book/tier-high.md");
    assert_eq!(invalid.len(), 1, "{:?}", run.tree_findings());
    assert_eq!(invalid[0]["cause"], "source", "Q1: {}", invalid[0]);
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
    assert_eq!(run.count("check/baseline/per_code/frontmatter-type"), 3);
    assert_eq!(run.count("check/baseline/per_code/tier-invalid"), 1);
    assert_eq!(
        run.count("fields/mismatched"),
        0,
        "the values read back as written"
    );
    assert_eq!(run.count("header/keys_dropped"), 0);
}

/// D5 ("YAML"): keys YAML would read as null or a bool (`null`, `True`,
/// `yes`, `On`, `y`, `NULL`, `off`) and `~` are quoted; each reads back as
/// the string it was, so every field matches.
#[test]
fn null_and_bool_like_keys_are_quoted_and_read_back_as_strings() {
    let scratch = Scratch::new("layout-null-keys");
    let corpus = copy_of(ONE, &scratch, "corpus");
    edit(
        &corpus.join("book/needs.md"),
        "| Code | Wording | Weight | Area |",
        "| Code | Wording | null | True |",
    );
    edit(
        &corpus.join("book/plan.md"),
        "| Keeper | team |\n",
        "| Keeper | team |\n| yes | sure |\n| On | lamp |\n| y | why |\n| NULL | none |\n| ~ | tilde |\n| off | dark |\n",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert_eq!(
        run.tree_text("book/atoms/NEED/NEED-01.md"),
        "---\nid: NEED-01\nclass: canon\n\"null\": \"high\"\n\"True\": \"input\"\n---\n\nThe engine reads a pipe \\| inside a cell.\n"
    );
    assert!(
        run.tree_text("book/plan.md").starts_with(
            "---\nid: NEED-30\nclass: canon\nserial: \"NEED-30\"\nstatus: \"accepted\"\nowner: \"team\"\n\"yes\": \"sure\"\n\"On\": \"lamp\"\n\"y\": \"why\"\n\"NULL\": \"none\"\n\"~\": \"tilde\"\n\"off\": \"dark\"\n---\n"
        ),
        "{}",
        run.tree_text("book/plan.md")
    );
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(
        run.count("fields/matched"),
        expected(ONE)["fields"]["matched"].as_u64().unwrap() + 6,
        "six more field-table values"
    );
    assert_eq!(run.count("header/unparseable"), 0);
    assert_eq!(run.count("reasons/unexplained"), 0);
    assert_eq!(run.count("extents/residue"), 0);
}

/// D6 ("Verifier": an in-place section at its own heading line, reshaped
/// nodes only among reshaped blocks): one ID twice in a document, once as a
/// list item reshaped into a section, once as a section that stays. Case 1:
/// the in-place section comes first in the tree, the item first in the
/// source (one ordinal pass in source order would swap them). Case 2: the
/// reshaped block comes first in the tree, before the in-place section
/// (an in-place pass taking the first untaken node would swap them). Each
/// finds its own node either way.
#[test]
fn a_reshaped_item_and_an_in_place_section_with_one_id_find_their_own_nodes() {
    let scratch = Scratch::new("layout-same-id");
    let cases = [
        (
            "the in-place section first in the tree",
            "A paragraph between the rules and the subsection.\n",
            "A paragraph between the rules and the subsection.\n\n### Again {#RULE-01}\n\nThe same rule defined again, as a section that stays.\n",
            "RULE-01",
            [
                "### Again {#RULE-01}\n",
                "### First rule {#RULE-01 ticked=false}\n",
            ],
            [(28, "section"), (33, "in-place")],
        ),
        (
            "the reshaped block first in the tree",
            "- **RULE-02**: The engine writes nothing outside its scratch.\n",
            "- **RULE-02**: The engine writes nothing outside its scratch.\n- **RULE-09**: A rule row under the number the legacy section takes.\n",
            "RULE-09",
            ["### RULE-09 {#RULE-09}\n", "## Old law {#RULE-09}\n"],
            [(30, "section"), (40, "in-place")],
        ),
    ];
    for (index, (what, from, to, id, order, want)) in cases.into_iter().enumerate() {
        let corpus = copy_of(ONE, &scratch, &format!("corpus-{index}"));
        edit(&corpus.join("book/needs.md"), from, to);
        let run = LayoutRun::new(&corpus, &scratch, &format!("out-{index}"), &[]);
        let needs = run.tree_text("book/needs.md");
        let first = needs
            .find(order[0])
            .unwrap_or_else(|| panic!("{what}: {needs}"));
        let second = needs
            .find(order[1])
            .unwrap_or_else(|| panic!("{what}: {needs}"));
        assert!(first < second, "{what}: {needs}");
        let found: Vec<(u64, String, String)> = run
            .records()
            .into_iter()
            .filter(|record| record["id"] == id)
            .map(|record| {
                (
                    record["line"].as_u64().unwrap(),
                    record["place"].as_str().unwrap().to_owned(),
                    record["hash"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let want: Vec<(u64, String, String)> = want
            .iter()
            .map(|(line, place)| (*line, (*place).to_owned(), "matched".to_owned()))
            .collect();
        assert_eq!(found, want, "{what}");
        assert_eq!(run.count("hashes/mismatched"), 0, "{what}");
        assert_eq!(run.count("hashes/extra"), 0, "{what}");
        assert_eq!(run.count("reasons/unexplained"), 0, "{what}");
        assert_eq!(run.count("titles/mismatched"), 0, "{what}");
        assert_eq!(run.count("prose/mismatched"), 0, "{what}");
        assert_eq!(
            run.count("check/emitter_findings"),
            0,
            "{what}: {}",
            run.result()
        );
    }
}

/// D7 ("Attribution"): `class-missing` where no `[layout] classes` glob
/// matches the after path is the source's (with a glob the layout writes
/// the class itself; its loss on disk is the emitter's: the tamper test
/// `class_missing_where_a_classes_glob_matches_is_the_emitter_s`).
#[test]
fn class_missing_without_a_matching_classes_glob_is_the_source_s() {
    let scratch = Scratch::new("layout-class-missing");
    let corpus = copy_of(ONE, &scratch, "corpus");
    edit(
        &corpus.join("census.toml"),
        ", { glob = \"book/**\", class = \"canon\" }",
        "",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert!(
        run.tree_text("book/glossary.md")
            .starts_with("# Glossary\n")
    );
    for path in ["book/glossary.md", "book/plan.md"] {
        let missing = findings_on(&run, "class-missing", path);
        assert_eq!(missing.len(), 1, "{path}: {:?}", run.tree_findings());
        assert_eq!(missing[0]["cause"], "source", "{}", missing[0]);
    }
    assert_eq!(run.count("check/emitter_findings"), 0);
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// D8 ("Residue"): a non-`.md` residue whose `.md` name a walked document
/// already holds keeps its name, with a layout diagnostic; nothing moves.
#[test]
fn a_non_md_residue_whose_md_name_is_taken_keeps_its_name_with_a_diagnostic() {
    let scratch = Scratch::new("layout-markdown-taken");
    let corpus = copy_with(
        ONE,
        &scratch,
        "corpus",
        &[(
            "book/glossary.markdown",
            "# Glossary twin\n\nA second glossary whose .md name is taken.\n",
        )],
    );
    edit(
        &corpus.join("census.toml"),
        "roots = [\"book\"]\n",
        "roots = [\"book\"]\nextensions = [\"md\", \"markdown\"]\n",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let documents = run.documents();
    let twin = documents
        .iter()
        .find(|document| document["source"] == "book/glossary.markdown")
        .expect("the .markdown document is walked");
    assert_eq!(twin["after"], "book/glossary.markdown", "{twin}");
    assert_eq!(twin["moved"], false, "{twin}");
    let original = documents
        .iter()
        .find(|document| document["source"] == "book/glossary.md")
        .expect("the .md document");
    assert_eq!(original["after"], "book/glossary.md");
    assert_eq!(
        run.tree_text("book/glossary.markdown"),
        "---\nclass: canon\n---\n# Glossary twin\n\nA second glossary whose .md name is taken.\n"
    );
    assert!(run.tree_text("book/glossary.md").contains("A bare CK-01"));
    assert_eq!(
        run.count("tree/moved"),
        expected(ONE)["tree"]["moved"].as_u64().unwrap()
    );
    let diagnostics = run.file("diagnostics.json")["layout"]
        .as_array()
        .unwrap()
        .clone();
    let taken: Vec<&Value> = diagnostics
        .iter()
        .filter(|entry| entry["path"] == "book/glossary.markdown")
        .collect();
    assert_eq!(taken.len(), 1, "{diagnostics:?}");
    assert!(
        taken[0]["message"]
            .as_str()
            .unwrap()
            .contains("book/glossary.md"),
        "{}",
        taken[0]
    );
    assert_eq!(
        run.count("detail/diagnostics"),
        expected(ONE)["detail"]["diagnostics"].as_u64().unwrap() + 1
    );
    assert_eq!(run.count("prose/mismatched"), 0);
}

/// A run of `layout` from `cwd` (relative spellings), the pilot variables
/// removed as `run` does.
fn run_in(cwd: &Path, args: &[&str]) -> Output {
    eval()
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("specengine-eval runs")
}

/// D4 / AC-08: the corpus under `--out` or `--out` under (or equal to) the
/// corpus, in absolute, `..` and relative spellings, is refused before
/// anything is written: exit 2, empty stdout, the corpus byte for byte as
/// before. The deadly case puts the corpus at `<out>/layout/pilot`, the
/// directory a run empties first.
#[test]
fn a_corpus_and_out_nested_either_way_are_refused_and_nothing_is_deleted() {
    let scratch = Scratch::new("layout-nested");
    let out = scratch.join("o");
    let corpus = out.join("layout").join("pilot");
    copy_dir(&fixture_dir(ONE), &corpus);
    let elsewhere = out.join("elsewhere").join("corpus");
    copy_dir(&fixture_dir(ONE), &elsewhere);
    let before = snapshot(&scratch.0);
    let corpus_arg = corpus.to_str().unwrap().to_owned();
    let up_two = format!("{corpus_arg}/../..");
    let under_dots = format!("{corpus_arg}/sub/../x");
    let cases: Vec<(&str, &Path, Vec<&str>)> = vec![
        (
            "the corpus at <out>/layout/pilot",
            &scratch.0,
            vec!["--pilot", &corpus_arg, "--out", out.to_str().unwrap()],
        ),
        (
            "--out spelled <corpus>/../..",
            &scratch.0,
            vec!["--pilot", &corpus_arg, "--out", &up_two],
        ),
        (
            "relative spellings",
            &scratch.0,
            vec!["--pilot", "o/layout/pilot", "--out", "o"],
        ),
        (
            "relative --out with ..",
            &corpus,
            vec!["--pilot", ".", "--out", "../.."],
        ),
        (
            "the corpus elsewhere under --out",
            &scratch.0,
            vec!["--pilot", elsewhere.to_str().unwrap(), "--out", "o"],
        ),
        (
            "--out equal to the corpus",
            &scratch.0,
            vec!["--pilot", &corpus_arg, "--out", &corpus_arg],
        ),
        (
            "--out under the corpus through ..",
            &scratch.0,
            vec!["--pilot", &corpus_arg, "--out", &under_dots],
        ),
    ];
    for (what, cwd, args) in cases {
        let mut full = vec!["layout"];
        full.extend(args);
        full.extend(["--today", TODAY]);
        let output = run_in(cwd, &full);
        assert_eq!(output.status.code(), Some(2), "{what}: {}", stderr(&output));
        assert!(output.stdout.is_empty(), "{what}: stdout");
        assert!(!stderr(&output).contains("panicked"), "{what}");
        assert!(
            stderr(&output).contains("nothing written"),
            "{what}: {}",
            stderr(&output)
        );
        assert_eq!(snapshot(&scratch.0), before, "{what}: a byte changed");
    }
    assert!(!corpus.join("x").exists() && !corpus.join("layout").exists());
}

/// D4 / AC-08: a symlinked `<out>/layout` or `<out>/layout/<label>` is
/// refused (exit 2): the link is neither followed for deletion or writes
/// nor removed, its target byte for byte as before.
#[cfg(unix)]
#[test]
fn a_symlinked_layout_directory_is_refused_and_neither_followed_nor_removed() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("layout-symlinks");
    let victim = scratch.join("victim");
    fs::create_dir_all(victim.join("pilot")).unwrap();
    fs::write(victim.join("keep.txt"), "kept\n").unwrap();
    fs::write(victim.join("pilot").join("keep.txt"), "kept too\n").unwrap();
    let before = snapshot(&victim);
    let corpus = fixture_dir(ONE);
    for (index, link_at) in ["layout", "layout/pilot"].into_iter().enumerate() {
        let out = scratch.join(&format!("out-{index}"));
        let link = out.join(link_at);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        let target = if link_at == "layout" {
            victim.clone()
        } else {
            victim.join("pilot")
        };
        symlink(&target, &link).unwrap();
        let output = run(&[
            "layout",
            "--pilot",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--today",
            TODAY,
        ]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{link_at}: {}",
            stderr(&output)
        );
        assert!(output.stdout.is_empty(), "{link_at}");
        assert!(
            stderr(&output).contains("symbolic link"),
            "{link_at}: {}",
            stderr(&output)
        );
        assert!(
            fs::symlink_metadata(&link).is_ok_and(|meta| meta.file_type().is_symlink()),
            "{link_at}: the link was removed"
        );
        assert_eq!(snapshot(&victim), before, "{link_at}: the target changed");
    }
}

/// T2 / AC-08: the refusal of a shard output a tree document takes names
/// the first scheme line quoting its path (a comment quoting it skipped).
#[test]
fn a_shard_output_refusal_names_the_first_line_quoting_it() {
    let scratch = Scratch::new("layout-shard-line");
    let corpus = copy_of(TWO, &scratch, "corpus");
    let scheme = corpus.join("specengine.toml");
    edit(
        &scheme,
        "[[generators]]\ncommand = \"spec export index\"\nwrites  = [\"pages/index.md\"]\nindex   = true\n",
        "# the shard \"pages/overview.md\" takes Tier 3\n[[generators]]\ncommand = \"spec export index\"\nwrites  = [\"pages/index.md\", \"pages/overview.md\"]\nindex   = true\nshards  = [{ path = \"pages/overview.md\", tier3 = true }]\n",
    );
    let line = fs::read_to_string(&scheme)
        .unwrap()
        .lines()
        .position(|line| line.starts_with("writes "))
        .unwrap()
        + 1;
    let out = scratch.join("out");
    let output = run(&[
        "layout",
        "--pilot",
        corpus.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--today",
        TODAY,
    ]);
    assert_refused(&output, &out, "a shard on a tree document");
    assert!(
        stderr(&output).contains(&format!(
            "specengine.toml:{line}: the index output `pages/overview.md`"
        )),
        "want specengine.toml:{line}: in {}",
        stderr(&output)
    );
}

/// T4 ("Headers"): a key kept as written because its `key_map` target is
/// already there keeps its value too — `value_map` applies only to a key
/// carried under the target.
#[test]
fn a_key_kept_as_written_after_a_collision_keeps_its_value_unmapped() {
    let scratch = Scratch::new("layout-collision-value");
    let run = LayoutRun::of(TWO, &scratch);
    assert_eq!(
        run.tree_text("log/OLD-0041.md"),
        "---\nclass: decision\naliases: [\"OLD-0041\"]\nid: DEC-0041\nstate: open\nOutcome: Agreed\n---\n# Retire the legacy numbers\n\nA legacy document record keeps its alias.\n"
    );
    assert!(
        run.tree_text("log/DEC-0040.md")
            .contains("\nstate: \"accepted\"\n"),
        "the same key without a collision is renamed and mapped"
    );
}

/// T5 ("Attribution", Q2): a link of a moved document whose target leaves
/// the tree (outside core's walk, so core reports nothing) while from the
/// source it named a walked document counts in `dangling.links.after`,
/// named in `findings.json` `left_tree`; a link dangling already in the
/// source is not one.
#[test]
fn a_link_leaving_the_tree_is_counted_layout_broken() {
    let scratch = Scratch::new("layout-left-tree");
    let run = LayoutRun::of(TWO, &scratch);
    let left = run.file("findings.json")["left_tree"].clone();
    assert_eq!(
        left,
        serde_json::json!([{
            "path": "topics/billing.md",
            "line": 10,
            "target": "../overview.md",
            "source": "pages/billing/index.md"
        }])
    );
    let core_links = run
        .tree_findings()
        .iter()
        .filter(|finding| finding["code"] == "link-dangling" || finding["code"] == "link-anchor")
        .count() as u64;
    assert_eq!(run.count("dangling/links/after"), core_links + 1);

    let corpus = copy_of(TWO, &scratch, "corpus");
    edit(
        &corpus.join("pages/billing/index.md"),
        "[the overview](../overview.md)",
        "[the overview](../missing.md)",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert_eq!(
        run.file("findings.json")["left_tree"],
        serde_json::json!([]),
        "dangling in the source already"
    );
    assert_eq!(run.count("dangling/links/before"), 1);
}

// ------------------------------------- iteration 3 (R1-R8, S1-S3)

/// R8 (AC-05): the layout's typed-key table is core's — the same keys in
/// the same order, each of the same type (the two enums are distinct
/// types, pinned by their variant names) — and core's reader dispatches on
/// exactly that table: under each key, a plain word, a flow list and an
/// empty flow mapping are mistyped or not as its type says, an integer
/// fits an integer key, no table key is `unknown-key`, a key outside the
/// table is.
#[test]
fn import_s_typed_key_table_is_core_s_and_core_reads_by_it() {
    use specengine_core::{KeyType, TYPED_KEYS};
    use specengine_model::{DiagnosticCode, IdScheme, PrefixSpec};

    let core: Vec<(String, String)> = TYPED_KEYS
        .iter()
        .map(|(key, kind)| ((*key).to_owned(), format!("{kind:?}")))
        .collect();
    let import: Vec<(String, String)> = specengine_import::layout::typed_keys()
        .into_iter()
        .map(|(key, kind)| (key, format!("{kind:?}")))
        .collect();
    assert_eq!(import, core, "import's typed keys vs core's, in order");
    assert_eq!(core.len(), 27);

    let scheme = IdScheme::new(vec![PrefixSpec::number("X", "x", 1)]).unwrap();
    let codes = |key: &str, value: &str| -> Vec<DiagnosticCode> {
        let text = format!("---\n{key}: {value}\n---\n# Probe\n");
        specengine_core::parse("probe.md", text.as_bytes(), &scheme)
            .diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.line == 2)
            .map(|diagnostic| diagnostic.code)
            .collect()
    };
    let mistyped =
        |key: &str, value: &str| codes(key, value).contains(&DiagnosticCode::FrontmatterType);
    for (key, kind) in TYPED_KEYS {
        let shape = (
            mistyped(key, "word"),
            mistyped(key, "[X-1]"),
            mistyped(key, "{}"),
        );
        let want = match kind {
            KeyType::Text | KeyType::Reference => (false, true, true),
            KeyType::Integer => (true, true, true),
            KeyType::List | KeyType::ReferenceList => (true, false, true),
            KeyType::Mapping => (true, true, false),
        };
        assert_eq!(
            shape, want,
            "{key}: {kind:?} (word, list, mapping mistyped)"
        );
        if kind == KeyType::Integer {
            assert!(!mistyped(key, "2"), "{key}: an integer");
        }
        for value in ["word", "[X-1]", "{}", "2"] {
            assert!(
                !codes(key, value).contains(&DiagnosticCode::UnknownKey),
                "{key}: {value} is typed"
            );
        }
    }
    assert!(codes("untyped", "word").contains(&DiagnosticCode::UnknownKey));
}

/// One `layout` run on a copy of fixture one with `census` edits
/// (from, to) applied to its census config and `files` written.
fn one_with(scratch: &Scratch, census: &[(&str, &str)], files: &[(&str, &str)]) -> LayoutRun {
    let corpus = copy_with(ONE, scratch, "corpus", files);
    for (from, to) in census {
        edit(&corpus.join("census.toml"), from, to);
    }
    LayoutRun::new(&corpus, scratch, "out", &[])
}

const KEY_MAP_ANCHOR: &str = "\"Weight\" = \"weight\"\n";

/// The `headers.json` `field_values` of `path`, as (key, expected, read,
/// matched).
fn values_of(run: &LayoutRun, path: &str) -> Vec<(String, String, String, bool)> {
    run.file("headers.json")["field_values"]
        .as_array()
        .expect("headers.json field_values")
        .iter()
        .filter(|value| value["path"] == path)
        .map(|value| {
            let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
            (
                text("key"),
                text("expected"),
                text("read"),
                value["matched"] == true,
            )
        })
        .collect()
}

/// R2 (AC-06 "Attribution", `frontmatter-type`): a typed core key's value
/// the layout carries verbatim — a literal block, a folded block, a tagged
/// scalar it does not rewrite — and core rejects is the source's: the
/// corpus wrote it so, compared and read back alike (R1); enforce stays
/// clean. The same keys written plain are retyped and raise nothing. (A
/// value core rejects only as the tree holds it is the tamper test
/// `a_typed_value_mistyped_in_the_tree_from_a_text_that_parses_is_the_emitter_s`.)
#[test]
fn a_verbatim_block_folded_or_tagged_typed_value_is_the_source_s_frontmatter_type() {
    let scratch = Scratch::new("layout-r2-verbatim");
    let run = one_with(
        &scratch,
        &[(
            KEY_MAP_ANCHOR,
            "\"Weight\" = \"weight\"\n\"Tier\" = \"tier\"\n\"Rev\" = \"rev\"\n\"Scope\" = \"scope\"\n",
        )],
        &[
            (
                "book/typed.md",
                "---\nScope: |\n  multi\n  line\nTier: >-\n  2\nRev: !!str 3\n---\n# Typed\n\nBlock, folded and tagged values.\n",
            ),
            (
                "book/typed-plain.md",
                "---\nTier: 2\nRev: \"3\"\nScope: alpha\n---\n# Typed plain\n\nValues the layout retypes.\n",
            ),
        ],
    );
    assert!(
        run.tree_text("book/typed.md").starts_with(
            "---\nclass: canon\nscope: |\n  multi\n  line\ntier: >-\n  2\nrev: !!str 3\n---\n"
        ),
        "carried verbatim: {}",
        run.tree_text("book/typed.md")
    );
    assert!(
        run.tree_text("book/typed-plain.md")
            .starts_with("---\nclass: canon\ntier: 2\nrev: 3\nscope: [\"alpha\"]\n---\n"),
        "{}",
        run.tree_text("book/typed-plain.md")
    );
    let mistyped = findings_on(&run, "frontmatter-type", "book/typed.md");
    let subjects: BTreeSet<&str> = mistyped
        .iter()
        .map(|finding| finding["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        subjects,
        BTreeSet::from(["rev", "scope", "tier"]),
        "{mistyped:?}"
    );
    for finding in &mistyped {
        assert_eq!(finding["cause"], "source", "{finding}");
    }
    assert!(findings_on(&run, "frontmatter-type", "book/typed-plain.md").is_empty());
    for path in ["book/typed.md", "book/typed-plain.md"] {
        let values = values_of(&run, path);
        assert_eq!(values.len(), 3, "{path}: {values:?}");
        assert!(values.iter().all(|value| value.3), "{path}: {values:?}");
    }
    assert_eq!(run.count("fields/mismatched"), 0);
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// Census edits and corpus files of the R3 cases: `Cites`, `Sees`, `Was`
/// renamed to the reference-list keys `refs`, `adrs`, `supersedes`; a
/// `value_map` turning `legacy` into two IDs.
const CITES_CENSUS: [(&str, &str); 2] = [
    (
        KEY_MAP_ANCHOR,
        "\"Weight\" = \"weight\"\n\"Cites\" = \"refs\"\n\"Sees\" = \"adrs\"\n\"Was\" = \"supersedes\"\n",
    ),
    (
        "[front_matter.value_map.class]\n",
        "[front_matter.value_map.refs]\n\"legacy\" = \"NEED-01, RULE-01\"\n[front_matter.value_map.class]\n",
    ),
];

const CITES_FILES: [(&str, &str); 5] = [
    (
        "book/cites.md",
        "---\nCites: NEED-01, NEED-02\nSees: RULE-01 , NEED-02\nWas: NEED-01\n---\n# Cites\n\n| Code | Wording | Cites |\n|------|---------|-------|\n| NEED-50 | Cites two needs. | NEED-01, NEED-02 |\n| NEED-51 | Cites in prose. | see the old notes |\n| NEED-53 | Cites one twice. | NEED-01, NEED-01, RULE-01 |\n",
    ),
    (
        "book/cites-prose.md",
        "---\nCites: NEED-02, prose words\n---\n# Cites in prose\n\nA header citing in prose.\n",
    ),
    (
        "book/cites-table.md",
        "| Attribute | Value |\n|-----------|-------|\n| Cites | RULE-01, NEED-02 |\n\n# Cites by a field table\n\nA field table citing two records.\n",
    ),
    (
        "book/cites-mapped.md",
        "---\nCites: legacy\n---\n# Cites by a mapped word\n\nA header citing through the value map.\n",
    ),
    (
        "book/cites-mapped-table.md",
        "| Attribute | Value |\n|-----------|-------|\n| Cites | legacy |\n\n# Cites by a mapped cell\n\nA field table citing through the value map.\n",
    ),
];

/// R3 (AC-05 "YAML", AC-06): a value carried under a reference-list key
/// (`refs`, `adrs`, `supersedes`) — a table cell, a header scalar, a
/// field-table row, a `value_map` mapping of either — is split at its
/// commas into a flow list of trimmed items and compared item-wise, a
/// repeated item kept; prose carried into one (a cell, a header scalar)
/// is the source's `unparsed-reference`. Nothing is the emitter's.
#[test]
fn reference_list_values_are_split_at_commas_and_prose_in_them_is_the_source_s() {
    let scratch = Scratch::new("layout-r3-refs");
    let run = one_with(&scratch, &CITES_CENSUS, &CITES_FILES);
    for (id, refs) in [
        ("NEED-50", "[\"NEED-01\", \"NEED-02\"]"),
        ("NEED-51", "[\"see the old notes\"]"),
        ("NEED-53", "[\"NEED-01\", \"NEED-01\", \"RULE-01\"]"),
    ] {
        let text = run.tree_text(&format!("book/atoms/NEED/{id}.md"));
        assert!(
            text.starts_with(&format!("---\nid: {id}\nclass: canon\nrefs: {refs}\n---\n")),
            "{id}: {text}"
        );
        let record = run
            .records()
            .into_iter()
            .find(|record| record["id"] == id)
            .unwrap_or_else(|| panic!("{id} in records.json"));
        assert_eq!(record["fields_matched"], 1, "{record}");
        assert_eq!(record["fields_mismatched"], 0, "{record}");
    }
    for (path, header) in [
        (
            "book/cites.md",
            "---\nclass: canon\nrefs: [\"NEED-01\", \"NEED-02\"]\nadrs: [\"RULE-01\", \"NEED-02\"]\nsupersedes: [\"NEED-01\"]\n---\n",
        ),
        (
            "book/cites-prose.md",
            "---\nclass: canon\nrefs: [\"NEED-02\", \"prose words\"]\n---\n",
        ),
        (
            "book/cites-table.md",
            "---\nclass: canon\nrefs: [\"RULE-01\", \"NEED-02\"]\n---\n",
        ),
        (
            "book/cites-mapped.md",
            "---\nclass: canon\nrefs: [\"NEED-01\", \"RULE-01\"]\n---\n",
        ),
        (
            "book/cites-mapped-table.md",
            "---\nclass: canon\nrefs: [\"NEED-01\", \"RULE-01\"]\n---\n",
        ),
    ] {
        let text = run.tree_text(path);
        assert!(text.starts_with(header), "{path}: {text}");
        let values = values_of(&run, path);
        assert!(!values.is_empty(), "{path}");
        assert!(values.iter().all(|value| value.3), "{path}: {values:?}");
    }
    let unparsed: Vec<Value> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"] == "unparsed-reference")
        .collect();
    let got: BTreeSet<(String, String, String)> = unparsed
        .iter()
        .map(|finding| {
            let text = |key: &str| finding[key].as_str().unwrap_or_default().to_owned();
            (text("path"), text("subject"), text("cause"))
        })
        .collect();
    let want: BTreeSet<(String, String, String)> = [
        ("book/atoms/NEED/NEED-51.md", "see the old notes"),
        ("book/cites-prose.md", "prose words"),
    ]
    .into_iter()
    .map(|(path, subject)| (path.to_owned(), subject.to_owned(), "source".to_owned()))
    .collect();
    assert_eq!(got, want, "{unparsed:?}");
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("reasons/unexplained"), 0);
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// R3 with R1: a quoted header scalar under a reference-list key whose
/// YAML text holds an escape (`\"`, `\\`, a doubled `''`) stays one item,
/// as written (its comma-split would change its text). It is the corpus's
/// prose either way: read back alike (no mismatched field) and its
/// `unparsed-reference` the source's, like the unescaped one. Core keeps
/// no span of an escaped scalar, so its finding is spanless and its
/// subject `""` (`docs/canon/spec-check.md`: a spanless finding takes
/// `""`); the unescaped one names its text.
#[test]
fn a_quoted_reference_scalar_with_an_escape_reads_back_and_its_prose_is_the_source_s() {
    let scratch = Scratch::new("layout-r3-escapes");
    let run = one_with(
        &scratch,
        &[(
            KEY_MAP_ANCHOR,
            "\"Weight\" = \"weight\"\n\"Cites\" = \"refs\"\n",
        )],
        &[
            (
                "book/q1.md",
                "---\nCites: \"NEED-01, \\\"x\\\"\"\n---\n# Q1\n\nAn escaped quote.\n",
            ),
            (
                "book/q2.md",
                "---\nCites: \"it's prose\"\n---\n# Q2\n\nNo escape.\n",
            ),
            (
                "book/q3.md",
                "---\nCites: 'NEED-01, it''s'\n---\n# Q3\n\nA doubled quote.\n",
            ),
            (
                "book/q5.md",
                "---\nCites: \"a \\\\ b\"\n---\n# Q5\n\nAn escaped backslash.\n",
            ),
        ],
    );
    for (path, line) in [
        ("book/q1.md", "refs: [\"NEED-01, \\\"x\\\"\"]\n"),
        ("book/q2.md", "refs: [\"it's prose\"]\n"),
        ("book/q3.md", "refs: ['NEED-01, it''s']\n"),
        ("book/q5.md", "refs: [\"a \\\\ b\"]\n"),
    ] {
        let text = run.tree_text(path);
        assert!(text.contains(line), "{path}: {text}");
    }
    let mut problems = Vec::new();
    for (path, written, subject) in [
        ("book/q1.md", "NEED-01, \"x\"", ""),
        ("book/q2.md", "it's prose", "it's prose"),
        ("book/q3.md", "NEED-01, it's", ""),
        ("book/q5.md", "a \\ b", ""),
    ] {
        let values = values_of(&run, path);
        if values
            != [(
                "refs".to_owned(),
                written.to_owned(),
                written.to_owned(),
                true,
            )]
        {
            problems.push(format!("{path}: field_values {values:?}"));
        }
        let unparsed = findings_on(&run, "unparsed-reference", path);
        let causes: Vec<(&str, &str)> = unparsed
            .iter()
            .map(|finding| {
                (
                    finding["subject"].as_str().unwrap_or_default(),
                    finding["cause"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        if causes != [(subject, "source")] {
            problems.push(format!(
                "{path}: unparsed-reference (subject, cause) {causes:?}"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert_eq!(run.count("fields/mismatched"), 0);
    assert_eq!(run.count("check/emitter_findings"), 0);
}

/// R1 with R8 (AC-05 "Fields", AC-06): `canon` is a reference-typed core
/// key whose value core also reads as a path (`path#anchor`). A carried
/// path-form `canon` — from a header scalar and from a table cell, valid
/// (a walked canon document and its anchor) — reads back as written: no
/// mismatched field, no finding, enforce clean.
#[test]
fn a_carried_path_form_canon_reads_back_as_written() {
    let scratch = Scratch::new("layout-canon-path");
    let corpus = copy_with(
        ONE,
        &scratch,
        "corpus",
        &[(
            "book/canon-doc.md",
            "---\nclass: decision\nstatus: accepted\nCanon: book/glossary.md#glossary\n---\n# A decision with a path canon\n\nText.\n",
        )],
    );
    edit(
        &corpus.join("census.toml"),
        KEY_MAP_ANCHOR,
        "\"Weight\" = \"weight\"\n\"Canon\" = \"canon\"\n\"Area\" = \"canon\"\n",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| NEED-01 | The engine reads a pipe \\| inside a cell. | high | input |",
        "| NEED-01 | The engine reads a pipe \\| inside a cell. | high | book/glossary.md#glossary |",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| low | store |",
        "| low | book/glossary.md#glossary |",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| high | store |",
        "| high | book/glossary.md#glossary |",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert!(
        run.tree_text("book/canon-doc.md")
            .contains("\ncanon: book/glossary.md#glossary\n")
    );
    assert!(
        run.tree_text("book/atoms/NEED/NEED-01.md")
            .contains("\ncanon: \"book/glossary.md#glossary\"\n")
    );
    let canon: Vec<Value> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"].as_str().unwrap().starts_with("canon-"))
        .collect();
    assert!(canon.is_empty(), "a valid canon: {canon:?}");
    let values = values_of(&run, "book/canon-doc.md");
    let records: Vec<(String, Value)> = run
        .records()
        .into_iter()
        .filter(|record| {
            ["NEED-01", "NEED-02", "NEED-05"].contains(&record["id"].as_str().unwrap())
        })
        .map(|record| {
            (
                record["id"].as_str().unwrap().to_owned(),
                record["fields_mismatched"].clone(),
            )
        })
        .collect();
    assert_eq!(
        run.count("fields/mismatched"),
        0,
        "header {values:?}; record files' mismatched fields {records:?}"
    );
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// R4 (AC-03 "Fields", AC-04 "Extent residue"): heading attributes the
/// record file drops as header conflicts — a reserved `title=`, `id=`,
/// `class=`, `aliases=`, the task key, a repeated attribute (the first
/// kept) — are counted in `header.conflicts` only: neither compared as
/// fields nor left as extent residue; the kept one is a matched field.
#[test]
fn heading_attributes_dropped_as_conflicts_are_neither_compared_nor_residue() {
    let scratch = Scratch::new("layout-r4-attributes");
    let run = one_with(
        &scratch,
        &[],
        &[(
            "book/attrs.md",
            "# Attributes\n\n### Attr {#NEED-44 title=Other a=1 a=2 class=x ticked=true id=NEED-99 aliases=y}\n\nBody forty-four.\n",
        )],
    );
    assert_eq!(
        run.tree_text("book/atoms/NEED/NEED-44.md"),
        "---\nid: NEED-44\nclass: canon\ntitle: \"Attr\"\na: \"1\"\n---\n\nBody forty-four.\n"
    );
    let base = expected(ONE);
    let record = run
        .records()
        .into_iter()
        .find(|record| record["id"] == "NEED-44")
        .expect("NEED-44 in records.json");
    assert_eq!(record["fields_matched"], 1, "{record}");
    assert_eq!(record["fields_mismatched"], 0, "{record}");
    assert_eq!(record["title_matched"], true, "{record}");
    assert_eq!(
        run.count("header/conflicts"),
        base["header"]["conflicts"].as_u64().unwrap() + 6,
        "title, a=2, class, ticked, id, aliases"
    );
    assert_eq!(
        run.count("fields/matched"),
        base["fields"]["matched"].as_u64().unwrap() + 1
    );
    assert_eq!(run.count("fields/mismatched"), 0);
    assert_eq!(
        run.count("extents/residue"),
        0,
        "{:?}",
        run.file("extents.json")
    );
    assert_eq!(run.count("reasons/unexplained"), 0);
}

/// R6 (AC-06 "Attribution", `dangling`): after D8 renames a non-`.md`
/// residue to `.md`, a link to its old name — which core never checks —
/// reached a walked document from the source and reaches no tree file
/// now: it counts in `dangling.links.after`, named in `left_tree`. A link
/// to a non-`.md` residue that keeps its name (its `.md` name taken) and
/// one dangling in the source already are not counted.
#[test]
fn a_link_to_a_renamed_non_md_residue_counts_layout_broken() {
    let scratch = Scratch::new("layout-r6-renamed-target");
    let run = one_with(
        &scratch,
        &[(
            "roots = [\"book\"]\n",
            "roots = [\"book\"]\nextensions = [\"md\", \"markdown\"]\n",
        )],
        &[
            ("book/a.markdown", "# A\n\nA text.\n"),
            (
                "book/b.md",
                "# B\n\nSee [a](a.markdown) and [c](c.markdown) and [d](d.markdown).\n",
            ),
            ("book/c.markdown", "# C\n\nC text.\n"),
            ("book/c.md", "# C2\n\nC2 text.\n"),
        ],
    );
    let after: BTreeMap<String, String> = run
        .documents()
        .iter()
        .map(|document| {
            (
                document["source"].as_str().unwrap().to_owned(),
                document["after"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(after["book/a.markdown"], "book/a.md");
    assert_eq!(after["book/c.markdown"], "book/c.markdown");
    assert_eq!(
        run.file("findings.json")["left_tree"],
        serde_json::json!([{
            "path": "book/b.md",
            "line": 6,
            "target": "a.markdown",
            "source": "book/b.md"
        }])
    );
    let base = expected(ONE);
    assert_eq!(
        run.count("dangling/links/after"),
        base["dangling"]["links"]["after"].as_u64().unwrap() + 1
    );
    assert_eq!(
        run.count("dangling/links/before"),
        base["dangling"]["links"]["before"].as_u64().unwrap()
    );
}

/// R7 (D8, "Residue"): the `.md` name of a non-`.md` residue is taken
/// case-insensitively too (`a.markdown` beside `A.md`, one file on a
/// case-insensitive disk): it keeps its name with a layout diagnostic;
/// both documents survive, nothing moves.
#[test]
fn a_non_md_residue_whose_md_name_differs_only_in_case_keeps_its_name() {
    let scratch = Scratch::new("layout-r7-case");
    let run = one_with(
        &scratch,
        &[(
            "roots = [\"book\"]\n",
            "roots = [\"book\"]\nextensions = [\"md\", \"markdown\"]\n",
        )],
        &[
            ("book/a.markdown", "# Lower\n\nLower text.\n"),
            ("book/A.md", "# Upper\n\nUpper text.\n"),
        ],
    );
    let documents = run.documents();
    let lower = documents
        .iter()
        .find(|document| document["source"] == "book/a.markdown")
        .expect("the .markdown document is walked");
    assert_eq!(lower["after"], "book/a.markdown", "{lower}");
    assert_eq!(lower["moved"], false, "{lower}");
    assert_eq!(
        run.tree_text("book/a.markdown"),
        "---\nclass: canon\n---\n# Lower\n\nLower text.\n"
    );
    assert_eq!(
        run.tree_text("book/A.md"),
        "---\nclass: canon\n---\n# Upper\n\nUpper text.\n"
    );
    let diagnostics = run.file("diagnostics.json")["layout"]
        .as_array()
        .unwrap()
        .clone();
    let taken: Vec<&Value> = diagnostics
        .iter()
        .filter(|entry| entry["path"] == "book/a.markdown")
        .collect();
    assert_eq!(taken.len(), 1, "{diagnostics:?}");
    assert!(
        taken[0]["message"].as_str().unwrap().contains("is taken"),
        "{}",
        taken[0]
    );
    let base = expected(ONE);
    assert_eq!(
        run.count("tree/moved"),
        base["tree"]["moved"].as_u64().unwrap()
    );
    assert_eq!(
        run.count("tree/documents"),
        base["tree"]["documents"].as_u64().unwrap() + 2
    );
    assert_eq!(run.count("prose/mismatched"), 0);
}

/// S1, S2 (AC-06 "Attribution"): a required key the corpus lacks
/// (`canon-missing` of an accepted decision, `shipped-missing` of a
/// shipped spec — record files whose class `record_class` sets) and a
/// carried value a check rule rejects (`value-invalid`), read back as the
/// corpus wrote it, are the source's: baselined, enforce clean. (The
/// tampered counterparts — the key carried then lost, the value changed —
/// are the emitter's: the tamper tests of `layout.rs`.)
#[test]
fn a_key_the_corpus_lacks_and_a_value_a_rule_rejects_are_the_source_s() {
    let scratch = Scratch::new("layout-s1-s2");
    let weight_row = "| NEED-01 | The engine reads a pipe \\| inside a cell. | high | input |";
    for (case, class, status, code, subject) in [
        ("canon", "decision", "accepted", "canon-missing", "canon"),
        ("shipped", "spec", "shipped", "shipped-missing", "shipped"),
    ] {
        let corpus = copy_of(ONE, &scratch, &format!("corpus-{case}"));
        edit(
            &corpus.join("census.toml"),
            KEY_MAP_ANCHOR,
            "\"Weight\" = \"status\"\n",
        );
        edit(
            &corpus.join("census.toml"),
            "[layout]\n",
            &format!("[layout]\nrecord_class = \"{class}\"\n"),
        );
        edit(
            &corpus.join("book/needs.md"),
            weight_row,
            &weight_row.replace("| high |", &format!("| {status} |")),
        );
        let run = LayoutRun::new(&corpus, &scratch, &format!("out-{case}"), &[]);
        let found = findings_on(&run, code, "book/atoms/NEED/NEED-01.md");
        assert_eq!(found.len(), 1, "{case}: {:?}", run.tree_findings());
        assert_eq!(found[0]["subject"], subject, "{}", found[0]);
        assert_eq!(found[0]["cause"], "source", "{}", found[0]);
        assert_eq!(
            run.count("check/emitter_findings"),
            0,
            "{case}: {}",
            run.result()
        );
        assert_eq!(run.result()["check"]["enforce"], "clean", "{case}");
    }

    let corpus = copy_of(ONE, &scratch, "corpus-rule");
    let scheme = corpus.join("specengine.toml");
    let text = fs::read_to_string(&scheme).unwrap();
    fs::write(
        &scheme,
        format!("{text}\n[[check.rules]]\nkinds = [\"requirement\"]\nvalues = {{ weight = [\"high\"] }}\n"),
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out-rule", &[]);
    let invalid: Vec<Value> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"] == "value-invalid")
        .collect();
    assert_eq!(invalid.len(), 1, "only NEED-02 is low: {invalid:?}");
    assert_eq!(invalid[0]["path"], "book/atoms/NEED/NEED-02.md");
    assert_eq!(invalid[0]["subject"], "weight");
    assert_eq!(invalid[0]["cause"], "source", "{}", invalid[0]);
    assert_eq!(run.count("fields/mismatched"), 0);
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// S3 (AC-04 "Extent residue"): a field table's extent opens at the
/// table's header line, so a field row's residue is named at that line —
/// line 1 for a table opening the document, line 2 behind a comment —
/// never at its first field row.
#[test]
fn a_field_table_s_residue_is_named_at_the_table_s_header_line() {
    let scratch = Scratch::new("layout-s3-extent-line");
    for (index, (above, line)) in [("", 1), ("<!-- the plan's attributes -->\n", 2)]
        .into_iter()
        .enumerate()
    {
        let corpus = copy_of(ONE, &scratch, &format!("corpus-{index}"));
        let plan = corpus.join("book/plan.md");
        edit(
            &plan,
            "| Attribute | Value |\n",
            &format!("{above}| Attribute | Value |\n"),
        );
        edit(
            &plan,
            "| Keeper | team |\n",
            "| Keeper | team | third cell words |\n",
        );
        let run = LayoutRun::new(&corpus, &scratch, &format!("out-{index}"), &[]);
        assert_eq!(
            run.file("extents.json"),
            serde_json::json!([{
                "path": "book/plan.md",
                "line": line,
                "id": null,
                "residue": "third cell words"
            }]),
            "{above:?}"
        );
        assert_eq!(run.count("fields/mismatched"), 0);
    }
}

// ---------------------------------- iteration 4 (#4a, #5, #7, F1, Tier 2.0)

/// The census edit renaming `Cites` to `refs`, `Canon` to `canon` and
/// `Tier` to `tier`.
const CITED_CENSUS: [(&str, &str); 1] = [(
    KEY_MAP_ANCHOR,
    "\"Weight\" = \"weight\"\n\"Cites\" = \"refs\"\n\"Canon\" = \"canon\"\n\"Tier\" = \"tier\"\n",
)];

/// The (subject, cause) of every finding with `code` on `path`, sorted.
fn subjects_on(run: &LayoutRun, code: &str, path: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = findings_on(run, code, path)
        .iter()
        .map(|finding| {
            let text = |key: &str| finding[key].as_str().unwrap_or_default().to_owned();
            (text("subject"), text("cause"))
        })
        .collect();
    found.sort();
    found
}

/// #7 (AC-05 "YAML"): a reference-list value split at its commas drops
/// empty items — a trailing comma, a doubled one — from a plain header
/// scalar, a quoted one without an escape, and a table cell; each reads
/// back alike (the verifier drops the empty expected items too), and no
/// `unparsed-reference` has an empty subject.
#[test]
fn a_reference_list_split_drops_empty_items_and_reads_back_alike() {
    let scratch = Scratch::new("layout-i4-empty-items");
    let run = one_with(
        &scratch,
        &CITED_CENSUS,
        &[
            (
                "book/e1.md",
                "---\nCites: XQ-42,\n---\n# E1\n\nA trailing comma.\n",
            ),
            (
                "book/e2.md",
                "---\nCites: a,, b\n---\n# E2\n\nA doubled comma.\n",
            ),
            (
                "book/e3.md",
                "---\nCites: \"NEED-01, NEED-02,\"\n---\n# E3\n\nA quoted trailing comma.\n",
            ),
            (
                "book/e4.md",
                "# E4\n\n| Code | Wording | Cites |\n|------|---------|-------|\n| NEED-60 | A trailing comma. | NEED-01, |\n| NEED-61 | A doubled comma. | NEED-01,, NEED-02 |\n",
            ),
        ],
    );
    for (path, line, written, read) in [
        ("book/e1.md", "refs: [\"XQ-42\"]\n", "XQ-42,", "XQ-42"),
        ("book/e2.md", "refs: [\"a\", \"b\"]\n", "a,, b", "a, b"),
        (
            "book/e3.md",
            "refs: [\"NEED-01\", \"NEED-02\"]\n",
            "NEED-01, NEED-02,",
            "NEED-01, NEED-02",
        ),
    ] {
        let text = run.tree_text(path);
        assert!(text.contains(line), "{path}: {text}");
        assert_eq!(
            values_of(&run, path),
            [("refs".to_owned(), written.to_owned(), read.to_owned(), true)],
            "{path}"
        );
    }
    for (id, line) in [
        ("NEED-60", "refs: [\"NEED-01\"]\n"),
        ("NEED-61", "refs: [\"NEED-01\", \"NEED-02\"]\n"),
    ] {
        let text = run.tree_text(&format!("book/atoms/NEED/{id}.md"));
        assert!(text.contains(line), "{id}: {text}");
        let record = run
            .records()
            .into_iter()
            .find(|record| record["id"] == id)
            .unwrap_or_else(|| panic!("{id} in records.json"));
        assert_eq!(record["fields_matched"], 1, "{record}");
        assert_eq!(record["fields_mismatched"], 0, "{record}");
    }
    let unparsed: Vec<Value> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"] == "unparsed-reference")
        .collect();
    assert!(
        unparsed.iter().all(|finding| finding["subject"] != ""),
        "{unparsed:?}"
    );
    assert_eq!(
        subjects_on(&run, "unparsed-reference", "book/e2.md"),
        [
            ("a".to_owned(), "source".to_owned()),
            ("b".to_owned(), "source".to_owned())
        ]
    );
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// #4a (AC-05 "YAML"): a quoted reference-list scalar stays one item only
/// with a real YAML escape; `"XQ-42, it's"` (an apostrophe in double
/// quotes) and `'NEED-01, NEED-02'` are split like plain ones, read back
/// alike, and the prose item is the source's `unparsed-reference` under
/// its own text.
#[test]
fn a_quoted_reference_scalar_without_an_escape_is_split_at_its_commas() {
    let scratch = Scratch::new("layout-i4-quoted-split");
    let run = one_with(
        &scratch,
        &CITED_CENSUS,
        &[
            (
                "book/s1.md",
                "---\nCites: \"XQ-42, it's\"\n---\n# S1\n\nAn apostrophe, no escape.\n",
            ),
            (
                "book/s2.md",
                "---\nCites: 'NEED-01, NEED-02'\n---\n# S2\n\nSingle quotes, no escape.\n",
            ),
        ],
    );
    for (path, line, written) in [
        ("book/s1.md", "refs: [\"XQ-42\", \"it's\"]\n", "XQ-42, it's"),
        (
            "book/s2.md",
            "refs: [\"NEED-01\", \"NEED-02\"]\n",
            "NEED-01, NEED-02",
        ),
    ] {
        let text = run.tree_text(path);
        assert!(text.contains(line), "{path}: {text}");
        assert_eq!(
            values_of(&run, path),
            [(
                "refs".to_owned(),
                written.to_owned(),
                written.to_owned(),
                true
            )],
            "{path}"
        );
    }
    assert_eq!(
        subjects_on(&run, "unparsed-reference", "book/s1.md"),
        [
            ("XQ-42".to_owned(), "source".to_owned()),
            ("it's".to_owned(), "source".to_owned())
        ]
    );
    assert!(subjects_on(&run, "unparsed-reference", "book/s2.md").is_empty());
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
}

/// #5 (AC-05 "Fields"): reference and reference-list values read back as
/// written, not by their IDs: `NEED-01@2` (a rev), `checkout/CK-01` (a
/// feature `slug/`, the checkout flow moved under `book/flows`),
/// `RULE-01#RULE-02` (a `#section`), `XQ-42@2` (an unknown prefix) — each
/// item, and each single `canon` — no mismatched field; only the corpus's
/// unparsable `XQ-42@2` raises a finding, the source's.
#[test]
fn references_read_back_as_written_with_their_rev_slug_and_section() {
    let scratch = Scratch::new("layout-i4-as-written");
    let run = one_with(
        &scratch,
        &CITED_CENSUS,
        &[
            (
                "book/r1.md",
                "---\nCites: NEED-01@2, checkout/CK-01, RULE-01#RULE-02, XQ-42@2\n---\n# R1\n\nReferences as written.\n",
            ),
            (
                "book/r2.md",
                "---\nCanon: NEED-01@2\n---\n# R2\n\nA canon with a rev.\n",
            ),
            (
                "book/r3.md",
                "---\nCanon: checkout/CK-01\n---\n# R3\n\nA canon with a slug.\n",
            ),
            (
                "book/r4.md",
                "---\nCanon: RULE-01#RULE-02\n---\n# R4\n\nA canon with a section.\n",
            ),
        ],
    );
    for (path, line, key, written) in [
        (
            "book/r1.md",
            "refs: [\"NEED-01@2\", \"checkout/CK-01\", \"RULE-01#RULE-02\", \"XQ-42@2\"]\n",
            "refs",
            "NEED-01@2, checkout/CK-01, RULE-01#RULE-02, XQ-42@2",
        ),
        ("book/r2.md", "canon: NEED-01@2\n", "canon", "NEED-01@2"),
        (
            "book/r3.md",
            "canon: checkout/CK-01\n",
            "canon",
            "checkout/CK-01",
        ),
        (
            "book/r4.md",
            "canon: RULE-01#RULE-02\n",
            "canon",
            "RULE-01#RULE-02",
        ),
    ] {
        let text = run.tree_text(path);
        assert!(text.contains(line), "{path}: {text}");
        assert_eq!(
            values_of(&run, path),
            [(key.to_owned(), written.to_owned(), written.to_owned(), true)],
            "{path}"
        );
    }
    let on_r: Vec<(String, String, String, String)> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| {
            let path = finding["path"].as_str().unwrap_or_default();
            ["book/r1.md", "book/r2.md", "book/r3.md", "book/r4.md"].contains(&path)
                && finding["code"] != "key-missing"
        })
        .map(|finding| {
            let text = |key: &str| finding[key].as_str().unwrap_or_default().to_owned();
            (text("path"), text("code"), text("subject"), text("cause"))
        })
        .collect();
    assert_eq!(
        on_r,
        [(
            "book/r1.md".to_owned(),
            "unparsed-reference".to_owned(),
            "XQ-42@2".to_owned(),
            "source".to_owned()
        )]
    );
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// F1 (AC-06 `canon-*`): a path-form `canon` the corpus wrote, carried
/// and read back alike, is the source's `canon-form` (no `#`: a header's
/// `book/glossary.md`, a table cell's `book/glossary.md` on a record
/// file) or `canon-file` (`glossary.md#glossary`, not the root's path);
/// one naming the document the layout moved (`book/checkout.md#checkout`)
/// is the layout's `canon-file`, one naming a section the layout cut
/// (`book/needs.md#NEED-40`) the layout's `canon-anchor`; a valid one
/// (`book/glossary.md#glossary`) raises nothing. No emitter finding; the
/// layout's errors are not the source's debt (enforce blocks on them),
/// and without them enforce is clean.
#[test]
fn a_carried_path_canon_is_the_source_s_or_the_layout_s_by_what_broke_it() {
    let scratch = Scratch::new("layout-i4-canon-path");
    let corpus = copy_with(
        ONE,
        &scratch,
        "corpus",
        &[
            (
                "book/c1.md",
                "---\nCanon: book/glossary.md\n---\n# C1\n\nNo anchor.\n",
            ),
            (
                "book/c2.md",
                "---\nCanon: glossary.md#glossary\n---\n# C2\n\nNot the root's path.\n",
            ),
            (
                "book/c3.md",
                "---\nCanon: book/checkout.md#checkout\n---\n# C3\n\nA moved document.\n",
            ),
            (
                "book/c4.md",
                "---\nCanon: book/needs.md#NEED-40\n---\n# C4\n\nA cut section.\n",
            ),
            (
                "book/c5.md",
                "---\nCanon: book/glossary.md#glossary\n---\n# C5\n\nA valid canon.\n",
            ),
        ],
    );
    edit(
        &corpus.join("census.toml"),
        KEY_MAP_ANCHOR,
        "\"Weight\" = \"weight\"\n\"Canon\" = \"canon\"\n\"Area\" = \"canon\"\n",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| low | store |",
        "| low | book/glossary.md |",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| high | input |",
        "| high | book/glossary.md#glossary |",
    );
    edit(
        &corpus.join("book/needs.md"),
        "| high | store |",
        "| high | book/glossary.md#glossary |",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert!(
        run.tree_text("book/atoms/NEED/NEED-02.md")
            .contains("\ncanon: \"book/glossary.md\"\n")
    );
    let got: BTreeSet<(String, String, String, String)> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"].as_str().unwrap().starts_with("canon-"))
        .map(|finding| {
            let text = |key: &str| finding[key].as_str().unwrap_or_default().to_owned();
            (text("path"), text("code"), text("subject"), text("cause"))
        })
        .collect();
    let want: BTreeSet<(String, String, String, String)> = [
        ("book/c1.md", "canon-form", "book/glossary.md", "source"),
        ("book/c2.md", "canon-file", "glossary.md#glossary", "source"),
        (
            "book/c3.md",
            "canon-file",
            "book/checkout.md#checkout",
            "layout",
        ),
        (
            "book/c4.md",
            "canon-anchor",
            "book/needs.md#NEED-40",
            "layout",
        ),
        (
            "book/atoms/NEED/NEED-02.md",
            "canon-form",
            "book/glossary.md",
            "source",
        ),
    ]
    .into_iter()
    .map(|(path, code, subject, cause)| {
        (
            path.to_owned(),
            code.to_owned(),
            subject.to_owned(),
            cause.to_owned(),
        )
    })
    .collect();
    assert_eq!(got, want);
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.count("check/findings/canon-file/layout"), 1);
    assert_eq!(run.count("check/findings/canon-anchor/layout"), 1);
    assert_eq!(run.result()["check"]["enforce"], "blocked");

    for path in ["book/c3.md", "book/c4.md"] {
        fs::remove_file(corpus.join(path)).unwrap();
    }
    let run = LayoutRun::new(&corpus, &scratch, "out-source-only", &[]);
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(
        run.result()["check"]["enforce"],
        "clean",
        "{}",
        run.result()
    );
}

/// The follow-up to #2 (AC-06 `frontmatter-type`): a float under an
/// integer key (`Tier: 2.0`) is carried as written, core keeps it as a
/// float, and its `frontmatter-type` is the source's: core's kept value
/// keeps its fraction (`2.0` does not parse as the key's integer).
#[test]
fn a_float_under_an_integer_key_is_the_source_s_frontmatter_type() {
    let scratch = Scratch::new("layout-i4-float-tier");
    let run = one_with(
        &scratch,
        &CITED_CENSUS,
        &[("book/t1.md", "---\nTier: 2.0\n---\n# T1\n\nA float tier.\n")],
    );
    assert!(run.tree_text("book/t1.md").contains("\ntier: 2.0\n"));
    assert_eq!(
        subjects_on(&run, "frontmatter-type", "book/t1.md"),
        [("tier".to_owned(), "source".to_owned())]
    );
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

// --------------------------------------------------- iteration 5 (P1-P6)

/// Fixture one's ID regex unanchored at its end: a cell or a header value
/// with text after the ID still names the ID (both pilots read IDs so).
const UNANCHORED_ONE: (&str, &str) = ("[0-9]{2,3}$'", "[0-9]{2,3}'");

/// Fixture two's ID regex unanchored at its end.
const UNANCHORED_TWO: (&str, &str) = ("[0-9]{4}$'", "[0-9]{4}'");

/// `files` as the `(&str, &str)` pairs `one_with` takes.
fn pairs(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect()
}

/// A document opening with a field table of `rows` (key, value).
fn field_table_document(rows: &[(&str, &str)], title: &str) -> String {
    let rows: String = rows
        .iter()
        .map(|(key, value)| format!("| {key} | {value} |\n"))
        .collect();
    format!("| Attribute | Value |\n|-----------|-------|\n{rows}\n# {title}\n\nThe body.\n")
}

/// The one record of `records.json` with `id` and `form`.
fn record_of(run: &LayoutRun, id: &str, form: &str) -> Value {
    let found: Vec<Value> = run
        .records()
        .into_iter()
        .filter(|record| record["id"] == id && record["form"] == form)
        .collect();
    assert_eq!(found.len(), 1, "{id} ({form}): {found:?}");
    found[0].clone()
}

/// The causes of the `code` findings on `path` whose subject is `subject`.
fn causes_of(run: &LayoutRun, code: &str, path: &str, subject: &str) -> Vec<String> {
    subjects_on(run, code, path)
        .into_iter()
        .filter(|(found, _)| found == subject)
        .map(|(_, cause)| cause)
        .collect()
}

/// The counts every iteration-5 corpus must keep (AC-11's targets on an
/// invented corpus): every definition found less the listed reasons, no
/// unexplained miss, no prose or residue loss, nothing the emitter's.
fn assert_round_trip_clean(run: &LayoutRun) {
    let result = run.result();
    assert_eq!(run.count("hashes/mismatched"), 0, "{result}");
    assert_eq!(run.count("hashes/extra"), 0, "{result}");
    assert_eq!(run.count("reasons/unexplained"), 0, "{result}");
    assert_eq!(run.count("prose/mismatched"), 0, "{result}");
    assert_eq!(run.count("extents/residue"), 0, "{result}");
    assert_eq!(run.count("header/keys_dropped"), 0, "{result}");
    assert_eq!(run.count("check/emitter_findings"), 0, "{result}");
    assert_eq!(result["check"]["enforce"], "clean", "{result}");
}

/// P1 (AC-05 "Headers"): a key whose `key_map` target is core `id` is
/// renamed only where the import read the document's ID from that key and
/// its value is that ID alone (`NEED-30`; the legacy `OLDR-35`, written
/// `id: NEED-35` by rule S with its alias). Any other is carried under its
/// written name, its value as written, and the `id` a document record
/// needs is added: an ID with a note (a table cell, a YAML scalar, a
/// legacy one), no ID at all, an ID the import read from the path, a
/// second key reaching `id`. Each document record is found and matches,
/// each carried value reads back alike, the carried key is the source's
/// `unknown-key`, and core reports no `id-not-in-scheme`. (Pilot B
/// before: 20 `id-not-in-scheme`, 4 `unexplained`.)
#[test]
fn p1_a_key_reaching_id_is_renamed_only_for_the_document_s_id_alone() {
    let scratch = Scratch::new("layout-i5-p1");
    let census = [
        ("\"Serial\" = \"serial\"", "\"Serial\" = \"id\""),
        (
            "id_key = \"serial\"",
            "id_key = \"id\"\nid_path = '^book/(?P<id>NEED-[0-9]{2})\\.md$'",
        ),
        UNANCHORED_ONE,
    ];
    let files: Vec<(String, String)> = [
        (
            "book/p-text.md",
            field_table_document(
                &[("Serial", "NEED-32 (draft)"), ("Keeper", "ops")],
                "With a note",
            ),
        ),
        (
            "book/p-none.md",
            field_table_document(&[("Serial", "provisional label")], "No ID"),
        ),
        (
            "book/p-yaml.md",
            "---\nSerial: NEED-33; notes\n---\n# YAML with a note\n\nThe body.\n".to_owned(),
        ),
        (
            "book/NEED-34.md",
            field_table_document(&[("Serial", "tbd")], "By path"),
        ),
        (
            "book/p-legacy.md",
            field_table_document(&[("Serial", "OLDR-35")], "Legacy exact"),
        ),
        (
            "book/p-legacy-text.md",
            field_table_document(&[("Serial", "OLDR-36 (old)")], "Legacy with a note"),
        ),
        (
            "book/p-two.md",
            field_table_document(&[("Serial", "NEED-38"), ("Serial", "NEED-39")], "Two keys"),
        ),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text))
    .collect();
    let run = one_with(&scratch, &census, &pairs(&files));
    for (path, header) in [
        (
            "book/plan.md",
            "---\nclass: canon\nid: \"NEED-30\"\nstatus: \"accepted\"\nowner: \"team\"\n---\n",
        ),
        (
            "book/p-text.md",
            "---\nid: NEED-32\nclass: canon\nSerial: \"NEED-32 (draft)\"\nowner: \"ops\"\n---\n",
        ),
        (
            "book/p-none.md",
            "---\nclass: canon\nSerial: \"provisional label\"\n---\n",
        ),
        (
            "book/p-yaml.md",
            "---\nid: NEED-33\nclass: canon\nSerial: NEED-33; notes\n---\n",
        ),
        (
            "book/NEED-34.md",
            "---\nid: NEED-34\nclass: canon\nSerial: \"tbd\"\n---\n",
        ),
        (
            "book/p-legacy.md",
            "---\nclass: canon\naliases: [\"OLDR-35\"]\nid: \"NEED-35\"\n---\n",
        ),
        (
            "book/p-legacy-text.md",
            "---\nid: NEED-36\nclass: canon\naliases: [\"OLDR-36\"]\nSerial: \"OLDR-36 (old)\"\n---\n",
        ),
        (
            "book/p-two.md",
            "---\nclass: canon\nid: \"NEED-38\"\nSerial: \"NEED-39\"\n---\n",
        ),
    ] {
        let text = run.tree_text(path);
        assert!(text.starts_with(header), "{path}:\n{text}");
    }
    for (id, path) in [
        ("NEED-30", "book/plan.md"),
        ("NEED-32", "book/p-text.md"),
        ("NEED-33", "book/p-yaml.md"),
        ("NEED-34", "book/NEED-34.md"),
        ("NEED-35", "book/p-legacy.md"),
        ("NEED-36", "book/p-legacy-text.md"),
        ("NEED-38", "book/p-two.md"),
    ] {
        let record = record_of(&run, id, "document");
        assert_eq!(record["after"], path, "{record}");
        assert_eq!(record["hash"], "matched", "{record}");
    }
    assert!(
        run.definitions()
            .iter()
            .all(|entry| entry["path"] != "book/p-none.md"),
        "a value naming no ID makes no document record"
    );
    for (path, value) in [
        ("book/p-text.md", "NEED-32 (draft)"),
        ("book/p-none.md", "provisional label"),
        ("book/NEED-34.md", "tbd"),
        ("book/p-legacy-text.md", "OLDR-36 (old)"),
        ("book/p-two.md", "NEED-39"),
    ] {
        let values = values_of(&run, path);
        assert!(
            values.contains(&(
                "Serial".to_owned(),
                value.to_owned(),
                value.to_owned(),
                true
            )),
            "{path}: {values:?}"
        );
        assert!(values.iter().all(|value| value.3), "{path}: {values:?}");
    }
    for path in [
        "book/p-text.md",
        "book/p-none.md",
        "book/p-yaml.md",
        "book/NEED-34.md",
        "book/p-legacy-text.md",
        "book/p-two.md",
    ] {
        assert_eq!(
            causes_of(&run, "unknown-key", path, "Serial"),
            ["source"],
            "{path}"
        );
    }
    assert!(
        run.tree_findings()
            .iter()
            .all(|finding| finding["code"] != "id-not-in-scheme"),
        "{:?}",
        run.tree_findings()
    );
    assert_eq!(
        run.count("hashes/matched"),
        run.count("before/definitions"),
        "{}",
        run.result()
    );
    assert_eq!(run.count("hashes/missing"), 0);
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_round_trip_clean(&run);
}

/// P2 (AC-03, AC-04 "Extent residue"): a row's ID cell holding a letter or
/// digit beyond its written ID is carried whole as a field, under the ID
/// column's header as written (`Code`; `col-1` in a headerless table),
/// and compared; a cell that is the ID with decoration only carries
/// nothing. The row leaves no residue, and the carried key is the source's
/// `unknown-key`. (Pilot B before: two rows lost their note, residue 2.)
#[test]
fn p2_an_id_cell_with_text_beyond_its_id_is_carried_and_compared() {
    let scratch = Scratch::new("layout-i5-p2");
    let one = copy_of(ONE, &scratch, "one");
    edit(&one.join("census.toml"), UNANCHORED_ONE.0, UNANCHORED_ONE.1);
    edit(
        &one.join("book/needs.md"),
        "| OLDR-05 | A need still cited under its old prefix. | high | store |\n",
        "| OLDR-05 | A need still cited under its old prefix. | high | store |\n\
         | NEED-03 (draft) | A need with a note in its code cell. | high | input |\n\
         | **NEED-04** revised by NEED-02 | A bold code with a note. | low | store |\n\
         | **NEED-06** | A bold code alone. | low | store |\n",
    );
    let run = LayoutRun::new(&one, &scratch, "out-one", &[]);
    assert_eq!(
        run.tree_text("book/atoms/NEED/NEED-03.md"),
        "---\nid: NEED-03\nclass: canon\nCode: \"NEED-03 (draft)\"\nweight: \"high\"\nArea: \"input\"\n---\n\nA need with a note in its code cell.\n"
    );
    assert!(
        run.tree_text("book/atoms/NEED/NEED-04.md").contains(
            "\nclass: canon\nCode: \"**NEED-04** revised by NEED-02\"\nweight: \"low\"\n"
        ),
        "{}",
        run.tree_text("book/atoms/NEED/NEED-04.md")
    );
    assert!(
        !run.tree_text("book/atoms/NEED/NEED-06.md").contains("Code"),
        "decoration alone carries nothing"
    );
    for (id, fields) in [("NEED-03", 3), ("NEED-04", 3), ("NEED-06", 2)] {
        let record = record_of(&run, id, "table-row");
        assert_eq!(record["hash"], "matched", "{record}");
        assert_eq!(record["fields_matched"], fields, "{record}");
        assert_eq!(record["fields_mismatched"], 0, "{record}");
    }
    for path in ["book/atoms/NEED/NEED-03.md", "book/atoms/NEED/NEED-04.md"] {
        assert_eq!(causes_of(&run, "unknown-key", path, "Code"), ["source"]);
    }
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_round_trip_clean(&run);

    let two = copy_of(TWO, &scratch, "two");
    edit(&two.join("census.toml"), UNANCHORED_TWO.0, UNANCHORED_TWO.1);
    edit(
        &two.join("log/decisions.md"),
        "| 4 | ZZQ-0001 | A row under a prefix the scheme lacks. |\n",
        "| 4 | ZZQ-0001 | A row under a prefix the scheme lacks. |\n\
         | 5 | DEC-0008 (draft) | A row whose number cell holds a note. |\n\
         | 6 | **OLD-0009** revised by DEC-0002 | A legacy bold number with a note. |\n\
         | 7 | **DEC-0010** | A bold number alone. |\n",
    );
    let run = LayoutRun::new(&two, &scratch, "out-two", &[]);
    assert_eq!(
        run.tree_text("ledger/DEC/DEC-0008.md"),
        "---\nid: DEC-0008\nclass: decision\ncol-0: \"5\"\ncol-1: \"DEC-0008 (draft)\"\n---\n\nA row whose number cell holds a note.\n"
    );
    assert_eq!(
        run.tree_text("ledger/DEC/DEC-0009.md"),
        "---\nid: DEC-0009\nclass: decision\naliases: [\"OLD-0009\"]\ncol-0: \"6\"\ncol-1: \"**OLD-0009** revised by DEC-0002\"\n---\n\nA legacy bold number with a note.\n"
    );
    assert!(
        !run.tree_text("ledger/DEC/DEC-0010.md").contains("col-1"),
        "decoration alone carries nothing"
    );
    for (id, fields) in [("DEC-0008", 2), ("DEC-0009", 2), ("DEC-0010", 1)] {
        let record = record_of(&run, id, "headerless-row");
        assert_eq!(record["hash"], "matched", "{record}");
        assert_eq!(record["fields_matched"], fields, "{record}");
        assert_eq!(record["fields_mismatched"], 0, "{record}");
    }
    for path in ["ledger/DEC/DEC-0008.md", "ledger/DEC/DEC-0009.md"] {
        assert_eq!(causes_of(&run, "unknown-key", path, "col-1"), ["source"]);
    }
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_round_trip_clean(&run);
}

/// P2 under a `section` target (accepted deviation 3): the carried ID cell
/// is one more heading attribute, under the attribute rule — a value with
/// a blank is left out (`section_fields`, one field miss), one without is
/// written `Code=<cell>` and read back. The row leaves no residue either
/// way.
#[test]
fn p2_an_id_cell_under_a_section_target_follows_the_attribute_rule() {
    let scratch = Scratch::new("layout-i5-p2-section");
    let corpus = copy_of(ONE, &scratch, "corpus");
    edit(
        &corpus.join("census.toml"),
        UNANCHORED_ONE.0,
        UNANCHORED_ONE.1,
    );
    edit(
        &corpus.join("book/needs.md"),
        "| OLDR-05 | A need still cited under its old prefix. | high | store |\n",
        "| OLDR-05 | A need still cited under its old prefix. | high | store |\n\
         | RULE-03 (draft) | A rule row with a note. | high | input |\n\
         | RULE-04;draft | A rule row with a glued note. | high | input |\n",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let needs = run.tree_text("book/needs.md");
    assert!(
        needs.contains("## RULE-03 {#RULE-03 weight=high Area=input}\n"),
        "{needs}"
    );
    assert!(
        needs.contains("## RULE-04 {#RULE-04 Code=RULE-04;draft weight=high Area=input}\n"),
        "{needs}"
    );
    let reasons: BTreeMap<String, Value> = run
        .definitions()
        .into_iter()
        .filter(|entry| entry["id"] == "RULE-03" || entry["id"] == "RULE-04")
        .map(|entry| {
            (
                entry["id"].as_str().unwrap().to_owned(),
                entry["reason"].clone(),
            )
        })
        .collect();
    assert_eq!(reasons["RULE-03"], "section_fields", "{reasons:?}");
    assert_eq!(reasons["RULE-04"], Value::Null, "{reasons:?}");
    assert_eq!(run.count("reasons/section_fields"), 1, "{}", run.result());
    assert_eq!(run.count("fields/mismatched"), 1, "{}", run.result());
    assert_eq!(
        record_of(&run, "RULE-04", "table-row")["fields_mismatched"],
        0
    );
    assert_eq!(record_of(&run, "RULE-04", "table-row")["fields_matched"], 3);
    assert_round_trip_clean(&run);
}

/// P3 (AC-05 "Headers"): a repeated field-table key is carried as
/// `<key>-<n>` (n from 2, the first name the header does not hold), its
/// cell as written — no value map, no rule S — and compared; nothing is
/// dropped. Each repeat is a header conflict with a diagnostic, and the
/// `<key>-<n>` key is the source's `unknown-key`. A field-table key
/// repeating a YAML header key takes `-2` too. (Pilot B before: one
/// document lost two rows, flagged by no count but `conflicts`.)
#[test]
fn p3_a_repeated_field_table_key_is_carried_as_key_n_and_compared() {
    let scratch = Scratch::new("layout-i5-p3");
    let corpus = copy_with(
        ONE,
        &scratch,
        "corpus",
        &[(
            "book/both.md",
            "---\nKeeper: lead\n---\n| Attribute | Value |\n|-----------|-------|\n| Keeper | team-a |\n\n# Both headers\n\nA YAML header and a field table.\n",
        )],
    );
    edit(
        &corpus.join("book/plan.md"),
        "| Keeper | team |\n",
        "| Keeper | team |\n| Keeper | crew |\n| Keeper | guild |\n| Stage | Settled |\n",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    let plan = run.tree_text("book/plan.md");
    assert!(
        plan.starts_with(
            "---\nid: NEED-30\nclass: canon\nserial: \"NEED-30\"\nstatus: \"accepted\"\nowner: \"team\"\nowner-2: \"crew\"\nowner-3: \"guild\"\nstatus-2: \"Settled\"\n---\n"
        ),
        "{plan}"
    );
    let both = run.tree_text("book/both.md");
    assert!(
        both.starts_with("---\nclass: canon\nowner: lead\nowner-2: \"team-a\"\n---\n"),
        "{both}"
    );
    let values = values_of(&run, "book/plan.md");
    for (key, value) in [
        ("owner", "team"),
        ("owner-2", "crew"),
        ("owner-3", "guild"),
        ("status", "accepted"),
        ("status-2", "Settled"),
    ] {
        assert!(
            values.contains(&(key.to_owned(), value.to_owned(), value.to_owned(), true)),
            "{key}: {values:?}"
        );
    }
    assert!(
        values_of(&run, "book/both.md").contains(&(
            "owner-2".to_owned(),
            "team-a".to_owned(),
            "team-a".to_owned(),
            true
        )),
        "{:?}",
        values_of(&run, "book/both.md")
    );
    for (path, key) in [
        ("book/plan.md", "owner-2"),
        ("book/plan.md", "owner-3"),
        ("book/plan.md", "status-2"),
        ("book/both.md", "owner-2"),
    ] {
        assert_eq!(
            causes_of(&run, "unknown-key", path, key),
            ["source"],
            "{path} {key}"
        );
    }
    assert_eq!(run.count("header/conflicts"), 4, "{}", run.result());
    let diagnostics = run.file("diagnostics.json")["layout"]
        .as_array()
        .expect("layout diagnostics")
        .clone();
    let repeats: BTreeSet<(String, u64)> = diagnostics
        .iter()
        .map(|entry| {
            (
                entry["path"].as_str().unwrap_or_default().to_owned(),
                entry["line"].as_u64().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        repeats,
        BTreeSet::from([
            ("book/both.md".to_owned(), 6),
            ("book/plan.md".to_owned(), 6),
            ("book/plan.md".to_owned(), 7),
            ("book/plan.md".to_owned(), 8),
        ]),
        "{diagnostics:?}"
    );
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_round_trip_clean(&run);
}

/// P4 (AC-06 "Attribution", accepted deviation 7): a bare feature-scoped
/// citation in a record the layout moved out of the feature document that
/// defines the subject (a list item of the checkout flow, to its record
/// file) is the layout's `mention-dangling`, like a moved link; a bare
/// citation of another feature document's criterion stays the source's,
/// as one outside any feature document does. Nothing is the emitter's.
/// (Pilot A before: one such finding was the emitter's.)
#[test]
fn p4_a_bare_criterion_citation_moved_out_of_its_feature_document_is_the_layout_s() {
    let scratch = Scratch::new("layout-i5-p4");
    let corpus = copy_of(ONE, &scratch, "corpus");
    edit(
        &corpus.join("book/checkout.md"),
        "Closing prose of the flow.\n",
        "- **ASK-030**: Does CK-01 hold for every cart?\n\
         - **ASK-031**: Does CK-03 of the sign-in flow hold here?\n\nClosing prose of the flow.\n",
    );
    edit(
        &corpus.join("book/flows/login.md"),
        "- **CK-02**: The user signs out.\n",
        "- **CK-02**: The user signs out.\n- **CK-03**: The user resets a password.\n",
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert_eq!(
        subjects_on(&run, "mention-dangling", "book/atoms/ASK/ASK-030.md"),
        [("CK-01".to_owned(), "layout".to_owned())]
    );
    assert_eq!(
        subjects_on(&run, "mention-dangling", "book/atoms/ASK/ASK-031.md"),
        [("CK-03".to_owned(), "source".to_owned())]
    );
    assert_eq!(
        subjects_on(&run, "mention-dangling", "book/glossary.md"),
        [("CK-01".to_owned(), "source".to_owned())]
    );
    let counts = &run.result()["check"]["findings"]["mention-dangling"];
    assert_eq!(counts["layout"], 1, "{counts}");
    assert_eq!(counts["emitter"], 0, "{counts}");
    assert_round_trip_clean(&run);
}

/// The census edit renaming `Cites` to the reference-list key `refs`.
const CITES_TO_REFS: [(&str, &str); 1] = [(
    KEY_MAP_ANCHOR,
    "\"Weight\" = \"weight\"\n\"Cites\" = \"refs\"\n",
)];

/// P5 (AC-05 "bad YAML is carried", accepted deviation 6): a YAML block
/// the before check cannot parse is carried verbatim — no rename, no value
/// map, no comma split — with only the missing `id` (a document record)
/// and `class` added above it. Its `frontmatter-yaml` stays the source's,
/// no `unparsed-reference` appears (the emitter repaired nothing), and a
/// document record under it is `header_unparseable`. The same keys in
/// valid YAML are renamed, mapped and split (the control). (Pilot B
/// before: 4 headers repaired, 9 `unparsed-reference` the emitter's.)
#[test]
fn p5_a_header_the_before_check_cannot_parse_is_carried_verbatim() {
    let scratch = Scratch::new("layout-i5-p5");
    let run = one_with(
        &scratch,
        &CITES_TO_REFS,
        &[
            (
                "book/bad.md",
                "---\nCites: see note: NEED-01, NEED-02\nStage: Settled\n---\n# Bad\n\nThe header above is no valid YAML.\n",
            ),
            (
                "book/bad-record.md",
                "---\nSerial: NEED-31\nCites: see: NEED-01, NEED-02\n---\n# Bad record\n\nA document record with a bad header.\n",
            ),
            (
                "book/good.md",
                "---\nCites: NEED-01, NEED-02\nStage: Settled\n---\n# Good\n\nThe same keys in valid YAML.\n",
            ),
        ],
    );
    assert_eq!(
        run.tree_text("book/bad.md"),
        "---\nclass: canon\nCites: see note: NEED-01, NEED-02\nStage: Settled\n---\n# Bad\n\nThe header above is no valid YAML.\n"
    );
    assert_eq!(
        run.tree_text("book/bad-record.md"),
        "---\nid: NEED-31\nclass: canon\nSerial: NEED-31\nCites: see: NEED-01, NEED-02\n---\n# Bad record\n\nA document record with a bad header.\n"
    );
    assert!(
        run.tree_text("book/good.md").starts_with(
            "---\nclass: canon\nrefs: [\"NEED-01\", \"NEED-02\"]\nstatus: \"accepted\"\n---\n"
        ),
        "{}",
        run.tree_text("book/good.md")
    );
    let unparsed: Vec<Value> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["code"] == "unparsed-reference")
        .collect();
    assert!(unparsed.is_empty(), "{unparsed:?}");
    for path in ["book/bad.md", "book/bad-record.md"] {
        assert_eq!(
            causes_of(&run, "frontmatter-yaml", path, ""),
            ["source"],
            "{path}"
        );
    }
    let record = record_of(&run, "NEED-31", "document");
    assert_eq!(record["reason"], "header_unparseable", "{record}");
    assert_eq!(run.count("reasons/header_unparseable"), 1);
    assert_eq!(run.count("header/unparseable"), 2, "{}", run.result());
    assert_eq!(
        run.count("hashes/matched"),
        run.count("before/definitions") - 1,
        "{}",
        run.result()
    );
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_round_trip_clean(&run);
}

/// P5's edge: a header block core reads but not as a mapping (a YAML
/// sequence; a bare sentence between two `---` lines, as a document
/// opening with a thematic break writes it) is the corpus's own defect
/// (`frontmatter-not-mapping` before). Carried with a `class` added above
/// it, the block turns into a different error, `frontmatter-yaml`, which
/// the attribution gives to the emitter (an error: enforce blocked).
/// Expected by AC-06 ("`emitter_findings` 0", nothing blocked by the
/// corpus's own header): no emitter finding, enforce clean.
#[test]
fn p5_a_header_core_reads_as_no_mapping_stays_the_source_s() {
    let scratch = Scratch::new("layout-i5-p5-mapping");
    let run = one_with(
        &scratch,
        &[],
        &[
            (
                "book/listed.md",
                "---\n- first\n- second\n---\n# Listed\n\nA header that is a list.\n",
            ),
            (
                "book/ruled.md",
                "---\nAn opening sentence between two rules.\n---\n# Ruled\n\nThe body.\n",
            ),
        ],
    );
    let emitter: Vec<Value> = run
        .tree_findings()
        .into_iter()
        .filter(|finding| finding["cause"] == "emitter")
        .collect();
    assert!(
        emitter.is_empty(),
        "emitter findings on the corpus's own non-mapping headers: {emitter:?}\nlisted:\n{}\nruled:\n{}",
        run.tree_text("book/listed.md"),
        run.tree_text("book/ruled.md")
    );
    assert_eq!(
        run.result()["check"]["enforce"],
        "clean",
        "{}",
        run.result()
    );
}

/// P6 (AC-06 "Attribution", `budget`): a tree file over its slot's cap
/// whose source document already exceeded it is the source's (baselined);
/// one the layout grew past the cap (a `class` header added to a source
/// just under it) stays the emitter's. Neither had a `budget` before: the
/// sources carry no class.
#[test]
fn p6_a_budget_over_a_cap_the_source_already_exceeded_is_the_source_s() {
    let scratch = Scratch::new("layout-i5-p6");
    let lines = |count: usize, words: &str| -> String {
        (0..count)
            .map(|line| format!("Line {line} of a decision {words}.\n"))
            .collect()
    };
    let near = format!("# Near the cap\n\n{}", lines(12, "close to the cap"));
    let cap = near.len() + 10;
    let big = format!(
        "# Over the cap\n\n{}",
        lines(20, "long enough to pass the cap")
    );
    assert!(big.len() > cap + 100);
    let corpus = copy_with(
        TWO,
        &scratch,
        "corpus",
        &[("log/big.md", big.as_str()), ("log/near.md", near.as_str())],
    );
    let scheme = corpus.join("specengine.toml");
    let text = fs::read_to_string(&scheme).unwrap();
    fs::write(
        &scheme,
        format!("{text}\n[budgets]\ndecision_bytes = {cap}\n"),
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert!(
        run.tree_text("log/near.md").len() > cap,
        "the layout grew it"
    );
    assert_eq!(
        subjects_on(&run, "budget", "log/big.md"),
        [("decision".to_owned(), "source".to_owned())]
    );
    assert_eq!(
        subjects_on(&run, "budget", "log/near.md"),
        [("decision".to_owned(), "emitter".to_owned())]
    );
    let budget = &run.result()["check"]["findings"]["budget"];
    assert_eq!(budget["before"], 0, "{budget}");
    assert_eq!(budget["source"], 1, "{budget}");
    assert_eq!(budget["emitter"], 1, "{budget}");
    assert_eq!(
        run.result()["check"]["baseline"]["per_code"]["budget"],
        1,
        "{}",
        run.result()
    );
    assert_eq!(run.count("check/emitter_findings"), 1, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "blocked");
}

// -------------------------------------------- iteration 5b (F-a, F-b)

/// A one-line text of exactly `bytes` bytes (no newline), opening with
/// `opening` and closing with a period.
fn text_of(bytes: usize, opening: &str) -> String {
    assert!(bytes > opening.len() + 1);
    format!("{opening}{}.", "x".repeat(bytes - opening.len() - 1))
}

/// The header the layout writes above a record file of fixture two
/// (`record_class = "decision"`), and above a residue that has none.
fn record_header(id: &str) -> String {
    format!("---\nid: {id}\nclass: decision\n---\n\n")
}
const RESIDUE_HEADER: &str = "---\nclass: decision\n---\n";

/// F-a (AC-06 "Attribution", `budget`; the reviewer's c4 pair): a record
/// file holds one record of its source, never the whole document, so its
/// source document's size says nothing of its own. Two record files the
/// layout's own header pushes over the decision cap are both the
/// emitter's: one from a document holding only it (under the cap), one
/// from a document of several records over the cap. The residue rules of
/// P6 still hold beside them: a residue whose source already exceeded the
/// cap is the source's, one the layout grew past it the emitter's. The
/// residues of both record documents stay under the cap.
#[test]
fn fa_a_record_file_over_the_cap_is_the_emitter_s_whatever_its_source_s_size() {
    let scratch = Scratch::new("layout-i5b-fa");
    let cap: usize = 1000;
    // A record file is its header, a blank, its text and a newline; the
    // record's text is chosen so the file passes the cap only by its header.
    let text_len = cap - 30;
    let solo_text = text_of(text_len, "The solo decision ");
    let first_text = text_of(text_len, "The first of many ");
    let solo = format!("# Solo\n\n- **DEC-0060** - {solo_text}\n");
    let many = format!(
        "# Many\n\n- **DEC-0061** - {first_text}\n- **DEC-0062** - The second of many.\n\nProse stays.\n"
    );
    assert!(solo.len() < cap, "solo.md is under the cap: {}", solo.len());
    assert!(many.len() > cap, "many.md is over the cap: {}", many.len());
    let near = format!("# Near the cap\n\n{}\n", text_of(cap - 27, "Near "));
    assert_eq!(near.len(), cap - 10);
    let big = format!("# Over the cap\n\n{}\n", text_of(cap + 200, "Over "));
    let corpus = copy_with(
        TWO,
        &scratch,
        "corpus",
        &[
            ("log/solo.md", solo.as_str()),
            ("log/many.md", many.as_str()),
            ("log/near.md", near.as_str()),
            ("log/big.md", big.as_str()),
        ],
    );
    let scheme = corpus.join("specengine.toml");
    let text = fs::read_to_string(&scheme).unwrap();
    fs::write(
        &scheme,
        format!("{text}\n[budgets]\ndecision_bytes = {cap}\n"),
    )
    .unwrap();
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    for (id, text) in [("DEC-0060", &solo_text), ("DEC-0061", &first_text)] {
        let path = format!("ledger/DEC/{id}.md");
        let written = run.tree_text(&path);
        assert_eq!(written, format!("{}{text}\n", record_header(id)), "{path}");
        assert!(written.len() > cap, "{path} is over the cap");
        assert!(
            written.len() - record_header(id).len() < cap,
            "{path} passes the cap only by its header"
        );
        assert_eq!(
            subjects_on(&run, "budget", &path),
            [("decision".to_owned(), "emitter".to_owned())],
            "{path}"
        );
    }
    assert_eq!(
        run.tree_text("log/near.md"),
        format!("{RESIDUE_HEADER}{near}"),
        "the layout grew the residue past the cap"
    );
    assert_eq!(
        subjects_on(&run, "budget", "log/near.md"),
        [("decision".to_owned(), "emitter".to_owned())]
    );
    assert_eq!(
        subjects_on(&run, "budget", "log/big.md"),
        [("decision".to_owned(), "source".to_owned())]
    );
    for path in ["ledger/DEC/DEC-0062.md", "log/solo.md", "log/many.md"] {
        assert!(run.tree_text(path).len() < cap, "{path}");
        assert!(subjects_on(&run, "budget", path).is_empty(), "{path}");
    }
    let budget = &run.result()["check"]["findings"]["budget"];
    assert_eq!(budget["before"], 0, "{budget}");
    assert_eq!(budget["source"], 1, "{budget}");
    assert_eq!(budget["emitter"], 3, "{budget}");
    assert_eq!(
        run.result()["check"]["baseline"]["per_code"]["budget"],
        1,
        "{}",
        run.result()
    );
    assert_eq!(run.count("check/emitter_findings"), 3, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "blocked");
}

/// F-b (AC-05 "bad YAML is carried"): a document record (its ID read from
/// the path) whose header core reads as no mapping — a YAML sequence, a
/// bare sentence between two `---` lines — is carried byte-verbatim with
/// no key added: neither the `id` a document record needs nor `class`.
/// Its record is `missing` for `header_unparseable` (core cannot read the
/// ID back), and every finding on the file is the source's
/// (`frontmatter-not-mapping`, baselined); nothing is the emitter's.
#[test]
fn fb_a_document_record_under_a_header_core_reads_as_no_mapping_takes_no_key() {
    let scratch = Scratch::new("layout-i5b-fb");
    let listed = "---\n- first\n- second\n---\n# Listed record\n\nA document record whose header is a list.\n";
    let ruled = "---\nAn opening sentence between two rules.\n---\n# Ruled record\n\nThe body.\n";
    let corpus = copy_with(
        TWO,
        &scratch,
        "corpus",
        &[("log/DEC-0070.md", listed), ("log/DEC-0071.md", ruled)],
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    for (id, source, kind) in [
        ("DEC-0070", listed, "sequence"),
        ("DEC-0071", ruled, "string"),
    ] {
        let path = format!("log/{id}.md");
        assert_eq!(run.tree_text(&path), source, "{path}: byte-verbatim");
        let record = record_of(&run, id, "document");
        assert_eq!(record["after"], path.as_str(), "{record}");
        assert_eq!(record["hash"], "missing", "{record}");
        assert_eq!(record["reason"], "header_unparseable", "{record}");
        let outcome: Vec<Value> = run.file("headers.json")["outcomes"]
            .as_array()
            .expect("headers.json outcomes")
            .iter()
            .filter(|outcome| outcome["path"] == path.as_str())
            .cloned()
            .collect();
        assert_eq!(outcome.len(), 1, "{outcome:?}");
        assert_eq!(outcome[0]["added"], serde_json::json!([]), "{outcome:?}");
        assert_eq!(outcome[0]["renamed"], 0, "{outcome:?}");
        let findings = findings_on(&run, "frontmatter-not-mapping", &path);
        assert_eq!(findings.len(), 1, "{path}: {findings:?}");
        assert!(
            findings[0]["message"]
                .as_str()
                .is_some_and(|message| message.contains(kind)),
            "{findings:?}"
        );
        let causes: BTreeSet<String> = run
            .tree_findings()
            .into_iter()
            .filter(|finding| finding["path"] == path.as_str())
            .map(|finding| finding["cause"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert_eq!(causes, BTreeSet::from(["source".to_owned()]), "{path}");
    }
    assert_eq!(
        run.count("reasons/header_unparseable"),
        2,
        "{}",
        run.result()
    );
    let counts = &run.result()["check"]["findings"]["frontmatter-not-mapping"];
    assert_eq!(counts["source"], 2, "{counts}");
    assert_eq!(counts["emitter"], 0, "{counts}");
    assert_eq!(
        run.result()["check"]["baseline"]["per_code"]["frontmatter-not-mapping"],
        2,
        "{}",
        run.result()
    );
    assert_eq!(run.count("check/emitter_findings"), 0, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "clean");
}

/// Accepted known limit (iteration 5b): a field table under a header core
/// reads as no mapping is appended after the verbatim block, inside its
/// delimiters; the block turns into invalid YAML, and that
/// `frontmatter-yaml` is the emitter's — visible (enforce blocked), never
/// a silent green. Pinned so a change of the rule shows here.
#[test]
fn a_field_table_under_a_header_core_reads_as_no_mapping_is_the_emitter_s_visible_limit() {
    let scratch = Scratch::new("layout-i5b-limit");
    let corpus = copy_with(
        TWO,
        &scratch,
        "corpus",
        &[(
            "log/tabled.md",
            "---\n- first\n- second\n---\n| Key | Value |\n|-----|-------|\n| Owner | ops |\n\n# Tabled\n\nA list header and a field table.\n",
        )],
    );
    let run = LayoutRun::new(&corpus, &scratch, "out", &[]);
    assert_eq!(
        run.tree_text("log/tabled.md"),
        "---\n- first\n- second\nsteward: \"ops\"\n---\n# Tabled\n\nA list header and a field table.\n"
    );
    assert_eq!(
        subjects_on(&run, "frontmatter-yaml", "log/tabled.md"),
        [(String::new(), "emitter".to_owned())]
    );
    assert!(
        findings_on(&run, "frontmatter-not-mapping", "log/tabled.md").is_empty(),
        "the before check's finding turned into another"
    );
    assert!(
        values_of(&run, "log/tabled.md").contains(&(
            "steward".to_owned(),
            "ops".to_owned(),
            String::new(),
            false
        )),
        "{:?}",
        values_of(&run, "log/tabled.md")
    );
    let counts = &run.result()["check"]["findings"]["frontmatter-yaml"];
    assert_eq!(counts["emitter"], 1, "{counts}");
    assert_eq!(run.count("check/emitter_findings"), 1, "{}", run.result());
    assert_eq!(run.result()["check"]["enforce"], "blocked");
}

/// P3's fallback (AC-05 "Headers"): a repeated field-table key whose
/// `<key>-2` the header already holds (written so in the YAML block) is
/// carried as `<key>-3`, its cell as written; each value reads back alike,
/// each carried name is the source's `unknown-key`, and the repeat is one
/// conflict naming the row's line and the name it took.
#[test]
fn p3_a_repeated_key_whose_key_2_is_taken_is_carried_as_key_3() {
    let scratch = Scratch::new("layout-i5b-p3-taken");
    let run = one_with(
        &scratch,
        &[],
        &[(
            "book/taken.md",
            "---\nKeeper: lead\nowner-2: second\n---\n| Attribute | Value |\n|-----------|-------|\n| Keeper | team-a |\n\n# Taken\n\nThe second name is taken.\n",
        )],
    );
    let taken = run.tree_text("book/taken.md");
    assert!(
        taken.starts_with(
            "---\nclass: canon\nowner: lead\nowner-2: second\nowner-3: \"team-a\"\n---\n"
        ),
        "{taken}"
    );
    let values = values_of(&run, "book/taken.md");
    assert!(
        values.contains(&(
            "owner-3".to_owned(),
            "team-a".to_owned(),
            "team-a".to_owned(),
            true
        )),
        "{values:?}"
    );
    assert!(
        values.iter().all(|(_, _, _, matched)| *matched),
        "{values:?}"
    );
    for key in ["owner-2", "owner-3"] {
        assert_eq!(
            causes_of(&run, "unknown-key", "book/taken.md", key),
            ["source"],
            "{key}"
        );
    }
    let repeats: Vec<Value> = run.file("diagnostics.json")["layout"]
        .as_array()
        .expect("layout diagnostics")
        .iter()
        .filter(|entry| entry["path"] == "book/taken.md")
        .cloned()
        .collect();
    assert_eq!(repeats.len(), 1, "{repeats:?}");
    assert_eq!(repeats[0]["line"], 7, "{repeats:?}");
    assert!(
        repeats[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("`owner-3`")),
        "{repeats:?}"
    );
    assert_eq!(run.count("header/conflicts"), 1, "{}", run.result());
    assert_eq!(run.count("fields/mismatched"), 0, "{}", run.result());
    assert_round_trip_clean(&run);
}

// --------------------------------------------------- AC-11 (owner-run)

/// AC-11 plumbing: the `#[ignore]` pilot runs of `layout` (owner-run, one
/// pilot at a time; the corpus, scheme and census config from the label's
/// variables, all outside the repository and read-only), and the same
/// helper on an invented setup so the plumbing itself is tested.
#[cfg(unix)]
mod pilots {
    use super::*;

    /// One `layout --label <label>` run through `pilot::run_read_only` (the
    /// proof over the scheme roots equal, the child with only `PATH`, the
    /// empty scratch `HOME` and the label's variables, exit 0, `HOME`
    /// empty); then the anonymous envelope and the AC-11 invariants.
    fn layout_run(label: &str, scratch: &Path, inputs: &pilot::Inputs) -> Value {
        let output = pilot::run_read_only("layout", label, scratch, inputs);
        let envelope = envelope(&output);
        assert_eq!(envelope["measurement"], "layout");
        assert_eq!(envelope["label"], label);
        let result = &envelope["result"];
        assert!(result.is_object(), "{label}: within --timeout: {result}");
        let mut found = Vec::new();
        leaves(result, "", &mut found);
        for (path, _) in &found {
            assert!(whitelisted(path), "{label}: {path} is not whitelisted");
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for leak in [".md", "/", "\\"] {
            assert!(!stdout.contains(leak), "{label}: {leak:?} on stdout");
        }
        let name = inputs.corpus.file_name().unwrap().to_string_lossy();
        assert!(
            !stdout.contains(name.as_ref()),
            "{label}: the corpus name on stdout"
        );
        let count = |path: &str| {
            let mut value = result;
            for part in path.split('/') {
                value = &value[part];
            }
            value.as_u64().unwrap_or_else(|| panic!("{label}: {path}"))
        };
        let listed = count("reasons/prefix_unknown")
            + count("reasons/slug")
            + count("reasons/path_taken")
            + count("reasons/section_fields")
            + count("reasons/header_unparseable")
            + count("reasons/reader_boundary");
        assert_eq!(count("reasons/unexplained"), 0, "{label}: {result}");
        assert_eq!(count("prose/mismatched"), 0, "{label}: {result}");
        let over_wide = over_wide_residue(label, &scratch.join("out"), inputs.corpus);
        assert_eq!(
            count("extents/residue"),
            u64::try_from(over_wide).unwrap(),
            "{label}: every extent residue is an over-wide row: {result}"
        );
        eprintln!("{label}: extent residue entries, all over-wide rows: {over_wide}");
        assert_eq!(count("check/emitter_findings"), 0, "{label}: {result}");
        assert_eq!(result["check"]["enforce"], "clean", "{label}: {result}");
        assert_eq!(
            count("hashes/matched"),
            count("before/definitions") - listed,
            "{label}: matched = definitions less the listed reasons: {result}"
        );
        let out = scratch.join("out");
        assert!(
            snapshot(&out)
                .into_keys()
                .all(|path| path.starts_with(Path::new("layout").join(label))),
            "{label}: everything under --out/layout/{label}"
        );
        eprintln!("{label} result: {result}");
        envelope
    }

    /// The cells of a pipe-table row: split at every `|` no backslash
    /// escapes (GFM splits inside a code span too), the outer pipes'
    /// empty cells dropped, each cell trimmed.
    fn row_cells(row: &str) -> Vec<String> {
        let row = row.trim();
        let mut cells = vec![String::new()];
        let mut escaped = false;
        for c in row.chars() {
            if c == '|' && !escaped {
                cells.push(String::new());
            } else {
                cells.last_mut().expect("one cell").push(c);
            }
            escaped = c == '\\' && !escaped;
        }
        if row.starts_with('|') {
            cells.remove(0);
        }
        if row.len() > 1 && row.ends_with('|') && !row.ends_with("\\|") {
            cells.pop();
        }
        cells
            .into_iter()
            .map(|cell| cell.trim().to_owned())
            .collect()
    }

    fn is_table_row(line: &str) -> bool {
        line.trim_start().starts_with('|')
    }

    fn is_delimiter(line: &str) -> bool {
        let cells = row_cells(line);
        is_table_row(line)
            && !cells.is_empty()
            && cells.iter().all(|cell| {
                let inner = cell.trim_start_matches(':').trim_end_matches(':');
                !inner.is_empty() && inner.chars().all(|c| c == '-')
            })
    }

    /// AC-11 under the owner's B.R1 decision: every `extents.json` entry
    /// is an over-wide row — a field-table or record row with more cells
    /// than its table's header (an unescaped `|` in the corpus) — and its
    /// residue is exactly those rows' cells past the header's width,
    /// nothing else. A field-table entry names the table's header line
    /// (`id` null; every over-wide row of the table counts), a record entry
    /// its row. Returns the number of entries; panics on any other residue.
    fn over_wide_residue(label: &str, out: &Path, corpus: &Path) -> usize {
        let detail = read_json(&out.join("layout").join(label).join("extents.json"));
        let entries = detail.as_array().expect("extents.json is an array");
        let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        for entry in entries {
            let path = entry["path"].as_str().expect("an extent's path");
            let line = entry["line"].as_u64().expect("an extent's line");
            let line = usize::try_from(line).unwrap();
            let residue = entry["residue"].as_str().expect("an extent's residue");
            let text = fs::read_to_string(corpus.join(path))
                .unwrap_or_else(|error| panic!("{label}: {path}: {error}"));
            let lines: Vec<&str> = text.trim_start_matches('\u{feff}').lines().collect();
            let at = |index: usize| lines.get(index).copied().unwrap_or_default();
            // (header index, row indices of the extent).
            let (header, rows): (Option<usize>, Vec<usize>) = if entry["id"].is_null() {
                let header = line - 1;
                let rows = (header + 2..lines.len())
                    .take_while(|&index| is_table_row(at(index)))
                    .collect();
                (is_delimiter(at(header + 1)).then_some(header), rows)
            } else {
                let row = line - 1;
                let delimiter = (0..row)
                    .rev()
                    .take_while(|&index| is_table_row(at(index)))
                    .find(|&index| is_delimiter(at(index)));
                (delimiter.and_then(|index| index.checked_sub(1)), vec![row])
            };
            let Some(header) = header.filter(|&index| is_table_row(at(index))) else {
                panic!("{label}: residue at {path}:{line} is not in a table with a header");
            };
            let width = row_cells(at(header)).len();
            let extra: Vec<String> = rows
                .iter()
                .map(|&index| row_cells(at(index)))
                .filter(|cells| cells.len() > width)
                .flat_map(|cells| cells.into_iter().skip(width))
                .collect();
            assert!(
                !extra.is_empty(),
                "{label}: residue at {path}:{line} has no row wider than its header ({width} cells)"
            );
            assert_eq!(
                words(residue),
                words(&extra.join(" ")),
                "{label}: residue at {path}:{line} is not exactly its over-wide rows' extra cells"
            );
        }
        entries.len()
    }

    fn pilot_from_environment(label: &str) {
        let variables = pilot::variables(label);
        let corpus = fs::canonicalize(pilot::required(variables.corpus, "the pilot corpus"))
            .unwrap_or_else(|error| panic!("{}: {error}", variables.corpus));
        let scheme = fs::canonicalize(pilot::required(variables.scheme, "the pilot scheme"))
            .unwrap_or_else(|error| panic!("{}: {error}", variables.scheme));
        let config = fs::canonicalize(pilot::required(
            variables.config,
            "the pilot census config with its [layout]",
        ))
        .unwrap_or_else(|error| panic!("{}: {error}", variables.config));
        for (variable, path) in [(variables.scheme, &scheme), (variables.config, &config)] {
            assert!(
                !path.starts_with(repository_root()),
                "{variable}: a pilot config lives outside the repository"
            );
        }
        let scratch = Scratch::new(label);
        assert!(!scratch.0.starts_with(&corpus));
        layout_run(
            label,
            &scratch.0,
            &pilot::Inputs {
                corpus: &corpus,
                scheme: &scheme,
                config: Some(&config),
            },
        );
    }

    /// The B.R1 rule on invented corpora: a field-table row wider than its
    /// header — a code span with two unescaped `|`, B.R1's shape — is
    /// accepted (its extra cells are the residue, counted); a key-less
    /// continuation row (residue, no row wider than the header) is not.
    /// (A record row's extra cell is carried as a field, no residue: the
    /// record branch has no invented case.)
    #[test]
    fn the_over_wide_rule_accepts_only_the_extra_cells_of_wide_rows() {
        let scratch = Scratch::new("layout-over-wide");
        let corpus = copy_of(TWO, &scratch, "corpus-wide");
        edit(
            &corpus.join("log/decisions.md"),
            "| Owner | ops |\n",
            "| Owner | `ops | dev | qa` |\n",
        );
        let run = LayoutRun::new(&corpus, &scratch, "out-wide", &[]);
        assert_eq!(run.count("extents/residue"), 1, "{}", run.result());
        assert_eq!(
            run.file("extents.json")[0]["residue"],
            "dev qa`",
            "cells 3-4 are the residue"
        );
        assert_eq!(
            over_wide_residue("pilot", &scratch.join("out-wide"), &corpus),
            1
        );
        let corpus = copy_of(TWO, &scratch, "corpus-continued");
        edit(
            &corpus.join("log/decisions.md"),
            "| Owner | ops |\n",
            "| Owner | ops |\n|  | orphan value with words |\n",
        );
        let run = LayoutRun::new(&corpus, &scratch, "out-continued", &[]);
        assert_eq!(run.count("extents/residue"), 1, "{}", run.result());
        let out = scratch.join("out-continued");
        let refused =
            std::panic::catch_unwind(|| over_wide_residue("pilot", &out, &corpus)).is_err();
        assert!(refused, "a continuation row is no over-wide row");
    }

    #[test]
    #[ignore = "needs SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CENSUS_CONFIG_A; read-only, owner-run"]
    fn pilot_a_layout_round_trips_read_only() {
        pilot_from_environment("pilot-a");
    }

    #[test]
    #[ignore = "needs SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CENSUS_CONFIG_B; read-only, owner-run"]
    fn pilot_b_layout_round_trips_read_only() {
        pilot_from_environment("pilot-b");
    }

    /// The helper on an invented setup: a `git init`ed copy of the first
    /// convention with both configs moved out of it (only the variables
    /// name them); `result` = its `expected.json`.
    #[test]
    fn the_pilot_helper_runs_layout_on_an_invented_setup_read_only() {
        for label in ["pilot-a", "pilot-b"] {
            let scratch = Scratch::new("layout-pilot-setup");
            let corpus = copy_of(ONE, &scratch, "corpus");
            let scheme = scratch.join("scheme-pilot.toml");
            let config = scratch.join("census-pilot.toml");
            fs::rename(corpus.join("specengine.toml"), &scheme).unwrap();
            fs::rename(corpus.join("census.toml"), &config).unwrap();
            let init = std::process::Command::new("git")
                .current_dir(&corpus)
                .args(["init", "-q"])
                .output()
                .expect("git runs");
            assert!(init.status.success(), "git init: {}", stderr(&init));
            let envelope = layout_run(
                label,
                &scratch.0,
                &pilot::Inputs {
                    corpus: &corpus,
                    scheme: &scheme,
                    config: Some(&config),
                },
            );
            assert_eq!(
                without_ms(&envelope["result"]),
                expected(ONE),
                "{label}: the configs came from the variables"
            );
        }
    }
}
