---
class: spec
status: shipped
shipped: 2026-10-31
scope: [ui, crates/specengine-http, crates/specengine-cli, crates/specengine-core]
ref: RM-0046; Phase 7 step 5 (08 s2)
adrs: [ADR-0056, ADR-0057]
---

# board-ui: the Board section and the daemon's card routes (RM-0046)

## Why

The owner wants an ordinary kanban with statuses, transitions, deadlines and assignees, starting empty and filling from the brief or by hand, every card linking to its source. `docs/canon/cards.md` holds the record. This slice draws it and gives the daemon its first writes beyond staging, for cards alone (ADR-0056), so one person runs the whole cycle on one screen.

## Summary

What is true now is `docs/canon/board-ui.md`; these headings stay because code cites them.

### Board

Section `board`, third of nine; a tab per flow; Discipline and Assignee filters; overdue by the viewer's local date. Canon "Screen".

### Moving

`MoveMenu`, native drag, Up and Down via `before`. Accept shows only in `accept.from`; a closed card has no Move. A card moved past the cap reveals its column once.

### Panel

Fields, evidence, history; a `url` is no `href`. After Accept from the panel focus goes to the heading; from the Move menu it follows the card.

### Creating and editing

In-app dialogs; New card on the tree's `NodePane` presets the node as target. Edit and Assign send only what differs from the form as opened, on the revision read now; fields left alone keep another writer's value.

### The acting name, and acceptance

"Acting as", `localStorage` `specengine.board.as`; the accept dialog is the consent. A convention until the hub's named login (ADR-0057).

### Data

#### Routes

Two GETs and six POSTs behind one door. Canon "Routes", "The door".

#### The fence stays

`fence()` is unchanged; `same_origin()` guards every POST. `door.rs` gains `CARD_CALLS` and `card_writes_touch_only_the_queue`. Canon "Fence and trust".

#### Types

`getCards(project, options?, signal?)`; `CardWrite` nullable; `daemon-keys.json` 98 sets. Canon "Types".

### Rules and edge cases

Nothing optimistic; a 409 shows the daemon's reason; no branch on a column word. Canon "Screen".

## Acceptance criteria

- [x] AC-01 — Over `fixtures/card-press`, `GET cards?with_cancelled=true` equals `spec card list --with-cancelled --json` byte for byte below the cap, and past it the daemon serves every row; `cards/C-0001` equals `show --history --json`, past the cap too.
- [x] AC-02 — Each POST from a same-origin page: 200, one `card.*`, `git status`, `HEAD`, files unchanged. M: a handler calling `task_claim` or an unlisted `card_` name turns `door.rs` red.
- [x] AC-03 — Each POST with no `Sec-Fetch-Site`, `cross-site`, a foreign `Origin` or `Host`: 403, no event. M: a POST without `same_origin`.
- [x] AC-04 — A stale `revision`, a move outside `moves`, an `as` outside `by`, accept by the assignee: 409 with the document. M: `Outcome::Card` falling to exit 1's 404.
- [x] AC-05 — On one screen new, assign, move, accept end accepted, `acceptance.by` the typed name, which survives a reload; `specengine-mcp` names no `/api/`. M: the name dropped from the body.
- [x] AC-06 — At `2026-03-01T23:30-08:00`, `due` `2026-03-01` is not overdue, `2026-02-28` is; accepted and cancelled never. M: `toISOString` in `due.ts`.
- [x] AC-07 — Keys alone: `m`, Down, Enter move a card, focus on it, the live region speaks. M: a menu reachable by pointer alone.
- [x] AC-08 — `copy`, `plates` give two tabs; filters AND; `blocked` looks as any column; `policy.test.ts` scans `src/board/`. M: a branch on a column word.
- [x] AC-09 — Empty: columns and the line naming how to add a card. The panel lists fields, evidence, history; New card from a node links back to it. M: a `Card` key dropped from `daemon-keys.json`.

## Verification

Delta review: ACCEPT. Gates: fmt, clippy, probes 0; `cargo nextest run --workspace` 4238 run, 4238 passed, 71 skipped; UI lint 0, vitest 2960 passed (a last wording edit follows), build 0; parity 3 of 3; 86 of 86 doc tests.

Mutations, first round, 7 of 8 killed: AC-02 (`door.rs`), AC-03 (`same_origin`), AC-04, AC-05's Rust half, `in_view` dropped, `or_usage_error` dropped, AC-07. AC-08's survivor, `lane.column.id !== "review"`, closed by adding `id`, `title`, `key` to the scan of `policy.test.ts`. AC-06 shown by the developer; AC-09 skipped. The developer's own 13 mutations of iteration 2 killed.

## Out of scope

Confirming a brief change on the page. A node's cards, rollup: RM-0047, RM-0048. Named login: RM-0049. Cards in the palette. An MCP tool for accept.

## Implementation

| Module | What it does |
|---|---|
| CLI `card.rs` | `CardCause::Usage`, and `exit()` keys on the cause (usage 2, unknown and refused 1). `move`, `edit`, `assign` asking nothing return that outcome, document `{id, reason}`, so MCP's `without_no_card` drops it and the tool result is unchanged; `or_usage_error` turns it back into a `CliError` in `main.rs` `run_card`, so the terminal prints one line and no stdout, `--json` too |
| CLI `card.rs`, lists | `CardListOutcome::in_view(View)` shows every row: `card_list` keeps its signature, so MCP is untouched and the handler calls a listed name |
| CLI `card.rs`, bodies | `Card{New,Move,Edit,Assign,Cancel,Accept}Body`: `from_json` (`deny_unknown_fields`, the key `as`, `card_id` left to the path) and `request`. A null `as` on accept is `local`, refused by the reviewer rule. Each `from_json` judges its persons (`persons_named`) with the flags' lines: a 400 before any call |
| http `app.rs` | `.../cards` GET and POST (`Allow: GET, POST`, HEAD a 405), `.../cards/{id}` a read, five POSTs (`Allow: POST`); `CARD_BODY_MAX` 1 MiB; `written_target`, `written_body` hold the door in order; accept's consent answers yes. `same_origin`'s line is `a write needs a same-origin page (not authentication: ADR-0034)`; `fence()` is unchanged. Handler names avoid the `card_` prefix `door.rs` reserves |
| http `answer.rs` | `caused` maps `Outcome::Stage` and `Outcome::Card` alike: 200, 400 the CLI's line, 404, 409. A `revision` off its grammar and a look-alike ID stay `CliError`: 503 |
| Core `card.rs`, `card_toml.rs` | Persons: `card_person_problem`, `card_person`, `CARD_PERSON_MAX` 128, beside `reviewer_problem`. `actor_of`, the CLI's `person_problem` and `card_toml.rs` `read_by` take it instead of `author_field_problem`, which keeps its callers unchanged. Every name is stored and compared trimmed. `CardCause::Usage` stays nothing asked |
| UI `src/board/` | One read of `getCards` (`with_cancelled` on Show cancelled), `getCard` for the routed panel; `card.*` events and gaps read both again. Columns, moves and accept come from the read: the UI looks them up and never judges, and `labels.ts` is the one word map |
| UI writes | Each goes through `useCardWrite`, which settles after the re-read, so nothing is optimistic; a 409 shows the daemon's reason, a dialog keeps its text and takes the new revision. `moveCard` has three doors: `MoveMenu`, native drag, Up and Down via `before`. Focus follows a written card once its re-read settles, revealing a capped column or letting go. Edit and Assign send only what differs from the form as opened, on the revision read now. Cards are memoised; the clock redraws only when the day turns |
| UI keys, clock, name | One tab stop over the cards; `m`, `n`, Enter, Esc, `?`. `due.ts` alone holds a clock: `localToday` by local date, overdue as `due < today`, never on a closed card. The acting name lives in `localStorage` `specengine.board.as`, sent trimmed as `as`, empty as `null`; a 400 naming `--as` shows under its field. The accept dialog is the consent; nothing verifies the name (ADR-0034). New card on `NodePane` presets the node as target |
| Keys | `fixtures/daemon-keys.json`: 98 sets |

### Files in scope

- **New:** `ui/src/board/{BoardView,BoardColumn,BoardCard,MoveMenu,CardPanel,CardDialogs}.tsx`, `{acting,due,labels,filter}.ts`, `BoardView.test.tsx`, `redraw.test.tsx`, `{acting,due,filter}.test.ts`; `ui/src/mocks/cards.ts`, `board.smoke.test.tsx`; http `tests/cards.rs`.
- **UI**, in `ui/src/`: `api/{provisional,types,client,http,queries}.ts`, `http.test.ts`, `daemonKeys.test.ts`, `queries.test.tsx`, `live.test.tsx`; `mocks/MockClient.ts`; `app/{sections,routes}.ts`, `routes.test.ts`, `App.test.tsx`, `Shell.tsx`, `shortcuts.tsx`; `tree/NodePane.tsx`; `ui/Icon.tsx`, `ui/useNow.ts`; `styles/app.css`; `policy.test.ts`.
- **Rust:** http `src/app.rs`, `src/answer.rs`; CLI `src/card.rs`, `src/lib.rs`, `src/main.rs`; core `src/card.rs`, `src/card_toml.rs`.
- **Tests:** http `door.rs`, `fence.rs`, `methods.rs`, `query.rs`, `parity.rs`, `round.rs`, `stage.rs`, `daemon_keys.rs`; `fixtures/daemon-keys.json`.
- **Unchanged:** `crates/specengine-mcp`, `queue/gate.rs`, `ui/package.json`.

**Divergences from the draft.** A column past 200 ends "Show all <n>", as drafted, and the Board filters on the client: `with_cancelled` is the one query it sends. AC-01 as drafted held only below the cap: the daemon serves every row past it. `CardWrite` is nullable, a refused `POST cards` having made no card. A person is `card_person_problem`'s, any script, not `author_field_problem`'s (ADR-0057).

**Known limits.** Drag is tested in jsdom only. No round trip against the real daemon in a browser yet. Not tested through the daemon: a corrupt card row, a chunked body over 1 MiB, `C` in `[ids]`.
