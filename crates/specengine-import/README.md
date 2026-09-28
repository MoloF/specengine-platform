---
class: canon
tier: 1
scope: [crates/specengine-import]
owner: owner
reviewed: 2026-09-29
---

# specengine-import — importers of existing spec corpora

Today: the **census**, a read-only dry-run counter over an existing corpus (08 §4). Nothing corpus-specific lives in code: the whole convention comes from a `CensusConfig` read at run time (ADR-0008); a fixture corpus and a real one take the same code path. Called by `specengine-eval census`; the writing importer of Phase 1 builds on it. Pins: `toml =1.1.4`, `regex =1.13.1`, `blake3`.

## Census config

`CensusConfig::load` / `parse` (`config.rs`): TOML, `deny_unknown_fields` in every table, errors as `file:line: message`. `[corpus]` and `ids.regex` are required; every other key is shown with its default.

```toml
[corpus]
roots = ["."]              # corpus-relative; `..` or an absolute path is an error
extensions = ["md"]        # document extensions, without the dot
exclude = []               # globs over corpus-relative `/` paths: `*`, `**`, `?`
[front_matter]
class_key = "kind"         # no default; absent → every document `unclassified`
[ids]
regex = '^[A-Z]{2}-[0-9]{3}$'  # searched after look-alike normalization, first match wins;
                               # prefix = named group `prefix`, else the leading ASCII letters
[tables]
id_column = 0              # 0-based column holding the ID
id_header = '^ID$'         # no default; regex over the ID-column header picks record tables;
                           # absent → a table is a record table if any row has an ID
headerless = false         # opt-in: `|` blocks without a GFM header row
[sections]
id_attr = true             # `{#ID}` on headings
[links]
wiki = false               # opt-in: also check `[[target]]` links
wiki_root = "."            # only with wiki = true; where wiki targets resolve
```

Example: `fixtures/corpus-mini/census.toml` (an invented convention). Configs of real corpora live outside this repository.

## What it counts

`census::run` → `Census`:

- **Documents**: files under `roots` with a listed extension, minus `exclude`; symlinks skipped, non-UTF-8 → diagnostic. Front-matter (`frontmatter::read`, lenient: only the `---` block's extent and the `class_key` scalar; CRLF, BOM; an unclosed block → diagnostic, body still read) gives the class; `with_front_matter`, `per_class`.
- **Records**: ID'd rows of record tables and `{#ID}` sections, each hashed with BLAKE3 — a row as its line written, without the terminator; a section from its heading up to the next heading of the same or higher level, trailing whitespace trimmed. Also rows without an ID, `duplicate_ids`, `tables`, `record_tables`, `headerless_blocks`.
- **ID scripts** (`script.rs`, ADR-0009): look-alikes (fullwidth ASCII, Cyrillic and Greek letters identical to a Latin one) are normalized one char for one before matching; the verbatim match is `latin`, `mixed-script` (ASCII letters plus a foreign letter or digit) or `non-latin` (a legacy ID, an alias at import). `per_prefix` uses the normalized prefix.
- **Links**: Markdown inline links and reference definitions, resolved after backslash unescaping and percent-decoding, relative to the linking document or from the corpus root when `/`-leading; scheme URLs and `#anchor`-only links are skipped. Wiki targets resolve against files under `wiki_root` (as written or with a document extension, case ignored, as a relative path or a `/`-bounded tail) or relative to the linking document. Broken links keep the target as written.
- **Diagnostics**: unreadable files, unclosed front-matter, missing roots — counted, never fatal.

## Cost bounds

`markdown::scan` is one line scanner, linear per line: fenced blocks and HTML comments hide their content; code spans close by binary search over the line's backtick runs (`CodeSpans`), link destinations by binary search over a one-pass `InlineIndex`, so a line of unclosed runs or of `](` never rescans. Tests: very long lines and many link openers finish (`tests/census.rs`).

## Open minors

- Exclude globs do not prune directories: an excluded subtree is still walked.
- Non-Latin IDs whose letters have no Latin look-alike fall into rows without ID.
- An unclosed fence or HTML comment and skipped symlinks drop records without a diagnostic.
- `markdown::strip_comments` (HTML comments) ignores backslash escapes; `<…>` destinations and `<!-->` comments deviate from CommonMark; entity references in destinations are not decoded.
- The front-matter reader is not shared with the std-only `xtask`.

What real corpora need beyond this — a per-corpus link base, a definition-versus-reference rule for IDs, aliases for non-Latin prefixes, locally numbered tables — is listed in 08 §4.3.

## Tests

`tests/census.rs` (config errors, scripts by escapes, front-matter, tables, links and escapes, sections, walk, cost bounds); end to end: `specengine-eval/tests/census_cli.rs` (fixture equals `fixtures/corpus-mini/expected.json`, anonymous stdout).
