import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aRing, aRings, aRingSector } from "../test/builders";
import { showRings } from "../test/render";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";

// SR7 of docs/features/scope-rings.md "Acceptance criteria" over HttpClient, the daemon a stubbed
// `fetch` and `EventSource`: the Graph's home reads `/rings` once on screen, with its query's signal;
// while it is on screen the rings are read again as a direction's catalog is: a `card.*` event
// (naming a card or none), any `proposal.*` event but a stage, an apply and a gap each read them
// once more; a stage and a task event read nothing of them.

const STREAM = "/api/projects/alpha/events";
const RINGS = "/api/projects/alpha/rings";

function daemon() {
  const rings = aRings([aRing("Inner", [aRingSector({ name: "SCN-ARRIVAL", title: "Arrival", state: "in work" })])]);
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
    if (url === RINGS) {
      return jsonAnswer(200, rings);
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

function fetchStub() {
  return vi.mocked(fetch);
}

function ringsReads(): string[] {
  return urlsOf(fetchStub()).filter((url) => url.startsWith(RINGS));
}

/** Runs the tasks jsdom queued: a refetch scheduled by an event, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** alpha's Graph home over HttpClient, its live tail open; the fetch stub's calls cleared after entering. */
async function onRings() {
  daemon();
  window.history.replaceState(null, "", "/#/alpha/graph");
  render(<App client={new HttpClient()} scenario={null} />);
  await showRings();
  await screen.findByRole("group", { name: "Scope rings" });
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual([STREAM]);
  });
  act(() => {
    stream().open();
  });
  await settle();
  const entered = [...fetchStub().mock.calls];
  fetchStub().mockClear();
  return entered;
}

function emit(type: string, payload: string, id: string) {
  act(() => {
    stream().emit(type, payload, id);
  });
}

beforeAll(async () => {
  await import("../rings/RingsView");
});

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("the rings' read on the live tail (SR7)", () => {
  it("the home on screen reads `/rings` once, with its query's signal", async () => {
    const entered = await onRings();
    const reads = entered.filter(([input]) => typeof input === "string" && input.startsWith(RINGS));
    expect(reads.map(([input]) => input)).toEqual([RINGS]);
    expect(reads[0]?.[1]?.signal).toBeInstanceOf(AbortSignal);
  });

  it.each([
    ["card.moved, naming a card", "card.moved", '{"id":"C-0001","from":"todo","to":"doing"}'],
    ["card.accepted, naming a card", "card.accepted", '{"id":"C-0001"}'],
    ["a card event naming none", "card.refreshed", "{}"],
    ["proposal.created", "proposal.created", '{"id":"PR-0001"}'],
    ["proposal.applied", "proposal.applied", '{"id":"PR-0001","commit":"abc1234"}'],
    ["proposal.rejected", "proposal.rejected", '{"id":"PR-0001"}'],
  ])("%s reads the rings once more (M: an apply read twice)", async (_name, type, payload) => {
    await onRings();
    emit(type, payload, "40");
    await waitFor(() => {
      expect(ringsReads()).toEqual([RINGS]);
    });
    await settle();
    expect(ringsReads()).toEqual([RINGS]);
  });

  it("a gap (a stream this client opened again) reads the rings once more", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await onRings();
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
      expect(ringsReads()).toEqual([RINGS]);
    });
    await settle();
    expect(ringsReads()).toEqual([RINGS]);
  });

  it.each([
    ["proposal.staged", '{"id":"PR-0001"}'],
    ["task.claimed", '{"id":"T-0109","run":1}'],
  ])("%s reads nothing of the rings", async (type, payload) => {
    await onRings();
    emit(type, payload, "42");
    await settle();
    await settle();
    expect(ringsReads()).toEqual([]);
  });
});
