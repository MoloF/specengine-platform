---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-15
---

# UI client: reads, live re-reads, stored choices and the one outside link

The client half of `ui/README.md` "Contract seam" and "Screen rules", kept here since the README holds to its 8 192 B (`ui_policy.rs`). Live re-reads built by `hub-turn-fairness` (RM-0119), the door's link by `door-step-up` (RM-0115), no ADR. Code: UI `src/api/client.ts`, `http.ts`, `queries.ts` (`useLiveQueue`), `coalesce.ts`, `door.ts`.

## Reads

Reads take the query's `AbortSignal`, but the check and health reads, which take none; writes take none, a write's answer always taken. A key change (a new search, another node) or a person's Retry, Read again or Check again aborts the superseded read: TanStack's own rule.

## Live re-reads

`useLiveQueue` re-reads what an event of the project's tail touches (`proposal.*`, `card.*`, `task.*`, an apply, a gap: `docs/features/ui-live.md`). None aborts a read in flight: on a hub every read waits in the project's one turn (`serve.md` "Concurrency"), and an aborted read's replacement joins the back of the line, so under a steady burst the page never reaches the head. Every such re-read goes through `coalesce.ts` `readAgainKept`:
- no read in flight: one at once (off screen only marked stale);
- one in flight: kept in its place, and exactly one more once it settles, answered or failed, however many events land meanwhile: at most one read in flight and one owed per query.

Activity's feed (`spec-feed.md` "UI") is re-read by every event and a gap, through the same `readAgainKept`: an event's read asks `since` the highest `seq` held and appends its answer; a gap's reads it whole.

Two keep their own rules (`health.md` "UI"): the health read owes one trailing read, its debt the subscription's, dropped by its cleanup; the check's walk in flight is kept, its answer taken, with no second walk. Window-focus re-reads are not stream-driven and are unchanged.

## Stored choices

Per viewer, in `localStorage`: `specengine.board.as` (`board-screen.md` "The acting name"), `specengine.board.lanes.<p>` and `specengine.board.level.<p>` (`board-lanes.md`), `specengine.workspace.<p>[.<login>]` and `specengine.module.<p>[.<login>]` (`workspaces.md`), `specengine.language`, `specengine.theme`, `specengine.feed.seen.<p>[.<login>]` (Activity's last seen `seq`) and `specengine.tasks.sort.<p>` (`spec-feed.md` "UI"), and the Tree's Show drift. Everything else is per-project screen state in the Shell.

## The door's link

Every `href` in `ui/src` comes from `routes.ts` but one: a step-up refusal's link to the hub's login door (`hub-confirm.md` "Screen"). `door.ts` `doorLink` takes the first double-quoted `href` of an `<a` tag in the door's 401 body by a regular expression, never the DOM; keeps it only as an absolute `https:` URL with no credentials; and builds its origin, `/?rd=` and the page's `location.href` encoded. It opens in a new tab, `rel="noopener noreferrer"`. The body itself is never shown, announced or parsed as markup.

## Tests

UI `src/api/live.test.tsx` (TF10-TF12), `feed.live.test.tsx` (FD6), `feed/feed.test.ts`, `catalog.live.test.tsx`, `delivery.live.test.tsx`, `roadmap.live.test.tsx`, `round/QuestionsView.test.tsx`, `queries.test.tsx`, `api/door.test.ts`, `board/level.test.ts`, `policy.test.ts`.
