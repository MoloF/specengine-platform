import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ru } from "./ru";

// docs/features/departments.md "Acceptance criteria", DP-8 and DP-9: the chrome speaks the owner's
// words (`docs/canon/workspaces.md` "The lens"). A workspace reads Department, a card Task, a child
// card Subtask, a `T-` task Agent task, and a `kinds` module a direction; every value that named a
// `T-` task says "agent task" in the same change that made a card a "task", so no screen says "task"
// for both. Keys, section IDs and routes are unchanged. Russian is reached through `ru.ts` alone:
// this file writes its words as escapes, never in Cyrillic.

/** Every leaf of a catalog by its dotted key. */
function leaves(group: unknown, path = ""): [string, string][] {
  if (typeof group === "string") {
    return [[path, group]];
  }
  if (typeof group !== "object" || group === null) {
    return [];
  }
  return Object.entries(group).flatMap(([key, value]) => leaves(value, path === "" ? key : `${path}.${key}`));
}

/** A value's words: its `{{placeholders}}` are names, not text, and are left out. */
function wordsOf(value: string): string {
  return value.replaceAll(/\{\{[^}]*\}\}/g, "");
}

/**
 * The groups that speak of `T-` tasks alone, whatever leaf a later change adds to them: the Tasks
 * screen, its nav entry, the Tasks screen's keys, and a card's linked agent tasks. The Overview's
 * Tasks region left with docs/features/department-overview.md; its agent task row is listed below.
 */
const AGENT_TASK_GROUPS: readonly string[] = ["tasks.", "app.sections.tasks.", "app.shortcuts.tasks.", "board.tasks."];

/**
 * The leaves outside those groups that named a `T-` task before this change, the sweep's list: each
 * now says "agent task" wherever it says "task". The work item's linked agent tasks' summary is
 * named by its content too, never by a word a department may take (Development).
 */
const AGENT_TASK_KEYS: readonly string[] = [
  "app.sections.roadmap.about",
  "app.shortcuts.intro.tasks",
  "app.shortcuts.jump",
  "palette.inputLabel",
  "palette.placeholder",
  "palette.groups.tasks",
  "home.department.agentTasks",
  "home.department.agentTasksLocal",
  "home.department.openTasks",
  "board.item.development",
  "board.item.agentTasks_one",
  "board.item.agentTasks_other",
  "board.item.noAgentTasks",
  "health.check.taskW",
  "inbox.about",
  "inbox.clearNextLink",
  "inbox.card.task",
  "inbox.card.noTask",
  "roadmap.tasksOf",
  "roadmap.poolTasks",
  "roadmap.task",
  "round.emptyTasks",
];

/** Under one of the groups of `T-` tasks. */
function inAgentTaskGroup(key: string): boolean {
  return AGENT_TASK_GROUPS.some((group) => key.startsWith(group));
}

/** The leaves that name a `T-` task: the groups of `T-` tasks and the swept list. */
function namesAnAgentTask(key: string): boolean {
  return inAgentTaskGroup(key) || AGENT_TASK_KEYS.includes(key);
}

/** A "task" not preceded by "agent ", any case. */
const BARE_TASK = /(?<!\bagent )\btasks?\b/i;

describe("the owner's words in English (DP-8 of departments)", () => {
  const all = leaves(en);

  it("no leaf says card or workspace: a card is a task, a workspace a department (M: the board's New card label kept)", () => {
    expect(all.filter(([, value]) => /\bcards?\b/i.test(wordsOf(value)) || /\bworkspaces?\b/i.test(wordsOf(value)))).toEqual([]);
    // Tool names are code, not words: `create_card` stays as the agent calls it.
    expect(en.board.noCards).toContain("create_card");
  });

  it("every task of a leaf naming a T- task is an agent task: the groups of T- tasks and the swept list (M: app.sections.tasks.label back to Tasks; board.tasks.title back to Tasks)", () => {
    const named = all.filter(([key]) => namesAnAgentTask(key));
    expect(named.length).toBeGreaterThan(AGENT_TASK_KEYS.length);
    expect(named.filter(([, value]) => BARE_TASK.test(wordsOf(value)))).toEqual([]);
    // Each group holds leaves: a renamed group is not guarded by nothing.
    expect(AGENT_TASK_GROUPS.filter((group) => !all.some(([key]) => key.startsWith(group)))).toEqual([]);
  });

  it("lists in AGENT_TASK_KEYS only leaves of the catalog outside the groups of T- tasks, each once", () => {
    const keys = new Set(all.map(([key]) => key));
    expect(AGENT_TASK_KEYS.filter((key) => !keys.has(key))).toEqual([]);
    expect(AGENT_TASK_KEYS.filter(inAgentTaskGroup)).toEqual([]);
    expect(new Set(AGENT_TASK_KEYS).size).toBe(AGENT_TASK_KEYS.length);
  });

  it("the detector: a bare task is caught, an agent task and a placeholder are not", () => {
    for (const bare of ["Tasks", "No task yet.", "Agents run the task", "Back to tasks"]) {
      expect([bare, BARE_TASK.test(wordsOf(bare))]).toEqual([bare, true]);
    }
    for (const agent of ["Agent tasks", "No agent task yet.", "Next: {{inbox}}, or {{tasks}}.", "Pool agent tasks"]) {
      expect([agent, BARE_TASK.test(wordsOf(agent))]).toEqual([agent, false]);
    }
  });

  it("names the nav's Tasks entry Agent tasks, the tree's second group Directions and the switcher Department; the section's ID stays", () => {
    expect(en.app.sections.tasks.label).toBe("Agent tasks");
    expect(en.workspace.directionsLabel).toBe("Directions");
    expect(en.workspace.workspaceLabel).toBe("Department");
    expect(en.board.modal.handOffs).toBe("Subtasks");
    expect(en.board.view.newCard).toBe("New task");
    expect([en.board.rollup.handOffs_one, en.board.rollup.handOffs_other]).toEqual(["{{count}} subtask", "{{count}} subtasks"]);
    expect(en.board.field.department).toBe("Department");
    // The work item's side block is named by its content, never "Development", a word a department may take (L1 of the looks).
    expect(en.board.item.development).toBe("Agent tasks");
    expect(en.board.handoff.action).toBe("Hand off");
    expect(Object.keys(en.app.sections)).toContain("tasks");
  });
});

/** The stem of today's Russian word for a card, before this change. */
const OLD_CARD_STEM = "\u043a\u0430\u0440\u0442\u043e\u0447";

/** Today's Russian workspace label, before this change, as its two stems: one phrase in any case. */
const OLD_WORKSPACE = new RegExp("\u0440\u0430\u0431\u043e\u0447\\S*\\s+\u043e\u0431\u043b\u0430\u0441\u0442");

/** The owner's Russian word for Department. */
const DEPARTMENT = "\u041e\u0442\u0434\u0435\u043b";

// U5 of docs/features/card-questions.md "Acceptance criteria": the chrome asks a question, never of a
// trade: no word names writers, the people a question may go to (ADR-0008). The Russian stems, a
// writer and a screenwriter, are escapes here.

/** A writer, or writers, as a word. */
const WRITER = /\bwriters?\b/i;

/** The Russian stems of a writer and of a screenwriter. */
const RUSSIAN_WRITERS = ["\u043f\u0438\u0441\u0430\u0442\u0435\u043b", "\u0441\u0446\u0435\u043d\u0430\u0440\u0438\u0441\u0442"];

describe("no trade in the chrome (U5 of card-questions)", () => {
  it("holds no writer in any English leaf, case-folded, the ask's words among them", () => {
    const all = leaves(en);
    expect(all.filter(([, value]) => WRITER.test(wordsOf(value)))).toEqual([]);
    expect(en.board.ask.action).toBe("Ask a question…");
    // The detector: a writer is caught, a write and its words are not.
    for (const said of ["Ask the writers", "Writer", "a writer's note"]) {
      expect([said, WRITER.test(said)]).toEqual([said, true]);
    }
    for (const said of ["Write a reason", "written", "rewrite"]) {
      expect([said, WRITER.test(said)]).toEqual([said, false]);
    }
  });

  it("holds neither Russian stem in any Russian leaf, case-folded", () => {
    expect(RUSSIAN_WRITERS.map((stem) => stem.length)).toEqual([7, 9]);
    const all = leaves(ru);
    expect(all.filter(([, value]) => RUSSIAN_WRITERS.some((stem) => value.toLowerCase().includes(stem)))).toEqual([]);
    // The detector: the word for writers in a sentence is caught.
    expect(RUSSIAN_WRITERS.some((stem) => "\u0421\u043f\u0440\u043e\u0441\u0438\u0442\u0435 \u043f\u0438\u0441\u0430\u0442\u0435\u043b\u0435\u0439".toLowerCase().includes(stem))).toBe(true);
  });
});

describe("the owner's words in Russian (DP-9 of departments)", () => {
  const all = leaves(ru);

  it("no leaf holds the stem of the old word for a card or the old workspace phrase, case-folded (M: workspace.workspaceLabel restored)", () => {
    expect(OLD_CARD_STEM).toHaveLength(6);
    expect(all.filter(([, value]) => value.toLowerCase().includes(OLD_CARD_STEM) || OLD_WORKSPACE.test(value.toLowerCase()))).toEqual([]);
    // The detector: the restored label is caught.
    expect(OLD_WORKSPACE.test("\u0420\u0430\u0431\u043e\u0447\u0430\u044f \u043e\u0431\u043b\u0430\u0441\u0442\u044c".toLowerCase())).toBe(true);
  });

  it("names the switcher with the owner's word for Department", () => {
    expect(ru.workspace.workspaceLabel).toBe(DEPARTMENT);
    expect(ru.board.field.department).toBe(DEPARTMENT);
    // The side block of a card's agent tasks reads as the nav's Agent tasks, in Russian too.
    expect(ru.board.item.development).toBe(ru.app.sections.tasks.label);
  });
});
