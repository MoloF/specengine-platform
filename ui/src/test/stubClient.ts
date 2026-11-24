import { vi } from "vitest";
import {
  ClientError,
  type BundleOptions,
  type CardsOptions,
  type CatalogOptions,
  type CommentsOptions,
  type DeliveryOptions,
  type FeedOptions,
  type FlowOptions,
  type GraphOptions,
  type LibraryOptions,
  type MapOptions,
  type MentionsOptions,
  type NodeOptions,
  type ReferencesOptions,
  type RoadmapOptions,
  type RoundOptions,
  type SearchOptions,
  type SpecEngineClient,
  type TraceOptions,
  type TreeOptions,
  type UploadProgress,
} from "../api/client";
import type {
  BlobStored,
  BundleView,
  Card,
  CardAcceptBody,
  CardAssignBody,
  CardCancelBody,
  CardClearBody,
  CardClearGroupBody,
  CardClearGroupWrite,
  CardCounts,
  CardCreateBody,
  CardEditBody,
  CardList,
  CardMoveBody,
  CardNotFound,
  CardRaiseBody,
  CardSplit,
  CardSplitBody,
  CardWrite,
  CheckReport,
  CommentAddBody,
  CommentDocument,
  CommentEditBody,
  CommentList,
  CommentRaiseBody,
  ConfirmBody,
  ConfirmRead,
  CatalogDocument,
  DeliveryDocument,
  FeedDocument,
  FlowView,
  GraphView,
  GuideIndex,
  GuidePage,
  HealthReport,
  HubSync,
  Identity,
  Inbox,
  InSpec,
  IntakeDocument,
  LockDiffView,
  LibraryDocument,
  MapDocument,
  MentionList,
  NodeView,
  Project,
  PromoteBody,
  Proposal,
  Reference,
  ReferenceAddBody,
  ReferenceList,
  RingsDocument,
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
  Team,
  TraceDocument,
  TreeView,
  WorkspaceDocument,
} from "../api/types";
import {
  aB3,
  aBlobStored,
  aBundle,
  aReference,
  aCardCounts,
  aCardList,
  aCheckReport,
  aComment,
  aCardWrite,
  aCatalog,
  aLibrary,
  aMap,
  aRings,
  aDelivery,
  aFeed,
  aFlowView,
  aGraphNode,
  aGraphView,
  aGuideIndex,
  aGuidePage,
  aHealthReport,
  aNode,
  aProposal,
  aRoadmap,
  aRoadmapGroup,
  aRoadmapItem,
  aRoundSheet,
  aSearchResults,
  anImplicitWorkspaces,
  aTrace,
  aTreeNode,
  aTreeView,
  entryOf,
  noReview,
} from "./builders";

/** When the stub stores a stage: one fixed time, so a test can name it. */
export const STAGED_AT = "2026-10-06T09:14:02Z";

/** STAGED_AT as the English page shows a stored time (`formatUtc`): the staged line and its announcement. */
export const STAGED_SHOWN = "2026-10-06 09:14 UTC";

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
  const state: { proposals: Proposal[]; notes: string[]; round: RoundSheet; comments: CommentDocument[]; references: Reference[]; blobs: Blob[]; inSpec: Record<string, InSpec[]> } = {
    proposals: [...proposals],
    notes,
    round: aRoundSheet([]),
    comments: [],
    references: [],
    blobs: [],
    inSpec: {},
  };
  /** The stub's reference by ID, or the daemon's 404. */
  const referenceOf = (id: string): Reference => {
    const found = state.references.find((reference) => reference.id === id);
    if (found === undefined) {
      throw new ClientError({ status: 404, message: `no reference ${id}` });
    }
    return found;
  };
  /** The stub's comment by ID, or the daemon's 404. */
  const commentOf = (id: string): CommentDocument => {
    const found = state.comments.find((comment) => comment.id === id);
    if (found === undefined) {
      throw new ClientError({ status: 404, message: `no comment ${id}` });
    }
    return found;
  };
  const client = {
    dataSource: "mock" as const,
    state,
    getProjects: vi.fn<(signal?: AbortSignal) => Promise<Project[]>>(() => Promise.resolve(PROJECTS.map((project) => ({ ...project })))),
    // A local daemon's answer (docs/canon/hub.md "Routes"); a test of a hub overrides it.
    getIdentity: vi.fn<(signal?: AbortSignal) => Promise<Identity>>(() => Promise.resolve({ login: null })),
    // A local daemon's answer: its unknown route (docs/features/hub-team.md A7); a test of a hub overrides it.
    getTeam: vi.fn<(project: string, signal?: AbortSignal) => Promise<Team>>(() => Promise.reject(new ClientError({ status: 404, message: "no route /api/projects/alpha/team: the routes are /api/projects and the web UI" }))),
    // A project with no `[[workspace]]` table: the implicit workspace, every section but Tasks (docs/features/workspaces.md D3); a test of a configured one overrides it.
    getWorkspaces: vi.fn<(project: string, signal?: AbortSignal) => Promise<WorkspaceDocument>>(() => Promise.resolve(anImplicitWorkspaces())),
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
        aGraphView([aGraphNode({ id: options.ref ?? "R-1", distance: 0 })], [], {
          ref: options.ref ?? null,
          impact: options.impact ?? false,
          both: options.both ?? false,
          kinds: options.kinds ?? null,
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
    // `as`, the acting name, is taken and not stored: a stage's event is the daemon's (docs/features/card-questions.md "Acceptance criteria", L4); a test reads it from the call.
    stageDecision: vi.fn<(project: string, id: string, stage: StageChoice, updatedAt: string, as?: string | null) => Promise<Proposal>>((_project, id, stage, updatedAt) => {
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
    // A local daemon's answers: its unknown route (docs/features/hub-confirm.md B8); a test of a hub overrides them.
    getConfirm: vi.fn<(project: string, id: string, signal?: AbortSignal) => Promise<ConfirmRead>>((project, id) =>
      Promise.reject(new ClientError({ status: 404, message: `no route /api/projects/${project}/proposals/${id}/confirm: the routes are /api/projects and the web UI` })),
    ),
    confirmProposal: vi.fn<(project: string, id: string, body: ConfirmBody) => Promise<Proposal>>((project, id) =>
      Promise.reject(new ClientError({ status: 404, message: `no route /api/projects/${project}/proposals/${id}/confirm: the routes are /api/projects and the web UI` })),
    ),
    // A local daemon's answer and a hub's without `--push`: the unknown route (docs/features/hub-git.md "Status and the screen"); a test of a pushing hub overrides it.
    getSync: vi.fn<(project: string, signal?: AbortSignal) => Promise<HubSync>>((project) =>
      Promise.reject(new ClientError({ status: 404, message: `no route /api/projects/${project}/sync: the routes are /api/projects and the web UI` })),
    ),
    // The plan: one release, current, holding one open item (docs/features/roadmap-screen.md).
    getRoadmap: vi.fn<(project: string, options?: RoadmapOptions, signal?: AbortSignal) => Promise<RoadmapDocument>>(() =>
      Promise.resolve(aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001", title: "First item" })], { current: true })])),
    ),
    // What was delivered (docs/features/delivery-analytics.md "Acceptance criteria", DA-11): no task, at the day asked; a test overriding what it answers.
    getDelivery: vi.fn<(project: string, options: DeliveryOptions, signal?: AbortSignal) => Promise<DeliveryDocument>>((_project, options) => Promise.resolve(aDelivery(options.asOf))),
    // A direction's table (docs/features/direction-catalog.md "Acceptance criteria", DC-9): no row, the kinds asked; a test overriding what it answers.
    getCatalog: vi.fn<(project: string, options: CatalogOptions, signal?: AbortSignal) => Promise<CatalogDocument>>((_project, options) => Promise.resolve(aCatalog([], { kinds: [...options.kinds], archive: options.archive }))),
    // The rings (docs/features/scope-rings.md "Acceptance criteria", SR4): no ring, no note; a test overriding what it answers.
    getRings: vi.fn<(project: string, signal?: AbortSignal) => Promise<RingsDocument>>(() => Promise.resolve(aRings([]))),
    // The feed with no `[auto]` table and no event (docs/features/auto-feed.md "Data").
    getFeed: vi.fn<(project: string, options: FeedOptions, signal?: AbortSignal) => Promise<FeedDocument>>(() => Promise.resolve(aFeed())),
    // The map (docs/features/corpus-map.md "Acceptance criteria", CM3): no node, the archive asked; a test overriding what it answers.
    getMap: vi.fn<(project: string, options: MapOptions, signal?: AbortSignal) => Promise<MapDocument>>((_project, options) => Promise.resolve(aMap([], [], { archive: options.archive }))),
    // The library (docs/canon/spec-library.md "The screen"): no row, the archive asked; a test overriding what it answers.
    getLibrary: vi.fn<(project: string, options: LibraryOptions, signal?: AbortSignal) => Promise<LibraryDocument>>((_project, options) => Promise.resolve(aLibrary([], { archive: options.archive }))),
    // SpecEngine's guide (docs/features/specengine-guide.md "The daemon"): no topic; a topic's page made from its name; a test overriding what they answer.
    getGuide: vi.fn<(signal?: AbortSignal) => Promise<GuideIndex>>(() => Promise.resolve(aGuideIndex([]))),
    getGuideTopic: vi.fn<(topic: string, signal?: AbortSignal) => Promise<GuidePage>>((topic) => Promise.resolve(aGuidePage(topic))),
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
    // The task counts (docs/features/department-overview.md "Acceptance criteria", DO10): no flow, agent tasks counted; a test overriding what it answers.
    getCardCounts: vi.fn<(project: string, signal?: AbortSignal) => Promise<CardCounts>>(() => Promise.resolve(aCardCounts())),
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
    // A raise on a card (docs/features/card-questions.md "Acceptance criteria", L3): stored as the next ID, no hit; a test overriding what it answers.
    raiseOnCard: vi.fn<(project: string, id: string, body: CardRaiseBody) => Promise<IntakeDocument>>(() =>
      Promise.resolve({ id: "PR-0101", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] }),
    ),
    // A split (docs/features/department-split.md "Acceptance criteria", U2): a plan with no subtask, a write making none; a test overriding what it answers.
    splitCard: vi.fn<(project: string, body: CardSplitBody) => Promise<CardSplit>>((_project, body) =>
      Promise.resolve({ dry_run: body.dry_run, parent: null, children: [], reviewer: body.reviewer, notes: [] }),
    ),
    clearCardGroup: vi.fn<(project: string, body: CardClearGroupBody) => Promise<CardClearGroupWrite>>((_project, body) => Promise.resolve({ group: body.group, cleared: [], notes: [] })),
    // Comments (docs/features/comments.md "HTTP") over `state.comments`: a list is every comment on
    // the target, oldest first, one page; mentions the login's (every one's without a login), newest
    // first. A write stores as the daemon does: the next CM-, a version compare-and-set (409), a
    // tombstone; an item raised is PR-0102. A test overriding what any answers.
    getComments: vi.fn<(project: string, options: CommentsOptions, signal?: AbortSignal) => Promise<CommentList>>((_project, options) => {
      const listed = state.comments.filter((comment) => comment.on.id === options.on);
      return Promise.resolve(structuredClone({ on: listed[0]?.on ?? { kind: "card", id: options.on }, total: listed.length, comments: listed, next_before: null, notes: [] }));
    }),
    getMentions: vi.fn<(project: string, options: MentionsOptions, signal?: AbortSignal) => Promise<MentionList>>((_project, options) => {
      const login = options.login ?? null;
      const listed = state.comments.filter((comment) => (login === null ? comment.mentions.length > 0 : comment.mentions.includes(login))).reverse();
      return Promise.resolve(structuredClone({ login, comments: listed, next_before: null, notes: [] }));
    }),
    addComment: vi.fn<(project: string, body: CommentAddBody) => Promise<CommentDocument>>((_project, body) => {
      const id = `CM-${String(state.comments.length + 12).padStart(4, "0")}`;
      const actor = body.as ?? "local";
      const made = aComment({ id, on: { kind: "card", id: body.on }, reply_to: body.reply_to, actor, author: { type: "human", role: null, model: null, run: null }, body: body.body, created_at: STAGED_AT, updated_at: STAGED_AT });
      state.comments.push(made);
      return Promise.resolve(structuredClone(made));
    }),
    editComment: vi.fn<(project: string, id: string, body: CommentEditBody) => Promise<CommentDocument>>((_project, id, body) => {
      const stored = commentOf(id);
      if (stored.version !== body.version) {
        return Promise.reject(new ClientError({ status: 409, message: `${id} is at version ${String(stored.version)}, not ${String(body.version)}: nothing changed` }));
      }
      Object.assign(stored, { body: body.body, version: stored.version + 1, updated_at: STAGED_AT });
      return Promise.resolve(structuredClone(stored));
    }),
    deleteComment: vi.fn<(project: string, id: string) => Promise<CommentDocument>>((_project, id) => {
      const stored = commentOf(id);
      Object.assign(stored, { body: null, mentions: [], deleted: { by: "local", at: STAGED_AT } });
      return Promise.resolve(structuredClone(stored));
    }),
    raiseFromComment: vi.fn<(project: string, id: string, body: CommentRaiseBody) => Promise<IntakeDocument>>((_project, id, body) => {
      commentOf(id).items.push({ id: "PR-0102", kind: body.kind, status: "open" });
      return Promise.resolve({ id: "PR-0102", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] });
    }),
    // References (docs/features/work-references.md "HTTP") over `state.references`: a list is every
    // reference on the target, oldest first, one page. An upload reports its whole body sent, keeps
    // the blob in `state.blobs` and answers an agent-ready PNG named by its order (`aB3` of its count);
    // an attach stores the next RF- under the name sent (`local` with none); a delete leaves the
    // tombstone. Files are read at `/api/projects/<p>/blobs/<b3>`, as the daemon's. A test overriding what any answers.
    getReferences: vi.fn<(project: string, options: ReferencesOptions, signal?: AbortSignal) => Promise<ReferenceList>>((_project, options) => {
      const listed = state.references.filter((reference) => reference.on.id === options.on);
      return Promise.resolve(structuredClone({ on: listed[0]?.on ?? { kind: "card", id: options.on }, total: listed.length, references: listed, next_before: null, notes: [], in_spec: state.inSpec[options.on] ?? [] }));
    }),
    putBlob: vi.fn<(project: string, blob: Blob, onProgress: UploadProgress, signal: AbortSignal) => Promise<BlobStored>>((_project, blob, onProgress) => {
      state.blobs.push(blob);
      onProgress(blob.size, blob.size);
      return Promise.resolve(aBlobStored({ b3: aB3(String(state.blobs.length)), size: blob.size }));
    }),
    addReference: vi.fn<(project: string, body: ReferenceAddBody) => Promise<Reference>>((_project, body) => {
      const id = `RF-${String(state.references.length + 20).padStart(4, "0")}`;
      const actor = body.as ?? "local";
      const author = { type: "human", role: null, model: null, run: null };
      const made =
        "url" in body
          ? aReference({ id, on: { kind: "card", id: body.on }, actor, author, kind: "link", url: body.url, title: body.title, matters: body.matters, name: null, b3: null, size: null, media: null, class: null, inline: null, meta: null, file: null, created_at: STAGED_AT })
          : aReference({ id, on: { kind: "card", id: body.on }, actor, author, b3: body.b3, agent_b3: body.agent_b3, name: body.name, title: body.title, matters: body.matters, created_at: STAGED_AT });
      state.references.push(made);
      return Promise.resolve(structuredClone(made));
    }),
    deleteReference: vi.fn<(project: string, id: string) => Promise<Reference>>((_project, id) => {
      const stored = referenceOf(id);
      Object.assign(stored, { title: null, matters: null, url: null, name: null, b3: null, agent_b3: null, meta: null, file: null, deleted: { by: "local", at: STAGED_AT } });
      return Promise.resolve(structuredClone(stored));
    }),
    // A promotion (docs/canon/work-references-promotion-surfaces.md "HTTP") stores the next PR- of kind `reference`
    // naming its reference, the entry its name and text, and answers its review document.
    promoteReference: vi.fn<(project: string, id: string, body: PromoteBody) => Promise<Proposal>>((_project, id, body) => {
      const stored = referenceOf(id);
      const made = aProposal({
        id: `PR-${String(state.proposals.length + 140).padStart(4, "0")}`,
        kind: "reference",
        target_id: body.to,
        target_ids: [body.to],
        new_text: body.matters === null ? (body.name ?? stored.name ?? "") : `${body.name ?? stored.name ?? ""} | ${body.matters}`,
        rationale: body.rationale,
        ref_id: id,
        notes: [`adds media/${body.name ?? stored.name ?? ""}: ${id}, ${stored.media ?? ""}, plain`],
      });
      state.proposals.push(made);
      return Promise.resolve(structuredClone(made));
    }),
    blobUrl: vi.fn<(project: string, b3: string) => string>((project, b3) => `/api/projects/${encodeURIComponent(project)}/blobs/${encodeURIComponent(b3)}`),
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
