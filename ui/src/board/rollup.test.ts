import { describe, expect, it } from "vitest";
import type { CardEvent } from "../api/types";
import { aCardFlow, aCardRow } from "../test/builders";
import { enT } from "../test/text";
import { handOffLook } from "./labels";
import { progressOf, returnReasonOf, rollupsOf, type Rollup } from "./rollup";

/** One card's rollup from one list: none when it has no child standing. */
function rollupOf(id: string, rows: Parameters<typeof rollupsOf>[0], flows: Parameters<typeof rollupsOf>[1]): Rollup {
  return rollupsOf(rows, flows).get(id) ?? { children: [], returned: 0 };
}

// BH-16 of board-handoff: a card's children are the list's rows whose `parent` is its ID, a cancelled
// one left out; each child's word is looked up, never judged: accepted when accepted; returned in its
// flow's `return` column; handed off in its first column; taken in any other; no word for a flow the
// read does not serve. BH-15: the latest reason a card's history keeps for its return column.

const FLOWS = [aCardFlow({ name: "art", return: "doing" }), aCardFlow({ name: "plain" })];

describe("a parent's rollup (BH-16)", () => {
  const rows = [
    aCardRow({ id: "C-0001", flow: "plain", title: "Parent" }),
    aCardRow({ id: "C-0002", flow: "art", column: "todo", parent: "C-0001" }),
    aCardRow({ id: "C-0003", flow: "art", column: "check", parent: "C-0001" }),
    aCardRow({ id: "C-0004", flow: "art", column: "doing", parent: "C-0001" }),
    aCardRow({ id: "C-0005", flow: "art", column: "closed", parent: "C-0001", accepted: true }),
    aCardRow({ id: "C-0006", flow: "art", column: "todo", parent: "C-0001", cancelled: true, position: 0 }),
    aCardRow({ id: "C-0007", flow: "gone", column: "x", parent: "C-0001" }),
    aCardRow({ id: "C-0008", flow: "gone", column: "x", parent: "C-0001", accepted: true }),
    aCardRow({ id: "C-0009", flow: "plain", column: "doing", parent: "C-0002" }),
  ];

  it("leaves a cancelled child out and words each one: handed off, taken, returned, accepted, none for a flow not served (M19)", () => {
    const rollup = rollupOf("C-0001", rows, FLOWS);
    expect(rollup.children.map(({ row, word }) => [row.id, word])).toEqual([
      ["C-0002", "handedOff"],
      ["C-0003", "taken"],
      ["C-0004", "returned"],
      ["C-0005", "accepted"],
      ["C-0007", null],
      ["C-0008", "accepted"],
    ]);
    expect(rollup.returned).toBe(1);
  });

  it("finds no return in a flow that names none: its columns are taken but the first", () => {
    expect(rollupOf("C-0002", rows, FLOWS).children.map(({ word }) => word)).toEqual(["taken"]);
  });

  it("reads every parent from one pass; a card with no child has none", () => {
    const all = rollupsOf(rows, FLOWS);
    expect([...all.keys()].sort()).toEqual(["C-0001", "C-0002"]);
    expect(rollupOf("C-0009", rows, FLOWS)).toEqual({ children: [], returned: 0 });
  });

  it("each word a label and an icon, accepted the card's own word", () => {
    expect((["handedOff", "taken", "returned", "accepted"] as const).map((word) => [handOffLook(word, enT).label, handOffLook(word, enT).icon.length > 0])).toEqual([
      ["Handed off", true],
      ["Taken", true],
      ["Returned", true],
      [enT("board.accepted"), true],
    ]);
  });
});

describe("why a card came back (BH-15)", () => {
  const moved = (seq: number, to: string, note: unknown, as = "Ann"): CardEvent => ({ seq, type: `card.${String(seq)}`, at: `2026-11-0${String(seq)}T10:00:00Z`, payload: { id: "C-1", from: "todo", to, as, note } });

  it("the last entry entering the column with a note, whatever its type, oldest first or not", () => {
    const history = [moved(3, "doing", "Again: the ending."), moved(1, "doing", "The ending."), moved(2, "todo", null), moved(4, "doing", null)];
    expect(returnReasonOf(history, "doing")).toEqual({ note: "Again: the ending.", by: "Ann", at: "2026-11-03T10:00:00Z" });
  });

  it("none when the history no longer holds one: no entry, a note that is no text, another column", () => {
    expect(returnReasonOf([], "doing")).toBeNull();
    expect(returnReasonOf([moved(1, "doing", null), moved(2, "doing", 7), moved(3, "check", "x")], "doing")).toBeNull();
    expect(returnReasonOf([{ seq: 1, type: "card.moved", at: "2026-11-01T10:00:00Z", payload: "garbled" }], "doing")).toBeNull();
  });
});

// M4 of docs/features/task-mandate.md "Acceptance criteria": nothing reads a mandate (ADR-0012). Rows
// alike but for it give the same progress, Ready and Relies on, and the same rollup and hand-off words.
describe("a parent's progress and rollup, whatever its subtasks' mandates (M4 of task-mandate)", () => {
  const rows = [
    aCardRow({ id: "C-0001", flow: "plain", title: "Parent" }),
    aCardRow({ id: "C-0002", flow: "art", column: "todo", parent: "C-0001" }),
    aCardRow({ id: "C-0003", flow: "art", column: "doing", parent: "C-0001" }),
    aCardRow({ id: "C-0004", flow: "art", column: "closed", parent: "C-0001", accepted: true }),
    aCardRow({ id: "C-0005", flow: "art", column: "todo", parent: "C-0001", cancelled: true, position: 0 }),
  ];

  it("counts, words, Ready and Relies on are equal across null, binding and open (M: a binding subtask left out of Relies on)", () => {
    const parent = { id: "C-0001", open: true };
    const plain = progressOf(rows, FLOWS, parent);
    for (const mandate of ["binding", "open"] as const) {
      const marked = rows.map((row) => (row.parent === null ? row : { ...row, mandate }));
      const progress = progressOf(marked, FLOWS, parent);
      expect([mandate, progress.segments, progress.ready, progress.reliesOn.map((row) => row.id), progress.children.map((child) => [child.row.id, child.word])]).toEqual([
        mandate,
        plain.segments,
        plain.ready,
        plain.reliesOn.map((row) => row.id),
        plain.children.map((child) => [child.row.id, child.word]),
      ]);
      expect(rollupOf("C-0001", marked, FLOWS)).toEqual({ ...rollupOf("C-0001", rows, FLOWS), children: rollupOf("C-0001", rows, FLOWS).children.map((child) => ({ ...child, row: { ...child.row, mandate } })) });
    }
  });
});
