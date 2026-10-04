---
class: spec
status: shipped
scope: [crates/specengine-import, crates/specengine-eval, fixtures, docs]
ref: import-layout analysis 2026-10-04, owner's answers Q1-Q9, 1-4, iteration decisions 1-5b; 08 §2 "Next", §3 AC-6, §4.2 item 1
shipped: 2026-10-04
adrs: []
---

# Import layout: the after-tree, read back and compared

## Why

08 §3 AC-6 wants every record carried with its verbatim hash. Round trip: the importer projects the "before" model (`docs/canon/import.md`) into the target layout per corpus config, core reads it back, each definition compared **by identity** (path, line, ID), never by hash; lossless = bytes, not rendering. Product: comparison and debt counts, the tree scratch; AC-6's first half, dry. No ADR: ADR-0026 (`docs/canon/architecture.md#layout`), ADR-0009 (`#ids`), ADR-0008/0031, ADR-0012, ADR-0013 apply; `#apply` is not engaged. Legacy citations stay verbatim, resolved by `aliases_from`.

## Data

Now canon: emission, record files, YAML, headers, emitted configs `docs/canon/import-layout.md`; verifier, attribution, stdout and detail, known limits `docs/canon/import-layout-verifier.md`; `[layout]`, refusals and the layout API `crates/specengine-import/README.md` "Config", "Layout"; `extent`, `written`, `task_box`, `id_cell` and the container cut `docs/canon/import.md` "Record model", "Known limits"; the CLI `crates/specengine-eval/README.md` "CLI contract"; `TYPED_KEYS` `crates/specengine-core/README.md` "Front-matter"; the default layout (was 05 §2) `docs/canon/architecture.md#layout`.

## Acceptance criteria

Fixture ACs (decide "implemented") on invented `fixtures/import-layout/{one,two}`, two conventions (non-Latin from `\u{…}` at test time), each `census.toml` + `[layout]`, `specengine.toml`, `expected.json`:

- [x] AC-01 Each `file` definition (row with fields, headerless row, item with, without title, section) → one record file (a duplicate `-2`); each feature-scoped one → a `{#ID}` section under `<features>`, one document moved by `slug`; a document record in place with `id:`; one residue per walked document; emitted configs load. M: a feature-scoped record as a file → `emitter_findings` ≥ 1.
- [x] AC-02 `hashes` = {`definitions`, 0, 0, 0}; a tamper test unescaping one `\|` on disk → `mismatched` 1. M1: hashing the emitter's output or the model → tamper test red. M2: matching by hash (two empty-text titled records) → `titles.mismatched` ≥ 1. M3: a duplicate dropped → `missing` 1.
- [x] AC-03 Titles (one with inline code, one with words after its `{…}`), fields `mismatched` 0; `task_box.dropped` 0 with `task_box_key`, = boxes without. M1: a field dropped → `fields.mismatched` 1. M2: a title cut at `{` → `titles.mismatched` 1.
- [x] AC-04 Prose between criteria, around a record table, an ID-less sibling item, an emptied table: `prose.mismatched`, `extents.residue` 0. M1: a prose line dropped → 1. M2: an item extent one line longer → ≥ 1.
- [x] AC-05 Every source key in the header (renamed, value mapped, a typed core target plain); field table → YAML; CRLF, BOM copies → equal hashes; `keys_dropped` 0. M1: an unmapped key dropped → 1. M2: a field-table value altered → `fields.mismatched` 1.
- [x] AC-06 Observe `clean`/`observed`; enforce + baseline `clean`, `stale` 0; `emitter_findings` 0; baseline codes ⊆ Q7; a legacy citation resolves; a bare feature-scoped one elsewhere → source `mention-dangling`; `index.files` = `tree.documents`; a tamper test renaming a record file → `file-name` `emitter`. M1: `file-name` source → red. M2: no `aliases_from` → red.
- [x] AC-07 Two runs: identical tree and detail bytes. M: `HashMap` order → red.
- [x] AC-08 `census`, `import` accept `[layout]`; each refusal → exit 2 (at `<config>:<line>`), `--out` empty; `--out` and corpus nested, a symlinked `<out>/layout` → 2, nothing deleted; a record path held by a walked document → `path_taken`. M: `deny_unknown_fields` off `[layout]` → red.
- [x] AC-09 `census` `corpus-mini`, `import` `import-one`, `import-two` = their unchanged `expected.json` and whitelist. M: `extent` on `import` stdout → red.
- [x] AC-10 Genre test (`import_genre.rs` rules) over `src/layout/`, eval `src/layout.rs`: no raw non-ASCII letter or fixture-config literal (fixture configs avoid the code's defaults, codes, file names); each layout rule non-zero in both `expected.json`; stdout = whitelist. M1: a hard-coded class or directory → red. M2: an ID or path on stdout → red.

Owner's check, by an agent on the owner's instruction:

- [x] AC-11 Both pilot configs gain `[layout]`; `layout --label pilot-a|pilot-b` (`#[ignore]` tests, one at a time) exits 0 within `--timeout`; proofs, `git --no-optional-locks status` equal, scratch `HOME` empty, `anonymity` green. `matched` = `before.definitions` less listed reasons; `unexplained`, `prose.mismatched`, `extents.residue`, `emitter_findings` 0; enforce `clean`. Recorded, dated: `tree`, `index`, `reasons`, `dangling` (before A 34 925, B 5 876), `code`, baseline per code, B hyphenless ("(h) pending").

At shipping:

- [x] AC-12 Rules in `docs/canon/import.md` (≤ 12 288 B) or a new Tier 2 canon, the container rule replacing "mismatch by design"; 05 §2 → canon; 08 §2 "Next" `pilot-w`, §3 AC-6 dated; READMEs ≤ 10 020 B; compacted; nextest `-p specengine-import -p specengine-eval`, clippy, fmt, mutations red; no manifest or lock diff; fixtures: only `import-layout`; check clean, worst W ≤ 109 484 B.

## Implementation

Five iterations plus 5b (the fourth under the owner's session rule, the fifth approved by the owner after the first pilot run); review accepted iterations 3 and 4, iterations 1, 2 and 5 were sent back (field-table and moved-heading false greens; YAML values never compared; the budget source of a record file). Every named mutation red; no manifest or lock change.

| Module | What it does |
|---|---|
| core `front_matter.rs`, `lib.rs` | pub `KeyType`, `TYPED_KEYS` (27 keys, the reader's order; `is_typed` reads it), re-exported |
| import `config.rs`, `lib.rs` | `[layout]` → `LayoutConfig` (`deny_unknown_fields`; `Target`, `DocClass`, `CoreKey`, `ClassRule`, `TargetRule`), refusals at the key's line, `check_start`, `target`, `class_of`; `pub mod layout` |
| import `import/model.rs`, `import/mod.rs`, `import/lead_in.rs` | `extent` (serialized last), `written`, `task_box`, `id_cell`, `Field.column`; extents per form; shared field-table helpers |
| import `frontmatter.rs` | entry ranges (key token, scalar), `block()`; census reading unchanged |
| import `layout/mod.rs` (new) | `emit`, `Layout`, the map, `SourceDocument` (`header_unparsed`, `header_not_mapping`), `read_sources`, `SourceBody` / `source_body`, rule S, `.md` renames |
| `layout/body.rs`, `records.rs`, `header.rs` | line destinations, reshaped blocks and their order, residue; record files, reshaped headings, `carried_fields`, `field_keys`, `section_heading`; header normalisation, verbatim carry |
| `layout/typed.rs`, `yaml.rs`, `toml_out.rs`, `scheme.rs` | typed values, reference-list split, `typed_keys`; the YAML and TOML writers; the emitted `specengine.toml` |
| eval `src/layout.rs` (new) | `check_out`, `prepare`, staging, the verifier, attribution, the baseline writer, the index, stdout and detail; `run_tampered` for tamper tests |
| eval `main.rs`, `check.rs`, `harness.rs` | `layout` (`LayoutArgs`: `--scheme`, `--today`), `refusal` pub(crate), `label_of` |

Tests: `fixtures/import-layout/{one,two}`; eval `tests/layout_cli.rs` (AC-01–AC-11, two `#[ignore]` pilot tests, the `TYPED_KEYS` pin), `import_genre.rs` (the layout scan), `src/layout.rs` `tamper_tests`.

**Deliberate deviations from the draft, now canon**: reshaped attributes sit inside the braces (`{#ID key=value}`: pulldown-cmark reads one trailing block); typed values plain only for canonical integers (dates quoted); field-table rows checked by identity (an empty-key row, a third cell are residue); a repeated field-table key carried as `<key>-<n>`, not dropped; a key reaching `id` renamed only when the import read the ID from it and its value is that ID alone (P1); an ID cell with text beyond the ID carried (P2); a header core cannot parse carried verbatim, one it reads as no mapping with nothing added (P5, F-b); a bare feature-scoped mention moved out of its document → `layout` (P4); `budget` source for a residue whose source is over the cap (P6, F-a); the canon is two Tier 2 documents (one would exceed 12 288 B); 08 §2 "Next" is `token-calibration`, then `pilot-w` (the owner's order); 05 §2's examples (§2.1, §2.2) stay in 05, cited from code.

**AC-11**, run 2, 2026-10-04, debug build, deterministic, read-only proofs equal, anonymity green. Pilot configs (outside the repository) gained `[layout]`; owner-settled scheme edits: A a feature-scoped criteria prefix, B one more prefix and `[classes]` requiring `status` — hence `dangling` before A 35 332, B 5 918, not the draft's figures.

- **A**: `matched` 2 242 = 2 249 − 7 (`header_unparseable`: 7 decision document records whose YAML core rejects); unexplained, prose, residue, emitter 0; enforce clean. Tree 1 601 documents, 1 226 record files, 68 feature documents, 998 reshaped, 0 moved; titles 899/0, fields 3 167/0, task boxes 998/0; index 1 601 files, 2 599 nodes, 44 040 links; mention-dangling 35 332 → 4 193 (`layout` 1: a record moved out of its feature document); link-dangling 8 → 8; code citations to moved documents 0. Baseline 5 336: key-missing 4 904, id-width 199, class-unknown 98, id-taken 64, frontmatter-yaml 22, canon-form 18, budget 16, frontmatter-type 9, homoglyph 6. 23.5 s.
- **B**: `matched` 251 = 251; unexplained, prose, emitter 0; enforce clean; `extents.residue` 1, a **corpus fact**: SPEC-015's field-table row holds a code span with two unescaped `|`, so GFM splits it into four cells and cells 3–4 are a third cell's residue (the owner may escape them when migrating). Tree 372 documents, 222 record files, 0 moved, 0 reshaped; index 372 / 372 / 5 341; mention-dangling 5 918 → 0; link-dangling 0 → 0. Baseline 340: key-missing 252, shipped-missing 36, status-invalid 15, budget 14, id-taken 10, frontmatter-yaml 7, canon-form 3, canon-missing 2, class-missing 1. Hyphenless 269 / 3 222 unchanged ("(h) pending"). 4.1 s.

Open before migrating: `#apply` vs one bulk commit; an ADR superseding ADR-0009 for (h) before B; a pilot hitting the last-row comment pulls its canon fix in. Out of scope stays: pilot or `.git` writes, markers, the migration (08 §4), re-padding, translation, YAML repair, W (`pilot-w`).
