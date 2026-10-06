import { describe, expect, it } from "vitest";
import { groupsOf } from "../tasks/groups";
import { aProposal, aTaskEntry } from "../test/builders";
import { SOME_TASKS } from "../test/taskStub";
import { kindCountsOf, otherOpenOf, proposalNoun, statusCountsOf, waitingOf } from "./tally";

// docs/features/ui-home.md "Home": what the home counts and groups, nothing else (AC-03, AC-04).

describe("the Tasks region's counts (AC-03)", () => {
  it("takes what waits for you in the Tasks list's order", () => {
    expect(waitingOf(groupsOf(SOME_TASKS)).map((entry) => entry.id)).toEqual(["T-0002", "T-0003"]);
  });

  it("counts every other open group in its order, never Closed", () => {
    expect(otherOpenOf(groupsOf(SOME_TASKS)).map(({ title, count }) => `${title}: ${String(count)}`)).toEqual([
      "Draft: 1",
      "Changes requested: 1",
      "Ready: 1",
      "In progress: 1",
      "Other states: 1",
    ]);
  });

  it("keeps a reserved state's group, which is not Closed", () => {
    const groups = groupsOf([aTaskEntry({ id: "T-1", status: "accepted" }), aTaskEntry({ id: "T-2", status: "done" })]);
    expect(otherOpenOf(groups).map(({ title }) => title)).toEqual(["Accepted"]);
  });

  it("has nothing waiting when no plan is in review and no ready task changed", () => {
    expect(waitingOf(groupsOf([aTaskEntry({ id: "T-1", status: "ready", stale: false })]))).toEqual([]);
  });
});

describe("the Inbox region's counts (AC-04)", () => {
  const queue = [
    aProposal({ id: "PR-1", status: "open", kind: "update" }),
    aProposal({ id: "PR-2", status: "escalated", kind: "question" }),
    aProposal({ id: "PR-3", status: "deferred", kind: "update" }),
    aProposal({ id: "PR-4", status: "open", kind: "discrepancy" }),
    aProposal({ id: "PR-5", status: "approved", kind: "question" }),
    aProposal({ id: "PR-6", status: "archived", kind: "update" }),
  ];

  it("counts by status in the table's order, unknown raw values last", () => {
    expect(statusCountsOf(queue)).toEqual([
      { status: "open", count: 2 },
      { status: "approved", count: 1 },
      { status: "deferred", count: 1 },
      { status: "archived", count: 1 },
      { status: "escalated", count: 1 },
    ]);
  });

  it("counts by raw kind, the most first, then by text", () => {
    expect(kindCountsOf(queue)).toEqual([
      { name: "update", count: 3 },
      { name: "question", count: 2 },
      { name: "discrepancy", count: 1 },
    ]);
  });

  it("sums each count to the total", () => {
    const sum = (counts: { count: number }[]) => counts.reduce((total, { count }) => total + count, 0);
    expect([sum(statusCountsOf(queue)), sum(kindCountsOf(queue))]).toEqual([queue.length, queue.length]);
  });

  it("says one proposal, two proposals", () => {
    expect([proposalNoun(1), proposalNoun(2), proposalNoun(0)]).toEqual(["proposal", "proposals", "proposals"]);
  });
});
