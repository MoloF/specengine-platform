import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aFeed, aFeedEntry, aFeedNeedsMe } from "../test/builders";
import { stubClient } from "../test/stubClient";
import type { FeedOptions } from "./client";
import { appendFeed, highestSeq } from "./feedMerge";
import { useFeed } from "./feedQueries";
import { ApiProvider, createQueryClient } from "./provider";
import { useLiveQueue } from "./queries";
import type { FeedDocument, QueueEvent } from "./types";

// FD6 of docs/features/auto-feed.md "Acceptance criteria" over the stub client: the feed on screen is
// read again on any event of the project's stream through `readAgainKept`. An `auto.landed` landing
// while a feed read is held aborts nothing; exactly one read follows it, asked `since` the highest
// seq held, and its entry is appended (docs/canon/ui-client.md "Live re-reads").

interface Held {
  options: FeedOptions;
  signal: AbortSignal | undefined;
  answer: (document: FeedDocument) => void;
}

function FeedOnScreen() {
  const feed = useFeed("alpha", { task: null, department: null, since: null });
  useLiveQueue("alpha");
  return <p>{feed.data === undefined ? "reading" : feed.data.entries.map((entry) => entry.seq).join(" ") || "empty"}</p>;
}

/** The stub client, every feed read held until answered by hand, its tail driven by `emit`. */
function heldFeed() {
  const client = stubClient();
  const reads: Held[] = [];
  client.getFeed.mockImplementation(
    (_project, options, signal) =>
      new Promise<FeedDocument>((resolve) => {
        reads.push({ options, signal, answer: resolve });
      }),
  );
  let tail: (event: QueueEvent) => void = () => undefined;
  let lost: (() => void) | undefined;
  client.subscribe = (_project, onEvent, onGap) => {
    tail = onEvent;
    lost = onGap;
    return () => undefined;
  };
  const emit = (seq: number, type: string, payload: unknown) => {
    act(() => {
      tail({ seq, type, payload });
    });
  };
  const gap = () => {
    act(() => {
      lost?.();
    });
  };
  return { client, reads, emit, gap };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe("the feed's live re-read (FD6 of auto-feed)", () => {
  it("an auto.landed during a held read aborts nothing; one more read follows, asked since the highest seq held, its entry appended (M: cancelRefetch true)", async () => {
    const { client, reads, emit } = heldFeed();
    render(
      <ApiProvider client={client} queryClient={createQueryClient()}>
        <FeedOnScreen />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(reads).toHaveLength(1);
    });
    expect(reads[0]?.options).toEqual({ needs_me: true });
    act(() => {
      reads[0]?.answer(aFeed([aFeedEntry({ seq: 1201 }), aFeedEntry({ seq: 1204 })]));
    });
    expect(await screen.findByText("1201 1204")).toBeTruthy();
    // An event with no read in flight: one read at once, since the highest held.
    emit(1205, "task.claimed", { id: "T-0042", run: 2 });
    await waitFor(() => {
      expect(reads).toHaveLength(2);
    });
    expect(reads[1]?.options).toEqual({ needs_me: true, since: 1204 });
    const held = reads[1];
    // The landing while that read is held: kept, nothing sent beside it.
    emit(1206, "auto.landed", { pack: 812, task: "T-0042", branch: "auto", commit: "9b1e77c2d0aa", code_commit: "1a2b3c", applied: [], left: [], rule: "b3:00" });
    await settle();
    expect([reads.length, held?.signal?.aborted]).toEqual([2, false]);
    act(() => {
      held?.answer(aFeed([aFeedEntry({ seq: 1205 })], { after: 1204 }));
    });
    await waitFor(() => {
      expect(reads).toHaveLength(3);
    });
    expect(reads[2]?.options).toEqual({ needs_me: true, since: 1205 });
    act(() => {
      reads[2]?.answer(aFeed([aFeedEntry({ seq: 1206, type: "auto.landed" })], { after: 1205 }));
    });
    expect(await screen.findByText("1201 1204 1205 1206")).toBeTruthy();
    await settle();
    expect(client.getFeed).toHaveBeenCalledTimes(3);
    expect(reads.map((read) => read.signal?.aborted)).toEqual([false, false, false]);
  });

  it("a gap reads the feed whole, what it held dropped: states refresh, and a queue restored below the held seqs is read; the next event appends again", async () => {
    const { client, reads, gap, emit } = heldFeed();
    render(
      <ApiProvider client={client} queryClient={createQueryClient()}>
        <FeedOnScreen />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(reads).toHaveLength(1);
    });
    act(() => {
      reads[0]?.answer(aFeed([aFeedEntry({ seq: 9 })]));
    });
    expect(await screen.findByText("9")).toBeTruthy();
    gap();
    await waitFor(() => {
      expect(reads).toHaveLength(2);
    });
    // No `since`: the newest entries, as the first read.
    expect(reads[1]?.options).toEqual({ needs_me: true });
    act(() => {
      reads[1]?.answer(aFeed([aFeedEntry({ seq: 3, status: "done" }), aFeedEntry({ seq: 4 })]));
    });
    expect(await screen.findByText("3 4")).toBeTruthy();
    // An ordinary event appends past the new highest.
    emit(5, "task.claimed", { id: "T-0001" });
    await waitFor(() => {
      expect(reads).toHaveLength(3);
    });
    expect(reads[2]?.options).toEqual({ needs_me: true, since: 4 });
    act(() => {
      reads[2]?.answer(aFeed([aFeedEntry({ seq: 5 })], { after: 4 }));
    });
    expect(await screen.findByText("3 4 5")).toBeTruthy();
  });
});

describe("appending a later read (docs/features/auto-feed.md \"The UI\", Live)", () => {
  it("keeps each entry once by seq, oldest first; takes the head, more and the lists now; keeps the window's failed runs and gate writes", () => {
    const held = aFeed([aFeedEntry({ seq: 1 }), aFeedEntry({ seq: 2 })], {
      needs_me: aFeedNeedsMe({
        failed_runs: [{ task: "T-1", run: 1, outcome: "failed", summary: null, ended_at: null }],
        held: [{ seq: 2, at: "2026-11-16T01:00:00Z", pack: 1, task: "T-1", line: null, proposal: null, act: "run", reason: "not-done", detail: null }],
      }),
      notes: ["a note"],
    });
    const later = aFeed([aFeedEntry({ seq: 2 }), aFeedEntry({ seq: 3 })], {
      auto: { state: "on", reason: null, why: null, rule: "b3:01" },
      after: 2,
      more: true,
      needs_me: aFeedNeedsMe({ failed_runs: [{ task: "T-2", run: 1, outcome: "partial", summary: null, ended_at: null }] }),
      notes: ["a note", "another"],
    });
    const merged = appendFeed(held, later);
    expect(merged.entries.map((entry) => entry.seq)).toEqual([1, 2, 3]);
    expect([merged.auto.state, merged.after, merged.last_seq, merged.more]).toEqual(["on", null, 3, true]);
    expect(merged.needs_me?.held).toEqual([]);
    expect(merged.needs_me?.failed_runs.map((run) => run.task)).toEqual(["T-1", "T-2"]);
    expect(merged.notes).toEqual(["a note", "another"]);
    expect(highestSeq(merged)).toBe(3);
    expect(highestSeq(aFeed([], { last_seq: 40 }))).toBe(40);
    expect(highestSeq(aFeed([]))).toBeNull();
  });
});
