import { QueryClient } from "@tanstack/react-query";
import { act, render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aBundle, aGraphView, aProposal, aSearchResults, aTreeView } from "../test/builders";
import { stubClient, type StubClient } from "../test/stubClient";
import { ClientError, DECIDED_ELSEWHERE } from "./client";
import { ApiProvider, createQueryClient } from "./provider";
import { queryKeys, useBundle, useDecideProposal, useGraph, useNode, useSearch, useTree } from "./queries";

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
    expect(client.getTree.mock.calls).toEqual([["alpha"], ["alpha", { root: "R-1", archive: true }]]);
    expect(client.getNode.mock.calls).toEqual([["alpha", "R-1"], ["alpha", "R-1", { with: ["links"] }]]);
    expect(client.search.mock.calls).toEqual([["alpha", { query: "a b" }]]);
    expect(client.getBundle.mock.calls).toEqual([["alpha", { node_ids: ["R-1"] }]]);
    expect(client.getGraph.mock.calls).toEqual([
      ["alpha", { ref: "R-1" }],
      ["alpha", { ref: "R-2", impact: true, types: ["t1"], depth: 0, archive: true }],
    ]);
  });
});

function Decider({ onReady }: { onReady: (decide: ReturnType<typeof useDecideProposal>["mutate"]) => void }) {
  const decide = useDecideProposal("alpha");
  onReady(decide.mutate);
  return null;
}

function seeded(): QueryClient {
  const queryClient = createQueryClient();
  queryClient.setQueryData(queryKeys.inbox("alpha"), { proposals: [aProposal({ id: "PR-1" })], notes: [] });
  queryClient.setQueryData(queryKeys.tree("alpha", { archive: true }), aTreeView([]));
  queryClient.setQueryData(queryKeys.node("alpha", "R-1"), { ref: "R-1", reason: null, notes: [], nodes: [] });
  queryClient.setQueryData(queryKeys.node("alpha", "R-1", { with: ["links"] }), { ref: "R-1", reason: null, notes: [], nodes: [] });
  queryClient.setQueryData(queryKeys.search("alpha", { query: "x" }), aSearchResults([]));
  queryClient.setQueryData(queryKeys.bundle("alpha", { node_ids: ["R-1"] }), aBundle(["R-1"]));
  queryClient.setQueryData(queryKeys.graph("alpha", { ref: "R-1", impact: true, types: ["t1"] }), aGraphView([], []));
  queryClient.setQueryData(queryKeys.tree("beta"), aTreeView([]));
  queryClient.setQueryData(queryKeys.graph("beta", { ref: "R-1" }), aGraphView([], []));
  return queryClient;
}

const AFTER_DECISION = [
  queryKeys.inbox("alpha"),
  queryKeys.tree("alpha", { archive: true }),
  queryKeys.node("alpha", "R-1"),
  queryKeys.node("alpha", "R-1", { with: ["links"] }),
  queryKeys.search("alpha", { query: "x" }),
  queryKeys.bundle("alpha", { node_ids: ["R-1"] }),
  queryKeys.graph("alpha", { ref: "R-1", impact: true, types: ["t1"] }),
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
    expect(queryClient.getQueryState(queryKeys.graph("beta", { ref: "R-1" }))?.isInvalidated).toBe(false);
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
