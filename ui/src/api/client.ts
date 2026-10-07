import type {
  ApiError,
  BundleView,
  CheckReport,
  GraphView,
  Inbox,
  NodeView,
  Project,
  Proposal,
  QueueEvent,
  SearchResults,
  StageChoice,
  TaskList,
  TaskNotFound,
  TaskPackage,
  TreeView,
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

/** `get_node`: `--links`, and with it `--archive` (the daemon refuses `archive` without `links`). */
export interface NodeOptions {
  with?: "links"[];
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
 * The one seam between the UI and SpecEngine (ADR-0033): methods named after the daemon's
 * endpoints (`docs/features/daemon-read.md` "Data", `docs/features/ui-live.md` "Data",
 * `docs/features/ui-live-tasks.md` "Data"; 07 §3). Only the bootstrap, src/main.tsx, picks the
 * implementation. A read answered with exit 1 (404 for tree, nodes, bundle, a proposal, graph,
 * task) resolves to its document, `reason` (a proposal: the last of `notes`) set; the check answers
 * every verdict as a 200 report, data too; every other failure rejects with a ClientError carrying
 * the daemon's status and message verbatim (exit 2: 503; no response: 0; a read the daemon does not
 * serve yet, none today: `notServed`, 501, nothing requested). A decision is only staged here and
 * confirmed on a terminal by `spec approve|reject PR` (`docs/canon/decision-staging.md` "Daemon",
 * ADR-0035): the stage's two writes answer the review document; a write refused with the review
 * document (409 not open or changed since read, 404 unknown) rejects with its last note. Tasks are
 * read only: an owner action is a command for a terminal (docs/features/ui-tasks.md).
 *
 * `signal`: the query's AbortSignal. A read superseded before it answered (a burst of live events
 * reading the same task again) is aborted, so the daemon, which drops a request whose client left,
 * never runs it; the mock ignores it. The check takes none: a walk in flight is kept and its
 * answer taken, never aborted for a second one (`docs/features/ui-live.md` "Data"); nor do the
 * stage's writes: a write's answer is always taken.
 */
export interface SpecEngineClient {
  /** Drives the permanent "Mock data" indicator. */
  readonly dataSource: "mock" | "daemon";
  /** GET /api/projects */
  getProjects(signal?: AbortSignal): Promise<Project[]>;
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
  /** GET /api/projects/:p/tasks (crates/specengine-http/README.md "Endpoints"; = spec task list --json) */
  getTasks(project: string, signal?: AbortSignal): Promise<TaskList>;
  /** GET /api/projects/:p/tasks/:id (crates/specengine-http/README.md "Endpoints"; = spec task show T --json, uncut; 404 the exit-1 document) */
  getTask(project: string, id: string, signal?: AbortSignal): Promise<TaskPackage | TaskNotFound>;
  /** GET /api/projects/:p/check (crates/specengine-http/README.md "Endpoints"; every verdict a 200 document) */
  getCheck(project: string): Promise<CheckReport>;
  // The stage's two writes: `stage` replaces any staged choice, the body exactly `stage` plus
  // `updated_at` as this page read it (changed since: 409); both answer the review document.
  /** POST /api/projects/:p/proposals/:id/decision (docs/canon/decision-staging.md "Daemon"; stages, nothing applied) */
  stageDecision(project: string, id: string, stage: StageChoice, updatedAt: string): Promise<Proposal>;
  /** DELETE /api/projects/:p/proposals/:id/decision (docs/canon/decision-staging.md "Daemon"; nothing staged: no event) */
  unstageDecision(project: string, id: string): Promise<Proposal>;
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
