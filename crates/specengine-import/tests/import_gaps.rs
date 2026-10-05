//! docs/features/import-gaps.md, fixture ACs on the engine
//! (`specengine_import::import::run`; the census `specengine_import::run`):
//! AC-01 titled lead-ins, AC-02 document records, AC-03 document
//! precedence, AC-04 stripped code forms after `/`, AC-05 header outcomes,
//! AC-06 lines opening inside a comment, AC-07 the census pinned on a
//! generated corpus. AC-08 and AC-09 are end to end in eval
//! `tests/import_gaps_cli.rs`, `import_cli.rs` and `import_genre.rs`.
//!
//! The corpora are the two committed conventions, `fixtures/import-one`
//! (YAML headers, `**ID:**` lead-ins, `documents.id_key` reached through
//! `key_map`) and `fixtures/import-two` (field tables, `__ID -__` and
//! `__ID;__` lead-ins, `documents.id_key` and `documents.id_path`), read in
//! place or copied into a fresh temporary directory and edited there, each
//! with its own `census.toml`. Non-Latin characters are Unicode escapes
//! only (ADR-0024, `docs/canon/architecture.md` "Repository language").

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_import::import::{self, Form, Import, ImportRecord, MapOutcome, Role, Scope};
use specengine_import::{Census, CensusConfig, RecordKind};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn fixture_config(name: &str) -> String {
    fs::read_to_string(fixture(name).join("census.toml")).expect("fixture config")
}

fn parse(config: &str, root: &Path) -> CensusConfig {
    CensusConfig::parse(config, &root.join("census.toml")).expect("config parses")
}

fn import_at(root: &Path, config: &str) -> Import {
    import::run(root, &parse(config, root)).expect("import runs")
}

/// A fixture read in place: the engine only reads.
fn import_fixture(name: &str) -> Import {
    import_at(&fixture(name), &fixture_config(name))
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("copy target");
    for entry in fs::read_dir(from).expect("readable fixture") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            // The bytes, not `fs::copy`: on macOS it clones, which a sandbox refuses.
            fs::write(target, fs::read(entry.path()).expect("read")).expect("copy");
        }
    }
}

/// A corpus in a fresh temporary directory, removed on drop; unique per
/// process and per call.
struct Corpus {
    root: PathBuf,
    config: String,
}

impl Corpus {
    fn empty(case: &str, config: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "specengine-import-gaps-{case}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("corpus directory");
        Self {
            root,
            config: config.to_owned(),
        }
    }

    /// A copy of a fixture with its own config.
    fn copy(name: &str, case: &str) -> Self {
        let corpus = Self::empty(case, &fixture_config(name));
        copy_dir(&fixture(name), &corpus.root);
        corpus
    }

    fn write(&self, path: &str, text: &str) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().unwrap()).expect("document directory");
        fs::write(file, text).expect("document");
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path)).expect("document")
    }

    /// Replaces the one occurrence of `from` in a document.
    fn edit(&self, path: &str, from: &str, to: &str) {
        let text = self.read(path);
        assert_eq!(text.matches(from).count(), 1, "{path}: {from:?} once");
        self.write(path, &text.replacen(from, to, 1));
    }

    fn remove(&self, path: &str) {
        fs::remove_file(self.root.join(path)).expect("document removed");
    }

    fn import(&self) -> Import {
        import_at(&self.root, &self.config)
    }

    fn import_with(&self, config: &str) -> Import {
        import_at(&self.root, config)
    }

    fn census(&self) -> Census {
        specengine_import::run(&self.root, &parse(&self.config, &self.root)).expect("census runs")
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn blake3_hex(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

/// The one record of `id` in `path`.
fn record<'a>(import: &'a Import, path: &str, id: &str) -> &'a ImportRecord {
    let found: Vec<&ImportRecord> = import
        .records
        .iter()
        .filter(|record| record.path == path && record.id == id)
        .collect();
    assert_eq!(found.len(), 1, "{path}: one record {id}: {found:#?}");
    found[0]
}

/// The record at (path, line), if any.
fn record_at<'a>(import: &'a Import, path: &str, line: usize) -> Option<&'a ImportRecord> {
    import
        .records
        .iter()
        .find(|record| record.path == path && record.line == line)
}

fn ids_in(import: &Import, path: &str) -> Vec<String> {
    import
        .records
        .iter()
        .filter(|record| record.path == path)
        .map(|record| record.id.clone())
        .collect()
}

/// Every ID-like token the import read: claimed, unclaimed, outside.
fn tokens(import: &Import) -> usize {
    import.claimed + import.unclaimed.len()
}

/// The config without its `[documents]` table.
fn without_documents(config: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in config.lines() {
        if line.starts_with('[') {
            skipping = line.trim() == "[documents]";
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    assert_ne!(out, config, "the config had a [documents] table");
    out
}

/// A list convention of one fixture: where a test document goes, its list
/// marker and continuation indent, its strong delimiter, its separators,
/// how its IDs are written.
struct Lists {
    fixture: &'static str,
    document: &'static str,
    marker: &'static str,
    indent: &'static str,
    strong: &'static str,
    separators: &'static [&'static str],
    id: fn(u32) -> String,
}

const LISTS: [Lists; 2] = [
    Lists {
        fixture: "import-one",
        document: "spec/gaps.md",
        marker: "- ",
        indent: "  ",
        strong: "**",
        separators: &[":"],
        id: |n| format!("REQ-{n:03}"),
    },
    Lists {
        fixture: "import-two",
        document: "log/gaps.markdown",
        marker: "1. ",
        indent: "   ",
        strong: "__",
        separators: &[" -", ";"],
        id: |n| format!("DEC-{n:04}"),
    },
];

impl Lists {
    /// `**ID<sep> Title**<sep> text`
    fn titled(&self, separator: &str, id: &str, title: &str, text: &str) -> String {
        let s = self.strong;
        format!(
            "{}{s}{id}{separator} {title}{s}{separator} {text}",
            self.marker
        )
    }

    /// `**ID**<sep> text`
    fn untitled(&self, separator: &str, id: &str, text: &str) -> String {
        let s = self.strong;
        format!("{}{s}{id}{s}{separator} {text}", self.marker)
    }

    /// `**ID<sep>** text`
    fn separator_inside(&self, separator: &str, id: &str, text: &str) -> String {
        let s = self.strong;
        format!("{}{s}{id}{separator}{s} {text}", self.marker)
    }
}

// ----------------------------------------------------------------- AC-01

/// (path, line, ID, title) of one list item of a fixture.
type TitleCase = (&'static str, usize, &'static str, Option<&'static str>);

/// AC-01 in the committed fixtures: each titled lead-in is one `list-item`
/// record whose title is the span content after the ID and its separator,
/// as written (inline code kept); `**ID:**`, `**ID**:`, `__ID;__`,
/// `__ID__;` carry none; `titled` counts the titled records.
#[test]
fn ac01_the_fixtures_titled_lead_ins_keep_their_titles_as_written() {
    let cases: [(&str, &[TitleCase]); 2] = [
        (
            "import-one",
            &[
                ("spec/summary.md", 7, "REQ-012", Some("Export format")),
                ("spec/summary.md", 8, "REQ-013", Some("Legacy title")),
                ("spec/summary.md", 9, "REQ-015", Some("Short `name`")),
                ("spec/req-012.md", 10, "REQ-014", Some("Field order")),
                ("spec/feature-export.md", 9, "AC-004", Some("Retry")),
                ("spec/feature-export.md", 11, "AC-005", Some("Backoff")),
                ("spec/feature-export.md", 7, "AC-001", None),
                ("spec/feature-login.md", 13, "AC-002", None),
            ],
        ),
        (
            "import-two",
            &[
                ("pages/summary.mdown", 7, "DEC-0013", Some("Cache policy")),
                ("pages/summary.mdown", 8, "DEC-0014", Some("Legacy title")),
                ("log/criteria-b.markdown", 8, "CRT-0004", Some("Retry")),
                ("log/criteria-b.markdown", 10, "CRT-0005", Some("Backoff")),
                ("log/criteria-b.markdown", 6, "CRT-0001", None),
                ("log/criteria.markdown", 9, "CRT-0002", None),
            ],
        ),
    ];
    for (name, expected) in cases {
        let found = import_fixture(name);
        for &(path, line, id, title) in expected {
            let record = record_at(&found, path, line)
                .unwrap_or_else(|| panic!("{name}: no record at {path}:{line}"));
            assert_eq!(record.id, id, "{name}: {path}:{line}");
            assert_eq!(record.form, Form::ListItem, "{name}: {id}");
            assert_eq!(record.title.as_deref(), title, "{name}: {id}");
        }
        let titled = expected.iter().filter(|case| case.3.is_some()).count();
        assert_eq!(found.titled(), titled, "{name}: titled");
        assert_eq!(
            found
                .records
                .iter()
                .filter(|record| record.title.is_some())
                .count(),
            titled,
            "{name}: only list items carry a title"
        );
    }
}

/// AC-01, each fixture with each of its separators: a titled item's `text`
/// and `hash` are those of the same item without a title; editing the title
/// keeps the hash, one text byte changes it. M: the title kept in `text`.
#[test]
fn ac01_a_titled_item_hashes_like_the_untitled_one() {
    for lists in &LISTS {
        for (index, separator) in lists.separators.iter().enumerate() {
            let corpus = Corpus::copy(lists.fixture, "titled-hash");
            let titled = (lists.id)(201);
            let untitled = (lists.id)(202);
            let document = |title: &str, text: &str| {
                format!(
                    "# Gaps\n\n{}\n{}continued here.\n\n{}\n{}continued here.\n",
                    lists.titled(separator, &titled, title, text),
                    lists.indent,
                    lists.untitled(separator, &untitled, "the text line"),
                    lists.indent,
                )
            };
            let context = format!("{} separator {index}", lists.fixture);
            corpus.write(lists.document, &document("A title", "the text line"));
            let found = corpus.import();
            let a = record(&found, lists.document, &titled);
            let b = record(&found, lists.document, &untitled);
            let text = format!("the text line\n{}continued here.", lists.indent);
            assert_eq!(a.title.as_deref(), Some("A title"), "{context}");
            assert_eq!(b.title, None, "{context}");
            assert_eq!(a.text, text, "{context}");
            assert_eq!(b.text, text, "{context}");
            assert_eq!(a.hash, blake3_hex(&text), "{context}");
            assert_eq!(a.hash, b.hash, "{context}");
            let hash = a.hash.clone();

            corpus.write(
                lists.document,
                &document("Another, longer title", "the text line"),
            );
            let found = corpus.import();
            let edited = record(&found, lists.document, &titled);
            assert_eq!(
                edited.title.as_deref(),
                Some("Another, longer title"),
                "{context}"
            );
            assert_eq!(edited.hash, hash, "{context}: a title edit keeps the hash");

            corpus.write(lists.document, &document("A title", "the text lime"));
            let found = corpus.import();
            assert_ne!(
                record(&found, lists.document, &titled).hash,
                hash,
                "{context}: one text byte changes the hash"
            );
        }
    }
}

/// AC-01: `**ID**<sep>`, `**ID<sep>**` carry no title; a span ending with
/// its separator and holding no title strips no separator after it, a
/// titled span strips one.
#[test]
fn ac01_a_span_without_a_title_has_none_and_strips_as_before() {
    for lists in &LISTS {
        let separator = lists.separators[0];
        let corpus = Corpus::copy(lists.fixture, "untitled");
        let ids: Vec<String> = (211..215).map(lists.id).collect();
        corpus.write(
            lists.document,
            &format!(
                "# Gaps\n\n{}\n{}\n{}\n{}\n",
                lists.untitled(separator, &ids[0], "after the span"),
                lists.separator_inside(separator, &ids[1], "inside the span"),
                lists.separator_inside(separator, &ids[2], &format!("{separator} kept")),
                lists.titled(separator, &ids[3], "T", &format!("{separator} kept")),
            ),
        );
        let found = corpus.import();
        let kept = format!("{separator} kept").trim_start().to_owned();
        for (id, title, text) in [
            (&ids[0], None, "after the span"),
            (&ids[1], None, "inside the span"),
            (&ids[2], None, kept.as_str()),
            (&ids[3], Some("T"), kept.as_str()),
        ] {
            let record = record(&found, lists.document, id);
            assert_eq!(record.title.as_deref(), title, "{}: {id}", lists.fixture);
            assert_eq!(record.text, text, "{}: {id}", lists.fixture);
        }
    }
}

/// AC-01: the ID is the longest leading part that resolves, so a separator
/// inside the ID (here `-`, added to the fixture's separators) and a
/// separator inside the title stay where they are; a span with no
/// resolving part is no record and no unmapped legacy ID. M: the span read
/// only as ID [+ separator].
#[test]
fn ac01_the_longest_resolving_part_is_the_id() {
    for lists in &LISTS {
        let separator = lists.separators[0];
        let corpus = Corpus::copy(lists.fixture, "longest");
        let dash = (lists.id)(221);
        let parted = (lists.id)(222);
        let s = lists.strong;
        corpus.write(
            lists.document,
            &format!(
                "# Gaps\n\n\
                 {m}{s}{dash}-Dash title{s}{separator} text a\n\
                 {}\n\
                 {m}{s}Not an ID{separator} A title{s}{separator} text c\n",
                lists.titled(
                    separator,
                    &parted,
                    &format!("Part{separator} two"),
                    "text b"
                ),
                m = lists.marker,
            ),
        );
        let separators: Vec<String> = lists
            .separators
            .iter()
            .chain(["-"].iter())
            .map(|separator| format!("{separator:?}"))
            .collect();
        let old = format!(
            "separators = [{}]",
            lists
                .separators
                .iter()
                .map(|separator| format!("{separator:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert_eq!(corpus.config.matches(&old).count(), 1, "{old}");
        let with_dash = corpus
            .config
            .replace(&old, &format!("separators = [{}]", separators.join(", ")));

        let found = corpus.import_with(&with_dash);
        let record_a = record(&found, lists.document, &dash);
        assert_eq!(record_a.title.as_deref(), Some("Dash title"));
        assert_eq!(record_a.text, "text a");
        assert_eq!(
            record_at(&found, lists.document, 5),
            None,
            "{}",
            lists.fixture
        );

        let found = corpus.import();
        let record_b = record(&found, lists.document, &parted);
        assert_eq!(
            record_b.title.as_deref(),
            Some(format!("Part{separator} two").as_str()),
            "{}",
            lists.fixture
        );
        assert_eq!(record_b.text, "text b");
        assert_eq!(
            ids_in(&found, lists.document),
            std::slice::from_ref(&parted)
        );
        assert!(
            found
                .legacy
                .unmapped
                .iter()
                .all(|token| token.path != lists.document),
            "{}: {:?}",
            lists.fixture,
            found.legacy.unmapped
        );
    }
}

/// AC-01, `docs/canon/import.md` "Titled lead-ins": the title is the span
/// content after the ID and its separator as written: inline markup,
/// escapes and a comment inside the span kept; the text is unaffected.
#[test]
fn ac01_the_title_keeps_markup_escapes_and_comments() {
    for lists in &LISTS {
        for separator in lists.separators {
            let corpus = Corpus::copy(lists.fixture, "title-as-written");
            let id = (lists.id)(231);
            let title = "A `code` \\* <!-- note --> *em* title";
            corpus.write(
                lists.document,
                &format!(
                    "# Gaps\n\n{}\n",
                    lists.titled(separator, &id, title, "the text")
                ),
            );
            let found = corpus.import();
            let record = record(&found, lists.document, &id);
            assert_eq!(record.title.as_deref(), Some(title), "{}", lists.fixture);
            assert_eq!(record.text, "the text", "{}", lists.fixture);
        }
    }
}

/// AC-01 in the fixtures: a titled record item nested in a record item ends
/// its parent's text and is a record of its own.
#[test]
fn ac01_a_nested_titled_record_item_ends_its_parent() {
    for (name, path, parent, child, text, child_text) in [
        (
            "import-one",
            "spec/feature-export.md",
            "AC-004",
            "AC-005",
            "Export retries once\n  after a failed write.",
            "The retry waits one second.",
        ),
        (
            "import-two",
            "log/criteria-b.markdown",
            "CRT-0004",
            "CRT-0005",
            "The exporter retries once\n   after a failed write.",
            "The retry waits one second.",
        ),
    ] {
        let found = import_fixture(name);
        assert_eq!(record(&found, path, parent).text, text, "{name}");
        assert_eq!(record(&found, path, child).text, child_text, "{name}");
    }
}

/// AC-01 in the fixtures: a legacy titled ID is the Latin ID with the
/// written one as alias and one `legacy.mapped` entry at its line.
#[test]
fn ac01_a_legacy_titled_id_is_the_latin_id_with_an_alias() {
    for (name, path, line, written, id) in [
        ("import-one", "spec/summary.md", 8, "SR-013", "REQ-013"),
        (
            "import-two",
            "pages/summary.mdown",
            8,
            "OLD-0014",
            "DEC-0014",
        ),
    ] {
        let found = import_fixture(name);
        let record = record(&found, path, id);
        assert_eq!(record.aliases, [written.to_owned()], "{name}");
        assert_eq!(record.line, line);
        assert!(record.title.is_some());
        let mapped: Vec<(&str, usize, &str, &str)> = found
            .legacy
            .mapped
            .iter()
            .filter(|change| change.path == path)
            .map(|change| {
                (
                    change.path.as_str(),
                    change.line,
                    change.written.as_str(),
                    change.id.as_str(),
                )
            })
            .collect();
        assert_eq!(mapped, [(path, line, written, id)], "{name}");
    }
}

/// AC-01 in the fixtures: a feature-scoped titled ID is a feature record,
/// and its citations in the same document are claimed (removing them
/// claims two tokens fewer, nothing becomes unclaimed).
#[test]
fn ac01_a_feature_scoped_titled_id_claims_its_citations_in_the_document() {
    for (name, path, ids, citation) in [
        (
            "import-one",
            "spec/feature-export.md",
            ["AC-004", "AC-005"],
            "\nRetries follow AC-004 and AC-005.\n",
        ),
        (
            "import-two",
            "log/criteria-b.markdown",
            ["CRT-0004", "CRT-0005"],
            "\nRetries follow CRT-0004 and CRT-0005.\n",
        ),
    ] {
        let corpus = Corpus::copy(name, "feature-titled");
        let found = corpus.import();
        for id in ids {
            let record = record(&found, path, id);
            assert_eq!(record.scope, Scope::Feature, "{name}: {id}");
            assert_eq!(record.role, Role::Definition, "{name}: {id}");
        }
        assert!(
            found.unclaimed.iter().all(|token| token.path != path),
            "{name}: {:?}",
            found.unclaimed
        );
        corpus.edit(path, citation, "\n");
        let without = corpus.import();
        assert_eq!(without.claimed + 2, found.claimed, "{name}");
        assert_eq!(without.unclaimed.len(), found.unclaimed.len(), "{name}");
    }
}

// ----------------------------------------------------------------- AC-02

/// AC-02 in the fixtures: a YAML key mapped to `id_key` (import-one), a
/// field-table key and an `id_path` match (import-two) each give one
/// `document` record: `line` the key's (1 for the path), `text` the file
/// after its header (headings before a field table kept), `hash` its
/// BLAKE3, no title, no fields.
#[test]
fn ac02_the_fixtures_document_records() {
    let cases = [
        (
            "import-one",
            "spec/req-012.md",
            "REQ-012",
            2,
            "# Export format\n\nThe export writes one JSON object per record.\n\n\
             - **REQ-014: Field order**: Fields keep the register's order.",
        ),
        (
            "import-two",
            "pages/dec-0012.mdown",
            "DEC-0012",
            5,
            "# Report layout\n\nThe report lists every decision once.",
        ),
        (
            "import-two",
            "log/DEC-0013.markdown",
            "DEC-0013",
            1,
            "# Cache policy\n\nThe cache keeps one entry per run; the report layout is DEC-0012.",
        ),
    ];
    for (name, path, id, line, text) in cases {
        let found = import_fixture(name);
        let record = record(&found, path, id);
        assert_eq!(record.form, Form::Document, "{name}: {id}");
        assert_eq!(record.line, line, "{name}: {id}");
        assert_eq!(record.text, text, "{name}: {id}");
        assert_eq!(record.hash, blake3_hex(text), "{name}: {id}");
        assert_eq!(record.role, Role::Definition, "{name}: {id}");
        assert_eq!(record.scope, Scope::Project, "{name}: {id}");
        assert_eq!(record.title, None, "{name}: {id}");
        assert!(record.fields.is_empty() && record.aliases.is_empty());
    }
    for (name, documents) in [("import-one", 1), ("import-two", 2)] {
        let found = import_fixture(name);
        assert_eq!(found.per_form(Form::Document), documents, "{name}");
        assert!(
            found
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("documents.")),
            "{name}: {:?}",
            found.diagnostics
        );
    }
}

/// AC-02: a CRLF copy, a BOM, an extra blank line after the header and an
/// edited header value keep a document's hash; one body byte changes it.
/// M: the hash over the whole file.
#[test]
fn ac02_the_document_hash_is_the_body_only() {
    // (fixture, document, ID, an extra blank after the header, a header
    // value edit, a body edit)
    let cases = [
        (
            "import-one",
            "spec/req-012.md",
            "REQ-012",
            Some(("---\n\n#", "---\n\n\n#")),
            Some(("owner: export team", "owner: another team")),
            ("one JSON object", "one JSON objects"),
        ),
        (
            "import-two",
            "pages/dec-0012.mdown",
            "DEC-0012",
            Some(("| Agreed |\n", "| Agreed |\n\n")),
            Some(("| Agreed |", "| Pending |")),
            ("every decision", "every decisions"),
        ),
        (
            "import-two",
            "log/DEC-0013.markdown",
            "DEC-0013",
            Some(("# Cache", "\n\n# Cache")),
            None,
            ("one entry", "one entries"),
        ),
    ];
    for (name, path, id, blank, header, body) in cases {
        let corpus = Corpus::copy(name, "document-hash");
        let original = corpus.read(path);
        let base = record(&corpus.import(), path, id).hash.clone();
        let mut same: Vec<(&str, String)> = vec![("crlf", original.replace('\n', "\r\n"))];
        // A BOM before a field table: `ac02_a_bom_before_a_field_table_is_excluded`.
        if !path.starts_with("pages/") {
            same.push(("bom", format!("\u{FEFF}{original}")));
        }
        if let Some((from, to)) = blank {
            same.push(("extra blank", original.replacen(from, to, 1)));
        }
        if let Some((from, to)) = header {
            same.push(("header value", original.replacen(from, to, 1)));
        }
        for (case, text) in same {
            assert_ne!(text, original, "{name}: {case} changes the file");
            corpus.write(path, &text);
            let found = corpus.import();
            assert_eq!(record(&found, path, id).hash, base, "{name} {path}: {case}");
        }
        corpus.write(path, &original.replacen(body.0, body.1, 1));
        assert_ne!(
            record(&corpus.import(), path, id).hash,
            base,
            "{name} {path}: one body byte"
        );
    }
}

/// AC-02, `docs/canon/import.md` "Text and hash" (BOM skipped): a BOM
/// before a field-table header, the table first or after a heading, leaves
/// the document record, its line and its hash as they are without it.
#[test]
fn ac02_a_bom_before_a_field_table_is_excluded() {
    let corpus = Corpus::copy("import-two", "bom-field-table");
    corpus.write(
        "pages/plain.mdown",
        "| Title | Plain |\n|---|---|\n| Doc | DEC-0040 |\n\nBody.\n",
    );
    let mut failures = Vec::new();
    for (path, id, line) in [
        ("pages/plain.mdown", "DEC-0040", 3),
        ("pages/dec-0012.mdown", "DEC-0012", 5),
    ] {
        let base = record(&corpus.import(), path, id).clone();
        assert_eq!((base.form, base.line), (Form::Document, line), "{path}");
        let original = corpus.read(path);
        corpus.write(path, &format!("\u{FEFF}{original}"));
        let found = corpus.import();
        let with_bom: Vec<(Form, usize, String)> = found
            .records
            .iter()
            .filter(|record| record.path == path && record.id == id)
            .map(|record| (record.form, record.line, record.hash.clone()))
            .collect();
        if with_bom != [(Form::Document, line, base.hash.clone())] {
            failures.push(format!(
                "{path}: with a BOM {with_bom:?}, header {:?}",
                found
                    .documents_detail
                    .iter()
                    .find(|document| document.path == path)
                    .map(|document| document.header)
            ));
        }
        corpus.write(path, &original);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-02: the value defining a document's ID is no ID-like token: without
/// `[documents]` the same corpus reads exactly one token more, at that
/// line; a citation elsewhere is claimed. M: the defining value counted as
/// a token.
#[test]
fn ac02_the_defining_value_is_no_token_and_citations_are_claimed() {
    for (name, id, path, line, citing, citation) in [
        (
            "import-one",
            "REQ-012",
            "spec/req-012.md",
            2,
            "spec/notes.md",
            "\nThe export format REQ-012 is settled.\n",
        ),
        (
            "import-two",
            "DEC-0012",
            "pages/dec-0012.mdown",
            5,
            "log/DEC-0013.markdown",
            "; the report layout is DEC-0012.",
        ),
    ] {
        let corpus = Corpus::copy(name, "defining-value");
        let with = corpus.import();
        let without = corpus.import_with(&without_documents(&corpus.config));
        assert_eq!(tokens(&without), tokens(&with) + 1, "{name}");
        assert!(
            with.unclaimed
                .iter()
                .all(|token| !(token.path == path && token.line == line)),
            "{name}: {:?}",
            with.unclaimed
        );
        assert!(
            with.unclaimed.iter().all(|token| token.token != id),
            "{name}: the citation is claimed"
        );
        corpus.edit(
            citing,
            citation,
            if name == "import-one" { "\n" } else { "." },
        );
        let uncited = corpus.import();
        assert_eq!(uncited.claimed + 1, with.claimed, "{name}");
        assert_eq!(uncited.unclaimed.len(), with.unclaimed.len(), "{name}");
    }
}

/// AC-02: a legacy header value or path group gives the Latin ID, the
/// written one as alias and one more `legacy.mapped`.
#[test]
fn ac02_a_legacy_document_id_maps_with_an_alias() {
    let one = Corpus::copy("import-one", "legacy-document");
    one.edit("spec/req-012.md", "Number: REQ-012", "Number: SR-012");
    let two = Corpus::copy("import-two", "legacy-document");
    two.edit(
        "pages/dec-0012.mdown",
        "| Doc | DEC-0012 |",
        "| Doc | OLD-0012 |",
    );
    let two_path = Corpus::copy("import-two", "legacy-path");
    let moved = two_path.read("log/DEC-0013.markdown");
    two_path.remove("log/DEC-0013.markdown");
    two_path.write("log/OLD-0013.markdown", &moved);
    for (name, corpus, path, line, written, id) in [
        (
            "import-one",
            &one,
            "spec/req-012.md",
            2,
            "SR-012",
            "REQ-012",
        ),
        (
            "import-two",
            &two,
            "pages/dec-0012.mdown",
            5,
            "OLD-0012",
            "DEC-0012",
        ),
        (
            "import-two",
            &two_path,
            "log/OLD-0013.markdown",
            1,
            "OLD-0013",
            "DEC-0013",
        ),
    ] {
        let base = import_fixture(name);
        let found = corpus.import();
        let record = record(&found, path, id);
        assert_eq!(record.form, Form::Document, "{path}");
        assert_eq!(record.aliases, [written.to_owned()], "{path}");
        assert_eq!(record.line, line, "{path}");
        assert_eq!(
            found.legacy.mapped.len(),
            base.legacy.mapped.len() + 1,
            "{path}"
        );
        assert!(
            found
                .legacy
                .mapped
                .iter()
                .any(|change| change.path == path && change.written == written && change.id == id),
            "{path}"
        );
    }
}

/// AC-02: header and path naming different IDs → the header's ID, one
/// diagnostic at the key's line; agreeing → no diagnostic.
#[test]
fn ac02_header_and_path_disagreeing_take_the_header_with_a_diagnostic() {
    let corpus = Corpus::copy("import-two", "disagree");
    corpus.write(
        "log/DEC-0020.markdown",
        "| Title | Clash |\n|---|---|\n| Doc | DEC-0021 |\n\nThe header and the path disagree.\n",
    );
    corpus.write(
        "log/DEC-0022.markdown",
        "| Title | Agree |\n|---|---|\n| Doc | DEC-0022 |\n\nThe header and the path agree.\n",
    );
    let base = import_fixture("import-two");
    let found = corpus.import();
    assert_eq!(ids_in(&found, "log/DEC-0020.markdown"), ["DEC-0021"]);
    let clash = record(&found, "log/DEC-0020.markdown", "DEC-0021");
    assert_eq!((clash.form, clash.line), (Form::Document, 3));
    let agree = record(&found, "log/DEC-0022.markdown", "DEC-0022");
    assert_eq!((agree.form, agree.line), (Form::Document, 3));
    assert_eq!(found.diagnostics.len(), base.diagnostics.len() + 1);
    let new: Vec<(&str, Option<usize>)> = found
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.path.starts_with("log/DEC-002"))
        .map(|diagnostic| (diagnostic.path.as_str(), diagnostic.line))
        .collect();
    assert_eq!(new, [("log/DEC-0020.markdown", Some(3))]);
}

/// AC-02 rules: two header keys reaching `id_key` → the first one's value,
/// one diagnostic; a header value that is no ID → one diagnostic, then
/// `id_path`.
#[test]
fn ac02_a_second_key_or_an_unresolving_value_is_one_diagnostic() {
    let corpus = Corpus::copy("import-two", "id-key-diagnostics");
    corpus.write(
        "pages/two-keys.mdown",
        "| Title | Two keys |\n|---|---|\n| Doc | DEC-0051 |\n| docid | DEC-0052 |\n\nBody.\n",
    );
    corpus.write(
        "log/DEC-0053.markdown",
        "| Title | No ID |\n|---|---|\n| Doc | not an ID |\n\nBody.\n",
    );
    let base = import_fixture("import-two");
    let found = corpus.import();
    let first = record(&found, "pages/two-keys.mdown", "DEC-0051");
    assert_eq!((first.form, first.line), (Form::Document, 3));
    assert_eq!(ids_in(&found, "pages/two-keys.mdown"), ["DEC-0051"]);
    let path = record(&found, "log/DEC-0053.markdown", "DEC-0053");
    assert_eq!((path.form, path.line), (Form::Document, 1));
    let new: Vec<(&str, Option<usize>)> = found
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.path == "pages/two-keys.mdown" || diagnostic.path == "log/DEC-0053.markdown"
        })
        .map(|diagnostic| (diagnostic.path.as_str(), diagnostic.line))
        .collect();
    assert_eq!(
        new,
        [
            ("log/DEC-0053.markdown", Some(3)),
            ("pages/two-keys.mdown", Some(4)),
        ]
    );
    assert_eq!(found.diagnostics.len(), base.diagnostics.len() + 2);
}

/// AC-02: a feature-scoped document ID (a header value, a path group) gives
/// one diagnostic and no record.
#[test]
fn ac02_a_feature_scoped_document_id_is_a_diagnostic_and_no_record() {
    let one = Corpus::copy("import-one", "feature-document");
    one.write(
        "spec/feature-doc.md",
        "---\nNumber: AC-009\n---\n# Feature\n\nBody.\n",
    );
    let two = Corpus::copy("import-two", "feature-document");
    two.write("log/CRT-0009.markdown", "# Feature\n\nBody.\n");
    for (name, corpus, path, line) in [
        ("import-one", &one, "spec/feature-doc.md", Some(2)),
        ("import-two", &two, "log/CRT-0009.markdown", None),
    ] {
        let base = import_fixture(name);
        let found = corpus.import();
        assert!(ids_in(&found, path).is_empty(), "{path}");
        assert_eq!(found.records.len(), base.records.len(), "{path}");
        assert_eq!(
            found.diagnostics.len(),
            base.diagnostics.len() + 1,
            "{path}"
        );
        let new: Vec<Option<usize>> = found
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.path == path)
            .map(|diagnostic| diagnostic.line)
            .collect();
        assert_eq!(new, [line], "{path}");
    }
}

/// AC-02: in a `reference_paths` document the document record is a
/// reference.
#[test]
fn ac02_a_document_record_in_a_reference_path_is_a_reference() {
    let one = Corpus::copy("import-one", "reference-document");
    let map = one.read("spec/map.md");
    one.write("spec/map.md", &format!("---\nNumber: REQ-016\n---\n{map}"));
    let two = Corpus::copy("import-two", "reference-document");
    two.write(
        "pages/refs/dec-0016.mdown",
        "| Title | Cited |\n|---|---|\n| Doc | DEC-0016 |\n\nA cited decision.\n",
    );
    for (name, corpus, path, id, line) in [
        ("import-one", &one, "spec/map.md", "REQ-016", 2),
        (
            "import-two",
            &two,
            "pages/refs/dec-0016.mdown",
            "DEC-0016",
            3,
        ),
    ] {
        let found = corpus.import();
        let record = record(&found, path, id);
        assert_eq!(
            (record.form, record.role, record.line),
            (Form::Document, Role::Reference, line),
            "{name}"
        );
        assert!(
            found.unresolved.iter().any(|token| token.token == id),
            "{name}: a reference defined nowhere"
        );
    }
}

/// AC-02: an unclosed YAML block is no header: the `id_path` ID, `text`
/// from line 1 (a key inside the block is not read).
#[test]
fn ac02_an_unclosed_yaml_block_leaves_the_path_id_and_the_whole_text() {
    let two = Corpus::copy("import-two", "unclosed");
    two.write(
        "log/DEC-0023.markdown",
        "---\ntype: draft\nDoc: DEC-0024\n\n# Unclosed\n\nBody.\n",
    );
    let one = Corpus::copy("import-one", "unclosed");
    one.write(
        "spec/REQ-023.md",
        "---\nNumber: REQ-024\n\n# Unclosed\n\nBody.\n",
    );
    let one_config = one.config.replace(
        "id_key = \"ident\"",
        "id_key = \"ident\"\nid_path = '^spec/(?P<id>[A-Z]{3}-[0-9]{3})\\.md$'",
    );
    assert_ne!(one_config, one.config);
    for (name, found, path, id, text) in [
        (
            "import-two",
            two.import(),
            "log/DEC-0023.markdown",
            "DEC-0023",
            "---\ntype: draft\nDoc: DEC-0024\n\n# Unclosed\n\nBody.",
        ),
        (
            "import-one",
            one.import_with(&one_config),
            "spec/REQ-023.md",
            "REQ-023",
            "---\nNumber: REQ-024\n\n# Unclosed\n\nBody.",
        ),
    ] {
        assert_eq!(ids_in(&found, path), [id.to_owned()], "{name}");
        let record = record(&found, path, id);
        assert_eq!((record.form, record.line), (Form::Document, 1), "{name}");
        assert_eq!(record.text, text, "{name}");
        assert!(
            found
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path == path && diagnostic.line == Some(1)),
            "{name}: the unclosed block's diagnostic"
        );
    }
}

/// AC-02 "no defaults": without `[documents]` no document is a record,
/// whatever its header keys or path are called.
#[test]
fn ac02_without_documents_no_document_record() {
    for (name, path, text) in [
        (
            "import-one",
            "spec/REQ-030.md",
            "---\nid: REQ-030\nident: REQ-031\nNumber: REQ-032\nID: REQ-033\n---\n# Keys\n",
        ),
        (
            "import-two",
            "log/DEC-0030.markdown",
            "| Title | Keys |\n|---|---|\n| id | DEC-0031 |\n| docid | DEC-0032 |\n| Doc | DEC-0033 |\n",
        ),
    ] {
        let corpus = Corpus::copy(name, "no-documents");
        corpus.write(path, text);
        let config = without_documents(&corpus.config);
        let found = corpus.import_with(&config);
        assert_eq!(found.per_form(Form::Document), 0, "{name}");
        let base = import_at(&fixture(name), &config);
        assert_eq!(found.diagnostics.len(), base.diagnostics.len(), "{name}");
    }
}

/// The diagnostics of one path: (line, message).
fn diagnostics_in(import: &Import, path: &str) -> Vec<(Option<usize>, String)> {
    import
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.path == path)
        .map(|diagnostic| (diagnostic.line, diagnostic.message.clone()))
        .collect()
}

/// AC-02 rules ("a header key's target after `key_map` equals" `id_key`):
/// a key written as the `id_key` target itself, with no `key_map` entry of
/// its own, defines the document: a YAML key and a field-table key in the
/// fixtures, and both under a config with no `key_map` at all.
#[test]
fn ac02_a_literal_id_key_key_without_a_key_map_entry_defines_the_document() {
    let one = Corpus::copy("import-one", "literal-id-key");
    one.write(
        "spec/literal.md",
        "---\nkind: note\nident: REQ-061\n---\n# Literal\n\nBody.\n",
    );
    let two = Corpus::copy("import-two", "literal-id-key");
    two.write(
        "pages/literal.mdown",
        "| Title | Literal |\n|---|---|\n| docid | DEC-0061 |\n\nBody.\n",
    );
    let bare_config = "[corpus]\nroots = [\"spec\"]\n\
        [front_matter]\nheader_table = '^Field$'\n\
        [ids]\nregex = '^[A-Z]{2,3}-[0-9]{3}$'\n\
        [documents]\nid_key = \"docnum\"\n";
    let bare = Corpus::empty("literal-id-key-bare", bare_config);
    bare.write(
        "spec/yaml.md",
        "---\ndocnum: AB-061\n---\n# Yaml\n\nBody.\n",
    );
    bare.write(
        "spec/table.md",
        "| Field | Value |\n|---|---|\n| docnum | AB-062 |\n\nBody.\n",
    );
    let found_one = one.import();
    let found_two = two.import();
    let found_bare = bare.import();
    for (found, path, id, line, text) in [
        (
            &found_one,
            "spec/literal.md",
            "REQ-061",
            3,
            "# Literal\n\nBody.",
        ),
        (&found_two, "pages/literal.mdown", "DEC-0061", 3, "Body."),
        (&found_bare, "spec/yaml.md", "AB-061", 2, "# Yaml\n\nBody."),
        (&found_bare, "spec/table.md", "AB-062", 3, "Body."),
    ] {
        assert_eq!(ids_in(found, path), [id], "{path}");
        let record = record(found, path, id);
        assert_eq!(
            (record.form, record.line, record.role, record.text.as_str()),
            (Form::Document, line, Role::Definition, text),
            "{path}"
        );
        assert_eq!(diagnostics_in(found, path), [], "{path}");
    }
    for (name, found) in [("import-one", &found_one), ("import-two", &found_two)] {
        let base = import_fixture(name);
        assert_eq!(
            found.per_form(Form::Document),
            base.per_form(Form::Document) + 1,
            "{name}"
        );
        assert_eq!(found.diagnostics.len(), base.diagnostics.len(), "{name}");
    }
}

/// `docs/canon/import.md` "Text and hash": `document_text` (public, what
/// `import-layout` normalises an emitted body with) drops a leading BOM, a
/// CR before an LF and leading lines of only spaces and tabs, trims
/// trailing spaces, tabs, CRs and LFs, and changes nothing else: a line of
/// other Unicode white space (NBSP, U+3000) is text at either end; inner
/// blanks and a content line's own indentation stay.
#[test]
fn ac02_document_text_trims_only_markdown_blanks() {
    let nbsp = "\u{00A0}";
    let ideographic = "\u{3000}";
    let cases: Vec<(&str, String, String)> = vec![
        // Kept: other white space at the start and the end.
        (
            "NBSP-only first line",
            format!("{nbsp}\nBody."),
            format!("{nbsp}\nBody."),
        ),
        (
            "U+3000-only first line",
            format!("{ideographic}\nBody."),
            format!("{ideographic}\nBody."),
        ),
        (
            "NBSP-only last line",
            format!("Body.\n{nbsp}\n"),
            format!("Body.\n{nbsp}"),
        ),
        (
            "U+3000 ending the last line",
            format!("Body.{ideographic}\n"),
            format!("Body.{ideographic}"),
        ),
        (
            "U+3000-only last line before blanks",
            format!("Body.\n{ideographic}\n \t\n"),
            format!("Body.\n{ideographic}"),
        ),
        (
            "NBSP after blank lines",
            format!(" \n\t\n{nbsp}x\n"),
            format!("{nbsp}x"),
        ),
        // Dropped and trimmed: Markdown blanks only.
        (
            "CR LF tab LF lead",
            "\r\n\t\nBody.".to_owned(),
            "Body.".to_owned(),
        ),
        (
            "trailing space LF",
            "Body. \n".to_owned(),
            "Body.".to_owned(),
        ),
        (
            "trailing blank line",
            "Body.\n \n".to_owned(),
            "Body.".to_owned(),
        ),
        (
            "trailing CR LF and tab",
            "Body.\r\n\t\r\n".to_owned(),
            "Body.".to_owned(),
        ),
        ("BOM", "\u{FEFF}Body.\n".to_owned(), "Body.".to_owned()),
        (
            "BOM then blank lines",
            "\u{FEFF}\n\nBody.".to_owned(),
            "Body.".to_owned(),
        ),
        (
            "CR LF joined",
            "a\r\nb\r\n\r\nc\r\n".to_owned(),
            "a\nb\n\nc".to_owned(),
        ),
        // Nothing else changes.
        (
            "inner blanks",
            "Body  \t\nmore".to_owned(),
            "Body  \t\nmore".to_owned(),
        ),
        (
            "indented first content line",
            "\n  indented\n".to_owned(),
            "  indented".to_owned(),
        ),
        (
            "inner blank lines",
            "a\n\n \n\nb".to_owned(),
            "a\n\n \n\nb".to_owned(),
        ),
        ("only blanks", " \r\n\t\n \n".to_owned(), String::new()),
        ("empty", String::new(), String::new()),
    ];
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(case, body, expected)| {
            let found = import::document_text(body);
            (&found != expected).then(|| format!("{case}: {body:?} -> {found:?}, not {expected:?}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    for (_, body, expected) in &cases {
        assert_eq!(&import::document_text(expected), expected, "idempotent");
        assert_eq!(import::document_text(body), import::document_text(body));
    }
}

/// `docs/canon/import.md` "Text and hash": a document record's `text` is
/// `document_text` of the file after its header, its `hash` BLAKE3 of that,
/// for a YAML header, a field table and a path ID, with CRLF, NBSP and
/// U+3000 lines at the start and the end of the body.
#[test]
fn ac02_the_document_hash_is_blake3_of_document_text() {
    let nbsp = "\u{00A0}";
    let ideographic = "\u{3000}";
    let one = Corpus::copy("import-one", "document-text-hash");
    let yaml_header = "---\r\nNumber: REQ-062\r\n---\r\n";
    let yaml_body =
        format!("{nbsp}\r\n# Heading\r\n\r\nBody{ideographic}\r\n{ideographic}\r\n \r\n");
    one.write("spec/ws.md", &format!("{yaml_header}{yaml_body}"));
    let two = Corpus::copy("import-two", "document-text-hash");
    let table_header = "| Title | WS |\n|---|---|\n| Doc | DEC-0062 |\n";
    let table_body = format!(" \t\n{ideographic}\nBody.\t \n{nbsp}\n\n");
    two.write("pages/ws.mdown", &format!("{table_header}{table_body}"));
    let path_body = format!("\r\n\t\n{nbsp}x\n\nBody  \t\nend \n");
    two.write("log/DEC-0063.markdown", &path_body);
    let found_one = one.import();
    let found_two = two.import();
    for (found, path, id, line, body, text) in [
        (
            &found_one,
            "spec/ws.md",
            "REQ-062",
            2,
            yaml_body.as_str(),
            format!("{nbsp}\n# Heading\n\nBody{ideographic}\n{ideographic}"),
        ),
        (
            &found_two,
            "pages/ws.mdown",
            "DEC-0062",
            3,
            table_body.as_str(),
            format!("{ideographic}\nBody.\t \n{nbsp}"),
        ),
        (
            &found_two,
            "log/DEC-0063.markdown",
            "DEC-0063",
            1,
            path_body.as_str(),
            format!("{nbsp}x\n\nBody  \t\nend"),
        ),
    ] {
        let record = record(found, path, id);
        assert_eq!((record.form, record.line), (Form::Document, line), "{path}");
        assert_eq!(import::document_text(body), text, "{path}: document_text");
        assert_eq!(record.text, text, "{path}: text");
        assert_eq!(
            record.hash,
            blake3_hex(&import::document_text(body)),
            "{path}"
        );
    }
}

/// AC-02 rules ("the header value read is never a token", item 4): an
/// `id_key` value that resolves to no single ID or to a feature-scoped one
/// is one diagnostic at its line and none of its ID-like tokens is claimed
/// or unclaimed; without `[documents]` the same values are tokens. M:
/// header-value tokens counted.
#[test]
fn ac02_an_unresolving_or_feature_scoped_id_key_value_is_no_token() {
    // (fixture, path, document, the value's tokens, its line)
    let cases = [
        (
            "import-one",
            "spec/two-ids.md",
            "---\nNumber: REQ-012 and REQ-099\n---\n# Two IDs\n\nBody.\n",
            ["REQ-012", "REQ-099"].as_slice(),
            2,
        ),
        (
            "import-one",
            "spec/feature-doc.md",
            "---\nNumber: AC-009\n---\n# Feature\n\nBody.\n",
            ["AC-009"].as_slice(),
            2,
        ),
        (
            "import-two",
            "pages/two-ids.mdown",
            "| Title | Two IDs |\n|---|---|\n| Doc | DEC-0012, DEC-0098 |\n\nBody.\n",
            ["DEC-0012", "DEC-0098"].as_slice(),
            3,
        ),
        (
            "import-two",
            "pages/feature-doc.mdown",
            "| Title | Feature |\n|---|---|\n| Doc | CRT-0009 |\n\nBody.\n",
            ["CRT-0009"].as_slice(),
            3,
        ),
    ];
    let mut failures = Vec::new();
    for (name, path, document, value_tokens, line) in cases {
        let corpus = Corpus::copy(name, "value-no-token");
        let base = corpus.import();
        let without = without_documents(&corpus.config);
        let base_without = corpus.import_with(&without);
        corpus.write(path, document);
        let found = corpus.import();
        let found_without = corpus.import_with(&without);
        assert!(ids_in(&found, path).is_empty(), "{path}: no record");
        let diagnostics: Vec<Option<usize>> = diagnostics_in(&found, path)
            .into_iter()
            .map(|(line, _)| line)
            .collect();
        assert_eq!(diagnostics, [Some(line)], "{path}: one diagnostic");
        let at_line: Vec<&str> = found
            .unclaimed
            .iter()
            .filter(|token| token.path == path)
            .map(|token| token.token.as_str())
            .collect();
        if (found.claimed, found.unclaimed.len()) != (base.claimed, base.unclaimed.len())
            || !at_line.is_empty()
        {
            failures.push(format!(
                "{path}: claimed {} -> {}, unclaimed {} -> {} ({at_line:?})",
                base.claimed,
                found.claimed,
                base.unclaimed.len(),
                found.unclaimed.len()
            ));
        }
        // The control: without `[documents]` the value is read as text.
        assert_eq!(
            tokens(&found_without),
            tokens(&base_without) + value_tokens.len(),
            "{path}: tokens without [documents]"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-02 rules / AC-08: an `id_path` whose group `id` is optional and takes
/// no part in a match gives one diagnostic without a line and no record;
/// a header ID in that document still defines it, without a diagnostic.
#[test]
fn ac02_an_id_path_match_without_group_id_is_one_diagnostic() {
    let corpus = Corpus::copy("import-two", "id-path-no-group");
    let config = corpus.config.replace(
        "id_path = '^log/(?P<id>[A-Z]{3}-[0-9]{4})\\.markdown$'",
        "id_path = '^log/(?:(?P<id>[A-Z]{3}-[0-9]{4})|plain)\\.markdown$'",
    );
    assert_ne!(config, corpus.config, "the fixture's id_path replaced");
    let base = corpus.import_with(&config);
    let path = "log/plain.markdown";
    corpus.write(path, "# Plain\n\nNo ID here.\n");
    let found = corpus.import_with(&config);
    assert!(ids_in(&found, path).is_empty(), "no record");
    assert_eq!(
        diagnostics_in(&found, path),
        [(
            None,
            "the path matches `documents.id_path` without its group `id`".to_owned()
        )]
    );
    assert_eq!(found.records.len(), base.records.len());
    assert_eq!(found.diagnostics.len(), base.diagnostics.len() + 1);
    let path_record = record(&found, "log/DEC-0013.markdown", "DEC-0013");
    assert_eq!(path_record.form, Form::Document);

    corpus.write(
        path,
        "| Title | Plain |\n|---|---|\n| Doc | DEC-0064 |\n\nA header ID.\n",
    );
    let found = corpus.import_with(&config);
    assert_eq!(ids_in(&found, path), ["DEC-0064"]);
    assert_eq!(record(&found, path, "DEC-0064").form, Form::Document);
    assert_eq!(diagnostics_in(&found, path), []);
}

/// AC-08 on the engine's config: an `id_key` with blanks around it would
/// match no header key (keys are read trimmed): refused at its line.
#[test]
fn ac08_an_id_key_with_blanks_around_is_refused_at_its_line() {
    let base =
        "[corpus]\nroots = [\"spec\"]\n[ids]\nregex = '^[A-Z]{2,3}-[0-9]{3}$'\n[documents]\n";
    let origin = Path::new("/nonexistent/census.toml");
    for value in [
        "\" ident\"",
        "\"ident \"",
        "\"\\tident\"",
        "\"ident\\t\"",
        "\"\\u00A0ident\"",
    ] {
        let text = format!("{base}id_key = {value}\n");
        let error = CensusConfig::parse(&text, origin).expect_err(value);
        assert_eq!(error.line, Some(6), "{value}: {error}");
        assert!(error.to_string().contains("id_key"), "{value}: {error}");
    }
    let text = format!("{base}id_key = \"ident\"\n");
    assert!(CensusConfig::parse(&text, origin).is_ok());
}

// ----------------------------------------------------------------- AC-03

/// AC-03 in the fixtures: an ID defined by a document, a record-table row
/// and a list item: both record positions are references (`by_document`
/// 2), no duplicate, nothing unresolved. M: precedence off.
#[test]
fn ac03_a_document_demotes_the_row_and_the_list_item() {
    for (name, id, document) in [
        ("import-one", "REQ-012", "spec/req-012.md"),
        ("import-two", "DEC-0013", "log/DEC-0013.markdown"),
    ] {
        let found = import_fixture(name);
        let roles: Vec<(&str, Form, Role)> = found
            .records
            .iter()
            .filter(|record| record.id == id)
            .map(|record| (record.path.as_str(), record.form, record.role))
            .collect();
        let summary = if name == "import-one" {
            "spec/summary.md"
        } else {
            "pages/summary.mdown"
        };
        // Records come in (path, line) order; the document sorts first.
        let expected = [
            (document, Form::Document, Role::Definition),
            (summary, Form::TableRow, Role::Reference),
            (summary, Form::ListItem, Role::Reference),
        ];
        assert_eq!(roles, expected, "{name}");
        assert_eq!(found.by_document, 2, "{name}");
        assert!(
            found.duplicates.iter().all(|duplicate| duplicate.id != id),
            "{name}"
        );
        assert!(
            found.unresolved.iter().all(|token| token.token != id),
            "{name}"
        );
    }
}

/// AC-03: two documents defining one ID are one duplicate, the later in
/// (path, line) order; a document-level reference demotes nothing.
#[test]
fn ac03_two_defining_documents_are_one_duplicate_and_a_reference_demotes_nothing() {
    let one = Corpus::copy("import-one", "two-documents");
    one.write("spec/zz.md", "---\nNumber: REQ-012\n---\n# Again\n");
    let two = Corpus::copy("import-two", "two-documents");
    two.write(
        "pages/zz.mdown",
        "| Title | Again |\n|---|---|\n| Doc | DEC-0013 |\n",
    );
    for (name, corpus, id, later, first) in [
        (
            "import-one",
            &one,
            "REQ-012",
            "spec/zz.md",
            "spec/req-012.md",
        ),
        (
            "import-two",
            &two,
            "DEC-0013",
            "pages/zz.mdown",
            "log/DEC-0013.markdown",
        ),
    ] {
        let base = import_fixture(name);
        let found = corpus.import();
        assert_eq!(found.duplicates.len(), base.duplicates.len() + 1, "{name}");
        let new: Vec<(&str, &str, &str)> = found
            .duplicates
            .iter()
            .filter(|duplicate| duplicate.id == id)
            .map(|duplicate| {
                (
                    duplicate.id.as_str(),
                    duplicate.path.as_str(),
                    duplicate.first_path.as_str(),
                )
            })
            .collect();
        assert_eq!(new, [(id, later, first)], "{name}");
        assert_eq!(found.by_document, 2, "{name}");
    }

    // A reference document naming IDs other places define: none demoted.
    let one = Corpus::copy("import-one", "reference-demotes-nothing");
    let map = one.read("spec/map.md");
    one.write("spec/map.md", &format!("---\nNumber: REQ-007\n---\n{map}"));
    let two = Corpus::copy("import-two", "reference-demotes-nothing");
    two.write(
        "pages/refs/dec-0009.mdown",
        "| Title | Cited |\n|---|---|\n| Doc | DEC-0009 |\n",
    );
    for (name, corpus, id) in [
        ("import-one", &one, "REQ-007"),
        ("import-two", &two, "DEC-0009"),
    ] {
        let base = import_fixture(name);
        let found = corpus.import();
        assert_eq!(found.by_document, base.by_document, "{name}");
        assert_eq!(found.duplicates.len(), base.duplicates.len(), "{name}");
        assert_eq!(found.unresolved.len(), base.unresolved.len(), "{name}");
        let mut roles: Vec<String> = found
            .records
            .iter()
            .filter(|record| record.id == id)
            .map(|record| format!("{:?} {:?}", record.form, record.role))
            .collect();
        roles.sort();
        assert_eq!(
            roles,
            ["Document Reference", "HeaderlessRow Definition"],
            "{name}"
        );
    }
}

// ----------------------------------------------------------------- AC-04

/// AC-04: with `strip = ["p/"]`, a stripped form right after `/` is no
/// citation; full paths keep their bounds (`../p/x.md`). The fixtures'
/// `other/<stripped form>` lines are no citation either. M: `/` accepted
/// before a stripped form.
#[test]
fn ac04_a_stripped_form_after_a_slash_is_no_citation() {
    let corpus = Corpus::empty(
        "strip-slash",
        "[corpus]\nroots = [\"p\"]\n[ids]\nregex = '^[A-Z]{2}-[0-9]{3}$'\n\
         [code]\nroots = [\"src\"]\nstrip = [\"p/\"]\n",
    );
    corpus.write("p/x.md", "# X\n");
    corpus.write("p/sub/x.md", "# Sub X\n");
    corpus.write(
        "src/a.rs",
        "const A: &str = \"other/x.md\";\n\
         const B: &str = \"x.md\";\n\
         const C: &str = \"sub/x.md\";\n\
         const D: &str = \"../p/x.md\";\n",
    );
    let found = corpus.import();
    let citations: Vec<(&str, &str, usize)> = found
        .code
        .citations
        .iter()
        .map(|citation| {
            (
                citation.document.as_str(),
                citation.file.as_str(),
                citation.line,
            )
        })
        .collect();
    assert_eq!(
        citations,
        [
            ("p/x.md", "src/a.rs", 2),
            ("p/sub/x.md", "src/a.rs", 3),
            ("p/x.md", "src/a.rs", 4),
        ]
    );
    assert_eq!(found.code.documents_cited(), 2);

    for (name, file, line) in [
        ("import-one", "src/lib.rs", 6),
        ("import-two", "tools/check.py", 7),
    ] {
        let text = fs::read_to_string(fixture(name).join(file)).unwrap();
        assert!(
            text.lines().nth(line - 1).unwrap().contains("\"other/"),
            "{name}: {file}:{line} holds the other/ form"
        );
        let found = import_fixture(name);
        assert!(
            found
                .code
                .citations
                .iter()
                .all(|citation| !(citation.file == file && citation.line == line)),
            "{name}: {file}:{line} counted"
        );
        assert_eq!(found.code.citations.len(), 3, "{name}");
    }
}

// ----------------------------------------------------------------- AC-05

/// AC-05: a `key_map` identity → `kept`; a renaming → `mapped`; a key equal
/// to a `key_map` value, a `value_map` table or `id_key` → `kept`; any
/// other key, Latin or not → `unmapped`; a `value_map` identity → `kept`.
/// The fixtures' identity entries (`owner`, `Owner`) are kept. M: an
/// identity counted `mapped`.
#[test]
fn ac05_header_outcomes() {
    let config = "[corpus]\nroots = [\"spec\"]\n\
        [front_matter]\nheader_table = '^Field$'\n\
        [front_matter.key_map]\n\"owner\" = \"owner\"\n\"Phase\" = \"status\"\n\
        \"R\\u00e9sum\\u00e9\" = \"summary\"\n\
        [front_matter.value_map.status]\n\"Done\" = \"shipped\"\n\"draft\" = \"draft\"\n\
        [front_matter.value_map.state]\n\"Agreed\" = \"accepted\"\n\
        [ids]\nregex = '^[A-Z]{2,3}-[0-9]{3}$'\n\
        [documents]\nid_key = \"ident\"\n";
    let corpus = Corpus::empty("header-outcomes", config);
    let cyrillic = "\u{0421}\u{0442}\u{0430}\u{0442}\u{0443}\u{0441}";
    corpus.write(
        "spec/a.md",
        &format!(
            "---\nowner: Ana\nPhase: Done\nstatus: draft\nsummary: s\nstate: Agreed\n\
             ident: REQ-001\nother: x\n{cyrillic}: y\nR\u{00e9}sum\u{00e9}: z\n---\n# A\n"
        ),
    );
    corpus.write(
        "spec/b.md",
        "| Field | Value |\n|---|---|\n| owner | Bo |\n| status | shipped |\n\
         | Phase | Open |\n| Remark | r |\n\n# B\n",
    );
    let found = corpus.import();
    let detail = |path: &str| {
        found
            .documents_detail
            .iter()
            .find(|document| document.path == path)
            .unwrap_or_else(|| panic!("{path} read"))
    };
    let keys = |path: &str| -> Vec<(String, MapOutcome)> {
        detail(path)
            .keys
            .iter()
            .map(|key| (key.written.clone(), key.outcome))
            .collect()
    };
    let values = |path: &str| -> Vec<(String, MapOutcome)> {
        detail(path)
            .values
            .iter()
            .map(|value| (value.written.clone(), value.outcome))
            .collect()
    };
    use MapOutcome::{Kept, Mapped, Unmapped};
    let owned = |pairs: &[(&str, MapOutcome)]| -> Vec<(String, MapOutcome)> {
        pairs
            .iter()
            .map(|(key, outcome)| ((*key).to_owned(), *outcome))
            .collect()
    };
    let resume = "R\u{00e9}sum\u{00e9}";
    assert_eq!(
        keys("spec/a.md"),
        owned(&[
            ("owner", Kept),
            ("Phase", Mapped),
            ("status", Kept),
            ("summary", Kept),
            ("state", Kept),
            ("ident", Kept),
            ("other", Unmapped),
            (cyrillic, Unmapped),
            (resume, Mapped),
        ])
    );
    assert_eq!(
        values("spec/a.md"),
        owned(&[("Done", Mapped), ("draft", Kept), ("Agreed", Mapped)])
    );
    assert_eq!(
        keys("spec/b.md"),
        owned(&[
            ("owner", Kept),
            ("status", Kept),
            ("Phase", Mapped),
            ("Remark", Unmapped),
        ])
    );
    assert_eq!(
        values("spec/b.md"),
        owned(&[("shipped", Kept), ("Open", Unmapped)])
    );

    for (name, path, key) in [
        ("import-one", "spec/requirements.md", "owner"),
        ("import-one", "spec/req-012.md", "owner"),
        ("import-two", "pages/spec-x.mdown", "Owner"),
    ] {
        let found = import_fixture(name);
        let outcomes: Vec<MapOutcome> = found
            .documents_detail
            .iter()
            .filter(|document| document.path == path)
            .flat_map(|document| document.keys.iter())
            .filter(|entry| entry.written == key)
            .map(|entry| entry.outcome)
            .collect();
        assert_eq!(outcomes, [Kept], "{name}: {path} {key}");
    }
}

// ----------------------------------------------------------------- AC-06

/// The AC-06 document in import-one's convention: each record item opens a
/// comment its next line closes, followed by what would otherwise open a
/// block or a record.
const COMMENT_LINES: &str = "# Comments

- **REQ-401:** a <!-- c
d --> - **REQ-402:** b
- **REQ-403:** c <!-- c
d --> # y
- **REQ-404:** e <!-- c
d --> - y
- **REQ-405:** f <!-- c
  d --> - **REQ-406:** g
- **REQ-407:** h <!-- c
d --> - **QP407:** i
- **REQ-408:** j <!-- c
d --> ```
- **REQ-409:** k <!-- c
d --> ## T {#REQ-410}
- **REQ-411:** last
";

/// AC-06: a line opening inside the comment its item's text opened is that
/// item's paragraph text whatever follows `-->`: no record, heading, list
/// item, fence or section is read on it, and the item goes on; a hyphenless
/// match there is a mention; the hidden record's token counts once.
/// M: records read on lines opening inside a comment; the heading test
/// applied to them.
#[test]
fn ac06_a_line_closing_the_items_comment_is_its_text() {
    let corpus = Corpus::copy("import-one", "comment-lines");
    let path = "spec/comments.md";
    corpus.write(path, COMMENT_LINES);
    let found = corpus.import();
    let texts: Vec<(String, Form, String)> = found
        .records
        .iter()
        .filter(|record| record.path == path)
        .map(|record| (record.id.clone(), record.form, record.text.clone()))
        .collect();
    let expected: Vec<(String, Form, String)> = [
        ("REQ-401", "a <!-- c\nd --> - **REQ-402:** b"),
        ("REQ-403", "c <!-- c\nd --> # y"),
        ("REQ-404", "e <!-- c\nd --> - y"),
        ("REQ-405", "f <!-- c\n  d --> - **REQ-406:** g"),
        ("REQ-407", "h <!-- c\nd --> - **QP407:** i"),
        ("REQ-408", "j <!-- c\nd --> ```"),
        ("REQ-409", "k <!-- c\nd --> ## T {#REQ-410}"),
        ("REQ-411", "last"),
    ]
    .iter()
    .map(|(id, text)| ((*id).to_owned(), Form::ListItem, (*text).to_owned()))
    .collect();
    assert_eq!(texts, expected);
    for hidden in ["REQ-402", "REQ-406"] {
        let count = found
            .unclaimed
            .iter()
            .filter(|token| token.path == path && token.token == hidden)
            .count();
        assert_eq!(count, 1, "{hidden} counted once");
    }
    let hyphenless: Vec<(usize, import::HyphenlessRole)> = found
        .legacy
        .hyphenless
        .iter()
        .filter(|found| found.path == path)
        .map(|found| (found.line, found.role))
        .collect();
    assert_eq!(hyphenless, [(12, import::HyphenlessRole::Mention)]);
}

/// (ID, form, text) of every record of one path, in order.
fn forms_in(import: &Import, path: &str) -> Vec<(String, Form, String)> {
    import
        .records
        .iter()
        .filter(|record| record.path == path)
        .map(|record| (record.id.clone(), record.form, record.text.clone()))
        .collect()
}

/// How often `token` is counted unclaimed in `path`.
fn unclaimed_count(import: &Import, path: &str, token: &str) -> usize {
    import
        .unclaimed
        .iter()
        .filter(|found| found.path == path && found.token == token)
        .count()
}

/// A record-table row in a fixture's convention: import-one's ID is the
/// first column, import-two's the second after a number.
fn table_row(fixture: &str, id: &str, text: &str) -> String {
    match fixture {
        "import-one" => format!("| {id} | {text} |"),
        _ => format!("| 1 | {id} | {text} |"),
    }
}

/// A record table's header and delimiter rows in a fixture's convention.
fn table_head(fixture: &str) -> &'static str {
    match fixture {
        "import-one" => "| ID | Statement |\n|---|---|\n",
        _ => "| # | Code | Decision |\n|---|---|---|\n",
    }
}

/// AC-06 ("no list recognizer ... from a line opening inside any comment",
/// tables alike): a pipe row after the `-->` closing a record item's
/// comment is that item's text, not a headerless row; the same row after a
/// blank line is one (the control), in both conventions. M: a table read
/// on a comment line.
#[test]
fn ac06_a_pipe_row_closing_the_items_comment_is_no_row() {
    let mut failures = Vec::new();
    for lists in &LISTS {
        let corpus = Corpus::copy(lists.fixture, "comment-row");
        let (item, hidden, control) = ((lists.id)(421), (lists.id)(422), (lists.id)(423));
        let s = lists.strong;
        let separator = lists.separators[lists.separators.len() - 1];
        corpus.write(
            lists.document,
            &format!(
                "# Comment rows\n\n{}{s}{item}{separator}{s} a <!-- c\nd --> {}\n\n{}\n",
                lists.marker,
                table_row(lists.fixture, &hidden, "text"),
                table_row(lists.fixture, &control, "control"),
            ),
        );
        let found = corpus.import();
        let expected = vec![
            (
                item.clone(),
                Form::ListItem,
                format!(
                    "a <!-- c\nd --> {}",
                    table_row(lists.fixture, &hidden, "text")
                ),
            ),
            (control.clone(), Form::HeaderlessRow, "control".to_owned()),
        ];
        let records = forms_in(&found, lists.document);
        if records != expected {
            failures.push(format!("{}: {records:?}", lists.fixture));
        }
        assert_eq!(
            unclaimed_count(&found, lists.document, &hidden),
            1,
            "{}: {hidden} counted once",
            lists.fixture
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-06, tables: a line opening inside a comment a table row opened
/// continues no table: the pipe row after `-->` is no row record (its token
/// counts once), the row before it stays a row. M: a table read on a
/// comment line.
#[test]
fn ac06_a_pipe_row_closing_a_rows_comment_is_no_row() {
    let mut failures = Vec::new();
    for lists in &LISTS {
        let corpus = Corpus::copy(lists.fixture, "comment-row-continued");
        let (row, hidden, after) = ((lists.id)(431), (lists.id)(432), (lists.id)(433));
        corpus.write(
            lists.document,
            &format!(
                "# Comment rows\n\n{}{}\nd --> {}\n{}\n",
                table_head(lists.fixture),
                table_row(lists.fixture, &row, "a <!-- c"),
                table_row(lists.fixture, &hidden, "b"),
                table_row(lists.fixture, &after, "c"),
            ),
        );
        let found = corpus.import();
        let ids = ids_in(&found, lists.document);
        if ids.contains(&hidden) {
            failures.push(format!(
                "{}: {:?}",
                lists.fixture,
                forms_in(&found, lists.document)
            ));
        }
        assert_eq!(
            record(&found, lists.document, &row).form,
            Form::TableRow,
            "{}",
            lists.fixture
        );
        assert_eq!(
            unclaimed_count(&found, lists.document, &hidden),
            1,
            "{}: {hidden} counted once",
            lists.fixture
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The headings document: a `{#ID}` heading after `-->` closing a plain
/// list item's comment and a column-0 comment, a strong lead right after
/// the second, then real headings after a closed comment.
const COMMENT_HEADINGS: &str = "# Comment headings

- plain item <!-- c
d --> ## T {#REQ-441}

<!-- c
d --> ## U {#REQ-442}
**QP442** follows the comment.

<!-- closed -->
## V {#REQ-443}

## W {#REQ-444}

Body.
";

/// AC-06, `docs/canon/import.md` "Comments" (import-wide): a line opening
/// inside any comment gives no heading: `d --> ## T {#ID}` after a plain
/// item's comment or a column-0 comment is no section (its token counts
/// once), and the line after it opens no paragraph (a hyphenless match
/// there is a mention); later real `{#ID}` headings are sections. M: a
/// heading read on a comment line (the scanner; the heading test of the
/// token pass).
#[test]
fn ac06_a_heading_closing_any_comment_is_no_section() {
    let corpus = Corpus::copy("import-one", "comment-headings");
    let path = "spec/comment-headings.md";
    corpus.write(path, COMMENT_HEADINGS);
    let found = corpus.import();
    let sections: Vec<(String, Form)> = forms_in(&found, path)
        .into_iter()
        .map(|(id, form, _)| (id, form))
        .collect();
    assert_eq!(
        sections,
        [
            ("REQ-443".to_owned(), Form::Section),
            ("REQ-444".to_owned(), Form::Section),
        ]
    );
    assert_eq!(
        record(&found, path, "REQ-444").text,
        "## W {#REQ-444}\n\nBody."
    );
    for hidden in ["REQ-441", "REQ-442"] {
        assert_eq!(unclaimed_count(&found, path, hidden), 1, "{hidden}");
    }
    let hyphenless: Vec<(usize, import::HyphenlessRole)> = found
        .legacy
        .hyphenless
        .iter()
        .filter(|found| found.path == path)
        .map(|found| (found.line, found.role))
        .collect();
    assert_eq!(hyphenless, [(8, import::HyphenlessRole::Mention)]);
}

/// (ID, line, form, text) of every record of one path, in order.
fn rows_in(import: &Import, path: &str) -> Vec<(String, usize, Form, String)> {
    import
        .records
        .iter()
        .filter(|record| record.path == path)
        .map(|record| {
            (
                record.id.clone(),
                record.line,
                record.form,
                record.text.clone(),
            )
        })
        .collect()
}

/// A record table in a fixture's convention whose first row (line 5) opens
/// a comment and has no closing pipe; `inside` are the lines opening inside
/// that comment, the last one closing it; two more rows follow.
fn comment_table(fixture: &str, ids: &[String; 3], inside: &[&str]) -> String {
    let open = table_row(fixture, &ids[0], "one <!-- c");
    let open = open.strip_suffix(" |").expect("a row ends with a pipe");
    format!(
        "# Comment table rows\n\n{}{open}\n{}\n{}\n{}\n",
        table_head(fixture),
        inside.join("\n"),
        table_row(fixture, &ids[1], "two"),
        table_row(fixture, &ids[2], "three"),
    )
}

/// The three rows of [`comment_table`] as `table-row` records, the later
/// two after `inside` lines; the first one's text is its cell as written,
/// the comment opener included.
fn comment_table_rows(ids: &[String; 3], inside: usize) -> Vec<(String, usize, Form, String)> {
    vec![
        (ids[0].clone(), 5, Form::TableRow, "one <!-- c".to_owned()),
        (ids[1].clone(), 6 + inside, Form::TableRow, "two".to_owned()),
        (
            ids[2].clone(),
            7 + inside,
            Form::TableRow,
            "three".to_owned(),
        ),
    ]
}

/// Trials a and b of iteration 4, both conventions: a row's comment closing
/// on the next line (`d --> |`) leaves every later row a `table-row`
/// record of the same table, whatever `headerless` says, and no token of
/// the document unclaimed.
fn rows_after_a_rows_comment(headerless: bool) -> Vec<String> {
    let mut failures = Vec::new();
    for lists in &LISTS {
        let corpus = Corpus::copy(lists.fixture, "comment-table");
        let ids = [(lists.id)(451), (lists.id)(452), (lists.id)(453)];
        corpus.write(
            lists.document,
            &comment_table(lists.fixture, &ids, &["d --> |"]),
        );
        let config = corpus
            .config
            .replace("headerless = true", &format!("headerless = {headerless}"));
        assert_eq!(
            config
                .matches(&format!("headerless = {headerless}"))
                .count(),
            1,
            "{}: headerless set",
            lists.fixture
        );
        let found = corpus.import_with(&config);
        let rows = rows_in(&found, lists.document);
        if rows != comment_table_rows(&ids, 1) {
            failures.push(format!("{}: {rows:?}", lists.fixture));
        }
        let unclaimed: Vec<&str> = found
            .unclaimed
            .iter()
            .filter(|token| token.path == lists.document)
            .map(|token| token.token.as_str())
            .collect();
        if !unclaimed.is_empty() {
            failures.push(format!("{}: unclaimed {unclaimed:?}", lists.fixture));
        }
    }
    failures
}

/// AC-06, tables, trial a (`headerless = true`): a line opening inside a
/// comment a row opened is neither a row nor the table's end: the rows
/// after it stay `table-row` records, not headerless rows. M: the in-table
/// skip turned back into `break`.
#[test]
fn ac06_rows_after_a_rows_comment_stay_table_rows_when_headerless() {
    let failures = rows_after_a_rows_comment(true);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-06, tables, trial b (`headerless = false`): the same rows stay
/// `table-row` records and nothing is unclaimed (a table ended at the
/// comment line would leave them unclaimed). M: the in-table skip turned
/// back into `break`.
#[test]
fn ac06_rows_after_a_rows_comment_stay_table_rows_without_headerless() {
    let failures = rows_after_a_rows_comment(false);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-06, tables, trial c: a line wholly inside a row's comment is no blank
/// ending the table, and `d --> # y [l](...) |` closing it is no heading:
/// all three rows are `table-row` records, nothing else is a record; the
/// link after `-->` is collected (reported broken at its line), the one
/// inside the comment is not. M: the in-table skip moved after the blank
/// check.
#[test]
fn ac06_a_line_wholly_inside_a_rows_comment_and_a_heading_after_it_keep_the_table() {
    let mut failures = Vec::new();
    for lists in &LISTS {
        let corpus = Corpus::copy(lists.fixture, "comment-table-inside");
        let ids = [(lists.id)(461), (lists.id)(462), (lists.id)(463)];
        corpus.write(
            lists.document,
            &comment_table(
                lists.fixture,
                &ids,
                &[
                    "still [x](gone-inside.md) comment",
                    "d --> # y [l](gone-after.md) |",
                ],
            ),
        );
        let found = corpus.import();
        let rows = rows_in(&found, lists.document);
        if rows != comment_table_rows(&ids, 2) {
            failures.push(format!("{}: {rows:?}", lists.fixture));
        }
        let links: Vec<(usize, &str, import::LinkProblem)> = found
            .links
            .iter()
            .filter(|link| link.path == lists.document)
            .map(|link| (link.line, link.target.as_str(), link.problem))
            .collect();
        if links != [(7, "gone-after.md", import::LinkProblem::File)] {
            failures.push(format!("{}: links {links:?}", lists.fixture));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-06, trial e: outside a table, a line opening inside a paragraph's
/// comment starts no table: the pipe row after `-->` is no record (its
/// token counts once), the rows after it are a headerless table of their
/// own. M: a table read on a comment line; with the headerless row loop
/// rejecting its first line, the spin guard removed (the scan must end).
#[test]
fn ac06_a_comment_line_outside_a_table_starts_no_table() {
    let mut failures = Vec::new();
    for lists in &LISTS {
        let corpus = Corpus::copy(lists.fixture, "comment-no-table");
        let ids = [(lists.id)(471), (lists.id)(472), (lists.id)(473)];
        corpus.write(
            lists.document,
            &format!(
                "# Comment outside a table\n\nText <!-- c\nd --> {}\n{}\n{}\n",
                table_row(lists.fixture, &ids[0], "one"),
                table_row(lists.fixture, &ids[1], "two"),
                table_row(lists.fixture, &ids[2], "three"),
            ),
        );
        let found = corpus.import();
        let rows = rows_in(&found, lists.document);
        let expected = vec![
            (ids[1].clone(), 5, Form::HeaderlessRow, "two".to_owned()),
            (ids[2].clone(), 6, Form::HeaderlessRow, "three".to_owned()),
        ];
        if rows != expected {
            failures.push(format!("{}: {rows:?}", lists.fixture));
        }
        assert_eq!(
            unclaimed_count(&found, lists.document, &ids[0]),
            1,
            "{}: {} counted once",
            lists.fixture,
            ids[0]
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ----------------------------------------------------------------- AC-07

/// AC-07: the census of a generated corpus holding titled lead-ins, a
/// document ID, comment-closing lines with a `{#ID}` heading and a fence
/// after `-->` is pinned to what it was before import-gaps: the heading
/// after `-->` is a section, the fence after `-->` hides the table up to
/// its closing fence. M: the shared scanner stops reading a heading after
/// `-->`.
#[test]
fn ac07_the_census_of_a_generated_corpus_is_pinned() {
    let corpus = Corpus::empty("census-pin", &fixture_config("import-one"));
    corpus.write(
        "spec/gen-a.md",
        "---\nNumber: REQ-501\n---\n# Generated\n\n\
         - **REQ-502: Titled**: text <!-- c\n\
         d --> ## T {#REQ-503}\n\n\
         ## Next {#REQ-504}\n\nBody.\n",
    );
    corpus.write(
        "spec/gen-b.md",
        "# Fenced\n\n\
         - **REQ-511:** item <!-- c\n\
         d --> ```\n\
         | ID | Statement |\n|---|---|\n| REQ-512 | Hidden from the census by the fence. |\n\
         ```\n\n\
         | ID | Statement |\n|---|---|\n| REQ-513 | Seen by both. |\n",
    );
    let census = corpus.census();
    let records: Vec<(&str, usize, RecordKind, &str)> = census
        .records
        .iter()
        .map(|record| {
            (
                record.path.as_str(),
                record.line,
                record.kind,
                record.id.as_str(),
            )
        })
        .collect();
    assert_eq!(
        records,
        [
            ("spec/gen-a.md", 7, RecordKind::Section, "REQ-503"),
            ("spec/gen-a.md", 9, RecordKind::Section, "REQ-504"),
            ("spec/gen-b.md", 12, RecordKind::Row, "REQ-513"),
        ]
    );
    let hash = |id: &str| {
        census
            .records
            .iter()
            .find(|record| record.id == id)
            .map(|record| record.blake3.clone())
            .unwrap()
    };
    assert_eq!(hash("REQ-504"), blake3_hex("## Next {#REQ-504}\n\nBody."));
    assert_eq!(hash("REQ-513"), blake3_hex("| REQ-513 | Seen by both. |"));
    assert_eq!(census.documents, 2);
    assert_eq!(census.with_front_matter(), 1);
    assert!(census.rows_without_id.is_empty());
    assert_eq!(census.headerless_id_rows, 0);
    assert!(census.broken_links.is_empty());
    assert!(census.diagnostics.is_empty(), "{:?}", census.diagnostics);
}

/// AC-07, census unchanged (`docs/canon/import.md` "Comments",
/// "Text and hash") for the import-only scanner rules of iterations 3 and
/// 4: the census still reads a heading after `-->` closing a plain item's
/// or a column-0 comment, a table row after `-->` closing a row's
/// comment (an ID-less one too), a line wholly inside a row's comment as a
/// blank ending the table (the rows after it headerless, the link after the
/// closing `-->` collected), a pipe row after a paragraph's `-->` as a
/// headerless table,
/// and gets nothing from a BOM-led field table (it reads no field tables),
/// while the import reads none of the comment lines, keeps the tables going
/// across them and skips the BOM.
#[test]
fn ac07_the_census_keeps_comment_lines_and_a_bom_as_before() {
    let corpus = Corpus::empty("census-pin-comments", &fixture_config("import-one"));
    corpus.write(
        "spec/gen-c.md",
        "# Census comment lines\n\n\
         - plain item <!-- c\n\
         d --> ## T {#REQ-541}\n\n\
         <!-- c\n\
         d --> ## U {#REQ-542}\n\n\
         | ID | Statement |\n|---|---|\n\
         | REQ-543 | a <!-- c |\n\
         d --> | REQ-544 | b |\n",
    );
    corpus.write(
        "spec/gen-d.md",
        "\u{FEFF}| Field | Value |\n|---|---|\n| Number | REQ-545 |\n\nBody.\n",
    );
    corpus.write(
        "spec/gen-e.md",
        "# E\n\n| ID | Statement |\n|---|---|\n\
         | REQ-546 | one <!-- c\n\
         d --> |\n\
         | REQ-547 | two |\n",
    );
    corpus.write(
        "spec/gen-f.md",
        "# F\n\n| ID | Statement |\n|---|---|\n\
         | REQ-548 | one <!-- c\n\
         still [x](gone-inside.md) comment\n\
         d --> # y [l](gone-after.md) |\n\
         | REQ-549 | two |\n\
         | REQ-550 | three |\n",
    );
    corpus.write(
        "spec/gen-g.md",
        "# G\n\nText <!-- c\n\
         d --> | REQ-551 | one |\n\
         | REQ-552 | two |\n",
    );
    let census = corpus.census();
    let records: Vec<(&str, usize, RecordKind, &str)> = census
        .records
        .iter()
        .map(|record| {
            (
                record.path.as_str(),
                record.line,
                record.kind,
                record.id.as_str(),
            )
        })
        .collect();
    assert_eq!(
        records,
        [
            ("spec/gen-c.md", 4, RecordKind::Section, "REQ-541"),
            ("spec/gen-c.md", 7, RecordKind::Section, "REQ-542"),
            ("spec/gen-c.md", 11, RecordKind::Row, "REQ-543"),
            ("spec/gen-c.md", 12, RecordKind::Row, "REQ-544"),
            ("spec/gen-e.md", 5, RecordKind::Row, "REQ-546"),
            ("spec/gen-e.md", 7, RecordKind::Row, "REQ-547"),
            ("spec/gen-f.md", 5, RecordKind::Row, "REQ-548"),
            ("spec/gen-f.md", 8, RecordKind::Row, "REQ-549"),
            ("spec/gen-f.md", 9, RecordKind::Row, "REQ-550"),
            ("spec/gen-g.md", 4, RecordKind::Row, "REQ-551"),
            ("spec/gen-g.md", 5, RecordKind::Row, "REQ-552"),
        ]
    );
    assert_eq!(census.documents, 5);
    assert_eq!(census.with_front_matter(), 0);
    let without_id: Vec<(&str, usize)> = census
        .rows_without_id
        .iter()
        .map(|location| (location.path.as_str(), location.line))
        .collect();
    assert_eq!(without_id, [("spec/gen-e.md", 6)]);
    assert_eq!(census.headerless_id_rows, 4);
    let broken: Vec<(&str, usize, &str)> = census
        .broken_links
        .iter()
        .map(|link| (link.path.as_str(), link.line, link.target.as_str()))
        .collect();
    assert_eq!(broken, [("spec/gen-f.md", 7, "gone-after.md")]);
    assert!(census.diagnostics.is_empty(), "{:?}", census.diagnostics);

    let found = corpus.import();
    let imported: Vec<(&str, usize, Form, &str)> = found
        .records
        .iter()
        .map(|record| {
            (
                record.path.as_str(),
                record.line,
                record.form,
                record.id.as_str(),
            )
        })
        .collect();
    assert_eq!(
        imported,
        [
            ("spec/gen-c.md", 11, Form::TableRow, "REQ-543"),
            ("spec/gen-d.md", 3, Form::Document, "REQ-545"),
            ("spec/gen-e.md", 5, Form::TableRow, "REQ-546"),
            ("spec/gen-e.md", 7, Form::TableRow, "REQ-547"),
            ("spec/gen-f.md", 5, Form::TableRow, "REQ-548"),
            ("spec/gen-f.md", 8, Form::TableRow, "REQ-549"),
            ("spec/gen-f.md", 9, Form::TableRow, "REQ-550"),
            ("spec/gen-g.md", 5, Form::HeaderlessRow, "REQ-552"),
        ]
    );
    let links: Vec<(&str, usize, &str)> = found
        .links
        .iter()
        .map(|link| (link.path.as_str(), link.line, link.target.as_str()))
        .collect();
    assert_eq!(links, [("spec/gen-f.md", 7, "gone-after.md")]);
}
