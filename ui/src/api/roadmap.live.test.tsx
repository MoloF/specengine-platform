import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aRoadmap, aRoadmapGroup, aRoadmapItem } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";

// UI-5 of docs/features/roadmap-screen.md over HttpClient, the daemon a stubbed `fetch` and
// `EventSource`: entering the Roadmap reads the plan once, with its query's signal; while it is on
// screen a `task.*` event (naming a task or none), `proposal.applied` and a gap each read it once
// more; a burst during a read aborts the read in flight and one read is answered; a stage,
// `proposal.created`, `.approved`, `.rejected` or `.apply_failed` reads nothing. Off screen nothing
// is read; entering reads once.

const ROADMAP = "/api/projects/alpha/roadmap";
const STREAM = "/api/projects/alpha/events";

function plan() {
  return aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001", title: "First item" })], { current: true })]);
}

/** What became of each plan read: aborted through its signal, or answered. */
interface Fates {
  aborted: number;
  answered: number;
  /** Holds every plan read from now on until `release()`. */
  hold: () => void;
  release: () => void;
}

/** The daemon: one project, an empty queue and task list, its plan; plan reads held while `hold()` is on. */
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
    if (url !== ROADMAP) {
      return errorAnswer(404, `no route ${url}`);
    }
    if (!held) {
      fates.answered += 1;
      return jsonAnswer(200, plan());
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
          resolve(jsonAnswer(200, plan()));
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

function planReads(): string[] {
  return urlsOf(fetchStub()).filter((url) => url === ROADMAP);
}

/** Runs the tasks jsdom queued: a refetch scheduled by an event, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The Roadmap of alpha over HttpClient, its live tail open; the fetch stub's calls cleared after entering. */
async function onRoadmap(hash = "#/alpha/roadmap") {
  const fates = daemon();
  window.history.replaceState(null, "", `/${hash}`);
  render(<App client={new HttpClient()} scenario={null} />);
  await screen.findByRole("heading", { level: 1, name: "Roadmap" });
  await screen.findByRole("tab", { name: "Lanes" });
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual([STREAM]);
  });
  act(() => {
    stream().open();
  });
  const entered = [...fetchStub().mock.calls];
  fetchStub().mockClear();
  return { fates, entered };
}

function emit(type: string, payload: string, id: string) {
  act(() => {
    stream().emit(type, payload, id);
  });
}

/** A section's link in the nav, followed as the browser does. */
async function go(name: string) {
  const link = within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name });
  fireEvent.click(link);
  await act(async () => {
    window.location.hash = link.getAttribute("href") ?? "";
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the plan's read on the live tail (UI-5)", () => {
  it("entering the Roadmap reads the plan once, with its query's signal, no query sent", async () => {
    const { entered } = await onRoadmap();
    const reads = entered.filter(([input]) => input === ROADMAP);
    expect(reads).toHaveLength(1);
    expect(reads[0]?.[1]?.signal).toBeInstanceOf(AbortSignal);
  });

  it.each([
    ["task.claimed, naming a task", "task.claimed", '{"id":"T-0109","run":1}'],
    ["task.created, naming a task", "task.created", '{"id":"T-0110"}'],
    ["a task event naming none", "task.refreshed", "{}"],
    ["proposal.applied", "proposal.applied", '{"id":"PR-0001","commit":"abc1234"}'],
  ])("%s reads the plan once more", async (_name, type, payload) => {
    await onRoadmap();
    emit(type, payload, "40");
    await waitFor(() => {
      expect(planReads()).toEqual([ROADMAP]);
    });
    await settle();
    expect(planReads()).toEqual([ROADMAP]);
  });

  it("a gap (a stream this client opened again) reads the plan once more", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await onRoadmap();
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
      expect(planReads()).toEqual([ROADMAP]);
    });
    await settle();
    expect(planReads()).toEqual([ROADMAP]);
  });

  it.each(["proposal.created", "proposal.approved", "proposal.rejected", "proposal.apply_failed", "proposal.staged", "proposal.unstaged"])(
    "%s reads nothing of the plan",
    async (type) => {
      await onRoadmap();
      emit(type, '{"id":"PR-0001"}', "41");
      await settle();
      await settle();
      expect(planReads()).toEqual([]);
    },
  );

  it("a burst during a read aborts the read in flight: one plan read answered", async () => {
    const { fates } = await onRoadmap();
    const before = fates.answered;
    fates.hold();
    emit("task.claimed", '{"id":"T-0109","run":1}', "50");
    await waitFor(() => {
      expect(planReads()).toHaveLength(1);
    });
    emit("task.run_reported", '{"id":"T-0109","run":1}', "51");
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "52");
    await waitFor(() => {
      expect(fates.aborted).toBe(2);
    });
    fates.release();
    await settle();
    expect(planReads()).toHaveLength(3);
    expect(fates.answered - before).toBe(1);
    expect(document.querySelector("[data-item]")?.getAttribute("data-item")).toBe("RM-0001");
  });

  it("off screen nothing is read; entering the Roadmap again reads once", async () => {
    await onRoadmap();
    await go("Tasks");
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    fetchStub().mockClear();
    emit("task.claimed", '{"id":"T-0109","run":1}', "60");
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "61");
    await settle();
    await settle();
    expect(planReads()).toEqual([]);
    await go("Roadmap");
    await screen.findByRole("tab", { name: "Lanes" });
    await settle();
    expect(planReads()).toEqual([ROADMAP]);
  });
});
