---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0067; Phase 7 step 7b (08 s2)
---

# proposal-rehearsal: a dry run, no false `ref-dangling`, one sitting of approvals (RM-0067)

## Why

In the trial of 2026-10-31 the queue was the only validator: 6 of 29 proposals were rejected probes, a bad create held its ID, a reference to an open proposal read as an error, 23 approvals took 23 commands. An agent now rehearses with nothing stored, and the owner walks the open proposals in one command, one key each.

## Summary

The truth: `proposal-kinds.md` "Propose", "MCP and plugin", "Known limits"; `proposal-apply.md` "Consent", "Known limits". No ADR: findings never refuse (ADR-0012), one key per item keeps ADR-0038.

### Schema

No queue step; `INDEX_FORMAT` stays. `propose_change` gains `dry_run`: 0.27.0, `plugin-files.md` "Changing it".

### Rules

- **D9.** `dry_run`: `update`, `create` only.
- **D10.** The filter: any live create or amendment, not only the agent's own.
- **The owner's answer.** One `[y/N]` per proposal; one consent over a set takes a new ADR.
- **The code moved past the first text**: five filter callers, not two; a place failure moves on; a reject staged is never asked.

## Acceptance criteria

- [x] AC-01 — A dry run of a new file gives the real `diagnostics`; no row, no event, `git status` empty; the real call then stores. M: the row stored; validation skipped.
- [x] AC-02 — A taken ID, a stale base, `class: generated`: the real reason, exit 1. M: returning before step 4.
- [x] AC-03 — `dry_run` with `interpretation` or `amendment`: an error naming it, no row. M: a row stored.
- [x] AC-04 — B `follows: [A]`, `parent: A` or an update adding the link: no `ref-dangling`, a note names A's PR; a reference to nothing keeps it; A rejected, B applied: `spec check` reports it. M: filtering on the code alone.
- [x] AC-05 — Three creates: three questions, three commits; `n` on the second leaves it in `left`, the third is asked, exit 1. M: one answer reused; stopping at the decline.
- [x] AC-06 — A piped stdin: exit 2, no event, no commit; `--open` beside a `PR` or `--editorial`: exit 2. M: the terminal check after the list read.
- [x] AC-07 — A stale item between two good ones: refusal printed, both applied, exit 1; a question and a drift item named, not asked. M: stopping at a refusal; walking `Decides`.

## Out of scope

One consent over a set. A multi-file proposal. Revisable drafts (RM-0057). A `--yes`.

## Implementation

| Module | What it does |
|---|---|
| CLI `propose.rs`, `create.rs` | `propose_dry_run`, `propose_create_dry_run`, `unheld_findings` |
| `amend.rs`, `interpret.rs`, `intake.rs` | The filter's other callers |
| `proposals.rs` | `rehearsed`, the note and tail |
| `apply.rs`, `inbox.rs`, `round.rs`, `lib.rs`, `main.rs` | `approve_open`, `OpenWalk`, `repository_proposals`, the flags |
| MCP `intake.rs` | `ChangeArgs.dry_run`, the form error |
| Plugin | 0.27.0, `PINS`, `SCHEMA_PINS` |
