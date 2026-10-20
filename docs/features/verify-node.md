---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-mcp, plugin]
ref: Phase 3 plan 2026-10-10, slice 6b-2, split from code-tools 2026-10-16 (its Q28; WA-16..WA-19; Q5 2026-10-10; Q28-Q34 2026-10-16, decided under the owner's pre-approval, review open); 05 s5.3; 07 s1.2; 08 s2, AC-3
adrs: []
---

# verify-node (6b-2): `spec verify --node`, MCP `check_binding`

## Why

Before handing in, an agent asks whether a node's bindings still hold. Plain `spec verify` judges every lock key against the whole tree (`docs/canon/spec-verify.md`), and MCP never runs it (its opening): an agent runs Bash and reads the whole project's findings.

Delivered: `spec verify --node REF [--symbol QPATH]`, plain verify's rows restricted to one node's bindings, judged over the files the code index selects (Q5, Q30); `check_binding`, its MCP twin, a report kept at exit 2; the `implement` skill naming it; `TOOLS` 19, the next plugin MINOR at shipping (L2 2026-10-19: `layer-c-reads.md` may take 0.11.0 first). Split from `code-tools.md` (Q28): 6b-1 ships the three code reads after 6a-3; this ships after 5c, so its rows carry 5b-1's `levels` and 5c's `anchor` from the start. No ADR: ADR-0042 (judge, exits; `architecture-code.md#verify`), ADR-0046 (`#sync-in-reads`: verify's plain state over the refreshed code index, the lock and the read's own parse), ADR-0012/0039 (no gate reads a finding), ADR-0004/0005, ADR-0050 (format 2), ADR-0008 applied.

## Description and interactions

- **An agent**: `check_binding {node}` per target before `report_run`; a Fix or cannot row fixed or raised with a working answer. **The owner**: `spec verify --node`.
- **Crates**: CLI `verify.rs`, `verify_rows.rs` as 5b-1 and 5c leave them: `--node`, `--symbol`, the selection, the cap, and 6a-2's refresh called as one named CLI-library function shared with `refs` (working name `refresh_code`); store: 6a-2's Q17 read and `code_markers` only; core unchanged (`judge_row`, `verdict`). MCP: 6b-1's `code.rs` gains `check_binding`, `mirror.rs` verify's document, `server.rs` the `INSTRUCTIONS` clause; `read.rs` unchanged (S13). `plugin/`: `implement`, version, `PINS`. MCP `[dependencies]` stay 7.
- **Freshness** (Q30): `--node` refreshes the md index, then the code index, the one verify that does (6a-2's **Freshness** and RD-9 keep plain, `--changed`, `--staged` off it); it uses the index only to pick files and their names (Q17), then reads, blob-hashes, parses and hashes those files afresh, never the index's level columns (`spec-verify.md` "States": "never the index's column"; `docs/canon/symbol-index.md` "Guarantees").
- **Unchanged**: plain, `--changed` and `--staged` verify (rows, keys, text; their JSON gains three `null` keys), `spec check`, hook, gate, CI, `INDEX_FORMAT`, queue schema, HTTP (no route: `daemon-keys`), UI. **Writes**: the data directory only; never `spec.lock`, a queue row, an event or a file under the root.
- **Overridden lines**, fixed at shipping (SW-2; S12): `spec-verify.md`'s opening ("MCP … never run it"; the command line), "Cascade" ("One code walk (`read_tree`) per call": plain only); `spec-verify-output.md` "Text" (`ok` rows under `--node`), "JSON" (`node`, `symbol`, `omitted`); `mcp-read.md` "Tools" **Parity** ("exit 2 -> … no `structuredContent`": a rendered report keeps its document, as `read.rs` does: S13); 6a-2's canon **Freshness** (verify never refreshes code); 07:26, :37 (`check_binding {node_id, qpath}`, "ast_hash, sync"); `mcp-code.md` (6b-1's); `plugin-files.md` **Content** ("all eighteen named"), **Version**.

## Data

### spec verify --node

WA-16, Q5, Q30. `spec verify --node REF [--symbol QPATH] [--json]` (+ `--root`, `--config`), plain only: with `--changed`, `--staged` or `--task` (the clap pair 8a-2 left to this; 8a-1's `--tests` alike: `spec-verify-scope.md`, `verify-tests.md`), or `--symbol` alone: usage, exit 2. Order: config, `[verify]`, REF, the md and code index refresh, selection, lock, files, judge.

- **REF**: `spec show`'s resolver; naming nothing, a look-alike, `project:`: exit 2, stdout empty, show's `spec: <reason>` (SW refinement: ADR-0042 keeps exit 1 for Fix alone); several holders: one `warning:`, all of them.
- **Rows** (8a-1's shared "A row in a set" over REF's holders: `spec-verify-tests.md` "The node set", P2): plain verify's rows (`spec-verify.md` "Rows", "States"; 5b-1's levels, 5c's pairing) that `spec refs REF` lists under REF's holders: holding a marker listed there (`code-bindings.md` "ID state": `resolved`, `alias`, each `ambiguous`, `homoglyph` under its fix's node), or a lock entry whose `node` is a holder's ID; never sections' or parents'. `--symbol`: rows whose `symbol` is QPATH byte for byte (`ron` also `<file>#<symbol>`); no tiers (qpaths come from `find_symbols`, `refs`); a gone symbol still matches its lock row; so does a moved row whose `anchor.from` symbol is QPATH, anchors paired before the filter (`anchor-states.md`). Zero rows: exit 0, `clean`, `note: verify: <REF> has no binding[ on <QPATH>]`.
- **Files** (Q30): only the selected rows' files (markers' from the refreshed `code_markers`, each stored ID judged once, `Corpus::judge`; entries' from `spec.lock`) are read, blob-hashed, parsed and hashed; their qpaths and ambiguity come from 6a-2's Q17 read, no tree walk. A walk gap anywhere stays plain verify's run cause (`cannot:`, `rows` `[]`, exit 2 with the report), never the refresh's `CliError`. Judge, classes, mode (`enforce-introduced` as `enforce`: no base), verdict, exits: plain verify's; `counts`, `cascade`, `delta` over the selected rows; `base` null, `pre_existing` false, `skipped` 0.
- **Text**: plain verify's row lines (`spec-verify-output.md` "Text"), `ok` rows too (class `-`); a moved row its `moved from` line, `ok` or not (Q34; `docs/canon/spec-verify-anchors.md` "Output"); the hints; the summary `verify --node <REF>[ --symbol <QPATH>]: ` + plain's after `verify: `; cut at `OUTPUT_CAP_CHARS`, then `[truncated: <k> rows not shown; narrow with --symbol QPATH]`.
- **JSON**: plain verify's document (`spec-verify-output.md` "JSON"; rows with 5b-1's `levels`, 5c's `anchor`), `rows` the printed ones; `node` (the holders' IDs) and `symbol` after 8a-2's `task` (after `base`: `spec-verify-output.md` "JSON", P6), `omitted` `{rows}` last; the three `null` without `--node`; `TOP_KEYS` 10 -> 13. **No new row key** (Q29): `markers` stay `{line, rev}`; `id_state`, `rev_state` come from `refs` and `find_symbols {node}`.

```
check | code_ahead code_changed body | LAW-BOUND | implements | crates/tally/src/counter.rs | item crates/tally::counter::impl Tally #1 impl_item
- | ok | LAW-BOUND | configures | data/limits.ron | ron root.cap
verify --node LAW-BOUND: observed [observe]: 0 fix, 1 check, 0 cannot, 1 ok; 1 by blob, 1 by ast; 0 skipped
```

```json
{"mode":"observe","base":null,"task":null,"node":["LAW-BOUND"],"symbol":null,"verdict":"observed",
 "counts":{"rows":2,"ok":1,"fix":0,"check":1,"cannot":0,"pre_existing":0,"skipped":0},"cascade":{"by_blob":1,"by_ast":1},
 "rows":[{"node":"LAW-BOUND","relation":"implements","file":"crates/tally/src/counter.rs","kind":"item",
   "symbol":"crates/tally::counter::impl Tally #1","item_kind":"impl_item","state":"code_ahead","category":"code_changed",
   "detail":"body","class":"check","pre_existing":false,"markers":[{"line":21,"rev":3}],"node_rev":3,
   "lock":{"rev":3,"commit":"<40 hex>","verified_at":"2026-10-10"},"compared":"ast",
   "levels":{"lock":["sig","body"],"markers":["sig","body"],"changed":["body"]},
   "anchor":{"state":"exact","from":null,"candidates":null}},{…}],
 "delta":{…},"cannot":[],"notes":[],"omitted":{"rows":0}}
```

### check_binding

WA-17. `check_binding {node, symbol?}` ≙ `spec verify --node REF [--symbol QPATH] --json`, in 6b-1's `code.rs`, with the read annotations and `_meta`. Input `{"node": string, "symbol": [string, null]}`, `required: ["node"]`, flat (`mcp-read.md` **Schemas**). Output mirror (S16): the document above, every key `required`; enums `mode`, `base`, `verdict`, `state`, `class`, `compared`, `relation`, binding `kind`, `anchor.state` (`exact`, `moved`, `ambiguous`, `lost`); free strings `item_kind`, `category`, `detail`, node kinds; `levels` lists; `anchor` an object or null; `delta.bindings.moved` objects `{from, to}`.

| Exit | `check_binding` |
|---|---|
| 0 | `clean`, `observed`: success |
| 1 | `blocked` (`enforce*` and a Fix row) -> `isError`, the text, the document: a finding, not a tool failure (its description says so) |
| 2 | a report (`cannot:` lines, a run cause) -> `isError`, the text **and the document** (S13: `read.rs` keeps every rendered outcome's; no MCP code); usage, discovery, a REF naming nothing -> `isError`, the `CliError` line(s), no `structuredContent` |

### INSTRUCTIONS

Q31. 6b-1's code line (`code-tools.md` "INSTRUCTIONS") gains `; check_binding(node, symbol?) = verify --node` (46 B) before its full stop:

```
- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; unmapped(path?) = unmapped; check_binding(node, symbol?) = verify --node.
```

1 938 B, 1 800 less the tasks line, 2 014 with `probes`: 34 B left (over 6b-1's text as built: 1 892, 1 754, 1 968; `mcp-read.md` **Texts**). The four pins re-pinned as 6b-1's.

**Description** (≤ 2 048 B, ASCII, the determinism sentence): plain verify of one node, its CLI twin, the `[code]` need, nothing written; a report kept at exit 2; under `enforce*` a Fix row comes back `isError`, a finding, not a failure; `OUTPUT_CAP_CHARS` asserted.

### Plugin

WA-19. `implement`'s body names `check_binding` (each target before `report_run`; a Fix or cannot row fixed or raised with a working answer, never hidden). Descriptions unchanged; the body ≤ 4 096 B (2 521 B now, front-matter included); all nineteen tools named; the next MINOR, a `PINS` line and an entry in 10a-2's schema-pin list (P1, 2026-10-16: `bevy-roles-reads.md` takes 0.10.0; L2, 2026-10-19: 11a-3 `layer-c-reads.md` takes the next MINOR if it ships first).

### Fixtures

Q33. No new fixture: scratch copies of `fixtures/code-a/` with 5b-1's format-2 golden `fixtures/spec-lock/code-a.lock` (`lock-levels.md` "Fixtures and the golden"; a format-1 lock exits 2: S15), 4a's scenarios as the CLI's verify tests build them.

## Rules and edge cases

- WHEN `--node` and plain `spec verify --json` run on one state, `--node`'s rows SHALL equal plain verify's filtered by "Rows", every key and value (`levels`, `anchor`) included (TE-3).
- WHEN `--node` judges, it SHALL read only its rows' files and SHALL never read the index's digest columns (Q30).
- WHEN `check_binding` runs from two roots, another cwd, in reverse creation order or on CRLF copies, its result SHALL be byte-identical.
- Single door (08 AC-3): `check_binding` writes no `spec.lock`, queue row, event or file under the root, and exposes no `lock accept` or `propose drift` (`report_drift`'s).
- ADR-0012, ADR-0042: no status, claim, approval, task transition or gate reads a `check_binding` result.
- Genre (`architecture.md#universal`): no stack, kind, fixture or Rust word in MCP sources and texts; qpaths are data, compared byte for byte.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- `tools/list`, both eras: the nineteen; `TOOLS` 19, `CODE_TOOLS` 4. M: `check_binding` without a door call -> `mcp_door`'s coverage; its description over 2 048 B or without the sentence -> `mcp_default`.
- [ ] RD-2 -- forms. M: `--node` with `--changed` or `--staged` -> `verify_node_is_plain_only`; `--symbol` alone -> `verify_symbol_needs_node`.
- [ ] RD-3 -- selection. M: own states or filter -> `check_binding_equals_the_verify_rows`; sections taken -> `verify_node_lists_the_node_not_its_sections`; a homoglyph marker dropped -> `verify_node_follows_refs_listing`; counts over all rows -> `verify_node_counts_its_rows_only`; REF naming nothing at exit 1 -> `verify_node_naming_nothing_is_exit_2`; tiers on QPATH -> `verify_symbol_is_byte_exact`; a moved row's `from` ignored -> `verify_symbol_matches_a_moved_rows_from`.
- [ ] RD-4 -- files and freshness (Q30). M: a tree walk -> `verify_node_walks_no_tree`; `--node` without its refresh -> `verify_node_refreshes_the_code_index` (an edited `.rs` reads fresh); plain verify refreshing -> 6a-2's `verify_leaves_the_code_index_untouched`; the index's digest columns read (tampered) -> `check_binding_levels_are_fresh`.
- [ ] RD-5 -- keys (Q29). M: a row key added, or `node`, `symbol`, `omitted` absent from plain JSON -> `verify_json_every_key_present` (CLI `verify_output.rs`).
- [ ] RD-6 -- exit 2 (S13; no MCP code). M: `read.rs` drops a rendered exit-2 document -> `check_binding_cannot_verify_keeps_its_rows`; a document at a usage exit 2 -> `check_binding_usage_has_no_document`.
- [ ] RD-7 -- text and size (Q34). M: a moved `ok` row without its `moved from` line -> `verify_node_prints_moved_ok_rows`; `--node` uncut -> `verify_node_cut_names_narrowing`; over `MAX_RESULT_CHARS` -> `mcp_size`.
- [ ] RD-8 -- `INSTRUCTIONS` (Q31). M: `check_binding` unnamed -> `mcp_default`; over 2 048 B, `probes` included -> `server.rs`' asserts; the text changed, a pin kept -> `mcp_decision.rs`, `mcp_path.rs`, `mcp_tasks.rs` (1 938), `mcp_create.rs` (2 014).
- [ ] RD-9 -- read-only, no reader. M: a write under the root or a queue row -> `mcp_door`; a task transition reading a result -> `no_task_transition_reads_a_finding` (new: S18).
- [ ] RD-10 -- graph. M: a store or `rusqlite` edge in MCP -> `build_graph.rs` (7); an HTTP route -> `fixtures/daemon-keys.json`'s test.
- [ ] RD-11 -- plugin. M: `check_binding` unnamed -> `plugin_skills.rs`; the version unchanged -> `plugin_files.rs`.

test-engineer:

- [ ] TE-1 -- `mcp_code.rs` gains `check_binding` over scratch `code-a` copies with the format-2 golden, both eras: forms, exits 0, 1 (`enforce`), 2 with and without a report; determinism (two roots, reverse creation, CRLF); quiet; genre (`spec-c`: an error result naming `[code]`).
- [ ] TE-2 -- single door: a temp git repo with `code-a` and the format-2 golden; `check_binding` valid and invalid; then `git status --porcelain=v1 -z --untracked-files=all`, `.git/index` mtime, `spec.lock` bytes, queue rows unchanged.
- [ ] TE-3 -- over 4a's TE-1 scenarios on the format-2 golden: per scenario and node, `check_binding`'s rows equal `spec verify --json`'s filtered; each state, category and anchor state met once; a moved pair matched by `--symbol` on either key.
- [ ] TE-4 -- cost: 1k unrelated code files, warm index: `verify --node` against plain verify, latency recorded ([A] well below; else 6c).
- [ ] TE-5 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground, scratch `HOME`, read-only): `check_binding` latency per call; no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec, split from `code-tools.md` 2026-10-16 (Q28); 08 s2 lists 6b-2 after 5c.
- [ ] SW-2 -- at shipping (Q32), bytes measured 2026-10-16: a Tier 2 `docs/canon/spec-verify-node.md` (command, rows, files, text, JSON; about 3 KB of 12 288; or a section of 8a-2's `spec-verify-scope.md`, its Q4), its line in `docs/index-canon-spec.md` (2 592 of 10 240); `spec-verify.md` (11 710 before 5b-1 and 5c write it: the opening's MCP clause and command line, "Cascade"'s walk, one pointer; over 12 288 a section moves to a Tier 2 file of its own); `spec-verify-output.md` (5 380: `node`, `symbol`, `omitted`, `ok` rows); `mcp-code.md` (6b-1's: `check_binding`, its exits); `mcp-read.md` **Parity** reworded byte-neutral, **Texts** figures; the MCP README `code.rs` row; the CLI README (9 086: the `--node` line); `plugin-files.md` (nineteen, the version taken), `plugin.md` "Skills" `implement`; `architecture-code.md` (10 711: one "As built" pointer); 6a-2's canon **Freshness**; 07:26, :37; 08 s2, :44; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Divergence**: if 6a-2's store pass and the walk disagree on ambiguity (`docs/canon/symbol-index.md` "Cache key and cross-file pass", Ambiguity), `check_binding` contradicts `spec verify` (ADR-0046): 6a-2's RD-15 and TE-3 here guard it.
- **Churn**: 5b-1 and 5c rewrite `verify.rs`, `verify_rows.rs`; this starts on the commit after 5c ships.
- **Agent reading**: under `enforce*` a Fix row comes back `isError`; the description says it is a finding.
- **`INSTRUCTIONS`**: 34 B left; 7a (about 33 B) and 7b (46 B) re-measure and cut about 45 B elsewhere, never a rule.
- **Cost**: each call refreshes the code index (`noop_ms` ≤ 200) against MCP's 1 s (`mcp-read.md` "Latency"); TE-4 decides 6c.

## Out of scope

`find_symbols`, `refs`, `unmapped`, `read-spec` (6b-1 `code-tools.md`); `verify --node --changed`, a base; tiers on `--symbol`; a judge of its own; HTTP, UI (Q8); `delta.symbols` (7b); `sync` in reads (7a); writing markers or binding code.

## Open

| Q | Answer (else: the other answer's cost) |
|---|---|
| 5 `check_binding` | twin of `spec verify --node REF [--symbol QPATH]` (else: a judge of its own drifts) |
| SW REF | naming nothing: exit 2 (else: exit 1 breaks ADR-0042's Fix-only exit 1) |
| 28 split (owner) | this after 5c; 6b-1 `code-tools.md` after 6a-3 (else: `code-tools.md` Q28) |
| 29 row keys | none: `levels` (5b-1) and `anchor` (5c) answer 07:37's "ast_hash, sync"; `markers` stay `{line, rev}` (else: every verify row, every stored drift item and 5b-1's, 5c's goldens wider) |
| 30 `--node`'s files (owner) | only the files the index selects (6a-2's Q17 read) (else: a full plain run filtered, every call parsing the whole tree against MCP's 1 s) |
| 31 `INSTRUCTIONS` | the 46 B clause on 6b-1's code line: 34 B left (else: a line of its own, over the limit) |
| 32 canon home | a new Tier 2 `docs/canon/spec-verify-node.md` (`docs/index-canon-spec.md`), one pointer in `spec-verify.md`'s opening (else: `spec-verify.md`, which 5b-1 and 5c fill first from 578 B of room) |
| 33 fixtures | `code-a` with 5b-1's format-2 golden (else: a format-1 lock exits 2) |
| 34 moved `ok` rows | printed with their `moved from` line, as 5c prints a moved row (else: a second rule for one class of row) |

Q5 (2026-10-10) and Q28-Q34 (2026-10-16) decided under the owner's pre-approval, review open; numbers shared with `code-tools.md`, where Q1, Q4, Q6-Q8, Q35 live (Q28, Q31-Q33 in both). S-n: `code-tools.md`'s 2026-10-16 refresh's stale points; WA-n: the 2026-10-10 analysis' working answers; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs**: 6b-1 `code-tools` (`code.rs`, the mirrors, `mcp-code.md`, the code line); 6a-2 `symbol-index` (the refresh as a named CLI function, the Q17 read, `code_markers`, RD-9's scope); 4a `spec-verify` (the judge, shipped); 5b-1 (`lock-levels`: format 2, the golden; `lock-levels-verify`: `levels`); 5c `anchor-states` (`anchor`, `delta.bindings.moved`, Q34); 8a-1 `verify-tests` ("The node set", P2); 8a-2 `verify-task` (shipped 2026-10-20: `task` precedes `node`, P6).
- **Order** (Q28, 08 s2): 6a-2 -> 6a-3 -> 6b-1 -> 5b-1a -> 5b-2 -> 5b-1b -> 5c -> 6b-2 (this) -> 6c -> 7a; on the commit after 5c ships. 8a-1, 8a-2 shipped; Phase 6's roadmap before this (the owner, 2026-10-20). **Independent**: 9a, 9b.

## Implementation

Not started. From the split (2026-10-16, `code-tools.md` Q28): RD-2 <- its RD-2's verify flags; RD-3 <- its RD-5 (+ `verify_symbol_matches_a_moved_rows_from`); RD-4 new (Q30), with its RD-6's fresh-levels mutation; RD-5 <- its RD-6 without `ast_hash` (Q29); RD-6 <- its RD-7 (no MCP code: S13); RD-7 <- its RD-9's `--node` half and Q34; RD-8 <- its RD-8; RD-9 <- its RD-10's task half (a new test: S18); RD-10, RD-11 <- its RD-11, RD-12 (the next MINOR: P1, then L2 2026-10-19); TE-1, TE-2 <- its TE-1, TE-2's `check_binding` half on the format-2 golden; TE-3, TE-4 <- its TE-3, TE-4; TE-5 <- its TE-5's `check_binding`. To record: modules, `INSTRUCTIONS` bytes, TE-4's and TE-5's figures, deviations with reasons.
