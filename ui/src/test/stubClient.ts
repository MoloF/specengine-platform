import { vi } from "vitest";
import {
  ClientError,
  type BundleOptions,
  type CardsOptions,
  type FlowOptions,
  type GraphOptions,
  type NodeOptions,
  type RoadmapOptions,
  type RoundOptions,
  type SearchOptions,
  type SpecEngineClient,
  type TraceOptions,
  type TreeOptions,
} from "../api/client";
import type {
  BundleView,
  Card,
  CardAcceptBody,
  CardAssignBody,
  CardCancelBody,
  CardClearBody,
  CardClearGroupBody,
  CardClearGroupWrite,
  CardCreateBody,
  CardEditBody,
  CardList,
  CardMoveBody,
  CardNotFound,
  CardWrite,
  CheckReport,
  FlowView,
  GraphView,
  HealthReport,
  Identity,
  Inbox,
  LockDiffView,
  NodeView,
  Project,
  Proposal,
  RoadmapDocument,
  RoundAnswerBody,
  RoundAnswersOutcome,
  RoundSheet,
  SearchResults,
  Stage,
  StageChoice,
  TaskList,
  TaskNotFound,
  TaskPackage,
  TraceDocument,
  TreeView,
} from "../api/types";
import {
  aBundle,
  aCardList,
  aCheckReport,
  aCardWrite,
  aFlowView,
  aGraphNode,
  aGraphView,
  aHealthReport,
  aNode,
  aRoadmap,
  aRoadmapGroup,
  aRoadmapItem,
  aRoundSheet,
  aSearchResults,
  aTrace,
  aTreeNode,
  aTreeView,
  entryOf,
  noReview,
} from "./builders";

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

/** The stub's stored stage of a stage body: less `updated_at`, an approve's `span_hash` null. */
function storedOf(stage: RoundAnswerBody["stage"]): Stage {
  return stage.decision === "approve"
    ? { decision: "approve", option: stage.option, answer: stage.answer, canon: stage.canon, editorial: stage.editorial, note: stage.note, span_hash: null }
    : { decision: "reject", reason: stage.reason };
}

/**
 * A hand-made SpecEngineClient for tests: every method a vi.fn counting its calls, reading an
 * editable queue (and an editable round sheet, empty by default). Override any method with
 * mockImplementation.
 */
export function stubClient(proposals: Proposal[] = [], notes: string[] = []) {
  const state: { proposals: Proposal[]; notes: string[]; round: RoundSheet } = { proposals: [...proposals], notes, round: aRoundSheet([]) };
  const client = {
    dataSource: "mock" as const,
    state,
    getProjects: vi.fn<(signal?: AbortSignal) => Promise<Project[]>>(() => Promise.resolve(PROJECTS.map((project) => ({ ...project })))),
    // A local daemon's answer (docs/canon/hub.md "Routes"); a test of a hub overrides it.
    getIdentity: vi.fn<(signal?: AbortSignal) => Promise<Identity>>(() => Promise.resolve({ login: null })),
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
    getFlow: vi.fn<(project: string, options: FlowOptions, signal?: AbortSignal) => Promise<FlowView>>((_project, options) =>
      Promise.resolve(aFlowView([], [], { root: options.root ?? null, depth: options.depth ?? null })),
    ),
    getTasks: vi.fn<(project: string, signal?: AbortSignal) => Promise<TaskList>>(() => Promise.resolve({ tasks: [], notes: [] })),
    getTask: vi.fn<(project: string, id: string, signal?: AbortSignal) => Promise<TaskPackage | TaskNotFound>>((_project, id) =>
      Promise.resolve({ id, reason: `no task ${id} in this repository` }),
    ),
    getCheck: vi.fn<(project: string) => Promise<CheckReport>>(() => Promise.resolve(aCheckReport())),
    // The unapplied amendments: none over aCheckReport's 7 documents (docs/features/health-left.md "Data").
    getHealth: vi.fn<(project: string) => Promise<HealthReport>>(() => Promise.resolve(aHealthReport())),
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
    // The plan: one release, current, holding one open item (docs/features/roadmap-screen.md).
    getRoadmap: vi.fn<(project: string, options?: RoadmapOptions, signal?: AbortSignal) => Promise<RoadmapDocument>>(() =>
      Promise.resolve(aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001", title: "First item" })], { current: true })])),
    ),
    // The lock diff (docs/features/lock-diff.md "Data"): the REF's one holder, no drifted row with a lock entry.
    getLockDiff: vi.fn<(project: string, ref: string, signal?: AbortSignal) => Promise<LockDiffView>>((_project, ref) =>
      Promise.resolve({ ref, reason: null, notes: [], nodes: [{ id: ref, path: "docs/spec/doc-1.md", line: 5, rows: [] }] }),
    ),
    getRound: vi.fn<(project: string, options?: RoundOptions, signal?: AbortSignal) => Promise<RoundSheet>>(() =>
      Promise.resolve(structuredClone(state.round)),
    ),
    // The daemon's round POST on the stub's sheet: an answer on a listed item at the `updated_at`
    // read is staged at STAGED_AT, any other refused as changed since read.
    stageRoundAnswers: vi.fn((project: string, answers: RoundAnswerBody[]): Promise<RoundAnswersOutcome> => {
      const outcomes = answers.map(({ id, stage }) => {
        const item = state.round.items.find((candidate) => candidate.id === id);
        if (item === undefined || item.updated_at !== stage.updated_at) {
          return { id, outcome: "refused", stage: null, staged_at: null, replaced: null, reason: `\`${id}\` changed since it was read` };
        }
        const staged = storedOf(stage);
        const replaced = item.staged_at;
        state.round = {
          ...state.round,
          items: state.round.items.map((candidate) => (candidate.id === id ? { ...candidate, staged, staged_at: STAGED_AT, updated_at: STAGED_AT } : candidate)),
        };
        return { id, outcome: "staged", stage: staged, staged_at: STAGED_AT, replaced, reason: null };
      });
      return Promise.resolve({ project, answers: outcomes, notes: [] });
    }),
    // The cards (docs/features/board-ui.md "Routes"): none; each write answers its card's document, a test overriding what it stores.
    getCards: vi.fn<(project: string, options?: CardsOptions, signal?: AbortSignal) => Promise<CardList>>(() => Promise.resolve(aCardList([]))),
    getCard: vi.fn<(project: string, id: string, signal?: AbortSignal) => Promise<Card | CardNotFound>>((_project, id) =>
      Promise.resolve({ id, reason: `no card ${id}` }),
    ),
    createCard: vi.fn<(project: string, body: CardCreateBody) => Promise<CardWrite>>(() => Promise.resolve(aCardWrite("C-0001"))),
    moveCard: vi.fn<(project: string, id: string, body: CardMoveBody) => Promise<CardWrite>>((_project, id) => Promise.resolve(aCardWrite(id))),
    editCard: vi.fn<(project: string, id: string, body: CardEditBody) => Promise<CardWrite>>((_project, id) => Promise.resolve(aCardWrite(id))),
    assignCard: vi.fn<(project: string, id: string, body: CardAssignBody) => Promise<CardWrite>>((_project, id) => Promise.resolve(aCardWrite(id))),
    cancelCard: vi.fn<(project: string, id: string, body: CardCancelBody) => Promise<CardWrite>>((_project, id) => Promise.resolve(aCardWrite(id))),
    acceptCard: vi.fn<(project: string, id: string, body: CardAcceptBody) => Promise<CardWrite>>((_project, id) => Promise.resolve(aCardWrite(id))),
    clearCard: vi.fn<(project: string, id: string, body: CardClearBody) => Promise<CardWrite>>((_project, id) => Promise.resolve(aCardWrite(id))),
    clearCardGroup: vi.fn<(project: string, body: CardClearGroupBody) => Promise<CardClearGroupWrite>>((_project, body) => Promise.resolve({ group: body.group, cleared: [], notes: [] })),
    // Marks and rollups (docs/features/card-trace.md): none.
    getTrace: vi.fn<(project: string, options?: TraceOptions, signal?: AbortSignal) => Promise<TraceDocument>>(() => Promise.resolve(aTrace([]))),
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
