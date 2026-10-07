import { vi } from "vitest";
import { ClientError, type BundleOptions, type GraphOptions, type NodeOptions, type SearchOptions, type SpecEngineClient, type TreeOptions } from "../api/client";
import type {
  BundleView,
  CheckReport,
  GraphView,
  Inbox,
  NodeView,
  Project,
  Proposal,
  SearchResults,
  Stage,
  StageChoice,
  TaskList,
  TaskNotFound,
  TaskPackage,
  TreeView,
} from "../api/types";
import { aBundle, aCheckReport, aGraphNode, aGraphView, aNode, aSearchResults, aTreeNode, aTreeView, entryOf, noReview } from "./builders";

/** When the stub stores a stage: one fixed time, so a test can name it. */
export const STAGED_AT = "2026-10-06T09:14:02Z";

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
    getProjects: vi.fn<(signal?: AbortSignal) => Promise<Project[]>>(() => Promise.resolve(PROJECTS.map((project) => ({ ...project })))),
    getInbox: vi.fn<(project: string, signal?: AbortSignal) => Promise<Inbox>>(() =>
      Promise.resolve({ proposals: state.proposals.map((p) => entryOf(p)), notes: [...state.notes] }),
    ),
    getProposal: vi.fn<(project: string, id: string, signal?: AbortSignal) => Promise<Proposal>>((_project, id) => {
      const found = state.proposals.find((proposal) => proposal.id === id);
      return Promise.resolve(found === undefined ? noReview(id) : structuredClone(found));
    }),
    getTree: vi.fn<(project: string, options?: TreeOptions, signal?: AbortSignal) => Promise<TreeView>>(() =>
      Promise.resolve(aTreeView(STUB_TREE.map((row) => ({ ...row })))),
    ),
    getNode: vi.fn<(project: string, ref: string, options?: NodeOptions, signal?: AbortSignal) => Promise<NodeView>>((_project, id) =>
      Promise.resolve({ ref: id, reason: null, notes: [], nodes: [aNode({ id })] }),
    ),
    search: vi.fn<(project: string, options: SearchOptions, signal?: AbortSignal) => Promise<SearchResults>>((_project, options) =>
      Promise.resolve(aSearchResults([], { query: options.query, archive: options.archive ?? false })),
    ),
    getBundle: vi.fn<(project: string, options: BundleOptions, signal?: AbortSignal) => Promise<BundleView>>((_project, options) =>
      Promise.resolve(aBundle(options.node_ids, { budget: options.budget ?? 2000 })),
    ),
    getGraph: vi.fn<(project: string, options: GraphOptions, signal?: AbortSignal) => Promise<GraphView>>((_project, options) =>
      Promise.resolve(
        aGraphView([aGraphNode({ id: options.ref, distance: 0 })], [], {
          ref: options.ref,
          impact: options.impact ?? false,
          depth: options.depth ?? null,
          archive: options.archive ?? false,
        }),
      ),
    ),
    getTasks: vi.fn<(project: string, signal?: AbortSignal) => Promise<TaskList>>(() => Promise.resolve({ tasks: [], notes: [] })),
    getTask: vi.fn<(project: string, id: string, signal?: AbortSignal) => Promise<TaskPackage | TaskNotFound>>((_project, id) =>
      Promise.resolve({ id, reason: `no task ${id} in this repository` }),
    ),
    getCheck: vi.fn<(project: string) => Promise<CheckReport>>(() => Promise.resolve(aCheckReport())),
    // The daemon's stage on the stub's queue: an open proposal at the `updated_at` read, else 409;
    // stored with STAGED_AT, the proposal kept in the queue.
    stageDecision: vi.fn((_project: string, id: string, stage: StageChoice, updatedAt: string): Promise<Proposal> => {
      const current = state.proposals.find((proposal) => proposal.id === id);
      if (current === undefined) {
        return Promise.reject(new ClientError({ status: 404, message: `no proposal \`${id}\` in this project's queue` }));
      }
      if (current.status !== "open" || current.updated_at !== updatedAt) {
        return Promise.reject(new ClientError({ status: 409, message: `\`${id}\` is not open at ${updatedAt}; nothing changed` }));
      }
      const staged: Stage = stage.decision === "approve" ? { ...stage, span_hash: current.base_hash } : stage;
      const updated: Proposal = { ...current, staged, staged_at: STAGED_AT, updated_at: STAGED_AT };
      state.proposals = state.proposals.map((proposal) => (proposal.id === id ? updated : proposal));
      return Promise.resolve(structuredClone(updated));
    }),
    unstageDecision: vi.fn((_project: string, id: string): Promise<Proposal> => {
      const current = state.proposals.find((proposal) => proposal.id === id);
      if (current === undefined) {
        return Promise.reject(new ClientError({ status: 404, message: `no proposal \`${id}\` in this project's queue` }));
      }
      const updated: Proposal = current.staged === null ? current : { ...current, staged: null, staged_at: null, updated_at: "2026-10-06T09:15:00Z" };
      state.proposals = state.proposals.map((proposal) => (proposal.id === id ? updated : proposal));
      return Promise.resolve(structuredClone(updated));
    }),
    /** No live tail; a plain function (`callsOf` never counts it) a test may replace to emit events. */
    subscribe: noLiveTail,
  } satisfies SpecEngineClient & { state: unknown };
  return client;
}

export type StubClient = ReturnType<typeof stubClient>;

/**
 * A stub method's calls as the app made them, less what every query adds: its AbortSignal last,
 * and the `undefined` standing for absent options before it.
 */
export function argsOf(method: { mock: { calls: readonly (readonly unknown[])[] } }): unknown[][] {
  return method.mock.calls.map((call) => {
    const args = [...call];
    if (args.at(-1) instanceof AbortSignal) {
      args.pop();
    }
    while (args.length > 0 && args.at(-1) === undefined) {
      args.pop();
    }
    return args;
  });
}
