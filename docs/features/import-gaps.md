---
class: spec
status: shipped
scope: [crates/specengine-import, crates/specengine-eval, fixtures, docs]
ref: import-gaps analysis 2026-10-04, every recommendation accepted by the owner; import-records "Open" 1-6; 08 §4.1 row 1, §4.2 item 1
shipped: 2026-10-04
adrs: []
---

# Import gaps: titled lead-ins, document records, four engine fixes

## Why

`import-layout` re-hashes the "before" model against the after-tree: a missed or misattributed record becomes a silent loss or a false mismatch. `import-records` left six gaps: (1) a strong lead-in holding "ID<sep> Title" resolved to nothing (A: ~135 of ~904 criteria lead-ins were records); (2) IDs a document defines itself (header key, file name; 08 §4.1 row 1) were no definitions, so bodies escaped the hash check and citations stayed unclaimed (A 226, B 173) while B's summary rows posed as definitions; (3) `strip` credited a deeper path to a shallower namesake; (4) identity or unlisted header keys counted `unmapped` (A 1 929); (5) a record line after `-->` joined the previous record; (6) `-->` then `#` ended an item that `-->` then `-` continued. Fixed generically, per corpus config (`docs/canon/architecture.md#universal`); no ADR: it applies ADR-0026 (title → heading of a `{#ID}` section, document record → record file) and ADR-0009 (legacy ID → alias). Census unchanged.

## Data

Now canon: `docs/canon/import.md` "Record model", "Text and hash", "Document records"; `[documents]`: `crates/specengine-import/README.md` "Config"; stdout: its "Before report".

## Acceptance criteria

Fixture ACs:

- [x] AC-01 Each fixture, own separators: `**ID<sep> Title**<sep> text` → one `list-item` record, `title` = Title as written, `text`, `hash` = those of `**ID**<sep> text`; a title edit keeps the hash, a text byte changes it; `**ID**`, `**ID<sep>**` → no `title`; a separator inside the ID → the longest resolving part; none resolving → no record; a titled record item nested in a record item ends the parent's text; a legacy titled ID → Latin `id` + alias; a feature-scoped one → a record, its citations in the same document `claimed`. M: the title kept in `text`; the span read only as ID [+ separator].
- [x] AC-02 `import-one`: a YAML key mapped to `id_key` → one `document` record; `import-two`: a field-table key, and an `id_path` match, each → one. `line` = the key's (1 from the path); `hash` = BLAKE3(`text`); a CRLF copy, an extra blank after the header, an edited header value → equal hash; one body byte → another; the defining value no ID-like token; a citation elsewhere `claimed`; a legacy value → Latin `id`, alias, `legacy.mapped` +1; header and path disagreeing → the header's ID, `detail.diagnostics` +1; a feature-scoped document ID → `diagnostics` +1, no record; in `reference_paths` → `reference`. M: the defining value counted as a token; the hash over the whole file.
- [x] AC-03 An ID defined by a document, a record-table row and a list item → both `reference`, `by_document` 2, `duplicate_definitions` 0, `unresolved` 0; two documents defining one ID → `duplicate_definitions` 1 (the later in path order); a document-level reference demotes nothing. M: precedence off.
- [x] AC-04 `strip = ["<p>/"]`, documents `<p>/x.md`, `<p>/sub/x.md`: code citing `other/x.md` → 0; `"x.md"` → `<p>/x.md`; `sub/x.md` → `<p>/sub/x.md`; `../<p>/x.md` → `<p>/x.md`; `citations` 3, `documents_cited` 2. M: `/` accepted before a stripped form.
- [x] AC-05 A `key_map` identity → `kept`; renaming → `mapped`; a key equal to a `key_map` value, a `value_map` table or `id_key` → `kept`; any other key, Latin or not → `unmapped`; a `value_map` identity → `kept`. M: an identity counted `mapped`.
- [x] AC-06 `- **X-1:** a <!-- c` / `d --> - **X-2:** b` → one record X-1 with both lines, no X-2 record, the X-2 token counted once; `d --> # y`, `d --> - y`, a deeper `  d --> - **X-3:** e` each continue; a hyphenless match after `-->` → a mention; `HT-231` / `HT-232` of `tests/import_hash.rs` unchanged. M: records read on lines opening inside a comment; the heading test applied to them.
- [x] AC-07 `corpus-mini` census = its `expected.json` (file unchanged); `import-one`, `import-two` = their runs without the import keys; a test-generated corpus with titled lead-ins, document IDs, comment-closing lines (incl. `d --> ## T {#ID}`) keeps the census counts pinned before the change. M: the shared scanner stops reading a heading after `-->`.
- [x] AC-08 `[documents]` keys accepted by `census` and `import`; exit 2 at `<config>:<line>`, nothing under `--out`, for an unknown key, a wrong type, a bad regex, an `id_path` matching `""` or without group `id`, an empty `id_key`. M: `deny_unknown_fields` dropped from `[documents]`.
- [x] AC-09 Stdout keys = the whitelist + `records.titled`, `records.per_form.document`, `references.by_document`; both `expected.json` regenerated, each new count ≥ 1 in both; the genre test covers the new fixture keys and values. M: an ID on stdout; a literal `id_key` default.
- [x] AC-10 The "A4" comments (import `tests/import_rules.rs`, eval `tests/import_cli.rs`, `tests/import_support/mod.rs`) cite `` `docs/canon/architecture.md` "Repository language" ``; eval `src/import.rs` cites the README's "Before report" in the pointer form; `doc_pointers` green. M: the quoted heading restored.

Owner's check:

- [x] AC-11 Both pilot configs extended (A: identity `key_map` entries + `id_key`; B: `id_key` targets + `id_path`); `import --label pilot-a|pilot-b` exits 0 within `--timeout`, proofs equal, scratch `HOME` empty, `anonymity` green with `SPECENGINE_PILOT_A` / `_B`; census = `import-records` AC-10's; before/after counts below, every miss explained (B code `citations` 40, not 41).

At shipping:

- [x] AC-12 `docs/canon/import.md` (Tier 2) holds the record model and rules; the import README ≤ 10 020 B points at it, keeps `## Before report`, minors incl. the census heading after `-->` and full-path forms after `/`; `doc_pointers` green; eval README unchanged; 08 §2 "Next" = `import-layout`, 08 < 15 737 B; compacted, AC numbers kept; nextest `-p specengine-import -p specengine-eval`, clippy, fmt green; every named mutation red; fixture diffs only `import-*`; no `Cargo.lock` or `Cargo.toml` diff; `export index && check` clean, worst W ≤ 109 486 B.

## Implementation

Four iterations (the fourth under the owner's session rule); review: iteration 1 changes needed (headings before a field table), 2–4 accept. Workspace 1 324 tests pass (17 skipped: pilot); clippy, fmt clean; every named mutation red. No manifest or lock change.

| Module | What it does |
|---|---|
| import `config.rs`, `lib.rs` | `[documents]` → `DocumentsConfig { id_key, id_path }`, refused at the key's line; re-exported |
| `markdown.rs` | `scan_import`: BOM skipped, comment-opened lines give no heading, fence or table start, inside a table neither row nor end; progress guard; census `scan` unchanged |
| `import/lead_in.rs` | `splits`: ID [separator [title]]; an item's comment continuation before any block test |
| `import/mod.rs` | titled records; `document()` (`id_key`, `id_path`, diagnostics); `document_body`, pub `document_text`; precedence in `settle`; the `id_key` value no token |
| `import/header.rs`, `code.rs`, `model.rs` | outcomes by target; stripped forms refused after `/`; `Form::Document`, `title`, `by_document` |
| eval `import.rs` | the three stdout keys; pointer to README "Before report" |

Tests: import `import_gaps.rs` (AC-01–07); eval `import_gaps_cli.rs` (AC-07–09), `import_cli.rs`, `import_genre.rs`; fixtures `import-one` (YAML `id_key` through `key_map`), `import-two` (field-table key, `id_path`), both `expected.json` regenerated.

Decided in iterations (canon now): the comment rule is import-wide (the scanner cannot tell an item's comment from another; CommonMark agrees), so import section spans may differ from the census; blank lines after a field table are header, headings before it body; the BOM skip; `id_key` with blanks refused, an `id_path` match without group `id` a diagnostic. Known limits: canon "Known limits".

**AC-11**, 2026-10-04, debug, exit 0. A: criteria list records 135 → 1 008 (873 titled), `titled` 1 431, `feature_outside` 5 968 → 4 431, keys kept/unmapped 0/1 929 → 1 796/133 (3 keys without a target), `unclaimed` 427 → 198, all met; duplicates 37 → 64: each new one a newly read titled item restating a register ID (21 question-register rows, 6 others), a corpus fact (`reference_paths` is the owner's call); `unresolved` 8 → 13, `legacy.unmapped` 0 → 7, `empty_text` 0 → 6, all in an archived feature; ~270 lead-in-shaped lines (~60 live) still no record (no separator after the ID, a wrapped span). B: `unclaimed` 178 → 8, duplicates 28 → 10 (pre-existing), hyphenless 269 / 3 222 unchanged, met; 29 document records (26 by path), 17 diagnostics, `by_document` 39; code `citations` 44 → 40: 3 relative `raw/<name>.md` forms (predicted) and an index generator's `../../index.md`, counted before only through `/` before a stripped form, which AC-04 refuses; base-relative resolution is out of scope, so 40 is recorded. Parse files = documents (375, 150); deterministic; pilots' `git status` unchanged.
