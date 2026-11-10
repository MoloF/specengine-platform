import { keepPreviousData, useIsMutating, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import {
  apiErrorOf,
  CARD_REFUSED,
  NO_SUCH_CARD,
  NO_SUCH_PROPOSAL,
  STAGE_REFUSED,
  type BundleOptions,
  type CardsOptions,
  type FlowOptions,
  type GraphOptions,
  type NodeOptions,
  type SearchOptions,
  type TraceOptions,
  type TreeOptions,
} from "./client";
import { endOwnWrite, isOwnStageEvent, outsideSince, startOwnWrite, type OwnWrite } from "./ownStages";
import { useClient } from "./provider";
import type {
  CardAcceptBody,
  CardAssignBody,
  CardCancelBody,
  CardClearBody,
  CardClearGroupBody,
  CardCreateBody,
  CardEditBody,
  CardMoveBody,
  CardWrite,
  ConfirmBody,
  FlowView,
  Inbox,
  Proposal,
  RoundAnswerBody,
  RoundAnswersOutcome,
  StageChoice,
  TreeView,
} from "./types";

/**
 * Query keys: every argument present, an absent one as `null`, `[]` or `false`, so two reads that
 * differ in any option are two cache entries (docs/features/ui-tree-node.md "Data").
 */
export const queryKeys = {
  projects: ["projects"] as const,
  /** Who this page acts as (`docs/canon/hub.md` "Routes"): no project, a fact of the page. */
  identity: ["identity"] as const,
  /** What `[team]` says of this login, per project (`docs/features/hub-team.md` "Data"). */
  team: (project: string) => ["team", project] as const,
  inbox: (project: string) => ["inbox", project] as const,
  proposal: (project: string, id: string) => ["proposal", project, id] as const,
  /** What a terminal would ask of a staged proposal (`docs/features/hub-confirm.md` "Data"): no event reads it, its dialog alone. */
  confirmRead: (project: string, id: string) => ["confirm", project, id] as const,
  /** A `--push` hub's push status of the project's repository (`docs/features/hub-git.md` "Data"): no event reads it. */
  sync: (project: string) => ["sync", project] as const,
  /** A project's workspaces and modules (`docs/features/workspaces.md` "Data"): its config alone, so no event reads it. */
  workspaces: (project: string) => ["workspaces", project] as const,
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
  /** One read of `spec flow`: `root` null at the top, `stage` null for all (`docs/features/writer-studio.md` D2), its badges with it (`docs/features/trace-views.md` "Flow badges"), cut to the flow or whole (`docs/features/flow-scenario.md` D6). */
  flow: (project: string, options: FlowOptions) => ["flow", project, options.root ?? null, options.depth ?? null, options.trace ?? false, options.stage ?? null, options.in_flow ?? false] as const,
  tasks: (project: string) => ["tasks", project] as const,
  task: (project: string, id: string) => ["task", project, id] as const,
  check: (project: string) => ["check", project] as const,
  /** `spec health --json`, rows 3 to 6 of What is left (`docs/features/health-left.md`, `docs/features/health-code-ui.md`, `docs/features/health-drift.md` "Data"). */
  health: (project: string) => ["health", project] as const,
  /** The round's sheet; `limit` null: the daemon's default (`docs/features/ui-round.md` "Data"). */
  round: (project: string, limit: number | null) => ["round", project, { limit }] as const,
  /** The whole plan, `spec roadmap --json` with neither option (`docs/features/roadmap-screen.md`). */
  roadmap: (project: string) => ["roadmap", project] as const,
  /** What moved since each drifted binding's lock entry, `spec lock diff REF --json` (`docs/features/lock-diff-ui.md` "Description and interactions"). */
  lockDiff: (project: string, ref: string) => ["lock-diff", project, ref] as const,
  /** The Board's read (`docs/features/board-ui.md` "Board"): the filters never read again, Show cancelled does; `parent` is never asked here. */
  cards: (project: string, options: BoardCardsOptions = {}) => ["cards", project, { with_cancelled: options.with_cancelled ?? false }] as const,
  /** One card, its history with it. */
  card: (project: string, id: string) => ["card", project, id] as const,
  /** Marks and rollups, joined per read; `ref` null: the whole project (`docs/features/card-trace.md`); a lane's rows or the lanes alone (`docs/features/trace-views.md` "Timeline"). */
  trace: (project: string, options: TraceOptions = {}) => ["trace", project, options.ref ?? null, options.stage ?? null, options.summary ?? false] as const,
};

/** What the Board's one read asks: Show cancelled alone. It finds a card's children in that answer, never by `parent`. */
export type BoardCardsOptions = Pick<CardsOptions, "with_cancelled">;

/** Every round sheet of a project, whatever its limit. */
function roundsOf(project: string) {
  return ["round", project] as const;
}

/** A project's round POSTs in the mutation cache: one key, whatever screen sent them. */
function roundAnswersKey(project: string) {
  return ["round-answers", project] as const;
}

/** A project's card writes in the mutation cache: one key, whichever dialog, menu or drop sent them. */
function cardWritesKey(project: string) {
  return ["card-writes", project] as const;
}

/** Every read of a project's card list, whatever its options. */
function cardListsOf(project: string) {
  return ["cards", project] as const;
}

/** Every card read of a project. */
function cardsOf(project: string) {
  return ["card", project] as const;
}

/** Every trace read of a project, whatever its REF. */
function tracesOf(project: string) {
  return ["trace", project] as const;
}

/** Every flow read of a project: each carries its boxes' badges. */
function flowsOf(project: string) {
  return ["flow", project] as const;
}

// What a request carries: absent options omitted (none at all: `undefined`), an empty array or a
// false `archive` too; then the query's AbortSignal. A read invalidated while in flight is aborted
// and read once more (TanStack's `cancelRefetch`), so a burst of live events costs the daemon one
// answered read per query, not one per event (`docs/features/ui-live-tasks.md` "Data"); the two
// walks, the check and the health read, never take their signal (useCheck, useHealth).

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

/** The options asked, an absent or empty one left out; none at all: `undefined`. */
function traceRequest(options: TraceOptions): TraceOptions | undefined {
  const sent: TraceOptions = {};
  if (options.ref !== undefined) {
    sent.ref = options.ref;
  }
  if (options.stage !== undefined) {
    sent.stage = options.stage;
  }
  if (options.summary === true) {
    sent.summary = true;
  }
  return Object.keys(sent).length === 0 ? undefined : sent;
}

function bundleRequest(options: BundleOptions): BundleOptions {
  return options.budget === undefined ? { node_ids: options.node_ids } : { node_ids: options.node_ids, budget: options.budget };
}

export function useProjects() {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.projects, queryFn: ({ signal }) => client.getProjects(signal) });
}

/**
 * Who this page acts as (`docs/canon/hub.md` "Routes"): one read for every field that asks, kept for
 * the page, so opening a dialog reads nothing and a login once resolved never goes back to loading.
 * A failed read keeps no answer: the next field to mount asks again.
 */
export function useIdentity() {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.identity, queryFn: ({ signal }) => client.getIdentity(signal), staleTime: Infinity, gcTime: Infinity });
}

/**
 * This login's row of the project's `[team]` (`docs/features/hub-team.md` "Data"): a hub's route, so
 * unread until `enabled`, a login known. One read per project however many fields ask; a failed one
 * is asked again by the next to mount, as the identity's. `[team]` changes by a commit, which raises
 * no event, so window focus reads it again.
 */
export function useTeam(project: string, enabled: boolean) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.team(project), queryFn: ({ signal }) => client.getTeam(project, signal), enabled, refetchOnMount: false, refetchOnWindowFocus: true });
}

/** How often the shell asks a hub's push status while the page has focus (`docs/features/hub-git.md` "Status and the screen"). */
export const SYNC_EVERY_MS = 30_000;

/**
 * The push status of the project's repository (`docs/features/hub-git.md` "Status and the screen"):
 * a hub's route, so unread until `enabled`, a login known; then on window focus and every
 * SYNC_EVERY_MS while the page is in front. A sync raises no event, so nothing else reads it again.
 * A 404 (a local daemon's or a hub's without `--push`) and any other failure reject: the read says
 * nothing then, and the next tick or focus asks again.
 */
export function useSync(project: string, enabled: boolean) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.sync(project),
    queryFn: ({ signal }) => client.getSync(project, signal),
    enabled,
    refetchOnWindowFocus: true,
    refetchInterval: SYNC_EVERY_MS,
  });
}

/**
 * The project's workspaces and modules (`docs/features/workspaces.md` "The UI"): the shell's one read,
 * unread until `enabled`, a project known. The config changes by an edit or a commit, which raises
 * no event, so window focus reads it again; nothing else does.
 */
export function useWorkspaces(project: string, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.workspaces(project),
    queryFn: ({ signal }) => client.getWorkspaces(project, signal),
    enabled,
    refetchOnWindowFocus: true,
  });
}

/** The workspaces read as the shell shares it. */
export type WorkspacesQuery = ReturnType<typeof useWorkspaces>;

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
 * spec (every tree, node, search, bundle, graph and flow of it, its check, its unapplied amendments,
 * its plan, its lock diffs and its trace, each `[read, project, …]`;
 * `docs/features/ui-live.md` "Rules and edge cases",
 * `docs/features/health-left.md` "Description and interactions",
 * `docs/features/roadmap-screen.md`: an item's file is a spec file; `docs/features/lock-diff-ui.md`:
 * a lock diff's spec side is the node's text against its entry's; `docs/features/card-trace.md`: a
 * mark is a card against the node's text, which `docs/features/trace-views.md` puts on each row of
 * the card list).
 */
const READS_OF_THE_SPEC = ["tree", "node", "search", "bundle", "graph", "flow", "check", "health", "roadmap", "lock-diff", "trace", "cards"] as const;

/** The spec reads that walk the whole corpus, kept when in flight (`READS_OF_THE_SPEC`'s check; the health read: readHealthAgain). */
const WALKS: ReadonlySet<string> = new Set(["check"]);

/** The queue event of an apply: its commit wrote a spec file (`docs/canon/proposal-queue.md` "States and events"). */
const APPLIED = "proposal.applied";

/**
 * The queue events after which the health read alone is read again, as after an apply
 * (`docs/canon/health.md` "UI"): no spec file changes, but row 4 moves. A rejection makes code
 * resting on that item `rejected`; an approval, logged inside an apply, moves `pending` to
 * `as_assumed` or `otherwise`; an apply refused at a later step may reopen the item
 * (`approved -> open`, `docs/canon/proposal-queue.md` "States and events").
 */
const HEALTH_EVENTS: ReadonlySet<string> = new Set(["proposal.approved", "proposal.rejected", "proposal.apply_failed"]);

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

/** A card's queue events (`docs/canon/cards.md` "Documents and events"). */
const CARD_EVENT = "card.";

/**
 * The project's live tail while `project` is set (`docs/features/daemon-read.md` "Data"): a
 * `proposal.*` event reads that project's inbox and that proposal again, and its task list and
 * every task of it (a task's open proposals and assumptions are the queue's); `proposal.applied`
 * also that project's trees, nodes, searches, bundles, graphs, check, unapplied amendments and lock
 * diffs (a spec file changed; `docs/features/ui-live.md` "Data"); `proposal.approved`, `.rejected` and
 * `.apply_failed` also that project's health read, which row 4 shows (`docs/canon/health.md` "UI").
 * A health read in flight when one of these, an apply or a gap lands is kept, never aborted, and
 * exactly one more is read when it settles, answered or failed, however many landed. A stage
 * event, `proposal.staged` or `.unstaged`, reads only that project's inbox and that proposal
 * again: a stage touches no task and no spec file (`docs/canon/decision-staging.md` "Queue"); one
 * no write of this tab accounts for is returned as the outside change to show, until `dismiss` or
 * the next one. A `task.*` event reads that project's task list and that task again (no string
 * `id`: every task of it), never the inbox, a proposal or a spec read
 * (`docs/features/ui-live-tasks.md` "Data"). Every `proposal.*` and `task.*` event also reads that
 * project's round sheets again, a read in flight aborted
 * (`docs/features/ui-round.md` "Description and interactions"). The plan is read again on a
 * `task.*` event, `proposal.applied` and a gap, a read in flight aborted, never on a stage,
 * `.created`, `.approved`, `.rejected` or `.apply_failed` (`docs/features/roadmap-screen.md`). A
 * `task.*` event also reads the project's cards again (the open one; a card serves each task it
 * links as read now, `docs/canon/card-tasks.md` "The rule"), never the card list, which shows no
 * task; `proposal.applied` reads them too, an apply moving a linked task's staleness. A `card.*`
 * event reads that project's card lists, that card (no string `id`: every card of it) and
 * its trace again, never anything else and with no outside-change alert: a card is no decision
 * (`docs/features/board-ui.md` "Rules and edge cases"); it reads the project's flows too, whose boxes
 * carry their rollups (`docs/features/trace-views.md` "Timeline"). A rejected discrepancy settles a
 * departure, so `.approved`, `.rejected` and `.apply_failed` read the trace, the flows and the card
 * lists too (`docs/features/card-trace.md` "Rollup"). No
 * other project's read, and no other read. A stream this client opened again after the browser gave up on it (events may be
 * missed, an apply among them) reads the inbox, every cached proposal, those spec reads, the task
 * reads, the round sheets and the cards of the project again. Only the reads on screen are fetched, once each
 * but the health read's one more after a walk (the check and the health read only while Health
 * shows them: `docs/features/health-left.md` "Description and interactions"; the health read also
 * while the Spec tree shows its drift: `docs/features/ui-tree-drift.md`); the rest are marked
 * stale.
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
    // Whether one more health read is owed once the walk in flight settles: one, however many
    // events land during it; dropped with the subscription (entering Health reads anyway).
    let healthOwed = false;
    // The health read, a walk of the whole tree (`docs/canon/health.md` "UI"): none in flight, read
    // at once (on screen; off screen it is only marked stale and entering Health reads it). One in
    // flight (Health entered, Check again, an earlier event) is kept, never aborted, its answer
    // taken; it may predate the event, so exactly one more is read after it settles, answered or
    // failed, active only. No timer: the walk's own settling sends the next.
    const readHealthAgain = () => {
      const key = queryKeys.health(project);
      const walk = queryClient.getQueryCache().find({ queryKey: key, exact: true })?.promise;
      void queryClient.invalidateQueries({ queryKey: key, exact: true }, { cancelRefetch: false });
      if (walk === undefined || healthOwed) {
        return;
      }
      healthOwed = true;
      const after = () => {
        if (healthOwed) {
          healthOwed = false;
          void queryClient.invalidateQueries({ queryKey: key, exact: true }, { cancelRefetch: false });
        }
      };
      void walk.then(after, after);
    };
    const readSpecAgain = () => {
      // `[read, project]` matches that project's entries of the read whatever their options.
      // The check and the health read are full walks: one already running (Health entered, Check
      // again) is kept and its answer taken, never cancelled for a second walk (the health read
      // then read once more, readHealthAgain); any other read is read again at once.
      for (const read of READS_OF_THE_SPEC) {
        if (read === "health") {
          readHealthAgain();
        } else {
          void queryClient.invalidateQueries({ queryKey: [read, project] }, { cancelRefetch: !WALKS.has(read) });
        }
      }
    };
    // The plan lists each open task beside the item it executes, or among the pool tasks: a task
    // event reads it again, a read in flight aborted (`docs/features/roadmap-screen.md`); an apply
    // and a gap read it as a read of the spec. No other event: a stage, a new proposal, an
    // approval, a rejection or a refused apply changes no item file and no task.
    const readPlanAgain = () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.roadmap(project), exact: true });
    };
    /** The task list and every task of the project, whatever task is open. */
    const readTasksAgain = () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.tasks(project), exact: true });
      void queryClient.invalidateQueries({ queryKey: ["task", project] });
    };
    // The question round's sheet lists the open questions and discrepancies, their stages and the
    // tasks waiting on them: any proposal or task event, and a gap, read it again; a read in flight
    // is aborted for the new one (`docs/features/ui-round.md` "Description and interactions").
    const readRoundAgain = () => {
      void queryClient.invalidateQueries({ queryKey: roundsOf(project) });
    };
    /** Marks and rollups: a read of the project's cards against its spec, wherever they show. */
    const readMarksAgain = () => {
      void queryClient.invalidateQueries({ queryKey: tracesOf(project) });
      void queryClient.invalidateQueries({ queryKey: flowsOf(project) });
      void queryClient.invalidateQueries({ queryKey: cardListsOf(project) });
    };
    const unsubscribe = client.subscribe(
      project,
      (event) => {
        const id = eventId(event.payload);
        if (event.type.startsWith(CARD_EVENT)) {
          if (id === null) {
            void queryClient.invalidateQueries({ queryKey: cardsOf(project) });
          } else {
            void queryClient.invalidateQueries({ queryKey: queryKeys.card(project, id), exact: true });
          }
          readMarksAgain();
          return;
        }
        if (event.type.startsWith(TASK_EVENT) || event.type.startsWith(PROPOSAL_EVENT)) {
          readRoundAgain();
        }
        if (event.type.startsWith(TASK_EVENT)) {
          if (id === null) {
            readTasksAgain();
          } else {
            void queryClient.invalidateQueries({ queryKey: queryKeys.tasks(project), exact: true });
            void queryClient.invalidateQueries({ queryKey: queryKeys.task(project, id), exact: true });
          }
          readPlanAgain();
          // A card reads each task it links now: the card on screen is read again, whichever task
          // this is (no read names which cards link it); the list shows no task and stays.
          void queryClient.invalidateQueries({ queryKey: cardsOf(project) });
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
          // An apply may move a linked task's `stale`, which a card serves as read now: the card on
          // screen is read again. The list is a spec read above, for its marks, and shows no task.
          void queryClient.invalidateQueries({ queryKey: cardsOf(project) });
        }
        if (HEALTH_EVENTS.has(event.type)) {
          readHealthAgain();
          readMarksAgain();
        }
      },
      () => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
        void queryClient.invalidateQueries({ queryKey: ["proposal", project] });
        readSpecAgain();
        readTasksAgain();
        readRoundAgain();
        void queryClient.invalidateQueries({ queryKey: cardsOf(project) });
      },
    );
    return () => {
      healthOwed = false;
      unsubscribe();
    };
  }, [client, queryClient, project]);
  return { outside: outside !== null && outside.project === project ? outside : null, dismiss };
}

/**
 * The containment tree; while new options are read, the last tree stays up (no layout jump).
 * `enabled` false keeps it unread: the Spec tree waits for its module's root
 * (`docs/features/workspaces.md` "The UI").
 */
export function useTree(project: string, options: TreeOptions, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.tree(project, options),
    queryFn: ({ signal }) => client.getTree(project, treeRequest(options), signal),
    enabled,
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

/**
 * A node's code bindings, `nodes/<REF>?with=bindings`:
 * `docs/canon/code-reads.md` "spec show --bindings". The Bindings tab's one read, made when it
 * mounts on the tab's first opening. Its key is the node's, `["node", project, ref, {with:
 * ["bindings"], archive: false}]`, so an apply and a gap read it again with the project's other
 * node reads; no other event does (a code edit fires none: the tab's Read again).
 */
export function useNodeBindings(project: string, ref: string) {
  return useNode(project, ref, { with: ["bindings"] });
}

/**
 * What moved since each drifted binding of a node was accepted, `lock-diff?ref=<REF>`
 * (`docs/canon/spec-lock-diff.md` "The read"): the Bindings tab's "Changes since the lock", read
 * only once `enabled` (its first expanding), never when the tab opens; then kept while the pane
 * shows the node. Its key, `["lock-diff", project, ref]`, is a read of the spec: an apply and a gap
 * read it again; no other event does (a code edit fires none: the panel's Read again).
 */
export function useLockDiff(project: string, ref: string, enabled: boolean) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.lockDiff(project, ref),
    queryFn: ({ signal }) => client.getLockDiff(project, ref, signal),
    enabled,
  });
}

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

/**
 * The three forms a Flow picture reads (`docs/features/writer-studio.md` D2), always with `trace` and
 * cut to the flow (`docs/features/flow-scenario.md` D6): the top `depth=0` with no root, a scope
 * `root=R&depth=1`, a stage of a scope `root=R&stage=S`. A stage read always carries its scope's
 * root: without one it would admit every document of the stage, a whole read in payload.
 */
export function flowForm(root: string | null, stage: string | null): FlowOptions {
  if (root === null) {
    return { depth: 0, in_flow: true, trace: true };
  }
  return stage === null ? { root, depth: 1, in_flow: true, trace: true } : { root, stage, in_flow: true, trace: true };
}

/**
 * The Records for the AI's read (`docs/features/flow-scenario.md` D6): the scope's read uncut and
 * without trace, made only while the dialog is open; its `in_flow: false` nodes are the list.
 */
export function recordsForm(root: string): FlowOptions {
  return { root, depth: 1 };
}

/**
 * The options of a Flow reading key, as the studio holds them: a place read before draws from its
 * kept answer, read again on a live event that marks it or on the next entry to the screen
 * (`docs/features/writer-studio.md` "The reads"). A key changing under a view reads nothing it
 * holds; a hand edit, which raises no event, shows on coming back.
 */
const KEPT_READ = { staleTime: Infinity, refetchOnMount: "always" } as const;

/**
 * One Flow picture's read, a `flowForm`. `enabled` false reads nothing and still says what is held:
 * the lens root is told from the top read already in hand.
 */
export function useFlowRead(project: string, options: FlowOptions, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.flow(project, options),
    queryFn: ({ signal }) => client.getFlow(project, options, signal),
    enabled,
    ...KEPT_READ,
  });
}

/** A Flow read as a view shares it. */
export type FlowQuery = ReturnType<typeof useFlowRead>;

/**
 * The Records for the AI's list, `recordsForm(root)`, read only while the dialog is open: a live
 * mark reads it again only then (`docs/canon/spec-flow-screen.md` "Reads"; RM-0094).
 */
export function useRecordsRead(project: string, root: string, enabled: boolean) {
  const client = useClient();
  const options = recordsForm(root);
  return useQuery({
    queryKey: queryKeys.flow(project, options),
    queryFn: ({ signal }) => client.getFlow(project, options, signal),
    enabled,
  });
}

/**
 * A scope's lanes, `spec trace REF --summary` (`docs/canon/spec-trace.md` "The read"), by the
 * scope's walk; never unrooted, one per scope, kept as a picture's read is.
 */
export function useScopeStages(project: string, ref: string, enabled = true) {
  const client = useClient();
  const options: TraceOptions = { ref, summary: true };
  return useQuery({
    queryKey: queryKeys.trace(project, options),
    queryFn: ({ signal }) => client.getTrace(project, options, signal),
    enabled,
    ...KEPT_READ,
  });
}

/** A scope's own text and links, `nodes/<REF>?with=links`, kept as a picture's read is: the panel shows it with nothing selected. */
export function useScopeNode(project: string, ref: string, enabled = true) {
  const client = useClient();
  const options: NodeOptions = { with: ["links"] };
  return useQuery({
    queryKey: queryKeys.node(project, ref, options),
    queryFn: ({ signal }) => client.getNode(project, ref, nodeRequest(options), signal),
    enabled,
    ...KEPT_READ,
  });
}

/**
 * The whole tree, on demand: the Flow asks it only for the `parent` of a name no read holds, never to
 * draw (`docs/features/writer-studio.md` R3). The Tree screen's entry, read again only once a live
 * event marks it.
 */
export function useTreeOnDemand(project: string): () => Promise<TreeView> {
  const client = useClient();
  const queryClient = useQueryClient();
  return useCallback(
    () =>
      queryClient.query({
        queryKey: queryKeys.tree(project, {}),
        queryFn: ({ signal }) => client.getTree(project, undefined, signal),
        staleTime: Infinity,
      }),
    [client, project, queryClient],
  );
}

/**
 * The Flow's top read on demand, the one `useFlowRead` keeps: a seek whose lens root is not known yet
 * (the hash named a scope, so the top was not read) finds it, so a fix never writes the lens root as
 * a scope (`docs/features/writer-studio.md` R3). Held, it is not read again.
 */
export function useTopOnDemand(project: string): () => Promise<FlowView> {
  const client = useClient();
  const queryClient = useQueryClient();
  return useCallback(() => {
    const options = flowForm(null, null);
    return queryClient.query({
      queryKey: queryKeys.flow(project, options),
      queryFn: ({ signal }) => client.getFlow(project, options, signal),
      staleTime: Infinity,
    });
  }, [client, project, queryClient]);
}

/** The project's tasks, one read for the whole list: filters never read again. */
export function useTasks(project: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.tasks(project), queryFn: ({ signal }) => client.getTasks(project, signal) });
}

/** The task list's read as the view shares it (the list, the summary beside it). */
export type TasksQuery = ReturnType<typeof useTasks>;

/**
 * The palette's reads (`docs/features/ui-home.md` "Data"): the cached answer as it is, never read
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
 * The project's `spec health` (rows 3 to 6 of What is left; `docs/canon/health.md` "UI"): on
 * entering Health, on "Check again" (`refetch`) and, while Health is on screen, after the live
 * tail's apply, approval, rejection, refused apply or gap (useLiveQueue); never on window focus, a
 * reconnect, an interval, a stage, `proposal.created` or a `task.*` event. A walk of the whole
 * tree: its query's AbortSignal is never taken, a walk in flight is kept and its answer used, then
 * read once more when an event landed during it. The home reads none. The Spec tree observes the
 * same read while its "Show drift" is on and its tree has answered (`enabled`, true by default;
 * docs/features/ui-tree-drift.md "Description and interactions"): off, it reads nothing and no
 * event reads it for the tree.
 */
export function useHealth(project: string, enabled = true) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.health(project),
    queryFn: () => client.getHealth(project),
    enabled,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    refetchInterval: false,
  });
}

/** The health read as Health shares it: What is left shows it, Check again reads it again. */
export type HealthQuery = ReturnType<typeof useHealth>;

/**
 * The stage a screen shows for a proposal, as its read gave it: whether one was staged, and when
 * that read was answered (the query's `dataUpdatedAt`). The Questions screen shows the round
 * sheet's, not the review document's (`docs/features/ui-round.md` "Description and interactions").
 */
export interface ShownStage {
  staged: boolean;
  readAt: number;
}

/**
 * A stage of one proposal as a screen sends it: the owner's choice with the `updated_at` as read,
 * or, `stage` null, an unstage (`docs/canon/decision-staging.md` "UI"); `shown` the stage the
 * screen showed when it was not the review document's.
 */
export type StageChange = { id: string; stage: StageChoice; updatedAt: string } | { id: string; stage: null; shown?: ShownStage };

/**
 * One stageDecision (or unstageDecision) call per submit; the stage is not a decision, so the
 * proposal stays in the inbox. On success the cached review document is the returned one and the
 * inbox entry keeps its place with the returned state and `staged_at`; after a success, a 409 (not
 * open, changed since read) or a 404, only that project's inbox, that proposal and its round sheets
 * are read again: a stage changes no task, no spec file and no check. Each write is noted as this
 * tab's own, so its event on the live tail is never shown as a change made outside this tab
 * (useLiveQueue); an unstage only when the stage the screen shows (`shown`, else the cached review
 * document) was staged and no outside change was heard since that read.
 */
export function useStageDecision(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  function readAgain(id: string) {
    void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
    void queryClient.invalidateQueries({ queryKey: queryKeys.proposal(project, id), exact: true });
    void queryClient.invalidateQueries({ queryKey: roundsOf(project) });
  }
  /**
   * The screen's read of the proposal shows a stage, and no outside change was heard since it was
   * read: `shown` when the screen says what it shows, else this tab's cached review document.
   */
  function showsStage(id: string, shown: ShownStage | undefined): boolean {
    if (shown !== undefined) {
      return shown.staged && !outsideSince(queryClient, project, id, shown.readAt);
    }
    const read = queryClient.getQueryState<Proposal>(queryKeys.proposal(project, id));
    return read?.data !== undefined && read.data.staged !== null && !outsideSince(queryClient, project, id, read.dataUpdatedAt);
  }
  return useMutation({
    mutationFn: (change: StageChange) =>
      change.stage === null ? client.unstageDecision(project, change.id) : client.stageDecision(project, change.id, change.stage, change.updatedAt),
    // A stage always logs its event; an unstage only when something is staged, which this tab
    // knows only from a current read (none: nothing awaited, see outsideSince).
    onMutate: (change) =>
      change.stage === null && !showsStage(change.id, change.shown) ? null : startOwnWrite(queryClient, project, change.id, change.stage),
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

/**
 * What a terminal would ask of a staged proposal (`docs/features/hub-confirm.md` "Data"): a hub's
 * read, asked when a Confirm dialog opens and kept as read while it stands, whatever the live tail
 * says, so the words on screen are the words the POST sends; only the dialog's Read again asks
 * again. A closed dialog keeps no answer: the next opening asks anew.
 */
export function useConfirmRead(project: string, id: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.confirmRead(project, id), queryFn: ({ signal }) => client.getConfirm(project, id, signal), staleTime: Infinity, gcTime: 0 });
}

/** One Confirm as a dialog sends it: the proposal, and the read's `question` and `updated_at`. */
export interface ConfirmRequest {
  id: string;
  body: ConfirmBody;
}

/**
 * One confirmProposal call per click (`docs/features/hub-confirm.md` "Description and interactions"):
 * nothing is optimistic. After an answer, a refusal (409), no such proposal (404) or no answer at all
 * (status 0, which may have reached the daemon), that project's inbox, that proposal and its round
 * sheets are read again, and the call settles once they have, so the item is gone from what shows
 * before the dialog closes. Tasks and the reads of the spec follow the live tail's
 * `proposal.applied`, as after a terminal's confirm; no `.unstaged` is logged, so no outside-change
 * alert either (`docs/canon/decision-staging.md` "Queue").
 */
export function useConfirmProposal(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  async function readAgain(id: string) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true }),
      queryClient.invalidateQueries({ queryKey: queryKeys.proposal(project, id), exact: true }),
      queryClient.invalidateQueries({ queryKey: roundsOf(project) }),
    ]);
  }
  return useMutation({
    mutationFn: ({ id, body }: ConfirmRequest) => client.confirmProposal(project, id, body),
    onSuccess: async (review, { id }) => {
      queryClient.setQueryData<Proposal>(queryKeys.proposal(project, id), review);
      await readAgain(id);
    },
    onError: async (error, { id }) => {
      const { status } = apiErrorOf(error);
      if (status === STAGE_REFUSED || status === NO_SUCH_PROPOSAL || status === 0) {
        await readAgain(id);
      }
    },
  });
}

/**
 * The project's whole plan (`docs/features/roadmap-screen.md`): one read, neither option sent, its
 * query's AbortSignal taken (a read superseded by a live event's is aborted). Read on entering the
 * Roadmap, on "Read again" (`refetch`) and, while the Roadmap is on screen, after a `task.*` event,
 * `proposal.applied` or a gap (useLiveQueue); never on window focus.
 */
export function useRoadmap(project: string) {
  const client = useClient();
  return useQuery({ queryKey: queryKeys.roadmap(project), queryFn: ({ signal }) => client.getRoadmap(project, undefined, signal) });
}

/** The plan's read as the Roadmap shares it among its parts. */
export type RoadmapQuery = ReturnType<typeof useRoadmap>;

/**
 * The Board's one read (`docs/features/board-ui.md` "Board"): the filters work on its answer, and
 * Show cancelled is another entry, the last board staying up while it is read. Each read asks the
 * marks, so a tile is never drawn without one (`docs/features/trace-views.md` "The board tile"). Read
 * on entering the Board, after a write of this page and, while it shows, after a `card.*` event, an
 * apply, a decision on a proposal, a gap, and on window focus, for a hand edit, which raises no event.
 */
export function useCards(project: string, options: BoardCardsOptions = {}) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.cards(project, options),
    queryFn: ({ signal }) => client.getCards(project, options.with_cancelled === true ? { with_cancelled: true, marks: true } : { marks: true }, signal),
    placeholderData: keepPreviousData,
    refetchOnWindowFocus: true,
  });
}

/** The card list's read as the Board shares it. */
export type CardsQuery = ReturnType<typeof useCards>;

/** One card with its history (`id` null: nothing read); a new ID never shows the last one's card. */
export function useCard(project: string, id: string | null) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.card(project, id ?? ""),
    queryFn: ({ signal }) => client.getCard(project, id ?? "", signal),
    enabled: id !== null,
  });
}

/** One card's read as the panel shares it. */
export type CardQuery = ReturnType<typeof useCard>;

/**
 * Marks and rollups (`docs/features/card-trace.md`), never stored: read when a screen shows them
 * (`enabled`), after a card write, a `card.*` event, an apply, a decision on a proposal or a gap; with
 * `onFocus` on window focus too, for a hand edit, which raises no event. A read superseded is aborted.
 */
export function useTrace(project: string, options: TraceOptions = {}, enabled = true, onFocus = false) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.trace(project, options),
    queryFn: ({ signal }) => client.getTrace(project, traceRequest(options), signal),
    enabled,
    refetchOnWindowFocus: onFocus,
  });
}

/** A trace read as a view shares it. */
export type TraceQuery = ReturnType<typeof useTrace>;

/** One of the card writes, as `useCardWrite` sends it (`docs/features/board-ui.md` "Routes"). */
export type CardWriteRequest =
  | { write: "create"; body: CardCreateBody }
  | { write: "move"; id: string; body: CardMoveBody }
  | { write: "edit"; id: string; body: CardEditBody }
  | { write: "assign"; id: string; body: CardAssignBody }
  | { write: "cancel"; id: string; body: CardCancelBody }
  | { write: "accept"; id: string; body: CardAcceptBody }
  | { write: "clear"; id: string; body: CardClearBody };

/**
 * One card write per call; nothing is optimistic. After an answer, a refusal (409, a lost
 * compare-and-set too) or no such card (404), the project's card lists, that card, its trace and its
 * flows, whose boxes carry their rollups, are read again, and the call settles once they have: the
 * board on screen is the daemon's. Any other failure stored nothing, and no answer at all (status 0)
 * may have stored it: read again too.
 */
export function useCardWrite(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  async function readAgain(id: string | null) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: cardListsOf(project) }),
      queryClient.invalidateQueries({ queryKey: tracesOf(project) }),
      queryClient.invalidateQueries({ queryKey: flowsOf(project) }),
      id === null ? Promise.resolve() : queryClient.invalidateQueries({ queryKey: queryKeys.card(project, id), exact: true }),
    ]);
  }
  return useMutation({
    mutationKey: cardWritesKey(project),
    mutationFn: (request: CardWriteRequest): Promise<CardWrite> => {
      switch (request.write) {
        case "create":
          return client.createCard(project, request.body);
        case "move":
          return client.moveCard(project, request.id, request.body);
        case "edit":
          return client.editCard(project, request.id, request.body);
        case "assign":
          return client.assignCard(project, request.id, request.body);
        case "cancel":
          return client.cancelCard(project, request.id, request.body);
        case "accept":
          return client.acceptCard(project, request.id, request.body);
        case "clear":
          return client.clearCard(project, request.id, request.body);
      }
    },
    onSuccess: (answer) => readAgain(answer.id),
    onError: async (error, request) => {
      const { status } = apiErrorOf(error);
      if (status === CARD_REFUSED || status === NO_SUCH_CARD || status === 0) {
        await readAgain(request.write === "create" ? null : request.id);
      }
    },
  });
}

/**
 * Whether a card write of the project is out, its re-read included: while one is, the Board's card
 * modal neither closes nor opens, Back included, which takes effect once it settles (`ui/README.md`
 * "Screen rules", Dialogs).
 */
export function useCardWriting(project: string): boolean {
  return useIsMutating({ mutationKey: cardWritesKey(project) }) > 0;
}

/**
 * The group clear (`docs/features/card-trace.md` "Clearing"): one POST; as `useCardWrite`, the call
 * settles once the lists, every card read, the trace and the flows have been read again.
 */
export function useCardClearGroup(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  async function readAgain() {
    await Promise.all([cardListsOf(project), cardsOf(project), tracesOf(project), flowsOf(project)].map((queryKey) => queryClient.invalidateQueries({ queryKey })));
  }
  return useMutation({
    mutationFn: (body: CardClearGroupBody) => client.clearCardGroup(project, body),
    onSuccess: readAgain,
    onError: async (error) => {
      const { status } = apiErrorOf(error);
      if (status === CARD_REFUSED || status === NO_SUCH_CARD || status === 0) {
        await readAgain();
      }
    },
  });
}

/** The round sheet at `limit` (null: the daemon's default); while another limit is read, the last sheet stays up. */
export function useRound(project: string, limit: number | null) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.round(project, limit),
    queryFn: ({ signal }) => client.getRound(project, limit === null ? undefined : { limit }, signal),
    placeholderData: keepPreviousData,
  });
}

/** The round sheet's read as the Questions screen shares it. */
export type RoundQuery = ReturnType<typeof useRound>;

/** A stage body's choice: the body less `updated_at`, as an event's stored stage holds it (less `span_hash`). */
function choiceOf(stage: RoundAnswerBody["stage"]): StageChoice {
  if (stage.decision === "approve") {
    return { decision: stage.decision, option: stage.option, answer: stage.answer, canon: stage.canon, editorial: stage.editorial, note: stage.note };
  }
  return { decision: stage.decision, reason: stage.reason };
}

/** This tab's write per answer of one round POST. */
type RoundWrites = readonly { id: string; write: OwnWrite }[];

/** The outcome of an answer the daemon refused: nothing of it was stored. */
const REFUSED = "refused";

/**
 * The round's one POST per Stage (`docs/features/ui-round.md` "Data"): every answer is noted as this
 * tab's own write before the POST is sent, so the N `proposal.staged` events, which may come before
 * the answer, never raise the outside-change alert; each write is ended by its answer's outcome (a
 * refused one stored nothing; a staged one awaits its event). No answer at all (status 0) may have
 * staged any: each write still awaits its event, and the sheet is read again; any other failure
 * stored nothing. After a 200 that project's round sheets, its inbox and each answered proposal are
 * read again. Every POST of a project carries one mutation key, so a screen mounted while one is on
 * its way knows it (useRoundPosting) and sends nothing until it settles.
 */
export function useStageRoundAnswers(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  function readAgain(writes: RoundWrites) {
    void queryClient.invalidateQueries({ queryKey: roundsOf(project) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project), exact: true });
    for (const { id } of writes) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.proposal(project, id), exact: true });
    }
  }
  return useMutation({
    mutationKey: roundAnswersKey(project),
    mutationFn: (answers: RoundAnswerBody[]): Promise<RoundAnswersOutcome> => client.stageRoundAnswers(project, answers),
    onMutate: (answers): RoundWrites => answers.map(({ id, stage }) => ({ id, write: startOwnWrite(queryClient, project, id, choiceOf(stage)) })),
    onSuccess: (document, _answers, writes) => {
      const outcomes = new Map(document.answers.map((answer) => [answer.id, answer.outcome]));
      for (const { id, write } of writes) {
        endOwnWrite(queryClient, project, id, write, outcomes.get(id) === REFUSED);
      }
      readAgain(writes);
    },
    onError: (error, _answers, writes) => {
      const unanswered = apiErrorOf(error).status === 0;
      for (const { id, write } of writes ?? []) {
        endOwnWrite(queryClient, project, id, write, !unanswered);
      }
      if (unanswered) {
        readAgain(writes ?? []);
      }
    },
  });
}

/**
 * Whether a round POST of the project is on its way, sent from this screen or from one the owner
 * left since: while it is, Stage sends nothing (the answers it carries are still on screen).
 */
export function useRoundPosting(project: string): boolean {
  return useIsMutating({ mutationKey: roundAnswersKey(project) }) > 0;
}
