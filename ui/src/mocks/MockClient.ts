import { ClientError, DECIDED_ELSEWHERE, type SpecEngineClient } from "../api/client";
import type { Decision, DecisionResult, Inbox, NodeView, Project, Proposal } from "../api/types";
import { stamp, type MockProject } from "./build";
import { harborSim } from "./harbor-sim/fixtures";
import { ledgerApi } from "./ledger-api/fixtures";
import { SLOW_MS, type Scenario } from "./scenario";

export interface MockOptions {
  /** The clock; fixture times are relative to it at construction. */
  now?: () => number;
  /** Delay of every call; `slow` defaults to SLOW_MS, the rest to none. */
  delayMs?: number;
}

const DECIDED_BY = "Mock Owner <owner@example.org>";

/** A stable 40-hex fake commit id for a proposal (FNV-1a over the text, repeated). */
function fakeSha(text: string): string {
  let hash = 0x811c9dc5;
  let out = "";
  for (let round = 0; out.length < 40; round += 1) {
    for (const char of `${text}:${round}`) {
      hash ^= char.charCodeAt(0);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    out += hash.toString(16).padStart(8, "0");
  }
  return out.slice(0, 40);
}

/**
 * The typed mock behind SpecEngineClient: two invented projects, their queues in memory
 * (decisions change them until reload), and the scenario picked at bootstrap.
 */
export class MockClient implements SpecEngineClient {
  readonly dataSource = "mock";
  readonly scenario: Scenario;
  private readonly projects: MockProject[];
  private readonly delayMs: number;
  private readonly now: () => number;

  constructor(scenario: Scenario, options: MockOptions = {}) {
    this.scenario = scenario;
    this.now = options.now ?? Date.now;
    this.delayMs = options.delayMs ?? (scenario === "slow" ? SLOW_MS : 0);
    const at = this.now();
    this.projects = [harborSim(at), ledgerApi(at)];
    if (scenario === "empty") {
      for (const project of this.projects) {
        project.proposals = [];
        project.notes = [];
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
    return structuredClone({ proposals: entry.proposals, notes: entry.notes });
  }

  async getNode(project: string, id: string): Promise<NodeView> {
    await this.read();
    const entry = this.project(project);
    const nodes = entry.nodes.filter((node) => node.id === id);
    return structuredClone({
      ref: id,
      reason: nodes.length === 0 ? `no node ${id} in ${project}` : null,
      notes: [],
      nodes,
    });
  }

  async decideProposal(project: string, id: string, decision: Decision): Promise<DecisionResult> {
    await this.wait();
    const entry = this.project(project);
    const index = entry.proposals.findIndex((proposal) => proposal.id === id);
    const current = entry.proposals[index];
    if (current === undefined) {
      throw new ClientError({ status: 404, message: `no open proposal ${id} in ${project}` });
    }
    if (this.scenario === "conflict") {
      entry.proposals.splice(index, 1);
      throw new ClientError({
        status: DECIDED_ELSEWHERE,
        message: `${id} is no longer open: another session applied it as ${fakeSha(`${id}:elsewhere`).slice(0, 7)}`,
      });
    }
    const decided: Proposal = {
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
          throw new ClientError({ status: 422, message: `${id} has no option ${decision.option}` });
        }
        const sha = fakeSha(id);
        const applied: Proposal = { ...decided, status: "applied", applied_commit: sha, decision_note: decision.note };
        entry.proposals.splice(index, 1);
        return structuredClone({ proposal: applied, commit: { sha, subject: `spec: apply ${id}` } });
      }
      case "reject": {
        if (decision.reason.trim() === "") {
          throw new ClientError({ status: 422, message: "reject needs a non-empty reason" });
        }
        const rejected: Proposal = { ...decided, status: "rejected", decision_note: decision.reason };
        entry.proposals.splice(index, 1);
        return structuredClone({ proposal: rejected, commit: null });
      }
      case "needs_clarification": {
        if (decision.note.trim() === "") {
          throw new ClientError({ status: 422, message: "needs_clarification needs a non-empty note" });
        }
        const sentBack: Proposal = { ...decided, status: "changes_requested", decision_note: decision.note };
        entry.proposals[index] = sentBack;
        return structuredClone({ proposal: sentBack, commit: null });
      }
      case "defer": {
        const deferred: Proposal = { ...decided, status: "deferred", decision_note: decision.note };
        entry.proposals[index] = deferred;
        return structuredClone({ proposal: deferred, commit: null });
      }
    }
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

  private project(slug: string): MockProject {
    const entry = this.projects.find((candidate) => candidate.project.slug === slug);
    if (entry === undefined) {
      throw new ClientError({ status: 404, message: `no project ${slug}` });
    }
    return entry;
  }
}
