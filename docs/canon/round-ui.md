---
class: canon
tier: 2
scope: [crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-14
---

# Round routes and screen

ADR-0035, ADR-0038 under ADR-0034 (`docs/canon/architecture.md#apply`, `#distribution`), built by `ui-round` (2026-10-08). The daemon serves the round's sheet and stages the owner's structured answers in one request; the browser prepares, the terminal confirms each with its own `[y/N]` (`spec round answer --staged`, `round.md` "Run"). Code: CLI `round.rs` (`round_stage_answers`, `RoundAnswersBody`, `round_limit`), http `app.rs` (`round`, `round_answers`), `answer.rs`; UI `src/round/`.

## Sheet route

`GET /api/projects/:p/round`, query `limit` only: 200, `spec round new [--limit N] --json` byte for byte, uncut (`round.md` "JSON"). 400, nothing run: another name, `limit` repeated or not decimal (`args.rs`'s words), outside 1-200 (`round_limit`: ``spec: --limit <n>: the limit is 1 to 200``, the one query 400 with `spec: `). 404 unknown slug (an unknown path's 404 lists `round` among the routes); 405 any other method, HEAD too, `Allow: GET`; 503 cannot run. In the project's turn (the index refreshed); no event.

## Answers route

`POST /api/projects/:p/round/answers`:

```json
{"project":"example","answers":[
 {"id":"PR-0003","stage":{"decision":"approve","option":null,"answer":"no, 2 s","canon":null,"editorial":null,"note":null,"updated_at":"2026-10-07T17:49:50Z"}},
 {"id":"PR-0005","stage":{"decision":"reject","reason":"dup","updated_at":"2026-10-07T17:40:02Z"}}]}
```

Each `stage` is the decision route's `StageBody` (`decision-staging.md` "Daemon"); `RoundAnswersBody::from_json` decodes the whole, `deny_unknown_fields`; the daemon keeps no decoder. In order, nothing read or stored on a refusal: the fence (403); the slug (404); `Sec-Fetch-Site: same-origin` (403) and one `Content-Type` `application/json` (415), the decision route's messages; over 4 194 304 bytes (`ROUND_BODY_MAX`: 200 answers at their caps, escaped) -> 413 ``the body is over 4194304 bytes``, a declared `Content-Length` over it unread; 400, the text escaped: ``spec: the answers are not `{"project","answers":[{"id","stage"}]}` as JSON: <why>; nothing changed``, ``spec: the answers are for `<x>`, this is `<slug>`; nothing changed``, ``spec: <n> answers; at most 200 at once, a sheet's most items: send the rest after; nothing changed``. Then one `round_stage_answers` call in the project's turn, one clock reading: 200 with the document whatever each answer became (`answer.rs`: `Outcome::RoundAnswers` by type, as `Outcome::Check`); 503 cannot run. An empty `answers`: 200, nothing staged. Another method: 405 `Allow: POST`, ``this path takes only POST (stage each answer of the round); a staged choice is confirmed on a terminal``.

```json
{"project":"example","answers":[
 {"id":"PR-0003","outcome":"staged","stage":{"decision":"approve","option":null,"answer":"no, 2 s","canon":null,"editorial":null,"note":null,"span_hash":null},"staged_at":"2026-10-08T09:12:40Z","replaced":null,"reason":null},
 {"id":"PR-0005","outcome":"refused","stage":null,"staged_at":null,"replaced":null,"reason":"`PR-0005` is rejected"}],"notes":[]}
```

Entries in request order, `id` as sent; `stage` as stored (`decision-staging.md` "The stage"); `staged_at` the request's clock; `replaced` the `staged_at` the planning read saw (one staged in the same second may go unnamed: "Gap"); `reason` without `; nothing changed`, `; nothing staged`; `notes` what reading the waiting tasks met (a corrupt task row, as the sheet notes it). Events: one `proposal.staged` `{"id","staged","staged_at"}` per staged answer, round order, nothing else. `fixtures/daemon-keys.json` pins `RoundSheet`, `RoundItem`, `RoundTarget`, `RoundTask`, `RoundAnswersOutcome`, `RoundAnswerEntry` (40 sets).

## Per answer

Each answer is refused alone, by its first refusal; the rest are staged in round order (`order_key`, `Waiting` read once), each by `stage_with` against the `updated_at` sent:

1. an ID in two or more entries by its Latin form (a look-alike equal to another entry's ID too): each ``answered at positions 2 and 5: give one answer per item``, 10 named, then ` and <k> more`; never last-wins;
2. a look-alike ID, naming its Latin form; `no proposal`;
3. `queue_checked`, shared with the text route (`round.md` "Answers", its words): another repository; an orphan's approve; applied, rejected, held; a change proposal; a path-only question's approve without `canon` (with one, `stage_with` judges); apply steps 2-3 read-only, the bare ``not applied (step <n>): <reason>`` plus this route's `; a reject still settles it` (the text route's: ``; `reject: <reason>` still settles it``); a corrupt row named: the store's own text;
4. `stage_with`'s (`decision-staging.md` "The stage"): changed since read, flags, a blank reason, caps, characters.

`round_stage` (a text, steps 2-4) stays for a later paste. No handler walks a round or decides: `tests/door.rs` bans `round_answer`, `RoundTerminal`, `WalkOutcome`, `AnswerOutcome` in http `src`; `tests/no_domain.rs` exempts `answers`, a `LINK_TYPES` name, only as a whole `/`-segment of a route-path literal.

## Screen

The Questions section (`#/<project>/questions`), over `getRound` and `stageRoundAnswers`:

- Header from the JSON (`<n> items, about <minutes> min of reading`), `notes` verbatim; `more > 0`: "Show all" reads `limit` = items + `more` (at most 200), remembered per project. Items in sheet order with the text sheet's fields, `kind` raw in a neutral badge, "Open in Inbox"; no worktree, branch, `gap_type`, author, evidence.
- Controls by structure, never a kind string: with options, an option (sent by number; the recommended one marked, never preselected) / Reject with reason / Leave open; without, Take the working answer (when given) / Your answer / Reject / Leave open, the default.
- The Shell holds a per-project store (`round/memory.ts`: drafts keyed by ID and the `updated_at` shown, the limit, the last outcome, refusals, a failure, dropped notices): it survives section switches, not a reload (said on screen). A newer `updated_at`: "Changed since you answered", left out until "Keep my answer for this version" or a new pick; an item gone: its draft dropped, a polite notice. Blank own text or reason is left out; text is trimmed before sending, its UTF-8 bytes counted as sent, never cut.
- A sticky Stage bar (max 6rem; `scroll-padding-bottom` keeps focus above it): the count, "Stage <k> answers" (one POST, none while one is pending: `useRoundPosting`), the picks that replace a staged choice, a one-line outcome focusing the details. Refused drafts stay with the reason verbatim, staged ones clear; status 0: the sheet read again, drafts kept. A failure with no Questions screen of the project mounted: assertive, `Questions of <project>: ` first.
- A staged item: "Staged", `Staged <at>`, `StagedChoice`, Unstage (the DELETE). One panel above the list: ``Confirm on a terminal in <root>: `spec round answer --staged`; each answer is asked again there, one at a time``; Copy copies exactly the command, never the root; `spec approve|reject <ID>` only for `^PR-[0-9]{4,}$`.
- Live: the sheet read again on every `proposal.*`, `task.*` event of the project and on a gap, a superseded read aborted. One `startOwnWrite` per answer before the POST, ended by its outcome; an Unstage here is tracked from the stage the sheet shows (`useStageDecision`'s `shown`).
- Agent text verbatim, not Markdown, wrapped, in `<bdi>`, controls and bidi characters written `\u{..}` (`shownSlot`; option labels too on the Inbox card, the decision dialog, `StagedChoice`).
- Mock: both routes; `?scenario=conflict`: the first POST refuses one answer as changed since read.

## Known limits

- An item pushed past the limit counts as gone: its draft is dropped (noticed) while `more > 0` still holds it.
- `Ready` caches only places that pass: without `[decision_records]`, 200 approves run `record_ready` 200 times in one turn (bounded, unmeasured).
- Any local process can stage 200 choices at once (ADR-0034); the terminal asks each whole.
- CR in agent text: its approve stages, never applies (`round.md` "Known limits").
