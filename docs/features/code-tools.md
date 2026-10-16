---
class: spec
status: draft
scope: [crates/specengine-mcp, crates/specengine-cli, plugin]
ref: Phase 3 plan 2026-10-10, slice 6b of symbol-index (WA-16..WA-20; Q1, Q4-Q8 decided under the owner's pre-approval, review open); refreshed and split 2026-10-16 against HEAD 6c75456 (6b-1; S1-S18; Q28-Q35 decided under the owner's pre-approval, review open); 05 s5.3; 07 s1.2; 08 s2, AC-3
adrs: []
---

# code-tools (6b-1): MCP find_symbols, refs, unmapped

## Why

An agent asks which item sits at a name or `PATH:LINE` and what binds it, what binds a node, where code has no spec. 6a-2 (`symbol-index.md`: `refs` from the code index) and 6a-3 (`symbol-reads.md`: `spec symbols`, `spec unmapped`) answer on the CLI; MCP has none (07 s1.2): agents run Bash or read files.

Delivered: three read tools, twins of `spec symbols`, `spec refs` and `spec unmapped`; the `read-spec` skill naming them; `TOOLS` 18, plugin 0.9.0. Slice 6 split (Q1): 6a, 6b, 6c if AC-10 fails; 6b again (Q28): 6b-1 (this) right after 6a-3, over shipped CLI reads; 6b-2 `verify-node.md` (`spec verify --node`, `check_binding`, `implement`, 0.10.0) after 5c. No ADR: ADR-0004/0005 (one door), ADR-0008 (genre), ADR-0046 (`architecture-code.md#sync-in-reads`: `refs`, `symbols`, `unmapped` and their MCP twins walk code) applied.

## Description and interactions

- **An agent**: before an edit `find_symbols {at}` or `{query}` for an item and its markers, `refs {id}` for what binds a node, `unmapped {path}` for where code has no spec. **The owner**: the same commands on the CLI.
- **Crates**: MCP `code.rs` (new: the `code_tools` router, argument types, descriptions and their asserts; one CLI library call each through `read.rs`' `call` and `Answer`, unchanged), `mirror.rs` (the three documents), `server.rs` (`INSTRUCTIONS`, the router joined); `plugin/` (`read-spec`, version, `PINS`). CLI: no behaviour; the entries are 6a-2's (`refs`, `RefsRequest`, `RefsOutcome`, `lib.rs`) and 6a-3's (`docs/canon/symbol-reads.md` "Crates and seams"), an export it lacks added in `lib.rs` only. MCP `[dependencies]` stay 7 (`build_graph.rs`); store, core, model unchanged.
- **Freshness**: each call refreshes the md index, then the code index, inside the CLI read (`docs/canon/symbol-index.md`, `refresh_code`); MCP keeps nothing between calls (`mcp-read.md` "No project, discovery, state").
- **Unchanged**: `spec check`, hook, gate, CI, `INDEX_FORMAT`, queue schema, HTTP (Q8; `daemon-keys`), UI, the other tools, verify. **Writes**: the data directory only (the refresh); never `spec.lock`, a queue row, an event or a file under the root.
- **Overridden lines**, fixed at shipping (SW-2): 07:26 (the shipped list), :30, :32, :33 (`find_symbols` `node_id`, `path:line`, "Bevy layer"; `refs` `node_id`, "tests, records"); `mcp-read.md` "Binary" (the tool sets served), "Tools" **Texts** (the count, the figures); `plugin-files.md` "Changing it" **Content** ("all fifteen named"), **Version** (0.8.0); the MCP README's opening and `server.rs` row; 6a-2's canon **Freshness** ("no MCP tool … refreshes code": these three do, through the CLI).

## Data

### Tools

WA-17, WA-20. **Every MCP tool's argument names are its CLI flag names, a positional REF `id`** (`get_node`, `refs`; 7b's `get_impact {id, symbol, since, depth}`, 2026-10-10); the read tools' annotations and `_meta` (`mcp-read.md` "Tools"); `outputSchema` the mirror of the `--json` document.

| Tool | Input (`?` nullable) | ≙ `spec … --json` |
|---|---|---|
| `find_symbols` | `query?`, `node?`, `file?`, `at?` (`PATH:LINE`), `limit?` | `symbols QUERY \| --node REF \| --file PATH \| --at PATH:LINE [--limit N]` (`docs/canon/symbol-reads.md` "spec symbols") |
| `refs` | `id` | `refs REF` (`docs/canon/spec-cli-refs.md`; command, order, cap: `code-bindings.md` "`spec refs`") |
| `unmapped` | `path?` | `unmapped [PATH]` (`docs/canon/symbol-reads.md` "spec unmapped") |

- **Input schemas**, flat (`mcp-read.md` "Tools", **Schemas**): strings, `limit` an integer as `search`'s, optionals `[T, "null"]`, `additionalProperties: false`, no root `anyOf`/`oneOf`/`allOf`, no `$ref`. `find_symbols`' four forms are four nullable keys; `query` passes as one positional (6a-3 joins positionals with a space), `at` raw to the CLI's one `PATH:LINE` parser (Q35): MCP never splits a qpath or `PATH:LINE`.
- **Form errors**, per surface as `get_task`'s (`task-package.md` "MCP"; SW refinement): not exactly one form -> error result ``spec: find_symbols takes exactly one of `query`, `node`, `file` and `at` ``; `limit` without `query` -> ``spec: find_symbols takes `limit` with `query` only``; no `structuredContent`. The CLI's (6a-3): usage, exit 2.
- **Output mirrors** (`mirror.rs`; S16), every key `required`, absent `null`, typed as 6a-3 pins them (Q35). `refs`: `spec-cli-refs.md` "JSON" (markers with `levels`, `levels_state`, `binding {kind, qpath, item_kind, line, ambiguity, category}`; `code`; `omitted {markers, mentions}`). `find_symbols`: `{form, query, node, path, line, reason, notes, symbols: [{target, qpath, name, discriminator, item_kind, path, start_line, end_line, signature, ambiguity, layer, tier, mentions, bindings: [{relation, line, column, token, id, rev, id_state, fix, rev_state, levels, levels_state, note, node}]}], total, omitted {symbols}, code}`. `unmapped`: `{path, rows: [{path, dir, units, mapped, percent, mentioned}], total (a row), unmapped: [{qpath, item_kind, path, line}], notes, omitted {rows, unmapped}, code}`. Enums, the CLI's closed ones: `form`, `target`, `tier`, `relation`, `id_state`, `rev_state`, `levels_state`, binding `kind`, `ambiguity`; free strings: `item_kind`, `layer`, `category`, `levels` items, node kinds (10a's role keys join free: `bevy-roles.md`).

### Results and exits

`content` = `spec <…> 2>&1`, `structuredContent` = the `--json` document (`mcp-read.md` "Tools", **Parity**); MCP never re-sorts or re-cuts.

| Exit | `find_symbols`, `refs`, `unmapped` |
|---|---|
| 0 | success, zero rows too |
| 1 | `find_symbols {node}`, `refs {id}`: REF names nothing -> `isError`, the text, the exit-1 document (`reason`; an outcome in 6a-3, never a `CliError`: Q35) |
| 2 | `isError`, the `CliError` line(s), no `structuredContent`: usage, discovery, config, no `[code]` (`code-bindings.md` "The `[code]` table"'s text), a walk gap (`spec: the code walk is incomplete: <gap>`; `code-bindings.md` "The walk"), PATH not a walked code file, a bad LINE |

### INSTRUCTIONS

WA-17, Q31 (S4). Today (4c-1's text, `server.rs`): 1 886 B, 1 748 less the tasks line, 2 043 with `probes`, against `TEXT_LIMIT` 2 048: 5 B left. Measured 2026-10-16 over `server.rs` as it stands:

- The reads become two lines, the second the code tools (`spec ` and two glosses dropped; the REF paragraph's flag map keeps `with ["links"]`):

```
Reads:
- get_tree(root?, depth?, kinds?, archive?) = tree; get_node(id, with?, archive?) = show; search(query, kinds?, limit?, archive?) = search; get_context_bundle(node_ids, budget?) = bundle; get_proposal(proposal_id) = review --brief.
- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; unmapped(path?) = unmapped.
```

- `Hints use CLI flags:` -> `Hints use flags:`; the probes paragraph (157 B with its blank line) -> `Probes build: review_proposal, probe_output, probe_sleep (owner's checks).` (76 B with it).
- Kept: the opening, the queue and tasks lines, REF, the cut, the resources, the four rules.
- **Result: 1 892 B, 1 755 less the tasks line, 1 968 with `probes`: 80 B left.** 6b-2 adds `; check_binding(node, symbol?) = verify --node` to the code line (46 B; `verify-node.md` "INSTRUCTIONS"): 34 B left, then 7a (about 33 B), and 7b (46 B) cuts about 45 B elsewhere (`impact.md` "INSTRUCTIONS").
- Re-pinned (S5): `mcp_tasks.rs` (`INSTRUCTIONS.len()`), `mcp_path.rs` and `mcp_decision.rs` (length and BLAKE3 less the tasks line, and of the whole text), `mcp_create.rs` (the paragraph's bytes, the probes total), `server.rs`' doc comment; `mcp-read.md` **Texts** at shipping.

**Descriptions**: each ≤ 2 048 B, ASCII, with the determinism sentence, its CLI twin and the `[code]` need; compile-time asserts tie each stated number to its CLI constant (6a-3's exported `limit` bounds 1/200/20, Q35; `OUTPUT_CAP_CHARS`); no `STACK_WORDS` word or kind (`mcp_genre`, `common/read.rs`).

### Plugin

WA-19. Bodies only: `read-spec` names `find_symbols`, `refs`, `unmapped` (after the bundle, before files: an item at a line, what binds a node, where code has no spec). Descriptions unchanged (1 170 of 1 200 B); the body ≤ 4 096 B (2 598 B now, front-matter included); all eighteen tools backticked across the texts (`plugin-files.md` **Content**); 0.9.0 (MINOR: newly named tools), a `PINS` line.

### Fixtures

Q33. No new fixture. Parity, forms and exits over scratch copies of 6a-2's `fixtures/code-index/` (twins, the tiers with a quoted `#2`, a RON anchor, an `unmapped` row of `-`: `percent` null); the single door over `fixtures/code-a/` with `fixtures/spec-lock/code-a.lock` as it stands (the three read no lock: S15); genre over `fixtures/spec-c`.

## Rules and edge cases

- WHEN a code tool and its `spec … --json` twin run on one state, its `content` and `structuredContent` SHALL equal the CLI's text and document (RD-3).
- WHEN a tool runs from two roots, another cwd, in reverse creation order or on CRLF copies, its result SHALL be byte-identical, holding no root, `HOME` or date.
- Single door (08 AC-3; S14): no code tool writes `spec.lock`, a queue row, an event or a file under the root, or exposes `index`, `lock accept`, `propose drift` (`report_drift` alone queues drift).
- Genre (`architecture.md#universal`): no stack, kind, fixture or Rust word in MCP sources and texts; qpaths, `item_kind` are data; `query_name` stays the one reader of `::` (`docs/canon/symbol-reads.md` "Crates and seams").

## Carry-overs

From 6a-3's review 1 (`symbol-reads.md`; none blocked it), rust-developer:

- X4 (owner pre-approved): one read snapshot per `symbols`, `unmapped` call: their `code_refs`, `lookup_symbols`/`file_symbols`, `code_symbols` read in one transaction (a store method returning refs and the needed symbols from one `with_worktree`); then drop canon `symbol-reads.md` "Known limits"' first line.
- CLI `unmapped.rs`: decide file or directory from `stored.files`, not `walked`, and refuse an unread file PATH (exit 2); then drop that canon's second line.
- Code `naming.rs` `file_place`: rewrite `\` only where `MAIN_SEPARATOR` is `\`, as `dir_text`; then drop `symbol-index.md` "Known limits"' `file_place` line.
- Store `symbols.rs`: `LOOKUP_SQL` typed `[&str; 3]` (`SymbolTier::ALL.len()`), so a tier is never dropped by the zip.
- CLI `symbols.rs`: find a bound item with `binary_search_by_key` on `ord`, not a linear `find`.
- CLI `lib.rs` `Exit` doc: exit 1 lists `refs` and `symbols` too.
- Re-point the plain `docs/features/symbol-reads.md` "Lookup" comments (code `naming.rs`, `lib.rs`; store `symbols.rs`, `lib.rs`) to `` `docs/canon/symbol-reads.md` "Lookup" `` in data form; then the shipped spec's "Lookup" pointer heading may go.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- `tools/list`, both eras: the eighteen; `TOOLS` 18 (S1), `CODE_TOOLS` 3 (`tests/common/read.rs`); the read annotations, `_meta`. M: a tool without a door call -> `mcp_door`'s coverage; a description over 2 048 B, non-ASCII or without the sentence -> `mcp_default`; a stated limit not the CLI's constant -> a compile-time assert.
- [ ] RD-2 -- forms. M: zero or two forms reaching the CLI, or `limit` with `node` -> `find_symbols_takes_exactly_one_form`; `at` split in MCP -> `find_symbols_bad_at_is_the_clis_error` (a bad LINE answers the CLI's exit-2 line).
- [ ] RD-3 -- parity per tool and exit over `fixtures/code-index/` (Q33). M: an MCP-side sort or cut, or exit 1 as success -> `code_tools_equal_the_cli`.
- [ ] RD-4 -- schemas. M: a nested or `oneOf` input -> `mcp_read`'s flat-schema test; a CLI key without its mirror -> the schema walker; `item_kind` an enum -> `code_mirrors_keep_kinds_free`; `percent` a string -> the walker over a `-` row (Q35).
- [ ] RD-5 -- `INSTRUCTIONS` (Q31). M: a tool unnamed -> `mcp_default`; over 2 048 B, `probes` included -> `server.rs`' asserts; the text changed, a pin kept -> `mcp_decision.rs`, `mcp_path.rs`, `mcp_tasks.rs` (1 892), `mcp_create.rs` (76 B, 1 968).
- [ ] RD-6 -- size: text + `structuredContent` ≤ `MAX_RESULT_CHARS` per tool. M: `unmapped` JSON uncut -> `mcp_size`.
- [ ] RD-7 -- read-only, the three code tools (S14). M: a write under the root or a queue row -> `mcp_door`.
- [ ] RD-8 -- graph. M: a store or `rusqlite` edge in MCP -> `build_graph.rs` (7); an HTTP route -> `fixtures/daemon-keys.json`'s test.
- [ ] RD-9 -- plugin. M: a tool unnamed -> `plugin_skills.rs`; the version unchanged -> `plugin_files.rs`; descriptions over 1 200 B -> `plugin_skills.rs`.

test-engineer:

- [ ] TE-1 -- `mcp_code.rs` over scratch `fixtures/code-index/` copies, both eras: forms, exits 0, 1, 2; determinism (two roots, reverse creation, CRLF); session (an edited `.rs` shows in the next `find_symbols`, no `spec index`); quiet (broken, non-UTF-8 `.rs`: stdout JSON-RPC only); genre (`spec-c`: an error result naming `[code]`); `mcp_index`; `mcp_size` maxima recorded.
- [ ] TE-2 -- single door: a temp git repo with `fixtures/code-a/` and `fixtures/spec-lock/code-a.lock` as it stands (S15); every code tool, valid and invalid; then `git status --porcelain=v1 -z --untracked-files=all`, `.git/index` mtime, `spec.lock` bytes, queue rows unchanged.
- [ ] TE-3 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground, scratch `HOME`, read-only): warm latency per tool against `mcp-read.md` "Latency"'s 1 s, `unmapped` totals; no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec; refreshed and split 2026-10-16 (Q28): `verify-node.md`; 08 s2 reordered; `symbol-index`, `symbol-reads` (Q35), `lock-levels`, `lock-rebase`, `anchor-states`, `bundle-bindings`, `impact`, `bevy-roles`, `bevy-dump`, `layer-c` pointers follow.
- [ ] SW-2 -- at shipping (Q32), bytes measured 2026-10-16: a Tier 2 `docs/canon/mcp-code.md` (the three tools, forms, exits, mirrors; about 3 KB of 12 288; 6b-2 adds `check_binding`), its line in `docs/index-canon.md` (7 978 of 10 240, + ≈ 150); `mcp-read.md` (12 255 of 12 288, **room first**: **Texts**' 368 B list of what `INSTRUCTIONS` names cut to a clause pointing at `server.rs`, ≈ 260 B freed; then "Binary" one clause naming `mcp-code.md`, **Texts** count-free with 6b-1's figures); the MCP README (10 235 of 10 240, **room first**: the opening's 685 B of tool lists to one pointer at `mcp-read.md` "Binary", the `server.rs` row's figures to `mcp-read.md` **Texts**; then a `code.rs` row); `plugin-files.md` (4 611: eighteen, 0.9.0, its `PINS` BLAKE3), `plugin.md` "Skills" `read-spec` (9 459); 07 s1.2 :26, :30-:33; 6a-2's canon **Freshness**; 08 s2; `CLAUDE.md` State (5 726).
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Doc room**: the MCP README has 5 B, `mcp-read.md` 33 B; adding before trimming turns `spec check` red (SW-2 orders it).
- **`INSTRUCTIONS`**: 80 B left, 34 after 6b-2; 7b still cuts about 45 B. The 76 B probes paragraph drops words of the owner's AC-16 checklist (the probe tools' descriptions keep them).
- **Latency**: each call refreshes the code index (6a-2's `noop_ms` ≤ 200) against MCP's 1 s warm (`mcp-read.md` "Latency"); TE-3 records it, a breach plans 6c.
- **Upstream JSON**: a 6a-3 key or type changed after this ships reopens the mirrors (Q35 pins them first); 10a's role keys join `find_symbols`' mirror, whichever ships second (`bevy-roles.md`).
- **Genre**: MCP texts hold no `STACK_WORDS` word (`common/read.rs`).

## Out of scope

`check_binding`, `spec verify --node`, the `implement` skill (6b-2 `verify-node.md`); HTTP routes, UI (Q8, Phase 4); `delta.symbols`, `get_impact` (7b, Q6); `with: ["bindings"]`, bindings in reads (7a); `--unbound` (Q4); fuzzy search, references, call graphs (LSP's); spec-side referrers in `refs` (`get_node` with links); writing markers or binding code (07's former `bind_code_symbol`); the Bevy layer (10a `bevy-roles.md`, 10b `bevy-dump.md`: `find_symbols`' mirror gains their keys, free strings, whichever ships second); code resources.

## Open

| Q | Answer (else: the other answer's cost) |
|---|---|
| 1 slice 6 | 6a, then 6b; 6c only if AC-10 fails (else: tools re-walk the tree per call) |
| 4 unbound | none; `unmapped {path}` lists a file's items (else: `unbound` overloaded) |
| 6 `delta.symbols` | 7b `impact` (else: a git reader here) |
| 7 coverage | 6a-3's containment; mentions apart (else: path classes break genre) |
| 8 HTTP | none now (else: keys before Phase 4 screens) |
| Names (2026-10-10) | CLI flag names, REF `id`, every tool; 7b amended (else: 07's `node_id`, `qpath`: two vocabularies) |
| 28 split (owner) | 6b-1 (this): `find_symbols`, `refs`, `unmapped`, `read-spec`, 0.9.0, right after 6a-3; 6b-2 `verify-node.md`: `--node`/`--symbol`, `check_binding`, `implement`, 0.10.0, after 5c (else: one L slice after 5c, the reads waiting on 5b-1's pilot gate, 12 RD in one review) |
| 31 `INSTRUCTIONS` | the two reads lines, no gloss on the code line, `Hints use flags:`, the 76 B probes paragraph: 80 B left, 34 after 6b-2 (else: 7 B left, 7a's and 7b's lines cut from elsewhere) |
| 32 canon home | a new Tier 2 `docs/canon/mcp-code.md`; `mcp-read.md` one clause in "Binary" once room is made; 6b-2's `--node` in `spec-verify-node.md` (else: `mcp-read.md` over its cap) |
| 33 fixtures | the reads over 6a-2's `fixtures/code-index/` (twins, tiers, RON, `-`); `code-a` for the door (else: no twin, tier or `-` reaches the mirror tests) |
| 35 6a-3's JSON | 6a-3 pins (shipped: `docs/canon/symbol-reads.md`), before these mirrors: `percent` a number (null for `-`), `total` a row object, `omitted` `{symbols}` / `{rows, unmapped}`, `target`'s values, `tier` null outside `query`; its exports and exit-1 outcome (`symbol-reads.md` Q35) (else: string-typed mirrors and a schema change inside 6b-1) |

Q1, Q4-Q8 (2026-10-10) and Q28-Q35 (2026-10-16) decided under the owner's pre-approval, review open; numbers shared with `verify-node.md`, where Q5, Q29, Q30, Q34 live (Q28, Q31-Q33 in both), and with `symbol-reads.md` (Q35). S-n: the 2026-10-16 refresh's stale points; WA-n: the 2026-10-10 analysis' working answers; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs**: 6a-2 `symbol-index` (`refs` from the index, its refresh and gap exit, `fixtures/code-index/`; its TE-7 at `TOOLS` 15); 6a-3 `symbol-reads` (shipped 2026-10-16: `symbols`, `unmapped`, its Q35 exports and types, `docs/canon/symbol-reads.md`); 4c-1's `INSTRUCTIONS` (shipped).
- **Order** (Q28, 08 s2): 6a-2 -> 6a-3 -> 6b-1 (this) -> 5b-1 -> 5b-2 -> 5c -> 6b-2 -> 6c -> 7a; nothing from 5b or 5c. **Independent**: 8a, 9a, 9b. **Later**: Phase 4 (HTTP, UI).

## Implementation

Not started. Split 2026-10-16 (Q28): RD-1..RD-4 kept (RD-1 at 18 and 3; RD-2 without verify's flags; RD-3 over `code-index`; RD-4 a `percent` case); old RD-8 -> RD-5 (`mcp_tasks.rs` added); old RD-9's `unmapped` half -> RD-6; old RD-10 -> RD-7, scoped (S14); old RD-11, RD-12 -> RD-8, RD-9 (0.9.0); old RD-5, RD-6, RD-7, RD-9's `--node` half, RD-10's task half, TE-3, TE-4 -> `verify-node.md`; TE-1, TE-2 without `check_binding`; old TE-5 -> TE-3. To record: modules, `INSTRUCTIONS` bytes, `mcp_size` maxima, TE-3's figures, deviations with reasons.
