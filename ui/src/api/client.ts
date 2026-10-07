import type {
  ApiError,
  BundleView,
  CheckReport,
  Decision,
  DecisionResult,
  GraphView,
  Inbox,
  NodeView,
  Project,
  Proposal,
  QueueEvent,
  SearchResults,
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
 * endpoints (`docs/features/daemon-read.md` "Data", `docs/features/ui-live.md` "Data"; 07 §3).
 * Only the bootstrap, src/main.tsx, picks the implementation. A read answered with exit 1 (404 for
 * tree, nodes, bundle, a proposal, graph, task) resolves to its document, `reason` (a proposal: the
 * last of `notes`) set; the check answers every verdict as a 200 report, data too; every other
 * failure rejects with a ClientError carrying the daemon's status and message verbatim (exit 2:
 * 503; no response: 0; a read the daemon does not serve yet, the tasks: `notServed`, 501, nothing
 * requested). Decisions are made on a terminal: the daemon refuses each one
 * (403) naming its `spec` command (docs/features/daemon-read.md, Q4). Tasks are read only: an owner
 * action is a command for a terminal (docs/features/ui-tasks.md).
 */
export interface SpecEngineClient {
  /** Drives the permanent "Mock data" indicator. */
  readonly dataSource: "mock" | "daemon";
  /** GET /api/projects */
  getProjects(): Promise<Project[]>;
  /** GET /api/projects/:p/inbox */
  getInbox(project: string): Promise<Inbox>;
  /** GET /api/projects/:p/proposals/:id */
  getProposal(project: string, id: string): Promise<Proposal>;
  /** GET /api/projects/:p/tree */
  getTree(project: string, options?: TreeOptions): Promise<TreeView>;
  /** GET /api/projects/:p/nodes/:ref */
  getNode(project: string, ref: string, options?: NodeOptions): Promise<NodeView>;
  /** GET /api/projects/:p/search */
  search(project: string, options: SearchOptions): Promise<SearchResults>;
  /** GET /api/projects/:p/bundle */
  getBundle(project: string, options: BundleOptions): Promise<BundleView>;
  /** GET /api/projects/:p/graph (crates/specengine-http/README.md "Endpoints"; the browser view, uncut) */
  getGraph(project: string, options: GraphOptions): Promise<GraphView>;
  /** MISSING ENDPOINT GET /api/projects/:p/tasks (07 section 3 lists it; = spec task list --json; rust-developer, daemon-read "Out of scope") */
  getTasks(project: string): Promise<TaskList>;
  /** MISSING ENDPOINT GET /api/projects/:p/tasks/:id (07 section 3 lacks it; = spec task show T --json, uncut) */
  getTask(project: string, id: string): Promise<TaskPackage | TaskNotFound>;
  /** GET /api/projects/:p/check (crates/specengine-http/README.md "Endpoints"; every verdict a 200 document) */
  getCheck(project: string): Promise<CheckReport>;
  /** POST /api/projects/:p/proposals/:id/decision */
  decideProposal(project: string, id: string, decision: Decision): Promise<DecisionResult>;
  /**
   * GET /api/projects/:p/events (SSE): `onEvent` per queue event of the project until the returned
   * function is called. `onGap` when the client opens the stream again after the browser gave up on
   * it: events may have been missed, so what they would refresh is read again (a reconnect the
   * browser makes itself resumes with `Last-Event-ID`: no gap). The mock: a no-op.
   */
  subscribe(project: string, onEvent: (event: QueueEvent) => void, onGap?: () => void): () => void;
}

/** HTTP 409: the proposal was decided elsewhere. */
export const DECIDED_ELSEWHERE = 409;

/**
 * HTTP 501 Not Implemented: the status of a read the daemon does not serve yet, refused by the
 * client with nothing requested. Never 0, which says no response came (the daemon is down).
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
