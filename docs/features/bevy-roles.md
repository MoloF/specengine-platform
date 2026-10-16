---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 10a-1 of bevy-dump (WA-1, WA-4, WA-11..WA-14; Q1, Q3, Q8); refreshed and split 2026-10-16 (D1; S1-S20, L1-L3, F1, F2, J1, G1, E1, C1, Z1, SR1-SR6); 05 s5.1; 07 s1.2, s5; 08 s2; Rust 2 iterations, review accepted, tests 2 rounds; TE-7 the owner's
shipped: 2026-10-16
adrs: []
---

# bevy-roles (10a-1): framework roles of Rust items in the code index, opt-in `[code] frameworks`

## Why

Neither an agent nor the owner could ask which items hold a framework role (for Bevy: component, resource, message, event, entity event, reflect, system, observer, plugin) or in which group, a Bevy schedule, a system is registered: they opened files, and the cost grew with the project. The detector (`specengine-code` `bevy`) read systems, observers and plugins for the eval only; derives and `.add_message::<T>()` were read nowhere (05:128).

Delivered: behind the opt-in `[code] frameworks = ["bevy"]`, the detector, extended with derives, `impl Component for` and `add_message`, runs on each `.rs` file's one parse into the code index (`code_roles`); a registration's link to its item is computed per read; `spec index` counts the roles. 10a-2 `bevy-roles-reads.md` adds them to `spec symbols` and `find_symbols`; 10b `bevy-dump.md` is parked. No ADR: ADR-0008 (a neutral key, the words data), ADR-0020 (a role never binds), ADR-0031 (code facts, never node kinds), ADR-0012 applied.

## Description and interactions

Canon: `docs/canon/code-roles.md` (all of it); `code-bindings.md` "The `[code]` table"; `symbol-index.md` "Crates and seams", "Tables", "Cache key and cross-file pass", "Analyzer stamp", "AC-10 measurement"; the code, store, CLI and eval READMEs; 05 s5.1, 07 s5.

## Data

`docs/canon/code-roles.md`: "`[code] frameworks`" (WA-4, Q3, S7, SR5, F2), "Role facts" (WA-1, WA-13, Z1, SR2-SR4), "Table" (WA-1, S6, SR6), "Links" (L1 as amended by B1, L2, L3), "spec index" (J1, G1, SR1), "Eval" (E1, B4); formats: `docs/canon/symbol-index.md` "Analyzer stamp" (F1, S2-S5).

## Rules and edge cases

- WHEN any sequence of `.rs`, manifest, `[code]` (`frameworks` too), `[ids]` or stamp changes is indexed, the index dump SHALL equal a fresh rebuild, `code_roles` in.
- WHEN `frameworks` is absent or empty, the detector SHALL NOT run, `code_roles` SHALL hold no row, every output SHALL equal 6b-1's but `code.roles: null`.
- WHEN a subject names several items at its best tier (own package the tie-break), its role SHALL count on each as `ambiguous`; WHEN its item is renamed or removed, the next read SHALL show the row `unlinked` or re-linked, its own file not re-derived.
- A role never binds, maps a unit, enters `spec.lock`, a verify row, a check finding or a node kind; nothing blocks. Determinism: two roots, cwd, reverse creation, CRLF give byte-identical `index` text and JSON. Genre: canon opening.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [x] RD-1 -- derives. M: first element only -> `derive_list_yields_every_role`; qualified path dropped -> `qualified_derive_path_is_read`; `cfg_attr` certain -> `cfg_attr_derive_is_uncertain`; a second `#[derive]` ignored -> `every_derive_attribute_is_read`; `Debug` given a role -> `other_derives_name_no_role`.
- [x] RD-2 -- `add_message`, `impl Component for`, subjects (Z1). M: method not read -> `add_message_registers_its_type`; macro tokens resolved -> `add_message_in_macro_tokens_is_name_only`; generics kept in `subject` -> `add_message_subject_drops_generic_arguments`; `impl Component for` not read -> `impl_component_links_its_type`; `crate::` kept -> `crate_prefixed_subject_meets_its_suffix` (SR3); a `pipe` argument dropped -> `piped_system_is_a_row` (SR4).
- [x] RD-3 -- storage, stamp, formats (F1, F2). M: a link stored or a cross-file pass -> TE-2 and `renaming_a_system_unlinks_its_registration`; `frameworks` outside the stamp -> `frameworks_change_rederives_roles`; the key left out when empty -> `stamp_always_holds_frameworks`; the detector run without the opt-in -> `no_roles_without_frameworks`; `code_roles` outside `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`; the table added under format 9 -> store `format.rs` (`10 <hash>`); a role rule changed, or `CODE_INDEX_FORMAT` kept at 2 -> `code_format_history_pins_the_code_index_dump` (`3 <hash>`, code-index then code-bevy).
- [x] RD-4 -- config. M: `bevy` kept in `NOT_BUILT_KEYS` -> `code_table_unknown_key_is_refused` (`bevy = true`, the unknown-key text) and the stack-word scan (`EXEMPT` 4); an unknown framework accepted -> `unknown_framework_is_exit_2_naming_the_line`; a bad shape accepted -> `frameworks_shape_is_exit_2_naming_the_line`; the value checked after rows are written -> `unknown_framework_leaves_code_rows_untouched`; a value list in the core -> the stack-word scan; `spec check` failing on the key -> `code_table_unknown_key_is_refused`'s name-check half.
- [x] RD-5 -- links (L1 with B1, L3). M: 6a-3's union lookup -> `qualified_subject_links_at_its_best_tier`; a plain best tier, twins not lifted -> `bare_subject_beside_cfg_twins_is_ambiguous`; the first candidate taken -> `subject_naming_two_items_is_ambiguous_on_each`; item kinds ignored -> `system_subject_skips_a_same_named_struct`; own package not the tie-break within the best tier -> `own_package_link_wins`; the package cut from a qpath -> the genre scan (`::`); a closure linked by name -> `closure_registration_sits_on_its_enclosing_fn` (SR2); an `unlinked` row on a symbol -> `unlinked_rows_are_counted_only`.
- RD-6 -- moved to `bevy-roles-reads.md` RD-6 (D1).
- [x] RD-7 (the index half) -- annotation only, the `spec index` line (J1, G1). M: a role maps a unit or alters a row -> `roles_change_no_refs_unmapped_verify_or_lock_output`; rows counted, not symbols -> `role_counts_count_symbols`; a zero word left out -> `role_counts_list_every_word`; `code.roles` not `null` without the opt-in -> `index_roles_are_null_without_frameworks`.
- [x] RD-8 -- genre. M: a role word or source literal or a framework name in a core, store or CLI module -> `new_modules_hold_no_fixture_prefix_kind_path_or_stack_word` (`NEW_MODULES` + both role modules; banned words read from the `frameworks` data).
- [x] RD-9 -- detector intact. M: a shipped registration changed -> `bevy_detector.rs`; the eval printing the new facts -> eval `bevy_cli.rs` on `bevy-mini`, byte-identical.
- RD-10 -- `bevy-roles-reads.md` (MCP, plugin).
- [x] RD-11 -- frozen bytes (owner). `git diff --stat 4deb4dd -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing; `git diff 4deb4dd -- Cargo.lock` holds no `tree-sitter` line. M (once, reverted): one byte of recipe input in `levels.rs` -> 5a-1b's stream pin and the `hash_levels_*` goldens red.

test-engineer:

- [x] TE-1 -- `fixtures/code-bevy` (parsed only, never built; `bevy-mini` untouched): a lib and a bin package, every derive form, `impl Component for`, `add_message::<T>()` in code and in `macro_rules!`, a generic system for 3 types, a closure, a pipe, a factory, a macro-registered system, a nested `fn`, cfg twins, `crate::`- and module-qualified subjects, a same-named struct and fn, `impl Plugin for`, `&mut App` fns, `DefaultPlugins`, two markers; `expected.json`: every row, state, link, count, the `spec index` line.
- [x] TE-2 -- seeded sequences of `.rs` edits, adds, removes, renames, `frameworks` on and off, stamp changes: after each step dump = rebuild; each RD-3 mutation red.
- [x] TE-3 (the `index` half) -- determinism: two roots, cwd, reverse creation, CRLF: `index` text and JSON byte-identical.
- [x] TE-4 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- TE-5 -- moved to `bevy-roles-reads.md` TE-5 (D1).
- [x] TE-6 -- regression: `bevy_detector.rs`, eval `bevy_cli.rs`, `code_refs_walk.rs`, `build_graph.rs` green; the history lines `10`, `3` and the four pins at 10; `spec-c`, `code-index`, `code-a`: every output 6b-1's but `code.roles: null`.
- [ ] TE-7 -- pilots, the owner's manual run (`code_index_pilot.rs` `te7_*`, `#[ignore]`, release, scratch `HOME`, read-only, counts only): `framework_roles`, `roles_ms`; `full_ms`, `one_file_ms`, `noop_ms` with and without the opt-in against 08 AC-10 (`one_file_ms` > 200: 6c); warm `spec index` against `mcp-read.md` "Latency"'s 1 s.

spec-writer:

- [x] SW-1 -- this spec (2026-10-10); refreshed and split 2026-10-16 (D1): `bevy-roles-reads.md`; 08 s2's order; the pointers in 07, `bevy-dump.md`, `layer-c.md`, `code-tools.md`.
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/code-roles.md` (C1) and its index line; `code-bindings.md`, `symbol-index.md` (room first: the 6b-1 stamp clause cut to its rule); the code, store (room first: its history clause to a pointer), CLI, eval READMEs; 05 s5.1, 07 s5, 08 s2; `CLAUDE.md` State; `bevy-roles-reads.md` "Carry-overs".
- [x] SW-3 -- no ADR (ADR-0048 is 10b's); `export index`, `check` clean.

## Out of scope

Role lines in `spec symbols`, `find_symbols`' `roles`, the plugin (10a-2); dump facts, groups as dumped, instances (10b, parked); a `--role` filter; resolved types (layer C, slice 11); `#[require(..)]`, `#[reflect(..)]`, sets, conditions, ordering; roles in `show`, bundle, impact, health, `unmapped`, verify, lock, check; a node's inferred layer; HTTP, UI; a Bevy 0.20 gate; a framework but `bevy`; writing markers.

## Open

Decided under the owner's pre-approval: Q1 derives and `add_message` in layer A; Q3 `[code] frameworks`, shape in the core, values in `specengine-code`; Q8 no ADR; Q9 10b parked (S18); L1 the link rule (B1 below), L2 links per read, L3 `code_files.package`; F1 two format bumps, code-index then code-bevy; F2 `frameworks` always in the stamp; J1 a CLI JSON wrapper; G1 the data decides, the CLI counts; E1 the eval opt-in; C1 `code-roles.md`; D1 the split; Z1 `impl Component for`, `cfg_attr` kept; SR1 `subjects` by state; SR2 a closure row `certain` on its enclosing fn; SR3 `crate::`, `self::`, `super::` dropped; SR4 each `pipe` argument a `system` row; SR5 the value check in `refresh_code`; SR6 `framework_roles`. Review 1 (2026-10-16): B1-B4, "Implementation".

## Implementation

| Module | What it does |
|---|---|
| code `frameworks.rs` (new), `lib.rs` | the data (`KNOWN`, `BEVY`), `role_facts`; `CODE_INDEX_FORMAT` 3 |
| code `bevy/{mod,expr,tokens}.rs` | derives, `cfg_attr`, `impl Component for`, `add_message`, `Subject`, `Piped`, `start`; shipped lists unchanged |
| core `code_toml.rs` | `frameworks`' shape and lines; `bevy` an unknown key |
| store `schema.rs`, `dump.rs`, `code.rs`, `lib.rs` | `code_roles`, `FORMAT_TABLES`, the dump, `CodeRoleRow`; `INDEX_FORMAT` 10 |
| store `roles.rs` (new) | `code_roles(kinds)`, `roles_in`, `CANDIDATES_SQL` |
| CLI `roles.rs` (new) | `state` (links), `role_counts`, `render_line` |
| CLI `code_index.rs` | `Stamp.frameworks`, facts on the one parse, `known_frameworks`, `refresh_code`'s `config_label` |
| CLI `refresh.rs`, `refs.rs`, `symbols.rs`, `unmapped.rs`, `lib.rs` | the `roles` line, `CodeJson`; the label passed; exports |
| eval `harness.rs`, `code_index.rs`, `main.rs` | `PILOT_CODE_FRAMEWORKS`, the opt-in, `framework_roles`, `roles_ms` |

The developer's readings (row order by first byte, `line`, `form`, `symbol_ord`, macro subjects, the new `uncertain` rows, `pipe` under `add_observer`, the unknown framework's routing, the eval label) are canon; the tester's L1-L24 hold but L15: `lock accept` refuses `--config`, so RD-7's test toggles the opt-in another way.

Decided after review 1 (owner pre-approved, iteration 2):

- B1 (L1 amended): candidates at the best tier first, own package the tie-break within it; an own-package name match no longer beats another package's suffix match.
- B2: `add_plugins(a::make_plugin())` is a `factory`, the whole callee its subject; a qualifier is the type only when its last segment starts uppercase.
- B3: a derive attribute with a parse error is an `uncertain` `parse_error` row, an unclosed token `add_message` turbofish `uncertain` `arguments` (both were silent).
- B4: the eval refuses (exit 2) a frameworks variable without its roots variable (was ignored).
- A non-path qualifier keeps single spaces (`<T as Tr>::f`, was `<TasTr>::f`).

Iteration 2 also closed review 1's other nits; re-review 2 accepted it. B5 (a spaced `<T as Tr>` qualifier rendered as `impl_label`) and the comments citing this spec go to `bevy-roles-reads.md` "Carry-overs". RD-11 held. Tests: round 1 2938/2944, the 6 red the B1-B4 targets by design, 8 mutations and RD-11's killed; round 2 2944/2946, the 2 red the untracked-fixture guards, 6 iteration-2 mutations killed. TE-7, the owner's pilot run, is pending.
