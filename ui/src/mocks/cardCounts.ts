import type { CardColumnCount, CardCounts, CardFlow, CardFlowCount } from "../api/types";
import type { MockProposal } from "./build";
import type { MockBoard } from "./cards";
import type { Scenario } from "./scenario";
import type { MockTasks } from "./tasks";

// The task counts as the daemon's `GET .../cards/counts` serves them (docs/features/department-overview.md
// "Acceptance criteria", DO2, DO4, DO9, DO10): counted from the mock's own cards, queue and tasks by
// the store's rules, so the Board and the Overview agree. A task counts in exactly one of
// cancelled, accepted and open (in that order) in its flow and column; an open one with a parent is
// an open subtask there. A parent counts as ready in its own flow when it is open, has a child, and
// no child is open; a grandchild never counts. An open item counts on its task's flow whatever the
// task's state, its least `created_at` the oldest. The agent tasks in review are the distinct `T-`
// IDs a flow's open tasks link whose row is in `review`, by number; a hub reads none, so it serves
// `local_only` and every list null. Order: the configured flows in config order, every column of
// theirs, zeros too, then each one's stranded columns by name; then the flows no `[card]` lists, by
// name. No clock is read: the counts hold stored times only.
//
// Scenarios: `local-only` and `hub` serve the hub's shape; `empty` serves a project with no `[card]`
// table and no task, no flow at all; `error` refuses as every read. The spec's own scenario is held
// too (`pinnedCounts`), its JSON in src/test/countsStub.ts.

/** A stored task as the counts read it: its place, its state and its links. */
export interface CountedTask {
  id: string;
  flow: string;
  column: string;
  parent: string | null;
  accepted: boolean;
  cancelled: boolean;
  tasks: readonly string[];
}

/** A queue row as the counts read it. */
export interface CountedItem {
  card_id: string | null;
  status: string;
  created_at: string;
}

/** What the counts read, once. */
export interface CountsInput {
  flows: readonly CardFlow[];
  tasks: readonly CountedTask[];
  items: readonly CountedItem[];
  /** An agent task's stored status; null for none. */
  agentTask: (id: string) => string | null;
  /** A local daemon reads its agent tasks; a hub does not. */
  tasksHere: boolean;
}

/** The status an agent task's plan waits in for the owner. */
const REVIEW = "review";

/** The open state of a queue row. */
const OPEN = "open";

const collator = new Intl.Collator("en");

/** An ID's number, as the store orders IDs. */
function idNumber(id: string): number {
  return Number(id.slice(id.indexOf("-") + 1));
}

function isOpen(task: CountedTask): boolean {
  return !task.accepted && !task.cancelled;
}

/** The flows in the document's order: the configured ones, then those no `[card]` lists, by name. */
function flowOrder(input: CountsInput): string[] {
  const configured = input.flows.map((flow) => flow.name);
  const others = [...new Set(input.tasks.map((task) => task.flow))].filter((flow) => !configured.includes(flow)).sort(collator.compare);
  return [...configured, ...others];
}

/** A flow's columns in the document's order: its configured ones, then the stranded ones by name. */
function columnOrder(input: CountsInput, flow: string): string[] {
  const configured = input.flows.find((candidate) => candidate.name === flow)?.columns.map((column) => column.id) ?? [];
  const stored = [...new Set(input.tasks.filter((task) => task.flow === flow).map((task) => task.column))];
  return [...configured, ...stored.filter((column) => !configured.includes(column)).sort(collator.compare)];
}

/** The counts over one stored state, by the store's rules. */
export function countsOf(input: CountsInput): CardCounts {
  const flows = flowOrder(input);
  const columns: CardColumnCount[] = flows.flatMap((flow) =>
    columnOrder(input, flow).map((column) => {
      const here = input.tasks.filter((task) => task.flow === flow && task.column === column);
      return {
        flow,
        column,
        open: here.filter(isOpen).length,
        accepted: here.filter((task) => !task.cancelled && task.accepted).length,
        cancelled: here.filter((task) => task.cancelled).length,
        open_subtasks: here.filter((task) => isOpen(task) && task.parent !== null).length,
      };
    }),
  );
  const byFlow: CardFlowCount[] = flows.map((flow) => {
    const own = input.tasks.filter((task) => task.flow === flow);
    const ids = new Set(own.map((task) => task.id));
    const items = input.items.filter((item) => item.status === OPEN && item.card_id !== null && ids.has(item.card_id));
    const oldest = items.map((item) => item.created_at).sort()[0] ?? null;
    const ready = own.filter((parent) => {
      const children = input.tasks.filter((task) => task.parent === parent.id);
      return isOpen(parent) && children.length > 0 && !children.some(isOpen);
    }).length;
    const review = [...new Set(own.filter(isOpen).flatMap((task) => task.tasks))].filter((id) => input.agentTask(id) === REVIEW).sort((a, b) => idNumber(a) - idNumber(b));
    return { flow, open_items: items.length, oldest_open_item: oldest, ready_parents: ready, review_agent_tasks: input.tasksHere ? review : null };
  });
  return {
    flows: structuredClone([...input.flows]),
    columns,
    by_flow: byFlow,
    agent_tasks: input.tasksHere ? "counted" : "local_only",
    notes: [],
    truncated: false,
    omitted: null,
  };
}

/** A project's counts in a scenario, over its mock board, tasks and queue as they stand now. */
export function cardCountsOf(board: MockBoard, tasks: MockTasks, proposals: readonly MockProposal[], scenario: Scenario): CardCounts {
  if (scenario === "empty") {
    return countsOf({ flows: [], tasks: [], items: [], agentTask: () => null, tasksHere: true });
  }
  return countsOf({
    flows: board.flows,
    tasks: board.cards.map((card) => ({
      id: card.id,
      flow: card.flow,
      column: card.column,
      parent: card.parent,
      accepted: card.acceptance !== null,
      cancelled: card.cancellation !== null,
      tasks: card.tasks,
    })),
    items: proposals.map(({ review }) => ({ card_id: review.card_id, status: review.status, created_at: review.created_at })),
    agentTask: (id) => tasks.tasks.find((task) => task.id === id)?.status ?? null,
    tasksHere: scenario !== "local-only" && scenario !== "hub",
  });
}

// The spec's scenario ("Data", "JSON"): three flows, seven tasks, four items and two agent tasks,
// every time at nine UTC.

/** A flow of the spec's scenario: its columns' titles their IDs, every move by anyone. */
function scenarioFlow(name: string, title: string, columns: readonly string[], accept: { from: string; to: string }, returned: string | null): CardFlow {
  const open = columns.filter((column) => column !== accept.to);
  return {
    name,
    title,
    disciplines: [],
    columns: columns.map((id) => ({ id, title: id })),
    moves: open.flatMap((from) => open.filter((to) => to !== from).map((to) => ({ from, to, by: [] }))),
    accept,
    return: returned,
  };
}

/** The spec's three flows, in config order. */
const PINNED_FLOWS: readonly CardFlow[] = [
  scenarioFlow("copy", "Copy", ["todo", "doing", "review", "returned", "done"], { from: "review", to: "done" }, "returned"),
  scenarioFlow("plates", "Plates", ["todo", "drawing", "done"], { from: "drawing", to: "done" }, null),
  scenarioFlow("sound", "Sound", ["todo", "check", "done"], { from: "check", to: "done" }, null),
];

/** A task of the spec's scenario: open, no parent, no link, unless given. */
function pinnedTask(fields: Partial<CountedTask> & Pick<CountedTask, "id" | "flow" | "column">): CountedTask {
  return { parent: null, accepted: false, cancelled: false, tasks: [], ...fields };
}

/** The spec's scenario counted, as a local daemon serves it; `tasksHere` false: as a hub does. */
export function pinnedCounts(tasksHere = true): CardCounts {
  const statuses: Readonly<Record<string, string>> = { "T-0001": REVIEW, "T-0002": "ready" };
  return countsOf({
    flows: PINNED_FLOWS,
    tasks: [
      pinnedTask({ id: "C-0001", flow: "copy", column: "review", tasks: ["T-0001"] }),
      pinnedTask({ id: "C-0002", flow: "plates", column: "done", accepted: true, parent: "C-0001" }),
      pinnedTask({ id: "C-0003", flow: "copy", column: "todo", cancelled: true, parent: "C-0001" }),
      pinnedTask({ id: "C-0004", flow: "copy", column: "returned", parent: "C-0005" }),
      pinnedTask({ id: "C-0005", flow: "plates", column: "todo", tasks: ["T-0001", "T-0002"] }),
      pinnedTask({ id: "C-0006", flow: "sound", column: "todo" }),
      pinnedTask({ id: "C-0007", flow: "copy", column: "proofing" }),
    ],
    items: [
      { card_id: "C-0001", status: OPEN, created_at: "2026-11-02T09:00:00Z" },
      { card_id: "C-0004", status: OPEN, created_at: "2026-11-05T09:00:00Z" },
      { card_id: "C-0001", status: "applied", created_at: "2026-11-03T09:00:00Z" },
      { card_id: "C-0003", status: OPEN, created_at: "2026-11-07T09:00:00Z" },
    ],
    agentTask: (id) => statuses[id] ?? null,
    tasksHere,
  });
}
