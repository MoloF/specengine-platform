import { describe, expect, it } from "vitest";
import { aTreeNode } from "../test/builders";
import { TREE } from "../test/treeStub";
import { ancestorsOf, buildRows, rowName, rowsForRef, visibleRows } from "./rows";

// AC-03 of docs/features/ui-tree-node.md: the hierarchy from depth alone, in pre-order.

const rows = buildRows(TREE);

function named(name: string, nth = 0) {
  const found = rows.filter((row) => rowName(row.node) === name)[nth];
  if (found === undefined) {
    throw new Error(`no row ${name}`);
  }
  return found;
}

describe("buildRows", () => {
  it("takes each row's parent from depth: the nearest earlier row one level up", () => {
    const parentOf = (name: string, nth = 0) => {
      const parent = named(name, nth).parent;
      return parent === null ? null : rowName(rows[parent]?.node ?? aTreeNode({ id: "?", depth: 0 }));
    };
    expect(parentOf("DOC-A")).toBeNull();
    expect(parentOf("SEC-A1X")).toBe("SEC-A1");
    expect(parentOf("SEC-DUP-KID")).toBe("SEC-DUP");
    expect(rows[named("SEC-DUP-KID").parent ?? -1]?.node.path).toBe("docs/spec/b.md");
    expect(parentOf("DOC-E")).toBe("DOC-D");
  });

  it("never looks a parent up by name: a parent field naming another row changes nothing", () => {
    const misleading = buildRows(TREE.map((node) => (node.id === "SEC-DUP-KID" ? { ...node, parent: "DOC-C" } : node)));
    const kid = misleading.find((row) => row.node.id === "SEC-DUP-KID");
    expect(misleading[kid?.parent ?? -1]?.node.line).toBe(9);
  });

  it("counts siblings for set size and position", () => {
    expect(["DOC-A", "DOC-C", "DOC-D", "DOC-F"].map((name) => [named(name).posInSet, named(name).setSize])).toEqual([
      [1, 4],
      [2, 4],
      [3, 4],
      [4, 4],
    ]);
    expect([named("SEC-DUP", 1).posInSet, named("SEC-DUP", 1).setSize]).toEqual([1, 1]);
  });

  it("keys rows by place, unique even when a place repeats", () => {
    const twice = buildRows([aTreeNode({ id: "A", depth: 0, path: "a.md" }), aTreeNode({ id: "A", depth: 0, path: "a.md" })]);
    expect(new Set(twice.map((row) => row.key)).size).toBe(2);
  });
});

describe("visibleRows", () => {
  it("shows a row only when every ancestor is expanded", () => {
    const open = new Set([named("DOC-A").key, named("DOC-D").key]);
    expect(visibleRows(rows, open).map((row) => rowName(row.node))).toEqual([
      "DOC-A",
      "SEC-A1",
      "SEC-DUP",
      "docs/spec/b.md",
      "DOC-C",
      "DOC-D",
      "DOC-E",
      "DOC-F",
    ]);
    expect(visibleRows(rows, new Set()).map((row) => rowName(row.node))).toEqual(["DOC-A", "DOC-C", "DOC-D", "DOC-F"]);
  });
});

describe("ancestorsOf and rowsForRef", () => {
  it("climbs nearest first", () => {
    expect(ancestorsOf(rows, named("SEC-DUP-KID")).map((row) => `${rowName(row.node)}@${String(row.node.line)}`)).toEqual([
      "SEC-DUP@9",
      "docs/spec/b.md@1",
      "DOC-A@1",
    ]);
  });

  it("finds the rows of a REF by its holders' places once read, by its forms before", () => {
    expect(rowsForRef(rows, "SEC-DUP", [{ path: "docs/spec/b.md", line: 9 }]).map((row) => row.node.line)).toEqual([9]);
    expect(rowsForRef(rows, "SEC-DUP", null).map((row) => row.node.line)).toEqual([30, 9]);
    expect(rowsForRef(rows, "a/DOC-A", null).map((row) => rowName(row.node))).toEqual(["DOC-A"]);
    expect(rowsForRef(rows, "DOC-A#SEC-A1", null).map((row) => rowName(row.node))).toEqual(["SEC-A1"]);
    expect(rowsForRef(rows, "docs/spec/b.md", null).map((row) => rowName(row.node))).toEqual(["docs/spec/b.md"]);
    expect(rowsForRef(rows, "R-404", [])).toEqual([]);
  });
});
