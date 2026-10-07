import type { QueryClient } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aCheckFinding, aCheckReport, aGraphNode, aGraphView, aProposal, aTaskEntry, aTaskPackage, entryOf } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, openStreams, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, QUEUE_EVENT_TYPES, REOPEN_FIRST_MS } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import {
  queryKeys,
  useBundle,
  useCheck,
  useDecideProposal,
  useGraph,
  useInbox,
  useLiveQueue,
  useNode,
  useProposal,
  useSearch,
  useTask,
  useTasks,
  useTree,
} from "./queries";
import type { Proposal, TaskListEntry } from "./types";

// AC-09 of docs/features/daemon-read.md: the live tail over a stubbed EventSource. A `proposal.*`
// event reads that project's inbox and that proposal again; `proposal.applied` also that project's
// trees, nodes, searches and bundles (R-n10); nothing of another project. A stream the browser
// resumes reads nothing again; one this client opened again after the browser gave up reads the
// project's inbox, proposals and spec reads (R-n6). The shell follows the shown project. AC-12 of
// docs/features/ui-live.md: the spec reads include the graphs and, while Health shows it, the check.
// AC-10 of docs/features/ui-live-tasks.md: a `task.*` event reads that project's task list and that
// task again (no string `id`: every task of it), nothing else; a `proposal.*` event and a gap read
// them too.

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
  return stubFetch((url, init) => graphAndCheckAnswer(url, init, hold));
}

/** How graphAndCheckDaemon answers one request. */
function graphAndCheckAnswer(url: string, init: RequestInit | undefined, hold: Hold = { next: null }): Response | Promise<Response> {
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

/** Each project's task list as the daemon holds it now: a test edits it before an event. */
type TaskBoard = Record<"alpha" | "beta", TaskListEntry[]>;

/** ui-live-tasks' daemon: the projects, each one's inbox, PR-0001, a tree, its task list and any task's package. */
function tasksDaemon(board: TaskBoard) {
  return stubFetch((url) => taskAnswer(board, url));
}

/** How tasksDaemon answers `url` from the board as it is now. */
function taskAnswer(board: TaskBoard, url: string): Response {
  if (url === "/api/projects") {
    return jsonAnswer(200, [
      { slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" },
      { slug: "beta", name: null, root: "/work/beta", branch: null },
    ]);
  }
  const match = /^\/api\/projects\/(alpha|beta)\/(inbox|tree|tasks|tasks\/(T-\d{4})|proposals\/PR-0001)$/.exec(url);
  if (match === null) {
    return errorAnswer(404, `no route ${url}`);
  }
  const project = match[1] === "beta" ? "beta" : "alpha";
  if (match[2] === "inbox") {
    return jsonAnswer(200, { proposals: [entryOf(aProposal({ id: "PR-0001", project }))], notes: [] });
  }
  if (match[2] === "tree") {
    return jsonAnswer(200, { ref: null, reason: null, notes: [], depth: null, kinds: [], archive: false, left_out: { generated: 0, tier3: 0 }, truncated: false, nodes: [] });
  }
  if (match[2] === "tasks") {
    return jsonAnswer(200, { tasks: board[project], notes: [] });
  }
  const id = match[3];
  if (id !== undefined) {
    const entry = board[project].find((candidate) => candidate.id === id);
    return entry === undefined
      ? jsonAnswer(404, { id, reason: `no task ${id} in this repository` })
      : jsonAnswer(200, aTaskPackage({ id, project, status: entry.status, title: entry.title, stale: entry.stale }));
  }
  return jsonAnswer(200, aProposal({ id: "PR-0001", project }));
}

/** Alpha's Tasks with T-0109 open, its inbox, PR-0001 and a tree on screen, alpha's live tail on; beta's list and T-0109 too. */
function TasksOnScreen() {
  const reads = [
    useTasks("alpha"),
    useTask("alpha", "T-0109"),
    useInbox("alpha"),
    useProposal("alpha", "PR-0001"),
    useTree("alpha", {}),
    useTasks("beta"),
    useTask("beta", "T-0109"),
  ];
  useLiveQueue("alpha");
  return <p>{reads.every((query) => query.data !== undefined && !query.isFetching) ? "ready" : "reading"}</p>;
}

const ALPHA_TASK_READS = ["/api/projects/alpha/tasks", "/api/projects/alpha/tasks/T-0109"];

/** Renders TasksOnScreen with alpha's T-0107 cached but not on screen; alpha's stream open; the fetch stub, cleared. */
async function tasksLive() {
  const board: TaskBoard = {
    alpha: [aTaskEntry({ id: "T-0107", status: "review" }), aTaskEntry({ id: "T-0109", status: "ready", stale: false })],
    beta: [aTaskEntry({ id: "T-0109", status: "draft" })],
  };
  const fetchStub = tasksDaemon(board);
  const queryClient = createQueryClient();
  queryClient.setQueryData(queryKeys.task("alpha", "T-0107"), aTaskPackage({ id: "T-0107", status: "review" }));
  render(
    <ApiProvider client={new HttpClient()} queryClient={queryClient}>
      <TasksOnScreen />
    </ApiProvider>,
  );
  await settled();
  expect(openStreams()).toEqual(["/api/projects/alpha/events"]);
  const source = stream("/api/projects/alpha/events");
  source.open();
  fetchStub.mockClear();
  return { fetchStub, queryClient, source, board };
}

/** Waits until nothing is read any more. */
async function still(queryClient: QueryClient) {
  await waitFor(() => {
    expect(queryClient.isFetching()).toBe(0);
  });
  await settled();
}

/** Whether a cached read was marked stale (read again once on screen). */
function stale(queryClient: QueryClient, key: readonly unknown[]): boolean | undefined {
  return queryClient.getQueryState(key)?.isInvalidated;
}

describe("the task reads on the live tail (AC-10 of ui-live-tasks)", () => {
  it("task.claimed on alpha reads alpha's list and that task once each; another cached task left as cached, unread; no inbox, proposal or spec read; nothing of beta", async () => {
    const { fetchStub, queryClient, source } = await tasksLive();
    act(() => {
      source.emit("task.claimed", '{"id":"T-0109","run":1}', "80");
    });
    await still(queryClient);
    expect(urlsOf(fetchStub).sort()).toEqual(ALPHA_TASK_READS);
    expect(stale(queryClient, queryKeys.task("alpha", "T-0107"))).toBe(false);
    expect(queryClient.getQueryState(queryKeys.task("alpha", "T-0107"))?.dataUpdateCount).toBe(1);
    expect(stale(queryClient, queryKeys.inbox("alpha"))).toBe(false);
    expect(stale(queryClient, queryKeys.tasks("beta"))).toBe(false);
    expect(stale(queryClient, queryKeys.task("beta", "T-0109"))).toBe(false);
  });

  it.each(QUEUE_EVENT_TYPES.filter((type) => type.startsWith("task.")))("%s with a string id reads alpha's list and that task, nothing else", async (type) => {
    const { fetchStub, queryClient, source } = await tasksLive();
    act(() => {
      source.emit(type, type === "task.refreshed" ? '{"id":"T-0109","proposal":"PR-0001","node":"MEC-TIDES"}' : '{"id":"T-0109"}', "81");
    });
    await still(queryClient);
    expect(urlsOf(fetchStub).sort()).toEqual(ALPHA_TASK_READS);
  });

  it("a task event naming another task reads the list, marks that task stale, and leaves the open one as it was", async () => {
    const { fetchStub, queryClient, source } = await tasksLive();
    act(() => {
      source.emit("task.approved", '{"id":"T-0107"}', "82");
    });
    await still(queryClient);
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/tasks"]);
    expect(stale(queryClient, queryKeys.task("alpha", "T-0107"))).toBe(true);
    expect(stale(queryClient, queryKeys.task("alpha", "T-0109"))).toBe(false);
  });

  it.each([
    ["no id", "{}"],
    ["a number for id", '{"id":7}'],
    ["no JSON", "not json"],
  ])("a task event with %s reads alpha's list and every task of alpha (T-0107 marked stale)", async (_name, data) => {
    const { fetchStub, queryClient, source } = await tasksLive();
    act(() => {
      source.emit("task.created", data, "83");
    });
    await still(queryClient);
    expect(urlsOf(fetchStub).sort()).toEqual(ALPHA_TASK_READS);
    expect(stale(queryClient, queryKeys.task("alpha", "T-0107"))).toBe(true);
    expect(stale(queryClient, queryKeys.task("beta", "T-0109"))).toBe(false);
  });

  it.each(QUEUE_EVENT_TYPES.filter((type) => type.startsWith("proposal.")))("%s reads the inbox, that proposal, alpha's list and its open task (WA-4)", async (type) => {
    const { fetchStub, queryClient, source } = await tasksLive();
    act(() => {
      source.emit(type, '{"id":"PR-0001"}', "84");
    });
    await still(queryClient);
    const spec = type === "proposal.applied" ? ["/api/projects/alpha/tree"] : [];
    expect(urlsOf(fetchStub).sort()).toEqual(["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0001", ...ALPHA_TASK_READS, ...spec].sort());
    expect(stale(queryClient, queryKeys.task("alpha", "T-0107"))).toBe(true);
    expect(stale(queryClient, queryKeys.tasks("beta"))).toBe(false);
    expect(stale(queryClient, queryKeys.task("beta", "T-0109"))).toBe(false);
  });

  it("a stream this client opened again (a gap) reads the queue, the spec reads and alpha's task reads", async () => {
    const { fetchStub, queryClient, source } = await tasksLive();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    source.fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    vi.useRealTimers();
    expect(fetchStub).not.toHaveBeenCalled();
    act(() => {
      stream("/api/projects/alpha/events").open();
    });
    await still(queryClient);
    expect(urlsOf(fetchStub).sort()).toEqual(
      ["/api/projects/alpha/inbox", "/api/projects/alpha/proposals/PR-0001", "/api/projects/alpha/tree", ...ALPHA_TASK_READS].sort(),
    );
    expect(stale(queryClient, queryKeys.task("alpha", "T-0107"))).toBe(true);
    expect(stale(queryClient, queryKeys.task("beta", "T-0109"))).toBe(false);
  });
});

/** What became of each request the stub held: aborted through its signal, or answered. */
interface Fates {
  aborted: string[];
  answered: string[];
  /** Answers every request still held and not aborted. */
  release: () => void;
}

/**
 * tasksDaemon whose reads matching `held` wait until `release()`; a held read whose signal aborts
 * rejects at once with the abort error, as the browser's fetch does, and is never answered.
 */
function heldTasksDaemon(board: TaskBoard, held: RegExp): Fates {
  const waiting: (() => void)[] = [];
  const fates: Fates = {
    aborted: [],
    answered: [],
    release: () => {
      for (const answer of waiting.splice(0)) {
        answer();
      }
    },
  };
  stubFetch((url, init) => {
    if (!held.test(url)) {
      return taskAnswer(board, url);
    }
    return new Promise<Response>((resolve, reject) => {
      const signal = init?.signal ?? undefined;
      signal?.addEventListener("abort", () => {
        fates.aborted.push(url);
        reject(new DOMException("The operation was aborted.", "AbortError"));
      });
      waiting.push(() => {
        if (signal?.aborted !== true) {
          fates.answered.push(url);
          resolve(taskAnswer(board, url));
        }
      });
    });
  });
  return fates;
}

describe("a burst of events on the live tail (AC-10 of ui-live-tasks, the review's burst)", () => {
  it("proposal.applied, then task.refreshed twice on the open task: the superseded reads are aborted, exactly one package read answered", async () => {
    const board: TaskBoard = {
      alpha: [aTaskEntry({ id: "T-0107", status: "review" }), aTaskEntry({ id: "T-0109", status: "in_progress", stale: false })],
      beta: [aTaskEntry({ id: "T-0109", status: "draft" })],
    };
    tasksDaemon(board);
    const queryClient = createQueryClient();
    render(
      <ApiProvider client={new HttpClient()} queryClient={queryClient}>
        <TasksOnScreen />
      </ApiProvider>,
    );
    await settled();
    const source = stream("/api/projects/alpha/events");
    source.open();
    const fates = heldTasksDaemon(board, /\/api\/projects\/alpha\/tasks/);

    // An agent's update to T-0109's target is applied: the apply, then two snapshot nodes refreshed.
    act(() => {
      source.emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "100");
    });
    await waitFor(() => {
      expect(queryClient.isFetching({ queryKey: queryKeys.task("alpha", "T-0109") })).toBe(1);
    });
    act(() => {
      source.emit("task.refreshed", '{"id":"T-0109","proposal":"PR-0001","node":"MEC-TIDES"}', "101");
    });
    act(() => {
      source.emit("task.refreshed", '{"id":"T-0109","proposal":"PR-0001","node":"RULE-TIDE-WINDOW"}', "102");
    });
    await waitFor(() => {
      expect(fates.aborted.filter((url) => url.endsWith("/tasks/T-0109"))).toHaveLength(2);
    });
    fates.release();
    await still(queryClient);

    const T_0109 = "/api/projects/alpha/tasks/T-0109";
    expect(fates.aborted.filter((url) => url === T_0109)).toEqual([T_0109, T_0109]);
    expect(fates.answered.filter((url) => url === T_0109)).toEqual([T_0109]);
    // The list read again with each event: likewise one answered, the rest aborted.
    expect(fates.answered.filter((url) => url === "/api/projects/alpha/tasks")).toEqual(["/api/projects/alpha/tasks"]);
    expect(fates.aborted.filter((url) => url === "/api/projects/alpha/tasks")).toHaveLength(2);
    expect(queryClient.getQueryState(queryKeys.task("alpha", "T-0109"))?.status).toBe("success");
    expect(screen.getByText("ready")).toBeTruthy();
  });

  it("a burst of applies aborts the superseded graph read, never the check in flight: one /check, sent with no signal", async () => {
    const waiting: (() => void)[] = [];
    const aborted: string[] = [];
    const checkInits: (RequestInit | undefined)[] = [];
    const answer = (url: string) =>
      url.endsWith("/check") ? jsonAnswer(200, aCheckReport()) : jsonAnswer(200, aGraphView([aGraphNode({ id: "MEC-TIDES", distance: 0 })], []));
    let holding = false;
    const fetchStub = stubFetch((url, init) => {
      if (url.endsWith("/check")) {
        checkInits.push(init);
      }
      if (!/\/(graph|check)/.test(url)) {
        return graphAndCheckAnswer(url, init);
      }
      if (!holding) {
        return answer(url);
      }
      return new Promise<Response>((resolve, reject) => {
        init?.signal?.addEventListener("abort", () => {
          aborted.push(url);
          reject(new DOMException("The operation was aborted.", "AbortError"));
        });
        waiting.push(() => {
          if (init?.signal?.aborted !== true) {
            resolve(answer(url));
          }
        });
      });
    });
    const queryClient = createQueryClient();
    render(
      <ApiProvider client={new HttpClient()} queryClient={queryClient}>
        <ProjectOnScreen project="alpha" health />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.check("alpha"))).toBeDefined();
      expect(queryClient.isFetching()).toBe(0);
    });
    stream("/api/projects/alpha/events").open();
    fetchStub.mockClear();
    checkInits.length = 0;
    holding = true;
    for (const seq of ["110", "111", "112"]) {
      act(() => {
        stream("/api/projects/alpha/events").emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', seq);
      });
      await act(async () => {
        await Promise.resolve();
      });
    }
    await waitFor(() => {
      expect(aborted.filter((url) => url.includes("/graph"))).toHaveLength(2);
    });
    holding = false;
    for (const release of waiting.splice(0)) {
      release();
    }
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
    expect(urlsOf(fetchStub).filter((url) => url === ALPHA_CHECK)).toEqual([ALPHA_CHECK]);
    expect(aborted.filter((url) => url.includes("/check"))).toEqual([]);
    expect(checkInits.map((init) => init !== undefined && "signal" in init)).toEqual([false]);
    expect(urlsOf(fetchStub).filter((url) => url === ALPHA_GRAPH)).toHaveLength(3);
  });
});

describe("the Tasks screen on the live tail (AC-10 of ui-live-tasks)", () => {
  it("shows an approval made on a terminal without a reload: task.approved, the row's new state", async () => {
    const board: TaskBoard = { alpha: [aTaskEntry({ id: "T-0107", status: "review", title: "Deep ships" })], beta: [] };
    tasksDaemon(board);
    window.history.replaceState(null, "", "/#/alpha/tasks");
    render(<App client={new HttpClient()} scenario={null} />);
    const list = await screen.findByRole("listbox", { name: "Tasks, what waits for you first" });
    const row = () => within(list).getByRole("option", { name: /T-0107/ });
    expect(row().textContent).toContain("Plan review");
    await waitFor(() => {
      expect(openStreams()).toEqual(["/api/projects/alpha/events"]);
    });
    const source = stream("/api/projects/alpha/events");
    source.open();

    // The owner runs `spec task approve T-0107` on a terminal; the daemon's tail says so.
    board.alpha = [aTaskEntry({ id: "T-0107", status: "ready", stale: false, title: "Deep ships" })];
    act(() => {
      source.emit("task.approved", '{"id":"T-0107"}', "90");
    });
    await waitFor(() => {
      expect(row().textContent).toContain("Ready");
    });
  });
});
