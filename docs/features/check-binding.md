---
class: spec
status: shipped
scope: [crates/specengine-mcp, plugin]
ref: Phase 3 slice 6b-2b of RM-0005, after 6b-2a `verify-node.md`; split 2026-10-21 (Q-B); Q-C, Q-D, Q-H and the review's mend wording owner pre-approved, review open; WA-17..WA-19, Q31; 07 s1.2; 08 AC-3; developer 2 iterations, review accepted
shipped: 2026-10-21
adrs: []
---

# check-binding (6b-2b): MCP `check_binding`

## Why

Before `report_run`, an agent asks whether its targets' bindings still hold, without Bash. `check_binding`, the MCP twin of 6b-2a's `spec verify --node` (`docs/canon/spec-verify-scope.md` "`--node`"), answers it, and the `implement` skill names it. RM-0005 shipped with this slice. No ADR: 6b-2a's decisions applied; ADR-0004/0005 (nothing written), ADR-0012, ADR-0042 (nothing reads a result; a finding is never `ok`).

## Description and interactions

The tool, its exits, the server's root as the worktree, no base or task, no `task_id` form (Q-H): `docs/canon/mcp-code.md`. Parity, a rendered exit-2 report keeping its document: `docs/canon/mcp-read.md` "Tools". `read-spec` unchanged (Q-D); CLI, HTTP, UI, gate untouched.

## Data

The input, the exit table, the mirror (`VerifyJson` key for key; `base`, `task`, `delta.symbols`, `delta.tests_reaching` `{"type": "null"}` with a description) and the description's rules and mend wording: `docs/canon/mcp-code.md` "Tools", "Mirrors", "Texts".

### INSTRUCTIONS

The code line gains `; check_binding(node, symbol?) = verify --node` (46 B), the `probes` paragraph drops ` (owner's checks)` (59 B): 1 978 B, 1 840 less the tasks line, 2 037 with `probes`, 11 B left (`docs/canon/mcp-read.md` **Texts**). BLAKE3 `d4194b2b…17bf`, less the tasks line `dc019aa4…d547`.

### Plugin

0.12.0: `implement` step 5 checks the bindings per target before the report (`docs/canon/plugin.md` "Skills"); `PINS`, `SCHEMA_PINS` (`docs/canon/plugin-files.md` "Changing it", `mcp-code.md` "Tests").

## Acceptance criteria

- [x] RD-7 -- parity per exit, `mcp_code.rs` `check_binding_parity_per_exit`, both eras, a git copy of `fixtures/code-a` with its golden lock: exit 0 (`LAW-STEP` with its bound qpath; zero rows); exit 1 under `enforce`; exit 2 with the document (plain `LAW-STEP`: a `cannot_verify ambiguous_symbol duplicate` row on `data/limits.ron` `root.step`; a walk gap; a format-1 lock); exit 2 without (a REF naming nothing, a look-alike, no project). M8 (a rendered exit-2 report answered as `CannotRun`) killed: this test and five others.
- [x] Pins: listed as a read (`check_binding_is_listed_as_a_read`, `TWINS`); flat input (`mcp_read.rs`); `INSTRUCTIONS` in five files; `TOOLS` 20, `CODE_TOOLS` 4, `PROBES_BUILD_TOOLS` 23; 0.12.0, `PINS`, `SCHEMA_PINS`; `implement` names the tool (`check_binding_implement_names_the_tool`); `read-spec`'s three reads; `build_graph.rs` 7.
- [x] TE-2 -- MCP: `check_binding_mirror_is_verify_json`, `check_binding_documents_meet_every_key` (code-a and `fixtures/anchor-states`: every state, nullable key both ways, anchor state, `delta` list); `check_binding_is_byte_identical_across_roots_order_and_cwd` (no CRLF leg); `check_binding_leaves_stdout_json_rpc_only`; genre; `check_binding_on_spec_c_is_the_code_table_report`; `mcp_size` over a git-committed corpus.
- [x] TE-3 (MCP half) -- `te3_check_binding_writes_nothing_under_the_root` and `mcp_door.rs`: git status, the `.git/index` mtime, `spec.lock`, the tree, the queue unchanged.
- [ ] TE-5 -- the owner's pilot run: `mcp_code_pilot.rs` times `check_binding {node}` (`#[ignore]`, against 1 s; 08 s2).
- [x] SW-1..SW-3 -- `mcp-code.md` 5 970 -> 8 000; `mcp-read.md` 12 172 -> 12 274; `spec-verify.md` 12 212 -> 12 246; `spec-verify-scope.md` 5 996 -> 6 083; `plugin-files.md` 5 125 -> 5 147; `plugin.md` 9 623 -> 9 753; root `README.md`; `CLAUDE.md` 6 344 -> 6 361; 07, 08, RM-0005; `impact.md`, `bundle-bindings.md`, `tested-evidence.md`, `layer-c-reads.md`.

## Out of scope

`spec verify --node` itself (6b-2a), a `task_id` form, `read-spec`'s text, HTTP, the UI; `tests_reaching` (7a), `delta.symbols` (7b), `tested` (8b): each moves the mirror, a plugin MINOR and a `SCHEMA_PINS` entry.

## Implementation

| Module | What it does |
|---|---|
| MCP `src/code.rs` | `CheckBindingArgs {node, symbol?}` (`deny_unknown_fields`), `CHECK_BINDING_DESCRIPTION` (2 040 B, asserted phrases), the tool: one `specengine_cli::verify` call (`VerifiedTree::WorkingTree`, no task, `process_git`) through `call`, `Answer`; the module doc |
| MCP `src/mirror.rs` | `VerifyDocument` and its row, levels, anchor, place, key, delta, counts types; closed enums; `AlwaysNull` with descriptions |
| MCP `src/server.rs` | the code line, the 59 B probes paragraph, their doc comments |
| MCP `src/read.rs`, `src/lib.rs` | comments only: a rendered exit 2 keeps its document; the crate doc |
| `plugin/` | `plugin.json` 0.12.0; `implement` step 5 (696 B), steps renumbered |
| tests | `mcp_code.rs` (eight new tests), `common/read.rs`, `mcp_door`, `mcp_read`, `mcp_session`, `mcp_size`, `mcp_stdio`, `mcp_create`, `mcp_decision`, `mcp_path`, `mcp_tasks`, `mcp_code_pilot`, `plugin_skills`, `plugin_files` (`as_of_0_11_0`), `plugin_roles` |

Tests: nextest over 8 crates 3 184 passed, the `probes` build 82; clippy both builds and fmt clean. Size: 140 536 characters at most (3 000 rows: text 40 172, `structuredContent` 100 364); one qpath 1 262.

Divergences, kept (owner pre-approved, review open): the review of iteration 1 added a marker naming a revision its node has not reached to what the agent mends and closed the owner's list (description, skill); the description trimmed two phrases to fit (2 040 B, 8 left).
