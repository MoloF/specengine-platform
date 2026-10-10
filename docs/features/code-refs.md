---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-core, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 1 (WA-1..WA-14, RD/TE/SW criteria; Q1, Q2 decided under the owner's pre-approval, review open); 05 s5.3; 07 s2, s5; 08 s2 Phase 3; Rust 2 iterations, review accepted
shipped: 2026-10-10
adrs: []
---

# code-refs: markers bound to code, spec refs

## Why

Nobody saw which code implements, verifies, configures or assumes a node: markers bound only in RON, for the eval alone; code citations were not the weak mentions ADR-0016 promised; a lagging marker (`@2` on rev 3) was invisible; agents lacked 07 s2's reverse lookup; slices 3-4 need a binding model. The slice reads a configured code tree and writes nothing: each marker gets one Rust item, module or RON path, or a reason; its ID resolves as `spec show`'s REF; `@N` meets the node's revision; comment citations become weak mentions; `spec refs REF [--json]` lists them. No ADR (ADR-0008, -0009, -0010, -0012, -0016, -0018, -0020, -0026, -0031 applied).

## Description and interactions

Canon: `docs/canon/code-bindings.md` (all of it); `code-identity.md` "Target table", "Marker grammar"; `architecture-code.md#markers`; `stack.md` (tree-sitter, `toml`); `spec-check.md` "Not checked yet"; the CLI, code, core, model, eval READMEs. The CLI gains `specengine-code` and `toml` (WA-12); MCP and the daemon reach tree-sitter through the CLI library. Unchanged (WA-11, WA-13): `spec check`, hook triggers, MCP tools, HTTP, UI, `INDEX_FORMAT`; this repository has no `[code]`.

## Data

### The code table

`docs/canon/code-bindings.md` "The `[code]` table" (WA-1).

### The walk

`docs/canon/code-bindings.md` "The walk" (WA-2, WA-3, WA-9); package names: `code-identity.md` "Target table".

### Binding rules

`docs/canon/code-bindings.md` "Rust binding" (WA-4).

### States

`docs/canon/code-bindings.md` "ID state", "Revision state" (WA-5, WA-6, WA-10); the token: `code-identity.md` "Marker grammar".

### Mentions

`docs/canon/code-bindings.md` "Mentions" (WA-7, Q2).

### spec refs

`docs/canon/code-bindings.md` "`spec refs`" (WA-8): text, JSON, cap, exits, the no-table sentence.

## Rules and edge cases

`docs/canon/code-bindings.md` "Guarantees": byte-identical output whatever the root, cwd, order or line ends; only the data directory written; every state listed, nothing blocked (ADR-0012).

## Acceptance criteria

Code `tests/code_bind.rs` (13), `marker_tokens.rs` (9); CLI `tests/code_refs.rs` (12), `code_refs_walk.rs` (15), `refs.rs` `#[cfg(test)]` (3); eval `build_graph.rs`, `ron_cli.rs` +1, `code_refs_pilot.rs` (1 + 2 `#[ignore]`); fixture `fixtures/code-a/`. Every named mutation red (22 in iteration 1, 5 in 2).

rust-developer:

- [x] RD-1 -- `code_table_unknown_key_is_refused` (seven bad tables, exit 2 at the key's line).
- [x] RD-2 -- `code_walk_skips_symlinks`, `code_walk_gap_is_exit_2`.
- [x] RD-3 -- `marker_above_use_is_unanchored`, `marker_above_derive_binds_the_struct`, `trailing_marker_binds_its_line_item`, `marker_in_body_is_unanchored`, `marker_on_broken_item_cannot_verify`; no new dependency.
- [x] RD-4 -- `section_marker_compares_the_section_rev`, `newer_marker_is_predated`, `homoglyph_marker_is_never_resolved`.
- [x] RD-5 -- `string_literal_ids_are_not_mentions`, `marker_id_is_not_a_mention`, `doc_comment_mention_belongs_to_its_item`.
- [x] RD-6 -- `refs_without_code_table_is_exit_2`, `refs_is_byte_identical_across_roots_and_order`.
- [x] RD-7 -- `refs_leaves_the_worktree_untouched`.
- [x] RD-8 -- `store_depends_on_model_core_and_sqlite_but_no_measurement_crate`, new `core_and_store_have_no_tree_sitter`; `Cargo.lock`: two CLI edges, no new package.
- [ ] RD-9 -- pilot counts: manual, owner (TE-9's harness).

test-engineer:

- [x] TE-1 -- `fixtures/code-a/` + `expected.json`; `code_a_refs_match_expected_json`, `code_a_generated_cases_match_expected_json`.
- [x] TE-2 -- `every_binding_rule_and_category_is_a_row_lf_and_crlf`, `the_table_names_every_rule_and_category`, `code_a_refs_are_the_same_with_crlf_line_ends`.
- [x] TE-3 -- `refs.rs` `#[cfg(test)]`: every `id_state`, `rev_state`, U+0420, `X@3a`, `@1` on no rev.
- [x] TE-4 -- `refs_is_byte_identical_across_roots_and_order`, `manifests_name_packages_alike_from_any_root`.
- [x] TE-5 -- `refs_leaves_the_worktree_untouched`.
- [x] TE-6 -- `refs_on_spec_c_exits_2_naming_the_code_table`, `new_modules_hold_no_fixture_prefix_kind_path_or_stack_word` (the 07 s5 keys exempt in `code_toml.rs`).
- [x] TE-7 -- `check_is_the_same_with_and_without_the_code`.
- [x] TE-8 -- `build_graph.rs`: `CLI_FORBIDDEN` loses only `specengine-code`; three direct-dependency lists gain it and `toml`; `proposal_genre.rs` `ac20_no_dependency_is_added`.
- [x] TE-9 -- `code_refs_pilot.rs`, `#[ignore]`, release; `the_census_runs_read_only_on_a_fixture_copy` green; pilot runs: RD-9.

spec-writer:

- [x] SW-1 -- this spec.
- [x] SW-2 -- `docs/canon/code-bindings.md` (new); `code-identity.md`, `architecture-code.md`, `stack.md`, `spec-check.md`; CLI, code, core, model, eval READMEs; 05 s5.3, 07 s2, 08 Phase 3; `CLAUDE.md`.
- [x] SW-3 -- no ADR; `export index`, `check` clean.
- [ ] SW-4 -- the code still cites this spec's headings, kept as pointers; moving them to `code-bindings.md` is a code edit, not done.

## Out of scope

Hashes, lock, sync, verify, bump, fingerprints (levels echoed); DB tables, `symbols`, `unmapped`, `find_symbols`, `show --bindings`, MCP, HTTP, UI; `cargo metadata`; layers B, C, Bevy; `@assumes` vs the queue (slice 9); `unbound`; Health; check findings; the gate's file -> node map; writing markers (ADR-0016); `[code]` here; spec-side referrers (`show --links`).

## Open

Q1 (a node without `rev` is revision 1) and Q2 (mentions from comments only, never string literals) were decided under the owner's pre-approval; the owner's review of them and of WA-1..WA-14 is open. Carried to slice 3: `FileAnalysis::hashed`'s doc ("Every item was hashed") is wrong for error items; give the binder an item view without `state` before hashing from it.

## Implementation

| Module | What it does |
|---|---|
| core `code_toml.rs` (new), `gate.rs`, `lib.rs` | `CodeConfig::from_toml` -> `Option` (closed table, `is_excluded`, `excludes_all_below`); `dialect_problem` shared with the zones |
| code `bind.rs` (new) | `bind_file`, `bind_tree` -> `RustBinding`; `Binding`, `Unanchored`, `BoundComment`; one explicit-stack walk |
| code `naming.rs` (new) | `name_files` -> `Names`: `qpath`, ambiguity; `ROOT_PACKAGE` |
| code `items.rs` | `analyze_with_nodes(.., Hashing)`, `ErrorsOnly` for the binder; `FileAnalysis::hashed` |
| code `markers.rs`, `ron/mod.rs`, `lib.rs` | `#` Latin, rev 1-9 digits ending the token, a token ends at an `@keyword`, `token_range`; `comment_anchors`; exports |
| CLI `code_walk.rs` (new) | the walk, notes, gaps, packages (`Manifest`, `read_manifest`), `read_tree` |
| CLI `refs.rs` (new), `show.rs`, `lib.rs`, `main.rs`, `Cargo.toml` | `refs`, rows, states, text and JSON, the cap; `pick_reference`, `latin_text` shared with `show`; `Outcome::Refs`; `specengine-code`, `toml` |

Deviations, accepted in review: 1 rule 5 judged before 1-4, as rustdoc reads `//!` (the draft let rule 2 bind the next item); 2 an `ERROR` node at an item's start or end is `cannot_verify` with its hash category, never a guessed item; 3 rule 1 crosses outer attributes starting on the `*/` line, as rule 2 does; 4 a module shows `path_attribute` as well as `unrooted`, an inline `mod`'s module its item's ambiguity, `duplicate` too: reported, never hidden; 5 a directory an exclude `G/**` covers is not listed, so an unlistable one inside is no gap; 6 smaller: `not_an_item` is the catch-all; an unanchored RON comment's mention belongs to the file (RON has no enclosing item); the summary counts printed rows per block; a `.md` REF without a node exits 1 (`has no node`); the no-table sentence names the config as given; a non-UTF-8 name is counted `skipped` without a note, even under an exclude (it cannot be matched); `cannot_verify` takes the hash's first category.

Iteration 2 (review minors): iteration 1's token ran past `@`, so `@implements @verifies X` lost its `verifies` marker (the eval's RON `markers.json` too): a token now ends at an `@keyword`, and a rev is read only after a non-empty ID (an empty one panicked). A root manifest without `[package] name` took the checkout's directory name (`.::bin:<dir>::main`): now a manifest without `[package]` is virtual (no targets, as `cargo metadata`), an unreadable or nameless one takes the directory's name, `.` for the root (`.::bin:.::main`, kept). The binder hashed every item (~41 % of the walk): now errors only; 457 `.rs` files in 5.3-5.8 s debug, against 9.2 s for the walk alone before. `MACRO_KINDS` keeps only macro calls and `macro_rules!` (an attribute's arguments are `not_an_item`); binding rows built lazily. API note: the binder's error-free items hold `HashState::CannotVerify { categories: [] }` with `hashed == false`, as a new variant would break `hash_traps.rs`'s exhaustive match; only `bind.rs` reads it (pinned).

Spec points decided as built: `#[derive(Debug)] // m` above `struct S;` is `unanchored not_an_item` (not on its own line, no item ends there); a trailing comment on an impl's last associated `type`/`const` is `no_item` (`}` follows). `bevy` stays a reserved key (SpecEngine's 07 s5 key; slice 10 `bevy-dump` replaces it with a neutral `frameworks` key).

Runs: iteration 2 code 195/195, CLI targets 34/34, eval targets 66 + 6 skipped; fmt, clippy, probes clean. The full workspace run (iteration 1, 2 346 tests) left `import_cli` and `layout_cli` red only because they read `git status -- fixtures/` and `fixtures/code-a/` was untracked. A developer run over pilot A (iteration 1, debug, not TE-9): 942 code files (195 `.rs`, 747 `.ron`), one requirement 0 markers and 178 mentions, 11.0 s.
