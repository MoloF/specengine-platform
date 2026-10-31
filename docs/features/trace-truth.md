---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin]
ref: RM-0066; Phase 7 step 7a (08 s2)
---

# trace-truth: a nested edit marks, counts stop at containers, a card read shows its mark (RM-0066)

## Why

The trial of 2026-10-31 found three reads that say something untrue. ADR-0026 puts rules in `{#ID}` sections and flow steps are documents, so a card stands on a document and its rule in a section of it. `norm_hash` covers own text only, so the section changed, the card stayed accepted and the rollup said `confirmed`. `no card` counted containers, so a fully confirmed group read "N confirmed, 1 no card". `get_card` showed no mark, so an AI about to act on a suspect card read a clean one.

First of the closing group of the first result (08 s2 Phase 7): truth comes before the authoring work that leans on it.

## Summary

What became true is in `docs/canon/`. Code cites the headings below, so each stays with its pointer.

### Description and interactions

`spec-trace.md` "Suspect", "The read". `show C [--history] [--marks]`, the four keys right after `targets`: `cards.md` "Documents and events"; `get_card` always, `null` on plain `show` and `GET cards/:id`.

### Data

`spec-trace-record.md` "Snapshot"; `plugin-files.md` "Changing it", 0.26.0. No queue step; `INDEX_FORMAT` stays 14.

### Recipe

`spec-trace-record.md` "Within".

### Editorial refresh

`spec-trace-record.md` "Within". Under `--editorial` `card_refresh` adds, by `paired`, a `CardRefresh {node, kind, old, new}` for each ID node over the same members. `vouched(entry, kind, old, new)` takes the kind from a non-null `within_hash`: a document still sends two changes, an entry takes one. A completion: D5.

### `card.cleared`

`spec-trace-record.md` "Events".

### Rules and edge cases

A new `{#ID}` section marks, under `--editorial` too. `spec-trace.md` "Clearing", "Known limits" hold. No ADR: this fits ADR-0056.

- **D1.** `norm_hash` stays own-text: `specengine-norm/v2` would be a re-lock.
- **D2.** Pairs sort by name, then hash for a repeated ID.
- **D3.** Holders: `graph.children`, stopped at the first live document.
- **D4.** No migration; the limit: `spec-trace-record.md` "Known limits".
- **D5.** A completion vouches only what its trailers name: a `within_hash` pair also needs equal member names and, every named node taken out, equal (name, `norm_hash`) pairs in both blobs. Else the mark stays for a person: a false mark, never a false clean.
- **D6, D7.** Detail in `spec-trace-record.md`; the panel keeps marks from the list.
- **D8.** An index failure in `--marks` is the read's error.

## Acceptance criteria

Fixture: copies of `fixtures/trace-almanac`; a test adds what it lacks (a sibling section, a `uses` link) to its copy, since its `nodes 8` are pinned.

- [x] AC-01 — A card on `STEP-SET` accepted, text appended under `STEP-SET-PROOF`: `spec trace STEP-SET` reads `suspect`, the card `suspect STEP-SET`; `spec flow --trace` and `list_cards marks:true` agree; `spec check` and `spec gate` output is unchanged. M: `within_hash` over the target alone.
- [x] AC-02 — The same edit leaves clean a card on `PART-GATHER`, on a sibling section, and on a node that `uses` `STEP-SET`. M: walking `held_documents` into the hash.
- [x] AC-03 — Whitespace under the section, or the section moved in its file: no mark. A new `{#ID}` section: a mark. M: pairs in file order; pairs without names.
- [x] AC-04 — A stored entry with no `within_hash` key on a sectioned document reads `accepted` with exactly one note; a meaning change of its own text marks it; the clear of that mark, or an accept of an open card in the same state, writes the key. M: an absent key read as a change.
- [x] AC-05 — `spec approve --editorial` of the section: the document's card stays clean here and in a worktree without the commit, one `card.refreshed` names the document, `revision` rises by one. An editorial edit of the document's own text gives one event too. M: `card_refresh` without holders; `nodes` not deduplicated; `vouched` ignoring the kind; `within_pair` without the member-name guard; `within_hash` vouched without the compare.
- [x] AC-06 — `--no-effect` clears it with `from` != `to`, and the next meaning change marks again. `--group STEP-SET` clears; `--group STEP-SET-PROOF` exits 1 and changes nothing; a correction's accept re-freezes it. M: `cleared` freezing `norm_hash` alone.
- [x] AC-07 — The almanac reads `trace all | nodes 8, cards 0 | no card 5, in work 0, suspect 0, departed 0, confirmed 0`. The trace-views cycle gives `PART-GATHER` counts (0,1,1,0,0) with its row still `no card`. A holder with a card counts; a document holding nothing counts `no card`; `BOOK-ALMANAC`'s counts equal the sum of its parts'. M: dropping every `no card`; keeping holders.
- [x] AC-08 — `get_card` on a suspect card equals its `list_cards marks:true` row on the four keys, and `spec card show C --marks` agrees. Plain `show` and `GET cards/:id` hold `null`. A `--marks` read over a broken index exits 2, no clean card. Schemas pass the strict walker. M: keys not joined in `run_show`; `--marks` swallowing the index error.

## Verification

Three iterations. 63 tests in CLI `trace_truth.rs`, 7 in core `trace_within.rs`; the mutations died, `nodes` not deduplicated end to end (`ac05_a_duplicated_id_logs_one_event`) and in the store (`queue_trace.rs`).

## Out of scope

Marks through `parent:`, `uses` or sibling sections. Any MCP tool naming a reviewer: ADR-0057 "No MCP tool reaches these routes".

## Implementation

| Module | What it does |
|---|---|
| Model `card.rs` | `CardFrozen.within_hash` |
| Core `trace.rs`, `check/spec_graph.rs`, `check/mod.rs` | `compared`, `HashKind`, `within_input`, `nodes_within` |
| Store `queue/cards.rs`, `queue.rs` | `CardRefresh.kind` |
| CLI `revisions.rs`, `apply.rs` | `within_moved`, `within_pair`, `named_refresh` |
| CLI `trace.rs`, `card.rs`, `main.rs` | `target_norms`, `Reading::counted`, `--marks` |
| MCP `mirror.rs`, `cards.rs`; HTTP `app.rs` | `get_card` asks marks, the daemon does not |

The code moved past the spec in review: one kind per entry, kinded pairs, the member-name guard, the completion's compare.

### Files in scope

The table's, plugin `PINS`, `SCHEMA_PINS`, `fixtures/daemon-keys.json`, `ui/src/api/provisional.ts`, `ui/src/api/daemonKeys.test.ts`, `ui/src/mocks/cards.ts`, `ui/src/mocks/trace.ts`, `ui/src/test/builders.ts`.
