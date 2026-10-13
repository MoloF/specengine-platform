---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-eval, crates/specengine-mcp, fixtures]
ref: Phase 3 plan 2026-10-10, slice 10b (WA-2..WA-10, WA-12, WA-14; Q2-Q9 decided under the owner's pre-approval, review open); 05 s5.1; 07 s5; 08 s2, s5
adrs: [ADR-0048]
---

# bevy-dump: the project's schedule dump annotates symbols (layer B)

## Why

Layer A cannot enumerate the instances of one generic system registered from one site, its only miss on the pilots (05:121: 93.4-100 % of dumped own-crate systems found). Bevy's `schedule_data` dump lists every system per group (schedule) with its full `type_name`; only `specengine-eval bevy-detector --dump` reads it.

**10b** (this) reads the project's own dump, named in `[[code.dump]]`, with the eval's reader moved into `specengine-code`, and annotates 6a's symbols with layer B role rows (group, instances, match category), B first on what is registered where, plus two staleness counts. Minimal (Q9): systems, instances, counts; **built right before slice 12 `machine-bindings`**, its first consumer, after 10a (`bevy-roles.md`). ADR-0048: a layer B fact is a file the project writes; it annotates, never binds.

## Description and interactions

- **Owner, agent**: `spec symbols regen` (`find_symbols`) shows `role system | b | dumps/app_data.ron:41 | dump | Update | exact | instances 3: …` above 10a's layer A rows; `spec index` prints each dump's state and counts.
- **Producer** (Q2, WA-2): the project, Bevy 0.19's `SerializeSchedulesPlugin` output unchanged (05 s5.1's recipe, on its side). SpecEngine never patches, builds, runs or schedules it; no helper crate, no dump command.
- **Crates**: `specengine-code` the reader (from eval `bevy/dump.rs`), `type_name` parse and label rule (from `compare.rs`), the format as `frameworks` data, the classifier; core `[[code.dump]]`'s shape; store `code_dumps`, `dump_entries`, `name` lookups; CLI matching, flags, output; eval calls the moved code; MCP mirror keys. No `ron`, serde-RON or Bevy crate (Q7).
- **Freshness**: each code read BLAKE3s each dump, parsing it only when (path, BLAKE3, format, `DUMP_READER_FORMAT`) changed.
- **Unchanged** (WA-8): `refs`, `unmapped`, `verify`, `lock`, `health`, `check`, bundle, impact; 6a's `symbols.layer` stays `a`. Without `[[code.dump]]` every output is 10a's but `dumps: null`. **Writes**: the data directory only.

## Data

### `[[code.dump]]`

WA-4, Q3.

```toml
[code]
roots      = ["crates"]
frameworks = ["bevy"]

[[code.dump]]                       # one per app; optional
path   = "dumps/app_data.ron"       # the project writes it, committed or not
format = "bevy-0.19-schedule"
```

- **Core** (shape): `dump` an array of tables, each exactly `path` (`is_clean_relative`, as `roots`) and `format` (`[a-z0-9][a-z0-9.-]{0,63}`); another key, a missing one, a wrong type, a repeated `path`: exit 2 at the line. `KEYS` gains `dump` (10a's three -> `["roots", "exclude", "frameworks", "dump"]`); `NOT_BUILT_KEYS` stays `["languages", "crates", "scip"]`.
- **Values** (`specengine-code` `frameworks::dump_formats`, data): an unknown format, or one whose framework `frameworks` does not list -> exit 2 at the `format` line, known names from data.
- **Not code**: a dump path is never walked (`.ron` or not), never an `unmapped` unit, not in `code_fp`: a `[[code.dump]]` change re-reads dumps only. Looked up as a root (`code-bindings.md` "The `[code]` table": exact name per component; a symlink on the way, not a regular file: `missing`).

### Reader

WA-3. `specengine-code` `bevy::dump`, moved: a pure function over `&str` (the store reads the bytes), the own RON lexer, an explicit stack to `MAX_DEPTH` 64, errors naming the line. Read: `schedules[].name`, `systems[].{name, apply_deferred}`; `system_sets`, `hierarchy`, `dependency`, `components`, `conflicts`, `exclusive`, `deferred` skipped token by token, no value built (slice 12's input); a field outside the 0.19 schema counted (`unknown_fields`), one absent too (`missing_fields`). `DUMP_CAP_BYTES` 33 554 432 [A; TE-7 records pilot sizes]: over it, not parsed. `DUMP_READER_FORMAT`, a stamp: bumped, every dump re-read.

`type_name` parse and label rule, moved from `compare.rs` (no `::` split outside `specengine-code`, WA-11): `{unit, segments, name, args, form}`.

| `type_name` | unit | segments | name | args | form |
|---|---|---|---|---|---|
| `game::stats::regen` | `game` | `[stats]` | `regen` | null | `path` |
| `game::stats::regen<game::stats::Hp>` | `game` | `[stats]` | `regen` | `game::stats::Hp` (≤ 128 B + `…`) | `path` |
| `game::plugin::setup::{{closure}}` | `game` | `[plugin]` | `setup` | null | `closure` |
| `Pipe(game::io::read, game::io::log)` | `game` | `[io]` | `read` (the first) | null | `pipe` |
| `<game::Hero as game::Act>::run` | `game` (the self type's) | null: name tier only | `run` | null | `path` |

Labels compare with path qualifiers dropped (`OnEnter(State::A)` -> `OnEnter(A)`, as `Debug` prints). Own units: the crate names of 6a's `code_packages.targets` (package, lib, bins; `-` -> `_`).

### Tables

WA-5. Keyed by worktree, appended to `FORMAT_TABLES`, rows a pure function of (bytes, format, `DUMP_READER_FORMAT`); `INDEX_FORMAT` +1 with its `format_history.txt` line; `CODE_INDEX_FORMAT` unchanged (no code file re-derived).

| Table | Key | Columns |
|---|---|---|
| `code_dumps` | `(wt, path)` | `format`, `blake3`, `size`, `reader`, `state` (`read`, `missing`, `unreadable`), `reason` (`read_error`, `not_utf8`, `too_large`, `parse`), `error_line`, `groups`, `unknown_fields`, `missing_fields` (JSON lists) |
| `dump_entries` | `(wt, path, ord)`; `name` indexed | `grp` (as dumped), `entry` (the `type_name`, ≤ 1 024 B + `…`), `unit`, `segments` (JSON, the parser's), `name`, `args`, `form`, `sync` (0/1), `line` |

`ord`: content order (group bytes, entry bytes, occurrence), never file order ([A] schedules come in map order). Store and CLI hold the format as an opaque string, group and entry as data (ADR-0008).

### Matching

WA-6, Q5. Per read, never stored (SW refinement: a stored match would rerun on every symbol change; per read costs one `name` lookup per own entry); the store serves lookups, `specengine-code` classifies. Per entry, the first that applies:

| Category | When | Shown |
|---|---|---|
| `sync_point` | `apply_deferred: true` | counted |
| `foreign` | `unit` is no own crate | counted |
| `exact` | one symbol of the unit's package and target whose qpath, discriminator dropped, renders `segments` + `name` | on it |
| `by_name` | no exact candidate; one `function_item` named `name` in the own packages | on it |
| `ambiguous` | ≥ 2 at the first tier holding any (cfg twins, a lib and a bin alike) | on each, with the count |
| `not_in_source` | none: macro-made, a nested `fn`, outside the roots, renamed since the dump | counted |

**Instances**: one dump's entries on one symbol with equal group, form and category form one row; `instances` their `args`, byte order.

### Precedence and flags

WA-7. Identity: the marker, then layer A's qpath; B moves no binding. Registration: with a readable dump B rows sort first, A rows stay. Flags, per read, only while some dump is `read`:

- `not_dumped`, on an A `system` row of a lib or bin target (test, example, bench are never an app's) when no B row of any read dump sits on its symbol;
- `other_group`, on an A `system` row whose group is a literal label that no B row on its symbol has (a variable or one-argument `add_systems`: never);
- `not_registered`, on a B row whose symbol holds no A `system` row.

Nothing is merged, dropped or fatal; the two staleness counts (Q4) are `not_in_source` (dumped own entries with no symbol) and `not_dumped`.

### Output

```
crates/game::stats::regen | function_item | crates/game/src/stats.rs:12-18 | layer a | mentions 0 | exact
  sig fn regen<T: Stat>(mut q: Query<&mut T>, time: Res<Time>)
  role system | b | dumps/app_data.ron:41 | dump | Update | exact | instances 3: game::stats::Hp, game::stats::Mp, game::stats::St
  role system | a | crates/game/src/lib.rs:20 | call | Update | linked
symbols: 1 of 1; code files 9 (.rs 9, .ron 0), skipped 0
dump dumps/app_data.ron: read; not_in_source 2, not_dumped 1
```

- **B line**: `role <word> | b | <dump path>:<line> | dump[ <form>] | <group> | <category>[ <count>][ | instances <k>: <args>, …][ | <flag>]…`, at most 8 args then `…`; A lines gain `| <flag>`. The word comes from `frameworks` data (`system`).
- **Dump lines**, after the summary, one per `[[code.dump]]` in config order: `dump <path>: <state>[ <reason>[ line <n>]][; not_in_source <u>, not_dumped <d>]`. Reads exit as before (Q6).
- **JSON** (`symbols`): each `roles` row gains `instances` (list, null for A) and `flags` (list); top level `dumps` after `code`: `[{path, format, state, reason, line, not_in_source, not_dumped}]`, `null` without `[[code.dump]]`. MCP mirrors: free strings.
- **`spec index`**: `dump code-bevy dumps/app_data.ron: read, groups 6, entries 40 (own 15, foreign 21, sync 4); exact 9, by_name 2, ambiguous 1, not_in_source 3; instance rows 1 (instances 3); not_dumped 1; unknown fields 1, missing 0`; JSON `code.dumps` `[{path, format, state, reason, line, groups, entries, own, foreign, sync, exact, by_name, ambiguous, not_in_source, instance_rows, instances, not_dumped, unknown_fields, missing_fields}]`, `null` without.

## Rules and edge cases

- WHEN a dump is missing, unreadable, over the cap or stale, every read SHALL keep its exit and show the state (Q6, ADR-0012).
- WHEN any sequence of dump, `[[code.dump]]`, `.rs` or stamp changes is indexed, the index dump SHALL equal a fresh rebuild; WHEN schedules are permuted, output SHALL be byte-identical.
- Reading a dump spawns nothing; nothing is built, run, patched or fetched. B facts never bind, lock, judge, map a unit, become a symbol or a check finding (ADR-0048, ADR-0020).
- Determinism (two roots, cwd, reverse creation, CRLF dump) and genre (`architecture.md#universal`: format, group, entry, args are data outside `specengine-code`) as 10a.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red:

- [ ] RD-1 -- reader move. M: `apply_deferred` dropped -> eval `fixture_with_dump_matches_expected_json`; the eval keeps a copy -> `eval_has_no_dump_reader_of_its_own`; a skipped field built as a value -> `skipped_fields_build_no_value` (a 10 MB `conflicts` within TE-8's bound).
- [ ] RD-2 -- build graph. M: `ron` added to `specengine-code` -> `code_crate_depends_on_exactly_the_parser_and_the_hash`, `no_workspace_dependency_is_added_against_main`; a Bevy crate -> `core_tree_has_no_rust_analyzer_syn3_or_bevy`.
- [ ] RD-3 -- config. M: unknown format accepted -> `unknown_dump_format_is_exit_2_naming_the_line`; format without its framework -> `dump_format_needs_its_framework`; repeated path -> `repeated_dump_path_is_exit_2`; missing file exit 2 -> `missing_dump_is_a_note_and_reads_answer`; dump walked as RON code -> `dump_file_is_not_a_code_unit`; dump in `code_fp` -> `dump_config_change_rederives_no_code_file`.
- [ ] RD-4 -- key. M: bytes outside the key -> `dump_edit_is_reindexed`; parse on every read -> `unchanged_dump_is_not_parsed_again`; reader stamp ignored -> `reader_format_change_rereads_every_dump`.
- [ ] RD-5 -- matching. M: a stored match -> `renaming_a_dumped_fn_turns_its_entry_into_not_in_source`; terminal name only -> `same_name_in_two_modules_matches_by_module_path`; first candidate -> `two_candidates_stay_ambiguous`; args in the key -> `instances_group_under_their_generic_item`; bin crate unmapped -> `bin_crate_systems_match_their_bin_unit`; closure by its own name -> `closure_entry_sits_on_its_fn`.
- [ ] RD-6 -- precedence, flags. M: A first -> `dump_rows_sort_first`; a flag without a read dump -> `no_flag_without_a_read_dump`; a test target flagged -> `test_target_registration_is_never_not_dumped`; a variable group flagged -> `non_literal_group_is_never_other_group`.
- [ ] RD-7 -- annotation only. M: a dump fact binds or alters a row -> `dump_changes_no_refs_unmapped_lock_or_verify_output` (byte-identical, dump present and absent).
- [ ] RD-8 -- no process. M: a command spawned on the read path -> `dump_read_runs_with_an_empty_path`; a source scan: no `std::process` in this slice's modules.
- [ ] RD-9 -- order. M: rows in file order -> `permuted_dump_gives_identical_output`.
- [ ] RD-10 -- drift, depth, cap. M: an unknown field fatal -> `unknown_dump_field_is_counted`; recursion past `MAX_DEPTH` -> `deep_dump_is_unreadable_not_a_crash`; cap ignored -> `dump_over_the_cap_is_unreadable`.
- [ ] RD-11 -- output. M: a key dropped -> 6a's `symbols_json_every_key_present`; `dumps` `[]` without config -> `dumps_are_null_without_config`; a dump state changing the exit -> `dump_state_never_changes_the_exit`; genre -> 10a's scan over this slice's modules.

test-engineer:

- [ ] TE-1 -- `fixtures/code-bevy` (10a's) gains `dumps/app_data.ron`, hand-written 0.19 (foreign entries, sync points, one unknown field, 3 instances of one generic system, a closure, a pipe, a macro-made system, a nested `fn`, cfg twins, a bin system, one renamed `fn`) and `[[code.dump]]`; `expected.json` covers every category, instance row, flag, count, state.
- [ ] TE-2 -- seeded sequences of dump edits, adds, removes, `[[code.dump]]` changes, `.rs` edits, `frameworks` changes, stamps; after each step dump = rebuild; each RD-4, RD-5 mutation red.
- [ ] TE-3 -- determinism: two roots, cwd, reverse creation, a CRLF dump, permuted schedules: `symbols`, `index` byte-identical.
- [ ] TE-4 -- read-only: porcelain empty; the dump's size, mtime, BLAKE3 kept; reads pass with `PATH` empty.
- [ ] TE-5 -- genre: `spec-c` unchanged; `code-a` byte-identical to 10a; the scans green.
- [ ] TE-6 -- regression: eval `bevy_cli.rs` fixture output byte-identical; `build_graph.rs` green; the `format_history.txt` line.
- [ ] TE-7 -- pilots, manual (`#[ignore]`, release, `--timeout`, scratch `HOME`, `SPECENGINE_PILOT_{A,B}_DUMP`, read-only, counts only, no name or path committed): dump sizes; shares of `exact`, `by_name`, `ambiguous`, `not_in_source` against Phase 0's 93.4-100 %; `not_dumped`; read and match timings against AC-10.
- [ ] TE-8 -- cost: a 10x synthetic dump costs at most 12x the time; a read with an unchanged dump adds one BLAKE3 and the lookups only.

spec-writer:

- [x] SW-1 -- this spec and `bevy-roles.md`; 08 s2 lists 10a, 10b (built right before 12); ADR-0048 with `architecture.md#code-identity`, `architecture-code.md#code-identity` in one change; dated notes in `symbol-index.md`; `export index`, `check` clean.
- [ ] SW-2 -- at shipping: a Tier 2 `docs/canon/code-layer-b.md` (format, reader, match, categories, flags, counts; ≤ 12 288 B), `architecture-code.md#code-identity` pointing at it; `specengine-code`, store, CLI, eval (:32 reader moved) READMEs; 05 :119, :128 pointers; 07 :204-205 keys; 08 :127 risk row (a format per minor, re-dump on the project's side); `CLAUDE.md` State.
- [x] SW-3 -- ADR-0048 ≤ 1 536 B, `canon:` `architecture.md#code-identity`, its sentence there; worded for layer B only, leaving slice 11's rust-analyzer decision open.

## Risks

R3: a rustc changing `type_name` turns entries `not_in_source` (counted). R4: a stale dump trusted: rows tagged `b`, counts beside, never evidence. R5: the schema changes across minors (08 :127): a format name and reader per minor. R6: `specengine-code` at three dependencies, no Bevy in the core graph, eval output byte-identical. R7: a dump is a project-side instrumented build (minutes, GBs), never in tests; parse on change only, the cap. R8: pilots read-only, dumps outside the repository. R10: 6a's `layer` never overloaded. Value: ≤ 6.6 % of own systems gain instances until slice 12.

## Out of scope

Producing a dump (build, run, patch, command, helper crate); `system_sets`, `hierarchy`, `dependency`, conditions, `conflicts`, `#[require(..)]` (slice 12); observers, plugins from the dump; resolved types (slice 11); another schema, `bevy_mod_debugdump`; a 0.20 format; linking `args` to symbols; an inferred node layer; `refs`, `unmapped`, bundle, impact, health, lock, verify, check, HTTP, UI; time-based staleness; writing markers.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q2 producer | the project, 0.19's own RON as is | a dump command: minutes, GBs, project code run, an ADR beside ADR-0044 |
| Q3 keys | `[[code.dump]] path, format`, several apps | a framework key in the core |
| Q4 staleness | BLAKE3 keys it; two counts; the minor in the format name | git age per refresh, none for untracked dumps |
| Q5 merge | annotation; exact, unique name, else `ambiguous` or a miss; B first | B rows as symbols: 6a's purity broken, `unmapped` inflated |
| Q6 broken dump | a state line, exit unchanged; bad config exit 2 | one stale dump stops every code read |
| Q7 dependency | none (own RON lexer, `stack.md` :27) | an owner pin, a second RON reader |
| Q8, Q9 | ADR-0048; minimal, right before slice 12 | precedence re-decided by 11, 12; a review now for ≤ 6.6 % |
| SW | match per read; 32 MiB cap; flags with a read dump; a format needs its framework | a pass per symbol change; unbounded memory |

WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Working answers

[P] checked, [A] assumed. WA-2 [P] ADR-0013, ADR-0044; eval README "never patches, builds or runs". WA-3 [P] `dump.rs` (schema, lexer, `MAX_DEPTH`, line errors), `compare.rs` (`type_name` keys). WA-5 [P/A] bytes, not mtime; content order. WA-6 [P] the marker is the identity; 6a's targets, tiers. WA-7 [P] 05:119 "the truth for registrations". WA-10 [P] 05:121; slice 12 needs `dependency`. A1 pilots on 0.19; A2 `type_name` best effort per rustc.

## Dependencies and ordering

- **Needs**: 10a `bevy-roles` (hard: `frameworks`, `code_roles`, `fixtures/code-bevy`); 6a `symbol-index` (hard: symbols, `code_packages.targets`, `name` lookups, `CodeAnalyzer`); 1 `code-refs` (shipped). Soft: 6b (MCP for free).
- **Order**: 5a-1 -> 10a -> 10b, **built right before 12 `machine-bindings`**, which reads the skipped `dependency` edges. Slice 11 `layer-c` is independent of 10b: ADR-0048 leaves layer C to its own decision. `INDEX_FORMAT` +1 per part, once if 10a and 10b ship together.

## Implementation

Not started. To record: modules, TE-7's and TE-8's figures, the cap's basis, deviations with reasons.
