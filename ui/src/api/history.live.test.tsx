import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { aHistory, aHistoryEntry, aNode } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import { queryKeys, useLiveQueue, useNodeHistory } from "./queries";

// NH7 and NH12 of docs/features/node-history.md "Acceptance criteria" over HttpClient, the daemon a
// stubbed `fetch` and `EventSource`: the history read is `nodes/<REF>?with=history`, made only once
// asked; its key is the node's, a read of the spec, so `proposal.applied` reads it again and
// `proposal.created` does not.

const HISTORY = "/api/projects/alpha/nodes/MEC-TIDES?with=history";
const STREAM = "/api/projects/alpha/events";

function daemon() {
  return stubFetch((url) => {
    if (url === HISTORY) {
      return jsonAnswer(200, { ref: "MEC-TIDES", reason: null, notes: [], nodes: [aNode({ id: "MEC-TIDES", history: aHistory([aHistoryEntry({ id: "a".repeat(40) })]) })] });
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

function stream(): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.url === STREAM && candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error(`no open stream ${STREAM}`);
  }
  return source;
}

function Reads({ asked }: { asked: boolean }) {
  const read = useNodeHistory("alpha", "MEC-TIDES", asked);
  useLiveQueue("alpha");
  return <p>{read.data === undefined ? "none" : read.isFetching ? "reading" : `entries ${String(read.data.nodes[0]?.history?.entries.length ?? 0)}`}</p>;
}

async function quiet() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the history read (NH7, NH12 of node-history)", () => {
  it("keys the read as the node's, `with: [history]`", () => {
    expect(queryKeys.node("alpha", "MEC-TIDES", { with: ["history"] })).toEqual(["node", "alpha", "MEC-TIDES", { with: ["history"], archive: false }]);
  });

  it("reads nothing until asked", async () => {
    const fetchStub = daemon();
    render(
      <ApiProvider client={new HttpClient()} queryClient={createQueryClient()}>
        <Reads asked={false} />
      </ApiProvider>,
    );
    await quiet();
    expect(urlsOf(fetchStub)).toEqual([]);
    expect(screen.getByText("none")).toBeTruthy();
  });

  it("asked, reads `with=history` once; proposal.created reads nothing of it, proposal.applied reads it again, once", async () => {
    const fetchStub = daemon();
    render(
      <ApiProvider client={new HttpClient()} queryClient={createQueryClient()}>
        <Reads asked />
      </ApiProvider>,
    );
    await screen.findByText("entries 1");
    expect(urlsOf(fetchStub)).toEqual([HISTORY]);
    const source = stream();
    act(() => {
      source.open();
    });
    fetchStub.mockClear();
    act(() => {
      source.emit("proposal.created", '{"id":"PR-0002"}', "10");
    });
    await quiet();
    expect(urlsOf(fetchStub)).toEqual([]);
    act(() => {
      source.emit("proposal.applied", '{"id":"PR-0002","commit":"abc1234"}', "11");
    });
    await waitFor(() => {
      expect(urlsOf(fetchStub)).toEqual([HISTORY]);
    });
    await quiet();
    expect(urlsOf(fetchStub)).toEqual([HISTORY]);
  });
});
