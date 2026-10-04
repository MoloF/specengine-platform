---
class: canon
tier: 2
scope: [crates/specengine-import]
owner: owner
reviewed: 2026-10-04
---

# Import: record model and rules

`import::run` (`run_walked` on a walk taken) → `Import` (`specengine-import` `src/import/`): the census walk and scanner in import mode (`markdown::scan_import`) plus record recognizers, read-only, never fatal. Every convention comes from the config (ADR-0008): keys and refusals in `crates/specengine-import/README.md` "Config", stdout and detail files in its "Before report". A record is what `import-layout` writes and re-hashes: a title becomes the heading of a `{#ID}` section, a document record a record file (ADR-0026), a legacy ID an alias (ADR-0009). The census (`markdown::scan`) is frozen: shared code changes only if every census output stays equal.

## Record model

`ImportRecord`, in (path, line, form) order: `path`; `line` (of the ID; a document's: its defining key's, 1 from a path); `form` (`table-row` · `headerless-row` · `list-item` · `section` · `document`); `id`; `prefix` (Latin); `aliases` (the written ID, if legacy-mapped); `script` (as written); `role` (`definition` · `reference`); `scope` (`project` · `feature`); `title` (titled list items only, omitted otherwise); `text` (the body as written); `hash` (BLAKE3 hex of `text`, every form); `fields` (table rows: other cells as `{header, value}`, header as written, `col-N` 0-based if absent, empty or repeated). A title is never body. From ``- **XQ-12: Short `name`**: body line`` and `notes/XQ-3.md` = `---` / `ident: XQ-3` / `---` / blank / `# Heading` / blank / `Body.` (`id_key = "ident"`):

```json
{"path":"specs/a.md","line":7,"form":"list-item","id":"XQ-12","prefix":"XQ","aliases":[],"script":"latin","role":"definition","scope":"project","title":"Short `name`","text":"body line","hash":"<hex>","fields":[]}
{"path":"notes/XQ-3.md","line":2,"form":"document","id":"XQ-3","prefix":"XQ","aliases":[],"script":"latin","role":"definition","scope":"project","text":"# Heading\n\nBody.","hash":"<hex>","fields":[]}
```

## Positions

- a `{#ID}` heading → `section`;
- a record-table row (an ID in the ID column, or `id_header` matching; never a field table) → `table-row`; a row of a `headerless` block → `headerless-row`;
- a list item opening (after `[ ]` / `[x]`) with a strong span `**…**` / `__…__` holding a lead-in ID (`[lists] lead_in`; "Titled lead-ins") → `list-item`;
- a document naming its own ID (`[documents]`; "Document records") → `document`.

## Titled lead-ins

One `[lists] separators` list splits ID from title and title from text.

- The span's trimmed content reads as ID [separator [title]]: the ID is the longest leading part that resolves, ending right before a listed separator or at the content end, each candidate trimmed; several separators at one position: the longest. Resolving: an ID, or an unmapped legacy prefix (→ `legacy.unmapped`, no record); a hyphenless match does not resolve, the next shorter part is tried. Nothing resolves: no record.
- A title exists iff the comment-free span holds content after the separator. Its text is the raw line from right after the separator to the closing delimiter, trimmed; inline markup, escapes and comments kept (`**ID. Title.**` → `Title.`). It stays outside `text` and `hash`; ID-like tokens in it count as anywhere; the hyphenless lead-in test is unchanged.
- A span ending in a separator with no title strips no separator after it; any other span strips one (right after the span, else past blanks).
- A legacy titled ID → the Latin `id` and an alias; a feature-scoped one → a record.

## Text and hash

- **Row**: the first non-ID cell whose header matches `text_header`, else `text_column`, trimmed, escapes and a comment opener as written.
- **Section**: the heading to the next heading of the same or higher level the import scanner reads, trailing whitespace trimmed: the census span unless a heading sits on a line opening inside a comment ("Comments").
- **List item**: the raw first line after the span and its separator, comments kept; then deeper and lazy lines, until a blank line before a shallower one, an item at the marker's indent or shallower, a nested record item (titled ones included), or a heading. A lazy line continues the paragraph unless it opens a list item, heading, fence, table row, thematic break, HTML block, block quote, or starts `<!--` outside a comment. A fence opened on a deeper line holds code to its closing fence, no lazy line after it. Block syntax is read past the marker's columns (tab: next multiple of four), not the content column. Lines joined by LF (CR before LF dropped), trailing whitespace trimmed.
- **Document**: the body is everything after a closed YAML block except a field table's lines (header through last row, lines skipped inside it included) and the blank lines (spaces, tabs) right after it; headings before the table are kept, in order. An unclosed YAML block is no header: the body starts at line 1. `text` = `import::document_text(body)` (pub): a leading BOM, a CR before LF and leading lines of only spaces and tabs dropped, trailing `[ \t\r\n]` trimmed (NBSP, U+3000 and other Unicode blanks kept), nothing else normalised, nested records included. The import path skips a leading BOM before header detection, so a heading or row on line 1 is read (the census does not). `import-layout` normalises an emitted file's body with the same function.

## Document records

`[documents]`, each key off when absent:

- **`id_key`**: a header key (YAML top level or field table) whose target after `key_map` equals it (a key literally equal to it is its own target) has its single-line value read as a record-table ID cell (legacy map, look-alikes, `ids.regex`). Resolving → one `document` record at the key's line. Not resolving, an unmapped legacy prefix included (no `legacy.unmapped`) → one diagnostic, then `id_path`. A second key reaching `id_key` → one diagnostic; the first counts.
- **`id_path`**, only when no header ID resolves: over the corpus-relative `/`-separated path, group `id` read the same way, `line` 1. A match in which group `id` took no part, or a group that does not resolve → one diagnostic (no line), no record.
- Header and path resolving to different IDs → the header's, one diagnostic. A feature-scoped document ID → one diagnostic, no record, no fallback to `id_path` (a disagreeing path adds one more).
- The header value read (resolving or not) is never an ID-like token; citations elsewhere become `claimed`. In a `reference_paths` document the record is a `reference`.

## Roles and precedence

- IDs in a `reference_paths` document or a `reference_headers` table are references, else definitions.
- **Precedence**: an ID a document defines turns every record-position definition of it into a `reference` (`references.by_document`); a document-level reference demotes nothing.
- A duplicate redefines an ID earlier in (path, line) order, per corpus (per document for `feature_prefixes`); two documents defining one ID → `duplicate_definitions`, the later. `unresolved`: a reference defined nowhere.

## IDs and tokens

- **IDs**: a candidate whose letter run equals a `legacy` key as written takes the Latin prefix before `ids.regex`, the written ID an alias (`legacy.mapped`); else look-alikes normalise only if the prefix keeps an ASCII letter (`homoglyph_fixes`). A non-Latin prefix in no map counts `legacy.unmapped`: no record, no guessed ID; found by shape (a letter of any script, letters or digits, `-`, digits), whatever `like`.
- **Rows without an ID**, record tables only: a cell matching `local_number` → `local_number`, else `none`.
- **Hyphenless**: a match at a strong lead-in (list item, paragraph start, not on a line opening inside a comment) is a definition, else a mention; never a record, ID, alias or unclaimed token; a token once, under the first pattern listed.
- **Unclaimed**: `like` tokens, bounded as the reference grammar's steps 1 and 4, over the body the scanner sees and front-matter values (scalars, sequence items, continuations; comments stripped, a quote opening only at a scalar or flow-item start, also after `!tag` / `&anchor`), minus record IDs, the `id_key` value read and hyphenless matches; emphasis delimiter runs (`*`, `_`, CommonMark flanking) read as blanks. Defined after mapping → `claimed`, else `unclaimed`; a feature-scoped ID is claimed only in a document defining it, elsewhere `feature_outside`.

## Header outcomes

YAML top-level keys and a field table's (both forms count `yaml`, a skipped document `none`): a `key_map` key with another target → `mapped`; a key equal to a target (an identity `key_map` entry, a `key_map` value, a `value_map` table, `documents.id_key`) → `kept`; any other key, any script → `unmapped`. Single-line values of a `value_map` target alike.

## Comments

Import mode only (`scan_import`); the census still reads these lines (`crates/specengine-import/README.md` "Open minors").

- A line opening inside any HTML comment gives no heading (no section starts or ends there, none for tokens), opens no fence, starts no table; no list recognizer (records, lead-ins, nested records) takes anything from it.
- Inside an item's comment it is the item's paragraph text whatever follows `-->`: it ends nothing, holds no list marker or record lead-in; blank lines inside the comment join. Literal rule: it continues the item even after a blank or a deeper HTML-block line (strict CommonMark would end it).
- Inside a table such a line, blank or a heading after `-->` included, is neither a row nor the table's end; its links are still collected. A line the row loop rejects is plain text; the loop always advances.

## Links and code scan

- **Links**: as the census; a missing file link is retried from `base` (`resolved_by_base`).
- **Code scan**: each document path, and each form minus a `strip` prefix, as a byte substring with no letter, digit, `_`, `-`, `.` before and no letter, digit, `_` after; a stripped form also not right after `/` (a deeper path is no shallower namesake's); once per occurrence, longest form, own path first. Dot-directories and names starting `target` below a root skipped; a missing root counts.

## Known limits

- A comment opened in the **last** row of a field table: its continuation line lands in the document body (one opened mid-table stays out). Fix: the scanner records the last consumed line as the table's end.
- A statement wholly inside the strong span gives empty `text` (`records.empty_text`); such records share BLAKE3(""), so `import-layout` compares their titles.
- No record: a separator not right after the ID, a strong span wrapped onto the next line, a paragraph lead-in.
- The code scan is path-only: a relative form resolving from where a generated file lands (`../../index.md`) is not credited; full-path forms still count right after `/`.
- Document text holds nested records: `import-layout`'s range files mismatch by design. A loose `id_path` demotes real definitions: watch `by_document`.
- Open minor: a second key reaching `id_key` (diagnosed) still counts its value's tokens.
