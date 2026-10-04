---
class: canon
tier: 1
scope: [crates/specengine-import]
owner: owner
reviewed: 2026-10-04
---

# specengine-import — importers of existing spec corpora

Two read-only passes over an existing corpus (08 §4): the **census** (dry-run counts) and the **import** (the census scanner in import mode plus record recognizers: records with verbatim hashes, the "before" report; model and rules: `docs/canon/import.md`). Nothing corpus-specific lives in code: every convention comes from a `CensusConfig` read at run time (ADR-0008). Called by `specengine-eval census`, `import`, `parse`. Pins: `toml =1.1.4`, `regex =1.13.1`, `blake3`.

**One walk**, `walk::documents(root, &CensusConfig) -> Walk`: files under `roots` with a listed extension, minus `exclude`; dot-directories and symlinks skipped; an unreadable directory or missing root is a diagnostic. `census::run`, `import::run` (`run_walked`: on a walk taken) and eval `parse` share it: equal document counts.

## Config

`CensusConfig::load` / `parse` (`config.rs`): TOML, `deny_unknown_fields` in every table, errors `file:line: message`. `[corpus]`, `ids.regex` required; `class_key`, `header_table`, `id_header`, `text_header`, `local_number`, `base`, `id_key`, `id_path` have no default (absent: off); the rest shows its default. `census` validates the import keys, `[documents]` included, then ignores them. Examples: `fixtures/{corpus-mini,import-one,import-two}/census.toml`.

```toml
[corpus]
roots = ["."]
extensions = ["md"]
exclude = [] # globs: `*`, `**`, `?`
[front_matter]
class_key = "kind"
header_table = '^Field$' # 1st header cell of a 2-column table opening the body
header_row_field = false # the header row is a field
key_map = {} # key as written → target
value_map = {} # per target: value → target
[ids]
regex = '^[A-Z]{2}-[0-9]{3}$' # after look-alikes; prefix: group `prefix`, else leading ASCII letters
like = '\p{Lu}[\p{Lu}\p{N}]*-[0-9]+' # ID-like tokens
feature_prefixes = [] # unique per document (ADR-0026)
hyphenless = [] # regexes, counted only
legacy = {} # prefix as written → Latin prefix
[tables]
id_column = 0
id_header = '^ID$' # record tables; absent: any with an ID row
headerless = false # `|` blocks without a GFM header row
text_column = 1 # default id_column + 1
text_header = '^Text$' # beats text_column
local_number = '^[0-9]+$' # an ID cell holding no ID
[sections]
id_attr = true # `{#ID}` headings
[lists]
lead_in = false # items opening with a strong ID [separator [title]]
separators = [] # split ID, title and text
[definitions]
reference_paths = [] # globs
reference_headers = [] # over any header cell
[links]
wiki = false # also `[[target]]`
wiki_root = "."
base = "docs" # retry base for missing file links
[code]
roots = [] # read as bytes, never built
extensions = [] # none: every file
exclude = []
strip = [] # path prefixes also matched without
[documents]
id_key = "ident" # a header target after key_map: its value is the document's ID
id_path = '^notes/(?P<id>[A-Z]+-[0-9]+)\.md$' # corpus-relative, `/`-separated; group `id`
```

Refused at the key's line: an unknown key, a wrong type, a bad regex or glob; an import regex matching `""`; a `..` or absolute root, `base` or code root, a code root naming the corpus root; a `feature_prefixes` entry or legacy target outside `[A-Z][A-Z0-9]*`, a legacy key not a letter run; a hyphenless pattern twice; an empty map key or target, separator or `strip` entry; an empty `id_key` or one with blanks around it, an `id_path` without group `id`.

## Census

`census::run` → `Census`. Front-matter (`frontmatter::read`, lenient: the `---` block and the `class_key` scalar; CRLF, BOM; unclosed: a diagnostic, body still read) gives `per_class`. Records: ID'd record-table rows, hashed as the whole line without terminator, and `{#ID}` sections, heading to the next of the same or higher level, trailing whitespace trimmed; rows without an ID, `duplicate_ids`, `tables`, `record_tables`, `headerless_blocks`. Diagnostics are never fatal.

- **ID scripts** (`script.rs`, ADR-0009): look-alikes (fullwidth ASCII; Cyrillic, Greek letters identical to a Latin one) normalise one for one before matching, an all-look-alike prefix too (the import refuses that); the match as written is `latin`, `mixed-script` or `non-latin`.
- **Links**: inline links and reference definitions, unescaped and percent-decoded, from the document or, `/`-leading, the root; scheme and `#anchor`-only links skipped. Wiki targets resolve under `wiki_root` (extension optional, case ignored, relative or a `/`-bounded tail) or from the document.
- **Cost**: `markdown::scan` (the import: `scan_import`, the same scanner in import mode), linear per line: fences and comments hide content; `CodeSpans`, `InlineIndex` close spans and destinations by binary search.

## Import

`import::run` → `Import` (`import/`): the record model, positions, titled lead-ins, document records, text and hash, precedence, the import-mode comment rules, header outcomes, the code scan and known limits are `docs/canon/import.md`. `import::document_text` (pub) is the document-text normalisation `import-layout` reuses.

## Before report

stdout `result`, counts only (`class-N`, `prefix-N`, `pattern-N` by descending count), exactly: `documents{total, per_class}`, `front_matter{yaml, field_table, none, unclosed, non_latin_keys, keys, values}` (each `{mapped, kept, unmapped}`), `records{total, empty_text, titled, per_form{table_row, headerless_row, list_item, section, document}, per_prefix}`, `definitions`, `references{total, unresolved, by_document}`, `duplicate_definitions`, `rows_without_id{local_number, none}`, `legacy{mapped, unmapped, homoglyph_fixes, hyphenless{definitions, mentions, per_pattern}}`, `id_like{claimed, unclaimed, feature_outside, files_with_unclaimed}`, `broken_links{file, wiki, resolved_by_base}`, `code{files, documents_cited, citations, roots_missing}`, `detail{files_skipped, roots_missing, diagnostics, import_ms}`; `titled` counts records with a `title`, `by_document` the record-position definitions a document demoted (within `references.total`). Detail, `--out/import/<label>/*.json` (`import/model.rs` types): `labels` (the mapping), `documents`, `records`, `rows_without_id`, `duplicates` (+ `unresolved_references`), `legacy`, `unclaimed` (cause `unclaimed` · `feature-outside`), `broken_links`, `code_citations`, `diagnostics`.

## Open minors

- Exclude globs do not prune directories; an unclosed fence or comment and skipped symlinks drop records silently; `strip_comments` ignores backslash escapes, `<…>` destinations and `<!-->` deviate from CommonMark; front-matter is read leniently, not by `specengine-core`.
- A fence at four columns or more is unseen by the scanner: a `<!--` in a nested item's code can hide later lines.
- The census is frozen (`markdown::scan`): after `-->` it still reads a heading, opens a fence and reads a table row; the import does not (`docs/canon/import.md` "Comments").
- The code scan still counts full-path forms right after `/` (only stripped forms are refused there).
- Import limits and the open `id_key` minor: `docs/canon/import.md` "Known limits".

## Tests

`tests/census.rs`, `tests/import_hash.rs`, `tests/import_rules.rs`, `tests/import_gaps.rs`; end to end: eval `census_cli.rs`, `import_cli.rs`, `import_genre.rs`, `import_gaps_cli.rs`.
