import type {
  ApiError,
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
  ConfirmBody,
  ConfirmRead,
  FlowView,
  GraphView,
  HealthReport,
  HubSync,
  Identity,
  Inbox,
  LockDiffView,
  NodeView,
  Project,
  Proposal,
  QueueEvent,
  RoadmapDocument,
  RoundAnswerBody,
  RoundAnswersOutcome,
  RoundSheet,
  SearchResults,
  StageChoice,
  TaskList,
  TaskNotFound,
  TaskPackage,
  Team,
  TraceDocument,
  TreeView,
  WorkspaceDocument,
} from "./types";

// Option types of the reads: the MCP tools' arguments, which are the daemon's query names
// (`docs/canon/mcp-read.md` "Tools"). An absent option is omitted from the request, an array
// repeats its key, `archive` is sent only as true.

/** `get_tree`: ROOT, `--depth N`, `--kind K`…, `--archive`. */
export interface TreeOptions {
  root?: string;
  depth?: number;
  kinds?: string[];
  archive?: boolean;
}

/**
 * `get_node`: `--links` and `--bindings`, each a repeated `with`; `--archive` with `links` only (the
 * daemon refuses `archive` without `links`). Source: `docs/canon/code-reads.md` "Twins".
 */
export interface NodeOptions {
  with?: ("links" | "bindings")[];
  archive?: boolean;
}

/** `search`: the query as typed, `--kind K`…, `--limit N`, `--archive`. */
export interface SearchOptions {
  query: string;
  kinds?: string[];
  limit?: number;
  archive?: boolean;
}

/** `get_context_bundle`: the REFs and `--budget N` (absent: the project's default). */
export interface BundleOptions {
  node_ids: string[];
  budget?: number;
}

/** `spec graph`: REF, `--impact`, `--type T`..., `--depth N`, `--archive`. */
export interface GraphOptions {
  ref: string;
  impact?: boolean;
  types?: string[];
  depth?: number;
  archive?: boolean;
}

/**
 * `spec flow`: ROOT, `--depth N`, `--stage S`, `--trace` (`docs/canon/spec-flow.md` "The read"). The
 * screen sends one of three forms: the top `depth=0`, a scope `root=R&depth=1`, a stage of a scope
 * `root=R&stage=S`; a whole-flow read, which fails at volume, is never made
 * (`docs/features/writer-studio.md` D2). `trace` is sent only as true, and the Flow always sends it
 * (`docs/features/trace-views.md` "Flow badges").
 */
export interface FlowOptions {
  root?: string;
  depth?: number;
  /** A served lane's stage, `-` for the steps with none. */
  stage?: string;
  trace?: boolean;
}

/** `spec round new`: `--limit N`, 1 to 200 (absent: the daemon's default, 20). */
export interface RoundOptions {
  limit?: number;
}

/**
 * `spec roadmap`: `--release R` or `--pool`, never both (the daemon refuses the pair, 400): one of
 * the two, the other key absent, so the pair cannot be written; or neither, the whole plan. The
 * Roadmap screen sends neither.
 */
export type RoadmapOptions = { release: string; pool?: never } | { pool: true; release?: never };

/**
 * `spec card list`: `--with-cancelled`, `--marks`, each sent only as true. The Board reads every card
 * of the project once and filters that answer, so no other flag is sent
 * (`docs/features/board-ui.md` "Board"), and always asks the marks, so a tile is never drawn without
 * one (`docs/features/trace-views.md` "The board tile").
 */
export interface CardsOptions {
  with_cancelled?: boolean;
  marks?: boolean;
  /** `--parent C`: the cards whose `parent` is exactly C. The Board never sends it: it finds a card's children in the one list it reads. */
  parent?: string;
}

/**
 * `spec trace [REF] [--stage S] [--summary]`: the node and what it holds; absent, every live document
 * and every section a card targets. `stage` keeps one lane's rows, `-` those with none; `summary`, sent
 * only as true, no row at all (`docs/features/trace-views.md` "Data").
 */
export interface TraceOptions {
  ref?: string;
  stage?: string;
  summary?: boolean;
}

/**
 * The one seam between the UI and SpecEngine (ADR-0033): methods named after the daemon's
 * endpoints (`docs/features/daemon-read.md` "Data", `docs/features/ui-live.md` "Data",
 * `docs/features/ui-live-tasks.md` "Data", `docs/features/health-left.md` "Data", the plan's
 * `crates/specengine-http/README.md` "Endpoints", the lock diff's `docs/features/lock-diff.md`
 * "Route", the flow's `docs/features/flow-canvas.md` "Data", the cards' `docs/features/board-ui.md`
 * "Routes", the identity's `docs/canon/hub.md` "Routes", the team's `docs/features/hub-team.md` "Data",
 * the confirm's `docs/features/hub-confirm.md` "Data", the sync's `docs/features/hub-git.md` "Data", the workspaces' `docs/features/workspaces.md` "Data"; 07 §3). Only the bootstrap, src/main.tsx, picks the implementation. A read answered
 * with exit 1 (404 for tree, nodes, bundle, a proposal, graph, flow, task, roadmap, lock-diff) resolves
 * to its document, `reason` (a proposal: the last of `notes`) set; the check answers every verdict as a 200 report, data too, and the health read
 * zero amendments as a 200 too; every other failure rejects with a ClientError carrying
 * the daemon's status and message verbatim (exit 2: 503; no response: 0; a read the daemon does not
 * serve yet, none today: `notServed`, 501, nothing requested). A decision is only staged here and
 * confirmed on a terminal by `spec approve|reject PR` (`docs/canon/decision-staging.md` "Daemon",
 * ADR-0035): the stage's two writes answer the review document; a write refused with the review
 * document (409 not open or changed since read, 404 unknown) rejects with its last note. On a hub
 * alone a login `[team]` entitles confirms one staged proposal at a time, the POST taking the read's
 * question and no choice. The
 * question round (`docs/features/ui-round.md` "Data") reads the sheet and stages its answers in one
 * POST, answered 200 whatever each answer became; `spec round answer --staged` confirms them on a
 * terminal. Tasks are read only: an owner action is a command for a terminal (docs/features/ui-tasks.md).
 * The plan is read only too: an item moves when its file's front-matter is edited
 * (`docs/canon/spec-roadmap.md` "Items"). Cards are the daemon's only writes beyond staging
 * (ADR-0056): a queue row, never a spec file; 200 the write document, a refusal (409) its last note,
 * no such card (404) its `reason`, an error body (400, 403, 413, 415, 503) its message.
 *
 * `signal`: the query's AbortSignal. A read superseded before it answered (a burst of live events
 * reading the same task again) is aborted, so the daemon, which drops a request whose client left,
 * never runs it; the mock ignores it. The check takes none: a walk in flight is kept and its
 * answer taken, never aborted for a second one (`docs/features/ui-live.md` "Data"); nor does the
 * health read, a walk too (`docs/features/health-left.md` "Data"), nor the stage's writes: a
 * write's answer is always taken.
 */
export interface SpecEngineClient {
  /** Drives the permanent "Mock data" indicator. */
  readonly dataSource: "mock" | "daemon";
  /** GET /api/projects */
  getProjects(signal?: AbortSignal): Promise<Project[]>;
  /** GET /api/identity (docs/canon/hub.md "Routes"; a hub's login, null on a local daemon; no project) */
  getIdentity(signal?: AbortSignal): Promise<Identity>;
  /** GET /api/projects/:p/team (docs/features/hub-team.md "Data"; a hub alone: the asking login's row of `[team]`, a local daemon's unknown route) */
  getTeam(project: string, signal?: AbortSignal): Promise<Team>;
  /** GET /api/projects/:p/workspaces (docs/features/workspaces.md "Data"; = spec workspace --json, no query, no turn; judged: only a 200 resolves) */
  getWorkspaces(project: string, signal?: AbortSignal): Promise<WorkspaceDocument>;
  /** GET /api/projects/:p/inbox */
  getInbox(project: string, signal?: AbortSignal): Promise<Inbox>;
  /** GET /api/projects/:p/proposals/:id */
  getProposal(project: string, id: string, signal?: AbortSignal): Promise<Proposal>;
  /** GET /api/projects/:p/tree */
  getTree(project: string, options?: TreeOptions, signal?: AbortSignal): Promise<TreeView>;
  /** GET /api/projects/:p/nodes/:ref */
  getNode(project: string, ref: string, options?: NodeOptions, signal?: AbortSignal): Promise<NodeView>;
  /** GET /api/projects/:p/search */
  search(project: string, options: SearchOptions, signal?: AbortSignal): Promise<SearchResults>;
  /** GET /api/projects/:p/bundle */
  getBundle(project: string, options: BundleOptions, signal?: AbortSignal): Promise<BundleView>;
  /** GET /api/projects/:p/graph (crates/specengine-http/README.md "Endpoints"; the browser view, uncut) */
  getGraph(project: string, options: GraphOptions, signal?: AbortSignal): Promise<GraphView>;
  /** GET /api/projects/:p/flow (docs/canon/spec-flow.md "The read"; = spec flow [--trace] --json, the browser view, uncut; 404 the exit-1 document) */
  getFlow(project: string, options: FlowOptions, signal?: AbortSignal): Promise<FlowView>;
  /** GET /api/projects/:p/tasks (crates/specengine-http/README.md "Endpoints"; = spec task list --json) */
  getTasks(project: string, signal?: AbortSignal): Promise<TaskList>;
  /** GET /api/projects/:p/tasks/:id (crates/specengine-http/README.md "Endpoints"; = spec task show T --json, uncut; 404 the exit-1 document) */
  getTask(project: string, id: string, signal?: AbortSignal): Promise<TaskPackage | TaskNotFound>;
  /** GET /api/projects/:p/check (crates/specengine-http/README.md "Endpoints"; every verdict a 200 document) */
  getCheck(project: string): Promise<CheckReport>;
  /** GET /api/projects/:p/health (= spec health --json); no AbortSignal, as getCheck */
  getHealth(project: string): Promise<HealthReport>;
  /** GET /api/projects/:p/roadmap (crates/specengine-http/README.md "Endpoints"; = spec roadmap --json, the browser view, uncut; 404 the exit-1 document) */
  getRoadmap(project: string, options?: RoadmapOptions, signal?: AbortSignal): Promise<RoadmapDocument>;
  /** GET /api/projects/:p/lock-diff (docs/features/lock-diff.md "Route"; = spec lock diff REF --json, uncut; 404 the exit-1 document) */
  getLockDiff(project: string, ref: string, signal?: AbortSignal): Promise<LockDiffView>;
  // The stage's two writes: `stage` replaces any staged choice, the body exactly `stage` plus
  // `updated_at` as this page read it (changed since: 409); both answer the review document.
  /** POST /api/projects/:p/proposals/:id/decision (docs/canon/decision-staging.md "Daemon"; stages, nothing applied) */
  stageDecision(project: string, id: string, stage: StageChoice, updatedAt: string): Promise<Proposal>;
  /** DELETE /api/projects/:p/proposals/:id/decision (docs/canon/decision-staging.md "Daemon"; nothing staged: no event) */
  unstageDecision(project: string, id: string): Promise<Proposal>;
  /**
   * GET /api/projects/:p/proposals/:id/confirm, docs/features/hub-confirm.md "Data": a hub alone,
   * asked when a Confirm dialog opens, a local daemon's unknown route; judged as the POST is, so every
   * refusal, a 403, 404 or 409 among them, rejects and none resolves as data.
   */
  getConfirm(project: string, id: string, signal?: AbortSignal): Promise<ConfirmRead>;
  /**
   * POST /api/projects/:p/proposals/:id/confirm, docs/features/hub-confirm.md "Data": a hub alone,
   * one staged proposal per call, no route takes a set; the body exactly the read's `question` and
   * `updated_at`, never a choice; 200 the review document; no AbortSignal.
   */
  confirmProposal(project: string, id: string, body: ConfirmBody): Promise<Proposal>;
  /**
   * GET /api/projects/:p/sync, docs/features/hub-git.md "Data": a hub with `--push` alone, asked once
   * a login is known, any other daemon's unknown route; judged, so only a 200 resolves, and a 404 or
   * any other answer rejects. No query, no turn: the push status of the project's repository.
   */
  getSync(project: string, signal?: AbortSignal): Promise<HubSync>;
  /** GET /api/projects/:p/round (docs/features/ui-round.md "Data"; = spec round new [--limit N] --json, uncut) */
  getRound(project: string, options?: RoundOptions, signal?: AbortSignal): Promise<RoundSheet>;
  /**
   * POST /api/projects/:p/round/answers (docs/features/ui-round.md "Data"; stages, nothing applied):
   * the body `{project, answers}`, each answer the decision route's body; 200 the answers document
   * whatever each answer became, a refused one in the daemon's words; no AbortSignal.
   */
  stageRoundAnswers(project: string, answers: RoundAnswerBody[]): Promise<RoundAnswersOutcome>;
  /** GET /api/projects/:p/cards (docs/features/board-ui.md "Routes"; = spec card list [--with-cancelled] [--marks] --json, the browser view) */
  getCards(project: string, options?: CardsOptions, signal?: AbortSignal): Promise<CardList>;
  /** GET /api/projects/:p/cards/:id (docs/features/board-ui.md "Routes"; = spec card show C --history --json, uncut; 404 the exit-1 document) */
  getCard(project: string, id: string, signal?: AbortSignal): Promise<Card | CardNotFound>;
  // The card writes (docs/features/board-ui.md "Routes"): no AbortSignal, a write's answer is always taken.
  /** POST /api/projects/:p/cards */
  createCard(project: string, body: CardCreateBody): Promise<CardWrite>;
  /** POST /api/projects/:p/cards/:id/move */
  moveCard(project: string, id: string, body: CardMoveBody): Promise<CardWrite>;
  /** POST /api/projects/:p/cards/:id/edit */
  editCard(project: string, id: string, body: CardEditBody): Promise<CardWrite>;
  /** POST /api/projects/:p/cards/:id/assign */
  assignCard(project: string, id: string, body: CardAssignBody): Promise<CardWrite>;
  /** POST /api/projects/:p/cards/:id/cancel */
  cancelCard(project: string, id: string, body: CardCancelBody): Promise<CardWrite>;
  /** POST /api/projects/:p/cards/:id/accept */
  acceptCard(project: string, id: string, body: CardAcceptBody): Promise<CardWrite>;
  /** POST /api/projects/:p/cards/:id/clear (docs/features/card-trace.md "Clearing"; the page's dialog is the consent) */
  clearCard(project: string, id: string, body: CardClearBody): Promise<CardWrite>;
  /** POST /api/projects/:p/trace/clear (docs/features/card-trace.md "Clearing"; no effect only, one event per card) */
  clearCardGroup(project: string, body: CardClearGroupBody): Promise<CardClearGroupWrite>;
  /** GET /api/projects/:p/trace (docs/features/card-trace.md; = spec trace [REF] [--stage S] [--summary] --json, the browser view; 404 the exit-1 document) */
  getTrace(project: string, options?: TraceOptions, signal?: AbortSignal): Promise<TraceDocument>;
  /**
   * GET /api/projects/:p/events (SSE): `onEvent` per queue event of the project until the returned
   * function is called. `onGap` when the client opens the stream again after the browser gave up on
   * it: events may have been missed, so what they would refresh is read again (a reconnect the
   * browser makes itself resumes with `Last-Event-ID`: no gap). The mock: a no-op.
   */
  subscribe(project: string, onEvent: (event: QueueEvent) => void, onGap?: () => void): () => void;
}

/**
 * HTTP 409: the daemon refused a stage or an unstage with the current review document: the
 * proposal is not open, an orphan's approve, or it changed since this page read it.
 */
export const STAGE_REFUSED = 409;

/** HTTP 404: no such proposal for a stage or an unstage. */
export const NO_SUCH_PROPOSAL = 404;

/** HTTP 409: the daemon refused a card write, a lost compare-and-set too (`docs/features/board-ui.md` "Routes"). */
export const CARD_REFUSED = 409;

/** HTTP 404: no such card for a card write. */
export const NO_SUCH_CARD = 404;

/** HTTP 400: a card write the daemon cannot take as asked: nothing asked, a person's name off its grammar (`docs/features/board-ui.md` "Routes"). */
export const CARD_USAGE = 400;

/**
 * HTTP 501 Not Implemented: the status of a read the daemon does not serve yet, refused by the
 * client with nothing requested (none today; kept for the next missing endpoint). Never 0, which
 * says no response came (the daemon is down).
 */
export const NOT_SERVED = 501;

/**
 * What a SpecEngineClient rejects with: the daemon's error body, its message verbatim. `notServed`
 * marks a read the daemon has no endpoint for yet (status NOT_SERVED): nothing was requested, so
 * the screen says the read is not built and offers no Retry; a daemon answering 501 itself is not.
 */
export class ClientError extends Error implements ApiError {
  readonly status: number;
  readonly notServed: boolean;

  constructor(body: ApiError, { notServed = false }: { notServed?: boolean } = {}) {
    super(body.message);
    this.name = "ClientError";
    this.status = body.status;
    this.notServed = notServed;
  }
}

/** True for a read the daemon does not serve yet: not built, as opposed to a daemon down or refusing. */
export function isNotServed(error: unknown): boolean {
  return error instanceof ClientError && error.notServed;
}

/** The status and the verbatim message of any rejection; status 0 when no response carried one. */
export function apiErrorOf(error: unknown): ApiError {
  if (error instanceof ClientError) {
    return { status: error.status, message: error.message };
  }
  if (error instanceof Error) {
    return { status: 0, message: error.message };
  }
  return { status: 0, message: String(error) };
}
