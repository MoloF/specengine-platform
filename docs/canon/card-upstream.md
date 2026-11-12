---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Card upstream: what changed above an open subtask

Built by `upstream-signal` (RM-0105, plugin 0.41.0), no ADR: it rests on ADR-0074 (no card write touches another), ADR-0012 (nothing is blocked) and ADR-0077 "The rule" (no rollup reads an item). Related: `cards.md`, `spec-trace.md` (marks), `card-split.md`, `card-progress.md`. Code: core `upstream.rs`; store `queue/cards.rs` (`events_of_types`); CLI `upstream.rs`, `card.rs`, `trace.rs` (`fresh_norms_reach`); MCP `cards.rs`, `mirror.rs`; http `app.rs`; UI `src/board/{CardUpstream.tsx,BoardCard.tsx,labels.ts}`, `src/api/queries.ts`.

## The rule

An open subtask carries `upstream`: what changed above it since it was made, computed per read from the queue's events and the index, never stored. It is its own key, never inside `mark`: `card_mark`, `spec trace`, `allowed`, Ready to accept, every move, accept, clear, gate and check read none of it; it moves, refuses and gates nothing.

- **Three sources, the direct parent only.** (1) The parent's `card.edited` holding `goal`, `targets` or `criteria` (`UPSTREAM_FIELDS`); no other edit or event counts. (2) Applied questions, discrepancies and decisions of the queue the read runs in, any repository's (a card read runs no git), one of whose `node_ids` meets the reach; the child's own items aside (its department's decision is no news from above; `items` lists it); one naming the parent or a sibling counts by its nodes. Open, staged, approved but unapplied or rejected items, and every `update`, `create`, `interpretation`, `amendment`, `drift`, never: an applied update reaches the child through the parent's mark. (3) The parent's mark when `suspect` or `correcting`, worded as the parent's, claiming no order against the child; `departed`, `open`, `accepted` say nothing; a cancelled parent has none, its edits before the cancel still count.
- **The reach**: the child's and the parent's current targets with their nested ID sections (`SpecGraph::within`, the mark's own reach), an ID-less document by its path, a gone target meeting only its stored text.
- **By `seq`, never clocks.** The anchor is the child's latest `card.created` or `card.edited` holding `parent`, so a re-parent moves it. The floor is the anchor raised by the child's own edit of goal, targets or criteria, which answers every change before it (it flowed down); its other edits end nothing. Accepted or cancelled: `null`. The mark line has no `seq`: it follows the parent's mark, ending when the parent is cleared or its correction accepted (`spec-trace.md` "Clearing"). No acknowledge act.
- **The cap**: at most `UPSTREAM_MAX` 16 changes, the newest, listed oldest first; `more` counts the rest.
- Grandparents are not read: a grandparent's correction reaches a grandchild once the parent's own goal, targets or criteria are edited.

## The judge

Core `upstream.rs`, pure: `upstream(child, parent, parent_mark, log: &[Logged], decided: &[Decided], reach) -> Option<Upstream>`, `anchor_of(child, log)`. In order: a closed child `None`; the anchor, none judging the mark alone; the floor; the parent's edits past it, `fields` the counted ones in the event's order, `by` its `as`; items not the child's, last `Applied` past the floor, meeting the reach, `on` the first meeting node in the item's order, `seq` and `at` the `Applied`'s, `by` `identity_name(decided_by)` (a name, never an email); by `seq`, the oldest past 16 into `more`; `None` with no mark and no change. Types `Upstream {parent, mark, changes, more}`, `UpstreamChange`, `UpstreamSource {Edit, Item}`, `LoggedType {Created, Edited, Applied}`.

## Reads

It rides `--marks`, since the reach and the parent's mark need the index: `card show --marks`, `card list --marks`, `get_card` (always), `list_cards {marks: true}`, the daemon's list with `marks=true`. The daemon's card route asks it alone (`CardShowRequest {upstream: true, marks: false, history: true, context: false}`), its `mark` staying `null`. No parent or a closed card: nothing read for it.

- **One read per call** (CLI `upstream.rs`): store `events_of_types(&[card.created, card.edited, proposal.applied])`, one uncapped statement, oldest first by `seq`, a payload that is no JSON passed over (never the 64-capped `card_history`); one `list_readable` of applied items, kept to question, discrepancy and decision; the reach from the marks' own index refresh (`fresh_norms_reach`, one `indexed_in`). No open subtask: none of them. `Judge` groups the facts once and passes core, per child, only the child's and parent's events and the items meeting the reach, with their applies; core re-checks all, so the result is the whole log's.
- **Library**: `CardShowRequest.upstream`, no `Default` (CLI from `--marks`; MCP and the card route `true`); `CardShowOutcome.upstream: Option<Upstream>`; `CardListOutcome.upstream`, the open subtasks judged non-`None`, and `marked`, whether `--marks` was asked (a row's `null` against `false`).
- **A list** finds each subtask's parent among every readable card, listed or filtered out, marks the unlisted ones from the same refresh, and reads the index when only parents have targets.
- **Notes**, `show` only, one line each (escaped on stderr, raw in JSON `notes`): `upstream: <C>: its parent <P> does not read; not judged` (`null`) when the parent is not found, does not decode or the queue errs; `upstream: <C>: no card.created event; only its parent's mark is read`; for the whole queue's applied rows that do not decode (a row cannot say which nodes it names), one a read (`unread_note`): ``upstream: <C>: proposal <PR>: the stored `<column>` cannot be read: <reason>; not judged``, for k ≥ 2 `upstream: <C>: <k> applied rows cannot be read, the first: proposal <PR>: …; not judged`, the lowest-numbered. A list adds none: an unreadable parent's row keeps its `card <P>: …; not listed`, the child's flag `false`; undecodable rows leave flags as judged. The parent's own mark notes are never the child's; the card route keeps the upstream notes alone.

## Text and JSON

- **The card**: `upstream` after `correction`, 36 keys; `null` unasked, with no parent, on a closed card or with nothing to say; else `{parent, mark, on, correction, changes, more}`, `mark`, `on`, `correction` the parent's mark (`suspect`, `correcting`; else all `null`). A change `{seq, at, source, by, fields, item, kind, record, on}`: `seq`, `at` its event's; `source` `edit` or `item`; `by` an edit's `as` or the decider's name, else `null`; `fields` an edit's counted fields, `[]` on an item; `item`, `kind` (`question`, `discrepancy`, `decision`), `record` (`record_id`), `on` an item's, `null` on an edit.
- **A list row**: `upstream` last, 23 keys; `true` or `false` under `--marks` (`false` closed or with no parent), `null` without. Write documents, a split child and `CardList` are unchanged.
- **Text**, after `correction:`: `upstream: -`, or the parent's line (its mark as a list row prints it, or `-`), each change (`-` for no `by` or `record`), then `upstream: more <k>` when `more` > 0; values pass `one_line`, never cut. A `--marks` row ends ` | upstream` after ` | mark <m>` when `true`.

```text
upstream: parent C-0001 | suspect TIDE-BOOK
upstream: 4 | edited goal | Ann | 2026-11-12T09:00:00Z
upstream: 9 | decision PR-0040 | record DEC-0031 | on TIDE-NEAP | Ann Owner | 2026-11-12T10:00:00Z
```

## MCP, daemon, hub, plugin

- **Mirror** (`deny_unknown_fields`): `CardShowDocument.upstream: Option<CardUpstreamShape>` (6 keys), `CardRowShape.upstream: Option<bool>`, `CardUpstreamChangeShape` (9); enums `CardUpstreamMark`, `CardUpstreamSource`, `CardUpstreamField`, `CardUpstreamKind`, the words above.
- **Texts**: `GET_DESCRIPTION` 2 041 of 2 048 B (`upstream: what changed above an open subtask, as list_cards names it, or null.`), so a later key on `get_card` trims first; `LIST_DESCRIPTION` 1 588 B, the flag's sentence after `a mark changes nothing.`; `INSTRUCTIONS`, `HUB_INSTRUCTIONS`, `read-spec` unchanged.
- **The daemon**: an index that cannot be read is the card route's 503 for an open subtask only, 200 for a card with no parent or a closed one; `GET …/cards?marks=true` rows carry the flag. Key sets `Card` 36, `CardRow` 23, `CardUpstream` 6, `CardUpstreamChange` 9 (`fixtures/daemon-keys.json`, 126).
- **The hub**: `/mcp/<slug>` `get_card`, `list_cards` and the routes are the same code over its queue and checkout, 18 tools; `get_card` and a marked list walk, the card route trusts (`spec-read-freshness.md` "Routes and tools"). A hub takes it at its next upgrade.
- **Plugin** 0.41.0, a MINOR (`plugin-files.md` "Changing it").

## Screen

- **Tile** (`BoardCard.tsx`): `row.upstream === true` adds a badge, `labels.ts` `upstreamLook(t)`, "Changed above", after the mark (first with none), before the accepted, cancelled and overdue states; tone `card-upstream` (`--card-upstream` the `information` lozenge's text colour), the hand-drawn icon `cardUpstream`, a bar with an arrow down; the tile's `aria-label` names it.
- **The part** (`CardUpstream.tsx` `UpstreamPart`), in the work item's Main right after the mark, titled `board.upstream.label`, when `card.upstream !== null`: the parent's mark line; each change oldest first, an edit "The parent's <fields> changed" (`Intl.ListFormat` in the viewer's language), an item "Decision <id> applied on <node>", the record a `DocLink`, else the item linking to the Inbox; a `<time>` (`formatUtc`), "by <name>" in a `bdi`; then the plural "<k> earlier changes not listed". No `seq` drawn; an unknown word raw under `lang="en"`; a null `on` or item `-`; a correcting mark with no correction "a correction". Only `labels.ts` compares a source, kind, field or mark word (`policy.test.ts`); 13 `board.upstream.*` keys.
- **Live** (`queries.ts` `useLiveQueue`): a `card.*` event reads again, besides the lists and that card, each cached card read naming it as `parent`; `proposal.applied` already reads both; a hand edit raises none, as for marks.

## Known limits

- **Trust on a hub.** The card route trusts the index; after a hand edit on the host its mark line may lag a reconcile, up to 300 s, while the walking marked list's row already flags it.
- **A list flag reads `false` when the parent's row does not read**: a tile cannot tell "could not judge"; the row's `not listed` and the card route's `not judged` notes say it.
- **The card route can answer 503 for an open subtask** when the index cannot be read (the work item's error state); it never read the index before.
- **Undecodable applied rows**: one note a read, the whole queue's count, not the reach's; none is judged, and a list's flag ignores them.
- **A parent that does not read**, a database error included, is the note, never the read's error.
- **The live re-read** runs on every `card.*` event of a parent, moves and reorders too: no loop, cheap, more reads than the signal needs.
- **A stuck mark line** until the parent is cleared or its correction accepted; the reach is current targets'; every repository's items count.
- **Cost**, per read, uncapped: 500 parents with 4 subtasks, 40 000 events, 2 000 applied decisions, 517 nodes: `card list --marks --json`, warm medians, release 85 ms, debug 542 ms (14 and 56 ms without parents). Past 1 s, a per-read memo; an `events(project, type)` index is a queue schema step.

## Tests

Core `tests/upstream.rs`; store `tests/queue_cards.rs`; CLI `tests/upstream_signal.rs`, `src/upstream_tests.rs` (one read per call), `tests/upstream_scene/` (shared by MCP and http), `tests/upstream_scale.rs` (the seeder, ignored); MCP, http `tests/upstream_signal.rs`, MCP `mcp_size.rs`, http `daemon_keys.rs`; UI `src/board/CardUpstream.test.tsx`, `labels.test.ts`, `mocks/upstream.smoke.test.tsx`, `daemonKeys.test.ts`, `policy.test.ts`. Criteria: `docs/features/upstream-signal.md`.
