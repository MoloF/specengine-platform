import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { localToday } from "../board/due";
import { aDelivery, aDeliveryCounts, aRoadmap, aRoadmapGroup, aRoadmapItem } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";

// DA-12 of docs/features/delivery-analytics.md "Acceptance criteria" over HttpClient, the daemon a
// stubbed `fetch` and `EventSource`: entering the Delivery tab reads `?as_of=` the viewer's day once,
// with its query's signal; while it is on screen a `card.*` event (naming a card or none), any
// `proposal.*` event but a stage, and a gap each read it once more, at the day on screen; a burst
// during a read aborts the read in flight and one read is answered; a stage and a task event read
// nothing of it.

const STREAM = "/api/projects/alpha/events";
const DELIVERY = "/api/projects/alpha/delivery";

/** What became of each delivery read: aborted through its signal, or answered. */
interface Fates {
  aborted: number;
  answered: number;
  /** Holds every delivery read from now on until `release()`. */
  hold: () => void;
  release: () => void;
}

/** The daemon: one project, an empty queue and task list, a one-item plan, a delivery of one task at the day asked. */
function daemon(): Fates {
  let held = false;
  const waiting: (() => void)[] = [];
  const fates: Fates = {
    aborted: 0,
    answered: 0,
    hold: () => {
      held = true;
    },
    release: () => {
      held = false;
      for (const answer of waiting.splice(0)) {
        answer();
      }
    },
  };
  const delivery = (url: string) => aDelivery(new URL(url, "http://local").searchParams.get("as_of") ?? "", { total: aDeliveryCounts({ tasks: 1, open: 1 }) });
  stubFetch((url, init) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url === "/api/projects/alpha/tasks") {
      return jsonAnswer(200, { tasks: [], notes: [] });
    }
    if (url === "/api/projects/alpha/roadmap") {
      return jsonAnswer(200, aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001", title: "First item" })], { current: true })]));
    }
    if (!url.startsWith(`${DELIVERY}?`)) {
      return errorAnswer(404, `no route ${url}`);
    }
    if (!held) {
      fates.answered += 1;
      return jsonAnswer(200, delivery(url));
    }
    return new Promise<Response>((resolve, reject) => {
      const signal = init?.signal ?? undefined;
      signal?.addEventListener("abort", () => {
        fates.aborted += 1;
        reject(new DOMException("The operation was aborted.", "AbortError"));
      });
      waiting.push(() => {
        if (signal?.aborted !== true) {
          fates.answered += 1;
          resolve(jsonAnswer(200, delivery(url)));
        }
      });
    });
  });
  return fates;
}

function stream(): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.url === STREAM && candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error("no open stream");
  }
  return source;
}

function fetchStub() {
  return vi.mocked(fetch);
}

function deliveryReads(): string[] {
  return urlsOf(fetchStub()).filter((url) => url.startsWith(DELIVERY));
}

/** Runs the tasks jsdom queued: a refetch scheduled by an event, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The Delivery tab of alpha's Roadmap over HttpClient, its live tail open; the fetch stub's calls cleared after entering. */
async function onDelivery() {
  const fates = daemon();
  window.history.replaceState(null, "", "/#/alpha/roadmap");
  render(<App client={new HttpClient()} scenario={null} />);
  fireEvent.click(await screen.findByRole("tab", { name: "Delivery" }));
  await screen.findByRole("table", { name: "By release" });
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual([STREAM]);
  });
  act(() => {
    stream().open();
  });
  await settle();
  const entered = [...fetchStub().mock.calls];
  fetchStub().mockClear();
  return { fates, entered };
}

function emit(type: string, payload: string, id: string) {
  act(() => {
    stream().emit(type, payload, id);
  });
}

/** The day on screen, as sent. */
function asked(): string {
  return `${DELIVERY}?as_of=${localToday(Date.now())}`;
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("the delivery's read on the live tail (DA-12)", () => {
  it("entering the Delivery tab reads `?as_of=` the viewer's day, with its query's signal", async () => {
    const { entered } = await onDelivery();
    const reads = entered.filter(([input]) => typeof input === "string" && input.startsWith(DELIVERY));
    expect(reads.at(-1)?.[0]).toBe(asked());
    expect(reads.at(-1)?.[1]?.signal).toBeInstanceOf(AbortSignal);
  });

  it.each([
    ["card.moved, naming a card", "card.moved", '{"id":"C-0001","from":"todo","to":"doing"}'],
    ["card.accepted, naming a card", "card.accepted", '{"id":"C-0001"}'],
    ["a card event naming none", "card.refreshed", "{}"],
    ["proposal.created", "proposal.created", '{"id":"PR-0001"}'],
    ["proposal.approved", "proposal.approved", '{"id":"PR-0001"}'],
    ["proposal.applied", "proposal.applied", '{"id":"PR-0001","commit":"abc1234"}'],
    ["proposal.rejected", "proposal.rejected", '{"id":"PR-0001"}'],
    ["proposal.apply_failed", "proposal.apply_failed", '{"id":"PR-0001"}'],
  ])("%s reads the delivery once more, at the day on screen", async (_name, type, payload) => {
    await onDelivery();
    emit(type, payload, "40");
    await waitFor(() => {
      expect(deliveryReads()).toEqual([asked()]);
    });
    await settle();
    expect(deliveryReads()).toEqual([asked()]);
  });

  it("an event reads the day the field holds, not the viewer's", async () => {
    await onDelivery();
    fireEvent.change(screen.getByLabelText("As of"), { target: { value: "2026-11-30" } });
    await waitFor(() => {
      expect(deliveryReads()).toEqual([`${DELIVERY}?as_of=2026-11-30`]);
    });
    await screen.findByRole("table", { name: "By release" });
    fetchStub().mockClear();
    emit("card.moved", '{"id":"C-0001"}', "41");
    await waitFor(() => {
      expect(deliveryReads()).toEqual([`${DELIVERY}?as_of=2026-11-30`]);
    });
  });

  it("a gap (a stream this client opened again) reads the delivery once more", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await onDelivery();
    act(() => {
      stream().fail(true);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(REOPEN_FIRST_MS);
    });
    act(() => {
      stream().open();
    });
    await waitFor(() => {
      expect(deliveryReads()).toEqual([asked()]);
    });
    await settle();
    expect(deliveryReads()).toEqual([asked()]);
  });

  it.each([
    ["proposal.staged", '{"id":"PR-0001"}'],
    ["proposal.unstaged", '{"id":"PR-0001"}'],
    ["task.claimed", '{"id":"T-0109","run":1}'],
    ["task.created", '{"id":"T-0110"}'],
  ])("%s reads nothing of the delivery", async (type, payload) => {
    await onDelivery();
    emit(type, payload, "42");
    await settle();
    await settle();
    expect(deliveryReads()).toEqual([]);
  });

  it("a burst during a read aborts the read in flight: one delivery read answered", async () => {
    const { fates } = await onDelivery();
    const before = fates.answered;
    fates.hold();
    emit("card.moved", '{"id":"C-0001"}', "50");
    await waitFor(() => {
      expect(deliveryReads()).toHaveLength(1);
    });
    emit("card.edited", '{"id":"C-0001"}', "51");
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "52");
    await waitFor(() => {
      expect(fates.aborted).toBe(2);
    });
    fates.release();
    await settle();
    expect(deliveryReads()).toHaveLength(3);
    expect(fates.answered - before).toBe(1);
  });
});
