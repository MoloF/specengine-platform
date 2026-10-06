import { describe, expect, it } from "vitest";
import { aTaskEntry } from "../test/builders";
import { SOME_TASKS } from "../test/taskStub";
import { byNumber, groupsOf, waitsForYou } from "./groups";

// AC-03 and AC-04 of docs/features/ui-tasks.md: groups from `status` and `stale` alone, what waits
// for the owner first, by number within; reserved states labelled; an unknown state last.

function shape(entries = SOME_TASKS) {
  return groupsOf(entries).map((group) => [group.title, group.entries.map((entry) => entry.id)]);
}

describe("groups (AC-04)", () => {
  it("put a plan in review and a ready task whose spec changed first, then the states in order", () => {
    expect(shape()).toEqual([
      ["Waiting for you", ["T-0002", "T-0003"]],
      ["Draft", ["T-0005"]],
      ["Changes requested", ["T-0007"]],
      ["Ready", ["T-0004"]],
      ["In progress", ["T-0006"]],
      ["Closed", ["T-0001", "T-0008"]],
      ["Other states", ["T-0009"]],
    ]);
  });

  it("never count a draft, an in-progress task or a ready one with unknown staleness as waiting", () => {
    expect(waitsForYou({ status: "draft", stale: null })).toBe(false);
    expect(waitsForYou({ status: "in_progress", stale: true })).toBe(false);
    expect(waitsForYou({ status: "ready", stale: null })).toBe(false);
    expect(waitsForYou({ status: "ready", stale: false })).toBe(false);
    expect(waitsForYou({ status: "ready", stale: true })).toBe(true);
    expect(waitsForYou({ status: "review", stale: null })).toBe(true);
  });

  it("label the reserved states and keep them after Closed, an unknown state after every known group (AC-03)", () => {
    const entries = [
      aTaskEntry({ id: "T-0010", status: "triage" }),
      aTaskEntry({ id: "T-0011", status: "accepted" }),
      aTaskEntry({ id: "T-0012", status: "in_review" }),
      aTaskEntry({ id: "T-0013", status: "analysis" }),
      aTaskEntry({ id: "T-0014", status: "done" }),
      aTaskEntry({ id: "T-0015", status: "draft" }),
    ];
    expect(shape(entries)).toEqual([
      ["Draft", ["T-0015"]],
      ["Closed", ["T-0014"]],
      ["Analysis", ["T-0013"]],
      ["Work in review", ["T-0012"]],
      ["Accepted", ["T-0011"]],
      ["Other states", ["T-0010"]],
    ]);
  });

  it("order by number, T-0999 before T-1000 and T-10000", () => {
    expect(["T-10000", "T-1000", "T-0999"].sort(byNumber)).toEqual(["T-0999", "T-1000", "T-10000"]);
    const entries = ["T-1000", "T-0999", "T-10000"].map((id) => aTaskEntry({ id, status: "draft" }));
    expect(shape(entries)).toEqual([["Draft", ["T-0999", "T-1000", "T-10000"]]]);
  });
});
