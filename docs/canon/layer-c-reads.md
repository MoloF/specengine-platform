---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-mcp]
owner: owner
reviewed: 2026-10-22
---

# Layer C in the reads: `spec symbols` and `find_symbols`

While the owner's `spec resolve` snapshot is stored (`layer-c.md`), `spec symbols`, every form, shows each item's layer C state, moniker and agreement, and finds items by moniker; `find_symbols` mirrors it (`mcp-code.md` "Mirrors"). No other read, tool or route shows a moniker: `impact`, `get_impact`, `unmapped`, `refs`, `health`, `bundle`, `verify`, `lock`, `lock diff`, `show --bindings` and `resolve`'s counts read `code_snapshot`, never layer C. A read never spawns `specengine-ra`, never makes or refreshes a snapshot, never calls `layer_c_state()`, writes only the data directory and keeps every exit code. The moniker displays and finds; it never binds, keys `spec.lock`, judges or becomes a finding (ADR-0049, `architecture-code.md#code-identity`). Built by `docs/features/layer-c-reads.md`.

## Read

One read transaction (`symbol-reads.md` "Crates and seams"): store `code_snapshot_with_layer_c(symbols, kinds, moniker, pick)` is `code_snapshot` (with `kinds`, `code_snapshot_with_roles`) plus `CodeSnapshot.layer_c`, a `LayerCRead {head, tree, files, found, joined}` from `layer_c_read_in`, or `None` without a snapshot (then no layer C statement runs):

- `head_in`: the run's `code_resolve` row; `tree_rows_in`: the code index's files and packages; `resolve_files_in`: the run's files with their BLAKE3;
- `found_in`: with a QUERY, the rows whose raw moniker is `raw_moniker(query)`, by `(path, start)`; `FOUND_SQL` starts `INDEXED BY code_monikers_by_moniker`;
- `joined`: per path `pick` chooses, byte order, its layer C rows and its layer A symbols. `pick`: QUERY, the files of the first `limit` lookup hits plus the found rows' files (the merge only moves a hit up, so no shown target lies elsewhere); `--file`, `--at`, the file; `--node`, the bound targets' files.
- the role rows, with `kinds`, last: beside the snapshot's symbols, and of the picked files' symbols only those at a found item's start and end.

CLI `LayerCView::of(read, root, query)`: the state from that read's own tree rows (`Tree::of`, `Tree::state_of`; `layer-c.md` "Freshness"), each picked file through `join_rows` (the per-file join `resolve`'s counts use, over the code crate's `join_file`), and `found`: the found rows joined, matched to their files' symbols on equal start and end, kept where the shown moniker equals the query.

## Rows

| `c_state` | When |
|---|---|
| `moniker`, `local`, `none`, `unresolved`, `not_loaded` | the joined row's state (`layer-c.md` "Join") |
| `unmatched` | the file in the run, its BLAKE3 unchanged, no row joins the item |
| `unseen` | the file not in the run, or its BLAKE3 changed since: every item, none found |

`moniker`: the shown form, `<raw> #<value>` for a collision (`shown_moniker`, "Join"), null unless the state is `moniker`. `agrees`: "Join", null where it gives neither.

## Text

`spec symbols 'arena impl#[Health] #2'` on a scratch `fixtures/ra-mini`, stale:

```
arena::impl Health #2 | impl_item | arena/src/lib.rs:23-27 | layer a | mentions 0 | moniker
  sig impl Health
  c moniker | arena impl#[Health] #2 | agrees | stale
symbols: 1 of 1; code files 6 (.rs 6, .ron 0), skipped 0
layer c: stale (ra_ap 0.0.352); spec resolve refreshes it
```

- **`c` line**, item targets only, right after `  sig …`, before the role and binding lines: `  c <c_state>[ | <moniker>][ | agrees|disagrees][ | stale]`, every field through `one_line`, counted toward `OUTPUT_CAP_CHARS` (fewer targets fit with a snapshot).
- **`layer c:` line**, after the summary: `layer c: fresh (ra_ap <v>)` or `layer c: stale (ra_ap <v>); spec resolve refreshes it`.

## JSON

Each target after `roles`: `c_state`, `moniker` (strings or null), `agrees` (bool or null): 18 keys. The document after `code`: `layer_c`, null or `{state, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items}` (`SymbolsLayerC`): `state` `fresh` | `stale`, the rest the run's row, `toolchains` its distinct known toolchains, byte-sorted: 12 keys. Null on `module` and `ron` targets, while absent (never the word `absent`) and on exit 1, as `code`.

## Tier moniker

QUERY's tiers (`QueryTier`): `exact`, `moniker`, `suffix`, `name`. (1) raw = the query up to its first `COLLISION_SEPARATOR` (`raw_moniker` in `specengine-code`: the CLI holds no `#` for it); (2) `found_in`; (3) rows of `unseen` files dropped; (4) joined to layer A rows; (5) kept where the shown moniker equals the query. `merged_hits`: each item once at its best tier, within a tier the lookup's order (qpath, path, start line, `ord`), the limit after the merge, `total` the merged count; a stale snapshot answers too. A raw query whose moniker collides finds nothing by this tier; only the shown `<raw> #<v>` does. The store's `SymbolTier`, `LOOKUP_SQL`, `lookup_in` keep three tiers: `impact --symbol`, `get_impact`'s seeds and role links never take a moniker.

## States

- **Absent** (never resolved; dropped by `clear_code` or an `INDEX_FORMAT` change): the text is the bytes without layer C, the new keys null.
- **Stale** (`tree_fp` differs): values kept, every `c` line ends `| stale`; a changed file's rows `unseen`.
- **Partial**: a reply ending before `done` is never stored, a failed run keeps the previous snapshot; gaps inside a complete one show as `unseen`, `unmatched`, `unresolved`, `not_loaded`, `layer_c.degraded`.
- **Unknown stored state**: a picked file's joined row holds a state this build does not know (the first by path, then `ord`): that read shows no snapshot (no `c` or `layer c:` line, no moniker tier, the keys and `layer_c` null) and prints one ``warning: the layer C snapshot holds an unknown state `<x>`; run `spec resolve` ``; nothing is blocked, the exit unchanged; a read whose picked files hold none still shows it.

Cost: a snapshot adds no measurable time on ra-mini (TE-9: `symbols --file`, `symbols Health`, release, warm, under 10 ms). Determinism: today's row order; values only from the stored rows, the stored stamp and the tree's bytes, read in one transaction: two roots, either creation order, identical bytes.

## MCP

`find_symbols` (`mcp-code.md`): `tier` the closed `SymbolsTier` (four words), `c_state` the closed `SnapshotState` (seven), `layer_c.state` `SnapshotFreshness`; `get_impact`'s `CodeTier` keeps three, its schema unchanged. Description 1 957 B: the query line names the moniker tier, one sentence the stored snapshot; `QUERY_DOC` "A qualified path, a suffix of one, a bare name or a layer C moniker, as one value."; `INSTRUCTIONS` unchanged. Plugin 0.20.0 (`plugin-files.md` "Changing it").

## Tests

CLI `tests/layer_c_reads.rs` (the six-file tally, absent bytes against `fixtures/layer-c/symbols-head.json`, stale, `unseen`, the tier, the collision form, gaps, the cap, the unknown state, determinism, `layer_c_reads_cost` ignored), `layer_c_outputs.rs` (`symbols` out of the equal set, `impact`, `show --bindings`, `lock diff` in; never-spawn keeps it), `layer_c_scan.rs` (the split's scan, `reads_never_call_the_public_layer_c_state`), `symbol_reads*.rs`; store `moniker_lookup_searches_its_index` (the `FOUND_SQL` plan); MCP `mcp_code.rs`, `mcp_layer_c.rs`, `common/layer_c.rs`, `plugin_files.rs`. Snapshots come from 11a-2's fake on a scratch `fixtures/ra-mini`; no test builds `specengine-ra`.

## Known limits

- Every read with a snapshot builds the whole project's run and current BLAKE3 maps and `TargetCrates` (review 1: later).
- `INSTRUCTIONS` names no moniker (2 040 of 2 048 B).
- One pilot's 17 disagrees are not attributed (TE-6, `layer-c-resolve.md`).
