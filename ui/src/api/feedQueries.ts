import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { FeedOptions } from "./client";
import { appendFeed, highestSeq } from "./feedMerge";
import { useClient } from "./provider";
import { feedsOf, feedsOwedWhole } from "./queries";
import type { FeedDocument } from "./types";

// The Activity view's one read, imported by src/feed/ alone so it loads with its chunk, as
// ./splitQueries.ts does for the Split dialog (docs/features/auto-feed.md "The UI"): `spec feed --needs-me --json`
// with the view's filters, so both tabs and the Needs me count come from one answer. Keyed under the
// project's feeds, which useLiveQueue reads again on every event and on a gap through
// `readAgainKept`: a read in flight is never aborted, one more follows it. Each read after the first
// asks `since` the highest `seq` held and is appended to it.

/** The newest entries a read without `since` answers (`spec feed`'s `FEED_DEFAULT`). */
export const FEED_DEFAULT = 100;

/** What the view asks beside `needs_me`: the agent task, the department, and `since` for New since your last visit. */
export interface FeedFilters {
  task: string | null;
  department: string | null;
  /** The last seen `seq` when only what is new is asked; null for the newest entries. */
  since: number | null;
}

/** One feed read of a project per filters, every one present (`queries.ts` keys alike). */
export function feedKey(project: string, filters: FeedFilters) {
  return [...feedsOf(project), { task: filters.task, department: filters.department, since: filters.since }] as const;
}

/** The request: `needs_me` always, each filter only when set, `since` the cursor when one is held. */
function optionsOf(filters: FeedFilters, since: number | null): FeedOptions {
  const options: FeedOptions = { needs_me: true };
  if (since !== null) {
    options.since = since;
  }
  if (filters.task !== null) {
    options.task = filters.task;
  }
  if (filters.department !== null) {
    options.department = filters.department;
  }
  return options;
}

/**
 * The feed with the view's filters: read on entering the view and on a filter change (its own
 * entry), then appended by each later read, the query's AbortSignal taken. A read a gap owes
 * (useLiveQueue) drops what is held and reads whole, as the first: states refresh, and a queue
 * restored below the held seqs is read. The daemon's answer is kept as sent; a refusal rejects in
 * its words.
 */
export function useFeed(project: string, filters: FeedFilters) {
  const client = useClient();
  const queryClient = useQueryClient();
  const key = feedKey(project, filters);
  return useQuery({
    queryKey: key,
    queryFn: async ({ signal }) => {
      const hash = queryClient.getQueryCache().find({ queryKey: key, exact: true })?.queryHash;
      const whole = hash !== undefined && feedsOwedWhole.get(queryClient)?.delete(hash) === true;
      const held = whole ? undefined : queryClient.getQueryData<FeedDocument>(key);
      const since = held === undefined ? filters.since : (highestSeq(held) ?? filters.since);
      const answer = await client.getFeed(project, optionsOf(filters, since), signal);
      return held === undefined ? answer : appendFeed(held, answer);
    },
  });
}

export type FeedQuery = ReturnType<typeof useFeed>;
