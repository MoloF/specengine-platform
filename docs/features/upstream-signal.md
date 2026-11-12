---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui]
ref: RM-0105, v5 core slice 3 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; the analyst's analysis of 2026-11-12 (section 4, US-1 to US-6); the orchestrator's working answers, the owner's review open
adrs: []
---

# upstream-signal: a subtask sees what changed above it, on the CLI, over MCP and on the Board (RM-0105)

## Why

The writer corrects a parent task's goal, or a decision lands on its document, and a subtask in another department sees nothing: a mark covers only a card's direct targets (`docs/canon/spec-trace.md` "Suspect"), a split's subtask targets only its kind's nodes under the parent's (`docs/canon/card-split.md` "The rule"), and no card write touches another (ADR-0074).

After this slice an open subtask carries `upstream`, what changed above it: its parent's goal, criteria or targets edited, a question, discrepancy or decision applied on their nodes, the parent reading suspect. `spec card show --marks` and `get_card` list the changes, `spec card list --marks` and `list_cards` flag the row, the Board's tile says "Changed above" and the work item lists the lines. Computed per read from the queue's events and the index, never stored; it moves, refuses and gates nothing (ADR-0012). No ADR: it rests on ADR-0074, ADR-0012 and ADR-0077, as its own key.

## Description and interactions

### Decisions

- **D1. Its own key**, never inside `mark`: `card_mark`, `spec trace`, `allowed`, Ready to accept, every move, accept, clear, gate and check stay byte-equal (US-4); a rollup reads no item (`docs/canon/card-questions.md` "The rule").
- **D2. Three sources, direct parent, open subtasks.** The parent's `card.edited` holding `goal`, `targets` or `criteria`; applied questions, discrepancies and decisions of the queue the read runs in, any repository, whose `node_ids` meet the reach (the child's and the parent's targets with their nested ID sections, `SpecGraph::within`, the mark's own reach), the child's own items aside; the parent's mark when `suspect` or `correcting`, worded as the parent's and claiming no order against the child.
- **D3. By `seq`, never clocks; how it ends.** From an anchor, the child's latest `card.created` or `card.edited` holding `parent`. The child's own edit of goal, targets or criteria answers every change before it (it flowed down); accepted or cancelled, it is `null`; the mark line follows the parent's mark, ending when the parent is cleared or its correction accepted (`docs/canon/spec-trace.md` "Clearing"). No acknowledge act (Q4).
- **D4. It rides `--marks`**: reach and the parent's mark need the index. `card show --marks`, `card list --marks`, `get_card` (always), `list_cards {marks: true}`, the daemon's list with `marks=true`; the daemon's card route asks `upstream` alone, its `mark` staying `null` (the panel's marks come from the list: `docs/canon/board-screen.md` "Reads"). No parent or a closed card: nothing read for it.
- **D5. One uncapped read per call**, store `events_of_types`, never the 64-capped `card_history`; RM-0107 reuses it. **D6.** At most `UPSTREAM_MAX` (16) changes, the newest, oldest first; `more` the rest. **D7. Plugin 0.41.0**, a MINOR; `INSTRUCTIONS` and `read-spec` unchanged. **D8. The Board**: a tile badge from the row's flag; a part in the work item from the card route.

"Data" fixes every name, so the lanes never wait on each other.

## Data

### Core, store, CLI library

Core, `crates/specengine-core/src/upstream.rs`, pure, no I/O:

```rust
pub const UPSTREAM_MAX: usize = 16;                                 // changes listed, the newest
pub const UPSTREAM_FIELDS: [&str; 3] = ["goal", "targets", "criteria"]; // card.edited's fields that count, table order
pub enum LoggedType { Created, Edited, Applied }                    // card.created, card.edited, proposal.applied
pub struct Logged<'a> { pub seq: i64, pub kind: LoggedType, pub id: &'a str,
    pub fields: Vec<&'a str>, pub by: Option<&'a str>, pub at: &'a str }
pub struct Decided<'a> { pub id: &'a str, pub kind: &'a str, pub card: Option<&'a str>,
    pub nodes: &'a [String], pub record: Option<&'a str>, pub decided_by: Option<&'a str> }
pub fn upstream(child: &Card, parent: &Card, parent_mark: Option<&CardMark>, log: &[Logged<'_>],
    decided: &[Decided<'_>], reach: &BTreeSet<String>) -> Option<Upstream>;
pub struct Upstream { pub parent: String, pub mark: Option<CardMark>, pub changes: Vec<UpstreamChange>, pub more: usize }
pub struct UpstreamChange { pub seq: i64, pub at: String, pub source: UpstreamSource, pub by: Option<String>,
    pub fields: Vec<&'static str>, pub item: Option<String>, pub kind: Option<String>,
    pub record: Option<String>, pub on: Option<String> }
pub enum UpstreamSource { Edit, Item }
```

The judge, in order:
1. The child accepted or cancelled: `None`.
2. The anchor (D3); none in `log`: edits and items skipped, the mark alone judged.
3. The floor: the anchor, raised by the child's own `Edited` past it whose `fields` meet `UPSTREAM_FIELDS`.
4. **Edits:** the parent's `Edited` past the floor whose `fields` meet `UPSTREAM_FIELDS`. `fields` keeps those, in the event's order, and `by` is the payload's `as`.
5. **Items:** each `Decided` whose `card` is not the child, whose last `Applied` (same `id`) is past the floor, and one of whose `nodes` is in `reach`. `on` is the first such node in the item's order, `seq` and `at` its `Applied`'s, and `by` `identity_name(decided_by)` (node-history's core function: the name, never an email).
6. Changes sorted by `seq`; past 16, the oldest drop into `more`.
7. `mark`: `parent_mark` when `Suspect` or `Correcting`, else `None`. A cancelled parent has none.
8. `None` when `mark` is `None` and `changes` is empty.

Store, `crates/specengine-store/src/queue/cards.rs`:

```rust
/// Every event of this project whose type is one of `types`, oldest first by `seq`, uncapped, in one
/// statement; a payload that is no JSON is passed over.
pub fn events_of_types(&self, types: &[&str]) -> Result<Vec<Event>, QueueError>;
```

CLI library:
- `CardShowRequest` gains `pub upstream: bool`: CLI `main.rs` from `--marks`, MCP `get_card` and the daemon's card route `true`, every other construction explicit (no `Default`). `CardShowOutcome` gains `pub upstream: Option<Upstream>`; `CardListOutcome` `pub upstream: BTreeSet<String>`, the IDs judged non-`None` (`--marks` only).
- New `crates/specengine-cli/src/upstream.rs`, once per call: `events_of_types(&[EVENT_CARD_CREATED, EVENT_CARD_EDITED, EVENT_APPLIED])`; `list_readable` of `Applied` items kept to question, discrepancy and decision, no repository filter (no git); the reach from the marks' own index refresh and `SpecGraph` (`fresh_norms`' graph shared, one `indexed_in`), an ID-less document keyed by its path, a gone target meeting only its stored text. A list judges the marks of parents it filters out too. No open subtask read: no events, items or reach.
- Notes, `show` only: `upstream: <C>: its parent <P> does not read; not judged` (`null`); `upstream: <C>: no card.created event; only its parent's mark is read`. A list: the unreadable row's own note, the row `false`.

### JSON (`--json`, `structuredContent`, the daemon's routes)

Card document, after `correction` (36 keys): `"upstream": null`, or the object below. **The scenario**, the tests' corpus too: `docs/tides.md` holds `TIDE-BOOK` with the sections `{#TIDE-SPRING}`, `{#TIDE-NEAP}`; C-0001, "Write the tide table foreword", targets `TIDE-BOOK`; a split (`copy=binding`, `plates=open`; a `[[card.split]]` row gives `TIDE-SPRING`'s kind both, each discipline's flow named for it, first column `backlog`) makes C-0002 and C-0003 on `TIDE-SPRING`, seqs 2 and 3; seq 4 edits C-0001's goal as `Ann`; PR-0040, a decision on `TIDE-NEAP`, is applied at seq 9 with DEC-0031, `decided_by` `Ann Owner <ann@example.org>`; then a hand edit of `TIDE-NEAP` makes C-0001 read `suspect TIDE-BOOK`.

```json
"upstream": {"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK", "correction": null,
 "changes": [
  {"seq": 4, "at": "2026-11-12T09:00:00Z", "source": "edit", "by": "Ann", "fields": ["goal"],
   "item": null, "kind": null, "record": null, "on": null},
  {"seq": 9, "at": "2026-11-12T10:00:00Z", "source": "item", "by": "Ann Owner", "fields": [],
   "item": "PR-0040", "kind": "decision", "record": "DEC-0031", "on": "TIDE-NEAP"}],
 "more": 0}
```

`parent` the parent's ID; `mark`, `on`, `correction` the parent's mark word (`suspect`, `correcting`) and its keys, all `null` when neither; a change's `seq`, `at` its event's (an item's `proposal.applied`), `source` `edit` or `item`, `by` an edit's `as` or an item's `decided_by` name (`null` none recorded), `fields` an edit's counted fields (`[]` on an item), `item`, `kind` (`question`, `discrepancy`, `decision`), `record` (`record_id`), `on` (the first meeting node) an item's (`null` on an edit); `more` the changes older than the 16 listed.

`null` unasked (`show` without `--marks`), with no parent, on a closed card, and when nothing is said. A list row gains `upstream` after `correction` (23 keys): `true` or `false` under `--marks` (`false` on a cancelled card), `null` without. Write documents, a split child (9 keys) and `CardList` are unchanged.

### Text

`show`'s lines after `correction:`, the scenario's C-0002:

```text
upstream: parent C-0001 | suspect TIDE-BOOK
upstream: 4 | edited goal | Ann | 2026-11-12T09:00:00Z
upstream: 9 | decision PR-0040 | record DEC-0031 | on TIDE-NEAP | Ann Owner | 2026-11-12T10:00:00Z
```

First `upstream: parent <P> | <the parent's mark as a list row prints it, or ->`; an edit `upstream: <seq> | edited <fields joined ", "> | <by or -> | <at>`; an item `upstream: <seq> | <kind> <item> | record <record or -> | on <on> | <by or -> | <at>`; then `upstream: more <k>` when `more` > 0. `null` prints `upstream: -`. Values pass `one_line`; among the card's first lines, the cut never reaches them.

A list row (`--marks`) gains ` | upstream` after ` | mark <m>` when its flag is `true`:

```text
C-0002 | copy/backlog | 1 | Write the tide table foreword | copy | - | - | TIDE-SPRING | mandate binding | mark open | upstream
```

### MCP, daemon, hub, plugin

- **Mirror** (`mirror.rs`, all `deny_unknown_fields`):
  - `CardShowDocument.upstream: Option<CardUpstreamShape>` after `correction`, and `CardRowShape.upstream: Option<bool>` after `correction`;
  - `CardUpstreamShape {parent, mark, on, correction, changes, more}`, with `mark` a nullable enum of `suspect` and `correcting`;
  - `CardUpstreamChangeShape {seq, at, source, by, fields, item, kind, record, on}`: `source` the enum `edit`, `item`; `fields` items the enum `goal`, `targets`, `criteria`; `kind` the nullable enum `question`, `discrepancy`, `decision`.
- **Texts**, one each, shared by stdio and a hub, ASCII:
  - `GET_DESCRIPTION`: `card_id: C- and 4 or more digits, as spec card list writes it. ` becomes `card_id: as spec card list writes it. `, and after `…in one read. ` (subtask_items) goes `upstream: what changed above an open subtask, as list_cards names it, or null. `. That makes 2 041 B of 2 048.
  - `LIST_DESCRIPTION`: after `a mark changes nothing.` goes ` upstream, with marks: true when, since it was made, its parent's goal, criteria or targets were edited or a question, discrepancy or decision on their nodes was applied, or when the parent reads suspect or correcting; false on a closed card or one with no parent; an edit of its own goal, criteria or targets answers every change before it; null without marks; get_card lists what changed.` That makes 1 588 B.
- **The daemon:** `GET …/cards/:id` builds `CardShowRequest {upstream: true, marks: false, history: true, context: false}`; `GET …/cards?marks=true` rows carry the flag, no parameter added. An index that cannot be read is a list's 503, now on the card route too, for an open subtask only. Keys: `Card` 36, `CardRow` 23; new sets `CardUpstream` (6) and `CardUpstreamChange` (9, in "JSON"'s order); `fixtures/daemon-keys.json` 126 sets, node-history's 124 plus 2.
- **The hub:** `/mcp/<slug>` `get_card`, `list_cards` and the daemon routes are the same code, over its own queue and its checkout's index in the project's turn. `HUB_TOOLS` (18), `INSTRUCTIONS`, `HUB_INSTRUCTIONS` unchanged; it takes the key at its next upgrade, the owner's call.
- **The plugin:** `plugin.json` and `scripts/deploy/.env.example` 0.41.0; `PINS` and `SCHEMA_PINS` (`plugin_files.rs`) append 0.41.0, the two outputs' schemas changed; `read-spec` unchanged (4 091 of 4 096 B).

### The UI line

- **Types** (`provisional.ts`), citing US-12: `CardRow.upstream: boolean | null`, `Card.upstream: CardUpstream | null`, each after `correction`; `CardUpstream {parent: string; mark: KnownUpstreamMark | Unlisted | null; on: string | null; correction: string | null; changes: CardUpstreamChange[]; more: number}`; `CardUpstreamChange {seq: number; at: string; source: KnownUpstreamSource | Unlisted; by: string | null; fields: string[]; item: string | null; kind: string | null; record: string | null; on: string | null}`. `Card`'s comment: `mark` still `null` on the card route, `upstream` served there. The mock: rows `true` on C-0013, `false` on the rest under marks, `null` without; C-0013's card a mark, an edit and a decision, C-0019's 16 changes with `more: 4`, the rest `null`.
- **Tile** (`BoardCard.tsx`): `row.upstream === true` adds a `Badge` of `labels.ts` `upstreamLook(t)` after the mark (first with none), before the accepted, cancelled and overdue states: label `board.upstream.label`; tone `card-upstream` (`.tone-card-upstream {--badge-color: var(--card-upstream)}`, `--card-upstream` in `tokens.css` onto the `information` lozenge's color, both themes); icon `cardUpstream`, new, hand-drawn at 16 px, a bar with an arrow down from it.
- **The part** (`src/board/CardUpstream.tsx` `UpstreamPart({project, upstream})`, in `CardPanel.tsx`'s Main right after `BriefMark`, only when `card.upstream !== null`; `CardPanel.tsx`'s `Part` titled `board.upstream.label`), a `ul`:
  - the parent's mark: `board.upstream.mark` or `.markCorrecting` (`{{on}}`, `{{correction}}`); an unknown word raw under `lang="en"`;
  - each change, oldest first: an edit `board.upstream.edited`, `{{fields}}` the words `board.upstream.field.{goal,targets,criteria}` joined by `Intl.ListFormat` in the viewer's language; an item `board.upstream.{question,discrepancy,decision}`, the `{{id}}` slot the record as a `DocLink`, else the item linking `sectionHash(p, "inbox", item)`, `{{nodes}}` `on`; an unknown source, kind or field raw under `lang="en"`. Then a `<time>` (`src/ui/time.ts` `formatUtc`) and, with `by`, `board.upstream.by`, `by` in a `bdi` (`src/i18n/fill.ts`);
  - `more` > 0: `board.upstream.more` (plural). Only `labels.ts` compares a source, kind, field or mark word (`policy.test.ts`); no `seq` is drawn.
- **Strings** (`en.ts`; `ru.ts` a draft with Russian plural forms), 13 English keys:

| Key | English |
|---|---|
| `board.upstream.label` | Changed above |
| `board.upstream.mark` | The parent's document changed at {{on}}. |
| `board.upstream.markCorrecting` | The parent's document changed at {{on}}; {{correction}} is putting it right. |
| `board.upstream.edited` | The parent's {{fields}} changed |
| `board.upstream.field.goal`, `.targets`, `.criteria` | goal; source; criteria |
| `board.upstream.question`, `.discrepancy`, `.decision` | Question {{id}} answered on {{nodes}}; Discrepancy {{id}} settled on {{nodes}}; Decision {{id}} applied on {{nodes}} |
| `board.upstream.by` | by {{name}} |
| `board.upstream.more_one`, `_other` | {{count}} earlier change not listed; {{count}} earlier changes not listed |

- **CSS:** only scale-step tokens (`--space-025`..`--space-600`, `--font-size-<step>`, `--line-height-<step>`, `--radius-*`), never the legacy `--space-1..6`, `--font-size-s|m|l|xl`, `--line-height-tight` that `design-sweep` removes (`docs/canon/ui-design.md` "Type, space and size").
- **Live:** a parent's edit is a `card.*` event, re-reading the list and the open card; `proposal.applied` re-reads both already (`docs/canon/card-tasks.md` "Screen"); a hand edit raises none, as for marks.

## Rules and edge cases

- WHEN a card has no `parent`, or is accepted or cancelled, the read SHALL give `upstream: null` (a row `false`) and read no event, item or index for it.
- WHEN a parent's `card.edited` holds no `goal`, `targets` or `criteria`, it SHALL NOT count; nor SHALL `card.moved`, `.assigned`, `.reestimated`, `.refreshed`, `.cleared` or `.cancelled`. The child's other edits end nothing.
- WHEN an item is open, staged, approved but not applied, or rejected, or is an `update`, `create`, `interpretation`, `amendment` or `drift`, it SHALL NOT count: an applied update's meaning change reaches the child through the parent's mark. An item naming the parent or a sibling counts by its nodes.
- WHEN the parent reads `departed`, `open` or `accepted`, the mark source SHALL say nothing; a cancelled parent has no mark, its edits before the cancel still count.
- `seq` alone orders: `at`, `created_at`, `updated_at` order nothing. One corpus and queue state gives one text and JSON at any root and `HOME`; `at` is stored data, no clock read (`docs/canon/mcp-read.md` "Tools", Parity). Writes: the index's own refresh only.
- Grandparents are not read: a grandparent's correction reaches a grandchild once the parent's own goal, targets or criteria are edited.

**Rollout.** An older `specengine-mcp` or daemon serves no `upstream`. There is no schema step, backup format, config key, route or tool. The hub takes it at its next upgrade.

**Test churn.** Every `card show` text golden gains `upstream: -` after `correction:`; every `card show --json` pin, `Card` key list and list row pin gains `upstream` (CLI `cards.rs`, `card_*.rs`, `card_context_before.json` if compared whole; MCP `mcp_cards.rs`, `card_context.rs`, `mcp_hub.rs`; http `cards.rs`, `card_*.rs`, `hub_mcp.rs`, `daemon_keys.rs`; UI builders, the mock). `plugin_files.rs`, `plugin_roles.rs`, `spec_ids_hub.rs` take the version; the descriptions' byte pins move.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turned red.

Rows are written through the store with fixed `now`s, applied items through approve and completion with a record, as the store's tests do; corpora are built at run time with invented names; "the scenario" is "Data"'s.

- [ ] US-1 — **By `seq`, never by clocks.** C-0001 made at `now` T5; a split makes C-0002, C-0003 at T9; the parent's goal edited at T5; then C-0004 made with `--parent C-0001` at T1. C-0002 and C-0003 each list the one edit (`seq`, `fields: ["goal"]`, `by`, `at` T5); C-0004's `upstream` is `null`. Mutation, red: `at` compared instead of `seq` (C-0002, C-0003 lose it, C-0004 gains it).
- [ ] US-2 — **Goal, targets, criteria only.** On the parent, nothing signals for `--title`, `--due`, `--estimate --reason` (`card.reestimated`), `--evidence`, `--task`, `assign`, a move, a reorder, a mandate edit by its reviewer; then `edit --title X --goal Y` lists one change, `fields: ["goal"]`, and `--target` `["targets"]`. Mutations, each red: any `card.edited` counted; `fields` unfiltered.
- [ ] US-3 — **Applied items on the reach.** The scenario plus `docs/harbor.md` `HARBOR-LIGHTS`: C-0002 lists PR-0040 (`kind` decision, `record` DEC-0031, `on` TIDE-NEAP, `by` "Ann Owner") and none of: PR-0041, a question on TIDE-SPRING applied before the split; PR-0042 open, PR-0043 rejected, PR-0044 staged, each a decision on TIDE-SPRING; PR-0045, a decision on HARBOR-LIGHTS, applied; PR-0046, a decision on TIDE-SPRING with `card_id` C-0002, applied. Mutations, each red: open items counted; the reach without `within` (PR-0040 lost); the child's own items counted; the applied `seq` ignored (PR-0041 listed).
- [ ] US-4 — **Marks untouched.** Over the scenario before seq 4 and after seq 9 (no hand edit), byte-equal: `spec trace` and `spec trace TIDE-BOOK`, text and `--json`; every `card list --marks --json` row's `mark`, `on`, `discrepancy`, `correction`; every card's `allowed` and `mark`; on the mock, Ready to accept. Mutation, red: the signal folded into `card_mark` (C-0002 reads `suspect`).
- [ ] US-5 — **Uncapped, then capped at 16.** After the parent's goal edit, 70 reorders of the parent and 70 of C-0002: the edit still listed. Store `events_of_types` over 200 mixed events returns exactly the asked types in `seq` order, an unparsable payload passed over. Twenty qualifying parent edits list the newest 16, `more: 4`, the text ending `upstream: more 4`. Mutations, each red: `card_history(…, 64)`; no cap; the oldest kept.
- [ ] US-6 — **How it ends.** (a) C-0002's own `--goal` edit after the parent's: `null`; a later parent `--criteria` edit lists that one alone. (b) Its own `--due` edit ends nothing. (c) An accepted and a cancelled subtask: `null`, rows `false`. (d) C-0005 made with no parent, the parent edited, then `edit C-0005 --parent C-0001`: nothing; the parent edited again: that edit. Mutations, each red: the child's edits ignored; any child edit ending it; the anchor from `card.created` alone.
- [ ] US-7 — **The parent's mark.** A hand edit of TIDE-NEAP: C-0001 reads `suspect TIDE-BOOK`, C-0002's own mark `open`, its `upstream` `{parent: "C-0001", mark: "suspect", on: "TIDE-BOOK", correction: null, changes: [], more: 0}`, its row `true`. Its own goal edit leaves that; `spec card clear C-0001 --no-effect` gives `null`, `false`. A live correction holding the parent: `correcting` with its `correction`; a parent reading `departed`: no mark source. Mutations, each red: the mark source dropped; `departed` counted; the mark ended by the child's edit.
- [ ] US-8 — **Text and JSON as pinned.** On the scenario, `spec card show C-0002 --marks` prints "Text"'s three lines byte for byte after `correction:`, `--json` "JSON"'s object; without `--marks`, `upstream: -` and `null`. `spec card list --marks` prints C-0002's row as pinned; JSON rows `true` for C-0002, C-0003, `false` for C-0001, `null` without `--marks`. A parent row made unreadable in SQL: `show` exits 0, `null` and the note; the row `false`. Mutation, red: `upstream` read without `--marks`.
- [ ] US-9 — **One read per call.** A test seam counts `events_of_types` calls, applied-item reads and index refreshes: `card list --marks` over 40 cards, 30 open subtasks: 1, 1, 1; `card show --marks` of an open subtask: 1, 1, 1; the daemon's card route of a card with no parent: 0, 0, 0. Mutation, red: a scan per card.
- [ ] US-10 — **MCP parity, schemas, hub, size.** Stdio `get_card {card_id: "C-0002"}` content is byte-equal to `spec card show C-0002 --marks 2>&1`, `structuredContent` to `--json`; `list_cards {marks: true}` likewise against `spec card list --marks`. The schema walker passes with exact key sets (`Card` 36, `CardRow` 23, `CardUpstream` 6, `CardUpstreamChange` 9) and the four enums. A hub's `/mcp/<slug>` answers alike, 18 tools. `mcp_size`'s `get_card` maximum gains a 16-change `upstream` with the longest ID the store takes, pinned, ≤ 500 000. Mutations, each red: `on` dropped from `CardUpstreamChangeShape`; `upstream` dropped from `CardRowShape`.
- [ ] US-11 — **Plugin 0.41.0, texts.** `plugin.json` and `.env.example` hold 0.41.0, `tag-check.sh v0.41.0` prints it, `PINS` and `SCHEMA_PINS` end with 0.41.0. `GET_DESCRIPTION` 2 041 B and `LIST_DESCRIPTION` 1 588 B, each its sentence once, as "Data" gives it; `INSTRUCTIONS`, `HUB_INSTRUCTIONS` and `read-spec` byte-equal to before. Mutation, red: the schema changed with no `SCHEMA_PINS` entry.
- [ ] US-12 — **The daemon serves it, the UI types it.** `GET …/cards/C-0002` serves US-8's `upstream`, `mark` `null`; `GET …/cards?marks=true` rows carry the flag, `null` without. The index unreadable: the card route answers 200 for a card with no parent (`upstream: null`), 503 for an open subtask. The fixture names 126 sets, `Card` 36 keys, `CardRow` 23; `provisional.ts` types the shapes; UI lint 0 problems, `tsc` clean, `pnpm test` green with `daemonKeys.test.ts`, `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: the card route not asking `upstream`; the index read before the applicability check; `upstream` out of the fixture.
- [ ] US-13 — **The tile and the part**, on the mock. C-0013's tile shows "Changed above" with `cardUpstream` after its mark, other rows none. C-0013's work item shows the part right after the brief's mark: the mark line, "The parent's goal changed", the decision with DEC-0031 a `DocLink`, each with its `<time>` and `bdi` name; C-0019's ends "4 earlier changes not listed". An unknown source, kind, field or mark word is raw under `lang="en"`; `upstream: null` draws no part. The 13 keys in `en.ts`, the Russian forms in `ru.ts`; `policy.test.ts` and `scale.test.ts` pass, the slice's CSS lines naming no legacy alias (`git diff` grep). After `pnpm build`, first-load JS grows at most 1 kB, ≤ 845 kB. Mutations, each red: the part drawn for `null`; a key missing in `ru.ts`.
- [ ] US-14 — **[measure] The list at scale.** Seeded through the store: 500 parents with 4 subtasks each, 40 000 events, 2 000 applied decisions over a 400-node corpus (`specengine-eval scale --corpus-only`). `spec card list --marks --json`, warm, debug and release, the upstream half timed by US-9's seam. Bar: release ≤ 1 s, times in "Implementation"; past it, "Known limits" names a per-read memo (an `events(project, type)` index is a queue schema step; slice 8 holds the next).
- [ ] US-15 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800: a split's two subtasks after the parent's goal edit, tiles and work items; a tile with both Suspect and Changed above; C-0019's 16 lines and the more line; the parent cleared, the badge gone after the re-read.
- [ ] US-16 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli`, `-mcp` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

An acknowledge act (Q4); grandparents, siblings' edits, a correction card (`corrects`) as sources; the flag on Subtasks rows, Parent lane heads, a filter, a count or a notification; items filtered by repository (a card read runs no git); a stored or cached signal, an events index.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{upstream,lib}.rs`; `crates/specengine-store/src/queue/cards.rs` (`events_of_types`); `crates/specengine-cli/src/{card,trace,main,lib}.rs` and a new `upstream.rs`; `crates/specengine-mcp/src/{cards,mirror}.rs`; `crates/specengine-http/src/app.rs`; `plugin.json`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `upstream_signal.rs` in CLI, MCP, http (new); store `queue_cards.rs`; core `#[cfg(test)]`; US-9's seam; `mcp_size.rs`; the churn; plugin tests; `daemon_keys.rs`, `spec_ids_hub.rs`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`) with, in the same change, `daemonKeys.test.ts`'s two `KEYS` sets, names, count 126, the two `upstream` entries out of `AHEAD_OF_FIXTURE`; US-14 |
| ui-developer | "The UI line": `provisional.ts`, the mock, `src/test/builders.ts`, `src/board/{BoardCard.tsx,CardPanel.tsx,CardUpstream.tsx,labels.ts}`, `src/ui/Icon.tsx`, `src/styles/{app,tokens}.css`, `en.ts`, `ru.ts`; US-13. `["CardRow", "upstream"]`, `["Card", "upstream"]` in `AHEAD_OF_FIXTURE` until the fixture lands |
| test-engineer, UI | the UI gates, US-13's mutations, US-4's Ready to accept, the first-load figure |

Comments cite `docs/features/upstream-signal.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after `node-history` ships: both edit CLI `lib.rs`, `main.rs`, MCP `mirror.rs`, http `app.rs`, the plugin version, the fixture, `provisional.ts`, the catalogs, `app.css`, and this slice uses its `identity_name`. The version stays 0.41.0.
2. The ui-developer lands the UI line behind `AHEAD_OF_FIXTURE`, gates green; the rust-developer goes core, store, CLI, MCP, http.
3. The Rust test-engineer overlaps: core (US-1, US-2, US-6 at the judge), store (US-5), CLI (US-3, US-4, US-7 to US-9), MCP and http (US-10 to US-12), US-14. Then the Rust and UI reviews.
4. The orchestrator's look (US-15). Ship: the spec-writer ("Canon at ship").

`design-sweep` runs in its own worktree; both edit `app.css` and the catalogs, so commits split by hunk. At most three iterations a lane.

### Risks

- **The card route's new 503**: it never read the index before; US-12 pins that only an open subtask pays.
- **Description room**: `GET_DESCRIPTION` keeps 7 B; a later key on `get_card` trims first.
- **A stuck mark line** until someone clears the parent (W2).

### Canon at ship

Free bytes now in brackets; "net" means trimmed in the same file to fit.

| File | Edit |
|---|---|
| `docs/canon/card-upstream.md` (new, Tier 2, `card*` shard) | the rule: sources, reach, the anchor, the end, the cap, JSON and text, MCP, daemon, hub, screen; "Known limits" (a stuck mark line, current targets' reach, every repository's items, the card route's 503, the measured cost); "Tests" |
| `docs/canon/cards.md` [73], `docs/canon/spec-trace.md` [66] | "Documents and events": `upstream` in the key list, a row 23 keys, the last five `--marks`'; "The read": rows carry `upstream`, no mark changed. Net |
| `docs/canon/board-screen.md` [64], `docs/canon/ui-design.md` [15], `docs/canon/board-ui.md` | the tile's badge; the work item's Main gains Changed above; the card route asks `upstream`. Net |
| `docs/canon/card-context.md`, `docs/canon/plugin-files.md`, `docs/canon/hub-deploy.md` [12], `docs/canon/plugin.md` [316], `docs/canon/ui-languages.md` [9] | `GET_DESCRIPTION` 2 041 B; version 0.41.0; the first-load figure. Net |
| READMEs: core [9], store [3], cli [1], mcp [7], http [4] | a pointer to `card-upstream.md`; store's `events_of_types`. Net |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 3; RM-0105 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q1. Fold the signal into `mark`? | No, its own key | `spec trace`'s rollup and `allowed` change; a rollup reads items, against `card-questions.md` "The rule" |
| Q2. What counts, ordered how? | Parent edits of goal, criteria or targets, applied deciding items on the nodes, the parent's mark; by `seq` | Clocks of different machines misorder writes, and two writes in one second tie |
| Q3. How far? | The direct parent, open subtasks, the store the read runs in | Grandparents multiply reads; closed cards would flag history |
| Q4. When does it end? | Accept, cancel, or a flow-down edit of the subtask's goal, criteria or targets | An acknowledge act: a hub's eighth plain write, an ADR supplementing ADR-0074, 1.5-2 hours |
| Q5. Show "the parent's document changed"? | Yes, worded as the parent's mark | A kind-targeted subtask misses the main case of a correction at the top |

**The spec-writer's:**
- W1: the child's own applied items do not count: its department's decision is no news from above, and `items` lists it.
- W2: the mark line has no `seq`, so no flow-down edit answers it; it stands until the parent is cleared or its correction accepted.
- W3: re-parenting moves the anchor (US-6 d). W4: `by` is a name, never an email. W5: the card route asks `upstream` without `marks`, its `mark` staying `null`. W6: an item names its first meeting node, `on`, bounding an entry for `mcp_size`. W7: the reach is both cards' current targets.
