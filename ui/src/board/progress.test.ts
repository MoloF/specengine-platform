import { describe, expect, it } from "vitest";
import type { CardRow } from "../api/types";
import { aCardFlow, aCardRow } from "../test/builders";
import { progressOf, PROGRESS_ORDER, type Progress } from "./rollup";

// docs/features/handoff-progress.md "Acceptance criteria", HP-2: a parent's progress from its children
// read, pure. The words in the bar's order: accepted, cancelled, then an open child's category looked
// up (returned, in progress, to do, not in the flow); Ready and Relies on on an open parent alone.

/** The work flow of the builders: todo, doing, check, closed; `accept` check to closed; `return` doing2 where a test sets it. */
const WORK = aCardFlow({ return: "back", columns: [...aCardFlow().columns, { id: "back", title: "Back" }] });
const FLOWS = [WORK];

const OPEN = { id: "P", open: true };
const CLOSED = { id: "P", open: false };

/** A child of P: open in `column` unless told. */
function child(id: string, fields: Partial<CardRow> = {}): CardRow {
  return aCardRow({ id, parent: "P", ...fields });
}

/** The segments as `word count` pairs. */
function counted(progress: Progress): string[] {
  return progress.segments.map((segment) => `${segment.word} ${String(segment.count)}`);
}

describe("the words, in the bar's order (HP-2)", () => {
  it("orders accepted, cancelled, returned, in progress, to do, not in the flow", () => {
    expect(PROGRESS_ORDER).toEqual(["accepted", "cancelled", "returned", "doing", "todo", "stranded"]);
  });

  it("counts each child once by its word, zero counts left out, the counts summing to the children", () => {
    const rows = [
      child("S1", { flow: "gone", column: "anywhere" }),
      child("T1", { column: "todo" }),
      child("D1", { column: "doing" }),
      child("R1", { column: "back" }),
      child("C1", { column: "doing", cancelled: true }),
      child("A1", { column: "closed", accepted: true }),
      child("D2", { column: "check" }),
    ];
    const progress = progressOf(rows, FLOWS, OPEN);
    // M: segments sorted by count.
    expect(counted(progress)).toEqual(["accepted 1", "cancelled 1", "returned 1", "doing 2", "todo 1", "stranded 1"]);
    expect(progress.segments.reduce((sum, segment) => sum + segment.count, 0)).toBe(rows.length);
    expect(counted(progressOf([child("T1"), child("T2")], FLOWS, OPEN))).toEqual(["todo 2"]);
  });

  it("counts an accepted child as accepted in a column its flow dropped, before any category", () => {
    // M: the category judged before `accepted` reads this child not in the flow.
    expect(counted(progressOf([child("A1", { column: "dropped", accepted: true })], FLOWS, OPEN))).toEqual(["accepted 1"]);
  });

  it("counts a cancelled child as cancelled, whatever its column", () => {
    expect(counted(progressOf([child("C1", { column: "back", cancelled: true })], FLOWS, OPEN))).toEqual(["cancelled 1"]);
  });

  it("counts an open child in the flow's return column as returned, in a middle column or in `accept.to` as in progress, in the first as to do", () => {
    expect(counted(progressOf([child("R1", { column: "back" })], FLOWS, OPEN))).toEqual(["returned 1"]);
    expect(counted(progressOf([child("D1", { column: "doing" }), child("D2", { column: "check" }), child("D3", { column: "closed" })], FLOWS, OPEN))).toEqual(["doing 3"]);
    expect(counted(progressOf([child("T1", { column: "todo" })], FLOWS, OPEN))).toEqual(["todo 1"]);
  });

  it("counts an open child of a flow not served, or in a column its flow does not list, as not in the flow", () => {
    expect(counted(progressOf([child("S1", { flow: "gone" }), child("S2", { column: "dropped" })], FLOWS, OPEN))).toEqual(["stranded 2"]);
  });

  it("ignores rows of another parent, a grandchild, and the parent's own row", () => {
    const rows = [child("T1"), aCardRow({ id: "X1", parent: "Q" }), aCardRow({ id: "G1", parent: "T1" }), aCardRow({ id: "P", parent: "P" }), aCardRow({ id: "N1" })];
    const progress = progressOf(rows, FLOWS, OPEN);
    expect(counted(progress)).toEqual(["todo 1"]);
    expect(progress.children.map(({ row }) => row.id)).toEqual(["T1"]);
  });

  it("keeps served order in the children, the cancelled and Relies on; the children hold the accepted with their hand-off word", () => {
    const rows = [child("Z9", { column: "doing" }), child("C2", { cancelled: true }), child("A1", { column: "closed", accepted: true }), child("B1"), child("C1", { cancelled: true })];
    const progress = progressOf(rows, FLOWS, OPEN);
    expect(progress.children.map(({ row }) => row.id)).toEqual(["Z9", "A1", "B1"]);
    expect(progress.children.map(({ word }) => word)).toEqual(["taken", "accepted", "handedOff"]);
    expect(progress.cancelled.map((row) => row.id)).toEqual(["C2", "C1"]);
    expect(progress.reliesOn.map((row) => row.id)).toEqual(["Z9", "B1"]);
  });
});

describe("Ready to accept (HP-2)", () => {
  it.each([
    ["one accepted child", [child("A1", { column: "closed", accepted: true })]],
    ["one cancelled child", [child("C1", { cancelled: true })]],
    // M: `ready` as every child accepted reads this case false.
    ["two cancelled", [child("C1", { cancelled: true }), child("C2", { cancelled: true })]],
    ["one of each", [child("A1", { column: "closed", accepted: true }), child("C1", { cancelled: true })]],
  ])("holds for an open parent with %s", (_case, rows) => {
    expect(progressOf(rows, FLOWS, OPEN).ready).toBe(true);
  });

  it.each([
    ["no child", [] as CardRow[], OPEN],
    ["a returned child", [child("A1", { column: "closed", accepted: true }), child("R1", { column: "back" })], OPEN],
    ["a child to do", [child("T1")], OPEN],
    ["an accepted parent", [child("A1", { column: "closed", accepted: true })], CLOSED],
    ["a cancelled parent", [child("C1", { cancelled: true })], CLOSED],
  ])("fails with %s", (_case, rows, parent) => {
    expect(progressOf(rows, FLOWS, parent).ready).toBe(false);
  });
});

describe("Relies on (HP-2)", () => {
  it("lists an open parent's children neither accepted nor cancelled, and none on a closed parent", () => {
    const rows = [child("T1"), child("A1", { column: "closed", accepted: true }), child("C1", { cancelled: true }), child("R1", { column: "back" }), child("S1", { flow: "gone" })];
    expect(progressOf(rows, FLOWS, OPEN).reliesOn.map((row) => row.id)).toEqual(["T1", "R1", "S1"]);
    expect(progressOf(rows, FLOWS, CLOSED).reliesOn).toEqual([]);
    // The bar and both lists stand on a closed parent.
    const closed = progressOf(rows, FLOWS, CLOSED);
    expect([closed.children.length, closed.cancelled.length, closed.segments.length]).toEqual([4, 1, 5]);
  });
});
