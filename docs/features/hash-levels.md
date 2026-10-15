---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5a-1 (WA-2..WA-5, WA-12, WA-14; Q1-Q16 decided under the owner's pre-approval, review open; refreshed and split 2026-10-13; refreshed 2026-10-15 against shipped 4b-1, 4c-1 and the drafts 4b-2, 4c-2, the RON value split off as 5a-1b); 05 s5.2; 08 s2, AC-14, AC-15
adrs: []
---

# hash-levels: per-level hashes and fingerprints of a bound item

## Why

A format-1 `spec.lock` entry holds one `ast_hash` of the whole item, levels ignored (`docs/canon/spec-lock.md` "Entry"): a `[sig]` marker reads a body edit as drift; `module` and `ron` bindings are judged by `file_blob` alone, so any byte of their file and an autocrlf checkout stale them ("Lockability"; `spec-verify.md` "Known limits"); a renamed function is `broken_binding symbol_gone` plus `unlocked` (`anchor-states.md` "Why"). Levels (`sig`, `body`, `deps`, `path`) and three fingerprints (`skeleton`, `quote`, `body_bytes`) answer all three (05 s5.2; a `ron` binding by its value hash, 5a-1b `hash-levels-ron.md`). Once a lock records them, their bytes change only with a `LEVEL_RECIPE` bump and a `spec lock rebase` of every lock (ADR-0050), which drops what it cannot locate: so they are fixed and measured before anything writes them.

Slice 5 is split (Q1): 5a, 5b `lock-levels`, 5c `anchor-states`; 5a again (Q10, Q12): **5a-1** (this), the Rust levels and fingerprints in `specengine-code` with their fixtures and pins; **5a-1b** `hash-levels-ron.md`, the RON value hash; **5a-2** `hash-levels-eval.md`, `specengine-eval`'s `ast-hash` and `ron` reports and the pilot figures (08 AC-2, AC-10). No CLI, lock, verify or ADR change. Built on the shipped slices 1-4a (`docs/canon/code-bindings.md`, `code-identity.md`, `spec-lock.md`, `spec-verify.md`); 4b-1 and 4c-1, shipped since, changed no `specengine-code` or `specengine-eval` source, and 4b-2, 4c-2 touch neither, so they run beside 5a. Order (08 s2): 5a-1 -> 5a-1b -> 5a-2 -> 6a-1 -> 6a-2 -> 5b, 5b only after 5a-2's pilot figures (`hash-levels-eval.md` TE-1); 5a-1 before 10a (both edit `lib.rs`: `bevy-roles.md` "Dependencies and ordering"); 6a-1 cuts signatures with "The body field" (`symbol-discriminator.md` "Signature"): 5a-1, shipping first, builds `body_nodes`.

## Description and interactions

- **Nobody sees it yet**: no command, MCP tool, lock field or UI changes. 5a-2 measures it, 5b records it (`lock-levels.md` "Format 2"), 5c matches on it, 6a-2 stores it (`symbol-index.md` Q11), 6a-1 cuts signatures with `body_nodes`. 4c-1's drift items see it only through 5b: a stored row gains `levels` after `compared` (core `drift.rs` `DriftRow` reads past a later key, `StoredJson` keeps it: `proposal-drift.md` "The item", **Store**); `drift_hash`'s `lock_code` becomes format 2's code fields as rendered (`lock-levels.md` Q7), its `code` fact 5b's.
- **Genre** (WA-14, ADR-0008): all language knowledge (tree-sitter-rust kinds and fields, N5, the RON lexer) stays in `specengine-code`; core will see only digests, lock keys and `body_bytes`. No Bevy special case: one-line `build` fns fall under the threshold.
- **Beside v2** (WA-4): `RECIPE`, `recipe_header()`, `hash_item`, `hash_node`, `hash_bound_items` and every v2 byte stay: format-1 locks name v2 (`spec-lock.md` "The file"); accept (CLI `lock.rs`), 4a's cascade (CLI `verify_rows.rs` `cascade`, which also serves `--changed`'s base, 4c-1's drift raise through `judge_commit` and 4b-2's `--staged`) and 5b's rebase cross-check call them; until 5b an open drift item's `drift_hash` holds v2 digests (`code`, `lock_code`: `proposal-drift.md` "The item"). `ItemRecord`, `FileAnalysis`, `HashState`, `Anchor` unchanged. v2's private walk (`hash.rs` `normalize_nested`) is shared only with its v2 setting byte-pinned (`hash_v2.rs`), else v3 walks on its own.
- **Callers hold indexes and text, never a node** (Q6): `analyze_with_nodes` is `pub(crate)`; the CLI walk keeps a Rust file's text and `RustBinding`, a RON file's text and `CommentAnchor`s, no `Tree` (cli `code_walk.rs` `RustFile`, `RonFile`); verify's cascade (`verify_rows.rs` `cascade`) and accept (from `HEAD`'s blobs) re-parse and hash by index (`hash_bound_items`); a commit's side (4c-1's `read_commit`, 4b-2's index side) holds the same `RustFile`, `RonFile`, bound from blobs by `tree_of_blobs`. "Entry points" take that shape.
- **Crates**: code (module `levels`, its body-field table exported as data; the twin in `bind.rs`; `grammar` re-exports `NODE_TYPES`; `lib.rs` exports and a module-doc paragraph for 5a); eval tests (the grammar pin, which reads JSON); `fixtures/hash-levels/`. The RON value range and hash in `ron`: 5a-1b (`hash-levels-ron.md`, Q12). Core, store, CLI untouched; no new dependency.
- **Overridden**: 05 s5.2 names the `body` field of three kinds only; "The body field" covers every collected kind.

## Data

### Entry points

Exported at the crate root, as `RECIPE` is (`lib.rs`). Digests are `hash.rs` `Digest`; the CLI renders `b3:` + 64 hex, as verify's cascade does.

| Name | Shape |
|---|---|
| `LEVEL_RECIPE` | `"specengine-hash/v3"`; 5b's `code_recipe` is composed from it as `build_recipes` composes v2's from `RECIPE` (cli `lock.rs`) |
| `level_header()` | `&'static [u8]`, "Levels" |
| `TRIVIAL_BODY_BYTES` | `u32` 124, "Triviality" |
| `LevelState` | `Hashed(LevelHashes)` or `CannotVerify { categories: Vec<ErrorCategory> }`, sorted as `error_categories`, the first the detail |
| `LevelHashes` | `sig`, `body`, `deps`, `skeleton`, `quote`: `Digest`; `body_bytes: u32`; `is_trivial()`, "Triviality" |
| `item_levels(tree: &Tree, source: &str, items: &[usize]) -> Vec<Option<LevelState>>` | in `bind.rs`, the twin of `hash_bound_items`: indexes into the `RustBinding::analysis` `bind_tree` gives for the same tree and source (the order `analyze_tree` gives too: both collect through `items.rs` `analyze_with_nodes`; 5a-2 indexes by it), in the order asked, `None` past its items; one item walk per call; never reads `ItemRecord.state` (an error-free item's is `NotHashed`); an inline `mod`'s module (`Binding::Module(Some(i))`) is item `i`'s entry |
| `module_levels(tree: &Tree, source: &str) -> LevelState` | the file's module (`Binding::Module(None)`), "File module" |
| `body_nodes(item: Node) -> Vec<Node>` | "The body field", in source order; empty: no body |
| `BODY_FIELDS` | `&'static [BodyField]`, one per `ITEM_KINDS` and `MEMBER_KINDS` kind: `kind`, `field` (none for `function_signature_item`; the `macro_rule` children for `macro_definition`), node `types`, `required`: "The body field" as data, read by `body_nodes` and RD-1's pin (Q13) |
| `call_targets(item: Node, source: &str) -> Vec<String>` | the texts `deps` hashes, byte-sorted, deduplicated |
| `grammar::NODE_TYPES` | `tree_sitter_rust::NODE_TYPES` re-exported, for RD-1's pin |

### Levels

WA-2, WA-12. `put(b)` = `len(b)` as u32 LE ‖ `b` (v2's private `put`, `hash.rs`); `level_header()` = `put(LEVEL_RECIPE) ‖ put(TREE_SITTER_RUST_VERSION) ‖ put(abi_version())`, built as `recipe_header()` is (today `0.24.2`, abi `15`). Each level is its own BLAKE3 digest.

| Level | Covers | Stream after `level_header()` |
|---|---|---|
| `sig` | attached attributes + the item without its name and body nodes; parameter names and `#[doc = ".."]` count (as in v2) | `put("sig") ‖ walk3(attr)... ‖ walk3(item, skip = {name, body})` |
| `body` | the body node(s), "The body field"; none: the empty stream | `put("body") ‖ walk3(body)...` |
| `deps` | call targets inside the body, byte-sorted, deduplicated | `put("deps") ‖ put(target)...` |
| `path` | the lock key `(file, kind, symbol)`; never hashed | changed iff the entry is `MOVED` (5c) |

- `walk3` = v2's walk with N1-N4 (crate README "Hash recipe") plus **N5**: a run of `mod x;` declarations (a `mod_item` without a body), or of `extern_crate_declaration`s, siblings below the root with comments between allowed, is emitted in the byte order of its members' walks, a member being its attached attributes ‖ itself (rustfmt `reorder_modules`, `reorder_imports`). A member carrying `#[macro_use]` breaks the run, staying in place (Q8: macro scope follows textual order), as another kind, item, statement or inner attribute does (N4); nesting is bounded by `MAX_USE_RUN_NESTING`.
- **CR rule** (Q9): in every v3 stream (levels, fingerprints, 5a-1b's RON token text) and in `body_bytes`, a CR right before an LF inside a leaf's text is dropped. rustc reads CRLF as LF, so a multi-line literal hashes and counts alike in both checkouts (`spec-verify.md` "Known limits", lifted by 5b).
- `sig` and `body` read exactly the leaves `hash_item` reads: the default `[sig, body]` (`markers.rs` `DEFAULT_LEVELS`) sees a change iff format 1's whole-item hash sees it, N5 and the CR rule aside (TE-6: 5b drops `ast_hash` on it).
- A `deps` target: a `call_expression`'s `function` (a `generic_function` replaced by its own `function`); a `field_expression` -> `.` ‖ its `field` (`x.collect::<V>()` -> `.collect`); otherwise its leaf texts, comments and every `type_arguments` with a `::` right before it dropped (`Vec::<u8>::new` -> `Vec::new`), concatenated with one space between two leaves where the first ends and the second starts with a letter, digit or `_` (Q11: `<T as Trait>::f`, never `<TasTrait>::f`). A `macro_invocation` -> its `macro` field's texts, joined alike, ‖ `!`. Every call inside the body node counts, closures and nested items included. Limits: calls inside a macro's token tree are invisible; a value passed as an argument is in `body`, not `deps` (`add_systems(Update, f)`); tuple-struct and variant constructors (`Some(x)`) count as calls.

### The body field

tree-sitter-rust 0.24.2 `node-types.json`, re-exported as `grammar::NODE_TYPES` (`specengine-code` reads no JSON). `body_field_table_matches_the_grammar` (eval tests) fails when `ITEM_KINDS` or `MEMBER_KINDS` (`items.rs`) holds a kind without a `BODY_FIELDS` row, or a row's field, node types or `required` differs from the grammar; re-pinned with every grammar bump (crate README "Rules"). `body_nodes(item)` is this table.

| Kind | Body | No body (empty stream) |
|---|---|---|
| `function_item` | `body` (`block`) | — |
| `function_signature_item` | — | always |
| `struct_item` | `body` (`field_declaration_list`, `ordered_field_declaration_list`) | unit struct |
| `enum_item`, `union_item`, `trait_item` | `body` | — |
| `impl_item`, `mod_item` | `body` (`declaration_list`; optional in the grammar) | `impl … ;`, `mod x;` |
| `const_item`, `static_item` | `value` (optional in the grammar) | `const X: T;`, `static X: T;` |
| `type_item` | `type` | — |
| `macro_definition` | every `macro_rule` child in order; the delimiters and `;` stay in `sig` (SW refinement) | — |

### File module

WA-3. A `//!` marker binds the file (`code-bindings.md` "Rust binding" rule 5). `sig` = the leading run of `source_file`'s `inner_attribute_item`s (comments between allowed); `body` = `walk3(source_file, skip = that run)`, N5 included; `deps` = every call in the file. `skeleton`, `quote`, `body_bytes` as for an item with the file as the item: no name, no context lines (5b records them for module entries).

- **Refusal**: rule 7 never refuses a file module, so `module_levels` does: any `ERROR` or `MISSING` node in the file gives `CannotVerify` with `error_categories(root)` (`hash.rs`), a run past `MAX_USE_RUN_NESTING` `[NestingTooDeep]`. 5b's accept row and verify detail read it (`lock-levels.md` "Accept", "Level-aware verify" step 3).
- **Asymmetry**: a file's `#![..]` is in its module's `sig`; an inline `mod x { #![..] }` is an item, its `#![..]` in `body`.

### Fingerprints

WA-5. Same `level_header()`, own tag.

| Fingerprint | Definition | Stream |
|---|---|---|
| `skeleton` | `walk3` of the attributes and the item without its name, every named leaf as `len‖kind` only: identifiers, literals, primitive types masked; keywords, operators, punctuation, structure kept; N4/N5 sort by the skeleton streams (SW refinement) | `put("skeleton") ‖ ...` |
| `quote` | textual: `head`, `tail` = the source from the first attached attribute (else the item) to the item's end, split around the name node (no name: `tail` empty); `before`, `after` = the nearest non-blank line before the line `head` starts on and after the item's last line, spaces and tabs trimmed (none: empty); the CR rule applied first | `put("quote") ‖ put(before) ‖ put(head) ‖ put(tail) ‖ put(after)` |
| `body_bytes` | the sum of the text lengths of the leaves `walk3(body)` emits (comments, `,`, N1-N3 forms excluded), after the CR rule: formatting- and checkout-independent | a `u32`, not hashed |

- **Never a judge** (Q16): `quote` reads the lines beside the item, so a neighbour's edit changes it by design (5a-2 reports it, never judges it). 5b stores the three fingerprints and compares none, and none is a drift item's current fact: an edit beside the item is the same drift (`proposal-drift.md` "The item").

**Triviality** (Q3, Q14): trivial iff `body_bytes < TRIVIAL_BODY_BYTES` (124, one constant, not configuration; 08 AC-15), answered by `LevelHashes::is_trivial()`, the one comparison in Rust code (5a-2 counts by it; 5c pins a copy in core). A trivial item is never followed (5c). No recipe, lock or index byte holds it: 5a-2's `trivial_at` sweep calibrates it, and a change before 5c moves only AC-15, the constant and 5c's copy.

**Broken** (AC-14): an item with `has_error()` on itself or an attached attribute (an `ERROR` or `MISSING` node), or a run past `MAX_USE_RUN_NESTING`, gets no level and no fingerprint: `CannotVerify` with v2's categories, never one level of a broken item.

## Rules and edge cases

- WHEN the same bytes are hashed by two parser instances, in two runs or in reverse file order, the system SHALL give the same digests: no path, line, offset or time enters a stream.
- Trap 1: comments by `is_comment`, never `is_extra()`; kinds by `kind()`, never `kind_id()`. Walks use an explicit stack, linear in the input; one item's levels and fingerprints come from one pass over several sinks where they can (AC-10, "Risks").
- No `.state` is read. The pin `nothing_outside_the_binder_reads_an_items_state` (cli `code_refs_walk.rs`) still asks cli `lock.rs` for `hash_bound_items(`; 5b widens it to `item_levels(` and to CLI `verify_rows.rs`, the cascade, `hash_bound_items`' other caller, unpinned today (it reads an item's `.state` only under `has_error`, for the error category).
- Code comments cite `crates/specengine-code/README.md` "Hash recipe" (kept as the pointer at shipping), never this spec's headings (compacted at shipping); "05 §5.2" joins `doc_pointers.rs` `STALE` only when that section is pointer-only (after 5c, 6a).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- "Levels" and "The body field" for every `ITEM_KINDS` and `MEMBER_KINDS` kind. M: name walked -> `rename_keeps_sig_body_and_deps`; body inside sig -> `body_edit_leaves_sig`; sig inside body -> `signature_edit_leaves_body`; const value in sig -> `const_value_edit_leaves_sig`; a bodiless `impl … ;` or `static X: T;` given a body -> `bodiless_impl_and_static_hash_an_empty_body`; source order -> `call_order_swap_keeps_deps`; calls of the signature -> `deps_ignore_calls_in_the_signature`; macro arguments read as calls -> `calls_inside_macro_arguments_are_not_deps`; a kind without a row, or a row off the grammar -> `body_field_table_matches_the_grammar` (eval tests, reading `grammar::NODE_TYPES` and `BODY_FIELDS`); a CR kept in a leaf -> `crlf_multiline_literal_keeps_every_level`.
- [ ] RD-2 -- a broken item gets no level and no fingerprint. M: sig hashed beside a body error -> `item_with_a_body_error_has_no_level_hash`; an `is_extra()` filter -> `broken_items_never_share_a_level_hash`.
- [ ] RD-3 -- v2 untouched, v3 frozen. M: `RECIPE` bumped -> `hash_v2.rs` `recipe_is_v2_and_heads_the_stream`, `unattributed_use_run_stream_is_pinned`; one header shared -> `level_header_is_v3_and_item_hash_is_v2`; any v3 byte -> `level_streams_are_pinned` (golden bytes per level and fingerprint, one input per kind).
- [ ] RD-4 -- file module and N5. M: no N5 -> `reordered_mod_declarations_keep_the_module_hash`; `#[macro_use]` sorted -> `macro_use_mod_order_changes_the_module_hash`; inner attributes in body -> `inner_attribute_edit_is_a_module_sig_change`; a file with an error elsewhere hashed -> `module_of_a_file_with_an_error_cannot_verify`; `extern crate` runs left unsorted -> `reordered_extern_crates_keep_the_module_hash`; a module's `deps` short of the file's calls -> `module_deps_are_every_call_in_the_file`.
- RD-5 -- the RON value: moved to `hash-levels-ron.md` RD-1 (Q12).
- [ ] RD-6 -- fingerprints. M: identifier text kept -> `renamed_locals_keep_the_skeleton`; literal text kept -> `changed_literal_keeps_the_skeleton`; no context lines -> `copies_differ_by_quote_when_their_context_differs`; CR kept -> `quote_is_equal_under_lf_and_crlf`; CR counted -> `crlf_literal_keeps_body_bytes`; `body_bytes` from the source span -> `rustfmt_variants_keep_body_bytes`; `<=` for `<` in `is_trivial` -> `body_of_exactly_124_bytes_is_not_trivial`.
- [ ] RD-7 -- linear and iterative. M: a recursive walk -> `deep_nesting_does_not_overflow_the_level_walks` (a 256 KiB thread, as `hash_depth.rs`); a quadratic quote -> `quote_cost_is_linear` (best of three, as `marker_levels.rs`).
- [ ] RD-8 -- genre and graph. M: a `build`, `Plugin` or `add_systems` literal -> `level_sources_name_no_framework`; a fourth dependency beside tree-sitter, tree-sitter-rust, blake3 -> `build_graph.rs` `code_crate_depends_on_exactly_the_parser_and_the_hash`; tree-sitter in core or store -> `core_and_store_have_no_tree_sitter`.
- [ ] RD-9 -- "Entry points". M: the twin's order off the indexes asked -> `level_twin_follows_binding_indexes` (an error-free item, `NotHashed` in the binding, `Hashed` there), and `analyze_tree`'s order differing -> the same test over both analyses; an inline `mod`'s module off item `i`'s entry -> `inline_mod_module_is_its_items_levels`; glued `deps` -> `qualified_call_targets_do_not_glue`; the RON entry's test: `hash-levels-ron.md` RD-2.

test-engineer:

- [ ] TE-1 -- `fixtures/hash-levels/` with `expected.json`: per level and per kind an equal pair and a different pair, LF and an in-memory CRLF copy for items and file modules (RON values: `hash-levels-ron.md` TE-1), each pair naming its mutation.
- [ ] TE-2 -- a property test over every item of every `.rs` file of `fixtures/hash-levels/` (the only fixture committing rustfmt variants): levels, `skeleton`, `body_bytes` stable under the committed rustfmt variants (default, `max_width = 60`), comment stripping and a neighbour edit (`#[cfg(all())]` and an LF before every other item's first attached attribute, both parities); each level changes under its own targeted edit. M: a constant hash; a hash that reads a neighbour.
- [ ] TE-3 -- AC-14 over TE-1's `expected.json`: every broken item and every broken file's module `CannotVerify` with its categories, no digest shared by two broken units (RD-2 pins the trap in a unit test; this over the fixture).
- [ ] TE-4 -- determinism: two parser instances, two runs, reversed file order (RON: `hash-levels-ron.md` TE-2).
- [ ] TE-5 -- AC-15, the library half: a rename keeps `body`, `skeleton`, `sig`, `quote`; a trivial body stays under 124 B in both rustfmt variants; two copies share `skeleton` and `body`.
- [ ] TE-6 -- v2 equivalence, 5b's ground for dropping `ast_hash`: over every fixture item and every TE-1, TE-2 edit, `hash_item` changes exactly when `sig` or `body` changes, the N5 and CR cases named as the only exceptions. M: a leaf in neither stream.

spec-writer:

- [x] SW-1 -- this spec (refreshed and split 2026-10-13; refreshed, the RON value split off, 2026-10-15).
- [ ] SW-2 -- at shipping: a Tier 2 `docs/canon/code-hash.md` (≤ 12 288 B; v2's detail ≈ 3 KB plus v3 ≈ 8-9 KB once compacted: measure the draft, and leave room for 5a-1b's "RON value") holding v2's "Hash recipe" detail, moved down from the crate README (10 199 of 10 240 B on 2026-10-15, re-measure; the heading stays as the pointer, "Known limits and open minors"' "(v3 candidate)" closed, the `levels` module row added, net ≤ 0) and v3 ("Entry points" to "Fingerprints"); none of it in `code-identity.md`, `spec-lock.md`, `spec-verify.md` or `proposal-drift.md`; `docs/index-canon.md` re-measured (9 092 of 10 240 B on 2026-10-15, a canon line ≈ 160 B; over the cap the shard splits: `docs/README.md` "Generated"); `code-identity.md` "Open" (12 259 of 12 288 B) loses "per-level hashes"; 05 s5.2's levels and fingerprints reduced to a pointer (5b's SW-4 takes the lock); 08 s2's slice list; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Frozen bytes**: once 5b writes locks, a leftover detail (N5, the `deps` text, the CR reach) costs a v4 recipe and a rebase of every lock; a touched v2 byte makes every format-1 lock "another recipe" (`hash_v2.rs` pins). After 5b the digests are drift-item evidence too (`lock_code`, `code`): a recipe change and its rebase re-key every open drift item and bring every rejected one back once (`proposal-drift.md` "Known limits").
- **AC-10** (08: 2 s per 300 kLOC): 6a hashes every item on each refresh (`symbol-index.md` "Risks"); levels and fingerprints are 3-4 walks per item against v2's one, and containers and the file module walk their subtrees again. Mitigation: one pass over several sinks; 5a-2 reports ms per kLOC.
- **Tree-sitter**: optional bodies ("The body field"); `MISSING` and `ERROR` refuse the whole unit; a grammar bump re-pins the body table.
- Q8 rests on rustfmt's reorder rules, not checked here.

## Out of scope

CLI, lock, verify, ADR; the RON value hash (5a-1b, `hash-levels-ron.md`); the eval reports and pilot figures (5a-2, `hash-levels-eval.md`). `--deep`, a `qpath` discriminator, the suffix index (slice 6); per-branch ratios (Phase 4); following RON or a RON shape level; ast-grep user anchors; MCP `check_binding`; UI; level 2 (Q5); a stateless item view (code-refs' carry-over, settled as `NotHashed` plus `hash_bound_items`: `spec-lock.md` feature "Implementation").

- **5b `lock-levels`** (`lock-levels.md`, ADR-0050): lock format 2 recording these digests, level-aware verify, `spec lock rebase`.
- **5c `anchor-states`** (`anchor-states.md`): a moved binding paired with its old entry by `body`, then `skeleton` in its file, then `quote`, never stored; trivial bodies and RON never followed.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 slice 5 | split: 5a `hash-levels`, 5b `lock-levels`, 5c `anchor-states` | — |
| Q2 format-1 and old-recipe locks | `spec lock rebase` in 5b, ADR-0050 (`lock-levels.md`) | — |
| Q3 triviality | `body_bytes < 124` blocks a follow everywhere (AC-15 as written) | — |
| Q4 a lost marker | not followed: the marker is the identity (ADR-0020); `broken_binding marker_gone` | — |
| Q5 level 2 (05 s5.3 step 2) | never built; done: 05 s5.3 reads "dropped" since 4a shipped | — |
| Q6 who adds the index twin and the RON hashes | 5a-1: the twin (levels and fingerprints for chosen `RustBinding::analysis.items` by index, one item walk per call, never reading `ItemRecord.state`; also an inline mod's module), `module_levels(tree, source)` for a file module, per-file RON value hashes keyed by comment range (one lex per file; 5a-1b since Q12); exported at the crate root with `LEVEL_RECIPE`, `TRIVIAL_BODY_BYTES`, `LevelState`; 5a-2, 5b, 5c and 6a-2 consume them, 6a-1 `body_nodes` | 5b adds code-crate API 5a's fixtures never pin; the eval copies the item walk, or `analyze_with_nodes` goes public |
| Q8 `#[macro_use]` in N5 | a member carrying `#[macro_use]` breaks the N5 run | reordering a `#[macro_use] mod` reads `ok`; fixable only by a v4 recipe and a rebase |
| Q9 reach of the CR rule | the CR rule reaches every v3 stream, RON token text and `body_bytes` included | RON multi-line strings stay `code_ahead` under autocrlf after 5b; triviality differs by checkout |
| Q10 split | 5a-1 (this): the library and fixtures with the pins; 5a-2 `hash-levels-eval.md`: the eval and the pilot figures, run by the owner; 5b after 5a-2 | one L slice: the eval may use up the three iterations and leave the library unshipped |
| Q11 a `deps` target's text | it joins two adjacent word leaves with one space | colliding `deps`; an unreadable call list for 7b's later call-graph radius (`impact.md` "Out of scope") |
| Q12 5a-1's size (owner) | split: the RON value as 5a-1b `hash-levels-ron.md` (the ranges in `ron/structure.rs`, `ron::value_hashes`, "RON value", Q7, Q15, its tests); it shares `level_header()`, `put`, the CR rule; order 5a-1 -> 5a-1b -> 5a-2 | a third iteration on one slice can leave the Rust levels unshipped, blocking 5a-2, 6a-1, 6a-2, 5b, 10a |
| Q13 the body-field table | exported as data, `BODY_FIELDS`, read by `body_nodes` and RD-1's pin | the pin compares the grammar with a test-local copy: the code's table can lose a row unseen |
| Q14 the triviality comparison | one, `LevelHashes::is_trivial`; 5a-2 counts by it, 5c pins a copy of 124 | RD-6's `<` mutation has no site; the eval and 5c each compare 124 on their own |
| Q16 fingerprints as evidence | never compared, never a drift fact: a constraint handed to 5b | a neighbour's edit changes `quote` and would re-key drift items, against `proposal-drift.md` "The item" |

Q1-Q5 (2026-10-10) and Q6-Q11 (2026-10-13) decided under the owner's pre-approval, review open; Q12-Q14 and Q16 (the refresh against shipped 4b-1 and 4c-1) decided 2026-10-15 under the owner's pre-approval, review open; Q7 and Q15 (the RON value) live in `hash-levels-ron.md` since Q12; WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. The split (2026-10-13, Q10) renumbered the criteria: RD-1..RD-8, TE-1..TE-4, SW-1..SW-3 keep their numbers; old RD-9 (eval) -> `hash-levels-eval.md` RD-1; old TE-5 (pilots) -> its TE-1; old TE-6 -> TE-5; new RD-9 (entry points), TE-6 (v2 equivalence); SW-2's eval README rows and 08 AC-2 -> its SW-2. Refreshed 2026-10-15, no renumbering: RD-1's pin reads `BODY_FIELDS`; RD-6's last test renamed; RD-4, RD-9 gain tests; TE-2's corpus named; TE-3 over the fixture. The split (Q12): RD-5, RD-9's RON test and the RON parts of TE-1, TE-4 -> `hash-levels-ron.md` RD-1, RD-2, TE-1, TE-2; RD-5 kept as a pointer. To record: modules, deviations with reasons.
