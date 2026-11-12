import { describe, expect, it } from "vitest";
import { apiErrorOf } from "../api/client";
import { pinnedDelivery as pinnedJson } from "../test/deliveryStub";
import { deliveryOf, NO_ROADMAP_NOTE, PINNED_AS_OF, pinnedDelivery } from "./delivery";
import { MockClient } from "./MockClient";

// The mock's delivery (src/mocks/delivery.ts;
// docs/features/delivery-analytics.md "Acceptance criteria", DA-11, DA-12): its judge over the
// spec's scenario gives the spec's JSON byte for byte; the rules the tab shows hold at other days;
// harbor-sim holds every shape the tab draws, at the day asked; `empty` holds no task; ledger-api
// has no `[roadmap]`; a day that is none is refused 400.

const AT = Date.parse("2026-11-12T12:00:00Z");

function harbor(asOf: string, scenario: "normal" | "empty" = "normal") {
  return deliveryOf("harbor-sim", scenario, AT, asOf);
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
