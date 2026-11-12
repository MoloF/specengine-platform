---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0088, its pair half, v5 core slice 10 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section 3, RM-0088, the pair criteria); docs/features/plain-questions.md and docs/features/collision-pass.md; the orchestrator's working answers, the owner's review open
adrs: []
---

# collision-pairs: a two-sided collision shows as two documents side by side under their titles (RM-0088, the pair half)

## Why

`collision-pass` has agents file a real collision as `report_discrepancy` with `gap_type: contradicts` (`docs/features/collision-pass.md` D4). The owner gets a summary, IDs and a flat evidence list, and works out which quote belongs to which document. His vision asks for "what conflicts with what", side by side under their titles (RM-0088).

After this slice the core computes per read, for a `contradicts` discrepancy with exactly two targets, a `pair`: each side's ID, title and path, resolved now, with the indices of the evidence citing that side's file, and the rest under `other`; any other item `null`. The review document (`spec review`, `get_proposal`, the daemon's `Proposal`) and the round item serve it; the Inbox, Questions and a card's Answer… render two columns from a lazy chunk. Nothing stored, no queue schema or backup format step; plugin 0.45.0, since `get_proposal`'s output schema gains a key.

No ADR: ADR-0012 (a pair decides nothing), `docs/canon/architecture.md#ui` (the UI renders the served pair, never groups), ADR-0072, and the keys of the two specs before it.

## Description and interactions

### Decisions

- **D1. Only a two-target `contradicts`**: `kind` `discrepancy`, `gap_type` `contradicts`, `target_ids` of length 2, any status; else `null`, three targets included. The UI never decides it: it shows a served pair.
- **D2. Per read, never stored.** Titles and paths are resolved as `spec round` resolves a target (`docs/canon/round.md` "JSON": `id` `null` for a path target, `title` `null` when gone), so a rename shows on the next read with no queue write.
- **D3. Evidence by index, not by copy.** A side lists positions in the item's `evidence`, so no quote is served twice and the review stays within its measured size (`docs/canon/agent-intake-documents.md` "Escaping and size"); `other` lists the rest, so the UI computes no set difference.
- **D4. Served where `evidence` is**: the review document and `RoundItem`. `InboxEntry` and `CardItem` hold no evidence or `gap_type`; the Inbox and a card's Answer… read the shown item's review document already (`InboxView.tsx`, `CardItems.tsx` `useProposal`).
- **D5. Plugin 0.45.0**, the MINOR after `collision-pass`'s 0.44.0: the `ReviewDocument` output schema gains the closed `pair`; descriptions, `INSTRUCTIONS` and skills unchanged.
- **D6. The UI** is built on the primitives (`docs/canon/ui-design.md` "Primitives") and scale tokens, strings in both catalogs, first-load JS within 845 kB.

## Data

### The rule (core, new `crates/specengine-core/src/pair.rs`, pure)

```rust
pub struct PairTarget<'a> { pub id: Option<&'a str>, pub title: Option<&'a str>, pub path: Option<&'a str> }
pub struct PairSide { pub id: Option<String>, pub title: Option<String>, pub path: Option<String>, pub evidence: Vec<usize> }
pub struct Pair { pub left: PairSide, pub right: PairSide, pub other: Vec<usize> }
pub fn pair_applies(kind: &str, gap_type: Option<&str>, targets: usize) -> bool;   // D1
pub fn collision_pair(left: PairTarget<'_>, right: PairTarget<'_>, evidence_files: &[&str]) -> Pair;
```

`left` is `target_ids[0]`, `right` `target_ids[1]`, as stored. An evidence item is a side's when its `file` equals that side's `path` byte for byte (`./docs/a.md` is not `docs/a.md`); with one path on both sides, under both. A side with `path` `null` holds none. Lists ascend. The caller resolves the two targets only when `pair_applies` holds (PP6's seam), by round's `target` (CLI `round.rs`), shared.

### The example (a scratch copy of `fixtures/spec-a`)

`spec propose discrepancy --input - --author-role developer`, with stdin:

```json
{"node_ids": ["RULE-SPRINT-COST", "RULE-STAM-REGEN"], "gap_type": "contradicts", "severity": "normal",
 "summary": "Sprint cost and regeneration disagree on when stamina refills",
 "evidence": [
  {"file": "docs/spec/movement/sprint.md", "lines": "18-20", "observed": "Sprinting pauses regeneration", "documented": "Cost is drained per second"},
  {"file": "docs/spec/movement/stamina.md", "lines": "21-23", "observed": "Regeneration never pauses", "documented": "Refill starts after 1.5 s"},
  {"file": "src/stamina.rs", "qpath": "stamina::regen", "lines": "40-52", "observed": "regen runs during sprint", "documented": "a pause is specified"}],
 "options": [{"label": "Pause while sprinting", "effect": "Sprint drains faster", "price": "Regeneration rewritten"},
             {"label": "Never pause", "effect": "Sprint lasts longer", "price": "Sprint cost rewritten"}],
 "recommendation": 0}
```

The review document (`--json`, `--brief` alike, and the daemon's `GET …/proposals/:id`) holds, right after `evidence`:

```json
"pair": {"left":  {"id": "RULE-SPRINT-COST", "title": "Cost", "path": "docs/spec/movement/sprint.md", "evidence": [0]},
         "right": {"id": "RULE-STAM-REGEN", "title": "Regeneration", "path": "docs/spec/movement/stamina.md", "evidence": [1]},
         "other": [2]}
```

The text, right after the evidence lines and only when `pair` is not `null`, is escaped as the rest of the text is:

```text
pair:
  left RULE-SPRINT-COST | Cost | docs/spec/movement/sprint.md | evidence 0
  right RULE-STAM-REGEN | Regeneration | docs/spec/movement/stamina.md | evidence 1
  other | evidence 2
```

A side: `<id, else the path> | <title, else -> | <path, else -> | evidence <i, j…, else ->`; then `other | evidence <…, else ->`; indices count the evidence lines from 0. With `pair` `null` no line, so every other item's text and MCP `content` stay as they are.

### Shapes and keys

| Shape | Gains | Keys |
|---|---|---|
| review document, daemon `Proposal`, MCP `ReviewDocument` | `pair` after `evidence` | 54: plain-questions' 53 plus one |
| `spec round --json` item, `RoundItem` | `pair` after `evidence` | 22 |
| new `Pair` | `left, right, other` | 3 |
| new `PairSide` | `id, title, path, evidence` | 4 |

MCP `mirror.rs`: `Pair` and `PairSide` closed (`additionalProperties: false`), `evidence` and `other` integer arrays. The round's text sheet is unchanged. **Plugin**: `plugin.json` and `scripts/deploy/.env.example` 0.45.0; `PINS`, `SCHEMA_PINS` (`plugin_files.rs`) append it; `tag-check.sh v0.45.0` prints it.

### The UI

- **Types** (`provisional.ts`, citing PP8): `Pair {left: PairSide; right: PairSide; other: number[]}`, `PairSide {id, title, path: string | null; evidence: number[]}`, `Proposal.pair` and `RoundItem.pair` `Pair | null`; `builders.ts`, `mocks/build.ts` follow; the mock gains PP1's item and a three-target `contradicts`.
- **`src/inbox/EvidenceItem.tsx`** (new): `ProposalCard.tsx`'s evidence `<li>` moved out, used by both.
- **`src/inbox/PairView.tsx`** (new, default export, lazy wherever used), `{pair, evidence}`, a region labelled `pair.label` "What conflicts with what":
  - two `section`s side by side, stacked when narrow (CSS grid, scale tokens, `.collision-pair-*`; `.pairs` is taken);
  - each headed one level below the item: the title in `<bdi>` (else the ID or path, `lang="en"`) as a button opening the document modal, `openDocFrom(id ?? path, button)` (`docs/canon/doc-view.md` "History and focus"); `title` `null` adds `pair.gone` "Not in the spec now", and with `path` `null` too the heading is no button;
  - under it the ID and path, mono, secondary, then the side's evidence in served order, else `pair.noEvidence` "No evidence cites this file";
  - below both, `pair.other` "Other evidence" with `other`'s items, when any.
- **Where**: the Inbox's `ProposalCard`, Questions' `RoundItemCard`, the card Activity's Answer… (`CardItems.tsx` `AnswerDialog`), each in a `Suspense` with a skeleton, right after the summary when `pair !== null`, in place of the flat evidence list; with `null`, as today.
- **Strings**: `pair.label`, `pair.other`, `pair.noEvidence`, `pair.gone` in `en.ts` and `ru.ts`, the Russian a draft.

## Rules and edge cases

- WHEN an item is not a two-target `contradicts` discrepancy, the system SHALL serve `pair: null` and resolve no target for it.
- WHEN a target is gone after intake, its side SHALL still be served: `title` `null`, `path` the stored `target_path` for the first target, `null` for the second.
- WHEN both targets share a file, its evidence SHALL be under both sides, not under `other`; every index of `evidence` SHALL be in `left`, `right` or `other`.
- The UI SHALL NOT compare `gap_type`, count targets or match `file` to a path (`policy.test.ts`; `labels.ts` stays the one `gap_type` compare). Agent text stays verbatim in `<bdi>`; no quote is reordered or cut. A pair refuses, gates and changes nothing (ADR-0012).

**Rollout.** No schema step, backup format or config key. An older daemon serves no `pair`, and the UI ships inside its daemon (`docs/canon/serve-ui.md`), so they never skew; an older hub's `/mcp` keeps the old output schema until its upgrade, the owner's call.

**Test churn.** The review key lists (CLI, the MCP walkers, http `daemon_keys.rs`, `fixtures/daemon-keys.json`, `daemonKeys.test.ts`); the round JSON pins; the plugin pins (`plugin_files.rs`, `plugin_skills.rs`, `plugin_roles.rs`) and http `spec_ids_hub.rs` (`v0.45.0`); `ProposalCard` tests (the `EvidenceItem` move).

## Acceptance criteria

A browser look is marked **[look]**. Every other criterion is a test that the named mutation turns red. "The example" is Data's, on a scratch copy of `fixtures/spec-a`.

- [ ] PP1 — **The pair, served alike.** The example's `spec review PR --json` holds Data's `pair`; `get_proposal`'s `structuredContent`, `GET …/proposals/PR` and the `spec round --json` item the same object. Mutations, each red: evidence grouped by position, not `file`; `left` and `right` swapped.
- [ ] PP2 — **Null elsewhere.** `pair` is `null` for the example with a third target (`MEC-SPRINT`), with one target, with `gap_type` `missing`; for a question on the two targets; for an update. Mutation, red: served for any two-target discrepancy.
- [ ] PP3 — **One file, a gone side.** Targets `MEC-STAMINA` and `RULE-STAM-REGEN`, both in `stamina.md`: evidence 1 under both, `other` `[0, 2]`. The example after `RULE-STAM-REGEN`'s section is removed: `right` `{"id": "RULE-STAM-REGEN", "title": null, "path": null, "evidence": []}`, `other` `[1, 2]`. Mutations, each red: `null` for a gone side; a shared file's evidence under `left` alone.
- [ ] PP4 — **Per read.** Retitling `RULE-SPRINT-COST` "Sprint cost" changes `pair.left.title` on the next review; the queue file's bytes and last event `seq` are unchanged by it; `QUEUE_SCHEMA_VERSION` and `STATE_FORMAT` keep `collision-pass`'s values. Mutation, red: the title taken from the row stored at intake.
- [ ] PP5 — **Text.** `spec review PR` prints Data's four lines after the evidence lines; a three-target item's text and an existing review golden are byte-equal to before. Mutation, red: a `pair: null` line printed.
- [ ] PP6 — **Resolution only for a pair.** A seam counts target resolutions in `spec review`: 0 for a question, an update and a one-target discrepancy, 2 for the example. Mutation, red: targets resolved for every item.
- [ ] PP7 — **MCP and plugin 0.45.0.** The schema walker passes with exact key sets, `Pair` and `PairSide` closed; tool counts unchanged. `plugin.json` 0.45.0, `.env.example` `SPECENGINE_VERSION=0.45.0`, `tag-check.sh v0.45.0` prints it, `PINS` and `SCHEMA_PINS` end with 0.45.0. Mutation, red: `pair` an open object.
- [ ] PP8 — **The daemon serves it, the UI types it.** The fixture's `Proposal` 54 keys, `RoundItem` 22, `pair` after `evidence`, sets `Pair` and `PairSide`; `provisional.ts` types them, `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty at the end. Mutation, red: a set left out of the fixture.
- [ ] PP9 — **The screens, on the mock.** Inbox, Questions and a card's Answer… show PP1's item as two sections under "Cost" and "Regeneration", each heading opening the document modal (`?doc=` in the hash), each listing its side's evidence, "Other evidence" the code quote, no flat list; the three-target item the flat list as today; a mock pair whose served grouping differs from a path match renders as served. `policy.test.ts`, `scale.test.ts` pass. Mutations, each red: the UI grouping by `file` itself; `gap_type` compared in `PairView.tsx`; a `pair.*` key missing in `ru.ts`.
- [ ] PP10 — **Lazy.** After `pnpm build` the `PairView` chunk is not among first load's files; first-load JS grows at most 1 kB, within 845 kB, the figure recorded. Mutation, red: a static import of `PairView` in `ProposalCard.tsx` (`lazy.test.ts`).
- [ ] PP11 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800 (stacked): PP1's item on the Inbox, Questions and Answer…; a gone side; a shared file; the three-target item; Tab reaches both headings, Enter opens the modal, Esc returns focus to the heading.
- [ ] PP12 — **Gates.** `cargo nextest run` green for `specengine-core`, `-cli`, `-mcp`, `-http`, then the full run once; the eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass; UI lint, `tsc`, `pnpm test`, `pnpm build` pass.

## Out of scope

A pair for three or more targets or another gap type; the documents' bodies inline (the heading opens the modal); `pair` on `InboxEntry` or `CardItem`; a `spec round` sheet line; a stored pair; a diff of the two texts; a new MCP tool, description or skill change; judging evidence paths (`./` prefixes).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{pair,lib}.rs`; `crates/specengine-cli/src/{review,round,proposals}.rs` (the resolver shared, the document and text); `crates/specengine-mcp/src/mirror.rs`; `plugin/specengine/.claude-plugin/plugin.json`, `scripts/deploy/.env.example` |
| test-engineer, Rust | core `tests/pair.rs`; CLI review and round tests (PP1-PP6, the seam); `plugin_files.rs` pins, the MCP walkers, http `daemon_keys.rs`, `spec_ids_hub.rs`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`, once) with `daemonKeys.test.ts` in the same change |
| ui-developer | `provisional.ts`, `builders.ts`, `src/mocks/{build,MockClient}.ts`, `src/inbox/{EvidenceItem,PairView,ProposalCard}.tsx`, `src/round/RoundItemCard.tsx`, `src/board/CardItems.tsx`, `src/app/lazy.test.ts`, `src/styles/app.css`, `en.ts`, `ru.ts` |
| test-engineer, UI | `PairView.test.tsx`, the three screens' tests, PP9 and PP10's mutations |

Comments cite `docs/features/collision-pairs.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. After core slice 9 `collision-pass` ships (0.44.0, schema 14), one Rust slice at a time; the UI after plain-questions' UI (both edit `ProposalCard`, `RoundItemCard`, `CardItems`).
2. Core, CLI, MCP and the plugin pins, http keys; the UI behind `AHEAD_OF_FIXTURE`; tests overlap; reviews; the look (PP11); ship ("Canon at ship"). At most three iterations a lane.

### Risks

- **Size**: three text lines at most; the review's `content` bound (`agent-intake-documents.md` "Escaping and size") is re-measured, not assumed.
- **Cost**: a two-target `contradicts` review resolves two targets, as round does per item; PP6 keeps every other review free of it.
- **Version**: if another slice takes 0.45.0 first, this one takes the next free MINOR and records it here.

### Canon at ship

Free bytes are in brackets. "Net" means trimmed in the same file to fit.

| File | Edit |
|---|---|
| `docs/canon/agent-intake-documents.md` [8 492] | "Review document": `pair`, its rule, the text lines; "Escaping and size": the re-measured bound |
| `docs/canon/round.md` [351] | "JSON": 22 keys, `pair` by pointer. Net |
| `docs/canon/proposal-review.md`, `docs/canon/round-ui.md`, `docs/canon/card-questions.md` | "Inbox", "Screen": the pair view, lazy, by pointer. Net where full |
| `docs/canon/plugin.md`, `docs/canon/plugin-files.md` | **Version** 0.45.0 |
| `docs/canon/ui-languages.md` | "Size": the figure. Net |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 10; RM-0088 | shipped, both halves |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, which the owner has not answered: `pair {left, right}` for a two-target `contradicts` only, served and never computed in the UI, side by side under titles, and lazy.

**The spec-writer's:**
- W1 (D4): served on the review document and `RoundItem` alone. The brief also named Inbox entries and card items; neither holds evidence or `gap_type`, and up to eight quotes in every Inbox row would weigh down a list read the screens do not need.
- W2 (D3): indices, not copies, plus `other`: the brief's shape had no place for evidence citing neither side, and the UI must not compute the remainder.
- W3: `left` is the first stored target, the agent's order, stable across reads. A text side line shows a path target in place of a `null` ID, as round's `Concerns` does.
