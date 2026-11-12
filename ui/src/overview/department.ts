import type { CardCounts, CardFlow, Workspace } from "../api/types";
import type { SectionId } from "../app/sections";
import { localToday } from "../board/due";
import { categoryOf, type StatusCategory } from "../board/category";

// The Overview's fold (docs/features/department-overview.md "Acceptance criteria", DO2 to DO6): the
// daemon's counts per flow and column, gathered over a department's flows by status category. A
// column's category is looked up in its served flow (`src/board/category.ts`), as the Board's
// column heads are, so the two agree by construction; a flow's department is the served
// workspace's `flows`. Nothing here compares a department's, a flow's or a column's name with a
// word: names are data, matched only against other served names. The one section this module
// names is Flow's, checked by `src/app/sections.ts`' type. No clock is read here: an age is counted
// from a stored time to the day the caller passes, the viewer's.

/** The section whose listing shows the Gaps block. */
const FLOW_SECTION: SectionId = "flow";

/** What a department's flows add up to, folded by status category. */
export interface DepartmentTally extends Record<StatusCategory, number> {
  /** Open tasks in each category; done holds the accepted ones too. */
  todo: number;
  doing: number;
  /** Open tasks in a flow's `accept.from`: a part of `doing`, never added to a row's sum. */
  review: number;
  returned: number;
  done: number;
  /** Open tasks in a column their flow no longer lists, or in a flow no `[card]` lists. */
  stranded: number;
  cancelled: number;
  /** Open questions, discrepancies and decisions on the flows' tasks. */
  questions: number;
  /** The least stored `created_at` of them; null with none. */
  oldest: string | null;
  /** Open subtasks in a flow's `return` column. */
  returnedSubtasks: number;
  /** Open parents whose every subtask is accepted or cancelled: a status, never a gate (ADR-0074). */
  ready: number;
  /** The distinct agent tasks the flows' open tasks link whose plan waits for review; null when not counted (a hub). */
  agentTasks: number | null;
}

/** The two words the daemon writes in `agent_tasks`, each by itself. */
const AGENT_TASKS = { counted: "counted", local_only: "local_only" } as const;

/** How the agent task row reads: a count, the hub's line, or the daemon's word raw. */
export type AgentTasksShown = { shown: "count"; count: number } | { shown: "local" } | { shown: "word"; word: string };

/**
 * The department's flows that the counts hold, in the department's order; with no department read
 * or `flows` null, every flow the counts hold, in theirs. A flow listed twice counts once.
 */
export function departmentFlows(counts: CardCounts, workspace: Workspace | null): string[] {
  const held = counts.by_flow.map((row) => row.flow);
  if (workspace === null || workspace.flows === null) {
    return held;
  }
  return [...new Set(workspace.flows)].filter((flow) => held.includes(flow));
}

/** A flow as served, by its name; undefined for a flow no `[card]` lists. */
export function servedFlow(counts: CardCounts, name: string): CardFlow | undefined {
  return counts.flows.find((flow) => flow.name === name);
}

/** The counts folded over `flows`, each counted once, column by column, then flow by flow. */
export function tallyOf(counts: CardCounts, flows: readonly string[]): DepartmentTally {
  const chosen = new Set(flows);
  const tally: DepartmentTally = {
    todo: 0,
    doing: 0,
    review: 0,
    returned: 0,
    done: 0,
    stranded: 0,
    cancelled: 0,
    questions: 0,
    oldest: null,
    returnedSubtasks: 0,
    ready: 0,
    agentTasks: counts.agent_tasks === AGENT_TASKS.local_only ? null : 0,
  };
  for (const row of counts.columns) {
    if (!chosen.has(row.flow)) {
      continue;
    }
    const flow = servedFlow(counts, row.flow);
    tally.done += row.accepted;
    tally.cancelled += row.cancelled;
    // An open task counts in its column's category, looked up: `accept.to` done, a column the flow
    // no longer lists not in the flow (DO2).
    tally[categoryOf(flow, row.column)] += row.open;
    if (flow !== undefined && row.column === flow.accept.from) {
      tally.review += row.open;
    }
    if (flow !== undefined && flow.return !== null && row.column === flow.return) {
      tally.returnedSubtasks += row.open_subtasks;
    }
  }
  const agentTasks = new Set<string>();
  for (const row of counts.by_flow) {
    if (!chosen.has(row.flow)) {
      continue;
    }
    tally.questions += row.open_items;
    tally.ready += row.ready_parents;
    if (row.oldest_open_item !== null && (tally.oldest === null || row.oldest_open_item < tally.oldest)) {
      tally.oldest = row.oldest_open_item;
    }
    if (row.review_agent_tasks === null) {
      tally.agentTasks = null;
    } else {
      for (const id of row.review_agent_tasks) {
        agentTasks.add(id);
      }
    }
  }
  if (tally.agentTasks !== null) {
    tally.agentTasks = agentTasks.size;
  }
  return tally;
}

/** A row's sum: what is to do, in progress, returned and done; waiting for review is a part of in progress. */
export function rowTotal(tally: DepartmentTally): number {
  return tally.todo + tally.doing + tally.returned + tally.done;
}

/** How the agent task row reads: the hub's line where none is counted, a word the page does not know raw. */
export function agentTasksShown(counts: CardCounts, tally: DepartmentTally): AgentTasksShown {
  if (counts.agent_tasks === AGENT_TASKS.local_only) {
    return { shown: "local" };
  }
  if (counts.agent_tasks !== AGENT_TASKS.counted || tally.agentTasks === null) {
    return { shown: "word", word: counts.agent_tasks };
  }
  return { shown: "count", count: tally.agentTasks };
}

/** No flow and no column served: a project with no `[card]` table and no task. */
export function isEmpty(counts: CardCounts): boolean {
  return counts.flows.length === 0 && counts.columns.length === 0 && counts.by_flow.length === 0;
}

/** Which blocks show past Work and Waiting, by config alone (docs/features/department-overview.md "Acceptance criteria", DO6). */
export interface Blocks {
  /** The department's sections list Flow, or no department was read. */
  gaps: boolean;
  /** The department's `flows` is null and two or more departments are served. */
  side: boolean;
}

/** The blocks a department shows, by its `sections` and `flows`, never by its name. */
export function blocksOf(workspace: Workspace | null, workspaces: readonly Workspace[]): Blocks {
  return {
    gaps: workspace === null || workspace.sections.includes(FLOW_SECTION),
    side: workspace !== null && workspace.flows === null && workspaces.length >= 2,
  };
}

const DAY_MS = 86_400_000;

/** A `YYYY-MM-DD` day as a count of days, by the calendar, never a clock. */
function dayNumber(day: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (match === null) {
    return null;
  }
  return Math.round(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])) / DAY_MS);
}

/**
 * Calendar days from the viewer's local date of a stored time to `today`, the viewer's day; 0 on
 * the same day. Null for a time that does not read, which the page then shows as stored.
 */
export function ageDays(createdAt: string, today: string): number | null {
  const at = Date.parse(createdAt);
  if (Number.isNaN(at)) {
    return null;
  }
  const from = dayNumber(localToday(at));
  const to = dayNumber(today);
  return from === null || to === null ? null : to - from;
}
