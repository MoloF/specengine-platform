import { describe, expect, it } from "vitest";
import type { GraphEdge, GraphNode, GraphView } from "../api/types";
import { aGraphEdge, aGraphNode, aGraphView } from "../test/builders";
import {
  BOX_HEIGHT,
  BOX_WIDTH,
  CANVAS_LIMIT,
  COLUMN_LIMIT,
  COLUMN_STEP,
  endsOf,
  layoutGraph,
  patternAt,
  PATTERNS,
  ROW_STEP,
  type Layout,
  type LayoutBox,
} from "./layout";

// AC-05 and AC-10 of docs/features/ui-graph.md: the hand-written layered layout. Columns by
// distance, each walked name once; no overlap; handles and arrow as step 6; stubs for ends outside
// `nodes`; 20 seeded shuffles of the input give the same picture; collapse and the 200-box limit.

const TYPES: GraphView["types"] = [
  { type: "zeta_type", direction: "out" },
  { type: "alpha_type", direction: "in" },
  { type: "gamma_type", direction: "out" },
];

const node = (id: string, distance: number, path = `docs/spec/${id.toLowerCase()}.md`, line = 1): GraphNode =>
  aGraphNode({ id, distance, path, line });

/** A walk from R-0: three at distance 1, two at distance 2, every kind of edge and stub. */
function answer(): GraphView {
  const nodes = [
    node("R-0", 0),
    // Path order differs from name order: (path, line) decides, never the name.
    node("A1", 1, "docs/spec/c.md"),
    node("A2", 1, "docs/spec/a.md"),
    node("A3", 1, "docs/spec/b.md"),
    node("B1", 2, "docs/spec/e.md", 4),
    node("B2", 2, "docs/spec/e.md", 2),
  ];
  const edges: GraphEdge[] = [
    aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A1", line: 3 }),
    aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A3", line: 4 }),
    // Followed in: the walk goes from its dst R-0 to its src A2; the arrow stays at R-0.
    aGraphEdge({ src: "A2", type: "alpha_type", dst: "R-0" }),
    aGraphEdge({ src: "A1", type: "zeta_type", dst: "B2" }),
    aGraphEdge({ src: "A3", type: "zeta_type", dst: "B1" }),
    // Back to an earlier column, and within one column.
    aGraphEdge({ src: "B1", type: "zeta_type", dst: "A1", line: 9 }),
    aGraphEdge({ src: "A1", type: "zeta_type", dst: "A3", line: 7 }),
    // Unresolved ends: a stub each, a column after their source.
    aGraphEdge({ src: "A1", type: "zeta_type", dst: null, written: "R-405", line: 8, state: "dangling", reason: "`R-405` resolves to nothing" }),
    aGraphEdge({ src: "A2", type: "zeta_type", dst: null, written: "R-404", state: "dangling", reason: "`R-404` resolves to nothing" }),
    aGraphEdge({ src: "B2", type: "gamma_type", dst: null, written: "other:R-9", state: "skipped" }),
    // A nested section named by itself, twice: one stub.
    aGraphEdge({ src: "SEC-X", type: "zeta_type", dst: "B1", path: "docs/spec/r-0.md", line: 12 }),
    aGraphEdge({ src: "SEC-X", type: "gamma_type", dst: "A2", path: "docs/spec/r-0.md", line: 13 }),
    // No walked end at all: its stubs go after the last column.
    aGraphEdge({ src: "SEC-Y", type: "zeta_type", dst: null, written: "R-406", path: "docs/spec/r-0.md", line: 14, state: "dangling" }),
  ];
  return aGraphView(nodes, edges, { types: TYPES });
}

function boxNamed(layout: Layout, id: string): LayoutBox {
  const box = layout.boxes.find((candidate) => candidate.id === id);
  if (box === undefined) {
    throw new Error(`no box ${id}`);
  }
  return box;
}

function edgeBetween(layout: Layout, src: string | null, type: string, dst: string | null) {
  const found = layout.edges.find((edge) => edge.edge.src === src && edge.edge.type === type && edge.edge.dst === dst);
  if (found === undefined) {
    throw new Error(`no edge ${String(src)} ${type} ${String(dst)}`);
  }
  return found;
}

/** A seeded generator (mulberry32): the same shuffles on every run. */
function seeded(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function shuffled<T>(items: readonly T[], random: () => number): T[] {
  const copy = [...items];
  for (let at = copy.length - 1; at > 0; at -= 1) {
    const other = Math.floor(random() * (at + 1));
    const here = copy[at];
    const there = copy[other];
    if (here !== undefined && there !== undefined) {
      copy[at] = there;
      copy[other] = here;
    }
  }
  return copy;
}

/** Everything the canvas draws, as plain data. */
function picture(layout: Layout) {
  return {
    boxes: layout.boxes.map((box) => [box.id, box.column, box.row, box.x, box.y]),
    edges: layout.edges.map((edge) => [edge.id, edge.source, edge.target, edge.sourceHandle, edge.targetHandle, edge.arrowAt, edge.position]),
    columns: layout.columns,
    limit: layout.limit,
  };
}

describe("columns and boxes (AC-05)", () => {
  it("draws each walked name once, in its distance's column, on the 336 x 88 grid", () => {
    const layout = layoutGraph(answer());
    const walked = layout.boxes.filter((box) => box.content.type === "walked");
    expect(walked.map((box) => box.id).sort()).toEqual(["n:A1", "n:A2", "n:A3", "n:B1", "n:B2", "n:R-0"]);
    for (const box of walked) {
      if (box.content.type !== "walked") {
        throw new Error("not walked");
      }
      expect([box.id, box.column]).toEqual([box.id, box.content.walked.distance]);
      expect([box.x, box.y]).toEqual([COLUMN_STEP * box.column, ROW_STEP * box.row]);
    }
  });

  it("lets no two boxes overlap", () => {
    const boxes = layoutGraph(answer()).boxes;
    for (const [index, a] of boxes.entries()) {
      for (const b of boxes.slice(index + 1)) {
        const apart = a.x + BOX_WIDTH <= b.x || b.x + BOX_WIDTH <= a.x || a.y + BOX_HEIGHT <= b.y || b.y + BOX_HEIGHT <= a.y;
        expect([a.id, b.id, apart]).toEqual([a.id, b.id, true]);
      }
    }
  });

  it("orders a column by (path, line) before the sweeps, never by name", () => {
    const layout = layoutGraph(aGraphView([node("R-0", 0), node("A1", 1, "docs/spec/z.md"), node("A2", 1, "docs/spec/a.md")], [
      aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A1" }),
      aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A2" }),
    ], { types: TYPES }));
    expect(layout.columns.map((column) => column.boxes)).toEqual([["n:R-0"], ["n:A2", "n:A1"]]);
  });

  it("gives one box to a name with several holders, holders by (path, line)", () => {
    const layout = layoutGraph(
      aGraphView(
        [node("R-0", 0), node("DUP", 1, "docs/spec/z.md", 9), node("DUP", 1, "docs/spec/a.md", 4)],
        [aGraphEdge({ src: "R-0", type: "zeta_type", dst: "DUP" })],
        { types: TYPES },
      ),
    );
    const box = boxNamed(layout, "n:DUP");
    expect(layout.boxes.filter((candidate) => candidate.id === "n:DUP")).toHaveLength(1);
    expect(box.content.type === "walked" ? box.content.walked.holders.map((holder) => holder.path) : []).toEqual([
      "docs/spec/a.md",
      "docs/spec/z.md",
    ]);
  });

  it("uncrosses edges: a column follows the mean row of its walked neighbours in the column before", () => {
    const nodes = [node("R-0", 0), node("P1", 1, "docs/spec/a.md"), node("P2", 1, "docs/spec/b.md"), node("Q1", 2, "docs/spec/a2.md"), node("Q2", 2, "docs/spec/b2.md")];
    const edges = [
      aGraphEdge({ src: "R-0", type: "zeta_type", dst: "P1" }),
      aGraphEdge({ src: "R-0", type: "zeta_type", dst: "P2" }),
      aGraphEdge({ src: "P1", type: "zeta_type", dst: "Q2" }),
      aGraphEdge({ src: "P2", type: "zeta_type", dst: "Q1" }),
    ];
    const layout = layoutGraph(aGraphView(nodes, edges, { types: TYPES }));
    expect(layout.columns.map((column) => column.boxes)).toEqual([["n:R-0"], ["n:P1", "n:P2"], ["n:Q2", "n:Q1"]]);
  });
});

describe("handles and arrows (AC-05, step 6)", () => {
  it("joins the next column out (right) to in (left), the arrow at dst", () => {
    const edge = edgeBetween(layoutGraph(answer()), "R-0", "zeta_type", "A1");
    expect([edge.source, edge.target, edge.sourceHandle, edge.targetHandle, edge.arrowAt]).toEqual(["n:R-0", "n:A1", "out", "in", "end"]);
  });

  it("walks a type followed in from its dst: the arrow at the walk-from end (markerStart)", () => {
    const edge = edgeBetween(layoutGraph(answer()), "A2", "alpha_type", "R-0");
    expect([edge.source, edge.target, edge.sourceHandle, edge.targetHandle, edge.arrowAt]).toEqual(["n:R-0", "n:A2", "out", "in", "start"]);
  });

  it("joins the same or an earlier column top to top", () => {
    const layout = layoutGraph(answer());
    const back = edgeBetween(layout, "B1", "zeta_type", "A1");
    const level = edgeBetween(layout, "A1", "zeta_type", "A3");
    expect([back.sourceHandle, back.targetHandle]).toEqual(["out-top", "in-top"]);
    expect([level.sourceHandle, level.targetHandle]).toEqual(["out-top", "in-top"]);
  });

  it("styles a type by its position in `types`, six patterns cycling", () => {
    const layout = layoutGraph(answer());
    expect(edgeBetween(layout, "R-0", "zeta_type", "A1").position).toBe(0);
    expect(edgeBetween(layout, "A2", "alpha_type", "R-0").position).toBe(1);
    expect(PATTERNS).toHaveLength(6);
    expect(patternAt(6)).toBe(patternAt(0));
    expect(new Set(PATTERNS).size).toBe(6);
  });

  it("styles a type by its place in the mode's chip order, unchanged when another type is not followed", () => {
    const order = ["zeta_type", "alpha_type", "gamma_type", "mentions"];
    const view = answer();
    const whole = layoutGraph(view, order);
    expect(edgeBetween(whole, "B2", "gamma_type", null).position).toBe(2);
    // alpha_type released: the answer no longer lists it, gamma_type keeps its pattern.
    const filtered = layoutGraph(
      { ...view, types: TYPES.filter((followed) => followed.type !== "alpha_type"), edges: view.edges.filter((edge) => edge.type !== "alpha_type") },
      order,
    );
    expect(edgeBetween(filtered, "B2", "gamma_type", null).position).toBe(2);
    expect(edgeBetween(filtered, "R-0", "zeta_type", "A1").position).toBe(0);
  });
});

describe("stubs (AC-06)", () => {
  it("gives every null end an unresolved stub a column after its source, never a walked box", () => {
    const layout = layoutGraph(answer());
    const unresolved = layout.boxes.filter((box) => box.content.type === "stub" && box.content.stub.stub.type === "unresolved");
    expect(unresolved).toHaveLength(4);
    const written = (box: LayoutBox) => (box.content.type === "stub" && box.content.stub.stub.type === "unresolved" ? box.content.stub.stub.edge.written : "");
    const columns = Object.fromEntries(unresolved.map((box) => [written(box), box.column]));
    expect(columns).toEqual({ "R-405": 2, "R-404": 2, "other:R-9": 3, "R-406": 4 });
  });

  it("gives an end outside `nodes` one section stub per name, beside a walked end of its edges", () => {
    const layout = layoutGraph(answer());
    const sections = layout.boxes.filter((box) => box.content.type === "stub" && box.content.stub.stub.type === "section");
    // SEC-X walks to A2 (distance 1) by its first edge: one column before it.
    expect(sections.map((box) => [box.id, box.column])).toEqual([
      ["s:SEC-X", 0],
      ["s:SEC-Y", 4],
    ]);
    const stub = boxNamed(layout, "s:SEC-X");
    expect(stub.content.type === "stub" ? stub.content.stub.edges.map((edge) => edge.dst) : []).toEqual(["A2", "B1"]);
  });

  it("puts a section stub its edge is walked from one column before the walked end, never below 0", () => {
    const view = (far: number, types: GraphView["types"], edge: GraphEdge) =>
      aGraphView(
        [node("R-0", 0), ...Array.from({ length: far }, (_, at) => node(`P${String(at + 1)}`, at + 1))],
        [
          ...Array.from({ length: far }, (_, at) => aGraphEdge({ src: at === 0 ? "R-0" : `P${String(at)}`, type: "zeta_type", dst: `P${String(at + 1)}` })),
          edge,
        ],
        { types },
      );
    const out: GraphView["types"] = [{ type: "zeta_type", direction: "out" }];
    const into: GraphView["types"] = [{ type: "zeta_type", direction: "in" }];
    const column = (layout: Layout) => boxNamed(layout, "s:SEC").column;
    // Followed out, the walk goes from src: SEC at src is the walk-from end.
    expect(column(layoutGraph(view(2, out, aGraphEdge({ src: "SEC", type: "zeta_type", dst: "P2", line: 9 }))))).toBe(1);
    expect(column(layoutGraph(view(0, out, aGraphEdge({ src: "SEC", type: "zeta_type", dst: "R-0", line: 9 }))))).toBe(0);
    // Followed in, the walk goes from dst: SEC at dst is the walk-from end, at src the walk-to end.
    expect(column(layoutGraph(view(2, into, aGraphEdge({ src: "P2", type: "zeta_type", dst: "SEC", line: 9 }))))).toBe(1);
    expect(column(layoutGraph(view(2, into, aGraphEdge({ src: "SEC", type: "zeta_type", dst: "P2", line: 9 }))))).toBe(3);
    // The walk-to end: one column after, as an unresolved end.
    expect(column(layoutGraph(view(2, out, aGraphEdge({ src: "P2", type: "zeta_type", dst: "SEC", line: 9 }))))).toBe(3);
    // Its link to the walked end runs to the next column, right to left side: no arc.
    const layout = layoutGraph(view(2, out, aGraphEdge({ src: "SEC", type: "zeta_type", dst: "P2", line: 9 })));
    const edge = edgeBetween(layout, "SEC", "zeta_type", "P2");
    expect([edge.source, edge.target, edge.sourceHandle, edge.targetHandle]).toEqual(["s:SEC", "n:P2", "out", "in"]);
  });

  it("lists every stub for the List, the canvas's limits aside", () => {
    const view = answer();
    expect(endsOf(view.nodes, view.edges).stubs.map((stub) => stub.id)).toEqual([
      "u:dst:zeta_type:docs%2Fspec%2Fa1.md:8:R-405",
      "u:dst:zeta_type:docs%2Fspec%2Fa2.md:3:R-404",
      "u:dst:gamma_type:docs%2Fspec%2Fb2.md:3:other%3AR-9",
      "s:SEC-X",
      "s:SEC-Y",
      "u:dst:zeta_type:docs%2Fspec%2Fr-0.md:14:R-406",
    ]);
  });

  it("names an unresolved end the same in every answer, whatever edges come before it", () => {
    const view = answer();
    const idOf = (edges: GraphEdge[]) =>
      layoutGraph({ ...view, edges }).boxes.find(
        (box) => box.content.type === "stub" && box.content.stub.stub.type === "unresolved" && box.content.stub.stub.edge.written === "R-404",
      )?.id;
    const whole = idOf(view.edges);
    expect(whole).toBe("u:dst:zeta_type:docs%2Fspec%2Fa2.md:3:R-404");
    expect(idOf(view.edges.filter((edge) => edge.written !== "R-405"))).toBe(whole);
    expect(idOf(view.edges.filter((edge) => edge.type !== "alpha_type"))).toBe(whole);
    // The same link written twice on one line: two stubs, two ids.
    const twice = endsOf(view.nodes, [...view.edges, aGraphEdge({ src: "A2", type: "zeta_type", dst: null, written: "R-404", state: "dangling" })]);
    expect(twice.stubs.map((stub) => stub.id).filter((id) => id.includes("R-404"))).toEqual([whole, `${whole ?? ""}#2`]);
  });

  it("holds a link from a section to itself once", () => {
    const loop = aGraphEdge({ src: "SEC-Z", type: "zeta_type", dst: "SEC-Z", path: "docs/spec/r-0.md", line: 20 });
    const { stubs, ends } = endsOf([node("R-0", 0)], [loop]);
    expect(stubs.map((stub) => [stub.id, stub.edges])).toEqual([["s:SEC-Z", [loop]]]);
    expect(ends).toEqual([{ src: "s:SEC-Z", dst: "s:SEC-Z" }]);
  });
});

describe("determinism (AC-05)", () => {
  it("gives 20 seeded shuffles of nodes and edges the same positions, handles and ids", () => {
    const view = answer();
    const first = picture(layoutGraph(view));
    const random = seeded(20261006);
    for (let round = 0; round < 20; round += 1) {
      const again = layoutGraph({ ...view, nodes: shuffled(view.nodes, random), edges: shuffled(view.edges, random) });
      expect(picture(again)).toEqual(first);
    }
  });
});

describe("collapse and the canvas limit (AC-10)", () => {
  function fan(count: number): GraphView {
    const nodes = [node("R-0", 0), ...Array.from({ length: count }, (_, at) => node(`N-${String(at).padStart(3, "0")}`, 1))];
    const edges = nodes.slice(1).map((target) => aGraphEdge({ src: "R-0", type: "zeta_type", dst: target.id }));
    return aGraphView(nodes, edges, { types: TYPES });
  }

  it("keeps 24 boxes of a fuller column and one '+k more' box; edges to hidden boxes undrawn", () => {
    const layout = layoutGraph(fan(30));
    const column = layout.columns.find((candidate) => candidate.column === 1);
    expect(column?.boxes).toHaveLength(COLUMN_LIMIT + 1);
    const more = boxNamed(layout, "m:1");
    expect(more.content).toEqual({ type: "more", hidden: 6, hiddenWalked: 6, column: 1 });
    expect(more.row).toBe(COLUMN_LIMIT);
    expect(layout.edges).toHaveLength(COLUMN_LIMIT);
    expect(layout.limit).toBeNull();
  });

  it("counts the walked names a more-box hides: none when it hides only stubs", () => {
    const view = fan(20);
    const dangling = Array.from({ length: 10 }, (_, at) =>
      aGraphEdge({ src: "R-0", type: "zeta_type", dst: null, written: `GONE-${String(at)}`, line: 30 + at, state: "dangling" }),
    );
    const layout = layoutGraph({ ...view, edges: [...view.edges, ...dangling] });
    expect(boxNamed(layout, "m:1").content).toEqual({ type: "more", hidden: 6, hiddenWalked: 0, column: 1 });
    const mixed = layoutGraph({ ...fan(26), edges: [...fan(26).edges, ...dangling] });
    expect(boxNamed(mixed, "m:1").content).toEqual({ type: "more", hidden: 12, hiddenWalked: 2, column: 1 });
  });

  it("keeps a column of exactly 24 whole", () => {
    const layout = layoutGraph(fan(24));
    expect(layout.boxes.some((box) => box.content.type === "more")).toBe(false);
  });

  it("draws whole columns left to right up to 200 boxes: 12 distances x 20 nodes show 0-9, 200 of 240", () => {
    const nodes: GraphNode[] = [];
    const edges: GraphEdge[] = [];
    for (let distance = 0; distance < 12; distance += 1) {
      for (let at = 0; at < 20; at += 1) {
        const id = `D${String(distance).padStart(2, "0")}-${String(at).padStart(2, "0")}`;
        nodes.push(node(id, distance));
        if (distance > 0) {
          edges.push(aGraphEdge({ src: `D${String(distance - 1).padStart(2, "0")}-${String(at).padStart(2, "0")}`, type: "zeta_type", dst: id }));
        }
      }
    }
    const layout = layoutGraph(aGraphView(nodes, edges, { types: TYPES }));
    expect(layout.boxes).toHaveLength(CANVAS_LIMIT);
    expect(layout.columns.map((column) => column.column)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    expect(layout.limit).toEqual({ lastColumn: 9, shown: 200, total: 240 });
  });
});
