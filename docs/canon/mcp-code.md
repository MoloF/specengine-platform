---
class: canon
tier: 2
scope: [crates/specengine-mcp, crates/specengine-cli]
owner: owner
reviewed: 2026-10-22
---

# MCP code tools: `find_symbols`, `refs`, `unmapped`, `check_binding`

The MCP twins of `spec symbols`, `spec refs`, `spec unmapped` and `spec verify --node` (07 §1.2): which item sits at a name or `PATH:LINE` and what binds it, what binds a node, where code has no spec, whether a node's bindings still hold, without Bash or reading files. Each is one CLI library call through `read.rs`' `call` and `Answer`, so the read tools' contract holds unchanged (`mcp-read.md`: "Tools" parity, annotations, `_meta`, flat schemas, mirrors; "No project, discovery, state"; "Size"). Here only what is the code tools' own; the commands: `symbol-reads.md`, `spec-cli-refs.md`, `symbol-index.md`, `spec-verify-scope.md` "`--node`".

## Tools

| Tool | Input (`?` nullable) | ≙ `spec … --json` |
|---|---|---|
| `find_symbols` | `query?`, `node?`, `file?`, `at?`, `limit?` | `symbols QUERY [--limit N] \| --node REF \| --file PATH \| --at PATH:LINE` |
| `refs` | `id` | `refs REF` |
| `unmapped` | `path?` | `unmapped [PATH]` |
| `check_binding` | `node`, `symbol?` | `verify --node REF [--symbol QPATH]` |

- **Inputs**: the CLI's flag names, a positional REF `id`. `query`, `node`, `file`, `at`, `path`, `symbol`: `["string","null"]`; `limit`: `["integer","null"]`, `format: int64`, as `search`'s; `id`, `check_binding`'s `node` required strings; `additionalProperties: false`, nothing nested.
- **Forms** (`find_symbols`), judged before discovery, each an error result of the text + `\n`, no `structuredContent`: first the count (a `null` is absent), not exactly one -> ``spec: find_symbols takes exactly one of `query`, `node`, `file` and `at` ``; then `limit` without `query` -> ``spec: find_symbols takes `limit` with `query` only``; then a negative `limit` -> the CLI's own `spec: --limit N: the limit is 1 to 200` (`symbols_limit_error`). 0 or 201 reach the library: the same line, exit 2. `query` passes as one positional, `at` raw to the CLI's one `PATH:LINE` reader: MCP never splits a qpath or `PATH:LINE`.
- **Results**: `content` = `spec … 2>&1`, `structuredContent` = the `--json` document; MCP never re-sorts or re-cuts.
- **`check_binding`**: the server's root as the working tree, no base or task (`enforce-introduced` as `enforce`); `node`, `symbol` as written; no `task_id` form.

| Exit | Result |
|---|---|
| 0 | success, zero rows too |
| 1 | `find_symbols {node}`, `refs {id}`: the REF names nothing -> `isError`, the text and the exit-1 document (`reason`); `check_binding` `blocked` (a Fix row, `enforce*`): `isError`, text and document, a finding |
| 2 | `check_binding`'s report (a Cannot row, a run cause after REF) -> `isError`, text and document. Else `isError`, the `CliError` line(s), no `structuredContent`: usage, discovery, config, no `[code]`, a walk gap, PATH not a walked code file, a bad LINE, a limit out of bounds; `check_binding`'s REF naming nothing |

## Mirrors

`mirror.rs`, every key `required`, absent `null`, key for key the CLI's serializers (`RefsJson`, `SymbolsJson`, `UnmappedJson`, `VerifyJson`):

- `refs`: `{reference, reason, notes, nodes: [{id, kind, title, path, line, rev, rev_written, markers, mentions}], code, omitted {markers, mentions}}`; a marker `{relation, path, line, column, token, id, rev, id_state, fix, rev_state, levels, levels_state, note, binding {kind, qpath, item_kind, line, ambiguity, category}}`; a mention `{path, line, column, written, id_state, fix, binding}`.
- `find_symbols`, `unmapped`: `symbol-reads.md` "spec symbols", "spec unmapped"; `percent` a number, null for `-`; `total` a row; `tier` null outside `query`. Layer C (`layer-c-reads.md` "JSON"): a target's `c_state`, `moniker`, `agrees` and the document's `layer_c` (`SymbolsLayerC`: `toolchains` strings, `degraded` a boolean, the counts integers), each nullable.
- `check_binding` (`spec-verify-output.md` "JSON"): `{mode, base, task, node, symbol, verdict, counts, cascade, rows, delta {symbols, bindings {moved [{from, to}], ambiguous, lost, stale}, affected_nodes, tests_reaching}, tested, cannot, notes, omitted {rows}}`, `tested` a nullable array of `{node, state, category, rev, node_rev, commit, verified_at}` (`spec-lock-tested.md` "Plain verify": `null` in a base mode, under a run cause or when the blocks cannot be judged, `isError` unchanged), `category`, `node_rev` nullable, mirrored as `VerifyTested`, the reads' `tested` too (`spec-lock-tested.md` "Reads"); a row's 17 keys, `levels {lock, markers, changed}`, `anchor {state, from, candidates}`; a key six fields, a place four; `base`, `task`, `delta.symbols`, `delta.tests_reaching` (plain verify's list, never `--node`'s: `code-reads.md`) `{"type": "null"}` with a description; `node` `[string]`.
- **Closed enums**: `form` query, node, file, at; `target` item, module, ron; `find_symbols`' `tier` (`SymbolsTier`) exact, moniker, suffix, name, `get_impact`'s (`CodeTier`, its schema unchanged) exact, suffix, name; `c_state` (`SnapshotState`) moniker, local, none, unresolved, not_loaded, unmatched, unseen; `layer_c.state` (`SnapshotFreshness`) fresh, stale; `ambiguity` path_attribute, unrooted, duplicate; `relation` implements, verifies, configures, assumes; `id_state` resolved, alias, ambiguous, homoglyph, other_project, queue, dangling; `rev_state` unversioned, bad_rev, rev_unknown, current, spec_ahead, predated; `levels_state` default, declared, unclosed, unknown, duplicate, empty; binding `kind` item, module, ron, unanchored, cannot_verify, file; verify `mode` observe, enforce-introduced, enforce; `verdict` clean, observed, blocked, cannot-verify; `state` `spec-verify.md`'s nine; `class` fix, check, cannot; `compared` blob, ast; anchor `state` exact, moved, ambiguous, lost; its `kind` as `target`; `tested` `state` verified, stale, unverifiable, node_gone. **Free strings**: `item_kind`, `moniker`, `layer`, `category`, `detail`, `levels` items, node `kind`; `SymbolsTarget.roles` (nullable): each `SymbolsRole` key a free string, `line` and `candidates` integers.
- The enums are hand-written: a word a code crate's `ALL` list gains (`Relation::ALL`) passes the CLI but breaks the `outputSchema` until mirrored; the parity walker sees only the values the fixtures produce.

## Texts

`INSTRUCTIONS`' code line: `mcp-read.md` **Texts**. Descriptions (ASCII, <= 2 048 B): `find_symbols` 1 957 (its query line names the moniker tier; one sentence the stored snapshot, which a read never makes), `refs` 1 177, `unmapped` 1 144, `check_binding` 2 040; each holds the determinism sentence, its CLI twin and the `` `[code]` `` need; `check_binding`'s also `this session's worktree` and `` `enforce-introduced` is judged as `enforce` ``, and the agent's mends (a binding no longer reaching its code, both sides changed, a bad marker or one naming a revision its node has not reached, one it cannot verify) apart from the owner's (code or spec ahead, unlocked), no state word. Compile-time asserts tie each stated number to its CLI constant: `OUTPUT_CAP_CHARS` 40 000 in each, `SYMBOLS_LIMIT_{MIN,MAX,DEFAULT}` 1/200/20 in `find_symbols`' and `limit`'s. No stack, kind or Rust word (`mcp_genre`).

## Freshness, writes, size

- Each call refreshes the md index, then the code index, inside the CLI read (`refresh_code`; `check_binding` judges the files it selects), and reads the rows as one snapshot (`symbol-reads.md` "Crates and seams"); MCP keeps nothing between calls. The first call after a `CODE_INDEX_FORMAT` change re-derives every file (cold).
- Writes: the data directory only; never `spec.lock`, a queue row, an event or a file under the root; no `index`, `lock accept` or drift operation (`report_drift` alone queues drift; 08 AC-3).
- MCP cuts nothing: the CLI's `OUTPUT_CAP_CHARS`. `mcp_size` maxima (text + JSON, characters): `find_symbols` 129 479, `refs` 135 868, `unmapped` 103 777, `check_binding` 140 536 (3 000 rows; `delta` uncut), of `MAX_RESULT_CHARS` 500 000.
- Latency: warm per tool against `mcp-read.md` "Latency"'s 1 s; the owner's pilot run (`mcp_code_pilot.rs`, `#[ignore]`, `check_binding` too) is open, `find_symbols` with and without `[code] frameworks` too (`bevy-roles-reads.md` TE-8); a breach plans 6c.

## Tests

`crates/specengine-mcp/tests`: `mcp_code` (listing, forms, parity per exit over scratch `fixtures/code-index/` copies and roles over a `fixtures/code-bevy/` one, mirrors, determinism, session, quiet, genre, the single door over `fixtures/code-a/` with its lock; `check_binding` over a git copy of code-a with its lock and `fixtures/anchor-states`: parity, every key, no CRLF leg), `mcp_layer_c` (`find_symbols` shows a snapshot, the other code tools and `get_impact` equal with and without one; none spawns), `common/layer_c.rs`, `mcp_door`, `mcp_read` (flat schemas, flag names), `mcp_default`, `mcp_index`, `mcp_size` (a committed corpus); `plugin_skills`, `plugin_files` (0.20.0, `PINS` `bf7093719f78ab598a3746c22c46d8510cd1b41c4ab580f4ec959c5ad0e672e1`; SR7 `tool_schemas_follow_the_plugin_version`, `SCHEMA_PINS` 0.9.0 `6e456d6f…e1c9`, 0.10.0 `a3ac083d…4cff`, 0.11.0 `ff1d31c9…5520`, 0.12.0 `5af6b592…68a7`, 0.13.0 `3570759a…8f10`, 0.14.0 `257a8f33…565a`, 0.15.0 `70ff3720…3581`, 0.16.0 `e86927e8…9f83`, 0.17.0 `f7ad1114…37a7`, 0.18.0 `16250039…afa3`, 0.19.0 `4186f343…815b`, 0.20.0 `225e45ef…8703`); `mcp_code_pilot` (`#[ignore]`).
