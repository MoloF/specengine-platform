import type { CardCounts, CardFlow, Workspace } from "../api/types";
import { aWorkspace } from "./builders";

// The spec's scenario as docs/features/department-overview.md "Data" pins it ("JSON"): the counts a
// local daemon serves over three flows, seven tasks, four items and two agent tasks, `flows` the
// card list's (elided in the spec: its config's three, every column titled by its ID, every move by
// anyone), and the three departments its config serves. Copied from the spec by hand, never
// computed: src/mocks/cardCounts.test.ts compares the mock's count with it, and
// src/overview/department.test.ts folds it.

/** A flow of the scenario's config: its columns' titles their IDs, every move by anyone. */
function configFlow(name: string, title: string, columns: readonly string[], accept: { from: string; to: string }, returned: string | null): CardFlow {
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

/** The scenario's counts, as "JSON" writes them. */
export function pinnedCountsJson(): CardCounts {
  return {
    flows: [
      configFlow("copy", "Copy", ["todo", "doing", "review", "returned", "done"], { from: "review", to: "done" }, "returned"),
      configFlow("plates", "Plates", ["todo", "drawing", "done"], { from: "drawing", to: "done" }, null),
      configFlow("sound", "Sound", ["todo", "check", "done"], { from: "check", to: "done" }, null),
    ],
    columns: [
      { flow: "copy", column: "todo", open: 0, accepted: 0, cancelled: 1, open_subtasks: 0 },
      { flow: "copy", column: "doing", open: 0, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "copy", column: "review", open: 1, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "copy", column: "returned", open: 1, accepted: 0, cancelled: 0, open_subtasks: 1 },
      { flow: "copy", column: "done", open: 0, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "copy", column: "proofing", open: 1, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "plates", column: "todo", open: 1, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "plates", column: "drawing", open: 0, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "plates", column: "done", open: 0, accepted: 1, cancelled: 0, open_subtasks: 0 },
      { flow: "sound", column: "todo", open: 1, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "sound", column: "check", open: 0, accepted: 0, cancelled: 0, open_subtasks: 0 },
      { flow: "sound", column: "done", open: 0, accepted: 0, cancelled: 0, open_subtasks: 0 },
    ],
    by_flow: [
      { flow: "copy", open_items: 3, oldest_open_item: "2026-11-02T09:00:00Z", ready_parents: 1, review_agent_tasks: ["T-0001"] },
      { flow: "plates", open_items: 0, oldest_open_item: null, ready_parents: 0, review_agent_tasks: ["T-0001"] },
      { flow: "sound", open_items: 0, oldest_open_item: null, ready_parents: 0, review_agent_tasks: [] },
    ],
    agent_tasks: "counted",
    notes: [],
    truncated: false,
    omitted: null,
  };
}

/** The same counts as a hub serves them: agent tasks not read, every list null (DO4). */
export function pinnedHubCountsJson(): CardCounts {
  const counts = pinnedCountsJson();
  return { ...counts, agent_tasks: "local_only", by_flow: counts.by_flow.map((row) => ({ ...row, review_agent_tasks: null })) };
}

/** The scenario's three departments, in config order: `words` and `print` with their flows, `press` every flow. */
export const PINNED_WORKSPACES: readonly Workspace[] = [
  aWorkspace({ name: "words", title: "Words", landing: "overview", sections: ["flow", "board", "questions", "inbox"], flows: ["copy"] }),
  aWorkspace({ name: "print", title: "Print", landing: "overview", sections: ["board", "health", "inbox"], flows: ["copy", "plates"] }),
  aWorkspace({ name: "press", title: "Press room", landing: "overview", sections: ["inbox", "board", "roadmap", "flow"], flows: null }),
];

/** One of the scenario's departments by its name. */
export function pinnedWorkspace(name: string): Workspace {
  const found = PINNED_WORKSPACES.find((workspace) => workspace.name === name);
  if (found === undefined) {
    throw new Error(`no department ${name} in the scenario`);
  }
  return found;
}
