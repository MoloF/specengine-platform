---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, fixtures]
ref: Phase 3 plan 2026-10-10, slice 10a of bevy-dump (WA-1, WA-4, WA-11..WA-14; Q1, Q3, Q8, Q9 decided under the owner's pre-approval, review open); 05 s5.1; 07 s1.2, s5; 08 s2
adrs: []
---

# bevy-roles: framework roles of Rust items in the code index, opt-in `[code] frameworks`

## Why

Neither an agent nor the owner can ask which items hold a framework role (for Bevy: component, resource, message, event, entity event, reflect, system, observer, plugin) or in which group, a Bevy schedule, a system is registered: they open files, and the cost grows with the project. The detector (`specengine-code` `bevy`) reads systems, observers and plugins, but only the eval calls it (`crates/specengine-code/README.md`); derives and `.add_message::<T>()` are read nowhere (05:128).

Slice 10 is split (Q9): **10a** (this) runs the detector, extended with derives and `add_message`, on each code file into 6a's index behind the opt-in `[code] frameworks = ["bevy"]`, shows roles in `spec symbols` (so 6b-1's `find_symbols`) and counts them in `spec index`; **10b** (`bevy-dump.md`) adds the project's own schedule dump as annotation. Layer A only: always fresh, no project action. No ADR: ADR-0008 (a neutral key, the words data), ADR-0020 (a role never binds), ADR-0031 (roles are code facts, never node kinds) applied; ADR-0048 is 10b's.

## Description and interactions

- **Owner, agent**: `spec symbols regen` (`find_symbols {query}`) shows `role system | a | crates/game/src/lib.rs:20 | call | Update | linked` under the item; `spec index` prints role counts. No new command, form or MCP tool.
- **Crates**: `specengine-code` the derive and `add_message` reading, `frameworks` (names, role words, sources, per-word item kinds: data), the subject text; core `code_toml.rs` the key's shape; store `code_roles` behind 6a's `CodeAnalyzer` (plain rows); CLI the value check, per-read links, output; MCP `mirror.rs` the `roles` keys. No new dependency (`build_graph.rs`).
- **Freshness**: 6a's (each code read refreshes the index first, `symbol-index.md` WA-6); roles are per-file rows re-derived with their file; links are computed per read.
- **Unchanged** (WA-8): `refs`, `unmapped`, `verify`, `lock`, `health`, `check`, hook, gate, bundle, impact. Without `frameworks` every output is 6a's byte for byte but `roles: null` (`symbols --json`) and `code.roles: null` (`index --json`). **Writes**: the data directory only.

## Data

### `[code] frameworks`

WA-4, Q3.

```toml
[code]
roots      = ["crates"]
frameworks = ["bevy"]          # optional; absent or [] = off
```

- **Core** (`code_toml.rs`, shape only): a list of strings, each `[a-z][a-z0-9_-]{0,31}`, repeats dropped, kept byte-sorted; a wrong type or shape: exit 2, `<config>:<line>: message`. **The refusal list changes exactly so**: `KEYS` `["roots", "exclude"]` -> `["roots", "exclude", "frameworks"]`; `NOT_BUILT_KEYS` `["languages", "crates", "bevy", "scip"]` -> `["languages", "crates", "scip"]`. `bevy` falls to the unknown-key arm: ``<config>:<line>: `[code]` has no key `bevy`: it takes `roots` and `exclude` and `frameworks` only`` (`KEYS` joined as today). The module doc drops `bevy`; `code_refs_walk.rs` `EXEMPT` drops `("crates/specengine-core/src/code_toml.rs", "bevy")` (5 -> 4), so the stack-word scan proves the core names no framework.
- **Values** (`specengine-code` `frameworks::KNOWN`, data; the CLI asks): unknown -> exit 2 at its line, ``[code] frameworks: `<name>` is not known: known `<names>` ``, the names from data. As every `[code]` error, only commands reading code fail (`code-bindings.md` "The `[code]` table").
- In 6a's `code_fp` (the checked table): a change re-derives every code file.

### Role facts

WA-1, WA-13. With `bevy` listed, the detector runs on 6a's tree of each `.rs` file (no second parse). Per file:

| Source | Read | Word | Sits on |
|---|---|---|---|
| `derive` | `#[derive(..)]` on a struct, enum or union: each element whose last path segment is `Component`, `Resource`, `Message`, `Event`, `EntityEvent`, `Reflect` (lists, qualified paths, several attributes) | `component`, `resource`, `message`, `event`, `entity_event`, `reflect` | the item |
| `derive` | the same in `#[cfg_attr(<p>, derive(..))]` | as above, `uncertain cfg_attr` | the item |
| `impl` | `impl Component for T`; `impl Plugin for T` (shipped `ImplPlugin`) | `component`, `plugin` | `T`'s path, linked |
| `fn` | a `fn` whose only parameter is `&mut App` (shipped `FnApp`) | `plugin` | the fn |
| `call` | `.add_systems(G, ..)` leaves; `.add_observer(x)`; `.add_plugins(..)` leaves; **`.add_message::<T>()`** | `system` (group `G`), `observer`, `plugin`, `message` | the leaf's path without generic arguments, linked; a closure: its enclosing fn |
| `call` | shipped `Uncertain` rows | the call's word, `uncertain <category>` | nothing (counted) |

- As written, never resolved (layer C, slice 11): a derive by its last segment; `Resource` is not also `component`; a derive in a macro body is not read (its item is no symbol). `add_message` reads as `add_observer`: code exactly, macro tokens name only (`origin` `macro_rules`, `macro_call`). [A] 0.19 has no `add_event`.
- Words, sources, categories, per-word item kinds: `specengine-code` data (`frameworks::roles`); store, CLI, MCP hold none (WA-11).

### Table

WA-1. Per file, `(file, ord)`, cascading, a pure function of (bytes, path, package table, `code_fp`) as 6a's rows; appended to `FORMAT_TABLES`; `dump.rs` maps `(path, ord)`.

| Table | Key | Columns |
|---|---|---|
| `code_roles` | `(file, ord)`; `subject_name` indexed | `word`, `source`, `form` (`path`, `closure`, `factory`, null), `origin` (`code`, `macro_rules`, `macro_call`), `group_text` (whitespace dropped, ≤ 128 B + `…`), `symbol_ord` (a derive's, `fn`'s item, a closure's enclosing fn), `subject` (path text, ≤ 128 B + `…`), `subject_name` (its last segment), `certainty` (`certain`, `uncertain`), `category`, `line` |

`CODE_INDEX_FORMAT` +1; `INDEX_FORMAT` +1 (10 after 6a alone) and its `format_history.txt` line over `spec-a`, `spec-b`, `code-a`, `code-bevy` (WA-14).

### Links

Per read, never stored (6a's purity; SW refinement). A `call` or `impl` row with a subject: 6a's lookup tiers (`exact`, `suffix`, `name`) over `subject`, item kinds by word (data: `system`, `observer` -> `function_item`; `plugin` -> `function_item`, `struct_item`, `enum_item`; others -> `struct_item`, `enum_item`, `union_item`, `type_item`), the row's own package first, else all packages. One hit: `linked`; several: `ambiguous` with the count, on each; none: `unlinked`, counted only (`DefaultPlugins`, a dependency's system). A row with `symbol_ord` is `certain` or `uncertain` on its item; a closure row `linked` on its enclosing fn, `form` `closure`.

### spec symbols

6a's forms unchanged. Per target, after `sig`, before its bindings, one line per role row on it (`spec symbols --file crates/game/src/stats.rs`, cut):

```
crates/game::stats::Health | struct_item | crates/game/src/stats.rs:4-6 | layer a | mentions 0
  sig pub struct Health
  role component | a | crates/game/src/stats.rs:3 | derive | - | certain
  role reflect | a | crates/game/src/stats.rs:3 | derive | - | certain
crates/game::stats::regen | function_item | crates/game/src/stats.rs:12-18 | layer a | mentions 0
  sig fn regen<T: Stat>(mut q: Query<&mut T>, time: Res<Time>)
  role system | a | crates/game/src/lib.rs:20 | call | Update | linked
```

- Line: `role <word> | a | <path>:<line> | <source>[ <form>] | <group or -> | <state>[ <category or count>]`; `<form>` only when `closure` or `factory`. Order: layer (`b` first, 10b), word, path bytes, line, source.
- JSON: each symbol gains `roles` after `bindings`: `[{role, layer, path, line, source, form, origin, group, state, detail, candidates}]`, every key present (absent `null`); `detail` the uncertain category, `candidates` the `ambiguous` count. `roles` is `null` without `frameworks`, `[]` with it and no row.
- MCP: `find_symbols`' mirror gains the keys, each a free string or integer, never an enum of words (`code-tools.md` "Tools", "Rules").

### spec index

```
code code-bevy: walked 9, parsed 9, unchanged 0, removed 0, unreadable 0, skipped 0, symbols 41, markers 0, mentions 0
roles code-bevy: component 3, entity_event 1, event 1, message 2, observer 1, plugin 3, reflect 2, resource 1, system 7; registrations 14 (linked 11, ambiguous 1, unlinked 2), uncertain 1
```

`<word> <n>`: symbols holding a row `certain`, `linked` or `ambiguous` of the word; every word of the listed frameworks, byte order, zeros too. `registrations`: `call` rows by state; `uncertain`: uncertain rows. JSON `code.roles` `{words: {<word>: n}, registrations: {total, linked, ambiguous, unlinked}, uncertain}`; off: `null`, no line.

## Rules and edge cases

- WHEN any sequence of `.rs`, manifest, `[code]` (`frameworks` too), `[ids]` or stamp changes is indexed, the index dump SHALL equal a fresh rebuild (6a's rule, `code_roles` in).
- WHEN `frameworks` is absent or empty, the detector SHALL NOT run; outputs SHALL equal 6a's but the two `null` keys.
- WHEN a subject names several items, the role SHALL show on each as `ambiguous`, never on a picked one (`code-identity.md`: reported, never guessed).
- A role never binds, maps a unit (`unmapped`), enters `spec.lock`, a verify row, a check finding or a node kind (ADR-0020, ADR-0031); nothing blocks (ADR-0012).
- Determinism: two roots, cwd, reverse creation, CRLF: byte-identical `symbols`, `index`.
- Genre (`architecture.md#universal`): no stack word, role word, framework method or `::` split in core, store, CLI or MCP sources.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red:

- [ ] RD-1 -- derives. M: first element only -> `derive_list_yields_every_role`; qualified path dropped -> `qualified_derive_path_is_read`; `cfg_attr` certain -> `cfg_attr_derive_is_uncertain`; a second `#[derive]` ignored -> `every_derive_attribute_is_read`; `Debug` given a role -> `other_derives_name_no_role`.
- [ ] RD-2 -- `add_message`. M: method not read -> `add_message_registers_its_type`; macro tokens resolved -> `add_message_in_macro_tokens_is_name_only`; generics kept in `subject` -> `add_message_subject_drops_generic_arguments`.
- [ ] RD-3 -- storage, key. M: a link stored or a cross-file pass -> TE-2 and `renaming_a_system_unlinks_its_registration`; `frameworks` outside `code_fp` -> `frameworks_change_rederives_roles`; detector run without the opt-in -> `no_roles_without_frameworks`; `CODE_INDEX_FORMAT` kept -> 6a's `analyzer_format_change_rederives_every_code_file`; table outside `FORMAT_TABLES` -> 6a's `format_change_recreates_code_tables_and_keeps_queue_rows`.
- [ ] RD-4 -- config. M: `bevy` kept in `NOT_BUILT_KEYS` -> `code_table_unknown_key_is_refused` (new case `bevy = true`, the unknown-key text) and the stack-word scan (`EXEMPT` 4); unknown framework accepted -> `unknown_framework_is_exit_2_naming_the_line`; bad shape accepted -> `frameworks_shape_is_exit_2_naming_the_line`; a value list in the core -> the stack-word scan; `spec check` failing on the key -> `code_table_unknown_key_is_refused`'s name-check half.
- [ ] RD-5 -- links. M: first candidate taken -> `subject_naming_two_items_is_ambiguous_on_each`; closure linked by name -> `closure_registration_sits_on_its_enclosing_fn`; own package not first -> `own_package_link_wins`; an `unlinked` row on a symbol -> `unlinked_rows_are_counted_only`.
- [ ] RD-6 -- output. M: `[]` without the opt-in -> `roles_are_null_without_frameworks`; a key dropped -> 6a's `symbols_json_every_key_present`; file order -> `role_rows_sort_by_content`; words an enum in the mirror -> 6b-1's `code_mirrors_keep_kinds_free`; rows counted, not symbols -> `role_counts_count_symbols`.
- [ ] RD-7 -- annotation only. M: a role maps a unit or alters a row -> `roles_change_no_refs_unmapped_verify_or_lock_output` (byte-identical with `frameworks` on and off).
- [ ] RD-8 -- genre. M: a `"component"` literal in a core, store, CLI or MCP module -> `new_modules_hold_no_fixture_prefix_kind_path_or_stack_word` (`NEW_MODULES` extended to this slice's modules); in MCP -> `mcp_genre.rs`.
- [ ] RD-9 -- detector intact. M: a shipped registration changed -> `bevy_detector.rs`; the eval printing the new facts -> eval `bevy_cli.rs` on `bevy-mini`, byte-identical.

test-engineer:

- [ ] TE-1 -- `fixtures/code-bevy` (WA-12; parsed only, no Bevy dependency, never built; `bevy-mini` untouched): `specengine.toml` with `[code]` and `frameworks`; a lib and a bin package; every derive form (list, qualified, two attributes, `cfg_attr`); `impl Component for`; `add_message::<T>()` in code and in a `macro_rules!` body; one generic system registered from one site for 3 types; a closure, a pipe, a factory, a macro-registered system, a nested `fn`, cfg twins; `impl Plugin for`, an `&mut App` fn, `DefaultPlugins`. `expected.json`: every role row, state, link, count.
- [ ] TE-2 -- seeded sequences of `.rs` edits, adds, removes, renames, `frameworks` on and off, stamp changes; after each step dump = rebuild; each RD-3 mutation red.
- [ ] TE-3 -- determinism: two roots, cwd, reverse creation, CRLF: `symbols`, `index` byte-identical.
- [ ] TE-4 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- [ ] TE-5 -- genre: `spec-c` unchanged; `code-a` byte-identical to before but the `null` keys; a scratch `code-a` with `frameworks = ["bevy"]`: `roles: []` everywhere.
- [ ] TE-6 -- regression: `bevy_detector.rs`, eval `bevy_cli.rs`, `code_refs_walk.rs`, `build_graph.rs` green; the `format_history.txt` line.
- [ ] TE-7 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground, scratch `HOME`, read-only, counts only, no name or path committed): rows per word, registrations by state, uncertain; `full_ms`, `one_file_ms` with and without the opt-in against 08 AC-10 (`one_file_ms` > 200: 6c).

spec-writer:

- [x] SW-1 -- this spec; 08 s2 lists 10a; a dated note in `symbol-index.md`; `export index`, `check` clean.
- [ ] SW-2 -- at shipping: `code-bindings.md` "The `[code]` table" (`frameworks`; `bevy` an unknown key); a Tier 2 `docs/canon/code-roles.md` (≤ 12 288 B; `code-identity.md` 9 918 B on 2026-10-15, 5a and 6a edit it first); `specengine-code` README (10 199 of 10 240 B: 5a-1 moves "Hash recipe" down first), store, CLI READMEs; 05 s5.1 :128 pointer; 07 :30 (`find_symbols` roles), :204 (`bevy = true` -> `frameworks`); `CLAUDE.md` State.
- [x] SW-3 -- no ADR of its own (ADR-0048 is 10b's).

## Risks

R1 (Q1): an owner expecting derives from the dump makes the split rework. R2: a framework key or word leaks into core, store, CLI or MCP (RD-4, RD-8, TE-5). R5: Bevy 0.20 changes events and observers (05:130): `event`, `entity_event`, observer reading go behind a version gate, a framework's data, never a new key. R9: one detector pass per `.rs` file per refresh against `one_file_ms` ≤ 200 (TE-7; else 6c). Text only: a project's own `Component` derive or `add_message` method reads as Bevy's (stated limit). The `specengine-code` README is at its cap.

## Out of scope

Dump facts, groups as dumped, instances (10b); a `--role` filter, a new form or tool; resolved types (layer C, slice 11); `#[require(..)]`, `#[reflect(..)]`, sets, conditions, ordering (slice 12); roles in `show`, bundle, impact, health, `unmapped`, verify, lock, check; a node's inferred layer (05:128); HTTP, UI; a Bevy 0.20 gate; a framework but `bevy`; writing markers.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 derives, `add_message` | layer A (this); 0.19's dump has no derive or message field, all messages share one update system | a SpecEngine dump schema and a producer per Bevy minor, for facts the source states exactly |
| Q3 keys | `[code] frameworks = ["bevy"]` here, `[[code.dump]]` in 10b; core checks the shape, `specengine-code` the values; 07 :204's `bevy = true` replaced at shipping | a framework word in the core's closed table, one key per framework |
| Q8 ADR | none here; ADR-0048 (10b) | -- |
| Q9 timing | 10a in its slot; 10b just before slice 12 | 10b now: an owner review for instances covering ≤ 6.6 % of own systems |
| SW links | per read through 6a's tiers, own package first | a stored cross-file pass beside `twin_files` |
| SW counts | symbols per word; registrations by state | rows double-count a derived and registered message |

WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Working answers

[P] checked, [A] assumed. WA-1 [P] 05:128 lists derives and `add_message` as not-built layer A items; the detector reads `add_systems`, `add_observer`, `add_plugins` only (`bevy/mod.rs`); 6a defers "the Bevy layer (10)"; per-file rows keep 6a's purity. WA-4 [P] `[code]` closed (`code_toml.rs` `KEYS`, `NOT_BUILT_KEYS`); 6a keys `[code]` in `code_fp`. WA-11 [P] `code_refs_walk.rs` `STACK_WORDS`, `EXEMPT`; 6a's genre rule. WA-12 a new fixture. WA-13 [A] as "Role facts". WA-14 a format bump per part.

## Dependencies and ordering

- **Needs**: 1 `code-refs` (shipped: `[code]`, the walk); 6a `symbol-index` (hard: tables, `CodeAnalyzer`, tiers, `code_fp`); 5a-1 `hash-levels` first (both edit `specengine-code` `lib.rs`). Soft: 6b-1 `code-tools` (`find_symbols` inherits roles; whichever ships second carries the mirror keys).
- **Order**: 5a-1, 6a -> 10a -> 10b (`bevy-dump.md`, its `not_dumped` reads 10a's rows), built right before 12 `machine-bindings`; 10a before 11 `layer-c`, which refines text-only roles. 6c only if TE-7 breaks AC-10.

## Implementation

Not started. To record: modules, TE-7's figures, deviations with reasons.
