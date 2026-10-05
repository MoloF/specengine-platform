import { vi } from "vitest";
import type { SpecEngineClient } from "../api/client";
import type { Decision, DecisionResult, Inbox, NodeView, Project, Proposal } from "../api/types";
import { aNode } from "./builders";

export const PROJECTS: Project[] = [
  { slug: "alpha", name: "Alpha" },
  { slug: "beta", name: "Beta" },
];

/**
 * A hand-made SpecEngineClient for tests: every method a vi.fn counting its calls, reading an
 * editable queue. Override any method with mockImplementation.
 */
export function stubClient(proposals: Proposal[] = [], notes: string[] = []) {
  const state = { proposals: [...proposals], notes };
  const client = {
    dataSource: "mock" as const,
    state,
    getProjects: vi.fn((): Promise<Project[]> => Promise.resolve(PROJECTS.map((project) => ({ ...project })))),
    getInbox: vi.fn(
      (): Promise<Inbox> => Promise.resolve({ proposals: state.proposals.map((p) => ({ ...p })), notes: [...state.notes] }),
    ),
    getNode: vi.fn(
      (_project: string, id: string): Promise<NodeView> =>
        Promise.resolve({ ref: id, reason: null, notes: [], nodes: [aNode({ id })] }),
    ),
    decideProposal: vi.fn((_project: string, id: string, decision: Decision): Promise<DecisionResult> => {
      const current = state.proposals.find((proposal) => proposal.id === id);
      if (current === undefined) {
        return Promise.reject(new Error(`no proposal ${id}`));
      }
      if (decision.decision === "accept") {
        state.proposals = state.proposals.filter((proposal) => proposal.id !== id);
        return Promise.resolve({
          proposal: { ...current, status: "applied", applied_commit: "c0ffee1" },
          commit: { sha: "c0ffee1", subject: `spec: apply ${id}` },
        });
      }
      if (decision.decision === "reject") {
        state.proposals = state.proposals.filter((proposal) => proposal.id !== id);
        return Promise.resolve({ proposal: { ...current, status: "rejected" }, commit: null });
      }
      const status = decision.decision === "defer" ? "deferred" : "changes_requested";
      const updated = { ...current, status };
      state.proposals = state.proposals.map((proposal) => (proposal.id === id ? updated : proposal));
      return Promise.resolve({ proposal: updated, commit: null });
    }),
  } satisfies SpecEngineClient & { state: unknown };
  return client;
}

export type StubClient = ReturnType<typeof stubClient>;
