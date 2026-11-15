---
class: spec
status: shipped
shipped: 2026-11-14
scope: [ui]
ref: RM-0117; the owner's report of 2026-11-14 from the hosted hub (0.46.0, his game project), after the coordinator's diagnosis; patch release 0.46.1
adrs: []
---

# inbox-closed-link: a link to a decided proposal shows that proposal (RM-0117)

## Why

On 2026-11-14 the owner followed a link to PR-100155, an applied question decided into DEC-100002, and the Inbox showed him a different proposal. `GET …/inbox` lists only `open` and `approved` items, and `InboxView.tsx` took `visible[0]` for a routed ID it did not list, with only a note: "PR-100155 is not in this inbox; it may have been decided. The first proposal is shown." A link to a decided item is the normal way back to it, from a card's Activity, a record or a chat. It must show that item.

## Description and interactions

A routed ID the inbox does not list is read alone, `useProposal` (`GET …/proposals/<id>`, any status, the same query an open item uses), and shown read-only in `ClosedProposal`, an `IssueView` with no decision bar: what was decided, by whom, when, in which commit and into which record. No list item is selected and the route stays. The exit-1 document says no proposal has the ID; a failed read shows its message and Retry. The view's own Confirm replaces the route with `#/<p>/inbox`; an item decided elsewhere while routed turns into its closed view. No ID, or a listed one, resolves as before. Canon: `docs/canon/proposal-review.md` "Inbox".

## Data

No daemon change. The pane reads the review document's `id`, `kind`, `status`, `target_id`, `target_ids`, `summary`, `gist`, `decided_by`, `decided_at`, `decision_note`, `applied_commit`, `record_id`, `record_path`, `record_title` (`fixtures/daemon-keys.json` `Proposal`). Routing in `InboxView.tsx`: `listed` when the inbox holds the routed ID; `selected` today's rule for a listed or absent ID, else null; `closedId` the routed ID once the inbox answered without it. `inbox.absent` is removed. New keys, both catalogs: `inbox.unknown` "{{id}}: no proposal has this ID in this project.", `inbox.closed.{notice, decidedBy, decidedAt, commit, record, targets, note, noDecision, reading}`.

## Rules and edge cases

An unlisted routed ID selects nothing and keeps the route; its document shows with no control that stages, confirms or decides; the exit-1 document shows no pane; served text is never markup (`shownText`, `shownSlot`); `useProposal` stays one query per ID.

## Acceptance criteria

All are vitest under `ui/` (`src/inbox/InboxView.test.tsx`, `ClosedProposal.test.tsx` where noted). The fixture: the inbox lists PR-0001 and PR-0002, both open; PR-0009's review is `applied`, `decided_by` `molof`, `decided_at` `2026-11-14T13:58:00Z`, `decision_note` `Use <b>the</b> working answer`, a 40-hex `applied_commit`, `record_id` `DEC-0002` with its path and title, `target_ids` `["MEC-STAMINA"]`. Evidence: the tests citing each ID; the UI review, ACCEPT after iteration 2; the whole UI suite, lint, typecheck and the build green, `ClosedProposal` a lazy chunk of 2.69 kB.

- [x] IC1 — **A routed applied proposal.** At `#/<p>/inbox/PR-0009`: the `h2` is its gist, the lozenge Applied; the six rows show their values, the note `<b>` literally; no Accept, Reject, Stage, Unstage or Confirm in the pane; no option `aria-selected="true"`, `replaceHash` not called. Mutation: `?? visible[0]` restored for an unlisted ID.
- [x] IC2 — **Rejected and withdrawn** (`ClosedProposal.test.tsx`). Rejected shows Rejected, no Commit or Record row; withdrawn shows its status raw if `statusLozenge` does not know it. Mutation: a null row drawn as an empty value.
- [x] IC3 — **An unknown ID.** The 404 exit-1 document: `inbox.unknown` with `PR-0404`, no pane, no selection, the route unchanged. Mutation: falling back to the first item.
- [x] IC4 — **A failed read.** A 503's message verbatim, Retry sends a second `GET …/proposals/PR-0009` whose 200 shows the pane; no selection throughout. Mutation: the failure falling back to the first item.
- [x] IC5 — **Keys.** From the closed state, `j` selects PR-0001 and replaces the route; the accept letter opens no dialog and announces `inbox.closed.noDecision` with `PR-0009`. Mutation: `openDialog` reached with the closed ID.
- [x] IC6 — **Unchanged routes.** `#/<p>/inbox` selects PR-0001, `#/<p>/inbox/PR-0002` PR-0002; PR-0002 routed and filtered out shows PR-0001, as before.
- [x] IC7 — **The view's own confirm** (hub mock). Confirming PR-0001: the route becomes `#/<p>/inbox`, after the re-read PR-0002 is selected and focused, the result line as before. Mutation: the route kept, so PR-0001's closed pane shows.
- [x] IC8 — **Decided elsewhere.** PR-0002 routed and selected; a live re-read drops it and its review re-reads applied: the pane becomes its closed view, the route stays. Mutation: the first item selected.
- [x] IC9 — **Strings and gates.** The keys in both catalogs, `inbox.absent` gone from both, `policy.test.ts`, the UI suite, lint, typecheck and the build pass.
- [x] IC10 — **[look]** The orchestrator's, 2026-11-14, Chrome at 1440x900 on `#/harbor-sim/inbox/PR-0056` of the mock: the Applied pane with its notice, Kind, Decided by, Decided at, Commit, Record and Targets, no decision bar, no list item selected. Cosmetic: a long "Decided by" address wraps with a lone `>` on its own line.

## Out of scope

A listed item hidden by the filter (the first visible item, as before); an "All" view or `inbox --all` on the daemon; decisions on a closed item; the ID's repository.

## Implementation

| Module | What it does |
|---|---|
| UI `src/inbox/ClosedProposal.tsx` (new, lazy) | the read-only pane: trail, title, status, notice, details, the note |
| UI `src/inbox/InboxView.tsx` | `listed`, `closedId`, no fallback selection, the keys from none, the letters' announcement, `confirmed` replacing the route |
| UI `src/inbox/ProposalList.tsx` | a list with no selection, its tab stop the first option |
| UI `src/inbox/ProposalCard.tsx` | `DriftSection`, `TerminalDecision` lazy, keeping the first load within budget |
| UI `src/i18n/`, `src/styles/app.css` | the keys, the pane's style |
| tests | `InboxView.test.tsx`, `ClosedProposal.test.tsx` (new), `drift.test.tsx`, `mocks/drift.smoke.test.tsx` |

**Departures**, accepted: a clear queue shows the closed pane beside its empty state; `ProposalCard` lazy-loads `DriftSection` and `TerminalDecision` (the `DriftItem` chunk, 3.82 kB); the Record link falls back to `record_path` when neither title nor ID is served; `<wbr>` after each `@` and `.` of the decider's address, for the look's lone `>`.

**Release**: the patch 0.46.1, `docs/features/board-subtasks.md` "Implementation".
