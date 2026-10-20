---
class: spec
status: draft
scope: [crates/specengine-mcp, plugin]
ref: Phase 3 slice 6b-2b of RM-0005, after 6b-2a; split 2026-10-21 (Q-B; Q-C, Q-D, Q-H owner pre-approved, review open); WA-17..WA-19, Q31; 07 s1.2; 08 AC-3
adrs: []
---

# check-binding (6b-2b): MCP `check_binding`

## Why

Before `report_run`, an agent asks whether its targets' bindings still hold, without Bash. `check_binding`, the MCP twin of 6b-2a's `spec verify --node` (`docs/canon/spec-verify-scope.md` "`--node`"), answers it, and the `implement` skill names it. RM-0005 ships with this slice. No ADR: 6b-2a's decisions, ADR-0004/0005 (nothing written), ADR-0012 (no gate reads a result).

## Description and interactions

- **MCP** (6b-2a shipped first; no CLI file changes): `code.rs` (6b-1's) gains `check_binding`, one library call (`verify`, `node` and `symbol` set, tree `WorkingTree`, no `task`) through `read.rs`' `call` and `Answer`, with the read annotations and `_meta`; `mirror.rs` verify's document; `server.rs` `INSTRUCTIONS`, `PROBE_INSTRUCTIONS`. `read.rs`: no code change (a rendered exit 2 keeps its document, `Answer::into_tool_result`); its module doc ("exit 2 … the error line(s) alone") corrected. `[dependencies]` stay 7; `TOOLS` 20, `CODE_TOOLS` 4.
- **Plugin**: `implement` names the tool; 0.12.0 (0.13.0 if 11a-3 `layer-c-reads.md` ships first).
- **Q-D**: `read-spec` unchanged; its `plugin_skills.rs` rule iterates its own list of the three reads, not `CODE_TOOLS`. **Q-H**: no `{task_id}` form; one call per target node.
- **Unchanged**: CLI, HTTP (no route), UI, gate, `spec check`.

## Data

### check_binding

Input `{"node": string, "symbol": ["string", "null"]}`, `required: ["node"]`, `additionalProperties: false`, flat (`mcp-read.md` **Schemas**); ≙ `spec verify --node REF [--symbol QPATH] --json`, `node` as written (MCP never splits, resolves or matches a qpath). A schema refusal: rmcp `Parameters`' error result.

| Exit | Result |
|---|---|
| 0 | `clean`, `observed`: success, text and document |
| 1 | `blocked` (`enforce*`, a Fix row): `isError`, text and document; a finding, not a tool failure |
| 2, a report | a Cannot row or run cause (no `[code]`, a walk gap, a format-1 lock): `isError`, text and document |
| 2, none | discovery, a REF naming nothing or refused, `HOME`, the data directory: `isError`, the `CliError` line(s) + `\n` |

**Mirror** (`mirror.rs`; every key `required`, absent `null`, key for key `VerifyJson`):

```
top (13): mode enum(observe, enforce-introduced, enforce)|null; base, task AlwaysNull; node [string];
  symbol string|null; verdict enum(clean, observed, blocked, cannot-verify);
  counts {rows, ok, fix, check, cannot, pre_existing, skipped}; cascade {by_blob, by_ast}; rows [row];
  delta {symbols AlwaysNull, bindings {moved [{from: key, to: key}], ambiguous, lost, stale: [key]},
         affected_nodes [string], tests_reaching AlwaysNull}; cannot, notes [string]; omitted {rows}
key: {node, relation, file, kind, symbol, item_kind}      place: {file, kind, symbol, item_kind}
row (17): node, file string; relation enum(implements, verifies, configures, assumes);
  kind enum(item, module, ron)|null (keyless); symbol, item_kind, category, detail string|null;
  state enum(cannot_verify, predated, bad_marker, broken_binding, unlocked, spec_ahead, code_ahead,
  conflict, ok); class enum(fix, check, cannot)|null; pre_existing bool; markers [{line, rev|null}];
  node_rev int|null; lock {rev, commit, verified_at}|null; compared enum(blob, ast)|null;
  levels {lock [string], markers [string], changed [string]|null}|null;
  anchor {state enum(exact, moved, ambiguous, lost), from place|null, candidates [place]|null}|null
```

Free strings: `item_kind`, `category`, `detail`, `levels` items (no node kind). Enums hand-written (`mcp-code.md` "Mirrors"): a new CLI word breaks `outputSchema` until mirrored.

**Description** (≤ 2 048 B, ASCII): "Deterministic: one state, one result; no LLM inside."; the CLI twin; the `[code]` need; this session's worktree; nothing written but the index outside the root; a report kept at exit 2; under the enforcing modes a Fix row an error result, a finding, not a failure; no base: `enforce-introduced` judged as `enforce`; the 40 000-character cut, narrowed by `symbol` (asserted against `OUTPUT_CAP_CHARS`). Genre (`mcp_code.rs` `genre_code_sources_and_texts_hold_no_kind_or_rust_word`): no `_item`, `::`, item kind, stack, language or fixture word or code file suffix in the texts or the `code.rs`, `mirror.rs` literals.

### INSTRUCTIONS

Q31, Q-C. The code line becomes (151 B with its LF):

```
- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; unmapped(path?) = unmapped; check_binding(node, symbol?) = verify --node.
```

`PROBE_INSTRUCTIONS` becomes `"\n\nProbes build: review_proposal, probe_output, probe_sleep."` (59 B, -17): no rule or agent-facing word lost. Bytes (2 048 cap): default 1 932 -> 1 978 (70 left); less the tasks line 1 794 -> 1 840; `probes` 2 008 -> 2 037 (11 left). `server.rs`' doc comment and asserts follow.

### Plugin

`plugin.json` 0.12.0, a `PINS` line, `(0.12.0, <schemas' BLAKE3>)` in `SCHEMA_PINS` (`plugin-files.md` "Changing it"). `implement` (2 521 B: front-matter 241, body 2 280 of 4 096): its body and AC-11 list (`plugin_skills.rs` `NAMES`) gain `check_binding`; outside `json` blocks no digit (but a line-start `N. `) and no backticked enum value; the five model-invoked descriptions stay 1 170 B together; what the agent mends apart from what the owner settles, no state words. Recommended step 5 (later steps renumbered; wording free within these rules):

> 5. **Check the bindings.** Before the report, call `check_binding` with each target's `node`; it judges this session's worktree. Mend what your change broke: a binding that no longer reaches its code, spec and code that both changed, or one it cannot verify; else raise it with `report_discrepancy` and keep working on your `working_answer`. Code that moved ahead of its spec is the owner's to accept: name it in the report, never hide it. An error result here is a finding, not a failed call.

## Rules and edge cases

- WHEN `check_binding` and `spec verify --node REF --json` run on one state, `content` SHALL be the CLI's `2>&1` bytes and `structuredContent` its document, at every exit.
- WHEN it runs from two roots, another cwd or in reverse creation order, its result SHALL be byte-identical (CRLF copies not covered: `spec-verify-scope.md` "Known limits").
- Single door (08 AC-3): no `spec.lock`, queue row, event or file under the root written; no accept or drift operation. ADR-0012, ADR-0042: nothing reads a result.

## Acceptance criteria

- [ ] RD-7 -- `mcp_code.rs` `check_binding_parity_per_exit`, both eras, scratch `code-a` with the golden lock: exit 0; exit 1 under `enforce`; exit 2 with a report (a walk gap; a format-1 lock) keeps its document; exit 2 at discovery or on a REF naming nothing has none. M8: a rendered exit-2 report answered as `CannotRun`.
- Pins, no mutation counted: `TOOLS` 20, `CODE_TOOLS` 4, `mcp_door`'s `called == TOOLS`; the description's bytes and sentence (`mcp_default`); `INSTRUCTIONS` and its five pin files; 0.12.0, `PINS`, `SCHEMA_PINS`; `implement` names `check_binding`; `read-spec`'s three reads; `build_graph.rs` 7.
- [ ] TE-2 -- MCP: the mirror walker with exact key sets; determinism over two roots, another cwd, reverse creation; quiet; genre (`spec-c`: the `[code]` cause with its document); `mcp_size`'s maximum recorded; `mcp_code_pilot.rs` gains a call (`#[ignore]`, the owner's).
- [ ] TE-3 (MCP half) -- `mcp_door.rs` calls `check_binding` valid and invalid; `git status --porcelain=v1 -z --untracked-files=all`, the `.git/index` mtime, `spec.lock`'s bytes, the queue rows and events unchanged.

**Pin churn** (lines at ce6065b): `mcp_code.rs` the planned text :1528-1562, `CODE_LINE` :1569 105 -> 151, `PROBES_PARAGRAPH` :1578 76 -> 59, the BLAKE3s :1585/:1590/:1593, the figures :1607-1643 (rd5's "before" strips both clauses back to 1 892 and `CODE_TOOLS_BLAKE3`); `mcp_path.rs` :366-373, `mcp_decision.rs` :356-363 (1 840, 1 978, their BLAKE3s); `mcp_tasks.rs` :249; `mcp_create.rs` :397, :421 (the probes literal), :431 (2 037); `common/read.rs` `CODE_TOOLS`, `TOOLS`; `plugin_skills.rs` :134-147, :1361, :1455; `plugin_files.rs` `PINS`, `SCHEMA_PINS`, :1352-1357, :1619-1621.

**spec-writer**, at shipping (bytes measured 2026-10-21):
- [ ] SW-1 -- `mcp-code.md` 5 970 -> ≤ 7 900: the tool, exits, mirror, description bytes, tests, 0.12.0.
- [ ] SW-2 -- `mcp-read.md` 12 172 -> ≤ 12 288: **Parity** +≤ 60 (a rendered exit-2 report keeps its document); **Texts** byte-neutral (1 978, 1 840, 2 037, 59 B, 11 B left); **Size** +≤ 50 (an uncut `delta` joins the unbounded residue).
- [ ] SW-3 -- `spec-verify.md`'s opening, MCP runs only `--node` as `check_binding` (≤ 12 280); `spec-verify-scope.md` `## --node` one line; `plugin-files.md` ≤ 5 165 ("all twenty named", 0.12.0, its BLAKE3); `plugin.md` ≤ 9 743 (`implement`'s row); MCP README 0 B (10 210, 30 left); 07 s1.2 :26, :38; 08 s2 :41, :42; RM-0005 `shipped`; `CLAUDE.md` State; the `INSTRUCTIONS` figures in `impact.md` :91, :130 and `bundle-bindings.md` :169; `layer-c-reads.md` 0.13.0; the Risks' mirror note in the 7a, 7b and 8b specs; this spec ≤ 3 KB.

## Out of scope

`spec verify --node` (6b-2a), a `task_id` form (Q-H), `read-spec`'s text (Q-D), HTTP, the UI, and `tests_reaching` (7a), `delta.symbols` (7b), `tested` (8b).

## Risks

- **Future keys move the mirror**: 8b `tested` (`tested-evidence.md`), 7a `tests_reaching`, 7b `delta.symbols` each change `outputSchema`: a plugin MINOR and a `SCHEMA_PINS` entry each.
- **Agents misreading findings**: code ahead of its spec after implementing is expected, the owner's accept; the description and the skill say so, no enum words.
- **Wrong worktree**: MCP judges the server's root; the texts say "this session's worktree".
- **No base**: `enforce-introduced` as `enforce`, so a pre-existing Fix row comes back `isError` (a known limit).
- **Headroom** after this slice: 70 B (11 B with `probes`); 7a (about 33 B) and 7b (46 B) re-measure.

## Implementation

Not started. rust-developer: MCP `code.rs`, `mirror.rs`, `server.rs`, the `read.rs` comment; `implement`'s `SKILL.md`, `plugin.json`. test-engineer: the pin-churn files, `mcp_door.rs`, `mcp_size.rs`, `mcp_code_pilot.rs`.
