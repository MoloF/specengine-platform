import { vi } from "vitest";
import type { BundleOptions, GraphOptions, NodeOptions, SearchOptions, SpecEngineClient, TreeOptions } from "../api/client";
import type {
  BundleView,
  CheckReport,
  Decision,
  DecisionResult,
  GraphView,
  Inbox,
  NodeView,
  Project,
  Proposal,
  SearchResults,
  TaskList,
  TaskNotFound,
  TaskPackage,
  TreeView,
} from "../api/types";
import { aBundle, aCheckReport, aGraphNode, aGraphView, aNode, aSearchResults, aTreeNode, aTreeView, entryOf, noReview } from "./builders";

/** The stub's live tail: none. */
const noLiveTail: SpecEngineClient["subscribe"] = () => () => undefined;

export const PROJECTS: Project[] = [
  { slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" },
  { slug: "beta", name: "Beta", root: "/work/beta", branch: null },
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
      (): Promise<Inbox> => Promise.resolve({ proposals: state.proposals.map((p) => entryOf(p)), notes: [...state.notes] }),
    ),
    getProposal: vi.fn((_project: string, id: string): Promise<Proposal> => {
      const found = state.proposals.find((proposal) => proposal.id === id);
      return Promise.resolve(found === undefined ? noReview(id) : structuredClone(found));
    }),
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
    getGraph: vi.fn<(project: string, options: GraphOptions) => Promise<GraphView>>((_project, options) =>
      Promise.resolve(
        aGraphView([aGraphNode({ id: options.ref, distance: 0 })], [], {
          ref: options.ref,
          impact: options.impact ?? false,
          depth: options.depth ?? null,
          archive: options.archive ?? false,
        }),
      ),
    ),
    getTasks: vi.fn<(project: string) => Promise<TaskList>>(() => Promise.resolve({ tasks: [], notes: [] })),
    getTask: vi.fn<(project: string, id: string) => Promise<TaskPackage | TaskNotFound>>((_project, id) =>
      Promise.resolve({ id, reason: `no task ${id} in this repository` }),
    ),
    getCheck: vi.fn<(project: string) => Promise<CheckReport>>(() => Promise.resolve(aCheckReport())),
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
    /** No live tail; a plain function (`callsOf` never counts it) a test may replace to emit events. */
    subscribe: noLiveTail,
  } satisfies SpecEngineClient & { state: unknown };
  return client;
}

export type StubClient = ReturnType<typeof stubClient>;
