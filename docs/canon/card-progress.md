---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-12
---

# Card progress: a parent's subtasks, Ready to accept and Relies on

Built by `handoff-progress` (RM-0098, v5 step 6), UI only, no ADR: the work item's Subtasks part shows a parent's progress from its own children read. Rules: ADR-0074 and ADR-0012 (nothing waits, nothing is blocked), ADR-0077 "The rule" (no rollup reads an item), ADR-0008. The Board's tile rollup: `card-flows.md` "Children"; the work item around it: `ui-design.md` "The work item". Code: UI `src/board/{rollup.ts,CardProgress.tsx,CardLink.tsx,labels.ts}`, `src/api/queries.ts` `useChildren`.

## The rule

Nothing closes a parent on its own; a person accepts it. "Ready to accept" is a status line, never a control: it never changes whether Accept shows, its state or its place; Accept stands in `accept.from` alone (`board-screen.md` "Moving"). An open question on the card leaves the bar, Ready and Relies on as they are.

## Reads

- **The children read.** `CardView` (`CardPanel.tsx`, modal and page) reads its card's children itself: `useChildren(project, id)` calls `getCards(project, {parent: id, with_cancelled: true})`, never `marks` (the mark comes from the Board's list). Its key `queryKeys.children` = `["cards", project, {parent, with_cancelled: true}]` sits under `cardListsOf(project)`, so a `card.*` event, a write's settle or a gap reads it again; enabled only while a card is shown, so a closed modal reads it no more. The Board's own list read is unchanged (`board-screen.md` "Reads").
- **The reasons.** One `useCard(project, childId)` per cancelled child, keyed `queryKeys.card`, shared with that child's own modal; its `cancellation {by, at, reason}`. An open or accepted child causes no card read. A list row keeps `cancelled: boolean`; nothing served changes.

## The model

`rollup.ts` `progressOf(rows, flows, parent: {id, open}): Progress`, pure, over the children read's rows and flows; rows naming another parent, and the parent's own, are ignored; a grandchild never counts.
- `children` (not cancelled, accepted ones too), `cancelled` and `reliesOn` keep served order.
- **Words**, in the bar's order (`PROGRESS_ORDER`): accepted (by the row's `accepted`, also in a column its flow dropped), cancelled (by `cancelled`), then an open child's `category.ts` `categoryOf`: returned, in progress (a middle column, and `accept.to`, open only after a config change), to do, not in the flow. Zero counts are omitted; the counts sum to every child. Looked up, never compared (`policy.test.ts`).
- **`ready`**: the parent open (no `acceptance`, no `cancellation`), one child or more, every child accepted or cancelled, all cancelled included; judged from the children read alone, never waiting on a reason.
- **`reliesOn`**: on an open parent, the children neither accepted nor cancelled; else empty. Never an item (`PR-`) nor a `T-` task.
- A child in a flow the chosen department hides counts (ADR-0073).

## Screen

`src/board/CardProgress.tsx`, in first load as the work item is. Subtasks (`board.modal.handOffs`), in order:
1. **Ready to accept**, on an open parent that is ready: a `success` lozenge `board.progress.ready` and `board.progress.readyAbout`; no button, link or `aria-disabled`.
2. **The bar**, with one child or more, on any parent: an `aria-hidden` row of segments, one per non-zero word, each `flex-grow` its count, each fill its appearance's lozenge text role (accepted `success` `taskAccepted`, cancelled `neutral` `taskCancelled`, returned `warning` `cardReturned`, in progress `information` `taskInProgress`, to do `neutral` `open`, not in the flow `neutral` `unknown`). Under it the words line, in that order, each word after the first led by a middle dot, its own `aria-hidden` element spaced as the breadcrumb's, each word with its icon and plural (`board.progress.<word>`): never colour alone.
3. **Relies on** (`board.progress.reliesOn`), on an open parent with an open child: each line as a Subtasks row: its ID a `CardLink` (a plain click replaces the modal's card), title, flow title (`flowTitleOf`, raw when unserved).
4. **The rows**, the not-cancelled children: ID, title, flow title, status lozenge, assignee, the hand-off word; no child at all, "None yet." alone.
5. **Cancelled subtasks** (`board.progress.cancelledList`): ID, title, flow title, status lozenge, the Cancelled badge, then the reason as shown text and `board.panel.byAt`. Its read pending: `board.progress.reasonReading`; failed: `board.progress.reasonFailed`, the daemon's words under `lang="en"`, no live region. The other parts stand either way.

After Subtasks, **Decisions from subtasks**, a lazy part from the card's `subtask_items` and this children read: `card-mandate.md` "Screen".

The children read pending: a `Skeleton` `board.progress.reading`. Failed: `board.progress.failed`, the daemon's words under `lang="en"`, `role="alert"` and Retry; the rest of the work item stands, and no count comes from the Board's list. A failed re-read keeps the last parts with its line (`useRetainedFailure`).

## Words

Every new word is a `board.progress.*` key, counts plural (`plural.test.ts`). "Ready to accept" is the owner's word (v5 Q3): the group stands outside the scan of `src/board/words.test.ts`, which bans "accept" in hand-off, return and rollup words, and that test pins `en.board.progress.ready` and that no `board.progress` key is scanned.

## Known limits

- **Tile against bar.** The Board tile's "<n> subtasks" counts the Board's list, which leaves cancelled children out; the bar counts every child.
- **One read per cancelled child**, each a `show --history`, unmeasured on a hub.
- **Shared key.** While a parent is open its reason reads run again on every `task.*`, `proposal.*` and gap event, which read every `["card", p]` key again.
- The children read is sent before the card read answers.
- One level: no grandchild; the tile shows no bar, no Ready.
- **The Overview's Ready to accept** counts `ready`'s rule again, in the store's SQL (`card-counts.md` "Counts"); only tests tie the two.

## Tests

UI `src/board/progress.test.ts` (the model), `CardProgress.test.tsx` (the view, stub clients), `words.test.ts` (Q3's pin), `Card{Modal,Page}.test.tsx`, `BoardView.test.tsx`, `api/live.test.tsx` (the re-read on `card.moved`, none after close), `mocks/progress.smoke.test.tsx` (harbor-sim: the reads, `C-0001`'s bar and Relies on, `C-0004` ready beside Accept, `C-0017`'s reason), `policy.test.ts`.
