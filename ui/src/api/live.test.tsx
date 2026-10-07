import type { QueryClient } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aCheckFinding, aCheckReport, aGraphNode, aGraphView, aProposal, entryOf } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, openStreams, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import { queryKeys, useBundle, useCheck, useDecideProposal, useGraph, useInbox, useLiveQueue, useNode, useProposal, useSearch, useTree } from "./queries";
import type { Proposal } from "./types";

// AC-09 of docs/features/daemon-read.md: the live tail over a stubbed EventSource. A `proposal.*`
// event reads that project's inbox and that proposal again; `proposal.applied` also that project's
// trees, nodes, searches and bundles (R-n10); nothing of another project. A stream the browser
// resumes reads nothing again; one this client opened again after the browser gave up reads the
// project's inbox, proposals and spec reads (R-n6). The shell follows the shown project. AC-12 of
// docs/features/ui-live.md: the spec reads include the graphs and, while Health shows it, the check.

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

/** A check answer the test holds back: while `next` is set, each check is answered once it settles. */
interface Hold {
  next: Promise<void> | null;
}

/** ui-live's daemon: each project's inbox, PR-0001, a graph and a check (held by `hold`); a decision answered as applied. */
function graphAndCheckDaemon(hold: Hold = { next: null }) {
  return stubFetch((url, init) => {
    const match = /^\/api\/projects\/(alpha|beta)\/(inbox|graph|check|proposals\/PR-0001(\/decision)?)(\?.*)?$/.exec(url);
    if (match === null) {
      return errorAnswer(404, `no route ${url}`);
    }
    if (match[2] === "inbox") {
      return jsonAnswer(200, { proposals: [entryOf(aProposal({ id: "PR-0001" }))], notes: [] });
    }
    if (match[2] === "graph") {
      return jsonAnswer(200, aGraphView([aGraphNode({ id: "MEC-TIDES", distance: 0 })], []));
    }
    if (match[2] === "check") {
      const report = () => jsonAnswer(200, aCheckReport({ verdict: "blocked", counts: { errors: 1 }, findings: [aCheckFinding({ code: "key-missing" })] }));
      return hold.next === null ? report() : hold.next.then(report);
    }
    if (match[3] !== undefined && init?.method === "POST") {
      return jsonAnswer(200, { proposal: aProposal({ id: "PR-0001", status: "applied" }), commit: { sha: "abc1234", subject: "spec: apply PR-0001" } });
    }
    return jsonAnswer(200, aProposal({ id: "PR-0001" }));
  });
}

/** Health's read of a project's check: mounted only while Health is on screen. */
function HealthOnScreen({ project }: { project: string }) {
  useCheck(project);
  return null;
}

/** A project's inbox, PR-0001 and graph on screen, its live tail on; its check while `health`. */
function ProjectOnScreen({ project, health }: { project: string; health: boolean }) {
  useInbox(project);
  useProposal(project, "PR-0001");
  useGraph(project, { ref: "MEC-TIDES" });
  useLiveQueue(project);
  return health ? <HealthOnScreen project={project} /> : null;
}

type Decide = ReturnType<typeof useDecideProposal>["mutate"];

/** Alpha's decision, handed to the test. */
function Decider({ onDecide }: { onDecide: (decide: Decide) => void }) {
  onDecide(useDecideProposal("alpha").mutate);
  return null;
}

/** Alpha and beta on screen, each with its live tail; their checks while `health`. */
function GraphAndCheck({ health, onDecide }: { health: boolean; onDecide: (decide: Decide) => void }) {
  return (
    <>
      <ProjectOnScreen project="alpha" health={health} />
      <ProjectOnScreen project="beta" health={health} />
      <Decider onDecide={onDecide} />
    </>
  );
}

/** Waits until nothing is read; the graphs (and the checks, `withChecks`) answered. */
async function quiet(queryClient: QueryClient, withChecks: boolean) {
  await waitFor(() => {
    expect(queryClient.isFetching()).toBe(0);
    for (const project of ["alpha", "beta"]) {
      expect(queryClient.getQueryData(queryKeys.graph(project, { ref: "MEC-TIDES" }))).toBeDefined();
      if (withChecks) {
        expect(queryClient.getQueryData(queryKeys.check(project))).toBeDefined();
      }
    }
  });
  await act(async () => {
    await Promise.resolve();
  });
  expect(queryClient.isFetching()).toBe(0);
}

/** Renders GraphAndCheck over the stubbed daemon with both streams open; the fetch stub, cleared. */
async function graphAndCheck(health = true) {
  const hold: Hold = { next: null };
  const fetchStub = graphAndCheckDaemon(hold);
  const queryClient = createQueryClient();
  // One client for every render: a new one would open the live tails again.
  const client = new HttpClient();
  let decide: Decide | null = null;
  const onDecide = (next: Decide) => {
    decide = next;
  };
  const view = render(
    <ApiProvider client={client} queryClient={queryClient}>
      <GraphAndCheck health={health} onDecide={onDecide} />
    </ApiProvider>,
  );
  await quiet(queryClient, health);
  expect(openStreams().sort()).toEqual(["/api/projects/alpha/events", "/api/projects/beta/events"]);
  for (const url of openStreams()) {
    stream(url).open();
  }
  fetchStub.mockClear();
  return {
    fetchStub,
    queryClient,
    hold,
    alpha: stream("/api/projects/alpha/events"),
    beta: stream("/api/projects/beta/events"),
    decide: (...args: Parameters<Decide>) => {
      decide?.(...args);
    },
    showHealth: (shown: boolean) => {
      view.rerender(
        <ApiProvider client={client} queryClient={queryClient}>
          <GraphAndCheck health={shown} onDecide={onDecide} />
        </ApiProvider>,
      );
    },
  };
}

const ALPHA_GRAPH = "/api/projects/alpha/graph?ref=MEC-TIDES";
const ALPHA_CHECK = "/api/projects/alpha/check";
const ALPHA_QUEUE = ["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0001"];

describe("the graph and the check on the live tail (AC-12 of ui-live)", () => {
  it("proposal.applied on alpha reads alpha's graph on screen once and, Health on screen, its check once; nothing of beta", async () => {
    const { fetchStub, queryClient, alpha } = await graphAndCheck();
    act(() => {
      alpha.emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "60");
    });
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub).sort()).toEqual([...ALPHA_QUEUE, ALPHA_CHECK, ALPHA_GRAPH].sort());
  });

  it("proposal.created and the other proposal events read neither", async () => {
    const { fetchStub, queryClient, alpha } = await graphAndCheck();
    for (const type of ["proposal.created", "proposal.approved", "proposal.rejected", "proposal.apply_failed"]) {
      act(() => {
        alpha.emit(type, '{"id":"PR-0001"}', "61");
      });
      await quiet(queryClient, true);
    }
    expect(urlsOf(fetchStub).filter((url) => /\/(graph|check)/.test(url))).toEqual([]);
    expect(urlsOf(fetchStub)).toContain("/api/projects/alpha/inbox");
  });

  it("a stream this client opened again (a gap) reads both, once each", async () => {
    const { fetchStub, queryClient, alpha } = await graphAndCheck();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    alpha.fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    vi.useRealTimers();
    expect(fetchStub).not.toHaveBeenCalled();
    act(() => {
      stream("/api/projects/alpha/events").open();
    });
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub).sort()).toEqual([...ALPHA_QUEUE, ALPHA_CHECK, ALPHA_GRAPH].sort());
  });

  it("another project's event reads nothing of this one: beta's apply, beta's reads alone", async () => {
    const { fetchStub, queryClient, beta } = await graphAndCheck();
    act(() => {
      beta.emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "62");
    });
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub).sort()).toEqual(
      ["/api/projects/beta/inbox", "/api/projects/beta/proposals/PR-0001", "/api/projects/beta/check", "/api/projects/beta/graph?ref=MEC-TIDES"].sort(),
    );
  });

  it("Health off screen: an apply reads the graph, marks the check stale and reads it on entering Health", async () => {
    const { fetchStub, queryClient, alpha, showHealth } = await graphAndCheck();
    showHealth(false);
    act(() => {
      alpha.emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "63");
    });
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub).sort()).toEqual([...ALPHA_QUEUE, ALPHA_GRAPH].sort());
    expect(queryClient.getQueryState(queryKeys.check("alpha"))?.isInvalidated).toBe(true);
    expect(queryClient.getQueryState(queryKeys.check("beta"))?.isInvalidated).toBe(false);
    fetchStub.mockClear();
    showHealth(true);
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub)).toContain(ALPHA_CHECK);
  });

  it("an apply while Health's check is read again (Health entered, a report cached) starts no second walk: one /check", async () => {
    const { fetchStub, queryClient, alpha, hold, showHealth } = await graphAndCheck();
    showHealth(false);
    let release: () => void = () => undefined;
    hold.next = new Promise<void>((resolve) => {
      release = resolve;
    });
    fetchStub.mockClear();
    showHealth(true);
    await waitFor(() => {
      expect(urlsOf(fetchStub)).toContain(ALPHA_CHECK);
    });
    act(() => {
      alpha.emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "64");
    });
    release();
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub).filter((url) => url === ALPHA_CHECK)).toEqual([ALPHA_CHECK]);
    expect(urlsOf(fetchStub)).toContain(ALPHA_GRAPH);
  });

  it("an apply during Health's very first check: one /check", async () => {
    const hold: Hold = { next: null };
    let release: () => void = () => undefined;
    hold.next = new Promise<void>((resolve) => {
      release = resolve;
    });
    const fetchStub = graphAndCheckDaemon(hold);
    const queryClient = createQueryClient();
    render(
      <ApiProvider client={new HttpClient()} queryClient={queryClient}>
        <ProjectOnScreen project="alpha" health />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(urlsOf(fetchStub)).toContain(ALPHA_CHECK);
      expect(openStreams()).toEqual(["/api/projects/alpha/events"]);
    });
    stream("/api/projects/alpha/events").open();
    act(() => {
      stream("/api/projects/alpha/events").emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "65");
    });
    release();
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.check("alpha"))).toBeDefined();
      expect(queryClient.isFetching()).toBe(0);
    });
    expect(urlsOf(fetchStub).filter((url) => url === ALPHA_CHECK)).toEqual([ALPHA_CHECK]);
  });

  it("a decision made in the UI reads the graph again, never the check", async () => {
    const { fetchStub, queryClient, decide } = await graphAndCheck();
    act(() => {
      decide({ id: "PR-0001", decision: { decision: "accept", option: null, note: null } });
    });
    await waitFor(() => {
      expect(urlsOf(fetchStub)).toContain(ALPHA_GRAPH);
    });
    await quiet(queryClient, true);
    expect(urlsOf(fetchStub)).toContain("/api/projects/alpha/proposals/PR-0001/decision");
    expect(urlsOf(fetchStub).filter((url) => url.includes("/check"))).toEqual([]);
    expect(queryClient.getQueryState(queryKeys.check("alpha"))?.isInvalidated).toBe(false);
  });
});
