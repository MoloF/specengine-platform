---
class: spec
status: draft
scope: [crates/specengine-mcp, crates/specengine-cli, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 6b of symbol-index (WA-16..WA-20; Q1, Q4-Q8 decided under the owner's pre-approval, review open); 05 s5.3; 07 s1.2; 08 s2, AC-3
adrs: []
---

# code-tools: MCP find_symbols, refs, unmapped, check_binding; spec verify --node

## Why

An agent asks which item sits at a name or `path:line` and what binds it, what binds a node, where code has no spec, whether a node's bindings still hold. 6a (`symbol-index.md`) and shipped `spec refs` (`docs/canon/code-bindings.md`) answer the first three on the CLI, 4a (`spec-verify.md`) judges the whole tree only; MCP has none (07 s1.2): agents run Bash or read files.

Delivered: four read tools, twins of `spec symbols`, `refs`, `unmapped` and the new `spec verify --node REF [--symbol QPATH]` (Q5); verify's report kept at exit 2 in MCP (WA-18); skills naming them; `TOOLS` 18, a MINOR plugin version. Slice 6 split (Q1): 6a, 6b, 6c if AC-10 fails. No ADR: ADR-0004/0005, ADR-0012/0042, ADR-0008 applied ("Rules").

## Description and interactions

- **An agent**: before an edit `find_symbols {at}` or `{query}` for an item and its markers, `refs {id}` for what binds a node; before handing in `check_binding {node}` per target (4a's plain rows). **The owner**: `spec verify --node`, `spec unmapped`.
- **Crates**: CLI the `--node` selection and row keys over 4a's command; MCP `code.rs` (new: the four tools over the CLI library), `mirror.rs`, `server.rs` (`INSTRUCTIONS`, the exit-2 rule); `plugin/` skills, version, `PINS`. MCP `[dependencies]` stay 7 (`build_graph.rs`); store, core, model unchanged.
- **Freshness**: each tool refreshes the md, then 6a's code index (`symbol-index.md` WA-6); `verify --node` uses the index only to pick files, then reads, parses, hashes them afresh (05:162).
- **Unchanged**: `spec check`, hook, gate, CI, `INDEX_FORMAT`, queue schema, HTTP (Q8; `daemon-keys`), UI, other tools, verify but for the row keys. **Writes**: the data directory only; never `spec.lock`, a queue row, an event, a file under the root.
- **Overridden lines**, fixed at shipping (SW-2): 07:30-37 (`find_symbols` `node_id`, `path:line`, "Bevy layer"; `refs` "tests, records"; `check_binding {node_id, qpath}`); `docs/canon/spec-verify.md` (its command line, "Rows"); `mcp-read.md` "Parity" (exit 2), "Texts"; `plugin-files.md` "Content" ("all thirteen named", stale at fourteen).

## Data

### Tools

WA-17, WA-20. **Every MCP tool's argument names are its CLI flag names, a positional REF `id`** (`get_node`, `refs`; 7b's `get_impact {id, symbol, since, depth}`, 2026-10-10); the read tools' annotations and `_meta` (`mcp-read.md` "Tools"); `outputSchema` the mirror of the `--json` document.

| Tool | Input (`?` nullable) | ≙ `spec … --json` |
|---|---|---|
| `find_symbols` | `query?`, `node?`, `file?`, `at?` (`PATH:LINE`), `limit?` | `symbols QUERY \| --node REF \| --file PATH \| --at PATH:LINE [--limit N]` (`symbol-index.md` "spec symbols") |
| `refs` | `id` | `refs REF` (`code-bindings.md` "`spec refs`") |
| `unmapped` | `path?` | `unmapped [PATH]` (`symbol-index.md` "spec unmapped") |
| `check_binding` | `node`, `symbol?` | `verify --node REF [--symbol QPATH]` ("spec verify --node") |

- **Input schemas**, flat (`mcp-read.md` "Schemas"): strings, `limit` as `search`'s, optionals `[T, "null"]`, `additionalProperties: false`, no root `anyOf`/`oneOf`/`allOf`, no `$ref`: `check_binding` `{"node": string, "symbol": [string, null]}`, `required: ["node"]`. `find_symbols`' four forms are four nullable keys; `query` passes as one positional (6a joins positionals with a space).
- **Form errors**, per surface as `get_task`'s (`task-package.md` "MCP"; SW refinement): not exactly one form -> error result ``spec: find_symbols takes exactly one of `query`, `node`, `file` and `at` ``; `limit` without `query` -> ``spec: find_symbols takes `limit` with `query` only``. The CLI's (6a): usage, exit 2.
- **Output mirrors** (`mirror.rs`): every key `required`, absent `null`; enums: `relation`, `id_state`, `rev_state`, `levels_state`, binding `kind`, `form`, `tier`, `ambiguity`, `state`, `class`, `verdict`, `compared`, `base`, `anchor` (5c: `exact`, `moved`, `ambiguous`, `lost`; 2026-10-12, `anchor-states.md`); free strings: `item_kind`, `layer`, `category`, `detail` (5a, 5c add), node kinds.

### Results and exits

`content` = `spec <…> 2>&1`, `structuredContent` = the `--json` document (`mcp-read.md` "Parity"); MCP never re-sorts or re-cuts.

| Exit | `find_symbols`, `refs`, `unmapped` | `check_binding` |
|---|---|---|
| 0 | success, zero rows too | `clean`, `observed`: success |
| 1 | `find_symbols {node}`, `refs`: REF names nothing -> `isError`, text, the exit-1 document | `blocked` (`enforce*` and a Fix row) -> `isError`, text, document |
| 2 | `isError`, the `CliError` line(s), no `structuredContent`: usage, discovery, config, no `[code]` (`code-refs.md`'s sentence), a walk gap, a bad PATH or LINE | a report with `cannot:` lines -> `isError`, text **and document** (WA-18); usage, discovery, REF: as the left column |

WA-18: MCP takes the CLI's own exception (`--json` "none for 2 (`check`: its report)", CLI README "Exit codes and streams"): a report outcome, here verify's, keeps its document at exit 2; precedent: the daemon's `answered` (`crates/specengine-http/README.md`).

### spec verify --node

WA-16, Q5. `spec verify --node REF [--symbol QPATH] [--json]` (+ `--root`, `--config`), plain only: with `--changed` (4b-2's `--staged`, `rev-rule.md`; 8a's `--task`, `--tests` alike), or `--symbol` alone: usage, exit 2. Order: config, `[verify]`, REF, the md and code index refresh, selection, lock, files, judge.

- **REF**: `spec show`'s resolver; naming nothing, a look-alike, `project:`: exit 2, stdout empty, show's `spec: <reason>` (SW refinement: ADR-0042 keeps exit 1 for Fix alone); several holders: one `warning:`, all of them.
- **Rows**: plain `spec verify`'s rows (4a "Command and rows", "States") that `spec refs REF` lists under REF's holders: holding a marker listed there (`code-bindings.md` "ID state": `resolved`, `alias`, each `ambiguous`, `homoglyph` under its fix's node), or a lock entry whose `node` is a holder's ID; never sections' or parents'. `--symbol`: then rows whose `symbol` is QPATH byte for byte (`ron` also `<file>#<symbol>`); no tiers (SW refinement: qpaths come from `find_symbols`, `refs`; a gone symbol still matches its lock row; 2026-10-12, `anchor-states.md`: so does a moved row whose `anchor.from` symbol is QPATH, anchors paired before the filter). Zero rows: exit 0, `clean`, `note: verify: <REF> has no binding[ on <QPATH>]`.
- **Files**: the rows' (markers' from the refreshed index, entries' from `spec.lock`); only they are read, blob-hashed, parsed, hashed. A walk gap anywhere stays 4a's run cause. Judge, classes, mode (`enforce-introduced` as `enforce`, 4a's note), verdict, exits: 4a's; `counts`, `cascade`, `delta` over the selected rows; `base` null, `pre_existing` false, `skipped` 0.
- **Text**: 4a's row lines, `ok` rows too (class `-`), hints, the summary `verify --node <REF>[ --symbol <QPATH>]: ` + 4a's after `verify: `; cut at `OUTPUT_CAP_CHARS`, then `[truncated: <k> rows not shown; narrow with --symbol QPATH]`.
- **JSON**: 4a's, rows the printed ones; `node` (holders' IDs), `symbol` after `base`; `omitted` `{rows}` last; the three null without `--node`.

**Row keys**, every verify (SW refinement: `--node` rows equal a full run's): `ast_hash` after `compared`, the item's v2 hash on disk for an `item` row whose item exists error-free, else null: level 1 equal: E's `ast_hash` (same bytes and recipe; nothing hashed, 4a's cascade unchanged); else `hash_item` over this run's parse; never the index's level columns (05:162). 2026-10-12, `lock-levels.md` "Level-aware verify": format 2 has no `ast_hash`; 5b's row key `levels` (`{lock, markers, changed}`) follows `compared`, and this key, the JSON below and RD-6 are restated over the level digests before 6b's code. Each `markers` item `{line, rev, id_state, rev_state}` (`code-bindings.md`).

```
check | code_ahead code_changed | LAW-BOUND | implements | crates/tally/src/counter.rs | item crates/tally::counter::impl Tally #1 impl_item
- | ok | LAW-BOUND | configures | data/limits.ron | ron root.cap
verify --node LAW-BOUND: observed [observe]: 0 fix, 1 check (0 pre-existing), 0 cannot, 1 ok; 1 by blob, 1 by ast; 0 skipped
```

```json
{"mode":"observe","base":null,"node":["LAW-BOUND"],"symbol":null,"verdict":"observed",
 "counts":{"rows":2,"ok":1,"fix":0,"check":1,"cannot":0,"pre_existing":0,"skipped":0},"cascade":{"by_blob":1,"by_ast":1},
 "rows":[{"node":"LAW-BOUND","relation":"implements","file":"crates/tally/src/counter.rs","kind":"item",
   "symbol":"crates/tally::counter::impl Tally #1","item_kind":"impl_item","state":"code_ahead","category":"code_changed",
   "detail":null,"class":"check","pre_existing":false,"markers":[{"line":21,"rev":3,"id_state":"resolved","rev_state":"current"}],
   "node_rev":3,"lock":{"rev":3,"commit":"<40 hex>","verified_at":"2026-10-10"},"compared":"ast","ast_hash":"<64 hex>"},{…}],
 "delta":{…},"cannot":[],"notes":[],"omitted":null}
```

### INSTRUCTIONS

WA-17. Today 1 885 B, 2 042 with `probes`, against `TEXT_LIMIT` 2 048 (`server.rs`'s asserts). 6b's text, measured (SW refinement): **1 928 B, 2 004 with `probes`: 44 B left.**

- The reads become two lines (`spec ` and two glosses dropped; the flag map keeps `with ["links"]`):

```
Reads:
- get_tree(root?, depth?, kinds?, archive?) = tree; get_node(id, with?, archive?) = show; search(query, kinds?, limit?, archive?) = search; get_context_bundle(node_ids, budget?) = bundle; get_proposal(proposal_id) = review --brief.
- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; unmapped(path?) = unmapped; check_binding(node, symbol?) = verify --node: its bindings vs spec.lock.
```

- `a project's specification as Markdown files in git` -> `a project's spec as Markdown in git`; `Hints use the command's flags:` -> `Hints use flags:`; the probes paragraph -> `Probes build: review_proposal, probe_output, probe_sleep (owner's checks).`
- Kept: the queue and tasks lines, REF, the cut, resources, the four rules. BLAKE3 pins (`mcp_decision.rs`, `mcp_path.rs`, `mcp_create.rs`) re-pinned.

**Descriptions**: each ≤ 2 048 B, ASCII, with the determinism sentence, its CLI twin and the `[code]` need; asserts tie stated numbers to CLI constants (6a's `limit` 1/200/20, `OUTPUT_CAP_CHARS`); no stack word or kind (`mcp_genre`). `check_binding`'s: plain verify of one node, nothing written, a report kept at exit 2.

### Plugin

WA-19. Bodies only: `read-spec` names `find_symbols`, `refs`, `unmapped` (after the bundle, before files); `implement` names `check_binding` (each target before `report_run`; a Fix or cannot row fixed or raised with a working answer, never hidden). Descriptions unchanged (1 170 of 1 200 B); bodies ≤ 4 096 B (both ~2 280); all eighteen tools backticked (`plugin-files.md` "Content"); the next MINOR (0.8.0 if first), a `PINS` line.

## Rules and edge cases

- WHEN `check_binding` and `spec verify --json` run on one state, its rows SHALL equal verify's filtered by "Rows" (TE-3).
- WHEN a tool runs from two roots, another cwd, in reverse creation order or on CRLF copies, its result SHALL be byte-identical, holding no root, `HOME` or date.
- Single door (08 AC-3): no tool writes `spec.lock`, a queue row, an event or a file under the root, or exposes `index`, `lock accept`, `propose drift`.
- ADR-0012, ADR-0042: no status, claim, approval, task transition or gate reads a `check_binding` result.
- Genre (`architecture.md#universal`): no stack, kind, fixture or Rust word in MCP sources and texts; qpaths, `item_kind` are data.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red:

- [ ] RD-1 -- `tools/list`, both eras: the eighteen; `TOOLS` 18, `CODE_TOOLS` 4 (`tests/common/read.rs`); annotations, `_meta`. M: a tool without a door call -> `mcp_door`'s coverage; a description over 2 048 B or without the sentence -> `mcp_default`.
- [ ] RD-2 -- forms. M: zero or two forms reaching the CLI, or `limit` with `node` -> `find_symbols_takes_exactly_one_form`; `--node` with `--changed` -> `verify_node_is_plain_only`; `--symbol` alone -> `verify_symbol_needs_node`.
- [ ] RD-3 -- parity per tool and exit on `code-a`. M: an MCP-side sort or cut, or exit 1 as success -> `code_tools_equal_the_cli`.
- [ ] RD-4 -- schemas. M: a nested or `oneOf` input -> `mcp_read`'s flat-schema test; a CLI key without its mirror -> the schema walker; `item_kind` an enum -> `code_mirrors_keep_kinds_free`.
- [ ] RD-5 -- `--node`. M: own states or filter -> `check_binding_equals_the_verify_rows`; sections taken -> `verify_node_lists_the_node_not_its_sections`; a homoglyph marker dropped -> `verify_node_follows_refs_listing`; counts over all rows -> `verify_node_counts_its_rows_only`; REF naming nothing at exit 1 -> `verify_node_naming_nothing_is_exit_2`; tiers on QPATH -> `verify_symbol_is_byte_exact`.
- [ ] RD-6 -- row keys. M: the index's level digests read (their columns tampered; 2026-10-13, `symbol-index.md` Q11) -> `check_binding_levels_are_fresh`; a blob-equal row hashed -> 4a's `blob_equal_entries_are_not_hashed`; a key dropped -> `verify_json_every_key_present`.
- [ ] RD-7 -- WA-18. M: the exit-2 document dropped -> `check_binding_cannot_verify_keeps_its_rows`; a document at a usage exit 2 -> `check_binding_usage_has_no_document`; another tool's exit-2 document kept -> `mcp_read`'s exit-2 test.
- [ ] RD-8 -- `INSTRUCTIONS`. M: a tool unnamed -> `mcp_default`; over 2 048 B, `probes` included -> `server.rs`'s asserts; text changed, pin kept -> `mcp_decision.rs`.
- [ ] RD-9 -- size: text + `structuredContent` ≤ `MAX_RESULT_CHARS` per tool. M: `unmapped` JSON uncut -> `mcp_size`; `--node` uncut -> `verify_node_cut_names_narrowing`.
- [ ] RD-10 -- read-only. M: a write under the root or a queue row -> `mcp_door`; a task transition reading a result -> `no_task_transition_reads_a_finding`.
- [ ] RD-11 -- graph. M: a store or `rusqlite` edge in MCP -> `build_graph.rs` (7); an HTTP route -> `fixtures/daemon-keys.json`'s test.
- [ ] RD-12 -- plugin. M: a tool unnamed -> `plugin_skills.rs`; version unchanged -> `plugin_files.rs`; descriptions over 1 200 B -> `plugin_skills.rs`.

test-engineer:

- [ ] TE-1 -- `mcp_code.rs` over scratch `code-a` copies, both eras: forms, exits; determinism (two roots, reverse creation, CRLF); session (an edited `.rs` shows in the next `find_symbols`, no `spec index`); quiet (broken, non-UTF-8 `.rs`: stdout JSON-RPC only); genre (`spec-c`: an error result naming `[code]`); `mcp_index`; `mcp_size` maxima recorded.
- [ ] TE-2 -- single door: a temp git repo with `code-a` and slice 3's golden `spec.lock`; every tool valid and invalid; then `git status --porcelain=v1 -z --untracked-files=all`, `.git/index` mtime, `spec.lock` bytes, queue rows unchanged.
- [ ] TE-3 -- `check_binding` over 4a's TE-1 scenarios: per scenario and node, rows equal `spec verify --json`'s filtered; each state and category met once.
- [ ] TE-4 -- cost: 1k unrelated code files, warm index: `verify --node` against plain verify, latency recorded ([A] well below; else 6c).
- [ ] TE-5 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground, scratch `HOME`, read-only): latency per tool, `unmapped` totals; no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec; dated notes in `symbol-index.md`, `spec-verify.md`, 08 s2; `export index`, `check` clean.
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/mcp-code.md` (≤ 12 288 B; `mcp-read.md`, at 12 276: a pointer row, "Parity", "Texts" reworded in place); MCP README; the `spec-verify` canon (`--node`, `--symbol`, row keys); `plugin-files.md`, `plugin.md` "Skills"; 07:30-37, 08:44; `CLAUDE.md` State.
- [x] SW-3 -- no ADR.

## Risks

`INSTRUCTIONS` keeps 44 B after 6b; 4c-1's `report_drift` line (~108 B), 7a's `--bindings` flag (~33 B), 7b's `get_impact` (46 B) want ~190: each re-measures after 6b, cutting the probes paragraph (76 B) or argument lists, never a rule. `check_binding`'s exit 1 under `enforce*` is `isError` (parity, described). Drafts 3, 4a, 6a under it: a change to verify's rows or 6a's JSON reopens "spec verify --node" and the mirrors. Each call refreshes the code index (6a's `noop_ms`) against MCP's 1 s (`mcp-read.md` "Latency").

## Out of scope

HTTP routes, UI (Q8, Phase 4); `delta.symbols`, `get_impact` (7b, Q6); `with: ["bindings"]`, bindings in reads (7a); `--unbound` (Q4); fuzzy search, references, call graphs (LSP's); spec-side referrers in `refs` (`get_node` with links); `verify --node --changed`, a base; writing markers or binding code (07's former `bind_code_symbol`); the Bevy layer (10a `bevy-roles.md`, 10b `bevy-dump.md`, 2026-10-10: `find_symbols`' mirror gains `roles`, then `instances`, `flags`, `dumps`, free strings; whichever of 6b, 10a/10b ships second carries them); code resources.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 split | 6a, then 6b; 6c only if AC-10 fails | tools re-walk the tree per call |
| Q4 unbound | none; `unmapped {path}` lists a file's items | `unbound` overloaded |
| Q5 `check_binding` | twin of `spec verify --node REF [--symbol QPATH]` | a judge of its own drifts |
| Q6 `delta.symbols` | 7b `impact` | a git reader here |
| Q7 coverage | 6a's containment; mentions apart | path classes break genre |
| Q8 HTTP | none now | keys before Phase 4 screens |
| SW `INSTRUCTIONS` | 6b's text; **4c-1, 7a, 7b re-measure after 6b** and make their own room | 6b cuts for drafts that may change |
| Names (2026-10-10) | CLI flag names, REF `id`, every tool; 7b amended | 07's `node_id`, `qpath`: two vocabularies |
| SW REF | naming nothing: exit 2 | exit 1 breaks ADR-0042's Fix-only exit 1 |

WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Working answers

[P] checked, [A] assumed. WA-16 [P/A] `check_binding` = `verify --node` (4a's judge, rows restricted). WA-17 [P] the mirror rules (`mcp-read.md` "Parity", "Schemas", "Texts"). WA-18 [P/A] MCP drops every exit-2 document; 4a prints its report at exit 2. WA-19 [P] `plugin.json` 0.7.0; a newly named tool: MINOR. WA-20 [P] no HTTP route.

## Dependencies and ordering

- **Needs**: 1 `code-refs` (shipped); 3 `spec-lock` (the reader); 4a `spec-verify` (the judge, the command, its JSON); 6a `symbol-index` (index, `symbols`, `unmapped`, `refs` from it).
- **Order**: 6a and 4a -> 6b -> the `INSTRUCTIONS` lines of 4c-1, 7a, 7b; 6b after 5b and 5c (2026-10-13, `symbol-index.md` Q11), its row keys over the level digests. **Independent**: 8a, 9a, 9b. **Later**: Phase 4 (HTTP, UI).

## Implementation

Not started. To record: modules, `INSTRUCTIONS` bytes, `mcp_size` maxima, TE-4's and TE-5's figures, deviations with reasons.
