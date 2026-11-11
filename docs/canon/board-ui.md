---
class: canon
tier: 2
scope: [crates/specengine-http, crates/specengine-cli, ui]
owner: owner
reviewed: 2026-11-09
---

# Board: card routes, door and fence

ADR-0056, ADR-0057, built by `board-ui` (RM-0046): an ordinary kanban on the local daemon, whose card routes are its first writes beyond staging, so one person runs the whole cycle on one screen. Confirming a brief change stays on a terminal (ADR-0035). The record: `cards.md`; marks, clearing, verdicts: `spec-trace.md`, whose page half is `board-screen.md`. Code: http `app.rs`, `answer.rs`; CLI `card.rs`; UI `src/board/`, `src/trace/`.

## Routes

`...` is `/api/projects/:p`. Body keys are the MCP arguments plus `as`, `card_id` left to the path; a key left out asks nothing.

| Method, path | Query or body | Answer |
|---|---|---|
| GET `.../cards` | `flow`, `column`, `discipline`, `assignee`, `no_target`, `with_cancelled`, `marks`, `parent` | `spec card list [--marks] --json`, every row |
| GET `.../cards/:id` | -- | `spec card show C --history --json` |
| POST `.../cards` | `CardNewBody` | `card_new` |
| POST `.../cards/:id/move` | `CardMoveBody` | `card_move` |
| POST `.../cards/:id/edit` | `CardEditBody` | `card_edit` |
| POST `.../cards/:id/assign` | `CardAssignBody` | `card_assign` |
| POST `.../cards/:id/cancel` | `CardCancelBody` | `card_cancel` |
| POST `.../cards/:id/items` | `CardRaiseBody` | `card_raise`: the intake document (`card-questions.md` "The card door") |
| POST `.../cards/:id/accept` | `CardAcceptBody`, a verdict's keys `briefed` or `departs`, `summary`, `observed`, `documented`, `gap_type` | `card_accept` |
| POST `.../cards/:id/clear` | `CardClearBody` `{outcome, reason, estimate, title, revision, as}`, `outcome` `no_effect`, `needs_correction` | `card_clear` |
| POST `.../trace/clear` | `CardClearGroupBody` `{group, reason, as, count?}` | `card_clear_group`: `{group, cleared, notes}` |
| GET `.../trace` | `ref`, `stage`, `summary` | `spec trace --json`, browser view: every kept node, `truncated` false |

`card_list` keeps its signature; `CardListOutcome::in_view(View::Browser)` shows every row (`truncated` false, `omitted` null), so list parity with the CLI holds below the cap, and past it the daemon serves what the CLI cuts. `show` holds past it. Filters stay exact and untrimmed; each key at most once, a second a 400; a `parent` off the card-ID grammar a 503 (`card-flows.md` "Children"). **Methods**: `.../cards` `Allow: GET, POST`, `HEAD` a 405; `.../cards/:id`, `.../trace` reads; the seven and `.../trace/clear` `Allow: POST`.

## The door

`written_target`, `written_body`, in order, nothing read or stored on a refusal: the served slug (404); `Sec-Fetch-Site: same-origin` (403, `same_origin`, ``a write needs a same-origin page (not authentication: ADR-0034)``); the path's ID decoded (400; `.../trace/clear` has none); JSON (415); at most `CARD_BODY_MAX` 1 MiB (413); `Card*Body::from_json`, `deny_unknown_fields`, the key `as`, its persons judged (`persons_named`, the flags' lines): 400. Then one library call in the project's turn, one clock reading. The `Consent` of `card_accept`, `card_clear` and `card_clear_group` is the page's dialog: the handler answers yes.

| Answer | When |
|---|---|
| 200 | the write document |
| 400 | a body or person refused; nothing asked, the CLI's line (`CardCause::Usage`) |
| 404 | `{id, reason}` |
| 409 | refused, a lost compare-and-set too: the write document, its reason the last of `notes` |
| 503 | cannot run: a `revision` off its grammar, a look-alike ID, a bad `[card]`, a corrupt row, a newer schema |

`answer.rs` `caused` maps `Outcome::Stage`, `Outcome::Card` and `Outcome::CardGroup` alike. A null `as` is `local`: on accept the reviewer rule refuses it, on a clear `clearer_problem`. A departing accept outside a worktree cannot run: 503. A card's raise answers as the intake: `card-questions.md` "The card door".

## Fence and trust

The local fence does not change: the `127.0.0.1` bind, `Host` and `Origin` naming the bound port, `Sec-Fetch-Site`, any sent twice a 403, no `Access-Control-*`, `no-store`. It runs before routing, so these routes are fenced. A hub fences otherwise, the login as `as`: the six plain writes for any login, accept, clear and `trace/clear` for a `[team]` login, with `revision` or `count` required (`hub-team.md`).

**A convention until the hub's named login** (ADR-0057). Locally the name is typed and nothing verifies it. `fence()` is the only guard, and any local process can send these headers, so an agent with a shell can post here under any name: `assign` a reviewer, then `accept` as that name, `clear` a mark, or ask a question on a card. `reviewer` and `by` stop slips only. No MCP tool reaches a route, and `specengine-mcp` holds no HTTP client. Only `spec card accept` and `spec card clear` ask on a terminal.

`tests/door.rs`: `CARD_CALLS` lists the eleven `card_` calls and any other `card_` name is forbidden, beside `approve`, `reject`, `propose*`, `round_answer`, `import_state`, `export_*`, `init`, `index` and every `task_` writer. Handlers avoid the prefix (`new_card`, `move_card`, ...). The two GETs join the endpoints that change nothing. `card_writes_touch_only_the_queue`: a POST leaves `git status`, `HEAD`, refs, files and proposals alone and adds one `card.*` event, a refused one none. Two exceptions, neither handler naming `propose*`, git and the files as they were: a departing accept (ADR-0057), `card_accept` making its discrepancy inside the library's accept, adds `proposal.created` then `card.accepted`; a card's raise (ADR-0077), `card_raise`, adds the item's one `proposal.created`, its `as` trailing. `no_domain.rs` stays: `with_cancelled` is one word, and `accept`, `cancel` are no task state.

## Screen

The Board's screen, the card modal and page, lanes, the hand-off and return dialogs and the rollup: `board-screen.md`.

## Types

`provisional.ts`; `CardFlow.return` a column or `null`; `CardCreateBody.reviewer`, `CardMoveBody.note`, `CardsOptions.parent` optional; `CardWrite`'s fields nullable, a refused `POST cards` having made no card. `fixtures/daemon-keys.json`: 107 sets, `CardList`, `CardFlow`, `CardColumn`, `CardMove`, `CardRow`, `Card`, `CardEvidence`, `CardAcceptance`, `CardCancellation`, `CardEvent`, `CardWrite`, `CardNotFound`, then `TraceDocument`, `TraceCounts`, `TraceNode`, `TraceCard`, `TraceUse`, `CardFrozen`, `CardClearGroupWrite`, `TraceStage`, `Identity`; `CardRow` 21 keys. `QUEUE_EVENT_TYPES` 25.

## Known limits

Drag: `board-screen.md` "Known limits". Not tested through the daemon: a corrupt card row, a chunked body over 1 MiB, `C` in `[ids]`.

## Tests

http `tests/cards.rs`, `trace.rs`, `door.rs`, `fence.rs`, `methods.rs`, `query.rs`, `parity.rs`, `round.rs`, `stage.rs`, `daemon_keys.rs`; UI `src/board/*.test.*`, `src/trace/*.test.*`, `mocks/board.smoke.test.tsx`, `policy.test.ts`.
