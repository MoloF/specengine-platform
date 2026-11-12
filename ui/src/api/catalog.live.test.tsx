import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aCatalog, aCatalogRow, aDirection, aTreeNode, aTreeView, aWorkspace, aWorkspaces } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";

// DC-10 of docs/features/direction-catalog.md "Acceptance criteria" over HttpClient, the daemon a
// stubbed `fetch` and `EventSource`: a direction's table reads `?kinds=` once on screen, with its
// query's signal; while it is on screen a `card.*` event (naming a card or none), any `proposal.*`
// event but a stage, an apply and a gap each read it once more; a burst during a read aborts the
// read in flight and one read is answered; a stage and a task event read nothing of it.

const STREAM = "/api/projects/alpha/events";
const CATALOG = "/api/projects/alpha/catalog";
const ASKED = `${CATALOG}?kinds=scene`;

/** What became of each catalog read: aborted through its signal, or answered. */
interface Fates {
  aborted: number;
  answered: number;
  /** Holds every catalog read from now on until `release()`. */
  hold: () => void;
  release: () => void;
}

/** The daemon: one project with the direction Scenes, an empty queue, a one-row tree and catalog. */
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
  const catalog = aCatalog([aCatalogRow({ id: "SCN-ARRIVAL", title: "Arrival", state: "in work" })]);
  stubFetch((url, init) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/workspaces") {
      return jsonAnswer(200, aWorkspaces([aWorkspace({ name: "writing" })], [aDirection({ name: "scenes", title: "Scenes", kinds: ["scene"] })]));
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url === "/api/projects/alpha/tasks") {
      return jsonAnswer(200, { tasks: [], notes: [] });
    }
    if (url.startsWith("/api/projects/alpha/tree?")) {
      return jsonAnswer(200, aTreeView([aTreeNode({ id: "SCN-ARRIVAL", depth: 0, kind: "scene", title: "Arrival" })], { kinds: ["scene"] }));
    }
    if (!url.startsWith(`${CATALOG}?`)) {
      return errorAnswer(404, `no route ${url}`);
    }
    if (!held) {
      fates.answered += 1;
      return jsonAnswer(200, catalog);
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
          resolve(jsonAnswer(200, catalog));
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

function catalogReads(): string[] {
  return urlsOf(fetchStub()).filter((url) => url.startsWith(CATALOG));
}

/** Runs the tasks jsdom queued: a refetch scheduled by an event, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** alpha's Spec tree at the direction Scenes over HttpClient, its live tail open; the fetch stub's calls cleared after entering. */
async function onCatalog() {
  const fates = daemon();
  window.localStorage.setItem("specengine.module.alpha", "scenes");
  window.history.replaceState(null, "", "/#/alpha/tree");
  render(<App client={new HttpClient()} scenario={null} />);
  await screen.findByRole("table", { name: "Scenes: 1 node" });
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

// The table's chunk loaded once before the first test, so its first transform never eats a query's wait.
beforeAll(async () => {
  await import("../tree/DirectionCatalog");
});

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("the catalog's read on the live tail (DC-10)", () => {
  it("the table on screen reads `?kinds=` once, no archive, with its query's signal", async () => {
    const { entered } = await onCatalog();
    const reads = entered.filter(([input]) => typeof input === "string" && input.startsWith(CATALOG));
    expect(reads.map(([input]) => input)).toEqual([ASKED]);
    expect(reads[0]?.[1]?.signal).toBeInstanceOf(AbortSignal);
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
  ])("%s reads the catalog once more (M: an apply read twice)", async (_name, type, payload) => {
    await onCatalog();
    emit(type, payload, "40");
    await waitFor(() => {
      expect(catalogReads()).toEqual([ASKED]);
    });
    await settle();
    expect(catalogReads()).toEqual([ASKED]);
  });

  it("a gap (a stream this client opened again) reads the catalog once more", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await onCatalog();
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
      expect(catalogReads()).toEqual([ASKED]);
    });
    await settle();
    expect(catalogReads()).toEqual([ASKED]);
  });

  it.each([
    ["proposal.staged", '{"id":"PR-0001"}'],
    ["proposal.unstaged", '{"id":"PR-0001"}'],
    ["task.claimed", '{"id":"T-0109","run":1}'],
    ["task.created", '{"id":"T-0110"}'],
  ])("%s reads nothing of the catalog", async (type, payload) => {
    await onCatalog();
    emit(type, payload, "42");
    await settle();
    await settle();
    expect(catalogReads()).toEqual([]);
  });

  it("a burst during a read aborts the read in flight: one catalog read answered", async () => {
    const { fates } = await onCatalog();
    const before = fates.answered;
    fates.hold();
    emit("card.moved", '{"id":"C-0001"}', "50");
    await waitFor(() => {
      expect(catalogReads()).toHaveLength(1);
    });
    emit("card.edited", '{"id":"C-0001"}', "51");
    emit("proposal.created", '{"id":"PR-0002"}', "52");
    await waitFor(() => {
      expect(fates.aborted).toBe(2);
    });
    fates.release();
    await settle();
    expect(catalogReads()).toHaveLength(3);
    expect(fates.answered - before).toBe(1);
  });
});
