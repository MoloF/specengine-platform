import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aMap, aMapNode } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";

// docs/features/corpus-map.md "Acceptance criteria", CM8, over HttpClient, the daemon a stubbed
// `fetch` and `EventSource`: the Graph's home reads `/map` once on screen, with its query's signal;
// the map is a read of the spec, so an apply and a gap each read it once more; a proposal raised, a
// stage, a card and a task event read nothing of it.

const STREAM = "/api/projects/alpha/events";
const MAP = "/api/projects/alpha/map";

function daemon() {
  const map = aMap([aMapNode({ name: "SCN-ARRIVAL", title: "Arrival" })]);
  stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/identity") {
      return jsonAnswer(200, { login: null });
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url === "/api/projects/alpha/tasks") {
      return jsonAnswer(200, { tasks: [], notes: [] });
    }
    if (url === MAP) {
      return jsonAnswer(200, map);
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

function stream(): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.url === STREAM && candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error("no open stream");
  }
  return source;
}

function mapReads(): string[] {
  return urlsOf(vi.mocked(fetch)).filter((url) => url.startsWith(MAP));
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** alpha's Graph home over HttpClient, its live tail open; the fetch stub's calls cleared after entering. */
async function onMap() {
  daemon();
  window.history.replaceState(null, "", "/#/alpha/graph");
  render(<App client={new HttpClient()} scenario={null} />);
  await screen.findByText("1 document, 0 links, 0 orphans");
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual([STREAM]);
  });
  act(() => {
    stream().open();
  });
  await settle();
  const entered = [...vi.mocked(fetch).mock.calls];
  vi.mocked(fetch).mockClear();
  return entered;
}

function emit(type: string, payload: string, id: string) {
  act(() => {
    stream().emit(type, payload, id);
  });
}

beforeAll(async () => {
  await import("../map/MapView");
});

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("the map's read on the live tail (CM8)", () => {
  it("the home on screen reads `/map` once, with its query's signal", async () => {
    const entered = await onMap();
    const reads = entered.filter(([input]) => typeof input === "string" && input.startsWith(MAP));
    expect(reads.map(([input]) => input)).toEqual([MAP]);
    expect(reads[0]?.[1]?.signal).toBeInstanceOf(AbortSignal);
  });

  it("an apply reads the map once more (M: the map left out of the reads of the spec)", async () => {
    await onMap();
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "40");
    await waitFor(() => {
      expect(mapReads()).toEqual([MAP]);
    });
    await settle();
    expect(mapReads()).toEqual([MAP]);
  });

  it("a gap (a stream this client opened again) reads the map once more", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await onMap();
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
      expect(mapReads()).toEqual([MAP]);
    });
  });

  it.each([
    ["proposal.created", '{"id":"PR-0001"}'],
    ["proposal.staged", '{"id":"PR-0001"}'],
    ["card.moved", '{"id":"C-0001","from":"todo","to":"doing"}'],
    ["task.claimed", '{"id":"T-0109","run":1}'],
  ])("%s reads nothing of the map", async (type, payload) => {
    await onMap();
    emit(type, payload, "42");
    await settle();
    await settle();
    expect(mapReads()).toEqual([]);
  });
});
