import { describe, expect, it } from "vitest";
import { aCardFlow, aCardList, aCardRow } from "../test/builders";
import { assigneeChoices, disciplineChoices, dropMove, lanesOf, matchesFilters, NO_FILTERS, NOBODY, openRows, reorderOf, standingBefore, tabsOf } from "./filter";
import { canClear, markLine, markLook, markNames, rowMark } from "./labels";

// The Board's arrangement of one answer (docs/features/board-ui.md "Board", "Rules and edge cases").

describe("the filters", () => {
  const row = aCardRow({ id: "C-0001", discipline: "Copy", assignee: "Mira Holt" });

  it("AND, each compared folded: NFC, lower case", () => {
    expect(matchesFilters(row, NO_FILTERS)).toBe(true);
    expect(matchesFilters(row, { discipline: "copy", assignee: "mira holt" })).toBe(true);
    expect(matchesFilters(row, { discipline: "copy", assignee: "ivo brandt" })).toBe(false);
    expect(matchesFilters(row, { discipline: "plates", assignee: "mira holt" })).toBe(false);
    // The composed and the decomposed spelling are one name, Cyrillic too.
    const composed = aCardRow({ id: "C-0002", assignee: "\u0419\u043e\u0448\u043a\u0430" });
    expect(matchesFilters(composed, { discipline: "", assignee: "\u0438\u0306\u043e\u0448\u043a\u0430".normalize("NFC") })).toBe(true);
  });

  it("Unassigned takes the rows with no assignee, and no name can equal its key", () => {
    expect(matchesFilters(aCardRow({ id: "C-0003" }), { discipline: "", assignee: NOBODY })).toBe(true);
    expect(matchesFilters(row, { discipline: "", assignee: NOBODY })).toBe(false);
    const named = aCardRow({ id: "C-0004", assignee: "(Unassigned)" });
    expect(matchesFilters(named, { discipline: "", assignee: NOBODY })).toBe(false);
  });

  it("lists each value once by its folded form, the first spelling read, in read order; Unassigned last", () => {
    const rows = [aCardRow({ id: "C-1", discipline: "Copy", assignee: "Ann" }), aCardRow({ id: "C-2", discipline: "copy", assignee: "ann" }), aCardRow({ id: "C-3", discipline: "plates" })];
    expect(disciplineChoices(rows, "")).toEqual([
      { key: "copy", label: "Copy" },
      { key: "plates", label: "plates" },
    ]);
    expect(assigneeChoices(rows, "")).toEqual([
      { key: "ann", label: "Ann" },
      { key: NOBODY, label: "Unassigned" },
    ]);
  });

  it("keeps a chosen value no card holds any more listed", () => {
    expect(disciplineChoices([], "gone")).toEqual([{ key: "gone", label: "gone" }]);
  });
});

describe("a row's mark", () => {
  const plain = { mark: "open", on: null, discrepancy: null, correction: null };

  it("says what a row says: a plain state and a read with no marks say nothing against the brief", () => {
    expect(rowMark(plain)).toBeNull();
    expect(rowMark({ ...plain, mark: "accepted" })).toBeNull();
    expect(rowMark({ ...plain, mark: null })).toBeNull();
  });

  it("names the target, the discrepancy or the correction, each mark its own", () => {
    const suspect = rowMark({ ...plain, mark: "suspect", on: "R-1" });
    const departed = rowMark({ ...plain, mark: "departed", on: "R-1", discrepancy: "PR-1" });
    const correcting = rowMark({ ...plain, mark: "correcting", on: "R-1", correction: "C-2" });
    expect([suspect, departed, correcting].map((mark) => (mark === null ? null : markNames(mark)))).toEqual(["R-1", "PR-1", "C-2"]);
    expect([suspect, departed, correcting].map((mark) => mark !== null && canClear(mark))).toEqual([true, false, false]);
  });

  it("says each mark in a whole sentence, a gone target by its name as any other", () => {
    expect(markLine({ ...plain, mark: "suspect", on: "R-1" })).toBe("The brief changed at R-1 after this card last read it.");
    expect(markLine({ ...plain, mark: "departed", on: "R-1", discrepancy: "PR-1" })).toBe("Accepted as departing from R-1; PR-1 is not settled yet.");
    expect(markLine({ ...plain, mark: "correcting", on: "R-1", correction: "C-2" })).toBe("The brief changed at R-1; C-2 is putting it right.");
  });

  it("shows a word this build does not know raw and neutral, never as nothing", () => {
    const mark = rowMark({ ...plain, mark: "pondering", on: "R-1" });
    expect(mark === null ? null : [markLook(mark)?.label, markLook(mark)?.tone, markNames(mark)]).toEqual(["pondering", "card-mark-unknown", "R-1"]);
  });

  it("shows it so on no target too: a word it cannot place is no plain state, and no tile reads clean for it", () => {
    const mark = rowMark({ ...plain, mark: "pondering" });
    expect(mark === null ? null : [markLook(mark)?.label, markLook(mark)?.tone, markLine(mark)]).toEqual(["pondering", "card-mark-unknown", "pondering"]);
  });
});

describe("the tabs and the columns", () => {
  it("gives a tab per flow in served order, then any flow only a row still names", () => {
    const list = aCardList([aCardRow({ id: "C-1", flow: "old" })], { flows: [aCardFlow({ name: "b" }), aCardFlow({ name: "a" })] });
    expect(tabsOf(list).map(({ name, flow }) => [name, flow !== undefined])).toEqual([
      ["b", true],
      ["a", true],
      ["old", false],
    ]);
  });

  it("keeps empty columns, the daemon's order, and counts what stands in a column: a cancelled card has left it", () => {
    const rows = [aCardRow({ id: "C-2", column: "doing" }), aCardRow({ id: "C-1", column: "doing" }), aCardRow({ id: "C-3", column: "doing", cancelled: true })];
    const lanes = lanesOf({ name: "work", flow: aCardFlow() }, rows);
    expect(lanes.map((lane) => [lane.title, lane.count])).toEqual([
      ["To do", 0],
      ["Doing", 2],
      ["Check", 0],
      ["Closed", 0],
    ]);
    expect(lanes[1]?.rows.map((row) => row.id)).toEqual(["C-2", "C-1", "C-3"]);
  });

  it("puts a row outside `columns[]` in a last lane, which is no column; a gone flow's rows all there", () => {
    const stranded = aCardRow({ id: "C-1", column: "parked" });
    const lanes = lanesOf({ name: "work", flow: aCardFlow() }, [stranded]);
    expect(lanes.at(-1)).toEqual({ key: "work/outside", column: null, title: "Not in the flow", count: 1, rows: [stranded] });
    expect(lanesOf({ name: "work", flow: undefined }, [stranded]).map((lane) => lane.column)).toEqual([null]);
  });

  it("keeps two flows' columns of one name apart: a lane's key holds its flow", () => {
    const key = (name: string) => lanesOf({ name, flow: aCardFlow({ name }) }, [])[0]?.key;
    expect(key("copy")).not.toBe(key("plates"));
  });

  it("leaves another flow's rows out", () => {
    expect(lanesOf({ name: "work", flow: aCardFlow() }, [aCardRow({ id: "C-1", flow: "other" })]).flatMap((lane) => lane.rows)).toEqual([]);
  });
});

describe("where a card goes", () => {
  const [a, b, c, d] = ["C-1", "C-2", "C-3", "C-4"].map((id) => aCardRow({ id }));
  const whole = [a, b, c, d].flatMap((row) => (row === undefined ? [] : [row]));

  it("Up takes the place of the card above; Down names the card after the one below, none: last", () => {
    expect(reorderOf("C-2", whole, whole)).toEqual({ up: { before: "C-1" }, down: { before: "C-4" } });
    expect(reorderOf("C-3", whole, whole)).toEqual({ up: { before: "C-2" }, down: { before: null } });
    expect(reorderOf("C-1", whole, whole).up).toBeNull();
    expect(reorderOf("C-4", whole, whole).down).toBeNull();
  });

  it("under a filter it steps over the hidden: Down lands right after the next card shown", () => {
    const shown = whole.filter((row) => row.id !== "C-2");
    expect(reorderOf("C-1", shown, whole).down).toEqual({ before: "C-4" });
    expect(reorderOf("C-3", shown, whole).up).toEqual({ before: "C-1" });
  });

  it("a drop posts where it lands, nothing when the card would stay", () => {
    const row = whole[1];
    if (row === undefined) {
      throw new Error("no row");
    }
    const doing = [aCardRow({ id: "C-9", column: "doing" })];
    expect(dropMove(row, "doing", "C-9", doing)).toEqual({ to: "doing", before: "C-9" });
    expect(dropMove(row, "doing", null, doing)).toEqual({ to: "doing" });
    expect(dropMove(row, "todo", "C-4", whole)).toEqual({ to: "todo", before: "C-4" });
    expect(dropMove(row, "todo", null, whole)).toEqual({ to: "todo" });
    expect(dropMove(row, "todo", "C-2", whole)).toBeNull();
    expect(dropMove(row, "todo", "C-3", whole)).toBeNull();
    const last = whole[3];
    if (last !== undefined) {
      expect(dropMove(last, "todo", null, whole)).toBeNull();
    }
  });

  it("a `before` that has left its column is no place: past the last that stands, nothing when the card is there already", () => {
    const row = whole[1];
    const last = whole[3];
    if (row === undefined || last === undefined) {
      throw new Error("no row");
    }
    expect(standingBefore("C-1", whole)).toBe("C-1");
    expect(standingBefore("C-8", whole)).toBeNull();
    expect(standingBefore(null, whole)).toBeNull();
    expect(dropMove(row, "todo", "C-8", whole)).toEqual({ to: "todo" });
    expect(dropMove(row, "doing", "C-8", [])).toEqual({ to: "doing" });
    expect(dropMove(last, "todo", "C-8", whole)).toBeNull();
  });

  it("places a card among those standing: cancelled ones have left", () => {
    expect(openRows([aCardRow({ id: "C-1" }), aCardRow({ id: "C-2", cancelled: true })]).map((row) => row.id)).toEqual(["C-1"]);
  });
});
