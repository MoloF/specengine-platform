import { vi } from "vitest";
import type { BundleOptions, NodeOptions, SearchOptions, SpecEngineClient, TreeOptions } from "../api/client";
import type {
  BundleView,
  Decision,
  DecisionResult,
  Inbox,
  NodeView,
  Project,
  Proposal,
  SearchResults,
  TreeView,
} from "../api/types";
import { aBundle, aNode, aSearchResults, aTreeNode, aTreeView } from "./builders";

export const PROJECTS: Project[] = [
  { slug: "alpha", name: "Alpha" },
  { slug: "beta", name: "Beta" },
];

/** The stub's tree: a document holding R-1. */
export const STUB_TREE = [
  aTreeNode({ id: "DOC-1", depth: 0, path: "docs/spec/doc-1.md" }),
  aTreeNode({ id: "R-1", depth: 1, parent: "DOC-1", path: "docs/spec/doc-1.md", line: 5 }),
];

/**
 * A hand-made SpecEngineClient for tests: every method a vi.fn counting its calls, reading an
 * editable queue. Override any method with mockImplementation.
 */
export function stubClient(proposals: Proposal[] = [], notes: string[] = []) {
  const state = { proposals: [...proposals], notes };
  const client = {
    dataSource: "mock" as const,
    state,
    getProjects: vi.fn((): Promise<Project[]> => Promise.resolve(PROJECTS.map((project) => ({ ...project })))),
    getInbox: vi.fn(
      (): Promise<Inbox> => Promise.resolve({ proposals: state.proposals.map((p) => ({ ...p })), notes: [...state.notes] }),
    ),
    getTree: vi.fn<(project: string, options?: TreeOptions) => Promise<TreeView>>(() =>
      Promise.resolve(aTreeView(STUB_TREE.map((row) => ({ ...row })))),
    ),
    getNode: vi.fn<(project: string, ref: string, options?: NodeOptions) => Promise<NodeView>>((_project, id) =>
      Promise.resolve({ ref: id, reason: null, notes: [], nodes: [aNode({ id })] }),
    ),
    search: vi.fn<(project: string, options: SearchOptions) => Promise<SearchResults>>((_project, options) =>
      Promise.resolve(aSearchResults([], { query: options.query, archive: options.archive ?? false })),
    ),
    getBundle: vi.fn<(project: string, options: BundleOptions) => Promise<BundleView>>((_project, options) =>
      Promise.resolve(aBundle(options.node_ids, { budget: options.budget ?? 2000 })),
    ),
    decideProposal: vi.fn((_project: string, id: string, decision: Decision): Promise<DecisionResult> => {
      const current = state.proposals.find((proposal) => proposal.id === id);
      if (current === undefined) {
        return Promise.reject(new Error(`no proposal ${id}`));
      }
      if (decision.decision === "accept") {
        state.proposals = state.proposals.filter((proposal) => proposal.id !== id);
        return Promise.resolve({
          proposal: { ...current, status: "applied", applied_commit: "c0ffee1" },
          commit: { sha: "c0ffee1", subject: `spec: apply ${id}` },
        });
      }
      if (decision.decision === "reject") {
        state.proposals = state.proposals.filter((proposal) => proposal.id !== id);
        return Promise.resolve({ proposal: { ...current, status: "rejected" }, commit: null });
      }
      const status = decision.decision === "defer" ? "deferred" : "changes_requested";
      const updated = { ...current, status };
      state.proposals = state.proposals.map((proposal) => (proposal.id === id ? updated : proposal));
      return Promise.resolve({ proposal: updated, commit: null });
    }),
  } satisfies SpecEngineClient & { state: unknown };
  return client;
}

export type StubClient = ReturnType<typeof stubClient>;
