import {
  ClientError,
  DECIDED_ELSEWHERE,
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
  Decision,
  DecisionResult,
  GraphView,
  Inbox,
  NodeView,
  Project,
  Proposal,
  SearchResults,
  TaskList,
  TaskNotFound,
  TaskPackage,
  TreeView,
} from "../api/types";
import { fakeHex, inboxEntryOf, noReview, stamp, type MockProject, type StoredReview } from "./build";
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

const DECIDED_BY = "Mock Owner <owner@example.org>";

/** The tree's note for a project with no spec document, as `spec tree` words it. */
export const EMPTY_TREE_NOTE = 'no document under [paths] spec "docs/spec"; give a ROOT';

/** A stable 40-hex fake commit id for a proposal. */
function fakeSha(text: string): string {
  return fakeHex(text, 40);
}

/**
 * The typed mock behind SpecEngineClient: two invented projects, their spec and their queues in
 * memory (decisions change the queues until reload; an accepted change is applied in the
 * proposal's worktree, so no node text changes here: ADR-0032), and the scenario picked at
 * bootstrap. Reads answer as `spec serve` does for a browser: uncut, an unknown REF as its exit-1
 * document, a refusal (exit 2) as 503 with the CLI's words.
 */
export class MockClient implements SpecEngineClient {
  readonly dataSource = "mock";
  readonly scenario: Scenario;
  private readonly projects: MockProject[];
  /** Each project's tasks by slug (src/mocks/tasks.ts); read only: no owner action reaches them here. */
  private readonly tasks = new Map<string, MockTasks>();
  private readonly delayMs: number;
  private readonly now: () => number;

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

  /** Served by the mock although the daemon lacks the endpoint (MISSING ENDPOINT in the client). */
  async getBundle(project: string, options: BundleOptions): Promise<BundleView> {
    await this.read();
    return bundleOf(this.project(project).corpus, options);
  }

  /** Served by the mock although the daemon lacks the endpoint (MISSING ENDPOINT in the client). */
  async getGraph(project: string, options: GraphOptions): Promise<GraphView> {
    await this.read();
    return graphOf(this.project(project).corpus, options);
  }

  /** Served by the mock although the daemon lacks the endpoint (MISSING ENDPOINT in the client). */
  async getTasks(project: string): Promise<TaskList> {
    await this.read();
    this.project(project);
    return structuredClone(taskListOf(this.taskStore(project)));
  }

  /**
   * Served by the mock although the daemon lacks the endpoint (MISSING ENDPOINT in the client). The
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
   * Served by the mock although the daemon lacks the endpoint (MISSING ENDPOINT in the client): the
   * project's report for the scenario (src/mocks/check.ts), read only; a decision changes nothing in it.
   */
  async getCheck(project: string): Promise<CheckReport> {
    await this.read();
    this.project(project);
    return structuredClone(checkReportOf(project, this.scenario));
  }

  async decideProposal(project: string, id: string, decision: Decision): Promise<DecisionResult> {
    await this.wait();
    const entry = this.project(project);
    const index = entry.proposals.findIndex(({ review }) => review.id === id);
    const stored = entry.proposals[index];
    if (stored === undefined) {
      throw new ClientError({ status: 404, message: `no open proposal ${id} in ${project}` });
    }
    if (this.scenario === "conflict") {
      entry.proposals.splice(index, 1);
      throw new ClientError({
        status: DECIDED_ELSEWHERE,
        message: `${id} is no longer open: another session applied it as ${fakeSha(`${id}:elsewhere`).slice(0, 7)}`,
      });
    }
    const current = stored.review;
    const decided: StoredReview = {
      ...current,
      decided_by: DECIDED_BY,
      decided_at: stamp(this.now(), 0),
      updated_at: stamp(this.now(), 0),
    };
    switch (decision.decision) {
      case "accept": {
        if (current.preview === "conflicts") {
          throw new ClientError({
            status: 422,
            message: `${id} conflicts with the current text of ${current.target_id ?? "its target"}; nothing was written`,
          });
        }
        if (decision.option !== null && current.options[decision.option] === undefined) {
          throw new ClientError({ status: 422, message: `${id} has no option ${String(decision.option)}` });
        }
        const sha = fakeSha(id);
        const applied: StoredReview = { ...decided, status: "applied", applied_commit: sha, decision_note: decision.note };
        entry.proposals.splice(index, 1);
        return structuredClone({ proposal: applied, commit: { sha, subject: `spec: apply ${id}` } });
      }
      case "reject": {
        if (decision.reason.trim() === "") {
          throw new ClientError({ status: 422, message: "reject needs a non-empty reason" });
        }
        const rejected: StoredReview = { ...decided, status: "rejected", decision_note: decision.reason };
        entry.proposals.splice(index, 1);
        return structuredClone({ proposal: rejected, commit: null });
      }
      case "needs_clarification": {
        if (decision.note.trim() === "") {
          throw new ClientError({ status: 422, message: "needs_clarification needs a non-empty note" });
        }
        const sentBack: StoredReview = { ...decided, status: "changes_requested", decision_note: decision.note };
        entry.proposals[index] = { ...stored, review: sentBack };
        return structuredClone({ proposal: sentBack, commit: null });
      }
      case "defer": {
        const deferred: StoredReview = { ...decided, status: "deferred", decision_note: decision.note };
        entry.proposals[index] = { ...stored, review: deferred };
        return structuredClone({ proposal: deferred, commit: null });
      }
    }
  }

  /** No live tail: the mock's queue changes only by this page's decisions, which read again themselves. */
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
