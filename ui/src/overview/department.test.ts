import { describe, expect, it } from "vitest";
import type { CardCounts } from "../api/types";
import { aColumnCount, aFlowCount } from "../test/builders";
import { PINNED_WORKSPACES, pinnedCountsJson, pinnedHubCountsJson, pinnedWorkspace } from "../test/countsStub";
import { ageDays, agentTasksShown, blocksOf, departmentFlows, rowTotal, tallyOf, type DepartmentTally } from "./department";

// docs/features/department-overview.md "Acceptance criteria": the Overview's fold over the spec's
// scenario (DO2), the blocks by config alone (DO6), ages against the viewer's day with no clock (DO5),
// the hub's agent tasks (DO4) and a flow no `[card]` lists (DO9), each on the pure model of
// src/overview/department.ts; Ready to accept as the rollup's `ready` over the mock (DO8) is
// src/mocks/cardCounts.test.ts'.

const TODAY = "2026-11-12";

/** A department's tally over the scenario's counts. */
function tallyIn(name: string | null, counts: CardCounts = pinnedCountsJson()): DepartmentTally {
  return tallyOf(counts, departmentFlows(counts, name === null ? null : pinnedWorkspace(name)));
}

/** A tally as the spec's criterion words it. */
function said(tally: DepartmentTally) {
  return {
    todo: tally.todo,
    doing: tally.doing,
    review: tally.review,
    returned: tally.returned,
    done: tally.done,
    stranded: tally.stranded,
    cancelled: tally.cancelled,
    questions: tally.questions,
    oldestDays: tally.oldest === null ? null : ageDays(tally.oldest, TODAY),
    returnedSubtasks: tally.returnedSubtasks,
    ready: tally.ready,
    agentTasks: tally.agentTasks,
  };
}

const WORDS = { todo: 0, doing: 1, review: 1, returned: 1, done: 0, stranded: 1, cancelled: 1, questions: 3, oldestDays: 10, returnedSubtasks: 1, ready: 1, agentTasks: 1 };

describe("the fold over the spec's scenario (DO2)", () => {
  it("words, over copy: to do 0, in progress 1, waiting for review 1, returned 1, done 0, not in the flow 1, cancelled 1, questions 3, oldest 10 days, returned subtasks 1, ready 1, agent tasks 1", () => {
    expect(departmentFlows(pinnedCountsJson(), pinnedWorkspace("words"))).toEqual(["copy"]);
    expect(said(tallyIn("words"))).toEqual(WORDS);
  });

  it("print, over copy and plates: to do 1 and done 1, the rest as words; T-0001, linked from both flows, counts once", () => {
    expect(departmentFlows(pinnedCountsJson(), pinnedWorkspace("print"))).toEqual(["copy", "plates"]);
    expect(said(tallyIn("print"))).toEqual({ ...WORDS, todo: 1, done: 1 });
  });

  it("press and All flows, every flow: to do 2, the rest as print", () => {
    expect(departmentFlows(pinnedCountsJson(), pinnedWorkspace("press"))).toEqual(["copy", "plates", "sound"]);
    expect(said(tallyIn("press"))).toEqual({ ...WORDS, todo: 2, done: 1 });
    expect(said(tallyIn(null))).toEqual(said(tallyIn("press")));
  });

  it("leaves waiting for review out of a row's sum: it is a part of in progress", () => {
    const words = tallyIn("words");
    expect(rowTotal(words)).toBe(words.todo + words.doing + words.returned + words.done);
    expect(rowTotal(words)).toBe(2);
  });

  it("counts an open task in accept.to as done, never in progress (M: accept.to counted in progress)", () => {
    const counts = { ...pinnedCountsJson(), columns: [aColumnCount({ flow: "copy", column: "done", open: 2 })] };
    expect([tallyIn("words", counts).done, tallyIn("words", counts).doing]).toEqual([2, 0]);
  });

  it("counts an accepted task as done whatever its column (M: an accepted task counted in its column's category)", () => {
    const counts = { ...pinnedCountsJson(), columns: [aColumnCount({ flow: "copy", column: "review", accepted: 1 }), aColumnCount({ flow: "copy", column: "todo", accepted: 1 })] };
    const tally = tallyIn("words", counts);
    expect([tally.done, tally.doing, tally.todo, tally.review]).toEqual([2, 0, 0, 0]);
  });

  it("counts a cancelled task as cancelled alone (M: a cancelled task counted in its column's category)", () => {
    const counts = { ...pinnedCountsJson(), columns: [aColumnCount({ flow: "copy", column: "review", cancelled: 2 }), aColumnCount({ flow: "copy", column: "proofing", cancelled: 1 })] };
    const tally = tallyIn("words", counts);
    expect([tally.cancelled, tally.doing, tally.review, tally.stranded, rowTotal(tally)]).toEqual([3, 0, 0, 0, 0]);
  });

  it("counts returned subtasks in the flow's return column alone, from open subtasks", () => {
    const counts = { ...pinnedCountsJson(), columns: [aColumnCount({ flow: "copy", column: "review", open: 2, open_subtasks: 2 }), aColumnCount({ flow: "copy", column: "returned", open: 3, open_subtasks: 1 })] };
    expect(tallyIn("words", counts).returnedSubtasks).toBe(1);
    // plates names no return column: its subtasks are never returned ones.
    const plates = { ...pinnedCountsJson(), columns: [aColumnCount({ flow: "plates", column: "todo", open: 1, open_subtasks: 1 })] };
    expect(tallyIn("print", plates).returnedSubtasks).toBe(0);
  });
});

describe("a flow no [card] lists (DO9)", () => {
  // `ink` is stored but no longer configured: its columns are not in the flow, and only a department
  // whose `flows` is null counts it.
  const counts: CardCounts = {
    ...pinnedCountsJson(),
    columns: [...pinnedCountsJson().columns, aColumnCount({ flow: "ink", column: "mixing", open: 2, accepted: 1 })],
    by_flow: [...pinnedCountsJson().by_flow, aFlowCount({ flow: "ink", open_items: 1, oldest_open_item: "2026-11-01T09:00:00Z" })],
  };

  it("counts under press alone, its open tasks not in the flow, its accepted ones done", () => {
    expect(departmentFlows(counts, pinnedWorkspace("press"))).toEqual(["copy", "plates", "sound", "ink"]);
    expect(departmentFlows(counts, pinnedWorkspace("print"))).toEqual(["copy", "plates"]);
    const press = tallyIn("press", counts);
    expect([press.stranded, press.done, press.questions, press.oldest]).toEqual([3, 2, 4, "2026-11-01T09:00:00Z"]);
    expect(said(tallyIn("words", counts))).toEqual(WORDS);
    expect(said(tallyIn("print", counts))).toEqual({ ...WORDS, todo: 1, done: 1 });
  });

  it("drops a flow a department lists that the counts do not hold, keeping the department's order and each flow once", () => {
    const workspace = { ...pinnedWorkspace("print"), flows: ["plates", "inks", "copy", "plates"] };
    expect(departmentFlows(pinnedCountsJson(), workspace)).toEqual(["plates", "copy"]);
  });
});

describe("the blocks by config alone (DO6)", () => {
  it("shows Gaps where sections list Flow and the side table where flows is null with two departments or more: words Gaps, print neither, press both", () => {
    expect(PINNED_WORKSPACES.map((workspace) => [workspace.name, blocksOf(workspace, PINNED_WORKSPACES)])).toEqual([
      ["words", { gaps: true, side: false }],
      ["print", { gaps: false, side: false }],
      ["press", { gaps: true, side: true }],
    ]);
  });

  it("shows Gaps and no side table with no department read; no side table for one department alone (M: side for flows set)", () => {
    expect(blocksOf(null, [])).toEqual({ gaps: true, side: false });
    expect(blocksOf(pinnedWorkspace("press"), [pinnedWorkspace("press")])).toEqual({ gaps: true, side: false });
    // A department renamed keeps its blocks: names are data (M: `name === "writing"`).
    expect(blocksOf({ ...pinnedWorkspace("words"), name: "writing" }, PINNED_WORKSPACES)).toEqual({ gaps: true, side: false });
    expect(blocksOf({ ...pinnedWorkspace("print"), flows: null }, PINNED_WORKSPACES)).toEqual({ gaps: false, side: true });
  });
});

describe("ages against the viewer's day, no clock (DO5)", () => {
  it("counts calendar days from the stored time's local date: 10 at 11-12, 12 at 11-14, 0 the same day", () => {
    expect(ageDays("2026-11-02T09:00:00Z", "2026-11-12")).toBe(10);
    expect(ageDays("2026-11-02T09:00:00Z", "2026-11-14")).toBe(12);
    expect(ageDays("2026-11-02T09:00:00Z", "2026-11-02")).toBe(0);
    expect(ageDays("2026-10-31T09:00:00Z", "2026-11-01")).toBe(1);
  });

  it("says nothing of a time that does not read", () => {
    expect(ageDays("yesterday-ish", TODAY)).toBeNull();
    expect(ageDays("2026-11-02T09:00:00Z", "the 12th")).toBeNull();
  });
});

describe("agent tasks on a hub (DO4)", () => {
  it("is null on local_only, never 0, and reads as the hub's line (M: null shown as 0)", () => {
    const counts = pinnedHubCountsJson();
    const tally = tallyIn("print", counts);
    expect(tally.agentTasks).toBeNull();
    expect(agentTasksShown(counts, tally)).toEqual({ shown: "local" });
  });

  it("is the union's size where counted, and a word the page does not know raw", () => {
    const counts = pinnedCountsJson();
    expect(agentTasksShown(counts, tallyIn("print", counts))).toEqual({ shown: "count", count: 1 });
    const later = { ...counts, agent_tasks: "sampled" };
    expect(agentTasksShown(later, tallyIn("print", later))).toEqual({ shown: "word", word: "sampled" });
  });
});
