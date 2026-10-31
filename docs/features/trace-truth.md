---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin]
ref: RM-0066; Phase 7 step 7a (08 s2)
---

# trace-truth: a nested edit marks, counts stop at containers, a card read shows its mark (RM-0066)

## Why

The trial of 2026-10-31 found three reads that say something untrue. ADR-0026 puts rules in `{#ID}` sections and flow steps are documents (`spec-flow.md` "The read"), so the prescribed layout puts a card on a document and its rule in a section of it. `norm_hash` covers own text only, so the section changes, the card stays accepted and the rollup says `confirmed`: "a changed node marks its cards" is false there. `no card` counts containers, so a fully confirmed group reads "N confirmed, 1 no card" until it gets a card of its own. `get_card` shows no mark, so an AI about to act on a suspect card reads a clean one.

First of the closing group of the first result (08 s2 Phase 7): truth comes before the authoring work that leans on it.

## Description and interactions

A card's entry freezes, beside the target's own `norm_hash`, a `within_hash` over the target and the ID sections nested in it, and suspect compares that. Text appended under `STEP-SET-PROOF` makes a card on `STEP-SET` read `suspect STEP-SET` in `spec trace`, `spec flow --trace`, `list_cards marks:true` and on the board. A mark stops at the file: `parent:` children, `uses` and sibling sections stay clean, so nothing floods. Nothing stops on a mark (ADR-0012): `spec check` and `spec gate` never read it.

`--editorial` of a section vouches for the document and sections around it, in the transaction recording `applied`, so a wording pass marks nothing. An accept and a clear freeze the new hash.

A node holding live documents, with no card of its own, is counted nowhere: not by `Reading::counts`, the header or a lane. Its row word stays `no card`, and a book's counts still equal its parts'.

`spec card show C --marks` adds the four keys a list row carries, and `get_card` always asks it. Plain `show` and `GET cards/:id` hold them `null`: the panel keeps its marks from the list (`board-ui.md` "Screen").

## Data

**`CardFrozen`** (model `card.rs`), `within_hash` `#[serde(default)]`, so a stored row with no key reads `None`:

```json
{"snapshot": [{"id": "STEP-SET", "rev": 1, "norm_hash": "b3:...", "within_hash": "b3:...", "also": []}]}
```

| Key | Holds |
|---|---|
| `norm_hash` | the target's own hash, as today |
| `within_hash` | the hash below; `null` when no such hash could be frozen |
| `also` | flat, both kinds of hash, the last `ALSO_MAX` (16) |

**`TargetNow`** gains `within_hash: Option<String>`, filled in the one constructor, `target_norms` (CLI `trace.rs:361`), per read, never stored.

**Recipe.** Members are `within(target)`: the node and the ID sections nested in it. A pair is (name, `norm_hash`), the name being `SpecGraph::name`'s: the node's `id`, else the file's path. Pairs sort by name.

- One member: `within_hash` is that member's `norm_hash`, the same string.
- Any member's hash null: `null`.
- Else `b3_hash` of `<name>\0<norm_hash>\n` per pair.

**Seams.** Core holds no BLAKE3 (`blake3` is a dependency of store, code and import), so core returns the input bytes, as `norm_input` does, and the CLI hashes them with the store's `b3_hash`. `SpecGraph::within` needs a graph and step 6 has two parses and none, so the span walk moves into a core function over `&[Node]`, which `within` calls: the read and step 6 must take the same members.

**Editorial refresh.** `Revisions::card_refresh` also returns `CardRefresh {node, old, new}` for each ID node, by `paired(before, after)`, whose `within_hash` moved, under `--editorial` only. `vouched` treats `within_hash` as a held hash beside `norm_hash` and `also`. `refresh_snapshots` (store `queue/cards.rs`) logs one `card.refreshed` per (card, node): a document whose own and `within_hash` both moved sends two changes, and `revision` already rises once per card.

**`card.cleared`** keeps its keys `{id, rev, from, to}` per node. `id` is the target, `from` and `to` its `within_hash` (else `norm_hash` on an entry with none), so they differ even when the own hash held. `rev` is the target's own, so a `Revision:` join for a nested change must walk `within`; RM-0056 carries that note.

**`spec card show C --marks`.** `CardShowRequest` gains `marks`. The show document gains `mark, on, discrepancy, correction`, every key present, `null` without the flag and on a cancelled card; values as `CardRowJson`. Text: a `key: value` line each, `-` for null, as any `show` key. `get_card` sets `marks: true` and takes no new argument; `GET cards/:id` sets `false`. Key sets: `fixtures/daemon-keys.json` `Card` +4, `CardFrozen` +1; `ui/src/api/provisional.ts`, `ui/src/mocks/cards.ts` follow; `CardFrozenShape` and `CardShowDocument` in `mirror.rs`. Plugin 0.26.0 (MINOR: a key a tool returns), `PINS`, `SCHEMA_PINS`.

**Schema.** No queue step: 9 stays, the column holds JSON, and dumps read rows raw (`queue/state.rs:5`). `INDEX_FORMAT` stays 14.

## Rules and edge cases

- WHEN an entry holds `within_hash`, suspect SHALL compare it with `TargetNow.within_hash`: suspect when the two differ and `also` holds no new one. A null now reads not suspect, with the note `mark_notes` gives today.
- WHEN an entry holds no `within_hash`, suspect SHALL compare `norm_hash` as today: no false mark, and a note only where the target's `within_hash` now differs from its `norm_hash`: `<C>: <target> froze its own text alone: a change in its sections marks nothing until an accept or a clear`.
- WHEN a group clear names the document or above, it clears as today. WHEN REF is the section, nothing qualifies and it exits 1: re-freezing would swallow other sections' changes. The rule of `spec-trace.md` "Clearing" is unchanged.
- WHEN a section is moved within its file, or only whitespace changes, nothing marks. WHEN a new `{#ID}` section appears, it marks: the pairs changed.
- A holder may stand in a lane, so a lane's count can differ from its rows: it says the row `no card` and counts it nowhere. Accepted. trace-views AC-07's "less than the header" ends: the almanac's lanes now sum to it.
- `norm_hash`, `specengine-norm/v1`, `spec.lock` and every revision stay as they are (`node-revision.md` "The recipe", ADR-0040). `spec-trace.md` "Known limits" holds: an id-less document's editorial pass still reads suspect.

**Decisions.**

- **D1.** `norm_hash` stays own-text. Widening it would be `specengine-norm/v2`, a re-lock, and raise documents on every section edit.
- **D2.** Pairs sort by name: a moved section is no change, as in `rev-not-raised`.
- **D3.** Holders are found by walking `graph.children` and stopping at the first live document, not by `held_documents`, which keeps the whole read light. A document whose children are sections alone holds nothing.
- **D4.** No migration of old entries: only cards made today can hold one.
- **D5.** A completion vouches only what its trailers name; a holder's mark stays for a person. That is a false mark, never a false clean.
- **D6.** Detail goes to `spec-trace-record.md` (under 4 KB), not to `spec-trace.md`.
- **D7.** `get_card` is `spec card show --marks`; the daemon's panel keeps marks from the list.
- **D8.** An index failure in `--marks` is the read's error, as in `list_marks`, never a clean card.

No ADR: this fits ADR-0056, and "Suspect" is canon detail, not a rule of `architecture.md`.

## Acceptance criteria

Fixture: copies of `fixtures/trace-almanac`; a test adds what it lacks (a sibling section, a `uses` link) to its copy, since its `nodes 8` are pinned.

- [ ] AC-01 — A card on `STEP-SET` accepted, text appended under `STEP-SET-PROOF`: `spec trace STEP-SET` reads `suspect`, the card `suspect STEP-SET`; `spec flow --trace` and `list_cards marks:true` agree; `spec check` and `spec gate` output is unchanged. M: `within_hash` over the target alone.
- [ ] AC-02 — The same edit leaves clean a card on `PART-GATHER`, on a sibling section, and on a node that `uses` `STEP-SET`. M: walking `held_documents` into the hash.
- [ ] AC-03 — Whitespace under the section, or the section moved in its file: no mark. A new `{#ID}` section: a mark. M: pairs in file order; pairs without names.
- [ ] AC-04 — A stored entry with no `within_hash` key on a sectioned document reads `accepted` with exactly one note; a meaning change of its own text marks it; the clear of that mark, or an accept of an open card in the same state, writes the key. M: an absent key read as a change.
- [ ] AC-05 — `spec approve --editorial` of the section: the document's card stays clean here and in a worktree without the commit, one `card.refreshed` names the document, `revision` rises by one. An editorial edit of the document's own text gives one event too. M: `card_refresh` without holders; `nodes` not deduplicated.
- [ ] AC-06 — `--no-effect` clears it with `from` != `to`, and the next meaning change marks again. `--group STEP-SET` clears; `--group STEP-SET-PROOF` exits 1 and changes nothing; a correction's accept re-freezes it. M: `cleared` freezing `norm_hash` alone.
- [ ] AC-07 — The almanac reads `trace all | nodes 8, cards 0 | no card 5, in work 0, suspect 0, departed 0, confirmed 0`. The trace-views cycle gives `PART-GATHER` counts (0,1,1,0,0) with its row still `no card`. A holder with a card counts; a document holding nothing counts `no card`; `BOOK-ALMANAC`'s counts equal the sum of its parts'. M: dropping every `no card`; keeping holders.
- [ ] AC-08 — `get_card` on a suspect card equals its `list_cards marks:true` row on the four keys, and `spec card show C --marks` agrees. Plain `show` and `GET cards/:id` hold `null`. A `--marks` read over a broken index exits 2, no clean card. Schemas pass the strict walker. M: keys not joined in `run_show`; `--marks` swallowing the index error.

## Out of scope

Marks through `parent:`, `uses` or sibling sections. A completion vouching holders (D5). Widening `norm_hash` (D1). Migrating entries (D4). Any MCP tool naming a reviewer: ADR-0057 "No MCP tool reaches these routes", and ADR-0056 has the requesting side accept, so an agent naming the reviewer picks who accepts its work; if it hurts at volume, a default reviewer in `[[card.flow]]` fits ADR-0056 with no tool, unplanned.

## Implementation

To fill in after implementation.

### Files in scope

Model `card.rs`. Core `trace.rs`, `check/spec_graph.rs` (the shared walk). Store `queue/cards.rs`. CLI `trace.rs`, `card.rs`, `revisions.rs`, `main.rs` (`--marks`). MCP `mirror.rs`, `cards.rs`. HTTP `app.rs` (`CardShowRequest`'s new field, `marks: false`). `fixtures/daemon-keys.json`, `ui/src/api/provisional.ts`, `ui/src/mocks/cards.ts`. Plugin version, `PINS`, `SCHEMA_PINS`.

Pinned and changing: `crates/specengine-cli/tests/trace_views.rs:999-1004,1269`. First at risk: `is_suspect` runs under every mark read, and a wrong legacy branch marks every card on a sectioned document (`card_trace.rs`, http `tests/cards.rs`); AC-04 guards it. A real editorial edit vouches holders only through `step_6`, new code near the apply door.

### Canon at ship

Free now: `spec-trace.md` 98 B, `cards.md` 4 B, `spec-trace-record.md` 9.5 KB. Tier 1: CLI 3 B, core 1 B, http 1 B, store 3 B free, so no README line grows here.

- **`spec-trace-record.md`** takes the recipe, the entry table and the `card.cleared` line (D6).
- **`spec-trace.md`** "Suspect", "The read": `within_hash` for `norm_hash`, a stop at the file, the `counts` rule, about 80 B. Cut, named: its sentence on `TargetNorms` (106 B) moves to `spec-trace-record.md`.
- **`cards.md`**: `[--marks]` and four keys, 44 B on a 4 B margin. Cut, named: "Known limits" (123 B, no citation reaches it) moves to `spec-trace-record.md`.
