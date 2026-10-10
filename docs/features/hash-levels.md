---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5a (WA-2..WA-5, WA-12..WA-14; Q1-Q5 decided under the owner's pre-approval, review open); 05 s5.2; 08 s2, AC-2, AC-14, AC-15
adrs: []
---

# hash-levels: per-level hashes and fingerprints of a bound item

## Why

A format-1 `spec.lock` entry (`spec-lock.md`) holds one `ast_hash` of the whole item: a `[sig]` marker reads a body edit as drift, and a renamed function is a `broken_binding`. Levels (`sig`, `body`, `deps`, `path`) and three fingerprints (`skeleton`, `quote`, `body_bytes`) answer both (05 s5.2), but once a lock records them their bytes cannot change without re-verifying every binding, so they are fixed and measured on the pilots (08 AC-2) before anything writes them.

Slice 5 is split (Q1): **5a** (this) — functions in `specengine-code` and their measurement in `specengine-eval`; no CLI, lock, verify or ADR change. **5b** `lock-levels` and **5c** `anchor-states` ("Out of scope"). 5a is drafted beside slice 1 (`code-refs.md`); it is implemented after code-refs ships (both edit `specengine-code`, `lib.rs` included).

## Description and interactions

- **Nobody sees it yet**: no command, MCP tool, lock field or UI changes; the owner reads `specengine-eval ast-hash` / `ron` figures. 5b records the values, 5c matches on them.
- **Genre** (WA-14): all language knowledge (tree-sitter-rust kinds and fields, the RON lexer) stays in `specengine-code`; core will see only digests, lock keys and `body_bytes` (5c). No Bevy special case: one-line `build` fns fall under the threshold.
- **Beside v2** (WA-4): `RECIPE`, `recipe_header()`, `hash_item`, `hash_node` and every v2 byte stay (format-1 locks name v2, `spec-lock.md` "The file"). Functions are added over `Node`; `ItemRecord`, `FileAnalysis` unchanged.
- **API** (SW refinement): module `levels` — `LEVEL_RECIPE`, `level_header()`, `TRIVIAL_BODY_BYTES`, `item_levels`/`module_levels(node, source) -> LevelState` (`Hashed(LevelHashes)` | `CannotVerify`), `call_targets`; `ron::value_hash`.
- **Overridden**: 05 s5.2 names the `body` field of three kinds only; "The body field" below covers every collected kind.

## Data

### Levels

WA-2, WA-12. `put(b)` = `len(b)` as u32 LE ‖ `b` (v2's); `level_header()` = `put("specengine-hash/v3") ‖ put(TREE_SITTER_RUST_VERSION) ‖ put(abi_version())`. Each level is its own BLAKE3 digest, rendered `b3:` + 64 hex.

| Level | Covers | Stream after `level_header()` |
|---|---|---|
| `sig` | attached attributes + the item without its name and body nodes; parameter names and `#[doc = ".."]` count (as in v2) | `put("sig") ‖ walk3(attr)... ‖ walk3(item, skip = {name, body})` |
| `body` | the body node(s), "The body field"; none: the empty stream | `put("body") ‖ walk3(body)...` |
| `deps` | call targets inside the body, byte-sorted, deduplicated | `put("deps") ‖ put(target)...` |
| `path` | the lock key `(file, kind, symbol)`; never hashed | changed iff the entry is `MOVED` (5c) |

- `walk3` = v2's walk with N1-N4 (crate README "Hash recipe") plus **N5**: a run of `mod x;` declarations (a `mod_item` without a body), or of `extern_crate_declaration`s, siblings below the root with comments between allowed, is emitted in the byte order of its members' walks, a member being its attached attributes ‖ itself (rustfmt `reorder_modules`, `reorder_imports`); another kind, item, statement or inner attribute breaks the run, as in N4; nesting is bounded by `MAX_USE_RUN_NESTING`. In every v3 stream a CR right before an LF inside a leaf's text is dropped (rustc reads CRLF as LF: a multi-line literal hashes alike in both checkouts; SW refinement).
- `sig` and `body` read exactly the leaves `hash_item` reads: the default `[sig, body]` (`markers.rs` `DEFAULT_LEVELS`) sees a change iff format 1's whole-item hash sees it, N5 and the CR rule aside.
- A `deps` target: a `call_expression`'s `function` (a `generic_function` replaced by its own `function`); a `field_expression` -> `.` ‖ its `field` (`x.collect::<V>()` -> `.collect`); otherwise its leaf texts concatenated, comments and every `type_arguments` with a `::` right before it dropped (`Vec::<u8>::new` -> `Vec::new`). A `macro_invocation` -> its `macro` field's texts ‖ `!`. Every call inside the body node counts, closures and nested items included. Limits: calls inside a macro's token tree are invisible; a value passed as an argument is in `body`, not `deps` (`add_systems(Update, f)`); tuple-struct and variant constructors (`Some(x)`) count as calls.

### The body field

tree-sitter-rust 0.24.2, `node-types.json`; a test pins it against the grammar and fails when `ITEM_KINDS` or `MEMBER_KINDS` (`items.rs`) holds a kind without a row; re-pinned with every grammar bump (crate README "Rules").

| Kind | Body | No body (empty stream) |
|---|---|---|
| `function_item` | `body` (`block`) | — |
| `function_signature_item` | — | always |
| `struct_item` | `body` (`field_declaration_list`, `ordered_field_declaration_list`) | unit struct |
| `enum_item`, `union_item`, `trait_item` | `body` | — |
| `impl_item`, `mod_item` | `body` (`declaration_list`) | `mod x;` |
| `const_item`, `static_item` | `value` | `const X: T;` |
| `type_item` | `type` | — |
| `macro_definition` | every `macro_rule` child in order; the delimiters and `;` stay in `sig` (SW refinement) | — |

### File module

WA-3. A `//!` marker binds the file (`code-refs.md`). `sig` = the leading run of `source_file`'s `inner_attribute_item`s (comments between allowed); `body` = `walk3(source_file, skip = that run)`, N5 included; `deps` = every call in the file. An inline `mod x { }` is an item. `skeleton`, `quote`, `body_bytes` as for an item with the file as the item: no name, no context lines (SW refinement: 5b records them for module entries).

### RON value

WA-3; closes `code-identity.md` "Open" (level meaning in RON). For a marker with a `Path` anchor: `value_hash` = BLAKE3(`level_header() ‖ put("ron") ‖ put(text(t))` for each lexer token `t` of the bound value, from its first token through its last: a container through its closer, a struct name included; comments dropped; a `,` whose next non-comment token is a closer dropped; every other token's text verbatim). `sig` = `body` = that digest; `deps` = the empty set's digest; no fingerprint (RON is never followed). `RonAnalysis.has_error` makes every bound value of the file `cannot_verify`; `Ambiguous`, `Unanchored`, `CannotVerify` anchors get no hash. The structure walk exposes the value's range (today only its path, `ron/mod.rs` `Anchor`).

### Fingerprints

WA-5. Same `level_header()`, own tag.

| Fingerprint | Definition | Stream |
|---|---|---|
| `skeleton` | `walk3` of the attributes and the item without its name, every named leaf as `len‖kind` only: identifiers, literals, primitive types masked; keywords, operators, punctuation, structure kept; N4/N5 sort by the skeleton streams (SW refinement) | `put("skeleton") ‖ ...` |
| `quote` | textual: `head`, `tail` = the source from the first attached attribute (else the item) to the item's end, split around the name node (no name: `tail` empty); `before`, `after` = the nearest non-blank line before the item's first line and after its last, spaces and tabs trimmed (none: empty); every CR before an LF dropped first | `put("quote") ‖ put(before) ‖ put(head) ‖ put(tail) ‖ put(after)` |
| `body_bytes` | the sum of the text lengths of the leaves `walk3(body)` emits (comments, `,`, N1-N3 forms excluded): formatting-independent | a `u32`, not hashed |

**Triviality** (Q3): trivial iff `body_bytes < TRIVIAL_BODY_BYTES` (124, one constant, not configuration; 08 AC-15). A trivial item is never followed (5c). TE-5's twin counts calibrate it; changing the number changes AC-15.

**Broken** (AC-14): an item with `has_error()` on itself or an attached attribute, or a run past `MAX_USE_RUN_NESTING`, gets no level and no fingerprint: `CannotVerify` with v2's categories, never one level of a broken item.

### Eval additions

WA-13; counts and percentages only (eval README "Output"). `specengine-eval ast-hash` adds `result.levels`:

- `hashed`, `cannot_verify`: items with and without levels.
- `stable_pct.<p>.<f>`: `p` in `fmt_default`, `fmt_contrast`, `comments`, `neighbour`; `f` in `sig`, `body`, `deps`, `skeleton`, `body_bytes`, `quote` (`quote` reported, never judged: textual by design, outside AC-2). `module_stable_pct.<p>.{sig, body, deps}` over files, the first three `p`.
- `neighbour.{edited, edited_changed, checked}`: two passes per file over its items in source order, one re-parse each; pass 1 edits every odd item and checks every even one, pass 2 swaps. The edit inserts `#[cfg(all())]` and an LF before the item's first attached attribute (else the item): its `sig` changes, every later byte and line shifts. Checked: items of the other parity neither holding nor inside an edited item. `edited_changed` = edited items whose `sig` changed, equal to `edited` (non-vacuity).
- `trivial.{items, pct}`; `body_groups.{groups, items}`: 2+ non-trivial items of one kind with one `body` across the corpus; `skeleton_groups_in_file.{groups, items}`: 2+ items of one kind and file with one `skeleton`; `quote_separates`: of these groups, those whose members' quotes all differ.
- `rename.{renamed, kept.<f>, followable}`: in memory, the name node of every named item holding no other named item replaced by a distinct fixed identifier, one re-parse per file; `kept` per `f` (all but `quote` equal to `renamed`); `followable` = renamed, non-trivial, with a `body` unique among the corpus's items of its kind.

`specengine-eval ron` adds `result.value_hash.{bound, hashed, cannot_verify, stable_pct}`: every comment holding no marker stripped (stripping a marker's comment drops its binding), values compared per marker in source order. `fixtures/ast-hash/expected.json` and `fixtures/ron/expected.json` pin the exact fixture counts.

## Rules and edge cases

- WHEN the same bytes are hashed by two parser instances, in two runs or in reverse file order, the system SHALL give the same digests: no path, line, offset or time enters a stream.
- Trap 1: comments by `is_comment`, never `is_extra()`; kinds by `kind()`, never `kind_id()`. Walks use an explicit stack, linear in the input.
- Pilots stay read-only: perturbations in memory or on scratch copies (eval README "Rules"); no pilot name or path committed.

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- "Levels" and "The body field" for every `ITEM_KINDS` and `MEMBER_KINDS` kind. M: name walked -> `rename_keeps_sig_body_and_deps`; body inside sig -> `body_edit_leaves_sig`; sig inside body -> `signature_edit_leaves_body`; const value in sig -> `const_value_edit_leaves_sig`; source order -> `call_order_swap_keeps_deps`; calls of the signature -> `deps_ignore_calls_in_the_signature`; macro arguments read as calls -> `calls_inside_macro_arguments_are_not_deps`; a kind without a row -> `body_field_table_matches_the_grammar`; a CR kept in a leaf -> `crlf_multiline_literal_keeps_every_level`.
- [ ] RD-2 -- a broken item gets no level and no fingerprint. M: sig hashed beside a body error -> `item_with_a_body_error_has_no_level_hash`; an `is_extra()` filter -> `broken_items_never_share_a_level_hash`.
- [ ] RD-3 -- v2 untouched. M: `RECIPE` bumped -> `hash_v2.rs` `recipe_is_v2_and_heads_the_stream`, `unattributed_use_run_stream_is_pinned`; one header shared -> `level_header_is_v3_and_item_hash_is_v2`.
- [ ] RD-4 -- file module and N5. M: no N5 -> `reordered_mod_declarations_keep_the_module_hash`; inner attributes in body -> `inner_attribute_edit_is_a_module_sig_change`.
- [ ] RD-5 -- RON value. M: comments hashed -> `ron_comment_edit_keeps_the_value_hash`; sibling tokens -> `ron_sibling_edit_keeps_the_value_hash`; trailing comma kept -> `ron_trailing_comma_keeps_the_value_hash`; a broken file hashed -> `broken_ron_file_value_cannot_verify`.
- [ ] RD-6 -- fingerprints. M: identifier text kept -> `renamed_locals_keep_the_skeleton`; literal text kept -> `changed_literal_keeps_the_skeleton`; no context lines -> `copies_differ_by_quote_when_their_context_differs`; CR kept -> `quote_is_equal_under_lf_and_crlf`; `body_bytes` from the source span -> `rustfmt_variants_keep_body_bytes`; `<=` for `<` -> `body_of_exactly_124_bytes_is_followable`.
- [ ] RD-7 -- linear and iterative. M: a recursive walk -> `deep_nesting_does_not_overflow_the_level_walks` (a 256 KiB thread, as `hash_depth.rs`); a quadratic quote -> `quote_cost_is_linear` (best of three, as `marker_levels.rs`).
- [ ] RD-8 -- genre and graph. M: a `build`, `Plugin` or `add_systems` literal -> `level_sources_name_no_framework`; a fourth dependency beside tree-sitter, tree-sitter-rust, blake3 -> the `specengine-code` pin in `build_graph.rs`.
- [ ] RD-9 -- "Eval additions". M: a neighbour pass skipped -> `levels.neighbour` in `fixtures/ast-hash/expected.json`; a constant hash -> `edited_changed` below `edited`.

test-engineer:

- [ ] TE-1 -- `fixtures/hash-levels/` with `expected.json`: per level and per kind an equal pair and a different pair, LF and an in-memory CRLF copy, each pair naming its mutation.
- [ ] TE-2 -- a property test over every item of every fixture `.rs` file: levels, `skeleton`, `body_bytes` stable under the committed rustfmt variants (default, `max_width = 60`), comment stripping and both neighbour passes; each level changes under its own targeted edit. M: a constant hash; a hash that reads a neighbour.
- [ ] TE-3 -- AC-14: two different broken items give no level hash. M: an `is_extra()` filter.
- [ ] TE-4 -- determinism: two parser instances, two runs, reversed file order.
- [ ] TE-5 -- pilots, manual (`#[ignore]`, release, foreground, `--timeout`, a scratch HOME): `levels` per level and perturbation (100.0 %, or named exceptions the owner accepts), twin and triviality counts, rename counts, wall time; read-only proof (`git status`, a file listing).
- [ ] TE-6 -- AC-15, the library half: a rename keeps `body`, `skeleton`, `sig`, `quote`; a trivial body stays under 124 B in both rustfmt variants; two copies share `skeleton` and `body`.

spec-writer:

- [x] SW-1 -- this spec.
- [ ] SW-2 -- at shipping: a Tier 2 canon for v3 (12 288 B cap; the crate README, 9 424 of 10 240 B, cannot hold it) pointed at from its "Hash recipe"; `code-identity.md` "Open"; 05 s5.2's levels and fingerprints reduced to a pointer; 08 AC-2 met with figures, 08 s2's slice list; the eval README rows.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- A touched v2 byte turns every format-1 lock into "another recipe" (`hash_v2.rs` pins); the body table moves with the grammar.
- Neither pilot holds a single marker in `.rs` or `.ron`: levels and anchors are proven on fixtures; pilot evidence is AC-2 figures, twin counts, the rename simulation. limpet's 124 B is not verifiable locally.
- Cost: `ast-hash` re-parses each file three more times (two neighbour passes, the rename).

## Out of scope

CLI, lock, verify, ADR. `--deep`, a `qpath` discriminator, the suffix index (slice 6); per-branch ratios (Phase 4); following RON or a RON shape level; ast-grep user anchors; MCP `check_binding`; UI. Level 2 (Q5).

- **5b `lock-levels`** (after slices 3, 4a): lock format 2 (every level and fingerprint per entry, `levels` the union of the markers', no `ast_hash`, `code_recipe` v3), level-aware verify (`code_ahead levels_changed`), `spec lock rebase` (Q2) re-deriving each entry from its own `file_blob`, a new ADR adding that writer to ADR-0041.
- **5c `anchor-states`** (after 5b): `EXACT`/`MOVED`/`AMBIGUOUS`/`LOST` per entry and run, never stored; candidates by equal `body`, then `skeleton` in the entry's file, ties by `quote`; a mutual unique match only; trivial bodies and RON `LOST`.

## Open

| Point | Answer |
|---|---|
| Q1 slice 5 | split: 5a `hash-levels`, 5b `lock-levels`, 5c `anchor-states` |
| Q2 format-1 and old-recipe locks | `spec lock rebase` in 5b, with a new ADR |
| Q3 triviality | `body_bytes < 124` blocks a follow everywhere (AC-15 as written) |
| Q4 a lost marker | not followed: the marker is the identity (ADR-0020); `broken_binding marker_gone` |
| Q5 level 2 (05 s5.3 step 2) | never built; its line goes at 5b's shipping |

Decided under the owner's pre-approval, review open; WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. To record: modules, TE-5's figures, deviations with reasons.
