---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 10a of bevy-dump (WA-1, WA-4, WA-11..WA-14; Q1, Q3, Q8 2026-10-10); refreshed and split 2026-10-16 against shipped 6a-1..6b-1 (S1-S20; L1-L3, F1, F2, J1, G1, E1, C1, D1, Z1, the frozen bytes, SR1-SR6; decided 2026-10-16 under the owner's pre-approval, review open); 05 s5.1; 07 s1.2, s5; 08 s2
adrs: []
---

# bevy-roles (10a-1): framework roles of Rust items in the code index, opt-in `[code] frameworks`

## Why

Neither an agent nor the owner can ask which items hold a framework role (for Bevy: component, resource, message, event, entity event, reflect, system, observer, plugin) or in which group, a Bevy schedule, a system is registered: they open files, and the cost grows with the project. The detector (`specengine-code` `bevy`) reads systems, observers and plugins, but only the eval calls it (`crates/specengine-code/README.md`); derives and `.add_message::<T>()` are read nowhere (05:128).

Delivered: behind the opt-in `[code] frameworks = ["bevy"]`, the detector, extended with derives, `impl Component for` and `add_message`, runs on each `.rs` file's one parse into the code index (`code_roles`); the link from a registration to its item is computed per read; `spec index` counts the roles. Slice 10a is split (D1): **10a-1** (this) and **10a-2** `bevy-roles-reads.md` (role lines in `spec symbols`, `find_symbols`' `roles`, plugin 0.10.0), back to back. 10b `bevy-dump.md` (layer B) is parked (S18). Layer A only: always fresh, no project action. No ADR: ADR-0008 (a neutral key, the words data), ADR-0020 (a role never binds), ADR-0031 (roles are code facts, never node kinds), ADR-0012 applied; `code-bindings.md` "The `[code]` table" already announces the key.

## Description and interactions

- **Owner, agent**: `spec index` prints a `roles <slug>:` line after `code <slug>:` (JSON `code.roles`); per item from 10a-2 (`spec symbols`, `find_symbols`). No new command, flag or MCP tool.
- **Crates** (S1, S13):
  - `specengine-code`: `frameworks` (data: `KNOWN`; per framework its words, sources, categories, per-word item kinds; the fact rows, the subject text), `bevy` extended (derives, `impl Component for`, `add_message`); `CODE_INDEX_FORMAT` 3.
  - core `code_toml.rs`: the key's shape.
  - store: `code_roles` behind 6a-2's `CodeAnalyzer` (plain rows), the role read (L3), `INDEX_FORMAT` 10.
  - CLI: `code_index.rs` (`Stamp.frameworks`, the detector in `Analyzer::rust` on the one parse, the value check), a role module (L1 links, G1 counts; working name `roles.rs`), `refresh.rs` (the line, J1).
  - eval `code-index`: the opt-in (E1).
  - MCP, HTTP, UI, plugin unchanged; no new dependency (`build_graph.rs`); `levels.rs`, `hash.rs`, `ron/` untouched (RD-11).
- **Freshness**: 6a-2's: each code read refreshes first (`docs/canon/symbol-index.md` "Crates and seams", `refresh_code`; S16); role rows are per file, re-derived with their file; links are computed per read (L2).
- **Unchanged**: `refs`, `unmapped`, `symbols` (10a-2 adds its keys), `verify`, `lock`, `health`, `check`, hook, gate, bundle, impact. Without `frameworks` every output is 6b-1's byte for byte but `code.roles: null` in `index --json`. **Writes**: the data directory only.

## Data

### `[code] frameworks`

WA-4, Q3, S7.

```toml
[code]
roots      = ["crates"]
frameworks = ["bevy"]          # optional; absent or [] = off
```

- **Core** (`code_toml.rs`, shape only): a list of strings, each `[a-z][a-z0-9_-]{0,31}`, repeats dropped, byte-sorted, as `CodeConfig.frameworks`, each name's line kept for the CLI's message. Refused at the line, exit 2, the forms of `exclude`'s: ``<config>:<line>: `[code] frameworks` is a <type>, not a list of names``; `` `[code] frameworks` holds a <type>: it lists names``; `` `[code] frameworks`: `<value>` is not a name: a lowercase letter, then at most 31 of a-z, 0-9, `_`, `-` ``.
- **The refusal list changes exactly so**: `KEYS` `["roots", "exclude"]` -> `["roots", "exclude", "frameworks"]`; `NOT_BUILT_KEYS` `["languages", "crates", "bevy", "scip"]` -> `["languages", "crates", "scip"]`. `bevy` falls to the unknown-key arm: ``<config>:<line>: `[code]` has no key `bevy`: it takes `roots` and `exclude` and `frameworks` only``. The module doc drops `bevy`; `code_refs_walk.rs` `EXEMPT` drops `("crates/specengine-core/src/code_toml.rs", "bevy")` (5 -> 4): the stack-word scan proves the core names no framework (11a drops `scip` alike, `layer-c.md` Q11).
- **Values** (`specengine-code` `frameworks::KNOWN`, `["bevy"]`; the CLI asks), checked in `refresh_code`, the key's one reader (SR5), before any row is touched: ``<config>:<line>: `[code] frameworks`: `<name>` is not known: known `bevy` `` (names from data, `` `, ` ``-joined), exit 2 for every code-index reader: `index` after its md index (as a bad `[code]`, `code` `null`), `refs`, `symbols`, `unmapped`, their MCP twins, eval `code-index`. `verify` and `lock` never read the key; `spec check` only name-checks `code`.
- **Stamp** (F2): `Stamp` (`code_index.rs`) gains `frameworks`, always present, sorted, `[]` when absent: `code_fp`'s JSON is `{format, levels, roots, exclude, frameworks}`; a change re-derives every code file.

### Role facts

WA-1, WA-13, Z1. With a framework listed, `Analyzer::rust` runs its detector (`bevy::detect`) on the file's one tree; RON, unreadable and non-UTF-8 files hold none. Per file, in source order:

| Source | Read | Word | Sits on |
|---|---|---|---|
| `derive` | `#[derive(..)]` on a struct, enum or union: each element whose last path segment is `Component`, `Resource`, `Message`, `Event`, `EntityEvent`, `Reflect` (lists, qualified paths, several attributes) | `component`, `resource`, `message`, `event`, `entity_event`, `reflect` | the item; `certain` |
| `derive` | the same inside `#[cfg_attr(<p>, derive(..))]` | as above | the item; `uncertain`, category `cfg_attr` |
| `impl` | `impl Component for T`; `impl Plugin for T` (shipped `ImplPlugin`) | `component`, `plugin` | subject `T`, linked |
| `fn` | a `fn` whose only parameter is `&mut App` (shipped `FnApp`) | `plugin` | the fn; `certain` |
| `call` | `.add_systems(G, ..)` leaves and each `pipe` argument (SR4); `.add_observer(x)`; `.add_plugins(..)` leaves; `.add_message::<T>()` | `system` (group `G`), `observer`, `plugin`, `message` | a path or a factory: its subject, linked; a closure: its enclosing fn, `certain` (SR2) |
| `call` | shipped `Uncertain` rows; an unreadable `add_message` alike | the call's word | nothing; `uncertain`, its category; counted only |

- **Subject** (`specengine-code`, text): the path as written, generic arguments dropped (`tick::<T>` -> `tick`), leading `crate::`, `self::`, `super::` dropped (SR3: no qpath holds them); a factory: its callee; `add_plugins`' `P::default()`, `P { .. }`, `P.set(..)`: `P`; ≤ 128 B + `…`. `subject_name`: its last segment, from the code crate (the CLI splits nothing).
- **As written, never resolved** (layer C, slice 11): a derive by its last segment; `Resource` is not also `component`; a derive in a macro body is not read (its item is no symbol). `add_message` reads as `add_observer`: code exactly, macro tokens name only (`origin` `macro_rules`, `macro_call`). [A] 0.19 has no `add_event`.
- `symbol_ord`: the item a derive or `fn` row sits on, a closure row's innermost enclosing `function_item`, by byte range over `bind_tree`'s items.
- **Data, not code** (WA-11, G1): words, sources, categories and per-word item kinds live in `specengine-code`'s `frameworks`; store and CLI hold none. Item kinds: `system`, `observer` -> `function_item`; `plugin` -> `function_item`, `struct_item`, `enum_item`; the rest -> `struct_item`, `enum_item`, `union_item`, `type_item`.
- `BevyAnalysis` gains fields; `CallSites` and the eval's `bevy-detector` output stay byte for byte (RD-9).

### Table

WA-1, S6. Per file, keyed `(file, ord)`, cascading, a pure function of (bytes, path, package table, `code_fp`) as 6a-2's rows; `FORMAT_TABLES` gains it before `code_files` (children first); `dump.rs` maps `(path, ord)`.

| Table | Key | Columns |
|---|---|---|
| `code_roles` | `(file, ord)`; index `code_roles_by_subject_name` | `word`, `source`, `form` (`path`, `closure`, `factory`, NULL), `origin` (`code`, `macro_rules`, `macro_call`), `group_text` (whitespace dropped, ≤ 128 B + `…`), `symbol_ord`, `subject`, `subject_name`, `certainty` (`certain`, `uncertain`), `category`, `line` |

Store: `CodeFileRows.framework_roles: Vec<CodeRoleRow>` (SR6: `role` already names the file's package role, `ROLE_SHARED`), applied and cascaded as the others; strings only.

### Formats

F1, S2-S5.

- `INDEX_FORMAT` 9 -> 10; store `tests/format_history.txt` gains `10 <hash>` over spec-a, spec-b and `code_stub`'s rows, the stub emitting role rows; the four pins of 9 move to 10: store `tests/format.rs` (:423, :454), CLI `tests/bundle_config.rs` (:304, :326).
- `CODE_INDEX_FORMAT` 2 -> 3; CLI `tests/code_format_history.txt` gains `3 <hash>`: from 3 on, the BLAKE3 of `fixtures/code-index`'s code-table dump followed by `fixtures/code-bevy`'s (roots masked; the header says so); lines 1, 2 verbatim. `code-index` stays without `frameworks`.

### Links

L1, L2, L3, S8, S9. Per read, never stored.

- **Store** (strings only, no word): role rows with their file's `path` and `package` (`code_files.package`); for each row with a subject, its candidates: the worktree's symbols whose `name` = `subject_name` and whose `item_kind` is among the kinds the CLI passes for the row's word, each with its file's `package` and its tier for `subject`: `exact` (`qpath` = subject), `suffix` (a stored suffix = subject), else `name`; a cfg twin at the tier `subject ‖ ' #' ‖ discriminator` reaches (as `TWIN_SQL`, no split). Index searches only (`symbols_by_name`, then the candidate's key); one deferred transaction (in 10a-2, `code_snapshot`'s).
- **CLI** (L1): the candidates in the row's own package if any, else all; of those, the ones at the best tier; one: `linked`; several: `ambiguous` on each, with their count; none: `unlinked`, counted only (`DefaultPlugins`, a dependency's system). A row without a subject is `certain` or `uncertain` (its `certainty`) on its `symbol_ord`, or on nothing.

### spec index

J1, G1, S10. After `code <slug>:`, with `frameworks` set (figures: TE-1's `expected.json`):

```
code code-bevy: walked 9, parsed 9, unchanged 0, removed 0, unreadable 0, skipped 0, symbols 41, markers 2, mentions 0
roles code-bevy: component 3, entity_event 1, event 1, message 2, observer 1, plugin 3, reflect 2, resource 1, system 7; subjects 14 (linked 11, ambiguous 1, unlinked 2), uncertain 1
```

- `<word> <n>`: the symbols holding a row of the word `certain`, `linked` or `ambiguous`; every word of the listed frameworks (data), byte order, zeros too. `subjects`: rows with a subject, by state (SR1). `uncertain`: rows `uncertain`. Over the stored rows, a gap too.
- JSON `code`: a CLI wrapper, `CodeReport` flattened plus `roles` `{words: {<word>: n}, subjects: {total, linked, ambiguous, unlinked}, uncertain}`; `null` and no line without `frameworks`. No `[code]`: `code` `null` as now.

### Eval

E1, S13. `code-index` keeps the scheme's `frameworks`; under a pilot label `SPECENGINE_CODE_FRAMEWORKS_A` / `_B` (`harness::PILOT_CODE_FRAMEWORKS`, comma-separated; unset or empty: off) join `SPECENGINE_CODE_ROOTS_A` / `_B`'s table. `result` gains `framework_roles` (SR6; `null` off), the `code.roles` object of the last refresh, and `roles_ms`, one role count read after the refreshes, links computed. `bevy-detector` unchanged.

## Rules and edge cases

- WHEN any sequence of `.rs`, manifest, `[code]` (`frameworks` too), `[ids]` or stamp changes is indexed, the index dump SHALL equal a fresh rebuild (6a-2's rule, `code_roles` in).
- WHEN `frameworks` is absent or empty, the detector SHALL NOT run, `code_roles` SHALL hold no row, and every output SHALL equal 6b-1's but `code.roles: null`.
- WHEN a subject names several items at its best tier, its role SHALL count on each as `ambiguous`, never on a picked one (`code-identity.md`: reported, never guessed); WHEN its item is renamed or removed, the next read SHALL show the row `unlinked` or re-linked, the row's own file not re-derived.
- A role never binds, maps a unit (`unmapped`), enters `spec.lock`, a verify row, a check finding or a node kind (ADR-0020, ADR-0031); nothing blocks (ADR-0012): an unknown framework is a config error, a role never changes an exit.
- Determinism: two roots, cwd, reverse creation, CRLF: byte-identical `index` text and JSON; nothing follows rowid.
- Genre (`architecture.md#universal`): no stack word, role word, source word, framework name or method, no `::` split in core, store or CLI sources; `specengine-code`'s `frameworks` and `bevy` name the framework and stay outside `NEW_MODULES`; `code-bevy`'s `[ids]` kinds avoid `role`, `group`, `state`. A second framework or a Bevy 0.20 gate is data, never a key or a CLI branch.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- derives. M: first element only -> `derive_list_yields_every_role`; qualified path dropped -> `qualified_derive_path_is_read`; `cfg_attr` certain -> `cfg_attr_derive_is_uncertain`; a second `#[derive]` ignored -> `every_derive_attribute_is_read`; `Debug` given a role -> `other_derives_name_no_role`.
- [ ] RD-2 -- `add_message`, `impl Component for`, subjects (Z1). M: method not read -> `add_message_registers_its_type`; macro tokens resolved -> `add_message_in_macro_tokens_is_name_only`; generics kept in `subject` -> `add_message_subject_drops_generic_arguments`; `impl Component for` not read -> `impl_component_links_its_type`; `crate::` kept -> `crate_prefixed_subject_meets_its_suffix` (SR3); a `pipe` argument dropped -> `piped_system_is_a_row` (SR4).
- [ ] RD-3 -- storage, stamp, formats (F1, F2). M: a link stored or a cross-file pass -> TE-2 and `renaming_a_system_unlinks_its_registration`; `frameworks` outside the stamp -> `frameworks_change_rederives_roles`; the key left out when empty -> `stamp_always_holds_frameworks`; the detector run without the opt-in -> `no_roles_without_frameworks`; `code_roles` outside `FORMAT_TABLES` -> 6a-2's `format_change_recreates_code_tables_and_keeps_queue_rows`; the table added under format 9 -> store `format.rs` (`10 <hash>`); a role rule changed, or `CODE_INDEX_FORMAT` kept at 2 -> `code_format_history_pins_the_code_index_dump` (`3 <hash>`, code-index then code-bevy).
- [ ] RD-4 -- config. M: `bevy` kept in `NOT_BUILT_KEYS` -> `code_table_unknown_key_is_refused` (new case `bevy = true`, the unknown-key text) and the stack-word scan (`EXEMPT` 4); an unknown framework accepted -> `unknown_framework_is_exit_2_naming_the_line`; a bad shape accepted -> `frameworks_shape_is_exit_2_naming_the_line`; the value checked after rows are written -> `unknown_framework_leaves_code_rows_untouched`; a value list in the core -> the stack-word scan; `spec check` failing on the key -> `code_table_unknown_key_is_refused`'s name-check half.
- [ ] RD-5 -- links (L1, L3). M: 6a-3's union lookup -> `qualified_subject_links_at_its_best_tier`; a plain best tier, twins not lifted -> `bare_subject_beside_cfg_twins_is_ambiguous`; the first candidate taken -> `subject_naming_two_items_is_ambiguous_on_each`; item kinds ignored -> `system_subject_skips_a_same_named_struct`; own package not first -> `own_package_link_wins`; the package cut from a qpath -> the genre scan (`::`); a closure linked by name -> `closure_registration_sits_on_its_enclosing_fn` (SR2); an `unlinked` row on a symbol -> `unlinked_rows_are_counted_only`.
- RD-6 -- moved to `bevy-roles-reads.md` RD-6 (D1).
- [ ] RD-7 (the index half; the `symbols` half: `bevy-roles-reads.md`) -- annotation only, the `spec index` line (J1, G1). M: a role maps a unit or alters a row -> `roles_change_no_refs_unmapped_verify_or_lock_output` (code-bevy with `frameworks` on and off: `refs`, `unmapped`, `verify` output and a scratch `lock accept`'s lock byte-identical; `index` differs only by the `roles` line and `code.roles`); rows counted, not symbols -> `role_counts_count_symbols`; a zero word left out -> `role_counts_list_every_word`; `code.roles` not `null` without the opt-in -> `index_roles_are_null_without_frameworks`.
- [ ] RD-8 -- genre. M: a role word or source literal (`"component"`, `"call"`) or a framework name in a core, store or CLI module -> `new_modules_hold_no_fixture_prefix_kind_path_or_stack_word`, `NEW_MODULES` 11 + 10a-1's store and CLI role modules, its banned words + every word and source the code crate's `frameworks` data lists (read from the data; `EXEMPT_WORDS` for a neutral use).
- [ ] RD-9 -- detector intact. M: a shipped registration changed -> `bevy_detector.rs`; the eval printing the new facts -> eval `bevy_cli.rs` on `bevy-mini`, byte-identical.
- RD-10 -- `bevy-roles-reads.md` (MCP, plugin).
- [ ] RD-11 -- frozen bytes (owner, 2026-10-16). On 10a-1's commit `git diff --stat 4deb4dd -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing and `git diff 4deb4dd -- Cargo.lock` holds no `tree-sitter` line. M (run once, reverted): one byte of recipe input in `levels.rs` -> 5a-1b's stream pin and the `hash_levels_*` goldens red.

test-engineer:

- [ ] TE-1 -- `fixtures/code-bevy` (WA-12, Z1; parsed only, no Bevy dependency, never built; `bevy-mini` untouched): `specengine.toml` with `[ids]` (kinds without `role`, `group`, `state`), `[code]`, `frameworks = ["bevy"]`; a lib and a bin package; every derive form (list, qualified, two attributes, `cfg_attr`); `impl Component for`; `add_message::<T>()` in code and in a `macro_rules!` body; one generic system registered from one site for 3 types; a closure, a pipe, a factory, a macro-registered system, a nested `fn`; cfg twins beside a bare-named registration; a `crate::`- and a module-qualified subject beside a same-named fn elsewhere; a same-named struct and fn; `impl Plugin for`, an `&mut App` fn, `DefaultPlugins`; two markers on role-holding items. `expected.json`: every role row, state, link, count, the `spec index` line.
- [ ] TE-2 -- seeded sequences of `.rs` edits, adds, removes, renames, `frameworks` on and off, stamp changes: after each step dump = rebuild; each RD-3 mutation red.
- [ ] TE-3 (the `index` half; the `symbols` half: `bevy-roles-reads.md`) -- determinism: two roots, cwd, reverse creation, CRLF: `index` text and JSON byte-identical.
- [ ] TE-4 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- TE-5 -- moved to `bevy-roles-reads.md` TE-5 (D1).
- [ ] TE-6 -- regression: `bevy_detector.rs`, eval `bevy_cli.rs`, `code_refs_walk.rs`, `build_graph.rs` green; the two history lines (`10`, `3`) and the four pins at 10 (S5); `spec-c`, `code-index`, `code-a`: every output 6b-1's but `code.roles: null`.
- [ ] TE-7 -- pilots, the owner's manual run (`code_index_pilot.rs`, `#[ignore]`, release, `--timeout`, foreground, scratch `HOME`, read-only, counts only, no name or path committed; E1's variables): `framework_roles` (rows per word, subjects by state, uncertain), `roles_ms`; `full_ms`, `one_file_ms`, `noop_ms` with and without the opt-in against 08 AC-10 (`one_file_ms` > 200: 6c); warm `spec index` against `mcp-read.md` "Latency"'s 1 s (S20; `find_symbols`: `bevy-roles-reads.md` TE-8).

spec-writer:

- [x] SW-1 -- this spec (2026-10-10); refreshed and split 2026-10-16 (D1): `bevy-roles-reads.md`; 08 s2's order; `verify-node.md` 0.11.0 (P1); the pointers in 07 :30, `bevy-dump.md`, `layer-c.md`, `code-tools.md`.
- [ ] SW-2 -- at shipping, bytes measured 2026-10-16 at f9a80ff (Tier 2 cap 12 288, Tier 1 10 240):
  - a Tier 2 `docs/canon/code-roles.md` (C1: key, values, facts, table, links, the `spec index` line, the eval keys; about 5-7 KB), its line in `docs/index-canon.md` (8 550: about 170);
  - `code-bindings.md` (10 549: "The `[code]` table", `frameworks`, `bevy` unknown; about +150 net);
  - `symbol-index.md` (12 119, **169 left**): "Tables" format 10, nine tables, a `code_roles` row pointing at `code-roles.md`; `code_fp`'s `frameworks`; "Analyzer stamp" 3, the CLI line over code-index then code-bevy, the store's line 10; an "AC-10 measurement" pointer for `framework_roles`, `roles_ms`: about 175 B, so room first: the stamp's 2 (6b-1) clause cut to its rule, about -70;
  - READMEs: code (9 259: `frameworks`, `bevy`'s new readings; about 300); store (10 236, **4 left**; room first: "Cache key and format stamp"'s history clause to a pointer at `symbol-index.md` "Analyzer stamp", which states it, about -45; then `INDEX_FORMAT = 10`, `code_roles` among the reads, about +10); CLI (9 578: the role module, the `spec index` line; about 200); eval (10 223, **17 left**: `SPECENGINE_CODE_{ROOTS,FRAMEWORKS}_A`, +13; the rest in `code-roles.md`); core none;
  - 05 s5.1 :128 (built); 07 :207 (`bevy = true` -> `frameworks = ["bevy"]`; S15); 08 s2; `CLAUDE.md` State (5 772 of 16 384).
- [x] SW-3 -- no ADR (ADR-0048 is 10b's); `export index`, `check` clean.

## Risks

- R1 (Q1): an owner expecting derives from the dump makes the split rework. R2: a framework key or word leaks into core, store or CLI (RD-4, RD-8).
- R5: Bevy 0.20 changes events and observers (05:130): `event`, `entity_event` and observer reading go behind a version gate, a framework's data, never a new key.
- R9: one detector pass per `.rs` file per refresh and links per read (`roles_ms`) against `one_file_ms` ≤ 200 (TE-7; else 6c).
- Text only (stated limits; layer C): a project's own `Component` derive or `add_message` method reads as Bevy's; `Self::x`, a crate-name path or a `use` alias meets no suffix and links by name, own package first.
- **Frozen bytes**: 10a-1 edits `specengine-code` before 5b-1 freezes `levels.rs`, `hash.rs`, `ron/` (RD-11; `lock-levels.md` "Risks"). **Caps**: the store README, the eval README and `symbol-index.md` need room first (SW-2).

## Out of scope

Role lines in `spec symbols`, `find_symbols`' `roles`, the plugin (10a-2); dump facts, groups as dumped, instances (10b, parked); a `--role` filter, a new form or tool; resolved types (layer C, slice 11); `#[require(..)]`, `#[reflect(..)]`, sets, conditions, ordering; roles in `show`, bundle, impact, health, `unmapped`, verify, lock, check; a node's inferred layer (05:128); HTTP, UI; a Bevy 0.20 gate; a framework but `bevy`; writing markers.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 derives, `add_message` | layer A (this); 0.19's dump has no derive or message field, all messages share one update system | a dump schema and a producer per Bevy minor, for facts the source states exactly |
| Q3 keys | `[code] frameworks = ["bevy"]`; the core checks the shape, `specengine-code` the values; 07 :207's `bevy = true` replaced at shipping | a framework word in the core's closed table, one key per framework |
| Q8 ADR | none here; ADR-0048 is 10b's | -- |
| Q9 10b's timing | superseded (S18, owner): 10b parked, its consumer (slice 12) gone | a review now for ≤ 6.6 % of own systems |
| L1 link rule | "Links" | 6a-3's union: `stats::regen` ambiguous with every `regen`; a plain best tier: a bare `run` links elsewhere beside its cfg twins, a guess |
| L2 stored links | none, per read ("Links") | a second cross-file pass beside `twin_files`; TE-2 doubles |
| L3 package | `code_files.package` from the store ("Links") | no preference, more `ambiguous`; or a qpath split in the CLI, which the scan forbids |
| F1 formats | "Formats" | code-index alone: a role rule changed under a kept stamp stays green; code-index opted in: every 6a-3, 6b-1 pin churns |
| F2 stamp | "`[code] frameworks`" | a conditional key saves nothing and adds a branch |
| J1 index JSON | "spec index" | `CodeReport.roles`: counting rules and link states in the store |
| G1 counts | the code data decides which rows carry a subject; the CLI counts them | `source == "call"` in a CLI module |
| E1 eval | "Eval" | timing `spec index --full`: figures not comparable with AC-10 |
| C1 canon home | `code-roles.md` (SW-2) | `symbol-index.md` over its cap at the first row |
| D1 split (owner) | 10a-1 (this), 10a-2 `bevy-roles-reads.md` | one L slice, 9 RD under the three-iteration cap |
| Z1 trim (owner) | keep `impl Component for` and `cfg_attr` (fixture-only, cheap) | 0 pilot rows lost today; a stated limit |
| Frozen bytes (owner) | RD-11 | TE-1's figures stop describing the bytes 5b-1 freezes |
| SR1 `subjects` | the count of rows with a subject by state (was `registrations`) | `impl` rows, which have a subject, counted as registrations |
| SR2 closure rows | `certain` on the enclosing fn: no lookup | `linked` without a link |
| SR3 `crate::`, `self::`, `super::` | dropped from `subject` | the commonest written form meets no suffix, links by name |
| SR4 `pipe` | each argument a `system` row of its group | piped systems hold no role |
| SR5 value check | in `refresh_code`, the key's one reader | five `CodeConfig` call sites wired; a typo stops `[verify]` CI |
| SR6 names | `framework_roles` (`CodeFileRows`, eval `result`) | beside `role` (a file's package role) and eval's `roles`, two meanings of one word |

Q1, Q3, Q8 (2026-10-10) and L1-SR6 (2026-10-16) decided under the owner's pre-approval, review open. WA-n: the 2026-10-10 analysis' working answers: WA-1 [P] derives and `add_message` are not-built layer A items, per-file rows keep the index pure; WA-4 [P] `[code]` closed; WA-11 [P] the genre scan; WA-12 a new fixture; WA-13 [A] as "Role facts"; WA-14 a format bump per part. S-n: the 2026-10-16 refresh's stale points, each fixed where cited; SR-n: the spec-writer's refinements.

## Dependencies and ordering

- **Needs**, all shipped (S1): 1 `code-refs` (`[code]`, the walk); 6a-2 `symbol-index` (tables, `CodeAnalyzer`, `code_fp`, `refresh_code`, `CodeReport`, `FORMAT_TABLES`, `dump.rs`, `code_stub`, eval `code-index`); 6a-3 `symbol-reads` (`symbols_by_name`, suffixes, `SymbolTier`); 6b-1 `code-tools` (`CODE_INDEX_FORMAT` 2, one snapshot per read); 5a-1 (`specengine-code` `lib.rs`).
- **Order** (08 s2, 2026-10-16): 6b-1 (shipped) -> **10a-1** -> 10a-2 -> 8a-1 -> 9a -> 12a -> (11a); 5b-1 at the next slice boundary once 5a-2's TE-1 lands. No file shared with 5b-1 (`lock-levels.md`: no code-crate change). 11a edits `NOT_BUILT_KEYS`, `EXEMPT` from 10a-1's state and takes the next `INDEX_FORMAT`. 10b parked. 6c only if TE-7 breaks AC-10.
- **Assumptions** (checked 2026-10-16 at f9a80ff): `INDEX_FORMAT` 9, `CODE_INDEX_FORMAT` 2, `Stamp {format, levels, roots, exclude}`, `KEYS`, `NOT_BUILT_KEYS`, `EXEMPT` 5, `NEW_MODULES` 11 as cited; the frozen paths unchanged since 4deb4dd. Pilots (read-only, counts): both pin bevy 0.19; `add_message::<` lines A 146 (65 files), B 21 (9); `add_event::<` 0; one-line role derives A 334, B 196; `cfg_attr(..derive(..))`, `impl Component for` 0.

## Implementation

Not started. The split (2026-10-16, D1): RD-1..RD-5, RD-8, RD-9, TE-1, TE-2, TE-4, TE-6, TE-7 kept; RD-6, TE-5 -> `bevy-roles-reads.md`; RD-7, TE-3 halved; RD-6's `role_counts_count_symbols` -> RD-7 here; RD-10 (there), RD-11 new; SW-1..SW-3 kept. To record: modules, TE-7's figures, deviations with reasons.
