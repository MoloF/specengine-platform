import { act, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aProposal, entryOf } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, openStreams, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import { useBundle, useInbox, useLiveQueue, useNode, useProposal, useSearch, useTree } from "./queries";
import type { Proposal } from "./types";

// AC-09 of docs/features/daemon-read.md: the live tail over a stubbed EventSource. A `proposal.*`
// event reads that project's inbox and that proposal again; `proposal.applied` also that project's
// trees, nodes, searches and bundles (R-n10); nothing of another project. A stream the browser
// resumes reads nothing again; one this client opened again after the browser gave up reads the
// project's inbox, proposals and spec reads (R-n6). The shell follows the shown project.

const QUEUE: Record<string, Proposal[]> = {
  alpha: [aProposal({ id: "PR-0001", summary: "First" }), aProposal({ id: "PR-0002", summary: "Second" })],
  beta: [aProposal({ id: "PR-0001", project: "beta", summary: "Beta's" })],
};

/** The stubbed daemon: projects, both inboxes, each review document, trees, nodes, searches, bundles; the rest its 404. */
function daemon(queue: Record<string, Proposal[]> = QUEUE) {
  return stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [
        { slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" },
        { slug: "beta", name: null, root: "/work/beta", branch: null },
      ]);
    }
    const match = /^\/api\/projects\/(\w+)\/(inbox|proposals\/([\w-]+)|tree|nodes\/([\w-]+)|search|bundle)(\?.*)?$/.exec(url);
    const proposals = queue[match?.[1] ?? ""];
    if (match === null || proposals === undefined) {
      return errorAnswer(404, `no route ${url}`);
    }
    if (match[2] === "inbox") {
      return jsonAnswer(200, { proposals: proposals.map(entryOf), notes: [] });
    }
    if (match[2] === "tree") {
      return jsonAnswer(200, { ref: null, reason: null, notes: [], depth: null, kinds: [], archive: false, left_out: { generated: 0, tier3: 0 }, truncated: false, nodes: [] });
    }
    if (match[4] !== undefined) {
      return jsonAnswer(200, { ref: match[4], reason: null, notes: [], nodes: [] });
    }
    if (match[2] === "search") {
      return jsonAnswer(200, { query: "tide", kinds: [], limit: null, archive: false, reason: null, notes: [], hits: [] });
    }
    if (match[2] === "bundle") {
      return jsonAnswer(200, { refs: ["MEC-TIDES"], reason: null, notes: [], body: "" });
    }
    const found = proposals.find((proposal) => proposal.id === match[3]);
    return found === undefined ? errorAnswer(404, "no such proposal") : jsonAnswer(200, found);
  });
}

function stream(url: string): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.url === url && candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error(`no open stream ${url}`);
  }
  return source;
}

/** Alpha's reads of the queue and of the spec (two trees), and beta's: alpha's live tail on. */
function Reads() {
  const inbox = useInbox("alpha");
  const beta = useInbox("beta");
  const betaFirst = useProposal("beta", "PR-0001");
  const first = useProposal("alpha", "PR-0001");
  const second = useProposal("alpha", "PR-0002");
  const tree = useTree("alpha", {});
  const deepTree = useTree("alpha", { depth: 2 });
  const betaTree = useTree("beta", {});
  const node = useNode("alpha", "MEC-TIDES");
  const betaNode = useNode("beta", "MEC-TIDES");
  const search = useSearch("alpha", { query: "tide" });
  const bundle = useBundle("alpha", { node_ids: ["MEC-TIDES"] }, true);
  useLiveQueue("alpha");
  const reads = [inbox, beta, betaFirst, first, second, tree, deepTree, betaTree, node, betaNode, search, bundle];
  const ready = reads.every((query) => query.data !== undefined && !query.isFetching);
  return <p>{ready ? "ready" : "reading"}</p>;
}

/** Alpha's reads of the spec, as requested. */
const ALPHA_SPEC_READS = [
  "/api/projects/alpha/bundle?node_ids=MEC-TIDES",
  "/api/projects/alpha/nodes/MEC-TIDES",
  "/api/projects/alpha/search?query=tide",
  "/api/projects/alpha/tree",
  "/api/projects/alpha/tree?depth=2",
];

/** Renders Reads over the stubbed daemon and opens alpha's stream; the fetch stub, cleared. */
async function live() {
  const fetchStub = daemon();
  render(
    <ApiProvider client={new HttpClient()} queryClient={createQueryClient()}>
      <Reads />
    </ApiProvider>,
  );
  await settled();
  expect(openStreams()).toEqual(["/api/projects/alpha/events"]);
  const source = stream("/api/projects/alpha/events");
  source.open();
  fetchStub.mockClear();
  return { fetchStub, source };
}

async function settled() {
  await waitFor(() => {
    expect(screen.getByText(/^(ready|reading)$/).textContent).toBe("ready");
  });
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("a queue event (AC-09)", () => {
  it("proposal.created reads that project's inbox and that proposal again, nothing else", async () => {
    const { fetchStub, source } = await live();

    act(() => {
      source.emit("proposal.created", '{"id":"PR-0002"}', "41");
    });
    await settled();
    expect(urlsOf(fetchStub).sort()).toEqual(["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0002"]);

    // A proposal no read holds: only the inbox.
    fetchStub.mockClear();
    act(() => {
      source.emit("proposal.created", '{"id":"PR-0003"}', "42");
    });
    await settled();
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/inbox"]);
  });

  it.each(["proposal.approved", "proposal.rejected", "proposal.apply_failed"])(
    "%s reads that project's inbox and that proposal again, no spec read",
    async (type) => {
      const { fetchStub, source } = await live();
      act(() => {
        source.emit(type, '{"id":"PR-0001"}', "50");
      });
      await settled();
      expect(urlsOf(fetchStub).sort()).toEqual(["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0001"]);
    },
  );

  it("proposal.applied also reads that project's trees, nodes, searches and bundles again, nothing of another project (R-n10)", async () => {
    const { fetchStub, source } = await live();
    act(() => {
      source.emit("proposal.applied", '{"id":"PR-0002","commit":"abc1234"}', "51");
    });
    await settled();
    expect(urlsOf(fetchStub).sort()).toEqual(
      ["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0002", ...ALPHA_SPEC_READS].sort(),
    );
  });

  it("a stream the browser resumes (Last-Event-ID) reads nothing again (R-n6)", async () => {
    const { fetchStub, source } = await live();
    source.fail(false);
    act(() => {
      source.open();
    });
    await act(async () => {
      await Promise.resolve();
    });
    await settled();
    expect(urlsOf(fetchStub)).toEqual([]);
  });

  it("a stream this client opened again after the browser gave up reads the project's inbox, proposals and spec reads, nothing else", async () => {
    const { fetchStub, source } = await live();
    // Only the client's own wait is faked: nothing of React Query runs in this window.
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    source.fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    vi.useRealTimers();
    expect(fetchStub).not.toHaveBeenCalled();
    act(() => {
      stream("/api/projects/alpha/events").open();
    });
    await settled();
    expect(urlsOf(fetchStub).sort()).toEqual(
      ["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0001", "/api/projects/alpha/proposals/PR-0002", ...ALPHA_SPEC_READS].sort(),
    );
  });
});

describe("the shell's live tail (AC-09, AC-11)", () => {
  it("shows a new question in the Inbox without a reload, and follows the shown project", async () => {
    const queue = { alpha: [...(QUEUE.alpha ?? [])], beta: [...(QUEUE.beta ?? [])] };
    daemon(queue);
    window.history.replaceState(null, "", "/#/alpha/inbox");
    render(<App client={new HttpClient()} scenario={null} />);
    const list = await screen.findByRole("listbox");
    expect(within(list).getAllByRole("option")).toHaveLength(2);
    expect(screen.queryByText("Mock data")).toBeNull();
    await waitFor(() => {
      expect(openStreams()).toEqual(["/api/projects/alpha/events"]);
    });
    const source = stream("/api/projects/alpha/events");
    source.open();

    // An agent asks a question elsewhere; the daemon logs it and the stream says so.
    queue.alpha.push(aProposal({ id: "PR-0003", kind: "question", severity: "high", summary: "Does regeneration wait for rest?" }));
    act(() => {
      source.emit("proposal.created", '{"id":"PR-0003"}', "7");
    });
    expect(await within(list).findByText("Does regeneration wait for rest?")).toBeTruthy();

    // Another project shown: its stream alone.
    act(() => {
      window.location.hash = "#/beta/inbox";
    });
    await waitFor(() => {
      expect(openStreams()).toEqual(["/api/projects/beta/events"]);
    });
  });

  it("opens no stream for a project the daemon does not serve", async () => {
    daemon();
    window.history.replaceState(null, "", "/#/zeta/inbox");
    render(<App client={new HttpClient()} scenario={null} />);
    await screen.findByRole("option", { name: "Alpha" });
    await act(async () => {
      await Promise.resolve();
    });
    expect(FakeEventSource.instances).toEqual([]);
  });
});
