---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: Phase 3 plan 2026-10-10, slice 10a-2, split from 10a `bevy-roles.md` 2026-10-16 (D1, P1, SR7; its S8, S11, S12, S20; decided 2026-10-16 under the owner's pre-approval, review open); 05 s5.1; 07 s1.2; 08 s2
adrs: []
---

# bevy-roles-reads (10a-2): roles in `spec symbols` and `find_symbols`, plugin 0.10.0

## Why

10a-1 (`bevy-roles.md`) stores which items hold a framework role and counts them in `spec index`, but an agent asking about one item still opens files: `spec symbols` and its MCP twin `find_symbols` show an item's signature and bindings, not its roles.

Delivered: under each target of `spec symbols`, every form, one line per role on it, and `roles` in its JSON; `find_symbols`' mirror carries the keys as free strings and integers; plugin 0.10.0, as a key a tool returns is a MINOR bump (`plugin-files.md`, P1). No format bump, no new command, form or tool. No ADR: ADR-0004/0005 (one door), ADR-0008 (genre), ADR-0020, ADR-0031 applied.

## Description and interactions

- **Owner, agent**: `spec symbols regen` (`find_symbols {query}`) shows `role system | a | crates/game/src/lib.rs:20 | call | Update | linked` under the item.
- **Crates**: store `code_snapshot` (10a-1's role read inside its transaction: the rows on the snapshot's symbols, the rows whose `subject_name` is one of theirs, their candidates); CLI `symbols.rs` (lines, `roles`, 10a-1's link module); MCP `mirror.rs` (`SymbolsTarget.roles`); `plugin/` (version, `PINS`). `INSTRUCTIONS`, descriptions and skills unchanged: `find_symbols(...) = symbols` covers it, the 1 755 B description is left alone. MCP `[dependencies]` stay 7; core, code, HTTP, UI unchanged.
- **Freshness**: 10a-1's and 6b-1's: each read refreshes, then one snapshot; links per read.
- **Unchanged**: what 10a-1 lists, `refs`, `unmapped`; without `frameworks` `symbols` is 6b-1's byte for byte but `roles: null`. **Writes**: the data directory only.

## Data

### spec symbols

`docs/canon/symbol-reads.md` "spec symbols"' forms and lines unchanged. Per item target, after `sig`, before its bindings, one line per role on it (`spec symbols --file crates/game/src/stats.rs`, cut):

```
crates/game::stats::Health | struct_item | crates/game/src/stats.rs:4-6 | layer a | mentions 0
  sig pub struct Health
  role component | a | crates/game/src/stats.rs:3 | derive | - | certain
  role reflect | a | crates/game/src/stats.rs:3 | derive | - | certain
crates/game::stats::regen | function_item | crates/game/src/stats.rs:12-18 | layer a | mentions 0
  sig fn regen<T: Stat>(mut q: Query<&mut T>, time: Res<Time>)
  role system | a | crates/game/src/lib.rs:20 | call | Update | linked
```

- **On a target**: the rows sitting on it (`symbol_ord`) and the rows whose subject it is `linked` or `ambiguous` for (`bevy-roles.md` "Links"); `unlinked` rows and rows on nothing show nowhere.
- Line: `role <word> | <layer> | <path>:<line> | <source>[ <form>][ <origin>] | <group or -> | <state>[ <detail>]`; `<form>` only `closure` or `factory`; `<origin>` only when not `code`; `<detail>` the category (`uncertain`) or the candidates' count (`ambiguous`); `<path>:<line>` the row's own place. Fields through `one_line`. Order: layer (`b` first, 10b), word, path bytes, line, source, `ord`.
- JSON: each target gains `roles` after `bindings`: `[{role, layer, path, line, source, form, origin, group, state, detail, candidates}]`, every key present (absent `null`); `detail` the category, `candidates` the `ambiguous` count; an item without a row, a module or RON target: `[]`; every target `null` without `frameworks`.
- Cap `OUTPUT_CAP_CHARS`: role lines are their target's; whole targets as now.

### find_symbols

`docs/canon/mcp-code.md` "Mirrors" (S11): `SymbolsTarget` gains `roles`, a nullable array, `required`, each item `{role, layer, path, line, source, form, origin, group, state, detail, candidates}`, each value a free string or integer, never an enum of words (as `item_kind`, `layer`). Doc comments hold no stack word: they become `outputSchema` text (`mcp_genre`). Parity runs over a scratch `fixtures/code-bevy` copy: the walker sees only the values a fixture produces.

### Plugin

P1, S12. `plugin.json` 0.9.0 -> 0.10.0 and a `PINS` line (`plugin_files.rs`); no other file under `plugin/specengine/` changes. Today nothing checks that a returned key bumps the version (`PINS` hashes `plugin/specengine/` only), so SR7 adds the check: `tool_schemas_follow_the_plugin_version` hashes `tools/list`'s `inputSchema` and `outputSchema` (BLAKE3 of their canonical JSON) against an append-only `(version, hash)` list, versions strictly increasing: its last hash is the current schemas', its last version at most `plugin.json`'s; entries 0.9.0 and 0.10.0. A schema change thus needs a new entry, so a higher version; a PATCH that changes no schema adds none. 6b-2 `verify-node.md` takes 0.11.0.

## Rules and edge cases

- WHEN `frameworks` is absent or empty, `spec symbols` SHALL print 6b-1's text and its JSON SHALL differ only by `roles: null`; `find_symbols` the same.
- WHEN `find_symbols` and `spec symbols --json` run on one state, `content` and `structuredContent` SHALL equal the CLI's (6b-1's RD-3), roles included.
- A role line changes no binding, `mentions`, tier, `total`, `omitted` or exit.
- Determinism: two roots, cwd, reverse creation, CRLF: byte-identical `symbols` and `find_symbols`.
- Genre (`architecture.md#universal`): no stack, role or source word in CLI, store or MCP sources or texts (10a-1's RD-8 scan; `mcp_genre`).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-6 -- output. M: `[]` without the opt-in -> `roles_are_null_without_frameworks`; a key dropped -> 6a-3's `symbols_json_every_key_present` (`roles` added); file order -> `role_rows_sort_by_content`; a module or RON target `null` with the opt-in -> `non_item_targets_hold_empty_roles`; role lines outside the cap -> `role_lines_count_against_the_cap`; a field not through `one_line` -> `role_fields_are_one_line`; an `unlinked` row shown on a same-named item -> `unlinked_rows_show_on_no_target`.
- [ ] RD-7 (the `symbols` half; the index half: `bevy-roles.md`) -- annotation only. M: a role alters a target's bindings, `mentions`, tier or `total` -> `symbols_equal_with_and_without_frameworks_but_roles` (code-bevy, every form).
- [ ] RD-10 -- MCP, plugin (P1, SR7). M: `roles` unmirrored -> the schema walker in `code_tools_equal_the_cli` (scratch `code-bevy`); a word an enum -> `code_mirrors_keep_kinds_free` (the `roles` keys); a stack word in a doc comment -> `mcp_genre`; the version kept -> `tool_schemas_follow_the_plugin_version` (new); `PINS` stale -> `plugin_files.rs`; `INSTRUCTIONS` changed -> 6b-1's pins (`mcp_create.rs` 1 968).

test-engineer:

- [ ] TE-3 (the `symbols` half) -- determinism: two roots, cwd, reverse creation, CRLF: `symbols` (every form) and `find_symbols` byte-identical.
- [ ] TE-5 -- genre: `spec-c` unchanged; `code-index`, `code-a`: `symbols` 6b-1's but `roles: null`; a scratch `code-a` with `frameworks = ["bevy"]`: `roles: []` everywhere; code-bevy per form against `expected.json`.
- [ ] TE-8 -- pilots, the owner's manual run (`mcp_code_pilot.rs`, `#[ignore]`, release, scratch `HOME`, read-only, counts only): warm `find_symbols` (a QUERY; `--file` of the file holding most role rows) with and without the opt-in against `mcp-read.md` "Latency"'s 1 s (S20; a breach plans 6c); `mcp_size` maxima on code-bevy recorded.

spec-writer:

- [x] SW-1 -- this spec, split from `bevy-roles.md` 2026-10-16 (D1).
- [ ] SW-2 -- at shipping, bytes measured 2026-10-16 at f9a80ff: `code-roles.md` (10a-1's: "spec symbols"); `symbol-reads.md` (10 287 of 12 288: "spec symbols"' line and key, a pointer; about 200); `mcp-code.md` (5 648: "Mirrors"' `roles`, the free list; "Tests"' 0.10.0, the schema pin; about 200); `plugin-files.md` (4 654: current 0.10.0, its hash, "a key a tool returns: … 0.10.0 `find_symbols`' `roles`"; about 70); the CLI README (after 10a-1's); the MCP README (10 210, 30 left: none planned; a test row needs room first); 07 s1.2 :30; 08 s2; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- Hand-written mirror (`mcp-code.md` "Mirrors"): free strings keep a new word from breaking `outputSchema`; a new key still does, until mirrored.
- Latency: links computed per read on every `find_symbols` call (TE-8).
- Versions: 8a-1's optional PATCH (`verify-tests.md`: 0.10.1 after this) and 6b-2's MINOR (0.11.0) follow; both append to `PINS`, 6b-2 to SR7's list too.

## Out of scope

A `--role` filter; roles in `refs`, `unmapped`, `show`, bundle, impact, health; HTTP, UI; a new tool; `INSTRUCTIONS` or description text; layer B rows (10b, parked).

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| D1 split (owner) | 10a-1 the facts, formats, `spec index`; this the reads | one L slice under the three-iteration cap |
| P1 plugin (owner) | 0.10.0 here; 6b-2 0.11.0 | `plugin-files.md`'s rule broken, unnoticed |
| SR7 schema pin | an append-only `(version, hash)` list over `tools/list`'s schemas | the bump unchecked, as every returned key's so far |
| On a target | rows on it and rows whose subject it is (`linked`, `ambiguous`) | rows on it only: a system's registration shows nowhere |

Decided 2026-10-16 under the owner's pre-approval, review open. S-n, SR-n: `bevy-roles.md` "Open".

## Dependencies and ordering

- **Needs**: 10a-1 (hard: `code_roles`, the role read, the link module, `fixtures/code-bevy`); shipped 6a-3 (`symbols`, `SymbolsJson`, `assert_symbols_keys`) and 6b-1 (`mirror.rs`, `mcp_code.rs`, `mcp_genre`, plugin 0.9.0, `PINS`).
- **Order** (08 s2, 2026-10-16): right after 10a-1, before 8a-1; 6b-2 after 5c, at 0.11.0.

## Implementation

Not started. From the split (2026-10-16, D1): RD-6 <- 10a's RD-6 (its count test stays in 10a-1's RD-7); RD-7, TE-3: the `symbols` halves; TE-5 <- 10a's TE-5; RD-10 (P1, SR7) and TE-8 (S20) new. To record: modules, deviations with reasons.
