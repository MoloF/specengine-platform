import { describe, expect, it } from "vitest";
import { apiErrorOf } from "../api/client";
import { pinnedDelivery as pinnedJson, pinnedWithTimeline } from "../test/deliveryStub";
import { deliveryOf, LARGE_TASKS, NO_ROADMAP_NOTE, PINNED_AS_OF, pinnedDelivery } from "./delivery";
import { MockClient } from "./MockClient";

// The mock's delivery (src/mocks/delivery.ts;
// docs/features/delivery-analytics.md "Acceptance criteria", DA-11, DA-12): its judge over the
// spec's scenario gives the spec's JSON byte for byte; the rules the tab shows hold at other days;
// harbor-sim holds every shape the tab draws, at the day asked; `empty` holds no task; ledger-api
// has no `[roadmap]`; a day that is none is refused 400.

const AT = Date.parse("2026-11-12T12:00:00Z");

function harbor(asOf: string, scenario: "normal" | "empty" = "normal") {
  return deliveryOf("harbor-sim", scenario, AT, asOf, true);
}

describe("the spec's scenario (DA-11)", () => {
  it("judges to the spec's JSON at 2026-11-20, key for key and in order", () => {
    expect(JSON.stringify(pinnedDelivery())).toBe(JSON.stringify(pinnedJson()));
    expect(PINNED_AS_OF).toBe("2026-11-20");
  });

  it("C-0003, due 11-12, is overdue at 11-20 by 8, not at 11-01 nor at 11-12; C-0001 is delivered late by 5 whatever the day", () => {
    const lateAt = (asOf: string) => pinnedDelivery(asOf).late.map((row) => [row.id, row.days_late]);
    expect(lateAt("2026-11-20")).toEqual([
      ["C-0003", 8],
      ["C-0001", 5],
    ]);
    expect(lateAt("2026-11-12")).toEqual([["C-0001", 5]]);
    expect(lateAt("2026-11-01")).toEqual([["C-0001", 5]]);
    expect(pinnedDelivery("2026-11-12").total.overdue).toBe(0);
  });
});

// docs/features/delivery-timeline.md "Acceptance criteria", DT1, DT2, DT4, DT5 and DT9: the mock's
// timeline, judged as the core's step 7 lays it out; built only when asked.
describe("the timeline (DT1, DT2, DT4, DT5)", () => {
  it("judges the spec's scenario to its timeline JSON at 2026-11-20, key for key and in order; C-0002 cancelled, no row", () => {
    expect(JSON.stringify(pinnedDelivery(PINNED_AS_OF, true))).toBe(JSON.stringify(pinnedWithTimeline()));
    expect(pinnedDelivery(PINNED_AS_OF, true).timeline?.tasks.map((task) => task.id)).not.toContain("C-0002");
  });

  it("takes C-0004 on 2026-11-03, its move to `review` (`copy` accepts from `review` alone), and the `none` marker's first take with it (DT1)", () => {
    const timeline = pinnedDelivery(PINNED_AS_OF, true).timeline;
    const fixIndex = timeline?.tasks.find((task) => task.id === "C-0004");
    expect([fixIndex?.taken_at, fixIndex?.accepted_at]).toEqual(["2026-11-03T09:00:00Z", "2026-11-05T09:00:00Z"]);
    const none = timeline?.markers.find((marker) => marker.group === "none");
    expect([none?.first_taken_at, none?.last_accepted_at]).toEqual(["2026-11-03T09:00:00Z", "2026-11-05T09:00:00Z"]);
  });

  it("is null unless asked, the rest byte-equal (DT4)", () => {
    expect(pinnedDelivery(PINNED_AS_OF).timeline).toBeNull();
    expect(JSON.stringify({ ...pinnedDelivery(PINNED_AS_OF, true), timeline: null })).toBe(JSON.stringify(pinnedDelivery(PINNED_AS_OF)));
  });

  it("ends the axis at the greatest of the day asked and every stored day of a row: at 2026-11-01, 2026-11-15 (DT5)", () => {
    const early = pinnedDelivery("2026-11-01", true).timeline;
    expect([early?.from, early?.to]).toEqual(["2026-11-01", "2026-11-15"]);
    const late = pinnedDelivery("2026-12-01", true).timeline;
    expect([late?.from, late?.to]).toEqual(["2026-11-01", "2026-12-01"]);
  });

  it("covers a creation the day asked precedes: ledger-api's C-0101, made 2026-11-11 after every accept and due, ends the axis asked at 2026-11-01 (DT5; M: `to` from the day asked, accepts and dues alone)", () => {
    const document = deliveryOf("ledger-api", "normal", AT, "2026-11-01", true);
    const timeline = document.timeline;
    const runbook = timeline?.tasks.find((task) => task.id === "C-0101");
    expect([runbook?.created_at, runbook?.taken_at, runbook?.accepted_at, runbook?.due]).toEqual(["2026-11-11T09:00:00Z", null, null, null]);
    // Every accept and due, and the day asked, before it: 2026-11-10's accept, 2026-11-09's due.
    expect(timeline?.tasks.flatMap((task) => [task.accepted_at?.slice(0, 10), task.due]).filter((value) => value != null)).toEqual(["2026-11-10", "2026-11-09"]);
    expect([document.as_of, timeline?.from, timeline?.to]).toEqual(["2026-11-01", "2026-11-06", "2026-11-11"]);
    // Asked at its creation's day or after, the day asked ends it.
    expect(deliveryOf("ledger-api", "normal", AT, "2026-11-12", true).timeline?.to).toBe("2026-11-12");
  });

  it("puts a marker per group holding a task, its first take and last accept, null with none (DT2)", () => {
    const timeline = harbor("2026-11-12").timeline;
    expect(timeline).not.toBeNull();
    const markers = timeline?.markers.map((marker) => [marker.group, marker.release, marker.first_taken_at, marker.last_accepted_at]);
    expect(markers).toEqual([
      ["release", "keel", "2026-10-28T09:00:00Z", "2026-11-11T09:00:00Z"],
      ["release", "hull", "2026-11-03T09:00:00Z", "2026-11-10T09:00:00Z"],
      ["release", "sails", "2026-11-08T09:00:00Z", "2026-11-11T21:00:00Z"],
      ["pool", null, null, null],
      ["unplaced", null, null, null],
      ["milestone", null, "2026-11-06T09:00:00Z", "2026-11-11T09:00:00Z"],
    ]);
    // No release holds C-0006 alone, cancelled: no marker, no row.
    expect(timeline?.tasks.map((task) => task.id)).not.toContain("C-0006");
    expect(timeline?.tasks.map((task) => task.id)).not.toContain("C-0017");
  });

  it("orders harbor-sim's rows by group, then creation, then ID; days late served on the late ones alone", () => {
    const timeline = harbor("2026-11-12").timeline;
    expect(timeline?.tasks.map((task) => [task.id, task.release ?? task.group, task.days_late])).toEqual([
      ["C-0005", "keel", 5],
      ["C-0001", "keel", 2],
      ["C-0013", "keel", null],
      ["C-0019", "keel", null],
      ["C-0011", "keel", null],
      ["C-0016", "keel", null],
      ["C-0008", "hull", 5],
      ["C-0003", "hull", null],
      ["C-0012", "hull", null],
      ["C-0015", "hull", null],
      ["C-0004", "sails", null],
      ["C-0009", "sails", null],
      ["C-0014", "sails", null],
      ["C-0018", "sails", null],
      ["C-0002", "pool", null],
      ["C-0007", "unplaced", null],
      ["C-0010", "milestone", null],
    ]);
    // C-0012, accepted from the column it was made in: its take is its accept.
    const spring = timeline?.tasks.find((task) => task.id === "C-0012");
    expect(spring?.taken_at).toBe(spring?.accepted_at);
    expect([timeline?.from, timeline?.to]).toEqual(["2026-10-27", "2026-11-22"]);
  });

  it("serves an empty timeline for `empty`: no end, no marker, no row", () => {
    expect(harbor("2026-11-12", "empty").timeline).toEqual({ from: null, to: null, markers: [], tasks: [] });
  });

  it("serves `large`: 2 000 rows over five releases and four departments, every one placed (DT9)", () => {
    const document = deliveryOf("harbor-sim", "large", AT, "2026-11-12", true);
    const timeline = document.timeline;
    expect(LARGE_TASKS).toBe(2000);
    expect(timeline?.tasks).toHaveLength(2000);
    expect(timeline?.markers.map((marker) => marker.release)).toEqual(["keel", "hull", "sails", "masts", "rigging"]);
    expect(document.departments.map((department) => [department.name, department.tasks])).toEqual([
      ["galley", 500],
      ["foundry", 500],
      ["chandlery", 500],
      ["ropewalk", 500],
    ]);
    expect(new Set(timeline?.tasks.map((task) => task.departments.length))).toEqual(new Set([1]));
    expect(timeline?.tasks.filter((task) => task.days_late !== null).length).toBeGreaterThan(20);
    expect(timeline?.tasks.filter((task) => task.taken_at === null).length).toBeGreaterThan(20);
    // Not asked: none built, even here.
    expect(deliveryOf("harbor-sim", "large", AT, "2026-11-12").timeline).toBeNull();
  });
});

describe("harbor-sim's delivery, every shape (DA-12)", () => {
  it("rows each group holding a task in plan order, an unknown word among them, then no release, holding a cancelled task alone", () => {
    const document = harbor("2026-11-12");
    expect(document.as_of).toBe("2026-11-12");
    expect(document.releases.map((row) => [row.group, row.release, row.delivered, row.tasks, row.cancelled])).toEqual([
      ["release", "keel", 3, 6, 1],
      ["release", "hull", 1, 4, 0],
      ["release", "sails", 1, 4, 0],
      ["pool", null, 0, 1, 0],
      ["unplaced", null, 0, 1, 0],
      ["milestone", null, 1, 1, 0],
      ["none", null, 0, 0, 1],
    ]);
    expect(document.releases[0]?.lead_days).toEqual({ median: 6, max: 12 });
    expect(document.releases.at(-1)?.lead_days).toBeNull();
  });

  it("rows every department in served order, one of zeros; the total counts every task once", () => {
    const document = harbor("2026-11-12");
    expect(document.departments.map((row) => [row.name, row.title, row.tasks, row.cancelled])).toEqual([
      ["galley", "Galley", 10, 1],
      ["foundry", "Foundry", 7, 1],
      ["chandlery", "Chandlery", 0, 0],
    ]);
    expect(document.total).toEqual({ tasks: 17, delivered: 6, delivered_late: 1, open: 11, overdue: 2, cancelled: 2, lead_days: { median: 4, max: 12 } });
  });

  it("lists the late tasks, most days late first, then by ID: each reason kind, a cut list, a re-estimate from none and one to a clear", () => {
    const document = harbor("2026-11-12");
    expect(document.late.map((row) => [row.id, row.days_late, row.accepted_at === null])).toEqual([
      ["C-0005", 5, false],
      ["C-0008", 5, true],
      ["C-0001", 2, true],
    ]);
    const foreword = document.late[2];
    expect([foreword?.returns.length, foreword?.more]).toEqual([16, { returns: 1, reestimates: 0, cancelled_subtasks: 0, items: 0 }]);
    expect(foreword?.returns.filter((entry) => entry.note === null)).toHaveLength(2);
    expect(foreword?.items.map((item) => item.kind)).toEqual(["discrepancy", "question", "decision"]);
    expect(foreword?.cancelled_subtasks.map((entry) => entry.id)).toEqual(["C-0017"]);
    expect(foreword?.departments).toEqual(["galley"]);
    expect(document.late[1]?.reestimates.at(-1)).toMatchObject({ old: "3", new: null });
    expect(document.late[1]?.estimate).toBeNull();
    expect(document.late[0]?.reestimates[0]).toMatchObject({ old: null, new: "1" });
  });

  it("judges the open tasks at the day asked: a due equal to it is not overdue, the next day is", () => {
    expect(harbor("2026-11-12").late.map((row) => row.id)).not.toContain("C-0004");
    expect(harbor("2026-11-13").late.map((row) => [row.id, row.days_late])).toEqual([
      ["C-0008", 6],
      ["C-0005", 5],
      ["C-0001", 3],
      ["C-0004", 1],
    ]);
    expect(harbor("2026-11-14").total.overdue).toBe(4);
    // A day before every due: only the task delivered late stays.
    expect(harbor("2026-11-01").late.map((row) => row.id)).toEqual(["C-0005"]);
  });

  it("`empty`: no task, every department at zeros, no release row, nothing late", () => {
    const document = harbor("2026-11-12", "empty");
    expect([document.releases, document.late, document.total.tasks, document.total.cancelled]).toEqual([[], [], 0, 0]);
    expect(document.departments.map((row) => row.tasks)).toEqual([0, 0, 0]);
  });

  it("ledger-api: no [roadmap], every task in no release, the note; the implicit department", () => {
    const document = deliveryOf("ledger-api", "normal", AT, "2026-11-12");
    expect(document.notes).toEqual([NO_ROADMAP_NOTE]);
    expect(document.releases.map((row) => row.group)).toEqual(["none"]);
    expect(document.departments.map((row) => [row.name, row.tasks])).toEqual([["all", 2]]);
  });
});

describe("the mock's route (DA-11)", () => {
  it("answers at the day asked, through the client", async () => {
    const client = new MockClient("normal", { now: () => AT });
    expect((await client.getDelivery("harbor-sim", { asOf: "2026-11-13" })).as_of).toBe("2026-11-13");
  });

  it.each(["2026-13-01", "2026-02-30", "20261112", ""])("refuses `%s` with a 400 naming it, as the daemon does", async (asOf) => {
    const client = new MockClient("normal", { now: () => AT });
    const refusal = await client.getDelivery("harbor-sim", { asOf }).then(
      () => null,
      (error: unknown) => apiErrorOf(error),
    );
    expect(refusal).toEqual({ status: 400, message: `\`as_of\`: \`${asOf}\` is no calendar date, YYYY-MM-DD` });
  });

  it("refuses the read as every read in `error`", async () => {
    const client = new MockClient("error", { now: () => AT });
    await expect(client.getDelivery("harbor-sim", { asOf: "2026-11-12" })).rejects.toMatchObject({ status: 503 });
  });
});
