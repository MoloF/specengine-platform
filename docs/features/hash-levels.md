---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5a-1 (WA-2..WA-5, WA-12, WA-14; Q1-Q16 decided under the owner's pre-approval; split 2026-10-13, the RON value split off as 5a-1b 2026-10-15); 05 s5.2; 08 s2, AC-14, AC-15; Rust 2 iterations, review accepted, tests 2 rounds, H1-H7 2026-10-16
shipped: 2026-10-16
adrs: []
---

# hash-levels: per-level hashes and fingerprints of a bound item

## Why

A format-1 `spec.lock` entry holds one `ast_hash` of the whole item, levels ignored (`docs/canon/spec-lock.md` "Entry"): a `[sig]` marker reads a body edit as drift; `module` and `ron` bindings are judged by `file_blob` alone, so any byte of their file and an autocrlf checkout stale them ("Lockability"; `spec-verify.md` "Known limits"); a renamed function is `broken_binding symbol_gone` plus `unlocked` (`anchor-states.md` "Why"). Levels (`sig`, `body`, `deps`, `path`) and three fingerprints (`skeleton`, `quote`, `body_bytes`) answer all three (a `ron` binding by its value hash, 5a-1b `hash-levels-ron.md`). Once a lock records them, their bytes change only with a `LEVEL_RECIPE` bump and a `spec lock rebase` of every lock (ADR-0050): so they are fixed and measured before anything writes them.

Slice 5 is split (Q1): 5a, 5b `lock-levels`, 5c `anchor-states`; 5a again (Q10, Q12): **5a-1** (this), the Rust levels and fingerprints in `specengine-code` with their fixtures and pins; **5a-1b** `hash-levels-ron.md`, the RON value hash; **5a-2** `hash-levels-eval.md`, the eval reports and pilot figures (08 AC-2, AC-10). No CLI, lock, verify or ADR change.

## Description and interactions

Canon: `docs/canon/code-hash.md` (all of it; v2's detail moved down from the code README, whose "Hash recipe" is now the pointer); the code README's `levels`, `bind`, `grammar` rows and "Rules"; `code-identity.md` "Open"; 05 s5.2 (levels and fingerprints a pointer); 08 s2. Nobody sees the digests yet: 5a-2 measures them, 5b-1 records them (`lock-levels.md` "Format 2"), 5c follows by them, 6a-2 stores them, 6a-1 cuts signatures with `body_nodes`. v2 and every format-1 caller (accept, verify's cascade, 4c-1's drift items) are unchanged. All language knowledge stays in `specengine-code` (WA-14, ADR-0008); core, store, CLI untouched; no new dependency.

## Data

### Entry points

`docs/canon/code-hash.md` "Entry points" (Q6, Q13, Q14).

### Levels

`docs/canon/code-hash.md` "Levels": the header and streams (WA-2, WA-12), `walk3` with N5 (Q8, H4), the CR rule (Q9, H5), a `deps` target (Q11, H1).

### The body field

`docs/canon/code-hash.md` "The body field" (Q13). Overrides 05 s5.2, which named the `body` field of three kinds only.

### File module

`docs/canon/code-hash.md` "File module" (WA-3, H2, H3).

### Fingerprints

`docs/canon/code-hash.md` "Fingerprints": triviality (Q3, Q14), broken units (08 AC-14), never a judge (Q16) (WA-5, H5).

## Rules and edge cases

Determinism, linearity, trap 1: `docs/canon/code-hash.md` "Levels", "Recipe v2". No `.state` is read; `code_refs_walk.rs` `nothing_outside_the_binder_reads_an_items_state` still asks cli `lock.rs` for `hash_bound_items(`, 5b-1 widens it to `item_levels(` (`lock-levels.md` Q16). Code comments cite `crates/specengine-code/README.md` "Hash recipe" or a `code-hash.md` heading, never this spec; "05 §5.2" joins `doc_pointers.rs` `STALE` only once that section is pointer-only (after 5c, 6a).

## Acceptance criteria

rust-developer:

- [x] RD-1 -- "Levels" and "The body field" for every `ITEM_KINDS` and `MEMBER_KINDS` kind. M: name walked -> `rename_keeps_sig_body_and_deps`; body inside sig -> `body_edit_leaves_sig`; sig inside body -> `signature_edit_leaves_body`; const value in sig -> `const_value_edit_leaves_sig`; a bodiless `impl … ;` or `static X: T;` given a body -> `bodiless_impl_and_static_hash_an_empty_body`; source order -> `call_order_swap_keeps_deps`; calls of the signature -> `deps_ignore_calls_in_the_signature`; macro arguments read as calls -> `calls_inside_macro_arguments_are_not_deps`; a kind without a row, or a row off the grammar -> `body_field_table_matches_the_grammar` (eval tests, reading `grammar::NODE_TYPES` and `BODY_FIELDS`); a CR kept in a leaf -> `crlf_multiline_literal_keeps_every_level`; a non-path function's text as its target (H1) -> `non_path_call_targets_are_their_kind`.
- [x] RD-2 -- a broken item gets no level and no fingerprint. M: sig hashed beside a body error -> `item_with_a_body_error_has_no_level_hash`; an `is_extra()` filter -> `broken_items_never_share_a_level_hash`.
- [x] RD-3 -- v2 untouched, v3 frozen. M: `RECIPE` bumped -> `hash_v2.rs` `recipe_is_v2_and_heads_the_stream`, `unattributed_use_run_stream_is_pinned`; one header shared -> `level_header_is_v3_and_item_hash_is_v2`; any v3 byte -> `level_streams_are_pinned` (golden bytes per level and fingerprint, one input per kind).
- [x] RD-4 -- file module and N5. M: no N5 -> `reordered_mod_declarations_keep_the_module_hash`; `#[macro_use]` sorted -> `macro_use_mod_order_changes_the_module_hash`; `#[cfg_attr(.., macro_use)]` sorted (H4) -> `cfg_attr_macro_use_breaks_the_run`; inner attributes in body -> `inner_attribute_edit_is_a_module_sig_change`; a shebang ending the run (H2) -> `shebang_keeps_the_module_inner_attribute_run`; trailing whitespace in the module `quote` (H3) -> `module_quote_trims_whitespace_at_both_ends`; a file with an error elsewhere hashed -> `module_of_a_file_with_an_error_cannot_verify`; `extern crate` runs left unsorted -> `reordered_extern_crates_keep_the_module_hash`; a module's `deps` short of the file's calls -> `module_deps_are_every_call_in_the_file`.
- RD-5 -- the RON value: moved to `hash-levels-ron.md` RD-1 (Q12).
- [x] RD-6 -- fingerprints. M: identifier text kept -> `renamed_locals_keep_the_skeleton`; literal text kept -> `changed_literal_keeps_the_skeleton`; no context lines -> `copies_differ_by_quote_when_their_context_differs`; CR kept -> `quote_is_equal_under_lf_and_crlf`; CR counted -> `crlf_literal_keeps_body_bytes`; `body_bytes` from the source span -> `rustfmt_variants_keep_body_bytes`; `<=` for `<` in `is_trivial` -> `body_of_exactly_124_bytes_is_not_trivial`.
- [x] RD-7 -- linear and iterative. M: a recursive walk -> `deep_nesting_does_not_overflow_the_level_walks` (a 256 KiB thread, as `hash_depth.rs`); a quadratic quote -> `quote_cost_is_linear` (best of three, as `marker_levels.rs`).
- [x] RD-8 -- genre and graph. M: a `build`, `Plugin` or `add_systems` literal -> `level_sources_name_no_framework`; a fourth dependency beside tree-sitter, tree-sitter-rust, blake3 -> `build_graph.rs` `code_crate_depends_on_exactly_the_parser_and_the_hash`; tree-sitter in core or store -> `core_and_store_have_no_tree_sitter`.
- [x] RD-9 -- "Entry points". M: the twin's order off the indexes asked -> `level_twin_follows_binding_indexes` (an error-free item, `NotHashed` in the binding, `Hashed` there), and `analyze_tree`'s order differing -> the same test over both analyses; an inline `mod`'s module off item `i`'s entry -> `inline_mod_module_is_its_items_levels`; glued `deps` -> `qualified_call_targets_do_not_glue`; `call_targets` drifting from `deps` -> `deps_are_the_digest_of_call_targets_for_items_and_modules`; the RON entry's test: `hash-levels-ron.md` RD-2.

test-engineer:

- [x] TE-1 -- `fixtures/hash-levels/` with `expected.json`: per level and per kind an equal pair and a different pair, LF and an in-memory CRLF copy for items and file modules (RON values: `hash-levels-ron.md` TE-1), each pair naming its mutation. `hash_levels_fixture.rs` `te1_*` (6 tests).
- [x] TE-2 -- a property test over every item of every `.rs` file of `fixtures/hash-levels/` (the only fixture committing rustfmt variants): levels, `skeleton`, `body_bytes` stable under the committed rustfmt variants (default, `max_width = 60`), comment stripping and a neighbour edit (`#[cfg(all())]` and an LF before every other item's first attached attribute, both parities); each level changes under its own targeted edit. M: a constant hash; a hash that reads a neighbour. `hash_levels_props.rs` `te2_*` (4 tests).
- [x] TE-3 -- AC-14 over TE-1's `expected.json`: every broken item and every broken file's module `CannotVerify` with its categories, no digest shared by two broken units (RD-2 pins the trap in a unit test; this over the fixture). `te3_broken_units_cannot_verify_with_v2_categories_and_share_no_digest`.
- [x] TE-4 -- determinism: two parser instances, two runs, reversed file order (RON: `hash-levels-ron.md` TE-2). `hash_levels_library.rs` `te4_*` (3 tests).
- [x] TE-5 -- AC-15, the library half: a rename keeps `body`, `skeleton`, `sig`, `quote`; a trivial body stays under 124 B in both rustfmt variants; two copies share `skeleton` and `body`. `hash_levels_library.rs` `te5_*` (4 tests).
- [x] TE-6 -- v2 equivalence, 5b's ground for dropping `ast_hash`: over every fixture item and every TE-1, TE-2 edit, `hash_item` changes exactly when `sig` or `body` changes, the N5 and CR cases named as the only exceptions. M: a leaf in neither stream. `te6_v2_changes_exactly_with_sig_or_body_over_the_pairs`, `te6_crlf_changes_v2_only_for_the_named_multiline_literals`; over TE-2's edits `v2_follows_the_levels` in every `te2_*`.

spec-writer:

- [x] SW-1 -- this spec (refreshed and split 2026-10-13; refreshed, the RON value split off, 2026-10-15).
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/code-hash.md` with v2's "Hash recipe" detail moved down from the crate README (the heading the pointer, "(v3 candidate)" closed, the `levels` row added, net ≤ 0) and v3, room left for 5a-1b's "RON value"; `docs/index-canon.md` re-measured; `code-identity.md` "Open" without "per-level hashes"; 05 s5.2's levels and fingerprints a pointer; 08 s2's slice list; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Frozen bytes**: once 5b-1 writes locks, a leftover detail costs a v4 recipe and a rebase of every lock, re-keying every open drift item (`proposal-drift.md` "Known limits"); a touched v2 byte makes every format-1 lock "another recipe" (`hash_v2.rs` pins).
- **AC-10** (08: 2 s per 300 kLOC): levels and fingerprints walk more than v2 (containers and the file module re-walk their subtrees); one pass feeds every sink; 5a-2 reports ms per kLOC.

## Out of scope

CLI, lock, verify, ADR; the RON value (5a-1b); the eval reports and pilot figures (5a-2); `--deep`, a `qpath` discriminator, the suffix index (slice 6); per-branch ratios (Phase 4); following RON or a RON shape level; ast-grep user anchors; MCP `check_binding`; UI; level 2 (Q5). 5b-1 `lock-levels` records these digests, 5b-2 `lock-rebase` rebases locks, 5c `anchor-states` follows by `body`, `skeleton`, `quote`, never a trivial body or RON.

## Open

Decided under the owner's pre-approval: Q1 the 5a/5b/5c split; Q2 `spec lock rebase` in 5b-2 (ADR-0050); Q3 `body_bytes < 124` blocks a follow everywhere; Q4 a lost marker is not followed (ADR-0020); Q5 level 2 never built; Q6 the entry points, callers hashing by index; Q8 `#[macro_use]` breaks N5; Q9 the CR rule reaches every v3 stream and `body_bytes`; Q10 the 5a-1/5a-2 split; Q11 adjacent word leaves joined by a space; Q12 the RON value split off (Q7, Q15 live there); Q13 `BODY_FIELDS` as data; Q14 one triviality comparison; Q16 fingerprints never compared, never a drift fact. H1-H7 (reviews 1-2, owner pre-approved 2026-10-16): "Implementation".

## Implementation

| Module | What it does |
|---|---|
| code `levels.rs` (new) | the public v3 API ("Entry points"); crate-private `FileLevels` (lines indexed once per file), `walk3` (an explicit `Task` stack, `RunWalk` for N4/N5 runs), `Stream`, `cr_rule`, `deps_digest`, `quote_digest`, `Lines` |
| code `bind.rs` | `item_levels`, the twin of `hash_bound_items` over `analyze_with_nodes(.., ErrorsOnly)` |
| code `grammar.rs` | `NODE_TYPES` |
| code `hash.rs` | visibility only (`put` over `?Sized` sinks, `previous_code_sibling`, `attributed_item`, `Frame` and its methods, `Digest`'s field: `pub(crate)`) plus `Frame::root`; no v2 byte |
| code `lib.rs` | `pub mod levels`, root re-exports, the 5a-1 module-doc paragraph |

Deviations, accepted in review 1: 1 `BodyField.field` is `BodySlot::{Absent, Field(name), Children}`, not an optional string, so "no body" and "the `macro_rule` children" differ; 2 `call_targets(root)` on `source_file` is the module's `deps`; 3 a `Stream` buffers 16 KiB per BLAKE3 update, digests unchanged; 4 `hash.rs` visibility only, plus `Frame::root`; 5 the RON seams are crate-private.

Readings (reviews 1-2; H1-H4 built in iteration 2, H7 a known limit): H1 a call whose `function` is not `identifier`, `scoped_identifier`, `field_expression` or `generic_function` contributes only its `kind()` (iteration 1 took its whole text: O(n²) on `f()()…`, closure bodies in `deps`); H2 a `shebang` does not end the module's inner-attribute run, its leaf in `body` (iteration 1 ended it); H3 the module `quote` spans the first child to the last, ASCII whitespace trimmed at both ends (iteration 1: the root range, a trailing newline in); H4 `#[cfg_attr(pred, ..)]` with `macro_use` after `pred` breaks N5 too, a nested `cfg_attr` unread; H5, spec lines only: L6 strips one trailing CR on every `before`/`after` line, the last and a lone CR at EOF included, and `boolean_literal`'s `true`/`false` stay in `skeleton` (anonymous children); H6 `levels.rs` cites the README "Hash recipe", not this spec; H7 a `scoped_identifier` callee keeps its whole text, so crafted `<[u8; { <…>::f() }] as T>::f()` nested n deep costs O(n²) in `deps`, kept: bounding it changes the pinned `<T as Trait>::f` targets (`code-hash.md` "Known limits", with the `#! [x]` shebang quirk). The developer's L1-L12 byte readings adopted.

Measured (developer, release, this repository, 325 kLOC in 516 files): `bind_tree` plus every item's and module's levels 1.61 s, ≈ 4.9 ms/kLOC; v2's `analyze_tree` 0.56 s; 5a-2 reports the official AC-10 figure. Probes on a 256 KiB thread: 20 000 nested blocks and parentheses, 2 000 nested inline mods; runs 63 deep hash, 64 refuse.

Tests: round 1 (iteration 1) 61 green, the 4 H1-H4 pins red by design; round 2 (iteration 2) 67/67 `hash_levels_*` (code `units`, `props`, `library`, `depth`; eval `fixture`, `grammar`), 13 mutations killed (M1-M8, H1-H3, H4a/b), fmt and clippy clean. `import_cli`, `layout_cli` red only until `fixtures/hash-levels/` is committed.
