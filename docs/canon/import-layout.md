---
class: canon
tier: 2
scope: [crates/specengine-import, crates/specengine-eval]
owner: owner
reviewed: 2026-10-04
---

# Import layout: the emitter and the after-tree

`specengine-eval layout` (CLI: `crates/specengine-eval/README.md` "CLI contract") projects the import's "before" model (`docs/canon/import.md`) into the target layout (`docs/canon/architecture.md#layout`, ADR-0026) per the corpus's `[layout]` (`crates/specengine-import/README.md` "Config"), reads the tree back through core and compares each definition **by identity** (path, line, ID), never by hash: lossless means bytes, not rendering. The verifier, the attribution of findings and the output: `docs/canon/import-layout-verifier.md`.

**One process**: refusals; `import::run_walked` and the **emitter** `layout::emit` (`specengine-import` `src/layout/`, pure: bytes, model, configs → files, map, scheme, header outcomes, diagnostics), both before `--timeout` starts; the tree written to `<out>/layout/<label>/tree/` (`<out>/layout/<label>/` emptied first; nothing deleted elsewhere), `index = true` entries rendered there (core `render_index_set`); the verifier (eval `src/layout.rs`) re-reads it through core; the before check (the corpus, the before scheme, no baseline); the tree check under observe → attribution → `.spec-debt.toml` → under enforce; the store's index. Never written: the corpus, the repository, `HOME` (nor read). Output in path, line, source order; no hash-map order reaches it.

## Emission

Definitions only; references and rows without an ID stay. A prefix's target is its `[layout] targets` entry, else `file` for a project prefix, `section` for a feature-scoped one.

- `file`: any form but a document → `<records>/<P>/<ID>.md`; later duplicates, in (path, line) order, `<ID>-2.md`, `-3`…
- `section`: a row or item → a reshaped `{#ID}` section of its own document; a section stays in place.
- A document record stays in place (a feature document may move), gaining `id` ("Headers").
- What lies inside a section moved to a file (a reshaped block, an in-place section) goes with it, and so does the map.
- Missing, the source lines kept in the residue: a prefix not in the scheme (`prefix_unknown`); a record path a walked document holds, not written (`path_taken`); `slug` ("Feature documents").
- **Rule S**: a carried `{#written}` or `id:` value of a legacy-mapped or look-alike-fixed record becomes the Latin ID; no other text byte changes (ADR-0009).
- **The map** (`emission.json`): per definition (source path, line, ID) → (after path, place `file` · `section` · `in-place`, reason?); per source document → after path, moved, feature. No text, no hash.

## Record files

`---`; `id`, `class` (`record_class`) plain; `title` (a titled item, a section); `aliases: ["<written>"]` if written ≠ `id`; the task key (bool); the fields in column order: an ID cell holding text beyond the ID (`id_cell`, key: its column header as written, else `col-N`), each other cell under its header's `key_map` target, else the header as written; a section's other heading attributes (bare → `true`); `---`, a blank line, `text` (a section's minus its heading). A section's title is its heading minus the ATX marker (closing run included) and exactly the `{…}` block the import read, both sides trimmed, joined by one space.

`- [x] **XQ-12: Short name**: body` → `docs/records/XQ/XQ-12.md`: `id: XQ-12`, `class: canon`, `title: "Short name"`, `done: true`; body `body`.

## YAML

Record files and headers alike. Values double-quoted. A typed core key (core `TYPED_KEYS`, pinned to `layout::typed_keys()`) is written in core's type where its text parses as it:

- an integer key: a canonical integer plain (`007`, `+1` stay quoted); text keys, dates included, stay quoted;
- a list key: a one-item flow list;
- `refs`, `adrs`, `supersedes`: the text split at commas, items trimmed, empty items dropped; a quoted scalar with a real escape (`\` in double quotes, `''` in single) stays one item;
- a mapping key: never from a scalar.

Keys are plain iff `^[A-Za-z_][A-Za-z0-9_-]*$` and not null- or bool-like (`null`, `~`, `true`, `no`, `on`, `y`…, any case). A field key repeating a core key, the task key or an earlier field → `col-N` (N the 0-based column); that taken too → dropped (`header.conflicts` +1).

## Reshaped sections

`#`×L, the title else the ID, ` {#ID`, ` <task_box_key>=true|false`, each field ` key=value`, `}`. A pair holding a blank or a brace, a key empty, holding `=` or opening `#` / `.` is left out (`section_fields`; a field or box miss). n = the level of the nearest import-scanner heading above the extent (none: 0), L = min(n+1, 6). `""`, heading, `""`, `text` follow that section's last non-blank residue line (n = 0: the document's); at one such anchor, the blocks of deeper enclosing sections come first, then source order.

## Feature documents

A document holding a feature-scoped definition stays when it lies directly under `<features>` with a slug stem; else it moves to `<features>/<slug>.md` (the `slug` regex's group, else the stem). A non-slug, a target shared or held by a walked document → a diagnostic, the document stays, its feature-scoped definitions are missing (`slug`).

## Residue

Each walked document at its (moved) path, a non-`.md` one renamed `.md` (`moved`; a name taken, compared case-insensitively → a diagnostic, the name kept): the normalised header, the body lines outside extents written elsewhere (an emptied table keeps its header rows), the reshaped blocks; LF-joined, no BOM, no CR before LF, a final LF.

## Headers

Keys read by the import's lenient reader:

- A top-level key → its `key_map` target; a target the header already holds → kept as written, `conflicts` +1. A key reaching `id` is renamed only when the import read the document's ID from that line and its value is that ID alone; otherwise it is kept as written and `id` is added. A key kept as written after a collision takes no value map and no retyping; a key reaching `id` kept as written and a repeated field-table key (below) take neither a value map nor rule S.
- A `value_map` target's single-line scalar → its mapping (quoted, comment kept); a scalar renamed to a typed key is retyped ("YAML").
- `class`: the source class's `value_map` entry, else the first `classes` glob matching the after path, else none; an existing one changes only by the map. A missing `id` (document record), `class`, `aliases` (written ≠ `id`) goes right after `---`; an existing `aliases` stays (`conflicts` +1).
- A field table → a YAML block on top, from each row's key and value cells as above; a repeated key → `<key>-<n>` as written (`conflicts` +1); lines above the table follow it. An unclosed block is no header (a new one goes above). A field table's last-row comment counts `last_row_comment` (`docs/canon/import.md` "Known limits").
- A block core cannot parse (`SourceDocument.header_unparsed`: `frontmatter-yaml`, `-unclosed`, `-not-mapping`, not UTF-8) is carried line-verbatim: no rename, value map or retyping; only a missing `id`, `class`, `aliases` is added. One core reads as no mapping (`header_not_mapping`: a list, a scalar) is carried byte-verbatim with nothing added. Either way a document record there is missing (`header_unparseable`).

## Emitted configs

`specengine.toml` is the before scheme with `[paths]` `records`, `features` from `[layout]`, `roots` the sorted first components of the tree documents and index outputs, `exclude = []`, `link_base` its own, else `[links] base`; `[ids.legacy]` keys join their target's `aliases_from` (a prefix or another's alias: a diagnostic, skipped); `scope = "feature"` per `feature_prefixes`; keys sorted (`layout::toml_text`, hand-written); it loads in core. `.spec-debt.toml` (eval) holds one sorted `[[debt]]` per distinct (code, path, subject) source-caused error, `reason = "import-layout source debt"`, `expires = debt_expires`. Both sit in `tree/`.

## Known limits

- Verbatim is line content (LF, `---`, BOM normalised as for every document).
- A field table under a not-mapping header lands after the verbatim block, and a source opening `---` with trailing blanks (the import accepts it, core does not) is normalised: each a `frontmatter-yaml` emitter finding, visible.
- Decoration after an ID with no letter or ASCII digit (`✓`, non-ASCII digits) is dropped silently; the residue test is the same.
- The `.md` rename collision test is `to_lowercase`, not macOS case folding or Unicode normalisation; record paths are case-sensitive.
- `read_sources` leaves `header_unparsed` and `header_not_mapping` `false`: without the caller's core reading nothing is carried verbatim.
