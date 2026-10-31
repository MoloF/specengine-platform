---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-31
---

# spec trace: suspect marks, clearing, verdicts, rollup

ADR-0056, ADR-0057, built by `card-trace` (RM-0047), read by `trace-views` (RM-0048): a changed brief reaches the work done against it and accepted work confirms the brief, with nothing written into the file and nothing stopped (ADR-0012): a suspect card moves and is accepted as any other, an open discrepancy holds no card. Marks and rollups are computed per read over the reading root's index and never stored, so a branch editing a target marks its cards there alone. The record: `cards.md`; the page: `board-ui.md`; what a card records (snapshot, re-estimating, events): `spec-trace-record.md`. Code: core `trace.rs`, pure; store `queue/cards.rs`, `read.rs`; CLI `trace.rs`, `card.rs`.

## Suspect

A card is **suspect** on a direct target whose `norm_hash` now is neither its entry's nor in its `also`, or which no longer resolves. A `null` hash on either side, or a card with no snapshot: not suspect, a note (`mark_notes`). `TargetNorms`, one target's first node by (path, position), comes per read from `SqliteIndex::node_norms`.

Whitespace leaves `norm_hash` equal. **`--editorial` does not**: it changes text, keeping only the revision. So the transaction recording `applied` (`applied_vouching`) adds each changed node's new hash to `also` of every entry holding its old one, on cards not cancelled, one `card.refreshed` each, `revision` one more, so a `--revision` held across an editorial apply goes stale. `also`, not a replace: a worktree without that commit stays clean. A completion reads the commit's `Revision: <ID> <rev> (editorial)` trailers, hashing the node at its first parent and at it; anything unread vouches for nothing, and the mark stays for a person. A card row that does not decode is left alone: recording a commit never fails for a card's sake. A hand edit marks: nobody vouched for it.

`uses` carries no mark, direct targets only, avoiding floods. B uses D; D changes: D's cards are suspect, B's hash holds, B's stay clean. `spec trace D` lists incoming `uses`, one level, as consequences; `spec impact D` the whole reach. A definition with no card marks nothing, so a consequence needs its REF.

## Marks and rollup

A card's mark over the targets asked, first match (`card_mark`):

| Mark | Printed | When |
|---|---|---|
| departed | `departed <PR>` | an accept departs from one of them, its discrepancy not `applied` or `rejected` |
| correcting | `correcting <C>` | suspect on one, while a live correction holds it |
| suspect | `suspect <ID>` | suspect on one |
| accepted, open | the word | else |

A **live correction**: a card with `corrects`, neither accepted nor cancelled, the first by number. A discrepancy that does not read, or a row naming none: unsettled, a note.

A node's cards: a target in `SpecGraph::within(node)`, cancelled ones ignored. Its rollup, first match (`rollup`):

| State | When |
|---|---|
| `no card` | none |
| `departed` | a card's mark over it is departed |
| `suspect` | one is suspect |
| `in work` | one is open or correcting |
| `confirmed` | else: at least one accepted, none suspect or departed |

## Accepting

```text
spec card accept C --as N (--briefed | --departs REF... --summary T --observed T --documented T
  [--gap-type G]) [--note T] [--revision R]
```

A person's act, on a terminal as `task approve`, or on the page by `POST .../accept` (ADR-0057): a stdin that is no terminal exits 2 before anything is read, no `--yes`. Prompt `accept C-0001 (<title>) as <N>? [y/N]`; not `y`: exit 1, no event. `card_accept` asks through its `Consent`, then compares `revision`. One verdict, else exit 2; a departure takes its three texts, `--briefed` none. Rules: `cards.md` "Rules".

`--departs` names 1-16 direct targets, a manual card having none, and makes a discrepancy in one `Immediate` transaction with the accept (`accept_departing`), events `proposal.created` then `card.accepted`; a failure leaves neither. `node_ids`, `summary`: its flags; `gap_type`: `--gap-type` or `contradicts`; `severity` `normal`; `evidence` `[{file: "C-0001", observed, documented}]`; `options` core's two, change the brief and rework the result, each `not priced at acceptance`; `recommendation` 0; author `human`.

**The dedup never refuses a departure** (ADR-0012). Hits are read under the write lock as `create_intake` reads them, but the accept writes them into the item's `distinct_from` itself, the corpus's first, the first 64 (`DISTINCT_MAX`) that fit (`agent-intake.md` "Dedup"), and its notes list them all. Nobody types one; the exit stays 0.

**Git.** The item binds a place as `propose update` step 5, so `--departs` alone reads git, reads only (exit 2 outside a worktree, nothing changed). `--briefed` runs at a root with no `.git`.

**A correction's accept** also re-freezes each suspect entry of the card it corrects, `card.cleared` `corrected`, the note its reason. A target absent from its read is left untouched, so gone targets survive it and stay suspect until a person clears them. An original cancelled, or a row that does not read: the accept stands.

## Clearing

```text
spec card clear C --no-effect --reason T --as N [--revision R]
spec card clear C --needs-correction --reason T --estimate E --as N [--title T] [--revision R]
spec card clear --group REF --no-effect --reason T --as N
```

A person's act, on a terminal as `accept` (prompt `clear C-0001 (<title>) as <N>: no effect? [y/N]`, a group `clear <n> cards of <REF> as <N>: no effect? [y/N]`) or on the page; no MCP tool. Library `card_clear`, `card_clear_group`; judged on the read and again under the lock (`clear_card`, `clear_cards`). Exit 1, nothing changed: `local` or `agent` as the name (`clearer_problem`: no one's name), a blank reason or one over `NOTE_MAX`, a cancelled card, one not suspect, one whose mark a live correction holds, a stale `--revision`. Lines `cleared C-0001: <flow>/<column>`; JSON the write document, a group's `{group, cleared, notes}`.

- **No effect**, open or accepted cards: re-freezes each suspect entry. One on a gone target takes a null hash and keeps its last `rev`: not suspect, a note on every read. The next meaning change marks again.
- **Needs correction**, accepted cards only: a new card with `corrects`, the targets, flow and discipline, first in that flow, `--estimate`, title `Correct <title>` by default. The original stays accepted, its row untouched, its mark held: it reads `correcting C-0002`, the node `in work` through it. The correction accepted re-freezes the original; cancelled, the original is suspect again.
- **Group**, scoped: a card qualifies only when suspect on a target inside `within` of REF or of a document under it by `parent:`, only those entries re-freeze, and a mark outside stays. A card a correction holds is passed over. One transaction, one `card.cleared` each; one refusal leaves every card as it was. Exit 1 when REF names nothing or none qualifies. No effect only, no `--revision`.

`card.cleared` names (node, `rev`), which `Revision:` trailers join to a commit; under needs correction `nodes` are the entries the correction answers, not yet moved.

## The read

`spec trace [REF] [--stage S] [--summary]`, MCP `get_trace {ref?, stage?, summary?}` (a read), daemon `GET .../trace?ref=&stage=&summary=` (browser view):

```text
trace <REF or all> | nodes <n>, cards <c> | no card <a>, in work <b>, suspect <s>, departed <d>, confirmed <f>
stage <name or -> | distance <d or -> | no card <a>, in work <b>, suspect <s>, departed <d>, confirmed <f>
<state> | <name> | <title> | <path> | cards <c>
  card <id> | <flow>/<column> | <mark> | <title>
  uses <name> | <state> | cards <c>
[truncated: <k> of <c> cards of <name>, <u> of <m> uses of <name>, <j> of <n> nodes not shown; give a narrower REF]
```

Scope: REF's holders and what they hold by `parent:`; no REF: every live document and every section a card targets, by (path, position). Exit 0 whatever is listed, 1 only a REF naming nothing, 2 as `spec flow`. It changes no status, finding, claim or gate. The queue is read as `spec roadmap` reads it (`open_existing`, no schema step); no queue: `no card`. A corrupt card row: skipped, a note.

**Lanes.** A lane holds the listed nodes carrying a `stage:`, a section reading its document's; the last lane, and `--stage -`, the flow steps with none. So a group with no stage of its own stands in no lane, `stages` can be `[]`, and the lanes can sum to less than the header. A lane's `distance` is the least of its nodes', from core's walk of the read (`flow_walk`); lanes by (`distance`, unreached last, stage), the `null` stage last. A `[flow]` table with an error clears distances: lanes by name, a note `no distance read: <config>:<line>: <message>`.

**`--stage S`** keeps one lane's rows, `-` the steps with none; **`--summary`** keeps none. The header and the stage lines stay the whole read's under both; a row's counts and `uses` are built only for a row kept, so a summary stays light.

**Cut** at `OUTPUT_CAP_CHARS`: nodes whole; a first node over the cap alone shows its row and the card and `uses` lines that fit, so an answer is never empty. Header, stage lines and row counts stay whole; JSON holds the printed rows.

`{ref, reason, notes, counts, stages, nodes, truncated, omitted}`; a lane `{stage, distance, counts}`, `stage` `null` for the steps with none; a node `{id, title, path, stage, distance, rollup, counts, cards: [{id, flow, column, title, mark, on, discrepancy, correction}], uses: [{id, rollup, cards}]}`. `rollup` is the state as printed, `no card`, `in work` included; `counts` `{no_card, in_work, suspect, departed, confirmed}`, a node's over it and what it holds by `parent:`, while `rollup` covers its own cards only, so a group's screen shows `counts`. `stage`, `distance`: its document's. `mark` the word alone. `omitted` counts the rows cut, a mixed count: nodes + cards + `uses`; `null` when uncut.

**`spec card list --marks`** adds each row's mark over all its targets, core's `card_mark`, with `on`, `discrepancy`, `correction`. A gone target stands on no node, so `spec trace` says it in a note alone and a node's row reads clean; the list shows the mark, and the page takes it from there, Clear included. A cancelled card has none. An index that cannot be read is the read's error: exit 2, the daemon's 503, never a clean row.

**`spec flow --trace`** joins each shown node's `rollup` and `counts` (`spec-flow.md` "The read"). It never fails a flow: a queue that cannot be read, a newer one included, leaves `rollup` and `counts` null with a note, never `no card`; one below this build's is named.

**`spec impact`** adds the cards not cancelled with a direct target in the reached set: `spec-impact.md`. Its mark is over all a card's targets, unlike a node's row here.

## Known limits

- `raise.rs` `meaning_changes` pairs nodes with an ID only, so a card on an id-less document by path gets no `card.refreshed` on `--editorial` and reads suspect: marking falsely, never fresh.
- An accepted card with no snapshot or a null hash rolls up `confirmed`; only a note differs.
- Dropped entries have no cap, bounded only by the distinct nodes a card ever targeted: a cap would reopen the hole a dropped entry closes.
- Stage lines are never cut, as `spec flow`'s gap rows: a project with more stages than the cap holds passes `OUTPUT_CAP_CHARS`.
- A literal `stage: "-"` cannot be reached by `--stage`, where `-` asks the steps with none; its line reads as theirs, and JSON tells them apart.

## Tests

CLI `card_trace.rs`, `trace_views.rs`, `cards.rs`; store `queue_trace.rs`; MCP `mcp_trace.rs`, `mcp_impact.rs`; http `trace.rs`, `door.rs`, `daemon_keys.rs`; UI `src/board/*.test.*`, `api/http.test.ts`; fixture `card-press`: five documents under `PRN-WEEKLY`, a `uses` from `SHT-BACK` to `SHT-FRONT-HEAD`, an accepted decision linked to `SHT-FRONT`; fixture `trace-almanac`: stages cutting across two parts, one step with none.
