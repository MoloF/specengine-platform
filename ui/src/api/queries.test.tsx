import { QueryClient } from "@tanstack/react-query";
import { act, render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aBundle, aCheckReport, aGraphView, anEntry, aProposal, aSearchResults, aTaskPackage, aTreeView } from "../test/builders";
import { argsOf, STAGED_AT, stubClient, type StubClient } from "../test/stubClient";
import { ClientError, NO_SUCH_PROPOSAL, STAGE_REFUSED } from "./client";
import { ApiProvider, createQueryClient } from "./provider";
import {
  queryKeys,
  useBundle,
  useCachedInbox,
  useCachedTasks,
  useCheck,
  useGraph,
  useInbox,
  useNode,
  useProjects,
  useProposal,
  useSearch,
  useStageDecision,
  useTask,
  useTasks,
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
    // `docs/features/daemon-read.md` "Data": one proposal's review document.
    expect(queryKeys.proposal("p", "PR-0001")).toEqual(["proposal", "p", "PR-0001"]);
    // AC-08 of docs/features/ui-tasks.md.
    expect(queryKeys.tasks("p")).toEqual(["tasks", "p"]);
    expect(queryKeys.task("p", "T-0001")).toEqual(["task", "p", "T-0001"]);
    // AC-01 of ui-health.
    expect(queryKeys.check("p")).toEqual(["check", "p"]);
    // AC-03 of docs/features/ui-graph.md.
    expect(queryKeys.graph("p", { ref: "R" })).toEqual(["graph", "p", { ref: "R", impact: false, types: [], depth: null, archive: false }]);
    expect(queryKeys.graph("p", { ref: "R", impact: true, types: ["t2", "t1"], depth: 3, archive: true })).toEqual([
      "graph",
      "p",
      { ref: "R", impact: true, types: ["t2", "t1"], depth: 3, archive: true },
    ]);
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
  useInbox("alpha");
  useCachedInbox("beta");
  useProposal("alpha", "PR-1");
  useTree("alpha", {});
  useNode("alpha", "R-1");
  useSearch("alpha", { query: "q" });
  useBundle("alpha", { node_ids: ["R-1"] }, true);
  useGraph("alpha", { ref: "R-1" });
  useTasks("alpha");
  useCachedTasks("beta");
  useTask("alpha", "T-0001");
  useCheck("alpha");
  return null;
}

describe("abort signals (ui-live-tasks: a superseded read is aborted)", () => {
  it("hand every read its query's AbortSignal last, the check none: a walk in flight is never aborted", async () => {
    const client = stubClient();
    render(
      <ApiProvider client={client}>
        <EveryRead />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(client.getCheck).toHaveBeenCalledTimes(1);
      expect(client.getTask).toHaveBeenCalledTimes(1);
    });
    const reads = {
      getProjects: client.getProjects,
      getInbox: client.getInbox,
      getProposal: client.getProposal,
      getTree: client.getTree,
      getNode: client.getNode,
      search: client.search,
      getBundle: client.getBundle,
      getGraph: client.getGraph,
      getTasks: client.getTasks,
      getTask: client.getTask,
    };
    for (const [name, read] of Object.entries(reads)) {
      const calls: readonly (readonly unknown[])[] = read.mock.calls;
      expect([name, calls.length > 0 && calls.every((call) => call.at(-1) instanceof AbortSignal)]).toEqual([name, true]);
    }
    expect(client.getCheck.mock.calls).toEqual([["alpha"]]);
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

const STAGE = { id: "PR-1", stage: { decision: "approve", option: null, answer: null, canon: null, note: "ok" }, updatedAt: "2026-10-01T10:00:00Z" } as const;

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
