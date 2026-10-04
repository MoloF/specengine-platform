//! Verbatim text and hashes of the import record model
//! (`docs/canon/import.md` "Text and hash"; AC-07 of
//! docs/features/import-records.md): per form — table row, headerless
//! row, list item, section — one inner change gives another hash, the
//! document with CRLF gives equal hashes, a cell's surrounding blanks give
//! equal hashes, and a hash is BLAKE3 of the text as written. A list item's
//! text is its raw first line (HTML comments kept), deeper lines and lazy
//! lines, up to a heading, fence or item, and never a nested record item; a
//! `text_header` matching the ID column's header never yields the ID cell.
//!
//! Every corpus is written into a fresh temporary directory by the test;
//! the convention is invented (prefix `HT`).

use std::fs;
use std::path::PathBuf;

use specengine_import::CensusConfig;
use specengine_import::import::{self, Form, ImportRecord};

const CONFIG: &str = r#"[corpus]
roots = ["spec"]
[ids]
regex = '^[A-Z]{2}-[0-9]{3}$'
[tables]
headerless = true
[lists]
lead_in = true
separators = [":"]
"#;

/// The document every variant starts from: one record of each form, and a
/// table row whose text holds escapes.
const BASE: &str = "# Hashes

| ID | Text | Note |
|---|---|---|
| HT-001 | One cell of text. | n |
| HT-002 | stop \\| pause<br>now | n |

| HT-003 | A headerless row. | n |

- **HT-004:** A list item
  continued on a second line.

## Section {#HT-005}

Section body.
";

/// The verbatim text of each record of [`BASE`].
const TEXTS: [(&str, Form, &str); 5] = [
    ("HT-001", Form::TableRow, "One cell of text."),
    ("HT-002", Form::TableRow, "stop \\| pause<br>now"),
    ("HT-003", Form::HeaderlessRow, "A headerless row."),
    (
        "HT-004",
        Form::ListItem,
        "A list item\n  continued on a second line.",
    ),
    (
        "HT-005",
        Form::Section,
        "## Section {#HT-005}\n\nSection body.",
    ),
];

/// A corpus in a temporary directory, removed on drop.
struct Corpus(PathBuf);

impl Corpus {
    fn new(name: &str, document: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-import-hash-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("spec")).expect("corpus directory");
        fs::write(path.join("spec/doc.md"), document).expect("document");
        Self(path)
    }

    fn records(&self) -> Vec<ImportRecord> {
        self.records_with(CONFIG)
    }

    fn records_with(&self, config: &str) -> Vec<ImportRecord> {
        let config = CensusConfig::parse(config, &self.0.join("census.toml")).expect("config");
        import::run(&self.0, &config).expect("import runs").records
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `(id, hash)` of every record of `document`, in record order.
fn hashes(name: &str, document: &str) -> Vec<(String, String)> {
    Corpus::new(name, document)
        .records()
        .into_iter()
        .map(|record| (record.id, record.hash))
        .collect()
}

fn blake3_hex(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

#[test]
fn each_form_keeps_its_text_as_written_and_hashes_it() {
    let records = Corpus::new("base", BASE).records();
    let found: Vec<(&str, Form, &str)> = records
        .iter()
        .map(|record| (record.id.as_str(), record.form, record.text.as_str()))
        .collect();
    assert_eq!(found, TEXTS);
    for record in &records {
        assert_eq!(
            record.hash,
            blake3_hex(&record.text),
            "{}: the hash is BLAKE3 of the text",
            record.id
        );
    }
}

#[test]
fn a_cell_hash_is_blake3_of_the_trimmed_cell() {
    let records = Corpus::new("cell", BASE).records();
    for (id, cell) in [
        ("HT-001", "One cell of text."),
        ("HT-002", "stop \\| pause<br>now"),
        ("HT-003", "A headerless row."),
    ] {
        let record = records.iter().find(|record| record.id == id).expect(id);
        assert_eq!(record.hash, blake3_hex(cell), "{id}");
    }
}

#[test]
fn the_document_with_crlf_gives_equal_hashes() {
    let lf = hashes("lf", BASE);
    let crlf = hashes("crlf", &BASE.replace('\n', "\r\n"));
    assert_eq!(lf.len(), 5);
    assert_eq!(crlf, lf, "CRLF changes no hash");
    // A comment on the first line, lazy lines, a nested record alike.
    for (name, document) in [("comments", COMMENTS), ("lazy", LAZY), ("nested", NESTED)] {
        let lf = hashes(&format!("lf-{name}"), document);
        let crlf = hashes(&format!("crlf-{name}"), &document.replace('\n', "\r\n"));
        assert!(!lf.is_empty(), "{name}");
        assert_eq!(crlf, lf, "{name}: CRLF changes no hash");
    }
}

#[test]
fn surrounding_blanks_of_a_cell_give_equal_hashes() {
    let base = hashes("blanks-base", BASE);
    let padded = BASE
        .replace(
            "| HT-001 | One cell of text. |",
            "| HT-001 |    One cell of text.\t  |",
        )
        .replace(
            "| HT-003 | A headerless row. |",
            "|HT-003|\tA headerless row.   |",
        );
    assert_ne!(padded, BASE);
    assert_eq!(hashes("blanks", &padded), base);
}

/// One inner change per variant: only the changed record's hash moves.
#[test]
fn one_inner_change_gives_another_hash_for_that_record_only() {
    let base = hashes("inner-base", BASE);
    let variants = [
        ("HT-001", "One cell of text.", "One cell  of text."),
        ("HT-002", "stop \\| pause", "stop &#124; pause"),
        ("HT-002", "pause<br>now", "pause<br/>now"),
        ("HT-002", "stop \\| pause", "stop \\|pause"),
        ("HT-003", "A headerless row.", "A  headerless row."),
        ("HT-004", "A list item", "A list  item"),
        ("HT-004", "  continued on", "  continued  on"),
        ("HT-005", "Section body.", "Section  body."),
    ];
    for (index, (id, from, to)) in variants.into_iter().enumerate() {
        assert_eq!(BASE.matches(from).count(), 1, "{from:?} occurs once");
        let changed = hashes(&format!("inner-{index}"), &BASE.replace(from, to));
        assert_eq!(changed.len(), base.len(), "{id}: {to:?} keeps every record");
        for ((base_id, base_hash), (changed_id, changed_hash)) in base.iter().zip(&changed) {
            assert_eq!(base_id, changed_id);
            if base_id == id {
                assert_ne!(
                    base_hash, changed_hash,
                    "{id}: {from:?} -> {to:?} changes its hash"
                );
            } else {
                assert_eq!(
                    base_hash, changed_hash,
                    "{base_id}: changing {id} must not move its hash"
                );
            }
        }
    }
}

/// `(id, text)` of every record of `document`, in record order.
fn texts(name: &str, document: &str) -> Vec<(String, String)> {
    Corpus::new(name, document)
        .records()
        .into_iter()
        .map(|record| (record.id, record.text))
        .collect()
}

/// List items whose first line holds an HTML comment: after the text, right
/// after the lead-in, and one that opens there and closes on a deeper line.
const COMMENTS: &str = "# Comments

- **HT-010:** First line <!-- a note --> goes on.
- **HT-011:** <!-- a lead note --> Text after it.
- **HT-012:** Text <!-- opens here
  and closes --> at the end.
";

/// Verbatim: "HTML comments kept (detection may use the comment-stripped
/// line)" — on the item's first line too, and editing the comment moves
/// the hash.
#[test]
fn a_comment_on_an_items_first_line_stays_in_its_text() {
    assert_eq!(
        texts("comments", COMMENTS),
        [
            (
                "HT-010".to_owned(),
                "First line <!-- a note --> goes on.".to_owned()
            ),
            (
                "HT-011".to_owned(),
                "<!-- a lead note --> Text after it.".to_owned()
            ),
            (
                "HT-012".to_owned(),
                "Text <!-- opens here\n  and closes --> at the end.".to_owned()
            ),
        ]
    );
    let base = hashes("comments-base", COMMENTS);
    for (index, (id, from, to)) in [
        ("HT-010", "<!-- a note -->", "<!-- another note -->"),
        ("HT-011", "<!-- a lead note -->", "<!-- a lead  note -->"),
        ("HT-012", "<!-- opens here", "<!-- opens  here"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(COMMENTS.matches(from).count(), 1, "{from:?}");
        let changed = hashes(&format!("comments-{index}"), &COMMENTS.replace(from, to));
        for ((base_id, base_hash), (changed_id, changed_hash)) in base.iter().zip(&changed) {
            assert_eq!(base_id, changed_id);
            if base_id == id {
                assert_ne!(base_hash, changed_hash, "{id}: editing {from:?}");
            } else {
                assert_eq!(base_hash, changed_hash, "{base_id}: editing {id}");
            }
        }
    }
}

/// Lazy lines: an unindented line directly continuing the item's
/// paragraph is in its text verbatim; a heading, a fence or an item after
/// it is not.
const LAZY: &str = "# Lazy

- **HT-020:** Item text
a lazy line.
## A heading after it

- **HT-021:** Item text
a lazy line.
```text
fenced
```

- **HT-022:** Item text
a lazy line.
- Another item.

- **HT-023:** Item text
  an indented line
a lazy line after it.

- **HT-024:** Item text
## A heading right after the item
";

#[test]
fn a_lazy_line_is_in_the_text_and_what_opens_a_block_after_it_is_not() {
    let lazy = "Item text\na lazy line.";
    assert_eq!(
        texts("lazy", LAZY),
        [
            ("HT-020".to_owned(), lazy.to_owned()),
            ("HT-021".to_owned(), lazy.to_owned()),
            ("HT-022".to_owned(), lazy.to_owned()),
            (
                "HT-023".to_owned(),
                "Item text\n  an indented line\na lazy line after it.".to_owned()
            ),
            ("HT-024".to_owned(), "Item text".to_owned()),
        ]
    );
    let records = Corpus::new("lazy-hash", LAZY).records();
    for record in &records {
        assert_eq!(record.hash, blake3_hex(&record.text), "{}", record.id);
    }
    // Editing the lazy line moves that record's hash only.
    let edited = LAZY.replacen(
        "Item text\na lazy line.\n## A",
        "Item text\na lazy  line.\n## A",
        1,
    );
    assert_ne!(edited, LAZY);
    let base = hashes("lazy-base", LAZY);
    let changed = hashes("lazy-edited", &edited);
    assert_ne!(base[0], changed[0], "HT-020: its lazy line edited");
    assert_eq!(base[1..], changed[1..]);
}

/// Verbatim: a list item ends at "a nested record item"; the nested lines
/// before it are in the parent's text, the nested record has its own.
const NESTED: &str = "# Nested

- **HT-030:** Parent text
  continued.
  - A nested plain item.
  - **HT-031:** A nested record.
    Its own continuation.
- **HT-032:** The next item.
";

#[test]
fn a_nested_record_item_is_not_in_its_parents_text() {
    assert_eq!(
        texts("nested", NESTED),
        [
            (
                "HT-030".to_owned(),
                "Parent text\n  continued.\n  - A nested plain item.".to_owned()
            ),
            (
                "HT-031".to_owned(),
                "A nested record.\n    Its own continuation.".to_owned()
            ),
            ("HT-032".to_owned(), "The next item.".to_owned()),
        ]
    );
    // Editing the nested record leaves the parent's hash alone.
    let base = hashes("nested-base", NESTED);
    let changed = hashes(
        "nested-edited",
        &NESTED.replace("A nested record.", "A nested  record."),
    );
    assert_eq!(
        base[0], changed[0],
        "HT-030 must not hash its nested record"
    );
    assert_ne!(base[1], changed[1], "HT-031");
}

/// Verbatim: the mapped cell is "the first non-ID cell whose header matches
/// `text_header`, else `text_column`" — a `text_header` that matches the
/// ID column's header never yields the ID cell.
#[test]
fn a_text_header_matching_the_id_header_never_yields_the_id_cell() {
    let document = "# Table

| ID | Note | Text |
|---|---|---|
| HT-040 | the note | the text |
";
    let corpus = Corpus::new("text-header", document);
    for (text_header, want) in [
        ("^(ID|Text)$", "the text"),
        ("^ID$", "the note"),
        ("^I", "the note"),
        (".", "the note"),
    ] {
        let anchor = "[tables]\nheaderless = true\n";
        assert_eq!(CONFIG.matches(anchor).count(), 1);
        let config = CONFIG.replace(anchor, &format!("{anchor}text_header = '{text_header}'\n"));
        let records = corpus.records_with(&config);
        assert_eq!(records.len(), 1, "{text_header}: {config}");
        assert_eq!(records[0].text, want, "text_header {text_header:?}");
        assert_eq!(records[0].hash, blake3_hex(want));
    }
}

/// Asserts that replacing the one occurrence of `from` in `document` by `to`
/// moves the hash of `id` and of no other record.
fn edit_moves_only(name: &str, document: &str, id: &str, from: &str, to: &str) {
    assert_eq!(document.matches(from).count(), 1, "{from:?}");
    let base = hashes(&format!("{name}-base"), document);
    let changed = hashes(&format!("{name}-edited"), &document.replace(from, to));
    assert_eq!(base.len(), changed.len());
    for ((base_id, base_hash), (changed_id, changed_hash)) in base.iter().zip(&changed) {
        assert_eq!(base_id, changed_id);
        if base_id == id {
            assert_ne!(base_hash, changed_hash, "{id}: editing {from:?}");
        } else {
            assert_eq!(
                base_hash, changed_hash,
                "{base_id}: editing {from:?} of {id}"
            );
        }
    }
}

/// Sibling record items nested four spaces or one tab deep (under a plain
/// parent item): block syntax is read past the container's indentation, so
/// each sibling is a record of its own and never a line of the previous
/// item's text.
const SIBLINGS: &str = "# Siblings

- A plain parent item.
    - **HT-101:** one
    - **HT-102:** two

- Another plain parent item.
\t- **HT-103:** tab one
\t- **HT-104:** tab two
";

#[test]
fn sibling_record_items_nested_four_spaces_or_a_tab_deep_are_records_of_their_own() {
    assert_eq!(
        texts("siblings", SIBLINGS),
        [
            ("HT-101".to_owned(), "one".to_owned()),
            ("HT-102".to_owned(), "two".to_owned()),
            ("HT-103".to_owned(), "tab one".to_owned()),
            ("HT-104".to_owned(), "tab two".to_owned()),
        ]
    );
    let records = Corpus::new("siblings-forms", SIBLINGS).records();
    for record in &records {
        assert_eq!(record.form, Form::ListItem, "{}", record.id);
        assert_eq!(record.hash, blake3_hex(&record.text), "{}", record.id);
    }
    edit_moves_only("siblings-space", SIBLINGS, "HT-102", "** two", "** two!");
    edit_moves_only("siblings-tab", SIBLINGS, "HT-104", "tab two", "tab  two");
}

/// An item whose marker sits at four columns ends at a heading, a fence, a
/// thematic break, an HTML block, a block quote or a table row at its own
/// indentation, just as an item at column 0 would; a lazy line after it is
/// still its text.
const BLOCKS_AT_FOUR: &str = "# Blocks after an item at four columns

- Parent.
    - **HT-111:** heading next
    # A heading

- Parent.
    - **HT-112:** fence next
    ```text
    fenced
    ```

- Parent.
    - **HT-113:** break next
    ***

- Parent.
    - **HT-114:** html next
    <div>block</div>

- Parent.
    - **HT-115:** quote next
    > quoted

- Parent.
    - **HT-116:** row next
    | a | b |

- Parent.
    - **HT-117:** lazy next
a lazy line.
";

#[test]
fn a_block_after_an_item_at_four_columns_ends_it() {
    assert_eq!(
        texts("blocks-at-four", BLOCKS_AT_FOUR),
        [
            ("HT-111".to_owned(), "heading next".to_owned()),
            ("HT-112".to_owned(), "fence next".to_owned()),
            ("HT-113".to_owned(), "break next".to_owned()),
            ("HT-114".to_owned(), "html next".to_owned()),
            ("HT-115".to_owned(), "quote next".to_owned()),
            ("HT-116".to_owned(), "row next".to_owned()),
            ("HT-117".to_owned(), "lazy next\na lazy line.".to_owned()),
        ]
    );
    // The same blocks indented by a tab instead of four spaces.
    let tabbed = BLOCKS_AT_FOUR.replace("\n    ", "\n\t");
    assert_ne!(tabbed, BLOCKS_AT_FOUR);
    assert_eq!(
        texts("blocks-at-tab", &tabbed),
        texts("blocks-at-four-again", BLOCKS_AT_FOUR)
    );
}

/// A comment the item's first line opens carries its text over lines that
/// are blank once the comment is removed: all three lines are the item's
/// text, and editing the middle one moves its hash. A comment opening a
/// line of its own at column 0 is an HTML block and ends the item.
const COMMENT_CONTINUATION: &str = "# Comment continuation

- **HT-121:** alpha <!-- start
middle only
end --> tail

- **HT-122:** item text
<!-- a comment of its own -->
after it

- **HT-123:** the next item.
";

#[test]
fn a_comment_the_item_opens_continues_it_and_one_at_column_0_ends_it() {
    assert_eq!(
        texts("comment-continuation", COMMENT_CONTINUATION),
        [
            (
                "HT-121".to_owned(),
                "alpha <!-- start\nmiddle only\nend --> tail".to_owned()
            ),
            ("HT-122".to_owned(), "item text".to_owned()),
            ("HT-123".to_owned(), "the next item.".to_owned()),
        ]
    );
    edit_moves_only(
        "comment-middle",
        COMMENT_CONTINUATION,
        "HT-121",
        "middle only",
        "middle  only",
    );
    edit_moves_only(
        "comment-own-line",
        COMMENT_CONTINUATION,
        "HT-122",
        "item text",
        "item  text",
    );
    // The column-0 comment is no text of HT-122: editing it moves nothing.
    let base = hashes("comment-col0-base", COMMENT_CONTINUATION);
    let edited = COMMENT_CONTINUATION.replace("of its own", "of  its own");
    assert_ne!(edited, COMMENT_CONTINUATION);
    assert_eq!(base, hashes("comment-col0-edited", &edited));
}

/// Fenced code in a nested record item (two spaces, four spaces or a tab
/// deep): the fence opens on a line past the marker's columns, and its
/// `# x` / `## x` code lines are code, not headings. The item's text keeps
/// the whole block (a blank line inside it too) and the paragraph after it;
/// a column-0 line right after the closing fence is no lazy line (no
/// paragraph to continue), so it is no text of the item.
const NESTED_FENCES: &str = "# Fences in nested record items

- Parent, two spaces.
  - **HT-201:** two-space nested
    ```sh
    # x

    ## x
    ```
    then build.

- Parent, four spaces.
    - **HT-202:** four-space nested
      ```sh
      # x
      ## x
      ```
      then build.

- Parent, a tab.
\t- **HT-203:** tab nested
\t  ```sh
\t  # x
\t  ## x
\t  ```
\t  then build.

- Parent, two spaces.
  - **HT-204:** two-space lazy
    ```sh
    # x
    ## x
    ```
lazy after fence 204

- Parent, four spaces.
    - **HT-205:** four-space lazy
      ```sh
      # x
      ```
lazy after fence 205

- Parent, a tab.
\t- **HT-206:** tab lazy
\t  ```sh
\t  ## x
\t  ```
lazy after fence 206
";

#[test]
fn a_fence_in_a_nested_record_item_holds_code_lines_and_no_lazy_line_follows_it() {
    assert_eq!(
        texts("nested-fences", NESTED_FENCES),
        [
            (
                "HT-201".to_owned(),
                "two-space nested\n    ```sh\n    # x\n\n    ## x\n    ```\n    then build."
                    .to_owned()
            ),
            (
                "HT-202".to_owned(),
                "four-space nested\n      ```sh\n      # x\n      ## x\n      ```\n      then build."
                    .to_owned()
            ),
            (
                "HT-203".to_owned(),
                "tab nested\n\t  ```sh\n\t  # x\n\t  ## x\n\t  ```\n\t  then build.".to_owned()
            ),
            (
                "HT-204".to_owned(),
                "two-space lazy\n    ```sh\n    # x\n    ## x\n    ```".to_owned()
            ),
            (
                "HT-205".to_owned(),
                "four-space lazy\n      ```sh\n      # x\n      ```".to_owned()
            ),
            (
                "HT-206".to_owned(),
                "tab lazy\n\t  ```sh\n\t  ## x\n\t  ```".to_owned()
            ),
        ]
    );
    let records = Corpus::new("nested-fences-forms", NESTED_FENCES).records();
    for record in &records {
        assert_eq!(record.form, Form::ListItem, "{}", record.id);
        assert_eq!(record.hash, blake3_hex(&record.text), "{}", record.id);
    }
    // A code line inside the fence is that record's text, hashed.
    edit_moves_only(
        "nested-fences-code",
        NESTED_FENCES,
        "HT-202",
        "four-space nested\n      ```sh\n      # x\n",
        "four-space nested\n      ```sh\n      # y\n",
    );
    edit_moves_only(
        "nested-fences-tab-code",
        NESTED_FENCES,
        "HT-203",
        "\t  ## x\n\t  ```\n\t  then",
        "\t  ##  x\n\t  ```\n\t  then",
    );
    // The column-0 lines after the closing fences are no record's text.
    let base = hashes("nested-fences-lazy-base", NESTED_FENCES);
    for number in ["204", "205", "206"] {
        let from = format!("lazy after fence {number}");
        let edited = NESTED_FENCES.replace(&from, &format!("lazy  after fence {number}"));
        assert_ne!(edited, NESTED_FENCES);
        assert_eq!(
            base,
            hashes(&format!("nested-fences-lazy-{number}"), &edited),
            "{from}"
        );
    }
}

/// Fence kinds in nested record items: a `~~~` fence is not closed by a
/// backtick fence; a four-backtick fence is not closed by three backticks,
/// only by four (equal) or five (longer). After the closing fence the item
/// goes on, and a heading at the item's indentation ends it — so a fence
/// closed too early (the `# x` after it read as a heading) or never closed
/// (the heading after it read as code) shows in the text.
const FENCE_KINDS: &str = "# Fence kinds in nested record items

- Parent.
    - **HT-211:** tilde fence
      ~~~
      ```
      # x
      ~~~
      after the tilde fence.
      # a heading after it

- Parent.
  - **HT-212:** longer fence
    ````md
    ```
    # x
    ```
    ## x
    `````
    after the longer fence.
    # a heading after it

- Parent.
\t- **HT-213:** equal fence
\t  ````
\t  ```
\t  ## x
\t  ````
\t  # a heading after it
";

#[test]
fn a_tilde_or_longer_fence_in_a_nested_item_closes_only_on_its_own_kind_and_length() {
    assert_eq!(
        texts("fence-kinds", FENCE_KINDS),
        [
            (
                "HT-211".to_owned(),
                "tilde fence\n      ~~~\n      ```\n      # x\n      ~~~\n      after the tilde fence."
                    .to_owned()
            ),
            (
                "HT-212".to_owned(),
                "longer fence\n    ````md\n    ```\n    # x\n    ```\n    ## x\n    `````\n    \
                 after the longer fence."
                    .to_owned()
            ),
            (
                "HT-213".to_owned(),
                "equal fence\n\t  ````\n\t  ```\n\t  ## x\n\t  ````".to_owned()
            ),
        ]
    );
    // The headings after the closing fences are no record's text.
    let base = hashes("fence-kinds-base", FENCE_KINDS);
    let edited = FENCE_KINDS.replace("# a heading after it", "# a heading  after it");
    assert_ne!(edited, FENCE_KINDS);
    assert_eq!(base, hashes("fence-kinds-headings", &edited));
}

/// The column-0 control: an item at column 0 whose fence the line
/// classification itself sees keeps the same text — the whole block and the
/// paragraph after it, no lazy line right after the closing fence.
const COLUMN_ZERO_FENCES: &str = "# Fences in a column-0 record item

- **HT-221:** column zero
  ```sh
  # x
  ## x
  ```
  then build.

- **HT-222:** column zero lazy
  ```sh
  # x
  ```
lazy after fence 222

- **HT-223:** column zero tilde
  ~~~
  ```
  # x
  ~~~
  after the tilde fence.
";

#[test]
fn a_fence_in_a_column_0_record_item_keeps_its_text() {
    assert_eq!(
        texts("column-zero-fences", COLUMN_ZERO_FENCES),
        [
            (
                "HT-221".to_owned(),
                "column zero\n  ```sh\n  # x\n  ## x\n  ```\n  then build.".to_owned()
            ),
            (
                "HT-222".to_owned(),
                "column zero lazy\n  ```sh\n  # x\n  ```".to_owned()
            ),
            (
                "HT-223".to_owned(),
                "column zero tilde\n  ~~~\n  ```\n  # x\n  ~~~\n  after the tilde fence."
                    .to_owned()
            ),
        ]
    );
    let base = hashes("column-zero-lazy-base", COLUMN_ZERO_FENCES);
    let edited = COLUMN_ZERO_FENCES.replace("lazy after fence 222", "lazy  after fence 222");
    assert_ne!(edited, COLUMN_ZERO_FENCES);
    assert_eq!(base, hashes("column-zero-lazy-edited", &edited));
}

/// A comment the item's first line opens and a column-0 line closes: the
/// rest of that line shows a list marker once the comment is removed, yet
/// the line opens inside the item's comment, so it continues the item and
/// starts no list item.
const COMMENT_THEN_MARKER: &str = "# Comment continuation into a list marker

- **HT-231:** alpha <!-- start
end -->    - x

- **HT-232:** beta <!-- start
end --> - y

- **HT-233:** the next item.
";

#[test]
fn a_line_closing_the_items_comment_continues_it_whatever_follows_the_comment() {
    assert_eq!(
        texts("comment-then-marker", COMMENT_THEN_MARKER),
        [
            (
                "HT-231".to_owned(),
                "alpha <!-- start\nend -->    - x".to_owned()
            ),
            (
                "HT-232".to_owned(),
                "beta <!-- start\nend --> - y".to_owned()
            ),
            ("HT-233".to_owned(), "the next item.".to_owned()),
        ]
    );
    edit_moves_only(
        "comment-then-marker",
        COMMENT_THEN_MARKER,
        "HT-231",
        "end -->    - x",
        "end -->    - z",
    );
}
