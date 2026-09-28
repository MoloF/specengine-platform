//! The census of `specengine-import` on real files (docs/features/phase-0-spikes.md,
//! spike group 3, AC-12): config validation with file and line, ID scripts and
//! look-alike normalization, what the Markdown scanner hides, front-matter
//! variants, record tables, wiki links, excludes, section extent, and
//! robustness on generated pathological Markdown.
//!
//! Every corpus is written into a fresh temporary directory by the test; the
//! convention is invented (prefixes `ZR`, `ZN`). Non-Latin characters are
//! Unicode escapes only (ADR-0024).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use specengine_import::{Census, CensusConfig, FrontMatterState, IdScript, RecordKind, run};

// ------------------------------------------------------------------ helpers

/// A corpus in a temporary directory, removed on drop.
struct Corpus(PathBuf);

impl Corpus {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-import-census-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("corpus directory");
        Self(path)
    }

    fn write(&self, relative: &str, content: impl AsRef<[u8]>) -> &Self {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("parent directory");
        fs::write(&path, content).expect("write corpus file");
        self
    }

    fn census(&self, config: &str) -> Census {
        let config = parse(config);
        run(&self.0, &config).expect("census runs")
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The spec's sample config (`fixtures/corpus-mini/census.toml`) plus extra lines.
fn config(extra: &str) -> String {
    format!(
        "[corpus]\nroots = [\"design\"]\n{extra}\n[front_matter]\nclass_key = \"kind\"\n\
         [ids]\nregex = '^[A-Z]{{2}}-[0-9]{{3}}$'\n[tables]\nid_column = 0\n\
         [sections]\nid_attr = true\n"
    )
}

fn parse(text: &str) -> CensusConfig {
    CensusConfig::parse(text, Path::new("census.toml"))
        .unwrap_or_else(|error| panic!("config must parse: {error}\n{text}"))
}

fn config_error(text: &str) -> specengine_import::ConfigError {
    match CensusConfig::parse(text, Path::new("conf/census.toml")) {
        Ok(_) => panic!("config must be rejected:\n{text}"),
        Err(error) => error,
    }
}

fn ids(census: &Census) -> Vec<(String, RecordKind)> {
    census
        .records
        .iter()
        .map(|record| (record.id.clone(), record.kind))
        .collect()
}

fn hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

const TABLE_HEAD: &str = "| ID | Text |\n|---|---|\n";

// ------------------------------------------------------ config: file and line

/// `(config text, line the error must name, a word the message must carry)`.
fn broken_configs() -> Vec<(String, usize, &'static str)> {
    let head = "[corpus]\nroots = [\"design\"]\n";
    let ids = "[ids]\nregex = '^[A-Z]{2}-[0-9]{3}$'\n";
    vec![
        (format!("{head}colour = \"blue\"\n{ids}"), 3, "colour"),
        (format!("{head}{ids}[extra]\nkey = 1\n"), 5, "extra"),
        (
            format!("{head}{ids}[tables]\nid_colum = 1\n"),
            6,
            "id_colum",
        ),
        (format!("{head}[ids]\nregex = '^[A-Z'\n"), 4, "ids.regex"),
        (format!("{head}[ids]\nregex = '^[A-Z]*$'\n"), 4, "empty"),
        (
            format!("{head}{ids}[tables]\nid_header = '(ID'\n"),
            6,
            "id_header",
        ),
        (
            format!("{head}{ids}[tables]\nid_column = \"zero\"\n"),
            6,
            "",
        ),
        (
            format!("[corpus]\nroots = [\"design\"] trailing\n{ids}"),
            2,
            "",
        ),
        (
            format!("{head}extensions = [\"md\", \"a/b\"]\n{ids}"),
            3,
            "a/b",
        ),
        (
            format!("{head}{ids}[front_matter]\nclass_key = \"  \"\n"),
            6,
            "class_key",
        ),
        (
            format!("{head}{ids}[links]\nwiki_root = \"design\"\n"),
            6,
            "wiki",
        ),
        (
            format!("{head}{ids}[links]\nwiki = true\nwiki_root = \"../elsewhere\"\n"),
            7,
            "..",
        ),
    ]
}

#[test]
fn config_errors_name_the_file_and_the_line() {
    for (text, line, word) in broken_configs() {
        let error = config_error(&text);
        assert_eq!(error.path, Path::new("conf/census.toml"), "{text}");
        assert_eq!(error.line, Some(line), "{error}\n--- config:\n{text}");
        let shown = error.to_string();
        assert!(
            shown.starts_with(&format!("conf/census.toml:{line}: ")),
            "Display must lead with file:line, got {shown}"
        );
        assert!(
            shown.contains(word),
            "message must mention {word:?}: {shown}"
        );
    }
}

#[test]
fn empty_lists_are_reported_at_their_line() {
    for text in [
        "[corpus]\nroots = []\n[ids]\nregex = '^[A-Z]{2}-[0-9]{3}$'\n",
        "[corpus]\nextensions = []\n[ids]\nregex = '^[A-Z]{2}-[0-9]{3}$'\n",
    ] {
        let error = config_error(text);
        assert!(error.message.contains("empty"), "{error}");
        assert_eq!(error.line, Some(2), "{error}\n--- config:\n{text}");
    }
}

#[test]
fn a_missing_ids_table_names_the_file() {
    let error = config_error("[corpus]\nroots = [\"design\"]\n");
    assert_eq!(error.path, Path::new("conf/census.toml"));
    assert!(error.to_string().contains("ids"), "{error}");
}

#[test]
fn roots_escaping_the_corpus_are_rejected() {
    let ids = "[ids]\nregex = '^[A-Z]{2}-[0-9]{3}$'\n";
    for root in ["..", "../outside", "design/../../outside", "/etc", ""] {
        let text = format!("[corpus]\nroots = [\"design\", \"{root}\"]\n{ids}");
        let error = config_error(&text);
        assert_eq!(error.line, Some(2), "{error}");
        assert!(error.message.contains("corpus.roots"), "{error}");
    }
    // Inner `..` that stays inside is still rejected: the rule is "without `..`".
    let error = config_error(&format!("[corpus]\nroots = [\"design/../design\"]\n{ids}"));
    assert_eq!(error.line, Some(2), "{error}");
    // `.` and nested roots are fine.
    parse(&format!("[corpus]\nroots = [\".\", \"design/sub\"]\n{ids}"));
}

#[test]
fn the_sample_config_of_the_spec_parses() {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus-mini/census.toml"),
    )
    .expect("fixture config readable");
    let config = parse(&text);
    assert_eq!(config.roots, vec![PathBuf::from("design")]);
    assert_eq!(config.class_key.as_deref(), Some("kind"));
    assert_eq!(config.id_column, 0);
    assert!(config.section_ids);
    assert_eq!(config.extensions, vec!["md".to_owned()]);
    assert!(config.wiki_root.is_none());
    assert!(!config.headerless_tables);
}

// -------------------------------------------------------- ID script (escapes)

#[test]
fn id_script_classifies_by_escapes() {
    let latin = ["ZR-003", "zr-003", "123-456", ""];
    for text in latin {
        assert_eq!(IdScript::of(text), IdScript::Latin, "{text:?}");
    }
    let mixed = [
        "\u{0396}R-003",               // Greek capital Zeta for Z
        "Z\u{0420}-003",               // Cyrillic capital Er for P
        "\u{FF3A}R-003",               // fullwidth Z
        "ZR-\u{0663}\u{0660}\u{0663}", // Arabic-Indic digits
        "Z\u{00C9}-003",               // accented Latin letter is foreign too
        "ZR-003\u{0430}",              // Cyrillic small a at the end
    ];
    for text in mixed {
        assert_eq!(IdScript::of(text), IdScript::MixedScript, "{text:?}");
    }
    let non_latin = [
        "\u{0417}\u{0420}-003", // Cyrillic Ze + Er
        "\u{0410}\u{0412}-004", // Cyrillic look-alikes only, no ASCII letter
        "\u{FF3A}\u{FF32}-005", // fullwidth only
        "\u{0661}\u{0662}",
    ];
    for text in non_latin {
        assert_eq!(IdScript::of(text), IdScript::NonLatin, "{text:?}");
    }
}

#[test]
fn look_alikes_are_normalized_before_matching_and_kept_verbatim() {
    let rows = [
        "\u{0421}R-001",        // Cyrillic Es for C: mixed
        "Z\u{0420}-002",        // Cyrillic Er for P: mixed
        "\u{FF3A}\u{FF32}-003", // fullwidth Z R: non-Latin
        "\u{0410}\u{0412}-004", // Cyrillic A, Ve: non-Latin
        "\u{0416}R-005",        // Cyrillic Zhe is no look-alike: no ID
        "ZR-\u{FF10}\u{FF10}6", // fullwidth digits: mixed
        "ZR-007",               // Latin
    ];
    let mut body = String::from(TABLE_HEAD);
    for id in rows {
        body.push_str(&format!("| {id} | row |\n"));
    }
    body.push_str("\n## A section {#\u{0396}N-010}\n\nText.\n");
    let corpus = Corpus::new("look-alikes");
    corpus.write("design/rules.md", &body);
    let census = corpus.census(&config(""));

    let got: Vec<(&str, &str, &str, IdScript)> = census
        .records
        .iter()
        .map(|r| {
            (
                r.verbatim_id.as_str(),
                r.id.as_str(),
                r.prefix.as_str(),
                r.script,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("\u{0421}R-001", "CR-001", "CR", IdScript::MixedScript),
            ("Z\u{0420}-002", "ZP-002", "ZP", IdScript::MixedScript),
            ("\u{FF3A}\u{FF32}-003", "ZR-003", "ZR", IdScript::NonLatin),
            ("\u{0410}\u{0412}-004", "AB-004", "AB", IdScript::NonLatin),
            (
                "ZR-\u{FF10}\u{FF10}6",
                "ZR-006",
                "ZR",
                IdScript::MixedScript
            ),
            ("ZR-007", "ZR-007", "ZR", IdScript::Latin),
            ("\u{0396}N-010", "ZN-010", "ZN", IdScript::MixedScript),
        ]
    );
    assert_eq!(census.id_rows(), 6);
    assert_eq!(census.id_sections(), 1);
    assert_eq!(census.mixed_script_ids(), 4);
    assert_eq!(census.non_latin_ids(), 2);
    assert_eq!(census.rows_without_id.len(), 1, "the Zhe row has no ID");
    assert_eq!(census.rows_without_id[0].line, 7);
    assert_eq!(census.per_prefix().get("ZR"), Some(&3));
}

// ----------------------------------------------- what the scanner must hide

#[test]
fn fenced_code_html_comments_and_code_spans_hide_records_and_links() {
    let body = format!(
        "# Exclusions\n\
         \n\
         ```text\n\
         {TABLE_HEAD}| ZR-101 | in a backtick fence |\n\
         ## Fenced heading {{#ZR-102}}\n\
         [x](fenced-missing.md)\n\
         ```\n\
         \n\
         ~~~~\n\
         {TABLE_HEAD}| ZR-103 | in a tilde fence |\n\
         ~~~~\n\
         \n\
         <!-- {TABLE_HEAD}| ZR-104 | in a comment |\n\
         ## Commented heading {{#ZR-105}}\n\
         [x](commented-missing.md) -->\n\
         \n\
         Inline <!-- [x](inline-missing.md) --> comment, a `[x](span-missing.md)` code span\n\
         and a ``[y](double-missing.md)`` one.\n\
         \n\
         ## Heading with a hidden anchor <!-- {{#ZR-106}} -->\n\
         \n\
         A code span `<!--` opens no comment.\n\
         \n\
         {TABLE_HEAD}| ZR-107 | visible |\n\
         \n\
         ## Kept {{#ZR-108}}\n"
    );
    let corpus = Corpus::new("hidden");
    corpus.write("design/hidden.md", &body);
    let census = corpus.census(&config(""));
    assert_eq!(
        ids(&census),
        vec![
            ("ZR-107".to_owned(), RecordKind::Row),
            ("ZR-108".to_owned(), RecordKind::Section),
        ],
        "only the visible row and section count"
    );
    assert_eq!(census.broken_links, vec![], "hidden links are not checked");
    assert_eq!(census.links_checked(), 0);
    assert_eq!(census.tables(), 1, "hidden tables are not tables");
    assert_eq!(census.other_anchors, 0);
}

// ------------------------------------------------------------ front-matter

#[test]
fn crlf_and_bom_front_matter_are_read() {
    let corpus = Corpus::new("front-matter");
    corpus
        .write(
            "design/crlf.md",
            "---\r\nkind: rule\r\n---\r\n\r\n| ID | Text |\r\n|---|---|\r\n| ZR-201 | CRLF row. |\r\n",
        )
        .write(
            "design/bom.md",
            "\u{FEFF}---\nkind: note\n---\n| ID | Text |\n|---|---|\n| ZR-202 | BOM row. |\n",
        )
        .write(
            "design/dots.md",
            "---\nkind: \"rule\" # a YAML comment\n  kind: nested\n...\nBody.\n",
        )
        .write("design/plain.md", "No front-matter.\n\n---\n\nkind: rule\n")
        .write("design/keyless.md", "---\ntitle: x\n---\nBody.\n");
    let census = corpus.census(&config(""));
    assert_eq!(census.documents, 5);
    assert_eq!(census.with_front_matter(), 4);
    let per_class = census.per_class();
    assert_eq!(per_class.get(&Some("rule".to_owned())), Some(&2));
    assert_eq!(per_class.get(&Some("note".to_owned())), Some(&1));
    assert_eq!(
        per_class.get(&None),
        Some(&1),
        "a block without the class key"
    );
    assert_eq!(census.diagnostics, vec![]);

    let crlf = census
        .records
        .iter()
        .find(|r| r.id == "ZR-201")
        .expect("CRLF row");
    assert_eq!((crlf.path.as_str(), crlf.line), ("design/crlf.md", 7));
    assert_eq!(
        crlf.blake3,
        hash("| ZR-201 | CRLF row. |"),
        "verbatim without CR"
    );
    let bom = census
        .records
        .iter()
        .find(|r| r.id == "ZR-202")
        .expect("BOM row");
    assert_eq!((bom.path.as_str(), bom.line), ("design/bom.md", 6));
}

#[test]
fn unclosed_front_matter_is_a_diagnostic_and_the_body_is_still_read() {
    let corpus = Corpus::new("unclosed");
    corpus.write(
        "design/unclosed.md",
        format!("---\nkind: rule\n\n{TABLE_HEAD}| ZR-203 | read as body |\n"),
    );
    let census = corpus.census(&config(""));
    assert_eq!(census.documents, 1);
    assert_eq!(census.with_front_matter(), 0);
    assert_eq!(census.front_matter_unclosed(), 1);
    assert!(census.per_class().is_empty());
    assert_eq!(
        census.documents_detail[0].front_matter,
        FrontMatterState::Unclosed
    );
    assert_eq!(census.diagnostics.len(), 1, "{:?}", census.diagnostics);
    let diagnostic = &census.diagnostics[0];
    assert_eq!(diagnostic.path, "design/unclosed.md");
    assert_eq!(diagnostic.line, Some(1));
    assert!(
        diagnostic.message.contains("never closed"),
        "{diagnostic:?}"
    );
    assert_eq!(ids(&census), vec![("ZR-203".to_owned(), RecordKind::Row)]);
    assert_eq!(census.records[0].line, 6);
}

// ------------------------------------------------------------ record tables

fn id_header_corpus(name: &str) -> Corpus {
    let corpus = Corpus::new(name);
    corpus.write(
        "design/tables.md",
        "| ID | Text |\n|---|---|\n| ZR-301 | counted |\n|  | no id |\n\n\
         | Key | Text |\n|---|---|\n| ZR-302 | ID under another header |\n| x | no id |\n\n\
         | ID | Text |\n|:--|--:|\n| n/a | an ID header without IDs |\n",
    );
    corpus
}

#[test]
fn id_header_selects_record_tables_by_header() {
    let corpus = id_header_corpus("id-header-on");
    let census = corpus.census(&config("").replace("[tables]\n", "[tables]\nid_header = '^ID$'\n"));
    assert_eq!(ids(&census), vec![("ZR-301".to_owned(), RecordKind::Row)]);
    assert_eq!(census.tables(), 3);
    assert_eq!(
        census.record_tables(),
        2,
        "both `ID` tables, not the `Key` one"
    );
    let lines: Vec<usize> = census.rows_without_id.iter().map(|l| l.line).collect();
    assert_eq!(lines, vec![4, 13]);
}

#[test]
fn without_id_header_any_table_with_an_id_is_a_record_table() {
    let corpus = id_header_corpus("id-header-off");
    let census = corpus.census(&config(""));
    assert_eq!(
        ids(&census),
        vec![
            ("ZR-301".to_owned(), RecordKind::Row),
            ("ZR-302".to_owned(), RecordKind::Row),
        ]
    );
    assert_eq!(census.record_tables(), 2);
    let lines: Vec<usize> = census.rows_without_id.iter().map(|l| l.line).collect();
    assert_eq!(lines, vec![4, 9]);
}

fn headerless_corpus(name: &str) -> Corpus {
    let corpus = Corpus::new(name);
    corpus.write(
        "design/appended.md",
        "Some text.\n\n| ZR-401 | appended row |\n| ZR-402 | another |\n|  | no id |\n\n\
         | plain | block |\n| without | ids |\n",
    );
    corpus
}

#[test]
fn headerless_blocks_become_record_tables_only_when_configured() {
    let on = headerless_corpus("headerless-on")
        .census(&config("").replace("[tables]\n", "[tables]\nheaderless = true\n"));
    assert_eq!(
        ids(&on),
        vec![
            ("ZR-401".to_owned(), RecordKind::Row),
            ("ZR-402".to_owned(), RecordKind::Row),
        ]
    );
    assert_eq!(on.headerless_blocks(), 2);
    assert_eq!(on.headerless_id_rows, 2);
    assert_eq!(on.record_tables(), 1);
    assert_eq!(on.rows_without_id.len(), 1);
    assert_eq!(on.tables(), 0, "headerless blocks are not GFM tables");

    let off = headerless_corpus("headerless-off").census(&config(""));
    assert!(off.records.is_empty());
    assert_eq!(off.headerless_blocks(), 2);
    assert_eq!(off.headerless_id_rows, 2, "counted as a hint even when off");
    assert_eq!(off.record_tables(), 0);
    assert!(off.rows_without_id.is_empty());
}

#[test]
fn id_column_other_than_zero_and_decorated_cells() {
    let corpus = Corpus::new("id-column");
    corpus.write(
        "design/t.md",
        "| Text | ID |\n|---|---|\n| a | **ZR-601** |\n| b | `ZR-602` |\n| c | [ZR-603](t.md) |\n\
         | d | ~~ZR-604~~ |\n| e | ZR-60 |\n",
    );
    let census = corpus.census(&config("").replace("id_column = 0", "id_column = 1"));
    let got: Vec<String> = census.records.iter().map(|r| r.id.clone()).collect();
    assert_eq!(got, ["ZR-601", "ZR-602", "ZR-603", "ZR-604"]);
    assert_eq!(census.rows_without_id.len(), 1, "`ZR-60` does not match");
}

// -------------------------------------------------------------------- links

#[test]
fn markdown_links_resolve_relative_to_the_document_or_the_root() {
    let corpus = Corpus::new("links");
    corpus
        .write("design/rules.md", "Rules.\n")
        .write("design/sub/deep-page.md", "Deep.\n")
        .write("design/img/pic.png", [0u8, 1, 2])
        .write("design/with space.md", "Spaced.\n")
        .write(
            "design/links.md",
            "[a](rules.md) [b](./sub/deep-page.md#part) [c](../design/rules.md)\n\
             [d](/design/rules.md) ![p](img/pic.png) [s](<with space.md>) [t](with%20space.md)\n\
             [e](https://example.com/missing.md) [f](#local) [g](mailto:x@example.com)\n\
             [h](missing.md \"title\") [i](sub/missing.md)\n\
             \n\
             [ref]: missing-ref.md\n\
             [^note]: footnote-not-a-link.md\n",
        );
    let census = corpus.census(&config(""));
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(
        broken,
        vec![
            (4, "missing.md"),
            (4, "sub/missing.md"),
            (6, "missing-ref.md")
        ]
    );
    assert!(
        census
            .broken_links
            .iter()
            .all(|l| l.path == "design/links.md")
    );
    assert_eq!(census.links_checked(), 10);
}

fn wiki_corpus(name: &str) -> Corpus {
    let corpus = Corpus::new(name);
    corpus
        .write("design/rules.md", "Rules.\n")
        .write("design/sub/deep-page.md", "Deep.\n")
        .write("design/img/pic.png", [0u8])
        .write("other/outside.md", "Outside the wiki root.\n")
        .write(
            "design/wiki.md",
            "[[rules]] [[Rules]] [[sub/deep-page]] [[deep-page]] [[rules#Section]]\n\
             [[rules|Alias]] ![[pic.png]] [[missing-page]] [[sub/missing]] [[]]\n\
             [[outside]] `[[in-code-span]]`\n",
        );
    corpus
}

#[test]
fn wiki_links_resolve_under_the_wiki_root() {
    let census = wiki_corpus("wiki-on").census(&config("").replace(
        "[sections]\nid_attr = true\n",
        "[sections]\nid_attr = true\n[links]\nwiki = true\nwiki_root = \"design\"\n",
    ));
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(
        broken,
        vec![(2, "missing-page"), (2, "sub/missing"), (3, "outside")]
    );
    assert_eq!(census.links_checked(), 10);
}

/// What the inline-link, code-span and wiki scanners extract, row by row
/// (CommonMark 0.31 for inline links and code spans). Rows are separate
/// paragraphs except the two lines of the HTML comment. Backslashes are
/// dropped from reported targets before comparing, so the table checks
/// where a destination ends, not how the escape is echoed.
#[test]
fn inline_link_extent_titles_code_spans_comments_and_wiki_table() {
    let corpus = Corpus::new("link-table");
    corpus
        .write("design/rules.md", "Rules.\n")
        .write("design/dir(1)/page.md", "Page.\n")
        .write("design/with space.md", "Spaced.\n")
        .write(
            "design/table.md",
            concat!(
                // 1: nested balanced parentheses stay in a bare destination
                "[n1](dir(1)/page.md) [n2](dir(1)/missing-a.md) [n3](dir((2))/missing-b.md)\n\n",
                // 3: a balanced pair before a title / at the end
                "[n4](missing-c.md(x) \"t\") [n5](missing-d.md(x)(y))\n\n",
                // 5: `\\)` does not close; `\\\\` is an escaped backslash;
                //    e3 is not a link (`still` is no title)
                "[e2](missing-e\\).md) [e4](missing-y.md\\\\) ",
                "[e3](missing-f.md\\) still)\n\n",
                // 7: `<…>` destinations with blanks and `)`; s4 is not a link
                "[s1](<with space.md>) [s2](<missing g.md> \"t\") [s3](<a)b missing.md>) ",
                "[s4](<missing h.md>junk)\n\n",
                // 9: the three title forms, a `)` inside a title, a blank before `)`
                "[t1](rules.md \"double\") [t2](missing-i.md 'single') [t3](missing-j.md (paren)) ",
                "[t4](missing-k.md \"a ) b\") [t5](rules.md 'x' )\n\n",
                // 11: an unclosed title and a non-title are no links
                "[t6](missing-l.md \"unclosed) [t7](missing-m.md junk)\n\n",
                // 13: a `` span holding a ` hides x; an unmatched `` and ` do not
                "``a ` [x](missing-n.md) `` and `` [y](missing-o.md) ` then [z](missing-p.md)\n\n",
                // 15: a code span hides q; an escaped backtick opens none
                "`[q](missing-q.md)` \\`[r](missing-r.md)`\n\n",
                // 17-18: an inline comment and one that runs into the next line
                "a <!-- [x](missing-s.md) --> b [y](missing-t.md) <!-- [z](missing-u.md)\n",
                "still hidden [w](missing-v.md) --> [v](missing-w.md)\n",
            ),
        )
        .write(
            "design/wiki-table.md",
            "[[rules]] [[missing-wiki\n\n[[a]] [[b\n\n[[unclosed [x](missing-x.md)\n",
        );
    let census = corpus.census(&config("").replace(
        "[sections]\nid_attr = true\n",
        "[sections]\nid_attr = true\n[links]\nwiki = true\nwiki_root = \"design\"\n",
    ));
    let broken_in = |path: &str| -> Vec<(usize, String)> {
        census
            .broken_links
            .iter()
            .filter(|l| l.path == path)
            .map(|l| (l.line, l.target.replace('\\', "")))
            .collect()
    };
    let checked_in = |path: &str| {
        census
            .documents_detail
            .iter()
            .find(|d| d.path == path)
            .map(|d| d.links_checked)
    };
    let expected = |rows: &[(usize, &str)]| -> Vec<(usize, String)> {
        rows.iter().map(|&(l, t)| (l, t.to_owned())).collect()
    };

    assert_eq!(
        broken_in("design/table.md"),
        expected(&[
            (1, "dir(1)/missing-a.md"),
            (1, "dir((2))/missing-b.md"),
            (3, "missing-c.md(x)"),
            (3, "missing-d.md(x)(y)"),
            (5, "missing-e).md"),
            (5, "missing-y.md"),
            (7, "missing g.md"),
            (7, "a)b missing.md"),
            (9, "missing-i.md"),
            (9, "missing-j.md"),
            (9, "missing-k.md"),
            (13, "missing-o.md"),
            (13, "missing-p.md"),
            (15, "missing-r.md"),
            (17, "missing-t.md"),
            (18, "missing-w.md"),
        ])
    );
    assert_eq!(checked_in("design/table.md"), Some(20));
    assert_eq!(
        broken_in("design/wiki-table.md"),
        expected(&[(3, "a"), (5, "missing-x.md")]),
        "`[[a]] [[b` is one wiki link; a Markdown link after an unclosed `[[` is found"
    );
    assert_eq!(checked_in("design/wiki-table.md"), Some(3));
}

/// A backslash escape in a link destination stands for the escaped character
/// (CommonMark 0.31, "Backslash escapes"; GitHub links `[a](x\_y.md)` to
/// `x_y.md`): each of these names an existing file, so none is broken.
#[test]
fn backslash_escapes_in_destinations_name_the_unescaped_file() {
    let corpus = Corpus::new("escaped-destinations");
    corpus
        .write("design/paren)name.md", "Paren.\n")
        .write("design/under_score.md", "Underscore.\n")
        .write(
            "design/escaped.md",
            "[e1](paren\\)name.md) [e2](under\\_score.md) [e3](<under\\_score.md>)\n",
        );
    let census = corpus.census(&config(""));
    let broken: Vec<&str> = census
        .broken_links
        .iter()
        .map(|l| l.target.as_str())
        .collect();
    assert_eq!(census.links_checked(), 3);
    assert_eq!(
        broken,
        Vec::<&str>::new(),
        "escaped destinations reported broken"
    );
}

/// A backslash before a character that is not ASCII punctuation is a literal
/// backslash (CommonMark 0.31): `dir\file.md` names a file with a backslash in
/// its name, not `dirfile.md` and not `dir/file.md`. The file system of the
/// owner's machine (APFS) holds such names.
#[test]
fn a_backslash_before_a_non_punctuation_character_stays_literal() {
    let corpus = Corpus::new("literal-backslash");
    corpus
        .write("design/dir\\file.md", "Backslash in the name.\n")
        .write("design/other/file.md", "A directory, not a backslash.\n")
        .write("design/otherfile.md", "The backslash is not dropped.\n")
        .write(
            "design/literal.md",
            "[x](dir\\file.md) [y](other\\file.md)\n",
        );
    let census = corpus.census(&config(""));
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(census.links_checked(), 2);
    assert_eq!(broken, vec![(1, "other\\file.md")]);
}

/// `\#` is an escaped `#`: the destination is an anchor and is not checked,
/// bare or in `<…>`.
#[test]
fn an_escaped_hash_is_an_anchor_and_not_checked() {
    let corpus = Corpus::new("escaped-anchor");
    corpus.write("design/rules.md", "Rules.\n").write(
        "design/anchors.md",
        "[x](\\#anchor) [y](<\\#other-anchor>) [z](rules.md)\n",
    );
    let census = corpus.census(&config(""));
    assert_eq!(census.broken_links, vec![]);
    assert_eq!(census.links_checked(), 1, "only `rules.md` is checked");
}

/// `%5C` decodes to a literal backslash after backslash escapes are resolved,
/// so it never escapes the next character: `c%5C_d.md` names `c\_d.md`, not
/// `c_d.md`, and `%5C#anchor` is a file named `\`, not an anchor.
#[test]
fn percent_5c_is_a_literal_backslash_not_an_escape() {
    let corpus = Corpus::new("percent-backslash");
    corpus
        .write("design/a\\b.md", "Backslash in the name.\n")
        .write("design/c\\_d.md", "Backslash and underscore.\n")
        .write("design/e_f.md", "Underscore only.\n")
        .write(
            "design/percent.md",
            "[p](a%5Cb.md) [q](c%5C_d.md) [r](e%5C_f.md) [s](%5C#anchor)\n",
        );
    let census = corpus.census(&config(""));
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(census.links_checked(), 4);
    assert_eq!(broken, vec![(1, "e%5C_f.md"), (1, "%5C#anchor")]);
}

/// A broken link reports its destination as written: backslash escapes and
/// percent-escapes are kept, only the `<…>` brackets are not part of it.
#[test]
fn a_broken_link_reports_the_destination_as_written() {
    let corpus = Corpus::new("escaped-broken");
    corpus.write(
        "design/broken.md",
        "[m](miss\\_ing.md) [n](<gone\\)here.md>)\n[o](lost%5C.md) [p](a%20b\\_c.md#part)\n",
    );
    let census = corpus.census(&config(""));
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(census.links_checked(), 4);
    assert_eq!(
        broken,
        vec![
            (1, "miss\\_ing.md"),
            (1, "gone\\)here.md"),
            (2, "lost%5C.md"),
            (2, "a%20b\\_c.md#part"),
        ]
    );
}

#[test]
fn wiki_links_are_not_checked_when_off() {
    let census = wiki_corpus("wiki-off").census(&config(""));
    assert_eq!(census.broken_links, vec![]);
    assert_eq!(census.links_checked(), 0);
}

// ------------------------------------------------------ walking the corpus

#[test]
fn exclude_globs_extensions_dot_directories_and_symlinks() {
    let corpus = Corpus::new("walk");
    corpus
        .write("design/keep.md", "Kept.\n")
        .write("design/xy.md", "Kept too.\n")
        .write("design/UPPER.MD", "Extension case is ignored.\n")
        .write("design/x.md", "Excluded by `?`.\n")
        .write("design/old/a.md", "Excluded by `**`.\n")
        .write("design/old/deeper/b.md", "Excluded by `**`.\n")
        .write("design/sub/draft-x.md", "Excluded by `**/`.\n")
        .write(
            "design/draft-y.md",
            "Excluded by `**/` with no directory.\n",
        )
        .write("design/notes.txt", "Not a document.\n")
        .write("design/.hidden/h.md", "Dot-directory.\n")
        .write("design/.md", "No stem.\n")
        .write("outside.md", "Outside the roots.\n");
    std::os::unix::fs::symlink(
        corpus.0.join("design/keep.md"),
        corpus.0.join("design/link.md"),
    )
    .expect("symlink");
    let census = corpus.census(&config(
        "exclude = [\"design/old/**\", \"**/draft-*.md\", \"design/?.md\"]",
    ));
    let paths: Vec<&str> = census
        .documents_detail
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(paths, ["design/UPPER.MD", "design/keep.md", "design/xy.md"]);
    assert_eq!(census.documents, 3);
}

#[test]
fn a_missing_root_is_a_diagnostic_not_a_failure() {
    let corpus = Corpus::new("missing-root");
    corpus.write("design/a.md", "A.\n");
    let census = corpus
        .census(&config("").replace("roots = [\"design\"]", "roots = [\"design\", \"absent\"]"));
    assert_eq!(census.documents, 1);
    assert_eq!(census.roots_missing, 1);
    assert_eq!(census.diagnostics.len(), 1);
    assert_eq!(census.diagnostics[0].path, "absent");
}

#[test]
fn a_non_utf8_file_is_skipped_with_a_diagnostic() {
    let corpus = Corpus::new("non-utf8");
    corpus
        .write("design/good.md", format!("{TABLE_HEAD}| ZR-701 | fine |\n"))
        .write(
            "design/latin1.md",
            b"| ID | Text |\n|---|---|\n| ZR-702 | caf\xe9 |\n".as_slice(),
        );
    let census = corpus.census(&config(""));
    assert_eq!(census.files_skipped, 1);
    assert_eq!(ids(&census), vec![("ZR-701".to_owned(), RecordKind::Row)]);
    assert_eq!(census.diagnostics.len(), 1);
    assert_eq!(census.diagnostics[0].path, "design/latin1.md");
    assert!(census.diagnostics[0].message.contains("UTF-8"));
    assert_eq!(
        census.documents_detail.len(),
        1,
        "no detail for a skipped file"
    );
}

#[test]
fn an_unreadable_corpus_root_is_an_error() {
    let config = parse(&config(""));
    let missing = std::env::temp_dir().join(format!(
        "specengine-import-census-absent-{}",
        std::process::id()
    ));
    let error = run(&missing, &config).expect_err("a missing root fails");
    assert!(error.contains("corpus root"), "{error}");
}

#[test]
fn an_empty_corpus_counts_zero() {
    let corpus = Corpus::new("empty");
    fs::create_dir_all(corpus.0.join("design")).unwrap();
    let census = corpus.census(&config(""));
    assert_eq!(census.documents, 0);
    assert!(census.records.is_empty());
    assert!(census.diagnostics.is_empty());
}

// ---------------------------------------------------------- section extent

#[test]
fn a_section_runs_to_the_next_heading_of_the_same_or_higher_level() {
    let section_a = "## Section A {#ZR-501}\n\nBody line one.\n\n### Sub of A\n\nSub body.\n\n\
                     ```text\n# not a heading inside a fence\n```";
    let section_c = "### Sub {#ZR-503 .class key=value}\n\nC body.";
    let section_d = "#### Deep {#ZR-504}\n\nD body.";
    let section_e = "# Last top {#ZR-505}\n\nLast body.";
    let body = format!(
        "---\nkind: rule\n---\n\n# Top\n\n{section_a}\n\n  \n\n## Section B\n\nB body.\n\n\
         {section_c}\n\n### Sibling of C\n\n{section_d}\n\n# Another top {{#no-id-anchor}}\n\n\
         {section_e}\n\n\n\n"
    );
    let corpus = Corpus::new("sections");
    corpus.write("design/sections.md", &body);
    let census = corpus.census(&config(""));
    let got: Vec<(&str, usize, &str)> = census
        .records
        .iter()
        .map(|r| (r.id.as_str(), r.line, r.blake3.as_str()))
        .collect();
    let (ha, hc, hd, he) = (
        hash(section_a),
        hash(section_c),
        hash(section_d),
        hash(section_e),
    );
    assert_eq!(
        got,
        vec![
            ("ZR-501", 7, ha.as_str()),
            ("ZR-503", 25, hc.as_str()),
            ("ZR-504", 31, hd.as_str()),
            ("ZR-505", 37, he.as_str()),
        ]
    );
    assert!(census.records.iter().all(|r| r.kind == RecordKind::Section));
    assert_eq!(census.other_anchors, 1);
}

#[test]
fn section_ids_are_ignored_when_id_attr_is_off() {
    let corpus = Corpus::new("sections-off");
    corpus.write("design/s.md", "## A {#ZR-801}\n\nText.\n");
    let census = corpus.census(&config("").replace("id_attr = true", "id_attr = false"));
    assert!(census.records.is_empty());
}

#[test]
fn row_hash_is_over_the_verbatim_row_and_census_is_deterministic() {
    let corpus = Corpus::new("determinism");
    corpus
        .write(
            "design/b.md",
            format!("{TABLE_HEAD}|  ZR-901  |  spaced  row |\n"),
        )
        .write(
            "design/a.md",
            format!("{TABLE_HEAD}| ZR-902 | x |\n## S {{#ZR-903}}\n\nT.\n"),
        );
    let one = corpus.census(&config(""));
    let two = corpus.census(&config(""));
    assert_eq!(one.records, two.records);
    assert_eq!(one.documents_detail, two.documents_detail);
    let paths: Vec<&str> = one.records.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(
        paths,
        ["design/a.md", "design/a.md", "design/b.md"],
        "sorted by path"
    );
    let spaced = one.records.iter().find(|r| r.id == "ZR-901").unwrap();
    assert_eq!(spaced.blake3, hash("|  ZR-901  |  spaced  row |"));
}

// ------------------------------------------------ pathological Markdown

/// Runs the census over one generated document on a helper thread; fails when
/// it panics or does not finish within `budget`.
fn census_within(
    name: &str,
    file: &str,
    content: String,
    config_text: &str,
    budget: Duration,
) -> Census {
    let corpus = Corpus::new(name);
    corpus.write(file, content);
    let config = parse(config_text);
    let root = corpus.0.clone();
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| run(&root, &config));
        let _ = sender.send(result);
    });
    match receiver.recv_timeout(budget) {
        Ok(Ok(result)) => {
            let census = result.expect("census runs");
            eprintln!("{name}: {} ms", started.elapsed().as_millis());
            census
        }
        Ok(Err(_)) => panic!("{name}: the census panicked"),
        Err(_) => panic!(
            "{name}: the census did not finish within {} s (super-linear?)",
            budget.as_secs()
        ),
    }
}

/// A linear scan of any input below takes well under a second in a debug build.
const BUDGET: Duration = Duration::from_secs(10);

#[test]
fn very_long_lines_finish() {
    let mut content = format!("# {}\n\n", "h".repeat(1 << 20));
    content.push_str(&"word ".repeat(400_000));
    content.push_str("[x](missing.md)\n\n");
    content.push_str(TABLE_HEAD);
    content.push_str(&format!("| ZR-001 | {} |\n", "cell ".repeat(200_000)));
    let census = census_within("long-lines", "design/long.md", content, &config(""), BUDGET);
    assert_eq!(census.broken_links.len(), 1);
    assert_eq!(census.id_rows(), 1);
}

#[test]
fn a_hundred_thousand_table_rows_finish() {
    let mut content = String::from(TABLE_HEAD);
    for index in 0..100_000 {
        content.push_str(&format!("| ZR-{:03} | row {index} |\n", index % 1000));
    }
    let census = census_within("rows", "design/rows.md", content, &config(""), BUDGET);
    assert_eq!(census.id_rows(), 100_000);
    assert_eq!(census.duplicate_ids(), 99_000);
}

#[test]
fn deeply_nested_lists_and_quotes_finish() {
    let mut content = String::new();
    for depth in 0..2_000 {
        content.push_str(&" ".repeat(depth * 2));
        content.push_str("- item [x](missing.md)\n");
    }
    content.push_str(&">".repeat(100_000));
    content.push_str(" quoted\n");
    content.push_str(&"- ".repeat(100_000));
    content.push_str("| ZR-001 | deep |\n");
    let census = census_within("nested", "design/nested.md", content, &config(""), BUDGET);
    assert_eq!(census.broken_links.len(), 2_000);
}

#[test]
fn an_unterminated_fence_hides_the_rest_of_the_document() {
    let mut content = format!("{TABLE_HEAD}| ZR-001 | before the fence |\n\n```rust\n");
    for index in 0..100_000 {
        content.push_str(&format!("| ZR-{:03} | hidden |\n", index % 1000));
    }
    let census = census_within("fence", "design/fence.md", content, &config(""), BUDGET);
    assert_eq!(census.id_rows(), 1);
}

#[test]
fn an_unterminated_html_comment_hides_the_rest_of_the_document() {
    let mut content = format!("{TABLE_HEAD}| ZR-001 | before |\n\n<!--\n");
    for index in 0..100_000 {
        content.push_str(&format!(
            "| ZR-{:03} | hidden [x](missing.md) |\n",
            index % 1000
        ));
    }
    let census = census_within("comment", "design/comment.md", content, &config(""), BUDGET);
    assert_eq!(census.id_rows(), 1);
    assert!(census.broken_links.is_empty());
}

/// `[` × n then `](` × n on one 300 KB line: every `](` may start a link.
#[test]
fn unmatched_link_brackets_on_one_line_finish() {
    let n = 100_000;
    let content = format!("{}{}\n", "[".repeat(n), "](".repeat(n));
    let census = census_within(
        "brackets",
        "design/brackets.md",
        content,
        &config(""),
        BUDGET,
    );
    assert!(census.broken_links.is_empty());
}

/// `[[` × n on one 400 KB line with wiki links on: none is closed.
#[test]
fn unmatched_wiki_brackets_on_one_line_finish() {
    let n = 200_000;
    let content = format!("{}\n", "[[".repeat(n));
    let wiki = config("").replace(
        "[sections]\nid_attr = true\n",
        "[sections]\nid_attr = true\n[links]\nwiki = true\n",
    );
    let census = census_within("wiki-brackets", "design/wiki.md", content, &wiki, BUDGET);
    assert!(census.broken_links.is_empty());
}

/// Backtick runs of every length 1..=k on one ~1 MB line (with an HTML
/// comment, so both the comment stripper and the link scanner read it):
/// no run has a closing run of its length.
#[test]
fn unclosed_backtick_runs_on_one_line_finish() {
    let k = 1_400;
    let mut content = String::from("<!-- x --> ");
    for length in 1..=k {
        content.push_str(&"`".repeat(length));
        content.push('a');
    }
    content.push_str(" [x](missing.md)\n");
    let census = census_within("backticks", "design/ticks.md", content, &config(""), BUDGET);
    assert!(census.broken_links.len() <= 1);
}

/// `[` × n then `](x (` × n on one 600 KB line: every `](` has a destination
/// and opens a `(` title that never closes. The second line closes the last
/// title and the link (`))`), so exactly one link to `x` is found.
#[test]
fn unclosed_paren_titles_on_one_line_finish() {
    let n = 100_000;
    let open = "[".repeat(n);
    let titles = "](x (".repeat(n);
    let content = format!("{open}{titles}\n\n{open}{titles}))\n");
    let census = census_within(
        "paren-titles",
        "design/titles.md",
        content,
        &config(""),
        BUDGET,
    );
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(broken, vec![(3, "x")]);
    assert_eq!(census.links_checked(), 1);
}

/// `[` × n then `](` × n then one `)` on one 300 KB line: no bare destination
/// closes before the last `](`, which is `[…]()` (an empty destination, not
/// checked). The second line ends in `missing.md)`: one broken link.
#[test]
fn a_single_closing_paren_after_many_link_openers_finishes() {
    let n = 100_000;
    let open = "[".repeat(n);
    let openers = "](".repeat(n);
    let content = format!("{open}{openers})\n\n{open}{openers}missing.md)\n");
    let census = census_within(
        "one-paren",
        "design/one-paren.md",
        content,
        &config(""),
        BUDGET,
    );
    let broken: Vec<(usize, &str)> = census
        .broken_links
        .iter()
        .map(|l| (l.line, l.target.as_str()))
        .collect();
    assert_eq!(broken, vec![(3, "missing.md")]);
    assert_eq!(census.links_checked(), 1);
}

/// `[` × n then `](<` × n then one `>` on one 400 KB line: every `<…>`
/// destination closes at the last byte, and none is followed by `)`.
#[test]
fn angle_destinations_closed_once_at_the_end_finish() {
    let n = 100_000;
    let content = format!("{}{}>\n", "[".repeat(n), "](<".repeat(n));
    let census = census_within("angles", "design/angles.md", content, &config(""), BUDGET);
    assert!(census.broken_links.is_empty());
    assert_eq!(census.links_checked(), 0);
}

#[test]
fn fifty_thousand_anchored_headings_finish() {
    let mut content = String::new();
    for index in 0..50_000 {
        content.push_str(&format!(
            "{} H {{#ZR-{:03}}}\n\nText.\n\n",
            "#".repeat(index % 6 + 1),
            index % 1000
        ));
    }
    let census = census_within(
        "headings",
        "design/headings.md",
        content,
        &config(""),
        BUDGET,
    );
    assert_eq!(census.id_sections(), 50_000);
}
