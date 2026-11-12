---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-eval, plugin, scripts]
ref: RM-0060, v5 core slice 9 (08 s2 Phase 7, "Owner's word, 2026-11-12")
adrs: []
---

# collision-pass: pairs that may collide, an agent's verdicts, a ledger (RM-0060)

## Why

Collisions already in the corpus were caught by nobody. An agent now lists candidate pairs, reports a real one as `contradicts` and marks the rest; a ledger keyed by both `norm_hash` values relists a pair once a side changes.

## Acceptance criteria

- [x] CP1 — A shared `uses` or `parent:` target pairs, score 200; `mentions` does not.
- [x] CP2 — Alike titles pair at `titles` >= 500, across kinds only with `--any-kind`.
- [x] CP3 — `distinct` hides a pair; a one-word edit relists it `marked`; whitespace does not.
- [x] CP4 — 1 000 children: no parent pair; byte-equal at any root; `--limit` 0, 201 exit 2.
- [x] CP5 — A refused mark writes nothing; a repeat replaces.
- [x] CP6 — Schema 14: no ledger in a dump, a restore empties it, older builds refuse.
- [x] CP7 — The ledger's names in nine files only (`collision_ban.rs`).
- [x] CP8 — 32 stdio tools at CLI parity, `INSTRUCTIONS` 2 039 B; the hub's 18 kept.
- [x] CP9 — Plugin 0.44.0: eighteen files, descriptions 1 197 B.
- [x] CP10 — [measure] 100k median 274 ms; twins 25/25 at 1k, 10k, 0/25 at 100k.
- [x] CP11 — The crates' runs, the full run after iteration 3, `spec check`.

16 mutations, each red.

## Implementation

Canon: `docs/canon/collision-pass.md`.

| Module | What it does |
|---|---|
| core `collisions.rs` | pairs, scores, mark caps |
| store `read.rs`, `index.rs`, `lib.rs`, `queue{,/collisions,/state}.rs` | `collision_input`, `STEP_14`, ledger, `clear_ledger` |
| CLI `collisions.rs`, `main.rs`, `lib.rs`, `state_file.rs` | both commands; format 7 at 13-14 |
| MCP `collisions.rs`, `server.rs`, `mirror.rs`, `lib.rs`, `hub.rs` | two tools, `INSTRUCTIONS`, `LEFT_OUT` |
| eval `src/scale/`; plugin | `collision_planted`; 0.44.0, `check-collisions` |

**Departures:** inherent `SqliteQueue` ops, `proposal_exists`; the cap cuts JSON too, into `left`; notes on stderr; `--kind` with `--any-kind` one group; `1.000`; `target(s)`.

**R1**, the orchestrator's ruling (the plan's capped sets failed at 10k, 100k): `titles` over full trigram sets; candidates from trigrams at most 200 titles hold and a words bucket (words sorted byte-wise, deduplicated, one space; 2-200 titles); skip a title-only pair when floor(2000 × min(|A|,|B|) / (|A|+|B|)) < 500; a bounded heap.

**Also:** skill step 3 uses the report's `id`, a queue hit's, else resends with `distinct_from`; a restore empties the project's ledger; the description reads `alike; report` (YAML).

**CP10:** 100k recall 0: 1 296 synthetic titles give 44k identical pairs first.

**Open:** bare IDs as names (`collision-names.md`); one shared target floods a real corpus (8 502 pairs), the owner's call; Q-A to Q-C stand as working answers.
