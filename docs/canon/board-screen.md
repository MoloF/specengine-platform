---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-11
---

# Board screen: a still grid, the card modal and page, lanes, hand-off and return

ADR-0056, ADR-0057, ADR-0074; built by `board-ui` (RM-0046), reworked by `board-handoff` (RM-0085). Routes, door, fence, types: `board-ui.md`; hand-off and return: `card-flows.md`. Code: UI `src/board/`, `src/trace/`, `src/ui/Dialog.tsx`, `useViewTop.ts`.

## Reads

Section `board`: `#/<p>/board[/<C>[/page]]`. One read of `getCards(project, options?, signal?)`, always `marks`, `with_cancelled` on Show cancelled, never `parent`; `getCard` for the routed card; `card.*` events and gaps read both again, apply and decision events and window focus (a hand edit raises none) the list. **Marks come from the list**: `rowMark` over its four served keys, so a card can never look normal while suspect or departed; an index that cannot be read is the read's 503, so `ErrorPanel` stands where a clean board would. The whole trace read (`useTrace`) serves the open card only, for each target's rollup word; `card.*`, apply and decision events and gaps read it again. Columns, moves, `accept` and `return` come from the read, looked up, never judged; `labels.ts` is the one word map. The page reads what the board reads.

## The grid

On one tab no column header moves on open, close, a write, a refusal, notes, Show cancelled, a filter, a lane change or a re-read; widths differ between tabs and viewports.

- **Layout.** Above 36rem the view fills the viewport below the shell (`--board-top` by `useViewTop`, as Flow's studio): the toolbar takes its height, `.board-region` the rest, `overflow: auto` both ways, and the page never scrolls. One CSS grid holds the header row and every lane (subgrid), columns `minmax(14rem, 1fr)`, so cells align with headers. `.board-head` is `position: sticky; top: 0`; a callback ref on it gives the region its height as `scroll-padding-top`, so a focused tile is never under it. Under 36rem the columns stack, each cell with its title, and the page scrolls.
- **Notices** stand in `.board-notices`, an overlay at the region's bottom edge (`position: absolute`), never in the grid's flow: refusals (`role="alert"`), write notes (`role="status"`), the list's notes, truncation, the empty and no-match lines. List notes, refusals and write notes can be dismissed until they change. Live regions sit outside the inert subtree. The view head holds one hint, "Press ? for the keys."
- **Tiles** show title, assignee, due, estimate and the source: the first target, a document link (`doc-view.md`), `+<n>` for the rest; no target, "By hand". Accepted and cancelled say so, with icons. A mark stands first: Suspect, Departed or Correcting, a label and an icon from `labels.ts`, naming the target, the discrepancy or the correction; an accepted word yields to it; a gone target shows its mark and Clear; an unknown mark word is a neutral badge. Its title is one `<a href="#/<p>/board/<C>/page">` stretched over it (`::after`), `draggable={false}`, buttons above: a plain click opens the modal; a Meta, Ctrl, Shift or middle click and the context menu are the browser's, opening the page.
- **Overdue.** `due.ts` alone holds a clock: `localToday` by local date, overdue as `due < today` strings, never on a closed card; label and icon; a redraw only when the day turns.
- **Vocabulary.** A column head is its category's lozenge, the count after it (`ui-design.md` "Status categories"). A column named `blocked` is one more column. `src/board/` branches on no column, flow, discipline or event word (`policy.test.ts`); the return column is looked up in `flows[]`, as `accept.from` is.
- **Stranded.** A row outside `columns[]` stands in "Not in the flow", last in every lane, with the daemon's note. A cancelled card in a column the flow no longer lists is not drawn.

## Flows, filters, lanes

- A tab per flow the chosen department shows (`workspaces.md` "Screen"). A flow's served `title` labels its tab, the board's name, a Parent lane head, the side Flow row and a Subtasks row; the hand-off's line and a document's Cards block keep raw `<flow>/<column>`. Discipline and Assignee (with Unassigned) AND, per-project screen state. A cell ends "Show all <n>" past 200 tiles.
- **Lanes**: a select, None (the default), Assignee or Parent, in `localStorage` `specengine.board.lanes.<p>`; unknown or missing reads None. Assignee lanes by name folded with `normalize("NFC").toLowerCase()`, ties by the raw name, then Unassigned. Parent lanes by the parent ID's number, each head the parent's ID (a link to its modal), title, flow title and status lozenge, a hidden flow's too, then Everything else. Unassigned and Everything else stand only when they hold a card. A same-flow parent is both a head and a tile. A head spans the grid, label sticky at the left. A lane collapses (`aria-expanded`) to its cells' counts, no tile, and stays collapsed through re-reads for the visit (Shell screen state).

## The card modal and page

- **Modal** at `#/<p>/board/<C>`: `role="dialog"`, `aria-modal`, labelled by its `h2`, the card's title, focus on it (`focusTitle`); a work item, `CardView`, at most `100dvh - 2rem` tall, its body scrolling, under 36rem the whole (layout, width, acts' order, the trap: `ui-design.md` "The work item", "The focus trap"). Region and toolbar are `inert` under it; a scrim click closes it. Targets open their document over it (`doc-view.md`), `parent` and `corrects` link their cards; evidence as kind, value, by, at, a `url` text never an `href`; `history` by `seq`, move notes included; **Subtasks**, its children; **Agent tasks** (`card-tasks.md` "Screen"). Each act (`CardActs.tsx`), Clear and Move open a dialog or menu over it; Move offers Up and Down only when the card is drawn on the current tab. A write from it reports inside it: a refusal as its `role="alert"`, a 400 naming `--as`, the write's notes. What it holds is the shown card's: another card, or none, starts clean.
- **History**: opening from the board pushes one entry; a card link inside replaces and never switches the tab; Esc and Close go back when this board pushed, else replace with `#/<p>/board`; Back closes it, and with a dialog or menu over it closes both. Arriving by URL selects the card's flow once, a hidden one by Show all flows. Closing returns focus to the tile, in its current column; one not drawn (another tab, a collapsed lane, filtered out) sends it to the board's heading.
- **Page** `#/<p>/board/<C>/page` (`cardPageHash`; `parseHash` gives `view: "page"` for exactly `board`, `<C>`, `page`, any other fourth segment not found): no columns, the same work item, its `h1` the title, and "Open on the board" to `#/<p>/board/<C>`. The nav marks Board; a reload keeps it. Its moves come from `allowed`, Up and Down from the list; a 400 naming `--as` is its refusal. No such card shows `no card C-9999` verbatim under `lang="en"`, page and modal alike.
- **Nothing opens while a card write is out**: every card write in the project shares one mutation key (`useCardWriting`); a click meanwhile is dropped, not delayed, and no dialog closes while its submit is pending.

## Moving

`moveCard` has three doors: `MoveMenu`, listing its column's `moves[]` with each `by` as "needs <names>"; Up and Down, by `before`; native drag, no new dependency. A closed card has no Move. Accept shows only in `accept.from`. A drop on a cell sends that cell's column as `to`, `before` from the drop point, and nothing else: never an assignee, a parent or a reviewer, whatever lane it lands in. Cells stand over the free height, so a drop anywhere lands, below every card last. The Move menu's entry for the flow's return column reads "Return…" and opens the Return dialog.

## Writes and dialogs

Each write goes through `useCardWrite`, settling after the re-read of lists, card, trace and flows, so nothing is optimistic; `clearCardGroup` through `useCardClearGroup` likewise. A 409 shows the daemon's reason; a dialog keeps its text and takes the new revision. Edit and Assign send only what differs from the form as opened, on the revision read now. New task on a node's Cards block presets the node as target; that block reuses Clear and adds Clear all, `ClearGroupDialog` (`spec-flow-screen.md` "Panel").

- **Against the brief.** The modal shows a card's mark with its sentence and, on a suspect card, open or accepted, a Clear button; a correcting mark takes none. A failed trace read is said, drops the targets' rollup words and keeps the list's mark.
- **Accept** asks As briefed or Departs from it; a departure names direct targets, a summary, what was observed, what was documented and a gap. **Clear** takes No effect with a required reason, or, on an accepted card, Needs correction with an estimate and an optional title. The name stands last; the dialog is the consent.
- **Hand off**: the dialog, its Department and Discipline selects and its write, `card-flows.md` "Hand-off".
- **Return** (an open card whose flow names `return`, not in it, `allowed` holding that move; the modal, the page, "Return…", `r`, or a drop onto that column from another): one required Reason. Submit: one `POST .../move` with `to`, `note`, `revision`, `as`, and a drop's `before`; the line "C-0003 sent back". Cancel moves nothing. A drop or Up and Down inside the column moves with no dialog. A `by` the acting name lacks reads "needs <names>".
- **Returned**: a tile in its flow's return column shows the Returned label and icon. The modal shows the latest reason, who and when (`returnReasonOf`: the last `history` entry whose payload's `to` is that column and `note` a string, no event type named), else that the reason is no longer in the card's history.
- **Rollup** (`card-flows.md` "Children"): a parent's tile shows "<n> subtasks" and, with any returned, the Returned badge and its count; Subtasks lists each child: ID (a link that replaces), title, flow title, status, assignee, word.

## The acting name

"Acting as", in the board's header, lives in `localStorage` `specengine.board.as`, sent trimmed as `as`, empty as `null`; a 400 naming `--as` shows under its field, positioned absolute, so no column moves. On a hub the field is read-only and shows the login (`hub.md` "Screen"); the dialogs' names and idle controls: `hub-team.md` "Screen".

## Keyboard, states

One tab stop over the tiles: arrows within a cell and across columns inside a lane; Home, End the lane's first and last tile; PageDown, PageUp the same column in the next or previous expanded lane holding a tile there; Enter the modal; `m` the Move menu, `n` New task, `h` Hand off, `r` Return, by `letterOf`; `?` lists them (`app.shortcuts.board.*`, `app.keys.*`). Esc closes only the top dialog, never mid-IME. Focus follows a written card once its re-read settles, revealing a capped column or letting go, and a polite live region says so; after Accept from the modal it goes to the heading, from the Move menu it follows the card.

Loading: a skeleton. Failed: `ErrorPanel` verbatim, Retry; a bad `[card]` is that 503. A failed list re-read does the same where the columns stood, no tile from the last read; it takes the modal's mark and Clear and closes the Move menu and any drag. Empty: the columns and how to add the first card. Mock scenarios `empty`, `conflict`, `large` reach the Board; the mock's `plates` flow takes `return: "blocked"`.

Strings are catalog keys in both languages (`ui-languages.md`); the daemon's text (a refusal, a note, `no card …`) stays verbatim under `lang="en"`. No word of a hand-off, take, return or rollup is built on "accept" or the Russian accept's stem (`words.test.ts`).

## Known limits

Drag is tested in jsdom and, in Chrome's layout, by synthetic drag events; a real-mouse drop is the owner's hand check. On a hub every write re-reads every open board.

## Tests

UI `src/board/*.test.*`, `src/trace/*.test.*`, `app/routes.test.ts`, `app/screens.language.test.tsx`, `mocks/board.smoke.test.tsx`, `policy.test.ts`.
