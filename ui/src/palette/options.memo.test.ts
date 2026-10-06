import { describe, expect, it, vi } from "vitest";
import type { Project, Proposal, TaskListEntry } from "../api/types";
import { aProposal } from "../test/builders";
import { SOME_TASKS } from "../test/taskStub";
import type * as OrderModule from "../inbox/order";
import type * as GroupsModule from "../tasks/groups";
import { paletteGroups, type PaletteInput } from "./options";

// docs/features/ui-home.md "Palette": typing only filters. The Tasks screen's grouping and the
// Inbox's queue order are wrapped to count how often the palette asks for them; the wrappers
// return the real orders.

const ordered = vi.hoisted(() => ({ groups: 0, queue: 0 }));

vi.mock("../tasks/groups", async (importOriginal) => {
  const actual = await importOriginal<typeof GroupsModule>();
  return {
    ...actual,
    groupsOf: (entries: readonly TaskListEntry[]) => {
      ordered.groups += 1;
      return actual.groupsOf(entries);
    },
  };
});

vi.mock("../inbox/order", async (importOriginal) => {
  const actual = await importOriginal<typeof OrderModule>();
  return {
    ...actual,
    queueOrder: (proposals: readonly Proposal[]) => {
      ordered.queue += 1;
      return actual.queueOrder(proposals);
    },
  };
});

const PROJECTS: Project[] = [{ slug: "alpha", name: "Alpha" }];

function input(text: string, tasks: readonly TaskListEntry[], proposals: readonly Proposal[]): PaletteInput {
  return {
    project: "alpha",
    text,
    tasks: { state: "ready", value: tasks },
    inbox: { state: "ready", value: proposals },
    projects: { state: "ready", value: PROJECTS },
    search: null,
  };
}

function keysOf(text: string, tasks: readonly TaskListEntry[], proposals: readonly Proposal[]): string[] {
  return paletteGroups(input(text, tasks, proposals)).flatMap((group) => group.options.map((option) => option.key));
}

describe("the palette's orders, once per answer", () => {
  it("orders the tasks and the queue once per answer while the text changes, again for a new answer", () => {
    const tasks = [...SOME_TASKS];
    const proposals = [aProposal({ id: "PR-2", severity: "low" }), aProposal({ id: "PR-1", severity: "high" })];
    ordered.groups = 0;
    ordered.queue = 0;
    for (const text of ["t", "t-", "t-0", "t-00", "t-000", "t-0002", "pr", "pr-1"]) {
      keysOf(text, tasks, proposals);
    }
    expect(ordered).toEqual({ groups: 1, queue: 1 });
    expect(keysOf("pr-", tasks, proposals).filter((key) => key.startsWith("proposal:"))).toEqual(["proposal:PR-1", "proposal:PR-2"]);
    expect(keysOf("t-0002", tasks, proposals)[0]).toBe("task:T-0002");
    expect(ordered).toEqual({ groups: 1, queue: 1 });

    const fresh = [...proposals, aProposal({ id: "PR-3", severity: "high" })];
    expect(keysOf("pr-", tasks, fresh).filter((key) => key.startsWith("proposal:"))).toEqual(["proposal:PR-1", "proposal:PR-3", "proposal:PR-2"]);
    keysOf("pr-3", [...tasks], fresh);
    expect(ordered).toEqual({ groups: 2, queue: 2 });
  });
});
