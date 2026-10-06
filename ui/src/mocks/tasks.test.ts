import { describe, expect, it, vi } from "vitest";
import type { TaskPackage } from "../api/types";
import { MockClient } from "./MockClient";
import tasksSource from "./tasks.ts?raw";
import { largeTasks, mockTasks, packageOf, taskListOf, type StoredTask } from "./tasks";

// AC-14 and AC-13 of docs/features/ui-tasks.md: the mock's tasks are built from fixed values (two
// builds equal, no clock), name no word of a tool chain or of this repository's roles, hold the
// seven reachable states and every case the slice names; role and profile values are never quoted
// in app code; `large` adds 320 tasks, T-0200 at every cap of the draft package.

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true });

function isAppCode(path: string): boolean {
  return !path.startsWith("/src/mocks/") && !path.startsWith("/src/test/") && !/\.test\.tsx?$/.test(path);
}

const NOW = Date.parse("2026-10-06T12:00:00Z");

function task(slug: string, id: string): StoredTask {
  const found = mockTasks(slug).tasks.find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`no mock task ${id}`);
  }
  return found;
}

async function packageFrom(client: MockClient, project: string, id: string): Promise<TaskPackage> {
  const answer = await client.getTask(project, id);
  if (!("schema_version" in answer)) {
    throw new Error(answer.reason);
  }
  return answer;
}

describe("the mock's tasks are fixed (AC-14)", () => {
  it("build equal twice, and never read the clock", () => {
    const now = vi.spyOn(Date, "now");
    const first = [mockTasks("harbor-sim"), mockTasks("ledger-api"), largeTasks()];
    const second = [mockTasks("harbor-sim"), mockTasks("ledger-api"), largeTasks()];
    expect(second).toEqual(first);
    expect(now).not.toHaveBeenCalled();
    expect(tasksSource).not.toMatch(/\bDate\b|performance\.now|Math\.random/);
  });

  it("name no tool chain word and none of this repository's role names (07 section 1.2, P2-3)", () => {
    const words = ["cargo", "nextest", "clippy", "bevy", "pnpm", "npm", "nest", "react", "jira"];
    const roles = ["requirement-analyst", "spec-writer", "rust-developer", "ui-developer", "test-engineer", "code-reviewer"];
    const lower = tasksSource.toLowerCase();
    expect([...words, ...roles].filter((word) => lower.includes(word))).toEqual([]);
  });

  it("hold the seven states a task reaches in harbor-sim, and a list note for the skipped row", () => {
    const list = taskListOf(mockTasks("harbor-sim"));
    expect(new Set(list.tasks.map((entry) => entry.status))).toEqual(
      new Set(["draft", "review", "changes_requested", "ready", "in_progress", "done", "cancelled"]),
    );
    expect(list.tasks.map((entry) => entry.id)).toEqual(["T-0104", "T-0105", "T-0107", "T-0108", "T-0109", "T-0110", "T-0111", "T-0112", "T-0113"]);
    expect(list.notes[0]).toMatch(/^T-0106: .*; skipped$/);
    expect(list.notes[1]).toMatch(/^T-0111: /);
  });

  it("resolve every task_id of both inboxes to a task of that project", async () => {
    const client = new MockClient("normal", { now: () => NOW });
    for (const project of ["harbor-sim", "ledger-api"]) {
      const { proposals } = await client.getInbox(project);
      const { tasks } = await client.getTasks(project);
      const ids = new Set(tasks.map((entry) => entry.id));
      const bound = proposals.flatMap((proposal) => proposal.task_id ?? []);
      expect(bound.length).toBeGreaterThan(0);
      for (const id of bound) {
        expect([project, id, ids.has(id)]).toEqual([project, id, true]);
      }
    }
  });

  it("answer the list with 329 tasks from one call in the large scenario", async () => {
    const client = new MockClient("large", { now: () => NOW });
    const read = vi.spyOn(client, "getTasks");
    const { tasks } = await client.getTasks("harbor-sim");
    expect(read).toHaveBeenCalledTimes(1);
    expect(tasks).toHaveLength(329);
    expect(tasks[9]?.id).toBe("T-0200");
    expect(tasks.at(-1)?.id).toBe("T-0519");
    expect(new Set(tasks.slice(9).map((entry) => entry.status)).size).toBe(7);
  });
});

describe("the cases the slice names (Data, Mock)", () => {
  it("give T-0107 a plan, PR-0041 and PR-0042 bound, PR-0044 on its nodes, PR-0046 deferred and left out", async () => {
    const pkg = await packageFrom(new MockClient("normal", { now: () => NOW }), "harbor-sim", "T-0107");
    expect(pkg.status).toBe("review");
    expect(pkg.plan).not.toBeNull();
    expect(pkg.open_proposals.map((proposal) => [proposal.id, proposal.task_id])).toEqual([
      ["PR-0041", "T-0107"],
      ["PR-0042", "T-0107"],
      ["PR-0044", "T-0112"],
    ]);
    expect(pkg.assumptions).toEqual([
      { proposal: "PR-0041", text: "Code to spec" },
      { proposal: "PR-0044", text: "Spec to data" },
    ]);
  });

  it("give the staleness cases their shapes", () => {
    const t0108 = task("harbor-sim", "T-0108");
    expect([t0108.status, t0108.stale, t0108.snapshot_diff?.map((entry) => entry.id)]).toEqual(["ready", true, ["MEC-NIGHT-PASSAGE", "RULE-NIGHT-LIGHTS"]]);
    expect(t0108.targets[1]).toEqual({ id: "RULE-NIGHT-LIGHTS", path: "docs/spec/fairway/night.md", kind: null, title: null });
    const t0109 = task("harbor-sim", "T-0109");
    expect([t0109.status, t0109.stale, t0109.runs.map((run) => run.ended_at), t0109.snapshot_diff?.map((entry) => entry.cut)]).toEqual([
      "in_progress",
      true,
      [null],
      [true],
    ]);
    const cut = t0109.snapshot_diff?.[0]?.diff ?? "";
    expect(cut.length).toBeLessThanOrEqual(8192);
    expect(cut.endsWith("\n")).toBe(true);
    const t0111 = task("harbor-sim", "T-0111");
    expect([t0111.status, t0111.stale, t0111.spec_snapshot !== null, t0111.snapshot_diff, t0111.notes.length]).toEqual(["ready", null, true, null, 1]);
    const t0112 = task("harbor-sim", "T-0112");
    expect([t0112.status, t0112.stale, t0112.snapshot_diff]).toEqual(["ready", false, []]);
    const t0113 = task("harbor-sim", "T-0113");
    expect([t0113.status, t0113.spec_snapshot, t0113.title]).toEqual(["draft", null, null]);
    expect(task("harbor-sim", "T-0110").owner_notes).toHaveLength(1);
    expect(task("harbor-sim", "T-0104").runs[0]?.outcome).toBe("completed");
  });

  it("give ledger-api a profile and roles of its own words", () => {
    const ledger = mockTasks("ledger-api").tasks;
    expect(ledger.map((entry) => [entry.id, entry.status, entry.profile])).toEqual([
      ["T-0031", "in_progress", "ledger-http-service"],
      ["T-0033", "review", "ledger-http-service"],
    ]);
    const roles = (tasks: StoredTask[]) => new Set(tasks.flatMap((entry) => [entry.claim?.role ?? [], ...entry.runs.map((run) => run.role)].flat()));
    const harborRoles = roles(mockTasks("harbor-sim").tasks);
    expect([...roles(ledger)].filter((role) => harborRoles.has(role))).toEqual([]);
  });

  it("drop a decided proposal from the package's proposals and assumptions on the next read", async () => {
    const client = new MockClient("normal", { now: () => NOW });
    await client.decideProposal("harbor-sim", "PR-0041", { decision: "reject", reason: "Not this way" });
    const pkg = await packageFrom(client, "harbor-sim", "T-0107");
    expect(pkg.open_proposals.map((proposal) => proposal.id)).toEqual(["PR-0042", "PR-0044"]);
    expect(pkg.assumptions.map((assumption) => assumption.proposal)).toEqual(["PR-0044"]);
  });

  it("answer an unknown T with the exit-1 document, as data", async () => {
    const client = new MockClient("normal", { now: () => NOW });
    expect(await client.getTask("harbor-sim", "T-0999")).toEqual({ id: "T-0999", reason: "no task T-0999 in this repository" });
  });

  it("keep every package key, in the documented order", () => {
    const pkg = packageOf(task("harbor-sim", "T-0113"), []);
    expect(Object.keys(pkg)).toEqual([
      "schema_version",
      "id",
      "project",
      "status",
      "title",
      "goal",
      "profile",
      "stale",
      "targets",
      "criteria",
      "affected_nodes",
      "plan",
      "assumptions",
      "open_proposals",
      "owner_notes",
      "bindings",
      "spec_snapshot",
      "snapshot_diff",
      "claim",
      "runs",
      "bundle",
      "author",
      "created_at",
      "updated_at",
      "notes",
    ]);
  });
});

describe("T-0200 at every cap of the package (task-package Data, Caps)", () => {
  const capped = largeTasks()[0];

  it("fills each capped field to its cap", () => {
    expect(capped?.id).toBe("T-0200");
    const run = capped?.runs[0];
    expect([
      capped?.title?.length,
      capped?.goal?.length,
      capped?.plan?.length,
      capped?.profile?.length,
      capped?.targets.length,
      capped?.affected_nodes.length,
      capped?.criteria.length,
      capped?.spec_snapshot?.nodes.length,
      capped?.owner_notes[0]?.note.length,
      run?.summary?.length,
      run?.changed_files.length,
    ]).toEqual([256, 4096, 16384, 64, 64, 64, 32, 128, 4096, 4096, 256]);
    expect(capped?.criteria.every((criterion) => criterion.text?.length === 1024)).toBe(true);
    expect(run?.changed_files.every((file) => file.length === 512)).toBe(true);
    expect(capped?.snapshot_diff?.every((entry) => entry.cut && entry.diff.length <= 8192 && entry.diff.length > 8000)).toBe(true);
    expect(/^[\x20-\x7e\n]*$/.test(JSON.stringify(capped))).toBe(true);
  });
});

describe("the large scenario's generated tasks", () => {
  const generated = largeTasks().slice(1);

  it("claim each one in progress in its worktree, run 1 open, as T-0109; claim nothing else", () => {
    const working = generated.filter((entry) => entry.status === "in_progress");
    expect(working.length).toBeGreaterThan(40);
    for (const entry of working) {
      const claim = entry.claim;
      expect([entry.id, claim?.worktree, claim?.branch, claim?.at === entry.updated_at, entry.bundle?.node_ids]).toEqual([
        entry.id,
        `/work/harbor-sim/${entry.id}`,
        `task/${entry.id}`,
        true,
        entry.targets.map((target) => target.id),
      ]);
      expect(entry.runs).toEqual([
        { run: 1, role: claim?.role, started_at: claim?.at, ended_at: null, outcome: null, summary: null, changed_files: [] },
      ]);
    }
    const rest = generated.filter((entry) => entry.status !== "in_progress");
    expect(rest.filter((entry) => entry.claim !== null || entry.runs.length > 0 || entry.bundle !== null)).toEqual([]);
  });
});

describe("role and profile values (AC-13)", () => {
  it("are never quoted in app code", () => {
    const tasks = [...mockTasks("harbor-sim").tasks, ...mockTasks("ledger-api").tasks, ...largeTasks().slice(0, 1)];
    const words = new Set(
      tasks.flatMap((entry) => [
        entry.profile ?? [],
        entry.claim?.role ?? [],
        entry.author?.role ?? [],
        ...entry.runs.map((run) => run.role),
        ...entry.targets.flatMap((target) => target.kind ?? []),
      ].flat()),
    );
    expect(words.size).toBeGreaterThan(5);
    const hits: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (!isAppCode(path)) {
        continue;
      }
      for (const word of words) {
        const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        if (new RegExp(`["'\`]${escaped}["'\`]`).test(text)) {
          hits.push(`${path}: ${word}`);
        }
      }
    }
    expect(hits).toEqual([]);
  });
});
