import { describe, expect, it, vi } from "vitest";
import type { SpecEngineClient } from "../api/client";
import type { MapDocument } from "../api/types";
import { aMap, aMapEdge, aMapNode } from "../test/builders";
import { pathsOf, recordingPen } from "../test/pen";
import { kindColours, MAP_SLOTS, paletteOf, slotOf } from "./colours";
import { composeMap, drawBase, drawHighlight, GHOST_RADIUS, labelledOf, labelOf, MAP_DIM_ALPHA, MAP_LABELS, radiusOf, type MapPicture } from "./draw";
import { DEFAULT_FILTERS, keptOf, litOf, MAP_DRAWN_MAX, sceneOf, type Scene } from "./filters";
import { MAP_TICKS, runSlice, startLayout } from "./layout";

// docs/features/corpus-map.md "Acceptance criteria", CM7 (the drawing) and CM12 (the work it bounds),
// over a recording pen: a node's radius follows its uses; a kind fills with its slot's role, slots by
// how many nodes hold each kind, ties by code point, a ninth kind and none with the grey; an orphan gets a dashed ring; a lit
// set draws at full alpha over the base image dimmed to MAP_DIM_ALPHA; the base of 5 000 nodes and
// 15 000 links is a few batched fills and strokes, each node and line once, and a hover draws only
// its highlight. M: slots by a kind's text order; a hover redrawing everything.

/** A palette whose every colour names its role, so a recording reads which role filled what. */
const PALETTE = paletteOf((role) => `role(${role})`, "sans-serif");

const BOX = { width: 800, height: 600, ratio: 1 };

/** A stand-in for the base's offscreen canvas: the pen records where it is drawn. */
const IMAGE = document.createElement("canvas");
const VIEW = { x: 0, y: 0, zoom: 1 };

/**
 * A node set's kinds, in an order no count or text sorts in: zeta_kind held most, then beta_kind and
 * alpha_kind twice each (alpha first by code point), then six kinds once each, of which kind_3 to
 * kind_7 take slots 4 to 8 and ninth_kind and unlisted_kind the grey; no kind the grey.
 */
const KINDS: (string | null)[] = [null, "beta_kind", "kind_5", "zeta_kind", "unlisted_kind", "alpha_kind", "kind_3", "zeta_kind", "ninth_kind", "kind_7", "beta_kind", "kind_4", "alpha_kind", "zeta_kind", "kind_6"];
const COLOURS = kindColours(KINDS);

/** A picture of a scene, each point on a row, with its slots from COLOURS. */
function pictureOf(answer: MapDocument, scene: Scene): MapPicture {
  const count = scene.points.length;
  const positions = new Float64Array(count * 2);
  const radii = new Float64Array(count);
  const slots = new Uint8Array(count);
  const ghost = new Uint8Array(count);
  const orphan = new Uint8Array(count);
  scene.points.forEach((point, at) => {
    positions[at * 2] = at * 50;
    positions[at * 2 + 1] = 0;
    if (point.node === null) {
      ghost[at] = 1;
      radii[at] = GHOST_RADIUS;
      return;
    }
    const node = answer.nodes[point.node];
    radii[at] = radiusOf(node?.used_by ?? 0);
    slots[at] = slotOf(node?.kind ?? null, COLOURS);
    orphan[at] = node?.orphan === true ? 1 : 0;
  });
  const labels = scene.points.map((point) => labelOf(point.node === null ? "" : (answer.nodes[point.node]?.title ?? "")));
  return { positions, radii, slots, ghost, orphan, labels, labelled: labelledOf(radii, ghost), lineFrom: scene.lineFrom, lineTo: scene.lineTo, lineStyle: scene.lineStyle };
}

/** The fill a disc at `x` was drawn in, and its alpha. */
function fillAt(calls: ReturnType<typeof recordingPen>["calls"], x: number) {
  const path = pathsOf(calls).find((candidate) => candidate.op === "fill" && candidate.arcs.some((arc) => arc.x === x));
  return path === undefined ? null : { fill: path.fillStyle, alpha: path.globalAlpha, radius: path.arcs.find((arc) => arc.x === x)?.r };
}

describe("the drawing (CM7)", () => {
  const answer = aMap([
    aMapNode({ name: "A", kind: "alpha_kind", used_by: 0 }),
    aMapNode({ name: "B", kind: "zeta_kind", used_by: 4 }),
    aMapNode({ name: "C", kind: "beta_kind", used_by: 100 }),
    aMapNode({ name: "D", kind: "unlisted_kind", orphan: true }),
    aMapNode({ name: "E", kind: "ninth_kind" }),
    aMapNode({ name: "F", kind: null }),
  ], [aMapEdge(0, 1), aMapEdge(1, 2), aMapEdge(2, 4, { type: "mentions", weak: true })]);
  const scene = sceneOf(answer, false);
  const picture = pictureOf(answer, scene);
  const kept = keptOf(answer, scene, DEFAULT_FILTERS);

  it("sizes each disc 3 + 2 sqrt(used_by), at most 18", () => {
    const pen = recordingPen();
    drawBase(pen, picture, kept, VIEW, BOX, PALETTE);
    expect([0, 50, 100].map((x) => fillAt(pen.calls, x)?.radius)).toEqual([3, 7, 18]);
    expect([radiusOf(0), radiusOf(1), radiusOf(4), radiusOf(56), radiusOf(10_000)]).toEqual([3, 5, 7, 3 + 2 * Math.sqrt(56), 18]);
  });

  it("fills each kind with its slot's role, most held first, ties by code point; a ninth kind and none with the grey (M: slots by a kind's text order)", () => {
    const pen = recordingPen();
    drawBase(pen, picture, kept, VIEW, BOX, PALETTE);
    expect([0, 50, 100, 150, 200, 250].map((x) => fillAt(pen.calls, x)?.fill)).toEqual([
      "role(--map-kind-2)",
      "role(--map-kind-1)",
      "role(--map-kind-3)",
      "role(--map-kind-other)",
      "role(--map-kind-other)",
      "role(--map-kind-other)",
    ]);
  });

  it("colours by kind: ranks the node set's kinds by how many nodes hold each, ties by code point, MAP_SLOTS of them; the rest and no kind the grey, counted", () => {
    expect(MAP_SLOTS).toBe(8);
    expect(COLOURS.coloured.map((each) => [each.kind, each.slot, each.count])).toEqual([
      ["zeta_kind", 1, 3],
      ["alpha_kind", 2, 2],
      ["beta_kind", 3, 2],
      ["kind_3", 4, 1],
      ["kind_4", 5, 1],
      ["kind_5", 6, 1],
      ["kind_6", 7, 1],
      ["kind_7", 8, 1],
    ]);
    expect([slotOf("zeta_kind", COLOURS), slotOf("ninth_kind", COLOURS), slotOf("unlisted_kind", COLOURS), slotOf(null, COLOURS), slotOf("absent_kind", COLOURS)]).toEqual([1, 0, 0, 0, 0]);
    // ninth_kind, unlisted_kind and the node of no kind.
    expect(COLOURS.other).toBe(3);
    // One node set, one colouring, in whatever order its nodes come.
    expect(kindColours([...KINDS].reverse())).toEqual(COLOURS);
    // A kind's text never picks its colour: renamed, the most held keeps slot 1.
    expect(kindColours(KINDS.map((kind) => (kind === "zeta_kind" ? "aaa_kind" : kind))).coloured[0]).toEqual({ kind: "aaa_kind", slot: 1, count: 3 });
    expect(kindColours(KINDS.map((kind) => (kind === "zeta_kind" ? "zzz_kind" : kind))).coloured[0]).toEqual({ kind: "zzz_kind", slot: 1, count: 3 });
    expect(kindColours([null, null])).toEqual({ coloured: [], slots: new Map(), other: 2 });
  });

  it("rings an orphan with a dashed line in the label's role, and draws a ghost hollow and dashed", () => {
    const withGhost = aMap(answer.nodes, answer.edges, { ghosts: [{ written: "GONE", reason: null, sources: [0], places: 1 }] });
    const ghostScene = sceneOf(withGhost, false);
    const ghostPicture = pictureOf(withGhost, ghostScene);
    const pen = recordingPen();
    drawBase(pen, ghostPicture, keptOf(withGhost, ghostScene, { ...DEFAULT_FILTERS, ghosts: true }), VIEW, BOX, PALETTE);
    const strokes = pathsOf(pen.calls).filter((path) => path.op === "stroke" && path.arcs.length > 0);
    const ring = strokes.find((path) => path.strokeStyle === "role(--map-label)");
    expect(ring?.arcs.map((arc) => [arc.x, arc.r])).toEqual([[150, 3 + 3]]);
    expect(ring?.dash.length).toBeGreaterThan(0);
    const ghost = strokes.find((path) => path.strokeStyle === "role(--map-ghost)");
    expect(ghost?.arcs.map((arc) => [arc.x, arc.r])).toEqual([[300, GHOST_RADIUS]]);
    expect(ghost?.dash.length).toBeGreaterThan(0);
    // A ghost is never filled.
    expect(pathsOf(pen.calls).some((path) => path.op === "fill" && path.arcs.some((arc) => arc.x === 300))).toBe(false);
  });

  it("draws a hovered node and its neighbours both ways at full alpha over the base dimmed to 0.15, and nothing else", () => {
    const pen = recordingPen();
    // B (point 1) links A and C: those three and their two lines drawn on top; D, E, F only in the dimmed base.
    composeMap(pen, { image: IMAGE, view: VIEW, box: BOX }, picture, kept, litOf(scene, kept, 1), null, VIEW, BOX, PALETTE);
    const image = pen.calls.filter((call) => call.op === "drawImage");
    expect(image.map((call) => call.globalAlpha)).toEqual([MAP_DIM_ALPHA]);
    expect(image[0]?.args).toEqual([0, 0, BOX.width, BOX.height]);
    expect([0, 50, 100, 150, 200, 250].map((x) => fillAt(pen.calls, x)?.alpha ?? null)).toEqual([1, 1, 1, null, null, null]);
    const lines = pathsOf(pen.calls).filter((path) => path.op === "stroke" && path.segments > 0);
    expect(lines.map((path) => [path.segments, path.globalAlpha])).toEqual([[2, 1]]);
    // With nothing lit, the base image at full alpha and nothing drawn over it.
    const rest = recordingPen();
    composeMap(rest, { image: IMAGE, view: VIEW, box: BOX }, picture, kept, null, null, VIEW, BOX, PALETTE);
    expect(rest.calls.filter((call) => call.op === "drawImage").map((call) => call.globalAlpha)).toEqual([1]);
    expect(rest.calls.filter((call) => call.op === "arc" || call.op === "lineTo")).toEqual([]);
  });

  it("moves and scales the base image from the view it was drawn for to the one shown", () => {
    const pen = recordingPen();
    composeMap(pen, { image: IMAGE, view: { x: 0, y: 0, zoom: 1 }, box: BOX }, picture, kept, null, null, { x: 10, y: -20, zoom: 2 }, BOX, PALETTE);
    // A map point at (0, 0) was at (400, 300); at zoom 2 and view (10, -20) it falls at (380, 340).
    const [dx, dy, dw, dh] = (pen.calls.find((call) => call.op === "drawImage")?.args ?? []).map(Number);
    expect([dx, dy, dw, dh]).toEqual([-420, -260, 1600, 1200]);
    expect((dx ?? 0) + 400 * 2).toBe(380);
    expect((dy ?? 0) + 300 * 2).toBe(340);
  });

  it("labels at most the MAP_LABELS largest; a long title cut with an ellipsis, the card and the List showing it whole", () => {
    expect(labelOf("Short")).toBe("Short");
    expect(Array.from(labelOf("x".repeat(80)))).toHaveLength(32);
    const radii = Float64Array.from({ length: 100 }, (_, at) => at % 7);
    expect(labelledOf(radii, new Uint8Array(100)).reduce((sum, one) => sum + one, 0)).toBe(MAP_LABELS);
  });
});

/** The mock client, loaded as data through Vite: only the bootstrap imports the mocks (policy.test.ts). */
const mock = import.meta.glob<{ MockClient: new (scenario: string, options: { delayMs: number }) => SpecEngineClient }>("/src/mocks/MockClient.ts", { eager: true });

describe("5 000 nodes in bounded work (CM12)", () => {
  it("lays the large map out in exactly MAP_TICKS ticks, draws its base with each node and line once in a few batched calls, and a hover only its highlight (M: a hover redrawing everything)", async () => {
    const MockClient = mock["/src/mocks/MockClient.ts"]?.MockClient;
    if (MockClient === undefined) {
      throw new Error("no mock client");
    }
    const answer = await new MockClient("large", { delayMs: 0 }).getMap("harbor-sim", { archive: false });
    expect([answer.nodes.length, answer.edges.length]).toEqual([5000, 15000]);
    const scene = sceneOf(answer, false);
    // The cap holds: at most MAP_DRAWN_MAX drawn.
    expect(scene.points.filter((point) => point.node !== null).length).toBeLessThanOrEqual(MAP_DRAWN_MAX);
    const layout = startLayout(scene.points.length, Array.from(scene.lineFrom, (from, at) => ({ source: from, target: scene.lineTo[at] ?? from })));
    const tick = vi.spyOn(layout.simulation, "tick");
    let clock = 0;
    while (!layout.done) {
      runSlice(layout, () => (clock += 4));
    }
    expect(tick).toHaveBeenCalledTimes(MAP_TICKS);
    const positions = layout.positions();
    expect(Array.from(positions).every(Number.isFinite)).toBe(true);
    const radii = new Float64Array(scene.points.length).fill(4);
    const picture: MapPicture = {
      positions,
      radii,
      slots: new Uint8Array(scene.points.length).map((_, at) => at % 9),
      ghost: new Uint8Array(scene.points.length),
      orphan: new Uint8Array(scene.points.length),
      labels: scene.points.map(() => "label"),
      labelled: labelledOf(radii, new Uint8Array(scene.points.length)),
      lineFrom: scene.lineFrom,
      lineTo: scene.lineTo,
      lineStyle: scene.lineStyle,
    };
    const kept = keptOf(answer, scene, DEFAULT_FILTERS);
    const keptPoints = kept.points.reduce((sum, one) => sum + one, 0);
    const keptLines = kept.lines.reduce((sum, one) => sum + one, 0);
    // The base: each kept node and line once, in a few batched calls.
    const basePen = recordingPen();
    const base = drawBase(basePen, picture, kept, VIEW, BOX, PALETTE);
    expect([base.nodes, base.lines]).toEqual([keptPoints, keptLines]);
    expect(basePen.calls.filter((call) => call.op === "arc").length).toBe(keptPoints);
    expect(base.fills).toBeLessThanOrEqual(9);
    expect(base.strokes).toBeLessThanOrEqual(4 + 2);
    expect(base.labels).toBeLessThanOrEqual(MAP_LABELS);
    // A hover on the most linked node: the base image once, then its highlight alone (M: a hover redrawing everything).
    const hub = Array.from(scene.incident.keys()).sort((a, b) => (scene.incident[b]?.length ?? 0) - (scene.incident[a]?.length ?? 0))[0] ?? 0;
    const lit = litOf(scene, kept, hub);
    const litPoints = lit.pointList.filter((point) => kept.points[point] === 1).length;
    // The busiest hub of the large map, its worst case: still a fraction of the whole.
    expect(lit.lineList.length).toBeGreaterThan(20);
    expect(litPoints).toBeLessThan(keptPoints / 4);
    expect(lit.lineList.length).toBeLessThan(keptLines / 10);
    const hoverPen = recordingPen();
    const hover = composeMap(hoverPen, { image: IMAGE, view: VIEW, box: BOX }, picture, kept, lit, hub, VIEW, BOX, PALETTE);
    expect([hover.nodes, hover.lines, hover.labels]).toEqual([litPoints, lit.lineList.length, litPoints]);
    expect(hoverPen.calls.filter((call) => call.op === "drawImage")).toHaveLength(1);
    // The highlight's discs and the focus ring: no other disc is drawn.
    expect(hoverPen.calls.filter((call) => call.op === "arc").length).toBe(litPoints + 1);
    expect(hoverPen.calls.filter((call) => call.op === "lineTo").length).toBe(lit.lineList.length);
    expect(hoverPen.calls.length).toBeLessThan(10 * (litPoints + lit.lineList.length) + 50);
    // A search lighting a few matches draws those alone.
    const highlight = drawHighlight(recordingPen(), picture, kept, { ...lit, lineList: [], lines: new Uint8Array(lit.lines.length) }, VIEW, BOX, PALETTE);
    expect([highlight.nodes, highlight.lines]).toEqual([litPoints, 0]);
    // The bound is the work counted above, never a time: a loaded machine takes longer to count it.
  }, 120_000);
});
