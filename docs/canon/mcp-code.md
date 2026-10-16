---
class: canon
tier: 2
scope: [crates/specengine-mcp, crates/specengine-cli]
owner: owner
reviewed: 2026-10-16
---

# MCP code tools: `find_symbols`, `refs`, `unmapped`

The MCP twins of `spec symbols`, `spec refs` and `spec unmapped` (07 §1.2): which item sits at a name or `PATH:LINE` and what binds it, what binds a node, where code has no spec, without Bash or reading files. Each is one CLI library call through `read.rs`' `call` and `Answer`, so the read tools' contract holds unchanged (`mcp-read.md`: "Tools" parity, annotations, `_meta`, flat schemas, mirrors; "No project, discovery, state"; "Size"). Here only what is the code tools' own; the commands: `symbol-reads.md`, `spec-cli-refs.md`, `symbol-index.md`. 6b-2 adds `check_binding` (`docs/features/verify-node.md`).

## Tools

| Tool | Input (`?` nullable) | ≙ `spec … --json` |
|---|---|---|
| `find_symbols` | `query?`, `node?`, `file?`, `at?`, `limit?` | `symbols QUERY [--limit N] \| --node REF \| --file PATH \| --at PATH:LINE` |
| `refs` | `id` | `refs REF` |
| `unmapped` | `path?` | `unmapped [PATH]` |

- **Inputs**: the CLI's flag names, a positional REF `id`. `query`, `node`, `file`, `at`, `path`: `["string","null"]`; `limit`: `["integer","null"]`, `format: int64`, as `search`'s; `id` a required string; `additionalProperties: false`, nothing nested.
- **Forms** (`find_symbols`), judged before discovery, each an error result of the text + `\n`, no `structuredContent`: first the count (a `null` is absent), not exactly one -> ``spec: find_symbols takes exactly one of `query`, `node`, `file` and `at` ``; then `limit` without `query` -> ``spec: find_symbols takes `limit` with `query` only``; then a negative `limit` -> the CLI's own `spec: --limit N: the limit is 1 to 200` (`symbols_limit_error`). 0 or 201 reach the library: the same line, exit 2. `query` passes as one positional, `at` raw to the CLI's one `PATH:LINE` reader: MCP never splits a qpath or `PATH:LINE`.
- **Results**: `content` = `spec … 2>&1`, `structuredContent` = the `--json` document; MCP never re-sorts or re-cuts.

| Exit | Result |
|---|---|
| 0 | success, zero rows too |
| 1 | `find_symbols {node}`, `refs {id}`: the REF names nothing -> `isError`, the text and the exit-1 document (`reason`) |
| 2 | `isError`, the `CliError` line(s), no `structuredContent`: usage, discovery, config, no `[code]`, a walk gap, PATH not a walked code file, a bad LINE, a limit out of bounds |

## Mirrors

`mirror.rs`, every key `required`, absent `null`, key for key the CLI's serializers (`RefsJson`, `SymbolsJson`, `UnmappedJson`):

- `refs`: `{reference, reason, notes, nodes: [{id, kind, title, path, line, rev, rev_written, markers, mentions}], code, omitted {markers, mentions}}`; a marker `{relation, path, line, column, token, id, rev, id_state, fix, rev_state, levels, levels_state, note, binding {kind, qpath, item_kind, line, ambiguity, category}}`; a mention `{path, line, column, written, id_state, fix, binding}`.
- `find_symbols`, `unmapped`: `symbol-reads.md` "spec symbols", "spec unmapped"; `percent` a number, null for `-`; `total` a row; `tier` null outside `query`.
- **Closed enums**: `form` query, node, file, at; `target` item, module, ron; `tier` exact, suffix, name; `ambiguity` path_attribute, unrooted, duplicate; `relation` implements, verifies, configures, assumes; `id_state` resolved, alias, ambiguous, homoglyph, other_project, queue, dangling; `rev_state` unversioned, bad_rev, rev_unknown, current, spec_ahead, predated; `levels_state` default, declared, unclosed, unknown, duplicate, empty; binding `kind` item, module, ron, unanchored, cannot_verify, file. **Free strings**: `item_kind`, `layer`, `category`, `levels` items, node `kind`; `SymbolsTarget.roles` (nullable): each `SymbolsRole` key a free string, `line` and `candidates` integers.
- The enums are hand-written: a word a code crate's `ALL` list gains (`Relation::ALL`) passes the CLI but breaks the `outputSchema` until mirrored; the parity walker sees only the values the fixtures produce.

## Texts

`INSTRUCTIONS`' code line: `mcp-read.md` **Texts**. Descriptions (ASCII, <= 2 048 B): `find_symbols` 1 755, `refs` 1 177, `unmapped` 1 144; each holds the determinism sentence, its CLI twin and the `` `[code]` `` need. Compile-time asserts tie each stated number to its CLI constant: `OUTPUT_CAP_CHARS` 40 000 in each, `SYMBOLS_LIMIT_{MIN,MAX,DEFAULT}` 1/200/20 in `find_symbols`' and `limit`'s. No stack, kind or Rust word (`mcp_genre`).

## Freshness, writes, size

- Each call refreshes the md index, then the code index, inside the CLI read (`refresh_code`), and reads the rows as one snapshot (`symbol-reads.md` "Crates and seams"); MCP keeps nothing between calls. The first call after a `CODE_INDEX_FORMAT` change re-derives every file (cold).
- Writes: the data directory only; never `spec.lock`, a queue row, an event or a file under the root; no `index`, `lock accept` or drift operation (`report_drift` alone queues drift; 08 AC-3).
- MCP cuts nothing: the CLI's `OUTPUT_CAP_CHARS`. `mcp_size` maxima (text + JSON, characters): `find_symbols` 129 479, `refs` 135 868, `unmapped` 103 777, of `MAX_RESULT_CHARS` 500 000.
- Latency: warm per tool against `mcp-read.md` "Latency"'s 1 s; the owner's pilot run (`mcp_code_pilot.rs`, `#[ignore]`) is open, `find_symbols` with and without `[code] frameworks` too (`bevy-roles-reads.md` TE-8); a breach plans 6c.

## Tests

`crates/specengine-mcp/tests`: `mcp_code` (listing, forms, parity per exit over scratch `fixtures/code-index/` copies and roles over a `fixtures/code-bevy/` one, mirrors, determinism, session, quiet, genre, the single door), `mcp_door` (`fixtures/code-a/` with its lock), `mcp_read` (flat schemas, flag names), `mcp_default`, `mcp_index`, `mcp_size`; `plugin_skills`, `plugin_files` (0.10.0; SR7 `tool_schemas_follow_the_plugin_version`, `SCHEMA_PINS` 0.9.0 `6e456d6f…e1c9`, 0.10.0 `a3ac083d…4cff`); `mcp_code_pilot` (`#[ignore]`).
