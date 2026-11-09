import { describe, expect, it } from "vitest";
import type { FlowNode } from "../api/types";
import { aFlowEdge, aFlowNode, aFlowView } from "../test/builders";
import { aLine, flow200, forwardCrossings, TYPE_B } from "../test/flowStub";
import { stretchCrosses, type Point } from "./geometry";
import { BOX_HEIGHT, BOX_WIDTH, boxAfterKey, CANVAS_LIMIT, COLUMN_GAP, layoutFlow, LANE_PAD, type FlowBox, type FlowLayout, type LaidEdge } from "./layout";

// docs/features/flow-canvas.md "Layout": AC-04 layers, AC-05 arcs, AC-11's cap, AC-12 the trial of
// owner's decision 11 and the keys' moves; one read, one lane (docs/features/writer-studio.md D1).
// Pure: a read in, positions out.

function box(layout: FlowLayout, name: string): FlowBox {
  const found = layout.boxes.find((candidate) => candidate.name === name);
  if (found === undefined) {
    throw new Error(`no box ${name}`);
  }
  return found;
}

function edge(layout: FlowLayout, from: string, to: string): LaidEdge {
  const found = layout.edges.find((candidate) => candidate.edge.from === from && candidate.edge.to === to);
  if (found === undefined) {
    throw new Error(`no edge ${from} -> ${to}`);
  }
  return found;
}

/** Whether a line's stretches pass through a box other than its own two ends'. */
function crossesABox(laid: LaidEdge, layout: FlowLayout): FlowBox[] {
  const stretches: [Point, Point][] = [];
  laid.points.forEach((point, index) => {
    const next = laid.points[index + 1];
    if (next !== undefined) {
      stretches.push([point, next]);
    }
  });
  return layout.boxes.filter((candidate) => stretches.some(([a, b]) => stretchCrosses(a, b, candidate)));
}

function overlapping(boxes: readonly FlowBox[]): string[] {
  const hits: string[] = [];
  boxes.forEach((a, index) => {
    for (const b of boxes.slice(index + 1)) {
      if (a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height) {
        hits.push(`${a.name} on ${b.name}`);
      }
    }
  });
  return hits;
}

describe("layers (AC-04)", () => {
  it("runs boxes at distance 0, 4 and 9 left to right: a column per distinct layer, no empty column", () => {
    const layout = layoutFlow(aFlowView([aFlowNode({ id: "NINE", distance: 9 }), aFlowNode({ id: "ZERO", distance: 0 }), aFlowNode({ id: "FOUR", distance: 4 })], []));
    expect(box(layout, "ZERO").x).toBeLessThan(box(layout, "FOUR").x);
    expect(box(layout, "FOUR").x).toBeLessThan(box(layout, "NINE").x);
    expect(["ZERO", "FOUR", "NINE"].map((name) => box(layout, name).column)).toEqual([0, 1, 2]);
    expect(box(layout, "FOUR").x - box(layout, "ZERO").x).toBe(BOX_WIDTH + COLUMN_GAP);
  });

  it("lays layer null in the band under the lane, its caption over them", () => {
    const layout = layoutFlow(aFlowView([...aLine(["A", "B"]), aFlowNode({ id: "DEF", distance: null })], []));
    const definition = box(layout, "DEF");
    expect(definition.column).toBeNull();
    expect(definition.y).toBeGreaterThan(box(layout, "A").y + BOX_HEIGHT);
    expect(layout.lanes[0]?.band).toEqual([["n:DEF"]]);
    expect(layout.lanes[0]?.bandY).toBeLessThan(definition.y);
  });

  it("pulls a join between its branches: rows are ordered, then aligned", () => {
    const nodes = [aFlowNode({ id: "S", distance: 0 }), aFlowNode({ id: "UP", distance: 1 }), aFlowNode({ id: "DOWN", distance: 1 }), aFlowNode({ id: "JOIN", distance: 2 })];
    const edges = [aFlowEdge({ from: "S", to: "UP" }), aFlowEdge({ from: "S", to: "DOWN" }), aFlowEdge({ from: "UP", to: "JOIN" }), aFlowEdge({ from: "DOWN", to: "JOIN" })];
    const layout = layoutFlow(aFlowView(nodes, edges));
    const middle = (box(layout, "UP").y + box(layout, "DOWN").y) / 2;
    expect(box(layout, "JOIN").y).toBeCloseTo(middle, 0);
    expect(box(layout, "S").y).toBeCloseTo(middle, 0);
  });

  it("orders rows by the neighbours: two parallel lines never swap", () => {
    const nodes = [aFlowNode({ id: "A1", distance: 0 }), aFlowNode({ id: "A2", distance: 0 }), aFlowNode({ id: "B1", distance: 1, path: "docs/spec/z.md" }), aFlowNode({ id: "B2", distance: 1, path: "docs/spec/a.md" })];
    const layout = layoutFlow(aFlowView(nodes, [aFlowEdge({ from: "A1", to: "B1" }), aFlowEdge({ from: "A2", to: "B2" })]));
    expect(forwardCrossings(layout)).toBe(0);
    expect(box(layout, "A1").y < box(layout, "A2").y).toBe(box(layout, "B1").y < box(layout, "B2").y);
  });
});

describe("edges (AC-05): columns decide the shape, never a kind", () => {
  const nodes = [...aLine(["A", "B", "C", "D"]), aFlowNode({ id: "A2", distance: 0 }), aFlowNode({ id: "B2", distance: 1 }), aFlowNode({ id: "C2", distance: 2 })];
  const edges = [
    aFlowEdge({ from: "A", to: "B", kind: "repeat" }),
    aFlowEdge({ from: "B", to: "C" }),
    aFlowEdge({ from: "C", to: "D" }),
    aFlowEdge({ from: "D", to: "A", kind: "repeat" }),
    aFlowEdge({ from: "C", to: "B" }),
    aFlowEdge({ from: "A", to: "A2" }),
    aFlowEdge({ from: "B", to: "B", kind: "repeat" }),
  ];
  const layout = layoutFlow(aFlowView(nodes, edges));

  it("arcs a backward edge with a kind and one without; a forward edge with a kind curves", () => {
    expect(edge(layout, "D", "A").route).toBe("arc");
    expect(edge(layout, "C", "B").route).toBe("arc");
    expect(edge(layout, "A", "B").route).toBe("forward");
    expect(edge(layout, "B", "C").route).toBe("forward");
  });

  it("puts an edge within a column in the gap on its right, an edge to itself under the box", () => {
    expect(edge(layout, "A", "A2").route).toBe("beside");
    const beside = edge(layout, "A", "A2");
    const [, turn] = beside.points;
    expect(turn?.x).toBeGreaterThan(box(layout, "A").x + BOX_WIDTH);
    expect(turn?.x).toBeLessThan(box(layout, "B").x);
    const loop = edge(layout, "B", "B");
    expect(loop.route).toBe("self");
    expect(loop.points.every((point) => point.y === box(layout, "B").y + BOX_HEIGHT)).toBe(true);
  });

  it("keeps a box clear of the row under a loop, its label on the free side", () => {
    expect(box(layout, "B2").y).toBeGreaterThan(box(layout, "B").y + BOX_HEIGHT + 24);
    expect(edge(layout, "B", "B").label.y).toBeGreaterThan(box(layout, "B").y + BOX_HEIGHT);
  });

  it("lets no arc point fall in a box: it runs through the gaps, on its own track under the lane", () => {
    const arcs = layout.edges.filter((laid) => laid.route === "arc");
    expect(arcs.length).toBe(2);
    for (const arc of arcs) {
      expect(crossesABox(arc, layout)).toEqual([]);
    }
    const tracks = arcs.map((arc) => arc.points[2]?.y);
    expect(new Set(tracks).size).toBe(2);
    const bottom = Math.max(...layout.boxes.map((candidate) => candidate.y + candidate.height));
    expect(tracks.every((track) => (track ?? 0) > bottom)).toBe(true);
  });

  it("gives the shorter arc the track nearer the boxes, so the longer wraps it", () => {
    expect(edge(layout, "C", "B").points[2]?.y).toBeLessThan(edge(layout, "D", "A").points[2]?.y ?? 0);
  });

  it("enters an arc's target by its left port, as any step is entered", () => {
    const arc = edge(layout, "D", "A");
    const last = arc.points[arc.points.length - 1];
    expect(last?.x).toBe(box(layout, "A").x);
  });

  it("keeps a lane's own margin for the arcs at its ends", () => {
    expect(box(layout, "A").x).toBeGreaterThan(LANE_PAD);
    const right = Math.max(...layout.boxes.map((candidate) => candidate.x + candidate.width));
    expect(layout.width).toBeGreaterThan(right + LANE_PAD);
  });

  it("carries a long forward edge past the boxes between: a passage in each column it skips", () => {
    const skipping = layoutFlow(
      aFlowView(
        [...aLine(["P", "Q", "R", "S"]), aFlowNode({ id: "Q2", distance: 1 }), aFlowNode({ id: "R2", distance: 2 })],
        [aFlowEdge({ from: "P", to: "S" }), aFlowEdge({ from: "P", to: "Q" }), aFlowEdge({ from: "Q", to: "R" }), aFlowEdge({ from: "R", to: "S" }), aFlowEdge({ from: "P", to: "Q2" }), aFlowEdge({ from: "Q2", to: "R2" })],
      ),
    );
    const long = edge(skipping, "P", "S");
    expect(long.route).toBe("forward");
    expect(crossesABox(long, skipping)).toEqual([]);
    expect(overlapping(skipping.boxes)).toEqual([]);
  });

  it("routes an edge to the band from the gap right of its source, through the arc band, into the box's left port", () => {
    const banded = layoutFlow(aFlowView([...aLine(["A", "B", "C"]), aFlowNode({ id: "DEF", distance: null })], [aFlowEdge({ from: "A", to: "DEF", type: TYPE_B }), aFlowEdge({ from: "C", to: "DEF", type: TYPE_B })]));
    const first = edge(banded, "A", "DEF");
    expect(first.route).toBe("band");
    expect(crossesABox(first, banded)).toEqual([]);
    expect(first.points[first.points.length - 1]?.x).toBe(box(banded, "DEF").x);
    // Two edges to one definition share their last stretch: a trunk, not a tangle.
    expect(edge(banded, "C", "DEF").points[3]).toEqual(first.points[3]);
  });

  it("feeds a step from the band through the same band", () => {
    const banded = layoutFlow(aFlowView([...aLine(["A", "B"]), aFlowNode({ id: "DEF", distance: null })], [aFlowEdge({ from: "DEF", to: "B", type: TYPE_B })]));
    const fed = edge(banded, "DEF", "B");
    expect(fed.route).toBe("band");
    expect(fed.points[fed.points.length - 1]?.x).toBe(box(banded, "B").x);
    expect(crossesABox(fed, banded)).toEqual([]);
  });

  it("keeps a chain off the walk together, each link a line from port to port with no track", () => {
    const chain = layoutFlow(
      aFlowView(
        [...aLine(["A", "B", "C", "D"]), aFlowNode({ id: "Z-3", distance: null, path: "docs/a.md" }), aFlowNode({ id: "Z-1", distance: null, path: "docs/b.md" }), aFlowNode({ id: "Z-2", distance: null, path: "docs/c.md" })],
        [aFlowEdge({ from: "Z-1", to: "Z-2" }), aFlowEdge({ from: "Z-2", to: "Z-3" })],
      ),
    );
    expect(chain.lanes[0]?.band).toEqual([["n:Z-1", "n:Z-2", "n:Z-3"]]);
    const link = edge(chain, "Z-1", "Z-2");
    expect(link.points[0]?.x).toBe(box(chain, "Z-1").x + BOX_WIDTH);
    expect(link.points[link.points.length - 1]?.x).toBe(box(chain, "Z-2").x);
    expect(link.points.every((point) => point.y >= box(chain, "Z-1").y)).toBe(true);
    expect(crossesABox(link, chain)).toEqual([]);
  });

  it("spreads edges along a box's side, so two between one pair of boxes stay two", () => {
    const pair = layoutFlow(aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B" }), aFlowEdge({ from: "A", to: "B", type: TYPE_B })]));
    const [first, second] = pair.edges;
    expect(first?.points[0]?.y).not.toBe(second?.points[0]?.y);
    expect(first?.id).not.toBe(second?.id);
  });

  it("stacks the words of two edges between one pair of boxes", () => {
    const pair = layoutFlow(aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B", kind: "alternative", when: "one way" }), aFlowEdge({ from: "A", to: "B", kind: "parallel", when: "other way" })]));
    const [first, second] = pair.edges;
    expect(Math.abs((first?.label.y ?? 0) - (second?.label.y ?? 0))).toBeGreaterThanOrEqual(30);
  });

  it("draws an edge with an undrawn end not at all", () => {
    const missing = layoutFlow(aFlowView(aLine(["A"]), [aFlowEdge({ from: "A", to: "NOWHERE" })]));
    expect(missing.edges).toEqual([]);
  });
});

describe("a group", () => {
  it("is a box the size of any other, its holdings its own", () => {
    const layout = layoutFlow(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 3 }), aFlowNode({ id: "AFTER", distance: 1 })], [aFlowEdge({ from: "G", to: "AFTER", count: 7 })]));
    expect([box(layout, "G").width, box(layout, "G").height]).toEqual([BOX_WIDTH, BOX_HEIGHT]);
    expect(box(layout, "G").node.holds).toBe(3);
    expect(edge(layout, "G", "AFTER").edge.count).toBe(7);
  });
});

describe("two holders of one ID in a read", () => {
  const first = aFlowNode({ id: "TWIN", distance: 0, path: "docs/spec/a.md", line: 1 });
  const second = aFlowNode({ id: "TWIN", distance: 1, path: "docs/spec/b.md", line: 12 });
  const after = aFlowNode({ id: "AFTER", distance: 2 });
  const layout = layoutFlow(aFlowView([first, second, after], [aFlowEdge({ from: "TWIN", to: "AFTER" })]));

  it("draws two boxes, each with its own id; nothing is called undrawn", () => {
    const ids = layout.boxes.map((candidate) => candidate.id);
    expect(ids).toEqual(["n:TWIN", "n:TWIN@docs/spec/b.md:12", "n:AFTER"]);
    expect(new Set(ids).size).toBe(3);
    expect([layout.total, layout.notDrawn]).toEqual([3, 0]);
    expect(layout.lanes[0]?.columns).toEqual([["n:TWIN"], ["n:TWIN@docs/spec/b.md:12"], ["n:AFTER"]]);
  });

  it("lands a link on the first holder", () => {
    expect(edge(layout, "TWIN", "AFTER").source).toBe("n:TWIN");
  });

});

describe("the cap (AC-11)", () => {
  it("draws the first 200 boxes in served order, no edge past it; the rest counted, nothing said under it (R6)", () => {
    const line = aFlowView(
      Array.from({ length: 205 }, (_, index) => aFlowNode({ id: `N-${String(index).padStart(3, "0")}`, distance: Math.floor(index / 20) })),
      [aFlowEdge({ from: "N-000", to: "N-204" }), aFlowEdge({ from: "N-000", to: "N-021" })],
    );
    const cut = layoutFlow(line);
    expect(cut.boxes).toHaveLength(CANVAS_LIMIT);
    expect(cut.boxes.map((one) => one.name)).toEqual(line.nodes.slice(0, CANVAS_LIMIT).map((node) => node.id));
    expect([cut.total, cut.notDrawn]).toEqual([205, 5]);
    expect(cut.edges.map((laid) => laid.edge.to)).toEqual(["N-021"]);
    expect(layoutFlow(aFlowView(aLine(["A", "B"]), [])).notDrawn).toBe(0);
  });
});

describe("the trial of owner's decision 11 (AC-12)", () => {
  const trial = flow200();
  const started = performance.now();
  const layout = layoutFlow(trial);
  const spent = performance.now() - started;

  it("holds 200 boxes, 12 columns, 30 off the walk, 40 edges against it", () => {
    expect(layout.boxes).toHaveLength(200);
    expect(layout.lanes[0]?.columns).toHaveLength(12);
    expect(layout.boxes.filter((candidate) => candidate.column === null)).toHaveLength(30);
    expect(layout.edges.filter((laid) => laid.route === "arc")).toHaveLength(40);
    expect(layout.notDrawn).toBe(0);
  });

  it("overlaps no box", () => {
    expect(overlapping(layout.boxes)).toEqual([]);
  });

  it("runs every forward edge right, clear of every box", () => {
    const forward = layout.edges.filter((laid) => laid.route === "forward");
    expect(forward.length).toBeGreaterThan(150);
    for (const laid of forward) {
      laid.points.forEach((point, index) => {
        const next = laid.points[index + 1];
        if (next !== undefined) {
          expect(next.x).toBeGreaterThanOrEqual(point.x);
        }
      });
      const inColumns = laid.points.flatMap((point, index) => (index % 2 === 0 && laid.points[index + 1] !== undefined ? [[point, laid.points[index + 1]] as const] : []));
      for (const [from, to] of inColumns) {
        if (to !== undefined) {
          expect(layout.boxes.filter((candidate) => stretchCrosses(from, to, candidate))).toEqual([]);
        }
      }
    }
  });

  it("clears every box with every arc and every edge to the band", () => {
    const hits = layout.edges.filter((laid) => laid.route === "arc" || laid.route === "band").flatMap((laid) => crossesABox(laid, layout).map((crossed) => `${laid.id} through ${crossed.name}`));
    expect(hits).toEqual([]);
  });

  it("gives each arc its own track", () => {
    const tracks = layout.edges.filter((laid) => laid.route === "arc").map((laid) => laid.points[2]?.y);
    expect(new Set(tracks).size).toBe(40);
  });

  it("gives the same picture for the same reads in reverse order", () => {
    const reversed = { ...trial, nodes: [...trial.nodes].reverse(), edges: [...trial.edges].reverse() };
    const again = layoutFlow(reversed);
    const places = (boxes: readonly FlowBox[]) => boxes.map((one) => [one.id, one.column, one.row, one.x, one.y]);
    expect(places(again.boxes)).toEqual(places(layout.boxes));
    expect(again.edges.map((laid) => [laid.id, laid.path])).toEqual(layout.edges.map((laid) => [laid.id, laid.path]));
  });

  it("takes under 500 ms", () => {
    expect(spent).toBeLessThan(500);
  });

  it("orders the rows: a floor against an ordering gone wrong, on wiring this random", () => {
    expect(forwardCrossings(layout)).toBeLessThan(2500);
  });
});

describe("keys' moves", () => {
  const layout = layoutFlow(aFlowView([aFlowNode({ id: "A", distance: 0 }), aFlowNode({ id: "B", distance: 1 }), aFlowNode({ id: "C", distance: 1 }), aFlowNode({ id: "LOOSE", distance: null }), aFlowNode({ id: "LOOSE2", distance: null })], []));

  it("goes up and down within a column, then down into the band", () => {
    expect(boxAfterKey(layout, "n:B", "ArrowDown")).toBe("n:C");
    expect(boxAfterKey(layout, "n:C", "ArrowUp")).toBe("n:B");
    expect(boxAfterKey(layout, "n:B", "ArrowUp")).toBeNull();
    expect(boxAfterKey(layout, "n:C", "ArrowDown")).toMatch(/^n:LOOSE2?$/);
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowUp")).not.toBeNull();
  });

  it("goes across to the nearest box of the column beside, and stays at either end", () => {
    expect(boxAfterKey(layout, "n:A", "ArrowRight")).toMatch(/^n:[BC]$/);
    expect(boxAfterKey(layout, "n:B", "ArrowLeft")).toBe("n:A");
    expect(boxAfterKey(layout, "n:A", "ArrowLeft")).toBeNull();
    expect(boxAfterKey(layout, "n:B", "ArrowRight")).toBeNull();
  });

  it("moves along the band, staying at its ends", () => {
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowRight")).toBe("n:LOOSE2");
    expect(boxAfterKey(layout, "n:LOOSE2", "ArrowLeft")).toBe("n:LOOSE");
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowLeft")).toBeNull();
    expect(boxAfterKey(layout, "n:LOOSE2", "ArrowRight")).toBeNull();
  });

  it("stays on an unknown key, on an unknown box", () => {
    expect(boxAfterKey(layout, "n:A", "x")).toBeNull();
    expect(boxAfterKey(layout, "n:GONE", "ArrowDown")).toBeNull();
  });
});

function nodesOf(layout: FlowLayout): FlowNode[] {
  return layout.boxes.map((candidate) => candidate.node);
}

describe("what a box carries", () => {
  it("is the read's node, unchanged", () => {
    const node = aFlowNode({ id: "A", distance: 0, gaps: 3, inner: [{ type: "alpha_link", count: 2 }] });
    expect(nodesOf(layoutFlow(aFlowView([node], [])))).toEqual([node]);
  });
});
