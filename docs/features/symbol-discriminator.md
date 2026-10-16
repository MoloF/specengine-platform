---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 slice 6a-1, split from symbol-index 2026-10-13 (WA-9, WA-10; Q3, Q9, Q11, Q12); refreshed 2026-10-15 (Q19-Q22); 05 s5.1, s5.2; 08 s2; Rust 1 iteration, review accepted, tests 1 round; S1-S4 owner pre-approved 2026-10-16
shipped: 2026-10-16
adrs: []
---

# symbol-discriminator: in-file twins named apart, display signatures

## Why

Two items of one file that render one qpath (adjacent `impl Tally` blocks, `cfg` twins) were `ambiguous duplicate`: a marker on either was never locked (`spec-lock.md` "Lockability"), and verify read it `cannot_verify ambiguous_symbol`. `code-a`'s two `impl Tally` blocks in `counter.rs` are such a pair. Nothing rendered an item's signature for a read either (05 s5.1).

Slice 6a is split (Q12). **6a-1** (this) names in-file twins apart with a discriminator in the qpath and adds the display-signature entry, both in `specengine-code`, with the goldens and canon lines of shipped slices 1, 3 and 4a that change. **6a-2** `symbol-index.md` stores the rows, and **6a-3** `symbol-reads.md` serves the reads (Q23). Listed (08 s2) 5a-1 -> 5a-1b -> 5a-2 -> 6a-1 -> 6a-2 -> 5b: before 5b and 5c, because twins become lockable (Q11). No ADR: ADR-0020 (a qpath is only a name) and ADR-0008 applied.

## Description and interactions

Canon: `docs/canon/symbol-names.md` (all of it); `code-identity.md` "Units and `qpath`" (Rendering; Ambiguity a pointer), "Eval outputs", "Open"; `code-bindings.md` "Rust binding"; `spec-cli-refs.md` "Rows"; `spec-lock.md` "Lockability"; `spec-verify.md` "States"; the code README's `items`, `naming`, `bind` rows; 05 s5.1, s5.2 pointers.

`spec refs` and `spec lock accept` print `item crates/tally::counter::impl Tally #1 impl_item` where they printed `… impl Tally impl_item ambiguous duplicate`, and such a marker is lockable. `spec verify` keys it by the discriminated qpath on every side (the disk, `--changed`, `--staged`, and the one-commit seam's `tree_of_blobs`), so `spec propose drift`'s rows follow. `item_signatures` is called by no command until 6a-2. No CLI source, `Cargo.toml`, hash byte or lock entry changes. The eval keeps its own undiscriminated `duplicate`, so `fixtures/ast-hash/expected.json` is byte-identical.

## Data

### Discriminator

`docs/canon/symbol-names.md` "Discriminator", "Ambiguity" (WA-9, Q3, Q19-Q21, S1, S3, S4).

### Signature

`docs/canon/symbol-names.md` "Signature" (WA-10, Q9, S2).

### Goldens and canon that move

- `fixtures/code-a/expected.json`: the `counter.rs:22:17` and `:45:6` rows read `item crates/tally::counter::impl Tally #1 impl_item` and `#2`, without `ambiguous duplicate`; their `rule` and `mutation` are kept. The RON `root.step ambiguous duplicate` rows and LAW-STEP's `refs_stdout_example` do not change.
- CLI tests: `code_refs.rs` (the `22:17` row); `lock_accept.rs` (`… impl Tally #1 impl_item | predated`, then `#2 … | bad_rev`); `verify_states.rs` (two `impl Tally` rows, golden counts rows 39, fix 19, cannot 6; `crlf_tree_verifies_as_lf` rows 38 -> 39; the `ambiguous_symbol duplicate` case moves to a cross-file pair, `crates/tally/src/pair.rs` beside `pair/mod.rs`); `code_refs_walk.rs` (`NEW_MODULES` names `items.rs`); `drift_approve.rs::drift_review_has_no_preview` (LAW-BOUND's summary now holds `predated, bad_marker bad_rev` and passes the 80-character inbox cut, so the inbox line is compared cut).
- Unchanged: the other 4c-1 drift tests, `fixtures/spec-lock/code-a.lock`, `fixtures/ast-hash/expected.json`.

## Rules and edge cases

Determinism, frozen names, ordinal shifts and the drift re-key: `docs/canon/symbol-names.md` "Frozen names", "Known limits". Genre (`docs/canon/architecture.md#universal`): Rust syntax stays in `specengine-code`; `bind.rs`, `naming.rs` and `items.rs` are in `code_refs_walk.rs` `NEW_MODULES`.

## Acceptance criteria

rust-developer:

- [x] RD-1 -- discriminator. M: a unique qpath discriminated -> `unique_qpath_is_unchanged`; the ordinal tried first -> `cfg_twins_are_named_by_cfg`; cross-file twins discriminated -> `cross_file_twins_stay_duplicate`; in-file twins kept ambiguous -> `marked_adjacent_impl_is_lockable`; a RON path discriminated -> `ron_paths_are_never_discriminated`; an enclosing mod's `cfg` ignored -> `items_in_cfg_twin_mods_take_the_mods_cfg`; rule 1 taken with a member lacking `cfg` -> `twins_with_a_cfg_less_member_take_ordinals`; ordinals off source order -> `ordinals_follow_source_order`; a comment inside `cfg(..)` kept -> `cfg_comment_edit_keeps_the_name` (Q20); an undiscriminated locked key matched to `#1` once a twin appears -> `twin_added_after_lock_breaks_the_entry` (CLI); a shifted key matched by ordinal fallback -> `inserted_twin_never_reads_all_ok` (CLI); the halves diverging from `name_files` -> `name_files_equals_its_halves` (Q21).
- [x] RD-2 -- signature. M: the body kept -> `signature_stops_at_the_body`; comments kept -> `signature_drops_comments`; broken text rendered -> `broken_item_has_no_signature`; a byte cut -> `long_signature_is_cut_at_a_char_boundary`; CR kept -> `signature_is_equal_under_lf_and_crlf`; `=` kept -> `const_signature_drops_the_value`; the macro body kept -> `macro_signature_ends_at_its_name`; the order asked ignored, or an index past its items answered -> `item_signatures_follow_the_indexes_asked`.
- [x] RD-3 -- seams. M: a new dependency -> `code_crate_depends_on_exactly_the_parser_and_the_hash`; a stack word or Cargo literal in `bind.rs`, `naming.rs` or `items.rs` ("Rules and edge cases", Genre) -> `code_refs_walk.rs`'s genre scan; an item's `state` read -> `nothing_outside_the_binder_reads_an_items_state`.

test-engineer:

- [x] TE-1 -- goldens of slices 1, 3, 4a as "Goldens and canon that move" lists; every other golden stays byte-identical, `fixtures/ast-hash/expected.json`, the golden lock and 4c-1's other drift tests included (M: a unique qpath discriminated turns `code_refs.rs` red).
- [x] TE-2 -- `specengine-code` tests over inline sources: cfg twins (fn, impl, inline mods with `//!` inside); ordinal twins with one member lacking `cfg`; `cfg` on an enclosing impl; twins in an unrooted file; a cross-file pair; a signature for every `BODY_FIELDS` row, the cases looked up from the exported table so a row without one fails (M: a kind left out -> `signature_covers_every_body_field_row`), a multi-line `where`, and a 600-byte signature with a multi-byte char at the cut; a `cfg` holding a comment and a string with spaces (S1: the string kept verbatim).
- [x] TE-3 -- determinism: LF and CRLF copies, two roots: equal `spec refs` output, names and signatures.

spec-writer:

- [x] SW-1 -- this spec (split from `symbol-index.md` 2026-10-13); 08 s2 lists 6a-1.
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/symbol-names.md`; `code-identity.md` pointers ("Units and `qpath`", "Eval outputs", "Measured", "Open" without the `disambiguator`); `code-bindings.md` "Rust binding"; `spec-cli-refs.md` "Rows"; `spec-lock.md` "Lockability"; `spec-verify.md` "States"; the code README's `items`, `naming`, `bind` rows; 05 s5.1, s5.2 pointers; `docs/index-canon.md` re-measured; 08 s2; `CLAUDE.md` State; `symbol-index.md`'s seams and carry-overs.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

Stored rows, refs from the index (6a-2 `symbol-index.md`); lookup, `spec symbols`, `unmapped` (6a-3 `symbol-reads.md`). Discriminating cross-file twins, RON paths or file modules. The eval's `duplicate` count. Printing the signature (6a-3, 7a). A content-derived impl discriminator (Q22).

## Open

Decided under the owner's pre-approval: Q3 a discriminator in the qpath; Q9 a signature without attributes or fields; Q11 6a after 5a-2, before 5b; Q12 the 6a-1/6a-2 split; Q19 the `cfg` chain on `ItemRecord`, recorded by the shared collect walk; Q20 (flagged) comments in `cfg(..)` dropped; Q21 `name_files`' halves shipped here; Q22 (flagged) ordinals kept, revisited from 6a-2's TE-8 counts before a pilot locks a twin. S1-S4 (2026-10-16, the readings of review 1 and the tests): `symbol-names.md`. Q-numbers are shared with `symbol-index.md` and `symbol-reads.md`.

## Implementation

| Module (code `src/`) | What it does |
|---|---|
| `items.rs` | `ItemRecord.cfg`; `Scope.cfg` carries the enclosing mods', impl's or trait's chain; `cfg_predicates` reads an attached `#[cfg(…)]` token tree iteratively |
| `naming.rs` | `Name.discriminator`, `Name::undiscriminated`, `FileNames`; `name_file` (per file: `name_one`, `discriminate`); `tree_passes` (`path_targets`, `path_attribute`, a cross-file `mark_duplicates`); `name_files` their composition; `DISCRIMINATOR_SEPARATOR`; module doc rewritten |
| `bind.rs` | `item_signatures`, `SIGNATURE_CAP_BYTES`; private `signature`, `cap_signature` |
| `lib.rs` | root re-exports; the `naming` line says another file; a 6a-1 module-doc section |
| `qpath.rs` | docs only: `Ambiguity::Duplicate` and `qpath()` say another file |

Deviations, accepted in review 1: D1 (owner S1) a `cfg` string literal is kept verbatim, a CR before an LF dropped, where the spec said whitespace removed; D2 public items the spec did not name: `FileNames.shared`, `Name::undiscriminated`, `path_targets`, `DISCRIMINATOR_SEPARATOR`; D3 code comments cite this spec's "Discriminator" and "Signature", kept here as pointers until 6a-2 re-points them to `symbol-names.md`.

Owner's answers (pre-approved): S1 a predicate's bytes; S2 a comment is whitespace in a signature, trailing commas stay; S3 an empty `cfg()` unrecorded, `cfg_attr`, inner, bracketed and absolute `cfg` ignored, a group keyed by qpath whatever the kind; S4 readers never split a qpath. Review 1's nits were done in 6a-2 (`symbol-index.md` "Implementation").

Developer's run: `specengine-code` green; CLI red only on the 10 pins above before the test edits; http, eval (`doc_pointers`, `build_graph`), core, mcp green; fmt, clippy clean.

Tests, 1 round: code + CLI 1399/1399; http, mcp, eval 598/600 (`import_cli`, `layout_cli` red only until `fixtures/code-a/expected.json` is committed); mutations 9/9 killed (M1-M8, M6b); new: `symbol_discriminator` 18, `symbol_signature` 12, `symbol_halves` 2, `symbol_twins` 7, `verify_states` 1.
