import type { DeliveryDocument } from "../api/types";
import { aDeliveryCounts, aDeliveryLate } from "./builders";
import { roadmapClient } from "./roadmapStub";

// The Delivery tab's documents on the stub client
// (docs/features/delivery-analytics.md "Acceptance criteria", DA-12): the spec's scenario at
// 2026-11-20 as its JSON pins it, and the same with every
// other shape the tab draws: the pool, the unplaced items and a group word this build does not know,
// a department of zeros, an open task with no reason and none taken, a re-estimate to a clear from
// none, every list cut (`more` above 0), an item kind this build does not know, a non-Latin title,
// and a note.

/** The day the spec's scenario is pinned at. */
export const PINNED_AS_OF = "2026-11-20";

/** The spec's JSON, verbatim. */
export function pinnedDelivery(): DeliveryDocument {
  return {
    as_of: "2026-11-20",
    notes: [],
    releases: [
      { group: "release", release: "r1", tasks: 1, delivered: 1, delivered_late: 1, open: 0, overdue: 0, cancelled: 1, lead_days: { median: 14, max: 14 } },
      { group: "release", release: "r2", tasks: 1, delivered: 0, delivered_late: 0, open: 1, overdue: 1, cancelled: 0, lead_days: null },
      { group: "none", release: null, tasks: 2, delivered: 1, delivered_late: 0, open: 1, overdue: 0, cancelled: 0, lead_days: { median: 3, max: 3 } },
    ],
    departments: [
      { name: "words", title: "Words", tasks: 2, delivered: 2, delivered_late: 1, open: 0, overdue: 0, cancelled: 0, lead_days: { median: 3, max: 14 } },
      { name: "print", title: "Print", tasks: 3, delivered: 2, delivered_late: 1, open: 1, overdue: 1, cancelled: 1, lead_days: { median: 3, max: 14 } },
    ],
    total: { tasks: 4, delivered: 2, delivered_late: 1, open: 2, overdue: 1, cancelled: 1, lead_days: { median: 3, max: 14 } },
    late: [
      {
        id: "C-0003",
        title: "Engrave the harbor map",
        flow: "plates",
        column: "drawing",
        parent: null,
        group: "release",
        release: "r2",
        departments: ["print"],
        created_at: "2026-11-04T09:00:00Z",
        taken_at: "2026-11-05T09:00:00Z",
        accepted_at: null,
        due: "2026-11-12",
        days_late: 8,
        lead_days: null,
        estimate: null,
        returns: [],
        reestimates: [],
        cancelled_subtasks: [],
        items: [],
        more: { returns: 0, reestimates: 0, cancelled_subtasks: 0, items: 0 },
      },
      {
        id: "C-0001",
        title: "Write the tide table foreword",
        flow: "copy",
        column: "done",
        parent: null,
        group: "release",
        release: "r1",
        departments: ["words", "print"],
        created_at: "2026-11-01T09:00:00Z",
        taken_at: "2026-11-03T09:00:00Z",
        accepted_at: "2026-11-15T09:00:00Z",
        due: "2026-11-10",
        days_late: 5,
        lead_days: 14,
        estimate: "5",
        returns: [
          { at: "2026-11-06T09:00:00Z", from: "review", note: "Too long" },
          { at: "2026-11-08T09:00:00Z", from: "review", note: "Wrong tone" },
        ],
        reestimates: [{ at: "2026-11-07T09:00:00Z", old: "3", new: "5", reason: "The tables doubled" }],
        cancelled_subtasks: [{ id: "C-0002", title: "Draw the tide plates", at: "2026-11-09T09:00:00Z", reason: "Plates reused from last year" }],
        items: [{ id: "PR-0040", kind: "question", status: "applied" }],
        more: { returns: 0, reestimates: 0, cancelled_subtasks: 0, items: 0 },
      },
    ],
    truncated: false,
    omitted: null,
  };
}

/** A title in Greek, written as escapes (mocks and tests are ASCII). */
export const GREEK_TASK_TITLE = "\u03a7\u03ac\u03c1\u03c4\u03b7\u03c2 \u03c4\u03bf\u03c5 \u03bb\u03b9\u03bc\u03b1\u03bd\u03b9\u03bf\u03cd";

/** A department's title in Cyrillic, written as escapes: a consumer's own words, in any script. */
export const CYRILLIC_DEPARTMENT = "\u0417\u0432\u0443\u043a";

/** The pinned document with every other shape the tab draws. */
export function everyShape(): DeliveryDocument {
  const pinned = pinnedDelivery();
  return {
    ...pinned,
    notes: ["delivery: [card] does not read: no return is counted", "C-0009: its row does not read (due: no calendar date); not counted"],
    releases: [
      ...pinned.releases.slice(0, 2),
      { group: "pool", release: null, ...aDeliveryCounts({ tasks: 2, open: 2 }) },
      { group: "unplaced", release: null, ...aDeliveryCounts({ tasks: 1, delivered: 1, lead_days: { median: 2, max: 2 } }) },
      { group: "milestone", release: null, ...aDeliveryCounts({ tasks: 1, open: 1 }) },
      ...pinned.releases.slice(2),
    ],
    departments: [...pinned.departments, { name: "sound", title: CYRILLIC_DEPARTMENT, ...aDeliveryCounts() }],
    late: [
      aDeliveryLate({
        id: "C-0007",
        title: GREEK_TASK_TITLE,
        group: "pool",
        departments: ["print", "gone"],
        due: "2026-11-19",
        days_late: 1,
        estimate: null,
        returns: Array.from({ length: 16 }, (_, index) => ({ at: `2026-11-${String(index + 2).padStart(2, "0")}T09:00:00Z`, from: "review", note: index === 0 ? null : `Pass ${String(index + 1)}` })),
        reestimates: [{ at: "2026-11-10T09:00:00Z", old: null, new: null, reason: "No estimate until the survey lands" }],
        cancelled_subtasks: [{ id: "C-0008", title: "Second plate", at: "2026-11-11T09:00:00Z", reason: "One plate holds both" }],
        items: [
          { id: "PR-0041", kind: "discrepancy", status: "open" },
          { id: "PR-0042", kind: "decision", status: "approved" },
          { id: "PR-0043", kind: "interpretation", status: "open" },
        ],
        more: { returns: 4, reestimates: 1, cancelled_subtasks: 2, items: 3 },
      }),
      ...pinned.late,
    ],
  };
}

/** A project with no task: every department at zeros, no release row, nothing late. */
export function noTaskDelivery(asOf: string = PINNED_AS_OF): DeliveryDocument {
  const pinned = pinnedDelivery();
  return { ...pinned, as_of: asOf, releases: [], departments: pinned.departments.map(({ name, title }) => ({ name, title, ...aDeliveryCounts() })), total: aDeliveryCounts(), late: [] };
}

/** The Roadmap's stub client answering `document` at the day asked, as the daemon echoes it. */
export function deliveryClient(document: DeliveryDocument = pinnedDelivery()) {
  const client = roadmapClient();
  client.getDelivery.mockImplementation((_project, options) => Promise.resolve({ ...structuredClone(document), as_of: options.asOf }));
  return client;
}
