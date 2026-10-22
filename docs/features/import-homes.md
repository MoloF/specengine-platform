---
class: spec
status: shipped
scope: [crates/specengine-import, crates/specengine-eval, fixtures]
ref: RM-0013 slice 1; analysis 2026-10-22, decisions owner pre-approved, review open; 08 §2 Phase 5, §3 AC-6, §4.2
shipped: 2026-10-22
adrs: [ADR-0008, ADR-0009, ADR-0012, ADR-0013, ADR-0016, ADR-0026]
---

# import-homes: a home per prefix and inherited header keys (RM-0013, slice 1)

## Why

Pilot A was migrated 2026-10-06, merged 2026-10-07, by a pilot-side script wrapping `specengine-eval layout`: the owner's bulk commit on a separate branch (08 §4.2 rule 6), an owner edit; `#apply` governs SpecEngine's writers. That met 08 §3 AC-6's pilot-A half (2 180 of 2 180 definitions matched, the suite green). The script worked around two importer gaps of pilot A's list: B1, the first definition by path was canonical, so a restatement sorting before the register became `<ID>.md` for 44 IDs (the script hid 54 restatements), its wrong text still matching by hash; B2, record files lacked their class contract's keys (4 904 `key-missing`; the script copied them from the source's header). This slice has SpecEngine's importer do both from config alone; `import-lead-ins` is next (RM-0013). Nothing is written into the pilot.

## Description and interactions

`[definitions] home` and `[layout] inherit`, now canon from `crates/specengine-import/README.md` "Config". `census` validates both and ignores them (frozen); `import` applies `home`; `layout` and `w` (through the emitter) apply both; neither set: output as before. No CLI, MCP, UI, hook, CI, core change or ADR: corpus config (ADR-0008).

## Data

```toml
[definitions]
home = { XQ = ["rolls/**"] }  # Latin prefix -> globs over corpus-relative `/` paths
[layout]
inherit = ["tier", "scope", "owner", "reviewed"]  # tree names, after `key_map`
```

Refused by `census`, `import`, `layout`: `<config>:<line>: <message>`, exit 2, nothing under `--out`; the line is the entry's (in a multi-line list, the offending item's):

| Case | Message |
|---|---|
| key not Latin | `` `definitions.home` key `x1` is not a Latin prefix (an ASCII capital, then ASCII capitals or digits) `` |
| key in `ids.legacy` | `` `definitions.home` key `OLD` is an `ids.legacy` prefix as written: its IDs take the prefix `DEC` `` |
| feature-scoped | `` `definitions.home.CRT` names a home, but `CRT` is one of `ids.feature_prefixes`: its IDs are defined per feature document `` |
| empty list | `` `definitions.home.XQ` is empty `` |
| blank glob | `` `definitions.home.XQ` holds an empty entry `` |
| bad glob | `` `definitions.home.XQ` glob `<glob>`: <error> `` (defensive: cannot fire) |
| blank entry | `` `layout.inherit` holds an empty entry `` |
| padded | `` `layout.inherit` entry ` scope` has blanks around it `` |
| written key | `` `layout.inherit` entry `class` is a key the layout writes itself `` (`id`, `class`, `title`, `aliases`) |
| task key | `` `layout.inherit` entry `done` is `layout.task_box_key` `` |
| repeated | `` `layout.inherit` entry `scope` is repeated `` |
| wrong type | the TOML reader's message |

### Output

- `import` stdout: `references.by_home`, the record-position definitions a home demoted, within `total`; the only new stdout key. stderr adds `, <n> by a home`; `records.json` shows them `role: reference`.
- `layout`: no new stdout key (no `references`); inherited values count in `fields`; `headers.json` `field_values` entries of inherited values add `record`, the record file's tree path. `emission.json` unchanged.

## Rules and edge cases

- `settle`'s order: reference roles (a home document under `reference_paths`, a home table under `reference_headers` define nothing), document precedence (`by_document`), the home, duplicates. A home never demotes a `document` record: a document outside the home defining an ID demotes the home's row, and the home then demotes nothing.
- WHEN an ID has a definition in a home document of its Latin prefix, each of its record-position definitions outside that home SHALL become a `reference` (`by_home` +1), its lines kept in the residue byte for byte; an ID no home document defines keeps its definitions. A glob matching nothing is no error.
- A demoted `{#ID}` section stays in place and core reads it: it counts only in `id-taken` (source, verifier "Attribution" rule 3); `hashes.extra` excuses it, as a `reference_paths` section (owner, 2026-10-22).
- WHEN a record is placed `file`, its file SHALL carry each `inherit` key its source's normalised header holds exactly once, as that header writes the entry, last, in `inherit` order, so core types it as it types the source's.
- Not inherited: a key the source lacks; one the record holds itself (a field, a kept heading attribute wins); anything from a header carried verbatim. Rule S never touches an inherited value.
- The verifier compares the inherited keys of each record file it found with the source's values, as a residue's. An inherited key the source holds is carried (rule 4: lost on disk -> emitter); one carried verbatim takes rule 5 as on a residue.
- Known limit: an inherited YAML alias (`*a`) whose anchor is not inherited breaks the record file's YAML, shown by `header.unparseable` and `hashes.missing`.

## Acceptance criteria

Fixture `fixtures/import-layout/three` (an invented bench-log convention: home `XQ` = `bench/probes.md`; inherit `rev`, `scope`, `owner`, `reviewed`). Mutations, all red in phase B on iteration 1:

- [x] AC-01 — home: the register's row is XQ-11's file body; the earlier restatement stays in its residue as a reference; XQ-90 keeps its lead-in definition; no `-2.md`, no `id-taken`; `hashes.matched` = `before.definitions` = 6; `by_home` 1. M1 `settle_homes` never called; M2 `homed.contains` dropped; M3 list-item reference extents cut; M4 `by_home` never incremented.
- [x] AC-02 — every "Data" refusal at its line, on the three subcommands. M5 the `feature_prefixes` check off.
- [x] AC-03 — inherit: each record file carries its own source's values (an integer, a list, two text keys; a YAML block and a field table), typed as core reads them; `fields` 39 matched, 0 mismatched; `key-missing` 0 in the tree; a tamper test changing one inherited value on disk is one mismatched field; a row's own cell wins. M6 the first document's `inherited` for all; M7 `Verifier::inherited` returns early; M8 the inherited entry written over a cell.
- [x] AC-04 — neither key set: the `import-layout/{one,two}`, `pilot-w/{one,two}`, `corpus-mini` `expected.json` unchanged; `import-{one,two}` add only `"by_home": 0`; the `import_cli.rs` whitelist 50 -> 51.
- [x] CR — iteration 1 accepted (genre scan over the home step and the inherit writer, `three`'s conventions forbidden; output additive only; no pilot literal); iteration 2's delta review accepted, no findings (verbatim condition = `header_values`', no stale cache, same details keys, no change without `inherit`).
- [x] Gates: fmt, clippy (probes too) clean; workspace nextest 3 943/3 944, 71 skipped, the failure the commit guard `census_and_import_outputs_are_unchanged` on the uncommitted `"by_home": 0`, green once committed; UI lint 0, tests 2 546/2 546, build clean; export, check, parity clean; eval quartet (`anonymity`) 86/86; no manifest or lock diff.
- [x] SW-1, SW-2 — this spec; RM-0013, RM-0039, RM-0040; 08 §2, §3 AC-6; the canon and the import README above; `CLAUDE.md` State.
- [ ] OC-1 — the owner's replay, in scratch `git clone`s of pilot A at the commit before its migration commit and at it (no hash in the repository), the script's census config with `home`, `inherit` replacing its two workarounds: `hashes.matched` = `before.definitions`; no `id-taken` but the source ones of demoted sections; no `key-missing` on a record file; `references.by_home` 54; `reasons.unexplained`, `prose.mismatched`, `extents.residue`, `check.emitter_findings` 0; `git diff --no-index --stat` against the migration commit's `docs/` lists only named difference classes; read-only proofs equal; scratch `HOME` empty.

## Out of scope

Writing into pilot A or running on its live tree; a SpecEngine migration writer (`spec init --import`, 07 §2: RM-0014's question); `import-lead-ins`; markers and code citations (ADR-0016); check parity (RM-0039, RM-0040); pilot B (RM-0014); the census.

## Implementation

| Module | What it does |
|---|---|
| import `src/config.rs` | `ImportConfig.home` (`in_home`), `LayoutConfig.inherit`, the refusals |
| import `src/import/{mod,model}.rs` | `settle_homes` after the document precedence; `Import.by_home` |
| import `src/layout/{mod,header,records}.rs` | each header's inherited entries (`named`, `starts`, kept only with `inherit`), written last in its record files; the scheme's `scope` key a serde-named word (no behaviour change) |
| eval `src/import.rs` | `References.by_home`; stderr |
| eval `src/layout.rs` | `Verifier::inherited`, `source_values` (`SourceValue.verbatim_rest`), `FieldValue.record`, rules 4 and 5 |

Tests: import `tests/import_homes.rs` (12), `import_rules.rs` (step order); eval `layout_cli.rs` (slice 1, about 20), `import_cli.rs`, `import_genre.rs`, `import_support/mod.rs`, `layout.rs` `tamper_tests`.

Iteration 1: review accepted. Iteration 2, after the test round, outputs byte-identical: `inherited()` fills `verbatim_keys`, so a value mistyped in the source is attributed `source`, not `emitter`; `named` and `starts` skipped without `inherit`; `source_values` parses each source once (`OnceCell`) with a path-keyed details map.

Deviations: the non-scalar check also requires core's readings equal, stricter than the residue rule (the text after the key); a mismatch it raises is visible, never hidden. The "bad glob" row is kept as defensive: `glob_to_regex` escapes every character, so it cannot fire.

Not covered by a test: rule S on an inherited legacy ID; a tree name held twice; an inherited key lost on disk (working answer `emitter`, rule 4).

Last test: `homes_ac03_a_mistyped_inherited_value_is_the_source_s_frontmatter_type_on_every_carrier` (`rev: [3]` in a residue and two record files: `source` on all three, `emitter_findings` 0, `fields` 39/0); red without the `verbatim_keys` insert. Open: OC-1, the owner's.
