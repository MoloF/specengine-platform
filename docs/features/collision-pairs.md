---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0088, its pair half, v5 core slice 10 (08 s2 Phase 7, "Owner's word, 2026-11-12")
adrs: []
---

# collision-pairs: a two-sided collision shows as two documents side by side under their titles (RM-0088, the pair half)

## Why

A `contradicts` discrepancy gave the owner a flat evidence list to sort by hand. Now a two-target one is served a `pair` per read, each side's ID, title, path and evidence; Inbox, Questions and Answer… show both under their titles. No ADR. Canon: `docs/canon/agent-intake-documents.md` "Pair".

## Acceptance criteria

The example: the canon's, on a scratch `fixtures/spec-a`, filed with `distinct_from: ["DEC-0023"]` (its 3- and 2-target variants too).

- [x] PP1 — `review --json`, `get_proposal`, `GET …/proposals/PR`, the round item: one `pair`.
- [x] PP2 — `null`: three targets, one, `missing`, a question, an update.
- [x] PP3 — A shared file under both sides; a gone side `title`, `path` `null`, `evidence` `[]`.
- [x] PP4 — A retitle shows on the next review; the queue's rows and last `seq` unchanged (not the `.db` bytes: a pair review refreshes the index in the shared `<slug>.db`); schema 14 kept.
- [x] PP5 — Four text lines after the evidence; other text byte-equal.
- [x] PP6 — The seam: 0 resolutions for a question, an update, one target; 2 for a pair.
- [x] PP7 — `Pair`, `PairSide` closed; tool counts kept; plugin 0.45.0 pinned.
- [x] PP8 — Fixture `Proposal` 54 keys, `RoundItem` 22, 145 sets; `AHEAD_OF_FIXTURE` empty.
- [x] PP9 — Mock: two sections, headings open the modal, own evidence, "Other evidence"; three targets flat.
- [x] PP10 — `PairView` lazy, 2.18 kB; first load 833.4 kB of 845.
- [x] PP11 — [look] Chrome, both themes and languages, 1440x900 and 1100x800 stacked; Tab, Enter, Esc.
- [x] PP12 — Gates: the full run green (5 970; two pins and a README line fixed, two timing failures green alone), eval, `spec check`, clippy; `daemonKeys` 445/445, UI 5 712/5 712, build.

## Out of scope

Three or more targets, other gap types; `pair` on `InboxEntry` or `CardItem`; a sheet line; a stored pair; a diff; tool or skill changes.

## Implementation

| Module | What it does |
|---|---|
| core `pair.rs` | the rule, the types |
| CLI `round.rs`, `review.rs`, `proposals.rs`, `subset.rs` | the shared resolver; `pair` and its text; the PP6 seam |
| MCP `mirror.rs`; plugin | closed shapes; 0.45.0 |
| UI `inbox/{PairView,PairSlot,EvidenceItem}.tsx`, three screens | the lazy view |

**Departures:** an unreadable index: `pair` `null`, note `pair unknown: <error>`, exit 0; the seam in `subset.rs`; only review and round fill `pair`, and `spec propose discrepancy --json`'s intake document has no `pair` key; with a gist, the pair sits in `AskedFold`.

**Tests:** PP5's byte-equality was checked against the `missing` twin, not HEAD's build.

**After review (UI):** slots escaped in `<bdi>`, the flat list too; one region; `pair.noEvidence`, "Other evidence" as positions resolve; a gone side's heading once.

**Open:** a titleless found target shows as gone; "No evidence cites this file" on a fileless side (PP11 nit); the owner's review.
