//! Rules of the import engine, `crates/specengine-import/README.md` "Import",
//! outside the fixture ACs of docs/features/import-records.md: the list-item
//! separator (Positions, Text; AC-07), unmapped non-Latin prefixes
//! under a config whose `ids.like` is ASCII-only and hyphenless overlaps
//! (IDs, Hyphenless; AC-05), the unclaimed counter over front-matter values,
//! feature scope and emphasis (Unclaimed; AC-06), and the first definition
//! of a duplicate in (path, line) order (Role).
//!
//! Every corpus is written into a fresh temporary directory by the test; the
//! convention is invented (prefixes `CRT`, `DEC`). Non-Latin characters are
//! Unicode escapes only (ADR-0024, A4).

use std::fs;
use std::path::PathBuf;

use specengine_import::CensusConfig;
use specengine_import::import::{self, HyphenlessRole, Import, TokenCause};

/// A corpus in a temporary directory, removed on drop.
struct Corpus(PathBuf);

impl Corpus {
    fn new(name: &str, document: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-import-rules-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("d")).expect("corpus directory");
        fs::write(path.join("d/a.md"), document).expect("document");
        Self(path)
    }

    /// A corpus of several documents under `d/` (paths relative to it).
    fn with(name: &str, documents: &[(&str, &str)]) -> Self {
        let corpus = Self::new(name, "");
        fs::remove_file(corpus.0.join("d/a.md")).expect("placeholder document");
        for (path, text) in documents {
            let file = corpus.0.join("d").join(path);
            fs::create_dir_all(file.parent().unwrap()).expect("document directory");
            fs::write(file, text).expect("document");
        }
        corpus
    }

    fn import(&self, config: &str) -> Import {
        let config = CensusConfig::parse(config, &self.0.join("census.toml")).expect("config");
        import::run(&self.0, &config).expect("import runs")
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const CONFIG: &str = r#"[corpus]
roots = ["d"]
[ids]
regex = '^[A-Z]{3}-[0-9]{4}$'
like = '[A-Z]{2,5}-[0-9]{2,5}'
[lists]
lead_in = true
separators = [" -", ";"]
"#;

/// Verbatim: "list item → after the span and separator". A configured
/// separator is stripped after the span as written, its leading blank
/// included, just as it is inside the span.
#[test]
fn a_separator_with_a_leading_blank_is_stripped_after_the_span() {
    let corpus = Corpus::new(
        "separator",
        "# A\n\n\
         1. __CRT-0001 -__ Inside the span.\n\
         2. __CRT-0002__ - After the span.\n\
         3. __CRT-0003__; After the span, no blank.\n",
    );
    let found = corpus.import(CONFIG);
    let texts: Vec<(&str, &str)> = found
        .records
        .iter()
        .map(|record| (record.id.as_str(), record.text.as_str()))
        .collect();
    assert_eq!(
        texts,
        [
            ("CRT-0001", "Inside the span."),
            ("CRT-0002", "After the span."),
            ("CRT-0003", "After the span, no blank."),
        ]
    );
    for record in &found.records {
        assert_eq!(
            record.hash,
            blake3::hash(record.text.as_bytes()).to_hex().to_string(),
            "{}: the hash of the text without the separator",
            record.id
        );
    }
}

/// IDs: "A non-Latin prefix that does neither SHALL count `legacy.unmapped`:
/// no record, no guessed ID" — at every record position, whatever
/// `ids.like` says ID-like text is.
#[test]
fn an_unmapped_non_latin_prefix_is_counted_whatever_ids_like_is() {
    // Pe, Er, Be: a Cyrillic prefix, the first letter without a look-alike.
    let prefix = "\u{041F}\u{0420}\u{0411}";
    // Te, Ie, Es: Cyrillic, every letter a Latin look-alike (`TEC` would
    // pass `ids.regex`).
    let look = "\u{0422}\u{0415}\u{0421}";
    let document = format!(
        "# A\n\n| ID | Text |\n|---|---|\n| DEC-0001 | A Latin row. |\n\
         | {prefix}-0002 | A row under an unmapped prefix. |\n\
         | {look}-0005 | A row under look-alikes only. |\n\n\
         - __{prefix}-0003;__ A list item under it.\n\n\
         ## Section {{#{prefix}-0004}}\n\nBody.\n"
    );
    let corpus = Corpus::new("unmapped", &document);
    for (config, like) in [
        (
            CONFIG.replace("like = '[A-Z]{2,5}-[0-9]{2,5}'\n", ""),
            "the default",
        ),
        (CONFIG.to_owned(), "an ASCII-only"),
    ] {
        let found = corpus.import(&config);
        let ids: Vec<&str> = found
            .records
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(ids, ["DEC-0001"], "{like} ids.like: no guessed record");
        let mut unmapped: Vec<&str> = found
            .legacy
            .unmapped
            .iter()
            .map(|token| token.token.as_str())
            .collect();
        unmapped.sort_unstable();
        let mut want = [
            format!("{prefix}-0002"),
            format!("{prefix}-0003"),
            format!("{prefix}-0004"),
            format!("{look}-0005"),
        ];
        want.sort_unstable();
        assert_eq!(
            unmapped,
            want,
            "{like} ids.like: the table rows, the list item and the section count \
             legacy.unmapped; rows without ID {:?}",
            found
                .rows_without_id
                .iter()
                .map(|row| row.cell.as_str())
                .collect::<Vec<_>>()
        );
        assert!(found.rows_without_id.is_empty(), "{like} ids.like");
    }
}

/// The config of the AC-06 cases: `CRT` feature-scoped, two hyphenless
/// patterns, a field table header, section anchors.
const SCOPED: &str = r#"[corpus]
roots = ["d"]
[front_matter]
header_table = '^Field$'
[ids]
regex = '^[A-Z]{3}-[0-9]{4}$'
like = '[A-Z]{2,5}-[0-9]{2,5}'
feature_prefixes = ["CRT"]
hyphenless = ['\bP[0-9]{4}\b', '\bG[0-9]{2}\b']
[sections]
id_attr = true
[lists]
lead_in = true
separators = [" -", ";"]
"#;

/// The definitions every AC-06 corpus starts from.
const REGISTER: (&str, &str) = (
    "register.md",
    "# Register\n\n| ID | Text |\n|---|---|\n| DEC-0001 | One. |\n| DEC-0002 | Two. |\n",
);

/// `(path, line, token, cause)` of every token that is not claimed.
fn not_claimed(found: &Import) -> Vec<(String, usize, String, TokenCause)> {
    found
        .unclaimed
        .iter()
        .map(|token| {
            (
                token.path.clone(),
                token.line,
                token.token.clone(),
                token.cause,
            )
        })
        .collect()
}

/// Unclaimed: front-matter values are scanned, once — a YAML scalar and a
/// list item citing a defined ID are claimed, an undefined one unclaimed;
/// a YAML comment (a line of its own or after a value) and a key are not
/// scanned; a field table's cells are scanned once.
#[test]
fn front_matter_values_are_scanned_once_and_yaml_comments_are_not() {
    let plain = "# Page\n\nBody.\n";
    let yaml = "---\n\
                type: page\n\
                depends: DEC-0001 # not DEC-0007\n\
                links:\n  - DEC-0002\n  - DEC-0404\n\
                # DEC-0405 is a YAML comment\n\
                DEC-0406: a key\n\
                ---\n# Page\n\nBody.\n";
    let field_table = "| Field | Value |\n|---|---|\n| Depends | DEC-0001 |\n\
                       | Blocks | DEC-0505 |\n\n# Page\n\nBody.\n";
    let base = Corpus::with("fm-base", &[REGISTER, ("page.md", plain)]).import(SCOPED);
    assert_eq!(base.claimed, 0);
    assert!(base.unclaimed.is_empty(), "{:?}", not_claimed(&base));

    let found = Corpus::with("fm-yaml", &[REGISTER, ("page.md", yaml)]).import(SCOPED);
    assert_eq!(found.claimed, 2, "the scalar and the list item");
    assert_eq!(
        not_claimed(&found),
        [(
            "d/page.md".to_owned(),
            6,
            "DEC-0404".to_owned(),
            TokenCause::Unclaimed
        )]
    );
    assert_eq!(found.files_with_unclaimed(), 1);

    let found = Corpus::with("fm-table", &[REGISTER, ("page.md", field_table)]).import(SCOPED);
    assert_eq!(found.claimed, 1, "a field-table value is scanned once");
    assert_eq!(
        not_claimed(&found),
        [(
            "d/page.md".to_owned(),
            4,
            "DEC-0505".to_owned(),
            TokenCause::Unclaimed
        )]
    );
}

/// Unclaimed: a feature-scoped token is claimed only in a document defining
/// it; defined only elsewhere it is `feature-outside`, neither claimed nor
/// unclaimed, and a document holding only such tokens is no file with
/// unclaimed tokens; one defined nowhere is unclaimed.
#[test]
fn a_feature_scoped_id_is_claimed_only_in_a_document_defining_it() {
    let found = Corpus::with(
        "feature",
        &[
            (
                "a.md",
                "# A\n\n1. __CRT-0001;__ A criterion.\n\nA cites CRT-0001 in its own document.\n",
            ),
            (
                "b.md",
                "# B\n\nB cites CRT-0001 from outside; CRT-0009 is defined nowhere.\n",
            ),
            ("c.md", "# C\n\nC cites CRT-0001 only.\n"),
        ],
    )
    .import(SCOPED);
    assert_eq!(found.records.len(), 1);
    assert_eq!(found.claimed, 1, "the citation in a.md");
    assert_eq!(
        not_claimed(&found),
        [
            (
                "d/b.md".to_owned(),
                3,
                "CRT-0001".to_owned(),
                TokenCause::FeatureOutside
            ),
            (
                "d/b.md".to_owned(),
                3,
                "CRT-0009".to_owned(),
                TokenCause::Unclaimed
            ),
            (
                "d/c.md".to_owned(),
                3,
                "CRT-0001".to_owned(),
                TokenCause::FeatureOutside
            ),
        ]
    );
    assert_eq!(found.unclaimed(TokenCause::FeatureOutside), 2);
    assert_eq!(found.unclaimed(TokenCause::Unclaimed), 1);
    assert_eq!(found.files_with_unclaimed(), 1, "b.md only");
}

/// Unclaimed: `like` and `hyphenless` read emphasis delimiter runs as
/// blanks — without `[lists]` an `__ID__` item is an unclaimed token, and a
/// `\b`-bounded hyphenless pattern matches inside `__…__`; an intraword `_`
/// is no delimiter, nor a backslash-escaped one or one in a code span.
#[test]
fn emphasis_delimiters_read_as_blanks_for_like_and_hyphenless() {
    let document = "# E\n\n\
                    1. __DEC-0101__ An item opening underscore-strong.\n\
                    2. _DEC-0102_ underscore emphasis.\n\
                    3. **DEC-0103** star-strong.\n\n\
                    Neither snake_DEC-0104 nor \\_DEC-0105\\_ nor `__DEC-0106__` is a token.\n\n\
                    The codes __P1234__ and *G12* are hyphenless; snake_P5678 is not.\n";
    let without_lists = SCOPED.replace(
        "[lists]\nlead_in = true\nseparators = [\" -\", \";\"]\n",
        "",
    );
    assert_ne!(without_lists, SCOPED);
    let found = Corpus::with("emphasis", &[("e.md", document)]).import(&without_lists);
    assert!(found.records.is_empty());
    let tokens: Vec<&str> = found
        .unclaimed
        .iter()
        .map(|token| token.token.as_str())
        .collect();
    assert_eq!(tokens, ["DEC-0101", "DEC-0102", "DEC-0103"]);
    let hyphenless: Vec<(&str, usize)> = found
        .legacy
        .hyphenless
        .iter()
        .map(|found| (found.token.as_str(), found.pattern))
        .collect();
    assert_eq!(hyphenless, [("P1234", 0), ("G12", 1)]);
}

/// Hyphenless: "a token range counts once, for the first matching pattern
/// in config order" — a code two or three patterns match is one match of
/// the first; a code only a later pattern matches is that pattern's.
#[test]
fn a_hyphenless_code_several_patterns_match_counts_once_under_the_first() {
    let like = "like = '[A-Z]{2,5}-[0-9]{2,5}'\n";
    assert_eq!(CONFIG.matches(like).count(), 1);
    let config = CONFIG.replace(
        like,
        &format!(
            "{like}hyphenless = ['\\b[A-Z]{{2}}[0-9]{{3}}\\b', '\\b[A-Z][A-Z0-9]{{4}}\\b', '[0-9]{{3}}\\b']\n"
        ),
    );
    let found = Corpus::new(
        "hyphenless",
        "# H\n\n**QP201:** Defined at a paragraph start.\n\nA mention of QP202 and of Z9A99.\n",
    )
    .import(&config);
    let matches: Vec<(&str, usize, HyphenlessRole)> = found
        .legacy
        .hyphenless
        .iter()
        .map(|found| (found.token.as_str(), found.pattern, found.role))
        .collect();
    assert_eq!(
        matches,
        [
            ("QP201", 0, HyphenlessRole::Definition),
            ("QP202", 0, HyphenlessRole::Mention),
            ("Z9A99", 1, HyphenlessRole::Mention),
        ]
    );
    assert_eq!(found.per_pattern(3), [2, 1, 0]);
    assert!(found.records.is_empty() && found.unclaimed.is_empty());
}

/// Duplicates: "a definition of an ID defined earlier in (path, line)
/// order" — the first is the byte-wise smaller path (`a-b.md` before
/// `a/x.md`), and within a document the smaller line whatever the form
/// (a section anchor above a table row).
#[test]
fn the_first_definition_is_first_in_path_then_line_order() {
    let table = |id: &str| format!("# T\n\n| ID | Text |\n|---|---|\n| {id} | A row. |\n");
    let found = Corpus::with(
        "duplicates",
        &[
            ("a/x.md", table("DEC-0001").as_str()),
            ("a-b.md", table("DEC-0001").as_str()),
            (
                "c.md",
                "# C\n\n## Above {#DEC-0002}\n\nBody.\n\n| ID | Text |\n|---|---|\n| DEC-0002 | Below. |\n",
            ),
        ],
    )
    .import(SCOPED);
    let records: Vec<(&str, usize, &str)> = found
        .records
        .iter()
        .map(|record| (record.path.as_str(), record.line, record.id.as_str()))
        .collect();
    assert_eq!(
        records,
        [
            ("d/a-b.md", 5, "DEC-0001"),
            ("d/a/x.md", 5, "DEC-0001"),
            ("d/c.md", 3, "DEC-0002"),
            ("d/c.md", 9, "DEC-0002"),
        ]
    );
    let duplicates: Vec<(&str, usize, &str, &str, usize)> = found
        .duplicates
        .iter()
        .map(|duplicate| {
            (
                duplicate.path.as_str(),
                duplicate.line,
                duplicate.id.as_str(),
                duplicate.first_path.as_str(),
                duplicate.first_line,
            )
        })
        .collect();
    assert_eq!(
        duplicates,
        [
            ("d/a/x.md", 5, "DEC-0001", "d/a-b.md", 5),
            ("d/c.md", 9, "DEC-0002", "d/c.md", 3),
        ]
    );
}

/// Unclaimed over front matter, YAML comments: a quote opens a quoted
/// scalar only where the scalar or a flow item starts, so the apostrophe of
/// `don't` leaves the `# …` after it a comment (not scanned), while a `#`
/// inside a quoted scalar — after `''` in single quotes, after `\"` in
/// double quotes, inside a quoted flow item — is value text (scanned).
#[test]
fn a_yaml_comment_after_an_apostrophe_is_not_scanned_and_a_quoted_hash_is() {
    let yaml = "---\n\
                note: don't DEC-0001 # DEC-0408\n\
                title: 'it''s # DEC-0409'\n\
                quote: \"say \\\" # DEC-0410\"\n\
                tags: [one, 'two # DEC-0411', three] # DEC-0412\n\
                links:\n  - don't DEC-0002 # DEC-0413\n  - 'it''s # DEC-0414'\n\
                ---\n# Page\n\nBody.\n";
    let found = Corpus::with("fm-quotes", &[REGISTER, ("page.md", yaml)]).import(SCOPED);
    assert_eq!(
        found.claimed, 2,
        "DEC-0001 and DEC-0002 before the comments"
    );
    let unclaimed = |line, token: &str| {
        (
            "d/page.md".to_owned(),
            line,
            token.to_owned(),
            TokenCause::Unclaimed,
        )
    };
    let mut tokens = not_claimed(&found);
    tokens.sort_by(|a, b| (a.1, &a.2).cmp(&(b.1, &b.2)));
    assert_eq!(
        tokens,
        [
            unclaimed(3, "DEC-0409"),
            unclaimed(4, "DEC-0410"),
            unclaimed(5, "DEC-0411"),
            unclaimed(8, "DEC-0414"),
        ]
    );
    assert_eq!(found.files_with_unclaimed(), 1);
}

/// Unclaimed over front matter, a tag (`!…`) or an anchor (`&…`) before a
/// quoted scalar: the quote still opens the scalar, so a `# …` inside the
/// quotes is value text (scanned) and the comment after the closing quote
/// is not — for a scalar, a sequence item and a flow item alike. After a
/// tag a plain scalar's apostrophe opens nothing: the `# …` after it stays
/// a comment.
#[test]
fn a_quoted_scalar_after_a_tag_or_an_anchor_is_scanned_and_its_comment_is_not() {
    let yaml = "---\n\
                t: !!str 'x # DEC-0420' # DEC-0421\n\
                r: &anc \"y # DEC-0422\" # DEC-0423\n\
                s: !!str DEC-0001 # DEC-0424\n\
                links:\n  - !!str 'z # DEC-0425' # DEC-0426\n  - &item \"w # DEC-0427\" # DEC-0428\n\
                tags: [!!str 'v # DEC-0429', &a \"u # DEC-0430\"] # DEC-0431\n\
                note: !!str don't DEC-0002 # DEC-0432\n\
                ---\n# Page\n\nBody.\n";
    let found = Corpus::with("fm-tags", &[REGISTER, ("page.md", yaml)]).import(SCOPED);
    assert_eq!(
        found.claimed, 2,
        "DEC-0001 and DEC-0002 before the comments"
    );
    let unclaimed = |line, token: &str| {
        (
            "d/page.md".to_owned(),
            line,
            token.to_owned(),
            TokenCause::Unclaimed,
        )
    };
    let mut tokens = not_claimed(&found);
    tokens.sort_by(|a, b| (a.1, &a.2).cmp(&(b.1, &b.2)));
    assert_eq!(
        tokens,
        [
            unclaimed(2, "DEC-0420"),
            unclaimed(3, "DEC-0422"),
            unclaimed(6, "DEC-0425"),
            unclaimed(7, "DEC-0427"),
            unclaimed(8, "DEC-0429"),
            unclaimed(8, "DEC-0430"),
        ]
    );
    assert_eq!(found.files_with_unclaimed(), 1);
}
