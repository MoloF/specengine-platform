---
class: canon
tier: 1
scope: [crates/specengine-import]
owner: owner
reviewed: 2026-10-04
---

# specengine-import — importers of existing spec corpora

Two read-only passes over an existing corpus (08 §4): the **census** (dry-run counts) and the **import** (the census scanner plus record recognizers: records with verbatim hashes, the "before" report). Nothing corpus-specific lives in code: every convention comes from a `CensusConfig` read at run time (ADR-0008). Called by `specengine-eval census`, `import`, `parse`. Pins: `toml =1.1.4`, `regex =1.13.1`, `blake3`.

**One walk**, `walk::documents(root, &CensusConfig) -> Walk`: files under `roots` with a listed extension, minus `exclude`; dot-directories and symlinks skipped; an unreadable directory or missing root is a diagnostic. `census::run`, `import::run` (`run_walked`: on a walk taken) and eval `parse` share it: equal document counts.

## Config

`CensusConfig::load` / `parse` (`config.rs`): TOML, `deny_unknown_fields` in every table, errors `file:line: message`. `[corpus]`, `ids.regex` required; `class_key`, `header_table`, `id_header`, `text_header`, `local_number`, `base` have no default (absent: off); the rest shows its default. `census` validates the import keys, then ignores them. Examples: `fixtures/{corpus-mini,import-one,import-two}/census.toml`.

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
lead_in = false # items opening with a strong ID
separators = [] # one stripped after the ID
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
```

Refused at the key's line: an unknown key, a wrong type, a bad regex or glob; an import regex matching `""`; a `..` or absolute root, `base` or code root, a code root naming the corpus root; a `feature_prefixes` entry or legacy target outside `[A-Z][A-Z0-9]*`, a legacy key not a letter run; a hyphenless pattern twice; an empty map key or target, separator or `strip` entry.

## Census

`census::run` → `Census`. Front-matter (`frontmatter::read`, lenient: the `---` block and the `class_key` scalar; CRLF, BOM; unclosed: a diagnostic, body still read) gives `per_class`. Records: ID'd record-table rows, hashed as the whole line without terminator, and `{#ID}` sections, heading to the next of the same or higher level, trailing whitespace trimmed; rows without an ID, `duplicate_ids`, `tables`, `record_tables`, `headerless_blocks`. Diagnostics are never fatal.

- **ID scripts** (`script.rs`, ADR-0009): look-alikes (fullwidth ASCII; Cyrillic, Greek letters identical to a Latin one) normalise one for one before matching, an all-look-alike prefix too (the import refuses that); the match as written is `latin`, `mixed-script` or `non-latin`.
- **Links**: inline links and reference definitions, unescaped and percent-decoded, from the document or, `/`-leading, the root; scheme and `#anchor`-only links skipped. Wiki targets resolve under `wiki_root` (extension optional, case ignored, relative or a `/`-bounded tail) or from the document.
- **Cost**: `markdown::scan`, one scanner, linear per line: fences and comments hide content; `CodeSpans`, `InlineIndex` close spans and destinations by binary search.

## Import

`import::run` → `Import` (`import/`), records in (path, line, form) order. `ImportRecord`: `path`, `line` (of the ID), `form` (`table-row` · `headerless-row` · `list-item` · `section`), `id`, `prefix` (Latin), `aliases` (the written ID, if legacy-mapped), `script` (as written), `role` (`definition` · `reference`), `scope` (`project` · `feature`), `text` (verbatim), `hash` (BLAKE3 hex of `text`), `fields` (table rows: other cells as `{header, value}`, header as written, `col-N` 0-based if absent, empty or repeated).

- **Positions**: a `{#ID}` heading; a record-table row (an ID in the ID column, or `id_header` matching; never a field table); a list item opening (after `[ ]` / `[x]`) with `**…**` / `__…__` whose trimmed content minus one separator is an ID (a span holding a title too: none).
- **Text**. Row: the first non-ID cell whose header matches `text_header`, else `text_column`, trimmed, escapes as written; section: the census span. List item: its raw first line after the span and one separator (right after the span, else past blanks), comments kept; then deeper and lazy lines, until a blank line before a shallower one, an item at the marker's indent or shallower, a nested record item, or a heading. A lazy line continues the paragraph unless it opens a list item, heading, fence, table row, thematic break, HTML block, block quote, or starts `<!--` outside a comment; a line inside a comment the item's text opened continues it (no block test); a fence opened on a deeper line holds code to its closing fence, no lazy line after it. Block syntax is read past the marker's columns (tab: next multiple of four), not the content column. Lines joined by LF (CR before LF dropped), nothing else normalised.
- **Role**: IDs in a `reference_paths` document or a `reference_headers` table are references, else definitions. A duplicate redefines an ID earlier in (path, line) order: per corpus, per document for `feature_prefixes`; `unresolved`: a reference defined nowhere.
- **IDs**: a candidate whose letter run equals a `legacy` key as written takes the Latin prefix before `ids.regex`, the written ID an alias (`legacy.mapped`); else look-alikes normalise only if the prefix keeps an ASCII letter (`homoglyph_fixes`). A non-Latin prefix in no map counts `legacy.unmapped`: no record, no guessed ID; found by shape (a letter of any script, letters or digits, `-`, digits), whatever `like`.
- **Rows without an ID**, record tables only: a cell matching `local_number` → `local_number`, else `none`.
- **Header**: YAML top-level keys and a field table's: in `key_map` → `mapped`, equal to a target → `kept`, else `unmapped`; single-line values of a `value_map` target alike. Both forms count `yaml`; a skipped document `none`.
- **Hyphenless**: a match at a strong lead-in (list item, paragraph start) is a definition, else a mention; never a record, ID, alias or unclaimed token; a token once, under the first pattern listed.
- **Unclaimed**: `like` tokens, bounded as the reference grammar's steps 1 and 4, over the body the scanner sees and front-matter values (scalars, sequence items, continuations; comments stripped, a quote opening only at a scalar or flow-item start, also after `!tag` / `&anchor`), minus record IDs and hyphenless matches; emphasis delimiter runs (`*`, `_`, CommonMark flanking) read as blanks. Defined after mapping → `claimed`, else `unclaimed`; a feature-scoped ID is claimed only in a document defining it, elsewhere `feature_outside`.
- **Links**: a missing file link is retried from `base` (`resolved_by_base`).
- **Code scan**: each document path, and each form minus a `strip` prefix, as a byte substring with no letter, digit, `_`, `-`, `.` before and no letter, digit, `_` after; once per occurrence, longest form, own path first. Dot-directories and names starting `target` below a root skipped; a missing root counts.

## The "before" report

stdout `result`, counts only (`class-N`, `prefix-N`, `pattern-N` by descending count), exactly: `documents{total, per_class}`, `front_matter{yaml, field_table, none, unclosed, non_latin_keys, keys, values}` (each `{mapped, kept, unmapped}`), `records{total, empty_text, per_form{table_row, headerless_row, list_item, section}, per_prefix}`, `definitions`, `references{total, unresolved}`, `duplicate_definitions`, `rows_without_id{local_number, none}`, `legacy{mapped, unmapped, homoglyph_fixes, hyphenless{definitions, mentions, per_pattern}}`, `id_like{claimed, unclaimed, feature_outside, files_with_unclaimed}`, `broken_links{file, wiki, resolved_by_base}`, `code{files, documents_cited, citations, roots_missing}`, `detail{files_skipped, roots_missing, diagnostics, import_ms}`. Detail, `--out/import/<label>/*.json` (`import/model.rs` types): `labels` (the mapping), `documents`, `records`, `rows_without_id`, `duplicates` (+ `unresolved_references`), `legacy`, `unclaimed` (cause `unclaimed` · `feature-outside`), `broken_links`, `code_citations`, `diagnostics`.

## Open minors

- Exclude globs do not prune directories; an unclosed fence or comment and skipped symlinks drop records silently; `strip_comments` ignores backslash escapes, `<…>` destinations and `<!-->` deviate from CommonMark; front-matter is read leniently, not by `specengine-core`.
- A fence at four columns or more is unseen by the scanner: a `<!--` in a nested item's code can hide later lines.
- Import gaps: `docs/features/import-records.md` "Open" (follow-up `import-gaps`).

## Tests

`tests/census.rs`, `tests/import_hash.rs`, `tests/import_rules.rs`; end to end: eval `census_cli.rs`, `import_cli.rs`, `import_genre.rs`.
