---
class: spec
status: shipped
shipped: 2026-11-09
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0085, v4 step 3a (08 s2 Phase 7, "v4 scope"); the owner's Q7 of 2026-11-08 and Q1-Q3 of 2026-11-09; working answers W1-W13 await the owner's review
adrs: [ADR-0074]
---

# board-handoff: a board that holds still, a card modal and page, swimlanes, hand-off and return (RM-0085, 3a)

## Why

Opening a card added a grid column beside the columns, so every column shifted, and notes and refusals pushed them down. A writer could not hand finished work to development in one act (New card, then Assign), a move carried no reason, so a return was invisible, and the panel never showed a card's children. A developer had no swimlanes and no page for one card. So the board holds still, a modal and a page replace the panel, lanes cut the one read, and today's child cards become a hand-off (`spec card new --reviewer`) and a return (a move into the flow's `return` column with `--note`); ADR-0074. Tasks inside cards are 3b, RM-0092. How it works now: `docs/canon/card-flows.md` (config, hand-off, return, children), `docs/canon/board-screen.md` (the screen), `docs/canon/cards.md`, `docs/canon/board-ui.md`, `docs/canon/workspaces.md` "Preset".

## Data

The `return` key, its five errors and the preset's `sent-back` column: `docs/canon/card-flows.md` "Config".

### Surfaces (Rust)

Plugin 0.30.0 -> 0.31.0, MINOR (keys the card tools take). `create_card`'s and `move_card`'s descriptions, stdio and hub, name `reviewer` and `note`, each hub text ASCII and at most 2 048 B, 17 hub tools. `read-spec`'s card bullet gains, its body at most 4 096 B:

> A hand-off is a child card (`parent`) in the other trade's flow, its `reviewer` the side that asked; a move into a flow's `return` column needs a `note`, why it goes back.

### Documents

`flows[]` gains `return` after `accept`, `null` with none (`cards.md` "Documents and events"); `CardRow`, `Card` and the write document are unchanged; `fixtures/daemon-keys.json`'s `CardFlow` gains `return`; `card-press` stays byte-equal.

### MCP and HTTP

`create_card {reviewer}`, `move_card {note}`, `list_cards {parent}`; the daemon's bodies are those plus `as`, and `GET .../cards?parent=` (`card-flows.md` "Hand-off", "Return", "Children", "MCP and hub").

## Acceptance criteria

Look = the orchestrator's dated check in Chrome on `vite preview` with the mock or on `spec serve` over a scratch `fixtures/studio-lamp`. Each M named turned its test red.

- [x] **BH-1 Columns hold still.** `board/styles.test.ts`, `BoardView.test.tsx`. M1, M2. Look at 1440x900: header rects equal across open, Esc, Show cancelled, a lane change; 1920x1080, a 409, a write's notes and a second tab's re-read were not looked at (jsdom pins the 409 and notes).
- [x] **BH-2 Headers stay; no sideways page scroll.** `styles.test.ts`. M3. Look: on `large` the head stays at the region's top past 2 000 px and ArrowDown, ArrowUp keep the tile below it; 320 px stacks; live narrative (8 columns) at 1024x768 scrolls inside the region.
- [x] **BH-3 A click opens the modal.** `board/CardModal.test.tsx`. M4.
- [x] **BH-4 Closing.** `CardModal.test.tsx`. M5, M6.
- [x] **BH-5 Modified clicks open the page.** `board/BoardCard.test.tsx`. M7.
- [x] **BH-6 The page.** `board/CardPage.test.tsx`, `app/routes.test.ts`. M8.
- [x] **BH-7 Old links open the modal.** `BoardView.test.tsx`, `mocks/board.smoke.test.tsx`. M9.
- [x] **BH-8 Swimlanes.** `board/swimlanes.test.ts`, `BoardView.test.tsx`. M10. Look at 1440x900: Assignee lanes' cells aligned with their headers.
- [x] **BH-9 A drop is a move.** `BoardView.test.tsx`. M11. Look amended: the drop was verified in Chrome's real layout with synthetic drag events (C-0019 dropped on Lena Fisk/Proof moved to Proof, stayed in its lane, no assign). A real-mouse drag starts native drag and drop (dragstart, dragenter seen), but the browser tools release before a drop. **Open: the owner's hand check of a real-mouse drop.**
- [x] **BH-10 Keyboard.** `BoardView.test.tsx`. M12.
- [x] **BH-11 Hand off in one call.** `board/HandOffDialog.test.tsx`. M13. Look (live): C-0001's Hand off sent one `POST /cards`; C-0003 landed in `development/backlog`, reviewer Ann, parent C-0001, git untouched.
- [x] **BH-12 `--reviewer`.** `crates/specengine-cli/tests/card_handoff.rs`, mcp `mcp_cards.rs`, http `cards.rs`. M14.
- [x] **BH-13 The return key.** `card_handoff.rs`, http `daemon_keys.rs`, `no_domain.rs`, core `workspace_toml.rs`, eval `workspace_names.rs`. M15.
- [x] **BH-14 The reason.** `card_handoff.rs`, store `queue_cards.rs`, `mcp_cards.rs`, http `cards.rs`. M16, M17.
- [x] **BH-15 Return in the UI.** `board/ReturnDialog.test.tsx`, `policy.test.ts`. M18. Amended: a Return opened by a drop between two cards also sends `before`. Look: on the mock a drop onto `plates`' `blocked` opens the dialog and Cancel posts nothing; live, a drop onto `sent-back` opened it, an empty reason sent nothing, a reason sent one move, and the parent showed "1 hand-off", "1 returned".
- [x] **BH-16 The parent's rollup.** `board/rollup.test.ts`, `CardModal.test.tsx`. M19.
- [x] **BH-17 Nothing waits on another card.** `card_handoff.rs`. M20.
- [x] **BH-18 The word.** `board/words.test.ts`. M21.
- [x] **BH-19 `--parent`.** `card_handoff.rs`, `mcp_cards.rs`, http `query.rs`. M22.
- [x] **BH-20 Door and hub.** http `door.rs`, `hub_team.rs`, `hub.rs`, `hub_mcp.rs`. M23.
- [x] **BH-21 Languages.** `i18n/catalog.test.ts`, `policy.test.ts`, `app/screens.language.test.tsx`. M24.
- [x] **BH-22 Size and scale.** `pnpm build` (below); `BoardView.test.tsx` on `large`. M25.
- [x] **F Form.** Rust: the full workspace `cargo nextest run`, 5 276 passed, 0 failed; clippy and rustfmt clean; eval `doc_pointers`, `anonymity`, `ui_policy`, `workspace_names`; `PINS`, `SCHEMA_PINS` 0.31.0; `read-spec` 4 079 B. UI: `pnpm lint`, `pnpm build`, `pnpm test` (3 730). `spec check` 0 errors, 0 warnings.

## Out of scope

3b (RM-0092); one board across trades (Q3); a Take button, the reason on the tile; automation; tab, lanes or filters in the URL; any schema step, event type, route or MCP tool.

## Implementation

Rust review ACCEPT with minors, no code defect; UI review ACCEPT with minors, all fixed in iteration 2. Both accepted every departure below.

**Rust departures.** (1) `--reviewer -` on `new` is refused, exit 1, as `--assignee -` is. (2) Core counts a blank note as none; the CLI's grammar refuses it first. (3) The preset's header comment is two lines, citing no spec. (4) `scripts/deploy/.env.example` went to 0.31.0 too (it must equal `plugin.json`), a file the role table omitted. (5) A move's checks run: closed, the note's grammar, a stale revision, then core (the flow's move, `by`, the return note), so `by` wins over a missing note. (6) `list_cards`' text also names `parent` and `return`. Recorded with them: a `?parent=` off the grammar is a 503 with the CLI's exit-2 line; `parent` stands last in the query-names message; a note is stored untrimmed; `move_card.note` is free text exempt from decision-staging's choice walk (`mcp_stage.rs`); `card-press`'s `expected.json` predates `return`, its oracle adding `"return": null`. After the ship, store `queue_cards.rs` `board_handoff_by_and_the_flows_moves_come_before_the_return_note` pinned that order, red under `NoNote` first.

**UI departures.** (1) Key words in `app.shortcuts.board.*`, `app.keys.pageDown`, `pageUp`; no lane collapse keys. (2) Every notice stands in `.board-notices`; list notes, refusals and write notes are dismissed until they change. (3) The modal's refusals and write reports show inside it. (4) A drop's Return sends `before` (BH-15). (5) Unassigned and Everything else stand only when they hold a card. (6) The modal's Up and Down only when the card is drawn on the tab; the page's moves from `allowed`. (7) The page shows a 400 naming `--as` as its refusal. (8) While any card write in the project is out nothing opens; a click is dropped, not delayed. (9) One hint line in the view head; Hand-offs show raw `<flow>/<column>`. `.board-grid-single` is `auto 1fr`. Iteration 2: the modal's state is the shown card's; Back with a dialog or menu over the modal closes both; the acting-name line is absolute; `focusTitle`'s Shift+Tab reaches the last stop; headers count no cancelled card; lanes in one pass; the line "C-0003 sent back"; Russian wording.

**Mutations**, each red: Rust M14-M17, M20, M22, M23 (M23 by a card-route pin read from source); UI M1-M13, M18, M19, M21, M24, M25, and R1-R7, one per iteration-2 fix.

**Size.** First-load JS 794.35 kB (794 345 B), gzip 225.62, against 768.96 at the base and the 845 bar; `ru-*.js` 137.88 kB; the Board stays in the first load.

**Looks, 2026-11-09**, at 1440x900 unless named; the rest in the criteria. Owner's eye: the board's head block puts the columns at y 296.5 of 900 on the mock.

| Module | What it does |
|---|---|
| core `card_toml.rs`, `card.rs` | `return` (`FLOW_KEYS` 7, `read_return`'s five errors); `card_move(…, note)`, `MoveRefusal::NoNote` |
| store `queue/cards.rs` | `reviewer` stored and judged; `card.created.reviewer`, `card.moved.note`; the `parent` filter |
| cli `card.rs`, `main.rs`; http `app.rs` | `--reviewer`, `--note`, `--parent`; `note_problem`, `MoveAsked`; the bodies; `?parent=` |
| mcp `cards.rs`, `hub.rs`, `mirror.rs` | the three keys, the texts, `return` in the flow schema, `NOTE_MAX` asserts |
| `plugin/`, `scripts/` | 0.31.0, `read-spec`'s sentence; the preset's `sent-back`; `.env.example` |
| ui `api/`, `app/` | `CardFlow.return`, the keys, `useCardWriting`; `cardPageHash`, `view: "page"`, the page in `Shell.tsx` |
| ui `board/` | the still grid, lanes (`swimlanes.ts`), notices; `CardModal`, `CardPage`, `CardBody`, `CardActs`; `HandOffDialog`, `ReturnDialog`; `rollup.ts` |
| ui `ui/`, `styles/`, `i18n/`, `mocks/` | `Dialog`'s `focusTitle`, `useViewTop`, icons, tokens; both catalogs; `plates`' `blocked`, children |
