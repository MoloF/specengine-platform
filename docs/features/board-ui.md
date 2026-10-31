---
class: spec
status: draft
scope: [ui, crates/specengine-http, crates/specengine-cli]
ref: RM-0046; Phase 7 step 5 (08 s2)
adrs: [ADR-0056]
---

# board-ui: the Board section and the daemon's card routes (RM-0046)

## Why

The owner wants an ordinary kanban with statuses, transitions, deadlines and assignees, starting empty and filling from the brief or by hand, every card linking to its source. `card-store.md` holds the record. This slice draws it and gives the daemon its first writes beyond staging, for cards alone (ADR-0056), so one person runs the whole cycle on one screen. Confirming a brief change stays on a terminal (ADR-0035).

## Description and interactions

### Board

A new section `board`, third in the nav: `#/<p>/board[/<C>]`, the ID selecting a card and opening its flow. `getCards` once, `getCard` for the panel; `card.*` events read them again.

- **One board per flow.** A tab per `flows[]` in served order; one flow, no tabs. Columns are `columns[]`, with `title` and a count.
- **Filters**, per-project screen state: Discipline, Assignee (with "Unassigned"), ANDed, compared as `ui/README.md` "Text"; a Show cancelled toggle (`with_cancelled=true`). Past 200 cards a column ends "Show all <n>".
- **A card** shows title, assignee or "Unassigned", due, estimate and its source: the first target, a `routes.ts` link to the Spec tree, `+<n>` for the rest; no target: "By hand". Accepted and cancelled say so, with an icon.
- **Overdue.** Reads carry no clock, so `src/board/due.ts` alone holds one: `localToday` from `useNow`, by `getFullYear`, `getMonth`, `getDate`, never `toISOString`. Overdue is `due < today` as strings, unless accepted or cancelled: the label "Overdue" and an icon.

### Moving

Each card has a Move button opening `MoveMenu`: its column's `moves[]` in served order, each `by` as "needs <names>", then Up, Down (`before`), and Accept when the column is `accept.from`. A choice posts `moveCard` with the row's `revision`. The UI looks moves up and never judges them; the board redraws from the answer, nothing optimistic.

**Drag** is an extra on native HTML5 events, no new dependency (the allowlist stays 17): a drop posts the same `moveCard`, between two cards with `before`.

**Keyboard.** One tab stop, roving: Up, Down in a column; Left, Right across; Home, End. Enter opens the panel; `m` opens `MoveMenu`, Up, Down, Enter, Esc inside; `n` New card; Esc closes, focus back on the card; `?` lists them. Focus follows a moved card; a polite live region says `C-0001 moved to Ready`.

### Panel

Beside the board, routed by ID, text as stored:

- Goal, criteria, discipline, estimate, due, assignee, reviewer, created by and at; targets link to the tree, `parent` and `corrects` to their cards.
- **Evidence**: kind, value, by, at. A `url` is text, never an `href`.
- **History**: `history` by `seq`: time, the event's words from `labels.ts`, `as`, from and to; an unknown type raw.
- **Actions**: Edit, Assign, Cancel card, Accept. An accepted or cancelled card has none and shows `acceptance` or `cancellation`.

### Creating and editing

In-app dialogs (`ui/README.md` "Dialogs").

- **New card**: title, goal, discipline, targets and criteria one per line, assignee, estimate, due (`type="date"`), parent, corrects. A New card button on the tree's `NodePane` opens it with that node as target: how a board fills from the brief.
- **Edit**: the same less discipline and assignee, changed fields only, `-` for a cleared one; Add evidence, a kind and a value.
- **Assign**: assignee, reviewer. **Cancel card**: a required reason.

### The acting name, and acceptance

"Acting as", a field in the board's header, kept per viewer in `localStorage` `specengine.board.as` (`src/board/acting.ts`), sent as `as` on every write; empty sends `null`, recorded `local`.

**Accept** opens a dialog with that name and a note, disabled while the name is empty. The rules are the CLI's: in `accept.from`, the name its `reviewer`, never its `assignee`.

This is a convention until the hub's named login. The name is typed and nothing verifies it (ADR-0034). No MCP tool reaches the route, `specengine-mcp` holds no HTTP client, and there is no credential to hand an agent; but any local process can send these headers, so an agent with a shell can post here. Only `spec card accept` asks on a terminal.

### States

Loading: a skeleton. Failed: `ErrorPanel` verbatim, Retry; a bad `[card]` is that 503. Empty: the columns, and "No cards yet. Next: press New card, press New card on a node in the Spec tree, or have your AI call create_card."

### Files in scope

- **New:** `ui/src/board/{BoardView,BoardColumn,BoardCard,MoveMenu,CardPanel,CardDialogs}.tsx`, `{acting,due,labels,filter}.ts`, `BoardView.test.tsx`, `{acting,due,filter}.test.ts`; `ui/src/mocks/cards.ts`, `board.smoke.test.tsx`; http `tests/cards.rs`.
- **UI**, in `ui/src/`: `api/{provisional,types,client,http,queries}.ts`, `http.test.ts`, `daemonKeys.test.ts`, `queries.test.tsx`, `live.test.tsx`; `mocks/MockClient.ts`; `app/{sections,routes}.ts`, `routes.test.ts`, `App.test.tsx`, `Shell.tsx`, `shortcuts.tsx`; `tree/NodePane.tsx`; `ui/Icon.tsx`; `styles/app.css`; `policy.test.ts`.
- **Rust:** http `src/app.rs`, `src/answer.rs`; CLI `src/card.rs`, `src/lib.rs`.
- **Tests:** http `door.rs`, `fence.rs`, `methods.rs`, `query.rs`, `parity.rs`, `daemon_keys.rs`; `fixtures/daemon-keys.json`.
- **Unchanged:** `crates/specengine-mcp`, `queue/gate.rs`, `ui/package.json`.
- **Canon at shipping:** new `board-ui.md`; `architecture.md#ui`, `serve.md#trust` (any local process can also move, cancel and accept a card), `ui/README.md`, the http README; an ADR for the page's accept and clear, written then, owner pre-approved, review open: ADR-0056 names moves alone. It states that a departing page accept (RM-0047) creates one proposal through the daemon, which is why `card_writes_touch_only_the_queue` admits it.

## Data

### Routes

`...` is `/api/projects/:p`; a null `as` records `local`, as the CLI's absent flag.

| Method, path | Query or body | Answer |
|---|---|---|
| GET `.../cards` | `flow`, `column`, `discipline`, `assignee`, `no_target`, `with_cancelled` | `spec card list --json`, browser view |
| GET `.../cards/:id` | -- | `spec card show C --history --json`, uncut |
| POST `.../cards` | `create_card`'s keys, `as` | `card_new` |
| POST `.../cards/:id/move` | `move_card`'s, `as` | `card_move` |
| POST `.../cards/:id/edit` | `edit_card`'s, `as` | `card_edit` |
| POST `.../cards/:id/assign` | `assignee`, `reviewer`, `revision`, `as` | `card_assign` |
| POST `.../cards/:id/cancel` | `reason`, `revision`, `as` | `card_cancel` |
| POST `.../cards/:id/accept` | `as`, `note`, `revision` | `card_accept` |

**The door**, as `round_answers`, in order, nothing read on a refusal: the served slug (404), a same-origin page (403), JSON (415), `CARD_BODY_MAX` 1 MiB (413), a body the CLI decodes (`Card*Body`, `deny_unknown_fields`: 400). Then one library call in the project's turn. `card_accept`'s `Consent` is the page's dialog: the handler answers yes.

**Answers**: 200 the write document; 400 usage; 404 `{id, reason}`; 409 refused, a lost compare-and-set too, the write document, its reason the last of `notes`; 503 cannot run. `Outcome::Card` carries `CardCause` (`Usage`, `Unknown`, `Refused`), mapped as `Outcome::Stage`. **Methods**: `.../cards` `Allow: GET, POST`; `.../cards/:id` a read; the five `Allow: POST`.

### The fence stays

`fence()` does not change: the `127.0.0.1` bind, `Host` and `Origin` naming the bound port, `Sec-Fetch-Site`, no `Access-Control-*`. It runs before routing, so new routes are fenced. `same_origin()` guards every POST; its line becomes `a write needs a same-origin page (not authentication: ADR-0034)`.

`tests/door.rs` changes for cards alone:

1. `CARD_CALLS`, beside `TASK_READS`, lists `card_{list,show,new,move,edit,assign,cancel,accept}`: any other `card_` name is forbidden. `approve`, `reject`, `propose*`, `round_answer`, `import_state`, `export_*`, `init`, `index`, every `task_` writer stay forbidden.
2. The two GETs join the endpoints that change nothing.
3. New `card_writes_touch_only_the_queue`: a POST leaves `git status`, `HEAD`, refs, files and proposals alone and adds one `card.*` event; a refused one adds none.

`fence.rs` adds the routes. `no_domain.rs` stays: `with_cancelled` is one word, `accept`, `cancel` no task state.

### Types

In `provisional.ts`, citing `docs/canon/cards.md`.

```ts
interface CardFlow { name: string; disciplines: string[]; columns: { id: string; title: string }[];
  moves: { from: string; to: string; by: string[] }[]; accept: { from: string; to: string } }
interface CardList { flows: CardFlow[]; cards: CardRow[]; notes: string[]; truncated: boolean; omitted: number | null }
interface CardWrite { id: string; flow: string; column: string; position: number; revision: string;
  allowed: { to: string; by: string[] }[]; notes: string[] }
getCards(project, signal?): Promise<CardList>;
getCard(project, id, signal?): Promise<Card | CardNotFound>;
createCard(project, body): Promise<CardWrite>;  // moveCard, editCard, assignCard, cancelCard, acceptCard (project, id, body)
```

`CardRow`, `Card` copy `card-store.md` "Documents and events". `fixtures/daemon-keys.json` gains `CardList`, `CardFlow`, `CardColumn`, `CardMove`, `CardRow`, `Card`, `CardEvidence`, `CardAcceptance`, `CardCancellation`, `CardEvent`, `CardWrite`, `CardNotFound`, pinned by `daemon_keys.rs`, `daemonKeys.test.ts`. `QUEUE_EVENT_TYPES` gains the six `card.*`: 22.

## Rules and edge cases

- WHEN a `card.*` event arrives THEN list and card are read again; no outside-change alert, a card being no decision.
- WHEN a write answers 409 THEN an alert shows the daemon's reason, a dialog keeps its text, the board reads again.
- WHEN a row's column is outside `columns[]` THEN it stands in a last "Not in the flow" lane, with the daemon's note.
- `src/board/` branches on no column, flow, discipline or event word, `labels.ts` the one map, and reads no clock but in `due.ts`. A column named `blocked` is data, shown raw (ADR-0012).

## Acceptance criteria

- [ ] AC-01 — Over `fixtures/card-press`, `GET cards?with_cancelled=true`, `cards/C-0001` equal `spec card list --with-cancelled --json`, `show --history --json`, byte for byte, past the cap.
- [ ] AC-02 — Each POST from a same-origin page: 200, one `card.*`, `git status`, `HEAD`, files unchanged. M: a handler calling `task_claim` or an unlisted `card_` name turns `door.rs` red.
- [ ] AC-03 — Each POST with no `Sec-Fetch-Site`, `cross-site`, a foreign `Origin` or `Host`: 403, no event. M: a POST without `same_origin`.
- [ ] AC-04 — A stale `revision`, a move outside `moves`, an `as` outside `by`, accept by the assignee: 409 with the document. M: `Outcome::Card` falling to exit 1's 404.
- [ ] AC-05 — On one screen new, assign, move, accept end accepted, `acceptance.by` the typed name, which survives a reload; `specengine-mcp` names no `/api/`. M: the name dropped from the body.
- [ ] AC-06 — At `2026-03-01T23:30-08:00`, `due` `2026-03-01` is not overdue, `2026-02-28` is; accepted and cancelled never. M: `toISOString` in `due.ts`.
- [ ] AC-07 — Keys alone: `m`, Down, Enter move a card, focus on it, the live region speaks. M: a menu reachable by pointer alone.
- [ ] AC-08 — `copy`, `plates` give two tabs; filters AND; `blocked` looks as any column; `policy.test.ts` scans `src/board/`. M: a branch on a column word.
- [ ] AC-09 — Empty: columns and the line naming how to add a card. The panel lists fields, evidence, history; New card from a node links back to it. M: a `Card` key dropped from `daemon-keys.json`.

## Out of scope

Confirming a brief change on the page. A node's cards, rollup: RM-0047, RM-0048. Named login: RM-0049. Cards in the palette. An MCP tool for accept.

## Implementation
