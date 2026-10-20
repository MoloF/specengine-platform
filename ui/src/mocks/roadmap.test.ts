import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { RoadmapDocument, TaskPackage } from "../api/types";
import { MockClient } from "./MockClient";
import { GREEK_TITLE, JAPANESE_TITLE, LARGE_PLAN_ITEMS, noPlanMessage, UNKNOWN_ITEM_STATUS } from "./roadmap";
import { SCENARIOS, type Scenario } from "./scenario";

// UI-6 of docs/features/roadmap-screen.md: the mock plans. In every scenario each open mock task
// appears once, on an item or among the pool tasks, and its package agrees; ledger-api, with no
// `[roadmap]`, answers 503 in the CLI's words in every scenario; harbor-sim's normal plan holds
// each case the screen draws; `empty` has no item, `large` 500, no lane over 200. Every document
// has the daemon's keys, the sums and counts adding up.

const NOW = Date.parse("2026-10-06T12:00:00Z");

/** A task is open unless done, accepted or cancelled (`docs/canon/spec-roadmap.md` "Position"). */
const CLOSED = new Set(["done", "accepted", "cancelled"]);

/** Scenarios whose reads answer (`error` refuses every read). */
const ANSWERING = SCENARIOS.filter((scenario) => scenario !== "error");

async function rejection(promise: Promise<unknown>): Promise<ClientError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ClientError) {
      return error;
    }
    throw error;
  }
  throw new Error("the call resolved");
}

/** The mock for a scenario, without `slow`'s delay (its plan is normal's). */
function client(scenario: Scenario) {
  return new MockClient(scenario, { now: () => NOW, delayMs: 0 });
}

async function packageOf(mock: MockClient, id: string): Promise<TaskPackage> {
  const answer = await mock.getTask("harbor-sim", id);
  if (!("schema_version" in answer)) {
    throw new Error(answer.reason);
  }
  return answer;
}

function itemsOf(plan: RoadmapDocument) {
  return plan.groups.flatMap((group) => group.items);
}

describe("the mock plans' tasks (UI-6)", () => {
  it.each(ANSWERING)("%s: each open task once, on an item or a pool task, its package agreeing", async (scenario) => {
    const mock = client(scenario);
    const plan = await mock.getRoadmap("harbor-sim");
    const list = await mock.getTasks("harbor-sim");
    const open = list.tasks.filter((task) => !CLOSED.has(task.status)).map((task) => task.id);
    const onItems = plan.groups.flatMap((group) => group.items.flatMap((item) => item.tasks.map((task) => task.id)));
    const inPool = plan.pool_tasks.map((task) => task.id);
    expect([...onItems, ...inPool].sort()).toEqual([...open].sort());
    expect(new Set([...onItems, ...inPool]).size).toBe(open.length);
    for (const group of plan.groups) {
      for (const item of group.items) {
        for (const task of item.tasks) {
          const pkg = await packageOf(mock, task.id);
          expect([task.id, pkg.status, pkg.roadmap]).toEqual([
            task.id,
            task.status,
            { item: item.id, group: group.group, release: item.release, position: item.position, of: group.open, priority: item.priority, waits_on: item.waits_on },
          ]);
        }
      }
    }
    for (const task of plan.pool_tasks.slice(0, 20)) {
      const pkg = await packageOf(mock, task.id);
      expect([task.id, pkg.roadmap, pkg.status, pkg.title, pkg.targets.map((target) => target.id ?? target.path)]).toEqual([
        task.id,
        null,
        task.status,
        task.title,
        task.targets,
      ]);
    }
  });

  it("puts T-0109 (in progress) and T-0112 (ready) on items in normal, every other open task in the pool", async () => {
    const plan = await client("normal").getRoadmap("harbor-sim");
    expect(itemsOf(plan).flatMap((item) => item.tasks.map((task) => [item.id, task.id, task.status]))).toEqual([
      ["PLAN-0006", "T-0109", "in_progress"],
      ["PLAN-0012", "T-0112", "ready"],
    ]);
    expect(plan.pool_tasks.map((task) => task.id)).toEqual(["T-0107", "T-0108", "T-0110", "T-0111", "T-0113"]);
    expect(plan.pool_tasks.find((task) => task.id === "T-0113")?.title).toBeNull();
  });

  it("has no plan for ledger-api: 503 in the CLI's words in every scenario", async () => {
    for (const scenario of SCENARIOS) {
      const error = await rejection(client(scenario).getRoadmap("ledger-api"));
      expect([scenario, error.status]).toEqual([scenario, 503]);
      if (scenario !== "error") {
        expect(error.message).toBe("spec: specengine.toml has no [roadmap]");
      }
    }
    expect(noPlanMessage()).toBe("spec: specengine.toml has no [roadmap]");
    const missing = await client("normal").getTask("ledger-api", "T-0031");
    expect("roadmap" in missing ? missing.roadmap : "none").toBeNull();
  });
});

describe("harbor-sim's plan (UI-6)", () => {
  it("holds every case the screen draws in normal", async () => {
    const plan = await client("normal").getRoadmap("harbor-sim");
    const releases = plan.groups.filter((group) => group.group === "release");
    expect(plan.groups.map((group) => group.group)).toEqual(["release", "release", "release", "pool", "unplaced"]);
    // The current release and the cut on two lanes; an empty release.
    const current = releases.findIndex((group) => group.current);
    const cut = releases.findIndex((group) => group.mvp_cut);
    expect(current).toBeGreaterThanOrEqual(0);
    expect(cut).toBeGreaterThanOrEqual(0);
    expect(current).not.toBe(cut);
    expect(releases.some((group) => group.items.length === 0)).toBe(true);
    expect(plan.current).toBe(releases[current]?.name);
    expect(plan.cut?.release).toBe(releases[cut]?.name);
    const items = itemsOf(plan);
    const groupOf = new Map(plan.groups.flatMap((group, index) => group.items.map((item) => [item.id, index])));
    // A chain, a wait across lanes (release to release and the pool to a release), a two-item and a one-item cycle.
    const pairs = items.flatMap((item) => item.waits_on.map((from) => [from, item.id] as const));
    expect(pairs.some(([from, to]) => from !== to && groupOf.get(from) === groupOf.get(to))).toBe(true);
    const across = pairs.filter(([from, to]) => groupOf.get(from) !== groupOf.get(to)).map(([from, to]) => [plan.groups[groupOf.get(from) ?? 0]?.group, plan.groups[groupOf.get(to) ?? 0]?.group]);
    expect(across).toEqual(expect.arrayContaining([["release", "release"], ["release", "pool"]]));
    expect(items.filter((item) => item.cycle && item.waits_on.includes(item.id)).map((item) => item.id)).toEqual(["PLAN-0013"]);
    expect(items.filter((item) => item.cycle && !item.waits_on.includes(item.id)).map((item) => item.id)).toEqual(["PLAN-0008", "PLAN-0009"]);
    // Unestimated items, an off-scale priority, a status of its own, the two titles, an unplaced item of each kind.
    expect(items.filter((item) => item.estimate === null).length).toBeGreaterThanOrEqual(2);
    expect(items.map((item) => item.status)).toContain(UNKNOWN_ITEM_STATUS);
    expect(items.map((item) => item.title)).toEqual(expect.arrayContaining([JAPANESE_TITLE, GREEK_TITLE]));
    const unplaced = plan.groups.find((group) => group.group === "unplaced")?.items ?? [];
    expect(unplaced.map((item) => item.release === null)).toEqual(expect.arrayContaining([true, false]));
    expect(plan.notes.length).toBeGreaterThanOrEqual(4);
    expect([...plan.notes].sort()).toEqual(plan.notes);
    expect([plan.prefix, plan.dir, plan.truncated, plan.omitted]).toEqual(["PLAN", "docs/plan", false, 0]);
  });

  it.each(ANSWERING)("%s: counts, sums, positions and the cut add up as the plan's", async (scenario) => {
    const plan = await client(scenario).getRoadmap("harbor-sim");
    const sum = (items: ReturnType<typeof itemsOf>) =>
      items.reduce(
        (total, item) => (item.estimate === null ? { ...total, unestimated: total.unestimated + 1 } : { ...total, min: total.min + item.estimate.min, max: total.max + item.estimate.max }),
        { min: 0, max: 0, unestimated: 0 },
      );
    for (const group of plan.groups) {
      expect([group.name, group.open, group.sum]).toEqual([group.name, group.items.length, sum(group.items)]);
      expect(group.items.map((item) => item.position)).toEqual(group.items.map((_, index) => (group.group === "release" ? index + 1 : null)));
    }
    expect(plan.counts.open).toBe(itemsOf(plan).length);
    expect(plan.counts.done).toBe(plan.groups.reduce((count, group) => count + group.done, 0));
    const releases = plan.groups.filter((group) => group.group === "release");
    expect(plan.total).toEqual(sum(releases.flatMap((group) => group.items)));
    const cutAt = releases.findIndex((group) => group.mvp_cut);
    expect(plan.cut).toEqual({ release: releases[cutAt]?.name, ...sum(releases.slice(0, cutAt + 1).flatMap((group) => group.items)) });
    expect(plan.current).toBe(releases.find((group) => group.items.length > 0)?.name ?? null);
    const listed = new Set(itemsOf(plan).map((item) => item.id));
    expect(itemsOf(plan).flatMap((item) => item.waits_on).filter((id) => !listed.has(id))).toEqual([]);
  });

  it("has no item in empty, 500 in large with no lane over 200", async () => {
    const empty = await client("empty").getRoadmap("harbor-sim");
    expect([empty.counts, itemsOf(empty).length, empty.pool_tasks, empty.notes]).toEqual([{ open: 0, done: 0 }, 0, [], []]);
    expect(empty.groups.map((group) => group.group)).toEqual(["release", "release", "release", "pool", "unplaced"]);
    const large = await client("large").getRoadmap("harbor-sim");
    expect(itemsOf(large)).toHaveLength(LARGE_PLAN_ITEMS);
    expect(Math.max(...large.groups.map((group) => group.items.length))).toBeLessThanOrEqual(200);
    expect(new Set(itemsOf(large).map((item) => item.id)).size).toBe(LARGE_PLAN_ITEMS);
  });

  it("names no title with a word of deleting, moving, dragging or connecting", async () => {
    for (const scenario of ANSWERING) {
      const plan = await client(scenario).getRoadmap("harbor-sim");
      const titles = [...itemsOf(plan).map((item) => item.title ?? ""), ...plan.pool_tasks.map((task) => task.title ?? "")];
      expect([scenario, titles.filter((title) => /delete|move|drag|connect/i.test(title))]).toEqual([scenario, []]);
    }
  });

  it("keeps a release's group for `release`, the pool and unplaced for `pool`, the whole plan's line either way; an unknown release is the exit-1 document", async () => {
    const mock = client("normal");
    const whole = await mock.getRoadmap("harbor-sim");
    const named = whole.groups[1]?.name ?? "";
    const one = await mock.getRoadmap("harbor-sim", { release: named });
    expect([one.release, one.groups.map((group) => group.name), one.counts, one.total]).toEqual([named, [named], whole.counts, whole.total]);
    const pool = await mock.getRoadmap("harbor-sim", { pool: true });
    expect([pool.pool, pool.groups.map((group) => group.group)]).toEqual([true, ["pool", "unplaced"]]);
    const none = await mock.getRoadmap("harbor-sim", { release: "nope" });
    expect([none.reason?.startsWith('no release "nope" in [roadmap] releases ('), none.groups]).toEqual([true, []]);
  });

  it("refuses every read in error, the scenario's 503", async () => {
    const error = await rejection(client("error").getRoadmap("harbor-sim"));
    expect(error.status).toBe(503);
  });

  it("ignores the signal and is ASCII in its source (escapes only)", async () => {
    const plan = await client("normal").getRoadmap("harbor-sim", undefined);
    expect(plan.groups.length).toBe(5);
    const source = Object.values(import.meta.glob<string>("/src/mocks/roadmap.ts", { query: "?raw", import: "default", eager: true }))[0] ?? "";
    expect(/^[\x20-\x7e\n]*$/.test(source)).toBe(true);
  });
});
