import { describe, expect, it } from "vitest";
import source from "./actions.ts?raw";
import { actionsFor, ownerCommands, TASK_ID } from "./actions";

// AC-09 of docs/features/ui-tasks.md: per state exactly its row of the transition table
// (`docs/canon/tasks.md` "Transitions", owner rows), then `spec task show`; fixed words and a
// checked ID, the note never composed into a command.

const TABLE: [string, string[]][] = [
  ["draft", ["approve", "cancel"]],
  ["changes_requested", ["approve", "cancel"]],
  ["review", ["approve", "changes", "cancel"]],
  ["ready", ["approve", "cancel"]],
  ["in_progress", ["cancel"]],
  ["done", []],
  ["cancelled", []],
  ["analysis", []],
  ["in_review", []],
  ["accepted", []],
  ["triage", []],
];

describe("the owner's actions per state (AC-09)", () => {
  it.each(TABLE)("%s: %j", (status, actions) => {
    expect(actionsFor(status)).toEqual(actions);
    expect(ownerCommands("T-0107", status)?.map((command) => command.action)).toEqual([...actions, "show"]);
  });

  it("words T-0107's commands exactly, the note a placeholder typed on the terminal", () => {
    expect(ownerCommands("T-0107", "review")?.map((command) => command.text)).toEqual([
      "spec task approve T-0107",
      'spec task changes T-0107 --note "..."',
      "spec task cancel T-0107",
      "spec task show T-0107",
    ]);
  });

  it("says approval again re-freezes the snapshot for a ready task", () => {
    expect(ownerCommands("T-0108", "ready")?.[0]?.meaning).toMatch(/re-freezes the snapshot/);
  });

  it("offers no command for an ID that is not a task ID", () => {
    for (const id of ["T-1; rm -rf ~", "T-1", "T-0107 ", "t-0107", "T-0107\nrm", "$(T-0107)", "T-01a7", ""]) {
      expect([id, ownerCommands(id, "review")]).toEqual([id, null]);
    }
    expect(TASK_ID.test("T-12345")).toBe(true);
  });

  it("cites the transition table and composes nothing but fixed words and the ID", () => {
    expect(source).toContain('`docs/canon/tasks.md` "Transitions"');
    const templates = [...source.matchAll(/`spec task [^`]*\$\{[^`]*`/g)].map((match) => match[0]);
    expect(templates.sort()).toEqual([
      "`spec task approve ${id}`",
      '`spec task changes ${id} --note "..."`',
      "`spec task cancel ${id}`",
      "`spec task show ${id}`",
    ].sort());
  });
});
