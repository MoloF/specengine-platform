---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin]
ref: RM-0047; Phase 7 step 6 (08 s2)
adrs: [ADR-0056]
---

# card-trace: suspect marks, clearing, verdicts, rollup (RM-0047)

## Why

A changed brief must reach the work done against it, and accepted work must confirm the brief, with nothing written into the file and nothing stopped. Every state here is a join at read; the only writes are a person's, plus one an editorial apply needs.

## Description and interactions

### Snapshot and suspect

`new`, `accept` and `edit --target` freeze each direct target in `snapshot`. A card is **suspect** on a target whose `norm_hash` now is neither its entry's nor in its `also`, or which no longer resolves: per read, over the reading root's index, never stored, direct targets only. A `null` hash, or a card with no snapshot (made before this slice): not suspect, a note.

Whitespace leaves `norm_hash` equal. **`--editorial` does not**: it changes text, keeping only the revision. So the transaction recording `applied` adds each changed node's new hash to `also` of every entry holding its old one, one `card.refreshed` each, as `applied_refreshing` does for tasks; a completion reads the commit's `Revision: <ID> <rev> (editorial)` trailers. `also`, not a replace, so a worktree without that commit stays clean. A hand edit rightly marks: nobody vouched for it.

### Rollup

A node's cards: a target in `SpecGraph::within(node)`, cancelled ones ignored. Per read, never stored; first match:

| State | When |
|---|---|
| `no card` | none |
| `departed` | a card departs from it, its discrepancy not `applied` or `rejected` |
| `suspect` | a card is suspect on it, with no live correction |
| `in work` | a card is not accepted |
| `confirmed` | else: at least one accepted, none suspect or departed |

### Accepting

```text
spec card accept C --as N (--briefed | --departs REF... --summary T --observed T --documented T
  [--gap-type G]) [--note T] [--revision R]
```

One of the two, else exit 2; the terminal as before. Either re-freezes the snapshot; accepting a correction also re-freezes what it corrects. `--departs` names 1-16 direct targets, a manual card having none, and makes the discrepancy:

- `node_ids`, `summary`: `--departs`, `--summary`; `gap_type`: `--gap-type` or `contradicts`; `severity`: `normal`.
- `evidence`: `[{file: "C-0001", observed, documented}]`.
- `options`: core's two, change the brief, rework the result, "not priced at acceptance"; `recommendation` 0.
- `distinct_from`: the hits, named by the accept itself.

The store's `accept_departing` runs `create_intake`'s body inside the accept's transaction, as `reject_with_task` composes: events `proposal.created`, `card.accepted`.

**The dedup never refuses a departure**, nothing being blocked by a discrepancy (ADR-0012). Hits are found under the write lock as ever; the accept names them itself in the item's `distinct_from`, the first 64 (`DISTINCT_MAX`), and lists them all in its notes. The person is never asked to type one, and the exit stays 0.

**Git.** The item binds a place as `propose update` step 5, so `--departs` reads git, reads only: the one exception to `cards.md`'s "no git" (exit 2 outside a worktree, nothing changed). A card path without `--departs` reads none.

The page's dialog gains the choice and these keys. `card_accept` makes the item, so `propose*` stays forbidden in `crates/specengine-http/src`; this slice widens `card_writes_touch_only_the_queue` to admit that one proposal (ADR-0057).

### Clearing

```text
spec card clear C --no-effect --reason T --as N [--revision R]
spec card clear C --needs-correction --reason T --estimate E --as N [--title T] [--revision R]
spec card clear --group REF --no-effect --reason T --as N
```

A person's act, on a terminal as `accept` (prompt `clear C-0001 (<title>) as <N>: no effect? [y/N]`, a group its count) or on the page; no MCP tool. Exit 1 on a card not suspect.

- **No effect** re-freezes each suspect entry. The next meaning change marks it again.
- **Needs correction**, accepted cards only: a new card with `corrects`, the targets, flow and discipline, `--estimate`, title `Correct <title>` by default. The original stays accepted, its snapshot untouched, its mark held: while the correction lives it reads `correcting C-0002`, and the node is `in work` through it. Accepted, it re-freezes the original. Cancelled, the original is suspect again.
- **Group**: every suspect card with a target in `within` of REF or of a document under it by `parent:`; one transaction, one event each, no effect only.

**On the page.** `POST .../cards/:id/clear` `{outcome, reason, estimate, title, revision, as}` and `POST .../trace/clear` `{group, reason, as}` (`card_clear`, `card_clear_group`, joining `CARD_CALLS`) stand behind `board-ui.md`'s door, the `Consent` the dialog's: a Clear dialog in the card panel, the name from "Acting as". As the page's accept, this is a convention until the hub's named login: the name is typed, nothing verifies it, any local process can post here (ADR-0034). No MCP tool reaches them.

### Re-estimating

`edit --estimate E --reason T` on a card holding an estimate: `--reason` required (exit 1), no other flag beside it (exit 2), `card.reestimated`. A first estimate stays `card.edited`. `edit_card` gains `reason?`.

### `spec trace [REF]`

```text
trace <REF or all> | nodes <n>, cards <c> | no card <a>, in work <b>, suspect <s>, departed <d>, confirmed <f>
<state> | <name> | <title> | <path> | cards <c>
  card <id> | <flow>/<column> | <mark> | <title>
  uses <name> | <state> | cards <c>
```

Rows only. Scope: REF's holders and what they hold by `parent:`; no REF: every live document and every section a card targets. Marks `open`, `accepted`, `suspect <ID>`, `departed <PR>`, `correcting <C>`. Exit 0 whatever is listed, 1 only a REF naming nothing, 2 as `spec flow`; cap as `spec flow`'s. It changes no status, finding, claim or gate. The queue read as `spec roadmap`'s (`open_existing`, no schema step); no queue: `no card`.

MCP `get_trace {ref?}`, a read. Daemon `GET .../trace?ref=`, browser view. `INSTRUCTIONS`, 1 B left: adds `; get_trace(ref?) = trace` to the reads line, cuts `; evidence computed, never typed`: 2 040 B.

### `spec impact`, and a changed definition

`spec impact` adds `cards`: cards not cancelled with a target in R, by number: `card <id> | <flow>/<column> | <mark> | <title> | <names>`, cut after tasks, `truncated.cards`. Nodes, tasks and the exit do not change.

`uses` carries no mark. B uses D; D changes: D's cards are suspect, and B's hash holds, so B's stay clean, avoiding floods. `spec trace D` lists incoming `uses`, one level, as consequences; `spec impact D` the whole reach. A definition with no card marks nothing, so a consequence needs its REF, or a change record.

### What `CR-` needs (RM-0056)

Consequences: `cards` in `spec impact --json`, `spec trace --json`. Events carry no commit, so `card.cleared` names (node, `rev`), which `Revision:` trailers join to one. The estimate delta: `card.reestimated`'s `old`, `new`, plus each correction's `estimate`.

## Data

```json
{"snapshot": [{"id": "SHT-FRONT", "rev": 2, "norm_hash": "b3:…", "also": []}],
 "acceptance": {"by": "desk", "at": "…", "note": null, "verdict": "departs",
  "departs": ["SHT-FRONT"], "discrepancy": "PR-0007"}}
```

No schema step: `snapshot`, `acceptance` hold JSON already; a step-4 row without `verdict` reads `briefed`. `also` keeps the last 16.

Events: `card.accepted` gains `verdict`, `discrepancy`; `card.cleared {id, outcome, nodes: [{id, rev, from, to}], correction, reason, as}`, `outcome` `no_effect`, `needs_correction`, `corrected`; `card.reestimated {id, old, new, reason, as}`; `card.refreshed {id, node, proposal}`. `QUEUE_EVENT_TYPES` 25. Core `trace.rs`, pure: `card_mark`, `rollup`. Store: `accept_departing`, `clear_card`, `list_existing_cards`, and `node_norms`, the first read of `nodes.norm_hash`.

`spec trace --json`, `{ref, reason, notes, counts, nodes, truncated, omitted}`: a node `{id, title, path, rollup, counts, cards: [{id, flow, column, title, mark, on, discrepancy, correction}], uses: [{id, rollup, cards}]}`. `counts` `{no_card, in_work, suspect, departed, confirmed}` over the node and what it holds by `parent:`.

`fixtures/card-press`' five documents under `PRN-WEEKLY` are the brief; it gains a `uses` from `SHT-BACK` to `SHT-FRONT-HEAD` and an accepted decision linked to `SHT-FRONT`.

## Rules and edge cases

- Nothing here stops anything (ADR-0012): a suspect card moves and is accepted as any other; an open discrepancy holds no card.
- Marks read the reading root: a branch editing a target marks its cards there alone.
- A corrupt card row: skipped, a note. An unreadable discrepancy reads unsettled, a note.

## Acceptance criteria

- [ ] AC-01 — On `fixtures/card-press`, approving a meaning change makes a card suspect; whitespace does not; a clear with a reason removes it, the next meaning change restores it. M: comparing `span_hash` turns the whitespace case red.
- [ ] AC-02 — `spec approve --editorial`: not suspect, one `card.refreshed`, also from a worktree without the commit. A hand edit: suspect. M: the apply refreshing nothing.
- [ ] AC-03 — `SHT-FRONT` is `confirmed` only with a card, all accepted or cancelled, one accepted, none suspect or departed; all cancelled reads `no card`; the file and `git status` stay as they were. M: confirming with no card.
- [ ] AC-04 — `--departs`: one `open` discrepancy, the card accepted, the node `departed`; `spec reject` of it gives `confirmed`. A failure injected after the insert leaves neither. M: the insert in its own transaction.
- [ ] AC-05 — The linked decision is a hit: the departing accept exits 0 with no flag, the item's `distinct_from` names it, a note lists it. M: the dedup's refusal kept.
- [ ] AC-06 — Needs correction: the original stays accepted, the node `in work`; the correction accepted gives `confirmed`; cancelled, `suspect`. M: re-freezing at the clear.
- [ ] AC-07 — `clear --group PRN-WEEKLY`: one event per card, no card outside it. M: a bulk clear ignoring `parent:`.
- [ ] AC-08 — `SHT-FRONT-HEAD` changes: its card suspect, `SHT-BACK`'s not, listed under `uses`. M: marking through `uses`.
- [ ] AC-09 — `spec trace`, `get_trace`, `GET trace` agree, exit 0 on suspect and departed rows; `spec check`, `spec gate` unchanged; `spec impact` lists the cards; a re-estimate without a reason exits 1. Piped `clear` exits 2; `--briefed` runs at a root with no `.git`.
- [ ] AC-10 — From a same-origin page a clear of each outcome and a group clear finish, the typed name in the event; no `Sec-Fetch-Site`: 403; `tools/list` holds no clear.

## Out of scope

`CR-` itself. Flow badges, timeline, a node's cards and the board tile's mark: RM-0048. Marks through `uses`, or past direct targets. The warning on a move to `ready` is dropped: no column word exists in code.

## Files in scope

- **New:** core `src/trace.rs`; CLI `src/trace.rs`, `tests/card_trace.rs`; store `tests/queue_trace.rs`; MCP `tests/mcp_trace.rs`; http `tests/trace.rs`; the fixture's `uses` and decision.
- **Changed:** core `card.rs`, `lib.rs`; store `queue.rs`, `queue/cards.rs`, `read.rs`, `lib.rs`; CLI `card.rs`, `intake.rs`, `impact.rs`, `apply.rs`, `revisions.rs`, `main.rs`, `lib.rs`; MCP `read.rs`, `cards.rs`, `mirror.rs`, `server.rs`; http `app.rs`; `plugin.json`, next MINOR; UI `src/board/{CardPanel,CardDialogs}.tsx`, `labels.ts`, `api/{provisional,client,http,queries}.ts`, `http.test.ts`.
- **Pins:** `INSTRUCTIONS`, `TOOLS` 29, `SCHEMA_PINS`, `door.rs`, `fence.rs`, `methods.rs`, `daemon-keys.json`, `mcp_door.rs`, genre scans.
- **Canon at shipping:** new `card-trace.md`; `cards.md`, `spec-impact.md`, `node-revision.md`, `proposal-revisions.md`, `mcp-read.md`, the http README.

## Implementation
