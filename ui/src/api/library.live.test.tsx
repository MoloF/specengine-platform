import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aLibrary, aLibraryRow } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";

// docs/features/doc-library.md "The UI seam" over HttpClient, the daemon a stubbed `fetch` and
// `EventSource`: Documents reads `/library` once on screen, with its query's signal; the library is a
// read of the spec (READS_OF_THE_SPEC), so an apply and a gap each read it once more; a proposal
// raised, a stage, a card and a task event read nothing of it: it serves no queue or card data.

const STREAM = "/api/projects/alpha/events";
const LIBRARY = "/api/projects/alpha/library";

function daemon() {
  const library = aLibrary([aLibraryRow({ name: "DOM-HARBOR", title: "Harbour" })]);
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
    if (url === LIBRARY) {
      return jsonAnswer(200, library);
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

function libraryReads(): string[] {
  return urlsOf(vi.mocked(fetch)).filter((url) => url.startsWith(LIBRARY));
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** alpha's Documents over HttpClient, its live tail open; the fetch stub's calls cleared after entering. */
async function onDocuments() {
  daemon();
  window.history.replaceState(null, "", "/#/alpha/documents");
  render(<App client={new HttpClient()} scenario={null} />);
  await screen.findByRole("table", { name: "1 of 1 document" });
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
  await import("../documents/DocumentsView");
});

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("the library's read on the live tail (doc-library)", () => {
  it("Documents on screen reads `/library` once, with its query's signal", async () => {
    const entered = await onDocuments();
    const reads = entered.filter(([input]) => typeof input === "string" && input.startsWith(LIBRARY));
    expect(reads.map(([input]) => input)).toEqual([LIBRARY]);
    expect(reads[0]?.[1]?.signal).toBeInstanceOf(AbortSignal);
  });

  it("an apply reads the library once more (M: the library left out of the reads of the spec)", async () => {
    await onDocuments();
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "40");
    await waitFor(() => {
      expect(libraryReads()).toEqual([LIBRARY]);
    });
    await settle();
    expect(libraryReads()).toEqual([LIBRARY]);
  });

  it("a gap (a stream this client opened again) reads the library once more", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await onDocuments();
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
      expect(libraryReads()).toEqual([LIBRARY]);
    });
  });

  it.each([
    ["proposal.created", '{"id":"PR-0001"}'],
    ["proposal.staged", '{"id":"PR-0001"}'],
    ["card.moved", '{"id":"C-0001","from":"todo","to":"doing"}'],
    ["task.claimed", '{"id":"T-0109","run":1}'],
  ])("%s reads nothing of the library", async (type, payload) => {
    await onDocuments();
    emit(type, payload, "42");
    await settle();
    await settle();
    expect(libraryReads()).toEqual([]);
  });
});
