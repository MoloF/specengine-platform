import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { apiErrorOf, DECIDED_ELSEWHERE, type BundleOptions, type NodeOptions, type SearchOptions, type TreeOptions } from "./client";
import { useClient } from "./provider";
import type { Decision, Inbox } from "./types";

/**
 * Query keys: every argument present, an absent one as `null`, `[]` or `false`, so two reads that
 * differ in any option are two cache entries (docs/features/ui-tree-node.md "Data").
 */
export const queryKeys = {
  projects: ["projects"] as const,
  inbox: (project: string) => ["inbox", project] as const,
  tree: (project: string, options: TreeOptions = {}) =>
    [
      "tree",
      project,
      {
        root: options.root ?? null,
        depth: options.depth ?? null,
        kinds: options.kinds ?? [],
        archive: options.archive ?? false,
      },
    ] as const,
  node: (project: string, ref: string, options: NodeOptions = {}) =>
    ["node", project, ref, { with: options.with ?? [], archive: options.archive ?? false }] as const,
  search: (project: string, options: SearchOptions) =>
    [
      "search",
      project,
      {
        query: options.query,
        kinds: options.kinds ?? [],
        limit: options.limit ?? null,
        archive: options.archive ?? false,
      },
    ] as const,
  bundle: (project: string, options: BundleOptions) =>
    ["bundle", project, { node_ids: options.node_ids, budget: options.budget ?? null }] as const,
};

/** The reads a decision can change, by their first key part: each is read again after one. */
const READS_AFTER_DECISION = ["inbox", "tree", "node", "search", "bundle"] as const;

// What a request carries: absent options omitted, an empty array or a false `archive` too.

function treeRequest(options: TreeOptions): TreeOptions | undefined {
  const sent: TreeOptions = {};
  if (options.root !== undefined) {
    sent.root = options.root;
  }
  if (options.depth !== undefined) {
    sent.depth = options.depth;
  }
  if (options.kinds !== undefined && options.kinds.length > 0) {
    sent.kinds = options.kinds;
  }
  if (options.archive === true) {
    sent.archive = true;
  }
  return Object.keys(sent).length === 0 ? undefined : sent;
}

function nodeRequest(options: NodeOptions): NodeOptions | undefined {
  const sent: NodeOptions = {};
  if (options.with !== undefined && options.with.length > 0) {
    sent.with = options.with;
  }
  if (options.archive === true) {
    sent.archive = true;
  }
  return Object.keys(sent).length === 0 ? undefined : sent;
}

function searchRequest(options: SearchOptions): SearchOptions {
  const sent: SearchOptions = { query: options.query };
  if (options.kinds !== undefined && options.kinds.length > 0) {
    sent.kinds = options.kinds;
  }
  if (options.limit !== undefined) {
    sent.limit = options.limit;
  }
  if (options.archive === true) {
    sent.archive = true;
  }
  return sent;
}

function bundleRequest(options: BundleOptions): BundleOptions {
  return options.budget === undefined ? { node_ids: options.node_ids } : { node_ids: options.node_ids, budget: options.budget };
}

export function useProjects() {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.projects, queryFn: () => client.getProjects() });
}

export function useInbox(project: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.inbox(project), queryFn: () => client.getInbox(project) });
}

/** The Inbox's read as a view shares it (the tree's counts, a node's Proposals tab). */
export type InboxQuery = ReturnType<typeof useInbox>;

/** The containment tree; while new options are read, the last tree stays up (no layout jump). */
export function useTree(project: string, options: TreeOptions) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.tree(project, options),
    queryFn: () => {
      const sent = treeRequest(options);
      return sent === undefined ? client.getTree(project) : client.getTree(project, sent);
    },
    placeholderData: keepPreviousData,
  });
}

/**
 * A node by REF; `enabled` false keeps it unread. When the options change (archive toggled) the
 * last answer stays up while the new one is read; a new REF is a new reader, never shown stale.
 */
export function useNode(project: string, ref: string, options: NodeOptions = {}, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.node(project, ref, options),
    queryFn: () => {
      const sent = nodeRequest(options);
      return sent === undefined ? client.getNode(project, ref) : client.getNode(project, ref, sent);
    },
    enabled,
    placeholderData: keepPreviousData,
  });
}

/** A search once submitted (`options` null before); a new query never shows the last one's hits. */
export function useSearch(project: string, options: SearchOptions | null) {
  const client = useClient();
  const submitted = options ?? { query: "" };
  return useQuery({
    queryKey: queryKeys.search(project, submitted),
    queryFn: () => client.search(project, searchRequest(submitted)),
    enabled: options !== null,
  });
}

/** The context bundle of some nodes; read only once `enabled` (its tab opened). */
export function useBundle(project: string, options: BundleOptions, enabled: boolean) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.bundle(project, options),
    queryFn: () => client.getBundle(project, bundleRequest(options)),
    enabled,
  });
}

/** Accept and reject close a proposal (06 §3.4): it leaves the inbox; the other two keep it there. */
function closes(decision: Decision): boolean {
  return decision.decision === "accept" || decision.decision === "reject";
}

/**
 * One decideProposal call per submit. On success a closed proposal (accepted, rejected) leaves the
 * cached inbox at once, never written back as applied or rejected; a kept one (needs clarification,
 * deferred) takes the daemon's returned state. After a success or a 409 (decided elsewhere) the
 * project's inbox, tree, nodes, searches and bundles are read again: an apply may change any.
 */
export function useDecideProposal(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  const inboxKey = queryKeys.inbox(project);
  function readAgain() {
    for (const read of READS_AFTER_DECISION) {
      void queryClient.invalidateQueries({ queryKey: [read, project] });
    }
  }
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
      readAgain();
    },
    onError: (error) => {
      if (apiErrorOf(error).status === DECIDED_ELSEWHERE) {
        readAgain();
      }
    },
  });
}
