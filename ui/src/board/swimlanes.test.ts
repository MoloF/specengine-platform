import { describe, expect, it, vi } from "vitest";
import { aCardFlow, aCardRow } from "../test/builders";
import { enT } from "../test/text";
import { columnHeadsOf, type BoardTab } from "./filter";
import { laneModeKey, recallLaneMode, rememberLaneMode, swimlanesOf, type Swimlane } from "./swimlanes";

// BH-8 of board-handoff: the lanes cut the one list read. None draws one lane; Assignee runs by the
// folded name, ties by the raw one, then Unassigned; Parent by the parent ID's number, then
// Everything else, each head naming its parent wherever it stands. A lane's place never comes from
// its rows, so a move does not reorder lanes. "Not in the flow" stands last in every lane.

const TAB: BoardTab = { name: "work", flow: aCardFlow() };

function lanes(rows: Parameters<typeof swimlanesOf>[2], mode: Parameters<typeof swimlanesOf>[3], all = rows): Swimlane[] {
  return swimlanesOf(TAB, columnHeadsOf(TAB, all, enT), rows, mode, all);
}

function heads(found: readonly Swimlane[]): string[] {
  return found.map(({ head }) => {
    switch (head.cut) {
      case "assignee":
        return `assignee ${head.name}`;
      case "parent":
        return `parent ${head.id} ${head.card === undefined ? "?" : `${head.card.title} ${head.card.flow}/${head.card.column}`}`;
      default:
        return head.cut;
    }
  });
}

describe("lanes (BH-8)", () => {
  it("None draws one lane, every column a cell in served order", () => {
    const found = lanes([aCardRow({ id: "C-1" }), aCardRow({ id: "C-2", column: "doing" })], "none");
    expect(heads(found)).toEqual(["all"]);
    expect(found[0]?.cells.map((cell) => [cell.title, cell.rows.map((row) => row.id)])).toEqual([
      ["To do", ["C-1"]],
      ["Doing", ["C-2"]],
      ["Check", []],
      ["Closed", []],
    ]);
  });

  it("Assignee runs by the folded name, ties by the raw name, then Unassigned", () => {
    const rows = ["Zoe", "\u00c9mile", "anna", "Anna", "\u0411\u043e\u0440\u0438\u0441", null].map((assignee, index) => aCardRow({ id: `C-${String(index + 1)}`, assignee }));
    expect(heads(lanes(rows, "assignee"))).toEqual(["assignee Anna", "assignee anna", "assignee Zoe", "assignee \u00c9mile", "assignee \u0411\u043e\u0440\u0438\u0441", "unassigned"]);
  });

  it("folds a name as the filters do: NFC first, so a composed and a decomposed name sort as one", () => {
    const rows = [aCardRow({ id: "C-1", assignee: "Ze\u0301" }), aCardRow({ id: "C-2", assignee: "Zf" }), aCardRow({ id: "C-3", assignee: "Z\u00e9" })];
    // "z\u00e9" folds past "zf"; the composed and the decomposed name fold alike and stand together, the raw name breaking the tie.
    expect(heads(lanes(rows, "assignee"))).toEqual(["assignee Zf", "assignee Ze\u0301", "assignee Z\u00e9"]);
  });

  it("Parent runs by the ID's number, then Everything else; a head names its parent's title and <flow>/<column>, a hidden flow's too", () => {
    const all = [
      aCardRow({ id: "C-0100", parent: "C-0012" }),
      aCardRow({ id: "C-0101", parent: "C-0009" }),
      aCardRow({ id: "C-0102" }),
      aCardRow({ id: "C-0012", title: "Writer's card", flow: "narrative", column: "draft" }),
      aCardRow({ id: "C-0009", title: "Same flow", column: "doing" }),
      aCardRow({ id: "C-0103", parent: "C-0404" }),
    ];
    const found = lanes(all, "parent");
    expect(heads(found)).toEqual(["parent C-0009 Same flow work/doing", "parent C-0012 Writer's card narrative/draft", "parent C-0404 ?", "rest"]);
    // A same-flow parent is a head and a tile alike.
    expect(found.at(-1)?.cells.flatMap((cell) => cell.rows.map((row) => row.id))).toEqual(["C-0102", "C-0009"]);
  });

  it("keeps the lanes' order through a move that changes a lane's first row (M10)", () => {
    const before = [aCardRow({ id: "C-1", assignee: "Mira" }), aCardRow({ id: "C-2", assignee: "Ann" }), aCardRow({ id: "C-3", assignee: "Ann", column: "doing" })];
    const after = [aCardRow({ id: "C-3", assignee: "Ann", column: "todo" }), aCardRow({ id: "C-1", assignee: "Mira" }), aCardRow({ id: "C-2", assignee: "Ann", column: "doing" })];
    expect(heads(lanes(before, "assignee"))).toEqual(["assignee Ann", "assignee Mira"]);
    expect(heads(lanes(after, "assignee"))).toEqual(["assignee Ann", "assignee Mira"]);
    const parents = [aCardRow({ id: "C-5", parent: "C-0002" }), aCardRow({ id: "C-6", parent: "C-0010" })];
    expect(heads(lanes(parents, "parent"))).toEqual(heads(lanes([...parents].reverse(), "parent")));
  });

  it("puts Not in the flow last in every lane, its cells for each lane's own rows", () => {
    const rows = [aCardRow({ id: "C-1", assignee: "Ann" }), aCardRow({ id: "C-2", assignee: "Bo", column: "parked" })];
    const found = lanes(rows, "assignee");
    expect(found.map((lane) => lane.cells.at(-1)?.title)).toEqual(["Not in the flow", "Not in the flow"]);
    expect(found.map((lane) => lane.cells.at(-1)?.rows.map((row) => row.id))).toEqual([[], ["C-2"]]);
  });

  it("counts what stands in a lane and in each of its cells, a cancelled card having left", () => {
    const found = lanes([aCardRow({ id: "C-1" }), aCardRow({ id: "C-2", cancelled: true, position: 0 })], "none");
    expect([found[0]?.count, found[0]?.cells[0]?.count, found[0]?.cells[0]?.rows.length]).toEqual([1, 1, 2]);
  });

  it("a mode finding no lane draws None's one lane, so the columns still take a drop", () => {
    expect(heads(lanes([], "assignee"))).toEqual(["all"]);
    expect(heads(lanes([], "parent"))).toEqual(["all"]);
  });

  it("keys a lane and its cells apart from another lane's and another flow's", () => {
    const found = lanes([aCardRow({ id: "C-1", assignee: "Ann" }), aCardRow({ id: "C-2", assignee: "Bo" })], "assignee");
    const keys = found.flatMap((lane) => lane.cells.map((cell) => cell.key));
    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe("the lane mode's storage (BH-8)", () => {
  it("survives a remount, per project; an unknown or missing value reads as None", () => {
    expect(recallLaneMode("alpha")).toBe("none");
    rememberLaneMode("alpha", "parent");
    expect(window.localStorage.getItem(laneModeKey("alpha"))).toBe("parent");
    expect(recallLaneMode("alpha")).toBe("parent");
    expect(recallLaneMode("beta")).toBe("none");
    window.localStorage.setItem(laneModeKey("alpha"), "swim");
    expect(recallLaneMode("alpha")).toBe("none");
    expect(laneModeKey("a b")).toBe("specengine.board.lanes.a b");
  });

  it("a storage that refuses reads as None and keeps nothing, throwing nothing", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("refused");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("refused");
    });
    expect(recallLaneMode("alpha")).toBe("none");
    expect(() => {
      rememberLaneMode("alpha", "assignee");
    }).not.toThrow();
  });
});
