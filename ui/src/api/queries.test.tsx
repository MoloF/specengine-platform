import { QueryClient } from "@tanstack/react-query";
import { act, render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aBundle, aCheckReport, aGraphView, anEntry, aProposal, aSearchResults, aTaskPackage, aTreeView } from "../test/builders";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";
import { ClientError, DECIDED_ELSEWHERE } from "./client";
import { ApiProvider, createQueryClient } from "./provider";
import {
  queryKeys,
  useBundle,
  useCachedInbox,
  useCachedTasks,
  useCheck,
  useDecideProposal,
  useGraph,
  useInbox,
  useNode,
  useProjects,
  useProposal,
  useSearch,
  useTask,
  useTasks,
  useTree,
} from "./queries";

// AC-13 of docs/features/ui-tree-node.md: query keys carry every argument (absent as null, [] or
// false); a request carries only the options given; after a decision, success or 409, the
// project's inbox, tree, nodes, searches and bundles are read again.

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

function Decider({ onReady }: { onReady: (decide: ReturnType<typeof useDecideProposal>["mutate"]) => void }) {
  const decide = useDecideProposal("alpha");
  onReady(decide.mutate);
  return null;
}

function seeded(): QueryClient {
  const queryClient = createQueryClient();
  queryClient.setQueryData(queryKeys.inbox("alpha"), { proposals: [anEntry({ id: "PR-1" })], notes: [] });
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

const AFTER_DECISION = [
  queryKeys.inbox("alpha"),
  queryKeys.proposal("alpha", "PR-1"),
  queryKeys.proposal("alpha", "PR-2"),
  queryKeys.tree("alpha", { archive: true }),
  queryKeys.node("alpha", "R-1"),
  queryKeys.node("alpha", "R-1", { with: ["links"] }),
  queryKeys.search("alpha", { query: "x" }),
  queryKeys.bundle("alpha", { node_ids: ["R-1"] }),
  queryKeys.graph("alpha", { ref: "R-1", impact: true, types: ["t1"] }),
  queryKeys.tasks("alpha"),
  queryKeys.task("alpha", "T-0001"),
];

async function decideWith(client: StubClient) {
  const queryClient = seeded();
  let mutate: ReturnType<typeof useDecideProposal>["mutate"] | null = null;
  render(
    <ApiProvider client={client} queryClient={queryClient}>
      <Decider
        onReady={(next) => {
          mutate = next;
        }}
      />
    </ApiProvider>,
  );
  act(() => {
    mutate?.({ id: "PR-1", decision: { decision: "defer", note: null } });
  });
  await waitFor(() => {
    expect(client.decideProposal).toHaveBeenCalledTimes(1);
  });
  return queryClient;
}

describe("after a decision (AC-13)", () => {
  it("success: reads the project's inbox, tree, nodes, searches, bundles and graphs again; another project's stay", async () => {
    const queryClient = await decideWith(stubClient([aProposal({ id: "PR-1" })]));
    await waitFor(() => {
      expect(AFTER_DECISION.map((key) => queryClient.getQueryState(key)?.isInvalidated)).toEqual(AFTER_DECISION.map(() => true));
    });
    expect(queryClient.getQueryState(queryKeys.tree("beta"))?.isInvalidated).toBe(false);
    expect(queryClient.getQueryState(queryKeys.proposal("beta", "PR-2"))?.isInvalidated).toBe(false);
    expect(queryClient.getQueryState(queryKeys.graph("beta", { ref: "R-1" }))?.isInvalidated).toBe(false);
    expect(queryClient.getQueryState(queryKeys.task("beta", "T-0001"))?.isInvalidated).toBe(false);
    // AC-01 of ui-health: the check is a full walk, read on entering Health and on Check again only.
    expect(queryClient.getQueryState(queryKeys.check("alpha"))?.isInvalidated).toBe(false);
  });

  it("409: reads them again too", async () => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    client.decideProposal.mockRejectedValueOnce(new ClientError({ status: DECIDED_ELSEWHERE, message: "decided elsewhere" }));
    const queryClient = await decideWith(client);
    await waitFor(() => {
      expect(AFTER_DECISION.map((key) => queryClient.getQueryState(key)?.isInvalidated)).toEqual(AFTER_DECISION.map(() => true));
    });
  });

  it("another refusal: reads nothing again", async () => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    client.decideProposal.mockRejectedValueOnce(new ClientError({ status: 422, message: "refused" }));
    const queryClient = await decideWith(client);
    await act(async () => {
      await Promise.resolve();
    });
    expect(AFTER_DECISION.map((key) => queryClient.getQueryState(key)?.isInvalidated)).toEqual(AFTER_DECISION.map(() => false));
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
