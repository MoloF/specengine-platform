import { ClientError } from "../api/client";
import type { RoadmapDocument, RoadmapGroup, RoadmapItem, RoadmapPoolTask, RoadmapSum, TaskRoadmap } from "../api/types";
import type { Scenario } from "./scenario";
import type { MockTasks, StoredTask } from "./tasks";

// The plans of the mock projects (docs/features/roadmap-screen.md, Mock plans): `spec roadmap
// --json` as the daemon's browser view sends it (`docs/canon/spec-roadmap.md` "Command"), uncut,
// built from stored items placed by hand in the order `plan` gives them ("Plan" steps 3, 5, 6),
// with the sums, counts, current release, cut and the task join ("Position") computed here, as the
// core does.
//
// harbor-sim's `[roadmap]` has its own vocabulary, quoted here alone (never in app code): releases
// keel, hull, sails; `mvp_cut` keel; priorities gale, breeze, calm; unit tides. Its plan: the cut
// on keel, an empty release, so the current release (hull) is not the first; a chain, an item
// waiting on one in another lane (release to release, the pool to a release), a two-item cycle
// and a one-item one, unestimated items (one's estimate unreadable), an off-scale priority and an
// item status of its own, an unplaced item of each kind (a release not listed, one not a string),
// a Japanese and a Greek title (escapes: mocks are ASCII), pool items and pool tasks, notes; two
// open tasks execute items, T-0109 in progress and T-0112 ready (src/mocks/tasks.ts), every other
// open task is a pool task. `empty`: the table, no item. `large`: 500 open items, no lane over
// 200. ledger-api has no `[roadmap]`: 503 in every scenario, the CLI's words.

/** harbor-sim's `[roadmap] releases`, in order. */
export const PLAN_RELEASES = ["keel", "hull", "sails"] as const;

/** harbor-sim's `[roadmap] priorities`, highest first. */
export const PLAN_PRIORITIES = ["gale", "breeze", "calm"] as const;

/** harbor-sim's `[roadmap] unit`, printed after every sum. */
export const PLAN_UNIT = "tides";

/** harbor-sim's `[roadmap] mvp_cut`. */
export const PLAN_MVP_CUT = "keel";

/** The plan's `[roadmap] prefix` and `dir`. */
const PREFIX = "PLAN";
const DIR = "docs/plan";

/** Release names by their place in harbor-sim's `[roadmap] releases`. */
const [KEEL, HULL, SAILS] = PLAN_RELEASES;

/** Priority names, highest first. */
const [GALE, BREEZE, CALM] = PLAN_PRIORITIES;

/** An item as written in its file: the plan's line for it less what the plan computes. */
interface StoredItem {
  id: string;
  title: string | null;
  status: string | null;
  /** `release:` as written; null when absent or no string. */
  release: string | null;
  priority: string | null;
  estimate: { min: number; max: number } | null;
  /** The open items it depends on, as `waits_on` lists them. */
  waitsOn: string[];
  cycle: boolean;
  /** Root-relative; by convention `<dir>/<ID>.md`. */
  path?: string;
}

/** A group as stored: its open items in the plan's order and its count of done items. */
interface StoredGroup {
  group: "release" | "pool" | "unplaced";
  name: string | null;
  done: number;
  items: StoredItem[];
}

/** A stored plan: the config's prefix, dir, unit and cut, the groups, the plan's notes (sorted). */
export interface MockPlan {
  prefix: string;
  dir: string;
  unit: string | null;
  mvpCut: string | null;
  groups: StoredGroup[];
  notes: string[];
}

function item(id: string, fields: Partial<StoredItem> = {}): StoredItem {
  return {
    id,
    title: fields.title === undefined ? `Title of ${id}` : fields.title,
    status: fields.status === undefined ? "draft" : fields.status,
    release: fields.release ?? null,
    priority: fields.priority ?? null,
    estimate: fields.estimate ?? null,
    waitsOn: fields.waitsOn ?? [],
    cycle: fields.cycle ?? false,
    ...(fields.path === undefined ? {} : { path: fields.path }),
  };
}

function range(min: number, max: number = min) {
  return { min, max };
}

/** "Harbour weather forecast" in Japanese. */
export const JAPANESE_TITLE = "\u6e2f\u306e\u5929\u6c17\u4e88\u5831";

/** "Lighthouses of the harbour" in Greek. */
export const GREEK_TITLE = "\u03a6\u03ac\u03c1\u03bf\u03b9 \u03c4\u03bf\u03c5 \u03bb\u03b9\u03bc\u03b1\u03bd\u03b9\u03bf\u03cd";

/** An item status outside the spec statuses, kept as written (the check flags it, the plan lists it open). */
export const UNKNOWN_ITEM_STATUS = "parked";

/** The two items the mock's tasks execute: T-0109's and T-0112's (src/mocks/tasks.ts). */
export const TIDE_TABLES_ITEM = "PLAN-0006";
export const DEEP_BERTH_ITEM = "PLAN-0012";

/** The items of harbor-sim's plan as `plan` orders them. */
function harborPlan(): MockPlan {
  return {
    prefix: PREFIX,
    dir: DIR,
    unit: PLAN_UNIT,
    mvpCut: PLAN_MVP_CUT,
    groups: [
      // The cut's release, every item of it done: empty, so the current release is the next one.
      { group: "release", name: KEEL, done: 3, items: [] },
      {
        group: "release",
        name: HULL,
        done: 1,
        items: [
          item(TIDE_TABLES_ITEM, { title: "Tide tables from both survey formats", status: "in-progress", release: HULL, priority: GALE, estimate: range(3, 5) }),
          item("PLAN-0007", { title: "Pilot boat dispatch", release: HULL, priority: GALE, estimate: range(2), waitsOn: [TIDE_TABLES_ITEM] }),
          // `estimate: soon`: not N or N-M, so unestimated, with a note.
          item("PLAN-0005", { title: "Harbour master's daily log", release: HULL, priority: BREEZE }),
          item("PLAN-0008", { title: "Night lights on the fairway", release: HULL, priority: BREEZE, estimate: range(1, 2), waitsOn: ["PLAN-0009"], cycle: true }),
          item("PLAN-0009", { title: "Fairway speed signs", release: HULL, priority: BREEZE, estimate: range(1), waitsOn: ["PLAN-0008"], cycle: true }),
          item("PLAN-0010", { title: JAPANESE_TITLE, status: UNKNOWN_ITEM_STATUS, release: HULL, priority: "urgent" }),
        ],
      },
      {
        group: "release",
        name: SAILS,
        done: 0,
        items: [
          item(DEEP_BERTH_ITEM, { title: "Deep-draft berth 4", release: SAILS, priority: GALE, estimate: range(4, 6), waitsOn: ["PLAN-0007"] }),
          item("PLAN-0013", { title: GREEK_TITLE, release: SAILS, priority: CALM, estimate: range(2), waitsOn: ["PLAN-0013"], cycle: true }),
          item("PLAN-0014", { title: "Cargo crane night shift", release: SAILS, estimate: range(3), waitsOn: [DEEP_BERTH_ITEM, "PLAN-0008"] }),
        ],
      },
      {
        group: "pool",
        name: null,
        done: 1,
        items: [
          item("PLAN-0020", { title: "Seagull ambience", priority: CALM, estimate: range(1) }),
          item("PLAN-0021", { title: "Photo mode for the quays", waitsOn: ["PLAN-0014"] }),
        ],
      },
      {
        group: "unplaced",
        name: null,
        done: 0,
        items: [
          // `release: [2027]`: no string, so null, and "release is not a string".
          item("PLAN-0031", { title: "Harbour festival", priority: GALE, estimate: range(2) }),
          item("PLAN-0030", { title: "Ferry routes to the islands", release: "someday", priority: BREEZE, estimate: range(5, 8) }),
        ],
      },
    ],
    notes: [
      'PLAN-0005: estimate "soon" is not N or N-M',
      'PLAN-0010: priority "urgent" is not in [roadmap] priorities',
      'PLAN-0030: release "someday" is not in [roadmap] releases',
      "PLAN-0031: release is not a string",
      "depends_on cycle: PLAN-0008, PLAN-0009",
      "depends_on cycle: PLAN-0013",
    ],
  };
}

/** Items the `large` plan holds in all. */
export const LARGE_PLAN_ITEMS = 500;

/** The groups the `large` plan adds generated items to, and how many each takes. */
const LARGE_SHARES: readonly (readonly [string, number])[] = [
  [HULL, 150],
  [SAILS, 150],
];

/**
 * The `large` plan: harbor-sim's, its open items brought to 500 (no lane over 200): 150 generated
 * after hull's, 150 after sails', the rest after the pool's; each waits on the one before it in its
 * lane, every fifth also on the last of the lane before. No priority, so they follow the written
 * items, by ID.
 */
function largePlan(): MockPlan {
  const plan = harborPlan();
  const listed = plan.groups.reduce((count, group) => count + group.items.length, 0);
  const shares = new Map<StoredGroup, number>();
  let left = LARGE_PLAN_ITEMS - listed;
  for (const group of plan.groups) {
    const share = LARGE_SHARES.find(([name]) => group.group === "release" && group.name === name)?.[1];
    if (share !== undefined) {
      shares.set(group, share);
      left -= share;
    }
  }
  const pool = plan.groups.find((group) => group.group === "pool");
  if (pool !== undefined) {
    shares.set(pool, left);
  }
  let next = 1000;
  let before: StoredGroup | null = null;
  for (const group of plan.groups) {
    const count = shares.get(group) ?? 0;
    for (let at = 0; at < count; at += 1) {
      const id = `${PREFIX}-${String(next)}`;
      const last = group.items.at(-1)?.id;
      const across = at % 5 === 4 ? before?.items.at(-1)?.id : undefined;
      group.items.push(
        item(id, {
          title: `Generated step ${String(next - 999)}`,
          release: group.name,
          estimate: next % 7 === 0 ? null : range(1, 1 + (next % 3)),
          waitsOn: [...(last === undefined ? [] : [last]), ...(across === undefined ? [] : [across])],
        }),
      );
      next += 1;
    }
    if (count > 0) {
      before = group;
    }
  }
  return plan;
}

/** The `empty` plan: the table and its releases, no item. */
function emptyPlan(): MockPlan {
  const plan = harborPlan();
  return { ...plan, groups: plan.groups.map((group) => ({ ...group, done: 0, items: [] })), notes: [] };
}

/** The plan a project has in a scenario; null when it has no `[roadmap]`. */
export function mockPlan(slug: string, scenario: Scenario): MockPlan | null {
  if (slug !== "harbor-sim") {
    return null;
  }
  if (scenario === "empty") {
    return emptyPlan();
  }
  return scenario === "large" ? largePlan() : harborPlan();
}

/** Why a project without `[roadmap]` has no plan: `spec roadmap`'s exit 2, as the daemon's 503 carries it. */
export function noPlanMessage(): string {
  return "spec: specengine.toml has no [roadmap]";
}

/** The refusal of a plan read in a project without `[roadmap]`. */
export function noPlan(): ClientError {
  return new ClientError({ status: 503, message: noPlanMessage() });
}

/** A task is open unless it is done, accepted or cancelled (`docs/canon/spec-roadmap.md` "Position"). */
const CLOSED = new Set(["done", "accepted", "cancelled"]);

/** A stored task's targets as stored: canonical IDs or paths. */
function targetsOf(stored: StoredTask): string[] {
  return stored.targets.flatMap((target) => target.id ?? target.path ?? []);
}

/** The open tasks by number. */
function openTasks(tasks: MockTasks): StoredTask[] {
  return tasks.tasks.filter((stored) => !CLOSED.has(stored.status)).sort((a, b) => a.id.localeCompare(b.id, "en", { numeric: true }));
}

/** The item a task executes: the first open item among its targets in the plan's order; null for a pool task. */
function executedItem(plan: MockPlan, stored: StoredTask): string | null {
  const targets = new Set(targetsOf(stored));
  for (const group of plan.groups) {
    for (const entry of group.items) {
      if (targets.has(entry.id)) {
        return entry.id;
      }
    }
  }
  return null;
}

function sumOf(items: readonly StoredItem[]): RoadmapSum {
  const sum: RoadmapSum = { min: 0, max: 0, unestimated: 0 };
  for (const entry of items) {
    if (entry.estimate === null) {
      sum.unestimated += 1;
    } else {
      sum.min += entry.estimate.min;
      sum.max += entry.estimate.max;
    }
  }
  return sum;
}

/** `spec roadmap --json` of the plan with the project's tasks joined ("Position"), every key present. */
export function roadmapOf(plan: MockPlan, tasks: MockTasks): RoadmapDocument {
  const open = openTasks(tasks);
  const executing = new Map<string, StoredTask[]>();
  const pool: RoadmapPoolTask[] = [];
  for (const stored of open) {
    const executed = executedItem(plan, stored);
    if (executed === null) {
      pool.push({ id: stored.id, status: stored.status, title: stored.title, targets: targetsOf(stored) });
    } else {
      executing.set(executed, [...(executing.get(executed) ?? []), stored]);
    }
  }
  const releases = plan.groups.filter((group) => group.group === "release");
  const current = releases.find((group) => group.items.length > 0)?.name ?? null;
  const cutAt = plan.mvpCut === null ? -1 : releases.findIndex((group) => group.name === plan.mvpCut);
  const groups: RoadmapGroup[] = plan.groups.map((group) => ({
    group: group.group,
    name: group.name,
    current: group.group === "release" && group.name === current,
    mvp_cut: group.group === "release" && group.name === plan.mvpCut,
    open: group.items.length,
    done: group.done,
    sum: sumOf(group.items),
    items: group.items.map(
      (entry, index): RoadmapItem => ({
        position: group.group === "release" ? index + 1 : null,
        id: entry.id,
        title: entry.title,
        path: entry.path ?? `${plan.dir}/${entry.id}.md`,
        status: entry.status,
        release: entry.release,
        priority: entry.priority,
        estimate: entry.estimate === null ? null : { ...entry.estimate },
        waits_on: [...entry.waitsOn],
        cycle: entry.cycle,
        tasks: (executing.get(entry.id) ?? []).map((stored) => ({ id: stored.id, status: stored.status })),
      }),
    ),
  }));
  const cutItems = cutAt < 0 ? null : releases.slice(0, cutAt + 1).flatMap((group) => group.items);
  return {
    release: null,
    pool: false,
    reason: null,
    prefix: plan.prefix,
    dir: plan.dir,
    unit: plan.unit,
    current,
    counts: {
      open: plan.groups.reduce((count, group) => count + group.items.length, 0),
      done: plan.groups.reduce((count, group) => count + group.done, 0),
    },
    cut: cutItems === null || plan.mvpCut === null ? null : { release: plan.mvpCut, ...sumOf(cutItems) },
    total: sumOf(releases.flatMap((group) => group.items)),
    groups,
    pool_tasks: pool,
    notes: [...plan.notes],
    truncated: false,
    omitted: 0,
  };
}

/**
 * The package's `roadmap` for a task (`docs/canon/task-package.md` "Package"): the open item it
 * executes and its group, from the same plan; null for a pool task or a project with no plan.
 */
export function taskPlaceOf(document: RoadmapDocument | null, id: string): TaskRoadmap | null {
  if (document === null) {
    return null;
  }
  for (const group of document.groups) {
    const found = group.items.find((entry) => entry.tasks.some((task) => task.id === id));
    if (found !== undefined) {
      return {
        item: found.id,
        group: group.group,
        release: found.release,
        position: found.position,
        of: group.open,
        priority: found.priority,
        waits_on: [...found.waits_on],
      };
    }
  }
  return null;
}
