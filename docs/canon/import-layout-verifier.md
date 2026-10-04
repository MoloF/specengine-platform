---
class: canon
tier: 2
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-10-04
---

# Import layout: verifier, attribution, output

The second half of `specengine-eval layout` (eval `src/layout.rs`): the after-tree the emitter wrote (`docs/canon/import-layout.md`) is read back through core and compared with the source, and every finding of the tree's check is attributed. Expected values come from the source alone: `layout::source_body` (the import's reading of a body) and core's parse of the source under the before scheme, never the emitter's output or memory.

## Verifier

Core parses each tree file under the emitted scheme. The map, then the ID, then the ordinal among nodes not yet taken locate a record: in place, a section on its own heading line (rule S applied); reshaped, only among the nodes of reshaped blocks (level L, a line no source heading holds).

- **Hashes**: after and before text, both through `document_text`, BLAKE3; equal → `matched`. A record file's `body` vs `text` (a section's minus its heading line); a reshaped section's `body` vs `text`; in place, a section's `span` or a document's `body` vs `text` with S. **A container** (a document or section record holding records moved out) is compared cut: before minus their extents (the form's text rule), after minus the blocks reshaped in. `extra`: an `id:` / `{#ID}` node no record maps to and none leaves in place (a reference-role section, a definition without a place).
- **Titles**, of titled items and sections moved to files: expected from the source heading bytes by the title rule; read back from front-matter `title` (no first-H1 fallback) or the reshaped heading bytes between the ATX marker and ` {#`.
- **Fields**: each carried cell (an ID cell too) and a moved section's kept heading attributes vs core's read-back under the same key; each residue YAML entry the source holds once, under its tree name, vs core's reading of the source (its `value_map` mapping unless kept as written; S on a document record's `id`; a non-scalar by the text after the key); each field-table value vs its cell or mapping (a repeated key under `<key>-<n>`). Reference and reference-list keys are read back as written (span text, else the decoded value: `X-1@2`, `slug/ID`, `#section`, a path `canon` with its anchor); lists compare as multisets, empty items dropped; one tree item equal to the whole value matches when the source scalar holds a real escape. Attributes dropped as header conflicts are neither compared nor residue. Detail: `headers.json` (`field_values`, form `yaml` · `field-table`).
- **Task box**: the source state; `dropped` when no `task_box_key` is set or its value does not read back (then also a miss).
- **Header**: `documents`, residues with core front-matter; `unparseable`, residues and record files whose front-matter fails (their values are not compared); `keys_dropped`, a source key of a parseable header found neither under its tree name nor as written.
- **Prose**: per document, the residue minus reshaped blocks vs the source body minus moved extents, S applied.
- **Extent residue**: a letter or digit left once the carried parts are cut. A row by whole cells (split at unescaped `|`; a carried ID cell whole); a field table, one extent at its header line, per row by its carried key and value (an empty-key row, a third cell stay); an item or section by text, written ID, title, box, configured separators; a moved section's heading line by title, written ID, kept attributes; an opening ordered-list marker ignored. `extents.total` counts each field table once.
- **Reasons**, per miss: the emitter's reason, else `header_unparseable` (the record file's or document record's front-matter fails), else `reader_boundary` (scanner and core heading lines differ between the scanner headings around the extent; core's: slug anchors and ID sections of the source), else `unexplained`.
- **Dangling**: `mentions` = `mention-dangling`; `links` = `link-dangling` + `link-anchor`, the after count plus links core never reports that named a walked document from the source and reach no tree file now (a target outside core's walk, a renamed or moved document; `findings.json` `left_tree`).

## Attribution

Each tree finding is `source`, `layout` or `emitter`; its source document comes from the map. **The value rule**: a value is the corpus's when the layout does not write the key itself, the file carries the key from the corpus, and the verifier compared it and found it alike — positive proof; a value never compared is not the corpus's. First match wins; a rule naming only when a finding is source gives emitter otherwise:

1. The finding's key (the subject's, else its line's: `canon-missing`, `shipped-missing` sit on `status`) read back rewritten → emitter.
2. `file-name`, `id-scope`, `index-*` → emitter.
3. Source: `id-width` when the model ID's digits ≠ the prefix `width`; `homoglyph` when the subject as written is in the source document; `id-taken`, `duplicate-id` for a model duplicate or a reference-role section; `class-unknown` always.
4. `key-missing`, `canon-missing`, `shipped-missing`: an absent key → source unless the layout writes it (a record file's `id`, `class`; a document record's `id`; `class` under a `classes` glob), added it or carried it; a present one → the value rule. `class-missing` → source unless the layout writes `class` there (a record file, a glob, a mapped source class) or the corpus carried one.
5. `frontmatter-type` of a typed key → source when the value rule holds and the value is carried verbatim (the source entry multi-line or opening `|` `>` `!` `&` `*` `[` `{`) or core's kept value does not parse as the type (`2.0` under an integer key); otherwise rule 11.
6. `unparsed-reference` → source when the before check has it or the value rule holds for its line's key. `canon-form`, `canon-file`, `canon-anchor`: the before check's → source; the value rule failing → emitter; a target the layout moved (an anchor: a section cut from it) → layout; else source.
7. `ref-dangling`, `mention-dangling` → source when the (legacy-mapped) subject has no definition the verifier found, or is feature-scoped and cited outside its document; a bare feature-scoped mention in a record file that left that document → layout; else emitter.
8. `unknown-key` → source for a carried or field key (`<key>-<n>` included) or the task key.
9. `budget` → source when the before check has it, or a residue's source document already exceeds the slot's cap (a record file: only the former).
10. `link-dangling`, `link-anchor`: the before check's → source; the citing file moved (record files included), the target moved (resolved from the source path, not `link_base`) or, for an anchor, a section was cut from it → layout; else emitter.
11. `frontmatter-*`, `key-extra` → source only on a carried header (not a record file) with the before check's finding, `key-extra` also on a carried key the layout did not add; else emitter.
12. Any other code → source when the before check has (code, subject) on the source document, or the finding lies in its subject key's entry under the value rule (a contract's or check rule's value: `value-invalid`, `tier-invalid`); else emitter.

The baseline takes the source-caused errors; `layout` findings stay errors and block enforce.

## Output

stdout `{measurement, label, versions, wall_ms, result}`, counts only; `result` is exactly: `before{documents, definitions, duplicates, hyphenless{definitions, mentions}}`, `tree{documents, record_files, feature_documents, moved, reshaped}`, `hashes{matched, mismatched, missing, extra}`, `titles`, `fields` (each `{matched, mismatched}`), `task_box{carried, dropped}`, `prose{documents, mismatched}`, `extents{total, residue}`, `header{documents, unparseable, keys_dropped, conflicts, last_row_comment}`, `reasons{prefix_unknown, slug, path_taken, section_fields, header_unparseable, reader_boundary, unexplained}` (all seven always), `check{observe, enforce, stale, emitter_findings, findings{<code>{before, source, layout, emitter}}, baseline{entries, per_code{<code>}}}` (non-zero codes), `dangling{mentions, links}` (each `{before, after}`), `index{files, nodes, links, full_ms}`, `code{moved_cited, citations_to_moved}`, `detail{diagnostics, layout_ms}` (emit + write). Under `<out>/layout/<label>/`: `tree/`, `index.db`, and `emission`, `records`, `prose`, `extents`, `headers`, `findings` (attributed), `diagnostics` (`.json`).

## Known limits

- The comparison is type-blind (an integer or bool as text; a one-item list equals a scalar): a type-only emitter fault passes as proven; `headers.json` shows `2.0` as `2`.
- Unparseable headers are not compared: an emitter rewrite that leaves one unparseable is invisible.
- A single-line scalar followed by an indented comment line counts multi-line (verbatim); the escape test reads a trailing comment too; `keys_dropped` counts a carried null typed key (core skips it).
- An empty corpus `class:` (null) counts carried → `class-missing` emitter (the safe direction).
- A corpus `slug/ID` reference to a missing feature path → emitter; layout-caused `canon-file`, `canon-anchor` are not baselined.
- A layout-attributed mention checks the definition across all features (a loss still counts as a hash miss).
- Empty texts share BLAKE3(""): identity matching and titles tell such records apart.
