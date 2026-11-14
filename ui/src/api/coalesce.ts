import type { InvalidateQueryFilters, QueryClient } from "@tanstack/react-query";

// A live event's re-read (docs/features/hub-turn-fairness.md "The UI half: a live event never aborts a read in flight"): on a hub every read waits
// in the project's one turn, so aborting a read in flight for a newer one sends it to the back of the
// line, and under a steady burst the page never reaches the head. A stream-driven re-read therefore
// never aborts: with no read in flight it reads at once, as before; with one in flight that read is
// kept and exactly one more follows it once it settles, answered or failed, however many events land
// meanwhile. A key change (a new search, another node) and a person's Retry, Read again or Check
// again keep TanStack's own semantics; window-focus re-reads are not stream-driven.

/** Each client's queries owed one more read once their read in flight settles, by query hash. */
const owedBy = new WeakMap<QueryClient, Set<string>>();

function owedOf(queryClient: QueryClient): Set<string> {
  let owed = owedBy.get(queryClient);
  if (owed === undefined) {
    owed = new Set();
    owedBy.set(queryClient, owed);
  }
  return owed;
}

/** Re-reads the matching queries for a live event: a read in flight is kept and followed by at most one more. */
export function readAgainKept(queryClient: QueryClient, filters: InvalidateQueryFilters): void {
  const owed = owedOf(queryClient);
  for (const query of queryClient.getQueryCache().findAll(filters)) {
    const flight = query.promise;
    if (query.state.fetchStatus === "idle" || flight === undefined || owed.has(query.queryHash)) {
      continue;
    }
    // The read in flight may predate the event: one more read is owed once it settles, and the mark
    // is taken off first, so an event during that read owes one more again.
    owed.add(query.queryHash);
    const after = () => {
      owed.delete(query.queryHash);
      void queryClient.invalidateQueries({ queryKey: query.queryKey, exact: true }, { cancelRefetch: false });
    };
    // `after` runs once TanStack's own continuation of this fetch has set the query idle and its data
    // in (query-core 5.104.0 awaits the same promise first), so its invalidation starts a new read
    // rather than joining the settled one; TF10 in live.test.tsx pins it.
    void flight.then(after, after);
  }
  // An idle query matched is read at once (on screen; else marked stale); one in flight keeps its read.
  void queryClient.invalidateQueries(filters, { cancelRefetch: false });
}
