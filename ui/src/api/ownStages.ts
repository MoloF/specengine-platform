import type { QueryClient } from "@tanstack/react-query";
import type { StageChoice } from "./types";

// This tab's own stage writes, so that the live tail tells a stage changed outside this tab from
// one this tab made (`docs/canon/decision-staging.md` "UI": an outside change raises an alert).
// Every stored stage is a `proposal.staged` event, an unstage of a staged choice a
// `proposal.unstaged` one (`docs/canon/decision-staging.md` "Queue"); the event of this tab's
// own write may come before or after its answer. Kept per QueryClient, so each page (and each
// test) has its own.

/** One stage write this tab sent: its choice (null: an unstage), and when it was answered. */
export interface OwnWrite {
  readonly choice: StageChoice | null;
  answeredAt: number | null;
}

/** How long after its answer a write still accounts for an event: the tail polls every 250 ms. */
const ANSWERED_MS = 60_000;

/** The most writes kept per proposal. */
const KEPT = 8;

const WRITES = new WeakMap<QueryClient, Map<string, OwnWrite[]>>();

/** When this tab last heard of a stage change made outside it, per proposal (`Date.now()`). */
const OUTSIDE = new WeakMap<QueryClient, Map<string, number>>();

function keyOf(project: string, id: string): string {
  return JSON.stringify([project, id]);
}

/**
 * Whether a stage change made outside this tab was heard after `since` (a read's `dataUpdatedAt`):
 * then that read no longer says what is staged. An unstage awaits its own event only when the read
 * it was made from showed a stage still current: with nothing staged the daemon answers 200 and
 * logs no event, and an entry left waiting would take a real outside unstage for this tab's own.
 */
export function outsideSince(queryClient: QueryClient, project: string, id: string, since: number): boolean {
  return (OUTSIDE.get(queryClient)?.get(keyOf(project, id)) ?? -Infinity) > since;
}

/** One proposal's writes in a project, oldest first. */
function writesOf(queryClient: QueryClient, project: string, id: string): OwnWrite[] {
  let byProposal = WRITES.get(queryClient);
  if (byProposal === undefined) {
    byProposal = new Map();
    WRITES.set(queryClient, byProposal);
  }
  const key = keyOf(project, id);
  let writes = byProposal.get(key);
  if (writes === undefined) {
    writes = [];
    byProposal.set(key, writes);
  }
  return writes;
}

/** A stage (or, `choice` null, an unstage) this tab is about to send. */
export function startOwnWrite(queryClient: QueryClient, project: string, id: string, choice: StageChoice | null): OwnWrite {
  const writes = writesOf(queryClient, project, id);
  const write: OwnWrite = { choice, answeredAt: null };
  writes.push(write);
  writes.splice(0, Math.max(0, writes.length - KEPT));
  return write;
}

/**
 * The write was answered. `refused`: the daemon stored nothing, so no event of it comes; else
 * (stored, or no answer at all, which may have stored it) its event is still awaited a while.
 */
export function endOwnWrite(queryClient: QueryClient, project: string, id: string, write: OwnWrite, refused: boolean): void {
  const writes = writesOf(queryClient, project, id);
  if (refused) {
    const at = writes.indexOf(write);
    if (at >= 0) {
      writes.splice(at, 1);
    }
    return;
  }
  write.answeredAt = Date.now();
}

/** Whether `staged`, an event's stored stage, holds exactly the choice this tab sent. */
function holdsChoice(staged: unknown, choice: StageChoice): boolean {
  if (typeof staged !== "object" || staged === null || Array.isArray(staged)) {
    return false;
  }
  const stored = new Map<string, unknown>(Object.entries(staged));
  return Object.entries(choice).every(([key, value]) => stored.get(key) === value);
}

/**
 * Whether a stage event is this tab's own write: a `proposal.staged` holding the choice of one of
 * this tab's stages of that proposal, a `proposal.unstaged` matched by one of its unstages. Each
 * write accounts for one event, then is dropped; one answered more than a minute ago no longer
 * counts. An event no write accounts for is noted as an outside change (outsideSince).
 */
export function isOwnStageEvent(queryClient: QueryClient, project: string, id: string, unstaged: boolean, staged: unknown): boolean {
  const writes = writesOf(queryClient, project, id);
  const now = Date.now();
  for (let at = writes.length - 1; at >= 0; at -= 1) {
    const answeredAt = writes[at]?.answeredAt ?? null;
    if (answeredAt !== null && now - answeredAt > ANSWERED_MS) {
      writes.splice(at, 1);
    }
  }
  const own = writes.findIndex(({ choice }) => (unstaged ? choice === null : choice !== null && holdsChoice(staged, choice)));
  if (own < 0) {
    let heard = OUTSIDE.get(queryClient);
    if (heard === undefined) {
      heard = new Map();
      OUTSIDE.set(queryClient, heard);
    }
    heard.set(keyOf(project, id), now);
    return false;
  }
  writes.splice(own, 1);
  return true;
}
