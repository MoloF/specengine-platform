import { describe, expect, it } from "vitest";
import type { FlowNode } from "../api/types";
import { asideFiring, asideLens } from "../test/asideStub";
import { aFlowEdge, aFlowNode, aFlowView } from "../test/builders";
import { aLine, flow200, forwardCrossings, TYPE_B } from "../test/flowStub";
import { fitOf, pointInside, stretchCrosses, type Point } from "./geometry";
import { BOX_HEIGHT, BOX_WIDTH, BOX_GAP, boxAfterKey, CANVAS_LIMIT, COLUMN_GAP, LABEL_WIDTH, labelSize, layoutFlow, LANE_PAD, type FlowBox, type FlowLayout, type LaidEdge } from "./layout";

// docs/features/flow-canvas.md "Layout" and docs/features/handoff-progress.md "Acceptance criteria",
// F-1 to F-3 and F-5: the walk left to right, a column per distance, stage dividers left of the
// columns where stages begin, arcs on their own tracks below the walk, the band below the tracks;
// AC-11's cap, AC-12's trial of owner's decision 11 and the keys' moves; one read, one lane
// (docs/features/writer-studio.md D1). Pure: a read in, positions out.

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

/** The lowest point of a line: an arc's track. */
function lowestY(laid: LaidEdge): number {
  return Math.max(...laid.points.map((point) => point.y));
}

/** The bottom of the walk's lowest box. */
function walkBottom(layout: FlowLayout): number {
  return Math.max(...layout.boxes.filter((one) => one.level !== null).map((one) => one.y + one.height));
}

describe("columns (AC-04, F-1)", () => {
  it("runs boxes at distance 0, 4 and 9 left to right: a column per distinct distance, no empty column, one beside the other", () => {
    const layout = layoutFlow(aFlowView([aFlowNode({ id: "NINE", distance: 9 }), aFlowNode({ id: "ZERO", distance: 0 }), aFlowNode({ id: "FOUR", distance: 4 })], []));
    expect(box(layout, "ZERO").x).toBeLessThan(box(layout, "FOUR").x);
    expect(box(layout, "FOUR").x).toBeLessThan(box(layout, "NINE").x);
    expect(["ZERO", "FOUR", "NINE"].map((name) => box(layout, name).level)).toEqual([0, 1, 2]);
    expect(layout.lanes[0]?.columns).toEqual([["n:ZERO"], ["n:FOUR"], ["n:NINE"]]);
    // M: `y` by distance; a walk with one box a column stands as one row.
    expect(new Set(["ZERO", "FOUR", "NINE"].map((name) => box(layout, name).y)).size).toBe(1);
    expect(box(layout, "FOUR").x - (box(layout, "ZERO").x + BOX_WIDTH)).toBeGreaterThanOrEqual(COLUMN_GAP);
  });

  it("on the firing run: x grows with distance, one distance shares one x, Mending and Packing one above the other", () => {
    const layout = layoutFlow(firingBoxes());
    const steps = layout.boxes.filter((one) => one.node.distance !== null).sort((a, b) => (a.node.distance ?? 0) - (b.node.distance ?? 0));
    steps.forEach((one, index) => {
      const next = steps[index + 1];
      if (next !== undefined) {
        expect([one.name, next.name, one.node.distance === next.node.distance ? one.x === next.x : one.x < next.x]).toEqual([one.name, next.name, true]);
      }
    });
    const [mend, pack] = [box(layout, "POT-MEND"), box(layout, "POT-PACK")];
    expect(mend.x).toBe(pack.x);
    expect(Math.abs(mend.y - pack.y)).toBeGreaterThanOrEqual(BOX_HEIGHT + BOX_GAP);
    expect(layout.lanes[0]?.columns).toHaveLength(9);
    expect(layout.lanes[0]?.columns.every((column) => column.length > 0)).toBe(true);
    expect(overlapping(layout.boxes)).toEqual([]);
  });

  it("runs every forward edge left to right: its source's right edge left of its target's left edge, its points clear of every box", () => {
    const layout = layoutFlow(firingBoxes());
    const forward = layout.edges.filter((laid) => laid.route === "forward");
    expect(forward.length).toBeGreaterThan(5);
    for (const laid of forward) {
      const [from, to] = [layout.boxes.find((one) => one.id === laid.source), layout.boxes.find((one) => one.id === laid.target)];
      expect([laid.id, (from?.x ?? 0) + BOX_WIDTH < (to?.x ?? 0)]).toEqual([laid.id, true]);
      expect([laid.id, crossesABox(laid, layout).map((one) => one.name)]).toEqual([laid.id, []]);
    }
  });

  it("stands four dividers, wheelwork, bisque, glazing and crating, left of columns 0, 3, 5 and 7; none in a stage view (F-3)", () => {
    const layout = layoutFlow(firingBoxes());
    const lane = layout.lanes[0];
    expect(lane?.dividers.map((divider) => [divider.level, divider.stages])).toEqual([
      [0, ["wheelwork"]],
      [3, ["bisque"]],
      [5, ["glazing"]],
      [7, ["crating"]],
    ]);
    const columnOf = (level: number) => (lane?.columns[level] ?? []).map((id) => layout.boxes.find((one) => one.id === id));
    const lowestTrack = Math.max(...layout.edges.filter((laid) => laid.route === "arc").map(lowestY));
    for (const divider of lane?.dividers ?? []) {
      const own = columnOf(divider.level);
      const before = columnOf(divider.level - 1);
      // M: a divider at its column's right edge.
      expect([divider.level, own.every((one) => divider.x < (one?.x ?? 0))]).toEqual([divider.level, true]);
      expect([divider.level, before.every((one) => divider.x > (one?.x ?? 0) + BOX_WIDTH)]).toEqual([divider.level, true]);
      // The rule from the words' strip, above every walk box, to below the lowest arc track.
      expect([divider.level, layout.boxes.every((one) => one.level === null || divider.y < one.y)]).toEqual([divider.level, true]);
      expect([divider.level, divider.y + divider.height > lowestTrack]).toEqual([divider.level, true]);
      expect([divider.level, divider.x + divider.width]).toEqual([divider.level, (own[0]?.x ?? 0) + BOX_WIDTH]);
    }
    // M: a divider per column.
    expect(lane?.dividers).toHaveLength(4);
    const glazing = firingBoxes();
    const stage = layoutFlow({ ...glazing, nodes: glazing.nodes.filter((node) => node.stage === "glazing") });
    expect(stage.boxes).toHaveLength(2);
    expect(stage.lanes[0]?.dividers).toEqual([]);
  });

  it("draws no divider for a picture of fewer than two stages, and none for a band box's stage (F-3)", () => {
    const one = layoutFlow(aFlowView([aFlowNode({ id: "A", distance: 0, stage: "only" }), aFlowNode({ id: "B", distance: 1, stage: "only" }), aFlowNode({ id: "C", distance: null, stage: "elsewhere" })], []));
    expect(one.lanes[0]?.dividers).toEqual([]);
  });

  it("shares one divider between stages beginning in one column, their words in column order (F-3)", () => {
    const layout = layoutFlow(
      aFlowView(
        [aFlowNode({ id: "A", distance: 0, stage: "first" }), aFlowNode({ id: "B", distance: 1, stage: "second", path: "docs/a.md" }), aFlowNode({ id: "C", distance: 1, stage: "third", path: "docs/b.md" })],
        [],
      ),
    );
    expect(layout.lanes[0]?.dividers.map((divider) => divider.level)).toEqual([0, 1]);
    const column = (layout.lanes[0]?.columns[1] ?? []).map((id) => layout.boxes.find((one) => one.id === id)?.node.stage);
    expect(layout.lanes[0]?.dividers[1]?.stages).toEqual(column);
  });

  it("lays the boxes off the walk in the band below the walk, its caption over them, wrapped at four boxes (F-2)", () => {
    const layout = layoutFlow(aFlowView([...aLine(["A", "B"]), ...["D1", "D2", "D3", "D4", "D5", "D6"].map((id) => aFlowNode({ id, distance: null }))], []));
    const band = layout.lanes[0]?.band ?? [];
    expect(band.map((shelf) => shelf.length)).toEqual([4, 2]);
    const bottom = walkBottom(layout);
    expect(layout.boxes.filter((one) => one.level === null).every((one) => one.y > bottom)).toBe(true);
    expect(layout.lanes[0]?.bandY).toBeGreaterThan(bottom);
    expect(layout.lanes[0]?.bandY).toBeLessThan(box(layout, "D1").y);
  });

  it("at the pottery's lens root, the gallery and the run in column 0, the nine parts in the band, its edges off the walk", () => {
    const read = asideLens();
    const layout = layoutFlow({ ...read, nodes: read.nodes.filter((node) => node.id !== "ATL-POTTERY") });
    expect(layout.lanes[0]?.columns.map((column) => column.length)).toEqual([2]);
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
    const middle = (box(layout, "UP").y + box(layout, "DOWN").y) / 2;
    expect(box(layout, "JOIN").y).toBeCloseTo(middle, 0);
    expect(box(layout, "S").y).toBeCloseTo(middle, 0);
  });

  it("orders places by the neighbours: two parallel lines never swap", () => {
    const nodes = [aFlowNode({ id: "A1", distance: 0 }), aFlowNode({ id: "A2", distance: 0 }), aFlowNode({ id: "B1", distance: 1, path: "docs/spec/z.md" }), aFlowNode({ id: "B2", distance: 1, path: "docs/spec/a.md" })];
    const layout = layoutFlow(aFlowView(nodes, [aFlowEdge({ from: "A1", to: "B1" }), aFlowEdge({ from: "A2", to: "B2" })]));
    expect(forwardCrossings(layout)).toBe(0);
    expect(box(layout, "A1").y < box(layout, "A2").y).toBe(box(layout, "B1").y < box(layout, "B2").y);
  });
});

/** The boxes an edge's words, as `labelSize` measures them round their point, lie on. */
function labelOnBoxes(laid: LaidEdge, layout: FlowLayout): string[] {
  const { width, height } = labelSize(laid.edge);
  if (width === 0) {
    return [];
  }
  const rect = { x: laid.label.x - width / 2, y: laid.label.y - height / 2, width, height };
  return layout.boxes.filter((one) => rect.x < one.x + one.width && one.x < rect.x + rect.width && rect.y < one.y + one.height && one.y < rect.y + rect.height).map((one) => `${laid.id} on ${one.name}`);
}

// The orchestrator's look L1 of handoff-progress: the pitch tightened, so more of a long walk stands in
// a 1440 px studio with both panes open; the edge words wrap within the gap, two lines at most.
describe("the column pitch (L1 of the looks)", () => {
  /** The studio's canvas at 1440 px, both panes open: 1440 less the nav (13rem) and its border, main's padding (2 x 1.5rem), the places (15rem), the panel (22rem) and two gaps (0.75rem). */
  const CANVAS_1440 = 1440 - 208 - 1 - 48 - 240 - 352 - 24;

  it("steps a column 224 px: a box and a 64 px gap, the edge words' width and their insets", () => {
    const layout = layoutFlow(aFlowView(aLine(["A", "B", "C"]), [aFlowEdge({ from: "A", to: "B" }), aFlowEdge({ from: "B", to: "C" })]));
    // M: the gap back to 136.
    expect(box(layout, "B").x - box(layout, "A").x).toBe(224);
    expect(box(layout, "C").x - box(layout, "B").x).toBe(BOX_WIDTH + COLUMN_GAP);
    expect(LABEL_WIDTH + 8).toBe(COLUMN_GAP);
  });

  it("shows two and a half pitches of the firing run in a 1440 px studio at the fit, the third column's box begun in view", () => {
    expect(CANVAS_1440).toBe(567);
    const layout = layoutFlow(firingBoxes());
    const fit = fitOf(layout, CANVAS_1440, 600);
    expect(fit?.zoom).toBe(0.92);
    const zoom = fit?.zoom ?? 1;
    const lefts = (layout.lanes[0]?.columns ?? []).map((column) => (fit?.x ?? 0) + (layout.boxes.find((one) => one.id === column[0])?.x ?? 0) * zoom);
    expect(lefts.filter((left) => left < CANVAS_1440)).toHaveLength(3);
    expect((CANVAS_1440 - (lefts[0] ?? 0)) / ((BOX_WIDTH + COLUMN_GAP) * zoom)).toBeGreaterThan(2.5);
  });

  it("keeps every edge's words off every box: in a gap, under a loop, on a track or a trunk", () => {
    const nodes = [...aLine(["A", "B", "C", "D"]), aFlowNode({ id: "A2", distance: 0 }), aFlowNode({ id: "A3", distance: 0, path: "docs/z.md" }), aFlowNode({ id: "DEF", distance: null }), aFlowNode({ id: "DEF2", distance: null, path: "docs/y.md" })];
    const said = { kind: "alternative", when: "the glaze has crawled over the foot ring" };
    const edges = [
      aFlowEdge({ from: "A", to: "B", ...said }),
      aFlowEdge({ from: "A", to: "B", kind: "parallel", when: "both kilns" }),
      aFlowEdge({ from: "D", to: "A", kind: "repeat", when: "again" }),
      aFlowEdge({ from: "C", to: "B", kind: "repeat" }),
      aFlowEdge({ from: "A", to: "A2", ...said }),
      aFlowEdge({ from: "A", to: "A3", ...said }),
      aFlowEdge({ from: "B", to: "B", ...said }),
      aFlowEdge({ from: "C", to: "DEF", ...said }),
      aFlowEdge({ from: "DEF", to: "DEF2", ...said }),
    ];
    const pictures = [layoutFlow(aFlowView(nodes, edges)), layoutFlow(firingBoxes()), layoutFlow(flow200())];
    for (const layout of pictures) {
      expect(layout.edges.flatMap((laid) => labelOnBoxes(laid, layout))).toEqual([]);
    }
    // Two lines at most, within the gap.
    expect(labelSize(aFlowEdge({ from: "A", to: "B", ...said }))).toEqual({ width: LABEL_WIDTH, height: 2 * 16 + 6 });
  });
});

describe("edges (AC-05, F-1, F-2): columns decide the shape, never a kind", () => {
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

  it("arcs a backward edge with a kind and one without; a forward edge with a kind runs right", () => {
    expect(edge(layout, "D", "A").route).toBe("arc");
    expect(edge(layout, "C", "B").route).toBe("arc");
    expect(edge(layout, "A", "B").route).toBe("forward");
    expect(edge(layout, "B", "C").route).toBe("forward");
  });

  it("joins two boxes of a column, and puts an edge to itself under its box, clear of the box below it", () => {
    const beside = edge(layout, "A", "A2");
    expect(beside.route).toBe("beside");
    expect(crossesABox(beside, layout)).toEqual([]);
    const loop = edge(layout, "B", "B");
    const b = box(layout, "B");
    expect(loop.route).toBe("self");
    expect(loop.points.every((point) => point.y === b.y + BOX_HEIGHT)).toBe(true);
    expect(loop.points.every((point) => point.x > b.x && point.x < b.x + BOX_WIDTH)).toBe(true);
    expect(loop.label.y).toBeGreaterThan(b.y + BOX_HEIGHT);
    const below = layout.boxes.filter((one) => one.level === b.level && one.y > b.y);
    for (const one of below) {
      expect(one.y).toBeGreaterThan(loop.label.y);
    }
  });

  it("keeps a box clear of the loop under its neighbour", () => {
    const b = box(layout, "B");
    const b2 = box(layout, "B2");
    expect(b2.y > b.y ? b2.y - (b.y + BOX_HEIGHT) : b.y - (b2.y + BOX_HEIGHT)).toBeGreaterThan(BOX_GAP);
  });

  it("lets no arc point fall in a box: it runs through the gaps, on its own track below the walk (F-2)", () => {
    const arcs = layout.edges.filter((laid) => laid.route === "arc");
    expect(arcs.length).toBe(2);
    const bottom = walkBottom(layout);
    for (const arc of arcs) {
      expect(crossesABox(arc, layout)).toEqual([]);
      expect(arc.points.some((point) => layout.boxes.some((one) => pointInside(point, one)))).toBe(false);
      // M: arcs on tracks right of the walk.
      expect(lowestY(arc)).toBeGreaterThan(bottom);
    }
    expect(new Set(arcs.map(lowestY)).size).toBe(2);
  });

  it("gives the shorter arc the track nearer the walk, so the longer wraps it (F-2)", () => {
    expect(lowestY(edge(layout, "C", "B"))).toBeLessThan(lowestY(edge(layout, "D", "A")));
  });

  it("runs the firing run's repeat, Glost firing back to Dipping in glaze, below the walk out of one bottom and into the other, crossing no box (F-2)", () => {
    const firing = layoutFlow(firingBoxes());
    const repeat = edge(firing, "POT-FIRE", "POT-GLAZE");
    expect(repeat.route).toBe("arc");
    expect(crossesABox(repeat, firing)).toEqual([]);
    const [fire, glaze] = [box(firing, "POT-FIRE"), box(firing, "POT-GLAZE")];
    expect(repeat.points[0]?.y).toBe(fire.y + BOX_HEIGHT);
    expect(repeat.points.at(-1)?.y).toBe(glaze.y + BOX_HEIGHT);
    // Every point at or below the bottoms it leaves and enters; its track below every walk box.
    expect(repeat.points.every((point) => point.y >= Math.min(fire.y, glaze.y) + BOX_HEIGHT)).toBe(true);
    expect(lowestY(repeat)).toBeGreaterThan(walkBottom(firing));
    expect(firing.edges.filter((laid) => laid.offWalk)).toEqual([]);
    for (const laid of firing.edges) {
      expect([laid.id, crossesABox(laid, firing).map((one) => one.name)]).toEqual([laid.id, []]);
    }
  });

  it("keeps a lane's own margin round the walk", () => {
    expect(box(layout, "A").x).toBeGreaterThanOrEqual(LANE_PAD);
    expect(box(layout, "A").y).toBeGreaterThanOrEqual(LANE_PAD);
    const right = Math.max(...layout.boxes.map((candidate) => candidate.x + candidate.width));
    expect(layout.width).toBeGreaterThanOrEqual(right + LANE_PAD);
    expect(layout.height).toBeGreaterThan(Math.max(...layout.edges.map(lowestY)));
  });

  it("carries a long forward edge past the columns between: a passage in each column it skips (F-1)", () => {
    const skipping = layoutFlow(
      aFlowView(
        [...aLine(["P", "Q", "R", "S"]), aFlowNode({ id: "Q2", distance: 1 }), aFlowNode({ id: "R2", distance: 2 })],
        [aFlowEdge({ from: "P", to: "S" }), aFlowEdge({ from: "P", to: "Q" }), aFlowEdge({ from: "Q", to: "R" }), aFlowEdge({ from: "R", to: "S" }), aFlowEdge({ from: "P", to: "Q2" }), aFlowEdge({ from: "Q2", to: "R2" })],
      ),
    );
    const long = edge(skipping, "P", "S");
    expect(long.route).toBe("forward");
    // Its runs: out of P, across columns 1 and 2, into S.
    expect(long.points).toHaveLength(8);
    expect(crossesABox(long, skipping)).toEqual([]);
    expect(overlapping(skipping.boxes)).toEqual([]);
  });

  it("routes an edge to the band down the gap right of its source, over the band, into the box's top, off the walk (F-2)", () => {
    const banded = layoutFlow(aFlowView([...aLine(["A", "B", "C"]), aFlowNode({ id: "DEF", distance: null })], [aFlowEdge({ from: "A", to: "DEF", type: TYPE_B }), aFlowEdge({ from: "C", to: "DEF", type: TYPE_B })]));
    const first = edge(banded, "A", "DEF");
    expect(first.route).toBe("band");
    expect(first.offWalk).toBe(true);
    expect(crossesABox(first, banded)).toEqual([]);
    expect(first.points[first.points.length - 1]?.y).toBe(box(banded, "DEF").y);
    expect(crossesABox(edge(banded, "C", "DEF"), banded)).toEqual([]);
  });

  it("feeds a step from the band up the gap left of its column, into its left side", () => {
    const banded = layoutFlow(aFlowView([...aLine(["A", "B"]), aFlowNode({ id: "DEF", distance: null })], [aFlowEdge({ from: "DEF", to: "B", type: TYPE_B })]));
    const fed = edge(banded, "DEF", "B");
    expect(fed.route).toBe("band");
    expect(fed.points[fed.points.length - 1]?.x).toBe(box(banded, "B").x);
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
    expect(first?.points[0]?.y).not.toBe(second?.points[0]?.y);
    expect(first?.id).not.toBe(second?.id);
  });

  it("sets the words of two edges between one pair of boxes apart", () => {
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

  it("holds 200 boxes, 12 columns, 30 off the walk, 40 edges against it (F-1)", () => {
    expect(layout.boxes).toHaveLength(200);
    expect(layout.lanes[0]?.columns).toHaveLength(12);
    expect(layout.boxes.filter((candidate) => candidate.level === null)).toHaveLength(30);
    expect(layout.lanes[0]?.band.flat()).toHaveLength(30);
    expect(layout.edges.filter((laid) => laid.route === "arc")).toHaveLength(40);
    expect(layout.notDrawn).toBe(0);
  });

  it("overlaps no box", () => {
    expect(overlapping(layout.boxes)).toEqual([]);
  });

  it("runs every forward edge right, clear of every box (F-1)", () => {
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

  it("clears every box with every arc and every edge to the band (F-2)", () => {
    const hits = layout.edges.filter((laid) => laid.route === "arc" || laid.route === "band").flatMap((laid) => crossesABox(laid, layout).map((crossed) => `${laid.id} through ${crossed.name}`));
    expect(hits).toEqual([]);
  });

  it("gives each arc its own track below the walk, the band below the lowest (F-2)", () => {
    const tracks = layout.edges.filter((laid) => laid.route === "arc").map(lowestY);
    expect(new Set(tracks).size).toBe(40);
    expect(Math.min(...tracks)).toBeGreaterThan(walkBottom(layout));
    const band = layout.boxes.filter((candidate) => candidate.level === null);
    expect(Math.min(...band.map((one) => one.y))).toBeGreaterThan(Math.max(...tracks));
    expect(layout.lanes[0]?.bandY).toBeGreaterThan(Math.max(...tracks));
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

  it("orders the columns: a floor against an ordering gone wrong, on wiring this random", () => {
    expect(forwardCrossings(layout)).toBeLessThan(2500);
  });
});

describe("keys' moves (F-5)", () => {
  // Column 0: A; column 1: B over C; column 2: D; the band: LOOSE, LOOSE2 on one shelf, LOOSE3 under it at the left.
  const layout = layoutFlow(
    aFlowView(
      [
        aFlowNode({ id: "A", distance: 0 }),
        aFlowNode({ id: "B", distance: 1, path: "docs/b.md" }),
        aFlowNode({ id: "C", distance: 1, path: "docs/c.md" }),
        aFlowNode({ id: "D", distance: 2 }),
        ...["LOOSE", "LOOSE2", "LOOSE3", "LOOSE4", "LOOSE5"].map((id, index) => aFlowNode({ id, distance: null, path: `docs/loose${String(index)}.md` })),
      ],
      [aFlowEdge({ from: "A", to: "B" }), aFlowEdge({ from: "A", to: "C" }), aFlowEdge({ from: "C", to: "D" })],
    ),
  );
  const lane = layout.lanes[0];
  const [upper, lower] = lane?.columns[1] ?? [];

  it("lays the keys' picture as the tests read it", () => {
    expect(lane?.columns.map((column) => column.length)).toEqual([1, 2, 1]);
    expect(lane?.band.map((shelf) => shelf.length)).toEqual([4, 1]);
  });

  it("goes right and left to the nearest box, by vertical centre, of the next or previous column, and stays at either end", () => {
    const centre = (id: string | undefined) => {
      const found = layout.boxes.find((one) => one.id === id);
      return (found?.y ?? 0) + (found?.height ?? 0) / 2;
    };
    const fromA = boxAfterKey(layout, "n:A", "ArrowRight");
    const nearest = Math.abs(centre(upper) - centre("n:A")) <= Math.abs(centre(lower) - centre("n:A")) ? upper : lower;
    expect(fromA).toBe(nearest);
    expect(boxAfterKey(layout, "n:D", "ArrowLeft")).toBe(Math.abs(centre(upper) - centre("n:D")) < Math.abs(centre(lower) - centre("n:D")) ? upper : lower);
    expect(boxAfterKey(layout, upper ?? "", "ArrowLeft")).toBe("n:A");
    expect(boxAfterKey(layout, lower ?? "", "ArrowRight")).toBe("n:D");
    expect(boxAfterKey(layout, "n:A", "ArrowLeft")).toBeNull();
    expect(boxAfterKey(layout, "n:D", "ArrowRight")).toBeNull();
  });

  it("goes up and down within a column, and down from its last box to the band's first shelf, nearest by horizontal centre", () => {
    // M: Up and Down going across columns.
    expect(boxAfterKey(layout, upper ?? "", "ArrowDown")).toBe(lower);
    expect(boxAfterKey(layout, lower ?? "", "ArrowUp")).toBe(upper);
    expect(boxAfterKey(layout, upper ?? "", "ArrowUp")).toBeNull();
    expect(boxAfterKey(layout, "n:A", "ArrowUp")).toBeNull();
    const shelf = lane?.band[0] ?? [];
    const middleX = (id: string | undefined) => {
      const found = layout.boxes.find((one) => one.id === id);
      return (found?.x ?? 0) + (found?.width ?? 0) / 2;
    };
    const nearestOf = (at: number) => [...shelf].sort((a, b) => Math.abs(middleX(a) - at) - Math.abs(middleX(b) - at))[0];
    expect(boxAfterKey(layout, "n:A", "ArrowDown")).toBe(nearestOf(middleX("n:A")));
    expect(boxAfterKey(layout, "n:D", "ArrowDown")).toBe(nearestOf(middleX("n:D")));
    expect(boxAfterKey(layout, lower ?? "", "ArrowDown")).toBe(nearestOf(middleX(lower)));
  });

  it("moves along a band shelf, crosses shelves, and goes up from the first shelf to the nearest column's last box", () => {
    const [first, second] = lane?.band[0] ?? [];
    const [under] = lane?.band[1] ?? [];
    expect(boxAfterKey(layout, first ?? "", "ArrowRight")).toBe(second);
    expect(boxAfterKey(layout, second ?? "", "ArrowLeft")).toBe(first);
    expect(boxAfterKey(layout, first ?? "", "ArrowLeft")).toBeNull();
    expect(boxAfterKey(layout, first ?? "", "ArrowDown")).toBe(under);
    expect(boxAfterKey(layout, under ?? "", "ArrowUp")).toBe(first);
    expect(boxAfterKey(layout, under ?? "", "ArrowDown")).toBeNull();
    expect(boxAfterKey(layout, first ?? "", "ArrowUp")).toBe("n:A");
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
