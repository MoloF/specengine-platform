---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-code]
ref: Phase 3 slice 5b-1b, split from lock-levels 2026-10-20 (Q22); refreshed 2026-10-20 against 7ecc9f6 (S1-S14); Q5, Q6, Q13, Q20, Q25, Q26, Q42-Q47 decided under the owner's pre-approval, review open (their alternatives in this file at c343910); 05 s5.3; 08 AC-2, AC-14; developer 1 iteration, review 1 accepted
shipped: 2026-10-20
adrs: [ADR-0050]
---

# lock-levels-verify (5b-1b): verify by levels

## Why

5b-1a (`lock-levels.md`) records each entry's `levels` and every level's digest, but verify still judged as v2 did: an item compared `sig` and `body` whatever its levels, so a `[sig]` marker read a body edit as drift; a `module` or `ron` row compared its file's blob, so any edit of a RON file or a module's file was drift for everything bound in it (05:135: the levels exist "so a refactor is not an alarm the owner learns to ignore"). This slice makes the declared levels count. It changes no lock byte: every digest it compares is already in format 2.

Delivered: verify judging every kind by levels, C = (L_lock ∪ L_mark) ∖ `path`; `code_changed <levels>` and `levels_changed <from>-><to>`; `module` and `ron` judged by digest; JSON `levels`; drift items storing verify's own rows; a row's `code` per changed level, never a blob OID; 4a's CRLF limit lifted; 6b-1's, 5b-1a's and 5b-2's review carry-overs (RD-9). Order (08 s2): after 5b-1a and 5b-2 (`lock-rebase.md`), before 5c (`anchor-states.md`).

## Description and interactions

As built, in canon: `docs/canon/spec-verify.md` "States", "Cascade", "Known limits"; `spec-verify-output.md` "Text", "JSON"; `proposal-drift-item.md` "The item"; `proposal-drift.md` "Known limits"; `code-hash-ron.md`; `architecture-code.md#verify`; `node-revision.md` "Raising" (RD-9 (a)); `spec-lock-rebase.md` "Steps" (k); the code README's `naming` (`dir_text`). Unchanged: every lock byte, its readers, accept and rebase; the queue's schema, store, MCP, HTTP, UI, plugin, the gate; no new crate, dependency or ADR (ADR-0050; under ADR-0042 a binding verify cannot verify exits 2). Nothing is blocked (ADR-0012): the new exit 2 stops only a project's own hook.

## Data

Pointers kept for the citations into this spec.

### Level-aware verify

`spec-verify.md` "States" (L_lock, L_mark, C; `code_changed`, `levels_changed`; `compared`), "Cascade" (steps 1-4, every kind); **JSON**: `spec-verify-output.md` "JSON" (`levels`, the 16th key). Q42 narrowed at shipping (owner pre-approved, review open): a `[path]`-only binding's state, category and detail never move; its `compared` reads `blob` while the file is unchanged, null after an edit (step 2 before step 3, as written; no code change). The developer's readings, accepted by the review: `levels.changed` and `code` are set only when `compared` is `ast`; `raised` and `conflict` carry no detail.

### Drift evidence

`proposal-drift-item.md` "The item" (`detail`, `code`; the stored rows `VerifyRow`'s own serialization, `levels` included; **Store**: core `DriftRow` the read view, `levels` never required); `proposal-drift.md` "Known limits" (items re-keyed once).

### The pin

CLI `code_refs_walk.rs` `nothing_outside_the_binder_reads_an_items_state`: `verify_rows.rs` joins with `item_levels(`, `module_levels(`; `hash_item(`, `hash_node(`, `hash_bound_items(` banned; `HashState` and `item.state` allowed in that file alone (Q26).

### Carry-overs

RD-9 (a)-(n), from 6b-1's, 5b-1a's and 5b-2's reviews (Q47), as listed in this file at c343910: all shipped, none returned to "Later"; (c), (e), (f), (k) as "Implementation" says.

## Rules and edge cases

- WHEN a marker narrows its levels, verify SHALL compare the union with the lock's (Q5).
- WHEN both sides are `[path]`-only, no edit SHALL change the row's state, category or detail, and its file SHALL NOT be parsed (Q42, narrowed at shipping).
- WHEN a marker's level list is `Invalid`, its row SHALL NOT be compared and SHALL read `bad_marker bad_levels` unless 4a's order decides it earlier (Q43).
- WHEN only a sibling RON value or a neighbour item changes, an item's, an inline `mod`'s or a RON value's row SHALL read `ok`; a file module with `body` in C SHALL read `code_changed body`.
- WHEN only a fingerprint or `body_bytes` differs, no verify row or `drift_hash` SHALL change (`lock-levels.md` Q12).
- WHEN run from two roots, another cwd or over the lock's entries reversed, verify SHALL give byte-identical stdout, stderr and JSON; under `core.autocrlf=true`, LF's states, categories and details, `compared` `ast`.
- Core sees level names and digest strings only, never tree-sitter (`build_graph.rs` `core_and_store_have_no_tree_sitter`); no stack, kind or prefix in rows or texts (ADR-0008, Q46).

## Acceptance criteria

rust-developer (M6-M8; M1-M5 are 5b-1a's; cap 3):

- [x] RD-5 -- "Level-aware verify". Core `tests/verify_judge.rs`, pure cells over core's comparison: a narrowed marker with a body edit is `code_changed body`; a widened marker is `levels_changed`; C = ∅ compares nothing; blob equal and L_lock ≠ L_mark is `levels_changed` by `blob`. New CLI `tests/verify_levels.rs`: `narrowed_marker_with_a_body_edit_is_code_changed`, `widened_marker_is_levels_changed`, `path_only_binding_ignores_a_body_edit`, `path_only_module_ignores_a_parse_error_elsewhere`, `sig_marker_ignores_a_body_edit`. **M6** `sig` and `body` compared whatever the levels -> `sig_marker_ignores_a_body_edit`, in the CLI test and its core cell.
- [x] RD-6 -- `module` and `ron` by digest. `verify_cannot.rs` through `exits_2_everywhere`: `module_file_error_elsewhere_cannot_verify`, `broken_ron_file_binding_cannot_verify` (detail its first category). `verify_cascade.rs` `ron_sibling_edit_is_ok` replaces `blob_only_binding_change_is_code_ahead`: the RON row `ok` by `ast`, the module row `code_ahead code_changed body` by `ast`, its text lines and `cascade` re-derived. **M7** RON judged by blob -> `ron_sibling_edit_is_ok`.
- [x] RD-7 -- "Drift evidence", CLI `drift_raise.rs`: `assert_item` compares `DriftItem.stored_rows` (store `queue.rs:673`) with verify's JSON rows, `levels` included; `item_code_names_its_changed_levels` and `regen_drift_hash` carry the detail. Green without edits (Q44): core `lock_model.rs` `lock_code_has_no_fingerprint`, core `drift_stored.rs`, store `queue_drift.rs`, `queue_follow_up.rs`, MCP `mcp_drift.rs` (`NULL_ROW`), CLI `health_rows`. Corrected at shipping: Q44 cannot leave `drift_approve.rs` green, so `drift_review_has_no_preview` (stored rows carry `levels`, the typed view none) and `review_gives_the_rows_as_stored` (CAP's detail `body`; a later build's key other than `levels`) are edited. **M8** a blob OID as a module's or RON row's `code` -> `module_neighbour_edit_keeps_the_drift_hash`: a drifted `[sig]` module, then a body edit; a drifted RON value, then a sibling edit; each raised again reads `same evidence`.
- [x] RD-8 -- "The pin": the edited pin green over `verify_rows.rs`; no mutation run.
- [x] RD-9 -- "Carry-overs" (a)-(n), in iteration 1, no mutation run: the review found each as listed; CLI `rev_bump.rs` `required_hint_without_a_line_names_only_editorial` pins (a)'s whole text; eval `doc_pointers.rs` green; `code_format_history.txt` gains no line; clippy clean.

test-engineer (temp repos over `verify-ecs` and code-a's golden; no fixture edited):

- [x] TE-5 -- the matrix in `verify_levels.rs`, each cell pinning state, category, detail, `compared`, `levels` and, where not `ok`, `code` through the raised item's `drift_hash` (`regen_drift_hash`'s way). Markers: default, `[sig]`, `[body]`, `[deps]`, `[path]`, two on one key, an inline `mod`, an `Invalid` list. Edits: signature, body, a call added, a comment, rustfmt `max_width = 60`, a neighbour, a marker narrowed, widened. RON: the value vs a sibling; `[deps]`. Module: an inner attribute vs its body.
- [x] TE-6 -- AC-14 for `module` and `ron` (`verify_cannot.rs`): exit 2 in every mode, plain and `--changed`; the RON error after the marked comment, so its anchor stays `Path`.
- [x] TE-7 -- `verify_output.rs` `verify_is_byte_identical_across_roots_and_order` gains a levels scenario (a narrowed marker, a `levels_changed` row) over the lock's entries reversed: byte-identical. CRLF inverted: `verify_cascade.rs` `autocrlf_items_module_and_ron_verify_as_lf`, `verify_states.rs` `lf_lock_over_a_crlf_tree_hashes_items_equal` and (added at shipping) `verify_staged.rs` `verify_staged_reads_clean_blobs_under_autocrlf`, plain half, assert LF's states, categories and details, `compared` `ast`.

spec-writer:

- [x] SW-1 -- this spec, split 2026-10-20 (Q22); refreshed 2026-10-20 against 7ecc9f6 (S1-S14, Q42-Q47, RD-9); `export index`, `check` clean.
- [x] SW-3 -- at shipping (2026-10-20), bytes before -> after (Tier 2 cap 12 288): `spec-verify.md` 11 961 -> 11 958 ("States", "Cascade" steps 1-4, "Known limits"; "Base" and "One commit" tightened to fit); `spec-verify-output.md` 9 204 -> 9 603; `proposal-drift-item.md` 4 339 -> 4 458; `proposal-drift.md` 8 794 -> 8 944; `architecture-code.md` 11 916 -> 11 956; `code-hash-ron.md` 4 324 -> 4 303; `node-revision.md` 12 283 -> 12 282; `spec-lock-rebase.md` 4 604 -> 4 554 (its panic text in "Steps"); the code README 9 883 -> 9 989 (`dir_text`); 05:135, s5.3; 08 s2, AC-2, AC-14; `CLAUDE.md` State; `anchor-states.md`, `lock-levels.md` re-pointed; this spec compacted; no new ADR.

## Out of scope

Any lock byte (5b-1a); `spec lock rebase` (5b-2); 5c's anchors and `path` (`anchor-states.md`); `--deep`; reads' `sync` (7a); Health's drift row (9b); an MCP tool, UI or plugin command.

## Later

The review's nits, closed by `hygiene-pass.md` (RM-0029, shipped 2026-10-21). Done: RON pairing is one helper, `lock.rs` `paired_value`, for accept, verify and rebase (the first match kept); `verify_rows.rs`' file-module arm lists `Target::Module { item: None, .. }`, `Ron`, `Keyless`, no `_`; core `LevelDigests`' fields documented; code `lib.rs`' `dir_text` doc current; the tests name `dir_text`, not `package_label`, and old rows carry `VerifyLevels`' object, not an array `levels`. Already gone: the UI mock labels its 15 keys `ROW_KEYS` (older) beside `ROW_KEYS_17`. Left: `level_code`'s two fingerprints never compared (accept's digests by construction); a range-keyed RON map (speed only; it would change first-match).

## Implementation

| Module | What it does |
|---|---|
| core `src/verify.rs` | `RowLevels` (C, `differ`, `change`), `LevelDigests`, pure `changed_levels`, `LEVELS_CHANGED`; `Comparison::{Blob, PathOnly, Digests, CannotVerify, NotHashed}`; the judge maps it to a category and detail, comparing no hash |
| core `src/drift.rs`, `src/lock.rs` | `HashedRow.code` doc; RD-9 (e) `debug_assert!`s, (l) |
| CLI `src/verify_rows.rs` | the cascade (`Unit` per kind, `made`, `digests_of`): each changed file parsed once, digests by `lock.rs` `level_code`; `Row.levels`, `code` on `ast` rows; (g), (h) |
| CLI `src/verify.rs`, `src/lib.rs` | `VerifyRow.levels` (`VerifyLevels`, exported); (d) |
| CLI `src/drift.rs` | `StoredEvidence`: verify's rows stored as serialized, replacing `drift_row()` |
| CLI `src/bump.rs` | (a) the editorial-only hint |
| code `src/naming.rs`, `src/lib.rs`; CLI `src/code_index.rs` | (b) `dir_text`, one function for both |
| CLI `src/{symbols,revisions,lock,lock_rebase}.rs`; code `src/markers.rs` | (c), (f), (i), (j), (k), (m), (n) |
| tests | new CLI `verify_levels.rs`; `verify_common/mod.rs`, `verify_{cannot,cascade,modes,output,staged,states}.rs`, `symbol_twins.rs`, `drift_{raise,approve}.rs`, `code_refs_walk.rs`, `rev_bump.rs`; core `verify_judge.rs` |

Deviations: (c) cites ``canon `symbol-reads`, "Lookup", "spec symbols"``: a CLI source may not contain `docs/` (`genre.rs`, AC-17), and `doc_pointers.rs` reads that form as `docs/canon/symbol-reads.md`. (e) the format assert sits in `render_lock` (`render_block` cannot see the format). (f) the arm commented at its invariant, not dropped. (k) the text option, with every rebase refusal's `; nothing written`; the no-tree text unchanged. Spec errors fixed at shipping: RD-7's `drift_approve.rs`, TE-7's `verify_staged.rs`; Q42 narrowed ("Level-aware verify"). A3 confirmed: `verify_modes.rs` `check` 1.

Tests: workspace nextest 3 437 passed, 46 skipped; eval `doc_pointers`, `ui_policy`, `anonymity`, `build_graph` 84 passed; parity 3 passed; fmt, clippy `--workspace --all-targets -D warnings` clean. New CLI `verify_levels.rs` 17 tests (TE-5: 7 bound units × 8 edits, RON and module cases; RD-5's cells; Q43); core `verify_judge.rs` +8 pure cells. Mutations 3 of 3 killed: M6 by core and CLI `sig_marker_ignores_a_body_edit`; M7 by `ron_sibling_edit_is_ok`; M8 by `module_neighbour_edit_keeps_the_drift_hash` and `verify_levels`' RON and module drift-hash cells. Unpinned: step 2 with L_lock ≠ L_mark through an alias, a core cell only (no fixture offers it unedited).
