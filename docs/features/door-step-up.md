---
class: spec
status: shipped
shipped: 2026-11-14
scope: [ui, docs, scripts/deploy]
ref: RM-0115; the owner's report of 2026-11-14 from the hosted hub (0.46.0, Authelia 4.39.28 behind Traefik forward-auth), with the door's log line; patch release 0.46.1
adrs: []
---

# door-step-up: a step-up refusal says what to do and links the door (RM-0115)

## Why

On 2026-11-14 the owner confirmed a staged proposal on the hosted hub with a session signed in by password alone. The door refused the confirm read: `... (method GET) is not authorized to user molof, responding with status code 401 with location redirect to https://login.../?rd=<the API URL>&rm=GET`. The confirm dialog showed the 401's body verbatim, as text, under "The daemon refused to confirm it.": `<a href="https://login.../?rd=...confirm&amp;rm=GET">401 Unauthorized</a>`. It was not clickable, and its `rd` led back to the API URL, not the page. The canon had assumed the door redirects, which the page sees as status 0, "No answer"; it was wrong.

## Description and interactions

On the confirm class's four acts (Confirm's read and write, a card's Accept and Clear, a node's Clear all) a 401 is the door's: the daemon never answers one. The refusal block then says "This action needs the second factor", what to do, and links "Open the login page" in a new tab, on the door's `https:` origin with `rd` the page's own `location.href`. No byte of the body is shown or announced. Retry is the dialog's existing control: Confirm's Read again, a write dialog's submit with its fields kept. Status 0 keeps today's words and adds one line, no link. Every other status keeps the daemon's words. The door sends no mail: a first registration of a second factor takes the operator's `admin code`. Canon: `docs/canon/hub-confirm.md` "Screen", "Known limits"; `hub.md` "Known limits"; `hub-deploy.md` "Gate"; `scripts/deploy/README.md` "Accounts", "Limits"; `ui-client.md` "The door's link".

## Data

The observed answer: `HTTP/1.1 401 Unauthorized`, `Content-Type: text/html; charset=utf-8`, the body one anchor, `<a href="https://login.example.org/?rd=https%3A%2F%2Fhub.example.org%2Fapi%2Fprojects%2Fdorm%2Fproposals%2FPR-100048%2Fconfirm&amp;rm=GET">401 Unauthorized</a>`. `request()` keeps it as `ClientError {status: 401, message: <the body>}`.

`ui/src/api/door.ts`: `DoorRefusal = {kind: "door"; link: string | null} | {kind: "noAnswer"}`; `doorRefusalOf(error, here)` (401 door, 0 noAnswer, else null); `doorLink(body, here)`: the first double-quoted `href` inside an `<a` tag, by a regular expression, never the DOM; `new URL(raw)` with no base; `https:` and no credentials, else null; `` `${origin}/?rd=${encodeURIComponent(here)}` ``. The observed body with `here` `https://hub.example.org/#/dorm/inbox/PR-100048` gives `https://login.example.org/?rd=https%3A%2F%2Fhub.example.org%2F%23%2Fdorm%2Finbox%2FPR-100048`; `http:`, relative, `javascript:`, credentialed, `Unauthorized` or empty give null. Strings `ui.stepUp.{title, text, open, noAnswer}`, both catalogs.

## Rules and edge cases

A 401 on the four acts shows the notice, never the body, the unmounted confirm announcing the title; no usable `href`, no link, never an empty one; status 0 its words plus one line; any other status verbatim; no body rendered as HTML (`policy.test.ts`); the link built when the refusal shows.

## Acceptance criteria

All but DS10 are vitest under `ui/`, the 401 mocked at `fetch` with the body above. Evidence: the tests citing each ID (`api/door.test.ts`, `inbox/ConfirmDialog.test.tsx`, `board/stepUp.test.tsx`); the UI review, ACCEPT after iteration 2; the whole UI suite, lint, typecheck, the build and `doc_pointers` green.

- [x] DS1 — **`doorLink`** gives every row of "Data" exactly. Mutations: the `https:` check removed; `here` not encoded; the whole `href` returned.
- [x] DS2 — **`doorRefusalOf`.** A 401 gives `door`, with or without a link; 0 `noAnswer`; 403, 404, 409, 503 null. Mutations: 401 not recognized; 403 recognized.
- [x] DS3 — **The confirm read.** The alert holds the title and the text; its one link reads "Open the login page" with the expected `href`, `target="_blank"`, `rel` holding `noopener` and `noreferrer`; no `401 Unauthorized`, `&amp;` or `/api/projects/` in the dialog; Confirm off, Read again with a 200 shows the question and turns it on. Mutation: the alert renders `message`.
- [x] DS4 — **The confirm write.** A 401 after a 200 read: the same notice, Confirm off until Read again; unmounted first, the announcer holds the title and no body byte. Mutation: the announcement passes `message`.
- [x] DS5 — **No answer on confirm.** Status 0 shows "No answer: read again to see whether it was confirmed." and the added line, no link. Mutations: a link on status 0; the line missing.
- [x] DS6 — **The three write dialogs** (`AcceptCardDialog` with a departure, `ClearCardDialog` with a reason, `ClearGroupDialog`): the notice and link, no body byte, the fields kept, the submit on; a second submit's 200 closes the dialog. Mutation: one dialog on today's verbatim refusal.
- [x] DS7 — **No usable link.** Bodies `Unauthorized`, the `http` one and the relative one show the notice with no `<a>`. Mutation: an anchor with an empty `href`.
- [x] DS8 — **Other refusals unchanged.** A 403 `refused: …` and a 409 with a review document keep today's heading and words. Mutation: every refusal a step-up.
- [x] DS9 — **Strings, comments, gates.** The four keys in both catalogs; the comments in `http.ts` (`getConfirm`) and `ConfirmDialog.tsx` describe the observed 401; `doc_pointers`, the UI suite, lint, typecheck and the build pass.
- [ ] DS10 — **[owner, live hub, after 0.46.1]** As a one-factor session: Confirm on a staged item shows the notice and the link opens the door in a new tab; after the second factor it lands on `#/<p>/inbox/<PR>`, Read again in the first tab reads the question and Confirm commits; the same on a card's Accept, its departure kept; a first registration takes `admin code`'s code. A door that returns at once without asking the factor is a finding for this spec.

## Out of scope

A 401 on any other route (`ErrorPanel`, other dialogs: verbatim, a candidate follow-up); the `Location` header; a door under a path prefix; a mock scenario; any daemon, deploy or Authelia change.

## Implementation

| Module | What it does |
|---|---|
| UI `src/api/door.ts` (new) | `doorLink`, `doorRefusalOf` |
| UI `src/ui/Refusal.tsx` (new) | the one refusal block: the step-up notice and link, status 0's line, else the daemon's words |
| UI `src/inbox/ConfirmDialog.tsx`, `src/board/CardDialogs.tsx` | the four acts' refusals through `Refusal`, the write dialogs keeping the `ApiError` |
| UI `src/api/http.ts`, `src/i18n/`, `src/styles/app.css` | the observed-401 comment, `ui.stepUp.*`, the notice's style |
| tests | `api/door.test.ts`, `inbox/ConfirmDialog.test.tsx`, `confirm.live.test.tsx`, `api/confirm.http.test.ts`, `board/stepUp.test.tsx`, `test/doorStub.ts`, `policy.test.ts` |

**Departures**, accepted: the keys are `ui.stepUp.*`; the tests' fixtures are project `alpha` and `PR-0007`; `"link" in door` narrows the type; `rel` joins `policy.test.ts`'s NOT_SHOWN list; the regular expression is `/<a\s[^>]*?(?<![\w-])href="([^"]*)"/i`, so `data-href` is never taken; `Refusal` is static in the card dialogs, so a status-0 refusal never fetches a chunk; a status 0 after the confirm dialog closed announces today's words alone; the door opens in a new tab, its `href` the one in `ui/src` not from `routes.ts` (`docs/canon/ui-client.md`).

**Release**: the patch 0.46.1, `docs/features/board-subtasks.md` "Implementation".
