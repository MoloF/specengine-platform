import { QueryClient } from "@tanstack/react-query";
import { act, render, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { aBundle, aCard, aCardList, aCardRow, aCheckReport, aGraphView, aHealthReport, anEntry, aProposal, aSearchResults, aTaskPackage, aTreeView } from "../test/builders";
import { argsOf, STAGED_AT, stubClient, type StubClient } from "../test/stubClient";
import { ClientError, NO_SUCH_PROPOSAL, STAGE_REFUSED } from "./client";
import { QUEUE_EVENT_TYPES } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import {
  queryKeys,
  useBundle,
  useCachedInbox,
  useCachedTasks,
  useCard,
  useCardClearGroup,
  useCards,
  useCardWrite,
  useCheck,
  flowForm,
  recordsForm,
  useFlowRead,
  useScopeNode,
  useScopeStages,
  useGraph,
  useHealth,
  useIdentity,
  useInbox,
  useLiveQueue,
  useNode,
  useProjects,
  useProposal,
  useSearch,
  useStageDecision,
  useTask,
  useTasks,
  useTeam,
  useTrace,
  useTree,
} from "./queries";

// AC-13 of docs/features/ui-tree-node.md: query keys carry every argument (absent as null, [] or
// false); a request carries only the options given. AC-14 of docs/features/decision-staging.md:
// after a stage or an unstage, success, 409 or 404, only the project's inbox and that proposal are
// read again, the entry kept in the inbox; a stage changes no task, spec read or check.

describe("query keys", () => {
  it("hold every argument, absent ones as null, [] or false", () => {
    expect(queryKeys.tree("p")).toEqual(["tree", "p", { root: null, depth: null, kinds: [], archive: false }]);
    expect(queryKeys.tree("p", { root: "R", depth: 2, kinds: ["k"], archive: true })).toEqual([
      "tree",
      "p",
      { root: "R", depth: 2, kinds: ["k"], archive: true },
    ]);
    expect(queryKeys.node("p", "R")).toEqual(["node", "p", "R", { with: [], archive: false }]);
    expect(queryKeys.node("p", "R", { with: ["links"], archive: true })).toEqual(["node", "p", "R", { with: ["links"], archive: true }]);
    expect(queryKeys.search("p", { query: "q" })).toEqual(["search", "p", { query: "q", kinds: [], limit: null, archive: false }]);
    expect(queryKeys.bundle("p", { node_ids: ["R"] })).toEqual(["bundle", "p", { node_ids: ["R"], budget: null }]);
    expect(queryKeys.bundle("p", { node_ids: ["R"], budget: 9 })).toEqual(["bundle", "p", { node_ids: ["R"], budget: 9 }]);
    expect(queryKeys.inbox("p")).toEqual(["inbox", "p"]);
    // AC-12 of docs/features/hub-identity.md: no project, a fact of the page.
    expect(queryKeys.identity).toEqual(["identity"]);
    // `docs/features/daemon-read.md` "Data": one proposal's review document.
    expect(queryKeys.proposal("p", "PR-0001")).toEqual(["proposal", "p", "PR-0001"]);
    // AC-08 of docs/features/ui-tasks.md.
    expect(queryKeys.tasks("p")).toEqual(["tasks", "p"]);
    expect(queryKeys.task("p", "T-0001")).toEqual(["task", "p", "T-0001"]);
    // AC-01 of ui-health.
    expect(queryKeys.check("p")).toEqual(["check", "p"]);
    // docs/features/health-left.md "Data".
    expect(queryKeys.health("p")).toEqual(["health", "p"]);
    // AC-03 of docs/features/ui-graph.md.
    // `kinds` and `both` since docs/features/corpus-map.md "Acceptance criteria", CM10: Both ways its own entry.
    expect(queryKeys.graph("p", { ref: "R" })).toEqual(["graph", "p", { ref: "R", kinds: [], impact: false, both: false, types: [], depth: null, archive: false }]);
    expect(queryKeys.graph("p", { ref: "R", impact: true, types: ["t2", "t1"], depth: 3, archive: true })).toEqual([
      "graph",
      "p",
      { ref: "R", kinds: [], impact: true, both: false, types: ["t2", "t1"], depth: 3, archive: true },
    ]);
    expect(queryKeys.graph("p", { ref: "R", both: true })).toEqual(["graph", "p", { ref: "R", kinds: [], impact: false, both: true, types: [], depth: null, archive: false }]);
    // docs/features/corpus-map.md "Acceptance criteria", CM8: the map, each archive its own entry.
    expect(queryKeys.map("p", false)).toEqual(["map", "p", false]);
    expect(queryKeys.map("p", true)).toEqual(["map", "p", true]);
  });

  it("tell graphs apart by every option, types included", () => {
    const keys = [
      queryKeys.graph("p", { ref: "R" }),
      queryKeys.graph("p", { ref: "R", types: ["t1"] }),
      queryKeys.graph("p", { ref: "R", types: ["t2"] }),
      queryKeys.graph("p", { ref: "R", impact: true }),
      queryKeys.graph("p", { ref: "R", depth: 1 }),
      queryKeys.graph("p", { ref: "R", archive: true }),
      queryKeys.graph("p", { ref: "S" }),
    ];
    expect(new Set(keys.map((key) => JSON.stringify(key))).size).toBe(keys.length);
  });

  it("tell a plain read from a links read and an archive read apart", () => {
    const keys = [queryKeys.node("p", "R"), queryKeys.node("p", "R", { with: ["links"] }), queryKeys.node("p", "R", { with: ["links"], archive: true })];
    expect(new Set(keys.map((key) => JSON.stringify(key))).size).toBe(3);
  });
});

function Reads({ budget }: { budget?: number }) {
  useTree("alpha", { archive: false });
  useTree("alpha", { root: "R-1", archive: true });
  useNode("alpha", "R-1");
  useNode("alpha", "R-1", { with: ["links"], archive: false });
  useSearch("alpha", { query: "a b", archive: false });
  useSearch("alpha", null);
  useBundle("alpha", budget === undefined ? { node_ids: ["R-1"] } : { node_ids: ["R-1"], budget }, true);
  useBundle("alpha", { node_ids: ["R-2"] }, false);
  useGraph("alpha", { ref: "R-1", impact: false, types: [], archive: false });
  useGraph("alpha", { ref: "R-2", impact: true, types: ["t1"], depth: 0, archive: true });
  useGraph("alpha", null);
  return null;
}

describe("requests", () => {
  it("carry only the options given: no false archive, no empty array, no absent key", async () => {
    const client = stubClient();
    render(
      <ApiProvider client={client}>
        <Reads />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(client.getBundle).toHaveBeenCalledTimes(1);
    });
    expect(argsOf(client.getTree)).toEqual([["alpha"], ["alpha", { root: "R-1", archive: true }]]);
    expect(argsOf(client.getNode)).toEqual([["alpha", "R-1"], ["alpha", "R-1", { with: ["links"] }]]);
    expect(argsOf(client.search)).toEqual([["alpha", { query: "a b" }]]);
    expect(argsOf(client.getBundle)).toEqual([["alpha", { node_ids: ["R-1"] }]]);
    expect(argsOf(client.getGraph)).toEqual([
      ["alpha", { ref: "R-1" }],
      ["alpha", { ref: "R-2", impact: true, types: ["t1"], depth: 0, archive: true }],
    ]);
  });
});

/** One of each read, the check among them. */
function EveryRead() {
  useProjects();
  useIdentity();
  useInbox("alpha");
  useCachedInbox("beta");
  useProposal("alpha", "PR-1");
  useTree("alpha", {});
  useNode("alpha", "R-1");
  useSearch("alpha", { query: "q" });
  useBundle("alpha", { node_ids: ["R-1"] }, true);
  useGraph("alpha", { ref: "R-1" });
  useFlowRead("alpha", flowForm(null, null));
  useFlowRead("alpha", flowForm("G-1", null));
  useTasks("alpha");
  useCachedTasks("beta");
  useTask("alpha", "T-0001");
  useCheck("alpha");
  useHealth("alpha");
  return null;
}

describe("the flow's reads (writer-studio D2)", () => {
  it("keys a read by [flow, project, root, depth, trace, stage, in_flow], root null at the top, stage null for all, the cut apart (flow-scenario D6)", () => {
    expect(queryKeys.flow("alpha", { depth: 0 })).toEqual(["flow", "alpha", null, 0, false, null, false]);
    expect(queryKeys.flow("alpha", { root: "G-1", depth: 1, trace: true })).toEqual(["flow", "alpha", "G-1", 1, true, null, false]);
    expect(queryKeys.flow("alpha", { root: "G-1", stage: "-", trace: true })).toEqual(["flow", "alpha", "G-1", null, true, "-", false]);
    expect(queryKeys.flow("alpha", { root: "G-1", depth: 1, in_flow: true, trace: true })).toEqual(["flow", "alpha", "G-1", 1, true, null, true]);
  });

  it("has three forms, each cut to the flow: the top at depth 0, a scope at depth 1, a stage under its root; never a whole read; the records' read uncut, no trace (flow-scenario D6)", () => {
    expect(flowForm(null, null)).toEqual({ depth: 0, in_flow: true, trace: true });
    expect(flowForm(null, "gather")).toEqual({ depth: 0, in_flow: true, trace: true });
    expect(flowForm("G-1", null)).toEqual({ root: "G-1", depth: 1, in_flow: true, trace: true });
    expect(flowForm("G-1", "gather")).toEqual({ root: "G-1", stage: "gather", in_flow: true, trace: true });
    expect(recordsForm("G-1")).toEqual({ root: "G-1", depth: 1 });
  });

  it("reads a picture, a scope's lanes and its node once; a key changing back reads nothing it holds", async () => {
    const client = stubClient();
    function Reads({ root, stage }: { root: string; stage: string | null }) {
      useFlowRead("alpha", flowForm(root, stage));
      useScopeStages("alpha", root);
      useScopeNode("alpha", root);
      return null;
    }
    const queryClient = createQueryClient();
    const mounted = (root: string, stage: string | null) => (
      <ApiProvider client={client} queryClient={queryClient}>
        <Reads root={root} stage={stage} />
      </ApiProvider>
    );
    const { rerender } = render(mounted("G-1", null));
    await waitFor(() => {
      expect(client.getFlow).toHaveBeenCalledTimes(1);
    });
    rerender(mounted("G-1", "gather"));
    await waitFor(() => {
      expect(client.getFlow).toHaveBeenCalledTimes(2);
    });
    rerender(mounted("G-1", null));
    rerender(mounted("G-1", "gather"));
    await waitFor(() => {
      expect(client.getNode).toHaveBeenCalledTimes(1);
    });
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([
      { root: "G-1", depth: 1, in_flow: true, trace: true },
      { root: "G-1", stage: "gather", in_flow: true, trace: true },
    ]);
    expect(argsOf(client.getTrace)).toEqual([["alpha", { ref: "G-1", summary: true }]]);
    expect(argsOf(client.getNode)).toEqual([["alpha", "G-1", { with: ["links"] }]]);
  });

  it("reads nothing while disabled, yet says what is held", async () => {
    const client = stubClient();
    const queryClient = createQueryClient();
    const { result, rerender } = renderHook(({ enabled }: { enabled: boolean }) => useFlowRead("alpha", flowForm(null, null), enabled).data, {
      initialProps: { enabled: false },
      wrapper: ({ children }: { children: ReactNode }) => (
        <ApiProvider client={client} queryClient={queryClient}>
          {children}
        </ApiProvider>
      ),
    });
    expect(client.getFlow).not.toHaveBeenCalled();
    expect(result.current).toBeUndefined();
    rerender({ enabled: true });
    await waitFor(() => {
      expect(result.current).toBeDefined();
    });
    rerender({ enabled: false });
    expect(result.current).toBeDefined();
    expect(client.getFlow).toHaveBeenCalledTimes(1);
  });
});

describe("abort signals (ui-live-tasks: a superseded read is aborted)", () => {
  it("hand every read its query's AbortSignal last, the check and the health read none: a walk in flight is never aborted (AC-U1 of health-left)", async () => {
    const client = stubClient();
    render(
      <ApiProvider client={client}>
        <EveryRead />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(client.getCheck).toHaveBeenCalledTimes(1);
      expect(client.getHealth).toHaveBeenCalledTimes(1);
      expect(client.getTask).toHaveBeenCalledTimes(1);
    });
    const reads = {
      getProjects: client.getProjects,
      getIdentity: client.getIdentity,
      getInbox: client.getInbox,
      getProposal: client.getProposal,
      getTree: client.getTree,
      getNode: client.getNode,
      search: client.search,
      getBundle: client.getBundle,
      getGraph: client.getGraph,
      getFlow: client.getFlow,
      getTasks: client.getTasks,
      getTask: client.getTask,
    };
    for (const [name, read] of Object.entries(reads)) {
      const calls: readonly (readonly unknown[])[] = read.mock.calls;
      expect([name, calls.length > 0 && calls.every((call) => call.at(-1) instanceof AbortSignal)]).toEqual([name, true]);
    }
    expect(client.getCheck.mock.calls).toEqual([["alpha"]]);
    expect(client.getHealth.mock.calls).toEqual([["alpha"]]);
  });
});

describe("useTeam (A9 of hub-team)", () => {
  function Reader({ enabled }: { enabled: boolean }) {
    useTeam("alpha", enabled);
    return null;
  }

  function reading(client: StubClient, queryClient: QueryClient, enabled = true) {
    return render(
      <ApiProvider client={client} queryClient={queryClient}>
        <Reader enabled={enabled} />
      </ApiProvider>,
    );
  }

  const ROW = { login: "mira", name: "Mira K", allowed: ["stage"] };

  it("reads nothing until enabled: a local daemon has no such route", async () => {
    const client = stubClient();
    const queryClient = createQueryClient();
    const first = reading(client, queryClient, false);
    await act(() => new Promise((settle) => setTimeout(settle, 0)));
    expect(client.getTeam).not.toHaveBeenCalled();
    first.unmount();
    client.getTeam.mockResolvedValue(ROW);
    reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.team("alpha"))).toEqual(ROW);
    });
  });

  it("keys a read by project and hands the fetch its AbortSignal", async () => {
    expect(queryKeys.team("alpha")).toEqual(["team", "alpha"]);
    const client = stubClient();
    client.getTeam.mockResolvedValue(ROW);
    reading(client, createQueryClient());
    await waitFor(() => {
      expect(client.getTeam).toHaveBeenCalledTimes(1);
    });
    expect(client.getTeam.mock.calls[0]?.[0]).toBe("alpha");
    expect(client.getTeam.mock.calls[0]?.[1]).toBeInstanceOf(AbortSignal);
  });

  it("is one read for every field that asks, none again when a new one mounts", async () => {
    const client = stubClient();
    client.getTeam.mockResolvedValue(ROW);
    const queryClient = createQueryClient();
    const first = reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.team("alpha"))).toEqual(ROW);
    });
    first.unmount();
    reading(client, queryClient);
    await act(() => new Promise((settle) => setTimeout(settle, 0)));
    expect(client.getTeam).toHaveBeenCalledTimes(1);
  });

  it("keeps no answer of a failed read: the next to mount asks again", async () => {
    const client = stubClient();
    client.getTeam.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec index unavailable" }));
    client.getTeam.mockResolvedValue(ROW);
    const queryClient = createQueryClient();
    const first = reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryState(queryKeys.team("alpha"))?.status).toBe("error");
    });
    first.unmount();
    reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.team("alpha"))).toEqual(ROW);
    });
    expect(client.getTeam).toHaveBeenCalledTimes(2);
  });
});

describe("useIdentity (AC-12 of hub-identity)", () => {
  function Reader() {
    useIdentity();
    return null;
  }

  function reading(client: StubClient, queryClient: QueryClient) {
    return render(
      <ApiProvider client={client} queryClient={queryClient}>
        <Reader />
      </ApiProvider>,
    );
  }

  it("is one read for every field that asks, none again when a new one mounts", async () => {
    const client = stubClient();
    client.getIdentity.mockResolvedValue({ login: "mira" });
    const queryClient = createQueryClient();
    const first = reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.identity)).toEqual({ login: "mira" });
    });
    first.unmount();
    reading(client, queryClient);
    expect(client.getIdentity).toHaveBeenCalledTimes(1);
  });

  it("outlives every field on the page, so coming back never shows it loading", async () => {
    const client = stubClient();
    client.getIdentity.mockResolvedValue({ login: "mira" });
    const queryClient = createQueryClient();
    const view = reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.identity)).toEqual({ login: "mira" });
    });
    vi.useFakeTimers();
    view.unmount();
    vi.advanceTimersByTime(24 * 60 * 60 * 1000);
    expect(queryClient.getQueryData(queryKeys.identity)).toEqual({ login: "mira" });
  });

  it("keeps no answer of a failed read: the next to mount asks again", async () => {
    const client = stubClient();
    client.getIdentity.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec index unavailable" }));
    const queryClient = createQueryClient();
    const first = reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryState(queryKeys.identity)?.status).toBe("error");
    });
    first.unmount();
    reading(client, queryClient);
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.identity)).toEqual({ login: null });
    });
    expect(client.getIdentity).toHaveBeenCalledTimes(2);
  });
});

type StageMutate = ReturnType<typeof useStageDecision>["mutate"];

function Stager({ onReady }: { onReady: (stage: StageMutate) => void }) {
  const stage = useStageDecision("alpha");
  onReady(stage.mutate);
  return null;
}

function seeded(): QueryClient {
  const queryClient = createQueryClient();
  queryClient.setQueryData(queryKeys.inbox("alpha"), { proposals: [anEntry({ id: "PR-2" }), anEntry({ id: "PR-1" })], notes: [] });
  queryClient.setQueryData(queryKeys.proposal("alpha", "PR-1"), aProposal({ id: "PR-1" }));
  queryClient.setQueryData(queryKeys.proposal("alpha", "PR-2"), aProposal({ id: "PR-2" }));
  queryClient.setQueryData(queryKeys.proposal("beta", "PR-2"), aProposal({ id: "PR-2" }));
  queryClient.setQueryData(queryKeys.tree("alpha", { archive: true }), aTreeView([]));
  queryClient.setQueryData(queryKeys.node("alpha", "R-1"), { ref: "R-1", reason: null, notes: [], nodes: [] });
  queryClient.setQueryData(queryKeys.node("alpha", "R-1", { with: ["links"] }), { ref: "R-1", reason: null, notes: [], nodes: [] });
  queryClient.setQueryData(queryKeys.search("alpha", { query: "x" }), aSearchResults([]));
  queryClient.setQueryData(queryKeys.bundle("alpha", { node_ids: ["R-1"] }), aBundle(["R-1"]));
  queryClient.setQueryData(queryKeys.graph("alpha", { ref: "R-1", impact: true, types: ["t1"] }), aGraphView([], []));
  queryClient.setQueryData(queryKeys.tasks("alpha"), { tasks: [], notes: [] });
  queryClient.setQueryData(queryKeys.task("alpha", "T-0001"), aTaskPackage({ id: "T-0001" }));
  queryClient.setQueryData(queryKeys.tree("beta"), aTreeView([]));
  queryClient.setQueryData(queryKeys.task("beta", "T-0001"), aTaskPackage({ id: "T-0001" }));
  queryClient.setQueryData(queryKeys.graph("beta", { ref: "R-1" }), aGraphView([], []));
  queryClient.setQueryData(queryKeys.check("alpha"), aCheckReport());
  queryClient.setQueryData(queryKeys.health("alpha"), aHealthReport());
  return queryClient;
}

/** The two reads a stage of PR-1 reads again. */
const AFTER_STAGE = [queryKeys.inbox("alpha"), queryKeys.proposal("alpha", "PR-1")];

/** Every other seeded read: none is read again after a stage. */
const UNTOUCHED = [
  queryKeys.proposal("alpha", "PR-2"),
  queryKeys.tree("alpha", { archive: true }),
  queryKeys.node("alpha", "R-1"),
  queryKeys.node("alpha", "R-1", { with: ["links"] }),
  queryKeys.search("alpha", { query: "x" }),
  queryKeys.bundle("alpha", { node_ids: ["R-1"] }),
  queryKeys.graph("alpha", { ref: "R-1", impact: true, types: ["t1"] }),
  queryKeys.tasks("alpha"),
  queryKeys.task("alpha", "T-0001"),
  queryKeys.check("alpha"),
  queryKeys.health("alpha"),
  queryKeys.proposal("beta", "PR-2"),
  queryKeys.tree("beta"),
  queryKeys.task("beta", "T-0001"),
  queryKeys.graph("beta", { ref: "R-1" }),
];

async function stageWith(client: StubClient, change: Parameters<StageMutate>[0]) {
  const queryClient = seeded();
  let mutate: StageMutate | null = null;
  render(
    <ApiProvider client={client} queryClient={queryClient}>
      <Stager
        onReady={(next) => {
          mutate = next;
        }}
      />
    </ApiProvider>,
  );
  act(() => {
    mutate?.(change);
  });
  await waitFor(() => {
    expect(client.stageDecision.mock.calls.length + client.unstageDecision.mock.calls.length).toBe(1);
  });
  return queryClient;
}

const STAGE = { id: "PR-1", stage: { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: "ok" }, updatedAt: "2026-10-01T10:00:00Z" } as const;

function invalidated(queryClient: QueryClient, keys: readonly (readonly unknown[])[]): (boolean | undefined)[] {
  return keys.map((key) => queryClient.getQueryState(key)?.isInvalidated);
}

describe("after a stage (AC-14 of decision-staging)", () => {
  it("success: keeps the entry with the returned staged_at, sets the returned document, reads only that inbox and proposal again", async () => {
    const client = stubClient([aProposal({ id: "PR-1" }), aProposal({ id: "PR-2" })]);
    const queryClient = await stageWith(client, STAGE);
    await waitFor(() => {
      expect(invalidated(queryClient, AFTER_STAGE)).toEqual([true, true]);
    });
    expect(invalidated(queryClient, UNTOUCHED)).toEqual(UNTOUCHED.map(() => false));
    expect(client.stageDecision.mock.calls).toEqual([["alpha", "PR-1", STAGE.stage, STAGE.updatedAt]]);
    const inbox = queryClient.getQueryData<{ proposals: { id: string; staged_at: string | null }[] }>(queryKeys.inbox("alpha"));
    expect(inbox?.proposals.map((entry) => [entry.id, entry.staged_at])).toEqual([
      ["PR-2", null],
      ["PR-1", STAGED_AT],
    ]);
    expect(queryClient.getQueryData<{ staged_at: string | null }>(queryKeys.proposal("alpha", "PR-1"))?.staged_at).toBe(STAGED_AT);
  });

  it("an unstage: one DELETE, the same two reads again, nothing else", async () => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    const queryClient = await stageWith(client, { id: "PR-1", stage: null });
    await waitFor(() => {
      expect(invalidated(queryClient, AFTER_STAGE)).toEqual([true, true]);
    });
    expect(invalidated(queryClient, UNTOUCHED)).toEqual(UNTOUCHED.map(() => false));
    expect(client.unstageDecision.mock.calls).toEqual([["alpha", "PR-1"]]);
    expect(client.stageDecision).not.toHaveBeenCalled();
  });

  it.each([
    ["409", STAGE_REFUSED],
    ["404", NO_SUCH_PROPOSAL],
  ])("%s: reads that inbox and proposal again, nothing else", async (_name, status) => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    client.stageDecision.mockRejectedValueOnce(new ClientError({ status, message: "refused" }));
    const queryClient = await stageWith(client, STAGE);
    await waitFor(() => {
      expect(invalidated(queryClient, AFTER_STAGE)).toEqual([true, true]);
    });
    expect(invalidated(queryClient, UNTOUCHED)).toEqual(UNTOUCHED.map(() => false));
  });

  it("another refusal: reads nothing again", async () => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    client.stageDecision.mockRejectedValueOnce(new ClientError({ status: 400, message: "refused" }));
    const queryClient = await stageWith(client, STAGE);
    await act(async () => {
      await Promise.resolve();
    });
    expect(invalidated(queryClient, [...AFTER_STAGE, ...UNTOUCHED])).toEqual([...AFTER_STAGE, ...UNTOUCHED].map(() => false));
  });
});

function OneTask({ id, onData }: { id: string; onData: (data: unknown) => void }) {
  const task = useTask("alpha", id);
  onData(task.data);
  return null;
}

describe("a task's read (AC-11 of ui-tasks)", () => {
  it("never answers a new T with the previous T's package while it is read", async () => {
    const client = stubClient();
    client.getTask.mockImplementation((_project, id) =>
      id === "T-0001" ? Promise.resolve(aTaskPackage({ id: "T-0001" })) : new Promise(() => undefined),
    );
    const seen: unknown[] = [];
    const { rerender } = render(
      <ApiProvider client={client}>
        <OneTask id="T-0001" onData={(data) => seen.push(data)} />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(seen.at(-1)).toMatchObject({ id: "T-0001" });
    });
    seen.length = 0;
    rerender(
      <ApiProvider client={client}>
        <OneTask id="T-0002" onData={(data) => seen.push(data)} />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(argsOf(client.getTask)).toContainEqual(["alpha", "T-0002"]);
    });
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.every((data) => data === undefined)).toBe(true);
  });
});

describe("the cards (docs/features/board-ui.md \"Board\", \"Moving\")", () => {
  type Write = ReturnType<typeof useCardWrite>;

  function Board({ withCancelled = false, onWrite }: { withCancelled?: boolean; onWrite?: (write: Write) => void }) {
    useCards("alpha", withCancelled ? { with_cancelled: true } : {});
    useCard("alpha", "C-0001");
    useCard("alpha", "C-0002");
    const write = useCardWrite("alpha");
    onWrite?.(write);
    return null;
  }

  async function mounted(client: StubClient, onWrite?: (write: Write) => void) {
    const queryClient = createQueryClient();
    render(
      <ApiProvider client={client} queryClient={queryClient}>
        <Board onWrite={onWrite} />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
    return queryClient;
  }

  function cardsClient() {
    const client = stubClient();
    client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-0001" })]));
    client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id })));
    return client;
  }

  it("keys the list by project and Show cancelled, a card by its ID", () => {
    expect(queryKeys.cards("alpha")).toEqual(["cards", "alpha", { with_cancelled: false }]);
    expect(queryKeys.cards("alpha", { with_cancelled: true })).toEqual(["cards", "alpha", { with_cancelled: true }]);
    expect(queryKeys.card("alpha", "C-0001")).toEqual(["card", "alpha", "C-0001"]);
  });

  it("always asks the marks, so no tile is drawn without one, and no other option unless cancelled cards are asked (AC-09 of trace-views)", async () => {
    const client = cardsClient();
    const queryClient = await mounted(client);
    expect(argsOf(client.getCards)).toEqual([["alpha", { marks: true }]]);
    render(
      <ApiProvider client={client} queryClient={queryClient}>
        <Board withCancelled />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(argsOf(client.getCards)).toEqual([["alpha", { marks: true }], ["alpha", { with_cancelled: true, marks: true }]]);
    });
  });

  it("after a write reads every list and that card again, and settles only then: the board is the daemon's", async () => {
    const client = cardsClient();
    let write: Write | undefined;
    await mounted(client, (latest) => {
      write = latest;
    });
    client.getCards.mockClear();
    client.getCard.mockClear();
    let settledAfter = -1;
    await act(async () => {
      await write?.mutateAsync({ write: "move", id: "C-0001", body: { to: "doing", revision: "1", as: null } });
      settledAfter = client.getCards.mock.calls.length;
    });
    expect(settledAfter).toBe(1);
    expect(argsOf(client.getCard)).toEqual([["alpha", "C-0001"]]);
    expect(client.moveCard.mock.calls).toEqual([["alpha", "C-0001", { to: "doing", revision: "1", as: null }]]);
  });

  it.each([
    [409, "`C-0001` changed since this run read it"],
    [404, "no card C-0001"],
    [0, "no response from the daemon"],
  ])("after a %i reads them again too: the answer rejects as it came", async (status, message) => {
    const client = cardsClient();
    client.moveCard.mockRejectedValue(new ClientError({ status, message }));
    let write: Write | undefined;
    await mounted(client, (latest) => {
      write = latest;
    });
    client.getCards.mockClear();
    await act(async () => {
      await expect(write?.mutateAsync({ write: "move", id: "C-0001", body: { to: "doing", revision: "1", as: null } })).rejects.toThrow(message);
    });
    expect(client.getCards).toHaveBeenCalledTimes(1);
  });

  it("after a refusal that stored nothing (400, 503) reads nothing", async () => {
    const client = cardsClient();
    client.moveCard.mockRejectedValue(new ClientError({ status: 400, message: "spec: --as: too long" }));
    let write: Write | undefined;
    await mounted(client, (latest) => {
      write = latest;
    });
    client.getCards.mockClear();
    await act(async () => {
      await expect(write?.mutateAsync({ write: "move", id: "C-0001", body: { to: "doing", revision: "1", as: "x" } })).rejects.toThrow("too long");
    });
    expect(client.getCards).not.toHaveBeenCalled();
  });

  it("creates through the list's path and reads no card it does not know", async () => {
    const client = cardsClient();
    client.createCard.mockRejectedValue(new ClientError({ status: 409, message: "no flow takes a card without a discipline" }));
    let write: Write | undefined;
    await mounted(client, (latest) => {
      write = latest;
    });
    client.getCard.mockClear();
    await act(async () => {
      await expect(write?.mutateAsync({ write: "create", body: { title: "T", as: null } })).rejects.toThrow("no flow takes");
    });
    expect(client.getCard).not.toHaveBeenCalled();
  });
});

describe("a card's linked tasks on the live tail (CT-5 of docs/features/card-tasks.md, D13)", () => {
  type Tail = Parameters<StubClient["subscribe"]>;

  /** The Board with C-0001 open, C-0002 read before and left: the list, one card on screen, the live tail. */
  function OpenCard() {
    useLiveQueue("alpha");
    useCards("alpha");
    useCard("alpha", "C-0001");
    return null;
  }

  async function onScreen() {
    const client = stubClient();
    client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002", position: 2 })]));
    client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, tasks: [{ id: "T-0001", status: "draft", title: "Light the north lamp", stale: null }] })));
    let tail: Tail[1] = () => undefined;
    const subscribe: StubClient["subscribe"] = (_project, onEvent) => {
      tail = onEvent;
      return () => undefined;
    };
    const queryClient = createQueryClient();
    // C-0002 was open before: cached, not on screen.
    queryClient.setQueryData(queryKeys.card("alpha", "C-0002"), aCard({ id: "C-0002" }));
    render(
      <ApiProvider client={{ ...client, subscribe }} queryClient={queryClient}>
        <OpenCard />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
    client.getCards.mockClear();
    client.getCard.mockClear();
    return {
      client,
      queryClient,
      tail: (event: Parameters<Tail[1]>[0]) => {
        tail(event);
      },
    };
  }

  async function quiet(queryClient: QueryClient) {
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
  }

  it.each(QUEUE_EVENT_TYPES.filter((type) => type.startsWith("task.")))("%s reads the open card once and the list never: a card serves its tasks as read now", async (type) => {
    const { client, queryClient, tail } = await onScreen();
    act(() => {
      tail({ seq: 70, type, payload: { id: "T-0001" } });
    });
    await quiet(queryClient);
    expect(argsOf(client.getCard)).toEqual([["alpha", "C-0001"]]);
    expect(client.getCards).not.toHaveBeenCalled();
    // The card not on screen is marked stale, read when it opens again.
    expect(queryClient.getQueryState(queryKeys.card("alpha", "C-0002"))?.isInvalidated).toBe(true);
  });

  it("a task event naming another task, or none, reads the open card all the same: no read says which cards link a task", async () => {
    const { client, queryClient, tail } = await onScreen();
    act(() => {
      tail({ seq: 71, type: "task.claimed", payload: { id: "T-0042", run: 1 } });
    });
    await quiet(queryClient);
    act(() => {
      tail({ seq: 72, type: "task.created", payload: "not JSON" });
    });
    await quiet(queryClient);
    expect(argsOf(client.getCard)).toEqual([
      ["alpha", "C-0001"],
      ["alpha", "C-0001"],
    ]);
    expect(client.getCards).not.toHaveBeenCalled();
  });

  it("the served state lands in the open card's read: after task.planned the card holds `review`", async () => {
    const { client, queryClient, tail } = await onScreen();
    client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, tasks: [{ id: "T-0001", status: "review", title: "Light the north lamp", stale: null }] })));
    act(() => {
      tail({ seq: 73, type: "task.planned", payload: { id: "T-0001" } });
    });
    await quiet(queryClient);
    const card = queryClient.getQueryData<ReturnType<typeof aCard>>(queryKeys.card("alpha", "C-0001"));
    expect(card?.tasks.map((task) => task.status)).toEqual(["review"]);
  });

  it("proposal.applied reads the open card once: an apply may move a linked task's `stale`; the list is read once, for its marks, as before, never again for the tasks", async () => {
    const { client, queryClient, tail } = await onScreen();
    client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, tasks: [{ id: "T-0001", status: "in_progress", title: "Light the north lamp", stale: true }] })));
    act(() => {
      tail({ seq: 75, type: "proposal.applied", payload: { id: "PR-0001" } });
    });
    await quiet(queryClient);
    expect(argsOf(client.getCard)).toEqual([["alpha", "C-0001"]]);
    expect(client.getCards).toHaveBeenCalledTimes(1);
    expect(queryClient.getQueryState(queryKeys.card("alpha", "C-0002"))?.isInvalidated).toBe(true);
    const card = queryClient.getQueryData<ReturnType<typeof aCard>>(queryKeys.card("alpha", "C-0001"));
    expect(card?.tasks.map((task) => task.stale)).toEqual([true]);
  });

  // Since card-questions (M1 of its review; docs/canon/card-questions.md "The rule"): a card serves the
  // items naming it and their events read now, and no event says which card names an item, so every
  // proposal event reads the open card once, the one not on screen marked stale.
  it.each(["proposal.created", "proposal.approved", "proposal.rejected", "proposal.apply_failed", "proposal.staged", "proposal.unstaged"])("%s reads the open card once: a card serves its items as read now", async (type) => {
    const { client, queryClient, tail } = await onScreen();
    act(() => {
      tail({ seq: 76, type, payload: { id: "PR-0001" } });
    });
    await quiet(queryClient);
    expect(argsOf(client.getCard)).toEqual([["alpha", "C-0001"]]);
    expect(queryClient.getQueryState(queryKeys.card("alpha", "C-0002"))?.isInvalidated).toBe(true);
  });

  it("a raise reads the open card and never the list: the list shows no item", async () => {
    const { client, queryClient, tail } = await onScreen();
    act(() => {
      tail({ seq: 74, type: "proposal.created", payload: { id: "PR-0001" } });
    });
    await quiet(queryClient);
    expect(argsOf(client.getCard)).toEqual([["alpha", "C-0001"]]);
    expect(client.getCards).not.toHaveBeenCalled();
  });
});

describe("the trace and the clears (docs/features/card-trace.md)", () => {
  it("keys a trace by project, REF, stage and summary, null the whole project; no option goes out unasked", async () => {
    expect(queryKeys.trace("alpha")).toEqual(["trace", "alpha", null, null, false]);
    expect(queryKeys.trace("alpha", { ref: "R-1" })).toEqual(["trace", "alpha", "R-1", null, false]);
    expect(queryKeys.trace("alpha", { stage: "print" })).toEqual(["trace", "alpha", null, "print", false]);
    expect(queryKeys.trace("alpha", { summary: true })).toEqual(["trace", "alpha", null, null, true]);
    const client = stubClient();
    function Reads() {
      useTrace("alpha");
      useTrace("alpha", { ref: "R-1" });
      useTrace("alpha", { stage: "print" });
      useTrace("alpha", { summary: true });
      useTrace("alpha", { summary: false, stage: "held" }, false);
      return null;
    }
    render(
      <ApiProvider client={client}>
        <Reads />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(argsOf(client.getTrace)).toEqual([["alpha"], ["alpha", { ref: "R-1" }], ["alpha", { stage: "print" }], ["alpha", { summary: true }]]);
    });
  });

  it("a clear goes out as a write, and the list, the card and the trace are read before it settles", async () => {
    const client = stubClient();
    let write: ReturnType<typeof useCardWrite> | undefined;
    function Board({ onWrite }: { onWrite: (latest: ReturnType<typeof useCardWrite>) => void }) {
      useTrace("alpha");
      useCards("alpha");
      onWrite(useCardWrite("alpha"));
      return null;
    }
    const queryClient = createQueryClient();
    render(
      <ApiProvider client={client} queryClient={queryClient}>
        <Board
          onWrite={(latest) => {
            write = latest;
          }}
        />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
    client.getTrace.mockClear();
    client.getCards.mockClear();
    const body = { outcome: "no_effect", reason: "Wording only", revision: "3", as: "Mira Holt" } as const;
    await act(async () => {
      await write?.mutateAsync({ write: "clear", id: "C-0001", body });
    });
    expect(client.clearCard.mock.calls).toEqual([["alpha", "C-0001", body]]);
    expect([client.getTrace.mock.calls.length, client.getCards.mock.calls.length]).toEqual([1, 1]);
  });

  it("a group clear reads every card read again", async () => {
    const client = stubClient();
    let clear: ReturnType<typeof useCardClearGroup> | undefined;
    function Board({ onClear }: { onClear: (latest: ReturnType<typeof useCardClearGroup>) => void }) {
      useCard("alpha", "C-0001");
      useTrace("alpha");
      onClear(useCardClearGroup("alpha"));
      return null;
    }
    const queryClient = createQueryClient();
    render(
      <ApiProvider client={client} queryClient={queryClient}>
        <Board
          onClear={(latest) => {
            clear = latest;
          }}
        />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
    client.getCard.mockClear();
    client.getTrace.mockClear();
    await act(async () => {
      await clear?.mutateAsync({ group: "PRN-WEEKLY", reason: "Wording only", as: "Mira Holt" });
    });
    expect(client.clearCardGroup.mock.calls).toEqual([["alpha", { group: "PRN-WEEKLY", reason: "Wording only", as: "Mira Holt" }]]);
    expect([client.getCard.mock.calls.length, client.getTrace.mock.calls.length]).toEqual([1, 1]);
  });
});
