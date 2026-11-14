import { describe, expect, it, vi } from "vitest";
import type { CardRow } from "../api/types";
import { aCardRow } from "../test/builders";
import { BOARD_LEVELS, levelKey, matchesLevel, recallLevel, rememberLevel, type BoardLevel } from "./level";

// BS1 and BS2 of docs/features/board-subtasks.md "Acceptance criteria": the Show select's predicate
// over the fixture list, and its storage per viewer and project, as the lane mode's.

/** The fixture: parent C-0001 with subtasks C-0002 and C-0003, lone task C-0004, and C-0005 whose parent C-0009 the list does not hold. */
const ROWS: CardRow[] = [
  aCardRow({ id: "C-0001", title: "Weekly run" }),
  aCardRow({ id: "C-0002", parent: "C-0001" }),
  aCardRow({ id: "C-0003", parent: "C-0001" }),
  aCardRow({ id: "C-0004" }),
  aCardRow({ id: "C-0005", parent: "C-0009" }),
];

function kept(level: BoardLevel): string[] {
  return ROWS.filter((row) => matchesLevel(row, level)).map((row) => row.id);
}

describe("the level's predicate (BS1)", () => {
  it("keeps every row for all, the rows with no parent for tasks, the rest for subtasks", () => {
    expect(BOARD_LEVELS).toEqual(["all", "tasks", "subtasks"]);
    expect(kept("all")).toEqual(["C-0001", "C-0002", "C-0003", "C-0004", "C-0005"]);
    expect(kept("tasks")).toEqual(["C-0001", "C-0004"]);
    expect(kept("subtasks")).toEqual(["C-0002", "C-0003", "C-0005"]);
  });
});

describe("the level's storage (BS2)", () => {
  it("reads all with nothing stored, a stored level as stored, any other word as all", () => {
    expect(recallLevel("harbor")).toBe("all");
    window.localStorage.setItem("specengine.board.level.harbor", "subtasks");
    expect(recallLevel("harbor")).toBe("subtasks");
    for (const stored of ["SUBTASKS", "", "parent"]) {
      window.localStorage.setItem("specengine.board.level.harbor", stored);
      expect([stored, recallLevel("harbor")]).toEqual([stored, "all"]);
    }
  });

  it("reads all and writes without throwing when storage refuses", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("denied", "QuotaExceededError");
    });
    expect(recallLevel("harbor")).toBe("all");
    expect(() => {
      rememberLevel("harbor", "tasks");
    }).not.toThrow();
  });

  it("writes exactly its own key, per project, and leaves the lane mode's as it was", () => {
    window.localStorage.setItem("specengine.board.lanes.harbor", "parent");
    rememberLevel("harbor", "tasks");
    expect(levelKey("harbor")).toBe("specengine.board.level.harbor");
    expect(window.localStorage.getItem("specengine.board.level.harbor")).toBe("tasks");
    expect(window.localStorage.getItem("specengine.board.lanes.harbor")).toBe("parent");
    expect(Object.keys(window.localStorage).sort()).toEqual(["specengine.board.lanes.harbor", "specengine.board.level.harbor"]);
  });
});
