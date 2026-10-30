//! docs/features/import-lead-ins.md on the engine
//! (`specengine_import::import::run`): AC-01 a list item's strong lead-in
//! wrapped onto the item's next lines is a record (V1), with what ends the
//! search for the closing delimiter, a comment crossing the break, blank
//! content on the item's line, the nested-record test ending a parent's
//! text and the hyphenless lead-in position; AC-02 the ID-led list
//! lead-ins no record was taken from (`Import.not_taken`). The CLI side
//! (`records.json`, `not_taken.json`, the stderr suffix, the emitted
//! `[paths]` of AC-03) is end to end in eval `tests/layout_cli.rs` and
//! `tests/import_cli.rs`.
//!
//! The corpus is the invented kiln-log convention
//! `fixtures/import-layout/four`, read in place or copied into a fresh
//! temporary directory with one document added or its config edited there.
//! Non-Latin characters are Unicode escapes only (ADR-0024,
//! `docs/canon/architecture.md` "Repository language").

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_import::CensusConfig;
use specengine_import::import::{self, Form, HyphenlessRole, Import, Role, Scope};

const FOUR: &str = "import-layout/four";

const FIRINGS: &str = "kiln/log/firings.md";
const LIMITS: &str = "kiln/rules/limits.md";
const CELADON: &str = "kiln/rules/glazes/celadon.md";

/// The document an edge case is written to, in a copy of `four` (under a
/// census root, outside the census exclude).
const CASE: &str = "kiln/log/case.md";

/// The `[lists]` switch of the fixture config, as written.
const LEAD_IN_LINE: &str = "lead_in = true\n";

/// The `feature_prefixes` line of the fixture config, as written: a
/// hyphenless pattern is added after it.
const FEATURE_PREFIXES_LINE: &str = "feature_prefixes = [\"GZ\"]\n";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn fixture_config() -> String {
    fs::read_to_string(fixture(FOUR).join("census.toml")).expect("fixture config")
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

/// A copy of `four` in a fresh temporary directory, removed on drop;
/// unique per process and per call.
struct Copy {
    root: PathBuf,
}

impl Copy {
    fn new(case: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "specengine-import-lead-ins-{case}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        copy_dir(&fixture(FOUR), &root);
        Self { root }
    }

    fn write(&self, path: &str, text: &str) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).expect("corpus file writable");
    }

    fn import_with(&self, config: &str) -> Import {
        import_at(&self.root, config)
    }

    fn import(&self) -> Import {
        self.import_with(&fixture_config())
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The import of a copy of `four` with `text` written to [`CASE`].
fn import_case(case: &str, text: &str, config: &str) -> Import {
    let copy = Copy::new(case);
    copy.write(CASE, text);
    copy.import_with(config)
}

/// One record as the spec's table reads it: path, line, ID, title, text,
/// extent, task box.
type Row = (
    String,
    usize,
    String,
    Option<String>,
    String,
    [usize; 2],
    Option<bool>,
);

fn row(
    path: &str,
    line: usize,
    id: &str,
    title: Option<&str>,
    text: &str,
    extent: [usize; 2],
    task_box: Option<bool>,
) -> Row {
    (
        path.to_owned(),
        line,
        id.to_owned(),
        title.map(str::to_owned),
        text.to_owned(),
        extent,
        task_box,
    )
}

/// The records of `found` (all of them, or those in `path`), in record order.
fn rows_in(found: &Import, path: Option<&str>) -> Vec<Row> {
    found
        .records
        .iter()
        .filter(|record| path.is_none_or(|path| record.path == path))
        .map(|record| {
            (
                record.path.clone(),
                record.line,
                record.id.clone(),
                record.title.clone(),
                record.text.clone(),
                record.extent,
                record.task_box,
            )
        })
        .collect()
}

/// The table of docs/features/import-lead-ins.md
/// "Fixture `fixtures/import-layout/four`", in record (path, line) order.
fn the_table() -> Vec<Row> {
    vec![
        row(
            FIRINGS,
            3,
            "KF-21",
            Some("Cone ten reached"),
            "The kiln holds cone ten for an hour.\n  The hold is logged.",
            [3, 5],
            None,
        ),
        row(
            FIRINGS,
            6,
            "KF-22",
            Some("A soak that spans three lines"),
            "The soak ends before the vent opens.",
            [6, 8],
            Some(true),
        ),
        row(
            FIRINGS,
            9,
            "KF-23",
            Some("Parent rule"),
            "The kiln cools slowly.",
            [9, 9],
            None,
        ),
        row(
            FIRINGS,
            10,
            "KF-24",
            Some("Nested and wrapped"),
            "The door stays shut below red heat.",
            [10, 11],
            None,
        ),
        row(
            FIRINGS,
            12,
            "KF-28",
            Some("Lazy wrap"),
            "A lazy line closes the span.",
            [12, 13],
            None,
        ),
        row(
            FIRINGS,
            19,
            "KF-27",
            None,
            "The next item is a record of its own.",
            [19, 19],
            None,
        ),
        row(
            CELADON,
            3,
            "GZ-01",
            Some("Even sheen"),
            "The glaze shows no crawl.",
            [3, 4],
            None,
        ),
        row(
            LIMITS,
            3,
            "KF-12",
            None,
            "The kiln never exceeds cone eleven.",
            [3, 3],
            None,
        ),
    ]
}

/// (path, line, lead) of every not-taken lead-in (or those in `path`).
fn not_taken_in(found: &Import, path: Option<&str>) -> Vec<(String, usize, String)> {
    found
        .not_taken
        .iter()
        .filter(|entry| path.is_none_or(|path| entry.path == path))
        .map(|entry| (entry.path.clone(), entry.line, entry.lead.clone()))
        .collect()
}

fn at(path: &str, line: usize, lead: &str) -> (String, usize, String) {
    (path.to_owned(), line, lead.to_owned())
}

/// The not-taken entries of [`CASE`] as (line, lead).
fn case_not_taken(found: &Import) -> Vec<(usize, String)> {
    not_taken_in(found, Some(CASE))
        .into_iter()
        .map(|(_, line, lead)| (line, lead))
        .collect()
}

// ----------------------------------------------------------------- AC-01

/// AC-01 (V1): on `four` the records are exactly the spec's table: eight
/// `list-item` definitions, six titled, the wrapped titles one line each,
/// the text from the closing line on, the extent to the text's last line,
/// the task box of the item's line; nothing at firings 14 (a blank line
/// ends the span) or 18 (the next item ends it). (M1 the span closes on the
/// item's line only: the wrapped five and GZ-01 missing; M2 a list-item
/// line does not end the search: a record at firings 18; M8 the
/// nested-record test reads one line: KF-23's extent 9-11.)
#[test]
fn ac01_the_wrapped_lead_ins_are_the_records_of_the_table() {
    let found = import_at(&fixture(FOUR), &fixture_config());
    assert_eq!(rows_in(&found, None), the_table());
    assert!(
        found
            .records
            .iter()
            .all(|record| record.form == Form::ListItem && record.role == Role::Definition),
        "{:?}",
        found.records
    );
    for record in &found.records {
        let scope = if record.prefix == "GZ" {
            Scope::Feature
        } else {
            Scope::Project
        };
        assert_eq!(record.scope, scope, "{}", record.id);
    }
    assert_eq!(found.per_form(Form::ListItem), 8);
    assert_eq!(found.titled(), 6);
    assert_eq!(found.role(Role::Definition), 8);
    assert!(found.duplicates.is_empty(), "{:?}", found.duplicates);
    for line in [14, 18] {
        assert!(
            !found
                .records
                .iter()
                .any(|record| record.path == FIRINGS && record.line == line),
            "no record at firings {line}"
        );
    }
    assert_eq!(found.documents, 3, "the retired log is excluded");
}

/// AC-01 ("the ID's token exclusion: the item's line"): a wrapped record's
/// ID is no ID-like token of its line; the two unclosed spans' IDs stay
/// unclaimed tokens and `KF-12 (part)` the one claimed token.
#[test]
fn ac01_a_wrapped_record_s_id_is_no_token_and_the_unclosed_ones_stay_unclaimed() {
    let found = import_at(&fixture(FOUR), &fixture_config());
    let unclaimed: Vec<(String, usize, String)> = found
        .unclaimed
        .iter()
        .map(|token| (token.path.clone(), token.line, token.token.clone()))
        .collect();
    assert_eq!(
        unclaimed,
        [
            (FIRINGS.to_owned(), 14, "KF-25".to_owned()),
            (FIRINGS.to_owned(), 18, "KF-26".to_owned()),
        ]
    );
    assert_eq!(found.claimed, 1, "KF-12 (part) at limits 5");
}

/// AC-01 (M8): the nested-record test reads a wrapped lead-in as the
/// records do: KF-24's item (its span closed on the next line) ends
/// KF-23's text, so KF-23 is its own line alone and KF-24 a record.
#[test]
fn ac01_a_nested_wrapped_record_ends_its_parent_s_text() {
    let found = import_at(&fixture(FOUR), &fixture_config());
    let parent = rows_in(&found, Some(FIRINGS))
        .into_iter()
        .find(|row| row.2 == "KF-23")
        .expect("KF-23 is a record");
    assert_eq!(parent.4, "The kiln cools slowly.");
    assert_eq!(parent.5, [9, 9], "KF-23's extent");
    assert!(
        found
            .records
            .iter()
            .any(|record| record.id == "KF-24" && record.line == 10),
        "KF-24 is a record at its item's line"
    );
}

/// AC-01: `__` wraps as `**` does.
#[test]
fn ac01_an_underscore_span_wraps_like_a_star_span() {
    let found = import_case(
        "underscore",
        "# Case\n\n- __KF-31: Under\n  score__: The text after an underscore span.\n",
        &fixture_config(),
    );
    assert_eq!(
        rows_in(&found, Some(CASE)),
        [row(
            CASE,
            3,
            "KF-31",
            Some("Under score"),
            "The text after an underscore span.",
            [3, 4],
            None,
        )]
    );
    assert_eq!(case_not_taken(&found), []);
}

/// The lines that end the search for the closing delimiter
/// (docs/features/import-lead-ins.md "Rules and edge cases"): each is
/// written between the item's line and a line that would close the span.
fn enders() -> Vec<(&'static str, &'static str)> {
    vec![
        ("a blank line", ""),
        ("a fence", "  ```"),
        ("a tilde fence", "  ~~~"),
        ("a heading", "  ## A heading"),
        ("a table row", "  | a | b |"),
        ("a block quote", "  > quoted"),
        ("a nested list item", "  - nested"),
        ("a list item at column 0", "- next"),
        ("an ordered list item", "1. next"),
        ("a thematic break", "  ___"),
        ("an HTML block", "  <div>"),
        ("a closed comment opening the line", "  <!-- a note -->"),
    ]
}

/// AC-01: a blank line, a fence, a heading, a table row, a block quote, a
/// list item (any indent), a thematic break or an HTML block (`<!--` as
/// written too) ends the search: no record, and the item is counted not
/// taken with its lead to the item line's end.
#[test]
fn ac01_a_line_opening_a_block_ends_the_search() {
    for (name, ender) in enders() {
        let text = format!("# Case\n\n- **KF-31: Cut\n{ender}\n  more**: The text.\n");
        let found = import_case("ender", &text, &fixture_config());
        assert_eq!(
            rows_in(&found, Some(CASE)),
            [],
            "{name} ends the search:\n{text}"
        );
        assert_eq!(
            case_not_taken(&found),
            [(3, "**KF-31: Cut".to_owned())],
            "{name}:\n{text}"
        );
    }
}

/// AC-01, not vacuous: the same item with paragraph text in place of the
/// ender is a record whose span crosses both breaks.
#[test]
fn ac01_paragraph_text_between_continues_the_search() {
    let found = import_case(
        "paragraph",
        "# Case\n\n- **KF-31: Cut\n  plain words\n  more**: The text.\n",
        &fixture_config(),
    );
    assert_eq!(
        rows_in(&found, Some(CASE)),
        [row(
            CASE,
            3,
            "KF-31",
            Some("Cut plain words more"),
            "The text.",
            [3, 5],
            None,
        )]
    );
    assert_eq!(case_not_taken(&found), []);
}

/// AC-01: a line opening inside a comment the span opened is paragraph
/// text, so the span closes after the comment; the title keeps the
/// comment, raw, the break one space.
#[test]
fn ac01_a_line_opening_inside_the_span_s_comment_continues_it() {
    let found = import_case(
        "comment",
        "# Case\n\n- **KF-33: Opens <!-- a note\n  that closes --> here**: The text after the note.\n",
        &fixture_config(),
    );
    assert_eq!(
        rows_in(&found, Some(CASE)),
        [row(
            CASE,
            3,
            "KF-33",
            Some("Opens <!-- a note that closes --> here"),
            "The text after the note.",
            [3, 4],
            None,
        )]
    );
    assert_eq!(case_not_taken(&found), []);
}

/// AC-01: blank content on the item's line is no lead-in (and, no ID-like
/// text opening it, nothing not taken); an ID candidate crossing the break
/// is none (its ID-like start on the item's line is counted not taken).
#[test]
fn ac01_blank_content_or_an_id_crossing_the_break_is_no_lead_in() {
    for (name, text) in [
        ("empty", "# Case\n\n- **\n  KF-34: Late**: The text.\n"),
        ("blanks", "# Case\n\n- **   \n  KF-34: Late**: The text.\n"),
    ] {
        let found = import_case("blank", text, &fixture_config());
        assert_eq!(rows_in(&found, Some(CASE)), [], "{name}");
        assert_eq!(case_not_taken(&found), [], "{name}");
    }
    let found = import_case(
        "crossing",
        "# Case\n\n- **KF-3\n  5: Split**: The text.\n",
        &fixture_config(),
    );
    assert_eq!(rows_in(&found, Some(CASE)), []);
    assert_eq!(case_not_taken(&found), [(3, "**KF-3".to_owned())]);
}

/// AC-01: a hyphenless match at a wrapped lead-in's content start is a
/// definition, as at a closed one; one in prose stays a mention.
#[test]
fn ac01_a_hyphenless_match_at_a_wrapped_lead_in_is_a_definition() {
    let config = replaced(
        &fixture_config(),
        FEATURE_PREFIXES_LINE,
        &format!("{FEATURE_PREFIXES_LINE}hyphenless = ['\\bKF[0-9]{{2}}\\b']\n"),
    );
    let found = import_case(
        "hyphenless",
        "# Case\n\n- **KF41: Hyphen\n  less**: The text.\n\n- **KF42 stays**: Closed on its line.\n\nProse naming KF43.\n",
        &config,
    );
    let matches: Vec<(usize, String, HyphenlessRole)> = found
        .legacy
        .hyphenless
        .iter()
        .filter(|found| found.path == CASE)
        .map(|found| (found.line, found.token.clone(), found.role))
        .collect();
    assert_eq!(
        matches,
        [
            (3, "KF41".to_owned(), HyphenlessRole::Definition),
            (6, "KF42".to_owned(), HyphenlessRole::Definition),
            (8, "KF43".to_owned(), HyphenlessRole::Mention),
        ]
    );
    assert_eq!(
        rows_in(&found, Some(CASE)),
        [],
        "a hyphenless code is no record"
    );
}

/// "Out of scope": paragraph lead-ins are unchanged: a wrapped one is no
/// record, and neither it nor a suffixed one is counted not taken.
#[test]
fn ac01_paragraph_lead_ins_are_unchanged() {
    let found = import_case(
        "para",
        "# Case\n\n**KF-37: Para\nwrapped**: Not a record.\n\n**KF-38.** Not counted either.\n",
        &fixture_config(),
    );
    assert_eq!(rows_in(&found, Some(CASE)), []);
    assert_eq!(case_not_taken(&found), []);
}

/// AC-01: a CRLF copy with a BOM reads the same wrapped records (titles,
/// texts, extents, boxes) and the same not-taken leads.
#[test]
fn ac01_a_crlf_bom_copy_reads_the_same_wrapped_records() {
    let copy = Copy::new("crlf");
    for path in [FIRINGS, LIMITS, CELADON] {
        let text = fs::read_to_string(copy.root.join(path)).unwrap();
        copy.write(path, &format!("\u{feff}{}", text.replace('\n', "\r\n")));
    }
    let found = copy.import();
    assert_eq!(rows_in(&found, None), the_table());
    assert_eq!(not_taken_in(&found, None), four_not_taken());
}

// ----------------------------------------------------------------- AC-02

/// The four not-taken lead-ins of docs/features/import-lead-ins.md
/// "Fixture `fixtures/import-layout/four`", in (path, line) order.
fn four_not_taken() -> Vec<(String, usize, String)> {
    vec![
        at(FIRINGS, 14, "**KF-25: Never closed"),
        at(FIRINGS, 18, "**KF-26: Cut by the next item"),
        at(LIMITS, 4, "**KF-17a.**"),
        at(LIMITS, 5, "**KF-12 (part)**"),
    ]
}

/// AC-02: on `four` the ID-led list lead-ins no record was taken from are
/// the unclosed span, the span cut by the next item, the suffixed ID and
/// the part note, in (path, line) order, each with its lead. (M3 the count
/// always 0.)
#[test]
fn ac02_four_id_led_lead_ins_are_not_taken() {
    let found = import_at(&fixture(FOUR), &fixture_config());
    assert_eq!(not_taken_in(&found, None), four_not_taken());
}

/// AC-02: with `[lists] lead_in = false` nothing is read and nothing is
/// counted not taken.
#[test]
fn ac02_without_lead_in_nothing_is_not_taken() {
    let config = replaced(&fixture_config(), LEAD_IN_LINE, "lead_in = false\n");
    let found = import_at(&fixture(FOUR), &config);
    assert_eq!(not_taken_in(&found, None), []);
    assert_eq!(found.per_form(Form::ListItem), 0);
}

/// AC-02: what is counted and how its lead reads: an item in a comment is
/// not read, an unmapped legacy ID is a `legacy.unmapped` token, not a
/// lead-in not taken; a task box and an ordered marker are skipped, a
/// comment after the closing delimiter is outside the lead, a comment and
/// blanks ending an unclosed span's line are trimmed off.
#[test]
fn ac02_what_is_counted_and_its_lead() {
    let unmapped = "\u{041F}\u{0420}-31";
    let text = format!(
        "# Case\n\n<!--\n- **KF-17b.** Inside a comment.\n-->\n\n- **{unmapped}**: An unmapped legacy ID.\n\n- **KF-17c.** <!-- a note --> Counted.\n- [ ] **KF-17e.** Boxed.\n1. **KF-17f.** Ordered.\n- **KF-29: Unclosed <!-- a note -->   \n\nEnd.\n"
    );
    let found = import_case("counted", &text, &fixture_config());
    assert_eq!(
        case_not_taken(&found),
        [
            (9, "**KF-17c.**".to_owned()),
            (10, "**KF-17e.**".to_owned()),
            (11, "**KF-17f.**".to_owned()),
            (12, "**KF-29: Unclosed".to_owned()),
        ]
    );
    assert!(
        found
            .legacy
            .unmapped
            .iter()
            .any(|token| token.path == CASE && token.line == 7 && token.token == unmapped),
        "the unmapped legacy ID is a legacy.unmapped token: {:?}",
        found.legacy.unmapped
    );
    assert_eq!(rows_in(&found, Some(CASE)), []);
}

/// AC-02: the not-taken list is in (path, line) order across documents and
/// changes no other count: the same copy with one more suffixed lead-in
/// sorting first gains exactly that entry.
#[test]
fn ac02_a_planted_suffix_is_one_more_entry_and_nothing_else_moves() {
    let base = import_at(&fixture(FOUR), &fixture_config());
    let copy = Copy::new("planted");
    copy.write("kiln/log/early.md", "# Early\n\n- **KF-17g.** Planted.\n");
    let planted = copy.import();
    let mut want = vec![at("kiln/log/early.md", 3, "**KF-17g.**")];
    want.extend(four_not_taken());
    assert_eq!(not_taken_in(&planted, None), want);
    assert_eq!(rows_in(&planted, None), rows_in(&base, None));
    assert_eq!(planted.documents, base.documents + 1);
    assert_eq!(planted.unclaimed.len(), base.unclaimed.len());
}

/// Determinism: two imports of `four` give the same records and the same
/// not-taken list.
#[test]
fn two_imports_of_four_are_identical() {
    let first = import_at(&fixture(FOUR), &fixture_config());
    let second = import_at(&fixture(FOUR), &fixture_config());
    assert_eq!(first.records, second.records);
    assert_eq!(first.not_taken, second.not_taken);
}
