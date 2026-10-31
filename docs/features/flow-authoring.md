---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
ref: RM-0068; Phase 7 step 7c (08 s2)
---

# flow-authoring: a `write-flow` skill, gap rows that say how, errors that teach (RM-0068)

## Why

In the trial of 2026-10-31 a fresh AI learned the flow grammar from validator errors: 70 % of its calls were discovery. No source said how.

## Summary

The truth: `plugin.md` "Skills", `plugin-files.md` "Changing it", `spec-flow.md` "Gap rows", `spec-check-process.md` "Findings". No ADR: ADR-0027 admits a stack-neutral skill; nothing newly refuses (ADR-0012).

### The skill

Five steps, the grammar, one made-up example. Stop: each row measured and finding nothing.

### Skill text

Outside `json` fences a test reads digits: the `toml` fence holds none.

## Data

The example: the skill's own config and eight `create` blocks.

### `cause`

**D11.** Remedies stay in `cause`, why plus how; no key added.

### Budgets

Body 4 086 of 4 096 B, description 30 B, model-invoked total 1 200 B, `get_flow` description 2 045 of 2 048 B, `INSTRUCTIONS` unchanged.

### Formats

`INDEX_FORMAT` 15: parser messages are stored rows. Plugin 0.28.0; `FlowGap.cause`'s doc moved the schemas, so `SCHEMA_PINS` too.

### `spec init`

Two lines, then comments; paths from `DEFAULT_SPEC`.

## Acceptance criteria

- [x] AC-01 — The skill's config and `json` blocks, from its file, in an empty git project, approved with a scripted consent: ten rows `measured` 0, `spec check` 0 errors, 0 warnings. M: `repeat` dropped.
- [x] AC-02 — On `fixtures/flow-gaps` each row above 0 has a one-line `cause` in text and JSON, a row at 0 `null`; `FlowGaps` draws a stubbed one; every `not_configured` cause keeps its prefix. M: a row left blank; a cause at 0.
- [x] AC-03 — An unknown link type lists the 15 types; a missing `status` its four values; `[ids] probe = 1` names no Rust type; `no_selector` says what `when` holds; `id: TOPIC-one` under no such prefix shows a `TOPIC = ` line. M: a literal list instead of `LINK_TYPES`.
- [x] AC-04 — `spec init`: a rerun exits 2; the parsed config equals the bare one; the example with its `# ` removed parses; `genre.rs` passes. M: a `docs/` literal; an active line.
- [x] AC-05 — Seven skills, `plugin.json` 0.28.0, `INSTRUCTIONS` pins unchanged, every body within 4 096 B, descriptions at most 1 200 B. M: a 31-byte description.

## Out of scope

`--pack`: RM-0061. Writers' prompts: RM-0062. `spec check` on `[flow]`.

## Implementation

| Module | What it does |
|---|---|
| Core `flow.rs` | `GapCause::Found` above 0 |
| Core `front_matter.rs`, `check/engine.rs`, `scheme_toml.rs`, `health_toml.rs` | The five messages |
| CLI `flow.rs`, `init.rs` | `remedy`, `scaffold` |
| Store `lib.rs` | `INDEX_FORMAT` 15 |
| MCP `read.rs`, `mirror.rs` | The pointer, `cause`'s doc |
| Plugin | `write-flow`, 0.28.0 |
| UI mocks | `GAP_REMEDIES`, `no-gaps` |

Overruled in review: "no `SCHEMA_PINS` entry", "no UI change".
