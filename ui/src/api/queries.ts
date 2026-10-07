import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import {
  apiErrorOf,
  NO_SUCH_PROPOSAL,
  STAGE_REFUSED,
  type BundleOptions,
  type GraphOptions,
  type NodeOptions,
  type SearchOptions,
  type TreeOptions,
} from "./client";
import { endOwnWrite, isOwnStageEvent, outsideSince, startOwnWrite } from "./ownStages";
import { useClient } from "./provider";
import type { Inbox, Proposal, StageChoice } from "./types";

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

// What a request carries: absent options omitted (none at all: `undefined`), an empty array or a
// false `archive` too; then the query's AbortSignal. A read invalidated while in flight is aborted
// and read once more (TanStack's `cancelRefetch`), so a burst of live events costs the daemon one
// answered read per query, not one per event (`docs/features/ui-live-tasks.md` "Data"); the check
// alone never takes its signal (useCheck).

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
  return useQuery({ queryKey: queryKeys.projects, queryFn: ({ signal }) => client.getProjects(signal) });
}

export function useInbox(project: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.inbox(project), queryFn: ({ signal }) => client.getInbox(project, signal) });
}

/** The Inbox's read as a view shares it (the tree's counts, a node's Proposals tab). */
export type InboxQuery = ReturnType<typeof useInbox>;

/** One proposal's review document (`id` null: nothing read); the Inbox's card and its decision read it. */
export function useProposal(project: string, id: string | null) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.proposal(project, id ?? ""),
    queryFn: ({ signal }) => client.getProposal(project, id ?? "", signal),
    enabled: id !== null,
  });
}

/** The review document's read as the Inbox shares it with its card. */
export type ProposalQuery = ReturnType<typeof useProposal>;

/**
 * The ID a queue event names: its payload's `id`, a proposal's or a task's
 * (`docs/canon/proposal-queue.md` "States and events", `docs/canon/tasks.md` "Store"); null when
 * the payload has no string `id`.
 */
function eventId(payload: unknown): string | null {
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

/** A proposal's queue events (`docs/canon/proposal-queue.md` "States and events"). */
const PROPOSAL_EVENT = "proposal.";

/** The stage's two events (`docs/canon/decision-staging.md` "Queue"): the queue only, never a task or the spec. */
const STAGED = "proposal.staged";
const UNSTAGED = "proposal.unstaged";

/** A stage event's stored stage: its payload's `staged` (undefined when it has none). */
function stagedOf(payload: unknown): unknown {
  return typeof payload === "object" && payload !== null && "staged" in payload ? payload.staged : undefined;
}

/** A stage event's time: its payload's `staged_at`, a string; else null. */
function stagedAtOf(payload: unknown): string | null {
  if (typeof payload !== "object" || payload === null || !("staged_at" in payload)) {
    return null;
  }
  return typeof payload.staged_at === "string" ? payload.staged_at : null;
}

/**
 * A stage changed outside this tab, as the live tail said it (`docs/canon/decision-staging.md`
 * "UI"): a `proposal.staged` or `.unstaged` event no write of this tab accounts for.
 */
export interface OutsideStage {
  /** The project whose tail said it. */
  project: string;
  /** The proposal. */
  id: string;
  /** The new stage's time, as stored; null when the stage was removed. */
  stagedAt: string | null;
  /** The event's sequence number: each change its own alert. */
  seq: number;
}

/** A task's queue events (`docs/canon/tasks.md` "Store"). */
const TASK_EVENT = "task.";

/**
 * The project's live tail while `project` is set (`docs/features/daemon-read.md` "Data"): a
 * `proposal.*` event reads that project's inbox and that proposal again, and its task list and
 * every task of it (a task's open proposals and assumptions are the queue's); `proposal.applied`
 * also that project's trees, nodes, searches, bundles, graphs and check (a spec file changed;
 * `docs/features/ui-live.md` "Data"). A stage event, `proposal.staged` or `.unstaged`, reads only
 * that project's inbox and that proposal again: a stage touches no task and no spec file
 * (`docs/canon/decision-staging.md` "Queue"); one no write of this tab accounts for is returned as
 * the outside change to show, until `dismiss` or the next one. A `task.*` event reads that
 * project's task list and that task again (no string `id`: every task of it), never the inbox, a
 * proposal or a spec read (`docs/features/ui-live-tasks.md` "Data"). No other project's read, and
 * no other read. A stream this client opened again after the browser gave up on it (events may be
 * missed, an apply among them) reads the inbox, every cached proposal, those spec reads and the
 * task reads of the project again. Only the reads on screen are fetched, once each (the check only
 * while Health shows it); the rest are marked stale.
 */
export function useLiveQueue(project: string | null): { outside: OutsideStage | null; dismiss: () => void } {
  const client = useClient();
  const queryClient = useQueryClient();
  const [outside, setOutside] = useState<OutsideStage | null>(null);
  const dismiss = useCallback(() => {
    setOutside(null);
  }, []);
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
    /** The task list and every task of the project, whatever task is open. */
    const readTasksAgain = () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.tasks(project), exact: true });
      void queryClient.invalidateQueries({ queryKey: ["task", project] });
    };
    return client.subscribe(
      project,
      (event) => {
        const id = eventId(event.payload);
        if (event.type.startsWith(TASK_EVENT)) {
          if (id === null) {
            readTasksAgain();
          } else {
            void queryClient.invalidateQueries({ queryKey: queryKeys.tasks(project), exact: true });
            void queryClient.invalidateQueries({ queryKey: queryKeys.task(project, id), exact: true });
          }
          return;
        }
        if (!event.type.startsWith(PROPOSAL_EVENT)) {
          return;
        }
        void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
        if (id !== null) {
          void queryClient.invalidateQueries({ queryKey: queryKeys.proposal(project, id), exact: true });
        }
        if (event.type === STAGED || event.type === UNSTAGED) {
          const unstaged = event.type === UNSTAGED;
          if (id !== null && !isOwnStageEvent(queryClient, project, id, unstaged, stagedOf(event.payload))) {
            setOutside({ project, id, stagedAt: unstaged ? null : stagedAtOf(event.payload), seq: event.seq });
          }
          return;
        }
        readTasksAgain();
        if (event.type === APPLIED) {
          readSpecAgain();
        }
      },
      () => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
        void queryClient.invalidateQueries({ queryKey: ["proposal", project] });
        readSpecAgain();
        readTasksAgain();
      },
    );
  }, [client, queryClient, project]);
  return { outside: outside !== null && outside.project === project ? outside : null, dismiss };
}

/** The containment tree; while new options are read, the last tree stays up (no layout jump). */
export function useTree(project: string, options: TreeOptions) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.tree(project, options),
    queryFn: ({ signal }) => client.getTree(project, treeRequest(options), signal),
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
    queryFn: ({ signal }) => client.getNode(project, ref, nodeRequest(options), signal),
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
    queryFn: ({ signal }) => client.search(project, searchRequest(submitted), signal),
    enabled: options !== null,
  });
}

/** The context bundle of some nodes; read only once `enabled` (its tab opened). */
export function useBundle(project: string, options: BundleOptions, enabled: boolean) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.bundle(project, options),
    queryFn: ({ signal }) => client.getBundle(project, bundleRequest(options), signal),
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
    queryFn: ({ signal }) => client.getGraph(project, graphRequest(asked), signal),
    enabled: options !== null,
    placeholderData: keepPreviousData,
  });
}

/** The project's tasks, one read for the whole list: filters never read again. */
export function useTasks(project: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.tasks(project), queryFn: ({ signal }) => client.getTasks(project, signal) });
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
    queryFn: ({ signal }) => client.getTasks(project, signal),
    refetchOnMount: false,
    enabled,
  });
}

/** The Inbox as cached, for the palette: see useCachedTasks. */
export function useCachedInbox(project: string, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.inbox(project),
    queryFn: ({ signal }) => client.getInbox(project, signal),
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
  return useQuery({ queryKey: queryKeys.task(project, id), queryFn: ({ signal }) => client.getTask(project, id, signal) });
}

/**
 * The project's `spec check` (docs/features/ui-health.md "Data"): read on entering Health, on
 * "Check again" (`refetch`) and, while Health is on screen, after the live tail's apply or gap
 * (useLiveQueue); never on window focus, a reconnect or an interval, and not after a stage (a
 * full walk per read; the report carries no time). Its query's AbortSignal is never taken: a walk
 * in flight is kept and its answer used, never aborted, whatever reads it again or unmounts.
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

/**
 * A stage of one proposal as the Inbox sends it: the owner's choice with the review document's
 * `updated_at` as read, or, `stage` null, an unstage (`docs/canon/decision-staging.md` "UI").
 */
export type StageChange = { id: string; stage: StageChoice; updatedAt: string } | { id: string; stage: null };

/**
 * One stageDecision (or unstageDecision) call per submit; the stage is not a decision, so the
 * proposal stays in the inbox. On success the cached review document is the returned one and the
 * inbox entry keeps its place with the returned state and `staged_at`; after a success, a 409 (not
 * open, changed since read) or a 404, only that project's inbox and that proposal are read again:
 * a stage changes no task, no spec file and no check. Each write is noted as this tab's own, so its
 * event on the live tail is never shown as a change made outside this tab (useLiveQueue).
 */
export function useStageDecision(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  function readAgain(id: string) {
    void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
    void queryClient.invalidateQueries({ queryKey: queryKeys.proposal(project, id), exact: true });
  }
  /** This tab's read of the proposal shows a stage, and no outside change was heard since it was read. */
  function showsStage(id: string): boolean {
    const read = queryClient.getQueryState<Proposal>(queryKeys.proposal(project, id));
    return read?.data !== undefined && read.data.staged !== null && !outsideSince(queryClient, project, id, read.dataUpdatedAt);
  }
  return useMutation({
    mutationFn: (change: StageChange) =>
      change.stage === null ? client.unstageDecision(project, change.id) : client.stageDecision(project, change.id, change.stage, change.updatedAt),
    // A stage always logs its event; an unstage only when something is staged, which this tab
    // knows only from a current read (none: nothing awaited, see outsideSince).
    onMutate: (change) => (change.stage === null && !showsStage(change.id) ? null : startOwnWrite(queryClient, project, change.id, change.stage)),
    onSuccess: (review, { id }, write) => {
      if (write !== null) {
        endOwnWrite(queryClient, project, id, write, false);
      }
      queryClient.setQueryData<Proposal>(queryKeys.proposal(project, id), review);
      queryClient.setQueryData<Inbox>(queryKeys.inbox(project), (inbox) => {
        if (inbox === undefined) {
          return inbox;
        }
        const proposals = inbox.proposals.map((entry) =>
          entry.id === id ? { ...entry, status: review.status ?? entry.status, staged_at: review.staged_at } : entry,
        );
        return { ...inbox, proposals };
      });
      readAgain(id);
    },
    onError: (error, { id }, write) => {
      const status = apiErrorOf(error).status;
      if (write !== undefined && write !== null) {
        // No answer (status 0): the daemon may have stored it, so its event is still this tab's.
        endOwnWrite(queryClient, project, id, write, status !== 0);
      }
      if (status === STAGE_REFUSED || status === NO_SUCH_PROPOSAL) {
        readAgain(id);
      }
    },
  });
}
