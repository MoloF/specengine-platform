import {
  ClientError,
  NO_SUCH_PROPOSAL,
  STAGE_REFUSED,
  type BundleOptions,
  type GraphOptions,
  type NodeOptions,
  type SearchOptions,
  type SpecEngineClient,
  type TreeOptions,
} from "../api/client";
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
import { inboxEntryOf, noReview, stamp, type MockProject, type MockProposal, type StoredReview } from "./build";
import { checkReportOf } from "./check";
import { bundleOf, graphOf, nodeViewOf, searchOf, treeOf } from "./corpus";
import { harborSim } from "./harbor-sim/fixtures";
import { largeDocuments, largeLinks } from "./harbor-sim/large";
import { ledgerApi } from "./ledger-api/fixtures";
import { SLOW_MS, type Scenario } from "./scenario";
import { largeTasks, mockTasks, packageOf, taskListOf, taskNotFound, type MockTasks } from "./tasks";

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
  /** `conflict`: the proposals another page has staged on already, once each. */
  private readonly stagedElsewhere = new Set<string>();

  constructor(scenario: Scenario, options: MockOptions = {}) {
    this.scenario = scenario;
    this.now = options.now ?? Date.now;
    this.delayMs = options.delayMs ?? (scenario === "slow" ? SLOW_MS : 0);
    const at = this.now();
    this.projects = [harborSim(at), ledgerApi(at)];
    for (const project of this.projects) {
      this.tasks.set(project.project.slug, mockTasks(project.project.slug));
    }
    if (scenario === "empty") {
      for (const project of this.projects) {
        project.proposals = [];
        project.notes = [];
        project.corpus = { documents: [], links: [], treeNotes: [EMPTY_TREE_NOTE] };
        this.tasks.set(project.project.slug, { tasks: [], notes: [] });
      }
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

  async getInbox(project: string): Promise<Inbox> {
    await this.read();
    const entry = this.project(project);
    return structuredClone({ proposals: entry.proposals.map(({ review }) => inboxEntryOf(review)), notes: entry.notes });
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

  async getNode(project: string, ref: string, options: NodeOptions = {}): Promise<NodeView> {
    await this.read();
    return nodeViewOf(this.project(project).corpus, ref, options);
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
    return structuredClone(stored === undefined ? taskNotFound(id) : packageOf(stored, entry.proposals));
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
   * Stages a choice as the daemon does (`docs/canon/decision-staging.md` "The stage", "Daemon"):
   * on an open proposal read at its current `updated_at` only, the flags checked against its
   * options, replacing any staged choice; nothing applied. The `conflict` scenario: the first stage
   * of each proposal finds one staged meanwhile by another page, so it is refused 409 with the
   * current document's reason, and the next one, on the new `updated_at`, is taken.
   */
  async stageDecision(project: string, id: string, stage: StageChoice, updatedAt: string): Promise<Proposal> {
    await this.wait();
    const { stored, index, entry } = this.openProposal(project, id);
    let current = stored.review;
    if (this.scenario === "conflict" && !this.stagedElsewhere.has(`${project}/${id}`)) {
      this.stagedElsewhere.add(`${project}/${id}`);
      current = this.store(entry, index, stored, { ...current, staged: ELSEWHERE, staged_at: this.stamp(), updated_at: this.stamp() });
    }
    if (current.updated_at !== updatedAt) {
      throw this.refusal(`\`${id}\` changed since it was read (updated ${current.updated_at}, read ${updatedAt}): read it again; nothing changed`);
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
    return structuredClone(this.store(entry, index, stored, { ...current, staged, staged_at: at, updated_at: at }));
  }

  /** Drops the staged choice of an open proposal; nothing staged: the document as it is. */
  async unstageDecision(project: string, id: string): Promise<Proposal> {
    await this.wait();
    const { stored, index, entry } = this.openProposal(project, id);
    if (stored.review.staged === null) {
      return structuredClone(stored.review);
    }
    return structuredClone(this.store(entry, index, stored, { ...stored.review, staged: null, staged_at: null, updated_at: this.stamp() }));
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
