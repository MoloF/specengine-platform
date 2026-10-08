---
class: spec
status: draft
scope: [crates/specengine-http, crates/specengine-cli, ui]
ref: ui-round analysis 2026-10-08 (HEAD 4b9738c), the orchestrator's working answers and Q1-Q3 for the owner's review; 07 s3, 08 s2 Phase 4
---

# UI round

## Why

Open questions and discrepancies pile up while agents work on their working answers (ADR-0012); `round` takes answers only as a file in a grammar on a terminal. Here the owner answers them on one screen and sends them at once; only the N `[y/N]` keys stay for the terminal (`spec round answer --staged`; ADR-0035, ADR-0038). Each answer keeps the one-item guarantees: stale choices refused, the round's queue checks, refusals alone, one event per stage, no alert for this tab's writes, no daemon path that walks or decides.

## Description and interactions

Built: two routes (http `app.rs`, `answer.rs`); a CLI library call staging structured answers (`round.rs`, `Outcome`); the Questions section (`sections.ts` `questions`, `about` without "pasted"), client, types, mock, live re-read, own writes; `fixtures/daemon-keys.json`. No new ADR, storage, event type or grammar; core, store, MCP, plugin untouched.

Reused, factored: `round_new`; `stage_with` per answer; `plan`'s checks past its token step; round order (`order_key`, `Waiting` read once); `refuse`'s trimming; the duplicate rule; one clock reading. Unused: `parse_answers`, `round_answer`, the walk; `round_stage` stays for a paste. Not the grammar: `plan` stamps the `updated_at` read at POST time (a stale screen would replace a newer stage), and a form serialised loses meaning (own text `ok`, `2`, LF).

**Screen** `#/<project>/questions`, states per `ui/README.md` "Screen rules":

- Header from the JSON: `<n> items, about <minutes> min of reading`, `notes` verbatim; `more > 0`: "Show all" reads `limit` = items + `more` (at most 200). Empty: "No open questions or discrepancies.", the notes, the next step.
- Items in sheet order with the text sheet's fields (`round.md` "Sheet"; `kind` raw, a neutral badge; Concerns, Waiting as links), "Open in Inbox"; no worktree, branch, `gap_type`, author, evidence.
- Controls by structure, never a kind string: with options, an option / Reject with reason / Leave open; without, Take the working answer (when given) / Your answer / Reject / Leave open, the default; nothing preselected.
- Drafts in tab memory per project, keyed by ID and the `updated_at` shown, survive reorder and section switches, not a reload (said on screen). A newer `updated_at`: "Changed since you answered", left out until picked again; an item gone: draft dropped, noticed.
- "Stage <k> answers": one POST, disabled while pending or empty, naming picks that replace a staged choice. Then outcomes and a polite summary; refused drafts stay with the reason verbatim, staged ones clear; status 0: the sheet read again, drafts kept.
- A staged item: "Staged", `Staged <at>`, `StagedChoice`, Unstage (the DELETE); then ``Confirm on a terminal in <root>: `spec round answer --staged`; each answer is asked again there, one at a time``, Copy copying exactly the command, never the root; `spec approve|reject <ID>` only for `^PR-[0-9]{4,}$`.
- Live: read again on every `proposal.*`, `task.*` event and on a gap, a superseded read aborted. `startOwnWrite` per answer before the POST, ended by its outcome; an Unstage here registers its write from the stage the sheet shows.
- Agent text verbatim, not Markdown, bidi-isolated, controls `\u{..}` as on the sheet; fields count UTF-8 bytes against the caps, never cut.
- Mock: both routes; `?scenario=conflict`: the first POST refuses one answer as changed since read.

## Data

Provisional types cite `docs/canon/round.md` "JSON" and this section until the POST moves into the http README.

**`GET /api/projects/:p/round`**, query `limit` only (1-200, default 20). 200: `spec round new [--limit N] --json` byte for byte, uncut. 400, nothing run: `limit` not decimal, repeated, out of range (the handler gives the CLI's `--limit N: the limit is 1 to 200`, not the library's 503), another name. 404 unknown slug; 405 another method, HEAD too, `Allow: GET`; 503 cannot run. In the project's turn, refreshing the index; no event.

**`POST /api/projects/:p/round/answers`**:

```json
{"project":"example","answers":[
 {"id":"PR-0003","stage":{"decision":"approve","option":null,"answer":"no, 2 s","canon":null,"note":null,"updated_at":"2026-10-07T17:49:50Z"}},
 {"id":"PR-0005","stage":{"decision":"reject","reason":"dup","updated_at":"2026-10-07T17:40:02Z"}}]}
```

`stage` is the decision route's body (`StageBody`); the CLI library decodes the envelope, `deny_unknown_fields`. The screen sends the working answer as an approve with every flag `null`, own text as `answer`, an option as `option`; `note`, `canon` `null`. Statuses in order, nothing read or stored before 6:

1. the fence 403 (http README "Fence"); unknown slug 404;
2. no `Sec-Fetch-Site: same-origin`: 403; 3. not one `Content-Type` `application/json`: 415 (the decision route's messages);
4. over 4 194 304 bytes (W-6): 413 ``the body is over 4194304 bytes``, an over-cap `Content-Length` unread;
5. 400: undecodable (``spec: the answers are not `{"project","answers":[{"id","stage"}]}` as JSON: <why>; nothing changed``); another `project` (``the answers are for `<x>`, this is `<slug>`; nothing changed``); over 200 answers;
6. 200, the document, whatever the outcomes (`answered` by the outcome type, as `Outcome::Check`);
7. 503 cannot run. Another method: 405 `Allow: POST`, ``this path takes only POST (stage the round's answers); a staged choice is confirmed on a terminal``.

```json
{"project":"example","answers":[
 {"id":"PR-0003","outcome":"staged","stage":{"decision":"approve","option":null,"answer":"no, 2 s","canon":null,"note":null,"span_hash":null},"staged_at":"2026-10-08T09:12:40Z","replaced":null,"reason":null},
 {"id":"PR-0005","outcome":"refused","stage":null,"staged_at":null,"replaced":null,"reason":"`PR-0005` is rejected"}],"notes":[]}
```

Request order; `id` as sent; `stage` as stored (`decision-staging.md` "The stage"); `staged_at` the request's one clock reading; `replaced` the `staged_at` replaced; `reason` less `; nothing changed`, `; nothing staged`; `notes` a corrupt row, as `spec inbox` words it. Events: one `proposal.staged` `{"id","staged","staged_at"}` per staged answer, round order, nothing else.

## Rules and edge cases

- An answer is refused alone, by its first refusal: an ID in two entries (Latin form), each ``answered at positions 2 and 5: give one answer per item`` (10 named, then `and <k> more`), never last-wins; a look-alike ID, naming the Latin one; `no proposal`; `round.md` "Answers"'s queue checks, its words; `stage_with`'s (`decision-staging.md` "The stage").
- Apply steps 2-3 not ready: ``not applied (step <n>): <reason>; a reject still settles it`` (W-16); the text route keeps `` `reject: <reason>` still settles it``.
- WHEN an item is settled on a terminal between GET and POST, its answer SHALL be refused alone; a POST replacing a stage the terminal asks SHALL be refused there (step 7). Within the read's second a stage may replace another (`decision-staging.md` "Gap").
- No handler matches a round outcome variant or calls `round_answer`; the POST writes only stages.

## Working answers

The orchestrator's, for the owner's review (ADR-0012). Q1: no Print, Copy or paste of the sheet now. Q2: one click on a discrepancy stages the recommended option by number; `ok` stays the working answer. Q3: no "take every working answer" control.

W-1 structured answers. W-2 the body names `project`. W-3 200 once the batch runs. W-4 empty `answers`: 200, no event; over 200: 400. W-5 staged in round order, reported in request order. W-6 4 MiB: 200 worst legal answers (2 048 + 4 096 + 512 B, escaping doubled) ~ 2.7 MB; 16 384 refuses four 4 KB reasons. W-7 `limit` 20, at most 200. W-8 LF in own text and reasons. W-9 no note, canon. W-10 no batch unstage. W-11 drafts lost on reload. W-12 nothing preselected. W-13 not Markdown. W-14 byte counters; the daemon's refusal decides. W-15 the text sheet's fields. W-16 the decision route's and `round.md`'s words, step 2-3's tail without grammar. W-17 no review documents returned. W-18 no hotkeys.

## Risks

Exit 1 maps to 404, a library usage error to 503: map by outcome type. `door.rs` bans `Staged` in http `src`: the library renders. N events may beat the answer (250 ms polls): own writes first; Unstage tracking reads only the proposal query; `useLiveQueue` ignores `task.*`. UTF-16 length is not bytes. Raw JSON carries U+202E. CR: an approve stages, never applies. Any local process can stage 200 choices (ADR-0034). 200 `record_ready` runs in one turn, unmeasured.

## Acceptance criteria

Http tests per its README "Tests"; M: the mutation that must turn it red.

- [ ] R-01 - `fixtures/spec-a`, an open question, a discrepancy, an item with a waiting task: GET `round`, `?limit=1` byte-equal `spec round new [--limit 1] --json`; an empty queue: 200 (M: `limit` ignored).
- [ ] R-02 - `limit=0`, `201`, `x`, `limit=1&limit=2`, `depth=1`: each 400, named, no event (M: the range left to the library).
- [ ] R-03 - HEAD, POST, PUT on `round`: 405 `Allow: GET`; GET, PUT, DELETE on `round/answers`: 405 `Allow: POST`, ending `confirmed on a terminal`; an unknown path's 404 lists `round`.
- [ ] R-04 - Over-cap `Content-Length`, no `Sec-Fetch-Site`: 403; `text/plain`: 415; over-cap JSON: 413 unread; `{"answers":[{"id":"PR-0001"}]}`: 400; another slug, 201 answers: 400; nothing staged or logged (M: the body read before the origin check).
- [ ] R-05 - Shuffled: Q1 the working answer, Q2 own text with LF, D1 option 1, D2 a reject: 200, four staged, four `proposal.staged` in round order, no other event; `git status`, `HEAD`, refs unchanged (M: request order).
- [ ] R-06 - Two valid answers and: a stale `updated_at`, applied, held, another repository's, an orphan's approve, an `update`, `PR-9999`, a U+0420 look-alike, an ID twice, a 2 049-byte answer, a 4 097-byte reason, U+202E, CR, a blank reason, option 5 of 0-2, no option and own text on a discrepancy, a path-only question's approve: each refused alone, named; two staged; 200. Without `[decision_records]`: an approve `not applied (step 3): ...; a reject still settles it`, a reject staged (M: the last duplicate wins; one refusal fails all; `updated_at` read at POST).
- [ ] R-07 - An answer on a staged item: `replaced` the earlier `staged_at`, one event.
- [ ] R-08 - GET and POST wait for their project's turn; a queued GET whose client left is dropped unrun.
- [ ] U-01 - Loading, empty, error, the daemon's text verbatim. U-02 - Sheet order, fields, header, notes (M: re-sorted).
- [ ] U-03 - An unlisted kind with options gets option controls, none preselected (M: a `kind ===` branch; the recommendation preselected). U-04 - One POST, the exact W-1, W-2 body; refused drafts stay, staged clear.
- [ ] U-05 - Drafts survive reorder; a newer `updated_at` marks and leaves out; a vanished item's draft dropped, noticed (M: keyed by position).
- [ ] U-06 - The POST's N events and an Unstage here raise no alert; a terminal's stage does (M: own writes after the answer, none for Unstage).
- [ ] U-07 - Copy: exactly `spec round answer --staged`, never the root; per-item commands only for `^PR-[0-9]{4,}$`.
- [ ] U-08 - 1 025 Cyrillic letters: `2050/2048 bytes`, uncut (M: counting characters). U-09 - U+202E in a summary: `\u{202e}`, no neighbour reordered.
- [ ] U-10 - A `proposal.*`, `task.*` event reads the sheet again, a superseded read aborted. U-11 - Mock: normal, empty, error, slow, conflict. U-12 - `pnpm lint`, `build`, `test` pass, no console errors.
- [ ] T-01 - Http `round.rs`: R-01, R-02, R-04 to R-07; `methods.rs`, `projects.rs`: R-03; `turn.rs`: R-08.
- [ ] T-02 - `door.rs`: GET and a refused POST change nothing, a valid one only stages; `round_answer`, `RoundTerminal` banned, positive controls by path and bare name, negative for `round_new` and the staging call (M: the handler calls `round_answer` with an always-yes terminal).
- [ ] T-03 - After a daemon POST, library `round_answer --staged`, scripted `n`: exactly the staged items in round order, each ending `, staged) ...? [y/N]`, all kept.
- [ ] T-04 - `daemon_keys.rs`, `fixtures/daemon-keys.json`: sheet, item, target, task, answers document, entry (34 + k); `daemonKeys.test.ts` agrees (M: an item key dropped). T-05 - `no_domain.rs`, CLI `proposal_genre.rs` scan the new code; CLI `round.rs`: shown `updated_at`, round order, duplicates.
- [ ] T-06 - Vitest covers U-01 to U-11; `ui_policy`, `anonymity`, `doc_pointers` pass; every M red, counted.
- [x] D-01 - This draft; 07 s3, `decision-staging.md`, `round.md` name `ui-round` planned; `check` clean.
- [ ] D-02 - Shipped: http and ui READMEs, those canons, 08 s2, `CLAUDE.md` State say built; spec compacted.

## Out of scope

Paste; Print or Copy of the sheet; print tokens (Q1). Browser confirmation; the walk from the UI. `--note`, `--canon`; batch unstage. `update`, `create` items; clarification, defer; R and A records; an MCP tool; the `round` skill; the CR follow-up; queue schema, event types, core grammar.

## Open

1. ADR-0038 stays accepted; the owner's explicit review is open (`round` "Open" 1).
2. W-16, decided here: the route's step 2-3 refusal ends `a reject still settles it`; the shared check returns the bare reason, each front end its tail.
3. Q1-Q3, W-1 to W-18 await the owner's review.

## Implementation

Not started.
