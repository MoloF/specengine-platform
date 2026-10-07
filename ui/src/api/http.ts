import {
  ClientError,
  type BundleOptions,
  type GraphOptions,
  type NodeOptions,
  type SearchOptions,
  type SpecEngineClient,
  type TreeOptions,
} from "./client";
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

// SpecEngineClient over the daemon, specengine-http (`docs/features/daemon-read.md` "Data", the
// graph and the check `docs/features/ui-live.md` "Data", the tasks
// `docs/features/ui-live-tasks.md` "Data"): `fetch` and `EventSource`, no package.
// Same origin: the dev server proxies `/api` to 127.0.0.1:7777 (vite.config.ts). A path segment
// (project, REF, proposal ID) is encoded once with encodeURIComponent (`#` → %23, `/` → %2F); a
// query repeats an array's key, an absent option is left out. Every answer is the CLI's document
// as sent: nothing is reworded or recomputed here.

/** Where the daemon's paths start, on this page's origin. */
const API = "/api";

/**
 * The queue's event types: the proposals' five
 * (`docs/canon/proposal-queue.md` "States and events"), then the tasks' nine
 * (`docs/canon/tasks.md` "Store"). An EventSource hands a named event only to a listener of that
 * name: a type missing here is never seen.
 */
export const QUEUE_EVENT_TYPES = [
  "proposal.created",
  "proposal.approved",
  "proposal.applied",
  "proposal.rejected",
  "proposal.apply_failed",
  "task.created",
  "task.planned",
  "task.approved",
  "task.changes_requested",
  "task.claimed",
  "task.run_reported",
  "task.completed",
  "task.cancelled",
  "task.refreshed",
] as const;

/** The first wait before a stream the browser gave up on is opened again; doubled per failure. */
export const REOPEN_FIRST_MS = 1_000;

/** The longest wait between two openings of a given-up stream. */
export const REOPEN_MAX_MS = 30_000;

/** EventSource's readyState once the browser has given up (a non-200 answer, another type). */
const CLOSED = 2;

type QueryValue = string | number | boolean | readonly string[] | undefined;

/** One path segment: percent-encoded once, `#` and `/` included. */
function segment(value: string): string {
  return encodeURIComponent(value);
}

/** `?name=value&…` in the given order: an array repeats its name, `undefined` is left out. */
function queryOf(pairs: readonly (readonly [string, QueryValue])[]): string {
  const parts: string[] = [];
  for (const [name, value] of pairs) {
    if (value === undefined) {
      continue;
    }
    const values = typeof value === "object" ? value : [String(value)];
    for (const one of values) {
      parts.push(`${encodeURIComponent(name)}=${encodeURIComponent(one)}`);
    }
  }
  return parts.length === 0 ? "" : `?${parts.join("&")}`;
}

/** The project's paths. */
function projectPath(project: string): string {
  return `/projects/${segment(project)}`;
}

const NOT_JSON = Symbol("not JSON");

function parsed(text: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return NOT_JSON;
  }
}

/** The daemon's error body: an object of exactly `status` (a number) and `message` (a string). */
export function errorBodyOf(value: unknown): ApiError | null {
  if (typeof value !== "object" || value === null || Array.isArray(value) || Object.keys(value).length !== 2) {
    return null;
  }
  if (!("status" in value) || !("message" in value)) {
    return null;
  }
  const { status, message } = value;
  return typeof status === "number" && typeof message === "string" ? { status, message } : null;
}

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** What a request sends besides its method and path: a POST's JSON body, a read's AbortSignal. */
interface Sent {
  body?: string;
  signal?: AbortSignal;
}

/**
 * One request. 2xx: its JSON document (the check's report whatever its verdict). A GET's 404 that
 * is not the error body: the exit-1 document, as data. Any other answer: a ClientError with the
 * HTTP status and the error body's `message` verbatim (another body: its text verbatim). No
 * answer: status 0. Aborted through `signal` (the read was superseded): the abort error as `fetch`
 * gave it, never a ClientError, since the daemon was not at fault.
 */
async function request(method: "GET" | "POST", path: string, { body, signal }: Sent = {}): Promise<unknown> {
  const url = `${API}${path}`;
  const asked = `${method} ${url}`;
  let response: Response;
  try {
    response = await fetch(url, {
      method,
      headers: body === undefined ? { Accept: "application/json" } : { Accept: "application/json", "Content-Type": "application/json" },
      body,
      cache: "no-store",
      ...(signal === undefined ? {} : { signal }),
    });
  } catch (error) {
    if (signal?.aborted === true) {
      throw error;
    }
    throw new ClientError({ status: 0, message: `${asked}: no response from the daemon (${reasonOf(error)})` });
  }
  let text: string;
  try {
    text = await response.text();
  } catch (error) {
    if (signal?.aborted === true) {
      throw error;
    }
    throw new ClientError({ status: 0, message: `${asked}: the answer broke off (${reasonOf(error)})` });
  }
  const value = parsed(text);
  if (response.ok) {
    if (value === NOT_JSON) {
      throw new ClientError({ status: response.status, message: `${asked} answered ${String(response.status)} without a JSON document: ${text}` });
    }
    return value;
  }
  const refusal = errorBodyOf(value);
  if (refusal !== null) {
    throw new ClientError({ status: response.status, message: refusal.message });
  }
  if (method === "GET" && response.status === 404 && value !== NOT_JSON) {
    return value;
  }
  throw new ClientError({ status: response.status, message: text === "" ? emptyAnswer(asked, response) : text });
}

/** An error answer with no body: said as such; the dev server's proxy answers 502 when no daemon listens. */
function emptyAnswer(asked: string, response: Response): string {
  const said = `${asked} answered ${String(response.status)} ${response.statusText} with an empty body`;
  return response.status === 502 ? `${said}: the dev server's proxy reached no daemon; is specengine-http running?` : said;
}

/** A named SSE message as a queue event: `id` the seq, the event's name the type, `data` parsed. */
function queueEventOf(message: MessageEvent<unknown>): QueueEvent {
  const data = typeof message.data === "string" ? message.data : "";
  const payload = parsed(data);
  return { seq: Number(message.lastEventId), type: message.type, payload: payload === NOT_JSON ? data : payload };
}

/** The daemon behind the dev server's `/api` proxy (`docs/features/daemon-read.md` "Data"). */
export class HttpClient implements SpecEngineClient {
  readonly dataSource = "daemon";

  async getProjects(signal?: AbortSignal): Promise<Project[]> {
    return (await request("GET", "/projects", { signal })) as Project[];
  }

  async getInbox(project: string, signal?: AbortSignal): Promise<Inbox> {
    return (await request("GET", `${projectPath(project)}/inbox`, { signal })) as Inbox;
  }

  async getProposal(project: string, id: string, signal?: AbortSignal): Promise<Proposal> {
    return (await request("GET", `${projectPath(project)}/proposals/${segment(id)}`, { signal })) as Proposal;
  }

  async getTree(project: string, options: TreeOptions = {}, signal?: AbortSignal): Promise<TreeView> {
    const query = queryOf([
      ["root", options.root],
      ["depth", options.depth],
      ["kinds", options.kinds],
      ["archive", options.archive],
    ]);
    return (await request("GET", `${projectPath(project)}/tree${query}`, { signal })) as TreeView;
  }

  async getNode(project: string, ref: string, options: NodeOptions = {}, signal?: AbortSignal): Promise<NodeView> {
    const query = queryOf([
      ["with", options.with],
      ["archive", options.archive],
    ]);
    return (await request("GET", `${projectPath(project)}/nodes/${segment(ref)}${query}`, { signal })) as NodeView;
  }

  async search(project: string, options: SearchOptions, signal?: AbortSignal): Promise<SearchResults> {
    const query = queryOf([
      ["query", options.query],
      ["kinds", options.kinds],
      ["limit", options.limit],
      ["archive", options.archive],
    ]);
    return (await request("GET", `${projectPath(project)}/search${query}`, { signal })) as SearchResults;
  }

  async getBundle(project: string, options: BundleOptions, signal?: AbortSignal): Promise<BundleView> {
    const query = queryOf([
      ["node_ids", options.node_ids],
      ["budget", options.budget],
    ]);
    return (await request("GET", `${projectPath(project)}/bundle${query}`, { signal })) as BundleView;
  }

  /**
   * `spec graph REF --json` as the browser view, uncut: the five options under their JSON echo
   * names (`docs/features/ui-graph.md` "Data"), the REF a query value (`#` as %23), `types`
   * repeated in order, `impact` and `archive` sent only as true, an absent option left out. A 404
   * is the exit-1 document (an unknown REF), resolved as data.
   */
  async getGraph(project: string, options: GraphOptions, signal?: AbortSignal): Promise<GraphView> {
    const query = queryOf([
      ["ref", options.ref],
      ["impact", options.impact === true ? true : undefined],
      ["types", options.types],
      ["depth", options.depth],
      ["archive", options.archive === true ? true : undefined],
    ]);
    return (await request("GET", `${projectPath(project)}/graph${query}`, { signal })) as GraphView;
  }

  /**
   * `spec task list --json`, the whole list: no `status` is sent, the screen filters the answer
   * (`docs/features/ui-live-tasks.md` "Data").
   */
  async getTasks(project: string, signal?: AbortSignal): Promise<TaskList> {
    return (await request("GET", `${projectPath(project)}/tasks`, { signal })) as TaskList;
  }

  /**
   * `spec task show T --json`, uncut, no query; the ID a path segment encoded once. A 404 is the
   * exit-1 document `{id, reason}` (no such task, or not a task ID: `id` null), resolved as data.
   */
  async getTask(project: string, id: string, signal?: AbortSignal): Promise<TaskPackage | TaskNotFound> {
    return (await request("GET", `${projectPath(project)}/tasks/${segment(id)}`, { signal })) as TaskPackage | TaskNotFound;
  }

  /**
   * `spec check --json` for the project, the plain run, no query: every verdict is a 200 report,
   * data (a check outcome, never a hold on work: ADR-0012); a 503 rejects in the daemon's words. No
   * AbortSignal: a walk in flight is never aborted (`docs/features/ui-live.md` "Data").
   */
  async getCheck(project: string): Promise<CheckReport> {
    return (await request("GET", `${projectPath(project)}/check`)) as CheckReport;
  }

  /** Always refused by the daemon (403): its message names the terminal command. */
  async decideProposal(project: string, id: string, decision: Decision): Promise<DecisionResult> {
    return (await request("POST", `${projectPath(project)}/proposals/${segment(id)}/decision`, { body: JSON.stringify(decision) })) as DecisionResult;
  }

  /**
   * The project's live tail. A dropped connection the browser re-opens itself, sending the last
   * `id` as `Last-Event-ID`, so the daemon resumes after it: no event is missed, no gap is said. A
   * stream the browser gives up on (CLOSED: a non-200 answer, the daemon restarting behind the
   * proxy) is opened again here after 1 s, 2 s, 4 s … at most 30 s; a new EventSource cannot send
   * `Last-Event-ID`, so only such an opening calls `onGap`.
   */
  subscribe(project: string, onEvent: (event: QueueEvent) => void, onGap?: () => void): () => void {
    const url = `${API}${projectPath(project)}/events`;
    let source: EventSource | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let wait = REOPEN_FIRST_MS;
    /** The browser gave up on the stream since it last opened: this client opened a new one, events may be missed. */
    let broken = false;
    let stopped = false;

    const deliver = (message: MessageEvent<unknown>) => {
      if (!stopped) {
        onEvent(queueEventOf(message));
      }
    };

    const open = () => {
      const current = new EventSource(url);
      source = current;
      for (const type of QUEUE_EVENT_TYPES) {
        current.addEventListener(type, deliver);
      }
      current.onopen = () => {
        wait = REOPEN_FIRST_MS;
        if (broken && !stopped) {
          broken = false;
          onGap?.();
        }
      };
      current.onerror = () => {
        // CONNECTING: the browser reconnects itself with Last-Event-ID; the daemon resumes after it.
        if (stopped || current.readyState !== CLOSED) {
          return;
        }
        broken = true;
        current.close();
        timer = setTimeout(() => {
          timer = undefined;
          if (!stopped) {
            open();
          }
        }, wait);
        wait = Math.min(wait * 2, REOPEN_MAX_MS);
      };
    };

    open();
    return () => {
      stopped = true;
      clearTimeout(timer);
      source?.close();
    };
  }
}
