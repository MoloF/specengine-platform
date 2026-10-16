---
class: spec
status: shipped
scope: [crates/specengine-mcp, crates/specengine-cli, crates/specengine-store, crates/specengine-code, plugin]
ref: Phase 3 slice 6b-1, split from 6b 2026-10-16 (WA-16..WA-20; Q1, Q4-Q8 2026-10-10; Q28-Q35, C1, K1 2026-10-16; decided under the owner's pre-approval); 05 s5.3; 07 s1.2; 08 s2, AC-3; Rust 2 iterations, review accepted, tests 1 round; TE-3 the owner's
shipped: 2026-10-16
adrs: []
---

# code-tools (6b-1): MCP find_symbols, refs, unmapped

## Why

An agent asks which item sits at a name or `PATH:LINE` and what binds it, what binds a node, where code has no spec. 6a-2 (`symbol-index.md`: `refs` from the code index) and 6a-3 (`symbol-reads.md`: `spec symbols`, `spec unmapped`) answered on the CLI only (07 s1.2): agents ran Bash or read files.

Delivered: three read tools, twins of `spec symbols`, `spec refs` and `spec unmapped`; the `read-spec` skill naming them; `TOOLS` 18, plugin 0.9.0; 6a-3's carry-overs (one read snapshot per call; `CODE_INDEX_FORMAT` 2). 6b-2 `verify-node.md` (`check_binding`, 0.11.0) follows after 5c (Q28). No ADR: ADR-0004/0005 (one door), ADR-0008 (genre), ADR-0046 (`architecture-code.md#sync-in-reads`) applied.

## Description and interactions

Canon: `docs/canon/mcp-code.md` (all of it); `mcp-read.md` "Binary", **Texts**; `symbol-reads.md` "Crates and seams", "spec unmapped", "Known limits"; `symbol-index.md` opening, "Analyzer stamp", "Known limits"; `symbol-names.md` (the stamp's value); `plugin.md` "Skills", `plugin-files.md`; the MCP, CLI, store and code READMEs; 07 s1.2. Besides MCP and the plugin, the CLI, store and code crates changed for the carry-overs; core, model, HTTP, UI unchanged; MCP `[dependencies]` stay 7.

## Data

### Tools

`docs/canon/mcp-code.md` "Tools" (inputs, forms and their error texts, exits) and "Mirrors" (WA-17, WA-20, Q35, C1).

### INSTRUCTIONS

`docs/canon/mcp-read.md` **Texts** (Q31): 1 892 B, 1 754 less the tasks line (the draft's 1 755 was off by one), 1 968 with `probes`' 76 B paragraph: 80 B left. 6b-2's `; check_binding(node, symbol?) = verify --node` takes 46 (`verify-node.md` "INSTRUCTIONS"), 7a about 33; 7b cuts about 45 elsewhere (`impact.md`).

### Plugin

`docs/canon/plugin.md` "Skills" `read-spec`, `plugin-files.md` (WA-19): the code tools as step-2 bullets, before "Files last"; 0.9.0.

## Rules and edge cases

- WHEN a code tool and its `spec … --json` twin run on one state, its `content` and `structuredContent` SHALL equal the CLI's text and document (RD-3).
- WHEN a tool runs from two roots, another cwd, in reverse creation order or on CRLF copies, its result SHALL be byte-identical, holding no root, `HOME` or date.
- Single door (08 AC-3): no code tool writes `spec.lock`, a queue row, an event or a file under the root, or exposes `index`, `lock accept`, `propose drift`.
- Genre (`architecture.md#universal`): no stack, kind, fixture or Rust word in MCP sources and texts; `query_name` stays the one reader of `::`.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [x] RD-1 -- `tools/list`, both eras: the eighteen; `TOOLS` 18, `CODE_TOOLS` 3 (`tests/common/read.rs`); the read annotations, `_meta`. M: a tool without a door call -> `mcp_door`'s coverage; a description over 2 048 B, non-ASCII or without the sentence -> `mcp_default`; a stated limit not the CLI's constant -> a compile-time assert.
- [x] RD-2 -- forms. M: zero or two forms reaching the CLI, or `limit` with `node` -> `find_symbols_takes_exactly_one_form`; `at` split in MCP -> `find_symbols_bad_at_is_the_clis_error` (a bad LINE answers the CLI's exit-2 line).
- [x] RD-3 -- parity per tool and exit over `fixtures/code-index/` (Q33). M: an MCP-side sort or cut, or exit 1 as success -> `code_tools_equal_the_cli`.
- [x] RD-4 -- schemas. M: a nested or `oneOf` input -> `mcp_read`'s flat-schema test; a CLI key without its mirror -> the schema walker; `item_kind` an enum -> `code_mirrors_keep_kinds_free`; `percent` a string -> the walker over a `-` row (Q35).
- [x] RD-5 -- `INSTRUCTIONS` (Q31). M: a tool unnamed -> `mcp_default`; over 2 048 B, `probes` included -> `server.rs`' asserts; the text changed, a pin kept -> `mcp_decision.rs`, `mcp_path.rs` (1 754), `mcp_tasks.rs` (1 892), `mcp_create.rs` (76 B, 1 968).
- [x] RD-6 -- size: text + `structuredContent` <= `MAX_RESULT_CHARS` per tool. M: `unmapped` JSON uncut -> `mcp_size`.
- [x] RD-7 -- read-only, the three code tools. M: a write under the root or a queue row -> `mcp_door`.
- [x] RD-8 -- graph. M: a store or `rusqlite` edge in MCP -> `build_graph.rs` (7); an HTTP route -> `fixtures/daemon-keys.json`'s test.
- [x] RD-9 -- plugin. M: a tool unnamed -> `plugin_skills.rs`; the version unchanged -> `plugin_files.rs`; descriptions over 1 200 B -> `plugin_skills.rs`.

test-engineer:

- [x] TE-1 -- `mcp_code.rs` over scratch `fixtures/code-index/` copies, both eras: forms, exits 0, 1, 2; determinism (two roots, reverse creation, CRLF); session (an edited `.rs` shows in the next `find_symbols`, no `spec index`); quiet (broken, non-UTF-8 `.rs`: stdout JSON-RPC only); genre (`spec-c`: an error result naming `[code]`); `mcp_index`; `mcp_size` maxima recorded.
- [x] TE-2 -- single door: a temp git repo with `fixtures/code-a/` and `fixtures/spec-lock/code-a.lock`; every code tool, valid and invalid; then `git status --porcelain=v1 -z --untracked-files=all`, `.git/index` mtime, `spec.lock` bytes, queue rows unchanged.
- [ ] TE-3 -- pilots, the owner's manual run (`mcp_code_pilot.rs`, `#[ignore]`, release, scratch `HOME`, read-only): warm latency per tool against `mcp-read.md` "Latency"'s 1 s, `unmapped` totals; no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec; refreshed and split 2026-10-16 (Q28): `verify-node.md`; 08 s2 reordered; the neighbours' pointers follow.
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/mcp-code.md`, its `docs/index-canon.md` line; `mcp-read.md` (room first: **Texts**' list to a pointer at `server.rs`, "Binary" count-free naming `mcp-code.md`); the MCP README (room first: the opening's tool lists to a pointer, a `code.rs` row); `symbol-reads.md`, `symbol-index.md` (stamp 2, **Freshness**: the MCP twins refresh), `symbol-names.md`; the CLI, store, code READMEs; `plugin-files.md`, `plugin.md`; 07 s1.2; 08 s2; `CLAUDE.md` State; review nits to `lock-levels.md` "Carry-overs".
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **`INSTRUCTIONS`**: 80 B left, 34 after 6b-2; 7b still cuts about 45 B.
- **Latency**: each call refreshes the code index against MCP's 1 s warm; TE-3 records it, a breach plans 6c. The first call after the stamp 2 upgrade is a cold refresh.
- **Hand-written enums** (deviation 5): a new code-crate variant (`Relation::ALL`) passes the CLI but breaks MCP's `outputSchema` until mirrored; the parity walker sees only the values the fixtures produce.
- **Upstream JSON**: a 6a-3 key or type change reopens the mirrors; 10a-2's role keys join `find_symbols`' mirror as free strings, plugin 0.10.0 (`bevy-roles-reads.md`).

## Out of scope

`check_binding`, `spec verify --node`, the `implement` skill (6b-2); HTTP routes, UI (Q8); `delta.symbols`, `get_impact` (7b, Q6); bindings in reads (7a); `--unbound` (Q4); fuzzy search, references, call graphs; spec-side referrers in `refs` (`get_node` with links); writing markers; the Bevy layer (10a, 10b); code resources.

## Open

Decided under the owner's pre-approval: Q1 slice 6 as 6a, 6b, 6c only if AC-10 fails; Q4 no `--unbound`; Q6 `delta.symbols` in 7b; Q7 6a-3's containment; Q8 no HTTP now; names (2026-10-10) the CLI flags, REF `id`; Q28 the split; Q31 the two reads lines, `Hints use flags:`, the 76 B probes paragraph; Q32 canon home `mcp-code.md`; Q33 `fixtures/code-index/`, `code-a` for the door; Q35 6a-3's JSON pinned first. C1: `limit` `["integer","null"]` int64; 0, 201 and negatives answer the CLI's `--limit N: the limit is 1 to 200` line. K1: `CODE_INDEX_FORMAT` 2, every file re-derived once, the stale-row known limits gone. X4: one snapshot. Q-numbers shared with `verify-node.md` (Q5, Q29, Q30, Q34 there) and `symbol-reads.md` (Q35).

## Implementation

Tests, 1 round: 2 339/2 339 over MCP, CLI, store, code, eval and HTTP, `probes` 79/79; 20 mutations killed (M1-M13 and variants; M10-M12 guarded only by the new `code_tools_reads.rs`, `file_place.rs`); the PHASE-B hashes and K1's history line pinned. TE-3 the owner's.

| Module | What it does |
|---|---|
| MCP `code.rs` (new) | `code_tools`: `SymbolsArgs`, `RefsArgs`, `UnmappedArgs`; `symbols_form` (the form checks before discovery); descriptions, asserts |
| MCP `mirror.rs` | `SymbolsDocument`, `RefsDocument`, `UnmappedDocument`, their rows, the closed enums |
| MCP `server.rs`, `lib.rs` | Q31 `INSTRUCTIONS`, the probes paragraph, `code_tools` joined; `mod code` |
| CLI `symbols.rs`, `unmapped.rs`, `lib.rs` | one snapshot per call; `--node` by binary search on (path, ord); `symbols_limit_error`; file or directory from the stored files; `Exit` docs |
| store `code.rs`, `symbols.rs`, `lib.rs` | `code_snapshot(SnapshotSymbols) -> CodeSnapshot`; `lookup_in`, `file_symbols_in` in the caller's transaction; `LOOKUP_SQL` typed |
| code `naming.rs`, `lib.rs` | `file_place` via `package_label` (`MAIN_SEPARATOR`); `CODE_INDEX_FORMAT` 2 |
| `plugin/` | `read-spec` step-2 bullets; 0.9.0 |

Figures: descriptions 1 755 / 1 177 / 1 144 B; `mcp_size` maxima 129 479 / 135 868 / 103 777. K1's history line: `2 206f941120e31b317e822f026607d6bbccd5efdf77a30f338446578d7fdc275a`. Re-pinned beyond S5's list (D2): `mcp_create` (1 886), `mcp_stdio` (21 tools), `plugin_roles`, `plugin_skills` (0.9.0), `mcp_session` `valid_args`, `mcp_door` `calls_of`.

Accepted deviations: 1, `symbols_limit_error` is a new CLI function, not an export only: the `--limit` text keeps one source (C1). 2, the code tools are step-2 bullets, not a step: "3. **Files last.**" stays. 4, `--node` reads only marker-bound symbols, keyed (path, ord). 5, the mirror enums are hand-written (Risks). 3 (no stamp bump) fell to K1.

The review's nits go to 5b-1: `lock-levels.md` "Carry-overs".
