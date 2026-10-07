import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { apiErrorOf, DECIDED_ELSEWHERE, type BundleOptions, type GraphOptions, type NodeOptions, type SearchOptions, type TreeOptions } from "./client";
import { useClient } from "./provider";
import type { Decision, Inbox, Proposal } from "./types";

/**
 * Query keys: every argument present, an absent one as `null`, `[]` or `false`, so two reads that
 * differ in any option are two cache entries (docs/features/ui-tree-node.md "Data").
 */
export const queryKeys = {
  projects: ["projects"] as const,
  inbox: (project: string) => ["inbox", project] as const,
  proposal: (project: string, id: string) => ["proposal", project, id] as const,
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
  graph: (project: string, options: GraphOptions) =>
    [
      "graph",
      project,
      {
        ref: options.ref,
        impact: options.impact ?? false,
        types: options.types ?? [],
        depth: options.depth ?? null,
        archive: options.archive ?? false,
      },
    ] as const,
  tasks: (project: string) => ["tasks", project] as const,
  task: (project: string, id: string) => ["task", project, id] as const,
  check: (project: string) => ["check", project] as const,
};

/**
 * The reads a decision can change, by their first key part: each is read again after one. A task's
 * open proposals and assumptions are the queue's (docs/features/ui-tasks.md "Data").
 */
const READS_AFTER_DECISION = ["inbox", "proposal", "tree", "node", "search", "bundle", "graph", "tasks", "task"] as const;

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

function graphRequest(options: GraphOptions): GraphOptions {
  const sent: GraphOptions = { ref: options.ref };
  if (options.impact === true) {
    sent.impact = true;
  }
  if (options.types !== undefined && options.types.length > 0) {
    sent.types = options.types;
  }
  if (options.depth !== undefined) {
    sent.depth = options.depth;
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

/** One proposal's review document (`id` null: nothing read); the Inbox's card and its decision read it. */
export function useProposal(project: string, id: string | null) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.proposal(project, id ?? ""),
    queryFn: () => client.getProposal(project, id ?? ""),
    enabled: id !== null,
  });
}

/** The review document's read as the Inbox shares it with its card. */
export type ProposalQuery = ReturnType<typeof useProposal>;

/** The proposal ID a queue event names: its payload's `id` (`docs/canon/proposal-queue.md` "States and events"). */
function eventProposalId(payload: unknown): string | null {
  if (typeof payload !== "object" || payload === null || !("id" in payload)) {
    return null;
  }
  return typeof payload.id === "string" ? payload.id : null;
}

/**
 * What an applied proposal changed besides the queue: a spec file, so the project's reads of the
 * spec (every tree, node, search, bundle and graph of it, and its check, each `[read, project, …]`;
 * `docs/features/ui-live.md` "Rules and edge cases").
 */
const READS_OF_THE_SPEC = ["tree", "node", "search", "bundle", "graph", "check"] as const;

/** The spec read that walks the whole corpus (`READS_OF_THE_SPEC`'s check). */
const WALK = "check";

/** The queue event of an apply: its commit wrote a spec file (`docs/canon/proposal-queue.md` "States and events"). */
const APPLIED = "proposal.applied";

/**
 * The project's live tail while `project` is set (`docs/features/daemon-read.md` "Data"): a
 * `proposal.*` event reads that project's inbox and that proposal again; `proposal.applied` also
 * that project's trees, nodes, searches, bundles, graphs and check (a spec file changed;
 * `docs/features/ui-live.md` "Data"), no other project's and no other read. A stream this client
 * opened again after the browser gave up on it (events may be missed, an apply among them) reads
 * the inbox, every cached proposal and those spec reads of the project again. Only the reads on
 * screen are fetched, once each (the check only while Health shows it); the rest are marked stale.
 */
export function useLiveQueue(project: string | null) {
  const client = useClient();
  const queryClient = useQueryClient();
  useEffect(() => {
    if (project === null) {
      return undefined;
    }
    const readSpecAgain = () => {
      // `[read, project]` matches that project's entries of the read whatever their options.
      // The check is a full walk: one already running (Health entered, Check again) is kept and its
      // answer taken, never cancelled for a second walk; any other read is read again at once.
      for (const read of READS_OF_THE_SPEC) {
        void queryClient.invalidateQueries({ queryKey: [read, project] }, { cancelRefetch: read !== WALK });
      }
    };
    return client.subscribe(
      project,
      (event) => {
        if (!event.type.startsWith("proposal.")) {
          return;
        }
        void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
        const id = eventProposalId(event.payload);
        if (id !== null) {
          void queryClient.invalidateQueries({ queryKey: queryKeys.proposal(project, id), exact: true });
        }
        if (event.type === APPLIED) {
          readSpecAgain();
        }
      },
      () => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
        void queryClient.invalidateQueries({ queryKey: ["proposal", project] });
        readSpecAgain();
      },
    );
  }, [client, queryClient, project]);
}

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

/** A node's read as a view holds it (the node pane's links read, shown by the Links tab). */
export type NodeQuery = ReturnType<typeof useNode>;

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

/**
 * One answer of `spec graph` (`options` null: no REF yet, nothing read). While another REF or
 * other options are read, the last answer stays up (the view marks it busy): the canvas never
 * blanks between two answers.
 */
export function useGraph(project: string, options: GraphOptions | null) {
  const client = useClient();
  const asked = options ?? { ref: "" };
  return useQuery({
    queryKey: queryKeys.graph(project, asked),
    queryFn: () => client.getGraph(project, graphRequest(asked)),
    enabled: options !== null,
    placeholderData: keepPreviousData,
  });
}

/** The project's tasks, one read for the whole list: filters never read again. */
export function useTasks(project: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.tasks(project), queryFn: () => client.getTasks(project) });
}

/** The task list's read as the view shares it (the list, the summary beside it). */
export type TasksQuery = ReturnType<typeof useTasks>;

/**
 * The palette's reads (docs/features/ui-home.md "Reads"): the cached answer as it is, never read
 * again by opening; read once when uncached or failed (`enabled` false: no project, nothing read).
 */
export function useCachedTasks(project: string, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.tasks(project),
    queryFn: () => client.getTasks(project),
    refetchOnMount: false,
    enabled,
  });
}

/** The Inbox as cached, for the palette: see useCachedTasks. */
export function useCachedInbox(project: string, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.inbox(project),
    queryFn: () => client.getInbox(project),
    refetchOnMount: false,
    enabled,
  });
}

/**
 * The palette's node search: one `search` per activation, the query as typed, nothing on a
 * keystroke; `reset` drops the answer (an edit), and a reply after it is never shown.
 */
export function useSearchOnActivation(project: string) {
  const client = useClient();
  return useMutation({
    mutationFn: (query: string) => client.search(project, { query }),
  });
}

/**
 * One task's package. No placeholder: a newly opened task never shows the previous one's package
 * while its own is read (docs/features/ui-tasks.md, States).
 */
export function useTask(project: string, id: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.task(project, id), queryFn: () => client.getTask(project, id) });
}

/**
 * The project's `spec check` (docs/features/ui-health.md "Data"): read on entering Health, on
 * "Check again" (`refetch`) and, while Health is on screen, after the live tail's apply or gap
 * (useLiveQueue); never on window focus, a reconnect or an interval, and not after a decision (a
 * full walk per read; the report carries no time).
 */
export function useCheck(project: string) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.check(project),
    queryFn: () => client.getCheck(project),
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    refetchInterval: false,
  });
}

/** The check's read as Health shares it among its regions. */
export type CheckQuery = ReturnType<typeof useCheck>;

/** Accept and reject close a proposal (06 §3.4): it leaves the inbox; the other two keep it there. */
function closes(decision: Decision): boolean {
  return decision.decision === "accept" || decision.decision === "reject";
}

/**
 * One decideProposal call per submit. On success a closed proposal (accepted, rejected) leaves the
 * cached inbox at once, never written back as applied or rejected; a kept one (needs clarification,
 * deferred) takes the daemon's returned state, and its review document is the returned one. After a
 * success or a 409 (decided elsewhere) the project's inbox, proposals, tree, nodes, searches,
 * bundles, graphs and tasks are read again: an apply may change any.
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
      queryClient.setQueryData<Proposal>(queryKeys.proposal(project, id), result.proposal);
      queryClient.setQueryData<Inbox>(inboxKey, (inbox) => {
        if (inbox === undefined) {
          return inbox;
        }
        const status = result.proposal.status;
        const proposals = closes(decision)
          ? inbox.proposals.filter((entry) => entry.id !== id)
          : inbox.proposals.map((entry) => (entry.id === id && status !== null ? { ...entry, status } : entry));
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
