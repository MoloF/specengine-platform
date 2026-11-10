import { describe, expect, it } from "vitest";
import { asideKilnTree } from "../test/asideStub";
import { aDirection, aModule, aTreeNode, aTreeView } from "../test/builders";
import { FERRY, ferryTree, ferryWorkspaces } from "../test/docStub";
import { INSIDE_SHOWN, insideDirections, insideOf, type InsideDirection, type InsideSet } from "./inside";

// DM-13 of docs/features/doc-modal.md: what a document consists of, from a rooted tree read two deep;
// DP-13 of docs/features/departments.md: a direction gathers its kinds' children under its title.

/** A set as its label, then its entries' names: a direction by `direction:<title>`, a raw kind as itself. */
function setShape(set: InsideSet): [string | null, string[]] {
  return [set.by === "direction" ? `direction:${set.title}` : set.kind, set.entries.map((entry) => entry.name)];
}

/** Each group as its holder, then each set with its entries' names. */
function shape(rows: Parameters<typeof aTreeView>[0], directions: readonly InsideDirection[] = []) {
  const { groups, noId } = insideOf(aTreeView(rows), directions);
  return { groups: groups.map((group) => [group.holder, group.direct, group.title, group.sets.map(setShape)]), noId };
}

describe("Inside (DM-13 of doc-modal)", () => {
  it("groups the ferry's children by the holder they are listed under, the document first, then by raw kind; the grandchild and the ID-less note left out of the lists", () => {
    expect(shape(ferryTree())).toEqual({
      groups: [
        [FERRY, true, "The ferry", [["mechanic", ["MEC-BOARDING"]]]],
        ["RULE-FERRY-TIDES", false, "Tides", [["assumption", ["ASM-0001", "ASM-0002"]], ["question", ["QST-0001"]]]],
      ],
      noId: 1,
    });
  });

  it("drops a row under a child, whatever its path: a child's own children and sections are its own Inside (M: grouping by parent alone)", () => {
    const flat = insideOf(aTreeView(ferryTree())).groups.flatMap((group) => group.sets.flatMap((set) => set.entries.map((entry) => entry.name)));
    expect(flat).not.toContain("ASM-0003");
    const kiln = insideOf(aTreeView(asideKilnTree()));
    expect(kiln.groups.flatMap((group) => group.sets.flatMap((set) => set.entries.map((entry) => entry.name)))).not.toContain("CONE-GLOST-SHELF");
  });

  it("sorts kinds by code point, no kind last, entries kept in tree order (M: kinds unsorted)", () => {
    const rows = [
      aTreeNode({ id: "D", depth: 0, path: "d.md" }),
      aTreeNode({ id: "Z-1", depth: 1, parent: "D", kind: "zeta", path: "z1.md" }),
      aTreeNode({ id: "N-1", depth: 1, parent: "D", kind: null, path: "n1.md" }),
      aTreeNode({ id: "B-2", depth: 1, parent: "D", kind: "Beta", path: "b2.md" }),
      aTreeNode({ id: "A-9", depth: 1, parent: "D", kind: "alpha", path: "a9.md" }),
      aTreeNode({ id: "A-1", depth: 1, parent: "D", kind: "alpha", path: "a1.md" }),
    ];
    expect(shape(rows).groups[0]?.[3]).toEqual([
      ["Beta", ["B-2"]],
      ["alpha", ["A-9", "A-1"]],
      ["zeta", ["Z-1"]],
      [null, ["N-1"]],
    ]);
  });

  it("gives the kiln bay one direct group of 144: conerule 2, edict 32, hunch 60, puzzle 50 (flow-aside, 146 rows)", () => {
    const rows = asideKilnTree();
    expect(rows).toHaveLength(146);
    const { groups, noId } = insideOf(aTreeView(rows));
    expect(groups.map((group) => [group.holder, group.direct])).toEqual([["WBAY-KILN", true]]);
    expect(groups[0]?.sets.map((set) => [set.by === "kind" ? set.kind : null, set.entries.length])).toEqual([
      ["conerule", 2],
      ["edict", 32],
      ["hunch", 60],
      ["puzzle", 50],
    ]);
    expect(noId).toBe(0);
    expect(INSIDE_SHOWN).toBe(50);
  });

  it("lists no group for a document with nothing under it, and counts an ID-less child under a section", () => {
    expect(shape([aTreeNode({ id: "D", depth: 0, path: "d.md" })])).toEqual({ groups: [], noId: 0 });
    const rows = [aTreeNode({ id: "D", depth: 0, path: "d.md" }), aTreeNode({ id: "S", depth: 1, parent: "D", path: "d.md", line: 9 }), aTreeNode({ id: null, depth: 2, parent: "S", path: "loose.md" })];
    expect(shape(rows)).toEqual({ groups: [], noId: 1 });
  });

  it("roots at a section: its own rows share its file, its children are listed under it", () => {
    const rows = [aTreeNode({ id: "RULE-FERRY-TIDES", depth: 0, title: "Tides", path: "docs/ferry/ferry.md", line: 15 }), aTreeNode({ id: "ASM-0001", depth: 1, parent: "RULE-FERRY-TIDES", kind: "assumption", path: "docs/ferry/asm-0001.md" })];
    expect(shape(rows).groups).toEqual([["RULE-FERRY-TIDES", true, "Tides", [["assumption", ["ASM-0001"]]]]]);
  });
});

describe("Inside by direction (DP-13 of departments)", () => {
  const directions = insideDirections(ferryWorkspaces().modules);

  it("takes the served directions in served order, a rooted module none", () => {
    expect(directions).toEqual([
      { name: "records", title: "Records", kinds: ["assumption"] },
      { name: "asked", title: "Asked", kinds: ["question", "assumption"] },
    ]);
  });

  it("groups as the spec's stub says: raw mechanic under the document; Records, then Asked, under the section", () => {
    expect(shape(ferryTree(), directions)).toEqual({
      groups: [
        [FERRY, true, "The ferry", [["mechanic", ["MEC-BOARDING"]]]],
        ["RULE-FERRY-TIDES", false, "Tides", [["direction:Records", ["ASM-0001", "ASM-0002"]], ["direction:Asked", ["QST-0001"]]]],
      ],
      noId: 1,
    });
  });

  it("puts a child under the first direction listing its kind, never the last (M: the last matching direction)", () => {
    const [, tides] = insideOf(aTreeView(ferryTree()), directions).groups;
    const holding = (name: string) => tides?.sets.find((set) => set.entries.some((entry) => entry.name === name));
    expect(["ASM-0001", "ASM-0002", "QST-0001"].map((name) => setShape(holding(name) ?? { by: "kind", kind: "none", entries: [] })[0])).toEqual(["direction:Records", "direction:Records", "direction:Asked"]);
  });

  it("lists directions first, in served order, then the raw kinds no direction lists by code point, no kind last (M: directions after raw kinds)", () => {
    const rows = [
      aTreeNode({ id: "D", depth: 0, path: "d.md" }),
      aTreeNode({ id: "Z-1", depth: 1, parent: "D", kind: "zeta", path: "z1.md" }),
      aTreeNode({ id: "Q-1", depth: 1, parent: "D", kind: "question", path: "q1.md" }),
      aTreeNode({ id: "N-1", depth: 1, parent: "D", kind: null, path: "n1.md" }),
      aTreeNode({ id: "A-2", depth: 1, parent: "D", kind: "assumption", path: "a2.md" }),
      aTreeNode({ id: "B-1", depth: 1, parent: "D", kind: "beta", path: "b1.md" }),
      aTreeNode({ id: "A-1", depth: 1, parent: "D", kind: "assumption", path: "a1.md" }),
    ];
    expect(shape(rows, directions).groups[0]?.[3]).toEqual([
      ["direction:Records", ["A-2", "A-1"]],
      ["direction:Asked", ["Q-1"]],
      ["beta", ["B-1"]],
      ["zeta", ["Z-1"]],
      [null, ["N-1"]],
    ]);
  });

  it("shows no group for a direction whose kinds match no child, and none at all with no direction: the raw kinds as before", () => {
    const rows = [aTreeNode({ id: "D", depth: 0, path: "d.md" }), aTreeNode({ id: "M-1", depth: 1, parent: "D", kind: "mechanic", path: "m1.md" })];
    expect(shape(rows, directions).groups[0]?.[3]).toEqual([["mechanic", ["M-1"]]]);
    expect(shape(ferryTree(), [])).toEqual(shape(ferryTree()));
    expect(insideDirections([aModule({ name: "river", root: "DOM-RIVER" })])).toEqual([]);
  });

  it("matches a kind exactly: a node with no kind joins no direction, a kind of another case none", () => {
    const loud = insideDirections([aDirection({ name: "loud", title: "Loud", kinds: ["Assumption"] })]);
    const rows = [aTreeNode({ id: "D", depth: 0, path: "d.md" }), aTreeNode({ id: "A-1", depth: 1, parent: "D", kind: "assumption", path: "a1.md" }), aTreeNode({ id: "N-1", depth: 1, parent: "D", kind: null, path: "n1.md" })];
    expect(shape(rows, loud).groups[0]?.[3]).toEqual([
      ["assumption", ["A-1"]],
      [null, ["N-1"]],
    ]);
  });
});
