---
class: canon
tier: 2
scope: [crates/specengine-http, crates/specengine-cli, ui]
owner: owner
reviewed: 2026-11-01
---

# Board: card routes and screen

ADR-0056, ADR-0057, built by `board-ui` (RM-0046): an ordinary kanban on the local daemon, whose card routes are its first writes beyond staging, so one person runs the whole cycle on one screen. Confirming a brief change stays on a terminal (ADR-0035). The record: `cards.md`; marks, clearing, verdicts: `spec-trace.md`, whose page half is here. Code: http `app.rs`, `answer.rs`; CLI `card.rs`; UI `src/board/`, `src/trace/`.

## Routes

`...` is `/api/projects/:p`. Body keys are the MCP arguments plus `as`, `card_id` left to the path; a key left out asks nothing.

| Method, path | Query or body | Answer |
|---|---|---|
| GET `.../cards` | `flow`, `column`, `discipline`, `assignee`, `no_target`, `with_cancelled`, `marks` | `spec card list [--marks] --json`, every row |
| GET `.../cards/:id` | -- | `spec card show C --history --json` |
| POST `.../cards` | `CardNewBody` | `card_new` |
| POST `.../cards/:id/move` | `CardMoveBody` | `card_move` |
| POST `.../cards/:id/edit` | `CardEditBody` | `card_edit` |
| POST `.../cards/:id/assign` | `CardAssignBody` | `card_assign` |
| POST `.../cards/:id/cancel` | `CardCancelBody` | `card_cancel` |
| POST `.../cards/:id/accept` | `CardAcceptBody`, a verdict's keys `briefed` or `departs`, `summary`, `observed`, `documented`, `gap_type` | `card_accept` |
| POST `.../cards/:id/clear` | `CardClearBody` `{outcome, reason, estimate, title, revision, as}`, `outcome` `no_effect`, `needs_correction` | `card_clear` |
| POST `.../trace/clear` | `CardClearGroupBody` `{group, reason, as}` | `card_clear_group`: `{group, cleared, notes}` |
| GET `.../trace` | `ref`, `stage`, `summary` | `spec trace --json`, browser view: every kept node, `truncated` false |

`card_list` keeps its signature; `CardListOutcome::in_view(View::Browser)` shows every row (`truncated` false, `omitted` null), so list parity with the CLI holds below the cap, and past it the daemon serves what the CLI cuts. `show` holds past it. Filters stay exact and untrimmed. **Methods**: `.../cards` `Allow: GET, POST`, `HEAD` a 405; `.../cards/:id`, `.../trace` reads; the six and `.../trace/clear` `Allow: POST`.

## The door

`written_target`, `written_body`, in order, nothing read or stored on a refusal: the served slug (404); `Sec-Fetch-Site: same-origin` (403, `same_origin`, ``a write needs a same-origin page (not authentication: ADR-0034)``); the path's ID decoded (400; `.../trace/clear` has none); JSON (415); at most `CARD_BODY_MAX` 1 MiB (413); `Card*Body::from_json`, `deny_unknown_fields`, the key `as`, its persons judged (`persons_named`, the flags' lines): 400. Then one library call in the project's turn, one clock reading. The `Consent` of `card_accept`, `card_clear` and `card_clear_group` is the page's dialog: the handler answers yes.

| Answer | When |
|---|---|
| 200 | the write document |
| 400 | a body or person refused; nothing asked, the CLI's line (`CardCause::Usage`) |
| 404 | `{id, reason}` |
| 409 | refused, a lost compare-and-set too: the write document, its reason the last of `notes` |
| 503 | cannot run: a `revision` off its grammar, a look-alike ID, a bad `[card]`, a corrupt row, a newer schema |

`answer.rs` `caused` maps `Outcome::Stage`, `Outcome::Card` and `Outcome::CardGroup` alike. A null `as` is `local`: on accept the reviewer rule refuses it, on a clear `clearer_problem`. A departing accept outside a worktree cannot run: 503.

## Fence and trust

The local fence does not change: the `127.0.0.1` bind, `Host` and `Origin` naming the bound port, `Sec-Fetch-Site`, any sent twice a 403, no `Access-Control-*`, `no-store`. It runs before routing, so these routes are fenced. A hub fences otherwise and lists five of these routes, the login as `as`; accept and clear wait for `hub-confirm` (`hub.md` "Routes").

**A convention until the hub's named login** (ADR-0057). The name is typed and nothing verifies it. `fence()` is the only guard, and any local process can send these headers, so an agent with a shell can post here under any name: `assign` a reviewer, then `accept` as that name, or `clear` a mark. `reviewer` and `by` stop slips only. No MCP tool reaches a route, and `specengine-mcp` holds no HTTP client. Only `spec card accept` and `spec card clear` ask on a terminal.

`tests/door.rs`: `CARD_CALLS` lists the ten `card_` calls and any other `card_` name is forbidden, beside `approve`, `reject`, `propose*`, `round_answer`, `import_state`, `export_*`, `init`, `index` and every `task_` writer. Handlers avoid the prefix (`new_card`, `move_card`, ...). The two GETs join the endpoints that change nothing. `card_writes_touch_only_the_queue`: a POST leaves `git status`, `HEAD`, refs, files and proposals alone and adds one `card.*` event, a refused one none. A departing accept is the one exception (ADR-0057): `card_accept` makes its discrepancy inside the library's accept, so no handler names `propose*`, and it adds `proposal.created` then `card.accepted`, git and the files as they were. `no_domain.rs` stays: `with_cancelled` is one word, and `accept`, `cancel` are no task state.

## Screen

Section `board`, third of nine: `#/<p>/board[/<C>]`, the ID selecting a card and opening its flow. One read of `getCards(project, options?, signal?)`, always `marks`, `with_cancelled` on Show cancelled; `getCard` for the routed panel; `card.*` events and gaps read both again, apply and decision events and window focus (a hand edit raises no event; `provider.tsx` brings `window` `focus` to `refetchOnWindowFocus`) the list. **Marks come from the list**: `rowMark` over its four served keys, so a card can never look normal while suspect or departed; an index that cannot be read is the read's 503, so `ErrorPanel` stands where a clean board would. The whole trace read (`useTrace`) serves the panel only, enabled while one is open, for each target's rollup word; `card.*`, apply and decision events and gaps read it again. Columns, moves and accept come from the read: the UI looks them up and never judges, and `labels.ts` is the one word map.

- **Flows and filters.** A tab per flow, none for one flow. Discipline and Assignee (with Unassigned) AND, per-project screen state. A column ends "Show all <n>" past 200.
- **A card** shows title, assignee, due, estimate and its source: the first target, a `routes.ts` link, `+<n>` for the rest; no target, "By hand". Accepted and cancelled say so, with an icon. A mark stands first on the tile: Suspect, Departed or Correcting, a label and an icon from `labels.ts`, naming the target, the discrepancy or the correction. An accepted word yields to it; a gone target shows its mark and Clear; an unknown mark word is a neutral badge, never a clean tile.
- **Overdue.** Reads carry no clock, so `due.ts` alone holds one: `localToday` by local date, overdue as `due < today` strings, never on a closed card; label and icon. Cards are memoised; the clock redraws only when the day turns.
- **Vocabulary.** A column named `blocked` is one more column. `src/board/` branches on no column, flow, discipline or event word (`policy.test.ts`).
- **Stranded.** A row outside `columns[]` stands in a last "Not in the flow" lane, with the daemon's note.

### Moving

`moveCard` has three doors: `MoveMenu`, listing its column's `moves[]` with each `by` as "needs <names>"; Up and Down, by `before`; native drag, no new dependency. A closed card has no Move. Accept shows only in `accept.from`. Side by side, columns stand over the free height (`min-height: 60vh`), so a drop anywhere in one lands, below every card last; a narrow screen stacks them with none.

### Writes

Each goes through `useCardWrite`, which settles after the re-read of lists, card, trace and flows, so nothing is optimistic; `clearCardGroup`, through `useCardClearGroup`, likewise. A 409 shows the daemon's reason, a dialog keeps its text and takes the new revision. Edit and Assign send only what differs from the form as opened, on the revision read now, so fields left alone keep another writer's value.

### Panel and dialogs

The panel lists every field; targets link to the tree, `parent` and `corrects` to their cards; evidence as kind, value, by, at, a `url` text and never an `href`; `history` by `seq`. Dialogs: New card, Edit (`-` for a cleared field, Add evidence), Assign, Cancel card with a required reason, Accept, Clear. New card on a node's Cards block presets the node as target; that block reuses Clear and adds Clear all, `ClearGroupDialog` (`spec-flow-screen.md` "Panel").

**Against the brief.** The panel shows a card's mark with its sentence and, on a suspect card, open or accepted, a Clear button; a correcting mark is held and takes none. A failed trace read is said in the panel, drops the targets' rollup words and keeps the mark, the list's.

**Accept** asks As briefed or Departs from it; a departure names direct targets, a summary, what was observed, what was documented and a gap, and a card with no target has none to name.

**Clear** takes No effect with a required reason, or, on an accepted card, Needs correction with an estimate and an optional title. The name stands last, and the dialog is the consent, as Accept's.

### The acting name

"Acting as", in the board's header, lives in `localStorage` `specengine.board.as`, sent trimmed as `as`, empty as `null`; a 400 naming `--as` shows under its field. The accept dialog is the consent. On a hub the field is read-only and shows the login (`hub.md` "Screen").

### Keyboard, states

One tab stop over the cards: arrows, Home, End; `m` the Move menu; `n` New card; Enter the panel; Esc closes, focus back on the card; `?` lists them. Focus follows a written card once its re-read settles, revealing a capped column or letting go, and a polite live region says so; after Accept from the panel it goes to the heading, from the Move menu it follows the card. A card moved past the cap reveals its column once.

Loading: a skeleton. Failed: `ErrorPanel` verbatim, Retry; a bad `[card]` is that 503. A failed list re-read does the same where the columns stood: no line, no tile from the last read, since a card may be suspect now. It takes the panel's mark and Clear and closes the Move menu and any drag. Empty: the columns, and how to add the first card. Mock scenarios `empty`, `conflict`, `large` reach the Board.

## Types

`provisional.ts`; `CardWrite`'s fields nullable, a refused `POST cards` having made no card. `fixtures/daemon-keys.json`: 107 sets, `CardList`, `CardFlow`, `CardColumn`, `CardMove`, `CardRow`, `Card`, `CardEvidence`, `CardAcceptance`, `CardCancellation`, `CardEvent`, `CardWrite`, `CardNotFound`, then `TraceDocument`, `TraceCounts`, `TraceNode`, `TraceCard`, `TraceUse`, `CardFrozen`, `CardClearGroupWrite`, `TraceStage`, `Identity`; `CardRow` 21 keys. `QUEUE_EVENT_TYPES` 25.

## Known limits

Drag is tested in jsdom only. No round trip against the real daemon in a browser yet. Not tested through the daemon: a corrupt card row, a chunked body over 1 MiB, `C` in `[ids]`.

## Tests

http `tests/cards.rs`, `trace.rs`, `door.rs`, `fence.rs`, `methods.rs`, `query.rs`, `parity.rs`, `round.rs`, `stage.rs`, `daemon_keys.rs`; UI `src/board/*.test.*`, `src/trace/*.test.*`, `mocks/board.smoke.test.tsx`, `policy.test.ts`.
