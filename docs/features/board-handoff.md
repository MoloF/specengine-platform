---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0085, v4 step 3a (08 s2 Phase 7, "v4 scope"); the analyst's requirement analysis of 2026-11-09; the owner's Q7 of 2026-11-08 and Q1-Q3 of 2026-11-09; W1-W10 working answers, the owner's review open
adrs: [ADR-0074]
---

# board-handoff: a board that holds still, a card modal and page, swimlanes, hand-off and return (RM-0085, 3a)

## Why

Opening a card today adds a 20-26rem grid column beside the columns (`.board-stage-panel`), so every column shifts; notes and a refusal render above the columns and push them down. A writer cannot hand a finished piece to development in one act: it takes New card with a typed parent, then Assign, because `spec card new` names no reviewer. A move carries no reason, so a return is invisible. The panel shows a card's parent but never its children. A developer has no swimlanes and no page for one card: Cmd-click on the title opens the board with the panel, and Cmd-click on the body ignores the modifier. The lead sees no returned hand-offs, which RM-0086 needs served. The tabs per workspace flow already shipped with 1a (`docs/canon/workspaces.md` "Screen").

This slice (3a) makes the board static, replaces the panel with a modal and a page, adds swimlanes, and turns today's child cards into a hand-off and a return through three small additions to card commands and one config key. Tasks inside cards is 3b, RM-0092 `card-tasks`: it carries the only schema step and is local only. ADR-0074 records the rule (`docs/canon/card-flows.md` "The rule").

The cut holds: 22 criteria and F (23, under the analysis's bar of about 30) at 9-12 pipeline hours. If 3a runs past 12 hours, apply the fallback: ship the screen alone first (static grid, modal, page, lanes: BH-1 to BH-10, BH-21, BH-22, no Rust), then the hand-off and return contract (BH-11 to BH-20) as a second pass.

### Decisions

- **D1** A "task in a modal" is a card (Jira's issue), never a `T-` task.
- **D2** Static: on one tab no column header moves on open, close, a write, a refusal, notes, Show cancelled, a filter, a lane change or a live re-read. Widths may differ between tabs and viewports. Columns are `minmax(14rem, 1fr)`. Above 36rem the board is its own scroll container, both axes, with a sticky header row; under 36rem it stacks as today.
- **D3** The modal replaces the side panel entirely; `.board-stage-panel` and its grid column go.
- **D4** `#/<p>/board/<C>` keeps its meaning, the board with C open, now as the modal. The page is `#/<p>/board/<C>/page`, never a tenth section (`SECTIONS` stays pinned).
- **D5** History: opening from the board pushes one entry; a card link inside the modal replaces; Esc and Close go back when this board pushed, else replace with `#/<p>/board`; arriving by URL selects the card's flow once; opening from inside never switches the tab beneath.
- **D6** A hand-off is an explicit act on one card. There is no automation.
- **D7** The receiving flow comes from the chosen discipline, as for any new card (`cards.md` "Config"). The dialog offers every served flow's disciplines, whatever the workspace shows.
- **D8** A hand-off copies title, goal and targets, all editable. It does not copy criteria, estimate, due date or evidence. The dialog has no assignee field (W12).
- **D9** The asking side is the child's reviewer: locally the acting name, on a hub the login. Hand off shows on open cards only.
- **D10** A return moves the child into its own flow's return column. The parent never moves by itself (Q1).
- **D11** A note may go with any move. Only a move into the return column from another column requires one.
- **D12** The reason lives only in the `card.moved` event; 3a adds no card column (W9).
- **D13** Taking a card is any move out of the receiving flow's first column. The rollup words are handed off, taken, returned and accepted (W4).
- **D14** Swimlanes are None (the default), Assignee or Parent, remembered per viewer and project (W10). "Not in the flow" stands last in every lane. A parent from a hidden flow still heads its lane, and a parent in the same flow is both a lane header and a tile.
- **D15** The page reads what the board reads (the list and the card). There is no new read route.
- **D16** The preset gains one return column per flow (W11), and `fixtures/studio-lamp` is re-pinned. `card-press` stays byte-equal.
- **D17** Every new UI string is a catalog key in both languages; the daemon's text stays English (ADR-0072).

### Questions

**The owner's answers, 2026-11-09:**
- **Q1** On a return only the child card moves, into its flow's return column with a reason; the writer's card stays and shows the rollup.
- **Q2** Tasks inside cards are local only; a hub card shows linked IDs without state (3b).
- **Q3** There is no board across trades now: tabs plus assignee lanes, revisited with RM-0086.

**Working answers, the owner's review open.** W1-W10 are the analyst's recommendations; W11-W13 are this spec's.
- **W1** The cut: 3a here, 3b as RM-0092.
- **W2** The ADRs: ADR-0074 `card-handoff` now; `card-task-link` with 3b's spec.
- **W3** `reviewer` reaches MCP too: the CLI, the daemon and `create_card` alike.
- **W4** No Take button. Taking is a move; the word appears only in the rollup.
- **W5** A scrim click closes the card modal, which holds no typed text. Write dialogs keep today's rule.
- **W6** The modal shows the reason, who and when; the tile shows a badge only.
- **W7** Hand off from the board's focused tile (`h`) as well as from the modal and page.
- **W8** Lanes cross with PageDown and PageUp, named keys, so no letter is spent.
- **W9** No return-reason column on the card row (D12).
- **W10** The lane mode lives in `localStorage` per project, one more key in `ui/README.md`.
- **W11** The preset's return column is `sent-back`, titled "Returned", second in every flow. `returned` is already a literal in `crates/specengine-cli/src/task.rs`, so the widened name guard would trip on it.
- **W12** The hand-off dialog has no assignee field: the receiving trade takes the card.
- **W13** The page is a fourth hash segment, `/page`, after the card ID, not a query.

## Description and interactions

### Screen (`ui/src/board/`)

- **The grid holds still.** The Board view fills the viewport below the shell, as Flow's studio does: the toolbar takes its natural height, and the board region (`.board-region`) takes the rest with `overflow: auto` and `min-height: 0`. Above 36rem the page itself never scrolls. One CSS grid holds the header row and every lane, so cells align with headers. The header row is `position: sticky; top: 0`, and the scroller's `scroll-padding-top` is its height, so a focused tile is never under it.
- **Notices.** Refusals (`role="alert"`) and write notes (`role="status"`) render in `.board-notices`, an overlay anchored to the board region's bottom edge (`position: absolute`). They wrap whole, can be dismissed, and never enter the grid's flow. Live regions sit outside the inert subtree.
- **Tiles.** Every tile holds one `<a href="#/<p>/board/<C>/page">`, its title, stretched over the tile (`::after`, `inset: 0`), with buttons and links above it. A plain click is default-prevented and opens the modal. A Meta, Ctrl or Shift click, a middle click and the context menu are left to the browser, so they open the page. The tile stays `draggable`.
- **Card modal.** `role="dialog"`, `aria-modal="true"`, `aria-labelledby` its `h2`, the card's title; focus starts on that heading. It is `min(48rem, 100vw - 2rem)` wide and at most `100dvh - 2rem` tall, its body scrolling inside; under 36rem it fills the screen. The board region and toolbar are `inert` while it is open. Its body is today's panel whole: every field, the mark and Clear, Accept's verdict, history (move notes included), parent and `corrects` links, plus **Hand-offs**, the card's children. Its actions are Move, Hand off, Return, Edit, Assign, Cancel, Accept (in `accept.from` only) and Clear, each opening its dialog over the modal. Esc closes only the top dialog (never mid-IME); while a submit is pending nothing closes or opens, Back included, which takes effect once the call settles. Closing returns focus to the tile it was opened from, in its current column. If that tile is not drawn (another tab, a collapsed lane, filtered out), focus goes to the board's heading.
- **Card page** `#/<p>/board/<C>/page`: no columns, one `h1` (the title), the modal's body and actions, and "Open on the board", a link to `#/<p>/board/<C>`. It reads `getCards` (for flows and children) and `getCard`, as the board does. The nav marks Board. A reload keeps the page. No such card shows the daemon's `no card C-9999` verbatim under `lang="en"`, in the page and in the modal alike.
- **Swimlanes.** A "Lanes" select in the toolbar offers None, Assignee and Parent, stored in `specengine.board.lanes.<p>`; an unknown or missing value reads as None.
  - Assignee lanes are ordered by name folded with `normalize("NFC").toLowerCase()`, ties by the raw name, then Unassigned.
  - Parent lanes are ordered by the parent ID's number, then Everything else. Each header shows the parent's ID (a link that opens its modal), title and `<flow>/<column>`, even when that flow is hidden.
  - A lane header spans the grid, its label sticky on the left. Each lane collapses (`aria-expanded`). A collapsed lane draws its row of cells with each column's count and no tile, and it stays collapsed through re-reads for the visit (Shell screen state, not storage).
  - Each cell is capped at 200 tiles and ends with "Show all <n>". Lanes come from the one list read (`useMemo`), with no new read.
- **Moving.** A drop on any cell sends `to` that cell's column and nothing else. A drop never assigns or re-parents, whichever lane it lands in. The Move menu, Up and Down, and native drag stay as they are. The Move menu entry for the flow's return column reads "Return…" and opens the Return dialog.
- **Hand off.** The dialog sits over the modal or page, or over the board from `h` on a focused tile.
  - Fields: Discipline, required, a text field with a datalist of every served flow's `disciplines`; Title, Goal and Targets, prefilled from the card; Reviewer. Reviewer is prefilled with "Acting as" and editable locally, and required: empty, it shows its hint and sends nothing. On a hub it is read-only and shows the login (`.field-fixed`). The parent shows fixed.
  - Submit is one `POST .../cards` with `title`, `goal`, `discipline`, `target`, `parent`, `reviewer` and `as`. The child appears under Hand-offs once the re-read settles, and a polite line names its ID and `<flow>/<column>` from the write document.
- **Return.** The dialog opens from Return (modal or page), from "Return…" in the Move menu, from a drop onto the return column from another column, or from `r` on a tile.
  - It holds one required Reason textarea. Submit is one `POST .../move` with `to`, the return column, plus `note`, `revision` and `as`. Cancel moves nothing, and the dropped tile stays where it was.
  - A drop or Up and Down inside the return column moves with no dialog.
  - Return shows only on an open card whose flow names `return`, which it is not in, and whose `allowed` holds a move to that column; a `by` the acting name lacks reads "needs <names>", as in the Move menu.
- **Returned.** A tile standing in its flow's return column shows the Returned label and icon (never colour alone); the badge goes when the card leaves.
  - The modal shows the latest reason, who and when: the last `history` entry whose payload `to` is the return column and whose `note` is a string. No event type is named in code.
  - If no such entry is left (64 events, Risk 12), the modal says the reason is no longer in the card's history.
- **Rollup.** A card's children are the list's rows whose `parent` is its ID; a cancelled child is left out.
  - Each child's word, looked up and never judged: accepted when `accepted`; returned in its flow's `return` column; handed off in its flow's first column; taken in any other column. A child whose flow is not served shows its `<flow>/<column>` with no word.
  - A parent's tile shows "<n> hand-offs" and, when any child is returned, the Returned badge with that count.
  - Hand-offs in the modal and page lists each child: ID (a link that replaces), title, `<flow>/<column>`, assignee and word.
- **Keyboard.** One tab stop over the tiles.
  - Arrows move within a cell and across columns inside a lane; Home and End go to the lane's first and last tile.
  - PageDown and PageUp go to the same column in the next or previous expanded lane holding a tile there; with none, focus stays.
  - Enter opens the modal. `m` opens the Move menu, `n` New card, `h` Hand off and `r` Return, each through `letterOf`, so a Russian layout works. `?` lists every key.

### Surfaces (Rust)

- **CLI** `crates/specengine-cli/src/card.rs`: `spec card new … [--reviewer N]`, `spec card move C [--to K] [--before C] [--note T] [--revision R] [--as N]`, `spec card list … [--parent C]`. Output lines and write JSON are unchanged.
- **Core**: `crates/specengine-core/src/card_toml.rs` reads `return`. A pure rule in `card.rs` refuses a move that enters the return column with no note, asked before the transaction and inside it, as every card rule is (`cards.md` "Rules").
- **Store** `queue/cards.rs`: `card.moved` carries `note`; `create_card` stores `reviewer` and `card.created` carries it. There is no schema step and no new event type.
- **MCP** `crates/specengine-mcp/src/cards.rs`: `create_card {reviewer}`, `move_card {note}`, `list_cards {parent}`. `create_card`'s and `move_card`'s descriptions (stdio and hub) name `reviewer` and `note`; each hub text stays ASCII and at most 2 048 B. The tool count stays 17 on a hub.
- **HTTP**: `CardNewBody.reviewer`, `CardMoveBody.note`, and `parent` in `GET .../cards`'s query (`app.rs`'s list arguments). No route is added; `GET_ROUTES` and the fence classes are unchanged.
- **Plugin** 0.30.0 -> 0.31.0, MINOR: keys that tools take. `read-spec`'s card bullet gains the sentence below, and its body stays at most 4 096 B:

  > A hand-off is a child card (`parent`) in the other trade's flow, its `reviewer` the side that asked; a move into a flow's `return` column needs a `note`, why it goes back.

- **Preset** `scripts/presets/gamedev.toml`: see "Data".

### Roles, files, order

The contract below is fixed, so the three roles start together.

| Role | May touch | Does |
|---|---|---|
| rust-developer | `crates/specengine-{core,store,cli,mcp,http}/src`, `plugin/`, `scripts/presets/gamedev.toml` | the return key and its messages, the note rule, the payloads, the three flags, the bodies and query, the MCP arguments and texts, plugin 0.31.0 and the `read-spec` sentence, the preset |
| test-engineer | `crates/*/tests`, `fixtures/` | new `crates/specengine-cli/tests/card_handoff.rs`; extends `store/tests/queue_cards.rs`, `mcp/tests/mcp_cards.rs`, `mcp/tests/plugin_files.rs` (`PINS`, `SCHEMA_PINS` 0.31.0), `http/tests/{cards,query,door,hub_team,hub,daemon_keys,no_domain}.rs`, `core/tests/workspace_toml.rs`, `eval/tests/workspace_names.rs`; `fixtures/studio-lamp/specengine.toml` (the preset's bytes), `fixtures/daemon-keys.json` (`CardFlow` + `return`) |
| ui-developer | `ui/**` but `ui/README.md`, `ui/dist` | `src/board/**` (the modal, page, lanes, dialogs, rollup), `src/app/{routes,Shell,shortcuts}.ts(x)`, `src/api/{provisional,client,http,queries}.ts`, `src/mocks/**`, `src/i18n/{en,ru}.ts`, `src/styles/{app,tokens}.css`, `src/workspace/choice.ts` (its comment's citation, "Canon at ship"), and every UI test named below, `BoardView.test.tsx` and `hubIdentity.test.tsx` rewritten for the modal |

**Order.**
1. The test-engineer writes the Rust tests red from "Data", alongside the rust-developer (about 3 h).
2. The ui-developer works against the mock from the start (about 7-9 h). The mock's `plates` flow takes `return: "blocked"`, so the column's word is shown to be data, and its cards gain children across `copy` and `plates`. A mock move into `blocked` with no note is refused 409 with the daemon's message.
3. Once Rust lands, the test-engineer runs the Rust mutations; once the UI lands, the UI mutations. Each M is run once, restored byte-equal (`cmp`), and recorded in "Implementation".
4. The orchestrator then runs the Looks. Nobody touches `docs/`, `.claude/` or another role's files.

Code comments cite canon headings that exist now, never this spec's sections, which compaction drops: `card-flows.md` "The rule"; `cards.md` "Commands", "Documents and events", "Rules"; `board-ui.md` "Routes", "Types"; `ui/README.md` "Screen rules".

### Canon at ship

`board-ui.md` (11 625 B) and `cards.md` (12 284 B) cannot take this slice in place, so at ship the spec-writer:
- **`docs/canon/card-flows.md`** goes from stub to whole: `cards.md`'s "Config" moves in with `return`; then the hand-off, the return and its messages, `--reviewer`, `--parent`, the payloads, the MCP arguments, the HTTP keys, the rollup words, reach on a hub, "Known limits" (Risks 1, 12 and 13) and "Tests". Its stub's "State" goes.
- **`docs/canon/cards.md`** keeps a pointer for "Config" and gains the three flags in "Commands", `note` and `reviewer` in "Documents and events" (both headings cited from code, so they stay), and the return line in "Rules".
- **`docs/canon/board-screen.md`** is new: `board-ui.md`'s "Screen" moves there, rewritten for the grid, notices, modal, page, URL and history, lanes, moving, dialogs, acting name, keyboard and states. `board-ui.md` keeps Routes (`parent` in the query), the door, the fence, Types (`CardFlow` + `return`), limits and tests, plus a pointer. `ui/src/workspace/choice.ts` cites `board-ui.md` "The acting name", which then moves, so the ui-developer re-points that comment at `ui/README.md` "Screen rules" (which lists the storage keys), a heading that holds before and after the ship.
- **`docs/canon/workspaces.md`**: "Preset" gains the return column, and "Guards" the return-column words.
- **`ui/README.md`**: "Shell" gains the page form and `specengine.board.lanes.<p>`, and "Dialogs" says a scrim click closes the palette and the card modal.
- **`07-interfaces.md`** §3 gains a Board row.
- `hub-team.md`, `hub-mcp.md` (2 B left each) and the cli and mcp READMEs (10 B left) are not touched.
- `docs/index-canon.md` reaches about 9.7 KB with the two new lines, so the next canon file there splits the shard (a `card*`/`board*` claim, `docs/README.md` "Generated" compressed to fit).
- This spec is compacted to 3 KB.

## Data

### `[[card.flow]]` `return`

One column of that flow; absent, or `null` when served, means no return column. Core field `CardFlow::return_column: Option<String>`, served as `return` after `accept`. `FLOW_KEYS` gains `return`. Each error stands at its line, exit 2 for `spec card` alone; `spec workspace` serves its flows unjudged with the card's note (`workspaces.md` "Config"):

```text
<config>:<line>: `return` is a string, not <type>
<config>:<line>: `return`: `<k>` is no column of this flow
<config>:<line>: `return`: `<k>` is the first column, where a new card starts
<config>:<line>: `return`: `<k>` is `accept.from`, where a card waits for its accept
<config>:<line>: `return`: `<k>` is `accept.to`, reached only by an accept
```

The preset, per flow: `"sent-back"` second in `columns`, `sent-back = "Returned"` in `titles`, and `return = "sent-back"` on the line after `accept`. For example:

```toml
[[card.flow]]
name = "development"
disciplines = ["development"]
columns = ["backlog", "sent-back", "ready", "in-progress", "in-review", "verify", "done"]
titles = { backlog = "Backlog", sent-back = "Returned", ready = "Ready", in-progress = "In progress", in-review = "In review", verify = "Verify", done = "Done" }
accept = { from = "verify", to = "done" }
return = "sent-back"
```

The preset's header comment gains one line: add `return` only after every terminal, plugin and hub serving the project runs this build (Risk 1).

### Commands

- **`--reviewer N`** on `new`: `assign`'s checks and texts, the person rule and then core `reviewer_problem`, so `agent` and `local` exit 1 with ``reviewer: `agent` can be no reviewer: a person accepts a result`` and nothing is written. `CardNewRequest.reviewer`.
- **`--note T`** on `move`: `cancel --reason`'s grammar, trimmed non-blank and at most `NOTE_MAX` (4 096) bytes. A blank note exits 1 with `note: blank; say why it moves`, an over-long one with `over_bytes("note", …)`. `CardMoveRequest.note`.
- **Into the return column** from another column with no `--note`: exit 1, nothing changed:

  ```text
  C-0007 enters `sent-back`, its flow's return column: say why with `--note`; nothing changed
  ```

  A reorder inside the column (`from` = `to`) needs none, nor does a move out of it.
- **`--parent C`** on `list`: the cards whose `parent` is exactly C. A C off the card-ID grammar exits 2, a look-alike naming its Latin form as every `C` argument does. No such card lists none, exit 0, with every flow line. `CardListRequest.parent`.

### Events (store)

```text
card.created {id, flow, column, position, assignee, reviewer, as}   reviewer null when none
card.moved   {id, from, to, position, as, note}                     note null when none
```

`QUEUE_EVENT_TYPES` stays 25, the queue schema 9 and the backup format 3. A move's own `history` entry is the only home of its note.

### Documents

`spec card list --json` `flows[]` becomes `{name, disciplines, columns, moves, accept, return}`. `CardRow` (21 keys), `Card` and the write document are unchanged. In `fixtures/daemon-keys.json`, `CardFlow` gains `"return"` and the set count is unchanged.

### MCP and HTTP

```json
{"name": "create_card", "arguments": {"title": "Lamp room dialogue", "discipline": "development", "parent": "C-0001", "reviewer": "Ann"}}
{"name": "move_card",   "arguments": {"card_id": "C-0002", "to": "sent-back", "note": "Two endings contradict SCN-LAMP-ROOM"}}
{"name": "list_cards",  "arguments": {"parent": "C-0001"}}
```

The arguments stay `deny_unknown_fields`, so an older plugin is refused with the new keys (Risk 1). The daemon's bodies are the arguments plus `as`, as before: `POST .../cards {…, reviewer, as}`, `POST .../cards/:id/move {to, before, note, revision, as}`, and `GET .../cards?parent=C-0001`, where a second `parent` is a 400 (`query.rs`). A missing note is a 409 with the write document, its reason the last of `notes`; `reviewer: "agent"` is a 409, as at `assign`; a reviewer off the person grammar is a 400.

**On a hub** both acts are plain writes for any login, `as` bound to it (`hub-team.md` "Acts"). Naming a reviewer at creation is what `assign` already allows (ADR-0060 "Cost"). Accepting the child takes a `[team]` login that is its reviewer. Every login reads every reason (ADR-0065 "Cost").

### UI types, keys, storage

- `provisional.ts`: `CardFlow.return: string | null`. The client's create input takes `reviewer`, and its move input `note`. The board does not send `parent`.
- Routes: `cardPageHash(project, id)`, and `parseHash` returning the board section with `id` and `view: "page"` for exactly the segments `board`, `<C>`, `page`; any other fourth segment is not found.
- Naming: a swimlane is `Swimlane`, and `filter.ts`'s `BoardLane` (a column) becomes `BoardColumnCell`.
- Catalog keys, with their final names left to the ui-developer: `board.modal.*` (close, openPage, handOffs, reasonGone), `board.page.openOnBoard`, `board.lanes.*` (label, none, assignee, parent, unassigned, everythingElse, collapse, expand, count plurals), `board.handoff.*`, `board.return.*` (badge included), `board.rollup.*` (handedOff, taken, returned, the count plurals; `accepted` reuses the existing accepted word), `board.keys.*`.
- Storage: `specengine.board.lanes.<p>` holds `none`, `assignee` or `parent`.

## Rules and edge cases

- **R1** WHEN a card is opened, closed, written or refused, or the list is read again, the system SHALL keep every column header's x, y and width on that tab.
- **R2** WHEN a person hands off or returns, the UI SHALL make exactly one write call: `POST .../cards` or `POST .../move`.
- **R3** WHEN a move enters a flow's `return` column from another column without a note, the system SHALL refuse it (exit 1, 409, a refusal for MCP) and change nothing. A reorder inside it, or a move out of it, SHALL need none.
- **R4** No card write SHALL move, create, refuse or re-read another card: a parent's move, accept and cancel ignore its children, and a child's ignore its parent (ADR-0074).
- **R5** `src/board/` SHALL name no column, flow, discipline or event type; the return column is looked up from `flows[]`, as `accept.from` is (`policy.test.ts`).
- **R6** The daemon's text (a refusal, a note, `no card …`) SHALL show verbatim under `lang="en"` on a Russian screen.
- **R7** A drop SHALL send `to` alone: never an assignee, a parent or a reviewer.
- **R8** A hidden flow's parent SHALL still head its lane, and a child in a hidden flow SHALL still count in its parent's rollup.
- **R9** A child whose targets include a node that no longer resolves is refused by the daemon as any new card is; the dialog keeps its text and shows the reason.
- **R10** The UI SHALL never say "blocked" (ADR-0012); Return asks for a reason before it sends.

### Risks

- **Risk 1 Older binaries refuse `return`.** An older `spec card` exits 2; an older daemon or the `v0.29.1` hub answers 503 on the Board; an older MCP server or plugin fails the card tools. → ADR-0073's rule: add the key only after every terminal, plugin and hub serving the project upgrades. The preset's comment says so, and so does `card-flows.md` "Known limits".
- **Risk 2 Canon caps.** → "Canon at ship".
- **Risk 3 The test rewrite.** `BoardView.test.tsx` (1 764 lines) and `hubIdentity.test.tsx` (1 226 lines) pin the panel's focus, Esc and routing. → They are rewritten for the modal inside the 9-12 h, and their hub assertions move to the modal.
- **Risk 4 Sticky headers** stick only inside a container that scrolls both ways. → Above 36rem the board region does; under 36rem it stacks; `scroll-padding-top` keeps focus out from under the header.
- **Risk 5 Width.** 8-column flows (narrative and level design, each with its return column) scroll sideways at 1 440 px. → The owner judges the look, BH-2.
- **Risk 6 Scale.** The board reads every row with marks on each `card.*` event and window focus, and lanes multiply cells. → A 200-tile cap per cell, collapsed lanes drawing nothing, one read; BH-22. On a hub every write re-reads every open board (a known limit).
- **Risk 7 Drag is tested in jsdom only.** → Looks: BH-9, BH-15.
- **Risk 8 Dialogs stack over the modal.** → One dialog stack, Esc on the top dialog only, nothing closing while a call is pending, live regions outside the inert subtree: BH-4.
- **Risk 9 First-load size.** 768.96 kB against the 845 kB bar, with steps 4 and 6 to follow. → BH-22 prints it. If the bar would be passed, the Board and the page become a lazy chunk in this slice.
- **Risk 10 The Russian accept word** ("Accept" is already the Russian verb built on `\u043f\u0440\u0438\u043d\u044f`). → BH-18.
- **Risk 11 Naming.** `BoardLane` already means a column. → It is renamed ("Data").
- **Risk 12 History keeps 64 events.** A returned card edited 64 times loses its reason. → The modal says so; a known limit.
- **Risk 13 Hub accepts.** Writers must be `[team]` logins to accept a hand-off's result on a hub. → A known limit, for the operator's guide.
- **Risk 14 The stretched link and drag.** A drag started on the title's overlay must still drag the tile. → The link is `draggable={false}`; the BH-9 Look starts its drag on the title.

## Acceptance criteria

Look = a dated check by the orchestrator in Chrome with real (CDP) input, at an exact viewport, on `vite preview` with the mock (`?scenario=`) or on `spec serve` over a scratch copy of `fixtures/studio-lamp` with cards made by `spec card new`. M = the mutation that must turn the named test red.

- [ ] **BH-1 Columns hold still.**
  - `ui/src/board/styles.test.ts`: no `.board-stage-panel` grid column; columns `minmax(14rem, 1fr)`; `.board-notices` `position: absolute`.
  - `BoardView.test.tsx`: opening the modal leaves the column elements and their order as they were; a 409 and a write's notes render inside `.board-notices`, never before the grid.
  - **Look** at 1440x900 and 1920x1080: each column header's rect is equal across open, close, a 409 (`conflict`), a write's notes, Show cancelled, a lane change, and a re-read after a move from a second tab.
  - M1: `.board-stage-panel`'s two-column grid restored. M2: the refusal rendered before the grid.
- [ ] **BH-2 Headers stay; no sideways page scroll.**
  - `styles.test.ts`: the header row `position: sticky; top: 0`; `.board-region` `overflow: auto` above 36rem; the stack under it.
  - **Look** (live): narrative's 8 columns at 1024x768 scroll inside the board, with `scrollWidth <= innerWidth`. At 1440x900 and 1920x1080, `scrollHeight == innerHeight`. On `large`, after 2 000 px down the header's top equals the region's top, and a tile reached by ArrowDown is never under it. At 320 px the columns stack.
  - M3: `sticky` removed.
- [ ] **BH-3 A click opens the modal.** `ui/src/board/CardModal.test.tsx` (new): a plain click gives one `role="dialog"`, `aria-modal`, labelled by the card's title, with `history.length` +1 and the hash `#/<p>/board/<C>`. Tab, filters, lane mode and the region's `scrollTop`/`scrollLeft` are unchanged, the region is `inert`, and focus is on the heading. M4: the hash pushed twice.
- [ ] **BH-4 Closing.** Same file:
  - Esc, Close and Back each close the modal and leave `#/<p>/board`. Esc and Close add no entry (opened by URL, they replace). A scrim click closes it.
  - Focus returns to the tile, in its new column if it moved meanwhile.
  - With a write dialog open, Esc closes only that dialog. While a submit is pending, Esc, Close and the scrim close nothing.
  - M5: Close pushing `#/<p>/board`. M6: one Esc closing both.
- [ ] **BH-5 Modified clicks open the page.** `ui/src/board/BoardCard.test.tsx` (new):
  - Every tile holds one `<a>` with `href` `#/<p>/board/<C>/page`.
  - A Meta, Ctrl, Shift or middle click anywhere on the tile is not `defaultPrevented` and changes neither the hash nor `history.length`.
  - A plain click is prevented and opens the modal.
  - M7: the link's click handler preventing the default whatever the modifier.
- [ ] **BH-6 The page.** `ui/src/board/CardPage.test.tsx` (new) and `ui/src/app/routes.test.ts`:
  - The page shows no column, one `h1` (the title), the modal's body, and "Open on the board" with `href` `#/<p>/board/<C>`; a remount on that hash draws it again.
  - `parseHash` and `cardPageHash` round-trip both forms for IDs and projects holding `%`, `/` and a space; any other fourth segment is not found.
  - `no card C-9999` shows verbatim under `lang="en"` in the page and the modal.
  - M8: the page form parsed as not found.
- [ ] **BH-7 Old links open the modal.** `BoardView.test.tsx`, `mocks/board.smoke.test.tsx`:
  - `sectionHash(p, "board", C)` (Flow's panel, `trace/CardsBlock.tsx`, the parent link) opens the modal over the board.
  - Arriving by URL selects the card's flow once, turning Show all flows on for a hidden one.
  - A parent or child link in the modal replaces the hash and switches no tab.
  - M9: an in-modal link switching the tab.
- [ ] **BH-8 Swimlanes.** `ui/src/board/swimlanes.test.ts` (new, pure) and `BoardView.test.tsx`:
  - None draws one lane. Assignee lanes run by folded name, then Unassigned. Parent lanes run by the ID's number, each header with ID, title and `<flow>/<column>` (a hidden flow's parent too), then Everything else.
  - A same-flow parent is both a header and a tile, and "Not in the flow" stands last in each lane.
  - Lane order survives a move that changes a lane's first row. A collapsed lane draws no tile, shows each column's count, and stays collapsed through a re-read.
  - The mode survives a remount; an unknown stored value reads as None.
  - **Look** at 1440x900: every cell's left and width equal its header's.
  - M10: lanes ordered by their first row.
- [ ] **BH-9 A drop is a move.**
  - `BoardView.test.tsx` (jsdom drag): a drop into another lane's cell of another column sends exactly one `POST .../move` with that `to`, and no `assign`.
  - **Look**: a real mouse drag, started on the title, dropped at 80 % of an empty cell's height, lands (`08-roadmap.md` step 7d).
  - M11: a drop into an Assignee lane also sending `assign`.
- [ ] **BH-10 Keyboard.** `BoardView.test.tsx`: one tab stop; arrows within a cell and across columns; Home and End; PageDown and PageUp across expanded lanes; Enter opens the modal; `m`, `n`, `h`, `r` through `letterOf`, a Russian layout's key included; `?` lists all. M12: `event.key === "h"`.
- [ ] **BH-11 Hand off in one call.** `ui/src/board/HandOffDialog.test.tsx` (new):
  - From the modal, the page and `h`, on open cards only.
  - The dialog offers every served flow's disciplines (a hidden flow's too), with title, goal and targets prefilled, and no criteria, estimate, due date or evidence.
  - Reviewer is the acting name and editable locally; on a hub it is the read-only login.
  - Submit sends exactly one `POST .../cards` with `title`, `goal`, `discipline`, `target`, `parent`, `reviewer` and `as`, and no other write. The child shows under Hand-offs after the re-read.
  - An empty acting name stops the submit with the field's hint and makes no call.
  - **Look** (live): a narrative card handed to `development` lands in `development/backlog`, its reviewer the acting name.
  - M13: two POSTs (create, then assign).
- [ ] **BH-12 `--reviewer`.** `crates/specengine-cli/tests/card_handoff.rs` (new), `crates/specengine-mcp/tests/mcp_cards.rs`, `crates/specengine-http/tests/cards.rs`:
  - On studio-lamp, `spec card new --title T --discipline narrative --as Ann`, then `spec card new --title T --parent C-0001 --discipline development --reviewer Ann --as Ann`, stores reviewer Ann, and `card.created` holds `"reviewer":"Ann"` (`null` without the flag).
  - `--reviewer agent` and `--reviewer local` exit 1 with `assign`'s message, writing nothing.
  - `create_card {reviewer}` and `POST .../cards {reviewer}` behave alike, `agent` a 409.
  - The child, moved to `verify`, is accepted `--as Ann` and refused `--as` its assignee.
  - M14: `card_new` dropping `reviewer`.
- [ ] **BH-13 The return key.** `card_handoff.rs`, `crates/specengine-http/tests/daemon_keys.rs`, `crates/specengine-core/tests/workspace_toml.rs`, `crates/specengine-eval/tests/workspace_names.rs`, `crates/specengine-http/tests/no_domain.rs`:
  - `flows[]` serves `return` after `accept`: `"sent-back"` on studio-lamp; `null` on card-press, every other byte of its JSON unchanged.
  - Each of the five errors in "Data" exits 2 at its line, for `spec card` alone.
  - The preset gives each flow `return = "sent-back"`, `studio-lamp` holds the preset verbatim, and both name guards take each preset `return` column as a word.
  - M15: the first column admitted as `return`.
- [ ] **BH-14 The reason.** `card_handoff.rs`, `crates/specengine-store/tests/queue_cards.rs`, `mcp_cards.rs`, `http/tests/cards.rs`:
  - `spec card move C --to sent-back` from another column without `--note` exits 1 with the message in "Data", with no event and nothing changed. With `--note "Two endings contradict"` it moves, and `card.moved` holds the note.
  - A note on any other move is recorded; without one, `note` is `null`. A reorder inside `sent-back` needs none.
  - A blank note, or one of 4 097 bytes, exits 1 in `cancel --reason`'s grammar.
  - `move_card {note}` and `POST .../move {note}` behave alike: 409 without the note, 200 with it.
  - M16: the note required on every move. M17: the note never required.
- [ ] **BH-15 Return in the UI.** `ui/src/board/ReturnDialog.test.tsx` (new):
  - The Move menu's entry for the return column reads "Return…" and opens a dialog with a required reason; `r` opens it too.
  - A drop onto the return column from another column opens it, and Cancel moves nothing. A drop inside the column moves with no dialog.
  - Submit sends exactly one `POST .../move` with `to` and `note`.
  - A tile in its return column shows the Returned label and icon, which go when it leaves. The modal shows the latest reason, who and when from `history`, or the "no longer in history" line.
  - `policy.test.ts`: `src/board/` names no column and no event type.
  - **Look** (live): a real mouse drop onto `sent-back` opens the dialog.
  - M18: a drop onto the return column posting no `note`.
- [ ] **BH-16 The parent's rollup.** `ui/src/board/rollup.test.ts` (new, pure), `CardModal.test.tsx`:
  - A cancelled child is left out. Each word follows "Rollup": handed off, taken, returned, accepted, and none for an unserved flow.
  - A parent's tile shows the count, plus the Returned badge with its count.
  - The modal lists each child with ID, title, `<flow>/<column>`, assignee and word.
  - M19: a cancelled child counted.
- [ ] **BH-17 Nothing waits on another card.** `card_handoff.rs`:
  - A parent with one open child and one returned child still moves and is accepted.
  - A child moves and is accepted under an accepted parent.
  - Creating, moving, returning or accepting a child adds no event for its parent and leaves the parent's `revision` unchanged.
  - M20: accept refused while a child is open.
- [ ] **BH-18 The word.** `ui/src/board/words.test.ts` (new):
  - The English values of `board.handoff.*`, `board.return.*` and the rollup's `handedOff`, `taken` and `returned` contain no `accept`, case-folded.
  - Their Russian values contain no `\u043f\u0440\u0438\u043d\u044f`, case-folded, the check written with those escapes only.
  - M21: the Russian for "taken" built on that stem.
- [ ] **BH-19 `--parent`.** `card_handoff.rs`, `mcp_cards.rs`, `crates/specengine-http/tests/query.rs`:
  - `spec card list --parent C-0001`, `list_cards {parent}` and `GET .../cards?parent=C-0001` list exactly its children, with every flow line.
  - No child: an empty list, exit 0. A second `parent`: a 400. A look-alike exits 2 with its Latin form.
  - M22: the filter ignored.
- [ ] **BH-20 Door and hub.** `crates/specengine-http/tests/door.rs`, `hub_team.rs`, `hub.rs`:
  - A hand-off and a return each add exactly one `card.*` event and leave git and proposals untouched; `CARD_CALLS` stays ten.
  - On a hub both pass for a login outside `[team]`, under that login.
  - Accepting the child passes for its reviewer only as a `[team]` login.
  - 28 routes; the fence classes are unchanged.
  - M23: a new `POST .../cards/:id/return` route.
- [ ] **BH-21 Languages.** `ui/src/i18n/catalog.test.ts`, `policy.test.ts`, `ui/src/app/screens.language.test.tsx`:
  - Every new string is a key with a Russian value.
  - In Russian, the board, the modal and the page show `ruT`'s strings.
  - A daemon refusal in the Return dialog stays verbatim under `lang="en"`.
  - M24: a literal "Return" in `src/board/`.
- [ ] **BH-22 Size and scale.**
  - `pnpm build`: first-load JS at most 845 kB (`ui-languages.md` "Size"), printed in "Implementation" with its gzip and `ru-*.js`.
  - `BoardView.test.tsx` on `large` (5 000 cards in one flow): every cell, Assignee lanes included, draws at most 200 tiles and "Show all"; a collapsed lane draws none.
  - M25: the per-cell cap removed.
- [ ] **F Form.**
  - `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo nextest run` for core, store, cli, mcp and http.
  - Eval: `doc_pointers`, `anonymity`, `ui_policy` (last) and `workspace_names`.
  - `pnpm lint`, `pnpm build`, `pnpm test`.
  - `spec check`: 0 errors, 0 warnings.
  - Plugin 0.31.0 with `PINS` and `SCHEMA_PINS` entries (`plugin_files.rs`), and `read-spec`'s body at most 4 096 B.

## Out of scope

- 3b, RM-0092: the card-to-`T-` link and the Tasks block.
- One board across trades (Q3); WIP limits; filters beyond Discipline and Assignee.
- Tab, lanes or filters in the URL; a per-workspace default lane mode.
- A Take or "Send again" button (W4); the reason on the tile (W6).
- Automation: an automatic hand-off on entering a column, a parent following its children.
- Comments; keyboard drag (the Move menu stays the keyboard path).
- Creating `T-` tasks from the UI; tasks on a hub.
- Dashboards and counts (RM-0086); a board snapshot in git (RM-0052).
- Any schema step, new event type, new route or new MCP tool; `.claude/`.

## Implementation

Filled in after implementation.
