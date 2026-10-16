---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-code, crates/specengine-core, crates/specengine-eval, plugin]
ref: Phase 3 plan 2026-10-10, slice 10a-2, split from 10a `bevy-roles.md` 2026-10-16 (D1, P1, SR7, K2; its S8, S11, S12, S20); 05 s5.1; 07 s1.2; 08 s2; Rust 2 iterations, review accepted, tests 1 round; TE-8 the owner's
shipped: 2026-10-16
adrs: []
---

# bevy-roles-reads (10a-2): roles in `spec symbols` and `find_symbols`, plugin 0.10.0

## Why

10a-1 (`bevy-roles.md`) stores which items hold a framework role and counts them in `spec index`, but an agent asking about one item still opened files: `spec symbols` and its MCP twin `find_symbols` showed an item's signature and bindings, not its roles.

Delivered: under each target of `spec symbols`, every form, one line per role on it, and `roles` in its JSON; `find_symbols`' mirror carries the keys as free strings and integers; plugin 0.10.0, as a key a tool returns is a MINOR bump (P1), now checked by a schema pin (SR7); `CODE_INDEX_FORMAT` 4 (B5; K2, owner pre-approved). No new command, form or tool. No ADR: ADR-0004/0005 (one door), ADR-0008 (genre), ADR-0020, ADR-0031 applied.

## Description and interactions

Canon: `docs/canon/symbol-reads.md` "Crates and seams", "spec symbols"; `code-roles.md` "Role facts", "Links", "Known limits"; `mcp-code.md` "Mirrors", "Tests"; `plugin-files.md` "Changing it"; `symbol-index.md` "Analyzer stamp"; the store, CLI and code READMEs; 07 s1.2.

- **Crates**: store `code_snapshot_with_roles`; CLI `symbols.rs` (lines, `roles`); MCP `mirror.rs`; code B5, `frameworks::PATH_FORM`/`CODE_ORIGIN` (D1), format 4; core and eval comments only; `plugin.json`. `INSTRUCTIONS`, descriptions, skills, MCP `[dependencies]` (7), HTTP, UI unchanged.
- **Freshness**: each read refreshes, then one snapshot; links per read. **Writes**: the data directory only.

## Data

### spec symbols

`docs/canon/symbol-reads.md` "spec symbols" (the role line, JSON `roles`, the cap); which rows sit on a target: `docs/canon/code-roles.md` "Links".

### find_symbols

`docs/canon/mcp-code.md` "Mirrors" (S11).

### Plugin

`docs/canon/plugin-files.md` "Changing it" (P1, S12, SR7); the pins: `mcp-code.md` "Tests".

## Rules and edge cases

- WHEN `frameworks` is absent or empty, `spec symbols` SHALL print 6b-1's text and its JSON SHALL differ only by `roles: null`; `find_symbols` the same.
- WHEN `find_symbols` and `spec symbols --json` run on one state, `content` and `structuredContent` SHALL equal the CLI's (6b-1's RD-3), roles included.
- A role line changes no binding, `mentions`, tier, `total` or exit; `omitted` only through the cap.
- Determinism: two roots, cwd, reverse creation, CRLF: byte-identical `symbols` and `find_symbols`.
- Genre (`architecture.md#universal`): no stack, role or source word in CLI, store or MCP sources or texts (10a-1's RD-8 scan; `mcp_genre`).

## Carry-overs

From 10a-1's reviews (owner pre-approved 2026-10-16), done:

- The `src` comments citing `bevy-roles` cite `docs/canon/code-roles.md` by its headings (data form, `doc_pointers.rs`); the CLI writes canon `code-roles`, "Links" instead, as CLI `genre.rs` AC-17 bans `docs/` there (D2), which `doc_pointers` now checks too (the CLI's 73). No `src` file names `bevy-roles`. The test citations name `code-roles.md` (11 re-pointed) or criterion IDs only, so `bevy-roles.md`'s pointer headings under "Data" are gone.
- B5: a `<T as Tr>` qualifier written as `impl_label` names the impl (`code-roles.md` "Role facts"), so K2.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [x] RD-6 -- output. M: `[]` without the opt-in -> `roles_are_null_without_frameworks`; a key dropped -> 6a-3's `symbols_json_every_key_present` (`roles` added); file order -> `role_rows_sort_by_content`; a module or RON target `null` with the opt-in -> `non_item_targets_hold_empty_roles`; role lines outside the cap -> `role_lines_count_against_the_cap`; a field not through `one_line` -> `role_fields_are_one_line`; an `unlinked` row shown on a same-named item -> `unlinked_rows_show_on_no_target`.
- [x] RD-7 (the `symbols` half; the index half: `bevy-roles.md`) -- annotation only. M: a role alters a target's bindings, `mentions`, tier or `total` -> `symbols_equal_with_and_without_frameworks_but_roles` (code-bevy, every form).
- [x] RD-10 -- MCP, plugin (P1, SR7). M: `roles` unmirrored -> the schema walker in `code_tools_equal_the_cli` (scratch `code-bevy`); a word an enum -> `code_mirrors_keep_kinds_free` (the `roles` keys); a stack word in a doc comment -> `mcp_genre`; the version kept -> `tool_schemas_follow_the_plugin_version` (new); `PINS` stale -> `plugin_files.rs`; `INSTRUCTIONS` changed -> 6b-1's pins (`mcp_create.rs` 1 968).

test-engineer:

- [x] TE-3 (the `symbols` half) -- determinism: two roots, cwd, reverse creation, CRLF: `symbols` (every form) and `find_symbols` byte-identical.
- [x] TE-5 -- genre: `spec-c` unchanged; `code-index`, `code-a`: `symbols` 6b-1's but `roles: null`; a scratch `code-a` with `frameworks = ["bevy"]`: `roles: []` everywhere; code-bevy per form against `expected.json`.
- [ ] TE-8 -- pilots, the owner's manual run (`mcp_code_pilot.rs`, `#[ignore]`, release, scratch `HOME`, read-only, counts only): warm `find_symbols` (a QUERY; `--file` of the file holding most role rows) with and without the opt-in against `mcp-read.md` "Latency"'s 1 s (S20; a breach plans 6c); `mcp_size` maxima on code-bevy recorded.

spec-writer:

- [x] SW-1 -- this spec, split from `bevy-roles.md` 2026-10-16 (D1).
- [x] SW-2 -- at shipping: `code-roles.md`, `symbol-reads.md`, `symbol-index.md`, `symbol-names.md`, `mcp-code.md`, `plugin-files.md`; the store, CLI and code READMEs; 07 s1.2, 08 s2; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

A `--role` filter; roles in `refs`, `unmapped`, `show`, bundle, impact, health; HTTP, UI; a new tool; `INSTRUCTIONS` or description text; layer B rows (10b, parked).

## Open

Decided under the owner's pre-approval: D1 the split; P1 0.10.0 here, 6b-2 0.11.0; SR7 an append-only `(version, hash)` list over `tools/list`'s schemas; on a target the rows on it and those whose subject it is (`linked`, `ambiguous`); K2 format 4.

## Implementation

| Module | What it does |
|---|---|
| store `roles.rs` | `FILE_ROLES_SQL` (a file's rows on symbols, by `(wt, path)`), `NAMED_ROLES_SQL` (`code_roles_by_subject_name`'s first reader), `roles_on_in`; `stored_role`, `with_candidates` out of `roles_in` |
| store `code.rs`, `lib.rs` | `code_snapshot_with_roles(symbols, kinds)`, `CodeSnapshot.roles`; `code_snapshot` unchanged |
| CLI `symbols.rs`, `lib.rs` | `SymbolTarget.roles`, `SymbolRole` (11 keys), `Marks::{with_roles, roles_on, no_roles}`, `role_line`; role lines in `target_lines` (the cap); roles read only with `frameworks` |
| CLI `roles.rs`, `code_index.rs`, `refresh.rs` | `RoleState::word()`; `LAYER` `pub(crate)`; comments |
| MCP `mirror.rs` | `SymbolsTarget.roles`, `SymbolsRole` (strings, integers) |
| code `bevy/mod.rs`, `frameworks.rs`, `lib.rs` | B5 `qualifier_text`; `PATH_FORM`, `CODE_ORIGIN`; `CODE_INDEX_FORMAT` 4 |
| core `code_toml.rs`, eval `code_index.rs` | comments re-pointed |
| `plugin.json` | 0.10.0 |

Deviations, accepted in review:

- D1: the code crate gains `PATH_FORM`, `CODE_ORIGIN` (the spec said code unchanged): the CLI drops `path`, `code` from text by comparing with them, as with `CERTAIN`.
- D2: the CLI's canon citations (Carry-overs).
- D3: a new store method; `code_snapshot`'s callers unchanged.
- D4: a QUERY reads role rows for every hit, not only the shown ones; output unchanged (`code-roles.md` "Known limits"; a TE-8 breach passes the shown symbols only).
- D5: layer `b` first is `Reverse(layer)`, untestable while 10b is parked.
- K2: B5 changes a stored subject, so `CODE_INDEX_FORMAT` 4 (history `4 8a7cd52d…f276`; the dumps as under 3); the spec's "No format bump" is void.

Review nits left: a bracketed plain type (`code-roles.md` "Known limits"); `with_roles` builds a row per candidate. Tests: round 1 green, 2965/2965, 8/8 mutations killed, the three hashes pinned (`PINS` 0.10.0, `SCHEMA_PINS` 0.9.0, 0.10.0), `doc_pointers` over the CLI's 73 canon citations, 11 test citations re-pointed to `code-roles.md`; TE-8 the owner's.
