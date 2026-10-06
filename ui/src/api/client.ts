import type {
  ApiError,
  BundleView,
  Decision,
  DecisionResult,
  GraphView,
  Inbox,
  NodeView,
  Project,
  SearchResults,
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
 * endpoints (`docs/specs/specengine-platform/07-interfaces.md` "3. HTTP (daemon)"). Only the
 * bootstrap, src/main.tsx, picks the implementation. A read answered with exit 1 (404 for tree,
 * nodes, bundle, graph) resolves to its document, `reason` set; every other failure rejects with a
 * ClientError carrying the daemon's status and message verbatim (exit 2: 503).
 */
export interface SpecEngineClient {
  /** Drives the permanent "Mock data" indicator. */
  readonly dataSource: "mock" | "daemon";
  /** MISSING ENDPOINT GET /api/projects (named for rust-developer; docs/features/ui-shell.md). */
  getProjects(): Promise<Project[]>;
  /** GET /api/projects/:p/inbox */
  getInbox(project: string): Promise<Inbox>;
  /** GET /api/projects/:p/tree */
  getTree(project: string, options?: TreeOptions): Promise<TreeView>;
  /** GET /api/projects/:p/nodes/:ref */
  getNode(project: string, ref: string, options?: NodeOptions): Promise<NodeView>;
  /** GET /api/projects/:p/search */
  search(project: string, options: SearchOptions): Promise<SearchResults>;
  /** MISSING ENDPOINT GET /api/projects/:p/bundle (07 §3 lacks it; rust-developer, daemon-read) */
  getBundle(project: string, options: BundleOptions): Promise<BundleView>;
  /** MISSING ENDPOINT GET /api/projects/:p/graph (07 section 3 lists it; uncut; rust-developer, daemon-read "Out of scope") */
  getGraph(project: string, options: GraphOptions): Promise<GraphView>;
  /** POST /api/projects/:p/proposals/:id/decision */
  decideProposal(project: string, id: string, decision: Decision): Promise<DecisionResult>;
}

/** HTTP 409: the proposal was decided elsewhere. */
export const DECIDED_ELSEWHERE = 409;

/** What a SpecEngineClient rejects with: the daemon's error body, its message verbatim. */
export class ClientError extends Error implements ApiError {
  readonly status: number;

  constructor(body: ApiError) {
    super(body.message);
    this.name = "ClientError";
    this.status = body.status;
  }
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
