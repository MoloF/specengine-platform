---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin]
ref: RM-0047; Phase 7 step 6 (08 s2)
adrs: [ADR-0056, ADR-0057]
---

# card-trace: suspect marks, clearing, verdicts, rollup (RM-0047)

## Why

A changed brief must reach the work done against it, and accepted work must confirm the brief, with nothing written into the file and nothing stopped. Every state here is a join at read; the only writes are a person's, plus one an editorial apply needs.

## Summary

What became true is in `docs/canon/spec-trace.md`; the page in `board-ui.md`, the record in `cards.md`, `spec impact`'s cards in `spec-impact.md`. Code comments cite the headings below, so each stays with its pointer.

### Snapshot and suspect

`snapshot`, dropped entries, the editorial `also`: `spec-trace.md` "Snapshot", "Suspect".

### Rollup

Per read, never stored, first match: `spec-trace.md` "Marks and rollup".

### Accepting

`--briefed` or `--departs`, its discrepancy in the accept's transaction, never refused by a hit: `spec-trace.md` "Accepting".

### Clearing

No effect, needs correction, a scoped group, on a terminal or the page: `spec-trace.md` "Clearing".

### Re-estimating

A held estimate changes with a reason: `spec-trace.md` "Re-estimating".

### Data

`CardFrozen`, `CardAcceptance`, the trace document, events: `spec-trace.md` "Snapshot", "The read", "Events"; key sets: `board-ui.md` "Types".

## Acceptance criteria

- [x] AC-01 — On `fixtures/card-press`, approving a meaning change makes a card suspect; whitespace does not; a clear with a reason removes it, the next meaning change restores it. M: comparing `span_hash` turns the whitespace case red.
- [x] AC-02 — `spec approve --editorial`: not suspect, one `card.refreshed`, also from a worktree without the commit. A hand edit: suspect. M: the apply refreshing nothing.
- [x] AC-03 — `SHT-FRONT` is `confirmed` only with a card, all accepted or cancelled, one accepted, none suspect or departed; all cancelled reads `no card`; the file and `git status` stay as they were. M: confirming with no card.
- [x] AC-04 — `--departs`: one `open` discrepancy, the card accepted, the node `departed`; `spec reject` of it gives `confirmed`. A failure injected after the insert leaves neither. M: the insert in its own transaction.
- [x] AC-05 — The linked decision is a hit: the departing accept exits 0 with no flag, the item's `distinct_from` names it, a note lists it. M: the dedup's refusal kept.
- [x] AC-06 — Needs correction: the original stays accepted, the node `in work`; the correction accepted gives `confirmed`; cancelled, `suspect`. M: re-freezing at the clear.
- [x] AC-07 — `clear --group PRN-WEEKLY`: one event per card, no card outside it, a mark outside the group stays. M: a bulk clear ignoring `parent:`; a group clear qualifying on any target.
- [x] AC-08 — `SHT-FRONT-HEAD` changes: its card suspect, `SHT-BACK`'s not, listed under `uses`. M: marking through `uses`.
- [x] AC-09 — `spec trace`, `get_trace`, `GET trace` agree, exit 0 on suspect and departed rows; `spec check`, `spec gate` unchanged; `spec impact` lists the cards; a re-estimate without a reason exits 1. Piped `clear` exits 2; `--briefed` runs at a root with no `.git`.
- [x] AC-10 — From a same-origin page a clear of each outcome and a group clear finish, the typed name in the event; no `Sec-Fetch-Site`: 403; `tools/list` holds no clear.

## Verification

Delta review accepted. Mutations M1 to M10 killed: M1 `span_hash` 1 red, M2 the apply refreshing nothing 5, M3 confirming with no card 1, M4 insert in its own transaction 2, M5 the dedup's refusal kept 4, M6 re-freezing at the clear 4, M7 bulk clear ignoring `parent:` 2, M8 marking through `uses` 1, M9 group clear qualifying on any target 2, M10 `retargeted` dropping an entry 3. `cargo nextest run --workspace` 4339 run, 4339 passed, 71 skipped; UI lint 0, vitest 3018 passed, build 0; fmt, clippy, probes 0; parity 3 of 3; 86 of 86 doc tests.

## Out of scope

`CR-` itself. Flow badges, timeline, a node's cards, the group clear's screen, a card on a gone target on the page, the list's `marks`: RM-0048; the tile's badge came here. Marks through `uses`, or past direct targets. The warning on a move to `ready` is dropped: no column word exists in code.

## Implementation

| Module | What it does |
|---|---|
| Model `card.rs` | `Card.snapshot` typed `Vec<CardFrozen>`; `CardAcceptance` gains `verdict`, `departs`, `discrepancy` |
| Core `trace.rs` | The pure judgement and the snapshot's steps; nothing computed is stored |
| Store | `accept_card`, `accept_departing`, `clear_card`, `clear_cards`, `applied_vouching`, `node_norms`; `CardChange::Edit` boxed, no accept in `CardChange` |
| CLI | `spec trace`, `card clear`, `accept --briefed\|--departs`, `edit --reason`, `spec impact`'s `cards` |
| Daemon, MCP | `GET trace`; the two clears behind the card door; `get_trace`, read-only |
| UI | One trace read per project, badges on panel and tile, the Clear dialog, Accept's verdict |

### Files in scope

New: core and CLI `src/trace.rs`; tests `card_trace.rs`, `queue_trace.rs`, `mcp_trace.rs`, http `trace.rs`. Changed: model `card.rs`; core `intake.rs`, `proposal.rs`; store `queue.rs`, `queue/cards.rs`, `read.rs`; CLI `card.rs`, `intake.rs`, `impact.rs`, `apply.rs`, `revisions.rs`; MCP `read.rs`, `cards.rs`, `mirror.rs`, `server.rs`; http `app.rs`, `answer.rs`; UI `src/board/`, `src/api/`. Plugin: `plugin.json` 0.23.0 -> 0.24.0, MINOR, and `read-spec`.
