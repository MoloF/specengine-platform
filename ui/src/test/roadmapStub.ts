import type { RoadmapDocument, RoadmapItemTask } from "../api/types";
import { aRoadmap, aRoadmapGroup, aRoadmapItem, sumOfItems } from "./builders";
import { stubClient } from "./stubClient";

// A plan on the stub client for the Roadmap's tests (docs/features/roadmap-screen.md), in the
// vocabulary of fixtures/roadmap-a: alpha all done (an empty release), beta current and the MVP
// cut, launch with a two-item cycle, a one-item cycle and a wait across lanes, the pool with an
// item waiting on a release's and two pool tasks, unplaced items of both kinds; every task state
// and one unknown on beta's items; an item status of its own and one absent.

/** Every task state, then one no table holds. */
export const EVERY_TASK_STATE: RoadmapItemTask[] = [
  { id: "T-0001", status: "draft" },
  { id: "T-0002", status: "analysis" },
  { id: "T-0003", status: "review" },
  { id: "T-0004", status: "changes_requested" },
  { id: "T-0005", status: "ready" },
  { id: "T-0006", status: "in_progress" },
  { id: "T-0007", status: "in_review" },
  { id: "T-0008", status: "done" },
  { id: "T-0009", status: "accepted" },
  { id: "T-0010", status: "cancelled" },
  { id: "T-0011", status: "paused" },
];

/** The labels taskStatusLook gives them, in order; the unknown one raw. */
export const EVERY_TASK_LABEL = [
  "Draft",
  "Analysis",
  "Plan review",
  "Changes requested",
  "Ready",
  "In progress",
  "Work in review",
  "Done",
  "Accepted",
  "Cancelled",
  "paused",
];

export function aPlan(): RoadmapDocument {
  const beta = aRoadmapGroup(
    "release",
    "beta",
    [
      aRoadmapItem({ id: "RM-0017", title: "Save and load", priority: "must", estimate: { min: 5, max: 5 }, tasks: EVERY_TASK_STATE.slice(0, 5) }),
      aRoadmapItem({ id: "RM-0010", title: "Tide engine core", status: "in-progress", priority: "could", estimate: { min: 2, max: 4 }, tasks: EVERY_TASK_STATE.slice(5) }),
      aRoadmapItem({ id: "RM-0011", title: "Boat physics", priority: "should", estimate: { min: 3, max: 3 }, waits_on: ["RM-0010"] }),
      aRoadmapItem({ id: "RM-0013", title: "Dock layout pass", status: "parked", priority: "should" }),
    ],
    { current: true, mvp_cut: true, done: 1 },
  );
  const launch = aRoadmapGroup(
    "release",
    "launch",
    [
      aRoadmapItem({ id: "RM-0030", title: "Ferry routes", priority: "must", estimate: { min: 2, max: 2 } }),
      aRoadmapItem({ id: "RM-0021", title: "Ferry ticket booth", priority: "should", estimate: { min: 1, max: 1 }, waits_on: ["RM-0020"], cycle: true }),
      aRoadmapItem({ id: "RM-0020", title: "Ferry crew roster", priority: "could", estimate: { min: 3, max: 5 }, waits_on: ["RM-0021"], cycle: true }),
      aRoadmapItem({ id: "RM-0022", title: "Ferry launch event", priority: "must", estimate: { min: 2, max: 3 }, waits_on: ["RM-0030", "RM-0020", "RM-0011"] }),
      aRoadmapItem({ id: "RM-0060", title: "Weather sync", priority: "could", estimate: { min: 1, max: 1 }, waits_on: ["RM-0060"], cycle: true }),
    ],
    { done: 1 },
  );
  const alpha = aRoadmapGroup("release", "alpha", [], { done: 2 });
  const pool = aRoadmapGroup(
    "pool",
    null,
    [
      aRoadmapItem({ id: "RM-0040", title: "Fishing side quest", priority: "should", estimate: { min: 5, max: 5 }, waits_on: ["RM-0022"] }),
      aRoadmapItem({ id: "RM-0041", title: null, status: null }),
    ],
    { done: 1 },
  );
  const unplaced = aRoadmapGroup("unplaced", null, [
    aRoadmapItem({ id: "RM-0050", title: "Co-op mode", release: "someday", priority: "must", estimate: { min: 2, max: 6 } }),
    aRoadmapItem({ id: "RM-0051", title: "Harbor festival", release: null }),
  ]);
  return aRoadmap([alpha, beta, launch, pool, unplaced], {
    cut: { release: "beta", ...sumOfItems(beta.items) },
    pool_tasks: [
      { id: "T-0020", status: "ready", title: "Harbor zone pass", targets: ["BAY-HARBOR", "docs/spec/bay.md"] },
      { id: "T-0021", status: "draft", title: null, targets: [] },
    ],
    notes: ["RM-0050: release \"someday\" is not in [roadmap] releases", "depends_on cycle: RM-0020, RM-0021"],
  });
}

/** aPlan's header line, as `spec roadmap` prints its first line less `roadmap: `. */
export const PLAN_LINE = "13 open, 5 done; current beta; to beta 10-12 days, 1 unestimated; total 19-24 days, 1 unestimated";

/** A stub client serving `plan` (aPlan by default) for every project. */
export function roadmapClient(plan: RoadmapDocument = aPlan()) {
  const client = stubClient();
  client.getRoadmap.mockImplementation(() => Promise.resolve(structuredClone(plan)));
  return client;
}
