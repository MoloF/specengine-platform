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
} from "./client";
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
  CardRaiseBody,
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
  IntakeDocument,
  LockDiffView,
  NodeView,
  Project,
  Proposal,
  QueueEvent,
  RoadmapDocument,
  RoundAnswerBody,
  RoundAnswersOutcome,
  RoundAnswersBody,
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

// SpecEngineClient over the daemon, specengine-http (`docs/features/daemon-read.md` "Data", the
// graph and the check `docs/features/ui-live.md` "Data", the tasks
// `docs/features/ui-live-tasks.md` "Data", the stage `docs/canon/decision-staging.md` "Daemon",
// the question round `docs/features/ui-round.md` "Data", the unapplied amendments
// `docs/features/health-left.md` "Data", the plan `crates/specengine-http/README.md` "Endpoints",
// the lock diff `docs/features/lock-diff.md` "Route", the flow `docs/features/flow-canvas.md`
// "Data", the cards `docs/features/board-ui.md` "Routes" (a card's raise `docs/canon/card-questions.md`
// "The rule"), the identity `docs/canon/hub.md` "Routes",
// the confirm `docs/features/hub-confirm.md` "Data", the sync `docs/features/hub-git.md` "Data",
// the workspaces `docs/features/workspaces.md` "Data"):
// `fetch` and `EventSource`, no package.
// Same origin: the dev server proxies `/api` to 127.0.0.1:7777 (vite.config.ts). A path segment
// (project, REF, proposal ID) is encoded once with encodeURIComponent (`#` → %23, `/` → %2F); a
// query repeats an array's key, an absent option is left out. Every answer is the CLI's document
// as sent: nothing is reworded or recomputed here.

/** Where the daemon's paths start, on this page's origin. */
const API = "/api";

/**
 * The queue's event types: the proposals' seven
 * (`docs/canon/proposal-queue.md` "States and events"; the stage's two after `apply_failed`:
 * `docs/canon/decision-staging.md` "Queue"), the tasks' nine (`docs/canon/tasks.md` "Store"), then
 * the cards' nine (`docs/canon/cards.md` "Documents and events", `docs/features/card-trace.md`
 * "Data"). An EventSource hands a named event only to a listener of that name: a type missing here is
 * never seen.
 */
export const QUEUE_EVENT_TYPES = [
  "proposal.created",
  "proposal.approved",
  "proposal.applied",
  "proposal.rejected",
  "proposal.apply_failed",
  "proposal.staged",
  "proposal.unstaged",
  "task.created",
  "task.planned",
  "task.approved",
  "task.changes_requested",
  "task.claimed",
  "task.run_reported",
  "task.completed",
  "task.cancelled",
  "task.refreshed",
  "card.created",
  "card.moved",
  "card.accepted",
  "card.edited",
  "card.assigned",
  "card.cancelled",
  "card.cleared",
  "card.reestimated",
  "card.refreshed",
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

/** A proposal's decision path: the stage's POST and DELETE. */
function decisionPath(project: string, id: string): string {
  return `${projectPath(project)}/proposals/${segment(id)}/decision`;
}

/** A proposal's confirm path: a hub's read and its POST. */
function confirmPath(project: string, id: string): string {
  return `${projectPath(project)}/proposals/${segment(id)}/confirm`;
}

/** The round's sheet; its answers' POST is under it. */
function roundPath(project: string): string {
  return `${projectPath(project)}/round`;
}

/** The project's cards; a card's five writes are under `cardPath`. */
function cardsPath(project: string): string {
  return `${projectPath(project)}/cards`;
}

/** One card's path. */
function cardPath(project: string, id: string): string {
  return `${cardsPath(project)}/${segment(id)}`;
}

/** A project's trace; the group clear's POST is under it. */
function tracePath(project: string): string {
  return `${projectPath(project)}/trace`;
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

/**
 * The reason a refused write's document gives: the last of its `notes` (the 409 of a stage on a
 * proposal not open or changed since read, `docs/canon/decision-staging.md` "Daemon"; a card's write
 * document, `docs/features/board-ui.md` "Routes"), else its `reason` (no such card's `{id, reason}`);
 * null for any other body.
 */
export function refusedReasonOf(value: unknown): string | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  if ("reason" in value && !("notes" in value)) {
    return typeof value.reason === "string" && value.reason !== "" ? value.reason : null;
  }
  const notes: unknown = "notes" in value ? value.notes : null;
  if (!Array.isArray(notes)) {
    return null;
  }
  const last: unknown = notes.at(-1);
  return typeof last === "string" && last !== "" ? last : null;
}

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * What a request sends besides its method and path: a POST's JSON body, a read's AbortSignal;
 * `judged`, a read answered as a write is: its refusal a ClientError, a document's last note too.
 */
interface Sent {
  body?: string;
  signal?: AbortSignal;
  judged?: boolean;
}

/** A read, or one of the stage's two writes. */
type Method = "GET" | "POST" | "DELETE";

/**
 * One request. 2xx: its JSON document (the check's report whatever its verdict). A GET's 404 that
 * is not the error body: the exit-1 document, as data. A write's refusal carrying a review
 * document (a stage's 409 or 404): a ClientError with the status and the document's last note.
 * Any other answer: a ClientError with the HTTP status and the error body's `message` verbatim
 * (another body: its text verbatim). No answer: status 0. Aborted through `signal` (the read was
 * superseded): the abort error as `fetch` gave it, never a ClientError, since the daemon was not
 * at fault. A `judged` read's refusal goes as a write's: nothing of it resolves as data.
 */
async function request(method: Method, path: string, { body, signal, judged = false }: Sent = {}): Promise<unknown> {
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
  const read = method === "GET" && !judged;
  if (read && response.status === 404 && value !== NOT_JSON) {
    return value;
  }
  const refused = read ? null : refusedReasonOf(value);
  if (refused !== null) {
    throw new ClientError({ status: response.status, message: refused });
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

  async getIdentity(signal?: AbortSignal): Promise<Identity> {
    return (await request("GET", "/identity", { signal })) as Identity;
  }

  /** A hub's route alone (`docs/features/hub-team.md` "Data"): asked once a login is known, a local daemon answering its unknown-route 404. */
  async getTeam(project: string, signal?: AbortSignal): Promise<Team> {
    return (await request("GET", `${projectPath(project)}/team`, { signal })) as Team;
  }

  /**
   * `spec workspace --json` (`docs/features/workspaces.md` "Data"): no query, which the daemon
   * refuses (400), and no turn. Judged, so only a 200 resolves: an older daemon's 404 and a 503 (a
   * config `discover` refuses) reject in the daemon's words, and the nav falls back to every section.
   */
  async getWorkspaces(project: string, signal?: AbortSignal): Promise<WorkspaceDocument> {
    return (await request("GET", `${projectPath(project)}/workspaces`, { signal, judged: true })) as WorkspaceDocument;
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
   * `spec flow [ROOT] [--depth N] [--stage S] [--trace] [--in-flow] --json` as the browser view, uncut
   * (`docs/canon/spec-flow-cuts.md` "The cuts"): `root` and `stage` query values, each encoded once, an
   * absent option left out, `trace` and `in_flow` sent only as true. A 404 is the exit-1 document (a ROOT naming
   * nothing, `reason` set), resolved as data. Takes its query's AbortSignal.
   */
  async getFlow(project: string, options: FlowOptions, signal?: AbortSignal): Promise<FlowView> {
    const query = queryOf([
      ["root", options.root],
      ["depth", options.depth],
      ["stage", options.stage],
      ["trace", options.trace === true ? true : undefined],
      ["in_flow", options.in_flow === true ? true : undefined],
    ]);
    return (await request("GET", `${projectPath(project)}/flow${query}`, { signal })) as FlowView;
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

  /**
   * `spec health --json` for the project, no query (`docs/features/health-left.md` "Data"): the
   * unapplied amendments, none a 200 too; a 503 (the walk incomplete, a config refused) and an
   * older daemon's 404 reject in the daemon's words, never read as none. No AbortSignal: a walk in
   * flight is never aborted, as the check's.
   */
  async getHealth(project: string): Promise<HealthReport> {
    return (await request("GET", `${projectPath(project)}/health`)) as HealthReport;
  }

  /**
   * `spec roadmap --json` as the browser view, uncut (`crates/specengine-http/README.md`
   * "Endpoints"): no query for the whole plan, else `release` or `pool=true`, never both (the
   * options' type cannot hold the pair the daemon refuses, 400). A 404 is the exit-1 document (a
   * `release` naming no release, `reason` set), resolved as data; a 503 (no `[roadmap]`, a bad
   * table) and an older daemon's 404 reject in the daemon's words. Takes its query's AbortSignal.
   */
  async getRoadmap(project: string, options?: RoadmapOptions, signal?: AbortSignal): Promise<RoadmapDocument> {
    const query = queryOf([
      ["release", options?.release],
      ["pool", options?.pool === true ? true : undefined],
    ]);
    return (await request("GET", `${projectPath(project)}/roadmap${query}`, { signal })) as RoadmapDocument;
  }

  /**
   * `spec lock diff REF --json`, uncut (`docs/features/lock-diff.md` "Route"): `ref` the one query,
   * the REF as given, encoded once (`#` as %23, `/` as %2F, a `%` as %25). A 404 is the exit-1
   * document (a REF naming nothing: `reason` set, `nodes` empty), resolved as data; a 400 (the
   * query refused), a 503 (exit 2: a look-alike ID, the config, the store) and an older daemon's
   * 404 error body reject in the daemon's words. Takes its query's AbortSignal.
   */
  async getLockDiff(project: string, ref: string, signal?: AbortSignal): Promise<LockDiffView> {
    const query = queryOf([["ref", ref]]);
    return (await request("GET", `${projectPath(project)}/lock-diff${query}`, { signal })) as LockDiffView;
  }

  /**
   * Stages the owner's choice (`docs/canon/decision-staging.md` "Daemon"): a JSON body of exactly
   * the stage's fields, then `updated_at` as this page read it; no AbortSignal (a write's answer is
   * always taken). 200: the review document. A refusal carrying the document (409: not open, an
   * orphan's approve, changed since read; 404): its last note; an error body (400 a flag's usage or
   * an `as` off the person grammar, 403 the fence or a hub's other login, 503): its message verbatim.
   */
  async stageDecision(project: string, id: string, stage: StageChoice, updatedAt: string, as?: string | null): Promise<Proposal> {
    // `as` last, and only when given: the Questions screen's body stays as it was.
    const body = JSON.stringify({ ...stage, updated_at: updatedAt, ...(as === undefined ? {} : { as }) });
    return (await request("POST", decisionPath(project, id), { body })) as Proposal;
  }

  /** Drops the staged choice: DELETE on the same path, no body, no AbortSignal; 200 the review document. */
  async unstageDecision(project: string, id: string): Promise<Proposal> {
    return (await request("DELETE", decisionPath(project, id))) as Proposal;
  }

  /**
   * What a terminal would ask of a staged proposal (`docs/features/hub-confirm.md` "Data"): a hub's
   * read, asked when a Confirm dialog opens, judged as the POST is, so a refusal rejects in the
   * daemon's words whatever its body and no 404 resolves as data. A one-factor session's `fetch`
   * meets the login door's redirect: status 0 (`docs/canon/hub.md` "Known limits").
   */
  async getConfirm(project: string, id: string, signal?: AbortSignal): Promise<ConfirmRead> {
    return (await request("GET", confirmPath(project, id), { signal, judged: true })) as ConfirmRead;
  }

  /**
   * Confirms one staged proposal (`docs/features/hub-confirm.md` "Data"): a JSON body of exactly the
   * read's `question` and `updated_at`, written out here so nothing else can travel; no AbortSignal
   * (a write's answer is always taken). 200: the review document. A refusal carrying a document
   * (409, 404): its last note; an error body (400, 403, 503): its message verbatim.
   */
  async confirmProposal(project: string, id: string, { question, updated_at }: ConfirmBody): Promise<Proposal> {
    const sent: ConfirmBody = { question, updated_at };
    return (await request("POST", confirmPath(project, id), { body: JSON.stringify(sent) })) as Proposal;
  }

  /**
   * The push status of a hub with `--push` (`docs/features/hub-git.md` "Data"): asked once a login
   * is known, judged, so a 404, the unknown route of a local daemon or of a hub without `--push`,
   * rejects as every other refusal does and nothing but a 200 resolves as data.
   */
  async getSync(project: string, signal?: AbortSignal): Promise<HubSync> {
    return (await request("GET", `${projectPath(project)}/sync`, { signal, judged: true })) as HubSync;
  }

  /**
   * `spec round new [--limit N] --json`, uncut: `limit` the one query, left out when absent (the
   * daemon's default, 20). An unknown project or a refused `limit` rejects in the daemon's words.
   */
  async getRound(project: string, options: RoundOptions = {}, signal?: AbortSignal): Promise<RoundSheet> {
    const query = queryOf([["limit", options.limit]]);
    return (await request("GET", `${roundPath(project)}${query}`, { signal })) as RoundSheet;
  }

  /**
   * Stages the round's answers in one POST (`docs/features/ui-round.md` "Data"): the body exactly
   * the project's slug, then the answers in order, each its ID and stage, the stage the decision
   * route's body (the choice, then `updated_at` as read); no AbortSignal (a write's answer is always
   * taken). 200: the answers document, whatever each answer became; an error body (400, 403, 413,
   * 415, 503): its message.
   */
  async stageRoundAnswers(project: string, answers: RoundAnswerBody[]): Promise<RoundAnswersOutcome> {
    const sent: RoundAnswersBody = { project, answers };
    return (await request("POST", `${roundPath(project)}/answers`, { body: JSON.stringify(sent) })) as RoundAnswersOutcome;
  }

  /**
   * `spec card list [--with-cancelled] [--marks] --json` as the browser view
   * (`docs/features/board-ui.md` "Routes"): `with_cancelled` and `marks`, each sent only as true, and
   * `parent` when asked (the Board never asks: it finds children in the one list); the screen filters
   * the answer. A 503 (a bad `[card]`, an index that cannot be read
   * for the marks) rejects in the daemon's words. Takes its query's AbortSignal.
   */
  async getCards(project: string, options: CardsOptions = {}, signal?: AbortSignal): Promise<CardList> {
    const query = queryOf([
      ["with_cancelled", options.with_cancelled === true ? true : undefined],
      ["marks", options.marks === true ? true : undefined],
      ["parent", options.parent],
    ]);
    return (await request("GET", `${cardsPath(project)}${query}`, { signal })) as CardList;
  }

  /**
   * `spec card show C --history --json`, uncut, no query; the ID a path segment encoded once. A 404
   * is the exit-1 document `{id, reason}`, resolved as data.
   */
  async getCard(project: string, id: string, signal?: AbortSignal): Promise<Card | CardNotFound> {
    return (await request("GET", cardPath(project, id), { signal })) as Card | CardNotFound;
  }

  // The card writes (`docs/features/board-ui.md` "Routes"), each a JSON body and no AbortSignal: a
  // write's answer is always taken. 200 the write document. A refusal (409) rejects with its last
  // note, no such card (404) with its `reason`, an error body with its message.

  async createCard(project: string, body: CardCreateBody): Promise<CardWrite> {
    return (await request("POST", cardsPath(project), { body: JSON.stringify(body) })) as CardWrite;
  }

  async moveCard(project: string, id: string, body: CardMoveBody): Promise<CardWrite> {
    return (await request("POST", `${cardPath(project, id)}/move`, { body: JSON.stringify(body) })) as CardWrite;
  }

  async editCard(project: string, id: string, body: CardEditBody): Promise<CardWrite> {
    return (await request("POST", `${cardPath(project, id)}/edit`, { body: JSON.stringify(body) })) as CardWrite;
  }

  async assignCard(project: string, id: string, body: CardAssignBody): Promise<CardWrite> {
    return (await request("POST", `${cardPath(project, id)}/assign`, { body: JSON.stringify(body) })) as CardWrite;
  }

  async cancelCard(project: string, id: string, body: CardCancelBody): Promise<CardWrite> {
    return (await request("POST", `${cardPath(project, id)}/cancel`, { body: JSON.stringify(body) })) as CardWrite;
  }

  async acceptCard(project: string, id: string, body: CardAcceptBody): Promise<CardWrite> {
    return (await request("POST", `${cardPath(project, id)}/accept`, { body: JSON.stringify(body) })) as CardWrite;
  }

  async clearCard(project: string, id: string, body: CardClearBody): Promise<CardWrite> {
    return (await request("POST", `${cardPath(project, id)}/clear`, { body: JSON.stringify(body) })) as CardWrite;
  }

  /**
   * A question or a decision raised on the card, L3 of
   * docs/features/card-questions.md "Acceptance criteria": 200 the intake document whether stored or
   * not, `created` saying which; a 409's intake document rejects with its last note, a 404's
   * `{id, reason}` with its reason.
   */
  async raiseOnCard(project: string, id: string, body: CardRaiseBody): Promise<IntakeDocument> {
    return (await request("POST", `${cardPath(project, id)}/items`, { body: JSON.stringify(body) })) as IntakeDocument;
  }

  /** Every suspect card of a node and what it holds, one transaction; a refusal (a 409 or 404 with `{group, cleared, notes}`) its last note. */
  async clearCardGroup(project: string, body: CardClearGroupBody): Promise<CardClearGroupWrite> {
    return (await request("POST", `${tracePath(project)}/clear`, { body: JSON.stringify(body) })) as CardClearGroupWrite;
  }

  /**
   * `spec trace [REF] [--stage S] [--summary] --json` as the browser view
   * (`docs/features/card-trace.md`, `docs/features/trace-views.md` "Routes and types"): `ref` and
   * `stage` query values, `summary` sent only as true. A 404 is the exit-1 document (a REF naming
   * nothing, `reason` set), resolved as data; a 503 rejects in the daemon's words. Takes its query's
   * AbortSignal.
   */
  async getTrace(project: string, options: TraceOptions = {}, signal?: AbortSignal): Promise<TraceDocument> {
    const query = queryOf([
      ["ref", options.ref],
      ["stage", options.stage],
      ["summary", options.summary === true ? true : undefined],
    ]);
    return (await request("GET", `${tracePath(project)}${query}`, { signal })) as TraceDocument;
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
