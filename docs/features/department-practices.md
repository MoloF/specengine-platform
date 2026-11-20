---
class: spec
status: shipped
shipped: 2026-11-20
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-import, crates/specengine-eval, plugin, scripts, ui]
ref: RM-0128; the owner's word of 2026-11-19, wish 2, and of 2026-11-20 (08 s2 Phase 7, "Owner's word, 2026-11-19"); the analyst's breakdown, slice 2; working answers A1-A8 (no new entitlement, `[auto]` kept off practices, disciplines not departments, no practices for a task no card lists, links only where the daemon resolves, a warning for an unlisted discipline, examples copied by a person, the word "practices") awaiting the owner's review
adrs: [ADR-0084]
---

# department-practices: each department's agreed rules, fed to its cards and tasks (RM-0128)

## Why

The owner (2026-11-19) wants a documentation corner per department, holding agreed rules such as realism, normal maps, level of detail, limits and optimisation for 3D models. He wants it served over MCP and cited by development and review, "a direct, current link to the documentation on that point".

Two pieces were missing:
- nothing tied a document to a discipline, so no card or task received its department's rules;
- a rule ID in a card's or proposal's text was not a link.

ADR-0084 makes a document naming disciplines a practice. A practice is standing context for those disciplines' cards and tasks, and gates nothing.

## Data

The shapes this slice fixed are now canon. Each heading names where its shape lives.

### Front-matter `disciplines`
`docs/canon/card-practices.md` "`disciplines:`": typed (`TYPED_KEYS` 33), never counted, `practice-discipline` (`CHECK_CODES` 40). Practices live under `docs/spec/practices/<trade>/`.

### The index: `INDEX_FORMAT` 19
`card-practices.md` "`disciplines:`", and the store README.

### The bundle: layer 10, `--discipline`
`card-practices.md` "The bundle", and `spec-cli-bundle.md` "Layers".

### The card: `practices` and `cited`
`card-practices.md` "The card", `cards.md` and `card-context.md`.

### The task package: `practices`
`card-practices.md` "The task and the proposal", and `task-package.md` "Package".

### Proposals: `cited`
`card-practices.md` "The task and the proposal", and `proposal-review.md` "Review document".

### The library
`spec-library.md` "Rows": `disciplines`, the Discipline facet.

### MCP and the plugin
`card-practices.md` "Library, presets, MCP", and `plugin-files.md` (0.48.0, MINOR).

### Presets and examples
`presets.md` "The presets", and `auto-mode.md` (`exclude_zones`).

## Acceptance criteria

The scene is CLI `tests/practices_scene/`:
- `fixtures/studio-lamp` with `PRA` in `[ids]`;
- `docs/spec/practices/{art/models,development/code,shared/naming,puppetry/ghost}.md`, and a superseded decision with `disciplines`;
- cards C-0001 (`art`), C-0002 (`development`) and C-0003 (`sound`);
- task T-0001, listed by C-0001 and C-0002, and T-0002, listed by none.

Mutations M1-M13 each turned their criterion red.

- [x] DP1 — `disciplines` is typed and never counted; `key-extra` under a closed contract; a string gives `frontmatter-type`.
- [x] DP2 — the entry warnings fall on entries 1, 3, 4 and 20 of 20; the kept list is `art` plus 15; the exit is unchanged.
- [x] DP3 — a discipline no flow lists is warned of once; nothing under `PLAIN_FLOW` or a broken `[card]`.
- [x] DP4 — `INDEX_FORMAT` 19 and the `disciplines` table; an index at 18 is re-read.
- [x] DP5 — layer 10 holds the asked disciplines' rules in (path, ord), never a Tier 3 practice.
- [x] DP6 — without disciplines, the body, text and hash are byte-equal; the JSON gains `layers.practices: []`.
- [x] DP7 — under a tight budget layers 1-9 are unchanged and the rules fall to tail IDs.
- [x] DP8 — a linked rule stays in its earlier layer.
- [x] DP9 — the card's `practices` and `cited`. Plain `show` reads no index, proven with every `.md` and the practices directory unreadable, on a card that links no task.
- [x] DP10 — `--context` equals `spec bundle … --discipline`; the new lines count in the room.
- [x] DP11 — the caps 32, 128 and 64, and 16 384 JSON characters each for practices and citations, with boundary tests in cards, the review and the package; `CARD_CONTEXT_MAXIMUM` re-pinned to 467 556, within 500 000.
- [x] DP12 — the package's `practices`, `bundle.disciplines` and the brief's section; a task no card lists gets none.
- [x] DP13 — a proposal's `cited`, in parity across `review`, `get_proposal` and `proposals/:id`. The review text prints no `cited:` line, only the JSON carries it.
- [x] DP14 — MCP and daemon parity, `?practices=true`, the key sets, descriptions within 2 048 B, plugin 0.48.0.
- [x] DP15 — nothing gated.
- [x] DP16 — [measure] at 10k nodes: 31 ms, 2 ms more than a no-match card, a hub's `run_ms` 76 ms. At 1k: 19 ms, 2 ms more, 25 ms.
- [x] DP17 — [soak] (bar 0.5 MiB/min; at most one walk per `/mcp` `get_card {context}` call, for its mark, never two; the page route within 1 + ⌊`SOAK_SECS`/300⌋): slopes 0.0 and 0.0 MiB/min (RSS 93.0→102.2 and 92.6→102.5 MiB); one walked line per `/mcp` `get_card` call in both runs (3 714 and 3 346 calls); the page route 0 walked of 372 and 335 reads (cap 2); client worst 1 912 and 2 245 ms, `run_ms` worst 430 and 410 ms; no hang. `mcp_hub_trust.rs` `dp17_a_hub_get_card_with_context_walks_once` pins "never two", since the slow line cannot count walks.
- [x] DP18 — the four gamedev examples, `spec init` writing one file, LG-8.
- [x] DP19 — `[auto]` keeps off practices.
- [x] DP20 — determinism. The review output masks `<root>`.
- [x] DP21 — UI: the Practices part, `cited` links (an escaped ID never links), the Discipline facet.
- [x] DP22 — this repository's outputs are unchanged without disciplines.

## Out of scope

- a gate, verdict or compliance score;
- entitlements beyond the leads' zones;
- writing practices from the UI;
- `spec init` writing examples;
- software examples;
- a hub's `T-` tasks;
- the guide (RM-0129).

## Implementation

| Module | What it does |
|---|---|
| model, core `front_matter.rs`, `practice.rs`, the check | `disciplines`, the entry rules, `practice_rules`, `cited_in`, the caps, `practice-discipline` |
| store `practices.rs`, `queue/cards.rs`, `lib.rs` | the `disciplines` table, `practices_of`, `cards_listing_task`, `INDEX_FORMAT` 19 |
| CLI `practice.rs`, `bundle.rs`, `card.rs`, `package.rs`, `task.rs`, `review.rs`, `library.rs` | layer 10, `--discipline`, the card's and task's practices, `cited`, the library's key |
| MCP `mirror.rs`, the descriptions; http `app.rs` | the schemas, `?practices=true` |
| `scripts/presets/`, `plugin/`, `.env.example` | the four examples and the comments; 0.48.0 |
| UI `src/practices/`, `src/markdown/`, `documents/` | the part, `cited` links, the facet (`documents.*.disciplines`); every card read asks `?practices=true` |

**Readings** (the orchestrator's):
- DP2 lands on entry 20.
- DP9's unreadable index means every `.md` and the practices directory, on a card that links no task.
- DP20's review output masks `<root>`.
- A card linking a task prints a walk warning twice on `--context`.
- DP17's walked lines: a hub's MCP `get_card` already walked for its mark, by read-trust, before this slice; the spec's "stay trusted, as today" was wrong for it. The context's bundle and practices now trust, so a call walks at most once, not twice; a cancelled card, or one with no targets and no live parent, gets no mark walk. The mark's walk grows with the corpus; trusting it belongs with the concurrent-trusted-reads ADR.

**Release.** Three iterations. Plugin 0.48.0 (MINOR), tag `v0.48.0`.
- The full run: 6 482 tests, 6 480 passed. Two failed:
  - `shards_repo`: its pinned `docs/canon/card*.md` list gained `card-practices.md`, fixed and green;
  - `autopilot rn4_until`: timing under full-suite load; it passes alone, and this slice does not touch it, as in RM-0127.
- The UI suite: 245 files, 6 421 tests.
- First load: 835 195 B of 845 kB, so the next UI slice plans a lazy chunk.
