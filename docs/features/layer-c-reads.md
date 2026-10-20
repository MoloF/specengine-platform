---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-mcp, plugin]
ref: Phase 3 plan 2026-10-10, slice 11a-3, split from 11a `layer-c.md` 2026-10-19 against 5b79b81 (drift 14, 18, 20, 31; Q4 display 2026-10-10 under the owner's pre-approval; L2 owner pre-approved 2026-10-19; review open); 07 s1.2; 08 s2, AC-10
adrs: [ADR-0049]
---

# layer-c-reads (11a-3): monikers in `spec symbols` and `find_symbols`, the next plugin MINOR

## Why

11a-2 (`layer-c-resolve.md`) stores a snapshot no read shows. Here an agent asking about an item sees its layer C state and moniker beside layer A's qpath, and can look an item up by the moniker; `find_symbols` mirrors it. Built only after the owner's TE-6 says the pilots' rows are worth showing. ADR-0049.

## Description and interactions

- **`spec symbols`**, every form: a `c` line per item target and a `layer c:` line after the summary while a snapshot exists; JSON keys always; tier `moniker` for QUERY. Without a snapshot the text is today's byte for byte.
- **`find_symbols`**: the mirror and its description follow (`mcp-code.md` "Mirrors": key for key); plugin the next MINOR (L2). `INSTRUCTIONS`, `TOOLS`, every other tool unchanged.
- **Nothing else**: `refs`, `unmapped`, `health`, `bundle`, `verify`, `lock` and the daemon unchanged.
- **Crates**: CLI `symbols`; store the moniker lookup; `specengine-code` 11a-2's join, read per row; MCP `mirror.rs` (`SymbolsTarget` +3 keys, `SymbolsDocument.layer_c`, `CodeTier::Moniker`), `code.rs` (the description, `QUERY_DOC`); `plugin.json`; tests `plugin_files.rs` (`PINS`, `SCHEMA_PINS`, its comment at :220 "the next MINOR").
- **Overridden live-spec lines** (rewritten at shipping): 07:30 (what `find_symbols` returns), 07:104; 08 AC-10 with TE-6's figures.

## Data

### spec symbols

```
arena::impl Health #2 | impl_item | arena/src/lib.rs:23-27 | layer a | mentions 0 | exact
  sig impl Health
  c moniker | arena impl#[Health] #2 | agrees | stale
symbols: 1 of 1; code files 6 (.rs 6, .ron 0), skipped 0
layer c: stale (ra_ap 0.0.352); spec resolve refreshes it
```

- **`c` line**, right after `sig`, before the role lines and bindings (`symbol-reads.md` "spec symbols"), on `item` targets only: `  c <c_state>[ | <moniker>][ | agrees|disagrees][ | stale]`. `c_state` and the rendered moniker (collision suffix included) from 11a-2's "Join"; `agrees` or `disagrees` unless null; `stale` while the snapshot is. None while `absent`, none on `module` and `ron` targets.
- **`layer c:` line**, after the summary, while a snapshot exists: `layer c: fresh (ra_ap <v>)`, or `layer c: stale (ra_ap <v>); spec resolve refreshes it`.

### JSON

- Each target, after `roles`: `c_state`, `moniker`, `agrees` (`SYMBOL_KEYS` 15 -> 18). Null without a snapshot (never `absent`), and on `module`, `ron` targets; `moniker` the rendered one.
- Top level, after `code`: `layer_c` (`SYMBOLS_KEYS` 11 -> 12): null, or `{state, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items}`, `state` `fresh` | `stale`, the rest `code_resolve`'s columns.

### Tier moniker

After `exact`, before `suffix` (`symbol-reads.md` "Lookup"): `code_monikers.moniker` = the query with a trailing ` #<value>` dropped (its index), then kept where the joined row's rendered moniker equals the query. A stale snapshot answers too. `tier`: `exact` | `moniker` | `suffix` | `name`; `CodeTier` a closed enum gains `Moniker`.

### find_symbols

The mirror equals the CLI key for key: `c_state`, `moniker` free strings or null, `agrees` a boolean or null, `layer_c` an object or null. The description names the `c` facts; `QUERY_DOC`: "A qualified path, a suffix of one, a bare name or a layer C moniker, as one value."

### Plugin

P1 (`plugin-files.md` "Changing it"): a key a tool returns is a MINOR. This part takes the next MINOR at shipping (0.13.0: 2b `roadmap-surfaces.md` took 0.11.0, 6b-2b `check-binding.md` 0.12.0), a `PINS` line and a `SCHEMA_PINS` entry (L2).

## Rules and edge cases

- WHEN no snapshot exists, `spec symbols` text SHALL be byte-identical to today's and every new key null.
- WHEN the snapshot is stale, rows SHALL show their old values marked `stale`; the tier still answers.
- A moniker never binds, locks, judges or becomes a finding; no other read prints one (ADR-0049).

## Acceptance criteria

M: mutation -> the test turning red; CLI tests seed a snapshot through 11a-2's fakes, never building `specengine-ra`.

- [ ] X0 -- as 11a-2's (`layer-c-resolve.md`): frozen `levels.rs`, `hash.rs`, `ron/`; `Cargo.lock` equals 11a-1's.
- [ ] S1 -- M: a `c` line while absent -> `symbols_text_is_unchanged_without_a_snapshot`; after the roles -> `c_line_precedes_the_roles` (`code-bevy`, `frameworks` set).
- [ ] S2 -- `SYMBOL_KEYS` 18, `SYMBOLS_KEYS` 12, nulls without a snapshot. M: a key dropped -> `symbols_json_every_key_present`.
- [ ] S3 -- `stale_snapshot_marks_every_c_line` (any `tree_fp` input changed); `changed_file_rows_are_unseen`; an `unmatched` row. M: no suffix -> `adjacent_inherent_impls_get_distinct_monikers`; a unique moniker suffixed -> `unique_moniker_is_unchanged`.
- [ ] S4 -- M: names compared -> `agrees_compares_crate_and_modules_only`; false without one target -> `agrees_is_null_without_a_single_target`.
- [ ] S5 -- M: no tier -> `moniker_query_finds_its_symbol` (after `exact`; a stale snapshot answers).
- [ ] S6 -- M: a key not mirrored -> `mcp_code.rs` `code_tools_equal_the_cli`; `mcp_genre` green; the schema changed at 0.11.0 -> `plugin_files.rs` `tool_schemas_follow_the_plugin_version` (`plugin.json`, `PINS`, `SCHEMA_PINS` bumped).
- [ ] S7 -- M: a moniker leaking -> 11a-2's `outputs_equal_with_and_without_a_snapshot` for `refs`, `unmapped`, `health`, `bundle`, `verify`, `lock`; `symbols` bytes identical across roots (TE-4, `symbol_reads_determinism.rs`).

spec-writer:

- [x] SW-1 -- this spec, split from `layer-c.md` 2026-10-19; `verify-node.md` and 08:41 say "the next MINOR" (L2).
- [ ] SW-2 -- at shipping: `symbol-reads.md` (11 692 of 12 288: a pointer in "spec symbols", detail in `docs/canon/layer-c.md`); `mcp-code.md` "Mirrors", "Texts", "Freshness, writes, size", "Tests" (5 967); `plugin-files.md` "Changing it" (5 072); 07:30, :104; 08 AC-10 with TE-6's figures; `CLAUDE.md` State.

## Out of scope

`spec resolve`, the tables, freshness (11a-2); a new tool, `INSTRUCTIONS`, plugin text (only the version moves); HTTP, UI; monikers in `refs`, `bundle`, `verify`, `lock`, `health`.

## Open

| Q | Working answer | The other answer costs |
|---|---|---|
| Q4 per row | `c_state`, `moniker`, `agrees` shown, never binding | lock re-keyed per `ra_ap` bump |
| SW | `unseen` for a changed file; tier after `exact` | moved offsets mislabelled |
| L2 plugin | the next MINOR here (0.12.0 if first); 6b-2 the one after | 0.12.0 held for 6b-2 parks this behind 5b-1..6b-2 |

Q4: 2026-10-10 under the owner's pre-approval; L2: owner pre-approved 2026-10-19; review open. The rest: `layer-c.md` "Open".

## Dependencies and ordering

- **Needs**: 11a-2 `layer-c-resolve` (the snapshot, `layer_c_state()`, the join); the owner's TE-6; shipped 6a-3 (`symbols`), 6b-1 (the mirror), 10a-2 (roles, `SCHEMA_PINS`).
- **Order**: 11a-1 -> 11a-2 -> TE-6 -> 11a-3 (this), unless the owner parks it on TE-6's figures; 5b-1 takes over at any boundary.
- **Carried from 11a-2's review** (2026-10-19, no change there): `layer_c_state` reads the `Tree` twice, so the reads take the state from the tree they refreshed (a `state_of`) or call `state_in` inside their own refresh; `tree_files` allocates per directory level: walk deepest-first and stop at the first known directory.

## Implementation

Not started; when filled, ≤ 3 072 B. To record: modules, the plugin version taken, deviations with reasons.
