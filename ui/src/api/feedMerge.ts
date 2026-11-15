import type { FeedDocument, FeedEntry, FeedNeedsMe } from "./types";

// The feed as the page holds it (docs/features/auto-feed.md "The UI", Live): the first read, then
// each later read asked with `since` the highest `seq` held and appended. Nothing is computed here
// but the union of what the daemon sent: an entry once per `seq`, oldest first as served.

/** The highest `seq` the held feed covers: its `last_seq` or its newest entry's, whichever is higher; null with neither. */
export function highestSeq(held: FeedDocument): number | null {
  const newest = held.entries.at(-1)?.seq ?? null;
  if (held.last_seq === null) {
    return newest;
  }
  return newest === null ? held.last_seq : Math.max(held.last_seq, newest);
}

/** The list a later read gives, each item kept once by its key, the held ones first. */
function union<T>(held: readonly T[], later: readonly T[], keyOf: (item: T) => string): T[] {
  const known = new Set(held.map(keyOf));
  return [...held, ...later.filter((item) => !known.has(keyOf(item)))];
}

/**
 * What needs the owner after a later read: `held`, `open` and `unlanded` are the queue's now, so the
 * later read's; `failed_runs` and `gate_observed` are of the read's window, so the held ones and the
 * later window's together.
 */
function needsAfter(held: FeedNeedsMe | null, later: FeedNeedsMe | null): FeedNeedsMe | null {
  if (held === null || later === null) {
    return later ?? held;
  }
  return {
    held: later.held,
    open: later.open,
    failed_runs: union(held.failed_runs, later.failed_runs, (run) => `${run.task}/${String(run.run)}`),
    gate_observed: union(held.gate_observed, later.gate_observed, (write) => String(write.seq)),
    unlanded: later.unlanded,
  };
}

/** The held feed with a later read appended: its entries after the held ones by `seq`, the auto head, `more` and the lists now. */
export function appendFeed(held: FeedDocument, later: FeedDocument): FeedDocument {
  const entries: FeedEntry[] = union(held.entries, later.entries, (entry) => String(entry.seq)).sort((a, b) => a.seq - b.seq);
  const lasts = [held.last_seq, later.last_seq].filter((seq): seq is number => seq !== null);
  return {
    auto: later.auto,
    after: held.after,
    last_seq: lasts.length === 0 ? null : Math.max(...lasts),
    more: later.more,
    entries,
    needs_me: needsAfter(held.needs_me, later.needs_me),
    notes: union(held.notes, later.notes, (note) => note),
  };
}
