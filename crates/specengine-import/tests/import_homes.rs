//! docs/features/import-homes.md on the engine
//! (`specengine_import::import::run`, `CensusConfig::parse`): AC-01 a home
//! per prefix in `[definitions]` (the register defines, a restatement
//! outside it stays a reference, `by_home`), AC-02 a bad home or
//! `[layout] inherit` refused at its line, AC-04 nothing moves without the
//! keys. The after-tree, the inherited header values (AC-03) and the CLI
//! refusals are end to end in eval `tests/import_homes_cli.rs`; the
//! on-disk tamper of AC-03 is the `#[cfg(test)]` module of
//! `crates/specengine-eval/src/layout.rs`.
//!
//! The corpus is the invented bench-log convention
//! `fixtures/import-layout/three`, read in place or copied into a fresh
//! temporary directory and edited there. Non-Latin characters are Unicode
//! escapes only (ADR-0024,
//! `docs/canon/architecture.md` "Repository language").

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_import::CensusConfig;
use specengine_import::import::{self, Form, Import, ImportRecord, Role};

const HOMES: &str = "import-layout/three";

/// The home line of the fixture config, as written.
const HOME_LINE: &str = "home = { XQ = [\"bench/probes.md\"] }\n";

/// The inherit line of the fixture config, as written.
const INHERIT_LINE: &str = "inherit = [\"rev\", \"scope\", \"owner\", \"reviewed\"]\n";

const NOTES: &str = "bench/notes.md";
const PROBES: &str = "bench/probes.md";
const GAUGES: &str = "bench/gauges.md";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn fixture_config(name: &str) -> String {
    fs::read_to_string(fixture(name).join("census.toml")).expect("fixture config")
}

fn parse(config: &str, root: &Path) -> CensusConfig {
    CensusConfig::parse(config, &root.join("census.toml"))
        .unwrap_or_else(|error| panic!("config must parse: {error}\n{config}"))
}

fn import_at(root: &Path, config: &str) -> Import {
    import::run(root, &parse(config, root)).expect("import runs")
}

/// `text` with `from` replaced once; `from` must be there.
fn replaced(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?} not in:\n{text}");
    text.replacen(from, to, 1)
}

/// The fixture config with its home line replaced by `line` (`""`: none).
fn with_home(line: &str) -> String {
    replaced(&fixture_config(HOMES), HOME_LINE, line)
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

/// A copy of the fixture in a fresh temporary directory, removed on drop;
/// unique per process and per call.
struct Copy {
    root: PathBuf,
}

impl Copy {
    fn new(case: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "specengine-import-homes-{case}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        copy_dir(&fixture(HOMES), &root);
        Self { root }
    }

    fn write(&self, path: &str, text: &str) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).expect("corpus file writable");
    }

    fn edit(&self, path: &str, from: &str, to: &str) {
        let text = fs::read_to_string(self.root.join(path)).expect("corpus file readable");
        self.write(path, &replaced(&text, from, to));
    }

    fn import_with(&self, config: &str) -> Import {
        import_at(&self.root, config)
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The one record of `id` in `path`.
fn record<'a>(found: &'a Import, path: &str, id: &str) -> &'a ImportRecord {
    let hits: Vec<&ImportRecord> = found
        .records
        .iter()
        .filter(|record| record.path == path && record.id == id)
        .collect();
    assert_eq!(hits.len(), 1, "records of {id} in {path}: {hits:?}");
    hits[0]
}

/// (path, line, role) of every record of `id`, in record order.
fn roles_of(found: &Import, id: &str) -> Vec<(String, usize, Role)> {
    found
        .records
        .iter()
        .filter(|record| record.id == id)
        .map(|record| (record.path.clone(), record.line, record.role))
        .collect()
}

fn definitions(found: &Import) -> usize {
    found
        .records
        .iter()
        .filter(|record| record.role == Role::Definition)
        .count()
}

/// The IDs of the duplicate definitions.
fn duplicate_ids(found: &Import) -> Vec<String> {
    found
        .duplicates
        .iter()
        .map(|duplicate| duplicate.id.clone())
        .collect()
}

/// The 1-based line of the first line of `text` holding `needle`.
fn line_holding(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not in:\n{text}"))
        + 1
}

// ----------------------------------------------------------------- AC-01

/// AC-01: the probes register is XQ's home: its row defines XQ-11, the
/// notes' earlier-sorting lead-in of XQ-11 is a reference
/// (`by_home` 1), XQ-90, which only the notes define, stays a definition,
/// and no duplicate is left. (M1 home ignored: the notes' XQ-11 defines and
/// the register's row is a duplicate; M2 a home demoting an ID it does not
/// define: XQ-90 a reference; M4 `by_home` 0.)
#[test]
fn ac01_the_home_register_defines_and_the_earlier_restatement_is_a_reference() {
    let found = import_at(&fixture(HOMES), &fixture_config(HOMES));
    assert!(NOTES < PROBES, "the restatement sorts before the home");
    assert_eq!(
        roles_of(&found, "XQ-11"),
        [
            (NOTES.to_owned(), 13, Role::Reference),
            (PROBES.to_owned(), 15, Role::Definition),
        ]
    );
    let home = record(&found, PROBES, "XQ-11");
    assert_eq!(home.form, Form::TableRow);
    assert_eq!(
        home.text,
        "The probe touches the plate before every reading."
    );
    let restatement = record(&found, NOTES, "XQ-11");
    assert_eq!(restatement.form, Form::ListItem);
    assert_eq!(restatement.text, "The probe touches the plate first.");
    assert_eq!(
        roles_of(&found, "XQ-90"),
        [(NOTES.to_owned(), 14, Role::Definition)]
    );
    assert_eq!(found.by_home, 1);
    assert_eq!(found.by_document, 0);
    assert_eq!(duplicate_ids(&found), Vec::<String>::new());
    assert_eq!(definitions(&found), 6, "XQ-11, XQ-12, XQ-90, YR-01..03");
    assert!(
        found.unresolved.is_empty(),
        "the demoted restatement resolves to the home: {:?}",
        found.unresolved
    );
}

/// AC-01 / AC-04, not vacuous: without the home the same corpus is the
/// gap the home closes: the earlier restatement defines XQ-11 and the
/// register's row is the duplicate; `by_home` 0.
#[test]
fn ac01_without_the_home_the_earlier_restatement_defines_and_the_register_duplicates() {
    let found = import_at(&fixture(HOMES), &with_home(""));
    assert_eq!(
        roles_of(&found, "XQ-11"),
        [
            (NOTES.to_owned(), 13, Role::Definition),
            (PROBES.to_owned(), 15, Role::Definition),
        ]
    );
    assert_eq!(duplicate_ids(&found), ["XQ-11"]);
    assert_eq!(found.by_home, 0);
    assert_eq!(definitions(&found), 7);
}

/// AC-01 (M2): a home naming a document that defines none of a prefix's
/// IDs demotes nothing: YR's home is the probes register, every YR row
/// stands in the gauges register and stays a definition.
#[test]
fn ac01_a_home_that_defines_none_of_the_ids_demotes_nothing() {
    let config = with_home("home = { XQ = [\"bench/probes.md\"], YR = [\"bench/probes.md\"] }\n");
    let found = import_at(&fixture(HOMES), &config);
    for id in ["YR-01", "YR-02", "YR-03"] {
        let roles = roles_of(&found, id);
        assert_eq!(roles.len(), 1, "{id}: {roles:?}");
        assert_eq!(roles[0].0, GAUGES, "{id}");
        assert_eq!(roles[0].2, Role::Definition, "{id}");
    }
    assert_eq!(found.by_home, 1, "only the XQ-11 restatement");
}

/// AC-01: home globs match the corpus-relative, `/`-separated path like
/// every config glob (`*`, `**`, `?`); a glob matching no document is no
/// home, so nothing is demoted.
#[test]
fn ac01_home_globs_match_corpus_relative_paths() {
    for glob in ["bench/p*.md", "**/probes.md", "bench/prob?s.md"] {
        let found = import_at(
            &fixture(HOMES),
            &with_home(&format!("home = {{ XQ = [\"{glob}\"] }}\n")),
        );
        assert_eq!(found.by_home, 1, "{glob}");
        assert_eq!(
            record(&found, NOTES, "XQ-11").role,
            Role::Reference,
            "{glob}"
        );
    }
    for glob in ["probes.md", "bench/probes", "BENCH/probes.md"] {
        let found = import_at(
            &fixture(HOMES),
            &with_home(&format!("home = {{ XQ = [\"{glob}\"] }}\n")),
        );
        assert_eq!(found.by_home, 0, "{glob} matches no document");
        assert_eq!(
            record(&found, NOTES, "XQ-11").role,
            Role::Definition,
            "{glob}"
        );
        assert_eq!(duplicate_ids(&found), ["XQ-11"], "{glob}");
    }
}

/// AC-01: a home holding both documents leaves the plain duplicate rule:
/// both are inside, nothing is demoted, the later definition duplicates.
#[test]
fn ac01_a_restatement_inside_the_home_is_a_plain_duplicate() {
    let config = with_home("home = { XQ = [\"bench/notes.md\", \"bench/probes.md\"] }\n");
    let found = import_at(&fixture(HOMES), &config);
    assert_eq!(found.by_home, 0);
    assert_eq!(record(&found, NOTES, "XQ-11").role, Role::Definition);
    assert_eq!(duplicate_ids(&found), ["XQ-11"]);
}

/// AC-01: the home defines whatever the order: a restatement sorting after
/// the register is a reference too, not a later duplicate (`by_home` 2).
#[test]
fn ac01_a_restatement_sorting_after_the_home_is_a_reference_too() {
    let corpus = Copy::new("later");
    corpus.write(
        "bench/zeta.md",
        "---\nclass: canon\n---\n\n# Zeta\n\n- **XQ-12**: The probe is wiped.\n",
    );
    let found = corpus.import_with(&fixture_config(HOMES));
    assert!(PROBES < "bench/zeta.md");
    assert_eq!(
        record(&found, "bench/zeta.md", "XQ-12").role,
        Role::Reference
    );
    assert_eq!(record(&found, PROBES, "XQ-12").role, Role::Definition);
    assert_eq!(found.by_home, 2);
    assert_eq!(duplicate_ids(&found), Vec::<String>::new());
}

/// AC-01: every record position outside the home is demoted: a `{#ID}`
/// section and a table row restating XQ-12 in the notes.
#[test]
fn ac01_a_section_or_row_restatement_outside_the_home_is_a_reference() {
    let corpus = Copy::new("forms");
    corpus.edit(
        NOTES,
        "The notes close with no further record.\n",
        "## Wipe {#XQ-12}\n\nThe probe is wiped.\n\n| Mark | Phrase |\n|------|--------|\n\
         | XQ-12 | The probe is wiped once. |\n",
    );
    let found = corpus.import_with(&fixture_config(HOMES));
    let restated: Vec<(Form, Role)> = found
        .records
        .iter()
        .filter(|record| record.path == NOTES && record.id == "XQ-12")
        .map(|record| (record.form, record.role))
        .collect();
    assert_eq!(
        restated,
        [
            (Form::Section, Role::Reference),
            (Form::TableRow, Role::Reference)
        ]
    );
    assert_eq!(found.by_home, 3, "XQ-11's item, XQ-12's section and row");
    assert_eq!(duplicate_ids(&found), Vec::<String>::new());
}

/// AC-01, genre (ADR-0008, ADR-0009): the home is the Latin prefix's; a
/// restatement written under a Cyrillic legacy prefix mapped to `XQ` (built
/// from escapes here) is demoted like a Latin one and keeps its alias.
#[test]
fn ac01_a_legacy_written_restatement_follows_the_latin_prefix_s_home() {
    let legacy = "\u{0416}\u{0414}"; // Zhe, De: no Latin look-alike
    let corpus = Copy::new("legacy");
    corpus.edit(NOTES, "- **XQ-11**:", &format!("- **{legacy}-11**:"));
    let config = replaced(
        &fixture_config(HOMES),
        "[tables]\n",
        &format!("[ids.legacy]\n\"{legacy}\" = \"XQ\"\n[tables]\n"),
    );
    let found = corpus.import_with(&config);
    let restatement = record(&found, NOTES, "XQ-11");
    assert_eq!(restatement.role, Role::Reference);
    assert_eq!(restatement.aliases, [format!("{legacy}-11")]);
    assert_eq!(found.by_home, 1);
    assert_eq!(duplicate_ids(&found), Vec::<String>::new());
}

/// AC-01: `by_home` is within the references, beside `by_document`: a
/// document defining an ID demotes first (the document precedence of
/// import-gaps AC-03), the home finds no record-position definition of it
/// left to weigh.
#[test]
fn ac01_the_document_precedence_runs_before_the_home() {
    let corpus = Copy::new("document");
    corpus.write(
        "bench/xq-12.md",
        "---\nclass: canon\nident: XQ-12\n---\n\n# Wipe\n\nThe probe is wiped after the run.\n",
    );
    let config = replaced(
        &fixture_config(HOMES),
        "[definitions]\n",
        "[documents]\nid_key = \"ident\"\n[definitions]\n",
    );
    let found = corpus.import_with(&config);
    assert_eq!(
        record(&found, "bench/xq-12.md", "XQ-12").form,
        Form::Document
    );
    assert_eq!(record(&found, PROBES, "XQ-12").role, Role::Reference);
    assert_eq!(found.by_document, 1);
    assert_eq!(found.by_home, 1, "only the XQ-11 restatement");
}

// ----------------------------------------------------------------- AC-02

/// A bad home or inherit line in place of the fixture's, the fixture's
/// other lines kept; and the 1-based line of the offending entry.
fn bad_configs() -> Vec<(&'static str, String, usize)> {
    let base = fixture_config(HOMES);
    let home_line = line_holding(&base, "home = ");
    let inherit_line = line_holding(&base, "inherit = ");
    let feature = replaced(
        &base,
        "[tables]\n",
        "feature_prefixes = [\"XQ\"]\n[tables]\n",
    );
    let feature_home = line_holding(&feature, "home = ");
    let table_form = replaced(
        &base,
        HOME_LINE,
        "[definitions.home]\nXQ = [\"bench/probes.md\"]\nYR = []\n",
    );
    let table_line = line_holding(&table_form, "YR = []");
    vec![
        ("home on a feature-scoped prefix", feature, feature_home),
        (
            "home with an empty glob list",
            with_home("home = { XQ = [] }\n"),
            home_line,
        ),
        (
            "home with an empty glob list, table form",
            table_form,
            table_line,
        ),
        (
            "home with an empty glob",
            with_home("home = { XQ = [\"\"] }\n"),
            home_line,
        ),
        (
            "home key in lower case",
            with_home("home = { xq = [\"bench/probes.md\"] }\n"),
            home_line,
        ),
        (
            "home key with a hyphen",
            with_home("home = { \"X-Q\" = [\"bench/probes.md\"] }\n"),
            home_line,
        ),
        (
            "home key empty",
            with_home("home = { \"\" = [\"bench/probes.md\"] }\n"),
            home_line,
        ),
        (
            "home key non-Latin",
            with_home("home = { \"\\u0416\\u0414\" = [\"bench/probes.md\"] }\n"),
            home_line,
        ),
        (
            "home not a table",
            with_home("home = [\"bench/probes.md\"]\n"),
            home_line,
        ),
        (
            "home key an ids.legacy prefix as written",
            replaced(
                &with_home("home = { OLD = [\"bench/probes.md\"] }\n"),
                "[tables]\n",
                "[ids.legacy]\n\"OLD\" = \"XQ\"\n[tables]\n",
            ),
            home_line + 2,
        ),
        (
            "inherit of a key the layout writes",
            replaced(&base, INHERIT_LINE, "inherit = [\"rev\", \"aliases\"]\n"),
            inherit_line,
        ),
        (
            "inherit entry with blanks around",
            replaced(&base, INHERIT_LINE, "inherit = [\"owner \"]\n"),
            inherit_line,
        ),
        (
            "inherit of the task key",
            replaced(
                &base,
                INHERIT_LINE,
                "task_box_key = \"checked\"\ninherit = [\"checked\"]\n",
            ),
            inherit_line + 1,
        ),
        (
            "inherit entry repeated, on its own line",
            replaced(
                &base,
                INHERIT_LINE,
                "inherit = [\n  \"owner\",\n  \"owner\",\n]\n",
            ),
            inherit_line + 2,
        ),
        (
            "inherit not a list",
            replaced(&base, INHERIT_LINE, "inherit = \"owner\"\n"),
            inherit_line,
        ),
        (
            "inherit with an empty key",
            replaced(&base, INHERIT_LINE, "inherit = [\"owner\", \"\"]\n"),
            inherit_line,
        ),
    ]
}

/// AC-02 (M5 a feature-scoped home accepted): each bad home or inherit
/// entry is refused when the config is parsed, at the line that holds it.
#[test]
fn ac02_a_bad_home_or_inherit_is_refused_at_its_line() {
    let origin = Path::new("/nonexistent/census.toml");
    let mut failures = Vec::new();
    for (what, text, line) in bad_configs() {
        match CensusConfig::parse(&text, origin) {
            Ok(_) => failures.push(format!("{what}: accepted")),
            Err(error) => {
                if error.line != Some(line) {
                    failures.push(format!(
                        "{what}: line {:?}, want {line}: {error}",
                        error.line
                    ));
                }
                if !error
                    .to_string()
                    .starts_with(&format!("{}:{line}:", origin.display()))
                {
                    failures.push(format!("{what}: not at <config>:<line>: {error}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AC-02, not vacuous: the fixture config itself, an empty home table and
/// the table form of a valid home all parse.
#[test]
fn ac02_valid_homes_parse() {
    let origin = Path::new("/nonexistent/census.toml");
    for text in [
        fixture_config(HOMES),
        with_home("home = {}\n"),
        with_home(
            "[definitions.home]\nXQ = [\"bench/probes.md\", \"bench/p*.md\"]\nYR = [\"**/gauges.md\"]\n",
        ),
    ] {
        CensusConfig::parse(&text, origin).unwrap_or_else(|error| panic!("{error}\n{text}"));
    }
}

// ----------------------------------------------------------------- AC-04

/// AC-04: an empty home table changes nothing: the import equals the one
/// without the key (records, roles, duplicates, `by_home` 0).
#[test]
fn ac04_an_empty_home_table_is_no_home() {
    let without = import_at(&fixture(HOMES), &with_home(""));
    let empty = import_at(&fixture(HOMES), &with_home("home = {}\n"));
    assert_eq!(without.records, empty.records);
    assert_eq!(duplicate_ids(&without), duplicate_ids(&empty));
    assert_eq!(empty.by_home, 0);
}
