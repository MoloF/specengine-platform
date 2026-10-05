import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { apiErrorOf, DECIDED_ELSEWHERE } from "./client";
import { useClient } from "./provider";
import type { Decision, Inbox } from "./types";

const keys = {
  projects: ["projects"] as const,
  inbox: (project: string) => ["inbox", project] as const,
  node: (project: string, id: string) => ["node", project, id] as const,
};

export function useProjects() {
  const client = useClient();
  return useQuery({ queryKey: keys.projects, queryFn: () => client.getProjects() });
}

export function useInbox(project: string) {
  const client = useClient();
  return useQuery({ queryKey: keys.inbox(project), queryFn: () => client.getInbox(project) });
}

export function useNode(project: string, id: string) {
  const client = useClient();
  return useQuery({ queryKey: keys.node(project, id), queryFn: () => client.getNode(project, id) });
}

/** Accept and reject close a proposal (06 §3.4): it leaves the inbox; the other two keep it there. */
function closes(decision: Decision): boolean {
  return decision.decision === "accept" || decision.decision === "reject";
}

/**
 * One decideProposal call per submit. On success a closed proposal (accepted, rejected) leaves the
 * cached inbox at once, never written back as applied or rejected; a kept one (needs clarification,
 * deferred) takes the daemon's returned state. Either way the inbox is read again. On 409 (decided
 * elsewhere) the inbox is read again.
 */
export function useDecideProposal(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  const inboxKey = keys.inbox(project);
  return useMutation({
    mutationFn: ({ id, decision }: { id: string; decision: Decision }) =>
      client.decideProposal(project, id, decision),
    onSuccess: (result, { id, decision }) => {
      queryClient.setQueryData<Inbox>(inboxKey, (inbox) => {
        if (inbox === undefined) {
          return inbox;
        }
        const proposals = closes(decision)
          ? inbox.proposals.filter((proposal) => proposal.id !== id)
          : inbox.proposals.map((proposal) => (proposal.id === id ? result.proposal : proposal));
        return { ...inbox, proposals };
      });
      void queryClient.invalidateQueries({ queryKey: inboxKey });
    },
    onError: (error) => {
      if (apiErrorOf(error).status === DECIDED_ELSEWHERE) {
        void queryClient.invalidateQueries({ queryKey: inboxKey });
      }
    },
  });
}
