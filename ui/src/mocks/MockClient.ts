import {
  ClientError,
  NO_SUCH_PROPOSAL,
  STAGE_REFUSED,
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
  IntakeMatch,
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
  Team,
  TraceDocument,
  TreeView,
  WorkspaceDocument,
} from "../api/types";
import { mockCode } from "./bindings";
import { fakeHex, inboxEntryOf, noReview, proposal, stamp, type MockProject, type MockProposal, type StoredReview } from "./build";
import { checkReportOf } from "./check";
import {
  acceptCardIn,
  assignCardIn,
  cancelCardIn,
  cardListOf,
  cardShowOf,
  changeElsewhere,
  clearCardGroupIn,
  clearCardIn,
  createCardIn,
  editCardIn,
  logItemEvent,
  mockBoard,
  moveCardIn,
  raiseOnCardIn,
  type MockBoard,
  type Settled,
  type StoredCard,
} from "./cards";
import { bundleOf, graphOf, nodeViewOf, searchOf, treeOf } from "./corpus";
import { editorialScenario } from "./editorial";
import { flowOf, mockFlowOf } from "./flow";
import { ASIDE_KINDS, ASIDE_STATUSES, asideCorpusLinks, asideDocuments } from "./harbor-sim/aside";
import { harborSim } from "./harbor-sim/fixtures";
import { healthReportOf, walkGapMessage } from "./health";
import { largeDocuments, largeLinks } from "./harbor-sim/large";
import { ledgerApi } from "./ledger-api/fixtures";
import { lockDiffOf } from "./lockDiff";
import { mockPlan, noPlan, roadmapOf, taskPlaceOf } from "./roadmap";
import { SLOW_MS, type Scenario } from "./scenario";
import { largeTasks, mockTasks, packageOf, taskListOf, taskNotFound, type MockTasks } from "./tasks";
import { traceOf, withTrace } from "./trace";
import { workspacesOf } from "./workspaces";

export interface MockOptions {
  /** The clock; fixture times are relative to it at construction. */
  now?: () => number;
  /** Delay of every call; `slow` defaults to SLOW_MS, the rest to none. */
  delayMs?: number;
}

/** The tree's note for a project with no spec document, as `spec tree` words it. */
export const EMPTY_TREE_NOTE = 'no document under [paths] spec "docs/spec"; give a ROOT';

/** The caps of a staged note and reason, as the CLI's (`docs/canon/decision-staging.md` "The stage"). */
const NOTE_MAX_BYTES = 4096;

/** The `conflict` scenario's stage made elsewhere: a reject another page staged meanwhile. */
const ELSEWHERE: Stage = { decision: "reject", reason: "Staged in another tab (mock scenario: conflict)" };

/** A local daemon's unknown route, the mock's words: a hub alone confirms (`docs/features/hub-confirm.md` B8). */
const NO_CONFIRM_ROUTE = "no route /api/projects/<slug>/proposals/<id>/confirm: the routes are /api/projects and the web UI (mock)";

const NO_SYNC_ROUTE = "no route /api/projects/<slug>/sync: the routes are /api/projects and the web UI (mock)";

/** The `hub` scenario's login and its `[team]` row: every act and Confirm (`docs/canon/hub-confirm.md` "Routes"). */
const HUB_TEAM: Team = { login: "mira", name: "Mira Holt", allowed: ["stage", "accept", "clear", "group_clear", "confirm"] };

/** Who a hub's confirm records, its login and `[team]` email (`docs/canon/hub-confirm.md` "Rules", Identity). */
const HUB_DECIDER = "mira <mira@harbor.example.invalid>";

/** The states `spec inbox` leaves out without `--all` (`docs/canon/proposal-review.md`): decided ones. */
const DECIDED = new Set(["applied", "rejected"]);

/**
 * The `conflict` scenario's corpus hit: the accepted decision a page's first raise on a project meets,
 * not stored until named (docs/features/card-questions.md "Acceptance criteria", U2).
 */
const CORPUS_HIT: IntakeMatch = {
  id: "DEC-0012",
  source: "corpus",
  status: "accepted",
  path: "docs/spec/records/DEC-0012-simulated-weather.md",
  answer: "Weather is simulated",
  record: "DEC-0012",
};

/** The mock's question round, in a chunk of its own: only the Questions screen reads it. */
function loadRound() {
  return import("./round");
}

function bytesOf(text: string): number {
  return new TextEncoder().encode(text).length;
}

/**
 * The typed mock behind SpecEngineClient: two invented projects, their spec and their queues in
 * memory (a staged choice changes the queue until reload; nothing is ever applied here, a stage
 * is confirmed only on a terminal: ADR-0035), and the scenario picked at bootstrap. Reads answer
 * as `spec serve` does for a browser: uncut, an unknown REF as its exit-1 document, a refusal
 * (exit 2) as 503 with the CLI's words. A read's AbortSignal is ignored: no daemon runs the read,
 * so there is nothing to spare.
 */
export class MockClient implements SpecEngineClient {
  readonly dataSource = "mock";
  readonly scenario: Scenario;
  private readonly projects: MockProject[];
  /** Each project's tasks by slug (src/mocks/tasks.ts); read only: no owner action reaches them here. */
  private readonly tasks = new Map<string, MockTasks>();
  private readonly delayMs: number;
  private readonly now: () => number;
  /** Each project's cards by slug (src/mocks/cards.ts): the daemon's writes on them, until a reload. */
  private readonly boards = new Map<string, MockBoard>();
  /** `conflict`: the projects whose first card write has met a change made elsewhere. */
  private readonly cardConflicted = new Set<string>();
  /** `conflict`: the proposals another page has staged on already, once each. */
  private readonly stagedElsewhere = new Set<string>();
  /** `conflict`: the projects whose first round POST has met a stage made elsewhere. */
  private readonly roundConflicted = new Set<string>();
  /** `conflict`: the projects whose first raise on a card has met its corpus hit. */
  private readonly raiseConflicted = new Set<string>();

  constructor(scenario: Scenario, options: MockOptions = {}) {
    this.scenario = scenario;
    this.now = options.now ?? Date.now;
    this.delayMs = options.delayMs ?? (scenario === "slow" ? SLOW_MS : 0);
    const at = this.now();
    this.projects = [harborSim(at), ledgerApi(at)];
    for (const project of this.projects) {
      this.tasks.set(project.project.slug, mockTasks(project.project.slug));
      this.boards.set(project.project.slug, mockBoard(project.project.slug, scenario, at));
    }
    if (scenario === "empty") {
      for (const project of this.projects) {
        project.proposals = [];
        project.notes = [];
        project.corpus = { documents: [], links: [], treeNotes: [EMPTY_TREE_NOTE] };
        this.tasks.set(project.project.slug, { tasks: [], notes: [] });
      }
    }
    if (scenario === "editorial") {
      editorialScenario(this.projects, at);
    }
    if (scenario === "aside" || scenario === "aside40") {
      // The pottery workshop of `fixtures/flow-aside` in place of harbor-sim's spec (`docs/canon/spec-flow-screen.md` "Mock"; RM-0094).
      const harbor = this.project("harbor-sim");
      const documents = asideDocuments(scenario === "aside40" ? 40 : 1);
      harbor.corpus = { documents, links: asideCorpusLinks(documents), treeNotes: [] };
      harbor.nodeKinds = ASIDE_KINDS;
      harbor.specStatuses = ASIDE_STATUSES;
    }
    if (scenario === "large") {
      const harbor = this.project("harbor-sim");
      const documents = largeDocuments();
      harbor.corpus = {
        ...harbor.corpus,
        documents: [...harbor.corpus.documents, ...documents],
        links: [...harbor.corpus.links, ...largeLinks(documents)],
      };
      const tasks = this.tasks.get("harbor-sim");
      if (tasks !== undefined) {
        tasks.tasks = [...tasks.tasks, ...largeTasks()];
      }
    }
  }

  async getProjects(): Promise<Project[]> {
    await this.read();
    return this.projects.map((entry) => structuredClone(entry.project));
  }

  /** No hub behind the mock but in `hub`: the answer of a local daemon (`docs/canon/hub.md` "Routes"), else `mira`. */
  async getIdentity(): Promise<Identity> {
    await this.read();
    return { login: this.scenario === "hub" ? HUB_TEAM.login : null };
  }

  /** No hub behind the mock but in `hub`: a local daemon has no such route (`docs/features/hub-team.md` A7), and the page never asks it; `hub`, mira's row. */
  async getTeam(project?: string): Promise<Team> {
    await this.read();
    if (this.scenario === "hub") {
      this.project(project ?? "");
      return structuredClone(HUB_TEAM);
    }
    throw new ClientError({ status: 404, message: "no route /api/projects/<slug>/team: the routes are /api/projects and the web UI (mock)" });
  }

  /** The project's workspaces and modules for the scenario (src/mocks/workspaces.ts); the daemon reads its own config (`docs/features/workspaces.md` "Data"). */
  async getWorkspaces(project: string): Promise<WorkspaceDocument> {
    await this.read();
    this.project(project);
    return structuredClone(workspacesOf(project, this.scenario));
  }

  /** The queue as `spec inbox` lists it: every state but the decided ones, which only `--all` lists. */
  async getInbox(project: string): Promise<Inbox> {
    await this.read();
    const entry = this.project(project);
    const listed = entry.proposals.filter(({ review }) => !DECIDED.has(review.status));
    return structuredClone({ proposals: listed.map(({ review }) => inboxEntryOf(review)), notes: entry.notes });
  }

  /** The review document; an unknown ID answers the exit-1 document, as data. */
  async getProposal(project: string, id: string): Promise<Proposal> {
    await this.read();
    const stored = this.project(project).proposals.find(({ review }) => review.id === id);
    return structuredClone(stored === undefined ? noReview(id) : stored.review);
  }

  async getTree(project: string, options: TreeOptions = {}): Promise<TreeView> {
    await this.read();
    return treeOf(this.project(project).corpus, options);
  }

  /** A node's read; with `bindings`, the project's code layer in this scenario (src/mocks/bindings.ts), built only then. */
  async getNode(project: string, ref: string, options: NodeOptions = {}): Promise<NodeView> {
    await this.read();
    const code = (options.with ?? []).includes("bindings") ? mockCode(project, this.scenario) : undefined;
    return nodeViewOf(this.project(project).corpus, ref, options, code);
  }

  async search(project: string, options: SearchOptions): Promise<SearchResults> {
    await this.read();
    return searchOf(this.project(project).corpus, options);
  }

  /** The mock corpus's bundle; the daemon serves its own (`docs/features/daemon-read.md` "Data"). */
  async getBundle(project: string, options: BundleOptions): Promise<BundleView> {
    await this.read();
    return bundleOf(this.project(project).corpus, options);
  }

  /** The mock corpus's walk; the daemon serves its own (`docs/features/ui-live.md` "Data"). */
  async getGraph(project: string, options: GraphOptions): Promise<GraphView> {
    await this.read();
    return graphOf(this.project(project).corpus, options);
  }

  /** The project's flow for the scenario (src/mocks/flow.ts), each box's rollup joined with `trace`; the daemon serves its own (`docs/canon/spec-flow.md` "The read"). */
  async getFlow(project: string, options: FlowOptions): Promise<FlowView> {
    await this.read();
    const view = flowOf(project, this.project(project).corpus, this.scenario, options);
    return structuredClone(options.trace === true ? withTrace(view, this.traced(project, {})) : view);
  }

  /** The mock's task list; the daemon serves its own (`docs/features/ui-live-tasks.md` "Data"). */
  async getTasks(project: string): Promise<TaskList> {
    await this.read();
    this.project(project);
    return structuredClone(taskListOf(this.taskStore(project)));
  }

  /**
   * The mock's package; the daemon serves its own (`docs/features/ui-live-tasks.md` "Data"). The
   * open proposals and assumptions are read from this mock's queue now, so a decision in the Inbox
   * drops a proposal from both; an unknown T answers the exit-1 document, as data.
   */
  async getTask(project: string, id: string): Promise<TaskPackage | TaskNotFound> {
    await this.read();
    const entry = this.project(project);
    const stored = this.taskStore(project).tasks.find((candidate) => candidate.id === id);
    return structuredClone(stored === undefined ? taskNotFound(id) : packageOf(stored, entry.proposals, taskPlaceOf(this.planOf(project), id)));
  }

  /**
   * The project's plan for the scenario (src/mocks/roadmap.ts), its tasks joined, read only; the
   * daemon serves its own (`crates/specengine-http/README.md` "Endpoints"). A project with no
   * `[roadmap]` (ledger-api) is refused 503 in the CLI's words in every scenario. `release` keeps
   * that release's group, `pool` the pool and unplaced, as `spec roadmap` does; a release not
   * listed answers the exit-1 document, as data.
   */
  async getRoadmap(project: string, options?: RoadmapOptions): Promise<RoadmapDocument> {
    await this.read();
    this.project(project);
    const whole = this.planOf(project);
    if (whole === null) {
      throw noPlan();
    }
    if (options === undefined) {
      return structuredClone(whole);
    }
    if (options.pool === true) {
      return structuredClone({ ...whole, pool: true, groups: whole.groups.filter((group) => group.group !== "release") });
    }
    const release = options.release;
    const kept = whole.groups.filter((group) => group.group === "release" && group.name === release);
    if (kept.length === 0) {
      const names = whole.groups.flatMap((group) => (group.group === "release" && group.name !== null ? [group.name] : []));
      return structuredClone({ ...whole, release, reason: `no release "${release}" in [roadmap] releases (${names.join(", ")})`, groups: [] });
    }
    return structuredClone({ ...whole, release, groups: kept });
  }

  /**
   * The project's lock diff for the scenario (src/mocks/lockDiff.ts), read only; the daemon serves
   * its own (`docs/features/lock-diff.md` "Route"). REF resolved as the node read resolves it: one
   * naming nothing answers the exit-1 document, as data; a look-alike is refused 503 in the CLI's
   * words.
   */
  async getLockDiff(project: string, ref: string): Promise<LockDiffView> {
    await this.read();
    const shown = nodeViewOf(this.project(project).corpus, ref);
    return structuredClone(lockDiffOf(project, this.scenario, shown));
  }

  /** The project's plan document now, its tasks joined; null without `[roadmap]`. */
  private planOf(project: string): RoadmapDocument | null {
    const plan = mockPlan(project, this.scenario);
    return plan === null ? null : roadmapOf(plan, this.taskStore(project));
  }

  /**
   * The project's report for the scenario (src/mocks/check.ts), read only; a decision changes
   * nothing in it. The daemon serves its own (`docs/features/ui-live.md` "Data").
   */
  async getCheck(project: string): Promise<CheckReport> {
    await this.read();
    this.project(project);
    return structuredClone(checkReportOf(project, this.scenario));
  }

  /**
   * The project's unapplied amendments for the scenario (src/mocks/health.ts), read only; a
   * decision changes nothing in them. `cannot-check`: the walk is incomplete, a 503 in the CLI's
   * words, never none. The daemon serves its own (`docs/features/health-left.md` "Data").
   */
  async getHealth(project: string): Promise<HealthReport> {
    await this.read();
    this.project(project);
    if (this.scenario === "cannot-check") {
      throw new ClientError({ status: 503, message: walkGapMessage(project) });
    }
    return structuredClone(healthReportOf(project, this.scenario));
  }

  /**
   * Stages a choice as the daemon does (`docs/canon/decision-staging.md` "The stage", "Daemon"):
   * on an open proposal read at its current `updated_at` only, the flags checked against its
   * options (`--editorial` only on a change that writes an existing node), replacing any staged
   * choice; nothing applied. A drift item takes none: decided on a terminal only, it is refused 409
   * in the daemon's words, after the `updated_at` check as the daemon orders them
   * (`docs/features/drift-queue.md` "Review, inbox, round, stage"). The `conflict` scenario: the
   * first stage of each proposal finds one staged meanwhile by another page, so it is refused 409
   * with the current document's reason, and the next one, on the new `updated_at`, is taken; a
   * drift item holds no stage, so none is found on it.
   */
  async stageDecision(project: string, id: string, stage: StageChoice, updatedAt: string, as?: string | null): Promise<Proposal> {
    await this.wait();
    const { stored, index, entry } = this.openProposal(project, id);
    let current = stored.review;
    if (this.scenario === "conflict" && current.drift === null && !this.stagedElsewhere.has(`${project}/${id}`)) {
      this.stagedElsewhere.add(`${project}/${id}`);
      current = this.store(entry, index, stored, { ...current, staged: ELSEWHERE, staged_at: this.stamp(), updated_at: this.stamp() });
    }
    if (current.updated_at !== updatedAt) {
      throw this.refusal(`\`${id}\` changed since it was read: updated at ${current.updated_at}, the choice was made on the one of ${updatedAt}; read it again; nothing staged`);
    }
    if (current.drift !== null) {
      throw this.refusal(`${id} is a drift item, decided on a terminal only; nothing staged`);
    }
    if (stage.decision === "approve") {
      this.checkApprove(current, stage);
    } else if (stage.reason.trim() === "") {
      throw new ClientError({ status: 400, message: "spec: --reason: the reason is blank" });
    } else if (bytesOf(stage.reason) > NOTE_MAX_BYTES) {
      throw new ClientError({ status: 400, message: `spec: --reason: over ${String(NOTE_MAX_BYTES)} bytes` });
    }
    const staged: Stage = stage.decision === "approve" ? { ...stage, span_hash: current.base_hash } : stage;
    const at = this.stamp();
    // Its event, a trailing `as` when one is sent: a card naming it reads it (docs/features/card-questions.md "Acceptance criteria", L4).
    logItemEvent(this.board(project), id, "proposal.staged", { staged, staged_at: at, ...(as === undefined || as === null ? {} : { as }) }, at);
    return structuredClone(this.store(entry, index, stored, { ...current, staged, staged_at: at, updated_at: at }));
  }

  /**
   * No hub behind the mock but in `hub`: a local daemon has no such route (`docs/features/hub-confirm.md`
   * B8), and the page never asks it. `hub`: an open proposal with a staged choice reads its question,
   * what a terminal would ask; nothing staged, a 409 in the daemon's words.
   */
  async getConfirm(project?: string, id?: string): Promise<ConfirmRead> {
    await this.read();
    if (this.scenario !== "hub") {
      throw new ClientError({ status: NO_SUCH_PROPOSAL, message: NO_CONFIRM_ROUTE });
    }
    const { stored } = this.openProposal(project ?? "", id ?? "");
    return { id: stored.review.id, question: this.confirmQuestion(stored.review), updated_at: stored.review.updated_at };
  }

  /**
   * As `getConfirm`: nothing is confirmed in a browser without a hub. `hub`: the read's question and
   * `updated_at` sent back confirm the staged choice under the login, as `spec approve|reject` would:
   * applied or rejected, the stage gone, its events logged; any other text, a 409, nothing changed.
   */
  async confirmProposal(project?: string, id?: string, body?: ConfirmBody): Promise<Proposal> {
    await this.wait();
    if (this.scenario !== "hub") {
      throw new ClientError({ status: NO_SUCH_PROPOSAL, message: NO_CONFIRM_ROUTE });
    }
    const { stored, index, entry } = this.openProposal(project ?? "", id ?? "");
    const review = stored.review;
    const staged = review.staged;
    if (staged === null || body?.updated_at !== review.updated_at || body.question !== this.confirmQuestion(review)) {
      throw this.refusal(`\`${review.id}\` changed since this dialog read it: read it again; nothing changed`);
    }
    const at = this.stamp();
    const approve = staged.decision === "approve";
    const board = this.board(project ?? "");
    logItemEvent(board, review.id, approve ? "proposal.approved" : "proposal.rejected", { as: HUB_TEAM.login }, at);
    const commit = approve ? fakeHex(`${review.id}\n${at}`, 40) : null;
    if (approve) {
      logItemEvent(board, review.id, "proposal.applied", { commit }, at);
    }
    const decided: StoredReview = {
      ...review,
      status: approve ? "applied" : "rejected",
      decided_by: HUB_DECIDER,
      decided_at: at,
      decision_note: approve ? staged.note : staged.reason,
      applied_commit: commit,
      staged: null,
      staged_at: null,
      updated_at: at,
    };
    return structuredClone(this.store(entry, index, stored, decided));
  }

  /** What a terminal would ask of a staged choice, its stage line first (`docs/canon/hub-confirm.md` "Routes"). */
  private confirmQuestion(review: StoredReview): string {
    const staged = review.staged;
    if (staged === null) {
      throw this.refusal(`\`${review.id}\` has no staged choice to confirm; nothing changed`);
    }
    const verb = staged.decision === "approve" ? "approve" : "reject";
    return `staged ${review.staged_at ?? ""}: spec ${verb} ${review.id}\n${verb} ${review.id} (${review.kind}, staged)? [y/N]`;
  }

  /** No hub with `--push` behind the mock: a local daemon has no such route (`docs/features/hub-git.md` "Status and the screen"), and the page never asks it. */
  async getSync(): Promise<HubSync> {
    await this.read();
    throw new ClientError({ status: 404, message: NO_SYNC_ROUTE });
  }

  /** Drops the staged choice of an open proposal; nothing staged: the document as it is. */
  async unstageDecision(project: string, id: string): Promise<Proposal> {
    await this.wait();
    const { stored, index, entry } = this.openProposal(project, id);
    if (stored.review.staged === null) {
      return structuredClone(stored.review);
    }
    logItemEvent(this.board(project), id, "proposal.unstaged", {}, this.stamp());
    return structuredClone(this.store(entry, index, stored, { ...stored.review, staged: null, staged_at: null, updated_at: this.stamp() }));
  }

  /**
   * The round sheet as the daemon serves it (`docs/features/ui-round.md` "Data"): the open
   * questions and discrepancies in round order, at most `limit` (1 to 200, default 20), the rest
   * noted; read only.
   */
  async getRound(project: string, options: RoundOptions = {}): Promise<RoundSheet> {
    await this.read();
    const entry = this.project(project);
    const { roundSheetOf } = await loadRound();
    return structuredClone(roundSheetOf(entry, this.taskStore(project), options.limit));
  }

  /**
   * Stages the round's answers as the daemon does (`docs/features/ui-round.md` "Data"): each
   * answer refused alone, the rest staged at one clock reading, nothing applied. The `conflict`
   * scenario: a project's first POST finds its first answer's proposal staged meanwhile by another
   * page, so that answer is refused as changed since read; the next POST, on the new sheet, takes it.
   */
  async stageRoundAnswers(project: string, answers: RoundAnswerBody[]): Promise<RoundAnswersOutcome> {
    await this.wait();
    const entry = this.project(project);
    const first = answers[0];
    if (this.scenario === "conflict" && first !== undefined && !this.roundConflicted.has(project)) {
      this.roundConflicted.add(project);
      const index = entry.proposals.findIndex(({ review }) => review.id === first.id);
      const stored = entry.proposals[index];
      // A drift item holds no stage: none is found staged on it meanwhile.
      if (stored !== undefined && stored.review.status === "open" && stored.review.drift === null) {
        this.store(entry, index, stored, { ...stored.review, staged: ELSEWHERE, staged_at: this.stamp(), updated_at: this.stamp() });
      }
    }
    const { stageAnswers } = await loadRound();
    return structuredClone(stageAnswers(entry, answers, this.stamp()));
  }

  /** The proposal a stage is for: unknown 404; not open 409 with its document's reason. */
  private openProposal(project: string, id: string): { stored: MockProposal; index: number; entry: MockProject } {
    const entry = this.project(project);
    const index = entry.proposals.findIndex(({ review }) => review.id === id);
    const stored = entry.proposals[index];
    if (stored === undefined) {
      throw new ClientError({ status: NO_SUCH_PROPOSAL, message: `no proposal \`${id}\` in this project's queue` });
    }
    if (stored.review.status !== "open") {
      throw this.refusal(`\`${id}\` is ${stored.review.status}, not open: only an open proposal takes a staged choice; nothing changed`);
    }
    return { stored, index, entry };
  }

  /** The decision flags against the proposal's options, as `spec approve` checks them (`docs/canon/decision-record.md` "Flags"). */
  private checkApprove(review: StoredReview, stage: Extract<StageChoice, { decision: "approve" }>): void {
    const last = review.options.length - 1;
    if (stage.option === null && last >= 0) {
      throw new ClientError({ status: 400, message: `spec: \`${review.id}\` takes --option N (0-${String(last)})` });
    }
    if (stage.option !== null && last < 0) {
      throw new ClientError({ status: 400, message: `spec: --option: \`${review.id}\` has no options` });
    }
    if (stage.option !== null && review.options[stage.option] === undefined) {
      throw this.refusal(`--option ${String(stage.option)}: \`${review.id}\` has options 0-${String(last)}; nothing changed`);
    }
    if (stage.note !== null && bytesOf(stage.note) > NOTE_MAX_BYTES) {
      throw new ClientError({ status: 400, message: `spec: --note: over ${String(NOTE_MAX_BYTES)} bytes` });
    }
    // A usage refusal, as `spec approve --editorial` on a kind writing no existing node exits 2
    // (`docs/features/rev-discipline.md` "Review document, stage, queue"): here, one with no base text.
    if (stage.editorial === true && review.base_text === null) {
      throw new ClientError({ status: 400, message: `spec: --editorial: \`${review.id}\` writes no existing node, so it raises no revision; approve it without --editorial` });
    }
  }

  /** A 409 as HttpClient rejects one: the refused review document carries `reason` as its last note. */
  private refusal(reason: string): ClientError {
    return new ClientError({ status: STAGE_REFUSED, message: reason });
  }

  /** Keeps a proposal's new review document in the queue and returns it. */
  private store(entry: MockProject, index: number, stored: MockProposal, review: StoredReview): StoredReview {
    entry.proposals[index] = { ...stored, review };
    return review;
  }

  /** Now, as the queue stores a time (1 s resolution). */
  private stamp(): string {
    return stamp(this.now(), 0);
  }

  /** The project's cards (src/mocks/cards.ts); the daemon serves its own (`docs/features/board-ui.md` "Routes"). */
  async getCards(project: string, options: CardsOptions = {}): Promise<CardList> {
    await this.read();
    return structuredClone(cardListOf(this.board(project), options, this.settled(project)));
  }

  /** One card with its history and its items, read from this mock's queue now; an unknown C answers the exit-1 document, as data. */
  async getCard(project: string, id: string): Promise<Card | CardNotFound> {
    await this.read();
    return cardShowOf(this.board(project), id, this.taskStore(project), this.project(project).proposals);
  }

  /**
   * A question or a decision raised on a card into this mock's queue (src/mocks/cards.ts
   * `raiseOnCardIn`). The `conflict` scenario: a project's first raise meets one corpus hit, the
   * accepted DEC-0012, and is not stored until it names it.
   */
  async raiseOnCard(project: string, id: string, body: CardRaiseBody): Promise<IntakeDocument> {
    await this.wait();
    const entry = this.project(project);
    const corpus = this.scenario === "conflict" && !this.raiseConflicted.has(project) ? CORPUS_HIT : null;
    const answer = raiseOnCardIn(this.board(project), entry.proposals, project, id, body, this.stamp(), corpus);
    if (corpus !== null) {
      this.raiseConflicted.add(project);
      // The hit stays the daemon's answer to every raise until one names it.
      if (!answer.created) {
        this.raiseConflicted.delete(project);
      }
    }
    return structuredClone(answer);
  }

  async createCard(project: string, body: CardCreateBody): Promise<CardWrite> {
    await this.wait();
    return createCardIn(this.board(project), body, this.stamp());
  }

  async moveCard(project: string, id: string, body: CardMoveBody): Promise<CardWrite> {
    await this.wait();
    this.meetConflict(project, id);
    return moveCardIn(this.board(project), id, body, this.stamp());
  }

  async editCard(project: string, id: string, body: CardEditBody): Promise<CardWrite> {
    await this.wait();
    this.meetConflict(project, id);
    return editCardIn(this.board(project), id, body, this.stamp());
  }

  async assignCard(project: string, id: string, body: CardAssignBody): Promise<CardWrite> {
    await this.wait();
    this.meetConflict(project, id);
    return assignCardIn(this.board(project), id, body, this.stamp());
  }

  async cancelCard(project: string, id: string, body: CardCancelBody): Promise<CardWrite> {
    await this.wait();
    this.meetConflict(project, id);
    return cancelCardIn(this.board(project), id, body, this.stamp());
  }

  async acceptCard(project: string, id: string, body: CardAcceptBody): Promise<CardWrite> {
    await this.wait();
    this.meetConflict(project, id);
    return acceptCardIn(this.board(project), id, body, this.stamp(), (card) => this.departure(project, card, body));
  }

  /** The discrepancy a departing accept makes, in the queue as `accept_departing` puts it (`docs/features/card-trace.md` "Accepting"); its ID. */
  private departure(project: string, card: StoredCard, body: CardAcceptBody): string {
    const entry = this.project(project);
    const highest = entry.proposals.reduce((most, { review }) => Math.max(most, Number(review.id.slice(3))), 0);
    const id = `PR-${String(highest + 1).padStart(4, "0")}`;
    const nodes = body.departs ?? [];
    entry.proposals.push(
      proposal({
        id,
        project,
        kind: "discrepancy",
        created_at: this.stamp(),
        target_id: nodes[0] ?? null,
        target_ids: nodes,
        severity: "normal",
        gap_type: body.gap_type ?? "contradicts",
        summary: body.summary ?? null,
        evidence: [{ file: card.id, qpath: null, lines: null, observed: body.observed ?? "", documented: body.documented ?? "" }],
        options: [
          { label: "Change the brief", effect: "The brief takes what the result does.", price: "not priced at acceptance" },
          { label: "Rework the result", effect: "A new card brings the result back to the brief.", price: "not priced at acceptance" },
        ],
        recommendation: 0,
        // A departing accept's discrepancy names the card accepted (docs/canon/card-questions.md "The rule").
        card_id: card.id,
      }),
    );
    // Raised by the accept, not the card door: its event names no acting name.
    logItemEvent(this.board(project), id, "proposal.created", {}, this.stamp());
    return id;
  }

  async clearCard(project: string, id: string, body: CardClearBody): Promise<CardWrite> {
    await this.wait();
    this.meetConflict(project, id);
    return clearCardIn(this.board(project), id, body, this.stamp());
  }

  async clearCardGroup(project: string, body: CardClearGroupBody): Promise<CardClearGroupWrite> {
    await this.wait();
    return clearCardGroupIn(this.board(project), body, this.stamp(), this.heldBy(project, body.group));
  }

  /** Marks and rollups joined per read (src/mocks/trace.ts); the daemon serves its own (`docs/features/card-trace.md`). */
  async getTrace(project: string, options: TraceOptions = {}): Promise<TraceDocument> {
    await this.read();
    return structuredClone(this.traced(project, options));
  }

  /** `spec trace` over the project's corpus, flow and queue; under a REF its lanes by the scope's own walk (`docs/canon/spec-trace.md` "The read"). */
  private traced(project: string, options: TraceOptions): TraceDocument {
    const { corpus } = this.project(project);
    const flow = mockFlowOf(project, corpus, this.scenario);
    const rooted = options.ref !== undefined && flow.documents.some((document) => document.name === options.ref);
    const walked = flowOf(project, corpus, this.scenario, rooted ? { root: options.ref } : {}).nodes;
    return traceOf(this.board(project), options, {
      flow,
      corpus,
      distance: new Map(walked.map((node) => [node.id ?? node.path, node.distance])),
      settled: this.settled(project),
    });
  }

  /** Whether a discrepancy of the project's queue is settled. */
  private settled(project: string): Settled {
    const entry = this.project(project);
    return (discrepancy) => {
      const status = entry.proposals.find(({ review }) => review.id === discrepancy)?.review.status;
      return status === "applied" || status === "rejected";
    };
  }

  /** REF and what it holds by `parent:`, as a group clear reads it. */
  private heldBy(project: string, ref: string): ReadonlySet<string> {
    const rows = treeOf(this.project(project).corpus).nodes;
    const held = new Set([ref]);
    for (const row of rows) {
      if (row.id !== null && row.parent !== null && held.has(row.parent)) {
        held.add(row.id);
      }
    }
    return held;
  }

  /** The `conflict` scenario: a project's first card write finds its card changed meanwhile, a lost compare-and-set; the next one, on the new read, is taken. */
  private meetConflict(project: string, id: string): void {
    if (this.scenario === "conflict" && !this.cardConflicted.has(project)) {
      this.cardConflicted.add(project);
      changeElsewhere(this.board(project), id, this.stamp());
    }
  }

  private board(project: string): MockBoard {
    const board = this.boards.get(project);
    if (board === undefined) {
      throw new ClientError({ status: 404, message: `no project ${project}` });
    }
    return board;
  }

  /** No live tail: the mock's queue changes only by this page's stages, which read again themselves. */
  subscribe(): () => void {
    return () => undefined;
  }

  /** A read: waits, then fails in the `error` scenario. */
  private async read(): Promise<void> {
    await this.wait();
    if (this.scenario === "error") {
      throw new ClientError({
        status: 503,
        message: "spec index unavailable: the database is locked by another process (mock scenario: error)",
      });
    }
  }

  private wait(): Promise<void> {
    if (this.delayMs <= 0) {
      return Promise.resolve();
    }
    return new Promise((resolve) => setTimeout(resolve, this.delayMs));
  }

  private taskStore(slug: string): MockTasks {
    return this.tasks.get(slug) ?? { tasks: [], notes: [] };
  }

  private project(slug: string): MockProject {
    const entry = this.projects.find((candidate) => candidate.project.slug === slug);
    if (entry === undefined) {
      throw new ClientError({ status: 404, message: `no project ${slug}` });
    }
    return entry;
  }
}
