import { describe, expect, it } from "vitest";
import type { FlowNode } from "../api/types";
import { asideFiring, asideLens } from "../test/asideStub";
import { aFlowEdge, aFlowNode, aFlowView } from "../test/builders";
import { aLine, flow200, forwardCrossings, TYPE_B } from "../test/flowStub";
import { stretchCrosses, type Point } from "./geometry";
import { BOX_HEIGHT, BOX_WIDTH, BOX_GAP, boxAfterKey, CANVAS_LIMIT, layoutFlow, LANE_PAD, type FlowBox, type FlowLayout, type LaidEdge } from "./layout";

// docs/features/flow-canvas.md "Layout" and flow-scenario FS-15 (RM-0094): the walk top to bottom,
// a row per distance, the band under the last row, stage dividers where stages begin, arcs on their
// own tracks right of the walk; AC-11's cap, AC-12's trial of owner's decision 11 and the keys'
// moves; one read, one lane (docs/features/writer-studio.md D1). Pure: a read in, positions out.

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

/** The boxes a line's stretches pass through, its own two ends' excepted when it touches only their sides. */
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

/** The firing run as the daemon serves it, the run's own node left out as the screen leaves it. */
function firingBoxes() {
  const read = asideFiring();
  return { ...read, nodes: read.nodes.filter((node) => node.id !== "KRUN-FIRING") };
}

describe("rows (AC-04, FS-15)", () => {
  it("runs boxes at distance 0, 4 and 9 top to bottom: a row per distinct distance, no empty row, one above the other", () => {
    const layout = layoutFlow(aFlowView([aFlowNode({ id: "NINE", distance: 9 }), aFlowNode({ id: "ZERO", distance: 0 }), aFlowNode({ id: "FOUR", distance: 4 })], []));
    expect(box(layout, "ZERO").y).toBeLessThan(box(layout, "FOUR").y);
    expect(box(layout, "FOUR").y).toBeLessThan(box(layout, "NINE").y);
    expect(["ZERO", "FOUR", "NINE"].map((name) => box(layout, name).level)).toEqual([0, 1, 2]);
    // M: `x` by distance; a walk with one box a row stands as one column.
    expect(new Set(["ZERO", "FOUR", "NINE"].map((name) => box(layout, name).x)).size).toBe(1);
  });

  it("on the firing run: y grows with distance, one distance shares one y, Mending and Packing side by side", () => {
    const layout = layoutFlow(firingBoxes());
    const steps = layout.boxes.filter((one) => one.node.distance !== null).sort((a, b) => (a.node.distance ?? 0) - (b.node.distance ?? 0));
    steps.forEach((one, index) => {
      const next = steps[index + 1];
      if (next !== undefined) {
        expect([one.name, next.name, one.node.distance === next.node.distance ? one.y === next.y : one.y < next.y]).toEqual([one.name, next.name, true]);
      }
    });
    const [mend, pack] = [box(layout, "POT-MEND"), box(layout, "POT-PACK")];
    expect(mend.y).toBe(pack.y);
    expect(Math.abs(mend.x - pack.x)).toBeGreaterThanOrEqual(BOX_WIDTH + BOX_GAP);
    expect(overlapping(layout.boxes)).toEqual([]);
  });

  it("stands four dividers, wheelwork, bisque, glazing and crating, above rows 0, 3, 5 and 7; none in a stage view", () => {
    const layout = layoutFlow(firingBoxes());
    const lane = layout.lanes[0];
    expect(lane?.dividers.map((divider) => [divider.level, divider.stages])).toEqual([
      [0, ["wheelwork"]],
      [3, ["bisque"]],
      [5, ["glazing"]],
      [7, ["crating"]],
    ]);
    for (const divider of lane?.dividers ?? []) {
      const first = layout.boxes.find((one) => one.id === lane?.rows[divider.level]?.[0]);
      expect([divider.level, divider.y < (first?.y ?? 0)]).toEqual([divider.level, true]);
    }
    // M: a divider per row.
    expect(lane?.dividers).toHaveLength(4);
    const glazing = firingBoxes();
    const stage = layoutFlow({ ...glazing, nodes: glazing.nodes.filter((node) => node.stage === "glazing") });
    expect(stage.boxes).toHaveLength(2);
    expect(stage.lanes[0]?.dividers).toEqual([]);
  });

  it("draws no divider for a picture of fewer than two stages, and none for a band box's stage", () => {
    const one = layoutFlow(aFlowView([aFlowNode({ id: "A", distance: 0, stage: "only" }), aFlowNode({ id: "B", distance: 1, stage: "only" }), aFlowNode({ id: "C", distance: null, stage: "elsewhere" })], []));
    expect(one.lanes[0]?.dividers).toEqual([]);
  });

  it("shares one divider between stages beginning in one row, their words joined in row order", () => {
    const layout = layoutFlow(aFlowView([aFlowNode({ id: "A", distance: 0, stage: "first" }), aFlowNode({ id: "B", distance: 1, stage: "second", path: "docs/a.md" }), aFlowNode({ id: "C", distance: 1, stage: "third", path: "docs/b.md" })], []));
    expect(layout.lanes[0]?.dividers.map((divider) => divider.level)).toEqual([0, 1]);
    expect([...(layout.lanes[0]?.dividers[1]?.stages ?? [])].sort()).toEqual(["second", "third"]);
  });

  it("lays the boxes off the walk in the band under the last row, its caption over them, wrapped at four boxes", () => {
    const layout = layoutFlow(aFlowView([...aLine(["A", "B"]), ...["D1", "D2", "D3", "D4", "D5", "D6"].map((id) => aFlowNode({ id, distance: null }))], []));
    const band = layout.lanes[0]?.band ?? [];
    expect(band.map((shelf) => shelf.length)).toEqual([4, 2]);
    const bottom = box(layout, "B").y + BOX_HEIGHT;
    expect(layout.boxes.filter((one) => one.level === null).every((one) => one.y > bottom)).toBe(true);
    expect(layout.lanes[0]?.bandY).toBeGreaterThan(bottom);
    expect(layout.lanes[0]?.bandY).toBeLessThan(box(layout, "D1").y);
  });

  it("at the pottery's lens root, the gallery and the run in row 0, the nine parts in the band, its edges off the walk", () => {
    const read = asideLens();
    const layout = layoutFlow({ ...read, nodes: read.nodes.filter((node) => node.id !== "ATL-POTTERY") });
    expect(layout.lanes[0]?.rows.map((row) => row.length)).toEqual([2]);
    expect(layout.lanes[0]?.band.flat()).toHaveLength(9);
    expect(layout.lanes[0]?.band.map((shelf) => shelf.length)).toEqual([4, 4, 1]);
    expect(layout.edges).toHaveLength(9);
    expect(layout.edges.every((laid) => laid.offWalk && laid.route === "band")).toBe(true);
    for (const laid of layout.edges) {
      expect([laid.id, crossesABox(laid, layout).map((one) => one.name)]).toEqual([laid.id, []]);
    }
  });

  it("pulls a join between its branches: places are ordered, then aligned", () => {
    const nodes = [aFlowNode({ id: "S", distance: 0 }), aFlowNode({ id: "UP", distance: 1 }), aFlowNode({ id: "DOWN", distance: 1 }), aFlowNode({ id: "JOIN", distance: 2 })];
    const edges = [aFlowEdge({ from: "S", to: "UP" }), aFlowEdge({ from: "S", to: "DOWN" }), aFlowEdge({ from: "UP", to: "JOIN" }), aFlowEdge({ from: "DOWN", to: "JOIN" })];
    const layout = layoutFlow(aFlowView(nodes, edges));
    const middle = (box(layout, "UP").x + box(layout, "DOWN").x) / 2;
    expect(box(layout, "JOIN").x).toBeCloseTo(middle, 0);
    expect(box(layout, "S").x).toBeCloseTo(middle, 0);
  });

  it("orders places by the neighbours: two parallel lines never swap", () => {
    const nodes = [aFlowNode({ id: "A1", distance: 0 }), aFlowNode({ id: "A2", distance: 0 }), aFlowNode({ id: "B1", distance: 1, path: "docs/spec/z.md" }), aFlowNode({ id: "B2", distance: 1, path: "docs/spec/a.md" })];
    const layout = layoutFlow(aFlowView(nodes, [aFlowEdge({ from: "A1", to: "B1" }), aFlowEdge({ from: "A2", to: "B2" })]));
    expect(forwardCrossings(layout)).toBe(0);
    expect(box(layout, "A1").x < box(layout, "A2").x).toBe(box(layout, "B1").x < box(layout, "B2").x);
  });
});

describe("edges (AC-05, FS-15): rows decide the shape, never a kind", () => {
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

  it("arcs a backward edge with a kind and one without; a forward edge with a kind runs down", () => {
    expect(edge(layout, "D", "A").route).toBe("arc");
    expect(edge(layout, "C", "B").route).toBe("arc");
    expect(edge(layout, "A", "B").route).toBe("forward");
    expect(edge(layout, "B", "C").route).toBe("forward");
  });

  it("joins two boxes of a row side by side, and puts an edge to itself beside its box", () => {
    const beside = edge(layout, "A", "A2");
    expect(beside.route).toBe("beside");
    expect(crossesABox(beside, layout)).toEqual([]);
    const loop = edge(layout, "B", "B");
    expect(loop.route).toBe("self");
    expect(loop.points.every((point) => point.x === box(layout, "B").x + BOX_WIDTH)).toBe(true);
    expect(loop.label.x).toBeGreaterThan(box(layout, "B").x + BOX_WIDTH);
  });

  it("keeps a box clear of the loop beside its neighbour", () => {
    const b = box(layout, "B");
    const b2 = box(layout, "B2");
    expect(b2.x > b.x ? b2.x - (b.x + BOX_WIDTH) : b.x - (b2.x + BOX_WIDTH)).toBeGreaterThan(BOX_GAP);
  });

  it("lets no arc point fall in a box: it runs through the gaps, on its own track right of the walk", () => {
    const arcs = layout.edges.filter((laid) => laid.route === "arc");
    expect(arcs.length).toBe(2);
    const walkRight = Math.max(...layout.boxes.map((candidate) => candidate.x + candidate.width));
    for (const arc of arcs) {
      expect(crossesABox(arc, layout)).toEqual([]);
      expect(Math.max(...arc.points.map((point) => point.x))).toBeGreaterThan(walkRight);
    }
    const tracks = arcs.map((arc) => Math.max(...arc.points.map((point) => point.x)));
    expect(new Set(tracks).size).toBe(2);
  });

  it("gives the shorter arc the track nearer the walk, so the longer wraps it", () => {
    const track = (laid: LaidEdge) => Math.max(...laid.points.map((point) => point.x));
    expect(track(edge(layout, "C", "B"))).toBeLessThan(track(edge(layout, "D", "A")));
  });

  it("runs the firing run's repeat, Glost firing back to Dipping in glaze, right of the walk, crossing no box", () => {
    const firing = layoutFlow(firingBoxes());
    const repeat = edge(firing, "POT-FIRE", "POT-GLAZE");
    expect(repeat.route).toBe("arc");
    expect(crossesABox(repeat, firing)).toEqual([]);
    const walkRight = Math.max(...firing.boxes.map((candidate) => candidate.x + candidate.width));
    expect(Math.max(...repeat.points.map((point) => point.x))).toBeGreaterThan(walkRight);
    expect(firing.edges.filter((laid) => laid.offWalk)).toEqual([]);
    for (const laid of firing.edges) {
      expect([laid.id, crossesABox(laid, firing).map((one) => one.name)]).toEqual([laid.id, []]);
    }
  });

  it("keeps a lane's own margin round the walk", () => {
    expect(box(layout, "A").y).toBeGreaterThanOrEqual(LANE_PAD);
    const right = Math.max(...layout.boxes.map((candidate) => candidate.x + candidate.width));
    expect(layout.width).toBeGreaterThan(right + LANE_PAD);
  });

  it("carries a long forward edge past the rows between: a passage in each row it skips", () => {
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

  it("routes an edge to the band down the gap under its source, into the box's top, off the walk", () => {
    const banded = layoutFlow(aFlowView([...aLine(["A", "B", "C"]), aFlowNode({ id: "DEF", distance: null })], [aFlowEdge({ from: "A", to: "DEF", type: TYPE_B }), aFlowEdge({ from: "C", to: "DEF", type: TYPE_B })]));
    const first = edge(banded, "A", "DEF");
    expect(first.route).toBe("band");
    expect(first.offWalk).toBe(true);
    expect(crossesABox(first, banded)).toEqual([]);
    expect(first.points[first.points.length - 1]?.y).toBe(box(banded, "DEF").y);
    expect(crossesABox(edge(banded, "C", "DEF"), banded)).toEqual([]);
  });

  it("feeds a step from the band on a trunk left of the walk", () => {
    const banded = layoutFlow(aFlowView([...aLine(["A", "B"]), aFlowNode({ id: "DEF", distance: null })], [aFlowEdge({ from: "DEF", to: "B", type: TYPE_B })]));
    const fed = edge(banded, "DEF", "B");
    expect(fed.route).toBe("band");
    expect(fed.points[fed.points.length - 1]?.y).toBe(box(banded, "B").y);
    expect(Math.min(...fed.points.map((point) => point.x))).toBeLessThan(box(banded, "A").x);
    expect(crossesABox(fed, banded)).toEqual([]);
  });

  it("keeps a chain off the walk together, each link a line from side to side", () => {
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
    expect(crossesABox(link, chain)).toEqual([]);
  });

  it("spreads edges along a box's side, so two between one pair of boxes stay two", () => {
    const pair = layoutFlow(aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B" }), aFlowEdge({ from: "A", to: "B", type: TYPE_B })]));
    const [first, second] = pair.edges;
    expect(first?.points[0]?.x).not.toBe(second?.points[0]?.x);
    expect(first?.id).not.toBe(second?.id);
  });

  it("sets the words of two edges between one pair of boxes apart", () => {
    const pair = layoutFlow(aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B", kind: "alternative", when: "one way" }), aFlowEdge({ from: "A", to: "B", kind: "parallel", when: "other way" })]));
    const [first, second] = pair.edges;
    expect(Math.abs((first?.label.x ?? 0) - (second?.label.x ?? 0))).toBeGreaterThanOrEqual(30);
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
    expect(layout.lanes[0]?.rows).toEqual([["n:TWIN"], ["n:TWIN@docs/spec/b.md:12"], ["n:AFTER"]]);
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

  it("holds 200 boxes, 12 rows, 30 off the walk, 40 edges against it", () => {
    expect(layout.boxes).toHaveLength(200);
    expect(layout.lanes[0]?.rows).toHaveLength(12);
    expect(layout.boxes.filter((candidate) => candidate.level === null)).toHaveLength(30);
    expect(layout.edges.filter((laid) => laid.route === "arc")).toHaveLength(40);
    expect(layout.notDrawn).toBe(0);
  });

  it("overlaps no box", () => {
    expect(overlapping(layout.boxes)).toEqual([]);
  });

  it("runs every forward edge down, clear of every box", () => {
    const forward = layout.edges.filter((laid) => laid.route === "forward");
    expect(forward.length).toBeGreaterThan(150);
    for (const laid of forward) {
      laid.points.forEach((point, index) => {
        const next = laid.points[index + 1];
        if (next !== undefined) {
          expect(next.y).toBeGreaterThanOrEqual(point.y);
        }
      });
      const inRows = laid.points.flatMap((point, index) => (index % 2 === 0 && laid.points[index + 1] !== undefined ? [[point, laid.points[index + 1]] as const] : []));
      for (const [from, to] of inRows) {
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
    const tracks = layout.edges.filter((laid) => laid.route === "arc").map((laid) => Math.max(...laid.points.map((point) => point.x)));
    expect(new Set(tracks).size).toBe(40);
  });

  it("gives the same picture for the same reads in reverse order", () => {
    const reversed = { ...trial, nodes: [...trial.nodes].reverse(), edges: [...trial.edges].reverse() };
    const again = layoutFlow(reversed);
    const places = (boxes: readonly FlowBox[]) => boxes.map((one) => [one.id, one.level, one.order, one.x, one.y]);
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

describe("keys' moves (FS-15)", () => {
  const layout = layoutFlow(aFlowView([aFlowNode({ id: "A", distance: 0 }), aFlowNode({ id: "B", distance: 1 }), aFlowNode({ id: "C", distance: 1 }), aFlowNode({ id: "LOOSE", distance: null }), aFlowNode({ id: "LOOSE2", distance: null })], []));

  it("goes left and right within a row, and stays at either end", () => {
    expect(boxAfterKey(layout, "n:B", "ArrowRight")).toBe("n:C");
    expect(boxAfterKey(layout, "n:C", "ArrowLeft")).toBe("n:B");
    expect(boxAfterKey(layout, "n:B", "ArrowLeft")).toBeNull();
    expect(boxAfterKey(layout, "n:C", "ArrowRight")).toBeNull();
  });

  it("goes up and down across the rows to the nearest box, the band last", () => {
    expect(boxAfterKey(layout, "n:A", "ArrowDown")).toMatch(/^n:[BC]$/);
    expect(boxAfterKey(layout, "n:B", "ArrowUp")).toBe("n:A");
    expect(boxAfterKey(layout, "n:A", "ArrowUp")).toBeNull();
    expect(boxAfterKey(layout, "n:C", "ArrowDown")).toMatch(/^n:LOOSE2?$/);
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowUp")).toMatch(/^n:[BC]$/);
  });

  it("moves along the band, staying at its ends", () => {
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowRight")).toBe("n:LOOSE2");
    expect(boxAfterKey(layout, "n:LOOSE2", "ArrowLeft")).toBe("n:LOOSE");
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowLeft")).toBeNull();
    expect(boxAfterKey(layout, "n:LOOSE2", "ArrowRight")).toBeNull();
    expect(boxAfterKey(layout, "n:LOOSE", "ArrowDown")).toBeNull();
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
